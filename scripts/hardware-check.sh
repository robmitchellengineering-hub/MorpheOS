#!/bin/sh
# MorpheOS on the hardware: what it is, what it can build with, and the workspace build.
# Run at the console as root. Every line is a measurement; nothing here claims success it did not see.
set -u
echo "=== 1. what this machine is ==="
grep PRETTY_NAME /etc/os-release; uname -s; uname -r; nproc; free -g | awk '/Mem:/{print "ram:", $2, "GiB"}'
echo "=== 2. the workspace libraries (the thing that was missing) ==="
for p in libudev wayland-client xkbcommon libinput libseat pixman-1 gbm libdrm; do
    printf '%-14s ' "$p"; pkg-config --exists "$p" && echo ok || echo MISSING
done
echo "=== 3. disk ==="
df -h /persist | tail -1
echo "=== 4. toolchain ==="
command -v gcc >/dev/null && echo "gcc: $(gcc -dumpversion)" || echo "gcc: ABSENT"
if [ -x /persist/cargo/bin/cargo ]; then
    echo "rust: $(/persist/cargo/bin/rustc -V)"
else
    echo "rust: absent - installing into /persist (needs network)"
    mkdir -p /persist/rust /persist/cargo
    curl -4 -fsSL --max-time 180 https://sh.rustup.rs -o /tmp/rustup.sh || { echo "NO NETWORK: cannot install rust"; exit 1; }
    RUSTUP_HOME=/persist/rust CARGO_HOME=/persist/cargo sh /tmp/rustup.sh -y --profile minimal \
        --default-toolchain 1.99.0 --no-modify-path >/tmp/rustup.log 2>&1 || { tail -3 /tmp/rustup.log; exit 1; }
    echo "rust: $(/persist/cargo/bin/rustc -V)"
fi
echo "=== 5. source ==="
if [ ! -d /persist/morpheos/.git ]; then
    git clone -q --depth 1 https://github.com/robmitchellengineering-hub/MorpheOS.git /persist/morpheos || { echo "clone failed"; exit 1; }
fi
cd /persist/morpheos || exit 1
echo "=== 6. the whole workspace ==="
export PATH=/persist/cargo/bin:$PATH RUSTUP_HOME=/persist/rust CARGO_HOME=/persist/cargo
export CARGO_TARGET_DIR=/persist/target
cargo build --release --workspace 2>&1 | tail -8
echo "BUILD-EXIT=$?"
echo "=== 7. after ==="
df -h /persist | tail -1
ls -l /persist/target/release/flow-compositor /persist/target/release/morphd 2>/dev/null | awk '{print $5, $9}'
