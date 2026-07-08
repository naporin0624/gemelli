#!/usr/bin/env python3
"""Draw the gemelli app icon: two separate glowing pads (twins) in the
Cannelloni visual language.

gemelli is the twin sibling of the Cannelloni app (same author, same
Spout/Syphon domain), so the icon speaks the same language: a deep cool-black
squircle tile with softly glowing rounded-square pads (cf. Cannelloni's 2x2
pad grid). Two pads — blue (webcam in) and cyan (shared texture out) — sit
apart as identical twins. Colours are the GUI theme tokens
(crates/gui/src/theme.rs).

Requires Pillow (`pip install pillow`). Writes crates/gui/assets/icon.png.
Rendered at 4x then downsampled for crisp antialiasing.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

# Cannelloni-family tile (sampled from Cannelloni/resources/icon.png) + gemelli
# accent tokens (crates/gui/src/theme.rs).
TILE_TOP = (20, 23, 31)          # cool near-black, top
TILE_BOTTOM = (12, 14, 20)       # deeper at the bottom
ACCENT = (57, 150, 255)          # #3996FF blue  (left / webcam in)
ACCENT_ALT = (52, 221, 229)      # #34DDE5 cyan  (right / shared texture out)

SUPERSAMPLE = 4
SIZE = 1024
N = SIZE * SUPERSAMPLE


def squircle_mask() -> Image.Image:
    margin = int(N * 0.055)
    radius = int(N * 0.225)
    mask = Image.new("L", (N, N), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [margin, margin, N - margin, N - margin], radius=radius, fill=255
    )
    return mask


def gradient_tile() -> Image.Image:
    grad = Image.new("RGB", (1, N))
    for y in range(N):
        t = y / (N - 1)
        grad.putpixel(
            (0, y),
            tuple(int(a + (b - a) * t) for a, b in zip(TILE_TOP, TILE_BOTTOM)),
        )
    return grad.resize((N, N))


def pad_box(cx: int, cy: int, half: int) -> list[int]:
    return [cx - half, cy - half, cx + half, cy + half]


def glow_layer(cx: int, cy: int, half: int, rad: int, color: tuple[int, int, int]) -> Image.Image:
    layer = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    halo = int(half * 1.28)
    ImageDraw.Draw(layer).rounded_rectangle(
        pad_box(cx, cy, halo), radius=int(rad * 1.28), fill=(*color, 135)
    )
    return layer.filter(ImageFilter.GaussianBlur(int(N * 0.05)))


def draw() -> Image.Image:
    mask = squircle_mask()
    img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    img.paste(gradient_tile(), (0, 0), mask)

    cy = N // 2
    half = int(N * 0.150)      # half the pad side
    rad = int(half * 0.42)     # pad corner radius (Cannelloni-ish rounding)
    off = int(N * 0.185)       # centres apart → a clear gap between the two pads
    cxl, cxr = cy - off, cy + off

    for cx, color in ((cxl, ACCENT), (cxr, ACCENT_ALT)):
        img.alpha_composite(glow_layer(cx, cy, half, rad, color))

    core = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    dc = ImageDraw.Draw(core)
    for cx, color in ((cxl, ACCENT), (cxr, ACCENT_ALT)):
        dc.rounded_rectangle(pad_box(cx, cy, half), radius=rad, fill=(*color, 255))
    img.alpha_composite(core)

    # Clip everything (including glow bleed) to the squircle.
    img.putalpha(Image.composite(img.getchannel("A"), Image.new("L", (N, N), 0), mask))
    return img.resize((SIZE, SIZE), Image.LANCZOS)


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    dst = root / "crates" / "gui" / "assets" / "icon.png"
    dst.parent.mkdir(parents=True, exist_ok=True)
    draw().save(dst)
    print(f"wrote {dst}")


if __name__ == "__main__":
    main()
