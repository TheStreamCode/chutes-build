use agent_client_protocol as acp;
use xai_grok_tools::implementations::chutes::GENERATE_MEDIA_TOOL_NAME;
use xai_grok_tools::implementations::grok_build::{
    IMAGINE_VIDEO_COMMAND_NAME, imagine_video_instruction, imagine_video_usage_message,
};

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

const REQUIRED_TOOLS: &[&str] = &[GENERATE_MEDIA_TOOL_NAME];

pub struct ImagineVideoCommand;

impl SlashCommand for ImagineVideoCommand {
    slash_meta! {
        name: IMAGINE_VIDEO_COMMAND_NAME,
        description: "Generate a video from a text description",
        usage: "/imagine-video <description>",
        takes_args: true,
        args_required: true,
        arg_placeholder: "description of the video to generate",
        required_tools: REQUIRED_TOOLS,
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let prompt = args.trim();
        if prompt.is_empty() {
            return CommandResult::Message(imagine_video_usage_message().to_string());
        }

        CommandResult::InjectSkill {
            display_text: format!("/imagine-video {prompt}"),
            prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(
                imagine_video_instruction(prompt),
            ))],
            display_as_skill: false,
            scheduled_task_preview: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_generate_media_tool() {
        assert_eq!(ImagineVideoCommand.required_tools(), &["generate_media"]);
    }

    #[test]
    fn empty_prompt_returns_usage() {
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = super::super::tests::make_ctx(&models);
        let result = ImagineVideoCommand.run(&mut ctx, "");
        assert!(matches!(result, CommandResult::Message(_)));
    }

    #[test]
    fn whitespace_prompt_returns_usage() {
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = super::super::tests::make_ctx(&models);
        let result = ImagineVideoCommand.run(&mut ctx, "   ");
        assert!(matches!(result, CommandResult::Message(_)));
    }

    #[test]
    fn valid_prompt_returns_inject_skill() {
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = super::super::tests::make_ctx(&models);
        let result = ImagineVideoCommand.run(&mut ctx, "a cat playing piano");
        match result {
            CommandResult::InjectSkill {
                display_text,
                prompt_blocks,
                display_as_skill,
                ..
            } => {
                assert_eq!(display_text, "/imagine-video a cat playing piano");
                assert!(!display_as_skill);
                assert_eq!(prompt_blocks.len(), 1);
                let [acp::ContentBlock::Text(t)] = prompt_blocks.as_slice() else {
                    panic!("expected Text block, got {prompt_blocks:?}");
                };
                let text = &t.text;
                assert!(
                    text.contains("generate_media"),
                    "skill should reference generate_media"
                );
                assert!(
                    text.contains("list_media_models"),
                    "skill should reference list_media_models"
                );
                assert!(text.contains("a cat playing piano"));
            }
            other => panic!("expected InjectSkill, got {other:?}"),
        }
    }
}
