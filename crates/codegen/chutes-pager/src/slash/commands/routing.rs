//! `/routing` shows the saved Chutes routing pool and live model latency.
//!
//! The command reads the cache filled by startup/`routing` fetches. On a cold
//! cache it starts a background refresh and asks the user to run it again, so
//! the TUI never blocks on the network.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub struct RoutingCommand;

impl SlashCommand for RoutingCommand {
    slash_meta! {
        name: "routing",
        description: "View saved Chutes routing pool and live model latency",
        usage: "/routing",
        takes_args: false,
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        if !args.trim().is_empty() {
            return CommandResult::Error(format!(
                "Unknown argument: {}. Use /routing",
                args.trim()
            ));
        }
        let aliases = chutes_build_core::routing_pools::cached_aliases();
        let stats = chutes_build_core::routing_pools::cached_stats();
        if aliases.is_none() || stats.is_none() {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async {
                    chutes_build_core::routing_pools::prefetch_view_from_env().await;
                });
            }
            return CommandResult::Message(
                "Routing data is loading. Run /routing again in a moment.".to_string(),
            );
        }
        let view = chutes_build_core::routing_pools::RoutingView {
            aliases: aliases.unwrap_or_default(),
            stats: stats.unwrap_or_default(),
            catalog: chutes_build_core::routing_pools::cached_catalog().unwrap_or_default(),
        };
        CommandResult::Message(crate::routing_cmd::format_human(&view))
    }
}
