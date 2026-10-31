#!/bin/bash
# build.sh — Phase 0 of PLAN-morpheos-own-kernel: build OUR Linux, with our name on it.
#
# What this is NOT: a new kernel. C from the survey (a kernel from scratch, or a Rust microkernel) is years
# and loses Firefox, PipeWire and every driver on the hardware we actually own. What this IS: the Linux
# kernel, built from a configuration we hold, with the branding patch that makes the machine report
# MorpheOS, and the hardening set that the Secure edition is supposed to be able to claim.
#
#   secure/kernel/build.sh [workdir] [jobs]
#
# Output: <workdir>/out/vmlinuz-morpheos, <workdir>/out/modules.tar.xz, <workdir>/out/build-report.txt
#
# The report is the point as much as the artefacts: it records the config decisions that were actually
# applied (not the ones we asked for) and the string the kernel carries, so "it says MorpheOS" is a fact
# with a command behind it rather than a patch we believe applied.
set -euo pipefail

KVER=${KVER:-6.8}
KURL="https://cdn.kernel.org/pub/linux/kernel/v${KVER%%.*}.x/linux-${KVER}.tar.xz"
HERE=$(cd "$(dirname "$0")" && pwd)
WORK=${1:-/tmp/morpheos-kernel}
JOBS=${2:-$(nproc 2>/dev/null || echo 4)}

mkdir -p "$WORK"
cd "$WORK"

# 1. The source, pinned. A kernel we cannot re-download byte-identically is not a kernel we own.
if [ ! -d "linux-$KVER" ]; then
    [ -f "linux-$KVER.tar.xz" ] || { echo "fetching $KURL"; curl -fsSL --retry 3 -o "linux-$KVER.tar.xz" "$KURL"; }
    echo "extracting"
    tar -xf "linux-$KVER.tar.xz"
fi
cd "linux-$KVER"

# 2. Our patches. The branding one is the whole identity argument: UTS_SYSNAME is a kernel constant, so
#    this is the only place `uname -s` can become MorpheOS.
if git apply --check "$HERE/branding.patch" 2>/dev/null; then
    git apply "$HERE/branding.patch"
    echo "branding patch applied"
elif grep -q 'UTS_SYSNAME "MorpheOS"' include/linux/uts.h; then
    echo "branding patch already applied"
else
    echo "branding patch does not apply — the source is not the version this patch was written for" >&2
    exit 1
fi

# 3. Our configuration, merged on top of a small base so the result is a decision list rather than a
#    vendor defconfig with everything enabled.
if [ ! -f .config ]; then
    make ARCH=x86_64 tinyconfig
fi
scripts/kconfig/merge_config.sh -m -O . .config "$HERE/config.fragment" >/dev/null
make ARCH=x86_64 olddefconfig

# 4. Build.
echo "building with -j$JOBS"
make ARCH=x86_64 -j"$JOBS" bzImage modules

# 5. Package, and write the report.
OUT="$WORK/out"
mkdir -p "$OUT"
cp arch/x86/boot/bzImage "$OUT/vmlinuz-morpheos"
INSTALL_MOD_PATH="$WORK/modules" make ARCH=x86_64 modules_install >/dev/null
tar -C "$WORK/modules" -cJf "$OUT/modules.tar.xz" lib/modules

{
    echo "kernel        : $KVER"
    echo "built with    : $(nproc 2>/dev/null) cores"
    echo "uname -s will : $(grep -m1 'define UTS_SYSNAME' include/linux/uts.h | sed 's/.*"\(.*\)"/\1/')"
    echo "localversion  : $(grep -m1 '^CONFIG_LOCALVERSION=' .config)"
    echo "modules signed: $(grep -m1 '^CONFIG_MODULE_SIG_FORCE' .config || echo 'CONFIG_MODULE_SIG_FORCE unset')"
    echo "lsm           : $(grep -m1 '^CONFIG_LSM=' .config)"
    echo "hardening     : $(grep -cE '^CONFIG_(RANDOMIZE_BASE|STRICT_KERNEL_RWX|STRICT_MODULE_RWX|SLAB_FREELIST_HARDENED|HARDENED_USERCOPY|FORTIFY_SOURCE|INIT_ON_ALLOC_DEFAULT_ON|INIT_STACK_ALL_ZERO|STACKPROTECTOR_STRONG|PAGE_TABLE_ISOLATION)=y' .config) of 10 present"
    echo "image bytes   : $(stat -c%s "$OUT/vmlinuz-morpheos" 2>/dev/null || stat -f%z "$OUT/vmlinuz-morpheos")"
    echo "the string it carries:"
    strings "$OUT/vmlinuz-morpheos" | grep -m3 -E '^6\.8\.' || true
} | tee "$OUT/build-report.txt"

echo "done: $OUT"
