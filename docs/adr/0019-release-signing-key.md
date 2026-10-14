# 0019 · Releases are signed offline by Rob, with two keys in every image and dated release numbers

Status: **Proposed**. Waiting on Rob (S2's notes, S3 in [doc 20](../20-build-plan.md)). Until it is
Accepted, every image trusts only a test key made for its own build, and nothing is published as
a release. Rob accepts it by answering "What Rob decides" below. The builder records the answers,
sets the status and date, and adds the row to [README.md](README.md). **A builder never makes,
holds or commits a release key** (AGENTS.md; HANDOFF.md).

## Context

What exists (S2, and S3 step 3.1):

- `morph-update` refuses a payload unless its `BLAKE3SUMS` has a valid ed25519 signature
  (`BLAKE3SUMS.sig`) by one of the keys in **`/usr/share/morpheos/update-keys/*.pub`**. That
  directory is on the read-only root, so it is part of the signed system.
- The manifest carries a required, signed `release <n>` line. `install` refuses a release lower
  than the running one (`/usr/lib/morpheos/release`).
- `build.sh` bakes in `build.pub`, from a key made per build unless `UPDATE_KEY` names one.
  `morph-heal` signs its one repair with a key made for that repair.
- **The signature is over `BLAKE3SUMS` alone**: four lines (the release and three digests). Signing
  does not need the 1.5 GB root image on the signing machine.

Rob's rule: no image a person uses should trust a key that lives in CI.

Two properties follow from the design and decide most of this ADR:

1. **Rotation is already possible.** The trusted set is part of each release, so release *n*
   says which keys may sign release *n+1*. To rotate, release *n* (signed by K1) ships K1 and K2.
   Release *n+1* is signed by K2 and may drop K1.
2. **Losing the only key strands every machine.** If every image in the field trusts only K1 and
   K1 is lost, no update can ever be installed again, and each machine needs a reinstall (E7)
   from a new stick.

## Options for custody

| | Where the secret lives | For | Against |
|---|---|---|---|
| **a** | A seed file on an encrypted USB stick, used only on a machine that is never online (an old laptop booted from a live stick) | Works today with `morph-update sign`, no new code. Only `BLAKE3SUMS` crosses to the offline machine and `.sig` back | A file can be copied. Its safety is the safety of the stick and the passphrase |
| b | A hardware key (YubiKey or similar) that signs ed25519 and never releases the secret | It cannot be copied | New code: `sign` must talk to the device. **verify:** which devices do *raw* ed25519 over arbitrary bytes. OpenPGP-card signing wraps the input; YubiKey PIV ed25519 needs firmware 5.7+ |
| c | A CI secret | Fully automatic | **Refused by Rob's rule.** A compromise of CI is a compromise of every machine |

## Decision (proposed)

1. **Custody (a) now, (b) later** as a new ADR when a device is chosen and tested. Moving keys is
   one rotation (property 1) and needs no other change.
2. **Two keys in every release image: `primary.pub` and `backup.pub`.** They are made on the
   offline machine and kept in different places, for example the primary at home and the backup
   with someone Rob trusts. Releases are signed with the primary. If one key is lost or
   suspected, the other signs a release that replaces it (property 1).
3. **Test images stay as they are.** They have a per-build key and no release keys, so a test
   image never accepts a real release, and a real machine never accepts a test payload.
   `build.sh` gains `RELEASE_KEYS=<dir>` to bake in the release `.pub` files instead of a test
   key. It refuses a directory that contains anything but `.pub` files, so a secret cannot be
   baked in by mistake.
4. **Release numbers are dated: `YYYYMMDDnn`.** For example, `2026101601` is the first release
   on 16 October 2026. They are always increasing, readable, and far above the test builds' `1`
   and `2`. The number is set when the payload is sealed and is never reused.
5. **The signing round.**
   1. CI builds and seals the payload at the release number. It does not sign.
   2. Rob copies `BLAKE3SUMS` to the offline machine and reads its `release` line.
   3. He runs `morph-update --payload <dir> --key <seed> sign` there.
   4. He copies `BLAKE3SUMS.sig` back to be published beside the payload.
   5. A `scripts/` helper for steps 2–4 is the first PR after this is accepted.

## What Rob decides

1. **Custody:** (a) now and (b) later, as recommended; or (b) now, which means buying and testing
   a device first.
2. **A backup key:** yes, as recommended, and where each of the two is kept. The places are not
   written in this public repository: say only that they are separate.
3. **Release numbers:** dated `YYYYMMDDnn` (recommended), or a plain counter starting at 100.
4. **Who may sign:** only Rob (recommended for v1), or a second named person holding the backup.

## Consequences

- No release can be published without Rob doing the signing round, which takes minutes. A builder
  can prepare everything up to the signature, and S3's server tests keep using test keys.
- **CI still builds what Rob signs.** A compromised CI could hand him a bad `root.img` with a
  correct manifest. Signing says "Rob approved these digests", not "these digests are
  harmless". Closing that gap needs reproducible builds, or Rob building on his own machine and
  comparing digests. It is a later task, and is not claimed here.
- **A stolen key that signs a huge release number** (for example, the largest `u64`) would make
  every real later release look older and be refused. That would need a reinstall to undo,
  though an attacker with a signing key can already install anything. Not solved in v1;
  recorded so it is not mistaken for solved.
- The image that people download to write a stick is a separate thing from an update payload.
  Its integrity (a published BLAKE3 digest, signed with the same key) is S3's or E8's to add.
