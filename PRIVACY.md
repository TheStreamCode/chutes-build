# Privacy

Chutes Build is privacy-first: it collects no product analytics, emits no
telemetry, performs no remote error reporting, and does not upload traces or
sessions. There is no hidden opt-in or installation identifier.

## Local data

The CLI may store configuration, an encrypted or OS-protected credential entry
where supported, sessions, logs, local exports, and user memory under
`~/.chutes-build` (or `CHUTES_BUILD_HOME`). Project memory is maintained in
`memories.md`. Known secret formats are filtered before memory writes, but users
should still avoid entering secrets into prompts.

`chutes-build trace` always creates a local archive. Remote session sharing and
upstream managed configuration commands are disabled. Automatic update checks
are disabled.

Sampling diagnostics are disabled by default. When explicitly enabled with
`--log-sampling` or `CHUTES_BUILD_LOG_SAMPLING=1`, the local
`logs/sampling.jsonl` file records operational metadata such as model, endpoint,
effort, token counts, errors, and SSE chunk byte lengths. It does not record
credential values or prefixes, request/response bodies, or normal SSE chunk
text. Provider-supplied error messages may still appear and should be reviewed
before sharing a diagnostic archive.

Prompt-efficiency debug events and the optional deterministic benchmark contain
only local numeric size estimates and built-in fixture data, never prompt or
source contents. Generated-media preview and audio playback stay local:
image/video decoding uses local image codecs and FFmpeg tools when installed,
and audio uses `ffplay` or the operating system's default player. Preview work
is bounded, off-thread, serialized, and deferred while inference is active. No
remote preview, thumbnail, codec, waveform, or playback service is used.

## Outbound connections

Connections occur only when required by an explicit feature:

| Destination | Purpose | Data sent |
| --- | --- | --- |
| `model-router-ten.vercel.app` | default Chutes-compatible model routing | prompts, selected tool schemas, model responses, Chutes API credential |
| `llm.chutes.ai` | Chutes inference and model catalog | prompts/model requests and Chutes API credential |
| `api.chutes.ai` | Chutes account and media APIs | requested media parameters/assets and Chutes API credential |
| `chutes.ai/docs`, `chutes.ai/news` | mandatory primary-source verification for Chutes topics | requested official page URLs; no Chutes API credential |
| `context7.com` | current library documentation | library names and documentation queries; credentials and known secrets are rejected |
| DuckDuckGo or Brave Search | web search | search query; Brave receives only its dedicated key |
| User-selected web pages | web fetch/browser automation | normal HTTP/browser traffic required by the requested action |
| User-configured MCP servers/plugins | external tools | data required by the selected tool and its own configuration |

The Chutes API credential is accepted only for allowlisted Chutes and router
hosts. It is never reused for web search, Context7, arbitrary websites, MCP
servers, or browser automation.

For Chutes research, the agent prefers direct official pages and indexes over a
broad search query. Search and page requests must not include credentials,
private code, prompts, or repository content. If web access is disabled, the
agent reports that current official verification could not be completed.

## Browser isolation

Agentic browser control launches a temporary Chrome/Edge profile and connects
through a loopback-only DevTools endpoint. It does not attach to the user's
normal browser profile. Sync and background network features are disabled.
Screenshots are restricted to the active workspace.

Browser actions can still disclose data to websites the user asks the agent to
visit. Review navigation, form submissions, uploads, and authenticated actions
before approval.

## Disabling stateful features

- Start with `--no-memory` to disable agent memory for a session.
- Disable web search with `--disable-web-search`.
- Do not configure external MCP servers or plugins you do not trust.
- Set `CHUTES_STRICT_MODEL=1` to disable model fallback.

Privacy regressions are security issues and should be reported through the
private process in [SECURITY.md](SECURITY.md).
