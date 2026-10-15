# Authentication

Chutes Build authenticates with a Chutes API key. There are no accounts to
sign in to, no browser login, no device-code flow, no enterprise SSO, and no
session tokens: the API key is the only credential, and every surface says
so. If you see a message naming `CHUTES_API_KEY`, that is the whole setup.

---

## Get a Chutes API key

Generate a key from [console.chutes.ai](https://console.chutes.ai). Keys start
with `cpk_`. Treat a key like a password: anyone holding it can bill your
account.

## Configure the key

Set the environment variable before starting Chutes Build:

```bash
export CHUTES_API_KEY="cpk_..."
chutes-build
```

On Windows (PowerShell), either for the session:

```powershell
$env:CHUTES_API_KEY = "cpk_..."
chutes-build
```

or persistently for your user:

```powershell
setx CHUTES_API_KEY "cpk_..."
```

A per-model key in `~/.chutes-build/config.toml` wins over the global one.
Under `[model.<name>]`, set `api_key` for a literal key or `env_key` for the
name of an environment variable holding it:

```toml
[model.my-model]
model = "my-model"
env_key = "MY_MODEL_KEY"
```

The legacy `CHUTES_BUILD_API_KEY` variable is still accepted when
`CHUTES_API_KEY` is unset, so existing deployments keep working; new setups
should use `CHUTES_API_KEY`.

Without any key, Chutes Build starts on a setup screen that names
`CHUTES_API_KEY` and exits only via Quit. Headless runs (`-p`) fail with the
same message instead of producing output.

## No login, no logout, no sessions

`chutes-build login` and `chutes-build logout` exist only to tell you the
above: both print the API-key setup message and exit. The `/login` and
`/logout` TUI commands are unavailable for the same reason. There is no
`~/.chutes-build/auth.json` session to create, refresh, expire, or steal —
this build neither reads nor writes a login store — and no auth-provider
command is ever honored, whatever the config names.

---

## Auth precedence

Chutes Build resolves credentials for each request in this order, highest to
lowest:

1. **Per-model `api_key` or `env_key`** — set under `[model.<name>]` in
   `config.toml`. Wins whenever present.
2. **`CHUTES_API_KEY`** — the global key from the environment.

That is the whole list. There is no session token below or above the key.

---

## Credential storage

The API key lives where you put it — the process environment, or a
per-model entry in `~/.chutes-build/config.toml` — and is never written
anywhere else by this build. MCP OAuth tokens in
`~/.chutes-build/mcp_credentials.json` are written with owner-only
permissions (`0600` on Unix). Anyone with filesystem access to those paths
can use the credentials, so:

- Prefer full-disk encryption (FileVault, BitLocker, LUKS, or equivalent).
- Do not copy config files or `mcp_credentials.json` into shared directories,
  tickets, or chat.
- On multi-user hosts, keep `$HOME` / `$CHUTES_BUILD_HOME` private to your
  account.

---

## Grove Git credentials (not an account login)

Grove (the `chutes-build clone` daemon) builds its own Git credential cell
from `auth_mode` in Grove config. Those credentials are managed with
`grove status` and `grove reload-credentials` — see
[chutes-build clone](27-grok-clone.md#authentication) for the failure classes
and their next steps.

---

## Related settings

Coding-data sharing — **Coding data, retention, and training** in Settings,
which `/privacy` opens — does not change these config knobs:

| Setting | How to set it |
|---------|---------------|
| `[features] telemetry` | `config.toml` or `CHUTES_BUILD_TELEMETRY_ENABLED` |
| `[telemetry] trace_upload` | `config.toml` or `CHUTES_BUILD_TELEMETRY_TRACE_UPLOAD` |
| External OpenTelemetry | `CHUTES_BUILD_EXTERNAL_OTEL` / `[telemetry] otel_*`. See [Monitoring Usage](24-monitoring-usage.md). |

On team accounts, only a team admin can change coding-data sharing.
Team admins can also enable or disable Zero Data Retention (ZDR) for their team.
See [How to enable ZDR](https://docs.chutes.ai/developers/faq/security#how-to-enable-zdr).
When ZDR is on, coding-data sharing cannot be changed at all — the settings
row shows `ZDR` in place of the value. ZDR does not turn off external OTEL
or `user.email` — see [ZDR and this stream](24-monitoring-usage.md#zdr-and-this-stream).

See [Monitoring Usage](24-monitoring-usage.md#related-settings) and [Configuration](05-configuration.md#telemetry).

---

## Troubleshooting

### Debug logging

Set `RUST_LOG` to control the verbosity of the file log and headless stderr output. (The TUI's on-screen tracing pane uses a fixed filter and ignores `RUST_LOG`.) In the TUI, file logging defaults to `DEBUG`; in headless mode (`-p`), `RUST_LOG` defaults to `off` so only the answer is printed — set `RUST_LOG=error` (or broader) to see logs on stderr.

In the TUI, set `CHUTES_BUILD_LOG_FILE` to an absolute path to write logs to that file:

```bash
CHUTES_BUILD_LOG_FILE=/tmp/chutes-build.log RUST_LOG=debug chutes-build
tail -f /tmp/chutes-build.log
```

`CHUTES_BUILD_LOG_FILE` is treated as a literal file path. A relative value such as `1` writes a file named `1` in the current directory.

In headless mode, logs go to stderr. Redirect them to a file:

```bash
RUST_LOG=debug chutes-build -p "hello" 2> /tmp/chutes-build.log
```

### Common fixes

- **"Not signed in"** — `CHUTES_API_KEY` is unset or empty in this shell.
  Export it (see above) and retry. There is no login command that can
  substitute for the key.
- **"Authentication failed"** — the key is rejected by the Chutes API.
  Generate a fresh key at [console.chutes.ai](https://console.chutes.ai),
  check for trailing whitespace or quotes in the value, and confirm no
  per-model `api_key`/`env_key` overrides the global one with a stale key.
- **"Set CHUTES_API_KEY ... and restart" on the start screen** — same cause
  as "Not signed in": quit, export the key, start again.
