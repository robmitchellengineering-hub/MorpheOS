# 0018 · Backups are Strata's own store, replicated, each object encrypted with `age` to a key the machine cannot read with

Status: **Proposed**. Waiting on Rob (task P1 in [doc 20](../20-build-plan.md)). A builder must not
build P1 past its step (a) until this says Accepted. Rob accepts it by answering the three
questions in "What Rob decides" below; the builder then records his answers here, sets the status
to Accepted with the date, and adds the row to [README.md](README.md).

## Context

Doc 15 promises "continuous encrypted replication to at least one other place", with keys "held by
the user and escrowed only if they choose". P1 is the first piece: a second copy of `/persist` on a
plugged-in USB disk, kept up to date by itself, and a command that restores it.

Three things already exist that shape the choice:

- **Strata's store** (ADR 0007) already holds every version of `/home` as content-addressed,
  BLAKE3-named objects with an SQLite index. Its history is the user's timeline (N3).
- **The machine can be taken over.** The ransomware scenario (`test-ransomware.sh`) assumes code
  running as root on the machine. A backup that such code can *read* leaks every old version. A
  backup it can *delete* is gone.
- **The same format must serve later work:** P3 (a peer's machine holds a copy) and W5 (a
  Windows agent). A peer must not be able to read what it holds.

## Options

| | Mechanism | For | Against |
|---|---|---|---|
| **A** | Strata's store replicated: each object encrypted with `age` (Rust crate `age`, MIT OR Apache-2.0) to an X25519 recipient | One history: a restored machine keeps its timeline. Dedup and BLAKE3 integrity already exist. Copying only what is missing is a set difference. The machine holds only the public half, so it can add to the backup and cannot read it. The same format serves P3 and W5 | New code: object naming, an index snapshot, restore |
| B | restic (Ubuntu 24.04: 0.16.4) on a timer over `/persist` | Mature, quick to ship | A second store and a second history beside Strata's. One password both writes and reads, so a taken-over machine can read every old backup. A Go binary in the slot |
| C | A LUKS disk with a plain copy | Kernel crypto, simple | Readable whenever unlocked; a peer (P3) would hold readable data; no history |

## Decision (proposed)

**A.** In detail:

1. **The machine holds only the recipient** (the public half), in `/persist/strata/replica.toml`
   with the disk label. The **identity** (the secret half) is made on the machine at
   `strata replica init`, shown once as a recovery code, and then erased from the machine.
2. **No hand-written cryptography.** The only primitives are `age`'s. Object names on the disk
   are a *keyed* BLAKE3 of the local name, with the key inside an `age`-encrypted header, so a
   disk holder cannot test whether you have a given file.
3. **The replicator never deletes or overwrites on the disk in v0.** Each object is written to
   a temporary name, fsynced, and renamed. Pruning waits until `storage.retention` is obeyed
   (S11, S7).
4. **v0 never formats a disk.** The person's disk must already carry the label `MORPH-BACKUP`.
5. **No disk is not a failure.** With no disk attached the service records "no backup disk"
   and exits 0. A failed unit is an unhealthy boot (ADR 0008), and a laptop without its backup
   disk must still take updates.

## What Rob decides

1. **Mechanism:** A (recommended), B or C.
2. **The key model:**
   - **(i) A public-key recovery code. Recommended.** The `age` identity, `AGE-SECRET-KEY-1…`,
     is 74 characters. It is strong, but nobody remembers it.
   - (ii) A passphrase, through `age`'s scrypt recipient. It is easier to remember and weaker,
     and the machine must ask for it on every push. That breaks point 1: the machine would then
     hold something that can read.
   - Recommendation: (i), plus an optional passphrase-encrypted copy of the identity that the
     person can keep anywhere.
3. **The form of the recovery code:** printed, a QR code on screen to photograph, or a file
   written to a second stick. Recommended: shown on screen as text and QR, with a typed check
   of its last 8 characters before it is erased.

The `age` crate is a new dependency. Accepting A accepts it. Its version is pinned when P1 adds
it to `Cargo.toml`, and the builder checks its licence and maintenance then.

## Consequences

- A stolen or taken-over machine can add to its backup and cannot read it. A person who loses
  the recovery code loses the backup, by design. The shell must say so in plain words when it
  shows the code.
- Restore needs the identity, so it runs where the person types or scans the code: on the new
  machine in v0, and in the recovery system (S4) later.
- P3 and W5 reuse the format unchanged. A peer stores encrypted objects it cannot name or read.
- The task's step (b) also lists `/storage/replicas` in `unenforced_settings()` until `morphd`
  (S11) writes `replica.toml` from the Manifest (AGENTS.md H5).
- Targets, not Measured: a save is on the disk within 60 s of the watcher recording it, and a
  second push of an unchanged store writes no object.
