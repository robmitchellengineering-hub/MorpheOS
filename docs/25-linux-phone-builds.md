# 25 · Linux phone builds: what to take, what to ignore

2026-11-01. Research for the Phone arm (doc 24). Status: **research note**, not a decision.
Question asked: what existing Linux phone work can we use code or inspiration from?

**Headline:** one project is a near-perfect fit and three are references. **Mobian** is Debian-native,
mainline-kernel, and **already supports the exact handsets we chose** — which matters because MorpheOS's
image build is Debian-family (mmdebstrap), not Alpine.

## The field

| project | base | device approach | UI | what we take | licence |
|---|---|---|---|---|---|
| **Mobian** ⭐ | **Debian** (Trixie stable, Oct 2025) | **mainline** kernels; **6.12 for almost all devices** | Phosh 46 / Plasma 6.3 | **device packages (kernel, dtb, UCM, firmware extraction) in Debian format**, from `salsa.debian.org/Mobian-team`; image-based update model | DFSG-free packaging (GPL family) |
| **DebianOnMobile** | Debian | upstreaming team for the mobile bits | — | packages already in Debian: phosh, modemmanager integration, mobile tweaks | Debian |
| **postmarketOS** | Alpine | mainline, widest device range | Phosh/Plasma/Sway | **pmaports device data as the reference of what works per device**; firmware-extraction scripts; `mkbootimg` tooling | GPL/MIT mix |
| **Droidian** | Mobian + **Android kernel + libhybris/Halium** | for handsets with **no** mainline port | Phosh | the **fallback path** if we ever need a vendor driver; concept of the Halium adaptation layer | GPL |
| **Ubuntu Touch (Lomiri)** | Ubuntu + Halium | Android kernels | Lomiri | **OTA / A-B update model** and the "device tarball + system image" split — the same shape as MorpheOS's A/B slots | GPL |
| **FuriOS (FuriLabs)** | **Debian Forky** (14.0.1, Jan 2026) | mainline + **Andromeda** for Android apps | Flutter | proof that a **Debian** phone with calls, mobile data and MMS attachments is viable now; their changelog is the honest bug list | mixed, partly open |
| Sailfish | Mer/Nemo | Android drivers | Silica | nothing usable (licence) | proprietary |
| PinePhone / Megi kernel | — | mainline reference work | — | modem, audio and power-management patterns | GPL |

## Why Mobian is the one

1. **Same packaging family as us.** MorpheOS's arm64 image is built with mmdebstrap from a Debian-family
   userland. Mobian's device packages are Debian packages. postmarketOS's are Alpine (APKBUILD) — every
   one would need rewriting to be used directly.
2. **It supports our target handsets on mainline**: OnePlus 6, OnePlus 6T, Pocophone F1, Pixel 3a, plus
   the Linux-native phones. That is the same shortlist we arrived at from `status_audio`/`status_otg`.
3. **Kernel 6.12 across the set** — a recent, consistent baseline, not a per-device fork zoo.
4. **The Debian team is upstreaming** (DebianOnMobile), so the packaging improves without us maintaining it.

## What we would actually reuse

| piece | from | how |
|---|---|---|
| kernel + dtb per device | Mobian / DebianOnMobile | add their Debian source package to our build |
| UCM (call + media audio routing) | Mobian / pmaports | `alsa-ucm-conf` per device |
| firmware extraction scripts | pmaports | **never redistributing a blob** — a fetch step that runs against the handset's own Android install |
| telephony | upstream | `modemmanager` + `libqmi` (QMI backend) |
| phone UX | Phosh's apps (GNOME Calls, Chatty) | either run them as Wayland clients under Flow, or take the interaction model for the Flow phone profile |
| update model | Ubuntu Touch / MorpheOS A/B | slots, one root at a time (doc 23 already notes arm64 has no A/B yet) |

## What to ignore, and why

- **Alpine packaging** (pmaports) as a build system — great data, wrong package format for us.
- **Halium/libhybris** as a first step — it is the "Android drivers" path; it works, but it locks us to a
  vendor kernel and defeats the point of mainline. Keep it as the fallback for a handset we must support.
- **Sailfish, Flutter-based UI choices** — nothing to take; FuriOS is useful as evidence, not as code.
- **Any distro's phone UI wholesale** — Flow is the product; Phosh is the reference, not the destination.

## The consequence for the plan

The Phone arm should **base its device layer on Mobian's Debian packages for OnePlus 6 / 6T**, and use
pmaports only as the "what is known to work" reference. That turns "port a modem" into "add a device
package and test it", which is the difference between a quarter and a year.

And it sharpens **M0 (VoLTE)**: Mobian and FuriOS both ship on real carriers, so the question is not
"does Linux do VoLTE" in the abstract — it is "does this handset + this carrier work", which is a test on
hardware, not a literature review.
