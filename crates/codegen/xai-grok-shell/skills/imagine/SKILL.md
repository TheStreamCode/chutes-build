---
name: imagine
description: >
  Chutes-native workflow for generating and editing images, video, music, and
  speech. Load this immediately before using the Chutes media tools.
metadata:
  short-description: "Chutes multimodal generation workflow"
---

# Chutes Media

`generate_media` is self-contained. It resolves the model name against the
catalog, picks the cord for the requested kind, and places `prompt` into
whatever field that cord declares for free text. One call is the normal case:

```
generate_media { model: "FLUX.1-schnell", kind: "image", prompt: "..." }
```

1. Call `generate_media` with the model, the media kind, and the prompt.
2. Report the workspace-relative output path and any material model limitation.

Add `params` only for settings the user actually asked for (size, steps,
duration, voice) or to pass a source asset. A wrong field name is caught
locally: the error lists the fields that cord accepts, so correct the call and
retry rather than starting the workflow over.

Reach for the other two tools when they earn their round-trip:

- `list_media_models` — the user has not named a model, or you need to compare
  candidates against stated constraints. Pass a `kind` and a focused `query`.
- `describe_media_model` — you need exact field names or value ranges before
  committing to an expensive video or multi-shot run, or an error told you the
  schema disagrees.

Do not invent durations, aspect ratios, or source-asset field names; those come
from the schema, not from habit. For edits, reference workspace files through a
field the model declares — Chutes Build encodes those assets locally before the
request and saves generated files with a provenance sidecar by default.

## Choosing the right medium

- Use code for visuals whose exact text, data, labels, geometry, or layout must be
  correct. Render and inspect the result.
- Use image models for photographic, illustrative, artistic, or decorative work.
- Use an edit-capable image model when the user supplies a source image or needs
  consistency with an earlier result.
- Use a video model whose described schema matches text-to-video, image-to-video,
  or reference-video requirements; do not assume one workflow fits every model.
- Use music or speech models only after checking language, duration, format, and
  voice fields in the live schema.

## Quality and safety

- Preserve the user's core prompt, required subjects, composition, and exclusions.
- Use a stable reference asset and shared visual description across related shots.
- Keep multi-shot video parameters consistent, then assemble compatible clips with
  FFmpeg when the user asks for a sequence.
- Verify output files exist and inspect them when the task requires visual or
  factual accuracy.
- Do not expose credentials, remote payload internals, or absolute private paths in
  the final response.
