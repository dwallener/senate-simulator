use std::{thread, time::Duration};

use reqwest::blocking::Client;
use serde_json::Value;

use crate::error::SenateSimError;

pub struct GdeltClient {
    client: Client,
    base_url: String,
    pacing_delay: Duration,
}

impl GdeltClient {
    pub fn new() -> Result<Self, SenateSimError> {
        let client = Client::builder().build().map_err(SenateSimError::HttpClient)?;
        Ok(Self {
            client,
            base_url: "https://api.gdeltproject.org/api/v2/doc/doc".to_string(),
            pacing_delay: Duration::from_millis(150),
        })
    }

    pub fn fetch_query(
        &self,
        query: &str,
        max_records: usize,
    ) -> Result<(Value, String), SenateSimError> {
        thread::sleep(self.pacing_delay);
        let response = self
            .client
            .get(&self.base_url)
            .query(&[
                ("query", query),
                ("mode", "ArtList"),
                ("format", "json"),
                ("maxrecords", &max_records.to_string()),
            ])
            .send()
            .map_err(SenateSimError::HttpClient)?;
        let url = response.url().to_string();
        if !response.status().is_success() {
            return Err(SenateSimError::HttpStatus {
                url,
                status: response.status(),
            });
        }
        let payload = response.json::<Value>().map_err(SenateSimError::HttpClient)?;
        Ok((payload, url))
    }
}
