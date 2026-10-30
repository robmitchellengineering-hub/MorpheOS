# 2026-10-29 · DeepSeek · the admin key the image carries, and the door it cannot open

Red-teaming our own booted machine (Rob: *"get in, report how you did, then make sure no one else
can do it again… a claim that is not shown to fail is false"*) found that the **intended** way in
does not work, and that the **unintended** one is documented in `HARDWARE.md`.

The probe, run against the live machine at `192.168.15.12`:

```
ok     A1 only sshd answers (22 open, nothing else of 25 services)
ok     A2 an unknown key is refused
FAIL   A3 the image's OWN admin key is refused — the remote-admin path this image is
       built to provide does not work: root@192.168.15.12: Permission denied (publickey).
ok     A4 only publickey is offered (publickey); no password to guess
```

The key is provably in the image (found at byte offset 413,889,090 of the flashed file) and
byte-identical to the private key on the admin machine — same `SHA256:BXBQNkqu…`. sshd offers
`publickey`, never accepts the offer, and denies.

## Why the build now pins and asserts what sshd checks

Three plausible causes, none provable without the machine's own state, and all cheap to eliminate:

- **`StrictModes`.** sshd refuses the whole `authorized_keys` file when the user's home directory
  is group- or world-writable, and the refusal is indistinguishable from "no matching key" at the
  client. `/root` was never `chmod`'d by the build — only `.ssh` and the file were.
- **`ProtectHome`.** Ubuntu's stock `ssh.service` may hide `/root` from the daemon, which makes root
  key auth impossible while looking exactly like a bad key.
- **The file not landing where sshd looks.**

So `build.sh` now: `chmod 0700` on `/root`; pins `AuthorizedKeysFile .ssh/authorized_keys`,
`StrictModes yes`, `PubkeyAuthentication yes`, `PasswordAuthentication no`,
`PermitRootLogin prohibit-password` in `sshd_config.d/`, rather than inheriting them;
adds `ssh.service.d/morpheos.conf` with `ProtectHome=no`; and **asserts the three modes**, failing
the build if they are wrong — because a wrong mode here costs a build, a flash and a boot to find,
and the symptom misleads.

## `images/x86-qemu/attack-probe.sh`

The seed of the attack suite: A1 (nothing but sshd listens), A2 (an unknown key is refused),
A3 (the intended key is accepted), A4 (no password authentication is offered). It exits non-zero
when a claim does not hold — A3 fails today, which is the point: a security claim that has not been
shown to fail is not a claim.

**Not verified:** that the fixes make A3 pass — that needs the next image build and a boot. What is
verified is that the probe reports the failure accurately, and that the build now refuses to ship
the modes that would cause it.
