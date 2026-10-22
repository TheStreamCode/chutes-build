//! Speech-to-text for workspace audio files via the AudioDojo chute.
//!
//! Unlike `generate_media` (which only represents media artifacts), this
//! tool targets the STT cords directly and returns the transcript as text.
//! Use it for voice memos, recordings, and meeting audio; text-to-speech
//! stays on `generate_media` with `kind: "speech"`.

use chutes_build_core::audio::{DEFAULT_STT_SLUG, SttEngine, TranscribeOptions, transcribe};
use chutes_build_core::media::ChutesMediaClient;

use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::{Cwd, DisplayCwd, FileSystem, resolve_model_path};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Default cap for the audio file read into memory, shared with the media
/// asset limit so one knob (`CHUTES_MAX_INPUT_ASSET_BYTES`) governs both.
fn max_input_bytes() -> usize {
    std::env::var("CHUTES_MAX_INPUT_ASSET_BYTES")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(64 * 1024 * 1024)
        .clamp(1, 512 * 1024 * 1024)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TranscribeAudioInput {
    /// Path to an audio file (wav, mp3, m4a, ogg, ...), relative to the
    /// working directory or absolute. Must stay inside the workspace.
    pub path: String,
    /// Transcription engine: `whisper` (default, widest language coverage),
    /// `canary`, or `parakeet`.
    pub engine: Option<String>,
    /// Chute slug override when the default STT chute is unavailable. Must
    /// expose a compatible `/stt/<engine>` cord.
    pub model: Option<String>,
    /// BCP-47 language code (`en`, `fr`, `zh`). Omitted means auto-detect.
    pub language: Option<String>,
    /// Include word-level timestamp chunks in the output. Off by default;
    /// transcripts are for reading, and timestamps multiply the tokens.
    pub timestamps: Option<bool>,
}

#[derive(Debug, Default)]
pub struct TranscribeAudioTool;

impl crate::types::tool_metadata::ToolMetadata for TranscribeAudioTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::ChutesBuild
    }

    fn description_template(&self) -> &str {
        "Transcribe speech from an audio file to text via a dedicated Chutes speech-to-text model \
         (Whisper by default; Canary or Parakeet on request). Use for voice memos, recordings, and \
         meeting audio — not for text-to-speech (use generate_media kind=speech for that). Reads \
         workspace audio files and returns the transcript as text, independent of the active chat model."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl chutes_tool_runtime::Tool for TranscribeAudioTool {
    type Args = TranscribeAudioInput;
    type Output = ToolOutput;

    fn id(&self) -> chutes_tool_protocol::ToolId {
        tool_id("transcribe_audio")
    }

    fn description(
        &self,
        _: &chutes_tool_runtime::ListToolsContext,
    ) -> chutes_tool_types::ToolDescription {
        chutes_tool_types::ToolDescription::new(
            "transcribe_audio",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> chutes_tool_protocol::ToolCapabilities {
        read_only_capabilities()
    }

    async fn run(
        &self,
        ctx: chutes_tool_runtime::ToolCallContext,
        input: TranscribeAudioInput,
    ) -> Result<ToolOutput, chutes_tool_runtime::ToolError> {
        let engine = input
            .engine
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(SttEngine::parse)
            .unwrap_or(Some(SttEngine::default()))
            .ok_or_else(|| {
                chutes_tool_runtime::ToolError::invalid_arguments(format!(
                    "unknown STT engine `{}` (expected whisper, canary, or parakeet)",
                    input.engine.as_deref().unwrap_or_default()
                ))
            })?;
        let slug = input
            .model
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(DEFAULT_STT_SLUG);

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
        let audio_bytes = fs.read_file(&path).await.map_err(|error| {
            execution_error(
                "transcribe_audio",
                format!("failed to read {}: {error}", path.display()),
            )
        })?;
        let max_bytes = max_input_bytes();
        if audio_bytes.len() > max_bytes {
            return Err(chutes_tool_runtime::ToolError::invalid_arguments(format!(
                "audio file exceeds the {max_bytes}-byte input limit; split it or raise CHUTES_MAX_INPUT_ASSET_BYTES"
            )));
        }

        let credentials = super::resolve_credentials(&ctx)
            .map_err(|error| execution_error("transcribe_audio", error))?;
        let client = ChutesMediaClient::from_credentials(credentials)
            .map_err(|error| execution_error("transcribe_audio", error))?;
        let detail = client.describe(slug).await.map_err(|error| {
            execution_error(
                "transcribe_audio",
                format!(
                    "STT chute `{slug}` is not reachable ({error}); use list_media_models with a speech query to find an equivalent transcription chute"
                ),
            )
        })?;
        let base_url = super::media::invoke_base_url(&detail).ok_or_else(|| {
            execution_error(
                "transcribe_audio",
                format!("STT chute `{slug}` publishes no invocation URL"),
            )
        })?;

        let options = TranscribeOptions {
            engine,
            language: input.language.clone(),
            return_timestamps: input.timestamps.unwrap_or(false),
            max_new_tokens: None,
        };
        let audio_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &audio_bytes);
        let result = transcribe(&client, &base_url, &audio_b64, &options)
            .await
            .map_err(|error| execution_error("transcribe_audio", error))?;

        let mut output = serde_json::Map::new();
        output.insert(
            "text".to_owned(),
            serde_json::Value::String(result.text.clone()),
        );
        output.insert(
            "engine".to_owned(),
            serde_json::Value::String(engine.as_str().to_owned()),
        );
        output.insert(
            "model".to_owned(),
            serde_json::Value::String(slug.to_owned()),
        );
        if let Some(language) = input.language.as_deref().map(str::trim)
            && !language.is_empty()
        {
            output.insert(
                "language".to_owned(),
                serde_json::Value::String(language.to_owned()),
            );
        }
        // Echo timestamp detail only when asked: the transcript join above
        // already consumed it, and the raw chunks are token-heavy.
        if options.return_timestamps {
            output.insert("detail".to_owned(), result.raw);
        }
        pretty_text(serde_json::Value::Object(output), "transcribe_audio")
    }
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
