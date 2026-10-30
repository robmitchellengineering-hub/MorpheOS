# 2026-10-29 · DeepSeek · two rules from this session, and the gate that holds one of them

Both came out of the same night: the first boots of the x86 image on real hardware, a stick that
had to be flashed while Rob was away from the machine, and a Wi-Fi network the machine had to join.

## H22 — a credential in a published artefact, and a gate that could not fail

The ask was to put the work Wi-Fi SSID and password into the image so the machine connects by
itself. That image is built in CI **on a public repository**, so its artefacts are public: baking
the password in publishes it. The image already had the correct pattern — `morpheos-tailscale-up`
reads `/efi/morpheos-tailscale.key`, *"written at flash time, never in the public image"*.

The rule is now in `AGENTS.md` (H22) and enforced twice:

- **`scripts/verify-no-secrets.mjs`** scans the tracked tree from `check.sh`, with a `--self-test`
  that proves each rule can fail: a private key block, a `tskey-` pre-auth key, a real Wi-Fi
  PSK (the simulated access point under `overlay-*-test/` is allowed, and says why), and any
  `morpheos-wifi.txt` / `morpheos-tailscale.key` in the tree. 7/7 self-test assertions; the
  tracked tree is clean in 439 files.
- **`build.sh`** runs the same rule over the assembled root before packing, in bash, because node
  is not guaranteed inside the image build.

**And the gate was broken in a way that mattered.** The first version of the build-time line
passed `-----BEGIN …PRIVATE KEY-----` to grep without `-e`, so grep read the pattern as *options*,
exited 2, and — because the error went to `/dev/null` — the check reported "clean" over a tree
that held a live key. Replaying the fix against three trees now gives: clean → `clean`,
`efi/morpheos-tailscale.key` → `BLOCKED`, `tskey-…` in `/etc` → `BLOCKED`, and a deliberately
invalid pattern → `CHECK BROKEN rc=2` (fails the build). *A check that cannot fail is worse than no
check.*

## H23 — "it needs a password" is a claim about the machine, not about the task

Told the stick had to be flashed and that Rob was remote, this session said it could not be done
and offered to wait until he was at the keyboard. Wrong: `sudo -n -l` shows
`(ALL) NOPASSWD: /bin/dd`, a rule written on 2026-10-16 for this exact job. The flash then ran with
no prompt — 6,247,415,808 bytes in 5m13s, read back and hashed.

The rule (H23): **before reporting a privileged action as blocked, run `sudo -n -l` and read what
is already permitted.** Rob's own words, twice: *"I thought we figured out a way around this
before, some kind of elevated terminal"*, then *"remember what we do with walls"*.

This one is held by reading, not by a gate — a gate cannot see a sentence. What it can do is remove
the excuse: the command is in the rule, and `sudo -n -l` costs one second.

## Verified here

| check | result |
|---|---|
| `node scripts/verify-no-secrets.mjs --self-test` | **7/7 ok**, "every rule can fail" |
| `node scripts/verify-no-secrets.mjs` | `no credential in 439 files` |
| the build-time rule replayed on three trees | clean / BLOCKED / BLOCKED as above |
| a deliberately invalid grep pattern | `CHECK BROKEN rc=2` — fails the build |
| `bash -n scripts/check.sh images/x86-qemu/build.sh` | clean |

**Not verified:** the rule firing inside a real image build — that needs the next `images` run,
which is what the gate is for.
