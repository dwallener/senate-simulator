use std::fmt;

use chrono::NaiveDate;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub senator_id: String,
    pub full_name: String,
    pub party: Party,
    pub state: String,
    pub class: SenateClass,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Party {
    Democrat,
    Republican,
    Independent,
    Other(String),
}

impl Party {
    fn as_str(&self) -> &str {
        match self {
            Self::Democrat => "Democrat",
            Self::Republican => "Republican",
            Self::Independent => "Independent",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl fmt::Display for Party {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Party {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Party {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let normalized = value.trim();

        if normalized.eq_ignore_ascii_case("democrat") {
            Ok(Self::Democrat)
        } else if normalized.eq_ignore_ascii_case("republican") {
            Ok(Self::Republican)
        } else if normalized.eq_ignore_ascii_case("independent") {
            Ok(Self::Independent)
        } else {
            Ok(Self::Other(normalized.to_string()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SenateClass {
    #[serde(rename = "I")]
    I,
    #[serde(rename = "II")]
    II,
    #[serde(rename = "III")]
    III,
}

impl fmt::Display for SenateClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::I => "I",
            Self::II => "II",
            Self::III => "III",
        };
        f.write_str(value)
    }
}
