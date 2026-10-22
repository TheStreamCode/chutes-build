//! Regression: `chutes-build plugin marketplace remove` must fail closed when the config-init flock
//! cannot be acquired — proceeding unlocked is the race the flock exists to prevent.

#[test]
fn cli_marketplace_remove_fails_closed_when_init_flock_held() {
    // One #[test] per binary: the env is process-global.
    let chutes_home = tempfile::tempdir().expect("Chutes Build home");
    // SAFETY: no other threads are running yet.
    unsafe { std::env::set_var("CHUTES_BUILD_HOME", chutes_home.path()) };

    let config_path = chutes_home.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[[marketplace.sources]]\nname = \"a\"\ngit = \"https://example.com/a.git\"\n",
    )
    .unwrap();

    let _flock =
        chutes_shell::util::config::acquire_init_lock(chutes_home.path()).expect("init flock");

    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(chutes_pager::plugin_cmd::run(
            chutes_pager::plugin_cmd::PluginArgs {
                command: chutes_pager::plugin_cmd::PluginCommand::Marketplace(
                    chutes_pager::plugin_cmd::MarketplaceArgs {
                        command: chutes_pager::plugin_cmd::MarketplaceCommand::Remove {
                            source: "https://example.com/a.git".into(),
                        },
                    },
                ),
            },
        ));

    assert!(
        result.is_err(),
        "remove must fail closed while the config-init flock is held"
    );
    assert!(
        std::fs::read_to_string(&config_path)
            .unwrap()
            .contains("example.com/a.git"),
        "a refused remove must leave the source configured"
    );
}
