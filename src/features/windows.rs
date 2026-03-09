use chrono::NaiveDate;

use crate::model::normalized_records::NormalizedVoteRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureWindowConfig {
    pub baseline_lookback_days: Option<i64>,
    pub recent_lookback_days: i64,
}

impl Default for FeatureWindowConfig {
    fn default() -> Self {
        Self {
            baseline_lookback_days: Some(365 * 4),
            recent_lookback_days: 365,
        }
    }
}

pub fn filter_votes_for_window<'a>(
    votes: &'a [NormalizedVoteRecord],
    snapshot_date: NaiveDate,
    lookback_days: Option<i64>,
) -> Vec<&'a NormalizedVoteRecord> {
    votes.iter()
        .filter(|vote| vote.vote_date <= snapshot_date)
        .filter(|vote| {
            lookback_days
                .map(|days| (snapshot_date - vote.vote_date).num_days() <= days)
                .unwrap_or(true)
        })
        .collect()
}
