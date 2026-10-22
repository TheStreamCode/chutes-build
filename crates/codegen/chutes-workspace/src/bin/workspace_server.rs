//! Standalone workspace ToolServer for remote sandboxes.
//!
//! Reads OIDC credentials from `~/.chutes-build/auth.json`, connects to a
//! server, exposes workspace tools, and refreshes tokens automatically.
#![deny(clippy::indexing_slicing)]
use chutes_diag_server::{self as diag_server, DiagHandle, ErrorClass};
use chutes_workspace::WorkspaceHostKind;
use chutes_workspace::config::{
    WorkspaceServerMetadata, merge_host_identity_metadata, merge_session_metadata,
};
use chutes_workspace::error::WorkspaceError;
use chutes_workspace_daemon::daemonize;
use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;
/// OTLP `service.name` for this binary's exported traces/logs/metrics and direct-OTLP fastrace export.
/// Single source so the call sites can't drift.
const SERVICE_NAME: &str = "prod_chutes_workspace";
const EXIT_SERVER_ID_INVALID: i32 = 3;
const INVALID_SERVER_ID_MARKER: &str = "workspace-server: invalid --server-id";
const WORKSPACE_HUB_AUTH_FAILED_MARKER: &str = "workspace hub auth failed";
/// Post-failure dwell so the host can poll `/ready` before exit ([500ms, 2s]).
const HUB_CONNECT_FAILED_DWELL: Duration = Duration::from_millis(750);
/// Capped log of pre-stop exec handler output, in the server's temp dir next to the daemon log.
#[cfg(unix)]
const LIFECYCLE_EXEC_LOG_FILE: &str = "workspace-server-lifecycle.log";
/// Bound on building the pre-stop broker; past it the diagnostics socket binds without the lifecycle routes.
#[cfg(unix)]
const LIFECYCLE_BROKER_START_TIMEOUT: Duration = Duration::from_secs(5);
/// The pre-stop broker: image manifests from `image_dir`, registrations under the durable workspace home.
///
/// Unix only. On Windows the diagnostics listener is loopback TCP, which every local account can reach, and the host
/// never sends pre-stop to Windows guests, so the lifecycle routes are not served there.
#[cfg(unix)]
fn lifecycle_broker(
    image_dir: &std::path::Path,
    workspace_home: &std::path::Path,
    temp_dir: &std::path::Path,
) -> chutes_lifecycle::LifecycleBroker {
    use chutes_lifecycle::{LifecycleBroker, LifecycleConfig};
    LifecycleBroker::new(LifecycleConfig {
        image_dir: image_dir.to_path_buf(),
        state_dir: Some(workspace_home.join("lifecycle").join("handlers")),
        exec_log: temp_dir.join(LIFECYCLE_EXEC_LOG_FILE),
    })
}
fn server_id_startup_error(id: &str) -> Option<String> {
    id.parse::<chutes_tool_protocol::ServerId>()
        .err()
        .map(|e| format!("{INVALID_SERVER_ID_MARKER} {id:?}: {e}"))
}
/// Classify hub-connect Display strings for `/ready` error_class.
/// Auth needles map to `hub_auth`; other hub-connect path failures map to `hub_connect`.
/// Pre-hub workspace setup messages map to `unknown` (still retryable alongside hub_connect).
fn classify_hub_connect_failure(err_msg: &str) -> ErrorClass {
    if err_msg.contains("handshake auth failed") || err_msg.contains("auth error:") {
        ErrorClass::HubAuth
    } else if err_msg.contains("failed to create workspace") {
        ErrorClass::Unknown
    } else {
        ErrorClass::HubConnect
    }
}
/// Drop outer `hub error: ` so `/ready` detail is the inner failure text.
fn hub_connect_error_detail(err_msg: &str) -> &str {
    err_msg.strip_prefix("hub error: ").unwrap_or(err_msg)
}
fn hub_connect_failure_log_message(class: ErrorClass) -> &'static str {
    match class {
        ErrorClass::HubAuth => WORKSPACE_HUB_AUTH_FAILED_MARKER,
        ErrorClass::HubConnect | ErrorClass::Unknown => "failed to connect workspace to hub",
    }
}
/// Mark `/ready` failed and dwell so the host can observe state before exit.
async fn report_hub_connect_failure(diag: &DiagHandle, err: &WorkspaceError) {
    let err_msg = err.to_string();
    let class = classify_hub_connect_failure(&err_msg);
    diag.set_failed(class, hub_connect_error_detail(&err_msg));
    tracing::error!(error = %err_msg, "{}", hub_connect_failure_log_message(class));
    dwell_after_hub_connect_failed().await;
}
async fn dwell_after_hub_connect_failed() {
    tokio::time::sleep(HUB_CONNECT_FAILED_DWELL).await;
}
#[derive(Parser)]
#[command(name = "chutes-workspace-server")]
#[command(about = "Standalone workspace ToolServer for the server connection")]
struct Args {
    /// Print the capability manifest as JSON to stdout and exit 0.
    /// Legacy binaries reject the unknown flag via clap (non-zero exit), giving the launcher a definitive feature probe.
    #[arg(long)]
    capabilities: bool,
    #[arg(long, default_value = "wss://computer-hub.chutes.ai/v1/tools")]
    hub_url: String,
    #[arg(long)]
    auth_config: Option<PathBuf>,
    #[arg(long)]
    cwd: Option<PathBuf>,
    /// Stable server identity for hub registration.
    /// Used as the `server_id` in `servers.list` and `server.bind` so clients can address this specific workspace server.
    /// When omitted, the SDK default ("workspace-server") is used.
    #[arg(long)]
    server_id: Option<String>,
    /// JSON metadata attached to the tool server registration.
    /// Propagated to `ServerInfo.metadata` in `servers.list` responses.
    #[arg(long)]
    metadata: Option<String>,
    /// Deprecated no-op, accepted for one release so existing callers don't trip clap: nothing writes or reads this path.
    #[arg(long, hide = true)]
    ready_file: Option<PathBuf>,
    /// Unix-socket path for the in-guest diagnostics HTTP server (`/ready`, `/statusz`).
    #[cfg(unix)]
    #[arg(long, default_value = diag_server::DEFAULT_DIAG_SOCKET_PATH)]
    diag_socket: PathBuf,
    /// Loopback TCP port for the diagnostics HTTP server (Windows guests, which lack a reliable Unix-socket HTTP client).
    #[cfg(windows)]
    #[arg(long, default_value_t = diag_server::DEFAULT_DIAG_PORT)]
    diag_port: u16,
    /// Permit a plaintext `ws://` hub on a non-loopback host.
    /// Only for a mesh-secured transport; the bearer crosses the network otherwise.
    #[arg(long)]
    allow_insecure_ws: bool,
    /// Route per-turn uploads through the durable on-disk upload queue (retries and spill-to-disk) instead of the legacy `gcs::upload_bytes` path.
    /// Enabled by default. Accepts `true`/`false`.
    /// Pass `--upload-queue-enabled false` (or set `CHUTES_BUILD_WORKSPACE_UPLOAD_QUEUE_ENABLED=false`) to fall back to the legacy inline path.
    #[arg(
        long,
        env = "CHUTES_BUILD_WORKSPACE_UPLOAD_QUEUE_ENABLED",
        default_value_t = true,
        action = clap::ArgAction::Set,
    )]
    upload_queue_enabled: bool,
    /// Fail `session.bind`s without an explicit toolset closed (RPC-only) instead of widening to the built-in default catalog.
    #[arg(long)]
    require_explicit_toolset: bool,
    /// Trust project-scoped LSP servers from `<repo>/.chutes-build/lsp.json`.
    /// Defaults off; sandbox opts in only after workspace trust is established.
    #[arg(
        long,
        env = "CHUTES_BUILD_WORKSPACE_PROJECT_LSP_TRUSTED",
        default_value_t = false,
        action = clap::ArgAction::Set,
    )]
    project_lsp_trusted: bool,
    /// Confine `chutes.ai/fs/*` resolution to the workspace root (reject `..`, absolute-outside-root, symlink escapes).
    /// On by default: the standalone server always backs a remote-sandbox workspace, a real tenant boundary.
    /// Override with `CHUTES_BUILD_WORKSPACE_CONFINE_FS_TO_ROOT=false` (e.g. local dev).
    #[arg(
        long,
        env = "CHUTES_BUILD_WORKSPACE_CONFINE_FS_TO_ROOT",
        default_value_t = true,
        action = clap::ArgAction::Set,
    )]
    confine_fs_to_workspace_root: bool,
    /// Self-daemonize at startup: double-fork and `setsid()` into a new session and process group, escaping the launcher's process-group reap.
    /// Redirect stdio to a log file and hold a single-instance pidfile lock.
    /// Off by default; the launcher passes it in the supervised deployment mode. With the flag absent, startup is unchanged.
    #[arg(long)]
    daemonize: bool,
    /// Where `--daemonize` redirects stdout and stderr. Ignored without `--daemonize`.
    #[arg(long, default_value = daemonize::DEFAULT_LOG_PATH)]
    log_file: PathBuf,
    /// Single-instance pidfile lock path used with `--daemonize`. Ignored without `--daemonize`.
    #[arg(long, default_value = daemonize::DEFAULT_PIDFILE_PATH)]
    pid_file: PathBuf,
    /// Record `workspace_oom_protect_applied` and lower or recheck `oom_score_adj` to -900.
    /// Force `CHUTES_BUILD_TOOLS_RESET_CHILD_OOM` so shell/pty children reset to 0.
    /// Complements always-on self-protect after pre-unshare inheritance; forces the child reset even if the early write failed. Off by default.
    #[arg(long)]
    oom_protect: bool,
    #[command(flatten)]
    preview: PreviewCliArgs,
}
/// Retired preview-proxy supervision flags (removed in 2.0.0: no Chutes preview
/// proxy exists to supervise). Parsed and ignored so launchers that still pass
/// them keep working; `--preview-enabled` logs a warning and does nothing.
/// Lenient by design: values are never validated anymore.
#[derive(clap::Args, Debug)]
struct PreviewCliArgs {
    /// Retired: accepted and ignored.
    #[arg(long)]
    preview_enabled: bool,
    /// Retired: accepted and ignored.
    #[arg(long)]
    preview_port: Option<u16>,
    /// Retired: accepted and ignored.
    #[arg(long)]
    preview_control_port: Option<u16>,
    /// Retired: accepted and ignored.
    #[arg(long)]
    preview_visibility: Option<String>,
    /// Retired: accepted and ignored.
    #[arg(long)]
    preview_instance_suffix: Option<String>,
    /// Retired: accepted and ignored.
    #[arg(long)]
    preview_auth_redirect: Option<String>,
    /// Retired: accepted and ignored.
    #[arg(long)]
    preview_allow_public: bool,
    /// Retired: accepted and ignored.
    #[arg(long)]
    preview_workspace_server_port: Option<u16>,
}
/// Capability manifest printed by `--capabilities`, consumed by the sandbox launcher to pick a launch protocol.
/// Additions are backward-compatible.
#[derive(Debug, serde::Serialize)]
struct Capabilities {
    /// The in-guest diagnostics HTTP server (`/ready`, `/statusz`, `/logs`).
    diag: bool,
}
const CAPABILITIES: Capabilities = Capabilities { diag: true };
fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();
    if args.capabilities {
        println!("{}", serde_json::to_string(&CAPABILITIES)?);
        return Ok(());
    }
    if let Some(msg) = args.server_id.as_deref().and_then(server_id_startup_error) {
        eprintln!("{msg}");
        std::process::exit(EXIT_SERVER_ID_INVALID);
    }
    let cwd = match args.cwd {
        Some(ref p) => dunce::canonicalize(p)?,
        None => std::env::current_dir()?,
    };
    let oom_protection = chutes_tty_utils::protect_from_oom_kill();
    let _pidfile_guard = if args.daemonize {
        let anchor = |p: PathBuf| if p.is_absolute() { p } else { cwd.join(p) };
        args.log_file = anchor(std::mem::take(&mut args.log_file));
        args.pid_file = anchor(std::mem::take(&mut args.pid_file));
        #[cfg(unix)]
        {
            args.diag_socket = anchor(std::mem::take(&mut args.diag_socket));
        }
        args.auth_config = args.auth_config.take().map(anchor);
        daemonize::daemonize(&args.log_file)?;
        match daemonize::PidFile::acquire_or_take_over(&args.pid_file, daemonize::TAKEOVER_GRACE)? {
            Some(guard) => Some(guard),
            None => return Ok(()),
        }
    } else {
        None
    };
    let oom_protect_applied = if args.oom_protect {
        Some(daemonize::apply_workspace_oom_protect())
    } else {
        None
    };
    #[cfg(unix)]
    if should_set_reset_child_oom(oom_protection.is_ok(), args.oom_protect) {
        unsafe { std::env::set_var(chutes_tty_utils::RESET_CHILD_OOM_ENV, "1") };
    }
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder
        .worker_threads(chutes_tty_utils::runtime::capped_worker_threads().get())
        .enable_all();
    let rt = chutes_tty_utils::runtime::build_with_blocking_pool(&mut builder)?;
    rt.block_on(run(args, cwd, oom_protection, oom_protect_applied))
}
/// The same binary serves sandbox containers and headless user machines. Only the sandbox launcher
/// passes a `sandbox_id` in `--metadata`, so that is the opt-in to the sandbox's full catalog and
/// credential reach; the environment is not a policy input (a `chutes-build --local-workspace` spawned by a
/// hook or MCP child inherits `CHUTES_BUILD_SESSION_ID`). Pickers label the server by the same kind.
fn host_kind_for(metadata: Option<&serde_json::Value>) -> WorkspaceHostKind {
    let is_sandbox = metadata
        .map(WorkspaceServerMetadata::from_metadata)
        .is_some_and(|identity| identity.sandbox_id.is_some_and(|id| !id.is_empty()));
    if is_sandbox {
        WorkspaceHostKind::Sandbox
    } else {
        WorkspaceHostKind::Daemon
    }
}
/// Whether to set `CHUTES_BUILD_TOOLS_RESET_CHILD_OOM` after the always-on protect attempt.
/// Always-on success must set it so children do not inherit -900.
/// `--oom-protect` forces the env even when the early write failed (pre-unshare may still have left the score at -900).
fn should_set_reset_child_oom(early_protect_ok: bool, oom_protect_flag: bool) -> bool {
    early_protect_ok || oom_protect_flag
}
/// Whether the startup log should report OOM protection as active.
/// Prefer the `--oom-protect` apply outcome when present (already-at-target after pre-unshare counts as active even if the early write failed).
/// Without the flag, report the always-on write only.
fn oom_protect_log_active(applied: Option<bool>, early_ok: bool) -> bool {
    applied.unwrap_or(early_ok)
}
async fn run(
    args: Args,
    cwd: PathBuf,
    oom_protection: std::io::Result<()>,
    oom_protect_applied: Option<bool>,
) -> anyhow::Result<()> {
    chutes_extra_ca::ensure_default_crypto_provider();
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let donating = chutes_computer_hub_sdk::DonatingLogLayer::new_inert();
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .with(donating.clone())
        .init();
    if oom_protect_log_active(oom_protect_applied, oom_protection.is_ok()) {
        tracing::info!("kernel OOM-kill protection active");
    } else if let (None, Err(e)) = (oom_protect_applied, &oom_protection) {
        tracing::info!(error = %e, "kernel OOM-kill protection not active");
    } else {
        tracing::info!("kernel OOM-kill protection not active");
    }
    let direct_otlp = match std::env::var("CHUTES_BUILD_WORKSPACE_OTLP_ENDPOINT") {
        Ok(endpoint) if !endpoint.is_empty() => {
            match chutes_tracing::init_fastrace(endpoint.clone(), SERVICE_NAME.to_owned(), None) {
                Ok(()) => {
                    tracing::info!(%endpoint, "trace export enabled (direct OTLP)");
                    true
                }
                Err(e) => {
                    tracing::warn!(error = %e, "direct OTLP trace export init failed");
                    false
                }
            }
        }
        _ => false,
    };
    let url = Url::parse(&args.hub_url).map_err(|e| anyhow::anyhow!("invalid --hub-url: {e}"))?;
    {
        use chutes_sandbox::{ProfileName, SandboxManager};
        let profile = match std::env::var("CHUTES_BUILD_SANDBOX_PROFILE").ok() {
            Some(val) => {
                let parsed = val
                    .parse::<ProfileName>()
                    .expect("ProfileName::from_str is infallible");
                if matches!(parsed, ProfileName::Custom(_)) {
                    tracing::warn!(value = %val,
                        "Unrecognized CHUTES_BUILD_SANDBOX_PROFILE, defaulting to workspace");
                    ProfileName::Workspace
                } else {
                    parsed
                }
            }
            None if chutes_sandbox::trust_bwrap_marker_for_devbox() => ProfileName::Devbox,
            None => ProfileName::Workspace,
        };
        let profile_name = profile.to_string();
        if profile == ProfileName::Off {
            tracing::info!(profile = %profile_name, "Sandbox explicitly disabled via CHUTES_BUILD_SANDBOX_PROFILE=off");
        } else {
            let mut sandbox = SandboxManager::new(profile, &cwd);
            if let Err(e) = sandbox.apply(&cwd) {
                tracing::warn!(error = %e, "Sandbox apply returned error, continuing unsandboxed");
            } else if !sandbox.is_applied() {
                tracing::warn!("Sandbox could not be applied (unsupported platform)");
            }
            sandbox.install();
            let active = chutes_sandbox::is_active();
            let status_msg = if active {
                "Workspace server sandbox active"
            } else {
                "Workspace server sandbox NOT active"
            };
            tracing::info!(
                profile = %profile_name,
                active,
                restrict_network_at_known_linux_launches = chutes_sandbox::should_restrict_child_network(),
                "{status_msg}"
            );
        }
    }
    let status_config = chutes_workspace::StatusConfig::from_env();
    let auth_provider = chutes_workspace::hub_auth::provider(
        &url,
        args.auth_config.as_deref(),
        &status_config.oidc_refresh,
    )?;
    tracing::info!(
        hub_url = %url,
        cwd = %cwd.display(),
        "Starting workspace server"
    );
    let cwd_display = cwd.display().to_string();
    let session_id = std::env::var("CHUTES_BUILD_SESSION_ID").ok();
    let parsed_metadata = match args.metadata {
        Some(json_str) => Some(
            serde_json::from_str(&json_str)
                .map_err(|e| anyhow::anyhow!("invalid --metadata JSON: {e}"))?,
        ),
        None => None,
    };
    let host_kind = host_kind_for(parsed_metadata.as_ref());
    let metadata = merge_host_identity_metadata(
        merge_session_metadata(parsed_metadata, session_id),
        host_kind.as_wire_str(),
    );
    let launch_id = metadata
        .as_ref()
        .and_then(|v| v.get("launch_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let diag_handle = diag_server::DiagHandle::new(launch_id);
    {
        let caps = chutes_workspace::image_capabilities::image_capabilities();
        diag_handle.set_image_capabilities(caps.wire(), caps.is_declared());
    }
    #[cfg(unix)]
    let diag_listener = diag_server::DiagListener::Unix(args.diag_socket);
    #[cfg(windows)]
    let diag_listener = diag_server::DiagListener::Tcp(args.diag_port);
    let diag_log_file = args.daemonize.then_some(args.log_file);
    #[cfg(unix)]
    let broker = tokio::time::timeout(
        LIFECYCLE_BROKER_START_TIMEOUT,
        tokio::task::spawn_blocking(|| {
            lifecycle_broker(
                std::path::Path::new(chutes_lifecycle::DEFAULT_IMAGE_HANDLER_DIR),
                &chutes_workspace::handle::resolve_workspace_home(),
                &std::env::temp_dir(),
            )
        }),
    )
    .await;
    #[cfg(unix)]
    let diag_options = match broker {
        Ok(Ok(broker)) => diag_server::DiagServeOptions {
            log_file: diag_log_file,
            extra_routes: broker.router(),
        },
        Ok(Err(e)) => {
            tracing::error!(error = %e, "pre-stop lifecycle broker failed to start; serving diagnostics without it");
            diag_server::DiagServeOptions {
                log_file: diag_log_file,
                ..diag_server::DiagServeOptions::default()
            }
        }
        Err(_elapsed) => {
            tracing::error!(
                timeout_s = LIFECYCLE_BROKER_START_TIMEOUT.as_secs(),
                "pre-stop lifecycle broker start timed out; serving diagnostics without it"
            );
            diag_server::DiagServeOptions {
                log_file: diag_log_file,
                ..diag_server::DiagServeOptions::default()
            }
        }
    };
    #[cfg(windows)]
    let diag_options = diag_server::DiagServeOptions {
        log_file: diag_log_file,
        ..diag_server::DiagServeOptions::default()
    };
    let _diag_server = match diag_server::serve_with(
        diag_listener,
        diag_handle.clone(),
        diag_options,
    )
    .await
    {
        Ok(bound) => {
            tracing::info!(addr = %bound.addr, "diagnostics server listening");
            Some(bound)
        }
        Err(e) => {
            if args.daemonize {
                tracing::error!(error = %e, "{}", diag_server::DIAG_BIND_FAILED_MARKER);
                std::process::exit(diag_server::EXIT_DIAG_BIND_FAILED);
            }
            tracing::warn!(error = %e, "{} (continuing without)", diag_server::DIAG_BIND_FAILED_MARKER);
            None
        }
    };
    tracing::info!(
        cwd = %cwd_display,
        "Workspace server starting — sessions created dynamically via server bind"
    );
    let server_id = args.server_id.clone();
    if args.preview.preview_enabled {
        tracing::warn!(
            "preview-proxy supervision was removed in 2.0.0 (no Chutes preview proxy exists); ignoring --preview-enabled"
        );
    }
    chutes_workspace::init_metrics();
    let ws_handle = match chutes_workspace::handle::connect_local_workspace(
        cwd,
        url,
        auth_provider,
        chutes_workspace::LocalWorkspaceConnectOptions {
            metadata,
            server_id: server_id.clone(),
            alpha_test_key: None,
            allow_insecure_ws: args.allow_insecure_ws,
            status_config,
            upload_queue_enabled: args.upload_queue_enabled,
            project_lsp_trusted: args.project_lsp_trusted,
            diag: Some(diag_handle.clone()),
            require_explicit_toolset: args.require_explicit_toolset,
            confine_fs_to_workspace_root: args.confine_fs_to_workspace_root,
            on_handshake_refused: None,
            bind_mcp: None,
            host_kind,
            sandbox: None,
        },
    )
    .await
    {
        Ok(handle) => handle,
        Err(e) => {
            report_hub_connect_failure(&diag_handle, &e).await;
            return Err(anyhow::anyhow!("failed to connect workspace to hub: {e}"));
        }
    };
    let mut donation_pump = None;
    if !direct_otlp {
        match ws_handle.trace_donation_reporter(SERVICE_NAME).await {
            Some((reporter, pump)) => {
                fastrace::set_reporter(reporter, fastrace::collector::Config::default());
                donation_pump = Some(pump);
                tracing::info!("trace export enabled");
            }
            None => tracing::info!("trace export disabled (not connected)"),
        }
    }
    let mut log_donation_pump = None;
    match ws_handle.log_donation_layer(SERVICE_NAME).await {
        Some((sender, pump)) => {
            donating.activate(sender);
            log_donation_pump = Some(pump);
            tracing::info!("log export enabled");
        }
        None => tracing::info!("log export disabled (not connected)"),
    }
    let mut metric_donation_pump = None;
    match ws_handle.metric_donation_reporter(SERVICE_NAME).await {
        Some(pump) => {
            metric_donation_pump = Some(pump);
            tracing::info!("metric export enabled");
        }
        None => tracing::info!("metric export disabled (not connected)"),
    }
    tracing::info!(
        server_id = ?server_id,
        "Workspace server connected to hub. Serving tools."
    );
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    diag_handle.set_shutting_down();
    tracing::info!("Received shutdown signal, draining...");
    let tracker = ws_handle.activity_tracker().clone();
    let grace_budget = chutes_workspace::handle::termination_grace_from_env();
    ws_handle
        .two_phase_drain(grace_budget, chutes_workspace::handle::DrainReason::Sigterm)
        .await;
    tracker.set_shutting_down();
    tracing::info!("Shutting down...");
    fastrace::flush();
    if let Some(pump) = &donation_pump {
        pump.drain().await;
    }
    chutes_computer_hub_sdk::flush_log_layer();
    if let Some(pump) = &log_donation_pump {
        pump.drain().await;
    }
    if let Some(pump) = &metric_donation_pump {
        pump.drain().await;
    }
    ws_handle.shutdown_hub().await;
    chutes_sandbox::flush();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Only the launcher's `sandbox_id` opts in; a session id alone (which a hook- or MCP-spawned
    /// server inherits from its environment) or no metadata at all is a daemon.
    #[test]
    fn only_a_launcher_supplied_sandbox_id_makes_a_sandbox() {
        assert_eq!(
            WorkspaceHostKind::Sandbox,
            host_kind_for(Some(
                &serde_json::json!({ "sandbox_id": "sb-1", "session_id": "s-1" })
            ))
        );
        for metadata in [
            None,
            Some(serde_json::json!({})),
            Some(serde_json::json!({ "session_id": "s-1" })),
            Some(serde_json::json!({ "sandbox_id": "" })),
            Some(serde_json::json!("not an object")),
        ] {
            assert_eq!(
                WorkspaceHostKind::Daemon,
                host_kind_for(metadata.as_ref()),
                "{metadata:?}"
            );
        }
    }
    /// Raw HTTP/1.1 over loopback TCP.
    #[cfg(unix)]
    async fn http(port: u16, method: &str, path: &str, body: &str) -> (u16, serde_json::Value) {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: ws\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write request");
        let mut response = Vec::new();
        tokio::time::timeout(Duration::from_secs(20), stream.read_to_end(&mut response))
            .await
            .expect("response in time")
            .expect("read response");
        let response = String::from_utf8(response).expect("utf8 response");
        let (head, body) = response.split_once("\r\n\r\n").expect("header end");
        let status = head
            .split(' ')
            .nth(1)
            .and_then(|code| code.parse().ok())
            .expect("status code");
        let body = if body.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(body).expect("json body")
        };
        (status, body)
    }
    /// The broker answers on the diag listener next to `/ready`, and runs what a process registers there.
    #[cfg(unix)]
    #[tokio::test]
    async fn lifecycle_broker_is_served_on_the_diag_listener() {
        let home = tempfile::tempdir().expect("tempdir");
        let broker = lifecycle_broker(&home.path().join("image"), home.path(), home.path());
        let bound = diag_server::serve_with(
            diag_server::DiagListener::Tcp(0),
            DiagHandle::new(None),
            diag_server::DiagServeOptions {
                log_file: None,
                extra_routes: broker.router(),
            },
        )
        .await
        .expect("bind");
        let port = bound.port.expect("tcp port");
        let trigger = r#"{"v":1,"reason":"manual","disk":"kept","deadline_ms":5000}"#;
        let (status, body) = http(port, "POST", "/lifecycle/pre-stop", trigger).await;
        assert_eq!(200, status);
        assert_eq!(
            Some("no_handlers"),
            body.get("verdict").and_then(serde_json::Value::as_str)
        );
        let registration = r#"{"v":1,"argv":["/bin/sh","-c","exit 0"]}"#;
        let (status, _) = http(port, "PUT", "/lifecycle/handlers/flush", registration).await;
        assert_eq!(200, status);
        let (status, body) = http(port, "POST", "/lifecycle/pre-stop", trigger).await;
        assert_eq!(200, status);
        assert_eq!(
            Some("ok"),
            body.get("verdict").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("flush"),
            body.pointer("/handlers/0/name")
                .and_then(serde_json::Value::as_str)
        );
        #[cfg(target_os = "linux")]
        assert!(home.path().join("lifecycle/handlers/flush.json").is_file());
        let (status, ready) = http(port, "GET", "/ready", "").await;
        assert_eq!(503, status);
        assert_eq!(
            Some("starting"),
            ready.get("state").and_then(serde_json::Value::as_str)
        );
    }
    #[test]
    fn hub_connect_failed_dwell_is_within_design_bounds() {
        assert!(HUB_CONNECT_FAILED_DWELL >= Duration::from_millis(500));
        assert!(HUB_CONNECT_FAILED_DWELL <= Duration::from_secs(2));
    }
    #[tokio::test(start_paused = true)]
    async fn hub_connect_failed_dwell_elapses_exact_budget() {
        let start = tokio::time::Instant::now();
        dwell_after_hub_connect_failed().await;
        assert_eq!(start.elapsed(), HUB_CONNECT_FAILED_DWELL);
    }
    #[test]
    fn classify_hub_connect_auth_needles() {
        assert_eq!(
            classify_hub_connect_failure("hub error: handshake auth failed: HTTP 401"),
            ErrorClass::HubAuth
        );
        assert_eq!(
            classify_hub_connect_failure("handshake auth failed: HTTP 401"),
            ErrorClass::HubAuth
        );
        assert_eq!(
            classify_hub_connect_failure("hub error: auth error: token rejected"),
            ErrorClass::HubAuth
        );
        assert_eq!(
            classify_hub_connect_failure("HTTP 401 unauthorized"),
            ErrorClass::HubConnect
        );
        assert_eq!(
            classify_hub_connect_failure("token refresh failed"),
            ErrorClass::HubConnect
        );
    }
    #[test]
    fn classify_from_client_error_display_round_trip() {
        let handshake = WorkspaceError::HubRefused {
            status: 401,
            refusal: None,
        };
        let handshake_msg = handshake.to_string();
        assert_eq!(
            classify_hub_connect_failure(&handshake_msg),
            ErrorClass::HubAuth
        );
        assert_eq!(
            hub_connect_failure_log_message(ErrorClass::HubAuth),
            WORKSPACE_HUB_AUTH_FAILED_MARKER
        );
        let auth = WorkspaceError::HubError(
            chutes_computer_hub_sdk::ClientError::AuthError("token rejected".into()).to_string(),
        );
        assert_eq!(
            classify_hub_connect_failure(&auth.to_string()),
            ErrorClass::HubAuth
        );
        let network = WorkspaceError::HubError(
            chutes_computer_hub_sdk::ClientError::NetworkError("connection refused".into())
                .to_string(),
        );
        assert_eq!(
            classify_hub_connect_failure(&network.to_string()),
            ErrorClass::HubConnect
        );
        assert_ne!(
            hub_connect_failure_log_message(ErrorClass::HubConnect),
            WORKSPACE_HUB_AUTH_FAILED_MARKER
        );
    }
    #[test]
    fn classify_hub_connect_non_auth_is_hub_connect() {
        assert_eq!(
            classify_hub_connect_failure("hub error: network error: connection refused"),
            ErrorClass::HubConnect
        );
        assert_eq!(
            classify_hub_connect_failure("hub error: protocol error: bad hello"),
            ErrorClass::HubConnect
        );
        assert_eq!(
            classify_hub_connect_failure("failed to create workspace: disk full"),
            ErrorClass::Unknown
        );
    }
    #[test]
    fn hub_auth_marker_is_stable_literal() {
        assert_eq!(
            WORKSPACE_HUB_AUTH_FAILED_MARKER,
            "workspace hub auth failed"
        );
    }
    #[test]
    fn hub_connect_error_detail_strips_hub_error_prefix() {
        let err = WorkspaceError::HubError("handshake auth failed: HTTP 401".into());
        assert_eq!(
            hub_connect_error_detail(&err.to_string()),
            "handshake auth failed: HTTP 401"
        );
        let other = WorkspaceError::HubError("network error: timeout".into());
        assert_eq!(
            hub_connect_error_detail(&other.to_string()),
            "network error: timeout"
        );
    }
    /// Install a capturing tracing subscriber for the duration of an async report; returns emitted event messages.
    async fn report_with_captured_messages(
        handle: &DiagHandle,
        err: &WorkspaceError,
    ) -> (Duration, Vec<String>) {
        use std::sync::{Arc, Mutex};
        use tracing::field::{Field, Visit};
        use tracing_subscriber::layer::{Context, SubscriberExt as _};
        use tracing_subscriber::{Layer, Registry};
        #[derive(Default)]
        struct MsgVisitor {
            message: Option<String>,
        }
        impl Visit for MsgVisitor {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = Some(format!("{value:?}").trim_matches('"').to_owned());
                }
            }
            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() == "message" {
                    self.message = Some(value.to_owned());
                }
            }
        }
        struct CaptureLayer {
            msgs: Arc<Mutex<Vec<String>>>,
        }
        impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
            fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
                let mut v = MsgVisitor::default();
                event.record(&mut v);
                if let Some(msg) = v.message {
                    self.msgs
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(msg);
                }
            }
        }
        let msgs = Arc::new(Mutex::new(Vec::new()));
        let subscriber = Registry::default().with(CaptureLayer { msgs: msgs.clone() });
        let _guard = tracing::subscriber::set_default(subscriber);
        let start = tokio::time::Instant::now();
        report_hub_connect_failure(handle, err).await;
        let elapsed = start.elapsed();
        let messages = msgs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        (elapsed, messages)
    }
    #[tokio::test(start_paused = true)]
    async fn report_hub_connect_failure_sets_ready_failed_auth_and_dwells() {
        let handle = DiagHandle::new(Some("nonce-auth".to_owned()));
        let bound = diag_server::serve(diag_server::DiagListener::Tcp(0), handle.clone(), None)
            .await
            .expect("bind");
        let port = bound.port.expect("tcp port");
        let err = WorkspaceError::HubError("handshake auth failed: HTTP 401".into());
        let (elapsed, messages) = report_with_captured_messages(&handle, &err).await;
        assert_eq!(elapsed, HUB_CONNECT_FAILED_DWELL);
        assert!(
            messages
                .iter()
                .any(|m| m == WORKSPACE_HUB_AUTH_FAILED_MARKER),
            "auth path must emit marker, got {messages:?}"
        );
        let response = reqwest::get(format!("http://127.0.0.1:{port}/ready"))
            .await
            .expect("request");
        assert_eq!(response.status().as_u16(), 503);
        let body: serde_json::Value = response.json().await.expect("json");
        assert_eq!(body.get("state").and_then(|v| v.as_str()), Some("failed"));
        assert_eq!(
            body.get("error_class").and_then(|v| v.as_str()),
            Some("hub_auth")
        );
        assert_eq!(
            body.get("error_detail").and_then(|v| v.as_str()),
            Some("handshake auth failed: HTTP 401")
        );
        assert_eq!(
            body.get("launch_id").and_then(|v| v.as_str()),
            Some("nonce-auth")
        );
    }
    #[tokio::test(start_paused = true)]
    async fn report_hub_connect_failure_sets_ready_failed_hub_connect() {
        let handle = DiagHandle::new(None);
        let bound = diag_server::serve(diag_server::DiagListener::Tcp(0), handle.clone(), None)
            .await
            .expect("bind");
        let port = bound.port.expect("tcp port");
        let err = WorkspaceError::HubError("network error: connection refused".into());
        let (elapsed, messages) = report_with_captured_messages(&handle, &err).await;
        assert_eq!(elapsed, HUB_CONNECT_FAILED_DWELL);
        assert!(
            messages
                .iter()
                .any(|m| m == "failed to connect workspace to hub"),
            "non-auth path must emit connect failure line, got {messages:?}"
        );
        assert!(
            messages
                .iter()
                .all(|m| m != WORKSPACE_HUB_AUTH_FAILED_MARKER),
            "non-auth path must not emit auth marker, got {messages:?}"
        );
        let response = reqwest::get(format!("http://127.0.0.1:{port}/ready"))
            .await
            .expect("request");
        assert_eq!(response.status().as_u16(), 503);
        let body: serde_json::Value = response.json().await.expect("json");
        assert_eq!(body.get("state").and_then(|v| v.as_str()), Some("failed"));
        assert_eq!(
            body.get("error_class").and_then(|v| v.as_str()),
            Some("hub_connect")
        );
        assert_eq!(
            body.get("error_detail").and_then(|v| v.as_str()),
            Some("network error: connection refused")
        );
    }
    #[test]
    fn capabilities_flag_parses_and_defaults_off() {
        let args = Args::try_parse_from(["chutes-workspace-server"]).unwrap();
        assert!(!args.capabilities);
        let args = Args::try_parse_from(["chutes-workspace-server", "--capabilities"]).unwrap();
        assert!(args.capabilities);
    }
    #[test]
    fn project_lsp_trust_defaults_off_and_is_opt_in() {
        unsafe { std::env::remove_var("CHUTES_BUILD_WORKSPACE_PROJECT_LSP_TRUSTED") };
        let args = Args::try_parse_from(["chutes-workspace-server"]).unwrap();
        assert!(!args.project_lsp_trusted);
        let args =
            Args::try_parse_from(["chutes-workspace-server", "--project-lsp-trusted", "true"])
                .unwrap();
        assert!(args.project_lsp_trusted);
    }
    #[test]
    fn capabilities_manifest_shape() {
        let value = serde_json::to_value(CAPABILITIES).unwrap();
        assert_eq!(value, serde_json::json!({"diag": true}));
    }
    #[test]
    fn capabilities_probe_of_legacy_binary_exits_nonzero() {
        /// A stand-in for the Args of a binary predating `--capabilities`.
        #[derive(Debug, Parser)]
        struct LegacyArgs {
            #[arg(long)]
            daemonize: bool,
        }
        let err = LegacyArgs::try_parse_from(["chutes-workspace-server", "--capabilities"])
            .expect_err("a legacy binary must reject the flag");
        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
        assert_ne!(
            err.exit_code(),
            0,
            "the probe relies on a non-zero exit distinguishing legacy binaries"
        );
    }
    #[test]
    fn daemonize_defaults_are_inert() {
        let args = Args::try_parse_from(["chutes-workspace-server"]).unwrap();
        assert!(!args.daemonize);
        assert!(!args.oom_protect);
        assert_eq!(args.log_file, PathBuf::from(daemonize::DEFAULT_LOG_PATH));
        assert_eq!(
            args.pid_file,
            PathBuf::from(daemonize::DEFAULT_PIDFILE_PATH)
        );
        assert_eq!(args.ready_file, None);
    }
    #[test]
    fn should_set_reset_child_oom_truth_table() {
        assert!(!should_set_reset_child_oom(false, false));
        assert!(should_set_reset_child_oom(true, false));
        assert!(should_set_reset_child_oom(false, true));
        assert!(should_set_reset_child_oom(true, true));
    }
    #[test]
    fn oom_protect_log_active_truth_table() {
        assert!(oom_protect_log_active(Some(true), false));
        assert!(!oom_protect_log_active(Some(false), true));
        assert!(oom_protect_log_active(None, true));
        assert!(!oom_protect_log_active(None, false));
    }
    #[test]
    fn oom_protect_flag_parses() {
        let args = Args::try_parse_from(["chutes-workspace-server", "--oom-protect"]).unwrap();
        assert!(args.oom_protect);
    }
    #[test]
    fn ready_file_is_accepted_as_a_deprecated_no_op() {
        let args =
            Args::try_parse_from(["chutes-workspace-server", "--ready-file", "/tmp/x.ready"])
                .unwrap();
        assert_eq!(args.ready_file, Some(PathBuf::from("/tmp/x.ready")));
    }
    #[test]
    fn invalid_server_id_produces_the_marker_line() {
        for bad in ["auto:tool:x", ""] {
            let msg = server_id_startup_error(bad)
                .unwrap_or_else(|| panic!("server id {bad:?} must be rejected"));
            assert!(
                msg.starts_with(INVALID_SERVER_ID_MARKER),
                "startup error must START with the greppable marker prefix: {msg}"
            );
        }
        assert_eq!(server_id_startup_error("session-abc-123"), None);
    }
    #[test]
    fn argv_rejection_exit_code_is_distinct_from_server_id_exit_code() {
        let err = Args::try_parse_from(["chutes-workspace-server", "--flag-from-the-future"])
            .err()
            .expect("unknown argv must be rejected");
        assert_eq!(err.exit_code(), 2, "clap argv rejection exits 2");
        assert_ne!(err.exit_code(), EXIT_SERVER_ID_INVALID);
        assert_ne!(EXIT_SERVER_ID_INVALID, 0);
        assert_ne!(EXIT_SERVER_ID_INVALID, 1);
    }
    #[test]
    fn retired_preview_flags_parse_and_are_ignored() {
        // Launcher compat: the removed supervision's flags still parse (any
        // value, including invalid ones) and change nothing.
        let args = Args::try_parse_from([
            "chutes-workspace-server",
            "--preview-enabled",
            "--preview-port",
            "6014",
            "--preview-control-port",
            "6015",
            "--preview-visibility",
            "nobody",
            "--preview-instance-suffix",
            ".inst.example",
            "--preview-auth-redirect",
            "https://chutes.ai/preview-auth",
            "--preview-allow-public",
            "--preview-workspace-server-port",
            "8470",
        ])
        .unwrap();
        assert!(args.preview.preview_enabled);
        assert_eq!(args.preview.preview_visibility.as_deref(), Some("nobody"));
        let defaults = Args::try_parse_from(["chutes-workspace-server"]).unwrap();
        assert!(!defaults.preview.preview_enabled);
    }
}
