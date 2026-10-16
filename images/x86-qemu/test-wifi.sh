#!/bin/bash
# Joining Wi-Fi with the picker (doc 20, E5): NetworkManager, a real WPA2 handshake, a typed
# password, and DHCP, in a VM.
#
#   sudo images/x86-qemu/test-wifi.sh <workdir>
#
# Boots the `wifi` image: the Flow shell, NetworkManager, and no wired network. The kernel's
# simulated radios (mac80211_hwsim) carry a WPA2 network, `MorpheOS-test`, served from inside
# the VM (hostapd and dnsmasq on wlan1); the machine's Wi-Fi is wlan0. With no network, the
# Wi-Fi picker opens by itself. The harness drives the VM's own keyboard and mouse through
# QMP, as a person would:
#   * the picker lists MorpheOS-test, selected, on the screen;
#   * Enter, a WRONG password, Enter: NetworkManager's handshake fails, the picker says it was
#     the password, and its status line turns red;
#   * Enter, the right password, Enter: the picker says it joined, its status line turns
#     green, and the network's own DHCP server logs the lease it gave (evidence from the
#     other end, not from the picker);
#   * Esc closes the picker, and the bar's Wi-Fi button opens it again: it lists the network
#     the machine is now on (doc 20, E5: the picker is reachable once the machine is online);
#   * Firefox (E14) then navigates to http://192.168.77.1/, a page the access point itself
#     serves, and its colour appears on screen — with no reload step anywhere: the first
#     navigation after the picker joins must just work, which is the bug the user.js fix
#     closes (there is no F5 / Ctrl+R in this sequence, so the page appearing is the proof);
#   * the boot was healthy and committed.
# The screenshots are the verdict on what a person sees (AGENTS.md H8); the log lines say
# when to look, and what NetworkManager and the network did.
set -euo pipefail

WORK=${1:?usage: test-wifi.sh <workdir>}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$WORK"

echo "=== wifi: the picker joins a simulated WPA2 network through NetworkManager"
disk=$("$HERE/build.sh" "$WORK" wifi | tail -1)
rm -f "${disk%.img}.vars.fd"
log="$WORK/serial-wifi.log"
qmp="$WORK/qmp-wifi.sock"
rm -f "$log" "$qmp" "$WORK"/screen-wifi-*.ppm

start=$(date +%s)
VM_GPU=1 VM_QMP="$qmp" VM_MEM=2048 "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-900}" &
vm=$!

# When to look: the picker has listed the test network.
ready=""
for _ in $(seq 1 600); do
    if grep -aq 'flow-wifi: networks .*MorpheOS-test' "$log" 2>/dev/null; then ready=1; break; fi
    if ! kill -0 "$vm" 2>/dev/null; then break; fi
    sleep 1
done

fail=0
if [ -z "$ready" ]; then
    echo "    FAIL: the picker never listed MorpheOS-test"
    fail=1
else
    sleep 2 # a few frames after the list arrived
    python3 - "$qmp" "$WORK" "$log" <<'PY' || fail=1
import json, socket, sys, time

qmp_path, work, log_path = sys.argv[1], sys.argv[2], sys.argv[3]
# The picker's colours (crates/flow-wifi/src/picker.rs, `colours`).
SELECTED, FIELD = (0x0e, 0x6b, 0x34), (0x05, 0x07, 0x0d)
FAIL, OK = (0xd1, 0x3a, 0x4b), (0x00, 0xff, 0x41)
# Firefox opens first and fills the usable screen; the picker then opens over it, cascaded to
# MARGIN + CASCADE (16 + 32) so it stays on screen (crates/flow-compositor/src/wm.rs). Its
# window is still 480 x 360, but it no longer sits at (16, 16) — Firefox has that spot.
WINDOW = (48, 48, 480, 360)
ROW = 480 * 36                   # one row or status line across the window, in pixels
WRONG, RIGHT = "wrong pass", "correct horse"

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

q = Qmp(qmp_path)

def shot(name):
    path = f"{work}/screen-wifi-{name}.ppm"
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

def count(screen, rgb):
    return sum(1 for p in screen[2] if p == rgb)

def said(pattern, times=1):
    return open(log_path, "rb").read().decode("utf-8", "replace").count(pattern) >= times

ok = True
def check(name, cond):
    global ok
    print(f"    {'ok  ' if cond else 'FAIL'}  {name}")
    ok = ok and cond
    return cond

def wait_for(pattern, seconds, times=1):
    end = time.time() + seconds
    while not said(pattern, times):
        if time.time() > end:
            return False
        time.sleep(1)
    return True

def until(name, rgb, least, seconds=20):
    end = time.time() + seconds
    while True:
        n = count(shot(name), rgb)
        if n >= least or time.time() > end:
            return check(f"{name}: #{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x} covers {n} px (at least {least})", n >= least)
        time.sleep(1)

def tablet(x, y, w, h):
    q.cmd("input-send-event", events=[
        {"type": "abs", "data": {"axis": "x", "value": int((x + 0.5) * 0x8000 / w)}},
        {"type": "abs", "data": {"axis": "y", "value": int((y + 0.5) * 0x8000 / h)}},
    ])

def click(x, y):
    w, h, _ = shot("before-click")
    tablet(x, y, w, h)
    time.sleep(0.3)
    for down in (True, False):
        q.cmd("input-send-event", events=[{"type": "btn", "data": {"down": down, "button": "left"}}])
        time.sleep(0.1)

# QEMU qcode names: letters and digits pass through; space and the URL's punctuation need a
# name (QKeyCode has no "/" or "." strings, and no ":" at all — ":" is Shift+Semicolon).
QCODE = {" ": "spc", "/": "slash", ".": "dot"}
def type_text(s):
    for c in s:
        if c == ":":
            q.cmd("send-key", keys=[{"type": "qcode", "data": "shift"},
                                     {"type": "qcode", "data": "semicolon"}])
        else:
            q.cmd("send-key", keys=[{"type": "qcode", "data": QCODE.get(c, c)}])
        time.sleep(0.05)

def key(name):
    q.cmd("send-key", keys=[{"type": "qcode", "data": name}])
    time.sleep(0.3)

# The list, with the network selected.
until("list", SELECTED, ROW * 8 // 10)
# A click on the picker's title gives it the keyboard.
click(WINDOW[0] + 40, WINDOW[1] + 15)

# A wrong password: NetworkManager's handshake fails, and the picker says why.
key("ret")
until("password", FIELD, 400 * 30)
type_text(WRONG)
key("ret")
check("a wrong password was refused as one (flow-wifi: failed MorpheOS-test wrong-password)",
      wait_for("flow-wifi: failed MorpheOS-test wrong-password", 120))
until("wrong", FAIL, ROW * 8 // 10)

# The right one: joined, and the network's DHCP server gave the machine an address.
key("ret")
until("password-again", FIELD, 400 * 30)
type_text(RIGHT)
key("ret")
check("the right password joined (flow-wifi: connected MorpheOS-test 192.168.77.x)",
      wait_for("flow-wifi: connected MorpheOS-test 192.168.77.", 120))
check("the network's DHCP server leased the machine an address (dnsmasq DHCPACK)",
      wait_for("DHCPACK(wlan1) 192.168.77.", 10))
until("joined", OK, ROW * 8 // 10)

# Online now, so nothing opens the picker by itself any more: Esc closes it, and the bar's
# Wi-Fi button (the 24 x 24 square at the bar's right end, wm::wifi_button) opens it again.
TITLE = (0x18, 0x21, 0x2e)
def networks_lines():
    text = open(log_path, "rb").read().decode("utf-8", "replace")
    return [l for l in text.splitlines() if "flow-wifi: networks " in l]
key("esc")
check("Esc closed the picker (flow-wifi: closed)", wait_for("flow-wifi: closed", 10))

# E14: the picker is gone, Firefox is the only window, and NetworkManager has a connection.
# Navigate Firefox to the page the access point serves and assert its colour is on screen.
# There is NO reload step anywhere in this sequence (no F5 / Ctrl+R): the first navigation
# after the picker joins must just work, which is the bug the firefox-user.js pref closes.
click(300, 31)  # Firefox's title bar (above where the picker sat), to give it the keyboard
q.cmd("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "l"}])
time.sleep(0.5)
type_text("http://192.168.77.1/")
key("ret")
CYAN = (0x00, 0xff, 0xff)  # overlay-wifi-test/.../wifi-test/index.html; nothing else uses it
until("online-page", CYAN, 100000, 90)
check("Firefox fetched the page over the network (python http.server GET /)",
      wait_for("GET / HTTP/1.1", 30))

before = len(networks_lines())
w, h, _ = shot("closed")
click(w - 8 - 12, h - 32 + 4 + 12)
check("the bar's Wi-Fi button opened the picker (flow-compositor: opening the Wi-Fi picker)",
      wait_for("flow-compositor: opening the Wi-Fi picker", 20))
end = time.time() + 60
while len(networks_lines()) <= before and time.time() < end:
    time.sleep(1)
new = networks_lines()[before:]
check(f"the picker it opened lists the network the machine is on ({new[:1]})",
      any("MorpheOS-test" in l for l in new))
until("reopened", TITLE, 480 * 40 * 8 // 10)
until("reopened-list", SELECTED, ROW * 8 // 10)
text = open(log_path, "rb").read().decode("utf-8", "replace")
check("neither password appears on the console", WRONG not in text.replace("wrong-password", "") and RIGHT not in text)
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
    echo "    ok    the boot was healthy and committed"
else
    echo "    FAIL  the boot was not a healthy committed slot A (got '$boots')"
    grep -a 'failed unit' "$log" | sed 's/^/      /' || true
    fail=1
fi

if [ $fail != 0 ]; then
    echo "    what the picker, NetworkManager and the access point said (serial console):"
    grep -a -iE 'flow-wifi|NetworkManager|wpa_supplicant|hostapd|dnsmasq|hwsim|wlan' "$log" |
        grep -v 'libinput error' | tail -50 | sed 's/^/      /' || true
fi
if [ $fail = 0 ]; then echo "    PASS"; else echo "    see $log and $WORK/screen-wifi-*.ppm"; fi
exit $fail
