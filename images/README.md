# Images

| Directory | What |
|---|---|
| `common/overlay` | Files every MorpheOS image gets: the per-boot health check that commits or falls back |
| `common/selftest-overlay` | Test images only: scripted drivers that run the update flow (`test-hook`) or a ransomware attack (`ransomware-test`) by themselves |
| `x86-qemu` | A/B disk for UEFI PCs and VMs (systemd-boot) with Strata watching `/home`, plus the automated update and ransomware tests |
| `pi5` | Raspberry Pi 5 A/B image on rpi-image-gen ([README](pi5/README.md)) |

## The x86 "Try to Break It" test

```bash
cargo build --release -p morph-boot -p strata -p morph-update
sudo apt-get install mmdebstrap qemu-system-x86 ovmf systemd-boot-efi dosfstools mtools fdisk e2fsprogs
sudo images/x86-qemu/test-fallback.sh /tmp/morph-img
sudo images/x86-qemu/test-ransomware.sh /tmp/morph-img
```

The disk has a fourth partition, `data`, shared by both slots. `/home` and the
Strata store (`/var/lib/strata`) are bind mounts from it, and `strata.service`
watches `/home` from the first boot. Because the health check is strict, a Strata
that fails to start makes every boot unhealthy, so the scenarios below also prove
that Strata runs on both slots.

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

### How big the update payload is, and why it matters

`build-payload.sh` shrinks the payload to the filesystem's **used** size (`resize2fs -M`)
before sealing it. The size is the time: `morph-update` reads the payload to hash it,
writes it, and reads the device back to verify it, so the cost is three times the image.

| | payload | `update` install |
|---|---|---|
| before the shrink (run `38117817003`) | 1,258,291,200 B (1200 MiB) | 3.836 s → 11.155 s = **7.32 s** |
| after (run `38118394719`) | 295,141,376 B (295 MB) | 2.478 s → 4.628 s = **2.15 s** |

**3.4× faster**, measured the same way both times — the guest's own kernel timestamps in
`serial-*.log`, not the job's wall clock. The read-back verification is untouched: it still
re-reads and hashes every byte that was written. Shrinking the image is the lever; sampling
the re-read would have been the wrong one.

`heal` is unchanged by this and reports the same `root_bytes: 1258291200`, because it
rebuilds the spare from the **running partition**, which is still the whole slot.

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
