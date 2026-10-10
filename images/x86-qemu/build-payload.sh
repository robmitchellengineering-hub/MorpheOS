#!/bin/bash
# Build the update medium for the `update` scenario.
#
#   images/x86-qemu/build-payload.sh <workdir> <payload.img>
#
# It produces a small disk holding the `morph-update` binary and a sealed payload —
# the new root image, a kernel, an initramfs, and `BLAKE3SUMS` over all three. The VM
# attaches it read-only as a second drive, which is how the update reaches a running
# system: NOT through a special path baked into the image.
#
# WHY A SEPARATE MEDIUM AND NOT PART OF THE MAIN DISK. Two reasons, and the second is
# the one that matters:
#
#   1. it needs no change to `build.sh`, so the update scenario cannot disturb the
#      three scenarios that already pass;
#   2. an update that arrives on the machine's own disk is not an update. Delivering it
#      from outside is what makes this test the update path rather than a rebuild.
#
# THE NEW SYSTEM IS SLOT A'S ROOT PLUS A MARKER, and that is deliberate rather than
# lazy: what an update has to prove here is that slot B's **content changed**, and that
# the system which then booted is the one that was written. A marker file is the
# smallest thing that distinguishes them. It is written into the image with `debugfs`,
# so the payload really is a different filesystem, not a flag the test reads back.
#
# `morph-update`'s own test suite covers the refusal cases against ordinary files; this
# is the end-to-end half, where the "device" is a real partition and the boot is real.
set -euo pipefail

WORK=${1:?usage: build-payload.sh <workdir> <payload.img>}
OUT=${2:?usage: build-payload.sh <workdir> <payload.img>}
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
MORPH_UPDATE=${MORPH_UPDATE:-$REPO/target/release/morph-update}

# Build the updater here rather than making the workflow build it: the workflow's
# `cargo build` line is shared ground, and a test that needs one more binary is not a
# reason to edit a line another change is also editing.
if [ ! -x "$MORPH_UPDATE" ]; then
    echo "building morph-update (release)" >&2
    (cd "$REPO" && cargo build --release -p morph-update >&2)
fi
[ -x "$MORPH_UPDATE" ] || {
    echo "$MORPH_UPDATE is still missing after building" >&2
    exit 1
}

# Slot A's root still exists in the work directory after build.sh assembles the disk,
# along with the ESP tree it copied into the image. Reuse both: a payload built from
# the same debootstrap is a system this machine can actually boot.
SRC="$WORK/root-a.img"
[ -f "$SRC" ] || { echo "no $SRC: run build.sh before this" >&2; exit 1; }
[ -f "$WORK/esp/morpheos/a/vmlinuz" ] || { echo "no kernel at $WORK/esp/morpheos/a" >&2; exit 1; }

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
P="$STAGE/payload"
mkdir -p "$P"

cp "$SRC" "$P/root.img"
cp "$WORK/esp/morpheos/a/vmlinuz" "$P/vmlinuz"
cp "$WORK/esp/morpheos/a/initrd.img" "$P/initrd.img"

MARKER="$STAGE/marker"
printf 'this file was installed by morph-update\n' > "$MARKER"
# `write` fails if the target exists, which is the check that the source image is the
# one build.sh produced and not a payload that has already been through this.
debugfs -w -R "write $MARKER /etc/morph-update-marker" "$P/root.img" >/dev/null

"$MORPH_UPDATE" --payload "$P" seal >/dev/null
cp "$MORPH_UPDATE" "$STAGE/morph-update"

rm -f "$OUT"
mkfs.ext4 -q -L payload -d "$STAGE" "$OUT" 1500M
echo "$OUT"
