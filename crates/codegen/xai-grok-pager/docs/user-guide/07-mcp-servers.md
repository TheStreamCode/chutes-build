# MCP Servers

Chutes Build supports external MCP servers over the inherited MCP runtime. Add
only servers you trust: MCP tools can receive conversation context and may read,
write, or contact external services according to their own implementation.

The core Chutes media workflow is built in natively. Image generation/editing,
video, music, and speech use `list_media_models`, `describe_media_model`, and
`generate_media`; a separate `chutes-media-mcp` Node process is not required.

The native implementation retains schema validation, optional warmup, cold
start retry, workspace-bounded output, and provenance sidecars. Compatible
settings include `CHUTES_OUTPUT_DIR`, `CHUTES_WARMUP`,
`CHUTES_COLD_START_RETRIES`, `CHUTES_ALLOW_UNKNOWN_PARAMS`, and
`CHUTES_PROVENANCE`.
