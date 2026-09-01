//! `chutes-build update` is a recovery command: a config failure must not block it.
//!
//! Hermetic: a local server serves the binary's own version as the channel
//! pointer, so a healthy run exits 0 ("already up to date") and a corrupt
//! config must too ÔÇö reintroducing a config `?` fails exactly that run.
//! The pointer must equal the current version: the installer converges in
//! both directions, so an older pointer triggers a downgrade attempt.

use std::io::{Read, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Resolve the pager binary like the PTY harness: `PAGER_BINARY` under
/// Bazel (runfiles-relative), else the PTY harness resolution order
/// (CARGO_BIN_EXE, then a local build of the composition-root bin).
fn pager_binary() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("PAGER_BINARY") {
        return std::path::absolute(&p)
            .unwrap_or_else(|e| panic!("failed to absolutize PAGER_BINARY {p}: {e}"));
    }
    xai_grok_pager_pty_harness::env::pager_binary()
        .unwrap_or_else(|e| panic!("failed to resolve the pager binary: {e}"))
}

/// Local base answering every request with the channel pointer body.
fn spawn_pointer_server(body: Arc<Mutex<String>>) -> (std::net::TcpListener, String) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let serving = listener.try_clone().unwrap();
    std::thread::spawn(move || {
        for stream in serving.incoming() {
            let Ok(stream) = stream else { return };
            // Drain the FULL request (headers + Content-Length body) before
            // answering: a client that is still writing when the server
            // closes the read side observes the response as a transport
            // error instead of a 200 (same shape as the compaction raw mock).
            let body = body.clone();
            std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(stream);
                use std::io::BufRead;
                let mut content_length = 0usize;
                loop {
                    let mut line = String::new();
                    let read = reader.read_line(&mut line).unwrap_or(0);
                    if read == 0 || line == "\r\n" || line == "\n" {
                        break;
                    }
                    let lower = line.to_ascii_lowercase();
                    if let Some(value) = lower.strip_prefix("content-length:")
                        && let Ok(v) = value.trim().parse::<usize>()
                    {
                        content_length = v;
                    }
                }
                let mut body_bytes = vec![0u8; content_length];
                if content_length > 0 {
                    use std::io::Read;
                    let _ = reader.read_exact(&mut body_bytes);
                }
                let version = body.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let mut stream = reader.into_inner();
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        version.len(),
                        version
                    )
                    .as_bytes(),
                );
            });
        }
    });
    (listener, base)
}

/// Run `chutes-build update` in an isolated home against the local pointer base.
fn run_update(base: &str, config_toml: &str, extra_args: &[&str]) -> std::process::Output {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("config.toml"), config_toml).unwrap();
    Command::new(pager_binary())
        .arg("update")
        .args(extra_args)
        .env_clear()
        .env("HOME", home.path())
        .env("CHUTES_BUILD_HOME", home.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("CHUTES_BUILD_CLI_BASE_URL", base)
        .output()
        .expect("spawn chutes-build update")
}

/// The valid run proves the environment resolves to success, so a nonzero
/// corrupt run can only mean a config failure aborted the update.
#[test]
fn corrupt_config_never_changes_update_outcome() {
    let body = Arc::new(Mutex::new("0.0.1".to_owned()));
    let (_listener, base) = spawn_pointer_server(body.clone());

    // Probe the binary's own version so the pointer matches it exactly.
    let check = run_update(&base, "[cli]\n", &["--check", "--json"]);
    let status: serde_json::Value = serde_json::from_slice(&check.stdout)
        .unwrap_or_else(|e| panic!("update --check --json must emit JSON: {e}"));
    let current = status["currentVersion"]
        .as_str()
        .expect("currentVersion in update --check --json")
        .to_owned();
    *body.lock().unwrap_or_else(|e| e.into_inner()) = current;

    let valid = run_update(&base, "[cli]\n", &[]);
    assert!(
        valid.status.success(),
        "healthy chutes-build update against the local base must exit 0\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&valid.stdout),
        String::from_utf8_lossy(&valid.stderr)
    );

    let corrupt = run_update(&base, "this is not toml {{{[[[", &[]);
    assert!(
        corrupt.status.success(),
        "a corrupt config.toml must not block chutes-build update\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&corrupt.stdout),
        String::from_utf8_lossy(&corrupt.stderr)
    );
}
