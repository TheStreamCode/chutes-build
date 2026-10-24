---
name: chutes-platform
description: Complete Chutes platform reference. Use for Chutes API, models, routing, media, OCR/PDF, embeddings, quotas, billing, TEE attestation, errors, and how each maps to this CLI.
---

# Chutes platform reference

Chutes is an OpenAI-compatible, TEE-backed inference platform. This skill is the
on-demand reference for how the platform works and how Chutes Build uses it.
For current values that change often (models, prices, quotas, limits), verify
against the live endpoints or the official documentation before answering.

## Hosts and authentication

Two hosts cover everything:

- `https://llm.chutes.ai/v1` is the OpenAI-compatible inference plane: chat
  completions, streaming, tool calls, JSON and structured outputs, vision, and
  the public model catalog at `GET /v1/models`.
- `https://api.chutes.ai` is the platform plane: account, quota and usage, API
  keys, model aliases, chute management, media APIs, and TEE evidence.

Authenticate with one `cpk_` API key as `Authorization: Bearer <key>` on both
planes. `X-API-Key` is ignored by the inference plane. `GET /v1/models` needs no
authentication, so a successful catalog read does not prove the key works; check
the key against an authenticated endpoint such as `GET /users/me` when that
matters. Never place the key, prompts, source code, or repository contents in a
search query, a third-party request, or a log.

## Model catalog

`GET https://llm.chutes.ai/v1/models` is the source of truth. Each entry carries
`id`, `context_length`, `max_output_length`, `pricing` (prompt, completion, and
the cheaper `input_cache_read`), `confidential_compute`, `supported_features`
(tools, json_mode, structured_outputs, reasoning), `supported_sampling_parameters`,
`input_modalities`, and `owned_by`. Read it instead of pinning model ids: names
change, and a model may be absent or renamed between releases.

Live latency and throughput are not in `/v1/models`. Use
`GET https://api.chutes.ai/invocations/stats/llm` when a task needs live
TTFT/TPS, or let routing choose. The CLI's `/model` picker and `Auto` already
use catalog capabilities for reasoning and vision decisions.

## Routing

The `model` field accepts more than one id:

- comma-separated concrete ids are a sequential failover pool;
- appending `:latency` selects the member with the fastest first token;
- appending `:throughput` selects the member with the highest tokens per second;
- saved dashboard pools expose `default`, `default:latency`, and
  `default:throughput`, managed through `GET/POST https://api.chutes.ai/model_aliases/`;
- a single concrete id bypasses routing entirely.

`Auto (Chutes Router)` in this CLI uses the native routing string and falls back
to a live inline pool with the `:latency` strategy, so it is the right default
for interactive work. Fallback is limited to transient or model-unavailable
failures before streaming begins; authentication and invalid-request errors do
not silently switch models. `CHUTES_STRICT_MODEL=1` disables fallback,
`CHUTES_FALLBACK_MODELS` sets a chain, and `CHUTES_ROUTING_POOL` sets an inline
pool. See `05-configuration.md` and `11-custom-models.md` for the full contract.

## Chat features

Standard OpenAI features are available per model capability: `stream: true` SSE,
tools/function calling, `response_format` JSON mode, JSON-schema structured
outputs, and image inputs for models whose `input_modalities` include `image`.
Only send sampling parameters listed in `supported_sampling_parameters`; sglang
and vllm engines accept different knobs. Reasoning controls depend on the model
generation, which the CLI resolves from the matching model entry.

Prompt pricing is per million tokens, and cached prompt reads bill at the
model's `input_cache_read` rate automatically. Keeping a stable prompt prefix
therefore reduces both latency and cost.

## Media chutes

Image, video, music, speech, and image-editing chutes do not run on the gateway.
Each has its own host (`https://<slug>.chutes.ai`) and a flat `POST /generate`
call that returns raw media bytes. They render synchronously, bill per
compute-second at the chute's GPU rate, and the public edge times out at about
ten minutes. A 504 is an HTML error page, not media, so check the status and
content type. Schemas differ per chute and published OpenAPI may be incomplete;
the deployed chute source is authoritative (`GET
https://api.chutes.ai/chutes/code/{chute_id}`).

In Chutes Build, `generate_media` handles this end to end. It resolves a model
from the live catalog, selects the right cord for the requested kind, fills the
declared text field, validates the payload locally, and saves outputs inside the
workspace. `/imagine [model] <prompt>` and `/imagine-video [model] <prompt>`
route through it; omit the model to auto-select an available one, or provide one
to choose explicitly. `list_media_models` and `describe_media_model` expose the
live list and exact schema.

## Documents, OCR, speech, embeddings

- `ocr_page` extracts text verbatim from one image or PDF page with a dedicated
  vision model; the image is not added to the conversation.
- `extract_document` handles multi-page PDFs and structured extraction through
  the document chute; prefer it over `ocr_page` for multi-page or structured
  input.
- `transcribe_audio` transcribes speech.
- `embed_texts` produces embeddings.
- `read_file` renders PDF pages as images for vision models, extracts PDF text
  with `format: "text"`, reads PPTX and notebooks, and passes raster images as
  multimodal content when the active model supports vision; otherwise the image
  is described by a vision-capable route instead of being dropped.

## Account, quota, and billing

Account and quota data live on `https://api.chutes.ai` and are derivable from
the API key alone. `/usage` and the status bar show the plan plus rolling
four-hour and monthly windows; `chutes-build account` and `chutes-build usage`
expose the same data, and their JSON forms are in the command help. Chat billing
is per million tokens with cache-read discounts; media billing is per
compute-second. Never invent balances or quotas: if the endpoint is unavailable,
say so.

## TEE and privacy

Hosted models advertise `confidential_compute: true` when they run inside a
hardware enclave. Filter on that boolean, not on a `-TEE` suffix. Attestation
evidence is available at `GET
https://api.chutes.ai/chutes/{chute_id}/evidence` with a nonce, and golden
measurements at `GET https://api.chutes.ai/servers/tee/measurements`. Do not
claim cryptographic attestation unless the quote was actually verified. The
inference plane never receives data from other sessions, and this CLI keeps
sessions, credentials, and generated artifacts local.

## Errors and gotchas

- `401` means the key is missing, invalid, or not allowed on that endpoint.
- `429` can be an anonymous rate limit (for example when auth was not sent) or a
  genuine quota/rate limit; check which before retrying.
- Model-specific `404` can mean a chute is private or not owned by the account;
  use the catalog to distinguish an unknown model from an access problem.
- Media `504` after roughly ten minutes is an ingress timeout, not a completed
  render.
- Auth and invalid-request errors must not trigger a silent model switch.
- Re-check the live catalog for model ids, context limits, and capabilities
  instead of trusting a cached list or a memory note.

## How this CLI maps the platform

Tools: `generate_media`, `list_media_models`, `describe_media_model`,
`ocr_page`, `extract_document`, `transcribe_audio`, `embed_texts`, `web_search`,
`web_fetch`, and the account/usage commands. Slash commands: `/model`,
`/effort`, `/imagine`, `/imagine-video`, `/usage`, `/account`. Configuration:
`05-configuration.md`, `11-custom-models.md`, and `26-config-reference.md`.
For anything not covered here, read those files and consult
https://chutes.ai/docs and https://chutes.ai/news; official docs take precedence
over third-party summaries, and unverified claims must be stated as unverified.
