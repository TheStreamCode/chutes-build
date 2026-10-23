use agent_client_protocol as acp;
use chutes_tools::implementations::chutes::GENERATE_MEDIA_TOOL_NAME;
use chutes_tools::implementations::chutes_build::{IMAGINE_COMMAND_NAME, imagine_instruction};

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

const REQUIRED_TOOLS: &[&str] = &[GENERATE_MEDIA_TOOL_NAME];

pub struct ImagineCommand;

impl SlashCommand for ImagineCommand {
    slash_meta! {
        name: IMAGINE_COMMAND_NAME,
        description: "Generate an image from a text description",
        usage: "/imagine [model] <description>",
        takes_args: true,
        arg_placeholder: "description, optionally after a model id",
        required_tools: REQUIRED_TOOLS,
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let args = args.trim();
        // Bare `/imagine` asks the model to list the live image models; any
        // argument generates, either with a model the user named or with one
        // the tool picks from the catalog.
        CommandResult::InjectSkill {
            display_text: if args.is_empty() {
                "/imagine".to_owned()
            } else {
                format!("/imagine {args}")
            },
            prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(
                imagine_instruction(args),
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
        assert_eq!(ImagineCommand.required_tools(), &["generate_media"]);
    }

    #[test]
    fn empty_prompt_lists_live_models() {
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = super::super::tests::make_ctx(&models);
        match ImagineCommand.run(&mut ctx, "") {
            CommandResult::InjectSkill {
                display_text,
                prompt_blocks,
                ..
            } => {
                assert_eq!(display_text, "/imagine");
                let text = format!("{:?}", prompt_blocks[0]);
                assert!(text.contains("list_media_models"));
            }
            other => panic!("bare /imagine must list models, got {other:?}"),
        }
    }

    #[test]
    fn whitespace_prompt_lists_live_models() {
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = super::super::tests::make_ctx(&models);
        match ImagineCommand.run(&mut ctx, "   ") {
            CommandResult::InjectSkill { prompt_blocks, .. } => {
                assert!(format!("{:?}", prompt_blocks[0]).contains("list_media_models"));
            }
            other => panic!("whitespace must behave like empty, got {other:?}"),
        }
    }

    #[test]
    fn named_model_and_prompt_reach_the_instruction() {
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = super::super::tests::make_ctx(&models);
        match ImagineCommand.run(&mut ctx, "z-image-turbo a spaceship in space") {
            CommandResult::InjectSkill {
                display_text,
                prompt_blocks,
                ..
            } => {
                assert_eq!(display_text, "/imagine z-image-turbo a spaceship in space");
                let text = format!("{:?}", prompt_blocks[0]);
                assert!(text.contains("z-image-turbo"));
                assert!(text.contains("a spaceship in space"));
            }
            other => panic!("expected inject_skill, got {other:?}"),
        }
    }

    #[test]
    fn valid_prompt_returns_inject_skill() {
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = super::super::tests::make_ctx(&models);
        let result = ImagineCommand.run(&mut ctx, "a golden sunset");
        match result {
            CommandResult::InjectSkill {
                display_text,
                prompt_blocks,
                display_as_skill,
                ..
            } => {
                assert_eq!(display_text, "/imagine a golden sunset");
                assert!(!display_as_skill);
                assert_eq!(prompt_blocks.len(), 1);
                let [acp::ContentBlock::Text(t)] = prompt_blocks.as_slice() else {
                    panic!("expected Text block, got {prompt_blocks:?}");
                };
                let text = &t.text;
                assert!(text.contains("generate_media"));
                assert!(text.contains("a golden sunset"));
            }
            other => panic!("expected InjectSkill, got {other:?}"),
        }
    }
}
