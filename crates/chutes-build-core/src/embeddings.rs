//! Text embeddings over the Qwen3-Embedding-8B chute
//! (`chutes-qwen-qwen3-embedding-8b-tee`).
//!
//! Typed wrapper around [`ChutesMediaClient`] for the OpenAI-compatible
//! `/v1/embeddings` cord. Vectors are large (up to 4096 floats per text),
//! so callers are expected to persist them and surface a summary — this
//! module parses the wire shape and validates dimensions.

use crate::media::{ChutesMediaClient, MediaError};

/// Known embedding chute slug. Callers may override it per call; the
/// default is what the tool layer passes when the user names no model.
pub const DEFAULT_EMBED_SLUG: &str = "chutes-qwen-qwen3-embedding-8b-tee";

/// Model name the cord expects in the `model` field.
pub const DEFAULT_EMBED_MODEL: &str = "Qwen/Qwen3-Embedding-8B-TEE";

/// Parsed embedding response: one vector per input text, in order.
#[derive(Debug, Clone)]
pub struct EmbeddingsOutput {
    pub embeddings: Vec<Vec<f32>>,
    pub model: String,
    pub dimensions: usize,
    pub usage: Option<serde_json::Value>,
}

/// Build the `/v1/embeddings` request body. A single text goes as a bare
/// string (the documented shape); several go as an array, per OpenAI
/// semantics. Pure so the wire shape stays pinned by tests.
pub fn embed_body(texts: &[String], model: &str, dimensions: Option<u32>) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    if texts.len() == 1 {
        body.insert(
            "input".to_owned(),
            serde_json::Value::String(texts[0].clone()),
        );
    } else {
        body.insert(
            "input".to_owned(),
            serde_json::Value::Array(
                texts
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    body.insert(
        "model".to_owned(),
        serde_json::Value::String(model.to_owned()),
    );
    body.insert(
        "encoding_format".to_owned(),
        serde_json::Value::String("float".to_owned()),
    );
    if let Some(dimensions) = dimensions {
        body.insert(
            "dimensions".to_owned(),
            serde_json::Value::Number(dimensions.into()),
        );
    }
    serde_json::Value::Object(body)
}

/// Parse an OpenAI-shaped embedding response (`{data: [{embedding, index}],
/// model?, usage?}`), validating that every vector is non-empty and shares
/// one dimension. Pure so fixtures pin it without network access.
pub fn parse_embeddings(value: &serde_json::Value) -> Result<EmbeddingsOutput, String> {
    let data = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "embedding response has no `data` array".to_owned())?;
    if data.is_empty() {
        return Err("embedding response `data` is empty".to_owned());
    }
    let mut embeddings = Vec::with_capacity(data.len());
    for (position, item) in data.iter().enumerate() {
        let vector = item
            .get("embedding")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                format!("embedding response item {position} has no `embedding` array")
            })?;
        if vector.is_empty() {
            return Err(format!(
                "embedding response item {position} has an empty vector"
            ));
        }
        let mut floats = Vec::with_capacity(vector.len());
        for component in vector {
            let number = component.as_f64().ok_or_else(|| {
                format!("embedding response item {position} has a non-numeric component")
            })?;
            floats.push(number as f32);
        }
        embeddings.push(floats);
    }
    let dimensions = embeddings[0].len();
    if embeddings.iter().any(|vector| vector.len() != dimensions) {
        return Err("embedding response vectors have inconsistent dimensions".to_owned());
    }
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(DEFAULT_EMBED_MODEL)
        .to_owned();
    Ok(EmbeddingsOutput {
        embeddings,
        model,
        dimensions,
        usage: value.get("usage").cloned(),
    })
}

/// Embed `texts` through `base_url` (a resolved chute host). At least one
/// non-blank text is required.
pub async fn embed_texts(
    client: &ChutesMediaClient,
    base_url: &str,
    model: &str,
    texts: &[String],
    dimensions: Option<u32>,
) -> Result<EmbeddingsOutput, MediaError> {
    if texts.is_empty() || texts.iter().all(|text| text.trim().is_empty()) {
        return Err(MediaError::UnexpectedResponse(
            "embed_texts needs at least one non-blank text".to_owned(),
        ));
    }
    let url = format!("{}/v1/embeddings", base_url.trim_end_matches('/'));
    let body = embed_body(texts, model, dimensions);
    let response = client.invoke(&url, "POST", &body).await?;
    let bytes = response.bytes().ok_or_else(|| {
        MediaError::UnexpectedResponse(
            "embedding cord returned a spilled (non-memory) response".to_owned(),
        )
    })?;
    let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(MediaError::Decode)?;
    parse_embeddings(&raw).map_err(MediaError::UnexpectedResponse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_text_body_uses_bare_string() {
        let body = embed_body(&["hello".to_owned()], DEFAULT_EMBED_MODEL, None);
        assert_eq!(body["input"], "hello");
        assert_eq!(body["model"], DEFAULT_EMBED_MODEL);
        assert_eq!(body["encoding_format"], "float");
        assert!(body.get("dimensions").is_none());
    }

    #[test]
    fn multi_text_body_uses_array_and_dimensions() {
        let body = embed_body(&["a".to_owned(), "b".to_owned()], "m", Some(1024));
        assert_eq!(body["input"], serde_json::json!(["a", "b"]));
        assert_eq!(body["dimensions"], 1024);
    }

    #[test]
    fn parse_accepts_openai_shape() {
        let value = serde_json::json!({
            "data": [
                {"embedding": [0.1, 0.2], "index": 0},
                {"embedding": [0.3, 0.4], "index": 1},
            ],
            "model": "m",
            "usage": {"total_tokens": 4},
        });
        let output = parse_embeddings(&value).unwrap();
        assert_eq!(output.dimensions, 2);
        assert_eq!(output.embeddings.len(), 2);
        assert_eq!(output.model, "m");
        assert!(output.usage.is_some());
    }

    #[test]
    fn parse_rejects_ragged_and_empty_shapes() {
        let value = serde_json::json!({"data": []});
        assert!(parse_embeddings(&value).is_err());
        let value = serde_json::json!({"data": [{"embedding": [0.1]}, {"embedding": [0.1, 0.2]}]});
        assert!(parse_embeddings(&value).is_err());
        let value = serde_json::json!({"data": [{"embedding": []}]});
        assert!(parse_embeddings(&value).is_err());
        let value = serde_json::json!({"nada": true});
        assert!(parse_embeddings(&value).is_err());
    }
}
