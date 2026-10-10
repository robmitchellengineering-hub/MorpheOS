# MorpheOS on Raspberry Pi 5

An immutable A/B image built with Raspberry Pi's own
[rpi-image-gen](https://github.com/raspberrypi/rpi-image-gen) (`image-rota`
layout), plus the MorpheOS layers in `layer/`:

| Partition | Contents |
|---|---|
| 1 `bootconfig` | `autoboot.txt`: which slot boots by default, and which on a trial |
| 2 `boot_a`, 3 `boot_b` | Firmware, kernel and `cmdline.txt` for each slot |
| 4 `system_a`, 5 `system_b` | Read-only system for each slot |
| 6 `persistent` | `/home`, logs and other data, shared by both slots |

`morph-boot` commits a trial slot only after the boot health check passes. A
crash, hang (hardware watchdog) or failed health check means the next boot is
the old slot again, because the Pi firmware clears the `tryboot` flag as soon
as it uses it.

## The self-test image

The CI workflow `images` builds `morpheos-pi5-selftest.img`. On first
power-on it tests the update path on real hardware, by itself:

1. Boots slot A, passes the health check, arms a **trial of slot B**, reboots.
2. Boots slot B as a trial, passes the health check, **commits B**, reboots.
3. Boots slot B again, proving the commit stuck, and prints `morph-test: done`.

Attach a screen to see progress. The whole run takes a few minutes, with
two automatic reboots.

## What is verified, and what is not

The `images` workflow builds this image and then **reads it back**: the six
`image-rota` partitions, the FAT boot selector, `autoboot.txt` with a **default** slot and
a **different** trial slot in their own `[all]` and `[tryboot]` sections, both slots'
boot partitions, and **both** system partitions carrying `morph-boot`, the health check,
the Pi drop-in and the **enabled** unit. The partition numbers
`crates/morph-boot/src/pi.rs` hard-codes are compared against the image that was actually
built, so a layout change cannot leave the code addressing a partition that is no longer
a slot.

It also asserts the thing that actually decides which slot boots, which the first version
of this check got wrong: **the per-slot root lives in `/etc/kernel/cmdline` inside each
slot's rootfs**, not in a `cmdline.txt` on the boot partition. rpi-image-gen assembles it
from the layers' `cmdline.d` fragments
(`image/gpt/ab_userdata/image.d/for/linux-base/cmdline.d/50-slot` sets
`root=/dev/disk/by-slot/active/system`), so both rootfs images must carry that line. Each
slot's `cmdline.txt` was never in this layout.

⚠️ **A build is not a boot, and this check does not change that.** Nothing above proves
the firmware boots the image, that the kernel starts, that systemd runs the health
check, or that `tryboot` falls back. The Pi 5 has no QEMU model, so the Pi path still
has **no end-to-end execution evidence** — that is what the self-test image on real
hardware is for. This closes the gap between "a file was produced" and "the file is the
one the code assumes", and nothing more.

## Building it yourself

```bash
cargo build --release -p morph-boot --target aarch64-unknown-linux-gnu
git clone https://github.com/raspberrypi/rpi-image-gen
(cd rpi-image-gen && sudo ./install_deps.sh)
images/pi5/build.sh rpi-image-gen target/aarch64-unknown-linux-gnu/release/morph-boot
```
