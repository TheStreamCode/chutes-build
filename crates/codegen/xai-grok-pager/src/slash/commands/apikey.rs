//! `/apikey` -- how to provide your Chutes API key.
//!
//! There is no login flow. The key comes from `CHUTES_API_KEY`, the saved key
//! file, or per-model config; this command prints those instructions instead
//! of dispatching anything.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub struct ApiKeyCommand;

impl SlashCommand for ApiKeyCommand {
    slash_meta! {
        name: "apikey",
        description: "Show how to provide your Chutes API key",
        usage: "/apikey",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Message(
            "Authenticate with a Chutes API key (get one at https://chutes.ai/app/settings/api-keys):\n\
             - Save it with `chutes-build login`, or enter it on the welcome screen (applies immediately).\n\
             - Or set the `CHUTES_API_KEY` env var before starting; an explicit env key always wins.\n\
             Changing an existing key requires restarting the app."
                .to_string(),
        )
    }
}
