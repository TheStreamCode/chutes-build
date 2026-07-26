# Configuration

Chutes Build layers command-line arguments, environment variables, user
configuration, project configuration, and trusted project instructions. Use
`chutes-build inspect --json` to inspect the effective non-secret structure.

## State and project files

User state defaults to `~/.chutes-build`. `CHUTES_BUILD_HOME` relocates the
complete tree, including credentials, sessions, logs, trace exports, plugins,
user roles/personas, and the managed bundled-agent cache.

Project-scoped `.chutes-build/` content remains inside the repository and has
higher precedence where supported. Project instructions use `AGENTS.md`; local
project memory uses `memories.md`.

Chutes Build does not combine the default user state with a custom state root.

## Authentication and model routing

| Variable | Purpose |
| --- | --- |
| `CHUTES_API_KEY` | Ambient Chutes API credential for allowlisted official HTTPS endpoints. |
| `CHUTES_MODELS_API_KEY` | Dedicated credential for a custom model-catalog endpoint. |
| `CHUTES_BUILD_OAUTH2_CLIENT_ID` | Registered Chutes OAuth application ID. |
| `CHUTES_BUILD_OAUTH2_CLIENT_SECRET` | Confidential OAuth secret, used for exchange and refresh but never persisted. |
| `CHUTES_BUILD_DEFAULT_MODEL` | Default model ID; use `model-router` for Auto. |
| `CHUTES_FALLBACK_MODELS` | Ordered comma-separated model fallback chain. |
| `CHUTES_STRICT_MODEL=1` | Disable automatic fallback. |
| `CHUTES_ROUTER_BASE_URL` | Compatible router endpoint override. |
| `CHUTES_INFERENCE_BASE_URL` | Compatible inference endpoint override. |
| `CHUTES_API_BASE_URL` | Compatible account/media API override. |

Ambient Chutes credentials are never inherited by arbitrary custom inference
or catalog endpoints. Custom models must declare their own `api_key` or
`env_key`.

Endpoint overrides fail closed unless they use an allowlisted Chutes HTTPS host
with no URL credentials and the default port. Local forks may set
`CHUTES_ALLOW_INSECURE_ENDPOINTS=1`, which relaxes endpoint/DNS trust checks but
does not make ambient credentials transferable to arbitrary model endpoints.
Use this opt-in only for an isolated development environment.

## Reasoning

Use `/model` to select Auto or a concrete model and `/effort` to select one of
that exact model's published options. Explicit catalog capability menus take
precedence over bundled compatibility data. Unknown future model generations
do not inherit guessed reasoning controls from a provider prefix.

`Auto (Chutes Router)` has no model-specific effort selector because its target
may change between requests.

## Web, Context7, and browser

| Variable | Purpose |
| --- | --- |
| `CHUTES_WEB_SEARCH_PROVIDER` | `auto`, `duckduckgo`, or `brave`. |
| `BRAVE_SEARCH_API_KEY` | Dedicated Brave Search credential. |
| `CONTEXT7_API_KEY` | Optional key sent only to official Context7 HTTPS endpoints. |
| `CONTEXT7_BASE_URL` | Context7-compatible endpoint override. |
| `CHUTES_BROWSER_EXECUTABLE` | Chrome/Edge executable override. |
| `CHUTES_BROWSER_HEADFUL=1` | Show the isolated automation browser. |

Custom Context7 endpoints require `CONTEXT7_ALLOW_INSECURE_ENDPOINTS=1` and
never receive `CONTEXT7_API_KEY`. Browser automation uses a temporary profile,
loopback DevTools, disabled sync/background updates, and workspace-only
screenshot destinations.

## Media and voice

| Variable | Default and purpose |
| --- | --- |
| `CHUTES_OUTPUT_DIR` | Generated artifact directory. |
| `CHUTES_MAX_MEDIA_BYTES` | 128 MiB download limit; values are clamped to a 512 MiB hard ceiling. |
| `CHUTES_MAX_INPUT_ASSET_BYTES` | 64 MiB workspace-input limit; values are clamped to a 512 MiB hard ceiling. |
| `CHUTES_FFMPEG_EXECUTABLE` | FFmpeg/ffprobe override for video inspection and local media controls. |
| `CHUTES_WARMUP` | Enable the compatible Chutes media warmup behavior. |
| `CHUTES_COLD_START_RETRIES` | Configure bounded media cold-start retries. |
| `CHUTES_ALLOW_UNKNOWN_PARAMS` | Allow model parameters absent from the discovered schema. |
| `CHUTES_PROVENANCE` | Write generated-media provenance sidecars. |

Non-JSON media responses stream to temporary files rather than accumulating in
memory. Error and JSON bodies are capped at 32 MiB. Final workspace persistence
uses create-new writes and rolls back partial bundles.

Voice recording is always manually activated. Use `/voice` for controls and
`--no-memory` when a session must avoid local memory recall and writes.

## Diagnostics

`CHUTES_BUILD_LOG_SAMPLING=1` or `--log-sampling` enables local sampling
metadata under the state root. It records operational fields such as model,
endpoint, token counts, timing, and error metadata, not request/response bodies
or credential values. Provider-supplied error strings should still be reviewed
before sharing logs.

Product telemetry, remote error reporting, trace upload, session sharing,
remote workspace exposure, upstream managed configuration, and automatic
updates remain disabled regardless of inherited configuration values.

## Related references

- [Getting started](getting-started.md)
- [Model reasoning compatibility](model-reasoning-compatibility.md)
- [Privacy](../PRIVACY.md)
- [Security review](security-review.md)
- [CLI reference](cli-reference.md)
