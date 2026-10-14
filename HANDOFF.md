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
| Bootable x86 USB image for a person: the shell, Firefox and the Wi-Fi picker, laptop firmware and a hardware report (E4, E12) | **builds, boots in QEMU** | [HARDWARE.md](images/x86-qemu/HARDWARE.md); `test-boot.sh` reads the screen and the report back. **Never booted on real hardware** |
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

#42 to #61 were merged on 2026-10-13 and 14 (#56 and #57, docs 21 and 22, too), as merge commits. Open when this was written:

| # | Branch | What |
|---|---|---|
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
   add a file to the **Session log** (`docs/log/<date>-<branch>.md`; see the section below).
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

- **The intent engine plan ([doc 21](docs/21-intent-engine.md), tasks I1–I10 in doc 20).** Seven decisions
  (D1–D7, section 11): when to open I1–I3 (recommended: now, as the lane for the days you wait on Rob's
  hardware reports), the evaluation bar's numbers, whether Morpheus may generate the practice sentences, the
  model-licence policy, the template syntax, vendor-registered intents, and English only. Until Rob opens
  them, I1–I10 are `proposed`: do not build them.
- **The diagnosis plan ([doc 22](docs/22-diagnosis.md), tasks F1–F10 in doc 20, stacked on doc 21).** Six
  decisions (D1–D6, section 12). The one worth acting on soon is **F1** (D1): when a trial boot fails, the
  history does not say why, because the failed units go only to the console log. F1 records them, serves
  the wedge directly, is small, and is provable in the existing VM suite. Until Rob opens them, F1–F10 are
  `proposed`: do not build them.

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

**One file per piece of work, in [docs/log/](docs/log/README.md)**, named
`<date>-<branch>.md` (`2026-10-16-feat-e7-installer.md`): the date, who, what was done, what was
found, and what is next. A new file per pull request means two pull requests never edit the same
lines, which is what made every merge on 2026-10-14 conflict here. CI refuses a pull request
that adds no log file (`scripts/verify-pr.sh`). The entries before 2026-10-15 are in
[docs/log/2026-10-13-to-14-history.md](docs/log/2026-10-13-to-14-history.md).
