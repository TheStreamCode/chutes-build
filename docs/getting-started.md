# Getting started

This guide takes Chutes Build from installation to a first safe session. The
installed binary remains authoritative for version-specific flags:

```powershell
chutes-build --help
```

## Requirements

The published npm package includes a native executable for Windows, macOS, and
Linux and requires Node.js 18 or newer. Building from source requires Rust
stable, Git, and the platform C/C++ toolchain used by Rust.

## Install

```powershell
npm install -g chutes-build
chutes-build
```

For a one-off run:

```powershell
npx chutes-build
```

The npm launcher resolves a platform package already installed by npm. It does
not fetch an executable from a post-install script.

## Authentication

### API key

Chutes documents API keys as the fastest way to get started. Create a scoped
key in the Chutes dashboard and provide it through one of these paths:

- press `k` on the welcome screen;
- run `/apikey` in an interactive session;
- run `chutes-build login` for hidden local input; or
- set `CHUTES_API_KEY` for the current process.

```powershell
$env:CHUTES_API_KEY = "your-api-key"
chutes-build
```

Avoid command-line arguments for secrets because process listings and shell
history may expose them. `login --api-key-stdin` is intended only for protected
automation.

### Browser OAuth (opt-in)

Browser login requires an OAuth application registered in your own Chutes account
area; there is no bundled client ID, because one app cannot sign in on behalf of
other accounts. Register the app, then give Chutes Build its client ID:

```powershell
$env:CHUTES_BUILD_OAUTH2_CLIENT_ID = "cid_..."
$env:CHUTES_BUILD_OAUTH2_CLIENT_SECRET = "csc_..."   # only for a confidential app
chutes-build
```

With those set, press `l` or run `/login` to start OAuth 2.0 Authorization Code +
PKCE. Chutes Build never persists a configured client secret; it reads it from the
environment for token exchange and refresh. Without a client ID configured there is
no OAuth method to offer, and `login` says so instead of opening a browser.

## First session

Run `chutes-build` from the repository you want the agent to inspect. Chutes
Build asks for the applicable folder-trust and tool permissions before
performing sensitive operations.

Useful first commands:

| Command | Purpose |
| --- | --- |
| `/help` | Browse commands and shortcuts. |
| `/model` | Choose Auto or a concrete Chutes model. |
| `/effort` | Select a supported reasoning mode for the active model. |
| `/usage` | Inspect available plan and quota windows. |
| `/docs` | Open concise guides embedded in the application. |
| `/settings` | Review interface and runtime preferences. |
| `/rewind` or `/undo` | Rewind the current session to a previous turn. |
| `/plan` | Enter plan mode before implementation. |

`Auto (Chutes Router)` is selected by default when no model preference exists.
It sends Chutes' native routing alias `default` (the pool saved at
chutes.ai/app → Model Routing). If that alias cannot resolve, the fallback
chain steps down to a live inline pool from the current catalogue (fastest
warm model first) so Auto works without a dashboard setup. Pin a concrete catalogue id with `/model` or
`CHUTES_BUILD_DEFAULT_MODEL`. Use `chutes-build models --json` when scripts
need the resolved catalog.

When revising a proposed plan, type revision notes and press Enter to request
changes. An empty Enter does not approve the plan; press `a` explicitly when
the revision prompt is empty and the plan is ready to implement.

## Safe operating habits

- Start in a repository you trust and keep permission prompts enabled.
- Review commands, destinations, and external network actions before approval.
- Do not paste secrets into prompts, source files, memory, or committed config.
- Use `--no-memory` for a stateless session or when memory text must not be sent
  to the Chutes-hosted embedding route.
- Use `CHUTES_STRICT_MODEL=1` when an explicit model must never fall back.
- Treat model output, repository instructions, web content, MCP servers,
  plugins, and downloaded documents as untrusted input.

## Headless use

```powershell
chutes-build -p "Summarize this repository" --output-format plain
chutes-build -p "Return the package list" --output-format json
```

Headless-only controls such as `--max-turns`, `--tools`,
`--disallowed-tools`, and `--json-schema` require a headless prompt. See the
[CLI reference](cli-reference.md).

## Troubleshooting

| Problem | Check |
| --- | --- |
| Command not found | Confirm the npm global bin directory is on `PATH`, or try `npx chutes-build`. |
| `invalid_client` during OAuth | The client ID is not a valid app on the issuer. Re-check the registration in your Chutes account area, or just use an API key. |
| Empty or stale model list | Confirm the credential and network access, then rerun `chutes-build models`. |
| Browser unavailable | Install Chrome/Edge or set `CHUTES_BROWSER_EXECUTABLE`. |
| Video inspection unavailable | Install FFmpeg or set `CHUTES_FFMPEG_EXECUTABLE`, then restart the process. |
| Terminal preview unavailable | Open the typed artifact with its native application; generation output remains on disk. |

Configuration paths and every supported public environment variable are in
[Configuration](configuration.md).
