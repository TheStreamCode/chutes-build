//! `CHUTES_BUILD_HOME` override tests in an isolated binary so `chutes_home()`'s process-wide `OnceLock` initializes from the overridden env var.

use std::path::PathBuf;

#[test]
#[serial_test::serial(CHUTES_BUILD_HOME)]
fn chutes_home_override_path_helpers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let chutes_home = tmp.path().to_path_buf();
    unsafe {
        std::env::set_var("CHUTES_BUILD_HOME", &chutes_home);
    }

    assert_eq!(
        chutes_pager::util::pager_toml_path(),
        chutes_home.join("pager.toml")
    );
    assert_eq!(
        chutes_pager::util::display_chutes_home_prefix(),
        "$CHUTES_BUILD_HOME"
    );
    assert_eq!(
        chutes_pager::util::display_user_chutes_path("config.toml"),
        "$CHUTES_BUILD_HOME/config.toml"
    );

    let memory_path = chutes_home.join("memory/MEMORY.md");
    assert_eq!(
        chutes_pager::util::abbreviate_path(&memory_path.display().to_string()),
        "$CHUTES_BUILD_HOME/memory/MEMORY.md"
    );

    // The copy toast abbreviates paths the same way, so a custom $CHUTES_BUILD_HOME outside $HOME still shows the short form
    assert_eq!(
        chutes_pager::clipboard::display_copy_path(&chutes_home.join("last-copy.txt")),
        "$CHUTES_BUILD_HOME/last-copy.txt"
    );

    assert!(chutes_pager::util::is_under_user_chutes_home(&memory_path));
    assert!(!chutes_pager::util::is_under_user_chutes_home(
        PathBuf::from("/tmp/other").as_path()
    ));
}

/// Isolated because `chutes_home()`'s `OnceLock` is already initialized by the time the shared lib-test binary reaches a case like this.
#[test]
#[serial_test::serial(CHUTES_BUILD_HOME)]
fn disk_usage_run_creates_no_chutes_home() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ghost = tmp.path().join("ghost-home");
    unsafe {
        std::env::set_var("CHUTES_BUILD_HOME", &ghost);
    }

    for json in [false, true] {
        chutes_pager::disk_usage_cmd::run(chutes_pager::disk_usage_cmd::DiskUsageArgs {
            json,
            clean: false,
            clean_orphaned: false,
            yes: false,
        })
        .expect("a missing home is not an error");
        assert!(
            !ghost.exists(),
            "chutes-build du must not create the home it reports on (json={json})"
        );
    }
}
