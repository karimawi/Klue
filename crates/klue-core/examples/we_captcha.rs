//! Experiment harness for WE's captcha and ddddocr.
//!
//! ```text
//! # Ask WE for captchas for a landline, save them and show ddddocr's reading:
//! cargo run -p klue-core --example we_captcha -- 0223584417 --count 5
//!
//! # Also sign in with each reading, to see whether WE accepts it
//! # (every attempt counts towards WE's limit, so keep --count small):
//! cargo run -p klue-core --example we_captcha -- 0223584417 --password '...'
//!
//! # Run ddddocr on saved images, offline:
//! cargo run -p klue-core --example we_captcha -- --image captchas/a.jpg --image captchas/b.jpg
//! ```
//!
//! Flags: `--out <dir>` (default `captchas/`), `--dump <dir>` (write raw WE responses,
//! secrets redacted), `--raw` (don't limit OCR output to letters and digits),
//! `--unit-norm` (ddddocr 1.6 pixel scaling instead of 1.4/1.5).

use std::{path::PathBuf, time::Duration};

use klue_core::carriers::{FetchContext, we::WeSession};

struct Args {
    number: Option<String>,
    password: Option<String>,
    count: usize,
    out: PathBuf,
    dump: Option<PathBuf>,
    images: Vec<PathBuf>,
    raw: bool,
    unit_norm: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        number: None,
        password: None,
        count: 1,
        out: PathBuf::from("captchas"),
        dump: std::env::var_os("KLUE_DEBUG_DIR").map(PathBuf::from),
        images: Vec::new(),
        raw: false,
        unit_norm: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--password" => args.password = it.next(),
            "--count" => args.count = it.next().and_then(|c| c.parse().ok()).unwrap_or(1),
            "--out" => args.out = it.next().map(PathBuf::from).unwrap_or(args.out),
            "--dump" => args.dump = it.next().map(PathBuf::from),
            "--image" => args.images.extend(it.next().map(PathBuf::from)),
            "--raw" => args.raw = true,
            "--unit-norm" => args.unit_norm = true,
            "-h" | "--help" => {
                println!("{}", include_str!("we_captcha.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!").trim_start_matches(' ')).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            other if !other.starts_with('-') => args.number = Some(other.to_string()),
            other => eprintln!("ignoring unknown flag {other}"),
        }
    }
    args
}

#[cfg(any(feature = "ocr", feature = "ocr-dynamic"))]
fn load_ocr(args: &Args) -> Option<klue_core::captcha::DdddOcr> {
    use klue_core::captcha::{DdddOcr, Normalization, OcrOptions};
    match DdddOcr::load_default() {
        Ok(ocr) => Some(ocr.with_options(OcrOptions {
            normalization: if args.unit_norm { Normalization::Unit } else { Normalization::Centered },
            alnum_only: !args.raw,
        })),
        Err(e) => {
            eprintln!("ddddocr unavailable: {e}");
            None
        }
    }
}

#[cfg(not(any(feature = "ocr", feature = "ocr-dynamic")))]
fn load_ocr(_: &Args) -> Option<NoOcr> {
    eprintln!("built without the `ocr` feature; images will be saved but not read");
    None
}

#[cfg(not(any(feature = "ocr", feature = "ocr-dynamic")))]
struct NoOcr;
#[cfg(not(any(feature = "ocr", feature = "ocr-dynamic")))]
impl NoOcr {
    fn classify(&self, _: &[u8]) -> Result<String, String> {
        Err("no OCR".into())
    }
}

fn extension(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xFF, 0xD8, ..] => "jpg",
        [0x89, b'P', b'N', b'G', ..] => "png",
        [b'G', b'I', b'F', ..] => "gif",
        _ => "bin",
    }
}

#[tokio::main]
async fn main() {
    let args = parse_args();
    let ocr = load_ocr(&args);

    if !args.images.is_empty() {
        for path in &args.images {
            match std::fs::read(path) {
                Ok(bytes) => match ocr.as_ref().map(|o| o.classify(&bytes)) {
                    Some(Ok(text)) => println!("{}\t{text}", path.display()),
                    Some(Err(e)) => println!("{}\terror: {e}", path.display()),
                    None => println!("{}\t(no OCR)", path.display()),
                },
                Err(e) => println!("{}\tunreadable: {e}", path.display()),
            }
        }
        return;
    }

    let Some(number) = args.number.clone() else {
        eprintln!("usage: we_captcha <landline number> [--count N] [--password PW] | --image FILE");
        std::process::exit(2);
    };
    let ctx = FetchContext { solver: None, debug_dir: args.dump.clone() };
    std::fs::create_dir_all(&args.out).expect("create output directory");

    let (mut asked, mut accepted) = (0, 0);
    for i in 1..=args.count {
        let session = match WeSession::new(&number) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        };
        session.warm_up().await;
        let challenge = match session.captcha(&ctx).await {
            Ok(c) => c,
            Err(e) => {
                println!("#{i}: {e}");
                break;
            }
        };
        let guess = match &challenge.image {
            None => {
                println!("#{i}: WE didn't ask for a captcha this time (status {:?})", challenge.status);
                None
            }
            Some(image) => {
                asked += 1;
                let file = args.out.join(format!("we-{}-{i}.{}", chrono::Utc::now().format("%Y%m%dT%H%M%S"), extension(image)));
                std::fs::write(&file, image).expect("save captcha");
                let guess = ocr.as_ref().and_then(|o| o.classify(image).ok());
                println!("#{i}: saved {} - ddddocr reads {:?}", file.display(), guess.as_deref().unwrap_or("-"));
                guess
            }
        };
        if let Some(password) = &args.password {
            match session.sign_in(password, &challenge, guess.as_deref(), &ctx).await {
                Ok(auth) => {
                    accepted += 1;
                    println!("#{i}: signed in (owner name: {:?})", auth.owner_name);
                    if i == args.count {
                        match session.read_usage(&auth, &ctx).await {
                            Ok(usage) => println!("{usage:#?}"),
                            Err(e) => println!("reading usage failed: {e}"),
                        }
                    }
                }
                Err(e) => println!("#{i}: sign-in failed: {e}"),
            }
        }
        if i < args.count {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
    if args.password.is_some() {
        println!("\ncaptchas shown: {asked}, sign-ins accepted: {accepted}/{}", args.count);
    }
}
