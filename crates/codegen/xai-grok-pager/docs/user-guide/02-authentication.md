# Authentication

**The API key is the primary credential.** It always works, it is what a fresh
install uses, and it is the only method that needs no setup beyond creating the key.
Skip to [API Key](#api-key) if that is all you need.

Browser login is available too, but it is opt-in: it requires an OAuth application
that **you** register in your own account area at
[chutes.ai/app/api](https://chutes.ai/app/api), because there is no shared app that
could sign in on everyone's behalf. Without one configured, `chutes-build login`
says so and points you at the API key rather than opening a browser.

Enterprise single sign-on (OIDC) and external credential providers are also
supported, for organisations that run their own IdP.

---

## Browser Login (opt-in)

First register an OAuth application in your Chutes account area, then tell Chutes
Build its client ID:

```bash
export CHUTES_BUILD_OAUTH2_CLIENT_ID="cid_..."   # your registered app
chutes-build login
```

Set `CHUTES_BUILD_OAUTH2_ISSUER` as well only for a non-standard deployment; the
default is the standard Chutes IdP. With no client ID configured there is no OAuth
method to offer, and `login` tells you to set `CHUTES_API_KEY` instead.

Once signed in, Chutes Build opens your browser on subsequent logins:

```bash
chutes-build
```

Chutes Build stores credentials in `~/.chutes-build/auth.json` and reuses them across sessions. Chutes Build refreshes access tokens automatically in the background. When a token can't be refreshed, Chutes Build prompts you to sign in again. Credentials without a server-provided expiry fall back to a 30-day lifetime.

### Credential storage

Tokens in `~/.chutes-build/auth.json` (and MCP OAuth tokens in `~/.chutes-build/mcp_credentials.json`) are written with owner-only permissions (`0600` on Unix). Anyone with filesystem access to those paths can use the credentials, so:

- Prefer full-disk encryption (FileVault, BitLocker, LUKS, or equivalent).
- Do not copy `auth.json` or `mcp_credentials.json` into shared directories, tickets, or chat.
- On multi-user hosts, keep `$HOME` / `$CHUTES_BUILD_HOME` private to your account.

### Re-authenticate

To switch accounts or resolve an authentication problem, run:

```bash
chutes-build login
```

Running `chutes-build login` starts the sign-in flow again, replacing your cached session. By default, it opens your browser and signs in through Chutes OAuth. The issuer is `api.chutes.ai` — that is the value its discovery document declares, even though the document itself is served from `idp.chutes.ai`. Pass a flag to select a different flow:

| Flag | Description |
|------|-------------|
| `--oauth` | Sign in through Chutes OAuth. Requires `CHUTES_BUILD_OAUTH2_CLIENT_ID` to name an app you registered; without it there is no OAuth method and the flag has nothing to select. |
| `--device-auth` (alias `--device-code`) | Sign in with the device-code flow for headless or remote environments. |

To sign out, run `chutes-build logout`. It takes no flags and clears your cached credentials.

---

## API Key

Create a key at [chutes.ai/app/api](https://chutes.ai/app/api). There are three
ways to give it to Chutes Build:

- **In the app**: press `k` on the login screen, or run `/apikey` in a session, and
  paste it. Chutes Build stores it in `~/.chutes-build/auth.json` with owner-only
  permissions, so you only do this once.
- **In the environment**, which is what CI and automation want:

  ```bash
  export CHUTES_API_KEY="cpk_..."
  chutes-build
  ```

- **From stdin** for protected automation: `chutes-build login --api-key-stdin`.

This is the method a fresh install uses, and the one that always works. If you have also signed in interactively, the stored session token takes precedence for as long as it is valid; run `chutes-build logout` or delete `~/.chutes-build/auth.json` to go back to the key.

A blank value counts as unset. A CI job that exports the variable from a secret
that does not exist gets "You are not authenticated" rather than a login that
claims to be using a key and then fails.

`CHUTES_BUILD_API_KEY` is accepted as a legacy alias, checked only when
`CHUTES_API_KEY` is unset or blank.

---

## OIDC (Customer SSO)

Authenticate developers through your own Identity Provider (IdP) -- such as Okta, Azure AD, or Auth0 -- instead of chutes.ai.

### 1. Register a public client in your IdP

- Grant type: Authorization Code with PKCE (Proof Key for Code Exchange)
- Redirect URI: `http://127.0.0.1/callback` -- a loopback address. Chutes Build binds a random port at sign-in time, and most IdPs treat the loopback redirect as port-agnostic per [RFC 8252](https://tools.ietf.org/html/rfc8252).
- No client secret. PKCE replaces it.

### 2. Configure the CLI

Via config file:

```toml
# ~/.chutes-build/config.toml
[grok_com_config.oidc]
issuer = "https://acme.okta.com"
client_id = "0oa1b2c3d4e5f6g7h8i9"
```

Or via environment variables:

```bash
export CHUTES_BUILD_OIDC_ISSUER="https://acme.okta.com"
export CHUTES_BUILD_OIDC_CLIENT_ID="0oa1b2c3d4e5f6g7h8i9"
```

You can also override the API endpoint to point at your own proxy:

```bash
export CHUTES_BUILD_CLI_CHAT_PROXY_BASE_URL="https://grok-proxy.acme.com/v1"
```

### 3. Run `chutes-build`

The CLI discovers endpoints via `{issuer}/.well-known/openid-configuration`, opens the IdP login page, and stores tokens in `~/.chutes-build/auth.json`. Tokens auto-refresh silently via the stored `refresh_token`.

### Optional fields

| Field | Default | Notes |
|-------|---------|-------|
| `scopes` | `["openid", "profile", "email", "offline_access", "api:access"]` | `offline_access` enables silent token refresh |
| `audience` | None | Required by some IdPs (e.g., Auth0) |

---

## External Auth Provider

When browser-based login isn't possible -- for example, on sandboxed VMs, CI runners, or air-gapped networks -- delegate authentication to an external binary or script.

### How It Works

```
+--------------+     sh -c     +------------------------+
|     Chutes Build     |-------------->|  your auth binary      |
|              |               |                        |
|  reads       |<-- stdout ----|  prints token          |
|  auth.json   |               |                        |
|              |   (stderr)    |  prints status/URLs    |--> surfaced to user
+--------------+               +------------------------+
```

1. Chutes Build runs your command via `sh -c "<command>"`
2. Your binary runs whatever auth flow it needs (SSO, device code, certificate exchange)
3. **stderr** carries human-readable output, such as login URLs and status messages. Chutes Build reads stderr and surfaces it to the user; in the TUI, it turns the first `https://` URL into a clickable sign-in link.
4. **stdout** is captured by Chutes Build and saved as the access token
5. Exit 0 = success; exit non-zero = Chutes Build falls back to interactive login

### The stdout / stderr Contract

| Stream | What to print | Who sees it |
|--------|---------------|-------------|
| **stdout** | The token -- nothing else | Chutes Build (parsed and stored in auth.json) |
| **stderr** | Login URLs, status messages, errors | The user (Chutes Build reads stderr and shows the sign-in URL as a clickable link in the TUI) |

**Do not print anything to stdout except the token.** No progress messages, no debug output. Chutes Build reads stdout, trims surrounding whitespace, and parses the result as a token.

### stdout Token Format

**Bare string** -- just the raw token:

```
eyJhbGciOiJSUzI1NiIs...
```

**JSON** -- with optional refresh token, expiry, and issuer:

```json
{"access_token": "eyJhbGciOi...", "refresh_token": "ref-tok", "expires_in": 3600, "issuer": "https://idp.example.com"}
```

Use JSON if your tokens expire and you want Chutes Build to automatically re-run the binary before expiry.

JSON fields:

| Field | Required | Meaning |
|-------|----------|---------|
| `access_token` | yes | Bearer token Chutes Build sends to the xAI API |
| `refresh_token` | no | Stored for reference. Chutes Build refreshes by re-running your binary, not with an OAuth refresh grant |
| `expires_in` | no | Token lifetime in seconds; enables proactive refresh before expiry |
| `issuer` | no | Identifies the token's issuer |

### Configuration

Via config file:

```toml
# ~/.chutes-build/config.toml
[auth]
auth_provider_command = "/usr/local/bin/my-auth-provider"
auth_provider_label = "Acme Corp"   # optional -- customizes the TUI login button
auth_token_ttl = 3600               # optional -- token lifetime in seconds
```

Or via environment variables:

```bash
export CHUTES_BUILD_AUTH_PROVIDER_COMMAND="/usr/local/bin/my-auth-provider"
export CHUTES_BUILD_AUTH_PROVIDER_LABEL="Acme Corp"
export CHUTES_BUILD_AUTH_TOKEN_TTL=3600
```

### Token Refresh

Chutes Build runs your binary on two different contracts, and `CHUTES_BUILD_AUTH_EXPIRED` is how
it tells them apart. Each run fully replaces the stored credential, so emit the
same JSON fields (such as `issuer`) on every invocation, including refreshes.

- **`CHUTES_BUILD_AUTH_EXPIRED=1` — a headless refresh.** Chutes Build is re-minting over a
  credential it already holds: a near-expiry rotation, or a token the server
  rejected. Nobody is watching. stdin is closed, your stderr is swallowed, and
  the binary is given a few seconds before it is killed. Mint silently or exit
  non-zero — never block.
- **Unset — a sign-in.** `chutes-build login`, the sign-in screen, or the escalation
  Chutes Build performs when a headless run couldn't mint. A user is waiting, your
  stderr reaches them, and you have 300 seconds — enough for a browser round
  trip or a device code.

```bash
#!/bin/sh
if [ "$CHUTES_BUILD_AUTH_EXPIRED" = "1" ]; then
    # Headless: silent refresh only. Declining is the fast, correct answer
    # when your SSO session has lapsed and only the user can renew it.
    echo "Refreshing token..." >&2
    TOKEN=$(my-company-auth --refresh --silent) || exit 1
else
    echo "Authenticating via Acme Corp SSO..." >&2
    TOKEN=$(my-company-auth --login --interactive)
fi

if [ -z "$TOKEN" ]; then
    echo "Authentication failed" >&2
    exit 1
fi

echo "{\"access_token\": \"$TOKEN\", \"expires_in\": 3600}"
```

When the headless run can't produce a token, Chutes Build stops treating the stored
credential as usable and starts the sign-in flow instead — the same one you get
on a machine that has never signed in, with your binary's stderr shown, so a
device-code URL or a browser prompt reaches you. Exiting promptly on
`CHUTES_BUILD_AUTH_EXPIRED=1` is what makes that handover fast; a binary that blocks
instead makes you wait out the refresh timeout on every start. Mid-session, the
turn fails with a re-auth prompt and `/login` re-runs the binary interactively.

One case stays ambiguous, and only in **leader mode** (`--leader`, or
`[cli] use_leader = true`; off by default): with no credential at all, the
leader makes one extra attempt in the background just after startup, and that
run has the variable unset, like a sign-in. A binary that mints without help
(service account, keytab, mounted token) succeeds there and the session heals
itself. One that must prompt just sits, up to the 300s sign-in ceiling —
nothing waits on it, the sign-in screen is already up, and that run's stderr
goes to `~/.chutes-build/leader.log` rather than to you.

### Environment Variables

| Variable | Description |
|----------|-------------|
| `CHUTES_BUILD_AUTH_PROVIDER_COMMAND` | Path to your auth binary |
| `CHUTES_BUILD_AUTH_PROVIDER_LABEL` | Display name on the TUI login screen (e.g., "Acme Corp") |
| `CHUTES_BUILD_AUTH_TOKEN_TTL` | Token lifetime in seconds (for bare-string tokens without `expires_in`) |
| `CHUTES_BUILD_AUTH_EXPIRED` | Set to `1` on a headless refresh: don't prompt, and don't hand back a cached token. Unset on a sign-in, where a user is attached |
| `CHUTES_BUILD_AUTH_EARLY_INVALIDATION_SECS` | Seconds before expiry to proactively refresh (default: 300) |

---

## Device Code Flow

For headless environments (SSH sessions, Docker containers, remote VMs) where no browser is available locally:

```bash
chutes-build login --device-auth    # or: chutes-build login --device-code
```

This prints a URL and code to the terminal. Open the URL on any device, enter the code, and complete authentication. Chutes Build polls until the login is confirmed.

You can also implement the device-code flow through an [External Auth Provider](#external-auth-provider) for full control.

---

## Automatic Credential Refresh

Chutes Build automatically refreshes expired credentials:

- **Before expiry:** If your auth provider returned `expires_in` (JSON output) or you set `auth_token_ttl`, Chutes Build re-runs the auth binary ~5 minutes before expiry.
- **On auth error:** If the server returns 401 Unauthorized, Chutes Build refreshes the credentials and retries the request.
- **OIDC:** If a `refresh_token` is available, Chutes Build silently refreshes via your IdP without re-opening the browser.

Tune the refresh buffer:

```bash
# Refresh 5 minutes before expiry (default)
export CHUTES_BUILD_AUTH_EARLY_INVALIDATION_SECS=300

# Disable the proactive buffer: refresh at expiry or on a 401 (set to 0)
export CHUTES_BUILD_AUTH_EARLY_INVALIDATION_SECS=0
```

---

## Hot Reload

Chutes Build picks up changes to `~/.chutes-build/auth.json` automatically. If you update credentials externally (for example, with a script that writes new tokens), Chutes Build uses the new credentials on the next API call without a restart.

---

## Auth Precedence

Chutes Build resolves credentials for each request in this order, highest to lowest:

1. **Per-model `api_key` or `env_key`** -- set under `[model.<name>]` in `config.toml`. Wins whenever present.
2. **Active session token** -- obtained through browser, OIDC/OAuth2, or external-provider login and stored in `~/.chutes-build/auth.json`.
3. **`CHUTES_API_KEY`** -- used when no session token is active, and whenever the
   model's endpoint is one a session token may not be sent to (see below). On a
   default install this is the only method in play.

A session token is attached only to an official Chutes endpoint. A model pointed at
a custom `base_url` -- your own gateway, a local proxy, a third-party provider --
never receives your Chutes session credential; it falls through to the per-model
key, and if there is none, the request goes out unauthenticated rather than
leaking one. Give such a model its own `api_key` or `env_key`.

When more than one login flow is configured, Chutes Build populates the session token from the first available source, highest to lowest:

1. **External auth provider** (`auth_provider_command`)
2. **Enterprise OIDC** -- when OIDC is configured, through `[grok_com_config.oidc]` in `config.toml` or the `CHUTES_BUILD_OIDC_ISSUER` and `CHUTES_BUILD_OIDC_CLIENT_ID` environment variables
3. **Chutes OAuth2 browser login** -- only when `CHUTES_BUILD_OAUTH2_CLIENT_ID` names an app you registered

During a session, the active method handles all mid-session refreshes.

---

## Related settings

Coding-data sharing — **Coding data, retention, and training** in Settings, which
`/privacy` opens — is locked in this build and changes nothing. It is listed here
because upstream's version does affect a user's account, and because these config
knobs are separate from it either way:

| Setting | How to set it |
|---------|---------------|
| `[features] telemetry` | `config.toml` or `CHUTES_BUILD_TELEMETRY_ENABLED` |
| `[telemetry] trace_upload` | `config.toml` or `CHUTES_BUILD_TELEMETRY_TRACE_UPLOAD` |
| External OpenTelemetry | `CHUTES_BUILD_EXTERNAL_OTEL` / `[telemetry] otel_*`. See [Monitoring Usage](24-monitoring-usage.md). |

On team accounts, only a team admin can change coding-data sharing.
Team admins can also enable or disable Zero Data Retention (ZDR) for their team.
See [How to enable ZDR](https://chutes.ai/docs#how-to-enable-zdr).
When ZDR is on, coding-data sharing cannot be changed at all — the settings
row shows `ZDR` in place of the value.

See [Monitoring Usage](24-monitoring-usage.md#related-settings) and [Configuration](05-configuration.md#telemetry).

---

## Troubleshooting

### Debug logging

Set `RUST_LOG` to control the verbosity of the file log and headless stderr output. (The TUI's on-screen tracing pane uses a fixed filter and ignores `RUST_LOG`.) In the TUI, file logging defaults to `DEBUG`; in headless mode (`-p`), `RUST_LOG` defaults to `off` so only the answer is printed — set `RUST_LOG=error` (or broader) to see logs on stderr.

In the TUI, set `CHUTES_BUILD_LOG_FILE` to an absolute path to write logs to that file:

```bash
CHUTES_BUILD_LOG_FILE=/tmp/grok.log RUST_LOG=debug chutes-build
tail -f /tmp/grok.log
```

`CHUTES_BUILD_LOG_FILE` is treated as a literal file path. A relative value such as `1` writes a file named `1` in the current directory.

In headless mode, logs go to stderr. Redirect them to a file:

```bash
RUST_LOG=debug chutes-build -p "hello" 2> /tmp/grok.log
```

### Common log messages

| Log message | What it means |
|-------------|---------------|
| `auth: running external auth provider (headless refresh)` / `(interactive login)` | Chutes Build is running your binary, and on which contract |
| `auth: external auth provider returned fresh token` | Chutes Build parsed and stored the token |
| `auth: external auth provider failed` | Binary exited non-zero or stdout was empty |
| `auth: external auth provider timed out (likely needs interactive auth), killing` | Binary did not exit before the timeout and was killed |
| `auth: failed to start external auth provider` | Command could not be spawned (binary not found) |

### Common fixes

- **"Authentication failed"** -- Run `chutes-build logout` to clear cached credentials, then `chutes-build login` to sign in again.
- **Token expires too quickly** -- Set `auth_token_ttl` or return `expires_in` in your auth provider's JSON output.
- **OIDC redirect fails** -- Ensure your IdP allows loopback redirect URIs (`http://127.0.0.1/callback`).
- **External auth provider not found** -- Check that the `auth_provider_command` path is correct and the binary is executable.
