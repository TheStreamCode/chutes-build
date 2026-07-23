# Architecture

Chutes Build preserves the proven upstream Rust agent runtime
while replacing public identity, authentication, provider integration, and
privacy-sensitive cloud behavior.

## Runtime layers

1. **Terminal and CLI:** command parsing, interactive rendering, sessions,
   approvals, keyboard input, and the animated Chutes Build welcome identity.
2. **Agent loop:** prompt construction, planning, tool dispatch, compaction,
   goals, advisor consultation, and subagent orchestration.
3. **Chutes routing:** model selection, live capability lookup, ordered fallback,
   image-capability delegation, retry classification, and streaming safeguards.
4. **Tools:** coding/filesystem tools, MCP, Context7, web search, isolated browser
   control, and native Chutes media invocation.
5. **Local state:** configuration, credentials, sessions, logs, exports, and
   secret-filtered `memories.md` maintenance.

## Routing behavior

`Auto (Chutes Router)` is inserted as the first catalog entry and is selected by
default when the user has no stored model preference. Its stable local ID is
`model-router`; only that virtual ID is dispatched to the router endpoint. A
concrete selection is attempted first, followed by `CHUTES_FALLBACK_MODELS`,
then Auto. Duplicates are removed. Only transient/model-unavailable failures
before stream start can advance the chain. Authentication, permission,
invalid-request, and mid-stream errors do not silently switch models.

Each fallback attempt is rebuilt from the unchanged logical request after its
candidate model is selected. This is important for mixed-family chains: Qwen,
Kimi, DeepSeek, GLM, MiniMax, and future generations cannot inherit one
another's template switches or scalar reasoning fields.

Vision inputs are checked against the live Chutes model catalog. If the selected
model cannot accept images, the request is delegated to a vision-capable Chutes
route. Media generation uses the dedicated Chutes media catalog rather than
legacy provider tools.

New media generation returns a versioned `MediaArtifact` through the tool
protocol. The model receives a compact path receipt, while ACP and the pager
retain kind, MIME type, size, provider/model provenance, cost, and sidecar path.
The pager prepares image and poster previews serially in a background worker,
only while inference is idle. Images are reduced to a 1280-pixel display bound,
individual prepared artifacts are capped at 8 MiB, and the shared CPU-side
cache is capped at 16 MiB. Video playback keeps one visible frame and a bounded
two-frame decode queue; pausing or dropping playback cancels its single-threaded
FFmpeg process. Music and speech use process-owned pause/resume, seek, volume,
duration, and lazy waveform state with native-open fallback. Media-only activity
uses the low-frequency TUI tick instead of the normal animation tick. Legacy
media variants remain readable for session compatibility.

## Reasoning compatibility

Reasoning behavior is resolved by one registry in `chutes-build-core`. Exact
known model generations map the user-facing mode to their native chat-template
control. Explicit capability menus returned by Chutes or supplied in model
configuration take precedence. Unknown future generations keep explicit
catalog controls but do not inherit guessed behavior from a broad family-name
match.

Auto and fixed-reasoning models expose no effort selector. This prevents a
model-specific field from constraining the router or being silently ignored by
a model whose reasoning mode cannot be changed. See
[Model reasoning compatibility](model-reasoning-compatibility.md).

## Inference transport

All sampling clients reuse a process-wide HTTP connection pool. TCP no-delay,
bounded connect timeouts, HTTP/2 keepalive, and an HTTP/1 fallback are configured
centrally. SSE chunks are parsed and forwarded as they arrive; there is no
default application-level replay buffer. Fallback is permitted only before a
successful stream begins, so a response never changes model mid-stream.

## Official Chutes source policy

The main agent and every worker receive the same primary-source rule. Before
answering or making implementation claims about Chutes, they consult both
[`chutes.ai/docs`](https://chutes.ai/docs) and
[`chutes.ai/news`](https://chutes.ai/news). Direct official pages and indexes
are preferred over broad web queries; third-party sources are supporting
evidence only. If official coverage is absent or unavailable, the agent must
label the claim as unverified and separate it from inference. Queries must not
contain credentials, private code, prompts, or repository content.

## Account usage surface

Account reads start subscription usage, quotas, optional aggregate quota usage,
and optional model statistics concurrently. Model statistics are opt-in. If the
aggregate quota response is absent or empty, the client reads each quota through
the documented per-chute endpoint concurrently and normalizes the results.

The shell preserves every independently enforced usage window and separately
selects the highest consumed percentage for warnings and severity. The pager
shows the monthly and rolling four-hour percentages in the compact status item
when both are available, exposes all windows through `/usage`, and preserves
each window type even when the API omits a reset timestamp.

## Advisor and workers

The executor owns the main loop and all mutations. The advisor is a read-only
subagent with the current conversation context; it returns recommendations that
the executor may accept or reject. Worker subagents can run concurrently in
foreground/background modes, wait as a group, and use isolated worktrees.
Nesting is bounded to prevent unreviewable recursive swarms.

## Privacy boundary

Outbound provider calls are allowlisted. Telemetry and remote error pipelines
are hard-disabled, traces export locally, the upstream relay defaults to a
closed loopback endpoint, and public commands that depended on upstream cloud
services are not registered. See [PRIVACY.md](../PRIVACY.md).

## Module ownership

This is a fork, not a rewrite: most of the runtime is upstream `grok-build`
infrastructure, unmodified or lightly adapted. Renaming every `xai-*` crate to
match the fork's identity is explicitly out of scope (it would complicate
future upstream syncs for no functional benefit) — this section documents the
*intended* ownership boundary instead, so a future change lands in the layer
it actually belongs to. It's a map of intent, not a strict enforcement
boundary; treat crate placement as one useful signal among several, not a
guarantee.

**Chutes-owned** (product identity, provider integration, privacy posture):

- `chutes-build-core` — the domain layer for everything Chutes-specific that
  isn't a user-facing tool: model-router dispatch and virtual-model handling,
  live capability catalog + cache, reasoning-compatibility registry, media
  catalog/invocation client, Context7 client, credential-endpoint trust policy
  and SSRF-safe DNS resolution, memory secret-filtering, and wellness nudges.
- `xai-grok-tools::implementations::chutes` — the agent-callable tools backed
  by the above (media generation/discovery, Context7 lookup, OCR
  transcription, account usage, isolated browser control). Despite living
  inside the (upstream) generic tool-runtime crate, this module is
  Chutes-owned; everything else under `implementations/` is upstream-style
  generic tooling (`read_file`, `bash`, `grep`, `web_search`, LSP, ...).
- `xai-grok-voice`, `xai-grok-models` — Chutes-hosted STT integration and the
  trimmed Chutes default model catalog, respectively.
- `xai-grok-secrets` — upstream-authored, but per this fork's policy it is
  now the single canonical secret-detection/redaction layer; Chutes-specific
  call sites (memory persistence, Context7's outbound guard) and upstream
  call sites (telemetry/log sanitization) both route through it rather than
  keeping separate detectors.
- Product-identity slices scattered through otherwise-upstream crates:
  rebranded welcome/auth screens and "Sign in with Chutes" OAuth in
  `xai-grok-pager`/`xai-grok-shell`, plus compile-time product policy in
  `chutes-build-core::product` that disables remote session sharing/search,
  workspace exposure, feedback/data-retention controls, trace upload, and
  automatic updates.

**Retained upstream infrastructure** (the proven agent runtime this fork
builds on, not specific to any provider):

- `xai-grok-agent` — prompt construction, planning, subagent/advisor
  orchestration, goal tracking.
- `xai-grok-shell`, `xai-grok-shell-base` — session lifecycle, config
  resolution/persistence, credential provider, ACP session handling.
- `xai-grok-sampler`, `xai-grok-sampling-types` — the HTTP/streaming client
  and wire types for chat completions (Chutes-specific dispatch, e.g. the
  model-router host substitution, is a small, explicitly-commented carve-out
  inside otherwise-generic client code, not a separate Chutes crate).
- `xai-tool-runtime`, `xai-tool-protocol`, `xai-tool-types` — the tool trait,
  wire protocol, and shared type definitions any tool (Chutes-owned or not)
  is built on.
- `xai-grok-tools` (outside `implementations::chutes`) — the generic tool
  implementations: filesystem, shell, search, LSP, MCP, skills.
- `xai-grok-pager`, `xai-grok-pager-render` — the TUI: rendering, input,
  scrollback, views. Chutes product identity is layered on top (see above),
  the rendering engine itself is upstream.
- `xai-grok-workspace` — permission model, sandboxing, worktrees.
- `xai-grok-auth` — OAuth/credential machinery in general; the Chutes issuer
  configuration is a caller-supplied parameter, not a fork of this crate.

Crates not listed here are unremarkable utility/support code (formatting,
markdown rendering, terminal primitives, telemetry plumbing that is disabled
by default, etc.) and follow the same rule of thumb: if it encodes a decision
specific to Chutes as a provider or to this fork's product identity, it's
Chutes-owned; if it would make equal sense in the upstream project, it's
retained infrastructure.
