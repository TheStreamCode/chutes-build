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

/// DNS resolver that refuses to hand back private/loopback/link-local/
/// reserved addresses, so an outbound request to an attacker-influenced
/// hostname (e.g. a URL embedded in a chute's response) can't reach the
/// internal network -- including via DNS rebinding, since the check runs on
/// the exact addresses the connection is about to use, not a separate
/// earlier lookup that could go stale before the connect happens.
///
/// Wraps the system resolver (`tokio::net::lookup_host`, the same
/// `getaddrinfo`-backed resolution reqwest's default resolver would use)
/// rather than implementing DNS itself.
///
/// Respects [`INSECURE_OPT_IN_VAR`], same as [`validate_endpoint_url`] --
/// otherwise a local dev/fork setup that opted a `127.0.0.1` endpoint past
/// URL validation would still get blocked here, at the DNS layer.
#[derive(Debug, Clone, Default)]
pub struct SsrfSafeResolver;

impl reqwest::dns::Resolve for SsrfSafeResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("DNS resolution failed for {host}: {error}").into()
                })?
                .collect();
            if addrs.is_empty() {
                return Err(format!("no addresses resolved for {host}").into());
            }
            if insecure_opt_in() {
                return Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs);
            }
            if let Some(blocked) = addrs.iter().find(|addr| is_blocked_address(addr.ip())) {
                return Err(format!(
                    "refusing to connect to {host}: resolves to {} \
                     (private/loopback/link-local/reserved address)",
                    blocked.ip()
                )
                .into());
            }
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

fn is_blocked_address(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_documentation()
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || is_ipv6_unique_local(&v6)
                || is_ipv6_link_local(&v6)
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_blocked_address(std::net::IpAddr::V4(v4)))
        }
    }
}

/// `fc00::/7` -- IPv6 unique local addresses (the IPv6 analog of
/// RFC1918 private ranges). Not yet covered by a stable `Ipv6Addr` method.
fn is_ipv6_unique_local(ip: &std::net::Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

/// `fe80::/10` -- IPv6 link-local addresses. Not yet covered by a stable
/// `Ipv6Addr` method.
fn is_ipv6_link_local(ip: &std::net::Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
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

    #[test]
    fn blocks_loopback_addresses() {
        assert!(is_blocked_address("127.0.0.1".parse().unwrap()));
        assert!(is_blocked_address("::1".parse().unwrap()));
    }

    #[test]
    fn blocks_private_ipv4_ranges() {
        for ip in ["10.0.0.1", "172.16.0.1", "192.168.1.1"] {
            assert!(
                is_blocked_address(ip.parse().unwrap()),
                "{ip} should be blocked"
            );
        }
    }

    #[test]
    fn blocks_link_local_addresses() {
        assert!(is_blocked_address("169.254.1.1".parse().unwrap()));
        assert!(is_blocked_address("fe80::1".parse().unwrap()));
    }

    #[test]
    fn blocks_unspecified_and_multicast() {
        assert!(is_blocked_address("0.0.0.0".parse().unwrap()));
        assert!(is_blocked_address("::".parse().unwrap()));
        assert!(is_blocked_address("224.0.0.1".parse().unwrap()));
        assert!(is_blocked_address("ff02::1".parse().unwrap()));
    }

    #[test]
    fn blocks_ipv6_unique_local_addresses() {
        assert!(is_blocked_address("fc00::1".parse().unwrap()));
        assert!(is_blocked_address("fd12:3456:789a::1".parse().unwrap()));
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6_private_address() {
        // ::ffff:10.0.0.1 -- an IPv6-mapped view of a private IPv4 address;
        // must be blocked via the same policy as the plain IPv4 form.
        assert!(is_blocked_address("::ffff:10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn allows_public_addresses() {
        assert!(!is_blocked_address("8.8.8.8".parse().unwrap()));
        assert!(!is_blocked_address("2001:4860:4860::8888".parse().unwrap()));
    }

    #[tokio::test]
    #[serial]
    async fn resolver_rejects_a_hostname_that_only_resolves_to_loopback() {
        use reqwest::dns::Resolve as _;
        // SAFETY: gated behind #[serial].
        unsafe {
            std::env::remove_var(INSECURE_OPT_IN_VAR);
        }
        let resolver = SsrfSafeResolver;
        let name: reqwest::dns::Name = "localhost".parse().unwrap();
        let result = resolver.resolve(name).await;
        assert!(
            result.is_err(),
            "localhost must not resolve for outbound requests"
        );
    }

    #[tokio::test]
    #[serial]
    async fn resolver_allows_loopback_when_insecure_opt_in_is_set() {
        use reqwest::dns::Resolve as _;
        // SAFETY: gated behind #[serial].
        unsafe {
            std::env::set_var(INSECURE_OPT_IN_VAR, "1");
        }
        let resolver = SsrfSafeResolver;
        let name: reqwest::dns::Name = "localhost".parse().unwrap();
        let result = resolver.resolve(name).await;
        unsafe {
            std::env::remove_var(INSECURE_OPT_IN_VAR);
        }
        assert!(
            result.is_ok(),
            "opt-in must allow local dev/test endpoints to resolve"
        );
    }
}
