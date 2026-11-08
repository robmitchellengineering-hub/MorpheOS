> Persisted from the planning room 2026-11-08 so the plan is git-tracked, not lost.

# PLAN — MorpheOS owns its kernel: what that unlocks, and in what order

Status:      proposal for Rob's decision
Decision it needs: whether Phase 0 starts now (≈1 week) or waits until 1.1 is on the stick
Context:     D12 (claim → falsifier → evidence), D17 (two editions), the night of 2026-10-30/31
Verified:    MorpheOS @ `0fb7855` (image), machine `MacBookPro9,1` on the robmesh, `/root`
             ownership defect proved and fixed, boot-to-desktop on real hardware

---

## Why the kernel at all

Everything we want to say about MorpheOS — "never breaks, heals itself", "an app cannot exfiltrate",
"the machine explains itself" — is today **enforced by userland**. That means it is enforced by
cooperative processes, on a kernel somebody else builds, with a driver set we borrow and cannot patch.
Owning the kernel moves those from *policy we ask processes to respect* to *properties the machine cannot
avoid*. That is the whole argument, and it is worth being precise about which claims it converts.

## What owning the kernel buys, by domain

| domain | today | with our kernel | why only the kernel can do it |
|---|---|---|---|
| **Identity** | `os-release` says MorpheOS; `uname -s` still says Linux | `uname -s` = **MorpheOS**, `uname -r` = `6.8.0-morpheos` | the UTS sysname is a kernel constant; a wrapper is bypassed by the syscall |
| **Self-healing** | A/B slots + `morph-heal` reinstall, driven by userspace | **kexec** into the known-good slot in seconds; **pstore/ramoops** flight recorder that survives a hard lockup; watchdog that is the kernel's, not a daemon's | a userspace supervisor cannot report on a kernel that died, and cannot switch kernels |
| **Rollback** | new slot boots, a userspace health check decides | the kernel marks its own slot healthy; a failed boot never reaches userspace at all | the decision must survive userspace being broken — that is the failure case |
| **Security** | seccomp/landlock profiles applied by systemd units | **eBPF LSM policy per service**, signed modules only, no module loading from disk, `IMA` + TPM measured boot so the machine can *prove* what it booted | an LSM is kernel-side by definition; trust anchored in userspace is not trust |
| **App permissions** | an app is a process with a sandbox we hope it respects | an app is a **kernel-enforced object**: its namespaces, cgroup, network and filesystem view are a policy the kernel applies, and a denied action is unauditable by the app | userspace sandboxes can be bypassed by the platform they run on |
| **Data ownership** | `strata` versions files in `/home` | versioning at the **filesystem/VFS** boundary: every write is a generation, unprivileged, unbypassable | a library can be skipped; a mount cannot |
| **Observability** | `journalctl -p err`, our `ssh.txt`, phone-home | our own **tracepoints + a ring buffer reserved in memory**, so the machine can say what happened even after a crash — the exact thing that cost us five image builds | `journalctl` dies with the userspace that writes it |
| **Latency** | PipeWire under systemd on a generic kernel | `PREEMPT_RT`, `drm lease` so the compositor owns the GPU directly, no initramfs, kernel-boot to compositor in about a second | scheduling class, lease and boot path are kernel decisions |
| **Attack surface** | Ubuntu's kernel with every subsystem | a config with only what the machine uses: no unused filesystems, filesystems, protocols, drivers | you cannot remove a subsystem you do not build |
| **Update model** | whole image, reboot | kernel **livepatch** for security fixes; `kexec` for instant switch; modules signed by us | patching a running kernel is a kernel facility |
| **Hardware** | we borrow Ubuntu's driver set and cannot fix a quirk | we carry the patch: `i915` on a 2012 panel, `b43` quirks, the NVIDIA card that has no driver today | a driver bug in someone else's kernel is a bug we cannot fix |
| **Boot chain** | UEFI → systemd-boot → Ubuntu kernel → systemd → our compositor | UEFI → **our stub** → our kernel (drivers built in) → our compositor; no initramfs, no systemd | the pieces in between are ours to delete |

## What it does *not* buy

- **Speed for its own sake.** A custom kernel is not faster because it is ours. It is faster where we
  remove work (no initramfs, no systemd, `drm lease`, RT scheduling), and those are measurable.
- **Firefox, PipeWire and drivers.** We keep the Linux ABI, so the ecosystem stays; that also means we
  inherit the driver maintenance, which is the real cost.
- **A free pass on the security claims.** An LSM we wrote is only a claim until the attack suite hits it
  and fails to get through.

## Phases, with effort and what each one proves

| phase | work | rough effort | it proves |
|---|---|---|---|
| **0. Own the build** | our kconfig fragment (b43, i915, snd-hda-intel, xhci, ext4, EFI stub), `UTS_SYSNAME="MorpheOS"`, `LOCALVERSION=-morpheos`, KSPP hardening, module signing; boot it in the QEMU matrix and on the machine | **≈1 week** | `uname -s` says MorpheOS, and our image boots our kernel on the 2012 laptop |
| **1. Own the failure path** | kexec rollback, pstore flight recorder, slot-health decided in the kernel, boot-to-compositor without initramfs | 3–5 weeks | the machine heals without userspace, and can explain a crash it did not survive |
| **2. Own the policy** | eBPF LSM per service, IMA + TPM measured boot, verity on the root, no module loading | 6–10 weeks | an attack that the suite runs is *denied by the kernel*, and the machine can attest what it booted |
| **3. Own the model** | remove systemd, kernel-supervised services, the app object, VFS-level versioning, livepatch | 3–6 months | "an app is a permission", not "an app is a process in a namespace" |

Phase 0 is the decision on the table. Phases 1–2 are where the Secure edition's certificates come from.
Phase 3 is the reinvention and should not start before 1 and 2 are boring.

## Risks, stated plainly

1. **Drivers.** Wi-Fi (`b43`), GPU (`i915`), suspend/resume and audio on the 2012 laptop are the
   regressions nobody tests for us. Mitigation: keep the Ubuntu kernel as the other A/B slot for the
   first release and compare boot-by-boot.
2. **CVE cadence.** Owning the kernel means owning the patch stream. Mitigation: track `stable` and
   rebuild weekly in CI; livepatch later.
3. **Ecosystem expectations.** Anything that reads `/sys` or expects a distro kernel will notice. That is
   the point, but it is also a compatibility surface.

## Falsifiers

- Phase 0 failed if the machine boots our kernel but loses Wi-Fi, audio or the panel.
- Phase 1 failed if a hard lockup leaves no evidence in the flight recorder.
- Phase 2 failed if the attack suite's A5/A6 can still succeed against a machine running our kernel.
