# MorpheOS on a real x86 machine

This is the image a person boots: no self-test driver, nothing that powers itself off, the
Flow shell with Firefox and a Wi-Fi picker on the screen, a root console one key away, and the
tools to drive an A/B update by hand.

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
xz -d disk-plain.img.xz          # 3758 MiB (3.7 GiB) unpacked; the download is ~1.3 GB
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

What should happen: kernel messages on the screen, then the shell's dark background with
**Firefox** on a purple MorpheOS start page. If the machine has no network yet, the **Wi-Fi
picker** opens in front of it: pick your network with the arrow keys, press Enter, type the
password and press Enter (click the picker first, so it has the keyboard). On a cable, or with
no Wi-Fi card, there is nothing to join and it says so in red.

**If the screen stays dark** for more than a minute, the shell could not start on this
machine's graphics: the console comes back on its own (the shell gives the screen back when it
fails), or press **Ctrl+Alt+F2**. Either way the hardware report records why, and a photo of
the screen helps.

The console, on **Ctrl+Alt+F2**, shows

```
MorpheOS: {"booted":"a","default":"a","phase":"committed"}
```

and a root shell, with no password; **Ctrl+Alt+F1** goes back to the shell. The banner line
comes from the machine's own boot state.

⚠️ **Booting a stick made before 2026-10-14?** Those boot to the console only, which is still
the safer first boot on a machine nobody has tried: it works when the graphics do not. A stick
from any newer `images` run has the shell. Booting both on a new machine tells the most.

## 4. Use it

In the shell: **Alt+Tab** switches windows, **Super+F** makes one fullscreen and back, and a
window moves when you drag it by its edge (or anywhere, holding Super). The button with three
bars at the right of the bar along the bottom (or **Super+W**) opens the Wi-Fi picker, to join
another network. Closing Firefox opens it again; there is no launcher yet.

On the console (Ctrl+Alt+F2):

```bash
morph-boot status                  # which slot is booted, which is committed
morph-boot trial b                 # boot slot b once; commit it if it is healthy
```

`trial b` reboots into slot B. Because both slots start as the same system, that on its own
proves the *flow*, not an update — the health check decides, and if it fails the next boot is
back on A. That is the wedge, and it can be watched happening.

**To try a real update**, you need a second stick carrying a payload, made by
`images/x86-qemu/build-payload.sh` on the machine that built this stick (CI does not publish one
yet: doc 20, E8). Its filesystem is labelled `payload`. The system root is read-only, so mount the
stick under `/run`, not `/mnt`:

```bash
mkdir -p /run/morph-media
mount -o ro /dev/disk/by-label/payload /run/morph-media
morph-update --esp /efi --root /dev/disk/by-partlabel/root-<the slot that is NOT booted> \
    --payload /run/morph-media/payload install
morph-boot trial <that slot> --reboot
```

The payload's signature is checked against the keys in the image, then its release number against the
one this stick runs (an older one is refused), then its contents against its own digests, all
before anything is written; the write is read back and hashed, and only then is
the slot allowed to look ready to trial.

⚠️ **The stick accepts only payloads signed by the key it was built with** (doc 20, S2). An image
from CI carries a throwaway test key made for that one build, so a payload from any other build is
refused, as it should be. Signing real releases waits on a release key, which is Rob's to hold.

## 5. Send back what happened

**Leave it running for a minute after the banner**, then shut it down (`poweroff`) or just
switch it off. About 15 seconds after boot it writes a **hardware report** to the stick.

Put the USB stick back in your computer. **The first partition is FAT and opens natively on
Windows and macOS**, and the machine's own record is on it:

```
/morpheos-report/001-<machine>-a/    the hardware report: one folder per boot
    summary.txt                      start here: screens, graphics, Wi-Fi, input, missing firmware
    lspci.txt lsusb.txt dmesg.txt    the raw facts behind it
    net-live.txt                     routes, nameservers, NetworkManager's own view of the link
    processes.txt                    every process: two copies of one client are visible here
/morpheos-report/live-<machine>.txt  a snapshot every 20 seconds while it runs (newest is last)
/loader/loader.conf                  which slot is the default
/morph-boot/history.jsonl            one line per boot: <slot>/<phase>/<action>
/morpheos/a|b/vmlinuz                the kernels
```

**Send back `morpheos-report` whole**, including `live-<machine>.txt`. The report is taken once,
about 15 s into the boot; the interesting failures happen after that — a Wi-Fi join that says it
worked and reaches nothing, a second window from the same program, a browser that exits by
itself. The live file is the rolling record of exactly those, and nothing else keeps it: the
system journal lives in memory and dies with the power.

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

- **Not hardened.** Root, no password, autologin on the console and the serial port, no firewall
  configured. It exists to be driven by hand.
- **Not private.** The hardware report records the machine's model, its devices and their
  MAC addresses on the stick. It records no files, no accounts and nothing from the machine's
  own disk.
- **Not a desktop, yet.** A shell, one browser and a Wi-Fi picker: no launcher, no settings, no
  sound, no suspend (doc 20, M3). Everything runs as root until the account model is decided
  (doc 20, S5). If Firefox crashes, it stays closed until the next boot.
- **Not installable.** It runs from the stick; it does not touch the machine's own disk.
