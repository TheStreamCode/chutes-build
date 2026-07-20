//! `CHUTES_BUILD_HOME` override tests in an isolated binary so `grok_home()`'s
//! process-wide `OnceLock` initializes from the overridden env var.

use std::path::PathBuf;

#[test]
fn grok_home_override_path_helpers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let grok_home = tmp.path().to_path_buf();
    unsafe {
        std::env::set_var("CHUTES_BUILD_HOME", &grok_home);
    }

    assert_eq!(
        xai_grok_pager::util::pager_toml_path(),
        grok_home.join("pager.toml")
    );
    assert_eq!(
        xai_grok_pager::util::display_grok_home_prefix(),
        "$CHUTES_BUILD_HOME"
    );
    assert_eq!(
        xai_grok_pager::util::display_user_grok_path("config.toml"),
        "$CHUTES_BUILD_HOME/config.toml"
    );

    let memory_path = grok_home.join("memory/memories.md");
    assert_eq!(
        xai_grok_pager::util::abbreviate_path(&memory_path.display().to_string()),
        "$CHUTES_BUILD_HOME/memory/memories.md"
    );

    assert!(xai_grok_pager::util::is_under_user_grok_home(&memory_path));
    assert!(!xai_grok_pager::util::is_under_user_grok_home(
        PathBuf::from("/tmp/other").as_path()
    ));
}
