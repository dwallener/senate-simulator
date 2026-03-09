use std::fmt;

use chrono::NaiveDate;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::SenateSimError;

use super::legislative_context::Chamber;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegislativeObject {
    pub object_id: String,
    pub title: String,
    pub object_type: LegislativeObjectType,
    pub policy_domain: PolicyDomain,
    pub summary: String,
    pub text_embedding_placeholder: Option<String>,
    pub sponsor: Option<String>,
    pub cosponsors: Vec<String>,
    pub origin_chamber: Chamber,
    pub introduced_date: NaiveDate,
    pub current_version_label: Option<String>,
    pub budgetary_impact: BudgetaryImpact,
    pub salience: f32,
    pub controversy: f32,
}

impl LegislativeObject {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        validate_required_string("legislative_object.object_id", &self.object_id)?;
        validate_required_string("legislative_object.title", &self.title)?;
        validate_required_string("legislative_object.summary", &self.summary)?;

        if let Some(sponsor) = &self.sponsor {
            validate_required_string("legislative_object.sponsor", sponsor)?;
        }

        if let Some(placeholder) = &self.text_embedding_placeholder {
            validate_required_string("legislative_object.text_embedding_placeholder", placeholder)?;
        }

        if let Some(label) = &self.current_version_label {
            validate_required_string("legislative_object.current_version_label", label)?;
        }

        for cosponsor in &self.cosponsors {
            validate_required_string("legislative_object.cosponsors", cosponsor)?;
        }

        validate_probability("legislative_object.salience", self.salience)?;
        validate_probability("legislative_object.controversy", self.controversy)?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LegislativeObjectType {
    Bill,
    Amendment,
    Motion,
    ClotureVote,
    Nomination,
    Resolution,
    UnanimousConsentRequest,
    SubstituteAmendment,
    Other(String),
}

impl LegislativeObjectType {
    fn as_str(&self) -> &str {
        match self {
            Self::Bill => "Bill",
            Self::Amendment => "Amendment",
            Self::Motion => "Motion",
            Self::ClotureVote => "ClotureVote",
            Self::Nomination => "Nomination",
            Self::Resolution => "Resolution",
            Self::UnanimousConsentRequest => "UnanimousConsentRequest",
            Self::SubstituteAmendment => "SubstituteAmendment",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl fmt::Display for LegislativeObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for LegislativeObjectType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for LegislativeObjectType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let normalized = value.trim();

        if normalized.eq_ignore_ascii_case("bill") {
            Ok(Self::Bill)
        } else if normalized.eq_ignore_ascii_case("amendment") {
            Ok(Self::Amendment)
        } else if normalized.eq_ignore_ascii_case("motion") {
            Ok(Self::Motion)
        } else if normalized.eq_ignore_ascii_case("cloturevote")
            || normalized.eq_ignore_ascii_case("cloture_vote")
        {
            Ok(Self::ClotureVote)
        } else if normalized.eq_ignore_ascii_case("nomination") {
            Ok(Self::Nomination)
        } else if normalized.eq_ignore_ascii_case("resolution") {
            Ok(Self::Resolution)
        } else if normalized.eq_ignore_ascii_case("unanimousconsentrequest")
            || normalized.eq_ignore_ascii_case("unanimous_consent_request")
        {
            Ok(Self::UnanimousConsentRequest)
        } else if normalized.eq_ignore_ascii_case("substituteamendment")
            || normalized.eq_ignore_ascii_case("substitute_amendment")
        {
            Ok(Self::SubstituteAmendment)
        } else {
            Ok(Self::Other(normalized.to_string()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PolicyDomain {
    Defense,
    BudgetTax,
    Healthcare,
    Immigration,
    EnergyClimate,
    Judiciary,
    Technology,
    ForeignPolicy,
    Labor,
    Education,
    Other(String),
}

impl PolicyDomain {
    fn as_str(&self) -> &str {
        match self {
            Self::Defense => "Defense",
            Self::BudgetTax => "BudgetTax",
            Self::Healthcare => "Healthcare",
            Self::Immigration => "Immigration",
            Self::EnergyClimate => "EnergyClimate",
            Self::Judiciary => "Judiciary",
            Self::Technology => "Technology",
            Self::ForeignPolicy => "ForeignPolicy",
            Self::Labor => "Labor",
            Self::Education => "Education",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl fmt::Display for PolicyDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for PolicyDomain {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for PolicyDomain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let normalized = value.trim();

        if normalized.eq_ignore_ascii_case("defense") {
            Ok(Self::Defense)
        } else if normalized.eq_ignore_ascii_case("budgettax")
            || normalized.eq_ignore_ascii_case("budget_tax")
        {
            Ok(Self::BudgetTax)
        } else if normalized.eq_ignore_ascii_case("healthcare") {
            Ok(Self::Healthcare)
        } else if normalized.eq_ignore_ascii_case("immigration") {
            Ok(Self::Immigration)
        } else if normalized.eq_ignore_ascii_case("energyclimate")
            || normalized.eq_ignore_ascii_case("energy_climate")
        {
            Ok(Self::EnergyClimate)
        } else if normalized.eq_ignore_ascii_case("judiciary") {
            Ok(Self::Judiciary)
        } else if normalized.eq_ignore_ascii_case("technology") {
            Ok(Self::Technology)
        } else if normalized.eq_ignore_ascii_case("foreignpolicy")
            || normalized.eq_ignore_ascii_case("foreign_policy")
        {
            Ok(Self::ForeignPolicy)
        } else if normalized.eq_ignore_ascii_case("labor") {
            Ok(Self::Labor)
        } else if normalized.eq_ignore_ascii_case("education") {
            Ok(Self::Education)
        } else {
            Ok(Self::Other(normalized.to_string()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetaryImpact {
    Low,
    Moderate,
    High,
    Unknown,
}

impl fmt::Display for BudgetaryImpact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::Unknown => "Unknown",
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
    use chrono::NaiveDate;

    use crate::model::legislative_context::Chamber;

    use super::{BudgetaryImpact, LegislativeObject, LegislativeObjectType, PolicyDomain};

    fn example_legislative_object() -> LegislativeObject {
        LegislativeObject {
            object_id: "obj_001".to_string(),
            title: "Clean Grid Permitting Reform Act".to_string(),
            object_type: LegislativeObjectType::Bill,
            policy_domain: PolicyDomain::EnergyClimate,
            summary: "Streamlines federal review for transmission corridors and paired clean energy projects."
                .to_string(),
            text_embedding_placeholder: Some("embedding_v1_placeholder".to_string()),
            sponsor: Some("sen_014".to_string()),
            cosponsors: vec!["sen_021".to_string(), "sen_044".to_string()],
            origin_chamber: Chamber::Senate,
            introduced_date: NaiveDate::from_ymd_opt(2026, 2, 11).unwrap(),
            current_version_label: Some("Reported Substitute".to_string()),
            budgetary_impact: BudgetaryImpact::Moderate,
            salience: 0.78,
            controversy: 0.63,
        }
    }

    #[test]
    fn validates_legislative_object() {
        assert!(example_legislative_object().validate().is_ok());
    }

    #[test]
    fn rejects_out_of_range_salience() {
        let mut legislative_object = example_legislative_object();
        legislative_object.salience = 1.2;

        let error = legislative_object.validate().unwrap_err();
        assert!(error.to_string().contains("legislative_object.salience"));
    }
}
