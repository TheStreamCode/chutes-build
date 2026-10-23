//! Chutes media catalog and invocation client.
//!
//! [`ChutesMediaClient::list`]/[`describe`](ChutesMediaClient::describe)
//! deliberately return raw [`serde_json::Value`] rather than typed structs:
//! each "chute" publishes its own arbitrary JSON schema for its invocation
//! parameters and metadata (see the schema-manipulation helpers in
//! `chutes-tools`' `implementations::chutes::media`, which walk this data
//! generically to encode workspace assets and enforce `additionalProperties`
//! regardless of which cord it came from). A fixed struct here would fight
//! that pass-through design instead of serving it. [`MediaResponse`] — the
//! actual invocation *result* (bytes, content type, cost) — is typed, since
//! that shape is ours to define and is security-relevant (size/URL/host
//! validation all happen before this struct is built).

use futures_util::StreamExt as _;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::{ChutesCredentials, ChutesEndpoints};

/// The account endpoint caps a page at 100 records; the public catalogue is
/// several hundred chutes, so the client walks every page.
const CATALOG_PAGE_SIZE: &str = "100";
const MAX_CATALOG_PAGES: usize = 20;

/// Merge paginated `/chutes/` responses into one `{items, total}` body,
/// de-duplicated by chute id. Pure so the pagination contract is testable.
fn combine_catalog_pages(pages: Vec<serde_json::Value>) -> serde_json::Value {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut total = 0u64;
    for page in pages {
        total = page
            .get("total")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(total)
            .max(total);
        let Some(page_items) = page.get("items").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for item in page_items {
            let key = item
                .get("chute_id")
                .and_then(serde_json::Value::as_str)
                .or_else(|| item.get("slug").and_then(serde_json::Value::as_str))
                .or_else(|| item.get("name").and_then(serde_json::Value::as_str))
                .unwrap_or_default()
                .to_owned();
            if seen.insert(key) {
                items.push(item.clone());
            }
        }
    }
    total = total.max(items.len() as u64);
    serde_json::json!({ "items": items, "total": total })
}

#[derive(Debug, Clone)]
pub struct ChutesMediaClient {
    http: reqwest::Client,
    endpoints: ChutesEndpoints,
    credentials: ChutesCredentials,
}

impl ChutesMediaClient {
    // WHY-ALLOW: product-policy client: the SSRF-safe resolver is the security boundary here; extra-CA support for these clients is tracked separately
    #[allow(clippy::disallowed_methods)]
    pub fn from_env() -> Result<Self, MediaError> {
        Self::from_credentials(ChutesCredentials::from_env()?)
    }

    /// Client for explicitly resolved credentials: tool dispatch publishes
    /// the agent's runtime key on the call context, which plain `from_env`
    /// cannot see.
    // WHY-ALLOW: same client as `from_env`; see the note there.
    #[allow(clippy::disallowed_methods)]
    pub fn from_credentials(credentials: ChutesCredentials) -> Result<Self, MediaError> {
        let endpoints = ChutesEndpoints::default();
        endpoints.validate()?;
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .redirect(reqwest::redirect::Policy::none())
                .dns_resolver(std::sync::Arc::new(
                    crate::endpoint_policy::SsrfSafeResolver::default(),
                ))
                .build()?,
            endpoints,
            credentials,
        })
    }

    /// Every public chute, across all API pages.
    ///
    /// The account endpoint returns 25 records per page by default, and media
    /// models are spread over the whole catalogue; a single-page listing silently
    /// hides most of them (which made the old fixed model ids the only option).
    pub async fn list(&self) -> Result<serde_json::Value, MediaError> {
        let url = format!("{}/chutes/", self.endpoints.account);
        let mut pages = Vec::new();
        let mut seen_total = 0usize;
        let mut total = None;
        for page in 0..MAX_CATALOG_PAGES {
            let page = page.to_string();
            let body = self
                .send_json(self.http.get(&url).query(&[
                    ("include_public", "true"),
                    ("include_schemas", "false"),
                    ("limit", CATALOG_PAGE_SIZE),
                    ("page", page.as_str()),
                ]))
                .await?;
            seen_total += body
                .get("items")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            total = body
                .get("total")
                .and_then(serde_json::Value::as_u64)
                .or(total);
            let empty = body
                .get("items")
                .and_then(serde_json::Value::as_array)
                .is_none_or(Vec::is_empty);
            pages.push(body);
            if empty || total.is_some_and(|total| seen_total as u64 >= total) {
                break;
            }
        }
        Ok(combine_catalog_pages(pages))
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
enum MediaBody {
    Bytes(Vec<u8>),
    TempFile {
        path: tempfile::TempPath,
        byte_len: u64,
    },
}

#[derive(Debug)]
pub struct MediaResponse {
    body: MediaBody,
    pub content_type: String,
    pub cost: Option<f64>,
}

impl MediaResponse {
    pub fn from_bytes(bytes: Vec<u8>, content_type: String, cost: Option<f64>) -> Self {
        Self {
            body: MediaBody::Bytes(bytes),
            content_type,
            cost,
        }
    }

    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.body {
            MediaBody::Bytes(bytes) => Some(bytes),
            MediaBody::TempFile { .. } => None,
        }
    }

    pub fn byte_len(&self) -> u64 {
        match &self.body {
            MediaBody::Bytes(bytes) => bytes.len() as u64,
            MediaBody::TempFile { byte_len, .. } => *byte_len,
        }
    }

    pub async fn read_prefix(&self, limit: usize) -> Result<Vec<u8>, std::io::Error> {
        match &self.body {
            MediaBody::Bytes(bytes) => Ok(bytes[..bytes.len().min(limit)].to_vec()),
            MediaBody::TempFile { path, .. } => {
                let mut file = tokio::fs::File::open(path).await?;
                let mut bytes = vec![0; limit];
                let read = file.read(&mut bytes).await?;
                bytes.truncate(read);
                Ok(bytes)
            }
        }
    }

    /// Copy the response into a newly-created destination without ever holding
    /// a streamed binary response in memory or overwriting an existing file.
    pub async fn write_new(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let mut output = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .await?;
        let result = match &self.body {
            MediaBody::Bytes(bytes) => output.write_all(bytes).await,
            MediaBody::TempFile { path, .. } => match tokio::fs::File::open(path).await {
                Ok(mut input) => tokio::io::copy(&mut input, &mut output).await.map(|_| ()),
                Err(error) => Err(error),
            },
        };
        if result.is_ok() {
            output.flush().await?;
        } else {
            drop(output);
            let _ = tokio::fs::remove_file(path).await;
        }
        result
    }
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
    let collect_in_memory =
        !status.is_success() || content_type.to_ascii_lowercase().contains("json");
    let response_limit = if collect_in_memory {
        max_bytes.min(32 * 1024 * 1024)
    } else {
        max_bytes
    };
    if response
        .content_length()
        .is_some_and(|length| length > response_limit as u64)
    {
        return Err(MediaError::TooLarge {
            max_bytes: response_limit,
        });
    }
    let content_length = response.content_length().unwrap_or_default();
    let mut stream = response.bytes_stream();
    if collect_in_memory {
        let mut bytes = Vec::with_capacity(content_length.min(response_limit as u64) as usize);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if bytes.len().saturating_add(chunk.len()) > response_limit {
                return Err(MediaError::TooLarge {
                    max_bytes: response_limit,
                });
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            return Err(MediaError::Http {
                status: status.as_u16(),
                body: String::from_utf8_lossy(&bytes).chars().take(500).collect(),
            });
        }
        return Ok(MediaResponse::from_bytes(bytes, content_type, cost));
    }

    let (temp_file, temp_path) = tempfile::NamedTempFile::new()?.into_parts();
    let mut temp_file = tokio::fs::File::from_std(temp_file);
    let mut byte_len = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        byte_len = byte_len.saturating_add(chunk.len() as u64);
        if byte_len > response_limit as u64 {
            return Err(MediaError::TooLarge {
                max_bytes: response_limit,
            });
        }
        temp_file.write_all(&chunk).await?;
    }
    temp_file.flush().await?;
    Ok(MediaResponse {
        body: MediaBody::TempFile {
            path: temp_path,
            byte_len,
        },
        content_type,
        cost,
    })
}

fn max_media_bytes() -> usize {
    const DEFAULT: usize = 128 * 1024 * 1024;
    const MAX: usize = 512 * 1024 * 1024;
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
    #[error("failed to stage Chutes media response: {0}")]
    Io(#[from] std::io::Error),
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
    #[error("unexpected Chutes response: {0}")]
    UnexpectedResponse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_pages_merge_and_dedupe() {
        let pages = vec![
            serde_json::json!({
                "total": 3,
                "items": [
                    {"chute_id": "a", "slug": "a"},
                    {"chute_id": "b", "slug": "b"},
                ]
            }),
            serde_json::json!({
                "total": 3,
                "items": [
                    {"chute_id": "b", "slug": "b"},
                    {"chute_id": "c", "slug": "c"},
                ]
            }),
        ];
        let merged = combine_catalog_pages(pages);
        let items = merged["items"].as_array().expect("items array");
        assert_eq!(items.len(), 3, "dedupe by chute id: {items:?}");
        assert_eq!(merged["total"], serde_json::json!(3));
    }

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

    #[test]
    fn from_credentials_builds_without_env() {
        let credentials =
            ChutesCredentials::from_key("cpk-test-key").expect("test key builds credentials");
        assert!(ChutesMediaClient::from_credentials(credentials).is_ok());
    }
}
