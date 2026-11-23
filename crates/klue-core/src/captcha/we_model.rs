//! Klue's model for WE captchas (red Arial text, blue strike line, 180x50 JPEG): a small
//! CNN with a CTC head, trained in `tools/captcha-model`, embedded and run with tract.
//! It keeps the image's real scale, which is what tells W from w in this font.

use std::{io::Cursor, sync::Arc};

use image::imageops::FilterType;
use tract_onnx::prelude::*;

use super::{BoxFuture, CaptchaSolver};

const MODEL: &[u8] = include_bytes!("../../assets/we_captcha.onnx");
/// Class `i + 1` of the model is `CHARSET[i]`; class 0 is the CTC blank.
const CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const WIDTH: u32 = 180;
const HEIGHT: u32 = 50;
/// Below this the solver declines. A wrong code costs one of WE's few attempts and a new
/// captcha costs nothing. At 0.95 it answers ~85% of captchas and is wrong on ~0.2%.
pub const MIN_CONFIDENCE: f32 = 0.95;

/// Text and confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub text: String,
    /// Lowest per-character probability.
    pub confidence: f32,
}

#[derive(Clone)]
pub struct WeCaptchaModel {
    plan: Arc<TypedRunnableModel>,
}

impl WeCaptchaModel {
    pub fn load() -> Result<Self, String> {
        let plan = tract_onnx::onnx()
            .model_for_read(&mut Cursor::new(MODEL))
            .and_then(|m| m.with_input_fact(0, f32::fact([1, 3, HEIGHT as usize, WIDTH as usize]).into()))
            .and_then(|m| m.into_optimized())
            .and_then(|m| m.into_runnable())
            .map_err(|e| format!("couldn't load the WE captcha model: {e}"))?;
        Ok(Self { plan })
    }

    /// Reads a WE captcha (JPEG or PNG).
    pub fn classify(&self, image: &[u8]) -> Result<String, String> {
        self.read(image).map(|r| r.text)
    }

    /// Same, with the confidence.
    pub fn read(&self, image: &[u8]) -> Result<Reading, String> {
        let img = image::load_from_memory(image).map_err(|e| format!("unreadable image: {e}"))?.to_rgb8();
        let img = if img.dimensions() == (WIDTH, HEIGHT) {
            img
        } else {
            image::imageops::resize(&img, WIDTH, HEIGHT, FilterType::Triangle)
        };
        let input = tract_ndarray::Array4::from_shape_fn((1, 3, HEIGHT as usize, WIDTH as usize), |(_, c, y, x)| {
            (img.get_pixel(x as u32, y as u32)[c] as f32 / 255.0 - 0.5) / 0.5
        });
        let outputs = self.plan.run(tvec!(input.into_tensor().into())).map_err(|e| format!("inference failed: {e}"))?;
        let logits = outputs[0].to_plain_array_view::<f32>().map_err(|e| e.to_string())?; // [1, T, classes]
        let classes = *logits.shape().last().ok_or("scalar output")?;
        let flat = logits.as_slice().ok_or("non-contiguous output")?;
        Ok(decode(flat, classes))
    }
}

/// Greedy CTC decode of `[T, classes]` logits. Confidence is the lowest per-character
/// softmax peak.
fn decode(flat: &[f32], classes: usize) -> Reading {
    let mut text = String::new();
    let mut confidence = 1.0_f32;
    let mut prev = 0usize;
    let mut run_best = 0.0_f32;
    for row in flat.chunks(classes) {
        let (best, &top) = row.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap_or((0, &0.0));
        let sum: f32 = row.iter().map(|v| (v - top).exp()).sum();
        let p = 1.0 / sum; // softmax probability of the best class
        if best != prev {
            if prev != 0 {
                confidence = confidence.min(run_best);
            }
            if best != 0
                && let Some(&c) = CHARSET.get(best - 1) {
                    text.push(c as char);
                }
            run_best = p;
        } else {
            run_best = run_best.max(p);
        }
        prev = best;
    }
    if prev != 0 {
        confidence = confidence.min(run_best);
    }
    if text.is_empty() {
        confidence = 0.0;
    }
    Reading { text, confidence }
}

impl CaptchaSolver for WeCaptchaModel {
    fn name(&self) -> &str {
        "we-model"
    }

    fn solve<'a>(&'a self, image: &'a [u8]) -> BoxFuture<'a, Option<String>> {
        let (model, image) = (self.clone(), image.to_vec());
        Box::pin(async move {
            match tokio::task::spawn_blocking(move || model.read(&image)).await {
                Ok(Ok(r)) if (4..=6).contains(&r.text.len()) && r.confidence >= MIN_CONFIDENCE => {
                    tracing::info!(confidence = r.confidence, "WE captcha read");
                    Some(r.text)
                }
                Ok(Ok(r)) => {
                    tracing::info!(confidence = r.confidence, len = r.text.len(), "WE captcha model unsure; not guessing");
                    None
                }
                Ok(Err(e)) => {
                    tracing::warn!("WE captcha model failed: {e}");
                    None
                }
                Err(e) => {
                    tracing::warn!("WE captcha model task failed: {e}");
                    None
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_ctc_with_confidence() {
        // classes: 0 blank, 1 '0', 2 '1'. Frames: '0' '0' blank '1' '1' (two characters).
        let c = 3;
        let frame = |best: usize, margin: f32| {
            let mut row = vec![0.0; c];
            row[best] = margin;
            row
        };
        let logits: Vec<f32> = [frame(1, 8.0), frame(1, 1.0), frame(0, 5.0), frame(2, 0.5), frame(2, 0.2)].concat();
        let r = decode(&logits, c);
        assert_eq!(r.text, "01");
        // '1' peaks at softmax(0.5 vs 0, 0) = e^0.5 / (e^0.5 + 2) ≈ 0.452
        assert!((r.confidence - 0.452).abs() < 0.01, "{}", r.confidence);
        assert_eq!(decode(&frame(0, 3.0), c), Reading { text: String::new(), confidence: 0.0 });
    }

    /// Real WE captchas, named after their answer. The model must never be confidently wrong.
    #[test]
    fn reads_real_we_captchas() {
        let model = WeCaptchaModel::load().unwrap();
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/captchas");
        let (mut total, mut correct, mut confident_wrong) = (0, 0, Vec::new());
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let label = path.file_stem().unwrap().to_string_lossy().to_string();
            let r = model.read(&std::fs::read(&path).unwrap()).unwrap();
            eprintln!("{label}: read {} ({:.2})", r.text, r.confidence);
            total += 1;
            if r.text == label {
                correct += 1;
            } else if r.confidence >= MIN_CONFIDENCE {
                confident_wrong.push(label);
            }
        }
        assert!(total >= 5);
        assert!(confident_wrong.is_empty(), "confidently wrong on {confident_wrong:?}");
        assert!(correct * 10 >= total * 9, "only {correct}/{total} read correctly");
    }
}
