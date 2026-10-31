//! Curated context limits for catalogue rows that omit them.
//!
//! Chutes publishes `context_length` for TEE models but currently leaves it out
//! of the PAYG beta rows. The parser would then fall back to a generic 256k
//! window and the UI would under-report the model. These values come from the
//! vendor and models.dev canonical entries (verified 2026-10-31); extend this
//! table only with a checkable source.

/// Context window for a model whose Chutes row has no usable value.
pub fn curated_context_window(model_id: &str) -> Option<u64> {
    let model = model_id.to_ascii_lowercase();
    if model.contains("deepseek-v4.1-flash") || model.contains("deepseek-v4-pro-0813") {
        Some(1_048_576)
    } else if model.contains("qwen3.8-max") {
        Some(1_000_000)
    } else if model.contains("qwen3.8-2.4t") {
        Some(262_144)
    } else if model.contains("glm-5.3") || model.contains("glm-5.2-fast") {
        Some(1_000_000)
    } else if model.contains("kimi-k2.7") {
        Some(262_144)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_the_payg_rows_without_catalogue_context() {
        let cases = [
            ("deepseek-ai/DeepSeek-V4.1-Flash-1017", 1_048_576),
            ("deepseek-ai/DeepSeek-V4-Pro-0813", 1_048_576),
            ("Qwen/Qwen3.8-Max-0902", 1_000_000),
            ("Qwen/Qwen3.8-2.4T-A95B", 262_144),
            ("zai-org/GLM-5.3", 1_000_000),
            ("zai-org/GLM-5.2-Fast", 1_000_000),
            ("moonshotai/Kimi-K2.7-Code", 262_144),
        ];
        for (model, expected) in cases {
            assert_eq!(
                curated_context_window(model),
                Some(expected),
                "unexpected context for {model}"
            );
        }
        assert_eq!(
            curated_context_window("deepseek-ai/DeepSeek-V4-Flash-0731-TEE"),
            None,
            "catalogue rows with a real context_length must not be overridden"
        );
        assert_eq!(curated_context_window("acme/unknown"), None);
    }
}
