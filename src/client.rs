use base64::Engine;
use chrono::NaiveDate;

use serde::Deserialize;

use crate::core::RawHeartbeat;
use crate::error::Result;

#[derive(Deserialize)]
struct HeartbeatsResponse {
    #[serde(default)]
    data: Vec<RawHeartbeat>,
}

pub const DEFAULT_BASE_URL: &str = "https://wakapi.dev";

pub fn encode_api_key(key: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(key.as_bytes());
    format!("Basic {encoded}")
}

pub struct WakapiClient {
    base_url: String,
    user: String,
    auth: String,
    client: reqwest::blocking::Client,
}

impl WakapiClient {
    pub fn new(base_url: &str, user: &str, auth: &str, timeout_secs: f64) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs_f64(timeout_secs))
            .build()
            .expect("failed to build HTTP client");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            user: user.to_string(),
            auth: auth.to_string(),
            client,
        }
    }

    pub fn fetch_heartbeats(&self, date: NaiveDate) -> Result<Vec<RawHeartbeat>> {
        let url = format!(
            "{}/api/compat/wakatime/v1/users/{}/heartbeats?date={}",
            self.base_url,
            self.user,
            date.format("%Y-%m-%d"),
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", &self.auth)
            .send()?;
        resp.error_for_status_ref()?;
        let payload: HeartbeatsResponse = resp.json()?;
        Ok(payload.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_api_key_basic() {
        let result = encode_api_key("mytoken");
        let expected = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(b"mytoken")
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn encode_api_key_empty() {
        assert_eq!(encode_api_key(""), "Basic ");
    }

    #[test]
    fn fetch_heartbeats_returns_data() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                "/api/compat/wakatime/v1/users/me/heartbeats?date=2026-02-01",
            )
            .match_header("Authorization", "Basic abc")
            .with_body(r#"{"data":[{"time":100,"project":"foo"}]}"#)
            .with_header("content-type", "application/json")
            .create();

        let client = WakapiClient::new(&server.url(), "me", "Basic abc", 15.0);
        let result = client
            .fetch_heartbeats(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap())
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].time, Some(100.0));
        assert_eq!(result[0].project.as_deref(), Some("foo"));
        mock.assert();
    }

    #[test]
    fn fetch_heartbeats_missing_data() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                "/api/compat/wakatime/v1/users/me/heartbeats?date=2026-02-01",
            )
            .with_body(r#"{}"#)
            .with_header("content-type", "application/json")
            .create();

        let client = WakapiClient::new(&server.url(), "me", "Basic abc", 15.0);
        let result = client
            .fetch_heartbeats(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap())
            .unwrap();
        assert!(result.is_empty());
        mock.assert();
    }

    #[test]
    fn fetch_heartbeats_non_list_data() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                "/api/compat/wakatime/v1/users/me/heartbeats?date=2026-02-01",
            )
            .with_body(r#"{"data":"not a list"}"#)
            .with_header("content-type", "application/json")
            .create();

        let client = WakapiClient::new(&server.url(), "me", "Basic abc", 15.0);
        let result = client.fetch_heartbeats(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        assert!(result.is_err());
        mock.assert();
    }

    #[test]
    fn fetch_heartbeats_strips_trailing_slash() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                "/api/compat/wakatime/v1/users/me/heartbeats?date=2026-02-01",
            )
            .with_body(r#"{"data":[]}"#)
            .with_header("content-type", "application/json")
            .create();

        // Pass URL with trailing slash
        let url = format!("{}/", server.url());
        let client = WakapiClient::new(&url, "me", "Basic abc", 15.0);
        client
            .fetch_heartbeats(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap())
            .unwrap();
        mock.assert();
    }

    #[test]
    fn fetch_heartbeats_http_error() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock(
                "GET",
                "/api/compat/wakatime/v1/users/me/heartbeats?date=2026-02-01",
            )
            .with_status(500)
            .create();

        let client = WakapiClient::new(&server.url(), "me", "Basic abc", 15.0);
        let result = client.fetch_heartbeats(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        assert!(result.is_err());
        mock.assert();
    }
}
