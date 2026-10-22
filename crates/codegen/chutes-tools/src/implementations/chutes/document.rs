//! Document OCR and structured extraction for workspace PDFs and images
//! via the docuextract chute.
//!
//! Three modes over one document: `markdown` OCR (fast Lighton default,
//! higher-quality NuExtract), reusable extraction `template` creation, and
//! `extract` — template-driven structured records. Unlike `ocr_page` (one
//! verbatim page through a vision model), this tool handles multi-page PDFs
//! natively and returns structure, not just text.

use chutes_build_core::docs::{
    DEFAULT_DOC_SLUG, DocCommon, DocEngine, DocInput, create_template, extract_markdown,
    extract_with_template, markdown_text,
};
use chutes_build_core::media::ChutesMediaClient;

use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::{Cwd, DisplayCwd, FileSystem, resolve_model_path};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Default cap for the document read into memory, shared with the media
/// asset limit so one knob (`CHUTES_MAX_INPUT_ASSET_BYTES`) governs both.
fn max_input_bytes() -> usize {
    std::env::var("CHUTES_MAX_INPUT_ASSET_BYTES")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(64 * 1024 * 1024)
        .clamp(1, 512 * 1024 * 1024)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExtractDocumentInput {
    /// Path to a PDF or image file, relative to the working directory or
    /// absolute. Must stay inside the workspace. Required for `markdown` and
    /// `template` modes; `extract` mode accepts it or `text`.
    pub path: Option<String>,
    /// Raw document text for `extract` mode when no file is involved.
    pub text: Option<String>,
    /// `markdown` (default): OCR the document to markdown. `template`:
    /// create a reusable extraction template from `description`.
    /// `extract`: fill a `template` with structured records.
    pub mode: Option<String>,
    /// OCR engine for `markdown` mode: `lighton` (default, fast) or
    /// `nuextract` (higher quality, reasoning mode for hard documents).
    pub engine: Option<String>,
    /// Chute slug override when the default docuextract chute is
    /// unavailable. Must expose compatible `/ocr/*` cords.
    pub model: Option<String>,
    /// What to extract, for `template` mode (required there). Example:
    /// "invoice number, date, line items with quantity and price".
    pub description: Option<String>,
    /// Template object from `template` mode, for `extract` mode (required
    /// there). Pass the `template` value from a previous call verbatim.
    pub template: Option<serde_json::Value>,
    /// Extra extraction guidance for `extract` mode.
    pub instructions: Option<String>,
    /// Few-shot examples for `extract` mode, as a JSON value.
    pub examples: Option<serde_json::Value>,
    /// Maximum PDF pages to process. Server default applies when omitted
    /// (10 for OCR, 1 for template creation).
    pub max_pages: Option<u32>,
    /// Sampling temperature override. Server default applies when omitted.
    pub temperature: Option<f64>,
}

#[derive(Debug, Default)]
pub struct ExtractDocumentTool;

impl crate::types::tool_metadata::ToolMetadata for ExtractDocumentTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::ChutesBuild
    }

    fn description_template(&self) -> &str {
        "Extract text and structure from PDFs and images via dedicated Chutes document models: \
         markdown OCR (fast Lighton default, higher-quality NuExtract), reusable extraction \
         templates, and template-driven structured extraction. Prefer over ocr_page for multi-page \
         PDFs and structured data; ocr_page stays for single-image verbatim transcription."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl chutes_tool_runtime::Tool for ExtractDocumentTool {
    type Args = ExtractDocumentInput;
    type Output = ToolOutput;

    fn id(&self) -> chutes_tool_protocol::ToolId {
        tool_id("extract_document")
    }

    fn description(
        &self,
        _: &chutes_tool_runtime::ListToolsContext,
    ) -> chutes_tool_types::ToolDescription {
        chutes_tool_types::ToolDescription::new(
            "extract_document",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> chutes_tool_protocol::ToolCapabilities {
        read_only_capabilities()
    }

    async fn run(
        &self,
        ctx: chutes_tool_runtime::ToolCallContext,
        input: ExtractDocumentInput,
    ) -> Result<ToolOutput, chutes_tool_runtime::ToolError> {
        let mode = input
            .mode
            .as_deref()
            .map(str::trim)
            .filter(|mode| !mode.is_empty())
            .unwrap_or("markdown")
            .to_ascii_lowercase();
        if !["markdown", "template", "extract"].contains(&mode.as_str()) {
            return Err(chutes_tool_runtime::ToolError::invalid_arguments(format!(
                "unknown mode `{mode}` (expected markdown, template, or extract)"
            )));
        }
        let engine = input
            .engine
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(DocEngine::parse)
            .unwrap_or(Some(DocEngine::default()))
            .ok_or_else(|| {
                chutes_tool_runtime::ToolError::invalid_arguments(format!(
                    "unknown OCR engine `{}` (expected lighton or nuextract)",
                    input.engine.as_deref().unwrap_or_default()
                ))
            })?;
        let slug = input
            .model
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(DEFAULT_DOC_SLUG);

        // Extract mode can run on raw text; every other mode needs a file.
        let mut doc = DocInput::default();
        let mut source_path: Option<String> = None;
        if let Some(path) = input
            .path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            let (file_bytes, is_pdf) = read_document_file(&ctx, path).await?;
            let max_bytes = max_input_bytes();
            if file_bytes.len() > max_bytes {
                return Err(chutes_tool_runtime::ToolError::invalid_arguments(format!(
                    "document exceeds the {max_bytes}-byte input limit; split it or raise CHUTES_MAX_INPUT_ASSET_BYTES"
                )));
            }
            let encoded =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &file_bytes);
            if is_pdf {
                doc.pdf_b64 = Some(encoded);
            } else {
                doc.image_b64 = Some(encoded);
            }
            source_path = Some(path.to_owned());
        } else if let Some(text) = input
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            doc.text = Some(text.to_owned());
        }
        if mode != "extract" && source_path.is_none() {
            return Err(chutes_tool_runtime::ToolError::invalid_arguments(format!(
                "`{mode}` mode needs a `path` to a PDF or image file"
            )));
        }

        let credentials = super::resolve_credentials(&ctx)
            .map_err(|error| execution_error("extract_document", error))?;
        let client = ChutesMediaClient::from_credentials(credentials)
            .map_err(|error| execution_error("extract_document", error))?;
        let detail = client.describe(slug).await.map_err(|error| {
            execution_error(
                "extract_document",
                format!(
                    "document chute `{slug}` is not reachable ({error}); use list_media_models with a document query to find an equivalent OCR chute"
                ),
            )
        })?;
        let base_url = super::media::invoke_base_url(&detail).ok_or_else(|| {
            execution_error(
                "extract_document",
                format!("document chute `{slug}` publishes no invocation URL"),
            )
        })?;

        let common = DocCommon {
            dpi: None,
            max_pages: input.max_pages,
            temperature: input.temperature,
            max_new_tokens: None,
            enable_thinking: false,
        };
        let raw = match mode.as_str() {
            "markdown" => extract_markdown(&client, &base_url, &doc, engine, None, &common)
                .await
                .map_err(|error| execution_error("extract_document", error))?,
            "template" => {
                let description = input.description.as_deref().unwrap_or("");
                create_template(&client, &base_url, &doc, description, &common)
                    .await
                    .map_err(|error| execution_error("extract_document", error))?
            }
            _ => {
                let template = input.template.as_ref().ok_or_else(|| {
                    chutes_tool_runtime::ToolError::invalid_arguments(
                        "extract mode needs the `template` object from a template-mode call",
                    )
                })?;
                extract_with_template(
                    &client,
                    &base_url,
                    &doc,
                    template,
                    input.instructions.as_deref(),
                    input.examples.as_ref(),
                    &common,
                )
                .await
                .map_err(|error| execution_error("extract_document", error))?
            }
        };

        let mut output = serde_json::Map::new();
        output.insert("mode".to_owned(), serde_json::Value::String(mode));
        output.insert(
            "engine".to_owned(),
            serde_json::Value::String(engine.as_str().to_owned()),
        );
        output.insert(
            "model".to_owned(),
            serde_json::Value::String(slug.to_owned()),
        );
        if let Some(path) = source_path {
            output.insert("path".to_owned(), serde_json::Value::String(path));
        }
        if let Some(text) = markdown_text(&raw) {
            output.insert("text".to_owned(), serde_json::Value::String(text));
        }
        output.insert("result".to_owned(), raw);
        pretty_text(serde_json::Value::Object(output), "extract_document")
    }
}

/// Read a workspace PDF or image file, rejecting anything else. Returns the
/// bytes plus whether the file is a PDF (by content sniffing, like ocr_page).
async fn read_document_file(
    ctx: &chutes_tool_runtime::ToolCallContext,
    path: &str,
) -> Result<(Vec<u8>, bool), chutes_tool_runtime::ToolError> {
    let resources = crate::types::tool_metadata::shared_resources(ctx)?;
    let (cwd, display_cwd, fs);
    {
        let res = resources.lock().await;
        cwd = res.require::<Cwd>()?.0.clone();
        display_cwd = res.get::<DisplayCwd>().map(|d| d.0.clone());
        fs = res.require::<FileSystem>()?.0.clone();
    }
    let joined = resolve_model_path(&cwd, display_cwd.as_deref(), path);
    let canonical = crate::util::fs::try_canonicalize(&joined)
        .await
        .unwrap_or(joined);
    let file_bytes = fs.read_file(&canonical).await.map_err(|error| {
        execution_error(
            "extract_document",
            format!("failed to read {}: {error}", canonical.display()),
        )
    })?;
    let metadata = crate::implementations::read_file::bytes_to_metadata(&file_bytes)?;
    let extension = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    if crate::implementations::read_file::is_pdf_file(&file_bytes, extension) {
        return Ok((file_bytes, true));
    }
    if metadata.is_image() {
        return Ok((file_bytes, false));
    }
    Err(chutes_tool_runtime::ToolError::invalid_arguments(format!(
        "{} is neither an image nor a PDF (detected: {})",
        canonical.display(),
        metadata.mime_type
    )))
}

fn pretty_text(
    value: serde_json::Value,
    tool: &str,
) -> Result<ToolOutput, chutes_tool_runtime::ToolError> {
    serde_json::to_string_pretty(&value)
        .map(|text| ToolOutput::Text(text.into()))
        .map_err(|error| execution_error(tool, error))
}

fn tool_id(name: &str) -> chutes_tool_protocol::ToolId {
    chutes_tool_protocol::ToolId::new(name).expect("valid tool id")
}

fn execution_error(tool: &str, error: impl std::fmt::Display) -> chutes_tool_runtime::ToolError {
    chutes_tool_runtime::ToolError::execution(tool_id(tool), error.to_string())
}

fn read_only_capabilities() -> chutes_tool_protocol::ToolCapabilities {
    chutes_tool_protocol::ToolCapabilities {
        is_read_only: true,
        tool_scope: Some(chutes_tool_protocol::ToolScope::Read),
        ..Default::default()
    }
}
