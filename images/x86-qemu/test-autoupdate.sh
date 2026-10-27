#!/bin/bash
# An update arrives by itself over HTTPS (doc 20, S3): boot the `autoupdate` image, whose
# timer checks the CI host's update server, downloads a signed payload, installs it into the
# spare slot with the image's own `morph-update`, and arms a trial — with no input from the
# harness after boot. The verdicts are read off the disk (the boot history and slot B's root)
# and the server's request log, never the serial console (AGENTS.md H8).
#
#   sudo images/x86-qemu/test-autoupdate.sh <workdir> [good|bad|tampered]
#
#   good     slot B is a healthy release 2, so the trial commits (PR 3): the machine powers
#            itself off once slot B is committed.
#   bad      a correctly signed payload whose system fails the health check: the trial falls
#            back, the default stays A, and later timer runs do not download it again. The
#            harness stops the VM (doc 20, S3 note 2).
#   tampered a byte of root.img changed after signing: no trial, slot B untouched, and the
#            server log shows the files WERE downloaded (so the refusal is the digest check,
#            not a failed fetch). The harness stops the VM.
set -euo pipefail

WORK=${1:?usage: test-autoupdate.sh <workdir> [good|bad|tampered]}
SCENARIO=${2:-good}
case "$SCENARIO" in good|bad|tampered) ;; *) echo "unknown scenario: $SCENARIO" >&2; exit 2 ;; esac
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$WORK"

# The verdict: one "<booted>/<phase>/<action>" per line of the boot history, read off the ESP.
recorded() {
    mtype -i "$1@@1M" ::/morph-boot/history.jsonl 2>/dev/null |
        sed -nE 's/.*"booted":"([ab])".*"phase":"([a-z]+)".*"action":"([a-z-]+)".*/\1\/\2\/\3/p'
}

# Does slot B's own root carry the file morph-update writes? Read off the disk, not a log
# line. debugfs exits 0 whether or not the file exists, so parse what it printed (H3).
marker_present() {
    local loop out
    loop=$(losetup -Pf --show "$1")
    out=$(debugfs -R "stat /etc/morph-update-marker" "${loop}p3" 2>&1)
    losetup -d "$loop"
    case "$out" in *"Inode:"*) return 0 ;; *) return 1 ;; esac
}

# Does slot B's boot directory on the ESP carry morph-update's `.morph-incomplete` marker?
# morph-update writes it before touching the slot and clears it only after the write verifies,
# so its presence means an install BEGAN; a refused payload never writes it.
incomplete_present() {
    mtype -i "$1@@1M" ::/morpheos/b/.morph-incomplete >/dev/null 2>&1
}

# Stop the VM the harness started in the background, cleanly through QMP (the same pattern
# test-browser.sh uses), then reap it. The verdicts are read from the disk only after this.
stop_vm() {
    python3 - "$qmp" <<'PY' 2>/dev/null || true
import json, socket, sys
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f = s.makefile("rwb"); f.readline()
for c in ("qmp_capabilities", "quit"):
    f.write((json.dumps({"execute": c}) + "\n").encode()); f.flush(); f.readline()
PY
    wait "$vm" 2>/dev/null || true
}

# 1. A throwaway CA and a server certificate for 10.0.2.2, made per run (doc 20, S3 note 4):
# the CA is baked into the image, the server cert is used by the HTTPS server below, and
# neither outlives this run.
ca="$WORK/update-test-ca.pem"
cakey="$WORK/update-test-ca.key"
srv="$WORK/update-server.pem"
srvkey="$WORK/update-server.key"
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$cakey" -out "$ca" -days 1 \
    -subj "/CN=MorpheOS test CA" -addext "basicConstraints=critical,CA:TRUE" >/dev/null 2>&1
openssl req -newkey rsa:2048 -nodes -keyout "$srvkey" -out "$WORK/server.csr" \
    -subj "/CN=10.0.2.2" >/dev/null 2>&1
openssl x509 -req -in "$WORK/server.csr" -CA "$ca" -CAkey "$cakey" -CAcreateserial \
    -out "$srv" -days 1 -extfile <(printf 'subjectAltName=IP:10.0.2.2\n') >/dev/null 2>&1

# 2. The image (with the CA baked in) and then the payload directory the server serves. The
# payload is built AFTER the image: build-payload.sh reuses slot A's root and the key build.sh
# made, both of which exist only once the image is assembled.
echo "=== autoupdate: $SCENARIO"
export UPDATE_TEST_CA="$ca"
disk=$("$HERE/build.sh" "$WORK" autoupdate | tail -1)
rm -f "${disk%.img}.vars.fd"

# The control, first: slot B starts as a copy of slot A, so it must NOT already carry the
# marker — otherwise the assertion after the run would prove nothing.
if marker_present "$disk"; then
    echo "    FAIL: slot B already has the update marker before the VM booted"
    exit 1
fi

# The payload the server serves. `good` is the healthy release; `bad` and `tampered` are the
# two refusal payloads build-payload.sh makes (doc 20, S3 note 1).
payload_kind=dir
[ "$SCENARIO" = good ] || payload_kind="$SCENARIO"
payload=$("$HERE/build-payload.sh" "$WORK" "$WORK/payload" "$payload_kind" | tail -1)

# The server tree: `stable/LATEST` (a hint, trusted for nothing) and
# `stable/<release>/{BLAKE3SUMS,BLAKE3SUMS.sig,root.img,vmlinuz,initrd.img}`.
serve="$WORK/server"
rm -rf "$serve"
mkdir -p "$serve/stable/2"
cp -a "$payload/." "$serve/stable/2/"
printf '2\n' > "$serve/stable/LATEST"

# 3. Serve it over HTTPS on 127.0.0.1:8123, which QEMU's user network shows the VM as
# 10.0.2.2 (the same pattern test-browser.sh uses for HTTP), logging every request.
server_log="$WORK/update-server-$SCENARIO.log"
serial="$WORK/serial-autoupdate-$SCENARIO.log"
rm -f "$server_log" "$serial"
cat > "$WORK/serve.py" <<'PY'
import http.server
import ssl
import sys

serve_dir, cert, key, log_path, port = sys.argv[1:6]
port = int(port)


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=serve_dir, **kwargs)

    def log_message(self, fmt, *args):
        with open(log_path, "a") as f:
            f.write(fmt % args + "\n")
            f.flush()


httpd = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(cert, key)
httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
httpd.serve_forever()
PY
python3 "$WORK/serve.py" "$serve" "$srv" "$srvkey" "$server_log" 8123 &
srv_pid=$!
vm=""
trap 'kill $srv_pid 2>/dev/null || true; [ -n "$vm" ] && kill "$vm" 2>/dev/null || true' EXIT
# Give the server a moment to bind. The VM's timer does not fire for ~30 s, so there is no
# race to win — if the process is still alive now, it has loaded its cert and is serving.
sleep 1
kill -0 "$srv_pid" 2>/dev/null || { echo "    FAIL: the HTTPS server did not start" >&2; exit 1; }

# 4. Boot, with networking. `good` powers itself off once slot B commits; `bad` and `tampered`
# stay up and the harness stops the VM (doc 20, S3 note 2), because their verdict is read after
# a period of skipped timer runs.
qmp="$WORK/qmp-autoupdate.sock"
rm -f "$qmp"
start=$(date +%s)
case "$SCENARIO" in
    good)
        VM_NET=1 "$HERE/run-vm.sh" "$disk" "$serial" "${VM_TIMEOUT:-900}" || {
            echo "    FAIL: the VM did not power off cleanly (see $serial)"
            exit 1
        }
        ;;
    bad)
        VM_NET=1 VM_QMP="$qmp" "$HERE/run-vm.sh" "$disk" "$serial" "${VM_TIMEOUT:-900}" &
        vm=$!
        # Wait for the fallback (the event that writes b/trial/fallback — H13), then for three
        # later timer runs that skip the already-tried release. Each skip prints "already
        # failed a trial" to the console: that is a TRIGGER for when to stop, never the
        # verdict — the verdict is the boot history and the server log, read after the VM is
        # gone (H8).
        fallback=0 skips=0 deadline=$(( $(date +%s) + 600 ))
        while [ "$(date +%s)" -lt "$deadline" ]; do
            kill -0 "$vm" 2>/dev/null || break
            if [ "$fallback" = 0 ] && grep -aq '"action":"fallback"' "$serial" 2>/dev/null; then
                fallback=1
            fi
            skips=$(grep -ac 'already failed a trial' "$serial" 2>/dev/null || true)
            if [ "$fallback" = 1 ] && [ "$skips" -ge 3 ]; then break; fi
            sleep 2
        done
        stop_vm
        if [ "$fallback" = 0 ]; then
            echo "    FAIL: the trial never fell back (see $serial)"
            exit 1
        fi
        if [ "$skips" -lt 3 ]; then
            echo "    FAIL: saw $skips skipped timer run(s), expected at least 3 (see $serial)"
            exit 1
        fi
        ;;
    tampered)
        VM_NET=1 VM_QMP="$qmp" "$HERE/run-vm.sh" "$disk" "$serial" "${VM_TIMEOUT:-900}" &
        vm=$!
        # Wait for the refusal: morph-autoupdate prints "the payload was refused" once the
        # digest check rejects the tampered root.img (again a trigger, not the verdict). The
        # verdict — no trial, slot B untouched, files downloaded — is read after the VM is gone.
        refused=0 deadline=$(( $(date +%s) + 300 ))
        while [ "$(date +%s)" -lt "$deadline" ]; do
            kill -0 "$vm" 2>/dev/null || break
            if grep -aq 'the payload was refused' "$serial" 2>/dev/null; then refused=1; break; fi
            sleep 2
        done
        stop_vm
        if [ "$refused" = 0 ]; then
            echo "    FAIL: the payload was never refused (see $serial)"
            exit 1
        fi
        ;;
esac
echo "    vm time: $(( $(date +%s) - start ))s"

# 5. The verdicts, read off the disk and the server log.
fail=0
got=$(recorded "$disk")
default=$(mtype -i "$disk@@1M" ::/loader/loader.conf | awk '$1=="default"{print $2}')
echo "    boots:   $(echo $got)"
echo "    default: $default"
downloads=$(grep -c 'GET /stable/2/root.img' "$server_log" 2>/dev/null || true)

case "$SCENARIO" in
    good)
        # Newline-joined, exactly as `recorded` emits and as test-fallback.sh's `expected`
        # does: the display/FAIL lines echo `$want` unquoted, which collapses whitespace, so a
        # space-joined literal *looks* right while `[ "$got" = "$want" ]` can never match it.
        want=$(printf '%s\n' a/committed/none b/trial/committed b/committed/none)
        [ "$got" = "$want" ] || { echo "    FAIL: expected boots: $(echo $want)"; fail=1; }
        [ "$default" = "morpheos-b.conf" ] || { echo "    FAIL: expected default slot b"; fail=1; }
        # The content, not just the flow: slot B's root must carry the payload's marker.
        if marker_present "$disk"; then
            echo "    slot b carries the payload's marker: the update was written AND booted"
        else
            echo "    FAIL: slot b booted without the payload's marker"
            fail=1
        fi
        # The server saw the root image downloaded exactly once: the timer fetched it, installed
        # it, and never fetched it again (a second fetch would mean the recorded state did not skip).
        if [ "$downloads" = 1 ]; then
            echo "    the server served root.img exactly once"
        else
            echo "    FAIL: the server served root.img $downloads time(s), expected 1"
            fail=1
        fi
        ;;
    bad)
        want=$(printf '%s\n' a/committed/none b/trial/fallback a/committed/none)
        [ "$got" = "$want" ] || { echo "    FAIL: expected boots: $(echo $want)"; fail=1; }
        [ "$default" = "morpheos-a.conf" ] || { echo "    FAIL: expected default slot a"; fail=1; }
        # The trial failed BECAUSE of the payload's broken unit, not some other fault. This is
        # a diagnostic read from the console; the verdict is the history above (H8).
        #
        # Match the prefix, not the whole sentence: the serial console is a shared buffered
        # stream, so a line can be cut off by another writer. On 2026-10-27 the console held
        #   boot-check[389]: morph-boot-check: failed unit: morph-broken.serv[ 4.891308] boot-check[313]: ... decision
        # — the health check had named morph-broken.service and the history said b/trial/fallback,
        # but the word "service" was overwritten, so an exact-match grep failed the job while the
        # machine had done exactly the right thing (one boot-check per boot: 3 boots, 3 decisions).
        if grep -aq 'failed unit: morph-broken' "$serial"; then
            echo "    the trial failed on morph-broken.service"
        else
            echo "    FAIL: the trial did not fail on morph-broken.service"
            grep -a 'failed unit' "$serial" | sed 's/^/      /' || true
            fail=1
        fi
        # Over at least three later timer runs (the stop loop above required three skips), the
        # server was never asked for that release again.
        if [ "$downloads" = 1 ]; then
            echo "    the server served root.img exactly once (no second download over $skips skipped runs)"
        else
            echo "    FAIL: the server served root.img $downloads time(s), expected 1"
            fail=1
        fi
        ;;
    tampered)
        want=$(printf '%s\n' a/committed/none)
        [ "$got" = "$want" ] || { echo "    FAIL: expected boots: $(echo $want) (no trial)"; fail=1; }
        [ "$default" = "morpheos-a.conf" ] || { echo "    FAIL: expected default slot a"; fail=1; }
        if marker_present "$disk"; then
            echo "    FAIL: slot b carries the payload's marker — the tampered payload was written"
            fail=1
        else
            echo "    slot b has no marker: the tampered payload was never written"
        fi
        if incomplete_present "$disk"; then
            echo "    FAIL: slot b has a .morph-incomplete marker — an install began"
            fail=1
        else
            echo "    slot b has no .morph-incomplete marker: no install began"
        fi
        if [ "$downloads" -ge 1 ]; then
            echo "    the server served root.img ($downloads time(s)): the refusal came from the digest check, not a failed fetch"
        else
            echo "    FAIL: the server never served root.img — the fetch failed, so the scenario proves nothing"
            fail=1
        fi
        ;;
esac

if [ $fail = 0 ]; then
    echo "    PASS"
else
    echo "    what the server saw (requests from the VM):"
    sed 's/^/      /' "$server_log" | tail -20
    echo "    see $serial and $server_log"
fi
exit $fail
