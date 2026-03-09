use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{
        legislative_context::ProceduralStage, next_event_prediction::EventScore,
        senate_event::SenateEvent,
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationStep {
    pub step_index: usize,
    pub starting_stage: ProceduralStage,
    pub predicted_event: SenateEvent,
    pub confidence: f32,
    pub analysis_summary: StepAnalysisSummary,
    pub alternative_events: Vec<EventScore>,
    pub top_reasons: Vec<String>,
}

impl SimulationStep {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        validate_probability("simulation_step.confidence", self.confidence)?;
        for alternative in &self.alternative_events {
            validate_probability(
                "simulation_step.alternative_events.score",
                alternative.score,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepAnalysisSummary {
    pub likely_support_count: usize,
    pub lean_support_count: usize,
    pub undecided_count: usize,
    pub likely_oppose_count: usize,
    pub simple_majority_viable: bool,
    pub cloture_viable: bool,
    pub coalition_stability: f32,
    pub filibuster_risk: f32,
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
