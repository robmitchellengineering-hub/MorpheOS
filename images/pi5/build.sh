#!/bin/bash
# Build the MorpheOS Raspberry Pi 5 self-test image with rpi-image-gen.
#
#   images/pi5/build.sh <rpi-image-gen dir> <aarch64 morph-boot binary> [build dir]
#
# Uses Raspberry Pi's own immutable A/B layout (image-rota: bootconfig,
# boot_a, boot_b, system_a, system_b, persistent) and adds the MorpheOS
# layers from images/pi5/layer. Runs on Debian/Ubuntu arm64, or amd64 with
# qemu-user-static + binfmt (see rpi-image-gen's getting_started.adoc).
set -euo pipefail

RIG=$(realpath "${1:?usage: build.sh <rpi-image-gen dir> <morph-boot aarch64 binary> [build dir]}")
BIN=$(realpath "${2:?usage: build.sh <rpi-image-gen dir> <morph-boot aarch64 binary> [build dir]}")
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
BUILD=$(realpath -m "${3:-$REPO/target/pi5}")

file "$BIN" | grep -q 'ARM aarch64' || { echo "$BIN is not an aarch64 binary" >&2; exit 1; }

# Assemble the overlays the layers install (gitignored, rebuilt every time).
OV="$HERE/layer/morpheos.d"
rm -rf "$OV"
mkdir -p "$OV/overlay" "$OV/selftest-overlay"
cp -a "$REPO/images/common/overlay/." "$OV/overlay/"
cp -a "$HERE/overlay/." "$OV/overlay/"
install -D -m 0755 "$BIN" "$OV/overlay/usr/bin/morph-boot"
cp -a "$REPO/images/common/selftest-overlay/." "$OV/selftest-overlay/"

"$RIG/rpi-image-gen" build -S "$HERE" -c morpheos-pi5-selftest.yaml -B "$BUILD"

IMG=$(find "$BUILD" -name 'morpheos-pi5-selftest.img' -type f | head -1)
[ -n "$IMG" ] || { echo "no image produced under $BUILD" >&2; exit 1; }
echo "$IMG"
