//! `chutes.ai/privacy/setCodingDataRetention` extension handler.
//!
//! Local-only: records the opt-out flag in local auth state. Chutes serves
//! no retention API, so no PUT is attempted and the toggle never fails for
//! want of a backend.

use agent_client_protocol as acp;
use serde::Deserialize;

use super::{ExtResult, parse_params, to_raw_response};
use crate::agent::MvpAgent;

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "chutes.ai/privacy/setCodingDataRetention" => handle_set(agent, args).await,
        _ => Err(acp::Error::method_not_found()),
    }
}

async fn handle_set(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Params {
        coding_data_retention_opt_out: bool,
    }

    let params: Params = parse_params(args)?;

    let auth = agent.auth_manager.auth().await.map_err(|e| {
        tracing::warn!(error = %e, "privacy: auth resolution failed");
        acp::Error::auth_required()
            .data("Authentication required. Set CHUTES_API_KEY to your Chutes API key.")
    })?;

    // Local-only: Chutes serves no coding-data-retention API, so there is
    // no server flag to PUT. The opt-out lives in local auth state, where
    // the pager already reads it (default opted-out), and gates local
    // behavior. Never fail the toggle for want of a backend.
    // Update local auth state to reflect the change.
    // Use save_without_enrichment to avoid a race
    // update() spawns a background GET /user enrichment that may read stale ACL state and overwrite the opt-out flag back to its previous value
    let mut updated = auth.clone();
    updated.coding_data_retention_opt_out = params.coding_data_retention_opt_out;
    let _ = agent.auth_manager.save_without_enrichment(updated).await;

    to_raw_response(&serde_json::json!({
        "codingDataRetentionOptOut": params.coding_data_retention_opt_out,
    }))
}
