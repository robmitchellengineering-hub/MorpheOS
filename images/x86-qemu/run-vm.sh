#!/bin/bash
# Boot a MorpheOS test disk in QEMU (UEFI, headless) until it powers off.
#
#   images/x86-qemu/run-vm.sh <disk.img> <serial.log> [timeout-seconds]
#
# Reboots stay inside the same QEMU process, so one run covers a whole
# trial → fallback → commit sequence. The UEFI variable store is a fresh copy
# per disk, kept next to it, because the one-shot trial lives there.
set -euo pipefail

DISK=${1:?usage: run-vm.sh <disk.img> <serial.log> [timeout]}
LOG=${2:?usage: run-vm.sh <disk.img> <serial.log> [timeout]}
TIMEOUT=${3:-1800}
OVMF_CODE=${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE_4M.fd}
OVMF_VARS=${OVMF_VARS:-/usr/share/OVMF/OVMF_VARS_4M.fd}

VARS="${DISK%.img}.vars.fd"
[ -f "$VARS" ] || cp "$OVMF_VARS" "$VARS"

ACCEL=tcg
[ -w /dev/kvm ] && ACCEL=kvm

exec timeout --foreground "$TIMEOUT" qemu-system-x86_64 \
    -machine q35,accel="$ACCEL" -cpu max -smp 2 -m 1024 \
    -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
    -drive if=pflash,format=raw,file="$VARS" \
    -drive file="$DISK",format=raw,if=virtio \
    -device i6300esb -action watchdog=reset \
    -nic none -display none -monitor none \
    -serial file:"$LOG"
