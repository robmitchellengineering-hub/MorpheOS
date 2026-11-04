# 28 · Accounts, and who the shell runs as (S5)

**Status: decided — Rob chose A on 2026-11-04** (*"Do a"*). The shell now runs as `morph`; B remains the answer for
a shipped OS and its questions stay open. E11 is blocked on exactly one question and this document exists so the answer
can be made with the consequences in front of it, rather than as a guess. Nothing here is decided.

## The situation, measured

MorpheOS autologins **as root** on VT1 and runs the shell, Firefox and the Wi-Fi picker as root. That was a
deliberate first-boot choice and it worked: the machine boots, draws, browses and builds. It also caused four
separate problems on 2026-11-04, each fixed at the symptom:

| what broke | why root was the cause |
|---|---|
| **no audio at all** | `pipewire`, `pipewire-pulse` and `wireplumber` ship `ConditionUser=!root`; systemd skipped them, so no application could make a sound |
| the boot history was lost | `boot-check` writes to `/var/lib`, and on a read-only root `/var` is a tmpfs that empties |
| keys and hostname had nowhere to live | `/root` is on the read-only root; every credential had to be moved to `/persist` by hand |
| `apt-daily*` failed on every boot | an immutable root; the system reported `degraded` every time, so its reports stopped meaning anything |

A non-root session does not fix those by itself — the read-only root stays — but it makes the *first* one correct
instead of worked around, and it is what polkit, seat management and every desktop application already assume.

## Three options

**A · One ordinary user, placeholder (what E11 asks for).**

Create `morph`, uid **1000**, no password, autologin on VT1, shell and Firefox as `morph`; the **Wi-Fi picker stays
a root system service** (moving it breaks every join: NetworkManager authorises non-root callers through polkit and
`polkitd` is not in the image). Root console stays on VT2.

- *Cost:* small — a user, a `chown` of `/persist/home/morph`, a `tmpfiles.d` line so an updated stick keeps its data,
  `User=` on the shell unit.
- *Risk:* two identities in play (root services, non-root desktop). Acceptable while S5 is open, and exactly what
  the placeholder is for.
- *Buys:* audio works by default; `/root` stops being where secrets go; E11 can be finished and its check enforced.

**B · A real account system now (S5 in full).**

Per-user accounts with passwords, a login screen, polkit, an admin user and a standard user, sessions via logind.

- *Cost:* large — a login stack, polkit in the image, a decision about `sudo`, and a story for a machine with no
  keyboard.
- *Risk:* a MorpheOS that asks for a password on first boot is a different product promise than "boot and it works".
- *Buys:* the thing a person expects from an OS they share, and the foundation for accounts on a tablet or phone.

**C · Stay root.**

- *Cost:* nothing today.
- *Risk:* every one of the four problems above stays, each worked around separately; a root desktop is a security
  posture that has to be explained rather than assumed.
- *Buys:* nothing that A does not, except avoiding the decision.

## Recommendation

**A, now.** It is the smallest change that removes the root cause of the audio failure, gives keys and state a
sensible home, and lets E11's check — already built and reporting `compositor_uid=0` on the reference machine —
become a hard failure. B remains the real answer for a shipped OS and is not made cheaper by doing it under time
pressure; A is a placeholder with a name (`morph`) and no password, and it can be replaced without moving data.

What A deliberately does **not** decide: whether accounts are per-person, whether there is a login screen, and
whether the admin user is separate. Those are S5's questions and they stay open.

## What the answer changes in the image

- `useradd -u 1000 -U -M -d /persist/home/morph -s /bin/bash morph` (fixed uid, because `/etc/passwd` is in the
  read-only slot and every future image must agree with the one a stick already has)
- `chown 1000:1000 /persist/home/morph`, plus `tmpfiles.d`: `d /persist/home/morph 0700 morph morph -`
- `User=morph` on `flow-shell.service` (and `flow-browser`); `flow-wifi` stays root
- the seat session stays seatd with the user in its group, per E11's option (a)
- E11's check flips from reporting to enforcing the moment `User=` is set
