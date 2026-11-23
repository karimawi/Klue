//! ddddocr's `common_old.onnx` on ONNX Runtime, with the Python package's preprocessing
//! (64 px high, Lanczos, PIL greyscale), CTC-decoded and limited to ASCII alphanumerics.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use image::imageops::FilterType;
use ort::{session::Session, value::Tensor};

use super::{BoxFuture, CaptchaSolver, find_model, model_search_paths};

/// Charset of `common_old.onnx`, from the Python package (MIT).
const CHARSET_JSON: &str = include_str!("../../assets/ddddocr_charset_old.json");

/// How pixel values are scaled before inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Normalization {
    /// `(x / 255 - 0.5) / 0.5`, as in ddddocr ≤ 1.5.
    Centered,
    /// `x / 255`, as in ddddocr 1.6 (default; reads real WE captchas better).
    Unit,
}

#[derive(Clone)]
pub struct DdddOcr {
    inner: Arc<Inner>,
    opts: Options,
}

/// For experiments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub normalization: Normalization,
    /// Keep only A-Z, a-z and 0-9.
    pub alnum_only: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { normalization: Normalization::Unit, alnum_only: true }
    }
}

struct Inner {
    session: Mutex<Session>,
    input_name: String,
    charset: Vec<String>,
    /// Indices of single ASCII alphanumeric characters (plus the CTC blank at 0).
    alnum: Vec<bool>,
}

impl DdddOcr {
    /// Loads the model from the first of [`model_search_paths`] that exists.
    pub fn load_default() -> Result<Self, String> {
        let path = find_model().ok_or_else(|| {
            format!(
                "{} not found (looked in {}). Run `cargo run -p klue-core --example fetch_ocr_model` to download it.",
                super::MODEL_FILE,
                model_search_paths().iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
            )
        })?;
        Self::load(&path)
    }

    pub fn load(model: &Path) -> Result<Self, String> {
        let env = environment()?;
        let session = Session::builder(&env)
            .and_then(|mut b| b.commit_from_file(model))
            .map_err(|e| format!("couldn't load {}: {e}", model.display()))?;
        let input_name = session.inputs().first().map(|i| i.name().to_string()).unwrap_or_else(|| "input1".into());
        let charset: Vec<String> = serde_json::from_str(CHARSET_JSON).map_err(|e| e.to_string())?;
        let alnum = charset
            .iter()
            .enumerate()
            .map(|(i, c)| i == 0 || (c.chars().count() == 1 && c.chars().all(|ch| ch.is_ascii_alphanumeric())))
            .collect();
        tracing::info!(model = %model.display(), "ddddocr loaded");
        Ok(Self {
            inner: Arc::new(Inner { session: Mutex::new(session), input_name, charset, alnum }),
            opts: Options::default(),
        })
    }

    pub fn with_options(mut self, opts: Options) -> Self {
        self.opts = opts;
        self
    }

    /// Reads the text in a captcha image.
    pub fn classify(&self, image: &[u8]) -> Result<String, String> {
        self.inner.classify(image, self.opts)
    }
}

impl CaptchaSolver for DdddOcr {
    fn name(&self) -> &str {
        "ddddocr"
    }

    fn solve<'a>(&'a self, image: &'a [u8]) -> BoxFuture<'a, Option<String>> {
        let (inner, opts) = (self.inner.clone(), self.opts);
        let image = image.to_vec();
        Box::pin(async move {
            let result = tokio::task::spawn_blocking(move || inner.classify(&image, opts)).await;
            match result {
                Ok(Ok(code)) => Some(code),
                Ok(Err(e)) => {
                    tracing::warn!("ddddocr failed: {e}");
                    None
                }
                Err(e) => {
                    tracing::warn!("ddddocr task failed: {e}");
                    None
                }
            }
        })
    }
}

impl Inner {
    fn classify(&self, bytes: &[u8], opts: Options) -> Result<String, String> {
        let img = image::load_from_memory(bytes).map_err(|e| format!("unreadable image: {e}"))?.to_rgb8();
        if img.height() == 0 || img.width() == 0 {
            return Err("empty image".into());
        }
        let width = ((img.width() as f32 * (64.0 / img.height() as f32)) as u32).max(1);
        let resized = image::imageops::resize(&img, width, 64, FilterType::Lanczos3);
        let data: Vec<f32> = resized
            .pixels()
            .map(|p| {
                // PIL's RGB -> L conversion (ITU-R 601-2 luma, fixed point).
                let l = ((p[0] as u32 * 19595 + p[1] as u32 * 38470 + p[2] as u32 * 7471 + 0x8000) >> 16) as f32 / 255.0;
                match opts.normalization {
                    Normalization::Centered => (l - 0.5) / 0.5,
                    Normalization::Unit => l,
                }
            })
            .collect();
        let tensor = Tensor::from_array((vec![1_i64, 1, 64, width as i64], data)).map_err(|e| e.to_string())?;

        let mut session = self.session.lock().map_err(|_| "OCR session poisoned".to_string())?;
        let outputs = session
            .run(ort::inputs![self.input_name.as_str() => tensor])
            .map_err(|e| format!("inference failed: {e}"))?;
        let output = &outputs[0];

        // common_old.onnx returns argmax indices; other models return logits.
        let indices: Vec<i64> = match output.try_extract_tensor::<i64>() {
            Ok((_, idx)) => idx.to_vec(),
            Err(_) => {
                let (shape, logits) = output.try_extract_tensor::<f32>().map_err(|e| format!("unexpected output: {e}"))?;
                let classes = *shape.last().ok_or("scalar output")? as usize;
                logits
                    .chunks(classes.max(1))
                    .map(|row| {
                        row.iter()
                            .enumerate()
                            .filter(|(i, _)| !opts.alnum_only || self.alnum.get(*i).copied().unwrap_or(false))
                            .max_by(|a, b| a.1.total_cmp(b.1))
                            .map_or(0, |(i, _)| i as i64)
                    })
                    .collect()
            }
        };
        Ok(self.decode(&indices, opts.alnum_only))
    }

    /// CTC decode: collapse repeats, drop blanks, optionally keep only ASCII alphanumerics.
    fn decode(&self, indices: &[i64], alnum_only: bool) -> String {
        let mut out = String::new();
        let mut prev = -1_i64;
        for &i in indices {
            if i != prev && i > 0 && (!alnum_only || self.alnum.get(i as usize).copied().unwrap_or(false))
                && let Some(c) = self.charset.get(i as usize) {
                    out.push_str(c);
                }
            prev = i;
        }
        out
    }
}

fn environment() -> Result<ort::environment::Environment, String> {
    #[cfg(feature = "ocr-dynamic")]
    if std::env::var_os("ORT_DYLIB_PATH").is_none() {
        let name = if cfg!(windows) { "onnxruntime.dll" } else if cfg!(target_os = "macos") { "libonnxruntime.dylib" } else { "libonnxruntime.so" };
        if let Some(lib) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(name))).filter(|p| p.is_file()) {
            return ort::init_from(&lib)
                .map_err(|e| format!("couldn't load {}: {e}", lib.display()))?
                .with_name("klue")
                .build()
                .map_err(|e| e.to_string());
        }
    }
    ort::init().with_name("klue").build().map_err(|e| e.to_string())
}
