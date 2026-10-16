# Broadcom BCM43xx firmware (brcmsmac)

`bcm43xx-0.fw` and `bcm43xx_hdr-0.fw` are the firmware for the `brcmsmac` driver, which
drives the Broadcom BCM4331 Wi-Fi chip in the 2012 MacBook Pro (MacBookPro9,1) — and the
other BCM43xx chips `b43` otherwise tries and fails on (b43 needs Broadcom's proprietary
firmware, which is not redistributable the way this one is).

Source: `brcm/` in the linux-firmware tree
(https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/).
License: `LICENCE.broadcom_bcm43xx` — redistributable firmware; the same files Debian ships
in `firmware-brcm80211` (main) and Ubuntu in `linux-firmware`.
