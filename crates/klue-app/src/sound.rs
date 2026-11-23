//! Notification sounds: `negative.wav` for warnings (a quota running low) and
//! `notification.wav` for everything else (renewals). Klue plays them itself rather than
//! through the notification, which can't carry a custom sound everywhere (Windows doesn't
//! let an unpackaged app give its toasts one), and the notifications are sent silent.

static NOTIFICATION: &[u8] = include_bytes!("../assets/sounds/notification.wav");
static NEGATIVE: &[u8] = include_bytes!("../assets/sounds/negative.wav");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Something needs attention: a quota is running low.
    Warning,
    /// Everything else: renewals, reminders.
    Notice,
}

impl Tone {
    fn wav(self) -> &'static [u8] {
        match self {
            Tone::Warning => NEGATIVE,
            Tone::Notice => NOTIFICATION,
        }
    }

    fn file_name(self) -> &'static str {
        match self {
            Tone::Warning => "negative.wav",
            Tone::Notice => "notification.wav",
        }
    }
}

/// The tone for several notifications sent together: one sound, a warning if any is one.
pub fn for_all(tones: impl IntoIterator<Item = Tone>) -> Option<Tone> {
    tones.into_iter().reduce(|a, b| if a == Tone::Warning || b == Tone::Warning { Tone::Warning } else { Tone::Notice })
}

/// Play `tone` without waiting for it to finish.
pub fn play(tone: Tone) {
    #[cfg(windows)]
    windows::play(tone);
    #[cfg(not(windows))]
    std::thread::spawn(move || {
        if let Err(e) = play_file(tone) {
            tracing::debug!("couldn't play {}: {e}", tone.file_name());
        }
    });
}

/// macOS and Linux play a file with the system's player: the sounds are written next to
/// each other in the temp folder once per run.
#[cfg(not(windows))]
fn play_file(tone: Tone) -> std::io::Result<()> {
    use std::{
        process::{Command, Stdio},
        sync::OnceLock,
    };

    static DIR: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    let dir = DIR
        .get_or_init(|| {
            let dir = std::env::temp_dir().join(format!("klue-sounds-{}", std::process::id()));
            std::fs::create_dir_all(&dir).ok()?;
            for t in [Tone::Warning, Tone::Notice] {
                std::fs::write(dir.join(t.file_name()), t.wav()).ok()?;
            }
            Some(dir)
        })
        .as_ref()
        .ok_or_else(|| std::io::Error::other("no temp folder for the sounds"))?;
    let path = dir.join(tone.file_name());
    let players: &[&[&str]] = if cfg!(target_os = "macos") { &[&["afplay"]] } else { &[&["paplay"], &["pw-play"], &["aplay", "-q"]] };
    let mut last = std::io::Error::other("no sound player");
    for player in players {
        let mut command = Command::new(player[0]);
        command.args(&player[1..]).arg(&path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        match command.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => last = std::io::Error::other(format!("{} exited with {status}", player[0])),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Remove this run's copies of the sounds (on quit).
pub fn cleanup() {
    #[cfg(not(windows))]
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("klue-sounds-{}", std::process::id())));
}

#[cfg(windows)]
mod windows {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

    use super::Tone;

    /// Plays from memory and returns at once; a new sound stops the one playing.
    pub fn play(tone: Tone) {
        // With SND_MEMORY the "name" is the WAV image itself, which must outlive the
        // sound: it's static.
        let wav = tone.wav();
        let ok = unsafe { PlaySoundW(wav.as_ptr().cast(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
        if ok == 0 {
            tracing::debug!("couldn't play {}", tone.file_name());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_sound_for_several_notifications() {
        assert_eq!(for_all([]), None);
        assert_eq!(for_all([Tone::Notice, Tone::Notice]), Some(Tone::Notice));
        assert_eq!(for_all([Tone::Notice, Tone::Warning, Tone::Notice]), Some(Tone::Warning));
    }

    #[test]
    fn the_sounds_are_wav_files() {
        for tone in [Tone::Warning, Tone::Notice] {
            let wav = tone.wav();
            assert_eq!((&wav[..4], &wav[8..12]), (&b"RIFF"[..], &b"WAVE"[..]));
        }
    }
}
