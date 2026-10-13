#!/usr/bin/env bash
# The hardware report's own test (doc 20, E12): it must name the firmware a machine is
# missing. The VM boot proves the report is written and carries the VM's facts, but a VM is
# never missing firmware, so that boot cannot show the most important section working. Here
# the report reads a kernel log with the three ways drivers say it (iwlwifi, amdgpu, the
# kernel's own loader), and a clean one, and must report each correctly.
set -euo pipefail
cd "$(dirname "$0")/.."
REPORT=images/x86-qemu/overlay-hardware/usr/lib/morpheos/hwreport
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

run() {  # run <name> <dmesg lines...>: write a report with that kernel log
    local name=$1; shift
    mkdir -p "$tmp/$name/bin" "$tmp/$name/esp"
    { echo '#!/bin/sh'; for l in "$@"; do printf 'echo %q\n' "$l"; done; } > "$tmp/$name/bin/dmesg"
    chmod +x "$tmp/$name/bin/dmesg"
    PATH="$tmp/$name/bin:$PATH" MORPH_ESP="$tmp/$name/esp" MORPH_HWREPORT_DELAY=0 \
        bash "$REPORT" > /dev/null 2>&1
    sed -n '/^FIRMWARE THE KERNEL ASKED FOR/,/^$/p' "$tmp/$name/esp"/morpheos-report/*/summary.txt
}
fail=0
expect() {  # expect <what> <section> <grep pattern> [absent]
    if grep -q -- "$3" <<< "$2"; then found=1; else found=0; fi
    if { [ "${4:-}" = absent ] && [ $found = 0 ]; } || { [ "${4:-}" != absent ] && [ $found = 1 ]; }; then
        echo "ok   $1"
    else
        echo "FAIL $1"; echo "$2" | sed 's/^/     | /'; fail=1
    fi
}
missing=$(run missing \
    "[4.1] iwlwifi 0000:01:00.0: Direct firmware load for iwlwifi-so-a0-gf-a0-86.ucode failed with error -2" \
    "[4.2] iwlwifi 0000:01:00.0: no suitable firmware found!" \
    "[5.0] amdgpu 0000:c4:00.0: firmware: failed to load amdgpu/gc_11_0_1_pfp.bin (-2)")
expect "names the Wi-Fi firmware that failed to load" "$missing" 'iwlwifi-so-a0-gf-a0-86.ucode'
expect "names the graphics firmware that failed to load" "$missing" 'amdgpu/gc_11_0_1_pfp.bin'
expect "says when a driver found no suitable firmware" "$missing" 'no suitable firmware found'
expect "does not say none when firmware is missing" "$missing" '^  none$' absent
clean=$(run clean "[1.0] Linux version 6.8.0" "[2.0] iwlwifi 0000:01:00.0: loaded firmware version 86.fb5c9aeb.0")
expect "says none on a clean boot" "$clean" '^  none$'
[ "$fail" = 0 ] && echo "hardware report: names missing firmware, and says none when there is none"
exit "$fail"
