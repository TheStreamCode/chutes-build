//! `chutes-build models` subcommand.

use anyhow::Result;
use chutes_shell::agent::config::Config as AgentConfig;
use chutes_shell::cli_models::{AuthStatus, list_models};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::client_identity::{PAGER_CLIENT_TYPE, PAGER_CLIENT_VERSION};

/// The documented `models --json` shape. Kept separate from the human rendering so the
/// contract is pinned by a unit test that does not need to spawn the agent.
fn models_report(auth: &AuthStatus, default_model: &str, model_ids: &[String]) -> Value {
    let (kind, detail) = match auth {
        AuthStatus::ApiKey => ("api_key", None),
        AuthStatus::LoggedIn(host) => ("logged_in", Some(host.clone())),
        AuthStatus::ModelCredentials(model) => ("model_credentials", Some(model.clone())),
        AuthStatus::DeploymentKey => ("deployment_key", None),
        AuthStatus::NotAuthenticated => ("none", None),
    };
    let models: Vec<Value> = model_ids
        .iter()
        .map(|id| json!({ "id": id, "default": id == default_model }))
        .collect();
    json!({
        "authenticated": !matches!(auth, AuthStatus::NotAuthenticated),
        "auth": kind,
        "authDetail": detail,
        "defaultModel": default_model,
        "models": models,
    })
}

pub async fn list_available_models(agent_config: &AgentConfig, json_output: bool) -> Result<()> {
    let auth = AuthStatus::resolve(agent_config);
    if !json_output {
        match &auth {
            AuthStatus::ApiKey => println!("You are using CHUTES_API_KEY."),
            AuthStatus::LoggedIn(host) => println!("You are logged in with {host}."),
            AuthStatus::ModelCredentials(model) => {
                println!("Model '{model}' is using its own API key.");
            }
            AuthStatus::DeploymentKey => println!("You are authenticated via deployment key."),
            AuthStatus::NotAuthenticated => println!(
                "You are not authenticated. Only the built-in default route is listed; \
                 set CHUTES_API_KEY to resolve the full catalog."
            ),
        }
        println!();
    }

    let cancel = CancellationToken::new();
    chutes_telemetry::startup::mark_utility_process();
    let spawned =
        crate::acp::spawn::spawn_chutes_shell(agent_config.clone(), &cancel, None).await?;
    // Cancel and join on every return path, including the `?` below
    let _agent_guard =
        crate::acp::spawn::AgentShutdownGuard::new(cancel.clone(), Some(spawned.thread_handle));

    let state = list_models(&spawned.channel.tx, PAGER_CLIENT_TYPE, PAGER_CLIENT_VERSION).await?;
    let model_ids: Vec<String> = state
        .available_models
        .iter()
        .map(|m| m.model_id.0.to_string())
        .collect();
    let default_model = state.current_model_id.0.to_string();

    if json_output {
        let report = models_report(&auth, &default_model, &model_ids);
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("Default model: {default_model}");
    println!();
    println!("Available models:");
    for id in &model_ids {
        if *id == default_model {
            println!("  * {id} (default)");
        } else {
            println!("  - {id}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_report_pins_the_documented_shape() {
        let ids = vec![
            "model-router".to_owned(),
            "moonshotai/Kimi-K2.6-TEE".to_owned(),
        ];
        let report = models_report(
            &AuthStatus::ModelCredentials("byok-model".to_owned()),
            "model-router",
            &ids,
        );
        assert_eq!(report["authenticated"], json!(true));
        assert_eq!(report["auth"], json!("model_credentials"));
        assert_eq!(report["authDetail"], json!("byok-model"));
        assert_eq!(report["defaultModel"], json!("model-router"));
        assert_eq!(
            report["models"],
            json!([
                { "id": "model-router", "default": true },
                { "id": "moonshotai/Kimi-K2.6-TEE", "default": false },
            ])
        );
    }

    #[test]
    fn json_report_is_honest_without_credentials() {
        let report = models_report(
            &AuthStatus::NotAuthenticated,
            "model-router",
            &["model-router".to_owned()],
        );
        assert_eq!(report["authenticated"], json!(false));
        assert_eq!(report["auth"], json!("none"));
        assert_eq!(report["authDetail"], Value::Null);
        assert_eq!(
            report["models"],
            json!([{ "id": "model-router", "default": true }])
        );
    }
}
