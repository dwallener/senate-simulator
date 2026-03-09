use serde::{Deserialize, Serialize};

use crate::error::SenateSimError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StanceScoreBreakdown {
    pub domain_affinity_score: f32,
    pub procedural_compatibility_score: f32,
    pub party_alignment_score: f32,
    pub salience_adjustment: f32,
    pub controversy_adjustment: f32,
    pub recent_drift_adjustment: f32,
    pub attendance_adjustment: f32,
    pub coverage_score: f32,
    pub fallback_notes: Vec<String>,
    pub top_factors: Vec<String>,
}

impl StanceScoreBreakdown {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        for (field, value) in [
            (
                "stance_score_breakdown.domain_affinity_score",
                self.domain_affinity_score,
            ),
            (
                "stance_score_breakdown.procedural_compatibility_score",
                self.procedural_compatibility_score,
            ),
            (
                "stance_score_breakdown.party_alignment_score",
                self.party_alignment_score,
            ),
            ("stance_score_breakdown.coverage_score", self.coverage_score),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(SenateSimError::Validation {
                    field,
                    message: "must be between 0 and 1".to_string(),
                });
            }
        }

        for (field, value) in [
            (
                "stance_score_breakdown.salience_adjustment",
                self.salience_adjustment,
            ),
            (
                "stance_score_breakdown.controversy_adjustment",
                self.controversy_adjustment,
            ),
            (
                "stance_score_breakdown.recent_drift_adjustment",
                self.recent_drift_adjustment,
            ),
            (
                "stance_score_breakdown.attendance_adjustment",
                self.attendance_adjustment,
            ),
        ] {
            if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
                return Err(SenateSimError::Validation {
                    field,
                    message: "must be between -1 and 1".to_string(),
                });
            }
        }

        Ok(())
    }
}
