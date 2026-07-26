# Upstream synchronization

Chutes Build is a specialized fork of `xai-org/grok-build`. Upstream changes
are monitored, but they are never merged or released automatically.

## Automated monitoring

The daily `Upstream watch` workflow checks both the latest GitHub release and
the head of upstream `main`. Upstream currently has no published releases or
tags, so commit monitoring is the active signal. When the reviewed baseline is
outdated, the workflow opens one review issue and refreshes its body while that
issue remains open, so the recorded current commit/version never goes stale.

The last reviewed commit, upstream source version, and release are recorded in
`.github/upstream.json`. These values are independent of the Chutes Build
product version. Update them only after completing the review and verification
below.

## Review procedure

1. Read the upstream release notes when a release exists, then inspect commits
   between `lastReviewedCommit` and the current upstream head.
2. Classify changes as runtime fixes, security fixes, performance improvements,
   dependencies, tests, or upstream-specific product behavior.
3. Port only changes that benefit Chutes Build. Preserve Chutes routing,
   privacy defaults, disabled telemetry, product identity, terminal behavior,
   and public license notices.
4. Resolve changes in small, reviewable patches instead of merging upstream
   `main` wholesale.
5. Run focused tests for each port, followed by the repository CI gates. For
   inference changes, compare time to first token, streaming cadence, token use,
   fallback behavior, and output quality against the prior Chutes Build state.
6. Record user-visible changes in `CHANGELOG.md` and update affected technical
   documentation.
7. Set `lastReviewedCommit`, `lastReviewedVersion`, `lastReviewedRelease`, and
   `reviewedAt` to the state actually reviewed, then close the upstream review
   issue.

Do not advance the baseline when a required build/test gate times out or remains
inconclusive. Record the port in `CHANGELOG.md`, keep the review issue current,
and advance the baseline only after local or CI evidence closes the gap. The
generated root `Cargo.toml` is not an upstream-port edit surface; change the
owning crate manifest or generator instead.

## Local inspection

Keep `origin` pointed at Chutes Build and `upstream` pointed at the source fork:

```powershell
git remote -v
git fetch upstream main
$baseline = (Get-Content .github/upstream.json | ConvertFrom-Json).lastReviewedCommit
git log --oneline "$baseline..upstream/main"
git diff --stat "$baseline..upstream/main"
```

Fetching is read-only; do not merge until the review scope is understood.
