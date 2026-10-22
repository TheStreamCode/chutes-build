# Authentication

Chutes Build authenticates with a Chutes API key. There are no accounts to
sign in to, no browser login, no device-code flow, no enterprise SSO, and no
session tokens: the API key is the only credential, and every surface says
so. If you see a message naming `CHUTES_API_KEY`, that is the whole setup.

---

## Get a Chutes API key

Generate a key from
[chutes.ai/app/settings/api-keys](https://chutes.ai/app/settings/api-keys).
Keys start with `cpk_`. Treat a key like a password: anyone holding it can
bill your account.

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

Save the key once with `chutes-build login`: it prompts (or reads piped
stdin), validates the key against the API, and stores it at
`~/.chutes-build/api_key` for future runs. `chutes-build logout` deletes
the saved key. An explicit `CHUTES_API_KEY` env value always wins over
the saved one.

Without any key, Chutes Build starts on a setup screen. Press `k` to open the
key field (typing in the composer does nothing there), paste the key and press
Enter: it is validated, saved to `~/.chutes-build/api_key`, and the session
resumes. `q` quits instead — never a login option. Headless runs (`-p`) fail
with the same setup message instead of producing output.

On Windows, a save that reports `permission denied` is usually Controlled
folder access blocking the unsigned binary, not a missing directory
permission: allow `chutes-build.exe` through it, or avoid the write entirely by
setting `CHUTES_API_KEY`.

## No account login, no sessions

There is no account sign-in: `/login` and `/logout` are unavailable, and
`login --oauth` / `login --device-auth` are refused. There is no
`~/.chutes-build/auth.json` session to create, refresh, expire, or steal —
this build neither reads nor writes a login store — and no auth-provider
command is ever honored, whatever the config names. `/apikey` prints
these setup instructions; a key changed mid-session needs a restart.

---

## Auth precedence

Chutes Build resolves credentials for each request in this order, highest to
lowest:

1. **Per-model `api_key` or `env_key`** — set under `[model.<name>]` in
   `config.toml`. Wins whenever present.
2. **`CHUTES_API_KEY`** — the global key from the environment (the legacy
   `CHUTES_BUILD_API_KEY` only when `CHUTES_API_KEY` is unset).
3. **The saved key** — `~/.chutes-build/api_key`, written by
   `chutes-build login` or the welcome-screen entry. An explicit env key
   always wins over it.

That is the whole list. There is no session token below or above the key.

---

## Credential storage

The API key lives where you put it — the process environment, a
per-model entry in `~/.chutes-build/config.toml`, or the saved-key file
`~/.chutes-build/api_key` written by `chutes-build login` and the
welcome-screen entry (owner-only `0600` on Unix; default file
permissions on Windows, with no extra ACL hardening). MCP OAuth tokens
in `~/.chutes-build/mcp_credentials.json` are likewise owner-only. Anyone with filesystem access to those paths can use
the credentials, so:

- Prefer full-disk encryption (FileVault, BitLocker, LUKS, or equivalent).
- Do not copy config files, the saved `api_key`, or `mcp_credentials.json`
  into shared directories, tickets, or chat.
- On multi-user hosts, keep `$HOME` / `$CHUTES_BUILD_HOME` private to your
  account.

---

## Grove Git credentials (not an account login)

Grove (the external worktree daemon, where used) builds its own Git
credential cell from `auth_mode` in Grove config. Those credentials are
managed with `grove status` and `grove reload-credentials`, separately
from Chutes Build: a `CHUTES_API_KEY` never grants Git access.

---

## Related settings

Coding-data sharing — **Coding data, retention, and training** in Settings,
which `/privacy` opens — is the only one of these knobs that does anything
here. Product analytics, session-trace upload, and the external OTEL stream
are off and inert in this build; their keys are parsed for compatibility
but change nothing:

| Setting | How to set it |
|---------|---------------|
| `[features] telemetry` | Inert — see [Monitoring Usage](24-monitoring-usage.md). |
| `[telemetry] trace_upload` | Inert — see [Monitoring Usage](24-monitoring-usage.md). |
| External OpenTelemetry | Inert — see [Monitoring Usage](24-monitoring-usage.md). |

Team-managed states exist for account-auth builds only: on a team account,
only a team admin can change coding-data sharing, and admins can enable or
disable Zero Data Retention (ZDR) for the team — with ZDR on, the settings
row shows `ZDR` in place of the value instead of opening the chooser. This
API-key-only build has no account auth, so the row always stays editable
here.

See [Monitoring Usage](24-monitoring-usage.md) and [Configuration](05-configuration.md#telemetry).

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
  Export it (see above), save one with `chutes-build login`, or paste it
  on the welcome screen, and retry.
- **"Authentication failed"** — the key is rejected by the Chutes API.
  Generate a fresh key at
  [chutes.ai/app/settings/api-keys](https://chutes.ai/app/settings/api-keys),
  check for trailing whitespace or quotes in the value, and confirm no
  per-model `api_key`/`env_key` overrides the global one with a stale key.
- **Missing key on the start screen** — same cause as "Not signed in":
  press `k` and paste the key inline, or quit, export the key, and start again.
