# Security review

Review date: 2026-08-01

The latest repository-wide findings and disposition are recorded in the
[2026-08-01 code and security audit](code-review-2026-08-01.md).

This review covers the public Chutes Build source tree, local agent/runtime
boundaries, Chutes and third-party network clients, browser automation, media
artifacts, dependency policy, upstream ports, and npm release assembly. It does
not certify third-party models, MCP servers, plugins, websites, or services a
user chooses to invoke.

## Network and credential controls

- Ambient Chutes credentials are attached only to official allowlisted HTTPS
  destinations. Redirects are disabled on credential-bearing clients.
- Endpoint URLs reject embedded credentials, unexpected ports, insecure
  schemes, and untrusted hosts. DNS resolution rejects private, loopback,
  link-local, multicast, documentation, and other special-use ranges at connect
  time to reduce SSRF and DNS-rebinding exposure.
- `CHUTES_ALLOW_INSECURE_ENDPOINTS=1` is an explicit development escape hatch
  for a local fork. It does not make ambient credentials available to arbitrary
  custom model endpoints.
- Context7 accepts its optional API key only for the official HTTPS service.
  Custom endpoints require `CONTEXT7_ALLOW_INSECURE_ENDPOINTS=1` and never
  receive that key.
- Custom inference models and custom model catalogs use dedicated credentials;
  neither inherits `CHUTES_API_KEY` or a cached Chutes session token.
- OAuth client secrets are read from environment variables, used for both
  exchange and refresh when configured, and never serialized to `config.toml`.

## Files, media, and browser controls

- Generated artifacts and browser screenshots resolve through canonical
  workspace ancestors, reject traversal/absolute destinations, and use
  create-new writes to prevent silent overwrite.
- Non-JSON media responses stream to temporary files. Downloads default to 128
  MiB and are clamped to a 512 MiB hard ceiling; error/JSON bodies are capped at
  32 MiB. Workspace inputs default to 64 MiB with a 512 MiB hard ceiling.
- Media/provenance bundles roll back partial writes. Temporary downloads are
  removed after persistence or failure.
- The browser uses an isolated temporary profile, a loopback DevTools endpoint
  with an exact origin, disabled sync/background features, and password-value
  redaction in snapshots.
- Local image/video decoding, waveform analysis, and playback are bounded,
  off-thread, non-autoplaying, and cancelled when their owning view is dropped.

## Agent and tool controls

- Folder trust gates project permission rules and settings. An untrusted clone
  cannot auto-apply its own bypass configuration.
- Automatic permission mode only fast-paths deterministic read-only shell
  commands. Builds, code runners, mutations, and network-capable actions use the
  classifier or normal prompt; classifier failures and malformed output return
  to the prompt.
- Repeated identical tool calls receive a corrective reminder and terminate at
  a bounded stationarity threshold without fabricating an assistant response.
- Machine-readable MCP listings omit environment/header values, command
  arguments, and sensitive URL components. Plugin subagents can inherit only
  the parent's already-connected, normally filtered MCP pool.
- Destructive session, memory, plugin, marketplace, and worktree operations
  require confirmation unless an explicit `--yes` is supplied.
- Fork/rewind copies only live history and referenced regular compaction
  checkpoint files; malformed paths, directories, and symlinks are skipped.

## Product and release controls

- Telemetry, remote error reporting, trace upload, upstream session
  sharing/search, remote workspace exposure, upstream managed configuration,
  and automatic updates are disabled by product policy.
- CI scans complete Git history with a pinned Gitleaks binary. Cargo Deny checks
  advisories, licenses, duplicate dependencies, and package sources for every
  supported release target.
- Each native npm archive is built on its target architecture, executed with
  `--version`, accompanied by a SHA-256 sidecar, re-verified after artifact
  download, and assembled with the launcher for a final smoke test.
- Dependabot version-update PRs are disabled to avoid automated dependency
  churn in the large generated workspace. Security alerts and automated
  security fixes remain enabled; security PRs require the normal review and CI
  gates.
- GitHub Actions are pinned to full commit SHAs, default to read-only token
  permissions, and cannot approve pull requests. The Windows job does not
  disable host malware protection.
- Upstream baselines advance only after selected ports pass the required local
  or CI gates. A timeout is inconclusive, not a successful review.

## Dependency policy

Direct dependencies with compatible security fixes are upgraded before a
release. `deny.toml` is the machine-readable policy and records bounded
exceptions for transitive advisories that lack a compatible maintained
replacement or do not reach the vulnerable operation in Chutes Build.

The accepted set currently includes the RSA decryption timing advisory (the
crate is used for JWT verification and test keys), unmaintained transitive
crates in syntax highlighting, async retry, TUI, desktop theme, ranking, and PDF
font stacks, plus a build-only Quick XML version used by Wayland protocol
generation. These exceptions remain visible in CI and must be revisited during
dependency or upstream synchronization.

## Static analysis triage

CodeQL default setup reports 309 open alerts against the workspace as of
2026-08-01: 287 `rust/cleartext-logging`, 17 `rust/cleartext-transmission`,
4 `rust/hard-coded-cryptographic-value`, and 1
`rust/uncontrolled-allocation-size`. None is dismissed, so a later true positive
in the same query stays visible. The categories below record why the current
set does not describe a reachable defect.

- `rust/hard-coded-cryptographic-value` (reported critical) matches literal
  `"nonce123"` and `TEST_NONCE` fixtures. All four sit in test-only code: the
  `#[cfg(test)] mod tests` block of `auth/oidc/protocol.rs` and the
  `#[cfg(test)]`-gated `auth/oidc/test_helpers.rs`. The shipped authorize flow
  derives its nonce at runtime.
- `rust/cleartext-logging` is dominated by pager dispatch test modules and by
  CLI/session code that prints or traces a local session identifier. Session
  identifiers are local correlation values, not credentials, and the shared
  log/trace sanitizer already redacts real secret shapes.
- `rust/cleartext-transmission` flags `reqwest` calls whose URL or body carries
  a session identifier. The Chutes-owned clients in `chutes-build-core`
  construct their endpoints through `validate_endpoint_url`, which rejects
  non-HTTPS schemes and untrusted hosts, and resolve DNS through
  `SsrfSafeResolver`. The remaining hits are on the retained upstream backend,
  feedback, and session-registry clients, which are unreachable in this
  product: `REMOTE_SESSION_REGISTRY` is `false`, so agent initialization forces
  `StorageMode::Local` and the only caller that computes `needs_remote` always
  resolves it to `false`; `REMOTE_FEEDBACK` and `REMOTE_SESSION_SHARING` gate
  their handlers to `method_not_found`.
- `rust/uncontrolled-allocation-size` is `ptyctl`'s scrollback reader reserving
  `count.min(history_size())` lines, bounded by the terminal's own buffer.

`chutes-build serve` deliberately prints its generated server key to the
operator's stderr so the WebSocket URL can be copied. That is interactive
startup output, not persisted logging.

Re-triage this list whenever the CodeQL query pack, the flagged files, or the
product policy constants change. A new alert outside these categories is a
release blocker until it is analysed.

## Residual trust boundaries

Chutes Build intentionally executes commands, modifies files, invokes hosted
models, starts subagents, and controls a browser after the applicable permission
decision. Repository instructions, model output, web content, downloaded
documents, plugins, MCP responses, and generated media remain untrusted input.

Generated assets may be hosted by a Chutes-provided external CDN. Downloads are
credential-free, redirect-free, DNS/size bounded, and validated before
persistence, but the bytes still cross normal decoder and file trust boundaries.

## Verification gates

Run the narrowest relevant tests first, then the release gates:

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

Validate changed workflow YAML and relative Markdown links. Run the `Package
release` workflow with publishing disabled before authorizing any public
release. If a required local gate cannot complete, preserve the failure/timeout
as evidence and wait for successful CI rather than marking it passed.
