# 26 — The fleet revenue path

**What this is.** The commercial plan for the one part of MorpheOS that can be sold before an OS ships: **verified
update, rollback and repair as a component** a device vendor embeds in their own image. It is a plan for revenue, not
for the five-pillar OS; it exists so the work that is already built has a buyer, and so future work knows which proof
it is for.

## What exists today, measured (2026-11-03)

| Piece | State | Evidence |
|---|---|---|
| A/B trial boot, commit-on-health, boot-count fallback | **proven** on x86 + systemd-boot | `images/x86-qemu/test-fallback.sh`, 3 scenarios, verdict read off the disk after poweroff; the `images` A/B job is 14/14 green |
| `morph-boot` (select, trial, commit) | working, tested | `crates/morph-boot` |
| `morph-heal` (spare-slot repair) | working, tested | `crates/morph-heal`, 802 lines |
| `morph-update` (payload, signature, seal, install) | **built, untested** | `crates/morph-update`, 872 lines: keygen, sign, seal, install; refuses an older release, an unfinished previous update, and a payload that fails its own check before the slot is touched; **no tests** |
| `morph-manifest` (typed description, risk tiers) | working, tested | `crates/morph-manifest`, 2,905 lines |
| `morphd` (apply a Manifest) | **absent** | no crate applies a manifest; ADR 0005 says only that it will |
| Pi 5 image | builds; **has never booted** | `images/pi5/README.md`: "No Raspberry Pi has ever run this image" |
| Publisher signing key | **no ceremony** | ADR 0019 status: proposed, waiting on a person |
| Installer (E7) | open | `docs/20-build-plan.md` |
| Fleet view (inventory, rollout, rollback history) | absent | — |

This is a better starting point than "an OS with an A/B idea": the hard mechanism is proven, and the update payload path
has a careful implementation. What is missing is **proof on real hardware and a way for a vendor to integrate**.

## The buyer, and what they are actually buying

An OEM, systems integrator or operator with 100–20,000 devices they cannot walk up to: digital signage, kiosks, industrial
HMIs, lab instruments, medical carts, retail terminals. Their trigger is either a bad update in memory or an update they
are now afraid to ship at all.

They do not want an operating system. They want **"the update that fails goes back to the last good state, and I can prove
it"** — inside the image they already build, with their own signing key, in their own pipeline.

| Offer | What they get | Price |
|---|---|---|
| **OEM integration** | bring-up on their board: A/B slots, trial boot, heal, their signing key, a documented integration test | **NZD 40–80k**, 4–8 weeks |
| **Per-device licence** | the component in their image, updates, support | **$2–4/device/year** |
| **Fleet console** (only after two OEMs) | inventory, staged rollout, rollback history, health | **$5–10/device/year** |
| **Support** | security updates and CVE response | 15–20% of licence |

The first invoice is the integration. The licence only becomes real after the pilot, and the console is a later product —
offering it early is how a component company turns into an unfinished platform company.

## What to build, in order, with acceptance

Each phase is a task in `docs/20-build-plan.md` terms and each has a check that can fail.

| Phase | Work | Acceptance (a check, not a claim) |
|---|---|---|
| **U1 — update is proven** | a test suite for `morph-update`: refuse older, refuse unfinished marker, refuse bad digest, install writes only the inactive slot, seal round-trips | the workspace's Rust test suite for `morph-update` has a case for each refusal and for a successful install; the three A/B VM scenarios run through `morph-update` rather than a scripted write |
| **U2 — Pi 5 boots** | `tryboot` / `autoboot.txt` against real firmware, device-tree parsing on the board | a Pi 5 boots, updates to a new release, and rolls back on a deliberately broken one — recorded on the board, not QEMU |
| **U3 — signing ceremony** | two custodian keys, offline root, signed manifests, verification at boot and before install (ADR 0019) | a tampered payload is refused by name; a key rotation leaves old releases valid |
| **U4 — `morphd`** | apply a Manifest transactionally with risk tiers; a refused apply is a no-op | applying a bad manifest changes nothing; applying a good one changes exactly what it names |
| **U5 — pilot** | one vendor, 10–20 devices, staged rollout, a rollback exercised on purpose | one real device updated and rolled back, with logs a buyer can read |
| **U6 — OEM pack** | board bring-up guide, integration test suite, support terms | a vendor engineer integrates without a call |

E7 (installer) is a prerequisite for U5, not for U1–U4.

## What we will not do

- **Sell the OS.** The five pillars stay a research programme until a component pays for them.
- **Patent the A/B mechanism.** It is prior art (systemd-boot, ChromeOS, Android, RAUC, Mender). Sell integration and
  support, not exclusivity.
- **Offer an SLA before a second maintainer exists.**
- **Ship a kernel whose source offer is not ready.** Today the kernel recipe lives on a side branch
  (`secure/kernel`), so a distribution built from `main` cannot satisfy GPL-2.0. U5 depends on moving it to `main`.

## Why a licence, and why per device

Per-device pricing is how this market already buys (Mender, balena, Ubuntu Core). It aligns the price with the thing the
vendor is protecting — headcount in the field — and it survives their procurement because it is a small line item against
a site visit. Per-fleet pricing invites a budget conversation the vendor's finance team wins.

## Kill criteria

- No vendor pilot by day 120 → keep the component for MorpheOS's own devices and stop treating it as a product.
- The first pilot cannot beat the vendor's existing updater on **rollback time or operator effort** → the differentiation
  is not real; say so and stop.

## What this changes about the current plan

The build plan's ordering was about a bootable OS for a person. This document does not replace it; it adds one rule: **any
work on `morph-update`, `morph-heal`, `morph-boot` or signing is revenue-critical, and gets tests before features.** The
rest of the OS remains a research programme and should be described that way to anyone outside the repository.
