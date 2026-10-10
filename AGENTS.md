# AGENTS.md

Operating rules for this repository. Read before changing anything.

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

- **On a non-Linux machine, `cargo test --all` runs everything except the three
  `strata` watcher tests, which report as `ignored`.** That is expected, not a
  failure — the watcher is inotify. Linux (CI, and the deployment targets) runs all
  of them. **If the ignored count ever grows, that is the thing to look at**: a
  silently skipped test reads exactly like a passing one. See H9.
- **Anything touching `images/` or `morph-boot` is additionally proven by the
  `images` workflow** — the x86 job builds a disk image and boots it in QEMU
  through **good, unhealthy and crashing** updates. Run it on your branch; a local
  compile says nothing about whether a firmware path works.
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
`boot.max_boot_attempts`, `heal.max_auto_rung` and `storage.retention` are parsed
and validated, and **no code consumes them**. The Manifest therefore describes
behaviour the system does not have — and a reader trusts it.
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
