//! Document OCR and structured extraction over the docuextract chute
//! (`vonkaiser-docuextract`).
//!
//! Typed wrapper around [`ChutesMediaClient`]: OCR cords return JSON
//! (markdown, templates, structured records), which the media-artifact
//! pipeline cannot represent, so this module builds the requests and
//! surfaces the raw JSON plus a best-effort text extraction.
//!
//! Only cords whose input contract was verified against the chute's
//! `llms.txt` are modeled (lighton/nuextract markdown, nuextract template
//! and extract). Anything else stays reachable through `generate_media` +
//! `describe_media_model`.

use crate::media::{ChutesMediaClient, MediaError};

/// Known docuextract chute slug. Callers may override it per call; the
/// default is what the tool layer passes when the user names no model.
pub const DEFAULT_DOC_SLUG: &str = "vonkaiser-docuextract";

/// OCR engines behind the markdown cords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocEngine {
    /// Fast single-shot OCR. Default.
    #[default]
    Lighton,
    /// Higher-quality OCR with reasoning mode for hard documents.
    Nuextract,
}

impl DocEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lighton => "lighton",
            Self::Nuextract => "nuextract",
        }
    }

    pub fn markdown_cord_path(self) -> &'static str {
        match self {
            Self::Lighton => "/ocr/lighton/markdown",
            Self::Nuextract => "/ocr/nuextract/markdown",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "lighton" | "lighton-ocr" => Some(Self::Lighton),
            "nuextract" | "nuextract-ocr" | "nu-extract" => Some(Self::Nuextract),
            _ => None,
        }
    }
}

/// Document input: base64 PDF or image. Exactly one must be set for the
/// markdown and template cords; the extract cord additionally accepts raw
/// `text`, of which at least one input must be present.
#[derive(Debug, Clone, Default)]
pub struct DocInput {
    pub pdf_b64: Option<String>,
    pub image_b64: Option<String>,
    pub text: Option<String>,
}

impl DocInput {
    fn has_pdf_or_image(&self) -> bool {
        self.pdf_b64
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
            || self
                .image_b64
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty())
    }

    fn has_any(&self) -> bool {
        self.has_pdf_or_image() || self.text.as_deref().is_some_and(|v| !v.trim().is_empty())
    }

    /// Shared rendering knobs; `None` fields are left for the server default.
    fn push_common(
        &self,
        body: &mut serde_json::Map<String, serde_json::Value>,
        common: &DocCommon,
    ) {
        if let Some(dpi) = common.dpi {
            body.insert("dpi".to_owned(), serde_json::Value::Number(dpi.into()));
        }
        if let Some(max_pages) = common.max_pages {
            body.insert(
                "max_pages".to_owned(),
                serde_json::Value::Number(max_pages.into()),
            );
        }
        if let Some(temperature) = common.temperature
            && let Some(number) = serde_json::Number::from_f64(temperature)
        {
            body.insert("temperature".to_owned(), serde_json::Value::Number(number));
        }
        if let Some(max_new_tokens) = common.max_new_tokens {
            body.insert(
                "max_new_tokens".to_owned(),
                serde_json::Value::Number(max_new_tokens.into()),
            );
        }
        if common.enable_thinking {
            body.insert("enable_thinking".to_owned(), serde_json::Value::Bool(true));
        }
    }

    fn push_document(&self, body: &mut serde_json::Map<String, serde_json::Value>) {
        if let Some(pdf) = self.pdf_b64.as_deref().map(str::trim)
            && !pdf.is_empty()
        {
            body.insert(
                "pdf_b64".to_owned(),
                serde_json::Value::String(pdf.to_owned()),
            );
        }
        if let Some(image) = self.image_b64.as_deref().map(str::trim)
            && !image.is_empty()
        {
            body.insert(
                "image_b64".to_owned(),
                serde_json::Value::String(image.to_owned()),
            );
        }
    }
}

/// Rendering knobs shared by every docuextract cord. All optional; the
/// server defaults apply when unset (dpi 200, max_pages 10 — 1 for the
/// template cord — temperature 0.0 for markdown, 0.2 for extract).
#[derive(Debug, Clone, Default)]
pub struct DocCommon {
    pub dpi: Option<u32>,
    pub max_pages: Option<u32>,
    pub temperature: Option<f64>,
    pub max_new_tokens: Option<u32>,
    pub enable_thinking: bool,
}

/// Build a markdown-OCR request body. Pure so the wire shape stays pinned.
/// `mode` is nuextract-only (`markdown` or `content`) and is sent only for
/// that engine when set.
pub fn markdown_body(
    input: &DocInput,
    engine: DocEngine,
    mode: Option<&str>,
    common: &DocCommon,
) -> Result<serde_json::Value, String> {
    if !input.has_pdf_or_image() {
        return Err("provide a PDF or an image (pdf_b64 or image_b64)".to_owned());
    }
    let mut body = serde_json::Map::new();
    input.push_document(&mut body);
    if matches!(engine, DocEngine::Nuextract)
        && let Some(mode) = mode.map(str::trim).filter(|m| !m.is_empty())
    {
        body.insert(
            "mode".to_owned(),
            serde_json::Value::String(mode.to_owned()),
        );
    }
    input.push_common(&mut body, common);
    Ok(serde_json::Value::Object(body))
}

/// Build a template-creation body: `description` says what to extract.
pub fn template_body(
    input: &DocInput,
    description: &str,
    common: &DocCommon,
) -> Result<serde_json::Value, String> {
    if !input.has_pdf_or_image() {
        return Err("provide a PDF or an image (pdf_b64 or image_b64)".to_owned());
    }
    let description = description.trim();
    if description.is_empty() {
        return Err("template mode needs a `description` of what to extract".to_owned());
    }
    let mut body = serde_json::Map::new();
    input.push_document(&mut body);
    body.insert(
        "description".to_owned(),
        serde_json::Value::String(description.to_owned()),
    );
    input.push_common(&mut body, common);
    Ok(serde_json::Value::Object(body))
}

/// Build a structured-extraction body: `template` comes from the template
/// cord (or the caller), `instructions`/`examples` refine it.
pub fn extract_body(
    input: &DocInput,
    template: &serde_json::Value,
    instructions: Option<&str>,
    examples: Option<&serde_json::Value>,
    common: &DocCommon,
) -> Result<serde_json::Value, String> {
    if !input.has_any() {
        return Err("provide document text, a PDF, or an image".to_owned());
    }
    if !template.is_object() {
        return Err("extract mode needs the template `object` from the template cord".to_owned());
    }
    let mut body = serde_json::Map::new();
    input.push_document(&mut body);
    if let Some(text) = input.text.as_deref().map(str::trim)
        && !text.is_empty()
    {
        body.insert(
            "text".to_owned(),
            serde_json::Value::String(text.to_owned()),
        );
    }
    body.insert("template".to_owned(), template.clone());
    if let Some(instructions) = instructions.map(str::trim).filter(|s| !s.is_empty()) {
        body.insert(
            "instructions".to_owned(),
            serde_json::Value::String(instructions.to_owned()),
        );
    }
    if let Some(examples) = examples {
        body.insert("examples".to_owned(), examples.clone());
    }
    input.push_common(&mut body, common);
    Ok(serde_json::Value::Object(body))
}

/// Best-effort markdown/text out of an OCR cord response. Shapes are
/// untyped, so probe the known slots (`markdown`, `content`, `text`,
/// `document`) before returning the whole body pretty-printed.
pub fn markdown_text(value: &serde_json::Value) -> Option<String> {
    for key in ["markdown", "content", "text", "document"] {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str)
            && !text.trim().is_empty()
        {
            return Some(text.to_owned());
        }
    }
    if let Some(items) = value.get("pages").and_then(serde_json::Value::as_array) {
        let joined = items
            .iter()
            .filter_map(|item| {
                item.as_str().or_else(|| {
                    ["markdown", "content", "text"]
                        .iter()
                        .find_map(|key| item.get(key).and_then(serde_json::Value::as_str))
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if !joined.trim().is_empty() {
            return Some(joined);
        }
    }
    None
}

async fn invoke_json(
    client: &ChutesMediaClient,
    url: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, MediaError> {
    let response = client.invoke(url, "POST", body).await?;
    let bytes = response.bytes().ok_or_else(|| {
        MediaError::UnexpectedResponse(
            "docuextract cord returned a spilled (non-memory) response".to_owned(),
        )
    })?;
    serde_json::from_slice(bytes).map_err(MediaError::Decode)
}

/// Run document OCR to markdown through `base_url` (a resolved chute host).
pub async fn extract_markdown(
    client: &ChutesMediaClient,
    base_url: &str,
    input: &DocInput,
    engine: DocEngine,
    mode: Option<&str>,
    common: &DocCommon,
) -> Result<serde_json::Value, DocError> {
    let body = markdown_body(input, engine, mode, common).map_err(DocError::InvalidInput)?;
    let url = format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        engine.markdown_cord_path()
    );
    invoke_json(client, &url, &body)
        .await
        .map_err(DocError::Media)
}

/// Create an extraction template through `base_url`.
pub async fn create_template(
    client: &ChutesMediaClient,
    base_url: &str,
    input: &DocInput,
    description: &str,
    common: &DocCommon,
) -> Result<serde_json::Value, DocError> {
    let body = template_body(input, description, common).map_err(DocError::InvalidInput)?;
    let url = format!("{}/ocr/nuextract/template", base_url.trim_end_matches('/'));
    invoke_json(client, &url, &body)
        .await
        .map_err(DocError::Media)
}

/// Run structured extraction through `base_url` with a template from
/// [`create_template`].
pub async fn extract_with_template(
    client: &ChutesMediaClient,
    base_url: &str,
    input: &DocInput,
    template: &serde_json::Value,
    instructions: Option<&str>,
    examples: Option<&serde_json::Value>,
    common: &DocCommon,
) -> Result<serde_json::Value, DocError> {
    let body = extract_body(input, template, instructions, examples, common)
        .map_err(DocError::InvalidInput)?;
    let url = format!("{}/ocr/nuextract/extract", base_url.trim_end_matches('/'));
    invoke_json(client, &url, &body)
        .await
        .map_err(DocError::Media)
}

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("{0}")]
    InvalidInput(String),
    #[error(transparent)]
    Media(#[from] MediaError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_paths_and_aliases() {
        assert_eq!(
            DocEngine::Lighton.markdown_cord_path(),
            "/ocr/lighton/markdown"
        );
        assert_eq!(
            DocEngine::Nuextract.markdown_cord_path(),
            "/ocr/nuextract/markdown"
        );
        assert_eq!(DocEngine::parse("lighton-ocr"), Some(DocEngine::Lighton));
        assert_eq!(DocEngine::parse("NuExtract"), Some(DocEngine::Nuextract));
        assert_eq!(DocEngine::parse("tesseract"), None);
    }

    #[test]
    fn markdown_body_needs_a_document() {
        let empty = DocInput::default();
        assert!(markdown_body(&empty, DocEngine::Lighton, None, &DocCommon::default()).is_err());
        let input = DocInput {
            pdf_b64: Some("QUJD".to_owned()),
            ..DocInput::default()
        };
        let body = markdown_body(&input, DocEngine::Lighton, None, &DocCommon::default()).unwrap();
        assert_eq!(body["pdf_b64"], "QUJD");
        assert!(body.get("mode").is_none());
    }

    #[test]
    fn markdown_body_sends_nuextract_mode_only() {
        let input = DocInput {
            image_b64: Some("QUJD".to_owned()),
            ..DocInput::default()
        };
        let body = markdown_body(
            &input,
            DocEngine::Nuextract,
            Some("content"),
            &DocCommon::default(),
        )
        .unwrap();
        assert_eq!(body["mode"], "content");
        let body = markdown_body(
            &input,
            DocEngine::Lighton,
            Some("content"),
            &DocCommon::default(),
        )
        .unwrap();
        assert!(body.get("mode").is_none());
    }

    #[test]
    fn template_body_needs_description_and_document() {
        let input = DocInput {
            image_b64: Some("QUJD".to_owned()),
            ..DocInput::default()
        };
        assert!(template_body(&input, "  ", &DocCommon::default()).is_err());
        assert!(template_body(&DocInput::default(), "invoices", &DocCommon::default()).is_err());
        let body = template_body(&input, "invoice fields", &DocCommon::default()).unwrap();
        assert_eq!(body["description"], "invoice fields");
    }

    #[test]
    fn extract_body_needs_template_object_and_input() {
        let template = serde_json::json!({"type": "object"});
        let input = DocInput {
            text: Some("hello".to_owned()),
            ..DocInput::default()
        };
        assert!(
            extract_body(
                &DocInput::default(),
                &template,
                None,
                None,
                &DocCommon::default()
            )
            .is_err()
        );
        assert!(
            extract_body(
                &input,
                &serde_json::json!("nope"),
                None,
                None,
                &DocCommon::default()
            )
            .is_err()
        );
        let body = extract_body(
            &input,
            &template,
            Some("be strict"),
            None,
            &DocCommon::default(),
        )
        .unwrap();
        assert_eq!(body["text"], "hello");
        assert_eq!(body["template"], template);
        assert_eq!(body["instructions"], "be strict");
    }

    #[test]
    fn markdown_extraction_probes_known_slots() {
        let value = serde_json::json!({"markdown": "# hi"});
        assert_eq!(markdown_text(&value).as_deref(), Some("# hi"));
        let value = serde_json::json!({"pages": [{"text": "a"}, {"text": "b"}]});
        assert_eq!(markdown_text(&value).as_deref(), Some("a\n\nb"));
        let value = serde_json::json!({"cost": 0.5});
        assert_eq!(markdown_text(&value), None);
    }
}
