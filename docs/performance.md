# Performance and latency

Chutes Build targets the fastest practical time to first token (TTFT) and the
fewest round trips when Chutes is the inference provider. This page covers what
to measure, the benchmark included in the repository, and the fixed costs the
client controls.

## Measure the provider, not the TUI

The TUI adds render and input cost, so provider measurements use the same
streaming chat-completions contract directly:

```powershell
$env:CHUTES_API_KEY = "cpk_..."
python scripts/bench_chutes.py --model default:latency
python scripts/bench_chutes.py --model default --runs 3
python scripts/bench_chutes.py --model zai-org/GLM-5.2-TEE --runs 5
```

The script prints JSON per run plus medians:

- `ttft_median_ms`: time from request start to the first content delta.
- `itl_median_ms`: median interval between content-bearing chunks.
- `total_median_ms`: full stream time for the requested `--max-tokens`.
- `chunks_median` / `characters_median`: output shape, so runs stay comparable.

Compare routing strategies with the same prompt and model pool. `default:latency`
selects the pool member with the fastest first token, `default` is sequential
failover, and a single concrete model id bypasses routing. A model id with a
`:latency` or `:throughput` suffix is an inline pool; see
`crates/codegen/chutes-pager/docs/user-guide/25-chutes-ecosystem.md`.

Do not benchmark with a cold prompt cache and a warm one as if they were the
same: the first run pays full prefill, later runs can read the cached prefix.
Run `--runs 3` and compare the median, and restart the script between cache
conditions.

## Fixed costs Chutes Build controls

- Tool catalog: 10,239 estimated tokens across 29 tools after the 2.0.3 compact
  schema export (was 11,573 in 2.0.2). `browser`, `extract_document`,
  `generate_media`, and `scheduler_create` export their canonical schemas with
  nested `description`/`examples` fields removed; dispatch still validates
  against the full schema.
- System prompt: 2,311 estimated tokens, including the Chutes platform map and
  the work-policy batching rule. Full platform knowledge is a bundled skill
  loaded on demand, not fixed prompt text.
- Tool loop: independent tool calls are issued in one response and dispatched
  concurrently, so a multi-read or multi-search step costs one model round trip
  instead of several.
- Streaming: the sampler keeps a shared HTTP/2 pool with keepalive, TCP no-delay,
  transport prewarm, and an HTTP/1.1 fallback. Prompt/tool prefixes are stable
  across turns so Chutes prompt caching can discount cache reads.

The remaining target is capability-scoped tool disclosure: keep the full
catalog locally but advertise only the tools a task needs, with details loaded
on demand. `docs/token-efficiency-plan.md` tracks the measurements and the
related workstreams.

## Reading the result

A slow TTFT with a short prompt points at routing, model load, or the provider,
not at the client. A slow TTFT with a long prompt points at prefill size: check
the prompt and catalog estimates above, and whether cache reads were reported.
ITL is dominated by the provider's decode speed; the client-side budget is the
stream parser, which tests pin separately. Never report a speed claim without a
fresh run of this script and the exact model or routing string used.
