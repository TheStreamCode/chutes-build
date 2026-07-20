//! Canonical Chutes endpoints and credential resolution.

use std::borrow::Cow;

pub const INFERENCE_BASE_URL: &str = "https://llm.chutes.ai/v1";
pub const ACCOUNT_BASE_URL: &str = "https://api.chutes.ai";
pub const ROUTER_BASE_URL: &str = "https://model-router-ten.vercel.app/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChutesEndpoints {
    pub inference: String,
    pub account: String,
    pub router: String,
}

impl Default for ChutesEndpoints {
    fn default() -> Self {
        Self {
            inference: env_url("CHUTES_INFERENCE_BASE_URL", INFERENCE_BASE_URL),
            account: env_url("CHUTES_API_BASE_URL", ACCOUNT_BASE_URL),
            router: env_url("CHUTES_ROUTER_BASE_URL", ROUTER_BASE_URL),
        }
    }
}

fn env_url(name: &str, fallback: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    Raw,
    Bearer,
}

impl AuthScheme {
    pub fn from_env() -> Self {
        match std::env::var("CHUTES_AUTH_SCHEME") {
            Ok(value) if value.eq_ignore_ascii_case("raw") => Self::Raw,
            _ => Self::Bearer,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChutesCredentials {
    api_key: String,
    pub management_scheme: AuthScheme,
}

impl std::fmt::Debug for ChutesCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChutesCredentials")
            .field("api_key", &"[REDACTED]")
            .field("management_scheme", &self.management_scheme)
            .finish()
    }
}

impl ChutesCredentials {
    pub fn from_env() -> Result<Self, CredentialError> {
        let api_key = std::env::var("CHUTES_API_KEY")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .ok_or(CredentialError::Missing)?;
        Ok(Self {
            api_key,
            management_scheme: AuthScheme::from_env(),
        })
    }

    pub fn inference_authorization(&self) -> Cow<'_, str> {
        Cow::Owned(format!("Bearer {}", self.api_key))
    }

    pub fn management_authorization(&self) -> Cow<'_, str> {
        match self.management_scheme {
            AuthScheme::Raw => Cow::Borrowed(&self.api_key),
            AuthScheme::Bearer => Cow::Owned(format!("Bearer {}", self.api_key)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("CHUTES_API_KEY is not set; configure it in the environment before using Chutes")]
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_have_no_trailing_slash() {
        let endpoints = ChutesEndpoints::default();
        assert!(!endpoints.inference.ends_with('/'));
        assert!(!endpoints.account.ends_with('/'));
        assert!(!endpoints.router.ends_with('/'));
    }

    #[test]
    fn credential_debug_output_is_redacted() {
        let credentials = ChutesCredentials {
            api_key: "fixture-credential".to_owned(),
            management_scheme: AuthScheme::Raw,
        };
        let debug = format!("{credentials:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("fixture-credential"));
    }
}
