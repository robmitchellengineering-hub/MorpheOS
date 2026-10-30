# 2026-10-30 · DeepSeek · the door that a mode cannot shut

Rob: *"Thats our last option you have till the morning to get in."* So it has to work first time — which
means not another guess between the four candidates.

## Why this is different from the last five attempts

Every candidate shared one property: it lives in the **file** path.

| candidate | what it breaks |
|---|---|
| `/root` owned by a non-root uid | `StrictModes` refuses on ownership |
| `authorized_keys` missing / not regular / symlinked | sshd will not read it |
| root's `pw_dir` not `/root` | the relative `AuthorizedKeysFile` resolves elsewhere |
| `.ssh` inside a world-writable directory | `StrictModes` refuses the path |

sshd's **`AuthorizedKeysCommand`** path touches none of them: sshd runs a program as root and asks it for
the user's key. No ownership, no mode, no home directory — it is the mechanism certificate authorities use,
and it is *not* a bypass: the key is still a key, and only root's is offered.

## What ships

- **`keys-for`** — answers for `root` only, prints the baked pub key (`/usr/lib/morpheos/mesh-admin.pub`,
  falling back to the key file), prints nothing for any other name.
- **`/etc/ssh/sshd_config.d/50-morpheos-keys.conf`** — `AuthorizedKeysCommand … %u` and
  `AuthorizedKeysCommandUser root`. Both paths stay live: the file is still tried first.
- **The build asserts both files are present *and* that `sshd -t` accepts the config inside the image** —
  a drop-in sshd would reject is a machine with no door, and that must fail the build, not the boot.

## Verified here

| check | result |
|---|---|
| `keys-for root` | prints the 100-byte key |
| `keys-for bob` | **0 bytes** — no other account can be handed a key |
| `sh -n` on the script; `bash -n` on `build.sh` | clean |
| sshd parses the two directives in a real config | accepted |

**Not verified:** a real login through the command path. That is the morning's flash — and if it fails, the
image now *also* carries `access-selftest` (the door knocks on itself and records sshd's own reason) and
`morph-phonehome.service` (the reason is sent, not carried).
