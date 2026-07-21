//! Chutes vision-model client for on-demand image/PDF-page text transcription.
//!
//! Always targets an official, curated Chutes-hosted model over the standard
//! inference endpoint (or the dedicated router endpoint for the virtual
//! `model-router` id) — billed against the account's subscription quota,
//! never the separate marketplace/wallet balance used by third-party chutes.

use base64::Engine as _;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};

use crate::{ChutesCredentials, ChutesEndpoints};

/// Generous cap so a dense document page isn't cut off mid-transcription;
/// `TranscribeResponse::truncated` reports when the model still hit it.
const MAX_TRANSCRIBE_TOKENS: u32 = 8192;

#[derive(Debug, Clone)]
pub struct ChutesVisionClient {
    http: reqwest::Client,
    endpoints: ChutesEndpoints,
    credentials: ChutesCredentials,
}

impl ChutesVisionClient {
    pub fn from_env() -> Result<Self, VisionError> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            endpoints: ChutesEndpoints::default(),
            credentials: ChutesCredentials::from_env()?,
        })
    }

    /// Ask `model` to transcribe visible text out of `image_bytes` verbatim.
    ///
    /// `model.eq_ignore_ascii_case("model-router")` is dispatched to the
    /// dedicated router endpoint — the virtual auto-routing model isn't
    /// served on the standard inference host.
    pub async fn transcribe(
        &self,
        model: &str,
        mime_type: &str,
        image_bytes: &[u8],
        prompt: &str,
    ) -> Result<TranscribeResponse, VisionError> {
        let base = if model.eq_ignore_ascii_case("model-router") {
            &self.endpoints.router
        } else {
            &self.endpoints.inference
        };
        let url = format!("{base}/chat/completions");
        let data_url = format!(
            "data:{mime_type};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(image_bytes)
        );
        let body = serde_json::json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {"url": data_url}},
                ],
            }],
            "max_tokens": MAX_TRANSCRIBE_TOKENS,
        });

        let response = self
            .http
            .post(url)
            .header(
                AUTHORIZATION,
                self.credentials.inference_authorization().as_ref(),
            )
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(VisionError::Http {
                status: status.as_u16(),
                body: text.chars().take(500).collect(),
            });
        }

        parse_transcribe_response(&text)
    }
}

fn parse_transcribe_response(body: &str) -> Result<TranscribeResponse, VisionError> {
    let parsed: serde_json::Value = serde_json::from_str(body)?;
    let content = parsed
        .pointer("/choices/0/message/content")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| VisionError::UnexpectedResponse("choices[0].message.content".to_owned()))?
        .to_owned();
    let truncated = parsed
        .pointer("/choices/0/finish_reason")
        .and_then(serde_json::Value::as_str)
        == Some("length");

    Ok(TranscribeResponse {
        text: content,
        truncated,
    })
}

#[derive(Debug, Clone)]
pub struct TranscribeResponse {
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error(transparent)]
    Credentials(#[from] crate::endpoints::CredentialError),
    #[error("Chutes vision request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Chutes returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("Chutes returned invalid JSON: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("Chutes vision response missing expected field: {0}")]
    UnexpectedResponse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_successful_response() {
        let body = r#"{"choices":[{"message":{"content":"Chutes Build OCR test 12345"},"finish_reason":"stop"}]}"#;
        let result = parse_transcribe_response(body).unwrap();
        assert_eq!(result.text, "Chutes Build OCR test 12345");
        assert!(!result.truncated);
    }

    #[test]
    fn flags_truncation_on_length_finish_reason() {
        let body = r#"{"choices":[{"message":{"content":"partial"},"finish_reason":"length"}]}"#;
        let result = parse_transcribe_response(body).unwrap();
        assert!(result.truncated);
    }

    #[test]
    fn rejects_response_missing_content() {
        let body = r#"{"choices":[{"message":{},"finish_reason":"stop"}]}"#;
        let err = parse_transcribe_response(body).unwrap_err();
        assert!(matches!(err, VisionError::UnexpectedResponse(_)));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = parse_transcribe_response("not json").unwrap_err();
        assert!(matches!(err, VisionError::Decode(_)));
    }
}
