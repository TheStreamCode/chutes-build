//! How the workspace-server daemonizes itself.
//!
//! Used only by the `chutes-workspace-server` binary and never by the `chutes-workspace` library:
//!
//! - [`daemonize`] takes the process into its own session (double-fork and `setsid()` on Unix, stdio redirection on Windows).
//!   It also holds the single-instance pidfile lock.
//!
//! This crate deliberately does not depend on `chutes-workspace`.

#![deny(clippy::indexing_slicing)]

pub mod daemonize;
