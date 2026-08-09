#!/usr/bin/env python3
"""List Rust source files that no `mod` declaration reaches.

Such a file is not compiled. Nothing warns, no test can cover it, and `cargo
build` is perfectly happy — so a feature copied in but never registered looks
exactly like a feature that works. The 1.0.0 re-base hit this four times:
`pcm.rs` and `stt/batch.rs` (batch STT, the only transport Chutes serves),
`slash/commands/apikey.rs` (login with the primary credential), and
`slash/commands/advisor.rs`.

Run after any phase that copies files between trees:

    python scripts/dead_modules.py

Exits 1 if anything is listed. `--allow <name>` tolerates a file by basename,
for the rare case of a source intentionally kept unbuilt.

A file counts as reached if any `.rs` in the tree declares `mod <stem>`, or names
it in a `#[path = "..."]` attribute or an `include!("...")`. Both forms are used
here — `session/acp_session.rs` attaches every `acp_session_impl/*.rs` by path,
and most `*_tests.rs` files are attached the same way — so a sweep that only
looked at sibling `mod.rs` files would report dozens of false positives and be
ignored, which is worse than not having one.

Excluded: `src/bin/**` (Cargo discovers those), `tests/`, `benches/`,
`examples/`, `fuzz/`, and the crate roots `lib.rs` / `main.rs` / `mod.rs` /
`build.rs`.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

MOD_DECL = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*[;{]", re.M
)
PATH_ATTR = re.compile(r'#\[path\s*=\s*"([^"]+)"\]')
INCLUDE = re.compile(r'include!\s*\(\s*"([^"]+)"')

SKIP_DIRS = {"target", "tests", "benches", "examples", "fuzz"}
CRATE_ROOTS = {"lib.rs", "main.rs", "mod.rs", "build.rs"}


def basename(spec: str) -> str:
    return spec.replace("\\", "/").rsplit("/", 1)[-1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        default="crates",
        help="directory to sweep (default: crates)",
    )
    parser.add_argument(
        "--allow",
        action="append",
        default=[],
        metavar="FILE",
        help="basename to tolerate; repeatable",
    )
    args = parser.parse_args()

    root = pathlib.Path(args.root)
    if not root.is_dir():
        print(f"dead_modules: {root} is not a directory", file=sys.stderr)
        return 2

    sources = [
        f
        for f in root.rglob("*.rs")
        if not SKIP_DIRS.intersection(f.parts)
    ]

    declared: set[str] = set()
    attached: set[str] = set()
    for f in sources:
        text = f.read_text(encoding="utf-8", errors="replace")
        declared.update(MOD_DECL.findall(text))
        for pattern in (PATH_ATTR, INCLUDE):
            attached.update(basename(spec) for spec in pattern.findall(text))

    allowed = set(args.allow)
    dead = [
        f
        for f in sources
        if f.name not in CRATE_ROOTS
        and "bin" not in f.parts
        and f.name not in allowed
        and f.stem not in declared
        and f.name not in attached
    ]

    if not dead:
        print(f"dead_modules: {len(sources)} files swept, all reachable")
        return 0

    print(f"dead_modules: {len(dead)} file(s) no `mod` declaration reaches:\n")
    for f in sorted(dead):
        print(f"  {f.as_posix()}")
    print(
        "\nEach is dead code: not compiled, so no warning and no test can reach it.\n"
        "Declare it in the owning module, or delete it. If a file is deliberately\n"
        "unbuilt, pass --allow <basename> and say why in the call site."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
