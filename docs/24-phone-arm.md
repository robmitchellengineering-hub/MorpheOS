# 24 · The Phone arm: MorpheOS that has to make a call

2026-11-01. A third line beside the laptop image and the Slate (doc 23). Status: **proposal, for
consideration.** Written to be decided on, not just described: the shell is the easy part, the modem is
the project.

## 1. What "phone functions" means, concretely

| function | depends on |
|---|---|
| calls out / in, with two-way audio | modem + voice-call audio routing (ADSP/UCM) |
| SMS / MMS | modem + an SMS stack and a store |
| SIM detection, PIN, APN, data | modem + ModemManager provisioning |
| emergency call (no SIM, no UI) | modem + a policy that is not allowed to be wrong |
| contacts, call log, missed-call notice | the shell, once telephony events exist |
| airplane mode, dual-SIM (later) | modem + shell |

## 2. The stack — nothing exotic, all of it standard Linux

- **ModemManager** over D-Bus, with the **QMI** backend and `libqmi`/`libqrtr` for Qualcomm modems.
  This is what postmarketOS and Phosh use, and it is what a phone UI talks to.
- **Voice-call audio**: routed through **UCM** (`alsa-ucm-conf`) per device, because the modem's voice
  path lives in the ADSP and every handset wires it differently.
- **The shell**: MorpheOS's Flow compositor gains a **phone profile** — dialer, in-call screen, SMS,
  contacts, and an overlay that can take the screen for an incoming call. That is real work, but it is
  work we control.
- **Emergency calls**: must work with no SIM and without the UI being up. It is a correctness
  requirement, not a feature.

## 3. Where the build starts — do not port a modem from scratch

**Start from a device postmarketOS already supports**, and take their device/kernel packages as the
device layer. Modem bring-up is where months disappear, and on SD 845 handsets it is already done.

The obvious target is the **OnePlus 6 (`enchilada`)**: `status_audio = Y`, calls and call audio working
with pipewire since pmOS v26.06, and modem firmware extractable from the device's own Android install.

Note what this changes from the AirMatrix node work: **`status_otg = N` on that handset does not matter
here.** USB host mode was only needed for a USB audio interface; a phone makes calls with its own modem
and codec.

### Firmware, legally

Modem and ADSP firmware are vendor blobs and cannot be redistributed. The build therefore carries a
**fetch step** — extract from the handset's own Android partition, or the vendor firmware package — never
a blob in our repo. The image is not complete without that step, and it must fail loudly when the firmware
is absent rather than booting a phone that cannot call.

## 4. The risks, worst first

| # | risk | why it is first |
|---|---|---|
| **1** | **VoLTE** | 3G is switched off in NZ and 2G is gone. A mainline phone without working VoLTE **cannot place a call on a 4G-only network**, no matter how good the rest is. Carrier IMS support in mainline is partial and device-specific. **This decides whether the arm is viable at all**, and it is the first thing to test on the target handset and carrier. |
| 2 | call audio routing | UCM + ADSP per device; silent failures (call connects, no audio) |
| 3 | emergency calling | has to work when everything else is broken; must be verified on real networks, not emulated |
| 4 | power management | mainline suspend is usually worse than Android's; a phone that lasts 6 hours is not a phone |
| 5 | modem firmware | licensing + per-device extraction, as above |

## 5. The build arm itself

A new image target beside the laptop and Slate ones:

- **arch**: arm64, device-specific kernel + dtbs, not a generic UEFI image
- **packages**: `modemmanager`, `libqmi`, `libmbim`, `alsa-ucm-conf`, pipewire, the Flow shell with the
  phone profile
- **artefact**: a flashable image per device, built the same way the arm64 image is (mmdebstrap + the
  MorpheOS units), with a device profile selecting kernel, dtb and UCM
- **test matrix**, in order: modem detected → SIM read → SMS out/in → call out/in with audio both ways →
  emergency call → airplane mode → 24 h idle battery

## 6. Milestones

| | milestone | proof |
|---|---|---|
| **M1** | the device boots MorpheOS and the modem is seen | `mmcli -L` lists it, `mmcli -m 0` shows SIM + operator |
| **M2** | SMS out and in, headless | `mmcli --messaging-create-sms` and a received message in the modem store |
| **M3** | a call out and in, audio both ways | real network, recorded both directions |
| **M4** | Flow phone UI | dialer, in-call screen, SMS, contacts, incoming-call overlay |
| **M0** | **VoLTE answer on the target device/carrier** | a call placed on a 4G-only SIM — **do this before M1** |

**M0 is deliberately numbered zero.** If VoLTE cannot work, the arm is a tablet with a modem, and we
should know that before spending a quarter on a dialer.

## 7. What this is not

It is not Android compatibility, not Play Services, not app-store apps. It is MorpheOS with a telephony
stack — a phone that makes calls, sends messages, and runs our shell.
