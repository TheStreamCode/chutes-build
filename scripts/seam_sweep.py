"""Find seams a re-base loses in silence.

Phase 3 of the 1.0.0 re-base found the Chutes seams by asking "which files
import `chutes-build-core`". That question misses a whole class: a file whose
*structure* matches upstream and whose *values* are ours. An issuer URL, an OAuth
client id, an allowlisted origin, a WebSocket endpoint — both values compile,
both are plausible strings, and the tests around them usually assert against the
constant rather than the literal, so nothing fails. The product just quietly
talks to the wrong host.

Two modes, because the first one's blind spot cost us the endpoint table:

* `consts` compares `const NAME: T = "value"` declarations by name. Precise, and
  it reads like a table.
* `literals` compares every string literal in the file. Noisier, but it sees
  values inside struct initialisers, function bodies and macro arguments —
  which is where `relay_ws_url: "wss://code.grok.com/..."` was hiding while the
  `consts` pass reported the file clean.

Run it against the previous release before trusting a re-based tree:

    python scripts/seam_sweep.py --base <ref>            # both modes
    python scripts/seam_sweep.py --base <ref> --mode literals

`--base` is the ref that carries the values we shipped (a release tag, or the
pre-re-base branch). Findings are for a human to read: the fork's value is not
automatically the right one — several of ours are deliberate improvements and
several of the fork's are simply older.
"""

import argparse
import collections
import pathlib
import re
import subprocess
import sys

# `re.M` alone is not enough: rustfmt wraps a long declaration so the value
# lands on the following line, and a per-line pattern then reports the file
# clean. That is how `CLI_BASE_URL_FALLBACK` kept pointing at a GCS bucket
# through a sweep that had already found five other endpoint defaults in the
# same crate. `\s*` around the `=` is the whole fix.
CONST = re.compile(
    r"^[ \t]*(?:pub(?:\([\w:]+\))?[ \t]+)?(?:const|static)[ \t]+(\w+)[ \t]*:"
    r"[^=]+=\s*(\"(?:[^\"\\]|\\.)*\"|&\[[^\]]*\])\s*;",
    re.M,
)

STRING = re.compile(r"\"(?:[^\"\\\n]|\\.)*\"")

# What counts as identity: a host, a URL, a product name, an app id, a state
# path. A diff in anything else is upstream drift and not this tool's business.
IDENTITY = re.compile(
    r"https?://|wss?://|\.ai\b|\.com\b|chutes|grok|cid_|xai|@xai-official|127\.0\.0\.1",
    re.I,
)

SKIP_DIRS = {".git", "target", "node_modules", "dist"}


def git(root: pathlib.Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args], capture_output=True, text=True, encoding="utf-8", cwd=root
    ).stdout


def tracked_sources(root: pathlib.Path, base: str) -> list[str]:
    names = git(root, "ls-tree", base, "-r", "--name-only").split()
    return [
        n
        for n in names
        if n.endswith((".rs", ".toml", ".json", ".js", ".mjs", ".ts", ".sh", ".ps1"))
        and not any(part in SKIP_DIRS for part in pathlib.PurePosixPath(n).parts)
    ]


def compare_consts(theirs: str, ours: str) -> list[tuple[str, str, str]]:
    a = {m.group(1): m.group(2) for m in CONST.finditer(theirs)}
    b = {m.group(1): m.group(2) for m in CONST.finditer(ours)}
    return [(k, a[k], b[k]) for k in sorted(set(a) & set(b)) if a[k] != b[k]]


def compare_literals(theirs: str, ours: str) -> list[tuple[str, str, str]]:
    """Identity-bearing literals present on one side only.

    Reported as a set difference rather than pairwise: the two files have
    diverged for other reasons too, so line numbers do not line up. A literal
    only the base has is a candidate for something the re-base dropped.
    """
    a = {s for s in STRING.findall(theirs) if IDENTITY.search(s)}
    b = {s for s in STRING.findall(ours) if IDENTITY.search(s)}
    rows = [("(only in base)", s, "") for s in sorted(a - b)]
    rows += [("(only in tree)", "", s) for s in sorted(b - a)]
    return rows


def main() -> int:
    # Findings carry text from either tree; a cp1252 console must not decide
    # which of them are printable.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")

    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--root", default=".", help="repository root")
    ap.add_argument("--base", required=True, help="ref carrying the values we shipped")
    ap.add_argument("--mode", choices=["consts", "literals", "both"], default="both")
    ap.add_argument(
        "--path", default="", help="limit to paths starting with this prefix"
    )
    args = ap.parse_args()

    root = pathlib.Path(args.root)
    modes = ["consts", "literals"] if args.mode == "both" else [args.mode]

    for mode in modes:
        compare = compare_consts if mode == "consts" else compare_literals
        by_file: dict[str, list[tuple[str, str, str]]] = collections.defaultdict(list)
        for name in tracked_sources(root, args.base):
            if args.path and not name.startswith(args.path):
                continue
            p = root / name
            if not p.exists():
                continue
            try:
                ours = p.read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            theirs = git(root, "show", f"{args.base}:{name}")
            rows = [
                r
                for r in compare(theirs, ours)
                if IDENTITY.search(r[1]) or IDENTITY.search(r[2])
            ]
            if rows:
                by_file[name] = rows

        total = sum(len(v) for v in by_file.values())
        print(f"\n{'=' * 78}\n{mode}: {total} divergenze in {len(by_file)} file\n{'=' * 78}")
        for name in sorted(by_file):
            print(f"\n{name}")
            for key, theirs_v, ours_v in by_file[name]:
                print(f"    {key}")
                if theirs_v:
                    print(f"      base:   {theirs_v[:110]}")
                if ours_v:
                    print(f"      albero: {ours_v[:110]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
