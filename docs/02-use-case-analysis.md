# 02 · Use-Case Analysis: Working Backwards From Real Work

We design MorpheOS backwards from what people actually do at a computer, and
from where the current OS gets in their way.

## Method

1. List the high-frequency workflows for each persona.
2. Break each one into steps and mark every step that isn't the task itself
   (the friction).
3. Trace each friction to its **root cause in the OS design**, not the app.
4. Group the root causes into architecture requirements.

## Personas

| Persona | Typical hardware | Core workflows |
|---|---|---|
| **Everyday user** | Mid-range laptop, phone | Browse, email, video calls, photos, documents, streaming, printing, online banking |
| **Professional / office** | Work laptop, external monitor | Docs/sheets/slides, Teams/Zoom, line-of-business Windows apps, VPN, file shares |
| **Creative (audio/video/design)** | High-RAM laptop or desktop, audio interface, GPU | DAW, plugins (VST, often Windows/macOS-only), video editing, large media libraries, low-latency I/O |
| **Developer / nerd** | Laptop + home server/Pi | Editors, containers, multiple language toolchains, SSH, homelab, scripting |
| **Gamer** | Gaming PC/laptop | Steam/Epic games, Discord, streaming, mods |
| **Enterprise IT** | Fleets of laptops and servers | Provisioning, policy, patching, compliance, monitoring, incident recovery |
| **Maker / edge** | Pi Zero, SBCs, IoT | Headless services, sensors, kiosks, small automations |

## Friction inventory → root causes

| Workflow | Friction today | OS-level root cause | MorpheOS answer |
|---|---|---|---|
| Open a file someone sent | "No app can open this", wrong app, install hunt | Files are tied to installed apps by file extension | **Capsules** fetch or launch a suitable handler on demand ([05](05-compatibility-capsules.md)) |
| Run a Windows-only tool on Mac/Linux | Dual boot, VM setup, licences, slow | Each platform is a separate world | Automatic tiered compatibility: Wine/Proton → microVM |
| Find "that contract from last spring" | Folder spelunking, bad search | Hierarchical paths are the primary identity of a file | **Strata** semantic, temporal and relational lookup ([04](04-strata-filesystem.md)) |
| Recover an overwritten file | Usually impossible without backups | Writes overwrite in place by default | Copy-on-write with continuous versions and time-travel |
| Update broke something | Hours of troubleshooting | Updates change system state in place, without transactions | Atomic A/B system plus declarative state, rolled back in one step |
| Join a video call | Wrong mic/camera, permission prompts, CPU fans | Devices and permissions are managed per app, with no task context | The **Session** knows the task is a call, picks devices and reserves resources |
| Set up a new machine | Hours to days | State is scattered and imperative | The whole machine is one declarative **Manifest**, restored in minutes |
| Low-latency audio | Driver models, buffer tuning, xruns | A general-purpose scheduler with no idea of real-time intent | "Studio" session profile with RT scheduling, CPU isolation and a tuned audio graph ([08](08-hardware-flows.md)) |
| Battery dies fast | Background junk, poor scheduling | No global model of what the user cares about right now | Intent-aware scheduler and power governor |
| Disk full | Manual cleanup | Duplication, caches and no tiering | Deduplication, compression, automatic tiering to slower or remote storage |
| Share a file with another device | Cloud upload, cables, AirDrop-only ecosystems | No native peer-to-peer device mesh | **Constellation** device mesh ([07](07-scaling-profiles.md)) |
| IT provisions 500 laptops | Imaging tools, scripts, drift | No declarative, verifiable machine state | Fleet Manifests, attestation, drift detection |
| Install dev toolchain | Version conflicts, PATH hell | One global namespace for software | Per-project capsule environments, reproducible like Nix |
| Pi project goes flaky after months | SD card corruption, config drift | Writes in place on fragile flash | Log-structured, flash-aware Strata, read-mostly root, auto-healing |

## Patterns that drive the architecture

1. **Most friction comes from bridging between worlds** (apps ↔ files,
   platform ↔ platform, device ↔ device). → Make bridging an OS service.
2. **Most damage comes from in-place mutation.** → Make everything immutable
   or versioned by default.
3. **Most slowness is cold-start and contention, not raw compute.** → Predict,
   prewarm and prioritise by intent.
4. **Most configuration is the user translating intent into settings.** →
   Let the AI do the translation, inside a validated configuration space.
5. **Users think in tasks, projects, people and time, not paths.** → Make
   those the main ways to organise and find things.

## Top 20 everyday tasks to optimise (v1 benchmark suite)

Every release is measured on these, end to end, from intent to done:

1. Power on → browser showing the last tabs
2. Open an email attachment (PDF, DOCX, XLSX)
3. Join a video call from a calendar link
4. Find a file from a vague description
5. Recover yesterday's version of a document
6. Install and launch a common Windows `.exe` installer
7. Print a document to a new network printer
8. Connect to new Wi-Fi or a captive portal
9. Plug in an external monitor and continue working
10. Screenshot → annotate → share
11. Import photos from a phone or camera
12. Play a Steam game
13. Load a DAW project with plugins at low latency
14. Clone a repo and get its dev environment running
15. Set up a new machine from an existing one
16. Roll back a bad update
17. Send a large file to a nearby device
18. Back up everything / restore everything
19. Make the battery last through a long flight ("Endurance mode")
20. Deploy a small web service on a Pi or server
