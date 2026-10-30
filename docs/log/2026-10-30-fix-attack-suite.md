# 2026-10-30 · DeepSeek · the red-team suite: A5, A6, A7, and each one proved able to go red

Rob: *"Build the suite"* — after the fair question "can't or won't". A1–A4 (`attack-probe.sh`) covered the
network. The three attacks that matter most were missing, and none of them needs entry:

| attack | claim it falsifies | file |
|---|---|---|
| **A5 console** | "the console is protected" | `attack-console.sh` |
| **A6 update** | "only a signed payload installs" | `attack-update.sh` |
| **A7 disk** | "data at rest is protected" | `attack-disk.sh` |

Every one follows the repo's rule that a check must be able to fail: each has a `--self-test` with
fixtures that must pass **and** fixtures that must be caught, plus an explicit `UNKNOWN` (exit 2) path for
"the test could not reach its subject" — because a test that cannot reach the thing under test is not a
pass.

## What each proves

- **A5** reads the console capture and asks whether a credential was demanded. It goes red on a bare
  `root@…#` prompt and *also* distinguishes "never reached the console" from "went red". Today the real
  answer is the finding already in `HARDWARE.md`: **Ctrl+Alt+F2 is root with no password**.
- **A6** attacks the update path from the outside: an unsigned payload and a signed-then-tampered payload
  must both be refused. Its self-test proves the harness notices an installer that accepts everything —
  a rubber-stamp harness is worse than none.
- **A7** reads a data partition and looks for `psk=`/`password=` in NetworkManager keyfiles. It goes red
  when a credential is readable, green when the keyfile carries none, and `UNKNOWN` when it cannot mount.
  Today the real answer is finding F2: **the Wi-Fi PSK is plaintext on an unencrypted stick**.

## Verified here

```
ok   a login prompt passes
ok   a login plus password prompt passes
ok   a bare root shell is caught
ok   a capture that never reached the console is UNKNOWN
ok   a readable PSK is caught
ok   a keyfile without a secret passes
ok   a path that is not there is UNKNOWN
ok   a refusing installer passes
ok   an accepting installer is caught
```

All three self-tests run in `scripts/check.sh` under **"the red-team suite can go red"**, so the suite
cannot rot silently.

**Not verified:** the live runs (A5 needs QEMU, A6 needs a built image and a signed payload, A7 needs a
Linux mount). Those run on the Linux node or in CI; the self-tests are what this Mac can honestly prove.
