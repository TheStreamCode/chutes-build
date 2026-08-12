#!/usr/bin/env python3
"""Sort an upstream delta into what a machine can take and what a human must read.

A sync's real cost is not the file count, it is deciding per file whether taking
upstream's version loses anything of ours. Measured against `b13fa526`: of 139
overlapping `.rs` files, 48 diverge from upstream by *exactly* the rebrand — the
script reproduces our version from theirs, character for character — so a third
of the work is a decision nobody needs to make by hand.

Four buckets, in descending safety:

  new         Not in our tree. Nothing of ours to lose.
  clean       We never touched it since the baseline. Nothing of ours to lose.
  mechanical  We touched it, and `rebrand(upstream@base) == ours@HEAD`, so our
              divergence *is* the rebrand and re-applying it reproduces the work.
  manual      Genuine divergence. Read the diff.

`--apply` writes the first three from `upstream/main` and re-runs the rebrand
over them. It never touches `manual`. Taking a file is where the work starts,
not where it ends: an area is routinely larger than its files, so compile,
run the suite, and check it against `scripts/known_failures.py` before
believing any of it.

Usage:
    python scripts/port_assist.py --base afbc0fb7
    python scripts/port_assist.py --base afbc0fb7 --path crates/codegen/xai-grok-tools
    python scripts/port_assist.py --base afbc0fb7 --apply
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import rebrand  # noqa: E402  (path set above)


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=REPO_ROOT,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    if result.returncode != 0:
        raise SystemExit(f"port_assist: `git {' '.join(args)}` failed:\n{result.stderr}")
    return result.stdout


def show(ref: str, path: str) -> str | None:
    result = subprocess.run(
        ["git", "show", f"{ref}:{path}"],
        cwd=REPO_ROOT,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    return result.stdout if result.returncode == 0 else None


def classify(base: str, upstream: str, paths: list[str]) -> dict[str, list[str]]:
    changed = [p for p in git("diff", "--name-only", base, upstream, "--", *paths).splitlines() if p]
    ours_changed = set(git("diff", "--name-only", base, "HEAD", "--", *paths).splitlines())
    decisions = rebrand.load_decisions()

    buckets: dict[str, list[str]] = {
        "done": [],
        "new": [],
        "clean": [],
        "mechanical": [],
        "manual": [],
    }

    def rebranded(source: str | None, path: str) -> str | None:
        if source is None:
            return None
        try:
            return rebrand.apply_rules(source, path, decisions)[0]
        except SystemExit:
            # A pinned upstream reference no longer matches — that is the
            # rebrand script's own guard firing, and it is a human's call.
            return None

    for path in changed:
        if not (REPO_ROOT / path).exists():
            buckets["new"].append(path)
            continue

        ours = show("HEAD", path)
        # Ask this first. A file ported in an earlier session already holds
        # upstream's new content, so every later test would call it divergent
        # and hand it back for review that has already happened.
        if ours is not None and rebranded(show(upstream, path), path) == ours:
            buckets["done"].append(path)
            continue

        if path not in ours_changed:
            buckets["clean"].append(path)
            continue

        if ours is not None and rebranded(show(base, path), path) == ours:
            buckets["mechanical"].append(path)
        else:
            buckets["manual"].append(path)
    return buckets


def apply(upstream: str, buckets: dict[str, list[str]]) -> int:
    taken = 0
    for bucket in ("new", "clean", "mechanical"):
        for path in buckets[bucket]:
            content = show(upstream, path)
            if content is None:
                print(f"  skipped (unreadable at {upstream}): {path}")
                continue
            target = REPO_ROOT / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(content, encoding="utf-8", newline="")
            taken += 1
    return taken


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="baseline commit (.github/upstream.json)")
    parser.add_argument("--upstream", default="upstream/main", help="upstream ref")
    parser.add_argument("--path", action="append", default=[], help="limit to a path")
    parser.add_argument("--apply", action="store_true", help="write the take-able buckets")
    args = parser.parse_args()

    paths = args.path or ["."]
    buckets = classify(args.base, args.upstream, paths)
    total = sum(len(v) for v in buckets.values())
    if total == 0:
        print("port_assist: nothing changed upstream for those paths")
        return 0

    labels = {
        "done": "already ported — we hold upstream's version",
        "new": "not in our tree",
        "clean": "untouched by us since the baseline",
        "mechanical": "our divergence is exactly the rebrand",
        "manual": "genuine divergence — read it",
    }
    for bucket, label in labels.items():
        files = buckets[bucket]
        share = 100 * len(files) // total
        print(f"\n{bucket:<11} {len(files):>4}  ({share:>2}%)  {label}")
        for path in files[:12]:
            print(f"    {path}")
        if len(files) > 12:
            print(f"    … and {len(files) - 12} more")

    takeable = sum(len(buckets[b]) for b in ("new", "clean", "mechanical"))
    remaining = takeable + len(buckets["manual"])
    print(
        f"\n{takeable} of the {remaining} still to do can be taken without a "
        f"judgement call ({len(buckets['done'])} already ported)."
    )

    if not args.apply:
        print("Re-run with --apply to write them, then rebrand, compile and test.")
        return 0

    taken = apply(args.upstream, buckets)
    print(f"\nWrote {taken} file(s) from {args.upstream}. Now:")
    print("  python scripts/rebrand.py --apply   # and read `git status`")
    print("  cargo clippy -p <crate> --all-targets -- -D warnings")
    print("  python scripts/known_failures.py --crate <crate>")
    print(f"{len(buckets['manual'])} file(s) were left alone and need reading.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
