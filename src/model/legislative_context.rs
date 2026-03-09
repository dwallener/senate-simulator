use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{error::SenateSimError, model::identity::Party};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegislativeContext {
    pub congress_number: i32,
    pub session: CongressionalSession,
    pub current_chamber: Chamber,
    pub procedural_stage: ProceduralStage,
    pub majority_party: Party,
    pub minority_party: Party,
    pub president_party: Party,
    pub days_until_election: Option<i32>,
    pub days_until_deadline: Option<i32>,
    pub under_unanimous_consent: bool,
    pub under_reconciliation: bool,
    pub leadership_priority: f32,
    pub media_attention: f32,
}

impl LegislativeContext {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.congress_number <= 0 {
            return Err(SenateSimError::Validation {
                field: "legislative_context.congress_number",
                message: "must be positive".to_string(),
            });
        }

        if let Some(days) = self.days_until_election {
            validate_non_negative("legislative_context.days_until_election", days)?;
        }

        if let Some(days) = self.days_until_deadline {
            validate_non_negative("legislative_context.days_until_deadline", days)?;
        }

        validate_probability(
            "legislative_context.leadership_priority",
            self.leadership_priority,
        )?;
        validate_probability("legislative_context.media_attention", self.media_attention)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Chamber {
    Senate,
    House,
}

impl fmt::Display for Chamber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Senate => "Senate",
            Self::House => "House",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CongressionalSession {
    First,
    Second,
    Special,
}

impl fmt::Display for CongressionalSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::First => "First",
            Self::Second => "Second",
            Self::Special => "Special",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProceduralStage {
    Introduced,
    InCommittee,
    Reported,
    OnCalendar,
    MotionToProceed,
    Debate,
    AmendmentPending,
    ClotureFiled,
    ClotureVote,
    FinalPassage,
    Conference,
    Stalled,
    Other(String),
}

impl ProceduralStage {
    fn as_str(&self) -> &str {
        match self {
            Self::Introduced => "Introduced",
            Self::InCommittee => "InCommittee",
            Self::Reported => "Reported",
            Self::OnCalendar => "OnCalendar",
            Self::MotionToProceed => "MotionToProceed",
            Self::Debate => "Debate",
            Self::AmendmentPending => "AmendmentPending",
            Self::ClotureFiled => "ClotureFiled",
            Self::ClotureVote => "ClotureVote",
            Self::FinalPassage => "FinalPassage",
            Self::Conference => "Conference",
            Self::Stalled => "Stalled",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl fmt::Display for ProceduralStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ProceduralStage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProceduralStage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let normalized = value.trim();

        if normalized.eq_ignore_ascii_case("introduced") {
            Ok(Self::Introduced)
        } else if normalized.eq_ignore_ascii_case("incommittee")
            || normalized.eq_ignore_ascii_case("in_committee")
        {
            Ok(Self::InCommittee)
        } else if normalized.eq_ignore_ascii_case("reported") {
            Ok(Self::Reported)
        } else if normalized.eq_ignore_ascii_case("oncalendar")
            || normalized.eq_ignore_ascii_case("on_calendar")
        {
            Ok(Self::OnCalendar)
        } else if normalized.eq_ignore_ascii_case("motiontoproceed")
            || normalized.eq_ignore_ascii_case("motion_to_proceed")
        {
            Ok(Self::MotionToProceed)
        } else if normalized.eq_ignore_ascii_case("debate") {
            Ok(Self::Debate)
        } else if normalized.eq_ignore_ascii_case("amendmentpending")
            || normalized.eq_ignore_ascii_case("amendment_pending")
        {
            Ok(Self::AmendmentPending)
        } else if normalized.eq_ignore_ascii_case("cloturefiled")
            || normalized.eq_ignore_ascii_case("cloture_filed")
        {
            Ok(Self::ClotureFiled)
        } else if normalized.eq_ignore_ascii_case("cloturevote")
            || normalized.eq_ignore_ascii_case("cloture_vote")
        {
            Ok(Self::ClotureVote)
        } else if normalized.eq_ignore_ascii_case("finalpassage")
            || normalized.eq_ignore_ascii_case("final_passage")
        {
            Ok(Self::FinalPassage)
        } else if normalized.eq_ignore_ascii_case("conference") {
            Ok(Self::Conference)
        } else if normalized.eq_ignore_ascii_case("stalled") {
            Ok(Self::Stalled)
        } else {
            Ok(Self::Other(normalized.to_string()))
        }
    }
}

fn validate_non_negative(field: &'static str, value: i32) -> Result<(), SenateSimError> {
    if value < 0 {
        return Err(SenateSimError::Validation {
            field,
            message: "must be non-negative".to_string(),
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
