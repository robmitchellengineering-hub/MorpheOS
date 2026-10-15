# 2026-10-16 · DeepSeek · E18 built: the mount half — flow-mount and a udev rule

The mount half of E18: `flow-mount` (a `flow-files` bin, Linux-only) reads the device from udev's
environment (`DEVNAME`, `ID_FS_LABEL`, `ID_FS_UUID`), decides the mount point with the pure
`mount_point`, and mounts it. A udev rule in the shell image runs it when a USB filesystem
appears, and `build.sh` installs the binary. The Linux parts are cross-checked with the Linux
target clippy here; CI proves them on the runner.

Next: the QEMU scenario that attaches a virtual USB disk and asserts the file view lists and
opens it.
