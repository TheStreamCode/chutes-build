# Chutes Build agent runtime

This crate contains the session, model, tool, memory, MCP, routing, and
orchestration runtime used by the `chutes-build` binary.

User-facing setup and architecture documentation is maintained in the
repository root. Build the workspace with Cargo and authenticate using a
Chutes API key through `chutes-build login`, protected standard input, or the
`CHUTES_API_KEY` environment variable.

Runtime data is stored under `~/.chutes-build/`. Product telemetry, remote
trace upload, session sharing, and automatic updates are disabled.

Model reasoning compatibility is centralized in `chutes-build-core`; the live
catalog may override bundled menus for forward compatibility. The virtual
`model-router` entry is presented as `Auto (Chutes Router)` and is kept separate
from concrete model reasoning controls. See the repository-level
[`docs/ARCHITECTURE.md`](../../../docs/ARCHITECTURE.md) and
[`docs/model-reasoning-compatibility.md`](../../../docs/model-reasoning-compatibility.md)
documents.

The runtime bounds repeated identical tool calls, persists remote ACP
background-task output locally, clears failed session startup state, and keeps
fork/rewind copies limited to live history and referenced checkpoint files.
