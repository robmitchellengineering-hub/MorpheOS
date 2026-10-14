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
case "$SCENARIO" in good|unhealthy|crash|ransomware|plain|shell|browser|wifi) ;; *) echo "unknown scenario: $SCENARIO" >&2; exit 2 ;; esac
# `plain` is the one that leaves the CI harness behind and is meant to be booted by a person.
# `shell` is `plain` with the Flow shell drawing the screen and taking the keyboard and mouse
# (doc 20, E3 and E10), and the root console moved to VT2. It stays separate from `plain`
# until there is something to do in the shell (a browser, E4): until then, the person
# booting `plain` on a laptop is better served by the root console it opens on.
# `browser` is `shell` with Firefox open on a test page instead of the demonstration window
# (doc 20, E4).
# `wifi` is `shell` with NetworkManager and no wired network: the Wi-Fi picker opens, and the
# kernel's simulated radios (mac80211_hwsim) carry a WPA2 network for it to join (E5). No
# Firefox, so it builds without Mozilla's servers.
HARDWARE=0
SHELL_UI=0
case "$SCENARIO" in plain) HARDWARE=1 ;; shell|browser|wifi) HARDWARE=1; SHELL_UI=1 ;; esac

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
SUITE=${SUITE:-noble}
MIRROR=${MIRROR:-http://archive.ubuntu.com/ubuntu}
MORPH_BOOT=${MORPH_BOOT:-$REPO/target/release/morph-boot}
STRATA=${STRATA:-$REPO/target/release/strata}
MORPH_UPDATE=${MORPH_UPDATE:-$REPO/target/release/morph-update}
MORPH_HEAL=${MORPH_HEAL:-$REPO/target/release/morph-heal}
SDBOOT_EFI=${SDBOOT_EFI:-/usr/lib/systemd/boot/efi/systemd-bootx64.efi}
FLOW_COMPOSITOR=${FLOW_COMPOSITOR:-$REPO/target/release/flow-compositor}
SOLID_WINDOW=${SOLID_WINDOW:-$REPO/target/release/solid-window}
FLOW_WIFI=${FLOW_WIFI:-$REPO/target/release/flow-wifi}
if [ "$SHELL_UI" = 1 ]; then
    for b in "$FLOW_COMPOSITOR" "$SOLID_WINDOW" "$FLOW_WIFI"; do
        [ -x "$b" ] || { echo "missing $b: run cargo build --release -p flow-compositor -p flow-wifi" >&2; exit 1; }
    done
fi

[ -x "$MORPH_BOOT" ] || { echo "missing $MORPH_BOOT: run cargo build --release -p morph-boot" >&2; exit 1; }
[ -x "$STRATA" ] || { echo "missing $STRATA: run cargo build --release -p strata" >&2; exit 1; }
[ -f "$SDBOOT_EFI" ] || { echo "missing $SDBOOT_EFI: install systemd-boot-efi" >&2; exit 1; }
mkdir -p "$WORK"

# 1. Base root filesystem (cached across scenarios).
# The cache is keyed on the package list, so adding a package rebuilds it.
# libpixman-1-0 and libxkbcommon0 are what flow-compositor links against; libinput10 and
# libseat1 its keyboard, mouse and seat session (E10), and seatd the seat manager it uses.
# pciutils and usbutils for the hardware report (with pci.ids and usb.ids, so devices are named,
# not just numbered), and the laptop firmware sets (E12): AMD,
# Intel and MediaTek graphics and Wi-Fi, Realtek, Broadcom, and Intel audio (~250 MB), so a
# first boot on a real machine shows what works, not only what is missing.
# ⚠️ THE KERNEL'S DRIVERS ARE IN TWO PACKAGES. `linux-image-virtual` brings the kernel and
# `linux-modules`, which is enough for a VM and has NO laptop graphics (amdgpu, i915, xe), NO
# Wi-Fi (iwlwifi, MediaTek, Broadcom), no touchscreen and no Realtek ethernet: those are in
# `linux-modules-extra-<version>`. `linux-image-generic` would pull it, and with it every
# firmware set Ubuntu has (~600 MB). So the base takes the extra modules for the kernel it
# installed, by name, in a hook; the firmware stays the chosen sets below. The extra modules
# also carry mac80211_hwsim, the simulated Wi-Fi the E5 test connects through.
BASE="$WORK/rootfs-base"
BASE_PKGS=systemd-sysv,udev,kmod,linux-image-virtual,initramfs-tools,systemd-boot,dosfstools,libpixman-1-0,libxkbcommon0,libinput10,libseat1,seatd,pciutils,usbutils,pci.ids,usb.ids,linux-firmware-amd-graphics,linux-firmware-intel-graphics,linux-firmware-intel-wireless,linux-firmware-mediatek,linux-firmware-realtek,linux-firmware-broadcom-wireless,firmware-sof-signed
MODULES_EXTRA_HOOK='chroot "$1" sh -c "apt-get install -y -q linux-modules-extra-\$(ls /usr/lib/modules | sort -V | tail -1)"'
BASE_KEY="$BASE_PKGS,(linux-modules-extra)"
if [ "$(cat "$BASE/.complete" 2>/dev/null)" != "$BASE_KEY" ]; then
    rm -rf "$BASE"
    mmdebstrap --variant=apt \
        --include="$BASE_PKGS" \
        --aptopt='APT::Install-Recommends "false"' \
        --customize-hook="$MODULES_EXTRA_HOOK" \
        "$SUITE" "$BASE" \
        "deb $MIRROR $SUITE main universe" \
        "deb $MIRROR $SUITE-updates main universe"
    echo "$BASE_KEY" > "$BASE/.complete"
fi

# 1b. The browser base: the base plus Firefox from Mozilla's own APT repository (doc 20, E4),
# cached on its own, so only the scenarios that carry a browser need Mozilla's servers.
# ⚠️ Ubuntu's own `firefox` package is a stub that installs the snap, and it has an epoch
# (1:...) that outranks Mozilla's version, so apt is pinned to prefer packages.mozilla.org.
# The signing key is fetched at build time and MUST have the fingerprint Mozilla publishes:
# a key that does not match stops the build rather than trusting whatever was served.
# ⚠️ WHERE APT LOOKS FOR THE KEY. mmdebstrap runs apt from the BUILD machine, so a `signed-by=`
# path is read there, not in the image; and a source line WITHOUT one gets a `signed-by` that
# mmdebstrap picks itself (the Ubuntu keyring), so a key dropped into the image's
# trusted.gpg.d is never consulted. Both failed in CI with NO_PUBKEY. So the line names the key
# at its build-machine path, and a customize hook copies the key into the image and points
# the image's own sources.list at that copy.
MOZILLA_KEY_URL=https://packages.mozilla.org/apt/repo-signing-key.gpg
MOZILLA_FPR=35BAA0B33E9EB396F59CA838C0BA5CE6DC6315A3
# NetworkManager (A6) with what Wi-Fi needs (wpasupplicant, a "recommends" we would skip) and
# systemd-resolved for DNS: the browser has to reach the network (E5).
BROWSER_PKGS="$BASE_PKGS,firefox,fonts-dejavu-core,network-manager,wpasupplicant,systemd-resolved"
BROWSER_KEY="$BROWSER_PKGS,(linux-modules-extra)"
if [ "$SCENARIO" = browser ]; then
    BROWSER_BASE="$WORK/rootfs-browser"
    if [ "$(cat "$BROWSER_BASE/.complete" 2>/dev/null)" != "$BROWSER_KEY" ]; then
        rm -rf "$BROWSER_BASE"
        key="$WORK/packages.mozilla.org.asc"
        curl -fsSL "$MOZILLA_KEY_URL" -o "$key"
        fpr=$(gpg --show-keys --with-colons "$key" 2>/dev/null | awk -F: '$1 == "fpr" {print $10; exit}')
        if [ "$fpr" != "$MOZILLA_FPR" ]; then
            echo "Mozilla's signing key has fingerprint '$fpr', not $MOZILLA_FPR: refusing it" >&2
            exit 1
        fi
        keyring="$WORK/packages.mozilla.org.gpg"
        gpg --dearmor < "$key" > "$keyring"
        mmdebstrap --variant=apt \
            --include="$BROWSER_PKGS" \
            --aptopt='APT::Install-Recommends "false"' \
            --customize-hook="mkdir -p \"\$1/etc/apt/keyrings\" && cp '$keyring' \"\$1/etc/apt/keyrings/packages.mozilla.org.gpg\" && sed -i 's|signed-by=$keyring|signed-by=/etc/apt/keyrings/packages.mozilla.org.gpg|' \"\$1/etc/apt/sources.list\"" \
            --customize-hook="$MODULES_EXTRA_HOOK" \
            --setup-hook='mkdir -p "$1/etc/apt/preferences.d" && printf "Package: *\nPin: origin packages.mozilla.org\nPin-Priority: 1000\n" > "$1/etc/apt/preferences.d/mozilla"' \
            "$SUITE" "$BROWSER_BASE" \
            "deb $MIRROR $SUITE main universe" \
            "deb $MIRROR $SUITE-updates main universe" \
            "deb [signed-by=$keyring] https://packages.mozilla.org/apt mozilla main"
        # Say which Firefox went in, and prove it is Mozilla's and not the snap stub.
        version=$(chroot "$BROWSER_BASE" dpkg-query -W -f='${Version}' firefox)
        [ -x "$BROWSER_BASE/usr/lib/firefox/firefox" ] || {
            echo "firefox $version is not Mozilla's build (no /usr/lib/firefox/firefox): the snap stub?" >&2
            exit 1
        }
        echo "browser base: firefox $version from packages.mozilla.org" >&2
        echo "$BROWSER_KEY" > "$BROWSER_BASE/.complete"
    fi
    BASE="$BROWSER_BASE"
fi

# 1c. The Wi-Fi test base: the base, NetworkManager as the browser base has it, and the other
# end of the simulated network: hostapd (the access point), dnsmasq (its DHCP server) and
# iproute2 (to address it). Test-only; the access point runs only in the `wifi` scenario.
WIFI_PKGS="$BASE_PKGS,network-manager,wpasupplicant,systemd-resolved,hostapd,dnsmasq-base,iproute2"
WIFI_KEY="$WIFI_PKGS,(linux-modules-extra)"
if [ "$SCENARIO" = wifi ]; then
    WIFI_BASE="$WORK/rootfs-wifi"
    if [ "$(cat "$WIFI_BASE/.complete" 2>/dev/null)" != "$WIFI_KEY" ]; then
        rm -rf "$WIFI_BASE"
        mmdebstrap --variant=apt \
            --include="$WIFI_PKGS" \
            --aptopt='APT::Install-Recommends "false"' \
            --customize-hook="$MODULES_EXTRA_HOOK" \
            "$SUITE" "$WIFI_BASE" \
            "deb $MIRROR $SUITE main universe" \
            "deb $MIRROR $SUITE-updates main universe"
        echo "$WIFI_KEY" > "$WIFI_BASE/.complete"
    fi
    BASE="$WIFI_BASE"
fi

# 2. Slot root filesystems: base + MorpheOS overlay (+ test hook, + scenario damage for B).
make_root() {
    local slot=$1 dir="$WORK/root-$1"
    rm -rf "$dir"
    cp -a "$BASE" "$dir"
    rm -f "$dir/.complete"
    cp -a "$REPO/images/common/overlay/." "$dir/"
    cp -a "$HERE/overlay/." "$dir/"
    # The base's /etc/pam.d/login prints the Ubuntu legal notice TWICE at login. It carries
    # two motd lines: `pam_motd.so motd=/run/motd.dynamic` (the welcome + help + legal notice,
    # kept) and `pam_motd.so noupdate` (the DEFAULT motd). Because /etc/motd was never created
    # — mmdebstrap's `--variant=apt` skips the base-files postinst that makes it — the default
    # falls back to the distributor motd, which is the legal notice again, so it prints twice.
    # Remove the `noupdate` line and assert BOTH halves of the result: the duplicate is gone
    # and the dynamic-motd line survived. A silent no-op here would leave the doubled notice
    # in place, which is the failure mode the assertion is for.
    sed -i '/pam_motd\.so[[:space:]]*noupdate/d' "$dir/etc/pam.d/login"
    grep -q 'pam_motd\.so[[:space:]][[:space:]]*motd=/run/motd\.dynamic' "$dir/etc/pam.d/login" \
        || { echo "$dir/etc/pam.d/login lost its dynamic-motd line" >&2; exit 1; }
    if grep -q 'pam_motd\.so[[:space:]][[:space:]]*noupdate' "$dir/etc/pam.d/login"; then
        echo "$dir/etc/pam.d/login still prints the default motd — the legal notice would print twice" >&2
        exit 1
    fi
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
# State systemd's own services keep in /var/lib/systemd (their StateDirectory), which the
# read-only root cannot hold: logind (lingering users), the radio switches (rfkill) and the
# screen brightness (backlight). Kept for the boot only.
tmpfs            /var/lib/systemd/linger     tmpfs  mode=0755  0  0
tmpfs            /var/lib/systemd/rfkill     tmpfs  mode=0755  0  0
tmpfs            /var/lib/systemd/backlight  tmpfs  mode=0755  0  0
EOF
    # ⚠️ A SERVICE THAT WRITES TO THE READ-ONLY ROOT FAILS, AND A FAILED UNIT IS AN UNHEALTHY
    # BOOT. systemd-rfkill and systemd-backlight start on any machine with a radio switch or
    # a backlight (every laptop, and no VM until the Wi-Fi test's simulated radios), and logind
    # wherever D-Bus is; each failed to create its state directory. Found by test-wifi.sh.
    mkdir -p "$dir/efi" "$dir/persist" "$dir/var/lib/strata" \
        "$dir/var/lib/systemd/linger" "$dir/var/lib/systemd/rfkill" "$dir/var/lib/systemd/backlight"
    for unit in morph-boot-check strata; do
        ln -sf "/usr/lib/systemd/system/$unit.service" \
            "$dir/etc/systemd/system/multi-user.target.wants/$unit.service"
    done
    if [ "$SHELL_UI" = 1 ]; then
        cp -a "$HERE/overlay-shell/." "$dir/"
        install -m 0755 "$FLOW_COMPOSITOR" "$dir/usr/bin/flow-compositor"
        install -m 0755 "$SOLID_WINDOW" "$dir/usr/bin/solid-window"
        install -m 0755 "$FLOW_WIFI" "$dir/usr/bin/flow-wifi"
        # The shell scenario shows the demonstration window; the browser scenario, Firefox;
        # the wifi scenario, only the Wi-Fi picker. Where there is NetworkManager, the picker
        # opens by itself when the machine has no network.
        apps=flow-demo-window
        case "$SCENARIO" in
            browser) apps="flow-browser NetworkManager flow-wifi" ;;
            wifi) apps="NetworkManager flow-wifi morph-wifi-test-ap" ;;
        esac
        case "$SCENARIO" in browser|wifi) cp -a "$HERE/overlay-network/." "$dir/" ;; esac
        if [ "$SCENARIO" = browser ]; then cp -a "$HERE/overlay-browser/." "$dir/"; fi
        if [ "$SCENARIO" = wifi ]; then
            cp -a "$HERE/overlay-wifi-test/." "$dir/"
            # The package's own hostapd service is not the test's access point: keep it off.
            ln -sf /dev/null "$dir/etc/systemd/system/hostapd.service"
        fi
        for unit in seatd flow-shell $apps; do
            ln -sf "/usr/lib/systemd/system/$unit.service" \
                "$dir/etc/systemd/system/multi-user.target.wants/$unit.service"
        done
        # The root console lives on VT2 while the shell has VT1 (Ctrl+Alt+F2 reaches it).
        # Started at boot under its own name, so its drop-in applies (logind's on-demand
        # `autovt@tty2` would not pick up a `getty@tty2` drop-in).
        mkdir -p "$dir/etc/systemd/system/getty.target.wants"
        ln -sf /usr/lib/systemd/system/getty@.service \
            "$dir/etc/systemd/system/getty.target.wants/getty@tty2.service"
        [ -f "$dir/usr/lib/systemd/system/seatd.service" ] \
            || { echo "seatd is not installed in $dir: the shell could not take the seat" >&2; exit 1; }
    fi
    if [ "$HARDWARE" = 1 ]; then
        for unit in morph-banner morph-hwreport; do
            ln -sf "/usr/lib/systemd/system/$unit.service" \
                "$dir/etc/systemd/system/multi-user.target.wants/$unit.service"
        done
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
        # And what a laptop needs once booted: its graphics, its Wi-Fi (and the stack under
        # it), its touchscreen, its wired network. Each of these was missing until the extra
        # modules went in, with the firmware for them already on the image.
        for mod in usb-storage uas xhci-hcd ahci sd_mod ext4 vfat \
            amdgpu i915 xe cfg80211 mac80211 iwlwifi mt7921e brcmfmac hid-multitouch r8169; do
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
