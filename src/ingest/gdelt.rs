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
        let body = response.text().map_err(SenateSimError::HttpClient)?;
        let payload = parse_gdelt_json_response(&url, &body)?;
        Ok((payload, url))
    }
}

fn parse_gdelt_json_response(url: &str, body: &str) -> Result<Value, SenateSimError> {
    let trimmed = body.trim_start();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return Err(SenateSimError::UnexpectedResponseFormat {
            url: url.to_string(),
            expected: "JSON",
            body_prefix: trimmed.chars().take(200).collect(),
        });
    }
    serde_json::from_str(trimmed).map_err(|_| SenateSimError::UnexpectedResponseFormat {
        url: url.to_string(),
        expected: "JSON",
        body_prefix: trimmed.chars().take(200).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_gdelt_json_response;

    #[test]
    fn rejects_html_error_pages() {
        let error = parse_gdelt_json_response(
            "https://example.test/gdelt",
            "<html><head><title>Error</title></head><body>blocked</body></html>",
        )
        .unwrap_err();
        assert!(format!("{error}").contains("unexpected response format"));
    }

    #[test]
    fn parses_json_payloads() {
        let payload = parse_gdelt_json_response(
            "https://example.test/gdelt",
            r#"{"articles":[{"title":"test"}]}"#,
        )
        .unwrap();
        assert!(payload.get("articles").is_some());
    }
}
