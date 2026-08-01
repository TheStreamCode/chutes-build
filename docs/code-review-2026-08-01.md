# Code and security audit — 2026-08-01

## Executive summary

The repository is suitable for continued public development after the changes
listed below. Its strongest properties are explicit crate boundaries, a
privacy-first network policy, extensive unit and integration coverage,
reproducible Rust/npm packaging, and GitHub workflows with least-privilege
permissions and immutable action references.

No unresolved critical or high-severity security vulnerability was found.
Five low-severity transitive alerts have compatible lockfile updates in this
review. Two low-severity `lru` alerts remain because the only published fix is
outside the version range accepted by `ratatui 0.29` and `aws-sdk-s3 1.112`.
They require coordinated dependency migrations and are not silently suppressed.

## Scope and method

The review covered the Rust workspace, Node.js launcher and native package
manifests, local configuration and credential boundaries, tests, documentation,
release assembly, GitHub Actions, repository rules, security settings, and the
selective upstream-sync policy. It combined source review, dependency-tree and
GitHub alert inspection, release-package inspection, formatting/lint/test/build
gates, secret scanning, and comparison with upstream Grok Build through
`0.2.117`.

Generated or upstream-derived code was reviewed as part of the executable trust
boundary, but broad refactors were avoided because they would increase sync
risk without changing product behavior. Existing design files, icons, and
assets were not modified.

## Findings and disposition

### CR-001 — High — resolved — tool-result history integrity

The action-stationarity reminder could previously be injected after an
assistant tool call was committed but before its result was committed. The
interjection path then repaired the open call as cancelled, after which the real
result arrived, creating duplicate results and an invalid conversation history.

The reminder now fires at the start of the following inference iteration, after
the preceding result is committed, and a per-run latch makes the reminder
one-shot (`crates/codegen/xai-grok-shell/src/session/acp_session_impl/turn.rs:1852`
and `:1886`). Unit coverage verifies the latch and integration coverage verifies
the complete history invariant
(`crates/codegen/xai-grok-shell/src/session/acp_session_tests/turn/chat_history_integrity_tests.rs:34`).

### CR-002 — Low — resolved — vulnerable transitive patch versions

GitHub Dependabot identified vulnerable versions of `rand 0.8.5`, `rand 0.10.0`,
`aws-sdk-sso 1.86.0`, `aws-sdk-ssooidc 1.89.0`, and `aws-sdk-sts 1.88.0` in the
root `Cargo.lock`. They were updated to the first patched compatible releases:
`rand 0.8.6`, `rand 0.10.1`, `aws-sdk-sso 1.89.0`,
`aws-sdk-ssooidc 1.91.0`, and `aws-sdk-sts 1.91.0`.

These are lockfile-only patch updates within existing manifest constraints. The
resolved graph remains validated with `--locked`.

### CR-003 — Low — accepted temporarily — `lru` soundness advisory

`lru 0.12.5` is reachable through both `ratatui 0.29` and
`aws-sdk-s3 1.112`; the independent Markdown fuzz lock reaches it through
`ratatui 0.29`. GHSA-rhfx-m35p-ff5j affects `lru >=0.9,<0.16.3` and describes
an `IterMut` Stacked Borrows violation. The available fixed release is
`0.16.3`, outside both transitive semver ranges.

No direct Chutes Build call to the affected iterator was identified. The alert
is low severity, remains visible in GitHub, and is not added to an ignore list.
A proper fix requires testing the coordinated `ratatui 0.30` and current AWS
SDK migrations; applying either as an unreviewed lockfile substitution would be
unsafe and potentially breaking.

### CR-004 — Medium — resolved — ambiguous plan approval

An empty Enter in the plan-revision input could approve a plan accidentally.
Approval now requires the explicit `a` shortcut and the UI states the available
decision. Tests cover empty input, explicit approval, revision, and cancellation.

### CR-005 — Medium — resolved — CI and supply-chain hardening

The Windows workflow no longer disables host malware protection. GitHub Actions
are pinned to full commit SHAs, tokens default to read-only permissions, the
security job scans full Git history and dependency policy, branch rules require
all five current checks, and Dependabot security updates remain enabled without
enabling noisy version-update PRs.

### CR-006 — Low — resolved — npm package completeness

The npm launcher and six native optional packages are version-aligned at
`0.4.2`. The public archive now includes the changelog and security policy, has
no lifecycle scripts, and can be checked locally with `npm run check`. Release
automation verifies archive checksums, native execution, and launcher assembly
before any manual publication step.

### CR-007 — Low — resolved — environment and build-artifact hygiene

Git ignores credentials, `.env` variants, state, traces, logs, generated media,
Rust output, npm archives, and release `dist`/`staging` directories. The runtime
reads the process environment and does not imply automatic dotenv loading;
configuration guidance now makes that boundary explicit.

### CR-008 — Low — resolved — Windows hook-test contention

The first post-review Windows CI run passed 7,245 pager tests but failed two
notification-hook tests when several tests launched PowerShell concurrently and
two output files were not created within five seconds. Linux, macOS, and a full
local Windows run passed, isolating the issue to test-process contention.

All notification-hook tests now share a `serial_test` group
(`crates/codegen/xai-grok-pager/src/notifications/hooks.rs:187`). This changes
test scheduling only; runtime hook concurrency and timeout behavior are
unchanged.

## Architecture, performance, and maintainability assessment

The product-specific shell, pager, tools, authentication, configuration, and
packaging layers have clear ownership boundaries. The retained upstream crate
layout is large but deliberate: preserving it keeps selective Grok Build ports
reviewable. Chutes-specific product invariants are centralized in policy and
configuration paths rather than scattered release-time patches.

The largest practical performance cost is cold compilation of the generated
Rust workspace, not an identified runtime hot path. CI caches Cargo data per
lockfile and uses all four hosted-runner CPUs. Local full-gate builds can exceed
60 GiB, so contributor instructions require measuring and cleaning `target/`
after verification. No asset optimization was justified by this audit.

No high-confidence duplicate application logic was safe to remove without
crossing upstream ownership boundaries. Existing shared modules already cover
notable relay, feedback, goal, and configuration behavior. Automated unused
dependency removal was intentionally deferred: `cargo machete`/`cargo udeps`
is not part of the pinned toolchain, and blind removal in this feature-heavy,
target-specific workspace risks false positives.

## Recommended follow-up

1. Test `ratatui 0.30` as an isolated migration, then regenerate both the root
   and Markdown fuzz lockfiles and run the full cross-platform pager suite.
2. Test a current AWS SDK set as a separate migration and verify S3 upload,
   OAuth/SSO-disabled builds, MSRV, binary size, and cold compile time.
3. Add an explicitly pinned unused-dependency tool only after establishing a
   checked-in allowlist for target-, feature-, build-, and fuzz-only crates.
4. Run publishing-disabled release assembly for `0.4.2`; publish npm packages
   and create a GitHub release only with explicit maintainer authorization.

## Verification record

The review gate includes formatting, Chutes-owned Clippy checks, core/CLI/pager/
settings/auth/tool tests, locked dependency resolution, npm tests and package
inspection, relative Markdown-link validation, Gitleaks, Cargo Deny, CodeQL, and
Linux/macOS/Windows GitHub Actions. Exact run links and final conclusions belong
in the release handoff rather than this durable review record.
