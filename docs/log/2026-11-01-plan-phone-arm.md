# 2026-11-01 · DeepSeek · the Phone arm, and what the Linux phone field gives us

Rob's ask: *"we have a version of MorpheOS that will run on Android phones, this version will need to be
built out to do phone functions too as a separate arm of the build full calls messages etc."*

Docs 24 and 25 are that arm and the research behind it. Both are **proposals** — nothing in the build
changed, and nothing here is accepted.

## What the research settled

- **Mobian is the device layer to build on.** Debian-native, so its packages drop into an mmdebstrap
  build; it already supports the OnePlus 6/6T and Pocophone F1 on a **6.12 mainline kernel** — the same
  shortlist we reached independently from `status_audio` / `status_otg` on the postmarketOS wiki.
- **postmarketOS is the reference** for what works per device; its Alpine packaging is the wrong format
  for us, but its device data is the best "does this actually work" source.
- **Droidian is the fallback** (Halium + libhybris on a vendor kernel) — it works, and it defeats the
  point of mainline, so it stays a fallback.
- **FuriOS is evidence, not a dependency**: a Debian phone with working calls, mobile data and MMS in
  2026, whose changelog is an honest list of what still breaks.

## The gate is VoLTE, and it is M0

3G is switched off in New Zealand and 2G is gone. A mainline phone without working VoLTE **cannot place a
call on a 4G-only network**, whatever the dialer does. The mechanism is concrete and already packaged:
**`81voltd`**, in Debian trixie, arm64, maintained by the DebianOnMobile team — *"the IMS Data service
for VoLTE calls on Qualcomm modem using the QMI/QRTR protocol"*. So M0 is a configured call attempt on a
real SIM, not a research project, and it is numbered zero because it decides whether the arm is worth a
quarter of work.

## The trap that was checked, not assumed

The same handsets fail the *audio* node test for an unrelated reason: `status_otg` is **N** on the OnePlus
6 and Poco F1 and **P** on the 6T, so a UAC2 interface cannot attach. That does not block the Phone arm —
a phone calls through its own modem and codec — but it would have been an unpleasant surprise later.
Recorded in doc 24 so it is not rediscovered.

## Why this is a plan and not work

A phone build needs a handset, the Mobian device packages on our image build, a Flow phone profile, and a
Telstra SIM for M0. Rob is on Telstra; no handset has been bought. Nothing here can be verified from this
machine, and the doc says so rather than implying readiness.
