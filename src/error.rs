use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SenateSimError {
    #[error("I/O error while reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON parse error in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("JSON serialization error: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("validation error for {field}: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },
}
