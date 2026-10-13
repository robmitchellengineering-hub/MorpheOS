# AGENTS.md

Operating rules for this repository. Read before changing anything.

**What to work on next, and how, is in [HANDOFF.md](HANDOFF.md).** It holds the current
state, the ordered task list with a machine-checked "Done when" for each task, and the
session log. Update it in the same pull request as the work.

**Read `docs/00-strategy.md` first.** It holds the strategic decisions, and it says
of itself: *"Where another doc disagrees with this one, this one wins."* The docs
describe an intended system; the code implements a small part of it. Both are
worth reading, and the difference between them matters more than either.

## What this is

An AI-native operating system, design phase. The wedge is decided and narrow:
**"never breaks, and heals itself."** Everything else — capsules, the AI
configurator, the mesh, Flow shell — waits behind it.

- **Phase 1 is only "unbreakable".** A change that does not serve it, or that
  weakens it, is out of scope even when it is a good idea. That is ADR
  [0003](docs/adr/0003-unbreakable-wedge.md), and it is the rule most likely to be
  violated by enthusiasm.
- **The Linux kernel is the base.** A new kernel is a research track, not a plan
  (ADR [0001](docs/adr/0001-linux-kernel-first.md)).
- **The userland is Apache-2.0.** Paid services are separate and closed. Code
  brought across from the proprietary sibling (Morpheus) has to be decided **file
  by file** — see [`docs/17`](docs/17-morpheus-family.md) and the licence risk in
  [`docs/11`](docs/11-risks-open-questions.md).

## How to make a change

1. **Branch from a fresh `origin/main`.** `git fetch && git switch -c <type>/<slug> origin/main`.
2. **Never push to `main`.** It is the published branch. Every change lands through
   a pull request.
3. **One worktree per session** if more than one session is working here. **Never
   `git add -A` in a shared checkout** — a blanket add cannot tell your work from
   somebody else's, and on the sibling repo it swept 89 unrelated files into one
   commit.
4. **Read `git status --short` before committing.** Files you did not touch are
   someone else's work in progress.
5. **One logical change per branch.** Small related changes may share a branch;
   unrelated ones do not.

## The verification bar

"It builds" is not verification. Before a pull request:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
for f in manifest/examples/*.toml; do cargo run -q -p morph-manifest -- validate "$f"; done
```

That is what CI runs. It is not the whole bar:

- **The bar is run on Linux.** On macOS or Windows, `cargo test --all` skips the three
  `strata` watcher tests (they report as `ignored`) and **compiles every
  `flow-compositor` test to nothing** (`#![cfg(target_os = "linux")]`, so they do not
  even appear as ignored). Most of M1 is compositor and image work, so off Linux the
  bar proves nearly nothing about it. On Windows, run it in WSL2 Ubuntu 24.04 (doc 20,
  S10). **If the ignored count ever grows, that is the thing to look at**: a silently
  skipped test reads exactly like a passing one. See H9 and H16.
- **Anything touching `images/`, `morph-boot`, `morph-update` or `strata` is additionally
  proven by the `images` workflow** (it runs on every pull request, H10) — the x86 job
  builds a disk image and boots it in QEMU through **good, unhealthy, crashing and
  updating** runs and a **ransomware** attack that Strata must undo. Run it on your
  branch; a local compile says nothing about whether a firmware path works.
- **A build is not a boot.** The Pi image building successfully proves that
  `rpi-image-gen` produced a file — nothing more. **No Pi image has ever been
  booted.** ADR [0006](docs/adr/0006-image-factory.md) concedes that real-hardware
  verification needs a person. Say "builds", not "works".
- **A claim about this repository is a test or it is deleted.** `README.md` said
  *"Every doc ends with open questions for that area"*; nine of twenty do. Nothing
  checked it, so it sat there being wrong. If a sentence describes the repo's own
  contents — a count, a path, "every X does Y" — write the check or take the
  sentence out.

## Hazards

Self-inflicted breakage that has already happened here, each with the rule it
bought. Add to this list when something breaks in a way a reader could repeat.

**H1 — `set -e` turns a diagnostic into a failed build.** `images/pi5/build.sh`
searched the build tree with `find … | head -1` under `set -euo pipefail`.
`rpi-image-gen` leaves root-owned, mode-700 directories in there, so `find`
printed `Permission denied` and **exited 1** — after a successful 3.29 GiB image
build. The job failed on **every run (0/3)** and the image was never published.
`head -1` closed the pipe early as well, SIGPIPE-ing `find` and tripping
`pipefail` for a second reason.
**Rule:** a command whose exit code reflects traversal or permission *noise* must
not be able to fail the script — normalise the exit code (`-print -quit`,
`2>/dev/null`, `|| true`) **and keep the substantive check** (here: an empty
result still fails). Suppress the noise, never the verdict.

**H2 — a job that has never been green is not a check.** The `images` workflow was
red on every run, so "CI is red" carried no information and a real regression
would have been invisible inside it.
**Rule:** fix a permanently red check or delete it. A check nobody can pass
teaches everyone to ignore the colour.

**H3 — a check satisfied by something other than its subject.** The general form,
seen repeatedly on the sibling repo: a guard that greps for a rule's *name* stays
green while the rule is disabled, because the name also appears in a comment, an
error message, or the condition next door.
**Rule:** assert the **condition**, scope it to **both ends** so it cannot pick up
a neighbouring function, and strip comments first.

**H4 — an unreachable branch in a classifier.** `plan.rs`'s `risk_of` can never
return `Risk::Cosmetic`, so tier 0 is dead and **every path the table does not
match silently lands at tier 2**. Nothing fails; the change is just classified by
a default.
**Rule:** a ranked classifier must be **total** over its schema, and every leaf
path should be asserted through `json_schema()`. An unmatched input is a bug, not
a fallback.

**H5 — a field nothing reads.** `system.channel`, `system.update_ring`,
`heal.max_auto_rung` and `storage.retention` are parsed and validated, and **no
code consumes them** (`unenforced_settings()` in `validate.rs` is the full list).
The Manifest therefore describes behaviour the system does not have — and a reader
trusts it. Another, `boot.max_boot_attempts`, promised a 3-boot retry the firmware
cannot do; ADR [0008](docs/adr/0008-one-try-trial.md) removed it.
**Rule:** a configuration field is not implemented until something obeys it. If
nothing does yet, mark it in the schema or leave it out.

**H6 — "nothing failed" is not "the gates ran".** GitHub does not always create a
workflow run for a pull request; a green preview with no CI run at all reads as a
pass. Verify the checks by **name** and confirm they exist before merging.

**H7 — the default slot is only proven by the flow, not by the API.**
`BootControl::set_default_slot` accepts any slot. The invariant "the default is
always a proven image" is a property of the current policy code, not something the
type or the API enforces.
**Rule:** an invariant that matters belongs where it cannot be bypassed. Until
then, say it is a policy property rather than a guarantee.

**H8 — a verdict scraped from a shared console.** The x86 update test read each
boot's decision from the serial console. On a KVM runner (run 38052762124) the
**good** scenario failed with B correctly committed (`loader.conf` said
`default morpheos-b.conf`): the decision happened, but the test found no
readable line for that boot. The same job had passed an hour earlier. The serial
log could not be retrieved to confirm the cause; the likely one is the kernel
printing into the middle of a line, since both share the console.
**Rule:** a test reads its verdict from the **record the system itself keeps**
(here the boot history on the ESP), not from a human-oriented stream. Logs and
consoles are diagnostics. Compare them to the record, but never decide from them.

**H9 — a Linux-only dependency that took the whole local bar with it.** `crates/strata`
declared `inotify` unconditionally, and `inotify` is a hard `compile_error!` off Linux,
not a missing symbol. So the crate did not merely fail to watch on a Mac: **it did not
compile**, which made `cargo test --all` — the bar this file tells every session to run
— impossible off Linux the moment Strata merged. It was found by building the branch,
not by reading it. The fix scopes the dependency to Linux and gives `watch` a non-Linux
stand-in that refuses with a reason.

**The second half is the trap, and it is why this is a hazard and not a chore:** the
stand-in fails for *every* input, and one of the three watcher tests asserts that
`Watcher::new` **fails** when the store sits inside the tree it protects. Off Linux that
assertion holds without checking its subject — the test would be green and prove
nothing. So all three tests are `#[ignore]`d off Linux with the reason attached, and the
skip shows up in the test count (`63 passed; 3 ignored`) instead of hiding inside it.
**Rule:** when you stub a platform out, audit every assertion that expects the stub's
failure. A negative test satisfied by "we never implemented this" is not a passing test.

**H10 — a required check that a path filter can skip, and a workflow nobody required.**
`main` sat **red** on the `images` workflow for hours, through three merged pull requests,
and every one of those merges was green. Both halves were needed for that to happen: the
two image jobs were **path-filtered** (so a PR touching only `crates/` or docs never ran
them) **and** they were **not required checks** (only `rust` was), so a red `main` blocked
nothing and warned nobody. It was found by comparing each workflow's latest run on `main`
against its trigger conditions — not by any failing gate, because there was no gate. The
fix for the red itself was sitting in a branch nobody had opened a pull request for.
**Rule:** a check that matters to `main` must be **required**, and a required check must be
one that **every** pull request actually runs. Those two constraints pull against each
other — requiring a path-filtered job makes it a wall that blocks every unrelated PR
forever, because it will never report — so the filter goes, not the requirement.
**If you rename a required job, update the branch protection in the same change**, or every
pull request waits on a check whose name no longer exists. And **"merged green" is not "the
repository is green":** check every workflow's latest run on `main`, not only the one your
pull request touched.

**H11 — one unguarded command on a failure path, and the run says nothing at all.** The x86
`update` scenario hung, twice, for the full harness timeout, and the job reported only *"the
VM did not power off cleanly"*. The cause was **one line**: the slot root is mounted
read-only — the loader entry passes `ro` — so `mkdir -p /mnt/update` failed with *"Read-only
file system"*, and the in-image driver runs under `set -e`, so the script **ended right
there**. No trial was armed, no poweroff happened, and the VM sat at a login prompt until
the harness gave up. The serial console said so in one line:

```
boot-check[292]: mkdir: cannot create directory '/mnt/update': Read-only file system
morpheos-a login:
```

**Rule:** on a failure path, guard **every** step, not the ones you were thinking about — the
first attempt at fixing this guarded the mount and the install and left the `mkdir`. And
**when a boot- or image-level test times out, read the console artifact, not the job log**;
the job log cannot know anything but that time ran out. A timeout is not a diagnosis. Two
900-second timeouts were spent on hypotheses (a dirty payload filesystem, a slow install)
that the console would have dismissed immediately — that log is uploaded by `if: always()`
for exactly this reason.

**H12 — a generator number is not `current + 1` when the pointer can move backwards.**
`morph-transaction` armed a trial for `current.number + 1`. That is right until a rollback:
generations are append-only, so after rolling back to 1 in a store that holds 1 and 2, the
next generation is **3**, not 2 — and the transaction then armed a trial for a number the store
would not use.
**Rule:** a number that identifies an entry in an append-only log comes from the log (a counter,
a max, a `next_number()` the owner exposes), never from the position of a pointer into it. Caught
here by the tests written in the same change, which is the only reason it is a rule and not an
incident: **if a store can roll back, `current + 1` is a different value from "the next one".**

**H13 — the thing under test beat the thing the verdict reads.** `test-shell.sh` took its
screenshot when the window appeared, then stopped the VM and read the boot history. Locally
(TCG, slow) the health check had always written the history by then; in CI (KVM, fast) the
window came first, the history did not exist yet, and the read under `set -euo pipefail` ended
the run with exit 1 and **no message**. It looked like a flake and was a race.
**Rule:** before reading a verdict, wait for the event that WRITES it (here, the
`morph-boot-check: decision` line), and guard every read on a failure path so that a missing
file is a reported FAIL, not a silent exit (H11). A test that only passes on a slow machine is
measuring the machine.

**H14 — an event source that never wakes.** The compositor found the keyboard and mouse at
startup, and announced them only after the first keypress, 540 seconds later. libinput queues
the devices it finds when the seat is assigned **without making its fd readable**, and the
event loop only drains libinput when the fd is readable. Nothing failed; the input simply
waited for a person to touch it. It was found because the VM test **waits for the devices to
be announced** before it sends any input.
**Rule:** after setting up a poll-driven source, drain it once by hand. And make a test wait
for the "ready" signal it needs instead of sleeping past it: a sleep would have hidden this.

**H15 — losing a shared resource is not a failure.** Ctrl+Alt+F2 made seatd take DRM master
back while a frame was in flight; the flush failed with `EACCES`, the compositor treated every
error as fatal, and **the shell exited**, so Ctrl+Alt+F1 had nothing to come back to.
**Rule:** anything the seat lends (the display, input devices) can be taken back at any
moment. An error from it while the session is paused, or about to be, means "not ours right
now". Only a test that switches away and **comes back** finds this; switching away alone
passed.

**H16 — a green bar on the wrong operating system.** `scripts/check.sh` passed on a Mac
while running none of the compositor's tests: they are `#![cfg(target_os = "linux")]`, so on
macOS they compile to an empty test binary that reports `0 passed` and nothing ignored. H9
had already shown this shape with three tests; with the compositor it is most of M1.
**Rule:** the bar is run on Linux, and `check.sh` says so instead of passing quietly elsewhere
(doc 20, S10). On Windows, that is WSL2.

**H17 — a protocol duty nothing tested.** Firefox put its window on screen and never painted
the page, and every test passed. A Wayland client asks to be told when its frame was shown
(`wl_surface.frame`) and draws its next frame only then; the compositor never answered, so
every client froze on its FIRST frame. The tests all looked at the first frame: `solid-window`
draws once, and GTK's widget factory drew everything in one go. It was found by the first
client whose content arrives in a later frame.
**Rule:** a test of a long-running client checks that it keeps going (here, `tests/toolkit.rs`
asserts GTK keeps asking for frames), not only that its first picture is right. A frozen
client and a finished one look the same in a single screenshot.

## Writing code here

- Match the existing style: doc comments that explain **why**, module headers that
  state the contract, and no `allow` attributes.
- **Prefer platform primitives over invented ones.** ADR 0006 chose Raspberry Pi's
  `tryboot` and systemd-boot's `bootctl set-oneshot` rather than a bespoke
  bootloader, and that is why the x86 path is the one thing here with a passing
  end-to-end test.
- **Numbers in docs are targets until measured.** `docs/13` prices the physics and
  `docs/11` lists what to validate; only the x86 A/B fallback has been measured.
  When you add a figure, say whether it was measured or intended.
