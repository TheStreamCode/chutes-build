//! Native Context7 documentation lookup tools.

use chutes_build_core::context7::Context7Client;

use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Context7SearchInput {
    /// Exact package or library name, for example `tokio` or `next.js`.
    pub library_name: String,
    /// The concrete API, setup, migration, or implementation question.
    pub query: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Context7DocsInput {
    /// Context7 library id returned by `context7_search`, for example `/tokio-rs/tokio`.
    pub library_id: String,
    /// The concrete documentation question. Never include secrets or private code.
    pub query: String,
    /// Desired context size. Values are clamped to 1,000..20,000 tokens.
    pub tokens: Option<u32>,
}

#[derive(Debug, Default)]
pub struct Context7SearchTool;

impl crate::types::tool_metadata::ToolMetadata for Context7SearchTool {
    fn kind(&self) -> ToolKind {
        ToolKind::WebSearch
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Find the canonical Context7 library id for version-aware coding documentation. Use the exact dependency name and a concrete technical question. Do not send secrets, private code, credentials, or proprietary data."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for Context7SearchTool {
    type Args = Context7SearchInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("context7_search").expect("valid tool id")
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "context7_search",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        read_only_capabilities()
    }

    async fn run(
        &self,
        _: xai_tool_runtime::ToolCallContext,
        input: Context7SearchInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        reject_sensitive(&input.library_name, &input.query, "context7_search")?;
        let result = Context7Client::default()
            .search_libraries(input.library_name.trim(), input.query.trim())
            .await
            .map_err(|error| execution_error("context7_search", error))?;
        render_json(result)
    }
}

#[derive(Debug, Default)]
pub struct Context7DocsTool;

impl crate::types::tool_metadata::ToolMetadata for Context7DocsTool {
    fn kind(&self) -> ToolKind {
        ToolKind::WebFetch
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Retrieve current, focused documentation from Context7 using a library id returned by context7_search. Query only public technical concepts; never send secrets or private code."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for Context7DocsTool {
    type Args = Context7DocsInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("context7_docs").expect("valid tool id")
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "context7_docs",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        read_only_capabilities()
    }

    async fn run(
        &self,
        _: xai_tool_runtime::ToolCallContext,
        input: Context7DocsInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        reject_sensitive(&input.library_id, &input.query, "context7_docs")?;
        let result = Context7Client::default()
            .get_context(input.library_id.trim(), input.query.trim(), input.tokens)
            .await
            .map_err(|error| execution_error("context7_docs", error))?;
        render_json(result)
    }
}

fn reject_sensitive(
    identity: &str,
    query: &str,
    tool: &str,
) -> Result<(), xai_tool_runtime::ToolError> {
    let combined = format!("{identity}\n{query}");
    if chutes_build_core::privacy::contains_probable_secret(&combined) {
        return Err(execution_error(
            tool,
            "request blocked locally because it appears to contain a secret",
        ));
    }
    Ok(())
}

fn render_json(value: serde_json::Value) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
    serde_json::to_string_pretty(&value)
        .map(|text| ToolOutput::Text(text.into()))
        .map_err(|error| execution_error("context7", error))
}

fn execution_error(tool: &str, error: impl std::fmt::Display) -> xai_tool_runtime::ToolError {
    xai_tool_runtime::ToolError::execution(
        xai_tool_protocol::ToolId::new(tool).expect("valid tool id"),
        error.to_string(),
    )
}

fn read_only_capabilities() -> xai_tool_protocol::ToolCapabilities {
    xai_tool_protocol::ToolCapabilities {
        is_read_only: true,
        tool_scope: Some(xai_tool_protocol::ToolScope::Read),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context7_tools_are_read_only() {
        assert!(xai_tool_runtime::Tool::capabilities(&Context7SearchTool).is_read_only);
        assert!(xai_tool_runtime::Tool::capabilities(&Context7DocsTool).is_read_only);
    }

    #[test]
    fn local_secret_guard_blocks_tokens() {
        assert!(
            reject_sensitive(
                "tokio",
                "api_key=abcdefghijklmnopqrstuvwxyz123456",
                "context7_search"
            )
            .is_err()
        );
    }

    /// This guard shares detection rules with memory persistence and log
    /// sanitization via `chutes_build_core::privacy::contains_probable_secret`
    /// -> `xai_grok_secrets::detect_probable_secret`; a secret shape with no
    /// keyword marker (an AWS key has no "api_key"/"token"/etc. substring)
    /// must still be blocked here, not just in the other two call sites.
    #[test]
    fn local_secret_guard_blocks_shapes_without_a_keyword_marker() {
        assert!(
            reject_sensitive(
                "tokio",
                "see aws AKIAABCDEFGHIJKLMNOP for setup",
                "context7_search"
            )
            .is_err()
        );
    }
}
