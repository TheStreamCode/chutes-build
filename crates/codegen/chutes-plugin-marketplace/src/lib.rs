//! Provides marketplace source configuration and plugin discovery, indexed with a filesystem fallback.
//! Install integration goes through the existing `InstallRegistry` pipeline.
//!
//! There is no official marketplace source: every configured source — a Git URL
//! or a local path, whatever its origin (Claude, Codex, or anything else)
//! — is treated uniformly. Resolution never prefers one source over another by
//! identity; ambiguous bare names must be qualified with `name@qualifier`.

#![deny(clippy::indexing_slicing)]

pub mod catalog;
pub mod config;
pub mod error;
pub mod git;
pub mod index;
pub mod install_resolve;
pub mod installer;
pub mod matcher;
pub mod scanner;
pub mod types;

pub use config::{
    env_require_sha, foreign_settings_roots, load_extra_sources_from_settings,
    load_extra_sources_from_settings_in, load_require_sha, load_sources, native_settings_roots,
};
pub use error::MarketplaceError;
pub use scanner::scan_marketplace;
pub use types::*;

/// Normalized lowercase `owner/repo` from a GitHub URL (HTTPS/http/ssh/scp, `www.`, trailing `.git`/`/`), or `None` if not a GitHub URL.
pub(crate) fn canonical_github_owner_repo(url: &str) -> Option<String> {
    let s = url.trim();
    let s = s.strip_suffix('/').unwrap_or(s);
    let s = s.strip_suffix(".git").unwrap_or(s);
    let lower = s.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .or_else(|| lower.strip_prefix("ssh://"))
        .unwrap_or(&lower);
    let rest = rest.strip_prefix("git@").unwrap_or(rest);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let owner_repo = rest
        .strip_prefix("github.com/")
        .or_else(|| rest.strip_prefix("github.com:"))?;
    if owner_repo.is_empty() {
        None
    } else {
        Some(owner_repo.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_owner_repo_matches_url_forms() {
        for url in [
            "https://github.com/acme-org/plugins.git",
            "https://github.com/acme-org/plugins",
            "git@github.com:acme-org/plugins.git",
            "git@github.com:acme-org/plugins",
            "ssh://git@github.com/acme-org/plugins.git",
            "ssh://git@github.com/acme-org/plugins",
            "https://GitHub.com/ACME-org/Plugins",
            "https://github.com/acme-org/plugins/",
            "https://github.com/acme-org/plugins.git/",
            "http://github.com/acme-org/plugins",
            "https://www.github.com/acme-org/plugins.git",
            "git@github.com:ACME-org/plugins.git",
        ] {
            assert_eq!(
                canonical_github_owner_repo(url).as_deref(),
                Some("acme-org/plugins"),
                "{url}"
            );
        }
    }

    #[test]
    fn canonical_owner_repo_rejects_non_github() {
        assert_eq!(canonical_github_owner_repo(""), None);
        assert_eq!(
            canonical_github_owner_repo("https://gitlab.com/acme-org/plugins.git"),
            None
        );
        assert_eq!(canonical_github_owner_repo("https://github.com/"), None);
    }
}
