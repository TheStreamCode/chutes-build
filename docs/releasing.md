# Releasing Chutes Build

Releases are deliberate, reviewable operations. Normal CI never publishes a
package, creates a release, or writes to the repository.

## Repository configuration

Configure the public GitHub repository with:

- `main` as the default branch;
- private vulnerability reporting enabled;
- branch protection requiring the secrets/dependency, Linux Rust, Windows
  Rust, and npm checks;
- an `npm-release` environment with required reviewer approval;
- an `NPM_TOKEN` environment secret limited to publishing the Chutes Build npm
  packages; and
- Actions permissions restricted to read-only by default.

Recommended repository topics are `chutes`, `coding-agent`, `cli`, `rust`,
`tui`, `ai-agents`, `mcp`, and `multimodal`.

## Version gate

Use one version for the root npm launcher, every native optional dependency,
the `chutes-build` Rust binary crate, and the lockstepped runtime crates checked
by `npm run verify:release`. After updating those manifests, run:

```powershell
npm run verify:release
npm test
npm pack --dry-run
$env:CARGO_BUILD_JOBS = "1"
cargo check -p chutes-build --locked
cargo deny --locked check advisories licenses bans sources
```

Review `CHANGELOG.md`, `LICENSE`, `NOTICE`, `THIRD-PARTY-NOTICES`, and the npm
package contents before continuing. Run Gitleaks against both the working tree
and complete Git history. Never place a Chutes API key in release configuration
or CI.

## Build and publish

1. Run the `Package release` workflow from `main` with `publish` disabled.
2. Confirm all six native binary smoke tests, SHA-256 verification, and the
   assembled Linux launcher test passed.
3. Inspect the six retained npm archives and checksum sidecars; confirm their
   target, version, executable name, license, and notices.
4. Run the workflow again from `main` with `publish` enabled.
5. Approve the protected `npm-release` environment after reviewing the run.
6. Confirm all six native packages were published before the root
   `chutes-build` launcher.
7. Install the published version on at least Windows and one Unix platform,
   then verify `chutes-build --version` and a non-billable startup path.
8. Create the matching `v<version>` Git tag and GitHub release only after npm
   installation is verified.

The workflow publishes native packages first because the root launcher depends
on them as optional dependencies. A failed or partial run must be investigated;
do not reuse an already published version.
