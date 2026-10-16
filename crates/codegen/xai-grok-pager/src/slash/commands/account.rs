//! `/account` shows the Chutes plan, quotas, usage, and PAYG credit for the configured API key.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub struct AccountCommand;

impl SlashCommand for AccountCommand {
    slash_meta! {
        name: "account",
        description: "View Chutes account (plan, quotas, usage, credit)",
        usage: "/account",
        takes_args: false,
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if !args.trim().is_empty() {
            return CommandResult::Error(format!(
                "Unknown argument: {}. Use /account",
                args.trim()
            ));
        }
        CommandResult::Action(Action::ShowAccount)
    }
}
