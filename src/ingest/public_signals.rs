use std::{collections::{HashMap, HashSet}, path::Path};

use chrono::NaiveDate;
use serde_json::{Value, json};

use crate::{
    error::SenateSimError,
    model::{
        data_snapshot::DataSnapshot,
        legislative::PolicyDomain,
        normalized_public_signal_record::NormalizedPublicSignalRecord,
        normalized_records::{NormalizedLegislativeRecord, NormalizedSenatorRecord},
        public_signal_summary::{PublicSignalSummary, build_public_signal_summary},
        raw_public_signal_record::{PublicSignalScope, RawPublicSignalRecord},
    },
};

use super::{
    config::{IngestionConfig, IngestionSourceMode},
    gdelt::GdeltClient,
    sources::{fetched_at_for, raw_storage_dir, read_string, resolve_fixture_path, write_json_value},
};

pub fn ingest_public_signals(
    config: &IngestionConfig,
    data_root: &Path,
    roster_records: &[NormalizedSenatorRecord],
    legislative_records: &[NormalizedLegislativeRecord],
) -> Result<Vec<RawPublicSignalRecord>, SenateSimError> {
    if !config.include_gdelt {
        return Ok(Vec::new());
    }

    match config.source_mode {
        IngestionSourceMode::Fixtures => ingest_fixture_public_signals(config, data_root),
        IngestionSourceMode::Live => ingest_live_public_signals(config, data_root, roster_records, legislative_records),
    }
}

pub fn normalize_public_signals(
    raw_records: &[RawPublicSignalRecord],
) -> Result<Vec<NormalizedPublicSignalRecord>, SenateSimError> {
    raw_records
        .iter()
        .map(normalize_public_signal_record)
        .collect()
}

pub fn build_public_signal_artifacts(
    snapshot_date: NaiveDate,
    raw_records: &[RawPublicSignalRecord],
) -> Result<(Vec<NormalizedPublicSignalRecord>, PublicSignalSummary), SenateSimError> {
    let normalized = normalize_public_signals(raw_records)?;
    let summary = build_public_signal_summary(snapshot_date, &normalized);
    summary.validate()?;
    Ok((normalized, summary))
}

pub fn persist_public_signal_records(
    data_root: &Path,
    snapshot_date: NaiveDate,
    records: &[NormalizedPublicSignalRecord],
    summary: &PublicSignalSummary,
) -> Result<(), SenateSimError> {
    let dir = super::snapshot::normalized_storage_dir(data_root, snapshot_date);
    super::snapshot::write_json_file(&dir.join("public_signals.json"), records)?;
    super::snapshot::write_json_file(&dir.join("public_signal_summary.json"), summary)?;
    Ok(())
}

fn ingest_fixture_public_signals(
    config: &IngestionConfig,
    data_root: &Path,
) -> Result<Vec<RawPublicSignalRecord>, SenateSimError> {
    let path = match resolve_fixture_path(&config.fixture_root, config.run_date, "gdelt_public_signals.json") {
        Ok(path) => path,
        Err(_) => return Ok(Vec::new()),
    };
    let contents = read_string(&path)?;
    let payload: Value = serde_json::from_str(&contents).map_err(|source| SenateSimError::Json {
        path: path.clone(),
        source,
    })?;
    let raw_dir = raw_storage_dir(data_root, config.run_date);
    std::fs::create_dir_all(&raw_dir).map_err(|source| SenateSimError::Io {
        path: raw_dir.clone(),
        source,
    })?;
    write_json_value(&raw_dir.join("gdelt_public_signals.json"), &payload)?;
    parse_raw_public_signal_payload(config.run_date, payload)
}

fn ingest_live_public_signals(
    config: &IngestionConfig,
    data_root: &Path,
    roster_records: &[NormalizedSenatorRecord],
    legislative_records: &[NormalizedLegislativeRecord],
) -> Result<Vec<RawPublicSignalRecord>, SenateSimError> {
    let client = GdeltClient::new()?;
    let raw_dir = raw_storage_dir(data_root, config.run_date);
    std::fs::create_dir_all(&raw_dir).map_err(|source| SenateSimError::Io {
        path: raw_dir.clone(),
        source,
    })?;

    let mut records = Vec::new();
    let query_limit = config.gdelt_query_limit.max(1);

    for object in legislative_records.iter().take(query_limit) {
        let query = object_query(object);
        let payload = fetch_or_load_query(config, &client, &raw_dir.join(format!("gdelt_object_{}.json", object.object_id)), &query)?;
        records.push(raw_record_from_query(
            config.run_date,
            PublicSignalScope::LegislativeObject,
            format!("gdelt_object_{}", object.object_id),
            query,
            None,
            Some(object.object_id.clone()),
            Some(object.policy_domain.clone()),
            payload,
        )?);
    }

    for senator in roster_records.iter().take(query_limit) {
        let query = senator_query(senator);
        let payload = fetch_or_load_query(config, &client, &raw_dir.join(format!("gdelt_senator_{}.json", senator.senator_id)), &query)?;
        records.push(raw_record_from_query(
            config.run_date,
            PublicSignalScope::Senator,
            format!("gdelt_senator_{}", senator.senator_id),
            query,
            Some(senator.senator_id.clone()),
            None,
            None,
            payload,
        )?);
    }

    let mut seen_domains = HashSet::new();
    for object in legislative_records.iter() {
        if seen_domains.insert(object.policy_domain.clone()) && seen_domains.len() <= query_limit {
            let query = domain_query(&object.policy_domain);
            let payload = fetch_or_load_query(
                config,
                &client,
                &raw_dir.join(format!("gdelt_domain_{}.json", sanitize_key(&object.policy_domain.to_string()))),
                &query,
            )?;
            records.push(raw_record_from_query(
                config.run_date,
                PublicSignalScope::PolicyDomain,
                format!("gdelt_domain_{}", sanitize_key(&object.policy_domain.to_string())),
                query,
                None,
                None,
                Some(object.policy_domain.clone()),
                payload,
            )?);
        }
    }

    let aggregate_payload = Value::Array(
        records
            .iter()
            .map(|record| {
                json!({
                    "signal_id": record.signal_id,
                    "scope": record.scope,
                    "query": record.query,
                    "linked_senator_id": record.linked_senator_id,
                    "linked_object_id": record.linked_object_id,
                    "policy_domain": record.policy_domain,
                    "source_url": record.source_url,
                    "raw_payload": record.raw_payload,
                })
            })
            .collect(),
    );
    write_json_value(&raw_dir.join("gdelt_public_signals.json"), &aggregate_payload)?;
    Ok(records)
}

fn fetch_or_load_query(
    config: &IngestionConfig,
    client: &GdeltClient,
    path: &Path,
    query: &str,
) -> Result<Value, SenateSimError> {
    if config.use_cached_raw_if_present && path.exists() {
        let contents = read_string(path)?;
        return serde_json::from_str(&contents).map_err(|source| SenateSimError::Json {
            path: path.to_path_buf(),
            source,
        });
    }
    let (payload, source_url) = client.fetch_query(query, config.gdelt_query_limit.max(10))?;
    let wrapped = json!({
        "query": query,
        "source_url": source_url,
        "fetched_at": fetched_at_for(config.run_date)?,
        "payload": payload,
    });
    write_json_value(path, &wrapped)?;
    Ok(wrapped)
}

fn raw_record_from_query(
    snapshot_date: NaiveDate,
    scope: PublicSignalScope,
    signal_id: String,
    query: String,
    linked_senator_id: Option<String>,
    linked_object_id: Option<String>,
    policy_domain: Option<PolicyDomain>,
    payload: Value,
) -> Result<RawPublicSignalRecord, SenateSimError> {
    Ok(RawPublicSignalRecord {
        signal_id: signal_id.clone(),
        snapshot_date,
        scope,
        query,
        linked_senator_id,
        linked_object_id,
        policy_domain,
        source_name: "gdelt".to_string(),
        source_identifier: signal_id,
        source_url: payload.get("source_url").and_then(Value::as_str).map(str::to_string),
        fetched_at: fetched_at_for(snapshot_date)?,
        raw_payload: payload,
    })
}

fn parse_raw_public_signal_payload(
    snapshot_date: NaiveDate,
    payload: Value,
) -> Result<Vec<RawPublicSignalRecord>, SenateSimError> {
    let items = payload.as_array().cloned().unwrap_or_default();
    items.into_iter()
        .map(|value| {
            let object = value.as_object().ok_or_else(|| SenateSimError::Validation {
                field: "raw_public_signal_record",
                message: "fixture GDELT records must be objects".to_string(),
            })?;
            Ok(RawPublicSignalRecord {
                signal_id: object.get("signal_id").and_then(Value::as_str).unwrap_or("gdelt_signal").to_string(),
                snapshot_date,
                scope: parse_scope(object.get("scope").and_then(Value::as_str).unwrap_or("mixed")),
                query: object.get("query").and_then(Value::as_str).unwrap_or("").to_string(),
                linked_senator_id: object.get("linked_senator_id").and_then(Value::as_str).map(str::to_string),
                linked_object_id: object.get("linked_object_id").and_then(Value::as_str).map(str::to_string),
                policy_domain: object.get("policy_domain").and_then(Value::as_str).map(parse_policy_domain),
                source_name: "gdelt".to_string(),
                source_identifier: object.get("signal_id").and_then(Value::as_str).unwrap_or("gdelt_signal").to_string(),
                source_url: object.get("source_url").and_then(Value::as_str).map(str::to_string),
                fetched_at: fetched_at_for(snapshot_date)?,
                raw_payload: object.get("raw_payload").cloned().unwrap_or(Value::Object(object.clone())),
            })
        })
        .collect()
}

fn normalize_public_signal_record(
    raw_record: &RawPublicSignalRecord,
) -> Result<NormalizedPublicSignalRecord, SenateSimError> {
    let payload = raw_record.raw_payload.get("payload").unwrap_or(&raw_record.raw_payload);
    let articles = payload
        .get("articles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| payload.as_array().cloned().unwrap_or_default());
    let mention_count = articles.len() as u32;
    let source_count = unique_source_count(&articles);
    let attention_score = ((mention_count as f32 / 10.0)
        + source_count.map(|count| count as f32 / 20.0).unwrap_or(0.0))
        .clamp(0.0, 1.0);
    let tone_score = average_tone(&articles);
    let (top_themes, top_persons, top_organizations) =
        extract_signal_metadata(raw_record, &articles);

    let record = NormalizedPublicSignalRecord {
        snapshot_date: raw_record.snapshot_date,
        signal_id: raw_record.signal_id.clone(),
        signal_scope: raw_record.scope.clone(),
        linked_senator_id: raw_record.linked_senator_id.clone(),
        linked_object_id: raw_record.linked_object_id.clone(),
        policy_domain: raw_record.policy_domain.clone(),
        mention_count,
        attention_score,
        tone_score,
        source_count,
        top_themes,
        top_persons,
        top_organizations,
    };
    record.validate()?;
    Ok(record)
}

fn extract_signal_metadata(
    raw_record: &RawPublicSignalRecord,
    articles: &[Value],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut organizations = HashMap::<String, u32>::new();
    let mut themes = HashMap::<String, u32>::new();
    for article in articles {
        for field in ["sourceCommonName", "domain", "sourcecountry"] {
            if let Some(value) = article.get(field).and_then(Value::as_str) {
                *organizations.entry(value.to_string()).or_default() += 1;
            }
        }
        if let Some(title) = article.get("title").and_then(Value::as_str) {
            for keyword in title
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .filter(|token| token.len() > 4)
                .take(6)
            {
                *themes.entry(keyword.to_ascii_lowercase()).or_default() += 1;
            }
        }
    }

    let mut top_themes = top_counts(themes);
    if top_themes.is_empty() {
        top_themes = raw_record
            .query
            .split_whitespace()
            .map(|token| token.trim_matches('"').to_ascii_lowercase())
            .filter(|token| token.len() > 3)
            .take(3)
            .collect();
    }

    let top_persons = raw_record
        .linked_senator_id
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let top_organizations = top_counts(organizations);
    (top_themes, top_persons, top_organizations)
}

fn unique_source_count(articles: &[Value]) -> Option<u32> {
    let mut domains = HashSet::new();
    for article in articles {
        if let Some(domain) = article
            .get("sourceCommonName")
            .or_else(|| article.get("domain"))
            .and_then(Value::as_str)
        {
            domains.insert(domain.to_string());
        }
    }
    if domains.is_empty() {
        None
    } else {
        Some(domains.len() as u32)
    }
}

fn average_tone(articles: &[Value]) -> Option<f32> {
    let tones = articles
        .iter()
        .filter_map(|article| match article.get("tone") {
            Some(Value::Number(number)) => number.as_f64().map(|value| value as f32),
            Some(Value::String(text)) => text.parse::<f32>().ok(),
            _ => None,
        })
        .collect::<Vec<_>>();
    if tones.is_empty() {
        None
    } else {
        Some((tones.iter().sum::<f32>() / tones.len() as f32 / 100.0).clamp(-1.0, 1.0))
    }
}

fn top_counts(map: HashMap<String, u32>) -> Vec<String> {
    let mut values = map.into_iter().collect::<Vec<_>>();
    values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    values.into_iter().map(|(key, _)| key).take(3).collect()
}

fn senator_query(senator: &NormalizedSenatorRecord) -> String {
    format!("\"{}\" AND {}", senator.full_name, senator.state)
}

fn object_query(object: &NormalizedLegislativeRecord) -> String {
    format!("\"{}\" OR {}", object.title, object.object_id)
}

fn domain_query(domain: &PolicyDomain) -> String {
    match domain {
        PolicyDomain::Defense => "defense OR pentagon".to_string(),
        PolicyDomain::BudgetTax => "budget OR tax OR spending".to_string(),
        PolicyDomain::Healthcare => "healthcare OR hospital OR medicare".to_string(),
        PolicyDomain::Immigration => "immigration OR border".to_string(),
        PolicyDomain::EnergyClimate => "energy OR climate OR permitting".to_string(),
        PolicyDomain::Judiciary => "judiciary OR court OR judge".to_string(),
        PolicyDomain::Technology => "technology OR privacy OR ai".to_string(),
        PolicyDomain::ForeignPolicy => "foreign policy OR diplomacy".to_string(),
        PolicyDomain::Labor => "labor OR workers OR union".to_string(),
        PolicyDomain::Education => "education OR schools".to_string(),
        PolicyDomain::Other(value) => value.clone(),
    }
}

fn parse_scope(value: &str) -> PublicSignalScope {
    match value {
        text if text.eq_ignore_ascii_case("senator") => PublicSignalScope::Senator,
        text if text.eq_ignore_ascii_case("legislative_object")
            || text.eq_ignore_ascii_case("object") =>
        {
            PublicSignalScope::LegislativeObject
        }
        text if text.eq_ignore_ascii_case("policy_domain")
            || text.eq_ignore_ascii_case("domain") =>
        {
            PublicSignalScope::PolicyDomain
        }
        _ => PublicSignalScope::Mixed,
    }
}

fn parse_policy_domain(value: &str) -> PolicyDomain {
    match value {
        text if text.eq_ignore_ascii_case("defense") => PolicyDomain::Defense,
        text if text.eq_ignore_ascii_case("budgettax") || text.eq_ignore_ascii_case("budget_tax") => PolicyDomain::BudgetTax,
        text if text.eq_ignore_ascii_case("healthcare") => PolicyDomain::Healthcare,
        text if text.eq_ignore_ascii_case("immigration") => PolicyDomain::Immigration,
        text if text.eq_ignore_ascii_case("energyclimate") || text.eq_ignore_ascii_case("energy_climate") => PolicyDomain::EnergyClimate,
        text if text.eq_ignore_ascii_case("judiciary") => PolicyDomain::Judiciary,
        text if text.eq_ignore_ascii_case("technology") => PolicyDomain::Technology,
        text if text.eq_ignore_ascii_case("foreignpolicy") || text.eq_ignore_ascii_case("foreign_policy") => PolicyDomain::ForeignPolicy,
        text if text.eq_ignore_ascii_case("labor") => PolicyDomain::Labor,
        text if text.eq_ignore_ascii_case("education") => PolicyDomain::Education,
        _ => PolicyDomain::Other(value.to_string()),
    }
}

fn sanitize_key(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

pub fn attach_public_signals_to_snapshot(
    snapshot: &mut DataSnapshot,
    records: Vec<NormalizedPublicSignalRecord>,
    summary: PublicSignalSummary,
) {
    snapshot.public_signal_records = records;
    snapshot.public_signal_summary = Some(summary);
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use crate::model::{
        legislative::PolicyDomain,
        normalized_public_signal_record::NormalizedPublicSignalRecord,
        raw_public_signal_record::{PublicSignalScope, RawPublicSignalRecord},
    };

    use super::{
        attach_public_signals_to_snapshot, build_public_signal_artifacts, parse_scope,
    };

    #[test]
    fn raw_payload_normalization_test() {
        let raw = RawPublicSignalRecord {
            signal_id: "gdelt_object_s2100".to_string(),
            snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            scope: PublicSignalScope::LegislativeObject,
            query: "s2100 clean energy".to_string(),
            linked_senator_id: None,
            linked_object_id: Some("s2100".to_string()),
            policy_domain: Some(PolicyDomain::EnergyClimate),
            source_name: "gdelt".to_string(),
            source_identifier: "gdelt_object_s2100".to_string(),
            source_url: Some("https://example.test/gdelt".to_string()),
            fetched_at: chrono::Utc::now(),
            raw_payload: json!({
                "payload": {
                    "articles": [
                        {"title": "Clean energy bill heats up", "sourceCommonName": "Reuters", "tone": 4.0},
                        {"title": "Permitting fight intensifies", "sourceCommonName": "AP", "tone": -1.0}
                    ]
                }
            }),
        };
        let (records, summary) =
            build_public_signal_artifacts(raw.snapshot_date, &[raw]).unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].attention_score > 0.0);
        assert_eq!(summary.object_attention.get("s2100"), Some(&records[0].attention_score));
    }

    #[test]
    fn scope_classification_test() {
        assert!(matches!(parse_scope("senator"), PublicSignalScope::Senator));
        assert!(matches!(
            parse_scope("legislative_object"),
            PublicSignalScope::LegislativeObject
        ));
        assert!(matches!(parse_scope("domain"), PublicSignalScope::PolicyDomain));
    }

    #[test]
    fn summary_aggregation_test() {
        let records = vec![
            NormalizedPublicSignalRecord {
                snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                signal_id: "sig1".to_string(),
                signal_scope: PublicSignalScope::LegislativeObject,
                linked_senator_id: None,
                linked_object_id: Some("s2100".to_string()),
                policy_domain: Some(PolicyDomain::EnergyClimate),
                mention_count: 5,
                attention_score: 0.6,
                tone_score: Some(0.1),
                source_count: Some(2),
                top_themes: vec!["energy".to_string()],
                top_persons: vec![],
                top_organizations: vec!["Reuters".to_string()],
            },
            NormalizedPublicSignalRecord {
                snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
                signal_id: "sig2".to_string(),
                signal_scope: PublicSignalScope::Senator,
                linked_senator_id: Some("real_a0001".to_string()),
                linked_object_id: None,
                policy_domain: Some(PolicyDomain::EnergyClimate),
                mention_count: 4,
                attention_score: 0.5,
                tone_score: None,
                source_count: Some(2),
                top_themes: vec!["climate".to_string()],
                top_persons: vec!["real_a0001".to_string()],
                top_organizations: vec!["AP".to_string()],
            },
        ];
        let summary = crate::model::public_signal_summary::build_public_signal_summary(
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            &records,
        );
        assert_eq!(summary.object_attention.get("s2100"), Some(&0.6));
        assert_eq!(summary.senator_attention.get("real_a0001"), Some(&0.5));
        assert!(!summary.senator_object_link_strength.is_empty());
    }

    #[test]
    fn snapshot_enrichment_test() {
        let mut snapshot = crate::DataSnapshot {
            snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            run_id: "snapshot-20260309".to_string(),
            created_at: chrono::Utc::now(),
            roster_records: vec![],
            legislative_records: vec![],
            action_records: vec![],
            vote_records: vec![],
            public_signal_records: vec![],
            public_signal_summary: None,
            source_manifests: vec![],
        };
        let records = vec![NormalizedPublicSignalRecord {
            snapshot_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            signal_id: "sig1".to_string(),
            signal_scope: PublicSignalScope::PolicyDomain,
            linked_senator_id: None,
            linked_object_id: None,
            policy_domain: Some(PolicyDomain::EnergyClimate),
            mention_count: 3,
            attention_score: 0.4,
            tone_score: None,
            source_count: None,
            top_themes: vec!["energy".to_string()],
            top_persons: vec![],
            top_organizations: vec![],
        }];
        let summary = crate::model::public_signal_summary::build_public_signal_summary(
            snapshot.snapshot_date,
            &records,
        );
        attach_public_signals_to_snapshot(&mut snapshot, records, summary);
        assert_eq!(snapshot.public_signal_records.len(), 1);
        assert!(snapshot.public_signal_summary.is_some());
    }

    #[test]
    fn feature_driven_stance_smoke_with_public_signals() {
        let snapshot = crate::run_ingestion(&crate::IngestionConfig {
            run_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            source_mode: crate::IngestionSourceMode::Fixtures,
            congress_api_key: None,
            output_root: std::env::temp_dir().join("senate_sim_public_signal_smoke"),
            fixture_root: std::path::PathBuf::from("fixtures/ingest"),
            use_cached_raw_if_present: false,
            include_gdelt: true,
            gdelt_query_limit: 5,
        })
        .unwrap();
        let features = crate::build_senator_features_for_snapshot(
            &snapshot,
            &snapshot.vote_records,
            &crate::FeatureWindowConfig::default(),
        )
        .unwrap();
        let senators = crate::snapshot_with_features_to_senators(&snapshot, &features).unwrap();
        let objects = crate::snapshot_to_legislative_objects(&snapshot).unwrap();
        let contexts = crate::snapshot_to_contexts(&snapshot).unwrap();

        let stance = crate::derive_stance_feature_driven(&senators[0], &objects[0], &contexts[0]).unwrap();
        assert!(stance.validate().is_ok());
        assert!(contexts[0].media_attention >= 0.45);
    }
}
