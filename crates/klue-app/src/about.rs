//! Who makes Klue and where to find it, and this build's version.

/// This build's version. Release builds take it from their tag (the release workflow
/// sets `KLUE_VERSION`); other builds use the version in Cargo.toml.
pub const VERSION: &str = match option_env!("KLUE_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};
pub const AUTHOR: &str = "Karim W. Almorsi";
pub const REPO: &str = "https://github.com/karimawi/Klue";
pub const WEBSITE: &str = "https://wkar.im";

/// Open an https link in the default browser.
pub fn open_url(url: &'static str) {
    open_url_owned(url);
}

/// [`open_url`] for a link that isn't a constant (a release page from GitHub).
pub fn open_url_owned(url: &str) {
    if !url.starts_with("https://") {
        return;
    }
    let result = open_command(url).spawn();
    if let Err(e) = result {
        tracing::warn!("couldn't open {url}: {e}");
    }
}

fn open_command(url: &str) -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]).creation_flags(CREATE_NO_WINDOW);
        c
    }
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    }
}
