Klue shows the quota, package and renewal date of your Egyptian telecom lines in one window, with low-quota and renewal notifications.

**Works with:** WE home internet (landline) and Vodafone mobile lines. Orange and e& are coming soon.

## Downloads

| System | File |
| --- | --- |
| Windows 10/11 (64-bit) | `Klue-…-windows-x64.zip`: unzip and run `Klue.exe` |
| macOS, Apple Silicon (M1 or later) | `Klue-…-macos-arm64.dmg` |
| macOS, Intel | `Klue-…-macos-x64.dmg` |
| Ubuntu / Debian 22.04+ | `Klue-…-linux-x64.deb`: `sudo apt install ./Klue-…-linux-x64.deb` |
| Other Linux | `Klue-…-linux-x64.tar.gz`: needs the usual X11 or Wayland libraries (and `libxkbcommon-x11` on X11) |

## First launch

These builds aren't code-signed yet, so the OS will warn you the first time:

- **Windows:** "Windows protected your PC" → **More info** → **Run anyway**.
- **macOS:** drag Klue to Applications and open it once; when macOS refuses, go to **System Settings → Privacy & Security** and click **Open Anyway** (on macOS 14 and older, right-click Klue → **Open** also works). If macOS says the app is damaged, run `xattr -dr com.apple.quarantine /Applications/Klue.app` once.

Passwords are stored in your system's credential store (Windows Credential Manager, macOS Keychain, Secret Service on Linux), never in Klue's data file. Updates keep using them. On macOS, because the builds aren't signed yet, the first launch of a new version may ask to use Klue's saved passwords: choose **Always Allow**.

## Notes

- After a few sign-ins WE asks for a captcha. Klue reads it with its own model; when it isn't sure, it asks WE for a new captcha instead of guessing, and only asks you to type one if it can't read any of them.
- Klue's window is drawn natively (no web view). Closing it leaves Klue in the tray using little memory; starting Klue again opens the running one.
- Klue is unofficial and not affiliated with any carrier.
