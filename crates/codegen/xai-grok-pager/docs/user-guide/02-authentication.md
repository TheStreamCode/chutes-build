# Authentication

Chutes Build supports browser-based OAuth 2.0 + PKCE and Chutes API keys.
Press `l` on the welcome screen or use `/login` for OAuth. The built-in public
client does not require a client secret.

Custom OAuth clients can be supplied without persisting their secret:

```powershell
$env:CHUTES_BUILD_OAUTH2_CLIENT_ID = "cid_..."
$env:CHUTES_BUILD_OAUTH2_CLIENT_SECRET = "csc_..."
chutes-build
```

The secret is used for both token exchange and refresh when configured.

For API-key authentication, the recommended environment variable is
`CHUTES_API_KEY`.

```powershell
$env:CHUTES_API_KEY = "your-api-key"
```

`chutes-build login` stores a key using the local credential mechanism. Use
`chutes-build login --api-key-stdin` only in automation where stdin is already
protected.

Ambient Chutes credentials are attached only to allowlisted official HTTPS
Chutes and Chutes-router hosts. They are never forwarded to custom endpoints,
Context7, web search, arbitrary pages, browser automation, plugins, or MCP
servers.

Custom inference models must configure their own `api_key` or `env_key`. A
custom model-catalog endpoint uses `CHUTES_MODELS_API_KEY`; it never receives
`CHUTES_API_KEY` implicitly.

Never place API keys in prompts, source code, `memories.md`, committed config,
or command-line arguments. Rotate a key immediately if it appears in logs,
screenshots, chat history, or a public repository.
