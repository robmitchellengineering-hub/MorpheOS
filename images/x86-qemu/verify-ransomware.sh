# The ransomware verdict both scenarios share, so the two tests cannot drift apart
# (doc 20, S6). SOURCED, never executed: it defines the helpers and the disk checks, and
# leaves the per-scenario verdicts (boot history, self-test step, the rewind-only checks)
# to the caller.
#
# The caller sets, before sourcing or before calling:
#   WORK       the work directory (both the data copy and the checksum file land here)
#   disk       the built disk image
#   DOCS       the folder the self-test attacked, as the VM sees it (/home/morph/Documents)
#   WANT_DOCS  how many documents the self-test wrote
#   fail       the caller's running pass/fail counter (0 = pass so far)
#
# Every verdict is read from the disk after the VM stops, never from the console or from what
# the self-test says about itself (AGENTS.md H8): the data partition is copied out with dd and
# read with debugfs, exactly as test-ransomware.sh first did it.

# One file off the FAT ESP (the boot partition the self-test writes its markers to).
esp() { mtype -i "$disk@@1M" "::/$1" 2>/dev/null; }

# One verdict: print ok/FAIL and set `fail` on a failure.
check() { if eval "$2"; then echo "    ok    $1"; else echo "    FAIL  $1"; fail=1; fi; }

# Copy the data partition out of the disk image, so debugfs reads it without mounting
# anything. Sets `data` (the copy's path) for the caller and the checks below.
data_partition() {
    read -r dstart dsize < <(sfdisk -d "$disk" | sed -nE 's/.*start= *([0-9]+), size= *([0-9]+),.*name="data".*/\1 \2/p')
    data="$WORK/data-out.img"
    dd if="$disk" of="$data" bs=512 skip="$dstart" count="$dsize" status=none
}

# The shared part of the verdict: Strata recorded the alert, its rewind point is after the
# documents were complete, every document matches its pre-attack checksum, nothing of the
# attacker's is left among them, and what the rewind moved away went to quarantine (not gone).
# Sets `out` (the rdumped documents + quarantine) for the caller.
verify_ransomware_docs() {
    # Strata's own record of the attack.
    alerts=$(debugfs -R "cat /strata/alerts.jsonl" "$data" 2>/dev/null || true)
    echo "    alerts: ${alerts:-none}"
    check "Strata recorded an alert" '[ -n "$alerts" ]'
    good_at=$(esp morph-test/good-at-ns | tr -d '\r\n' || true)
    rewind_to=$(printf '%s\n' "$alerts" | head -1 | sed -nE 's/.*"rewind_to":([0-9]+).*/\1/p')
    check "its rewind point is after the documents were complete" \
        '[ -n "$good_at" ] && [ -n "$rewind_to" ] && [ "$rewind_to" -gt "$good_at" ]'

    # The documents: every one back, byte for byte.
    esp morph-test/before.sha256 > "$WORK/before.sha256" || true
    n=$(wc -l < "$WORK/before.sha256")
    check "checksums were taken of $WANT_DOCS documents before the attack (got $n)" '[ "$n" -eq "$WANT_DOCS" ]'
    out="$WORK/docs-out"
    rm -rf "$out" && mkdir -p "$out"
    debugfs -R "rdump $DOCS $out" "$data" >/dev/null 2>&1 || true
    check "every document matches its checksum from before the attack" \
        '(cd "$out/Documents" && sha256sum --quiet -c "$WORK/before.sha256")'
    left=$(cd "$out/Documents" 2>/dev/null && find . -type f \( -name '*.locked' -o -name 'READ_ME_TO_RECOVER.txt' \) | sort | tr '\n' ' ')
    check "nothing of the attacker's is left among them (got: ${left:-nothing})" '[ -z "$left" ]'

    # What the rewind moved away is kept, not deleted.
    quarantined=$(debugfs -R "rdump /strata/quarantine $out" "$data" >/dev/null 2>&1;
        find "$out/quarantine" -type f -name '*.locked' 2>/dev/null | wc -l)
    check "the .locked files went to quarantine (got $quarantined)" '[ "$quarantined" -gt 0 ]'
}
