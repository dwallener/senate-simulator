use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::Serialize;

use crate::{
    error::SenateSimError,
    model::{
        data_snapshot::{DataSnapshot, SourceManifest},
        dynamic_state::{DynamicState, PublicPosition},
        identity::Identity,
        issue_preferences::IssuePreferences,
        legislative::LegislativeObject,
        legislative_context::LegislativeContext,
        normalized_records::{
            NormalizedActionRecord, NormalizedLegislativeRecord, NormalizedSenatorRecord,
        },
        procedural::Procedural,
        senator::Senator,
        structural::Structural,
    },
};

use super::{
    normalize::default_context_for_stage,
    sources::{content_hash_string, fetched_at_for},
};

pub fn build_snapshot(
    run_date: chrono::NaiveDate,
    raw_roster: &[crate::model::raw_records::RawRosterRecord],
    raw_legislation: &[crate::model::raw_records::RawLegislativeRecord],
    raw_actions: &[crate::model::raw_records::RawActionRecord],
    roster_records: Vec<NormalizedSenatorRecord>,
    legislative_records: Vec<NormalizedLegislativeRecord>,
    action_records: Vec<NormalizedActionRecord>,
) -> Result<DataSnapshot, SenateSimError> {
    let manifests = vec![
        manifest_for("roster", run_date, raw_roster)?,
        manifest_for("legislation", run_date, raw_legislation)?,
        manifest_for("actions", run_date, raw_actions)?,
    ];

    let snapshot = DataSnapshot {
        snapshot_date: run_date,
        run_id: format!("snapshot-{}", run_date.format("%Y%m%d")),
        created_at: Utc::now(),
        roster_records,
        legislative_records,
        action_records,
        source_manifests: manifests,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

pub fn persist_normalized_records(
    data_root: &Path,
    run_date: chrono::NaiveDate,
    roster_records: &[NormalizedSenatorRecord],
    legislative_records: &[NormalizedLegislativeRecord],
    action_records: &[NormalizedActionRecord],
) -> Result<(), SenateSimError> {
    let base = normalized_storage_dir(data_root, run_date);
    write_json_file(&base.join("senators.json"), roster_records)?;
    write_json_file(&base.join("legislation.json"), legislative_records)?;
    write_json_file(&base.join("actions.json"), action_records)?;
    Ok(())
}

pub fn persist_snapshot(data_root: &Path, snapshot: &DataSnapshot) -> Result<(), SenateSimError> {
    let dir = snapshot_storage_dir(data_root, snapshot.snapshot_date);
    write_json_file(&dir.join("snapshot.json"), snapshot)
}

pub fn load_snapshot(
    data_root: &Path,
    snapshot_date: chrono::NaiveDate,
) -> Result<DataSnapshot, SenateSimError> {
    let path = snapshot_storage_dir(data_root, snapshot_date).join("snapshot.json");
    let contents = fs::read_to_string(&path).map_err(|source| SenateSimError::Io {
        path: path.clone(),
        source,
    })?;
    let snapshot: DataSnapshot =
        serde_json::from_str(&contents).map_err(|source| SenateSimError::Json {
            path: path.clone(),
            source,
        })?;
    snapshot.validate()?;
    Ok(snapshot)
}

pub fn snapshot_storage_dir(data_root: &Path, snapshot_date: chrono::NaiveDate) -> PathBuf {
    data_root.join("snapshots").join(snapshot_date.to_string())
}

pub fn normalized_storage_dir(data_root: &Path, snapshot_date: chrono::NaiveDate) -> PathBuf {
    data_root.join("normalized").join(snapshot_date.to_string())
}

pub fn snapshot_to_senators(snapshot: &DataSnapshot) -> Result<Vec<Senator>, SenateSimError> {
    snapshot
        .roster_records
        .iter()
        .enumerate()
        .map(|(index, record)| materialize_senator(record, index))
        .collect()
}

pub fn snapshot_to_legislative_objects(
    snapshot: &DataSnapshot,
) -> Result<Vec<LegislativeObject>, SenateSimError> {
    let mut objects = Vec::with_capacity(snapshot.legislative_records.len());
    for record in &snapshot.legislative_records {
        let object = LegislativeObject {
            object_id: record.object_id.clone(),
            title: record.title.clone(),
            object_type: record.object_type.clone(),
            policy_domain: record.policy_domain.clone(),
            summary: record.summary.clone(),
            text_embedding_placeholder: None,
            sponsor: record.sponsor.clone(),
            cosponsors: Vec::new(),
            origin_chamber: record.origin_chamber,
            introduced_date: record.introduced_date,
            current_version_label: record.latest_status_text.clone(),
            budgetary_impact: record.budgetary_impact.clone(),
            salience: record.salience,
            controversy: record.controversy,
        };
        object.validate()?;
        objects.push(object);
    }
    Ok(objects)
}

pub fn snapshot_to_contexts(
    snapshot: &DataSnapshot,
) -> Result<Vec<LegislativeContext>, SenateSimError> {
    let mut contexts = Vec::with_capacity(snapshot.legislative_records.len());
    for record in &snapshot.legislative_records {
        let context =
            default_context_for_stage(snapshot.snapshot_date, record.current_stage.clone());
        context.validate()?;
        contexts.push(context);
    }
    Ok(contexts)
}

fn materialize_senator(
    record: &NormalizedSenatorRecord,
    index: usize,
) -> Result<Senator, SenateSimError> {
    let ideology = signed_from_hash(&record.senator_id, 0);
    let tax = signed_from_hash(&record.senator_id, 1);
    let energy = signed_from_hash(&record.senator_id, 2);
    let immigration = signed_from_hash(&record.senator_id, 3);
    let judiciary = signed_from_hash(&record.senator_id, 4);
    let senator = Senator {
        identity: Identity {
            senator_id: record.senator_id.clone(),
            full_name: record.full_name.clone(),
            party: record.party.clone(),
            state: record.state.clone(),
            class: record.class,
            start_date: record.start_date,
            end_date: record.end_date,
        },
        structural: Structural {
            ideology_score: party_adjusted_ideology(&record.party, ideology),
            party_loyalty_baseline: probability_from_hash(&record.senator_id, 5, 0.58, 0.94),
            bipartisanship_baseline: probability_from_hash(&record.senator_id, 6, 0.18, 0.74),
            committee_assignments: vec![format!("Synthetic Committee {}", (index % 8) + 1)],
            reelection_year: Some(2026 + ((index % 3) as i32 * 2)),
            electoral_vulnerability: probability_from_hash(&record.senator_id, 7, 0.12, 0.78),
        },
        issue_preferences: IssuePreferences {
            defense: party_issue_shift(
                &record.party,
                signed_from_hash(&record.senator_id, 8),
                0.15,
                -0.15,
            ),
            immigration: party_issue_shift(&record.party, immigration, -0.10, 0.20),
            energy_climate: party_issue_shift(&record.party, energy, 0.25, -0.25),
            labor: party_issue_shift(
                &record.party,
                signed_from_hash(&record.senator_id, 9),
                0.20,
                -0.10,
            ),
            healthcare: party_issue_shift(
                &record.party,
                signed_from_hash(&record.senator_id, 10),
                0.18,
                -0.12,
            ),
            tax_spending: party_issue_shift(&record.party, tax, -0.10, 0.20),
            judiciary: party_issue_shift(&record.party, judiciary, -0.05, 0.12),
            trade: signed_from_hash(&record.senator_id, 11),
            tech_privacy: signed_from_hash(&record.senator_id, 12),
            foreign_policy: signed_from_hash(&record.senator_id, 13),
        },
        procedural: Procedural {
            cloture_support_baseline: probability_from_hash(&record.senator_id, 14, 0.38, 0.88),
            motion_to_proceed_baseline: probability_from_hash(&record.senator_id, 15, 0.45, 0.92),
            uc_objection_tendency: probability_from_hash(&record.senator_id, 16, 0.05, 0.48),
            leadership_deference: probability_from_hash(&record.senator_id, 17, 0.25, 0.88),
            amendment_openness: probability_from_hash(&record.senator_id, 18, 0.28, 0.86),
            attendance_reliability: probability_from_hash(&record.senator_id, 19, 0.85, 0.99),
        },
        dynamic_state: DynamicState {
            current_public_position: PublicPosition::Undeclared,
            current_substantive_support: probability_from_hash(&record.senator_id, 20, 0.35, 0.65),
            current_procedural_support: probability_from_hash(&record.senator_id, 21, 0.35, 0.65),
            current_negotiability: probability_from_hash(&record.senator_id, 22, 0.22, 0.82),
            current_party_pressure: probability_from_hash(&record.senator_id, 23, 0.30, 0.88),
            current_issue_salience_in_state: probability_from_hash(
                &record.senator_id,
                24,
                0.25,
                0.80,
            ),
        },
    };
    senator.validate()?;
    Ok(senator)
}

fn manifest_for<T: Serialize>(
    source_name: &str,
    run_date: chrono::NaiveDate,
    records: &[T],
) -> Result<SourceManifest, SenateSimError> {
    let payload = serde_json::to_string(records).map_err(SenateSimError::Serialize)?;
    Ok(SourceManifest {
        source_name: source_name.to_string(),
        fetched_at: fetched_at_for(run_date)?,
        as_of_date: run_date,
        source_identifier: format!("{source_name}-{}", run_date.format("%Y%m%d")),
        content_hash: content_hash_string(&payload),
        record_count: records.len(),
    })
}

fn write_json_file<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), SenateSimError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SenateSimError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let contents = serde_json::to_string_pretty(value).map_err(SenateSimError::Serialize)?;
    fs::write(path, contents).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn probability_from_hash(id: &str, salt: u64, min: f32, max: f32) -> f32 {
    let base = hash_fraction(id, salt);
    min + (max - min) * base
}

fn signed_from_hash(id: &str, salt: u64) -> f32 {
    (hash_fraction(id, salt) * 2.0) - 1.0
}

fn hash_fraction(id: &str, salt: u64) -> f32 {
    let mut hash = 14695981039346656037u64 ^ salt;
    for byte in id.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    (hash % 10_000) as f32 / 10_000.0
}

fn party_adjusted_ideology(party: &crate::model::identity::Party, base: f32) -> f32 {
    match party {
        crate::model::identity::Party::Democrat => (base - 0.35).clamp(-1.0, 1.0),
        crate::model::identity::Party::Republican => (base + 0.35).clamp(-1.0, 1.0),
        crate::model::identity::Party::Independent => base.clamp(-1.0, 1.0),
        crate::model::identity::Party::Other(_) => base.clamp(-1.0, 1.0),
    }
}

fn party_issue_shift(
    party: &crate::model::identity::Party,
    base: f32,
    dem_shift: f32,
    rep_shift: f32,
) -> f32 {
    match party {
        crate::model::identity::Party::Democrat => (base + dem_shift).clamp(-1.0, 1.0),
        crate::model::identity::Party::Republican => (base + rep_shift).clamp(-1.0, 1.0),
        _ => base.clamp(-1.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use chrono::NaiveDate;

    use crate::ingest::{
        run_daily_ingestion_with_roots,
        snapshot::{load_snapshot, normalized_storage_dir, snapshot_storage_dir},
        snapshot_to_contexts, snapshot_to_legislative_objects, snapshot_to_senators,
    };

    fn test_roots() -> (std::path::PathBuf, &'static Path) {
        let temp_dir = std::env::temp_dir().join("senate_sim_ingest_snapshot_tests");
        let _ = std::fs::remove_dir_all(&temp_dir);
        (temp_dir, Path::new("fixtures/ingest"))
    }

    #[test]
    fn snapshot_creation_test() {
        let (temp_dir, fixture_root) = test_roots();
        let snapshot = run_daily_ingestion_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            fixture_root,
            &temp_dir,
        )
        .unwrap();
        assert_eq!(snapshot.roster_records.len(), 4);
        assert_eq!(snapshot.legislative_records.len(), 1);
        assert_eq!(snapshot.action_records.len(), 2);
    }

    #[test]
    fn daily_snapshot_repeatability_test() {
        let (temp_dir, fixture_root) = test_roots();
        let first = run_daily_ingestion_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            fixture_root,
            &temp_dir,
        )
        .unwrap();
        let second = run_daily_ingestion_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            fixture_root,
            &temp_dir,
        )
        .unwrap();

        assert_eq!(first.roster_records, second.roster_records);
        assert_eq!(first.legislative_records, second.legislative_records);
        assert_eq!(first.action_records, second.action_records);
        assert_eq!(first.source_manifests, second.source_manifests);
    }

    #[test]
    fn roster_materialization_test() {
        let (temp_dir, fixture_root) = test_roots();
        let snapshot = run_daily_ingestion_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            fixture_root,
            &temp_dir,
        )
        .unwrap();
        let senators = snapshot_to_senators(&snapshot).unwrap();
        assert_eq!(senators.len(), 4);
        assert!(senators.iter().all(|senator| senator.validate().is_ok()));
    }

    #[test]
    fn legislative_materialization_test() {
        let (temp_dir, fixture_root) = test_roots();
        let snapshot = run_daily_ingestion_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            fixture_root,
            &temp_dir,
        )
        .unwrap();
        let objects = snapshot_to_legislative_objects(&snapshot).unwrap();
        let contexts = snapshot_to_contexts(&snapshot).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(contexts.len(), 1);
        assert!(objects[0].validate().is_ok());
        assert!(contexts[0].validate().is_ok());
    }

    #[test]
    fn snapshot_storage_path_test() {
        let base = Path::new("/tmp/senate-sim");
        let date = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
        assert_eq!(
            snapshot_storage_dir(base, date),
            Path::new("/tmp/senate-sim/snapshots/2026-03-09")
        );
        assert_eq!(
            normalized_storage_dir(base, date),
            Path::new("/tmp/senate-sim/normalized/2026-03-09")
        );
    }

    #[test]
    fn content_hash_manifest_test() {
        let (temp_dir, fixture_root) = test_roots();
        let snapshot = run_daily_ingestion_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            fixture_root,
            &temp_dir,
        )
        .unwrap();
        assert_eq!(snapshot.source_manifests.len(), 3);
        assert!(
            snapshot
                .source_manifests
                .iter()
                .all(|manifest| !manifest.content_hash.is_empty() && manifest.record_count > 0)
        );
    }

    #[test]
    fn end_to_end_ingestion_smoke_test() {
        let (temp_dir, fixture_root) = test_roots();
        let snapshot = run_daily_ingestion_with_roots(
            NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(),
            fixture_root,
            &temp_dir,
        )
        .unwrap();
        let loaded =
            load_snapshot(&temp_dir, NaiveDate::from_ymd_opt(2026, 3, 10).unwrap()).unwrap();
        assert_eq!(snapshot.legislative_records, loaded.legislative_records);
        assert_eq!(loaded.action_records.len(), 3);
    }
}
