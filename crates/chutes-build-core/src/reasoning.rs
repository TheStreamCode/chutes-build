//! Central Chutes reasoning capability and wire-compatibility registry.
//!
//! Model catalogs describe whether a model can reason, but currently do not
//! describe how its chat template controls that reasoning. Keep the
//! model-specific knowledge here so the UI and sampler cannot drift apart.

#[path = "reasoning_efforts_snapshot.rs"]
mod reasoning_efforts_snapshot;

/// Chat-template switch understood by a model family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateSwitch {
    EnableThinking,
    Thinking,
    Both,
}

/// User-facing reasoning controls that are safe to expose for a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningProfile {
    /// The model does not advertise reasoning.
    Unsupported,
    /// The model reasons, but its published template has no supported switch.
    Fixed,
    /// The published template supports a binary instant/thinking switch.
    Toggle {
        switch: TemplateSwitch,
        default_enabled: bool,
    },
    /// GLM-5.2 supports instant, high, and maximum reasoning.
    Glm52,
    /// The catalog says the model reasons, but no verified control is known.
    Unknown,
}

impl ReasoningProfile {
    pub fn is_selectable(self) -> bool {
        matches!(self, Self::Toggle { .. } | Self::Glm52)
    }
}

/// Provider-neutral effort requested by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedReasoning {
    Unspecified,
    None,
    Minimal,
    Low,
    Medium,
    High,
    Maximum,
}

/// Effort values accepted by Chutes' OpenAI-compatible request schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireReasoningEffort {
    Medium,
    High,
}

/// Fully normalized controls to place on a Chutes chat-completions request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReasoningWirePlan {
    pub enable_thinking: Option<bool>,
    pub thinking: Option<bool>,
    pub reasoning_effort: Option<WireReasoningEffort>,
    /// Preserve a catalog-provided scalar that is newer than this profile.
    pub preserve_reasoning_effort: bool,
}

fn switched(switch: TemplateSwitch, enabled: bool) -> ReasoningWirePlan {
    match switch {
        TemplateSwitch::EnableThinking => ReasoningWirePlan {
            enable_thinking: Some(enabled),
            ..ReasoningWirePlan::default()
        },
        TemplateSwitch::Thinking => ReasoningWirePlan {
            thinking: Some(enabled),
            ..ReasoningWirePlan::default()
        },
        TemplateSwitch::Both => ReasoningWirePlan {
            enable_thinking: Some(enabled),
            thinking: Some(enabled),
            ..ReasoningWirePlan::default()
        },
    }
}

/// Return the verified reasoning control for a Chutes model identifier.
///
/// Rules intentionally target published model generations instead of broad
/// provider prefixes. An unknown future generation therefore fails closed
/// until Chutes publishes explicit effort metadata or its template is
/// verified.
pub fn reasoning_profile(model_id: &str) -> ReasoningProfile {
    let model = model_id.to_ascii_lowercase();

    if model == "model-router"
        || model.contains("qwen3-embedding")
        || model.contains("mistral-nemo-instruct")
    {
        return ReasoningProfile::Unsupported;
    }

    if model.contains("qwen3") && model.contains("thinking") {
        return ReasoningProfile::Fixed;
    }

    // Qwen3.8 verified 2026-10-14 against the published Qwen/Qwen3.8-27B
    // chat template: `enable_thinking` defaults to thinking, `false` emits
    // an empty think block. Same switch family as Qwen3.5/3.6.
    if model.contains("qwen3-32b")
        || model.contains("qwen3.5-")
        || model.contains("qwen3.6-")
        || model.contains("qwen3.8-")
    {
        return ReasoningProfile::Toggle {
            switch: TemplateSwitch::EnableThinking,
            default_enabled: true,
        };
    }

    if model.contains("deepseek-v3.2") {
        return ReasoningProfile::Toggle {
            // Chutes accepts both keys for the V3.2 serving adapter. Sending
            // both also keeps compatibility with its non-Jinja encoder.
            switch: TemplateSwitch::Both,
            default_enabled: true,
        };
    }

    if model.contains("minimax-m2.5") {
        return ReasoningProfile::Fixed;
    }

    if model.contains("kimi-k2.5") || model.contains("kimi-k2.6") {
        return ReasoningProfile::Toggle {
            switch: TemplateSwitch::Thinking,
            default_enabled: true,
        };
    }

    if model.contains("glm-5.2") {
        return ReasoningProfile::Glm52;
    }

    if model.contains("glm-5.1") || model.contains("glm-5-") {
        return ReasoningProfile::Toggle {
            switch: TemplateSwitch::EnableThinking,
            default_enabled: true,
        };
    }

    if model.contains("gemma-4-") {
        return ReasoningProfile::Toggle {
            switch: TemplateSwitch::EnableThinking,
            default_enabled: false,
        };
    }

    ReasoningProfile::Unknown
}

/// Models whose catalogue row omits `supported_features` but whose
/// tool-calling support was verified against the live endpoint (PAYG beta wave,
/// 2026-10-26). Auto may pool them once the subscription-only split is gone.
pub fn curated_supports_tools(model_id: &str) -> bool {
    let model = model_id.to_ascii_lowercase();
    [
        "deepseek-v4-pro-0813",
        "deepseek-v4.1-flash",
        "qwen3.8-max-0902",
        "qwen3.8-2.4t",
        "glm-5.3",
        "glm-5.2-fast",
        "kimi-k2.7",
    ]
    .iter()
    .any(|needle| model.contains(needle))
}

/// Reasoning-effort values verified against the live Chutes endpoint for
/// generations whose catalogue row does not publish a menu. The values are sent
/// verbatim as the top-level `reasoning_effort`; keep this list limited to
/// generations probed against the endpoint (2026-10-26).
pub fn curated_reasoning_efforts(model_id: &str) -> Option<&'static [&'static str]> {
    if let Some(values) = snapshot_efforts(model_id) {
        return Some(values);
    }
    let model = model_id.to_ascii_lowercase();
    if model.contains("deepseek-v4-pro-0813") {
        Some(&["low", "medium", "high"])
    } else if model.contains("deepseek-v4.1-flash") {
        Some(&["low", "medium", "high", "max"])
    } else if model.contains("qwen3.8-max-0902") || model.contains("qwen3.8-2.4t") {
        Some(&["low", "medium", "xhigh"])
    } else if model.contains("deepseek-v4-flash") {
        Some(&["minimal", "low", "medium", "high", "xhigh"])
    } else if model.contains("kimi-k3") || model.contains("kimi-k2.7") {
        Some(&["low", "medium", "high", "max"])
    } else if model.contains("glm-5.3") || model.contains("glm-5.2-fast") {
        Some(&["low", "high", "max"])
    } else if model.contains("glm-5.2") {
        Some(&["low", "medium", "high", "max"])
    } else {
        None
    }
}

fn snapshot_efforts(model_id: &str) -> Option<&'static [&'static str]> {
    reasoning_efforts_snapshot::EFFORT_SNAPSHOT
        .iter()
        .find(|(id, _)| id.eq_ignore_ascii_case(model_id))
        .map(|(_, values)| *values)
}

/// Convert a provider-neutral effort into a request compatible with the
/// selected model's published chat template and Chutes' request schema.
pub fn reasoning_wire_plan(model_id: &str, requested: RequestedReasoning) -> ReasoningWirePlan {
    match reasoning_profile(model_id) {
        ReasoningProfile::Toggle { switch, .. } => match requested {
            RequestedReasoning::Unspecified => ReasoningWirePlan::default(),
            RequestedReasoning::None => switched(switch, false),
            RequestedReasoning::High => switched(switch, true),
            RequestedReasoning::Minimal
            | RequestedReasoning::Low
            | RequestedReasoning::Medium
            | RequestedReasoning::Maximum => ReasoningWirePlan {
                // The built-in binary menu never emits these values. If one
                // arrives, it came from newer catalog/config metadata.
                preserve_reasoning_effort: true,
                ..ReasoningWirePlan::default()
            },
        },
        ReasoningProfile::Glm52 => match requested {
            RequestedReasoning::None => switched(TemplateSwitch::EnableThinking, false),
            RequestedReasoning::Maximum => ReasoningWirePlan {
                enable_thinking: Some(true),
                // The template maps every accepted non-`high` value to `max`.
                // `xhigh` itself is rejected by the gateway schema.
                reasoning_effort: Some(WireReasoningEffort::Medium),
                ..ReasoningWirePlan::default()
            },
            RequestedReasoning::Unspecified | RequestedReasoning::High => ReasoningWirePlan {
                enable_thinking: Some(true),
                reasoning_effort: Some(WireReasoningEffort::High),
                ..ReasoningWirePlan::default()
            },
            RequestedReasoning::Minimal | RequestedReasoning::Low | RequestedReasoning::Medium => {
                ReasoningWirePlan {
                    preserve_reasoning_effort: true,
                    ..ReasoningWirePlan::default()
                }
            }
        },
        ReasoningProfile::Unsupported | ReasoningProfile::Fixed | ReasoningProfile::Unknown => {
            ReasoningWirePlan::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_covers_the_probed_catalogue() {
        assert_eq!(
            curated_reasoning_efforts("deepseek-ai/DeepSeek-V4.1-Flash-1017"),
            Some(
                &[
                    "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"
                ][..]
            )
        );
        assert_eq!(
            curated_reasoning_efforts("zai-org/GLM-5.3"),
            Some(&["low", "high", "max"][..])
        );
        assert_eq!(
            curated_reasoning_efforts("zai-org/GLM-5.2-TEE"),
            Some(&["none", "low", "medium", "high", "max"][..])
        );
        assert_eq!(
            curated_reasoning_efforts("deepseek-ai/DeepSeek-V4-Pro-0813"),
            Some(&["low", "medium", "high", "xhigh", "max"][..])
        );
        assert_eq!(
            curated_reasoning_efforts("Qwen/Qwen3.8-Max-0902"),
            Some(&["none", "minimal", "low", "medium", "high", "xhigh", "max"][..])
        );
        assert_eq!(curated_reasoning_efforts("acme/unknown"), None);
    }

    #[test]
    fn unprobed_models_still_use_the_family_fallback() {
        assert_eq!(
            curated_reasoning_efforts("moonshotai/Kimi-K2.7-Code"),
            Some(&["low", "medium", "high", "max"][..])
        );
    }

    #[test]
    fn curated_tool_support_covers_verified_payg_models() {
        let yes = [
            "deepseek-ai/DeepSeek-V4-Pro-0813",
            "deepseek-ai/DeepSeek-V4.1-Flash-1017",
            "Qwen/Qwen3.8-Max-0902",
            "Qwen/Qwen3.8-2.4T-A95B",
            "zai-org/GLM-5.3",
            "zai-org/GLM-5.2-Fast",
            "moonshotai/Kimi-K2.7-Code",
        ];
        for model in yes {
            assert!(curated_supports_tools(model), "expected tools for {model}");
        }
        assert!(!curated_supports_tools("acme/unknown"));
        assert!(!curated_supports_tools("google/gemma-4-31B-turbo-TEE"));
    }

    #[test]
    fn current_chutes_catalog_has_conservative_profiles() {
        // Live catalog snapshot 2026-10-14 (14 models). MiniMax-M2.5 and the
        // base GLM-5 chute left the catalog; their generation rules stay so a
        // returning chute keeps its verified profile.
        let cases = [
            ("deepseek-ai/DeepSeek-V3.2-TEE", true),
            ("deepseek-ai/DeepSeek-V4-Flash-0731-TEE", false),
            ("google/gemma-4-31B-turbo-TEE", true),
            ("moonshotai/Kimi-K2.6-TEE", true),
            ("moonshotai/Kimi-K3-TEE", false),
            ("Nemotron-3-Nano-Omni-30B-TEE", false),
            ("Qwen/Qwen3-235B-A22B-Thinking-2507-TEE", false),
            ("Qwen/Qwen3-32B-TEE", true),
            ("Qwen/Qwen3.5-397B-A17B-TEE", true),
            ("Qwen/Qwen3.6-27B-TEE", true),
            ("Qwen/Qwen3.8-27B-TEE", true),
            ("unsloth/Mistral-Nemo-Instruct-2407-TEE", false),
            ("zai-org/GLM-5.1-TEE", true),
            ("zai-org/GLM-5.2-TEE", true),
        ];

        for (model, selectable) in cases {
            assert_eq!(
                reasoning_profile(model).is_selectable(),
                selectable,
                "unexpected reasoning profile for {model}"
            );
        }
    }

    #[test]
    fn unverifiable_new_generations_fail_closed() {
        // Kimi-K3, DeepSeek-V4-Flash and Nemotron-3-Nano publish no chat
        // template (HF repos carry no template; Nemotron is gated) and the
        // gateway exposes no per-model reasoning metadata, so all three stay
        // Unknown until a template switch is verified.
        for model in [
            "moonshotai/Kimi-K3-TEE",
            "deepseek-ai/DeepSeek-V4-Flash-0731-TEE",
            "Nemotron-3-Nano-Omni-30B-TEE",
        ] {
            assert_eq!(
                reasoning_profile(model),
                ReasoningProfile::Unknown,
                "unverified generation must fail closed for {model}"
            );
            assert_eq!(
                reasoning_wire_plan(model, RequestedReasoning::High),
                ReasoningWirePlan::default(),
                "unverified generation must send no reasoning keys for {model}"
            );
        }
    }

    #[test]
    fn future_unknown_generation_fails_closed() {
        assert_eq!(
            reasoning_profile("Qwen/Qwen4-Next-TEE"),
            ReasoningProfile::Unknown
        );
        assert_eq!(
            reasoning_wire_plan("Qwen/Qwen4-Next-TEE", RequestedReasoning::Maximum),
            ReasoningWirePlan::default()
        );
    }

    #[test]
    fn binary_models_use_their_native_template_key() {
        assert_eq!(
            reasoning_wire_plan("Qwen/Qwen3.6-27B-TEE", RequestedReasoning::None),
            ReasoningWirePlan {
                enable_thinking: Some(false),
                ..ReasoningWirePlan::default()
            }
        );
        assert_eq!(
            reasoning_wire_plan("moonshotai/Kimi-K2.6-TEE", RequestedReasoning::High),
            ReasoningWirePlan {
                thinking: Some(true),
                ..ReasoningWirePlan::default()
            }
        );
    }

    #[test]
    fn fixed_thinking_models_ignore_invalid_disable_requests() {
        for model in [
            "MiniMaxAI/MiniMax-M2.5-TEE",
            "Qwen/Qwen3-235B-A22B-Thinking-2507-TEE",
        ] {
            assert_eq!(
                reasoning_wire_plan(model, RequestedReasoning::None),
                ReasoningWirePlan::default()
            );
        }
    }

    #[test]
    fn glm_52_max_uses_gateway_compatible_wire_value() {
        assert_eq!(
            reasoning_wire_plan("zai-org/GLM-5.2-TEE", RequestedReasoning::Maximum),
            ReasoningWirePlan {
                enable_thinking: Some(true),
                reasoning_effort: Some(WireReasoningEffort::Medium),
                ..ReasoningWirePlan::default()
            }
        );
    }

    #[test]
    fn unexpected_new_scalar_is_preserved_for_catalog_driven_updates() {
        let plan = reasoning_wire_plan("Qwen/Qwen3.6-27B-TEE", RequestedReasoning::Medium);
        assert!(plan.preserve_reasoning_effort);
        assert_eq!(plan.enable_thinking, None);
    }
}
