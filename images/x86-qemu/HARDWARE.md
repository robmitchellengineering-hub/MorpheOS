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

**Leave it running for a minute after the banner**, then shut it down (`poweroff`) or just
switch it off. About 15 seconds after boot it writes a **hardware report** to the stick.

Put the USB stick back in your computer. **The first partition is FAT and opens natively on
Windows and macOS**, and the machine's own record is on it:

```
/morpheos-report/001-<machine>-a/    the hardware report: one folder per boot
    summary.txt                      start here: screens, graphics, Wi-Fi, input, missing firmware
    lspci.txt lsusb.txt dmesg.txt    the raw facts behind it
/loader/loader.conf                  which slot is the default
/morph-boot/history.jsonl            one line per boot: <slot>/<phase>/<action>
/morpheos/a|b/vmlinuz                the kernels
```

**Booting the same stick on several machines collects all of them**: each boot adds a
numbered folder (`001-…`, `002-…`), named after the machine's model. That is how the reference
laptop gets chosen (doc 20, E6): boot it on each candidate, then send the whole
`morpheos-report` folder.

In `summary.txt`, the two sections that matter most on a first boot:
- **SCREENS**: which outputs the graphics found, and which is `connected`. A laptop whose
  internal panel is broken still lists it; an external monitor should appear as connected.
- **FIRMWARE THE KERNEL ASKED FOR AND DID NOT FIND**: anything here is a device that will not
  work until its firmware is added. Wi-Fi is the usual one.

`history.jsonl` is the verdict, and it is written by the machine rather than by a log:

| line | what it means |
|---|---|
| `a/committed/none` | slot A is committed and nothing was in flight |
| `b/trial/committed` | slot B booted as a trial and its health check passed, so it was committed |
| `b/trial/fallback` | slot B booted as a trial, its health check failed, and the next boot went back to A |

Send the `morpheos-report` folder and `history.jsonl`, plus a photo of the screen if anything
looked wrong. If it does not reach a shell at all, there is no report: the screen is the whole
diagnosis, so photograph the last lines.

## What this image is not

- **Not hardened.** Root, no password, autologin on the console and the serial port, no network
  configured. It exists to be driven by hand.
- **Not private.** The hardware report records the machine's model, its devices and their
  MAC addresses on the stick. It records no files, no accounts and nothing from the machine's
  own disk.
- **Not a desktop.** There is no graphical session, no browser and no session model — this is the
  A/B update and self-heal behaviour, which is the part of MorpheOS that is built.
- **Not installable.** It runs from the stick; it does not touch the machine's own disk.
