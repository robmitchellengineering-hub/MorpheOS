#!/bin/bash
# Build a MorpheOS A/B test disk for a UEFI x86_64 VM.
#
#   images/x86-qemu/build.sh <workdir> <scenario>
#
# scenario: good | unhealthy | crash   (what slot B's "update" does)
#           ransomware                 (no update: slot A attacks its own files,
#                                       and Strata has to get them back)
#           plain                      (no self-test at all: a machine to USE, for real
#                                       hardware — see images/x86-qemu/HARDWARE.md)
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
case "$SCENARIO" in good|unhealthy|crash|ransomware|plain) ;; *) echo "unknown scenario: $SCENARIO" >&2; exit 2 ;; esac
# `plain` is the one that leaves the CI harness behind and is meant to be booted by a person.
HARDWARE=0
if [ "$SCENARIO" = plain ]; then HARDWARE=1; fi

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
SUITE=${SUITE:-noble}
MIRROR=${MIRROR:-http://archive.ubuntu.com/ubuntu}
MORPH_BOOT=${MORPH_BOOT:-$REPO/target/release/morph-boot}
STRATA=${STRATA:-$REPO/target/release/strata}
MORPH_UPDATE=${MORPH_UPDATE:-$REPO/target/release/morph-update}
MORPH_HEAL=${MORPH_HEAL:-$REPO/target/release/morph-heal}
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
    if [ "$HARDWARE" = 1 ]; then
        # No self-test driver: nothing here powers the machine off or walks a scenario. The
        # tools the person at the console needs instead, and an autologin so that a machine
        # whose screen is dark can still be reached (see the overlay's own comments).
        cp -a "$HERE/overlay-hardware/." "$dir/"
        for tool in "$MORPH_UPDATE:$dir/usr/bin/morph-update" "$MORPH_HEAL:$dir/usr/bin/morph-heal"; do
            src=${tool%%:*}; dst=${tool#*:}
            if [ -x "$src" ]; then install -m 0755 "$src" "$dst"; fi
        done
    else
        cp -a "$REPO/images/common/selftest-overlay/." "$dir/"
    fi
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
    if [ "$HARDWARE" = 1 ]; then
        ln -sf /usr/lib/systemd/system/morph-banner.service \
            "$dir/etc/systemd/system/multi-user.target.wants/morph-banner.service"
        # The overlay is what makes the console reachable at all. Assert it landed, because a
        # copy that silently did nothing would leave an image that looks right and cannot be
        # logged into — and the CI check would be looking for its own marker.
        [ -f "$dir/root/.profile" ] \
            || { echo "the hardware overlay did not land in $dir" >&2; exit 1; }
        # ⚠️ A VM IS NOT A LAPTOP, AND THE KERNEL IS WHERE THAT BITES.
        #
        # This image is meant to boot from a USB stick on real hardware, and the drivers it
        # needs for that are simply not exercised by QEMU — which presents virtio. Asserting
        # them here is the difference between finding out now and finding out on a dark screen:
        # a kernel flavour can be built without USB storage at all (the `-virtual` flavour
        # exists for VMs, and this image used it).
        # ⚠️ MODULE FILE NAMES DO NOT MATCH MODULE NAMES. The first version of this check asked
        # for `xhci_hcd.ko` when the file is `xhci-hcd.ko`, and CI failed the build for it — which
        # is the check doing its job on the only input it could be wrong about.
        builtin=$(cat "$dir"/usr/lib/modules/*/modules.builtin 2>/dev/null || true)
        missing=""
        for mod in usb-storage uas xhci-hcd ahci sd_mod ext4 vfat; do
            found=$(find "$dir/usr/lib/modules" -name "$mod.ko*" -print -quit 2>/dev/null || true)
            if [ -z "$found" ] && ! printf '%s' "$builtin" | grep -q "/$mod\.ko"; then
                missing="$missing $mod"
            fi
        done
        if [ -n "$missing" ]; then
            echo "the image has no driver for:$missing — a real machine cannot boot from this" >&2
            echo "kernel flavour: $(ls "$dir"/boot/vmlinuz-* 2>/dev/null | xargs -n1 basename)" >&2
            exit 1
        fi
        # The screen, asserted from the kernel's own config instead: `efifb` is built into this
        # kernel, so it has no .ko to look for, and a check that looked for one failed a good
        # image. A dark screen with no serial port is a machine nobody can use.
        config=$(ls "$dir"/boot/config-* 2>/dev/null | tail -1 || true)
        if [ -z "$config" ] || ! grep -qE '^CONFIG_(FB_EFI|DRM_SIMPLEDRM|FB_VESA)=y' "$config"; then
            echo "this kernel has no built-in framebuffer console, so a laptop screen stays dark" >&2
            exit 1
        fi
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
    console="console=ttyS0"
    if [ "$HARDWARE" = 1 ]; then
        # A laptop's screen is the only console anyone is looking at. `tty0` is listed FIRST
        # so the kernel's own messages reach it, while `/dev/console` stays on the serial port
        # (the last one wins) — which keeps this image's behaviour identical to the test
        # images' on the one console CI reads.
        console="console=tty0 console=ttyS0,115200"
    fi
    if [ "$slot" = b ] && [ "$SCENARIO" = crash ]; then
        # An "update" whose init dies immediately: the kernel panics.
        extra=" init=/bin/false"
    fi
    cat > "$ESPDIR/loader/entries/morpheos-$slot.conf" <<EOF
title   MorpheOS (slot $slot)
linux   /morpheos/$slot/vmlinuz
initrd  /morpheos/$slot/initrd.img
options root=PARTLABEL=root-$slot ro morph.slot=$slot $console panic=5$extra
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
