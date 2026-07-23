# Privacy

Chutes Build is privacy-first: it collects no product analytics, emits no
telemetry, performs no remote error reporting, and does not upload traces or
sessions. There is no hidden opt-in or installation identifier.

## Local data

The CLI may store configuration, an encrypted or OS-protected credential entry
where supported, sessions, logs, local exports, and user memory under
`~/.chutes-build` (or `CHUTES_BUILD_HOME`). Plugins, user roles/personas, and
managed bundled-agent definitions use that same root; setting
`CHUTES_BUILD_HOME` does not fall back to those resources in the default home.
Project memory is maintained in `memories.md`. Known secret formats are filtered
before memory writes, but users should still avoid entering secrets into
prompts.

`chutes-build trace` always creates a local archive. Remote session sharing and
search, remote workspace exposure, upstream managed configuration commands, and
automatic update checks are disabled. Session storage is forced to local mode
even if inherited write-back settings are present. Updates are installed
manually.

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

Connections occur when required by startup model/account discovery or an
explicit hosted feature:

| Destination | Purpose | Data sent |
| --- | --- | --- |
| `model-router-ten.vercel.app` | default Chutes-compatible model routing | prompts, selected context/tool schemas, model inputs, Chutes API credential |
| `llm.chutes.ai` | Chutes inference, model catalog, semantic memory, voice transcription, OCR, and vision | model requests and the selected inputs: prompts/context, memory chunks, audio, images, PDF pages, or sampled video frames; Chutes API credential |
| `api.chutes.ai` | Chutes account and media APIs | account requests, media parameters and user-selected media assets; Chutes API credential |
| `api.chutes.ai/idp` | OAuth sign-in and refresh | authorization code or refresh token, PKCE verifier, public/custom client ID, and a custom client secret only when configured |
| `chutes.ai/docs`, `chutes.ai/news` | mandatory primary-source verification for Chutes topics | requested official page URLs; no Chutes API credential |
| `context7.com` | current library documentation | library names and documentation queries; credentials and known secrets are rejected |
| DuckDuckGo or Brave Search | web search | search query; Brave receives only its dedicated key |
| User-selected web pages | web fetch/browser automation | normal HTTP/browser traffic required by the requested action |
| User-configured MCP servers/plugins | external tools | data required by the selected tool and its own configuration |

The ambient Chutes API credential and cached Chutes session token are accepted
only for allowlisted HTTPS Chutes and router hosts. They are never reused for a
custom endpoint, web search, Context7, arbitrary websites, MCP servers, or
browser automation. Custom inference models must declare a dedicated
`api_key`/`env_key`; a custom model-catalog endpoint uses
`CHUTES_MODELS_API_KEY`.

Model catalog and account usage requests may run during startup after
authentication. Prompts and repository content are not sent by those discovery
requests. They leave the machine only in a model/tool request that needs them.

Semantic memory is local at rest, but hybrid recall uses a Chutes-hosted
embedding model and therefore sends the selected memory text chunks to Chutes.
Use `--no-memory` to disable both memory recall and writes for that session.
Voice capture is manual and sends the recorded audio only after activation.
OCR, vision, video analysis, and media generation send only the inputs selected
for that operation.

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

`chutes-build mcp list --json` intentionally reports configuration structure,
not credential material: environment/header values and URL credentials, query,
and fragment components are redacted. MCP servers and plugins still receive
the data required by tools the user enables, so their own privacy policies
apply.

## Disabling stateful features

- Start with `--no-memory` to disable agent memory for a session.
- Disable web search with `--disable-web-search`.
- Do not configure external MCP servers or plugins you do not trust.
- Set `CHUTES_STRICT_MODEL=1` to disable model fallback.

Privacy regressions are security issues and should be reported through the
private process in [SECURITY.md](SECURITY.md).
