# Changelog

All notable changes to Chutes Build will be documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The `browser` tool gained the actions its workflow was missing: `wait`
  (until a selector is visible or page text appears), `key` (`Enter`, `Tab`,
  `Escape`, arrows, `Home`/`End`, `PageUp`/`PageDown`), `scroll`, `select`,
  `back`, `reload`, `text` (full visible page text), and `console` / `network`,
  which report the session's browser logs, uncaught errors, and requests.
- Documented desktop control as an opt-in MCP integration, including what
  enabling it gives away, since no built-in tool captures the screen or
  synthesizes input.
- `--output-format streaming-messages-json` emits headless output in the
  Anthropic Messages API wire shape: a `system`/`init` line, `assistant` lines
  whose `content[]` carries thinking, text and `tool_use` blocks, `user` lines
  carrying each `tool_result`, and a final `result`. Ported from upstream.
- `streaming-json` now reports tool activity as `tool_call` and
  `tool_call_update` lines. The headless runner ignored ACP tool-call updates
  entirely, so a consumer could see the agent's prose but never what it did.
- `CHUTES_EXTRA_CA_BUNDLE` adds extra TLS roots from a PEM bundle, for networks
  where a proxy terminates TLS with its own root. Opt-in and additive: unset by
  default, never replaces the built-in roots, and never disables verification.
  A bundle that is oversized, unreadable, or unparseable is reported and
  ignored instead of breaking every request. Ported from upstream, reworked to
  use reqwest's own PEM bundle parsing rather than adding a crate to the
  generated workspace root.

### Fixed

- `browser click` now focuses the element before clicking, matching a real
  pointer click. Without it a click on a field left focus on the body, so a
  following `key` went nowhere.
- Cancelling now stops an in-flight `/compact`. Compaction runs as a command
  rather than a turn, so the cancel path returned early and left a long
  compaction running with no way to stop it. Other commands are unaffected.
  Ported from upstream.
- Saving MCP settings no longer discards the rest of `config.toml`. Every MCP
  writer parsed the file with a fallback to an empty table, so one syntax error
  turned the next save into a full rewrite containing only what that call was
  persisting. Unparseable configs are now refused, the user-config write lock
  covers the whole read-modify-write so concurrent saves cannot erase each
  other, and writes go through the shared atomic helper instead of a fixed
  temporary filename two writers could collide on. Ported from upstream.

## [0.4.3] - 2026-08-02

### Fixed

- `/advisor` now spawns. Its curated toolset already declares `memory_search`,
  `memory_get` and `web_fetch`, and session-level injection appended a second
  copy of each; both entries resolved to one client-facing name, so toolset
  validation rejected the agent with `duplicate client_name` on every attempt.
  Injection now skips tools the agent already declares.
- Bundled skills cited tool names the model is never shown (`task` is exposed
  as `spawn_subagent`, `run_in_background` as `background`), sending it after
  tools absent from its schema. Names corrected in `/best-of-n` and
  `/check-work`, with a test tying skill text to the advertised names.
- Repaired the nine stale `xai-grok-agent` tests: two asserted behavior the
  code no longer has (retired `grok_*` toolset presets, `web_search` before it
  became a default-on native provider), three compared prompt budgets against
  raw `include_str!` output that a CRLF checkout inflates, two compared paths
  with hardcoded forward slashes, and two staged a fake home through `$HOME`,
  which `dirs::home_dir` honours only on Unix. The two home-dependent rules are
  now split into injectable helpers covered on every platform, and their
  environment-driven integration tests are marked Unix-only.

### Added

- The CI Rust job now runs the `xai-grok-agent` library tests and the bundled
  skill checks, which were previously never executed by any workflow.

### Changed

- `generate_media` is self-contained: it resolves a plain model name against
  the catalog and places the new top-level `prompt` into whatever text field
  the selected cord declares, so `{model, kind, prompt}` is a complete call.
  `params` is now optional and only needed for non-default settings or input
  assets, and schema-mismatch errors list the accepted fields instead of
  directing the caller back to `describe_media_model`. The `/imagine`,
  `/imagine-video` and `imagine` skill instructions no longer mandate the
  three-call list → describe → generate sequence.
- Updated the pinned `actions/checkout` workflow dependency to `v7.0.1`,
  including its Git argument escaping and pull-request safety fixes.

### Security

- Marked the three synthetic secret-detection fixtures explicitly so both
  worktree and full-history Gitleaks scans pass without weakening real-secret
  detection.

### Documentation

- Recorded the CodeQL static-analysis triage in the security review: current
  alert counts, the disposition of each query, and the reachability argument
  showing the retained upstream backend client cannot run while
  `REMOTE_SESSION_REGISTRY` is `false`.
- Documented the pinned Rust/Node toolchain, the single supported package
  manager, the artifacts the project actually produces, the protected assets
  and generated files, and the repository-visibility rules in `AGENTS.md`.
- Described the advisor's actual read-only toolset and the `/advisor` controls
  in the subagents guide, and recorded the self-contained `generate_media`
  workflow in the Chutes ecosystem and MCP guides.

## [0.4.2] - 2026-08-01

### Added

- `/undo` is accepted as an alias for `/rewind`.
- Added repository-wide coding-agent guidance and editor defaults.
- The npm launcher archive now includes the changelog and security policy.

### Changed

- Reviewed Grok Build upstream through `0.2.117` and recorded the selective
  port and deferral decisions.
- Removed the orphaned Dependabot auto-merge workflow after automated version
  update PRs were disabled.
- The Windows CI job no longer disables real-time malware protection.

### Fixed

- Repeated-tool-call reminders are injected only after the preceding tool
  results are committed, preventing duplicate results from corrupting session
  history.
- Empty Enter no longer silently approves a plan while the revision prompt is
  focused; revision notes are required, with `a` as the explicit approve key.
- Notification-hook tests that launch external shells are serialized, avoiding
  Windows CI failures caused by concurrent PowerShell startup contention.
- The asynchronous notification-hook test now joins its worker before checking
  output, removing a race with slow PowerShell startup on loaded CI runners.
- Removed Rust 1.94 Clippy warnings in the Windows crash handler, PTY text
  rendering, and workspace utilities without changing their behavior.
- Notification events without an owning session explicitly remove any stale
  `CHUTES_BUILD_SESSION_ID` inherited from the parent process.

### Security

- Documented and aligned repository security controls for SHA-pinned Actions,
  read-only workflow tokens, and reviewed security-update PRs.
- Updated vulnerable transitive `rand` and AWS SDK lockfile entries to their
  first compatible patched releases.

## [0.4.1] - 2026-07-26

### Added

- Remote ACP background terminal tasks now persist their cumulative output
  locally while they run; terminal results use the real remote exit status.
- Repeated identical tool calls are detected, warned about, and eventually
  stopped without emitting a misleading assistant response; repeated `true`
  no-ops use a shorter limit.

### Changed

- Dependabot auto-merge is limited to patch updates. Minor and major updates
  remain open for manual review.
- The upstream-watch workflow refreshes the existing review issue with the
  current upstream commit and version instead of leaving stale issue content.
- Plugin subagents inherit the parent's already-connected MCP pool through the
  normal inheritance filter, without allowing plugins to create connections.
- A fresh managed-MCP catalog is propagated to live sessions and refreshes the
  tool-search index without requiring an application restart.

### Fixed

- Session-creation failures now clear the stuck loading state, remove orphaned
  sessions, and surface a useful warning or failure block.
- A bare `Esc` cancels a running turn in non-vim and minimal modes while
  fullscreen vim retains navigation semantics; the queued draft is preserved.
- Fork/rewind copies only live history and the compaction checkpoint files it
  actually references, rejecting malformed paths and non-regular files.

### Security

- Chutes and Context7 endpoint resolution now rejects insecure schemes,
  credentials in URLs, private/special-use network targets, and unapproved
  custom endpoints unless the corresponding explicit development opt-in is
  enabled. API keys are never sent to custom Context7 endpoints.
- Generated media streams to temporary files with bounded response sizes and
  uses create-new persistence so existing artifacts cannot be overwritten.
- Automatic permission classification only fast-paths deterministic read-only
  shell commands; classifier failures fall back to the normal user prompt.
- Permission resolution now honors the folder-trust verdict: an untrusted
  project's `.claude/settings.json` (including `defaultMode: bypassPermissions`)
  and `.chutes-build/config.toml` `[permission]` rules are no longer
  auto-applied. A cloned repository could previously ship permission rules
  that auto-approved tool calls regardless of the folder's trust state. Global
  and admin-tier permission sources are unaffected. Found while reviewing
  upstream grok-build for changes worth porting.

### Documentation

- Reworked the README around a concise product overview, static terminal
  screenshot, quick start, privacy contract, common workflows, and repository
  map; removed the obsolete promotional video assets.
- Added dedicated getting-started and configuration guides, reorganized the
  documentation index, and aligned privacy, security, architecture, npm,
  contributor, and embedded `/docs` references with current behavior.
- Corrected OAuth guidance, media response limits, automatic-permission
  semantics, remote terminal behavior, `Esc` cancellation, and upstream
  baseline rules.

## [0.4.0] - 2026-07-23

### Added

- Machine-readable `--json` output for the model catalog and local session
  list/search/delete commands.
- Headless worktree creation, including `--worktree-ref`, and headless
  `--no-subagents` / memory controls.
- CSS-selector or snapshot-index addressing for browser click/type actions.
- A complete public CLI and slash-command reference.

### Changed

- Session storage, search, deletion, traces, and workspace state are now
  enforced as local-only product policy. Inherited remote write-back settings
  cannot override it.
- Updates are manual through npm/release artifacts. The runtime no longer links
  or starts the inherited automatic updater.
- Custom model catalogs use `CHUTES_MODELS_API_KEY`; ambient Chutes API/session
  credentials are restricted to allowlisted official HTTPS endpoints.
- Destructive session, memory, plugin, marketplace, and worktree operations
  require confirmation or an explicit `--yes`.

### Fixed

- OAuth login (`l` / `/login`, "Sign in with Chutes"): the token exchange (and
  now refresh) send a `client_secret` when one is configured. The built-in app
  (`cid_nyt9i...`) currently rejects the token exchange with `invalid_client`
  unless a secret is sent, despite being documented as a public PKCE client —
  confirmed by isolating scope as a non-factor in a controlled A/B test.
  `openid` is also requested again — Chutes' own app docs list it as
  required, contradicting the assumption an earlier revision of this list was
  based on.
- A `client_secret` supplied via `CHUTES_BUILD_OAUTH2_CLIENT_SECRET` /
  `CHUTES_BUILD_OIDC_CLIENT_SECRET` was silently dropped before reaching the
  token request whenever `config.toml` also had an
  `[grok_com_config.oidc]`/`[oauth2]` table: the config merge round-trips
  through a generic TOML value, which does not preserve fields marked
  `#[serde(skip)]`.
- Relative `--cwd` values are canonicalized before changing directories,
  avoiding a second relative-path resolution.
- `--load` now behaves as an alias of `--resume`; resume/restore conflicts,
  headless-only options, `--no-plan`, `--best-of-n`, and prompt/subcommand
  combinations fail with actionable parser errors instead of being ignored.
- Agent `--reauthenticate`, `--reasoning-effort`, model-list cancellation,
  export/clipboard failures, worktree partial failures, plugin registry
  failures, and custom OAuth refresh secrets now behave consistently.
- `CHUTES_BUILD_HOME` now relocates user roles/personas and the bundled-agent
  cache together with the rest of the application state, including on Windows.
- Media writes roll back partial artifacts when either media or provenance
  persistence fails.

### Security

- Removed the hidden remote workspace-exposure CLI and disabled the matching
  leader protocol capability.
- `mcp list --json` redacts environment/header values, command arguments, and
  URL credentials/query/fragment data.
- `agent serve` defaults to loopback, rejects short secrets, and requires
  `--allow-remote-bind` for non-loopback listeners.
- Plugin/marketplace removal and generated-media persistence now surface
  partial failures instead of reporting false success.

### Removed

- Inherited remote share, feedback, coding-data retention, update, setup, and
  workspace-exposure commands that were unavailable or incompatible with the
  Chutes Build privacy contract.
- The no-op `trace --local`, `agent serve --remote`, and duplicate load/update
  flags.

### Documentation

- README: documented the OAuth environment variables and the custom-client
  fallback.

## [0.3.0] - 2026-07-22

### Added

- Voice recording bar: an animated wave visual while the mic is capturing.
- Auto-update checks re-enabled (were previously hardcoded off regardless of
  config or CLI flag). Still off in debug builds; honors `--no-auto-update`
  and the new `CHUTES_BUILD_DISABLE_AUTOUPDATER` env var.

### Fixed

- Voice dictation actually works now: default transcription switched to a
  batch REST call against Chutes' own STT endpoint. The prior WebSocket
  streaming client only ever worked against the original upstream backend —
  every dictation attempt against Chutes' inference API failed with a
  WebSocket 404 (no such route exists there). The streaming transport is
  still available (`stt_mode = "streaming"`) for a backend that speaks it.
- Voice dictation: the 10-second no-speech timeout now distinguishes true mic
  silence (permissions, muted input) from other failures and surfaces a
  platform-specific fix hint instead of one generic message.
- Sampler: a zero-retry budget now fails fast instead of silently falling
  through the generic/rate-limit retry paths; retry backoff sleeps are now
  cancellable, so a cancelled request during backoff no longer waits out the
  full delay.

### Security

- Sandbox: fixed a fail-open bypass where an unopenable device file (e.g.
  `/dev/tty` with no controlling terminal under headless/CI) aborted the
  entire Landlock/Seatbelt ruleset instead of just skipping that one rule,
  leaving the sandbox unenforced.
- Permission auto-mode and exec-risk classification brought forward from a
  stale pre-0.2.106 baseline: kubectl/`ps`/`rg` heuristic hardening
  (credential-plugin and environment-dump risks), a 3-tier environment-risk
  model replacing a binary check, opaque-shell detection, an ambient
  exec-risk scan for `git -C`/`--git-dir`/submodule config paths, and
  reinstated consecutive/total auto-deny limits.
- MCP tool permission checks now use an overlap-aware `server__tool` name
  parser (rejects names with more than one delimiter boundary) instead of
  splitting on the first occurrence.

## [0.2.0] - 2026-07-22

### Added

- Chutes-native inference, model discovery, automatic routing, and fallback
  handling.
- Adaptive reasoning controls backed by a centralized model capability
  registry.
- Advisor and parallel subagent orchestration.
- Built-in Context7, official Chutes research, web search, browser automation,
  and project memory.
- Chutes usage indicators for rolling four-hour and monthly limits.
- Typed image, video, and audio artifacts with bounded, opt-in terminal
  previews and native-player fallbacks.
- Privacy-first defaults with telemetry, remote trace upload, and automatic
  update checks disabled.
- Cross-platform npm launcher and native package pipeline.
- Voice dictation (`/voice`, Ctrl+Space, or the mic icon), enabled by default;
  recording still starts only on an explicit manual press.
- Hybrid memory search enabled by default: local recall now combines full-text
  search with semantic vector search against a built-in Chutes-hosted
  embedding model (`Qwen/Qwen3-Embedding-8B-TEE`).
- Advisor now reasons at maximum effort by default (previously inherited
  whatever effort the parent session was using). `/advisor on|off` enables or
  disables the subagent; `/advisor <model>` pins the model it uses;
  `/advisor default` clears the pin. Writes `[subagents.roles.advisor]` /
  `[subagents.toggle]` in config.toml — the running session's own model is
  never touched.
- On-demand OCR (`ocr_page` tool): extracts text verbatim from a single image
  or PDF page via a dedicated Chutes vision model, independent of the active
  chat model's vision support. Billed against the account's subscription
  quota, never the separate marketplace/wallet balance third-party chutes use.

### Changed

- Rebranded the public product, binary, user data directory, and themes as
  Chutes Build.
- Cached FFmpeg and package-manager discovery for the lifetime of the process
  so idle TUI rendering does not repeatedly probe external commands.
- Bounded account quota fallback concurrency and cached model-capability
  discovery to reduce unnecessary API work.
- The model-capability catalog response and the vision transcription client's
  chat-completion response are now parsed into small typed structs instead of
  navigated as raw JSON, so an unexpected shape in a field we don't read can't
  silently change behavior; fields whose absence or wrong shape must stay a
  soft "not found"/"no match" (a model's `input_modalities`, a transcription's
  `content`/`finish_reason`) are still parsed leniently, only genuinely
  malformed responses now surface as a distinct decode error. Account usage
  and per-chute media responses stay dynamic on purpose — their shapes
  legitimately vary per tier/quota kind and per third-party chute schema.

### Fixed

- Interactive login: the welcome screen and `/login` had no working path to
  the OAuth method the rebrand advertised, and no way to enter an API key
  from inside the running app at all (only the separate `chutes-build login`
  CLI subcommand worked, before ever starting the TUI). Added "Sign in with
  Chutes" OAuth end-to-end (issuer, client, scopes, loopback callback) and an
  in-TUI API key entry reachable from the welcome screen (`k`), `/login`, and
  `/apikey`, and fixed two stale-state bugs (`auth_show_raw_url`,
  `welcome_prompt_focused`) that silently swallowed keyboard input on the
  auth screen.
- `get_chutes_usage` and all media tools 401'd by default: the account/media
  HTTP client sent the API key without the `Bearer` prefix `api.chutes.ai`
  requires.
- The macOS native build failed to compile (a stray `cfg` left an
  `std::process` import unreachable in a macOS-only module).
- The secret sanitizer that filters Sentry/Mixpanel/log output did not
  recognize the `cpk_` Chutes API key prefix, so real keys were not redacted
  from those sinks.
- The browser automation tool left a dead session in place after any
  connection-level failure (closed socket, crashed browser), making it
  unusable for the rest of the session until an explicit `close`.
- `generate_media` only validated the top-level shape of `params` against a
  cord's schema, so a payload with the right outer wrapper (e.g. `args`) but
  wrong fields nested inside it passed local validation and round-tripped to
  Chutes for a generic "Invalid input parameters" error instead of a precise
  local one.
- The terminal window/tab title showed "grok" instead of "chutes-build".
- Memory embeddings always sent requests to the chat completion base URL
  (`llm.chutes.ai`, which does not proxy `/embeddings` and 404s), and
  unconditionally requested Matryoshka-truncated output via a `dimensions`
  field the default embedding model rejects outright (400). Together these
  made vector memory search completely unusable even when `[memory.embedding]`
  was explicitly configured. Embeddings now use their own configurable base
  URL (defaulting to the model's dedicated endpoint) and no longer request
  truncation.
- `generate_media` only encoded workspace file paths found at the top level
  of `params`; a cord requiring a wrapper shape (e.g. `{"args": {"image":
  "input.png"}}`) never got its nested path encoded, so the literal filename
  round-tripped to Chutes instead of the file's contents. Asset discovery now
  recurses through nested objects and arrays.
- `generate_media`'s schema guard unconditionally overwrote a cord's own
  `additionalProperties`/`unevaluatedProperties` with its own default —
  including stripping them entirely when `CHUTES_ALLOW_UNKNOWN_PARAMS` was
  set, removing a restriction the provider declared rather than just
  disabling this tool's own default. It now only adds its own restriction
  where the schema has no existing opinion.
- `generate_media` accepted any response with an empty or `octet-stream`
  Content-Type outright, so a Chutes error response (HTML/JSON, sometimes
  served as `octet-stream` by a proxy) could be saved to disk as if it were
  the requested media. Responses are now verified against actual byte
  content, not just the declared header.
- The model-capability catalog cache held a single global lock for the
  entire network refresh, serializing every concurrent lookup behind one
  HTTP request whenever the cache was stale.
- On the Windows native build, file-tool path remapping (`resolve_model_path`)
  and the `.gitignore` out-of-repo guard both tested `Path::is_absolute()`,
  which on Windows also requires a drive letter. The forward-slash-rooted
  virtual paths these paths actually deal with (e.g. a sandboxed session's
  `/home/user/project`) were therefore misclassified as relative, silently
  breaking model-facing path resolution and letting an out-of-repo path fall
  through to a crate call that can panic. Both now check for a root instead
  of a full drive-qualified absolute path.
- On the Windows native build, LSP crash-recovery silently failed to replay
  a server's previously-open documents after a restart: it derived each
  document's file path by stripping the literal `"file://"` prefix off its
  URI, which on Windows leaves an invalid leading slash before the drive
  letter (`/C:/...`), so the on-disk re-read failed and the document was
  quietly dropped instead of being replayed. Now uses proper file-URI-to-path
  conversion.

### Security

- Restricted credential-bearing media invocation to Chutes HTTPS hosts,
  disabled credential-bearing redirects, and bounded generated-media and input
  asset sizes.
- Restricted browser and media output paths to canonical workspace locations,
  prevented silent file overwrite, and redacted password values from browser
  snapshots.
- Added full-history secret scanning, Rust dependency/source policy checks,
  native archive checksums, and assembled-package smoke testing to CI.
- `CHUTES_INFERENCE_BASE_URL`/`CHUTES_API_BASE_URL`/`CHUTES_ROUTER_BASE_URL`
  previously accepted any value with no validation, so an env-var override
  could silently redirect Chutes credentials to an arbitrary host. These
  (and the model-router dispatch path, which had the same gap) now require
  HTTPS and a trusted Chutes/router host by default; `CHUTES_ALLOW_INSECURE_ENDPOINTS`
  is an explicit opt-in for local development and forks.
- Outbound requests for Chutes credentials and generated-media downloads now
  refuse to connect to loopback/private/link-local/reserved addresses,
  checked at the exact point of connection (closing a DNS-rebinding gap
  where only literal IP addresses were previously rejected).
- Unified secret detection: Context7's outbound-query guard and persistent
  memory filtering matched only 12 hardcoded substrings (`api_key`,
  `bearer`, `password`, ...) and could miss shapes like a bare AWS key or
  GitHub token that the existing log/trace sanitizer already caught. Both
  now route through the same canonical detector.
