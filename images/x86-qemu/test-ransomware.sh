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
#   * Strata's alert record on the data partition: an alert exists, and its rewind
#     point is after the documents were complete;
#   * the documents themselves, copied out of the data partition: every one matches
#     the checksum taken before the attack, and no .locked file or ransom note is
#     left among them;
#   * the quarantine: the attacker's files were moved there, not deleted.
set -euo pipefail

WORK=${1:?usage: test-ransomware.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
DOCS=/home/morph/Documents   # as the VM sees it; on the data partition: /home/morph/Documents
WANT_DOCS=24

esp() { mtype -i "$disk@@1M" "::/$1" 2>/dev/null; }

echo "=== scenario: ransomware"
disk=$("$HERE/build.sh" "$WORK" ransomware | tail -1)
rm -f "${disk%.img}.vars.fd"
log="$WORK/serial-ransomware.log"
start=$(date +%s)
"$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-2400}" || { echo "    VM did not power off cleanly"; }
echo "    vm time: $(( $(date +%s) - start ))s"

fail=0
check() { if eval "$2"; then echo "    ok    $1"; else echo "    FAIL  $1"; fail=1; fi; }

# The self-test reached its last step.
step=$(esp morph-test/step | tr -d '\r\n' || true)
check "self-test finished (step 2, got '${step}')" '[ "$step" = 2 ]'

# Both boots: slot A, committed, healthy (a failed strata.service would make them unhealthy).
boots=$(esp morph-boot/history.jsonl |
    sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p' | tr '\n' ' ')
check "two healthy boots of slot A (got: ${boots})" '[ "$boots" = "a/committed/none a/committed/none " ]'

# The data partition, copied out so debugfs can read it without mounting anything.
read -r dstart dsize < <(sfdisk -d "$disk" | sed -nE 's/.*start= *([0-9]+), size= *([0-9]+),.*name="data".*/\1 \2/p')
data="$WORK/data-out.img"
dd if="$disk" of="$data" bs=512 skip="$dstart" count="$dsize" status=none

# Strata's own record of the attack.
alerts=$(debugfs -R "cat /strata/alerts.jsonl" "$data" 2>/dev/null || true)
echo "    alerts: ${alerts:-none}"
check "Strata recorded an alert" '[ -n "$alerts" ]'
good_at=$(esp morph-test/good-at-ns | tr -d '\r\n' || true)
rewind_to=$(printf '%s\n' "$alerts" | head -1 | sed -nE 's/.*"rewind_to":([0-9]+).*/\1/p')
check "its rewind point is after the documents were complete" \
    '[ -n "$good_at" ] && [ -n "$rewind_to" ] && [ "$rewind_to" -gt "$good_at" ]'

# The documents: every one back, byte for byte.
esp morph-test/before.sha256 > "$WORK/before.sha256" || true
n=$(wc -l < "$WORK/before.sha256")
check "checksums were taken of $WANT_DOCS documents before the attack (got $n)" '[ "$n" -eq "$WANT_DOCS" ]'
out="$WORK/docs-out"
rm -rf "$out" && mkdir -p "$out"
debugfs -R "rdump $DOCS $out" "$data" >/dev/null 2>&1 || true
check "every document matches its checksum from before the attack" \
    '(cd "$out/Documents" && sha256sum --quiet -c "$WORK/before.sha256")'
left=$(cd "$out/Documents" 2>/dev/null && find . -type f \( -name '*.locked' -o -name 'READ_ME_TO_RECOVER.txt' \) | sort | tr '\n' ' ')
check "nothing of the attacker's is left among them (got: ${left:-nothing})" '[ -z "$left" ]'

# What the rewind moved away is kept, not deleted.
quarantined=$(debugfs -R "rdump /strata/quarantine $out" "$data" >/dev/null 2>&1;
    find "$out/quarantine" -type f -name '*.locked' 2>/dev/null | wc -l)
check "the .locked files went to quarantine (got $quarantined)" '[ "$quarantined" -gt 0 ]'

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $log"; fi
exit $fail
