#!/bin/bash
# "Try to Break It", part 3: ransomware against a running MorpheOS.
#
#   sudo images/x86-qemu/test-ransomware.sh <workdir>
#
# Boots a disk whose self-test (images/common/selftest-overlay/.../ransomware-test)
# writes 24 documents, lets Strata record them, encrypts them the way ransomware
# does (in place, and as .locked copies with the originals deleted, plus a ransom
# note), reboots, and then rewinds each folder Strata's alert names to the point
# the alert gives.
#
# The verdict is read from the disk after the VM powers off, never from the VM's
# console or from what the self-test says about itself (AGENTS.md H8):
#   * boot history: both boots ran slot A and were healthy, Strata included;
#   * Strata's alert record and the documents themselves — the shared verdict in
#     verify-ransomware.sh, which test-rewind.sh sources too, so the two cannot
#     drift apart.
set -euo pipefail

WORK=${1:?usage: test-ransomware.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
DOCS=/home/morph/Documents   # as the VM sees it; on the data partition: /home/morph/Documents
WANT_DOCS=24
# shellcheck source=verify-ransomware.sh
. "$HERE/verify-ransomware.sh"

echo "=== scenario: ransomware"
disk=$("$HERE/build.sh" "$WORK" ransomware | tail -1)
rm -f "${disk%.img}.vars.fd"
log="$WORK/serial-ransomware.log"
start=$(date +%s)
"$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-2400}" || { echo "    VM did not power off cleanly"; }
echo "    vm time: $(( $(date +%s) - start ))s"

fail=0

# The self-test reached its last step.
step=$(esp morph-test/step | tr -d '\r\n' || true)
check "self-test finished (step 2, got '${step}')" '[ "$step" = 2 ]'

# Both boots: slot A, committed, healthy (a failed strata.service would make them unhealthy).
boots=$(esp morph-boot/history.jsonl |
    sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p' | tr '\n' ' ')
check "two healthy boots of slot A (got: ${boots})" '[ "$boots" = "a/committed/none a/committed/none " ]'

# The shared verdict: the data partition, the alert, the documents and the quarantine.
data_partition
verify_ransomware_docs

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $log"; fi
exit $fail
