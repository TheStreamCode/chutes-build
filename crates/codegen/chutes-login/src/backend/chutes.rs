//! The Chutes Build backend: Chutes OAuth2, enterprise OIDC, the operator's auth binary, or a devbox.

use std::sync::Arc;

use super::{AuthBackend, LoginRequest};
use crate::refresh::{
    AuthSnapshot, DiagnosticUploader, ExternalBinaryRefresher, ExternalCommandRunner,
    OidcRefresher, TokenRefresher,
};
use crate::{AuthManager, ChutesAuth, LoginConfig};

#[derive(Default)]
pub struct ChutesAuthBackend;

#[async_trait::async_trait(?Send)]
impl AuthBackend for ChutesAuthBackend {
    fn scope_key(&self, config: &LoginConfig) -> String {
        config.auth_scope()
    }

    /// No inherited scopes: retired upstream scope keys are never adopted into this app's auth.
    fn inherited_scopes(&self) -> &'static [&'static str] {
        &[]
    }

    /// A Chutes login can come from OAuth2, a customer's own login provider, the auth binary, or a devbox, so there is no one issuer to check for.
    /// Saying yes to all of them is safe: a credential minted elsewhere still gets sent to Chutes, which rejects it.
    fn owns(&self, _auth: &ChutesAuth) -> bool {
        true
    }

    /// Some customers run their own gateway and sign in there with the session Chutes issued them, so a list of allowed hosts would lock them out.
    /// The models cache stops one backend's models from being used by another: it remembers the URL each entry came from and ignores the rest.
    fn may_receive_session(&self, _url: &str) -> bool {
        true
    }

    fn login_host(&self, config: &LoginConfig) -> String {
        super::host_of(&config.ws_origin)
    }

    fn is_chutes_authority(&self) -> bool {
        true
    }

    async fn login(&self, req: LoginRequest<'_>) -> anyhow::Result<(ChutesAuth, bool)> {
        crate::flow::run_auth_flow_steps(
            req.auth_manager,
            req.login_config,
            req.config_device_flow,
            req.reauth,
            req.force_interactive,
            req.on_stderr,
            req.url_tx,
            req.code_rx,
            req.login_override,
        )
        .await
    }

    fn refresher(
        &self,
        manager: Arc<AuthManager>,
        auth_provider_command: Option<String>,
        diagnostic_uploader: Option<DiagnosticUploader>,
    ) -> Arc<dyn TokenRefresher> {
        match auth_provider_command {
            Some(cmd) => {
                let snapshot: Arc<dyn AuthSnapshot> = manager.clone();
                let runner: Arc<dyn ExternalCommandRunner> = manager;
                Arc::new(ExternalBinaryRefresher::new(runner, snapshot, cmd))
            }
            None => {
                let snapshot: Arc<dyn AuthSnapshot> = manager;
                let refresher = OidcRefresher::new(snapshot);
                match diagnostic_uploader {
                    Some(uploader) => Arc::new(refresher.with_diagnostic_upload(uploader)),
                    None => Arc::new(refresher),
                }
            }
        }
    }
}
