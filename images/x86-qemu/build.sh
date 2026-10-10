#!/bin/bash
# Build a MorpheOS A/B test disk for a UEFI x86_64 VM.
#
#   images/x86-qemu/build.sh <workdir> <scenario>
#
# scenario: good | unhealthy | crash   (what slot B's "update" does)
#           ransomware                 (no update: slot A attacks its own files,
#                                       and Strata has to get them back)
#
# Disk layout (GPT):
#   1 esp     FAT32  systemd-boot, loader.conf, one kernel+initrd per slot
#   2 root-a  ext4   slot A system (mounted read-only)
#   3 root-b  ext4   slot B system (mounted read-only)
#   4 data    ext4   shared by both slots, mounted at /persist. /home and the
#                    Strata store (/var/lib/strata) are bind mounts from it, so
#                    files and their history survive reboots and updates.
#
# Needs root (mmdebstrap), plus: mmdebstrap mtools dosfstools e2fsprogs fdisk
# systemd-boot-efi, and built morph-boot and strata binaries
# (cargo build --release -p morph-boot -p strata).
set -euo pipefail

WORK=${1:?usage: build.sh <workdir> <good|unhealthy|crash|ransomware>}
SCENARIO=${2:?usage: build.sh <workdir> <good|unhealthy|crash|ransomware>}
case "$SCENARIO" in good|unhealthy|crash|ransomware) ;; *) echo "unknown scenario: $SCENARIO" >&2; exit 2 ;; esac

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
SUITE=${SUITE:-noble}
MIRROR=${MIRROR:-http://archive.ubuntu.com/ubuntu}
MORPH_BOOT=${MORPH_BOOT:-$REPO/target/release/morph-boot}
STRATA=${STRATA:-$REPO/target/release/strata}
SDBOOT_EFI=${SDBOOT_EFI:-/usr/lib/systemd/boot/efi/systemd-bootx64.efi}

[ -x "$MORPH_BOOT" ] || { echo "missing $MORPH_BOOT: run cargo build --release -p morph-boot" >&2; exit 1; }
[ -x "$STRATA" ] || { echo "missing $STRATA: run cargo build --release -p strata" >&2; exit 1; }
[ -f "$SDBOOT_EFI" ] || { echo "missing $SDBOOT_EFI: install systemd-boot-efi" >&2; exit 1; }
mkdir -p "$WORK"

# 1. Base root filesystem (cached across scenarios).
BASE="$WORK/rootfs-base"
if [ ! -f "$BASE/.complete" ]; then
    rm -rf "$BASE"
    mmdebstrap --variant=apt \
        --include=systemd-sysv,udev,kmod,linux-image-virtual,initramfs-tools,systemd-boot,dosfstools \
        --aptopt='APT::Install-Recommends "false"' \
        "$SUITE" "$BASE" \
        "deb $MIRROR $SUITE main universe" \
        "deb $MIRROR $SUITE-updates main universe"
    touch "$BASE/.complete"
fi

# 2. Slot root filesystems: base + MorpheOS overlay (+ test hook, + scenario damage for B).
make_root() {
    local slot=$1 dir="$WORK/root-$1"
    rm -rf "$dir"
    cp -a "$BASE" "$dir"
    rm -f "$dir/.complete"
    cp -a "$REPO/images/common/overlay/." "$dir/"
    cp -a "$HERE/overlay/." "$dir/"
    cp -a "$REPO/images/common/selftest-overlay/." "$dir/"
    install -m 0755 "$MORPH_BOOT" "$dir/usr/bin/morph-boot"
    install -m 0755 "$STRATA" "$dir/usr/bin/strata"
    echo "morpheos-$slot" > "$dir/etc/hostname"
    : > "$dir/etc/machine-id"
    cat > "$dir/etc/fstab" <<EOF
# The system root is mounted read-only from the kernel command line.
PARTLABEL=esp    /efi             vfat   umask=0077,nofail  0  0
tmpfs            /tmp             tmpfs  mode=1777          0  0
# Everything that must outlive an update lives on the shared data partition.
PARTLABEL=data   /persist         ext4   defaults           0  2
/persist/home    /home            none   bind,x-systemd.requires-mounts-for=/persist  0  0
/persist/strata  /var/lib/strata  none   bind,x-systemd.requires-mounts-for=/persist  0  0
EOF
    mkdir -p "$dir/efi" "$dir/persist" "$dir/var/lib/strata"
    for unit in morph-boot-check strata; do
        ln -sf "/usr/lib/systemd/system/$unit.service" \
            "$dir/etc/systemd/system/multi-user.target.wants/$unit.service"
    done
    if [ "$SCENARIO" = ransomware ]; then
        # No update in this scenario: the self-test attacks the files instead.
        ln -sf ransomware-test "$dir/usr/lib/morpheos/test-hook"
    fi
    # Hardware watchdog: a hung kernel or PID 1 gets reset, and a reset ends a trial.
    mkdir -p "$dir/etc/systemd/system.conf.d"
    printf '[Manager]\nRuntimeWatchdogSec=30s\n' > "$dir/etc/systemd/system.conf.d/50-morpheos-watchdog.conf"
    # The image is headless; nothing should wait for a network that isn't there.
    rm -f "$dir/etc/systemd/system/multi-user.target.wants/systemd-networkd-wait-online.service"
    if [ "$slot" = b ] && [ "$SCENARIO" = unhealthy ]; then
        # An "update" that boots but breaks something essential.
        cat > "$dir/usr/lib/systemd/system/morph-broken.service" <<'EOF'
[Unit]
Description=Deliberately broken service (test scenario: unhealthy update)
[Service]
Type=oneshot
ExecStart=/bin/false
[Install]
WantedBy=multi-user.target
EOF
        ln -sf /usr/lib/systemd/system/morph-broken.service \
            "$dir/etc/systemd/system/multi-user.target.wants/morph-broken.service"
    fi
}
make_root a
make_root b

# 3. ESP contents.
ESPDIR="$WORK/esp"
rm -rf "$ESPDIR"
mkdir -p "$ESPDIR/EFI/BOOT" "$ESPDIR/EFI/systemd" "$ESPDIR/loader/entries"
cp "$SDBOOT_EFI" "$ESPDIR/EFI/BOOT/BOOTX64.EFI"
cp "$SDBOOT_EFI" "$ESPDIR/EFI/systemd/systemd-bootx64.efi"
printf 'default morpheos-a.conf\ntimeout 0\nconsole-mode keep\n' > "$ESPDIR/loader/loader.conf"
for slot in a b; do
    root="$WORK/root-$slot"
    mkdir -p "$ESPDIR/morpheos/$slot"
    cp "$(ls "$root"/boot/vmlinuz-* | sort | tail -1)" "$ESPDIR/morpheos/$slot/vmlinuz"
    cp "$(ls "$root"/boot/initrd.img-* | sort | tail -1)" "$ESPDIR/morpheos/$slot/initrd.img"
    extra=""
    if [ "$slot" = b ] && [ "$SCENARIO" = crash ]; then
        # An "update" whose init dies immediately: the kernel panics.
        extra=" init=/bin/false"
    fi
    cat > "$ESPDIR/loader/entries/morpheos-$slot.conf" <<EOF
title   MorpheOS (slot $slot)
linux   /morpheos/$slot/vmlinuz
initrd  /morpheos/$slot/initrd.img
options root=PARTLABEL=root-$slot ro morph.slot=$slot console=ttyS0 panic=5$extra
EOF
done

# 4. The data partition's starting tree: one user's home, and the store's
#    folder, private to the service as Strata::open makes it.
DATADIR="$WORK/data"
rm -rf "$DATADIR"
mkdir -p "$DATADIR/home/morph/Documents" "$DATADIR/strata"
chmod 0700 "$DATADIR/strata"

# 5. Partition images, then the disk. No loop devices or mounts needed.
ESP_MB=300 ROOT_MB=1200 DATA_MB=256
rm -f "$WORK/esp.img"
mkfs.vfat -F 32 -n ESP -C "$WORK/esp.img" $((ESP_MB * 1024)) >/dev/null
mcopy -s -i "$WORK/esp.img" "$ESPDIR"/* ::/
for slot in a b; do
    rm -f "$WORK/root-$slot.img"
    mkfs.ext4 -q -L "root-$slot" -d "$WORK/root-$slot" "$WORK/root-$slot.img" "${ROOT_MB}M"
done
rm -f "$WORK/data.img"
mkfs.ext4 -q -L data -d "$DATADIR" "$WORK/data.img" "${DATA_MB}M"

DISK="$WORK/disk-$SCENARIO.img"
rm -f "$DISK"
truncate -s $((1 + ESP_MB + 2 * ROOT_MB + DATA_MB + 1))M "$DISK"
sfdisk -q "$DISK" <<EOF
label: gpt
start=1MiB, size=${ESP_MB}MiB, type=uefi, name=esp
size=${ROOT_MB}MiB, type=4f68bce3-e8cd-4db1-96e7-fbcaf984b709, name=root-a
size=${ROOT_MB}MiB, type=4f68bce3-e8cd-4db1-96e7-fbcaf984b709, name=root-b
size=${DATA_MB}MiB, type=linux, name=data
EOF
dd if="$WORK/esp.img" of="$DISK" bs=1M seek=1 conv=notrunc status=none
dd if="$WORK/root-a.img" of="$DISK" bs=1M seek=$((1 + ESP_MB)) conv=notrunc status=none
dd if="$WORK/root-b.img" of="$DISK" bs=1M seek=$((1 + ESP_MB + ROOT_MB)) conv=notrunc status=none
dd if="$WORK/data.img" of="$DISK" bs=1M seek=$((1 + ESP_MB + 2 * ROOT_MB)) conv=notrunc status=none
echo "$DISK"
