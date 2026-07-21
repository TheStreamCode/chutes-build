//! On-demand verbatim text transcription for a single image or PDF page.
//!
//! Unlike the automatic vision-delegation pipeline (which narrates images
//! into the conversation when the active chat model can't see them), this
//! tool always calls a dedicated Chutes-hosted vision model directly, works
//! regardless of which model is active in the session, and returns only the
//! extracted text -- the image itself is never added to the conversation.

use base64::Engine as _;
use chutes_build_core::vision::ChutesVisionClient;

use crate::types::output::{ReadFileOutput, ToolOutput};
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::{Cwd, DisplayCwd, FileSystem, resolve_model_path};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Default vision model for transcription. Pinned to a concrete model rather
/// than the virtual `model-router` id: OCR fidelity (character-exact
/// transcription, not narrative description) is the point of this tool, and
/// only this concrete model's transcription quality has been verified.
const DEFAULT_OCR_MODEL: &str = "google/gemma-4-31B-turbo-TEE";

const TRANSCRIBE_PROMPT: &str = "Transcribe verbatim, exactly, every character of text visible \
    in this image. Output only the transcribed text, nothing else.";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct OcrPageInput {
    /// Path to an image or PDF file, relative to the working directory or absolute.
    pub path: String,
    /// 1-based page number for PDFs. Ignored for plain image files. Defaults to 1.
    pub page: Option<usize>,
    /// Override the default vision model used for transcription.
    pub model: Option<String>,
}

#[derive(Debug, Default)]
pub struct OcrPageTool;

impl crate::types::tool_metadata::ToolMetadata for OcrPageTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Extract text verbatim from a single image or PDF page via a dedicated Chutes vision \
         model, independent of the active chat model's vision support. Returns extracted text \
         only -- the image itself is never added to the conversation."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for OcrPageTool {
    type Args = OcrPageInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        tool_id("ocr_page")
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "ocr_page",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        read_only_capabilities()
    }

    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: OcrPageInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        let resources = crate::types::tool_metadata::shared_resources(&ctx)?;
        let (cwd, display_cwd, fs);
        {
            let res = resources.lock().await;
            cwd = res.require::<Cwd>()?.0.clone();
            display_cwd = res.get::<DisplayCwd>().map(|d| d.0.clone());
            fs = res.require::<FileSystem>()?.0.clone();
        }

        let joined = resolve_model_path(&cwd, display_cwd.as_deref(), &input.path);
        let path = crate::util::fs::try_canonicalize(&joined)
            .await
            .unwrap_or(joined);

        let file_bytes = fs.read_file(&path).await.map_err(|error| {
            execution_error(
                "ocr_page",
                format!("failed to read {}: {error}", path.display()),
            )
        })?;

        let metadata = crate::implementations::read_file::bytes_to_metadata(&file_bytes)?;
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        let (image_bytes, mime, total_pages) =
            if crate::implementations::read_file::is_pdf_file(&file_bytes, extension) {
                let page = input.page.unwrap_or(1);
                let file_size = file_bytes.len();
                let output = crate::implementations::read_file::pdf::render_pdf_pages(
                    file_bytes,
                    Some(&page.to_string()),
                    file_size,
                )
                .map_err(|error| execution_error("ocr_page", error))?;
                let ReadFileOutput::PdfPageImages(images) = output else {
                    return Err(execution_error("ocr_page", "unexpected PDF render output"));
                };
                let rendered = images.pages.into_iter().next().ok_or_else(|| {
                    execution_error("ocr_page", format!("page {page} not found in PDF"))
                })?;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&rendered.data)
                    .map_err(|error| execution_error("ocr_page", error))?;
                (bytes, rendered.mime_type, Some(images.total_pages))
            } else if metadata.is_image() {
                (file_bytes, metadata.mime_type.clone(), None)
            } else {
                return Err(xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "{} is neither an image nor a PDF (detected: {})",
                    path.display(),
                    metadata.mime_type
                )));
            };

        let (compressed_bytes, compressed_mime) =
            crate::implementations::read_file::compress_image_for_conversation(image_bytes, mime)
                .map_err(|error| execution_error("ocr_page", error))?;

        let model = input.model.as_deref().unwrap_or(DEFAULT_OCR_MODEL);
        let client =
            ChutesVisionClient::from_env().map_err(|error| execution_error("ocr_page", error))?;
        let result = client
            .transcribe(
                model,
                &compressed_mime,
                &compressed_bytes,
                TRANSCRIBE_PROMPT,
            )
            .await
            .map_err(|error| execution_error("ocr_page", error))?;

        pretty_text(
            serde_json::json!({
                "text": result.text,
                "path": path,
                "page": input.page.unwrap_or(1),
                "total_pages": total_pages,
                "model": model,
                "truncated": result.truncated,
            }),
            "ocr_page",
        )
    }
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

    #[test]
    fn default_model_is_not_the_virtual_router() {
        // OCR fidelity is the point of this tool; only a concrete, verified
        // model is used by default -- see DEFAULT_OCR_MODEL's doc comment.
        assert_ne!(DEFAULT_OCR_MODEL, "model-router");
    }

    #[test]
    fn prompt_asks_for_verbatim_transcription() {
        assert!(TRANSCRIBE_PROMPT.to_lowercase().contains("verbatim"));
    }

    /// Exercises the full path-resolution -> compression -> live Chutes
    /// vision-model call, end to end. Transcription *quality* against real
    /// text was separately verified by hand against this exact endpoint
    /// shape; this test only proves the tool's wiring is correct.
    #[tokio::test]
    #[ignore = "hits the live Chutes API; run manually with CHUTES_API_KEY set"]
    async fn live_ocr_page_round_trips_a_real_image() {
        if std::env::var("CHUTES_API_KEY").is_err() {
            eprintln!("skipping: CHUTES_API_KEY not set");
            return;
        }
        use crate::computer::local::LocalFs;
        use crate::types::resources::Resources;
        use crate::types::tool_metadata::test_ctx;
        use std::sync::Arc;

        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("test.png");
        let png = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 255, 255]));
        png.save(&file_path).unwrap();

        let mut resources = Resources::new();
        resources.insert(Cwd(tmp.path().to_path_buf()));
        resources.insert(FileSystem(Arc::new(LocalFs)));
        let shared = resources.into_shared();

        let tool = OcrPageTool;
        let input = OcrPageInput {
            path: "test.png".to_string(),
            page: None,
            model: None,
        };
        let result = xai_tool_runtime::Tool::run(&tool, test_ctx(shared), input)
            .await
            .expect("live OCR call should succeed");
        let ToolOutput::Text(text) = result else {
            panic!("expected ToolOutput::Text, got {result:?}");
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();
        assert!(
            parsed.get("text").and_then(|v| v.as_str()).is_some(),
            "response missing text field: {}",
            text.text
        );
    }
}
