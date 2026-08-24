//! Verifies that the gRPC-transport external OTEL stream is **deadened** in
//! Chutes Build: even with every env var set, no data can leave the process.

mod otlp_collector;

use otlp_collector as col;

#[test]
fn external_stream_grpc_is_deadened() {
    let collected = col::Collected::default();
    let endpoint = col::start_collector(collected.clone());

    let mut cfg = xai_grok_telemetry::external::ExternalOtelConfig::resolve_with(
        |name| match name {
            "CHUTES_BUILD_EXTERNAL_OTEL" => Some("1".into()),
            "OTEL_LOGS_EXPORTER" | "OTEL_METRICS_EXPORTER" => Some("otlp".into()),
            "OTEL_EXPORTER_OTLP_ENDPOINT" => Some(endpoint.clone()),
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

    xai_grok_telemetry::log_event(xai_grok_telemetry::events::SessionNew {
        session_id: "sess-grpc-deadened".into(),
        client_identifier: None,
        client_version: None,
        is_git_repo: true,
        permission_mode: xai_grok_telemetry::enums::PermissionMode::Ask,
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

    xai_grok_telemetry::external::shutdown();
}
