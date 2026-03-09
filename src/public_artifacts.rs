use std::{collections::HashMap, fs, path::Path};

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    analyze_chamber, assess_floor_action, derive_stance_with_mode,
    error::SenateSimError,
    features::materialize::{senators_for_snapshot, SenatorProfileMode},
    ingest::{load_snapshot, run_daily_ingestion_with_roots, snapshot_to_contexts, snapshot_to_legislative_objects},
    model::{
        floor_action_assessment::FloorActionAssessment,
        legislative_context::LegislativeContext,
        next_event_prediction::{EventScore, NextEventPrediction},
        senate_analysis::{PivotSummary, SenateAnalysis, SenatorSignalSummary},
        simulation_step::SimulationStep,
        trajectory_result::TerminationReason,
    },
    predict_next_event, rollout_with_mode, StanceDerivationMode, SimulationState,
};

const PUBLIC_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct TrackedBillsManifest {
    pub tracked: Vec<TrackedBillEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TrackedBillEntry {
    ObjectId(String),
    Detailed { object_id: String, label: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicLastUpdated {
    pub schema_version: u32,
    pub snapshot_date: NaiveDate,
    pub generated_at: String,
    pub tracked_bill_count: usize,
    pub exported_bill_count: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicSummary {
    pub schema_version: u32,
    pub snapshot_date: NaiveDate,
    pub generated_at: String,
    pub tracked_bill_count: usize,
    pub exported_bill_count: usize,
    pub rows: Vec<PublicSummaryRow>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicSummaryRow {
    pub object_id: String,
    pub label: String,
    pub title: String,
    pub stage: String,
    pub support_count: usize,
    pub lean_support_count: usize,
    pub undecided_count: usize,
    pub oppose_count: usize,
    pub majority_viable: bool,
    pub cloture_viable: bool,
    pub predicted_floor_action: String,
    pub predicted_next_event: String,
    pub next_event_score: f32,
    pub next_event_confidence: f32,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicTrackedBills {
    pub schema_version: u32,
    pub snapshot_date: NaiveDate,
    pub tracked: Vec<PublicTrackedBill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicTrackedBill {
    pub object_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicBillDetail {
    pub schema_version: u32,
    pub snapshot_date: NaiveDate,
    pub generated_at: String,
    pub object_id: String,
    pub label: String,
    pub title: String,
    pub summary: String,
    pub stage: String,
    pub support_count: usize,
    pub lean_support_count: usize,
    pub undecided_count: usize,
    pub oppose_count: usize,
    pub majority_viable: bool,
    pub cloture_viable: bool,
    pub expected_present_count: usize,
    pub coalition_stability: f32,
    pub filibuster_risk: f32,
    pub predicted_floor_action: String,
    pub floor_action_confidence: f32,
    pub support_margin_estimate: i32,
    pub cloture_gap_estimate: i32,
    pub predicted_next_event: String,
    pub next_event_score: f32,
    pub next_event_confidence: f32,
    pub alternatives: Vec<PublicAlternativeEvent>,
    pub pivots: Vec<PublicPivot>,
    pub blockers: Vec<PublicSignalActor>,
    pub defectors: Vec<PublicSignalActor>,
    pub rollout_steps: Vec<PublicRolloutStep>,
    pub top_findings: Vec<String>,
    pub top_reasons: Vec<String>,
    pub termination_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicAlternativeEvent {
    pub event: String,
    pub score: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRolloutStep {
    pub step_index: usize,
    pub starting_stage: String,
    pub predicted_event: String,
    pub confidence: f32,
    pub support_count: usize,
    pub lean_support_count: usize,
    pub undecided_count: usize,
    pub oppose_count: usize,
    pub simple_majority_viable: bool,
    pub cloture_viable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicPivot {
    pub senator_id: String,
    pub senator_name: String,
    pub state: String,
    pub party: String,
    pub reason: String,
    pub stance_label: String,
    pub procedural_posture: String,
    pub substantive_support: f32,
    pub procedural_support: f32,
    pub negotiability: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicSignalActor {
    pub senator_id: String,
    pub senator_name: String,
    pub state: String,
    pub party: String,
    pub reason: String,
    pub public_position: String,
    pub defection_probability: f32,
    pub rigidity: f32,
}

#[derive(Debug, Clone)]
struct SenatorDisplayInfo {
    name: String,
    state: String,
    party: String,
}

#[derive(Debug, Clone)]
pub struct ExportArtifacts {
    pub last_updated: PublicLastUpdated,
    pub summary: PublicSummary,
    pub tracked_bills: PublicTrackedBills,
    pub bill_details: Vec<PublicBillDetail>,
}

pub fn export_public_artifacts(
    snapshot_date: NaiveDate,
    tracked_bills_file: &Path,
    out_dir: &Path,
    mode: StanceDerivationMode,
    steps: usize,
) -> Result<ExportArtifacts, SenateSimError> {
    export_public_artifacts_with_roots(
        snapshot_date,
        tracked_bills_file,
        out_dir,
        Path::new("data"),
        Path::new("fixtures/ingest"),
        mode,
        steps,
    )
}

pub fn export_public_artifacts_with_roots(
    snapshot_date: NaiveDate,
    tracked_bills_file: &Path,
    out_dir: &Path,
    data_root: &Path,
    fixture_root: &Path,
    mode: StanceDerivationMode,
    steps: usize,
) -> Result<ExportArtifacts, SenateSimError> {
    let snapshot = match load_snapshot(data_root, snapshot_date) {
        Ok(snapshot) => snapshot,
        Err(_) => run_daily_ingestion_with_roots(snapshot_date, fixture_root, data_root)?,
    };
    let manifest = load_tracked_bills_manifest(tracked_bills_file)?;
    let requested_count = manifest.tracked.len();
    let senators = senators_for_snapshot(&snapshot, data_root, SenatorProfileMode::HistoricalFeatures)?;
    let objects = snapshot_to_legislative_objects(&snapshot)?;
    let contexts = snapshot_to_contexts(&snapshot)?;
    let generated_at = Utc::now().to_rfc3339();
    let senator_lookup = senators
        .iter()
        .map(|senator| {
            (
                senator.identity.senator_id.clone(),
                SenatorDisplayInfo {
                    name: senator.identity.full_name.clone(),
                    state: senator.identity.state.clone(),
                    party: senator.identity.party.to_string(),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let mut rows = Vec::new();
    let mut details = Vec::new();
    let mut tracked_public = Vec::new();
    let mut notes = Vec::new();

    for entry in manifest.tracked {
        let (object_id, label) = match entry {
            TrackedBillEntry::ObjectId(object_id) => (object_id.clone(), object_id),
            TrackedBillEntry::Detailed { object_id, label } => {
                let label = label.unwrap_or_else(|| object_id.clone());
                (object_id, label)
            }
        };

        let Some(index) = objects.iter().position(|object| object.object_id == object_id) else {
            notes.push(format!("tracked object {object_id} not found in snapshot {snapshot_date}"));
            continue;
        };

        let object = &objects[index];
        let context = &contexts[index];
        let stances = senators
            .iter()
            .map(|senator| derive_stance_with_mode(senator, object, context, mode))
            .collect::<Result<Vec<_>, _>>()?;
        let analysis = analyze_chamber(object, context, &stances)?;
        let floor_action = assess_floor_action(object, context, &analysis)?;
        let next_event = predict_next_event(object, context, &analysis)?;
        let trajectory = rollout_with_mode(
            &SimulationState {
                legislative_object: object.clone(),
                context: context.clone(),
                roster: senators.clone(),
                step_index: 0,
                last_event: None,
                consecutive_no_movement: 0,
                days_elapsed: 0,
                cloture_attempts: 0,
            },
            steps,
            mode,
        )?;

        tracked_public.push(PublicTrackedBill {
            object_id: object.object_id.clone(),
            label: label.clone(),
        });
        rows.push(build_summary_row(
            object,
            &label,
            &analysis,
            &floor_action,
            &next_event,
            context,
            snapshot_date,
        ));
        details.push(build_bill_detail(
            object,
            &label,
            &analysis,
            &floor_action,
            &next_event,
            context,
            &trajectory.terminated_reason,
            &trajectory.steps,
            &senator_lookup,
            snapshot_date,
            generated_at.as_str(),
        ));
    }

    rows.sort_by(|a, b| a.object_id.cmp(&b.object_id));
    details.sort_by(|a, b| a.object_id.cmp(&b.object_id));
    tracked_public.sort_by(|a, b| a.object_id.cmp(&b.object_id));

    let last_updated = PublicLastUpdated {
        schema_version: PUBLIC_SCHEMA_VERSION,
        snapshot_date,
        generated_at: generated_at.clone(),
        tracked_bill_count: requested_count,
        exported_bill_count: details.len(),
        notes: notes.clone(),
    };
    let summary = PublicSummary {
        schema_version: PUBLIC_SCHEMA_VERSION,
        snapshot_date,
        generated_at: generated_at.clone(),
        tracked_bill_count: requested_count,
        exported_bill_count: rows.len(),
        rows,
        notes: notes.clone(),
    };
    let tracked_bills = PublicTrackedBills {
        schema_version: PUBLIC_SCHEMA_VERSION,
        snapshot_date,
        tracked: tracked_public,
    };

    persist_public_artifacts(out_dir, &last_updated, &summary, &tracked_bills, &details)?;

    Ok(ExportArtifacts {
        last_updated,
        summary,
        tracked_bills,
        bill_details: details,
    })
}

pub fn load_tracked_bills_manifest(path: &Path) -> Result<TrackedBillsManifest, SenateSimError> {
    let text = fs::read_to_string(path).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| SenateSimError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn persist_public_artifacts(
    out_dir: &Path,
    last_updated: &PublicLastUpdated,
    summary: &PublicSummary,
    tracked_bills: &PublicTrackedBills,
    details: &[PublicBillDetail],
) -> Result<(), SenateSimError> {
    let bills_dir = out_dir.join("bills");
    fs::create_dir_all(&bills_dir).map_err(|source| SenateSimError::Io {
        path: bills_dir.clone(),
        source,
    })?;

    write_json(&out_dir.join("last_updated.json"), last_updated)?;
    write_json(&out_dir.join("summary.json"), summary)?;
    write_json(&out_dir.join("tracked_bills.json"), tracked_bills)?;
    for detail in details {
        write_json(&bills_dir.join(format!("{}.json", detail.object_id)), detail)?;
    }
    Ok(())
}

fn build_summary_row(
    object: &crate::LegislativeObject,
    label: &str,
    analysis: &SenateAnalysis,
    floor_action: &FloorActionAssessment,
    next_event: &NextEventPrediction,
    context: &LegislativeContext,
    snapshot_date: NaiveDate,
) -> PublicSummaryRow {
    PublicSummaryRow {
        object_id: object.object_id.clone(),
        label: label.to_string(),
        title: object.title.clone(),
        stage: format!("{:?}", context.procedural_stage),
        support_count: analysis.likely_support_count,
        lean_support_count: analysis.lean_support_count,
        undecided_count: analysis.undecided_count,
        oppose_count: analysis.lean_oppose_count + analysis.likely_oppose_count,
        majority_viable: analysis.simple_majority_viable,
        cloture_viable: analysis.cloture_viable,
        predicted_floor_action: floor_action.predicted_action.to_string(),
        predicted_next_event: next_event.predicted_event.to_string(),
        next_event_score: next_event.predicted_event_score,
        next_event_confidence: next_event.confidence,
        last_updated: snapshot_date.to_string(),
    }
}

fn build_bill_detail(
    object: &crate::LegislativeObject,
    label: &str,
    analysis: &SenateAnalysis,
    floor_action: &FloorActionAssessment,
    next_event: &NextEventPrediction,
    context: &LegislativeContext,
    terminated_reason: &TerminationReason,
    steps: &[SimulationStep],
    senator_lookup: &HashMap<String, SenatorDisplayInfo>,
    snapshot_date: NaiveDate,
    generated_at: &str,
) -> PublicBillDetail {
    PublicBillDetail {
        schema_version: PUBLIC_SCHEMA_VERSION,
        snapshot_date,
        generated_at: generated_at.to_string(),
        object_id: object.object_id.clone(),
        label: label.to_string(),
        title: object.title.clone(),
        summary: object.summary.clone(),
        stage: format!("{:?}", context.procedural_stage),
        support_count: analysis.likely_support_count,
        lean_support_count: analysis.lean_support_count,
        undecided_count: analysis.undecided_count,
        oppose_count: analysis.lean_oppose_count + analysis.likely_oppose_count,
        majority_viable: analysis.simple_majority_viable,
        cloture_viable: analysis.cloture_viable,
        expected_present_count: analysis.expected_present_count,
        coalition_stability: analysis.coalition_stability,
        filibuster_risk: analysis.filibuster_risk,
        predicted_floor_action: floor_action.predicted_action.to_string(),
        floor_action_confidence: floor_action.confidence,
        support_margin_estimate: floor_action.support_margin_estimate,
        cloture_gap_estimate: floor_action.cloture_gap_estimate,
        predicted_next_event: next_event.predicted_event.to_string(),
        next_event_score: next_event.predicted_event_score,
        next_event_confidence: next_event.confidence,
        alternatives: next_event
            .alternative_events
            .iter()
            .map(public_alternative)
            .collect(),
        pivots: analysis
            .pivotal_senators
            .iter()
            .map(|pivot| public_pivot(pivot, senator_lookup))
            .collect(),
        blockers: analysis
            .likely_blockers
            .iter()
            .map(|blocker| public_signal_actor(blocker, senator_lookup))
            .collect(),
        defectors: analysis
            .likely_defectors
            .iter()
            .map(|defector| public_signal_actor(defector, senator_lookup))
            .collect(),
        rollout_steps: steps.iter().map(public_rollout_step).collect(),
        top_findings: analysis.top_findings.clone(),
        top_reasons: merged_reasons(analysis, floor_action, next_event),
        termination_reason: format!("{terminated_reason:?}"),
    }
}

fn public_alternative(event: &EventScore) -> PublicAlternativeEvent {
    PublicAlternativeEvent {
        event: event.event.to_string(),
        score: event.score,
        reason: event.reason.clone(),
    }
}

fn public_rollout_step(step: &SimulationStep) -> PublicRolloutStep {
    PublicRolloutStep {
        step_index: step.step_index,
        starting_stage: format!("{:?}", step.starting_stage),
        predicted_event: step.predicted_event.to_string(),
        confidence: step.confidence,
        support_count: step.analysis_summary.likely_support_count,
        lean_support_count: step.analysis_summary.lean_support_count,
        undecided_count: step.analysis_summary.undecided_count,
        oppose_count: step.analysis_summary.likely_oppose_count,
        simple_majority_viable: step.analysis_summary.simple_majority_viable,
        cloture_viable: step.analysis_summary.cloture_viable,
    }
}

fn public_pivot(
    pivot: &PivotSummary,
    senator_lookup: &HashMap<String, SenatorDisplayInfo>,
) -> PublicPivot {
    let display = senator_lookup.get(&pivot.senator_id);
    PublicPivot {
        senator_id: pivot.senator_id.clone(),
        senator_name: display
            .map(|value| value.name.clone())
            .unwrap_or_else(|| pivot.senator_id.clone()),
        state: display
            .map(|value| value.state.clone())
            .unwrap_or_else(|| "--".to_string()),
        party: display
            .map(|value| value.party.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        reason: pivot.reason.clone(),
        stance_label: format!("{:?}", pivot.stance_label),
        procedural_posture: format!("{:?}", pivot.procedural_posture),
        substantive_support: pivot.substantive_support,
        procedural_support: pivot.procedural_support,
        negotiability: pivot.negotiability,
    }
}

fn public_signal_actor(
    actor: &SenatorSignalSummary,
    senator_lookup: &HashMap<String, SenatorDisplayInfo>,
) -> PublicSignalActor {
    let display = senator_lookup.get(&actor.senator_id);
    PublicSignalActor {
        senator_id: actor.senator_id.clone(),
        senator_name: display
            .map(|value| value.name.clone())
            .unwrap_or_else(|| actor.senator_id.clone()),
        state: display
            .map(|value| value.state.clone())
            .unwrap_or_else(|| "--".to_string()),
        party: display
            .map(|value| value.party.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        reason: actor.reason.clone(),
        public_position: format!("{:?}", actor.public_position),
        defection_probability: actor.defection_probability,
        rigidity: actor.rigidity,
    }
}

fn merged_reasons(
    analysis: &SenateAnalysis,
    floor_action: &FloorActionAssessment,
    next_event: &NextEventPrediction,
) -> Vec<String> {
    let mut reasons = Vec::new();
    reasons.extend(next_event.top_reasons.iter().cloned());
    reasons.extend(floor_action.top_reasons.iter().cloned());
    reasons.extend(analysis.top_findings.iter().cloned());
    reasons.truncate(8);
    reasons
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SenateSimError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| SenateSimError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let text = serde_json::to_string_pretty(value).map_err(|source| SenateSimError::Validation {
        field: "public_artifacts.serialize",
        message: source.to_string(),
    })?;
    fs::write(path, text).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use chrono::NaiveDate;

    use super::{
        export_public_artifacts_with_roots, load_tracked_bills_manifest, PublicBillDetail,
        PublicLastUpdated, PublicSummary, PublicTrackedBills,
    };
    use crate::StanceDerivationMode;

    #[test]
    fn predict_export_writes_public_artifacts() {
        let temp = std::env::temp_dir().join("senate_sim_public_export");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let tracked = temp.join("tracked.json");
        std::fs::write(&tracked, r#"{"tracked":["s_2100"]}"#).unwrap();

        let artifacts = export_public_artifacts_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            &tracked,
            &temp.join("public"),
            &temp.join("data"),
            Path::new("fixtures/ingest"),
            StanceDerivationMode::FeatureDriven,
            2,
        )
        .unwrap();

        assert_eq!(artifacts.summary.rows.len(), 1);
        assert!(temp.join("public/summary.json").exists());
        assert!(temp.join("public/last_updated.json").exists());
        assert!(temp.join("public/tracked_bills.json").exists());
        assert!(temp.join("public/bills/s_2100.json").exists());
    }

    #[test]
    fn tracked_bills_filtering_only_exports_requested_objects() {
        let temp = std::env::temp_dir().join("senate_sim_public_export_filter");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let tracked = temp.join("tracked.json");
        std::fs::write(&tracked, r#"{"tracked":["missing_bill","s_2100"]}"#).unwrap();

        let artifacts = export_public_artifacts_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            &tracked,
            &temp.join("public"),
            &temp.join("data"),
            Path::new("fixtures/ingest"),
            StanceDerivationMode::FeatureDriven,
            1,
        )
        .unwrap();

        assert_eq!(artifacts.summary.rows.len(), 1);
        assert_eq!(artifacts.bill_details.len(), 1);
        assert!(artifacts.summary.notes.iter().any(|note| note.contains("missing_bill")));
    }

    #[test]
    fn public_schema_contains_required_fields() {
        let temp = std::env::temp_dir().join("senate_sim_public_export_schema");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let tracked = temp.join("tracked.json");
        std::fs::write(&tracked, r#"{"tracked":["s_2100"]}"#).unwrap();

        export_public_artifacts_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            &tracked,
            &temp.join("public"),
            &temp.join("data"),
            Path::new("fixtures/ingest"),
            StanceDerivationMode::FeatureDriven,
            1,
        )
        .unwrap();

        let summary: PublicSummary = serde_json::from_str(
            &std::fs::read_to_string(temp.join("public/summary.json")).unwrap(),
        )
        .unwrap();
        let last_updated: PublicLastUpdated = serde_json::from_str(
            &std::fs::read_to_string(temp.join("public/last_updated.json")).unwrap(),
        )
        .unwrap();
        let tracked_bills: PublicTrackedBills = serde_json::from_str(
            &std::fs::read_to_string(temp.join("public/tracked_bills.json")).unwrap(),
        )
        .unwrap();
        let bill: PublicBillDetail = serde_json::from_str(
            &std::fs::read_to_string(temp.join("public/bills/s_2100.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(summary.schema_version, 1);
        assert_eq!(last_updated.schema_version, 1);
        assert_eq!(tracked_bills.schema_version, 1);
        assert_eq!(bill.schema_version, 1);
        assert_eq!(summary.rows[0].object_id, "s_2100");
        assert_eq!(bill.object_id, "s_2100");
        assert!(!bill.predicted_next_event.is_empty());
    }

    #[test]
    fn tracked_bills_manifest_supports_rich_entries() {
        let temp = std::env::temp_dir().join("senate_sim_public_manifest");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let tracked = temp.join("tracked.json");
        std::fs::write(
            &tracked,
            r#"{"tracked":[{"object_id":"hr144","label":"TVA Salary Transparency Act"}]}"#,
        )
        .unwrap();
        let manifest = load_tracked_bills_manifest(&tracked).unwrap();
        assert_eq!(manifest.tracked.len(), 1);
    }
}
