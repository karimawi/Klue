//! Windows: Klue ships as a folder (no installer), so on launch it adds a Start menu
//! shortcut and an App Paths entry for search and `start klue`. Per user, and redone only
//! if Klue moved.

/// No-op outside Windows.
pub fn register() {
    #[cfg(windows)]
    if let Err(e) = windows::register() {
        tracing::warn!("couldn't add Klue to the Start menu: {e}");
    }
}

#[cfg(windows)]
mod windows {
    use std::{error::Error, path::PathBuf};

    const APP_PATHS: &str = r"Software\Microsoft\Windows\CurrentVersion\App Paths\Klue.exe";

    pub fn register() -> Result<(), Box<dyn Error>> {
        let exe = std::env::current_exe()?;
        let exe_str = exe.to_str().ok_or("Klue's path isn't valid Unicode")?.to_string();
        let dir = exe.parent().and_then(|d| d.to_str()).ok_or("Klue's folder isn't valid Unicode")?.to_string();
        let programs = PathBuf::from(std::env::var_os("APPDATA").ok_or("no APPDATA")?).join(r"Microsoft\Windows\Start Menu\Programs");
        let shortcut = programs.join("Klue.lnk");

        let key = windows_registry::CURRENT_USER.create(APP_PATHS)?;
        if key.get_string("").ok().as_deref() == Some(exe_str.as_str()) && shortcut.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(&programs)?;
        let mut link = mslnk::ShellLink::new(&exe)?;
        link.set_name(Some("Klue: your telecom quotas".into()));
        link.set_working_dir(Some(dir.clone()));
        link.set_icon_location(Some(exe_str.clone()));
        link.create_lnk(&shortcut)?;
        key.set_string("", &exe_str)?;
        key.set_string("Path", &dir)?;
        tracing::info!("added Klue to the Start menu ({})", shortcut.display());
        Ok(())
    }
}
