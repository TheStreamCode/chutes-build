#!/usr/bin/env python3
"""Measure Chutes chat-completions latency from this machine.

The script talks to the OpenAI-compatible Chutes inference endpoint with the
same streaming contract the CLI uses, so it isolates provider latency from TUI
rendering. It reports time to first token (TTFT), median inter-token latency
(ITL), total stream time, and output size for each run, then the medians.

Usage:
    export CHUTES_API_KEY=cpk_...          # PowerShell: $env:CHUTES_API_KEY = "cpk_..."
    python scripts/bench_chutes.py --model default:latency
    python scripts/bench_chutes.py --model zai-org/GLM-5.2-TEE --runs 5
    python scripts/bench_chutes.py --model "zai-org/GLM-5.2-TEE,deepseek-ai/DeepSeek-V3.2-TEE:latency"

No key is written anywhere; the request goes only to the configured base URL.
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
import time
import urllib.error
import urllib.request

DEFAULT_BASE_URL = "https://llm.chutes.ai/v1"
DEFAULT_MODEL = "default:latency"
DEFAULT_PROMPT = (
    "Explain in three short sentences why a simple, stable prompt prefix helps "
    "time to first token in streaming inference."
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", default=DEFAULT_MODEL, help="model id or routing string")
    parser.add_argument("--prompt", default=DEFAULT_PROMPT, help="user prompt to send")
    parser.add_argument("--runs", type=int, default=3, help="number of measured runs (default 3)")
    parser.add_argument("--max-tokens", type=int, default=256, help="max completion tokens")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL, help="OpenAI-compatible base URL")
    parser.add_argument("--api-key-env", default="CHUTES_API_KEY", help="environment variable holding the key")
    return parser.parse_args()


def build_request(args: argparse.Namespace, api_key: str) -> urllib.request.Request:
    body = {
        "model": args.model,
        "messages": [{"role": "user", "content": args.prompt}],
        "stream": True,
        "max_tokens": args.max_tokens,
    }
    return urllib.request.Request(
        f"{args.base_url.rstrip('/')}/chat/completions",
        data=json.dumps(body).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
            "Accept": "text/event-stream",
        },
        method="POST",
    )


def measure_once(args: argparse.Namespace, api_key: str) -> dict:
    started = time.perf_counter()
    first_token_at = None
    token_times: list[float] = []
    chunks = 0
    characters = 0
    prompt_tokens = None
    completion_tokens = None

    try:
        with urllib.request.urlopen(build_request(args, api_key), timeout=180) as response:
            status = response.status
            for raw_line in response:
                line = raw_line.decode("utf-8", "replace").strip()
                if not line.startswith("data:"):
                    continue
                payload = line[5:].strip()
                if payload == "[DONE]":
                    break
                try:
                    event = json.loads(payload)
                except json.JSONDecodeError:
                    continue
                usage = event.get("usage") or {}
                prompt_tokens = usage.get("prompt_tokens", prompt_tokens)
                completion_tokens = usage.get("completion_tokens", completion_tokens)
                for choice in event.get("choices") or []:
                    delta = choice.get("delta") or {}
                    text = delta.get("content")
                    if text:
                        chunks += 1
                        characters += len(text)
                        now = time.perf_counter()
                        if first_token_at is None:
                            first_token_at = now
                        token_times.append(now)
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", "replace")[:300]
        return {"status": error.code, "error": body}
    except OSError as error:
        return {"status": None, "error": str(error)}

    total_s = time.perf_counter() - started
    ttft_ms = None if first_token_at is None else (first_token_at - started) * 1000
    intervals = [
        (later - earlier) * 1000 for earlier, later in zip(token_times, token_times[1:])
    ]
    return {
        "status": status,
        "ttft_ms": ttft_ms,
        "itl_ms": statistics.median(intervals) if intervals else None,
        "total_ms": total_s * 1000,
        "chunks": chunks,
        "characters": characters,
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
    }


def main() -> int:
    args = parse_args()
    api_key = os.environ.get(args.api_key_env)
    if not api_key:
        print(
            f"error: {args.api_key_env} is not set; export a Chutes API key before running",
            file=sys.stderr,
        )
        return 2
    if args.runs < 1:
        print("error: --runs must be at least 1", file=sys.stderr)
        return 2

    results = []
    for index in range(args.runs):
        result = measure_once(args, api_key)
        result["run"] = index + 1
        results.append(result)

    ok = [result for result in results if result.get("status") == 200 and result.get("ttft_ms")]
    summary = {
        "model": args.model,
        "runs": args.runs,
        "ok_runs": len(ok),
        "ttft_median_ms": statistics.median([r["ttft_ms"] for r in ok]) if ok else None,
        "itl_median_ms": statistics.median([r["itl_ms"] for r in ok if r["itl_ms"]]) if ok else None,
        "total_median_ms": statistics.median([r["total_ms"] for r in ok]) if ok else None,
        "chunks_median": statistics.median([r["chunks"] for r in ok]) if ok else None,
        "characters_median": statistics.median([r["characters"] for r in ok]) if ok else None,
    }
    print(json.dumps({"summary": summary, "runs": results}, indent=2))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
