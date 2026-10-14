# 2026-10-14 · Claude Code · HARDWARE.md gives the image's real unpacked size

HARDWARE.md said the stick image is "~4.5 GiB when unpacked". It is 3758 MiB (3.7 GiB):
the ESP (300 MiB), two slots of 1600 MiB each, the data partition (256 MiB) and 2 MiB of
GPT. This is computed from `build.sh` (the `truncate` line), not measured on a downloaded
file. The download, a zip holding an `.xz`, is about 1.3 GB (Measured: the
`morpheos-x86-usb` artifact of images run 38604870469 is 1,322,275,374 bytes). Rob asked why
yesterday's "3.something GB" and today's 1.3 GB differed; it is one image, packed and
unpacked.
