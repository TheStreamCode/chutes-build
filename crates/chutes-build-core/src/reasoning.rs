//! Central Chutes reasoning capability and wire-compatibility registry.
//!
//! Model catalogs describe whether a model can reason, but currently do not
//! describe how its chat template controls that reasoning. Keep the
//! model-specific knowledge here so the UI and sampler cannot drift apart.

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

    if model.contains("qwen3-32b") || model.contains("qwen3.5-") || model.contains("qwen3.6-") {
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
    fn current_chutes_catalog_has_conservative_profiles() {
        let cases = [
            ("deepseek-ai/DeepSeek-V3.2-TEE", true),
            ("google/gemma-4-31B-turbo-TEE", true),
            ("MiniMaxAI/MiniMax-M2.5-TEE", false),
            ("moonshotai/Kimi-K2.5-TEE", true),
            ("moonshotai/Kimi-K2.6-TEE", true),
            ("Qwen/Qwen3-235B-A22B-Thinking-2507-TEE", false),
            ("Qwen/Qwen3-32B-TEE", true),
            ("Qwen/Qwen3.5-397B-A17B-TEE", true),
            ("Qwen/Qwen3.6-27B-TEE", true),
            ("unsloth/Mistral-Nemo-Instruct-2407-TEE", false),
            ("zai-org/GLM-5-TEE", true),
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
