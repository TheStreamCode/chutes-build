//! Optional multi-user workspace helpers for loading per-user agent config.
//!
//! When `CHUTES_ROOT` and `CHUTES_USER` are set and the resolved directory exists, that path can contribute AGENTS.md / rules / skills discovery.
//! The legacy `XAI_ROOT`/`XAI_USER` names still work as a silent fallback.
//! Unset env vars are a no-op (typical for standalone installs).

use std::path::PathBuf;

/// If optional workspace env vars are set, returns the user's config directory when the resolved path exists on disk.
/// Unset or missing paths yield `None`.
pub fn optional_workspace_user_dir() -> Option<PathBuf> {
    let root = workspace_env("CHUTES_ROOT", "XAI_ROOT")?;
    let user = workspace_env("CHUTES_USER", "XAI_USER")?;
    resolve_workspace_user_dir(&root, &workspace_user_relpath(&user))
}

/// Read a workspace env knob: canonical name first, legacy name as a silent fallback.
fn workspace_env(canonical: &str, legacy: &str) -> Option<String> {
    std::env::var(canonical)
        .or_else(|_| std::env::var(legacy))
        .ok()
        .filter(|value| !value.is_empty())
}

/// Map `$CHUTES_USER` to a path relative to the workspace root.
/// A bare username is nested under `x/` so it cannot collide with an unrelated same-named directory.
/// Values that already contain a path separator are used as-is.
fn workspace_user_relpath(user: &str) -> String {
    if user.contains('/') || user.contains('\\') {
        user.to_string()
    } else {
        format!("x/{user}")
    }
}

/// Join `root` with a relative `user` path and return it if the directory exists.
/// `None` if either argument is empty or the result is not a directory.
pub fn resolve_workspace_user_dir(root: &str, user: &str) -> Option<PathBuf> {
    if root.is_empty() || user.is_empty() {
        return None;
    }
    let path = PathBuf::from(root).join(user);
    path.is_dir().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── resolve_workspace_user_dir (pure, no env vars) ───────────────

    #[test]
    fn resolve_returns_none_for_empty_root() {
        assert!(resolve_workspace_user_dir("", "users/someone").is_none());
    }

    #[test]
    fn resolve_returns_none_for_empty_user() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(resolve_workspace_user_dir(tmp.path().to_str().unwrap(), "").is_none());
    }

    #[test]
    fn resolve_returns_none_for_both_empty() {
        assert!(resolve_workspace_user_dir("", "").is_none());
    }

    #[test]
    fn resolve_returns_none_when_dir_does_not_exist() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            resolve_workspace_user_dir(tmp.path().to_str().unwrap(), "users/nonexistent").is_none()
        );
    }

    #[test]
    fn resolve_returns_path_when_dir_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let user_dir = tmp.path().join("users").join("testuser");
        fs::create_dir_all(&user_dir).unwrap();

        let result = resolve_workspace_user_dir(tmp.path().to_str().unwrap(), "users/testuser");
        assert_eq!(result, Some(user_dir));
    }

    #[test]
    fn resolve_handles_single_component_user() {
        let tmp = tempfile::tempdir().unwrap();
        let user_dir = tmp.path().join("alice");
        fs::create_dir_all(&user_dir).unwrap();

        let result = resolve_workspace_user_dir(tmp.path().to_str().unwrap(), "alice");
        assert_eq!(result, Some(user_dir));
    }

    #[test]
    fn resolve_handles_deeply_nested_user() {
        let tmp = tempfile::tempdir().unwrap();
        let user_dir = tmp.path().join("org").join("team").join("user");
        fs::create_dir_all(&user_dir).unwrap();

        let result = resolve_workspace_user_dir(tmp.path().to_str().unwrap(), "org/team/user");
        assert_eq!(result, Some(user_dir));
    }

    #[test]
    fn resolve_returns_none_when_path_is_file_not_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("users").join("testuser");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "not a directory").unwrap();

        assert!(
            resolve_workspace_user_dir(tmp.path().to_str().unwrap(), "users/testuser").is_none()
        );
    }

    #[test]
    fn resolve_supports_nested_user_layout_path() {
        let tmp = tempfile::tempdir().unwrap();
        let user_dir = tmp.path().join("x").join("testuser");
        fs::create_dir_all(&user_dir).unwrap();

        let result = resolve_workspace_user_dir(tmp.path().to_str().unwrap(), "x/testuser");
        assert_eq!(result, Some(user_dir));
    }

    // ── workspace_user_relpath ───────────────────────────────────────

    #[test]
    fn bare_username_is_nested_under_x() {
        assert_eq!(workspace_user_relpath("alice"), "x/alice");
        assert_eq!(workspace_user_relpath("bob"), "x/bob");
    }

    #[test]
    fn multi_segment_user_is_explicit_relative_path() {
        assert_eq!(workspace_user_relpath("users/alice"), "users/alice");
        assert_eq!(workspace_user_relpath(r"users\alice"), r"users\alice");
    }

    // ── optional_workspace_user_dir (env names) ────────────────────

    struct EnvRestore {
        key: &'static str,
        prior: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let prior = std::env::var_os(key);
            // SAFETY: serialized via `serial_test`; restored on drop.
            unsafe { std::env::set_var(key, value) };
            Self { key, prior }
        }

        fn unset(key: &'static str) -> Self {
            let prior = std::env::var_os(key);
            // SAFETY: serialized via `serial_test`; restored on drop.
            unsafe { std::env::remove_var(key) };
            Self { key, prior }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            // SAFETY: serialized via `serial_test`.
            unsafe {
                match self.prior.take() {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    fn workspace_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let user_dir = root.path().join("x").join("alice");
        fs::create_dir_all(&user_dir).unwrap();
        let root_path = root.path().to_path_buf();
        (root, root_path, user_dir)
    }

    #[test]
    #[serial_test::serial]
    fn canonical_env_names_resolve() {
        let (_tmp, root_path, user_dir) = workspace_fixture();
        let _root = EnvRestore::set("CHUTES_ROOT", root_path.to_str().unwrap());
        let _user = EnvRestore::set("CHUTES_USER", "alice");
        let _legacy_root = EnvRestore::unset("XAI_ROOT");
        let _legacy_user = EnvRestore::unset("XAI_USER");
        assert_eq!(optional_workspace_user_dir(), Some(user_dir));
    }

    #[test]
    #[serial_test::serial]
    fn legacy_env_names_still_resolve() {
        let (_tmp, root_path, user_dir) = workspace_fixture();
        let _root = EnvRestore::unset("CHUTES_ROOT");
        let _user = EnvRestore::unset("CHUTES_USER");
        let _legacy_root = EnvRestore::set("XAI_ROOT", root_path.to_str().unwrap());
        let _legacy_user = EnvRestore::set("XAI_USER", "alice");
        assert_eq!(optional_workspace_user_dir(), Some(user_dir));
    }

    #[test]
    #[serial_test::serial]
    fn canonical_env_names_win_over_legacy() {
        let (_tmp, root_path, user_dir) = workspace_fixture();
        let _root = EnvRestore::set("CHUTES_ROOT", root_path.to_str().unwrap());
        let _user = EnvRestore::set("CHUTES_USER", "alice");
        let _legacy_root = EnvRestore::set("XAI_ROOT", "/nonexistent-legacy-root");
        let _legacy_user = EnvRestore::set("XAI_USER", "nobody");
        assert_eq!(optional_workspace_user_dir(), Some(user_dir));
    }

    #[test]
    fn bare_username_does_not_resolve_to_same_named_root_dir() {
        // Prefer the nested layout even when a same-named directory exists at the workspace root
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("alice")).unwrap();
        let user_dir = root.join("x").join("alice");
        fs::create_dir_all(&user_dir).unwrap();

        let rel = workspace_user_relpath("alice");
        let resolved = resolve_workspace_user_dir(root.to_str().unwrap(), &rel);
        assert_eq!(resolved, Some(user_dir));
        assert_ne!(
            resolved.as_deref(),
            Some(root.join("alice").as_path()),
            "must not resolve to a same-named directory at the workspace root"
        );
    }
}
