# 15 · Self-Healing: The Unbreakable Computer

This is the **flagship** feature (decision D2 in [00](00-strategy.md)), and it gets
designed first and deepest.

## The promise

> **Whatever happens (a bad update, a bad driver, malware, a deleted system
> folder, a full disk, a dead SSD, a lost laptop), you are back to work within
> minutes, with nothing lost and nothing to configure.**

## What "broken" means: the failure taxonomy

| Class | Examples | Today's typical outcome |
|---|---|---|
| **Software state** | Bad update, broken config, driver conflict, dependency mismatch | Hours of troubleshooting or a reinstall |
| **App failure** | Crash loops, corrupted app data, an app update regression | Reinstall, data loss |
| **User mistakes** | Deleted or overwritten files, wrong setting, "I clicked something" | Usually permanent |
| **Malicious** | Ransomware, persistence, tampering | Wipe and restore, if a backup exists |
| **Resource exhaustion** | Disk full, memory leaks, thermal throttling | Slowdowns, crashes |
| **Hardware degradation** | A failing SSD, worn battery, bad RAM, failing fan | A sudden loss of everything |
| **Total loss** | Stolen, dropped or dead machine | Days to rebuild, with partial data |

## Architecture: the Heal Engine

Self-healing is a **closed control loop**, the same pattern as Morpheus's
build → diagnose → fix → verify cycle (see [17](17-morpheus-family.md)), applied to a
running machine:

```
   ┌─────────► DETECT ──────► DIAGNOSE ──────► REPAIR ──────► VERIFY ─────┐
   │        health signals    playbooks +     cheapest       health      │
   │        & invariants      AI hypotheses   rung first     checks      │
   │                                                              │      │
   └──────────────────────────── LEARN ◄──────────────────────────┘      │
                         (fleet recipe, never user data)                 │
                                                       escalate ◄────────┘
                                         (next rung, or ask the user with a clear action)
```

### Detect
- **Invariants** from doc 12 §9 are checked continuously: bootable, network, display, input,
  admin path, free space above the floor, and recent backup present.
- **Health probes** for every service and capsule: liveness, readiness and latency budget.
- **Hardware telemetry:** NVMe/SMART health logs, battery cycle and capacity,
  thermals and fan RPM, ECC/EDAC memory errors, and firmware status (fwupd/LVFS).
- **Anomaly signals:** crash loops, error-rate spikes, unexpected writes to protected
  data (a ransomware tell), and boot-time regressions.
- **The user's own signal:** "this is broken" or "this is slow" is a first-class input.

### Diagnose
- Most diagnosis is **deterministic playbooks**: typed decision trees for known
  failure classes such as Wi-Fi, audio, display, printing, boot and disk.
- **Change correlation:** "this started after transaction #4182". Because every
  change is a transaction, the prime suspect is usually one lookup away.
- **Bisect across forks:** replay the failure in multiverse forks with
  transactions reverted one at a time, to find the exact cause (doc 12 §1, §10).
- **AI hypotheses** come in only when the playbooks run out, and they *propose*
  tests and fixes. The Verifier still decides.

### Repair: the healing ladder (cheapest rung first)

| Rung | Action | Typical time | Data loss |
|---|---|---|---|
| H0 | Restart the process or service | < 1 s | None |
| H1 | Rebuild the capsule from its recipe (app data kept in Strata) | seconds | None |
| H2 | Roll back the offending transaction (one config change, driver or app update) | seconds | None |
| H3 | Roll back to the last known-good system generation | < 30 s (reboot) | None (user data is separate) |
| H4 | **Rebuild in place** from the immutable recovery image: reflash system, keep Strata | ~2–5 min | None |
| H5 | **Phoenix restore**: a new or wiped device is rebuilt from a mesh peer or backup | ~5–15 min, with data streaming in as it's needed | Back to the last sync point (usually seconds) |

Data-level healing works alongside the ladder:
- **File rewind:** any file or folder back to any version (Strata CoW).
- **Ransomware rewind:** detect mass encryption, freeze the capsule doing it, and rewind the
  affected objects to the moment before it started.
- **Bit-rot repair:** a hash mismatch is fixed automatically from a mirror or mesh peer copy.

### Verify
- Every repair re-runs the invariants and health probes. If it fails, it escalates one rung.
- After 2 failed rungs, it **asks the user, with a single clear action** and a
  deep link to the exact step ("Morpheus rule: no dead ends").

### Learn
- Successful repairs become **recipes**: symptom fingerprint → fix. They're shared
  across the fleet or community opt-in, with **no user data**.
- The second machine that hits a known problem heals in seconds, without diagnosis.

## Making it impossible to brick

| Layer | Mechanism |
|---|---|
| Firmware/boot | Secure Boot plus a UKI, A/B boot entries, and boot counting: 3 failed boots trigger an automatic fallback |
| System image | Read-only and verified (dm-verity). The system can't be "partially" modified |
| Recovery | A separate, **immutable recovery image** that can rebuild everything else, including over the mesh or network |
| Config | Manifest generations, each checked against invariants before it's applied |
| Data | CoW versions, continuous encrypted replication to at least one other place (mesh peer, USB disk or hosted) |
| Last resort | A "Phoenix key": a USB stick, or a phone running Morph Link, that can restore a machine from bare metal |

## Backup is part of healing, not a separate app

- **Default 3-2-1, with zero setup:** the live copy, a local snapshot history, and one
  other place. That's any other MorpheOS device on the mesh, a plugged-in disk,
  or the hosted service (paid).
- End-to-end encrypted. Keys are held by the user and escrowed only if they choose
  (for example, a printed recovery code).
- The OS shows a **resilience status** in plain language: *"Fully protected: last copy
  2 minutes ago on 'Studio PC'"* or *"At risk: only one copy. Plug in a drive or add
  a device."*

## Predictive healing (hardware)

| Signal | Prediction | Action |
|---|---|---|
| NVMe spare blocks dropping, media errors rising | SSD failure in weeks | Bring the replica up to date, warn the user, suggest a replacement, pre-stage Phoenix |
| Battery capacity < 70%, or swelling indicators | Battery end of life | Gentler charge limits, warn the user |
| Corrected ECC errors rising | RAM failure | Retire pages, warn the user |
| Fan RPM falling with temperature rising | Fan or thermal paste failure | Cap performance, warn the user |
| SD card write latency rising (Pi) | SD wear-out | Move writes to RAM/log, sync to a peer, warn the user |

## Metrics we publish

| Metric | Target |
|---|---|
| Mean time to recovery, software faults | < 60 s, with no user action |
| Mean time to recovery, dead or stolen device → working on a new one | < 15 min |
| Data lost in any recoverable scenario | 0 (bounded by the sync interval) |
| User troubleshooting time per year | → 0 |
| Share of faults healed without asking the user | > 90% |

## Phase-1 demo: "Try to Break It"

A live, public challenge on a stock MorpheOS laptop and a Pi:

1. `rm -rf` the entire system root → automatic **H4**, back in about 3 minutes, with all files intact.
2. Install a deliberately broken driver → boot counting brings it back on **H3**.
3. Run a ransomware simulator on Documents → it's detected, frozen and rewound (**data heal**).
4. Fill the disk to 100% → caches are evicted and old versions tiered out, with no crash.
5. Pull the SSD → put in a blank one → **Phoenix** restores from the Pi on the mesh.
6. Corrupt a config by hand → H2 rolls it back, and the user is shown which transaction caused it.

This demo **is the phase-1 acceptance test.**

## Open questions
- Ransomware detection heuristics: entropy plus write rate plus file type, and how
  to avoid false positives on legitimate encryption and compression.
- How much of the recovery image can be shared on a Pi with a 16 GB SD card?
- Phoenix over the internet through a peer we can't reach directly: is a relay required (paid tier) or a
  self-hosted relay (free)?
