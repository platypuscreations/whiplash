#!/bin/bash
# Builds Whiplash Player.app in target/release/bundle, and with --dmg also a disk
# image for installing it.
#
# Needs: Rust, CMake, Xcode's command line tools, and rsvg-convert to draw
# the icon (`brew install cmake librsvg`).
#
# The app is signed "ad hoc": enough for it to run on this Mac. On another
# Mac, macOS will warn that it's from an unidentified developer (right-click
# the app and choose Open the first time). Signing for distribution needs a
# paid Apple Developer ID and notarization.
set -euo pipefail

cd "$(dirname "$0")/../.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
bundle=target/release/bundle
app="$bundle/Whiplash Player.app"

echo "Building Whiplash $version..."
cargo build --release

echo "Assembling $app..."
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/whiplash "$app/Contents/MacOS/whiplash"
# macOS wants plain numbers here: 0.4.0 for a release candidate 0.4.0-rc1.
sed "s/@VERSION@/${version%%-*}/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
# The license, and the notices and license texts for what's included.
cp LICENSE THIRD-PARTY-NOTICES.md "$app/Contents/Resources/"
cp -R licenses "$app/Contents/Resources/"

# The icon: draw the SVG at each size macOS wants, then pack them.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
iconset="$work/whiplash.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
    rsvg-convert -w "$size" -h "$size" packaging/icon.svg -o "$iconset/icon_${size}x${size}.png"
    rsvg-convert -w $((size * 2)) -h $((size * 2)) packaging/icon.svg -o "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/whiplash.icns"

codesign --force --sign - "$app"
echo "Built $app"

if [[ "${1:-}" == "--dmg" ]]; then
    # A disk image showing the app next to a shortcut to Applications, so
    # installing is a drag.
    dmg="$bundle/whiplash-$version.dmg"
    stage="$work/dmg"
    mkdir "$stage"
    cp -R "$app" "$stage/"
    ln -s /Applications "$stage/Applications"
    rm -f "$dmg"
    hdiutil create -quiet -volname "Whiplash Player $version" -srcfolder "$stage" -format UDZO "$dmg"
    echo "Built $dmg"
fi
