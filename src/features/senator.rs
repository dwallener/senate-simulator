use std::{collections::BTreeSet, fs, path::Path};

use chrono::NaiveDate;

use crate::{
    error::SenateSimError,
    features::{
        domains::domain_score,
        procedure::{amendment_votes, cloture_votes, motion_to_proceed_votes, support_rate},
        votes::{group_votes_by_roll_call, party_majority_position, valid_participation},
        windows::{FeatureWindowConfig, filter_votes_for_window},
    },
    model::{
        data_snapshot::DataSnapshot,
        identity::Party,
        legislative::PolicyDomain,
        normalized_records::{NormalizedVoteRecord, VotePosition},
        senator_feature_record::{FeatureReport, SenatorFeatureRecord},
    },
};

pub fn build_senator_features_for_snapshot(
    snapshot: &DataSnapshot,
    votes: &[NormalizedVoteRecord],
    config: &FeatureWindowConfig,
) -> Result<Vec<SenatorFeatureRecord>, SenateSimError> {
    snapshot.validate()?;

    let baseline_votes = filter_votes_for_window(votes, snapshot.snapshot_date, config.baseline_lookback_days);
    let recent_votes = filter_votes_for_window(votes, snapshot.snapshot_date, Some(config.recent_lookback_days));

    let mut records = Vec::with_capacity(snapshot.roster_records.len());
    for senator in &snapshot.roster_records {
        let senator_baseline = baseline_votes
            .iter()
            .copied()
            .filter(|vote| vote.senator_id == senator.senator_id)
            .collect::<Vec<_>>();
        let senator_recent = recent_votes
            .iter()
            .copied()
            .filter(|vote| vote.senator_id == senator.senator_id)
            .collect::<Vec<_>>();

        let party_loyalty_baseline =
            party_loyalty(&baseline_votes, senator.senator_id.as_str(), &senator.party)
                .unwrap_or(party_loyalty_fallback(&senator.party));
        let recent_party_loyalty =
            party_loyalty(&recent_votes, senator.senator_id.as_str(), &senator.party)
                .unwrap_or(party_loyalty_baseline);

        let bipartisanship_baseline =
            bipartisanship(&baseline_votes, senator.senator_id.as_str(), &senator.party)
                .unwrap_or(bipartisanship_fallback(&senator.party));
        let recent_bipartisanship =
            bipartisanship(&recent_votes, senator.senator_id.as_str(), &senator.party)
                .unwrap_or(bipartisanship_baseline);

        let attendance_reliability = attendance(&senator_baseline)
            .unwrap_or(0.95);
        let recent_attendance_reliability = attendance(&senator_recent)
            .unwrap_or(attendance_reliability);

        let cloture_support_baseline = support_rate(&cloture_votes(&senator_baseline))
            .unwrap_or(procedural_fallback(&senator.party, "cloture"));
        let recent_cloture_support = support_rate(&cloture_votes(&senator_recent))
            .unwrap_or(cloture_support_baseline);
        let motion_to_proceed_baseline = support_rate(&motion_to_proceed_votes(&senator_baseline))
            .unwrap_or(procedural_fallback(&senator.party, "motion"));
        let amendment_openness = support_rate(&amendment_votes(&senator_baseline))
            .unwrap_or(procedural_fallback(&senator.party, "amendment"));

        let procedural_rigidity = clamp_unit(
            party_loyalty_baseline * 0.40
                + (1.0 - bipartisanship_baseline) * 0.25
                + (1.0 - amendment_openness) * 0.20
                + (recent_party_loyalty * 0.15),
        );

        let (defense_score, defense_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::Defense, &senator.party);
        let (budget_tax_score, budget_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::BudgetTax, &senator.party);
        let (healthcare_score, healthcare_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::Healthcare, &senator.party);
        let (immigration_score, immigration_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::Immigration, &senator.party);
        let (energy_climate_score, energy_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::EnergyClimate, &senator.party);
        let (judiciary_score, judiciary_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::Judiciary, &senator.party);
        let (technology_score, technology_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::Technology, &senator.party);
        let (foreign_policy_score, foreign_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::ForeignPolicy, &senator.party);
        let (labor_score, labor_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::Labor, &senator.party);
        let (education_score, education_coverage) =
            domain_score(&senator_baseline, &PolicyDomain::Education, &senator.party);

        let ideology_proxy = ideology_proxy(
            &senator.party,
            party_loyalty_baseline,
            bipartisanship_baseline,
            budget_tax_score,
            labor_score,
            judiciary_score,
            energy_climate_score,
        );
        let historical_vote_count = senator_baseline.len();
        let recent_vote_count = senator_recent.len();
        let coverage_score = clamp_unit(historical_vote_count as f32 / 12.0);
        let mut notes = Vec::new();
        if historical_vote_count < 5 {
            notes.push("limited historical vote coverage; using party-level fallbacks".to_string());
        }
        if cloture_votes(&senator_baseline).is_empty() {
            notes.push("limited procedural vote history; cloture support uses fallback".to_string());
        }
        if BTreeSet::from([
            defense_coverage,
            budget_coverage,
            healthcare_coverage,
            immigration_coverage,
            energy_coverage,
            judiciary_coverage,
            technology_coverage,
            foreign_coverage,
            labor_coverage,
            education_coverage,
        ])
        .into_iter()
        .filter(|count| *count == 0)
        .count()
            > 5
        {
            notes.push("multiple issue-domain scores use party fallback due to sparse coverage".to_string());
        }

        let record = SenatorFeatureRecord {
            snapshot_date: snapshot.snapshot_date,
            senator_id: senator.senator_id.clone(),
            full_name: senator.full_name.clone(),
            party: senator.party.clone(),
            state: senator.state.clone(),
            party_loyalty_baseline,
            bipartisanship_baseline,
            attendance_reliability,
            ideology_proxy,
            cloture_support_baseline,
            motion_to_proceed_baseline,
            amendment_openness,
            procedural_rigidity,
            defense_score,
            budget_tax_score,
            healthcare_score,
            immigration_score,
            energy_climate_score,
            judiciary_score,
            technology_score,
            foreign_policy_score,
            labor_score,
            education_score,
            recent_party_loyalty,
            recent_bipartisanship,
            recent_cloture_support,
            recent_attendance_reliability,
            historical_vote_count,
            recent_vote_count,
            coverage_score,
            notes,
        };
        record.validate()?;
        records.push(record);
    }

    Ok(records)
}

pub fn build_feature_report(
    snapshot_date: NaiveDate,
    features: &[SenatorFeatureRecord],
) -> FeatureReport {
    let senators_processed = features.len();
    let senators_with_sparse_history = features
        .iter()
        .filter(|feature| feature.historical_vote_count < 5)
        .count();
    let average_coverage_score = if features.is_empty() {
        0.0
    } else {
        features
            .iter()
            .map(|feature| feature.coverage_score)
            .sum::<f32>()
            / features.len() as f32
    };

    FeatureReport {
        snapshot_date,
        senators_processed,
        senators_with_sparse_history,
        average_coverage_score,
        notes: vec![
            "feature records use only votes on or before the snapshot date".to_string(),
            "sparse-history senators fall back to party baselines with explicit notes".to_string(),
        ],
    }
}

pub fn persist_feature_artifacts(
    data_root: &Path,
    snapshot_date: NaiveDate,
    features: &[SenatorFeatureRecord],
    report: &FeatureReport,
) -> Result<(), SenateSimError> {
    let dir = feature_storage_dir(data_root, snapshot_date);
    write_json(&dir.join("senator_features.json"), features)?;
    write_json(&dir.join("feature_report.json"), report)?;
    Ok(())
}

pub fn load_feature_records(
    data_root: &Path,
    snapshot_date: NaiveDate,
) -> Result<Vec<SenatorFeatureRecord>, SenateSimError> {
    read_json(&feature_storage_dir(data_root, snapshot_date).join("senator_features.json"))
}

pub fn load_feature_report(
    data_root: &Path,
    snapshot_date: NaiveDate,
) -> Result<FeatureReport, SenateSimError> {
    read_json(&feature_storage_dir(data_root, snapshot_date).join("feature_report.json"))
}

pub fn feature_storage_dir(data_root: &Path, snapshot_date: NaiveDate) -> std::path::PathBuf {
    data_root.join("features").join(snapshot_date.to_string())
}

fn party_loyalty(
    all_votes: &[&NormalizedVoteRecord],
    senator_id: &str,
    party: &Party,
) -> Option<f32> {
    let grouped = group_votes_by_roll_call(all_votes);
    let mut matches = 0usize;
    let mut total = 0usize;

    for votes in grouped.values() {
        let Some(senator_vote) = votes.iter().copied().find(|vote| vote.senator_id == senator_id) else {
            continue;
        };
        let Some(majority) = party_majority_position(votes, party) else {
            continue;
        };
        if !matches!(senator_vote.vote_position, VotePosition::Yea | VotePosition::Nay) {
            continue;
        }
        total += 1;
        if senator_vote.vote_position == majority {
            matches += 1;
        }
    }

    ratio(matches, total)
}

fn bipartisanship(
    all_votes: &[&NormalizedVoteRecord],
    senator_id: &str,
    party: &Party,
) -> Option<f32> {
    let grouped = group_votes_by_roll_call(all_votes);
    let opposite_party = match party {
        Party::Democrat => Party::Republican,
        Party::Republican => Party::Democrat,
        _ => return None,
    };

    let mut cross_matches = 0usize;
    let mut total = 0usize;
    for votes in grouped.values() {
        let Some(senator_vote) = votes.iter().copied().find(|vote| vote.senator_id == senator_id) else {
            continue;
        };
        let Some(own_majority) = party_majority_position(votes, party) else {
            continue;
        };
        let Some(opposite_majority) = party_majority_position(votes, &opposite_party) else {
            continue;
        };
        if own_majority == opposite_majority {
            continue;
        }
        if !matches!(senator_vote.vote_position, VotePosition::Yea | VotePosition::Nay) {
            continue;
        }
        total += 1;
        if senator_vote.vote_position == opposite_majority {
            cross_matches += 1;
        }
    }

    ratio(cross_matches, total)
}

fn attendance(votes: &[&NormalizedVoteRecord]) -> Option<f32> {
    let total = votes.len();
    if total == 0 {
        return None;
    }
    let participated = votes
        .iter()
        .filter(|vote| valid_participation(vote))
        .count();
    Some(participated as f32 / total as f32)
}

fn ideology_proxy(
    party: &Party,
    loyalty: f32,
    bipartisanship: f32,
    budget: f32,
    labor: f32,
    judiciary: f32,
    energy: f32,
) -> f32 {
    let anchor = match party {
        Party::Democrat => -0.45,
        Party::Republican => 0.45,
        _ => 0.0,
    };
    let discipline = match party {
        Party::Democrat => -((loyalty - 0.5) * 0.7),
        Party::Republican => (loyalty - 0.5) * 0.7,
        _ => 0.0,
    };
    let crossover = match party {
        Party::Democrat => bipartisanship * 0.2,
        Party::Republican => -bipartisanship * 0.2,
        _ => 0.0,
    };
    let issue_mix = (budget + judiciary - labor - energy) * 0.15;
    (anchor + discipline + crossover + issue_mix).clamp(-1.0, 1.0)
}

fn procedural_fallback(party: &Party, kind: &str) -> f32 {
    match (party, kind) {
        (Party::Democrat, "cloture") => 0.68,
        (Party::Republican, "cloture") => 0.42,
        (Party::Democrat, "motion") => 0.74,
        (Party::Republican, "motion") => 0.55,
        (Party::Democrat, "amendment") => 0.62,
        (Party::Republican, "amendment") => 0.48,
        (_, _) => 0.50,
    }
}

fn party_loyalty_fallback(party: &Party) -> f32 {
    match party {
        Party::Democrat | Party::Republican => 0.82,
        _ => 0.55,
    }
}

fn bipartisanship_fallback(party: &Party) -> f32 {
    match party {
        Party::Democrat | Party::Republican => 0.22,
        _ => 0.45,
    }
}

fn ratio(numerator: usize, denominator: usize) -> Option<f32> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f32 / denominator as f32)
    }
}

fn clamp_unit(value: f32) -> f32 {
    if !value.is_finite() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn write_json<T: serde::Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), SenateSimError> {
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

fn read_json<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Result<T, SenateSimError> {
    let contents = fs::read_to_string(path).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| SenateSimError::Json {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};

    use crate::{
        features::{
            materialize::feature_record_to_senator,
            procedure::{cloture_votes, support_rate},
            windows::FeatureWindowConfig,
        },
        model::{
            data_snapshot::{DataSnapshot, SourceManifest},
            identity::Party,
            legislative::{BudgetaryImpact, LegislativeObjectType, PolicyDomain},
            legislative_context::{Chamber, ProceduralStage},
            normalized_records::{
                NormalizedLegislativeRecord, NormalizedSenatorRecord, NormalizedVoteRecord,
                ProceduralKind, VoteCategory, VotePosition,
            },
        },
        SenateClass,
    };

    use super::{
        build_feature_report, build_senator_features_for_snapshot, load_feature_records,
        persist_feature_artifacts,
    };

    fn snapshot() -> DataSnapshot {
        DataSnapshot {
            snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            run_id: "snapshot-20260309".to_string(),
            created_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
            roster_records: vec![
                NormalizedSenatorRecord {
                    senator_id: "sen_a".to_string(),
                    full_name: "Sen A".to_string(),
                    party: Party::Democrat,
                    state: "CA".to_string(),
                    class: SenateClass::I,
                    start_date: NaiveDate::from_ymd_opt(2023, 1, 3).unwrap(),
                    end_date: None,
                    source_member_id: "a".to_string(),
                    as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                },
                NormalizedSenatorRecord {
                    senator_id: "sen_b".to_string(),
                    full_name: "Sen B".to_string(),
                    party: Party::Republican,
                    state: "TX".to_string(),
                    class: SenateClass::II,
                    start_date: NaiveDate::from_ymd_opt(2021, 1, 3).unwrap(),
                    end_date: None,
                    source_member_id: "b".to_string(),
                    as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                },
            ],
            legislative_records: vec![NormalizedLegislativeRecord {
                object_id: "obj".to_string(),
                title: "Obj".to_string(),
                summary: "Summary".to_string(),
                object_type: LegislativeObjectType::Bill,
                policy_domain: PolicyDomain::EnergyClimate,
                sponsor: None,
                introduced_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                latest_status_text: Some("Debate".to_string()),
                current_stage: ProceduralStage::Debate,
                origin_chamber: Chamber::Senate,
                budgetary_impact: BudgetaryImpact::Moderate,
                salience: 0.5,
                controversy: 0.5,
                as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            }],
            action_records: vec![],
            vote_records: vec![],
            public_signal_records: vec![],
            public_signal_summary: None,
            source_manifests: vec![SourceManifest {
                source_name: "test".to_string(),
                fetched_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
                as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                source_identifier: "test".to_string(),
                content_hash: "abc".to_string(),
                record_count: 1,
            }],
        }
    }

    fn vote(
        vote_id: &str,
        date: (i32, u32, u32),
        senator_id: &str,
        party: Party,
        category: VoteCategory,
        position: VotePosition,
        domain: Option<PolicyDomain>,
        procedural_kind: Option<ProceduralKind>,
    ) -> NormalizedVoteRecord {
        NormalizedVoteRecord {
            vote_id: vote_id.to_string(),
            vote_date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            senator_id: senator_id.to_string(),
            senator_name: senator_id.to_string(),
            object_id: Some("obj".to_string()),
            vote_category: category,
            vote_position: position,
            party_at_time: party,
            policy_domain: domain,
            is_procedural: procedural_kind.is_some(),
            procedural_kind,
            as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
        }
    }

    #[test]
    fn party_loyalty_and_bipartisanship_are_derived() {
        let votes = vec![
            vote("v1", (2026, 1, 1), "sen_a", Party::Democrat, VoteCategory::Passage, VotePosition::Yea, None, None),
            vote("v1", (2026, 1, 1), "sen_d2", Party::Democrat, VoteCategory::Passage, VotePosition::Yea, None, None),
            vote("v1", (2026, 1, 1), "sen_d3", Party::Democrat, VoteCategory::Passage, VotePosition::Yea, None, None),
            vote("v1", (2026, 1, 1), "sen_b", Party::Republican, VoteCategory::Passage, VotePosition::Nay, None, None),
            vote("v1", (2026, 1, 1), "sen_r2", Party::Republican, VoteCategory::Passage, VotePosition::Nay, None, None),
            vote("v2", (2026, 1, 2), "sen_a", Party::Democrat, VoteCategory::Passage, VotePosition::Nay, None, None),
            vote("v2", (2026, 1, 2), "sen_d2", Party::Democrat, VoteCategory::Passage, VotePosition::Yea, None, None),
            vote("v2", (2026, 1, 2), "sen_d3", Party::Democrat, VoteCategory::Passage, VotePosition::Yea, None, None),
            vote("v2", (2026, 1, 2), "sen_b", Party::Republican, VoteCategory::Passage, VotePosition::Nay, None, None),
            vote("v2", (2026, 1, 2), "sen_r2", Party::Republican, VoteCategory::Passage, VotePosition::Nay, None, None),
        ];
        let features = build_senator_features_for_snapshot(&snapshot(), &votes, &FeatureWindowConfig::default()).unwrap();
        let sen_a = features.iter().find(|record| record.senator_id == "sen_a").unwrap();
        assert!(sen_a.party_loyalty_baseline < 1.0);
        assert!(sen_a.bipartisanship_baseline > 0.0);
    }

    #[test]
    fn attendance_and_cloture_support_are_windowed() {
        let votes = vec![
            vote("c1", (2025, 2, 1), "sen_a", Party::Democrat, VoteCategory::Cloture, VotePosition::Nay, None, Some(ProceduralKind::Cloture)),
            vote("c2", (2026, 2, 1), "sen_a", Party::Democrat, VoteCategory::Cloture, VotePosition::Yea, None, Some(ProceduralKind::Cloture)),
            vote("m1", (2026, 2, 2), "sen_a", Party::Democrat, VoteCategory::MotionToProceed, VotePosition::NotVoting, None, Some(ProceduralKind::MotionToProceed)),
        ];
        let features = build_senator_features_for_snapshot(
            &snapshot(),
            &votes,
            &FeatureWindowConfig {
                baseline_lookback_days: None,
                recent_lookback_days: 365,
            },
        )
        .unwrap();
        let sen_a = features.iter().find(|record| record.senator_id == "sen_a").unwrap();
        assert!(sen_a.attendance_reliability < 1.0);
        assert!(sen_a.recent_cloture_support > sen_a.cloture_support_baseline);
    }

    #[test]
    fn votes_after_snapshot_date_are_excluded() {
        let votes = vec![
            vote("v1", (2026, 3, 8), "sen_a", Party::Democrat, VoteCategory::Cloture, VotePosition::Yea, None, Some(ProceduralKind::Cloture)),
            vote("v2", (2026, 3, 10), "sen_a", Party::Democrat, VoteCategory::Cloture, VotePosition::Nay, None, Some(ProceduralKind::Cloture)),
        ];
        let features = build_senator_features_for_snapshot(&snapshot(), &votes, &FeatureWindowConfig::default()).unwrap();
        let sen_a = features.iter().find(|record| record.senator_id == "sen_a").unwrap();
        assert_eq!(sen_a.historical_vote_count, 1);
        assert_eq!(sen_a.cloture_support_baseline, 1.0);
    }

    #[test]
    fn sparse_history_emits_fallback_note() {
        let features = build_senator_features_for_snapshot(&snapshot(), &[], &FeatureWindowConfig::default()).unwrap();
        let sen_a = features.iter().find(|record| record.senator_id == "sen_a").unwrap();
        assert!(sen_a.notes.iter().any(|note| note.contains("limited historical vote coverage")));
    }

    #[test]
    fn feature_to_senator_materialization_is_valid() {
        let features = build_senator_features_for_snapshot(&snapshot(), &[], &FeatureWindowConfig::default()).unwrap();
        let senator = feature_record_to_senator(&features[0]).unwrap();
        assert!(senator.validate().is_ok());
    }

    #[test]
    fn feature_artifacts_persist_to_dated_folder() {
        let features = build_senator_features_for_snapshot(&snapshot(), &[], &FeatureWindowConfig::default()).unwrap();
        let report = build_feature_report(NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(), &features);
        let temp_dir = std::env::temp_dir().join("senate_sim_feature_artifacts");
        let _ = std::fs::remove_dir_all(&temp_dir);
        persist_feature_artifacts(&temp_dir, NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(), &features, &report).unwrap();
        let loaded = load_feature_records(&temp_dir, NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()).unwrap();
        assert_eq!(loaded.len(), features.len());
        assert!(temp_dir.join("features/2026-03-09/senator_features.json").exists());
    }

    #[test]
    fn cloture_support_uses_only_cloture_votes() {
        let votes = vec![
            vote("c1", (2026, 1, 1), "sen_a", Party::Democrat, VoteCategory::Cloture, VotePosition::Yea, None, Some(ProceduralKind::Cloture)),
            vote("a1", (2026, 1, 2), "sen_a", Party::Democrat, VoteCategory::Amendment, VotePosition::Nay, None, Some(ProceduralKind::AmendmentProcess)),
        ];
        let senator_votes = votes.iter().collect::<Vec<_>>();
        assert_eq!(support_rate(&cloture_votes(&senator_votes)).unwrap(), 1.0);
    }
}
