# 2026-10-31 · DeepSeek · Phase 0 of our own kernel, in the security arm

Rob: *"yeah i agree then with develop along side, start in the security arm"*, after D19 — MorpheOS owns its
kernel (Linux, built and patched by us), developed in the Secure edition.

## What is here

- **`config.fragment`** — the configuration decisions, on top of `defconfig`: `LOCALVERSION="-morpheos"`, the
  drivers this machine actually has (b43, i915, snd-hda-intel, xhci, ext4, vfat), our userland's
  requirements (io_uring, userfaultfd, BPF LSM), and the KSPP hardening set.
- **`branding.patch`** — `UTS_SYSNAME` from "Linux" to "MorpheOS". This is the only place `uname -s` can
  change: it is a kernel constant, which is why a userland wrapper never worked and why Tailscale kept
  reporting Linux.
- **`build.sh`** — fetch a pinned kernel, apply the branding, merge the fragment over `defconfig`, build
  `bzImage` + modules, and **write a report** recording what was actually applied rather than what was asked
  for.

## What it took to get a build host (all measured, no root)

The KVM host has `gcc`, `make`, `bc` and headers but not the kernel's build tools. Extracted into
`/tmp/kdeps` instead of asking for root:

| needed | why | fixed by |
|---|---|---|
| `bison`, `flex` | kconfig's parser and lexer | debs from the Ubuntu pool, extracted |
| `m4` | flex execs it from a compiled-in path | deb extracted, `M4=` set explicitly |
| bison's data files | `bison: cannot open m4sugar.m4` | `BISON_PKGDATADIR=/tmp/kdeps/root/usr/share/bison` |
| `libelf` headers | objtool | deb extracted, `HOSTCFLAGS`/`HOSTLDFLAGS` pointed at it |

And one design correction: **`tinyconfig` cannot work as the base** — it sets `CONFIG_BLOCK=n`, so ext4 and
vfat are impossible and the fragment's filesystem lines silently vanish. Phase 0 builds on `defconfig`
(with the fragment overriding it); trimming to the last kilobyte is Phase 2's job, not the first boot's.

## Verified so far

```
CONFIG_LOCALVERSION="-morpheos"
CONFIG_B43=m       CONFIG_SND_HDA_INTEL=m     CONFIG_EXT4_FS=y
CONFIG_BPF_LSM=y   CONFIG_SLAB_FREELIST_HARDENED=y   CONFIG_USERFAULTFD=y
#include/linux/uts.h:  #define UTS_SYSNAME "MorpheOS"
```
The build itself was running on pacland-rog (8 cores) when this was written.

## The falsifier, unchanged

Phase 0 has failed if the machine boots our kernel and loses Wi-Fi, audio or the panel.
