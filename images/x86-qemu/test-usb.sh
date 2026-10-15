#!/bin/bash
# A USB stick plugged into the shell (doc 20, E18): the udev rule mounts it, so the file view
# lists it.
#
#   sudo images/x86-qemu/test-usb.sh <workdir>
#
# Builds a FAT stick with a label and one known file, boots the `plain` image with it attached
# as a USB storage device, and asserts flow-mount mounted it at /run/media/<label> — the mount
# point the pure function decides. The serial line is a dedicated marker (flow-mount writes it to
# the console on purpose), so it is the verdict here, not a line racing the kernel.
set -euo pipefail

WORK=${1:?usage: test-usb.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$WORK"

echo "=== usb: a stick with one file mounts itself"

disk=$("$HERE/build.sh" "$WORK" plain | tail -1)
rm -f "${disk%.img}.vars.fd"

# A FAT stick the way a person's arrives: a label and one file.
STICK="$WORK/stick.img"
rm -f "$STICK"
truncate -s 64M "$STICK"
mkfs.fat -F 32 -n TESTSTICK "$STICK" >/dev/null
printf 'hello from the stick\n' > "$WORK/hello.txt"
mcopy -i "$STICK" "$WORK/hello.txt" ::/hello.txt

log="$WORK/serial-usb.log"
rm -f "$log"
VM_USB="$STICK" "$HERE/run-vm.sh" "$disk" "$log" 240 &
vm=$!

fail=0
line=""
for _ in $(seq 1 240); do
    line=$(grep -ao 'flow-mount: mounted .*' "$log" 2>/dev/null | tail -1 || true)
    [ -n "$line" ] && break
    if ! kill -0 "$vm" 2>/dev/null; then break; fi
    sleep 1
done
set +e; kill "$vm" 2>/dev/null; wait "$vm" 2>/dev/null; set -e

if [ -z "$line" ]; then
    echo "    FAIL: flow-mount never mounted the stick"
    grep -a 'flow-mount\|usb-storage\|sd[a-z]' "$log" | tail -10 || true
    fail=1
else
    echo "    $line"
    case "$line" in
        *"at /run/media/teststick"*) echo "    ok    mounted at /run/media/teststick — the file view lists it";;
        *) echo "    FAIL  mounted somewhere unexpected"; fail=1;;
    esac
fi

if [ "$fail" != 0 ]; then
    echo "usb: FAILED" >&2
    exit 1
fi
echo "usb: PASS — a stick with one file mounts itself at /run/media/<label>"
