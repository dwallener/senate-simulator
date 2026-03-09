use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DynamicState {
    pub current_public_position: PublicPosition,
    pub current_substantive_support: f32,
    pub current_procedural_support: f32,
    pub current_negotiability: f32,
    pub current_party_pressure: f32,
    pub current_issue_salience_in_state: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicPosition {
    Support,
    Oppose,
    Undeclared,
    Negotiating,
    Mixed,
}

impl fmt::Display for PublicPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Support => "support",
            Self::Oppose => "oppose",
            Self::Undeclared => "undeclared",
            Self::Negotiating => "negotiating",
            Self::Mixed => "mixed",
        };
        f.write_str(value)
    }
}
