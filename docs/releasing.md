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

> [!IMPORTANT]
> **Regenerate `Cargo.lock` before committing the bump.** The workflow builds with
> `--locked`, and a lock still naming the old version fails every one of the six
> native jobs with `cannot update the lock file`. `npm run verify:release` checks
> the manifests and does not touch the lock; `cargo check -p chutes-build --locked`
> is what tells you. The 1.0.1 release failed exactly here on its first attempt.

```powershell
npm run verify:release
npm test
npm pack --dry-run
$env:CARGO_BUILD_JOBS = "1"
cargo check -p chutes-build --locked
python scripts/known_failures.py --crate chutes-tools --crate chutes-workspace
cargo deny --locked check advisories licenses bans sources
```

`known_failures.py` is here because CI does not run it: the set of failures it
compares is not stable across machines or runs, so it cannot be a gate (see
`AGENTS.md`). Locally it still answers the question that matters before a
release — did anything in the ~188 tests the CI filters skip change under me? —
and this is the only place that asks it. Read a red result rather than
`--record`-ing past it; the baseline is the claim that nothing new broke, and
rewriting it to match reality turns the claim into a tautology.

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

Push-to-publish: one tag, one run, no manual dispatch.

1. Land the version bump on `main` (manifests, `Cargo.lock`, `CHANGELOG.md`
   section — the release notes are sliced from it, and an absent section
   fails the run).
2. Tag and push the tag from the release commit:
   `git tag v<version> && git push origin v<version>`.
3. The tag push runs `Package release` once: validate (including a
   tag-matches-`package.json` check), the six native builds, the assembled
   launcher smoke test, npm publish (native packages first, launcher last),
   and the GitHub release with every asset attached — the six platform
   executables, the six npm archives, and their checksum sidecars. Nothing
   to do by hand.
4. Confirm `npm view chutes-build version`, the `v<version>` release and its
   assets, allowing a few minutes for registry propagation.
5. Install the published version on at least Windows and one Unix platform,
   then verify `chutes-build --version` and a non-billable startup path.

`workflow_dispatch` remains for validation-only runs (`publish` disabled)
and for re-publishing after a partial failure (`publish` enabled reuses the
idempotent publish jobs). A failed or partial run must be investigated; do
not reuse an already published version.

Step 5 is not a formality either. Doing it for 1.0.0 — `npm install chutes-build`
into a throwaway prefix, then reading every subcommand's `--help` — found five
places where the re-base had taken upstream's branding back over the fork's, three
of them flag *names* (`--grok-ws-origin`, `--grok-ws-url`, `--xai-api-base-url`).
Nothing in the build or the test suites looks at that text. Grep the installed
binary's help for `grok`, `xai` and `x.ai`, not just the version string.

**And then start it.** That help sweep read every command's text and still missed
the thing a user sees first: 1.0.0 shipped Grok's wordmark on the splash, because
the re-base had replaced both logo assets with upstream's. `--help` never draws it.
Launch the TUI, look at the welcome screen, and open the settings and the usage
paths — the upsell there was offering a SuperGrok subscription. A test now pins the
wordmark's hash so that particular loss cannot repeat silently, but the general
lesson is that reading strings is not the same as running the program.

The workflow publishes native packages first because the root launcher depends
on them as optional dependencies.

**A cancelled build job ships nothing, even when it built everything.** The first
1.0.3 run produced all six artifacts and published none. `darwin-arm64` finished
in 118m40s of a 120-minute budget — binary, strip, smoke test, package, checksum
and upload all green — and the wall then killed the cache-save step eighty
seconds later. A cancelled job fails the run, so both publish jobs were skipped.
The budget is 180 minutes now, sized against the 102-minute slowest build of the
previous three releases rather than against the fastest.

So when a release "fails", read the *steps*, not the job conclusion. If every
step that produces an artifact succeeded, nothing is wrong with the code and the
fix belongs in the workflow. Check `npm view chutes-build version` and
`gh release view` before doing anything else: if neither moved, the version is
still free and a re-dispatch is clean. That check is what made cancelling and
re-running safe here rather than burning a version number.

### A PUT 404 is an auth failure, and the secret has two homes

The 1.5.0 publish failed on its first package — all six builds and the smoke
test green — with `npm error 404 Not Found - PUT
https://registry.npmjs.org/chutes-build-darwin-arm64`. The package exists
(through 1.4.0); npm answers 404 to an unauthorized publish, so on an
existing package that error means the credential, never the code. The release
files were byte-identical to the working 1.4.0 apart from the version bumps,
which closed the case from the other side.

The credential had two layers of wrong. The `NPM_TOKEN` the job reads lives on
the `npm-release` environment, where it had sat untouched since July and died
quietly — and an environment secret **shadows** a repo secret of the same
name, so two fresh repo-level tokens had exactly zero effect until the new
token was set on the environment with `gh secret set NPM_TOKEN --env
npm-release`. There is deliberately no repo-level `NPM_TOKEN` any more: one
home for the secret means the next failure cannot be this one again. Verify a
replacement token with `npm whoami` against the package maintainers before
re-running; then `gh run rerun <id> --failed` — the publish loop skips what
already landed, so no version burns.

One more timing trap in the same run: npm answers `+ chutes-build@1.5.0`
followed by "Your package is being processed and may take a few minutes to
become available", and it means it — the packument still shows the old
`latest` for minutes after a genuinely successful PUT. Wait before declaring
a phantom failure.

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
