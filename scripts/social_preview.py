#!/usr/bin/env python3
"""Build `assets/chutes/social-preview.png`, the 1280x640 card GitHub shows for links.

Without one, every link shared to Slack, X or Discord renders GitHub's generic
auto-card: repo name, owner avatar, description in a grey box. For a terminal
product the screenshot *is* the pitch, so the card is that screenshot, framed,
with the name and the line that distinguishes this fork.

Composed from `assets/chutes/screenshot/chutes-build.png` rather than generated: a
real screenshot is honest about what you get. The crop is to the splash's content —
the source has 166 rows of empty panel interior below the menu — which is a crop,
not a fabrication. Nothing in the terminal text is edited.

    python scripts/social_preview.py

Re-run it whenever the screenshot is re-shot. Uploading the result is manual:
GitHub exposes no API for the social preview, so it goes in
**Settings -> General -> Social preview**.

Requires Pillow (`pip install pillow`).
"""

from __future__ import annotations

import pathlib
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFont

WIDTH, HEIGHT = 1280, 640

# GitHub's dark canvas, so the card sits on its own ground rather than borrowing
# whatever the client puts behind it.
BACKGROUND = (13, 17, 23)
FOREGROUND = (230, 237, 243)
MUTED = (139, 148, 158)
ACCENT = (88, 166, 255)
FRAME = (48, 54, 61)

TITLE = "Chutes Build"
SUBTITLE = "Terminal AI coding agent for the Chutes ecosystem"
# The differentiators, in the order they matter to someone deciding whether to
# look further. All four are enforced, not aspirational — see PRIVACY.md.
TAGLINE = "No telemetry  ·  no phone-home  ·  no self-update  ·  Apache-2.0"

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
SCREENSHOT = REPO_ROOT / "assets/chutes/screenshot/chutes-build.png"
OUTPUT = REPO_ROOT / "assets/chutes/social-preview.png"

PAD = 60
# GitHub rejects a social preview above 1 MB.
MAX_BYTES = 1024 * 1024


def load_font(size: int, *, bold: bool = False) -> ImageFont.FreeTypeFont:
    """A real UI font where one exists, else Pillow's built-in."""
    candidates = (
        ["segoeuib.ttf", "arialbd.ttf", "DejaVuSans-Bold.ttf"]
        if bold
        else ["segoeui.ttf", "arial.ttf", "DejaVuSans.ttf"]
    )
    roots = [
        pathlib.Path("C:/Windows/Fonts"),
        pathlib.Path("/usr/share/fonts/truetype/dejavu"),
        pathlib.Path("/System/Library/Fonts"),
    ]
    for root in roots:
        for name in candidates:
            path = root / name
            if path.exists():
                return ImageFont.truetype(str(path), size)
    return ImageFont.load_default(size)


def splash_content(screenshot: Image.Image) -> Image.Image:
    """Crop to the splash's logo and menu, dropping its empty panel interior.

    Found by scanning for the longest run of rows holding nothing but the panel's
    two vertical borders, rather than hard-coding pixel offsets that a re-shot
    screenshot would silently invalidate.
    """
    background = Image.new("RGB", screenshot.size, screenshot.getpixel((0, 0)))
    mask = ImageChops.difference(screenshot, background).convert("L")
    per_row = [
        sum(1 for x in range(screenshot.width) if mask.getpixel((x, y)) > 10)
        for y in range(screenshot.height)
    ]

    content_rows = [y for y, count in enumerate(per_row) if count > 20]
    if not content_rows:
        raise SystemExit("social_preview: the screenshot looks blank")
    top = content_rows[0]

    # The longest run of near-empty rows *after* the first content is the panel's
    # interior; the menu ends where that run starts.
    longest = (0, 0)
    run_start: int | None = None
    for y in range(top, screenshot.height):
        if per_row[y] <= 6:
            run_start = y if run_start is None else run_start
        else:
            if run_start is not None and y - run_start > longest[1] - longest[0]:
                longest = (run_start, y)
            run_start = None
    bottom = longest[0] if longest[1] > longest[0] else content_rows[-1]

    bbox = mask.point(lambda v: 255 if v > 8 else 0).getbbox()
    left, right = (bbox[0], bbox[2]) if bbox else (0, screenshot.width)
    # A few rows of slack under the last menu item so it does not touch the frame.
    return screenshot.crop((left, top, right, min(bottom + 11, screenshot.height)))


def main() -> int:
    if not SCREENSHOT.exists():
        print(f"social_preview: {SCREENSHOT} is missing", file=sys.stderr)
        return 1

    card = Image.new("RGB", (WIDTH, HEIGHT), BACKGROUND)
    draw = ImageDraw.Draw(card)
    # A hairline accent along the top, so the card reads as designed rather than
    # as a bare paste.
    draw.rectangle([(0, 0), (WIDTH, 4)], fill=ACCENT)

    y = 70
    draw.text((PAD, y), TITLE, font=load_font(58, bold=True), fill=FOREGROUND)
    y += 78
    draw.text((PAD, y), SUBTITLE, font=load_font(27), fill=MUTED)
    y += 44
    draw.text((PAD, y), TAGLINE, font=load_font(22), fill=ACCENT)

    shot = splash_content(Image.open(SCREENSHOT).convert("RGB"))
    band_top = y + 60
    band_width = WIDTH - PAD * 2
    scaled = shot.resize(
        (band_width, round(shot.height * band_width / shot.width)), Image.LANCZOS
    )

    # One pixel of border lifts the terminal off the identical dark background.
    framed = Image.new("RGB", (scaled.width + 2, scaled.height + 2), FRAME)
    framed.paste(scaled, (1, 1))
    band_height = HEIGHT - band_top - 34
    offset = max(0, (band_height - framed.height) // 2)
    card.paste(framed, (PAD - 1, band_top - 1 + offset))

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    card.save(OUTPUT, optimize=True)
    size = OUTPUT.stat().st_size
    print(f"{OUTPUT.relative_to(REPO_ROOT).as_posix()}: {WIDTH}x{HEIGHT}, {size / 1024:.0f} KB")
    if size > MAX_BYTES:
        print("social_preview: over GitHub's 1 MB limit", file=sys.stderr)
        return 1
    print("Upload it at Settings -> General -> Social preview (no API for this).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
