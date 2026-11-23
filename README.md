<p align="center">
  <img src="crates/klue-app/icons/klue.svg" width="104" alt="Klue logo">
</p>

<h1 align="center">Klue</h1>

<p align="center">
  Your Egyptian telecom lines in one window: quota, balance, renewals and usage.
</p>

<p align="center">
  <a href="https://github.com/karimawi/Klue/releases"><img src="https://img.shields.io/github/v/release/karimawi/Klue?include_prereleases&label=download&color=FFBE32&labelColor=1D1B1B" alt="Download"></a>
  <img src="https://img.shields.io/badge/Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-1D1B1B" alt="Windows, macOS and Linux">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-1D1B1B" alt="GPL-3.0"></a>
</p>

<p align="center">
  <a href="https://klue.wkar.im">Website</a> ·
  <a href="https://github.com/karimawi/Klue/releases">Download</a> ·
  <a href="#build-from-source">Build from source</a>
</p>

<p align="center">
  <img src="docs/screenshots/dashboard-light-we.png" alt="Klue showing a WE home internet line" width="880">
</p>

## Features

- **Every line at a glance.** What's left, the package and its speed, the balance, the renewal date and the payment history, for each of your numbers.
- **Usage graphs.** What was left after every refresh and what you used per day or per hour, with averages and the day you'll run out.
- **Alerts that land.** Low-quota and renewal notifications with sound, per number or for all of them, even ones that came due while Klue was closed.
- **Arabic and English.** A fully mirrored right-to-left layout, using the carriers' own Arabic names.
- **Light, Dark and Match.** Match dresses the app in each carrier's colours.
- **Stays out of the way.** A tray quick view of every number, launch at startup, built-in updates.
- **Private.** Passwords stay in your system's credential store. Klue talks to your carrier and nothing else (apart from GitHub, to check for updates).

## Supported lines

| Carrier | Works today | Coming |
| --- | --- | --- |
| WE | Home internet (DSL) | Mobile, OneHome, Ardy |
| Vodafone | Mobile | DSL, USB |
| Orange | | DSL, Mobile |
| e& | | DSL, Mobile |

Also on the way: Klue Server, a Docker container that keeps watch around the clock on your own VPS, linked to the app with a key, with a Telegram bot and phone widgets.

## Screenshots

| | |
| --- | --- |
| ![Usage per day](docs/screenshots/usage-days.png) | ![Usage per hour, dark theme](docs/screenshots/usage-hours-dark.png) |
| ![Match theme, low quota](docs/screenshots/match-we-low-quota.png) | ![Dark theme, Vodafone](docs/screenshots/dark-vodafone.png) |
| ![Bundles and payments](docs/screenshots/vodafone-bundles-payments.png) | ![Arabic](docs/screenshots/arabic-rtl.png) |
| ![Add a number](docs/screenshots/add-number.png) | ![Alerts](docs/screenshots/alerts.png) |
| ![Borrowed Flex bundle](docs/screenshots/vodafone-flex-borrowed.png) | <img src="docs/screenshots/tray-quick-view.png" alt="Tray quick view" width="240"> |

## Download

Grab the latest build for Windows, macOS (Apple Silicon and Intel) or Linux (`.deb` and `.tar.gz`) from [Releases](https://github.com/karimawi/Klue/releases). The builds aren't code-signed yet; the release notes show how to open them the first time.

## Build from source

You need Rust 1.92 or newer with CMake and Clang (and NASM and LLVM on Windows).

```sh
cargo run --release      # the app
cargo run -- --demo      # try it with made-up numbers
cargo test -p klue-core --no-default-features
```

More in [docs/development.md](docs/development.md).

## License

[GPL-3.0](LICENSE). Klue is unofficial and isn't affiliated with WE, Vodafone, Orange or e&; their names and logos belong to them. DM Sans and Google Sans are under the SIL Open Font License, and the optional ddddocr model is MIT-licensed (© sml2h3).
