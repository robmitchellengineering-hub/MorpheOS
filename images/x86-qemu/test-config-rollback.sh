#!/bin/bash
# "Try to Break It", part 6: a corrupted setting rolls back (doc 20, S8).
#
#   sudo images/x86-qemu/test-config-rollback.sh <workdir>
#
# Two VM runs:
#
#   run 1 — apply generation 2 (a new hostname) through `morphd apply`, which commits it; then
#           corrupt generation 2's stored FILE by hand — the generation file, never the
#           `current` symlink (it is a symlink renamed into place, so corrupting it would prove
#           a different failure). The hook writes the kernel hostname to the ESP and powers off.
#   run 2 — boot with the corrupted generation in force. morphd rolls it back to generation 1,
#           records the heal, and the shell shows the notice naming generation 2. The hook writes
#           the kernel hostname (generation 1's again) and powers off; the harness screendumps
#           the notice while it is up.
#
# The verdict is read off the disk after the VM stops (AGENTS.md H8), never from the console:
#   * the store's `current` names generation 1, and generation 2 is still in the store;
#   * heal.jsonl names generation 2, the reason, and generation 1 as now in force;
#   * generation 2's own record (meta + .rob mark) names it and its approver;
#   * the kernel hostname on the ESP is generation 1's again;
#   * a screendump shows the notice, and flow-notice's own "shown" line names generation 2.
set -euo pipefail

WORK=${1:?usage: test-config-rollback.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)

esp() { mtype -i "$disk@@1M" "::/$1" 2>/dev/null || true; }
# Read a file off the copied data partition with debugfs, without mounting anything (the repo's
# H8 rule: verdicts come from the disk, never the console).
dcat() { debugfs -R "cat $1" "$data" 2>/dev/null || true; }
dstat() { debugfs -R "stat $1" "$data" 2>&1 || true; }

fail=0
check() { if eval "$2"; then echo "    ok    $1"; else echo "    FAIL  $1"; fail=1; fi; }

echo "=== scenario: config-rollback"
disk=$("$HERE/build.sh" "$WORK" config-rollback | tail -1)
rm -f "${disk%.img}.vars.fd"

# Run 1: apply generation 2 (a new hostname) through morphd, and corrupt its stored file.
VM_GPU=1 "$HERE/run-vm.sh" "$disk" "$WORK/serial-config-rollback-1.log" "${VM_TIMEOUT:-1800}" \
    || { echo "    run 1 did not power off cleanly"; fail=1; }

check "generation 2 applied cleanly (hostname $(esp config-rollback/hostname))" \
    '[ "$(esp config-rollback/hostname)" = "morpheos-rollback" ]'

# Run 2: boot with the corrupted generation; morphd rolls back and the shell shows the notice.
log="$WORK/serial-config-rollback-2.log"
qmp="$WORK/qmp-config-rollback.sock"
rm -f "$log" "$qmp" "$WORK"/screen-config-rollback-*.ppm
# Run 2 must start from a fresh UEFI variable store (see test-full-disk.sh): run 1's NVRAM
# entry is not usable next boot, and reusing its vars.fd drops run 2 into the UEFI Interactive
# Shell before the kernel runs. Nothing this scenario needs lives in NVRAM, so a fresh vars.fd
# is correct and makes run 2 boot the same removable-media fallback run 1 used.
rm -f "${disk%.img}.vars.fd"
VM_GPU=1 VM_QMP="$qmp" "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-1800}" &
vm=$!

# The notice appears on the rollback boot; wait for it before screendumping (AGENTS.md H14: wait
# for the "ready" signal, never sleep past it).
shown=""
for _ in $(seq 1 600); do
    if grep -aq 'flow-notice: shown' "$log" 2>/dev/null; then shown=1; break; fi
    if ! kill -0 "$vm" 2>/dev/null; then break; fi
    sleep 1
done
if [ -z "$shown" ]; then
    echo "    FAIL: the notice never showed"
    grep -a 'morphd\|flow-notice\|config-rollback' "$log" | tail -20 || true
    fail=1
else
    python3 - "$qmp" "$WORK" <<'PY' || fail=1
import json, socket, sys

qmp_path, work = sys.argv[1], sys.argv[2]
WARN = (0xFF, 0x6A, 0x00)  # flow-notice's colours::WARN — the notice's accent band

class Qmp:
    def __init__(self, path):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.connect(path)
        self.f = self.sock.makefile("rwb")
        self.f.readline()  # the greeting
        self.cmd("qmp_capabilities")

    def cmd(self, name, **args):
        self.f.write((json.dumps({"execute": name, "arguments": args}) + "\n").encode())
        self.f.flush()
        while True:
            msg = json.loads(self.f.readline())
            if "return" in msg:
                return msg["return"]
            if "error" in msg:
                raise RuntimeError(f"{name}: {msg['error']}")

q = Qmp(qmp_path)
path = f"{work}/screen-config-rollback-notice.ppm"
q.cmd("screendump", filename=path)
data = open(path, "rb").read()
fields, i = [], 0
while len(fields) < 4:  # P6 header: magic, width, height, maxval
    while data[i:i+1].isspace():
        i += 1
    j = i
    while not data[j:j+1].isspace():
        j += 1
    fields.append(data[i:j])
    i = j
w, h = int(fields[1]), int(fields[2])
px = data[i+1:]
warn = sum(1 for k in range(0, w * h * 3, 3) if tuple(px[k:k+3]) == WARN)
print(f"    screen {w}x{h}; {warn} pixels of the notice's warning colour")
if warn > 2000:
    sys.exit(0)
print("    FAIL: the screendump does not show the notice's warning band")
sys.exit(1)
PY
fi

# Wait for the decision before reading verdicts (AGENTS.md H13), then let the driver power off.
for _ in $(seq 1 300); do
    grep -aq 'morph-boot-check: decision' "$log" 2>/dev/null && break
    kill -0 "$vm" 2>/dev/null || break
    sleep 1
done
wait "$vm" 2>/dev/null || true

# --- The verdicts, read off the disk. ---

# The data partition, copied out, so the store and its heal record are read from the disk.
read -r dstart dsize < <(sfdisk -d "$disk" | sed -nE 's/.*start= *([0-9]+), size= *([0-9]+),.*name="data".*/\1 \2/p')
data="$WORK/data-out.img"
dd if="$disk" of="$data" bs=512 skip="$dstart" count="$dsize" status=none

# The store's `current` is a symlink; `debugfs cat` refuses a non-regular file, so `stat`'s
# `Fast link dest` line is what says which generation it names. e2fsprogs 1.47 prints that
# field with a space — `Fast link dest: "generations/0001.json"` — not an underscore, and it
# quotes the target; the assertion matches the real output rather than the assumed spelling.
check "the store's current names generation 1 (got: $(dstat /morph/current | grep -o 'Fast link dest:.*' || true))" \
    'printf "%s" "$(dstat /morph/current)" | grep -q "Fast link dest: .*generations/0001.json"'

# Rollback moves the pointer only; generation 2 is still in the store (store.rs property 3).
check "generation 2 is still in the store (not deleted)" \
    'case "$(dstat /morph/generations/0002.json)" in *"Inode:"*) ;; *) false ;; esac'

# The heal record: the store's own record names N, the reason, and N-1 as now in force.
check "the heal record names generation 2 as the bad one (got: $(dcat /morph/heal.jsonl))" \
    'printf "%s" "$(dcat /morph/heal.jsonl)" | grep -q "\"bad\":2"'
check "the heal record names generation 1 as now in force" \
    'printf "%s" "$(dcat /morph/heal.jsonl)" | grep -q "\"now\":1"'
check "the heal record names the reason (a digest mismatch)" \
    'printf "%s" "$(dcat /morph/heal.jsonl)" | grep -q "does not match its digest"'
check "the heal record is rung H2" \
    'printf "%s" "$(dcat /morph/heal.jsonl)" | grep -q "\"rung\":\"H2\""'

# "Names the transaction" comes from the store's own record (number + approver, and the .rob
# mark), not a log line — AGENTS.md H8.
check "generation 2's record names it and its approver (got: $(dcat /morph/generations/0002.meta.json))" \
    'printf "%s" "$(dcat /morph/generations/0002.meta.json)" | grep -q "\"number\": 2" && printf "%s" "$(dcat /morph/generations/0002.meta.json)" | grep -q "\"approved_by\": \"test\""'
check "generation 2 carries the approver's .rob mark" \
    'case "$(dstat /morph/generations/0002.rob)" in *"Inode:"*) ;; *) false ;; esac'

# The machine shows generation 1's hostname again (written to the ESP by the hook).
check "the kernel hostname is generation 1's again (got: $(esp config-rollback/hostname))" \
    '[ "$(esp config-rollback/hostname)" = "morpheos-a" ]'

# The notice shown is the rollback notice naming generation 2 (flow-notice's own record of what
# it displayed; the screendump above proves it was on screen).
check "the notice names generation 2" \
    'grep -aq "flow-notice: shown .*generation 2" "$log"'

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $WORK/serial-config-rollback-*.log"; fi
exit $fail
