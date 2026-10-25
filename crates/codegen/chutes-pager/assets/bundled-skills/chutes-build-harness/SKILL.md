---
name: chutes-build-harness
description: Chutes Build CLI harness reference. Use when a tool, command, permission, session, configuration, or capability fails, or when you need to know which harness features exist and how to operate them.
---

# Chutes Build harness reference

This skill documents the CLI that is running you. It is not the Chutes platform
reference; use the `chutes-platform` skill for API hosts, models, routing, and
billing questions. Load this skill when something in the harness fails, when a
capability seems missing, or before answering a question about how Chutes Build
itself works.

## What this harness is

Chutes Build is a terminal coding agent. One process owns a session: a working
directory, a conversation, a finalized toolset, permission state, and optional
child agents. Sessions are local under `~/.chutes-build` (override with
`CHUTES_BUILD_HOME`). There is no telemetry, no login store, and no automatic
self-update; the only credential is the Chutes API key.

Modes: the interactive TUI, and headless `-p/--single` (plus `chutes-build
agent`). Headless is how automated checks should run: `--permission-mode
bypassPermissions` when the user asked for unattended work, `--cwd` to pin the
workspace, `--max-turns` to bound a loop, `--output-format plain|json|streaming-json`.

## Finding out what this build has

- `chutes-build --help` lists flags and every subcommand.
- `chutes-build inspect --cwd <dir>` prints effective config, permissions,
  skills, and project instructions for that directory.
- `chutes-build doctor` checks terminal, clipboard, colors, and input support.
- `chutes-build models` lists the live Chutes catalog; `chutes-build account`
  and `chutes-build usage <session>` show plan, quotas, and token/cost records.
- `chutes-build sessions list|search|restore`, `export <session>`,
  `trace <session>`, `du`, `mcp`, `plugin`, `memory`, `worktree`, `leader`,
  `completions` cover the rest.
- Embedded user guide: `~/.chutes-build/docs/user-guide/*.md`, especially
  `01-getting-started.md`, `04-slash-commands.md`, `05-configuration.md`,
  `25-chutes-ecosystem.md`, and `26-config-reference.md`.

## Tools

Files and search: `read_file` (text, images, PDF, PPTX, notebooks),
`write`/`edit`/`apply_patch`/`search_replace`, hashline variants when the
toolset selects them, `grep`, `glob`, `ls`.
Execution: `run_terminal_command` (foreground), background commands, `monitor`
for polling and log watching, task output/wait/kill tools.
Planning and delegation: `todo_write`, plan mode, `spawn_subagent` and
`get_command_or_subagent_output`, `workflow` for Rhai scripts (including
`/deep-research`), `ask_user_question` for genuine blocking decisions.
Knowledge: `skill` loads a skill such as this one; `search_tool` and `use_tool`
reach MCP servers; `web_search` uses a direct public provider because the
Chutes router has no Responses API; `web_fetch` retrieves a URL.
Chutes ecosystem: `generate_media` (image, video, music, speech; omit `model`
to auto-select, or list/describe first), `list_media_models`,
`describe_media_model`, `ocr_page`, `extract_document`, `transcribe_audio`,
`embed_texts`. `browser` drives a local Chrome/Edge session when configured.

## Permissions and safety

Permission modes are `default`, `acceptEdits`, `auto`, `dontAsk`,
`bypassPermissions`, and `plan`. The mode is a launch flag
(`--permission-mode`); there is no separate `/permissions` slash command. `--allow`/`--deny` rules and the permission
profile decide each call; a denied call means the user declined it, so adapt the
request instead of retrying it verbatim. Confirm destructive, shared, or
irreversible actions unless the user explicitly authorized them. Never put
credentials, prompts, repository contents, or secrets into external requests,
logs, or files.

## Slash commands (interactive TUI)

`/model`, `/effort`, `/usage`, `/account`, `/imagine [model] <prompt>`,
`/imagine-video [model] <prompt>`, `/deep-research <query>`, `/workflow`,
`/settings`, `/docs`, `/help`, `/feedback` (opens the public GitHub issue form;
without a browser it prints the URL and copies the report). `/imagine` and
`/imagine-video` require `generate_media`; they are not present in headless
mode, where you call the tool directly.

## Configuration and environment

User config lives at `~/.chutes-build/config.toml`; managed configuration and
`requirements.toml` can pin or deny behavior, and `inspect` shows the result.
Useful environment variables: `CHUTES_API_KEY`, `CHUTES_BUILD_HOME`,
`CHUTES_BUILD_MODEL`, `CHUTES_BUILD_FORCE_LEGACY_CONSOLE=0` on legacy Windows
consoles, sandbox and web-fetch switches from `05-configuration.md`. Config is
read at session start; changing it mid-session requires a new session.

## Troubleshooting

- A tool is missing or not advertised: it is gated by the active toolset,
  capability mode, or `--tools`/`--disallowed-tools`. Check `inspect`, use an
  equivalent advertised tool, or ask the user to enable it. MCP-only tools need
  `search_tool`/`use_tool`.
- `401`/`403`: the key is missing, invalid, or not allowed for that endpoint.
  Never echo it. Confirm with `chutes-build account`.
- `429`: rate or quota limit. Check `account`/`usage`, back off, and do not
  retry in a tight loop.
- `404` "Chute not found, or does not belong to you": the account API needs the
  chute id, not the public slug. Prefer a tool that resolves through the live
  catalog, or resolve the id first. For model ids, `models` is the source of
  truth.
- Media `400 Invalid input parameters`: call `describe_media_model`, use the
  cord it names, and remember some v2 chutes wrap fields in `args`. Try an
  equivalent model from `list_media_models`.
- Media `500`/`503` or "No infrastructure available": platform capacity, not a
  payload bug. Retry later or pick another model; report the limitation.
  A media `504` after about ten minutes is a gateway timeout, not media output.
- `extract_document` 503: the document chute has no warm instance. Retry, or
  read the PDF with `read_file` (`format: "text"` for text extraction).
- `web_search` empty or failing: retry with a shorter query; the direct
  provider is external and may rate-limit. `web_fetch` failures name the status.
- A skill appears missing: bundled skills are materialized under
  `~/.chutes-build/bundled/skills` at startup; user and repository skills shadow
  them. List skills through the `skill` tool description.
- `update --check` uses the npm registry in this fork and prints the npm update
  command; self-update is disabled. Update with
  `npm install -g chutes-build@latest`.
- Long output or context pressure: read ranges instead of whole files, grep
  before reading, summarize tool results, and let the harness compact the
  conversation. Do not re-derive facts already established.
- A loop: stop repeating an identical failing call. Change the approach, use a
  different tool, or surface the blocker.

## Performance habits

Batch independent tool calls in one response; the harness dispatches them in
parallel. Prefer `grep`/`glob`/ranged reads over dumping large files. Use
background commands for long builds and tests. Keep the prompt prefix stable for
Chutes prompt caching, avoid unnecessary web fetches, and delegate independent
work to subagents only when the user asked for it or the task genuinely benefits
from isolation.

## Accuracy rules

Verify claims against tool output, local docs, and the repository. When a
capability is blocked, say what is blocked and what you tried. When you report a
failure, include the exact tool name, status, and message so the user can act on
it. Never invent a flag, command, config key, or version; read `--help`,
`inspect`, or the embedded guide first.
