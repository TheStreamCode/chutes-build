# Interactive slash commands

Type `/help` in the TUI to browse the commands and their current arguments.
The list below is the built-in command set registered by Chutes Build; plugins
may add more commands at runtime.

## Session and navigation

| Command | Purpose |
| --- | --- |
| `/quit` | Exit Chutes Build |
| `/home` | Return to the welcome screen |
| `/new` | Start a new session |
| `/resume` | Resume a local session |
| `/fork` | Branch the current session into a peer agent |
| `/rename` | Rename the current session |
| `/session-info` | Show local session metadata |
| `/history` | Search prompt history |
| `/find` | Search conversation scrollback |
| `/jump` | Jump to a turn |
| `/rewind` | Rewind to a previous turn |
| `/recap` | Summarize the session so far |
| `/compact` | Compact conversation history |
| `/context` | View context usage |
| `/expand` | Reprint the last collapsed block in full |
| `/copy` | Copy the latest or selected response |
| `/export` | Export the conversation locally or to the clipboard |
| `/transcript` | Open the full transcript in `$PAGER` |

## Model and agent control

| Command | Purpose |
| --- | --- |
| `/model` | Switch the active model |
| `/effort` | Select a reasoning option supported by that model |
| `/advisor` | Enable, disable, or select the read-only advisor |
| `/plan` | Enter plan mode |
| `/view-plan` | View the current plan |
| `/always-approve` | Toggle permission-prompt bypass |
| `/auto` | Toggle classifier-based approval for safe tools |
| `/btw` | Ask a side question without interrupting the active turn |
| `/queue` | List queued prompts |
| `/tasks` | List background tasks, subagents, and schedules |
| `/loop` | Run a prompt on a recurring interval |
| `/config-agents` | Manage agent definitions |
| `/personas` | Manage personas |
| `/remember` | Save a memory note |

## Authentication and Chutes

| Command | Purpose |
| --- | --- |
| `/login` | Open OAuth/API-key authentication |
| `/apikey` | Enter a Chutes API key directly |
| `/logout` | Clear the cached credential |
| `/usage` | View Chutes usage; `/usage manage` opens the Chutes usage page |
| `/imagine` | Generate an image with a Chutes media model |
| `/imagine-video` | Generate a video with a Chutes media model |
| `/voice` | Configure or activate manual voice input |
| `/docs` | Open embedded guides or official Chutes documentation |
| `/release-notes` | Show the repository changelog for this version |

## Plugins and tools

| Command | Purpose |
| --- | --- |
| `/hooks` | View active hooks |
| `/plugins` | View/manage plugins |
| `/marketplace` | View/manage marketplace sources |
| `/skills` | View installed skills |
| `/mcps` | Show MCP server status |
| `/cd` | Change the working directory used by new agents |
| `/import-claude` | Open the Claude settings import flow |

Plugin and marketplace removal is destructive. The interactive plugin command
requires explicit `--yes` for uninstall operations, matching the CLI safety
gate.

## Capability-gated session controls

The agent advertises these additional built-ins only when the matching local
capability is active:

| Command | Availability and purpose |
| --- | --- |
| `/memory [on\|off]` | Browse or toggle configured cross-session memory |
| `/flush` | Flush pending memory changes to disk |
| `/dream` | Consolidate local memory topics |
| `/goal <OBJECTIVE> [--budget TOKENS]` | Start or inspect an autonomous goal; also accepts `status`, `pause`, `resume`, and `clear` |
| `/hooks-list` | List hooks loaded in the session |
| `/hooks-trust` / `/hooks-untrust` | Add or remove project hook trust |
| `/hooks-add <PATH>` / `/hooks-remove <PATH>` | Add or remove a custom hook path |
| `/reload-plugins` | Reload the active plugin registry |

These commands can disappear when memory, goals, hooks, plugins, or scheduling
are unavailable. Skill-provided slash commands are dynamic and are listed by
`/help` for the current session.

## Interface

| Command | Purpose |
| --- | --- |
| `/help` | Browse commands and keyboard shortcuts |
| `/dashboard` | Open the local agent dashboard |
| `/settings` | Open settings |
| `/theme` | Change theme |
| `/minimal` | Switch to scrollback-native minimal mode |
| `/fullscreen` | Switch to fullscreen mode |
| `/compact-mode` | Toggle reduced UI spacing |
| `/multiline` | Swap Enter and Shift+Enter behavior |
| `/vim-mode` | Toggle vim-style scrollback keys |
| `/timestamps` | Toggle message timestamps |
| `/timeline` | Toggle the timeline sidebar |
| `/toggle-mouse-reporting` | Toggle terminal mouse reporting |
| `/terminal-setup` | Check terminal, color, and clipboard support |
| `/announcements` | Show or hide announcements |
| `/debug` | Toggle debug overlays |
| `/scroll-debug` | Inspect scroll/render diagnostics |
| `/gboom` | Open the built-in terminal game |

Remote `/share`, feedback upload, and upstream coding-data retention controls
are intentionally not registered in Chutes Build. Automatic update commands
are also absent; update the installed package or release artifact manually.
