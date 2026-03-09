use std::path::Path;

use chrono::NaiveDate;
use serde_json::Value;

use crate::{
    error::SenateSimError,
    model::{
        identity::Party,
        legislative::PolicyDomain,
        raw_records::RawVoteRecord,
    },
};

use super::sources::{
    fetched_at_for, read_source_array, resolve_fixture_path, write_json_value,
};

pub fn ingest_votes(
    run_date: NaiveDate,
    fixture_root: &Path,
    data_root: &Path,
) -> Result<Vec<RawVoteRecord>, SenateSimError> {
    let source_path = resolve_fixture_path(fixture_root, run_date, "votes.json")?;
    let records = read_source_array(&source_path)?;
    let fetched_at = fetched_at_for(run_date)?;
    let raw_path = data_root.join("raw").join(run_date.to_string()).join("votes.json");
    write_json_value(&raw_path, &Value::Array(records.clone()))?;

    records
        .into_iter()
        .map(|record| parse_raw_vote_record_value(record, run_date, fetched_at))
        .collect()
}

pub fn parse_raw_vote_record_value(
    value: Value,
    run_date: NaiveDate,
    fetched_at: chrono::DateTime<chrono::Utc>,
) -> Result<RawVoteRecord, SenateSimError> {
    let object = value
        .as_object()
        .ok_or_else(|| SenateSimError::Validation {
            field: "raw_vote_record",
            message: "fixture records must be JSON objects".to_string(),
        })?;

    Ok(RawVoteRecord {
        source_vote_id: required_string(object.get("source_vote_id"), "source_vote_id")?,
        vote_date: required_date(object.get("vote_date"), "vote_date")?,
        senator_id: required_string(object.get("senator_id"), "senator_id")?,
        senator_name: required_string(object.get("senator_name"), "senator_name")?,
        object_id: optional_string(object.get("object_id"))?,
        vote_category: required_string(object.get("vote_category"), "vote_category")?,
        vote_position: required_string(object.get("vote_position"), "vote_position")?,
        party_at_time: parse_party(required_string(object.get("party_at_time"), "party_at_time")?),
        policy_domain: parse_policy_domain(optional_string(object.get("policy_domain"))?),
        is_procedural: object
            .get("is_procedural")
            .and_then(Value::as_bool)
            .ok_or_else(|| SenateSimError::Validation {
                field: "raw_vote_record.is_procedural",
                message: "must be boolean".to_string(),
            })?,
        procedural_kind: optional_string(object.get("procedural_kind"))?,
        as_of_date: run_date,
        source_name: required_string(object.get("source_name"), "source_name")?,
        source_identifier: required_string(object.get("source_identifier"), "source_identifier")?,
        source_url: optional_string(object.get("source_url"))?,
        fetched_at,
        raw_payload: Value::Object(object.clone()),
    })
}

fn required_string(value: Option<&Value>, field: &'static str) -> Result<String, SenateSimError> {
    value
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| SenateSimError::Validation {
            field,
            message: "must not be empty".to_string(),
        })
}

fn optional_string(value: Option<&Value>) -> Result<Option<String>, SenateSimError> {
    match value {
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(SenateSimError::Validation {
            field: "optional_string",
            message: "must be string or null".to_string(),
        }),
    }
}

fn required_date(
    value: Option<&Value>,
    field: &'static str,
) -> Result<NaiveDate, SenateSimError> {
    let text = value
        .and_then(Value::as_str)
        .ok_or_else(|| SenateSimError::Validation {
            field,
            message: "must not be empty".to_string(),
        })?;
    NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|_| SenateSimError::Validation {
        field,
        message: format!("invalid date {text}"),
    })
}

fn parse_party(value: String) -> Party {
    if value.eq_ignore_ascii_case("d") || value.eq_ignore_ascii_case("democrat") {
        Party::Democrat
    } else if value.eq_ignore_ascii_case("r") || value.eq_ignore_ascii_case("republican") {
        Party::Republican
    } else if value.eq_ignore_ascii_case("i") || value.eq_ignore_ascii_case("independent") {
        Party::Independent
    } else {
        Party::Other(value)
    }
}

fn parse_policy_domain(value: Option<String>) -> Option<PolicyDomain> {
    let value = value?;
    Some(match value.as_str() {
        text if text.eq_ignore_ascii_case("defense") => PolicyDomain::Defense,
        text if text.eq_ignore_ascii_case("budgettax") || text.eq_ignore_ascii_case("budget_tax") => {
            PolicyDomain::BudgetTax
        }
        text if text.eq_ignore_ascii_case("healthcare") => PolicyDomain::Healthcare,
        text if text.eq_ignore_ascii_case("immigration") => PolicyDomain::Immigration,
        text if text.eq_ignore_ascii_case("energyclimate")
            || text.eq_ignore_ascii_case("energy_climate") =>
        {
            PolicyDomain::EnergyClimate
        }
        text if text.eq_ignore_ascii_case("judiciary") => PolicyDomain::Judiciary,
        text if text.eq_ignore_ascii_case("technology") => PolicyDomain::Technology,
        text if text.eq_ignore_ascii_case("foreignpolicy")
            || text.eq_ignore_ascii_case("foreign_policy") =>
        {
            PolicyDomain::ForeignPolicy
        }
        text if text.eq_ignore_ascii_case("labor") => PolicyDomain::Labor,
        text if text.eq_ignore_ascii_case("education") => PolicyDomain::Education,
        _ => PolicyDomain::Other(value),
    })
}
