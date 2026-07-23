//! `chutes-build models` subcommand.

use anyhow::Result;
use tokio_util::sync::CancellationToken;
use xai_grok_shell::agent::config::Config as AgentConfig;
use xai_grok_shell::cli_models::{AuthStatus, list_models};

use crate::client_identity::{PAGER_CLIENT_TYPE, PAGER_CLIENT_VERSION};

pub async fn list_available_models(agent_config: &AgentConfig, json: bool) -> Result<()> {
    let auth_status = AuthStatus::resolve(agent_config);
    if !json {
        match &auth_status {
            AuthStatus::ApiKey => println!("You are using CHUTES_API_KEY."),
            AuthStatus::LoggedIn(host) => println!("You are logged in with {host}."),
            AuthStatus::ModelCredentials(model) => {
                println!("Model '{model}' is using its own API key.");
            }
            AuthStatus::DeploymentKey => println!("You are authenticated via deployment key."),
            AuthStatus::NotAuthenticated => println!("You are not authenticated."),
        }
        println!();
    }

    let cancel = CancellationToken::new();
    let spawned = crate::acp::spawn::spawn_grok_shell(agent_config.clone(), &cancel, None).await?;

    let state_result =
        list_models(&spawned.channel.tx, PAGER_CLIENT_TYPE, PAGER_CLIENT_VERSION).await;
    cancel.cancel();
    let state = state_result?;

    if json {
        let authentication = match auth_status {
            AuthStatus::ApiKey => serde_json::json!({"method": "api_key_env"}),
            AuthStatus::LoggedIn(host) => {
                serde_json::json!({"method": "session", "host": host})
            }
            AuthStatus::ModelCredentials(model) => {
                serde_json::json!({"method": "model_credentials", "model": model})
            }
            AuthStatus::DeploymentKey => serde_json::json!({"method": "deployment_key"}),
            AuthStatus::NotAuthenticated => serde_json::json!({"method": "none"}),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "authentication": authentication,
                "default_model": state.current_model_id,
                "models": state.available_models,
            }))?
        );
    } else {
        println!("Default model: {}", state.current_model_id.0);
        println!();
        println!("Available models:");
        for m in state.available_models {
            if m.model_id == state.current_model_id {
                println!("  * {} (default)", m.model_id.0);
            } else {
                println!("  - {}", m.model_id.0);
            }
        }
    }

    Ok(())
}
