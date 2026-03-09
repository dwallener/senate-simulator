use std::{collections::BTreeMap, thread, time::Duration};

use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::{HeaderMap, HeaderValue},
};
use serde_json::Value;

use crate::error::SenateSimError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitStatus {
    pub limit: Option<u32>,
    pub remaining: Option<u32>,
}

pub struct CongressApiClient {
    client: Client,
    api_key: String,
    base_url: String,
    pacing_delay: Duration,
}

impl CongressApiClient {
    pub fn new(api_key: String) -> Result<Self, SenateSimError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Api-Key",
            HeaderValue::from_str(&api_key).map_err(|_| SenateSimError::Validation {
                field: "ingest.congress_api_key",
                message: "API key contains invalid header characters".to_string(),
            })?,
        );
        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(SenateSimError::HttpClient)?;
        Ok(Self {
            client,
            api_key,
            base_url: "https://api.congress.gov/v3".to_string(),
            pacing_delay: Duration::from_millis(150),
        })
    }

    pub fn fetch_bills(&self, congress: u32) -> Result<(Value, RateLimitStatus, String), SenateSimError> {
        self.fetch_json(&format!("/bill/{congress}"), &[("limit", "250"), ("format", "json")])
    }

    pub fn fetch_amendments(
        &self,
        congress: u32,
    ) -> Result<(Value, RateLimitStatus, String), SenateSimError> {
        self.fetch_json(
            &format!("/amendment/{congress}"),
            &[("limit", "250"), ("format", "json")],
        )
    }

    pub fn fetch_members(&self) -> Result<(Value, RateLimitStatus, String), SenateSimError> {
        self.fetch_json("/member", &[("currentMember", "true"), ("format", "json")])
    }

    pub fn fetch_bill_actions(
        &self,
        congress: u32,
        bill_type: &str,
        bill_number: &str,
    ) -> Result<(Value, RateLimitStatus, String), SenateSimError> {
        self.fetch_json(
            &format!("/bill/{congress}/{bill_type}/{bill_number}/actions"),
            &[("limit", "250"), ("format", "json")],
        )
    }

    fn fetch_json(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<(Value, RateLimitStatus, String), SenateSimError> {
        thread::sleep(self.pacing_delay);

        let mut query = BTreeMap::new();
        for (key, value) in params {
            query.insert(*key, *value);
        }
        query.insert("api_key", self.api_key.as_str());

        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .query(&query)
            .send()
            .map_err(SenateSimError::HttpClient)?;
        let status = parse_rate_limit_headers(response.headers());
        let response = handle_response(url.clone(), response)?;
        let body = response.text().map_err(SenateSimError::HttpClient)?;
        let payload = serde_json::from_str(&body).map_err(|source| SenateSimError::Validation {
            field: "ingest.congress_api_payload",
            message: format!("invalid JSON payload from {url}: {source}"),
        })?;
        Ok((payload, status, url))
    }
}

fn handle_response(url: String, response: Response) -> Result<Response, SenateSimError> {
    match response.status() {
        StatusCode::OK => Ok(response),
        StatusCode::TOO_MANY_REQUESTS | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
            Err(SenateSimError::HttpStatus {
                url,
                status: response.status(),
            })
        }
        status => Err(SenateSimError::HttpStatus { url, status }),
    }
}

pub fn parse_rate_limit_headers(headers: &HeaderMap) -> RateLimitStatus {
    let limit = headers
        .get("x-ratelimit-limit")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u32>().ok());
    let remaining = headers
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u32>().ok());
    RateLimitStatus { limit, remaining }
}

#[cfg(test)]
mod tests {
    use reqwest::header::{HeaderMap, HeaderValue};

    use super::parse_rate_limit_headers;

    #[test]
    fn parses_rate_limit_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-limit", HeaderValue::from_static("1000"));
        headers.insert("x-ratelimit-remaining", HeaderValue::from_static("742"));
        let parsed = parse_rate_limit_headers(&headers);
        assert_eq!(parsed.limit, Some(1000));
        assert_eq!(parsed.remaining, Some(742));
    }
}
