use std::path::Path;

use chrono::NaiveDate;
use serde_json::Value;

use crate::{error::SenateSimError, model::raw_records::RawLegislativeRecord};

use super::sources::{fetched_at_for, read_source_array, resolve_fixture_path, write_json_value};

pub fn ingest_legislation(
    run_date: NaiveDate,
    fixture_root: &Path,
    data_root: &Path,
) -> Result<Vec<RawLegislativeRecord>, SenateSimError> {
    let source_path = resolve_fixture_path(fixture_root, run_date, "legislation.json")?;
    let records = read_source_array(&source_path)?;
    let fetched_at = fetched_at_for(run_date)?;
    let raw_path = data_root
        .join("raw")
        .join(run_date.to_string())
        .join("legislation.json");
    write_json_value(&raw_path, &Value::Array(records.clone()))?;

    records
        .into_iter()
        .map(|record| parse_raw_legislative_record(record, run_date, fetched_at))
        .collect()
}

fn parse_raw_legislative_record(
    value: Value,
    run_date: NaiveDate,
    fetched_at: chrono::DateTime<chrono::Utc>,
) -> Result<RawLegislativeRecord, SenateSimError> {
    let object = value
        .as_object()
        .ok_or_else(|| SenateSimError::Validation {
            field: "raw_legislative_record",
            message: "fixture records must be JSON objects".to_string(),
        })?;

    Ok(RawLegislativeRecord {
        source_object_id: required_string(object.get("source_object_id"), "source_object_id")?,
        title: required_string(object.get("title"), "title")?,
        summary: optional_string(object.get("summary"))?,
        introduced_date: optional_date(object.get("introduced_date"))?,
        sponsor: optional_string(object.get("sponsor"))?,
        latest_status_text: optional_string(object.get("latest_status_text"))?,
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

fn optional_date(value: Option<&Value>) -> Result<Option<NaiveDate>, SenateSimError> {
    match value {
        Some(Value::String(text)) => NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| SenateSimError::Validation {
                field: "optional_date",
                message: format!("invalid date {text}"),
            }),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(SenateSimError::Validation {
            field: "optional_date",
            message: "must be string or null".to_string(),
        }),
    }
}
