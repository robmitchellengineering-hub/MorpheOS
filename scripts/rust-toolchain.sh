#!/usr/bin/env bash
# Install, and use, exactly the Rust that rust-toolchain.toml names (doc 20, S9).
#
#     bash scripts/rust-toolchain.sh
#
# Called by CI (every job that builds Rust) and by scripts/setup-linux.sh, so CI and the
# builder's machine run the same compiler and the same clippy. It installs rustup if there is
# none, installs the pinned toolchain with its components, and FAILS if the `rustc` that
# cargo will run is any other version (an override, or a `RUSTUP_TOOLCHAIN` in the
# environment, would otherwise win quietly).
set -euo pipefail
cd "$(dirname "$0")/.."

want=$(sed -nE 's/^channel *= *"([^"]+)".*/\1/p' rust-toolchain.toml)
case "$want" in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) echo "rust-toolchain.toml must name an exact version, like 1.99.0 (it says '$want')" >&2; exit 1 ;;
esac

if ! command -v rustup > /dev/null && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
if ! command -v rustup > /dev/null; then
    echo "== installing rustup"
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
    . "$HOME/.cargo/env"
    # Later steps of a CI job need it on their PATH too.
    if [ -n "${GITHUB_PATH:-}" ]; then echo "$HOME/.cargo/bin" >> "$GITHUB_PATH"; fi
fi

# With no argument, rustup installs what rust-toolchain.toml says: version and components.
rustup toolchain install
got=$(rustc --version | awk '{print $2}')
if [ "$got" != "$want" ]; then
    echo "rustc here is $got, but rust-toolchain.toml says $want" >&2
    echo "(is RUSTUP_TOOLCHAIN set, or a rustup override in this directory?)" >&2
    exit 1
fi
echo "== $(rustc --version), $(cargo clippy --version), as rust-toolchain.toml says"
