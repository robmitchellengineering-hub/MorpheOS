> Persisted from the planning room 2026-11-08 so the plan is git-tracked, not lost.

# Booting MorpheOS's own kernel on morpheos-a-1 — the Phase 0 runbook

The claim this settles: **our kernel boots the real machine with its Wi-Fi (b43), its audio
(snd-hda-intel) and its panel (i915) working.** Phase 0 has *failed* if any of the three is lost. The
everyday image keeps Ubuntu's kernel in slot B, so a bad boot is a comparison, not a brick.

Order matters: nothing here starts until the VM proof is green (image boots our kernel, `test-shell` and
`test-wifi` pass on it), because a machine is the expensive place to find a bug.

## 1. Build the image (me, CI)

The workflow is dispatch-only: `.github/workflows/secure-image.yml` in the MorpheOS repo.

```
gh workflow run secure-image.yml --ref feat/k1-image-kernel
```

It builds our kernel, builds the `plain` image with it in slot A, **reads the finished disk back** and
refuses to publish unless slot A carries `6.8.0-morpheos` *and slot B does not*. The artefact is
`morpheos-x86-usb-ours-kernel` (`disk-plain.img.xz` + the kernel's `build-report.txt`).

## 2. Fetch and check it (me, on the Mac)

```
gh run download <run-id> -n morpheos-x86-usb-ours-kernel -D ~/morpheos-usb
xz -dk ~/morpheos-usb/disk-plain.img.xz        # ~3.7 GiB raw
shasum -a 256 ~/morpheos-usb/disk-plain.img
```

and read the two things that matter out of the raw image, without flashing anything:

```
mtype -i ~/morpheos-usb/disk-plain.img@@1M ::/loader/entries/morpheos-a.conf
```

## 3. Flash a USB stick (needs you, and needs a yes)

This is destructive to whatever is on the stick, so it does not happen without an explicit yes and an
identified device:

1. Plug the stick in.
2. `diskutil list external` — the stick's disk identifier (e.g. `disk4`), never `disk0`.
3. Confirm the name and size match the stick you just plugged in.
4. Then: `diskutil unmountDisk /dev/disk4` and
   `sudo dd if=~/morpheos-usb/disk-plain.img of=/dev/rdisk4 bs=4m status=progress`.

I will not run step 4 against a device I have not shown you by name and size first.

## 4. Boot the machine (you, at the laptop)

1. Stick in, power on, hold the boot menu key (this machine: **Option/Alt** for the Apple picker, or its
   firmware's boot menu).
2. Pick the USB device.
3. It should reach the Flow shell.

## 5. What to look at — the falsifier, in order

| # | check | how | fails if |
|---|---|---|---|
| 1 | **the kernel is ours** | `uname -s`, `uname -r` in a terminal (the test image prints `MORPHEOS-KERNEL uname-s:` on the serial console) | not `MorpheOS` / not `6.8.0-morpheos` |
| 2 | **the panel** | the screen, from the kernel's own console messages through to the compositor | black screen, or the compositor never draws |
| 3 | **Wi-Fi** | the picker lists networks and joins | no networks listed; `dmesg | grep b43` shows the firmware failing |
| 4 | **audio** | the image's sound test / `aplay -l` saw a card; the volume control moves | no card, or silence with a card |
| 5 | **the comparison still boots** | next boot, choose slot B in the boot menu | slot B does not boot — then the arm has cost us the fallback |

If 2, 3 or 4 fails, the run is a **negative result and worth having**: the kernel config is wrong for
this machine, and the log says which driver. That is what the falsifier is for.

## 6. Roll back

Pick slot B (Ubuntu's kernel) in the boot menu, or reflash the last published everyday image. Nothing on
the machine's data partition is touched by this test.

## What must not happen
- Flashing over a device that was not named and sized back to you first.
- Claiming the hardware test passed because the VM passed. The VM proves the image; only the machine
  proves the machine.
