#!/bin/bash
# The image a person uses, booted: does it come up, and does it STAY up?
#
#   images/x86-qemu/test-boot.sh <workdir> [seconds]
#
# Every other x86 test asks whether the machine does the right thing and then powers itself
# off. This one asks the opposite question, because `plain` has no self-test driver at all:
# it must reach a login prompt on the console, commit slot A, and keep running until the
# harness takes the VM away. A machine that powers itself off is a test rig, not an image
# somebody can use, and that distinction is the whole point of the scenario.
#
# The verdict comes from two places, neither of which is a log line the image chose to print:
#   - the boot history on the ESP, read off the disk after the VM is gone;
#   - the serial console, for the login prompt the getty produces.
set -euo pipefail

WORK=${1:?usage: test-boot.sh <workdir> [seconds]}
SECONDS_TO_RUN=${2:-150}

HERE=$(cd "$(dirname "$0")" && pwd)
DISK="$WORK/disk-plain.img"
[ -f "$DISK" ] || { echo "no $DISK: run build.sh $WORK plain first" >&2; exit 1; }

LOG="$WORK/serial-plain.log"
echo "=== plain: booting for ${SECONDS_TO_RUN}s, expecting a login prompt and no poweroff"
start=$(date +%s)
# A non-zero exit here is the EXPECTED outcome: `run-vm.sh` kills QEMU when its timeout
# expires, which is exactly what "the machine stayed up" looks like from outside.
set +e
"$HERE/run-vm.sh" "$DISK" "$LOG" "$SECONDS_TO_RUN"
code=$?
set -e
echo "    vm time: $(( $(date +%s) - start ))s (exit $code — 124 means the timeout took it, which is the pass)"

fail=0
if [ "$code" != 124 ]; then
    echo "    FAIL: the VM exited $code instead of still running — an image for a person must not shut itself down"
    fail=1
fi

# ⚠️ A GETTY PRINTING A BANNER IS NOT A PERSON BEING ABLE TO USE THE MACHINE. The first version
# of this check accepted `morpheos-a login:` — which is exactly what it looks like when a login
# prompt is waiting for a password that this image never sets. The marker below is printed by
# /root/.profile, so it appears only if a login actually COMPLETED and started a shell.
if grep -aq 'MORPHEOS CONSOLE READY' "$LOG"; then
    echo "    console: a login completed with nothing typed — $(grep -ao 'MORPHEOS CONSOLE READY.*' "$LOG" | tail -1)"
else
    echo "    FAIL: no completed login on the console. A prompt nobody can answer is not a usable image:"
    grep -aoE '[a-z0-9-]* login:|MORPHEOS CONSOLE READY' "$LOG" | tail -3 | sed 's/^/      /'
    tail -5 "$LOG" | sed 's/^/      /'
    fail=1
fi

# And the boot decision, from the disk rather than the console: slot A is committed, and the
# history has exactly one line, which is what "no trial, no fallback" looks like.
# The same parse the other scenarios use — the file holds one JSON record per boot, not the
# `<slot>/<phase>/<action>` summary this prints. Reading it as if it were already summarised was
# the second bug this check found in itself.
history=$(mtype -i "$DISK@@1M" ::/morph-boot/history.jsonl 2>/dev/null |
    sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p')
echo "    boots:   $(echo "$history" | tr '\n' ' ')"
if [ -z "$history" ]; then
    echo "    FAIL: the image recorded no boot history at all"
    fail=1
elif [ "$(echo "$history" | tr '\n' ' ' | tr -s ' ')" != "a/committed/none " ]; then
    echo "    FAIL: expected exactly one boot, slot A committed with nothing to fall back from"
    fail=1
fi

# The self-test driver must NOT be in this image. If it were, the machine would walk a
# scenario and power off — the failure this scenario exists to catch.
if mtype -i "$DISK@@1M" ::/MORPH-TEST-RESULT.TXT > /dev/null 2>&1; then
    echo "    FAIL: the hardware image carries a self-test result file — it is a test rig after all"
    fail=1
fi

# The hardware report (doc 20, E12), read off the stick's FAT partition the way a person reads
# it on their own computer. The VM's own facts must be in it: it is a QEMU machine, it has a
# screen connected, its disk is virtio with the virtio driver bound, and its keyboard is the
# PS/2 one. A report that is there but empty would pass a check for the file alone.
reports=$({ mdir -b -i "$DISK@@1M" ::/morpheos-report 2>/dev/null || true; } | sed 's|/$||; s|.*/||' | grep -E '^[0-9]{3}-' || true)
echo "    reports: $(echo "$reports" | tr '\n' ' ')"
if [ "$(echo "$reports" | grep -c .)" != 1 ]; then
    echo "    FAIL: expected exactly one hardware report on the stick after one boot"
    fail=1
else
    rdir="::/morpheos-report/$reports"
    summary=$(mtype -i "$DISK@@1M" "$rdir/summary.txt" 2>/dev/null || true)
    lspci=$(mtype -i "$DISK@@1M" "$rdir/lspci.txt" 2>/dev/null || true)
    report_check() {
        if [ "$2" = yes ]; then echo "    ok    report: $1"; else echo "    FAIL  report: $1"; fail=1; fi
    }
    yes_if() { if "$@" > /dev/null 2>&1; then echo yes; else echo no; fi; }
    report_check "the folder is the first report, from slot A ($reports)" \
        "$(yes_if grep -qE '^001-.*-a$' <<< "$reports")"
    report_check "the machine is the QEMU VM" "$(yes_if grep -q '^MACHINE   QEMU' <<< "$summary")"
    report_check "a screen is connected" "$(yes_if grep -qE '^  card[0-9]+-.*: connected' <<< "$summary")"
    report_check "the virtio disk has its driver bound" \
        "$(if grep -iE -A3 'virtio.*block' <<< "$lspci" | grep -q 'Kernel driver in use: virtio-pci'; then echo yes; else echo no; fi)"
    report_check "devices are named, not just numbered (the PCI name database is there)" \
        "$(yes_if grep -q 'Red Hat, Inc' <<< "$lspci")"
    report_check "the PS/2 keyboard is an input device" \
        "$(yes_if grep -q 'AT Translated Set 2 keyboard' <<< "$summary")"
    report_check "it says what firmware is missing (a section, even if 'none')" \
        "$(yes_if grep -q 'FIRMWARE THE KERNEL ASKED FOR AND DID NOT FIND' <<< "$summary")"
    if [ "$fail" != 0 ]; then
        echo "    the summary was:"
        sed 's/^/      | /' <<< "$summary" | head -40
    fi
fi

if [ "$fail" != 0 ]; then
    echo "plain image: FAILED" >&2
    exit 1
fi
echo "plain image: PASS — boots, reaches a login, commits slot A, stays up, and reports its hardware"
