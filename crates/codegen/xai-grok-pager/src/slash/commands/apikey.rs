//! `/apikey` -- log in by pasting a Chutes API key directly.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct ApiKeyCommand;

impl SlashCommand for ApiKeyCommand {
    fn name(&self) -> &str {
        "apikey"
    }

    fn description(&self) -> &str {
        "Log in by pasting a Chutes API key directly"
    }

    fn usage(&self) -> &str {
        "/apikey"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::EnterApiKey)
    }
}
