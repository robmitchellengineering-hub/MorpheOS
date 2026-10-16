# 2026-10-16 · DeepSeek · the plain image works on the 2012 MacBook Pro

The E6 boot on a MacBookPro9,1 (2012 15" Retina) reported "no Firefox, no network". The
hardware report pinned both causes:

- **Wi-Fi** — the BCM4331 chip loads `b43`, whose firmware (`b43/ucode29_mimo.fw`) is
  Broadcom's proprietary firmware and is not in the image. Fix: blacklist `b43` and ship the
  free `brcmsmac` firmware (`brcm/bcm43xx-0.fw`, from linux-firmware, committed under
  `images/common/firmware/brcm/`).
- **Firefox** — no crash, but the image has no Mesa, and Firefox 158 needs GL (WebRender) to
  draw, so its window never rendered. Fix: add `libgl1-mesa-dri libegl-mesa0 libgbm1
  libglx-mesa0` to the browser base.
