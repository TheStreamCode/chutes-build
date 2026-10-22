//! Bridge the shell's `AuthManager` onto the voice crate's STT routes.
//!
//! voice-api accepts a Chutes API key or a Chutes OAuth2 token at `api.chutes.ai` and attributes per-user billing for OAuth.
//! The bearer comes from the shell's side-call resolver, the same one the Imagine tools use, so a login issued by a
//! foreign authority is refused here and no socket opens for it. A build that can transcribe a finished clip through
//! that login's own backend offers it as the second route, so such a login records and transcribes instead of failing.
//!
//! Resolved per request: the agent's refreshing manager in direct-spawn mode.
//! In leader mode, a non-refreshing one adopts the agent's rotated `auth.json` token under the file lock (see [`crate::acp`]).
use chutes_tools::types::SharedApiKeyProvider;
use chutes_tools::types::api_key_provider::SideCallBearerError;
use chutes_voice::{SharedVoiceAuth, SttRoutes, VoiceAuthError, VoiceAuthProvider};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
/// Adapts the shell's `ApiKeyProvider` onto [`VoiceAuthProvider`].
///
/// Resolves a token per request (never a static snapshot), so a long session follows the `AuthManager` instead of pinning a token that 401s.
struct AuthManagerVoiceAuth(SharedApiKeyProvider);
impl std::fmt::Debug for AuthManagerVoiceAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthManagerVoiceAuth")
    }
}
fn voice_auth_error(error: SideCallBearerError) -> VoiceAuthError {
    match error {
        SideCallBearerError::ForeignSession => VoiceAuthError::ForeignSession,
        SideCallBearerError::Missing => VoiceAuthError::NotSignedIn,
    }
}
impl VoiceAuthProvider for AuthManagerVoiceAuth {
    fn bearer(&self) -> Pin<Box<dyn Future<Output = Result<String, VoiceAuthError>> + Send + '_>> {
        let provider = self.0.clone();
        Box::pin(async move { provider.side_call_bearer().await.map_err(voice_auth_error) })
    }
}
/// Build the voice bearer provider from the connection's `AuthManager`.
///
/// Serves Chutes logins and `CHUTES_API_KEY` / per-model BYOK keys. A foreign-issuer login resolves to
/// [`VoiceAuthError::ForeignSession`] instead of a bearer.
fn build_voice_auth(auth_manager: Arc<chutes_login::AuthManager>) -> SharedVoiceAuth {
    Arc::new(AuthManagerVoiceAuth(chutes_login::shared_api_key_provider(
        auth_manager,
    )))
}
/// Streaming bearer, plus the clip transcriber on builds that can transcribe for the signed-in login. The pipeline
/// consults it only after [`VoiceAuthError::ForeignSession`], so a Chutes credential always keeps streaming.
pub fn build_stt_routes(auth_manager: Arc<chutes_login::AuthManager>) -> SttRoutes {
    let clip_transcriber = None;
    SttRoutes {
        auth: build_voice_auth(auth_manager),
        clip_transcriber,
    }
}
#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
