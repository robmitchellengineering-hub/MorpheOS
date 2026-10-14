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
VM_GPU=1 VM_QMP="$qmp" "$HERE/run-vm.sh" "$disk" "$log" "${VM_TIMEOUT:-900}" &
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
SELECTED, FIELD = (0x2a, 0x5c, 0xaa), (0x0e, 0x11, 0x16)
FAIL, OK = (0xc0, 0x32, 0x32), (0x1e, 0xa0, 0x5a)
WINDOW = (16, 16, 480, 360)      # where the window manager puts the first window
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

def said(pattern):
    return pattern in open(log_path, "rb").read().decode("utf-8", "replace")

ok = True
def check(name, cond):
    global ok
    print(f"    {'ok  ' if cond else 'FAIL'}  {name}")
    ok = ok and cond
    return cond

def wait_for(pattern, seconds):
    end = time.time() + seconds
    while not said(pattern):
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

QCODE = {" ": "spc"}
def type_text(s):
    for c in s:
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
