#!/usr/bin/env bash
# Package a release build into dist/. Used by .github/workflows/release.yml.
#   packaging/package.sh <name> <rust target> <version>
#   e.g. packaging/package.sh macos-arm64 aarch64-apple-darwin 0.1.0
set -euo pipefail
name="$1"
target="$2"
version="${3#v}"
release="target/$target/release"
stage="dist/stage"
rm -rf "$stage"
mkdir -p "$stage" dist

case "$name" in
windows-*)
    mkdir -p "$stage/Klue"
    cp "$release/klue.exe" "$stage/Klue/Klue.exe"
    cp LICENSE "$stage/Klue/LICENSE.txt"
    (cd "$stage" && 7z a -tzip "../Klue-$version-$name.zip" Klue >/dev/null)
    ;;

macos-*)
    app="$stage/Klue.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$release/klue" "$app/Contents/MacOS/klue"
    sed "s/@VERSION@/$version/g" packaging/macos/Info.plist >"$app/Contents/Info.plist"
    iconset="$stage/Klue.iconset"
    mkdir -p "$iconset"
    for s in 16 32 128 256 512; do
        sips -z "$s" "$s" crates/klue-app/icons/klue-macos-1024.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
        sips -z $((s * 2)) $((s * 2)) crates/klue-app/icons/klue-macos-1024.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
    done
    iconutil -c icns "$iconset" -o "$app/Contents/Resources/Klue.icns"
    # Ad-hoc signature: required for Apple Silicon, not a Developer ID (see release notes).
    codesign --force --deep --sign - "$app"
    mkdir -p "$stage/dmg"
    cp -R "$app" "$stage/dmg/"
    ln -s /Applications "$stage/dmg/Applications"
    # hdiutil sometimes fails with "Resource busy" on CI machines; try a few times.
    for attempt in 1 2 3 4; do
        hdiutil create -volname "Klue" -srcfolder "$stage/dmg" -ov -format UDZO "dist/Klue-$version-$name.dmg" && break
        [ "$attempt" = 4 ] && exit 1
        sleep $((attempt * 5))
    done
    ;;

linux-*)
    dir="$stage/klue-$version"
    mkdir -p "$dir"
    cp "$release/klue" "$dir/klue"
    cp LICENSE packaging/linux/klue.desktop "$dir/"
    cp crates/klue-app/icons/klue-256.png "$dir/klue.png"
    tar -C "$stage" -czf "dist/Klue-$version-$name.tar.gz" "klue-$version"
    cargo deb -p klue-app --no-build --target "$target" --deb-version "$version" -o "dist/Klue-$version-$name.deb"
    ;;

*)
    echo "unknown package name: $name" >&2
    exit 1
    ;;
esac

rm -rf "$stage"
ls -la dist
