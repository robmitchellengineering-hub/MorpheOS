#!/usr/bin/env bash
# Prove the transaction command decides the way its exit codes claim.
#
# The library tests call `Transaction` directly, so nothing else covers the argument handling,
# the `Error` → exit-code mapping, or the ordering a *caller* imposes. The property that matters
# most here is not "it returns false" but "the machine is on the generation it was on before",
# so every case that expects a rollback checks the store as well as the code.
#
# Run from the repository root.
set -euo pipefail

TX=target/debug/morph-transaction
MAN=target/debug/morph-manifest
cargo build -q -p morph-transaction -p morph-manifest

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

FAILED=0
fail() {
    echo "FAIL: $*" >&2
    FAILED=1
}

run() {
    set +e
    "$@" >"$WORK/stdout" 2>"$WORK/stderr"
    CODE=$?
    set -e
}

# One valid machine, and two changes to it that a tier lets through unattended.
write() {
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
write "$WORK/v1.toml" ""
write "$WORK/v2.toml" '[storage]
min_free_percent = 12
'
write "$WORK/v3.toml" '[storage]
min_free_percent = 15
'
write "$WORK/v4.toml" '[storage]
min_free_percent = 20
'
# Security-critical: a capsule runs arbitrary code and carries its own grants. It starts from
# generation 2's contents, because a plan has to start from the generation IN FORCE — otherwise
# the store refuses it for being stale and the tier is never reached.
write "$WORK/dangerous.toml" '[storage]
min_free_percent = 15

[[capsules]]
name = "crm"
runtime = "wine"
source = "recipe:acme-crm-2012"
grants = ["files:documents"]
'

STORE="$WORK/store"
current() { readlink "$STORE/current"; }
generations() { find "$STORE/generations" -name '*.json' -not -name '*.meta.json' | wc -l | tr -d ' '; }

echo "1. a transaction with nothing to fall back to is refused, and says what to do"
run "$TX" apply --store "$STORE" --old "$WORK/v1.toml" --new "$WORK/v2.toml" --health true
[ "$CODE" = 1 ] || fail "expected exit 1 (a decision), got $CODE: $(cat "$WORK/stderr")"
grep -q 'morph-manifest apply --store' "$WORK/stderr" || fail "the refusal does not name the way to establish a first generation"
if grep -q '^REFUSED: refused:' "$WORK/stderr"; then
    fail "the refusal printed its marker twice"
fi

echo "2. establish generation 1 the way the refusal said to"
"$MAN" apply "$WORK/v1.toml" "$WORK/v2.toml" --store "$STORE" > /dev/null
[ "$(current)" = "generations/0001.json" ] || fail "generation 1 is not in force"

echo "3. a healthy transaction commits"
run "$TX" apply --store "$STORE" --old "$WORK/v2.toml" --new "$WORK/v3.toml" --health '"true"'
[ "$CODE" = 0 ] || fail "expected exit 0 (committed), got $CODE: $(cat "$WORK/stderr")"
grep -q '"committed": true' "$WORK/stdout" || fail "the outcome does not say it committed: $(cat "$WORK/stdout")"
[ "$(current)" = "generations/0002.json" ] || fail "the committed generation is not in force"

echo "4. an unhealthy transaction takes itself back, and the machine is where it was"
run "$TX" apply --store "$STORE" --old "$WORK/v3.toml" --new "$WORK/v4.toml" --health false
[ "$CODE" = 1 ] || fail "expected exit 1 (rolled back), got $CODE"
grep -q 'ROLLED BACK' "$WORK/stderr" || fail "the rollback was not reported: $(cat "$WORK/stderr")"
grep -q '"committed": false' "$WORK/stdout" || fail "the outcome does not say it rolled back: $(cat "$WORK/stdout")"
[ "$(current)" = "generations/0002.json" ] || fail "the machine is not on the generation it was on before"
[ "$(generations)" = 3 ] || fail "the trialled generation was deleted from the history ($(generations) present)"
run "$TX" status --store "$STORE"
grep -q '"trial": null' "$WORK/stdout" || fail "a trial is still open after a rollback: $(cat "$WORK/stdout")"

echo "5. a change the Manifest layer refuses leaves nothing armed"
before=$(find "$STORE" | sort)
run "$TX" apply --store "$STORE" --old "$WORK/v3.toml" --new "$WORK/dangerous.toml" --health true
[ "$CODE" = 1 ] || fail "expected exit 1 (refused), got $CODE"
grep -q 'above the tier' "$WORK/stderr" || fail "refused, but not for the tier: $(cat "$WORK/stderr")"
grep -q 'ROLLED BACK' "$WORK/stderr" && fail "a refused change was reported as a rollback"
[ "$(find "$STORE" | sort)" = "$before" ] || fail "a refused change altered the store"
run "$TX" status --store "$STORE"
grep -q '"trial": null' "$WORK/stdout" || fail "a refused change left a trial armed: $(cat "$WORK/stdout")"

echo "6. begin blocks a second trial, names both ways out, and rollback resolves it"
# The generation numbers are read from the store rather than assumed: a rolled-back trial has
# already consumed a number, so `current + 1` is not the next generation. That mistake is one
# the library's own tests caught, and a hard-coded number here would hide it again.
in_force=$(current)
before_count=$(generations)
run "$TX" begin --store "$STORE" --old "$WORK/v3.toml" --new "$WORK/v4.toml"
[ "$CODE" = 0 ] || fail "begin exited $CODE: $(cat "$WORK/stderr")"
[ "$(generations)" = "$((before_count + 1))" ] || fail "begin did not add a generation ($(generations) present)"
next=$(printf 'generations/%04d.json' "$((before_count + 1))")
[ "$(current)" = "$next" ] || fail "begin did not put the trial in force (current is $(current), expected $next)"
run "$TX" begin --store "$STORE" --old "$WORK/v3.toml" --new "$WORK/v4.toml"
[ "$CODE" = 1 ] || fail "expected exit 1 for a second trial, got $CODE"
grep -q 'commit' "$WORK/stderr" || fail "the refusal does not name commit"
grep -q 'rollback' "$WORK/stderr" || fail "the refusal does not name rollback"
run "$TX" rollback --store "$STORE"
[ "$CODE" = 0 ] || fail "rollback exited $CODE: $(cat "$WORK/stderr")"
[ "$(current)" = "$in_force" ] || fail "rollback did not restore $(basename "$in_force")"

echo "7. begin then commit keeps the trial"
in_force=$(current)
before_count=$(generations)
run "$TX" begin --store "$STORE" --old "$WORK/v3.toml" --new "$WORK/v4.toml"
[ "$CODE" = 0 ] || fail "begin exited $CODE"
run "$TX" commit --store "$STORE"
[ "$CODE" = 0 ] || fail "commit exited $CODE: $(cat "$WORK/stderr")"
next=$(printf 'generations/%04d.json' "$((before_count + 1))")
[ "$(current)" = "$next" ] || fail "the committed trial is not in force (current is $(current))"
run "$TX" status --store "$STORE"
grep -q '"trial": null' "$WORK/stdout" || fail "the trial survived a commit"

echo "8. a health check that cannot finish is bounded, and counts as a failure"
in_force=$(current)
started=$(date +%s)
run "$TX" apply --store "$STORE" --old "$WORK/v4.toml" --new "$WORK/v3.toml" --health 'sleep 30' --timeout 1
[ "$CODE" = 1 ] || fail "expected exit 1 (rolled back on timeout), got $CODE"
grep -q 'did not finish' "$WORK/stderr" || fail "the timeout is not the reported reason: $(cat "$WORK/stderr")"
elapsed=$(( $(date +%s) - started ))
[ "$elapsed" -lt 10 ] || fail "the transaction waited ${elapsed}s on a check bounded at 1s"
[ "$(current)" = "$in_force" ] || fail "the machine moved after a timed-out check"

echo "9. a missing health check is a usage error, not a silent commit"
run "$TX" apply --store "$STORE" --old "$WORK/v4.toml" --new "$WORK/v3.toml"
[ "$CODE" = 2 ] || fail "expected exit 2 (a usage error), got $CODE"
grep -q 'needs --health' "$WORK/stderr" || fail "the missing --health was not named"
run "$TX" commit --store "$WORK/empty-store"
[ "$CODE" = 1 ] || fail "expected exit 1 for a commit with no trial, got $CODE"

if [ "$FAILED" != 0 ]; then
    echo "transaction CLI: FAILED" >&2
    exit 1
fi
echo "transaction CLI: 9/9 — a check that fails or never finishes is taken back, a refusal changes nothing, and commit is the only thing that keeps a trial"
