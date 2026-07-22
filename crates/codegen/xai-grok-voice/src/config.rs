use serde::{Deserialize, Serialize};

use crate::error::VoiceError;

/// STT transport mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SttMode {
    /// One-shot batch transcription (default): the pipeline buffers the whole
    /// utterance locally and posts it once, on release, to a REST endpoint
    /// (Chutes' `/stt/whisper`-shaped backends). No live interim preview —
    /// the transcript arrives all at once after a short round trip.
    #[default]
    Batch,
    /// Real-time WebSocket streaming STT with live interim results. Requires
    /// a backend that speaks the streaming protocol (`[`Self::stt_ws_url`]),
    /// which Chutes' default inference API does not.
    Streaming,
}

/// Voice settings for the STT transport.
///
/// Prefer **https** `api_base` (same shape as chat). [`Self::stt_ws_url`] derives
/// `wss://`. When `[voice].api_base` is unset, inherits
/// `[endpoints].xai_api_base_url` so enterprise proxies need no second knob.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VoiceConfig {
    /// HTTPS API root (or bare host). Bases may end in `/v1` or `/xai/v1`; the
    /// default STT path de-duplicates a leading `v1/` so both become `…/v1/stt`.
    /// Only consulted in [`SttMode::Streaming`].
    pub api_base: String,
    pub stt_ws_path: String,
    /// Which STT transport to use. See [`SttMode`].
    pub stt_mode: SttMode,
    /// HTTPS base for the [`SttMode::Batch`] REST transcription endpoint
    /// (Chutes' `vonkaiser-audiodojo` chute by default). Unlike `api_base`,
    /// this always has a working default — Chutes' own inference API has no
    /// batch STT route, so there is no `[endpoints]` fallback to inherit.
    pub batch_api_base: String,
    /// Preferred STT language (catalog code or `"auto"`). Resolved via
    /// [`crate::language_for_api`] at connect time in [`SttMode::Streaming`];
    /// in [`SttMode::Batch`], `"auto"` is sent as an omitted `language` field
    /// so the backend's own (real) auto-detection runs instead of guessing
    /// from the process locale.
    pub language: String,
    pub sample_rate: u32,
    pub stt_endpointing_ms: u32,
    pub stt_interim_results: bool,

    /// Pager-stamped request identity; not user config.
    #[serde(skip)]
    pub client_identifier: String,
    #[serde(skip)]
    pub user_agent: String,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            api_base: "https://127.0.0.1:9".into(),
            stt_ws_path: "/v1/stt".into(),
            stt_mode: SttMode::default(),
            batch_api_base: "https://vonkaiser-audiodojo.chutes.ai".into(),
            language: "en".into(),
            sample_rate: 16_000,
            stt_endpointing_ms: 400,
            stt_interim_results: true,
            client_identifier: String::new(),
            user_agent: String::new(),
        }
    }
}

impl VoiceConfig {
    /// Streaming STT WebSocket URL. Rejects plaintext `http://` / `ws://`.
    pub fn stt_ws_url(&self) -> Result<String, VoiceError> {
        ws_url(&self.api_base, &self.stt_ws_path)
    }

    /// `api_base`: non-empty `[voice].api_base`, else `[endpoints].xai_api_base_url`
    /// from `root`, else `resolved_endpoints_base`, else a fail-closed loopback endpoint.
    ///
    /// `resolved_endpoints_base` carries the caller's env / CLI overrides; it
    /// ranks below the raw table so config keeps beating env (shell precedence).
    pub fn from_config_table(root: &toml::Table, resolved_endpoints_base: Option<&str>) -> Self {
        let voice_table = root.get("voice").and_then(|v| v.as_table());
        let mut cfg: Self = voice_table
            .and_then(|t| toml::Value::Table(t.clone()).try_into().ok())
            .unwrap_or_default();

        // Read `[voice].api_base` from the raw table, not `cfg`: serde default
        // makes "unset" and an explicit default indistinguishable.
        cfg.api_base = non_empty_str(
            voice_table
                .and_then(|t| t.get("api_base"))
                .and_then(|v| v.as_str()),
        )
        .or_else(|| {
            non_empty_str(
                root.get("endpoints")
                    .and_then(|e| e.get("xai_api_base_url"))
                    .and_then(|v| v.as_str()),
            )
        })
        .or_else(|| non_empty_str(resolved_endpoints_base))
        .map(|base| base.trim_end_matches('/').to_owned())
        .unwrap_or_else(|| Self::default().api_base);
        cfg
    }
}

fn non_empty_str(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// `strip_prefix` ignoring ASCII case: RFC 3986 schemes are case-insensitive,
/// so `HTTP://` must hit the plaintext rejection and `HTTPS://` must work.
fn strip_scheme<'a>(s: &'a str, scheme: &str) -> Option<&'a str> {
    s.get(..scheme.len())
        .filter(|p| p.eq_ignore_ascii_case(scheme))
        .map(|_| &s[scheme.len()..])
}

fn ws_url(api_base: &str, path: &str) -> Result<String, VoiceError> {
    let base = api_base.trim().trim_end_matches('/');
    let path = path.trim().trim_start_matches('/');
    if strip_scheme(base, "http://").is_some() || strip_scheme(base, "ws://").is_some() {
        return Err(VoiceError::Config(format!(
            "insecure voice api_base {api_base:?}: voice requires a TLS endpoint \
             (https:// / wss://). Refusing to send the bearer token over a \
             plaintext connection."
        )));
    }
    let rest = strip_scheme(base, "https://")
        .or_else(|| strip_scheme(base, "wss://"))
        .unwrap_or(base);
    // Default path `/v1/stt`; bases often end in `/v1` or `/xai/v1`.
    let path = match (rest.ends_with("/v1"), path.strip_prefix("v1/")) {
        (true, Some(rest_path)) => rest_path,
        _ => path,
    };
    Ok(format!("wss://{rest}/{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stt_ws_uses_wss() {
        assert_eq!(
            VoiceConfig::default().stt_ws_url().unwrap(),
            "wss://127.0.0.1:9/v1/stt"
        );
    }

    #[test]
    fn default_stt_mode_is_batch_with_a_working_endpoint() {
        let cfg = VoiceConfig::default();
        assert_eq!(cfg.stt_mode, SttMode::Batch);
        assert_eq!(cfg.batch_api_base, "https://vonkaiser-audiodojo.chutes.ai");
    }

    #[test]
    fn stt_mode_round_trips_through_toml() {
        let table: toml::Table = toml::from_str(
            r#"
[voice]
stt_mode = "streaming"
batch_api_base = "https://stt.example.com"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table(&table, None);
        assert_eq!(cfg.stt_mode, SttMode::Streaming);
        assert_eq!(cfg.batch_api_base, "https://stt.example.com");
    }

    #[test]
    fn scheme_less_and_wss_bases() {
        for base in [
            "voice.example.com",
            "wss://voice.example.com",
            "HTTPS://voice.example.com",
        ] {
            let cfg = VoiceConfig {
                api_base: base.into(),
                ..VoiceConfig::default()
            };
            assert_eq!(cfg.stt_ws_url().unwrap(), "wss://voice.example.com/v1/stt");
        }
    }

    #[test]
    fn v1_base_dedupes_default_path() {
        let cfg = VoiceConfig {
            api_base: "https://proxy.example.com/v1".into(),
            ..VoiceConfig::default()
        };
        assert_eq!(cfg.stt_ws_url().unwrap(), "wss://proxy.example.com/v1/stt");
    }

    #[test]
    fn nested_v1_base_preserves_prefix() {
        let cfg = VoiceConfig {
            api_base: "https://proxy.example.com/speech/v1".into(),
            ..VoiceConfig::default()
        };
        assert_eq!(
            cfg.stt_ws_url().unwrap(),
            "wss://proxy.example.com/speech/v1/stt"
        );
    }

    #[test]
    fn rejects_plaintext_bases() {
        for base in [
            "http://localhost:8080",
            "ws://localhost:8080",
            "HTTP://localhost:8080",
            "Ws://localhost:8080",
        ] {
            let cfg = VoiceConfig {
                api_base: base.into(),
                ..VoiceConfig::default()
            };
            assert!(matches!(cfg.stt_ws_url(), Err(VoiceError::Config(_))));
        }
    }

    #[test]
    fn inherits_endpoints_when_voice_api_base_unset() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://proxy.example.com/speech/v1"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table(&table, None);
        assert_eq!(cfg.api_base, "https://proxy.example.com/speech/v1");
        assert_eq!(
            cfg.stt_ws_url().unwrap(),
            "wss://proxy.example.com/speech/v1/stt"
        );
    }

    #[test]
    fn empty_voice_api_base_still_inherits_endpoints() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://proxy.example.com/speech/v1"
[voice]
api_base = "  "
language = "fr"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table(&table, None);
        assert_eq!(cfg.api_base, "https://proxy.example.com/speech/v1");
        assert_eq!(cfg.language, "fr");
    }

    #[test]
    fn whitespace_voice_api_base_without_endpoints_uses_default() {
        let table: toml::Table = toml::from_str(
            r#"
[voice]
api_base = "  "
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table(&table, None);
        assert_eq!(cfg.api_base, VoiceConfig::default().api_base);
        assert_eq!(cfg.stt_ws_url().unwrap(), "wss://127.0.0.1:9/v1/stt");
    }

    #[test]
    fn resolved_endpoints_base_used_when_table_has_none() {
        let cfg = VoiceConfig::from_config_table(
            &toml::Table::new(),
            Some("https://proxy.example.com/v1/"),
        );
        assert_eq!(cfg.api_base, "https://proxy.example.com/v1");
        assert_eq!(cfg.stt_ws_url().unwrap(), "wss://proxy.example.com/v1/stt");

        // Whitespace-only resolved base falls through to the default.
        let cfg = VoiceConfig::from_config_table(&toml::Table::new(), Some("  "));
        assert_eq!(cfg.api_base, VoiceConfig::default().api_base);
    }

    /// config.toml beats the env/CLI fallback (shell endpoints precedence).
    #[test]
    fn table_endpoints_beat_resolved_endpoints_base() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://config.example.com"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table(&table, Some("https://env.example.com"));
        assert_eq!(cfg.api_base, "https://config.example.com");
    }

    #[test]
    fn voice_api_base_overrides_endpoints() {
        let table: toml::Table = toml::from_str(
            r#"
[endpoints]
xai_api_base_url = "https://proxy.example.com/speech/v1"
[voice]
api_base = "https://voice.example.com"
language = "es"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table(&table, None);
        assert_eq!(cfg.api_base, "https://voice.example.com");
        assert_eq!(cfg.language, "es");
        assert_eq!(cfg.stt_ws_url().unwrap(), "wss://voice.example.com/v1/stt");
    }

    #[test]
    fn ignores_unknown_and_identity_fields() {
        let table: toml::Table = toml::from_str(
            r#"
[voice]
enabled = false
client_identifier = "spoofed"
user_agent = "malicious/9.9"
language = "es"
"#,
        )
        .unwrap();
        let cfg = VoiceConfig::from_config_table(&table, None);
        assert_eq!(cfg.language, "es");
        assert!(cfg.client_identifier.is_empty());
        assert!(cfg.user_agent.is_empty());
    }
}
