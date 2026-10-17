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
