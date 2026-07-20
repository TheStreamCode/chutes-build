use super::types::WebSearchConfig;
use crate::types::SharedApiKeyProvider;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::time::Duration;

const SEARCH_ID: &str = "web_search";
const DEFAULT_RESULT_LIMIT: usize = 8;

#[derive(Clone, Debug)]
enum SearchProvider {
    Brave { api_key: String },
    DuckDuckGo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Privacy-first native web search. Chutes API credentials are never sent to
/// third-party search providers. Brave is used only when the user configured a
/// dedicated `BRAVE_SEARCH_API_KEY`; otherwise the client uses DuckDuckGo HTML.
#[derive(Clone)]
pub struct WebSearchClient {
    http: reqwest::Client,
    provider: SearchProvider,
}

impl WebSearchClient {
    pub fn new(
        config: &WebSearchConfig,
        _api_key_provider: Option<SharedApiKeyProvider>,
    ) -> Result<Self, xai_tool_runtime::ToolError> {
        if matches!(config, WebSearchConfig::Disabled) {
            return Err(tool_error("Web search is disabled"));
        }

        let requested = std::env::var("CHUTES_WEB_SEARCH_PROVIDER")
            .unwrap_or_else(|_| "auto".to_owned())
            .to_ascii_lowercase();
        let brave_key = std::env::var("BRAVE_SEARCH_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let provider = match (requested.as_str(), brave_key) {
            ("brave", Some(api_key)) | ("auto", Some(api_key)) => SearchProvider::Brave { api_key },
            ("duckduckgo" | "ddg" | "auto", _) => SearchProvider::DuckDuckGo,
            ("brave", None) => {
                return Err(tool_error(
                    "CHUTES_WEB_SEARCH_PROVIDER=brave requires BRAVE_SEARCH_API_KEY",
                ));
            }
            (other, _) => {
                return Err(tool_error(format!(
                    "Unsupported CHUTES_WEB_SEARCH_PROVIDER '{other}'; use auto, brave, or duckduckgo"
                )));
            }
        };

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("chutes-build-web-search/1")
            .build()
            .map_err(|error| tool_error(format!("Failed to build web search client: {error}")))?;
        Ok(Self { http, provider })
    }

    /// Retained for upstream constructor compatibility. Native search does not
    /// emit auth-attribution telemetry.
    pub fn with_attribution_callback(
        self,
        _callback: Option<crate::attribution::SharedAttributionCallback>,
    ) -> Self {
        self
    }

    pub async fn search(
        &self,
        query: &str,
        allowed_domains: Option<Vec<String>>,
    ) -> Result<(String, Vec<String>), xai_tool_runtime::ToolError> {
        let results = self
            .search_results(query, allowed_domains.as_deref())
            .await?;
        let citations = results.iter().map(|result| result.url.clone()).collect();
        Ok((format_results(&results), citations))
    }

    pub async fn search_with_titles(
        &self,
        query: &str,
        allowed_domains: Option<Vec<String>>,
    ) -> Result<(String, Vec<(String, String)>), xai_tool_runtime::ToolError> {
        let results = self
            .search_results(query, allowed_domains.as_deref())
            .await?;
        let citations = results
            .iter()
            .map(|result| (result.title.clone(), result.url.clone()))
            .collect();
        Ok((format_results(&results), citations))
    }

    async fn search_results(
        &self,
        query: &str,
        allowed_domains: Option<&[String]>,
    ) -> Result<Vec<SearchResult>, xai_tool_runtime::ToolError> {
        let query = scoped_query(query, allowed_domains);
        let results = match &self.provider {
            SearchProvider::Brave { api_key } => self.search_brave(&query, api_key).await?,
            SearchProvider::DuckDuckGo => self.search_duckduckgo(&query).await?,
        };
        Ok(filter_domains(results, allowed_domains))
    }

    async fn search_brave(
        &self,
        query: &str,
        api_key: &str,
    ) -> Result<Vec<SearchResult>, xai_tool_runtime::ToolError> {
        let response = self
            .http
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", api_key)
            .query(&[("q", query), ("count", &DEFAULT_RESULT_LIMIT.to_string())])
            .send()
            .await
            .map_err(|error| tool_error(format!("Brave Search request failed: {error}")))?;
        let status = response.status();
        let body = response.text().await.map_err(|error| {
            tool_error(format!("Failed to read Brave Search response: {error}"))
        })?;
        if !status.is_success() {
            return Err(tool_error(format!("Brave Search returned HTTP {status}")));
        }
        parse_brave_results(&body)
    }

    async fn search_duckduckgo(
        &self,
        query: &str,
    ) -> Result<Vec<SearchResult>, xai_tool_runtime::ToolError> {
        let response = self
            .http
            .get("https://html.duckduckgo.com/html/")
            .query(&[("q", query)])
            .send()
            .await
            .map_err(|error| tool_error(format!("DuckDuckGo search request failed: {error}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| tool_error(format!("Failed to read DuckDuckGo response: {error}")))?;
        if !status.is_success() {
            return Err(tool_error(format!(
                "DuckDuckGo search returned HTTP {status}"
            )));
        }
        parse_duckduckgo_results(&body)
    }
}

fn scoped_query(query: &str, domains: Option<&[String]>) -> String {
    let sites = domains
        .unwrap_or_default()
        .iter()
        .filter_map(|domain| normalized_domain(domain))
        .map(|domain| format!("site:{domain}"))
        .collect::<Vec<_>>();
    if sites.is_empty() {
        query.to_owned()
    } else {
        format!("{query} ({})", sites.join(" OR "))
    }
}

fn normalized_domain(input: &str) -> Option<String> {
    let candidate = input.trim().trim_start_matches("*.");
    let parsed = if candidate.contains("://") {
        reqwest::Url::parse(candidate).ok()?
    } else {
        reqwest::Url::parse(&format!("https://{candidate}")).ok()?
    };
    parsed.host_str().map(|host| host.to_ascii_lowercase())
}

fn filter_domains(results: Vec<SearchResult>, domains: Option<&[String]>) -> Vec<SearchResult> {
    let allowed = domains
        .unwrap_or_default()
        .iter()
        .filter_map(|domain| normalized_domain(domain))
        .collect::<Vec<_>>();
    if allowed.is_empty() {
        return deduplicate(results);
    }
    deduplicate(
        results
            .into_iter()
            .filter(|result| {
                reqwest::Url::parse(&result.url)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
                    .is_some_and(|host| {
                        allowed
                            .iter()
                            .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
                    })
            })
            .collect(),
    )
}

fn deduplicate(mut results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut seen = HashSet::new();
    results.retain(|result| seen.insert(result.url.clone()));
    results.truncate(DEFAULT_RESULT_LIMIT);
    results
}

fn parse_brave_results(body: &str) -> Result<Vec<SearchResult>, xai_tool_runtime::ToolError> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| tool_error(format!("Invalid Brave Search response: {error}")))?;
    Ok(value
        .pointer("/web/results")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(SearchResult {
                title: item.get("title")?.as_str()?.to_owned(),
                url: item.get("url")?.as_str()?.to_owned(),
                snippet: item
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
        })
        .collect())
}

fn parse_duckduckgo_results(body: &str) -> Result<Vec<SearchResult>, xai_tool_runtime::ToolError> {
    let document = Html::parse_document(body);
    let result_selector = Selector::parse(".result")
        .map_err(|error| tool_error(format!("Invalid search result selector: {error}")))?;
    let link_selector = Selector::parse("a.result__a")
        .map_err(|error| tool_error(format!("Invalid search link selector: {error}")))?;
    let snippet_selector = Selector::parse(".result__snippet")
        .map_err(|error| tool_error(format!("Invalid search snippet selector: {error}")))?;

    Ok(document
        .select(&result_selector)
        .filter_map(|node| {
            let link = node.select(&link_selector).next()?;
            let href = link.value().attr("href")?;
            let url = duckduckgo_target_url(href)?;
            let title = link.text().collect::<String>().trim().to_owned();
            let snippet = node
                .select(&snippet_selector)
                .next()
                .map(|item| item.text().collect::<String>().trim().to_owned())
                .unwrap_or_default();
            Some(SearchResult {
                title,
                url,
                snippet,
            })
        })
        .collect())
}

fn duckduckgo_target_url(href: &str) -> Option<String> {
    let url = reqwest::Url::parse(href)
        .or_else(|_| reqwest::Url::parse(&format!("https:{href}")))
        .ok()?;
    if url
        .host_str()
        .is_some_and(|host| host.ends_with("duckduckgo.com"))
    {
        if let Some((_, target)) = url.query_pairs().find(|(key, _)| key == "uddg") {
            return Some(target.into_owned());
        }
    }
    Some(url.to_string())
}

fn format_results(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "No search results found.".to_owned();
    }
    results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            format!(
                "{}. [{}]({})\n{}",
                index + 1,
                result.title,
                result.url,
                result.snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn tool_error(message: impl Into<String>) -> xai_tool_runtime::ToolError {
    xai_tool_runtime::ToolError::execution(
        xai_tool_protocol::ToolId::new(SEARCH_ID).expect("valid tool id"),
        message.into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_duckduckgo_html_and_redirect_target() {
        let html = r#"<div class="result"><a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fdocs.rs%2Ftokio">Tokio</a><div class="result__snippet">Async runtime</div></div>"#;
        let results = parse_duckduckgo_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://docs.rs/tokio");
        assert_eq!(results[0].snippet, "Async runtime");
    }

    #[test]
    fn domain_filter_accepts_subdomains_only() {
        let results = vec![
            SearchResult {
                title: "Allowed".into(),
                url: "https://docs.example.com/page".into(),
                snippet: String::new(),
            },
            SearchResult {
                title: "Blocked".into(),
                url: "https://example.com.evil.test/page".into(),
                snippet: String::new(),
            },
        ];
        let allowed = vec!["example.com".to_owned()];
        assert_eq!(filter_domains(results, Some(&allowed)).len(), 1);
    }
}
