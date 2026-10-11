# Images

| Directory | What |
|---|---|
| `common/overlay` | Files every MorpheOS image gets: the per-boot health check that commits or falls back |
| `common/selftest-overlay` | Test images only: a scripted driver that runs the update flow by itself |
| `x86-qemu` | A/B disk for UEFI PCs and VMs (systemd-boot), plus the automated fallback test |
| `pi5` | Raspberry Pi 5 A/B image on rpi-image-gen ([README](pi5/README.md)) |

## The x86 "Try to Break It" test

```bash
cargo build --release -p morph-boot
sudo apt-get install mmdebstrap qemu-system-x86 ovmf systemd-boot-efi dosfstools mtools fdisk e2fsprogs
sudo images/x86-qemu/test-fallback.sh /tmp/morph-img
```

Each scenario builds a disk with slots A and B, boots it in QEMU, and lets the image
run the A/B flow by itself:

| Scenario | What slot B holds before the run | Expected |
|---|---|---|
| `good` | A healthy system, put there by `build.sh` | Trial, committed, still B after a reboot |
| `unhealthy` | As `good`, but a required service fails | Trial, health check fails, back on A, A stays default |
| `crash` | As `good`, but its init dies, so the kernel panics | Trial, panic and reset, back on A, A stays default |
| `update` | A healthy system — and the run **replaces it first**: the driver installs a sealed payload from a second read-only disk with `morph-update`, then trials what it wrote | Trial, committed, still B; **and slot B's root carries the payload's marker, which it did not before** |
| `heal` | The **`unhealthy`** disk: a required service fails — and after the fallback the driver **rebuilds the broken spare from the running slot** with `morph-heal`, then trials it again | The `unhealthy` boots, then a trial of the rebuilt slot, committed, still B; **and the broken service is gone from slot B's root** |

⚠️ **Only `update` performs an update, and until it existed none of them did.** Every
image built here makes both slots from **one** image — `boot_a`/`boot_b` from one
`boot.vfat`, `system_a`/`system_b` from one `system.erofs` — so `trial b` booted a
byte-identical copy of the system already running. The first three scenarios exercise the
boot policy faithfully and say nothing whatever about updating a machine; `update` is the
one that writes a slot on a running system, verifies the write, and boots it.
