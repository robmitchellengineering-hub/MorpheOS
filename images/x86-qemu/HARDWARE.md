# MorpheOS on a real x86 machine

This is the image a person boots: no self-test driver, nothing that powers itself off, a root
shell on the local console, and the tools to drive an A/B update by hand.

> ⚠️ **It has never been booted on real hardware.** Every x86 test so far has run in QEMU, which
> presents virtio devices and a UEFI firmware that is not the one in your laptop. The image
> asserts at build time that the drivers it needs are present (USB storage, xHCI, AHCI, ext4,
> VFAT) and CI boots it and reaches a login — but "boots in QEMU" is not "boots on your machine".
> The first power-on is the experiment, and this file says what to send back from it.

## What you need

- An **x86_64 machine with UEFI** (any Windows 8-era laptop or later). **Legacy BIOS is not
  supported** — the boot path is systemd-boot on an ESP, and there is no CSM fallback.
- A USB stick, 8 GB or larger. **Everything on it is destroyed.**
- A screen and keyboard for the machine, at least once, to select the USB at power-on.
- A computer to write the stick with.

## 1. Get the image

Download `morpheos-x86-usb` from the newest green `images` run on `main`:

https://github.com/robmitchellengineering-hub/MorpheOS/actions/workflows/images.yml

Unpack it twice — the artifact is a zip containing an `.xz`:

```bash
unzip morpheos-x86-usb.zip
xz -d disk-plain.img.xz          # ~4.5 GiB when unpacked
```

## 2. Write it to the stick

On macOS, find the stick first — **this is the step where the wrong disk number costs you a
drive**, so check the size:

```bash
diskutil list
```

Then, with `N` from that listing (`/dev/disk4` → `rdisk4`; the `r` device is much faster):

```bash
diskutil unmountDisk /dev/diskN
sudo dd if=disk-plain.img of=/dev/rdiskN bs=4m
```

## 3. Boot it

Power on, press the boot-menu key (F12, F2 or Esc depending on the machine), and pick the USB
stick **in UEFI mode**. If the machine has Secure Boot on, turn it off.

What should happen: kernel messages on the screen, then

```
MorpheOS: {"booted":"a","default":"a","phase":"committed"}
```

and a root shell, with no password. The banner line comes from the machine's own boot state.

## 4. Use it

```bash
morph-boot status                  # which slot is booted, which is committed
morph-boot trial b                 # boot slot b once; commit it if it is healthy
```

`trial b` reboots into slot B. Because both slots start as the same system, that on its own
proves the *flow*, not an update — the health check decides, and if it fails the next boot is
back on A. That is the wedge, and it can be watched happening.

**To try a real update**, you need a second stick carrying the payload. It is built by CI too,
from `images/x86-qemu/build-payload.sh`; the tool finds it by its filesystem label:

```bash
morph-update --esp /efi --root /dev/disk/by-partlabel/root-<the slot that is NOT booted> \
    --payload /mnt/payload install
```

The payload is verified before anything is written, the write is read back and hashed, and only
then is the slot allowed to look ready to trial.

## 5. Send back what happened

Put the USB stick back in your computer. **The first partition is FAT and mounts natively on
macOS**, and the machine's own record is on it:

```
/loader/loader.conf                  which slot is the default
/morph-boot/history.jsonl            one line per boot: <slot>/<phase>/<action>
/morpheos/a|b/vmlinuz                the kernels
```

`history.jsonl` is the verdict, and it is written by the machine rather than by a log:

| line | what it means |
|---|---|
| `a/committed/none` | slot A is committed and nothing was in flight |
| `b/trial/committed` | slot B booted as a trial and its health check passed, so it was committed |
| `b/trial/fallback` | slot B booted as a trial, its health check failed, and the next boot went back to A |

Send `history.jsonl`, plus a photo of the screen if anything looked wrong. If it does not reach
a shell at all, the screen is the whole diagnosis.

## What this image is not

- **Not hardened.** Root, no password, autologin on the console and the serial port, no network
  configured. It exists to be driven by hand.
- **Not a desktop.** There is no graphical session, no browser and no session model — this is the
  A/B update and self-heal behaviour, which is the part of MorpheOS that is built.
- **Not installable.** It runs from the stick; it does not touch the machine's own disk.
