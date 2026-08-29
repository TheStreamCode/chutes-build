# Changelog

All notable changes to Chutes Build will be documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- **Generated shell completions no longer leak the upstream binary name.** The zsh generator embeds subcommand doc comments, and an MCP argument comment still said `grok`; all four generators now produce scripts with zero upstream-name occurrences.
- **The encrypted agent prompt matches the current prompt templates again.** The 1.0.8 session port reverted `prompt_encrypted.rs` to stale ciphertext; the fork's template encryption generator is restored, so the staleness gate is reproducible instead of hand-patched.
- **Windows session bookkeeping no longer reports phantom flush failures.** The durable summary sync reopened `summary.json` read-only, and Windows refuses `FlushFileBuffers` on a write-less handle, so cwd-switch and rewind bookkeeping failed even after the append had committed.

## [1.3.0] - 2026-08-25

### Added

- **Opt-in model switching around plan mode.** `[models] plan_model` switches to a stronger model when plan mode engages and `[models] build_model` back to a cheaper one when it exits; each direction fires only when its key is set, a key that resolves to nothing logs a warning and keeps the current model, and the mode change itself never fails because of it. Client-driven transitions (Shift+Tab, `/plan`, the toggle ext method) are covered; approving an `exit_plan_mode` proposal keeps the current model.

### Changed

- **The runtime is synced to upstream `1.0.8` (`07b2f714`).** Ported by hand per area: MCP elicitation, shared HTTP client reuse, task-tool capability modes, kitty keyboard protocol, NFS fast-worktree backend, scheduled-task management, worktree detach/salvage, a new `xai-grok-status-line` crate, session-events, auth-manager bounded refresh, and the restructured agent tool registration.
- **The advisor subagent is removed.** The always-registered read-only reviewer (and its `/advisor` command writing untyped config tables) was an upstream concept this fork had no Chutes reason to keep; removing it shrinks the task-tool catalogue advertised to every model. Historical mentions in past changelog entries are records of past states.
- **Telemetry deadening verified end-to-end.** The external OTEL stream stays compile-time inert — `init` is a no-op, `is_active()` is constantly false, no exporter is ever constructed, and the silence tests pass regardless of environment or remote settings.

### Fixed

- **A fresh install is never sent through xAI's OAuth device flow.** The ported auth default constructed a hardcoded provider with the upstream issuer, so an unconfigured install opened `https://accounts.x.ai/oauth2/device` even with `CHUTES_API_KEY` present; the default now creates a provider only from explicit configuration, and the API-key scope constant returned to its Chutes value.
- **Tool calls a model emits as text are recovered again.** The upstream session port silently dropped the recovery shipped in 1.2.x — the module stayed in the tree unreferenced, compiling to nothing; the declaration, implementation, and call site are restored, and a new checker makes that failure mode impossible to miss.

## [1.2.4] - 2026-08-24

### Fixed

- **The welcome wordmark no longer disappears on some consoles.** Terminal detection used to suppress the Chutes Build logo entirely on hosts flagged as a legacy Windows console — a plain PowerShell launch could trip it, leaving the first screen without any identity. Height is now the only input: the box-drawing wordmark from 1.2.0 renders on every console at every height that already showed a logo, and the pinned art hashes are untouched.

### Changed

- **Brighter welcome shimmer.** The light sweep across the logo is clearly visible on a bright terminal too: peak strength doubled (0.33 → 0.60), a wider band and a longer sweep per cycle, deeper background breathing.

## [1.2.3] - 2026-08-23

### Fixed

- **Auto (Chutes Router) works without a dashboard-configured pool.** The native `default` alias resolves only against a routing pool saved at chutes.ai/app; accounts without one got a raw `404 model not found: default`. Auto now appends a live inline pool built from the current catalogue (chat-capable models, `:latency` so the warmest/fastest answers) and failovers to the next member when one is cold. Set `CHUTES_ROUTING_STRATEGY=throughput` to prefer tokens-per-second instead.

## [1.2.2] - 2026-08-23

### Changed

- **Model routing targets Chutes' native grammar.** The standalone router deployment the virtual `model-router` id pointed at no longer exists; routing is native to the inference host now — the saved-pool alias `default` (with optional `:latency` / `:throughput`), or inline comma-separated pools composed from CHUTES_ROUTING_POOL and CHUTES_ROUTING_STRATEGY. Legacy configs naming `model-router` keep working, and the dead host is removed from every trust list and default.

### Fixed

- **`update --check` works on Windows.** The updater spawned bare `npm`, which does not resolve to `npm.cmd` there, so the version check always reported "program not found".
- **Models with strict role alternation no longer reject the conversation.** Adjacent same-role system/user messages are merged before sending; Mistral-Nemo-Instruct returned "conversation roles must alternate" before.
- **`sessions search` queries the local index only.** The CLI shipped every query to a remote registry that answers 404 on Chutes — a wasted credentialed round trip and a warning per search.

## [1.2.1] - 2026-08-22

### Fixed

- **`completions <shell>` now targets the public binary.** Every generated script (bash, zsh, fish, powershell) was built under the upstream binary name, completing calls to a program that does not exist; all four generators derive from one public-name constant and a test pins the absence of the upstream name in the emitted scripts.
- **`du` works on Windows.** Worktree-home resolution read `$HOME` directly, which is typically unset there, so `chutes-build du` aborted with "neither $CHUTES_BUILD_HOME nor $HOME is set"; the resolver now falls back to `std::env::home_dir()` (`%USERPROFILE%` on Windows), leaving Unix behaviour unchanged.
- **No upstream placeholders leak into help or completions.** Clap value names derived from internal field ids surfaced as `<XAI_API_BASE_URL>` and `GROK_WS_*` in `--help` and shell completion scripts; those arguments carry explicit value names now, and one MCP doc comment no longer names the upstream binary.

## [1.2.0] - 2026-08-21

### Added

- **Parallel media-generation calls are capped per tool name.** image_gen/image_edit cap at 8 per model step and video-generation tools cap at 4; a burst at least twice the cap is discarded once and retried with a reminder, while any other over-cap keeps the first K and rejects the tail. Configurable via [tools.media_gen] in config.toml or CHUTES_BUILD_MAX_PARALLEL_IMAGE_GEN_CALLS / CHUTES_BUILD_MAX_PARALLEL_VIDEO_GEN_CALLS.

### Changed

- **The runtime is synced to upstream `1.0.5` (`d71f6e0c`).** Eleven upstream crates are added and rebranded: `xai-grok-active-sessions`, `xai-grok-foreign-sessions`, `xai-grok-session-events`, `xai-grok-session-search`, `xai-grok-home`, `xai-grok-workspace-daemon`, `xai-grok-diag-server`, `xai-grok-bundle`, `xai-compaction-transcript`, and `xai-fuzzy-file-search`. `xai-file-utils::events` and the shell's `active_sessions` / `storage/search_*` modules moved into them.

### Fixed

- **`models` no longer fails with "unknown ACP extension method".** The model-list client and handler had drifted onto different wire namespaces after the re-base; the client now sends what the handler registers (`chutes.build/models/list`).
- **Worktree detection on Windows.** `WorktreeDb::get` treated only forward-slash input as a path, so a backslash path never reached the DB lookup; it now accepts both separators. `get_worktree_info` also normalises the git2 commondir back to native separators before display.

## [1.1.0] - 2026-08-18

### Added

- **Tool calls emitted as plain text are recovered.** An open-weight model served through vLLM or SGLang can emit a well-formed tool call in its own chat-template syntax while the server-side parser returns an empty `tool_calls` array, so the turn ends having done nothing and the user sees raw markup. The runtime now recognises the Hermes/Qwen `<tool_call>`, Kimi K2 and Llama inline-function shapes (plus a strict fenced-JSON form) and turns them back into real calls only when the name resolves to a registered tool and the arguments pass that tool's own parser. `CHUTES_DISABLE_TOOL_TEXT_RECOVERY=1` disables it.
- **`/usage` reports cache effectiveness.** A `cache_read_ratio` (cached read tokens divided by input tokens, clamped to `[0, 1]`) and per-call `cache_hit_calls` / `cache_miss_calls` counters join the token totals; the TUI usage block now shows the cached percentage and the hit count.

### Changed

- **Models without tool calling no longer receive a `tools` array.** The live catalog's `supported_features` now gates tool specs: a model that does not advertise `tools` gets an empty tool list instead of a request its endpoint rejects. The `StructuredOutput` tool is gated the same way and falls back to validating the final answer text directly. The default stays `true` for models absent from the catalog.

### Fixed

- **Parallel tool calls from providers that omit the chunk index no longer fuse.** `ToolCallDelta.index` is now optional, and the stream correlates by tool-call `id` when the index is absent; previously a missing index defaulted to `0`, merging every parallel call into one accumulator and concatenating their arguments into invalid JSON.
- **Non-streaming requests now apply the Chutes reasoning plan.** Compaction, title generation and the advisor sent the bare payload to the configured inference base, dropping the `chat_template_kwargs` thinking switches and never routing `model-router` to the router endpoint, the same asymmetry the streaming path already handled.
- **Stream deserialization errors name the offending chute.** A failed chunk now carries a short excerpt of the SSE payload alongside the serde error, so "expected value at line 1 column 1" says which backend produced what.

## [1.0.4] - 2026-08-12

Two threads. The manual port from upstream `b13fa526` begins — one area per
commit, each landing with its own tests green and measured against a pre-port
baseline rather than an absolute count. And the verification system itself was
overhauled, because three of the previous release's problems were failures to
*notice*, not failures of code: 188 tests were failing where nobody looked, a
pager suite reported failures that were the terminal rather than the product,
and a kill switch that documents a privacy guarantee turned out to gate nothing.

### Security

- **`webbrowser` updated for RUSTSEC-2026-0257**, argument injection through the
  Unix `BROWSER` template: an attacker-controlled non-HTTP(S) URL retaining
  spaces could become extra browser arguments, reproduced upstream by injecting
  `--remote-debugging-port` and `--proxy-server`. Chutes Build opens a browser on
  the OAuth login path, so this is a path we use. Fixed in 1.2.2; the lock now
  resolves 1.2.4. The advisory was published while this release was being
  prepared and turned a commit red that had passed an hour earlier — which is
  what `cargo deny` reading the live database is for.
- **The self-update kill switch gated nothing.**
  `product::AUTOMATIC_SELF_UPDATE` documents that a running Chutes Build never
  downloads its own update, and it appeared exactly once in the tree — inside a
  comment. What held the line was narrower and accidental: the `internal`
  installer's base URLs are deadened to `127.0.0.1:9`. But `get_installer()`
  honours `CHUTES_BUILD_INSTALLER=npm` and `[cli] installer` in `config.toml`,
  and neither the npm path (`npm view`) nor gh-release (the GitHub API) is
  deadened, so one line of user configuration turned a release binary into one
  that contacts a registry at startup and can spawn a download child. The gate
  is read now, and a test asserts it is read rather than merely present.

### Added

- **A failed `SKILL.md` read now says where that skill is actually registered**,
  instead of a bare "does not exist". Ambiguous names, disabled skills, stale
  registrations and reads that are not `SKILL.md` deliberately suggest nothing,
  so a wrong pointer is never preferred to no pointer.
- **Every workspace RPC declares whether it reads or writes** (`ACTIVITY`), and
  the proxy client can report the server binary's version.

### Changed

- The terminal runtime sizes its worker pool against per-user ceilings
  (`pids.max` / `RLIMIT_NPROC`) using upstream's reworked logic, with 329 lines
  of tests it did not have before — including the EAGAIN path.
- Sandbox hook write-deny is compiled only where it enforces. Its identity
  capture, symlink and rename-race errors and JSON snapshot check move from
  `#[cfg(unix)]` to `#[cfg(any(target_os = "linux", all(unix, test)))]`: the
  rules are applied through Linux namespaces, so on macOS that code compiled and
  never ran. Linux behaviour is unchanged.

### Fixed

- **A busy model is waited for, not replaced.** A capacity 429 used to swap the
  model you chose for `model-router` on the first attempt, instantly, discarding
  the `Retry-After` the server had just sent — the header was parsed and then
  dropped at that layer. A short delay (≤5s) on a 429 or 503 is now honoured on
  the selected model before any switch; a long one still switches, because that
  chute is busy for a while and another candidate can answer now. A server
  sending `x-should-retry: false` is obeyed rather than retried against.
- **Esc worked only once a stream existed.** Everything before the first chunk
  ignored the cancellation token, so a slow connect left the key dead until the
  request returned on its own. All three backends now race the request against
  cancellation, with the cancel arm biased first — the request goes out on the
  future's first poll, so an already-cancelled turn was previously winning only
  about half the time.
- **`scripts/rebrand.py` rewrote the sentences that name upstream on purpose.**
  Running `--apply` over the tree changed six files with nothing to do with the
  port in hand, every one of them wrongly: the README attribution became
  "SpaceXAI's Chutes Build", `AGENTS.md` came to say the re-base replaced our own
  wordmark with our own, and the getting-started guide gained a fork link to
  `github.com/xai-org/chutes-build`, which does not exist. `--check` reported the
  correct sentences as violations, so it would have pushed the next person to
  "fix" them. The rules cannot tell "Grok Build, the thing we forked" from "Grok
  Build, a name to replace"; those six are now pinned by exact text, and the
  script fails loudly if one is reworded rather than silently exempting a file.

### Verification

- **Those two crates' failing tests are not a fixed set, and that is the
  finding.** CI ran narrow filters over `xai-grok-tools` and
  `xai-grok-workspace`, leaving ~188 tests neither passing nor watched, so a new
  failure among them was indistinguishable from the rest. The intended fix was a
  committed per-test baseline that CI compares against — and it does not work
  here, because the set moves: of 53 `xai-grok-workspace` failures recorded on a
  local Windows machine, 17 pass on the Windows runner and 2 others fail; a
  `daemonize` test failed on Linux in one run and not in the run 90 minutes
  before. Same commit, same platform.

  So `scripts/known_failures.py` and the two baselines ship as a **local
  diagnostic**, not a CI gate, and the CI steps are removed. A gate is possible
  once these suites are deterministic, and making them so is the actual work
  this uncovered — the baseline is not a substitute for it. Separately, the full
  `xai-grok-tools --lib` suite never completes on the Windows runner at all: 107
  minutes, then 140, then a 40-minute timeout, against six minutes on Linux.
  Both are recorded in `docs/upstream-sync.md` under what is still open.
- **The pager suite reported seven failures that were the terminal.**
  `default_actions()` reads the process environment, so inside a VS Code terminal
  Quit binds to Ctrl+D, interject moves to Ctrl+L, and a terminal with native
  link hover owns link clicks. Each test asserted one side of a rule that has
  two. They use the pinned registries that already existed, and the link test now
  asserts both sides — covering something nothing covered: with native link
  hover, *not* intercepting is correct. 8275 pass in both configurations.
- `scripts/port_assist.py` sorts an upstream delta into what needs a judgement
  call and what does not — files absent here, files we never touched, and files
  whose divergence *is* the rebrand, verifiable because
  `rebrand(upstream@base)` reproduces our version character for character. 127 of
  the 208 outstanding files need no human decision. It leaves genuine divergence
  alone, refuses to resurrect files we deleted on purpose, and skips binaries.
- A notification-hook test waited five seconds for a forked shell and went red on
  a loaded runner. `run_hook` returns its join handle now, so the test waits for
  the fact rather than the clock.

### Documentation

- `docs/upstream-sync.md` records what the first areas taught, for the eighty-six
  files still to come: an area is always bigger than the module it started as and
  clippy's `dead_code` is what proves it, since `dead_modules.py` cannot see a
  module that *is* reachable; measure the baseline failure set before attributing
  anything; and upstream's fixtures assume POSIX absolute paths, which on Windows
  are drive-relative, so eleven tests failed here that pass on upstream's CI.
- `AGENTS.md` gains the three rules the port produced — check `git status` after
  every rebrand run, finish an area on clippy rather than on green tests, and
  measure the baseline failure set before attributing anything — and
  `docs/releasing.md` records that a cancelled job ships nothing even when it
  built everything.



## [1.0.3] - 2026-08-11

Measured against the live Chutes endpoint with a real key, not reasoned about from
the source. Two of the findings below contradicted what the code looked like it
did, and one contradicted an earlier fix in this same release.

### Fixed

- **Every model claimed a 256,000-token context.** The catalogue at
  `llm.chutes.ai/v1/models` publishes `context_length` per model;
  `parse_remote_model_value` looked for `contextWindow`, `context_window` and two
  `_meta` spellings, none of which Chutes sends, so all thirteen fell through to
  one default. It was wrong in both directions at once: `Qwen3-32B` holds 40,960,
  so the product promised six times what exists and its requests were refused;
  `DeepSeek-V4-Flash`, `Kimi-K3` and `GLM-5.2` hold 1,048,576, so three quarters
  of the window went unused. Compaction and truncation were sizing themselves
  against a figure no model agreed with. Eleven of thirteen now carry their
  published value; the two that publish nothing keep a default.
- **Each model now carries its own output limit**, from `max_output_length` —
  but only where that is *smaller* than the window. Several models publish the
  two as the same number, which means "output may use the whole context" rather
  than "always ask for this much": sending it as `max_tokens` leaves no room for
  the prompt, and `Qwen3-32B` answers `400 Requested token count exceeds`. Nine
  models gain a real cap, two are skipped. This was caught by testing the fix
  against the endpoint before shipping it — the first version of the change would
  have broken a model that previously worked.
- **An endpoint override reached inference but not the catalogue.**
  `docs/configuration.md` documents `CHUTES_ROUTER_BASE_URL` and
  `CHUTES_INFERENCE_BASE_URL` — `CHUTES_MODELS_BASE_URL` was supported and
  undocumented, and is in the table now — and `chutes-build-core` honours them,
  but the re-base left the agent's
  `EndpointsConfig` reading only the `CHUTES_BUILD_*` spellings. The documented
  names come first now, with the others kept as fallbacks.
- **The system prompt introduced the agent as xAI's.** The re-base overwrote
  `templates/prompt.md` with upstream's, so every model was told at every turn
  that it was "released by xAI" instead of "a privacy-first coding agent
  optimized for the Chutes ecosystem" — the branding fix in 1.0.1 covered what
  the user could see, not what the model was told. The same overwrite deleted
  the `<official_chutes_sources>` block, which is the larger loss: it is what
  directed the model to treat `chutes.ai/docs` and `chutes.ai/news` as the
  primary authority on Chutes products, APIs, models, pricing and quotas, to
  say when a claim is unverified rather than blending fact with inference, and
  — a security instruction, not a stylistic one — never to put API keys,
  credentials, private code or repository contents into a search query or
  outbound request. Both restored from 0.4.3 and the encrypted templates
  regenerated.
- **Only streaming requests could survive a busy model.** The Chutes fallback
  chain — selected model, then `CHUTES_FALLBACK_MODELS`, then `model-router` —
  was wired into `chat_completion_stream` and never into `chat_completion`. The
  two paths had diverged by omission rather than by any decision, so an
  interactive turn recovered quietly from `429 Infrastructure is at maximum
  capacity` while compaction, title generation and the advisor surfaced it raw.
  Both paths now share one chain and one policy. Nothing on the non-streaming
  path can be mid-stream, so the "never switch models once bytes have shipped"
  rule is satisfied trivially there. The chain's ordering also became testable
  without touching process-wide environment variables, which no test could do
  safely while the rest of the suite runs in parallel — the env reads moved to a
  wrapper around a pure core.
- **`CHUTES_BUILD_OAUTH2_PRINCIPAL_TYPE=Team` could never complete a flow.**
  `default_team_oauth2_scopes()` was still upstream's — `grok-cli:access`,
  `api:access`, `team:read`, `conversations:*`, `workspaces:*`. The user-facing
  list was corrected during the re-base cleanup and this one was missed.
  `api.chutes.ai` advertises no team principal at all: its `scopes_supported` is
  entirely user-level, so an authorization server was being asked for scopes it
  has never heard of. The default is now the user list; a deployment that really
  does define a team principal says so through `CHUTES_BUILD_OAUTH2_SCOPES`.
  Established by reading the IdP's own discovery document rather than the prose
  documentation, which does not cover team principals either way.

### Release engineering

- **The first 1.0.3 built every artifact and shipped none.** `darwin-arm64`
  finished in 118m40s of a 120-minute budget: binary, strip, smoke test,
  package, checksum and upload all succeeded, and the wall then killed the
  cache-save step eighty seconds later. A cancelled job fails the run, so
  `Publish GitHub release` and `Publish npm packages` were skipped. Not a
  regression — the macOS jobs of the previous three releases ran 67, 74, 91, 84,
  96 and 102 minutes, so the budget had been within ~15% of the slowest observed
  build for several releases. Raised to 180.
- **`main` had been red since 06:40 and nothing showed it.** A rebranded string
  grew three characters past 100 columns, `Check formatting` is the first step of
  the Linux job, and its failure skipped the six test steps and clippy behind it.
  It stayed invisible because `cancel-in-progress: true` plus a run of quick
  pushes cancelled every CI run in between.

### Documentation

- `docs/upstream-sync.md` records the pending port measured rather than
  estimated: 3 commits, 238 files, of which 147 overlap work of ours and 91 do
  not. It also records that `git diff HEAD upstream/main` is the wrong
  instrument — it reports 1424 files, because the fork is not a descendant of
  upstream and the diff is mostly our own re-base read backwards.
- `AGENTS.md` records that `xai-grok-pager --lib` reports seven failures when
  run from a VS Code terminal, and that they are the terminal rather than the
  code: `default_actions()` reads the process environment, so Quit binds to
  Ctrl+D there and the `ctrl_q` tests fail against a correct app.

### Notes on tool compatibility

The catalogue's `supported_features` is accurate: every model advertising `tools`
emits a real `tool_calls` finish, and the two advertising nothing do not. A first
probe suggested three Qwen models were broken — they were not. The probe gave them
96 output tokens, which a reasoning model spends thinking before it can emit the
call. That is the finding worth keeping: **a thinking model needs output budget
before tool use works at all**, and without it the failure is silent and looks like
missing support.

## [1.0.2] - 2026-08-11

Three faults a user meets in the first minute. All reproduced against the live
service with a real API key, not inferred from the source.

### Fixed

- **`/model` offered a single model.** `ModelFetchAuth::resolve` treated any stored
  credential as a *session*, and logging in with an API key stores one — so
  `auth.json` holding a `chutes::api_key` entry sent the catalogue fetch down the
  session path, which asks the router proxy. That proxy advertises exactly one
  model: itself. `llm.chutes.ai/v1/models` returns thirteen. The request URL was
  already right for API keys; only the classification was wrong. `models` now
  lists all fourteen, with the configured default marked — a default the product
  previously could not resolve even when the user had named it.
- **"Sign in with Chutes" could not complete.** The re-base replaced the fork's
  OAuth scopes with upstream's — `grok-cli:access`, `api:access`,
  `conversations:*`, `workspaces:*` — none of which exist at `api.chutes.ai`. Its
  discovery document offers `account:read`, `chutes:read`, `chutes:invoke`; an
  authorization server rejects scopes it does not know. Restored to the minimum the
  client uses. OAuth still requires an application you register yourself at
  [chutes.ai](https://chutes.ai/docs/sign-in-with-chutes/overview) — there is no
  shared client id, by design — and the API key remains the primary method.
- **The welcome screen disagreed with its own input bar.** The wordmark blended
  from a fixed gray because the re-base replaced `theme.accent_assistant` with
  upstream's `theme.gray`. It takes the accent again, so the centre of the screen
  and the prompt border move together.
- The feedback prompt's default still read "You've been using Grok Code
  productively!". It lives in `prod/mc/cli-chat-proxy-types`, outside the crates
  the 1.0.1 sweep looked at — found by searching the shipped binary rather than
  the tree.

### Changed

- **Silver is the brand accent.** Not a rename of the green: in this palette one
  constant carried both the brand and the *meaning* of success and of a diff's
  insertion side. Those are now two families. `SILVER` takes the chrome — wordmark,
  assistant, model and command labels, active prompt border, H1 rule — and green
  stays exactly where it says something, because a silver diff against a red one is
  unreadable. The silver is cool on purpose: these neutral grays are warm, so a
  warm silver would sink into the furniture.

## [1.0.1] - 2026-08-11

### Fixed (the TUI wore upstream's identity)

- **The splash showed Grok's wordmark.** `assets/logo/logo07.txt` and
  `logo05.txt` were byte-identical to upstream's: the re-base overwrote the Chutes
  wordmark and 1.0.0 shipped it, so every start drew the wrong brand. Restored. A
  sweep of every asset in the tree says these two were the only ones taken.
- **A Chutes user was offered a subscription to another company's product.**
  Hitting a usage limit produced "Upgrade to SuperGrok" and "Upgrade to SuperGrok
  Heavy"; a restricted command said it "requires SuperGrok". The buttons already
  pointed at chutes.ai/pricing — the fork had rebranded where they *go* and left
  what they *say*. This one predates the re-base; v0.4.3 carries the same strings.
- The screen-mode settings described opening "plain grok", the welcome gate read
  "SuperGrok subscription required", and the feedback prompt opened with "You've
  been using Grok Code productively!".

Rust identifiers, crate names, telemetry event types and model ids such as
`grok-4.5` are deliberately untouched: `AGENTS.md` keeps them so upstream diffs
stay readable, and a model name is data rather than branding.

The wordmark's hash is now pinned by a test, so a merge that takes upstream's
side there fails instead of shipping. The loss was silent by nature: nothing
compiles differently, no test read the bytes, and the only symptom was on screen.

### Security

- **The `agent serve` token comparison no longer rests on a function that
  disclaims the property it was chosen for.** It used
  `ring::constant_time::verify_slices_are_equal`, which ring now deprecates as an
  "internal function not intended for external use with no promises regarding side
  channels" — while the reason for calling it at all is that `==` on `&str` returns
  at the first differing byte, and over a socket with no rate limit that leaks the
  token one byte at a time. It now uses `subtle::ConstantTimeEq`, which is
  maintained for this and also resists the compiler folding the comparison back
  into an early return. No new code enters the build: `subtle` was already in the
  graph through rustls, so the lock file moves by one line.

### Fixed

- **`--help` had upstream's branding back in it.** The re-base took upstream's
  wording over the fork's in five places a user reads, three of them flag *names*:
  `--grok-ws-origin` and `--grok-ws-url`, which 0.4.3 spelled `--chutes-ws-*` and
  kept hidden, so the re-base both renamed them and put internal plumbing into the
  top-level help; `--xai-api-base-url`, described as "the public xAI API base URL"
  when it overrides the Chutes inference base URL; the positional-prompt example,
  which read ``grok "fix the bug"`` beside a `chutes-build` example on the same
  line; and the `screen_mode` help, which said "To default plain `grok` to
  minimal". Found by installing the published 1.0.0 the way a user gets it and
  reading every help surface — the step the release procedure asks for and which
  had not been done. `leader/mod.rs` passes those flags to the child process, so
  the rename covers it too.

### Repository and CI

- **`xai-grok-shell --lib auth::` runs on Windows.** It failed 24 there, and the
  CI step had never executed at all, because the pager step above it always failed
  first. A provider command goes through `cmd /C` off Unix — deliberately, for
  exit-code propagation — while the fixtures were POSIX one-liners. They now drive
  `auth-provider-fixture`, a real helper invoked with `args`, so no shell
  interprets them. Two lock tests read the lock file through a second handle while
  holding it, which Windows refuses; they read through the holding handle now, as
  the product does.
- **The Linux job reaches its end.** It had not passed in the last twenty runs, so
  four steps below the failure — auth and session integration, agent construction
  and bundled skills, Chutes-native tools, and clippy — had not run either. Behind
  them were a banned `tokio::process::Command::spawn` in a test the ban does not
  describe, and a doc comment separated from the function it documents.
- **That job's 60-minute limit was sized for the broken state.** The first run to
  reach the end spent 59.7 minutes on work that passed and was killed during the
  cache *save* — which left the next run cold, slower, and killed in the same
  place. Raised to 90, matching Windows. Measured twice since at 65 and 67 minutes:
  the cost is roughly 45 minutes of tests plus 11 of clippy, not compilation, so a
  warm cache moves it very little and the headroom is about a quarter.
- **All five CI jobs pass.** Windows had been red since 1.0.0 landed and Linux had
  not finished in over twenty runs; this is the first complete green run.
- `RUSTSEC-2026-0249` is recorded in `deny.toml`: `smartstring` is unmaintained —
  archived, not vulnerable — and reaches this tree only as a non-optional
  dependency of `rhai`, whose latest stable we already use, so no bump closes it.
  It turned the dependency job red on a documentation-only commit, an hour after
  the same check passed, because that job reads the live advisory database.
- A flaky history-delivery test asked for a repaint and a result in one condition;
  it fails when the daemon is *fast* enough to answer before the eager snapshot,
  which is why widening its deadline had not helped.

## [1.0.0] - 2026-08-07

Chutes Build is re-based onto `xai-org/grok-build` 1.0.0 (`afbc0fb`) and adopts
its version, so the number now says which upstream this build actually is.

### Why the re-base

`.github/upstream.json` recorded `lastReviewedVersion: 0.2.117`, but those
commits had been *reviewed and not taken* — the records said "nothing safe to
port in isolation". For the retained crates the tree was still at the 2026-07-17
fork point, 2075 files and roughly 440k lines behind upstream. Cherry-picking
could not close that: upstream added 94k lines in five days. Measuring the fork
showed why re-basing was the cheaper path — only 102 files existed here and not
upstream, and only 26 actually consume `chutes-build-core`.

### Added

Everything upstream shipped between the fork point and 1.0.0 arrives at once:
1348 files of work from the never-taken window, plus the roughly 110 changes
between 0.2.117 and 1.0.0. Highlights users will notice: session recaps that
follow the language of the conversation, narrow markdown tables that reflow
inside their cells with hard splits on grapheme boundaries, a plan-viewer
scrollbar that is easier to grab, `--output-format streaming-messages-json`,
tabbed usage and session-info, and a large batch of terminal, tmux and session
fixes.

### Security

Carried forward and, where upstream had since fixed them differently, taken from
upstream instead:

- Permission rules for Read, Edit and Grep lexically normalize the tool's path
  before the glob match, so `Read(src/**)` can no longer be escaped with
  `src/../../etc/passwd`.
- `NotebookEdit` / `NotebookRead` no longer alias onto `Edit` / `Read`.
- A client-forwarded MCP server matching an on-disk Claude or Cursor config is
  rejected while that vendor's `mcps` switch is off.
- Diagnostic logging of a credential fragment no longer panics on a non-ASCII
  token.
- Chutes API keys (`cpk_`) are recognised by the secret redactor.
- Memory passes through the secret filter at every write path, and every write
  path now creates its file owner-only. Three of the four created files with the
  process umask, so a fresh install's `MEMORY.md` and both bootstrap templates
  were world-readable — and memory holds whatever you asked the agent to
  remember.
- `model-router` dispatch validates `CHUTES_ROUTER_BASE_URL` through the
  endpoint policy before a session credential is sent to it.
- The default inference endpoint is `llm.chutes.ai` again. The re-base had left
  upstream's `api.x.ai/v1`, so any model entry without an explicit base URL
  would have sent a Chutes API key to xAI.
- The relay and gateway WebSocket endpoints are loopback sentinels again, not
  `code.grok.com` and `grok.com`. Chutes Build has no vendor relay; those paths
  must fail closed.
- The changelog fetch, the update check and the remote-session/workspace clients
  point at a closed loopback port, so none of them can phone home.
- The npm scaffold that would install `@xai-official/grok-<platform>` binaries on
  postinstall is gone, along with the version check that compared this build
  against upstream's package.

### Fixed

- Cloudflare edge failures (520–524, 529, 530) are retried instead of ending the
  turn, while origin-TLS 525/526 stay fatal.
- A server `Retry-After` is clamped to 30s and jittered.
- Failed requests no longer dump a Cloudflare HTML error page into the terminal.
- The sandbox starts on large workspaces (deny-glob entry budget 200k → 2M).
- `bin/protoc` is a DotSlash wrapper Windows cannot execute from a build script;
  a vendored platform binary is now the fallback.

### Fixed (the Chutes tool layer was unreachable)

- **`generate_media`, `list_media_models`, `describe_media_model`, `browser`,
  `ocr_page` and `get_chutes_usage` were in no toolset.** The re-base kept every
  implementation and dropped every registration, so six thousand lines of working
  code compiled, passed their own tests, and could not be called. Upstream's
  toolsets carried its Imagine tools instead, which talk to an endpoint Chutes has
  no equivalent for.
- **`/imagine` and `/imagine-video` had disappeared**, for the same reason: the
  pager hides a command whose required tools are not advertised, and they required
  the xAI Imagine tools. Both now use `generate_media`, and their instructions
  teach the Chutes workflow — list the catalog, describe the model, compose the
  payload from the cord's own example — because no two Chutes media models share a
  schema. `/imagine-video` is rewritten rather than renamed: upstream's guidance
  opens with "there is no text-to-video tool", which is true of that API and false
  here.
- **Plugin sources in the system temp directory were auto-trusted on Windows.**
  Auto-trust covers anything under the user's home, and on Windows temp *is* under
  the home — so a plugin unpacked there skipped the explicit trust step entirely.
  Found by a test that had been failing on Windows since before the re-base.
- Five upstream test failures on Windows are gone; `xai-grok-agent` now passes all
  578. Two were real defects rather than test bugs: the trust hole above, and a
  home-directory guard no test on Windows could reach.

### Fixed (Windows, and these were the product)

The pager's Windows suite had sixty-six failures. Most were fixtures written for
POSIX, but five were defects users would meet:

- **`doctor fix` could not apply anything.** The managed-config writer opened the
  parent directory as a file, to `sync_all` it after the rename. That sync is
  Unix-only — Windows has no durability barrier for a directory — so off Unix the
  handle was captured and never read, and Windows cannot produce one at all:
  `File::open` on a directory needs `FILE_FLAG_BACKUP_SEMANTICS`, which `std` does
  not set. Every fix planned, then failed to write, reporting "access denied"
  against the directory rather than the file.
- **No file path written in agent prose was clickable.** The scanner that turns
  paths into `file://` overlays matched `~?/…` only, so `C:\Users\me\x.md` was
  never recognised. Only tool headers worked, because those receive the path
  directly instead of finding it in text.
- **Tab completion in the extensions modal destroyed the path.** It rebuilt the
  directory prefix by looking for a `/`; a Windows path has none, so completing
  `C:\src\plugin-s` replaced the whole field with `plugin-source`. The directory
  listing, gated on the text ending in `/`, never triggered either.
- **The memory-search header showed `\MEMORY.md`.** The path shortener looked for
  `/` alone, so the separator after the memory root survived — and a nested result
  was not shortened at all, painting the whole `\xai-…\sessions\…` tail.
- **A test fixture named `nul` proved nothing**, because on Windows that name is
  the null device wherever it appears: the write went nowhere and the NUL-byte
  refusal it existed to check was never reached.

### Fixed (releases carried an unstripped binary)

`release-dist` keeps symbols on purpose, for sidecars extracted "before stripping
post-build" — and that post-build strip had never been written. The npm packages
therefore shipped the profile's full DWARF: `linux-arm64-gnu` reached 938 MB
unpacked and the registry refused it outright with `413 Payload Too Large`.
Stripping now happens between the build and the smoke test, so the run check, the
package, its checksum and the release asset all see the binary that ships. Linux
falls from ~890 MB to ~130 MB; Windows needs none, since MSVC keeps debug info in
a separate `.pdb`.

The macOS packages for this version were published before that fix and are larger
than the rest; they cannot be replaced, because an npm version is immutable.

### Authentication

Upstream ships an OAuth application everyone signs in through, so its
documentation and defaults put browser login first and treat the API key as a
fallback. Chutes does not work that way: OAuth needs an application you register in
your own account area at [chutes.ai/app/api](https://chutes.ai/app/api), so there is
no client ID that could serve anyone but its owner.

- **The compiled-in OAuth client ID is gone.** It offered a sign-in that would fail
  for every user but one. `OAuth2ProviderConfig::from_env` now returns `None` unless
  `CHUTES_BUILD_OAUTH2_CLIENT_ID` names an app, which is also upstream's shape and
  so one seam fewer at the next merge. With no app configured, `login` already said
  the right thing — "Sign-in is not available for this deployment. Set
  CHUTES_API_KEY instead." — and now that message is reachable.
- **A cached session survives a shell with no OAuth app configured.** The
  compatibility check compares a credential's issuer against the configured one;
  with no provider configured there is nothing to compare against, and the question
  becomes which credentials are self-sufficient. A session is: `oidc::refresh`
  renews it from the issuer and client id stored in the credential itself, not from
  the config. So exporting `CHUTES_BUILD_OAUTH2_CLIENT_ID`, signing in, and then
  launching from a desktop shortcut or a cron entry that lacks the variable keeps
  working — discarding the session there would strand the user in the one state
  where interactive login is unavailable. The legacy issuer-less `WebLogin`
  credential is still rejected, now for either config shape, because it has neither
  issuer nor client id to renew with.
- **The documentation was rewritten around the API key**, which is the primary and
  always-valid credential rather than a fallback: the authentication chapter, the
  first-launch section of the getting-started guide, `docs/getting-started.md` and
  the README. `docs/getting-started.md` had also described a "bundled client ID"
  that no longer exists.
- **`jsonwebtoken` had no provider to sign or verify with.** It picks one from cargo
  features, and this graph enables both: the shell declares `rust_crypto`, while
  `xai-file-utils` -> `gcloud-storage` -> `gcloud-auth` enables `jwt-aws-lc-rs`. Cargo
  unifies features across the graph, so neither can be turned off from here, and with
  both on the crate refuses to choose — every signature operation panics. Production
  survived on placement alone: `run_login_flow_with_config` installed a provider and
  is the only entry to the one function that verifies an id_token, which holds exactly
  until a second verification path appears. The install now sits at each point of use,
  behind a `OnceLock`, so the library is correct by construction; 23 auth tests that
  had been failing on Linux CI depend on it too.
- **Six example keys read `CHUTES_API_KEY="xai-..."`** — upstream's prefix, left
  behind when the rebrand renamed the variable but not its value. A Chutes key is
  `cpk_`-prefixed, which the redactor already knew and the docs did not.

The API key also has to be *enterable*, and on this branch it was not:

- **`/apikey` did not exist.** `slash/commands/apikey.rs` came across in the
  re-base and was never declared in `slash/commands/mod.rs`, so it was not
  compiled — the same registration gap as the Chutes tool layer, in a third
  registry, and invisible for the same reason: an unreferenced module is not
  built, so nothing warns. Everything it needs went with it —
  `Action::EnterApiKey`/`SubmitApiKey`, `Effect::SubmitApiKey`,
  `AuthMode::ApiKeyEntry`, the dispatchers, the welcome-screen arm, the `k` key.
  The shell half was intact the whole time: `chutes.build/setApiKey` is handled in
  `extensions/auth.rs` and allow-listed in `acp_agent.rs`. Nothing could reach it.
- **`/login` jumped straight into OAuth** instead of asking, because
  `Action::ShowLoginMenu` went the same way. It now opens the choice menu
  ("Login with …" / "Enter API key" / "Quit") that the fork's `/login` always did.
- Together these meant a fresh install advertised exactly one login method —
  browser sign-in — which without a registered app cannot run. The only way in was
  to set the environment variable before starting the program.
- Four registries now, and the fourth caught itself: the pager's
  `pager_builtin_triggers_are_reserved_in_shell` failed the moment `/apikey`
  became real, because `apikey` was not in the shell's `PAGER_COMMAND_KEYS` and a
  skill of that name could have shadowed it. Reserved. The new
  `apikey_registered_in_builtin_commands` is the test that would have caught the
  original gap.

The documentation had described `/apikey` and the in-app entry all along, in
`docs/slash-commands.md` and `docs/getting-started.md`. Third time in this release
that auditing the docs against the code found the code wrong.

### Fixed (voice had no working transport)

- **Speech-to-text could not work at all.** The re-base took upstream's 1.0.0 voice
  crate, which is streaming-only — it opens a WebSocket to `{api_base}/v1/stt` — and
  the rebrand pointed that at `api.chutes.ai`, where the route does not exist. Chutes
  serves Whisper-family models as ordinary chutes over request/response HTTP, which
  is exactly why the fork wrote a batch transport and made it the default. The two
  files implementing it, `pcm.rs` and `stt/batch.rs`, came across in Fase 1 and were
  never declared in any module, so neither was compiled; `SttMode`, `batch_api_base`
  and the pipeline's batch branch were not copied at all. What shipped was a feature
  whose only transport was a route the backend does not serve.
- Restored as a graft onto upstream's newer pipeline rather than a copy over it:
  the subprocess capture that replaced the in-process one no longer exposes
  `peak_meter()`, so the silence guard takes its running peak from the chunks with
  `pcm::peak_abs_i16_le` — which is what that module is for, and the batch path holds
  every sample anyway.
- `[voice].api_base` goes back to a closed loopback address. Streaming has no Chutes
  backend, so the default must fail immediately rather than resolve to a real host
  that cannot answer.
- Batch STT now posts through `xai_grok_http::shared_client()` instead of building a
  bare `reqwest::Client`. The fork's version skipped `CHUTES_EXTRA_CA_BUNDLE` — a
  proxy-terminated corporate network is precisely where that request would otherwise
  fail — and the connection health-checks.
- The STT transport is documented in `docs/configuration.md`, which had listed the
  media variables and said nothing about how speech is transcribed. The 0.4.x
  changelog had mentioned `stt_mode = "streaming"` all along.
- `xai-grok-http/src/extra_ca.rs` deleted: upstream 1.0.0 ships `xai-grok-extra-ca`,
  reading the same `CHUTES_EXTRA_CA_BUNDLE` with a size cap and rustls validation the
  fork's copy lacked, and `xai-grok-http` already calls it. The old module was dead
  code that a reader would have taken for the live one.

A sweep for the same failure mode — a source file that no `mod` declaration
reaches, and so is never compiled — found four in total across the tree, honouring
`#[path]` and `include!` to avoid false positives. All four are addressed in this
release; the sweep now returns nothing.

### Added (the Advisor is back)

The fourth undeclared file was `/advisor`, and unlike the others its backend had
not come across either — so this is a feature port rather than a registration,
though a small one, because the `[subagents.roles]` model-pin mechanism it hangs
off is upstream's and was already here.

- **The `advisor` built-in subagent**, invoked like any other through `task` with
  `subagent_type: "advisor"`: an on-demand senior reviewer for architecture,
  correctness, security and trade-offs. Read-only by construction — repository
  inspection, Context7, memory and web fetch, with no shell, no edit and no nested
  agents — so it can review a claim without being able to act on it. Defaults to
  maximum reasoning effort, because it is called sparingly for plans, blockers and
  completion claims, where review quality matters more than latency.
- **`/advisor`** turns it on or off and pins the model it uses, writing
  `[subagents.toggle].advisor` and `[subagents.roles.advisor].model`. An empty pin
  clears it and the advisor inherits the session's model. `true` is removed rather
  than written, so an untouched config file stays untouched.
- `advisor` is reserved in the shell's `PAGER_COMMAND_KEYS` — the check that caught
  the same omission for `/apikey`.

### Security (`agent serve` authentication)

Two CodeQL alerts pointed at `print_serve_startup_info` writing a secret to stderr.
Following them found the token itself was the problem, and the printing was the
least of it:

```rust
let raw = uuid::Uuid::new_v4().to_string().replace('-', "");
raw.chars().cycle().take(len).collect()   // len = 12
```

- **The token was 48 bits.** Twelve characters of a v4 UUID's hex, for a credential
  that authorises a server which runs shell commands and edits files, on a `/ws`
  endpoint with no rate limit. Now 32 bytes from the OS CSPRNG, base64url without
  padding. The `cycle()` also meant any length above 32 repeated the same UUID while
  looking like it added entropy.
- **The comparison was not constant-time.** `token == expected_secret` on `&str`
  returns at the first differing byte, so an unauthenticated caller could recover the
  token one byte at a time — roughly 12 x 16 attempts instead of 2^48. Now
  `ring::constant_time::verify_slices_are_equal`, with the length compared separately
  because it is not secret. `ring` moves from dev-dependency to dependency, which
  costs nothing: it was already in this crate's normal build graph via aws-config.
- **The token is printed only when stderr is a terminal.** An operator reading their
  own console still gets it; a redirected stderr — a systemd journal, a CI log,
  `2>serve.log` — no longer persists a live credential to disk. That was CodeQL's
  actual finding.
- **A non-loopback `--bind` now warns.** The transport is `ws://` and the token
  travels as a query parameter, so binding to a routable address puts both on the
  wire in cleartext. Previously silent.

Eight tests, including that a correct prefix does not authenticate and that an empty
presented token is rejected.

The other 308 open alerts were triaged by reading each flagged line on `main`: 263
are test fixtures, and the rest are `session_id` values (a UUID naming a local file,
not a credential), closed-loopback sentinels for the features policy disables, or
URLs whose `https` scheme `endpoint_policy::validate_endpoint_url` enforces where
CodeQL cannot see it through a `format!`. One more real finding needs no fix here:
`bin/trace_classify.rs` logged an API key, and this release deletes that binary.

Those 308 are now dismissed on GitHub, each with the reasoning for its group rather
than a blanket note; the 3 real ones stay open so the next scan closes them as fixed,
which is a truer record. The two Dependabot `lru` alerts are dismissed as `not_used`:
the advisory is an `IterMut` Stacked Borrows violation, and neither crate holding an
`LruCache` ever iterates it — `ratatui 0.29` memoizes layouts through `get`/`put`,
`aws-sdk-s3` has no `iter_mut()` at all. Bumping `aws-sdk-s3` to the release that
does require the patched `lru` was tried and reverted: it drags the aws-smithy stack
forward, wants rustc 1.94.1 against this repo's 1.94.0 pin, and still leaves
`ratatui 0.29` as a source. `docs/code-review-2026-08-01.md` CR-003 records the
evidence and what closing it would actually cost.

### Repository and CI

Landing the re-base exposed two checks that had never run against this tree, both
broken by the re-base itself rather than by anything in it:

- **The secrets scan pinned its allowlist to commit SHAs.** `.gitleaksignore` listed
  twelve reviewed fixtures as `<sha>:<path>:<rule>:<line>`; the re-base moved every
  one into a new commit and all twelve went stale together. Since this project tracks
  upstream by re-basing, that mechanism was guaranteed to break at every sync. Six
  fixtures are now allowlisted **by value** in `.gitleaks.toml`, which survives a
  rewrite, and six that exist only in old commits — from before upstream annotated
  the lines — keep SHA pins, in published history that will not move. Verified three
  ways: clean tree, clean 159-commit history, and a probe proving a realistic `ghp_`
  token and a real ES256 JWT sharing the fixtures' own header are still reported.
- **`cargo deny` had been skipped every run**, because gitleaks failed before it in
  the same job. It reported an undeclared git source — `our-forks/async-openai`,
  upstream's fork and the client the sampler uses for Chutes' OpenAI-compatible API —
  and five advisories, none with a patched release. Each is now ignored with a
  reachability check that was run rather than assumed: no `{:p}` anywhere for the
  crossbeam-epoch formatting bug, zero `git2::Remote::list` call sites, and a logging
  stack that does not depend on `rand` for the re-entrancy advisory.

- `git2` 0.20.4 for the `Buf` dereference advisory, carrying libgit2 1.9.1 -> 1.9.6.
  Its two other advisories have no fixed release yet.
- Releases now carry the six platform executables. All three existing releases have
  notes and **zero assets**, while the README said "take a binary from Releases" —
  so anyone not using npm had nowhere to download this program from. The workflow
  attaches them from the run that built them, and refuses to publish if one is
  missing.
- A social-preview card, three README badges, and the release notes taken from this
  file rather than written twice.

### Documentation

The documentation was audited against the code rather than proofread, which is a
different exercise and found things a read-through would not:

- **The README presented the project as SpaceXAI's, under SpaceXAI's logo.** This
  is a fork; upstream wrote most of the code and is credited for it, but not with
  authorship of this tree, and their mark is theirs. `CONTRIBUTING.md` likewise
  described upstream's "no external patches" policy as if it were ours.
- **`SECURITY.md` sent vulnerability reports to xAI's HackerOne programme** — to
  people who do not maintain this code and cannot fix it, disclosing our issue to
  a third party on the way.
- **Every install instruction was a 404.** `chutes.ai/cli/install.sh`,
  `install.ps1`, `chutes.ai/cli`, `chutes.ai/build/changelog`,
  `console.chutes.ai`, and the `docs.chutes.ai/build/overview` that `/docs` itself
  opened. Each replacement was checked with a request.
- **Config examples named `grok-4.5`**, which this build cannot serve, and
  `chutes-build update` was documented as the way to upgrade when it installs
  nothing.
- **The ACP extension methods were documented under `x.ai/`** while the handlers
  answer on `chutes.build/` — the page integrators read to know what to call.
- **Version pinning, the telemetry knobs and the coding-data row were documented
  as working controls.** All four version keys parse and nothing acts on them; the
  telemetry switches are compiled out; the privacy row is locked. Each now says so.
- **Two guide chapters existed on disk and shipped nowhere.** `USER_GUIDE` lists
  what `/docs` shows and what is extracted to `$CHUTES_BUILD_HOME/docs`, and the
  fork's Chutes-ecosystem and browser chapters were not in it — the same
  registration gap as the tool layer, in a different registry. Renumbered to 25 and
  26 and registered.
- Memory's owner-only file permissions and the absence of any coding-data
  retention control are now stated in `PRIVACY.md`, where a reader looks for them.

One code defect came out of the audit, in the direction that matters — the docs
were right and the code was not:

- **A custom model-catalog endpoint received the ambient `CHUTES_API_KEY`.**
  `PRIVACY.md` promises it never does and documents `CHUTES_MODELS_API_KEY` for
  that case; the re-base dropped the separate auth arm, so pointing
  `CHUTES_BUILD_MODELS_LIST_URL` at any host sent it your Chutes credential during
  startup. Restored, with the test asserting the separation.

### Changed

- The upstream sync procedure changes from "port selectively, never merge" to a
  merge of `upstream/main` with conflicts expected in the Chutes seams. See
  `docs/upstream-sync.md`.
- `project_picker` and the bundled `SKILL.md` files follow upstream and are
  removed.
- The coding-data retention row in settings is locked with a reason instead of
  offered: this build has no such control, and the extension behind the row
  already refused the call. The data-sharing banner never renders for the same
  reason.
- Rate-limit and tier-restriction messages no longer pitch an upstream
  subscription. The 429 message named SuperGrok and linked to it; the media
  tools told the model to sell it.
- `scripts/seam_sweep.py` (new) compares the values the previous release shipped
  against a re-based tree. It exists because a constant whose value came from
  upstream compiles fine and its tests assert against the constant, which is how
  the endpoint and credential defaults above survived a green gate.

### Fixed (identity)

Behavioural verification of the built binary, which the gate cannot do, found
the product still calling itself Chutes Build in the places users actually look:

- `--version` printed `grok 1.0.0`; `Usage:` printed `grok`, because `parse_cli`
  filters `argv[0]` against a launcher allowlist that did not include our own
  binary name.
- 983 occurrences of the bare product word in CLI help and error text had no
  rebrand rule at all, and 14 stderr diagnostics were prefixed `grok:` — the
  name of a program that does not exist.
- `models` offered `grok-4.5`: the model catalog had come across from upstream.
- The built-in agent profiles parsed as `grok-build-plan` while every caller
  asked for `chutes-build-plan`, so `--agent chutes-build-plan` resolved to
  nothing and plan mode fell back to the default profile.
- The session update and close notifications kept upstream's `_x.ai` extension
  namespace in the replay filter, the session store and the pager's method table,
  while the handlers answered on ours. (Written without the trailing slash on
  purpose: the rebrand rule that fixes those names would otherwise rewrite this
  sentence describing them, as it did once already.)

Then a real Chutes API key went in, and found what no static check had:

- **The ambient API key was read from `XAI_API_KEY`**, while every error message,
  every doc page and the fork itself said `CHUTES_API_KEY`. Anyone following the
  instructions was never authenticated. The auth-method id users write as
  `preferred_method` in `config.toml` was upstream's too. 304 occurrences across
  69 files.
- **570 places told the user to run `grok login`, `grok doctor`, `grok wrap …`**
  in help text, error messages and the bundled user guide. An instruction naming
  a binary that does not exist is worse than no instruction.
- **`update --check` reached `storage.googleapis.com`** for a channel pointer in a
  bucket that does not exist — a third update endpoint, hidden from the seam
  sweep because rustfmt had wrapped its value onto the next line. Deadened at
  loopback like the other two.

- **The notification-hook tests were writing the whole process environment,
  API key included, into files in the crate root.** They interpolated an
  unquoted Windows path into `sh -c`, so the redirect missed the temp directory.
  Thirty such files had accumulated and been committed to the re-base branch;
  history was purged, `.gitignore` now blocks the pattern, and the keys were
  rotated. The branch had never been pushed.
- **A blank `CHUTES_API_KEY` counted as set**, so a CI job exporting a secret
  that does not exist was told it was using an API key and then failed to
  authenticate. Blank is unset now, as it already was for per-model `env_key`.

With those fixed, a live run authenticates, lists the 15 Chutes models from
`llm.chutes.ai`, and completes a turn through the Auto router in about 13
seconds, talking only to `api.chutes.ai`, `llm.chutes.ai` and the configured
router — never to `x.ai` or `grok.com`, and writing nothing outside
`$CHUTES_BUILD_HOME`.


## [0.4.3] - 2026-08-03

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
- The encrypted prompt templates no longer depend on how the repository was
  checked out. Generator and staleness test compared raw `include_str!` bytes,
  so arrays built on a CRLF working tree matched only on Windows; both sides
  now fold line endings first, which also makes the decrypted prompt
  byte-identical on every platform.
- Two pager tests no longer depend on the machine they run on: one budgeted its
  wait in poll counts rather than elapsed time, and two built past instants by
  subtracting more than the machine's uptime, which panics on a recently booted
  box or a fresh CI runner.
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
- Listed the browser tool's full action set — what each reading action reports
  and how elements are addressed — in the web and browser guide, and documented
  `CHUTES_EXTRA_CA_BUNDLE` alongside the endpoint settings.
- Documented both headless streaming formats and what each line carries in the
  CLI reference, and noted in the slash-command list that `/compact` can now be
  stopped while it runs.

## [0.4.2] - 2026-08-01

### Added

- `/undo` is accepted as an alias for `/rewind`.
- Added repository-wide coding-agent guidance and editor defaults.
- The npm launcher archive now includes the changelog and security policy.

### Changed

- Reviewed Chutes Build upstream through `0.2.117` and recorded the selective
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
