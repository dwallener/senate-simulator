use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{legislative_context::ProceduralStage, senate_analysis::PivotSummary},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FloorActionAssessment {
    pub object_id: String,
    pub procedural_stage: ProceduralStage,
    pub predicted_action: FloorAction,
    pub confidence: f32,
    pub simple_majority_viable: bool,
    pub cloture_viable: bool,
    pub coalition_stability: f32,
    pub filibuster_risk: f32,
    pub support_margin_estimate: i32,
    pub cloture_gap_estimate: i32,
    pub pivotal_senators: Vec<PivotSummary>,
    pub top_reasons: Vec<String>,
}

impl FloorActionAssessment {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.object_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "floor_action_assessment.object_id",
                message: "must not be empty".to_string(),
            });
        }

        validate_probability("floor_action_assessment.confidence", self.confidence)?;
        validate_probability(
            "floor_action_assessment.coalition_stability",
            self.coalition_stability,
        )?;
        validate_probability(
            "floor_action_assessment.filibuster_risk",
            self.filibuster_risk,
        )?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FloorAction {
    LikelyAdvanceToDebate,
    LikelyClotureVote,
    LikelyClotureFailure,
    LikelyFinalPassage,
    LikelyStall,
    LikelyNegotiation,
    LikelyProceduralBlock,
    LikelyAmendmentFight,
}

impl fmt::Display for FloorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::LikelyAdvanceToDebate => "LikelyAdvanceToDebate",
            Self::LikelyClotureVote => "LikelyClotureVote",
            Self::LikelyClotureFailure => "LikelyClotureFailure",
            Self::LikelyFinalPassage => "LikelyFinalPassage",
            Self::LikelyStall => "LikelyStall",
            Self::LikelyNegotiation => "LikelyNegotiation",
            Self::LikelyProceduralBlock => "LikelyProceduralBlock",
            Self::LikelyAmendmentFight => "LikelyAmendmentFight",
        };
        f.write_str(value)
    }
}

fn validate_probability(field: &'static str, value: f32) -> Result<(), SenateSimError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(SenateSimError::Validation {
            field,
            message: "must be between 0 and 1".to_string(),
        });
    }

    Ok(())
}
