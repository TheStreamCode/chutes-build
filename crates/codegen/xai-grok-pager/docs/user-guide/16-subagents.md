# Advisor and Subagents

The main executor owns the conversation and all mutations. It can consult a
read-only advisor for difficult plans, blockers, changes of approach, and final
verification. Advice is returned to the executor; the advisor cannot execute
tools or modify the workspace.

Worker subagents support:

- foreground and background execution;
- concurrent fan-out for independent tasks;
- waiting for multiple workers as a group;
- isolated worktrees when file edits would otherwise conflict;
- bounded nesting to prevent recursive, unreviewable swarms.

Use the `chutes-build-orchestrator` preset when a task clearly benefits from
parallel decomposition. Avoid parallel agents for tightly coupled edits or
tasks too small to justify coordination overhead.
