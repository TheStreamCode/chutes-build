# Web Search and Browser Automation

Web search uses DuckDuckGo by default. Brave Search is available with its own
credential:

```powershell
$env:BRAVE_SEARCH_API_KEY = "your-brave-key"
$env:CHUTES_WEB_SEARCH_PROVIDER = "brave"
```

The Chutes API key is never reused for search.

The `browser` tool controls a local Chrome or Edge instance through a
loopback-only DevTools endpoint. The browser starts with an isolated temporary
profile; it does not attach to the user's normal profile or existing signed-in
sessions.

Reading the page:

| Action | Reports |
| --- | --- |
| `snapshot` | Interactive elements with indices, for clicking by index. |
| `text` | The full visible page text. |
| `screenshot` | A PNG written inside the workspace. |
| `console` | Console output and uncaught JavaScript errors. |
| `network` | Requests, responses, and failures. |

Acting on it: `navigate`, `click`, `type`, `select`, `key` (`Enter`, `Tab`,
`Escape`, arrows, `Home`/`End`, `PageUp`/`PageDown`), `scroll`, `wait` (until a
selector becomes visible or page text appears), `back`, `reload`, and `close`.
Elements are addressed by CSS selector or by the index reported by `snapshot`.

The console and network logs cover the whole browser session and keep their most
recent 200 entries each, so they can be inspected after the fact rather than
only while the page is loading. Prefer `wait` over retrying a click that failed
because the page had not settled.

Browser activity still sends normal network traffic and form data to visited
sites. Review uploads, submissions, authentication, purchases, and external
messages before approval. Screenshots must remain inside the active workspace.
