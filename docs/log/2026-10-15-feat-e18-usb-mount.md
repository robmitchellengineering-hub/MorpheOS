# 2026-10-15 · DeepSeek · E18 built: where a USB stick is mounted (pure, unit-tested)

The mount-point decision for E18 (`open · parallel`): `flow_files::mount_point(label, uuid)`
turns a removable disk's label or UUID into a stable, sanitised place under `/run/media`, so
the file view lists a stick by a name a person reads, and a label with spaces or shell
metacharacters cannot name anything but a directory. Unit-tested in `lib.rs` (6 tests now),
run anywhere.

Next (Linux-only, CI-proven): the udev rule and mount that use this decision, then a QEMU
scenario that attaches a virtual USB disk and opens a file from it.
