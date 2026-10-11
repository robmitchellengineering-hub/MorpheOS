#!/usr/bin/env bash
# Prove the `apply` command refuses what it must — and that a refusal writes nothing.
#
# `apply` is the one command whose exit code *is* the contract: 0 applied, 1 REFUSED (a
# decision), 2 a defect. The library's tests call `apply()` directly, so nothing else covers
# the argument handling or the mapping from `Error` to an exit code: a refusal turned into a
# crash, or `--to` renamed out from under the CLI, would stay green everywhere.
#
# Every case asserts the exit code AND the effect on the file. "Exit 1 with the target
# rewritten" is the failure this exists for, and it is invisible to an exit code alone.
#
# Run from the repository root. `set -euo pipefail` throughout, so an unset variable is a
# failure rather than an empty string that silently changes what is being compared.
set -euo pipefail

BIN=target/debug/morph-manifest
cargo build -q -p morph-manifest

OLD=manifest/examples/desk-laptop.toml
NEW=manifest/examples/desk-laptop-proposed.toml
SENTINEL='{"untouched": true}'

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

FAILED=0
fail() {
    echo "FAIL: $*" >&2
    FAILED=1
}

# A target that already holds known content, so "nothing was written" is a claim about a file
# that had something in it — not about a path that was never there.
reset_target() {
    printf '%s\n' "$SENTINEL" > "$WORK/out.json"
}

# Run the command, capture the code without letting `set -e` end the script.
run() {
    set +e
    "$@" >"$WORK/stdout" 2>"$WORK/stderr"
    CODE=$?
    set -e
}

expect_untouched() {
    if ! cmp -s "$WORK/out.json" <(printf '%s\n' "$SENTINEL"); then
        fail "$1: the target was written to. It now holds: $(cat "$WORK/out.json")"
    fi
    # The temp file `apply` writes before the rename must not survive: a leftover is a
    # half-written generation sitting next to the real one. The suffix is APPENDED, so the
    # glob is on the suffix rather than on the target's stem.
    if compgen -G "$WORK/*.morph-new" > /dev/null; then
        fail "$1: a temporary generation was left behind"
    fi
}

echo "1. above the unattended tier, no approver — refused, and nothing written"
reset_target
run "$BIN" apply "$OLD" "$NEW" --to "$WORK/out.json"
[ "$CODE" = 1 ] || fail "expected exit 1 (refused), got $CODE: $(cat "$WORK/stderr")"
grep -q REFUSED "$WORK/stderr" || fail "the refusal was not reported on stderr"
expect_untouched "tier-3 without an approver"

echo "2. the same change with a named approver — applied"
reset_target
run "$BIN" apply "$OLD" "$NEW" --to "$WORK/out.json" --approved-by rob
[ "$CODE" = 0 ] || fail "expected exit 0 (applied), got $CODE: $(cat "$WORK/stderr")"
grep -q '"approved_by": "rob"' "$WORK/stdout" || fail "the approver is not recorded"
grep -q '"risk": "data-affecting"' "$WORK/stdout" || fail "the tier is not reported"
# A refusal proves nothing if the write path never works: assert the generation really landed.
"$BIN" apply "$OLD" "$NEW" --to "$WORK/out.json" --approved-by rob > /dev/null
if cmp -s "$WORK/out.json" <(printf '%s\n' "$SENTINEL"); then
    fail "an approved apply wrote nothing"
fi

echo "3. an invalid generation — refused for everyone, approver or not"
reset_target
# `$2` is appended to a manifest that is valid on its own, so the ONLY thing wrong with the
# broken one is the setting under test. A fixture with two faults would let this case pass for
# a reason that is not its subject.
write_manifest() {
    cat > "$1" <<'TOML'
[machine]
name = "t"
profile = "desk"
arch = "x86_64"

[[users]]
name = "a"
admin = true
recovery_key = true

[[storage.replicas]]
name = "usb"
kind = "disk"
target = "USB"
TOML
    printf '%s' "$2" >> "$1"
}
write_manifest "$WORK/ok.toml" ""
write_manifest "$WORK/broken.toml" '[storage]
min_free_percent = 0
'
run "$BIN" apply "$WORK/ok.toml" "$WORK/broken.toml" --to "$WORK/out.json" --approved-by rob
[ "$CODE" = 1 ] || fail "expected exit 1 (refused), got $CODE: $(cat "$WORK/stderr")"
grep -q 'min_free_percent' "$WORK/stderr" || fail "refused, but not for the reason under test: $(cat "$WORK/stderr")"
expect_untouched "an invalid generation"

echo "4. within the unattended tier — applied with no approver"
reset_target
write_manifest "$WORK/small.toml" '[storage]
min_free_percent = 12
'
run "$BIN" apply "$WORK/ok.toml" "$WORK/small.toml" --to "$WORK/out.json"
[ "$CODE" = 0 ] || fail "expected exit 0 (applied), got $CODE: $(cat "$WORK/stderr")"
if grep -q '"approved_by"' "$WORK/stdout"; then
    fail "an unattended apply named an approver"
fi

echo "5. a usage error is a defect (exit 2), and never claims to be a manifest problem"
run "$BIN" apply "$OLD" "$NEW"
[ "$CODE" = 2 ] || fail "expected exit 2 (usage), got $CODE"
grep -q "needs --to" "$WORK/stderr" || fail "the missing --to was not named"
if grep -q "invalid manifest" "$WORK/stderr"; then
    fail "a usage error blamed the manifest"
fi
run "$BIN" apply "$OLD" "$NEW" --to "$WORK/out.json" --nope
[ "$CODE" = 2 ] || fail "expected exit 2 for an unknown option, got $CODE"

if [ "$FAILED" != 0 ]; then
    echo "apply CLI: FAILED" >&2
    exit 1
fi
echo "apply CLI: 5/5 — refusals refuse, approvals apply, usage errors are not manifest errors"
