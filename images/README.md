# Images

| Directory | What |
|---|---|
| `common/overlay` | Files every MorpheOS image gets: the per-boot health check that commits or falls back |
| `common/selftest-overlay` | Test images only: scripted drivers that run the update flow (`test-hook`) or a ransomware attack (`ransomware-test`) by themselves |
| `x86-qemu` | A/B disk for UEFI PCs and VMs (systemd-boot) with Strata watching `/home`, plus the automated update and ransomware tests |
| `pi5` | Raspberry Pi 5 A/B image on rpi-image-gen ([README](pi5/README.md)) |

## The x86 "Try to Break It" test

```bash
cargo build --release -p morph-boot -p strata
sudo apt-get install mmdebstrap qemu-system-x86 ovmf systemd-boot-efi dosfstools mtools fdisk e2fsprogs
sudo images/x86-qemu/test-fallback.sh /tmp/morph-img
sudo images/x86-qemu/test-ransomware.sh /tmp/morph-img
```

The disk has a fourth partition, `data`, shared by both slots. `/home` and the
Strata store (`/var/lib/strata`) are bind mounts from it, and `strata.service`
watches `/home` from the first boot. Because the health check is strict, a Strata
that fails to start makes every boot unhealthy, so the update scenarios below
also prove that Strata runs on both slots.

Each scenario builds a disk with slots A and B, boots it in QEMU, and lets the
image run an update into slot B by itself:

| Scenario | Slot B "update" | Expected |
|---|---|---|
| `good` | Healthy | Trial, committed, still B after a reboot |
| `unhealthy` | A required service fails | Trial, health check fails, back on A, A stays default |
| `crash` | Its init dies, so the kernel panics | Trial, panic and reset, back on A, A stays default |

### Ransomware

`test-ransomware.sh` boots slot A only, with no update. The self-test writes 24
documents and waits for Strata to record them. It then encrypts half of them in
place, replaces the other half with `.locked` copies, and leaves a ransom note.
After a reboot, it rewinds each folder named in Strata's alert to the point that
alert gives.

The verdict is read from the disk after the VM powers off: the alert in the
store, the rewind point (after the documents were complete), every document
matching its checksum from before the attack, no `.locked` file or ransom note
left, and the attacker's files in quarantine rather than deleted.
