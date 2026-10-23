#!/bin/bash
# "Try to Break It", part 5: the Manifest is obeyed (S11). Two proofs:
#
#   sudo images/x86-qemu/test-manifest.sh <workdir>
#
#   apply — a new generation changes the hostname and the retention, and the machine shows
#           both (read from /proc/sys/kernel/hostname and `strata policy`, never morphd's log).
#   fail  — a generation whose hostname the kernel refuses is rolled back: the machine's
#           hostname and Strata's policy are the seeded generation's again.
#
# The verdict is read from the ESP after the VM powers off (the same rule as test-recovery.sh).
set -euo pipefail

WORK=${1:?usage: test-manifest.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)

esp() { mtype -i "$disk@@1M" "::/$1" 2>/dev/null || true; }
esp_write() { # esp_write <src-file> <dst-path>
    mmd -i "$disk@@1M" "::/morph-manifest-test" 2>/dev/null || true
    mcopy -i "$disk@@1M" "$1" "::/morph-manifest-test/$2"
}

fail=0
check() { if eval "$2"; then echo "    ok    $1"; else echo "    FAIL  $1"; fail=1; fi; }

echo "=== scenario: manifest"
disk=$("$HERE/build.sh" "$WORK" manifest | tail -1)
rm -f "${disk%.img}.vars.fd"

# Run 1: apply a generation that changes the hostname and the retention.
echo apply > "$WORK/manifest-mode.txt"
esp_write "$WORK/manifest-mode.txt" mode
"$HERE/run-vm.sh" "$disk" "$WORK/serial-manifest-apply.log" "${VM_TIMEOUT:-1800}" || { echo "    VM did not power off cleanly"; fail=1; }

check "the kernel hostname is the applied one (got: $(esp morph-manifest-test/hostname))" \
    '[ "$(esp morph-manifest-test/hostname)" = "studio" ]'
check "Strata reports the applied retention" \
    'printf "%s" "$(esp morph-manifest-test/policy)" | grep -q "7 hours"'

# Run 2: a generation whose hostname the kernel refuses must be rolled back.
echo fail > "$WORK/manifest-mode.txt"
esp_write "$WORK/manifest-mode.txt" mode
"$HERE/run-vm.sh" "$disk" "$WORK/serial-manifest-fail.log" "${VM_TIMEOUT:-1800}" || { echo "    VM did not power off cleanly"; fail=1; }

check "the hostname is the seeded one again (got: $(esp morph-manifest-test/hostname))" \
    '[ "$(esp morph-manifest-test/hostname)" = "morpheos-a" ]'
check "Strata reports the seeded retention again" \
    'printf "%s" "$(esp morph-manifest-test/policy)" | grep -q "24 hours"'

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $WORK/serial-manifest-*.log"; fi
exit $fail
