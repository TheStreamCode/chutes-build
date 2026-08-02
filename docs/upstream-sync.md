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

## Review record: 2026-08-02

Upstream `main` was still `a4221165824e5b1f5c4c10b7459f65e78dd6448d` (`0.2.117`),
unchanged since the previous review, and upstream still publishes no releases or
tags. No new upstream work existed to review, so this pass reopened the areas the
2026-08-01 review deferred.

Ported:

- MCP config persistence. The four MCP writers parsed `config.toml` with a
  fallback to an empty table and then rewrote the whole file, so an unparseable
  config was replaced by only the keys being saved; none took the config write
  lock, and all staged through a fixed temp filename. Now one helper refuses
  unparseable input, holds the user-config lock across the read-modify-write,
  writes atomically, and skips no-op writes.
- Extra TLS roots via `CHUTES_EXTRA_CA_BUNDLE`, for proxy-terminated TLS.
  Reworked rather than copied: upstream adds a crate that depends on `rustls`
  directly, which would mean editing the generated workspace root, so this uses
  reqwest's own PEM bundle parsing inside `xai-grok-http`.
- Cancelling an in-flight `/compact`, including the `is_compact_running` /
  `cancel_compact_command` state predicates it needs.

Already present, nothing to port: background subagent cancellation (identical,
tests included), terminal resize (`ptyctl` differs only by a comment and an
upstream-only lint allow), and session cold-start (voice cold start, already
implemented).

Not portable: the `auth_retry` module behind the ACP task-lifecycle changes is
bound to upstream's `AuthManager`/`SentCredential` flow. Chutes Build
authenticates differently, so adopting it means reworking authentication, not
porting a module.

Deferred with a known reason: `streaming-messages-json` (NDJSON in the Messages
API wire format). Upstream implements it as a reducer split across 19 files;
this fork still has a monolithic `headless.rs` that branches on the output
format in twelve separate places. The port is worth doing but needs the reducer
refactor first, not a thirteenth branch.

The baseline in `.github/upstream.json` is unchanged because upstream itself has
not moved.

## Review record: 2026-08-01

Reviewed upstream `main` through `a4221165824e5b1f5c4c10b7459f65e78dd6448d`
(`0.2.117`). Upstream still publishes no GitHub releases or tags, so the
versioned changelogs and `main` commits were the authoritative review inputs.

Ported selectively:

- `0.2.115`: deliver the repeated-tool-call stationarity reminder only after
  the preceding tool results are committed, preventing duplicate tool results
  from corrupting chat history;
- `0.2.116`: accept `/undo` as an alias for `/rewind`; and
- `0.2.117`: require revision notes before Enter requests plan changes, with an
  explicit `a` shortcut for approval.

Already covered by Chutes-specific code: Windows external-auth shell handling
uses the platform-aware command builder and retains its dedicated tests.

Deferred because they are coupled to newer upstream runtime/configuration
surfaces and need a separate Chutes compatibility review: MCP enable/disable
persistence, streaming JSON events, additional CA-bundle handling, background
subagent cancellation, ACP task lifecycle changes, terminal resize behavior,
and session cold-start work. No upstream identity, billing, telemetry, or
enterprise-only behavior was imported.
