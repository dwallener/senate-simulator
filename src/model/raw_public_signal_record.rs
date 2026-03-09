use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::legislative::PolicyDomain;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicSignalScope {
    Senator,
    LegislativeObject,
    PolicyDomain,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPublicSignalRecord {
    pub signal_id: String,
    pub snapshot_date: NaiveDate,
    pub scope: PublicSignalScope,
    pub query: String,
    pub linked_senator_id: Option<String>,
    pub linked_object_id: Option<String>,
    pub policy_domain: Option<PolicyDomain>,
    pub source_name: String,
    pub source_identifier: String,
    pub source_url: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub raw_payload: Value,
}
