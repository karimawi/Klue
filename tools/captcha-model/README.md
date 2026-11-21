# WE captcha model

Klue's own reader for the captchas WE (`captcha.te.eg`) shows after a few sign-ins:
180×50 JPEGs with five red Arial characters, random gaps, a blue strike-through line
and colour noise. The trained model ships in the app as
`crates/klue-core/assets/we_captcha.onnx` and runs on [tract](https://github.com/sonos/tract),
a pure-Rust ONNX runtime, so the app needs no download or native ML library.

## Why not ddddocr or PaddleOCR

General OCR models normalise text size away. In Arial, `C O S V W X Z` (and `c o s v w x z`)
differ from their other case only by size, so a general model reads the letters but
guesses their case: ddddocr reads all 15 real samples correctly ignoring case, but only
9 exactly. About 23% of the alphabet has this problem, so most 5-character
codes contain at least one such letter. Fine-tuning PaddleOCR would mean a large
general-purpose recogniser and the Paddle toolchain for a single fixed-format captcha.
This model sees WE's images at their real scale, so it learns cap height vs x-height,
and uses colour to ignore the strike line.

## Current model

| | Real (15, never trained on) | Synthetic (2,000 fresh) |
| --- | --- | --- |
| Exact | 15/15 | 96.1% |
| Confidence ≥ 0.7 (Klue's threshold) | answers 14/15, all correct | answers 95.5%, 98.2% correct |
| ddddocr, for comparison | 9/15 (15/15 ignoring case) | |

The real samples were used to report accuracy while training and to tune the generator,
never as training data. What matching the generator to them showed:

- characters sit on a fixed ~20 px pitch (wide letters overlap, narrow ones leave gaps);
- most captchas use Arial, some Times New Roman (2 of 15);
- font size varies per captcha (caps 20–25 px), so case must be judged relative to the
  other letters as well as absolutely; digits are sometimes smaller or raised;
- text and line are anti-aliased; the line is drawn over the text.

## Files

- `synth.py`: renders captchas in WE's style (measured from real samples: font size,
  baseline, gaps, colours, noise, line), so training data is unlimited and labelled.
  `python synth.py out/ -n 50` writes samples named `<label>.jpg` to compare by eye.
- `train.py`: a small CNN with a CTC head (~0.4 M parameters), trained on synthetic
  captchas and evaluated on real ones; exports ONNX and prints how accuracy trades off
  against the confidence threshold.
- `we_captcha.pt`: the PyTorch weights of the shipped model, for fine-tuning (`--resume`).

## Retraining

```sh
python -m venv .venv && .venv/bin/pip install -r requirements.txt
.venv/bin/python train.py --real ../../crates/klue-core/tests/captchas \
    --pregen 150000 --steps 8000 --out ../../crates/klue-core/assets/we_captcha.onnx
cargo test -p klue-core --lib we_model   # checks the exported model on the real samples
```

From scratch this takes about 40 minutes on a 4-core CPU (rendering the 150k samples takes
3–4 of them). Add `--resume we_captcha.pt` to start from the shipped weights instead.
`--real` is a folder of real captchas named `<answer>.jpg`, used to report accuracy while
training. The exported model must keep the input/output contract the Rust side expects:
`image` float32 `[1, 3, 50, 180]` RGB scaled to [-1, 1], `logits` `[1, 45, 63]` with class 0
the CTC blank and classes 1–62 `0-9A-Za-z`.

## Getting more real captchas

Real labelled captchas are the best way to make the model robust:

- `cargo run -p klue-core --example we_captcha -- <landline> --count 20` saves fresh captchas
  to `captchas/`. Rename each file to its answer (`yvgSA.jpg`) to label it.
- With `--password`, the harness signs in with the model's reading; every captcha WE
  accepts lands in `captchas/labeled/<code>.jpg`, already verified.

With a few hundred real samples, fine-tune on a mix of real and synthetic data:

```sh
.venv/bin/python train.py --real captchas/labeled --real-train 0.3 \
    --resume we_captcha.pt --pregen 60000 --steps 2000 --lr 1e-3 \
    --out ../../crates/klue-core/assets/we_captcha.onnx
```

(`--real-train` makes each batch ~30% real captchas; keep a few real samples out of the
folder to measure honestly.)
