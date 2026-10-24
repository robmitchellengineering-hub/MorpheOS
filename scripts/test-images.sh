#!/usr/bin/env bash
# The `images` workflow's x86 suite, on this machine (doc 20, S10).
#
#     bash scripts/test-images.sh [WORKDIR] [plain|scenarios|shell|all]   # default: all
#
# The same steps, in the same order, as the x86 job in .github/workflows/images.yml: build the
# binaries as you, then build and boot the images as root. Run `bash scripts/setup-linux.sh
# --images` once first. With KVM a full run takes minutes; without it (emulated CPU) it can
# take over an hour, so `plain` or `shell` alone is the quick check for work on one of them.
#
# Keep it in step with images.yml. If you add a step there, add it here.
set -euo pipefail
cd "$(dirname "$0")/.."

WORK=${1:-$HOME/morpheos-img}
WHAT=${2:-all}
case "$WHAT" in plain|scenarios|shell|all) ;; *)
    echo "usage: test-images.sh [WORKDIR] [plain|scenarios|shell|all]" >&2; exit 2 ;;
esac
if [ "$(uname -s)" != Linux ]; then
    echo "test-images.sh: the images are built and booted on Linux (WSL2 on Windows)" >&2
    exit 2
fi
step() { printf '\n== %s\n' "$*"; }
trap 'printf "\nFAILED at: %s\n(the serial logs and screenshots are in %s)\n" "${BASH_COMMAND}" "$WORK" >&2' ERR
mkdir -p "$WORK"
[ -e /dev/kvm ] || echo "note: no /dev/kvm, so the VMs emulate the CPU; this will be slow"

# The harness scripts run under sudo, whose PATH has no cargo: build first, as yourself.
step "build the binaries the images carry"
cargo build --release -p morph-boot -p strata -p morph-update -p morph-heal -p morph-manifest -p morphd -p flow-compositor -p flow-wifi -p flow-files -p flow-notice

if [ "$WHAT" = plain ] || [ "$WHAT" = all ]; then
    step "the image a person uses: build, boot, and read back its report"
    sudo images/x86-qemu/build.sh "$WORK" plain
    sudo images/x86-qemu/test-boot.sh "$WORK" 300
    node scripts/verify-motd.mjs "$WORK/serial-plain.log"
fi
if [ "$WHAT" = scenarios ] || [ "$WHAT" = all ]; then
    step "good, unhealthy, crashing, updating and healing"
    sudo images/x86-qemu/test-fallback.sh "$WORK" good unhealthy crash update heal
    step "ransomware against Strata"
    sudo images/x86-qemu/test-ransomware.sh "$WORK"
    step "the recovery image (S4)"
    sudo images/x86-qemu/test-recovery.sh "$WORK"
    step "the Manifest is obeyed (S11)"
    sudo images/x86-qemu/test-manifest.sh "$WORK"
    step "a full disk does not break the machine (S7)"
    sudo images/x86-qemu/test-full-disk.sh "$WORK"
    step "a corrupted setting rolls back (S8)"
    sudo images/x86-qemu/test-config-rollback.sh "$WORK"
fi
if [ "$WHAT" = shell ] || [ "$WHAT" = all ]; then
    step "the shell on a real display, with a keyboard and a mouse"
    sudo images/x86-qemu/test-shell.sh "$WORK"
    step "a broken or hung shell update falls back"
    sudo images/x86-qemu/test-fallback.sh "$WORK" broken-shell hung-shell
    step "a browser on the Flow shell"
    sudo images/x86-qemu/test-browser.sh "$WORK"
    step "joining Wi-Fi with the picker"
    sudo images/x86-qemu/test-wifi.sh "$WORK"
    step "a USB stick mounts itself"
    sudo images/x86-qemu/test-usb.sh "$WORK"
fi
printf '\nimages suite (%s) passed\n' "$WHAT"
