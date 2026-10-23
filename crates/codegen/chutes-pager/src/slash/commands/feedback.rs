//! `/feedback`: open the public GitHub issue form.
//!
//! Chutes Build has no in-app feedback backend: remote feedback collection is
//! compiled out (product-spec §1.5/§7), so reports belong on the public GitHub
//! repository. The command copies any given text to the clipboard, opens the
//! issue-form chooser, and transmits nothing itself.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

/// The public repository's issue-form chooser (templates: bug report, feature request).
pub(crate) const FEEDBACK_ISSUES_URL: &str =
    "https://github.com/TheStreamCode/chutes-build/issues/new/choose";

/// `/feedback` opens the repository's issue form for feedback and bug reports.
pub struct FeedbackCommand;

impl SlashCommand for FeedbackCommand {
    slash_meta! {
        name: "feedback",
        description: "Open the GitHub issue form to report an issue",
        usage: "/feedback [text]",
        takes_args: true,
        arg_placeholder: "[feedback text]",
    }

    fn submission_refusal(&self, args: &str, voice_owns_prompt: bool) -> Option<&'static str> {
        if args.trim().is_empty() && voice_owns_prompt {
            Some("Stop voice input before opening the issue form")
        } else {
            None
        }
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let user_text = args.trim();
        let report = (!user_text.is_empty()).then(|| user_text.to_owned());
        let action = if report.is_some() {
            "copy_report_and_open"
        } else {
            "open_issue_chooser"
        };
        crate::unified_log::info(
            "feedback.command",
            ctx.session_id.map(|s| s.0.as_ref()),
            Some(serde_json::json!({
                "screen_mode": ctx.screen_mode.meta_label(),
                "arg_chars": user_text.chars().count(),
                "action": action,
            })),
        );
        CommandResult::Action(Action::OpenFeedbackIssue { report })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;

    fn make_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        let bundle = Box::leak(Box::new(crate::app::bundle::BundleState::default()));
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        }
    }

    fn opened_report(result: CommandResult) -> Option<String> {
        match result {
            CommandResult::Action(Action::OpenFeedbackIssue { report }) => report,
            other => panic!("feedback must open the GitHub issue form, got {other:?}"),
        }
    }

    /// The whitespace case matters: the composer keeps a trailing space while the user is still typing the command.
    #[test]
    fn bare_and_whitespace_open_the_issue_chooser() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);
        let cmd = FeedbackCommand;

        for args in ["", "   ", "\t"] {
            assert_eq!(
                opened_report(cmd.run(&mut ctx, args)),
                None,
                "{args:?} should open the issue chooser"
            );
        }
    }

    #[test]
    fn inline_text_copies_the_report_and_opens_the_issue_form() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models);

        assert_eq!(
            opened_report(FeedbackCommand.run(&mut ctx, "  todo is chopped  ")),
            Some("todo is chopped".to_owned())
        );
    }
}
