//! Verifies that the external OTEL stream is **deadened** in Chutes Build:
//! even with both content gates ON and every env var set, no data can leave
//! the process.

mod otlp_collector;

use otlp_collector as col;

const SECRET_KEY: &str = "sk-LEAKaaaaaaaaaaaaaaaa1234567890";
const CANARY_PROMPT: &str = "CANARY_PROMPT_TEXT do not export";

#[test]
fn external_stream_gates_on_is_deadened() {
    let collected = col::Collected::default();
    let endpoint = col::start_collector(collected.clone());

    let mut cfg = xai_grok_telemetry::external::ExternalOtelConfig::resolve_with(
        |name| match name {
            "CHUTES_BUILD_EXTERNAL_OTEL" => Some("1".into()),
            "OTEL_LOGS_EXPORTER" | "OTEL_METRICS_EXPORTER" => Some("otlp".into()),
            "OTEL_EXPORTER_OTLP_ENDPOINT" => Some(endpoint.clone()),
            // Both content gates ON.
            "OTEL_LOG_USER_PROMPTS" | "OTEL_LOG_TOOL_DETAILS" => Some("1".into()),
            "OTEL_METRIC_EXPORT_INTERVAL" => Some("200".into()),
            _ => None,
        },
        None,
    )
    .expect("double opt-in must resolve");
    cfg.client = xai_grok_telemetry::external::config::ExternalClientInfo {
        service_version: "0.0.0-test".into(),
        client_version: "0.0.0-test".into(),
        app_entrypoint: "cli".into(),
    };

    xai_grok_telemetry::external::init(Some(cfg));
    assert!(
        !xai_grok_telemetry::external::is_active(),
        "deadened stream must never report active"
    );

    xai_grok_telemetry::log_event(xai_grok_telemetry::events::PromptSubmitted {
        prompt_length: CANARY_PROMPT.len(),
        model_id: CANARY_PROMPT.into(),
        client_identifier: None,
        screen_mode: None,
        prompt_text: Some(CANARY_PROMPT.into()),
    });

    xai_grok_telemetry::external::flush();
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        collected.logs_len(),
        0,
        "deadened stream must not export logs"
    );
    assert_eq!(
        collected.metrics_len(),
        0,
        "deadened stream must not export metrics"
    );
    let raw = collected.raw_text();
    assert!(!raw.contains("CANARY"), "canary reached the wire");
    assert!(!raw.contains(SECRET_KEY), "secret reached the wire");

    xai_grok_telemetry::external::shutdown();
}
