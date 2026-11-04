# 2026-10-31 · DeepSeek · the kernel gate, and proof that it can fail

## The gate

`.github/workflows/kernel.yml`, on the `secure` arm: it builds the kernel from `secure/kernel`, boots the
result in QEMU with a statically-linked init that prints `uname -s`, and fails unless the machine says
MorpheOS. Three architectures, because that is the claim:

| arch | what it covers | gate |
|---|---|---|
| x86_64 | the 2012 laptop, enterprise servers | builds, boots, `MORPHEOS-GATE uname-s: MorpheOS` |
| arm64 | Pi 3/4/5, arm64 servers | builds, boots in QEMU virt, `uname-s: MorpheOS` |
| arm | Raspberry Pi Zero (armv6) | builds (no faithful emulator; needs the board to boot) |

```
arm64 serial:
[    0.000000] Linux version 6.8.0-morpheos (…) (aarch64-linux-gnu-gcc …)
MORPHEOS-GATE uname-s: MorpheOS
MORPHEOS-GATE uname-r: 6.8.0-morpheos
```

## Proof it can fail — a gate that has never failed is theatre

Branch `test/gate-must-fail` builds the same kernel with the branding **deliberately** left as Linux:

```
MORPHEOS-GATE uname-s: Linux
MORPHEOS-GATE uname-r: 6.8.0-morpheos
Boot it and read what it says -> failure
```

The build succeeds and the boot succeeds; the **assertion** is what goes red. That is the distinction the
gate exists for: a kernel that builds is not a kernel that is ours.

## Four defects it caught on its first day

1. the scripts were committed `100644` — CI: `Permission denied` (they are `100755` now);
2. the arch argument was being used as the work directory (`build.sh arm64 …` made a folder called arm64);
3. `bzImage` is an x86-only make target — arm64 builds `Image`, arm builds `zImage`;
4. the first initramfs carried the runner's **x86_64 busybox**, so x86_64 passed while arm64 built, booted,
   and could not exec its init. Each architecture now gets a static init built with its own cross compiler.

Item 4 is the reason the gate is worth its minutes: it was silently testing one architecture and appearing to
test three.

## Boot time without an initramfs (Phase 1 measurement)

Our kernel's loader entry names no `initrd`:

```
cmdline: root=PARTLABEL=root-a ro morph.slot=a console=tty0 console=ttyS0,115200 panic=5
Startup finished in 1.489s (kernel) + 12.536s (userspace) = 14.025s
```

The stock 1.1 image for comparison, which does use one:

```
cmdline: initrd=\morpheos\a\initrd.img
Startup finished in 2.246s (kernel) + 27.720s (userspace) = 29.967s
```

**Comparable, and quoted as such:** the kernel stage is **0.757 s faster** without an initramfs. The
userspace totals are **not** comparable and are not claimed: the stock VM's blame shows
`morpheos-tailscale-up.service` waiting (it is logged out), which is a network state, not a kernel property.

```
stock : 59min 16.337s morpheos-tailscale-up.service / 15.364s systemd-binfmt / 15.302s systemd-resolved
ours  : 10.640s morph-boot-sound.service (the theme playing) / 738ms dev-vda2 / 602ms NetworkManager
```
