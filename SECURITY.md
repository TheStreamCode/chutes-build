# Security Policy

## Supported versions

Chutes Build is currently a development preview. Security fixes are applied to
the latest revision of the default branch; older source snapshots are not
supported.

## Reporting a vulnerability

Use the repository's private GitHub Security Advisory reporting flow. Do not
open a public issue and do not include live credentials, private source code,
session archives, or personal data in a report.

Include:

- the affected revision and platform;
- a concise description of the impact and trust boundary;
- minimal reproduction steps using synthetic data;
- whether exploitation requires user approval, repository control, browser
  access, or a malicious model/tool response;
- any proposed mitigation.

You should receive an acknowledgement when the report is reviewed. Disclosure
and remediation timing will be coordinated according to severity and the
availability of a safe fix.

## Security boundaries

Chutes Build can execute commands, modify files, invoke models, start subagents,
and control an isolated browser. Model output, repository contents, web pages,
MCP servers, tool responses, and downloaded documents are untrusted input. Keep
permission prompts enabled, use trusted repositories, and never expose secrets
in prompts or committed files.

The project intentionally disables telemetry, remote trace uploads, automatic
updates, upstream session sharing/search, remote workspace exposure, and
upstream managed configuration. These are compile-time product policies, not
server-controlled feature flags. Any change to those guarantees requires an
explicit security and privacy review.

Ambient Chutes credentials are restricted to allowlisted official HTTPS hosts.
Custom inference and model-catalog endpoints require dedicated credentials.
OAuth client secrets are read from the environment, used for token exchange
and refresh when configured, and are never persisted by Chutes Build.

Machine-readable MCP listings redact environment/header values and URL
credentials. Destructive session, memory, plugin, marketplace, and worktree
operations require an interactive confirmation unless an explicit `--yes`
flag is provided. Media downloads enforce HTTPS, redirect and destination
checks, private-address rejection, size limits, and transactional artifact
writes.

## Automated release checks

CI scans the complete Git history for known secret patterns and evaluates the
resolved Rust dependency graph for advisories, licenses, duplicate versions,
and unapproved sources. Release packaging additionally executes every native
binary, creates SHA-256 sidecars for native npm archives, verifies those
sidecars after artifact download, and runs the assembled launcher with its
native Linux package before publication can begin.

Documented advisory exceptions must include a bounded rationale in `deny.toml`.
New advisories or source-policy violations fail CI by default. The current
review record is maintained in [docs/security-review.md](docs/security-review.md).
