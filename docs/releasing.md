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

Run that `cargo deny` **online**. It checks the live advisory database, so an
`--offline` run reuses whatever is already on disk and cannot see anything
published since — which is how a commit passes locally and fails in CI an hour
later, as `RUSTSEC-2026-0249` did on 2026-08-10. A new entry belongs in
`deny.toml` with a reason that records what was checked.

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

Step 8 is not a formality either. Doing it for 1.0.0 — `npm install chutes-build`
into a throwaway prefix, then reading every subcommand's `--help` — found five
places where the re-base had taken upstream's branding back over the fork's, three
of them flag *names* (`--grok-ws-origin`, `--grok-ws-url`, `--xai-api-base-url`).
Nothing in the build or the test suites looks at that text. Grep the installed
binary's help for `grok`, `xai` and `x.ai`, not just the version string.

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

### The shipped binary is stripped, and has to be

`release-dist` builds with `strip = false` and `debug = 1`, which is right for a
profile whose comment promises that CI extracts debug sidecars "before stripping
post-build". That post-build strip had never been written. So every npm package
carried the profile's full DWARF, and the numbers were not marginal:

| package | packed | unpacked |
| --- | ---: | ---: |
| `darwin-arm64` | 57.3 MB | 146.7 MB |
| `darwin-x64` | 60.2 MB | 158.8 MB |
| `linux-arm64-gnu` | 209.7 MB | **938.2 MB** |

The registry refused the last one outright — `E413 Payload Too Large` — which is
what stopped the 1.0.0 publish after two packages had already gone out.

The 1.0.0 publish had already put `darwin-x64` and `darwin-arm64` on the registry
before it hit that error, so those two carry their symbols and are visibly larger
than the other four. They cannot be replaced: an npm version is immutable. It is a
size wart in one release, not a fault — the launcher pins the whole set, so nothing
resolves a mismatched pair.

The `Strip the shipped binary` step now runs between the build and the smoke test,
so the run check, the package, its checksum and the release asset all see the
binary that ships. Each target builds on its own native runner, so it is the host's
own `strip`; Windows needs none, because MSVC keeps debug info in a separate
`.pdb`. The cost is line numbers in a local backtrace — worth it against a
download that the registry will not accept at all.

## Repository presentation

One thing stays manual because GitHub exposes no API for it: the **social preview**,
the 1280x640 card shown when a repository link is shared to Slack, X or Discord.
Without one, GitHub renders a generic auto-card.

The card is designed in CSS — `assets/chutes/social-preview.html` — and rendered by
`python scripts/social_preview.py`, which serves it on loopback and screenshots it at
1280x640 with `playwright-cli`. Edit the HTML, re-run the script, upload the PNG at
**Settings -> General -> Social preview**.

Designed rather than assembled from bitmaps for two reasons: real typography, and a
layout that stays balanced when a line changes — the footer sits in the flow and the
content centres in what is left, rather than against a padding constant that has to
be re-guessed. And the card is reviewable as a diff.

> [!IMPORTANT]
> `assets/chutes/screenshot/chutes-build.png` is **stale**: it reads
> "Chutes Build Beta 0.1.0". The product no longer labels itself Beta — there is a
> test in `views/welcome` asserting the version badge must not contain "Beta" — and
> the version is no longer 0.1.0. That screenshot is the README hero image and the
> source of the social card, so both currently misdescribe the build. Re-capturing it
> is a maintainer decision — `AGENTS.md` bars agents from touching the asset, and
> rightly so, since a retouched screenshot would misrepresent the product. After a
> fresh capture, re-run the script and re-upload the card.
