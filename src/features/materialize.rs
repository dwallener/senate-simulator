use crate::{
    error::SenateSimError,
    features::senator::{
        build_feature_report, build_senator_features_for_snapshot, load_feature_records,
        persist_feature_artifacts,
    },
    features::windows::FeatureWindowConfig,
    model::{
        dynamic_state::{DynamicState, PublicPosition},
        identity::Identity,
        issue_preferences::IssuePreferences,
        senator::Senator,
        senator_feature_record::{FeatureReport, SenatorFeatureRecord},
        structural::Structural,
        procedural::Procedural,
        data_snapshot::DataSnapshot,
    },
};
use chrono::Datelike;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenatorProfileMode {
    Synthetic,
    HistoricalFeatures,
}

pub fn feature_record_to_senator(
    record: &SenatorFeatureRecord,
) -> Result<Senator, SenateSimError> {
    let senator = Senator {
        identity: Identity {
            senator_id: record.senator_id.clone(),
            full_name: record.full_name.clone(),
            party: record.party.clone(),
            state: record.state.clone(),
            class: default_class(&record.senator_id),
            start_date: record.snapshot_date - chrono::TimeDelta::days(365),
            end_date: None,
        },
        structural: Structural {
            ideology_score: record.ideology_proxy,
            party_loyalty_baseline: record.party_loyalty_baseline,
            bipartisanship_baseline: record.bipartisanship_baseline,
            committee_assignments: vec!["Feature-Derived Committee".to_string()],
            reelection_year: Some(record.snapshot_date.year() + 2),
            electoral_vulnerability: (1.0 - record.party_loyalty_baseline * 0.4
                + record.recent_bipartisanship * 0.2)
                .clamp(0.0, 1.0),
        },
        issue_preferences: IssuePreferences {
            defense: record.defense_score,
            immigration: record.immigration_score,
            energy_climate: record.energy_climate_score,
            labor: record.labor_score,
            healthcare: record.healthcare_score,
            tax_spending: record.budget_tax_score,
            judiciary: record.judiciary_score,
            trade: 0.0,
            tech_privacy: record.technology_score,
            foreign_policy: record.foreign_policy_score,
        },
        procedural: Procedural {
            cloture_support_baseline: record.cloture_support_baseline,
            motion_to_proceed_baseline: record.motion_to_proceed_baseline,
            uc_objection_tendency: (1.0 - record.bipartisanship_baseline * 0.6
                - record.amendment_openness * 0.2)
                .clamp(0.0, 1.0),
            leadership_deference: (record.party_loyalty_baseline * 0.7
                + (1.0 - record.bipartisanship_baseline) * 0.2)
                .clamp(0.0, 1.0),
            amendment_openness: record.amendment_openness,
            attendance_reliability: record.attendance_reliability,
        },
        dynamic_state: DynamicState {
            current_public_position: PublicPosition::Undeclared,
            current_substantive_support: 0.5,
            current_procedural_support: record.recent_cloture_support,
            current_negotiability: (record.recent_bipartisanship * 0.6
                + (1.0 - record.procedural_rigidity) * 0.3)
                .clamp(0.0, 1.0),
            current_party_pressure: record.recent_party_loyalty,
            current_issue_salience_in_state: 0.5,
        },
        feature_coverage_score: Some(record.coverage_score),
        feature_notes: record.notes.clone(),
    };
    senator.validate()?;
    Ok(senator)
}

pub fn snapshot_with_features_to_senators(
    snapshot: &DataSnapshot,
    features: &[SenatorFeatureRecord],
) -> Result<Vec<Senator>, SenateSimError> {
    let summary = snapshot.public_signal_summary.as_ref();
    features
        .iter()
        .map(|record| {
            let mut senator = feature_record_to_senator(record)?;
            if let Some(summary) = summary {
                let attention = summary
                    .senator_attention
                    .get(&record.senator_id)
                    .copied()
                    .unwrap_or(0.0);
                let strongest_link = summary
                    .senator_object_link_strength
                    .iter()
                    .filter(|link| link.senator_id == record.senator_id)
                    .map(|link| link.public_association_score)
                    .fold(0.0, f32::max);
                senator.dynamic_state.current_issue_salience_in_state = senator
                    .dynamic_state
                    .current_issue_salience_in_state
                    .max((attention * 0.7) + (strongest_link * 0.3));
                if attention > 0.0 {
                    senator
                        .feature_notes
                        .push(format!("public narrative attention score {:.2}", attention));
                }
            }
            senator.validate()?;
            Ok(senator)
        })
        .collect()
}

pub fn senators_for_snapshot(
    snapshot: &DataSnapshot,
    data_root: &std::path::Path,
    mode: SenatorProfileMode,
) -> Result<Vec<Senator>, SenateSimError> {
    match mode {
        SenatorProfileMode::Synthetic => crate::ingest::snapshot_to_senators(snapshot),
        SenatorProfileMode::HistoricalFeatures => {
            let features = match load_feature_records(data_root, snapshot.snapshot_date) {
                Ok(records) => records,
                Err(_) => {
                    let config = FeatureWindowConfig::default();
                    let features =
                        build_senator_features_for_snapshot(snapshot, &snapshot.vote_records, &config)?;
                    let report = build_feature_report(snapshot.snapshot_date, &features);
                    persist_feature_artifacts(data_root, snapshot.snapshot_date, &features, &report)?;
                    features
                }
            };
            snapshot_with_features_to_senators(snapshot, &features)
        }
    }
}

pub fn build_and_persist_features(
    snapshot: &DataSnapshot,
    data_root: &std::path::Path,
    config: &FeatureWindowConfig,
) -> Result<(Vec<SenatorFeatureRecord>, FeatureReport), SenateSimError> {
    let features = build_senator_features_for_snapshot(snapshot, &snapshot.vote_records, config)?;
    let report = build_feature_report(snapshot.snapshot_date, &features);
    persist_feature_artifacts(data_root, snapshot.snapshot_date, &features, &report)?;
    Ok((features, report))
}

fn default_class(senator_id: &str) -> crate::SenateClass {
    match senator_id.bytes().fold(0u8, |acc, value| acc.wrapping_add(value)) % 3 {
        0 => crate::SenateClass::I,
        1 => crate::SenateClass::II,
        _ => crate::SenateClass::III,
    }
}
