# Manifest

The Manifest is the one document that describes a MorpheOS machine. See
[ADR 0005](../docs/adr/0005-manifest-format.md) for the design, and
`crates/morph-manifest` for the implementation.

```bash
cargo run -q -p morph-manifest -- validate manifest/examples/studio-pc.toml
cargo run -q -p morph-manifest -- render   manifest/examples/seed-shed-pi.toml
cargo run -q -p morph-manifest -- plan     manifest/examples/desk-laptop.toml manifest/examples/desk-laptop-proposed.toml
cargo run -q -p morph-manifest -- schema
```

| Example | Profile | What it shows |
|---|---|---|
| `seed-shed-pi.toml` | seed | A Pi Zero 2 W replicating to a desktop over the mesh |
| `desk-laptop.toml` | desk | A work laptop with a Windows CRM in a Wine capsule |
| `desk-laptop-proposed.toml` | desk | An AI-proposed change: widened file access is held for approval |
| `studio-pc.toml` | studio | A real-time audio workstation with local and hosted replicas |

## Invariants (a Manifest that breaks any of these can't be applied)

| Code | Rule |
|---|---|
| `INV-BOOT` | A/B boot slots on, 1–5 boot attempts before fallback |
| `INV-RECOVERY` | The immutable recovery image stays on |
| `INV-ADMIN` | At least one admin user |
| `INV-RECOVERY-KEY` | At least one user holds a recovery key |
| `INV-HEAL` | The heal engine is on, auto-healing stops at rung 4, and the user is asked after ≥ 1 failed rung |
| `INV-COPIES` | At least one replica, unless `accept_single_copy` is set (then it's a warning) |
| `INV-FREE-SPACE` | Free-space floor of 5–50% |
