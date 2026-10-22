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
        (
            ToolFeature::ImageEdit,
            vec![chutes_build::ImageEditTool.id()],
        ),
        (ToolFeature::ImageGen, vec![chutes_build::ImageGenTool.id()]),
        (ToolFeature::LspTools, vec![chutes_build::LspTool.id()]),
        (
            ToolFeature::VideoGen,
            vec![
                chutes_build::ImageToVideoTool.id(),
                chutes_build::ReferenceToVideoTool.id(),
            ],
        ),
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
}
