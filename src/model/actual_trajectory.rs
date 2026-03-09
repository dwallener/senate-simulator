use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::model::senate_event::SenateEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActualTrajectory {
    pub snapshot_date: NaiveDate,
    pub object_id: String,
    pub events: Vec<ActualTrajectoryEvent>,
    pub horizon_days: Option<i64>,
    pub max_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActualTrajectoryEvent {
    pub event: SenateEvent,
    pub event_date: NaiveDate,
    pub source_action_text: String,
}
