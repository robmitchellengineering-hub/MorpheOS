#!/bin/bash
# boot-in-vm.sh — put a kernel we built into a copy of a MorpheOS image and boot it under KVM.
#
# Phase 0/1 of PLAN-morpheos-own-kernel. Run from this Mac; it uses pacland-rog as the VM host and the
# KVM node's port forward for ssh.
#
#   secure/kernel/boot-in-vm.sh <image.img> [port] [entry-name]
#
# WHAT IT DOES, and why each step exists (all learned the hard way on 2026-10-31):
#   1. takes the kernel the build produced on the host (/tmp/kbuild/linux-6.8/arch/x86/boot/bzImage);
#   2. extracts the image's ESP (sector 2048, 614400 sectors), adds /morpheos/our/vmlinuz and a
#      loader entry whose options are the SAME as slot a's (root=PARTLABEL=root-a ro morph.slot=a);
#   3. VERIFIES the kernel is in the ESP by size before anything boots — an scp that closes early once
#      produced an entry pointing at a kernel that was never there;
#   4. writes the ESP into a DISK COPY on the node (dd at offset 2048), so the original image is untouched
#      and no hardware is flashed;
#   5. boots it with a FRESH UEFI variable store (adding devices changes the boot path; a stale store
#      drops the VM into the EFI shell).
set -uo pipefail

IMG=${1:?usage: boot-in-vm.sh <image.img> [port] [entry-name]}
PORT=${2:-2303}
ENTRY=${3:-morpheos-our.conf}
NODE=${MORPH_VM_NODE:-rob@192.168.15.18}
KEY=$HOME/.ssh/mesh-admin
O=(-o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=10 -o IdentitiesOnly=yes)
say() { printf '%s\n' "$*"; }

[ -f "$IMG" ] || { say "no image at $IMG"; exit 1; }

say "1/5 fetching the kernel we built on the node"
ssh -i "$KEY" "${O[@]}" "$NODE" 'cat /tmp/kbuild/linux-6.8/arch/x86/boot/bzImage' > /tmp/bzImage-our 2>/dev/null
[ -s /tmp/bzImage-our ] || { say "   no kernel at /tmp/kbuild/linux-6.8/arch/x86/boot/bzImage on $NODE"; exit 2; }
say "   $(stat -f%z /tmp/bzImage-our) bytes"

say "2/5 building an ESP with our kernel in it"
dd if="$IMG" of=/tmp/esp-boot.img bs=512 skip=2048 count=614400 status=none
ATT=$(hdiutil attach /tmp/esp-boot.img 2>/dev/null | tail -1)
MP=$(printf '%s\n' "$ATT" | awk '{print $NF}')
ND=$(printf '%s\n' "$ATT" | awk '{print $1}')
[ -d "$MP" ] || { say "   could not mount the ESP"; exit 3; }
mkdir -p "$MP/morpheos/our"
cp /tmp/bzImage-our "$MP/morpheos/our/vmlinuz"
printf 'title   MorpheOS (our kernel)\nlinux   /morpheos/our/vmlinuz\noptions root=PARTLABEL=root-a ro morph.slot=a console=tty0 console=ttyS0,115200 panic=5\n' > "$MP/loader/entries/$ENTRY"
printf 'default %s\ntimeout 0\nconsole-mode keep\n' "$ENTRY" > "$MP/loader/loader.conf"
K=$(stat -f%z "$MP/morpheos/our/vmlinuz" 2>/dev/null || echo 0)
sync
hdiutil detach "$ND" >/dev/null 2>&1

say "3/5 verifying the kernel is really inside the ESP"
[ "$K" -gt 1000000 ] || { say "   kernel size in the ESP is $K — refusing to boot an entry with no kernel"; exit 4; }
say "   $K bytes in the ESP"

say "4/5 sending it to $NODE and writing it into a disk copy"
cat /tmp/esp-boot.img | ssh -i "$KEY" "${O[@]}" "$NODE" "cat > /tmp/morph-vm/esp-boot.img && ls -l /tmp/morph-vm/esp-boot.img | awk '{print \"   received:\", \$5}'"
ssh -i "$KEY" "${O[@]}" "$NODE" "cd /tmp/morph-vm && cp --sparse=always \$(basename \"$IMG\") vm-boot.img 2>/dev/null || cp --sparse=always disk11.img vm-boot.img; dd if=esp-boot.img of=vm-boot.img bs=512 seek=2048 conv=notrunc status=none && echo '   esp written'" < /dev/null

say "5/5 booting it (fresh UEFI vars, port $PORT)"
ssh -i "$KEY" "${O[@]}" "$NODE" "cd /tmp/morph-vm/images/x86-qemu && rm -f ../vm-boot.vars.fd && VM_SSH=1 VM_SSH_PORT=$PORT VM_AUDIO=1 VM_MEM=4096 setsid nohup bash run-vm.sh /tmp/morph-vm/vm-boot.img /tmp/morph-vm/serial-vm.log 86400 >/tmp/morph-vm/qemu-vm.out 2>&1 < /dev/null & sleep 8; echo \"   qemu: \$(pgrep -c qemu-system-x86)\"" < /dev/null

say ""
say "it is booting. Reach it from the node with:"
say "  ssh -i /tmp/mt2/key -p $PORT root@127.0.0.1 uname -s"
