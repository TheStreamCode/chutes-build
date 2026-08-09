#!/usr/bin/env python3
"""Render `assets/chutes/social-preview.png`, the 1280x640 card GitHub shows for links.

Without one, a repository link shared to Slack, X or Discord renders GitHub's generic
auto-card: repo name, owner avatar, description in a grey box.

The source is `assets/chutes/social-preview.html` — the card is designed in CSS and
screenshotted, not assembled from bitmaps. That buys real typography and a layout that
stays balanced when a line changes, and it means the card is reviewable as a diff.

    python scripts/social_preview.py

Uploading it stays manual: GitHub exposes no API for the social preview, so it goes in
**Settings -> General -> Social preview**.

Requires `playwright-cli` on PATH (`npm i -g @playwright/cli`).
"""

from __future__ import annotations

import contextlib
import functools
import http.server
import pathlib
import shutil
import subprocess
import sys
import threading

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
ASSETS = REPO_ROOT / "assets" / "chutes"
SOURCE = ASSETS / "social-preview.html"
OUTPUT = ASSETS / "social-preview.png"

WIDTH, HEIGHT = 1280, 640
# GitHub rejects a social preview above 1 MB.
MAX_BYTES = 1024 * 1024
# Chromium blocks `file:` URLs under automation, so the card is served for the
# duration of the render. Loopback only.
PORT = 8731


def serve(directory: pathlib.Path) -> http.server.ThreadingHTTPServer:
    handler = functools.partial(
        http.server.SimpleHTTPRequestHandler, directory=str(directory)
    )
    # Quiet: one GET per render is not worth a log line.
    handler.log_message = lambda *_args, **_kwargs: None  # type: ignore[method-assign]
    server = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def run(*args: str) -> None:
    # Resolve the executable: on Windows `playwright-cli` is a `.cmd` shim that
    # `shutil.which` finds but `CreateProcess` will not, given the bare name.
    exe = shutil.which("playwright-cli")
    if exe is None:
        raise SystemExit("social_preview: playwright-cli vanished from PATH mid-run")
    result = subprocess.run(
        [exe, *args], capture_output=True, text=True, encoding="utf-8"
    )
    if result.returncode != 0:
        raise SystemExit(
            f"playwright-cli {' '.join(args)} failed:\n{result.stderr or result.stdout}"
        )


def main() -> int:
    if not SOURCE.exists():
        print(f"social_preview: {SOURCE} is missing", file=sys.stderr)
        return 1
    if shutil.which("playwright-cli") is None:
        print(
            "social_preview: playwright-cli not on PATH — `npm i -g @playwright/cli`",
            file=sys.stderr,
        )
        return 1

    server = serve(ASSETS)
    try:
        run("open")
        run("resize", str(WIDTH), str(HEIGHT))
        run("goto", f"http://127.0.0.1:{PORT}/{SOURCE.name}")
        run("screenshot", "--filename", str(OUTPUT))
    finally:
        server.shutdown()
        with contextlib.suppress(SystemExit):
            run("close")

    if not OUTPUT.exists():
        print("social_preview: playwright-cli wrote no file", file=sys.stderr)
        return 1

    size = OUTPUT.stat().st_size
    print(
        f"{OUTPUT.relative_to(REPO_ROOT).as_posix()}: "
        f"{WIDTH}x{HEIGHT}, {size / 1024:.0f} KB"
    )
    if size > MAX_BYTES:
        print("social_preview: over GitHub's 1 MB limit", file=sys.stderr)
        return 1
    print("Upload it at Settings -> General -> Social preview (no API for this).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
