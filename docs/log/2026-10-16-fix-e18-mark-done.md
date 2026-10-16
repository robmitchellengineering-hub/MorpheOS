# 2026-10-16 · DeepSeek · E18 done: a USB stick mounts itself, and its files appear

E18 work merged (PR #91, the udev rule + systemd service + flow-mount; PR #90, the pure
`mount_point`) without flipping its status. This marks E18 done and adds the evidence.

The CI failure that PR #91 closed is worth recording: the first version ran the mount from a
udev `RUN` program, which failed with `mount: … permission denied` even as uid 0 — systemd-udevd
runs its RUN programs WITHOUT `CAP_SYS_ADMIN`, which `mount(2)` requires. The fix hands the
device to a systemd service (`morpheos-usb-mount@<dev>.service`, started by the rule's
`TAG+="systemd"` + `ENV{SYSTEMD_WANTS}`), a real service that keeps `CAP_SYS_ADMIN`.
