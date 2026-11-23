# Development

## Build and test

```sh
cargo run                    # the app (binary: klue)
cargo run -- --demo          # made-up numbers, nothing saved, no network
cargo run -- --debug         # with a console window showing the whole log
cargo test -p klue-core --no-default-features
cargo test -p klue-app
```

Linux needs `cmake clang pkg-config`; the GUI libraries are loaded at run time (minimal installs may need `libxkbcommon-x11-0`). Windows needs the Visual Studio Build Tools, CMake, LLVM and NASM. macOS needs the Xcode command line tools and CMake.

## Real accounts

```sh
KLUE_DEBUG_DIR=/tmp/klue-debug cargo run --release -p klue-core --example live_check -- accounts.env [--only we2]
```

`accounts.env` holds `TENUMBER`/`TEPASSWORD` and `VFNUMBER`/`VFPWD`, with `2`, `3`... appended for more accounts. `KLUE_DEBUG_DIR` saves every response with secrets blanked. Keep both out of git: they have names and numbers. WE counts failed sign-ins, so run each account once.

Behind a proxy that inspects TLS, point `SSL_CERT_FILE` or `SSL_CERT_DIR` at its CA.

## WE captchas

```sh
cargo run -p klue-core --example we_captcha -- 0223584417 --count 3
cargo test -p klue-core --lib we_model
```

The model is trained in `tools/captcha-model` and ships as `crates/klue-core/assets/we_captcha.onnx`. Real labelled samples live in `crates/klue-core/tests/captchas/`.

## Releasing

1. Bump `version` in the root `Cargo.toml`.
2. Push a `vX.Y.Z` tag, or run **Actions → Release → Run workflow**.

The workflow builds Windows, macOS and Linux packages and publishes them as a pre-release, or leaves a draft with the build logs if something fails. The release body is `packaging/RELEASE_NOTES.md`; the app's update dialog shows it up to the first HTML comment. The updater only offers releases built from `main`.

## iced notes

- No blurred shadows: tiny-skia redraws them unclipped and they pile up.
- Don't use `Text::align_x` on one-line Arabic text; let the container place it.
- In a mirrored row, a Fill-height child placed before content-sized siblings gets no height.
- Canvas text needs a finite `max_width`, or Arabic crashes tiny-skia.
