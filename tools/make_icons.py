"""Ven4Docking icons from the master picture (Venchik holding a phone).

Master: assets/icon/ven4docking-1024.png (1024x1024 RGBA, transparent rounded corners).
Regenerate with `python tools/make_icons.py` (needs Pillow). Outputs (committed):
  assets/icon/ven4docking.ico          16..256, used by the editor, the agent (build.rs) and the installer
  assets/icon/ven4docking-256.png      README header, About section of the editor
  android/app/src/main/res/mipmap-<dpi>/ic_launcher.png      legacy launcher icon (Android 7.x): the full master
  android/app/src/main/res/mipmap-<dpi>/ic_launcher_fg.png   adaptive foreground (Android 8+): the master
                                                              scaled into the 66 % safe zone, over @color/ic_bg
"""

from __future__ import annotations

import io
import struct
from pathlib import Path

from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
MASTER = ROOT / "assets" / "icon" / "ven4docking-1024.png"

ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
# density -> (legacy 48 dp, adaptive layer 108 dp)
MIPMAPS = {"mdpi": (48, 108), "hdpi": (72, 162), "xhdpi": (96, 216), "xxhdpi": (144, 324), "xxxhdpi": (192, 432)}
SAFE_ZONE = 0.66


def scaled(master: Image.Image, size: int) -> Image.Image:
    """Downscale in two steps (better than one huge LANCZOS jump); small sizes get extra sharpening."""
    im = master
    while im.size[0] > size * 4:
        im = im.resize((im.size[0] // 2, im.size[1] // 2), Image.LANCZOS)
    im = im.resize((size, size), Image.LANCZOS)
    if size <= 32:
        # sharpen colour only: keep the anti-aliased alpha edge untouched
        alpha = im.getchannel("A")
        rgb = im.convert("RGB").filter(ImageFilter.UnsharpMask(radius=0.8, percent=120 if size <= 16 else 80, threshold=1))
        im = rgb.convert("RGBA")
        im.putalpha(alpha)
    return im


def dib_bytes(img: Image.Image) -> bytes:
    """32-bit BGRA DIB with an all-zero AND mask (what classic .ico consumers expect)."""
    w, h = img.size
    header = struct.pack("<IiiHHIIiiII", 40, w, h * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    px = img.tobytes("raw", "BGRA")
    stride = w * 4
    rows = b"".join(px[y * stride:(y + 1) * stride] for y in range(h - 1, -1, -1))
    mask_stride = ((w + 31) // 32) * 4
    return header + rows + b"\0" * (mask_stride * h)


def write_ico(path: Path, images: list[Image.Image]) -> None:
    """ICO: 256 as PNG, the rest as 32-bit DIB (works for Explorer, .NET, the linker .res and NSIS)."""
    blobs = []
    for im in images:
        if im.size[0] >= 256:
            buf = io.BytesIO()
            im.save(buf, "PNG", optimize=True)
            blobs.append(buf.getvalue())
        else:
            blobs.append(dib_bytes(im))
    out = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for im, blob in zip(images, blobs):
        w = im.size[0]
        out += struct.pack("<BBBBHHII", 0 if w >= 256 else w, 0 if w >= 256 else w, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    path.write_bytes(out + b"".join(blobs))


def adaptive_fg(master: Image.Image, px: int) -> Image.Image:
    layer = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    inner = round(px * SAFE_ZONE)
    icon = scaled(master, inner)
    off = (px - inner) // 2
    layer.alpha_composite(icon, (off, off))
    return layer


def main() -> None:
    master = Image.open(MASTER).convert("RGBA")
    icon_dir = MASTER.parent
    write_ico(icon_dir / "ven4docking.ico", [scaled(master, n) for n in ICO_SIZES])
    scaled(master, 256).save(icon_dir / "ven4docking-256.png", optimize=True)

    res = ROOT / "android" / "app" / "src" / "main" / "res"
    for bucket, (legacy, layer) in MIPMAPS.items():
        out = res / f"mipmap-{bucket}"
        out.mkdir(parents=True, exist_ok=True)
        scaled(master, legacy).save(out / "ic_launcher.png", optimize=True)
        adaptive_fg(master, layer).save(out / "ic_launcher_fg.png", optimize=True)
    print("icons written to", icon_dir, "and", res)


if __name__ == "__main__":
    main()
