# 2026-11-04 · DeepSeek · the 2012 MacBook builds MorpheOS offline, on itself

Rob booted the stick at work. The work network blocks **all** outbound traffic — TCP 443 fails even to raw
IPs — so the machine could not fetch a toolchain, crates or source. It could still be reached over the LAN,
so everything was carried to it.

## What was carried, and why

| payload | size | why |
|---|---|---|
| `rust-1.99.0-x86_64-unknown-linux-gnu` distribution | 379 MB | the image has no Rust toolchain, and there is no network to install one |
| 274 vendored crates (`cargo vendor`) | 62 MB compressed, 421 MB on disk | a build needs crates; with no network it needs them on disk |
| the source tree | 4 MB | no `git clone` without a network |

## The two failures on the way, both real

1. **`can't find crate for core`** — the standalone distribution keeps `rust-std` in its own component, so
   pointing `PATH` at `rustc/bin` gives a compiler with no standard library. Fixed by installing with
   `--components=rustc,cargo,rust-std-x86_64-unknown-linux-gnu`.
2. **`No space left on device`** — the first install pulled the docs components and filled the 3.9 GB data
   partition. The minimal component set installs in about a gigabyte; the docs were gigabytes of nothing.

## The result, on the hardware

```
   Compiling flow-compositor / flow-wifi / flow-timeline / strata / morphd / morph-heal / morph-graph
   Finished `release` profile [optimized] target(s) in 3m 56s
BUILD-EXIT=0

3897880  /persist/target/release/flow-compositor
1802912  /persist/target/release/morphd
4232488  /persist/target/release/strata
/persist  1.9G used of 3.9G
```

**The machine compiles its own operating system, with no network at all.**

And through the dispatch path, not a manual ssh:

```
node  : morpheos-2012 (192.168.15.13)
{"ok": true, "exitCode": 0}
binary: 1074592 bytes   sha256 c1b0aa02fbd5d6742f01a9198e1edd22
morph-update: no --payload DIR given
```

## Repeatable after a reboot

`/persist/morph-build [crate]` — one command, offline, using the `/persist` toolchain and vendored crates.
It lives on `/persist` because the root is read-only; the *durable* PATH hook is an image overlay
(`overlay-hardware/etc/profile.d/50-morpheos-toolchain.sh`), which only takes effect after a flash.

## What this does not do

**It is not on the robmesh.** Tailscale needs outbound connectivity and this network blocks it, so the node
joins when the machine is on a network that allows internet — a phone hotspot is enough. The card records the
LAN address (`192.168.15.13`), the measured capabilities, and the reason the tailnet is absent, rather than
implying a membership it does not have.
