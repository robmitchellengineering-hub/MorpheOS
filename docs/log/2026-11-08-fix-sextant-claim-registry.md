# 2026-11-08 · DeepSeek · re-home the SEXTANT claim registry under fix/ (feat/ names no doc-20 task)

The morph-verify crate (SEXTANT claim registry + four native falsifiers — Clifford, Tsirelson,
Bell, matchgate) was on `feat/sextant-claim-registry`. `verify-plan.mjs` refuses a `feat/` branch
that names no doc-20 task, and SEXTANT is not a doc-20 task. Re-homed as
`fix/sextant-claim-registry`, matching #186's `fix/verification-discipline`.

Also fixed the two Clippy `needless_range_loop` lints in `crates/morph-verify/src/verify.rs::det4`
that were failing `rust` before the branch-name gate was reached. The Pi-image red on this branch
is an infra flake (Syft release download 504 + rpi-image-gen chroot mount noise), not this code.
