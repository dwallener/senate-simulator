pub mod actions;
pub mod config;
pub mod congress_api;
pub mod legislation;
pub mod live;
pub mod normalize;
pub mod roster;
pub mod senate_votes;
pub mod snapshot;
pub mod sources;
pub mod votes;

use std::path::Path;

use chrono::NaiveDate;

use crate::{error::SenateSimError, model::data_snapshot::DataSnapshot};

pub use config::{IngestionConfig, IngestionSourceMode};
pub use snapshot::{
    load_snapshot, snapshot_storage_dir, snapshot_to_contexts, snapshot_to_legislative_objects,
    snapshot_to_senators,
};

pub fn run_daily_ingestion(run_date: NaiveDate) -> Result<DataSnapshot, SenateSimError> {
    run_ingestion(&IngestionConfig::fixtures(run_date))
}

pub fn run_live_ingestion(config: &IngestionConfig) -> Result<DataSnapshot, SenateSimError> {
    let live_config = IngestionConfig {
        source_mode: IngestionSourceMode::Live,
        ..config.clone()
    };
    run_ingestion(&live_config)
}

pub fn run_ingestion(config: &IngestionConfig) -> Result<DataSnapshot, SenateSimError> {
    match config.source_mode {
        IngestionSourceMode::Fixtures => run_daily_ingestion_with_roots(
            config.run_date,
            &config.fixture_root,
            &config.output_root,
        ),
        IngestionSourceMode::Live => run_live_ingestion_with_roots(config),
    }
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

pub fn run_live_ingestion_with_roots(
    config: &IngestionConfig,
) -> Result<DataSnapshot, SenateSimError> {
    let records = live::ingest_live_records(config)?;
    let normalized_roster = normalize::normalize_roster(&records.roster)?;
    let normalized_legislation = normalize::normalize_legislation(&records.legislation)?;
    let normalized_actions = normalize::normalize_actions(&records.actions)?;
    let normalized_votes = normalize::normalize_votes(&records.votes)?;

    snapshot::persist_normalized_records(
        &config.output_root,
        config.run_date,
        &normalized_roster,
        &normalized_legislation,
        &normalized_actions,
        &normalized_votes,
    )?;

    let data_snapshot = snapshot::build_snapshot(
        config.run_date,
        &records.roster,
        &records.legislation,
        &records.actions,
        &records.votes,
        normalized_roster,
        normalized_legislation,
        normalized_actions,
        normalized_votes,
    )?;
    snapshot::persist_snapshot(&config.output_root, &data_snapshot)?;
    Ok(data_snapshot)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use chrono::NaiveDate;

    use crate::ingest::{
        IngestionConfig, IngestionSourceMode, run_daily_ingestion_with_roots, run_ingestion,
        sources::{raw_storage_dir, write_json_value, write_string},
    };

    #[test]
    fn config_routing_dispatches_fixture_mode() {
        let temp_dir = std::env::temp_dir().join("senate_sim_ingest_mode_dispatch");
        let _ = fs::remove_dir_all(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
        let mut config = IngestionConfig::fixtures(date);
        config.output_root = temp_dir.clone();

        let via_config = run_ingestion(&config).unwrap();
        let direct =
            run_daily_ingestion_with_roots(date, Path::new("fixtures/ingest"), &temp_dir).unwrap();
        assert_eq!(via_config.roster_records.len(), direct.roster_records.len());
        assert_eq!(
            via_config.legislative_records.len(),
            direct.legislative_records.len()
        );
    }

    #[test]
    fn cached_live_raw_reuse_builds_snapshot_without_fetch() {
        let temp_dir = std::env::temp_dir().join("senate_sim_ingest_live_cache");
        let _ = fs::remove_dir_all(&temp_dir);
        let date = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
        let raw_dir = raw_storage_dir(&temp_dir, date);
        fs::create_dir_all(&raw_dir).unwrap();

        write_json_value(
            &raw_dir.join("congress_members.json"),
            &serde_json::json!({
                "payload": {
                    "members": [
                        {
                            "bioguideId": "A0001",
                            "directOrderName": "Alex Adams",
                            "party": "D",
                            "state": "WA"
                        },
                        {
                            "bioguideId": "B0002",
                            "directOrderName": "Blair Baker",
                            "party": "R",
                            "state": "TX"
                        }
                    ]
                }
            }),
        )
        .unwrap();
        write_json_value(
            &raw_dir.join("congress_bills.json"),
            &serde_json::json!({
                "payload": {
                    "bills": [
                        {
                            "type": "s",
                            "number": "2100",
                            "title": "Clean Energy Permitting Reform Act",
                            "summary": "Permitting reform for transmission.",
                            "introducedDate": "2026-03-01",
                            "latestAction": { "text": "Cloture filed in Senate." }
                        }
                    ]
                }
            }),
        )
        .unwrap();
        write_json_value(
            &raw_dir.join("congress_actions_s_2100.json"),
            &serde_json::json!({
                "payload": {
                    "actions": [
                        {
                            "actionCode": "S001",
                            "billId": "s2100",
                            "actionDate": "2026-03-05",
                            "text": "Cloture filed in Senate.",
                            "type": "floor"
                        }
                    ]
                }
            }),
        )
        .unwrap();
        write_string(
            &raw_dir.join("senate_vote_index.xml"),
            r#"<roll_call_votes><vote><vote_number>1</vote_number><vote_date>January 15, 2026</vote_date></vote></roll_call_votes>"#,
        )
        .unwrap();
        write_string(
            &raw_dir.join("senate_vote_119_2_00001.xml"),
            r#"
            <roll_call_vote>
              <vote_date>January 15, 2026</vote_date>
              <vote_question>On the Cloture Motion S.2100</vote_question>
              <vote_title>On the Cloture Motion S.2100</vote_title>
              <document>
                <document_type>s</document_type>
                <document_number>2100</document_number>
              </document>
              <members>
                <member>
                  <lis_member_id>A0001</lis_member_id>
                  <first_name>Alex</first_name>
                  <last_name>Adams</last_name>
                  <state>WA</state>
                  <party>D</party>
                  <vote_cast>Yea</vote_cast>
                </member>
                <member>
                  <lis_member_id>B0002</lis_member_id>
                  <first_name>Blair</first_name>
                  <last_name>Baker</last_name>
                  <state>TX</state>
                  <party>R</party>
                  <vote_cast>Nay</vote_cast>
                </member>
              </members>
            </roll_call_vote>
            "#,
        )
        .unwrap();

        let config = IngestionConfig {
            run_date: date,
            source_mode: IngestionSourceMode::Live,
            congress_api_key: Some("dummy-key".to_string()),
            output_root: temp_dir.clone(),
            fixture_root: Path::new("fixtures/ingest").to_path_buf(),
            use_cached_raw_if_present: true,
        };
        let snapshot = run_ingestion(&config).unwrap();
        assert_eq!(snapshot.legislative_records.len(), 1);
        assert_eq!(snapshot.action_records.len(), 1);
        assert_eq!(snapshot.vote_records.len(), 2);
    }
}
