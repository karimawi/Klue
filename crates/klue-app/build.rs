//! Windows: embeds the app icon and version info in klue.exe.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    windows_resources();
}

/// Icon and version info for klue.exe (Explorer, taskbar, Alt+Tab).
#[cfg(windows)]
fn windows_resources() {
    println!("cargo:rerun-if-changed=icons/klue.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("icons/klue.ico").set("ProductName", "Klue").set("FileDescription", "Klue");
    if let Err(e) = res.compile() {
        println!("cargo:warning=couldn't embed the Windows icon: {e}");
    }
}
