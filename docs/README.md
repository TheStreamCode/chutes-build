# Documentation

This directory contains repository-level technical references and visual
prototypes for Chutes Build.

## Technical references

- [CLI reference](cli-reference.md) documents every visible top-level command,
  global option family, machine-readable mode, and destructive-operation gate.
- [Slash commands](slash-commands.md) lists the interactive command surface
  registered by the TUI.
- [Architecture](ARCHITECTURE.md) describes runtime layers, model routing,
  reasoning isolation, inference transport, account quota normalization,
  official-source policy, orchestration, and privacy boundaries.
- [Model reasoning compatibility](model-reasoning-compatibility.md) records the
  current Chutes model matrix, upstream evidence, UI choices, wire behavior,
  and forward-compatibility policy.
- [Token efficiency plan](token-efficiency-plan.md) tracks the implemented
  low-risk reductions plus measurement, quality gates, and remaining staged
  work for reducing model input and latency.
- [MediaArtifact plan](media-artifact-plan.md) documents the implemented typed
  media path, bounded image and video previews, local audio controls, native
  fallbacks, lifecycle constraints, and the remaining terminal-coverage work.
- [Privacy](../PRIVACY.md) is the authoritative description of local state,
  diagnostics, outbound connections, browser isolation, and disabled cloud
  behavior.
- [Security review](security-review.md) records the release threat surfaces,
  implemented controls, dependency policy, and verification commands.
- [Releasing](releasing.md) documents repository protections, version gates,
  cross-platform packaging, and the manual npm publication workflow.
- [Upstream sync](upstream-sync.md) defines how new `xai-org/grok-build`
  releases and commits are detected, reviewed, ported, and recorded.

The concise user guides embedded in the CLI live under
`crates/codegen/xai-grok-pager/docs/user-guide/` and are available through
`/docs`.

## Visual prototypes

- `ascii-logo-concepts.html` contains terminal identity studies.
- `chutes-build-promo.html` contains the animated product presentation.

These HTML files are design artifacts, not runtime dependencies. Keep product
claims in them aligned with implemented and documented behavior.
