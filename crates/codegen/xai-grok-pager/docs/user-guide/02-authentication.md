# Authentication

Chutes Build supports Chutes API keys and browser OAuth 2.0 Authorization Code
with PKCE.

## API keys

Press `k`, run `/apikey`, or use `chutes-build login` for hidden local input.
For a process-local credential:

```powershell
$env:CHUTES_API_KEY = "your-api-key"
```

`login --api-key-stdin` is intended only for protected automation. Avoid
command-line secret values because process listings and shell history may expose
them.

## Browser OAuth

Press `l` or run `/login`. Chutes OAuth applications may require a registered
client ID and client secret. Chutes Build reads both from the environment and
never persists the secret:

```powershell
$env:CHUTES_BUILD_OAUTH2_CLIENT_ID = "cid_..."
$env:CHUTES_BUILD_OAUTH2_CLIENT_SECRET = "csc_..."
chutes-build
```

The secret is used for token exchange and refresh when configured. If the
bundled client returns `invalid_client`, use an API key or configure a dedicated
registered application.

## Credential boundaries

Ambient Chutes credentials are attached only to allowlisted official HTTPS
Chutes/router hosts. They are never forwarded to custom endpoints, Context7,
web search, arbitrary pages, browser automation, plugins, or MCP servers.

Custom inference models must configure their own `api_key` or `env_key`. A
custom model catalog uses `CHUTES_MODELS_API_KEY` and never receives
`CHUTES_API_KEY` implicitly.

Never place secrets in prompts, source code, `memories.md`, committed config,
logs, or screenshots. Rotate a credential immediately after accidental
exposure.
