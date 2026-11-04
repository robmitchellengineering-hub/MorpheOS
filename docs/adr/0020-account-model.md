# 0020 · One owner with a password chosen at first boot, in a user database on `/persist`; root is locked

Status: **Accepted** — 2026-11-04. Rob answered all three questions in "What Rob decides"
below (his words are recorded in `docs/log/2026-11-04-plan-secure-kernel.md`), and S5 in
[doc 20](../20-build-plan.md) is open and marked `parallel`, so the account work may start
beside the express road. E11's first half (the shell not running as root) is unblocked by it.
From here this record is not edited; a change to the model is a new ADR that supersedes it.

## Context

Today there is no ordinary account:

- Everything runs as root: the shell (`flow-shell.service`, "ROOT FOR NOW") and Firefox.
- The images log root in automatically on `tty1`, the serial console and VT2. HARDWARE.md tells
  people that VT2 is a root shell. It is also how Rob debugs real hardware.
- The Manifest has `[[users]]` (`name`, `admin`, `recovery_key`) with invariants, and nothing
  creates an account from it. It is listed in `unenforced_settings()` (H5).

The root filesystem is read-only, so `useradd`, `passwd` and `chpasswd` (which lock and rename
files in `/etc`) fail. Any model with a password chosen on the machine needs a writable user
database elsewhere.

## Options

| | Model | For | Against |
|---|---|---|---|
| 1 | **An appliance:** one account `morph` baked into the image, autologin, no password; admin is physical presence plus the `.rob` mark | Smallest; the account database stays in the read-only root | Whoever holds the laptop is its owner. Cannot fit doc 09's "keys unlocked by user auth", so it would be replaced before disk encryption |
| **2** | **One owner with a password chosen at first boot**, in `libnss-extrausers` (`/var/lib/extrausers`, bind-mounted from `/persist`); the shell's own login and lock screen; admin through `morphd` with the same password, writing `.rob` with the user's name | The pattern Ubuntu Core uses for a read-only root. Grows into 3 without a rewrite. Fits disk encryption later | First-boot window, a login screen and PAM work |
| 3 | Several people, with the owner as admin; per-user sessions, perhaps `systemd-homed` | Right for a family machine | A week or more on top of 2, and not needed for N1–N5 |

## Decision (proposed)

**Model 2.**

1. **The account lives on `/persist`.** `/etc/nsswitch.conf` reads `files extrausers`, and PAM
   uses `pam_extrausers`. `/persist/extrausers` is bind-mounted onto `/var/lib/extrausers`.
   The Manifest's `[[users]]` records the name and the admin flag (risk tier 4, `.rob`). It
   never records the password.
2. **First boot shows one window:** a name and a password, typed twice. Until it is done there
   is no account, and the shell shows nothing else.
3. **Root is locked** (`passwd -l root` at build time) and there is no `sudo`. The person is not
   in the `disk` group. A test asserts this by attempting the writes, not by reading
   `/etc/group`.
4. **No root autologin in `plain`** on `tty1` or VT2. Test images that need a console keep
   theirs, through their own overlays.
5. **The debugging console moves to the recovery system (S4),** which is reached on purpose and
   writes nothing to `/persist`. Until S4 exists, Rob's hardware sticks keep the VT2 console
   (see "What Rob decides", question 2).
6. **The shell runs as that user** (E11): in the `video` group for `seatd` (**verify:** that
   Ubuntu's `seatd.service` uses `-g video`), with a logind session (`PAMName=login`).

## What Rob decided (2026-11-04)

1. **The model: 2.** One owner, a password chosen at first boot, in the database on `/persist`.
   The accounts, the locked root and the shell's login screen all follow from it.
2. **The root console on VT2: kept on the hardware test sticks only** (`overlay-hardware`),
   with a visible "root console: test stick" banner, and **removed from `plain`**. Rob's own
   debugging console therefore survives until S4's recovery console replaces it, and the
   shipping image stops being a passwordless root shell for anyone who reads HARDWARE.md.
3. **A forgotten password: the recovery system (S4) resets it** after a confirmation typed on
   the machine — physical presence, the same threat model the stick already uses. Not the
   Manifest's `recovery_key`, and not "nothing".

## Consequences

- `test-boot.sh`'s root-login check changes to the new login, typed through QMP. It is never
  deleted. S5's Done-when is a scenario that logs in and fails three writes, with the verdict
  read off the disk.
- Strata's store stays root-only (0700). S6's timeline then needs a privileged path, through
  `morphd`, once the shell is not root.
- `/users` leaves `unenforced_settings()` when `morphd` (S11) creates the account from the
  Manifest. Until then the first-boot window is the only writer, and the list says so.
