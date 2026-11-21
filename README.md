# Klue

A desktop app that puts the quota, package and renewal date of all your Egyptian telecom lines in one window. It's written in Rust: [Dioxus](https://dioxuslabs.com) for the UI, and the carrier sign-in flows were reverse-engineered.

![Dashboard, light theme](docs/screenshots/dashboard-light-we.png)

| Carrier | Status |
| --- | --- |
| WE (Telecom Egypt) | Home internet (landline) |
| Vodafone | Mobile lines |
| Orange | Coming soon |
| e& | Coming soon |

## Features

- One tab per number, grouped by carrier. Each number shows its registered owner, an optional nickname, the package, GB left with percentages, the subscription date and the next renewal.
- Quota alerts at X GB or X% left, per bundle, and a renewal reminder X days or hours before renewal. Both arrive as desktop notifications.
- Adding a number signs in with the carrier's own password, checks it, and fills in the owner name from the account.
- Light, Dark and Match themes. Match recolors the app with each carrier's brand colors.
- English and Arabic, with a fully mirrored right-to-left layout. Fonts are DM Sans and Google Sans, bundled with the app.
- A tray icon with a quick-view popup. Optional launch at startup, minimized to the tray.
- Passwords live in the OS credential store (Windows Credential Manager, macOS Keychain or Secret Service), never in Klue's data file.

| | |
| --- | --- |
| ![Match theme, low quota](docs/screenshots/match-we-low-quota.png) | ![Dark theme, Vodafone](docs/screenshots/dark-vodafone.png) |
| ![Arabic](docs/screenshots/arabic-rtl.png) | ![Add a number](docs/screenshots/add-number-verified.png) |
| ![Alerts](docs/screenshots/alerts.png) | ![Coming soon](docs/screenshots/coming-soon.png) |

## Download

Builds for Windows, macOS (Apple Silicon and Intel) and Linux (`.deb` and `.tar.gz`) are on the [Releases](https://github.com/karimawi/Klue/releases) page. They aren't code-signed yet; the release notes explain how to open them the first time.

Klue draws its own title bar on every platform. On Windows and Linux the window is frameless; on macOS it keeps the native window shape and edges, with the system title bar made transparent and its buttons hidden.

## Building

You need Rust 1.92 or newer plus a few native tools. `wreq` builds BoringSSL so that requests carry Chrome's TLS fingerprint.

- **Windows:** Visual Studio Build Tools (C++ workload), [CMake](https://cmake.org) and [LLVM](https://github.com/llvm/llvm-project/releases), which provides `libclang` for bindgen: `winget install Kitware.CMake LLVM.LLVM`. WebView2 is already part of Windows 10 and 11.
- **Linux:** `cmake clang pkg-config libssl-dev libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev libdbus-1-dev`
- **macOS:** Xcode command line tools and `cmake`.

```sh
cargo run --release            # the app (binary: klue)
cargo run -- --demo            # sample numbers from the mockups, nothing saved, no network
cargo test -p klue-core --no-default-features
```

## WE captchas

After a few sign-ins, WE's `GenerateCaptcha` starts returning an image: five red characters with a blue line struck through them. Klue reads it with its own model, trained only on this captcha (`tools/captcha-model`). It is a small CNN (0.4 M parameters, 1.7 MB), embedded in the app and run with [tract](https://github.com/sonos/tract), a pure-Rust ONNX runtime, so nothing is downloaded and no native ML library is needed.

The model also reports how sure it is. Below 0.7 it doesn't guess, because every wrong answer uses up one of the few attempts WE allows before blocking sign-ins. Instead, Klue asks you to type the code when you're adding a number or pressing **Refresh**, and skips that attempt during background refreshes. If WE answers `Blocked`, Klue waits an hour before trying that number again.

Results on the 15 real captchas collected so far (`crates/klue-core/tests/captchas`; the model never trained on them):

| Reader | Exact | Ignoring case | Wrong answers it would submit |
| --- | --- | --- | --- |
| ddddocr (`common_old.onnx`) | 9/15 | 15/15 | 6 |
| Klue's WE model | **15/15** | 15/15 | **0** |

ddddocr reads the letters but guesses the case of `C O S V W X Z` (and, in the serif font, `Y`), which differ from their other case only in size. WE's codes are case-sensitive, and most 5-character codes contain at least one such letter. Klue's model sees the image at its real scale. Of the 15, it reads 14 with confidence above 0.7; the fifteenth (`G5DZC`, a serif captcha with an odd raised `5`) it reads correctly but at 0.63, so Klue would ask you to type it rather than risk an attempt. On 2,000 fresh synthetic captchas it answers 95.5% of the time and is right 98.2% of the time when it does.

WE draws most captchas in Arial and some in Times New Roman (2 of the 15). Characters sit on a fixed 20 px pitch, so wide letters overlap their neighbours, and the font size varies from one captcha to the next.

Fifteen real samples are encouraging but not proof. If WE changes its generator, or the model starts declining often, collect more real captchas and fine-tune on them:

```sh
cargo run -p klue-core --example we_captcha -- 0223584417 --count 20            # saves fresh captchas to captchas/
cargo run -p klue-core --example we_captcha -- 0223584417 --password '…'        # also signs in; accepted codes land in captchas/labeled/
cargo run -p klue-core --example we_captcha -- --image captchas/a.jpg           # read saved images offline
```

Then follow `tools/captcha-model/README.md` to fine-tune. ddddocr is still available as an optional extra solver: build with `--features ocr` and fetch its model with `cargo run -p klue-core --example fetch_ocr_model`. That feature downloads ONNX Runtime from cdn.pyke.io at build time; `ocr-dynamic` loads it from `ORT_DYLIB_PATH` instead.

## Releasing

Push a version tag and GitHub Actions builds and publishes the release (`.github/workflows/release.yml`):

```sh
git tag v0.1.1 && git push origin v0.1.1
```

It builds `klue` on Windows, macOS (arm64 and x64) and Ubuntu 22.04, packages it with `packaging/package.sh` (zip, `.dmg` with `Klue.app`, `.deb` and `.tar.gz`) and attaches the files to a pre-release. If a build fails, the release stays a draft with that build's log attached. Bump `version` in the root `Cargo.toml` first.

## Debugging the carrier parsers

The field names for the registrant name (WE) and the package name are not documented; the parsers try the likely names in order. To see what your account actually returns, run with `KLUE_DEBUG_DIR=debug`. Every carrier response is then written under `debug/`, with passwords, tokens and captcha images redacted. The parsers have fixtures in `crates/klue-core/tests/fixtures/`.

## Layout

```
crates/klue-core     carrier clients (WE, Vodafone), captcha readers, storage, alert rules; no UI
crates/klue-app      Dioxus desktop app: window, tray, notifications, launch at startup
tools/captcha-model  synthetic WE captcha generator and model training (Python, PyTorch)
packaging/           release packaging (macOS bundle, Linux desktop entry, release notes)
design/              UI mockups
```

Data lives in `%APPDATA%\Klue\data\klue.json` on Windows, `~/.local/share/klue/` on Linux, and `~/Library/Application Support/Klue/` on macOS.

## Notices

Klue is unofficial and not affiliated with any carrier. Carrier names and logos belong to their owners. DM Sans and Google Sans are under the SIL Open Font License 1.1. The ddddocr model and charset are MIT-licensed (© sml2h3).
