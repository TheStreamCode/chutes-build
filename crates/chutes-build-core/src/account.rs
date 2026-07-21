//! Read-only access to Chutes account usage endpoints.

use futures_util::StreamExt as _;
use reqwest::header::{ACCEPT, AUTHORIZATION};

use crate::{ChutesCredentials, ChutesEndpoints};

#[derive(Debug, Clone)]
pub struct ChutesAccountClient {
    http: reqwest::Client,
    endpoints: ChutesEndpoints,
    credentials: ChutesCredentials,
}

impl ChutesAccountClient {
    pub fn from_env() -> Result<Self, AccountError> {
        let endpoints = ChutesEndpoints::default();
        endpoints.validate()?;
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            endpoints,
            credentials: ChutesCredentials::from_env()?,
        })
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
        let response = self
            .http
            .get(format!("{}{}", self.endpoints.account, path))
            .header(
                AUTHORIZATION,
                self.credentials.management_authorization_header(),
            )
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
}
