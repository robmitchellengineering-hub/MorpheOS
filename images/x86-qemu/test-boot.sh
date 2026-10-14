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
# The verdict comes from three places, none of which is a log line the image chose to print:
#   - the screen (doc 20, E4): the shell, Firefox on its start page, and (the VM has no
#     network and no Wi-Fi device) the Wi-Fi picker in front, saying so in red;
#   - the boot history on the ESP, read off the disk after the VM is gone;
#   - the serial console, for the login prompt the getty produces.
set -euo pipefail

WORK=${1:?usage: test-boot.sh <workdir> [seconds]}
SECONDS_TO_RUN=${2:-300}

HERE=$(cd "$(dirname "$0")" && pwd)
DISK="$WORK/disk-plain.img"
[ -f "$DISK" ] || { echo "no $DISK: run build.sh $WORK plain first" >&2; exit 1; }

LOG="$WORK/serial-plain.log"
QMP="$WORK/qmp-plain.sock"
rm -f "$LOG" "$QMP" "$WORK"/screen-plain*.ppm
echo "=== plain: booting for ${SECONDS_TO_RUN}s, expecting the shell, a login prompt and no poweroff"
start=$(date +%s)
# A non-zero exit here is the EXPECTED outcome: `run-vm.sh` kills QEMU when its timeout
# expires, which is exactly what "the machine stayed up" looks like from outside.
VM_GPU=1 VM_QMP="$QMP" VM_MEM=2048 "$HERE/run-vm.sh" "$DISK" "$LOG" "$SECONDS_TO_RUN" &
vm=$!

fail=0
# What a person sees. The log line only says WHEN to look (the picker has found there is no
# Wi-Fi device); the pixels are the verdict.
for _ in $(seq 1 "$SECONDS_TO_RUN"); do
    grep -aq 'flow-wifi: unavailable' "$LOG" 2>/dev/null && break
    kill -0 "$vm" 2>/dev/null || break
    sleep 1
done
python3 - "$QMP" "$WORK" <<'PY' || fail=1
import json, socket, sys, time

qmp_path, work = sys.argv[1], sys.argv[2]
BG = (0x10, 0x12, 0x16)               # the shell's background (flow-shell's palette)
START = (0x5a, 0x3c, 0xb4)            # the start page (overlay-browser/.../start.html)
TITLE = (0x26, 0x2c, 0x38)            # the Wi-Fi picker's title bar (flow-wifi colours::TITLE)
FAIL = (0xc0, 0x32, 0x32)             # its status line in red (colours::FAIL)

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

def count(px, colour):
    return sum(1 for p in px if near(p, colour))

q = Qmp(qmp_path)
# Look until all of it is there, or two minutes have gone (Firefox is slow without KVM).
end = time.time() + 120
while True:
    w, h, px = shot(q, f"{work}/screen-plain.ppm")
    n = {k: count(px, c) for k, c in (("bg", BG), ("start", START), ("title", TITLE), ("fail", FAIL))}
    done = (n["start"] >= w * h // 4 and n["title"] >= 480 * 40 * 8 // 10
            and n["fail"] >= 480 * 36 * 8 // 10)
    if done or time.time() > end:
        break
    time.sleep(2)
ok = True
def check(name, cond):
    global ok
    print(f"    {'ok  ' if cond else 'FAIL'}  {name}")
    ok = ok and cond
print(f"    screen {w}x{h}")
check(f"the shell's background shows round the windows ({n['bg']} px)", n["bg"] >= 20000)
check(f"Firefox shows its start page over a large area ({n['start']} px)", n["start"] >= w * h // 4)
check(f"the Wi-Fi picker is on the screen, in front ({n['title']} px of its title bar)",
      n["title"] >= 480 * 40 * 8 // 10)
check(f"it says, in red, that there is no Wi-Fi ({n['fail']} px of its status line)",
      n["fail"] >= 480 * 36 * 8 // 10)
sys.exit(0 if ok else 1)
PY
set +e
wait "$vm"
code=$?
set -e
echo "    vm time: $(( $(date +%s) - start ))s (exit $code — 124 means the timeout took it, which is the pass)"

if [ "$code" != 124 ]; then
    echo "    FAIL: the VM exited $code instead of still running — an image for a person must not shut itself down"
    fail=1
fi

# ⚠️ A GETTY PRINTING A BANNER IS NOT A PERSON BEING ABLE TO USE THE MACHINE. The first version
# of this check accepted `morpheos-a login:` — which is exactly what it looks like when a login
# prompt is waiting for a password that this image never sets. The marker below is printed by
# /root/.profile, so it appears only if a login actually COMPLETED and started a shell.
if grep -aq 'MORPHEOS CONSOLE READY' "$LOG"; then
    echo "    console: a login completed with nothing typed — $(grep -ao 'MORPHEOS CONSOLE READY.*' "$LOG" | tail -1)"
else
    echo "    FAIL: no completed login on the console. A prompt nobody can answer is not a usable image:"
    grep -aoE '[a-z0-9-]* login:|MORPHEOS CONSOLE READY' "$LOG" | tail -3 | sed 's/^/      /'
    tail -5 "$LOG" | sed 's/^/      /'
    fail=1
fi

# And the boot decision, from the disk rather than the console: slot A is committed, and the
# history has exactly one line, which is what "no trial, no fallback" looks like.
# The same parse the other scenarios use — the file holds one JSON record per boot, not the
# `<slot>/<phase>/<action>` summary this prints. Reading it as if it were already summarised was
# the second bug this check found in itself.
history=$(mtype -i "$DISK@@1M" ::/morph-boot/history.jsonl 2>/dev/null |
    sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p')
echo "    boots:   $(echo "$history" | tr '\n' ' ')"
if [ -z "$history" ]; then
    echo "    FAIL: the image recorded no boot history at all"
    fail=1
elif [ "$(echo "$history" | tr '\n' ' ' | tr -s ' ')" != "a/committed/none " ]; then
    echo "    FAIL: expected exactly one boot, slot A committed with nothing to fall back from"
    fail=1
fi

# The self-test driver must NOT be in this image. If it were, the machine would walk a
# scenario and power off — the failure this scenario exists to catch.
if mtype -i "$DISK@@1M" ::/MORPH-TEST-RESULT.TXT > /dev/null 2>&1; then
    echo "    FAIL: the hardware image carries a self-test result file — it is a test rig after all"
    fail=1
fi

# The hardware report (doc 20, E12), read off the stick's FAT partition the way a person reads
# it on their own computer. The VM's own facts must be in it: it is a QEMU machine, it has a
# screen connected, its disk is virtio with the virtio driver bound, and its keyboard is the
# PS/2 one. A report that is there but empty would pass a check for the file alone.
reports=$({ mdir -b -i "$DISK@@1M" ::/morpheos-report 2>/dev/null || true; } | sed 's|/$||; s|.*/||' | grep -E '^[0-9]{3}-' || true)
echo "    reports: $(echo "$reports" | tr '\n' ' ')"
if [ "$(echo "$reports" | grep -c .)" != 1 ]; then
    echo "    FAIL: expected exactly one hardware report on the stick after one boot"
    fail=1
else
    rdir="::/morpheos-report/$reports"
    summary=$(mtype -i "$DISK@@1M" "$rdir/summary.txt" 2>/dev/null || true)
    lspci=$(mtype -i "$DISK@@1M" "$rdir/lspci.txt" 2>/dev/null || true)
    report_check() {
        if [ "$2" = yes ]; then echo "    ok    report: $1"; else echo "    FAIL  report: $1"; fail=1; fi
    }
    yes_if() { if "$@" > /dev/null 2>&1; then echo yes; else echo no; fi; }
    report_check "the folder is the first report, from slot A ($reports)" \
        "$(yes_if grep -qE '^001-.*-a$' <<< "$reports")"
    report_check "the machine is the QEMU VM" "$(yes_if grep -q '^MACHINE   QEMU' <<< "$summary")"
    report_check "a screen is connected" "$(yes_if grep -qE '^  card[0-9]+-.*: connected' <<< "$summary")"
    report_check "the virtio disk has its driver bound" \
        "$(if grep -iE -A3 'virtio.*block' <<< "$lspci" | grep -q 'Kernel driver in use: virtio-pci'; then echo yes; else echo no; fi)"
    report_check "devices are named, not just numbered (the PCI name database is there)" \
        "$(yes_if grep -q 'Red Hat, Inc' <<< "$lspci")"
    report_check "the PS/2 keyboard is an input device" \
        "$(yes_if grep -q 'AT Translated Set 2 keyboard' <<< "$summary")"
    report_check "it says what firmware is missing (a section, even if 'none')" \
        "$(yes_if grep -q 'FIRMWARE THE KERNEL ASKED FOR AND DID NOT FIND' <<< "$summary")"
    if [ "$fail" != 0 ]; then
        echo "    the summary was:"
        sed 's/^/      | /' <<< "$summary" | head -40
    fi
fi

if [ "$fail" != 0 ]; then
    echo "plain image: FAILED" >&2
    exit 1
fi
echo "plain image: PASS — boots into the shell with Firefox and the Wi-Fi picker, reaches a login, commits slot A, stays up, and reports its hardware"
