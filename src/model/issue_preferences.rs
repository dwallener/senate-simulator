use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IssuePreferences {
    pub defense: f32,
    pub immigration: f32,
    pub energy_climate: f32,
    pub labor: f32,
    pub healthcare: f32,
    pub tax_spending: f32,
    pub judiciary: f32,
    pub trade: f32,
    pub tech_privacy: f32,
    pub foreign_policy: f32,
}
