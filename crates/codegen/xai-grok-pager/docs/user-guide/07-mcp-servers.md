# MCP Servers

Chutes Build supports external MCP servers over the inherited MCP runtime. Add
only servers you trust: MCP tools can receive conversation context and may read,
write, or contact external services according to their own implementation.

The core Chutes media workflow is built in natively. Image generation/editing,
video, music, and speech use `list_media_models`, `describe_media_model`, and
`generate_media`; a separate `chutes-media-mcp` Node process is not required.
`generate_media` is self-contained — it resolves the model name and the cord's
own prompt field — so the other two tools are for choosing between models and
inspecting an exact schema, not mandatory preliminaries.

The native implementation retains schema validation, optional warmup, cold
start retry, workspace-bounded output, and provenance sidecars. Compatible
settings include `CHUTES_OUTPUT_DIR`, `CHUTES_WARMUP`,
`CHUTES_COLD_START_RETRIES`, `CHUTES_ALLOW_UNKNOWN_PARAMS`, and
`CHUTES_PROVENANCE`.

Managed MCP catalog refreshes propagate into live parent sessions and rebuild
the tool-search index without an application restart. Plugin subagents can see
only their parent's already-connected pool after the normal inheritance filter;
plugins cannot declare a new server, hook, or permission policy for themselves.

Generated media downloads default to 128 MiB with a 512 MiB hard ceiling and
stream through temporary files instead of requiring a complete in-memory body.
