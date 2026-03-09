use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{legislative_context::ProceduralStage, senate_event::SenateEvent},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NextEventPrediction {
    pub object_id: String,
    pub current_stage: ProceduralStage,
    pub predicted_event: SenateEvent,
    pub confidence: f32,
    pub alternative_events: Vec<EventScore>,
    pub top_reasons: Vec<String>,
    pub simple_majority_viable: bool,
    pub cloture_viable: bool,
    pub coalition_stability: f32,
    pub filibuster_risk: f32,
}

impl NextEventPrediction {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.object_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "next_event_prediction.object_id",
                message: "must not be empty".to_string(),
            });
        }

        validate_probability("next_event_prediction.confidence", self.confidence)?;
        validate_probability(
            "next_event_prediction.coalition_stability",
            self.coalition_stability,
        )?;
        validate_probability(
            "next_event_prediction.filibuster_risk",
            self.filibuster_risk,
        )?;

        for alternative in &self.alternative_events {
            validate_probability(
                "next_event_prediction.alternative_events.score",
                alternative.score,
            )?;
        }

        if self
            .alternative_events
            .windows(2)
            .any(|pair| pair[0].score < pair[1].score)
        {
            return Err(SenateSimError::Validation {
                field: "next_event_prediction.alternative_events",
                message: "must be sorted in descending score order".to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventScore {
    pub event: SenateEvent,
    pub score: f32,
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
