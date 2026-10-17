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
where §§1.4–1.5 allow.

1.3. The binary is `chutes-build`; `--version` reports `chutes-build x.y.z`;
the TUI terminal title is `chutes-build` (or `<title> - chutes-build`,
80 columns max).

1.4. Retained internal names are allowed and MUST stay internal: `xai-*`
crate/module identifiers, `~/.chutes-build/` paths, the
`CHUTES_BUILD_API_KEY` legacy fallback, and `grok-*` prefixes kept only to
recognize and prune legacy downloads. A retained name that reaches a user
surface is a bug.

1.5. Retained wire compatibilities are allowed and MUST stay invisible in
normal use: the local ACP `x.ai/*` method namespace, the `grok.com` ACP
auth-method id, the `x-grok-client-*` request headers (attached only on
xAI hosts for BYOK users; Chutes traffic carries none), `refs/grok/*`
worktree pins in user repos (renaming orphans live worktrees), the
`CHUTES_BUILD_XAI_API_BASE_URL` env fallback, matching (never emitting)
server-sent `grok.com/supergrok` upsell copy in order to rewrite it, and
displaying server-sent tier names verbatim, the ACP/voice client-type
identifiers (`grok-pager`, `grok-shell`, the `grok-shell/<version>` voice
user-agent, the unified-log `src` tag) matched on both ends of the local
protocols, and the telemetry schema's external-client allowlist entries
(`grok-agent-sdk`) for third-party SDK interop. The external OTLP
taxonomy is `chutes_build.*` under scope `ai.chutes.build`; the
internal-only `SuperGrokUpsell*` events have no external mapping and
MUST NOT gain one.

1.6. Legal, upsell, and key links MUST be live Chutes pages:
`https://chutes.ai/terms`, `https://chutes.ai/privacy`,
`https://chutes.ai/?_s=usage` (plan/credits/rate-limit tiers), and
`https://chutes.ai/app/settings/api-keys` (key management).
Managed-configuration copy points at `https://chutes.ai`. A link to a
`grok.com`, `x.ai`, `console.chutes.ai`, or any other 404 page is a
release-blocking bug.

## 2. Authentication

2.1. The Chutes API key is the ONLY credential. Sources, highest to lowest:
per-model `api_key`/`env_key` in `config.toml`, then `CHUTES_API_KEY`
(`CHUTES_BUILD_API_KEY` accepted only when `CHUTES_API_KEY` is unset).

2.2. There MUST NOT be any account login: no sign-in screen, no browser or
device-code flow, no OIDC/SSO, no session token (inline env or `auth.json`),
no auth-provider command honored, no login store read or written.

2.3. Starting without a key MUST show a setup screen naming `CHUTES_API_KEY`
with no login option — never a `Login with …` entry that can only fail.
The screen MUST accept the key inline (paste and submit validates it
against the API, persists it, and resumes the session); Esc cancels.
The product MUST never dead-block on a missing key.

2.4. `chutes-build login` MUST read an API key (interactive prompt or
piped stdin), validate it against the API, and persist it to
`~/.chutes-build/api_key`, exiting zero on success. `chutes-build logout`
MUST remove the saved key. OAuth/device flags MUST be refused honestly.
`/login` and `/logout` MUST stay unavailable; `/apikey` MUST print setup
instructions naming `chutes-build login`, the welcome-screen entry, and
the env var (mid-session key rotation is out of scope: a new key needs
a restart). Inline entry lives on the welcome screen only
(`AuthState::Pending` → `AuthMode::ApiKeyEntry`).

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
`xai-grok-pager/src/app/effects/mod.rs` (no login upsell in session info),
`chutes-build-core/src/api_key_store.rs` (save/load/remove, file injected
as env at startup; env always wins),
`xai-grok-pager/src/app/actions.rs|dispatch/auth.rs|effects` (`AuthMode::ApiKeyEntry`,
`Action::EnterApiKey/SubmitApiKey/CancelApiKeyEntry`,
`Effect::ValidateApiKey`, `TaskResult::ApiKeyAccepted/Rejected`,
`Effect::AuthenticateWithApiKey` pushing the key into the agent over
`chutes.ai/setApiKey` + `authenticate`, memory-only agent-side when the
login store is withheld).

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

4.6. Media generation MUST route through the Chutes-native `generate_media`
tool (live-catalog resolution with equivalent-model fallback per §4.3).
`/imagine` and `/imagine-video` MUST gate on `generate_media` and steer
the model to it — never to the legacy `image_gen`/`image_edit`/
`image_to_video`/`reference_to_video` tools, which target the xAI
Imagine API that Chutes does not serve. The legacy tools MUST default
off (re-enable only via `CHUTES_BUILD_IMAGE_GEN=1`,
`CHUTES_BUILD_IMAGE_EDIT=1`, `CHUTES_BUILD_VIDEO_GEN=1`, `[features]`,
or remote flags, for xAI-compatible hosts). Media batch caps are per
tool name: legacy image tools share the image budget, and
`generate_media` takes the image budget (8) by name for all its calls.

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
- Legacy Imagine clients (`image_gen`/`image_edit`/`video_gen`): default
  off per §4.6; they 404 on Chutes. Re-enable only against an
  xAI-compatible host.
- `chat_modes`/`ChatModelsClient` (grok.com `/rest/modes`): hard-off
  (`process_chat_mode_enabled() == false`); the grok.com default URL is
  unreachable. Removal is a follow-up, not a revival.
- `SuperGrokUpsell*` telemetry events: internal firehose only, no
  external OTLP mapping; the internal pipeline itself is inert unless a
  build-time URL is baked (§1.5).
- `FIRST_PARTY_FAMILY == "xai"` subagent-model classification: inert on
  Chutes (fails open to Selectable); do not retune without knowing the
  server's `modelFamily` values.
- The winget `xAI.ChutesBuild` installer id: dormant packaging metadata,
  unreachable from the product; rename only with the installer manifest.
- The `grok-agent-sdk` telemetry allowlist entry: reserved for
  third-party SDK interop; no first-party SDK exists.
- Legacy agent-type spellings (`grok-build*`, `grok_build_concise`) in
  match arms and the registry alias map: accepted as input for
  back-compat, never emitted (§1.5).

## 8. Spec → test map

| Clause | Pin test |
|---|---|
| 1.3 title | `xai-grok-pager` `terminal_title_strips_control_characters` |
| 2.3 blocked screen | `xai-grok-pager` `pending_without_account_login_shows_api_key_setup_instead_of_login` |
| 2.4 CLI key entry | release smoke: piped `login` validates+saves, `logout` removes; `chutes-build-core` `api_key_store::*` |
| 2.5 advertisement | `xai-grok-shell` `withheld_login_with_key_advertises_only_api_key`, `withheld_login_without_key_advertises_nothing` |
| 2.2 no session | `xai-grok-shell` `legacy_token_in_env_ignored_without_account_login`, `resolve_session_env_ignored_without_account_login` |
| 2.7 store | `xai-grok-login` `storage_distribution_tests::*` |
| 2.4/2.6 no upsell | `xai-grok-pager` `format_session_info_api_key_auth_suggests_no_login` |
| 4.3 fallback | `chutes-build-core` `equivalent_model_skips_excluded_aliases_and_pools`, `equivalent_model_returns_none_when_only_excluded_remains` |
| 4.2 delegation | `xai-grok-shell` `prompt_build::needs_image_transcription_tests::{confirmed_image_input_stays_inline, text_only_model_transcribes, unknown_capability_transcribes}` |
| 2.4/2.6 re-auth text | `xai-grok-pager` `reauth_required_message_points_at_api_key` |
| 4.4 account CLI | `xai-grok-pager` `account_cmd::tests::*` |
| 4.4 account TUI | `xai-grok-pager` `show_account_schedules_direct_fetch`, `account_complete_renders_dashboard_text`, `account_failed_renders_honest_error`, `account_dispatches_show_account` |
| 4.5 inference speed | `xai-grok-pager` `turn_completed_message`, `status_blocks` `session_usage_block_*` + snapshots |
| 1.5 OTLP taxonomy | `xai-grok-telemetry` `external::tests::*` + `external_otlp*` e2e (scope `ai.chutes.build`, `chutes_build.*` events) |
| 1.6 legal links | `xai-grok-pager` privacy-banner/banner-URL tests pin `chutes.ai/terms` + `chutes.ai/privacy`; billing upsell pins `_s=usage` |
| 2.3/2.4 key entry | `xai-grok-pager` `api_key_entry::*`, `api_key_accepted::*`, `api_key_rejected::*`, `welcome_entry::*`, `welcome_pending::*`, `dispatch::tests::auth::*`, `api_key_entry_auth_complete_finishes_and_resets_entry_flags`, `api_key_entry_auth_failed_rolls_back_to_pending`; `xai-grok-shell` `set_api_key_is_memory_only_without_account_login` |
| 4.6 media routing | `xai-grok-tools-api` `imagine_instruction_carries_prompt_verbatim`, `imagine_video_instruction_carries_prompt_and_workflow`; `xai-grok-pager` `requires_generate_media_tool` ×2; `xai-grok-shell` `resolve_image_gen_gates`, `resolve_video_gen_gates`, `imagine_tools_disabled_gates_image_edit`; `xai-grok-tools` `generate_media_takes_image_budget_by_name` |
| 1.3 CLI name | `xai-grok-pager` `cli_command_name_is_chutes_build`, `cli_help_output_header`, `zsh_completions_drop_prompt_slot_and_dispatch_on_line_1` |
| 1.5 agent profiles | `xai-grok-pager` `effects::tests::{all_flags_meta, plan_only_meta, plan_with_ask_user_uses_plan_profile, plan_with_subagents_meta, runtime_default_flags_produce_plan_meta, ask_user_alone_meta, agent_profile_names_are_valid_builtins, agent_profile_definitions_have_correct_names}`; `xai-grok-agent` `test_builtin_agent_name_strum_round_trip` |
| 1.5 leader mismatch | `xai-grok-pager` `acp::version_mismatch*` + `headless::ext_protocol` version-mismatch arm (`chutes.ai/leader/version_mismatch`) |
| 1.6 rate-limit/usage URLs | `xai-grok-shell` `sampling::error::tests::{rate_limited_fallback_oauth_vs_api_key, format_rate_limited_*}`; `xai-grok-pager` `free_usage_translate_local_submit_maps_options`, `upsell_non_max_upgrade_url_is_usage` |
| Settings footer | `xai-grok-pager` `settings_e2e::docs_footer_renders_for_browse_and_picker` (MEDIUM variant at 120 cols) |

When a clause has no passing row here, the clause is aspirational: add the
test or weaken the clause.
