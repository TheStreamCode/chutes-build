//! Chutes-native discovery and multimodal generation tools.

use std::path::{Component, Path, PathBuf};

use base64::Engine as _;
use chutes_build_core::media::{ChutesMediaClient, MediaError, MediaResponse};
use tokio::io::AsyncWriteExt as _;

use crate::types::output::{MediaArtifact, MediaArtifactKind, ToolOutput};
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::Cwd;
use crate::types::tool::{ToolKind, ToolNamespace};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Video,
    Music,
    Speech,
}

impl MediaKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Music => "music",
            Self::Speech => "speech",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ListMediaModelsInput {
    /// Optional media capability filter.
    pub kind: Option<MediaKind>,
    /// Optional case-insensitive text filter over name, slug, template, and tagline.
    pub query: Option<String>,
    /// Maximum results to return (1..100, default 25).
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DescribeMediaModelInput {
    /// Chute id, slug, or model name returned by `list_media_models`.
    pub model: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GenerateMediaInput {
    /// Chute id, slug, or model name.
    pub model: String,
    /// Expected output family.
    pub kind: MediaKind,
    /// Cord payload. Workspace file paths in non-text fields are encoded as base64.
    pub params: serde_json::Value,
    /// Optional cord name or public path. The primary POST cord is selected by default.
    pub cord: Option<String>,
    /// Relative output directory within the current workspace.
    pub output_dir: Option<String>,
    /// Optional safe filename stem. The extension is selected from the response type.
    pub filename: Option<String>,
}

#[derive(Debug, Default)]
pub struct ListMediaModelsTool;

impl crate::types::tool_metadata::ToolMetadata for ListMediaModelsTool {
    fn kind(&self) -> ToolKind {
        ToolKind::List
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "List public Chutes media models for image generation/editing, video, music, and speech. This queries the live Chutes catalog and never exposes the API key."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for ListMediaModelsTool {
    type Args = ListMediaModelsInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        tool_id("list_media_models")
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "list_media_models",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        read_only_capabilities()
    }

    async fn run(
        &self,
        _: xai_tool_runtime::ToolCallContext,
        input: ListMediaModelsInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        let catalog = ChutesMediaClient::from_env()
            .map_err(|error| execution_error("list_media_models", error))?
            .list()
            .await
            .map_err(|error| execution_error("list_media_models", error))?;
        let query = input
            .query
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty());
        let limit = input.limit.unwrap_or(25).clamp(1, 100);
        let models = catalog_records(&catalog)
            .into_iter()
            .filter_map(summarize_model)
            .filter(|model| {
                input
                    .kind
                    .is_none_or(|kind| model["kind"].as_str() == Some(kind.as_str()))
                    && query.is_none_or(|query| {
                        model
                            .to_string()
                            .to_lowercase()
                            .contains(&query.to_lowercase())
                    })
            })
            .take(limit)
            .collect::<Vec<_>>();
        pretty_text(
            serde_json::json!({ "count": models.len(), "models": models }),
            "list_media_models",
        )
    }
}

#[derive(Debug, Default)]
pub struct DescribeMediaModelTool;

impl crate::types::tool_metadata::ToolMetadata for DescribeMediaModelTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Describe a Chutes media model, its callable cords, input schemas, methods, and output types before generation."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for DescribeMediaModelTool {
    type Args = DescribeMediaModelInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        tool_id("describe_media_model")
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "describe_media_model",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        read_only_capabilities()
    }

    async fn run(
        &self,
        _: xai_tool_runtime::ToolCallContext,
        input: DescribeMediaModelInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        let detail = ChutesMediaClient::from_env()
            .map_err(|error| execution_error("describe_media_model", error))?
            .describe(input.model.trim())
            .await
            .map_err(|error| execution_error("describe_media_model", error))?;
        pretty_text(compact_detail(&detail), "describe_media_model")
    }
}

#[derive(Debug, Default)]
pub struct GenerateMediaTool;

impl crate::types::tool_metadata::ToolMetadata for GenerateMediaTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Generate or edit image, video, music, or speech with a selected Chutes model. Call describe_media_model first. Outputs and a provenance sidecar are saved only inside the current workspace."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for GenerateMediaTool {
    type Args = GenerateMediaInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        tool_id("generate_media")
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "generate_media",
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
        input: GenerateMediaInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        let resources = crate::types::tool_metadata::shared_resources(&ctx)?;
        let cwd = resources.lock().await.require::<Cwd>()?.0.clone();
        let client = ChutesMediaClient::from_env()
            .map_err(|error| execution_error("generate_media", error))?;
        let detail = client
            .describe(input.model.trim())
            .await
            .map_err(|error| execution_error("generate_media", error))?;
        let cord = select_cord(&detail, input.kind, input.cord.as_deref())
            .map_err(|error| execution_error("generate_media", error))?;
        let invoke_base = invoke_base_url(&detail)
            .ok_or_else(|| execution_error("generate_media", "model has no invocation URL"))?;
        let mut params = input
            .params
            .as_object()
            .cloned()
            .ok_or_else(|| execution_error("generate_media", "params must be a JSON object"))?;
        validate_schema_fields(
            &cord.schema,
            &params,
            env_flag_default("CHUTES_ALLOW_UNKNOWN_PARAMS", false),
        )
        .map_err(|error| execution_error("generate_media", error))?;
        encode_workspace_assets(&mut params, &cwd)
            .await
            .map_err(|error| execution_error("generate_media", error))?;

        if env_flag_default("CHUTES_WARMUP", true) {
            let _ = client.warmup(input.model.trim()).await;
        }
        let url = format!("{}{}", invoke_base.trim_end_matches('/'), cord.path);
        let response = invoke_with_cold_start_retry(
            &client,
            input.model.trim(),
            &url,
            &cord.method,
            &serde_json::Value::Object(params),
            env_usize("CHUTES_COLD_START_RETRIES", 4).min(10),
        )
        .await
        .map_err(|error| execution_error("generate_media", error))?;
        let media = resolve_media_response(&client, response, input.kind)
            .await
            .map_err(|error| execution_error("generate_media", error))?;
        assert_content_type(input.kind, &media.content_type)
            .map_err(|error| execution_error("generate_media", error))?;
        let detected = verify_media_content(input.kind, &media.bytes)
            .map_err(|error| execution_error("generate_media", error))?;

        let configured_output = input.output_dir.clone().or_else(|| {
            std::env::var("CHUTES_OUTPUT_DIR").ok().map(|base| {
                PathBuf::from(base)
                    .join(input.kind.as_str())
                    .to_string_lossy()
                    .into_owned()
            })
        });
        let output_dir = secure_output_dir(&cwd, configured_output.as_deref(), input.kind)
            .await
            .map_err(|error| execution_error("generate_media", error))?;
        let extension = detected
            .map(|detected| detected.extension())
            .unwrap_or_else(|| extension_for(&media.content_type, input.kind));
        let stem = input
            .filename
            .as_deref()
            .map(sanitize_filename)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(default_filename_stem);
        let path = output_dir.join(format!("{stem}.{extension}"));
        write_new_file(&path, &media.bytes)
            .await
            .map_err(|error| execution_error("generate_media", error))?;
        let sidecar = if env_flag_default("CHUTES_PROVENANCE", true) {
            let sidecar = path.with_extension(format!("{extension}.provenance.json"));
            let provenance = serde_json::json!({
                "provider": "chutes",
                "model": &input.model,
                "kind": input.kind.as_str(),
                "cord": cord.path,
                "content_type": &media.content_type,
                "cost": media.cost,
                "created_at": chrono::Utc::now().to_rfc3339(),
            });
            write_new_file(
                &sidecar,
                &serde_json::to_vec_pretty(&provenance)
                    .map_err(|error| execution_error("generate_media", error))?,
            )
            .await
            .map_err(|error| execution_error("generate_media", error))?;
            Some(sidecar)
        } else {
            None
        };

        Ok(ToolOutput::MediaArtifact(MediaArtifact {
            schema_version: MediaArtifact::SCHEMA_VERSION,
            kind: match input.kind {
                MediaKind::Image => MediaArtifactKind::Image,
                MediaKind::Video => MediaArtifactKind::Video,
                MediaKind::Music => MediaArtifactKind::Music,
                MediaKind::Speech => MediaArtifactKind::Speech,
            },
            path,
            mime_type: media.content_type,
            byte_len: media.bytes.len() as u64,
            provenance_path: sidecar,
            provider: "chutes".to_owned(),
            model: input.model,
            cost: media.cost,
        }))
    }
}

#[derive(Debug, Clone)]
struct Cord {
    name: String,
    path: String,
    method: String,
    content_type: Option<String>,
    schema: Option<serde_json::Value>,
}

fn catalog_records(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    value
        .as_array()
        .or_else(|| value.get("items").and_then(serde_json::Value::as_array))
        .or_else(|| value.get("chutes").and_then(serde_json::Value::as_array))
        .or_else(|| value.get("data").and_then(serde_json::Value::as_array))
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn summarize_model(value: &serde_json::Value) -> Option<serde_json::Value> {
    let name = first_string(value, &["name", "slug", "chute_id", "id"])?;
    let slug = first_string(value, &["slug"]);
    let tagline = first_string(value, &["tagline"]);
    let template = first_string(value, &["standard_template", "template"]);
    let searchable = format!(
        "{} {} {}",
        template.as_deref().unwrap_or_default(),
        tagline.as_deref().unwrap_or_default(),
        value.get("cords").unwrap_or(&serde_json::Value::Null)
    );
    Some(serde_json::json!({
        "id": first_string(value, &["chute_id", "id"]),
        "name": name,
        "slug": slug,
        "username": first_string(value, &["username"])
            .or_else(|| value.pointer("/user/username").and_then(serde_json::Value::as_str).map(str::to_owned)),
        "tagline": tagline,
        "template": template,
        "kind": infer_kind(&searchable),
    }))
}

fn compact_detail(value: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": first_string(value, &["chute_id", "id"]),
        "name": first_string(value, &["name", "slug", "chute_id", "id"]),
        "slug": first_string(value, &["slug"]),
        "username": first_string(value, &["username"])
            .or_else(|| value.pointer("/user/username").and_then(serde_json::Value::as_str).map(str::to_owned)),
        "tagline": first_string(value, &["tagline"]),
        "template": first_string(value, &["standard_template", "template"]),
        "invoke_base_url": invoke_base_url(value),
        "cords": parse_cords(value).into_iter().map(|cord| serde_json::json!({
            "name": cord.name,
            "path": cord.path,
            "method": cord.method,
            "output_content_type": cord.content_type,
            "input_schema": cord.schema,
        })).collect::<Vec<_>>(),
    })
}

fn parse_cords(value: &serde_json::Value) -> Vec<Cord> {
    value
        .get("cords")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|cord| {
            let path =
                first_string(cord, &["public_api_path", "path"]).unwrap_or_else(|| "/".to_owned());
            let path = if path.starts_with('/') {
                path
            } else {
                format!("/{path}")
            };
            Cord {
                name: first_string(cord, &["name"]).unwrap_or_else(|| {
                    path.trim_start_matches('/')
                        .split('/')
                        .next_back()
                        .unwrap_or("generate")
                        .to_owned()
                }),
                path,
                method: first_string(cord, &["public_api_method", "method"])
                    .unwrap_or_else(|| "POST".to_owned())
                    .to_uppercase(),
                content_type: first_string(cord, &["output_content_type"]),
                schema: first_value(cord, &["input_schema", "minimal_input_schema", "input"])
                    .or_else(|| cord.pointer("/schema/input").cloned()),
            }
        })
        .collect()
}

fn select_cord(
    detail: &serde_json::Value,
    kind: MediaKind,
    requested: Option<&str>,
) -> Result<Cord, String> {
    let cords = parse_cords(detail);
    if cords.is_empty() {
        return Err("model exposes no callable cords".to_owned());
    }
    if let Some(requested) = requested {
        let requested = requested.trim_start_matches('/').to_lowercase();
        return cords
            .into_iter()
            .find(|cord| {
                cord.name.to_lowercase() == requested
                    || cord.path.trim_start_matches('/').to_lowercase() == requested
            })
            .ok_or_else(|| format!("cord `{requested}` was not found"));
    }
    let preferences: &[&str] = match kind {
        MediaKind::Image => &["generate", "text-to-image", "text2image", "edit"],
        MediaKind::Video => &["generate", "text-to-video", "image-to-video"],
        MediaKind::Music => &["generate", "music", "text-to-music"],
        MediaKind::Speech => &["generate", "tts", "text-to-speech", "speak"],
    };
    preferences
        .iter()
        .find_map(|preferred| {
            cords
                .iter()
                .find(|cord| cord.name.eq_ignore_ascii_case(preferred))
                .cloned()
        })
        .or_else(|| cords.iter().find(|cord| cord.method == "POST").cloned())
        .or_else(|| cords.first().cloned())
        .ok_or_else(|| "model exposes no callable cords".to_owned())
}

fn invoke_base_url(value: &serde_json::Value) -> Option<String> {
    if let Some(explicit) = first_string(value, &["invocation_url", "invoke_url", "subdomain"]) {
        return Some(explicit.trim_end_matches('/').to_owned());
    }
    let slug = first_string(value, &["slug"])?;
    let username = first_string(value, &["username"]).or_else(|| {
        value
            .pointer("/user/username")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    });
    let label = username
        .filter(|username| !slug.starts_with(&format!("{username}-")))
        .map_or_else(|| slug.clone(), |username| format!("{username}-{slug}"));
    Some(format!("https://{label}.chutes.ai"))
}

/// Validates `params` against the cord's full (possibly nested) JSON Schema
/// -- not just its top-level `required`/`properties` -- so a payload that
/// gets the outer shape right (e.g. the `args` wrapper some cords require)
/// but has the wrong fields *inside* a nested object is rejected locally,
/// with a precise error, instead of round-tripping to Chutes for a generic
/// "Invalid input parameters" 400.
fn validate_schema_fields(
    schema: &Option<serde_json::Value>,
    params: &serde_json::Map<String, serde_json::Value>,
    allow_unknown: bool,
) -> Result<(), String> {
    let Some(schema) = schema.as_ref() else {
        return Ok(());
    };
    let mut schema = schema.clone();
    close_object_schemas(&mut schema, !allow_unknown);
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| format!("cord input_schema is not a valid JSON Schema: {error}"))?;
    let instance = serde_json::Value::Object(params.clone());
    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|error| {
            let path = error.instance_path.to_string();
            if path.is_empty() {
                error.to_string()
            } else {
                format!("{path}: {error}")
            }
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "params do not match the cord's input schema: {}. Call describe_media_model again to see the exact schema{}",
            errors.join("; "),
            if allow_unknown {
                ""
            } else {
                ", or set CHUTES_ALLOW_UNKNOWN_PARAMS=1 to bypass unknown-field checks"
            }
        ))
    }
}

/// Recursively force (`close = true`) `additionalProperties: false` on every
/// object schema segment that declares `properties` and doesn't already
/// restrict it itself. Chutes cord schemas rarely set `additionalProperties`
/// themselves, but this tool treats every declared-`properties` object as
/// closed by default (catches typo'd field names instead of silently
/// dropping/forwarding them) -- `CHUTES_ALLOW_UNKNOWN_PARAMS=1` disables this
/// pass entirely (`close = false`), leaving the schema exactly as Chutes
/// published it, standard JSON Schema's open-by-default semantics included.
///
/// Never overwrites a schema that already has an opinion:
/// `additionalProperties` (boolean *or* schema-valued -- either means the
/// author already restricted or intentionally allowed extra fields) and
/// `unevaluatedProperties` are both left untouched when present, so this
/// tool only ever narrows validation the schema left unspecified, never a
/// restriction (or permission) Chutes explicitly declared.
fn close_object_schemas(schema: &mut serde_json::Value, close: bool) {
    if !close {
        return;
    }
    match schema {
        serde_json::Value::Object(map) => {
            if map.contains_key("properties")
                && !map.contains_key("additionalProperties")
                && !map.contains_key("unevaluatedProperties")
            {
                map.insert("additionalProperties".to_owned(), serde_json::json!(false));
            }
            for value in map.values_mut() {
                close_object_schemas(value, close);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                close_object_schemas(item, close);
            }
        }
        _ => {}
    }
}

/// Known free-text field names that must never be treated as a file path,
/// however path-like their value happens to look. Checked at every nesting
/// level (a `caption` inside `args.caption` is excluded exactly like a
/// top-level `caption` would be), not just at the top level.
const TEXT_FIELDS: &[&str] = &[
    "prompt",
    "negative_prompt",
    "text",
    "lyrics",
    "caption",
    "description",
    "style",
];

/// Recursively walks `params` and base64-encodes any string value that
/// resolves to a real file inside the workspace, so nested wrapper shapes
/// some cords require (e.g. `{"args": {"image": "input.png"}}`) are handled
/// the same as a top-level field.
async fn encode_workspace_assets(
    params: &mut serde_json::Map<String, serde_json::Value>,
    cwd: &Path,
) -> Result<(), String> {
    let canonical_cwd = dunce::canonicalize(cwd).map_err(|error| error.to_string())?;
    for (key, value) in params.iter_mut() {
        encode_workspace_assets_in_value(key, value, &canonical_cwd).await?;
    }
    Ok(())
}

fn encode_workspace_assets_in_value<'a>(
    key: &'a str,
    value: &'a mut serde_json::Value,
    cwd: &'a Path,
) -> futures::future::BoxFuture<'a, Result<(), String>> {
    Box::pin(async move {
        if TEXT_FIELDS.contains(&key.to_lowercase().as_str()) {
            return Ok(());
        }
        match value {
            serde_json::Value::String(candidate) => {
                if let Some(encoded) = encode_workspace_file(candidate, cwd).await? {
                    *candidate = encoded;
                }
            }
            serde_json::Value::Array(items) => {
                // Array items have no field name of their own; the parent
                // key's text-field exclusion still applies to each element
                // (e.g. an `examples` array of caption strings).
                for item in items.iter_mut() {
                    encode_workspace_assets_in_value(key, item, cwd).await?;
                }
            }
            serde_json::Value::Object(map) => {
                for (child_key, child_value) in map.iter_mut() {
                    encode_workspace_assets_in_value(child_key, child_value, cwd).await?;
                }
            }
            _ => {}
        }
        Ok(())
    })
}

async fn encode_workspace_file(value: &str, cwd: &Path) -> Result<Option<String>, String> {
    if value.len() > 1_024 || value.contains('\n') {
        return Ok(None);
    }
    let candidate = Path::new(value);
    let path = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        cwd.join(candidate)
    };
    if !path.is_file() {
        return Ok(None);
    }
    let canonical = dunce::canonicalize(&path).map_err(|error| error.to_string())?;
    if !canonical.starts_with(cwd) {
        return Err(format!(
            "input asset is outside the workspace: {}",
            path.display()
        ));
    }
    let max_bytes =
        env_usize("CHUTES_MAX_INPUT_ASSET_BYTES", 64 * 1024 * 1024).clamp(1, 512 * 1024 * 1024);
    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|error| error.to_string())?;
    if metadata.len() > max_bytes as u64 {
        return Err(format!(
            "input asset exceeds the configured {max_bytes}-byte safety limit: {}",
            path.display()
        ));
    }
    let bytes = tokio::fs::read(canonical)
        .await
        .map_err(|error| error.to_string())?;
    Ok(Some(
        base64::engine::general_purpose::STANDARD.encode(bytes),
    ))
}

async fn invoke_with_cold_start_retry(
    client: &ChutesMediaClient,
    model: &str,
    url: &str,
    method: &str,
    body: &serde_json::Value,
    max_retries: usize,
) -> Result<MediaResponse, MediaError> {
    let mut attempt = 0usize;
    loop {
        match client.invoke(url, method, body).await {
            Ok(response) => return Ok(response),
            Err(error) if attempt < max_retries && is_cold_start_error(&error) => {
                attempt += 1;
                let _ = client.warmup(model).await;
                tokio::time::sleep(std::time::Duration::from_secs(8 * attempt as u64)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn env_flag_default(name: &str, default: bool) -> bool {
    std::env::var(name).ok().map_or(default, |value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}

fn is_cold_start_error(error: &MediaError) -> bool {
    match error {
        MediaError::Http { status: 503, .. } => true,
        MediaError::Http { status, body } if *status >= 500 => {
            let body = body.to_lowercase();
            ["no instances", "instance", "cold", "capacity", "not ready"]
                .iter()
                .any(|needle| body.contains(needle))
        }
        _ => false,
    }
}

async fn resolve_media_response(
    client: &ChutesMediaClient,
    response: MediaResponse,
    kind: MediaKind,
) -> Result<MediaResponse, String> {
    if !response.content_type.to_lowercase().contains("json") {
        return Ok(response);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&response.bytes).map_err(|error| error.to_string())?;
    let candidate = find_asset_string(&value)
        .ok_or_else(|| "model returned JSON without a recognizable media asset".to_owned())?;
    if let Some((metadata, encoded)) = candidate
        .strip_prefix("data:")
        .and_then(|value| value.split_once(','))
    {
        let content_type = metadata
            .split(';')
            .next()
            .unwrap_or(default_content_type(kind));
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|error| error.to_string())?;
        return Ok(MediaResponse {
            bytes,
            content_type: content_type.to_owned(),
            cost: response.cost,
        });
    }
    if candidate.starts_with("https://") || candidate.starts_with("http://") {
        let mut downloaded = client
            .download(candidate)
            .await
            .map_err(|error| error.to_string())?;
        downloaded.cost = response.cost.or(downloaded.cost);
        return Ok(downloaded);
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(candidate.trim())
        .map_err(|error| error.to_string())?;
    Ok(MediaResponse {
        bytes,
        content_type: default_content_type(kind).to_owned(),
        cost: response.cost,
    })
}

fn find_asset_string(value: &serde_json::Value) -> Option<&str> {
    const KEYS: &[&str] = &[
        "url",
        "image_url",
        "video_url",
        "audio_url",
        "result_url",
        "output_url",
        "b64_json",
        "base64",
        "image",
        "video",
        "audio",
        "output",
        "result",
        "data",
    ];
    match value {
        serde_json::Value::String(value) if !value.is_empty() => Some(value),
        serde_json::Value::Array(values) => values.iter().find_map(find_asset_string),
        serde_json::Value::Object(values) => KEYS.iter().find_map(|key| {
            values.get(*key).and_then(|value| match value {
                serde_json::Value::String(value) if !value.is_empty() => Some(value.as_str()),
                value => find_asset_string(value),
            })
        }),
        _ => None,
    }
}

async fn secure_output_dir(
    cwd: &Path,
    requested: Option<&str>,
    kind: MediaKind,
) -> Result<PathBuf, String> {
    let canonical_cwd = dunce::canonicalize(cwd).map_err(|error| error.to_string())?;
    let requested = requested
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets").join("chutes").join(kind.as_str()));
    if requested.is_absolute()
        || requested.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("output_dir must be a relative path inside the workspace".to_owned());
    }
    let target = canonical_cwd.join(requested);
    let mut existing_ancestor = target.as_path();
    while !existing_ancestor.exists() {
        existing_ancestor = existing_ancestor
            .parent()
            .ok_or_else(|| "output_dir has no existing workspace ancestor".to_owned())?;
    }
    let canonical_ancestor =
        dunce::canonicalize(existing_ancestor).map_err(|error| error.to_string())?;
    if !canonical_ancestor.starts_with(&canonical_cwd) {
        return Err("output_dir must stay inside the current workspace".to_owned());
    }
    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|error| error.to_string())?;
    let canonical_target = dunce::canonicalize(&target).map_err(|error| error.to_string())?;
    if !canonical_target.starts_with(&canonical_cwd) {
        return Err("output_dir must stay inside the current workspace".to_owned());
    }
    Ok(canonical_target)
}

async fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(bytes).await?;
    file.flush().await
}

fn assert_content_type(kind: MediaKind, content_type: &str) -> Result<(), String> {
    let content_type = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if content_type.is_empty() || content_type.ends_with("/octet-stream") {
        return Ok(());
    }
    let expected = match kind {
        MediaKind::Image => "image/",
        MediaKind::Video => "video/",
        MediaKind::Music | MediaKind::Speech => "audio/",
    };
    if ["image/", "video/", "audio/"]
        .iter()
        .any(|family| content_type.starts_with(family))
        && !content_type.starts_with(expected)
    {
        return Err(format!(
            "model returned `{content_type}`, expected `{expected}*`"
        ));
    }
    Ok(())
}

/// Confirms `bytes` actually look like the requested media family before
/// they're saved to disk, using magic-byte detection rather than trusting
/// only the (server- or proxy-controlled) declared `Content-Type` header.
/// Chutes error responses are typically HTML or JSON; neither has a
/// reliable magic-byte signature (`infer` can't identify them), so those are
/// caught separately by a lightweight textual sniff.
///
/// Returns the sniffed type on success (`None` when the bytes are a
/// recognized family member but not one `infer` has a matcher for, or when
/// they're inconclusive but pass through anyway -- e.g. a real WAV/FLAC
/// variant `infer` doesn't cover) so the caller can prefer it over the
/// declared header when choosing a filename extension.
fn verify_media_content(kind: MediaKind, bytes: &[u8]) -> Result<Option<infer::Type>, String> {
    if bytes.is_empty() {
        return Err("model returned an empty response body".to_owned());
    }
    if looks_like_text_error_document(bytes) {
        return Err(format!(
            "model returned what looks like an HTML/JSON/text document instead of {} data",
            kind.as_str()
        ));
    }
    let Some(detected) = infer::get(bytes) else {
        // No magic-byte match at all (e.g. some raw PCM/less-common audio
        // containers): not proof of anything either way, fall through to
        // the header-based check already run by the caller.
        return Ok(None);
    };
    let expected_family = match kind {
        MediaKind::Image => "image/",
        MediaKind::Video => "video/",
        MediaKind::Music | MediaKind::Speech => "audio/",
    };
    if !detected.mime_type().starts_with(expected_family) {
        return Err(format!(
            "response bytes are `{}` (detected from content), expected `{expected_family}*`",
            detected.mime_type()
        ));
    }
    Ok(Some(detected))
}

/// Cheap prefix sniff for HTML/JSON error bodies, which `infer`'s
/// magic-byte matchers can't catch (plain text has no binary signature).
/// Only a short prefix is UTF-8-decoded; real binary media essentially
/// never happens to be valid UTF-8 there, so this is a fast, low-risk check
/// that doesn't require decoding the whole (possibly huge) response.
fn looks_like_text_error_document(bytes: &[u8]) -> bool {
    let prefix_len = bytes.len().min(512);
    let Ok(text) = std::str::from_utf8(&bytes[..prefix_len]) else {
        return false;
    };
    let trimmed = text.trim_start();
    if trimmed.starts_with('<') {
        return true;
    }
    (trimmed.starts_with('{') || trimmed.starts_with('['))
        && serde_json::from_slice::<serde_json::Value>(bytes).is_ok()
}

fn extension_for(content_type: &str, kind: MediaKind) -> &'static str {
    match content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase()
        .as_str()
    {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" => "ogg",
        "audio/flac" => "flac",
        _ => match kind {
            MediaKind::Image => "png",
            MediaKind::Video => "mp4",
            MediaKind::Music => "mp3",
            MediaKind::Speech => "wav",
        },
    }
}

fn default_content_type(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "image/png",
        MediaKind::Video => "video/mp4",
        MediaKind::Music => "audio/mpeg",
        MediaKind::Speech => "audio/wav",
    }
}

fn infer_kind(text: &str) -> Option<&'static str> {
    let text = text.to_lowercase();
    if text.contains("video") || text.contains("text2video") || text.contains("image2video") {
        Some("video")
    } else if ["tts", "text-to-speech", "speech", "voice", "speak"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some("speech")
    } else if ["music", "song", "melody", "diffrhythm", "ace-step"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some("music")
    } else if ["image", "diffusion", "flux", "text2image", "sdxl"]
        .iter()
        .any(|term| text.contains(term))
    {
        Some("image")
    } else {
        None
    }
}

fn first_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_owned)
}

fn first_value(value: &serde_json::Value, keys: &[&str]) -> Option<serde_json::Value> {
    keys.iter().find_map(|key| value.get(*key).cloned())
}

fn sanitize_filename(value: &str) -> String {
    let stem = Path::new(value)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    stem.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['.', '-'])
        .to_owned()
}

fn default_filename_stem() -> String {
    format!(
        "asset-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f"),
        uuid::Uuid::new_v4().simple()
    )
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

fn read_only_capabilities() -> xai_tool_protocol::ToolCapabilities {
    xai_tool_protocol::ToolCapabilities {
        is_read_only: true,
        tool_scope: Some(xai_tool_protocol::ToolScope::Read),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that call `encode_workspace_assets`/
    /// `encode_workspace_file` against the process-global
    /// `CHUTES_MAX_INPUT_ASSET_BYTES` env var, so the one test that mutates
    /// it can't make an unrelated concurrently-running test see a bogus
    /// byte limit.
    async fn with_asset_encoding_lock<F, Fut, R>(f: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = LOCK.lock().await;
        f().await
    }

    #[test]
    fn invoke_url_does_not_duplicate_owner() {
        let detail = serde_json::json!({ "username": "mike", "slug": "mike-flux" });
        assert_eq!(
            invoke_base_url(&detail).as_deref(),
            Some("https://mike-flux.chutes.ai")
        );
    }

    #[test]
    fn output_filename_is_sanitized() {
        assert_eq!(sanitize_filename("../my unsafe/image.png"), "image");
    }

    #[test]
    fn accepts_real_media_bytes_for_each_family() {
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR";
        let jpeg = b"\xff\xd8\xff\xe0\x00\x10JFIF";
        let webp = b"RIFF\x00\x00\x00\x00WEBPVP8 ";
        let gif = b"GIF89a\x01\x00\x01\x00";
        let mp4 = b"\x00\x00\x00\x20ftypisom\x00\x00\x02\x00isomiso2avc1mp41";
        let webm = b"\x1a\x45\xdf\xa3\x9f\x42\x86\x81\x01";
        let wav = b"RIFF\x00\x00\x00\x00WAVEfmt ";
        let mp3 = b"ID3\x03\x00\x00\x00\x00\x00\x00";
        let ogg = b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00";
        let flac = b"fLaC\x00\x00\x00\x22";

        for (bytes, kind, label) in [
            (&png[..], MediaKind::Image, "png"),
            (&jpeg[..], MediaKind::Image, "jpeg"),
            (&webp[..], MediaKind::Image, "webp"),
            (&gif[..], MediaKind::Image, "gif"),
            (&mp4[..], MediaKind::Video, "mp4"),
            (&webm[..], MediaKind::Video, "webm"),
            (&wav[..], MediaKind::Music, "wav"),
            (&mp3[..], MediaKind::Music, "mp3"),
            (&ogg[..], MediaKind::Music, "ogg"),
            (&flac[..], MediaKind::Music, "flac"),
        ] {
            let result = verify_media_content(kind, bytes);
            assert!(result.is_ok(), "{label} should be accepted: {result:?}");
        }
    }

    #[test]
    fn rejects_html_error_page() {
        let body = b"<!DOCTYPE html><html><body>502 Bad Gateway</body></html>";
        assert!(verify_media_content(MediaKind::Image, body).is_err());
    }

    #[test]
    fn rejects_json_error_payload() {
        let body = br#"{"error": "no instances available"}"#;
        assert!(verify_media_content(MediaKind::Image, body).is_err());
    }

    #[test]
    fn rejects_empty_response() {
        assert!(verify_media_content(MediaKind::Image, b"").is_err());
    }

    #[test]
    fn rejects_wrong_media_family_by_content() {
        let wav = b"RIFF\x00\x00\x00\x00WAVEfmt ";
        // Real audio bytes returned when an image was requested must still
        // be rejected -- the declared header isn't the only thing checked.
        assert!(verify_media_content(MediaKind::Image, wav).is_err());
    }

    #[test]
    fn octet_stream_with_real_media_bytes_is_identified_and_accepted() {
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR";
        let detected = verify_media_content(MediaKind::Image, png).unwrap();
        assert_eq!(detected.unwrap().mime_type(), "image/png");
    }

    #[test]
    fn schema_guard_catches_wrong_fields_nested_inside_a_required_wrapper() {
        // Mirrors a real cord shape: the top-level payload must wrap the
        // actual arguments in `args`, and `args` itself has its own
        // required/known fields. The old top-level-only checker accepted
        // any payload with an `args` key, regardless of what was inside it.
        let schema = Some(serde_json::json!({
            "type": "object",
            "required": ["args"],
            "properties": {
                "args": {
                    "type": "object",
                    "required": ["prompt"],
                    "properties": {
                        "prompt": { "type": "string" },
                        "size": { "type": "string" }
                    }
                }
            }
        }));
        let correct = serde_json::json!({"args": {"prompt": "a cat"}})
            .as_object()
            .cloned()
            .unwrap();
        assert!(validate_schema_fields(&schema, &correct, false).is_ok());

        let missing_nested = serde_json::json!({"args": {"size": "1024x1024"}})
            .as_object()
            .cloned()
            .unwrap();
        assert!(validate_schema_fields(&schema, &missing_nested, false).is_err());

        let unknown_nested = serde_json::json!({"args": {"prompt": "a cat", "typo": true}})
            .as_object()
            .cloned()
            .unwrap();
        assert!(validate_schema_fields(&schema, &unknown_nested, false).is_err());
        assert!(validate_schema_fields(&schema, &unknown_nested, true).is_ok());
    }

    #[test]
    fn schema_guard_rejects_missing_and_unknown_fields() {
        let schema = Some(serde_json::json!({
            "required": ["prompt"],
            "properties": { "prompt": { "type": "string" } }
        }));
        let missing = serde_json::Map::new();
        assert!(validate_schema_fields(&schema, &missing, false).is_err());

        let mut unknown = serde_json::Map::new();
        unknown.insert("prompt".to_owned(), serde_json::json!("hello"));
        unknown.insert("typo".to_owned(), serde_json::json!(true));
        assert!(validate_schema_fields(&schema, &unknown, false).is_err());
        assert!(validate_schema_fields(&schema, &unknown, true).is_ok());
    }

    #[tokio::test]
    async fn output_directory_cannot_escape_workspace() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            secure_output_dir(root.path(), Some("../outside"), MediaKind::Image)
                .await
                .is_err()
        );
        let outside = tempfile::tempdir().unwrap();
        assert!(
            secure_output_dir(root.path(), outside.path().to_str(), MediaKind::Image)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn generated_media_does_not_overwrite_existing_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("asset.png");
        write_new_file(&path, b"first").await.unwrap();
        assert!(write_new_file(&path, b"second").await.is_err());
        assert_eq!(tokio::fs::read(path).await.unwrap(), b"first");
    }

    #[test]
    fn schema_close_preserves_explicit_additional_properties_true() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": { "prompt": { "type": "string" } },
            "additionalProperties": true
        });
        close_object_schemas(&mut schema, true);
        assert_eq!(schema["additionalProperties"], serde_json::json!(true));
    }

    #[test]
    fn schema_close_preserves_schema_valued_additional_properties() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": { "prompt": { "type": "string" } },
            "additionalProperties": { "type": "string" }
        });
        close_object_schemas(&mut schema, true);
        assert_eq!(
            schema["additionalProperties"],
            serde_json::json!({ "type": "string" })
        );
    }

    #[test]
    fn schema_close_preserves_unevaluated_properties() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": { "prompt": { "type": "string" } },
            "unevaluatedProperties": false
        });
        close_object_schemas(&mut schema, true);
        assert!(
            !schema
                .as_object()
                .unwrap()
                .contains_key("additionalProperties"),
            "must not add additionalProperties alongside an explicit unevaluatedProperties: {schema}"
        );
    }

    #[test]
    fn schema_close_still_closes_schemas_with_no_explicit_opinion() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": { "prompt": { "type": "string" } }
        });
        close_object_schemas(&mut schema, true);
        assert_eq!(schema["additionalProperties"], serde_json::json!(false));
    }

    #[test]
    fn allow_unknown_leaves_the_schema_completely_untouched() {
        // CHUTES_ALLOW_UNKNOWN_PARAMS must never *remove* a restriction the
        // provider declared -- it only disables this tool's own default.
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": { "prompt": { "type": "string" } },
            "additionalProperties": false
        });
        let original = schema.clone();
        close_object_schemas(&mut schema, false);
        assert_eq!(
            schema, original,
            "allow_unknown must not alter provider schema"
        );
    }

    #[tokio::test]
    async fn encodes_asset_nested_inside_an_object_wrapper() {
        with_asset_encoding_lock(|| async {
            let root = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("input.png"), b"fake png bytes").unwrap();
            let mut params = serde_json::json!({ "args": { "image": "input.png" } })
                .as_object()
                .cloned()
                .unwrap();
            encode_workspace_assets(&mut params, root.path())
                .await
                .unwrap();
            let encoded = params["args"]["image"].as_str().unwrap();
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .unwrap(),
                b"fake png bytes"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn encodes_asset_nested_inside_an_array_of_objects() {
        with_asset_encoding_lock(|| async {
            let root = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("a.png"), b"a-bytes").unwrap();
            std::fs::write(root.path().join("b.png"), b"b-bytes").unwrap();
            let mut params = serde_json::json!({
                "images": [{ "path": "a.png" }, { "path": "b.png" }]
            })
            .as_object()
            .cloned()
            .unwrap();
            encode_workspace_assets(&mut params, root.path())
                .await
                .unwrap();
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(params["images"][0]["path"].as_str().unwrap())
                    .unwrap(),
                b"a-bytes"
            );
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(params["images"][1]["path"].as_str().unwrap())
                    .unwrap(),
                b"b-bytes"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn text_fields_are_never_treated_as_paths_even_when_nested() {
        with_asset_encoding_lock(|| async {
            let root = tempfile::tempdir().unwrap();
            // A file that happens to exist at this relative path -- if the
            // exclusion didn't apply at nested levels too, this would
            // wrongly get base64-encoded instead of staying literal prompt
            // text.
            std::fs::write(root.path().join("a cat"), b"decoy").unwrap();
            let mut params = serde_json::json!({ "args": { "prompt": "a cat" } })
                .as_object()
                .cloned()
                .unwrap();
            encode_workspace_assets(&mut params, root.path())
                .await
                .unwrap();
            assert_eq!(params["args"]["prompt"], serde_json::json!("a cat"));
        })
        .await;
    }

    #[tokio::test]
    async fn rejects_asset_outside_the_workspace() {
        with_asset_encoding_lock(|| async {
            let root = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let outside_file = outside.path().join("secret.png");
            std::fs::write(&outside_file, b"outside bytes").unwrap();
            let mut params = serde_json::json!({ "image": outside_file.to_str().unwrap() })
                .as_object()
                .cloned()
                .unwrap();
            assert!(
                encode_workspace_assets(&mut params, root.path())
                    .await
                    .is_err()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn rejects_oversized_asset() {
        with_asset_encoding_lock(|| async {
            let root = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("big.bin"), vec![0u8; 4096]).unwrap();
            // SAFETY: serialized by `with_asset_encoding_lock` against every
            // other test in this module that reads this env var.
            unsafe {
                std::env::set_var("CHUTES_MAX_INPUT_ASSET_BYTES", "10");
            }
            let mut params = serde_json::json!({ "image": "big.bin" })
                .as_object()
                .cloned()
                .unwrap();
            let result = encode_workspace_assets(&mut params, root.path()).await;
            unsafe {
                std::env::remove_var("CHUTES_MAX_INPUT_ASSET_BYTES");
            }
            assert!(result.is_err());
        })
        .await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_symlink_escaping_the_workspace() {
        with_asset_encoding_lock(|| async {
            let root = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let target = outside.path().join("secret.png");
            std::fs::write(&target, b"outside bytes").unwrap();
            let link = root.path().join("escape.png");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            let mut params = serde_json::json!({ "image": "escape.png" })
                .as_object()
                .cloned()
                .unwrap();
            assert!(
                encode_workspace_assets(&mut params, root.path())
                    .await
                    .is_err()
            );
        })
        .await;
    }

    #[tokio::test]
    async fn relative_and_absolute_in_workspace_paths_both_resolve() {
        with_asset_encoding_lock(|| async {
            let root = tempfile::tempdir().unwrap();
            let canonical_root = dunce::canonicalize(root.path()).unwrap();
            std::fs::write(canonical_root.join("rel.png"), b"rel-bytes").unwrap();
            std::fs::write(canonical_root.join("abs.png"), b"abs-bytes").unwrap();
            let abs_path = canonical_root.join("abs.png").to_str().unwrap().to_owned();
            let mut params = serde_json::json!({
                "rel": "rel.png",
                "abs": abs_path,
            })
            .as_object()
            .cloned()
            .unwrap();
            encode_workspace_assets(&mut params, root.path())
                .await
                .unwrap();
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(params["rel"].as_str().unwrap())
                    .unwrap(),
                b"rel-bytes"
            );
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(params["abs"].as_str().unwrap())
                    .unwrap(),
                b"abs-bytes"
            );
        })
        .await;
    }
}
