# Configuration

User state defaults to `~/.chutes-build`. Override the root with
`CHUTES_BUILD_HOME`. Project instructions use `AGENTS.md`; project memory uses
`memories.md`.

Common environment variables:

| Variable | Purpose |
| --- | --- |
| `CHUTES_API_KEY` | Chutes API credential |
| `CHUTES_ROUTER_BASE_URL` | compatible Chutes router override |
| `CHUTES_FALLBACK_MODELS` | ordered comma-separated fallback models |
| `CHUTES_STRICT_MODEL=1` | disable automatic fallback |
| `CHUTES_BUILD_DEFAULT_MODEL` | default model ID; use `model-router` for Auto |
| `CHUTES_BUILD_LOG_SAMPLING=1` | opt in to local sampling diagnostics |
| `CHUTES_WEB_SEARCH_PROVIDER` | `auto`, `duckduckgo`, or `brave` |
| `BRAVE_SEARCH_API_KEY` | dedicated Brave Search credential |
| `CHUTES_BROWSER_EXECUTABLE` | Chrome/Edge executable override |
| `CHUTES_BROWSER_HEADFUL=1` | show the isolated automation browser |
| `CHUTES_FFMPEG_EXECUTABLE` | FFmpeg override for video inspection |

The CLI supports local TOML configuration inherited from the agent runtime.
Do not enable unknown upstream cloud, telemetry, relay, or upload settings:
those paths are intentionally disabled in Chutes Build.

Use `/model` to select Auto or a concrete model and `/effort` to change the
reasoning mode when the current model supports it. Model-specific capability
menus take precedence over bundled compatibility data. Unknown future models do
not receive guessed reasoning fields.
