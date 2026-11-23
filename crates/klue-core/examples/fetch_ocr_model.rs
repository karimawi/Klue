//! Downloads ddddocr's `common_old.onnx` from its PyPI wheel into Klue's model folder.
//!
//! ```text
//! cargo run -p klue-core --example fetch_ocr_model
//! cargo run -p klue-core --example fetch_ocr_model -- assets/models
//! ```

use std::{io::Read, path::PathBuf};

use sha2::{Digest, Sha256};

const WHEEL_URL: &str = "https://files.pythonhosted.org/packages/0e/48/cbaed3981b8d8d51141b9b4779b811f4728e65d952a1e3e2e5e929539183/ddddocr-1.6.1-py3-none-any.whl";
const WHEEL_SHA256: &str = "c7c70f4ae2d0335440ae8b272eea48c9f6888ecef46785fe2311f0c97a133935";
const MODEL_IN_WHEEL: &str = "ddddocr/common_old.onnx";
const MODEL_SHA256: &str = "b8f2ad9cbc1f2e3922a6cb9459e30824e7e2467f3fb4fd61420640e34ea0bf68";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| klue_core::store::default_data_dir().map(|d| d.join("models")))
        .ok_or("no data directory; pass an output directory")?;
    let dest = dir.join(klue_core::captcha::MODEL_FILE);
    if dest.is_file() && hex(&Sha256::digest(std::fs::read(&dest)?)) == MODEL_SHA256 {
        println!("{} is already in place", dest.display());
        return Ok(());
    }

    println!("downloading ddddocr 1.6.1 (76 MB) from PyPI...");
    let client = wreq::Client::builder().build()?;
    let wheel = client.get(WHEEL_URL).send().await?.error_for_status()?.bytes().await?;
    if hex(&Sha256::digest(&wheel)) != WHEEL_SHA256 {
        return Err("checksum mismatch for the downloaded wheel".into());
    }

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(wheel))?;
    let mut model = Vec::new();
    archive.by_name(MODEL_IN_WHEEL)?.read_to_end(&mut model)?;
    if hex(&Sha256::digest(&model)) != MODEL_SHA256 {
        return Err("checksum mismatch for common_old.onnx".into());
    }

    std::fs::create_dir_all(&dir)?;
    std::fs::write(&dest, model)?;
    println!("saved {}", dest.display());
    Ok(())
}
