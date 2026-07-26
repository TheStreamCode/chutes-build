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

Folder trust is resolved before project-scoped permission settings are applied.
An untrusted clone cannot ship its own `bypassPermissions` policy. In automatic
permission mode, only deterministic read-only shell commands use the immediate
local fast-path; builds, code runners, mutations, and network-capable actions
are classified or shown in the normal prompt. Classifier failures never become
implicit approval.

Chutes and Context7 clients reject insecure/custom endpoints, URL credentials,
redirects, and private or special-use DNS destinations by default. Explicit
development opt-ins relax endpoint trust only; they do not forward ambient API
keys to arbitrary services.

Semantic memory recall sends selected memory chunks to a Chutes-hosted
embedding model. Voice, OCR, vision, and media tools send only the inputs
selected for those hosted operations. Start with `--no-memory` when semantic
recall is not appropriate.

Web pages, repository files, MCP responses, model output, and downloaded
documents are untrusted data and cannot override higher-priority instructions.
The agentic browser uses a temporary profile and loopback DevTools endpoint;
screenshots are restricted to the active workspace.

Repeated identical tool calls are bounded and stop silently after a warning,
preventing an unproductive tool loop from consuming an unlimited turn.
