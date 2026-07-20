//! Task routing and safe pre-stream fallback policy.

use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    Coding,
    Reasoning,
    Vision,
    LongContext,
    Fast,
    General,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelCapabilities {
    pub tool_calling: bool,
    pub vision: bool,
    pub reasoning: bool,
    pub context_window: u64,
    pub input_modalities: BTreeSet<String>,
    pub output_modalities: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelCandidate {
    pub id: String,
    pub capabilities: ModelCapabilities,
    pub available: bool,
}

impl ModelCandidate {
    fn score(&self, class: TaskClass, require_tools: bool) -> i64 {
        if !self.available || (require_tools && !self.capabilities.tool_calling) {
            return i64::MIN;
        }
        let mut score = 0i64;
        match class {
            TaskClass::Vision => score += i64::from(self.capabilities.vision) * 10_000,
            TaskClass::Reasoning => score += i64::from(self.capabilities.reasoning) * 5_000,
            TaskClass::LongContext => {
                score += (self.capabilities.context_window.min(2_000_000) / 1_000) as i64
            }
            TaskClass::Coding => {
                score += i64::from(self.capabilities.tool_calling) * 5_000;
                score += i64::from(self.capabilities.reasoning) * 1_000;
            }
            TaskClass::Fast => score -= (self.capabilities.context_window / 100_000) as i64,
            TaskClass::General => score += i64::from(self.capabilities.tool_calling) * 500,
        }
        score
    }
}

pub fn select_capable_model<'a>(
    models: &'a [ModelCandidate],
    class: TaskClass,
    require_tools: bool,
) -> Option<&'a ModelCandidate> {
    models
        .iter()
        .max_by_key(|candidate| candidate.score(class, require_tools))
        .filter(|candidate| candidate.score(class, require_tools) != i64::MIN)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FallbackPolicy {
    pub strict_model: bool,
}

impl Default for FallbackPolicy {
    fn default() -> Self {
        Self {
            strict_model: false,
        }
    }
}

impl FallbackPolicy {
    pub fn permits_fallback(self, status: Option<u16>, stream_started: bool) -> bool {
        if self.strict_model || stream_started {
            return false;
        }
        matches!(
            status,
            None | Some(408 | 409 | 425 | 429 | 500 | 502 | 503 | 504)
        )
    }

    /// Permit a retry on another model when the provider explicitly reports
    /// that the selected model cannot serve the request. Client errors are
    /// deliberately narrow so malformed prompts and invalid tool schemas are
    /// not silently routed elsewhere.
    pub fn permits_model_fallback(
        self,
        status: Option<u16>,
        message: &str,
        stream_started: bool,
    ) -> bool {
        if self.permits_fallback(status, stream_started) {
            return true;
        }
        if self.strict_model || stream_started || !matches!(status, Some(400 | 404 | 422)) {
            return false;
        }

        let message = message.to_ascii_lowercase();
        let mentions_model = message.contains("model") || message.contains("chute");
        let unavailable = [
            "not found",
            "unavailable",
            "not available",
            "not deployed",
            "offline",
            "no active",
            "cannot serve",
            "does not support",
            "unsupported",
        ]
        .iter()
        .any(|needle| message.contains(needle));
        mentions_model && unavailable
    }
}

#[derive(Debug, Clone, Default)]
pub struct StickyTurnRoute {
    selected_model: Option<String>,
}

impl StickyTurnRoute {
    pub fn select(&mut self, model: impl Into<String>) -> &str {
        self.selected_model.get_or_insert_with(|| model.into())
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected_model.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_falls_back_after_streaming_starts() {
        assert!(!FallbackPolicy::default().permits_fallback(Some(503), true));
    }

    #[test]
    fn strict_model_never_falls_back() {
        assert!(!FallbackPolicy { strict_model: true }.permits_fallback(Some(503), false));
    }

    #[test]
    fn unavailable_model_client_error_can_fall_back() {
        assert!(FallbackPolicy::default().permits_model_fallback(
            Some(404),
            "Model is not available",
            false,
        ));
        assert!(!FallbackPolicy::default().permits_model_fallback(
            Some(400),
            "Invalid tool schema",
            false,
        ));
    }

    #[test]
    fn route_is_sticky_within_a_turn() {
        let mut route = StickyTurnRoute::default();
        assert_eq!(route.select("first"), "first");
        assert_eq!(route.select("second"), "first");
    }
}
