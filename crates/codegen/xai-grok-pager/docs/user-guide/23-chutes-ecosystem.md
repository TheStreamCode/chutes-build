# Chutes Models and Ecosystem

Chutes Build discovers models and capabilities from the live Chutes catalog.
`Auto (Chutes Router)` is inserted as the first model choice; its stable ID is
`model-router`. Auto delegates task classification and model selection to the
Chutes router. A concrete model is tried first, followed by
`CHUTES_FALLBACK_MODELS`, then Auto. Set `CHUTES_STRICT_MODEL=1` to disable
fallback entirely.

Fallback is limited to transient/model-unavailable failures before streaming
begins. Authentication and invalid-request errors do not silently change models.
Every fallback attempt recalculates reasoning controls for its own model family.

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
