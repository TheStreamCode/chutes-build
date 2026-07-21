//! `/advisor` — enable/disable the built-in advisor subagent, or pin the
//! model it uses. Writes `[subagents.roles.advisor].model` / `[subagents.toggle].advisor`
//! in config.toml; the running session's own model is never touched.

use crate::app::actions::Action;
use crate::slash::command::{AppCtx, ArgItem, CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::commands::model::build_model_items;

const KEYWORDS: &[(&str, &str)] = &[
    ("on", "Enable the advisor subagent"),
    ("off", "Disable the advisor subagent"),
    (
        "default",
        "Clear the model pin (inherit the session's model)",
    ),
];

/// Enable/disable the built-in advisor, or pin its model.
pub struct AdvisorCommand;

impl SlashCommand for AdvisorCommand {
    fn name(&self) -> &str {
        "advisor"
    }

    fn description(&self) -> &str {
        "Enable/disable the advisor subagent, or pin its model"
    }

    fn usage(&self) -> &str {
        "/advisor on|off|default|<model>"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn args_required(&self) -> bool {
        true
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("on|off|default|<model>")
    }

    fn suggest_args(&self, ctx: &AppCtx, _args_query: &str) -> Option<Vec<ArgItem>> {
        let mut items: Vec<ArgItem> = KEYWORDS
            .iter()
            .map(|(word, description)| ArgItem {
                display: (*word).to_string(),
                match_text: (*word).to_string(),
                insert_text: (*word).to_string(),
                description: (*description).to_string(),
            })
            .collect();
        items.extend(build_model_items(ctx.models));
        Some(items)
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let trimmed = args.trim();
        match trimmed {
            "" => CommandResult::Error("Usage: /advisor on|off|default|<model>".into()),
            "on" => CommandResult::Action(Action::SetAdvisorEnabled(true)),
            "off" => CommandResult::Action(Action::SetAdvisorEnabled(false)),
            "default" | "clear" | "reset" | "inherit" => {
                CommandResult::Action(Action::SetAdvisorModel(String::new()))
            }
            name => match ctx.models.resolve_by_name_or_id(name) {
                Some(id) => CommandResult::Action(Action::SetAdvisorModel(id.0.to_string())),
                None => CommandResult::Error(format!("Unknown model: {name}")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use agent_client_protocol as acp;
    use std::sync::Arc;

    static EMPTY_BUNDLE: crate::app::bundle::BundleState = crate::app::bundle::BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn dummy_exec_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &EMPTY_BUNDLE,
            screen_mode: crate::app::ScreenMode::Inline,
            pager_state: crate::settings::PagerLocalSnapshot {
                multiline_mode: false,
                yolo_mode: false,
                ..crate::settings::PagerLocalSnapshot::default()
            },
        }
    }

    fn plain_model(id: &str, name: &str) -> (acp::ModelId, acp::ModelInfo) {
        let id = acp::ModelId::new(Arc::from(id));
        let info = acp::ModelInfo::new(id.clone(), name.to_string());
        (id, info)
    }

    #[test]
    fn empty_args_is_error() {
        let state = ModelState::default();
        let mut ctx = dummy_exec_ctx(&state);
        assert!(matches!(
            AdvisorCommand.run(&mut ctx, ""),
            CommandResult::Error(_)
        ));
    }

    #[test]
    fn on_dispatches_set_advisor_enabled_true() {
        let state = ModelState::default();
        let mut ctx = dummy_exec_ctx(&state);
        assert!(matches!(
            AdvisorCommand.run(&mut ctx, "on"),
            CommandResult::Action(Action::SetAdvisorEnabled(true))
        ));
    }

    #[test]
    fn off_dispatches_set_advisor_enabled_false() {
        let state = ModelState::default();
        let mut ctx = dummy_exec_ctx(&state);
        assert!(matches!(
            AdvisorCommand.run(&mut ctx, "off"),
            CommandResult::Action(Action::SetAdvisorEnabled(false))
        ));
    }

    #[test]
    fn default_clears_the_model_pin() {
        let state = ModelState::default();
        let mut ctx = dummy_exec_ctx(&state);
        match AdvisorCommand.run(&mut ctx, "default") {
            CommandResult::Action(Action::SetAdvisorModel(id)) => assert_eq!(id, ""),
            other => panic!("expected SetAdvisorModel(\"\"), got {other:?}"),
        }
    }

    #[test]
    fn known_model_name_pins_it() {
        let mut state = ModelState::default();
        let (id, info) = plain_model("kimi-k2.6", "Kimi K2.6");
        state.available.insert(id.clone(), info);
        let mut ctx = dummy_exec_ctx(&state);
        match AdvisorCommand.run(&mut ctx, "Kimi K2.6") {
            CommandResult::Action(Action::SetAdvisorModel(resolved)) => {
                assert_eq!(resolved, id.0.as_ref());
            }
            other => panic!("expected SetAdvisorModel(kimi-k2.6), got {other:?}"),
        }
    }

    #[test]
    fn unknown_model_name_is_error() {
        let state = ModelState::default();
        let mut ctx = dummy_exec_ctx(&state);
        assert!(matches!(
            AdvisorCommand.run(&mut ctx, "not-a-real-model"),
            CommandResult::Error(_)
        ));
    }

    #[test]
    fn suggestions_include_keywords_and_catalog_models() {
        let mut state = ModelState::default();
        let (id, info) = plain_model("kimi-k2.6", "Kimi K2.6");
        state.available.insert(id, info);
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            screen_mode: crate::app::ScreenMode::Fullscreen,
        };
        let items = AdvisorCommand.suggest_args(&ctx, "").unwrap();
        assert!(items.iter().any(|i| i.match_text == "on"));
        assert!(items.iter().any(|i| i.match_text == "off"));
        assert!(items.iter().any(|i| i.match_text == "default"));
        assert!(items.iter().any(|i| i.match_text == "Kimi K2.6"));
    }
}
