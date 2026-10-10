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
set -euo pipefail

WORK=${1:?usage: test-fallback.sh <workdir> [good|unhealthy|crash ...]}
shift
SCENARIOS=("$@")
[ ${#SCENARIOS[@]} -gt 0 ] || SCENARIOS=(good unhealthy crash)
HERE=$(cd "$(dirname "$0")" && pwd)

# One line per boot that reached the health check: "<booted>/<phase>/<action>".
expected() {
    case "$1" in
        good)      printf '%s\n' a/committed/none b/trial/committed b/committed/none ;;
        unhealthy) printf '%s\n' a/committed/none b/trial/fallback a/committed/none a/committed/none ;;
        crash)     printf '%s\n' a/committed/none a/committed/none a/committed/none ;;
    esac
}
expected_default() { [ "$1" = good ] && echo b || echo a; }

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

fail=0
for sc in "${SCENARIOS[@]}"; do
    echo "=== scenario: $sc"
    disk=$("$HERE/build.sh" "$WORK" "$sc" | tail -1)
    rm -f "${disk%.img}.vars.fd"
    log="$WORK/serial-$sc.log"
    start=$(date +%s)
    "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-2400}" || { echo "VM did not power off cleanly"; fail=1; }
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
    if [ $ok = 1 ]; then echo "    PASS"; else fail=1; fi
done
exit $fail
