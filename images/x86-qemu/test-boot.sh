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

# A console a person can reach. `agetty` prints `/etc/issue` on the console it owns, which is
# the banner in this image — and with the autologin drop-in it then starts a shell without
# asking for a password. Either marker will do: a login prompt means the password path was
# reached, the banner means a getty is live on this console either way.
if grep -aqE 'never breaks, and heals itself|login:' "$LOG"; then
    echo "    console reached: $(grep -aoE 'never breaks, and heals itself|[a-z0-9-]* login:' "$LOG" | tail -1)"
else
    echo "    FAIL: no getty output on the console — the machine booted but nobody can use it"
    tail -5 "$LOG" | sed 's/^/      /'
    fail=1
fi

# And the boot decision, from the disk rather than the console: slot A is committed, and the
# history has exactly one line, which is what "no trial, no fallback" looks like.
history=$(mtype -i "$DISK@@1M" ::/morph-boot/history.jsonl 2>/dev/null || true)
echo "    boots:   $(echo "$history" | tr '\n' ' ')"
if [ -z "$history" ]; then
    echo "    FAIL: the image recorded no boot history at all"
    fail=1
elif ! echo "$history" | grep -q '^a/committed/none$'; then
    echo "    FAIL: expected slot A committed with nothing to fall back from"
    fail=1
fi

# The self-test driver must NOT be in this image. If it were, the machine would walk a
# scenario and power off — the failure this scenario exists to catch.
if mtype -i "$DISK@@1M" ::/MORPH-TEST-RESULT.TXT > /dev/null 2>&1; then
    echo "    FAIL: the hardware image carries a self-test result file — it is a test rig after all"
    fail=1
fi

if [ "$fail" != 0 ]; then
    echo "plain image: FAILED" >&2
    exit 1
fi
echo "plain image: PASS — boots, reaches a login, commits slot A, and stays up"
