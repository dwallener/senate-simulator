use serde::{Deserialize, Serialize};

use super::{
    legislative::LegislativeObject, legislative_context::LegislativeContext, senator::Senator,
    senator_stance::SenatorStance,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SenatorScenario {
    pub senator: Senator,
    pub legislative_object: LegislativeObject,
    pub context: LegislativeContext,
    pub stance: SenatorStance,
}
