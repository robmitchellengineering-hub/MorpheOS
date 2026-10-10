# Architecture Decision Records

One file per decision: context, decision, consequences. A record is never edited
after it's accepted. A later ADR supersedes it instead.

| # | Decision | Status |
|---|---|---|
| [0001](0001-linux-kernel-first.md) | Use the Linux kernel; innovate above it | Accepted |
| [0002](0002-rust-core.md) | Rust for all core services | Accepted |
| [0003](0003-unbreakable-wedge.md) | "Never breaks, self-heals" is the first thing we're famous for | Accepted |
| [0004](0004-open-core-apache2.md) | Open core: Apache-2.0 userland, paid scale and hosted services | Accepted |
| [0005](0005-manifest-format.md) | Manifest: TOML files, layered profiles, invariants, risk-tiered plans | Accepted |
| [0006](0006-image-factory.md) | Image factory: one A/B trial-and-commit model; rpi-image-gen for Pi, mmdebstrap + systemd-boot for x86 | Accepted |
| [0007](0007-strata-v0-watcher.md) | Strata v0: inotify watcher plus content-addressed store, not FUSE; rewind never destroys | Accepted |
| [0008](0008-one-try-trial.md) | A trial boot gets exactly one try; no boot counter | Accepted |
