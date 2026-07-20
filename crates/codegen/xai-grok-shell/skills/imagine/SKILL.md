---
name: imagine
description: >
  Chutes-native workflow for generating and editing images, video, music, and
  speech. Load this immediately before using the Chutes media tools.
metadata:
  short-description: "Chutes multimodal generation workflow"
---

# Chutes Media

Use the native Chutes tools as a schema-first workflow:

1. Call `list_media_models` with the requested media kind and a focused query.
2. Select the best model for the user's constraints, not merely the first result.
3. Call `describe_media_model` and inspect its callable cords and exact input schema.
4. Call `generate_media` with that model, media kind, and only supported parameters.
5. Report the workspace-relative output path and any material model limitation.

Never guess a model slug, cord, parameter name, duration, aspect ratio, or source
asset field. For edits, pass workspace files only through fields declared by the
selected model schema. Chutes Build encodes those assets locally before the
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
