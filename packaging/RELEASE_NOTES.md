Klue keeps the quota, balance and renewal date of your Egyptian telecom lines in one window, and tells you before anything runs out. It works with WE home internet (DSL), Vodafone mobile, and Orange mobile and home internet (DSL) lines.

This is a beta. If something looks off, please open an issue.

## New in 0.4.1
- Archive a number you don't need checked for now: Klue stops refreshing it and hides it but keeps its password, and Settings → Archived numbers brings it back
- Orange mobile lines show every bundle Orange lists: internet add-ons, extras, bonuses, Kart El Kebir units and roaming
- An Orange bundle that renews when you recharge shows as not active until then, without a low-quota alert
- Orange mobile lines show the date the line stays valid until
- Orange home internet counts the double-quota add-on

## New in 0.4.0
- Orange: mobile lines, and home internet (DSL) through your My Orange account, which finds the linked landline
- Each number can refresh on its own schedule, and the title bar shows when the number you're looking at last updated
- A 3-hour option for how often Klue checks
- Drag carriers and numbers to put them in your order
- 12-hour time, if you prefer it (Settings → Time format)
- The usage graph's tooltips no longer leave thin lines behind on Windows
- The Orange logo fits its tile

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
