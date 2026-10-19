//! Live Chutes model capability discovery.

use crate::ChutesEndpoints;

const CATALOG_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

/// Modalities the virtual `model-router` id is known to accept regardless of
/// which concrete model it dispatches a turn to. Anything not in this set
/// falls through to per-model catalog data instead of being claimed
/// unconditionally.
const ROUTER_SUPPORTED_MODALITIES: &[&str] = &["text", "image"];

struct CatalogCacheEntry {
    endpoint: String,
    fetched_at: std::time::Instant,
    body: CatalogResponse,
}

/// Typed shell around the `/models` response. Kept deliberately shallow:
/// `data` decodes as raw [`serde_json::Value`] entries rather than a typed
/// `Vec<CatalogModel>` so that one malformed or unexpected-shape model entry
/// (a third-party chute publishing an odd catalog record, say) can't fail
/// the parse for the whole catalog — [`supports_input`] decodes each entry
/// individually and best-effort skips ones that don't fit.
#[derive(Debug, Clone, serde::Deserialize, Default)]
struct CatalogResponse {
    #[serde(default)]
    data: Vec<serde_json::Value>,
}

/// The subset of a catalog entry's fields this crate actually consumes.
/// Every field is optional/defaulted on purpose: a model missing `id`,
/// `root`, `input_modalities`, or `supported_features` must still be
/// treated as "no match" / "no declared modalities or features", not fail
/// this entry's parse.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct CatalogModel {
    id: Option<String>,
    root: Option<String>,
    #[serde(default)]
    input_modalities: Vec<String>,
    #[serde(default)]
    output_modalities: Vec<String>,
    #[serde(default)]
    supported_features: Vec<String>,
}

/// Cached catalog body, read on the fast path without ever holding a lock
/// across network I/O.
static CACHE: std::sync::LazyLock<tokio::sync::RwLock<Option<CatalogCacheEntry>>> =
    std::sync::LazyLock::new(|| tokio::sync::RwLock::new(None));

/// Held only while an actual refresh request is in flight, so concurrent
/// callers that need a refresh queue up behind one shared fetch instead of
/// each issuing their own. Callers with a valid cache entry never touch this.
static REFRESH_GATE: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Shared client so every call reuses pooled connections instead of building
/// a fresh `reqwest::Client` per request.
// WHY-ALLOW: product-policy client with `redirect::none` and a live catalogue
// base URL from the allowlisted endpoints; extra-CA support for these clients
// is tracked separately.
#[allow(clippy::disallowed_methods)]
static HTTP_CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("static reqwest client config is valid")
});

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("model catalog request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Chutes model catalog returned HTTP {status}")]
    Status { status: reqwest::StatusCode },
    #[error("Chutes model catalog returned invalid JSON: {0}")]
    Decode(serde_json::Error),
    #[error(transparent)]
    EndpointTrust(#[from] crate::endpoint_policy::EndpointTrustError),
}

/// Return whether a catalogued model accepts an input modality. `None` means
/// the model was not present in the current live catalog. The virtual
/// `model-router` id accepts [`ROUTER_SUPPORTED_MODALITIES`] regardless of
/// which concrete model handles a given turn.
pub async fn model_supports_input(
    model: &str,
    modality: &str,
) -> Result<Option<bool>, CatalogError> {
    if model == "model-router" {
        return Ok(Some(ROUTER_SUPPORTED_MODALITIES.contains(&modality)));
    }
    let body = live_catalog_body().await?;
    Ok(supports_input(&body, model, modality))
}

/// The live catalog body behind every query below: fresh-or-cached within
/// the TTL, single-flighted across concurrent callers, stale-cache tolerant
/// when a refresh fails.
async fn live_catalog_body() -> Result<CatalogResponse, CatalogError> {
    let endpoint = ChutesEndpoints::default().inference;
    crate::endpoint_policy::validate_endpoint_url(&endpoint)?;
    if let Some(body) = cached_body(&endpoint).await {
        return Ok(body);
    }
    // Single-flight: only one task fetches at a time. Others queue here
    // (not on the network call itself) and re-check the cache once they get
    // the gate, in case another task just refreshed it while they waited.
    let _gate = REFRESH_GATE.lock().await;
    if let Some(body) = cached_body(&endpoint).await {
        return Ok(body);
    }
    match refresh_catalog(&endpoint).await {
        Ok(body) => Ok(body),
        Err(error) => {
            // Stale-cache fallback: prefer a previous (even expired) entry
            // for this endpoint over a hard failure, if one exists.
            let cache = CACHE.read().await;
            if let Some(entry) = cache.as_ref()
                && entry.endpoint == endpoint
            {
                return Ok(entry.body.clone());
            }
            Err(error)
        }
    }
}

async fn cached_body(endpoint: &str) -> Option<CatalogResponse> {
    let cache = CACHE.read().await;
    let entry = cache.as_ref()?;
    if entry.endpoint == endpoint && entry.fetched_at.elapsed() < CATALOG_CACHE_TTL {
        Some(entry.body.clone())
    } else {
        None
    }
}

/// Chat-capable catalogue ids for Auto's live inline pool. Cached for the
/// same TTL as modality lookups. Order follows the live catalogue.
pub async fn live_chat_model_ids() -> Result<Vec<String>, CatalogError> {
    let body = live_catalog_body().await?;
    Ok(chat_model_ids(&body))
}

/// A chat-capable catalog entry with the modalities and features fallback
/// selection needs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatModelEntry {
    id: String,
    input_modalities: Vec<String>,
    supported_features: Vec<String>,
}

/// Chat-capable entries in live catalog order: concrete text-in/text-out ids
/// only (no saved-pool aliases, no virtual router id, no inline pools).
/// Missing modality lists default to text, matching the inference API's
/// behavior for legacy entries.
fn chat_models(body: &CatalogResponse) -> Vec<ChatModelEntry> {
    body.data
        .iter()
        .filter_map(|raw_entry| {
            let entry: CatalogModel = serde_json::from_value(raw_entry.clone()).ok()?;
            let id = entry.id.filter(|value| !value.trim().is_empty())?;
            if id == crate::routing::DEFAULT_ALIAS || id == crate::routing::LEGACY_AUTO_MODEL_ID {
                return None;
            }
            if id.contains(',') {
                return None;
            }
            let input = if entry.input_modalities.is_empty() {
                vec!["text".to_owned()]
            } else {
                entry.input_modalities
            };
            let output = if entry.output_modalities.is_empty() {
                vec!["text".to_owned()]
            } else {
                entry.output_modalities
            };
            if input.iter().any(|m| m == "text") && output.iter().any(|m| m == "text") {
                Some(ChatModelEntry {
                    id,
                    input_modalities: input,
                    supported_features: entry.supported_features,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Whether the excluded model positively lacks tool support: found in the
/// catalog with a non-empty features list that has no `tools`. Anything
/// else (declares `tools`, has no features list, or is absent entirely)
/// keeps the tool requirement on — a tool-capable replacement is a safe
/// superset for any turn, while the reverse breaks agentic turns. Mirrors
/// the image fail-closed default at the sampler call site.
fn exclude_lacks_tools(body: &CatalogResponse, exclude: &str) -> bool {
    body.data
        .iter()
        .find_map(|raw_entry| {
            let entry: CatalogModel = serde_json::from_value(raw_entry.clone()).ok()?;
            if entry.id.as_deref() == Some(exclude) || entry.root.as_deref() == Some(exclude) {
                Some(
                    !entry.supported_features.is_empty()
                        && !entry.supported_features.iter().any(|f| f == "tools"),
                )
            } else {
                None
            }
        })
        .unwrap_or(false)
}

/// First live chat model other than `exclude`, in catalog order. When
/// `require_image`, only models declaring image input qualify. Tool support
/// follows the excluded model: unless it positively lacks `tools`, only
/// replacements declaring `tools` qualify (a missing features list counts
/// as no tools). Pure over a fetched body so fallback selection stays
/// hermetic and testable.
fn equivalent_model(body: &CatalogResponse, exclude: &str, require_image: bool) -> Option<String> {
    let require_tools = !exclude_lacks_tools(body, exclude);
    chat_models(body).into_iter().find_map(|model| {
        if model.id == exclude {
            return None;
        }
        if require_image && !model.input_modalities.iter().any(|m| m == "image") {
            return None;
        }
        if require_tools && !model.supported_features.iter().any(|f| f == "tools") {
            return None;
        }
        Some(model.id)
    })
}

/// Live equivalent of [`equivalent_model`] over the cached/live catalog, for
/// sampler pre-stream fallback when a model is gone or cannot serve. Tool
/// support follows the failed model (see [`equivalent_model`]).
pub async fn find_equivalent_chat_model(
    exclude: &str,
    require_image: bool,
) -> Result<Option<String>, CatalogError> {
    let body = live_catalog_body().await?;
    Ok(equivalent_model(&body, exclude, require_image))
}

async fn refresh_catalog(endpoint: &str) -> Result<CatalogResponse, CatalogError> {
    let body = fetch_catalog(endpoint).await?;
    *CACHE.write().await = Some(CatalogCacheEntry {
        endpoint: endpoint.to_owned(),
        fetched_at: std::time::Instant::now(),
        body: body.clone(),
    });
    Ok(body)
}

fn chat_model_ids(body: &CatalogResponse) -> Vec<String> {
    chat_models(body)
        .into_iter()
        .map(|model| model.id)
        .collect()
}

async fn fetch_catalog(endpoint: &str) -> Result<CatalogResponse, CatalogError> {
    let response = HTTP_CLIENT.get(format!("{endpoint}/models")).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(CatalogError::Status { status });
    }
    let body = response.text().await?;
    serde_json::from_str(&body).map_err(CatalogError::Decode)
}

fn supports_input(body: &CatalogResponse, model: &str, modality: &str) -> Option<bool> {
    body.data.iter().find_map(|raw_entry| {
        // Best-effort per entry: an entry that doesn't fit `CatalogModel`'s
        // shape is skipped rather than failing the whole catalog lookup.
        let entry: CatalogModel = serde_json::from_value(raw_entry.clone()).ok()?;
        if entry.id.as_deref() == Some(model) || entry.root.as_deref() == Some(model) {
            Some(entry.input_modalities.iter().any(|m| m == modality))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn virtual_router_accepts_text_and_image_only() {
        assert_eq!(
            model_supports_input("model-router", "image").await.unwrap(),
            Some(true)
        );
        assert_eq!(
            model_supports_input("model-router", "text").await.unwrap(),
            Some(true)
        );
        assert_eq!(
            model_supports_input("model-router", "video").await.unwrap(),
            Some(false)
        );
    }

    /// Clears the process-global cache between env-var-dependent tests so
    /// one test's mock endpoint can't leak into another's assertions.
    async fn reset_cache() {
        *CACHE.write().await = None;
    }

    // SAFETY: gated behind #[serial] -- no other test in this module runs
    // concurrently while these env vars are set.
    unsafe fn set_test_env(inference_base_url: &str) {
        unsafe {
            std::env::set_var("CHUTES_INFERENCE_BASE_URL", inference_base_url);
            std::env::set_var("CHUTES_ALLOW_INSECURE_ENDPOINTS", "1");
        }
    }

    unsafe fn clear_test_env() {
        unsafe {
            std::env::remove_var("CHUTES_INFERENCE_BASE_URL");
            std::env::remove_var("CHUTES_ALLOW_INSECURE_ENDPOINTS");
        }
    }

    #[tokio::test]
    #[serial]
    async fn fetches_and_caches_from_mock_server() {
        reset_cache().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "test-model", "input_modalities": ["text", "image"]}],
            })))
            .expect(1)
            .mount(&server)
            .await;
        unsafe { set_test_env(&server.uri()) };

        let first = model_supports_input("test-model", "image").await.unwrap();
        let second = model_supports_input("test-model", "image").await.unwrap();

        unsafe { clear_test_env() };
        assert_eq!(first, Some(true));
        // Second call must hit the cache, not the mock server again --
        // `.expect(1)` above fails the test on drop if it was called twice.
        assert_eq!(second, Some(true));
    }

    /// Forward-compat: unknown top-level fields, an unrelated entry with a
    /// completely different (also unknown-field-bearing) shape, and a
    /// missing `input_modalities` on the matched entry must not fail the
    /// parse or the lookup -- only the fields this crate actually reads
    /// are typed, everything else is ignored by default.
    #[tokio::test]
    #[serial]
    async fn tolerates_unknown_fields_and_missing_modalities() {
        reset_cache().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "future_top_level_field": {"nested": true},
                "data": [
                    {
                        "id": "some-other-model",
                        "root": "some-other-model",
                        "input_modalities": ["text"],
                        "pricing": {"prompt": "0.0"},
                        "future_field": "whatever",
                    },
                    {"id": "test-model", "root": "test-model"},
                ],
            })))
            .expect(1)
            .mount(&server)
            .await;
        unsafe { set_test_env(&server.uri()) };

        let result = model_supports_input("test-model", "image").await;

        unsafe { clear_test_env() };
        // Matched entry has no `input_modalities` at all -> not an error,
        // just "doesn't declare this modality".
        assert_eq!(result.unwrap(), Some(false));
    }

    #[tokio::test]
    #[serial]
    async fn falls_back_to_stale_cache_when_refresh_fails() {
        reset_cache().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "test-model", "input_modalities": ["image"]}],
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        unsafe { set_test_env(&server.uri()) };

        let warm = model_supports_input("test-model", "image").await.unwrap();
        assert_eq!(warm, Some(true));

        // Force the cache to look expired so the next call attempts a
        // refresh, which the mock server above will now reject (no handler
        // left after `up_to_n_times(1)`) -- the stale entry must still win.
        {
            let mut cache = CACHE.write().await;
            if let Some(entry) = cache.as_mut() {
                entry.fetched_at -= CATALOG_CACHE_TTL * 2;
            }
        }
        let stale = model_supports_input("test-model", "image").await.unwrap();

        unsafe { clear_test_env() };
        assert_eq!(
            stale,
            Some(true),
            "stale cache should serve when refresh fails"
        );
    }

    #[tokio::test]
    #[serial]
    async fn concurrent_callers_trigger_only_one_refresh() {
        reset_cache().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(100))
                    .set_body_json(serde_json::json!({
                        "data": [{"id": "test-model", "input_modalities": ["image"]}],
                    })),
            )
            .expect(1)
            .mount(&server)
            .await;
        unsafe { set_test_env(&server.uri()) };

        let (a, b, c) = tokio::join!(
            model_supports_input("test-model", "image"),
            model_supports_input("test-model", "image"),
            model_supports_input("test-model", "image"),
        );

        unsafe { clear_test_env() };
        // `.expect(1)` above fails on drop if more than one request landed.
        assert_eq!(a.unwrap(), Some(true));
        assert_eq!(b.unwrap(), Some(true));
        assert_eq!(c.unwrap(), Some(true));
    }

    #[tokio::test]
    #[serial]
    async fn rejects_untrusted_endpoint_without_opt_in() {
        reset_cache().await;
        // SAFETY: gated behind #[serial].
        unsafe {
            std::env::set_var("CHUTES_INFERENCE_BASE_URL", "http://127.0.0.1:1");
            std::env::remove_var("CHUTES_ALLOW_INSECURE_ENDPOINTS");
        }
        let result = model_supports_input("test-model", "image").await;
        unsafe {
            std::env::remove_var("CHUTES_INFERENCE_BASE_URL");
        }
        assert!(matches!(result, Err(CatalogError::EndpointTrust(_))));
    }

    #[test]
    fn chat_ids_keep_text_models_and_drop_aliases() {
        let body = CatalogResponse {
            data: vec![
                serde_json::json!({"id": "a/Chat", "input_modalities": ["text"], "output_modalities": ["text"]}),
                serde_json::json!({"id": "default"}),
                serde_json::json!({"id": "b/Image", "input_modalities": ["text"], "output_modalities": ["image"]}),
                serde_json::json!({"id": "model-router"}),
                serde_json::json!({"id": "c/Bare"}),
            ],
        };
        assert_eq!(chat_model_ids(&body), ["a/Chat", "c/Bare"]);
    }

    #[test]
    fn equivalent_model_skips_excluded_aliases_and_pools() {
        let body = CatalogResponse {
            data: vec![
                serde_json::json!({"id": "a/Text", "input_modalities": ["text"], "output_modalities": ["text"], "supported_features": ["tools"]}),
                serde_json::json!({"id": "b/Multi", "input_modalities": ["text", "image"], "output_modalities": ["text"], "supported_features": ["tools"]}),
                serde_json::json!({"id": "default"}),
                serde_json::json!({"id": "model-router"}),
                serde_json::json!({"id": "x,y"}),
                serde_json::json!({"id": "c/Text", "input_modalities": ["text"], "output_modalities": ["text"], "supported_features": ["tools"]}),
            ],
        };
        assert_eq!(
            equivalent_model(&body, "a/Text", false),
            Some("b/Multi".to_owned())
        );
        assert_eq!(
            equivalent_model(&body, "missing", false),
            Some("a/Text".to_owned())
        );
        assert_eq!(
            equivalent_model(&body, "a/Text", true),
            Some("b/Multi".to_owned())
        );
        assert_eq!(equivalent_model(&body, "b/Multi", true), None);
    }

    #[test]
    fn equivalent_model_follows_excluded_tools_support() {
        let body = CatalogResponse {
            data: vec![
                serde_json::json!({"id": "a/Tools", "input_modalities": ["text"], "output_modalities": ["text"], "supported_features": ["tools"]}),
                serde_json::json!({"id": "b/Plain", "input_modalities": ["text"], "output_modalities": ["text"], "supported_features": ["json_mode"]}),
                serde_json::json!({"id": "c/Bare", "input_modalities": ["text"], "output_modalities": ["text"]}),
            ],
        };
        // Excluded model has tools: only tool replacements qualify.
        assert_eq!(
            equivalent_model(&body, "a/Tools", false),
            None,
            "no tool replacement besides the excluded one"
        );
        // Excluded model positively lacks tools: the requirement lifts.
        assert_eq!(
            equivalent_model(&body, "b/Plain", false),
            Some("a/Tools".to_owned())
        );
        // Excluded model absent from the catalog: fail closed, require tools.
        assert_eq!(
            equivalent_model(&body, "missing", false),
            Some("a/Tools".to_owned())
        );
        // A bare entry (no features list) never qualifies as a tool
        // replacement, but can still be replaced by one.
        assert_eq!(
            equivalent_model(&body, "c/Bare", false),
            Some("a/Tools".to_owned())
        );
    }

    #[test]
    fn equivalent_model_returns_none_when_only_excluded_remains() {
        let solo = CatalogResponse {
            data: vec![serde_json::json!({"id": "only", "supported_features": ["tools"]})],
        };
        assert_eq!(equivalent_model(&solo, "only", false), None);
        assert_eq!(
            equivalent_model(&solo, "other", false),
            Some("only".to_owned())
        );
    }
}
