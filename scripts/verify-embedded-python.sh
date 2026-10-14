#!/bin/bash
# Every Python program embedded in the image tests compiles (doc 20, E5).
#
# The VM tests drive QEMU with Python written inside the shell scripts (`python3 - <<'PY'`).
# `bash -n` does not read inside a heredoc, so a Python syntax error there was found only
# 25 minutes into the images workflow, when the VM it drives was already up (test-wifi.sh,
# 2026-10-14). This compiles each one in a second, and its self-test proves it can fail.
#
#   bash scripts/verify-embedded-python.sh [--self-test]
set -euo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Write each `<<'PY'` heredoc of $1 to its own file in $TMP; print how many.
extract() {
    awk -v out="$TMP/$(basename "$1")" '
        /<<.PY.( |$)/ { n++; f = out "." n ".py"; inside = 1; next }
        inside && /^PY$/ { inside = 0; next }
        inside { print > f }
        END { print n + 0 }' "$1"
}

check() {
    local fail=0 n
    for script in "$@"; do
        n=$(extract "$script")
        for py in "$TMP/$(basename "$script")".*.py; do
            [ -e "$py" ] || continue
            python3 -m py_compile "$py" 2>"$TMP/err" || {
                echo "  FAIL  $script: $(tail -1 "$TMP/err")"
                fail=1
            }
        done
        echo "  ok    $script: $n embedded program(s)"
    done
    return $fail
}

if [ "${1:-}" = --self-test ]; then
    bad="$TMP/bad.sh"
    printf 'python3 - <<%sPY%s\nx = 1\n    y = 2\nPY\n' "'" "'" > "$bad"
    if check "$bad" > /dev/null; then
        echo "verify-embedded-python: the self-test's broken program compiled" >&2
        exit 1
    fi
    echo "verify-embedded-python: a broken embedded program fails the check"
    exit 0
fi

mapfile -t scripts < <(grep -l "<<'PY'" "$HERE"/images/x86-qemu/*.sh)
check "${scripts[@]}"
