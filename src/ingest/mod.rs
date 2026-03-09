pub mod actions;
pub mod legislation;
pub mod normalize;
pub mod roster;
pub mod snapshot;
pub mod sources;
pub mod votes;

use std::path::Path;

use chrono::NaiveDate;

use crate::{error::SenateSimError, model::data_snapshot::DataSnapshot};

pub use snapshot::{
    load_snapshot, snapshot_storage_dir, snapshot_to_contexts, snapshot_to_legislative_objects,
    snapshot_to_senators,
};

pub fn run_daily_ingestion(run_date: NaiveDate) -> Result<DataSnapshot, SenateSimError> {
    run_daily_ingestion_with_roots(run_date, Path::new("fixtures/ingest"), Path::new("data"))
}

pub fn run_daily_ingestion_with_roots(
    run_date: NaiveDate,
    fixture_root: &Path,
    data_root: &Path,
) -> Result<DataSnapshot, SenateSimError> {
    let raw_roster = roster::ingest_roster(run_date, fixture_root, data_root)?;
    let raw_legislation = legislation::ingest_legislation(run_date, fixture_root, data_root)?;
    let raw_actions = actions::ingest_actions(run_date, fixture_root, data_root)?;
    let raw_votes = votes::ingest_votes(run_date, fixture_root, data_root)?;

    let normalized_roster = normalize::normalize_roster(&raw_roster)?;
    let normalized_legislation = normalize::normalize_legislation(&raw_legislation)?;
    let normalized_actions = normalize::normalize_actions(&raw_actions)?;
    let normalized_votes = normalize::normalize_votes(&raw_votes)?;

    snapshot::persist_normalized_records(
        data_root,
        run_date,
        &normalized_roster,
        &normalized_legislation,
        &normalized_actions,
        &normalized_votes,
    )?;

    let data_snapshot = snapshot::build_snapshot(
        run_date,
        &raw_roster,
        &raw_legislation,
        &raw_actions,
        &raw_votes,
        normalized_roster,
        normalized_legislation,
        normalized_actions,
        normalized_votes,
    )?;
    snapshot::persist_snapshot(data_root, &data_snapshot)?;

    Ok(data_snapshot)
}
