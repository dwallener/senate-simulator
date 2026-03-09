use std::path::Path;

use chrono::NaiveDate;
use serde_json::Value;

use crate::{
    error::SenateSimError,
    model::{legislative_context::Chamber, raw_records::RawActionRecord},
};

use super::sources::{fetched_at_for, read_source_array, resolve_fixture_path, write_json_value};

pub fn ingest_actions(
    run_date: NaiveDate,
    fixture_root: &Path,
    data_root: &Path,
) -> Result<Vec<RawActionRecord>, SenateSimError> {
    let source_path = resolve_fixture_path(fixture_root, run_date, "actions.json")?;
    let records = read_source_array(&source_path)?;
    let fetched_at = fetched_at_for(run_date)?;
    let raw_path = data_root
        .join("raw")
        .join(run_date.to_string())
        .join("actions.json");
    write_json_value(&raw_path, &Value::Array(records.clone()))?;

    records
        .into_iter()
        .map(|record| parse_raw_action_record_value(record, run_date, fetched_at))
        .collect()
}

pub fn parse_raw_action_record_value(
    value: Value,
    run_date: NaiveDate,
    fetched_at: chrono::DateTime<chrono::Utc>,
) -> Result<RawActionRecord, SenateSimError> {
    let object = value
        .as_object()
        .ok_or_else(|| SenateSimError::Validation {
            field: "raw_action_record",
            message: "fixture records must be JSON objects".to_string(),
        })?;

    Ok(RawActionRecord {
        source_action_id: required_string(object.get("source_action_id"), "source_action_id")?,
        object_id: required_string(object.get("object_id"), "object_id")?,
        action_date: required_date(object.get("action_date"), "action_date")?,
        action_text: required_string(object.get("action_text"), "action_text")?,
        action_type: optional_string(object.get("action_type"))?,
        chamber: parse_chamber(object.get("chamber"))?,
        as_of_date: run_date,
        source_name: required_string(object.get("source_name"), "source_name")?,
        source_identifier: required_string(object.get("source_identifier"), "source_identifier")?,
        source_url: optional_string(object.get("source_url"))?,
        fetched_at,
        raw_payload: Value::Object(object.clone()),
    })
}

fn parse_chamber(value: Option<&Value>) -> Result<Chamber, SenateSimError> {
    match value.and_then(Value::as_str) {
        Some(text) if text.eq_ignore_ascii_case("house") => Ok(Chamber::House),
        Some(_) => Ok(Chamber::Senate),
        None => Err(SenateSimError::Validation {
            field: "raw_action_record.chamber",
            message: "must not be empty".to_string(),
        }),
    }
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

fn required_date(value: Option<&Value>, field: &'static str) -> Result<NaiveDate, SenateSimError> {
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
