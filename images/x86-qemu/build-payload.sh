#!/bin/bash
# Build the update medium for the `update` scenario.
#
#   images/x86-qemu/build-payload.sh <workdir> <payload.img>
#
# It produces a small disk holding a sealed, signed payload: the new root image, a kernel, an
# initramfs, `BLAKE3SUMS` over all three, and `BLAKE3SUMS.sig` (doc 20, S2). No updater: the
# machine installs it with its own. The VM
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
MORPH_HEAL=${MORPH_HEAL:-$REPO/target/release/morph-heal}

# Which medium: `update` carries a signed payload; `heal` carries only the
# healer. The in-image driver tells them apart by what it finds on the medium rather than by
# being told, so a scenario cannot be handed the wrong one and quietly do nothing.
KIND=${3:-update}
case "$KIND" in update|heal) ;; *) echo "unknown medium kind: $KIND" >&2; exit 2 ;; esac

# Built by the workflow, like morph-boot, and for the same reason: this script runs
# under sudo, whose PATH does not contain the invoking user's cargo. A clear error beats
# a build that cannot find its own toolchain.
[ -x "$MORPH_UPDATE" ] || {
    echo "$MORPH_UPDATE is not built; run: cargo build --release -p morph-update" >&2
    exit 1
}
[ -x "$MORPH_HEAL" ] || {
    echo "$MORPH_HEAL is not built; run: cargo build --release -p morph-heal" >&2
    exit 1
}

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

if [ "$KIND" = heal ]; then
    # A healer needs no payload: it rebuilds the spare from the running system, which is
    # already on the machine. So this medium is just the binary, and it is small.
    cp "$MORPH_HEAL" "$STAGE/morph-heal"
    rm -f "$OUT"
    # `-d` is the staging DIRECTORY, then the device, then the size — the same order as the
    # update medium below. Without `-d`, mkfs read the staging directory as the device name
    # and died with "invalid blocks ... on device /tmp/tmp.XXXX".
    mkfs.ext4 -q -L heal -d "$STAGE" "$OUT" 64M
    echo "$OUT"
    exit 0
fi

# Slot A's root still exists in the work directory after build.sh assembles the disk,
# along with the ESP tree it copied into the image. Reuse both: a payload built from
# the same debootstrap is a system this machine can actually boot.
SRC="$WORK/root-a.img"
[ -f "$SRC" ] || { echo "no $SRC: run build.sh before this" >&2; exit 1; }
[ -f "$WORK/esp/morpheos/a/vmlinuz" ] || { echo "no kernel at $WORK/esp/morpheos/a" >&2; exit 1; }

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

# ⚠️ MAKE THE FILESYSTEM CLEAN, AND PROVE IT. `debugfs -w` edits an image without
# clearing the superblock's state, so the image comes out marked **dirty** — and a dirty
# ext4 with an unreplayed journal cannot be mounted **read-only**, which is precisely how
# the loader entry boots this slot (`ro` in its options). A payload that cannot be
# mounted is not a payload, and the failure mode is not a clean error: it stalled the
# first update run for twenty minutes while each boot's health check waited out its
# timeout. `-n` afterwards means "make no changes and tell me if anything is wrong", so
# the check cannot pass by repairing what it is testing.
e2fsck -f -y "$P/root.img" >/dev/null 2>&1 || true
e2fsck -f -n "$P/root.img" >/dev/null 2>&1 || {
    echo "the payload root image is still not clean after e2fsck" >&2
    exit 1
}

# ⚠️ SHRINK THE PAYLOAD TO WHAT IT ACTUALLY HOLDS. The traffic IS the time.
#
# Measured on run `38117817003`, from the guest's own kernel timestamps in
# `serial-update.log`: install ran 3.836 s → 11.155 s, and the serial log says why —
# `morph-update` reported `root_bytes: 1258291200`. That is the whole 1200 MiB slot, for a
# debootstrap whose **content** is a fraction of it. `install` reads the payload to hash it,
# writes it, and reads the device back to verify it, so the cost is 3x the image size:
# ~3.6 GB of I/O to install a few hundred megabytes of system.
#
# So the image is resized to the smallest filesystem that still holds its contents and then
# truncated to that length. Nothing in the update path assumes the image fills the slot:
# `write_image` copies the file's bytes rather than the partition, `hash_prefix` re-reads
# exactly those bytes, and slot B's boot entry mounts the filesystem the superblock
# describes. The tail of the partition keeps whatever was there before and is never read.
#
# This is the honest lever rather than a weaker check: the read-back verification stays
# whole, because it is the operation the wedge depends on.
BEFORE=$(stat -c %s "$P/root.img")
resize2fs -M "$P/root.img" >/dev/null 2>&1 || {
    echo "resize2fs could not shrink the payload root image" >&2
    exit 1
}
# `resize2fs` shrinks the filesystem *inside* the file; the file stays 1200 MiB until it is
# truncated, and it is the file's length that `morph-update` writes and hashes.
SIZE=$({ dumpe2fs -h "$P/root.img" 2>/dev/null || true; } | awk -F: '
    /^Block count:/ { c = $2 }
    /^Block size:/  { s = $2 }
    END { gsub(/[^0-9]/, "", c); gsub(/[^0-9]/, "", s); if (c != "" && s != "") print c * s }')
[ -n "$SIZE" ] || {
    echo "could not read the resized payload's size" >&2
    exit 1
}
truncate -s "$SIZE" "$P/root.img"
AFTER=$(stat -c %s "$P/root.img")

# The shrink must have DONE something (a `resize2fs` that quietly no-opped would leave the
# cost where it was and the job green), and it must not have broken the filesystem.
[ "$AFTER" -lt "$BEFORE" ] || {
    echo "the payload root image did not shrink ($BEFORE -> $AFTER bytes)" >&2
    exit 1
}
e2fsck -f -n "$P/root.img" >/dev/null 2>&1 || {
    echo "the payload root image is not clean after the shrink" >&2
    exit 1
}
echo "payload root image: $BEFORE -> $AFTER bytes ($((AFTER * 100 / BEFORE))%)"

# The payload says which release it is, under the signature (doc 20, S3): the machine refuses
# one older than the release it runs. Newer than build.sh's default, as a real update is.
PAYLOAD_RELEASE=${PAYLOAD_RELEASE:-2}
"$MORPH_UPDATE" --payload "$P" --release "$PAYLOAD_RELEASE" seal >/dev/null
# Signed with the key the image trusts (build.sh made it, or was given it): an unsigned
# payload is refused by the machine before anything is written (doc 20, S2).
UPDATE_KEY=${UPDATE_KEY:-$WORK/update-test.key}
[ -f "$UPDATE_KEY" ] || { echo "no signing key at $UPDATE_KEY: run build.sh first" >&2; exit 1; }
"$MORPH_UPDATE" --payload "$P" --key "$UPDATE_KEY" sign
# No updater on the medium: the machine installs with its own, which checks the signature
# against its own keys. A medium's binary could check nothing.

rm -f "$OUT"
mkfs.ext4 -q -L payload -d "$STAGE" "$OUT" 1500M
echo "$OUT"
