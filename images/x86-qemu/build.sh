#!/bin/bash
# Build a MorpheOS A/B test disk for a UEFI x86_64 VM.
#
#   images/x86-qemu/build.sh <workdir> <scenario>
#
# scenario: good | unhealthy | crash   (what slot B's "update" does)
#           ransomware                 (no update: slot A attacks its own files,
#                                       and Strata has to get them back)
#           plain                      (no self-test at all: a machine to USE, for real
#                                       hardware: the shell, Firefox and the Wi-Fi picker,
#                                       see images/x86-qemu/HARDWARE.md)
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
case "$SCENARIO" in good|unhealthy|crash|ransomware|plain|shell|browser|wifi|broken-shell|hung-shell) ;; *) echo "unknown scenario: $SCENARIO" >&2; exit 2 ;; esac
# `plain` is the one that leaves the CI harness behind and is meant to be booted by a person:
# the Flow shell drawing the screen and taking the keyboard and mouse, Firefox on a local
# start page, NetworkManager, and the Wi-Fi picker when there is no network (doc 20, E4 and
# E5); the root console is on VT2.
# `shell` is the shell with only a demonstration window, for the shell's own tests (E3, E10).
# `browser` is `plain` with Firefox on a test page that then goes to the CI host (E4, E5).
# `wifi` is `shell` with NetworkManager and no wired network: the Wi-Fi picker opens, and the
# kernel's simulated radios (mac80211_hwsim) carry a WPA2 network for it to join (E5). It also
# carries Firefox, so the test can prove it comes online once the picker joins (E14).
HARDWARE=0
SHELL_UI=0
# `broken-shell` and `hung-shell` are self-test images (the scripted A/B walk) WITH the shell:
# slot B's update ships a shell that cannot start, or one that never draws (E11).
case "$SCENARIO" in
    plain|shell|browser|wifi) HARDWARE=1; SHELL_UI=1 ;;
    broken-shell|hung-shell) SHELL_UI=1 ;;
esac

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
FLOW_MOUNT=${FLOW_MOUNT:-$REPO/target/release/flow-mount}
if [ "$SHELL_UI" = 1 ]; then
    for b in "$FLOW_COMPOSITOR" "$SOLID_WINDOW" "$FLOW_WIFI" "$FLOW_MOUNT"; do
        [ -x "$b" ] || { echo "missing $b: run cargo build --release -p flow-compositor -p flow-wifi -p flow-files" >&2; exit 1; }
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
BASE_PKGS=systemd-sysv,udev,kmod,linux-image-virtual,initramfs-tools,systemd-boot,dosfstools,libpixman-1-0,libxkbcommon0,libinput10,libseat1,seatd,pciutils,usbutils,pci.ids,usb.ids,linux-firmware-amd-graphics,linux-firmware-intel-graphics,linux-firmware-intel-wireless,linux-firmware-mediatek,linux-firmware-realtek,linux-firmware-broadcom-wireless,firmware-sof-signed,firmware-b43-installer,openssh-server,git,curl,ca-certificates
MODULES_EXTRA_HOOK='chroot "$1" sh -c "apt-get install -y -q linux-modules-extra-\$(ls /usr/lib/modules | sort -V | tail -1)"'
BASE_KEY="$BASE_PKGS,(linux-modules-extra)"
if [ "$(cat "$BASE/.complete" 2>/dev/null)" != "$BASE_KEY" ]; then
    rm -rf "$BASE"
    mmdebstrap --variant=apt \
        --include="$BASE_PKGS" \
        --aptopt='APT::Install-Recommends "false"' \
        --customize-hook="$MODULES_EXTRA_HOOK" \
        "$SUITE" "$BASE" \
        "deb $MIRROR $SUITE main universe multiverse" \
        "deb $MIRROR $SUITE-updates main universe multiverse"
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
# Tailscale (the mesh's reach layer): shipped from its own APT repository. The key file is
# already de-armored (".noarmor.gpg"), so it is used directly as a keyring — no `gpg --dearmor`.
TAILSCALE_KEY_URL=https://pkgs.tailscale.com/stable/ubuntu/noble.noarmor.gpg
# NetworkManager (A6) with what Wi-Fi needs (wpasupplicant, a "recommends" we would skip) and
# systemd-resolved for DNS: the browser has to reach the network (E5).
# Mesa, because Firefox needs GL to render (WebRender), and the base carries none: with no
# GL it comes up blank — "no Firefox" on a screen that otherwise works (E6, MacBookPro9,1).
BROWSER_PKGS="$BASE_PKGS,firefox,fonts-dejavu-core,network-manager,wpasupplicant,systemd-resolved,libgl1-mesa-dri,libegl-mesa0,libgbm1,libglx-mesa0,pipewire,pipewire-pulse,wireplumber,alsa-ucm-conf,tailscale"
BROWSER_KEY="$BROWSER_PKGS,(linux-modules-extra)"
if [ "$SCENARIO" = browser ] || [ "$SCENARIO" = plain ]; then
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
        ts_key="$WORK/tailscale-keyring.gpg"
        curl -fsSL "$TAILSCALE_KEY_URL" -o "$ts_key"
        mmdebstrap --variant=apt \
            --include="$BROWSER_PKGS" \
            --aptopt='APT::Install-Recommends "false"' \
            --customize-hook="mkdir -p \"\$1/etc/apt/keyrings\" && cp '$keyring' \"\$1/etc/apt/keyrings/packages.mozilla.org.gpg\" && sed -i 's|signed-by=$keyring|signed-by=/etc/apt/keyrings/packages.mozilla.org.gpg|' \"\$1/etc/apt/sources.list\"" \
            --customize-hook="mkdir -p \"\$1/etc/apt/keyrings\" && cp '$ts_key' \"\$1/etc/apt/keyrings/tailscale-keyring.gpg\" && sed -i 's|signed-by=$ts_key|signed-by=/etc/apt/keyrings/tailscale-keyring.gpg|' \"\$1/etc/apt/sources.list\"" \
            --customize-hook="$MODULES_EXTRA_HOOK" \
            --setup-hook='mkdir -p "$1/etc/apt/preferences.d" && printf "Package: *\nPin: origin packages.mozilla.org\nPin-Priority: 1000\n" > "$1/etc/apt/preferences.d/mozilla"' \
            "$SUITE" "$BROWSER_BASE" \
            "deb $MIRROR $SUITE main universe multiverse" \
            "deb $MIRROR $SUITE-updates main universe multiverse" \
            "deb [signed-by=$keyring] https://packages.mozilla.org/apt mozilla main" \
            "deb [signed-by=$ts_key] https://pkgs.tailscale.com/stable/ubuntu noble main"
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
# E14 needs Firefox in this image too (so the test can prove it comes online when the picker
# joins), with its GL deps, plus python3 to serve the test page. It does NOT carry the
# pipewire/audio packages: the test asserts no sound.
WIFI_PKGS="$BASE_PKGS,network-manager,wpasupplicant,systemd-resolved,hostapd,dnsmasq-base,iproute2,firefox,fonts-dejavu-core,libgl1-mesa-dri,libegl-mesa0,libgbm1,libglx-mesa0,python3"
WIFI_KEY="$WIFI_PKGS,(linux-modules-extra)"
if [ "$SCENARIO" = wifi ]; then
    WIFI_BASE="$WORK/rootfs-wifi"
    if [ "$(cat "$WIFI_BASE/.complete" 2>/dev/null)" != "$WIFI_KEY" ]; then
        rm -rf "$WIFI_BASE"
        # Firefox must come from Mozilla's own APT repository, exactly as the browser base
        # gets it: Ubuntu's `firefox` is the snap stub, so the key is fetched, verified
        # against Mozilla's published fingerprint, and the source is pinned to
        # packages.mozilla.org (see the browser base's notes above for the signed-by path).
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
            --include="$WIFI_PKGS" \
            --aptopt='APT::Install-Recommends "false"' \
            --customize-hook="mkdir -p \"\$1/etc/apt/keyrings\" && cp '$keyring' \"\$1/etc/apt/keyrings/packages.mozilla.org.gpg\" && sed -i 's|signed-by=$keyring|signed-by=/etc/apt/keyrings/packages.mozilla.org.gpg|' \"\$1/etc/apt/sources.list\"" \
            --customize-hook="$MODULES_EXTRA_HOOK" \
            --setup-hook='mkdir -p "$1/etc/apt/preferences.d" && printf "Package: *\nPin: origin packages.mozilla.org\nPin-Priority: 1000\n" > "$1/etc/apt/preferences.d/mozilla"' \
            "$SUITE" "$WIFI_BASE" \
            "deb $MIRROR $SUITE main universe multiverse" \
            "deb $MIRROR $SUITE-updates main universe multiverse" \
            "deb [signed-by=$keyring] https://packages.mozilla.org/apt mozilla main"
        # Say which Firefox went in, and prove it is Mozilla's and not the snap stub.
        version=$(chroot "$WIFI_BASE" dpkg-query -W -f='${Version}' firefox)
        [ -x "$WIFI_BASE/usr/lib/firefox/firefox" ] || {
            echo "firefox $version is not Mozilla's build (no /usr/lib/firefox/firefox): the snap stub?" >&2
            exit 1
        }
        echo "wifi base: firefox $version from packages.mozilla.org" >&2
        echo "$WIFI_KEY" > "$WIFI_BASE/.complete"
    fi
    BASE="$WIFI_BASE"
fi

# 1d. Update signing (doc 20, S2). An image accepts an update only if its BLAKE3SUMS is signed
# by a key the image carries. UPDATE_KEY names the secret key whose public half (UPDATE_KEY.pub)
# goes in; without one, a TEST key is made for this build, in the work directory, and
# build-payload.sh signs with the same one. ⚠️ A real release key is Rob's to hold: never
# commit one, and never let an image a person uses trust a key that lives in CI.
UPDATE_KEY=${UPDATE_KEY:-$WORK/update-test.key}
# The release number this image is (doc 20, S3). Payloads say theirs; nothing older installs.
RELEASE=${RELEASE:-1}
case "$RELEASE" in ''|*[!0-9]*) echo "RELEASE must be a number, not '$RELEASE'" >&2; exit 2 ;; esac
[ -x "$MORPH_UPDATE" ] || { echo "missing $MORPH_UPDATE: run cargo build --release -p morph-update" >&2; exit 1; }
if [ ! -f "$UPDATE_KEY.pub" ]; then
    rm -f "$UPDATE_KEY"
    "$MORPH_UPDATE" keygen --key "$UPDATE_KEY" > /dev/null
    echo "update signing: made a TEST key for this build ($UPDATE_KEY.pub)" >&2
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
        if [ -x "$MORPH_HEAL" ]; then install -m 0755 "$MORPH_HEAL" "$dir/usr/bin/morph-heal"; fi
    else
        cp -a "$REPO/images/common/selftest-overlay/." "$dir/"
    fi
    install -m 0755 "$MORPH_BOOT" "$dir/usr/bin/morph-boot"
    # The updater and the keys it accepts, in the image's own read-only root: an update
    # medium's own binary could check nothing, so the machine verifies with these (S2).
    install -m 0755 "$MORPH_UPDATE" "$dir/usr/bin/morph-update"
    install -D -m 0644 "$UPDATE_KEY.pub" "$dir/usr/share/morpheos/update-keys/build.pub"
    # Which release this image is: morph-update refuses a payload older than it, however
    # well signed, and morph-heal reinstalls exactly it (doc 20, S3).
    install -D -m 0644 /dev/null "$dir/usr/lib/morpheos/release"
    echo "$RELEASE" > "$dir/usr/lib/morpheos/release"
    install -m 0755 "$STRATA" "$dir/usr/bin/strata"
    # SSH in, so the mesh can drive this machine over the network (E6: no stick, no console
    # person). Root, key only — the mesh admin key is baked into the image.
    mkdir -p "$dir/root/.ssh" "$dir/etc/ssh/sshd_config.d"
    printf '%s\n' 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJbhcvkozieT1Efw8FGAObkRbiqe+Tm8nMprjOFvp99N harness-mesh admin' \
        > "$dir/root/.ssh/authorized_keys"
    chmod 700 "$dir/root/.ssh"; chmod 600 "$dir/root/.ssh/authorized_keys"
    printf 'PermitRootLogin prohibit-password\nPasswordAuthentication no\n' > "$dir/etc/ssh/sshd_config.d/morpheos.conf"
    ln -sf /usr/lib/systemd/system/ssh.service "$dir/etc/systemd/system/multi-user.target.wants/ssh.service"
    # The 2012 MacBook Pro's BCM4331 Wi-Fi (E6): the `b43` driver is the one that works with
    # this chip; its firmware comes from `firmware-b43-installer` (multiverse), asserted below.
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
        install -m 0755 "$FLOW_MOUNT" "$dir/usr/bin/flow-mount"
        # The shell scenario shows the demonstration window; the browser scenario, Firefox;
        # the wifi scenario, Firefox plus the Wi-Fi picker (E14: once the picker joins,
        # Firefox must come online). Where there is NetworkManager, the picker opens by
        # itself when the machine has no network.
        apps=flow-demo-window
        case "$SCENARIO" in
            plain) apps="flow-browser NetworkManager flow-wifi morpheos-tailscale-up" ;;
            browser) apps="flow-browser NetworkManager flow-wifi" ;;
            wifi) apps="flow-browser NetworkManager flow-wifi morph-wifi-test-ap" ;;
        esac
        case "$SCENARIO" in plain|browser|wifi) cp -a "$HERE/overlay-network/." "$dir/" ;; esac
        case "$SCENARIO" in plain|browser|wifi) cp -a "$HERE/overlay-browser/." "$dir/" ;; esac
        # What only the browser TEST needs (its page, and the drop-in that opens it).
        if [ "$SCENARIO" = browser ]; then cp -a "$HERE/overlay-browser-test/." "$dir/"; fi
        if [ "$SCENARIO" = wifi ]; then
            cp -a "$HERE/overlay-wifi-test/." "$dir/"
            # The package's own hostapd service is not the test's access point: keep it off.
            ln -sf /dev/null "$dir/etc/systemd/system/hostapd.service"
        fi
        for unit in seatd flow-shell $apps; do
            ln -sf "/usr/lib/systemd/system/$unit.service" \
                "$dir/etc/systemd/system/multi-user.target.wants/$unit.service"
        done
        # tailscaled ships at /lib/systemd/system (the Debian convention), which is
        # /usr/lib/systemd/system on a usrmerged image. Link to whichever the package
        # actually created, and fail loudly if neither does — a silently broken link is
        # exactly how it did not start on the reference laptop.
        if [ "$SCENARIO" = plain ]; then
            ts_unit=""
            for p in "$dir/usr/lib/systemd/system/tailscaled.service" "$dir/lib/systemd/system/tailscaled.service"; do
                if [ -f "$p" ]; then ts_unit="${p#$dir}"; break; fi
            done
            [ -n "$ts_unit" ] || { echo "tailscaled.service is missing from the image: the tailscale package did not install it" >&2; exit 1; }
            ln -sf "$ts_unit" "$dir/etc/systemd/system/multi-user.target.wants/tailscaled.service"
        fi
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
            amdgpu i915 xe cfg80211 mac80211 iwlwifi mt7921e brcmfmac b43 hid-multitouch r8169; do
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
        # The firmware b43 loads for the BCM4331 (MacBookPro9,1). Assert it landed, not
        # just that the module did: a missing .fw is the exact "no Wi-Fi" the hardware report
        # found, and it reads as green until a person boots it.
        [ -f "$dir/lib/firmware/b43/ucode29_mimo.fw" ] \
            || { echo "b43 firmware is missing from $dir: BCM4331 Wi-Fi cannot start" >&2; exit 1; }
        # The screen, asserted from the kernel's own config instead: `efifb` is built into this
        # kernel, so it has no .ko to look for, and a check that looked for one failed a good
        # image. A dark screen with no serial port is a machine nobody can use.
        config=$(ls "$dir"/boot/config-* 2>/dev/null | tail -1 || true)
        if [ -z "$config" ] || ! grep -qE '^CONFIG_(FB_EFI|DRM_SIMPLEDRM|FB_VESA)=y' "$config"; then
            echo "this kernel has no built-in framebuffer console, so a laptop screen stays dark" >&2
            exit 1
        fi
        # Why the shell picks its card (`--drm auto`, crates/flow-compositor/src/display.rs):
        # with simpledrm built in, the firmware framebuffer is card0 and the real driver card1.
        if grep -q '^CONFIG_DRM_SIMPLEDRM=y' "$config"; then
            echo "kernel: simpledrm is built in, so a laptop's graphics driver is not card0" >&2
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
    if [ "$slot" = b ] && [ "$SCENARIO" = broken-shell ]; then
        # An update whose shell cannot start: its compositor exits at once.
        printf '#!/bin/sh\necho "flow-compositor: deliberately broken (test scenario: broken-shell)" >&2\nexit 1\n' \
            > "$dir/usr/bin/flow-compositor"
        chmod 0755 "$dir/usr/bin/flow-compositor"
    fi
    if [ "$slot" = b ] && [ "$SCENARIO" = hung-shell ]; then
        # An update whose shell starts and never draws: the process runs, nothing is shown.
        printf '#!/bin/sh\necho "flow-compositor: deliberately hung (test scenario: hung-shell)" >&2\nexec sleep infinity\n' \
            > "$dir/usr/bin/flow-compositor"
        chmod 0755 "$dir/usr/bin/flow-compositor"
    fi
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
# ROOT_MB was 1200 until 2026-10-14, when `plain` took Firefox and the check above measured slot
# A at 1134 MiB (94%). 1600 puts it at 71%. Then 2026-10-16, git+curl+ca-certificates (node/dev
# tools) pushed slot A to 1428 MiB (89%); 1800 puts it back to 79%. Each raise is deliberate and
# said in the pull request: a machine installed with one slot size keeps it (doc 20, E7).
ESP_MB=300 ROOT_MB=1800 DATA_MB=256
rm -f "$WORK/esp.img"
mkfs.vfat -F 32 -n ESP -C "$WORK/esp.img" $((ESP_MB * 1024)) >/dev/null
mcopy -s -i "$WORK/esp.img" "$ESPDIR"/* ::/
for slot in a b; do
    # The slot's size is fixed when a machine is installed (E7), so a system that has grown
    # into its slot is found here, at build time, not by an update that no longer fits.
    used=$(du -sm "$WORK/root-$slot" | cut -f1)
    echo "slot $slot: ${used} MiB of ${ROOT_MB} MiB" >&2
    if [ "$used" -gt $((ROOT_MB * 85 / 100)) ]; then
        echo "slot $slot's system (${used} MiB) is over 85% of ROOT_MB (${ROOT_MB} MiB): raise ROOT_MB on purpose, and say so" >&2
        exit 1
    fi
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
# conv=sparse: the disk file is freshly truncated (all zeros), so skipping the filesystems' zero
# blocks writes the same disk and keeps the file sparse, which keeps the CI runner's disk free.
dd if="$WORK/root-a.img" of="$DISK" bs=1M seek=$((1 + ESP_MB)) conv=notrunc,sparse status=none
dd if="$WORK/root-b.img" of="$DISK" bs=1M seek=$((1 + ESP_MB + ROOT_MB)) conv=notrunc,sparse status=none
dd if="$WORK/data.img" of="$DISK" bs=1M seek=$((1 + ESP_MB + 2 * ROOT_MB)) conv=notrunc,sparse status=none
echo "$DISK"
