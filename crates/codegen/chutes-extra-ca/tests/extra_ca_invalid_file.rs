// These tests exercise the wrapping itself; the builder under test ends in
// a .build() by design.
#![allow(clippy::disallowed_methods)]

//! Process-isolated: missing CHUTES_EXTRA_CA_BUNDLE path → fail-open client build.

#[test]
fn missing_bundle_path_builds_clients_without_panic() {
    // Safety: sole test in this binary; set before any OnceLock resolve.
    unsafe {
        std::env::set_var(
            chutes_extra_ca::ENV_CHUTES_BUILD_EXTRA_CA_BUNDLE,
            "/nonexistent/chutes-extra-ca-bundle-invalid-file.pem",
        );
    }

    assert!(chutes_extra_ca::extra_root_ders().is_empty());

    chutes_extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .build()
        .expect("async client builds when bundle is unreadable");

    chutes_extra_ca::with_extra_root_certificates_blocking(reqwest::blocking::Client::builder())
        .build()
        .expect("blocking client builds when bundle is unreadable");
}
