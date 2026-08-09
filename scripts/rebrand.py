#!/usr/bin/env python3
"""Re-apply the Chutes Build identity on top of an upstream `grok-build` tree.

This exists because the fork is a re-base, not a one-off rename: every time we
move onto a newer upstream tree the same substitutions have to be re-applied,
and doing it by hand or with a loose `sed` is how an environment variable or a
state path silently stops resolving. A wrong rename here does not fail the
build — it fails at runtime, in production, quietly.

So the rules are explicit and the script is loud:

  * `RULES` are the substitutions we are certain about. They are applied.
  * `AMBIGUOUS` are tokens that mean the product in some places and the
    upstream harness in others (`grok-build` is both a binary name and an
    `agent_type`). They are never rewritten automatically — they are reported
    for a human to classify.
  * `MUST_NOT_APPEAR` is checked after applying. Anything left that is not
    covered by `ALLOWED_RESIDUAL` is an error, so a token we forgot cannot slip
    through unnoticed.

Usage:
    python scripts/rebrand.py            # dry run: report only, write nothing
    python scripts/rebrand.py --apply    # rewrite files in place
    python scripts/rebrand.py --check    # verify a tree is already clean

Derived empirically from the original rebrand commit `51f9fb8` plus a
comparison of upstream 1.0.0 against the fork; see `docs/upstream-sync.md`.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# Per-file decisions for the ambiguous tokens, derived by comparing upstream
# against the fork file by file: where the fork had renamed every occurrence the
# token means the product, where it had kept every occurrence it means the
# upstream harness or an identity we match against. Regenerate with
# `--reclassify` after a re-base; review the diff before trusting it.
DECISIONS_PATH = Path(__file__).resolve().parent / "rebrand_files.json"

# Applied only inside files listed under a token's `rename` set.
AMBIGUOUS_REPLACEMENTS: dict[str, tuple[re.Pattern[str], str]] = {
    "grok-build": (re.compile(r"\bgrok-build\b"), "chutes-build"),
    "grok": (
        re.compile(r"(?<![A-Za-z0-9_./-])grok(?![A-Za-z0-9_./-])"),
        "chutes-build",
    ),
    "x.ai": (re.compile(r"\bx\.ai\b"), "chutes.ai"),
    "grok.com": (re.compile(r"\bgrok\.com\b"), "chutes.ai"),
}

# Directories we never walk into.
SKIP_DIRS = {
    ".git",
    "target",
    "node_modules",
    "third_party",  # vendored upstream crates keep their own identity
    "output",
}

# Files that are already ours and must not be touched: they were written for
# Chutes Build in the first place, so a rule firing here means the rule is
# wrong.
SKIP_PREFIXES = (
    "crates/chutes-build-core/",
    "scripts/rebrand.py",
    # Our own records, which upstream does not have. They must be able to quote
    # the names they describe: the ACP-namespace rule rewrote a CHANGELOG
    # sentence explaining that very rename, leaving "`_chutes.build/…` survived
    # while the handlers answered on `chutes.build/`" — a sentence about nothing.
    # A record whose text a script rewrites is not a record.
    "CHANGELOG.md",
    "docs/upstream-sync.md",
)

TEXT_SUFFIXES = {
    ".rs",
    ".toml",
    ".md",
    ".json",
    ".yml",
    ".yaml",
    ".js",
    ".mjs",
    ".cjs",
    ".sh",
    ".ps1",
    ".txt",
    ".html",
}

# ── Substitutions we are certain about ────────────────────────────────────
#
# Order matters: longer, more specific patterns come first so a broader rule
# cannot eat their prefix.
#
# NOTE on identifiers: Rust identifiers such as `grok_home()`, `grok_dir`,
# `grok_application()` and the `xai_grok_*` crate paths are deliberately NOT
# renamed. The fork keeps them so upstream diffs stay reviewable — only the
# strings those functions *return* become Chutes. The same goes for the tool
# dialect module `implementations/grok_build/`.

RULES: list[tuple[str, re.Pattern[str], str]] = [
    # Environment variables, and the Rust statics that mirror their names.
    # 419 of upstream's 505 `GROK_*` variables follow this rule exactly.
    # Note the lookbehind rather than `\b`: an underscore is a word character,
    # so `\bGROK_` does not match inside `__GROK_INSIDE_BWRAP`,
    # `LC_GROK_OSC52_SINK` or `XAI_GROK_TEST_*`. Those are subprocess-boundary
    # markers — a shell state sentinel, an SSH-forwarded `LC_*` variable, a
    # bwrap re-entry flag — and both sides of each boundary are ours, so the
    # mismatch was invisible until the seam sweep compared them against the
    # names the fork actually shipped.
    ("env var", re.compile(r"(?<![A-Za-z0-9])GROK_(?=[A-Z0-9_])"), "CHUTES_BUILD_"),
    # User-facing product name, in prose and in UI strings.
    ("product name", re.compile(r"\bGrok Build\b"), "Chutes Build"),
    ("product type", re.compile(r"\bGrokBuild\b"), "ChutesBuild"),
    # State root. Only as a path component: `.grok/` or a quoted `".grok"`,
    # never the bare word, so `xai-grok-*` and prose survive untouched.
    ("state dir", re.compile(r"(?<![\w.-])\.grok(?=[/\\\"'\s])"), ".chutes-build"),
    ("home tilde", re.compile(r"~/\.grok\b"), "~/.chutes-build"),
    # A sibling state directory, not the state root: the `state dir` rule above
    # requires a separator after `.grok` and a hyphen is not one. Kept as its
    # own rule so the general rule's guard against `.grok_*` and `xai-grok-`
    # stays as tight as it is.
    ("snapshot dir", re.compile(r"(?<![\w.-])\.grok-snapshots\b"), ".chutes-build-snapshots"),
    # Theme display labels, before the general product-word rule below could
    # turn them into "Chutes Build Night".
    ("theme label", re.compile(r"\bGrok Night\b"), "Chutes Night"),
    ("theme label", re.compile(r"\bGrok Day\b"), "Chutes Day"),
    # The bare product word in user-facing text: CLI help, error messages,
    # crash-report headers. This was the gap behavioural verification found —
    # the gate was green while `--help` still read "Run Grok without the
    # interactive UI".
    #
    # Excluded: a following version or model token, because `Grok 4.5` and
    # `Grok Code Fast` name upstream *models* this build can still route to;
    # `SuperGrok`, an upstream subscription this build only detects in order to
    # suppress it for API-key users; and the `GrokAuth` / `GrokCom` types, whose
    # names are load-bearing in the auth layer.
    # Also excluded: anything in a path or field position (`SourceFilter::Grok`,
    # `.Grok`), because the replacement contains a space and would not even parse.
    (
        "product word",
        re.compile(
            r"(?<!Super)(?<![:.])\bGrok\b(?![-_ ]?(?:\d|Build\b|Code\b|Auth\b|Com\b))"
        ),
        "Chutes Build",
    ),
    # The Unix diagnostic prefix, `progname: message`. It names the binary, so it
    # has to be the binary that exists — `grok: ignoring …` on stderr of a
    # program called `chutes-build` is a message from nowhere. Anchored on the
    # colon-space and a line- or quote-start so it cannot touch a module path
    # (`grok::`) or prose.
    ("diagnostic prefix", re.compile(r'(?<=")grok: '), "chutes-build: "),
    # The state root in prose: "the grok home". Lowercase, so the product-word
    # rule above does not reach it.
    ("home phrase", re.compile(r"\bgrok home\b"), "Chutes Build home"),
    # Bundled themes.
    ("theme type", re.compile(r"\bGrokNight\b"), "ChutesNight"),
    ("theme type", re.compile(r"\bGrokDay\b"), "ChutesDay"),
    ("theme id", re.compile(r"\bgroknight\b"), "chutesnight"),
    ("theme id", re.compile(r"\bgrokday\b"), "chutesday"),
    # ACP extension-method namespace. `x.ai` as a hostname is ambiguous and left
    # to a human, but the `_x.ai/` form is never a host: it is the reverse-DNS
    # prefix of an extension method, and both ends of every one of those calls
    # are ours. Renaming only some of them is how 102 pager tests failed with
    # zero compile errors — the handler answered on one name while the callers
    # used another. Anchored on the underscore so the hostname stays ambiguous.
    ("acp namespace", re.compile(r"_x\.ai/"), "_chutes.build/"),
    # The ambient API-key variable, and the auth-method id users write in
    # `config.toml` as `preferred_method`. Both are Chutes-side: the key is a
    # `cpk_` token for `llm.chutes.ai`, and the method it selects is the Chutes
    # one. The re-base left `XAI_API_KEY` / `xai.api_key` while every message and
    # every doc said `CHUTES_API_KEY`, so a user following the instructions was
    # simply never authenticated — found by trying it with a real key, not by any
    # test. The legacy fallback name follows the fork's, so an existing
    # deployment keeps working.
    ("api key env", re.compile(r"\bXAI_API_KEY\b"), "CHUTES_API_KEY"),
    (
        "api key env legacy",
        re.compile(r"\bCHUTES_BUILD_CODE_XAI_API_KEY\b"),
        "CHUTES_BUILD_API_KEY",
    ),
    ("api key ident", re.compile(r"\bXAI_API_KEY_ENV_VAR\b"), "CHUTES_API_KEY_ENV_VAR"),
    (
        "api key ident",
        re.compile(r"\bLEGACY_XAI_API_KEY_ENV_VAR\b"),
        "LEGACY_CHUTES_API_KEY_ENV_VAR",
    ),
    ("api key method", re.compile(r"\bxai\.api_key\b"), "chutes.api_key"),
    ("api key fn", re.compile(r"\b(read|has)_xai_api_key_env\b"), r"\1_chutes_api_key_env"),
    # The CLI invocation in prose: `grok login`, `grok doctor`, `grok wrap ...`.
    # An instruction naming a binary that does not exist is worse than no
    # instruction, and there were 570 of them across help text, error messages,
    # the bundled user guide and doc comments. Anchored on a following
    # subcommand or flag so the bare word (handled by `product word`, and by the
    # per-file decisions for lowercase `grok`) is not touched here, and on a
    # non-identifier character before, so `xai-grok-…`, `grok_home` and
    # `implementations/grok_build/` survive.
    (
        "cli invocation",
        re.compile(
            r"(?<![A-Za-z0-9_./-])grok (?=(?:login|logout|setup|update|models|doctor|trace|sessions|memory|mcp|plugin|worktree|export|inspect|du|disk-usage|dashboard|wrap|completions|agent|leader|version)\b|-[A-Za-z-]|\{[a-z_]+\})"
        ),
        "chutes-build ",
    ),
    # macOS MDM managed-preferences domain.
    ("mdm domain", re.compile(r"\bai\.x\.grok\b"), "ai.x.chutes-build"),
    # Plugin namespace. Anchored on a non-`xai-` prefix: without that guard it
    # also fires inside the crate name `xai-grok-plugin-marketplace`, renaming a
    # workspace member and breaking the manifest — which is how this guard came
    # to exist.
    ("plugin ns", re.compile(r"(?<!xai-)\bgrok-plugin\b"), "chutes-build-plugin"),
]

# `CHUTES_EXTRA_CA_BUNDLE` is deliberately outside the `CHUTES_BUILD_` family:
# it configures the Chutes transport, not the product shell. The env-var rule
# above would have produced `CHUTES_BUILD_EXTRA_CA_BUNDLE`, so undo it.
POST_RULES: list[tuple[str, re.Pattern[str], str]] = [
    (
        "extra CA",
        re.compile(r"\bCHUTES_BUILD_EXTRA_CA_BUNDLE\b"),
        "CHUTES_EXTRA_CA_BUNDLE",
    ),
]

# ── Tokens a human must classify ──────────────────────────────────────────
#
# `grok-build` means two different things upstream and the fork keeps 399 of
# its occurrences: harness `agent_type` values ("grok-build", "grok-build-plan"),
# prompt-template labels, and model slugs (`grok-build-0.1`) all stay, while the
# binary name, the npm package and install paths become `chutes-build`. Only a
# reader can tell which is which, so they are reported, never rewritten.
#
# Hostnames are here for the same reason, and it is not a formality: the fork's
# `sampling/error.rs` matches the literal `grok.com/supergrok` on the way *in*,
# to recognise an upsell the upstream server sends and suppress it for API-key
# users. Rewriting that string would not fail a test — it would quietly stop
# matching, and team users would start seeing a personal-subscription pitch.
# Outbound URLs we present and inbound strings we match against look identical
# to a regex, so neither hostname is ever rewritten mechanically.
AMBIGUOUS: list[tuple[str, re.Pattern[str]]] = [
    ("grok-build", re.compile(r"\bgrok-build\b")),
    ("grok", re.compile(r"(?<![A-Za-z0-9_./-])grok(?![A-Za-z0-9_./-])")),
    ("grok.com", re.compile(r"\bgrok\.com\b")),
    ("x.ai", re.compile(r"\bx\.ai\b")),
]

# ── What must be gone once the rules have run ─────────────────────────────
MUST_NOT_APPEAR: list[tuple[str, re.Pattern[str]]] = [
    ("GROK_ env var", re.compile(r"\bGROK_[A-Z0-9_]+")),
    ("Grok Build", re.compile(r"\bGrok Build\b")),
    ("GrokBuild", re.compile(r"\bGrokBuild\b")),
    ("GrokNight/GrokDay", re.compile(r"\bGrok(Night|Day)\b")),
    (".grok state dir", re.compile(r"(?<![\w.-])\.grok(?=[/\\\"'\s])")),
]

# Occurrences allowed to remain, with the reason. Checked as substrings of the
# matching line.
ALLOWED_RESIDUAL: list[tuple[str, str]] = [
    ("historic_bash_cmds.txt", "recorded-history test fixture, not live config"),
]


def is_text_file(path: Path) -> bool:
    return path.suffix in TEXT_SUFFIXES


def walk(root: Path):
    for path in sorted(root.rglob("*")):
        if not path.is_file() or not is_text_file(path):
            continue
        rel = path.relative_to(root).as_posix()
        if any(part in SKIP_DIRS for part in path.relative_to(root).parts):
            continue
        if rel.startswith(SKIP_PREFIXES):
            continue
        yield path, rel


# `chutes-build` is not a Rust identifier and `Chutes Build` contains a space, so
# either landing in code position is always a mistake. A per-file rename decision
# cannot tell a local variable from a product mention, so the rules run first and
# this undoes the damage.
#
# It matches the damage *shapes* rather than trying to decide what is inside a
# string literal. Two earlier versions did the latter, with a per-line quote
# scanner, and both mangled multi-line strings: a `"…\` continuation or an
# `r#"…"#` fixture looks like code to any scanner that starts fresh at each line,
# so `~/.chutes-build/config.toml` in a help message got "repaired" back to
# `~/.grok`. These patterns cannot fire inside prose because none of them is
# valid English either.
#
# Keeping the repair inside the script is what makes re-running idempotent; the
# first version was a separate throwaway pass and the next run of the rules
# silently undid its work.
# What disqualifies a `chutes-build` match from being a damaged Rust identifier:
# a path separator, another word character, a hyphen — or a quote, because inside
# a string literal there is no identifier to repair. That last one is not
# theoretical: without it `"chutes-build.md"` matched `chutes-build` followed by
# `.`, the repair read it as field access, and a filename silently went back to
# `"grok.md"` — reverting a fix whose own comment was still sitting above it.
NOT_A_DAMAGED_IDENT = """(?<![./\\\\\\w'\"-])"""

IDENTIFIER_REPAIRS: list[tuple[re.Pattern[str], str]] = [
    # `let chutes-build = …`, including `mut` and tuple-destructuring positions.
    (re.compile(r"\blet (mut )?chutes-build\b"), r"let \1grok"),
    (re.compile(r"(\blet \([^)]*?)\bchutes-build\b"), r"\1grok"),
    # Uses: `&chutes-build`, `chutes-build.foo()`, `chutes-build =`, `(… chutes-build …)`.
    #
    # The lookbehind is load-bearing: without it `(~/.chutes-build)` in a help
    # string matches `chutes-build` followed by `)` and gets "repaired" to
    # `~/.grok`. The rules then rewrite it back on the next run, and the two
    # oscillate — the script reports "0 files changed" while the tree sits in the
    # wrong state, which is a worse failure than a loud one.
    # The quote in the lookbehind is what stops this from eating a filename:
    # `"chutes-build.md"` matched `chutes-build` followed by `.`, read that as
    # field access, and silently rewrote the string back to `grok.md` — undoing
    # a fix whose own comment was still sitting above it. Inside a string
    # literal there is no identifier to repair.
    (re.compile(NOT_A_DAMAGED_IDENT + r"&chutes-build\b"), "&grok"),
    (re.compile(NOT_A_DAMAGED_IDENT + r"chutes-build(?=\s*[.=,)])"), "grok"),
    # Path and field position: `SourceFilter::Chutes Build`, `Self::Chutes Build`.
    (re.compile(r"(::)Chutes Build\b"), r"\1Grok"),
    # Enum variant *declarations* — `    Chutes Build,` on its own line inside an
    # enum body. The product-word rule's `(?<![:.])` guard cannot see these,
    # because a declaration has no path prefix. This is the honest limit of
    # renaming a bare capitalised word: `Grok` is both a product name in prose
    # and a type name in Rust, and only position tells them apart.
    (re.compile(r"(?m)^(\s*)Chutes Build(\s*[,{=])"), r"\1Grok\2"),
    # A doc link to that variant.
    (re.compile(r"\[`(Self|[A-Z]\w*)::Chutes Build`\]"), r"[`\1::Grok`]"),
]


def repair_identifiers(text: str, rel: str) -> tuple[str, int]:
    if not rel.endswith(".rs"):
        return text, 0
    fixed = 0
    for pattern, replacement in IDENTIFIER_REPAIRS:
        text, n = pattern.subn(replacement, text)
        fixed += n
    return text, fixed


def load_decisions() -> dict[str, dict[str, list[str]]]:
    if not DECISIONS_PATH.exists():
        return {}
    return json.loads(DECISIONS_PATH.read_text(encoding="utf-8"))


def apply_rules(
    text: str, rel: str, decisions: dict[str, dict[str, list[str]]]
) -> tuple[str, Counter]:
    hits: Counter = Counter()
    for name, pattern, replacement in RULES:
        text, n = pattern.subn(replacement, text)
        if n:
            hits[name] += n
    for token, (pattern, replacement) in AMBIGUOUS_REPLACEMENTS.items():
        if rel in set(decisions.get(token, {}).get("rename", ())):
            text, n = pattern.subn(replacement, text)
            if n:
                hits[f"{token} (scoped)"] += n
    for name, pattern, replacement in POST_RULES:
        text, n = pattern.subn(replacement, text)
        if n:
            hits[name] += n
    text, repaired = repair_identifiers(text, rel)
    if repaired:
        hits["identifier repair"] += repaired
    return text, hits


def residual_is_allowed(rel: str, line: str) -> bool:
    return any(marker in rel or marker in line for marker, _ in ALLOWED_RESIDUAL)


def assert_no_control_bytes() -> None:
    """Fail loudly if this file carries a mangled escape.

    Three times now, an edit routed through a shell heredoc has collapsed a
    word-boundary escape to a literal 0x08 byte, and a capture reference to 0x01.
    The rule then matches nothing, or the replacement drops its capture, while
    reading perfectly in a diff — the worst failure mode this script has: silent,
    and indistinguishable from a rule with no work to do. Prefer the explicit
    character-class lookarounds the API-key rules use; they say the same thing
    and survive any round-trip.
    """
    raw = Path(__file__).read_bytes()
    bad = {b for b in raw if b < 9 or 10 < b < 32}
    if bad:
        names = ", ".join(f"0x{b:02x}" for b in sorted(bad))
        raise SystemExit(
            f"rebrand.py contains control bytes ({names}). An escape was eaten in "
            "transit — re-check the RULES table before trusting a run."
        )


def main() -> int:
    assert_no_control_bytes()

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--apply", action="store_true", help="rewrite files (default: dry run)"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="only verify the tree is clean; no rules applied",
    )
    parser.add_argument("--root", default=str(REPO_ROOT), help="tree to operate on")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    decisions = load_decisions()
    review: Counter = Counter()
    totals: Counter = Counter()
    changed_files = 0
    ambiguous: Counter = Counter()
    ambiguous_files: dict[str, set[str]] = {}
    residual: list[tuple[str, str, str]] = []

    for path, rel in walk(root):
        try:
            original = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue

        text = original
        if not args.check:
            text, hits = apply_rules(text, rel, decisions)
            # Count only rules that survived into the written file. A rule and
            # the identifier repair can cancel each other out within one pass —
            # the tree converges, which is what matters, but a counter that
            # reports 22 renames on a file it did not change sends the next
            # reader looking for a bug in the wrong place.
            if text != original:
                totals.update(hits)
                changed_files += 1
                if args.apply:
                    path.write_text(text, encoding="utf-8", newline="\n")

        for name, pattern in AMBIGUOUS:
            n = len(pattern.findall(text))
            if n:
                ambiguous[name] += n
                ambiguous_files.setdefault(name, set()).add(rel)
                if rel in set(decisions.get(name, {}).get("review", ())):
                    review[name] += n

        for name, pattern in MUST_NOT_APPEAR:
            for line in text.splitlines():
                if pattern.search(line) and not residual_is_allowed(rel, line):
                    residual.append((name, rel, line.strip()[:110]))

    mode = "CHECK" if args.check else ("APPLY" if args.apply else "DRY RUN")
    print(f"=== rebrand: {mode} on {root} ===\n")

    if not args.check:
        print("Applied rules:")
        for name, count in totals.most_common():
            print(f"  {name:<16} {count:>7}")
        print(f"\n  files changed: {changed_files}\n")

    print("Ambiguous - remaining occurrences (kept by decision, or unreviewed):")
    for name, count in ambiguous.most_common():
        files = len(ambiguous_files.get(name, ()))
        pending = review.get(name, 0)
        note = f"  [{pending} still unreviewed]" if pending else ""
        print(f"  {name:<20} {count:>7} occurrences in {files} files{note}")

    if residual:
        print(f"\nERROR: {len(residual)} occurrences that must not remain:\n")
        for name, rel, line in residual[:40]:
            print(f"  [{name}] {rel}\n      {line}")
        if len(residual) > 40:
            print(f"  … and {len(residual) - 40} more")
        return 1

    print("\nNo forbidden token remains.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
