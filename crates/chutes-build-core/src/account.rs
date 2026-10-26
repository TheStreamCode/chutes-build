//! Chutes account and model-management endpoints.
//!
//! Usage endpoints are read-only. The same client also exposes the saved
//! routing aliases (`/model_aliases/`) and their hidden, experimental writes;
//! alias deletion uses a raw-key Authorization header scoped to that route.
//!
//! Responses stay as raw [`serde_json::Value`] deliberately rather than
//! typed structs: `subscription_usage`/`quotas`/`quota_usage`/`model_stats`
//! shapes vary per account tier and per quota kind (aggregate vs. per-chute
//! maps, see [`has_quota_usage_data`]/[`quota_chute_ids`]), and this module's
//! job is to pass that heterogeneous data through to the tool layer, not to
//! constrain it. The one place a fixed shape would help — telling "no quota
//! data" apart from "endpoint failure" — is already handled explicitly by
//! [`ChutesAccountClient::usage_snapshot`]'s per-field `Result` handling and
//! the `quota_usage_fallback` path, not by typing the payload itself.

use futures_util::StreamExt as _;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, AUTHORIZATION};

use crate::{ChutesCredentials, ChutesEndpoints};

#[derive(Debug, Clone)]
pub struct ChutesAccountClient {
    http: reqwest::Client,
    endpoints: ChutesEndpoints,
    credentials: ChutesCredentials,
}

impl ChutesAccountClient {
    // WHY-ALLOW: product-policy client: the SSRF-safe resolver is the security boundary here; extra-CA support for these clients is tracked separately
    #[allow(clippy::disallowed_methods)]
    pub fn from_env() -> Result<Self, AccountError> {
        Self::from_credentials(ChutesCredentials::from_env()?)
    }

    /// Client for an explicitly provided key (interactive setup flows).
    pub fn from_key(key: &str) -> Result<Self, AccountError> {
        Self::from_credentials(ChutesCredentials::from_key(key)?)
    }

    /// Client for explicitly resolved credentials: tool dispatch publishes
    /// the agent's runtime key on the call context, which plain `from_env`
    /// cannot see.
    // WHY-ALLOW: same client as `from_env`; see the note there.
    #[allow(clippy::disallowed_methods)]
    pub fn from_credentials(credentials: ChutesCredentials) -> Result<Self, AccountError> {
        let endpoints = ChutesEndpoints::default();
        endpoints.validate()?;
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .dns_resolver(std::sync::Arc::new(
                    crate::endpoint_policy::SsrfSafeResolver::default(),
                ))
                .build()?,
            endpoints,
            credentials,
        })
    }

    /// Check the key against `/users/me`: `Ok` when the key is accepted.
    /// A 401/403 means a wrong key; anything else (network, 5xx) means the
    /// check itself failed and must not be reported as a bad key.
    pub async fn validate(&self) -> Result<(), AccountError> {
        self.get_json("/users/me").await.map(|_| ())
    }

    /// Returns the `/users/me` profile payload, or `Null` when the endpoint
    /// fails. User surfaces (CLI, TUI) use it for the credit balance and plan
    /// context; the agent tool deliberately never sees the profile.
    pub async fn profile(&self) -> serde_json::Value {
        self.get_json("/users/me")
            .await
            .unwrap_or(serde_json::Value::Null)
    }

    /// Returns account consumption data without fetching the user profile.
    pub async fn usage_snapshot(
        &self,
        include_model_stats: bool,
    ) -> Result<serde_json::Value, AccountError> {
        let model_stats_request = async {
            if include_model_stats {
                self.get_json("/invocations/stats/llm").await
            } else {
                Ok(serde_json::Value::Null)
            }
        };
        let (subscription_usage, quotas, quota_usage, model_stats) = tokio::join!(
            self.get_json("/users/me/subscription_usage"),
            self.get_json("/users/me/quotas"),
            self.get_json("/users/me/quota_usage/me"),
            model_stats_request,
        );
        let subscription_usage = subscription_usage?;
        let quotas = quotas?;
        let quota_usage = match quota_usage {
            Ok(value) if has_quota_usage_data(&value) => value,
            Ok(_) | Err(_) => self.quota_usage_fallback(&quotas).await,
        };
        let model_stats = model_stats.unwrap_or(serde_json::Value::Null);

        Ok(serde_json::json!({
            "subscription_usage": subscription_usage,
            "quotas": quotas,
            "quota_usage": quota_usage,
            "model_stats": model_stats,
        }))
    }

    /// Public inference catalogue, used to map model slugs to chute ids for
    /// alias writes and stats matching.
    pub async fn model_catalog(&self) -> Result<serde_json::Value, AccountError> {
        self.get_absolute_json_retrying(&format!("{}/models", self.endpoints.inference))
            .await
    }

    /// Create or replace a routing alias. The API has no update verb, so a
    /// replace deletes first; callers that need a blue/green switch must do the
    /// two calls themselves.
    pub async fn create_model_alias(
        &self,
        alias: &str,
        chute_ids: &[String],
    ) -> Result<serde_json::Value, AccountError> {
        let body = serde_json::json!({ "alias": alias, "chute_ids": chute_ids });
        self.post_json_retrying("/model_aliases/", &body).await
    }

    /// Delete a routing alias. A missing alias surfaces as
    /// `AccountError::Http { status: 404 }`.
    pub async fn delete_model_alias(&self, alias: &str) -> Result<(), AccountError> {
        self.delete_retrying(&format!("/model_aliases/{alias}"))
            .await
    }

    async fn get_json_retrying(&self, path: &str) -> Result<serde_json::Value, AccountError> {
        let mut attempt = 0;
        loop {
            match self.get_json(path).await {
                Ok(value) => return Ok(value),
                Err(error) if transient_gateway(&error) && attempt < 3 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn get_absolute_json_retrying(
        &self,
        url: &str,
    ) -> Result<serde_json::Value, AccountError> {
        let mut attempt = 0;
        loop {
            match self.get_absolute_json(url).await {
                Ok(value) => return Ok(value),
                Err(error) if transient_gateway(&error) && attempt < 3 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn post_json_retrying(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, AccountError> {
        let mut attempt = 0;
        loop {
            match self.post_json(path, body).await {
                Ok(value) => return Ok(value),
                Err(error) if transient_gateway(&error) && attempt < 3 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn delete_retrying(&self, path: &str) -> Result<(), AccountError> {
        let mut attempt = 0;
        loop {
            match self.delete(path).await {
                Ok(()) => return Ok(()),
                Err(error) if transient_gateway(&error) && attempt < 3 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn get_absolute_json(&self, url: &str) -> Result<serde_json::Value, AccountError> {
        let response = self
            .http
            .get(url)
            .header(ACCEPT, "application/json")
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(AccountError::Http {
                status: status.as_u16(),
                body: body.chars().take(500).collect(),
            });
        }
        serde_json::from_str(&body).map_err(AccountError::Decode)
    }

    async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, AccountError> {
        let response = self
            .http
            .post(format!("{}{}", self.endpoints.account, path))
            .header(
                AUTHORIZATION,
                self.credentials.management_authorization_header(),
            )
            .header(ACCEPT, "application/json")
            .timeout(std::time::Duration::from_secs(30))
            .json(body)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(AccountError::Http {
                status: status.as_u16(),
                body: text.chars().take(500).collect(),
            });
        }
        if text.trim().is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&text).map_err(AccountError::Decode)
    }

    async fn delete(&self, path: &str) -> Result<(), AccountError> {
        let response = self
            .http
            .delete(format!("{}{}", self.endpoints.account, path))
            // The alias delete route only accepts the raw key; Bearer answers
            // 404 for an alias that GET lists. Verified 2026-10-26.
            .header(
                AUTHORIZATION,
                self.credentials.management_raw_authorization_header(),
            )
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        Err(AccountError::Http {
            status: status.as_u16(),
            body: body.chars().take(500).collect(),
        })
    }

    /// Saved model-routing aliases (`GET /model_aliases/`).
    pub async fn model_aliases(&self) -> Result<serde_json::Value, AccountError> {
        self.get_json_retrying("/model_aliases/").await
    }

    /// Live per-chute TTFT/TPS table (`GET /invocations/stats/llm`).
    pub async fn invocation_stats(&self) -> Result<serde_json::Value, AccountError> {
        self.get_json_with_timeout(
            "/invocations/stats/llm",
            Some(std::time::Duration::from_secs(120)),
            true,
        )
        .await
    }

    async fn quota_usage_fallback(&self, quotas: &serde_json::Value) -> serde_json::Value {
        let requests =
            futures_util::stream::iter(quota_chute_ids(quotas).into_iter().take(100).map(
                |chute_id| async move {
                    let path = format!("/users/me/quota_usage/{}", encode_path_segment(&chute_id));
                    self.get_json(&path)
                        .await
                        .ok()
                        .map(|usage| (chute_id, usage))
                },
            ));
        let entries = requests
            .buffer_unordered(8)
            .filter_map(|entry| async move { entry })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<serde_json::Map<_, _>>();
        if entries.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::Object(entries)
        }
    }

    async fn get_json(&self, path: &str) -> Result<serde_json::Value, AccountError> {
        self.get_json_with_timeout(path, None, false).await
    }

    /// The live stats table is tens of megabytes; the client-wide 15 s deadline
    /// is right for account metadata but too short for this body.
    async fn get_json_with_timeout(
        &self,
        path: &str,
        timeout: Option<std::time::Duration>,
        identity_encoding: bool,
    ) -> Result<serde_json::Value, AccountError> {
        let mut request = self
            .http
            .get(format!("{}{}", self.endpoints.account, path))
            .header(
                AUTHORIZATION,
                self.credentials.management_authorization_header(),
            )
            .header(ACCEPT, "application/json");
        if identity_encoding {
            // The stats body is large and some edges answer with a compressed
            // encoding this client build cannot decode; ask for the raw bytes.
            request = request.header(ACCEPT_ENCODING, "identity");
        }
        if let Some(timeout) = timeout {
            request = request.timeout(timeout);
        }
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(AccountError::Http {
                status: status.as_u16(),
                body: body.chars().take(500).collect(),
            });
        }
        serde_json::from_str(&body).map_err(AccountError::Decode)
    }
}

fn transient_gateway(error: &AccountError) -> bool {
    matches!(
        error,
        AccountError::Http {
            status: 502 | 503 | 504,
            ..
        }
    )
}

fn has_quota_usage_data(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    has_number_like(object.get("used"))
        || has_number_like(object.get("quota"))
        || object.values().any(|entry| {
            entry.as_object().is_some_and(|entry| {
                has_number_like(entry.get("used")) || has_number_like(entry.get("quota"))
            })
        })
}

fn has_number_like(value: Option<&serde_json::Value>) -> bool {
    value.is_some_and(|value| {
        value.as_f64().is_some_and(f64::is_finite)
            || value
                .as_str()
                .and_then(|value| value.trim().parse::<f64>().ok())
                .is_some_and(f64::is_finite)
    })
}

fn quota_chute_ids(value: &serde_json::Value) -> Vec<String> {
    let items = value.as_array().or_else(|| {
        value
            .get("items")
            .and_then(serde_json::Value::as_array)
            .or_else(|| value.get("quotas").and_then(serde_json::Value::as_array))
    });
    let mut ids = items
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("chute_id")?.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn encode_path_segment(value: &str) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            write!(&mut encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    encoded
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error(transparent)]
    Credentials(#[from] crate::endpoints::CredentialError),
    #[error(transparent)]
    EndpointTrust(#[from] crate::endpoint_policy::EndpointTrustError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error("Chutes account API returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("Chutes account API returned invalid JSON: {0}")]
    Decode(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_errors_redact_credentials() {
        let error = AccountError::Http {
            status: 401,
            body: "unauthorized".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "Chutes account API returned HTTP 401: unauthorized"
        );
    }

    #[test]
    fn quota_usage_detection_accepts_aggregate_and_per_chute_shapes() {
        assert!(has_quota_usage_data(&serde_json::json!({
            "used": "11",
            "quota": 5000
        })));
        assert!(has_quota_usage_data(&serde_json::json!({
            "*": {"used": 11, "quota": 5000}
        })));
        assert!(!has_quota_usage_data(&serde_json::json!({})));
        assert!(!has_quota_usage_data(&serde_json::json!([])));
    }

    #[test]
    fn quota_ids_are_deduplicated_and_path_encoded_strictly() {
        let quotas = serde_json::json!({
            "items": [
                {"chute_id": "my chute"},
                {"chute_id": "*"},
                {"chute_id": "my chute"},
                {"quota": 100}
            ]
        });
        assert_eq!(quota_chute_ids(&quotas), vec!["*", "my chute"]);
        assert_eq!(encode_path_segment("*"), "%2A");
        assert_eq!(encode_path_segment("my chute"), "my%20chute");
    }

    #[test]
    fn from_credentials_builds_without_env() {
        let credentials =
            ChutesCredentials::from_key("cpk-test-key").expect("test key builds credentials");
        assert!(ChutesAccountClient::from_credentials(credentials).is_ok());
    }
}
