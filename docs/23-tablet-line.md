# 23 · The Slate line: MorpheOS on a tablet

Rob, 2026-10-31: *"i want to build a version of MorpheOS that will go on old ios devices … to be
able to operate the firefox browser on them and do personal tablet computing tasks"*, and then,
when the hardware question resolved: *"This can be put in the MorpheOS repo as another product
line"*. This doc is that line. Decision record: [ADR 0021](adr/0021-slate-line.md).

## 1. What a Slate is

The image built for real hardware — `images/x86-qemu/build.sh <dir> plain`,
[described here](../images/x86-qemu/HARDWARE.md) — running on a slate: a touch screen, a battery, no
keyboard on the desk.

Everything a Slate runs is the image that already ships: the Flow shell, Firefox, the Wi-Fi
picker, a root console on VT2, and the A/B pair with the one-try trial and the fallback. **There
is no second image, no second port and no tablet build.** A Slate is the reference laptop's
software in a different shape, which is the only reason the line is affordable at all.

The line exists because the wedge — *never breaks, and heals itself* — is proven on one kind of
machine today, and a tablet is the second machine a person actually carries. It is not a new
architecture; it is the existing one meeting a touch screen and a battery.

## 2. What hardware can be a Slate

| Requirement | Why | What it rules out |
|---|---|---|
| **x86_64 with UEFI** | the boot path is systemd-boot on an ESP, and there is no CSM fallback | firmware that boots only in legacy BIOS mode |
| **Intel Wi-Fi (`iwlwifi`)** | the picker drives NetworkManager; an unsupported chip means no network on the first boot | Broadcom, Realtek and vendor-bundled chips — a driver bring-up ([19](19-any-hardware.md)), not a first device |
| **Touch through mainline HID** (`i2c-hid` / xHCI) | libinput then sees a touchscreen with no vendor software | vendor-specific touch controllers, which are that same bring-up |
| **4 GB RAM or more, an 8 GB stick** | the image is ~3.3 GiB written to a stick | 2 GB slates and tablets with no USB |
| **Secure Boot can be turned off, and it boots USB** | the image is not Microsoft-signed | locked firmware |

An **Intel Wi-Fi card and a mainline touch controller** are the two choices that decide whether a
slate is an evening's work or a driver project. A used 2-in-1 is the safe class: its keyboard is
also a screen protector, and its touch panel is usually the generic kind.

## 3. What an iPad can never be

The line started as an iPad and the hardware decided against it. Recorded here so it is not
proposed again:

- **checkm8 is the only public bootrom exploit, and it covers A5–A11.** The 2018 iPad Pros are
  **A12X**, above that ceiling, so no bootloader path exists.
- **Asahi Linux** — the only project that boots a custom kernel on Apple silicon — says of
  iPads: *"do not support running custom OS kernels. Due to the system design, even if one were
  to achieve arbitrary code execution in userspace (say, via a jailbreak), that would not help
  with running a Linux kernel."* ([asahilinux.org FAQ](https://asahilinux.org/docs/project/faq/))
- **Project Sandcastle**, the one public bootrom-exploit Linux, supports **no iPad at all** —
  iPhone 7 / 7+ / iPod touch 7 only — and even there GPU, audio, camera and cellular are all
  "No" ([support matrix](https://projectsandcastle.org/status)).
- **A jailbreak does not rescue it either.** Every A12+ jailbreak installs from an app on the
  home screen, and an activation-locked device has no home screen to install it from.

So an iPad is **at most a client**: Morph Link ([16](16-interop-and-link.md), task W5) is the
decided way an iPad participates, and it is not part of this line.

## 4. What the line would prove, in order

1. **E6 first.** The image has booted on one real machine — a 2012 MacBook Pro, from the stick, on
   2026-10-31, with a read-only root, `/persist` mounted and root ssh answering ([HARDWARE.md](../images/x86-qemu/HARDWARE.md)).
   That is the baseline a Slate inherits, and it is more than most lines start with. What has
   **not** happened is a boot on a *tablet*: its touch panel, its Wi-Fi chip and its firmware are
   all untried, and a laptop boot does not speak for any of them.
2. **Then the touch path:** touch input into the compositor, an on-screen keyboard, rotation, and
   suspend/resume. None of these are exercised by a keyboard-and-mouse boot, so none of them are
   proven by it.
3. **Then the personal-computing claim:** E5's Wi-Fi and E4's browser, joined on a touch device.

## 5. The mesh role

A Slate is a mesh node while it is switched on: it pulls its inbox and runs jobs like any other
machine. Its capacity is its `probe-node.sh` card and nothing else — until it has been probed it
declares no capability, and it never carries a merge gate, because a tablet that sleeps is not a
guarantee.

## 6. Open questions

1. **Which device first?** The criteria in §2, not a model number. Buy with a return window.
2. **Run from the stick, or install to internal storage?** The stick is reversible and costs the
   device nothing; E7 is the internal path and is not built yet.
3. **Does the compositor need a virtual keyboard before a Slate is usable away from a desk?** If
   it does, that work is not in this line as planned and needs its own task.
4. **Is a Slate a scaling profile of its own** ([07](07-scaling-profiles.md)), or Desk with a
   touch screen and a battery?

## 7. The arm64 image, as built and booted (2026-11-01)

Why arm64 at all: no iOS device can boot a kernel (§3), so the only way MorpheOS runs **on** an
iPad is as a local VM — and that VM is **aarch64**. The x86 `plain` image cannot be that guest.
What follows is the order that worked, on an x86 host with `qemu-system-aarch64`, recorded so it can
be repeated rather than rediscovered.

1. **Root filesystem** — `mmdebstrap --arch=arm64 --variant=apt`, noble, from
   `ports.ubuntu.com/ubuntu-ports`, including `linux-image-generic`, `systemd`, `udev`, `seatd`
   (the shell's seat), `openssh-server` and the pixman/wayland/XKB/input runtime libraries.
2. **`linux-modules-extra-<version>`** — the `virtio_gpu` module lives there and **not** in
   `linux-image-generic`'s own set. Without it `/dev/dri` never appears and the compositor dies with
   `reading /dev/dri: No such file or directory`. This is the same package the x86 build installs
   with its `MODULES_EXTRA_HOOK`.
3. **Mask `apparmor` and `snapd`** — neither belongs in an appliance guest, and both stall a boot
   under emulation for minutes.
4. **The binaries** — cross-built, not built on the guest: `cargo build --release --target
   aarch64-unknown-linux-gnu` with `PKG_CONFIG_SYSROOT_DIR=<rootfs>`,
   `PKG_CONFIG_LIBDIR=<rootfs>/usr/lib/aarch64-linux-gnu/pkgconfig`, and
   `RUSTFLAGS="-C link-arg=--sysroot=<rootfs>"`. Two traps cost an evening each: the target must be
   added to the **pinned** toolchain in `rust-toolchain.toml` (not the default one), and the linker
   needs `--sysroot` because Debian's `libm.so` is a linker script pointing at absolute paths.
   Result: `flow-compositor`, 3.9 MB, aarch64.
5. **The units** — `flow-shell.service` unchanged from the x86 image, plus a `flow-browser.service`
   that runs `firefox --kiosk` on `WAYLAND_DISPLAY=wayland-0`. That pair is the whole browser
   appliance.
6. **The disk** — GPT with a 300 MiB ESP and a root labelled `root`, so `root=PARTLABEL=root`
   resolves. The ESP carries `EFI/BOOT/BOOTAA64.EFI` and a `grub.cfg` that loads the kernel and
   initrd **from the ESP**.

**Two arm64-specific findings.**

- **Ubuntu ships no `systemd-boot-efi` for arm64** — it is amd64-only there. The loader is therefore
  **GRUB's** arm64 monolithic EFI (`grubaa64.efi` from `grub-efi-arm64-bin`), a deviation from the
  x86 image that belongs in the image build when this becomes a real target.
- **Ubuntu's `firefox` beats Mozilla's on epoch.** `1:1snap1-0ubuntu5` is a transitional package
  whose `/usr/bin/firefox` prints *"snap install firefox"*. An apt pin —
  `Pin: origin packages.mozilla.org`, `Pin-Priority: 1000` — installs the real browser:
  **Mozilla Firefox 159.0, arm64**. The x86 build documents this trap in a comment; it caught an
  agent that had read the comment anyway, which is the argument for a check rather than a comment.

**Verified, in this order**, booting that disk under `QEMU_EFI.fd`:

```
EFI/BOOT/BOOTAA64.EFI -> GRUB -> kernel -> systemd-remount-fs, local-fs.target
flow-compositor: showing on /dev/dri/card0 at 1280x800
flow-compositor: first frame on the display; ready
flow-compositor: splash done
flow-browser.service started        ->  1 window(s) on screen
```

**Not verified, and not claimed:** that Firefox *paints* on arm64. Under TCG the window attaches but
the captured frame stays background plus the probe window's colour. The x86 browser test runs on KVM
and never on TCG, and the same limit applies here: this needs **native arm64** — the iPad through
UTM once the lock opens, or any arm64 board with UEFI firmware, which can boot this exact disk.
