#!/usr/bin/env python3
"""Compare a crate's failing tests against a committed per-platform baseline.

Two crates fail a lot of tests on Windows and always have: 69 in
`xai-grok-tools`, 53 in `xai-grok-workspace`, all of them pre-dating the 1.0.0
re-base. CI worked around that by running narrow filters
(`implementations::chutes::`), which means those 122 tests are neither passing
nor watched — a *new* failure among them would look exactly like the other 122.

This makes the count a gate instead of a shrug. The baseline names every test
known to fail on a platform; a failure not in it fails the build, and a test in
it that starts passing also fails the build, because a baseline nobody prunes
stops describing anything.

Usage:
    python scripts/known_failures.py --crate xai-grok-tools           # check
    python scripts/known_failures.py --crate xai-grok-tools --record  # write

Baselines live in `.github/known-failures/<platform>.txt`, one
`crate<TAB>test::path` per line. A platform with no baseline file is reported
loudly and does not fail the build: that is the bootstrap state, and the run
prints exactly what to commit.
"""

from __future__ import annotations

import argparse
import platform
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BASELINE_DIR = REPO_ROOT / ".github" / "known-failures"

# `cargo test` prints a `failures:` block with each failure's panic text, then a
# second one that is a clean list of names. Both are indented four spaces, so
# anchor on the section header and take whatever lines in it look like a test
# path; the panic block contributes nothing that matches.
FAILURES_HEADER = re.compile(r"^failures:$")
# No `::` required: a `#[test]` at a lib's crate root has a bare name, and
# demanding a module path would make exactly such a failure invisible to the
# gate — the hole this script exists to close.
TEST_PATH = re.compile(r"^    ([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z0-9_]+)*)$")


def platform_name() -> str:
    system = platform.system().lower()
    return {"darwin": "macos", "windows": "windows"}.get(system, system)


def baseline_path(name: str) -> Path:
    return BASELINE_DIR / f"{name}.txt"


def load_baseline(name: str) -> set[tuple[str, str]] | None:
    path = baseline_path(name)
    if not path.exists():
        return None
    entries: set[tuple[str, str]] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        crate, _, test = line.partition("\t")
        if test:
            entries.add((crate, test))
    return entries


def run_suite(crate: str) -> set[str]:
    """Run `cargo test -p <crate> --lib` and return the set of failing tests."""
    result = subprocess.run(
        ["cargo", "test", "-p", crate, "--lib", "--locked"],
        cwd=REPO_ROOT,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    output = (result.stdout or "") + (result.stderr or "")
    if "test result:" not in output:
        # The suite did not run at all — a compile error, a missing crate, a
        # linker failure. Reporting "no failures" here would turn a broken build
        # into a green check, which is the exact failure mode this script exists
        # to remove.
        sys.stderr.write(output[-4000:])
        raise SystemExit(f"known_failures: `{crate}` never produced a test result")

    failing: set[str] = set()
    in_block = False
    for line in output.splitlines():
        if FAILURES_HEADER.match(line):
            in_block = True
            continue
        if in_block:
            match = TEST_PATH.match(line)
            if match:
                failing.add(match.group(1))
            elif line.strip():
                in_block = False
    return failing


def render(entries: set[tuple[str, str]]) -> str:
    header = (
        "# Tests known to fail on this platform, checked by\n"
        "# `scripts/known_failures.py`. A failure not listed here fails CI; an\n"
        "# entry here that starts passing also fails CI, so the file cannot rot.\n"
        "# Regenerate with `--record` and read the diff before committing it.\n"
    )
    body = "\n".join(f"{crate}\t{test}" for crate, test in sorted(entries))
    return f"{header}{body}\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--crate", action="append", required=True, help="crate to run")
    parser.add_argument("--record", action="store_true", help="write the baseline")
    parser.add_argument("--platform", default=platform_name(), help="baseline name")
    args = parser.parse_args()

    observed: set[tuple[str, str]] = set()
    for crate in args.crate:
        for test in run_suite(crate):
            observed.add((crate, test))

    # The baseline covers every crate; this run covers the ones named. Compare
    # and record only within those, or a single-crate run reports the other
    # crate's entries as newly passing and `--record` deletes them — which the
    # docstring's own one-crate example would have done.
    ran = set(args.crate)
    existing = load_baseline(args.platform) or set()
    untouched = {entry for entry in existing if entry[0] not in ran}

    if args.record:
        BASELINE_DIR.mkdir(parents=True, exist_ok=True)
        baseline_path(args.platform).write_text(
            render(observed | untouched), encoding="utf-8"
        )
        print(
            f"known_failures: recorded {len(observed)} for {args.platform} "
            f"({len(untouched)} kept for crates not run)"
        )
        return 0

    baseline = load_baseline(args.platform)
    if baseline is not None:
        baseline = {entry for entry in baseline if entry[0] in ran}
    if baseline is None:
        print(f"known_failures: no baseline for {args.platform!r} yet.")
        print(f"  {len(observed)} failing test(s) observed. To adopt them, commit:")
        print(f"  {baseline_path(args.platform).relative_to(REPO_ROOT)}")
        for crate, test in sorted(observed):
            print(f"    {crate}\t{test}")
        return 0

    new = sorted(observed - baseline)
    fixed = sorted(baseline - observed)

    for crate, test in new:
        print(f"NEW FAILURE  {crate}  {test}")
    for crate, test in fixed:
        print(f"NOW PASSING  {crate}  {test}")

    if new:
        print(f"\n{len(new)} test(s) fail that the baseline does not know about.")
        print("Re-run before concluding: some of these suites do flake under")
        print("load, and one appeared and vanished within three runs while this")
        print("script was being written. If it reproduces it is a regression —")
        print("fix it, or justify the entry in the commit message before")
        print("adding it with --record.")
    if fixed:
        print(f"\n{len(fixed)} baselined test(s) now pass. Re-record so the file")
        print("keeps describing reality — a stale baseline hides the next one.")
    if not new and not fixed:
        print(f"known_failures: {len(observed)} failing, all expected ({args.platform})")

    return 1 if (new or fixed) else 0


if __name__ == "__main__":
    raise SystemExit(main())
