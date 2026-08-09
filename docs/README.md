# Documentation

This directory is the public documentation hub for Chutes Build. User guides,
maintainer procedures, and design records are separated below so the status of
each document is explicit.

## Use Chutes Build

- [Getting started](getting-started.md) — installation, authentication, first
  session, headless use, safety habits, and troubleshooting.
- [Configuration](configuration.md) — state layout, credentials, routing,
  browser/search options, media limits, diagnostics, and development overrides.
- [CLI reference](cli-reference.md) — top-level commands, options, JSON modes,
  MCP, plugins, sessions, and worktrees.
- [Interactive slash commands](slash-commands.md) — the built-in TUI command
  surface and capability-gated entries.
- [Model reasoning compatibility](model-reasoning-compatibility.md) — exact
  model-generation controls and forward-compatibility rules.
- [Privacy](../PRIVACY.md) and [security policy](../SECURITY.md) — authoritative
  data boundaries, trust model, supported versions, and vulnerability reports.

The concise guides shown by `/docs` live under
`crates/codegen/xai-grok-pager/docs/user-guide/` and are embedded into the
binary. Update them with the matching public guide whenever behavior changes.

## Maintain the project

- [Architecture](ARCHITECTURE.md) — runtime layers, routing, media lifecycle,
  ownership boundaries, and retained upstream infrastructure.
- [Security review](security-review.md) — implemented controls, accepted
  dependency exceptions, residual boundaries, and release gates.
- [Code and security audit (2026-08-01)](code-review-2026-08-01.md) — dated
  findings across architecture, correctness, security, performance,
  maintainability, dependencies, and repository operations.
- [Releasing](releasing.md) — repository protection, version alignment,
  packaging, verification, and manual npm publication.
- [npm distribution](../npm/README.md) — launcher/native package boundaries,
  archive contents, local verification, and publication safeguards.
- [Upstream synchronization](upstream-sync.md) — the merge procedure, the Chutes
  seams a merge is expected to conflict in, the deliberate divergences, the
  verification that follows, and a record of each sync. Since the 1.0.0 re-base
  this is a merge, not a selective port.
- Two scripts serve that procedure: `scripts/rebrand.py` re-applies the product
  identity and fails loudly on anything it does not recognise, and
  `scripts/seam_sweep.py` compares the values a previous release shipped against
  the current tree — because a constant whose value came from upstream compiles
  fine and its tests assert against the constant.
- [Contributing](../CONTRIBUTING.md) — development workflow and pull-request
  expectations.

## Design records

These documents describe implemented architecture plus clearly labelled
remaining work. They are not user setup guides.

- [Media artifact architecture](media-artifact-plan.md) — typed artifact
  transport, previews, playback, lifecycle limits, and remaining terminal
  coverage.
- [Token efficiency plan](token-efficiency-plan.md) — implemented low-risk
  reductions, measurement strategy, and staged follow-up work.

## Presentation sources

- `ascii-logo-concepts.html` contains terminal identity studies.
- `chutes-build-promo.html` is a standalone motion-presentation source.

These HTML files are design artifacts, not runtime dependencies. Public product
claims belong in the Markdown documentation and must match implemented behavior.
