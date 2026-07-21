//! Chutes media catalog and invocation client.

use futures_util::StreamExt as _;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};

use crate::{ChutesCredentials, ChutesEndpoints};

#[derive(Debug, Clone)]
pub struct ChutesMediaClient {
    http: reqwest::Client,
    endpoints: ChutesEndpoints,
    credentials: ChutesCredentials,
}

impl ChutesMediaClient {
    pub fn from_env() -> Result<Self, MediaError> {
        let endpoints = ChutesEndpoints::default();
        endpoints.validate()?;
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .redirect(reqwest::redirect::Policy::none())
                .dns_resolver(std::sync::Arc::new(
                    crate::endpoint_policy::SsrfSafeResolver,
                ))
                .build()?,
            endpoints,
            credentials: ChutesCredentials::from_env()?,
        })
    }

    pub async fn list(&self) -> Result<serde_json::Value, MediaError> {
        let url = format!("{}/chutes/", self.endpoints.account);
        self.send_json(
            self.http
                .get(url)
                .query(&[("include_public", "true"), ("include_schemas", "false")]),
        )
        .await
    }

    pub async fn describe(&self, model: &str) -> Result<serde_json::Value, MediaError> {
        let encoded = urlencoding::encode(model);
        let url = format!("{}/chutes/{encoded}", self.endpoints.account);
        self.send_json(self.http.get(url)).await
    }

    pub async fn warmup(&self, model: &str) -> Result<(), MediaError> {
        let encoded = urlencoding::encode(model);
        let url = format!("{}/chutes/warmup/{encoded}", self.endpoints.account);
        self.send_json(self.http.get(url).query(&[("quick", "true")]))
            .await
            .map(|_| ())
    }

    pub async fn invoke(
        &self,
        url: &str,
        method: &str,
        body: &serde_json::Value,
    ) -> Result<MediaResponse, MediaError> {
        let url = validate_chutes_invocation_url(url)?;
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| MediaError::InvalidMethod(method.to_owned()))?;
        let response = self
            .http
            .request(method, url)
            .header(
                AUTHORIZATION,
                self.credentials.management_authorization_header(),
            )
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "*/*")
            .json(body)
            .send()
            .await?;
        response_to_media(response).await
    }

    pub async fn download(&self, url: &str) -> Result<MediaResponse, MediaError> {
        let url = validate_download_url(url)?;
        let mut request = self.http.get(url.clone()).header(ACCEPT, "*/*");
        if is_chutes_url(&url) {
            request = request.header(
                AUTHORIZATION,
                self.credentials.management_authorization_header(),
            );
        }
        response_to_media(request.send().await?).await
    }

    async fn send_json(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<serde_json::Value, MediaError> {
        let response = request
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
            return Err(MediaError::Http {
                status: status.as_u16(),
                body: body.chars().take(500).collect(),
            });
        }
        serde_json::from_str(&body).map_err(MediaError::Decode)
    }
}

fn validate_chutes_invocation_url(raw: &str) -> Result<url::Url, MediaError> {
    let url = validate_download_url(raw)?;
    if !is_chutes_url(&url) {
        return Err(MediaError::UntrustedInvocationUrl);
    }
    Ok(url)
}

fn validate_download_url(raw: &str) -> Result<url::Url, MediaError> {
    let url = url::Url::parse(raw).map_err(|_| MediaError::InvalidUrl)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(MediaError::InvalidUrl);
    }
    match url.host() {
        Some(url::Host::Domain(host))
            if !host.eq_ignore_ascii_case("localhost")
                && !host.to_ascii_lowercase().ends_with(".localhost") =>
        {
            Ok(url)
        }
        _ => Err(MediaError::InvalidUrl),
    }
}

fn is_chutes_url(url: &url::Url) -> bool {
    url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("chutes.ai") || host.to_ascii_lowercase().ends_with(".chutes.ai")
    })
}

#[derive(Debug)]
pub struct MediaResponse {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub cost: Option<f64>,
}

async fn response_to_media(response: reqwest::Response) -> Result<MediaResponse, MediaError> {
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let cost = [
        "x-chutes-cost",
        "x-cost",
        "x-compute-units",
        "x-chutes-compute-units",
    ]
    .into_iter()
    .find_map(|name| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
    });
    let max_bytes = max_media_bytes();
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(MediaError::TooLarge { max_bytes });
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(max_bytes as u64) as usize,
    );
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(MediaError::TooLarge { max_bytes });
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        return Err(MediaError::Http {
            status: status.as_u16(),
            body: String::from_utf8_lossy(&bytes).chars().take(500).collect(),
        });
    }
    Ok(MediaResponse {
        bytes,
        content_type,
        cost,
    })
}

fn max_media_bytes() -> usize {
    const DEFAULT: usize = 512 * 1024 * 1024;
    const MAX: usize = 2 * 1024 * 1024 * 1024;
    std::env::var("CHUTES_MAX_MEDIA_BYTES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map_or(DEFAULT, |value| value.clamp(1, MAX))
}

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error(transparent)]
    Credentials(#[from] crate::endpoints::CredentialError),
    #[error(transparent)]
    EndpointTrust(#[from] crate::endpoint_policy::EndpointTrustError),
    #[error("Chutes media request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Chutes returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("Chutes returned invalid JSON: {0}")]
    Decode(serde_json::Error),
    #[error("invalid HTTP method: {0}")]
    InvalidMethod(String),
    #[error("invalid media URL; only credential-free HTTPS URLs with DNS hostnames are allowed")]
    InvalidUrl,
    #[error("untrusted media invocation URL; API credentials are sent only to Chutes HTTPS hosts")]
    UntrustedInvocationUrl,
    #[error("media response exceeds the configured {max_bytes}-byte safety limit")]
    TooLarge { max_bytes: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invocation_credentials_are_restricted_to_chutes_https_hosts() {
        assert!(validate_chutes_invocation_url("https://demo.chutes.ai/generate").is_ok());
        assert!(validate_chutes_invocation_url("https://chutes.ai/generate").is_ok());
        assert!(validate_chutes_invocation_url("https://chutes.ai.evil.example/generate").is_err());
        assert!(validate_chutes_invocation_url("http://demo.chutes.ai/generate").is_err());
        assert!(validate_chutes_invocation_url("https://example.com/generate").is_err());
    }

    #[test]
    fn downloads_reject_local_and_credentialed_urls() {
        assert!(validate_download_url("https://cdn.example.com/asset.png").is_ok());
        assert!(validate_download_url("https://localhost/asset.png").is_err());
        assert!(validate_download_url("https://127.0.0.1/asset.png").is_err());
        assert!(validate_download_url("https://user:pass@example.com/asset.png").is_err());
    }
}
