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

### Browser OAuth

Press `l` or run `/login` to start OAuth 2.0 Authorization Code + PKCE. Chutes'
current Sign in with Chutes documentation provisions a client ID and client
secret for registered applications. Chutes Build never persists a configured
client secret; it reads credentials from the environment for token exchange
and refresh:

```powershell
$env:CHUTES_BUILD_OAUTH2_CLIENT_ID = "cid_..."
$env:CHUTES_BUILD_OAUTH2_CLIENT_SECRET = "csc_..."
chutes-build
```

The bundled client ID may depend on provider-side registration state. If it is
rejected with `invalid_client`, use an API key or register a dedicated Chutes
OAuth application and provide both values above.

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

`Auto (Chutes Router)` is selected by default when no model preference exists.
Use `chutes-build models --json` when scripts need the resolved catalog.

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
| `invalid_client` during OAuth | Use an API key or configure a registered OAuth client ID and secret. |
| Empty or stale model list | Confirm the credential and network access, then rerun `chutes-build models`. |
| Browser unavailable | Install Chrome/Edge or set `CHUTES_BROWSER_EXECUTABLE`. |
| Video inspection unavailable | Install FFmpeg or set `CHUTES_FFMPEG_EXECUTABLE`, then restart the process. |
| Terminal preview unavailable | Open the typed artifact with its native application; generation output remains on disk. |

Configuration paths and every supported public environment variable are in
[Configuration](configuration.md).
