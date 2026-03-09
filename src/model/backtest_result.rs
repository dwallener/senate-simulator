use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{normalized_records::NormalizedActionCategory, senate_event::SenateEvent},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BacktestResult {
    pub snapshot_date: NaiveDate,
    pub object_id: String,
    pub predicted_next_event: Option<SenateEvent>,
    pub actual_next_event: Option<NormalizedActionCategory>,
    pub match_top_1: bool,
    pub match_top_k: bool,
    pub prediction_confidence: Option<f32>,
    pub notes: Vec<String>,
}

impl BacktestResult {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.object_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "backtest_result.object_id",
                message: "must not be empty".to_string(),
            });
        }

        if let Some(confidence) = self.prediction_confidence {
            if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
                return Err(SenateSimError::Validation {
                    field: "backtest_result.prediction_confidence",
                    message: "must be between 0 and 1".to_string(),
                });
            }
        }

        Ok(())
    }
}
