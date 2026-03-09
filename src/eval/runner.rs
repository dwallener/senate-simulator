use std::path::Path;

use chrono::NaiveDate;

use crate::{
    analyze_chamber, derive_stance,
    error::SenateSimError,
    eval::{
        examples::{
            build_evaluation_artifacts, collect_actions_from_snapshot_date,
            load_evaluation_artifacts, persist_evaluation_artifacts,
        },
        timeline::build_historical_timelines,
    },
    features::{
        materialize::snapshot_with_features_to_senators,
        senator::build_senator_features_for_snapshot,
        windows::FeatureWindowConfig,
    },
    ingest::{
        load_snapshot, run_daily_ingestion_with_roots, snapshot_to_contexts,
        snapshot_to_legislative_objects,
    },
    model::{
        data_snapshot::DataSnapshot,
        evaluation_example::{EvaluationExample, EvaluationSummary},
    },
    predict_next_event, rollout, SimulationState,
};

pub fn evaluate_from_snapshot_date(
    snapshot_date: NaiveDate,
) -> Result<EvaluationSummary, SenateSimError> {
    evaluate_from_snapshot_date_with_roots(
        snapshot_date,
        Path::new("data"),
        Path::new("fixtures/ingest"),
    )
}

pub fn evaluate_from_snapshot_date_with_roots(
    snapshot_date: NaiveDate,
    data_root: &Path,
    fixture_root: &Path,
) -> Result<EvaluationSummary, SenateSimError> {
    let snapshot = match load_snapshot(data_root, snapshot_date) {
        Ok(snapshot) => snapshot,
        Err(_) => run_daily_ingestion_with_roots(snapshot_date, fixture_root, data_root)?,
    };

    let artifacts = match load_evaluation_artifacts(data_root, snapshot_date) {
        Ok(artifacts) => artifacts,
        Err(_) => {
            let future_actions =
                collect_actions_from_snapshot_date(data_root, fixture_root, snapshot_date)?;
            let timelines = build_historical_timelines(&future_actions)?;
            let artifacts = build_evaluation_artifacts(&snapshot, &timelines, 3, Some(30))?;
            persist_evaluation_artifacts(data_root, snapshot_date, &artifacts)?;
            artifacts
        }
    };

    evaluate_snapshot_examples(&snapshot, &artifacts.examples)
}

pub fn evaluate_snapshot_examples(
    snapshot: &DataSnapshot,
    examples: &[EvaluationExample],
) -> Result<EvaluationSummary, SenateSimError> {
    let features =
        build_senator_features_for_snapshot(snapshot, &snapshot.vote_records, &FeatureWindowConfig::default())?;
    let senators = snapshot_with_features_to_senators(snapshot, &features)?;
    let legislative_objects = snapshot_to_legislative_objects(snapshot)?;
    let contexts = snapshot_to_contexts(snapshot)?;

    let mut total_examples = 0usize;
    let mut top_1_hits = 0usize;
    let mut top_k_hits = 0usize;
    let mut trajectory_prefix_hits = 0usize;
    let mut unscorable_examples = 0usize;

    for example in examples {
        total_examples += 1;
        let Some(actual_event) = example.actual_next_event.clone() else {
            unscorable_examples += 1;
            continue;
        };

        let Some(index) = legislative_objects
            .iter()
            .position(|object| object.object_id == example.object_id)
        else {
            unscorable_examples += 1;
            continue;
        };

        let legislative_object = &legislative_objects[index];
        let context = &contexts[index];
        let stances = senators
            .iter()
            .map(|senator| derive_stance(senator, legislative_object, context))
            .collect::<Result<Vec<_>, _>>()?;
        let analysis = analyze_chamber(legislative_object, context, &stances)?;
        let prediction = predict_next_event(legislative_object, context, &analysis)?;

        if prediction.predicted_event == actual_event {
            top_1_hits += 1;
        }
        if prediction.predicted_event == actual_event
            || prediction
                .alternative_events
                .iter()
                .any(|event| event.event == actual_event)
        {
            top_k_hits += 1;
        }

        let rollout_result = rollout(
            &SimulationState {
                legislative_object: legislative_object.clone(),
                context: context.clone(),
                roster: senators.clone(),
                step_index: 0,
                last_event: None,
                consecutive_no_movement: 0,
                days_elapsed: 0,
                cloture_attempts: 0,
            },
            3,
        )?;
        if rollout_result
            .steps
            .first()
            .map(|step| step.predicted_event == actual_event)
            .unwrap_or(false)
        {
            trajectory_prefix_hits += 1;
        }
    }

    let scored_examples = total_examples.saturating_sub(unscorable_examples).max(1);
    let summary = EvaluationSummary {
        total_examples,
        top_1_next_event_accuracy: top_1_hits as f32 / scored_examples as f32,
        top_k_next_event_accuracy: top_k_hits as f32 / scored_examples as f32,
        trajectory_prefix_match_rate: trajectory_prefix_hits as f32 / scored_examples as f32,
        unscorable_examples,
        notes: vec![
            format!("evaluated snapshot {}", snapshot.snapshot_date),
            "examples are scored strictly against future-only aligned events".to_string(),
        ],
    };
    summary.validate()?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};

    use crate::model::{
        data_snapshot::{DataSnapshot, SourceManifest},
        evaluation_example::{EvaluationExample, EvaluationSummary},
        legislative::{BudgetaryImpact, LegislativeObjectType, PolicyDomain},
        legislative_context::{Chamber, ProceduralStage},
        normalized_records::{NormalizedLegislativeRecord, NormalizedSenatorRecord},
        senate_event::SenateEvent,
    };

    use super::evaluate_snapshot_examples;

    fn snapshot() -> DataSnapshot {
        DataSnapshot {
            snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            run_id: "snapshot-20260309".to_string(),
            created_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
            roster_records: vec![
                NormalizedSenatorRecord {
                    senator_id: "real_a".to_string(),
                    full_name: "A Example".to_string(),
                    party: crate::Party::Democrat,
                    state: "CA".to_string(),
                    class: crate::SenateClass::I,
                    start_date: NaiveDate::from_ymd_opt(2023, 1, 3).unwrap(),
                    end_date: None,
                    source_member_id: "a".to_string(),
                    as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                },
                NormalizedSenatorRecord {
                    senator_id: "real_b".to_string(),
                    full_name: "B Example".to_string(),
                    party: crate::Party::Republican,
                    state: "TX".to_string(),
                    class: crate::SenateClass::II,
                    start_date: NaiveDate::from_ymd_opt(2021, 1, 3).unwrap(),
                    end_date: None,
                    source_member_id: "b".to_string(),
                    as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                },
            ],
            legislative_records: vec![NormalizedLegislativeRecord {
                object_id: "s_2100".to_string(),
                title: "Clean Grid Permitting Act".to_string(),
                summary: "Energy bill".to_string(),
                object_type: LegislativeObjectType::Bill,
                policy_domain: PolicyDomain::EnergyClimate,
                sponsor: Some("A Sponsor".to_string()),
                introduced_date: NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
                latest_status_text: Some("Cloture filed in Senate after structured debate.".to_string()),
                current_stage: ProceduralStage::ClotureFiled,
                origin_chamber: Chamber::Senate,
                budgetary_impact: BudgetaryImpact::Moderate,
                salience: 0.7,
                controversy: 0.6,
                as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            }],
            action_records: vec![],
            vote_records: vec![],
            source_manifests: vec![SourceManifest {
                source_name: "test".to_string(),
                fetched_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
                as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                source_identifier: "test".to_string(),
                content_hash: "abc".to_string(),
                record_count: 1,
            }],
        }
    }

    #[test]
    fn evaluation_summary_metrics_are_computed() {
        let examples = vec![EvaluationExample {
            example_id: "ex_1".to_string(),
            snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            object_id: "s_2100".to_string(),
            current_stage: Some(ProceduralStage::ClotureFiled),
            actual_next_event: Some(SenateEvent::ClotureVoteScheduled),
            actual_next_event_date: Some(NaiveDate::from_ymd_opt(2026, 3, 10).unwrap()),
            snapshot_path: "data/snapshots/2026-03-09/snapshot.json".to_string(),
            timeline_position: 0,
            notes: vec![],
        }];

        let summary: EvaluationSummary = evaluate_snapshot_examples(&snapshot(), &examples).unwrap();
        assert_eq!(summary.total_examples, 1);
        assert!((0.0..=1.0).contains(&summary.top_1_next_event_accuracy));
        assert!((0.0..=1.0).contains(&summary.top_k_next_event_accuracy));
        assert!((0.0..=1.0).contains(&summary.trajectory_prefix_match_rate));
    }
}
