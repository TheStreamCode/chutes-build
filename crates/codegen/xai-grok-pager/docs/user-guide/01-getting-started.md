# Getting Started

Chutes Build is a terminal-based AI coding assistant for the Chutes ecosystem. It runs as a TUI (Terminal User Interface) that understands your codebase, executes shell commands, edits files, searches the web, and manages tasks.

It is a fork of [Grok Build](https://github.com/xai-org/grok-build), re-based on upstream 1.0.0 and adapted to run on Chutes. Not affiliated with or endorsed by SpaceXAI.

You can use it interactively as a full-screen TUI, run it headlessly for scripting and CI/CD, or integrate it into editors via the Agent Client Protocol (ACP).

---

## Installation

Via npm, which fetches the binary for your platform (macOS, Linux, Windows):

```bash
npm i -g chutes-build
```

A specific version:

```bash
npm i -g chutes-build@1.0.0
```

Or download a binary from
[Releases](https://github.com/TheStreamCode/chutes-build/releases) and put it on
your `PATH`, or build from source — see the repository README.

Verify the installation:

```bash
chutes-build --version
```

### Updating

Chutes Build does not update itself. It never contacts an update server, by
design: `chutes-build update` reports where to get a newer version rather than
fetching one. To upgrade, re-run the npm install above or download a newer
release.

---

## First Launch

Start Chutes Build by running:

```bash
chutes-build
```

On first launch, Chutes Build asks for a Chutes API key. Create one at
[chutes.ai/app/api](https://chutes.ai/app/api), then either paste it at the prompt
(or with `/apikey` later) or set it in the environment:

```bash
export CHUTES_API_KEY="cpk_..."
chutes-build
```

The API key is the primary credential and the one that always works. Chutes Build
stores what you paste in `~/.chutes-build/auth.json`, with owner-only permissions,
and reuses it across sessions.

Browser login is available as well, but it is opt-in: it needs an OAuth application
you register in your own account area, since there is no shared app that could sign
in on your behalf. Set `CHUTES_BUILD_OAUTH2_CLIENT_ID` to its client ID and
`chutes-build login` will use it; without one, `login` tells you to use the API key.

See [Authentication](02-authentication.md) for the full set of auth options,
including OIDC, external auth providers, and the device code flow.

---

## Basic Interaction

Once authenticated, Chutes Build presents a full-screen TUI with two main areas:

- **Scrollback** -- the conversation history showing your prompts, Chutes Build's responses, tool calls, file edits, and more.
- **Prompt** -- the input area at the bottom where you type messages.

Type a message and press `Enter` to send it. Chutes Build reads files, runs commands, and edits code as needed. Each tool run streams into the scrollback in real time.

Press `Tab` to move focus between the prompt and the scrollback. While a turn is running, `Esc` cancels it (the exception is fullscreen vim scrollback mode, where mid-turn `Esc` is a no-op; minimal mode cancels even with vim on); `Ctrl+C` cancels once the composer is empty — with a draft, the first press only clears it. Idle, press `Esc` twice within 800ms to clear a non-empty prompt, or (with an empty prompt and conversation messages) to open rewind — see [Keyboard Shortcuts](03-keyboard-shortcuts.md#escape). With the scrollback focused, use the arrow keys to select entries and to collapse or expand them. To navigate with `j`/`k` and fold with `h`/`l` instead, enable Vim mode.

### File References

Use `@` in your prompt to attach files:

```
@src/main.rs              # Attach a file
@src/main.rs:10-50        # Attach lines 10-50
@src/                     # Browse a directory
```

The `@` operator opens a fuzzy file picker. By default it respects `.gitignore` and hides dotfiles. Prefix with `!` to search hidden files:

```
@!.github                 # Search hidden files
@!.env                    # Attach a .env file
```

### Permissions

By default, Chutes Build asks for permission before executing shell commands or editing files. You can approve individually or toggle always-approve mode:

- Press `Ctrl+O` to toggle always-approve mode
- Use the `--yolo` flag at launch: `chutes-build --yolo`
- Type `/always-approve` in the prompt to toggle the mode

---

## Key Concepts

### Sessions

Every conversation is a **session**. Sessions are automatically saved to `~/.chutes-build/sessions/` and can be resumed later. Each session tracks the full conversation history, tool calls, file edits, and task state.

- Start a new session: `Ctrl+N` or `/new`
- Resume a previous session: `/resume` in the TUI, or `--resume <ID>` from the CLI
- Continue the most recent session: `chutes-build -c`

### Scrollback

The scrollback is the main display area. It shows:

- **User prompts** -- your messages, rendered as sticky headers
- **Agent messages** -- Chutes Build's responses with full markdown rendering and syntax highlighting
- **Thinking blocks** -- Chutes Build's reasoning process (collapsible)
- **Tool calls** -- file edits (with inline diffs), command executions, search results, and more
- **Task lists** -- TODO items tracking progress

Collapse or expand the selected entry with the `Left`/`Right` arrow keys (or `h`/`l` and `e` in Vim mode). In Vim mode, press `y` to copy its content and `Y` to copy its metadata (for example, the command that ran). Press `Enter` to open it in the fullscreen viewer (in any mode).

### Tools

Chutes Build has built-in tools for:

| Tool | Description |
|------|-------------|
| `read_file` / `search_replace` | Read and edit files with line-precise changes |
| `grep` | Regex search across your codebase (powered by ripgrep) |
| `list_dir` | List directory contents |
| `run_terminal_command` | Execute shell commands |
| `web_search` / `web_fetch` | Search the web and fetch URLs |
| `todo_write` | Create and manage task lists |
| `spawn_subagent` | Spawn parallel subagent sessions |
| `memory_search` | Search cross-session memory |

Tools can be extended with [MCP servers](05-configuration.md#mcp-servers) for integrations like GitHub, databases, and more.

### Slash Commands

Type `/` in the prompt to access commands. These provide quick actions without writing a full prompt:

```
/model chutes-build                 # Switch model
/compact                          # Compress conversation history
/always-approve                   # Toggle always-approve mode
/new                              # Start a new session
```

See [Slash Commands](04-slash-commands.md) for the complete reference.

---

## Common Launch Options

```bash
# Launch the interactive TUI and submit an initial prompt as the first turn
chutes-build "fix the failing auth test and run it"

# Initial prompt in a new git worktree. Use --worktree=<name> (with `=`) so the
# prompt isn't swallowed as the worktree name — `chutes-build -w "refactor module X"`
# would treat "refactor module X" as the worktree label, not the prompt.
chutes-build --worktree=feat "refactor module X"

# Base the worktree on a specific branch (e.g. main) instead of the current HEAD:
chutes-build -w --ref main "implement feature from main"


# Start in a specific project directory
chutes-build --cwd ~/projects/my-app

# Add project-specific rules
chutes-build --rules "Always use TypeScript. Prefer functional components."

# Auto-approve all tool executions
chutes-build --yolo

# Use a specific model
chutes-build -m chutes-build

# Resume a previous session
chutes-build --resume <session-id>

# Continue the most recent session
chutes-build -c

# Experimental scrollback-native render mode. Sticky: plain `chutes-build` reopens in
# the mode last chosen via --minimal/--fullscreen (or /minimal//fullscreen).
chutes-build --minimal

# Back to the standard fullscreen TUI (and make it sticky again)
chutes-build --fullscreen

# Headless mode (for scripts)
chutes-build -p "Explain this codebase"
```

---

## Headless Mode

Run Chutes Build non-interactively for scripting, CI/CD, and automation:

```bash
chutes-build -p "Your prompt here"
```

Output formats:

| Format | Flag | Description |
|--------|------|-------------|
| `plain` | (default) | Human-readable text |
| `json` | `--output-format json` | Single JSON object with `text`, `stopReason`, `sessionId`, and `requestId` |
| `streaming-json` | `--output-format streaming-json` | NDJSON event stream for real-time processing |

Example CI/CD usage:

```bash
chutes-build -p "Review changes for bugs" --output-format json --yolo | jq -r '.text'
```

---

## Project Rules (AGENTS.md)

Add per-project instructions by creating an `AGENTS.md` file in your repository. Chutes Build reads these files and injects their contents as a project-instructions message at the start of the conversation:

```
~/.chutes-build/AGENTS.md           # Global rules (apply to all projects)
<repo-root>/AGENTS.md       # Repository-level rules
<cwd>/AGENTS.md             # Directory-level rules (highest priority)
```

Deeper files take precedence. Chutes Build also reads `CLAUDE.md` files for compatibility.

---

## Where to Go Next

| Document | What You Will Learn |
|----------|-------------------|
| [Authentication](02-authentication.md) | Browser login, API keys, OIDC, external auth, device code flow |
| [Keyboard Shortcuts](03-keyboard-shortcuts.md) | Complete reference for all key bindings |
| [Slash Commands](04-slash-commands.md) | All available `/` commands |
| [Configuration](05-configuration.md) | config.toml, pager.toml, environment variables |
