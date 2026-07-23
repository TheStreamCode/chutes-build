# CLI reference

This document describes the public command surface of Chutes Build. The
installed binary is authoritative for its exact version:

```powershell
chutes-build --help
chutes-build <command> --help
```

## Interactive and single-turn modes

Running `chutes-build` starts the interactive TUI. A positional `PROMPT`
supplies its first message.

Use one of these mutually exclusive inputs for a non-interactive turn:

- `-p, --single <PROMPT>`
- `--prompt-file <PATH>`
- `--prompt-json <JSON>`

Headless output supports `--output-format plain|json|streaming-json`.
`--json-schema <SCHEMA>` selects JSON output and constrains the model response.
`--max-turns`, `--best-of-n 2..10`, `--check`, `--agents`, `--tools`, and
`--disallowed-tools` are headless-only. `--no-plan` is interactive-only.

A positional interactive prompt cannot be combined with a subcommand.
Headless-only options without a headless prompt are rejected instead of being
silently ignored.

The interactive file and executable suggestion providers currently use a
POSIX-shell tokenizer and are enabled on Unix only. On Windows, command
execution still uses the configured Windows shell, but these two Tab-completion
providers stay disabled rather than emitting incorrectly escaped commands.

## Local state root

`CHUTES_BUILD_HOME` relocates the entire user state root. When unset, the root
is `~/.chutes-build`. Configuration, credentials, sessions, logs, trace
exports, plugins, user roles/personas, and the managed bundled-agent cache all
resolve below the same root. Project-scoped `.chutes-build` directories are not
relocated.

## Session startup

- `-r, --resume [SESSION_ID]` resumes the named or most recent session.
  `--load` is a compatibility alias.
- `-c, --continue` resumes the most recent session for the current directory.
- `--fork-session` creates a new ID while resuming; `--session-id <UUID>` may
  name that fork.
- `--restore-code` is valid only with `--resume` or `--continue`.
- `-s, --session-id <UUID>` otherwise names a new session and must not collide
  with an existing one.
- `-w, --worktree [NAME]` starts in a new git worktree.
- `--worktree-ref <REF>` selects its base and requires `--worktree`.
- `--cwd <DIR>` is validated, canonicalized, and applied once.

Worktree startup is available in both the interactive and single-turn paths.
Forking a resumed session and creating a new worktree in the same invocation is
rejected because the two operations have incompatible ownership semantics.

## Models, agents, and permissions

- `-m, --model <ID>` selects a model.
- `--reasoning-effort <OPTION>` (`--effort`) accepts only an option supported
  by that model.
- `--agent <NAME|PATH>` and `--agents <JSON>` select agent definitions.
- `--no-subagents` prevents subagent spawning.
- `--memory` / `--no-memory` control local cross-session memory.
- `--permission-mode default|acceptEdits|auto|dontAsk|bypassPermissions|plan`
  selects the permission policy.
- `--allow <RULE>` and `--deny <RULE>` are repeatable/comma-delimited
  permission rules.
- `--tools <LIST>` and `--disallowed-tools <LIST>` filter built-in tools.
- `--always-approve` bypasses permission prompts and should be used only in a
  trusted environment.
- `--sandbox <PROFILE>` selects a configured sandbox profile.

`--rules`, `--system-prompt-override`, `--verbatim`, `--disable-web-search`,
`--no-alt-screen`, `--minimal`, `--fullscreen`, `--debug`, `--debug-file`, and
`--leader-socket` provide the remaining runtime/UI controls. Use `--help` for
their version-specific value syntax.

## Commands

| Command | Purpose | Important options/subcommands |
| --- | --- | --- |
| `agent` | Run without the TUI | `stdio`, `headless`, `serve`, `leader`; `--reauth`, `--model`, `--effort`, `--always-approve`, `--agent-profile`, repeatable `--plugin-dir`, `--leader`/`--no-leader` |
| `completions <SHELL>` | Generate completion script | `bash`, `elvish`, `fish`, `powershell`, `zsh` |
| `dashboard` | Open the local agent dashboard | Respects the dashboard configuration |
| `export <SESSION_ID> [OUTPUT]` | Export a local transcript as Markdown | `--clipboard` conflicts with an output path |
| `inspect` | Show resolved configuration | `--json` |
| `leader` | Manage local shared leader processes | `list [--json]`, `info [--pid PID] [--json]`, `kill` |
| `login` | Store a Chutes API key | `--api-key-stdin` for protected automation |
| `logout` | Clear cached Chutes credentials | No subcommands |
| `mcp` | Manage MCP server configuration | `list`, `add`, `remove`, `doctor` |
| `memory` | Manage local cross-session memory | `clear [--workspace|--global|--all] [--yes]` |
| `models` | List the resolved model catalog | `--json` |
| `plugin` | Manage plugins and marketplaces | `list`, `install`, `uninstall`, `update`, `enable`, `disable`, `details`, `validate`, `tag`, `marketplace` |
| `sessions` | Manage the local session registry | `list`, `search`, `delete` |
| `trace <SESSION_ID>` | Create a local trace archive | `--output`, `--json`; remote upload is not available |
| `version` (`v`) | Print version/channel information | `--json` |
| `worktree` | Manage tracked git worktrees | `list`, `show`, `rm`, `gc`, `db` |
| `wrap <CMD>...` | Forward OSC 52 clipboard writes from a local PTY | Pass the wrapped command and arguments verbatim |

The inherited remote session/share/update/workspace-exposure commands are not
part of the Chutes Build CLI.

## Agent server

The common `agent` options apply before its mode subcommand:
`--reauthenticate`, `--model`, `--reasoning-effort`, `--always-approve`,
`--agent-profile`, repeatable `--plugin-dir`, `--leader`/`--no-leader`,
`--cli-chat-proxy-base-url`, and `--chutes-api-base-url`. Custom endpoint
overrides never inherit ambient Chutes credentials.

`chutes-build agent serve` listens on `127.0.0.1:2419` by default.
An omitted secret is generated with 32 characters; a supplied secret shorter
than 32 characters is rejected. Binding to a non-loopback address requires the
explicit `--allow-remote-bind` acknowledgement.

`agent leader --no-exit-on-disconnect` keeps the shared local leader alive when
its final client exits.

## MCP

`mcp add <NAME> [COMMAND_OR_URL] [ARGS]...` supports:

- `--transport stdio|http|sse`
- `--scope user|project`
- repeatable `--env KEY=value`
- repeatable `--header "NAME: VALUE"`

Use `--` before a stdio server command whose arguments begin with `-`.
`mcp remove <NAME> [--scope ...]` removes configuration, and
`mcp doctor [NAME] [--json]` checks parsing/connectivity.

`mcp list --json` is safe for diagnostics: it omits environment/header values,
command arguments, and URL user information, query strings, and fragments.

## Plugins and marketplaces

- `plugin install <SOURCE> [--trust]` accepts a git URL, GitHub shorthand, or
  local path.
- `plugin list --json [--available]` can include candidates from configured
  marketplace sources; `--available` requires JSON output.
- `plugin uninstall <NAME> [--keep-data] [--yes]` confirms before removal.
- `plugin update [NAME]`, `enable <NAME>`, `disable <NAME>`, and
  `details <NAME>` manage installed plugins.
- `plugin validate [PATH]` checks a manifest.
- `plugin tag [PATH] [--dry-run] [--force] [--push]` creates a release tag;
  `--push` performs an external git operation.
- `plugin marketplace list [--json]`, `add <SOURCE>`, `update [SOURCE]`, and
  `remove <SOURCE> [--yes]` manage sources. Removing a source also uninstalls
  its plugins and reports partial failures.

Per-process `agent --plugin-dir` plugins are trusted by definition and should
only point to reviewed directories.

## Sessions, traces, and exports

`sessions list` and `sessions search <QUERY>` accept `--limit` and `--json`.
`sessions delete <ID>` requires confirmation, or `--yes` for non-interactive
automation; `--json` returns a machine-readable result.

All session operations use the local store. `trace` writes a `.tar.gz` archive
under `<state-root>/trace-exports` by default and never uploads it.

## Worktrees and destructive operations

- `worktree list [--repo PATH] [--type LIST] [--all] [--json]`
- `worktree show <ID_OR_PATH>`
- `worktree rm <ID_OR_PATH>... [--force] [--dry-run] [--yes]`
- `worktree gc [--max-age DURATION] [--force] [--dry-run] [--yes]`
- `worktree db rebuild|stats|path`

`sessions delete`, `memory clear`, `plugin uninstall`,
`plugin marketplace remove`, `worktree rm`, and `worktree gc` fail closed on
non-interactive stdin unless `--yes` is supplied. Dry-run worktree operations
do not require confirmation. Failures produce a non-zero exit status instead
of being reported as successful.

## Machine-readable output

The following commands expose a stable JSON mode:

- `inspect --json`
- `leader list|info --json`
- `mcp list|doctor --json`
- `models --json`
- `plugin list --json`
- `plugin marketplace list --json`
- `sessions list|search|delete --json`
- `trace --json`
- `version --json`
- `worktree list --json`

Diagnostics intended for machines are written to stdout; operational messages
and errors use stderr.
