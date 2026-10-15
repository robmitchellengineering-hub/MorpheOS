# 2026-10-16 · DeepSeek · E18 built: the mount half — flow-mount and a udev rule

The mount half of E18: `flow-mount` (a `flow-files` bin, Linux-only) reads the device from udev's
environment (`DEVNAME`, `ID_FS_LABEL`, `ID_FS_UUID`), decides the mount point with the pure
`mount_point`, and mounts it. A udev rule in the shell image runs it when a USB filesystem
appears, and `build.sh` installs the binary. The Linux parts are cross-checked with the Linux
target clippy here; CI proves them on the runner.

The QEMU scenario (`test-usb.sh`) is in this PR too: it boots `plain` with a FAT stick
attached as a USB storage device and asserts flow-mount mounted it at `/run/media/<label>`.
