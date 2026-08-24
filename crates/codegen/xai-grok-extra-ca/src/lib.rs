//! Opt-in extra TLS roots via `CHUTES_EXTRA_CA_BUNDLE` (PEM path).
//!
//! Default-off (unset/empty env → no I/O); parsed once into a process
//! `OnceLock`; additive to webpki roots. Each DER is validated with
//! `rustls::RootCertStore::add` before caching so a bad bundle cannot fail
//! `ClientBuilder::build()`. Unreadable/oversized/empty/unparsable → warn and
//! continue. Size cap: [`MAX_EXTRA_CA_BUNDLE_BYTES`].
//!
//! Source of truth is validated DER ([`extra_root_ders`]) so reqwest 0.12
//! (this crate's adapters) and MCP's 0.13 can each build their own
//! `Certificate`s.

use std::io::Read;
use std::sync::OnceLock;

use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;

/// Hard cap on `CHUTES_EXTRA_CA_BUNDLE` (1 MiB) — avoids unbounded startup reads.
pub const MAX_EXTRA_CA_BUNDLE_BYTES: u64 = 1024 * 1024;

/// Env var name for the opt-in extra CA bundle (PEM path).
pub const ENV_CHUTES_BUILD_EXTRA_CA_BUNDLE: &str = "CHUTES_EXTRA_CA_BUNDLE";

/// Process-wide extra roots as validated DER, parsed once.
///
/// Empty when the env var is unset/empty or the file yields no usable certs.
pub fn extra_root_ders() -> &'static [Vec<u8>] {
    static DERS: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    DERS.get_or_init(load_extra_root_ders).as_slice()
}

/// Apply [`extra_root_ders`] to a workspace (reqwest 0.12) async `ClientBuilder`.
pub fn with_extra_root_certificates(mut builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    for der in extra_root_ders() {
        match reqwest::Certificate::from_der(der) {
            Ok(cert) => builder = builder.add_root_certificate(cert),
            // WHY: rustls already accepted this DER; skip rather than poison build.
            Err(e) => tracing::warn!(
                error = %e,
                "CHUTES_EXTRA_CA_BUNDLE: validated DER rejected by reqwest; skipping cert"
            ),
        }
    }
    builder
}

/// Apply [`extra_root_ders`] to a workspace (reqwest 0.12) blocking `ClientBuilder`.
pub fn with_extra_root_certificates_blocking(
    mut builder: reqwest::blocking::ClientBuilder,
) -> reqwest::blocking::ClientBuilder {
    for der in extra_root_ders() {
        match reqwest::Certificate::from_der(der) {
            Ok(cert) => builder = builder.add_root_certificate(cert),
            // WHY: rustls already accepted this DER; skip rather than poison build.
            Err(e) => tracing::warn!(
                error = %e,
                "CHUTES_EXTRA_CA_BUNDLE: validated DER rejected by reqwest; skipping cert"
            ),
        }
    }
    builder
}

/// Interim no-op placeholder for the upstream TLS-policy entry point.
/// With the current feature set, rustls falls back to its default process
/// provider, so there is nothing to install yet.
pub fn ensure_default_crypto_provider() {}

/// Configure and build an async client under the crate's root policy.
///
/// Interim compat surface for the 1.0.8 tool callers: applies
/// [`with_extra_root_certificates`], then hands the builder to `configure`
/// and builds. The fuller upstream rework (rustls backend pin, shared root
/// store, process crypto provider) is intentionally not ported yet.
pub fn build_reqwest_client(
    configure: impl FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
) -> Result<reqwest::Client, reqwest::Error> {
    configure(with_extra_root_certificates(reqwest::Client::builder())).build()
}

/// Blocking twin of [`build_reqwest_client`].
pub fn build_blocking_reqwest_client(
    configure: impl FnOnce(reqwest::blocking::ClientBuilder) -> reqwest::blocking::ClientBuilder,
) -> Result<reqwest::blocking::Client, reqwest::Error> {
    configure(with_extra_root_certificates_blocking(
        reqwest::blocking::Client::builder(),
    ))
    .build()
}

fn load_extra_root_ders() -> Vec<Vec<u8>> {
    let path = match std::env::var_os(ENV_CHUTES_BUILD_EXTRA_CA_BUNDLE) {
        Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => return Vec::new(),
    };

    let bytes = match read_bundle_capped(&path) {
        Ok(b) => b,
        Err(BundleReadError::Io(e)) => {
            // WHY: MITM CA is optional; a missing path must not brick HTTP.
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "CHUTES_EXTRA_CA_BUNDLE unreadable; continuing without extra roots"
            );
            return Vec::new();
        }
        Err(BundleReadError::TooLarge) => {
            tracing::warn!(
                path = %path.display(),
                max_bytes = MAX_EXTRA_CA_BUNDLE_BYTES,
                "CHUTES_EXTRA_CA_BUNDLE exceeds size cap; continuing without extra roots"
            );
            return Vec::new();
        }
    };

    let outcome = parse_and_validate_pem(&bytes);
    if outcome.no_pem_blocks {
        tracing::warn!(
            path = %path.display(),
            "CHUTES_EXTRA_CA_BUNDLE contains no PEM certificate blocks; continuing without extra roots"
        );
        return outcome.accepted;
    }
    if outcome.rejected > 0 {
        tracing::warn!(
            path = %path.display(),
            accepted = outcome.accepted.len(),
            rejected = outcome.rejected,
            "CHUTES_EXTRA_CA_BUNDLE: dropped unusable certificate block(s)"
        );
    }
    if outcome.accepted.is_empty() {
        tracing::warn!(
            path = %path.display(),
            "CHUTES_EXTRA_CA_BUNDLE produced zero usable certificates; continuing without extra roots"
        );
    } else {
        tracing::info!(
            path = %path.display(),
            accepted = outcome.accepted.len(),
            "CHUTES_EXTRA_CA_BUNDLE: loaded extra root certificate(s)"
        );
    }
    outcome.accepted
}

#[derive(Debug)]
enum BundleReadError {
    Io(std::io::Error),
    TooLarge,
}

fn read_bundle_capped(path: &std::path::Path) -> Result<Vec<u8>, BundleReadError> {
    let file = std::fs::File::open(path).map_err(BundleReadError::Io)?;
    let mut buf = Vec::new();
    let n = file
        .take(MAX_EXTRA_CA_BUNDLE_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(BundleReadError::Io)?;
    if (n as u64) > MAX_EXTRA_CA_BUNDLE_BYTES {
        return Err(BundleReadError::TooLarge);
    }
    Ok(buf)
}

/// Result of parsing a PEM bundle into rustls-validated DER roots.
#[derive(Debug, Default)]
pub(crate) struct ParseOutcome {
    pub(crate) accepted: Vec<Vec<u8>>,
    /// PEM blocks that failed decode or rustls X.509 validation.
    pub(crate) rejected: usize,
    /// Input (non-empty) contained no PEM certificate blocks at all.
    pub(crate) no_pem_blocks: bool,
}

/// Parse PEM into rustls-validated DER (no env / OnceLock). Input with no PEM
/// certificate blocks (including empty) → empty accepted, zero rejected,
/// `no_pem_blocks` set.
pub(crate) fn parse_and_validate_pem(pem: &[u8]) -> ParseOutcome {
    let mut accepted = Vec::new();
    let mut rejected = 0usize;
    let mut saw_block = false;

    // WHY: reject non-X.509 DER before any ClientBuilder sees it; `add`
    // validates per certificate, so one store serves the whole bundle.
    let mut store = RootCertStore::empty();
    for item in CertificateDer::pem_slice_iter(pem) {
        saw_block = true;
        match item {
            Ok(der) => match store.add(der.clone()) {
                Ok(()) => accepted.push(der.as_ref().to_vec()),
                Err(_) => rejected += 1,
            },
            Err(_) => rejected += 1,
        }
    }

    ParseOutcome {
        accepted,
        rejected,
        no_pem_blocks: !saw_block,
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
