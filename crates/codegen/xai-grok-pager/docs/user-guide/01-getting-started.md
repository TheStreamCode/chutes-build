# Getting Started

Chutes Build is a privacy-first terminal coding agent for the Chutes ecosystem.

## Install

Published packages include a native executable:

```powershell
npm install -g chutes-build
chutes-build
```

Source builds use `cargo build -p chutes-build --release`.

## Authenticate

An API key is the most reliable first-run path. Press `k`, run `/apikey`, or
use the hidden local prompt:

```powershell
chutes-build login
```

For a process-local key:

```powershell
$env:CHUTES_API_KEY = "your-api-key"
chutes-build
```

Press `l` or run `/login` for browser OAuth + PKCE. If the bundled client is
rejected with `invalid_client`, use an API key or configure a registered Chutes
OAuth client ID and secret through the environment.

## Start safely

Run Chutes Build from the repository you want it to inspect. Keep permission
prompts enabled, review commands before approval, and treat repository
instructions, model output, websites, MCP servers, and plugins as untrusted.

`Auto (Chutes Router)` is the first model choice and default when no preference
has been saved. Use `/model` or `--model <id>` to choose a concrete model and
`/effort` for one of that model's supported reasoning modes. Run
`chutes-build models --json` for a machine-readable catalog.

The status bar shows available Chutes plan/quota windows. Click it or run
`/usage` for details. Use `/help` for commands, `/docs` for these guides, and
`--no-memory` when the session must be stateless.

For Chutes-specific questions, the main agent and subagents consult official
[documentation](https://chutes.ai/docs) and [news](https://chutes.ai/news).
When current official verification is unavailable, they must say so.
