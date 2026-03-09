use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::legislative_context::Chamber;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawRosterRecord {
    pub source_member_id: String,
    pub name: String,
    pub party: String,
    pub state: String,
    pub chamber: Chamber,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub as_of_date: NaiveDate,
    pub source_name: String,
    pub source_identifier: String,
    pub source_url: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub raw_payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawLegislativeRecord {
    pub source_object_id: String,
    pub title: String,
    pub summary: Option<String>,
    pub introduced_date: Option<NaiveDate>,
    pub sponsor: Option<String>,
    pub latest_status_text: Option<String>,
    pub as_of_date: NaiveDate,
    pub source_name: String,
    pub source_identifier: String,
    pub source_url: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub raw_payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawActionRecord {
    pub source_action_id: String,
    pub object_id: String,
    pub action_date: NaiveDate,
    pub action_text: String,
    pub action_type: Option<String>,
    pub chamber: Chamber,
    pub as_of_date: NaiveDate,
    pub source_name: String,
    pub source_identifier: String,
    pub source_url: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub raw_payload: Value,
}
