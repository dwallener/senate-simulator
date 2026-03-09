pub mod analysis;
pub mod backtest;
pub mod derive;
pub mod error;
pub mod ingest;
pub mod io;
pub mod model;
pub mod simulation;
pub mod synthetic;

pub use analysis::chamber::analyze_chamber;
pub use analysis::floor_action::assess_floor_action;
pub use analysis::transition::predict_next_event;
pub use backtest::runner::run_backtest;
pub use derive::stance::derive_stance;
pub use error::SenateSimError;
pub use ingest::{
    run_daily_ingestion, run_daily_ingestion_with_roots, snapshot_to_contexts,
    snapshot_to_legislative_objects, snapshot_to_senators,
};
pub use model::backtest_result::BacktestResult;
pub use model::data_snapshot::{DataSnapshot, SourceManifest};
pub use model::dynamic_state::PublicPosition;
pub use model::floor_action_assessment::{FloorAction, FloorActionAssessment};
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
    NormalizedSenatorRecord,
};
pub use model::raw_records::{RawActionRecord, RawLegislativeRecord, RawRosterRecord};
pub use model::scenario::SenatorScenario;
pub use model::senate_analysis::{PivotSummary, SenateAnalysis, SenatorSignalSummary};
pub use model::senate_event::SenateEvent;
pub use model::senator::Senator;
pub use model::senator_stance::{ProceduralPosture, SenatorStance, StanceLabel};
pub use model::simulation_state::SimulationState;
pub use model::simulation_step::{SimulationStep, StepAnalysisSummary};
pub use model::trajectory_result::{TerminationReason, TrajectoryResult};
pub use simulation::apply::apply_event;
pub use simulation::rollout::rollout;
pub use synthetic::roster::build_synthetic_senate;
