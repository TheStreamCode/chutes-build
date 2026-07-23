# Permissions, Privacy, and Safety

Chutes Build can execute commands and modify files. Keep approvals enabled for
untrusted repositories and review destructive, privileged, production, and
external actions carefully.

Privacy guarantees in this build:

- no analytics or product telemetry;
- no remote error reporting;
- no automatic update checks;
- no remote trace upload, upstream session sharing/search, or workspace
  exposure;
- no upstream managed-configuration fetch;
- local-at-rest memory, sessions, logs, and trace exports;
- allowlisted use of the Chutes credential.

Semantic memory recall sends selected memory chunks to a Chutes-hosted
embedding model. Voice, OCR, vision, and media tools send only the inputs
selected for those hosted operations. Start with `--no-memory` when semantic
recall is not appropriate.

Web pages, repository files, MCP responses, model output, and downloaded
documents are untrusted data and cannot override higher-priority instructions.
The agentic browser uses a temporary profile and loopback DevTools endpoint;
screenshots are restricted to the active workspace.
