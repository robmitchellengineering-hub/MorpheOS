#!/usr/bin/env bash
# Everything this repository needs on Ubuntu 24.04, in one place (doc 20, S10).
#
#     bash scripts/setup-linux.sh            # what `scripts/check.sh` needs
#     bash scripts/setup-linux.sh --images   # and the image and VM tools (the `images` suite)
#
# WHY ONE SCRIPT. The builder works on a Windows laptop, inside WSL2 Ubuntu 24.04, and CI runs
# on Ubuntu 24.04 runners. CI calls THIS script to install its own dependencies, so a package
# the build needs cannot be added to CI and forgotten here: if this script is wrong, CI is red.
# Safe to run again; it installs only what is missing.
#
# On WSL2: run it inside the Ubuntu shell, in a clone under your Linux home (~/), not under
# /mnt/c. If /dev/kvm exists, the VM tests use it and take minutes; without it they emulate
# the CPU and take much longer, but they still run (see HANDOFF.md).
set -euo pipefail

IMAGES=0
for a in "$@"; do
    case "$a" in
        --images) IMAGES=1 ;;
        *) echo "usage: setup-linux.sh [--images]" >&2; exit 2 ;;
    esac
done

if [ "$(uname -s)" != Linux ]; then
    echo "setup-linux.sh: this is $(uname -s). The bar runs on Linux; on Windows, in WSL2:" >&2
    echo "    wsl --install -d Ubuntu-24.04" >&2
    exit 2
fi
if ! grep -q 'VERSION_ID="24.04"' /etc/os-release 2>/dev/null; then
    echo "setup-linux.sh: warning: written for Ubuntu 24.04 (the image's base and CI's runner)" >&2
fi
SUDO=""
[ "$(id -u)" = 0 ] || SUDO=sudo

# The compositor links libxkbcommon and pixman, reads keymaps from xkb-data, and its live
# backend links libinput, libseat and libudev (E3, E10). The docs and plan checks are Node.
PACKAGES=(build-essential pkg-config curl ca-certificates git
    libxkbcommon-dev libpixman-1-dev xkb-data libinput-dev libseat-dev libudev-dev)
command -v node > /dev/null || PACKAGES+=(nodejs)
# The images: a Debian bootstrapper, QEMU with UEFI firmware, and the filesystem tools the
# image is assembled and read back with; python3 judges the screenshots.
[ "$IMAGES" = 1 ] && PACKAGES+=(mmdebstrap qemu-system-x86 ovmf systemd-boot-efi dosfstools
    mtools fdisk e2fsprogs python3)

missing=()
for p in "${PACKAGES[@]}"; do
    dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q 'install ok installed' || missing+=("$p")
done
if [ ${#missing[@]} -gt 0 ]; then
    echo "== installing: ${missing[*]}"
    $SUDO apt-get update -q
    $SUDO apt-get install -y -q --no-install-recommends "${missing[@]}"
else
    echo "== system packages: all present"
fi

# Rust: rustup's stable, with the components the bar runs. A toolchain that is already
# there (CI's) is left alone; `rustup update stable` is the builder's job before each
# pull request (HANDOFF.md), because CI uses the newest stable.
if ! command -v cargo > /dev/null; then
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
fi
if ! command -v cargo > /dev/null; then
    echo "== installing Rust (rustup, stable)"
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --component rustfmt,clippy
    . "$HOME/.cargo/env"
else
    echo "== Rust: $(cargo --version)"
fi

if [ "$IMAGES" = 1 ]; then
    # KVM makes the VM suite minutes instead of most of an hour. The harness runs QEMU under
    # sudo, but the device must also be usable by the user for anything run without it.
    if [ -e /dev/kvm ]; then
        echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' |
            $SUDO tee /etc/udev/rules.d/99-kvm.rules > /dev/null
        if command -v udevadm > /dev/null && $SUDO udevadm control --reload-rules 2>/dev/null; then
            $SUDO udevadm trigger --name-match=kvm || true
        else
            $SUDO chmod 0666 /dev/kvm || true  # WSL2 without systemd has no udev to apply it
        fi
        echo "== KVM: available"
    else
        echo "== KVM: not available; the VM suite will emulate the CPU (slow, still correct)."
        echo "   On WSL2, nested virtualisation gives it /dev/kvm: see HANDOFF.md."
    fi
fi
echo "== ready. Next: bash scripts/check.sh"
