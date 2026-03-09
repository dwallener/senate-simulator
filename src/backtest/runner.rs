use std::{fs, path::Path};

use chrono::NaiveDate;

use crate::{
    analyze_chamber, derive_stance_with_mode,
    derive::StanceDerivationMode,
    error::SenateSimError,
    features::materialize::{senators_for_snapshot, SenatorProfileMode},
    ingest::{
        load_snapshot, run_daily_ingestion_with_roots, snapshot_to_contexts,
        snapshot_to_legislative_objects,
    },
    model::{
        backtest_result::BacktestResult,
        normalized_records::{NormalizedActionCategory, NormalizedActionRecord},
        senate_event::SenateEvent,
    },
    predict_next_event,
};

pub fn run_backtest(
    snapshot_date: NaiveDate,
    object_id: &str,
) -> Result<BacktestResult, SenateSimError> {
    run_backtest_with_mode(
        snapshot_date,
        object_id,
        StanceDerivationMode::FeatureDriven,
    )
}

pub fn run_backtest_with_mode(
    snapshot_date: NaiveDate,
    object_id: &str,
    mode: StanceDerivationMode,
) -> Result<BacktestResult, SenateSimError> {
    run_backtest_with_roots(
        snapshot_date,
        object_id,
        Path::new("data"),
        Path::new("fixtures/ingest"),
        mode,
    )
}

pub fn run_backtest_with_roots(
    snapshot_date: NaiveDate,
    object_id: &str,
    data_root: &Path,
    fixture_root: &Path,
    mode: StanceDerivationMode,
) -> Result<BacktestResult, SenateSimError> {
    let snapshot = match load_snapshot(data_root, snapshot_date) {
        Ok(snapshot) => snapshot,
        Err(_) => run_daily_ingestion_with_roots(snapshot_date, fixture_root, data_root)?,
    };

    let senators = senators_for_snapshot(&snapshot, data_root, SenatorProfileMode::HistoricalFeatures)?;
    let legislative_objects = snapshot_to_legislative_objects(&snapshot)?;
    let contexts = snapshot_to_contexts(&snapshot)?;

    let object_index = legislative_objects
        .iter()
        .position(|object| object.object_id == object_id)
        .ok_or_else(|| SenateSimError::Validation {
            field: "backtest.object_id",
            message: format!("object {object_id} not found in snapshot {}", snapshot_date),
        })?;
    let legislative_object = &legislative_objects[object_index];
    let context = &contexts[object_index];

    let stances = senators
        .iter()
        .map(|senator| derive_stance_with_mode(senator, legislative_object, context, mode))
        .collect::<Result<Vec<_>, _>>()?;
    let analysis = analyze_chamber(legislative_object, context, &stances)?;
    let prediction = predict_next_event(legislative_object, context, &analysis)?;

    let actual_action = find_actual_next_action(data_root, fixture_root, snapshot_date, object_id)?;
    let predicted_category = map_event_to_category(&prediction.predicted_event);
    let alternative_categories = prediction
        .alternative_events
        .iter()
        .map(|event| map_event_to_category(&event.event))
        .collect::<Vec<_>>();

    let result = BacktestResult {
        snapshot_date,
        object_id: object_id.to_string(),
        predicted_next_event: Some(prediction.predicted_event.clone()),
        actual_next_event: actual_action.as_ref().map(|record| record.category),
        match_top_1: actual_action
            .as_ref()
            .map(|record| record.category == predicted_category)
            .unwrap_or(false),
        match_top_k: actual_action
            .as_ref()
            .map(|record| {
                record.category == predicted_category
                    || alternative_categories.contains(&record.category)
            })
            .unwrap_or(false),
        prediction_confidence: Some(prediction.confidence),
        notes: build_notes(context.procedural_stage.to_string(), &actual_action),
    };
    result.validate()?;
    Ok(result)
}

fn find_actual_next_action(
    data_root: &Path,
    fixture_root: &Path,
    snapshot_date: NaiveDate,
    object_id: &str,
) -> Result<Option<NormalizedActionRecord>, SenateSimError> {
    let mut available_dates = available_snapshot_dates(data_root, fixture_root)?;
    available_dates.sort();

    for date in available_dates
        .into_iter()
        .filter(|date| *date > snapshot_date)
    {
        let snapshot = match load_snapshot(data_root, date) {
            Ok(snapshot) => snapshot,
            Err(_) => run_daily_ingestion_with_roots(date, fixture_root, data_root)?,
        };
        if let Some(action) = snapshot
            .action_records
            .into_iter()
            .filter(|record| record.object_id == object_id && record.action_date > snapshot_date)
            .min_by_key(|record| record.action_date)
        {
            return Ok(Some(action));
        }
    }

    Ok(None)
}

fn available_snapshot_dates(
    data_root: &Path,
    fixture_root: &Path,
) -> Result<Vec<NaiveDate>, SenateSimError> {
    let mut dates = Vec::new();
    for root in [data_root.join("snapshots"), fixture_root.to_path_buf()] {
        if !root.exists() {
            continue;
        }
        for entry in fs::read_dir(&root).map_err(|source| SenateSimError::Io {
            path: root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| SenateSimError::Io {
                path: root.clone(),
                source,
            })?;
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(date) = NaiveDate::parse_from_str(name, "%Y-%m-%d") {
                    if !dates.contains(&date) {
                        dates.push(date);
                    }
                }
            }
        }
    }
    Ok(dates)
}

fn map_event_to_category(event: &SenateEvent) -> NormalizedActionCategory {
    match event {
        SenateEvent::MotionToProceedAttempted => NormalizedActionCategory::MotionToProceed,
        SenateEvent::DebateBegins => NormalizedActionCategory::Debate,
        SenateEvent::AmendmentFightBegins => NormalizedActionCategory::Amendment,
        SenateEvent::ClotureFiled
        | SenateEvent::ClotureVoteScheduled
        | SenateEvent::ClotureInvoked
        | SenateEvent::ClotureFails => NormalizedActionCategory::Cloture,
        SenateEvent::FinalPassageScheduled
        | SenateEvent::FinalPassageSucceeds
        | SenateEvent::FinalPassageFails => NormalizedActionCategory::Passage,
        SenateEvent::NoMeaningfulMovement | SenateEvent::ProceduralBlock => {
            NormalizedActionCategory::Stall
        }
        SenateEvent::LeadershipSignalsAction
        | SenateEvent::NegotiationIntensifies
        | SenateEvent::ReturnedToCalendar
        | SenateEvent::Other(_) => NormalizedActionCategory::Other,
    }
}

fn build_notes(
    starting_stage: String,
    actual_action: &Option<NormalizedActionRecord>,
) -> Vec<String> {
    let mut notes = vec![format!("backtest started from stage {starting_stage}")];
    if let Some(action) = actual_action {
        notes.push(format!(
            "actual next action observed on {} as {:?}",
            action.action_date, action.category
        ));
    } else {
        notes.push("no subsequent action found in later dated snapshots".to_string());
    }
    notes
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::derive::StanceDerivationMode;

    use super::run_backtest_with_roots;

    #[test]
    fn anti_leakage_backtest_uses_future_only_for_evaluation() {
        let temp_dir = std::env::temp_dir().join("senate_sim_backtest_anti_leakage");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let result = run_backtest_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            "s_2100",
            &temp_dir,
            std::path::Path::new("fixtures/ingest"),
            StanceDerivationMode::FeatureDriven,
        )
        .unwrap();

        assert_eq!(
            result.snapshot_date,
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()
        );
        assert!(result.actual_next_event.is_some());
        assert!(
            result
                .notes
                .iter()
                .any(|note| note.contains("actual next action"))
        );
    }
}
