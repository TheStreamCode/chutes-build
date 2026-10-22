# Monitoring Usage (External OpenTelemetry)

The external OpenTelemetry stream is **deadened in this build**: `init`
is a no-op, `is_active()` is always `false`, and no OTLP exporter is ever
constructed. No data can leave the process via this path regardless of any
environment variable or setting.

Concretely, all of these are parsed but inert here:

- `CHUTES_BUILD_EXTERNAL_OTEL` and the `OTEL_*` exporter variables.
- `[telemetry] otel_*` config keys.
- `[features] telemetry`, `[telemetry] trace_upload`, and their
  `CHUTES_BUILD_TELEMETRY_*` variables (the distribution withholds the
  telemetry capability outright).

For what the product sends where, see `PRIVACY.md` in the repository. For
the per-session token and cost totals shown in the app, see `/usage` in
[Slash Commands](04-slash-commands.md).
