# Hook Examples

Sample hooks for Chutes Build. Copy to `~/.chutes-build/hooks/` to enable globally, or to `<project>/.chutes-build/hooks/` for project-scoped hooks (requires `/hooks-trust`).

The shell-script examples require Bash (for example Git for Windows Bash, WSL,
or a Unix host). The Python guard requires `python3`. On Windows, copy the
files with PowerShell; executable-bit changes are not required on NTFS.

## Available Examples

### 1. Safe Shell Guard (`safe-shell.json`)

**Type:** blocking (`PreToolUse`)

Denies obviously destructive shell commands before they execute:
- `rm -rf /`, `sudo rm -rf`, `mkfs`, `dd` to devices, fork bombs

**Install (PowerShell):**
```powershell
$hooks = Join-Path $HOME ".chutes-build\hooks"
New-Item -ItemType Directory -Force -Path (Join-Path $hooks "bin")
Copy-Item "examples\hooks\safe-shell.json" $hooks
Copy-Item "examples\hooks\bin\safe-shell-guard.sh" (Join-Path $hooks "bin")
```

**Install (Bash):**
```sh
mkdir -p ~/.chutes-build/hooks/bin
cp examples/hooks/safe-shell.json ~/.chutes-build/hooks/
cp examples/hooks/bin/safe-shell-guard.sh ~/.chutes-build/hooks/bin/
chmod +x ~/.chutes-build/hooks/bin/safe-shell-guard.sh
```

### 2. No Recursive Grep (`no-recursive-grep.json`)

**Type:** blocking (`PreToolUse`)

Denies recursive `grep` invocations in the shell before they execute:
- `grep -r`, `grep -R`, `grep --recursive`, `grep --dereference-recursive`,
  `grep -d recurse`, clustered flags (`grep -rn`, `grep -nri`), and `rgrep`

Recursive grep walks an entire directory tree into memory and can OOM-kill the
agent process on large repos. The system prompt already steers the model away from
this, but a prompt is advisory — this hook makes it a hard, deterministic block.
Point the model at the dedicated search tool (ripgrep-backed) instead.

It is careful to avoid false positives: `ls -R | grep foo` (the `-R` belongs to
`ls`), `grep -e -r file` (`-r` is the pattern), and `grep -- -r file` are all
allowed.

**Install (PowerShell):**
```powershell
$hooks = Join-Path $HOME ".chutes-build\hooks"
New-Item -ItemType Directory -Force -Path (Join-Path $hooks "bin")
Copy-Item "examples\hooks\no-recursive-grep.json" $hooks
Copy-Item "examples\hooks\bin\no-recursive-grep-guard.py" (Join-Path $hooks "bin")
```

**Install (Bash):**
```sh
mkdir -p ~/.chutes-build/hooks/bin
cp examples/hooks/no-recursive-grep.json ~/.chutes-build/hooks/
cp examples/hooks/bin/no-recursive-grep-guard.py ~/.chutes-build/hooks/bin/
chmod +x ~/.chutes-build/hooks/bin/no-recursive-grep-guard.py
```
(Requires `python3` on `PATH`.)

### 3. Session Audit Log (`session-log.json`)

**Type:** passive (`SessionStart` + `SessionEnd`)

Appends session metadata to `~/.chutes-build/session-audit.log` — event, session ID, cwd, timestamp.

**Install (PowerShell):**
```powershell
$hooks = Join-Path $HOME ".chutes-build\hooks"
New-Item -ItemType Directory -Force -Path (Join-Path $hooks "bin")
Copy-Item "examples\hooks\session-log.json" $hooks
Copy-Item "examples\hooks\bin\session-log.sh" (Join-Path $hooks "bin")
```

**Install (Bash):**
```sh
mkdir -p ~/.chutes-build/hooks/bin
cp examples/hooks/session-log.json ~/.chutes-build/hooks/
cp examples/hooks/bin/session-log.sh ~/.chutes-build/hooks/bin/
chmod +x ~/.chutes-build/hooks/bin/session-log.sh
```

### 4. Tool Activity Logger (`tool-logger.json`)

**Type:** passive (`PreToolUse` + `PostToolUse`)

Logs all tool calls to `~/.chutes-build/tool-activity.log` — tool name, event type, effective tool name, backgrounded status.

**Install (PowerShell):**
```powershell
$hooks = Join-Path $HOME ".chutes-build\hooks"
New-Item -ItemType Directory -Force -Path (Join-Path $hooks "bin")
Copy-Item "examples\hooks\tool-logger.json" $hooks
Copy-Item "examples\hooks\bin\tool-logger.sh" (Join-Path $hooks "bin")
```

**Install (Bash):**
```sh
mkdir -p ~/.chutes-build/hooks/bin
cp examples/hooks/tool-logger.json ~/.chutes-build/hooks/
cp examples/hooks/bin/tool-logger.sh ~/.chutes-build/hooks/bin/
chmod +x ~/.chutes-build/hooks/bin/tool-logger.sh
```

## Format

Hook files use the Claude-compatible JSON format:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "bin/check.sh", "timeout": 5 }
        ]
      }
    ]
  }
}
```

- **Event names:** `SessionStart`, `PreToolUse`, `PostToolUse`, `SessionEnd`
- **Matcher:** regex on tool name. Claude-compatible names like `Bash`, `Read`,
  and `Edit` are auto-expanded to the runtime names (`run_terminal_cmd`,
  `read_file`, and `search_replace`).
- **Timeout:** in seconds (default: 5)
- **Command:** path to script (relative to hook file directory) or inline shell command

## Script Contract

Scripts receive the hook event envelope as JSON on **stdin** and should write a response to **stdout**:

**For blocking hooks (`PreToolUse`):**
```json
{"decision":"allow"}
```
or
```json
{"decision":"deny","reason":"Explanation for the user"}
```

**Exit codes:** `0` = allow, `2` = deny, other = fail-open.

**For passive hooks:** stdout is informational only. Exit `0` for success.

## Uninstall

Remove the JSON file from `~/.chutes-build/hooks/`. The hook stops running on the next session.
