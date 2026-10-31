# 2026-10-31 · DeepSeek · a check must live where the promise is made

Three times today a new assertion turned the A/B job red **while the thing it guarded was correct**.

| check | where it was | why it was wrong |
|---|---|---|
| `missing /usr/bin/gcc` | every image | `DEV_PKGS` rides only on the person-facing image; the test scenarios are built small on purpose |
| `identity file /etc/os-release is missing` | the shell-overlay path *and* `make_root` | the A/B fixture is built from a path that never receives the identity overlay |
| `no aplay in the image` | the shell-overlay path | `theme.wav` lives in the browser overlay, which the A/B fixture does not get |

Each one was a true statement about an image that never claimed it. The A/B job is a **test fixture**, not a
person's machine; asserting product promises on it proves nothing and blocks everything.

## What is there now

- The compiler check lives on the path that installs `DEV_PKGS`.
- The identity and audio checks **report what they see** and do not fail the build; the real verification is
  on the built image, where it belongs (and where the 1.1 image was verified: `NAME="MorpheOS"` grepped from
  the raw disk, and the boot sound heard in a VM for eleven seconds).
- The warning text names the directory, so the next person does not need a round trip to learn where it
  looked: `WARNING identity: /etc/os-release is NOT in …/root-a (the overlay copy did not land here)`.

## What this cost, and the rule it bought

Four CI cycles on identity and two on audio, at roughly twenty-five minutes each, on checks that were
red for a reason that had nothing to do with the change. The rule:

> **Assert where the promise is made, or do not assert.** A warning that names what it saw is worth more
> than a red gate nobody can explain — and a gate that is red for the wrong reason teaches the team to
> ignore gates.

## Still open

The A/B job's own `verify-motd.mjs` now fails with `welcome header printed 0 time(s)` on the identity
branch: the fixture has no identity files, so the console never prints the MorpheOS welcome. That is a
consequence of the same fact — the fixture is not a person's machine — and it is the next thing to fix
properly, by giving the fixture the identity files it asserts (or by moving the motd check to the
person-facing image), rather than by weakening the check.
