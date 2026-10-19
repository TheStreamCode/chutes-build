//! Text embeddings for workspace workflows via a Chutes embedding model.
//!
//! Embeds one or more texts into vectors for similarity, clustering, and
//! retrieval. Vectors are large (thousands of floats per text), so they are
//! written to a workspace JSON file and only a summary — model, count,
//! dimensions, path — is returned into the conversation.

use chutes_build_core::embeddings::{DEFAULT_EMBED_MODEL, DEFAULT_EMBED_SLUG, embed_texts};
use chutes_build_core::media::ChutesMediaClient;

use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::Cwd;
use crate::types::tool::{ToolKind, ToolNamespace};

/// Maximum texts per call. Batches stay small enough to fit the cord's
/// 40k-token window several times over without silent truncation surprises.
const MAX_TEXTS: usize = 100;

/// Fixed output directory under the working directory. Not configurable:
/// embedding files are intermediates, and one known location keeps them
/// out of the way.
const OUTPUT_SUBDIR: &str = "assets/chutes/embeddings";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct EmbedTextsInput {
    /// Texts to embed (1..=100). Each should fit the cord's context; long
    /// documents should be chunked by the caller first.
    pub texts: Vec<String>,
    /// Chute slug override when the default embedding chute is unavailable.
    /// Must expose a compatible OpenAI-style `/v1/embeddings` cord.
    pub model: Option<String>,
    /// Model name sent in the cord's `model` field. Defaults to the default
    /// chute's model; set it together with `model` when overriding.
    pub model_name: Option<String>,
    /// Truncate vectors to this dimension (MRL-style). Omitted keeps full width.
    pub dimensions: Option<u32>,
    /// Output file stem for the vectors JSON. Sanitized to `[A-Za-z0-9._-]`;
    /// defaults to a timestamped `embeddings-<millis>` name.
    pub filename: Option<String>,
}

#[derive(Debug, Default)]
pub struct EmbedTextsTool;

impl crate::types::tool_metadata::ToolMetadata for EmbedTextsTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::ChutesBuild
    }

    fn description_template(&self) -> &str {
        "Embed texts into vectors via a Chutes embedding model for similarity, clustering, and \
         retrieval. Vectors are saved to a workspace JSON file (too large for context); returns \
         the path, dimensions, and count. Reads nothing, writes one JSON file."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for EmbedTextsTool {
    type Args = EmbedTextsInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        tool_id("embed_texts")
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "embed_texts",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: EmbedTextsInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        if input.texts.is_empty() || input.texts.len() > MAX_TEXTS {
            return Err(xai_tool_runtime::ToolError::invalid_arguments(format!(
                "embed_texts needs 1..={MAX_TEXTS} texts, got {}",
                input.texts.len()
            )));
        }
        let slug = input
            .model
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(DEFAULT_EMBED_SLUG);
        let model_name = input
            .model_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(DEFAULT_EMBED_MODEL);

        let credentials = super::resolve_credentials(&ctx)
            .map_err(|error| execution_error("embed_texts", error))?;
        let client = ChutesMediaClient::from_credentials(credentials)
            .map_err(|error| execution_error("embed_texts", error))?;
        let detail = client.describe(slug).await.map_err(|error| {
            execution_error(
                "embed_texts",
                format!(
                    "embedding chute `{slug}` is not reachable ({error}); use list_media_models with an embedding query to find an equivalent chute"
                ),
            )
        })?;
        let base_url = super::media::invoke_base_url(&detail).ok_or_else(|| {
            execution_error(
                "embed_texts",
                format!("embedding chute `{slug}` publishes no invocation URL"),
            )
        })?;

        let output = embed_texts(
            &client,
            &base_url,
            model_name,
            &input.texts,
            input.dimensions,
        )
        .await
        .map_err(|error| execution_error("embed_texts", error))?;

        let resources = crate::types::tool_metadata::shared_resources(&ctx)?;
        let cwd = {
            let res = resources.lock().await;
            res.require::<Cwd>()?.0.clone()
        };
        let stored = write_vectors_file(&cwd, input.filename.as_deref(), &output)
            .await
            .map_err(|error| execution_error("embed_texts", error))?;

        pretty_text(
            serde_json::json!({
                "model": output.model,
                "count": output.embeddings.len(),
                "dimensions": output.dimensions,
                "path": stored,
                "usage": output.usage,
            }),
            "embed_texts",
        )
    }
}

/// Keep only filename-safe characters; fall back to a timestamped default
/// when nothing (or nothing usable) remains.
fn sanitize_filename_stem(filename: Option<&str>) -> String {
    let cleaned: String = filename
        .unwrap_or("")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c| c == '_' || c == '.');
    if trimmed.is_empty() {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("embeddings-{millis}")
    } else {
        trimmed.chars().take(64).collect()
    }
}

/// Write the vectors JSON under `<cwd>/assets/chutes/embeddings/`,
/// refusing to escape the workspace and to overwrite an existing file.
/// Returns the stored path relative to the working directory.
async fn write_vectors_file(
    cwd: &std::path::Path,
    filename: Option<&str>,
    output: &chutes_build_core::embeddings::EmbeddingsOutput,
) -> Result<String, String> {
    let anchor = crate::util::fs::try_canonicalize(cwd)
        .await
        .unwrap_or_else(|_| cwd.to_path_buf());
    let dir = anchor.join(OUTPUT_SUBDIR);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| format!("failed to create {}: {error}", dir.display()))?;
    let file_name = format!("{}.json", sanitize_filename_stem(filename));
    let path = dir.join(&file_name);
    let canonical_parent = crate::util::fs::try_canonicalize(&dir)
        .await
        .unwrap_or_else(|_| dir.clone());
    if !canonical_parent.starts_with(&anchor) {
        return Err("refusing to write outside the working directory".to_owned());
    }
    let payload = serde_json::json!({
        "model": output.model,
        "dimensions": output.dimensions,
        "count": output.embeddings.len(),
        "embeddings": output.embeddings,
        "usage": output.usage,
    });
    let bytes = serde_json::to_vec_pretty(&payload)
        .map_err(|error| format!("failed to serialize vectors: {error}"))?;
    tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
        .map_err(|error| {
            format!(
                "failed to create {} (refusing to overwrite): {error}",
                path.display()
            )
        })?;
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    path.strip_prefix(&anchor)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| "failed to relativize output path".to_owned())
}

fn pretty_text(
    value: serde_json::Value,
    tool: &str,
) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
    serde_json::to_string_pretty(&value)
        .map(|text| ToolOutput::Text(text.into()))
        .map_err(|error| execution_error(tool, error))
}

fn tool_id(name: &str) -> xai_tool_protocol::ToolId {
    xai_tool_protocol::ToolId::new(name).expect("valid tool id")
}

fn execution_error(tool: &str, error: impl std::fmt::Display) -> xai_tool_runtime::ToolError {
    xai_tool_runtime::ToolError::execution(tool_id(tool), error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_sanitizer_keeps_safe_stems() {
        assert_eq!(
            sanitize_filename_stem(Some("run-1_final.v2")),
            "run-1_final.v2"
        );
        assert_eq!(
            sanitize_filename_stem(Some("../../etc/passwd")),
            "etc_passwd"
        );
        assert_eq!(
            sanitize_filename_stem(Some("a".repeat(100).as_str())).len(),
            64
        );
    }

    #[test]
    fn filename_sanitizer_falls_back_to_timestamped_default() {
        for candidate in [None, Some(""), Some("..."), Some("___")] {
            let stem = sanitize_filename_stem(candidate);
            assert!(
                stem.starts_with("embeddings-"),
                "expected timestamped default, got {stem}"
            );
        }
    }
}
