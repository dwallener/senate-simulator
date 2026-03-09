use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    analyze_chamber, assess_floor_action, derive_stance_with_mode,
    error::SenateSimError,
    features::materialize::{senators_for_snapshot, SenatorProfileMode},
    ingest::{load_snapshot, run_daily_ingestion_with_roots, snapshot_to_contexts, snapshot_to_legislative_objects},
    model::{
        floor_action_assessment::FloorActionAssessment,
        legislative_context::{LegislativeContext, ProceduralStage},
        next_event_prediction::{EventScore, NextEventPrediction},
        senate_analysis::{PivotSummary, SenateAnalysis, SenatorSignalSummary},
        simulation_step::SimulationStep,
        normalized_records::NormalizedActionCategory,
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
    pub candidate_count: usize,
    pub exported_bill_count: usize,
    pub most_likely_to_move: Vec<PublicHomepageEntry>,
    pub most_likely_to_get_moving: Vec<PublicHomepageEntry>,
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
    pub candidate_priority_score: i32,
    pub likely_to_move_score: f32,
    pub likely_to_get_moving_score: f32,
    pub inclusion_reasons: Vec<String>,
    pub days_since_latest_action: Option<i64>,
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
    pub candidate_priority_score: i32,
    pub likely_to_move_score: f32,
    pub likely_to_get_moving_score: f32,
    pub inclusion_reasons: Vec<String>,
    pub days_since_latest_action: Option<i64>,
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
pub struct PublicHomepageEntry {
    pub object_id: String,
    pub label: String,
    pub title: String,
    pub stage: String,
    pub predicted_next_event: String,
    pub predicted_floor_action: String,
    pub next_event_score: f32,
    pub next_event_confidence: f32,
    pub majority_viable: bool,
    pub cloture_viable: bool,
    pub candidate_priority_score: i32,
    pub summary_blurb: String,
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
struct CandidateMetrics {
    latest_action_date: Option<NaiveDate>,
    recent_action_count_3: usize,
    recent_action_count_7: usize,
    recent_action_count_14: usize,
    recent_action_count_30: usize,
    cloture_action_count: usize,
    amendment_action_count: usize,
    passage_action_count: usize,
    object_attention: f32,
    domain_attention: f32,
    strongest_public_link: f32,
}

#[derive(Debug, Clone)]
struct CandidateRecord {
    label: String,
    object: crate::LegislativeObject,
    context: LegislativeContext,
    analysis: SenateAnalysis,
    floor_action: FloorActionAssessment,
    next_event: NextEventPrediction,
    trajectory_steps: Vec<SimulationStep>,
    termination_reason: TerminationReason,
    metrics: CandidateMetrics,
    candidate_priority_score: i32,
    likely_to_move_score: f32,
    likely_to_get_moving_score: f32,
    inclusion_reasons: Vec<String>,
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
    let watchlist_labels = manifest_label_map(&manifest);
    let watchlist_ids = watchlist_labels.keys().cloned().collect::<HashSet<_>>();
    let top_attention_ids = top_attention_object_ids(&snapshot, 25);
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
    let mut candidate_records = Vec::new();
    let mut tracked_public = Vec::new();
    let mut notes = Vec::new();

    for object_id in &watchlist_ids {
        let Some(_index) = objects.iter().position(|object| &object.object_id == object_id) else {
            notes.push(format!("tracked object {object_id} not found in snapshot {snapshot_date}"));
            continue;
        };
        tracked_public.push(PublicTrackedBill {
            object_id: object_id.clone(),
            label: watchlist_labels
                .get(object_id)
                .cloned()
                .unwrap_or_else(|| object_id.clone()),
        });
    }

    for (index, object) in objects.iter().enumerate() {
        if object.origin_chamber != crate::model::legislative_context::Chamber::Senate {
            continue;
        }

        let context = &contexts[index];
        let label = watchlist_labels
            .get(&object.object_id)
            .cloned()
            .unwrap_or_else(|| object.object_id.clone());
        let metrics = candidate_metrics(&snapshot, object, snapshot_date);
        let is_watchlist = watchlist_ids.contains(&object.object_id);
        let is_top_attention = top_attention_ids.contains(&object.object_id);

        let stances = senators
            .iter()
            .map(|senator| derive_stance_with_mode(senator, object, context, mode))
            .collect::<Result<Vec<_>, _>>()?;
        let analysis = analyze_chamber(object, context, &stances)?;
        let floor_action = assess_floor_action(object, context, &analysis)?;
        let next_event = predict_next_event(object, context, &analysis)?;

        let (candidate_priority_score, inclusion_reasons) = candidate_priority_score(
            object,
            context,
            &analysis,
            &floor_action,
            &next_event,
            &metrics,
            is_watchlist,
            is_top_attention,
            snapshot_date,
        );

        let include = should_include_candidate(
            object,
            context,
            &metrics,
            is_watchlist,
            is_top_attention,
            candidate_priority_score,
        );
        if !include {
            continue;
        }

        let likely_to_move_score =
            likely_to_move_score(object, context, &floor_action, &next_event, &metrics, snapshot_date);
        let likely_to_get_moving_score = likely_to_get_moving_score(
            object,
            context,
            &floor_action,
            &next_event,
            &metrics,
            snapshot_date,
        );
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

        candidate_records.push(CandidateRecord {
            label,
            object: object.clone(),
            context: context.clone(),
            analysis,
            floor_action,
            next_event,
            trajectory_steps: trajectory.steps,
            termination_reason: trajectory.terminated_reason,
            metrics,
            candidate_priority_score,
            likely_to_move_score,
            likely_to_get_moving_score,
            inclusion_reasons,
        });
    }

    for record in &candidate_records {
        rows.push(build_summary_row(record, snapshot_date));
        details.push(build_bill_detail(
            record,
            &senator_lookup,
            snapshot_date,
            generated_at.as_str(),
        ));
    }

    rows.sort_by(|a, b| {
        compare_f32_desc(a.likely_to_move_score, b.likely_to_move_score)
            .then_with(|| a.object_id.cmp(&b.object_id))
    });
    details.sort_by(|a, b| a.object_id.cmp(&b.object_id));
    tracked_public.sort_by(|a, b| a.object_id.cmp(&b.object_id));

    let most_likely_to_move = top_homepage_entries(&candidate_records, true);
    let most_likely_to_get_moving = top_homepage_entries(&candidate_records, false);

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
        candidate_count: candidate_records.len(),
        exported_bill_count: rows.len(),
        most_likely_to_move,
        most_likely_to_get_moving,
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

fn build_summary_row(record: &CandidateRecord, snapshot_date: NaiveDate) -> PublicSummaryRow {
    PublicSummaryRow {
        object_id: record.object.object_id.clone(),
        label: record.label.clone(),
        title: record.object.title.clone(),
        stage: record.context.procedural_stage.to_string(),
        support_count: record.analysis.likely_support_count,
        lean_support_count: record.analysis.lean_support_count,
        undecided_count: record.analysis.undecided_count,
        oppose_count: record.analysis.lean_oppose_count + record.analysis.likely_oppose_count,
        majority_viable: record.analysis.simple_majority_viable,
        cloture_viable: record.analysis.cloture_viable,
        predicted_floor_action: record.floor_action.predicted_action.to_string(),
        predicted_next_event: record.next_event.predicted_event.to_string(),
        next_event_score: record.next_event.predicted_event_score,
        next_event_confidence: record.next_event.confidence,
        candidate_priority_score: record.candidate_priority_score,
        likely_to_move_score: record.likely_to_move_score,
        likely_to_get_moving_score: record.likely_to_get_moving_score,
        inclusion_reasons: record.inclusion_reasons.clone(),
        days_since_latest_action: record
            .metrics
            .latest_action_date
            .map(|date| (snapshot_date - date).num_days()),
        last_updated: snapshot_date.to_string(),
    }
}

fn build_bill_detail(
    record: &CandidateRecord,
    senator_lookup: &HashMap<String, SenatorDisplayInfo>,
    snapshot_date: NaiveDate,
    generated_at: &str,
) -> PublicBillDetail {
    PublicBillDetail {
        schema_version: PUBLIC_SCHEMA_VERSION,
        snapshot_date,
        generated_at: generated_at.to_string(),
        object_id: record.object.object_id.clone(),
        label: record.label.clone(),
        title: record.object.title.clone(),
        summary: record.object.summary.clone(),
        stage: record.context.procedural_stage.to_string(),
        support_count: record.analysis.likely_support_count,
        lean_support_count: record.analysis.lean_support_count,
        undecided_count: record.analysis.undecided_count,
        oppose_count: record.analysis.lean_oppose_count + record.analysis.likely_oppose_count,
        majority_viable: record.analysis.simple_majority_viable,
        cloture_viable: record.analysis.cloture_viable,
        expected_present_count: record.analysis.expected_present_count,
        coalition_stability: record.analysis.coalition_stability,
        filibuster_risk: record.analysis.filibuster_risk,
        predicted_floor_action: record.floor_action.predicted_action.to_string(),
        floor_action_confidence: record.floor_action.confidence,
        support_margin_estimate: record.floor_action.support_margin_estimate,
        cloture_gap_estimate: record.floor_action.cloture_gap_estimate,
        predicted_next_event: record.next_event.predicted_event.to_string(),
        next_event_score: record.next_event.predicted_event_score,
        next_event_confidence: record.next_event.confidence,
        candidate_priority_score: record.candidate_priority_score,
        likely_to_move_score: record.likely_to_move_score,
        likely_to_get_moving_score: record.likely_to_get_moving_score,
        inclusion_reasons: record.inclusion_reasons.clone(),
        days_since_latest_action: record
            .metrics
            .latest_action_date
            .map(|date| (snapshot_date - date).num_days()),
        alternatives: record.next_event
            .alternative_events
            .iter()
            .map(public_alternative)
            .collect(),
        pivots: record
            .analysis
            .pivotal_senators
            .iter()
            .map(|pivot| public_pivot(pivot, senator_lookup))
            .collect(),
        blockers: record
            .analysis
            .likely_blockers
            .iter()
            .map(|blocker| public_signal_actor(blocker, senator_lookup))
            .collect(),
        defectors: record
            .analysis
            .likely_defectors
            .iter()
            .map(|defector| public_signal_actor(defector, senator_lookup))
            .collect(),
        rollout_steps: record.trajectory_steps.iter().map(public_rollout_step).collect(),
        top_findings: record.analysis.top_findings.clone(),
        top_reasons: merged_reasons(&record.analysis, &record.floor_action, &record.next_event),
        termination_reason: format!("{:?}", record.termination_reason),
    }
}

fn manifest_label_map(manifest: &TrackedBillsManifest) -> HashMap<String, String> {
    manifest
        .tracked
        .iter()
        .map(|entry| match entry {
            TrackedBillEntry::ObjectId(object_id) => (object_id.clone(), object_id.clone()),
            TrackedBillEntry::Detailed { object_id, label } => (
                object_id.clone(),
                label.clone().unwrap_or_else(|| object_id.clone()),
            ),
        })
        .collect()
}

fn top_attention_object_ids(
    snapshot: &crate::model::data_snapshot::DataSnapshot,
    limit: usize,
) -> HashSet<String> {
    let Some(summary) = &snapshot.public_signal_summary else {
        return HashSet::new();
    };
    let mut values = summary
        .object_attention
        .iter()
        .map(|(object_id, score)| (object_id.clone(), *score))
        .collect::<Vec<_>>();
    values.sort_by(|left, right| compare_f32_desc(left.1, right.1).then_with(|| left.0.cmp(&right.0)));
    values.into_iter().take(limit).map(|(object_id, _)| object_id).collect()
}

fn candidate_metrics(
    snapshot: &crate::model::data_snapshot::DataSnapshot,
    object: &crate::LegislativeObject,
    snapshot_date: NaiveDate,
) -> CandidateMetrics {
    let mut latest_action_date: Option<NaiveDate> = None;
    let mut recent_action_count_3 = 0;
    let mut recent_action_count_7 = 0;
    let mut recent_action_count_14 = 0;
    let mut recent_action_count_30 = 0;
    let mut cloture_action_count = 0;
    let mut amendment_action_count = 0;
    let mut passage_action_count = 0;

    for action in snapshot
        .action_records
        .iter()
        .filter(|action| action.object_id == object.object_id)
    {
        latest_action_date = Some(match latest_action_date {
            Some(current) => current.max(action.action_date),
            None => action.action_date,
        });
        let days_ago = (snapshot_date - action.action_date).num_days();
        if days_ago <= 3 {
            recent_action_count_3 += 1;
        }
        if days_ago <= 7 {
            recent_action_count_7 += 1;
        }
        if days_ago <= 14 {
            recent_action_count_14 += 1;
        }
        if days_ago <= 30 {
            recent_action_count_30 += 1;
        }
        match action.category {
            NormalizedActionCategory::Cloture => cloture_action_count += 1,
            NormalizedActionCategory::Amendment => amendment_action_count += 1,
            NormalizedActionCategory::Passage => passage_action_count += 1,
            _ => {}
        }
    }

    let (object_attention, domain_attention, strongest_public_link) =
        if let Some(summary) = &snapshot.public_signal_summary {
            let object_attention = summary
                .object_attention
                .get(&object.object_id)
                .copied()
                .unwrap_or(0.0);
            let domain_attention = summary
                .domain_attention
                .get(&object.policy_domain)
                .copied()
                .unwrap_or(0.0);
            let strongest_public_link = summary
                .senator_object_link_strength
                .iter()
                .filter(|link| link.object_id == object.object_id)
                .map(|link| link.public_association_score)
                .fold(0.0, f32::max);
            (object_attention, domain_attention, strongest_public_link)
        } else {
            (0.0, 0.0, 0.0)
        };

    CandidateMetrics {
        latest_action_date,
        recent_action_count_3,
        recent_action_count_7,
        recent_action_count_14,
        recent_action_count_30,
        cloture_action_count,
        amendment_action_count,
        passage_action_count,
        object_attention,
        domain_attention,
        strongest_public_link,
    }
}

fn candidate_priority_score(
    object: &crate::LegislativeObject,
    context: &LegislativeContext,
    analysis: &SenateAnalysis,
    floor_action: &FloorActionAssessment,
    next_event: &NextEventPrediction,
    metrics: &CandidateMetrics,
    is_watchlist: bool,
    is_top_attention: bool,
    snapshot_date: NaiveDate,
) -> (i32, Vec<String>) {
    let mut score = 0;
    let mut reasons = Vec::new();

    if is_watchlist {
        reasons.push("manual watchlist".to_string());
    }
    if metrics.recent_action_count_14 > 0 {
        reasons.push("recent action in the last 14 days".to_string());
    }
    if is_stage_on_calendar_or_later(&context.procedural_stage) {
        reasons.push("stage is OnCalendar or later".to_string());
    }
    if metrics.cloture_action_count > 0 {
        reasons.push("cloture-related action exists".to_string());
    }
    if is_top_attention {
        reasons.push("top object-attention bill in public signals".to_string());
    }

    score += recent_action_points(metrics, snapshot_date);
    score += stage_points(&context.procedural_stage);
    score += procedural_intensity_points(metrics);
    score += coalition_prediction_points(analysis, floor_action, next_event);
    score += narrative_points(metrics);
    score += institutional_points(object);
    score += negative_points(object, next_event, metrics, snapshot_date);

    (score, reasons)
}

fn should_include_candidate(
    object: &crate::LegislativeObject,
    context: &LegislativeContext,
    metrics: &CandidateMetrics,
    is_watchlist: bool,
    is_top_attention: bool,
    candidate_priority_score: i32,
) -> bool {
    is_watchlist
        || metrics.recent_action_count_14 > 0
        || is_stage_on_calendar_or_later(&context.procedural_stage)
        || metrics.cloture_action_count > 0
        || is_top_attention
        || candidate_priority_score >= 6
        || object.salience >= 0.8
}

fn top_homepage_entries(records: &[CandidateRecord], movement: bool) -> Vec<PublicHomepageEntry> {
    let mut entries = records
        .iter()
        .map(|record| {
            let summary_blurb = if movement {
                format!(
                    "{} stage, {} with procedural path {}",
                    record.context.procedural_stage,
                    if record.analysis.simple_majority_viable {
                        "simple majority viable"
                    } else {
                        "majority still short"
                    },
                    if record.analysis.cloture_viable {
                        "open"
                    } else {
                        "constrained"
                    }
                )
            } else {
                format!(
                    "{} with {} and next event {}",
                    if record.metrics.recent_action_count_14 > 0 {
                        "recent pressure"
                    } else {
                        "rising attention"
                    },
                    if record.next_event.predicted_event.to_string() == "NoMeaningfulMovement" {
                        "slow activation odds"
                    } else {
                        "plausible activation"
                    },
                    record.next_event.predicted_event
                )
            };
            PublicHomepageEntry {
                object_id: record.object.object_id.clone(),
                label: record.label.clone(),
                title: record.object.title.clone(),
                stage: record.context.procedural_stage.to_string(),
                predicted_next_event: record.next_event.predicted_event.to_string(),
                predicted_floor_action: record.floor_action.predicted_action.to_string(),
                next_event_score: record.next_event.predicted_event_score,
                next_event_confidence: record.next_event.confidence,
                majority_viable: record.analysis.simple_majority_viable,
                cloture_viable: record.analysis.cloture_viable,
                candidate_priority_score: record.candidate_priority_score,
                summary_blurb,
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| {
        let left_score = if movement {
            records
                .iter()
                .find(|record| record.object.object_id == left.object_id)
                .map(|record| record.likely_to_move_score)
                .unwrap_or(0.0)
        } else {
            records
                .iter()
                .find(|record| record.object.object_id == left.object_id)
                .map(|record| record.likely_to_get_moving_score)
                .unwrap_or(0.0)
        };
        let right_score = if movement {
            records
                .iter()
                .find(|record| record.object.object_id == right.object_id)
                .map(|record| record.likely_to_move_score)
                .unwrap_or(0.0)
        } else {
            records
                .iter()
                .find(|record| record.object.object_id == right.object_id)
                .map(|record| record.likely_to_get_moving_score)
                .unwrap_or(0.0)
        };
        compare_f32_desc(left_score, right_score).then_with(|| left.object_id.cmp(&right.object_id))
    });
    entries.truncate(10);
    entries
}

fn likely_to_move_score(
    object: &crate::LegislativeObject,
    context: &LegislativeContext,
    floor_action: &FloorActionAssessment,
    next_event: &NextEventPrediction,
    metrics: &CandidateMetrics,
    snapshot_date: NaiveDate,
) -> f32 {
    let recent_action_strength = if metrics.recent_action_count_3 > 0 {
        1.0
    } else if metrics.recent_action_count_7 > 0 {
        0.8
    } else if metrics.recent_action_count_14 > 0 {
        0.6
    } else if metrics.recent_action_count_30 > 0 {
        0.3
    } else {
        0.0
    };
    let procedural_intensity = ((metrics.cloture_action_count as f32 * 0.5)
        + (metrics.amendment_action_count as f32 * 0.2)
        + (metrics.passage_action_count as f32 * 0.35))
        .clamp(0.0, 1.0);
    let inactivity_penalty = metrics
        .latest_action_date
        .map(|date| ((snapshot_date - date).num_days() as f32 / 120.0).clamp(0.0, 0.35))
        .unwrap_or(0.35);
    let floor_action_bonus = if floor_action.predicted_action.to_string() == "LikelyStall" {
        0.0
    } else {
        1.0
    };

    (stage_move_weight(&context.procedural_stage) * 0.23
        + recent_action_strength * 0.19
        + procedural_intensity * 0.14
        + next_event.predicted_event_score * 0.16
        + next_event.confidence * 0.08
        + floor_action_bonus * 0.10
        + context.leadership_priority * 0.05
        + object.salience * 0.05
        + metrics.object_attention * 0.04
        - inactivity_penalty)
        .clamp(0.0, 1.0)
}

fn likely_to_get_moving_score(
    object: &crate::LegislativeObject,
    context: &LegislativeContext,
    floor_action: &FloorActionAssessment,
    next_event: &NextEventPrediction,
    metrics: &CandidateMetrics,
    snapshot_date: NaiveDate,
) -> f32 {
    let early_stage_bonus = match context.procedural_stage {
        ProceduralStage::Introduced | ProceduralStage::InCommittee | ProceduralStage::Reported => 1.0,
        ProceduralStage::OnCalendar => 0.7,
        _ => 0.25,
    };
    let activation_signal = match next_event.predicted_event.to_string().as_str() {
        "LeadershipSignalsAction" | "MotionToProceedAttempted" | "NegotiationIntensifies" => 1.0,
        "NoMeaningfulMovement" => 0.3,
        _ => 0.7,
    };
    let attention_signal = metrics.object_attention.max(metrics.domain_attention);
    let freshness_drag = metrics
        .latest_action_date
        .map(|date| ((snapshot_date - date).num_days() as f32 / 90.0).clamp(0.0, 0.4))
        .unwrap_or(0.15);
    let negotiation_bonus = if floor_action.predicted_action.to_string() == "LikelyNegotiation" {
        1.0
    } else {
        0.35
    };

    (early_stage_bonus * 0.22
        + activation_signal * 0.18
        + next_event.predicted_event_score * 0.16
        + context.leadership_priority * 0.14
        + attention_signal * 0.11
        + metrics.strongest_public_link * 0.06
        + object.salience * 0.08
        + negotiation_bonus * 0.10
        - freshness_drag)
        .clamp(0.0, 1.0)
}

fn recent_action_points(metrics: &CandidateMetrics, snapshot_date: NaiveDate) -> i32 {
    if metrics.recent_action_count_3 > 0 {
        5
    } else if metrics.recent_action_count_7 > 0 {
        4
    } else if metrics.recent_action_count_14 > 0 {
        3
    } else if metrics.recent_action_count_30 > 0 {
        1
    } else if metrics
        .latest_action_date
        .map(|date| (snapshot_date - date).num_days() > 60)
        .unwrap_or(true)
    {
        -3
    } else {
        0
    }
}

fn stage_points(stage: &ProceduralStage) -> i32 {
    match stage {
        ProceduralStage::Introduced => 0,
        ProceduralStage::InCommittee => 1,
        ProceduralStage::Reported => 2,
        ProceduralStage::OnCalendar => 3,
        ProceduralStage::MotionToProceed => 4,
        ProceduralStage::Debate | ProceduralStage::AmendmentPending => 5,
        ProceduralStage::ClotureFiled | ProceduralStage::ClotureVote | ProceduralStage::FinalPassage => 6,
        ProceduralStage::Conference => 4,
        ProceduralStage::Stalled => -1,
        ProceduralStage::Other(_) => 0,
    }
}

fn procedural_intensity_points(metrics: &CandidateMetrics) -> i32 {
    let mut points = 0;
    if metrics.recent_action_count_7 >= 2 || metrics.recent_action_count_14 >= 3 {
        points += 2;
    }
    if metrics.cloture_action_count > 0 {
        points += 3;
    }
    if metrics.amendment_action_count >= 2 {
        points += 1;
    }
    if metrics.passage_action_count > 0 {
        points += 2;
    }
    points
}

fn coalition_prediction_points(
    analysis: &SenateAnalysis,
    floor_action: &FloorActionAssessment,
    next_event: &NextEventPrediction,
) -> i32 {
    let mut points = 0;
    if analysis.simple_majority_viable {
        points += 1;
    }
    if analysis.cloture_viable {
        points += 2;
    }
    if floor_action.predicted_action.to_string() != "LikelyStall" {
        points += 2;
    }
    if next_event.predicted_event.to_string() != "NoMeaningfulMovement" {
        points += 2;
    }
    points += if next_event.predicted_event_score >= 0.75 {
        3
    } else if next_event.predicted_event_score >= 0.60 {
        2
    } else if next_event.predicted_event_score >= 0.50 {
        1
    } else {
        0
    };
    if next_event.confidence >= 0.65 {
        points += 1;
    }
    points
}

fn narrative_points(metrics: &CandidateMetrics) -> i32 {
    let mut points = 0;
    points += if metrics.object_attention >= 0.60 {
        3
    } else if metrics.object_attention >= 0.40 {
        2
    } else if metrics.object_attention >= 0.20 {
        1
    } else {
        0
    };
    if metrics.domain_attention >= 0.45 {
        points += 1;
    }
    if metrics.strongest_public_link >= 0.45 {
        points += 1;
    }
    points
}

fn institutional_points(object: &crate::LegislativeObject) -> i32 {
    let mut points = 0;
    if object.salience >= 0.70 {
        points += 2;
    } else if object.salience >= 0.50 {
        points += 1;
    }
    if object.cosponsors.len() >= 5 {
        points += 1;
    }
    points
}

fn negative_points(
    object: &crate::LegislativeObject,
    next_event: &NextEventPrediction,
    metrics: &CandidateMetrics,
    snapshot_date: NaiveDate,
) -> i32 {
    let mut points = 0;
    if metrics
        .latest_action_date
        .map(|date| (snapshot_date - date).num_days() > 60)
        .unwrap_or(true)
    {
        points -= 3;
    }
    if matches!(object.object_type, crate::model::legislative::LegislativeObjectType::Bill)
        && metrics.recent_action_count_14 == 0
        && next_event.predicted_event.to_string() == "NoMeaningfulMovement"
    {
        points -= 2;
    }
    points
}

fn is_stage_on_calendar_or_later(stage: &ProceduralStage) -> bool {
    matches!(
        stage,
        ProceduralStage::OnCalendar
            | ProceduralStage::MotionToProceed
            | ProceduralStage::Debate
            | ProceduralStage::AmendmentPending
            | ProceduralStage::ClotureFiled
            | ProceduralStage::ClotureVote
            | ProceduralStage::FinalPassage
            | ProceduralStage::Conference
    )
}

fn stage_move_weight(stage: &ProceduralStage) -> f32 {
    match stage {
        ProceduralStage::Introduced => 0.1,
        ProceduralStage::InCommittee => 0.2,
        ProceduralStage::Reported => 0.35,
        ProceduralStage::OnCalendar => 0.55,
        ProceduralStage::MotionToProceed => 0.72,
        ProceduralStage::Debate => 0.82,
        ProceduralStage::AmendmentPending => 0.85,
        ProceduralStage::ClotureFiled => 0.92,
        ProceduralStage::ClotureVote => 0.97,
        ProceduralStage::FinalPassage => 1.0,
        ProceduralStage::Conference => 0.65,
        ProceduralStage::Stalled => 0.1,
        ProceduralStage::Other(_) => 0.2,
    }
}

fn compare_f32_desc(left: f32, right: f32) -> Ordering {
    right.partial_cmp(&left).unwrap_or(Ordering::Equal)
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
        assert_eq!(summary.candidate_count, summary.rows.len());
        assert!(!summary.most_likely_to_move.is_empty());
        assert!(!summary.most_likely_to_get_moving.is_empty());
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
