# 2026-10-15 · DeepSeek · E6 prep: hardware-report scaffolding, and the guard that keeps it honest

Doc 20's E6 "builder's half" (note 3) is `plan/` work that needs no report. Done here:

- Created `docs/hardware/{win,win-2,mac-2015}.md`, each with the standard headings
  (Machine, Boot path, Display, Keyboard, Touchpad, Wi-Fi, Wired, Sound, Firmware missing,
  Boot history, Tasks), empty pending Rob's reports.
- Extended `scripts/verify-docs.mjs` to check every `docs/hardware/*.md` has those headings
  and contains no MAC address (`([0-9a-f]{2}:){5}[0-9a-f]{2}`). The check is a pure function,
  so a new `--self-test` flag can prove it fails. Negative-tested: a file with
  `aa:bb:cc:dd:ee:ff` makes the check exit 1; removing it restores a pass.
- Wired `verify-docs.mjs --self-test` into `scripts/check.sh` and the `rust` CI job, so a
  future edit that quietly breaks the check is refused.

What was found that the next person needs: `verify-docs.mjs` previously refused every
argument, so it could not be self-tested at all (H17's shape — "nothing failed" reading as
"the check ran"). Now the one flag is `--self-test`; anything else is still refused.

Next: Rob's E6 report folders, to fill the three files in and turn every failure into a task.
