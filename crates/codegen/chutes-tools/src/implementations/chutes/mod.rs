//! Chutes-native ecosystem tools.

pub mod account;
pub mod browser;
pub mod context7;
pub mod document;
pub mod embed;
pub mod media;
pub mod ocr;
pub mod speech;

pub use account::{GetChutesUsageInput, GetChutesUsageTool};
pub use browser::{BrowserClient, BrowserInput, BrowserTool};
pub use context7::{Context7DocsInput, Context7DocsTool, Context7SearchInput, Context7SearchTool};
pub use document::{ExtractDocumentInput, ExtractDocumentTool};
pub use embed::{EmbedTextsInput, EmbedTextsTool};
pub use media::{
    DescribeMediaModelInput, DescribeMediaModelTool, GenerateMediaInput, GenerateMediaTool,
    ListMediaModelsInput, ListMediaModelsTool,
};
pub use ocr::{OcrPageInput, OcrPageTool};
pub use speech::{TranscribeAudioInput, TranscribeAudioTool};
/// Canonical tool name, shared with the pager so `/imagine` and
/// `/imagine-video` gate on the tool that actually exists here.
pub const GENERATE_MEDIA_TOOL_NAME: &str = "generate_media";

/// Credentials for a Chutes tool call: the dispatch-published context value
/// first — it sees the agent's runtime key, which plain env reads miss —
/// then `CHUTES_API_KEY` for harnesses that build their own context.
pub(crate) fn resolve_credentials(
    ctx: &chutes_tool_runtime::ToolCallContext,
) -> Result<chutes_build_core::ChutesCredentials, chutes_build_core::endpoints::CredentialError> {
    if let Some(credentials) = ctx.get::<chutes_build_core::ChutesCredentials>() {
        return Ok((*credentials).clone());
    }
    chutes_build_core::ChutesCredentials::from_env()
}

/// Register the Chutes-native tools exactly once, so curated agent toolsets
/// listing `ChutesBuild:*` ids resolve at finalize time. Called from the agent
/// builder ahead of every finalize (mirrors the 1.0.12 up-front registration).
pub fn ensure_registered() {
    static REGISTERED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    REGISTERED.get_or_init(|| {
        crate::registry::types::register_tool_pack(register_chutes_pack);
    });
}

fn register_chutes_pack(b: &mut crate::registry::types::ToolRegistryBuilder) {
    b.register::<Context7SearchTool>();
    b.register::<Context7DocsTool>();
    b.register::<GetChutesUsageTool>();
    b.register::<ListMediaModelsTool>();
    b.register::<DescribeMediaModelTool>();
    b.register::<GenerateMediaTool>();
    b.register::<BrowserTool>();
    b.register::<OcrPageTool>();
    b.register::<TranscribeAudioTool>();
    b.register::<ExtractDocumentTool>();
    b.register::<EmbedTextsTool>();
}

// The registry's `register` bound needs `Args: Into<ToolInput>`. The Chutes
// inputs have no dedicated `ToolInput` variant (they are not upstream tools),
// so they convert through the dynamic JSON catch-all, mirroring the opencode
// pack's per-tool impls. Centralized here because the pack registers as a unit.
macro_rules! dynamic_tool_input {
    ($($input:ty),+) => {
        $(
            impl From<$input> for crate::types::ToolInput {
                fn from(value: $input) -> Self {
                    crate::types::ToolInput::Dynamic(
                        serde_json::to_value(value).expect("chutes tool input serializes to JSON"),
                    )
                }
            }
        )+
    };
}

dynamic_tool_input!(
    GetChutesUsageInput,
    BrowserInput,
    Context7SearchInput,
    Context7DocsInput,
    ListMediaModelsInput,
    DescribeMediaModelInput,
    GenerateMediaInput,
    OcrPageInput,
    TranscribeAudioInput,
    ExtractDocumentInput,
    EmbedTextsInput
);

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ctx() -> chutes_tool_runtime::ToolCallContext {
        chutes_tool_runtime::ToolCallContext::new(chutes_tool_protocol::ToolCallId::new_v7())
    }

    fn ctx_with(key: &str) -> chutes_tool_runtime::ToolCallContext {
        let mut ctx = empty_ctx();
        ctx.insert(chutes_build_core::ChutesCredentials::from_key(key).expect("test key builds"));
        ctx
    }

    /// Save both key vars; restore on drop. The tests below are `#[serial]`
    /// against each other; no other test in this binary touches these vars.
    struct EnvRestore {
        key: Option<std::ffi::OsString>,
        legacy: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn capture() -> Self {
            Self {
                key: std::env::var_os("CHUTES_API_KEY"),
                legacy: std::env::var_os("CHUTES_BUILD_API_KEY"),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            // SAFETY: `#[serial]` within this module; restored unconditionally.
            unsafe {
                match self.key.take() {
                    Some(value) => std::env::set_var("CHUTES_API_KEY", value),
                    None => std::env::remove_var("CHUTES_API_KEY"),
                }
                match self.legacy.take() {
                    Some(value) => std::env::set_var("CHUTES_BUILD_API_KEY", value),
                    None => std::env::remove_var("CHUTES_BUILD_API_KEY"),
                }
            }
        }
    }

    #[serial_test::serial]
    #[test]
    fn ctx_credentials_win_without_env() {
        let _restore = EnvRestore::capture();
        // SAFETY: serialized; restored by `_restore`.
        unsafe {
            std::env::remove_var("CHUTES_API_KEY");
            std::env::remove_var("CHUTES_BUILD_API_KEY");
        }
        let resolved = resolve_credentials(&ctx_with("cpk-ctx-key")).expect("ctx key resolves");
        assert_eq!(
            resolved,
            chutes_build_core::ChutesCredentials::from_key("cpk-ctx-key").expect("test key builds")
        );
    }

    #[serial_test::serial]
    #[test]
    fn falls_back_to_env_without_ctx_value() {
        let _restore = EnvRestore::capture();
        // SAFETY: serialized; restored by `_restore`.
        unsafe {
            std::env::set_var("CHUTES_API_KEY", "cpk-env-key");
            std::env::remove_var("CHUTES_BUILD_API_KEY");
        }
        let resolved = resolve_credentials(&empty_ctx()).expect("env key resolves");
        assert_eq!(
            resolved,
            chutes_build_core::ChutesCredentials::from_key("cpk-env-key").expect("test key builds")
        );
    }

    #[serial_test::serial]
    #[test]
    fn ctx_credentials_win_over_env() {
        let _restore = EnvRestore::capture();
        // SAFETY: serialized; restored by `_restore`.
        unsafe {
            std::env::set_var("CHUTES_API_KEY", "cpk-env-key");
            std::env::remove_var("CHUTES_BUILD_API_KEY");
        }
        let resolved = resolve_credentials(&ctx_with("cpk-ctx-key")).expect("ctx key resolves");
        assert_eq!(
            resolved,
            chutes_build_core::ChutesCredentials::from_key("cpk-ctx-key").expect("test key builds")
        );
    }

    #[serial_test::serial]
    #[test]
    fn missing_without_ctx_value_or_env() {
        let _restore = EnvRestore::capture();
        // SAFETY: serialized; restored by `_restore`.
        unsafe {
            std::env::remove_var("CHUTES_API_KEY");
            std::env::remove_var("CHUTES_BUILD_API_KEY");
        }
        assert!(matches!(
            resolve_credentials(&empty_ctx()),
            Err(chutes_build_core::endpoints::CredentialError::Missing)
        ));
    }
}
