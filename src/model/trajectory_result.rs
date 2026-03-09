use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{simulation_state::SimulationState, simulation_step::SimulationStep},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryResult {
    pub steps: Vec<SimulationStep>,
    pub final_state: SimulationState,
    pub terminated_reason: TerminationReason,
}

impl TrajectoryResult {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        self.final_state.validate()?;

        for (index, step) in self.steps.iter().enumerate() {
            if step.step_index != index {
                return Err(SenateSimError::Validation {
                    field: "trajectory_result.steps",
                    message: "step indices must be monotonic and zero-based".to_string(),
                });
            }
            step.validate()?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminationReason {
    ReachedHorizon,
    ReachedTerminalEvent,
    NoMeaningfulFurtherMovement,
    LoopDetected,
}
