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
#   heal       the same failed trial as `unhealthy`, and then the machine REPAIRS itself:
#              the in-image driver rebuilds the broken spare from the running slot with
#              `morph-heal` and trials it again, so slot B ends up **bootable and
#              committed** instead of left broken. The verdict is content, not flow: the
#              service whose failure broke the trial must be GONE from slot B's root, with
#              the same check run before the machine boots as the control.
set -euo pipefail

WORK=${1:?usage: test-fallback.sh <workdir> [good|unhealthy|crash|update|heal ...]}
shift
SCENARIOS=("$@")
[ ${#SCENARIOS[@]} -gt 0 ] || SCENARIOS=(good unhealthy crash update heal)
HERE=$(cd "$(dirname "$0")" && pwd)

# One line per boot that reached the health check: "<booted>/<phase>/<action>".
expected() {
    case "$1" in
        good)      printf '%s\n' a/committed/none b/trial/committed b/committed/none ;;
        unhealthy) printf '%s\n' a/committed/none b/trial/fallback a/committed/none a/committed/none ;;
        crash)     printf '%s\n' a/committed/none a/committed/none a/committed/none ;;
        update)    printf '%s\n' a/committed/none b/trial/committed b/committed/none ;;
        # `unhealthy`'s four boots, then the repair, then the repaired slot trialling and
        # committing: the machine ends up on B, which is the whole point — a healed slot is
        # one the machine can USE, not merely one that was written to.
        heal)      printf '%s\n' a/committed/none b/trial/fallback a/committed/none b/trial/committed b/committed/none ;;
    esac
}
expected_default() { case "$1" in good|update|heal) echo b ;; *) echo a ;; esac; }

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

# Verdict for the heal scenario: does slot B's own root still carry the service that made
# its trial fail? `build.sh` puts `morph-broken.service` into slot B for the `unhealthy`
# scenario, and a repair replaces that root with the running slot's — so the file being GONE
# is the evidence that the spare was rebuilt, and it is read off the disk rather than taken
# from a log line. Same helper shape as `marker_present`, including the debugfs caveat.
broken_service_present() {
    local loop out
    loop=$(losetup -Pf --show "$1")
    out=$(debugfs -R "stat /usr/lib/systemd/system/morph-broken.service" "${loop}p3" 2>&1)
    losetup -d "$loop"
    case "$out" in *"Inode:"*) return 0 ;; *) return 1 ;; esac
}

fail=0
for sc in "${SCENARIOS[@]}"; do
    echo "=== scenario: $sc"
    # Both of these reuse a disk built for another scenario, deliberately. `update` uses
    # `good` because slot B is a healthy system either way and the scenario's change is that
    # morph-update REPLACES it first. `heal` uses `unhealthy` because that is the disk whose
    # slot B has a service that fails — which is precisely what there is to heal.
    build_sc=$sc
    [ "$sc" = update ] && build_sc=good
    [ "$sc" = heal ] && build_sc=unhealthy
    disk=$("$HERE/build.sh" "$WORK" "$build_sc" | tail -1)
    rm -f "${disk%.img}.vars.fd"

    payload=""
    # The same control for the self-test report: if it is already on the disk, the file proves
    # nothing about the machine that just ran. It must be written BY the run.
    if mtype -i "$disk@@1M" ::/MORPH-TEST-RESULT.TXT > /dev/null 2>&1; then
        echo "    FAIL: the self-test report is already on the disk before the VM booted"
        fail=1
    fi
    if [ "$sc" = update ]; then
        # The control, and it has to come first: if slot B already carried the marker,
        # the assertion after the run would prove nothing.
        if marker_present "$disk"; then
            echo "    FAIL: slot B already has the update marker before the VM booted"
            fail=1
        fi
        payload=$("$HERE/build-payload.sh" "$WORK" "$WORK/payload.img" | tail -1)
    fi
    if [ "$sc" = heal ]; then
        # The control for this one: slot B must HAVE the broken service, or a "repair" would
        # be indistinguishable from nothing having happened.
        if broken_service_present "$disk"; then
            echo "    slot b carries the broken service its trial will fail on"
        else
            echo "    FAIL: slot B has no broken service, so this scenario would heal nothing"
            fail=1
        fi
        payload=$("$HERE/build-payload.sh" "$WORK" "$WORK/heal.img" heal | tail -1)
    fi

    log="$WORK/serial-$sc.log"
    start=$(date +%s)
    # 900s, not 2400s: the passing scenarios take under 30 SECONDS each, so a run that
    # reaches a quarter of an hour is hung, and the only thing a longer timeout buys is a
    # longer wait to find that out. Debugging a hang against a 40-minute loop is how a
    # twenty-minute stall got read twice as "probably just slow".
    "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-900}" "$payload" || { echo "VM did not power off cleanly"; fail=1; }
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
    # The report: written by the machine, on the FAT partition a person can read without a
    # serial cable. This is the mechanism the Pi's first boot depends on, so it is proven here
    # on the platform that can be run — the report file, its trail, and the fact that it
    # survives the poweroff.
    report=$(mtype -i "$disk@@1M" ::/MORPH-TEST-RESULT.TXT 2>/dev/null || true)
    if [ -z "$report" ]; then
        echo "    FAIL: the machine wrote no self-test report"
        ok=0
    else
        echo "$report" | grep -q '^RESULT:' || { echo "    FAIL: the report has no RESULT line"; ok=0; }
        echo "$report" | grep -q 'step 0: starting' || { echo "    FAIL: the report does not say the walk started"; ok=0; }
        echo "$report" | grep -q 'arming trial of slot b' || { echo "    FAIL: the report does not name the trial"; ok=0; }
        # The trail must name what THIS scenario did, not merely exist. `RESULT:` is UNKNOWN on
        # x86 by design — the image declares no expected slot here, because the verdict for this
        # platform is the boot history below. What the file has to prove is that the machine
        # recorded the steps it actually took.
        case "$sc" in
            update)
                echo "$report" | grep -q 'installing the payload' || { echo "    FAIL: the report does not mention the install"; ok=0; }
                echo "$report" | grep -q 'morph-update reported success' || { echo "    FAIL: the report does not mention the install succeeding"; ok=0; }
                ;;
            heal)
                echo "$report" | grep -q 'repairing it from the running system' || { echo "    FAIL: the report does not mention the repair"; ok=0; }
                echo "$report" | grep -q 'morph-heal reported success' || { echo "    FAIL: the report does not mention the repair succeeding"; ok=0; }
                ;;
            *)
                echo "$report" | grep -q 'arming trial of slot b' || { echo "    FAIL: the report does not mention the trial"; ok=0; }
                ;;
        esac
        if [ $ok = 1 ]; then
            echo "    report: $(echo "$report" | grep '^RESULT:' | head -1) ($(echo "$report" | wc -l | tr -d ' ') lines, written by the machine)"
        fi
    fi
    if [ "$sc" = heal ]; then
        # The verdict, and it is content rather than flow: the service whose failure broke
        # the trial must be GONE from slot B's root. The history above would look identical
        # if the repair had done nothing at all and slot B happened to boot anyway, so this
        # is the assertion that means something.
        if broken_service_present "$disk"; then
            echo "    FAIL: slot b still carries the broken service — the spare was never repaired"
            ok=0
        else
            echo "    slot b no longer carries the broken service: the spare was rebuilt from the running slot AND booted"
        fi
    fi
    if [ $ok = 1 ]; then
        echo "    PASS"
        # Reclaimed while the verdicts are already in hand: the work directory also holds the
        # hardware image, and four disks of ~3 GiB is the difference between fitting on a
        # runner and not. A FAILED disk is kept for the next look.
        rm -f "$disk" "$payload"
    else
        fail=1
    fi
done
exit $fail
