//! Regression: CLI `marketplace remove` of a source that lives only in a JSON
//! store removes it there and reports `MarketplaceSourceRemoval::JsonStore`.
//! `config.toml` gains no marketplace table as a side effect.

#[test]
fn cli_remove_of_json_store_only_source_removes_it() {
    // One #[test] per binary: the env is process-global.
    let chutes_home = tempfile::tempdir().expect("Chutes Build home");
    // SAFETY: no other threads are running yet.
    unsafe { std::env::set_var("CHUTES_BUILD_HOME", chutes_home.path()) };

    // Source known ONLY via the JSON store.
    let plugins_dir = chutes_home.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    let known_path = plugins_dir.join("known_marketplaces.json");
    std::fs::write(
        &known_path,
        r#"{"acme": {"source": {"source": "github", "repo": "acme-org/plugin-marketplace"}}}"#,
    )
    .unwrap();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(chutes_pager::plugin_cmd::run(
            chutes_pager::plugin_cmd::PluginArgs {
                command: chutes_pager::plugin_cmd::PluginCommand::Marketplace(
                    chutes_pager::plugin_cmd::MarketplaceArgs {
                        command: chutes_pager::plugin_cmd::MarketplaceCommand::Remove {
                            source: "https://github.com/acme-org/plugin-marketplace.git".into(),
                        },
                    },
                ),
            },
        ))
        .expect("remove succeeds");

    let known: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&known_path).unwrap()).unwrap();
    assert!(
        known.get("acme").is_none(),
        "source must be removed from the JSON store: {known}"
    );

    let config_path = chutes_home.path().join("config.toml");
    if config_path.exists() {
        let raw = std::fs::read_to_string(&config_path).unwrap();
        assert!(
            !raw.contains("[marketplace]"),
            "JSON-store removal must not create a [marketplace] table: {raw}"
        );
    }
}
