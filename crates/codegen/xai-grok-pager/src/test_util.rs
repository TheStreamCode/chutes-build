//! Shared test utilities for the pager crate.
//!
//! Compiled only in `#[cfg(test)]` builds. Import via `crate::test_util`.

/// A temporary directory whose path has no 8.3 short components — use it wherever
/// a test hands a sandbox root to code that treats it as a real `HOME` or cwd.
///
/// `tempfile::tempdir()` builds under `std::env::temp_dir()`, and on the GitHub
/// Windows runner that is `C:\Users\RUNNER~1\AppData\Local\Temp`: `RUNNER~1` is the
/// 8.3 short name for `runneradmin`. `SafeAbsoluteDirectory::parse` rejects any
/// path containing a `~`, and is right to — the path it guards gets written into a
/// shell rc file, where a literal `~` would be re-expanded against the home
/// directory instead of naming the file meant. So thirty-two tests failed on CI and
/// nowhere else, over a property of the runner's `%TEMP%` rather than anything they
/// were testing. A real Windows home arrives from `dirs::home_dir()` in long form,
/// which is what this reproduces.
///
/// Canonicalising resolves every short component. `dunce` is used rather than
/// `std::fs::canonicalize` so the result keeps the plain `C:\…` form: the `\\?\`
/// prefix would pass the guard but not string comparisons against paths the test
/// built by hand.
pub struct SandboxDir {
    // Held only for its `Drop`: the directory lives as long as this value.
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl SandboxDir {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// A temporary directory safe to use as a sandbox root. See [`SandboxDir`].
pub fn sandbox_dir() -> SandboxDir {
    let dir = tempfile::tempdir().unwrap();
    // A path that will not canonicalise is still worth returning: the directory was
    // just created, so this is some transient condition, and the raw path is what
    // the test would have got before.
    let path = dunce::canonicalize(dir.path()).unwrap_or_else(|_| dir.path().to_path_buf());
    SandboxDir { _dir: dir, path }
}

/// A POSIX-shaped fixture path, made absolute for the platform under test.
///
/// `/Users/me/project/src/main.rs` is **not** an absolute path on Windows:
/// `Path::is_absolute` wants a prefix such as `C:`, and a rooted-but-prefixless path
/// is drive-relative. So a fixture written that way exercises a shape the product
/// never meets on Windows — the link layer declines to build a `file:` target from
/// it, and the header relativiser finds no root to strip — and the tests that use
/// one failed there over their fixture rather than over anything they assert.
///
/// Unix returns the string unchanged. Windows prefixes `C:` and switches the
/// separators, so the example above becomes `C:\Users\me\project\src\main.rs`.
pub fn abs_path(posix: &str) -> String {
    if cfg!(windows) {
        format!("C:{}", posix.replace('/', "\\"))
    } else {
        posix.to_owned()
    }
}

/// A POSIX-shaped **relative** path as this platform renders it, for assertions
/// against text the product produced.
///
/// The pager rebuilds a relative path from its components, so it comes back joined
/// with the native separator: `src/main.rs` under a Windows cwd renders as
/// `src\main.rs`. That is the right thing to show a Windows user; it is only the
/// expectations that were POSIX-only. See [`abs_path`].
pub fn rel_path(posix: &str) -> String {
    if cfg!(windows) {
        posix.replace('/', "\\")
    } else {
        posix.to_owned()
    }
}

/// The `file:` URL this platform produces for [`abs_path`] of the same input — on Windows
/// the drive letter joins the path, giving `file:///C:/…`.
pub fn file_url(posix: &str) -> String {
    if cfg!(windows) {
        format!("file:///C:{posix}")
    } else {
        format!("file://{posix}")
    }
}

/// Minimal `AgentView` for unit tests outside the dispatch/handler modules
/// (which keep their own richer factories).
pub fn make_agent_view(session_id: Option<&str>, cwd: &str) -> crate::app::agent_view::AgentView {
    use crate::app::agent::{AgentId, AgentSession, AgentState};
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let session = AgentSession {
        id: AgentId(0),
        acp_tx: tx,
        session_id: session_id.map(agent_client_protocol::SessionId::new),
        models: crate::acp::model_state::ModelState::default(),
        state: AgentState::Idle,
        tracker: crate::acp::tracker::AcpUpdateTracker::new(),
        cwd: std::path::PathBuf::from(cwd),
        is_worktree: false,
        forked_from: None,
        pending_prompts: std::collections::VecDeque::new(),
        next_queue_id: 0,
        yolo_mode: false,
        auto_mode: false,
        prompt_history: Vec::new(),
        prompt_history_loading: false,
        loading_replay: false,
        restore_degree: None,
        rate_limited: false,
        model_incompatible: false,
        credit_limit_blocked: false,
        free_usage_blocked: false,
        available_commands: Vec::new(),
        available_commands_generation: 0,
        available_tools: None,
        model_switch_pending: false,
        user_model_preference: None,
        deferred_model_switch: None,
        bg_tasks: std::collections::BTreeMap::new(),
        bg_tool_call_to_task: std::collections::HashMap::new(),
        scheduled_tasks: std::collections::HashMap::new(),
        in_flight_prompt: None,
        compact_held_prompt: None,
        current_prompt_id: None,
        created_via_new: false,
    };
    crate::app::agent_view::AgentView::new(
        session,
        crate::scrollback::state::ScrollbackState::new(),
    )
}
pub fn make_worktree_record(
    id: &str,
    path: &std::path::Path,
    label: &str,
) -> xai_fast_worktree::WorktreeRecord {
    use xai_fast_worktree::{WorktreeKind, WorktreeRecord, WorktreeStatus};
    WorktreeRecord {
        id: id.to_owned(),
        path: path.to_path_buf(),
        source_repo: "/repo".into(),
        repo_name: "repo".into(),
        kind: WorktreeKind::Session,
        creation_mode: "linked".into(),
        git_ref: None,
        head_commit: None,
        session_id: None,
        creator_pid: None,
        created_at: 0,
        last_accessed_at: None,
        status: WorktreeStatus::Alive,
        metadata: Some(serde_json::json!({ "label": label })),
    }
}
/// Every row containing `row_marker` starts its PATH cell at the header's
/// PATH column, measured in display width so CJK regressions fail.
pub fn assert_path_column_aligned(text: &str, row_marker: &str) {
    use unicode_width::UnicodeWidthStr;
    let lines: Vec<&str> = text.lines().collect();
    let header = lines
        .iter()
        .find(|l| l.ends_with("PATH"))
        .unwrap_or_else(|| panic!("no PATH header in: {text}"));
    let path_col = header.width() - "PATH".width();
    let mut rows = 0;
    for line in lines.iter().filter(|l| l.contains(row_marker)) {
        let (_, path) = line
            .rsplit_once(' ')
            .expect("rows end in a space-free test path (PATH is the last cell)");
        assert_eq!(
            line.width() - path.width(),
            path_col,
            "path column must stay width-aligned: {line:?}"
        );
        rows += 1;
    }
    assert!(rows > 0, "no table rows matched {row_marker:?} in: {text}");
}
/// RAII guard for temporarily overriding an environment variable.
///
/// Captures the original value on construction and restores it on drop.
/// Used by theme and persist tests to redirect `HOME`/`USERPROFILE` to
/// temp directories without affecting the real user config.
pub struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}
impl EnvVarGuard {
    /// Override `key` to `value` (paths, URLs, flags — anything OsStr-able),
    /// returning a guard that restores the original on drop.
    pub fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, original }
    }
}
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = &self.original {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
/// Shared CHUTES_BUILD_HOME boundary fixture for the resume-by-title startup and
/// pre-sandbox tests.
///
/// `grok_home()` is OnceLock-cached process-wide, so summaries land under the
/// *resolved* home (possibly the real `~/.chutes-build` when another test pinned the
/// cache first); cwd-encoded dirnames are tempdir-unique, and cleanup runs on
/// drop so it survives assertion panics. Callers must hold
/// `#[serial_test::serial(CHUTES_BUILD_HOME)]`.
pub struct GrokHomeFixture {
    _home: tempfile::TempDir,
    cwd: tempfile::TempDir,
    cleanup: Vec<std::path::PathBuf>,
}
impl Drop for GrokHomeFixture {
    fn drop(&mut self) {
        for dir in &self.cleanup {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
impl Default for GrokHomeFixture {
    fn default() -> Self {
        Self::new()
    }
}
impl GrokHomeFixture {
    pub fn new() -> Self {
        let home = tempfile::tempdir().expect("home tempdir");
        unsafe { std::env::set_var("CHUTES_BUILD_HOME", home.path()) };
        let cwd = tempfile::tempdir().expect("cwd tempdir");
        Self {
            _home: home,
            cwd,
            cleanup: Vec::new(),
        }
    }
    /// Canonicalized so the summary cwd encoding matches what production
    /// path resolution sees (macOS tempdirs are symlinked). Tests pass this
    /// through the explicit `*_for_cwd` seams; the process cwd is never
    /// mutated.
    pub fn cwd_str(&self) -> String {
        dunce::canonicalize(self.cwd.path())
            .expect("canonicalize cwd")
            .to_string_lossy()
            .to_string()
    }
    /// Write a minimal valid summary.json (every non-defaulted `Summary`
    /// field) for `id` under `cwd`, merging `extra` fields on top.
    pub fn write_summary(&mut self, cwd: &str, id: &str, extra: serde_json::Value) {
        let sessions_cwd_dir = Self::sessions_cwd_dir(cwd);
        if !self.cleanup.contains(&sessions_cwd_dir) {
            self.cleanup.push(sessions_cwd_dir.clone());
        }
        let dir = sessions_cwd_dir.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let mut v = serde_json::json!({
            "info": { "id": id, "cwd": cwd },
            "session_summary": "auto summary",
            "created_at": "2026-07-01T00:00:00Z",
            "updated_at": "2026-07-01T00:00:00Z",
            "num_messages": 1,
            "current_model_id": "chutes-build",
        });
        if let Some(map) = extra.as_object() {
            for (k, val) in map {
                v[k.as_str()] = val.clone();
            }
        }
        std::fs::write(dir.join("summary.json"), serde_json::to_vec(&v).unwrap()).unwrap();
    }
    /// Delete a previously written session dir (concurrent-delete simulation).
    pub fn remove_session(&self, cwd: &str, id: &str) {
        let _ = std::fs::remove_dir_all(Self::sessions_cwd_dir(cwd).join(id));
    }
    fn sessions_cwd_dir(cwd: &str) -> std::path::PathBuf {
        let encoded = xai_grok_shell::util::grok_home::encode_cwd_dirname(cwd);
        xai_grok_shell::util::grok_home::grok_home()
            .join("sessions")
            .join(&encoded)
    }
}
