use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{error::SenateSimError, model::dynamic_state::PublicPosition};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SenatorStance {
    pub senator_id: String,
    pub object_id: String,
    pub context_id: Option<String>,
    pub substantive_support: f32,
    pub procedural_support: f32,
    pub public_support: f32,
    pub negotiability: f32,
    pub rigidity: f32,
    pub defection_probability: f32,
    pub absence_probability: f32,
    pub stance_label: StanceLabel,
    pub procedural_posture: ProceduralPosture,
    pub public_position: PublicPosition,
    pub top_factors: Vec<String>,
}

impl SenatorStance {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        validate_required_string("senator_stance.senator_id", &self.senator_id)?;
        validate_required_string("senator_stance.object_id", &self.object_id)?;

        if let Some(context_id) = &self.context_id {
            validate_required_string("senator_stance.context_id", context_id)?;
        }

        validate_probability(
            "senator_stance.substantive_support",
            self.substantive_support,
        )?;
        validate_probability("senator_stance.procedural_support", self.procedural_support)?;
        validate_probability("senator_stance.public_support", self.public_support)?;
        validate_probability("senator_stance.negotiability", self.negotiability)?;
        validate_probability("senator_stance.rigidity", self.rigidity)?;
        validate_probability(
            "senator_stance.defection_probability",
            self.defection_probability,
        )?;
        validate_probability(
            "senator_stance.absence_probability",
            self.absence_probability,
        )?;

        for factor in &self.top_factors {
            validate_required_string("senator_stance.top_factors", factor)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StanceLabel {
    Support,
    LeanSupport,
    Undecided,
    LeanOppose,
    Oppose,
}

impl fmt::Display for StanceLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Support => "Support",
            Self::LeanSupport => "LeanSupport",
            Self::Undecided => "Undecided",
            Self::LeanOppose => "LeanOppose",
            Self::Oppose => "Oppose",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProceduralPosture {
    SupportDebate,
    OpposeDebate,
    SupportCloture,
    OpposeCloture,
    SupportAmendmentProcess,
    BlockByProcedure,
    Unclear,
}

impl fmt::Display for ProceduralPosture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::SupportDebate => "SupportDebate",
            Self::OpposeDebate => "OpposeDebate",
            Self::SupportCloture => "SupportCloture",
            Self::OpposeCloture => "OpposeCloture",
            Self::SupportAmendmentProcess => "SupportAmendmentProcess",
            Self::BlockByProcedure => "BlockByProcedure",
            Self::Unclear => "Unclear",
        };
        f.write_str(value)
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

fn validate_probability(field: &'static str, value: f32) -> Result<(), SenateSimError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(SenateSimError::Validation {
            field,
            message: "must be between 0 and 1".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ProceduralPosture, SenatorStance, StanceLabel};
    use crate::model::dynamic_state::PublicPosition;

    fn example_stance() -> SenatorStance {
        SenatorStance {
            senator_id: "sen_001".to_string(),
            object_id: "obj_001".to_string(),
            context_id: Some("ctx_001".to_string()),
            substantive_support: 0.63,
            procedural_support: 0.71,
            public_support: 0.58,
            negotiability: 0.69,
            rigidity: 0.24,
            defection_probability: 0.17,
            absence_probability: 0.02,
            stance_label: StanceLabel::LeanSupport,
            procedural_posture: ProceduralPosture::SupportCloture,
            public_position: PublicPosition::Negotiating,
            top_factors: vec![
                "leadership support is high".to_string(),
                "substantive concerns remain negotiable".to_string(),
            ],
        }
    }

    #[test]
    fn validates_senator_stance() {
        assert!(example_stance().validate().is_ok());
    }

    #[test]
    fn rejects_out_of_range_defection_probability() {
        let mut stance = example_stance();
        stance.defection_probability = 1.1;

        let error = stance.validate().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("senator_stance.defection_probability")
        );
    }
}
