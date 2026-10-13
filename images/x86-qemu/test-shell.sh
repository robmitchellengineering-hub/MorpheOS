#!/bin/bash
# The Flow shell on a real display (doc 20, E3): boot, and look at the screen.
#
#   sudo images/x86-qemu/test-shell.sh <workdir>
#
# Boots the `shell` image with a virtio GPU. The compositor takes the display through DRM
# and a demonstration client opens a window. When the compositor reports the window on
# screen, the harness takes a screenshot through the QEMU monitor (`screendump`) and judges
# the PIXELS, not a log line the image chose to print (AGENTS.md H8):
#   * the shell's background covers most of the screen, so it is the compositor drawing,
#     not a text console;
#   * the demonstration window is there, exactly where the window manager puts it, at its
#     exact size;
#   * the boot was healthy and committed (the shell running did not fail the health check).
set -euo pipefail

WORK=${1:?usage: test-shell.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$WORK"

echo "=== shell: the compositor on a virtio-gpu display"
disk=$("$HERE/build.sh" "$WORK" shell | tail -1)
rm -f "${disk%.img}.vars.fd"
log="$WORK/serial-shell.log"
mon="$WORK/monitor-shell.sock"
shot="$WORK/screen-shell.ppm"
rm -f "$log" "$mon" "$shot"

start=$(date +%s)
VM_GPU=1 VM_MONITOR="$mon" "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-900}" &
vm=$!

monitor() {
    python3 - "$mon" "$1" <<'PY'
import socket, sys, time
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[1])
s.sendall((sys.argv[2] + "\n").encode())
time.sleep(1.5)
s.close()
PY
}

fail=0
seen=""
for _ in $(seq 1 600); do
    if grep -aq 'flow-compositor: 1 window(s) on screen' "$log" 2>/dev/null; then seen=1; break; fi
    if ! kill -0 "$vm" 2>/dev/null; then break; fi
    sleep 1
done
if [ -z "$seen" ]; then
    echo "    FAIL: the compositor never reported the window on screen"
    grep -a 'flow-compositor\|flow-shell\|solid-window' "$log" | tail -20 || true
    fail=1
else
    sleep 3 # a few frames after the window arrived
    monitor "screendump $shot"
fi
# ⚠️ THE WINDOW CAN BEAT THE HEALTH CHECK. With KVM (CI) the window is on screen before
# morph-boot-check has decided and written the boot history; pulling the plug then left no
# history, and the read below failed the run without a word. Wait for the decision (a
# signal of WHEN only; the history on disk is still the verdict). boot-check writes and
# fsyncs the history right after it logs the decision, so a moment later it is on disk.
for _ in $(seq 1 300); do
    grep -aq 'morph-boot-check: decision' "$log" 2>/dev/null && break
    kill -0 "$vm" 2>/dev/null || break
    sleep 1
done
sleep 3
monitor "quit" 2>/dev/null || true
wait "$vm" 2>/dev/null || true
echo "    vm time: $(( $(date +%s) - start ))s"

if [ -z "$seen" ]; then
    echo "    see $log"
    exit 1
fi

# Judge the screenshot.
python3 - "$shot" <<'PY' || fail=1
import sys
data = open(sys.argv[1], "rb").read()
# P6 header: magic, width, height, maxval, each followed by whitespace.
fields, i = [], 0
while len(fields) < 4:
    while data[i:i+1].isspace(): i += 1
    j = i
    while not data[j:j+1].isspace(): j += 1
    fields.append(data[i:j]); i = j
w, h = int(fields[1]), int(fields[2])
px = data[i+1:]
def at(x, y):
    o = (y * w + x) * 3
    return tuple(px[o:o+3])
bg, win = (0x10, 0x12, 0x16), (0x00, 0xa0, 0xff)
total = w * h
n_bg = sum(1 for k in range(0, total * 3, 3) if tuple(px[k:k+3]) == bg)
hits = [(k // 3 % w, k // 3 // w) for k in range(0, total * 3, 3) if tuple(px[k:k+3]) == win]
ok = True
def check(name, cond):
    global ok
    print(f"    {'ok  ' if cond else 'FAIL'}  {name}")
    ok = ok and cond
print(f"    screen {w}x{h}")
check(f"the shell's background covers most of the screen ({n_bg}/{total})", n_bg > total // 2)
if hits:
    xs, ys = [p[0] for p in hits], [p[1] for p in hits]
    box = (min(xs), min(ys), max(xs) - min(xs) + 1, max(ys) - min(ys) + 1)
else:
    box = None
check(f"the window is at (16, 16), 320x200, solid (got {box}, {len(hits)} px)",
      box == (16, 16, 320, 200) and len(hits) == 320 * 200)
sys.exit(0 if ok else 1)
PY

# Guarded: a missing history must be a reported FAIL, not a silent exit (AGENTS.md H11).
boots=$({ mtype -i "$disk@@1M" ::/morph-boot/history.jsonl 2>/dev/null || true; } |
    sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p' | head -1)
if [ "$boots" = "a/committed/none" ]; then
    echo "    ok    the boot was healthy and committed with the shell running"
else
    echo "    FAIL  the boot was not a healthy committed slot A (got '$boots')"
    fail=1
fi

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $log and $shot"; fi
exit $fail
