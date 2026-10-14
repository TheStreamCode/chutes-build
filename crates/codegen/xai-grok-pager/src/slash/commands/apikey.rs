//! `/apikey` -- how to log in with a Chutes API key.
//!
//! Upstream 1.0.45 removed the TUI paste-box login (`AuthMode::ApiKeyEntry`);
//! API-key login now happens outside the TUI via the `CHUTES_API_KEY` env var
//! (read live on every start). This command prints those instructions instead
//! of dispatching a login flow. Restoring an inline paste box would need a new
//! `AuthMode`, welcome-view render + input handling, and a shell endpoint that
//! persists the pasted key -- tracked as follow-up work, not part of the
//! 1.0.45 rebase.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub struct ApiKeyCommand;

impl SlashCommand for ApiKeyCommand {
    slash_meta! {
        name: "apikey",
        description: "Show how to log in with a Chutes API key",
        usage: "/apikey",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Message(
            "Log in with a Chutes API key (get one at https://chutes.ai):\n\
             - Set the `CHUTES_API_KEY` env var before starting; export it in your shell profile to persist it.\n\
             - Or register your own OAuth app (https://docs.chutes.ai/oauth), set `CHUTES_BUILD_OAUTH2_CLIENT_ID`, then run `chutes-build login`.\n\
             Then restart the app to pick up the new credentials."
                .to_string(),
        )
    }
}
