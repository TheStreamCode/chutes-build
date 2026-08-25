//! Automatic model switch on plan-mode transitions.
//!
//! Opt-in via `[models] plan_model` / `build_model`: entering plan mode
//! applies `plan_model` (a stronger model for planning), leaving it applies
//! `build_model` (the cheaper implementation model). Each direction fires
//! only when its key is set, so either can be used alone. A misconfigured
//! id logs a warning and leaves the session's model alone — the mode change
//! itself always succeeds.
//!
//! Hooked where client-driven mode changes converge (`set_session_mode` and
//! the `toggle_plan_mode` ext method). Model-driven exits — approving an
//! `exit_plan_mode` proposal — do not pass through here and keep the current
//! model; switch back with Shift+Tab or `/model`.
use super::model_switch;
use crate::agent::mvp_agent::MvpAgent;
use agent_client_protocol::{self as acp};

/// Apply the configured mode-model for a plan-mode transition.
///
/// `entering_plan` selects `plan_model`; `false` selects `build_model`.
/// No-op when the target key is unset, unresolvable, or already active.
pub(crate) async fn apply_for_mode_transition(
    agent: &MvpAgent,
    session_id: &acp::SessionId,
    entering_plan: bool,
) {
    let direction = if entering_plan {
        "plan_model"
    } else {
        "build_model"
    };
    let target = {
        let cfg = agent.cfg.borrow();
        let configured = if entering_plan {
            cfg.models.plan_model.as_deref()
        } else {
            cfg.models.build_model.as_deref()
        };
        match configured {
            Some(id) => id.to_owned(),
            None => return,
        }
    };
    let model_id = acp::ModelId::new(target);
    let model = match agent.resolve_model_id(&model_id) {
        Ok(model) => model,
        Err(error) => {
            tracing::warn!(
                session_id = %session_id.0,
                config_key = direction,
                model_id = %model_id.0,
                error = ?error,
                "mode model switch: configured model not resolvable; keeping current model"
            );
            return;
        }
    };
    let already_active = agent
        .resident_handle(session_id)
        .is_some_and(|handle| handle.model_id == model_id);
    if already_active {
        return;
    }
    tracing::info!(
        session_id = %session_id.0,
        config_key = direction,
        model_id = %model_id.0,
        "mode model switch: applying configured mode model"
    );
    let request = acp::SetSessionModelRequest::new(session_id.clone(), model_id.clone());
    if let Err(error) = model_switch::apply(agent, request, None).await {
        tracing::warn!(
            session_id = %session_id.0,
            config_key = direction,
            model_id = %model_id.0,
            error = ?error,
            "mode model switch: apply failed; keeping previous model"
        );
    }
}
