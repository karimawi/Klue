"""Synthetic captchas in the style of WE's GenerateCaptcha (captcha.te.eg).

Measured from real samples: 180x50 JPEG, light grey background with per-pixel colour
noise, five red characters in Arial or (about a third of the time) Times New Roman
(28-34 px, caps 20-25 px tall, baseline y~35) on a fixed ~20 px pitch, drawn over one thin
blue line. Liberation Sans/Serif are metrically identical to Arial/Times; pass --font to
use the originals if you have them.
"""

import io
import random
import string

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

CHARSET = string.digits + string.ascii_uppercase + string.ascii_lowercase  # 62, blank = index 0 in the model
# Letters whose upper and lower case differ only in size; never resize these on their own.
SIZE_IS_CASE = set("cosuvwxzCOSUVWXZ")
WIDTH, HEIGHT = 180, 50
# Share of captchas with the text over the strike line, as in the real ones (letters stay
# red where the line crosses). A few line-on-top ones help robustness.
LINE_UNDER = 0.85
# (path, weight). Mostly Arial, about a third Times New Roman. Liberation Sans/Serif have
# the same metrics; the others add variety so the model doesn't overfit one rasteriser.
DEFAULT_FONTS = [
    ("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf", 60),
    ("/usr/share/fonts/truetype/freefont/FreeSans.ttf", 8),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 6),
    ("/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf", 30),
    ("/usr/share/fonts/truetype/freefont/FreeSerif.ttf", 4),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf", 2),
]


class Generator:
    def __init__(self, fonts=None, seed=None, line_under=LINE_UNDER):
        import os

        self.line_under = line_under

        if fonts:
            self.fonts, self.weights = list(fonts), [1] * len(fonts)
        else:
            found = [(p, w) for p, w in DEFAULT_FONTS if os.path.exists(p)]
            self.fonts, self.weights = [p for p, _ in found], [w for _, w in found]
        self.rng = random.Random(seed)
        self.np_rng = np.random.default_rng(seed)
        self._cache = {}

    def font(self, path, size):
        key = (path, size)
        if key not in self._cache:
            self._cache[key] = ImageFont.truetype(path, size)
        return self._cache[key]

    def text(self):
        r = self.rng
        length = r.choices([4, 5, 6], weights=[1, 18, 1])[0]
        return "".join(r.choice(CHARSET) for _ in range(length))

    def render(self, text=None):
        r = self.rng
        text = text or self.text()

        # Background: flat light grey plus per-pixel colour noise.
        base = r.uniform(196, 208)
        noise = self.np_rng.normal(0, r.uniform(11, 17), (HEIGHT, WIDTH, 3))
        bg = np.clip(base + noise, 0, 255).astype(np.uint8)
        img = Image.fromarray(bg, "RGB")

        # Text and line are drawn at 4x and scaled down, which gives the same soft,
        # anti-aliased edges as the real images.
        k = 4
        layer = Image.new("RGBA", (WIDTH * k, HEIGHT * k), (0, 0, 0, 0))
        draw = ImageDraw.Draw(layer)

        # Text: red, Arial-like. WE draws each character in a fixed ~20 px slot
        # (ink starts at x = 7, 27, 47, 67, 87 +/- 2), whatever its width: narrow
        # letters leave gaps and wide ones (W, M) overlap their neighbours.
        font_path = r.choices(self.fonts, weights=self.weights)[0]
        # Font size varies per captcha (caps 20-25 px in real samples, i.e. ~28-34 px).
        size = r.randint(27, 35)
        font = self.font(font_path, size * k)
        red = (r.randint(240, 255), r.randint(10, 55), r.randint(10, 55), 255)
        pitch = r.uniform(19.0, 21.0)
        x0 = r.uniform(4.0, 9.5)
        baseline = r.uniform(33, 37)
        for i, ch in enumerate(text):
            x = x0 + i * pitch + r.uniform(-1.5, 1.5)
            y = baseline + r.uniform(-3.5, 3.5)
            # Sometimes a character is a bit smaller or larger (seen on digits), but never one
            # whose case only differs by size: that made real accuracy worse (47/53 vs 51/53).
            resize = ch not in SIZE_IS_CASE and r.random() < 0.2
            glyph = self.font(font_path, round(size * r.uniform(0.85, 1.08)) * k) if resize else font
            draw.text((x * k, y * k), ch, font=glyph, fill=red, anchor="ls")

        # Stroke weight and opacity vary between renderers; vary them here too.
        alpha = layer.getchannel("A")
        roll = r.random()
        if roll < 0.2:
            alpha = alpha.filter(ImageFilter.MinFilter(3))  # thinner
        elif roll < 0.4:
            alpha = alpha.filter(ImageFilter.MaxFilter(3))  # bolder
        opacity = r.uniform(0.88, 1.0)
        layer.putalpha(alpha.point(lambda v: int(v * opacity)))

        # Strike-through: one thin blue line across the text (usually under it, see LINE_UNDER).
        line = Image.new("RGBA", layer.size, (0, 0, 0, 0))
        x0, x1 = r.uniform(12, 35), r.uniform(138, 168)
        y0, y1 = r.uniform(14, 34), r.uniform(14, 34)
        blue = (r.randint(80, 130), r.randint(80, 130), r.randint(225, 255), 255)
        ImageDraw.Draw(line).line([(x0 * k, y0 * k), (x1 * k, y1 * k)], fill=blue, width=round(r.uniform(1.6, 2.6) * k))
        if r.random() < self.line_under:
            layer = Image.alpha_composite(line, layer)  # text drawn over the line
        else:
            layer = Image.alpha_composite(layer, line)  # line drawn over the text

        layer = layer.resize((WIDTH, HEIGHT), Image.LANCZOS)
        img = Image.alpha_composite(img.convert("RGBA"), layer).convert("RGB")

        # Mild softening, then JPEG like the real thing.
        if r.random() < 0.5:
            img = img.filter(ImageFilter.GaussianBlur(r.uniform(0.2, 0.6)))
        buf = io.BytesIO()
        img.save(buf, "JPEG", quality=r.randint(55, 85))
        return Image.open(io.BytesIO(buf.getvalue())).convert("RGB"), text


if __name__ == "__main__":
    import argparse
    import os

    p = argparse.ArgumentParser(description="Write synthetic WE-style captchas named <label>.jpg")
    p.add_argument("out")
    p.add_argument("-n", type=int, default=20)
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--font", action="append")
    a = p.parse_args()
    os.makedirs(a.out, exist_ok=True)
    g = Generator(a.font, a.seed)
    for _ in range(a.n):
        im, label = g.render()
        im.save(os.path.join(a.out, f"{label}.jpg"), quality=95)
