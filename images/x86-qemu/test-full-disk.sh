#!/bin/bash
# "Try to Break It", part 4: a full disk does not break the machine (doc 20, S7).
#
#   sudo images/x86-qemu/test-full-disk.sh <workdir>
#
# Two VM runs (S7 note 3), then the verdict is read from the disk (AGENTS.md H8), never from
# what the VM says about itself:
#
#   Run 1, guest clock 30 days ago: the driver writes five versions of three files, so Strata
#           has versions old enough for the retention policy to prune.
#   Run 2, today: the driver fills /persist as root until a write fails with ENOSPC, and
#           reboots. On that second boot the space guard prunes the old versions and the shell
#           shows the notice; a screendump is taken while the notice is up.
#
# Then, off the disk:
#   * boot history: the fill boot was committed and healthy — strata.service not a failed unit;
#   * the store, copied out and read with the host's own `strata --store`: versions older than
#     the policy are gone, and the newest version of every file is kept;
#   * dumpe2fs on the copied partition: free space is back above storage.min_free_percent, or
#     the notice says why not (the guard prunes by the retention policy only, never beyond it);
#   * a screendump shows the notice.
set -euo pipefail

WORK=${1:?usage: test-full-disk.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
STRATA=${STRATA:-"$REPO/target/release/strata"}
DOCS=/home/morph/Documents

esp() { mtype -i "$disk@@1M" "::/$1" 2>/dev/null || true; }
fail=0
check() { if eval "$2"; then echo "    ok    $1"; else echo "    FAIL  $1"; fail=1; fi; }

echo "=== scenario: full-disk"
disk=$("$HERE/build.sh" "$WORK" full-disk | tail -1)
rm -f "${disk%.img}.vars.fd"

# Run 1: several versions of a few files, stamped 30 days ago so the policy prunes them.
old=$(date -u -d '30 days ago' '+%Y-%m-%dT%H:%M:%S')
echo "    writing old versions with the guest clock at $old"
VM_RTC="$old" VM_GPU=1 "$HERE/run-vm.sh" "$disk" "$WORK/serial-full-disk-1.log" "${VM_TIMEOUT:-1800}" \
    || { echo "    run 1 did not power off cleanly"; fail=1; }

# Run 2: fill the disk, reboot, and screendump the notice once it is up.
log="$WORK/serial-full-disk-2.log"
qmp="$WORK/qmp-full-disk.sock"
rm -f "$log" "$qmp" "$WORK"/screen-full-disk-*.ppm
# Run 2 must start from a fresh UEFI variable store. Run 1 booted through OVMF's
# removable-media fallback (/EFI/BOOT/BOOTX64.EFI, build.sh), and the NVRAM entry systemd-boot
# left behind on that boot is not usable next boot — reusing run 1's vars.fd drops run 2 into
# the UEFI Interactive Shell before the kernel runs. Nothing this scenario needs lives in NVRAM
# (its inter-run state is the Strata store and the step file on the disk), so a fresh vars.fd is
# correct, and it makes run 2 boot the same fallback that worked for run 1.
rm -f "${disk%.img}.vars.fd"
VM_GPU=1 VM_QMP="$qmp" "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-1800}" &
vm=$!

# The notice appears on the second boot, after the fill and the reboot; wait for it before
# screendumping (AGENTS.md H14: wait for the "ready" signal, never sleep past it).
shown=""
for _ in $(seq 1 600); do
    if grep -aq 'flow-notice: shown' "$log" 2>/dev/null; then shown=1; break; fi
    if ! kill -0 "$vm" 2>/dev/null; then break; fi
    sleep 1
done
if [ -z "$shown" ]; then
    echo "    FAIL: the notice never showed"
    grep -a 'space-guard\|flow-notice\|strata' "$log" | tail -20 || true
    fail=1
else
    python3 - "$qmp" "$WORK" <<'PY' || fail=1
import json, socket, sys, time

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
path = f"{work}/screen-full-disk-notice.ppm"

def read_ppm():
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
    return w, h, px

# The notice's client draws only after the compositor answers its frame callback (H17), so a
# single screendump can race the first frame. Re-screendump briefly until the WARN band shows.
warn = 0
for attempt in range(10):
    q.cmd("screendump", filename=path)
    w, h, px = read_ppm()
    warn = sum(1 for k in range(0, w * h * 3, 3) if tuple(px[k:k+3]) == WARN)
    if warn > 2000:
        break
    time.sleep(1)

print(f"    screen {w}x{h}; {warn} pixels of the notice's warning colour")
if warn > 2000:
    sys.exit(0)
print("    FAIL: the screendump does not show the notice's warning band")
sys.exit(1)
PY
fi

# Wait for the decision before stopping (AGENTS.md H13), then let the driver power off.
for _ in $(seq 1 300); do
    grep -aq 'morph-boot-check: decision' "$log" 2>/dev/null && break
    kill -0 "$vm" 2>/dev/null || break
    sleep 1
done
wait "$vm" 2>/dev/null || true

# --- The verdicts, read off the disk. ---

step=$(esp morph-test/step | tr -d '\r\n')
check "self-test reached the final boot (step 3, got '$step')" '[ "$step" = 3 ]'

enospc=$(esp morph-test/enospc | tr -d '\r\n')
check "the fill reached ENOSPC (got '$enospc')" '[ "$enospc" = "enospc" ]'

last=$(esp morph-boot/history.jsonl | tail -1)
check "the fill boot was committed and healthy (got: $last)" \
    'printf "%s" "$last" | grep -q "\"state\":\"running\"" && printf "%s" "$last" | grep -q "\"phase\":\"committed\"" && printf "%s" "$last" | grep -q "\"action\":\"none\""'
check "strata.service was not a failed unit" \
    '! grep -aq "failed unit: strata" "$WORK/serial-full-disk-2.log"'

# The data partition, copied out, so the host's own `strata` reads the store and dumpe2fs
# reads the free space — both from the disk, not from the console.
read -r dstart dsize < <(sfdisk -d "$disk" | sed -nE 's/.*start= *([0-9]+), size= *([0-9]+),.*name="data".*/\1 \2/p')
data="$WORK/data-out.img"
dd if="$disk" of="$data" bs=512 skip="$dstart" count="$dsize" status=none
out="$WORK/data-out"
rm -rf "$out"; mkdir -p "$out"
debugfs -R "rdump /strata $out" "$data" >/dev/null 2>&1 || true

stats=$("$STRATA" --store "$out/strata" stats 2>/dev/null || true)
check "the newest version of every file is kept, and the retention buckets are honoured (got: $stats)" \
    'printf "%s" "$stats" | grep -q "6 versions of 3 files"'

mkdir -p "$DOCS"
for f in a b c; do
    n=$("$STRATA" --store "$out/strata" history "$DOCS/$f.txt" 2>/dev/null | wc -l)
    check "$DOCS/$f.txt keeps its newest version and one daily bucket (got $n)" '[ "$n" -eq 2 ]'
done
rmdir "$DOCS" 2>/dev/null || true

# Free space back above the floor, or the notice says why not. The floor is the value morphd
# wrote to the data partition (H5: read it, never assume); the "why not" is the guard's own
# line, which says "still below the floor" when pruning could not get back above it.
floor=$(debugfs -R "cat /morph/strata-min-free.json" "$data" 2>/dev/null | sed -nE 's/.*"min_free_percent": *([0-9]+).*/\1/p' || true)
floor=${floor:-10}
free=$(dumpe2fs -h "$data" 2>/dev/null | awk -F: '/Free blocks/ {gsub(/ /, "", $2); print $2}' || true)
total=$(dumpe2fs -h "$data" 2>/dev/null | awk -F: '/Block count/ {gsub(/ /, "", $2); print $2}' || true)
free_pct=$(( free * 100 / total ))
guard_line=$(grep -a 'space-guard: free space' "$log" | tail -1 || true)
check "free space recovered or the notice says why not (free ${free_pct}%, floor ${floor}%)" \
    '[ "$free_pct" -ge "$floor" ] || printf "%s" "$guard_line" | grep -q "still below the"'

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $WORK/serial-full-disk-*.log"; fi
exit $fail
