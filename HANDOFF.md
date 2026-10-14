# HANDOFF

From **2026-10-16** this repository is built by a DeepSeek agent (DSH) on Rob's **Windows
laptop at home** (host `win` in the harness-mesh repository's `MACHINES.md`), working inside
**WSL2 Ubuntu 24.04**. (harness-mesh is a private repository of Rob's, and you will not have it: the
machines that matter are named in doc 20, E6.) Until then it was built by Claude Code. This file is how the work
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

## Open pull requests

**Look at GitHub, not at a table here:** the repository's *Pull requests* tab (or
`gh pr list`; `setup-linux.sh` installs `gh`, and `gh auth login` needs the GitHub login Rob
gives you) is the only list that is never out of date. Before you start, read every open
pull request's title. **If an open `plan/` pull request changes the rules** (this file,
AGENTS.md, doc 20 or `scripts/`), ask Rob to merge it first, or base your branch on it and say
so in your pull request; otherwise you work to rules that are about to change.

If CI goes red on a pull request of yours, it is yours to fix, on that branch. **When a pull request's base is
another open branch** (a stack), and the base merges: if it was squash-merged, the next branch
may conflict where both touched the same lines; `git fetch origin && git merge origin/main`,
keep both sides' lines, run `bash scripts/check.sh`, and push. Never rebase or force-push a
branch that has a pull request open.

## How to work (every task, every time)

1. Run `node scripts/task.mjs --next`: the open tasks that can start now (their `needs` done,
   their milestone current; ⛔ marks one waiting on Rob). Take the one the milestone's **Order
   of work** in [doc 20](docs/20-build-plan.md) puts first, and read its card:
   `node scripts/task.mjs <ID>` prints its recipe, whether its needs are done, the hazards it
   cites in full, which paths it names exist, and what in it waits on Rob. If nothing is free,
   see "While you wait" below.
2. `git fetch origin && git switch -c feat/<id>-<slug> origin/main` (`fix/<slug>` or
   `plan/<slug>` for those). One task per branch, and its ID in the name
   (`feat/e2-classic-windows`). Never commit to `main`.
3. **Write the check first.** Every task's "Done when" names something a machine verifies.
   Make that check exist and fail *before* writing the code that makes it pass. If you cannot
   make it fail, it does not test anything (AGENTS.md H3, H9).
4. Implement the smallest change that makes it pass.
5. Write the session-log file (step 7) now, then `bash scripts/check.sh`. It must end with
   `all checks passed` (it counts the log file whether or not it is committed yet). The Rust version is pinned
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

**A test that fails once and then passes is a bug, not luck** (H13, H14: timing). Rerun it
once, to see whether it repeats. Either way, paste the failing output into your log file and
fix it on a `fix/` branch. If the fix is not yours to make now, add a `proposed` task for it.
Never rerun until green and say nothing: that is how a real race is hidden. (Example: #73.)

**When stuck for more than two attempts:** stop. Write what you tried and what you saw under
**Open questions** below, in a `plan/` pull request (with its log file), and ask Rob. A clear
question beats a third guess.

## Decisions waiting for Rob now

**`node scripts/task.mjs --rob` prints them**, from doc 20's three Rob fields (`**Waiting on
Rob:**`, `**Ask Rob to open:**`, `**Rob decides before <step>:**`; doc 20, "How this doc
works"). That is the only list: this file does not keep a copy, so it cannot go stale. Each
task's **Notes** lay out the options and a recommendation, and three are drafted as Proposed
ADRs for his answer: [0018](docs/adr/0018-backup-replica.md) (backups, P1),
[0019](docs/adr/0019-release-signing-key.md) (the release key, S3) and
[0020](docs/adr/0020-account-model.md) (accounts, S5).

### While you wait

This is the one place that says what to do when nothing you may start is free
(`node scripts/task.mjs --next` ends with "Nothing you may start is free").

1. **Ask Rob, once, the question that blocks the next task** in the milestone's Order of work.
   - **How:** Rob is the person who runs you. Ask him in the session, in one message: the
     task, the question, the options from its Notes, and your recommendation. If he is not
     there, put the same text under **For Rob:** in your next pull request's description
     (the PR template has the heading).
   - **Record that you asked:** append ` Asked: <date>, <in the session | in #<PR>>.` to
     that field's line in doc 20, in a `plan/` pull request. The next session then sees it
     was asked and does not ask again.
2. **When he answers,** the branch that acts on it records his words in its own log file,
   one line per task: `Rob on <ID>: <what he said, and when>`. It then removes or rewrites
   the field. `verify-plan.mjs` refuses, on any branch, a task opened, marked `parallel`, or
   freed from **Waiting on Rob:** without that line. So a pull request cannot do Rob's part
   for him.
3. **Meanwhile, only `fix/` and `plan/` work.**
   - `fix/` is a bug in something built.
   - `plan/` is checking a `verify:` fact and correcting its note, making a check stricter,
     or correcting a stale line. No code under `crates/` or `images/`: `verify-pr.sh`
     refuses that on a `plan/` branch.
   - Do not start a task that `--next` does not list, or one it marks ⛔: `verify-plan.mjs`
     refuses both.
   - Ready-made `plan/` work, if you want a first one: give `scripts/verify-docs.mjs` a
     real `--self-test` (doc 20, E6, note 3).
4. **If there is no `fix/` or `plan/` work worth doing,** say so to Rob and stop.

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

`scripts/verify-plan.mjs` enforces its format in CI. It refuses, with the reason:
- a `feat/` branch that does not name exactly one task that may start, meaning one whose
  needs are not done, one in a later milestone not marked `parallel`, or one marked **Waiting
  on Rob:**;
- any branch that is not `feat/`, `fix/` or `plan/`;
- on any branch, a change that is Rob's to make without his words in the log.

Use `fix/…` for bugs in built things and `plan/…` for plan and check changes; those two are
not tied to a task.

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
