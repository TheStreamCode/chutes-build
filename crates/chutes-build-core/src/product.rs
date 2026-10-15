//! Product-level capabilities for the Chutes Build distribution.
//!
//! These constants are intentionally compile-time policy. Remote configuration
//! must not be able to re-enable features that the privacy contract excludes.
//! Each constant names its enforcement site: if you change a value, update
//! the enforcement to match, and vice versa.

/// Uploading or publicly sharing local sessions is not part of Chutes Build.
///
/// Enforced by `require_xai_auth_for_share` (xAI auth cannot exist here) plus
/// the `sharing_enabled` remote flag defaulting to false; the CLI surface is
/// additionally hidden.
pub const REMOTE_SESSION_SHARING: bool = false;

/// Session list/search/delete operate only on the local session store.
///
/// There is no remote registry to query: cross-device pull targets the
/// backend session API, which Chutes does not serve (unknown ids fail closed
/// as `NotFound`).
pub const REMOTE_SESSION_REGISTRY: bool = false;

/// Remote driving of this machine is not part of Chutes Build.
///
/// Enforced by `Distribution::current()`, which withholds `RemoteControl`:
/// the relay connection is never started and the headless relay entrypoint
/// refuses with the distribution refusal. The compiled relay/gateway
/// defaults additionally point at the discard port.
pub const REMOTE_WORKSPACE_EXPOSURE: bool = false;

/// Product analytics and event reporting are not part of Chutes Build. The
/// telemetry crates are retained so upstream merges stay reviewable, but no
/// configuration or remote setting can switch them on: `Distribution::current()`
/// withholds `Telemetry`, which forces the telemetry mode to `Disabled`,
/// disables trace upload, and (via the feature registry) the feedback
/// surface and session-signal sync. This constant is the floor every consumer
/// applies.
pub const TELEMETRY: bool = false;

/// Trace archives can be exported locally but are never uploaded by Chutes Build.
///
/// Enforced by the `Telemetry` withholding above (`resolve_trace_upload`
/// resolves false) and by `SESSION_TRACES_BUCKET` defaulting to `None`
/// unless baked in at compile time for an operator-owned bucket.
pub const REMOTE_TRACE_UPLOAD: bool = false;

/// The coding-data-retention toggle is local-only in Chutes Build.
///
/// Chutes serves no retention API, so `setCodingDataRetention` records the
/// opt-out in local auth state and never PUTs. The pager defaults to
/// opted-out.
pub const CODING_DATA_RETENTION_CONTROLS: bool = false;

/// Upstream feedback upload is not part of Chutes Build.
///
/// Enforced by gating the `Feedback` and `FeedbackTraceCard` features on the
/// `Telemetry` capability, which this distribution withholds: popups, the
/// `/feedback` command, and session-signal sync stay off.
pub const REMOTE_FEEDBACK: bool = false;

/// Chutes Build updates are installed through the published package/release
/// channel and are never downloaded automatically by the running process.
///
/// Enforced by `should_check_for_updates`, which reads this constant before
/// any flag or environment check. The `update` subcommand still checks on
/// explicit user request.
pub const AUTOMATIC_SELF_UPDATE: bool = false;
