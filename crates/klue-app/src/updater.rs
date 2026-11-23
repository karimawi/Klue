//! Installing an update.
//!
//! Klue never kills a process to replace its own program file. On Windows and with the
//! Linux archive it moves the running program aside (Windows allows renaming a running
//! .exe), puts the new one at the same path, starts it with `--after-update`, and quits
//! normally. The new Klue waits until the old one has gone
//! ([`crate::instance::wait_until_gone`]) before it takes over, then deletes the old file
//! ([`cleanup`]). A macOS disk image or a Linux package is handed to the system instead.

use std::path::{Path, PathBuf};

/// What [`install`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The new version is in place and starting: quit now.
    Restart,
    /// The download was opened for the system to install (a disk image or a package).
    Opened,
}

/// The release file this build installs from, in order of preference.
pub fn suffixes() -> &'static [&'static str] {
    if cfg!(windows) {
        &["-windows-x64.zip"]
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        &["-macos-arm64.dmg"]
    } else if cfg!(target_os = "macos") {
        &["-macos-x64.dmg"]
    } else if replaceable() {
        &["-linux-x64.tar.gz", "-linux-x64.deb"]
    } else {
        &["-linux-x64.deb"]
    }
}

/// Where downloads go: a folder of their own in the temp folder.
pub fn download_dir(version: &str) -> PathBuf {
    std::env::temp_dir().join(format!("klue-update-{version}"))
}

/// Install a downloaded release file.
pub fn install(file: &Path) -> Result<Outcome, String> {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
    if name.ends_with(".zip") {
        let new = extract_exe_from_zip(file)?;
        replace_running(&new)?;
        relaunch()?;
        Ok(Outcome::Restart)
    } else if name.ends_with(".tar.gz") {
        let new = extract_from_tarball(file)?;
        replace_running(&new)?;
        relaunch()?;
        Ok(Outcome::Restart)
    } else if name.ends_with(".dmg") || name.ends_with(".deb") {
        open_file(file)?;
        Ok(Outcome::Opened)
    } else {
        Err(format!("don't know how to install {name}"))
    }
}

/// The program file an update moved aside: `Klue.exe.old` next to `Klue.exe`.
fn old_path(exe: &Path) -> PathBuf {
    let mut s = exe.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}

fn current_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("can't find Klue's program file: {e}"))?;
    // Replace the file itself, not a link to it.
    Ok(std::fs::canonicalize(&exe).map(strip_verbatim).unwrap_or(exe))
}

/// `\\?\C:\…` (what canonicalize gives on Windows) as `C:\…`.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path,
    }
}

/// Whether Klue's own folder can be written, so its program can be replaced in place.
fn replaceable() -> bool {
    let Some(dir) = current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) else { return false };
    let probe = dir.join(format!(".klue-write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Move the running program aside and copy `new` to its path.
fn replace_running(new: &Path) -> Result<(), String> {
    replace(&current_exe()?, new)
}

/// Move `exe` aside (to [`old_path`]) and copy `new` to its path; on failure put it back.
fn replace(exe: &Path, new: &Path) -> Result<(), String> {
    let exe = exe.to_path_buf();
    let old = old_path(&exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|e| format!("couldn't move {} aside: {e}", exe.display()))?;
    if let Err(e) = std::fs::copy(new, &exe) {
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::rename(&old, &exe);
        return Err(format!("couldn't put the new Klue at {}: {e}", exe.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

/// Start the new Klue. It waits for this one to quit before it takes over.
fn relaunch() -> Result<(), String> {
    let exe = current_exe()?;
    let mut command = std::process::Command::new(&exe);
    command.arg("--after-update");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    command.spawn().map(|_| ()).map_err(|e| format!("couldn't start the new Klue: {e}"))
}

/// After an update: delete the old program file it left. Klue waited for the old
/// process to quit first, so the file is free.
pub fn cleanup() {
    let Ok(exe) = current_exe() else { return };
    let old = old_path(&exe);
    if old.exists() {
        match std::fs::remove_file(&old) {
            Ok(()) => tracing::info!("removed {}", old.display()),
            Err(e) => tracing::warn!("couldn't remove {}: {e}", old.display()),
        }
    }
}

/// `Klue/Klue.exe` from the Windows release archive, next to the archive.
fn extract_exe_from_zip(zip_path: &Path) -> Result<PathBuf, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("the download isn't a zip archive: {e}"))?;
    let index = (0..archive.len())
        .find(|&i| archive.by_index(i).is_ok_and(|f| f.name().to_ascii_lowercase().ends_with("klue.exe")))
        .ok_or("the archive has no Klue.exe")?;
    let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
    let out = zip_path.with_file_name("Klue.exe.new");
    let mut target = std::fs::File::create(&out).map_err(|e| e.to_string())?;
    std::io::copy(&mut entry, &mut target).map_err(|e| e.to_string())?;
    Ok(out)
}

/// `klue-<version>/klue` from the Linux archive, unpacked next to it with `tar`.
fn extract_from_tarball(tarball: &Path) -> Result<PathBuf, String> {
    let dir = tarball.parent().ok_or("no folder for the download")?.join("unpacked");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let status = std::process::Command::new("tar").arg("-xzf").arg(tarball).arg("-C").arg(&dir).status().map_err(|e| format!("couldn't run tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar failed ({status})"));
    }
    std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path().join("klue"))
        .find(|p| p.is_file())
        .ok_or_else(|| "the archive has no klue program".into())
}

/// Hand a disk image or package to the system.
fn open_file(file: &Path) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(opener).arg(file).spawn().map(|_| ()).map_err(|e| format!("couldn't open {}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_program_sits_next_to_the_new_one() {
        assert_eq!(old_path(Path::new("/opt/klue/klue")), PathBuf::from("/opt/klue/klue.old"));
        assert_eq!(strip_verbatim(PathBuf::from(r"\\?\C:\Klue\Klue.exe")), PathBuf::from(r"C:\Klue\Klue.exe"));
        assert_eq!(strip_verbatim(PathBuf::from(r"\\?\UNC\server\share")), PathBuf::from(r"\\?\UNC\server\share"));
    }

    #[test]
    fn replaces_a_program_in_place_and_keeps_the_old_one_aside() {
        let dir = std::env::temp_dir().join(format!("klue-updater-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (exe, new) = (dir.join("klue"), dir.join("klue.new"));
        std::fs::write(&exe, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        // A leftover from an earlier update doesn't get in the way.
        std::fs::write(old_path(&exe), b"older").unwrap();
        replace(&exe, &new).unwrap();
        assert_eq!((std::fs::read(&exe).unwrap(), std::fs::read(old_path(&exe)).unwrap()), (b"new".to_vec(), b"old".to_vec()));
        // Nothing to copy: the old program goes back where it was.
        std::fs::remove_file(&new).unwrap();
        assert!(replace(&exe, &new).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"new".to_vec());
        let _ = std::fs::remove_dir_all(dir);
    }
}
