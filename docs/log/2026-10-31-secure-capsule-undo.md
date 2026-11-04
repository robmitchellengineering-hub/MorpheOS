# 2026-10-31 · DeepSeek · P1, first rung: a capsule whose state can be undone under a running app

Rob picked P1 from D22: make the Session and Strata the system rather than a demo. The hardest and most
falsifiable piece is done first — **undo that reaches a running application**.

## What exists now

`crates/morph-capsule` — doc 05's capsule in the smallest form that is real:

- a **user layer** the application lives in (`<root>/layer`, with `HOME` inside it);
- every file versioned **from the first write** by `strata` (content-addressed, FastCDC + BLAKE3, already in
  the repo and already real);
- **undo** = `strata.rewind(layer, at)` — restore the versions in force at a moment, remove what was created
  after it;
- a **manifest** that records the rung honestly (`layer-only (no namespace yet)`) and says the capability set
  is **not enforced** — an empty allowlist that looks like an enforcement list would be a lie;
- writes that escape the layer are refused (`'../escaped.txt' escapes the capsule layer`).

## The falsifier, and the false pass it caught

The first version of the test spawned a long-running process, undid the change, then asked **a newly spawned
process** what the file said. That proves the file changed on disk and **nothing about a running
application** — the same class of false pass as a tamper test that sets a field to the value it already had.

The version that passes reads the running child's **own stdout**:

```
test tests::undo_reaches_a_running_application ... ok
```

It asserts the child reports the pre-undo state (`after`) first, then performs the undo, then waits for the
*same* child to report `before`. A rollback that needs a reboot is a backup tool; this one does not.

All five cases pass on the mesh node (`morpheos-vm`):

```
a_write_is_versioned_and_undo_restores_the_earlier_content ... ok
undo_reaches_a_running_application ... ok
files_created_after_the_undo_point_are_removed ... ok
a_capsule_cannot_write_outside_its_layer ... ok
the_manifest_records_what_the_capsule_can_and_cannot_do ... ok
```

## The honest limit, stated in the code and here

An application that **caches** its state in memory will not see the reverted file until it re-reads it. What
the OS guarantees is that the state *under* the process is reverted without a restart; making a process's
*memory* rewind is checkpoint/restore (CRIU-class), which is doc 12's "deterministic replay" — a later rung,
not this one.

And the capsule is **not yet a sandbox**: rung 1 of doc 05's ladder is namespaces and cgroups. The layer and
its undo came first because that is where the product's promise lives; the namespace is the next step, and the
manifest says so rather than implying otherwise.

## What remains for P1

1. **Session as the unit** — `flow-session` has a `Session` (id, label, openables); it needs the capsule
   reference and a `strata://` query for its files, and `flow-shell` must treat the Session as the unit of the
   session manager rather than a window list.
2. **One capsule running a real application** through rung 1 (the namespace), not a shell loop.
3. **One transition spanning both** — a change made inside a capsule *and* a file in the session's own
   directory undone as a single unit.
