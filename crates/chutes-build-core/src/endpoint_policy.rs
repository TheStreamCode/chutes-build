//! Central, typed trust policy for Chutes endpoint URLs.
//!
//! [`ChutesEndpoints`](crate::ChutesEndpoints) resolves its inference/account/
//! router base URLs from environment variables (`CHUTES_INFERENCE_BASE_URL`,
//! `CHUTES_API_BASE_URL`, `CHUTES_ROUTER_BASE_URL`) so self-hosted forks and
//! local development can point at a different backend. Without validation,
//! that same override mechanism would let e.g.
//! `CHUTES_INFERENCE_BASE_URL=http://attacker.example` silently redirect
//! every Chutes API credential to an arbitrary host. This module is the
//! single place that decides whether a resolved endpoint URL is safe to send
//! a Chutes credential to (or, for the model catalog, safe to trust the
//! response of).
//!
//! Every credential-bearing client (`ChutesMediaClient`, `ChutesVisionClient`,
//! `ChutesAccountClient`) validates its endpoints once at construction time,
//! before any request is built.

const TRUSTED_HOSTS: &[&str] = &["chutes.ai", "model-router-ten.vercel.app"];

/// Env var that opts a fork or local dev setup out of the trusted-host
/// policy. Only affects *validation* of the configured base URL; it never
/// widens redirect handling or attaches a credential to a host this function
/// did not just approve.
pub const INSECURE_OPT_IN_VAR: &str = "CHUTES_ALLOW_INSECURE_ENDPOINTS";

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum EndpointTrustError {
    #[error("endpoint URL could not be parsed")]
    InvalidUrl,
    #[error("endpoint must not embed a username or password")]
    EmbeddedCredentials,
    #[error(
        "endpoint must use HTTPS (set {INSECURE_OPT_IN_VAR}=1 for local development or a fork)"
    )]
    InsecureScheme,
    #[error(
        "endpoint host is not a trusted Chutes host (set {INSECURE_OPT_IN_VAR}=1 for local \
         development or a fork)"
    )]
    UntrustedHost,
    #[error(
        "endpoint port {0} is not the default HTTPS port (set {INSECURE_OPT_IN_VAR}=1 for local \
         development or a fork)"
    )]
    UnexpectedPort(u16),
}

fn insecure_opt_in() -> bool {
    std::env::var(INSECURE_OPT_IN_VAR).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

fn is_trusted_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    TRUSTED_HOSTS
        .iter()
        .any(|trusted| host == *trusted || host.ends_with(&format!(".{trusted}")))
}

/// Validate that `url` is safe to use as a Chutes endpoint base.
///
/// Fails closed: parse errors, embedded userinfo, non-HTTPS schemes,
/// untrusted hosts, and non-default ports are all rejected unless
/// [`INSECURE_OPT_IN_VAR`] is set. Embedded credentials are rejected
/// unconditionally — the opt-in relaxes *trust*, not basic URL hygiene.
pub fn validate_endpoint_url(url: &str) -> Result<(), EndpointTrustError> {
    let parsed = url::Url::parse(url).map_err(|_| EndpointTrustError::InvalidUrl)?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(EndpointTrustError::EmbeddedCredentials);
    }
    if insecure_opt_in() {
        return Ok(());
    }
    if parsed.scheme() != "https" {
        return Err(EndpointTrustError::InsecureScheme);
    }
    let host = parsed.host_str().unwrap_or_default();
    if !is_trusted_host(host) {
        return Err(EndpointTrustError::UntrustedHost);
    }
    if let Some(port) = parsed.port() {
        return Err(EndpointTrustError::UnexpectedPort(port));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// `catalog.rs`'s tests also read/write [`INSECURE_OPT_IN_VAR`]; every
    /// test here that touches it is `#[serial]` (plain, no key -- same
    /// default group as catalog.rs) so the two modules' tests never race.
    fn with_insecure_opt_in<T>(value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let previous = std::env::var(INSECURE_OPT_IN_VAR).ok();
        // SAFETY: every caller of this helper is `#[serial]`, so no other
        // thread reads/writes process env vars while this runs.
        match value {
            Some(v) => unsafe { std::env::set_var(INSECURE_OPT_IN_VAR, v) },
            None => unsafe { std::env::remove_var(INSECURE_OPT_IN_VAR) },
        }
        let result = f();
        match previous {
            Some(v) => unsafe { std::env::set_var(INSECURE_OPT_IN_VAR, v) },
            None => unsafe { std::env::remove_var(INSECURE_OPT_IN_VAR) },
        }
        result
    }

    #[test]
    #[serial]
    fn accepts_chutes_ai_and_subdomains() {
        with_insecure_opt_in(None, || {
            assert!(validate_endpoint_url("https://chutes.ai").is_ok());
            assert!(validate_endpoint_url("https://api.chutes.ai").is_ok());
            assert!(validate_endpoint_url("https://llm.chutes.ai/v1").is_ok());
            assert!(validate_endpoint_url("https://chutes-qwen-embed.chutes.ai").is_ok());
        });
    }

    #[test]
    #[serial]
    fn accepts_exact_router_host() {
        with_insecure_opt_in(None, || {
            assert!(validate_endpoint_url("https://model-router-ten.vercel.app/v1").is_ok());
        });
    }

    #[test]
    #[serial]
    fn rejects_lookalike_domains() {
        with_insecure_opt_in(None, || {
            assert_eq!(
                validate_endpoint_url("https://chutes.ai.evil.example"),
                Err(EndpointTrustError::UntrustedHost)
            );
            assert_eq!(
                validate_endpoint_url("https://evilchutes.ai"),
                Err(EndpointTrustError::UntrustedHost)
            );
            assert_eq!(
                validate_endpoint_url("https://notmodel-router-ten.vercel.app"),
                Err(EndpointTrustError::UntrustedHost)
            );
            assert_eq!(
                validate_endpoint_url("https://model-router-ten.vercel.app.evil.example"),
                Err(EndpointTrustError::UntrustedHost)
            );
        });
    }

    #[test]
    #[serial]
    fn rejects_http_scheme() {
        with_insecure_opt_in(None, || {
            assert_eq!(
                validate_endpoint_url("http://chutes.ai"),
                Err(EndpointTrustError::InsecureScheme)
            );
        });
    }

    #[test]
    #[serial]
    fn rejects_embedded_userinfo_even_with_opt_in() {
        with_insecure_opt_in(Some("1"), || {
            assert_eq!(
                validate_endpoint_url("https://user:pass@chutes.ai"),
                Err(EndpointTrustError::EmbeddedCredentials)
            );
        });
    }

    #[test]
    #[serial]
    fn rejects_unexpected_port() {
        with_insecure_opt_in(None, || {
            assert_eq!(
                validate_endpoint_url("https://chutes.ai:8443"),
                Err(EndpointTrustError::UnexpectedPort(8443))
            );
        });
    }

    #[test]
    #[serial]
    fn rejects_invalid_url() {
        with_insecure_opt_in(None, || {
            assert_eq!(
                validate_endpoint_url("not a url"),
                Err(EndpointTrustError::InvalidUrl)
            );
        });
    }

    #[test]
    #[serial]
    fn empty_override_falls_back_to_default_and_is_accepted() {
        // `env_url()` already filters empty overrides back to the compiled
        // default before this function ever sees a URL; this test pins that
        // the default itself always validates cleanly.
        with_insecure_opt_in(None, || {
            assert!(validate_endpoint_url(crate::endpoints::INFERENCE_BASE_URL).is_ok());
            assert!(validate_endpoint_url(crate::endpoints::ACCOUNT_BASE_URL).is_ok());
            assert!(validate_endpoint_url(crate::endpoints::ROUTER_BASE_URL).is_ok());
        });
    }

    #[test]
    #[serial]
    fn insecure_opt_in_allows_http_and_loopback_for_local_dev() {
        with_insecure_opt_in(Some("1"), || {
            assert!(validate_endpoint_url("http://127.0.0.1:4010").is_ok());
            assert!(validate_endpoint_url("http://localhost:4010").is_ok());
        });
    }

    #[test]
    #[serial]
    fn insecure_opt_in_recognizes_true_case_insensitively() {
        with_insecure_opt_in(Some("TRUE"), || {
            assert!(validate_endpoint_url("http://127.0.0.1:4010").is_ok());
        });
        with_insecure_opt_in(Some("0"), || {
            assert_eq!(
                validate_endpoint_url("http://127.0.0.1:4010"),
                Err(EndpointTrustError::InsecureScheme)
            );
        });
    }
}
