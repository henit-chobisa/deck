#!/usr/bin/env python3
"""Re-export the reel from the original recordings, not the reduced WebPs."""
import argparse
from pathlib import Path
import subprocess
import tempfile
from PIL import Image, ImageDraw

# Crops keep the native Retina pixels and remove the recording's black margin.
# The tiny recorder badge is outside the title; covering it must not erase text.
SEGMENTS = [
    ("10.21.51", 2, 46, 3, 0, "2374:1896:114:78"),
    ("8.19.22", 40, 32, 3, 138, "2326:1906:114:78"),
    ("7.02.19", 98, 18, 4, 234, "2326:1906:114:80"),
    ("7.02.19", 132, 18, 3, 306, "2326:1906:114:80"),
]


def remove_badge(image):
    # Two rectangles stay inside the rounded orange window border. The badge
    # ends just above the title; a taller mask would erase the first text row.
    band = image.getpixel((500, 20))
    draw = ImageDraw.Draw(image)
    draw.rectangle((12, 14, 143, 49), fill=band)
    draw.rectangle((20, 6, 143, 49), fill=band)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("recordings", type=Path, help="Directory containing the October 5 original .mov files")
    parser.add_argument("--output", type=Path, required=True, help="Output directory for l/ and h/; publish as site-frames/retina on pr-assets")
    args = parser.parse_args()
    for tier in ("l", "h"):
        (args.output / tier).mkdir(parents=True, exist_ok=True)
    for clock, start, duration, fps, offset, crop in SEGMENTS:
        matches = list(args.recordings.glob(f"*2026-10-05 at {clock}*PM.mov"))
        if len(matches) != 1:
            raise SystemExit(f"Expected one recording for {clock}, found {matches}")
        # Homebrew's FFmpeg need not include a WebP encoder. PNG is a lossless
        # hand-off to Pillow, so the installed codec set cannot lower quality.
        with tempfile.TemporaryDirectory(prefix="deck-frames-") as temp:
            subprocess.run([
                "ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
                "-ss", str(start), "-t", str(duration), "-i", str(matches[0]),
                "-vf", f"fps={fps},crop={crop}",
                "-frames:v", str(duration * fps), "-start_number", str(offset),
                str(Path(temp) / "%03d.png"),
            ], check=True)
            for path in sorted(Path(temp).glob("*.png")):
                with Image.open(path) as original:
                    image = original.convert("RGB")
                    remove_badge(image)
                    image.save(args.output / "h" / (path.stem + ".webp"), lossless=True, method=4)
                    small = image.resize((1200, round(image.height * 1200 / image.width)), Image.Resampling.LANCZOS)
                    small.save(args.output / "l" / (path.stem + ".webp"), lossless=True, method=4)
    for tier in ("l", "h"):
        files = list((args.output / tier).glob("*.webp"))
        if len(files) != 360:
            raise SystemExit(f"Expected 360 {tier} frames, found {len(files)}")
        print(f"{tier}: 360 lossless frames, {sum(p.stat().st_size for p in files) / 1024**2:.1f} MiB")


if __name__ == "__main__":
    main()
