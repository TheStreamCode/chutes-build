//! Chutes account and usage tool.

use chutes_build_core::account::ChutesAccountClient;

use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetChutesUsageInput {
    /// Include per-model invocation statistics. Disabled by default to minimize data retrieval.
    #[serde(default)]
    pub include_model_stats: bool,
}

#[derive(Debug, Default)]
pub struct GetChutesUsageTool;

impl crate::types::tool_metadata::ToolMetadata for GetChutesUsageTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Read the authenticated Chutes subscription, quotas, and current usage. Per-model statistics are opt-in; the user profile and API key are never returned."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for GetChutesUsageTool {
    type Args = GetChutesUsageInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        tool_id()
    }

    fn description(
        &self,
        _: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "get_chutes_usage",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    async fn run(
        &self,
        _: xai_tool_runtime::ToolCallContext,
        input: GetChutesUsageInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        let snapshot = ChutesAccountClient::from_env()
            .map_err(execution_error)?
            .usage_snapshot(input.include_model_stats)
            .await
            .map_err(execution_error)?;
        serde_json::to_string_pretty(&snapshot)
            .map(|text| ToolOutput::Text(text.into()))
            .map_err(execution_error)
    }
}

fn tool_id() -> xai_tool_protocol::ToolId {
    xai_tool_protocol::ToolId::new("get_chutes_usage").expect("valid tool id")
}

fn execution_error(error: impl std::fmt::Display) -> xai_tool_runtime::ToolError {
    xai_tool_runtime::ToolError::execution(tool_id(), error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tool_metadata::ToolMetadata;

    #[test]
    fn account_tool_is_read_only_and_stats_are_opt_in() {
        let tool = GetChutesUsageTool;
        assert_eq!(tool.kind(), ToolKind::Read);
        assert!(!GetChutesUsageInput::default().include_model_stats);
    }
}
