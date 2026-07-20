# Contributing to Chutes Build

Thank you for improving Chutes Build. Focused issues and pull requests are
welcome.

## Before opening a change

1. Search existing issues and pull requests.
2. Keep the proposal specific to Chutes Build and its supported runtime.
3. Discuss major architecture changes in an issue before implementation.
4. Report security vulnerabilities through the private process in
   [SECURITY.md](SECURITY.md), never in a public issue.

## Development workflow

Chutes Build is a Rust workspace. Use the toolchain and dependency versions
already selected by the repository and do not mix package managers or regenerate
unrelated lockfiles.

```powershell
cargo check -p chutes-build
cargo test -p chutes-build-core
cargo fmt --all -- --check
```

Run the smallest relevant tests first, then expand verification according to the
risk of the change. Network-dependent tests must not require a contributor's
personal credentials or make billable calls by default.

## Model compatibility changes

Reasoning compatibility belongs in the centralized registry at
`crates/chutes-build-core/src/reasoning.rs`. When adding or updating a model:

1. verify the exact deployed model ID against the Chutes catalog;
2. verify controls and defaults against the official model card or chat
   template;
3. add registry and sampler tests, including fallback-family isolation when
   relevant; and
4. update `docs/model-reasoning-compatibility.md` with primary-source links and
   the verification date.

Do not infer controls for a future generation from a provider or family prefix.
An explicit capability menu from the catalog or user configuration is the
forward-compatibility mechanism.

## Pull-request expectations

- Keep changes narrow and preserve unrelated behavior.
- Add tests for new public behavior and regressions where practical.
- Update documentation when commands, configuration, privacy boundaries, or
  network behavior change.
- Never commit credentials, session data, browser profiles, traces, or generated
  media containing sensitive information.
- Do not add telemetry, tracking, remote error reporting, automatic upload, or
  a new outbound service without explicit project discussion and an updated
  privacy review.
- Preserve Apache-2.0 attribution when modifying or moving upstream code.
- Use professional English for code, documentation, and commit messages.
- Do not add generated-by footers or AI co-author trailers.

By contributing, you agree that your contribution is licensed under the
repository's Apache License 2.0.
