#!/usr/bin/env python3
"""Brand guard for the Chutes Build tree.

Until 1.7.x this script re-applied the Chutes identity after every upstream
port: a table of substitutions, a per-file classifier for the ambiguous tokens,
and a repair pass for the damage a blind rename does. As of 2.0.0 that is over.
Upstream synchronization is frozen (`docs/upstream-sync.md`) and the rebrand is
complete: crates, modules, identifiers and the local wire all carry Chutes
names, and the only upstream spellings left are the compatibility surface
documented in `docs/product-spec.md` §1.5.

So this script now does one thing: **fail when an upstream token appears
outside that surface**. It is the executable form of spec §1.4/§1.5, and it runs
in CI and in `AGENTS.md`'s gate.

Usage:
    python scripts/rebrand.py --check     # verify; exit 1 on any violation
    python scripts/rebrand.py             # same, without the CI-friendly exit code

The allowlist lives in `scripts/rebrand_files.json`, next to this file: every
entry names the reason it is allowed, so "why is this `grok` still here?" has an
answer in the diff. Adding an entry is a policy decision, not a cleanup —
prefer fixing the token unless it is one of the §1.5 classes.

`--apply`, `--reclassify` and the dry-run substitution mode are gone on
purpose: there is no upstream tree to re-brand, and a script that can rewrite
identifiers is exactly the tool that produced the 2026-10 rebrand defects (a
rewritten historical changelog, a duplicated match arm). The frozen replacement
procedure is a spec change, not a flag on this script.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
ALLOWLIST_PATH = Path(__file__).resolve().parent / "rebrand_files.json"

# Directories never walked: vendored identity, build output, dependencies.
SKIP_DIRS = {
    ".git",
    "target",
    "node_modules",
    "third_party",
    "output",
}

SKIP_PREFIXES = (
    "dist/",
    "npm/dist/",
)

# What counts as an upstream token in a live file. `grok` and `xai` cover the
# crate, identifier, host and prose families; `x.ai` / `grok.com` are the two
# host names that do not contain either substring in every spelling.
FORBIDDEN = re.compile(r"(?i)\bgrok|\bxai\b|x\.ai|grok\.com|supergrok|grokbuild")

TEXT_SUFFIXES = {
    ".rs",
    ".toml",
    ".json",
    ".md",
    ".proto",
    ".py",
    ".mjs",
    ".cjs",
    ".js",
    ".ts",
    ".yml",
    ".yaml",
    ".txt",
    ".snap",
    ".sh",
    ".ps1",
    ".html",
    ".css",
    ".lock",
    ".cfg",
    ".ini",
    ".kt",
    ".swift",
    ".sql",
}


def load_allowlist() -> dict:
    if not ALLOWLIST_PATH.exists():
        raise SystemExit(f"missing allowlist: {ALLOWLIST_PATH}")
    return json.loads(ALLOWLIST_PATH.read_text(encoding="utf-8"))


def compile_patterns(entries: list[dict]) -> list[tuple[re.Pattern[str], str]]:
    return [(re.compile(entry["pattern"]), entry.get("reason", "")) for entry in entries]


def walk(root: Path):
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
            continue
        rel = path.relative_to(root).as_posix()
        if any(part in SKIP_DIRS for part in path.relative_to(root).parts):
            continue
        if rel.startswith(SKIP_PREFIXES):
            continue
        yield path, rel


def assert_no_control_bytes() -> None:
    """Fail loudly if an escape in this file was eaten in transit."""
    raw = Path(__file__).read_bytes()
    bad = {b for b in raw if b < 9 or 10 < b < 32}
    if bad:
        names = ", ".join(f"0x{b:02x}" for b in sorted(bad))
        raise SystemExit(
            f"rebrand.py contains control bytes ({names}); an escape was eaten "
            "in transit. Re-check the file before trusting a run."
        )


def main() -> int:
    assert_no_control_bytes()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit 1 when a forbidden token is outside the allowlist",
    )
    parser.add_argument("--root", default=str(REPO_ROOT), help="tree to check")
    parser.add_argument("--show-allowed", action="store_true", help="print the allowlist")
    args = parser.parse_args()

    if not args.check and args.show_allowed is False and sys.stdout.isatty():
        print("note: this script only checks; --apply was removed at 2.0.0 "
              "(docs/upstream-sync.md)\n", file=sys.stderr)

    allowlist = load_allowlist()
    allowed_files = allowlist.get("files", {})
    allowed_prefixes = allowlist.get("prefixes", {})
    global_lines = compile_patterns(allowlist.get("lines", []))
    per_file = {
        rel: compile_patterns(entries)
        for rel, entries in allowlist.get("per_file_lines", {}).items()
    }

    if args.show_allowed:
        print("== files allowed to carry upstream spellings ==")
        for rel, reason in sorted(allowed_files.items()):
            print(f"  {rel}\n      {reason}")
        print("\n== allowed line patterns ==")
        for pattern, reason in global_lines:
            print(f"  {pattern.pattern}\n      {reason}")
        return 0

    root = Path(args.root).resolve()
    violations: list[tuple[str, int, str, str]] = []
    for path, rel in walk(root):
        if rel in allowed_files:
            continue
        if any(rel.startswith(prefix) for prefix in allowed_prefixes):
            continue
        file_patterns = per_file.get(rel, [])
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (UnicodeDecodeError, OSError):
            continue
        for number, line in enumerate(lines, 1):
            if not FORBIDDEN.search(line):
                continue
            if any(pattern.search(line) for pattern, _ in global_lines):
                continue
            if any(pattern.search(line) for pattern, _ in file_patterns):
                continue
            violations.append((rel, number, line.strip()[:120], ""))

    if violations:
        print(f"ERROR: {len(violations)} upstream token(s) outside the section 1.5 allowlist:\n")
        shown = 0
        current = None
        for rel, number, line, _ in violations:
            if rel != current:
                print(f"\n  {rel}")
                current = rel
            print(f"    {number:>5}: {line}")
            shown += 1
            if shown >= 60:
                print(f"\n  … and {len(violations) - shown} more")
                break
        print(
            "\nFix the token, or add a justified entry to "
            "scripts/rebrand_files.json (spec section 1.5 classes only)."
        )
        return 1

    print(f"brand guard: clean ({len(allowed_files)} files and "
          f"{len(global_lines)} line patterns allowed by spec section 1.5).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
