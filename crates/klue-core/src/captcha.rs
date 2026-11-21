//! Captcha solving. WE starts asking for a captcha after a few sign-ins. Klue reads it
//! with its own model (feature `we-model`, the default), optionally with ddddocr
//! (feature `ocr`), and falls back to asking the user.

use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
};

use base64::Engine;

#[cfg(any(feature = "ocr", feature = "ocr-dynamic"))]
mod ddddocr;
#[cfg(any(feature = "ocr", feature = "ocr-dynamic"))]
pub use ddddocr::{DdddOcr, Normalization, Options as OcrOptions};
#[cfg(feature = "we-model")]
mod we_model;
#[cfg(feature = "we-model")]
pub use we_model::WeCaptchaModel;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Something that can read a captcha image: an OCR model, or a person.
pub trait CaptchaSolver: Send + Sync {
    fn name(&self) -> &str;
    /// The code in `image`, or `None` when this solver can't answer.
    fn solve<'a>(&'a self, image: &'a [u8]) -> BoxFuture<'a, Option<String>>;
}

/// Tries each solver in turn until one answers.
pub struct SolverChain(pub Vec<Arc<dyn CaptchaSolver>>);

impl CaptchaSolver for SolverChain {
    fn name(&self) -> &str {
        "chain"
    }

    fn solve<'a>(&'a self, image: &'a [u8]) -> BoxFuture<'a, Option<String>> {
        Box::pin(async move {
            for solver in &self.0 {
                if let Some(code) = solver.solve(image).await.map(|c| clean_code(&c)).filter(|c| !c.is_empty()) {
                    tracing::debug!(solver = solver.name(), "captcha solved");
                    return Some(code);
                }
            }
            None
        })
    }
}

/// WE codes are case-sensitive letters and digits; drop everything else.
pub fn clean_code(raw: &str) -> String {
    raw.chars().filter(char::is_ascii_alphanumeric).collect()
}

/// Bytes of a `data:image/…;base64,…` URI. WE labels its JPEGs as PNG, so the
/// declared type is ignored.
pub fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    let b64 = uri.split_once(',').map_or(uri, |(_, data)| data).trim();
    if b64.is_empty() {
        return None;
    }
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

/// File name of the ddddocr model Klue uses (the default model of the Python package).
pub const MODEL_FILE: &str = "common_old.onnx";

/// Where Klue looks for the ddddocr model, in order:
/// `KLUE_OCR_MODEL`, `<data dir>/models/`, `models/` next to the executable,
/// and `assets/models/` in the working directory (for `cargo run`).
pub fn model_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(p) = std::env::var_os("KLUE_OCR_MODEL") {
        paths.push(PathBuf::from(p));
    }
    if let Some(dir) = crate::store::default_data_dir() {
        paths.push(dir.join("models").join(MODEL_FILE));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)) {
        paths.push(dir.join("models").join(MODEL_FILE));
    }
    paths.push(PathBuf::from("assets").join("models").join(MODEL_FILE));
    paths
}

pub fn find_model() -> Option<PathBuf> {
    model_search_paths().into_iter().find(|p| p.is_file())
}

/// The best captcha reader compiled in: Klue's WE model, else ddddocr when its model
/// file is present.
pub fn default_solver() -> Option<Arc<dyn CaptchaSolver>> {
    #[cfg(feature = "we-model")]
    match WeCaptchaModel::load() {
        Ok(model) => return Some(Arc::new(model)),
        Err(e) => tracing::warn!("{e}"),
    }
    #[cfg(any(feature = "ocr", feature = "ocr-dynamic"))]
    match DdddOcr::load_default() {
        Ok(ocr) => return Some(Arc::new(ocr)),
        Err(e) => tracing::warn!("ddddocr unavailable: {e}"),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_data_uris() {
        assert_eq!(decode_data_uri("data:image/png;base64,aGk=").unwrap(), b"hi");
        assert_eq!(decode_data_uri("aGk=").unwrap(), b"hi");
        assert!(decode_data_uri("data:image/png;base64,").is_none());
    }

    #[test]
    fn cleans_codes() {
        assert_eq!(clean_code(" a7-K 9\n"), "a7K9");
    }
}
