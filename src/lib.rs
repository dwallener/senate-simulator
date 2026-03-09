pub mod analysis;
pub mod derive;
pub mod error;
pub mod io;
pub mod model;
pub mod synthetic;

pub use analysis::chamber::analyze_chamber;
pub use analysis::floor_action::assess_floor_action;
pub use derive::stance::derive_stance;
pub use error::SenateSimError;
pub use model::dynamic_state::PublicPosition;
pub use model::floor_action_assessment::{FloorAction, FloorActionAssessment};
pub use model::identity::{Party, SenateClass};
pub use model::legislative::{
    BudgetaryImpact, LegislativeObject, LegislativeObjectType, PolicyDomain,
};
pub use model::legislative_context::{
    Chamber, CongressionalSession, LegislativeContext, ProceduralStage,
};
pub use model::scenario::SenatorScenario;
pub use model::senate_analysis::{PivotSummary, SenateAnalysis, SenatorSignalSummary};
pub use model::senator::Senator;
pub use model::senator_stance::{ProceduralPosture, SenatorStance, StanceLabel};
pub use synthetic::roster::build_synthetic_senate;
