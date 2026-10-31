# 2026-10-29 · DeepSeek · the splash's music, played for the first time

Rob, on the first real-hardware boot: *"there is no audio on the splash screen when it boots"*.

Correct, and structural. The boot splash is the **software port** of the design
(`crates/flow-compositor/src/splash.rs`), and it mentions audio **zero times** — the music lives in
the browser prototype (`design/login.html`, three.js with an audio master clock) and nothing ever
played it on a boot. The image already shipped the track (`/usr/share/morpheos/theme.mp3`) and
PipeWire (`pipewire`, `pipewire-pulse`, `alsa-ucm-conf`) — so the file and the stack were present
and nothing connected them.

## What this adds

- **The track as WAV.** `design/theme.mp3` → `theme.wav` (1.8 MB, 10.5 s, 44.1 kHz stereo) with
  `afconvert`, shipped beside the mp3. `aplay` plays WAV directly; an mp3 would need a decoder the
  boot path does not have.
- **`alsa-utils`** in `BASE_PKGS` — the image had the ALSA libraries and PipeWire, but no `aplay`.
- **`morph-boot-sound.service`** — `After=flow-shell.service`, because the splash *is* the
  compositor's first frames; starting the track earlier plays most of it to a blank display.
  `ConditionPathExists` the track; `ExecStart=-` so a chime can never make a trial boot unhealthy
  (E11) and roll an update back.
- **`boot-sound`** — unmutes Master/PCM (a fresh ALSA state is often muted, which is another silence
  that looks like a missing driver) and chooses the card by name: internal speakers preferred, HDMI
  only as a last resort.

## Why the card choice is the whole job

The machine that booted has **two HDA devices** — Intel analog for the speakers, NVIDIA HDMI for a
monitor. HDMI with nothing plugged in is silence that is indistinguishable from "audio is broken",
so a player that takes the default sink very likely plays into the void. The chooser is a pure
function of `aplay -l` output and is tested here against real layouts:

| `aplay -l` layout | chosen |
|---|---|
| one Intel analog card | `card 0 device 0` |
| NVIDIA HDMI first, Intel analog second | **`card 1 device 0`** — the analog, not the HDMI |
| HDMI only | `card 0 device 3` — best effort, and honest about it |

## Verified here

| check | result |
|---|---|
| `bash -n` on `build.sh` and `boot-sound` | clean |
| the chooser against the three layouts above | as tabled — the HDMI trap is avoided |
| the build asserts the script, the unit **and** the track are in the image, and that `aplay` exists | added, so "silent splash" fails the build instead of costing a boot |

**Not verified:** actual sound out of that MacBook's speakers. That needs the built image and a
boot — and this is exactly the class of thing the hardware report and the live snapshot cannot
hear, so the proof is Rob pressing play on a boot, or me hearing it once the mesh is up.
