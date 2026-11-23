//! Windows: embeds the icon, publisher and version in klue.exe. Release builds take the
//! version from `KLUE_VERSION`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KLUE_VERSION");
    #[cfg(windows)]
    windows_resources();
}

/// Icon and version info for klue.exe.
#[cfg(windows)]
fn windows_resources() {
    println!("cargo:rerun-if-changed=icons/klue.ico");
    let version = std::env::var("KLUE_VERSION").unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());
    let mut res = winresource::WindowsResource::new();
    res.set_icon("icons/klue.ico")
        .set("ProductName", "Klue")
        .set("FileDescription", "Klue")
        .set("CompanyName", "Karim W. Almorsi")
        .set("LegalCopyright", "© 2026 Karim W. Almorsi")
        .set("ProductVersion", &version)
        .set("FileVersion", &version);
    let parts: Vec<u64> = version.split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect();
    if let [major, minor, patch] = parts[..] {
        let packed = major << 48 | minor << 32 | patch << 16;
        res.set_version_info(winresource::VersionInfo::FILEVERSION, packed).set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
    }
    if let Err(e) = res.compile() {
        println!("cargo:warning=couldn't embed the Windows icon: {e}");
    }
}
