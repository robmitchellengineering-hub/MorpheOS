# 2026-10-14 · Claude Code · How to try an update by hand, as it really works

HARDWARE.md's update steps said three wrong things:

- that CI builds the payload stick. It uploads none (doc 20, E8).
- that the tool finds the stick by its label. It takes a path.
- that the stick mounts under `/mnt`. That is on the read-only root.

Now the steps say to build the payload with `build-payload.sh` on the machine that built the
stick, and to mount it read-only at `/run/morph-media`, as the self-test does. Then run
`morph-update ... install` and `morph-boot trial <slot> --reboot`.

The text printed on the stick (`overlay-hardware/etc/morpheos-test-image.txt`) now says the same.
Both now say that a payload older than the running release is refused (#68). That check comes
after the signature check and before the digests.
