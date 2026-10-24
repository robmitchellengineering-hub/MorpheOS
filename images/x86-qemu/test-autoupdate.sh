#!/bin/bash
# An update arrives by itself over HTTPS (doc 20, S3): boot the `autoupdate` image, whose
# timer checks the CI host's update server, downloads a signed payload, installs it into the
# spare slot with the image's own `morph-update`, and arms a trial — with no input from the
# harness after boot. The verdicts are read off the disk (the boot history and slot B's root)
# and the server's request log, never the serial console (AGENTS.md H8).
#
#   sudo images/x86-qemu/test-autoupdate.sh <workdir> [good]
#
# Only `good` exists in this change (PR 4 adds `bad` and `tampered`): slot B is a healthy
# release 2, so the trial commits, and the boot history is
# `a/committed/none b/trial/committed b/committed/none`.
set -euo pipefail

WORK=${1:?usage: test-autoupdate.sh <workdir> [good]}
SCENARIO=${2:-good}
case "$SCENARIO" in good) ;; *) echo "unknown scenario: $SCENARIO" >&2; exit 2 ;; esac
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

payload=$("$HERE/build-payload.sh" "$WORK" "$WORK/payload" dir | tail -1)

# The server tree: `stable/LATEST` (a hint, trusted for nothing) and
# `stable/<release>/{BLAKE3SUMS,BLAKE3SUMS.sig,root.img,vmlinuz,initrd.img}`.
serve="$WORK/server"
rm -rf "$serve"
mkdir -p "$serve/stable/2"
cp -a "$payload/." "$serve/stable/2/"
printf '2\n' > "$serve/stable/LATEST"

# 3. Serve it over HTTPS on 127.0.0.1:8123, which QEMU's user network shows the VM as
# 10.0.2.2 (the same pattern test-browser.sh uses for HTTP), logging every request.
server_log="$WORK/update-server.log"
serial="$WORK/serial-autoupdate.log"
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
trap 'kill $srv_pid 2>/dev/null || true' EXIT
# Give the server a moment to bind. The VM's timer does not fire for ~30 s, so there is no
# race to win — if the process is still alive now, it has loaded its cert and is serving.
sleep 1
kill -0 "$srv_pid" 2>/dev/null || { echo "    FAIL: the HTTPS server did not start" >&2; exit 1; }

# 4. Boot, with networking, until the machine powers itself off (the test driver does, once
# slot B is committed). Reboots stay inside the one QEMU process.
start=$(date +%s)
VM_NET=1 "$HERE/run-vm.sh" "$disk" "$serial" "${VM_TIMEOUT:-900}" || {
    echo "    FAIL: the VM did not power off cleanly (see $serial)"
    exit 1
}
echo "    vm time: $(( $(date +%s) - start ))s"

fail=0
got=$(recorded "$disk")
# Newline-joined, exactly as `recorded` emits and as test-fallback.sh's `expected` does:
# the display/FAIL lines echo `$want` unquoted, which collapses whitespace, so a
# space-joined literal *looks* right while `[ "$got" = "$want" ]` can never match it.
want=$(printf '%s\n' a/committed/none b/trial/committed b/committed/none)
default=$(mtype -i "$disk@@1M" ::/loader/loader.conf | awk '$1=="default"{print $2}')
echo "    boots:   $(echo $got)"
echo "    default: $default"
[ "$got" = "$want" ] || { echo "    FAIL: expected boots: $(echo $want)"; fail=1; }
[ "$default" = "morpheos-b.conf" ] || { echo "    FAIL: expected default slot b"; fail=1; }

# The content, not just the flow: slot B's root must carry the payload's marker.
if marker_present "$disk"; then
    echo "    slot b carries the payload's marker: the update was written AND booted"
else
    echo "    FAIL: slot b booted without the payload's marker"
    fail=1
fi

# The server saw the root image downloaded exactly once: the timer fetched it, installed it,
# and never fetched it again (a second fetch would mean the recorded state did not skip).
downloads=$(grep -c 'GET /stable/2/root.img' "$server_log" 2>/dev/null || true)
if [ "$downloads" = 1 ]; then
    echo "    the server served root.img exactly once"
else
    echo "    FAIL: the server served root.img $downloads time(s), expected 1"
    fail=1
fi

if [ $fail = 0 ]; then
    echo "    PASS"
else
    echo "    what the server saw (requests from the VM):"
    sed 's/^/      /' "$server_log" | tail -20
    echo "    see $serial and $server_log"
fi
exit $fail
