# Web Search and Browser Automation

Web search uses DuckDuckGo by default. Brave Search is available with its own
credential:

```powershell
$env:BRAVE_SEARCH_API_KEY = "your-brave-key"
$env:CHUTES_WEB_SEARCH_PROVIDER = "brave"
```

The Chutes API key is never reused for search.

The `browser` tool controls a local Chrome or Edge instance through a
loopback-only DevTools endpoint. Supported actions include navigation,
accessibility snapshots, click, type, screenshots, and close. The browser starts
with an isolated temporary profile; it does not attach to the user's normal
profile or existing signed-in sessions.

Browser activity still sends normal network traffic and form data to visited
sites. Review uploads, submissions, authentication, purchases, and external
messages before approval. Screenshots must remain inside the active workspace.
