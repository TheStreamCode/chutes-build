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

1.4. The workspace carries Chutes names. Crates, modules, Rust identifiers,
protobuf packages, request headers, environment variables, state paths and
the telemetry taxonomy all read `chutes*`; a `grok`, `xai` or `x.ai` token in
a live code path is a bug. `~/.chutes-build/` paths and the
`CHUTES_BUILD_API_KEY` fallback are already Chutes names and stay.

1.5. Retained upstream spellings are a compatibility surface, not an
identity, and MUST stay invisible in normal use. Three classes are allowed:

- **History and attribution**: past `CHANGELOG` entries, bundled crate
  changelogs, the upstream repository link in `NOTICE` /
  `THIRD-PARTY-NOTICES`, and the README's fork attribution.
- **Local data written by releases up to 1.7.2**, read-compat only: the
  `[grok_com_config]` table (aliased to `[login_config]`), `grok_ws_*` keys,
  persisted session/manifest fields, legacy agent-type spellings
  (`grok-build*`, `grok_build_concise`), legacy client-type strings
  (`grok-shell`, `grok-tui`, `grok-web`, `grok-desktop`, `grok-pager`),
  `refs/grok/*` worktree pins (read; new pins are `refs/chutes/*`), legacy
  `grok-*` download names pruned from `~/.chutes-build/`, the `XAI_*`
  environment names older orchestration scripts export (including
  `CHUTES_BUILD_XAI_API_BASE_URL` and the `xai_api_base_url` requirements
  key, both read-compat fallbacks behind the canonical endpoint names).
- **Inbound values, matched and never emitted**: server-sent upsell copy
  (`grok.com/supergrok`, `chutes.ai/supergrok`), the legacy `grok.com` ACP
  auth-method id, the `grok-agent-sdk` telemetry allowlist entry, the
  `modelFamily` values the catalog returns, and the legacy signed
  managed-configuration document types and nonce header
  (`grok.managed_policy.v1`, `grok.managed_identity.v1`,
  `x-grok-managed-config-nonce`) accepted from older deployments.

Anything outside those classes is a release-blocking bug. The external OTLP
taxonomy is `chutes_build.*` under scope `ai.chutes.build`; the internal-only
plan and credit-limit upsell events (`PlanUpsell*`, `CreditLimitUpsell*`) have no
external mapping and MUST NOT gain one.

The checked-in Kotlin and Swift bot-relay clients under
`crates/common/chutes-tool-protocol/generated/` are upstream-generated artifacts,
not shipped in the binary and not part of the product's identity; regenerate them
with the external protocol codegen before using them against the Chutes wire.

1.6. Legal, upsell, and key links MUST be live Chutes pages:
`https://chutes.ai/terms`, `https://chutes.ai/privacy`,
`https://chutes.ai/?_s=usage` (plan/credits/rate-limit tiers), and
`https://chutes.ai/app/settings/api-keys` (key management).
Managed-configuration copy points at `https://chutes.ai`. A link to a
`grok.com`, `x.ai`, `console.chutes.ai`, or any other 404 page is a
release-blocking bug.

1.7. 2.0.0 renamed the local wire — the ACP method namespace, request
headers and protobuf packages all moved to Chutes names, and the workspace
crates were renamed with them. Wire compatibility with peers built from
1.7.2 or earlier is **not** retained; the local-data compatibility in §1.5
is. The legacy endpoint configuration stays honored as read-compat:
`CHUTES_BUILD_XAI_API_BASE_URL` and the `xai_api_base_url` requirements key
still work when the canonical names (`base_url`, `CHUTES_API_BASE_URL`,
`chutes_api_base_url`) are unset.

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
`crates/codegen/chutes-config/src/distribution.rs` (flag + refusal text),
`chutes-shell/src/agent/auth_method.rs` (`build_auth_methods`),
`chutes-shell/src/agent/mvp_agent/acp_agent.rs` (`authenticate`),
`chutes-login/src/storage.rs` (no login store),
`chutes-login/src/flow.rs` (login flow refusal),
`chutes-pager/src/views/welcome/mod.rs` (blocked screen),
`chutes-build/src/main.rs` (`login`/`logout`),
`chutes-pager/src/app/dispatch/auth.rs` + `slash/commands/login.rs|logout.rs`,
`chutes-pager/src/app/effects/mod.rs` (no login upsell in session info),
`chutes-build-core/src/api_key_store.rs` (save/load/remove, file injected
as env at startup; env always wins),
`chutes-pager/src/app/actions.rs|dispatch/auth.rs|effects` (`AuthMode::ApiKeyEntry`,
`Action::EnterApiKey/SubmitApiKey/CancelApiKeyEntry`,
`Effect::ValidateApiKey`, `TaskResult::ApiKeyAccepted/Rejected`,
`Effect::AuthenticateWithApiKey` pushing the key into the agent over
`chutes.ai/setApiKey` + `authenticate`, memory-only agent-side when the
login store is withheld),
`chutes-shell/src/session/acp_session_impl/tool_dispatch.rs` (dispatch
publishes the resolved credentials on the call context) +
`chutes-tools/src/implementations/chutes/mod.rs`
(`resolve_credentials`, context first, env fallback).

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

4.0. Web search MUST work without a Responses-API endpoint: the Chutes
router serves Chat Completions only, so the `web_search` tool and the
`deep-research` workflow MUST use the direct public search provider
(`https://html.duckduckgo.com/html/`) in that configuration. The search
request MUST NOT carry a Chutes credential and MUST NOT contact the model.
A custom endpoint that implements the Responses API MAY keep the
model-hosted search path.

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

5.4. Feedback and bug reports MUST be collected on the public GitHub
repository (`https://github.com/TheStreamCode/chutes-build/issues`). The
product MUST NOT expose an in-app feedback uploader: no reachable command,
palette entry or popup may upload a report. Retained-but-unreachable client code
is covered by §7. `/feedback` opens the repository's issue-form chooser (given
text is copied to the clipboard first) and MUST NOT transmit report content
itself.

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
  session store): unreachable while §2.2 holds. Its `chutes-login`
  machinery tests fail for lack of a store (see CHANGELOG 1.5.1); the
  injected-distribution storage tests pin both sides.
- Upstream subscription/tier upsell surfaces: unreachable while tiers come
  from Chutes. Server-sent upsell copy is matched only to rewrite it (§1.5);
  the product's own plan vocabulary is the Chutes one, and a max-tier check
  that cannot recognise a tier resolves false (fail-closed upsell gating).
- `auth_provider_command`, `CHUTES_BUILD_OAUTH2_CLIENT_ID`, enterprise OIDC
  config keys: parsed but never honored while §2.2 holds.
- Legacy Imagine clients (`image_gen`/`image_edit`/`video_gen`): default
  off per §4.6; they 404 on Chutes. Re-enable only against an
  xAI-compatible host.
- Plan- and credit-limit-upsell telemetry (`PlanUpsell*`,
  `CreditLimitUpsell*`): internal firehose only, no external OTLP mapping;
  the internal pipeline itself is inert unless a build-time URL is baked (§1.5).
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
- In-app feedback collection (the feedback modal, its local drafts, the
  trace-consent card and the cli-chat-proxy feedback client): unreachable
  in this build. Product feedback goes to the public GitHub repository and
  `/feedback` only opens its issue form (§5.4).

### Removed in 2.0.0

These went away with the rebrand; do not revive them without a spec change:

- `chat_modes` / `ChatModelsClient` and the remote chat-mode URL: the chat
  mode was hard-off and its remote listing had no Chutes backend.
- The preview-proxy supervisor (`preview_supervisor`, preview-activity
  scraping in `status_config`/`activity`) in the workspace daemon: remote
  workspace exposure is withheld (§3.1), so there is no preview proxy to
  supervise.
- The "official marketplace" concept: marketplace sources are configured by
  the user; there is no first-party marketplace URL to auto-trust.
- The xAI tier vocabulary (`Basic`, `Premium`, `Heavy`, `Lite`, `SuperGrok`)
  and the `X-XAI-Token-Auth` wire header.

## 8. Spec → test map

| Clause | Pin test |
|---|---|
| 1.3 title | `chutes-pager` `terminal_title_strips_control_characters` |
| 2.3 blocked screen | `chutes-pager` `pending_without_account_login_shows_api_key_setup_instead_of_login` |
| 2.4 CLI key entry | release smoke: piped `login` validates+saves, `logout` removes; `chutes-build-core` `api_key_store::*` |
| 2.5 advertisement | `chutes-shell` `withheld_login_with_key_advertises_only_api_key`, `withheld_login_without_key_advertises_nothing` |
| 2.2 no session | `chutes-shell` `legacy_token_in_env_ignored_without_account_login`, `resolve_session_env_ignored_without_account_login` |
| 2.7 store | `chutes-login` `storage_distribution_tests::*` |
| 2.4/2.6 no upsell | `chutes-pager` `format_session_info_api_key_auth_suggests_no_login` |
| 4.3 fallback | `chutes-build-core` `equivalent_model_skips_excluded_aliases_and_pools`, `equivalent_model_returns_none_when_only_excluded_remains` |
| 4.2 delegation | `chutes-shell` `prompt_build::needs_image_transcription_tests::{confirmed_image_input_stays_inline, text_only_model_transcribes, unknown_capability_transcribes}` |
| 2.4/2.6 re-auth text | `chutes-pager` `reauth_required_message_points_at_api_key` |
| 4.4 account CLI | `chutes-pager` `account_cmd::tests::*` |
| 4.4 account TUI | `chutes-pager` `show_account_schedules_direct_fetch`, `account_complete_renders_dashboard_text`, `account_failed_renders_honest_error`, `account_dispatches_show_account` |
| 4.5 inference speed | `chutes-pager` `turn_completed_message`, `status_blocks` `session_usage_block_*` + snapshots |
| 1.5 OTLP taxonomy | `chutes-telemetry` `external::tests::*` + `external_otlp*` e2e (scope `ai.chutes.build`, `chutes_build.*` events) |
| 1.6 legal links | `chutes-pager` privacy-banner/banner-URL tests pin `chutes.ai/terms` + `chutes.ai/privacy`; billing upsell pins `_s=usage` |
| 2.3/2.4 key entry | `chutes-pager` `api_key_entry::*`, `api_key_accepted::*`, `api_key_rejected::*`, `welcome_entry::*`, `welcome_pending::*`, `dispatch::tests::auth::*`, `api_key_entry_auth_complete_finishes_and_resets_entry_flags`, `api_key_entry_auth_failed_rolls_back_to_pending`; `chutes-shell` `set_api_key_is_memory_only_without_account_login` |
| 4.6 media routing | `chutes-tools-api` `imagine_instruction_carries_prompt_verbatim`, `imagine_video_instruction_carries_prompt_and_workflow`; `chutes-pager` `requires_generate_media_tool` ×2; `chutes-shell` `resolve_image_gen_gates`, `resolve_video_gen_gates`, `imagine_tools_disabled_gates_image_edit`; `chutes-tools` `generate_media_takes_image_budget_by_name` |
| 1.3 CLI name | `chutes-pager` `cli_command_name_is_chutes_build`, `cli_help_output_header`, `zsh_completions_drop_prompt_slot_and_dispatch_on_line_1` |
| 1.5 agent profiles | `chutes-pager` `effects::tests::{all_flags_meta, plan_only_meta, plan_with_ask_user_uses_plan_profile, plan_with_subagents_meta, runtime_default_flags_produce_plan_meta, ask_user_alone_meta, agent_profile_names_are_valid_builtins, agent_profile_definitions_have_correct_names}`; `chutes-agent` `test_builtin_agent_name_strum_round_trip` |
| 1.5 local-data compat | `chutes-login` `legacy_grok_com_config_table_parses_into_login_config`, `login_config_table_wins_over_legacy_alias`, `requirements_force_login_team_reads_legacy_table_tiers`, `expand_auth_alias_folds_auth_over_legacy_table`; `chutes-shell` `legacy_grok_com_config_table_is_read_as_login_config_alias`; `chutes-workspace` permission client-type alias tests; `chutes-fast-worktree` `refs/grok/*` read test |
| 1.7 endpoint read-compat | `chutes-config` `legacy_xai_api_base_url_key_parses_and_is_not_written`; `chutes-shell` `public_api_base_url_env_reads_legacy_var_as_fallback`, `config::tests::apply_requirements_reads_legacy_xai_api_base_url_key` |
| 1.7 wire names | `chutes-tool-protocol` serde round-trips and `chutes-pager` ACP tests pin `chutes.ai/*` + `x-chutes-build-*`; `scripts/rebrand.py --check` fails on any upstream token outside the §1.5 allowlist |
| 1.5 leader mismatch | `chutes-pager` `acp::version_mismatch*` + `headless::ext_protocol` version-mismatch arm (`chutes.ai/leader/version_mismatch`) |
| 5.4 feedback channel | `chutes-pager` `feedback::tests::bare_and_whitespace_open_the_issue_chooser`, `inline_text_copies_the_report_and_opens_the_issue_form` |
| 1.6 rate-limit/usage URLs | `chutes-shell` `sampling::error::tests::{rate_limited_fallback_oauth_vs_api_key, format_rate_limited_*}`; `chutes-pager` `free_usage_translate_local_submit_maps_options`, `upsell_non_max_upgrade_url_is_usage` |
| Settings footer | `chutes-pager` `settings_e2e::docs_footer_renders_for_browse_and_picker` (MEDIUM variant at 120 cols) |

When a clause has no passing row here, the clause is aspirational: add the
test or weaken the clause.
