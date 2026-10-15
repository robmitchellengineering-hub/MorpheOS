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
  Only Rob turns `proposed` into `open`, or marks a task `parallel`. `verify-plan.mjs` refuses
  either change, on any branch, unless the branch's log file carries his words as a line
  `Rob on <ID>: <what he said, and when>`.
- **What waits on Rob is written in three fields**, and nowhere else: `**Waiting on Rob:**`
  (the task may not start until he answers: `verify-plan.mjs` refuses a `feat/` branch for it),
  `**Ask Rob to open:**` (a proposed task worth opening now, and why), and
  `**Rob decides before <step>:**` (it may start; that step waits). `node scripts/task.mjs
  --rob` prints them all: it is the only list. When he answers, the branch that acts on it
  records his words (`Rob on <ID>: ...`) and removes or rewrites the line.
- **Branches carry the task ID** (`feat/e2-classic-windows`): exactly one task, one that may
  start. CI refuses any other `feat/` branch, and any branch that is not `feat/`, `fix/…` (a
  bug in something already built) or `plan/…` (changes to this doc and to the checks; no
  code under `crates/` or `images/`, which `verify-pr.sh` refuses).
- **New idea?** Add it as a `proposed` task under the milestone it serves, in a `plan/`
  pull request, and don't build it. That is how nothing is lost and nothing jumps the queue.
- **A milestone starts** when the one before it is done, except tasks marked *parallel*. The
  first pull request of a milestone is a `plan/` PR that expands its tasks (engineering notes,
  traps, the exact check) the way M1's are expanded.
- **Every 10 merged pull requests**, re-anchor: compare the HANDOFF state table with the north
  star below, write one paragraph in a log file of its own (`docs/log/<date>-re-anchor.md`, in
  a `plan/` pull request, which also fixes any task that has drifted). Count from the last
  re-anchor file; until there is one, from pull request #71, so the first is due once #81
  has merged.

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
| **A1** | **systemd is PID 1 for v1.** `morphd` (transactions, Manifest, service graph: `morph-transaction`, `morph-graph`) is a service under it, not init. *(recorded: [ADR 0017](adr/0017-systemd-is-pid-1.md))* | Both images already run systemd; replacing init is years of work that serves nothing in the north star |
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

**Order of work (M1).**

What the builder does in M1, in order. Each step is one branch and one pull request unless it
says otherwise; Rob merges every one. **Steps 1 to 3 are done** (#63 to #66, 2026-10-14); M1
continues at step 4, which waits on Rob's reports.

1. **E4, PR A: choose the display by what is connected** (`--drm auto`, S). Do this before
   `plain` goes onto laptops, where the screen is often not `/dev/dri/card0`.
2. **E4, PR B: `plain` boots into the shell** with Firefox, the Wi-Fi picker and the console
   fallback, `test-boot.sh` judged by a screendump, and HARDWARE.md rewritten (M). This also
   finishes E5's "picker in `plain`". E4 is then done.
3. **E5: the bar, and the picker from it** (a Wi-Fi button and Super+W), with `test-wifi.sh`
   reopening the picker once online (M). Ask Rob the password-storage question when the
   branch starts; the code does not wait for the answer. E5 is then done.
4. **E6, the builder's half, whenever Rob sends report folders:** one `docs/hardware/<machine>.md`
   per machine, MAC addresses removed, and every failure turned into a task (S per batch).
   This slots in between other work.
5. **E13, if Rob opens it** (M). It comes before E7 and E8 if the reference laptop's internal
   screen is broken or portrait.
6. **E7: the installer** (L, four PRs), once E6 is done. Rob's disk decisions come first.
7. **E11, first half: the shell as an ordinary user** (M), once Rob has decided the account
   model (S5), or has agreed to the placeholder user described in E11.
8. **E8, the builder's half: media, a runbook and a ransomware demo for the laptop** (M); then
   Rob runs it.

**What waits on Rob** is printed by `node scripts/task.mjs --rob`, from the tasks' own fields
("How this doc works"). He also merges every pull request.

**If the builder runs out of M1 work,** HANDOFF.md's "While you wait" says what to do. It is
the one place that says it.
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

### E11 · The shell as an ordinary user, and a broken shell falls back · blocked · needs E3

**What:** start the compositor in a seat session (`libseat` with `seatd` or logind) as the
ordinary user (A9; Rob decides the account model in S5), not root. Add an image scenario
whose update ships a compositor that fails to start.

**Done when:** the shell scenario passes with the compositor not running as root, and the
broken-shell update is trialled once and falls back, like `unhealthy`. "Not running as root"
is read off the disk (sharpened): a test-only unit in the `shell` image writes the
compositor's `Uid:` line from `/proc/<pid>/status`, and the owner of `/run/flow/wayland-0`,
to the ESP. `test-shell.sh` reads them with `mtype` and requires a non-zero uid that is the
same as the one in the image's `/etc/passwd`.

**Blocked on:** Rob's account model (S5), or his yes to the placeholder user in note 6 below.
The status goes back to `open` in the `plan/` pull request that records his answer.

**Progress:** the second half is done. `test-fallback.sh broken-shell hung-shell` trials an
update whose shell cannot start, and one whose shell starts and never draws; each is
trialled once and falls back to A, whose shell draws on every boot. The hung case is why the
shell is `Type=notify`, ready only once its first frame is on the display: a running
compositor that shows nothing was a healthy unit. Still to do: the compositor as an ordinary
user, which waits on Rob's account model (S5).

**Notes** (the first half):
1. **Files:** `images/x86-qemu/build.sh` and
   `images/x86-qemu/overlay-shell/usr/lib/systemd/system/flow-shell.service`; also
   `crates/flow-compositor/src/live.rs`, only if the backend changes. The new unit is
   `overlay-shell-test/usr/lib/systemd/system/morph-session-facts.service`, copied for
   `shell` only, never for `plain`. The verdict goes in `test-shell.sh`.
2. **The user.** Create it in `make_root` with a **fixed** uid and gid (1000). Use
   `chroot useradd -u 1000 -U -M -d /home/morph -s /bin/bash morph`, and assert the result in
   `/etc/passwd`. ⚠️ `/etc/passwd` lives in the read-only slot while the files live on
   `/persist`, so every image ever built must give the person the same uid. Chown
   `$DATADIR/home/morph` to 1000:1000 (`mkfs.ext4 -d` keeps ownership). A stick already in
   use gets the new image through an update and keeps its old data partition, so add a
   `tmpfiles.d` line `d /persist/home/morph 0700 morph morph -`.
3. **The session. Options, with a recommendation:**
   - (a) Keep seatd, with `User=morph` and the user in the seatd socket's group. seatd opens
     the DRM and input devices for its clients, so no `input` group is needed (verify:
     the group Ubuntu's `seatd.service` passes with `-g`). This is the smallest change.
   - (b) A logind session: `PAMName=login`, `TTYPath=/dev/tty1`, `LIBSEAT_BACKEND=logind`.
     This is the standard way. It brings device ACLs and `/run/user/1000`, and it is what
     polkit needs to know that the person is "at the console". It needs `libpam-systemd` and
     `dbus` in the image (verify both).
   Recommendation: (a) for this task, because the Done when only needs "not root". Then (b)
   when S5 settles accounts and polkit.
4. **Check first.** Write `morph-session-facts` and the `test-shell.sh` check while the
   compositor still runs as root, and see it go red. Then switch `User=`. Everything
   `test-shell.sh` already checks must still pass, Ctrl+Alt+F2 and back above all (H15). So
   must `test-fallback.sh broken-shell hung-shell`. `Type=notify` works with `User=` because
   the compositor is the main process (`NotifyAccess=main` is the default). Remember that
   `RuntimeDirectory=flow` becomes owned by the user.
5. **Traps.** Clients that still run as root (`flow-browser`, `flow-wifi`) can still connect
   to the user's socket, because root ignores the socket's mode. ⚠️ Moving the picker to the
   user breaks every Wi-Fi join, since NetworkManager authorises a non-root caller through
   polkit and `polkitd` is not in the image (see E5's notes). Keep `flow-wifi` a root system
   service in this task. Moving Firefox to the user (`HOME=/home/morph`) is part of the same
   S5 decision; do not do it here unless Rob says so.
6. **Rob's decision (S5, `proposed`):** the account model. While it is open, ask one
   question: "May E11 use one ordinary user, `morph`, uid 1000, no password, logged in
   automatically on VT1, as a placeholder until S5?" The root console on VT2 stays as it is
   (HARDWARE.md: a test image, not hardened). Without a yes, this half stays blocked: write
   it under Open questions and do not choose an account model yourself.
7. **Size:** M, one PR.

### E4 · A browser · done · needs E10

**What:** Firefox from Mozilla's own APT repository (packages.mozilla.org, pinned), running on
Wayland (`MOZ_ENABLE_WAYLAND=1`). Then `plain` boots into the shell, with the browser in it
and the root console on VT2 (moved here from E10: until there is something to run, the
console serves a person better), and `images/x86-qemu/HARDWARE.md` says how to reach both.

**Done when:** in QEMU, Firefox shows a local `file://` page of large known colours, and the
screendump contains them; `plain` boots into the shell. For the second half, the verdict is
`images/x86-qemu/test-boot.sh`. It boots `plain` with virtio-gpu, and a screendump must show
three things: the shell's background, the colour of Firefox's start page over a large area,
and the Wi-Fi picker's window (the VM has no network). The boot history on the ESP must read
exactly `a/committed/none`, a login must still complete on the serial console, and the
hardware report checks must still pass.

**Evidence:** `images/x86-qemu/test-boot.sh` and `images/x86-qemu/test-browser.sh`. In the
`images` workflow, `plain` boots into the shell: a screendump shows the shell's background,
Firefox on its local start page over a quarter of the screen or more, and the Wi-Fi picker in
front saying in red that there is no Wi-Fi; slot A is committed, a login completes on the
serial console, and the hardware report reads back. `test-browser.sh` shows Firefox drawing a
local page and then one the CI host serves. Writing the `plain` check found that a window
opening under Firefox would be off the screen and behind it (`fix/new-window-in-view`), and
`build.sh`'s new slot-size check measured slot A at 1134 MiB of the 1200 MiB `ROOT_MB`
(94%, Measured); `ROOT_MB` is now 1600.

**Progress:** the first half is done and VM-proven. In `test-browser.sh`, Firefox 158
(Mozilla's build) draws a local page on the shell and then a page the CI host serves (#47,
#49). Popups (menus and dropdowns) are placed, drawn on top, take clicks and close on a click
outside (`tests/popups.rs`). Step 5 (PR A) is done: the service runs `--drm auto`, which
takes the first card in `/dev/dri` with a connected display (`src/display.rs`, unit-tested;
`test-shell.sh` checks the compositor said which card it chose). PR B is done too: `plain`
boots into the shell, and HARDWARE.md says how to reach the shell and the console.

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

What is left, step by step (steps 1 to 4 above are done):

5. **PR A: choose the display by what is connected (S).** Today `flow-shell.service` passes
   `--drm /dev/dri/card0`. ⚠️ On a laptop that path is often wrong. When the kernel has
   `simpledrm` built in, the firmware framebuffer becomes `card0`. Then `amdgpu`, `i915` or
   `xe` loads, takes over from it, and registers as `card1` (card numbers are not reused).
   The shell would fail on a machine whose graphics work. Add `--drm auto` to
   `crates/flow-compositor/src/main.rs` and `drm_output.rs`: try each `/dev/dri/card*` in
   order and take the first one with a connected connector that has modes. Log which card
   was chosen and why. Put the selection in a pure function over a list of cards and their
   connector states, with unit tests, so that it is tested on any machine. Make
   `--drm auto` the service's argument. To check the premise, make `build.sh` print whether
   the kernel config has `CONFIG_DRM_SIMPLEDRM=y` (verify: Ubuntu 24.04's kernel is believed
   to have it).
6. **PR B: `plain` boots into the shell (M).** Write the check first: make `test-boot.sh`
   boot with `VM_GPU=1 VM_QMP=… VM_MEM=2048` and take a screendump (copy the QMP helper from
   `test-browser.sh`). It must fail on today's `plain`. Then:
   - `images/x86-qemu/build.sh`: `plain) HARDWARE=1; SHELL_UI=1`. Build `plain` from the
     browser base: change the `if [ "$SCENARIO" = browser ]` around step 1b to cover `plain`
     too. Give `plain` the same apps as `browser` (`flow-browser NetworkManager flow-wifi`)
     and `overlay-network`. Update the comment at the top of the file, which still says
     `plain` keeps the console.
   - Split `overlay-browser`. What a person gets stays in `overlay-browser`: the policies, a
     new local start page (`usr/share/morpheos/start.html`, one large block of a colour
     nothing else draws) and `flow-browser.service` opening that page. What only the test
     needs goes into a new `overlay-browser-test`, copied only for `browser`: `test-page.html`
     and a drop-in `etc/systemd/system/flow-browser.service.d/test-page.conf`. The drop-in
     must reset the command with an empty `ExecStart=` line before it sets a new one.
   - **Console fallback.** Add `OnFailure=getty@tty1.service` to `flow-shell.service`. A shell
     that fails or hangs (`TimeoutStartSec=60`) then leaves a working console on VT1 instead
     of a blank screen (its `Conflicts=` stopped that getty). Prove it does not hide a
     failure: `test-fallback.sh broken-shell hung-shell` must still fall back.
   - **Firefox closed by the person.** There is no launcher yet, so closing the window would
     leave an empty shell. Set `Restart=on-success` in `flow-browser.service`: a close
     (exit 0) reopens Firefox, while a crash stays a failed unit that the health check sees.
     A crash on the committed slot then leaves no browser until the next boot; say so in
     HARDWARE.md. (verify: what Firefox's exit code is when its last window is closed.)
   - **Firefox's cache and Strata.** Strata versions every file written under `/home` and
     has no exclude list (`crates/strata`). Firefox's disk cache would therefore fill the
     store. Set `XDG_CACHE_HOME=/persist/browser/cache` in `flow-browser.service` (done: the profile is in `/persist/browser/profile` too) and add a
     `tmpfiles.d` line for that directory (verify: Firefox puts `cache2` under
     `$XDG_CACHE_HOME/mozilla/`).
   - **Size.** Make `build.sh` print `du -sm` of `root-a` and fail when it is over 85% of
     `ROOT_MB`. Record the Measured size in the Evidence line. Raise `ROOT_MB` only if that
     check says so, and say so in the pull request: E7's slot sizes depend on it.
   - `.github/workflows/images.yml`: ⚠️ the step "Build the image a person can use" runs
     before `flow-compositor` and `flow-wifi` are built, and with `SHELL_UI=1` the build stops
     at "missing flow-compositor". Move `cargo build --release -p flow-compositor -p flow-wifi`
     into the first `cargo build` step. `scripts/test-images.sh` already builds them first.
   - `HARDWARE.md`: update "What should happen" (the shell, Firefox, and the picker when there
     is no network) and "Use it" (Alt+Tab, Super+F, a window's edge to move it, Ctrl+Alt+F2
     for the root console and Ctrl+Alt+F1 back). Add "If the screen stays dark" (wait about a
     minute for the console to return on VT1, or press Ctrl+Alt+F2), and rewrite the "Not a
     desktop, yet" bullet. The serial autologin stays as it is: `test-boot.sh` reads
     `MORPHEOS CONSOLE READY` from it.
7. **The verdict:** pixels and the disk, as in the Done when. Wait for the
   `morph-boot-check: decision` line before you stop the VM (H13). Read the history with
   `mtype`, never from the console (H8). Prove the check can fail before you trust it (H3):
   with the start page's colour changed, the check must go red, and with `flow-shell`
   masked it must go red too.
8. **Traps:** with the shell in it, the `plain` build needs packages.mozilla.org (the key's
   fingerprint is checked). If Mozilla's servers are down, the stick artefact is not built.
   That is accepted; say so in the pull request. Without KVM, Firefox can take minutes to
   draw. Give the screendump loop the 120 s it has in `test-browser.sh`, and give
   `test-boot.sh` more than 150 s. Firefox draws in software at the laptop's full resolution
   (the Legion Go's is larger than the VM's). How fast that is has not been measured; ask
   Rob to note it in E6.
9. **Rob's decision:** what page Firefox opens on. Recommendation: the local start page. It
   works offline and sends nothing to anyone at boot. Terms of Use are skipped in `plain`
   (Rob, 2026-10-13).
10. **Size:** M for what is left (PR A is S, PR B is M). Each PR leaves `main` green on its
   own.

### E5 · Networking · done · needs E3

**What:** NetworkManager (A6) in the image, wired and Wi-Fi, and a minimal Wi-Fi picker in
the shell (list networks, enter a password) over NetworkManager's D-Bus API.

**Done when:** in QEMU (user networking), the image gets an address and Firefox loads a page
served by the CI host. Wi-Fi is proven in E6. Also (sharpened so that it covers what is
left): `test-boot.sh` finds the picker on `plain`'s screen when the machine has no network
(with E4). And `test-wifi.sh`, after it has joined, closes the picker and opens it again from
the shell's bar. A screendump must show the picker open again, and the network the machine
is on listed in it.

**Evidence:** `images/x86-qemu/test-wifi.sh`, `images/x86-qemu/test-browser.sh`,
`images/x86-qemu/test-boot.sh` and `crates/flow-wifi/tests/on_the_shell.rs`. In the `images`
workflow Firefox reaches a page the CI host serves; the picker joins a simulated WPA2 network
(a wrong password refused, the right one leased an address by the network's own DHCP
server), then Esc closes it and the bar's Wi-Fi button opens it again, listing the network;
and `plain` with no network shows the picker in front of Firefox. Headless, the bar's button
and Super+W open exactly one picker (sabotaged twice: a button that never hits, and no
"only one" rule, each fails the test).

**Progress:** part 1 (#49): NetworkManager in the browser image; `test-browser.sh` sends
Firefox from its local page to one the CI host serves at 10.0.2.2. Part 2: the Wi-Fi picker
(`crates/flow-wifi`), a window on the shell over NetworkManager's D-Bus API that opens by
itself when the machine has no network. `tests/on_the_shell.rs` drives it on the compositor
with a fake backend; `images/x86-qemu/test-wifi.sh` joins a WPA2 network carried by the
kernel's simulated radios (`mac80211_hwsim`) in a VM, a wrong password and then the right one
typed on the VM's keyboard, judged by screendumps and the network's own DHCP log. Firefox and
the picker are together in `plain` (E4). Part 3: the bar along the bottom of the screen, with a
Wi-Fi button (and Super+W) that opens the picker when the machine is already online; there is
only ever one picker.

**Notes:**
1. **The picker in `plain`** comes with E4's PR B (`overlay-network` and `flow-wifi.service`
   in `plain`). There is nothing else to build for it. With no Wi-Fi device (the VM), the
   picker opens and shows "no Wi-Fi device" in its red status line. That is what a desktop
   with neither a cable nor Wi-Fi should show, and it is not a failed unit: `run` keeps the
   window open when `Nm::open` fails.
2. **The bar. There is none today.** `flow-compositor` draws the background and the windows,
   and nothing else. The bar is built here, as small as it can be: a strip `BAR_HEIGHT`
   high (32, from `flow-shell`) in `palette::BAR`, **at the bottom** of the screen, with one
   Wi-Fi button. At the bottom, windows still open at `(16, 16)`, so `test-shell.sh`,
   `test-wifi.sh` (`WINDOW = (16, 16, …)`) and the exact coordinates in
   `crates/flow-compositor/tests/` stay valid. A bar at the top would move all of them.
   Order of work:
   - `crates/flow-compositor/src/wm.rs` first, with unit tests: the space windows may use
     leaves out the bar (`max_size` and `fit` take off `BAR_HEIGHT`, and so does where a
     moved window is kept). A fullscreen window still covers the whole screen, bar included.
   - `headless.rs`: draw the bar after the windows (a fullscreen window draws over it). A
     press on the button is handled before any window sees it, in `pointer_button`, which is
     the path real input takes too (AGENTS.md H3). Super+W does the same thing, in `key`, next
     to Super+F.
   - Opening the picker: a new `--launch-wifi CMD` option in `main.rs`. The service passes
     `/usr/bin/flow-wifi`; tests pass `flow-wifi --fake …`. Spawn it with
     `std::process::Command`, with `WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR` set. **One
     picker only:** if a toplevel with app_id `morpheos.wifi` is already mapped, focus and
     raise it instead. **Reap the child** (`try_wait` in `turn()`), or every closed picker
     stays as a zombie. The picker that opens at boot is still `flow-wifi.service`; the
     `ExecCondition` there is unchanged.
3. **Check first.** Extend `crates/flow-wifi/tests/on_the_shell.rs`, which already runs the
   compositor with a fake picker. With no picker open, `click` the bar's button and the frame
   must show the picker's title colour (`colours::TITLE`). A second click must leave exactly
   one picker. Super+W must open it as well. Then the VM: `test-wifi.sh`, after its green
   "joined" verdict, presses Esc until the picker closes and takes a screendump with no
   picker in it. It then clicks the button through QMP's tablet at a pixel computed from the
   screen size, and waits for a new `flow-wifi: networks … MorpheOS-test` line (that is when
   to look). A screendump decides: the picker is open and MorpheOS-test is listed. (verify:
   how the picker marks the network it is on, `Status::Connected` and the list order in
   `picker.rs`; assert that marking if it has a colour of its own.)
4. **Traps:** `test-shell.sh` counts background pixels (`count(screen, BG) > total // 2`). The
   bar takes 1280 × 32 = 40960 px from the background, so check the threshold again. ⚠️ A
   process the compositor starts runs as the compositor's user. Today that is root, and
   NetworkManager lets root do anything. After E11 the picker runs as the person, and
   NetworkManager asks polkit, which is not in the image. Every join is then refused. That
   belongs to E11 and S5 (see E11's notes); write it as an Open question if E11 reaches it
   first.
5. **Rob's decision: where a saved Wi-Fi password is kept.** Today NetworkManager writes it,
   in plain text, into a keyfile under `/persist/network/system-connections` (directory 0700,
   owned by root). Strata does not version it (it watches `/home` only), so no old copies
   pile up. But anyone holding the disk or the stick can read it, because nothing is
   encrypted. The options:
   - (a) Keep it as it is for M1 and say so in HARDWARE.md.
   - (b) Agent-owned secrets (`psk-flags=1`). NetworkManager then asks a secret agent in the
     person's session every time. The image has no keyring or agent, and it adds a prompt.
   - (c) Encrypt `/persist` (LUKS2, unlocked by the TPM). This protects every file, not just
     Wi-Fi.
   Recommendation: (a) now, and decide (c) together with E7. ⚠️ The installer lays out the
   data partition, and encrypting it later means reformatting it.
6. **Size:** M, one PR (the bar, the shortcut, the launch, the tests).

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
candidate machine (this list is the one the repository uses; it comes from a private list of
Rob's): the Windows laptop at home (`win`), the
Lenovo Legion Go (`win-2`: AMD graphics, MediaTek Wi-Fi, a portrait panel) and the 2015 Mac
at work (Broadcom Wi-Fi). He sends back the report folders. The reference laptop is Rob's
choice, made with those reports in hand. Intel Wi-Fi and Intel graphics have the fewest
firmware surprises on Linux.

**Done when:** `docs/hardware/<machine>.md` exists for each machine booted, written from its
report (display, keyboard, touchpad, Wi-Fi, sound, firmware missing), and the reference
laptop is recorded here. Every failure becomes a task.

**Waiting on Rob:** the report folders from one boot of the newest `plain` stick on each of
`win`, `win-2` and `mac-2015` (HARDWARE.md). The builder's half starts on the first folder
he sends; record it in that branch's log as "Rob on E6: <which reports, and when>".

**Rob decides before E6 is done:** the reference laptop (note 6).

**Not yet hardware, but a first step (2026-10-15):** Rob ran the `plain` image from images run
38604870469 (main at #71) in **VMware Workstation on `win-2`**: UEFI, Secure Boot off, the raw disk
used in place through a flat VMDK, a SATA disk, NAT networking (`e1000`) and no 3D. In his words,
"It booted, it's live now and I can use Firefox." That is the first boot on virtual hardware other
than QEMU's: VMware's SVGA (`vmwgfx`), mouse and SATA controller. It proves nothing about the
Legion Go's own AMD graphics, MediaTek Wi-Fi or portrait panel, which only a USB boot shows. The
VM was made by the mesh job `2026-10-14T2203Z-win-2-morpheos-vmware-2`. The `.vmx`, and how the
VMDK wraps the raw disk, are in that job's `command.ps1` (in Rob's harness-mesh repository).

**Notes:** moved ahead of E4 on 2026-10-13 (Rob): the riskiest unknown is the hardware, and
finding it while the browser is being built costs nothing. Suspend and sound wait for M3;
note what the report shows of them.

The builder's half (Rob does the booting):
1. **Before Rob boots,** give him the number of the newest green `images` run on `main`
   (GitHub, the repository's *Actions* tab, workflow `images`, filtered to branch `main`; or
   `gh run list --workflow images.yml --branch main`). Its `morpheos-x86-usb` artefact is the
   stick to use, and it is kept for 30 days. Since E4 there is one stick: it boots into the
   shell, and when the shell cannot draw it falls back to a console on `tty1` by itself
   (`flow-shell.service`, `OnFailure=getty@tty1.service`); VT2 (Ctrl+Alt+F2) is a console
   either way. So it shows whether the touchpad, the keyboard in a window and the
   compositor's display work, and still reports when they do not. Every boot adds a numbered
   folder, so one stick collects every machine.
2. **Files:** create `docs/hardware/` with one file per machine (`win.md`, `win-2.md`,
   `mac-2015.md`). Each has the same headings: Machine, Boot path (boot menu, Secure Boot,
   anything that went wrong), Display, Keyboard, Touchpad, Wi-Fi, Wired, Sound, Firmware
   missing, Boot history (the `history.jsonl` lines), and Tasks. Write each line from the
   report's `summary.txt`, `lspci.txt` and `drm.txt`, or from what Rob says he saw, and say
   which. "Detected, driver `iwlwifi` bound" is not "works". Only Rob joining a network shows
   that.
3. ⚠️ **The repository is public.** The report records MAC addresses (`net.txt`, `dmesg.txt`)
   and `dmesg` can hold serial numbers. Never commit a raw report folder. Copy only what the
   doc needs, with MAC addresses replaced by `xx:xx:…`. Add a check in the same PR (AGENTS.md:
   a claim about the repository is a test): extend `scripts/verify-docs.mjs` so that every
   `docs/hardware/*.md` has the headings above and contains no MAC address
   (`([0-9a-f]{2}:){5}[0-9a-f]{2}`, case-insensitive). ⚠️ `verify-docs.mjs` has no self-test
   mode yet (`--self-test` is refused with exit 2, like any argument it does not know): add
   one, with a case that fails, and run it in `check.sh` and CI. That much is `plan/` work
   that needs no report, so it can be done while waiting for the first one.
4. **Every failure becomes a task, in a `plan/` PR.** A firmware file or a driver that is
   missing for one of the machines this task names (`win`, `win-2`, `mac-2015`) is a bug in E12. Fix it on a `fix/` branch: add the
   package to `BASE_PKGS` and the module name to `build.sh`'s laptop driver list (H18).
   Anything new goes in as a `proposed` task under M1, with the report line that shows it. A
   portrait or broken internal screen is E13, so link it there; do not add a second task.
5. **Traps:** the 2015 Mac's Wi-Fi may be a BCM4360, which has no open driver (HARDWARE.md
   says so). That is a "no driver" line, not a broken boot. If a machine reaches no shell,
   there is no report; work from Rob's photo of the screen, and make that its own task.
6. **Rob's decisions:** the reference laptop, recorded in this task (its
   **Rob decides before E6 is done:** line, then his words in the log). Weigh this in it: E7 wipes the reference laptop's internal disk
   (see E7), and `win` is the machine the builder works on.
7. **Size:** S for each batch of reports.

### E7 · Install to the internal disk · open · needs E6

**What:** an installer that lays out ESP, slot A, slot B and data on the internal disk, and
writes the system with `morph-update`'s verified write.

**Done when:** a QEMU scenario boots from a second disk, installs to the first, and then boots
the first alone to a committed slot. It refuses to touch the disk it booted from. Made exact
(sharpened), in `images/x86-qemu/test-install.sh`:
- (1) After the first boot, `sfdisk -J` on the target image shows GPT partitions named `esp`,
  `root-a`, `root-b` and `data`, with the plan's types and sizes.
- (2) The stick's partition table and its `root-a` and `root-b` regions hash the same before
  and after, and the in-VM attempt to install onto the stick is refused. That attempt's
  exit status is written to the stick's ESP.
- (3) The second boot, from the target disk alone, records exactly `a/committed/none` in the
  **target's** `morph-boot/history.jsonl`, read with `mtype`, and writes a hardware report to
  the target's ESP.

**Waiting on Rob:** note 8's three choices: the whole internal disk or alongside Windows;
`/persist` encrypted or plain (which also settles E5's open question, where saved Wi-Fi
passwords live); and that the reference laptop is not `win`.

**Notes:**
1. **The layout,** derived from `images/x86-qemu/build.sh` (the stick: `esp` 300 MiB FAT32;
   `root-a` and `root-b` 1600 MiB ext4 each (1200 until E4 added Firefox), type `4f68bce3-e8cd-4db1-96e7-fbcaf984b709`,
   x86-64 root; `data` ext4) and from what `morph-update` writes. The names must stay the
   same, because the slot images find their partitions by `PARTLABEL` (`fstab`; the loader
   entries' `root=PARTLABEL=root-<slot>`; `morph-update`'s and `morph-heal`'s default
   `--root /dev/disk/by-partlabel/root-<slot>`).

   | # | name | type | size (Target) | holds |
   |---|---|---|---|---|
   | 1 | `esp` | `uefi`, FAT32 | 1 GiB, from 1 MiB | `EFI/BOOT/BOOTX64.EFI`, `EFI/systemd/`, `loader/loader.conf` (`default morpheos-a.conf`, `timeout 0`, `console-mode keep`), `loader/entries/morpheos-{a,b}.conf`, `morpheos/{a,b}/{vmlinuz,initrd.img}` (written by `morph-update`), and `morph-boot/` and `morpheos-report/` (left empty) |
   | 2 | `root-a` | x86-64 root | 8 GiB | slot A's ext4 image |
   | 3 | `root-b` | x86-64 root | 8 GiB | slot B's ext4 image |
   | 4 | `data` | `linux`, ext4 | the rest, at least 16 GiB | `/persist`: `home/morph/Documents`, `strata/` (mode 0700), `network/system-connections/` (mode 0700) |

   The installer refuses a disk smaller than 34 GiB (Target). The slots are larger than
   today's 1600 MiB image on purpose. `write_image` writes into the device without resizing
   it, and `hash_prefix` reads back only the image's length, so a smaller image in a larger
   partition is fine. A later image larger than its partition can never be installed
   without repartitioning, which loses the A/B guarantee. Use E4's Measured root size to
   check that 8 GiB leaves room; apps go on `/persist` (A7), not into the slot.
2. **What gets written: the running system, both slots, verified.** The stick's running root
   is the payload, as in `morph-heal`. The root is a symlink to the running root's block
   device, which is mounted read-only, so it cannot change under the read. The kernel and
   initrd come from `/efi/morpheos/<running slot>/`. Write slot A **and** slot B from it.
   ⚠️ Do not leave B empty with its `.morph-incomplete` marker: `morph-update install`
   refuses any slot that carries the marker, so the machine could never take its first
   update. Move the staging code out of `Heal::repair` (`crates/morph-heal/src/lib.rs`) into
   one function that both use, rather than copying it. S2 has merged, so stage the way
   `morph-heal` does there: a key made for this one install signs the staged manifest.
3. **The policy, without a fake.** `morph_update::install` asks `morph_boot::update_target`
   which slot may be written, and that answer comes from the running machine's
   `/proc/cmdline`. For a blank disk it is the wrong question. Implement `BootControl` in
   the installer for the target disk: its ESP, and a "booted" slot that is the other slot
   from the one being written. Document it. The real guard is the refusal in note 4, and it
   has its own tests. Do not reach for `--cmdline` with a file you made up just to steer the
   CLI; that is a check satisfied by something other than its subject (H3).
4. **Files:** a new crate, `crates/morph-install` (Rust, ADR 0002), with `src/lib.rs` (pure
   parts tested on any machine) and `src/main.rs` (`morph-install [--yes] DISK`).
   - The pure parts: the plan from a disk size; the partition device names
     (`/dev/vda` → `vda1`, `/dev/nvme0n1` → `nvme0n1p1`, `/dev/mmcblk0` → `mmcblk0p1`); and
     the refusals.
   - **Refuse:** the disk holding `/` or `/efi` (find the parent disk of each mount source in
     `/proc/self/mountinfo` through `/sys/class/block/<part>/..`); any disk with a mounted
     partition; a disk too small; a removable disk unless `--yes` names it by its
     `/dev/disk/by-id` path. Each refusal is a unit test, and each test is sabotaged once to
     prove it can fail.
   - **Without `--yes`:** print the disk's model, size and current partitions, and ask the
     person to type the disk's name back.
   - **Steps:** `sfdisk`, `mkfs.vfat -F 32 -n ESP`, `mkfs.ext4 -L data`, the ESP tree, then
     `morph_update::install` for A and then B. Every step is guarded, and a failure says
     which step it was (H11).
   - Then `bootctl install --esp-path=<target esp>` adds the firmware boot entry
     (verify: that it writes the NVRAM entry inside OVMF, and that `systemd-boot-efi` is in
     the image).
   - Ship the binary in the hardware images (`build.sh`, next to `morph-update`). Add
     `fdisk` and `e2fsprogs` to `BASE_PKGS`, and assert that `sfdisk` and `mkfs.ext4` exist,
     the way `seatd` is asserted. ⚠️ Neither package is in the list today, and
     `--variant=apt` installs only Essential packages and their dependencies.
   - Add the crate to `scripts/test-images.sh`'s `cargo build` line and to `images.yml`'s.
5. **The VM scenario: `images/x86-qemu/test-install.sh`, in the `scenarios` group of
   `scripts/test-images.sh`, and a step in `images.yml` after "Ransomware against Strata".**
   - `run-vm.sh` gains `VM_TARGET=<blank.img>`: a 40 GiB sparse file (`truncate`) attached as
     an **NVMe** disk (`-device nvme,serial=morph-target`), like a real laptop's. The stick is
     given `bootindex=0`, so OVMF boots it first.
   - `build.sh` gets an `install` scenario: `plain` plus `overlay-install-test`. That overlay
     holds a oneshot unit with `ConditionKernelCommandLine=morph.install-test`, which only the
     stick's own loader entries carry. The installer writes its own entries from a template,
     so the installed system does not run the test again. Assert that the target's entries
     do not contain `morph.install-test`.
   - The unit runs `morph-install --yes` against the stick and records the refusal, then
     against `/dev/disk/by-id/nvme-…morph-target` (find the device by its serial, not its
     position), then powers off.
   - The second boot uses `run-vm.sh` with the target as the only disk. It is a `plain`
     system, so it stays up until the timeout, as in `test-boot.sh`.
   - Read the slots on the host with `losetup -P` and `debugfs`, as `test-fallback.sh` does,
     and parse debugfs's output, not its exit code.
6. **The verdict** is the target disk's own records, read on the host (the Done when). Never
   read it from the installer's output. Before you trust each check, make it fail once:
   install onto the stick, or skip `root-b`.
7. **Traps:**
   - ⚠️ **Two disks with the same labels.** Once the installer has written the target,
     `/dev/disk/by-partlabel/root-a`, `esp` and `data` match two disks. udev points each
     link at whichever disk it saw last. A tool on the stick that takes the default
     `--root`, or the stick's own `/efi` mount, could then be talking to the wrong disk. So
     the installer names every device explicitly, and never uses `by-partlabel`. After the
     install, the same trouble comes back for a person who leaves the stick in. Fix that in
     PR 3: the loader entries say `root=PARTUUID=<…>`, and the ESP and data mounts come from
     the entries (`systemd.mount-extra=PARTUUID=…:/persist:ext4:defaults`; verify that
     systemd 255 in Ubuntu 24.04 supports it) instead of `PARTLABEL` lines in the slot's
     `fstab`. Then `morph-update` and `morph-heal` find `root-<slot>` on the disk that holds
     `/efi`. Check it with a third boot of the target with the stick still attached: the
     history and the report must land on the target.
   - Fresh OVMF variables boot only `\EFI\BOOT\BOOTX64.EFI`. `run-vm.sh` keeps one vars file
     per disk path, so the second boot starts with no NVRAM entry, which tests the fallback
     path. Real laptops often prefer an NVRAM entry, which is why `bootctl install` is in
     the steps.
   - After the install, Secure Boot must stay off: the image is not signed.
   - Writing two 1600 MiB slots under TCG (no KVM) is slow, so give the VM 1800 s.
8. **Rob's decisions** (before PR 1):
   - (a) The whole internal disk, or alongside Windows? Recommendation: the whole disk for
     M1. Shrinking NTFS and sharing the ESP with Windows is where installers lose data.
   - (b) An encrypted `/persist` (LUKS2 with the TPM) or a plain one? It has to be decided
     now, because the installer lays it out (E5's password question). Recommendation: plain
     for M1, and encryption as a `proposed` task under M2.
   - (c) The reference laptop must not be `win` while the builder works on it, or the
     install wipes the build machine (E6).
   - (d) The installed system is the stick's: root with no password and autologin, until S5.
     Should HARDWARE.md say so before the install section?
   - (e) Whether the installer is run from the console (the M1 plan) or from a window in the
     shell. A window would be a `proposed` follow-up task.
9. **Size:** L. The PRs, each green on its own:
   - (1) `morph-install`: the plan, the refusals and the staging function shared with
     `morph-heal`, all unit-tested, plus the binary in the image (M).
   - (2) `run-vm.sh` `VM_TARGET`, the `install` scenario, `test-install.sh` and the
     workflow step. This finishes the Done when (M).
   - (3) Disk identity by PARTUUID, with the boot that has the stick still attached (M).
   - (4) HARDWARE.md, "Install to this machine's disk" (S).

### E13 · The shell on the screen a person is looking at · proposed · needs E12

**What:** the compositor shows the shell on the first connected output only. On a laptop
that is the internal panel, even when it is broken and an external screen is plugged in (the
hardware image's own notes say a reference candidate has no working internal screen), and
on the Legion Go the panel is natively portrait. Show the shell on every connected output
(mirrored, each at its own preferred mode) and honour the kernel's `panel orientation`.

**Done when:** a QEMU scenario with two virtio-gpu heads (`max_outputs=2`) takes a
screendump of each (`screendump` takes `head`) and finds the shell on both.

**Ask Rob to open:** once an E6 report shows the reference laptop's internal screen is
broken or portrait (the Legion Go's is portrait). It then comes before E7 and E8.

**Notes:** not started. It is `proposed`, so Rob opens it. Open it before E7 and E8 if the
reference laptop's own panel is broken or portrait: without this task, the shell may not be
on the screen Rob is looking at.
1. **Experiment first (half a day), before any code.** Boot the `shell` image with
   `-device virtio-gpu-pci,max_outputs=2` and read `drm.txt` in the hardware report. Is the
   second connector (`card0-Virtual-2`) `connected`? (verify: with `-display none`, QEMU may
   turn on only the first head.) If it is not, try a VNC display per head
   (`-vnc unix:…,display=<gpu id>,head=1`). If nothing connects it, write that under Open
   questions with the options, and ask before you change the Done when.
2. **Files:** `crates/flow-compositor/src/drm_output.rs` (one `Output` per connected
   connector, each with its own CRTC (`pick_crtc` must not hand out the same one twice), its
   own dumb buffer and its own preferred mode); `live.rs` (show and restore every output,
   H15); `wm.rs` (the screen size); `run-vm.sh` (`VM_GPU_OUTPUTS=2`); and a new
   `images/x86-qemu/test-screens.sh`. Put it in the `shell` group of `scripts/test-images.sh`
   and add a step to `images.yml`. It uses the `shell` image; no new scenario is needed.
3. **The smallest first slice: mirroring without scaling.** The logical screen is the
   smallest connected output. Every output shows that frame centred, with the shell's
   background around it. Windows, `fit` and the pointer all work in that one logical space,
   so nothing in `wm.rs` changes beyond the size. Put the copy (offset and rotation) in a pure
   function compiled on every platform, and unit-test it.
4. **The verdict:** QMP `screendump` with `device` and `head` (QEMU 8.2 in Ubuntu 24.04 has
   both). On each head, the shell's background covers most of the screen and the
   demonstration window is at its place, read the way `test-shell.sh` reads it. Make it
   fail first by showing on head 0 only.
5. **Panel orientation:** read the connector's `panel orientation` property and rotate
   during the copy, swapping the logical width and height for 90° and 270°. ⚠️ QEMU cannot
   present this property, so the Done when cannot see it. Unit-test the rotation, and leave
   the Legion Go to prove it (E6, E8). The touchscreen's coordinates need the same rotation.
   The kernel console may already be rotated by the kernel's own panel quirks (verify).
6. **Traps:** with an external screen plugged in after boot (the broken-panel case), nothing
   happens until there is hotplug. Re-scanning connectors on a udev `drm` change event is a
   second PR, and it cannot be seen in QEMU without `device_add`. Use E4's `--drm auto` card
   choice; on a laptop with two GPUs, the external port may hang off the other card. Treat
   that as out of scope, and write it as a `proposed` task if a report shows it.
7. **Rob's decision:** open the task. Recommendation: open it once the first E6 report
   shows which outputs each machine has.
8. **Size:** M. PR 1: several outputs, mirrored, with the test. PR 2: orientation, unit-tested.
   PR 3, if it is needed: hotplug.

### E14 · Firefox goes online when the Wi-Fi picker joins · proposed

**What:** on the first real boot (mac-2015, 2026-10-15) Rob joined Wi-Fi through the picker,
but Firefox still showed its offline page. The browser does not learn the network is up until
it is reloaded. It should come online once NetworkManager reports a connection, with no
manual reload.

**Done when:** a QEMU scenario joins a simulated network through the picker (test-wifi.sh's
rig) and then asserts Firefox loads a page served by the CI host, without any reload step.

**From the report:** "I was able to connect to the wifi … but firefox didnt connect it was
still off line" — Rob, mac-2015 boot, 2026-10-15.

### E15 · The shell is readable on a high-DPI panel · proposed

**What:** the compositor draws at the panel's native mode, so on mac-2015's 4096×2304 screen
everything is tiny. The shell needs a scale (a default output scale, or honouring the mode's
physical size) so a 4K panel is readable.

**Done when:** a QEMU scenario on a high-resolution head asserts the shell's chrome is drawn
at a minimum on-screen size (a scale factor is applied), not 1:1 at the native mode.

**From the report:** "everything was really small" — Rob, mac-2015 boot, 2026-10-15.

### E16 · Resize a window · open · needs E3

**What:** the Classic feel (ADR 0013, item 3) manages windows: focus, move, close, fullscreen,
switch — and resize. A drag on a window's corner resizes it: the corner follows the pointer,
the opposite corner stays put, and the client is told the new size (its `configure`). A drag on
an edge (not a corner) still moves. A window cannot be resized below a floor, nor past the
screen's `max_size`, and a fullscreen window does not resize.

**Done when:** a headless test drags a window's corner and asserts the client is told a new
size (its `configure WxH` line changes) and the frame shows it; the same drag on an edge moves
instead. Manager decisions are unit-tested first (wm.rs), the same way move and fullscreen are.

**From the report:** "i have to be able to resize a window" — Rob, in the session, 2026-10-15.

### E17 · A file view in the shell · done

**What:** a window in the shell that lists a folder's files and opens one — the first thing a
person reaches for after the browser ("where are my downloads"). It lists the person's folders
on `/persist` (home, Downloads, Firefox's profile), opens a file with its default handler, and is
the base D5's timeline and USB-mounting sit on.

**Done when:** a headless test opens the file view, asserts a known file is listed, and that
opening it runs the right command; the listing is a pure function (unit-tested), and the window
is driven through the same input handlers as every other window.

**From the report:** "how to access my files i downloaded" — Rob, in the session, 2026-10-15.

**Evidence:** `crates/flow-files/src/lib.rs` (the listing and the click decision, pure and
unit-tested), `crates/flow-files/src/main.rs` (the window), and
`crates/flow-files/tests/on_the_shell.rs` (the headless test: lists a known file and opens it with
the right command).

### E18 · A USB stick shows up in the file view · open · parallel · needs E17

**What:** a person plugs in a USB stick and its files appear where they can be reached: the
stick is mounted automatically when it appears, under a stable place the file view (E17)
lists, so its files can be opened or copied to `/persist` without a terminal. E17 already
lists and opens files; this is the "where did my stick go" half of that question.

**Done when:** a pure function turns a block device into a mount point (unit-tested); a udev
rule mounts the stick when it is plugged in; and a QEMU scenario attaches a virtual USB disk
with a known file and asserts the file view lists it and opens it.

**From the report:** "if i plug a stick in with data that i want on the computer how does that
process work" — Rob, in the session, 2026-10-15.

### E20 · The file view opens from the bar · open · parallel · needs E17

**What:** E17 built the file view window, but nothing opens it — there is no bar button and no
shortcut, so a person cannot reach their files. Add a Files button to the shell's bar (E5) and a
Super+E shortcut that open the file view, exactly as the Wi-Fi button and Super+W open the picker.

**Done when:** a headless test clicks the bar's Files button and asserts the file view window
appears (and Super+E opens it too); the button's geometry is a pure function (unit-tested), as
the Wi-Fi button's is.

**From the report:** "how to access my files i downloaded" — Rob, in the session, 2026-10-15.

### E19 · The boot/login animation and brand mark · proposed

**What:** the animated MorpheOS mark that opens the shell — a green geomesh drawing itself into
completion (cyberpunk colours phasing to matrix green) with gyro data rings and a twinkling
starfield, over the "MorpheOS" wordmark. This is the brand's first impression, and becomes the
login screen's own (S5). The prototype lives in `design/` — self-contained Three.js; open
`design/login.html` in a browser. It auto-syncs to a music track (`theme.mp3`): the audio's clock
is the master, so the animation and the music stay locked and the tail plays out uncut.

**Done when:** the animation and its still/logo mark are reviewed in `design/`, and the chosen
mark is exported as the SVG logo for the shell and boot.

### E9 · Move, fullscreen, and the switch shortcut, on real input · done · needs E3

**What:** what E2 left for real input: drag a window by its edge, fullscreen and back, and
Alt+Tab running `wm::Manager::cycle`. Decisions go in `wm.rs` with unit tests first.

**Done when:** the headless script gains `drag`, `fullscreen` and `key` commands through
the same handlers real input uses, and a test asserts each in the frame.

**Evidence:** `crates/flow-compositor/tests/move_fullscreen.rs` and `crates/flow-compositor/src/wm.rs`.
The window manager decides (moving by the edge or with Super, staying reachable, raising,
one fullscreen window that returns to its place and size; unit-tested), and the test
drives the compositor through the script's `drag` and `key`:
an edge drag and a Super+drag move windows without the window seeing the button, Alt+Tab
switches and raises without the window seeing the Tab, Super+F and a client's own request
(Firefox's F11) go fullscreen and back, and a plain click still reaches its window at the
end. Fullscreen is the Super+F chord through `key` rather than a `fullscreen` command of its
own: that is the path a keyboard takes. Firefox's title-bar drag arrives as the client's own
`move` request, which starts the same move.

### E8 · Try to break it, on the laptop · open · needs E7

**What:** Rob runs the steps the VM suite covers, on the real machine.

**Done when:** results are in `docs/hardware/`, and every failure is a task.

**Rob decides before the hardware run:** when to run it, and on which machine (note 6).

**Notes:** Rob does the running. The builder makes it possible to run, on the laptop's
internal disk (E7):
1. **The steps: what the VM suite covers today, and nothing from M2.** A good update, an
   unhealthy update, a crashing update (a kernel panic), a broken shell and a hung shell
   (doc 15's "broken driver as an update", as near as M1 gets), heal (the spare rebuilt
   after a failed trial), and ransomware against `~/Documents`. Deleting the system root,
   filling the disk and corrupting a config are S4, S7 and S8 (M2); leave them out.
2. **The media, published by CI.** `images.yml` uploads no payload yet; HARDWARE.md says so
   (fixed on 2026-10-14, with the mount under `/run`), and this step changes that. Teach
   `images/x86-qemu/build-payload.sh` kinds `unhealthy`, `crash`, `broken-shell` and
   `hung-shell`, made with `debugfs` on a copy of `root-a.img` the way the marker is. For
   example, `crash` points `/usr/sbin/init` at `/bin/false`, so the kernel panics and
   `panic=5` reboots it (verify: on Ubuntu 24.04 `/sbin` is a symlink into `/usr`). Upload
   each payload as an artefact. ⚠️ **Publish them from the same run as the stick:** with S2,
   each build makes its own test key, and a payload from another run is refused, as it
   should be. A test that runs each payload kind in the VM must exist before Rob gets the
   sticks. Extend `test-fallback.sh` to take a payload instead of a pre-built slot B.
3. **A ransomware demo a person can run.** The hardware image has no self-test driver, so add
   `/usr/lib/morpheos/demo-ransomware` to `overlay-hardware`. It writes its own files into
   `~/Documents/demo` and waits for Strata to record them. It then attacks them the way
   `images/common/selftest-overlay/usr/lib/morpheos/ransomware-test` does, and waits for
   Strata's alert. Finally it prints the `strata rewind` command, and the checksums before
   and after, to the screen and to `/efi/morph-demo/`. Put the attack in one place that both
   scripts call, and do not copy it. It must never touch a file it did not create.
4. **The runbook:** `docs/hardware/try-to-break-it.md`. For each step: what to plug in, what to
   type on the console (Ctrl+Alt+F2), what to expect on the screen, and **the line in
   `history.jsonl` that is the verdict** (`b/trial/committed`, `b/trial/fallback`). Then what
   to send back. Results go in `docs/hardware/<machine>-try-to-break-it.md`, with MAC
   addresses removed (E6's check covers it).
5. **Traps:** `morph-update --root` must name the inactive slot on the **internal** disk.
   Unless E7's PR 3 has landed, leave the stick out during every step (E7's two-disks trap).
   The payload stick is ext4, so the runbook says to mount it read-only, at
   `/run/morph-media`, and not under `/mnt`, which is on the read-only root (H11).
6. **Rob's decisions:** when to run it, and on which machine (the reference laptop from E6).
7. **Size:** M for the builder (the payloads and their VM test, the demo, the runbook). One
   PR for the media and their test, and one for the demo and the runbook. Then Rob's session.

## M2 · Safe to hand to someone

Exit: N2 holds in full, and updates arrive and apply by themselves, safely.

**Order of work (M2).**

M2 is not `parallel` (S1, S2, S9, S10 were), so by this doc's rule its other tasks start when M1
is done, unless Rob marks one `parallel`. Within M2, take them in this order. Each line says what
it waits on.

1. **S3 · updates arrive by themselves.** Needs S2 (done). The VM proof needs nothing from Rob: it
   uses the per-build test key and a server on the CI host. A person's machine needs two decisions
   from Rob: where releases are served from, and the release key and who holds it (HANDOFF,
   "Decisions waiting for Rob"). And one more: when an installed update reboots.
2. **S6 · rewind from the shell.** Needs E3 (done). Can run alongside S3: S3 is the image's plumbing
   and S6 is a window on the shell, so they share no files. Runs as root until E11.
3. **S4 · the recovery image (H4).** Comes after S3, because it reuses S3's payload and release
   number and changes the disk layout that S3's scenario boots. Rob decides how big the recovery
   partition may be, whether updates ever refresh it, and how the demo is worded (S4's notes).
   None of those block the VM proof.
4. **S11 · `morphd` v0 obeys the Manifest.** `proposed`: **waits on Rob opening it.** S7 and S8
   both depend on it.
5. **S7 · a full disk** and **S8 · a corrupted setting rolls back (H2)**, after S11, in either
   order. S7 also needs a small notice window on the shell. S7 builds it and S8 reuses it.
6. **S5 · accounts and admin.** `proposed`: **waits on Rob choosing an account model** (three are
   laid out under S5, with a recommendation). The first half of E11 (the shell as an ordinary
   user) waits on S5 too. Once Rob decides, S5 is M-sized and can go anywhere in this order.

**Blocked on Rob, in one list:** opening S11 and S5. The S5 account model. The release key and
where it is kept (S2/S3). The update host and when an update reboots (S3). The recovery
partition's size and whether it is refreshed (S4). The wording of demo step 1 (S4). Whether S7
may delete versions the retention policy keeps (recommended: no).

**The x86 job's budget.** Every scenario below adds a step to the `Try to Break It: A/B updates
(x86 VM)` job, which has `timeout-minutes: 90`. Record the job's time in each PR. Raising the
timeout is allowed. Renaming the job is not, without changing the branch protection in the same
step (AGENTS.md H10).
### S1 · Record A1 as an ADR · done · parallel

**Done when:** an ADR states that systemd is PID 1 for v1 and that `morphd` is a service,
superseding doc 03's claim; doc 03 links it.

**Evidence:** `docs/adr/0017-systemd-is-pid-1.md`, indexed, and doc 03's `morphd` section links it.

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

### S9 · Pin the exact Rust version, locally and in CI · done · parallel

**What:** `rust-toolchain.toml` names an exact version, and the workflows honour it (drop
`dtolnay/rust-toolchain@stable`, which overrides the file). Bump it deliberately, in its own PR.

**Done when:** CI prints the same `rustc --version` as `rust-toolchain.toml`. Found on
2026-10-13: CI's newer stable clippy rejected code that passed `scripts/check.sh` locally
(`clippy::chunks_exact_to_as_chunks`), so the local bar was not CI's bar.

**Evidence:** `rust-toolchain.toml` (1.99.0) and `scripts/rust-toolchain.sh`, which every
workflow job and `scripts/setup-linux.sh` run: it installs the pinned toolchain and fails
unless `rustc --version` is that version, and the job logs print it.

### S2 · Signed updates (A4) · done · parallel

**What:** `morph-update` refuses a payload whose `BLAKE3SUMS` lacks a valid ed25519
signature by a key baked into the image. Key custody is Rob's.

**Done when:** tests show an unsigned, wrongly signed or tampered payload is refused before
any write, and the `update` VM scenario passes with a signed payload.

**Evidence:** `crates/morph-update/tests/install.rs` (unsigned, signed by another key, a manifest
changed after signing, a garbage signature, and no trusted keys are each refused with the slot
untouched; sabotaging the check fails three of them) and the `update` scenario of
`images/x86-qemu/test-fallback.sh`, whose payload is signed by `build-payload.sh` and installed
by the image's OWN `morph-update` against the key in its own root: the medium carries no
updater, since one it brought could check nothing.

**Notes:** the images trust a test key made per build (`build.sh`, `UPDATE_KEY` to give a real
one). A release key, and where it is kept, is Rob's: no image a person uses should trust a key
that lives in CI. The custody, a backup key and the release numbering are drafted for him as
[ADR 0019](adr/0019-release-signing-key.md) (Proposed). `morph-heal`, which rebuilds a slot from the running system, signs that copy
with a key made for the one repair, so there is still no unsigned way to open a payload.

### S3 · Updates arrive by themselves · open · needs S2

**What:** a timer checks an HTTPS update server for a newer signed payload than the running
system. It downloads the payload to `/persist`, checks the signature before downloading the
large file, installs it into the spare slot with the image's own `morph-update`, and arms a
trial. The trial's health check then commits it or falls back, exactly as it does today. A
payload that fails its trial is not tried again. A payload that is not signed by a trusted key,
or that does not match its manifest, is refused before anything is written.

**Done when:** `images/x86-qemu/test-autoupdate.sh` runs three scenarios against a payload
served over HTTPS by the CI host, with no input from the harness after boot. The verdicts are
read off the disk after the VM stops, and from the server's request log:
- `good`: the boot history is `a/committed/none b/trial/committed b/committed/none`, slot B
  carries the payload's marker, and the server log shows `root.img` downloaded exactly once.
- `bad` (signed, but its system fails health): `a/committed/none b/trial/fallback
  a/committed/none`, the default is still A, and over at least three later timer runs the
  server log shows no second download of that release.
- `tampered` (a byte of `root.img` changed after signing): no trial in the history, slot B has
  no marker and no `.morph-incomplete`, and the server log shows that the files *were*
  downloaded, so the refusal came from the check and not from a failed fetch.

**Progress:** step 3.1 is done (`fix/payload-rollback`): the signed manifest carries a
required `release <n>` line, every image says its release in `/usr/lib/morpheos/release`
(`RELEASE` in `build.sh`, default 1; payloads default to 2), and `install` refuses an older
release before anything is written and allows an equal one (`morph-heal` seals at the running
release). `tests/install.rs` covers older, equal, newer, a missing, doubled or malformed release
line, and a missing release file; removing the check fails the older-release test.

**Rob decides before step 3.3 (the fetch):** where releases are hosted, and when an
installed update reboots (note 6).

**Rob decides before the first real release:** the release key's custody, a backup key and
the release numbering, drafted for his answer as [ADR 0019](adr/0019-release-signing-key.md).

**Notes:**

1. **Files.**
   - `crates/morph-update`: a release number in the signed manifest, and the decision of
     what to do (a new `auto` subcommand, or a crate `morph-autoupdate` if `morph-update`
     gets too big).
   - `images/x86-qemu/build.sh`: writes the running release to `/usr/lib/morpheos/release`.
     It also needs a new scenario `autoupdate`: the `good` disk plus networking, the timer
     and the test CA.
   - `images/x86-qemu/build-payload.sh`: a third kind, `dir`, which leaves a signed payload
     *directory* for a server instead of an ext4 medium. It gets `bad` and `tampered`
     variants too.
   - A new overlay `images/x86-qemu/overlay-autoupdate/`:
     `usr/lib/systemd/system/morph-update-check.{service,timer}` and a test-only drop-in with
     short timer intervals.
   - `scripts/test-images.sh` (the `scenarios` group) and a new step in the x86 job of
     `.github/workflows/images.yml`.
   - Add the new binary to the `cargo build` lines in **both** files. The harness runs under
     sudo, whose PATH has no cargo.

2. **How payloads arrive today.** The `update` scenario does not use the network.
   `build-payload.sh` makes a separate **ext4** disk labelled `payload`, not a FAT one.
   `run-vm.sh` attaches it read-only as `/dev/vdb`. The self-test driver
   (`images/common/selftest-overlay/usr/lib/morpheos/test-hook`, step 0) mounts it under
   `/run/morph-media`, runs `/usr/bin/morph-update --esp /efi --root /dev/vda3 --payload … install`,
   and then runs `morph-boot trial b --reboot`. Only the self-test *report* goes on FAT (the
   ESP, `MORPH-TEST-RESULT.TXT`).
   - S3 replaces the medium and the driver with a server and a timer. `test-hook` must
     therefore **not** install or arm anything in `autoupdate`. Give it a step that only
     powers the machine off once `morph-boot status` says `b` is committed. In `bad` and
     `tampered` the harness stops the VM instead (step 4).

3. **How to approach it** (check first, AGENTS.md H3):
   1. **Release numbers, in `morph-update`, with tests.**
      - Today a correctly signed *old* payload installs. Nothing in `BLAKE3SUMS` says which
        release it is, so "the newest" cannot be decided and a replayed old payload is
        accepted.
      - Add one required line `release <n>` to the manifest. Today's parser refuses any line
        that is not `<hex>  <name>`, so the format must change on purpose; do not hide the
        number in a `#` comment, which the parser skips.
      - `install` refuses a release lower than `/usr/lib/morpheos/release` and allows an
        equal one, because `morph-heal` reinstalls the running release.
      - Tests to write: an older signed payload is refused before any write; a missing
        `release` line is refused; equal is allowed.
   2. **The decision, as a pure function with a table of tests.** Inputs: the boot phase, the
      running release, the server's release, the releases already tried, and whether a trial
      is armed. Outputs: skip (and why), or fetch.
      - **During a trial, always skip.** `morph_boot::update_target` refuses then anyway.
      - **No network, nothing newer, or a release already tried: exit 0.** A failed unit
        is an unhealthy boot (H19's other side). An updater that fails because there is no
        Wi-Fi would make the *next* trial fall back for a reason that has nothing to do with
        the update.
   3. **The fetch.**
      - First `BLAKE3SUMS` and `BLAKE3SUMS.sig`. `Payload::open` checks the signature and
        parses the manifest without hashing anything, so an old or untrusted release costs
        two small files.
      - Only then the large files, into `/persist/morph-update/staging/<release>/`.
      - Then `install`, then `morph-boot trial <slot>`.
      - Record `{release, slot, state}` in `/persist/morph-update/state.json` **before**
        arming the trial. After a fallback, the boot history says slot B failed and this
        record says which release was in it, so that release is marked tried.
   4. **The image and the VM scenario.** `good` first, then `bad`, then `tampered`, each in
      its own commit, each failing before its code exists.

4. **How the CI host serves it.** `test-browser.sh` already serves HTTP on `127.0.0.1:8123`,
   and QEMU's user network shows it to the VM as `10.0.2.2` (`VM_NET=1`).
   - For HTTPS, make a throwaway CA and a server certificate per run with
     `openssl req … -addext "subjectAltName=IP:10.0.2.2"`. Serve with a few lines of `python3`
     (`http.server` wrapped in `ssl.SSLContext`), logging every request to
     `$WORK/update-server.log`.
   - The tree: `stable/LATEST` (just a release number: a hint, trusted for nothing) and
     `stable/<release>/{BLAKE3SUMS,BLAKE3SUMS.sig,root.img,vmlinuz,initrd.img}`.
   - Only the `autoupdate` test image trusts the CA. Put it at
     `/usr/share/morpheos/update-test-ca.pem` and name it in the test drop-in, never in the
     real unit.
   - The server URL is a setting in the unit file. Do not read it from the Manifest's
     `system.channel`: that field is unenforced (H5), and S3 does not change that.

5. **Traps.**
   - **`Install-Recommends` is off in every base.** `curl` alone does not bring
     `ca-certificates`, so HTTPS to a real server fails on certificates while the test's own
     CA works. Name `ca-certificates` explicitly. (Or use a Rust client with rustls, which
     links no system library, as `zbus` and `ed25519-dalek` were chosen. Verify it builds on
     the pinned 1.99.0.)
   - **The base image has no network.** NetworkManager is only in the `browser` and `wifi`
     bases. Add an `autoupdate` base cached the same way (`network-manager`, `curl`,
     `ca-certificates`), and use `VM_NET=1`. `systemd-networkd-wait-online` is removed by
     `build.sh`, so order the check after `NetworkManager-wait-online.service` or `nm-online`.
   - **The data partition is 256 MiB** (`DATA_MB` in `build.sh`). The payload's root image is
     the shrunk slot root, about 450 MB of content (Measured: E12's "root 450 of 1200 MB"), so
     it does not fit. Raise `DATA_MB` for this scenario (`morph-heal` puts its staging in
     `/run`, a tmpfs, only because it symlinks the running root; a download cannot). Verify:
     the size `build-payload.sh` prints (`payload root image: X -> Y bytes`).
   - **Do not reuse `/run/morph-heal`** (a tmpfs): the VM has 1 GiB of RAM, and this is how
     `morph-heal` found that a copy is wrong.
   - **The timer must not start during startup's own health window.** `boot-check` waits on
     `systemctl is-system-running --wait`. Use `OnBootSec=` comfortably past it (production
     minutes; test about 20 s after the decision line) plus `RandomizedDelaySec=`.
   - **After arming, the timer fires again before the reboot.** The recorded state must make
     it skip; otherwise it rewrites slot B and re-arms in a loop.
   - **Read the verdict from the disk** (`mtype` on `history.jsonl`, `debugfs` on `p3` as
     `marker_present` does) and the server log. Never from the console (H8). Wait for the
     `morph-boot-check: decision` line that writes the history before stopping the VM (H13).
     Guard every read (H11).
   - **Payload size is time.** Install cost is about 3x the image size (Measured, run
     38117817003: 3.8 s → 11.2 s for a 1200 MiB image, before the shrink).

6. **Rob's decisions.**
   - **Where releases are hosted.** Recommend GitHub Releases or any static HTTPS host: A4
     needs no server code.
   - **The release key and its custody** (already open since S2).
   - **When an installed update reboots.** Options: immediately; at the next boot the person
     makes; or when idle. Recommend the next boot plus a shell notice, so there are no
     surprise reboots. The test drop-in sets "reboot now".

7. **Size: L.** Four PRs, each green on its own:
   1. Release numbers and anti-rollback in `morph-update`, with tests.
   2. The decision function and the fetcher, with tests against a local server in `cargo test`.
   3. The image and `test-autoupdate.sh good`.
   4. `bad` and `tampered`.

### S4 · The recovery image (rung H4) · open

**What:** a third, immutable system, the recovery image. It is a known-good, signed copy of
the system on its own partition, with its own boot entry, written only when the disk is made.
When a slot's root is missing or cannot start, the machine boots the recovery image instead of
hanging. The recovery system checks itself, rewrites **both** slots and their boot files from
itself with `morph-update`'s verified write, makes slot A the default, and reboots. It never
writes `/persist`.

**Done when:** `images/x86-qemu/test-recovery.sh` writes a known set of files under `/home`,
then wipes the root filesystem of both slots. With no person involved, the machine boots the
recovery system, rebuilds both slots and boots slot A committed (doc 15, demo step 1). Read off
the disk after the VM stops:
- the boot history ends `a/committed/none`;
- a recovery record on the ESP names both slots as rebuilt;
- both slot partitions hold a valid filesystem again (`debugfs` finds `/etc/hostname` on `p2`
  and `p3`);
- every file under `/home` matches its checksum from before the wipe.

Also: a recovery image whose own digest does not match is refused, and nothing is written.

**Rob decides before the recovery image ships:** its size, and whether an update may refresh
it (note 5). Neither blocks the VM proof.

**Notes:**

1. **What exists.**
   - ADR 0009: "the mechanism exists, unwired". `morph-update install` writes and verifies a
     slot.
   - `morph-heal repair` already rebuilds *one* slot from a running root image without
     copying it: it symlinks the block device in as `root.img`, seals and signs with a key
     made for that one repair, and installs. Recovery is the same move with the recovery
     partition as the source and both slots as targets.
   - The rung's time is a Target (~2–5 min, doc 15). Measure it in this task and record it.

2. **Files.**
   - `images/x86-qemu/build.sh`: partition 5 `recovery`, the same size as a slot, holding
     the factory root. Its kernel and initrd go in `esp/morpheos/recovery/`, with an entry
     `loader/entries/morpheos-recovery.conf`:
     `root=PARTLABEL=recovery ro morph.recovery=1 systemd.unit=morph-recovery.target`.
     Also a signed `BLAKE3SUMS` and `.sig` for the recovery image beside its kernel.
   - The recovery root gets a unit `morph-recover.service` and `morph-recovery.target`, so
     `boot-check`, the shell and Strata do not run there.
   - `crates/morph-heal`: a `recover` command.
   - `crates/morph-boot`: a recovery state the policy knows about (step 3.2).
   - An initramfs-tools hook in `images/common/overlay/etc/initramfs-tools/` (step 3.3).
   - `test-recovery.sh`, in the `scenarios` group of `scripts/test-images.sh`, and a step in
     the x86 job.

3. **How to approach it.**
   1. **Boot the recovery system by hand first.** In a `recovery-boot` scenario the harness
      arms the recovery entry the way `morph-boot` arms a trial (`bootctl set-oneshot`). The
      check: the history is unchanged, and the recovery's own record is on the ESP. This
      proves the partition, the entry and the target before anything writes.
   2. **The policy.** Today `booted_slot` comes from `morph.slot=` and `update_target` only
      ever allows the other slot. In recovery neither slot is running, and both may be
      written.
      - Add this to `morph-boot` as a separate, tested function. It requires `morph.recovery=1`
        on the command line **and** `/` being the `recovery` partition (from
        `/proc/self/mountinfo`).
      - Never add it as a flag on `install`: the invariant belongs where it cannot be bypassed
        (H7).
      - After writing A and B, set the default to A. Say plainly in the code that "A is the
        signed factory image" is a policy property, not a proof that A boots on this machine
        (H7).
   3. **The way in.** This is the trap that decides the design (step 4). Recommended: an
      initramfs hook in every slot's initrd. When the slot's root partition has no
      filesystem, or has no `/sbin/init` once mounted, it mounts `efivarfs` and the ESP, runs
      `bootctl --esp-path=… set-oneshot morpheos-recovery.conf` (`bootctl` copied in with
      `copy_exec`), and runs `reboot -f`. This is the platform's one-shot, as ADR 0008
      chose.
      - The alternative: the hook pivots straight into the recovery partition. It needs no
        EFI variable, but the command line then says `morph.slot=a` while running recovery,
        and every tool that reads it would be wrong.
   4. **The wipe, in two steps.**
      - First from the harness, between two VM runs: run 1 writes the files and records
        their checksums on the ESP; the harness zeroes the first 64 MiB of `p2` and `p3`
        with `dd … conv=notrunc`; run 2 must heal. This is deterministic.
      - Then from inside the running machine (`test-hook`: `wipefs -a` and zero both slot
        partitions, then `echo b > /proc/sysrq-trigger`). Verify: Ubuntu 24.04's 6.8 kernel
        lets root write a block device whose filesystem is mounted
        (`CONFIG_BLK_DEV_WRITE_MOUNTED`, default y).
   5. **Self-check.** `recover` hashes the recovery partition's prefix against its signed
      `BLAKE3SUMS` before writing anything. A test flips one byte of the recovery partition
      and expects refusal, an untouched `p2`, and a console message instead of a reboot loop.

4. **Traps.**
   - **A missing root does not panic: initramfs-tools drops to an `(initramfs)` shell and
     waits for ever.** systemd has not started, so `RuntimeWatchdogSec` has armed nothing
     and the i6300esb never fires. Verify: whether Ubuntu 24.04's initramfs-tools honours
     `panic=` (already on our command line) to reboot instead of opening a shell (read
     `/usr/share/initramfs-tools/scripts/functions` in the base rootfs). Either way a
     reboot alone loops on slot A for ever; the hook is what breaks the loop.
   - **Hook order.** `local-top` runs before the root device is awaited; `local-bottom` runs
     after it is mounted. A wiped partition needs the first, and an emptied filesystem (no
     init) needs the second. Verify both against the scripts in the image.
   - **ESP space.** The ESP is 300 MB and holds two kernels and initrds already. Measure
     `du -sh "$WORK/esp"` before adding a third copy, and raise `ESP_MB` if needed.
   - **Disk size.** A third 1200 MB partition makes every test disk bigger, and CI disk is
     limited (the job already drops the raw `plain` image for room). Build the recovery
     partition only for the scenarios that need it until it is part of `plain`.
   - **The recovery root must not mount `/persist`.** The slots' `fstab` mounts it read-write
     and bind-mounts `/home` and the Strata store, so recovery needs its own `fstab`.
   - **The demo's wording.** A literal `rm -rf /` on a running MorpheOS cannot touch the
     read-only slot. It *does* reach `/persist` (`/home` **and** the Strata store under it)
     and the ESP, which is mounted read-write. So it deletes the files and their history.
     This test's "system root" is the two slot partitions.

5. **Rob's decisions** (none blocks the VM proof).
   - **How big the recovery partition may be.** Recommend the size of a slot.
   - **Whether a signed update may refresh it.** Recommend: factory only in v1. A refresh is
     its own later task, because a bad refresh removes the last rung below Phoenix.
   - **Demo step 1's wording.** Recommend "wipe both system slots", until `/persist` and the
     ESP are protected from root, which is a security-model change.
   - Doc 00's "30 seconds" stays unsupported (ADR 0009). Record the Measured time here.

6. **Size: L.** Four PRs:
   1. The partition, the entry, the target, and `recovery-boot`.
   2. The `morph-boot` recovery policy and `morph-heal recover`, with unit tests.
   3. The initramfs hook and the harness-side wipe scenario.
   4. The in-machine wipe and the corrupt-recovery refusal.

### S5 · Accounts and admin (A9) · proposed

**What:** an ordinary account a person logs in as, a locked root, and admin actions only
through `morphd` transactions with the `.rob` mark (A9, ADRs 0011 and 0012). Rob chooses the
model first. Three are laid out below, and drafted for his answer as
[ADR 0020](adr/0020-account-model.md) (Proposed).

**Done when:** Rob has decided the account model, and a VM scenario
(`images/x86-qemu/test-accounts.sh`) logs in as the user with the model's credentials, typed on
the VM's keyboard. As that user it cannot write the slots: writing `/dev/vda2`, `/dev/vda3` or
`/efi/loader/loader.conf` fails. Read off the disk afterwards, slot B and `loader.conf` are
byte-identical to before (a control taken before boot), and root's password is locked.

**Ask Rob to open:** E11's first half (the shell not as root) waits on the account model,
drafted for his answer as [ADR 0020](adr/0020-account-model.md).

**Notes:**

1. **What is there today.**
   - There is no ordinary account at all. Everything runs as root, including the shell
     (`overlay-shell/…/flow-shell.service`: "ROOT FOR NOW") and Firefox.
   - The images log root in automatically: `overlay-hardware` autologins root on `tty1` and
     the serial console, and `overlay-shell` on VT2.
   - `test-boot.sh` *requires* a completed root login (`MORPHEOS CONSOLE READY` from
     `/root/.profile`).
   - `/home/morph/Documents` exists on the data partition but belongs to no user.
   - The Manifest has `[[users]]` (`name`, `admin`, `recovery_key`), validated
     (`INV-ADMIN`, `INV-RECOVERY-KEY`, risk tier 4 for `/users`). **Nothing creates an
     account from it**. `/users` is listed in `unenforced_settings()` (#67); it leaves the
     list only when something creates the account from it.

2. **Three models.**
   - **(1) One person, no password: an appliance.**
     - One account `morph` (uid 1000) is baked into the image, and it autologins into the
       shell.
     - Admin is physical presence plus the `.rob` mark. Root is locked, and there is no
       `sudo`.
     - Smallest, because the account database can stay in the read-only root.
     - Against it: whoever holds the laptop is its owner. It also cannot fit doc 09's
       "keys unlocked by user auth", so it would have to be replaced before disk encryption.
   - **(2) One owner, with a password chosen at first boot. Recommended.**
     - The account lives in a writable database on `/persist`, through `libnss-extrausers`
       (`/var/lib/extrausers/{passwd,shadow,group}`, bind-mounted from `/persist`). Ubuntu
       Core uses this for exactly this reason: a read-only root.
     - First boot shows one window in the shell: a name and a password. The Manifest's
       `[[users]]` records the name and admin flag (tier 4, `.rob`), never the password.
     - Login is the shell's own lock and login screen.
     - Admin actions go through `morphd`, which asks for the same password and writes the
       `.rob` mark with the user's name.
     - Root is locked. No `sudo`. No root console on VT2 in `plain`.
     - Grows into (3) without a rewrite.
   - **(3) Several people, an owner who is admin.**
     - As (2), with a user list at login and per-user sessions (`flow-session`).
     - Possibly `systemd-homed`, with per-user encrypted homes on `/persist/home`. Verify that
       noble ships it, and its PAM module.
     - Strata history per user.
     - Right for a family machine, and a week or more on top of (2). Not needed for N1–N5.

3. **Files** (model 2).
   - `build.sh`: add `libnss-extrausers` to the base, and set `passwd: files extrausers` in
     `/etc/nsswitch.conf`.
   - PAM, so that `pam_extrausers` is used for authentication.
   - `fstab`: a bind of `/persist/extrausers` onto `/var/lib/extrausers`.
   - `passwd -l root` at build time. Remove `sudo` if anything pulls it in.
   - Chown `/home/morph` to uid 1000 in `DATADIR`.
   - Drop the VT2 and `tty1` root autologins from `plain`, but keep them in the test images
     that need a console.
   - `test-boot.sh`: change its root-login check to the new login; never delete it.

4. **The verdict.** A login typed through QMP (`send-key`, as `test-wifi.sh` types a password)
   on a getty or the shell's login screen. Then a hook, *as that user* (`runuser` is not a
   login; the typed session is), tries the three writes and logs what it saw. The verdict is
   the disk: a checksum of `p3` and of `loader.conf` before boot and after poweroff. A
   refused write that left no trace and a write that "failed" after landing look the same in
   a log (H8).

5. **Traps.**
   - **`/etc` is read-only.** `useradd`, `passwd` and `chpasswd` lock and rename files in
     `/etc` and fail. This is why extrausers, and not a bind mount over `/etc/shadow`.
   - **Root autologin on VT2 is a root shell for anyone at the keyboard.** HARDWARE.md tells
     people about it. Removing it from `plain` takes Rob's real-hardware debugging console,
     so ask him (model 2 recommends a recovery console in S4's recovery system instead).
   - **The block devices are `root:disk 0660`** and the ESP is mounted `umask=0077`. Keep
     the user out of the `disk` group. Assert this in the test, not by reading
     `/etc/group`.
   - **E11 follows.** The shell as this user needs a seat (verify: Ubuntu's `seatd.service`
     uses `-g video`, so the user goes in `video`) and a logind session
     (`PAMName=login` in the unit, for `XDG_RUNTIME_DIR=/run/user/1000`).
   - **Strata's store stays root-only (0700) by design** (doc 09). S6's timeline needs a
     privileged path once the shell is not root.

6. **Rob's decision: the model.** Recommendation: **(2)**. Also: whether `plain` keeps any root
   console. And whether the password also unlocks disk encryption later (doc 09; out of
   scope here).

7. **Size: M** once decided. Two PRs:
   1. The account, locked root and `test-accounts.sh`.
   2. The first-boot window and the shell's login screen.

### S6 · Rewind from the shell · open · needs E3

**What:** a timeline window on the Flow shell, `flow-timeline`, built the way `flow-wifi` is. It
lists the moments a person can go back to: Strata's ransomware alerts first, each with its
folder, file count and rewind point. Choosing one and confirming it runs Strata's rewind and
shows what came back. It opens by itself when Strata raises an alert. No terminal is involved
(N3).

**Done when:** `images/x86-qemu/test-rewind.sh` boots a shell image whose self-test attacks
`/home/morph/Documents` as `test-ransomware.sh`'s does, but does **not** rewind. The timeline
window opens by itself. The harness chooses the alert and confirms it with the VM's keyboard,
and a screendump shows the window before and after.

After poweroff, read off the data partition as `test-ransomware.sh` reads it: all 24 documents
match their checksums from before the attack, no `.locked` file or ransom note is left, and the
`.locked` files are in Strata's quarantine. The quarantine folder's timestamp is later than the
keypress that confirmed, so the shell did the rewind and not a script. The boot is committed.

**Notes:**

1. **Files.**
   - New `crates/flow-timeline`, the same shape as `crates/flow-wifi`:
     - `src/timeline.rs`: the model, what the person sees and does, with no Wayland in it,
       unit-tested anywhere.
     - `src/backend.rs`: a trait, plus a `strata` backend using the `strata` library
       (`Strata::alerts`, `Strata::rewind(dir, at, dry_run)`) and a fake.
     - `src/main.rs`: the window, a plain Wayland client drawing with `flow-shell`'s font,
       as `flow-wifi` does.
     - `tests/on_the_shell.rs`: drives it on the headless compositor through `--script`.
   - `images/x86-qemu/overlay-shell/…`: `flow-timeline.service`, plus a `.path` unit on
     `/var/lib/strata/alerts.jsonl` (`PathModified=`) that starts it. Verify that a path
     unit fires through the `/persist/strata` bind mount.
   - `build.sh`: a scenario `rewind` (`SHELL_UI=1`, the ransomware hook with
     `MORPH_TEST_REWIND=no`).
   - The `shell` group of `scripts/test-images.sh` and a step in the x86 job.
   - Move `test-ransomware.sh`'s document checks into a shared file both tests source, so the
     two verdicts cannot drift apart.

2. **How to approach it.**
   1. The model and its unit tests first: list, select, the two-step confirm, the result
      line.
   2. The fake-backend test on the compositor, with screendump colour bands for each step,
      as `flow-wifi`'s test does.
   3. The VM scenario last. Its first run must fail on the missing window before the
      service exists.
   - Do a dry run (`rewind … dry_run=true`) before the confirm, and show its counts ("24
     files will come back, 25 will go to quarantine"). The confirm then runs the same plan.

3. **The verdict.** Use `test-ransomware.sh`'s disk reads (`dd` the data partition out, then
   `debugfs rdump` and `sha256sum -c`). Screendumps decide what the person saw. The window's
   log lines (`flow-timeline: points …`, `flow-timeline: rewound …`) only say *when* to look
   (H8, H13).

4. **Traps.**
   - **The store is root-only (0700, doc 09)** and the window runs as root, like the shell,
     until E11. Keep every Strata call behind the backend trait, so that a non-root shell
     only swaps in a broker (a small root service on a socket). It must never change the
     store's permissions.
   - **Rewind needs the alert to be recorded first.** `ransomware-test` waits for Strata to
     record the files (`sync_strata`) and for the alert before it does anything else; keep
     that.
   - **Several folders.** Alerts can name several folders (`clients/` here). One confirm
     must rewind every folder the alert names, as the CLI loop does.
   - **H17:** the window must keep answering frame callbacks while the rewind runs. Do the
     rewind off the draw loop and paint progress.
   - **H14:** after the window opens, wait for its "ready" line before sending keys.

5. **Rob's decisions:** none for this slice. Deleted and overwritten files (not only alerts)
   are D5's file view.

6. **Size: M.** Two PRs:
   1. The crate, its model and the compositor test.
   2. The image, the path unit and `test-rewind.sh`.

### S7 · A full disk does not break the machine · open · needs S11

**What:** when `/persist` fills, the machine keeps working and recovers space by itself. Every
boot is still healthy. Strata keeps going and does not crash-loop. A space guard prunes
Strata's old versions by the retention policy in the Manifest, and the shell shows a warning
that says how full the disk is and what was freed.

**Done when:** a VM scenario fills `/persist`; the machine still boots healthy, Strata prunes
by its retention policy, and the shell warns (doc 15, step 4). Made exact: in
`images/x86-qemu/test-full-disk.sh`, root fills the data partition until a write fails with
ENOSPC, and the machine reboots.
- The boot history shows that boot `committed` and healthy, with `strata.service` not among
  any failed units.
- Strata versions older than the policy are gone and the newest version of every file is kept
  (read with the host's own `strata --store <copied store> stats` and `history`).
- Free space on the data partition is back above `storage.min_free_percent`, or the warning
  says why not (read with `dumpe2fs` on the copied-out partition).
- A screendump shows the warning.

**Notes:** "Strata prunes by its retention policy" waits on S11: `storage.retention` is parsed
and validated, and nothing obeys it yet (AGENTS.md H5).

1. **Files.**
   - `crates/strata`: a space guard, either `strata guard` or a check in the `watch` loop. It
     reads free space (`statvfs`) and, below the floor, runs `prune` with the policy S11
     materialises.
   - A small notice window, `crates/flow-notice` (one message, one colour, the `flow-wifi`
     shape). S8 reuses it.
   - `images/x86-qemu/build.sh`: a scenario `full-disk` (`SHELL_UI=1`).
   - `run-vm.sh`: a `VM_RTC=` variable (step 3).
   - The new test in the `scenarios` group of `scripts/test-images.sh` and a step in the x86
     job.

2. **How to approach it.** Check first: make the scenario boot with a full `/persist` before
   writing any guard, and record what breaks. Expect `strata.service`, and NetworkManager in
   the network images. Then:
   1. Make Strata survive ENOSPC.
   2. Add the guard.
   3. Add the notice.

3. **Making old versions in a VM that runs for minutes.** The policy keeps *every* version
   younger than `all_versions_hours` (24 by default), so in a fresh VM nothing is old enough
   to prune. Do it in two VM runs:
   - Run 1 with `-rtc base=<30 days ago>` writes several versions of a few files, which Strata
     stamps with that date.
   - Run 2 with today's date fills the disk.
   - Verify: systemd moves the clock forward to its own build date when the RTC is earlier;
     30 days ago is well after noble's systemd was built.

4. **Traps.**
   - **ext4 keeps 5% for root.** A fill run as an ordinary user stops short and never
     touches the root-run services. Fill as root (`fallocate` or `dd` until ENOSPC), or
     the test proves nothing.
   - **Strata on a full disk.** SQLite cannot grow its journal (`SQLITE_FULL`). If
     `Strata::open` fails, `strata.service` restarts every 2 s (`RestartSec=2s`). systemd's
     default start limit (5 starts in 10 s) can then make it a **failed unit**, which is an
     unhealthy boot. On a committed boot that is only recorded; during an update trial it
     falls back for a reason that is not the update's. `watch` already prints snapshot
     errors and carries on (`Event::Error`). Make `open` and the first scan do the same on
     ENOSPC, and say so once.
   - **Never prune beyond the policy, or the newest version.** `retention.rs` promises the
     newest version of every path is always kept. If the policy cannot free enough, warn; do
     not delete more.
   - **`storage.min_free_percent` is unenforced.** Nothing reads it, and
     `unenforced_settings()` lists it (#67). Obey it here (through S11's `morphd`) and take it
     off the list in the same PR.
   - The boot history lives on the ESP, not `/persist`, so the verdict survives a full data
     partition. Keep it there.

5. **Rob's decisions.** Whether anything may go beyond the retention policy when the disk is
   still full. Recommendation: no, only warn. "Caches are evicted" (doc 15) has no cache to
   evict until Flatpak (D4) and the browser cache.

6. **Size: M.** Two PRs:
   1. Strata survives ENOSPC, and the guard, with unit tests.
   2. `flow-notice` and `test-full-disk.sh`.

### S8 · A corrupted setting rolls back (rung H2) · open · needs S11

**What:** H2 is rolling back the one offending configuration change. When the Manifest
generation in force is corrupted, so that it no longer parses, no longer validates, or no
longer matches what was approved, `morphd` takes the machine back to the newest good
generation. It does this through `morph-transaction` and `Store::rollback`, which only moves
`current` and deletes nothing. It records which generation, and so which transaction, was
the cause. The shell names it.

**Done when:** a VM scenario corrupts a Manifest-managed setting, and `morph-transaction`
rolls it back and names the transaction that caused it (doc 15, step 6). Made exact: in
`images/x86-qemu/test-config-rollback.sh`, the self-test applies generation N (a new hostname)
through `morphd` and it commits. It then corrupts generation N's stored setting by hand,
writing to the file, not through `morphd`, and reboots.

Read off the data partition:
- the store's `current` names N-1;
- generation N is still in the store;
- a record names N, what was wrong with it, and N-1 as the generation now in force.

Also: the kernel's hostname is N-1's again (written to the ESP by the hook), and a screendump
shows the notice naming N.

**Notes:** waits on S11 (proposed). Nothing on the machine reads the Manifest yet (AGENTS.md
H5), so there is no Manifest-managed setting to corrupt, and ADR 0009 says H2 is not to be
attempted before the transactional apply exists.

1. **What "corrupted by hand" means here: the plan owner should confirm.**
   - The recommended reading is above. The generation in force is damaged on disk, and
     `morphd` finds it at the next boot.
   - The store keeps no digest of a generation today (`Generation` has number, risk, changes,
     approved_by), so add one. A generation whose file no longer matches the digest recorded
     when it was applied is corrupt even if it still parses.
   - The fuller H2 (doc 15, "change correlation") is a generation that applied cleanly and
     breaks something later, rolled back because a health signal started failing after it.
     That needs health signals beyond what S11 checks. Propose it as its own task rather than
     stretching this one.

2. **Files.**
   - `crates/morph-manifest/src/store.rs`: record a BLAKE3 digest in the meta file at
     `apply`, and check it in `load`. Old stores without one are read as "unknown", not as
     corrupt, with a test for each.
   - `crates/morphd` (S11): at start, load `current`. If it fails to parse, validate or match
     its digest, roll back to the newest generation that passes all three, apply its settings,
     and append `{rung:"H2", bad:N, reason, now:M, at}` to `/persist/morph/heal.jsonl`.
   - `crates/flow-notice` (S7) shows the message.
   - The test, in the `scenarios` group and a step in the x86 job.

3. **How to approach it.**
   1. Unit tests in `morph-manifest` and `morphd` first: corrupt TOML, a valid TOML that fails
      validation, and a valid one with the wrong digest. Each rolls back and names N.
   2. A store with only one generation refuses and reports it. It does not apply a default.
   3. Then the VM scenario.

4. **Traps.**
   - **Corrupting the file by hand through the bind mount.** The hook runs as root and the
     store is on `/persist`, so writing the file is easy. The trap is `current` itself: it
     is a symlink renamed into place, so corrupt the generation *file*, not the link, or the
     test proves a different failure.
   - **"Names the transaction"** must come from the store's own record (number, risk,
     approved_by, the `.rob` mark if any), not from a log line (H8).
   - **H12:** after the rollback, the next generation is `next_number()`, never `current + 1`.
   - **Do not delete N.** A rollback that deletes cannot be undone (`store.rs` property 3).

5. **Rob's decisions:** none, beyond opening S11.

6. **Size: M.** Two PRs:
   1. The digest and `morphd`'s check, with unit tests.
   2. The scenario and the notice.

### S11 · The Manifest is obeyed: `morphd` v0 applies its first settings · proposed

**What:** the smallest `morphd` (a service under systemd, ADR 0017) that makes the current
Manifest generation real on the machine, through `morph-transaction`: plan, apply, health
check, commit or roll back. It applies only settings the machine can obey today, each one
moving out of `unenforced_settings()` as it does: `machine.name` as the hostname, and
`storage.retention` as Strata's pruning policy. Both images carry it.

**Done when:** in a VM, a new Manifest generation changes the hostname and the retention, and
the machine shows both (read from the machine, not from `morphd`'s log); a generation whose
health check fails is rolled back and the old values are back; `unenforced_settings()` no
longer lists `/storage/retention`. Made exact: in `images/x86-qemu/test-manifest.sh`, a hook
writes the kernel's hostname (`/proc/sys/kernel/hostname`) and the policy Strata itself reports
to a file on the ESP after each step. The test reads that file with `mtype`, and reads the
store's `current` off the data partition.

**Ask Rob to open:** S7 and S8 need it, and nothing inside it changes the security model.

**Notes:** proposed 2026-10-14 by Claude Code, while picking up S8. It unblocks S8 (a
Manifest-managed setting to corrupt) and S7 (Strata pruning by `storage.retention`). Rob opens
it or not; it is the first time the Manifest drives the machine, which is why it is a task of
its own rather than a part of S8.

1. **Files.**
   - New `crates/morphd`: a library (the "settings appliers", one per setting, each with
     `apply`, `read_back` and `undo`) and a binary.
     - `morphd boot` converges the machine to `current` at every start, and first resolves an
       interrupted transaction (the marker `morph-transaction` leaves).
     - `morphd apply <new.toml> [--approved-by WHO]` runs one transaction.
   - The health check is `read_back` equalling the generation, plus `strata.service` active.
   - The store and the transaction live in `/persist/morph/`.
   - `crates/strata`: `strata prune` reads a policy file that `morphd` writes
     (`/persist/morph/strata-retention.json`) when no flags are given, and a new
     `strata policy` prints the policy in force.
   - `crates/morph-manifest/src/validate.rs`: remove `/storage/retention` and `/machine/name`
     from `unenforced_settings()`. Its test (and the `W-UNENFORCED` count) changes with it.
   - `images/common/overlay`: `morphd.service` (`Type=oneshot`, `RemainAfterExit=yes`,
     `RequiresMountsFor=/persist`, `Before=strata.service`).
   - `build.sh`: installs `morphd`, and seeds generation 1 into `DATADIR/morph/` with the host's
     `morph-manifest apply --store` from a new `images/x86-qemu/manifest/machine.toml`. It must
     validate, so it needs an admin user and a recovery-key holder.
   - Add `morphd` and `morph-manifest` to the `cargo build` lines in `images.yml` and
     `scripts/test-images.sh`.

2. **How to approach it.**
   1. The hostname applier first, with a fake system in unit tests: apply, read back, undo,
      and a failing apply that is undone.
   2. Then retention.
   3. Then the service and `test-manifest.sh`. The first transaction has to exist before a
      rollback can (`Transaction::begin` refuses without one).
   - The failing generation in the test should fail for a real reason, not a test switch.
     Example: a `machine.name` longer than the kernel's 64-byte limit, which `validate`
     accepts today (it only checks for empty) and `sethostname` refuses. If `validate` gains
     that rule, which it should, pick another fault the validator cannot see.

3. **Traps.**
   - **`/etc/hostname` is in the read-only root** (`build.sh` writes `morpheos-<slot>`).
     `hostnamectl set-hostname` writes it and fails. Set the kernel's hostname directly
     (`sethostname`, or `hostnamectl --transient`), and re-apply it at every boot.
   - **`/storage/retention` is tier 3 (DataAffecting, `plan.rs`).** A generation that
     changes it needs a person's approval (`--approved-by`, which writes the `.rob` mark).
     The test supplies one, and that is the only place a test name appears. `/machine/name` is
     tier 1.
   - **A `morphd` failure is a failed unit, and a failed unit is an unhealthy boot.** That is
     right when a new slot's `morphd` is broken. It is wrong when the Manifest on `/persist`
     is bad. In that case `morphd boot` must roll back and succeed (that is S8), not fail.
   - **No test depends on the hostname today** (`test-boot.sh` waits for
     `MORPHEOS CONSOLE READY`, not `morpheos-a login:`), but grep again before changing it.
   - **The store is single-writer by contract** (`store.rs`). `morphd` is the only writer,
     and the hook calls `morphd apply`, never `morph-manifest apply` on the live store.

4. **Rob's decisions:** opening the task. Nothing inside it changes the security model.

5. **Size: M–L.** Three PRs:
   1. `crates/morphd` with the hostname applier and tests.
   2. Retention, Strata's policy file and the `unenforced_settings()` change.
   3. The service, the seeded generation and `test-manifest.sh`.

## M3 · Daily driver

Each task is expanded in M3's planning PR. *Done when* is the shape of the check.

**Order of work (M3).**

M3 starts when M2 is done. Each step below is one branch and one pull request unless it says
otherwise; Rob merges every one. What M3 takes from earlier milestones:

| From | What M3 uses it for |
|---|---|
| E4 (`plain` boots into the shell) | every D task ships in `plain`, the image a person boots |
| E5 (the bar at the bottom, `--launch-wifi`) | the place D1's settings, D2's volume and D3's battery and suspend button live |
| E11 / S5 (the person's account) | until they land, every daemon below is driven as root. After them, NetworkManager, Flatpak, CUPS and BlueZ ask **polkit**, which is not in the image (E5's notes). Each D task says what changes then |
| S6 (rewind from the shell) | D5 reuses its timeline view |
| S2 (signed updates) and the `update` scenario | D4 proves a Flatpak survives a real A/B update |
| E6 (the reference laptop) | D3 and D7 are proven by a person on it |
| E7 (the installer) | the data partition's size: on the stick it is 256 MB (`DATA_MB`), which no real Flathub app fits |

1. **One desktop base first** (part of whichever of D2, D4 or D6 starts first, S). Today each
   image family has its own cached `mmdebstrap` root (`rootfs-base`, `rootfs-browser`,
   `rootfs-wifi` in `images/x86-qemu/build.sh`), and each costs a full bootstrap. Add one
   `DESKTOP_PKGS` list (PipeWire, CUPS, BlueZ, Flatpak, UPower, power-profiles-daemon) that
   the M3 test scenarios share, and that `plain` takes when the last D task lands. Measure the
   root after each addition (`du -sm`), record it in the PR, and raise `ROOT_MB` (1200 today)
   only on purpose.
2. **In parallel from the start, in any order:** D2 (sound), D4 (Flatpak), D5 (Files) and the
   service half of D6 (CUPS and the virtual printer, proven with `lp`). None needs another
   D task.
3. **D1 after D2.** D1's sound page is D2's volume control. D1 is also the window D6's
   "add a printer" and D7's pairing pages live in, so D6's and D7's shell pages come after
   D1's first PR.
4. **D3 and D7: the builder's half in parallel with everything** (the report gains its
   sections, the VM scenario), and then Rob's half on the reference laptop (E6).
5. Each new VM scenario is a new step in the x86 job of `.github/workflows/images.yml` and a
   line in `scripts/test-images.sh` (it says to keep the two in step; add an `m3` group
   there). That job has a 90-minute timeout and is a required check (AGENTS.md H10). Seven
   scenarios at a few minutes each with KVM (Target) may push it near the limit: raise the
   timeout or split the job, and make a split job a required check in the same change.
6. **A shared harness helper.** `test-shell.sh` and `test-wifi.sh` each carry their own copy
   of the QMP client, `shot`, `count`, `click` and `type_text`. The first M3 scenario moves
   them into `images/x86-qemu/vmtest.py`, used by the new scripts; leave the old ones as
   they are (a merge that rewrites a passing test is the quiet way to lose one).

**A6 says the shell drives these services over D-Bus. That holds for NetworkManager, BlueZ,
UPower, power-profiles-daemon and logind. It does not hold for PipeWire or CUPS:** neither
has a D-Bus control API. PipeWire is driven over its own protocol (`wpctl`, or libpipewire),
and CUPS over IPP (`lpadmin`, `lp`). D2 and D6 say what to use; the plan owner may want A6's
wording corrected.

**Waiting on Rob in M3:** D1, whether settings go through the Manifest (S11, `proposed`) or
straight to each service; D2, the audio daemons as root until E11; D4, which Flathub (all,
or the verified subset), and whether Firefox moves to a Flatpak; D6, whether the machine
listens for printers on the network (mDNS); D7, which device he pairs, and where pairing keys
are kept; D3 and D7, the runs on the reference laptop.
### D1 · Settings in the shell (Wi-Fi, display, sound, power) · open
**What:** a Settings window on the shell, opened from the bar (a button next to E5's Wi-Fi
button, and Super+I), with four pages: **Wi-Fi** (on or off; the network list is the
existing picker, `flow-wifi`, opened from here), **display** (the screen's resolution),
**sound** (output volume and mute, D2's control), **power** (the power profile: power saver
or balanced, and performance where the machine has it). Every choice outlives a reboot and
an update (A3: it lives on `/persist`).

**Done when:** `images/x86-qemu/test-settings.sh`, in the `images` workflow, opens Settings
by a click on the bar and changes each setting by scripted input, each read from the other
end: Wi-Fi off makes the simulated access point drop the machine (hostapd's
`AP-STA-DISCONNECTED`), and on brings a new lease (dnsmasq's `DHCPACK`); the resolution
change makes QEMU's `screendump` 1024x768 instead of 1280x800, with the shell's background
over most of it; the volume at 50% makes the tone QEMU captures quieter by the measured
ratio (D2); the power profile is `power-saver` in power-profiles-daemon's own state file,
read off the data partition after power-off. After a reboot all four are still in effect,
and every boot is healthy. (Sharpened: the script, what each verdict is read from, and the
reboot.)

**Notes:**
1. **Files:** a new crate `crates/flow-settings`, shaped like `crates/flow-wifi`: a model
   with no Wayland and no D-Bus in it (`pages.rs`, unit-tested anywhere), a `Backend` trait
   with a fake for the compositor tests, the real backends (`nm.rs`, `ppd.rs`, `audio.rs`)
   under `cfg(target_os = "linux")`, and the window in `main.rs` (zbus, as `flow-wifi`).
   The compositor gains `--launch-settings CMD` beside E5's `--launch-wifi` (the same
   one-instance and reap rules). The test is `crates/flow-settings/tests/on_the_shell.rs`
   (headless, fake backend), then the VM script. The image scenario `settings` is the
   `wifi` scenario's base (it has hostapd, dnsmasq and `mac80211_hwsim`) plus the desktop
   packages.
2. **The APIs:**
   - Wi-Fi: NetworkManager's `WirelessEnabled` property on `/org/freedesktop/NetworkManager`
     (`org.freedesktop.NetworkManager`), the same bus `flow-wifi` already uses.
   - Power: power-profiles-daemon's `ActiveProfile` and `Profiles` properties. Ubuntu 24.04
     ships 0.21, on both `net.hadess.PowerProfiles` and `org.freedesktop.UPower.PowerProfiles`
     (verify: `busctl --system list`). Use the second, the new name. In a VM there is no
     cpufreq driver, so it offers only `power-saver` and `balanced`: the page lists what
     `Profiles` says, never a fixed three.
   - Sound: D2's backend (`wpctl`, see D2).
   - Display: no D-Bus API exists for the compositor. Options: (a) the
     `wlr-output-management-unstable-v1` protocol (the `wayland-protocols-wlr` crate, server
     side), the standard one that `wlr-randr` and `kanshi` speak; (b) a private socket.
     Recommendation: (a), because a standard client can test it: add a cargo test that runs
     `wlr-randr` (verify it is packaged in noble's universe) against the headless compositor
     and asserts the output it lists. Smithay 0.7 has no ready handler for it (verify), so
     the server side is written by hand. The mode change itself is in
     `crates/flow-compositor/src/drm_output.rs`: a new dumb buffer at the new size, a modeset,
     and the window manager refits windows (`wm.rs`, unit tests first). Read the connector's
     mode list from the kernel; virtio-gpu offers several (verify with the E12 report's
     `drm.txt` from a VM boot).
3. **Check first,** page by page, one PR each: write the VM check for the page, see it fail,
   then build the page. Start with power (the smallest: one property, one state file).
4. **The verdicts** are from the other end, never from Settings' own log (H8, H19): hostapd
   and dnsmasq lines on the console (the access point is a separate process in the VM, as in
   `test-wifi.sh`); the size QEMU itself reports in the screendump's PPM header; the WAV that
   QEMU writes (D2); and the state file read with `debugfs` from a copy of the data
   partition, as `test-ransomware.sh` does.
5. **Traps:**
   - ⚠️ power-profiles-daemon keeps its choice in `/var/lib/power-profiles-daemon/state.ini`
     (its `StateDirectory`). On the read-only root the unit fails to start, and a failed
     unit is an unhealthy boot (the rfkill and backlight trap in `build.sh`). Bind
     `/persist/power` there in the image's `fstab`, as `/var/lib/strata` is, with a
     `tmpfiles.d` line that makes the folder.
   - The display choice is the compositor's own: keep it in `/persist/settings/display`,
     read at start. If the saved mode is not on the connector any more (a different screen),
     fall back to the preferred mode. A saved mode must never be able to leave the screen
     dark.
   - `test-shell.sh` counts background pixels against the whole screen. At 1024x768 the
     threshold changes: compute it from the screendump's size.
   - Brightness is not on this list because a VM has no backlight. On a laptop it is
     logind's `SetBrightness` (session object, needs E11's logind session) or
     `/sys/class/backlight` as root; note what the reference laptop's report shows (E6).
6. **Rob's decision:** do settings go through the Manifest? S11 (`proposed`) would make
   `morphd` the one place settings change, with transactions and rollback (S8). Options:
   (a) straight to each service now, stored where the service keeps it, on `/persist`;
   (b) wait for S11. Recommendation: (a). Adding Manifest fields that nothing obeys is H5;
   when S11 lands, the settings move into the Manifest one by one, each with its test.
7. **Size:** L. PRs: (1) the window, the bar button and the power page; (2) Wi-Fi on and
   off; (3) display, with the output-management protocol (the largest); (4) sound, after D2;
   (5) the reboot check. Each PR leaves `test-settings.sh` green for the pages it has.

### D2 · Sound (PipeWire) with a volume control · open
**What:** PipeWire with WirePlumber (A6) in the image, playing through the machine's sound
card, and a volume control on the bar: a speaker button with a level that scroll and the
keyboard's volume keys change, and a click mutes. Firefox plays through it (N1: a video
with sound).

**Done when:** `images/x86-qemu/test-sound.sh`, in the `images` workflow, boots a VM whose
sound card QEMU records to a WAV file on the CI host (`-audiodev wav`). In the VM, a known
tone (440 Hz) plays through PipeWire's default sink, and is also recorded from that sink's
monitor inside the PipeWire graph to a file on the ESP. The harness finds the tone in both
files (440 Hz at least 30 dB above any other frequency it checks; Target, set from the first
measurement). The volume key, sent through QMP, lowers the level the bar draws, and the next
tone in QEMU's file is quieter by the ratio PipeWire's volume curve gives (measured once, then
asserted within 10%). The boot is healthy. (Sharpened: the script, the capture at QEMU as
well as in the graph, and the volume control's effect.)

**Notes:**
1. **Files:** `images/x86-qemu/run-vm.sh` gains `VM_AUDIO=<wav path>`: `-audiodev
   wav,id=snd0,path=$VM_AUDIO,out.frequency=48000 -device ich9-intel-hda -device
   hda-output,audiodev=snd0` (q35's own HDA controller, driven by `snd-hda-intel`, which is
   in `linux-modules-extra` like every laptop sound driver). `build.sh` gains a `sound`
   scenario on the desktop base. New overlay `images/x86-qemu/overlay-sound/` with the
   units below and the tone (`/usr/share/morpheos/tone-440.wav`, generated at build time by
   Python's `wave` module, not committed). The bar's control is in `crates/flow-compositor`
   (drawn after the windows, like E5's Wi-Fi button). The test-only player is
   `overlay-sound-test/usr/lib/morpheos/sound-test`.
2. **Packages** (verify each with `apt-cache depends` before adding): `pipewire`,
   `pipewire-bin` (`pw-play`, `pw-record`), `wireplumber` (0.4.17 in noble: Lua config, not
   0.5's JSON), `pipewire-pulse` (Firefox speaks the PulseAudio protocol), `libspa-0.2-modules`,
   and `alsa-ucm-conf` (see traps).
3. **How it runs. PipeWire is written as user units** (`/usr/lib/systemd/user/pipewire.service`,
   `wireplumber.service`), and the image has no user session: everything runs as root until
   E11. Options: (a) three system units in the overlay (`pipewire`, `wireplumber`,
   `pipewire-pulse`), as root, with `XDG_RUNTIME_DIR=/run/flow`, the shell's runtime folder,
   so Firefox (`flow-browser.service`, same folder) finds `/run/flow/pulse/native` with no
   setting; (b) a user manager for the person (`user@1000.service`), which needs E11's
   account. Recommendation: (a) now, moved to (b) with E11. Both are Rob's to accept (what
   runs as root).
4. **The volume control's API.** PipeWire has no D-Bus API. Options: (a) `wpctl get-volume`
   and `set-volume @DEFAULT_AUDIO_SINK@` from the shell; (b) the `pipewire` crate (binds the
   C libpipewire, needs `libpipewire-0.3-dev` and clang at build time). Recommendation: (a)
   for D2. It is one process per change, which is fine for a person pressing a key. Behind a
   `Backend` trait with a fake, so the headless tests need no PipeWire.
5. **Check first:** the analysis before the image. Write the WAV check and prove it both ways
   on files the host generates (a tone passes, silence and a 1 kHz tone fail). Then boot the
   image with the sound card and no PipeWire: QEMU's file comes back silent and the check
   fails. Then PipeWire.
6. **The verdict:** QEMU's WAV is the other end: the sound left the machine. Read it with
   Python's `wave` module and a Goertzel filter at 440 Hz against 1000 and 3000 Hz (plain
   Python is fast enough for a few seconds of audio; no numpy). The monitor capture
   (`pw-record` with `stream.capture.sink=true` on the default sink; verify the property name
   against noble's `pw-record --help`) proves the tone went through the graph; it is written to
   the ESP and read with `mtype`, as the boot history is. Wait for the test hook's "played"
   line before stopping the VM (H13).
7. **Traps:**
   - ⚠️ QEMU writes the WAV header's length when it exits. Stop it with QMP `quit`, as
     `test-wifi.sh` does, and wait for the process, before reading the file.
   - WirePlumber gives a new sink a default volume well below 100% (verify the value in
     0.4.17), and the kernel's HDA mixer may start muted. Set the volume to 1.0 and unmute in
     the test hook before the first tone, and read `wpctl status` into the console when the
     WAV is silent.
   - `wpctl set-volume 0.5` may be linear or cubic (verify in 0.4.17). Measure the ratio once,
     write it in the test with the reason, and never guess it.
   - WirePlumber keeps the chosen volume in `$XDG_STATE_HOME/wireplumber`; for root that is
     under `/root`, on the read-only root. Set `XDG_STATE_HOME=/persist/audio` in its unit,
     or the volume resets on every boot and the journal fills with write errors.
   - Do not install `alsa-utils` without care: its `alsa-restore` unit writes
     `/var/lib/alsa/asound.state` on the read-only root, and a failed unit is an unhealthy
     boot. PipeWire does not need it.
   - Real laptops: Intel's Sound Open Firmware machines need `firmware-sof-signed` (in the
     base) **and** `alsa-ucm-conf`, or PipeWire shows only a dummy output. Add `snd-hda-intel`
     and `snd-sof-pci` (verify the file name) to the driver list `build.sh` asserts for the
     hardware image (H18), and `/proc/asound/cards` plus `wpctl status` to the hardware report.
   - WirePlumber 0.4 tries the session bus for device reservation and logs when there is
     none (verify that it keeps going; if not, turn `alsa.reserve` off in a
     `main.lua.d` file).
8. **Size:** M. PRs: (1) `VM_AUDIO`, PipeWire in the image, the tone through the graph, both
   captures; (2) the bar's control and the volume keys.

### D3 · Suspend, resume and battery · open
**What:** the machine suspends to RAM from the shell (a button on the bar), from the lid and
from the power button, and comes back with the screen, the keyboard and mouse, and the network
as they were. The bar shows the battery's charge and whether it is charging, from UPower.

**Done when:** the hardware report from the reference laptop shows suspend and resume work:
at least three cycles in one boot, one of them by closing the lid, each recorded with the
sleep state used (`deep` or `s2idle`), the kernel's success count rising and no failed
device, and after each resume a connected screen, the input devices present and the network
back; the report also gives the battery's charge, status and design capacity. Before that,
`images/x86-qemu/test-suspend.sh`, in the `images` workflow, proves the same on a VM.
(Sharpened: the report's content, and a VM scenario, which A8 asks of every user-facing task.)

**Notes:**
1. **What the hardware report must gain** (`images/x86-qemu/overlay-hardware/usr/lib/morpheos/hwreport`
   and `scripts/verify-hwreport.sh`, its check):
   - In `summary.txt`, a **POWER** section: `/sys/power/state`, `/sys/power/mem_sleep` (the
     bracketed one is in use), the ACPI states the firmware offers (`ACPI: PM: (supports S0
     S3 S4 S5)` from `dmesg`), the lid switch if there is one (`Lid Switch` in
     `/proc/bus/input/devices`), and each `/sys/class/power_supply/*`: type, status,
     capacity, `energy_full` against `energy_full_design` (the wear, as a percentage) and
     `cycle_count`.
   - A **sleep log**, `sleep.txt` in the boot's report folder, written by a hook in
     `/usr/lib/systemd/system-sleep/morph-sleep-report` (systemd-sleep runs it with `pre` or
     `post` and `suspend`). On `pre`: the time and the state. On `post`: the time, the
     kernel's counters (`/sys/power/suspend_stats/success`, `fail`, `last_failed_dev`; verify
     the folder exists on noble's 6.8, or read `/sys/kernel/debug/suspend_stats`), then after
     10 s the connectors' status, the input device names, NetworkManager's state (`nmcli -t
     -f STATE g`) and the compositor's last frame time. `hwreport` writes the folder it chose to
     `/run/morph-hwreport.dir` so the hook appends to the same boot. Every step best effort
     and the hook exits 0, as `hwreport` does (it must never fail a suspend).
   - HARDWARE.md says how to run the test on the laptop: suspend from the bar, by the lid and
     by the power button, wake each time, then shut down normally and send the folder.
2. **The VM scenario.** `run-vm.sh` gains `VM_S3=1` (`-global ICH9-LPC.disable_s3=0`; verify
   whether q35 in noble's QEMU 8.2 offers S3 without it), and the kernel command line of the
   `suspend` scenario gets `mem_sleep_default=deep`, because s2idle in a VM is only idling and
   QEMU cannot see it. The harness clicks the bar's suspend button, waits for QMP's `SUSPEND`
   event (QEMU's own evidence that the guest entered S3, from the other end), waits 60 s,
   sends `system_wakeup`, waits for `WAKEUP`, and judges a screendump: the shell and its
   window are back. Then it reads the ESP: the sleep log has a `pre` and a `post`, and the boot
   history has **one** boot (no reset while asleep). The battery: the kernel's `test_power`
   module (in `linux-modules-extra`, like `mac80211_hwsim`) adds a fake battery and charger;
   the test hook sets `/sys/module/test_power/parameters/battery_capacity` to 15 and
   `ac_online` to 0, and the bar's battery mark must show the low colour (verify that UPower
   lists `test_power`'s supplies).
3. **The APIs:** suspend is logind's `org.freedesktop.login1.Manager.Suspend(false)` on the
   system bus, never `echo mem > /sys/power/state` (logind runs the sleep hooks and takes
   NetworkManager's inhibitor). The compositor listens for logind's `PrepareForSleep(false)`
   signal. The battery is UPower's display device (`org.freedesktop.UPower`,
   `/org/freedesktop/UPower/devices/DisplayDevice`: `Percentage`, `State`). The lid and the
   power button are logind's (`HandleLidSwitch`, `HandlePowerKey`, default `suspend` and
   `poweroff`; Rob decides, below).
4. **Check first:** the VM scenario before the compositor change. It will most likely fail
   on the screen: after a resume the display may need a fresh commit. On `PrepareForSleep(false)`
   the compositor damages the whole frame and flushes it again (DIRTYFB, as E3 found), and it
   expects libinput devices to go and come back (the USB tablet re-enumerates): the same
   lesson as H14 and H15. Test that in `crates/flow-compositor` with a scripted "resumed"
   command through the same handler the signal uses.
5. **Traps:**
   - ⚠️ The image arms a 30 s hardware watchdog (`RuntimeWatchdogSec=30s`, the i6300esb in the
     VM). If it keeps counting while the guest is suspended, a suspend longer than 30 s is
     a reset and a new boot. That is why the VM test sleeps 60 s and counts the boots
     (verify; if it resets, systemd-sleep must stop the watchdog before sleeping, or a
     laptop's iTCO watchdog could do the same).
   - ⚠️ noble's 6.8 `virtio_gpu` driver may not restore its display after S3 (verify). If the
     screen stays black in the VM while the compositor's frames keep going, try the VM's
     `bochs-display` before concluding the compositor is wrong, and say which in the PR.
   - UPower keeps its history in `/var/lib/upower` (its `StateDirectory`): on the read-only
     root the unit fails. A tmpfs there, as for rfkill (the history is not worth keeping).
   - Hibernate needs a swap area as large as memory and a resume path through the
     initramfs; it is not in this task.
   - Many recent laptops offer only `s2idle` (Modern Standby). The report says which; s2idle
     that drains the battery overnight is a hardware finding and becomes a task.
6. **Rob's decisions:** what the lid and the power button do (recommendation: lid suspends,
   power button suspends with a long press powering off, as most laptops do), and whether
   hibernate is ever wanted (recommendation: not in v1). And the run on the reference laptop.
7. **Size:** M for the builder (PRs: the report's sections and sleep hook; the VM scenario
   and the compositor's resume; the battery mark), then Rob's run.

### D4 · Apps from Flathub (A7) · open
**What:** Flatpak in the image, with its system installation on the data partition, so an
installed app is outside both slots and survives every A/B update. Flathub is the remote
for people; the VM test uses a small repository the CI host serves, so the test does not
depend on Flathub being up.

**Done when:** `images/x86-qemu/test-flatpak.sh`, in the `images` workflow: the CI host builds
a one-app Flatpak repository, signed with a test GPG key made for the build, and serves it
over HTTP at 10.0.2.2. On slot A the machine installs the app from it and runs it; the update
medium (as in the `update` scenario) installs a new system into slot B, which is trialled and
committed; on B the app is listed and runs again. The verdict, read after power-off: the
host's HTTP log shows the guest fetched the repository's summary and objects; the app's own
record in its data folder on the data partition says it ran on `a`, then on `b`; the app's
files are under `/persist/flatpak` and not in slot B's root; the boot history is
`a/committed/none b/trial/committed b/committed/none`. (Sharpened: the script, the local
signed repository, and each verdict.)

**Notes:**
1. **Files:** `images/x86-qemu/build.sh` (a `flatpak` scenario on the self-test images,
   with `overlay-network` for DHCP over QEMU's user network, `VM_NET=1`), a new
   `images/x86-qemu/build-flatpak-repo.sh <workdir>` (host side), a new step in the self-test
   driver (`images/common/selftest-overlay/usr/lib/morpheos/test-hook`): an optional
   `test-hook.d/` folder whose scripts run at each step, empty for every other scenario, so
   no walk that passes today changes. `build-payload.sh` already builds slot B's payload from
   slot A's root, so it carries Flatpak too. `scripts/setup-linux.sh --images` gains `flatpak`
   and `busybox-static` on the host.
2. **The repository without a network.** A real runtime (`org.freedesktop.Platform`) is
   hundreds of MB from Flathub (Target, not measured). Instead build a tiny one by hand: a
   folder with a `metadata` file (`[Runtime]`, name `org.morpheos.TestPlatform`) and
   `files/bin/busybox` with its `sh` link (busybox-static needs no libraries), exported with
   `flatpak build-export --runtime`; and an app, `org.morpheos.Hello`, whose `files/bin/hello`
   is a shell script that appends the slot (from `/proc/cmdline`) to
   `$XDG_DATA_HOME/runs`. Export with `flatpak build-export --gpg-sign=<id> --gpg-homedir=…`,
   then `flatpak build-update-repo --gpg-sign`. Serve with `python3 -m http.server` and keep
   its access log. Verify each command on noble's flatpak 1.14 first, in a container, with no
   network.
3. **Read-only root: where the installation lives.** Flatpak's system installation is
   `/var/lib/flatpak`, on the read-only root. Options: (a) bind `/persist/flatpak` onto
   `/var/lib/flatpak` in `fstab`, like `/var/lib/strata` (with the mountpoint made in
   `make_root` and a `tmpfiles.d` line for the folder); (b) a second installation in
   `/etc/flatpak/installations.d/` with `Path=/persist/flatpak`. Recommendation: (a): the
   default path keeps every tool, the exported `.desktop` files and the `XDG_DATA_DIRS`
   that Flatpak's environment generators set working with no change.
4. **The remote.** Defined by the image, read-only: a `.flatpakrepo` file (with its
   `GPGKey=`) in `/etc/flatpak/remotes.d/` (verify that 1.14 installs from a remote defined
   there; if not, a oneshot unit adds it to the installation on `/persist` at first boot). The
   test image carries the test repository's remote; `plain` carries Flathub's. Never
   `--no-gpg-verify`: a remote without a signature check is a change to the security model.
5. **Check first:** run the test with Flatpak installed and no bind mount: the install must
   fail on the read-only root, and the test must say so. Then the bind.
6. **Traps:**
   - Flatpak also writes outside `/var/lib/flatpak`: as root, the user cache (`~/.cache`,
     under `/root`) and `/var/tmp` (verify both with `strace -f -e trace=%file` on an
     install). Give the installing process `HOME`/`XDG_CACHE_HOME` on `/persist` or `/run`,
     and add a `/var/tmp` tmpfs line beside `/tmp`'s.
   - `flatpak run` needs a writable `HOME` (the app's data goes in `~/.var/app/<id>`) and an
     `XDG_RUNTIME_DIR`. As root with `HOME=/root` it fails on the read-only root. Run the app
     as the person once E11 exists (`runuser -u morph`, `HOME=/home/morph`); until then, root
     with `HOME=/home/morph`.
   - Ubuntu 24.04 restricts unprivileged user namespaces through AppArmor, and Flatpak's
     sandbox (bubblewrap) needs them when it runs as a person. As root it does not. The
     image does not load AppArmor profiles today (no `apparmor` package): verify the sysctl
     `kernel.apparmor_restrict_unprivileged_userns` in the VM, and expect this to bite at E11.
   - **The stick's data partition is 256 MB** (`DATA_MB`). The test app fits; no real Flathub
     app does. `plain` needs the data partition grown to fill the stick at first boot
     (systemd-repart with `GrowFileSystem=yes`, verify), or E7's installer. Say so in
     HARDWARE.md until then.
   - Flatpak apps that open files or links need `xdg-desktop-portal` and a backend, which need
     a session bus. Not needed by the test app; needed by real apps. Note it.
7. **Rob's decisions:** (a) Flathub whole, or only its verified subset
   (`--subset=verified`, verify it in 1.14); recommendation: verified only, for a machine
   handed to someone. (b) Does Firefox move from the `.deb` (E4) to Flathub's Flatpak now
   (A7 says "until Flatpak lands")? Recommendation: a separate task, with its own browser
   test, because the Flatpak needs portals and a session bus. (c) Who may install: root only
   until S5. (d) N5 says "installs an app from a store": no store window is planned. That
   needs a `proposed` task.
8. **Size:** M. PRs: (1) Flatpak on `/persist` in the image, with the build check that
   `/var/lib/flatpak` is a mountpoint on the data partition; (2) the local repository and
   `test-flatpak.sh` through the update.

### D5 · Files: a file view with Strata's timeline · open · needs S6
**What:** a Files window on the shell: browse the person's home, open a file, and for any file
open its timeline (every version Strata kept, newest first) and put an older version back.
It reuses the timeline view S6 built for folders. N3 is this task and S6 together.

**Done when:** `images/x86-qemu/test-files.sh`, in the `images` workflow: the test hook writes
`Documents/colour.ppm` three times (red, then green, then blue), with Strata recording each.
By scripted input only, the harness opens Files from the bar, goes into Documents, opens
`colour.ppm` (the screendump shows blue in the viewer), opens its timeline (three versions
listed), chooses the oldest and puts it back (the viewer shows red). After power-off, the
file on the data partition is the red version byte for byte, and the blue one is still in
Strata's history, read on the host. The boot is healthy. (Sharpened: the script, the image
file whose colour is the verdict, and the history read from the disk.)

**Notes:**
1. **Files:** a new crate `crates/flow-files` (the same shape as `flow-wifi`: the model in
   `browse.rs`, unit-tested anywhere; the window in `main.rs`), using the `strata` crate as a
   library (`Strata::open`, the index's history, `restore_version`; read
   `crates/strata/src/strata.rs`) rather than calling the CLI. The viewer opens PPM images
   and plain text only (the shell has a bitmap font, `crates/flow-shell/src/font.rs`); other
   files are listed and their timeline works, but they are not opened here. The compositor
   gains `--launch-files`. Scenario `files` in `build.sh` (on the `shell` base: no network
   needed).
2. **Why a PPM:** its colour is the verdict, read off a screendump the way `test-shell.sh`
   reads the demo window. Text would need the font's glyphs matched instead.
3. **Check first:** the host-side history read before any window. Copy the store out of the
   data partition (as `test-ransomware.sh` does with `debugfs rdump /strata`), and run the host's
   own `target/release/strata --store <copy> history /home/morph/Documents/colour.ppm`. ⚠️ The
   CLI's `absolute()` canonicalises the parent folder, which does not exist on the host
   (verify); if so, add a flag that takes the path as stored, with a test, in the first PR.
4. **Traps:**
   - Strata's index is SQLite in WAL mode with a 10 s busy timeout, so Files can read while
     `strata watch` writes. A restore is a new save of the file, so the watcher records it as
     a new version: the history after the restore has four entries, not three. Assert four.
   - The store is `/var/lib/strata`, mode 0700, owned by root. Files runs as root today. ⚠️
     Once E11 moves the shell to the person, Files cannot open the store: the person needs a
     way in that does not give them every user's history. That is Rob's (below).
   - Three saves in under a second may be one version if the watcher coalesces them (verify
     in `crates/strata/src/watch.rs`); the test hook waits for each version to be in the index
     (`strata history`) before the next save.
5. **Rob's decision** (at E11, not now): how the person reaches their own history. Options:
   (a) a small root service that answers only for files the caller owns (a broker; later part
   of `morphd`); (b) one store per person, in their home. Recommendation: (a), because one
   store keeps dedup and the ransomware detector in one place.
6. **Size:** M if S6's timeline view can be reused as it is; L if it must first be moved out
   of wherever S6 put it (split: move it, with its tests, then Files).

### D6 · Printing (CUPS, driverless IPP) · open
**What:** CUPS in the image, printing to IPP Everywhere printers with no driver (CUPS's
`everywhere` model). In Settings (D1), a Printers page adds a printer by its address and
prints a test page. Printers found on the network by mDNS are Rob's decision (below).

**Done when:** `images/x86-qemu/test-print.sh`, in the `images` workflow: the CI host runs a
virtual IPP Everywhere printer (`ippeveprinter`, from Ubuntu's `cups-ipp-utils`) that keeps
every job it receives in a folder. In the VM, by scripted input, Settings adds the printer at
`ipp://10.0.2.2:8631/ipp/print` and prints the test page. The verdict is from the printer's
end: a job file arrives in that folder within 60 s and contains the test page's marker text,
and `ippeveprinter`'s log records the job as completed. The boot is healthy. (Sharpened: the
script, where the printer runs, and what is read.)

**Notes:**
1. **Files:** `build.sh` scenario `print` on the desktop base with `overlay-network` and
   `VM_NET=1` (QEMU's user network reaches the host's loopback at 10.0.2.2, as
   `test-browser.sh` does); a new `images/x86-qemu/overlay-print/` (units, `cupsd` settings,
   `tmpfiles.d`, `/usr/share/morpheos/print-test.pdf` with a known marker string);
   `crates/flow-settings` gains the page; `scripts/setup-linux.sh --images` gains
   `cups-ipp-utils` and `poppler-utils` (`pdftotext`) on the host.
2. **The printer:** `ippeveprinter -p 8631 -k -d "$WORK/ipp-spool" -f
   application/pdf,image/pwg-raster -r off "MorpheOS Test Printer"`, its output kept as the
   log. `-k` keeps the job files, `-r off` turns off DNS-SD (the host has no Avahi). Check
   every flag against noble's `ippeveprinter --help` (CUPS 2.4.7) before relying on it.
3. **The API.** CUPS has no D-Bus control API; its API is IPP. Options: (a) the CLIs from
   the shell (`lpadmin -p <name> -E -v <uri> -m everywhere`, `lp -d <name> <file>`,
   `lpstat -W completed`); (b) IPP from Rust (a crate such as `ipp`, verify its licence and
   upkeep). Recommendation: (a) now, behind a `Backend` trait with a fake, as `flow-wifi` does.
4. **Check first:** the service half without the window. The test hook runs `lpadmin` and
   `lp` itself and the host's folder must receive the job; see it fail with no CUPS in the
   image. Then the Settings page replaces the hook's commands with clicks.
5. **Traps:**
   - ⚠️ CUPS writes its configuration: `lpadmin` saves `/etc/cups/printers.conf` and
     `/etc/cups/ppd/<name>.ppd` (the `everywhere` model generates one), and `cupsd` rewrites
     `subscriptions.conf`. All are on the read-only root. Keep `/etc/cups` on the data
     partition: a `tmpfiles.d` line that copies the image's defaults into `/persist/cups/etc`
     once (`C`), and a bind mount onto `/etc/cups`. The spool (`/var/spool/cups`) and
     cache (`/var/cache/cups`) go on tmpfs (a job lost at power-off is acceptable), or `cupsd`
     cannot accept a job.
   - The packages: `cups-daemon`, `cups-client`, and `cups-filters` for `pdftopdf` (CUPS
     still filters a PDF before sending it). Check what `cups-filters` pulls (Ghostscript is
     large) with `apt-cache depends`, and measure the root.
   - `lpadmin -m everywhere` asks the printer for its attributes when the queue is made, so
     the printer must be running before Settings adds it.
   - `pdftopdf` may rewrite the PDF, so the marker may not be the file's bytes. Read it with
     `pdftotext` on the host, not with `grep`.
   - After E11, a person adding a printer goes through polkit (`cups-pk-helper`), not in the
     image. Root until then.
   - Real printers are found by mDNS. QEMU's user network does not carry multicast, so the VM
     cannot prove discovery; a hardware note can (Rob prints to a real printer once).
6. **Rob's decision:** does the machine listen for printers on the network? It needs
   `avahi-daemon` (UDP 5353, mDNS) and either CUPS's own temporary queues or `cups-browsed`.
   Options: (a) addresses only (no mDNS); (b) Avahi browsing, publishing nothing.
   Recommendation: (b), because no one types a printer's address. It opens a port, so it is
   his call.
7. **Size:** M. PRs: (1) CUPS on `/persist`, the virtual printer and the hook-driven job;
   (2) the Settings page, after D1's first PR.

### D7 · Bluetooth (BlueZ) · open
**What:** BlueZ in the image, and a Bluetooth page in Settings (D1): turn Bluetooth on and off,
list nearby devices, pair one (confirming a passkey where the device shows one), connect and
forget. A paired keyboard or mouse works on the shell; paired headphones are a PipeWire
output (D2).

**Done when:** the hardware report shows a pairing: from the reference laptop, the report
names the Bluetooth controller and the driver that took it, says its firmware loaded, and
lists the device Rob paired through the shell as paired, bonded and connected (its address
masked), and the device worked (for a keyboard or mouse its input device appears in the
report; for headphones a Bluetooth sink appears in PipeWire). Before that,
`images/x86-qemu/test-bluetooth.sh`, in the `images` workflow, pairs two virtual controllers
through the shell's page in a VM. (Sharpened: what the report shows, and a VM scenario, which
A8 asks of every user-facing task.)

**Notes:**
1. **What the hardware report must gain** (`overlay-hardware/usr/lib/morpheos/hwreport`,
   checked by `scripts/verify-hwreport.sh`):
   - In `summary.txt`, a **BLUETOOTH** section: each `/sys/class/bluetooth/hci*`, its bus
     (USB or UART), the driver bound (`btusb`, with `btintel`, `btmtk` or `btbcm`), and the
     kernel's firmware lines for it (`Bluetooth: hci0: ...` in `dmesg`; a missing firmware
     already lands in the missing-firmware section).
   - A `bluetooth.txt` written **after** the pairing, not at boot: a oneshot unit with
     `RemainAfterExit=yes` whose `ExecStop` runs at shutdown, plus a timer every 10 minutes
     (best effort, for a machine that loses power), writing `bluetoothctl devices Paired` and
     `bluetoothctl info` for each (name, icon, `Paired`, `Bonded`, `Trusted`, `Connected`,
     the profiles' UUIDs), the input devices, and `wpctl status`. Mask addresses to their last
     two bytes: the report is sent around (E6 already removes MAC addresses).
   - HARDWARE.md: how to pair from the shell, then shut down normally and send the folder.
2. **The APIs:** BlueZ on the system bus (`org.bluez`): `org.bluez.Adapter1` (`Powered`,
   `StartDiscovery`, `RemoveDevice`), `org.bluez.Device1` (`Pair`, `Connect`, `Trusted`), and
   an agent the Settings window registers with `org.bluez.AgentManager1.RegisterAgent` and
   `RequestDefaultAgent`, implementing `org.bluez.Agent1` (`RequestConfirmation`,
   `DisplayPasskey`, `RequestPinCode`) with zbus's server side. The page shows the passkey and
   asks the person to confirm. BlueZ in noble is 5.72.
3. **The VM scenario:** the kernel's virtual controller driver `hci_vhci` (in
   `linux-modules-extra`) and BlueZ's emulator `btvirt` (in noble's `bluez-test-tools`,
   test image only): `btvirt -l2` makes two controllers that hear each other (verify the flag
   and that both appear to `bluetoothd`). hci1 is made discoverable and pairable by the test
   hook with its own NoInputNoOutput agent (`bluetoothctl`); the harness, by scripted input
   on the Settings page with hci0 selected, finds and pairs it. The verdict, read after
   power-off: hci0's pairing record exists in BlueZ's store on the data partition
   (`/persist/bluetooth/<hci0 address>/<hci1 address>/info` with a `[LinkKey]` or `[LongTermKey]`
   section) and the screendump shows the device as paired. If two controllers under one
   `bluetoothd` cannot pair (verify early, before any window), use two VMs on a QEMU socket
   network instead, and say so in the PR.
4. **Check first:** the report's section and the `btvirt` pairing by `bluetoothctl` alone in
   the VM, before the Settings page.
5. **Traps:**
   - ⚠️ BlueZ keeps adapters and pairing keys in `/var/lib/bluetooth` (its `StateDirectory`),
     on the read-only root: `bluetooth.service` fails, an unhealthy boot. Bind
     `/persist/bluetooth` there (mode 0700: the folder holds link keys).
   - Add `btusb` to the drivers `build.sh` asserts for the hardware image (H18), with the
     vendor helpers the candidate machines need (`btintel`, `btmtk`, `btbcm`).
   - Firmware: Intel Bluetooth (`intel/ibt-*`) and MediaTek (`mediatek/BT_RAM_CODE_*`) should be
     in the firmware sets already in the base (verify the file lists of
     `linux-firmware-intel-wireless` and `linux-firmware-mediatek`). Broadcom Bluetooth on the
     2015 Mac needs a `.hcd` file that is not in linux-firmware; expect the report to say it
     is missing.
   - Headphones need PipeWire's BlueZ plugin (`libspa-0.2-bluetooth`) and WirePlumber's BlueZ
     monitor, which wants logind seat information (verify with D2's system units).
   - Many laptops keep Bluetooth soft-blocked by rfkill at boot (the rfkill state is on a
     tmpfs, so it resets each boot). The page must unblock through BlueZ's `Powered`, and the
     report records `rfkill` state.
6. **Rob's decisions:** which device he pairs for the report (recommendation: a keyboard or
   mouse first, which needs no audio; headphones as a second run); whether Bluetooth is on by
   default (recommendation: off until the person turns it on); and where pairing keys are kept
   (on `/persist` in plain files, as Wi-Fi passwords are; the same question as E5's).
7. **Size:** M for the builder (PRs: the report and BlueZ on `/persist`; the VM scenario;
   the Settings page), then Rob's run.

## M4 · Never lose anything

**Order of work (M4 and M5).**

M4 starts when M3 is done. M5 waits until Rob orders it: every W task is `proposed`, and its
notes below are lighter on purpose (what, the check, the facts that decide the design, what it
waits on). They are not step lists.

1. **P1 first, and its first pull request is an ADR, not code.** P1 decides the replica format
   and the key model, and P2, P3 and W5 all read that format. Rob approves the keys (it is the
   security model). Then the format library, then the service, then the VM scenario.
2. **P4 can run beside P1.** It shares nothing with P1 except the shell's warning mark, which
   S7 also needs ("the shell warns"). Whichever of S7 and P4 lands first builds that mark, and
   the other uses it.
3. **P3 after P1.** It is P1's replica sent over the network to a peer that holds only
   ciphertext. The transport and the pairing are Rob's decisions.
4. **P2 last in M4.** It needs E7's installer, and E7 needs E6 (Rob's machines). P2 restores
   P1's backup onto the disk E7 lays out, so its VM scenario reuses P1's backup disk.
5. **M5, if Rob orders it, in this order:** W4 (the wedge on the Pi, using P1's work for Strata
   on the Pi), then W1 (N5 needs one Windows program), then W5 (it needs P1's format and P3's
   transport), then W2, with W3 following doc 21's I tasks and W6 left until the revenue gate.

Each VM scenario added here is a new step in the x86 job of `.github/workflows/images.yml` and a
line in `scripts/test-images.sh` (the script says to keep the two in step). That job has a
90-minute timeout and is a required check (AGENTS.md H10): if the new steps push it near the
limit, raise the timeout or split the job, and a split job must be made a required check in the
same change.
### P1 · Encrypted backup to a USB disk (Phoenix v0) · open
**What:** a second, encrypted copy of `/persist` on a plugged-in disk, kept up to date by itself
(doc 15: "continuous encrypted replication to at least one other place"). The copy is Strata's
own store (every version of `/home`, and of the machine settings on `/persist`, such as the saved
Wi-Fi networks in `/persist/network`), written to the disk as encrypted objects, so a restore
brings back the history and not only the latest files. A command restores from it.

**Done when:** `images/x86-qemu/test-backup.sh`, in the `images` workflow, boots a VM with a
second, USB-attached disk labelled `MORPH-BACKUP`. After the VM powers off, the CI host, which
holds the only copy of the decryption key, restores the backup with `strata replica restore` and
finds every document the self-test wrote, byte for byte, including one saved **after** the first
copy was made, without a reboot in between. The raw backup disk does not contain a known
plaintext marker that the documents contain. Every boot is healthy, and a boot with no backup
disk attached is healthy too. (Sharpened from "replicates `/persist` to a second disk, encrypted,
continuously": the script, the key held off the machine, the later save, and the boot without
the disk are new.)

**Waiting on Rob:** his answers to [ADR 0018](adr/0018-backup-replica.md): the mechanism,
the key model and the recovery code's form. Step (a), marking it Accepted, is the first PR.

**Notes:**
1. **Decide the mechanism first: [ADR 0018](adr/0018-backup-replica.md), drafted and
   Proposed, approved by Rob.** The
   options:

   | | Mechanism | For | Against |
   |---|---|---|---|
   | A | **Strata's store replicated**: each object encrypted with age (the Rust `age` crate, 0.12.1, MIT OR Apache-2.0) to an X25519 recipient | One history, so a restored machine keeps its timeline (N3). Dedup and the BLAKE3 integrity check already exist (doc 04: "self-healing when there's a redundant copy"). Copying only what is missing is a set difference. The same format serves P3's peer and W5's Windows agent | New code: the object naming, a snapshot of the index, restore |
   | B | restic (Ubuntu 24.04 has 0.16.4) on a timer over `/persist` | Mature and quick to ship | A second store and a second history beside Strata's. One password both writes and reads, so the machine holds a key that can read every old backup. A Go binary in the slot |
   | C | A LUKS disk with a plain copy | Kernel crypto, simple | The disk is readable whenever it is unlocked, and a peer (P3) would hold readable data |

   **Recommendation for Rob: A.** The device keeps only the **recipient** (the public half).
   The **identity** (the secret half) is shown once as a recovery code and kept by the person
   (doc 15: "keys are held by the user and escrowed only if they choose"). Then a machine that
   has been taken over can add to the backup and cannot read it. Rob decides the key model (a
   public-key recovery code, or a passphrase through age's scrypt recipient, which is easier to
   remember and weaker), the form of the recovery code (printed, a QR code, a file on a stick),
   and whether `age` is an acceptable dependency. Do not write any encryption by hand: the only
   primitives are age's.
2. **Files.** In `crates/strata`: a `replica` module (format, naming, push, restore) that has no
   Linux-only dependency (W5 builds it on Windows; AGENTS.md H9). New CLI commands
   `strata replica init|push|status|restore`. In `images/x86-qemu/overlay/usr/lib/systemd/system/`:
   `strata-replica.service`, plus a mount for the labelled disk. `build.sh` gets a `backup`
   scenario that links the test hook to a new driver,
   `images/common/selftest-overlay/usr/lib/morpheos/backup-test` (the pattern
   `ransomware-test` uses). `run-vm.sh` gets `VM_DISK2=<file>`: a writable disk on
   `-device usb-storage` behind the `qemu-xhci` it already adds for input, so the path a real
   USB disk takes (`usb-storage`, `uas`, which `build.sh` already asserts) is exercised.
3. **The format, in short.** On the disk: `morpheos-replica/<machine-id>/`, holding a header
   encrypted to the recipient (format version and the naming key), objects named by **keyed**
   BLAKE3 of their local name (the plain BLAKE3 name of a chunk would let anyone holding the disk
   test whether you have a given file), and numbered snapshots of the index. Take the index
   snapshot with SQLite's backup API or `VACUUM INTO`: `index.sqlite` is in WAL mode, so copying
   the file alone loses the newest rows. Write each object to a temporary name, fsync, then
   rename, so a pulled plug leaves no half object under a real name.
4. **How to approach it.** (a) The ADR. (b) The format as a library, with unit tests: round trip,
   a wrong identity is refused, a flipped byte is refused, a second push writes only the new
   objects. `/storage/replicas` is already in `unenforced_settings()`
   (`crates/morph-manifest/src/validate.rs`, #67): it is parsed and validated and nothing reads
   it (H5). It stays there until `morphd` writes `replica.toml` from it. (c) The CLI and the service, and test-backup.sh written to fail before them
   (AGENTS.md H3). (d) The images workflow step. Configuration lives in the store
   (`/persist/strata/replica.toml`: the disk label and the recipient) until S11's `morphd` writes
   it from `storage.replicas[].target`, which is already a disk label in
   `manifest/examples/desk-laptop.toml`.
5. **The verdict.** The harness makes the key pair on the host (`age-keygen`; Ubuntu 24.04 has
   age 1.1.1), puts only the recipient in the data partition's starting tree (`build.sh` step 4),
   and keeps the identity. After power-off it copies the backup disk's files out (debugfs, as
   test-ransomware.sh does), decrypts one object with the reference `age` tool (a second
   implementation, so the format is checked against something we did not write), and then runs
   the host-built `strata replica restore` into a temporary store and compares checksums with the
   `before.sha256` the self-test wrote to the ESP. `grep -c` for the marker over the raw disk image
   must be 0. Boot health comes from `morph-boot/history.jsonl` on the ESP (H8), never the console.
   Before reading, wait for the self-test's last step to be on the ESP (H13).
6. **Traps.**
   - **A failed unit is an unhealthy boot.** If the replica service fails when no disk is plugged
     in, every laptop without a backup disk falls back on its next update. With no disk it must
     exit 0 and record "no backup disk". Run with the disk absent, as the Done-when requires.
   - The root is read-only: mount the disk under `/run` (tmpfs), never `mkdir` in `/mnt` (H11).
     Tie the mount to the disk with a `.mount` unit on `/dev/disk/by-label/MORPH-BACKUP`
     (systemd escapes the `-` in a unit name as `\x2d`).
   - "Continuously" means after each recorded save, with a debounce (Target: a save is on the
     disk within 60 s), plus a timer for when the disk comes back. Do not copy the whole store on
     every event.
   - **The replicator never deletes on the disk in v0.** A machine running ransomware as root can
     still wipe an attached disk; deleting nothing at least makes the replicator unable to do it.
     Pruning the replica waits until `storage.retention` is obeyed (S11, S7).
   - Formatting a person's disk loses data, so v0 does not format anything: the disk must already
     carry the label (ext4 or exFAT). The harness formats its own test disk.
   - Strata watches with inotify, and `strata watch` takes several folders: add `/persist/network`
     beside `/home`. Never a folder that contains the store (Strata refuses that, and a test
     asserts it).
7. **Size: L.** PRs: (1) the ADR, which is drafted as
   [ADR 0018](adr/0018-backup-replica.md) (Proposed): Rob answers its questions and it is marked
   Accepted; (2) the `replica` library
   and its tests; (3) the CLI and the service; (4) test-backup.sh and its workflow step. Each
   leaves `main` green.

### P2 · Restore onto a blank machine (rung H5) · open · needs P1, E7
**What:** the installer (E7) can rebuild a machine from a P1 backup: it lays out the internal disk,
writes the system, asks for the recovery code, restores the store from the backup disk into the new
`/persist`, and puts back the newest version of every file. ADR 0009: H5 may lose data back to
the last copy, so it **asks first** and says which copy it is restoring.

**Done when:** `images/x86-qemu/test-restore.sh` boots the installer from a stick with a blank
internal disk and P1's backup disk attached; scripted keyboard input confirms the restore and types
the recovery code; then the internal disk boots **alone** to a committed slot, and, read off that
disk after power-off, every document from the backup is in `/home` byte for byte, a file has at least
two versions in the restored store, and the saved Wi-Fi network is in `/persist/network`. A wrong
recovery code is refused before anything is written to the internal disk. (Sharpened from "restores a
blank disk from that backup to a working machine": "working" is now a committed boot from the internal
disk alone, with the files, the history and the settings.)

**Notes:**
1. **Files.** E7's installer (whatever it is named when E7 lands); `strata replica restore` from
   P1; `images/x86-qemu/test-restore.sh`, and a `restore` step in the `images` workflow after
   `test-backup.sh`. It reuses that scenario's backup disk file, the way `heal` reuses `unhealthy`'s
   disk in `test-fallback.sh`, so the backup it restores was made by a running machine and not
   by a helper.
2. **How to approach it.** The check first: the harness asserts the internal disk is blank before
   the VM starts (so a pass cannot come from a pre-built disk), then expects the restore. Restore the
   store first, then put files back with Strata's own restore (newest version of every path), so
   one code path writes files on both a rewind and a restore.
3. **The verdict.** From the internal disk after power-off: boot history on its ESP (committed),
   the data partition read with debugfs, and the host-built `strata` run against the restored
   store for the history. Measure the restore time from the guest's kernel timestamps and record it
   as Measured (VM); doc 15's 5–15 minutes is a Target.
4. **Traps.** The recovery code is typed with QMP `send-key` the way `test-wifi.sh` types a
   password; an age identity is 74 characters, so each key needs its own event, and the code must
   never be printed to the console or a log. The installer must refuse to restore over a
   disk that is not blank without a second, separate confirmation (it destroys that disk's data).
   The restored machine gets a new machine id, and its own replica must not overwrite the old
   machine's copy on the backup disk (a new `<machine-id>` folder).
5. **Rob's decisions:** the form of the recovery code (from P1), and whether the restore may
   begin without asking on a machine that has no system at all (ADR 0009 allows it; an installer
   stick is not that case).
6. **Size: M**, after E7 and P1. One PR if E7's installer has a clear place for the step;
   otherwise (1) restore in the installer with a headless test, (2) the VM scenario.

### P3 · Replicate to a second MorpheOS device · open · needs P1
**What:** two MorpheOS machines on one network keep each other's P1 replica. Each peer stores the
other's encrypted objects and cannot read them (it never has the other's identity).

**Done when:** `images/x86-qemu/test-peers.sh` boots two VMs on one virtual network. Each writes
documents; after both power off, each disk holds the other machine's replica, and the CI host,
using each machine's identity, restores the other machine's documents from it byte for byte,
including one saved after the first copy. Neither disk contains the other's plaintext marker. The
pairing was made by the test's scripted step, not by a key put on the images at build time.
(Sharpened from "two VMs on one network keep each other's backup".)

**Notes:**
1. **Transport, a recommendation for Rob:** SFTP over OpenSSH, with a restricted account per peer
   (`ForceCommand internal-sftp`, `ChrootDirectory /persist/peers/<peer>`) and host keys pinned at
   pairing. It is a platform primitive (AGENTS.md "prefer platform primitives"), and because the
   objects are already encrypted the transport only has to prove which peer it is talking to.
   WireGuard and mDNS (doc 07) are the mesh later, not needed for two machines on one LAN.
   Pairing (how two machines come to trust each other: a code typed on both, a QR code) is the
   security model, so it is Rob's decision.
2. **Files.** A `push` target over SFTP in P1's `replica` module; `openssh-server` in the
   image (Ubuntu 24.04 has 9.6p1); `images/x86-qemu/test-peers.sh`. `run-vm.sh` gets a way to put
   two VMs on one network: `-netdev dgram` or `-netdev socket,mcast=…` (QEMU 8.2 in Ubuntu 24.04
   has both), each VM with its own OVMF variables file, run in parallel and waited on.
3. **Traps.** sshd generates host keys in `/etc/ssh`, on the read-only root: the key generation
   fails and the boot is unhealthy. Put `HostKey` on `/persist/ssh/` and make it with a tmpfiles
   rule. A quota for each peer's folder, or one peer can fill the other's `/persist` (S7's
   failure, caused from outside). The two VMs must not share the backup-disk label or
   machine id. Two VMs at 1 GiB each fit a standard runner (16 GB, spec), but the run time
   roughly doubles.
4. **The verdict** is from each disk after power-off, with the host doing the decrypting, as in
   P1. The peer's own `sftp` log is a diagnostic, not the verdict.
5. **Size: L.** PRs: (1) the SFTP target with a test against a local `sshd`; (2) pairing (after
   Rob decides); (3) the two-VM harness and scenario.

### P4 · Hardware health warnings · open
**What:** the machine reads its disks' health (NVMe and SATA SMART) on a timer, records what it
found, and the shell shows a warning when a disk reports it is failing (doc 15, "Predictive
healing"). Battery, thermals and SD wear come later; this task is disks.

**Done when:** `images/x86-qemu/test-health.sh` boots the shell image with a QEMU NVMe disk. After a
healthy boot, and a screendump that shows no warning, the test sets the disk's
`smart_critical_warning` over QMP; within 60 s a screendump shows the warning mark in the shell's
bar at its known place and colour, and after power-off a record on `/persist` names the disk and
the reason. The boot stays healthy throughout. (Sharpened from "a fake failing SMART report raises
a shell warning": the fault is injected into the disk device model, so the real kernel and
`smartctl` path is read, and the before-picture is required.)

**Notes:**
1. **Files.** A new crate, `crates/morph-health`: it runs `smartctl --json -H -A` (smartmontools
   7.4 in Ubuntu 24.04), parses the result, and writes `/run/morpheos/warnings/disk-<serial>.json`
   and a line in `/persist/health/history.jsonl`. Unit tests on committed JSON fixtures: healthy,
   NVMe `critical_warning` set, `percentage_used` at 100, and ATA attributes 5, 197 and 198
   rising. A service and a timer in `images/x86-qemu/overlay/`. In `crates/flow-shell`, a warning
   mark in the bar for any file in the warnings folder (S7 needs the same mark). `run-vm.sh` gets
   `VM_NVME=<file>`: `-drive if=none,id=nv0,file=… -device nvme,serial=MORPHNVME,drive=nv0,id=nvme0`.
2. **How to approach it.** The parser and its fixtures first, in `cargo test`. Then the scenario,
   written to fail. Then the shell mark, then the service.
3. **The verdict.** Screendumps read as `test-shell.sh` reads them (the mark's rectangle and
   colour), and the record copied off `/persist` after power-off. The injection happens in the
   device model and the reading is the system's own, so a mark that appears without the injection
   is a failure (assert the before-picture).
4. **Traps.** `smart_critical_warning` is in QEMU 8.2's binary (checked in the Ubuntu 24.04 package);
   verify: that `qom-set /machine/peripheral/nvme0 smart_critical_warning 4` takes effect on a
   running VM and that `smartctl` then reports `smart_status.passed: false`. Do not enable the
   package's own `smartd.service`; the timer is ours. A failing disk is a warning, never a
   failed unit: a failed unit would make the boot unhealthy and trigger a fallback that cannot
   help a dying disk (H19's opposite case). smartctl's exit code is a bit mask, so a non-zero exit
   is data, not an error; decode it, and do not let `set -e` end a script on it (H1).
   USB disk bridges often hide SMART: report "unknown", not "healthy".
5. **Optional, and say so if done:** a warning also asks P1 to copy now (doc 15: "bring the
   replica up to date").
6. **Size: M.** PRs: (1) `morph-health` and its fixtures; (2) the bar mark (or S7's, if it exists);
   (3) the service and the scenario.

## M5 · Widening (ADR 0014 lane 2)

Approved direction, not yet ordered. Each becomes `open` when Rob orders it.

### W1 · Windows programs (Wine/Proton capsule) · proposed
**What:** one Windows program runs on the shell through Wine, as a Wayland client (no X server:
A5), with its prefix on `/persist`. It is the first rung-3 capsule of doc 05.

**Done when:** one named Windows program runs in a VM scenario (N5).

**Notes:**
- **The program, a suggestion: SumatraPDF**, the portable 64-bit build (GPLv3, so it may be
  fetched at build time and pinned by checksum; verify: the current version and the checksum on its
  download page). It opens a PNG or PDF given on its command line, so the verdict can be a page
  of large known colours read off a screendump, the way `test-browser.sh` reads Firefox. A first
  slice is a tiny Win32 program built in CI with `gcc-mingw-w64-x86-64` (in Ubuntu 24.04) that
  paints known colours, so that Wine itself is proven before a real program is.
- **Wine's Wayland driver is in Ubuntu 24.04's Wine** (`libwine` 9.0 carries `winewayland.drv`;
  checked). Wine prefers X11 when `DISPLAY` is set, so leave it unset. The compositor serves
  `wl_compositor`, `wl_shm`, `wl_seat`, `wl_output`, `wl_data_device` and `xdg_shell`; log what Wine
  binds with `WAYLAND_DEBUG=1`, as E4 did. Choose a 64-bit program, so no i386 multiarch is needed.
- `libwine` installs about 650 MiB (its package's Installed-Size): give it its own cached base, as
  `rootfs-browser` is, and raise `ROOT_MB` deliberately. The first run creates the prefix and asks
  to install Mono and Gecko in dialogs; suppress them in the test (`WINEDLLOVERRIDES="mscoree=;mshtml="`).
- DXVK and Proton need Vulkan, and the pixman renderer and plain virtio-gpu have none: games are
  not in this task.
- **Blocker, Rob's decision:** doc 09 says a `.exe` "runs in a Wine capsule with no ambient file
  access". A Wine with no sandbox, running as root until E11 and S5, does not meet that. Options: the VM
  scenario only, with no Wine in `plain` until a sandbox exists (bubblewrap plus Landlock, which is
  built into the Ubuntu 24.04 kernel); or the sandbox inside W1. Recommendation: the sandbox is part
  of W1 before Wine reaches `plain`.

### W2 · Android apps (Waydroid) · proposed
**What:** Waydroid in the image, its Android system on the slot and its data on `/persist`, with one
Android app shown in a window on the shell.

**Done when:** one Android app runs in a VM scenario.

**Notes:**
- **The kernel has binder.** The Ubuntu 24.04 kernel the images use (6.8.0-146) has
  `CONFIG_ANDROID_BINDER_IPC=m` and `CONFIG_ANDROID_BINDERFS=m` with `CONFIG_ANDROID_BINDER_DEVICES=""`,
  and `binder_linux.ko` is in `linux-modules-extra`, which the images already install (checked
  in the packages). So Waydroid uses binderfs; load `binder_linux` at boot.
- **Waydroid and `python3-gbinder` are not in Ubuntu 24.04's archive** (checked): they come from
  Waydroid's own repository, which is pinned and has its key checked, as Mozilla's is in `build.sh`.
  `lxc` 5.0 is in the archive.
- Waydroid's Android images (about a gigabyte; verify:) go on the slot, in
  `/usr/share/waydroid-extra/images`, so they are immutable and updated with A/B; `/var/lib/waydroid`
  goes on `/persist`. `waydroid init` downloads at run time by default: do it at build time.
- **Riskiest unknown:** the VM has no GPU, so Android needs software rendering (verify: the
  `ro.hardware.gralloc` and `ro.hardware.egl` settings for SwiftShader), and verify: whether
  Waydroid's window bridge can draw through `wl_shm`, since the compositor deliberately offers no
  `linux-dmabuf` (E4). If it needs dmabuf, this task waits for a GPU renderer.
- The verdict: a screendump shows the app's window, and Android's own record
  (`waydroid shell -- dumpsys window`, the focused package) names the app. The app: an open-source
  APK from F-Droid (the F-Droid client itself, GPLv3, also serves as the store). Rob's decisions:
  Waydroid runs a privileged container as root (security model), and which store (doc 05: F-Droid,
  Aurora, or a user's own).

### W3 · The intent bar with a local model (AI proposes, deterministic code decides) · proposed · needs I10
**What:** put into the image whichever tiers I10 adopted. The design, rules and evaluation are
[doc 21](21-intent-engine.md); I1–I10 below are this task expanded. If I10 finds a model does not
earn its place, this task closes with that result.

**Done when:** the top 20 tasks of doc 02 are measured, with and without the model.

### I1 · The catalogue and the registry (`morph-intent`, types only) · proposed
**What:** a new library crate `crates/morph-intent`: no I/O, and it must build and test on Linux,
macOS and Windows alike (AGENTS H9, H16). It holds `IntentSpec`, `SlotSpec`, `Catalogue` and a
registry that builds the catalogue only from registered executors. An executor has three parts: its
spec, the live entities for each `Entity` slot, and `plan(slots)`, which returns the action to take (a
Manifest diff, or typed command arguments) so that R3 can re-derive the risk from it. `Risk` is re-exported from
`morph-manifest`, never redefined. Start from the compiled reference in doc 21 appendix A. Register
the v1 intents of doc 21 section 4 as specs (wiring them to run is I7).

**Done when:** `cargo test -p morph-intent` passes with tests that registration refuses a
duplicate id, an empty id, fewer than 6 distinct examples, and an intent with no executor; that
the catalogue serialises to a JSON schema whose enum of ids equals the registered ids (registering
one more changes the schema); and the PR description quotes a sabotage run for each rule (rule
removed, named test fails).

**Ask Rob to open:** I1–I3, as the lane for the days the builder waits on his hardware
reports ([doc 21](21-intent-engine.md), D1). They sit under M5, so opening them is not enough:
he also marks each `parallel`, or they still cannot start.

**Notes:** traps T3 and T4. `IntentSpec::new` must give `risk` and `model_reachable` no default.
Check each executor's current API before registering it; HANDOFF moves daily.

### I2 · The disposition policy · proposed · needs I1
**What:** the pure function of doc 21 section 6, in `morph-intent`: `disposition(policy, catalogue,
candidate, origin, planned_risk)` returning AutoRun, Confirm, NeedsPerson or Refuse. It looks the spec
up in the catalogue itself; a caller never passes one beside the candidate. Port the
appendix A reference and its tests.

**Done when:** an exhaustive test over every tier, risk, planned risk, origin, reachability and
score band asserts that AutoRun implies risk at most ReversibleLocal, origin not Untrusted, and
tier T0 or T1 (T1 only at or above `fuzzy_auto`); that every risk of PersistentConfig or higher is
NeedsPerson or Refuse; that a lower planned risk never lowers the declared one; that a candidate is judged by its own
intent in the catalogue, never a neighbour's; and that a candidate whose intent the catalogue does not
hold is refused. The `match` on
tier is under `#[deny(clippy::wildcard_enum_match_arm)]`, so `cargo clippy --all-targets -- -D
warnings` fails if someone adds a wildcard arm. The PR quotes a sabotage run for each rule.

**Notes:** trap T5. The reference was verified to catch seven sabotages; port those checks too.

### I3 · The judge: corpus validator, scorer and corpus v0 · proposed · needs I1, I2
**What:** two crates. `crates/morph-eval` holds what any judge here needs and nothing about intents:
group-aware splitting with a leakage check, the bootstrap interval, the rule-of-three bound, the
pinned predictions header, provenance labels, and the 0 / 1 / 2 exit-code convention (diagnosis, doc 22,
reuses it). `crates/morph-intent-eval` (library and binary) is the intent judge on top of it, per doc 21
section 8: the corpus and predictions formats, `validate`, and `score`, which recomputes the
disposition itself with I2's function. Commit corpus v0: at
least 12 hand-written seeds for each v1 intent, at least 60 out-of-scope and near-miss strings and
at least 30 injection strings, all tagged `seed`.

**Done when:** `validate` fails on an unknown intent id, a duplicate id, a missing provenance, a
group split across splits, and a test utterance whose normalised text is in train or dev (each has
its own planted-defect test); `score` fed a synthetic predictions file with a known number of wrong
AutoRuns reports exactly that number and ignores a prediction's own claim of AutoRun; exit codes 0,
1 and 2 each have a test; and the scorer refuses a predictions file whose catalogue hash differs.

**Notes:** Rob reads the seed list before this merges: it defines what people say. No resolver
exists yet; that is the point (R8, trap T1).

### I4 · Corpus v1: practice sentences, expanded and reviewed · proposed · needs I3
**What:** grow corpus v0 into the corpus the experiments need (doc 21 section 8.1): at least 100
in-scope utterances per v1 intent, out-of-scope and near-miss at least 40% of the whole, at least
100 injection strings, across the registers of 8.1 (terse, polite, typo-laden, non-native phrasing,
simulated speech-to-text confusions). Generated paraphrases are tagged `synthetic`, keep their
generating prompt in the repo, and share their seed's `group`. Splits are by group.

**Done when:** `morph-intent-eval validate` passes on the corpus; a committed report counts
utterances by intent, scope, provenance and split and meets the sizes above; a human-reviewed sample
of at least 10% of the synthetic rows is recorded with the fraction rejected; and no generated row is
tagged `seed`.

**Notes:** needs Rob's decision D3 first (whether Morpheus may generate the sentences). Synthetic
text is kinder than real people, so every later number is reported by provenance and labelled
Simulated until real, consented utterances exist. Trap T2: the validator's leakage check is the guard.

### I5 · Tier 0: the template matcher · proposed · needs I1, I4
**What:** the template language of doc 21 section 5 (alternatives, optional words, slots) and an
exact matcher over normalised text, with entity lists supplied at run time. Registration rejects two
intents whose templates can produce the same sentence, and lists the collision.

**Done when:** every template expansion resolves to its own intent; a registered collision is
refused with both intents named; `morph-intent-eval score` for arm A on the dev split is committed
under `docs/intent/results/` as the baseline, with the corpus provenance mix stated; and a
measured p95 resolve time for 200 synthetic intents is reported and asserted under 50 ms in the profile
CI runs.

**Notes:** if the timing assertion flakes, say so and ask; do not delete or loosen it. Do not start from
a model or a library that needs a runtime. Pure Rust, no new
dependency without a line of justification in the PR.

### I6 · Tier 1: normaliser and fuzzy matcher · proposed · needs I5
**What:** the normaliser (case, punctuation, number words, units, contractions), token-level edit
distance against template expansions, fuzzy entity matching, and the margin rule.

**Done when:** arm B is scored against arm A on dev and the report is committed; the typo and
speech-noise tagged subset improves; wrong AutoRun does not rise; no near-miss resolves; and a test
shows an Ambiguous result (not a guess) when two entities are within the margin.

**Notes:** tune thresholds on dev only. The test split is not opened until I10's decision.

### I7 · The intent bar runs intents (T0 and T1 only) · proposed · needs I2, I6
**What:** replace `match_query` in `flow-shell` with the resolver. Enter resolves; AutoRun runs;
Confirm shows one line ("Open Wi-Fi? Enter / Esc"); NeedsPerson shows the plan and waits for a
person (and places `.rob`); Ambiguous shows up to three; NoMatch paints "no match" as today. Origin
is `Typed` for the bar. Existing session-focus behaviour becomes the `session.focus` intent.

**Done when:** the existing `match_query` tests still pass or are migrated with a stated reason; a
VM scenario by scripted input types "wifi" and a frame shows the Wi-Fi picker; types a phrase outside
the catalogue and the frame says "no match"; and types a NeedsPerson intent and nothing runs until a
confirm is scripted.

**Notes:** schedule after E4 and E5 settle; `flow-shell` is busy. Trap T6: typing must not wait.

### I8 · The embedding arm (offline experiment) · proposed · needs I4, I6
**What:** arm C of doc 21 section 8, run on a laptop with no image change. Choose a small
sentence-embedding model, verify and record its licence, size and quantisation. Report the residual
after arm B first (section 8.6).

**Done when:** `docs/intent/results/I8-<date>.md` holds the residual, then arm C against arm B on the
test split, once, judged by the pre-registered bar (B1 to B6), with latency and memory on named
hardware labelled as an x86 proxy where it is not a Pi.

**Notes:** the test split is touched once for this decision. Rob sets the bar numbers first (D2).

### I9 · The small-model arm (offline experiment) · proposed · needs I4, I6
**What:** arm D, with a grammar generated from the catalogue, temperature 0 and no chain-of-thought,
at least two model sizes (one fine-tuned on the train split, one general), plus arm E (a large
remote model, offline) as the ceiling. Verify and record each model's licence.

**Done when:** either `docs/intent/results/I9-<date>.md` judges arm D against arm C by the bar, or
the file records that the residual after arm C was under 5 points and arm D was therefore not run
(section 8.6). Each needs the pinned header of section 8.2.

**Notes:** trap T2. Fine-tune on train only; thresholds on dev only; test once.

### I10 · Decide and record · proposed · needs I8, I9
**What:** ADR 0015 records which tiers ship, their thresholds, the model-licence policy and what
runs out of process, with the numbers from I8 and I9 and their labels.

**Done when:** ADR 0015 is `Accepted` by Rob and indexed in `docs/adr/README.md`, and W3's text
matches it.

**Notes:** Rob decides. If the answer is "T0 and T1 are enough", that is a good result.

### F1 · Evidence at the moment of failure · proposed
**What:** when `boot-check` decides *unhealthy*, the boot history also records why (doc 22 section 4):
the failed units and their results, up to five error lines each from their journal, the kernel taint
flag, and the data partition's free-space percentage. **Rust writes it** (`serde_json`, in
`morph-boot`), not `sh`. It is a new `"evidence"` field; existing fields do not move. Bounded to a few
KiB, best effort, and it must never fail or block a boot.

**Done when:** the `unhealthy` VM scenario reads `history.jsonl` from the disk (AGENTS H8) and finds the
broken service's name in the evidence; a planted journal line holding quotes, backslashes, a newline
and non-UTF-8 bytes produces a history line that still parses, and `morph-heal`'s `last_failure` still
reads the file; a planted failure to read the journal still records the decision and the boot still
falls back correctly; a journal flooded with 1 MiB of errors still yields a record under the cap; and
every existing scenario passes unchanged.

**Ask Rob to open:** F1 is the one worth acting on soon ([doc 22](22-diagnosis.md), D1). It
sits under M5, so he also marks it `parallel`, or it still cannot start.

**Notes:** traps T7, T8, T13. `morph-heal` errors on any unparsable history line and then refuses to
repair, so a corrupt line disables healing. Guard every step on this failure path (H11).

### F2 · Probes: registry, pure parsers and the first probes · proposed · needs F1
**What:** a new library crate `crates/morph-diag`: the typed `Value` (bool, int, text **and list**) and `Finding`, a sensitivity tag, a
probe registry that takes each probe's implementation, and a bounded runner (timeout, output cap, no
write access). The first probes are those of doc 22 section 5. Each parser is a pure function over
captured text, tested on fixtures taken from real `hwreport` folders and the VM.

**Done when:** `cargo test -p morph-diag` passes on Linux, macOS and Windows; every probe has a healthy
and a faulty fixture test; registration refuses a probe with no sensitivity tag or no timeout; the runner checks every answer against the kind the probe declared; the
runner kills a probe that sleeps past its timeout and truncates one that floods its output (a fake
probe for each); and the PR quotes a sabotage run for each rule.

**Notes:** traps T6, T9. Only the runner that reads `/sys` and calls `systemctl` may be Linux-only.

### F3 · Playbooks: the format, the validator, the engine and six seeds · proposed · needs F2, I1
**What:** the TOML playbook format of doc 22 section 6, a loader, `validate`, and `run` with a probe
budget, ported from the compiled reference in doc 22 appendix A. Six seed playbooks: `boot.trial_failed`,
`wifi.no_link`, `display.no_output`, `disk.low`, `service.crash_loop`, `data.mass_write`.

**Done when:** `cargo test -p morph-diag` includes the reference's tests ported over: each defect class
is refused with its own named error (unknown probe, wrong kind, bad jump, cycle, unreachable step,
`Certain` without citations, a citation not asked on the path, a repair not in the catalogue); a
validated playbook always ends within budget over the full input space; a failed probe, or one answering with the wrong kind of value, is `Unknown`,
never false; every seed loads, validates, and is tested against a fixture machine for each of its
verdicts plus a healthy one; and a sabotage run is quoted for each rule.

**Notes:** traps T2, T3, T12. A repair may name only an intent whose executor exists (I1's catalogue).

### F4 · Redaction · proposed · needs F2
**What:** `redact(bundle, policy)`, pure, working from each value's declared sensitivity (doc 22
section 8), and `morph-diag redact <report-dir>` for an `hwreport` folder.

**Done when:** a planted-secret test (a Wi-Fi password, an SSID, a MAC, a hostname, a username, a home
path, IPv4 and IPv6 addresses, a serial number) finds none of them in the serialised output under the
default policy; equal values map to equal tokens within one bundle and to different tokens under a
different salt; each sensitivity class has a sabotage run; and the CLI run on a fixture report folder
produces output containing no MAC address.

**Notes:** trap T11. This also lets a person share a hardware report without leaking their hardware.

### F5 · The judge for diagnosis · proposed · needs I3, F3
**What:** `crates/morph-diag-eval`, on the shared `morph-eval` core: the fault-corpus format, `validate`,
and `score` for the metrics of doc 22 section 9.3 (the disposition of any named repair is recomputed
with I2's function), with the same exit codes.

**Done when:** `validate` fails on each planted defect (a family split across splits; a record with no
expected diagnosis; a playbook that cites a withheld family; fewer healthy or wrong-symptom cases than
the minimums); `score` fed a synthetic predictions file reports exactly the planted number of
confident-wrong verdicts; exit codes 0, 1 and 2 each have a test; and at least six hand-written fixture
records are committed.

**Notes:** trap T4, T5. No fault harness exists yet; that is the point (G9).

### F6 · The fault harness (VM) · proposed · needs F1, F5
**What:** `images/x86-qemu/test-faults.sh`: inject at least 12 faults across at least 6 families into the
existing images (reusing the scenario machinery), including healthy-with-complaint controls; capture
each evidence bundle from the disk; run the diagnosis against it; and for each repairable fault, apply
the repair and run the health check again.

**Done when:** a local run writes a `faults.jsonl` that `morph-diag-eval validate` accepts; every
record's cause matches the recipe that injected it (a planted mismatch fails the run); repair outcomes
are read from the disk, not a log (H8); and the run's duration and CI cost are reported.

**Notes:** needs Rob's decision D3 (where it runs). Wi-Fi uses the simulated radios `test-wifi.sh`
already has; real firmware-missing evidence comes from E6 reports. Label results Simulated (T10).

### F7 · Playbook authoring by a model (offline experiment) · proposed · needs F6
**What:** arm B of doc 22 section 9.4. Measure the residual first. Then give a model the faults arm A
mis-handles and ask for playbook or probe additions as a diff, through Morpheus. Accept nothing that
fails `validate`, the held-out families or a person's review.

**Done when:** `docs/diagnosis/results/F7-<date>.md` states the residual; then either arm B against arm
A on the withheld families by the pre-registered bar (B1 to B6), once, or a statement that the residual
was under 10 points and arm B was therefore not run.

**Notes:** the withheld test families are touched once for this decision. Rob sets the bar first (D6).

### F8 · The runtime hypothesis proposer (offline experiment) · proposed · needs F6, F4
**What:** arm C: when a playbook returns `Unknown`, a constrained model names at most three causes from
the registered list and which registered probes tell them apart; the engine runs those under budget.
Include arm D (a large remote model, offline) as the ceiling, over redacted bundles only.

**Done when:** `docs/diagnosis/results/F8-<date>.md` judges arm C against arm A by the bar, or records
that the residual after F7 was under 10 points and arm C was not run.

**Notes:** G7 and G8. No raw log text reaches a model. Verify and record each model's licence.

### F9 · Diagnose from the intent bar · proposed · needs I7, F3
**What:** `diagnose.*` intents in the catalogue, resolved by the engine of doc 21 (no extra AI). The bar
shows the verdict and any repair offer through the disposition policy.

**Done when:** a VM scenario types "my wifi isn't working" with a planted fault and the frame shows the
verdict and a repair offer that waits for a confirm; on a healthy machine the frame says healthy and
offers nothing; and a `Likely` verdict never offers an automatic repair.

**Notes:** schedule after I7, which touches `flow-shell`.

### F10 · Decide and record · proposed · needs F7, F8
**What:** ADR 0016 records which diagnosis tiers ship, the playbook format, and the consent design for a
remote tier, with F7 and F8's numbers and their labels.

**Done when:** ADR 0016 is `Accepted` by Rob and indexed in `docs/adr/README.md`.

**Notes:** Rob decides. "Playbooks are enough" is a good result.

### W4 · Pi 5 parity · proposed
**What:** the Pi 5 image gets what the x86 image has: Strata on the persistent partition, `morph-update`
and `morph-heal` with a Pi backend, and the same health check, so the A/B trial, a rewind and a spare
rebuild behave the same on both.

**Done when:** the Pi image boots on a Pi 5 with the same A/B, Strata and heal behaviour.

**Notes:**
- **What the Pi image is today** (`images/pi5/`): rpi-image-gen's `image-rota` layout (bootconfig,
  boot_a, boot_b, system_a, system_b, persistent), Debian `trixie-minbase` (not Ubuntu 24.04 as A2
  says for the userland; flag it to Rob), and only `morph-boot`, the health check and the self-test
  driver (`layer/morpheos-core.yaml`). No Strata, no `morph-update`, no `morph-heal`. The persistent
  partition is mounted at `/persistent`, the x86 one at `/persist`: pick one name in this task.
- **It has never been booted.** There is no QEMU model of a Pi 5, so this task's Done-when needs a
  person with a Pi 5: Rob (and buying one, if he has none, is money). The result comes back on
  `MORPH-TEST-RESULT.TXT` on the FAT boot partition (`images/pi5/README.md`). Boot the existing
  self-test image before adding anything to it.
- **CI cannot read the slot root.** It is EROFS with 16 KiB blocks, and the runner's kernel has no
  erofs driver and `fsck.erofs` refuses that block size, so `scripts/verify-pi-image.mjs` only
  checks the superblock. Anything new on the slot is verified in the layer's own customize hook at
  build time (as the motd fix is), or by a newer `erofs-utils` (verify: which release reads 16 KiB
  blocks).
- `morph-update` writes an ext4 `root.img` on x86; a Pi slot is a vfat boot partition plus an
  EROFS system partition, and the slot's root comes from `root=/dev/disk/by-slot/active/system` in
  each rootfs's `/etc/kernel/cmdline`, not from `cmdline.txt`. The Pi backend in
  `crates/morph-boot/src/pi.rs` hard-codes partition numbers that `verify-pi-image.mjs` compares
  against each build.
- Split it: Strata on the Pi (built and checked at build time); the Pi update backend, with unit
  tests against the real layout; the Pi heal; then Rob's boot. Each is a PR; the boot is evidence in
  `docs/hardware/`.

### W5 · Morph Link on one platform · proposed · needs P1, P3
**What:** Morph Link on Windows (doc 16: "Phoenix-backup *of* the Windows machine"): a small agent
that keeps a P1 replica of a Windows user's folders on a MorpheOS peer or a USB disk. Windows first
because it is Rob's machine, the builder's machine, and doc 16's migration path (install Link, then
switching is a Phoenix restore).

**Done when:** one non-MorpheOS device keeps a Phoenix backup through it.

**Notes:**
- **It needs P1's format and P3's transport** (the heading lists neither; see the reply to the plan
  owner). The format must be a library that builds on Windows: no Linux-only dependency, the
  pattern AGENTS.md H9 set for `inotify`.
- A verdict across two operating systems: a `windows-latest` job backs up a folder, and a Linux
  job restores it with the identity and compares checksums. Then Rob runs it on his own laptop.
- Open files on Windows: consistent copies need the Volume Shadow Copy Service, which needs
  administrator rights. v0 can skip locked files and report them; that is Rob's call, since running
  as administrator changes what Link can do to a machine.
- Rob's decisions: Windows code signing (SmartScreen warns about unsigned programs, and a
  certificate costs money), and the licence line: Link is open (doc 00: Phoenix to your own devices
  is free), and a hosted relay is paid.

### W6 · Fleet (paid) · proposed
**What:** the paid console for many machines (doc 00: policy, attestation dashboards, update rings
beyond N devices). It is closed source and lives **outside this repository** (ADR 0004).

**Done when:** revenue-gated (doc 00). Rob decides.

**Notes:**
- Nothing is built for this task in this repository. Doc 00 and ADR 0014 gate it on revenue but
  name no figure, so the gate itself (what revenue, measured how) is Rob's to define.
- What the open side would have to offer a Fleet console, each already a task or a known gap:
  `system.update_ring`, which nothing reads (`unenforced_settings()`, AGENTS.md H5); signed updates
  (S2) and updates that arrive by themselves (S3); and attestation of which Manifest generation a
  machine runs (doc 07), which needs measured boot and is not built. Add those as `proposed` tasks
  when Rob opens W6, not before.

### W7 · Vendor layers, driver trials and a change freeze · proposed · needs E6
**What:** three generic extension points, so a vendor can ship its own appliance image on top of
MorpheOS without forking it (ADR 0006: *"MorpheOS adds layers on top"*; doc 01 principle 7):
(1) build an image from a pinned MorpheOS commit plus a layer directory that is not in this
repository; (2) trial a kernel and driver set as one A/B unit (doc 15, doc 19 section 4), with a
health check that a layer can extend; (3) a change-freeze flag that makes updates refuse and log
(doc 12 section 13: *"during studio sessions ... never run updates"*).

**Done when:** a VM scenario builds an image with a stand-in layer that adds one health-check unit;
a deliberately failing check makes the trial fall back (`b/trial/fallback`) and a passing one
commits (`b/trial/committed`); and with the freeze flag set `morph-update` refuses with a logged
reason, and without it proceeds.

**Notes:** proposed 2026-10-13. Not started: Rob opens it. Nothing in this repository may depend on
any particular vendor or layer. Open question: can a layer be added to a finished image, or must the
image be rebuilt from the pinned commit? ADR 0006 applies layers at build time, so the second is
the likelier. The signing key for a vendor's own updates is the vendor's (A4 and doc 20 S2 cover
MorpheOS's), so this task needs Rob's decision on the security model before it is opened.
