"""Turn the bundled variable WOFF2 fonts into static TTF instances for the native UI.

iced's text engine (cosmic-text) reads TTF/OTF, picks faces by family + weight, and
renders a variable font's default instance only, so each weight Klue uses gets its own
file. Run from the repo root:

    python tools/fonts/make_static.py crates/klue-app/assets/fonts
"""

import sys
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

SOURCES = {
    # file stem -> (family, weights)
    "dm-sans-latin": ("DM Sans", [400, 600, 800]),
    "dm-sans-latin-ext": ("DM Sans", [400, 600, 800]),
    "google-sans-latin": ("Google Sans", [400, 600, 700]),
    "google-sans-arabic": ("Google Sans", [400, 600, 700]),
}
STYLE = {400: "Regular", 500: "Medium", 600: "SemiBold", 700: "Bold", 800: "ExtraBold"}


def set_names(font: TTFont, family: str, weight: int) -> None:
    style = STYLE[weight]
    name = font["name"]
    for rec in list(name.names):
        if rec.nameID in (1, 2, 3, 4, 6, 16, 17, 25):
            name.removeNames(nameID=rec.nameID)
    # Name IDs 1/2 hold the family and style the way fontdb expects for static faces.
    name.setName(family, 1, 3, 1, 0x409)
    name.setName(style, 2, 3, 1, 0x409)
    name.setName(f"{family} {style}", 4, 3, 1, 0x409)
    name.setName(f"{family.replace(' ', '')}-{style}", 6, 3, 1, 0x409)
    name.setName(family, 16, 3, 1, 0x409)
    name.setName(style, 17, 3, 1, 0x409)
    font["OS/2"].usWeightClass = weight
    font["OS/2"].fsSelection = (font["OS/2"].fsSelection & ~0b1100001) | (0b1000000 if weight == 400 else 0)
    font["head"].macStyle = 0b1 if weight >= 700 else 0


def main(folder: str) -> None:
    out = Path(folder)
    for stem, (family, weights) in SOURCES.items():
        src = out / f"{stem}.woff2"
        for w in weights:
            font = TTFont(src)
            if "fvar" in font:
                axes = {a.axisTag: (a.minValue, a.defaultValue, a.maxValue) for a in font["fvar"].axes}
                pins = {"wght": max(axes["wght"][0], min(w, axes["wght"][2]))}
                if "opsz" in axes:
                    pins["opsz"] = axes["opsz"][1]
                for tag, (_, default, _) in axes.items():
                    pins.setdefault(tag, default)
                font = instancer.instantiateVariableFont(font, pins)
            set_names(font, family, w)
            font.flavor = None
            dest = out / f"{stem}-{w}.ttf"
            font.save(dest)
            print(f"{dest.name}: {dest.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main(sys.argv[1])
