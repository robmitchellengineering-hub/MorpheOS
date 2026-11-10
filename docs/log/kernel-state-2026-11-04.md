> Persisted from the planning room 2026-11-08 so the plan is git-tracked, not lost.

# MorpheOS's own kernel — state at the end of 2026-11-04

Written for whoever picks this up next: what is **measured**, what is **in flight**, what is **not claimed**, and
the traps that cost time today so they are not paid for twice.

## Measured, with the run that says so

| claim | evidence |
|---|---|
| the kernel builds for three architectures | kernel workflow, [run 41849919635](https://github.com/robmitchellengineering-hub/MorpheOS/actions/runs/41849919635): `x86_64`, `arm64`, `arm` all success |
| the kernel builds on our own hardware | `secure/kernel/build.sh` on `studio-linux2` (4 cores): `uname -s will : MorpheOS`, 59 modules signed, `hardening 10 / 10 present`, `6.8.0-morpheos`, **54m53s** |
| **the image boots our kernel** | `image/shell` in that run; `serial-shell.log`: `Linux version 6.8.0-morpheos (runner@runnervm48cm3)` |
| **the running system calls itself MorpheOS** | same log: `MORPHEOS-KERNEL uname-s: MorpheOS` |
| the Flow shell runs on it | same log: `showing on /dev/dri/card0 at 1280x800`, `first frame on the display; ready`, `1 window(s) on screen`, keyboard + two pointers from libinput |
| `test-shell` passes on our kernel | `image/shell`: success |
| `test-wifi` joins a WPA2 network on our kernel | `image/wifi`: success (over `mac80211_hwsim`, which our kernel now builds) |
| slot B keeps Ubuntu's kernel | `install_our_kernel` writes only slot A; the hardware workflow asserts slot B is **not** ours |

Quoted evidence and the analysis: [REVIEWS/2026-11-04-morpheos-kernel-boot.md](REVIEWS/2026-11-04-morpheos-kernel-boot.md).

## In flight when this was written
- **PR #181** (the arm's code on `main`) is green on every kernel and image check and waits on the everyday
  `images` suite — D26's falsifier, "the arm must not cost the build beside it". **Auto-merge is armed**, so it
  lands without anyone watching.
- The hardware image cannot be dispatched until that merge: `secure-image.yml` is `workflow_dispatch`-only, and
  GitHub only dispatches a workflow that exists on the **default branch**.

## Not claimed — and the two honest limits
1. **There is no audio scenario in this suite.** Thirteen `test-*.sh`, none touches a sound card; the only audio
   check is the build's driver assertion. "test-audio passes" cannot be said about this repository, in a VM.
   Audio is judged on hardware, through the hardware report.
2. **Nothing has run on hardware.** Phase 0's falsifier is `morpheos-a-1` booting our kernel with Wi-Fi (b43),
   audio (snd-hda-intel) and the panel (i915) intact, Ubuntu's kernel in slot B as the comparison. Runbook:
   [MORPHEOS-HARDWARE-BOOT-RUNBOOK.md](MORPHEOS-HARDWARE-BOOT-RUNBOOK.md).

## Next, in order
```
gh pr view 181 --json state                     # wait for MERGED (auto-merge is armed)
gh workflow run secure-image.yml --ref main     # the plain image with our kernel in slot A
gh run download <run-id> -n morpheos-x86-usb-ours-kernel -D ~/morpheos-usb
xz -dk ~/morpheos-usb/disk-plain.img.xz
```
then a USB stick (named and sized back before anything is written), and the runbook's checks on the laptop.

## The traps this cost, so the next person does not
- **`strings` cannot see inside a `bzImage`.** It is compressed; the release string is not in the clear. Every
  "is this the kernel we built" check is `cmp -s` against the artefact.
- **`update-initramfs -u -k all` does not know a kernel dpkg did not install.** Our initrd is built explicitly
  with `-c -k $KREL` inside `install_our_kernel`, from our own modules.
- **A module the test needs must be in *our* config.** `mac80211_hwsim` — the simulated radio `test-wifi` joins
  through — came from Ubuntu's `linux-modules-extra` until it was added to `config.x86_64`.
- **systemd does not hand `ExecStart=/bin/sh -c '…'` to a shell the way a shell does.** The identity unit now
  calls a script and takes its output with `StandardOutput=journal+console`.
- **`concurrency: cancel-in-progress` will cancel the run you are waiting on** if you keep pushing. Six pushes
  did exactly that today (HM26 in the mesh ledger). While waiting on a gate, the tree does not change.
- **A mute mesh node looks idle.** `studio-linux2` had not pulled since 2026-11-02: the two VMs had swapped NAT
  addresses, its loop was wedged behind a root-owned lock, and `repair-node.sh` could only fix a mute loop with
  `launchctl` (HM25). Restored and verified healthy; the repair now restarts and re-checks on Linux.
