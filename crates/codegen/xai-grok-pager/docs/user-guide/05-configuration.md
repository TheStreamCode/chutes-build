# Configuration

User state defaults to `~/.chutes-build`; `CHUTES_BUILD_HOME` relocates the
complete state tree. Project instructions use `AGENTS.md`, project memory uses
`memories.md`, and project-scoped `.chutes-build/` configuration remains in the
repository.

## Common environment variables

| Variable | Purpose |
| --- | --- |
| `CHUTES_BUILD_HOME` | Complete user state root. |
| `CHUTES_API_KEY` | Ambient credential for official Chutes HTTPS endpoints. |
| `CHUTES_MODELS_API_KEY` | Dedicated custom model-catalog credential. |
| `CHUTES_BUILD_OAUTH2_CLIENT_ID` | Registered OAuth application ID. |
| `CHUTES_BUILD_OAUTH2_CLIENT_SECRET` | OAuth secret used for exchange/refresh and never persisted. |
| `CHUTES_FALLBACK_MODELS` | Ordered comma-separated fallback models. |
| `CHUTES_STRICT_MODEL=1` | Disable automatic fallback. |
| `CHUTES_BUILD_DEFAULT_MODEL` | Default model ID; `model-router` selects Auto. |
| `CHUTES_BUILD_LOG_SAMPLING=1` | Opt in to local metadata-only sampling diagnostics. |
| `CHUTES_WEB_SEARCH_PROVIDER` | `auto`, `duckduckgo`, or `brave`. |
| `BRAVE_SEARCH_API_KEY` | Dedicated Brave Search key. |
| `CHUTES_BROWSER_EXECUTABLE` | Chrome/Edge executable override. |
| `CHUTES_BROWSER_HEADFUL=1` | Show the isolated automation browser. |
| `CHUTES_FFMPEG_EXECUTABLE` | FFmpeg override for video/local media controls. |
| `CHUTES_MAX_MEDIA_BYTES` | Media download limit: 128 MiB default, 512 MiB hard ceiling. |
| `CHUTES_MAX_INPUT_ASSET_BYTES` | Workspace media input: 64 MiB default, 512 MiB hard ceiling. |

Official endpoint overrides fail closed on insecure schemes, URL credentials,
custom ports, untrusted hosts, and private/special-use DNS results. Local forks
may explicitly set `CHUTES_ALLOW_INSECURE_ENDPOINTS=1`; custom endpoints still
do not inherit ambient credentials. Custom Context7 endpoints require
`CONTEXT7_ALLOW_INSECURE_ENDPOINTS=1` and never receive `CONTEXT7_API_KEY`.

Use `/model` and `/effort` for model-specific controls. Explicit live catalog
menus win over bundled compatibility data; unknown generations do not receive
guessed reasoning fields.

Remote session write-back, sharing, workspace exposure, trace upload,
telemetry, and automatic updates cannot be enabled through configuration.
