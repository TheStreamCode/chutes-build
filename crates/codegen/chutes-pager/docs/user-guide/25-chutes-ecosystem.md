# Chutes Models and Ecosystem

Chutes Build discovers models and capabilities from the live Chutes catalog.
`Auto (Chutes Router)` is inserted as the first model choice; its stable local
ID is `model-router`. Auto sends Chutes' native routing string — the saved-pool
alias `default`, a strategy suffix, or an inline `CHUTES_ROUTING_POOL` — to the
normal inference host. If the account has no dashboard pool, the fallback chain
steps down to a live inline pool built from the current catalogue
(`:latency` by default, so the fastest warm model answers). A concrete model is tried first,
followed by `CHUTES_FALLBACK_MODELS`, then Auto. Set `CHUTES_STRICT_MODEL=1` to
disable fallback entirely.

Fallback is limited to transient/model-unavailable failures before streaming
begins. Authentication and invalid-request errors do not silently change models.
Every fallback attempt recalculates reasoning controls for its own model family.

## Platform API

Two hosts cover the platform. `https://llm.chutes.ai/v1` is the
OpenAI-compatible inference plane: chat completions, streaming SSE, tool calls,
JSON/structured outputs, vision, and the public model catalog at
`GET /v1/models` (no auth). `https://api.chutes.ai` is everything else: account,
quota and usage, API keys, model aliases, media/chute APIs, and TEE evidence.
Both accept the same `Authorization: Bearer cpk_...` key; `X-API-Key` is ignored
on the inference plane. Treat `GET /v1/models` as the source of truth for model
ids and capabilities: it publishes `context_length`, `max_output_length`,
`supported_features`, `supported_sampling_parameters`, `input_modalities`,
`pricing` (with the cheaper `input_cache_read` rate), and
`confidential_compute` (filter on the boolean, not the `-TEE` name).

The `model` field accepts more than one concrete id. A comma-separated list is a
sequential failover pool; `:latency` picks the member with the fastest first
token and `:throughput` the highest tokens per second. Saved dashboard pools
expose `default`, `default:latency`, and `default:throughput` aliases, and the
aliases API manages them. A single concrete id bypasses routing. `Auto (Chutes
Router)` uses this native routing and falls back to a live `:latency` pool, so
it is the default choice for interactive work.

Live TTFT and throughput are not in `/v1/models`; they are published by
`GET https://api.chutes.ai/invocations/stats/llm`. Chat billing is per million
tokens, with cached prompt reads billed at the cheaper cache rate. Media chutes
run on their own hosts and bill per compute-second; their gateway has a public
timeout of about ten minutes, and a 504 response is an HTML error page rather
than media, so always check the status and content type.

`chutes-build routing` (and `/routing` in the TUI) prints the saved Chutes
routing pool, the three strategies (`default`, `default:latency`,
`default:throughput`), and the live per-model TTFT/TPS table. Pool members and
stats are resolved by `chute_id` against the live catalogue, so model names are
shown instead of private chute labels. When a saved `default` pool exists, Auto
targets `default:latency`, so the pool configured in the Chutes web app wins
over the client-side inline pool.

Writes are explicit and capped at 20 models:

```bash
chutes-build routing add zai-org/GLM-5.2-TEE
chutes-build routing remove Qwen/Qwen3.8-27B-TEE --alias default
chutes-build routing save model-a model-b --alias default --dry-run
```

The alias endpoint is eventually consistent and occasionally returns 502; the
command polls the list until the write is visible and restores the old pool if a
replace fails. `/routing` itself stays read-only.

Some catalog entries have `confidential_compute: false`: they run on an external
provider and require a positive PAYG balance, separately from the subscription
quota. When that balance is not positive Chutes returns
`402 A positive balance is required for external provider execution`. This is a
PAYG funding condition, not the weekly or monthly subscription limit; top up the
PAYG balance at https://chutes.ai/?_s=usage or use a TEE model. The picker marks
those entries with a `PAYG` notice.

TEE models run inside hardware enclaves and expose attestation evidence through
`GET https://api.chutes.ai/chutes/{chute_id}/evidence`; do not claim verified
attestation unless the quote was actually checked. Official docs and news remain
the authority for current behavior, quotas, and pricing.

## Reasoning modes

Use `/model` to choose a route. When the selected model has configurable
reasoning, the picker offers only modes supported by that exact generation. Use
`/effort` to change the active mode later.

- Hybrid Qwen, Kimi, DeepSeek, Gemma, and GLM generations expose `Instant` and
  `Thinking` when their published templates support a binary switch.
- GLM-5.2 exposes `Instant`, `Fast reasoning`, and `Maximum reasoning`.
- Thinking-only, fixed-reasoning, and non-reasoning models expose no selector.
- Auto exposes no selector because its target model can change per task.

Chutes-provided capability menus override the bundled registry. Unknown future
generations are not assigned controls based only on a family prefix.

Image inputs remain on a model that advertises vision support; otherwise they
are described by a vision-capable Chutes route. PDF pages returned by the file
tool follow the same rule. Video attachments are sampled locally with FFmpeg and
their representative frames follow the vision route.

The `ocr_page` tool extracts text verbatim from a single image or PDF page on
demand, independent of the active chat model's vision support. It always calls
a dedicated Chutes vision model directly and returns only the extracted text —
the image itself is never added to the conversation. Billed against the
account's subscription quota like any other official Chutes model call, never
the separate marketplace/wallet balance used by third-party chutes.

Use the native media tools for generation and editing. `generate_media` is
self-contained: it resolves the model name against the catalog, selects the
cord for the requested kind, and places `prompt` into the text field that cord
declares, so `{model, kind, prompt}` is a complete call. Because each public
chute exposes different cords and schemas, payloads are validated locally
first; a mismatch reports the fields that cord accepts, and
`describe_media_model` remains available when an exact schema is needed up
front.

## Capability defaults

Single-purpose tools ship with a pinned default chute, overridable per call.
`generate_media` instead resolves the model name against the live catalog, so
new image, video, music, and speech chutes work with no client update.

| Capability | Tool | Default |
|---|---|---|
| Image OCR (verbatim) | `ocr_page` | `google/gemma-4-31B-turbo-TEE` vision model |
| PDF / document extraction | `extract_document` | [vonkaiser-docuextract](https://chutes.ai/docs/models/vonkaiser-docuextract) (LightOnOCR + NuExtract3) |
| Speech-to-text | `transcribe_audio` | [vonkaiser-audiodojo](https://chutes.ai/docs/models/vonkaiser-audiodojo) |
| Embeddings | `embed_texts` | `Qwen/Qwen3-Embedding-8B-TEE` ([docs](https://chutes.ai/docs/models/chutes-qwen-qwen3-embedding-8b-tee)) |
| Image generation | `generate_media` | Live catalog: `/imagine <prompt>` auto-selects an available model, `/imagine <model> <prompt>` picks one |
| Image editing | `generate_media` | Live catalog, e.g. [vonkaiser-qwen-image-edit-2511](https://chutes.ai/docs/models/vonkaiser-qwen-image-edit-2511) |
| Video generation | `generate_media` | Live catalog: `/imagine-video <prompt>` auto-selects, `/imagine-video <model> <prompt>` picks one |
| Music generation | `generate_media` | Live catalog, e.g. [vonkaiser-ace-step-15-music-generator](https://chutes.ai/docs/models/vonkaiser-ace-step-15-music-generator) |

Media models are resolved against the live Chutes catalog on every call: a
requested model that disappeared falls back to an available generation model
of the same kind and the tool says which replacement answered. Model ids are
resolved to Chutes chute ids internally, because the account API's `describe`
lookup only accepts those.

Generated files are saved inside the workspace and returned as typed media
artifacts rather than path text. Supported terminals keep inline image/video
previews. Music and speech cards provide a local audio toggle backed by
`ffplay`; if it is unavailable, Chutes Build opens the file in the operating
system's default player. Media never autoplays.

Downloads stream through temporary files and default to a 128 MiB limit with a
512 MiB hard ceiling. Workspace inputs default to 64 MiB with the same hard
ceiling. Final artifact writes do not overwrite an existing destination.

Context7 tools provide current library documentation. They reject known secret
material and send only library identifiers and documentation queries. An
optional Context7 key is sent only to the official HTTPS service; a custom
endpoint requires an explicit development opt-in and never receives that key.

## Official Chutes sources

For any Chutes product, API, model, pricing, plan, quota, platform, or ecosystem
question, the agent consults both [Chutes Docs](https://chutes.ai/docs) and
[Chutes News](https://chutes.ai/news) before answering. Directly relevant
official pages take precedence over third-party summaries. If the official
sources do not cover the claim or cannot be reached, the response identifies
the gap and separates documented facts from inference.

## Plan and quota indicator

The status bar shows the account plan plus the rolling four-hour and monthly
percentages when both are available. Its severity follows the highest consumed
active window: green is below 80%, warning color begins at 80%, and the error
color is used at 100% or above. Click the indicator or run `/usage` to inspect
every active window and its reset time.

Subscription usage and quota metadata are loaded concurrently. When aggregate
quota usage is not available, Chutes Build queries the documented per-chute
quota endpoints concurrently and combines their used/limit values. The compact
indicator and detailed view preserve whether each window is daily, four-hour,
weekly, or monthly.
