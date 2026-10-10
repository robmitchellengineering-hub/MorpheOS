#!/bin/bash
# "Try to Break It", part 1: A/B updates on a real UEFI boot chain.
#
#   sudo images/x86-qemu/test-fallback.sh <workdir> [scenario...]
#
# For each scenario, build a disk whose slot B holds an "update", boot it in
# QEMU, let the in-image test driver run the update flow, and check which slot
# ran at every boot and what morph-boot decided.
#
# The verdict comes from the boot history the image itself writes to the ESP
# (/morph-boot/history.jsonl), read from the disk after the VM powers off. The
# serial console is NOT the source of truth: the kernel prints to it too and can
# land in the middle of a line, which once made a correct boot unreadable on a
# fast (KVM) runner. The console is still parsed, and any disagreement printed,
# as a diagnostic.
#
#   good       B is healthy  → committed, and still B after a plain reboot
#   unhealthy  B boots but a service fails → one trial, then back on A for good
#   crash      B's kernel panics → one trial, then back on A for good
#   update     slot B starts without the new system: the in-image driver installs a
#              payload into it with `morph-update` FIRST, then trials it. The flow is
#              `good`'s; what differs is that the slot B which boots was **written
#              during the run**. Every other scenario trials a slot B that build.sh
#              had already populated — which, since both slots come from one image, is
#              a copy of the system already running, so no update happens at all.
set -euo pipefail

WORK=${1:?usage: test-fallback.sh <workdir> [good|unhealthy|crash ...]}
shift
SCENARIOS=("$@")
[ ${#SCENARIOS[@]} -gt 0 ] || SCENARIOS=(good unhealthy crash update)
HERE=$(cd "$(dirname "$0")" && pwd)

# One line per boot that reached the health check: "<booted>/<phase>/<action>".
expected() {
    case "$1" in
        good)      printf '%s\n' a/committed/none b/trial/committed b/committed/none ;;
        unhealthy) printf '%s\n' a/committed/none b/trial/fallback a/committed/none a/committed/none ;;
        crash)     printf '%s\n' a/committed/none a/committed/none a/committed/none ;;
        update)    printf '%s\n' a/committed/none b/trial/committed b/committed/none ;;
    esac
}
expected_default() { case "$1" in good|update) echo b ;; *) echo a ;; esac; }

# The verdict: one "<booted>/<phase>/<action>" per line of the boot history.
recorded() {
    mtype -i "$1@@1M" ::/morph-boot/history.jsonl 2>/dev/null |
        sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p'
}

# Diagnostics only: the same, scraped from the serial console.
console() {
    # Pair each status line with the decision line that follows it.
    grep -aoE 'morph-boot-check: (status|decision) \{[^}]*\}' "$1" |
        sed -E 's/.*"booted":"([ab])","default":"[ab]","phase":"([a-z]+)".*/S \1\/\2/;
                s/.*"action":"([a-z]+)".*/D \1/' |
        awk '$1=="S"{s=$2} $1=="D"{print s "/" $2}'
}

# Verdict for the update scenario: does slot B's own root now carry the file
# morph-update writes? Read off the disk after the VM powers off, with the partition
# mounted by the kernel — not from a log line the image could have printed for any
# reason. The boot history shows the FLOW; only this shows the CONTENT changed.
marker_present() {
    local loop out
    loop=$(losetup -Pf --show "$1")
    # ⚠️ DEBUGFS EXITS 0 WHETHER OR NOT THE FILE EXISTS. The first version of this
    # treated the exit code as the answer and reported the marker present on every disk,
    # so the control check failed on a slot that had never been written — a check
    # satisfied by something other than its subject, which is the failure mode this repo
    # keeps meeting. Parse what it printed instead.
    out=$(debugfs -R "stat /etc/morph-update-marker" "${loop}p3" 2>&1)
    losetup -d "$loop"
    case "$out" in *"Inode:"*) return 0 ;; *) return 1 ;; esac
}

fail=0
for sc in "${SCENARIOS[@]}"; do
    echo "=== scenario: $sc"
    # `update` reuses the `good` disk deliberately: slot B is a healthy system either
    # way, and what the scenario changes is that morph-update REPLACES it first.
    build_sc=$sc
    [ "$sc" = update ] && build_sc=good
    disk=$("$HERE/build.sh" "$WORK" "$build_sc" | tail -1)
    rm -f "${disk%.img}.vars.fd"

    payload=""
    if [ "$sc" = update ]; then
        # The control, and it has to come first: if slot B already carried the marker,
        # the assertion after the run would prove nothing.
        if marker_present "$disk"; then
            echo "    FAIL: slot B already has the update marker before the VM booted"
            fail=1
        fi
        payload=$("$HERE/build-payload.sh" "$WORK" "$WORK/payload.img" | tail -1)
    fi

    log="$WORK/serial-$sc.log"
    start=$(date +%s)
    "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-2400}" "$payload" || { echo "VM did not power off cleanly"; fail=1; }
    echo "    vm time: $(( $(date +%s) - start ))s"

    got=$(recorded "$disk")
    want=$(expected "$sc")
    default=$(mtype -i "$disk@@1M" ::/loader/loader.conf | awk '$1=="default"{print $2}')
    echo "    boots:   $(echo $got)"
    echo "    default: $default"
    seen=$(console "$log")
    [ "$seen" = "$got" ] || echo "    note: console showed $(echo $seen) (diagnostic only; history is the verdict)"

    ok=1
    [ "$got" = "$want" ] || { echo "    FAIL: expected boots: $(echo $want)"; ok=0; }
    [ "$default" = "morpheos-$(expected_default "$sc").conf" ] || { echo "    FAIL: expected default slot $(expected_default "$sc")"; ok=0; }
    if [ "$sc" = crash ]; then
        if grep -aq 'Kernel panic' "$log"; then
            echo "    slot b panicked as intended"
        else
            echo "    FAIL: slot b never panicked"; ok=0
        fi
    fi
    if [ "$sc" = update ]; then
        # The verdict. The history above shows the trial flow ran; only the marker shows
        # that the slot B which booted is the one morph-update WROTE, rather than the
        # one build.sh had already put there.
        if marker_present "$disk"; then
            echo "    slot b carries the payload's marker: the update was written AND booted"
        else
            echo "    FAIL: slot b booted without the payload's marker — the trial ran on the pre-built image"
            ok=0
        fi
    fi
    if [ $ok = 1 ]; then echo "    PASS"; else fail=1; fi
done
exit $fail
