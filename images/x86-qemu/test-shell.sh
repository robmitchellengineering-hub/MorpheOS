#!/bin/bash
# The Flow shell on a real display, with a real keyboard and mouse (doc 20, E3 and E10).
#
#   sudo images/x86-qemu/test-shell.sh <workdir>
#
# Boots the `shell` image with a virtio GPU, the machine's PS/2 keyboard and a USB tablet.
# The compositor takes the seat through seatd, and a demonstration client opens a window
# that changes colour when it is clicked and when a key reaches it. The harness drives the
# VM's own input devices through QMP and judges SCREENSHOTS, not a log line the image chose
# to print (AGENTS.md H8):
#   E3   the shell's background covers most of the screen (the compositor is drawing, not a
#        text console), and the window is exactly where the window manager puts it;
#   E10  a click on the window reaches it (it turns purple) and the pointer is drawn where
#        the mouse went; a key then reaches it (green); Ctrl+Alt+F2 leaves the shell for the
#        root console on VT2, and Ctrl+Alt+F1 brings the shell back, window and all;
#   and the boot was healthy and committed (the shell running did not fail the health check).
set -euo pipefail

WORK=${1:?usage: test-shell.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$WORK"

echo "=== shell: the compositor on a virtio-gpu display, with a keyboard and a mouse"
disk=$("$HERE/build.sh" "$WORK" shell | tail -1)
rm -f "${disk%.img}.vars.fd"
log="$WORK/serial-shell.log"
qmp="$WORK/qmp-shell.sock"
rm -f "$log" "$qmp" "$WORK"/screen-shell-*.ppm

start=$(date +%s)
VM_GPU=1 VM_QMP="$qmp" "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-900}" &
vm=$!

# Wait for the window, and for libinput to have found both devices. These lines only say
# WHEN to look; what is on the screen is the verdict.
ready=""
for _ in $(seq 1 600); do
    if grep -aq 'flow-compositor: 1 window(s) on screen' "$log" 2>/dev/null &&
        grep -aq 'flow-compositor: input: .*keyboard' "$log" 2>/dev/null &&
        grep -aq 'flow-compositor: input: .*pointer' "$log" 2>/dev/null; then
        ready=1
        break
    fi
    if ! kill -0 "$vm" 2>/dev/null; then break; fi
    sleep 1
done

fail=0
if [ -z "$ready" ]; then
    echo "    FAIL: the compositor never reported the window and both input devices"
    grep -a 'flow-compositor\|flow-shell\|solid-window\|seatd' "$log" | tail -20 || true
    fail=1
else
    sleep 2 # a few frames after the window arrived
    python3 - "$qmp" "$WORK" <<'PY' || fail=1
import json, socket, sys, time

qmp_path, work = sys.argv[1], sys.argv[2]
BG, IDLE, CLICKED, KEYED = (0x10, 0x12, 0x16), (0x00, 0xa0, 0xff), (0xc0, 0x00, 0xc0), (0x00, 0xc0, 0x60)
POINTER = (0xd8, 0xde, 0xe9)        # the pointer's fill (palette::TEXT)
WINDOW = (16, 16, 320, 200)         # where the window manager puts the first window

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
shots = 0

def shot(name):
    global shots
    shots += 1
    path = f"{work}/screen-shell-{name}.ppm"
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

def box(screen, rgb):
    w, _, px = screen
    hits = [(k % w, k // w) for k, p in enumerate(px) if p == rgb]
    if not hits:
        return None, 0
    xs, ys = [p[0] for p in hits], [p[1] for p in hits]
    return (min(xs), min(ys), max(xs) - min(xs) + 1, max(ys) - min(ys) + 1), len(hits)

def count(screen, rgb):
    return sum(1 for p in screen[2] if p == rgb)

ok = True
def check(name, cond):
    global ok
    print(f"    {'ok  ' if cond else 'FAIL'}  {name}")
    ok = ok and cond
    return cond

def until(name, judge, seconds=20):
    """Take screenshots until `judge` passes or time runs out; report the last one."""
    end = time.time() + seconds
    while True:
        screen = shot(name)
        good, detail = judge(screen)
        if good or time.time() > end:
            return check(f"{name}: {detail}", good)
        time.sleep(0.5)

def window_is(rgb, label):
    def judge(screen):
        b, n = box(screen, rgb)
        return b == WINDOW and n == WINDOW[2] * WINDOW[3], f"the window is {label} at {WINDOW[:2]}, {WINDOW[2]}x{WINDOW[3]} (got {b}, {n} px)"
    return judge

def tablet(x, y, w, h):
    """Move the USB tablet to a pixel. QEMU's absolute axes run 0..0x7fff."""
    q.cmd("input-send-event", events=[
        {"type": "abs", "data": {"axis": "x", "value": int((x + 0.5) * 0x8000 / w)}},
        {"type": "abs", "data": {"axis": "y", "value": int((y + 0.5) * 0x8000 / h)}},
    ])

def button(down):
    q.cmd("input-send-event", events=[{"type": "btn", "data": {"down": down, "button": "left"}}])

def keys(*names):
    q.cmd("send-key", keys=[{"type": "qcode", "data": n} for n in names])

# E3: the shell is drawing, the window is in place, and nothing has moved the pointer.
first = shot("start")
w, h, _ = first
total = w * h
print(f"    screen {w}x{h}")
check(f"the shell's background covers most of the screen ({count(first, BG)}/{total})", count(first, BG) > total // 2)
window_ok = window_is(IDLE, "blue")(first)
check(window_ok[1], window_ok[0])
check("no pointer drawn before the mouse moved", count(first, POINTER) == 0)

# E10: click the window, then move the mouse away so the pointer does not cover it.
tablet(WINDOW[0] + WINDOW[2] // 2, WINDOW[1] + WINDOW[3] // 2, w, h)
time.sleep(0.3)
button(True); time.sleep(0.1); button(False)
time.sleep(0.3)
px, py = 900, 600
tablet(px, py, w, h)
until("click", window_is(CLICKED, "purple (it got the click)"))
def pointer_drawn(screen):
    b, n = box(screen, POINTER)
    good = b is not None and b[2:] == (10, 10) and abs(b[0] - (px + 1)) <= 3 and abs(b[1] - (py + 1)) <= 3
    return good, f"the pointer is drawn where the mouse went, near ({px}, {py}) (got {b})"
until("pointer", pointer_drawn, 5)

# The click gave the window the keyboard: type `a`.
keys("a")
until("key", window_is(KEYED, "green (it got the key)"))

# The way out: Ctrl+Alt+F2 shows the root console instead of the shell.
keys("ctrl", "alt", "f2")
def console(screen):
    n = count(screen, BG)
    return n < total // 10 and count(screen, KEYED) == 0, f"Ctrl+Alt+F2 left the shell for the console (shell background on {n}/{total} px)"
until("vt2", console)

# And back: on the console the kernel handles Ctrl+Alt+F1, the seat gives the shell its
# screen back, and the picture returns as it was.
keys("ctrl", "alt", "f1")
def back(screen):
    b, n = box(screen, KEYED)
    good = count(screen, BG) > total // 2 and b == WINDOW and n == WINDOW[2] * WINDOW[3]
    return good, f"Ctrl+Alt+F1 brought the shell back, window and all (window {b}, {n} px)"
until("vt1", back)

sys.exit(0 if ok else 1)
PY
fi
python3 - "$qmp" <<'PY' 2>/dev/null || true
import json, socket, sys
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f = s.makefile("rwb"); f.readline()
for c in ("qmp_capabilities", "quit"):
    f.write((json.dumps({"execute": c}) + "\n").encode()); f.flush(); f.readline()
PY
wait "$vm" 2>/dev/null || true
echo "    vm time: $(( $(date +%s) - start ))s"

boots=$(mtype -i "$disk@@1M" ::/morph-boot/history.jsonl 2>/dev/null |
    sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p' | head -1)
if [ "$boots" = "a/committed/none" ]; then
    echo "    ok    the boot was healthy and committed with the shell running"
else
    echo "    FAIL  the boot was not a healthy committed slot A (got '$boots')"
    fail=1
fi

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $log and $WORK/screen-shell-*.ppm"; fi
exit $fail
