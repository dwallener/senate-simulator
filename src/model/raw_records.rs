use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::legislative_context::Chamber;
use crate::model::{identity::Party, legislative::PolicyDomain};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawVoteRecord {
    pub source_vote_id: String,
    pub vote_date: NaiveDate,
    pub senator_id: String,
    pub senator_name: String,
    pub object_id: Option<String>,
    pub vote_category: String,
    pub vote_position: String,
    pub party_at_time: Party,
    pub policy_domain: Option<PolicyDomain>,
    pub is_procedural: bool,
    pub procedural_kind: Option<String>,
    pub as_of_date: NaiveDate,
    pub source_name: String,
    pub source_identifier: String,
    pub source_url: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub raw_payload: Value,
}
