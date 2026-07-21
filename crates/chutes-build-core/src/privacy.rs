//! Conservative secret filtering for persistent Markdown memory.
//!
//! Detection is a union of these memory-specific substring markers (kept for
//! their original recall on plain-English mentions like "my password is...")
//! and [`xai_grok_secrets::detect_probable_secret`], the same canonical
//! pattern set used for outbound Context7 queries (via this function) and
//! log/trace sanitization — so a secret shape caught in one place (e.g. an
//! AWS key or GitHub token) is caught everywhere, not just here.

const SECRET_MARKERS: &[&str] = &[
    "api_key",
    "apikey",
    "authorization:",
    "bearer ",
    "private_key",
    "client_secret",
    "access_token",
    "refresh_token",
    "password",
    "secret=",
    "cpk_",
    "sk-",
];

pub fn contains_probable_secret(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    SECRET_MARKERS.iter().any(|marker| lower.contains(marker))
        || xai_grok_secrets::detect_probable_secret(text)
}

pub fn filter_memory_markdown(text: &str) -> String {
    text.lines()
        .map(|line| {
            if contains_probable_secret(line) {
                "[redacted: probable secret]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_probable_api_key_line() {
        let filtered = filter_memory_markdown("keep this\nCHUTES_API_KEY=cpk_secret\nalso keep");
        assert_eq!(
            filtered,
            "keep this\n[redacted: probable secret]\nalso keep"
        );
    }

    /// Regression: before the union with `xai_grok_secrets`, none of these
    /// shapes matched any of the 12 substring markers, so an AWS key or
    /// GitHub token pasted into memory (or a Context7 query, which reuses
    /// this same function) would not have been caught.
    #[test]
    fn catches_secret_shapes_the_local_markers_alone_would_miss() {
        for (line, label) in [
            ("aws AKIAABCDEFGHIJKLMNOP", "aws access key"),
            (
                "token ghp_0123456789abcdefghijABCDEFGHIJ012345",
                "github pat",
            ),
            (
                "deployment key eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4f", // gitleaks:allow
                "bare jwt",
            ),
            ("AIzaSyD0123456789abcdefghijklmnopqrstuvw", "google api key"),
        ] {
            assert!(
                !SECRET_MARKERS
                    .iter()
                    .any(|marker| line.to_ascii_lowercase().contains(marker)),
                "test fixture invalid: {label} unexpectedly matches a local marker"
            );
            assert!(
                contains_probable_secret(line),
                "{label} not caught by the canonical detector: {line}"
            );
        }
    }

    #[test]
    fn plain_prose_mentioning_password_is_still_caught_by_local_markers() {
        // The original, memory-specific recall this module existed for:
        // conversational text has no regex-matchable secret *shape*, only
        // the word "password" itself.
        assert!(contains_probable_secret("my password is hunter2"));
    }
}
