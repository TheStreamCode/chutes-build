//! One-shot batch transcription (`SttMode::Batch`): the whole utterance is
//! buffered locally and posted once, on release, to a REST endpoint. No
//! interim/partial results exist in this mode — the backend (Chutes'
//! `/stt/whisper`) is a plain request/response API, not a streaming one.

use base64::Engine;
use serde::Deserialize;

use crate::config::VoiceConfig;
use crate::error::VoiceError;
use crate::language::{STT_LANGUAGE_AUTO, canonicalize_stt_language};

const WHISPER_PATH: &str = "/stt/whisper";
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Debug, Deserialize)]
struct BatchSttResponse {
    #[serde(default)]
    text: String,
}

/// Wrap raw little-endian PCM16 mono samples in a minimal 44-byte-header WAV
/// container so the backend ("any common format") can decode it unambiguously.
fn wav_from_pcm16(pcm: &[u8], sample_rate: u32) -> Vec<u8> {
    let data_len = pcm.len() as u32;
    let byte_rate = sample_rate * 2; // mono, 16-bit
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes()); // block align (mono, 16-bit)
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

/// Transcribe a complete utterance in one request.
///
/// `pcm` is raw little-endian PCM16 mono samples captured at
/// `config.sample_rate`. Returns `Ok("")` without a network call for empty
/// input (nothing captured).
pub async fn transcribe_batch(
    config: &VoiceConfig,
    bearer: &str,
    pcm: &[u8],
) -> Result<String, VoiceError> {
    if pcm.is_empty() {
        return Ok(String::new());
    }
    let wav = wav_from_pcm16(pcm, config.sample_rate);
    let audio_b64 = base64::engine::general_purpose::STANDARD.encode(wav);

    // Omit `language` for "auto" so the backend's own detection runs, rather
    // than guessing a concrete code from the process locale (the streaming
    // path's `language_for_api` workaround for a backend that lacks real
    // auto-detect — this one has it).
    let canonical = canonicalize_stt_language(Some(&config.language));
    let language = (canonical != STT_LANGUAGE_AUTO).then_some(canonical);

    let body = serde_json::json!({
        "audio_b64": audio_b64,
        "language": language,
        "return_timestamps": false,
    });

    let url = format!(
        "{}{WHISPER_PATH}",
        config.batch_api_base.trim_end_matches('/')
    );
    let client = reqwest::Client::new();
    let mut request = client.post(&url).bearer_auth(bearer).json(&body);
    if !config.client_identifier.is_empty() {
        request = request.header(
            "x-chutes-build-client-identifier",
            &config.client_identifier,
        );
    }
    if !config.user_agent.is_empty() {
        request = request.header(reqwest::header::USER_AGENT, &config.user_agent);
    }

    let response = tokio::time::timeout(REQUEST_TIMEOUT, request.send())
        .await
        .map_err(|_| VoiceError::Stt("batch STT request timed out".into()))?
        .map_err(|e| VoiceError::Stt(format!("batch STT request failed: {e}")))?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(VoiceError::Stt(format!(
            "batch STT HTTP {status}: {body_text}"
        )));
    }

    let parsed: BatchSttResponse = response
        .json()
        .await
        .map_err(|e| VoiceError::Stt(format!("batch STT response parse failed: {e}")))?;
    Ok(parsed.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_reports_correct_lengths() {
        let pcm = vec![0u8; 32_000]; // 1s @ 16kHz mono 16-bit
        let wav = wav_from_pcm16(&pcm, 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        let riff_len = u32::from_le_bytes(wav[4..8].try_into().unwrap());
        assert_eq!(riff_len as usize, 36 + pcm.len());
        let data_len = u32::from_le_bytes(wav[40..44].try_into().unwrap());
        assert_eq!(data_len as usize, pcm.len());
        assert_eq!(wav.len(), 44 + pcm.len());
    }

    #[tokio::test]
    async fn empty_pcm_skips_the_network_call() {
        let config = VoiceConfig::default();
        assert_eq!(
            transcribe_batch(&config, "unused-bearer", &[])
                .await
                .unwrap(),
            ""
        );
    }

    #[test]
    fn response_parses_text_field() {
        let parsed: BatchSttResponse =
            serde_json::from_str(r#"{"text":"hello world","segments":[]}"#).unwrap();
        assert_eq!(parsed.text, "hello world");
    }

    #[test]
    fn response_missing_text_defaults_to_empty() {
        let parsed: BatchSttResponse = serde_json::from_str(r#"{"segments":[]}"#).unwrap();
        assert_eq!(parsed.text, "");
    }
}
