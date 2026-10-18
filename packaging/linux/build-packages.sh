#!/bin/bash
# Builds Linux packages in target/release/bundle: a .deb (Debian, Ubuntu)
# and an AppImage (runs on most distributions without installing).
#
# Needs: Rust, CMake, a C++ compiler, the ALSA and OpenGL development files,
# libclang (used to generate projectM's Rust bindings), rsvg-convert, and
# cargo-deb. On Ubuntu:
#   sudo apt install cmake g++ pkg-config libasound2-dev libgl-dev libegl-dev libclang-dev librsvg2-bin
#   cargo install cargo-deb
# The AppImage step downloads linuxdeploy.
#
# Any arguments are passed to `cargo build`, for example
# `--no-default-features` to leave out MilkDrop.
set -euo pipefail

cd "$(dirname "$0")/../.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
bundle=target/release/bundle
mkdir -p "$bundle" target/icons

echo "Building Whiplash $version..."
cargo build --release "$@"

# The icon, at the size desktops use for app launchers.
rsvg-convert -w 256 -h 256 packaging/icon.svg -o target/icons/whiplash.png

echo "Building the .deb..."
# Debian marks pre-releases with "~", which sorts before the release:
# 0.4.0~rc1 comes before 0.4.0, so installing 0.4.0 is an upgrade.
deb_version="$(echo "$version" | sed 's/-/~/')-1"
cargo deb --no-build --output "$bundle/" --deb-version "$deb_version"

echo "Building the AppImage..."
arch=$(uname -m)
tools=target/linuxdeploy
mkdir -p "$tools"
linuxdeploy="$tools/linuxdeploy-$arch.AppImage"
if [[ ! -x "$linuxdeploy" ]]; then
    curl -fsSL -o "$linuxdeploy" \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$arch.AppImage"
    chmod +x "$linuxdeploy"
fi
# Run linuxdeploy (itself an AppImage) by unpacking it, so it doesn't need
# FUSE, which containers and build servers often lack.
export APPIMAGE_EXTRACT_AND_RUN=1
appdir=target/AppDir
rm -rf "$appdir"
# The license, and the notices and license texts for what's included.
mkdir -p "$appdir/usr/share/doc/whiplash"
cp LICENSE THIRD-PARTY-NOTICES.md "$appdir/usr/share/doc/whiplash/"
cp -R licenses "$appdir/usr/share/doc/whiplash/"
# linuxdeploy copies in the libraries Whiplash needs, except ones every
# system has (like the C library and the graphics drivers).
LDAI_OUTPUT="$bundle/whiplash-$version-$arch.AppImage" "$linuxdeploy" \
    --appdir "$appdir" \
    --executable target/release/whiplash \
    --desktop-file packaging/linux/whiplash.desktop \
    --icon-file target/icons/whiplash.png \
    --output appimage

ls -l "$bundle"
