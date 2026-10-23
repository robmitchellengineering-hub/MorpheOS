#!/bin/bash
# "Try to Break It", part 4: the recovery image (S4). Three proofs, in one script:
#
#   sudo images/x86-qemu/test-recovery.sh <workdir>
#
#   recovery-boot  — arm the recovery entry the way morph-boot arms a trial, reboot, and
#                    check that the recovery system rebuilt BOTH slots, set slot A default,
#                    and wrote its record on the ESP — with no extra boot-history line for
#                    the recovery boot itself (it is not a slot).
#   wipe           — seed /home and record checksums, then zero the first 64 MiB of both
#                    slot partitions from the harness; the next boot must arm recovery from
#                    the initramfs (the "way in") and heal, with every /home file surviving
#                    on the data partition.
#   wipe-in-machine — seed /home, then the running machine destroys BOTH slot partitions
#                    itself (wipefs + zero) and force-reboots; the initramfs "way in" must
#                    arm recovery and heal, with every /home file surviving on the data
#                    partition. This is the same destruction a person could cause from a
#                    root shell, done by the machine rather than by the harness.
#
# The verdict is read from the disk after the VM powers off (the same rule as
# test-fallback.sh: the serial console is diagnostics only, never the source of truth).
set -euo pipefail

WORK=${1:?usage: test-recovery.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
TYPE=4f68bce3-e8cd-4db1-96e7-fbcaf984b709

esp() { mtype -i "$disk@@1M" "::/$1" 2>/dev/null || true; }
esp_write() { # esp_write <src-file> <dst-path>
    mmd -i "$disk@@1M" "::/morph-recovery-test" 2>/dev/null || true
    mcopy -i "$disk@@1M" "$1" "::/morph-recovery-test/$2"
}

# A slot partition's /etc/hostname, read off the disk (debugfs finds it = a valid filesystem).
hostname_present() { # hostname_present <root-a|root-b>
    local name="$1"
    local img="$WORK/slot-$name.img"
    read -r start size < <(sfdisk -d "$disk" | sed -nE "s/.*start= *([0-9]+), size= *([0-9]+),.*name=\"$name\".*/\1 \2/p")
    [ -n "$start" ] || { echo "no $name partition on $disk" >&2; return 1; }
    dd if="$disk" of="$img" bs=512 skip="$start" count="$size" status=none
    out=$(debugfs -R "stat /etc/hostname" "$img" 2>&1)
    case "$out" in *"Inode:"*) return 0 ;; *) return 1 ;; esac
}

boots() {
    esp morph-boot/history.jsonl |
        sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p' | tr '\n' ' '
}
default_slot() { esp loader/loader.conf | awk '$1=="default"{print $2}'; }
recovered() { esp morph-boot/recovery.json; }

fail=0
check() { if eval "$2"; then echo "    ok    $1"; else echo "    FAIL  $1"; fail=1; fi; }

# ---------------------------------------------------------------------------
# Part 1: recovery-boot — arm recovery by hand and prove it rebuilds both slots.
# ---------------------------------------------------------------------------
echo "=== scenario: recovery-boot"
disk=$("$HERE/build.sh" "$WORK" recovery | tail -1)
rm -f "${disk%.img}.vars.fd"
echo arm > "$WORK/recovery-mode.txt"
esp_write "$WORK/recovery-mode.txt" mode
log="$WORK/serial-recovery-boot.log"
start=$(date +%s)
"$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-1800}" || { echo "    VM did not power off cleanly"; fail=1; }
echo "    vm time: $(( $(date +%s) - start ))s"

# The recovery boot must NOT append a history line (it is not a slot): two slot-A commits —
# one before arming, one after the rebuild — and nothing between them.
check "slot A committed before and after recovery, with no recovery line (got: $(boots))" \
    '[ "$(boots)" = "a/committed/none a/committed/none " ]'
check "the recovery record names both slots as rebuilt" \
    '[ -n "$(recovered)" ] && printf "%s" "$(recovered)" | grep -q "\"a\"" && printf "%s" "$(recovered)" | grep -q "\"b\""'
check "slot A is the default again" \
    '[ "$(default_slot)" = "morpheos-a.conf" ]'
check "root-a holds a valid filesystem (/etc/hostname present)" 'hostname_present root-a'
check "root-b holds a valid filesystem (/etc/hostname present)" 'hostname_present root-b'

# ---------------------------------------------------------------------------
# Part 2: wipe — zero both slot partitions; the initramfs must arm recovery.
# ---------------------------------------------------------------------------
echo "=== scenario: wipe"
disk=$("$HERE/build.sh" "$WORK" recovery | tail -1)
rm -f "${disk%.img}.vars.fd"
echo seed > "$WORK/recovery-mode.txt"
esp_write "$WORK/recovery-mode.txt" mode

# Run 1: the driver seeds /home and records checksums, then powers off.
log="$WORK/serial-wipe-seed.log"
"$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-1800}" || { echo "    seed VM did not power off cleanly"; fail=1; }
n=$(esp morph-recovery-test/before.sha256 | wc -l)
check "the driver seeded and recorded checksums for 24 files (got $n)" '[ "$n" -eq 24 ]'

# The wipe, from the harness: zero the first 64 MiB of each slot partition. This is what
# "the system root is gone" means to the machine — a partition with no filesystem at all.
for part in root-a root-b; do
    read -r start size < <(sfdisk -d "$disk" | sed -nE "s/.*start= *([0-9]+), size= *([0-9]+),.*name=\"$part\".*/\1 \2/p")
    [ -n "$start" ] || { echo "no $part partition on $disk" >&2; exit 1; }
    dd if=/dev/zero of="$disk" bs=1M count=64 seek=$((start * 512 / 1048576)) conv=notrunc status=none
done

# Run 2: slot A has no filesystem, so the initramfs "way in" arms recovery, which rebuilds
# both slots, and the machine lands back on slot A.
log="$WORK/serial-wipe-heal.log"
"$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-1800}" || { echo "    heal VM did not power off cleanly"; fail=1; }

check "slot A committed after the wipe and heal (got: $(boots))" \
    '[ "$(boots)" = "a/committed/none a/committed/none " ]'
check "the recovery record names both slots as rebuilt" \
    '[ -n "$(recovered)" ] && printf "%s" "$(recovered)" | grep -q "\"a\"" && printf "%s" "$(recovered)" | grep -q "\"b\""'
check "root-a holds a valid filesystem (/etc/hostname present)" 'hostname_present root-a'
check "root-b holds a valid filesystem (/etc/hostname present)" 'hostname_present root-b'

# Every /home file survived on the data partition (which the wipe never touched).
read -r dstart dsize < <(sfdisk -d "$disk" | sed -nE 's/.*start= *([0-9]+), size= *([0-9]+),.*name="data".*/\1 \2/p')
data="$WORK/data-out.img"
dd if="$disk" of="$data" bs=512 skip="$dstart" count="$dsize" status=none
esp morph-recovery-test/before.sha256 > "$WORK/before.sha256"
out="$WORK/home-out"
rm -rf "$out" && mkdir -p "$out"
debugfs -R "rdump /home/morph/Documents $out" "$data" >/dev/null 2>&1 || true
check "every /home file matches its checksum from before the wipe" \
    '(cd "$out/Documents" && sha256sum --quiet -c "$WORK/before.sha256")'

# ---------------------------------------------------------------------------
# Part 3: wipe-in-machine — the machine destroys its own slots and recovers.
# ---------------------------------------------------------------------------
echo "=== scenario: wipe-in-machine"
disk=$("$HERE/build.sh" "$WORK" recovery | tail -1)
rm -f "${disk%.img}.vars.fd"
echo wipe > "$WORK/recovery-mode.txt"
esp_write "$WORK/recovery-mode.txt" mode

# One run: slot A commits, then the driver seeds /home, wipes both slot partitions from
# inside the running machine, and force-reboots. The initramfs "way in" arms recovery, which
# rebuilds both slots, and the machine lands back on slot A.
log="$WORK/serial-wipe-in-machine.log"
"$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-1800}" || { echo "    VM did not power off cleanly"; fail=1; }

check "slot A committed before and after the in-machine wipe and heal (got: $(boots))" \
    '[ "$(boots)" = "a/committed/none a/committed/none " ]'
check "the recovery record names both slots as rebuilt" \
    '[ -n "$(recovered)" ] && printf "%s" "$(recovered)" | grep -q "\"a\"" && printf "%s" "$(recovered)" | grep -q "\"b\""'
check "root-a holds a valid filesystem (/etc/hostname present)" 'hostname_present root-a'
check "root-b holds a valid filesystem (/etc/hostname present)" 'hostname_present root-b'
n=$(esp morph-recovery-test/before.sha256 | wc -l)
check "the driver seeded and recorded checksums for 24 files (got $n)" '[ "$n" -eq 24 ]'

# Every /home file survived on the data partition (which the in-machine wipe never touched).
read -r dstart dsize < <(sfdisk -d "$disk" | sed -nE 's/.*start= *([0-9]+), size= *([0-9]+),.*name="data".*/\1 \2/p')
data="$WORK/data-out.img"
dd if="$disk" of="$data" bs=512 skip="$dstart" count="$dsize" status=none
esp morph-recovery-test/before.sha256 > "$WORK/before.sha256"
out="$WORK/home-out"
rm -rf "$out" && mkdir -p "$out"
debugfs -R "rdump /home/morph/Documents $out" "$data" >/dev/null 2>&1 || true
check "every /home file matches its checksum from before the wipe" \
    '(cd "$out/Documents" && sha256sum --quiet -c "$WORK/before.sha256")'

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $log"; fi
exit $fail
