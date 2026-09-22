<div align="center">

<h1>Chutes Build (<code>chutes-build</code>)</h1>

[![CI](https://github.com/TheStreamCode/chutes-build/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/TheStreamCode/chutes-build/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/chutes-build?logo=npm&color=cb3837)](https://www.npmjs.com/package/chutes-build)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

**Chutes Build** is a terminal-based AI coding agent for the
[Chutes](https://chutes.ai) ecosystem. It runs as a full-screen TUI that
understands your codebase, edits files, executes shell commands, searches the web,
and manages long-running tasks — interactively, headlessly for scripting/CI, or
embedded in editors via the Agent Client Protocol (ACP).

[Installing](#installing) ·
[Building from source](#building-from-source) ·
[Documentation](#documentation) ·
[Repository layout](#repository-layout) ·
[Development](#development) ·
[Troubleshooting](#troubleshooting) ·
[Contributing](#contributing) ·
[License](#license)

![Chutes Build TUI](assets/chutes/screenshot/chutes-build.png)

</div>

## What this is, and what it is built on

Chutes Build is a fork of [`xai-org/grok-build`](https://github.com/xai-org/grok-build)
— SpaceXAI's Grok Build — re-based onto upstream 1.0.0 and adapted to run on
Chutes. Upstream wrote most of the code in this tree and it is used here under
Apache-2.0; the project is not affiliated with or endorsed by SpaceXAI.

What the fork changes:

- **Chutes, not xAI.** Inference goes to `llm.chutes.ai`, the model catalog is
  Chutes', the default route is Chutes' native model routing (the saved
  `default` pool, or an inline pool via `CHUTES_ROUTING_POOL`), and media
  generation, OCR, usage and quota all go through Chutes APIs.
- **Nothing phones home.** Telemetry, remote error reporting, upload, session
  sharing, remote workspace exposure and self-update are off at compile time and
  their endpoints are deadened, not merely unused. See
  [`PRIVACY.md`](PRIVACY.md).
- **A few exclusive features**: Chutes media generation and editing, OCR, voice
  input through a Chutes STT endpoint, Context7 documentation lookup, and an
  isolated browser tool.

Everything else is meant to track upstream closely, so their fixes arrive by
merge rather than by hand. [`docs/upstream-sync.md`](docs/upstream-sync.md)
documents that procedure and records each sync;
[`.github/upstream.json`](.github/upstream.json) records the commit this tree is
level with. `SOURCE_REV` is inherited from upstream's published tree and refers to
their monorepo, not to anything in this repository.

---

## Installing

Via npm, which fetches the binary for your platform:

```sh
npm i -g chutes-build
chutes-build --version
```

Or build from source below.

From 1.0.0 on, each
[release](https://github.com/TheStreamCode/chutes-build/releases) also carries the
six platform executables as downloadable assets. Earlier releases (0.4.x) carry
release notes only — npm is the way to install those.

There is no self-update: `chutes-build` never contacts an update server, and the
`update` subcommand reports where to get a newer version rather than fetching one.
[`CHANGELOG.md`](CHANGELOG.md) records what changed in each release.

## Building from source

Requirements:

- **Rust** — the toolchain is pinned by [`rust-toolchain.toml`](rust-toolchain.toml);
  `rustup` installs it automatically on first build.
- **[DotSlash](https://dotslash-cli.com)** — required so hermetic tools under
  [`bin/`](bin/) (notably [`bin/protoc`](bin/protoc)) can download and run.
  Install it and ensure `dotslash` is on your `PATH` **before** building:

  ```sh
  cargo install dotslash
  # or: prebuilt packages — https://dotslash-cli.com/docs/installation/
  /usr/bin/env dotslash --help   # sanity check
  ```

- **protoc** — proto codegen resolves [`bin/protoc`](bin/protoc) via DotSlash,
  or falls back to a `protoc` on `PATH` / `$PROTOC`.
- Windows, macOS and Linux all build. Windows is where this tree is developed and
  gated, and it needs the vendored `protoc` fallback carried here — upstream 1.0.0
  does not build on Windows at all, because `bin/protoc` is a DotSlash wrapper the
  platform cannot execute from a build script.

```sh
cargo run -p chutes-build              # build + launch the TUI
cargo build -p chutes-build --release  # release binary: target/release/chutes-build
cargo check -p chutes-build            # fast validation
```

The binary artifact is named `xai-grok-pager`; official installs ship it as
`chutes-build`. On first launch it asks for a Chutes API key — the primary
credential, created at [chutes.ai/app/api](https://chutes.ai/app/api) and also
readable from `CHUTES_API_KEY`. Browser login is opt-in and needs an OAuth app you
register yourself; see the
[authentication guide](crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md).

## Documentation

The documentation lives in this repository. There is no separate site: an earlier
link to `docs.chutes.ai/build/overview` was a 404 dressed as a promise.

The user guide ships with the pager crate:
[`crates/codegen/xai-grok-pager/docs/user-guide/`](crates/codegen/xai-grok-pager/docs/user-guide/)
— getting started, keyboard shortcuts, slash commands, configuration, theming,
MCP servers, skills, plugins, hooks, headless mode, sandboxing, and more.

## Repository layout

| Path | Contents |
|------|----------|
| `crates/codegen/xai-grok-pager-bin` | Composition-root package `chutes-build`; builds the `chutes-build` binary |
| `crates/codegen/xai-grok-pager` | The TUI: scrollback, prompt, modals, rendering |
| `crates/codegen/xai-grok-shell` | Agent runtime + leader/stdio/headless entry points |
| `crates/codegen/xai-grok-tools` | Tool implementations (terminal, file edit, search, ...) |
| `crates/codegen/xai-grok-workspace` | Host filesystem, VCS, execution, checkpoints |
| `crates/codegen/...` | The rest of the CLI crate closure (config, MCP, markdown, sandbox, ...) |
| `crates/common/`, `crates/build/`, `prod/mc/` | Small shared leaf crates pulled in by the closure |
| `third_party/` | Vendored upstream source (Mermaid diagram stack) — see below |

> [!IMPORTANT]
> The root `Cargo.toml` (workspace members, dependency versions, lints,
> profiles) is **generated** — treat it as read-only. Prefer editing per-crate
> `Cargo.toml` files.

## Development

```sh
cargo check -p <crate>        # always target specific crates; full-workspace builds are slow
cargo test -p xai-grok-config # per-crate tests
cargo clippy -p <crate>       # lint config: clippy.toml at the repo root
```

## Troubleshooting

### Missing `CHUTES_API_KEY`

On first launch `chutes-build` asks for a Chutes API key (created at
[chutes.ai/app/api](https://chutes.ai/app/api)); it is also readable from the
`CHUTES_API_KEY` environment variable. If requests fail with authentication
errors, check that the variable is exported in the launching shell — Chutes
Build reads the process environment and does not load `.env` files
automatically (see [`docs/configuration.md`](docs/configuration.md)).
Never commit a real key; use your shell, CI platform, or secret manager to
provide it, and run `chutes-build inspect --json` to verify the effective
non-secret configuration.

### Model-pool routing errors

The default model route is Chutes' native routing: the saved `default` pool
(managed at chutes.ai/app → Model Routing), or an inline pool via
`CHUTES_ROUTING_POOL` (comma-separated catalogue ids sent as one `model`
value, e.g. `modelA,modelB,modelC`). If the `default` alias cannot resolve
and no dashboard pool is saved, the built-in fallback chain steps down to a
live inline pool from the current catalogue; a saved dashboard pool, once
configured, takes priority. Common fixes: save a pool on the dashboard, set
`CHUTES_ROUTING_POOL` explicitly, check `CHUTES_FALLBACK_MODELS` ordering (or
`CHUTES_STRICT_MODEL=1`, which disables automatic fallback), and pick a
strategy with `CHUTES_ROUTING_STRATEGY` (`sequential` default, `latency`, or
`throughput`). Full reference: [`docs/configuration.md`](docs/configuration.md)
("Native model routing").

### Windows console shows wrong glyphs

The TUI degrades decorative glyphs on the legacy Windows console (ConHost
`cmd.exe` / `powershell.exe`, Consolas/Lucida Console raster font, no font
fallback): arrows, checkmarks, diamonds, and spinners fall back to ASCII or
CP437-safe stand-ins (see
[`crates/codegen/xai-grok-pager-render/src/glyphs.rs`](crates/codegen/xai-grok-pager-render/src/glyphs.rs)).
This is expected on legacy ConHost — for the full rendering use Windows
Terminal, VS Code's terminal, or another modern emulator. Note that CI forces
the modern-glyph path with `CHUTES_BUILD_FORCE_LEGACY_CONSOLE=0` (see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml)), so screenshots and
test rendering may look richer than a legacy local console.


## Contributing

> [!NOTE]
> External contributions are not accepted. See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

First-party code in this repository is licensed under the **Apache License,
Version 2.0** — see [`LICENSE`](LICENSE).

Third-party and vendored code remains under its original licenses. See:

- [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES) — crates.io / git dependencies,
  bundled UI themes, and **in-tree source ports** (including openai/codex and
  sst/opencode tool implementations)
- [`crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md`](crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md)
  — crate-local notice for the codex and opencode ports (license texts +
  Apache §4(b) change notice)
- [`third_party/NOTICE`](third_party/NOTICE) — vendored Mermaid-stack index
