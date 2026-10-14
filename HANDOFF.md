# HANDOFF

From **2026-10-16** this repository is built by a DeepSeek agent (DSH) on Rob's **Windows
laptop at home** (host `win` in the harness-mesh repository's `MACHINES.md`), working inside
**WSL2 Ubuntu 24.04**. Until then it was built by Claude Code. This file is how the work
continues without anyone who was here before.

**Read in this order:** [AGENTS.md](AGENTS.md) (the rules, and the hazards: mistakes already
made here), this file, [doc 00](docs/00-strategy.md) (what wins arguments), then
[ADR 0014](docs/adr/0014-express-road-first.md) (what we are building next and why).

## The goal, in one line

**A general user boots a laptop, gets online, browses, and cannot make the machine
unbootable** (ADR 0014's exit). Everything below serves that. Anything that doesn't goes in
the widening backlog (ADR 0014, lane 2) and waits.

## Where things stand (2026-10-14)

| Piece | State | Evidence |
|---|---|---|
| A/B boot, one-shot trial, fallback (`morph-boot`) | **VM-proven** | `images` workflow: good / unhealthy / crash scenarios |
| Write + verify a new system into the spare slot (`morph-update`) | **VM-proven** | `update` scenario: slot B carries the payload's marker after boot |
| Rebuild the spare after a failed trial (`morph-heal`, H3/H4 groundwork) | **VM-proven** | ADR 0009; `images` workflow |
| Transactions that roll themselves back (`morph-transaction`, `morph-graph`) | **tested, CLI-proven** | `scripts/verify-transaction-cli.sh`, `verify-apply-cli.sh` |
| Manifest, invariants, risk tiers, signed generations (`.rob`) | **tested** | `morph-manifest` tests; ADRs 0005, 0011, 0012 |
| Strata: versions, rewind, ransomware detection, in the x86 image | **VM-proven** | `test-ransomware.sh`: 24 files back byte-for-byte |
| Flow shell: frame, input, session model, two feels | **software-rendered only** | CI renders `frame.png` and fails if blank |
| `flow-compositor`: real Wayland clients, Classic windows (E1, E2) | **tested headless** | `crates/flow-compositor/tests/` read pixels back from real clients |
| The shell on a real display inside the image (E3) | **VM-proven** | `test-shell.sh`: a virtio-gpu screendump shows the shell and the window in place |
| Real keyboard and mouse, root console on VT2 (E10) | **VM-proven** | `test-shell.sh`: a USB-tablet click and a PS/2 key change the window's colour; Ctrl+Alt+F2 and back |
| Firefox on the shell, a local page read off the screen (E4, part) | **VM-proven** | `test-browser.sh`: the page's colours, inside the screen, boot committed. No menus yet |
| Bootable x86 USB image for a person, with laptop firmware and a hardware report (E12) | **builds, boots in QEMU** | [HARDWARE.md](images/x86-qemu/HARDWARE.md); `test-boot.sh` reads the report back. **Never booted on real hardware** |
| Wi-Fi picker over NetworkManager, a WPA2 join typed on the keyboard (E5, part) | **VM-proven** | `test-wifi.sh`: simulated radios, a wrong password refused, the right one joins, the network's own DHCP log |
| Move, fullscreen, Alt+Tab on real input paths (E9) | **tested headless** | `crates/flow-compositor/tests/move_fullscreen.rs` |
| A shell update that cannot start, or starts and never draws, falls back (E11, part) | **VM-proven** | `test-fallback.sh broken-shell hung-shell`; the shell is `Type=notify`, ready on its first frame |
| Signed updates: a payload is opened only if a key in the image signed it (S2) | **tested, VM-proven** | `morph-update` tests (unsigned, wrong key, tampered); the `update` scenario installs a signed payload. Releases still need Rob's key |
| Pi 5 image | **builds and is read back** | `verify-pi-image.mjs`. **Never booted on a Pi** |

"VM-proven" means a CI job boots it and reads the verdict off the disk. It does **not** mean
it has run on real hardware. Say "builds" or "VM-proven", never "works", until a person has
run it on a machine (AGENTS.md, verification bar).

## Your machine: Windows, and Linux inside it

Everything left in M1 is Linux work: the compositor, the images, the VM tests. On Windows
(or a Mac) `cargo test` compiles the compositor's tests **to nothing** and still says it
passed (AGENTS.md H16). So the work happens in WSL2:

```powershell
wsl --install -d Ubuntu-24.04        # once, from an administrator PowerShell; then reboot
wsl -d Ubuntu-24.04                  # a Linux shell; clone the repository INSIDE it (~/),
                                     # not under /mnt/c, which is slow and breaks permissions
```

Then, inside Ubuntu:

```bash
git clone https://github.com/robmitchellengineering-hub/MorpheOS ~/MorpheOS && cd ~/MorpheOS
bash scripts/setup-linux.sh --images   # what CI installs; CI runs this same script
bash scripts/check.sh                  # the bar (the `rust` job); ends "all checks passed"
bash scripts/test-images.sh ~/img shell   # one VM scenario; `all` runs the `images` suite
```

From a DSH shell in PowerShell, run any of them as
`wsl -d Ubuntu-24.04 -- bash -lc "cd ~/MorpheOS && bash scripts/check.sh"`.

- **KVM makes the VM suite minutes instead of an hour.** `setup-linux.sh` says whether
  `/dev/kvm` exists. If not, put `nestedVirtualization=true` under `[wsl2]` in
  `%UserProfile%\.wslconfig`, run `wsl --shutdown`, and start Ubuntu again (Windows 11 on a
  CPU with virtualisation turned on in its firmware). Without KVM the suite still runs, slowly.
- **Disk:** a full `images` run uses about 20 GB inside WSL's virtual disk.
- `scripts/check.sh` on Windows or macOS **refuses**, and says why (AGENTS.md H16); with
  `--partial` it runs what it can and ends with `PARTIAL`, never `all checks passed`.

## Open pull requests at handover

#42 to #61 were merged on 2026-10-13 and 14, as merge commits. Open when this was written:

| # | Branch | What |
|---|---|---|
| #57 | `plan/diagnosis` | Doc 22 and tasks F1–F10 (written in another session; plan only) |
| (this) | `plan/build-ready` | Every unfinished task expanded into a recipe, an order of work per milestone, S11 proposed |

If CI goes red on it, it is yours to fix, on that branch. **When a pull request's base is
another open branch** (a stack), and the base merges: if it was squash-merged, the next branch
may conflict where both touched the same lines; `git fetch origin && git merge origin/main`,
keep both sides' lines, run `bash scripts/check.sh`, and push. Never rebase or force-push a
branch that has a pull request open.

## How to work (every task, every time)

1. `git fetch origin && git switch -c <type>/<slug> origin/main`. Never commit to `main`.
2. Take the **lowest-numbered open task** in [doc 20](docs/20-build-plan.md) whose `needs` are
   done. Put its ID in the branch name (`feat/e2-classic-windows`). One task per branch.
3. **Write the check first.** Every task's "Done when" names something a machine verifies.
   Make that check exist and fail *before* writing the code that makes it pass. If you cannot
   make it fail, it does not test anything (AGENTS.md H3, H9).
4. Implement the smallest change that makes it pass.
5. `bash scripts/check.sh`. It must end with `all checks passed`. The Rust version is pinned
   in `rust-toolchain.toml` and CI installs exactly that one (`scripts/rust-toolchain.sh`,
   doc 20 S9), so your clippy is CI's clippy. Do not `rustup update` your way past a
   warning; a version bump is its own pull request.
6. Push, open a pull request, and read **every** check by name, including the `images`
   workflow (AGENTS.md H6, H10). If a VM job times out, download the serial-log artifact and
   read it before forming any theory (H11).
7. In the same pull request: set the task's status in doc 20, add its **Evidence:** line (CI run or artifact), and
   add one line to the **Session log** at the bottom.
8. Rob merges. Do not merge your own pull requests.

**Never:** weaken, skip or delete a test to get green; claim something works without
evidence; rename a required CI job without saying so in the PR (H10); edit an accepted ADR
(write a new one that supersedes it).

**A new idea** goes into doc 20 as a `proposed` task in a `plan/` pull request. Don't build it
until Rob opens it.

**When stuck for more than two attempts:** stop. Write what you tried and what you saw under
**Open questions** below, push it, and ask Rob. A clear question beats a third guess.

## Decisions waiting for Rob now

Each item names the task that waits on it; the task's **Notes** in doc 20 lay out the options
and a recommendation. Ask the one that blocks your next task, once, and work on something else
until he answers.

- **The reference laptop.** Candidates: the Windows laptop at home, the Legion Go, the 2015
  Mac at work. The E12 report from one boot on each decides it (E6).
- **`plain` boots into the shell at E4, not E10.** Until the browser lands, the root console
  serves a person on the laptop better than an empty shell (doc 20, E10's notes).
- **The shell runs as root until E11**, which needs the account model (S5).
- **The release signing key (S2).** Images trust a test key made per build. Releases need a key
  Rob holds, and somewhere to keep it (an offline machine, a hardware key): his call, as is
  whether a lost key can be replaced by an update signed with the old one.
- **Open S11** (`morphd` v0 obeys the Manifest). S7 and S8 need it.
- **The account model (S5)**: an appliance with no password, one owner with a password
  (recommended), or several people. E11's first half waits on it.
- **Where releases are served from, and when an installed update reboots (S3).**
- **The installer (E7):** the whole internal disk or alongside Windows (recommended: whole
  disk); `/persist` encrypted or not (recommended: plain for M1); the reference laptop must not
  be `win`, the machine the builder works on.
- **Where saved Wi-Fi passwords live (E5)** (recommended: as now, in NetworkManager's keyfiles
  on `/persist`, until `/persist` encryption is decided).
- **The recovery partition (S4):** its size, and whether updates refresh it.
- **Backups (P1):** the encryption and key model (recommended: Strata's store replicated, each
  object encrypted with `age`; the machine holds only the public half). Its first PR is an ADR.
- **Open E13** (the shell on every screen, and a portrait panel) once the E6 reports show it
  is needed.

## Decisions that are Rob's, not yours

Write the question up (options and a recommendation) and wait:
- merging any pull request;
- anything that changes the security model: signing, the `.rob` mark (ADRs 0011, 0012), keys,
  what runs as root;
- copying code from Morpheus (licence, decided file by file: [doc 17](docs/17-morpheus-family.md));
- which laptop is the reference machine, and anything that costs money;
- turning a `proposed` task into `open`, dropping a task, or reordering milestones;
- changing a settled-architecture decision (A1–A9 in doc 20).

## What to build: docs/20-build-plan.md

**The task list is [doc 20](docs/20-build-plan.md), and only doc 20.** It holds:
- the north star (N1–N5: what "finished" means);
- the settled architecture (A1–A9: decisions you do not reopen without an ADR Rob approves);
- every task, M1 to M5, each with a **Done when:** check.

`scripts/verify-plan.mjs` enforces its format in CI, and refuses a pull request whose branch
names no open task. Use `fix/…` for bugs in built things and `plan/…` for plan changes.

**Current milestone: M1, the express road.** Each milestone opens with an **Order of work**
paragraph: follow it rather than picking by number. Every unfinished task has **Notes** that
are its recipe: the files, the order, the check to write first, what the verdict is read from,
the traps already known, Rob's decisions, and how to split it into pull requests. Lines marked
"verify:" are facts nobody has checked yet: check them first, and fix the note in the same pull
request if it was wrong. A note is advice, not a rule: if the code says otherwise, the code
wins, and you correct the note.

## Open questions

Write here when stuck: the date, what you tried, what you saw, what you need.

- *(none yet)*

## Session log

One line per piece of work: date, who, what, what's next.

- 2026-10-13 · Claude Code · Wrote this file and `scripts/check.sh` (the whole local bar in one
  command). Next: E1.
- 2026-10-13 · Claude Code · Doc 20 (the build plan: north star, settled architecture, M1–M5
  tasks) and `verify-plan.mjs` (format and branch guard in CI). Next: E2.
- 2026-10-13 · Claude Code · E1 done (#38): `flow-compositor` hosts real Wayland clients,
  headless, proven by pixels. Doc 20 merged (#39). Next: E2, then E3.
- 2026-10-13 · Claude Code · E2 done: `wm.rs` (window decisions, unit-tested anywhere),
  `--script` input, focus border, keyboard focus, close. Found and fixed: Smithay's
  `draw_solid` damage is relative to its rectangle. E9 holds the rest. Next: E3.
- 2026-10-13 · Claude Code · E3 (#42): `flow-compositor --drm` shows the frame through a DRM
  dumb buffer; the `shell` scenario's screendump is the verdict. Found: the image lacked the
  compositor's libraries; virtio-gpu shows nothing without DIRTYFB; with KVM the window beat
  the health check, and the harness then exited silently (fixed, H11). Split the old E3 into
  E3 (display), E10 (input) and E11 (non-root, broken-shell fallback). Next: E10.
- 2026-10-13 · Claude Code · E10 (#43, stacked on #42): libinput through a seatd session that
  owns VT1; the script and real devices share one set of input handlers. Found: libinput's
  startup devices waited for the first keypress; losing DRM master on Ctrl+Alt+F2 killed the
  shell. `plain` keeps its console until E4 (Rob to confirm). Next: the handover (plan PR).
- 2026-10-13 · Claude Code · Handover review with Rob: the builder works on the Windows laptop
  in WSL2, not a Mac, and off Linux the bar silently skipped the compositor (H16, S10). Real
  hardware brought forward: E12 (a report from every boot, plus laptop firmware), then E6 on
  three candidate machines. Proposed E13 (the screen a person is looking at). Hazards
  H13–H15 from E3/E10. Next: S10, E12.
- 2026-10-13 · Claude Code · S10: `scripts/setup-linux.sh` (CI's `rust` and `images` jobs now
  install through it, so the builder's setup is proven on every PR), `scripts/test-images.sh`
  (the x86 suite locally), and `check.sh` refuses a partial bar off Linux (proven in CI with a
  `uname` shim). Next: E12.
- 2026-10-13 · Claude Code · E12: `morph-hwreport` writes a report of every boot to the stick
  (screens, drivers, missing firmware, lspci/lsusb); laptop firmware sets in the images (root
  450 of 1200 MB). Found: `mdir -b` lists folders with a trailing slash; the PCI name
  database is a "recommends" we skip, so devices were numbers. Next: Rob boots the stick on
  the three candidates (E6); the builder takes S9 or E4.
- 2026-10-13 · Claude Code · E4 begun (`feat/e4-browser`). Experiment first: GTK3, the toolkit
  Firefox is built on, already rendered fully on the compositor. What it lacked, now fixed
  and tested with a real GTK app (`tests/toolkit.rs`): `wl_output` (GTK logged criticals every
  frame without one); placement by window geometry (GTK's shadow put the window ~40 px off);
  a window wider than the screen is asked to fit. Firefox is next, through CI only (Mozilla's
  hosts are unreachable from the Claude sandbox; GitHub's runners reach them).
- 2026-10-13 · Claude Code · Firefox 158 (Mozilla's build) draws a local page on the Flow shell
  in a VM, read off a screendump (#47, `test-browser.sh`). Found on the way, each from the CI
  log: mmdebstrap reads `signed-by` on the BUILD machine and adds its own to lines without one
  (the key now named at its build path, the image fixed up after); the compositor never
  answered frame callbacks, so every client froze on its first frame (H17); Firefox's
  first-run Terms of Use dialog dimmed the page (`SkipTermsOfUse`, test image only: Rob's
  call for `plain`). E4 still needs popups (menus) and `plain` booting into the shell.
- 2026-10-13 · Claude Code · Popups (`feat/e4-popups`, stacked on #47): menus and dropdowns are
  placed by their positioner relative to their parent, kept on screen, drawn on top, take
  clicks first, and close on a click outside (`tests/popups.rs`, with a real client's popup;
  both sabotages caught). E4 still needs `plain` booting into the shell.
- 2026-10-13 · Claude Code · E5 begun (`feat/e5-networking`): NetworkManager (with
  wpasupplicant and systemd-resolved) in the browser image, managing wired and Wi-Fi; the VM
  gets QEMU's user network; the browser test's local page redirects to a page the host
  serves at 10.0.2.2, so one boot proves the browser (E4) and the network (E5). The Wi-Fi
  picker in the shell is next; Wi-Fi itself is proven on the laptops (E6).
- 2026-10-13 · Claude Code · Rob's call: `plain` skips Firefox's Terms of Use dialog too
  (`SkipTermsOfUse`, as the test image does), when it carries Firefox (E4).
- 2026-10-13 · Claude Code · Found before any laptop boot: `linux-image-virtual` carries no
  laptop drivers. amdgpu, i915, xe, iwlwifi, MediaTek and Broadcom Wi-Fi, the touchscreen and
  Realtek ethernet are all in `linux-modules-extra-<version>`, which the images never
  installed, so the firmware sets E12 added had nothing to load them. A stick would have
  booted on the efifb console with no Wi-Fi. The base now installs the extra modules for its
  kernel, and the hardware build check refuses an image without those drivers (H18).
- 2026-10-13 · Claude Code · Also before any laptop boot: systemd-rfkill, systemd-backlight and
  logind each failed to create their state directory on the read-only root, and a failed
  unit is an unhealthy boot. rfkill and backlight start on any machine with a radio switch
  or a backlight (every laptop; no VM until test-wifi.sh's simulated radios showed it), so
  `plain` on a laptop could have been judged unhealthy. Their directories are tmpfs now.
- 2026-10-13 · Claude Code · Proposed W7 (vendor layers, driver trials and a change freeze) in a
  `plan/` branch, for Rob to open or drop. It is deliberately product-free: a vendor wants to ship
  an appliance image on MorpheOS without forking it, which ADR 0006 and principle 7 already allow
  but nothing yet exercises. Not started. Next: Rob decides.
- 2026-10-13 · Claude Code · E5 (2), the Wi-Fi picker (`feat/e5-wifi-picker`): `flow-wifi`, a
  window on the shell that lists networks, takes a password (drawn as marks, never printed)
  and joins over NetworkManager's D-Bus API (zbus: pure Rust, no libdbus). Its model is
  unit-tested; `tests/on_the_shell.rs` drives the real window on the compositor; the VM test
  (`test-wifi.sh`) joins a simulated WPA2 network (`mac80211_hwsim`, hostapd, dnsmasq). Found
  on the way: a Wayland read after `poll` can return EAGAIN (a `delete_id` alone, say), and
  the picker took that for a lost connection and quit a second after it opened.
- 2026-10-14 · Claude Code · #50 and #51 merged (CI ran the Wi-Fi test: PASS, 74 s). E6 is
  next and it is Rob's: HARDWARE.md now covers his three machines (`feat/e6-stick-guide`):
  writing the stick from Windows (balenaEtcher), each machine's boot menu and Secure Boot
  switch, and, first, the BitLocker recovery key, since turning Secure Boot off on an
  encrypted Windows laptop makes Windows ask for it.
- 2026-10-14 · Claude Code · S9 (`feat/s9-pin-rust`): Rust pinned to 1.99.0 in
  `rust-toolchain.toml`; `scripts/rust-toolchain.sh` installs exactly that, with clippy and
  rustfmt, and fails if any other `rustc` would run. Every workflow and `setup-linux.sh` use
  it; `dtolnay/rust-toolchain@stable` is gone.
- 2026-10-14 · Claude Code · E9 (`feat/e9-move-fullscreen-switch`): windows move by their
  edge, or anywhere with Super held, or by their own title bar (a client's `move` request);
  the window used comes to the front; Alt+Tab switches; Super+F, or the client's own
  request, goes fullscreen and back to the same place and size. Found while sabotaging the
  test: a press passed to a window whose release is not leaves the pointer grabbed for
  good, and every window deaf to the mouse; the test now ends with a click that must arrive.
- 2026-10-14 · Claude Code · E11, second half (`feat/e11-broken-shell-falls-back`): an update
  whose shell exits at once, and one whose shell runs and never draws, are each trialled
  once and fall back (`test-fallback.sh broken-shell hung-shell`). The second needed a change:
  the shell is `Type=notify` and tells systemd it is ready after its first frame is on the
  display; before, a hung compositor was a running unit and the trial would have committed
  (AGENTS.md H19). Running as an ordinary user still waits on S5.
- 2026-10-14 · Claude Code · S1 (`plan/s1-adr-systemd-pid1`): A1 recorded as ADR 0017, systemd
  is PID 1 for v1 and `morphd` a service under it; doc 03 links it. 0015 and 0016 are left for
  the intent-engine and diagnosis records (docs 21, 22).
- 2026-10-14 · Claude Code · S2, signed updates (`feat/s2-signed-updates`): `morph-update`
  opens a payload only if `BLAKE3SUMS.sig` is an ed25519 signature by a key in the image
  (`/usr/share/morpheos/update-keys`), checked before the manifest is parsed; `keygen` and
  `sign` are the builder's half. Every image carries the updater and the key, and the test
  driver installs with the image's own updater (a medium's could check nothing). Images trust
  a per-build test key; the release key and its custody are Rob's (decisions above).
- 2026-10-14 · Claude Code · Picking up S8 found it blocked: nothing on the machine reads the
  Manifest (H5), so there is no Manifest-managed setting to corrupt, and ADR 0009 forbids H2
  before the transactional apply exists. Proposed S11 (`plan/s11-manifest-obeyed`): `morphd` v0
  applies `machine.name` and `storage.retention` through `morph-transaction`. It unblocks S8
  and S7's pruning. Rob opens it or not; meanwhile S2 (signed updates, with a test key).
- 2026-10-14 · Claude Code · Expanded every unfinished task in doc 20 (M1 to M5, not the I, F
  or W7 tasks) into a recipe, and gave each milestone an order of work, so the builder from
  2026-10-16 does not have to plan (`plan/build-ready`, which also carries S11). Added the needs
  the notes already implied (S7 and S8 need S11, D5 needs S6, W5 needs P1 and P3). Found on the
  way: a correctly signed older payload still installs (no release number in the signed
  manifest; S3's first PR), the 256 MiB data partition cannot hold a downloaded payload (S3),
  `storage.min_free_percent`, `storage.replicas` and `[[users]]` are parsed and obeyed by
  nothing (H5; S7, P1, S5), and the Pi image is Debian trixie, not A2's Ubuntu 24.04 (W4).
  Next: E4's `--drm auto`, then `plain` boots into the shell.
