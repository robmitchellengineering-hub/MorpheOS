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

mkdir -p "$BUILD"   # rpi-image-gen refuses a build directory that does not exist
"$RIG/rpi-image-gen" build -S "$HERE" -c morpheos-pi5-selftest.yaml -B "$BUILD"

# Find the image rpi-image-gen just produced.
#
# WHY THIS CANNOT BE A BARE `find`. rpi-image-gen leaves root-owned, mode-700
# directories inside the build tree (the target user's home, apt's partial dirs).
# A recursive find as an unprivileged user reports `Permission denied` for those
# and — this is the part that bit us — **exits 1**, even though the image is
# sitting right there one directory away. Under `set -e` that killed the script
# AFTER a successful 3.29 GiB build, on every run, so the Pi image was never
# published by CI. `head -1` made it worse by closing the pipe early, which
# SIGPIPEs find and trips `pipefail` for a second reason.
#
# So: `-print -quit` stops at the first match instead of walking the whole tree,
# `2>/dev/null` drops diagnostics about directories we do not need, and `|| true`
# normalises the exit code — which is the only thing `set -e` was reacting to.
#
# The real question — "was an image produced?" — is the line below, and it is
# NOT relaxed: an empty result still fails the build. Only the noise is dropped.
IMG=$(find "$BUILD" -name 'morpheos-pi5-selftest.img' -type f -print -quit 2>/dev/null || true)
[ -n "$IMG" ] || { echo "no image produced under $BUILD" >&2; exit 1; }

# HOW MANY images match, and how big they are.
#
# `-print -quit` stops at the first match — which is right when there is exactly
# one, and arbitrary when there is not. rpi-image-gen writes an android-sparse
# variant alongside the plain image and keeps a working tree, so "first match" is
# only safe if it is also the only match. This prints every candidate to stderr so
# the build log answers the question instead of leaving it to a 17-minute guess.
# It does not change which image is used; if there is ever more than one, that is
# a finding to act on rather than a number to guess at silently.
# ⚠️ AND THE SAME TRAP, IMMEDIATELY RE-ENTERED. The first version of this
# diagnostic was `COUNT=$(find … | wc -l)`, which has no `|| true` — so `find`'s
# exit 1 on those same root-owned directories failed the assignment, and under
# `set -euo pipefail` it killed the script **before the image was reported**. The
# build went red for exactly the reason the comment above describes, three lines
# above where the new code was written. Lesson: the normalisation is needed on
# EVERY find in this file, not only the one that was being fixed at the time.
COUNT=$( { find "$BUILD" -name 'morpheos-pi5-selftest.img' -type f 2>/dev/null || true; } | wc -l )
echo "images named morpheos-pi5-selftest.img: $COUNT" >&2
find "$BUILD" -name 'morpheos-pi5-selftest.img*' -type f -printf '  %10s  %p\n' 2>/dev/null | sort -k1 -n >&2 || true
[ "$COUNT" -le 1 ] || echo "WARNING: $COUNT candidates; the one above is whichever find reached first" >&2

echo "$IMG"