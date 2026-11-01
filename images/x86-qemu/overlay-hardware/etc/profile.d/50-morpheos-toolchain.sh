# MorpheOS on this machine: the Rust toolchain lives on /persist, so put it on PATH for every shell.
#
# WHY THIS FILE EXISTS. The image carries a C toolchain (build-essential) because a compiler on the machine is
# what lets it build itself; the Rust toolchain is installed into /persist because the root is read-only. But a
# toolchain that needs `export PATH=/persist/cargo/bin:...` before every use is a toolchain nobody uses — and the
# robmesh dispatches non-interactive shells, which never read a person's profile.
#
# So this is read by login shells, and `morph-toolchain` gives a non-interactive caller the same answer. Nothing
# here is a claim that a build will succeed: it makes the tools reachable.

if [ -x /persist/cargo/bin/cargo ]; then
    RUSTUP_HOME=/persist/rust
    CARGO_HOME=/persist/cargo
    export RUSTUP_HOME CARGO_HOME
    case ":${PATH}:" in
        *":/persist/cargo/bin:"*) ;;
        *) PATH="/persist/cargo/bin:${PATH}" ;;
    esac
    export PATH
fi
