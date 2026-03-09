use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;

use crate::error::SenateSimError;

pub fn resolve_fixture_path(
    fixture_root: &Path,
    run_date: NaiveDate,
    file_name: &str,
) -> Result<PathBuf, SenateSimError> {
    let direct = fixture_root.join(run_date.to_string()).join(file_name);
    if direct.exists() {
        return Ok(direct);
    }

    let mut candidates = fs::read_dir(fixture_root)
        .map_err(|source| SenateSimError::Io {
            path: fixture_root.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let date = NaiveDate::parse_from_str(name, "%Y-%m-%d").ok()?;
            if date <= run_date {
                Some((date, entry.path().join(file_name)))
            } else {
                None
            }
        })
        .filter(|(_, path)| path.exists())
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates
        .pop()
        .map(|(_, path)| path)
        .ok_or_else(|| SenateSimError::Validation {
            field: "ingest.fixture_path",
            message: format!(
                "no fixture file {file_name} found at or before {}",
                run_date
            ),
        })
}

pub fn read_source_array(path: &Path) -> Result<Vec<Value>, SenateSimError> {
    let contents = fs::read_to_string(path).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| SenateSimError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub fn write_json_value(path: &Path, value: &Value) -> Result<(), SenateSimError> {
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

pub fn write_string(path: &Path, contents: &str) -> Result<(), SenateSimError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SenateSimError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, contents).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn read_string(path: &Path) -> Result<String, SenateSimError> {
    fs::read_to_string(path).map_err(|source| SenateSimError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn raw_storage_dir(data_root: &Path, run_date: NaiveDate) -> PathBuf {
    data_root.join("raw").join(run_date.to_string())
}

pub fn fetched_at_for(run_date: NaiveDate) -> Result<DateTime<Utc>, SenateSimError> {
    let naive = run_date
        .and_hms_opt(12, 0, 0)
        .ok_or_else(|| SenateSimError::Validation {
            field: "ingest.fetched_at",
            message: "invalid run date".to_string(),
        })?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

pub fn content_hash_string(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
