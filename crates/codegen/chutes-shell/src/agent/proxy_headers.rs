use indexmap::IndexMap;
/// Every authenticated tool request (web search, image gen, and any future tools) carries client-identity headers.
/// Headers injected: `x-chutes-build-version` (uses `client_version` when provided, otherwise the CLI `CARGO_PKG_VERSION`) and `x-chutes-build-identifier`.
/// Existing entries are never overwritten so callers can pre-set a value.
pub(crate) fn inject_proxy_headers(
    headers: &mut IndexMap<String, String>,
    client_version: Option<&str>,
    alpha_test_key: Option<&str>,
    base_url: &str,
) {
    headers
        .entry("x-chutes-build-version".to_string())
        .or_insert_with(|| {
            client_version
                .map(String::from)
                .unwrap_or_else(|| chutes_version::VERSION.to_string())
        });
    headers
        .entry("x-chutes-build-identifier".to_string())
        .or_insert_with(crate::http::process_client_identifier);
    let _ = (alpha_test_key, base_url);
}
