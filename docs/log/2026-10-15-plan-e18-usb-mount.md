# 2026-10-15 · DeepSeek · E18 opened: a USB stick shows up in the file view

Rob asked how a stick with data gets onto the computer, and said "keep going" on the
usability gaps. This `plan/` PR adds E18 (USB mounting) as `open · parallel`, on top of E17:
the stick is mounted automatically when it appears, so the file view lists its files and a
person can open or copy them to `/persist` without a terminal.

- Rob on E18: "if i plug a stick in with data that i want on the computer how does that
  process work", in the session, 2026-10-15.

Next: `feat/e18-usb-mount` — the mount point as a pure function (unit-tested) first, then a
udev rule, then a QEMU scenario that attaches a virtual USB disk and opens a file from it.
