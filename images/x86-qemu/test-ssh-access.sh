#!/bin/bash
# test-ssh-access.sh — boot the image in a VM and try to log in, exactly as the operator would.
#
# THIS IS THE TEST THAT SHOULD HAVE EXISTED FROM THE START (V-21, and Rob's question on 2026-10-30:
# "can you boot it on a vm and see if you can access it"). Five image builds went onto a stick before
# anyone asked the image whether its own door opens in a VM. It takes a minute and answers directly.
#
#   images/x86-qemu/test-ssh-access.sh <disk.img> [ssh-port]
#
# Needs: qemu-system-x86_64, OVMF, an ssh client. Runs on a Linux node with KVM (pacland-rog) — the
# Mac has no QEMU at all.
#
# Exit 0 = the key logged in. 1 = it did not (the finding, with sshd's own reason quoted). 2 = could not test.
set -uo pipefail

IMG=${1:?usage: test-ssh-access.sh <disk.img> [port]}
PORT=${2:-2222}
KEY=${MORPH_SSH_KEY:-$HOME/.ssh/mesh-admin}
HERE=$(cd "$(dirname "$0")" && pwd)
SERIAL=$(mktemp /tmp/ssh-access-serial.XXXXXX.log)

command -v qemu-system-x86_64 >/dev/null || { echo "UNKNOWN: no qemu here (run this on the Linux node)"; exit 2; }
[ -f "$IMG" ] || { echo "UNKNOWN: no image at $IMG"; exit 2; }

echo "booting $IMG with a forwarded port (host $PORT -> guest 22)"
# VM_SSH=1 makes run-vm.sh attach a user-mode NIC with the forward; the boot itself is unchanged.
VM_SSH=1 VM_SSH_PORT="$PORT" "$HERE/run-vm.sh" "$IMG" "$SERIAL" 900 &
BOOTPID=$!
trap 'kill $BOOTPID 2>/dev/null; rm -f "$SERIAL"' EXIT

# Wait for the guest's sshd to answer on the forwarded port.
for i in $(seq 1 120); do
    if nc -z 127.0.0.1 "$PORT" 2>/dev/null; then echo "sshd is up after ~$((i*2))s"; break; fi
    kill -0 "$BOOTPID" 2>/dev/null || { echo "the VM exited before sshd came up"; break; }
    sleep 2
done

SSH_OPTS=(-i "$KEY" -p "$PORT" -o BatchMode=yes -o StrictHostKeyChecking=no
          -o UserKnownHostsFile=/dev/null -o ConnectTimeout=8)

echo "--- trying the mesh admin key ---"
if ssh "${SSH_OPTS[@]}" root@127.0.0.1 'echo OK; uname -r' 2>&1 | tail -3; then
    echo "PASS: the door opens — the key logs in as root"
    exit 0
fi

echo "FAIL: the key was refused. sshd said:"
# The image runs access-selftest at boot; its verdict is on the serial console and is the whole point.
grep -aE 'Authentication refused|authorized keys .* is not a regular file|access-selftest' "$SERIAL" 2>/dev/null | tail -5 \
    || echo "  (nothing in the capture — check the serial log: $SERIAL)"
exit 1
