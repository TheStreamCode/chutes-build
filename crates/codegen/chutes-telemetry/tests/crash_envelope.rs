use chutes_telemetry::sentry::{self, Config};

#[test]
fn crash_envelope_omits_the_secret() {
    let sink = chutes_test_support::EnvelopeSink::start();
    unsafe { std::env::set_var("SENTRY_DSN", sink.dsn()) };
    let _guard = sentry::init(Config {
        client: "chutes-pager",
        client_version: "test",
        release: "test",
        disabled: false,
    });
    ::sentry::capture_message(
        "conformance-crash-marker token=longvalue123",
        ::sentry::Level::Error,
    );
    sentry::flush_on_shutdown();
    let joined = sink.bodies().join("\n");
    assert!(
        joined.contains("conformance-crash-marker"),
        "envelope missing the marker: {joined}"
    );
    assert!(
        !joined.contains("token=longvalue123"),
        "secret reached the envelope: {joined}"
    );
    assert!(
        joined.contains("[REDACTED_SECRET]"),
        "scrubbed stand-in missing: {joined}"
    );
}
