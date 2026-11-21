//! Linux link fix. dioxus-desktop links the system OpenSSL (tungstenite → native-tls),
//! while wreq links BoringSSL as static `libssl.a`/`libcrypto.a` with prefixed symbols
//! (wreq's `prefix-symbols` feature). Both ask the linker for `-lssl`, and BoringSSL's
//! search path comes first, so OpenSSL's symbols go missing. Passing the system shared
//! libraries explicitly resolves them. Windows and macOS use their native TLS stacks
//! for native-tls and aren't affected.

use std::{path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    let libdir = Command::new("pkg-config")
        .args(["--variable=libdir", "openssl"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .filter(|p| p.is_dir());
    let candidates = libdir.into_iter().chain(["/usr/lib/x86_64-linux-gnu", "/usr/lib64", "/usr/lib"].map(PathBuf::from));
    for dir in candidates {
        let (ssl, crypto) = (dir.join("libssl.so"), dir.join("libcrypto.so"));
        if ssl.exists() && crypto.exists() {
            println!("cargo:rustc-link-arg={}", ssl.display());
            println!("cargo:rustc-link-arg={}", crypto.display());
            return;
        }
    }
    println!("cargo:warning=system OpenSSL not found; install libssl-dev if linking fails");
}
