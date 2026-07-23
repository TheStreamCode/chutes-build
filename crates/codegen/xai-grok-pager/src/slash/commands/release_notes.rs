//! `/release-notes` -- view release notes for the current version.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

const EMBEDDED_CHANGELOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../CHANGELOG.md"
));

/// Show release notes for the current pager version.
pub struct ReleaseNotesCommand;

impl SlashCommand for ReleaseNotesCommand {
    fn name(&self) -> &str {
        "release-notes"
    }

    fn aliases(&self) -> &[&str] {
        &["changelog"]
    }

    fn description(&self) -> &str {
        "View release notes for the current version"
    }

    fn usage(&self) -> &str {
        "/release-notes"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::ShowReleaseNotes {
            title: "Release Notes".to_string(),
            content: EMBEDDED_CHANGELOG.trim().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_notes_metadata() {
        let cmd = ReleaseNotesCommand;
        assert_eq!(cmd.name(), "release-notes");
        assert_eq!(cmd.aliases(), &["changelog"]);
        assert!(!cmd.takes_args());
    }

    #[test]
    fn release_notes_are_available_offline() {
        let models = crate::acp::model_state::ModelState::default();
        let mut ctx = super::super::tests::make_ctx(&models);
        let result = ReleaseNotesCommand.run(&mut ctx, "");
        let CommandResult::Action(Action::ShowReleaseNotes { title, content }) = result else {
            panic!("expected ShowReleaseNotes action");
        };
        assert_eq!(title, "Release Notes");
        assert!(content.starts_with("# Changelog"));
        assert!(content.contains(env!("CARGO_PKG_VERSION")));
    }
}
