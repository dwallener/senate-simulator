use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    eval::timeline::build_historical_timelines,
    ingest::{load_snapshot, run_daily_ingestion_with_roots},
    model::{
        action_alignment::AlignmentReport,
        actual_trajectory::{ActualTrajectory, ActualTrajectoryEvent},
        data_snapshot::DataSnapshot,
        evaluation_example::EvaluationExample,
        historical_timeline::HistoricalTimeline,
        senate_event::SenateEvent,
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationArtifacts {
    pub examples: Vec<EvaluationExample>,
    pub trajectories: Vec<ActualTrajectory>,
    pub alignment_report: AlignmentReport,
}

pub fn build_evaluation_artifacts_for_snapshot_date(
    snapshot_date: NaiveDate,
) -> Result<EvaluationArtifacts, SenateSimError> {
    build_evaluation_artifacts_for_snapshot_date_with_roots(
        snapshot_date,
        Path::new("data"),
        Path::new("fixtures/ingest"),
    )
}

pub fn build_evaluation_artifacts_for_snapshot_date_with_roots(
    snapshot_date: NaiveDate,
    data_root: &Path,
    fixture_root: &Path,
) -> Result<EvaluationArtifacts, SenateSimError> {
    let snapshot = match load_snapshot(data_root, snapshot_date) {
        Ok(snapshot) => snapshot,
        Err(_) => run_daily_ingestion_with_roots(snapshot_date, fixture_root, data_root)?,
    };

    let future_actions = collect_actions_from_snapshot_date(data_root, fixture_root, snapshot_date)?;
    let timelines = build_historical_timelines(&future_actions)?;
    let artifacts = build_evaluation_artifacts(&snapshot, &timelines, 3, Some(30))?;
    persist_evaluation_artifacts(data_root, snapshot_date, &artifacts)?;
    Ok(artifacts)
}

pub fn build_evaluation_artifacts(
    snapshot: &DataSnapshot,
    timelines: &[HistoricalTimeline],
    max_steps: usize,
    horizon_days: Option<i64>,
) -> Result<EvaluationArtifacts, SenateSimError> {
    let examples = generate_next_event_examples(snapshot, timelines)?;
    let mut trajectories = Vec::new();
    let mut ambiguous_actions = 0usize;
    let mut unaligned_consequential_actions = 0usize;

    for timeline in timelines {
        for event in &timeline.events {
            if event.is_consequential && event.aligned_senate_event.is_none() {
                ambiguous_actions += 1;
                unaligned_consequential_actions += 1;
            }
        }
    }

    for record in &snapshot.legislative_records {
        if let Some(timeline) = timelines.iter().find(|timeline| timeline.object_id == record.object_id)
        {
            trajectories.push(generate_actual_trajectory(
                snapshot,
                timeline,
                max_steps,
                horizon_days,
            )?);
        }
    }

    let alignment_report = AlignmentReport {
        snapshot_date: snapshot.snapshot_date,
        objects_processed: snapshot.legislative_records.len(),
        examples_generated: examples.len(),
        ambiguous_actions,
        unaligned_consequential_actions,
        notes: vec![
            format!("evaluation artifacts built from snapshot {}", snapshot.snapshot_date),
            "labels only use consequential aligned events strictly after the snapshot date"
                .to_string(),
        ],
    };

    Ok(EvaluationArtifacts {
        examples,
        trajectories,
        alignment_report,
    })
}

pub fn generate_next_event_examples(
    snapshot: &DataSnapshot,
    timelines: &[HistoricalTimeline],
) -> Result<Vec<EvaluationExample>, SenateSimError> {
    snapshot.validate()?;

    let snapshot_path = format!("data/snapshots/{}/snapshot.json", snapshot.snapshot_date);
    let mut examples = Vec::new();

    for (index, record) in snapshot.legislative_records.iter().enumerate() {
        let example_id = format!(
            "{}:{}:{}",
            snapshot.snapshot_date.format("%Y%m%d"),
            record.object_id,
            index
        );
        let timeline = timelines.iter().find(|timeline| timeline.object_id == record.object_id);

        let (actual_next_event, actual_next_event_date, timeline_position, notes) =
            match timeline.and_then(|timeline| first_future_consequential_event(snapshot.snapshot_date, timeline)) {
                Some((event_index, event)) => (
                    event.aligned_senate_event
                        .clone()
                        .or(Some(SenateEvent::NoMeaningfulMovement)),
                    Some(event.action_date),
                    event_index,
                    vec![format!(
                        "aligned next event selected strictly after snapshot date at {}",
                        event.action_date
                    )],
                ),
                None => (
                    Some(SenateEvent::NoMeaningfulMovement),
                    None,
                    0,
                    vec!["no consequential future event found; labeled as NoMeaningfulMovement"
                        .to_string()],
                ),
            };

        examples.push(EvaluationExample {
            example_id,
            snapshot_date: snapshot.snapshot_date,
            object_id: record.object_id.clone(),
            current_stage: Some(record.current_stage.clone()),
            actual_next_event,
            actual_next_event_date,
            snapshot_path: snapshot_path.clone(),
            timeline_position,
            notes,
        });
    }

    Ok(examples)
}

pub fn generate_actual_trajectory(
    snapshot: &DataSnapshot,
    timeline: &HistoricalTimeline,
    max_steps: usize,
    horizon_days: Option<i64>,
) -> Result<ActualTrajectory, SenateSimError> {
    let mut events = Vec::new();

    for event in &timeline.events {
        if !event.is_consequential || event.action_date <= snapshot.snapshot_date {
            continue;
        }
        let Some(aligned_event) = event.aligned_senate_event.clone() else {
            continue;
        };
        if let Some(days) = horizon_days {
            if (event.action_date - snapshot.snapshot_date).num_days() > days {
                break;
            }
        }

        events.push(ActualTrajectoryEvent {
            event: aligned_event.clone(),
            event_date: event.action_date,
            source_action_text: event.raw_action_text.clone(),
        });

        if events.len() >= max_steps || is_terminal(&aligned_event) {
            break;
        }
    }

    Ok(ActualTrajectory {
        snapshot_date: snapshot.snapshot_date,
        object_id: timeline.object_id.clone(),
        events,
        horizon_days,
        max_steps,
    })
}

pub fn persist_evaluation_artifacts(
    data_root: &Path,
    snapshot_date: NaiveDate,
    artifacts: &EvaluationArtifacts,
) -> Result<(), SenateSimError> {
    let dir = evaluation_storage_dir(data_root, snapshot_date);
    write_json(&dir.join("examples.json"), &artifacts.examples)?;
    write_json(&dir.join("trajectories.json"), &artifacts.trajectories)?;
    write_json(&dir.join("alignment_report.json"), &artifacts.alignment_report)?;
    Ok(())
}

pub fn load_evaluation_artifacts(
    data_root: &Path,
    snapshot_date: NaiveDate,
) -> Result<EvaluationArtifacts, SenateSimError> {
    let dir = evaluation_storage_dir(data_root, snapshot_date);
    Ok(EvaluationArtifacts {
        examples: read_json(&dir.join("examples.json"))?,
        trajectories: read_json(&dir.join("trajectories.json"))?,
        alignment_report: read_json(&dir.join("alignment_report.json"))?,
    })
}

pub fn evaluation_storage_dir(data_root: &Path, snapshot_date: NaiveDate) -> PathBuf {
    data_root.join("evaluation").join(snapshot_date.to_string())
}

pub fn collect_actions_from_snapshot_date(
    data_root: &Path,
    fixture_root: &Path,
    snapshot_date: NaiveDate,
) -> Result<Vec<crate::model::normalized_records::NormalizedActionRecord>, SenateSimError> {
    let mut dates = std::fs::read_dir(fixture_root)
        .map_err(|source| SenateSimError::Io {
            path: fixture_root.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| NaiveDate::parse_from_str(name, "%Y-%m-%d").ok())
        })
        .filter(|date| *date >= snapshot_date)
        .collect::<Vec<_>>();
    dates.sort();
    dates.dedup();

    let mut actions = Vec::new();
    for date in dates {
        let snapshot = match load_snapshot(data_root, date) {
            Ok(snapshot) => snapshot,
            Err(_) => run_daily_ingestion_with_roots(date, fixture_root, data_root)?,
        };
        actions.extend(snapshot.action_records);
    }
    Ok(actions)
}

fn first_future_consequential_event<'a>(
    snapshot_date: NaiveDate,
    timeline: &'a HistoricalTimeline,
) -> Option<(usize, &'a crate::model::historical_timeline::HistoricalActionEvent)> {
    timeline
        .events
        .iter()
        .enumerate()
        .find(|(_, event)| {
            event.is_consequential
                && event.action_date > snapshot_date
                && event.aligned_senate_event.is_some()
        })
}

fn is_terminal(event: &SenateEvent) -> bool {
    matches!(
        event,
        SenateEvent::FinalPassageSucceeds
            | SenateEvent::FinalPassageFails
            | SenateEvent::ClotureFails
    )
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SenateSimError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SenateSimError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let contents = serde_json::to_string_pretty(value).map_err(SenateSimError::Serialize)?;
    fs::write(path, contents).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, SenateSimError> {
    let contents = fs::read_to_string(path).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| SenateSimError::Json {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::model::{
        data_snapshot::{DataSnapshot, SourceManifest},
        historical_timeline::{HistoricalActionEvent, HistoricalTimeline},
        legislative::{BudgetaryImpact, LegislativeObjectType, PolicyDomain},
        legislative_context::{Chamber, ProceduralStage},
        normalized_records::{NormalizedActionCategory, NormalizedLegislativeRecord},
        senate_event::SenateEvent,
    };

    use super::{
        build_evaluation_artifacts, evaluation_storage_dir, generate_actual_trajectory,
        generate_next_event_examples, load_evaluation_artifacts, persist_evaluation_artifacts,
    };

    fn snapshot() -> DataSnapshot {
        DataSnapshot {
            snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            run_id: "snapshot-20260309".to_string(),
            created_at: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap().to_utc(),
            roster_records: vec![],
            legislative_records: vec![NormalizedLegislativeRecord {
                object_id: "obj_1".to_string(),
                title: "Test Bill".to_string(),
                summary: "Test summary".to_string(),
                object_type: LegislativeObjectType::Bill,
                policy_domain: PolicyDomain::EnergyClimate,
                sponsor: Some("A. Sponsor".to_string()),
                introduced_date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                latest_status_text: Some("Cloture filed".to_string()),
                current_stage: ProceduralStage::ClotureFiled,
                origin_chamber: Chamber::Senate,
                budgetary_impact: BudgetaryImpact::Moderate,
                salience: 0.7,
                controversy: 0.6,
                as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            }],
            action_records: vec![],
            source_manifests: vec![SourceManifest {
                source_name: "test".to_string(),
                fetched_at: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap().to_utc(),
                as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                source_identifier: "test".to_string(),
                content_hash: "abc".to_string(),
                record_count: 1,
            }],
        }
    }

    fn timeline(events: Vec<HistoricalActionEvent>) -> HistoricalTimeline {
        HistoricalTimeline {
            object_id: "obj_1".to_string(),
            events,
        }
    }

    fn event(
        date: (i32, u32, u32),
        event: Option<SenateEvent>,
        consequential: bool,
    ) -> HistoricalActionEvent {
        HistoricalActionEvent {
            object_id: "obj_1".to_string(),
            action_date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            raw_action_text: "event".to_string(),
            normalized_action_category: NormalizedActionCategory::Cloture,
            aligned_senate_event: event,
            is_consequential: consequential,
            source_record_id: Some(format!("{}_{}_{}", date.0, date.1, date.2)),
        }
    }

    #[test]
    fn next_event_label_generation_skips_same_day_or_past_events() {
        let examples = generate_next_event_examples(
            &snapshot(),
            &[timeline(vec![
                event((2026, 3, 8), Some(SenateEvent::ClotureFiled), true),
                event((2026, 3, 9), Some(SenateEvent::ClotureVoteScheduled), true),
                event((2026, 3, 10), Some(SenateEvent::ClotureInvoked), true),
            ])],
        )
        .unwrap();

        assert_eq!(
            examples[0].actual_next_event,
            Some(SenateEvent::ClotureInvoked)
        );
        assert_eq!(
            examples[0].actual_next_event_date,
            Some(NaiveDate::from_ymd_opt(2026, 3, 10).unwrap())
        );
    }

    #[test]
    fn no_event_case_labels_no_meaningful_movement() {
        let examples = generate_next_event_examples(&snapshot(), &[timeline(vec![])]).unwrap();
        assert_eq!(
            examples[0].actual_next_event,
            Some(SenateEvent::NoMeaningfulMovement)
        );
    }

    #[test]
    fn actual_trajectory_generation_respects_horizon_and_terminal_event() {
        let trajectory = generate_actual_trajectory(
            &snapshot(),
            &timeline(vec![
                event((2026, 3, 10), Some(SenateEvent::ClotureVoteScheduled), true),
                event((2026, 3, 11), Some(SenateEvent::ClotureInvoked), true),
                event((2026, 3, 12), Some(SenateEvent::FinalPassageSucceeds), true),
                event((2026, 3, 13), Some(SenateEvent::ReturnedToCalendar), true),
            ]),
            5,
            Some(5),
        )
        .unwrap();

        assert_eq!(trajectory.events.len(), 3);
        assert_eq!(trajectory.events[2].event, SenateEvent::FinalPassageSucceeds);
    }

    #[test]
    fn artifact_persistence_writes_dated_files() {
        let artifacts = build_evaluation_artifacts(
            &snapshot(),
            &[timeline(vec![event(
                (2026, 3, 10),
                Some(SenateEvent::ClotureInvoked),
                true,
            )])],
            3,
            Some(10),
        )
        .unwrap();
        let temp_dir = std::env::temp_dir().join("senate_sim_eval_artifacts");
        let _ = std::fs::remove_dir_all(&temp_dir);

        persist_evaluation_artifacts(
            &temp_dir,
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            &artifacts,
        )
        .unwrap();
        let loaded =
            load_evaluation_artifacts(&temp_dir, NaiveDate::from_ymd_opt(2026, 3, 9).unwrap())
                .unwrap();

        assert_eq!(loaded.examples.len(), 1);
        assert_eq!(
            evaluation_storage_dir(&temp_dir, NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()),
            temp_dir.join("evaluation/2026-03-09")
        );
    }
}
