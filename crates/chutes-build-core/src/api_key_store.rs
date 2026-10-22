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

/// English, OS-locale-independent description of a failed key write.
///
/// `std::io::Error`'s `Display` embeds the operating system's localized message, so on an
/// Italian Windows the user-facing copy would read "Accesso negato. (os error 5)" in an
/// otherwise English product. User-facing callers format failures through this instead, and
/// get the path and the numeric code needed to act on the problem.
pub fn describe_write_failure(path: &Path, error: &io::Error) -> String {
    let phrase = match error.raw_os_error() {
        Some(5) | Some(32) | Some(33) => "permission denied",
        Some(2) | Some(3) => "path not found",
        Some(112) | Some(28) => "no space left on the device",
        Some(13) => "permission denied",
        _ => match error.kind() {
            io::ErrorKind::PermissionDenied => "permission denied",
            io::ErrorKind::NotFound => "path not found",
            io::ErrorKind::InvalidInput => "invalid path",
            _ => "I/O error",
        },
    };
    match error.raw_os_error() {
        Some(code) => {
            // Windows Controlled folder access treats an unsigned build as an untrusted
            // app and refuses writes under the profile ("permission denied" on a directory
            // the user can write). Name the fix, because "check that the directory is
            // writable" is misleading there.
            #[cfg(windows)]
            let hint = if phrase == "permission denied" {
                "; on Windows, Controlled folder access can block unsigned builds: \
                 allow chutes-build.exe in Windows Security, or point CHUTES_BUILD_HOME \
                 at a folder it does not protect"
            } else {
                "; check that the directory is writable"
            };
            #[cfg(not(windows))]
            let hint = "; check that the directory is writable";
            format!("{}: {} (os error {}){}", path.display(), phrase, code, hint)
        }
        None => format!(
            "{}: {}; check that the directory is writable",
            path.display(),
            phrase
        ),
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
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(0o600);
    let mut file = options.open(path)?;
    use std::io::Write as _;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    // `mode` applies at creation only: an overwrite of a pre-existing
    // world-readable file would keep its loose permissions without this.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
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

    #[test]
    fn write_failure_description_is_english_and_names_the_path() {
        let error = io::Error::from_raw_os_error(5);
        let text = describe_write_failure(Path::new("C:\\probe\\api_key"), &error);
        assert!(text.contains("permission denied"), "{text}");
        assert!(text.contains("os error 5"), "{text}");
        assert!(text.contains("api_key"), "{text}");
        assert!(
            !text.contains("Accesso negato") && !text.contains("Access is denied"),
            "the OS-localized Display must never reach user copy: {text}"
        );
        #[cfg(windows)]
        assert!(
            text.contains("Controlled folder access"),
            "Windows permission failures should name the controlled-folder-access fix: {text}"
        );
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

    #[cfg(unix)]
    #[test]
    fn overwrite_tightens_loose_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let home = tempfile::tempdir().expect("tempdir");
        let path = key_path(home.path());
        std::fs::create_dir_all(home.path()).expect("home");
        std::fs::write(&path, "cpk-old-key").expect("pre-existing file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("loosen permissions");
        save(home.path(), "cpk-new-key").expect("save");
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "overwrite must tighten to 0600, got {mode:o}");
        assert_eq!(load(home.path()).as_deref(), Some("cpk-new-key"));
    }
}
