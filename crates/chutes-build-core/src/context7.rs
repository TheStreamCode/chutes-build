//! Native Context7 REST client used for version-aware coding documentation.

const DEFAULT_BASE_URL: &str = "https://context7.com/api/v2";
const CONTEXT7_HOST: &str = "context7.com";
pub const CONTEXT7_INSECURE_OPT_IN_VAR: &str = "CONTEXT7_ALLOW_INSECURE_ENDPOINTS";

#[derive(Debug, Clone)]
pub struct Context7Client {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for Context7Client {
    fn default() -> Self {
        Self::new(
            std::env::var("CONTEXT7_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned()),
            std::env::var("CONTEXT7_API_KEY").ok(),
        )
    }
}

impl Context7Client {
    // WHY-ALLOW: product-policy client: the SSRF-safe resolver is the security boundary here; extra-CA support for these clients is tracked separately
    #[allow(clippy::disallowed_methods)]
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        let base_url = base_url.trim_end_matches('/').to_owned();
        let send_api_key = is_official_context7_url(&base_url);
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .dns_resolver(std::sync::Arc::new(
                    crate::endpoint_policy::SsrfSafeResolver::with_insecure_opt_in_var(
                        CONTEXT7_INSECURE_OPT_IN_VAR,
                    ),
                ))
                .build()
                .expect("Context7 HTTP client configuration is valid"),
            base_url,
            // Ambient credentials are never forwarded to custom endpoints,
            // even when the explicit local-development opt-in is enabled.
            api_key: send_api_key
                .then_some(api_key)
                .flatten()
                .filter(|key| !key.trim().is_empty()),
        }
    }

    pub async fn search_libraries(
        &self,
        library_name: &str,
        query: &str,
    ) -> Result<serde_json::Value, Context7Error> {
        validate_context7_base_url(&self.base_url)?;
        let url = format!("{}/libs/search", self.base_url);
        self.get_json(
            self.http
                .get(url)
                .query(&[("libraryName", library_name), ("query", query)]),
        )
        .await
    }

    pub async fn get_context(
        &self,
        library_id: &str,
        query: &str,
        tokens: Option<u32>,
    ) -> Result<serde_json::Value, Context7Error> {
        validate_context7_base_url(&self.base_url)?;
        if !library_id.starts_with('/') || library_id.chars().any(char::is_whitespace) {
            return Err(Context7Error::InvalidLibraryId);
        }
        let url = format!("{}/context", self.base_url);
        let mut params = vec![
            ("libraryId", library_id.to_owned()),
            ("query", query.to_owned()),
        ];
        if let Some(tokens) = tokens {
            params.push(("tokens", tokens.clamp(1_000, 20_000).to_string()));
        }
        self.get_json(self.http.get(url).query(&params)).await
    }

    async fn get_json(
        &self,
        mut request: reqwest::RequestBuilder,
    ) -> Result<serde_json::Value, Context7Error> {
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(Context7Error::Http {
                status: status.as_u16(),
                body: body.chars().take(500).collect(),
            });
        }
        serde_json::from_str(&body).map_err(Context7Error::Decode)
    }
}

fn is_official_context7_url(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case(CONTEXT7_HOST)
                || host
                    .to_ascii_lowercase()
                    .ends_with(&format!(".{CONTEXT7_HOST}"))
        })
}

fn validate_context7_base_url(raw: &str) -> Result<(), Context7Error> {
    let url = url::Url::parse(raw).map_err(|_| Context7Error::InvalidEndpoint)?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Context7Error::EmbeddedCredentials);
    }
    if is_official_context7_url(raw) {
        return Ok(());
    }
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Context7Error::InvalidEndpoint);
    }
    if crate::endpoint_policy::env_opt_in(CONTEXT7_INSECURE_OPT_IN_VAR) {
        return Ok(());
    }
    Err(Context7Error::UntrustedEndpoint)
}

#[derive(Debug, thiserror::Error)]
pub enum Context7Error {
    #[error("invalid Context7 endpoint URL")]
    InvalidEndpoint,
    #[error("Context7 endpoint must not embed a username or password")]
    EmbeddedCredentials,
    #[error(
        "custom Context7 endpoints require {CONTEXT7_INSECURE_OPT_IN_VAR}=1; ambient API keys are sent only to official Context7 HTTPS hosts"
    )]
    UntrustedEndpoint,
    #[error("invalid Context7 library id")]
    InvalidLibraryId,
    #[error("Context7 request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Context7 returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("Context7 returned invalid JSON: {0}")]
    Decode(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_context7_urls_are_strictly_scoped() {
        assert!(is_official_context7_url(DEFAULT_BASE_URL));
        assert!(is_official_context7_url("https://api.context7.com/v2"));
        assert!(!is_official_context7_url("http://context7.com/api/v2"));
        assert!(!is_official_context7_url(
            "https://context7.com.evil.test/api/v2"
        ));
        assert!(!is_official_context7_url(
            "https://context7.com:8443/api/v2"
        ));
        assert!(!is_official_context7_url(
            "https://user:pass@context7.com/api/v2"
        ));
    }

    #[test]
    fn ambient_key_is_discarded_for_custom_endpoint() {
        let client = Context7Client::new(
            "https://docs.example.test/api/v2".to_owned(),
            Some("secret".to_owned()),
        );
        assert!(client.api_key.is_none());
    }
}
