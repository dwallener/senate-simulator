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
    #[error("HTTP client error: {0}")]
    HttpClient(#[source] reqwest::Error),
    #[error("HTTP error from {url}: status {status}")]
    HttpStatus { url: String, status: reqwest::StatusCode },
    #[error("unexpected response format from {url}: expected {expected}, body starts with {body_prefix}")]
    UnexpectedResponseFormat {
        url: String,
        expected: &'static str,
        body_prefix: String,
    },
    #[error("XML parse error: {0}")]
    Xml(#[source] quick_xml::DeError),
    #[error("JSON serialization error: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("validation error for {field}: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },
}
