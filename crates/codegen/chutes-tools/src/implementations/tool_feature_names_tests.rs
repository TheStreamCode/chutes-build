use chutes_config::ToolFeature;
use chutes_tool_runtime::Tool;

use super::{chutes_build, opencode};

#[test]
fn tool_features_name_the_tools_they_remove() {
    let cases = [
        (
            ToolFeature::AskUserQuestion,
            vec![chutes_build::AskUserQuestionTool.id()],
        ),
        (ToolFeature::LspTools, vec![chutes_build::LspTool.id()]),
        (ToolFeature::WebFetch, vec![chutes_build::WebFetchTool.id()]),
        (
            ToolFeature::WriteFile,
            vec![opencode::OpenCodeWriteTool.id()],
        ),
    ];
    for (feature, tools) in cases {
        let names: Vec<&str> = tools.iter().map(|id| id.as_str()).collect();
        assert_eq!(names, feature.tool_names(), "{feature:?}");
    }
    // The legacy Imagine features remain parseable but no longer name a tool.
    for feature in [
        ToolFeature::ImageGen,
        ToolFeature::ImageEdit,
        ToolFeature::VideoGen,
    ] {
        assert!(
            feature.tool_names().is_empty(),
            "{feature:?} must not gate a removed tool"
        );
    }
}
