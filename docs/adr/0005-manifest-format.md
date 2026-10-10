# 0005 · Manifest format

**Status:** Accepted, 2026-10-10

## Context
The whole machine is described by one declarative Manifest (doc 03). People, the AI
and updates all propose changes to it, so it must be easy to write, strictly typed,
diffable, and safe by construction.

Options considered: Nickel, CUE, KCL, Nix, a custom DSL, plain TOML/JSON.

## Decision
- **Authoring format: TOML.** Readable, comment-friendly, already familiar from Cargo.
  A richer authoring language (likely Nickel, which is Rust-native) can be added later
  as a front end that *outputs* the same data. It is not needed for v0.
- **Layering:** `base` → profile (`seed`/`desk`/`studio`/`fleet`) → machine file.
  Tables merge, lists of named items merge by `name`, and everything else replaces.
- **Strict model:** Rust types with `deny_unknown_fields`. A JSON Schema is generated from
  the same types for editors and AI planners.
- **Invariants are code**, not prompts: A/B boot, the recovery image, an admin and a recovery key,
  heal enabled and capped below Phoenix, at least two copies of data, and a free-space floor.
  A Manifest that breaks one can't be applied, whoever proposed it.
- **Plans:** every change between generations is classified into the doc 06 risk tiers.
  Only a valid plan at tier ≤ 1 may auto-apply.

## Consequences
- Implemented in `crates/morph-manifest`. `morphd` will consume it.
- Secrets (Wi-Fi passwords, keys) never live in the Manifest. Only references do.
- An SMT-based checker (doc 12 §9) can later be added beside the invariant code.
