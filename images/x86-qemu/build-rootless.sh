#!/bin/bash
# Build a MorpheOS A/B test disk with NO root, NO sudo and NO user namespaces —
# the same image `build.sh` produces, on a machine whose build user has none of
# those (pacland-rog's `rob`).
#
#   images/x86-qemu/build-rootless.sh <workdir> <scenario>
#
# scenario: good | unhealthy | crash | ransomware | plain | shell | browser |
#           wifi | broken-shell | hung-shell | recovery | manifest | autoupdate
#           (the same set build.sh accepts)
#
# ---------------------------------------------------------------------------
# WHY THIS WRAPPER EXISTS (and why `fakeroot build.sh` alone is not enough)
#
# build.sh has three kinds of step that need real root:
#
#   1. the four `mmdebstrap --variant=apt` calls  — apt needs a real chroot(2)
#      and real bind mounts of /proc /sys /dev, none of which fakeroot fakes;
#   2. `mount --bind` of /proc /sys /dev into the chroot (regenerate_initrd);
#   3. `chroot` itself (regenerate_initrd, and the two `dpkg-query` version
#      checks for the browser/wifi bases).
#
# fakeroot fakes ownership and device nodes (chown/mknod/…), but NOT chroot(2)
# and NOT mount(2). So this driver does two things:
#
#   a. It re-execs itself under `fakeroot` and keeps that ONE fakeroot instance
#      alive across the whole build. That is what makes the image's ownership
#      and device nodes correct: every `chown`/`mknod` the package installs
#      perform is recorded in fakeroot's database, and every tool that reads the
#      tree back to pack it (`mkfs.ext4 -d`, the initrd's `cpio`, `mcopy`) reads
#      through fakeroot and therefore sees root ownership and real device nodes —
#      even though the files on disk belong to the unprivileged user.
#
#   b. It runs a patched COPY of build.sh in which
#        - the four `mmdebstrap --variant=apt` become `--variant=fakechroot`
#          (mmdebstrap builds the rootfs under fakeroot+fakechroot, no chroot(2)
#          needed), and
#        - the three standalone `chroot` calls become `fakechroot chroot`
#          (fakechroot fakes chroot(2)).
#
# `mount --bind` is deliberately left untouched: under fakeroot it fails with
# EPERM, and regenerate_initrd already treats a failed bind as "no bind mounts"
# (its `if mount --bind …; then mounted=…; fi` keeps going). update-initramfs
# runs fine under fakechroot without the binds — the same way mmdebstrap's own
# fakechroot kernel postinst already built the first initrd for the base.
#
# The patch is applied to a throwaway copy of build.sh, never to build.sh
# itself, and it is ASSERTED (a silent sed no-op would otherwise run the apt
# variant under fakeroot and fail deep in the build with no clear reason).
#
# Needs on the build host: fakeroot, fakechroot, mmdebstrap, mtools, dosfstools,
# e2fsprogs, fdisk/sfdisk, systemd-boot-efi, gnupg, curl, and the built
# morph-boot / strata / morph-update / morph-manifest / morphd release binaries
# (produced exactly as build.sh and scripts/setup-linux.sh expect them).
# ---------------------------------------------------------------------------
set -euo pipefail

# Resolve our own absolute path BEFORE re-exec so fakeroot re-runs this script
# regardless of the caller's working directory.
SELF=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")

if [ "${MORPHEOS_ROOTLESS_FAKEROOT:-0}" != 1 ]; then
    export MORPHEOS_ROOTLESS_FAKEROOT=1
    exec fakeroot -- "$SELF" "$@"
fi

# From here on we are inside the single persistent fakeroot.
HERE=$(cd "$(dirname "$0")" && pwd)
SRC="$HERE/build.sh"
[ -f "$SRC" ] || { echo "build-rootless.sh: missing $SRC" >&2; exit 1; }

# The patched copy lives in a throwaway directory removed on exit.
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cp "$SRC" "$TMP/build.sh"
P="$TMP/build.sh"

# (a) apt -> fakechroot: the four mmdebstrap calls. apt needs real chroot(2);
#     fakechroot needs none. Scoped to the `mmdebstrap --variant=apt` calls so the
#     comment in make_root that documents the apt variant's motd behaviour
#     ("mmdebstrap's `--variant=apt` skips the base-files postinst") is left alone.
sed -i 's/mmdebstrap --variant=apt/mmdebstrap --variant=fakechroot/g' "$P"

# (b) the standalone chroot calls -> fakechroot chroot. The `chroot "$1"` inside
#     MODULES_EXTRA_HOOK is a STRING mmdebstrap runs under its own fakechroot,
#     so it is deliberately not matched here (it contains `"$1"`, not `"$dir"`,
#     `"$BROWSER_BASE"` or `"$WIFI_BASE"`).
sed -i \
    -e 's|chroot "$BROWSER_BASE" dpkg-query|fakechroot chroot "$BROWSER_BASE" dpkg-query|' \
    -e 's|chroot "$WIFI_BASE" dpkg-query|fakechroot chroot "$WIFI_BASE" dpkg-query|' \
    -e 's|chroot "$dir" update-initramfs|fakechroot chroot "$dir" update-initramfs|' \
    "$P"

# Assert the patch actually applied (a silent no-op here would run the apt
# variant under fakeroot, which fails with EPERM far into the build).
[ "$(grep -c -- 'mmdebstrap --variant=apt' "$P" || true)" = 0 ] \
    || { echo "build-rootless.sh: patch failed — an mmdebstrap apt call remains in the copy" >&2; exit 1; }
[ "$(grep -c -- 'mmdebstrap --variant=fakechroot' "$P" || true)" = 4 ] \
    || { echo "build-rootless.sh: patch failed — expected 4 fakechroot variants" >&2; exit 1; }
grep -q 'fakechroot chroot "$dir" update-initramfs' "$P" \
    || { echo "build-rootless.sh: patch failed — regenerate_initrd chroot was not faked" >&2; exit 1; }

exec bash "$P" "$@"
