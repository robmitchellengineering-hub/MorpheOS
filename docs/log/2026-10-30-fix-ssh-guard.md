# 2026-10-30 · DeepSeek · the SSH guard: enforce at boot what the build only asserted

Four builds went into a flashed image refusing the key it carries, and the network could not say why.
Fan-out research (see [PLAN-morpheos-sshd-access.md] on the planning side) changed two beliefs:

1. **A read-only `/root` is not a cause.** Tested live against the system sshd under a private mount
   namespace: with `/root` bind-mounted **read-only**, key authentication still returns
   `Accepted publickey`. `StrictModes` is a pure `stat()`; sshd never writes to the home directory.
2. **`StrictModes` refuses on OWNERSHIP as well as mode — and the build only ever set the mode.**
   `chmod 0700 /root` does nothing when `/root` is owned by another uid. The client sees exactly what
   we saw: `Offering public key … Permission denied (publickey)`, with no `Server accepts key`.

Also ruled out, so no more builds go into them: no OpenSSH 9.6p1 authentication bypass exists
(CVE-2024-6387 does not apply to Ubuntu 24.04 — Qualys notes its socket-activation patch disables the
race); a hostile DHCP/DNS/TLS server buys a MITM but no authentication and cannot forge our signed
update; IPMI/AMT need a BMC; PXE needs a reboot and the same L2 segment.

## What ships

`morph-ssh-guard.service` (before `ssh.service` **and** `ssh.socket`, because Ubuntu 24.04
socket-activates sshd) runs `ssh-guard` on every boot:

- `mkdir` **then** `chown 0:0` **then** `chmod 0700` on `/root` and `/root/.ssh`, `0600` on the file —
  the ordering matters and the guard's own test caught it: a `chmod` that runs before a `mkdir`
  fails silently and leaves the directory at `0755`, which `StrictModes` then refuses;
- installs the baked key when the file is missing, empty, or holds a different key (append, never
  replace);
- **reports** the home path, the modes, the key fingerprint and any symlink — the lines that would
  have ended this four builds ago.

The build installs the key twice on purpose: `/root/.ssh/authorized_keys` and
`/usr/lib/morpheos/mesh-admin.pub`, so the guard has an authoritative copy to restore from.
`ExecStart=-` and best effort throughout: a repair must never be able to fail a trial boot and roll
an update back.

## Verified here

| check | result |
|---|---|
| fresh home | dirs `0700`, key installed, fingerprint `SHA256:BXBQNkqu…` — the expected key |
| wrong key already present | **appended** (2 lines), not replaced |
| `bash -n` on the guard and `build.sh` | clean |
| build asserts the guard, the pub key and the unit are in the image | added |

**Not verified:** that it fixes the refusal on real hardware. That needs the next flash — and the
guard's own report line in the hardware report is what will say so either way.
