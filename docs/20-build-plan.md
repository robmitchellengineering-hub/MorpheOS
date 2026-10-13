# 20 · Build plan: from here to a finished MorpheOS

This is **the only task list**. If work is not a task here, it is not on the plan. The vision
lives in docs 00–19, the decisions in the [ADRs](adr/README.md), and the order in
[ADR 0014](adr/0014-express-road-first.md). This doc turns all three into tasks a builder can
pick up without having to guess. `scripts/verify-plan.mjs` checks it in CI.

## How this doc works

- **A task** is a heading `### <ID> · <title> · <status>`, optionally followed by
  `· needs <ID>, <ID>`. It always has a **Done when:** line: something a machine or a person
  can check, never "it works". A `done` task also has an **Evidence:** line: a pull request,
  a CI run, an artifact or a file in this repository.
- **Statuses:** `proposed` (an idea, not approved), `open`, `in progress`, `blocked`, `done`.
  Only Rob turns `proposed` into `open`.
- **Branches carry the task ID** (`feat/e2-classic-windows`). CI refuses a pull request whose
  branch names no open task. The exceptions are `fix/…` (a bug in something already built)
  and `plan/…` (changes to this doc).
- **New idea?** Add it as a `proposed` task under the milestone it serves, in a `plan/`
  pull request, and don't build it. That is how nothing is lost and nothing jumps the queue.
- **A milestone starts** when the one before it is done, except tasks marked *parallel*. The
  first pull request of a milestone is a `plan/` PR that expands its tasks (engineering notes,
  traps, the exact check) the way M1's are expanded.
- **Every 10 merged pull requests**, re-anchor: compare the HANDOFF state table with the north
  star below, write one paragraph in the HANDOFF session log, and fix any task that has
  drifted.

## The north star: what "finished" means

v1.0 is done when all five hold on the reference laptop, shown by a person, not only in a VM:

| | Scenario | Proven by |
|---|---|---|
| **N1** | **Daily use.** A person boots, signs in, gets online over Wi-Fi, browses, watches a video with sound, and suspends and resumes, all without a terminal. | M1, M3 |
| **N2** | **Unbreakable.** Every step of the "Try to Break It" demo ([doc 15](15-self-healing.md)) recovers on its own: a bad update, a crashing update, ransomware, the system root deleted, a full disk, a corrupted config. | M1, M2 |
| **N3** | **Never lose a file.** An overwritten, deleted or encrypted file comes back from the shell's own timeline, without a terminal. | M2, M3 |
| **N4** | **A new machine in minutes.** A blank laptop becomes the person's machine, with their files and settings, from a backup they made without thinking about it. | M4 |
| **N5** | **Runs what you have.** A person installs an app from a store, and runs one Windows program. | M3, M5 |

## Settled architecture

Decided now so that nobody improvises them mid-task. Changing one takes a new ADR, and Rob
approves it. Items marked *(record)* are true in the code today but have no ADR yet. Write
the ADR as the first task of the milestone named.

| | Decision | Why |
|---|---|---|
| **A1** | **systemd is PID 1 for v1.** `morphd` (transactions, Manifest, service graph: `morph-transaction`, `morph-graph`) is a service under it, not init. *(record in M2; supersedes doc 03's "PID 1")* | Both images already run systemd; replacing init is years of work that serves nothing in the north star |
| **A2** | **The userland base is Ubuntu 24.04, built with `mmdebstrap`**, pinned. | Drivers, firmware and packages for a real laptop, now |
| **A3** | **Immutable A/B system slots; everything mutable lives on the data partition** (`/persist`: `/home`, the Strata store, app data, machine settings). Nothing writes to a slot except `morph-update`. | It is what makes N2 possible, and it is built (ADRs 0006, 0008) |
| **A4** | **Updates are whole slot images, signed.** A `BLAKE3SUMS` manifest (as today) signed with **ed25519** by an offline key that Rob holds, served as static files over HTTPS. The device checks the signature **before** `morph-update` writes anything. | Integrity exists today; authenticity does not. Static files need no server code |
| **A5** | **One process draws the screen:** `flow-compositor` (Smithay). The shell's own chrome is painted by `flow-shell`; programs are Wayland clients. No X server, no GNOME, no KDE (ADR 0010). | Built (E1) |
| **A6** | **NetworkManager** for networking, **PipeWire + WirePlumber** for audio, **CUPS** for printing, **BlueZ** for Bluetooth. The shell drives them over D-Bus. | Standard, with D-Bus APIs the shell can use; picking now prevents two of each |
| **A7** | **Apps come as Flatpaks** (Flathub), installed to the data partition: sandboxed, and they survive every A/B update. The browser is the exception for M1 (a `.deb` in the image, E4) until Flatpak lands (D4). **Never snaps.** | An app store on day one without building one; apps outside the immutable slot |
| **A8** | **Testing pyramid:** unit tests → CLI scripts → a VM scenario in the `images` workflow → a hardware report in `docs/hardware/`. **Every user-facing task ends in a VM scenario** whose verdict is read off the disk or the screen (AGENTS.md H8). | It is how everything built so far was proven |
| **A9** | **People log in as an ordinary user; root is locked.** Admin actions go through `morphd` transactions with the `.rob` mark (ADRs 0011, 0012). *(Rob decides the details in S5)* | Security model: Rob's call, so it is a task for him, not a default |

## M1 · Express road: a person drives it

Exit: a general user boots the laptop, gets online, browses, and cannot make it unbootable
(ADR 0014).

### E1 · The shell becomes a real Wayland compositor (headless) · done

**What:** `crates/flow-compositor`: a Smithay compositor, headless with the pixman renderer,
that hosts real clients' windows.

**Done when:** a real client's window appears in the compositor's frame exactly where it was
placed, two windows both appear, and with no client no frame is written (`tests/frame.rs`).

**Evidence:** https://github.com/robmitchellengineering-hub/MorpheOS/pull/38 and `crates/flow-compositor/tests/frame.rs`

### E2 · Classic windows · done · needs E1

**What:** the Classic feel (ADR 0013, item 3) manages windows: click to focus, move, close,
fullscreen, and a keyboard shortcut to switch windows. Focus decides who gets the keyboard
(`wl_seat` keyboard focus).

**Done when:** in the headless test, two clients map; a scripted click on the second gives it
keyboard focus (the client receives `wl_keyboard.enter`); a scripted close removes it from the
frame. Frames are asserted after each step.

**Notes:** drive input through a test-only control (a command on stdin or a socket) that
injects events into the same code path real input uses. Never a separate path that only
tests take (AGENTS.md H3).

**Evidence:** `crates/flow-compositor/tests/windows.rs` (click focuses B: border and
`keyboard-enter`; close: B's client exits, B leaves the frame, focus moves to A) and
`crates/flow-compositor/src/wm.rs` (the decisions, unit-tested). Move, fullscreen and the
switch shortcut on real input are E9.

### E3 · The shell on a real display, inside the image · done · needs E1

**What:** `flow-compositor --drm /dev/dri/card0`: the same software renderer, its frame
copied into a DRM dumb buffer on the first connected output (no GPU stack, works on any KMS
driver). A `shell` image scenario runs it as a service, with a demonstration window.

**Done when:** the `images` workflow boots the `shell` image in QEMU with virtio-gpu, takes a
`screendump`, and finds the shell's background over most of the screen and the window at its
exact place and size; the boot is healthy and committed (`images/x86-qemu/test-shell.sh`).

**Evidence:** https://github.com/robmitchellengineering-hub/MorpheOS/pull/42 and
`images/x86-qemu/test-shell.sh` (the background over 960000 of 1024000 pixels,
the window at (16, 16), 320x200, the boot committed). It found two real faults first: the
image lacked the compositor's libraries, and virtio-gpu shows nothing until DIRTYFB says the
frame changed.

**Notes:** `plain` (the image for people) keeps its text console until E4, because a
shell with nothing to run in it serves a person worse than the console. The service runs as
root for now (E11).

### E10 · Real keyboard and mouse (libinput) · done · needs E3

**What:** libinput through Smithay feeding the same handlers the `--script` commands use
(E2), in a seat session from `seatd` (libseat): it owns VT1 for the shell, so keys reach
windows and not the console underneath, and Ctrl+Alt+F2 reaches the root console on VT2.

**Done when:** a QEMU scenario clicks a test client with the VM's USB tablet and types into
it with its PS/2 keyboard, and the screendump shows both arrive; Ctrl+Alt+F2 shows the
console and Ctrl+Alt+F1 brings the shell back.

**Evidence:** https://github.com/robmitchellengineering-hub/MorpheOS/pull/43 and
`images/x86-qemu/test-shell.sh` (click, pointer, key, VT2 and back, all read
off screendumps) and `crates/flow-compositor/tests/input.rs` (the same handlers headless:
the client gets `button 272` and `key 30`, and Ctrl+Alt+F2 never reaches it).

**Notes:** `plain` keeps its text console until E4: a shell with nothing to run in it serves
a person on the laptop worse than the root console it opens on.

### E11 · The shell as an ordinary user, and a broken shell falls back · open · needs E3

**What:** start the compositor in a seat session (`libseat` with `seatd` or logind) as the
ordinary user (A9; Rob decides the account model in S5), not root. Add an image scenario
whose update ships a compositor that fails to start.

**Done when:** the shell scenario passes with the compositor not running as root, and the
broken-shell update is trialled once and falls back, like `unhealthy`.

### E4 · A browser · open · needs E10

**What:** Firefox from Mozilla's own APT repository (packages.mozilla.org, pinned), running on
Wayland (`MOZ_ENABLE_WAYLAND=1`). Then `plain` boots into the shell, with the browser in it
and the root console on VT2 (moved here from E10: until there is something to run, the
console serves a person better), and `images/x86-qemu/HARDWARE.md` says how to reach both.

**Done when:** in QEMU, Firefox shows a local `file://` page of large known colours, and the
screendump contains them; `plain` boots into the shell.

**Notes:** ⚠️ Ubuntu 24.04's `firefox` and `chromium` packages are snap stubs. Firefox needs
more protocols than E1 serves: `linux-dmabuf` (or it falls back to shm), `xdg-decoration`,
`wl_output`, `viewporter`, `xdg-activation`, `primary-selection`, `text-input-v3`. Add them
and log which ones Firefox actually binds (`WAYLAND_DEBUG=1`). Fonts, a D-Bus session and a
writable home on `/persist`. Raise `ROOT_MB` deliberately and record the new size.

How to approach it (written at handover, from what E1–E10 taught):
1. **Protocols first, headless, in `cargo test`.** Each one is a Smithay `delegate_*!` plus
   its state in `App`. `wl_output` is the one GTK cannot start without: add one `Output`
   with the screen's size. **Do not advertise `linux-dmabuf`:** the pixman renderer cannot
   import dmabufs, and without it Firefox draws into `wl_shm` buffers in software (WebRender's
   software backend), which is exactly what this renderer shows. Popups (`xdg_popup`) must be
   drawn at their positioner's place, or menus and dropdowns are invisible; today
   `new_popup` ignores them.
2. **A real toolkit client before Firefox.** Add a test that runs a small GTK or terminal
   client (`foot` is Wayland-only and small) against the headless compositor and asserts its
   window in the frame. If that fails, Firefox will too, and the GTK one is faster to debug.
3. **Then Firefox in the `shell` image.** Mozilla's APT repo, pinned, its signing key checked
   in. The VM needs more memory for it (`-m 2048` for that scenario). It runs as root until
   E11; if it refuses to, record it under **Open questions** in HANDOFF.md and ask Rob rather
   than reaching for `--no-sandbox`-style switches, which change the security model.
4. The verdict is a screendump of a `file://` page of large blocks of known colours, read the
   way `test-shell.sh` reads the demo window.

### E5 · Networking · open · needs E3

**What:** NetworkManager (A6) in the image, wired and Wi-Fi, and a minimal Wi-Fi picker in
the shell (list networks, enter a password) over NetworkManager's D-Bus API.

**Done when:** in QEMU (user networking), the image gets an address and Firefox loads a page
served by the CI host. Wi-Fi is proven in E6.

### E12 · A hardware report from every boot · done

**What:** the `plain` and `shell` images write a report of the machine they booted on to the
USB stick's FAT partition, which a Mac or Windows PC reads natively: the screens (connectors,
connected or not, modes), the graphics, Wi-Fi and input devices, the kernel driver bound to
each PCI device, every **firmware file the kernel asked for and did not find**, and
`lspci`/`lsusb`. The images also carry the laptop firmware sets (AMD, Intel and MediaTek
graphics and Wi-Fi, Realtek, Broadcom, Intel audio), so a first boot shows what works rather
than only what is missing.

**Done when:** the `plain` boot in the `images` workflow reads the report back off the disk
and finds the VM's own facts in it (its virtio display connector, its virtio disk, its
keyboard), and HARDWARE.md says how to send a report back.

**Evidence:** https://github.com/robmitchellengineering-hub/MorpheOS/pull/46 and
`images/x86-qemu/test-boot.sh` (the `plain` boot reads
`morpheos-report/001-…-a/` off the disk: a QEMU machine, a connected screen, the virtio disk
with its driver, named devices, the PS/2 keyboard) and `scripts/verify-hwreport.sh` (the
missing-firmware section names what a kernel log says is missing, and says none on a clean
one).

**Notes:** opened by Rob on 2026-10-13, to bring real hardware forward: the report is what
makes one boot of a stick worth a whole debugging session. One folder per boot, so booting
the same stick on several machines collects all of them.

### E6 · First boot on real machines, and the reference laptop · open · needs E12

**What:** Rob boots the `plain` stick ([HARDWARE.md](../images/x86-qemu/HARDWARE.md)) on each
candidate machine, from harness-mesh's `MACHINES.md`: the Windows laptop at home (`win`), the
Lenovo Legion Go (`win-2`: AMD graphics, MediaTek Wi-Fi, a portrait panel) and the 2015 Mac
at work (Broadcom Wi-Fi). He sends back the report folders. The reference laptop is Rob's
choice, made with those reports in hand. Intel Wi-Fi and Intel graphics have the fewest
firmware surprises on Linux.

**Done when:** `docs/hardware/<machine>.md` exists for each machine booted, written from its
report (display, keyboard, touchpad, Wi-Fi, sound, firmware missing), and the reference
laptop is recorded here. Every failure becomes a task.

**Notes:** moved ahead of E4 on 2026-10-13 (Rob): the riskiest unknown is the hardware, and
finding it while the browser is being built costs nothing. Suspend and sound wait for M3;
note what the report shows of them.

### E7 · Install to the internal disk · open · needs E6

**What:** an installer that lays out ESP, slot A, slot B and data on the internal disk, and
writes the system with `morph-update`'s verified write.

**Done when:** a QEMU scenario boots from a second disk, installs to the first, and then boots
the first alone to a committed slot. It refuses to touch the disk it booted from.

### E13 · The shell on the screen a person is looking at · proposed · needs E12

**What:** the compositor shows the shell on the first connected output only. On a laptop
that is the internal panel, even when it is broken and an external screen is plugged in (the
hardware image's own notes say a reference candidate has no working internal screen), and
on the Legion Go the panel is natively portrait. Show the shell on every connected output
(mirrored, each at its own preferred mode) and honour the kernel's `panel orientation`.

**Done when:** a QEMU scenario with two virtio-gpu heads (`max_outputs=2`) takes a
screendump of each (`screendump` takes `head`) and finds the shell on both.

### E9 · Move, fullscreen, and the switch shortcut, on real input · open · needs E3

**What:** what E2 left for real input: drag a window by its edge, fullscreen and back, and
Alt+Tab running `wm::Manager::cycle`. Decisions go in `wm.rs` with unit tests first.

**Done when:** the headless script gains `drag`, `fullscreen` and `key` commands through
the same handlers real input uses, and a test asserts each in the frame.

### E8 · Try to break it, on the laptop · open · needs E7

**What:** Rob runs the steps the VM suite covers, on the real machine.

**Done when:** results are in `docs/hardware/`, and every failure is a task.

## M2 · Safe to hand to someone

Exit: N2 holds in full, and updates arrive and apply by themselves, safely.

### S1 · Record A1 as an ADR · open · parallel

**Done when:** an ADR states that systemd is PID 1 for v1 and that `morphd` is a service,
superseding doc 03's claim; doc 03 links it.

### S10 · The whole bar on the builder's Windows laptop (WSL2) · done · parallel

**What:** from 2026-10-16 the builder works on Rob's Windows laptop (`win` in harness-mesh).
Every M1 task is Linux work, and off Linux the bar silently skips the compositor's tests
(AGENTS.md H16). A setup script for WSL2 Ubuntu 24.04 (the image's own base) installs what
CI installs; `scripts/check.sh` refuses to report a partial bar as passing off Linux; and one
command runs the `images` VM suite locally, with KVM when WSL2 provides it.

**Done when:** `scripts/check.sh` exits non-zero on a non-Linux machine unless told it is a
partial run, and says why; the setup script, run on a fresh Ubuntu 24.04, leaves a machine
on which `scripts/check.sh` and `images/x86-qemu/test-shell.sh` both pass (shown in CI by
running it in the `ubuntu-24.04` runner).

**Evidence:** https://github.com/robmitchellengineering-hub/MorpheOS/pull/45 and
`scripts/setup-linux.sh`, which CI's `rust` and `images` jobs now install
through (so every pull request runs it on a fresh Ubuntu 24.04 runner, and the `images` job
then passes `test-shell.sh`); `scripts/test-images.sh`; and the `ci` step "Prove check.sh
refuses a partial bar off Linux".

### S9 · Pin the exact Rust version, locally and in CI · open · parallel

**What:** `rust-toolchain.toml` names an exact version, and the workflows honour it (drop
`dtolnay/rust-toolchain@stable`, which overrides the file). Bump it deliberately, in its own PR.

**Done when:** CI prints the same `rustc --version` as `rust-toolchain.toml`. Found on
2026-10-13: CI's newer stable clippy rejected code that passed `scripts/check.sh` locally
(`clippy::chunks_exact_to_as_chunks`), so the local bar was not CI's bar.

### S2 · Signed updates (A4) · open · parallel

**What:** `morph-update` refuses a payload whose `BLAKE3SUMS` lacks a valid ed25519
signature by a key baked into the image. Key custody is Rob's.

**Done when:** tests show an unsigned, wrongly signed or tampered payload is refused before
any write, and the `update` VM scenario passes with a signed payload.

### S3 · Updates arrive by themselves · open · needs S2

**What:** a timer fetches the newest signed payload over HTTPS, installs it into the spare
slot, and trials it at the next boot.

**Done when:** a VM scenario serves a payload from the CI host, and the machine updates and
commits with no person involved; a bad payload falls back.

### S4 · The recovery image (rung H4) · open

**What:** an immutable recovery system that rebuilds both slots from a known-good image,
keeping `/persist`.

**Done when:** a VM scenario deletes the system root of both slots, and the machine rebuilds
itself and keeps its files (doc 15, demo step 1).

### S5 · Accounts and admin (A9) · proposed

**Done when:** Rob has decided the account model, and a VM scenario logs in as the user and
cannot write the slots.

### S6 · Rewind from the shell · open · needs E3

**Done when:** in a VM scenario, an encrypted folder is restored from the shell's timeline
view by scripted input, with no terminal (N3).

### S7 · A full disk does not break the machine · open

**Done when:** a VM scenario fills `/persist`; the machine still boots healthy, Strata prunes
by its retention policy, and the shell warns (doc 15, step 4).

### S8 · A corrupted setting rolls back (rung H2) · open

**Done when:** a VM scenario corrupts a Manifest-managed setting, and `morph-transaction`
rolls it back and names the transaction that caused it (doc 15, step 6).

## M3 · Daily driver

Each task is expanded in M3's planning PR. *Done when* is the shape of the check.

### D1 · Settings in the shell (Wi-Fi, display, sound, power) · open
**Done when:** each setting changes through the shell by scripted input in a VM scenario.

### D2 · Sound (PipeWire) with a volume control · open
**Done when:** a VM scenario plays a tone and captures it from the PipeWire graph.

### D3 · Suspend, resume and battery · open
**Done when:** the hardware report shows suspend and resume work on the reference laptop.

### D4 · Apps from Flathub (A7) · open
**Done when:** a VM scenario installs a Flatpak, which survives an A/B update.

### D5 · Files: a file view with Strata's timeline · open
**Done when:** a VM scenario browses, opens and rewinds a file from the shell.

### D6 · Printing (CUPS, driverless IPP) · open
**Done when:** a VM scenario prints to a virtual IPP printer and the job arrives.

### D7 · Bluetooth (BlueZ) · open
**Done when:** the hardware report shows a pairing.

## M4 · Never lose anything

### P1 · Encrypted backup to a USB disk (Phoenix v0) · open
**Done when:** a VM scenario replicates `/persist` to a second disk, encrypted, continuously.

### P2 · Restore onto a blank machine (rung H5) · open · needs P1, E7
**Done when:** a VM scenario restores a blank disk from that backup to a working machine (N4).

### P3 · Replicate to a second MorpheOS device · open · needs P1
**Done when:** two VMs on one network keep each other's backup.

### P4 · Hardware health warnings · open
**Done when:** a VM scenario with a fake failing SMART report raises a shell warning.

## M5 · Widening (ADR 0014 lane 2)

Approved direction, not yet ordered. Each becomes `open` when Rob orders it.

### W1 · Windows programs (Wine/Proton capsule) · proposed
**Done when:** one named Windows program runs in a VM scenario (N5).

### W2 · Android apps (Waydroid) · proposed
**Done when:** one Android app runs in a VM scenario.

### W3 · The intent bar with a local model (AI proposes, deterministic code decides) · proposed
**Done when:** the top 20 tasks of doc 02 are measured, with and without the model.

### W4 · Pi 5 parity · proposed
**Done when:** the Pi image boots on a Pi 5 with the same A/B, Strata and heal behaviour.

### W5 · Morph Link on one platform · proposed
**Done when:** one non-MorpheOS device keeps a Phoenix backup through it.

### W6 · Fleet (paid) · proposed
**Done when:** revenue-gated (doc 00). Rob decides.
