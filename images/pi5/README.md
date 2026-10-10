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

## Building it yourself

```bash
cargo build --release -p morph-boot --target aarch64-unknown-linux-gnu
git clone https://github.com/raspberrypi/rpi-image-gen
(cd rpi-image-gen && sudo ./install_deps.sh)
images/pi5/build.sh rpi-image-gen target/aarch64-unknown-linux-gnu/release/morph-boot
```
