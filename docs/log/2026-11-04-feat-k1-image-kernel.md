# 2026-11-04 · DeepSeek · K1: the arm's code on main, and the image next

The `secure` arm (62 commits, 12 behind) lands on `main` as its own thing, on Rob's instruction:
*"put it on main as its own thing separate from the everyday use morpheos"*, then *"do the work and
flash on vms"*. The plan that makes it legal is #180 (doc 26, ADRs 0020/0022, a gate that reads the
arm's task list); this branch is the code.

## What the merge needed

Three conflicts, all of the "both sides added something here" kind, none of them a disagreement:

- `images/x86-qemu/build.sh`, two hunks. **Both kept**: the DEV_PKGS toolchain check (main) and the
  `morph-attest` / `morph-session` installs (arm) are additive — the image a person boots should
  carry both the compiler and the commands that can state and seal what it is. The second hunk was
  main-only (the phone-home and the sshd key path), so it was kept and the empty side dropped.
- `images/x86-qemu/run-vm.sh`: the arm's `VM_SSH` explanation, kept — the flag existed on another
  branch first and that is exactly the surprise worth a comment.
- `docs/ATTESTATION.md` (add/add): main's version, which points at doc 26 and ADR 0022 instead of
  at a planning file that is not in this repository.

## A defect the merge found, which is the point of landing an arm

`verify-docs.mjs` was **red on the merge**: `crates/morph-attest` and `crates/morph-capsule` existed
but the README's crate table did not name them. The arm had not run its checks since 2026-11-01, and
nothing would have noticed until someone looked. The table now names both, with what each proves —
and the attest crate's honest limit (signed claim, not proof of integrity) is in its row, not only in
its module doc.

## What is true after this lands

- the kernel recipe (`secure/kernel/`), its three-arch gate, the attack suite (`security/attacks/`),
  and the seal / attestation / measured-boot records are on `main` as the arm's own tree;
- `morph-attest`, `morph-capsule`, `morph-session` and the seal's mandatory path are in the
  workspace, so `cargo test` covers them;
- **the image still ships Ubuntu's kernel.** K1's open half is that boot: an image whose slots carry
  our kernel and modules, booted by `test-shell.sh`, `test-wifi.sh` and `test-audio.sh` in a VM, with
  the drivers intact. That is the next piece of work, and it is VM-only until `morpheos-a-1` is free.

## The image now carries our kernel — the code, not yet the evidence

`images/x86-qemu/build.sh` gains `KERNEL=ours` (+`KERNEL_OUT=<secure/kernel/build.sh out/>`):

- **slot A gets our kernel and our modules**, an initrd generated from them by the image's own
  `update-initramfs`, and the ESP's slot-A entry is asserted to carry our release string — not
  `ls | sort | tail -1`, which with two kernels in one root is a coin toss between ours and Ubuntu's.
- **slot B keeps Ubuntu's kernel** on purpose: Phase 0's comparison is ours against theirs on the same
  machine, and a kernel we cannot fall back from is not shippable.
- **the default path is untouched.** `KERNEL=ubuntu` builds exactly what it built before, so the
  everyday image cannot become red because the arm exists.

`.github/workflows/kernel.yml` gains `image-carries-our-kernel`: build the kernel, build the image with
it, boot it, and fail unless the boot log carries `6.8.0-morpheos`. That job is the difference between
"a kernel builds" and "the machine runs it", and it has not run yet — nothing is claimed until it does.

## Not claimed

- the job above has not run. Until it is green, the image carrying our kernel is code, not evidence;
- the hardware falsifier is untouched — nothing boots our kernel on real hardware, so Phase 0 is not
  passed. `morpheos-a-1` takes that boot when Rob says it is free.

- DSH on K1: opened as `open · parallel` — its build is done and gated on the arm; the image running
  it is the remaining work, 2026-11-04.

## Round 1: the things that would have made the claim false

CI was red on `ci` (`cargo fmt --all --check`, **102 diffs** — the arm's crates were never rustfmt'd; the
toolchain is pinned at 1.99.0 and this machine has exactly that, so the diff is the one CI would produce).
Formatted, and `--check` is clean.

Then three defects in the claim itself, found by asking "would this actually work", none of them visible from
a green build:

1. **The driver assertion was about somebody else's kernel.** The HARDWARE=1 images assert that a Wi-Fi,
   graphics, storage and input driver is present, by searching `usr/lib/modules` — and Ubuntu's modules are
   still installed for slot B, so a kernel with no Wi-Fi driver passed the check on Ubuntu's. Scoped to the
   module tree of the kernel under test when slot A carries ours.
2. **Our config did not carry the drivers that assertion is about.** It had b43/i915/snd — the 2012 laptop —
   and nothing for amdgpu, xe, iwlwifi, mt7921, brcmfmac, r8169, hdmi/console, SATA or USB UAS. The image
   claims those for every laptop it is flashed on, so a kernel in slot A must carry them or the image boots
   and cannot get online. Added, and the set is now the image's own list.
3. **`CONFIG_MODULE_SIG_FORCE=y` and no module signing.** The hardening we want means an *unsigned* module
   will not load: Wi-Fi and audio would have been dead on a real machine while the build was green, and it
   would have read at first boot as "no WiFi", not as a build defect. `build.sh` now makes a per-build key,
   embeds it as `CONFIG_SYSTEM_TRUSTED_KEYS`, signs every `.ko`, proves one signature is attached, and prints
   the fingerprint in the report. Per-build on purpose: build B must not load build A's modules.

And one guard so the fragment cannot lie: after `olddefconfig`, **every decision in the fragment is looked
for in `.config`** and the build fails naming the ones that did not apply (`olddefconfig` resolves a
dependency by quietly dropping the symbol that asked for it, which is how a kernel ships without the driver
its own image asserts).

Not claimed: none of this has been proven by a boot yet. The `image-carries-our-kernel` job is the evidence,
and it is the next thing to read.

## Round 1b: what the guard found, and the machine reporting its own name

The applied-config guard failed the kernel job on **nine decisions that were never applied** — and each was a
real one, checked against the 6.8 tree's own Kconfig files, not from memory:

| what we asked for | what the tree says |
|---|---|
| `PSTORE_EFI_VARS`, `PSTORE_DEFLATE_COMPRESS` | no such symbols (`EFI_VARS_PSTORE`, `PSTORE_COMPRESS`) |
| `EFI_VARS` | removed from the kernel; `EFIVAR_FS` is the mount |
| `XHCI_HCD` | not a symbol — `USB_XHCI_HCD`, so **no USB 3 host controller at all** |
| `B43_SSB`, `CPU_V6` | hidden bools (no prompt), selected by Kconfig; a fragment cannot set them |
| `MMC_SDHOST` | a menu title, not a symbol |
| `PAGE_SIZE_4KB` (arm64) | arm64 has `ARM64_4K_PAGES` |
| `BPF_LSM` | depends on `BPF_EVENTS`, so **the LSM the arm exists for was off** |
| `FRAMEBUFFER_CONSOLE` | needs `FB_CORE`, which only `FB` selects — **no console on the panel** |

Every one of those would have shipped in a green build. That is the guard paying for itself on its first run.

Also, the claim got sharper: the image's boot log proves our *release string* reached the kernel, but not what
the running system calls itself. A test image with `KERNEL=ours` now installs one one-shot unit that prints
`MORPHEOS-KERNEL uname-s: …` to the console at boot, and the job fails unless it reads `MorpheOS` there. The
name is now read off the running machine instead of a build artefact — and the unit exists only on those test
images, never on one a person flashes.

## Not claimed

- the kernel jobs have not passed yet. The fragment corrections are in; arm and arm64 still fail the config
  step and the reason is not in the logs until the run completes (a run-level log only appears at the end);
- nothing has booted our kernel in an image yet, and nothing has touched hardware.

## Round 2: the VM proof is the suite's own scenario, and there is no audio scenario to run

The image job no longer boots the disk and greps a banner: it runs **`test-shell.sh`** — the suite's own
driver, which boots a real display and asserts a window is on the screen with a keyboard and a pointer — with
`KERNEL=ours`. Two questions are judged separately, because a harness failure must not hide a kernel failure:

1. **did our kernel boot and call itself MorpheOS** — `6.8.0-morpheos` in the log, and
   `MORPHEOS-KERNEL uname-s: MorpheOS` printed by the running system;
2. **did the shell scenario pass** — the driver's own verdict, reported on its own line.

**Measured, and it changes what the goal can claim: there is no audio scenario.** The suite has thirteen
`test-*.sh` and none of them exercises a sound card; `grep -il audio images/x86-qemu/*.sh` matches
`build.sh` alone. The only audio check in this repository is the build's driver assertion
(`snd-hda-intel`, `snd-hda-codec-analog`, `snd-usb-audio` present in the kernel's module tree), which is a
test that the driver was built — not that it works. "test-audio passes with our kernel" cannot be claimed
until such a scenario exists, and inventing one now would be a second harness with its own bugs. The honest
VM claim is shell + Wi-Fi once the second leg is added, and audio on real hardware through the hardware
report.

## Round 3: the race is proven fixed, not assumed fixed

`ci` failed on the fixture race and the fix was plausible — which is not the same as proven. The arm's
attestation tests were run **eight times on a clean clone of this branch on win-2** (Windows, the pinned
1.99.0 toolchain), because the failure only appeared when the tests interleaved and one pass proves nothing:

```
passed 8 of 8
```

The dispatch itself took two attempts and the first is worth recording: the mesh's PowerShell job wrapper sets
`$ErrorActionPreference = 'Stop'`, and `rustup` writing "syncing channel updates" to **stderr** was treated as a
terminating error — the job died before running a test. A Windows shell job that runs a tool which writes to
stderr must set `Continue` first.

Also measured while trying to put these builds back on our own hardware: **`studio-linux2` has not pulled a job
since 2026-11-02.** Both its VMs are running; the host-only network's ARP table shows `.128` and `.129` while the
node card says `.130`, and neither address accepts the mesh key from win-2, so the guest could not be
identified from here. Recorded as HM25 in the mesh's hazard ledger, with the rule that a node's address is
measured at dispatch time and a queue that is not draining is a finding. Every build in this session therefore
went to CI, at roughly 50 minutes an iteration.

## Round 11: the image boots the kernel we built, and the Flow shell comes up on it

The `image/shell` leg's serial log, from the run at 14:09 on `dda300e` — the artefact is
`image-our-kernel-shell-serial-log` on that run:

```
[    0.000000] Linux version 6.8.0-morpheos (runner@runnervm48cm3) ...
[   10.497] flow-compositor: showing on /dev/dri/card0 at 1280x800
[   10.663] flow-compositor: input: QEMU QEMU USB Tablet (pointer)
[   10.669] flow-compositor: input: AT Translated Set 2 keyboard (keyboard)
[   10.680] flow-compositor: first frame on the display; ready
[   10.796] flow-compositor: 1 window(s) on screen
[   21.237] flow-compositor: session paused (another virtual terminal is showing)
[   22.132] flow-compositor: session active
```

That is our kernel, systemd reaching its targets, the compositor on the real DRM device, libinput finding a
keyboard and two pointers, a frame on the screen, and the scenario driving the console chord. **Slot A carries
our kernel; slot B still carries Ubuntu's** — the comparison Phase 0 falls back to.

**The one thing that failed is my test.** `MORPHEOS-KERNEL uname-s:` printed `/usr/bin/bash`: systemd does not
hand `ExecStart=/bin/sh -c '... "$(uname -s)"'` to a shell the way a shell would. The unit now calls a script
(`/usr/lib/morpheos/kernel-identity`) with `StandardOutput=journal+console` and no inline quoting to get wrong.

**Not claimed:** the Wi-Fi scenario on our kernel is still running in that same run, and nothing has touched
hardware. The VM proves the image; only the machine proves the machine.

## Round 12: two verdicts, both mine

The other two jobs in that run came back, and they are worth separating from the kernel's own behaviour:

- **`arm64` passed** — the three-arch gate is now green on all three (x86_64, arm64, arm) with the corrected
  fragment. That gate had never passed arm64.
- **`image/wifi` failed** — and its serial log shows our kernel booted (`Linux version 6.8.0-morpheos`) and then
  `flow-wifi: unavailable: no Wi-Fi device`. The scenario joins a WPA2 network through `mac80211_hwsim`, which
  Ubuntu's kernel ships in `linux-modules-extra`; our kernel did not build it, so the test asked a kernel
  without a simulated radio to invent one. Added to `config.x86_64`, with a comment saying it is a real kernel
  feature the suite depends on rather than a test hack.
- Both image legs failed the **sysname** assertion for the same reason (the unit's inline shell), which is fixed
  by the script above.

So the honest summary of that run: our kernel booted the image, drew the screen, found a keyboard and pointers,
and the three-arch gate is green; the two remaining reds were a broken test of mine and a missing test module.

