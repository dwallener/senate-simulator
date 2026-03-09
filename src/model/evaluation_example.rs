use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{legislative_context::ProceduralStage, senate_event::SenateEvent},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationExample {
    pub example_id: String,
    pub snapshot_date: NaiveDate,
    pub object_id: String,
    pub current_stage: Option<ProceduralStage>,
    pub actual_next_event: Option<SenateEvent>,
    pub actual_next_event_date: Option<NaiveDate>,
    pub snapshot_path: String,
    pub timeline_position: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationSummary {
    pub total_examples: usize,
    pub top_1_next_event_accuracy: f32,
    pub top_k_next_event_accuracy: f32,
    pub trajectory_prefix_match_rate: f32,
    pub unscorable_examples: usize,
    pub notes: Vec<String>,
}

impl EvaluationSummary {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        for (field, value) in [
            (
                "evaluation_summary.top_1_next_event_accuracy",
                self.top_1_next_event_accuracy,
            ),
            (
                "evaluation_summary.top_k_next_event_accuracy",
                self.top_k_next_event_accuracy,
            ),
            (
                "evaluation_summary.trajectory_prefix_match_rate",
                self.trajectory_prefix_match_rate,
            ),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(SenateSimError::Validation {
                    field,
                    message: "must be between 0 and 1".to_string(),
                });
            }
        }

        Ok(())
    }
}
