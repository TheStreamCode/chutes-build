#!/usr/bin/env python3
"""Sort an upstream delta into what a machine can take and what a human must read.

A sync's real cost is not the file count, it is deciding per file whether taking
upstream's version loses anything of ours. Measured against `b13fa526`: of 139
overlapping `.rs` files, 48 diverge from upstream by *exactly* the rebrand — the
script reproduces our version from theirs, character for character — so a third
of the work is a decision nobody needs to make by hand.

Buckets, in descending safety:

  done        We already hold upstream's version. Asked first, or work from an
              earlier session comes back looking divergent and gets reviewed twice.
  new         Not in our tree. Nothing of ours to lose.
  clean       We never touched it since the baseline. Nothing of ours to lose.
  mechanical  We touched it, and `rebrand(upstream@base) == ours@HEAD`, so our
              divergence *is* the rebrand and re-applying it reproduces the work.
  deleted     We removed it on purpose. Absent from the tree, so it would
              otherwise read as `new` and be resurrected.
  manual      Genuine divergence. Read the diff.

`--apply` writes `new`, `clean` and `mechanical` from `upstream/main` and
re-runs the rebrand over them. It never touches `deleted` or `manual`, and
skips binaries, which `git show` cannot round-trip through a text decode. Taking a file is where the work starts,
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

# Extensions `show()` cannot round-trip: it decodes as UTF-8 with replacement,
# so a PNG or a font written back through it is quietly destroyed.
BINARY_SUFFIXES = frozenset(
    {".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".ttf", ".otf", ".woff", ".woff2",
     ".zip", ".gz", ".zst", ".tar", ".pdf", ".wasm", ".bin"}
)
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

    # Paths we removed on purpose. Absent from the working tree, so without
    # this they read as "new — nothing of ours to lose" and `--apply` puts them
    # back. 19 files qualify today (the `npm/grok-*` packages the rebrand
    # replaced, `theme/groknight.rs`); none is in the current delta, but the
    # first upstream release that touches one would resurrect xAI-branded npm
    # packages silently.
    deleted_by_us = {
        p
        for p in git("diff", "--diff-filter=D", "--name-only", base, "HEAD", "--", *paths)
        .splitlines()
        if p
    }

    buckets: dict[str, list[str]] = {
        "done": [],
        "new": [],
        "clean": [],
        "mechanical": [],
        "deleted": [],
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
        if path in deleted_by_us:
            buckets["deleted"].append(path)
            continue
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
            if REPO_ROOT.joinpath(path).suffix.lower() in BINARY_SUFFIXES:
                # `show()` decodes with errors="replace", so writing a blob back
                # as text corrupts it silently. The repo tracks PNGs and fonts.
                print(f"  skipped (binary, take it by hand): {path}")
                continue
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
        "deleted": "we removed it on purpose — do not resurrect",
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
