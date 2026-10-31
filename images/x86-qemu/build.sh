#!/bin/bash
# Build a MorpheOS A/B test disk for a UEFI x86_64 VM.
#
#   images/x86-qemu/build.sh <workdir> <scenario>
#
# scenario: good | unhealthy | crash   (what slot B's "update" does)
#           ransomware                 (no update: slot A attacks its own files,
#                                       and Strata has to get them back)
#           rewind                     (the shell: the self-test attacks its files but does
#                                       NOT rewind; the timeline window opens by itself and
#                                       the harness confirms the rewind, doc 20 S6)
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
#   5 recovery ext4 slot A's full tree (the factory image) plus the recovery units: it
#                    boots into morph-recover, not the shell (S4).
#
# Needs root (mmdebstrap), plus: mmdebstrap mtools dosfstools e2fsprogs fdisk
# systemd-boot-efi, and built morph-boot and strata binaries
# (cargo build --release -p morph-boot -p strata).
set -euo pipefail

WORK=${1:?usage: build.sh <workdir> <good|unhealthy|crash|ransomware>}
SCENARIO=${2:?usage: build.sh <workdir> <good|unhealthy|crash|ransomware>}
case "$SCENARIO" in good|unhealthy|crash|ransomware|plain|shell|browser|wifi|broken-shell|hung-shell|recovery|manifest|autoupdate|rewind|full-disk|config-rollback) ;; *) echo "unknown scenario: $SCENARIO" >&2; exit 2 ;; esac
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
    # `rewind` is a self-test image WITH the shell: the ransomware hook attacks, the timeline
    # window opens by itself, and the harness drives the rewind (doc 20, S6).
    broken-shell|hung-shell|rewind) SHELL_UI=1 ;;
    # The full-disk test (S7): the shell, so the space guard's notice has a display to show
    # on; no recovery partition (it is a self-test image, not a person-facing one).
    full-disk) SHELL_UI=1 ;;
    # The config-rollback test (S8): the shell, so morphd's rollback notice has a display to
    # show on; no recovery partition (it is a self-test image, not a person-facing one).
    config-rollback) SHELL_UI=1 ;;
    # The recovery test (S4): a person-facing image — so it carries the recovery partition —
    # but no shell, and the recovery-test driver instead (see make_root).
    recovery) HARDWARE=1 ;;
esac

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
SUITE=${SUITE:-noble}
MIRROR=${MIRROR:-http://archive.ubuntu.com/ubuntu}
MORPH_BOOT=${MORPH_BOOT:-$REPO/target/release/morph-boot}
STRATA=${STRATA:-$REPO/target/release/strata}
MORPH_UPDATE=${MORPH_UPDATE:-$REPO/target/release/morph-update}
MORPH_HEAL=${MORPH_HEAL:-$REPO/target/release/morph-heal}
MORPH_MANIFEST=${MORPH_MANIFEST:-$REPO/target/release/morph-manifest}
MORPH_D=${MORPH_D:-$REPO/target/release/morphd}
MORPH_AUTOUPDATE=${MORPH_AUTOUPDATE:-$REPO/target/release/morph-autoupdate}
SDBOOT_EFI=${SDBOOT_EFI:-/usr/lib/systemd/boot/efi/systemd-bootx64.efi}
FLOW_COMPOSITOR=${FLOW_COMPOSITOR:-$REPO/target/release/flow-compositor}
SOLID_WINDOW=${SOLID_WINDOW:-$REPO/target/release/solid-window}
FLOW_WIFI=${FLOW_WIFI:-$REPO/target/release/flow-wifi}
FLOW_MOUNT=${FLOW_MOUNT:-$REPO/target/release/flow-mount}
FLOW_TIMELINE=${FLOW_TIMELINE:-$REPO/target/release/flow-timeline}
FLOW_NOTICE=${FLOW_NOTICE:-$REPO/target/release/flow-notice}
if [ "$SHELL_UI" = 1 ]; then
    for b in "$FLOW_COMPOSITOR" "$SOLID_WINDOW" "$FLOW_WIFI" "$FLOW_MOUNT" "$FLOW_TIMELINE" "$FLOW_NOTICE"; do
        [ -x "$b" ] || { echo "missing $b: run cargo build --release -p flow-compositor -p flow-wifi -p flow-files -p flow-timeline -p flow-notice" >&2; exit 1; }
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
# A MACHINE THAT CAN BUILD ON ITSELF. Rob, 2026-10-31: "build yourself properly from the inside your online".
# The image carried git and curl but no compiler, so a Rust toolchain installed into /persist could compile
# and then fail to link: no cc, no ld, no libc6-dev, and rust-lld without binutils is not a real linker.
# gcc + binutils + libc6-dev + make is ~200 MB, which is why it goes in the person-facing images rather
# than every test scenario.
DEV_PKGS=build-essential,pkg-config
BROWSER_PKGS="$BASE_PKGS,$DEV_PKGS,firefox,fonts-dejavu-core,network-manager,wpasupplicant,systemd-resolved,libgl1-mesa-dri,libegl-mesa0,libgbm1,libglx-mesa0,pipewire,pipewire-pulse,wireplumber,alsa-ucm-conf,tailscale"
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

# 1e. The autoupdate base (doc 20, S3): the base has NO network — NetworkManager is only in
# the `browser` and `wifi` bases — so the timer's machine needs its own base. curl and
# ca-certificates are already in BASE_PKGS (the fetch client is rustls, which needs neither,
# but they were in the base before S3). NetworkManager brings the DHCP client the VM's
# user-mode network hands an address to; wpasupplicant is NetworkManager's wifi backend and
# systemd-resolved its DNS. `network-manager` is the package, `network-manager` the service
# this scenario enables below.
AUTOUPDATE_PKGS="$BASE_PKGS,network-manager,wpasupplicant,systemd-resolved"
AUTOUPDATE_KEY="$AUTOUPDATE_PKGS,(linux-modules-extra)"
if [ "$SCENARIO" = autoupdate ]; then
    AUTOUPDATE_BASE="$WORK/rootfs-autoupdate"
    if [ "$(cat "$AUTOUPDATE_BASE/.complete" 2>/dev/null)" != "$AUTOUPDATE_KEY" ]; then
        rm -rf "$AUTOUPDATE_BASE"
        mmdebstrap --variant=apt \
            --include="$AUTOUPDATE_PKGS" \
            --aptopt='APT::Install-Recommends "false"' \
            --customize-hook="$MODULES_EXTRA_HOOK" \
            "$SUITE" "$AUTOUPDATE_BASE" \
            "deb $MIRROR $SUITE main universe multiverse" \
            "deb $MIRROR $SUITE-updates main universe multiverse"
        echo "$AUTOUPDATE_KEY" > "$AUTOUPDATE_BASE/.complete"
    fi
    BASE="$AUTOUPDATE_BASE"
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
[ "$SCENARIO" != autoupdate ] || [ -x "$MORPH_AUTOUPDATE" ] || { echo "missing $MORPH_AUTOUPDATE: build the morph-autoupdate binary first" >&2; exit 1; }
if [ ! -f "$UPDATE_KEY.pub" ]; then
    rm -f "$UPDATE_KEY"
    "$MORPH_UPDATE" keygen --key "$UPDATE_KEY" > /dev/null
    echo "update signing: made a TEST key for this build ($UPDATE_KEY.pub)" >&2
fi

# 2. Slot root filesystems: base + MorpheOS overlay (+ test hook, + scenario damage for B).
# Regenerate the initrd AFTER the overlay has landed: the base's initrd was built by
# mmdebstrap's kernel postinst, before the overlay — which carries the recovery "way in"
# boot script (local-bottom/morpheos-recovery) and the bootctl build hook — existed. Without
# this the hook and bootctl would be missing from the initrd that /boot ships.
regenerate_initrd() {
    local dir=$1 mounted=""
    for m in proc sys dev; do
        if mount --bind "/$m" "$dir/$m" 2>/dev/null; then mounted="$mounted $dir/$m"; fi
    done
    if ! chroot "$dir" update-initramfs -u -k all; then
        echo "update-initramfs failed in $dir: the recovery hook would not reach the initrd" >&2
        for m in $mounted; do umount "$m" 2>/dev/null || true; done
        exit 1
    fi
    for m in $mounted; do umount "$m" 2>/dev/null || true; done
}

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
    if [ "$SCENARIO" = recovery ]; then
        # The recovery test's own driver: arms the recovery entry, or seeds /home for the
        # wipe part. It is a test hook, so it runs from boot-check (see the overlay).
        cp -a "$HERE/overlay-recovery-test/." "$dir/"
    fi
    if [ "$SCENARIO" = manifest ]; then
        # The manifest test's driver (S11): runs `morphd apply` with the mode's Manifest and
        # records the machine's hostname and Strata's policy. A test hook, run from boot-check.
        cp -a "$HERE/overlay-manifest-test/." "$dir/"
    fi
    if [ "$SCENARIO" = config-rollback ]; then
        # The config-rollback test's driver (S8): applies a generation through `morphd`, corrupts
        # its stored file by hand, and records the kernel hostname for the harness to read. A
        # test hook, run from boot-check.
        cp -a "$HERE/overlay-config-rollback/." "$dir/"
    fi
    install -m 0755 "$MORPH_BOOT" "$dir/usr/bin/morph-boot"
    # The updater and the keys it accepts, in the image's own read-only root: an update
    # medium's own binary could check nothing, so the machine verifies with these (S2).
    install -m 0755 "$MORPH_UPDATE" "$dir/usr/bin/morph-update"
    install -D -m 0644 "$UPDATE_KEY.pub" "$dir/usr/share/morpheos/update-keys/build.pub"
    # The unattended-update entry point (doc 20, S3): the timer runs this, in the image's own
    # read-only root, with its own trusted keys.
    if [ "$SCENARIO" = autoupdate ]; then
        install -m 0755 "$MORPH_AUTOUPDATE" "$dir/usr/bin/morph-autoupdate"
        # Only the autoupdate TEST image trusts this throwaway CA (doc 20, S3 note 4). It is
        # generated per run by test-autoupdate.sh and baked in here so the timer's rustls
        # client accepts the test server's certificate; the production unit names no CA.
        if [ -n "${UPDATE_TEST_CA:-}" ]; then
            install -D -m 0644 "$UPDATE_TEST_CA" "$dir/usr/share/morpheos/update-test-ca.pem"
        fi
    fi
    # Which release this image is: morph-update refuses a payload older than it, however
    # well signed, and morph-heal reinstalls exactly it (doc 20, S3).
    install -D -m 0644 /dev/null "$dir/usr/lib/morpheos/release"
    echo "$RELEASE" > "$dir/usr/lib/morpheos/release"
    install -m 0755 "$STRATA" "$dir/usr/bin/strata"
    # The Manifest applier (doc 20, S11): makes the current generation real at every boot.
    install -m 0755 "$MORPH_D" "$dir/usr/bin/morphd"
    # A COMPILER IN THE IMAGE. Rob, 2026-10-31: "build yourself properly from the inside". With git and
    # curl alone a toolchain installed into /persist could compile and then fail at the link — no cc, no
    # ld, no libc6-dev. gcc/binutils/make come in with DEV_PKGS; this fails the build if they did not.
    for tool in /usr/bin/gcc /usr/bin/ld /usr/bin/make; do
        [ -x "$dir$tool" ] || { echo "missing $tool: the machine could not build on itself" >&2; exit 1; }
    done
    # SSH in, so the mesh can drive this machine over the network (E6: no stick, no console
    # person). Root, key only — the mesh admin key is baked into the image.
    mkdir -p "$dir/root/.ssh" "$dir/etc/ssh/sshd_config.d" "$dir/etc/systemd/system/ssh.service.d"
    printf '%s\n' 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJbhcvkozieT1Efw8FGAObkRbiqe+Tm8nMprjOFvp99N harness-mesh admin' \
        > "$dir/root/.ssh/authorized_keys"
    chmod 700 "$dir/root/.ssh"; chmod 600 "$dir/root/.ssh/authorized_keys"
    # ⚠️ ROOT IS THE HOME DIRECTORY sshd CHECKS, NOT JUST .ssh. StrictModes refuses the whole
    # file when the home directory is group- or world-writable, and the refusal looks exactly
    # like "no matching key" to the client: on 2026-10-29 a machine booted from this image
    # answered `Permission denied (publickey)` to the very key baked into it, with the key
    # provably present in the image and byte-identical on the client. The modes are asserted
    # below rather than assumed, and the drop-in pins the settings that matter so a future
    # Ubuntu default cannot quietly change the answer.
    # ⚠️ OWNERSHIP, NOT JUST MODE. StrictModes refuses the key when the home directory is not owned by
    # the account or root, and a build tree can carry a different uid: a KVM guest booted from this very
    # image had /root owned by uid 1001 while /root/.ssh and the key were root's. `chmod 0700` changes
    # nothing about that, and the client sees only "no matching key" — which cost five builds. Proved by
    # changing that one inode and nothing else: the same image then accepted the key (2026-10-30).
    chown 0:0 "$dir/root" "$dir/root/.ssh" "$dir/root/.ssh/authorized_keys" 2>/dev/null || true
    chmod 700 "$dir/root"
    printf '%s\n' \
        '# Pinned, not inherited: the mesh drives this machine over ssh (E6), so the settings' \
        '# that decide whether the baked key is accepted are ours, not the base image s.' \
        'PermitRootLogin prohibit-password' \
        'PubkeyAuthentication yes' \
        'PasswordAuthentication no' \
        'StrictModes yes' \
        'AuthorizedKeysFile .ssh/authorized_keys' \
        > "$dir/etc/ssh/sshd_config.d/morpheos.conf"
    printf '%s\n' \
        '[Service]' \
        '# Ubuntu s ssh.service may carry ProtectHome, which hides /root from the daemon and makes' \
        '# root key auth impossible while looking like a bad key. sshd needs /root to read' \
        '# authorized_keys; nothing else here does.' \
        'ProtectHome=no' \
        > "$dir/etc/systemd/system/ssh.service.d/morpheos.conf"
    # Assert what sshd will check, because a wrong mode here costs a build + a flash + a boot to
    # discover, and the symptom misleads (see above).
    [ "$(stat -c '%a' "$dir/root" 2>/dev/null)" = 700 ] \
        || { echo "root is not 0700: sshd StrictModes would refuse the baked key" >&2; exit 1; }
    # The owner is the other half of StrictModes, and the half that actually bit: mode 0700 with an
    # owner of 1001 refuses every key.
    [ "$(stat -c '%u' "$dir/root" 2>/dev/null)" = 0 ] \
        || { echo "root is not owned by uid 0: sshd StrictModes would refuse the baked key" >&2; exit 1; }
    [ "$(stat -c '%u' "$dir/root/.ssh/authorized_keys" 2>/dev/null)" = 0 ] \
        || { echo "authorized_keys is not owned by uid 0: sshd would refuse it" >&2; exit 1; }
    [ "$(stat -c '%a' "$dir/root/.ssh" 2>/dev/null)" = 700 ] \
        || { echo ".ssh is not 0700: sshd StrictModes would refuse the baked key" >&2; exit 1; }
    [ "$(stat -c '%a' "$dir/root/.ssh/authorized_keys" 2>/dev/null)" = 600 ] \
        || { echo "authorized_keys is not 0600" >&2; exit 1; }
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
# NetworkManager's runtime state and tailscaled's node key live in /var/lib, which the
# read-only root cannot hold. The first real-hardware boot (MacBookPro9,1, 2026-10-28) failed
# here: tailscaled could not start at all — "Failed to set up special execution directory in
# /var/lib: Read-only file system" — three times, and a Wi-Fi join that cannot write its
# runtime state reports success while nothing downstream works. Saved networks are already
# redirected to /persist (conf.d/30-saved-networks-on-persist.conf); this is the same fix for
# the state that only has to last the boot.
tmpfs            /var/lib/NetworkManager      tmpfs  mode=0700  0  0
tmpfs            /var/lib/tailscale           tmpfs  mode=0700  0  0
EOF
    # ⚠️ A SERVICE THAT WRITES TO THE READ-ONLY ROOT FAILS, AND A FAILED UNIT IS AN UNHEALTHY
    # BOOT. systemd-rfkill and systemd-backlight start on any machine with a radio switch or
    # a backlight (every laptop, and no VM until the Wi-Fi test's simulated radios), and logind
    # wherever D-Bus is; each failed to create its state directory. Found by test-wifi.sh.
    mkdir -p "$dir/efi" "$dir/persist" "$dir/var/lib/strata" \
        "$dir/var/lib/systemd/linger" "$dir/var/lib/systemd/rfkill" "$dir/var/lib/systemd/backlight" \
        "$dir/var/lib/NetworkManager" "$dir/var/lib/tailscale"
    # /etc/resolv.conf is written by NetworkManager on every join, and /etc is read-only here:
    # point it at the copy NM writes under /run (conf.d/40-resolv-on-run.conf), or the machine
    # joins a network and resolves nothing. Assert the link, because a plain file left in place
    # would look correct and silently never update.
    rm -f "$dir/etc/resolv.conf"
    ln -sf /run/NetworkManager/resolv.conf "$dir/etc/resolv.conf"
    [ -L "$dir/etc/resolv.conf" ] || { echo "/etc/resolv.conf is not a symlink: DNS would never update" >&2; exit 1; }
    # morphd boot converges the machine to the newest good generation at every start (doc 20,
    # S8/S11) — rolling back a corrupt one and recording the heal. It must be enabled here, or
    # the config rollback never happens (S8). The order in the list does not matter: systemd
    # resolves the Before=/Requires= from the unit files, not from this loop.
    for unit in morphd morph-boot-check strata strata-guard; do
        ln -sf "/usr/lib/systemd/system/$unit.service" \
            "$dir/etc/systemd/system/multi-user.target.wants/$unit.service"
    done
    if [ "$SCENARIO" = autoupdate ]; then
        # The timer's machine: NetworkManager (to get the VM's DHCP address), the timer that
        # runs morph-autoupdate, and the test driver that walks the boots without installing
        # or arming anything (doc 20, S3 note 2). The overlay's own drop-ins set the short
        # test intervals and the test CA path; the CA file was installed above.
        cp -a "$HERE/overlay-network/." "$dir/"
        cp -a "$HERE/overlay-autoupdate/." "$dir/"
        ln -sf /usr/lib/systemd/system/NetworkManager.service \
            "$dir/etc/systemd/system/multi-user.target.wants/NetworkManager.service"
        ln -sf /usr/lib/systemd/system/morph-update-check.timer \
            "$dir/etc/systemd/system/timers.target.wants/morph-update-check.timer"
    fi
    if [ "$SHELL_UI" = 1 ]; then
        cp -a "$HERE/overlay-shell/." "$dir/"
        install -m 0755 "$FLOW_COMPOSITOR" "$dir/usr/bin/flow-compositor"
        install -m 0755 "$SOLID_WINDOW" "$dir/usr/bin/solid-window"
        install -m 0755 "$FLOW_WIFI" "$dir/usr/bin/flow-wifi"
        install -m 0755 "$FLOW_MOUNT" "$dir/usr/bin/flow-mount"
        install -m 0755 "$FLOW_TIMELINE" "$dir/usr/bin/flow-timeline"
        install -m 0755 "$FLOW_NOTICE" "$dir/usr/bin/flow-notice"
        # The shell scenario shows the demonstration window; the browser scenario, Firefox;
        # the wifi scenario, Firefox plus the Wi-Fi picker (E14: once the picker joins,
        # Firefox must come online). Where there is NetworkManager, the picker opens by
        # itself when the machine has no network. The full-disk scenario shows nothing but
        # the space guard's notice, and config-rollback nothing but morphd's rollback notice,
        # so each notice is the first (and only) client window and a screendump reads it
        # unambiguously.
        apps=flow-demo-window
        case "$SCENARIO" in
            plain) apps="flow-browser NetworkManager flow-wifi morpheos-tailscale-up" ;;
            browser) apps="flow-browser NetworkManager flow-wifi" ;;
            wifi) apps="flow-browser NetworkManager flow-wifi morph-wifi-test-ap" ;;
            full-disk) apps="" ;;
            config-rollback) apps="" ;;
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
        for unit in seatd flow-shell flow-notice flow-rollback-notice $apps; do
            ln -sf "/usr/lib/systemd/system/$unit.service" \
                "$dir/etc/systemd/system/multi-user.target.wants/$unit.service"
        done
        # The rewind timeline opens by itself when Strata records an alert (doc 20, S6): the
        # .path unit is enabled at boot, and the .service is NOT — it starts only when the
        # path unit fires, so a machine with nothing to rewind never shows the window.
        ln -sf "/usr/lib/systemd/system/flow-timeline.path" \
            "$dir/etc/systemd/system/multi-user.target.wants/flow-timeline.path"
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
        # The boot report is one snapshot; the live one is a timer, because a Wi-Fi join that
        # half-completes, a second picker window and a browser that dies all happen after it,
        # and the journal is in RAM. A timer rather than a service so it cannot hold the boot
        # open, and its ExecStart carries a `-` so a snapshot that fails can never mark the boot
        # unhealthy and roll an update back (doc 20, E11).
        ln -sf /usr/lib/systemd/system/morph-observe.timer \
            "$dir/etc/systemd/system/timers.target.wants/morph-observe.timer"
        # Enabling is a symlink, which succeeds even when the unit it names is not in the image —
        # and then the stick is silent with nothing saying why. Assert the files are there.
        for f in /usr/lib/morpheos/observe \
                 /usr/lib/systemd/system/morph-observe.service \
                 /usr/lib/systemd/system/morph-observe.timer; do
            [ -e "$dir$f" ] || { echo "missing $f: the live snapshot would never run" >&2; exit 1; }
        done
        # THE FEEDBACK LOOP. A boot can send its own report to the operator, so a diagnosis never has to
        # be carried back on the stick again — the failure that cost five builds on 2026-10-30. Opt-in by
        # a file on the ESP, absent by default, and it never delays or fails a boot.
        ln -sf /usr/lib/systemd/system/morph-phonehome.service \
            "$dir/etc/systemd/system/multi-user.target.wants/morph-phonehome.service"
        for f in /usr/lib/morpheos/phonehome /usr/lib/systemd/system/morph-phonehome.service; do
            [ -e "$dir$f" ] || { echo "missing $f: a boot could not report itself" >&2; exit 1; }
        done
        # THE DOOR THAT CANNOT BE SHUT BY A MODE. sshd takes root's key from a command as well as from
        # /root/.ssh/authorized_keys: ownership, file modes and the home directory cannot refuse it. This
        # is the last-option path — if StrictModes or a bad pw_dir is what refused four builds, this is
        # what opens the door first time.
        install -D -m 0755 "$HERE/overlay-hardware/usr/lib/morpheos/keys-for" \
            "$dir/usr/lib/morpheos/keys-for"
        install -D -m 0644 "$HERE/overlay-hardware/usr/lib/systemd/system/sshd_config.d/50-morpheos-keys.conf" \
            "$dir/etc/ssh/sshd_config.d/50-morpheos-keys.conf"
        for f in /usr/lib/morpheos/keys-for /etc/ssh/sshd_config.d/50-morpheos-keys.conf; do
            [ -e "$dir$f" ] || { echo "missing $f: the door could be shut by a mode again" >&2; exit 1; }
        done
        # And prove sshd accepts it, in the image, before anything is flashed: a drop-in sshd refuses
        # would be a machine with no door at all, and it would only be discovered at boot.
        # sshd -t itself needs /run/sshd (its privilege-separation directory); a chroot has no /run at
        # all, and the failure reads "Missing privilege separation directory", not "bad config".
        mkdir -p "$dir/run/sshd" 2>/dev/null
        # The check needs a host key, and a fresh image deliberately has none — they are generated per
        # machine on first boot, because identical host keys on every stick would be its own defect. So
        # the check makes its own, inside the chroot, and removes it: what is being validated is the
        # config, not the key.
        if ! chroot "$dir" /bin/sh -c 'k=/run/sshd-check; rm -f "$k" "$k.pub";
                ssh-keygen -q -t ed25519 -N "" -f "$k" >/dev/null 2>&1 || exit 1;
                /usr/sbin/sshd -t -h "$k"; rc=$?;
                rm -f "$k" "$k.pub"; exit $rc' 2>/tmp/sshd-t.err; then
            echo "sshd -t failed in the image: $(head -2 /tmp/sshd-t.err)" >&2; exit 1
        fi

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
    if [ "$SCENARIO" = ransomware ] || [ "$SCENARIO" = rewind ]; then
        # No update in these scenarios: the self-test attacks the files instead. `rewind`
        # attacks but does NOT rewind — the timeline window, opened by the path unit, does
        # the rewind once the harness confirms it (doc 20, S6).
        ln -sf ransomware-test "$dir/usr/lib/morpheos/test-hook"
    fi
    if [ "$SCENARIO" = rewind ]; then
        # The flag reaches the test hook through boot-check's environment (boot-check execs
        # the hook). It is the only difference from the `ransomware` scenario's hook.
        mkdir -p "$dir/etc/systemd/system/morph-boot-check.service.d"
        printf '[Service]\nEnvironment=MORPH_TEST_REWIND=no\n' \
            > "$dir/etc/systemd/system/morph-boot-check.service.d/rewind.conf"
    fi
    if [ "$SCENARIO" = full-disk ]; then
        # The full-disk driver (S7): fill /persist until ENOSPC, then let the guard recover.
        cp -a "$HERE/overlay-full-disk/." "$dir/"
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
    # The overlay (recovery hook + bootctl) must be inside the initrd the ESP ships.
    regenerate_initrd "$dir"
}
make_root a
make_root b

# The recovery root (S4): slot A's FULL tree — the factory image — so that when recovery
# rebuilds slot A it is a complete system, plus the recovery units. Built before the ESP so
# its kernel and initrd can be copied into /morpheos/recovery. The recovery BOOT runs only
# morph-recover because its entry boots morph-recovery.target (systemd.unit=…), not
# multi-user.target — the boot services are left IN the tree, they are just never reached.
#
# Built only for the person-facing images (HARDWARE=1): the self-test scenarios do not
# use recovery, and each disk carries a whole extra slot's worth of space, which the CI
# runner's disk does not have to spare (doc 20, S4 note 6). `plain` is the image a
# person boots, so it always carries recovery.
if [ "$HARDWARE" = 1 ]; then
    rm -rf "$WORK/root-recovery"
    cp -a "$WORK/root-a" "$WORK/root-recovery"
    # The recovery root is slot A's FULL tree — every boot service left in — so that when
    # recovery rebuilds slot A (morph-heal recover writes this same tree into both slots),
    # slot A is a complete system that boots and commits, not a husk with the services
    # stripped. The recovery BOOT does not run those services because its entry boots
    # morph-recovery.target (systemd.unit=morph-recovery.target), not multi-user.target;
    # morph-recover.service is Wanted by that target, so it needs no multi-user symlink.
    cp -a "$HERE/overlay-recovery/." "$WORK/root-recovery/"
    echo "morpheos-recovery" > "$WORK/root-recovery/etc/hostname"
    # The recovery root must NOT mount /persist (or its /home and Strata bind mounts): those
    # are the slots' read-write data partition, and recovery leaves it untouched. Assert the
    # recovery fstab actually replaced the slots' one — a silent no-op would ship a recovery
    # that mounts the data partition. The marker is the `PARTLABEL=data` mount itself, not
    # the word "/persist" (which the recovery fstab's own comment also mentions).
    if grep -q 'PARTLABEL=data' "$WORK/root-recovery/etc/fstab"; then
        echo "the recovery root still mounts the data partition — its own fstab did not land" >&2
        exit 1
    fi
fi

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
    if [ "$SCENARIO" = recovery ]; then
        # The in-machine wipe (S4, note 4) writes the mounted slot root from inside the
        # running machine. The kernel refuses that unless it is allowed to write mounted
        # block devices (CONFIG_BLK_DEV_WRITE_MOUNTED); `bdev_allow_write_mounted` is the
        # boot override. Test image only — this is the very thing the scenario proves.
        extra="$extra bdev_allow_write_mounted"
    fi
    cat > "$ESPDIR/loader/entries/morpheos-$slot.conf" <<EOF
title   MorpheOS (slot $slot)
linux   /morpheos/$slot/vmlinuz
initrd  /morpheos/$slot/initrd.img
options root=PARTLABEL=root-$slot ro morph.slot=$slot $console panic=5$extra
EOF
done
# The recovery entry (S4): a one-shot target the boot chain arms when a slot is lost.
# Only on the person-facing images (see the recovery root above).
if [ "$HARDWARE" = 1 ]; then
    mkdir -p "$ESPDIR/morpheos/recovery"
    rkernel=$(ls "$WORK/root-recovery"/boot/vmlinuz-* | sort | tail -1)
    rinitrd=$(ls "$WORK/root-recovery"/boot/initrd.img-* | sort | tail -1)
    # A recovery root with no kernel is a recovery image that cannot boot: fail the build,
    # not silently ship an entry whose vmlinuz never made it into the ESP.
    if [ -z "$rkernel" ] || [ -z "$rinitrd" ]; then
        echo "recovery root has no kernel/initrd in $WORK/root-recovery/boot" >&2
        exit 1
    fi
    cp "$rkernel" "$ESPDIR/morpheos/recovery/vmlinuz"
    cp "$rinitrd" "$ESPDIR/morpheos/recovery/initrd.img"
    cat > "$ESPDIR/loader/entries/morpheos-recovery.conf" <<EOF
title   MorpheOS (recovery)
linux   /morpheos/recovery/vmlinuz
initrd  /morpheos/recovery/initrd.img
options root=PARTLABEL=recovery ro morph.recovery=1 systemd.unit=morph-recovery.target $console panic=5
EOF
fi

# 4. The data partition's starting tree: one user's home, and the store's
#    folder, private to the service as Strata::open makes it.
DATADIR="$WORK/data"
rm -rf "$DATADIR"
mkdir -p "$DATADIR/home/morph/Documents" "$DATADIR/strata" "$DATADIR/morph"
chmod 0700 "$DATADIR/strata"

# Generation 1 (doc 20, S11): the Manifest the machine boots with. `morphd boot` converges to
# it at every start, and a later `morphd apply` needs it as the generation to replace. The
# first generation is not a transaction — it is established here, with the store empty.
"$MORPH_MANIFEST" apply \
    "$HERE/manifest/machine.toml" "$HERE/manifest/machine.toml" \
    --store "$DATADIR/morph" >/dev/null

# 5. Partition images, then the disk. No loop devices or mounts needed.
# ROOT_MB was 1200 until 2026-10-14, when `plain` took Firefox and the check above measured slot
# A at 1134 MiB (94%). 1600 puts it at 71%. Then 2026-10-16, git+curl+ca-certificates (node/dev
# tools) pushed slot A to 1428 MiB (89%); 1800 puts it back to 79%. Each raise is deliberate and
# said in the pull request: a machine installed with one slot size keeps it (doc 20, E7).
# ROOT_MB 1800 -> 2400 on 2026-10-31, deliberately: adding a build toolchain (DEV_PKGS) took slot A to
# 1761 MiB of 1800, past the 85% guard this file itself enforces — and the same guard was about to trip for
# any further addition (audio, identity). A machine installed with one slot size keeps it (E7), so this is
# the moment to raise it: 2400 leaves the toolchain with real headroom, and the stick is 123 GB.
ESP_MB=300 ROOT_MB=2400 DATA_MB=256
# The autoupdate scenario downloads the payload's shrunk root image (~450 MiB — see the size
# build-payload.sh prints) to /persist before installing it, which 256 MiB cannot hold
# (doc 20, S3 note 5). Raised only for that scenario, so the other disks keep their size.
if [ "$SCENARIO" = autoupdate ]; then
    DATA_MB=2048
fi
# A stick-sized image must be able to hold a person's work — and, for the mesh, a toolchain and a work
# tree, because the machine has no other writable place: the root is read-only and /persist is this
# partition. At 256 MiB that is impossible (measured on the first real hardware node, 2026-10-30: "the
# data partition is 268 MB, a Rust toolchain is ~1.5 GB"). The cost is sparse — an empty data partition
# adds almost nothing to the compressed artefact — so the person-facing images get room and the
# self-test scenarios keep their small disks for CI.
if [ "$HARDWARE" = 1 ]; then
    DATA_MB=${MORPH_DATA_MB:-4096}
fi
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
# The recovery root (S4): slot A's tree with every boot service dropped — assembled
# above, before the ESP, so its kernel/initrd reach /morpheos/recovery. It boots into
# morph-recover, not the shell. Built only for the person-facing images (HARDWARE=1),
# which is where the factory reset is reachable.
RECOVERY_MB=0
if [ "$HARDWARE" = 1 ]; then
    rm -f "$WORK/recovery.img"
    mkfs.ext4 -q -L recovery -d "$WORK/root-recovery" "$WORK/recovery.img" "${ROOT_MB}M"
    RECOVERY_MB=$ROOT_MB

    # The recovery image's factory manifest (S4 self-check): morph-heal recover refuses to
    # rebuild from a recovery partition that no longer matches this, so a corrupted recovery
    # image cannot overwrite both slots with corruption. root.img is the recovery partition
    # image itself; vmlinuz/initrd.img are the recovery boot files already on the ESP. The
    # manifest is sealed and signed with UPDATE_KEY — the same build key the slots already
    # trust for updates — and carried beside the recovery kernel on the ESP. e2fsck first, so
    # the journal is clean and the on-disk bytes are exactly what got hashed.
    e2fsck -fy "$WORK/recovery.img" >/dev/null 2>&1 || true
    RPAY="$WORK/recovery-payload"
    rm -rf "$RPAY"; mkdir -p "$RPAY"
    cp "$WORK/recovery.img" "$RPAY/root.img"
    cp "$ESPDIR/morpheos/recovery/vmlinuz" "$RPAY/vmlinuz"
    cp "$ESPDIR/morpheos/recovery/initrd.img" "$RPAY/initrd.img"
    "$MORPH_UPDATE" --payload "$RPAY" --release "$RELEASE" seal >/dev/null
    "$MORPH_UPDATE" --payload "$RPAY" --key "$UPDATE_KEY" sign
    mcopy -i "$WORK/esp.img" "$RPAY/BLAKE3SUMS" "$RPAY/BLAKE3SUMS.sig" "::/morpheos/recovery/"
fi

DISK="$WORK/disk-$SCENARIO.img"
rm -f "$DISK"
truncate -s $((1 + ESP_MB + 2 * ROOT_MB + DATA_MB + RECOVERY_MB + 1))M "$DISK"
{
    echo "label: gpt"
    echo "start=1MiB, size=${ESP_MB}MiB, type=uefi, name=esp"
    echo "size=${ROOT_MB}MiB, type=4f68bce3-e8cd-4db1-96e7-fbcaf984b709, name=root-a"
    echo "size=${ROOT_MB}MiB, type=4f68bce3-e8cd-4db1-96e7-fbcaf984b709, name=root-b"
    echo "size=${DATA_MB}MiB, type=linux, name=data"
    if [ "$HARDWARE" = 1 ]; then
        echo "size=${ROOT_MB}MiB, type=4f68bce3-e8cd-4db1-96e7-fbcaf984b709, name=recovery"
    fi
} | sfdisk -q "$DISK"
dd if="$WORK/esp.img" of="$DISK" bs=1M seek=1 conv=notrunc status=none
# conv=sparse: the disk file is freshly truncated (all zeros), so skipping the filesystems' zero
# blocks writes the same disk and keeps the file sparse, which keeps the CI runner's disk free.
dd if="$WORK/root-a.img" of="$DISK" bs=1M seek=$((1 + ESP_MB)) conv=notrunc,sparse status=none
dd if="$WORK/root-b.img" of="$DISK" bs=1M seek=$((1 + ESP_MB + ROOT_MB)) conv=notrunc,sparse status=none
dd if="$WORK/data.img" of="$DISK" bs=1M seek=$((1 + ESP_MB + 2 * ROOT_MB)) conv=notrunc,sparse status=none
if [ "$HARDWARE" = 1 ]; then
    dd if="$WORK/recovery.img" of="$DISK" bs=1M seek=$((1 + ESP_MB + 2 * ROOT_MB + DATA_MB)) conv=notrunc,sparse status=none
fi
echo "$DISK"
