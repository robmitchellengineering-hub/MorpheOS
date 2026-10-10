# 19 · Any Hardware: Adapting and Creating Drivers

> *"It should be able to integrate any hardware by either editing found
> drivers or creating what it needs to run it."* (Rob, 2026-10-10)

Most new operating systems die on drivers (ADR 0001). MorpheOS starts with
Linux's huge driver base. This doc covers the rest: the device nobody wrote a
driver for, the driver that *almost* works, and the one-off gadget. The OS
should find, fix or build what it needs, **without ever risking the machine.**

## 1. The ladder: cheapest working option first

When a device appears with no working driver, the **Device Doctor** walks this
ladder and stops at the first rung that works:

| Rung | What | Example |
|---|---|---|
| D0 | **Already supported**: the mainline kernel or a firmware package | Most USB, PCIe, Wi-Fi, audio, GPUs |
| D1 | **Known fix recipe**: a quirk, module parameter, firmware file or device-ID binding from the MorpheOS driver-recipe database | A USB audio interface that needs `quirk_alias`, a webcam needing a UVC quirk |
| D2 | **Adapt a nearby driver**: same chip family, different IDs or slightly different registers. Patch and rebuild the existing driver | A new revision of a supported NIC, a rebadged sensor |
| D3 | **Userspace driver**: generate a driver that runs outside the kernel, over USB (libusb), HID, serial, I²C/SPI (from userspace), Bluetooth LE or network protocols | Lab instruments, MIDI and control surfaces, label printers, Pi HATs, smart-home gadgets |
| D4 | **Kernel driver in a driver VM**: when it must be in-kernel, it runs inside an isolated microVM with the device passed through (VFIO), exposed to the host through a standard virtio interface | A PCIe capture card with no Linux driver |
| D5 | **Promote to the host kernel**: only after a long, clean record at D4, signed, and still able to roll back | A widely used community driver graduating to the main image |

Most of the value is at D1–D3. Most consumer gadgets talk over USB, HID,
serial or the network, and a userspace driver can't crash the kernel.

## 2. How a driver is created (D2–D4)

This is the Morpheus build loop (doc 17) aimed at hardware, with the same discipline:
real evidence, root-cause before patching, verify against reality.

```
identify ──► research ──► plan ──► generate/patch ──► build ──► test in a fork ──► sign ──► promote
   ▲                                                              │
   └──────────── evidence: logs, traces, descriptors ◄────────────┘
```

1. **Identify:** bus IDs (USB VID:PID, PCI IDs, ACPI/DT compatible strings),
   descriptors, class codes, firmware strings, and the chip markings if a camera
   or the user can see them.
2. **Research:** find existing drivers for the same chip family, public datasheets,
   other open-source operating systems' drivers, and protocol notes.
   *Every external claim gets checked against the real device* (the Morpheus "verify against reality" rule).
3. **Observe:** with the user's permission, capture what the vendor's software does,
   for example by running the Windows vendor tool in a Wine or VM capsule (doc 05)
   and recording the USB traffic. This is the classic, legal route for
   interoperability reverse-engineering, and it can now be largely automated.
4. **Generate or patch:** write the D3 userspace driver (Rust, against a small
   MorpheOS device SDK) or the D2/D4 kernel patch.
5. **Test in a fork:** the driver runs in a disposable capsule or driver VM
   against the real device, with automated probes (does it enumerate, read,
   write, survive unplug and replug, suspend and resume?). Failures feed back into step 4.
6. **Sign and promote:** a passing driver is signed, recorded as a
   Manifest change (risk tier 2, so it's shown to the user), and can be rolled back
   like any other change.
7. **Share (opt-in):** the *recipe* (device IDs → driver source and tests, never
   user data) goes to the community driver database, so the next person
   with that device gets it at D1 in seconds.

## 3. Safety: why this can't brick the machine

| Risk | Guard |
|---|---|
| A generated driver crashes the system | D3 runs in userspace capsules; D4 in a microVM. Neither can take down the host kernel |
| A driver corrupts data | Capsule grants: a device driver gets *that device* and nothing else (doc 09) |
| A bad driver persists | It's a Manifest change, so heal rung H2 rolls it back (doc 15) |
| A malicious driver from the community DB | Signed recipes, reproducible builds from source, and the same tests run locally before use |
| Damaging the hardware (writing firmware, over-volting) | Firmware flashing and any write to non-volatile device memory is **risk tier 4**: always explicit, and never automatic |

## 4. Graphics cards: CAD, video editing and gaming

GPUs get their own section because the stakes are highest: they're the
difference between a workstation and a paperweight for CAD, video and games,
and GPU driver updates are the most common way desktops get broken today.

### What MorpheOS uses

| Vendor | Stack | Notes |
|---|---|---|
| AMD | `amdgpu` kernel driver + Mesa (RADV Vulkan, radeonsi OpenGL) + ROCm for compute | Fully open, in mainline. The best-integrated option |
| Intel | `i915` / `xe` kernel drivers + Mesa (ANV, iris) + oneAPI/Level Zero | Fully open, in mainline. Newer iGPUs also support SR-IOV (see below) |
| NVIDIA | NVIDIA's open kernel modules + proprietary userspace (Vulkan, OpenGL, CUDA, NVENC); `nouveau`/NVK as the open fallback | Full performance and CUDA need NVIDIA's userspace, which MorpheOS installs and manages for you |

No "go find a driver" step: the Device Broker identifies the GPU and installs
the right stack as a Manifest change.

### Adapting GPU drivers: where the AI earns its keep

Almost all real-world GPU pain is **mismatch, not missing drivers**. These are
all D1/D2 fixes:

| Problem today | MorpheOS fix |
|---|---|
| A new card revision isn't recognised yet | D2: add its device ID to the driver for the same architecture, then build, test in a fork, and sign |
| NVIDIA module won't build after a kernel update (black screen on reboot) | The kernel and GPU driver are updated **together** as one trial (doc 15). If either fails, the old pair boots |
| A driver update breaks one app or game | Per-capsule driver versions: a game or CAD capsule can pin the Mesa or NVIDIA userspace version that works for it, while the rest of the system moves on |
| Wrong settings (Wayland/X11 quirks, power modes, multi-monitor, HDR, VRR) | D1 recipes from the shared database, applied as a reversible Manifest change |
| Shader stutter on first launch of a game | Shared, precompiled shader caches per GPU and driver version (mesh and community), prewarmed by flow learning (doc 18) |

Every GPU driver change is a trial: if the display doesn't come up, or the
health check fails, the next boot is the previous working driver. **A driver
update can never leave you at a black screen.**

### Workload by workload (honest status)

| Workload | Runs natively | Windows-only apps | Known blockers |
|---|---|---|---|
| **Gaming** | Native Linux games; Steam Proton (DXVK, VKD3D-Proton) runs a large share of the Windows library at close to native speed | Proton capsule, chosen automatically (rung 3, doc 05) | Games with kernel-level anti-cheat that block non-Windows systems |
| **Video editing** | DaVinci Resolve (native Linux build), Blender, Kdenlive; hardware encode and decode through NVENC, VA-API and Vulkan Video | Premiere Pro and After Effects don't run well under Wine, so they get a Windows VM with GPU passthrough | Some codec licensing limits in free editions; a Windows VM needs a Windows licence |
| **CAD / 3D** | Blender, FreeCAD, BricsCAD (Linux build), Onshape (browser) | SolidWorks, Inventor and AutoCAD are Windows-only, so they get a Windows VM with GPU passthrough | ISV-certified driver requirements; some vendors only support certified Windows setups |
| **AI / compute** | CUDA, ROCm and oneAPI natively, with PyTorch and others in capsules | (rarely needed) | Vendor support matrices per card generation |

### Running Windows-only pro apps at full GPU speed

For the apps Wine can't handle, the capsule ladder's rung 6 (doc 05) gets a real GPU:

1. **Second GPU passthrough (best).** Machines with an iGPU plus a dGPU, or two
   dGPUs, hand one GPU completely to a Windows VM through VFIO. That gives near-native
   performance for SolidWorks or Premiere, while the host keeps the other GPU.
2. **SR-IOV / vGPU (split one GPU).** Newer Intel iGPUs and NVIDIA/AMD datacentre
   and pro cards can be split into virtual GPUs, so host and VM share one card.
3. **Single-GPU handover.** The host steps aside, the VM gets the only GPU while
   it runs, and gives it back when it closes. It works on most desktops, at the cost
   of the host desktop being paused.
4. **Paravirtual GPU (virtio-gpu Venus / native context).** For lighter work
   it's no passthrough needed, and it improves every year.

The capsule planner picks the best option the hardware allows, and remembers it
per app.

## 5. Honest limits

- **Signed firmware and locked devices.** Some hardware only accepts vendor-signed
  firmware (many phone modems, some Wi-Fi chips, Apple silicon secure parts). We can
  load the vendor's firmware where its licence allows, but we can't write our own.
- **Writing a new GPU driver from scratch.** Adapting, configuring and managing
  modern GPU drivers is core MorpheOS work (section 4). Writing a full driver for an
  undocumented GPU *from nothing* is a multi-year project, not a fork test. It's
  rarely needed, because AMD, Intel and NVIDIA cover nearly every card people buy.
- **Undocumented hardware with no observable vendor software.** If there's no
  datasheet, no existing driver and no vendor tool to observe, there may be nothing
  to learn from. The Device Doctor says so plainly instead of guessing.
- **Legal.** Interoperability reverse-engineering is allowed in many jurisdictions
  but not all. Observation (step 3) is opt-in, and the community database only
  accepts clean-room recipes.

## 6. Roadmap placement

- **Phase 1:** D0 plus D1 for known quirks (a recipe file format and a small built-in set).
  GPU drivers are updated together with the kernel as one A/B trial.
- **Phase 2:** the D3 device SDK (USB, HID, serial), the Device Doctor UI, recipe
  sharing, and test-in-a-fork for userspace drivers. The managed GPU stack (AMD, Intel,
  NVIDIA), per-capsule driver pinning, Proton capsules, and shared shader caches.
- **Phase 3:** AI-generated D3 drivers through the Morpheus loop, D2 patching of
  nearby kernel drivers (including GPU device-ID and build fixes), the USB-capture
  observation workflow, and Windows pro-app VMs with GPU passthrough and SR-IOV.
- **Phase 4:** D4 driver VMs (VFIO), and the D5 promotion pipeline.

## Open questions
- Device SDK language: Rust only, or WASM components so drivers are architecture-neutral?
- How much of D2 (kernel patching) should run locally vs on a Morpheus build service?
- Should the community driver DB be part of the paid tier, or free, like the compatibility DB?
