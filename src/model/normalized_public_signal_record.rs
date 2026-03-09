use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{legislative::PolicyDomain, raw_public_signal_record::PublicSignalScope},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPublicSignalRecord {
    pub snapshot_date: NaiveDate,
    pub signal_id: String,
    pub signal_scope: PublicSignalScope,
    pub linked_senator_id: Option<String>,
    pub linked_object_id: Option<String>,
    pub policy_domain: Option<PolicyDomain>,
    pub mention_count: u32,
    pub attention_score: f32,
    pub tone_score: Option<f32>,
    pub source_count: Option<u32>,
    pub top_themes: Vec<String>,
    pub top_persons: Vec<String>,
    pub top_organizations: Vec<String>,
}

impl NormalizedPublicSignalRecord {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.signal_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_public_signal_record.signal_id",
                message: "must not be empty".to_string(),
            });
        }
        if !self.attention_score.is_finite() || !(0.0..=1.0).contains(&self.attention_score) {
            return Err(SenateSimError::Validation {
                field: "normalized_public_signal_record.attention_score",
                message: "must be between 0 and 1".to_string(),
            });
        }
        if let Some(tone_score) = self.tone_score {
            if !tone_score.is_finite() || !(-1.0..=1.0).contains(&tone_score) {
                return Err(SenateSimError::Validation {
                    field: "normalized_public_signal_record.tone_score",
                    message: "must be between -1 and 1".to_string(),
                });
            }
        }
        for field in [&self.top_themes, &self.top_persons, &self.top_organizations] {
            for value in field {
                if value.trim().is_empty() {
                    return Err(SenateSimError::Validation {
                        field: "normalized_public_signal_record.metadata",
                        message: "must not contain empty strings".to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}
