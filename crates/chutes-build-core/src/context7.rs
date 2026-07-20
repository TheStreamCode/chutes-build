//! Native Context7 REST client used for version-aware coding documentation.

const DEFAULT_BASE_URL: &str = "https://context7.com/api/v2";

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
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Context7 HTTP client configuration is valid"),
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key: api_key.filter(|key| !key.trim().is_empty()),
        }
    }

    pub async fn search_libraries(
        &self,
        library_name: &str,
        query: &str,
    ) -> Result<serde_json::Value, Context7Error> {
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

#[derive(Debug, thiserror::Error)]
pub enum Context7Error {
    #[error("invalid Context7 library id")]
    InvalidLibraryId,
    #[error("Context7 request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Context7 returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("Context7 returned invalid JSON: {0}")]
    Decode(serde_json::Error),
}
