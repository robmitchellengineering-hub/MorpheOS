#!/bin/bash
# The rewind timeline on the Flow shell (doc 20, S6).
#
#   sudo images/x86-qemu/test-rewind.sh <workdir>
#
# Boots the `rewind` image: the self-test attacks /home/morph/Documents the way
# test-ransomware.sh's does, but does NOT rewind. Strata records the alert, a .path unit
# opens the flow-timeline window by itself, and the harness chooses the alert and confirms
# the rewind on the VM's own keyboard. Screendumps show the window before (the list) and
# after (its green result band).
#
# The verdict is read from the disk after the VM powers off, never from the console or from
# what the self-test says (AGENTS.md H8, H13): the shared ransomware verdict in
# verify-ransomware.sh (all 24 documents match, nothing of the attacker's is left, the
# .locked files are in quarantine), plus two rewind-specific facts:
#   * the quarantine folder's timestamp is later than the confirming keypress, so the shell
#     did the rewind and not a script;
#   * the boot was healthy and committed.
set -euo pipefail

WORK=${1:?usage: test-rewind.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
DOCS=/home/morph/Documents   # as the VM sees it; on the data partition: /home/morph/Documents
WANT_DOCS=24
# shellcheck source=verify-ransomware.sh
. "$HERE/verify-ransomware.sh"

echo "=== rewind: the timeline opens by itself and the person confirms it"
disk=$("$HERE/build.sh" "$WORK" rewind | tail -1)
rm -f "${disk%.img}.vars.fd"
log="$WORK/serial-rewind.log"
qmp="$WORK/qmp-rewind.sock"
rm -f "$log" "$qmp" "$WORK"/screen-rewind-*.ppm "$WORK/confirm-at"

start=$(date +%s)
VM_GPU=1 VM_QMP="$qmp" "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-2400}" &
vm=$!

# Wait for the window to open with the FULL alert (H14). The parent folder appears only once
# Strata has re-issued the alert to cover it (the first alert names only clients/), so waiting
# for it means the window has every folder the attack hit, not just the first (doc 20, S6).
ready=""
for _ in $(seq 1 600); do
    if grep -aq 'flow-timeline: points .* /home/morph/Documents(' "$log" 2>/dev/null &&
        grep -aq 'flow-timeline: ready' "$log" 2>/dev/null; then
        ready=1
        break
    fi
    if ! kill -0 "$vm" 2>/dev/null; then break; fi
    sleep 1
done

fail=0
if [ -z "$ready" ]; then
    echo "    FAIL  the timeline never opened with the full alert"
    grep -a 'flow-timeline\|strata\|morph-test' "$log" | tail -30 || true
    fail=1
else
    python3 - "$qmp" "$WORK" <<'PY' || fail=1
import json, socket, sys, time

qmp_path, work = sys.argv[1], sys.argv[2]
SELECTED = (0x7a, 0x4a, 0x00)   # the selected row's band (flow-timeline colours::SELECTED)
CONFIRM = (0xff, 0xc0, 0x00)   # the confirm band after the dry run
OK = (0x00, 0xc0, 0x7a)        # the green result band once the rewind is done

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

def shot(name):
    path = f"{work}/screen-rewind-{name}.ppm"
    q.cmd("screendump", filename=path)
    data = open(path, "rb").read()
    fields, i = [], 0
    while len(fields) < 4:  # P6 header: magic, width, height, maxval
        while data[i:i+1].isspace(): i += 1
        j = i
        while not data[j:j+1].isspace(): j += 1
        fields.append(data[i:j]); i = j
    w, h = int(fields[1]), int(fields[2])
    px = data[i+1:]
    return w, h, [tuple(px[k:k+3]) for k in range(0, w * h * 3, 3)]

def count(screen, rgb):
    return sum(1 for p in screen[2] if p == rgb)

ok = True
def check(name, cond):
    global ok
    print(f"    {'ok  ' if cond else 'FAIL'}  {name}")
    ok = ok and cond
    return cond

def wait_for(name, rgb, label, seconds=40):
    end = time.time() + seconds
    while True:
        screen = shot(name)
        good = count(screen, rgb) > 1000  # a full-width band, not a stray pixel
        if good or time.time() > end:
            return screen, check(f"{label} (screendump {name})", good)
        time.sleep(0.5)

def tablet(x, y, w, h):
    q.cmd("input-send-event", events=[
        {"type": "abs", "data": {"axis": "x", "value": int((x + 0.5) * 0x8000 / w)}},
        {"type": "abs", "data": {"axis": "y", "value": int((y + 0.5) * 0x8000 / h)}},
    ])

def button(down):
    q.cmd("input-send-event", events=[{"type": "btn", "data": {"down": down, "button": "left"}}])

def keys(*names):
    q.cmd("send-key", keys=[{"type": "qcode", "data": n} for n in names])

# The window opens already showing the list, first row (the parent folder) selected.
first, _ = wait_for("before", SELECTED, "the timeline window shows the list")
w, h, _ = first

# A window that opens is raised but not focused (wm.rs, ADR 0013): click its title bar first,
# the way the compositor test does, so the keys can reach it. The window is NOT at (16,16)
# here — the demo window is the first window, so the timeline opens below it — so find it
# from the selected row's amber band instead: the band is the first row, its left edge is the
# window's left edge, and the title bar is TITLE_H (40px) above it.
def band_top(screen):
    sw, sh, px = screen
    for y in range(sh):
        for x in range(sw):
            if px[y * sw + x] == SELECTED:
                return x, y
    return None

# Focus + dry-run Enter, RETRYING until the confirm band appears. The single-shot click
# was flaky: the timeline opens below the demo window, so a colour-derived title-bar click
# can land on the wrong surface and the Enter goes to the demo window. Re-shoot each attempt
# and retry the whole focus+Enter sequence instead of trusting one coordinate guess.
confirm_shown = False
for _attempt in range(6):
    _scr = shot("focus")
    _b = band_top(_scr)
    if _b:
        bx, by = _b
        tablet(bx + 40, by - 20, w, h); time.sleep(0.6)
        button(True); time.sleep(0.1); button(False); time.sleep(0.4)
        keys("ret")
    _deadline = time.time() + 6
    while time.time() < _deadline:
        if count(shot("confirm"), CONFIRM) > 1000:
            confirm_shown = True
            break
        time.sleep(0.5)
    if confirm_shown:
        break
check("the dry run's plan is shown to confirm", confirm_shown)

# Confirm: the second Enter runs the real rewind. Record the keypress so the disk verdict can
# prove the quarantine was made after it (the shell did it, not a script).
confirm_at = time.time()
open(f"{work}/confirm-at", "w").write(f"{confirm_at}\n")
keys("ret")

# The rewind runs off the draw loop (H17); wait for its green result band.
wait_for("after", OK, "the rewind finished (green result band)")

sys.exit(0 if ok else 1)
PY
fi

# Wait for the VM to power off (the self-test does it once the rewind is done). The decision
# line says WHEN to look; the disk below is still the verdict (H8, H13).
wait "$vm" 2>/dev/null || true
echo "    vm time: $(( $(date +%s) - start ))s"

# The shared verdict, read from the disk.
data_partition
verify_ransomware_docs

# The self-test reached its last step.
step=$(esp morph-test/step | tr -d '\r\n' || true)
check "self-test finished (step 2, got '${step}')" '[ "$step" = 2 ]'

# The boot was committed and healthy, with the shell and Strata running.
boots=$(esp morph-boot/history.jsonl |
    sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p' | head -1)
check "the boot was healthy and committed (got: ${boots})" '[ "$boots" = "a/committed/none" ]'

# The quarantine folder's name is the moment the rewind ran (Strata's time::format, UTC). It
# must be later than the confirming keypress, or a script rewound before the person confirmed.
# Python, not `date -d`, so the UTC stamp is parsed as UTC on any host timezone.
qdir=$(find "$out/quarantine" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' -quit 2>/dev/null || true)
confirm_at=$(cat "$WORK/confirm-at" 2>/dev/null || echo "")
if python3 - "$qdir" "$confirm_at" <<'PY'
import sys, datetime
qdir, confirm = sys.argv[1], sys.argv[2].strip()
if not qdir or not confirm:
    sys.exit(1)
q = datetime.datetime.strptime(qdir, "%Y-%m-%dT%H:%M:%S.%fZ").replace(tzinfo=datetime.timezone.utc).timestamp()
# Allow 2 s of QEMU RTC skew against the host clock: the point is that the rewind ran at the
# confirm (a script that rewound during boot is tens of seconds earlier), not sub-second.
sys.exit(0 if q + 2 > float(confirm) else 1)
PY
then
    echo "    ok    the quarantine is newer than the confirming keypress"
else
    echo "    FAIL  the quarantine is newer than the confirming keypress (quarantine '$qdir', confirm '$confirm_at')"
    fail=1
fi

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $log and $WORK/screen-rewind-*.ppm"; fi
exit $fail
