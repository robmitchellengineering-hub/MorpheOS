# 00 · Strategy & Decisions

This doc records the strategic decisions that shape everything else. Where
another doc disagrees with this one, this one wins.

## What MorpheOS has to be

> **The last OS you'll need to think about.** Every device you own acts as one
> computer. Anything runs on it. Nothing breaks, and when something does go wrong,
> it fixes itself. The laptop, desktop or phone becomes just a tool that gets
> the job done and stays out of the way.

The goal is that "Mac vs PC vs Linux vs Android" stops being a question people ask.

## Decisions (2026-10-10)

| # | Decision | Choice | Consequence |
|---|---|---|---|
| D1 | Product shape | **The OS first, then Morph Link** | The bootable OS is built first. Morph Link (the agent for Windows, macOS, Android and iOS) follows once the OS core is proven, and reuses the same mesh and healing code |
| D2 | The wedge (what we're famous for first) | **Never breaks, and heals itself** | Every phase-1 feature must serve "unbreakable". Compatibility, AI and UX polish come after that |
| D3 | Business model | **Open core** | The OS core is open source. Paid tiers cover fleet, hosted services and the Morpheus-powered features (see below) |
| D4 | Family | **Part of the Morpheus family** | MorpheOS and Morpheus share patterns, a build pipeline and eventually an assistant. See [17](17-morpheus-family.md) |

## Why "never breaks" is the right wedge

- **It's what everyone feels.** Every Windows, Mac and Linux user has lost a day to a
  broken update, a bad driver, malware or a dead disk. Nobody has solved this end to end.
- **It's demonstrable.** "Delete the system folder live on stage, and it heals in
  30 seconds" is a demo nobody else can give ([15](15-self-healing.md)).
- **It's the foundation for everything else.** The AI setup, the Windows capsules and the
  experiments (doc 12) are only safe *because* nothing can break.
- **It fits the market.** Pi and edge devices, schools, small businesses,
  and IT fleets all pay real money for "it just keeps working".

## Product family

```
                         ┌──────────────────────────────┐
                         │   Morpheus (builder / AI dev)│  proprietary, morpheus.nz
                         │   builds apps, capsules,     │
                         │   images, widgets            │
                         └──────────────┬───────────────┘
                                        │ builds for / deploys to
┌───────────────────────────────────────▼──────────────────────────────────┐
│                       MorpheOS core (open source)                       │
│ morphd · Strata · Capsules · Heal engine · Mesh · Flow shell · CLI      │
├──────────────────────────────┬───────────────────────────────────────────┤
│  MorpheOS Seed/Desk/Studio   │  Morph Link (phase 3)                     │
│  (bootable OS)               │  agent for Windows, macOS, Android, iOS   │
└──────────────────────────────┴───────────────────────────────────────────┘
                                        │
                         ┌──────────────▼───────────────┐
                         │  MorpheOS Fleet + Services   │  paid
                         │  console, hosted relay,      │
                         │  encrypted backup, premium AI│
                         └──────────────────────────────┘
```

## Open-core boundary (draft)

The test for where the line goes: **anything needed to make one person's own
devices unbreakable is free and open.** Paid features cover scale,
hosted infrastructure and convenience.

| Open source (free) | Paid |
|---|---|
| The whole bootable OS, every profile | Fleet console: policy, attestation dashboards, update rings for more than N devices |
| Heal engine, rollback, Phoenix restore to your own devices or disks | Hosted encrypted off-site backup and Phoenix restore from the cloud |
| Mesh between your own devices on LAN, or relaying through your own server | Hosted mesh relay and NAT traversal |
| Capsules, the compatibility runtime, local AI | Premium cloud AI tier and the compatibility DB service level agreement (SLA) |
| Manifest spec, capsule format | Morpheus integration: build capsules, verbs and images from chat |
| | Commercial support and OEM licensing |

Licences (**confirmed 2026-10-10**): Apache-2.0 for the userland crates (patent grant,
enterprise-friendly). The kernel stays GPL-2.0. Paid services are closed source. The
free/paid boundary above is confirmed. Morpheus is proprietary, and code may be brought
across because the same owner holds both ([17](17-morpheus-family.md)).

## Honest limits (published, not hidden)

"Everything just works" is the goal, but these are walls we don't control:

| Limit | Reason | Our position |
|---|---|---|
| Kernel-level anti-cheat games (some major online shooters) | Vendors block anything that isn't Windows | Flagged clearly; microVM with real Windows is the fallback where vendors allow it |
| DRM streaming quality (Netflix and others) | Hardware-backed DRM certification is controlled by the providers | Works at reduced quality; a certification path is a phase-4 business task |
| macOS apps | Apple's licence allows macOS only on Apple hardware | macOS *files* work everywhere, macOS *apps* only in a VM on Macs |
| Replacing iOS | iPhones can't run another OS | Morph Link app on iOS (phase 3) |
| Replacing Android on most phones | Locked bootloaders, modem and carrier certification | Morph Link on Android, with full MorpheOS on selected unlocked devices later |
| Hardware that fails physically | Physics | We predict failures early and make *recovering* from them a 5-minute job |

## Revised roadmap (summary)

The full roadmap is in [10](10-roadmap.md).

1. **Phase 1, "Unbreakable":** an immutable bootable OS on a Pi and an x86 laptop, with
   transactional state, versioned storage, the heal engine and Phoenix restore. The
   demo: *try to break it.*
2. **Phase 2, "Daily driver":** capsules (Wine, Android, translation), Flow shell,
   AI setup and diagnosis, the mesh between MorpheOS devices.
3. **Phase 3, "Everything talks":** Morph Link on Windows, macOS, Android and iOS,
   interoperability protocols ([16](16-interop-and-link.md)), and paid Fleet.
4. **Phase 4, "Everywhere":** phones on unlocked hardware, the Morph kernel research track, and
   hardware partners.
