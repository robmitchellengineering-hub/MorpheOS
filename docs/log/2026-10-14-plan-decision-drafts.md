# 2026-10-14 · Claude Code · Rob's decisions, drafted as ADRs he only has to answer

Three decisions block work and are Rob's. Each is now a **Proposed** ADR. Each ends with
"What Rob decides": numbered questions with a recommendation, so one reply settles it.

- **0018, backups (P1).** Strata's store replicated, with each object encrypted with `age`.
  The machine holds only the public half.
- **0019, the release signing key (S2, S3).** Rob signs offline, which works today because only
  the four-line `BLAKE3SUMS` is signed. Every release image carries a primary and a backup key.
  Release numbers are dated (`YYYYMMDDnn`). Rotation already follows from the trusted keys
  living in the signed root. Two limits are written down, not hidden: CI still builds what
  Rob signs, and a stolen key can sign a huge release number.
- **0020, the account model (S5).** One owner, with a password set at first boot, in
  `libnss-extrausers` on `/persist`. Root is locked.

The ADR index says what Proposed means: such a record may be edited until Rob answers it,
and never after. Doc 20 (P1, S2, S5) and HANDOFF's decision list now link the drafts.

Also fixed: three plan lines that #67 made stale. They said `/users`,
`storage.min_free_percent` and `/storage/replicas` were missing from `unenforced_settings()`,
and #67 lists all three.
