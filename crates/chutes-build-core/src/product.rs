//! Product-level capabilities for the Chutes Build distribution.
//!
//! These constants are intentionally compile-time policy. Remote configuration
//! must not be able to re-enable features that the privacy contract excludes.

/// Uploading or publicly sharing local sessions is not part of Chutes Build.
pub const REMOTE_SESSION_SHARING: bool = false;

/// Session list/search/delete operate only on the local session store.
pub const REMOTE_SESSION_REGISTRY: bool = false;

/// Remote workspace exposure through the inherited Computer Hub is not part of Chutes Build.
pub const REMOTE_WORKSPACE_EXPOSURE: bool = false;

/// Trace archives can be exported locally but are never uploaded by Chutes Build.
pub const REMOTE_TRACE_UPLOAD: bool = false;

/// Upstream coding-data retention controls do not apply to Chutes Build.
pub const CODING_DATA_RETENTION_CONTROLS: bool = false;

/// Upstream feedback upload is not part of Chutes Build.
pub const REMOTE_FEEDBACK: bool = false;

/// Chutes Build updates are installed through the published package/release
/// channel and are never downloaded automatically by the running process.
pub const AUTOMATIC_SELF_UPDATE: bool = false;
