use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{
        legislative::LegislativeObject, legislative_context::LegislativeContext,
        senate_event::SenateEvent, senator::Senator,
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulationState {
    pub legislative_object: LegislativeObject,
    pub context: LegislativeContext,
    pub roster: Vec<Senator>,
    pub step_index: usize,
    pub last_event: Option<SenateEvent>,
    pub consecutive_no_movement: usize,
    pub days_elapsed: i32,
    pub cloture_attempts: i32,
}

impl SimulationState {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        self.legislative_object.validate()?;
        self.context.validate()?;

        if self.roster.is_empty() {
            return Err(SenateSimError::Validation {
                field: "simulation_state.roster",
                message: "must contain at least one senator".to_string(),
            });
        }

        if self.days_elapsed < 0 {
            return Err(SenateSimError::Validation {
                field: "simulation_state.days_elapsed",
                message: "must be non-negative".to_string(),
            });
        }

        if self.cloture_attempts < 0 {
            return Err(SenateSimError::Validation {
                field: "simulation_state.cloture_attempts",
                message: "must be non-negative".to_string(),
            });
        }

        for senator in &self.roster {
            senator.validate()?;
        }

        Ok(())
    }
}
