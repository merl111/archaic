#!/usr/bin/env python3
"""Export native icon formats from the approved raster master. Requires Pillow."""
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
BRAND = ROOT / "assets/branding"
SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)


def main():
    master = Image.open(BRAND / "icon-master.png").convert("RGBA")
    if master.width != master.height or master.getchannel("A").getextrema()[0] != 0:
        raise ValueError("Expected a square transparent icon master")
    for size in SIZES:
        icon = master.resize((size, size), Image.Resampling.LANCZOS)
        icon.save(BRAND / f"icon-{size}.png")
        if size in (32, 256):
            (BRAND / f"icon-{size}.rgba").write_bytes(icon.tobytes())
    master.save(BRAND / "archaic.ico", sizes=[(s, s) for s in SIZES if s <= 256])
    master.resize((1024, 1024), Image.Resampling.LANCZOS).save(BRAND / "archaic.icns")
    print("Exported PNG sizes, embedded RGBA, Windows ICO and macOS ICNS")


if __name__ == "__main__":
    main()
