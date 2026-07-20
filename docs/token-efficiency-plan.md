# Token Efficiency Plan

## Implementation status (2026-07-19)

The low-risk foundation is implemented:

- generated media now returns a compact model receipt while the complete typed
  metadata remains available to ACP and the TUI;
- identical project-instruction bodies are emitted once, keeping the deepest
  occurrence so precedence is unchanged;
- prompt rendering records local numeric character/token estimates; existing
  per-turn signals already record provider input/cache/output tokens and TTFT.
- a deterministic local micro-benchmark compares exact-body project-instruction
  deduplication and compact typed-media receipts against their legacy forms,
  including explicit content-preservation invariants.

No model, reasoning level, validation evidence, or answer-quality setting is
reduced by these changes. The full task benchmark corpus, capability-scoped
schema loading, content-hash context reuse, structured conversation deltas, and
release gate remain follow-up work; the targets below are not claimed as
achieved yet.

Run the implemented micro-benchmark locally with:

```powershell
cargo run -p xai-grok-agent --example token_efficiency_benchmark --quiet
```

It uses built-in synthetic fixtures, prints JSON, performs no network access,
and reads no repository source or user prompts.

## Objective

Reduce input tokens, time to first token, and avoidable model work without
lowering answer quality, silently changing reasoning modes, or hiding evidence
needed for safe coding decisions.

The optimization target is the complete agent workload: main-loop prompts,
tool results, repository context, compaction, advisor calls, and subagent
handoffs. Output brevity alone is not a sufficient optimization.

## Non-negotiable quality constraints

- Preserve the user's selected model and reasoning mode.
- Do not lower validation, truncate error evidence, or omit requested detail to
  meet a token target.
- Keep full tool artifacts locally when their inline representation is reduced.
- Measure success on task completion and regression suites, not token counts in
  isolation.
- Keep metrics local and free of prompt, secret, and source-content payloads.

## Baseline and measurement

Add a local per-turn efficiency record containing only numeric and categorical
metadata:

- model and route selected;
- input, cached-input, reasoning, and output tokens when reported;
- estimated tokens by prompt section;
- time to first token, total latency, and streaming throughput;
- tool-call count and inline tool-result bytes;
- compaction count and before/after token estimates;
- fallback, advisor, and subagent counts;
- task outcome from deterministic checks where available.

Aggregate this data only through an explicit local diagnostic command. Never
send it to Chutes Build or third-party telemetry services.

## Workstreams

### 1. Stable prompt core

Build the system prompt from named, hashed sections. Keep the stable safety and
agent contract first, followed by session-specific sections. Remove repeated
policy text across the base prompt, project instructions, subagent templates,
and tool guidance. Add snapshot tests that fail when duplicate normalized
paragraphs reappear.

Success criterion: at least 15% fewer fixed prompt tokens with identical policy
coverage in prompt-contract tests.

### 2. Capability-scoped tool disclosure

Expose only tools available in the active mode and defer long instructions
until the relevant tool, skill, MCP server, or media workflow is selected.
Preserve short, stable tool summaries in the initial schema and load detailed
operating guidance on demand.

Success criterion: tool-definition tokens scale with active capabilities, not
with the total installed ecosystem.

### 3. Repository context selection

Prefer the existing codebase graph, symbol boundaries, focused search results,
and changed hunks over whole-file replay. Cache immutable file excerpts by
content hash within the session and reference unchanged evidence instead of
re-inserting it. Invalidate entries when file identity or content changes.

Success criterion: repeated edits to the same area do not resend unchanged
files, while repository-grounded evaluation accuracy remains unchanged.

### 4. Tool-result budgets with durable artifacts

Give each tool a typed inline budget. Deduplicate search matches, collapse
repeated diagnostics, and summarize long command or MCP output while retaining
the complete raw result in the existing local artifact storage. The model must
receive the artifact path, truncation reason, retained ranges, and a supported
way to retrieve more.

Success criterion: no unmarked truncation and at least 30% fewer tool-result
tokens on log-heavy benchmark tasks.

### 5. Incremental conversation state

Stop re-describing stable task state every turn. Maintain a compact structured
state containing the objective, accepted constraints, decisions, changed
files, verification evidence, and remaining work. Include only deltas plus the
small current state in normal turns.

Success criterion: a 20-turn coding session grows sublinearly when the working
set is stable.

### 6. Evidence-preserving compaction

Tune the existing compaction system around structured state. Summaries must
retain exact file paths, commands, failures, decisions, and unverified claims.
Large historical evidence remains recoverable through local compaction
segments instead of being copied into every post-compaction turn.

Success criterion: post-compaction task completion matches the uncompressed
baseline, with no increase in repeated reads or forgotten constraints.

### 7. Advisor and subagent handoffs

Send workers a minimal task capsule rather than the full main transcript. A
capsule contains scope, relevant constraints, selected evidence references,
expected output, and budget. Workers return structured findings with artifact
references; the orchestrator merges findings once and removes duplicates.

Success criterion: parallel execution consumes fewer aggregate input tokens
than sequential full-transcript replay for the same decomposed task.

### 8. Provider-aware caching

Record provider-reported cached tokens and keep cacheable prompt prefixes
stable. Do not assume a Chutes model or route supports a specific cache-control
extension. Enable model-specific wire fields only when the live capability
contract explicitly advertises them.

Success criterion: supported routes show higher cache reuse without changing
requests sent to routes that do not advertise caching.

### 9. Output discipline

Default to outcome-first, non-repetitive answers while honoring requests for
detail. Avoid restating plans, logs, and diffs already visible to the user.
This is presentation discipline, not a hidden output-token cap.

## Benchmark suite

Create reproducible local scenarios for:

1. small bug fix with one regression test;
2. repository-wide investigation;
3. long failing build output;
4. documentation lookup with Context7 and official sources;
5. advisor-assisted architecture decision;
6. three-worker parallel implementation;
7. image/video generation followed by inspection;
8. a 20-turn session crossing the compaction threshold.

Compare the baseline and candidate on median and p95 input tokens, time to first
token, total latency, tool calls, compactions, deterministic test results, and a
fixed quality rubric.

## Delivery order

1. Instrument local measurements and freeze the benchmark corpus.
2. Remove prompt duplication and scope tool disclosure.
3. Add typed tool-result budgets and content-hash reuse.
4. Introduce structured conversation state and compact worker capsules.
5. Tune compaction and provider-aware caching from measured results.
6. Enable optimizations by default only after quality and safety gates pass.

## Release gate

The first release should target at least 25% fewer median input tokens and 20%
faster median time to first token on the benchmark suite, with no regression in
deterministic task success, safety decisions, or reviewer-rated output quality.
Results must be published as local reproducible benchmark data, not marketing
estimates.
