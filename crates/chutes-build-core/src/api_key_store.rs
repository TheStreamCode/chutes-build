//! Local persistence for a user-entered Chutes API key.
//!
//! This is the interactive-setup counterpart to `CHUTES_API_KEY`: the welcome
//! screen and `chutes-build login` validate a pasted key and save it here so
//! the next launch just works. It is NOT a login session store (there are no
//! sessions in this build): one file holding the user's own key, nothing else.
//!
//! Precedence stays explicit: per-model `api_key`/`env_key`, then
//! `CHUTES_API_KEY` (`CHUTES_BUILD_API_KEY` legacy), then this file. Callers
//! implement that by injecting the file's key into the process environment
//! only when no env key is set ([`ensure_env_from_store`]).

use std::io;
use std::path::{Path, PathBuf};

/// File name under the Chutes home directory (`~/.chutes-build/`).
pub const API_KEY_FILE_NAME: &str = "api_key";

pub fn key_path(home: &Path) -> PathBuf {
    home.join(API_KEY_FILE_NAME)
}

/// Load the saved key, trimmed. `None` when the file is missing, unreadable,
/// or blank — all three mean "no saved key", never an error to the user.
pub fn load(home: &Path) -> Option<String> {
    let content = std::fs::read_to_string(key_path(home)).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}

/// Save (or replace) the key. Blank input is rejected, never written.
/// The file is created owner-only on unix; on Windows it inherits the user
/// profile ACL.
pub fn save(home: &Path, key: &str) -> io::Result<()> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the API key is empty",
        ));
    }
    std::fs::create_dir_all(home)?;
    write_private(&key_path(home), trimmed)?;
    Ok(())
}

/// Delete the saved key. Returns `true` when a file was removed.
pub fn remove(home: &Path) -> io::Result<bool> {
    match std::fs::remove_file(key_path(home)) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Make the saved key live for this process when no env key is set.
/// Returns `true` when a key was injected. Reads env first so an explicit
/// `CHUTES_API_KEY` always wins over the file.
pub fn ensure_env_from_store(home: &Path) -> bool {
    if std::env::var_os("CHUTES_API_KEY").is_some_and(|value| !value.is_empty())
        || std::env::var_os("CHUTES_BUILD_API_KEY").is_some_and(|value| !value.is_empty())
    {
        return false;
    }
    let Some(key) = load(home) else {
        return false;
    };
    // SAFETY: callers run this at startup before worker threads spawn; the TUI
    // accept path runs it on the UI thread while no other thread touches this
    // variable (the key is only ever read afterwards). Same pattern as the
    // test `EnvGuard` usage across the workspace.
    unsafe {
        std::env::set_var("CHUTES_API_KEY", key);
    }
    true
}

#[cfg(unix)]
fn write_private(path: &Path, content: &str) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(0o600);
    let mut file = options.open(path)?;
    use std::io::Write as _;
    file.write_all(content.as_bytes())?;
    file.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &Path, content: &str) -> io::Result<()> {
    std::fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_trims_and_overwrites() {
        let home = tempfile::tempdir().expect("tempdir");
        assert_eq!(load(home.path()), None);
        save(home.path(), "  cpk-test-key\n").expect("save");
        assert_eq!(load(home.path()).as_deref(), Some("cpk-test-key"));
        save(home.path(), "cpk-second").expect("overwrite");
        assert_eq!(load(home.path()).as_deref(), Some("cpk-second"));
    }

    #[test]
    fn blank_key_is_rejected_and_never_written() {
        let home = tempfile::tempdir().expect("tempdir");
        assert!(save(home.path(), "  \n ").is_err());
        assert!(!key_path(home.path()).exists());
        assert_eq!(load(home.path()), None);
    }

    #[test]
    fn blank_file_loads_as_missing() {
        let home = tempfile::tempdir().expect("tempdir");
        std::fs::write(key_path(home.path()), " \n").expect("write");
        assert_eq!(load(home.path()), None);
    }

    #[test]
    fn remove_reports_whether_a_file_existed() {
        let home = tempfile::tempdir().expect("tempdir");
        assert!(!remove(home.path()).expect("remove"));
        save(home.path(), "cpk-test-key").expect("save");
        assert!(remove(home.path()).expect("remove"));
        assert_eq!(load(home.path()), None);
    }

    #[cfg(unix)]
    #[test]
    fn saved_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let home = tempfile::tempdir().expect("tempdir");
        save(home.path(), "cpk-test-key").expect("save");
        let mode = std::fs::metadata(key_path(home.path()))
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "mode must be 0600, got {mode:o}");
    }
}
