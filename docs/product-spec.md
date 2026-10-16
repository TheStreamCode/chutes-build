# Product Spec

The contract Chutes Build must satisfy, written so every clause is checkable.
It exists because inherited upstream behavior keeps resurfacing as product
bugs (a login screen in an API-key-only product, a `grok` terminal title, a
SuperGrok upsell, docs describing flows that cannot run). When this spec and
retained upstream code disagree, this spec wins: gate the code, never the
spec.

Normative language: MUST (required), MUST NOT (forbidden), SHOULD (default
with a documented reason to diverge).

## 1. Identity

1.1. The product is Chutes Build, for the Chutes ecosystem only. It MUST NOT
present itself as Grok, xAI, or any SpaceX property, and MUST NOT upsell
their subscriptions.

1.2. User-facing strings (TUI, CLI help/errors, docs, splash, window title)
MUST NOT contain `grok`, `xai`, `x.ai`, `SuperGrok`, or `grok.com`, except
where §1.4 allows.

1.3. The binary is `chutes-build`; `--version` reports `chutes-build x.y.z`;
the TUI terminal title is `chutes-build` (or `<title> - chutes-build`,
80 columns max).

1.4. Retained internal names are allowed and MUST stay internal: `xai-*`
crate/module identifiers, `~/.chutes-build/` paths, the
`CHUTES_BUILD_API_KEY` legacy fallback, and `grok-*` prefixes kept only to
recognize and prune legacy downloads. A retained name that reaches a user
surface is a bug.

## 2. Authentication

2.1. The Chutes API key is the ONLY credential. Sources, highest to lowest:
per-model `api_key`/`env_key` in `config.toml`, then `CHUTES_API_KEY`
(`CHUTES_BUILD_API_KEY` accepted only when `CHUTES_API_KEY` is unset).

2.2. There MUST NOT be any account login: no sign-in screen, no browser or
device-code flow, no OIDC/SSO, no session token (inline env or `auth.json`),
no auth-provider command honored, no login store read or written.

2.3. Starting without a key MUST show a setup screen naming `CHUTES_API_KEY`
with no login option — never a `Login with …` entry that can only fail.

2.4. `chutes-build login` and `chutes-build logout` MUST print the API-key
setup guidance and exit non-zero. `/login` and `/logout` MUST stay
unavailable; `/apikey` MUST print setup instructions only.

2.5. Method advertisement MUST be API-key-only: with a key, at most
`chutes.api_key`; without one, nothing. Cached tokens, pins, OIDC, and
provider commands MUST be ignored, not merely deprioritized.

2.6. The agent MUST refuse session-based authentication even when called
explicitly, and sampling/headless/ACP auth errors MUST name the API key,
never a login command.

2.7. Enforcement map (all gated on `Distribution` withholding
`Capability::AccountLogin`):
`crates/codegen/xai-grok-config/src/distribution.rs` (flag + refusal text),
`xai-grok-shell/src/agent/auth_method.rs` (`build_auth_methods`),
`xai-grok-shell/src/agent/mvp_agent/acp_agent.rs` (`authenticate`),
`xai-grok-login/src/storage.rs` (no login store),
`xai-grok-login/src/flow.rs` (login flow refusal),
`xai-grok-pager/src/views/welcome/mod.rs` (blocked screen),
`xai-grok-pager-bin/src/main.rs` (`login`/`logout`),
`xai-grok-pager/src/app/dispatch/auth.rs` + `slash/commands/login.rs|logout.rs`,
`xai-grok-pager/src/app/effects/mod.rs` (no login upsell in session info).

## 3. Privacy and network

3.1. No telemetry, no remote crash/error reporting, no remote control of the
machine, no voice dictation to third-party STT, no phone-home updates, no
upstream session sharing. Withheld via `Distribution`, not via defaults a
config could re-enable.

3.2. Network requests MUST target the Chutes ecosystem (API, inference,
model catalog) or an explicitly user-configured endpoint. Ambient Chutes
credentials MUST NOT be sent to custom or untrusted endpoints, and
credential/media fetches MUST refuse loopback/private/link-local targets
(checked at connection time).

3.3. Sessions, memory, config, logs, traces, exports, and generated media
stay local unless a documented feature explicitly needs a provider request.

## 4. Ecosystem capabilities

4.1. Chutes-native capabilities (chat, image, video, music, TTS, STT,
OCR/PDF extraction, embeddings, image edit, usage) MUST be agent-callable
tools, so the agent uses them without user plumbing.

4.2. A text-only default model MUST delegate image reads to a multimodal
model automatically; an already-multimodal default MUST NOT pay the extra
hop.

4.3. Every capability MUST fail over to an equivalent chute when its
primary is unavailable or failing, and MUST return an honest error when no
fallback exists at all — never a silent wrong result.

4.4. Account data (plan, quotas, usage windows, PAYG credit) MUST be
derivable from the API key alone and visible via `chutes-build account`
(human and `--json`) and `/account`. Failures (no key, network error)
MUST report honestly, never as fake zeros.

4.5. Inference speed MUST be reported as output tokens per API second
(`tok/s`): after each turn next to the elapsed time, and as a Speed row
in the session usage totals. Missing usage or timing MUST render no
speed, never `0`.

## 5. Docs and naming truth

5.1. Public docs, embedded `/docs` guides, `--help`, and privacy/security
documents MUST describe shipped behavior only. A doc describing a flow the
build refuses (login, OIDC, providers) is a release-blocking bug.

5.2. The root README, the user guide, and `docs/getting-started.md` MUST
agree on authentication (API key only).

5.3. `CHANGELOG.md` MUST gain a non-empty section per release; release notes
are sliced from it and an absent section fails the release workflow.

## 6. Change protocol

Any behavior change MUST, in one commit:

1. cite or add the spec clause it implements;
2. implement it at the enforcement point from §2.7 (or add the point to the
   map for a new surface — a new surface that offers login without
   consulting `Distribution` is how regressions ship);
3. add a pin test asserting the user-visible outcome (text shown, method
   advertised, exit status), not just the helper;
4. update every doc from §5 that the behavior touches;
5. run the release gate from `docs/releasing.md` (manifest alignment,
   `cargo check --locked`, launcher tests, release-binary smokes including
   a real TUI start and a `--help` grep for §1.2 terms).

## 7. Known dormant code (do not revive without a spec change)

- Upstream login machinery (OAuth/OIDC/device flows, refresh/recovery,
  session store): unreachable while §2.2 holds. Its `xai-grok-login`
  machinery tests fail for lack of a store (see CHANGELOG 1.5.1); the
  injected-distribution storage tests pin both sides.
- Upstream subscription/tier upsell surfaces (`SuperGrok`, free-tier
  gating): unreachable while tiers come from Chutes; audit before any tier
  work.
- `auth_provider_command`, `CHUTES_BUILD_OAUTH2_CLIENT_ID`, enterprise OIDC
  config keys: parsed but never honored while §2.2 holds.

## 8. Spec → test map

| Clause | Pin test |
|---|---|
| 1.3 title | `xai-grok-pager` `terminal_title_strips_control_characters` |
| 2.3 blocked screen | `xai-grok-pager` `pending_without_account_login_shows_api_key_setup_instead_of_login` |
| 2.4 CLI refusal | release smoke: `login`/`logout` print guidance, exit 1 |
| 2.5 advertisement | `xai-grok-shell` `withheld_login_with_key_advertises_only_api_key`, `withheld_login_without_key_advertises_nothing` |
| 2.2 no session | `xai-grok-shell` `legacy_token_in_env_ignored_without_account_login`, `resolve_session_env_ignored_without_account_login` |
| 2.7 store | `xai-grok-login` `storage_distribution_tests::*` |
| 2.4/2.6 no upsell | `xai-grok-pager` `format_session_info_api_key_auth_suggests_no_login` |
| 4.3 fallback | `chutes-build-core` `equivalent_model_skips_excluded_aliases_and_pools`, `equivalent_model_returns_none_when_only_excluded_remains` |
| 4.2 delegation | UNPINNED — locate the vision-delegation tests and name them here |
| 2.4/2.6 re-auth text | `xai-grok-pager` `reauth_required_message_points_at_api_key` |
| 4.4 account CLI | `xai-grok-pager` `account_cmd::tests::*` |
| 4.4 account TUI | `xai-grok-pager` `show_account_schedules_direct_fetch`, `account_complete_renders_dashboard_text`, `account_failed_renders_honest_error`, `account_dispatches_show_account` |
| 4.5 inference speed | `xai-grok-pager` `turn_completed_message`, `status_blocks` `session_usage_block_*` + snapshots |

When a clause has no passing row here, the clause is aspirational: add the
test or weaken the clause.
