use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignmentReport {
    pub snapshot_date: NaiveDate,
    pub objects_processed: usize,
    pub examples_generated: usize,
    pub ambiguous_actions: usize,
    pub unaligned_consequential_actions: usize,
    pub notes: Vec<String>,
}
