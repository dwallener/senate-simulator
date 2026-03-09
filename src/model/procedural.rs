use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Procedural {
    pub cloture_support_baseline: f32,
    pub motion_to_proceed_baseline: f32,
    pub uc_objection_tendency: f32,
    pub leadership_deference: f32,
    pub amendment_openness: f32,
    pub attendance_reliability: f32,
}
