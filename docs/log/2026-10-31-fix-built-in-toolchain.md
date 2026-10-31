# 2026-10-31 · DeepSeek · a machine that can build on itself

Rob: *"ok now, build yourself properly from the inside your online."*

## What happened when I tried, on the machine

The Rust toolchain installed into `/persist` cleanly (`rustc 1.99.0`, the pinned version) and the source
cloned, but every link failed:

```
rust-lld: error: unable to find library -lm / -ldl / -lc
rust-lld: error: unknown emulation: 64
rust-lld: error: unable to find library -lgcc_s
```

The image has `git` and `curl` and no compiler at all — no `cc`, no `ld`, no `libc6-dev`. I got as far as
extracting `libc6-dev` and hand-writing a `cc → rust-lld` wrapper, which is the wrong kind of engineering:
fighting the absence of a toolchain instead of declaring the requirement.

## What ships

`DEV_PKGS=build-essential,pkg-config` on the person-facing images (**not** on the test scenarios, where
~200 MB of toolchain would slow every CI run for nothing), and an assertion that fails the build if
`/usr/bin/gcc`, `/usr/bin/ld` or `/usr/bin/make` are missing from the image:

```
missing /usr/bin/gcc: the machine could not build on itself
```

## What this unlocks

The machine already has the parts a build node needs and that this image could not use: 8 cores, 7 GiB,
104 GB of stick (once the work partition is re-created after a flash), a Rust toolchain that installs into
`/persist`, and a network. With a compiler in the image, `cargo build` on the machine stops failing at the
link and starts being a real build — which is what makes it a node for this project rather than only a
machine I can log into.
