use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::model::{normalized_records::NormalizedActionCategory, senate_event::SenateEvent};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalTimeline {
    pub object_id: String,
    pub events: Vec<HistoricalActionEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalActionEvent {
    pub object_id: String,
    pub action_date: NaiveDate,
    pub raw_action_text: String,
    pub normalized_action_category: NormalizedActionCategory,
    pub aligned_senate_event: Option<SenateEvent>,
    pub is_consequential: bool,
    pub source_record_id: Option<String>,
}
