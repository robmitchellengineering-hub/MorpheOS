#!/bin/sh
# offline-bootstrap.sh — set up a MorpheOS machine that CANNOT reach the internet, from a Mac that can.
#
# WHY THIS EXISTS. On 2026-11-04 the 2012 MacBook booted MorpheOS on a work network where every outbound
# connection failed (TCP 443 to raw IPs included), so rustup, crates.io and GitHub were all unreachable. The
# machine could still be reached over the LAN. This script is the procedure that worked, written down so it is a
# command instead of an hour of discovery.
#
#   scripts/offline-bootstrap.sh <machine-ip> [branch]
#
# It asks the Mac to do the network work and the machine to do the compiling:
#   1. download the standalone Linux Rust distribution (rustc + cargo + rust-std only)
#   2. vendor the crate sources for the branch
#   3. tar the source tree
#   4. copy all three over ssh
#   5. install the toolchain with --components and build the whole workspace, offline
#
# The two failures this encodes, both real:
#   - the standalone distribution keeps rust-std in its own component, so a PATH-only setup gives
#     "can't find crate for core"; install with --components=rustc,cargo,rust-std-x86_64-unknown-linux-gnu
#   - the docs components are gigabytes and fill /persist; never install them on a small data partition
set -eu
HOST=${1:?usage: offline-bootstrap.sh <machine-ip> [branch]}
BRANCH=${2:-main}
RUST_VERSION=${RUST_VERSION:-1.99.0}
KEY=${MORPH_SSH_KEY:-$HOME/.ssh/mesh-admin}
SSH=(ssh -i "$KEY" -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=10 -o IdentitiesOnly=yes)
HERE=$(cd "$(dirname "$0")/.." && pwd)
TMP=${TMPDIR:-/tmp}/morph-offline
mkdir -p "$TMP"

say() { printf '%s\n' "$*"; }

say "1/5 fetching the standalone Linux toolchain ($RUST_VERSION)"
if [ ! -f "$TMP/rust-linux.tar.gz" ]; then
    curl -4 -fsSL --max-time 900 -o "$TMP/rust-linux.tar.gz" \
        "https://static.rust-lang.org/dist/rust-${RUST_VERSION}-x86_64-unknown-linux-gnu.tar.gz"
fi
say "   $(du -h "$TMP/rust-linux.tar.gz" | cut -f1)"

say "2/5 vendoring the crates for $BRANCH"
git -C "$HERE" fetch -q origin "$BRANCH" || true
git -C "$HERE" checkout -q "$BRANCH" 2>/dev/null || say "   (staying on the current checkout)"
rm -rf "$TMP/vendor"
(cd "$HERE" && cargo vendor "$TMP/vendor" >/dev/null)
tar -C "$TMP" -czf "$TMP/vendor.tar.gz" vendor
say "   $(du -sh "$TMP/vendor" | cut -f1)"

say "3/5 tarring the source"
git -C "$HERE" archive --format=tar.gz -o "$TMP/src.tar.gz" HEAD

say "4/5 copying to $HOST over the LAN"
for f in rust-linux.tar.gz vendor.tar.gz src.tar.gz; do
    cat "$TMP/$f" | "${SSH[@]}" "root@$HOST" "cat > /persist/$f"
    say "   $f -> /persist/$f"
done

say "5/5 installing and compiling, offline, on the machine"
"${SSH[@]}" "root@$HOST" 'sh -s' <<'REMOTE'
set -eu
P=/persist
rm -rf "$P/rust" "$P/rust-install" "$P/morpheos" "$P/vendor"
mkdir -p "$P/rust"
tar -xzf "$P/rust-linux.tar.gz" -C "$P/rust"
R="$P/rust/rust-"*"-x86_64-unknown-linux-gnu"
"$R/install.sh" --prefix="$P/rust-install" --disable-ldconfig \
    --components=rustc,cargo,rust-std-x86_64-unknown-linux-gnu >/tmp/install.log 2>&1
tar -xzf "$P/vendor.tar.gz" -C "$P"
mkdir -p "$P/morpheos"
tar -xzf "$P/src.tar.gz" -C "$P/morpheos"
mkdir -p "$P/morpheos/.cargo"
printf '[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = "/persist/vendor"\n\n[net]\noffline = true\n' \
    > "$P/morpheos/.cargo/config.toml"
rm -rf "$P/rust" "$P/rust-linux.tar.gz" "$P/vendor.tar.gz" "$P/src.tar.gz"
export PATH="$P/rust-install/bin:$PATH" LD_LIBRARY_PATH="$P/rust-install/lib"
export CARGO_HOME="$P/cargo-home" CARGO_TARGET_DIR="$P/target"
cd "$P/morpheos"
"$P/rust-install/bin/cargo" build --release --workspace
echo "offline build finished on $(hostname)"
REMOTE

say ""
say "done. On the machine: /persist/morph-build [crate] rebuilds without a network."
