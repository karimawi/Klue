Klue keeps the quota, balance and renewal date of your Egyptian telecom lines in one window, and tells you before anything runs out. It works with WE home internet (DSL), landlines and WE Mobile, Vodafone mobile, and Orange mobile and home internet (DSL) lines.

This is a beta. If something looks off, please open an issue.

## New in 0.5.0
- WE landlines (Ardy) and WE Mobile lines: their minutes, their bundles, balance and recharges, read through the My WE account of your home internet
- Adding a number is a step-by-step wizard: pick the carrier and the kind of line, then sign in the way its app does. Lines Klue can't read yet (WE Air and WE Gold; Vodafone home internet and USB; e&) show as coming soon
- Signing in to a WE or Orange account finds its other lines too, and offers to add them: a WE account's landline and mobile lines, the home internet linked to an Orange mobile number, or the mobile line behind an Orange home internet account
- Export numbers to a file, one from its Edit dialog or several at once from Settings, with their settings, usage history and, if you like, their passwords sealed with a passphrase. Import the file on another computer to pick up where you left off
- Archived numbers have their own button under Add, and a bin beside Restore removes one
- Dialogs open at the top instead of where the last one was scrolled to
- When Orange says to try again later, home internet tries once more before giving up

## New in 0.4.1
- Archive a number you don't need checked for now: Klue stops refreshing it and hides it, but keeps its password
- Orange mobile lines show every bundle Orange lists: internet add-ons, extras, bonuses, Kart El Kebir units and roaming
- An Orange bundle that renews when you recharge shows as not active until then, without a low-quota alert
- Orange mobile lines show the date the line stays valid until
- Orange home internet counts the double-quota add-on

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
