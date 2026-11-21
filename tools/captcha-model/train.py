"""Train the WE captcha model and export it to ONNX for Klue.

    python train.py --out we_captcha.onnx --real real/            # real/ holds <label>.jpg files

The model is a small CNN with a CTC head (~0.4 M parameters). It sees WE's native
180x50 colour image, so it can use absolute glyph height (W vs w, Z vs z...) and
colour (red text vs blue strike line) - the two things a general OCR model gets wrong.

Training data is generated on the fly by synth.py. Real labelled captchas given with
--real are used for evaluation and, with --real-train, mixed into training.
"""

import argparse
import math
import os
import random
import time

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F
from PIL import Image
from torch.optim.swa_utils import AveragedModel, get_ema_multi_avg_fn
from torch.utils.data import DataLoader, IterableDataset

from synth import CHARSET, HEIGHT, WIDTH, Generator

BLANK = 0
INDEX = {c: i + 1 for i, c in enumerate(CHARSET)}


def to_tensor(img):
    """RGB PIL image -> float32 CHW in [-1, 1]. Must match the Rust preprocessing."""
    if img.size != (WIDTH, HEIGHT):
        img = img.resize((WIDTH, HEIGHT), Image.BILINEAR)
    a = np.asarray(img.convert("RGB"), dtype=np.float32) / 255.0
    return torch.from_numpy((a - 0.5) / 0.5).permute(2, 0, 1)


class Net(nn.Module):
    def __init__(self, n_classes=len(CHARSET) + 1):
        super().__init__()

        def block(cin, cout, pool=None):
            layers = [nn.Conv2d(cin, cout, 3, padding=1, bias=False), nn.BatchNorm2d(cout), nn.ReLU(inplace=True)]
            if pool:
                layers.append(nn.MaxPool2d(pool))
            return layers

        self.cnn = nn.Sequential(
            *block(3, 24, (2, 2)),  # 25 x 90
            *block(24, 48, (2, 2)),  # 12 x 45
            *block(48, 64),
            *block(64, 64, (2, 1)),  # 6 x 45
            *block(64, 96),
            *block(96, 96, (2, 1)),  # 3 x 45
        )
        self.proj = nn.Linear(96 * 3, 160)
        self.ctx1 = nn.Conv1d(160, 160, 3, padding=1)
        self.ctx2 = nn.Conv1d(160, 160, 3, padding=1)
        self.out = nn.Linear(160, n_classes)

    def forward(self, x):  # [B, 3, 50, 180] -> [B, 45, classes]
        f = self.cnn(x)
        b, c, h, w = f.shape
        f = f.permute(0, 3, 1, 2).reshape(b, w, c * h)
        f = F.relu(self.proj(f))
        g = F.relu(self.ctx1(f.transpose(1, 2)))
        g = F.relu(self.ctx2(g)).transpose(1, 2)
        return self.out(f + g)


def decode(logits):
    """Greedy CTC decode of [T, C] logits."""
    out, prev = [], BLANK
    for i in logits.argmax(-1).tolist():
        if i != prev and i != BLANK:
            out.append(CHARSET[i - 1])
        prev = i
    return "".join(out)


class Synthetic(IterableDataset):
    def __init__(self, fonts, real=None, real_ratio=0.0):
        self.fonts, self.real, self.real_ratio = fonts, real or [], real_ratio

    def __iter__(self):
        info = torch.utils.data.get_worker_info()
        seed = (info.seed if info else random.randrange(1 << 30)) % (1 << 30)
        gen = Generator(self.fonts, seed)
        rng = random.Random(seed)
        while True:
            if self.real and rng.random() < self.real_ratio:
                img, label = rng.choice(self.real)
            else:
                img, label = gen.render()
            yield to_tensor(img), label


def _render_chunk(args):
    fonts, seed, n = args
    gen = Generator(fonts, seed)
    imgs, labels = [], []
    for _ in range(n):
        img, label = gen.render()
        imgs.append(np.asarray(img, dtype=np.uint8))
        labels.append(label)
    return np.stack(imgs), labels


def pregenerate(fonts, n, procs):
    """Render n synthetic captchas up front (uint8 HWC), in parallel."""
    import multiprocessing as mp

    chunk = 2000
    jobs = [(fonts, 1000 + i, min(chunk, n - i * chunk)) for i in range(math.ceil(n / chunk))]
    with mp.Pool(procs) as pool:
        parts = pool.map(_render_chunk, jobs)
    imgs = np.concatenate([p[0] for p in parts])
    labels = [l for p in parts for l in p[1]]
    return torch.from_numpy(imgs), labels


def batch_from(imgs, labels, idx):
    x = imgs[idx].permute(0, 3, 1, 2).float().div_(255.0).sub_(0.5).div_(0.5)
    picked = [labels[i] for i in idx.tolist()]
    targets = torch.tensor([INDEX[c] for s in picked for c in s], dtype=torch.long)
    lengths = torch.tensor([len(s) for s in picked], dtype=torch.long)
    return x, targets, lengths, picked


def collate(batch):
    imgs = torch.stack([b[0] for b in batch])
    labels = [b[1] for b in batch]
    targets = torch.tensor([INDEX[c] for s in labels for c in s], dtype=torch.long)
    lengths = torch.tensor([len(s) for s in labels], dtype=torch.long)
    return imgs, targets, lengths, labels


def load_real(path):
    items = []
    for f in sorted(os.listdir(path)):
        label, ext = os.path.splitext(f)
        label = label.split("_")[-1]  # allow "<anything>_<label>.jpg"
        if ext.lower() in (".jpg", ".jpeg", ".png") and label and all(c in INDEX for c in label):
            items.append((Image.open(os.path.join(path, f)).convert("RGB"), label))
    return items


@torch.no_grad()
def evaluate(model, items):
    model.eval()
    if not items:
        return 1.0, 1.0, []
    x = torch.stack([to_tensor(img) for img, _ in items])
    preds = [decode(l) for l in model(x)]
    exact = sum(p == l for p, (_, l) in zip(preds, items)) / len(items)
    nocase = sum(p.lower() == l.lower() for p, (_, l) in zip(preds, items)) / len(items)
    model.train()
    return exact, nocase, preds


@torch.no_grad()
def confidence_report(model, items, thresholds=(0.0, 0.5, 0.6, 0.7, 0.8, 0.9)):
    """How often the model would answer at each confidence threshold, and how often it's
    right when it does. Confidence = lowest per-character probability (as in Klue)."""
    model.eval()
    x = torch.stack([to_tensor(img) for img, _ in items])
    probs = model(x).softmax(-1)
    rows = []
    for p, (_, label) in zip(probs, items):
        best = p.argmax(-1).tolist()
        text, conf, prev, run = [], 1.0, BLANK, 0.0
        for t, i in enumerate(best):
            if i != prev:
                if prev != BLANK:
                    conf = min(conf, run)
                if i != BLANK:
                    text.append(CHARSET[i - 1])
                run = p[t, i].item()
            else:
                run = max(run, p[t, i].item())
            prev = i
        if prev != BLANK:
            conf = min(conf, run)
        rows.append(("".join(text) == label, conf if text else 0.0))
    for th in thresholds:
        answered = [ok for ok, c in rows if c >= th]
        acc = sum(answered) / len(answered) if answered else float("nan")
        print(f"  confidence >= {th:.1f}: answers {len(answered) / len(rows):6.1%}, correct when answering {acc:6.1%}")
    model.train()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--out", default="we_captcha.onnx")
    p.add_argument("--steps", type=int, default=6000)
    p.add_argument("--batch", type=int, default=64)
    p.add_argument("--lr", type=float, default=3e-3)
    p.add_argument("--workers", type=int, default=3)
    p.add_argument("--font", action="append")
    p.add_argument("--real", help="folder of real labelled captchas (<label>.jpg) for evaluation")
    p.add_argument("--real-train", type=float, default=0.0, help="fraction of each batch drawn from --real (0 = eval only)")
    p.add_argument("--resume")
    p.add_argument("--pregen", type=int, default=0, help="render this many samples up front instead of on the fly")
    p.add_argument("--threads", type=int, default=0)
    p.add_argument("--ema", type=float, default=0.999,
                   help="evaluate and export an exponential moving average of the weights (0 = the last step's)")
    a = p.parse_args()
    if a.threads:
        torch.set_num_threads(a.threads)

    torch.manual_seed(0)
    real = load_real(a.real) if a.real else []
    val = [Generator(a.font, 12345 + i).render() for i in range(500)]
    model = Net()
    if a.resume:
        model.load_state_dict(torch.load(a.resume))
    print(f"parameters: {sum(p.numel() for p in model.parameters()):,}  real samples: {len(real)}")

    if a.pregen:
        t = time.time()
        pool_imgs, pool_labels = pregenerate(a.font, a.pregen, os.cpu_count() or 2)
        print(f"rendered {len(pool_labels):,} synthetic captchas in {time.time() - t:.0f}s", flush=True)

        real_x = torch.stack([to_tensor(img) for img, _ in real]) if real else None
        n_real = round(a.batch * a.real_train) if real else 0

        def batches():
            while True:
                x, _, _, labels = batch_from(pool_imgs, pool_labels, torch.randint(0, len(pool_labels), (a.batch - n_real,)))
                if n_real:
                    # Mix in real captchas, shifted by up to 2 px so they aren't memorised.
                    idx = torch.randint(0, len(real), (n_real,))
                    xr = torch.roll(real_x[idx], shifts=(random.randint(-2, 2), random.randint(-2, 2)), dims=(2, 3))
                    x = torch.cat([x, xr])
                    labels = labels + [real[i][1] for i in idx.tolist()]
                targets = torch.tensor([INDEX[c] for s in labels for c in s], dtype=torch.long)
                lengths = torch.tensor([len(s) for s in labels], dtype=torch.long)
                yield x, targets, lengths, labels

        data = batches()
    else:
        data = DataLoader(
            Synthetic(a.font, real, a.real_train), batch_size=a.batch, num_workers=a.workers, collate_fn=collate,
            persistent_workers=a.workers > 0,
        )
    opt = torch.optim.AdamW(model.parameters(), lr=a.lr, weight_decay=1e-4)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=a.lr, total_steps=a.steps, pct_start=0.15)
    ctc = nn.CTCLoss(blank=BLANK, zero_infinity=True)
    # Accuracy on real captchas moves a few points between checkpoints; an average of the
    # recent weights is steadier than whichever step happens to be last.
    ema = AveragedModel(model, multi_avg_fn=get_ema_multi_avg_fn(a.ema), use_buffers=True) if a.ema else None
    final = ema.module if ema else model

    t0, running = time.time(), None
    for step, (x, targets, lengths, _) in enumerate(data, 1):
        logits = model(x)  # [B, T, C]
        logp = logits.log_softmax(-1).transpose(0, 1)  # [T, B, C]
        input_lengths = torch.full((x.shape[0],), logp.shape[0], dtype=torch.long)
        loss = ctc(logp, targets, input_lengths, lengths)
        opt.zero_grad()
        loss.backward()
        nn.utils.clip_grad_norm_(model.parameters(), 5.0)
        opt.step()
        sched.step()
        if ema:
            ema.update_parameters(model)
        running = loss.item() if running is None else 0.98 * running + 0.02 * loss.item()
        if step % 250 == 0 or step == a.steps:
            sv, _, _ = evaluate(final, val)
            rv, rn, preds = evaluate(final, real)
            print(f"step {step:5d}  loss {running:.4f}  synth {sv:.3f}  real {rv:.2f} (no-case {rn:.2f}) {preds}  {time.time() - t0:.0f}s", flush=True)
            torch.save(final.state_dict(), a.out + ".pt")
        if step >= a.steps:
            break

    print("synthetic (fresh seeds):")
    confidence_report(final, [Generator(a.font, 777000 + i).render() for i in range(2000)])
    if real:
        print("real:")
        confidence_report(final, real)

    final.eval()
    dummy = torch.zeros(1, 3, HEIGHT, WIDTH)
    torch.onnx.export(
        final, dummy, a.out, input_names=["image"], output_names=["logits"],
        dynamic_axes={"image": {0: "batch"}, "logits": {0: "batch"}}, opset_version=17, dynamo=False,
    )
    print(f"wrote {a.out} ({os.path.getsize(a.out) / 1e6:.2f} MB)")


if __name__ == "__main__":
    main()
