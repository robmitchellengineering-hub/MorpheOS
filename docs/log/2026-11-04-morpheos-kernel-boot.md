> Persisted from the planning room 2026-11-08 so the plan is git-tracked, not lost.

# REVIEW — the image boots the kernel we built (2026-11-04)

Evidence for the first row of [doc 26](https://github.com/robmitchellengineering-hub/MorpheOS/blob/main/docs/26-secure-arm.md) K1 that had never held: the image the
suite boots runs **our** kernel, not Ubuntu's. Quoted, not summarised.

- **Run:** `MorpheOS` kernel workflow, [run 41844787758](https://github.com/robmitchellengineering-hub/MorpheOS/actions/runs/41844787758), jobs `image/shell` and `image/wifi`, on `dda300e`.
- **Artefacts:** `image-our-kernel-shell-serial-log`, `image-our-kernel-wifi-serial-log` (uploaded by the job, kept 30 days).
- **How the image was built:** `KERNEL=ours KERNEL_OUT=<build>` — slot A carries the kernel from `secure/kernel/`, slot B keeps Ubuntu's as the comparison. `install_our_kernel` installs the vmlinuz, the module tree, runs `depmod`, builds the initrd from *those* modules through the image's own initramfs-tools, and compares bytes against the artefact.

## The boot, from `serial-shell.log`

```
[    0.000000] Linux version 6.8.0-morpheos (runner@runnervm48cm3) (gcc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0, GNU ld (GNU Binutils for Ubuntu) 2.42
[    2.571749] systemd[1]: Reached target network-online.target - Network is Online.
[   10.497480] flow-compositor[277]: flow-compositor: showing on /dev/dri/card0 at 1280x800
[   10.663640] flow-compositor[277]: flow-compositor: input: QEMU QEMU USB Tablet (pointer)
[   10.669065] flow-compositor[277]: flow-compositor: input: AT Translated Set 2 keyboard (keyboard)
[   10.670860] flow-compositor[277]: flow-compositor: input: ImExPS/2 Generic Explorer Mouse (pointer)
[   10.680146] flow-compositor[277]: flow-compositor: first frame on the display; ready
[   10.682077] flow-compositor[277]: flow-compositor: 0 window(s) on screen
[   10.796706] flow-compositor[277]: flow-compositor: 1 window(s) on screen
[   17.731200] flow-compositor[277]: flow-compositor: splash done
[   21.237225] flow-compositor[277]: flow-compositor: session paused (another virtual terminal is showing)
[   22.132309] flow-compositor[277]: flow-compositor: session active
```

What that establishes, and nothing more: our kernel booted, systemd reached its targets, the compositor took
the DRM device and drew, libinput found a keyboard and two pointers, and the scenario drove the console chord.

## What the same run also said, and it is not the kernel's fault

```
MORPHEOS-KERNEL uname-s: /usr/bin/bash
```

The identity unit used `ExecStart=/bin/sh -c 'printf … "$(uname -s)"'`, which systemd does not hand to a shell
the way a shell would. The unit now calls a script (`/usr/lib/morpheos/kernel-identity`) with
`StandardOutput=journal+console`. So **`uname -s` read from inside the image is not yet proven** — the boot is.

```
flow-wifi: unavailable: no Wi-Fi device
boot-check[878]: morph-boot-check: failed unit: morph-wifi-test-ap.service
```

The Wi-Fi scenario joins a WPA2 network through `mac80211_hwsim`, which Ubuntu ships in `linux-modules-extra`.
Our kernel did not build it, so the test asked a kernel without a simulated radio to invent one. Added to
`config.x86_64`.

## Also from that run

- `arm64` **passed** — the three-arch gate is green on all three for the first time.
- `secure/kernel/build.sh` on `studio-linux2` (4 cores): `uname -s will : MorpheOS`, 59 signed modules,
  hardening 10/10, 54m53s. A kernel built on our own hardware, with the report that says what it applied.

## Not claimed
- The Wi-Fi join on our kernel (the missing module, now added, unproven).
- Anything on hardware. Phase 0's falsifier is `morpheos-a-1` with Wi-Fi, audio and the panel, and that has not
  been attempted.

## FOLLOW-UP — the run after the fixes: all five jobs green

[Run 41849919635](https://github.com/robmitchellengineering-hub/MorpheOS/actions/runs/41849919635) on `fdbbc66`:

| job | verdict |
|---|---|
| `x86_64` | **success** |
| `arm64` | **success** |
| `arm` | **success** |
| `image/shell` | **success** — the image booted our kernel **and** `MORPHEOS-KERNEL uname-s: MorpheOS` was read off the running system |
| `image/wifi` | **success** — the WPA2 join over `mac80211_hwsim` now works, because our kernel builds the simulated radio |

That closes the VM proof the objective asked for, with two honest qualifications: there is no audio scenario in
this suite (so "test-audio" cannot be passed or failed here — audio is a hardware-report item), and the
hardware falsifier on `morpheos-a-1` has still not been attempted. The VM proves the image; only the machine
proves the machine.

