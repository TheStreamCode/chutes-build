use std::sync::Arc;

use base64::Engine as _;
use chrono::{Duration, Utc};
use chutes_test_support::EnvGuard;
use chutes_tools::types::api_key_provider::SideCallBearerError;
use pretty_assertions::assert_eq;

use super::is_chutes_side_call_principal;
use crate::config::{CHUTES_OAUTH2_ISSUER, LoginConfig};
use crate::{AuthManager, AuthMode, ChutesAuth};

const FOREIGN_ISSUER: &str = "https://cursor.com";

fn unsigned_jwt(header: &str, payload: &str) -> String {
    let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    format!("{}.{}.sig", enc.encode(header), enc.encode(payload))
}

fn foreign_access_token() -> String {
    unsigned_jwt(
        r#"{"alg":"RS256","typ":"at+jwt"}"#,
        &format!(r#"{{"iss":"{FOREIGN_ISSUER}","sub":"user-1"}}"#),
    )
}

fn session(key: &str, mode: AuthMode, issuer: Option<&str>) -> ChutesAuth {
    ChutesAuth {
        key: key.to_owned(),
        auth_mode: mode,
        oidc_issuer: issuer.map(str::to_owned),
        refresh_token: Some("rt".to_owned()),
        expires_at: Some(Utc::now() + Duration::hours(1)),
        ..ChutesAuth::test_default()
    }
}

fn api_key(key: &str) -> ChutesAuth {
    ChutesAuth {
        key: key.to_owned(),
        auth_mode: AuthMode::ApiKey,
        oidc_issuer: None,
        expires_at: None,
        ..ChutesAuth::test_default()
    }
}

/// The process env may carry a developer's key; every test pins it out so the credential alone decides.
fn static_key_guards() -> [EnvGuard; 3] {
    [
        EnvGuard::unset("CHUTES_API_KEY"),
        EnvGuard::unset("CHUTES_BUILD_API_KEY"),
        EnvGuard::unset("CHUTES_BUILD_AUTH_PATH"),
    ]
}

fn manager(dir: &tempfile::TempDir, auth: Option<ChutesAuth>) -> Arc<AuthManager> {
    let mgr = Arc::new(AuthManager::new(dir.path(), LoginConfig::default()));
    if let Some(auth) = auth {
        mgr.hot_swap(auth);
    }
    mgr
}

/// Only a foreign-issuer credential is refused. Everything chat sends today keeps going to the
/// server, which decides: an enterprise IdP session, a bare external-provider token, a legacy web login.
#[test]
fn only_a_foreign_credential_is_refused() {
    assert!(is_chutes_side_call_principal(&api_key("chutes-plain-key")));
    assert!(is_chutes_side_call_principal(&session(
        "opaque",
        AuthMode::Oidc,
        Some(CHUTES_OAUTH2_ISSUER)
    )));
    assert!(is_chutes_side_call_principal(&session(
        "opaque",
        AuthMode::Oidc,
        Some("https://idp.acme.example")
    )));
    assert!(is_chutes_side_call_principal(&session(
        "bare-provider-token",
        AuthMode::External,
        None
    )));
    assert!(is_chutes_side_call_principal(&session(
        "legacy",
        AuthMode::WebLogin,
        None
    )));

    assert!(!is_chutes_side_call_principal(&session(
        &foreign_access_token(),
        AuthMode::Oidc,
        Some(FOREIGN_ISSUER)
    )));
    assert!(!is_chutes_side_call_principal(&session(
        &foreign_access_token(),
        AuthMode::Oidc,
        Some("https://api.cursor.com")
    )));
    assert!(is_chutes_side_call_principal(&session(
        "opaque",
        AuthMode::Oidc,
        Some("https://cursor.com.evil.test")
    )));
}

#[test]
#[serial_test::serial]
fn foreign_session_is_an_error_not_a_bearer() {
    let _guards = static_key_guards();
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(
        &dir,
        Some(session(
            &foreign_access_token(),
            AuthMode::Oidc,
            Some(FOREIGN_ISSUER),
        )),
    );

    assert_eq!(
        Err(SideCallBearerError::ForeignSession),
        mgr.side_call_bearer()
    );
}

#[tokio::test]
#[serial_test::serial]
async fn foreign_session_is_an_error_after_the_refresh_path_too() {
    let _guards = static_key_guards();
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(
        &dir,
        Some(session(
            &foreign_access_token(),
            AuthMode::Oidc,
            Some(FOREIGN_ISSUER),
        )),
    );

    assert_eq!(
        Err(SideCallBearerError::ForeignSession),
        mgr.side_call_bearer_async().await
    );
}

#[test]
#[serial_test::serial]
fn no_credential_is_missing_not_a_fallback() {
    let _guards = static_key_guards();
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(&dir, None);

    assert_eq!(Err(SideCallBearerError::Missing), mgr.side_call_bearer());
}

#[test]
#[serial_test::serial]
fn static_key_still_serves_a_foreign_login() {
    let _guards = static_key_guards();
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(
        &dir,
        Some(session(
            &foreign_access_token(),
            AuthMode::Oidc,
            Some(FOREIGN_ISSUER),
        )),
    );
    mgr.set_process_static_api_key(Some("chutes-static-key".to_owned()));

    assert_eq!(Ok("chutes-static-key".to_owned()), mgr.side_call_bearer());
}

#[test]
#[serial_test::serial]
fn kill_switch_blocks_the_static_key_for_side_calls_too() {
    let _guards = static_key_guards();
    let dir = tempfile::tempdir().unwrap();
    let mgr = Arc::new(AuthManager::new(
        dir.path(),
        LoginConfig {
            disable_api_key_auth: Some(true),
            ..LoginConfig::default()
        },
    ));
    mgr.hot_swap(session(
        &foreign_access_token(),
        AuthMode::Oidc,
        Some(FOREIGN_ISSUER),
    ));
    mgr.set_process_static_api_key(Some("chutes-static-key".to_owned()));

    assert_eq!(
        Err(SideCallBearerError::ForeignSession),
        mgr.side_call_bearer()
    );
}

#[test]
#[serial_test::serial]
fn chutes_principals_are_returned() {
    let _guards = static_key_guards();
    let dir = tempfile::tempdir().unwrap();

    let mgr = manager(&dir, Some(api_key("chutes-plain-key")));
    assert_eq!(Ok("chutes-plain-key".to_owned()), mgr.side_call_bearer());

    mgr.hot_swap(session(
        "chutes-session",
        AuthMode::Oidc,
        Some(CHUTES_OAUTH2_ISSUER),
    ));
    assert_eq!(Ok("chutes-session".to_owned()), mgr.side_call_bearer());
}

#[test]
#[serial_test::serial]
fn an_expired_chutes_login_is_missing_in_the_sync_path() {
    let _guards = static_key_guards();
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(
        &dir,
        Some(ChutesAuth {
            expires_at: Some(Utc::now() - Duration::hours(1)),
            ..session("chutes-session", AuthMode::Oidc, Some(CHUTES_OAUTH2_ISSUER))
        }),
    );

    assert_eq!(Err(SideCallBearerError::Missing), mgr.side_call_bearer());
}
