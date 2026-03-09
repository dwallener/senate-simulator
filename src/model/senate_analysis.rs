use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{
        dynamic_state::PublicPosition,
        legislative_context::ProceduralStage,
        senator_stance::{ProceduralPosture, StanceLabel},
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SenateAnalysis {
    pub object_id: String,
    pub procedural_stage: ProceduralStage,
    pub total_senators: usize,
    pub likely_support_count: usize,
    pub lean_support_count: usize,
    pub undecided_count: usize,
    pub lean_oppose_count: usize,
    pub likely_oppose_count: usize,
    pub expected_present_count: usize,
    pub simple_majority_viable: bool,
    pub cloture_viable: bool,
    pub filibuster_risk: f32,
    pub coalition_stability: f32,
    pub pivotal_senators: Vec<PivotSummary>,
    pub likely_defectors: Vec<SenatorSignalSummary>,
    pub likely_blockers: Vec<SenatorSignalSummary>,
    pub top_findings: Vec<String>,
}

impl SenateAnalysis {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        let bucket_total = self.likely_support_count
            + self.lean_support_count
            + self.undecided_count
            + self.lean_oppose_count
            + self.likely_oppose_count;

        if bucket_total != self.total_senators {
            return Err(SenateSimError::Validation {
                field: "senate_analysis.total_senators",
                message: "bucket counts must sum to total_senators".to_string(),
            });
        }

        if self.expected_present_count > self.total_senators {
            return Err(SenateSimError::Validation {
                field: "senate_analysis.expected_present_count",
                message: "must not exceed total_senators".to_string(),
            });
        }

        validate_probability("senate_analysis.filibuster_risk", self.filibuster_risk)?;
        validate_probability(
            "senate_analysis.coalition_stability",
            self.coalition_stability,
        )?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PivotSummary {
    pub senator_id: String,
    pub stance_label: StanceLabel,
    pub procedural_posture: ProceduralPosture,
    pub substantive_support: f32,
    pub procedural_support: f32,
    pub negotiability: f32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SenatorSignalSummary {
    pub senator_id: String,
    pub public_position: PublicPosition,
    pub defection_probability: f32,
    pub rigidity: f32,
    pub reason: String,
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
