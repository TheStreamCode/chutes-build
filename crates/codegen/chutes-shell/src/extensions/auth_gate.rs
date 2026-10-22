use agent_client_protocol as acp;

use chutes_login::{AuthManager, ChutesAuth};

/// Require Chutes auth from a sync context: with no `.await` to refresh, a token inside the client's early-invalidation buffer still counts.
pub(crate) fn require_chutes_auth(
    auth_manager: &AuthManager,
    missing_message: &'static str,
    non_chutes_message: &'static str,
) -> Result<ChutesAuth, acp::Error> {
    let auth = auth_manager
        .current_or_expired()
        .ok_or_else(|| acp::Error::auth_required().data(missing_message))?;
    if !auth.is_chutes_auth() {
        return Err(acp::Error::auth_required().data(non_chutes_message));
    }
    Ok(auth)
}
