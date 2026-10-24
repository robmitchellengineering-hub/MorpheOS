# 2026-10-23 · DeepSeek · S8 config rollback — PR 1: the digest and the rollback

PR 1 of S8 (rung H2): a corrupted generation rolls back. `morph-manifest` now records a
BLAKE3 digest of each generation's stored file in its meta record at `apply`, and holds every
`load` to it — a file whose bytes changed reads as `Error::Corrupt`, while a store that predates
the digest reads as `DigestCheck::Unknown` (readable, never rolled back for that alone).
`morphd boot` judges the generation in force (parse, validate, digest) and, on failure, rolls
`current` back to the newest older generation that passes all three, applies it, and appends
`{rung:"H2", bad, reason, now, at}` to `heal.jsonl`. A store with one corrupt generation
refuses rather than applying a default. Rollback moves the pointer only (generation N is not
deleted) and the next generation remains `next_number()` (H12). Unit tests cover corrupt TOML,
a manifest that fails validation, a wrong digest, a single-generation refusal, and the
unknown-vs-corrupt distinction. PR 2 adds the `flow-notice` message and the VM scenario.
