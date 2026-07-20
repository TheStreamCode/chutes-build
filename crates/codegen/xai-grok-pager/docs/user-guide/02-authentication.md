# Authentication

Chutes Build uses a Chutes API key. The recommended environment variable is
`CHUTES_API_KEY`.

```powershell
$env:CHUTES_API_KEY = "your-api-key"
```

`chutes-build login` stores a key using the local credential mechanism. Use
`chutes-build login --api-key-stdin` only in automation where stdin is already
protected.

The credential is attached only to allowlisted Chutes and Chutes-router hosts.
It is never forwarded to Context7, web search, arbitrary pages, browser
automation, plugins, or MCP servers.

Never place API keys in prompts, source code, `memories.md`, committed config,
or command-line arguments. Rotate a key immediately if it appears in logs,
screenshots, chat history, or a public repository.
