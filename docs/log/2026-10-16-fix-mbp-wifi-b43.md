# 2026-10-16 · DeepSeek · BCM4331 Wi-Fi: the b43 firmware, not brcmsmac

The first fix (PR #98) blacklisted `b43` and shipped the free `brcmsmac` firmware. On the
MacBookPro9,1 it still did not see a network: `brcmsmac` is the flaky driver for this chip
(ArchWiki: "either b43 or broadcom-wl work"). So switch to `b43`, the driver that works, and
give it the firmware it asks for via `firmware-b43-installer` (multiverse). The build asserts
`b43/ucode29_mimo.fw` landed.
