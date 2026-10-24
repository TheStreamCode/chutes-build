//! `ToolMetadata` — chutes-tools-specific metadata for tools.
//!
//! Each tool implements two traits:
//! 1. `chutes_tool_runtime::Tool` — typed Args/Output, `run()` with actual logic
//! 2. `ToolMetadata` — kind, namespace, description template, and optional
//!    overrides for fingerprinting, reminders, etc.
//!
//! Only three methods are required (`kind`, `tool_namespace`,
//! `description_template`); all others have sensible defaults derived
//! from `kind()`.
//!
//! ## Context helpers
//!
//! Tools access session state through `chutes_tool_runtime::ToolCallContext`
//! extensions. This module provides helper functions to extract
//! `SharedResources`, resolve the working directory, and read the
//! behavior version.

use std::path::PathBuf;

use crate::types::definition::ToolDefinition;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::SharedResources;
use crate::types::template_renderer::TemplateRenderer;
use crate::types::tool::{ToolKind, ToolNamespace};

/// Chutes Build-tools-specific metadata trait. Each tool struct implements this alongside `chutes_tool_runtime::Tool`. Only `kind()`, `namespace()`, and
/// `description_template()` are required; all other methods have defaults. The `ToolRegistry` stores a type-erased handle to each tool's
/// `ToolMetadata` impl so it can call `versioned_definition()`, etc. after dispatch.
pub trait ToolMetadata: Send + Sync {
    /// High-level category (Read, Edit, Search, Execute, ...).
    /// Drives template rendering (`${{ tools.by_kind.search }}`) and the
    /// default `is_read_only()` derivation.
    fn kind(&self) -> ToolKind;

    /// Namespace grouping (ChutesBuild, Cursor, OpenCode, ...).
    /// Used to build the fully-qualified tool ID at registration time
    /// (e.g., `"ChutesBuild:grep"`).
    fn tool_namespace(&self) -> ToolNamespace;

    /// Raw MiniJinja description template with `${{ tools.by_kind.X }}` and
    /// `${{ params.tool.param }}` placeholders. Resolved at finalize time by
    /// the `TemplateRenderer`.
    fn description_template(&self) -> &str;

    // -----------------------------------------------------------------------
    // Defaults — override only when needed
    // -----------------------------------------------------------------------

    /// Whether the tool is read-only (no filesystem / external side-effects).
    /// Default: derived from `kind()`.
    fn is_read_only(&self) -> bool {
        self.kind().is_read_only()
    }

    /// The canonical input field naming the workspace path this call touches
    /// (`file_path` on `search_replace`, `filePath` on opencode `edit`, ...).
    ///
    /// The tool server advertises it (under the client-facing name, post-rename)
    /// as `lock_path_param:<name>` so a client can serialize same-path writers
    /// from the arguments it sends, without a hand-kept table. The server itself
    /// does not lock: the client that issues the batch owns ordering, as the
    /// production dispatchers do. Default `None`: the tool has no single path
    /// key (shells, `apply_patch`, `todo_write`, directory listings whose
    /// `path` is a directory).
    fn lock_path_param(&self) -> Option<&'static str> {
        None
    }

    /// Notification variant tags this tool may emit during execution.
    /// Default: none. Tags match `ToolNotification`'s serde `type` discriminator
    /// (the keys of [`notification_schema_catalog`](crate::notification::notification_schema_catalog)).
    fn emitted_notifications(&self) -> &'static [&'static str] {
        &[]
    }

    /// Requirements expression evaluated at finalize time.
    /// Default: `Expr::True` (no requirements).
    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }

    /// Model-safe fallback description for `chutes_tool_runtime::Tool::description()` implementations: the raw template with all `${{ … }}` / `${% …
    /// %}` markers stripped. The registry path (`versioned_definition`) renders templates properly with the finalized toolset context; this is only
    /// for consumers that bypass the registry, which must never see raw template syntax.
    fn sanitized_description_template(&self) -> String {
        crate::types::template_renderer::strip_template_markers(self.description_template())
    }

    /// The input schema this tool advertises under `contract_version`, when it differs from `input_schema`.
    /// The registry fills `${{ params.<kind>.* }}` from this schema.
    fn advertised_input_schema(
        &self,
        _contract_version: Option<&str>,
        _input_schema: &serde_json::Value,
        _effective_params: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        None
    }

    /// Whether the model-facing definition may drop nested parameter descriptions.
    ///
    /// Dispatch always validates against the canonical `input_schema`; this only
    /// trims the exported copy for tools whose schema prose dominates the catalog.
    /// The tool-level description and every parameter name, type, enum, required
    /// entry, and union member survive compaction.
    fn compact_input_schema(&self) -> bool {
        false
    }

    /// Build the tool definition for a given contract version. Default: renders `description_template()` via the
    /// `TemplateRenderer` and remaps schema parameter names. Override for tools that need params-aware descriptions or
    /// schemas (e.g., BashTool removes `is_background` when disabled).
    fn versioned_definition(
        &self,
        _contract_version: Option<&str>,
        client_name: &str,
        description_override: Option<&str>,
        renderer: &TemplateRenderer,
        param_map: &std::collections::HashMap<String, String>,
        input_schema: &serde_json::Value,
        _effective_params: &serde_json::Value,
    ) -> ToolDefinition {
        let raw_desc = description_override.unwrap_or_else(|| self.description_template());
        let description = renderer.render(raw_desc).unwrap_or_else(|e| {
            crate::types::template_renderer::strip_markers_on_render_failure(raw_desc, &e)
        });
        let remapped_schema = if param_map.is_empty() {
            input_schema.clone()
        } else {
            crate::util::remap::remap_schema_properties(input_schema, param_map)
        };
        let remapped_schema = if self.compact_input_schema() {
            compact_tool_schema(&remapped_schema)
        } else {
            remapped_schema
        };
        ToolDefinition::function(client_name, Some(&description), remapped_schema)
    }
}

/// Drop nested schema prose from a model-facing copy: `description` and
/// `examples` keys disappear recursively, while parameter names, types, enums,
/// `required` lists, and union members survive unchanged.
pub(crate) fn compact_tool_schema(schema: &serde_json::Value) -> serde_json::Value {
    match schema {
        serde_json::Value::Object(map) => map
            .iter()
            .filter(|(key, _)| key.as_str() != "description" && key.as_str() != "examples")
            .map(|(key, value)| (key.clone(), compact_tool_schema(value)))
            .collect(),
        serde_json::Value::Array(items) => items.iter().map(compact_tool_schema).collect(),
        other => other.clone(),
    }
}

/// Extract `SharedResources` from the runtime tool-call context. `ToolBridge` inserts
/// `SharedResources` into `ctx.extensions` before dispatching through the `LocalRegistry`.
pub fn shared_resources(
    ctx: &chutes_tool_runtime::ToolCallContext,
) -> Result<SharedResources, chutes_tool_runtime::ToolError> {
    ctx.extensions
        .get::<SharedResources>()
        .map(|arc| (*arc).clone())
        .ok_or_else(|| {
            chutes_tool_runtime::ToolError::custom(
                "missing_resources",
                "SharedResources not available in ToolCallContext extensions",
            )
        })
}

/// Resolve the working directory from the runtime context. Checks `Cwd` extension first (set when
/// the caller provides a per-call override), then falls back to `Cwd` in `SharedResources`.
pub async fn resolve_cwd(
    ctx: &chutes_tool_runtime::ToolCallContext,
    resources: &SharedResources,
) -> Result<PathBuf, chutes_tool_runtime::ToolError> {
    if let Some(cwd) = ctx.extensions.get::<chutes_tool_runtime::Cwd>() {
        return Ok(cwd.0.clone());
    }
    let res = resources.lock().await;
    res.get::<crate::types::resources::Cwd>()
        .map(|c| c.0.clone())
        .ok_or_else(|| {
            chutes_tool_runtime::ToolError::custom("missing_cwd", "Cwd not available in Resources")
        })
}

/// Build a `ToolCallContext` with `SharedResources` installed and a fresh v7 call id. Convenience
/// for tests — replaces the per-tool `make_ctx` / `runtime_ctx` helpers that were duplicated across
/// ~50 tool implementations. Use [`test_ctx_with_call_id`] when the test needs a specific call id.
pub fn test_ctx(resources: SharedResources) -> chutes_tool_runtime::ToolCallContext {
    let mut ctx = chutes_tool_runtime::ToolCallContext::default();
    ctx.extensions.insert(resources);
    // Default streaming gate ON so existing tests exercise the stream path.
    ctx.extensions
        .insert(chutes_tool_runtime::WorkspaceViewerContext {
            stream_tool_progress: true,
        });
    ctx
}

/// Like [`test_ctx`] but with a caller-specified call id.
///
/// Falls back to a fresh v7 id if `call_id` is not a valid `ToolCallId`.
pub fn test_ctx_with_call_id(
    resources: SharedResources,
    call_id: &str,
) -> chutes_tool_runtime::ToolCallContext {
    let id = chutes_tool_protocol::ToolCallId::new(call_id)
        .unwrap_or_else(|_| chutes_tool_protocol::ToolCallId::new_v7());
    let mut ctx = chutes_tool_runtime::ToolCallContext::new(id);
    ctx.extensions.insert(resources);
    ctx.extensions
        .insert(chutes_tool_runtime::WorkspaceViewerContext {
            stream_tool_progress: true,
        });
    ctx
}

/// Read the behavior version from the runtime context, if set.
pub fn behavior_version(ctx: &chutes_tool_runtime::ToolCallContext) -> Option<String> {
    ctx.extensions
        .get::<chutes_tool_runtime::BehaviorVersion>()
        .map(|v| v.0.clone())
}

/// This tool's own canonical→client param-name map, stamped on the dispatch context by
/// `prepare_dispatch` / `call_raw`. Returns an empty (identity) map when absent — e.g. unit tests
/// that call `Tool::run` directly — so callers resolve to canonical names.
pub fn invoking_param_names(
    ctx: &chutes_tool_runtime::ToolCallContext,
) -> crate::types::resources::InvokingToolParamNames {
    ctx.extensions
        .get::<crate::types::resources::InvokingToolParamNames>()
        .map(|arc| (*arc).clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::compact_tool_schema;

    #[test]
    fn compact_schema_drops_prose_and_keeps_structure() {
        let schema = serde_json::json!({
            "type": "object",
            "description": "top-level prose",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "file to read",
                    "examples": ["src/main.rs"]
                },
                "kind": {
                    "type": "string",
                    "enum": ["image", "video"],
                    "description": "generation kind"
                },
                "options": {
                    "type": "object",
                    "properties": {
                        "steps": { "type": "integer", "description": "sampling steps" }
                    }
                }
            },
            "required": ["path", "kind"],
            "oneOf": [{ "description": "variant prose", "required": ["path"] }]
        });
        let compact = compact_tool_schema(&schema);
        let text = serde_json::to_string(&compact).expect("serializes");
        assert!(
            !text.contains("prose"),
            "nested descriptions were kept: {text}"
        );
        assert!(
            !text.contains("file to read"),
            "property descriptions were kept: {text}"
        );
        assert!(!text.contains("examples"), "examples were kept: {text}");
        assert_eq!(
            compact.pointer("/properties/kind/enum"),
            Some(&serde_json::json!(["image", "video"]))
        );
        assert_eq!(
            compact.pointer("/required"),
            Some(&serde_json::json!(["path", "kind"]))
        );
        assert_eq!(
            compact.pointer("/oneOf/0/required"),
            Some(&serde_json::json!(["path"]))
        );
        assert!(
            compact
                .pointer("/properties/options/properties/steps/type")
                .is_some()
        );
    }
}
