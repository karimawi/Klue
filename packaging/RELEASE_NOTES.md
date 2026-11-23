Klue keeps the quota, balance and renewal date of your Egyptian telecom lines in one window, and tells you before anything runs out. It works with WE home internet (DSL) and Vodafone mobile lines.

This is a beta. If something looks off, please open an issue.

## New in 0.3.0
- Usage graphs: what was left after every refresh, and what you used per day and per hour
- Daily and hourly averages, and the day your quota runs out at that rate
- WE package speeds, and arrows to flip through a number's bundles
- Alert settings for all numbers at once; alerts missed while Klue was closed arrive when it's back
- Notification sounds, an About page and built-in updates
- Klue shows up in Windows search

<!-- The app's update dialog stops here. -->

## Downloads

| System | File |
| --- | --- |
| Windows 10/11 (64-bit) | `Klue-…-windows-x64.zip`: unzip and run `Klue.exe` |
| macOS, Apple Silicon | `Klue-…-macos-arm64.dmg` |
| macOS, Intel | `Klue-…-macos-x64.dmg` |
| Ubuntu / Debian 22.04+ | `Klue-…-linux-x64.deb`: `sudo apt install ./Klue-…-linux-x64.deb` |
| Other Linux | `Klue-…-linux-x64.tar.gz` (needs X11 or Wayland, and `libxkbcommon-x11` on X11) |

## First launch

The builds aren't code-signed yet, so your system will warn you the first time:

- **Windows:** "Windows protected your PC" → **More info** → **Run anyway**.
- **macOS:** drag Klue to Applications and open it. When macOS refuses, go to **System Settings → Privacy & Security** and click **Open Anyway**. If it says the app is damaged, run `xattr -dr com.apple.quarantine /Applications/Klue.app` once.

Passwords stay in your system's credential store (Windows Credential Manager, macOS Keychain, Secret Service on Linux). On macOS a new version may ask to use them the first time: choose **Always Allow**.

Klue is unofficial and not affiliated with any carrier. Use it at your own risk.
