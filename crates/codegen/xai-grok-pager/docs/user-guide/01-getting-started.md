# Getting Started

Chutes Build is a terminal coding agent for the Chutes ecosystem.

## Build

```powershell
cargo build -p chutes-build --release
```

Run `target\release\chutes-build.exe` on Windows or
`target/release/chutes-build` on macOS and Linux.

## Authenticate

Run `chutes-build` and press `l` to sign in with Chutes through the browser, or
press `k` to enter an API key. For a non-interactive environment:

```powershell
$env:CHUTES_API_KEY = "your-api-key"
chutes-build
```

`chutes-build login` stores an API key through a hidden prompt. `/login` and
`/apikey` reopen the corresponding TUI choices.

`Auto (Chutes Router)` is the first model choice and the default when no model
preference has been saved. Its stable ID is `model-router`. Use `/model` in an
interactive session or `--model <id>` at launch to select a concrete model. Run
`chutes-build models` to inspect the current catalog, or add `--json` for
machine-readable output.

When a concrete model supports configurable reasoning, the model picker shows
only its valid modes. Use `/effort` to change the mode later. Auto and
fixed-reasoning models do not show an effort selector.

After account usage loads, the status bar shows the current Chutes plan and the
rolling four-hour/monthly percentages when available. Its color follows the
most constrained window. Click it or run `/usage` to inspect every active
window and reset time.

Chutes Build can inspect and edit files, execute commands, invoke MCP tools,
start subagents, search the web, and control an isolated browser. Review tool
requests before approval, especially in untrusted repositories.

For Chutes-specific questions, the main agent and subagents check the official
[documentation](https://chutes.ai/docs) and [news](https://chutes.ai/news)
before answering. When current official verification is unavailable, the agent
must say so.
