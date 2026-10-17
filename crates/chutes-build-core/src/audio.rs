//! Speech-to-text over the AudioDojo chute (`vonkaiser-audiodojo`).
//!
//! Thin typed wrapper around [`ChutesMediaClient`]: STT cords return JSON
//! (transcript plus optional word timestamps), which the media-artifact
//! pipeline cannot represent, so this module builds the request and extracts
//! the transcript instead of going through `generate_media`.

use crate::media::{ChutesMediaClient, MediaError};

/// Known AudioDojo chute slug. Callers may override it per call; the default
/// is what the tool layer passes when the user names no model.
pub const DEFAULT_STT_SLUG: &str = "vonkaiser-audiodojo";

/// Transcription engines exposed as STT cords. Whisper is the default:
/// highest language coverage with word timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SttEngine {
    #[default]
    Whisper,
    Canary,
    Parakeet,
}

impl SttEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Whisper => "whisper",
            Self::Canary => "canary",
            Self::Parakeet => "parakeet",
        }
    }

    pub fn cord_path(self) -> &'static str {
        match self {
            Self::Whisper => "/stt/whisper",
            Self::Canary => "/stt/canary",
            Self::Parakeet => "/stt/parakeet",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "whisper" | "whisper-large-v3-turbo" | "large-v3-turbo" => Some(Self::Whisper),
            "canary" | "canary-qwen" | "nvidia-canary" => Some(Self::Canary),
            "parakeet" | "parakeet-tdt" | "nvidia-parakeet" => Some(Self::Parakeet),
            _ => None,
        }
    }
}

/// Transcription request parameters.
#[derive(Debug, Clone, Default)]
pub struct TranscribeOptions {
    pub engine: SttEngine,
    /// BCP-47 language code (`en`, `fr`, `zh`). `None` auto-detects.
    pub language: Option<String>,
    /// Include word-level timestamp chunks. Off by default: transcripts are
    /// for reading, and timestamps multiply the tokens.
    pub return_timestamps: bool,
    /// Token cap for engines that accept one (Canary). `None` uses the cord default.
    pub max_new_tokens: Option<u32>,
}

/// Transcription result: the best-effort transcript plus the raw cord JSON
/// for timestamps and engine metadata.
#[derive(Debug, Clone)]
pub struct Transcription {
    pub text: String,
    pub raw: serde_json::Value,
}

/// Build the STT request body. Pure so the wire shape stays pinned by tests.
pub fn transcribe_body(audio_b64: &str, options: &TranscribeOptions) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    body.insert(
        "audio_b64".to_owned(),
        serde_json::Value::String(audio_b64.to_owned()),
    );
    if let Some(language) = options.language.as_deref().map(str::trim)
        && !language.is_empty()
    {
        body.insert(
            "language".to_owned(),
            serde_json::Value::String(language.to_owned()),
        );
    }
    // Whisper is the only engine whose schema documents timestamps; sending
    // the flag elsewhere trips closed-schema validation on the chute.
    if matches!(options.engine, SttEngine::Whisper) && options.return_timestamps {
        body.insert(
            "return_timestamps".to_owned(),
            serde_json::Value::Bool(true),
        );
    }
    if let Some(max) = options.max_new_tokens
        && !matches!(options.engine, SttEngine::Whisper)
    {
        body.insert(
            "max_new_tokens".to_owned(),
            serde_json::Value::Number(max.into()),
        );
    }
    serde_json::Value::Object(body)
}

/// Extract the transcript from an STT cord response. Shapes are untyped
/// (`additionalProperties: true`), so probe the known text slots before
/// giving up: `text`, `transcript`, `transcription`, then a single-string
/// `segments`/`chunks` join, else the whole body pretty-printed.
pub fn transcript_text(value: &serde_json::Value) -> Option<String> {
    for key in ["text", "transcript", "transcription"] {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str)
            && !text.trim().is_empty()
        {
            return Some(text.to_owned());
        }
    }
    for key in ["segments", "chunks"] {
        if let Some(items) = value.get(key).and_then(serde_json::Value::as_array) {
            let joined = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| item.as_str())
                })
                .collect::<Vec<_>>()
                .join(" ");
            if !joined.trim().is_empty() {
                return Some(joined);
            }
        }
    }
    None
}

/// Transcribe base64 audio through `base_url` (a resolved chute host such as
/// `https://vonkaiser-audiodojo.chutes.ai`). Returns an error naming the
/// engine when no transcript slot is recognizable.
pub async fn transcribe(
    client: &ChutesMediaClient,
    base_url: &str,
    audio_b64: &str,
    options: &TranscribeOptions,
) -> Result<Transcription, MediaError> {
    let url = format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        options.engine.cord_path()
    );
    let body = transcribe_body(audio_b64, options);
    let response = client.invoke(&url, "POST", &body).await?;
    let bytes = response.bytes().ok_or_else(|| {
        MediaError::UnexpectedResponse(
            "STT cord returned a spilled (non-memory) response".to_owned(),
        )
    })?;
    let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(MediaError::Decode)?;
    let text = transcript_text(&raw).ok_or_else(|| {
        let preview: String = serde_json::to_string(&raw)
            .unwrap_or_else(|_| "<unserializable>".to_owned())
            .chars()
            .take(300)
            .collect();
        MediaError::UnexpectedResponse(format!(
            "STT cord returned JSON without a transcript: {preview}"
        ))
    })?;
    Ok(Transcription { text, raw })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_paths_and_aliases() {
        assert_eq!(SttEngine::Whisper.cord_path(), "/stt/whisper");
        assert_eq!(SttEngine::Canary.cord_path(), "/stt/canary");
        assert_eq!(SttEngine::Parakeet.cord_path(), "/stt/parakeet");
        assert_eq!(SttEngine::parse("Whisper"), Some(SttEngine::Whisper));
        assert_eq!(SttEngine::parse("large-v3-turbo"), Some(SttEngine::Whisper));
        assert_eq!(SttEngine::parse("canary-qwen"), Some(SttEngine::Canary));
        assert_eq!(SttEngine::parse("parakeet-tdt"), Some(SttEngine::Parakeet));
        assert_eq!(SttEngine::parse("wav2vec"), None);
        assert_eq!(SttEngine::default(), SttEngine::Whisper);
    }

    #[test]
    fn whisper_body_carries_language_and_timestamps() {
        let options = TranscribeOptions {
            engine: SttEngine::Whisper,
            language: Some("en".to_owned()),
            return_timestamps: true,
            max_new_tokens: None,
        };
        let body = transcribe_body("QUJD", &options);
        assert_eq!(body["audio_b64"], "QUJD");
        assert_eq!(body["language"], "en");
        assert_eq!(body["return_timestamps"], true);
        assert!(body.get("max_new_tokens").is_none());
    }

    #[test]
    fn non_whisper_body_omits_timestamps_flag() {
        let options = TranscribeOptions {
            engine: SttEngine::Canary,
            language: None,
            return_timestamps: true,
            max_new_tokens: Some(512),
        };
        let body = transcribe_body("QUJD", &options);
        assert!(body.get("language").is_none());
        assert!(body.get("return_timestamps").is_none());
        assert_eq!(body["max_new_tokens"], 512);
    }

    #[test]
    fn transcript_extraction_prefers_text_slots() {
        let value = serde_json::json!({"text": "hello", "language": "en"});
        assert_eq!(transcript_text(&value).as_deref(), Some("hello"));
        let value = serde_json::json!({"transcript": "hi"});
        assert_eq!(transcript_text(&value).as_deref(), Some("hi"));
        let value = serde_json::json!({"segments": [{"text": "a"}, {"text": "b"}]});
        assert_eq!(transcript_text(&value).as_deref(), Some("a b"));
        let value = serde_json::json!({"chunks": ["x", "y"]});
        assert_eq!(transcript_text(&value).as_deref(), Some("x y"));
        let value = serde_json::json!({"cost": 0.5});
        assert_eq!(transcript_text(&value), None);
    }
}
