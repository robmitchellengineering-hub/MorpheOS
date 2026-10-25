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
# This wrapper runs a patched COPY of build.sh (never build.sh itself) in which:
#   - the four mmdebstrap calls gain `--mode=fakechroot` (mmdebstrap builds the
#     rootfs under its OWN fakeroot+fakechroot instead of a real chroot — no
#     root, and no user namespaces needed);
#   - the three standalone `chroot` calls become `fakechroot chroot`
#     (`fakeroot fakechroot chroot` for update-initramfs, which also needs
#     fakeroot to create the initrd's device nodes).
# `mount --bind` is left untouched: under a non-root build it fails with EPERM,
# and regenerate_initrd already tolerates a failed bind (its `if mount --bind …`
# keeps going); update-initramfs then runs under fakechroot with /proc /sys /dev
# in FAKECHROOT_EXCLUDE_PATH, so it sees the host's populated mounts.
#
# KNOWN DIFFERENCE vs the root (`--variant=apt`) build: mmdebstrap's fakechroot
# mode installs the same packages, but its fakeroot records are process-local,
# so the DIRECTORY it leaves on disk is owned by the build user, not root.
#
# STATUS (measured on pacland-rog, 2026-10-25): this does NOT produce a bootable
# image. The base rootfs builds (systemd, kernel, firmware, openssh all install),
# but the kernel postinst's update-initramfs crashes while generating
# /boot/initrd.img — the initramfs-tools `kmod` hook aborts with
# "*** buffer overflow detected ***" (exit 134), i.e. depmod/libkmod segfaults
# under the fakechroot+fakeroot LD_PRELOAD. No initrd means the disk cannot boot,
# and mmdebstrap exits non-zero, so build.sh stops before any disk image exists.
# A build that produces a file is not a boot; here not even the file is produced.
# The rootless approach needs real root (or a working user namespace) for the
# initrd step, which fakechroot cannot fake.
#
# Needs on the build host: fakeroot, fakechroot, mmdebstrap, mtools, dosfstools,
# e2fsprogs, fdisk/sfdisk, systemd-boot-efi, gnupg, curl, and the built
# morph-boot / strata / morph-update / morph-manifest / morphd release binaries.
# ---------------------------------------------------------------------------
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
SRC="$HERE/build.sh"
[ -f "$SRC" ] || { echo "build-rootless.sh: missing $SRC" >&2; exit 1; }

# The patched copy must live NEXT TO build.sh: the patched build.sh derives its
# own HERE from `$0` (dirname) and REPO from `$HERE/../..`, so a copy run from
# /tmp would resolve every overlay/ and manifest/ path against /tmp. Removed by
# the EXIT trap.
P="$HERE/.build.sh.rootless.$$"
trap 'rm -f "$P"' EXIT
cp "$SRC" "$P"

# (a) force fakechroot MODE on the four mmdebstrap calls. The apt VARIANT stays
#     (it selects the package set); fakechroot is the chroot MODE mmdebstrap uses
#     to build a rootfs without root. Scoped to the `mmdebstrap --variant=apt`
#     calls so the make_root comment about the apt variant's motd is left alone.
sed -i 's/mmdebstrap --variant=apt/mmdebstrap --mode=fakechroot --variant=apt/g' "$P"

# (b) the standalone chroot calls -> fakechroot chroot (fakeroot added for
#     update-initramfs so its mknod'd initrd devices pack correctly). The
#     `chroot "$1"` inside MODULES_EXTRA_HOOK is a STRING mmdebstrap runs under
#     its own fakechroot, so it is deliberately not matched (it uses "$1").
sed -i \
    -e 's|chroot "$BROWSER_BASE" dpkg-query|fakechroot chroot "$BROWSER_BASE" dpkg-query|' \
    -e 's|chroot "$WIFI_BASE" dpkg-query|fakechroot chroot "$WIFI_BASE" dpkg-query|' \
    -e 's|chroot "$dir" update-initramfs|fakeroot fakechroot chroot "$dir" update-initramfs|' \
    "$P"

# Assert the patch actually applied (a silent no-op here would run the apt
# variant under a real chroot, which fails with EPERM deep in the build).
[ "$(grep -c -- 'mmdebstrap --mode=fakechroot' "$P" || true)" = 4 ] \
    || { echo "build-rootless.sh: patch failed — expected 4 fakechroot-mode calls" >&2; exit 1; }
[ "$(grep -c -- 'mmdebstrap --mode=fakechroot --variant=apt' "$P" || true)" = 4 ] \
    || { echo "build-rootless.sh: patch failed — the apt variant was lost" >&2; exit 1; }
grep -q 'fakeroot fakechroot chroot "$dir" update-initramfs' "$P" \
    || { echo "build-rootless.sh: patch failed — regenerate_initrd chroot was not faked" >&2; exit 1; }

# Not `exec`: the EXIT trap must fire to remove the patched copy beside build.sh.
bash "$P" "$@"
