# Releasing Chutes Build

Releases are deliberate, reviewable operations. Normal CI never publishes a
package, creates a release, or writes to the repository.

## Repository configuration

Configure the public GitHub repository with:

- `main` as the default branch;
- private vulnerability reporting enabled;
- branch protection requiring the secrets/dependency, Linux Rust, Windows
  Rust, macOS Rust, and npm checks;
- full commit SHA pinning for every GitHub Action;
- an `npm-release` environment used only by the publish job, with required
  reviewer approval when repository policy supports it;
- an `NPM_TOKEN` environment secret limited to publishing the Chutes Build npm
  packages, with OIDC enabled for npm provenance attestations; and
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
package contents before continuing. Never place a Chutes API key in release
configuration or CI.

**Run Gitleaks against both the working tree and the complete Git history**, not
just the diff. This is not a formality: two live API keys reached seven commits on
the re-base branch, from a test that shelled out to `env` and a `git add -A` that
swept up the result. The history was purged before anything was pushed; a scan of
the diff alone would not have found them, because by then they were in earlier
commits. See the leak record in `docs/upstream-sync.md`.

The root launcher archive must contain `README.md`, `CHANGELOG.md`,
`SECURITY.md`, `PRIVACY.md`, `LICENSE`, `NOTICE`, and `npm/bin/`. It must not
define install or post-install lifecycle scripts. Verify the exact file list
with `npm pack --dry-run` before starting the release workflow.

Validate relative Markdown links and changed workflow YAML. The root README,
public guides, embedded `/docs` guides, privacy/security documents, and current
CLI behavior must agree before a release candidate is packaged.

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
7. The same run then creates the `v<version>` tag and the GitHub release, with
   the notes taken from this version's `CHANGELOG.md` section and every asset
   attached: the six platform executables, the six npm archives, and their
   checksum sidecars. Nothing to do by hand.
8. Install the published version on at least Windows and one Unix platform, then
   verify `chutes-build --version` and a non-billable startup path.

The workflow publishes native packages first because the root launcher depends
on them as optional dependencies. A failed or partial run must be investigated;
do not reuse an already published version.

### Why the release is not a manual step any more

The releases for v0.4.1, v0.4.2 and v0.4.3 exist and carry real notes. What none of
them carries is a **binary**: zero assets on all three, while the README says "take
a binary from [Releases]". Anyone who does not use npm has had nowhere to download
this program from, and the download page looked like it should work.

Attaching assets by hand, to a release created by hand, after a workflow that
already built exactly those files, is three chances to forget. The `github-release`
job runs after `publish-npm` in the same workflow, so the documented order is
preserved — npm first, tag and release last — and the assets come from the run that
built them. It refuses to publish if any of the six platform executables is missing,
because a partial release is worse than none: the download page still looks
complete. Release notes come from `npm/scripts/changelog-notes.mjs`, which slices
this version's `CHANGELOG.md` section and exits non-zero if the section is absent or
empty, so a release cannot ship with blank notes.

Assets are the bare executables as well as the npm archives. Someone downloading
from a release wants something they can run, not a tarball to unpack.

## Repository presentation

One thing stays manual because GitHub exposes no API for it: the **social preview**,
the 1280x640 card shown when a repository link is shared to Slack, X or Discord.
Without one, GitHub renders a generic auto-card.

`python scripts/social_preview.py` builds `assets/chutes/social-preview.png` from
the terminal screenshot already in the tree; upload it at **Settings -> General ->
Social preview**. Re-run the script whenever the screenshot is re-shot — it crops to
the splash's content by scanning for the panel's empty interior, so it survives a
differently sized capture.

> [!IMPORTANT]
> `assets/chutes/screenshot/chutes-build.png` is **stale**: it reads
> "Chutes Build Beta 0.1.0". The product no longer labels itself Beta — there is a
> test in `views/welcome` asserting the version badge must not contain "Beta" — and
> the version is no longer 0.1.0. That screenshot is the README hero image and the
> source of the social card, so both currently misdescribe the build. Re-capturing it
> is a maintainer decision — `AGENTS.md` bars agents from touching the asset, and
> rightly so, since a retouched screenshot would misrepresent the product. After a
> fresh capture, re-run the script and re-upload the card.
