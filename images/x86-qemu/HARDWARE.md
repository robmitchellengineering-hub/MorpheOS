# MorpheOS on a real x86 machine

This is the image a person boots: no self-test driver, nothing that powers itself off, a root
shell on the local console, and the tools to drive an A/B update by hand.

> ⚠️ **It has never been booted on real hardware.** Every x86 test so far has run in QEMU, which
> presents virtio devices and a UEFI firmware that is not the one in your laptop. The image
> asserts at build time that the drivers it needs are present (USB storage, xHCI, AHCI, ext4,
> VFAT, and the laptop graphics, Wi-Fi, touchscreen and wired-network drivers: AGENTS.md H18)
> and CI boots it and reaches a login, but "boots in QEMU" is not "boots on your machine".
> The first power-on is the experiment, and this file says what to send back from it.

## What you need

- An **x86_64 machine with UEFI** (any Windows 8-era laptop or later). **Legacy BIOS is not
  supported** — the boot path is systemd-boot on an ESP, and there is no CSM fallback.
- A USB stick, 8 GB or larger. **Everything on it is destroyed.**
- A screen and keyboard for the machine, at least once, to select the USB at power-on.
- A computer to write the stick with.

## 0. Before you start, on a Windows machine: your BitLocker recovery key

⚠️ **Read this before changing any firmware setting on a Windows laptop.** Step 3 turns Secure
Boot off. On a laptop whose disk is encrypted with BitLocker (most Windows 11 laptops are,
often without saying so), Windows notices the change and asks for the **BitLocker recovery
key** the next time it starts. Without the key, Windows and everything on it stay locked.

So first, in Windows, open **Settings → Privacy & security → Device encryption** (or search
for "Manage BitLocker"). If encryption is on, get the recovery key and keep it somewhere that is
not that laptop: it is in the Microsoft account the laptop is signed into, at
https://aka.ms/myrecoverykey, or **Back up your recovery key** in the BitLocker settings.

Turning Secure Boot back on after the MorpheOS test usually makes Windows start normally
again; the key is for when it does not. The MorpheOS stick itself never touches the laptop's
own disk.

## 1. Get the image

Download `morpheos-x86-usb` from the newest green `images` run on `main` (you need to be
signed in to GitHub; each download is kept for 30 days):

https://github.com/robmitchellengineering-hub/MorpheOS/actions/workflows/images.yml

Open the newest run with a green tick, scroll to **Artifacts**, and download
`morpheos-x86-usb`. It is a zip containing `disk-plain.img.xz`.

## 2. Write it to the stick

### On Windows: balenaEtcher

1. Unzip `morpheos-x86-usb.zip` (right-click, **Extract All**). Leave `disk-plain.img.xz` as
   it is: Etcher unpacks it itself.
2. Install balenaEtcher (https://etcher.balena.io), open it, **Flash from file**, and choose
   `disk-plain.img.xz`.
3. **Select target**: the USB stick. **Check the size** against your stick: this is the step
   where the wrong drive costs you a drive. Etcher hides the system disk, but not other USB
   drives.
4. **Flash!**, and let it finish its verification pass.

Windows may then say the stick needs formatting, or show several small drives: **do not
format anything**. That is Windows seeing Linux partitions; the first one (FAT) is readable.

(Rufus works too: choose the unpacked `disk-plain.img` and, when asked, **DD Image mode**.
Its default, ISO mode, does not apply to this image.)

### On macOS: dd

Unpack it twice, since the artifact is a zip containing an `.xz`:

```bash
unzip morpheos-x86-usb.zip
xz -d disk-plain.img.xz          # ~4.5 GiB when unpacked
```

Find the stick first. **This is the step where the wrong disk number costs you a drive**, so
check the size:

```bash
diskutil list
```

Then, with `N` from that listing (`/dev/disk4` → `rdisk4`; the `r` device is much faster):

```bash
diskutil unmountDisk /dev/diskN
sudo dd if=disk-plain.img of=/dev/rdiskN bs=4m
```

## 3. Boot it

Pick the USB stick from the machine's boot menu, **in UEFI mode**. The image is not signed
for Secure Boot yet, so Secure Boot must be off (on Windows machines, read step 0 first).

| Machine | Boot menu | Secure Boot |
|---|---|---|
| **The Windows laptop** (`win`) | Hold Shift while clicking **Restart** in Windows, then **Use a device → the USB stick**. Or power on pressing the maker's boot key: F12 (Lenovo, Dell, Acer), F9 (HP), Esc or F8 (ASUS). | In the firmware setup (F2 or Del at power-on, or Shift+Restart → **Troubleshoot → UEFI Firmware Settings**): **Security → Secure Boot → Disabled**. |
| **Lenovo Legion Go** (`win-2`) | Power off. Hold **Volume Up** and press **Power**: the Novo menu opens; choose **Boot Menu** and the stick. It needs a USB-C hub for the stick and a keyboard. | In the same Novo menu, **BIOS Setup → Security → Secure Boot → Disabled**. |
| **The 2015 Mac** (work) | Power on holding **Option (⌥)** until the disk icons appear; choose the one labelled **EFI Boot**. | A 2015 Mac has no Secure Boot to turn off. |

Expect two things that are not failures:
- **Legion Go:** its built-in screen is a portrait panel, so the text console can come up
  sideways. The report still records it.
- **The 2015 Mac:** its Wi-Fi is Broadcom. A 2015 MacBook Pro usually has a BCM43602, which
  the image's open driver (`brcmfmac`) supports. A 2015 MacBook Air usually has a BCM4360,
  for which Linux has no open-source driver (only Broadcom's own `wl`, which this image does
  not carry), so the report will list the card with no driver bound. Either way the report
  answers "does its Wi-Fi work"; an unclaimed card is not a broken boot.

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
- **Not a desktop, yet.** The Flow shell, Firefox and the Wi-Fi picker are built and tested in
  VMs (the `shell`, `browser` and `wifi` images), but this image boots to a root console on
  purpose: on a machine nobody has booted before, a console is what still works when the
  graphics do not. It joins the shell once the hardware reports are in (doc 20, E4 and E6).
- **Not installable.** It runs from the stick; it does not touch the machine's own disk.
