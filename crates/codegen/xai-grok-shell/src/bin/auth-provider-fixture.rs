//! A stand-in credential helper for the auth-provider tests.
//!
//! A configured provider is a *command*, and off Unix that command runs through
//! `cmd /C` — `util::subprocess::shell_c` picks `cmd` deliberately, because the
//! provider contract is "exit 0 means success" and PowerShell's `-Command` does not
//! propagate a child's exit code. The fixtures were written as POSIX one-liners
//! (`printf`, `sleep`, `$(…)`, `${VAR:-0}`), none of which `cmd` has, so every test
//! resting on one failed on Windows and the whole suite had never run there.
//!
//! Writing each fixture twice, once per dialect, would encode the problem rather
//! than remove it: `cmd` has no `sleep`, no `printf`, no default-valued expansion,
//! and quoting JSON through `cmd /C` is its own small nightmare. This binary is the
//! other answer — the tests point `command` at it and pass `args`, which takes the
//! direct-exec branch and no shell at all. Deterministic everywhere, and it cannot
//! drift from what the shells happen to provide.
//!
//! Tests locate it beside the running test binary — `CARGO_BIN_EXE_*` is set for
//! integration tests and benches, not for a lib's own unit tests, and
//! `cargo test --lib` does not build a crate's binaries. So it has to be built
//! first; `auth_provider::test_provider_fixture_bin` says the command when it is
//! missing, and the gate in `AGENTS.md` runs it.
//!
//! The paths that take a command and no `args` still go through a shell, and there
//! the command names this binary bare — `cmd /C` strips the first and last quote of
//! the whole string, so quoting the program and its arguments comes apart in its
//! hands, while bare words mean the same to both shells.
//!
//! ```text
//! auth-provider-fixture print <text>              write <text>
//! auth-provider-fixture count <path> [delay_ms]   append a line, then write tok-<lines>
//! auth-provider-fixture count-fail-after-first <path>   tok-1 once, then exit 1
//! auth-provider-fixture sleep <ms> [text]         wait, then write [text]
//! auth-provider-fixture env [prefix] <VAR> <dflt>  write [prefix] + $VAR, or <dflt>
//! auth-provider-fixture exit <code> [text]        write [text], exit <code>
//! ```
//!
//! Output carries no trailing newline, so a test can assert an exact token without
//! relying on the provider's trim.

use std::io::Write as _;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();

    let (text, code) = match argv.as_slice() {
        ["print", text] => ((*text).to_owned(), 0),

        // The counting provider: successive runs answer tok-1, tok-2, … so a test
        // can tell a cached token from a fresh mint. The optional delay lets a test
        // hold a mint open long enough for a second caller to arrive on it.
        ["count", path] => (format!("tok-{}", record_run(path)), 0),
        ["count", path, delay_ms] => {
            std::thread::sleep(std::time::Duration::from_millis(parse(delay_ms, "count")));
            (format!("tok-{}", record_run(path)), 0)
        }

        // Records the run but answers a fixed payload, for the cases that assert on
        // an expiry the token itself carries rather than on which run this is.
        ["count-print", path, text] => {
            record_run(path);
            ((*text).to_owned(), 0)
        }

        // Succeeds once, then fails: a 401 that arrives after the only good mint
        // must invalidate the cached token rather than serve it again.
        ["count-fail-after-first", path] => match record_run(path) {
            1 => ("tok-1".to_owned(), 0),
            _ => (String::new(), 1),
        },

        // Milliseconds, not seconds: a timeout case should not cost a test suite
        // twenty real seconds to prove the timeout fires.
        ["sleep", ms] => {
            std::thread::sleep(std::time::Duration::from_millis(parse(ms, "sleep")));
            (String::new(), 0)
        }
        ["sleep", ms, text] => {
            std::thread::sleep(std::time::Duration::from_millis(parse(ms, "sleep")));
            ((*text).to_owned(), 0)
        }

        // The provider is handed context through the environment; this reports what
        // arrived, with a default so "unset" and "set to the default" stay distinct
        // at the call site rather than here.
        ["env", name, default] => (
            std::env::var(name).unwrap_or_else(|_| (*default).to_owned()),
            0,
        ),
        ["env", prefix, name, default] => {
            let value = std::env::var(name).unwrap_or_else(|_| (*default).to_owned());
            (format!("{prefix}{value}"), 0)
        }

        // A lot of stderr, then a token: the CLI path has to inherit stderr rather
        // than pipe it, or a helper that says this much deadlocks against a pipe
        // nobody is draining.
        ["stderr", bytes, text] => {
            let n = usize::try_from(parse(bytes, "stderr")).unwrap_or(usize::MAX);
            // Not `eprint!`, which panics if the write fails — and the dev profile
            // sets `panic = "abort"`, so the caller would see a fastfail exit code
            // instead of the token, and read it as the deadlock it was testing for.
            let mut err = std::io::stderr();
            let _ = err.write_all("x".repeat(n).as_bytes());
            let _ = err.flush();
            ((*text).to_owned(), 0)
        }

        // Refuses while the environment says the credential is expired, mints
        // otherwise: a provider that only knows how to sign in interactively.
        ["gate", name, value, text] => {
            if std::env::var(name).as_deref() == Ok(*value) {
                (String::new(), 1)
            } else {
                ((*text).to_owned(), 0)
            }
        }

        // More stdout than the mint will accept, for the cap that keeps a runaway
        // helper from exhausting memory or putting a huge token on the wire.
        ["flood", bytes] => (
            "x".repeat(usize::try_from(parse(bytes, "flood")).unwrap_or(usize::MAX)),
            0,
        ),

        ["exit", code] => (String::new(), parse(code, "exit") as i32),
        ["exit", code, text] => ((*text).to_owned(), parse(code, "exit") as i32),

        other => {
            eprintln!("auth-provider-fixture: unknown invocation {other:?}");
            return std::process::ExitCode::from(2);
        }
    };

    if !text.is_empty() {
        print!("{text}");
        let _ = std::io::stdout().flush();
    }
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// Append one line to `path` and return how many runs it now records.
fn record_run(path: &str) -> usize {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap_or_else(|e| panic!("open {path}: {e}"));
    writeln!(file, "run").unwrap_or_else(|e| panic!("append {path}: {e}"));
    drop(file);
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .count()
}

fn parse(value: &str, what: &str) -> u64 {
    value
        .parse()
        .unwrap_or_else(|_| panic!("auth-provider-fixture: {what} wants a number, got {value:?}"))
}
