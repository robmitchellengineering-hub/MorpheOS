#!/bin/bash
# A browser on the Flow shell (doc 20, E4): boot, and read the page off the screen.
#
#   sudo images/x86-qemu/test-browser.sh <workdir>
#
# Boots the `browser` image: the shell, with Firefox (Mozilla's own build) opening a local
# test page of two large blocks of colour nothing else on the screen uses. The verdict is the
# SCREENSHOT (AGENTS.md H8), not a log line:
#   * both of the page's colours cover a large area, so Firefox rendered the page;
#   * the page lies inside the screen, a margin from its edges, so the window was made to fit;
#   * the boot was healthy and committed with Firefox running.
set -euo pipefail

WORK=${1:?usage: test-browser.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$WORK"

echo "=== browser: Firefox on the Flow shell, a local page read off the screen"
disk=$("$HERE/build.sh" "$WORK" browser | tail -1)
rm -f "${disk%.img}.vars.fd"
log="$WORK/serial-browser.log"
qmp="$WORK/qmp-browser.sock"
rm -f "$log" "$qmp" "$WORK"/screen-browser*.ppm

start=$(date +%s)
VM_GPU=1 VM_QMP="$qmp" VM_MEM=2048 "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-900}" &
vm=$!

# When to look: the compositor reports a window on screen (Firefox's). The pixels decide.
ready=""
for _ in $(seq 1 600); do
    if grep -aq 'flow-compositor: [1-9][0-9]* window(s) on screen' "$log" 2>/dev/null; then ready=1; break; fi
    if ! kill -0 "$vm" 2>/dev/null; then break; fi
    sleep 1
done

fail=0
if [ -z "$ready" ]; then
    echo "    FAIL: no window ever reached the screen"
    grep -a 'flow-compositor\|flow-browser\|firefox\|Firefox' "$log" | tail -25 || true
    fail=1
else
    python3 - "$qmp" "$WORK" <<'PY' || fail=1
import json, socket, sys, time

qmp_path, work = sys.argv[1], sys.argv[2]
PAGE_A, PAGE_B = (0xc8, 0x1e, 0x78), (0x1e, 0xc8, 0x78)
MARGIN = 16

class Qmp:
    def __init__(self, path):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.connect(path)
        self.f = self.sock.makefile("rwb")
        self.f.readline()
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

def shot(q, path):
    q.cmd("screendump", filename=path)
    data = open(path, "rb").read()
    fields, i = [], 0
    while len(fields) < 4:
        while data[i:i+1].isspace(): i += 1
        j = i
        while not data[j:j+1].isspace(): j += 1
        fields.append(data[i:j]); i = j
    w, h = int(fields[1]), int(fields[2])
    px = data[i+1:]
    return w, h, [tuple(px[k:k+3]) for k in range(0, w * h * 3, 3)]

def near(p, c, tol=3):
    return all(abs(a - b) <= tol for a, b in zip(p, c))

def region(w, px, colour):
    hits = [(k % w, k // w) for k, p in enumerate(px) if near(p, colour)]
    if not hits:
        return None, 0
    xs, ys = [x for x, _ in hits], [y for _, y in hits]
    return (min(xs), min(ys), max(xs), max(ys)), len(hits)

q = Qmp(qmp_path)
# Firefox takes a while to draw its first page (longer still without KVM): look until the
# page is there or two minutes have gone.
end, n, verdict = time.time() + 120, 0, None
while True:
    n += 1
    w, h, px = shot(q, f"{work}/screen-browser.ppm")
    (box_a, n_a), (box_b, n_b) = region(w, px, PAGE_A), region(w, px, PAGE_B)
    big = 20000
    if (n_a >= big and n_b >= big) or time.time() > end:
        verdict = (w, h, box_a, n_a, box_b, n_b)
        break
    time.sleep(2)
w, h, box_a, n_a, box_b, n_b = verdict
ok = True
def check(name, cond):
    global ok
    print(f"    {'ok  ' if cond else 'FAIL'}  {name}")
    ok = ok and cond
print(f"    screen {w}x{h}, {n} screenshot(s)")
check(f"the page's first colour covers a large area ({n_a} px)", n_a >= 20000)
check(f"the page's second colour covers a large area ({n_b} px)", n_b >= 20000)
if box_a and box_b:
    x0, y0 = min(box_a[0], box_b[0]), min(box_a[1], box_b[1])
    x1, y1 = max(box_a[2], box_b[2]), max(box_a[3], box_b[3])
    check(f"the page is inside the screen, a margin from its edges (page at {x0},{y0} to {x1},{y1})",
          x0 >= MARGIN and y0 >= MARGIN and x1 <= w - MARGIN - 1 and y1 <= h - MARGIN - 1)
    check("the first colour is above the second, as the page lays them out", box_a[3] < box_b[1] + 2)
sys.exit(0 if ok else 1)
PY
fi
# Wait for the health check's decision before stopping the VM (AGENTS.md H13).
for _ in $(seq 1 300); do
    grep -aq 'morph-boot-check: decision' "$log" 2>/dev/null && break
    kill -0 "$vm" 2>/dev/null || break
    sleep 1
done
sleep 3
python3 - "$qmp" <<'PY' 2>/dev/null || true
import json, socket, sys
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f = s.makefile("rwb"); f.readline()
for c in ("qmp_capabilities", "quit"):
    f.write((json.dumps({"execute": c}) + "\n").encode()); f.flush(); f.readline()
PY
wait "$vm" 2>/dev/null || true
echo "    vm time: $(( $(date +%s) - start ))s"

boots=$({ mtype -i "$disk@@1M" ::/morph-boot/history.jsonl 2>/dev/null || true; } |
    sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p' | head -1)
if [ "$boots" = "a/committed/none" ]; then
    echo "    ok    the boot was healthy and committed with Firefox running"
else
    echo "    FAIL  the boot was not a healthy committed slot A (got '$boots')"
    grep -a 'failed unit' "$log" | sed 's/^/      /' || true
    fail=1
fi

if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $log and $WORK/screen-browser.ppm"; fi
exit $fail
