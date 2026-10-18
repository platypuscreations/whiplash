#!/bin/bash
# Makes packaging/windows/whiplash.ico, the icon built into whiplash.exe,
# from packaging/icon.svg. Run it after changing the icon, and commit the
# result: the Windows build uses the .ico as it is.
#
# Needs rsvg-convert (`brew install librsvg`, or librsvg2-bin on Linux) and
# Python 3.
set -euo pipefail

cd "$(dirname "$0")/../.."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The SVG leaves the margins macOS icons have; Windows icons fill their
# square, so crop to the tile.
sed 's/viewBox="0 0 1024 1024"/viewBox="88 88 848 848"/' packaging/icon.svg > "$work/icon.svg"
sizes=(16 20 24 32 40 48 64 256)
for size in "${sizes[@]}"; do
    rsvg-convert -w "$size" -h "$size" "$work/icon.svg" -o "$work/$size.png"
done

# An .ico file is a small directory followed by the images, which may be
# PNGs (Windows Vista and later).
python3 - "$work" "${sizes[@]}" <<'PYTHON'
import struct, sys
work, sizes = sys.argv[1], [int(s) for s in sys.argv[2:]]
images = [open(f"{work}/{size}.png", "rb").read() for size in sizes]
header = struct.pack("<HHH", 0, 1, len(images))
offset = len(header) + 16 * len(images)
entries = b""
for size, image in zip(sizes, images):
    side = 0 if size >= 256 else size  # 0 means 256
    entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(image), offset)
    offset += len(image)
with open("packaging/windows/whiplash.ico", "wb") as f:
    f.write(header + entries + b"".join(images))
PYTHON
echo "Wrote packaging/windows/whiplash.ico"
