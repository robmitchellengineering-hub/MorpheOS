# 2026-11-08 · DeepSeek · land the claim discipline at apply (rebased onto main)

Took over the idle `fix/verification-discipline` branch and landed it.

- The `Claim` type (`crates/morph-manifest/src/claim.rs`) — oracle class, falsifier, remedy,
  status — and the judge in `validate.rs` that refuses `demonstrated` without a falsifier
  (`CLAIM-NO-FALSIFIER`), warns on `stated` (`CLAIM-STATED`), and treats an empty/duplicate id
  as a tension.
- Rebased onto `main` by merging the two commits that landed after this branch forked: #187
  (doc 27 "proposed") and #190 (docs 28/29). Resolved the `docs/27` add/add conflict by keeping
  the branch's "implemented" §5 — this change lands the slice #187 described as proposed — and
  resolved the README list by keeping doc 27's richer description plus main's new 28/29 entries.
- Root cause of the earlier `rust` failure was the missing session-log file (this file).

Verified: `cargo test -p morph-manifest` and `cargo clippy --all-targets -- -D warnings` green
after the resolution.
