"""Merge Google Sans' Latin and Arabic static fonts into one font per weight, for the
images and reports klue-report draws.

klue-report lays text out with usvg, which picks one font for a run of text and can't mix
two fonts within right-to-left text, so Arabic with figures or Latin words in it needs one
font that has both. Run from the repo root after make_static.py:

    python tools/fonts/merge_for_reports.py crates/klue-app/assets/fonts crates/klue-report/assets/fonts
"""

import sys
from pathlib import Path

from fontTools.merge import Merger

sys.path.insert(0, str(Path(__file__).parent))
from make_static import set_names  # noqa: E402

WEIGHTS = [400, 600, 700]


def main(source: str, dest: str) -> None:
    src, out = Path(source), Path(dest)
    out.mkdir(parents=True, exist_ok=True)
    for w in WEIGHTS:
        merged = Merger().merge([str(src / f"google-sans-latin-{w}.ttf"), str(src / f"google-sans-arabic-{w}.ttf")])
        set_names(merged, "Google Sans", w)
        path = out / f"google-sans-{w}.ttf"
        merged.save(path)
        print(f"{path.name}: {path.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
