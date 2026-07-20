//! Live Chutes model capability discovery.

use crate::ChutesEndpoints;

const CATALOG_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

struct CatalogCacheEntry {
    endpoint: String,
    fetched_at: std::time::Instant,
    body: serde_json::Value,
}

static CATALOG_CACHE: std::sync::LazyLock<tokio::sync::Mutex<Option<CatalogCacheEntry>>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(None));

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("model catalog request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Chutes model catalog returned HTTP {status}")]
    Status { status: reqwest::StatusCode },
}

/// Return whether a catalogued model accepts an input modality. `None` means
/// the model was not present in the current live catalog. The virtual
/// `model-router` is capability-aware and can accept multimodal turns.
pub async fn model_supports_input(
    model: &str,
    modality: &str,
) -> Result<Option<bool>, CatalogError> {
    if model == "model-router" {
        return Ok(Some(true));
    }
    let endpoint = ChutesEndpoints::default().inference;
    let mut cache = CATALOG_CACHE.lock().await;
    if let Some(entry) = cache.as_ref()
        && entry.endpoint == endpoint
        && entry.fetched_at.elapsed() < CATALOG_CACHE_TTL
    {
        return Ok(supports_input(&entry.body, model, modality));
    }
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .get(format!("{endpoint}/models"))
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        return Err(CatalogError::Status { status });
    }
    let body: serde_json::Value = response.json().await?;
    let result = supports_input(&body, model, modality);
    *cache = Some(CatalogCacheEntry {
        endpoint,
        fetched_at: std::time::Instant::now(),
        body,
    });
    Ok(result)
}

fn supports_input(body: &serde_json::Value, model: &str, modality: &str) -> Option<bool> {
    body.get("data")
        .and_then(serde_json::Value::as_array)
        .and_then(|models| {
            models.iter().find(|entry| {
                entry.get("id").and_then(serde_json::Value::as_str) == Some(model)
                    || entry.get("root").and_then(serde_json::Value::as_str) == Some(model)
            })
        })
        .map(|entry| {
            entry
                .get("input_modalities")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|modalities| {
                    modalities
                        .iter()
                        .any(|value| value.as_str() == Some(modality))
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn virtual_router_is_multimodal_without_network() {
        assert_eq!(
            model_supports_input("model-router", "image").await.unwrap(),
            Some(true)
        );
        assert_eq!(
            model_supports_input("model-router", "video").await.unwrap(),
            Some(true)
        );
    }
}
