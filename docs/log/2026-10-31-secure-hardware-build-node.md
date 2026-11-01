# 2026-10-31 · DeepSeek · the hardware as a build node, ready for Tuesday

Rob: *"keep going as you were in development for Tuesday on hardware full morpheos build node on the robmesh"*.

## What changed

**The toolchain is where a shell finds it.** The image carries `gcc`/`ld`/`make` (so a machine can build itself),
and the Rust toolchain lives on `/persist` (the root is read-only). But a toolchain that needs
`export PATH=/persist/cargo/bin:...` before every use is a toolchain nobody uses — and **the mesh dispatches
non-interactive shells, which never read a profile**. Two files:

- `overlay-hardware/etc/profile.d/50-morpheos-toolchain.sh` — puts `/persist/cargo/bin` on `PATH` and sets
  `RUSTUP_HOME`/`CARGO_HOME` for login shells.
- `overlay-hardware/usr/bin/morph-toolchain` — prints what the machine can build with, including the exact
  environment a non-interactive caller needs, and **says plainly when a tool is absent** instead of printing
  nothing and letting the caller infer success.

Wiring: `build.sh` copies `overlay-hardware/` into the image when `HARDWARE=1` (line 322), and the binary is
committed `100755`, so it lands executable.

## The node card, corrected

`nodes/morpheos-2012.json` named a tailnet address that **no longer exists** (`100.99.205.117`) and listed **no
build capability at all**, while the machine has `gcc`/`ld`/`make` in the image and can compile Rust once the
`/persist` toolchain is there. Now:

- addresses are marked **last observed** with when (`192.168.15.12`, `100.94.132.16`), because the machine was
  off when the card was corrected and a card must not claim a current address it has not seen;
- `build:c` is a real capability, `build:rust` is **conditional** on `/persist` with `morph-toolchain` as the way
  to ask;
- `cannot`: TPM (this model has none — see [MEASURED-BOOT.md](../MEASURED-BOOT.md)), 3D GPU, nested KVM.

## What could not be done, and is not claimed

**The machine was off.** Both `192.168.15.12` and `100.94.132.16` timed out, so there is **no dispatched compile
on the hardware** in this record. The card says so, and the conditional capability stays conditional until a job
runs on it.

The recipe for that moment, in order:

```
ssh root@192.168.15.12 morph-toolchain          # what is actually installed
ssh root@192.168.15.12 'cd /persist/morpheos && \
  RUSTUP_HOME=/persist/rust CARGO_HOME=/persist/cargo \
  PATH=/persist/cargo/bin:$PATH cargo build --release -p morph-update'
```

then a `dispatch-ssh.sh` job to `morpheos-2012`, and the card's `build:rust` becomes measured rather than
conditional.
