# Advisor and Subagents

The main executor owns the conversation and all mutations. It can consult a
read-only advisor for difficult plans, blockers, changes of approach, and final
verification. The advisor inspects the repository with read-only tools
(`read_file`, `list_dir`, `grep`, `web_fetch`, Context7 documentation, and
memory lookups) and returns advice to the executor; it has no shell, no file
editing, and cannot spawn further agents, so it can never modify the workspace.

Use `/advisor on|off` to enable or disable it, and `/advisor <model>` to pin the
model it runs on — the session's own model is left unchanged.

Worker subagents support:

- foreground and background execution;
- concurrent fan-out for independent tasks;
- waiting for multiple workers as a group;
- isolated worktrees when file edits would otherwise conflict;
- bounded nesting to prevent recursive, unreviewable swarms.

Use the `chutes-build-orchestrator` preset when a task clearly benefits from
parallel decomposition. Avoid parallel agents for tightly coupled edits or
tasks too small to justify coordination overhead.

Plugin-provided workers may inherit only the parent's already-connected MCP
pool after the normal filter. They cannot declare their own servers, hooks, or
permission policy.
