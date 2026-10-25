#!/usr/bin/env bash
# The x86 image must run `morphd boot` at every start (doc 20, S8/S11): that is what finds a
# corrupt generation in force, rolls it back to the newest good one, and records the heal.
# It runs only if morphd.service is enabled, and "enabled" is one line in the build that no
# VM test can read without a ~30-minute boot. This checks the two things that must agree,
# without building an image:
#
#   * the service declares itself for multi-user.target (so `WantedBy=` is real, not prose);
#   * the x86 build actually symlinks it into multi-user.target.wants/ (so it runs, rather
#     than merely sitting in the overlay).
#
# Each rule can fail, and --self-test proves it: a verifier that stopped biting is found here,
# in seconds, not in the QEMU job that boots the image.
set -euo pipefail

SERVICE=images/common/overlay/etc/systemd/system/morphd.service
BUILD=images/x86-qemu/build.sh

# The service file must run `morphd boot` and be wanted by the target the build starts it in.
service_rule() {
    local svc=$1
    [ -f "$svc" ] || { echo "$svc: morphd.service is missing"; return 1; }
    grep -q '^ExecStart=/usr/bin/morphd boot$' "$svc" \
        || { echo "$svc: does not run 'morphd boot'"; return 1; }
    grep -q '^WantedBy=multi-user.target$' "$svc" \
        || { echo "$svc: is not wanted by multi-user.target"; return 1; }
}

# The unit names the build enables, from its `for unit in …` loops. Only literal names count:
# a `$var` is not a unit, and a comment must not be (AGENTS.md H3).
enabled_units() {
    local build=$1
    grep -vE '^[[:space:]]*#' "$build" \
        | grep -oE 'for unit in [^;]+' \
        | sed -E 's/^for unit in //' \
        | tr ' ' '\n' \
        | grep -vE '^\$'
}

# The build must enable morphd, or `morphd boot` never runs and the rollback never happens.
build_rule() {
    local build=$1
    if ! enabled_units "$build" | grep -qx 'morphd'; then
        echo "$build: morphd.service is never enabled (no multi-user.target.wants loop names morphd)"
        return 1
    fi
}

if [ "${1:-}" = --self-test ]; then
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    fail() { echo "verify-morphd-boot: self-test: $*" >&2; exit 1; }

    printf 'ExecStart=/usr/bin/other\nWantedBy=multi-user.target\n' > "$tmp/bad-exec.svc"
    ! service_rule "$tmp/bad-exec.svc" > /dev/null || fail "a service that does not run 'morphd boot' was accepted"
    printf 'ExecStart=/usr/bin/morphd boot\nWantedBy=multi-user.target\n' > "$tmp/good.svc"
    service_rule "$tmp/good.svc" > /dev/null || fail "a correct service file was refused"
    printf 'ExecStart=/usr/bin/morphd boot\n' > "$tmp/no-want.svc"
    ! service_rule "$tmp/no-want.svc" > /dev/null || fail "a service without WantedBy was accepted"

    printf 'for unit in strata strata-guard; do\n  ln -sf "x/$unit.service" "y/$unit.service"\ndone\n' > "$tmp/bad-build.sh"
    ! build_rule "$tmp/bad-build.sh" > /dev/null || fail "a build that does not enable morphd was accepted"
    printf 'for unit in morphd strata; do\n  ln -sf "x/$unit.service" "y/$unit.service"\ndone\n' > "$tmp/good-build.sh"
    build_rule "$tmp/good-build.sh" > /dev/null || fail "a build that enables morphd was refused"
    # A comment naming morphd must not count as enablement (H3).
    printf '# for unit in morphd strata; do\nfor unit in strata; do\n  ln -sf x y\ndone\n' > "$tmp/comment-build.sh"
    ! build_rule "$tmp/comment-build.sh" > /dev/null || fail "a comment naming morphd was counted as enablement"

    echo "verify-morphd-boot: every rule can fail"
    exit 0
fi

cd "$(dirname "$0")/.."
status=0
if why=$(service_rule "$SERVICE"); then
    echo "  ok    morphd.service runs 'morphd boot' and is wanted by multi-user.target"
else
    echo "  FAIL  $why"
    status=1
fi
if why=$(build_rule "$BUILD"); then
    echo "  ok    the x86 build enables morphd.service at boot"
else
    echo "  FAIL  $why"
    status=1
fi
exit $status
