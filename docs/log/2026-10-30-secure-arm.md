# 2026-10-30 · DeepSeek · the Security arm: the suite lives here, and the door tests itself

Rob: *"Keep suite saved in new security morpheOS arm"*, and, on walls: *"Or we use the wall or we build to
get over it"*.

## The arm

`secure` is the Secure edition of D17 — the same core, posture as the product, every claim a falsifier.
`security/README.md` is the charter and the measured scorecard; the attacks live in `security/attacks/`.

## Over the wall

The wall was never "nobody can get in" — it was **the machine would not say why it refused**. So the image
now knocks on its own door, every boot, with no operator and no network:

1. `sshd -T` — the configuration the daemon actually resolved, not the file that looked right.
2. A throwaway `sshd -ddd` on port 2222 reading the **real** `/root` through the **real** `StrictModes`,
   with its own host key in `/run` — so the test exercises the exact path that failed.
3. A key generated on the machine tries to log in. It never leaves the machine, and it is deleted after.
4. **sshd's own words** for the refusal go into the hardware report:
   `Authentication refused: bad ownership or modes for directory /root`, or
   `User 'root' authorized keys … is not a regular file`.

That is the line five builds were spent guessing at. With `morph-phonehome.service` (#167) it also arrives
in the operator's chat, so the next refusal is answered on the boot that produced it.

## Verified here

| check | result |
|---|---|
| `access-selftest --self-test` | an accepted key recognised · a modes refusal named · a non-regular key file named · neither → `UNKNOWN` |
| the three attacks' self-tests | console both ways · disk both ways · update notices acceptance |
| `bash -n` on the script and `build.sh` | clean |
| build asserts the script and the unit are in the image | added |

**Not verified:** the live knock — that needs a boot; the extraction logic and the report path are what
this Mac can prove.
