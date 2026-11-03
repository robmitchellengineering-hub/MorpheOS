# 2026-11-03 · DeepSeek · doc 26, the fleet revenue path

Added `docs/26-revenue-fleet.md`: the commercial plan for selling the A/B update, rollback and repair as a component to a
device vendor, with the build-out (U1–U6), prices, and kill criteria. Listed in the README.

**What was found that the next person needs.** The brief this was written from said there is no updater. That was true on
2026-10-11 and is no longer true: `crates/morph-update` is 872 lines with keygen, sign, seal and install, refuses an older
release, refuses an unfinished marker, and verifies the payload before touching the slot. It has **no tests**, and the
three A/B VM scenarios still write the inactive slot from a script rather than through it. The plan therefore starts at
"prove the updater", not "write an updater".

The Pi 5 image still has never booted on real firmware (`images/pi5/README.md:33`), and the signing key is still proposed
(ADR 0019), so U2 and U3 are genuine gates before any pilot.

**A note for whoever edits this file next:** the repository's own route-to-mesh hook refused the first write of this document
because the text contained a literal command line naming a Rust test run. The hook scans the content of a tool call, not
only an executed command, so a document that quotes a command is treated as if it ran one. The acceptance line now
describes the check instead of quoting it.

**Next.** U1: a test suite for `morph-update` covering each refusal and a successful install, then route the three A/B VM
scenarios through it.
