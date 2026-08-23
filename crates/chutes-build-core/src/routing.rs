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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FallbackPolicy {
    pub strict_model: bool,
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

/// Native Chutes server-side routing (chutes.ai/app → Model Routing).
///
/// The `model` field accepts a saved-pool alias (`default`), an alias with a
/// strategy suffix (`default:latency`, `default:throughput`), or an inline
/// comma-separated pool (`modelA,modelB[:strategy]`). The server resolves the
/// pool per request; the client only composes the string. Saved aliases
/// require a dashboard-configured pool, while inline pools work on any
/// account.
pub const DEFAULT_ALIAS: &str = "default";

/// The legacy virtual auto-router id. Configs written before the native
/// grammar still name it; the sampler maps it to the current auto string.
pub const LEGACY_AUTO_MODEL_ID: &str = "model-router";

/// Whether the string is the saved-pool alias, optionally with a strategy
/// suffix (`default`, `default:latency`, `default:throughput`). Such a
/// string resolves only against an account-level pool; an inline pool never
/// matches, and neither does a concrete catalogue id.
pub fn is_dashboard_alias(model: &str) -> bool {
    let Some(rest) = model.strip_prefix(DEFAULT_ALIAS) else {
        return false;
    };
    rest.is_empty() || matches!(rest, ":latency" | ":throughput")
}

/// Strategy for a live-composed Auto pool: honour an alias suffix or
/// `CHUTES_ROUTING_STRATEGY`, otherwise prefer lowest time-to-first-token
/// (`:latency`) so Auto answers quickly from whatever is warm.
pub fn auto_live_strategy(alias: &str) -> RoutingStrategy {
    if let Some((_, suffix)) = alias.split_once(':') {
        return match suffix {
            "throughput" => RoutingStrategy::Throughput,
            "latency" => RoutingStrategy::Latency,
            _ => RoutingStrategy::Latency,
        };
    }
    std::env::var("CHUTES_ROUTING_STRATEGY")
        .ok()
        .and_then(|value| RoutingStrategy::from_env_value(&value))
        .unwrap_or(RoutingStrategy::Latency)
}

/// Compose an inline pool from live catalogue ids. Drops the dashboard alias,
/// the legacy virtual id, empty members, and anything containing a comma
/// (which would break the grammar). `None` when nothing usable remains.
pub fn compose_live_auto_pool(ids: &[&str], strategy: RoutingStrategy) -> Option<String> {
    let members: Vec<&str> = ids
        .iter()
        .copied()
        .map(str::trim)
        .filter(|id| {
            !id.is_empty()
                && !id.contains(',')
                && *id != LEGACY_AUTO_MODEL_ID
                && !is_dashboard_alias(id)
        })
        .collect();
    if members.is_empty() {
        return None;
    }
    Some(compose_routing_model(&members, Some(strategy)))
}

/// Append a live-composed Auto pool after the dashboard alias in a fallback
/// chain. No-op when the chain has no alias or the live list is empty.
pub fn append_live_auto_pool(candidates: &mut Vec<String>, live_ids: &[String], alias: &str) {
    if !is_dashboard_alias(alias) {
        return;
    }
    let refs: Vec<&str> = live_ids.iter().map(String::as_str).collect();
    let Some(pool) = compose_live_auto_pool(&refs, auto_live_strategy(alias)) else {
        return;
    };
    if !candidates.iter().any(|candidate| candidate == &pool) {
        candidates.push(pool);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingStrategy {
    Sequential,
    Latency,
    Throughput,
}

impl RoutingStrategy {
    fn suffix(self) -> &'static str {
        match self {
            Self::Sequential => "",
            Self::Latency => ":latency",
            Self::Throughput => ":throughput",
        }
    }

    fn from_env_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sequential" | "none" | "" => Some(Self::Sequential),
            "latency" => Some(Self::Latency),
            "throughput" => Some(Self::Throughput),
            _ => None,
        }
    }
}

/// Compose the native routing string for a pool and optional strategy.
///
/// An empty pool yields the dashboard alias (`default`), so callers without
/// configuration still target server-side failover once a pool is saved.
pub fn compose_routing_model(pool: &[&str], strategy: Option<RoutingStrategy>) -> String {
    let strategy = strategy.unwrap_or(RoutingStrategy::Sequential);
    if pool.is_empty() {
        return format!("{DEFAULT_ALIAS}{}", strategy.suffix());
    }
    let members: Vec<&str> = pool
        .iter()
        .map(|m| m.trim())
        .filter(|m| !m.is_empty())
        .collect();
    format!("{}{}", members.join(","), strategy.suffix())
}

/// The auto-routing model string this process should send, resolved from the
/// environment: `CHUTES_ROUTING_POOL` (comma-separated catalogue ids) and
/// `CHUTES_ROUTING_STRATEGY` (`sequential`, `latency`, or `throughput`;
/// unknown values are ignored). Unset environment means the plain `default`
/// alias.
pub fn auto_model_from_env() -> String {
    let pool: Vec<String> = std::env::var("CHUTES_ROUTING_POOL")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|member| !member.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let strategy = std::env::var("CHUTES_ROUTING_STRATEGY")
        .ok()
        .and_then(|raw| RoutingStrategy::from_env_value(&raw));
    compose_routing_model(
        &pool.iter().map(String::as_str).collect::<Vec<_>>(),
        strategy,
    )
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

    #[test]
    fn dashboard_alias_detection_covers_strategy_suffixes_only() {
        assert!(is_dashboard_alias("default"));
        assert!(is_dashboard_alias("default:latency"));
        assert!(is_dashboard_alias("default:throughput"));
        assert!(!is_dashboard_alias("model-router"));
        assert!(!is_dashboard_alias("Qwen/Qwen3.5-397B-A17B-TEE,default"));
        assert!(!is_dashboard_alias("Qwen/Qwen3.5-397B-A17B-TEE:latency"));
        assert!(!is_dashboard_alias("default:sequential"));
    }

    #[test]
    fn live_auto_pool_drops_aliases_and_applies_strategy() {
        assert_eq!(
            compose_live_auto_pool(&["a/One", "default", "b/Two"], RoutingStrategy::Latency)
                .as_deref(),
            Some("a/One,b/Two:latency")
        );
        assert_eq!(
            compose_live_auto_pool(&["model-router", "c/Three"], RoutingStrategy::Throughput)
                .as_deref(),
            Some("c/Three:throughput")
        );
        assert!(compose_live_auto_pool(&["default", ""], RoutingStrategy::Latency).is_none());
    }

    #[test]
    fn live_auto_pool_appends_only_behind_the_dashboard_alias() {
        let live = ["a/One".to_owned(), "b/Two".to_owned()];
        let mut chain = vec!["picked".to_owned(), "default".to_owned()];
        append_live_auto_pool(&mut chain, &live, "default");
        assert_eq!(chain, ["picked", "default", "a/One,b/Two:latency"]);

        let mut already = vec!["a/One,b/Two:latency".to_owned()];
        append_live_auto_pool(&mut already, &live, "default");
        assert_eq!(already, ["a/One,b/Two:latency"]);

        let mut pinned = vec!["a/One".to_owned()];
        append_live_auto_pool(&mut pinned, &live, "a/One");
        assert_eq!(pinned, ["a/One"]);
    }

    #[test]
    fn empty_pool_composes_the_dashboard_alias() {
        use RoutingStrategy::*;
        assert_eq!(compose_routing_model(&[], None), "default");
        assert_eq!(compose_routing_model(&[], Some(Sequential)), "default");
        assert_eq!(compose_routing_model(&[], Some(Latency)), "default:latency");
        assert_eq!(
            compose_routing_model(&[], Some(Throughput)),
            "default:throughput"
        );
    }

    #[test]
    fn inline_pools_compose_in_order_with_optional_strategy() {
        use RoutingStrategy::*;
        let pool = ["zai-org/GLM-5.1-TEE", "Qwen/Qwen3-32B-TEE"];
        assert_eq!(
            compose_routing_model(&pool, None),
            "zai-org/GLM-5.1-TEE,Qwen/Qwen3-32B-TEE"
        );
        assert_eq!(
            compose_routing_model(&pool, Some(Throughput)),
            "zai-org/GLM-5.1-TEE,Qwen/Qwen3-32B-TEE:throughput"
        );
    }

    #[test]
    fn env_resolution_reads_pool_and_strategy() {
        // SAFETY: test-scoped env mutation, serialized by the suite runner
        // (single-threaded harness for this crate's unit tests).
        unsafe {
            std::env::set_var(
                "CHUTES_ROUTING_POOL",
                " zai-org/GLM-5.1-TEE , deepseek-ai/DeepSeek-V3.2-TEE ",
            );
            std::env::set_var("CHUTES_ROUTING_STRATEGY", "Latency");
        }
        assert_eq!(
            auto_model_from_env(),
            "zai-org/GLM-5.1-TEE,deepseek-ai/DeepSeek-V3.2-TEE:latency"
        );
        unsafe {
            std::env::remove_var("CHUTES_ROUTING_POOL");
            std::env::set_var("CHUTES_ROUTING_STRATEGY", "throughput");
        }
        assert_eq!(auto_model_from_env(), "default:throughput");
        unsafe {
            std::env::set_var("CHUTES_ROUTING_STRATEGY", "nonsense");
        }
        // Unknown strategy falls back to the sequential alias.
        assert_eq!(auto_model_from_env(), "default");
        unsafe {
            std::env::remove_var("CHUTES_ROUTING_STRATEGY");
        }
        assert_eq!(auto_model_from_env(), "default");
    }
}
