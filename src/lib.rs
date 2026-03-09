pub mod analysis;
pub mod backtest;
pub mod derive;
pub mod error;
pub mod eval;
pub mod features;
pub mod ingest;
pub mod io;
pub mod model;
pub mod public_artifacts;
pub mod simulation;
pub mod synthetic;

pub use analysis::chamber::analyze_chamber;
pub use analysis::floor_action::assess_floor_action;
pub use analysis::transition::predict_next_event;
pub use backtest::runner::{run_backtest, run_backtest_with_mode};
pub use derive::feature_driven::derive_stance_feature_driven;
pub use derive::stance::{derive_stance_heuristic, derive_stance_with_mode};
pub use derive::StanceDerivationMode;
pub use derive::stance::derive_stance;
pub use error::SenateSimError;
pub use eval::align::{align_action_to_senate_event, is_consequential_action};
pub use eval::examples::{
    build_evaluation_artifacts, build_evaluation_artifacts_for_snapshot_date,
    generate_actual_trajectory, generate_next_event_examples, load_evaluation_artifacts,
    persist_evaluation_artifacts,
};
pub use eval::runner::{
    evaluate_from_snapshot_date, evaluate_from_snapshot_date_with_mode,
    evaluate_snapshot_examples,
};
pub use features::materialize::{
    build_and_persist_features, feature_record_to_senator, senators_for_snapshot,
    snapshot_with_features_to_senators, SenatorProfileMode,
};
pub use features::senator::{
    build_feature_report, build_senator_features_for_snapshot, load_feature_records,
    load_feature_report, persist_feature_artifacts,
};
pub use features::windows::FeatureWindowConfig;
pub use eval::timeline::build_historical_timelines;
pub use ingest::{
    IngestionConfig, IngestionSourceMode, load_snapshot, run_daily_ingestion, run_daily_ingestion_with_roots,
    run_ingestion, run_live_ingestion, run_live_ingestion_with_roots, snapshot_to_contexts,
    snapshot_to_legislative_objects, snapshot_to_senators,
};
pub use model::action_alignment::AlignmentReport;
pub use model::actual_trajectory::{ActualTrajectory, ActualTrajectoryEvent};
pub use model::backtest_result::BacktestResult;
pub use model::data_snapshot::{DataSnapshot, SourceManifest};
pub use model::dynamic_state::PublicPosition;
pub use model::evaluation_example::{EvaluationExample, EvaluationSummary};
pub use model::floor_action_assessment::{FloorAction, FloorActionAssessment};
pub use model::historical_timeline::{HistoricalActionEvent, HistoricalTimeline};
pub use model::identity::{Party, SenateClass};
pub use model::legislative::{
    BudgetaryImpact, LegislativeObject, LegislativeObjectType, PolicyDomain,
};
pub use model::legislative_context::{
    Chamber, CongressionalSession, LegislativeContext, ProceduralStage,
};
pub use model::next_event_prediction::{EventScore, NextEventPrediction};
pub use model::normalized_records::{
    NormalizedActionCategory, NormalizedActionRecord, NormalizedLegislativeRecord,
    NormalizedSenatorRecord, NormalizedVoteRecord, ProceduralKind, VoteCategory, VotePosition,
};
pub use model::normalized_public_signal_record::NormalizedPublicSignalRecord;
pub use model::public_signal_summary::{PublicSignalSummary, SenatorObjectSignalLink};
pub use model::raw_records::{RawActionRecord, RawLegislativeRecord, RawRosterRecord, RawVoteRecord};
pub use model::raw_public_signal_record::{PublicSignalScope, RawPublicSignalRecord};
pub use model::senator_feature_record::{FeatureReport, SenatorFeatureRecord};
pub use model::scenario::SenatorScenario;
pub use model::senate_analysis::{PivotSummary, SenateAnalysis, SenatorSignalSummary};
pub use model::senate_event::SenateEvent;
pub use model::senator::Senator;
pub use model::senator_stance::{ProceduralPosture, SenatorStance, StanceLabel};
pub use model::stance_score_breakdown::StanceScoreBreakdown;
pub use model::simulation_state::SimulationState;
pub use model::simulation_step::{SimulationStep, StepAnalysisSummary};
pub use model::trajectory_result::{TerminationReason, TrajectoryResult};
pub use public_artifacts::{
    export_public_artifacts, export_public_artifacts_with_roots, load_tracked_bills_manifest,
    ExportArtifacts, PublicBillDetail, PublicLastUpdated, PublicSummary, PublicTrackedBill,
    PublicTrackedBills, TrackedBillEntry, TrackedBillsManifest,
};
pub use simulation::apply::apply_event;
pub use simulation::rollout::{rollout, rollout_with_mode};
pub use synthetic::roster::build_synthetic_senate;
