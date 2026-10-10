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

Each scenario builds a disk with slots A and B, boots it in QEMU, and lets the
image run an update into slot B by itself:

| Scenario | Slot B "update" | Expected |
|---|---|---|
| `good` | Healthy | Trial, committed, still B after a reboot |
| `unhealthy` | A required service fails | Trial, health check fails, back on A, A stays default |
| `crash` | Its init dies, so the kernel panics | Trial, panic and reset, back on A, A stays default |
