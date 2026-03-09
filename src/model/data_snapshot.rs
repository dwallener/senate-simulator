use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::normalized_records::{
        NormalizedActionRecord, NormalizedLegislativeRecord, NormalizedSenatorRecord,
        NormalizedVoteRecord,
    },
    model::normalized_public_signal_record::NormalizedPublicSignalRecord,
    model::public_signal_summary::PublicSignalSummary,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceManifest {
    pub source_name: String,
    pub fetched_at: DateTime<Utc>,
    pub as_of_date: NaiveDate,
    pub source_identifier: String,
    pub content_hash: String,
    pub record_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataSnapshot {
    pub snapshot_date: NaiveDate,
    pub run_id: String,
    pub created_at: DateTime<Utc>,
    pub roster_records: Vec<NormalizedSenatorRecord>,
    pub legislative_records: Vec<NormalizedLegislativeRecord>,
    pub action_records: Vec<NormalizedActionRecord>,
    pub vote_records: Vec<NormalizedVoteRecord>,
    #[serde(default)]
    pub public_signal_records: Vec<NormalizedPublicSignalRecord>,
    #[serde(default)]
    pub public_signal_summary: Option<PublicSignalSummary>,
    pub source_manifests: Vec<SourceManifest>,
}

impl DataSnapshot {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.run_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "data_snapshot.run_id",
                message: "must not be empty".to_string(),
            });
        }

        for record in &self.roster_records {
            record.validate()?;
        }
        for record in &self.legislative_records {
            record.validate()?;
        }
        for record in &self.action_records {
            record.validate()?;
        }
        for record in &self.vote_records {
            record.validate()?;
        }
        for record in &self.public_signal_records {
            record.validate()?;
        }
        if let Some(summary) = &self.public_signal_summary {
            summary.validate()?;
        }

        Ok(())
    }
}
