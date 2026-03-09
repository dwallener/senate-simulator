use serde::{Deserialize, Serialize};

use crate::error::SenateSimError;

use super::{
    dynamic_state::DynamicState, identity::Identity, issue_preferences::IssuePreferences,
    procedural::Procedural, structural::Structural,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Senator {
    pub identity: Identity,
    pub structural: Structural,
    pub issue_preferences: IssuePreferences,
    pub procedural: Procedural,
    pub dynamic_state: DynamicState,
}

impl Senator {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        validate_required_string("identity.senator_id", &self.identity.senator_id)?;
        validate_required_string("identity.full_name", &self.identity.full_name)?;
        validate_state_code("identity.state", &self.identity.state)?;
        validate_optional_date_order(
            "identity.end_date",
            self.identity.start_date,
            self.identity.end_date,
        )?;

        validate_signed_score("structural.ideology_score", self.structural.ideology_score)?;
        validate_probability(
            "structural.party_loyalty_baseline",
            self.structural.party_loyalty_baseline,
        )?;
        validate_probability(
            "structural.bipartisanship_baseline",
            self.structural.bipartisanship_baseline,
        )?;
        validate_probability(
            "structural.electoral_vulnerability",
            self.structural.electoral_vulnerability,
        )?;
        validate_non_empty_list(
            "structural.committee_assignments",
            &self.structural.committee_assignments,
        )?;

        validate_signed_score("issue_preferences.defense", self.issue_preferences.defense)?;
        validate_signed_score(
            "issue_preferences.immigration",
            self.issue_preferences.immigration,
        )?;
        validate_signed_score(
            "issue_preferences.energy_climate",
            self.issue_preferences.energy_climate,
        )?;
        validate_signed_score("issue_preferences.labor", self.issue_preferences.labor)?;
        validate_signed_score(
            "issue_preferences.healthcare",
            self.issue_preferences.healthcare,
        )?;
        validate_signed_score(
            "issue_preferences.tax_spending",
            self.issue_preferences.tax_spending,
        )?;
        validate_signed_score(
            "issue_preferences.judiciary",
            self.issue_preferences.judiciary,
        )?;
        validate_signed_score("issue_preferences.trade", self.issue_preferences.trade)?;
        validate_signed_score(
            "issue_preferences.tech_privacy",
            self.issue_preferences.tech_privacy,
        )?;
        validate_signed_score(
            "issue_preferences.foreign_policy",
            self.issue_preferences.foreign_policy,
        )?;

        validate_probability(
            "procedural.cloture_support_baseline",
            self.procedural.cloture_support_baseline,
        )?;
        validate_probability(
            "procedural.motion_to_proceed_baseline",
            self.procedural.motion_to_proceed_baseline,
        )?;
        validate_probability(
            "procedural.uc_objection_tendency",
            self.procedural.uc_objection_tendency,
        )?;
        validate_probability(
            "procedural.leadership_deference",
            self.procedural.leadership_deference,
        )?;
        validate_probability(
            "procedural.amendment_openness",
            self.procedural.amendment_openness,
        )?;
        validate_probability(
            "procedural.attendance_reliability",
            self.procedural.attendance_reliability,
        )?;

        validate_probability(
            "dynamic_state.current_substantive_support",
            self.dynamic_state.current_substantive_support,
        )?;
        validate_probability(
            "dynamic_state.current_procedural_support",
            self.dynamic_state.current_procedural_support,
        )?;
        validate_probability(
            "dynamic_state.current_negotiability",
            self.dynamic_state.current_negotiability,
        )?;
        validate_probability(
            "dynamic_state.current_party_pressure",
            self.dynamic_state.current_party_pressure,
        )?;
        validate_probability(
            "dynamic_state.current_issue_salience_in_state",
            self.dynamic_state.current_issue_salience_in_state,
        )?;

        Ok(())
    }
}

fn validate_required_string(field: &'static str, value: &str) -> Result<(), SenateSimError> {
    if value.trim().is_empty() {
        return Err(SenateSimError::Validation {
            field,
            message: "must not be empty".to_string(),
        });
    }
    Ok(())
}

fn validate_non_empty_list(field: &'static str, values: &[String]) -> Result<(), SenateSimError> {
    if values.is_empty() {
        return Err(SenateSimError::Validation {
            field,
            message: "must contain at least one entry".to_string(),
        });
    }

    for value in values {
        validate_required_string(field, value)?;
    }

    Ok(())
}

fn validate_state_code(field: &'static str, value: &str) -> Result<(), SenateSimError> {
    let trimmed = value.trim();
    let is_valid = trimmed.len() == 2 && trimmed.chars().all(|ch| ch.is_ascii_uppercase());

    if !is_valid {
        return Err(SenateSimError::Validation {
            field,
            message: "must be a 2-letter uppercase code".to_string(),
        });
    }

    Ok(())
}

fn validate_signed_score(field: &'static str, value: f32) -> Result<(), SenateSimError> {
    validate_range(field, value, -1.0, 1.0)
}

fn validate_probability(field: &'static str, value: f32) -> Result<(), SenateSimError> {
    validate_range(field, value, 0.0, 1.0)
}

fn validate_range(
    field: &'static str,
    value: f32,
    min: f32,
    max: f32,
) -> Result<(), SenateSimError> {
    if !value.is_finite() || value < min || value > max {
        return Err(SenateSimError::Validation {
            field,
            message: format!("must be between {min} and {max}"),
        });
    }
    Ok(())
}

fn validate_optional_date_order(
    field: &'static str,
    start_date: chrono::NaiveDate,
    end_date: Option<chrono::NaiveDate>,
) -> Result<(), SenateSimError> {
    if let Some(end_date) = end_date {
        if end_date < start_date {
            return Err(SenateSimError::Validation {
                field,
                message: "must not be earlier than start_date".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::model::{
        dynamic_state::{DynamicState, PublicPosition},
        identity::{Identity, Party, SenateClass},
        issue_preferences::IssuePreferences,
        procedural::Procedural,
        structural::Structural,
    };

    use super::Senator;

    fn example_senator() -> Senator {
        Senator {
            identity: Identity {
                senator_id: "sen_001".to_string(),
                full_name: "Example Senator".to_string(),
                party: Party::Independent,
                state: "OR".to_string(),
                class: SenateClass::II,
                start_date: NaiveDate::from_ymd_opt(2023, 1, 3).unwrap(),
                end_date: Some(NaiveDate::from_ymd_opt(2029, 1, 3).unwrap()),
            },
            structural: Structural {
                ideology_score: 0.15,
                party_loyalty_baseline: 0.72,
                bipartisanship_baseline: 0.44,
                committee_assignments: vec![
                    "Budget".to_string(),
                    "Energy and Natural Resources".to_string(),
                ],
                reelection_year: Some(2028),
                electoral_vulnerability: 0.31,
            },
            issue_preferences: IssuePreferences {
                defense: 0.1,
                immigration: -0.2,
                energy_climate: 0.7,
                labor: 0.4,
                healthcare: 0.5,
                tax_spending: -0.1,
                judiciary: 0.2,
                trade: 0.0,
                tech_privacy: 0.6,
                foreign_policy: 0.25,
            },
            procedural: Procedural {
                cloture_support_baseline: 0.66,
                motion_to_proceed_baseline: 0.7,
                uc_objection_tendency: 0.18,
                leadership_deference: 0.54,
                amendment_openness: 0.77,
                attendance_reliability: 0.95,
            },
            dynamic_state: DynamicState {
                current_public_position: PublicPosition::Negotiating,
                current_substantive_support: 0.58,
                current_procedural_support: 0.63,
                current_negotiability: 0.81,
                current_party_pressure: 0.4,
                current_issue_salience_in_state: 0.67,
            },
        }
    }

    #[test]
    fn validates_example_senator() {
        let senator = example_senator();
        assert!(senator.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_state_code() {
        let mut senator = example_senator();
        senator.identity.state = "Oregon".to_string();

        let error = senator.validate().unwrap_err();
        assert!(error.to_string().contains("identity.state"));
    }

    #[test]
    fn rejects_out_of_range_issue_preference() {
        let mut senator = example_senator();
        senator.issue_preferences.defense = 1.5;

        let error = senator.validate().unwrap_err();
        assert!(error.to_string().contains("issue_preferences.defense"));
    }
}
