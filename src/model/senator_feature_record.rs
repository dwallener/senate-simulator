use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{error::SenateSimError, model::identity::Party};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SenatorFeatureRecord {
    pub snapshot_date: NaiveDate,
    pub senator_id: String,
    pub full_name: String,
    pub party: Party,
    pub state: String,
    pub party_loyalty_baseline: f32,
    pub bipartisanship_baseline: f32,
    pub attendance_reliability: f32,
    pub ideology_proxy: f32,
    pub cloture_support_baseline: f32,
    pub motion_to_proceed_baseline: f32,
    pub amendment_openness: f32,
    pub procedural_rigidity: f32,
    pub defense_score: f32,
    pub budget_tax_score: f32,
    pub healthcare_score: f32,
    pub immigration_score: f32,
    pub energy_climate_score: f32,
    pub judiciary_score: f32,
    pub technology_score: f32,
    pub foreign_policy_score: f32,
    pub labor_score: f32,
    pub education_score: f32,
    pub recent_party_loyalty: f32,
    pub recent_bipartisanship: f32,
    pub recent_cloture_support: f32,
    pub recent_attendance_reliability: f32,
    pub historical_vote_count: usize,
    pub recent_vote_count: usize,
    pub coverage_score: f32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureReport {
    pub snapshot_date: NaiveDate,
    pub senators_processed: usize,
    pub senators_with_sparse_history: usize,
    pub average_coverage_score: f32,
    pub notes: Vec<String>,
}

impl SenatorFeatureRecord {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.senator_id.trim().is_empty() || self.full_name.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "senator_feature_record.senator_id",
                message: "senator_id and full_name must not be empty".to_string(),
            });
        }
        if self.state.len() != 2 || !self.state.chars().all(|value| value.is_ascii_uppercase()) {
            return Err(SenateSimError::Validation {
                field: "senator_feature_record.state",
                message: "must be a 2-letter uppercase code".to_string(),
            });
        }

        for (field, value) in [
            ("senator_feature_record.party_loyalty_baseline", self.party_loyalty_baseline),
            (
                "senator_feature_record.bipartisanship_baseline",
                self.bipartisanship_baseline,
            ),
            (
                "senator_feature_record.attendance_reliability",
                self.attendance_reliability,
            ),
            (
                "senator_feature_record.cloture_support_baseline",
                self.cloture_support_baseline,
            ),
            (
                "senator_feature_record.motion_to_proceed_baseline",
                self.motion_to_proceed_baseline,
            ),
            (
                "senator_feature_record.amendment_openness",
                self.amendment_openness,
            ),
            (
                "senator_feature_record.procedural_rigidity",
                self.procedural_rigidity,
            ),
            (
                "senator_feature_record.recent_party_loyalty",
                self.recent_party_loyalty,
            ),
            (
                "senator_feature_record.recent_bipartisanship",
                self.recent_bipartisanship,
            ),
            (
                "senator_feature_record.recent_cloture_support",
                self.recent_cloture_support,
            ),
            (
                "senator_feature_record.recent_attendance_reliability",
                self.recent_attendance_reliability,
            ),
            ("senator_feature_record.coverage_score", self.coverage_score),
        ] {
            validate_unit(field, value)?;
        }

        for (field, value) in [
            ("senator_feature_record.ideology_proxy", self.ideology_proxy),
            ("senator_feature_record.defense_score", self.defense_score),
            ("senator_feature_record.budget_tax_score", self.budget_tax_score),
            ("senator_feature_record.healthcare_score", self.healthcare_score),
            ("senator_feature_record.immigration_score", self.immigration_score),
            (
                "senator_feature_record.energy_climate_score",
                self.energy_climate_score,
            ),
            ("senator_feature_record.judiciary_score", self.judiciary_score),
            ("senator_feature_record.technology_score", self.technology_score),
            (
                "senator_feature_record.foreign_policy_score",
                self.foreign_policy_score,
            ),
            ("senator_feature_record.labor_score", self.labor_score),
            ("senator_feature_record.education_score", self.education_score),
        ] {
            validate_signed(field, value)?;
        }

        Ok(())
    }
}

fn validate_unit(field: &'static str, value: f32) -> Result<(), SenateSimError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(SenateSimError::Validation {
            field,
            message: "must be between 0 and 1".to_string(),
        });
    }
    Ok(())
}

fn validate_signed(field: &'static str, value: f32) -> Result<(), SenateSimError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(SenateSimError::Validation {
            field,
            message: "must be between -1 and 1".to_string(),
        });
    }
    Ok(())
}
