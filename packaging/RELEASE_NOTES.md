Klue shows the quota, package and renewal date of your Egyptian telecom lines in one window, with low-quota and renewal notifications.

**Works with:** WE home internet (landline) and Vodafone mobile lines. Orange and e& are coming soon.

## Downloads

| System | File |
| --- | --- |
| Windows 10/11 (64-bit) | `Klue-…-windows-x64.zip`: unzip and run `Klue.exe` |
| macOS, Apple Silicon (M1 or later) | `Klue-…-macos-arm64.dmg` |
| macOS, Intel | `Klue-…-macos-x64.dmg` |
| Ubuntu / Debian 22.04+ | `Klue-…-linux-x64.deb`: `sudo apt install ./Klue-…-linux-x64.deb` |
| Other Linux | `Klue-…-linux-x64.tar.gz`: needs WebKitGTK 4.1 and libayatana-appindicator3 |

## First launch

These builds aren't code-signed yet, so the OS will warn you the first time:

- **Windows:** "Windows protected your PC" → **More info** → **Run anyway**.
- **macOS:** drag Klue to Applications, then right-click it → **Open** → **Open**. If macOS says the app is damaged, run `xattr -dr com.apple.quarantine /Applications/Klue.app` once.

Passwords are stored in your system's credential store (Windows Credential Manager, macOS Keychain, Secret Service on Linux), never in Klue's data file.

## Notes

- After a few sign-ins WE asks for a captcha. Klue reads it with its own model; when it isn't sure, it asks you to type the code instead of guessing.
- Klue is unofficial and not affiliated with any carrier.
