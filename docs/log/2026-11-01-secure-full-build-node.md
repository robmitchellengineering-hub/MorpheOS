# 2026-11-01 · DeepSeek · a full build node, proven by exit code

Rob, after I called the VM a build node: *"Why do i not belive you, its fully tested on the vm? Full harness? Full
build node? Working? MorpheOS?"* — the right challenge, and the answer was **no**. This is the fix and its proof.

## What was false

```
cargo build --release --workspace      (on the MorpheOS VM, 2026-11-01)
libudev-sys build.rs panicked:
  Package 'libudev' was not found in the pkg-config search path
build failed
```

The image had **two** `-dev` packages and **none** of the eight libraries the workspace links against. It could
compile a pure-Rust crate and not its own OS. The failed build then put 1.4 GB of `target/` into the 4.3 GB data
partition and took it to 98%.

## What changed

1. **The libraries.** `libudev-dev libwayland-dev libxkbcommon-dev libinput-dev libseat-dev libpixman-1-dev
   libgbm-dev libdrm-dev libx11-dev libxkbcommon-x11-dev` on the person-facing image, with an assertion that each
   **`.pc` file is inside the image** — what pkg-config and the crates' build scripts actually look for, which a
   runtime-only package would pass a `dpkg -l` check and still fail.
2. **The disk.** `VM_EXTRA_DISK` attaches a builder disk. The image's data partition is sized for a person; a
   builder needs GBs, so 12 GB went on its own device, mounted under `/persist/build` because the root is
   read-only (the first attempt died at `mkdir /mnt/build: Read-only file system` — recorded, not hidden).

## The proof

Fresh image, no toolchain, everything installed from nothing:

```
=== 1. builder disk ===   /dev/vdb  12G  24K  12G  1% /persist/build
=== 2. toolchain ===      rustc 1.99.0 / cargo 1.99.0
=== 3. source ===         cloned to /persist/build/morpheos
=== 4. the whole workspace ===
   Compiling flow-wifi / morphd / morph-autoupdate / morph-heal / flow-notice / morph-graph
   Finished `release` profile [optimized] target(s) in 8m 19s
BUILD-EXIT=0
/dev/vdb  12G  745M  11G  7% /persist/build
```

**`BUILD-EXIT=0`** for `cargo build --release --workspace`, including `flow-compositor`, `flow-wifi`, `morphd`,
`strata`. 745 MB of output on a 12 GB disk.

Before the boot, the image was checked inside itself: `pkg-config --exists` returned **present for all eight**
(`libudev`, `wayland-client`, `xkbcommon`, `libinput`, `libseat`, `pixman-1`, `gbm`, `libdrm`).

## The harness, honestly

The pull path works — measured runs in the mesh repo:

| node | runs executed | transport |
|---|---|---|
| win | 134 | pull agent |
| win-2 | 325 | pull agent |
| pacland-rog | 195 | pull agent |
| **morpheos-vm** | every run is `*-ssh-dispatch` | **push only: no pull agent** |

So `dispatch.py` works for nodes with an agent, and **queued jobs for `morpheos-vm` never execute** — the node card
now says so instead of implying otherwise. Installing a pull agent on the MorpheOS node is the next step, not a
quiet assumption.

## What is still conditional

- The **running** `morpheos-vm` (port 2302) still has the OLD image: the dev libraries are in
  `fix/build-node-image`, so the node becomes a full builder when it is rebooted from that image.
- The **hardware** has not run any of this: it was off, and nothing here is a hardware measurement.
