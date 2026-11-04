# 2026-10-31 · DeepSeek · P2: the seal becomes mandatory

## What ships

`SealedSession` (+ the `morph-session` binary): a session opened **with a key**, where every change goes through
one door, a failed change is taken back and leaves **no seal**, and an undo is itself a sealed transition — an
undo is a change, so it must be as checkable as the change it reverses.

```
keygen   → verifying key fbacd6a4…3a9e
change 1 → sealed: echo one > /home/n.txt … (transition 1793480214732204211)
change 2 → sealed: echo two > /home/n.txt … (transition 1793480214834422443)
history before undo: 2 entries
undo     → undone and sealed: 2 paths restored (transition 1793480214862985545)
history AFTER undo: 3 entries, all "verified"
the file now says: one
```

A failing change, from the same run:

```
run --isolated -- 'echo "half" > /home/note.txt; exit 7'
morph-session: refused: the change failed (refused: the command exited 7) and was taken back
              (2 paths restored); no seal was written
→ the note still said "first draft"; the history still had one seal
```

17 tests in `morph-capsule` pass on the mesh, including
`a_failed_change_is_taken_back_and_leaves_no_seal`, `an_undo_is_itself_a_sealed_change`,
`a_foreign_key_cannot_vouch_for_this_history`.

## Two real design bugs, both found by running it end to end

**1. A change and the undo of it shared a filename.** Seals were named by `record.at` — the moment *before* the
change — so a change and its undo wrote to the same file and one silently overwrote the other. The history lost
an entry and nothing said so. Each transition now carries `sealed_at`, its own moment, and files are named by it.

**2. The history lived inside the tree that undo rewinds.** Seals sat in `session/.transitions`, so the first
end-to-end undo **deleted the seal recording the change it was undoing**: after the undo, the history held only
the undo. They live at `<root>/transitions` now, outside anything a rewind touches. A history inside the thing
it describes is not a history.

Neither bug was visible in a unit test's shape; both were obvious the moment a person-level sequence ran.

## What mandatory sealing does and does not guarantee

- **Guarantees:** every change that goes through `change()` — and the CLI only offers that door — is signed, and
  a record whose signature does not cover it is refused when the history is read.
- **Does not guarantee:** that nothing was written *around* the door. A caller with write access to the layer
  can still change files without a seal. Closing that needs the store to be read-only to everyone but the
  session owner — a mount-level property, and the next step. The module says this where it matters, in the
  code, not only here.

## Still to do (goal item 3)

Install `morph-session` in the image with a build-time presence assertion, and build the image green — the same
treatment `morph-attest` already has.
