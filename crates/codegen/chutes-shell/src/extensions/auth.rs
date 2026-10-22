//! `chutes.ai/auth/*` and legacy `chutes.ai/{get,set}ApiKey` extension handlers.
//!
//! These methods let the client read/write the API key via the agent and drive the OAuth login flow.
//! The agent is the single source of truth for `auth.json`.

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};

use super::{ExtResult, parse_params, to_raw_response};
use crate::agent::MvpAgent;
use crate::session::ExtMethodResult;

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "chutes.ai/auth/getBearerToken" => handle_get_bearer_token(agent).await,
        "chutes.ai/getApiKey" => handle_get_api_key(),
        "chutes.ai/setApiKey" => handle_set_api_key(args),
        "chutes.ai/auth/submit_code" => handle_submit_code(agent, args),
        "chutes.ai/auth/get_url" => handle_get_url(agent).await,
        "chutes.ai/auth/cancel" => handle_cancel(agent, args),
        "chutes.ai/auth/logout" => handle_logout(agent, args).await,
        "chutes.ai/auth/info" => handle_info(agent),
        "chutes.ai/auth/check_subscription" => handle_check_subscription(agent).await,
        "chutes.ai/auth/hydrate_team_capability" => {
            handle_hydrate_team_capability(agent, args).await
        }
        _ => Err(acp::Error::method_not_found()),
    }
}

/// `pub` with both serde directions so the pager builds the request from the type the agent parses.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HydrateTeamCapabilityRequest {
    pub email: Option<String>,
    pub team_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HydrateTeamCapabilityResponse {
    /// Required but nullable: the handler always emits it, so an absent or misspelled key is a broken peer, not an unknown answer.
    #[serde(deserialize_with = "Option::deserialize")]
    pub can_administer_team: Option<bool>,
}

/// Backfills `canAdministerTeam` for a cached credential that predates it: startup only enriches a missing user id and the token fast path skips `/user`.
/// The ACP layer clones the agent's `Rc` into this request's task, so a dropped connection does not cancel the GET; nobody reads the answer, but the shared cache still fills.
async fn handle_hydrate_team_capability(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let params: HydrateTeamCapabilityRequest = parse_params(args)?;
    let can_administer_team = agent
        .auth_manager
        .hydrate_can_administer_team(params.email.as_deref(), params.team_id.as_deref())
        .await;
    to_raw_response(&HydrateTeamCapabilityResponse {
        can_administer_team,
    })
}

/// Stop an in-flight interactive login (device poll or loopback wait).
/// Calling it when nothing is waiting does nothing.
/// When `request_seq` is present, only that attempt is cancelled, so a delayed cancel cannot cancel a newer login that already replaced it.
fn handle_cancel(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    struct CancelParams {
        #[serde(default)]
        request_seq: Option<u64>,
    }
    let params: CancelParams =
        serde_json::from_str(args.params.get()).unwrap_or(CancelParams { request_seq: None });
    match params.request_seq {
        Some(seq) => agent.interactive_auth.cancel_for_client_seq(seq),
        None => agent.interactive_auth.cancel(),
    }
    to_raw_response(&serde_json::json!({ "cancelled": true }))
}

async fn handle_get_bearer_token(agent: &MvpAgent) -> ExtResult {
    // Fail closed for session tokens: desktop resume treats non-null as success. Never return a hard-expired access token
    // Still return wire-valid session tokens and static user-supplied keys (process model key, env, or disk api_key) That keeps non-session sessions working when AuthManager has no OIDC entry
    let token = match agent.auth_manager.get_valid_token().await {
        Ok(token) => Some(token),
        Err(_) => agent
            .auth_manager
            .current_wire_valid()
            .map(|a| a.key)
            .or_else(|| agent.auth_manager.static_api_key_for_export()),
    };
    ExtMethodResult::success(serde_json::json!({ "token": token }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

fn handle_get_api_key() -> ExtResult {
    let key = crate::agent::auth_method::read_chutes_api_key_env().ok();
    ExtMethodResult::success(serde_json::json!({ "key": key }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

fn handle_set_api_key(args: &acp::ExtRequest) -> ExtResult {
    let params: serde_json::Value = parse_params(args)?;
    let key = params.get("key").and_then(|v| v.as_str());
    let chutes_home = crate::util::chutes_home::chutes_home();
    // A build without account login keeps no login store (writes are refused
    // by design): the client persists to its own key store first, so here the
    // key goes memory-only.
    let keeps_login_store =
        chutes_config::Distribution::current().allows(chutes_config::Capability::AccountLogin);
    match key {
        Some(k) if !k.is_empty() => {
            if keeps_login_store {
                chutes_login::store_api_key(&chutes_home, k)
                    .map_err(|e| acp::Error::internal_error().data(e.to_string()))?;
            }
            chutes_login::auth_method::set_runtime_chutes_api_key(k);
        }
        _ => {
            if keeps_login_store {
                chutes_login::clear_api_key(&chutes_home)
                    .map_err(|e| acp::Error::internal_error().data(e.to_string()))?;
            }
            chutes_login::auth_method::clear_runtime_chutes_api_key();
        }
    }
    ExtMethodResult::success(serde_json::json!({ "ok": true }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

/// Handles an auth code submitted from the TUI.
fn handle_submit_code(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    struct SubmitCodeParams {
        code: String,
    }

    let params: SubmitCodeParams = serde_json::from_str(args.params.get())
        .map_err(|e| acp::Error::invalid_params().data(format!("invalid params: {e}")))?;

    match agent.interactive_auth.submit_code(params.code) {
        Ok(()) => to_raw_response(&serde_json::json!({ "submitted": true })),
        Err(chutes_login::single_flight::SubmitCodeError::SendFailed(e)) => {
            Err(acp::Error::internal_error().data(format!("failed to submit auth code: {e}")))
        }
        Err(chutes_login::single_flight::SubmitCodeError::NoPendingAttempt) => {
            Err(acp::Error::invalid_params().data("no pending auth session"))
        }
    }
}

/// Awaits the auth URL from the oneshot channel (blocks until ready).
async fn handle_get_url(agent: &MvpAgent) -> ExtResult {
    let rx = agent.interactive_auth.take_url_rx();
    // `None` when no URL was sent (cached credentials, early error, second poll): report mode as `null` rather than mislabeling it `loopback`
    let (auth_url, mode) = match rx {
        Some(rx) => match rx.await {
            Ok(info) => (Some(info.url), Some(info.mode)),
            Err(_) => (None, None),
        },
        None => (None, None),
    };
    to_raw_response(&serde_json::json!({
        "auth_url": auth_url,
        // `external_provider` kept for older clients; `mode` is authoritative.
        "external_provider": mode.is_some_and(|m| m.is_external_provider()),
        "mode": mode.map(|m| m.as_wire_str()),
    }))
}

async fn handle_logout(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    struct LogoutParams {
        scope: Option<String>,
    }

    let params: LogoutParams = serde_json::from_str(args.params.get())
        .map_err(|e| acp::Error::invalid_params().data(format!("invalid params: {e}")))?;

    // Stop any in-flight login so it cannot write credentials back after logout.
    agent.interactive_auth.cancel();

    let result = chutes_login::perform_logout(
        &agent.auth_manager,
        params.scope.as_deref(),
        chutes_cloud_config::managed_config::clear_orphan,
    )
    .map_err(|e| acp::Error::internal_error().data(format!("failed to logout: {e}")))?;
    // `auth.lifecycle` (not `auth`) avoids colliding with the pre-existing per-request `AuthManager::auth()` `#[instrument]` span
    chutes_telemetry::event_span!("auth.lifecycle", action = "logout", success = true);

    agent.models_manager.on_auth_changed().await;

    to_raw_response(&serde_json::json!({
        "ok": true,
        "was_logged_in": result.was_logged_in,
        "email": result.email,
        "api_key_still_set": result.api_key_still_set,
    }))
}

/// Re-checks the subscription once, for the retry button on the paywall screen.
/// Returns the updated auth response with gate info so the pager can refresh the gate state.
async fn handle_check_subscription(agent: &MvpAgent) -> ExtResult {
    agent.retry_subscription_check().await;
    let response = agent.auth_response_with_meta();
    to_raw_response(&serde_json::json!({
        "authenticated": response.meta.is_some(),
        "meta": response.meta,
    }))
}

/// Returns current auth method ID, user profile fields, and team/principal metadata.
fn handle_info(agent: &MvpAgent) -> ExtResult {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AuthInfoResponse {
        method_id: Option<String>,
        email: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        /// `chutes-asset://` URL resolved by the Electron protocol handler, or a full `http(s)://` URL passed through unchanged.
        profile_image_url: Option<String>,
        team_id: Option<String>,
        team_name: Option<String>,
        team_role: Option<String>,
        organization_id: Option<String>,
        organization_name: Option<String>,
        organization_role: Option<String>,
        principal_type: Option<String>,
        principal_id: Option<String>,
        user_blocked_reason: Option<String>,
        team_blocked_reasons: Vec<String>,
        coding_data_retention_opt_out: bool,
    }

    let method_id = agent
        .auth_method_id
        .load()
        .as_ref()
        .map(|m| m.0.to_string());
    let auth = agent.auth_manager.current_or_expired();
    let raw_asset_id = auth.as_ref().and_then(|a| a.profile_image_asset_id.clone());

    // Return a chutes-asset:// URL that the Electron renderer resolves at display time via a custom protocol handler
    // The handler proxies through cli-chat-proxy's /asset endpoint; Electron's HTTP cache handles reuse
    // Nothing here touches a disk cache or the network
    let profile_image_url = match raw_asset_id.as_deref().filter(|k| !k.is_empty()) {
        Some(key) if key.starts_with("http://") || key.starts_with("https://") => {
            Some(key.to_owned())
        }
        Some(key) => Some(format!("chutes-asset:///{key}")),
        None => None,
    };
    to_raw_response(&AuthInfoResponse {
        method_id,
        email: auth.as_ref().and_then(|a| a.email.clone()),
        first_name: auth.as_ref().and_then(|a| a.first_name.clone()),
        last_name: auth.as_ref().and_then(|a| a.last_name.clone()),
        profile_image_url,
        team_id: auth.as_ref().and_then(|a| a.team_id.clone()),
        team_name: auth.as_ref().and_then(|a| a.team_name.clone()),
        team_role: auth.as_ref().and_then(|a| a.team_role.clone()),
        organization_id: auth.as_ref().and_then(|a| a.organization_id.clone()),
        organization_name: auth.as_ref().and_then(|a| a.organization_name.clone()),
        organization_role: auth.as_ref().and_then(|a| a.organization_role.clone()),
        principal_type: auth.as_ref().and_then(|a| a.principal_type.clone()),
        principal_id: auth.as_ref().and_then(|a| a.principal_id.clone()),
        user_blocked_reason: auth.as_ref().and_then(|a| a.user_blocked_reason.clone()),
        team_blocked_reasons: auth
            .as_ref()
            .map(|a| a.team_blocked_reasons.clone())
            .unwrap_or_default(),
        // With no credential the privacy state is unknown, so report opted-out (fail closed)
        // This matches `AuthManager::allows_data_collection` and the ChutesAuth Default
        coding_data_retention_opt_out: auth
            .as_ref()
            .map(|a| a.coding_data_retention_opt_out)
            .unwrap_or_else(chutes_login::default_coding_data_retention_opt_out),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_key_request(key: Option<&str>) -> acp::ExtRequest {
        let params = serde_json::json!({ "key": key });
        acp::ExtRequest::new(
            "chutes.ai/setApiKey",
            serde_json::value::to_raw_value(&params)
                .expect("serialize setApiKey params")
                .into(),
        )
    }

    /// Without account login there is no login store: the key must land in
    /// the runtime override (visible to sampling resolution) and nowhere on
    /// disk. The client persists to its own key store first.
    #[serial_test::serial(RUNTIME_API_KEY)]
    #[test]
    fn set_api_key_is_memory_only_without_account_login() {
        assert!(
            !chutes_config::Distribution::current().allows(chutes_config::Capability::AccountLogin)
        );
        let home = tempfile::tempdir().expect("temp home");
        let _home_guard = chutes_test_support::env::EnvGuard::set("CHUTES_BUILD_HOME", home.path());
        let prior_key = std::env::var_os("CHUTES_API_KEY");
        let prior_legacy = std::env::var_os("CHUTES_BUILD_API_KEY");
        // SAFETY: serialized via `serial_test`; restored below.
        unsafe {
            std::env::remove_var("CHUTES_API_KEY");
            std::env::remove_var("CHUTES_BUILD_API_KEY");
        }
        chutes_login::auth_method::clear_runtime_chutes_api_key();

        handle_set_api_key(&set_key_request(Some("cpk-test-key"))).expect("set succeeds");
        assert_eq!(
            crate::agent::auth_method::read_chutes_api_key_env().expect("runtime key readable"),
            "cpk-test-key"
        );
        assert!(
            !home.path().join("auth.json").exists(),
            "no login store may be written"
        );

        handle_set_api_key(&set_key_request(None)).expect("clear succeeds");
        assert!(crate::agent::auth_method::read_chutes_api_key_env().is_err());

        // SAFETY: serialized via `serial_test`; restore the env for other tests.
        unsafe {
            match prior_key {
                Some(value) => std::env::set_var("CHUTES_API_KEY", value),
                None => std::env::remove_var("CHUTES_API_KEY"),
            }
            match prior_legacy {
                Some(value) => std::env::set_var("CHUTES_BUILD_API_KEY", value),
                None => std::env::remove_var("CHUTES_BUILD_API_KEY"),
            }
        }
    }
}
