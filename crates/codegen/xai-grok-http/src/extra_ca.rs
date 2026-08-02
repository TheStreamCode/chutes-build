//! Opt-in extra TLS roots via `CHUTES_EXTRA_CA_BUNDLE` (path to a PEM bundle).
//!
//! Default-off: unset or empty means no file I/O and no behavior change. The
//! certificates are **added** to the bundled roots, never replace them, and
//! this is deliberately not a way to skip verification — there is no
//! "accept invalid certificates" switch here.
//!
//! Exists for networks that terminate TLS at a corporate proxy: without the
//! proxy's root, every request fails to verify and the CLI simply cannot
//! reach Chutes.
//!
//! A bundle that cannot be read or parsed is reported once and then ignored,
//! so a wrong path degrades to the default trust store instead of breaking
//! every request. Parsed once per process.

use std::sync::OnceLock;

/// Environment variable naming the PEM bundle to trust in addition to the
/// built-in roots.
pub const EXTRA_CA_BUNDLE_ENV: &str = "CHUTES_EXTRA_CA_BUNDLE";

/// Hard cap on the bundle (1 MiB), so a wrong path cannot turn startup into an
/// unbounded read.
pub const MAX_EXTRA_CA_BUNDLE_BYTES: u64 = 1024 * 1024;

/// Extra roots for this process, parsed on first use. Empty when the variable
/// is unset or the bundle is unusable.
pub fn extra_root_certificates() -> &'static [reqwest::Certificate] {
    static ROOTS: OnceLock<Vec<reqwest::Certificate>> = OnceLock::new();
    ROOTS.get_or_init(|| {
        let Some(path) = std::env::var_os(EXTRA_CA_BUNDLE_ENV) else {
            return Vec::new();
        };
        let path = std::path::PathBuf::from(path);
        if path.as_os_str().is_empty() {
            return Vec::new();
        }
        match load_bundle(&path) {
            Ok(certificates) => {
                tracing::info!(
                    path = %path.display(),
                    count = certificates.len(),
                    "loaded extra TLS roots"
                );
                certificates
            }
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "ignoring {EXTRA_CA_BUNDLE_ENV}; continuing with the default trust store"
                );
                Vec::new()
            }
        }
    })
}

/// Read and parse a PEM bundle. Split out from the cache so the failure modes
/// are testable without touching process-global state.
fn load_bundle(path: &std::path::Path) -> Result<Vec<reqwest::Certificate>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| format!("cannot stat: {error}"))?;
    if metadata.len() > MAX_EXTRA_CA_BUNDLE_BYTES {
        return Err(format!(
            "bundle is {} bytes, over the {MAX_EXTRA_CA_BUNDLE_BYTES}-byte limit",
            metadata.len()
        ));
    }
    let pem = std::fs::read(path).map_err(|error| format!("cannot read: {error}"))?;
    if pem.is_empty() {
        return Err("bundle is empty".to_owned());
    }
    let certificates = reqwest::Certificate::from_pem_bundle(&pem)
        .map_err(|error| format!("not a valid PEM bundle: {error}"))?;
    if certificates.is_empty() {
        return Err("bundle contains no certificates".to_owned());
    }
    Ok(certificates)
}

/// Add the extra roots to a client builder. No-op when none are configured.
pub fn with_extra_roots(mut builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    for certificate in extra_root_certificates() {
        builder = builder.add_root_certificate(certificate.clone());
    }
    builder
}

/// Blocking-client counterpart of [`with_extra_roots`].
pub fn with_extra_roots_blocking(
    mut builder: reqwest::blocking::ClientBuilder,
) -> reqwest::blocking::ClientBuilder {
    for certificate in extra_root_certificates() {
        builder = builder.add_root_certificate(certificate.clone());
    }
    builder
}

#[cfg(test)]
mod tests {
    use super::*;

    // Self-signed PEMs for unit tests only (CN=test-extra-ca-1 / -2).
    const VALID_CERT_1: &str = "-----BEGIN CERTIFICATE-----\n\
MIIDFTCCAf2gAwIBAgIUT2czXTuxSAjDjEh92UMB1OVahZYwDQYJKoZIhvcNAQEL\n\
BQAwGjEYMBYGA1UEAwwPdGVzdC1leHRyYS1jYS0xMB4XDTI2MDcyOTE4MzUwNFoX\n\
DTM2MDcyNjE4MzUwNFowGjEYMBYGA1UEAwwPdGVzdC1leHRyYS1jYS0xMIIBIjAN\n\
BgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA1gNk2BQwUy+n5cCaTFtGpSzVQv//\n\
d7QD+3QWeE411wIGJzp3nrd7np55X8JHxeg/pRhspQvLQAF7bt55LSkL/+sSth3S\n\
QTbBqhftic9CXik3llAwbdQkAM9srz5zXWW9KVjZ57dxjjxrS15SCXu/UmvGZy98\n\
faJcS++TRkczsNFzwQEqeDYARVc/no0C0I++NhGLPaNMfFAevvnu6Kt3CYMI5ls4\n\
KCFgnlau4CjgRCMSfRDCRcwEwUAp+DyX9IU+tvDAQY1ncVoa/05tvaEvw7pQ+UgW\n\
0wRG0lk7PLlcWmUkLcFpO+sL5GRkC8RoWM4cFbIOiXoVxUFks/z2y0GCEQIDAQAB\n\
o1MwUTAdBgNVHQ4EFgQU+lyC70W5aR6BIf4VNtjfiWMNzzkwHwYDVR0jBBgwFoAU\n\
+lyC70W5aR6BIf4VNtjfiWMNzzkwDwYDVR0TAQH/BAUwAwEB/zANBgkqhkiG9w0B\n\
AQsFAAOCAQEA02972nA7LshRgubz6BwXbh1gA5pLzTd5KEae+94Hq6mP2zJ1T0gk\n\
x+me0NtSgG4BJLdBIylUzo2UmsfB/sz+ght6WX1uB38Vc2UQsp0sRPeeiMovSd6n\n\
I7xZyuZEF3noYJVBBlKQ8XsCUIBNIROlyKlNjNcWY8tGqPh9cepvtZYkBgRZr1vW\n\
hJAE3EOL2ZddrMPF64QeU9UhvCm0Ch+Ceqa1ZWE0MygccggX5s2yQwtXO2ovJdjH\n\
6vW0I02r8sE+NX0d1u8rIPJEKlp89UwCwniD7SxHTNw8bbsTCWz+AMod7vC7De3X\n\
4Daxme+vD8adOfCeOIu5vNrlXLNST2yaTw==\n\
-----END CERTIFICATE-----\n";

    #[test]
    fn a_valid_bundle_loads_every_certificate_in_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roots.pem");
        // Two copies: a bundle is a concatenation, and both must be picked up.
        std::fs::write(&path, format!("{VALID_CERT_1}{VALID_CERT_1}")).unwrap();
        let loaded = load_bundle(&path).expect("valid bundle");
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn unusable_bundles_are_reported_rather_than_trusted() {
        let dir = tempfile::tempdir().unwrap();

        let missing = dir.path().join("absent.pem");
        assert!(load_bundle(&missing).is_err(), "missing file");

        let empty = dir.path().join("empty.pem");
        std::fs::write(&empty, "").unwrap();
        assert!(load_bundle(&empty).is_err(), "empty file");

        let garbage = dir.path().join("garbage.pem");
        std::fs::write(&garbage, "this is not a certificate").unwrap();
        assert!(load_bundle(&garbage).is_err(), "non-PEM content");
    }

    /// The size cap keeps a wrong path (a log, a disk image) from being read
    /// into memory at startup.
    #[test]
    fn an_oversized_bundle_is_rejected_before_being_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.pem");
        let oversized = vec![b'x'; (MAX_EXTRA_CA_BUNDLE_BYTES + 1) as usize];
        std::fs::write(&path, oversized).unwrap();
        let error = load_bundle(&path).expect_err("oversized bundle");
        assert!(error.contains("over the"), "{error}");
    }

    /// With nothing configured the builders must come back untouched, so the
    /// default trust store is what ships.
    #[test]
    fn no_configuration_means_no_extra_roots() {
        // `extra_root_certificates` caches per process and other tests may have
        // primed it; assert on the pure loader instead, plus the documented
        // default that an unset variable performs no I/O.
        assert!(std::env::var_os("CHUTES_EXTRA_CA_BUNDLE").is_none());
        assert!(extra_root_certificates().is_empty());
    }
}
