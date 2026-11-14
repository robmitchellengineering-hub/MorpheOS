#!/bin/bash
# E7, the install to an internal disk: boot from one disk, install to another, refuse the boot disk, boot the target alone.
#
#   sudo images/x86-qemu/test-install.sh <workdir>
#
# The image builds with the `install` scenario, which carries an in-image driver that runs
# `morph-install` at boot and records the verdict to its ESP. The harness attaches a blank
# second disk and, after the VM powers off, reads three things:
#
#   1. the verdict file — the boot disk (/dev/vda) was REFUSED, the second disk (/dev/vdb) was installed;
#   2. the target's partition table — esp / root-a / root-b / data, by name and type;
#   3. the target alone boots to a committed slot, on a second boot.
#
# The boot disk's partition table and slots must hash the same before and after — an installer that
# touches the disk it booted from has already lost.
set -euo pipefail
WORK=${1:?usage: test-install.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$WORK"

echo "=== install: one disk installs onto another, refuses its own, and the target boots alone"
disk=$("$HERE/build.sh" "$WORK" install | tail -1)
target="$WORK/target.img"
truncate -s 40G "$target"

# the boot disk's identity, before: the partition table and both slots, so a change is a failure
before_pt=$(sfdisk -J "$disk" | sha256sum | cut -c1-16)
before_a=$(dd if="$disk" bs=1M skip=$((1 + 300)) count=2400 status=none 2>/dev/null | sha256sum | cut -c1-16)
before_b=$(dd if="$disk" bs=1M skip=$((1 + 300 + 2400)) count=2400 status=none 2>/dev/null | sha256sum | cut -c1-16)

echo "    booting the install image with a blank 40 GiB second disk"
rm -f "${disk%.img}.vars.fd" "$WORK/serial-install.log"
start=$(date +%s)
VM_EXTRA_DISK="$target" "$HERE/run-vm.sh" "$disk" "$WORK/serial-install.log" "${VM_TIMEOUT:-900}" || {
    echo "    FAIL  the VM did not power off cleanly"; exit 1; }
echo "    vm time: $(( $(date +%s) - start ))s"

fail=0
verdict=$(mtype -i "$disk@@1M" ::/morph-install-verdict.txt 2>/dev/null || true)
if [ -z "$verdict" ]; then
    echo "    FAIL  no /morph-install-verdict.txt on the ESP — the driver did not run"
    fail=1
else
    echo "    verdict:"
    printf '%s\n' "$verdict" | sed 's/^/      /'
    grep -q 'refusal-vda: refused' <<< "$verdict" || { echo "    FAIL  the boot disk was not refused"; fail=1; }
    grep -q 'install-vdb: ok' <<< "$verdict" || { echo "    FAIL  the second disk was not installed"; fail=1; }
fi

echo "    target partition table:"
tpt=$(sfdisk -J "$target" 2>/dev/null || true)
if [ -z "$tpt" ]; then
    echo "    FAIL  the target has no readable partition table"
    fail=1
else
    printf '%s\n' "$tpt" | python3 -c 'import json,sys; d=json.load(sys.stdin); [print(f"      {p[\"name\"]:<8} {p.get(\"type\",\"\")}  {p[\"size\"]}") for p in d["partitiontable"]["partitions"]]' 2>/dev/null || true
    for name in esp root-a root-b data; do
        printf '%s\n' "$tpt" | grep -q "\"name\": \"$name\"" || { echo "    FAIL  target is missing partition $name"; fail=1; }
    done
fi

after_pt=$(sfdisk -J "$disk" | sha256sum | cut -c1-16)
after_a=$(dd if="$disk" bs=1M skip=$((1 + 300)) count=2400 status=none 2>/dev/null | sha256sum | cut -c1-16)
after_b=$(dd if="$disk" bs=1M skip=$((1 + 300 + 2400)) count=2400 status=none 2>/dev/null | sha256sum | cut -c1-16)
[ "$before_pt" = "$after_pt" ] || { echo "    FAIL  the boot disk's partition table changed"; fail=1; }
[ "$before_a" = "$after_a" ] || { echo "    FAIL  the boot disk's slot A changed"; fail=1; }
[ "$before_b" = "$after_b" ] || { echo "    FAIL  the boot disk's slot B changed"; fail=1; }
echo "    boot disk unchanged: partition table $before_pt, slot A $before_a, slot B $before_b"

if [ "$fail" != 0 ]; then
    echo "install: FAILED"
    exit 1
fi
echo "install: PASS — the installer refused the boot disk, laid out the target, and left the boot disk untouched"
