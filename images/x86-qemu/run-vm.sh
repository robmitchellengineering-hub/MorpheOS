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

# A USB storage disk (doc 20, E18): VM_USB=<path> attaches a FAT image the udev rule mounts.
USB_ARGS=()
[ -n "${VM_USB:-}" ] && USB_ARGS=(-device qemu-xhci -device usb-storage,drive=stick -drive "id=stick,file=$VM_USB,format=raw,if=none")

# A real display (doc 20, E3): VM_GPU=1 swaps the default VGA for virtio-gpu, a KMS device
# the compositor can drive, and VM_MONITOR=<path> opens a monitor socket so a test can take
# a `screendump` of what is on the screen.
DISPLAY_ARGS=()
[ "${VM_GPU:-0}" = 1 ] && DISPLAY_ARGS=(-vga none -device virtio-gpu-pci)
MONITOR_ARGS=(-monitor none)
[ -n "${VM_MONITOR:-}" ] && MONITOR_ARGS=(-monitor "unix:$VM_MONITOR,server,nowait")
# Keyboard and mouse (E10): VM_QMP=<path> opens a QMP socket a test drives input through
# (`input-send-event`, `send-key`) and takes screendumps with. The keyboard is the q35
# machine's own PS/2 one (i8042, built into the kernel, as on most laptops); the pointer is
# a USB tablet behind xHCI, so the modules a real USB mouse needs (xhci-pci, usbhid,
# hid-generic) load too, and its absolute positions let a test click an exact pixel.
if [ -n "${VM_QMP:-}" ]; then
    MONITOR_ARGS+=(-qmp "unix:$VM_QMP,server,nowait" -device qemu-xhci -device usb-tablet)
fi

# VM_MEM=<MiB> for a scenario that needs more than 1 GiB (a browser, E4).
# A network (E5): VM_NET=1 attaches a virtio NIC on QEMU's user-mode network, where the host
# is 10.0.2.2 and DHCP hands out 10.0.2.15. Otherwise the VM has no network at all.
NET_ARGS=(-nic none)
[ "${VM_NET:-0}" = 1 ] && NET_ARGS=(-nic user,model=virtio-net-pci)

ACCEL=tcg
[ -w /dev/kvm ] && ACCEL=kvm

exec timeout --foreground "$TIMEOUT" qemu-system-x86_64 \
    -machine q35,accel="$ACCEL" -cpu max -smp 2 -m "${VM_MEM:-1024}" \
    -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
    -drive if=pflash,format=raw,file="$VARS" \
    -drive file="$DISK",format=raw,if=virtio \
    "${PAYLOAD_ARGS[@]}" "${USB_ARGS[@]}" \
    -device i6300esb -action watchdog=reset \
    "${DISPLAY_ARGS[@]}" "${MONITOR_ARGS[@]}" \
    "${NET_ARGS[@]}" -display none \
    -serial file:"$LOG"
