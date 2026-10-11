# Manifest

The Manifest is the one document that describes a MorpheOS machine. See
[ADR 0005](../docs/adr/0005-manifest-format.md) for the design, and
`crates/morph-manifest` for the implementation.

```bash
cargo run -q -p morph-manifest -- validate manifest/examples/studio-pc.toml
cargo run -q -p morph-manifest -- render   manifest/examples/seed-shed-pi.toml
cargo run -q -p morph-manifest -- plan     manifest/examples/desk-laptop.toml manifest/examples/desk-laptop-proposed.toml
cargo run -q -p morph-manifest -- apply    manifest/examples/desk-laptop.toml manifest/examples/desk-laptop-proposed.toml --to /tmp/out.json
cargo run -q -p morph-manifest -- schema
```

`plan` reports what would change and at which risk tier; **`apply` is where that tier refuses
something.** It exits **0** applied, **1** `REFUSED` (a decision — the reason is on stderr and the target is
left untouched) or **2** a defect. An invalid generation is refused for every caller, and above tier 1 it
is refused unless a person is named with `--approved-by <who>`. So the `apply` line above **refuses** —
widening a capsule's file access is tier 3. Add `--approved-by <who>` to watch it apply.

| Example | Profile | What it shows |
|---|---|---|
| `seed-shed-pi.toml` | seed | A Pi Zero 2 W replicating to a desktop over the mesh |
| `desk-laptop.toml` | desk | A work laptop with a Windows CRM in a Wine capsule |
| `desk-laptop-proposed.toml` | desk | An AI-proposed change: widened file access is held for approval |
| `studio-pc.toml` | studio | A real-time audio workstation with local and hosted replicas |

## Invariants (a Manifest that breaks any of these can't be applied)

| Code | Rule |
|---|---|
| `INV-BOOT` | A/B boot slots on (a failed trial falls back after one boot, [ADR 0008](../docs/adr/0008-one-try-trial.md)) |
| `INV-RECOVERY` | The immutable recovery image stays on |
| `INV-ADMIN` | At least one admin user |
| `INV-RECOVERY-KEY` | At least one user holds a recovery key |
| `INV-HEAL` | The heal engine is on, auto-healing stops at rung 4, and the user is asked after ≥ 1 failed rung |
| `INV-COPIES` | At least one replica, unless `accept_single_copy` is set (then it's a warning) |
| `INV-FREE-SPACE` | Free-space floor of 5–50% |
