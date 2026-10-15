//! `/apikey` -- how to authenticate with a Chutes API key.
//!
//! There is no login flow: the key is read from the `CHUTES_API_KEY` env var
//! (live on every start). This command prints those instructions instead of
//! dispatching anything.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub struct ApiKeyCommand;

impl SlashCommand for ApiKeyCommand {
    slash_meta! {
        name: "apikey",
        description: "Show how to authenticate with a Chutes API key",
        usage: "/apikey",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Message(
            "Authenticate with a Chutes API key (get one at https://chutes.ai):\n\
             - Set the `CHUTES_API_KEY` env var before starting; export it in your shell profile to persist it.\n\
             Then restart the app to pick up the new credentials."
                .to_string(),
        )
    }
}
