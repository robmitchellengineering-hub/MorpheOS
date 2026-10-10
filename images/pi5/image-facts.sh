#!/bin/bash
# Gather the facts about a built Raspberry Pi image, for scripts/verify-pi-image.mjs
# to assert on.
#
#   sudo images/pi5/image-facts.sh <image> [path/to/pi.rs] > facts.txt
#
# WHY THIS IS A SEPARATE SCRIPT FROM THE ASSERTIONS, and why it is not a verifier
# itself. Reading a 3.29 GiB disk image needs root, `losetup` and `mount`, none of
# which exist on a development Mac — and building that image takes ~17 minutes in
# CI. A single script that both read the image and judged it would therefore be
# testable **nowhere except a CI run**, so every mistake in the judgement would
# cost a build to discover. This script only OBSERVES; the assertions live in the
# .mjs, which takes a facts file and can therefore be run — and broken — anywhere.
#
# WHAT THE IMAGE ACTUALLY IS, learned from the first run rather than assumed:
#   * GPT, six partitions, from rpi-image-gen's `image/gpt/ab_userdata` layer;
#   * p1 vfat, label BOOTCONFIG — rw boot metadata, where `autoboot.txt` lives;
#   * p2/p3 vfat, label BOOT — slot A's and slot B's boot files;
#   * p4/p5 EROFS — the read-only system root of each slot
#     (`IGconf_image_rootfs_type=erofs`, which is why blkid gives them no type and
#     why a plain `mount` refuses them);
#   * p6 ext4, label PERSISTENT — shared writable state.
#
# The output is line-oriented so the verifier's parser stays trivial:
#
#   image <bytes>
#   table <label>
#   partition <n> <size-bytes> <fstype> <label>
#   mount <n> ok|failed [how] [why]
#   probe <n> <blkid -p type or unknown>
#   magic <n> <first 16 bytes, hex>
#   file <n> <path> <bytes>|missing
#   content <n> <path> <text with \n and \t escaped>
#   autoboot <section>.<key>=<value>
#   const <NAME> <value>
#   note <anything>
#
# A partition that will not mount is a FACT TO REPORT, not a reason to stop: the
# verifier says which one, the reason is captured, and the log keeps the rest.
set -uo pipefail

IMG=${1:?usage: image-facts.sh <image> [path/to/pi.rs]}
[ -f "$IMG" ] || { echo "no such image: $IMG" >&2; exit 1; }
PI_RS=${2:-}

say() { printf '%s\n' "$*"; }

# Files small enough to print whole and text enough to be worth printing. This is
# what turns "config.txt is 15 bytes" into a fact you can act on.
dumpable() {
    case "$1" in
        autoboot.txt|config.txt|cmdline.txt|cmdline|issue|kernel/cmdline) return 0 ;;
        *) return 1 ;;
    esac
}

# GNU stat on Linux; the BSD form is a fallback so that running this on a Mac
# degrades into honest output instead of a blank number.
size_of() { stat -c %s "$1" 2>/dev/null || stat -f %z "$1" 2>/dev/null || echo 0; }

say "image $(size_of "$IMG")"
# Whether this kernel can read EROFS at all decides how to read a failed mount:
# "the image is empty" and "this kernel has no erofs driver" look identical from
# `mount` alone. The first run could not tell them apart, and reported the second
# as if it were the first.
say "kernel erofs=$(grep -qw erofs /proc/filesystems 2>/dev/null && echo yes || echo no)"
command -v fsck.erofs >/dev/null && say "note fsck.erofs available" || say "note fsck.erofs NOT available"
command -v sfdisk >/dev/null && say "table $(sfdisk -d "$IMG" 2>/dev/null | sed -n 's/^label: //p' | head -1)"

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

# Try the filesystems this image is actually made of, and report EVERY attempt's
# reason rather than only the last one. Reporting just the last was a real mistake:
# the failures are tried in order and the final message came from the `vfat`
# attempt, which made a kernel that cannot mount EROFS look like a corrupt
# superblock.
#
# EROFS is not guaranteed to be in a CI kernel at all, so `fsck.erofs --extract`
# is the fallback that does not need kernel support — it is the difference between
# "this image is empty" and "this kernel cannot read it", which are very different
# findings.
mount_part() {
    local dev=$1 dir=$2 spec t opts tried=""
    if mount -o ro "$dev" "$dir" 2>/tmp/.mnt.err; then echo "ok auto"; return 0; fi
    tried="auto: $(tr -d '\n' < /tmp/.mnt.err | tail -c 90)"
    for spec in "erofs:-t erofs -o ro" "ext4:-t ext4 -o ro,noload" "vfat:-t vfat -o ro"; do
        t=${spec%%:*}; opts=${spec#*:}
        # shellcheck disable=SC2086
        if mount $opts "$dev" "$dir" 2>/tmp/.mnt.err; then echo "ok $t"; return 0; fi
        tried="$tried | $t: $(tr -d '\n' < /tmp/.mnt.err | tail -c 90)"
    done
    # No kernel filesystem driver worked. erofs-utils reads the image in userspace.
    if command -v fsck.erofs >/dev/null; then
        if fsck.erofs --extract="$dir" "$dev" >/dev/null 2>/tmp/.erofs.err; then
            echo "ok fsck.erofs"; return 0
        fi
        tried="$tried | fsck.erofs: $(tr -d '\n' < /tmp/.erofs.err | tail -c 90)"
    fi
    echo "failed $tried"
    return 1
}

for dev in $(ls -1 "${LOOP}"p* 2>/dev/null | sort -V); do
    n=${dev##*p}
    size=$(blockdev --getsize64 "$dev" 2>/dev/null || echo 0)
    fstype=$(blkid -o value -s TYPE "$dev" 2>/dev/null || echo "")
    label=$(blkid -o value -s LABEL "$dev" 2>/dev/null || echo "")
    say "partition $n $size ${fstype:-none} ${label}"

    dir="$MNT/$n"
    mkdir -p "$dir"
    result=$(mount_part "$dev" "$dir")
    say "mount $n $result"

    if [ "${result%% *}" = "ok" ]; then
        for rel in autoboot.txt cmdline.txt config.txt etc/kernel/cmdline usr/bin/morph-boot \
                   usr/lib/morpheos/boot-check usr/lib/morpheos/test-hook \
                   usr/lib/systemd/system/morph-boot-check.service \
                   usr/lib/systemd/system/morph-boot-check.service.d/10-pi.conf \
                   etc/systemd/system/multi-user.target.wants/morph-boot-check.service; do
            if [ -e "$dir/$rel" ]; then
                say "file $n $rel $(size_of "$dir/$rel")"
                if dumpable "$rel" && [ "$(size_of "$dir/$rel")" -lt 4096 ]; then
                    # Escape newlines so one fact stays on one line.
                    say "content $n $rel $(sed -e ':a' -e 'N' -e '$!ba' -e 's/\t/\\t/g' -e 's/\n/\\n/g' "$dir/$rel")"
                fi
            else
                say "file $n $rel missing"
            fi
        done
        # autoboot.txt is SECTIONED, and the sections are the whole point: the
        # default slot is `[all] boot_partition` and the trial is `[tryboot]
        # boot_partition`. Flattening it — which the first version of this script
        # did — turns two different settings into one key and invents a failure.
        if [ -f "$dir/autoboot.txt" ]; then
            section=all
            while IFS= read -r line; do
                line=$(printf '%s' "$line" | tr -d '[:space:]')
                [ -n "$line" ] || continue
                case "$line" in
                    \[*\]) section=$(printf '%s' "$line" | tr -d '[]'); continue ;;
                esac
                key=${line%%=*}
                [ "$key" != "$line" ] || continue
                say "autoboot $section.$key=${line#*=}"
            done < "$dir/autoboot.txt"
        fi
    else
        # Why not: what the kernel thinks is there, and the superblock itself.
        # BOTH erofs and ext4 put their superblock at offset 1024 — an earlier version
        # of this read offset 0, which is zeros for both and told us nothing.
        say "probe $n $(blkid -p -o value -s TYPE "$dev" 2>/dev/null || echo unknown)"
        sb=$(dd if="$dev" bs=1 skip=1024 count=64 2>/dev/null | od -An -tx1 | tr -d ' \n')
        say "magic $n ${sb:0:8} ${sb:8:8}"
        # An EROFS superblock, decoded enough to judge the filesystem without a
        # reader for it. THIS IS WHY IT MATTERS HERE: the slot rootfs is EROFS with a
        # **16 KiB block size** (the Pi 5's page size, from
        # `IGconf_fs_erofs_mkfs_args=-b ${IGconf_linux_page_size}`), and NO tool on a
        # GitHub runner can read it — the kernel has no erofs driver at all
        # (`kernel erofs=no`) and userspace `fsck.erofs` refuses 16 KiB blocks
        # ("blksize 16384 isn't supported on this platform"). So "the rootfs would
        # not mount" is a fact about the RUNNER, not about the image.
        #
        # struct erofs_super_block: magic u32 @0, blkszbits u8 @12, blocks u32 @36.
        if [ "${sb:0:8}" = "e2e1f5e0" ]; then
            bits=$(dd if="$dev" bs=1 skip=$((1024 + 12)) count=1 2>/dev/null | od -An -tu1 | tr -d ' ')
            blocks=$(dd if="$dev" bs=1 skip=$((1024 + 36)) count=4 2>/dev/null | od -An -tu4 | tr -d ' ')
            size=$(( (1 << ${bits:-0}) * ${blocks:-0} ))
            say "erofs $n magic=ok blocksize=$((1 << ${bits:-0})) blocks=${blocks:-0} bytes=$size"
        else
            say "erofs $n magic=bad"
        fi
    fi
done

# The constants morph-boot's Pi backend hard-codes. The whole point of checking the
# image is that these two facts — "partition 1 selects, 2 and 3 are the slots" —
# must still be true of the image the firmware will actually boot.
if [ -n "$PI_RS" ] && [ -f "$PI_RS" ]; then
    while IFS= read -r name value; do
        say "const $name $value"
    done < <(sed -n 's/^pub const \(SLOT_[AB]_PARTITION\): u32 = \([0-9]*\);/\1 \2/p' "$PI_RS")
fi
