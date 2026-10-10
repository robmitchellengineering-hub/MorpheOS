#!/bin/bash
# Gather the facts about a built Raspberry Pi image, for scripts/verify-pi-image.mjs
# to assert on.
#
#   sudo images/pi5/image-facts.sh <image> > facts.txt
#
# WHY THIS IS A SEPARATE SCRIPT FROM THE ASSERTIONS, and why it is not a verifier
# itself. Reading a 3.29 GiB disk image needs root, `losetup` and `mount`, none of
# which exist on a development Mac — and building that image takes ~17 minutes in
# CI. A single script that both read the image and judged it would therefore be
# testable **nowhere except a CI run**, so every mistake in the judgement would
# cost a build to discover.
#
# This script only OBSERVES. It asserts nothing and always exits 0 unless it
# cannot open the image at all. The assertions live in the .mjs, which takes a
# facts file, so it runs against captured or synthetic facts in a second, on any
# machine — and can be shown to fail.
#
# The output is line-oriented so the verifier's parser stays trivial:
#
#   image <bytes>
#   table <label>
#   partition <n> <size-bytes> <fstype> <label>
#   mount <n> ok|failed
#   file <n> <path> <bytes>|missing
#   autoboot <key>=<value>
#   const <NAME> <value>
#   note <anything>
#
# A partition that will not mount is a FACT TO REPORT, not a reason to stop: the
# verifier says which one and why, and the log keeps the rest.
set -uo pipefail

IMG=${1:?usage: image-facts.sh <image>}
[ -f "$IMG" ] || { echo "no such image: $IMG" >&2; exit 1; }

# Where the constants the image has to agree with are declared. Read from source
# rather than repeated here, so the number lives in exactly one place.
PI_RS=${2:-}

say() { printf '%s\n' "$*"; }

say "image $(stat -c %s "$IMG")"

if command -v sfdisk >/dev/null; then
    say "table $(sfdisk -d "$IMG" 2>/dev/null | sed -n 's/^label: //p' | head -1)"
fi

LOOP=$(losetup -Pf --show "$IMG" 2>/dev/null) || LOOP=""
if [ -z "$LOOP" ]; then
    say "note losetup -Pf failed; partitions could not be mapped"
    exit 0
fi
say "note loop device $LOOP"

MNT=$(mktemp -d)
cleanup() {
    for d in "$MNT"/*; do [ -d "$d" ] && umount "$d" 2>/dev/null; done
    losetup -d "$LOOP" 2>/dev/null
    rm -rf "$MNT"
}
trap cleanup EXIT

# Enumerate the partitions the kernel actually produced for this loop device,
# in numeric order, however the table names them.
PARTS=$(ls -1 "${LOOP}"p* 2>/dev/null | sort -V)
for dev in $PARTS; do
    n=${dev##*p}
    size=$(blockdev --getsize64 "$dev" 2>/dev/null || echo 0)
    fstype=$(blkid -o value -s TYPE "$dev" 2>/dev/null || echo none)
    label=$(blkid -o value -s LABEL "$dev" 2>/dev/null || echo "")
    say "partition $n $size ${fstype:-none} ${label}"

    dir="$MNT/$n"
    mkdir -p "$dir"
    if mount -o ro "$dev" "$dir" 2>/dev/null; then
        say "mount $n ok"
        # The paths that matter, each reported whether or not it is there: a
        # verifier that only hears about successes cannot report a missing file.
        for rel in autoboot.txt cmdline.txt config.txt usr/bin/morph-boot \
                   usr/lib/morpheos/boot-check usr/lib/morpheos/test-hook \
                   usr/lib/systemd/system/morph-boot-check.service \
                   usr/lib/systemd/system/morph-boot-check.service.d/10-pi.conf \
                   etc/systemd/system/multi-user.target.wants/morph-boot-check.service; do
            if [ -e "$dir/$rel" ]; then
                say "file $n $rel $(stat -c %s "$dir/$rel" 2>/dev/null || echo 0)"
            else
                say "file $n $rel missing"
            fi
        done
        if [ -f "$dir/autoboot.txt" ]; then
            while IFS= read -r line; do
                # Only the entries the firmware format defines; anything else is
                # passed through so the verifier can refuse an unknown key rather
                # than silently ignore it.
                key=${line%%=*}
                val=${line#*=}
                [ "$line" = "$key" ] && continue
                key=$(printf '%s' "$key" | tr -d '[:space:]')
                [ -n "$key" ] || continue
                say "autoboot $key=$(printf '%s' "$val" | tr -d '[:space:]')"
            done < "$dir/autoboot.txt"
        fi
    else
        say "mount $n failed"
    fi
done

# The constants morph-boot's Pi backend hard-codes. The whole point of checking
# the image is that these two facts — "partition 1 selects, 2 and 3 are the
# slots" — must still be true of the image the firmware will actually boot.
if [ -n "$PI_RS" ] && [ -f "$PI_RS" ]; then
    while IFS= read -r name value; do
        say "const $name $value"
    done < <(sed -n 's/^pub const \(SLOT_[AB]_PARTITION\): u32 = \([0-9]*\);/\1 \2/p' "$PI_RS")
fi
