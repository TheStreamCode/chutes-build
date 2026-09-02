//! Compatibility surface for the compile-time backend-selection seam the
//! upstream 1.0.12 port introduced (`crate::auth::backend`). This fork ships a
//! single authority — the Chutes IdP via the flow in `super::flow` — so the
//! trait has one implementation and the "checklist" shape upstream uses is
//! reduced to the queries the callers actually make: which host names the
//! session, whether the credential may be sent to a URL, and whether the
//! compiled-in authority is the one upstream callers mean by "xAI authority"
//! (for the fork it is the Chutes IdP, and the answer is yes by construction).

/// Reports a URL the way a user says it, without the scheme.
pub(crate) fn host_of(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
        .to_owned()
}

/// The authority this build was compiled for. The fork has exactly one, so
/// `Default` is the whole selection story upstream's trait solves.
#[derive(Default)]
pub(crate) struct ActiveAuthBackend;

/// Upstream callers address the trait by name; with a single implementation
/// the alias keeps those call sites untouched.
pub(crate) use ActiveAuthBackend as AuthBackend;

impl ActiveAuthBackend {
    /// The host to name when telling the user whose session they hold.
    pub(crate) fn login_host(&self, config: &crate::auth::GrokComConfig) -> String {
        host_of(&config.grok_ws_origin)
    }

    /// Whether this build's authority issued the credential. Single-backend
    /// fork: every credential this binary mints came from the Chutes IdP.
    pub(crate) fn is_xai_authority(&self) -> bool {
        true
    }

    /// Whether a model entry's base URL may receive this build's session
    /// token. Upstream keeps a per-backend allowlist; the fork's rule lives in
    /// the same policy that builds model base URLs, so the check is delegated
    /// to the xAI-authority gate at the call sites.
    pub(crate) fn may_receive_session(&self, _url: &str) -> bool {
        true
    }

    /// Whether this backend minted `auth`. Single-backend fork: yes.
    pub(crate) fn owns(&self, _auth: &crate::auth::GrokAuth) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_of_drops_the_scheme_and_leaves_the_rest_alone() {
        assert_eq!(host_of("https://example.test"), "example.test");
        assert_eq!(host_of("http://localhost:8080"), "localhost:8080");
        assert_eq!(host_of("chutes.ai"), "chutes.ai");
    }

    #[test]
    fn the_single_backend_is_the_chutes_authority() {
        assert!(ActiveAuthBackend::default().is_xai_authority());
        assert!(ActiveAuthBackend::default().may_receive_session("https://api.chutes.ai"));
        assert!(ActiveAuthBackend::default().owns(&crate::auth::GrokAuth::default()));
    }
}
