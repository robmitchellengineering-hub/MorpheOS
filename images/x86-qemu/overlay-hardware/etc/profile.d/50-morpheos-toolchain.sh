# MorpheOS on this machine: the Rust toolchain lives on /persist, so put it on PATH for every shell.
#
# WHY THIS FILE EXISTS. The image carries a C toolchain (build-essential) because a compiler on the machine is
# what lets it build itself; the Rust toolchain is installed into /persist because the root is read-only. But a
# toolchain that needs `export PATH=/persist/cargo/bin:...` before every use is a toolchain nobody uses — and the
# robmesh dispatches non-interactive shells, which never read a person's profile.
#
# So this is read by login shells, and `morph-toolchain` gives a non-interactive caller the same answer. Nothing
# here is a claim that a build will succeed: it makes the tools reachable.
#
# TWO LAYOUTS, because a machine can be set up either way and the shell should not care:
#   /persist/cargo/bin         rustup (installed with a network)
#   /persist/rust-install/bin  the standalone distribution (installable with no network at all)
# Measured 2026-11-04 on the reference laptop: a machine whose Rust came from the standalone distribution had no
# toolchain on PATH at all, because only the rustup path was checked.

for dir in /persist/cargo/bin /persist/rust-install/bin; do
    [ -x "$dir/cargo" ] || continue
    case ":${PATH}:" in
        *":$dir:"*) ;;
        *) PATH="$dir:${PATH}" ;;
    esac
    export PATH
done

if [ -x /persist/cargo/bin/cargo ]; then
    RUSTUP_HOME=/persist/rust
    CARGO_HOME=/persist/cargo
    export RUSTUP_HOME CARGO_HOME
fi
if [ -d /persist/rust-install/lib ]; then
    LD_LIBRARY_PATH="/persist/rust-install/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export LD_LIBRARY_PATH
fi

# The other half of the same lesson: a build fails on this image when the crates were never vendored, so say so
# where a person will see it rather than letting them infer it from a cargo error.
if [ -x /usr/bin/morph-build ] && [ ! -d /persist/vendor ]; then
    printf 'morph-build: no vendored crates at /persist/vendor - a build will use the network\n' >&2
fi
