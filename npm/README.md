# npm distribution and native packages

The public `chutes-build` package is a dependency-free Node.js launcher. Its
optional dependencies contain the native Rust executable for each supported
platform, so installation does not require Rust and does not download an
executable from a lifecycle script.

The launcher archive includes the product README, changelog, security and
privacy policies, license, and notice. It deliberately defines no install or
post-install lifecycle script; executable selection happens only when the
`chutes-build` command runs.

Supported release targets:

- Windows x64 and ARM64
- macOS x64 and ARM64
- Linux glibc x64 and ARM64

For installation and first-run instructions, see the repository
[`README`](../README.md) and the
[getting-started guide](https://github.com/TheStreamCode/chutes-build/blob/main/docs/getting-started.md).

## Prepare a platform package

Build the release executable for the target first, then stage its npm package:

```powershell
node npm/scripts/prepare-platform-package.mjs --target win32-x64 --binary target/release/chutes-build.exe --out-dir target/npm
npm pack ./target/npm/chutes-build-win32-x64
```

Valid target names are `win32-x64`, `win32-arm64`, `darwin-x64`,
`darwin-arm64`, `linux-x64-gnu`, and `linux-arm64-gnu`.

Publish all platform packages for a version before publishing the root
launcher. The version in `package.json` must match the Rust binary crate and
every platform package. Publishing is intentionally not performed by the build
or test scripts. Validate release metadata before packaging:

```powershell
npm run verify:release
npm test
npm pack --dry-run
```

The manual GitHub Actions workflow in `.github/workflows/package-release.yml`
builds all six native packages. Its publish input is disabled by default and
requires the `npm-release` environment when enabled. Native archives
are published before the launcher, every publish uses npm provenance, and an
already published package/version is skipped during a recovery run. See
the [release procedure](https://github.com/TheStreamCode/chutes-build/blob/main/docs/releasing.md)
for the complete workflow.

## Verify the launcher

The launcher accepts `CHUTES_BUILD_BINARY` as a local development override:

```powershell
$env:CHUTES_BUILD_BINARY = (Resolve-Path target/debug/chutes-build.exe)
node npm/bin/chutes-build.js --help
npm test
npm pack --dry-run
```
