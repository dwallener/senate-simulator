use std::path::PathBuf;

use chrono::NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestionSourceMode {
    Fixtures,
    Live,
}

#[derive(Debug, Clone)]
pub struct IngestionConfig {
    pub run_date: NaiveDate,
    pub source_mode: IngestionSourceMode,
    pub congress_api_key: Option<String>,
    pub output_root: PathBuf,
    pub fixture_root: PathBuf,
    pub use_cached_raw_if_present: bool,
    pub include_gdelt: bool,
    pub gdelt_query_limit: usize,
}

impl IngestionConfig {
    pub fn fixtures(run_date: NaiveDate) -> Self {
        Self {
            run_date,
            source_mode: IngestionSourceMode::Fixtures,
            congress_api_key: None,
            output_root: PathBuf::from("data"),
            fixture_root: PathBuf::from("fixtures/ingest"),
            use_cached_raw_if_present: false,
            include_gdelt: false,
            gdelt_query_limit: 5,
        }
    }

    pub fn live(run_date: NaiveDate, congress_api_key: Option<String>) -> Self {
        Self {
            run_date,
            source_mode: IngestionSourceMode::Live,
            congress_api_key,
            output_root: PathBuf::from("data"),
            fixture_root: PathBuf::from("fixtures/ingest"),
            use_cached_raw_if_present: false,
            include_gdelt: false,
            gdelt_query_limit: 5,
        }
    }
}
