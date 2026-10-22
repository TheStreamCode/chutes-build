use super::{direct_origin, read_projection};
use chutes_telemetry::events::{ReadLimitKind, ReadSkillMatch};
use chutes_tool_runtime::Tool;
use chutes_tools::implementations::chutes_build::ReadFileTool;
use chutes_tools::types::source_summary::{
    CapApplicability, CapDisposition, ReadDetail, ReadLimitSlot, ReadRole, ToolSourceDetail,
    ToolSourceResult, ToolSourceSummary,
};
use chutes_tools::types::tool::ToolNamespace;
use chutes_tools::types::tool_call_origin::InvocationSource;

#[test]
fn system_origin_leaves_the_model_absent_unless_it_is_known() {
    let absent = direct_origin(
        InvocationSource::System,
        "call-1",
        None,
        None,
        None,
        "opaque",
        None,
    );
    assert!(absent.requested_model().is_none());
    let known = direct_origin(
        InvocationSource::UserDirect,
        "call-2",
        Some("session"),
        Some(4),
        Some("model-4.6"),
        "ChutesBuild:read_file",
        None,
    );
    assert_eq!(known.requested_model(), Some("model-4.6"));
    assert_eq!(known.source(), InvocationSource::UserDirect);
}

#[test]
fn two_caps_project_to_multiple() {
    let mut detail = ReadDetail::unknown();
    detail.role = ReadRole::Ordinary;
    detail.skill_match = chutes_tools::types::source_summary::RegistryMatch::Unknown;
    detail.lines = ReadLimitSlot {
        applicability: CapApplicability::Applies,
        configured: Some(2),
        observed: Some(6),
        disposition: CapDisposition::Truncated,
    };
    detail.formatted_bytes = ReadLimitSlot {
        applicability: CapApplicability::Applies,
        configured: Some(15),
        observed: Some(40),
        disposition: CapDisposition::Truncated,
    };
    detail.tokens = ReadLimitSlot {
        applicability: CapApplicability::Applies,
        configured: Some(25_000),
        observed: None,
        disposition: CapDisposition::WithinLimit,
    };
    let summary = ToolSourceSummary {
        result: ToolSourceResult::Succeeded,
        output_limit: detail.output_limit(),
        detail: ToolSourceDetail::Read(detail),
    };
    let tool_id = chutes_telemetry::events::CanonicalToolId::from_qualified(&format!(
        "{}:{}",
        ToolNamespace::ChutesBuild,
        ReadFileTool.id()
    ))
    .expect("registered read id");
    let (_, _, read) = read_projection(&tool_id, Some(&summary));
    let read = read.expect("read profile");
    assert_eq!(read.read_limit_kind, ReadLimitKind::Multiple);
    assert_eq!(read.read_skill_match, ReadSkillMatch::Unknown);
    assert!(read.read_tokens_observed.is_none());
}
