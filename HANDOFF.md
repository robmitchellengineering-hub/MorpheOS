# HANDOFF

From **2026-10-16** this repository is built by a DeepSeek agent on Rob's Mac. Until then it was
built by Claude Code. This file is how the work continues without anyone who was here before.

**Read in this order:** [AGENTS.md](AGENTS.md) (the rules, and the hazards: mistakes already
made here), this file, [doc 00](docs/00-strategy.md) (what wins arguments), then
[ADR 0014](docs/adr/0014-express-road-first.md) (what we are building next and why).

## The goal, in one line

**A general user boots a laptop, gets online, browses, and cannot make the machine
unbootable** (ADR 0014's exit). Everything below serves that. Anything that doesn't goes in
the widening backlog (ADR 0014, lane 2) and waits.

## Where things stand (2026-10-13)

| Piece | State | Evidence |
|---|---|---|
| A/B boot, one-shot trial, fallback (`morph-boot`) | **VM-proven** | `images` workflow: good / unhealthy / crash scenarios |
| Write + verify a new system into the spare slot (`morph-update`) | **VM-proven** | `update` scenario: slot B carries the payload's marker after boot |
| Rebuild the spare after a failed trial (`morph-heal`, H3/H4 groundwork) | **VM-proven** | ADR 0009; `images` workflow |
| Transactions that roll themselves back (`morph-transaction`, `morph-graph`) | **tested, CLI-proven** | `scripts/verify-transaction-cli.sh`, `verify-apply-cli.sh` |
| Manifest, invariants, risk tiers, signed generations (`.rob`) | **tested** | `morph-manifest` tests; ADRs 0005, 0011, 0012 |
| Strata: versions, rewind, ransomware detection, in the x86 image | **VM-proven** | `test-ransomware.sh`: 24 files back byte-for-byte |
| Flow shell: frame, input, session model, two feels | **software-rendered only** | CI renders `frame.png` and fails if blank. **No Wayland yet** |
| Bootable x86 USB image for a person | **builds, boots in QEMU** | [HARDWARE.md](images/x86-qemu/HARDWARE.md). **Never booted on real hardware** |
| Pi 5 image | **builds and is read back** | `verify-pi-image.mjs`. **Never booted on a Pi** |

"VM-proven" means a CI job boots it and reads the verdict off the disk. It does **not** mean
it has run on real hardware. Say "builds" or "VM-proven", never "works", until a person has
run it on a machine (AGENTS.md, verification bar).

## How to work (every task, every time)

1. `git fetch origin && git switch -c <type>/<slug> origin/main`. Never commit to `main`.
2. Take the **lowest-numbered open task** below whose "Needs" are met. One task per branch.
3. **Write the check first.** Every task's "Done when" names something a machine verifies.
   Make that check exist and fail *before* writing the code that makes it pass. If you cannot
   make it fail, it does not test anything (AGENTS.md H3, H9).
4. Implement the smallest change that makes it pass.
5. `bash scripts/check.sh`. It must end with `all checks passed`.
6. Push, open a pull request, and read **every** check by name, including the `images`
   workflow (AGENTS.md H6, H10). If a VM job times out, download the serial-log artifact and
   read it before forming any theory (H11).
7. In the same pull request: tick the task below, link the evidence (CI run or artifact), and
   add one line to the **Session log** at the bottom.
8. Rob merges. Do not merge your own pull requests.

**Never:** weaken, skip or delete a test to get green; claim something works without
evidence; rename a required CI job without saying so in the PR (H10); edit an accepted ADR
(write a new one that supersedes it).

**When stuck for more than two attempts:** stop. Write what you tried and what you saw under
**Open questions** below, push it, and ask Rob. A clear question beats a third guess.

## Decisions that are Rob's, not yours

Write the question up (options and a recommendation) and wait:
- merging any pull request;
- anything that changes the security model: signing, the `.rob` mark (ADRs 0011, 0012), keys,
  what runs as root;
- copying code from Morpheus (licence, decided file by file: [doc 17](docs/17-morpheus-family.md));
- which laptop is the reference machine, and anything that costs money;
- dropping or reordering a task in the express road.

## The express road

Each task: what, why, the check that proves it, and engineering notes. Notes marked ⚠️ are
traps that are easy to fall into.

### E1 · The shell becomes a real Wayland compositor, nested · open

**What:** a Linux-only binary in `flow-shell` (behind a cargo feature, so the crate still
builds everywhere) that is a Wayland compositor built on Smithay. It shows clients' windows,
and the shell's own frame painted by the code that exists today.

**Done when:** a CI job starts the compositor **headless**, starts a test client that maps
one `xdg_toplevel` filled with a known colour, captures a frame, and asserts that colour is on
screen. It must fail with the client removed. That is the repo's rule: "CI proves a frame, not
a unit" (ADR 0010).

**Notes:**
- Start from Smithay's **`anvil`** example compositor. It is the reference implementation of
  everything below. Strip it down; don't write protocol handling from scratch.
- Protocols a browser needs, at minimum: `wl_compositor`, `wl_subcompositor`, `wl_shm`,
  `linux-dmabuf`, `xdg_shell`, `wl_seat` (keyboard and pointer), `wl_output`,
  `wl_data_device` (clipboard), `xdg-decoration` (prefer server-side). Add `text-input` and
  `viewporter` when a client asks for them.
- Backends: **winit** to run nested inside a desktop while developing; **headless with the
  pixman (software) renderer** for CI, because CI runners have no GPU. Check the current
  Smithay release for what each backend is called. The API moves between versions, so pin
  the version.
- Keep the existing software painter (`tiles.rs`, `shelf.rs`, `terminal.rs`) and upload its
  output as a texture for the shell's own chrome. Don't rewrite what is already tested.
- The test client: a small Rust binary using `wayland-client` / `smithay-client-toolkit`
  that maps one `xdg_toplevel` with an `shm` buffer of a solid colour.

### E2 · Classic windows · open · needs E1

**What:** the Classic feel (ADR 0013, item 3) manages real windows: focus follows click,
move, close, fullscreen, and a keyboard shortcut to switch windows.

**Done when:** in the headless CI test, two clients map; a scripted click focuses the second;
a scripted close removes it; frames are asserted after each step.

### E3 · The compositor on bare metal (DRM/KMS) inside the image · open · needs E1

**What:** the udev backend (DRM/KMS output, libinput input, `libseat` session) and a systemd
unit that starts the compositor on tty1 at boot, as a normal user, not root.

**Done when:** the `images` workflow boots the image in QEMU with `-device virtio-gpu-pci`,
takes a QEMU `screendump`, and asserts the shell's frame is in it, not a text console.

**Notes:**
- ⚠️ The compositor must not become a reason the machine "breaks". If it fails to start,
  the health check (`boot-check`) must see a failed unit, so a bad update that breaks the
  shell **falls back** like any other. Add that as an `unhealthy`-style scenario.
- Keep the root shell on another VT (Ctrl+Alt+F2) for recovery. It is how a person fixes
  the machine when the shell is wrong.
- Create a real user in the image. Running a browser as root is a security decision
  (Rob's, above). Don't make it by default.

### E4 · A browser · open · needs E3

**What:** Firefox in the image, running natively on Wayland under the compositor.

**Done when:** the QEMU test launches Firefox on a local `file://` page made of large blocks
of known colours, and the screendump contains them.

**Notes:**
- ⚠️ **On Ubuntu 24.04, `apt install firefox` installs a snap**, and snaps do not run without
  `snapd`, which this image does not have and should not. Install the `.deb` from Mozilla's
  own APT repository (packages.mozilla.org), pinned. The same is true of Chromium.
- Run it with `MOZ_ENABLE_WAYLAND=1`. It needs fonts (`fonts-dejavu-core` at least), a D-Bus
  session bus, and a writable home on the data partition, which is where Strata already watches.
- The image grows a lot. Check the slot size (`ROOT_MB` in `build.sh`) and the update payload
  size (images README, "How big the update payload is"). Raise them deliberately and record
  the numbers.

### E5 · Networking · open · needs E3

**What:** wired and wireless networking with a way to pick a Wi-Fi network from the shell.

**Done when:** in QEMU (user-mode networking), the image gets an address by DHCP and Firefox
loads a page served by the CI host. Wi-Fi itself can only be proven on hardware (E6).

**Notes:** NetworkManager is the common choice and has a D-Bus API the shell can drive later;
`iwd` plus `systemd-networkd` is smaller. Pick one, write a short ADR, and don't run both.
Wi-Fi needs `linux-firmware` (large); include only the firmware families the reference
laptop needs once it is known.

### E6 · First boot on Rob's laptop · open · needs Rob, best after E4

**What:** Rob writes the USB image and boots it, following [HARDWARE.md](images/x86-qemu/HARDWARE.md).

**Done when:** the report HARDWARE.md asks for is in `docs/hardware/<machine>.md`: what booted,
what didn't, `lspci`/`lsusb` output, and whether Wi-Fi, keyboard, touchpad and display worked.
Every failure becomes its own task here.

### E7 · Install to the internal disk · open · needs E6

**What:** an installer that partitions the internal disk (ESP, slot A, slot B, data), writes
the running system into both slots with `morph-update`'s verified write, and makes the disk
bootable. Without it, a person runs from the USB forever.

**Done when:** a QEMU scenario boots from a USB-like second disk, installs to the first, powers
off, boots the first disk alone, and the boot history shows a committed slot. ⚠️ An installer
destroys a disk: it must refuse to touch the disk it booted from, and ask before wiping.

### E8 · Try to break it, on the laptop · open · needs E6, E7

**What:** Rob runs the "Try to Break It" steps that the VM suite covers (bad update, crashed
update, ransomware, the spare rebuilt after a failure) on the real laptop.

**Done when:** each result is written up in `docs/hardware/`, and the express road's exit is
met or the gaps are tasks here.

## Open questions

Write here when stuck: the date, what you tried, what you saw, what you need.

- *(none yet)*

## Session log

One line per piece of work: date, who, what, what's next.

- 2026-10-13 · Claude Code · Wrote this file and `scripts/check.sh` (the whole local bar in one
  command). Next: E1.
