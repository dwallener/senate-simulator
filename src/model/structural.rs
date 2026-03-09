use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Structural {
    pub ideology_score: f32,
    pub party_loyalty_baseline: f32,
    pub bipartisanship_baseline: f32,
    pub committee_assignments: Vec<String>,
    pub reelection_year: Option<i32>,
    pub electoral_vulnerability: f32,
}
