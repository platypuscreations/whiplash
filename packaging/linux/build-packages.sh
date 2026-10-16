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

echo "Building skinamp $version..."
cargo build --release "$@"

# The icon, at the size desktops use for app launchers.
rsvg-convert -w 256 -h 256 packaging/icon.svg -o target/icons/skinamp.png

echo "Building the .deb..."
cargo deb --no-build --output "$bundle/"

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
# linuxdeploy copies in the libraries skinamp needs, except ones every
# system has (like the C library and the graphics drivers).
LDAI_OUTPUT="$bundle/skinamp-$version-$arch.AppImage" "$linuxdeploy" \
    --appdir "$appdir" \
    --executable target/release/skinamp \
    --desktop-file packaging/linux/skinamp.desktop \
    --icon-file target/icons/skinamp.png \
    --output appimage

ls -l "$bundle"
