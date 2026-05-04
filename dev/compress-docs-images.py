"""Compress docs/images/*.png in-place: resize to fit a max edge + re-encode
with optimized PNG compression. Used once before publishing the docs site so
gh-pages doesn't carry tens of MB of full-resolution screenshots.

Run from repo root with the docs venv activated:
    python dev/compress-docs-images.py
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image

IMAGES_DIR = Path("docs/images")
MAX_EDGE = 1600  # widest dimension after resize, in px
SKIP_FILES = {"icon.png", "favicon.png"}  # logos shipped at native resolution


def compress(path: Path) -> tuple[int, int]:
    before = path.stat().st_size
    with Image.open(path) as img:
        # Drop animation/multi-frame just in case (Pillow handles GIF/WebP);
        # we only ship single-frame PNGs.
        img.load()
        # Convert palette / RGBA images sanely. Keep alpha if it's used.
        if img.mode not in ("RGB", "RGBA"):
            img = img.convert("RGBA" if "A" in img.getbands() else "RGB")

        # Resize so the longest edge equals MAX_EDGE — preserves aspect ratio.
        # Skip if already smaller; we don't want to upscale.
        w, h = img.size
        longest = max(w, h)
        if longest > MAX_EDGE:
            scale = MAX_EDGE / longest
            new_size = (round(w * scale), round(h * scale))
            img = img.resize(new_size, Image.LANCZOS)

        # `optimize=True` runs the deflate compression a few extra passes —
        # cheap, and shaves ~10-30 % off most PNGs.
        img.save(path, format="PNG", optimize=True, compress_level=9)

    after = path.stat().st_size
    return before, after


def main() -> None:
    if not IMAGES_DIR.is_dir():
        raise SystemExit(f"docs/images not found: {IMAGES_DIR.resolve()}")

    total_before = 0
    total_after = 0
    print(f"{'file':40} {'before':>10} {'after':>10} {'gain':>6}")
    print("-" * 70)
    for path in sorted(IMAGES_DIR.glob("*.png")):
        if path.name in SKIP_FILES:
            continue
        before, after = compress(path)
        total_before += before
        total_after += after
        gain = (1 - after / before) * 100 if before else 0
        print(f"{path.name:40} {before/1024:9.0f}K {after/1024:9.0f}K  {gain:5.0f}%")

    print("-" * 70)
    gain = (1 - total_after / total_before) * 100 if total_before else 0
    print(
        f"{'TOTAL':40} "
        f"{total_before/1024/1024:8.1f}MB {total_after/1024/1024:8.1f}MB  {gain:5.0f}%"
    )


if __name__ == "__main__":
    main()
