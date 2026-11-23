"""Render Klue's icons from crates/klue-app/icons/klue.svg.

    python tools/icons/make_icons.py crates/klue-app/icons

Writes klue-{1024,256,64,32}.png, klue.ico (Windows) and klue-macos-1024.png (the
macOS app icon: Apple's 824 px rounded square on a 1024 px canvas, with a soft shadow).
"""

import io
import sys
from pathlib import Path

import cairosvg
from PIL import Image, ImageDraw, ImageFilter


def render(svg: bytes, size: int) -> Image.Image:
    # Cairo clips each shape on its own, so where the K runs off the tile the yellow under it
    # shows through the anti-aliased corner. Render the square tile unclipped and cut the
    # rounded corners here instead.
    flat = svg.replace(b' clip-path="url(#tile)"', b"").replace(b' rx="80" fill="#FFBE32"', b' fill="#FFBE32"')
    assert flat.count(b"clip-path") == 0 and b'rx="80" fill="#FFBE32"' not in flat, "klue.svg changed shape"
    png = cairosvg.svg2png(bytestring=flat, output_width=size, output_height=size)
    art = Image.open(io.BytesIO(png)).convert("RGBA")
    art.putalpha(rounded_mask(size, round(size * 0.08)))
    return art


def rounded_mask(size: int, radius: int, scale: int = 4) -> Image.Image:
    big = Image.new("L", (size * scale, size * scale), 0)
    ImageDraw.Draw(big).rounded_rectangle((0, 0, size * scale - 1, size * scale - 1), radius * scale, fill=255)
    return big.resize((size, size), Image.LANCZOS)


def main(folder: str) -> None:
    out = Path(folder)
    svg = (out / "klue.svg").read_bytes()
    for size in (1024, 256, 64, 32):
        render(svg, size).save(out / f"klue-{size}.png")
    render(svg, 256).save(out / "klue.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])

    # macOS: content inset to Apple's grid, with its corner radius and a soft shadow.
    body, radius, canvas = 824, 185, 1024
    art = render(svg, body)
    art.putalpha(rounded_mask(body, radius))
    shadow = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    silhouette = Image.new("RGBA", (body, body), (0, 0, 0, 90))
    silhouette.putalpha(Image.eval(rounded_mask(body, radius), lambda a: a * 90 // 255))
    off = (canvas - body) // 2
    shadow.paste(silhouette, (off, off + 12), silhouette)
    shadow = shadow.filter(ImageFilter.GaussianBlur(14))
    shadow.alpha_composite(art, (off, off))
    shadow.save(out / "klue-macos-1024.png")
    print("ok")


if __name__ == "__main__":
    main(sys.argv[1])
