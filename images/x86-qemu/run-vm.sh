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

# An optional second, read-only disk: the update medium. It is how the update
# scenario delivers a payload to a running system, the way a real one would arrive
# on a stick, a mounted image or a network share — and keeping it OFF the main disk
# means the harness needs no change to build.sh to exercise an update.
PAYLOAD=${4:-}
PAYLOAD_ARGS=()
[ -n "$PAYLOAD" ] && PAYLOAD_ARGS=(-drive "file=$PAYLOAD,format=raw,if=virtio,readonly=on")

ACCEL=tcg
[ -w /dev/kvm ] && ACCEL=kvm

exec timeout --foreground "$TIMEOUT" qemu-system-x86_64 \
    -machine q35,accel="$ACCEL" -cpu max -smp 2 -m 1024 \
    -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
    -drive if=pflash,format=raw,file="$VARS" \
    -drive file="$DISK",format=raw,if=virtio \
    "${PAYLOAD_ARGS[@]}" \
    -device i6300esb -action watchdog=reset \
    -nic none -display none -monitor none \
    -serial file:"$LOG"
