# Permissions, Privacy, and Safety

Chutes Build can execute commands and modify files. Keep approvals enabled for
untrusted repositories and review destructive, privileged, production, and
external actions carefully.

Privacy guarantees in this build:

- no analytics or product telemetry;
- no remote error reporting;
- no automatic update checks;
- no remote trace upload or upstream session sharing;
- no upstream managed-configuration fetch;
- local-only memory, sessions, logs, and trace exports;
- allowlisted use of the Chutes credential.

Web pages, repository files, MCP responses, model output, and downloaded
documents are untrusted data and cannot override higher-priority instructions.
The agentic browser uses a temporary profile and loopback DevTools endpoint;
screenshots are restricted to the active workspace.
