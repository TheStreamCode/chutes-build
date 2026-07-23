use serde_json::json;
use xai_grok_tools::computer::local::{LocalFs, LocalTerminalBackend};
use xai_grok_tools::computer::types::{AsyncFileSystem, TerminalBackend};
use xai_grok_tools::notification::ToolNotificationHandle;
use xai_grok_tools::registry::types::{SessionContext, ToolConfig, ToolServerConfig};

#[test]
fn web_search_runtime_config_never_forwards_inference_credentials() {
    assert!(matches!(
        super::spawn::runtime_web_search_config(false),
        xai_grok_tools::implementations::web_search::WebSearchConfig::Native
    ));
    assert!(matches!(
        super::spawn::runtime_web_search_config(true),
        xai_grok_tools::implementations::web_search::WebSearchConfig::Disabled
    ));
}

#[tokio::test]
async fn web_search_errors_when_configured_model_cannot_be_resolved() {
    let builder = crate::tools::bridge::ToolBridge::get_builder();
    let config = ToolServerConfig {
        tools: vec![ToolConfig {
            id: "ChutesBuild:web_search".into(),
            params: None,
            name_override: None,
            params_name_overrides: None,
            description_override: None,
            behavior_version: None,
            kind: None,
        }],
        behavior_preset: None,
    };
    let fs: std::sync::Arc<dyn AsyncFileSystem> = std::sync::Arc::new(LocalFs);
    let terminal: std::sync::Arc<dyn TerminalBackend> =
        std::sync::Arc::new(LocalTerminalBackend::new());
    let ctx = SessionContext {
        backend: terminal,
        fs,
        cwd: std::env::temp_dir(),
        session_folder: std::env::temp_dir().join("grok-web-search-disabled"),
        session_env: std::sync::Arc::new(std::collections::HashMap::new()),
        notification_handle: ToolNotificationHandle::noop(),
        owner_session_id: None,
        parent_scheduler_handle: None,
        skills: vec![],
        state_path: std::env::temp_dir().join("grok-web-search-disabled/state.json"),
        memory_backend: None,
        web_search_config: xai_grok_tools::implementations::web_search::WebSearchConfig::Disabled,
        web_fetch_config: Default::default(),
        lsp: None,
        image_gen_config: Default::default(),
        video_gen_config: Default::default(),
        app_builder_deployer_config: Default::default(),
        api_key_provider: None,
        auth_provider: None,
        attribution_callback: None,
        system_reminder_tag: xai_grok_tools::reminders::DEFAULT_REMINDER_TAG,
    };
    let bridge = crate::tools::bridge::ToolBridge::finalize_builder(builder, config, ctx)
        .await
        .expect("finalize_builder should succeed");
    let result = bridge
        .call(
            "web_search",
            json!({
                "query": "test query"
            }),
            "web-search-disabled",
        )
        .await;
    assert!(result.is_err(), "web_search should fail when disabled");
}
