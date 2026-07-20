# Security Review

Review date: 2026-07-19

This review covers the public Chutes Build source tree, local agent/runtime
boundaries, Chutes and third-party network clients, browser automation, media
artifacts, dependency policy, and npm release assembly. It does not certify
third-party model behavior, MCP servers, websites, or services selected by a
user.

## Release-blocking controls

- Chutes credentials use redacted debug output. Redirects are disabled on
  credential-bearing HTTP clients.
- Media invocation accepts only HTTPS DNS hosts under `chutes.ai`. External
  media downloads never receive Chutes credentials and reject embedded
  credentials, IP literals, and localhost hostnames.
- Generated media and browser screenshots resolve through canonical workspace
  ancestors, reject traversal and absolute output paths, and use create-new
  writes to prevent silent overwrite.
- Generated-media downloads default to a 512 MiB limit with a 2 GiB hard
  ceiling. Workspace media inputs default to 64 MiB with a 512 MiB hard
  ceiling.
- The browser uses an isolated temporary profile, a loopback DevTools endpoint
  with an exact origin, and password-value redaction in page snapshots.
- Account quota fallback work is capped at 100 chute IDs and eight concurrent
  requests. Model capability discovery has an endpoint-aware 60-second cache
  and a bounded request timeout.
- CI scans complete Git history with a version- and checksum-pinned Gitleaks
  binary. Cargo Deny evaluates advisories, licenses, duplicate dependencies,
  and package sources for every supported release target.
- Each native npm archive is built on its target architecture, executed with
  `--version`, accompanied by a SHA-256 sidecar, re-verified after artifact
  download, and assembled with the launcher for a final Linux smoke test.

## Dependency policy

Direct dependencies with available compatible security fixes are upgraded
before release. `deny.toml` is the machine-readable policy and records bounded
exceptions for transitive advisories that currently lack compatible maintained
replacements or do not reach the vulnerable operation in Chutes Build.

The accepted set currently consists of the RSA decryption timing advisory (the
crate is used here for JWT signature verification and test key generation),
unmaintained transitive crates in syntax highlighting, async retry, TUI,
desktop theme, ranking, and PDF font stacks, plus a build-only Quick XML version
used by Wayland protocol generation on Linux. These exceptions remain visible
in CI and must be revisited during dependency or upstream synchronization.

## Residual trust boundaries

Chutes Build is intentionally capable of executing commands, modifying files,
starting subagents, and controlling a browser after the applicable permission
decision. Repository instructions, model output, web content, downloaded
documents, plugins, and MCP responses are untrusted input. Users should retain
permission prompts for sensitive actions and avoid exposing credentials or
private source in prompts sent to external services.

Generated assets may be hosted by a Chutes-provided external CDN. Those HTTPS
downloads are credential-free, size-bounded, and do not follow redirects, but
the remote bytes still require normal decoder and content handling safeguards.

## Local verification

The narrow release checks are:

```powershell
npm test
npm run verify:release
cargo fmt --all -- --check
$env:CARGO_BUILD_JOBS = "1"
cargo check -p chutes-build --locked
cargo test -p chutes-build-core --locked
cargo test -p xai-grok-tools --lib implementations::chutes:: --locked
cargo deny --locked check advisories licenses bans sources
git diff --check
```

The cross-platform native builds and assembled npm installation are verified by
the `Package release` workflow with publishing disabled before any public
release is authorized.
