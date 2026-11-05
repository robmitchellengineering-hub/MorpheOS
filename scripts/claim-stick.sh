#!/bin/bash
# claim-stick.sh — write the claim file onto a prepared MorpheOS stick, so the machine it boots becomes a node.
#
# WHY. The operator steps to join the mesh (a deploy key on the repo, an identity, a tailnet tap) belong in
# preparing the stick, not in installing the OS. This writes /efi/morpheos-node.conf; the image's
# morpheos-claim.service reads it at first boot. No secret goes in the file: the deploy key is still generated on
# the machine and printed for an operator to add.
#
#   ./scripts/claim-stick.sh /dev/disk2 morpheos-2012 [bus-url]
set -euo pipefail
DEV=${1:?usage: claim-stick.sh <device> <host-id> [bus-url]}
HOST_ID=${2:?usage: claim-stick.sh <device> <host-id> [bus-url]}
BUS=${3:-git://100.90.120.32:9418/mesh-hub.git}
say() { printf '%s\n' "$*"; }

info=$(diskutil info "$DEV" 2>/dev/null) || { say "$DEV does not exist"; exit 1; }
rem=$(printf '%s\n' "$info" | awk -F': *' '/Removable Media/ {print $2}')
[ "$rem" = "Removable" ] || { say "refusing: $DEV is not removable media"; exit 2; }

say "mounting the ESP of $DEV"
diskutil mountDisk "$DEV" >/dev/null 2>&1 || true
ESP=$(diskutil list "$DEV" 2>/dev/null | awk '/EFI ESP/ {print $NF}' | head -1)
[ -n "$ESP" ] || { say "no EFI ESP partition found on $DEV"; exit 3; }
MNT=$(diskutil info "$ESP" 2>/dev/null | awk -F': *' '/Mount Point/ {print $2}')
[ -n "$MNT" ] && [ -d "$MNT" ] || { say "the ESP is not mounted; mount $ESP and retry"; exit 4; }

printf 'host=%s\nbus=%s\n' "$HOST_ID" "$BUS" > "$MNT/morpheos-node.conf"
say "wrote $MNT/morpheos-node.conf:"
sed 's/^/  /' "$MNT/morpheos-node.conf"
sync
diskutil unmount "$ESP" >/dev/null 2>&1 || true
say ""
say "the stick now claims $HOST_ID on first boot."
say "remember: the deploy key it generates must still be added to the repo once (morph-node key)."
