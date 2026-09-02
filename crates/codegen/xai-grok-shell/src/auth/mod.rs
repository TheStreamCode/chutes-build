pub(crate) mod api_key_probe;
pub(crate) mod attribution;
mod auth_provider;
pub(crate) mod backend;
mod config;
pub mod credential_provider;
#[path = "devbox_login_stub.rs"]
pub(crate) mod devbox_login;
pub(crate) mod device_code;
pub mod error;
mod external_auth;
mod flow;
mod jwt;
pub(crate) mod manager;
mod model;
pub mod oidc;
pub(crate) mod recovery;
pub(crate) mod refresh;
pub(crate) mod single_flight;
mod storage;
mod token_output;
pub(crate) mod token_type;

/// Isolate the static API-key environment tier for a test.
///
/// Production reads `CHUTES_API_KEY` (legacy `CHUTES_BUILD_API_KEY`);
/// upstream's `CHUTES_API_KEY` name is no longer read but is cleared anyway so
/// an ambient developer shell cannot leak into assertions through any tier.
#[cfg(test)]
pub(crate) fn static_key_env_names() -> [&'static str; 3] {
    ["CHUTES_API_KEY", "CHUTES_BUILD_API_KEY", "CHUTES_API_KEY"]
}

/// Locate the `auth-provider-fixture` binary beside this test binary.
///
/// `CARGO_BIN_EXE_*` is set for integration tests and benches, not for a
/// lib's own unit tests, and `cargo test --lib` does not build binaries —
/// build it first (`cargo build -p xai-grok-shell --bin
/// auth-provider-fixture`); the gate runs that step before this suite.
#[cfg(test)]
pub(crate) fn provider_fixture_bin() -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let dir = exe.parent().expect("test exe directory");
    let name = if cfg!(windows) {
        "auth-provider-fixture.exe"
    } else {
        "auth-provider-fixture"
    };
    // A `cargo build --bin` artifact lands next to the profile root
    // (`target/debug`), while lib unit tests run from `target/debug/deps`;
    // accept either so both invocation orders work.
    let mut bin = dir.join(name);
    if !bin.exists() {
        bin = dir.parent().unwrap_or(dir).join(name);
    }
    assert!(
        bin.exists(),
        "auth-provider-fixture is missing under {}; build it first: \
         cargo build -p xai-grok-shell --bin auth-provider-fixture",
        dir.display()
    );
    bin
}

/// A shell-form provider command naming the fixture: bare words mean the
/// same under both `sh -c` and `cmd /C`, which quoting across the two does
/// not.
#[cfg(test)]
pub(crate) fn provider_fixture_command(args: &[&str]) -> String {
    let mut cmd = provider_fixture_bin().into_os_string();
    for arg in args {
        cmd.push(" ");
        cmd.push(arg);
    }
    cmd.to_string_lossy().into_owned()
}

/// A `std::process::Output` with a synthesized exit status, for tests that
/// shape provider output without spawning a program: `true` and `false` do
/// not exist on Windows, so the old fixture spawned them and the whole test
/// module panicked with "program not found" there.
#[cfg(test)]
pub(crate) fn fake_output(success: bool, stdout: &str) -> std::process::Output {
    let raw: std::os::raw::c_int = if success { 0 } else { 1 };
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(raw);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(raw as u32);
    std::process::Output {
        status,
        stdout: stdout.as_bytes().to_vec(),
        stderr: vec![],
    }
}

pub(crate) use api_key_probe::{
    DEFAULT_PROBE_TIMEOUT, first_party_env_key_allows_advertise, should_probe_first_party_env_key,
};
pub use auth_provider::{AuthProviderConfig, AuthProviderRef};
pub(crate) use auth_provider::{
    PROVIDER_TIMEOUT_CEILING_SECS, PROVIDER_TOKEN_EXPIRY_SKEW_SECS, ProviderRefreshOutcome,
};
#[cfg(test)]
pub(crate) use auth_provider::{test_backdate_provider_mint, test_counting_provider};
pub(crate) use config::LEGACY_AUTH_SCOPE;
pub use config::{
    ForceLoginTeam, GrokComConfig, OAuth2ProviderConfig, OidcAuthConfig, PreferredAuthMethod,
    XAI_OAUTH2_ISSUER, is_xai_oauth2_issuer, xai_oauth2_issuer,
};
pub(crate) use config::{
    force_login_team_from_env, force_login_team_from_requirements, resolve_force_login_team,
};
pub(crate) use external_auth::{parse_output, refresh_with_command};
pub(crate) use flow::{
    AuthChannels, mint_session_noninteractive, run_auth_flow, run_auth_flow_with_stderr_bridge,
    try_noninteractive_auth_no_mint,
};
pub use flow::{
    AuthUrlInfo, AuthUrlMode, LoginTransportOverride, LogoutResult, ensure_authenticated,
    ensure_authenticated_or_noninteractive, ensure_authenticated_with_override, perform_logout,
    run_cli_login, run_cli_logout, try_ensure_fresh_auth,
};
pub use jwt::{is_jwt_expired_or_near, parse_jwt_expiration};
mod meta;
pub use error::{AuthError, RefreshTokenError, RefreshTokenFailedReason};
pub use manager::{AuthManager, shared_api_key_provider};
pub(crate) use manager::{AuthRemedy, SilentRefresh};
pub use meta::{AuthMeta, GateInfo};
pub use model::{AuthMode, GrokAuth, lookup_auth};
pub(crate) use model::{TOKEN_TTL, UserInfo, default_coding_data_retention_opt_out, is_expired};
pub(crate) use refresh::DiagnosticUploader;
pub use storage::{clear_api_key, read_api_key, read_auth_json, store_api_key};
