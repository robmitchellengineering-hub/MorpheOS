# 2026-10-14 · Claude Code · The shell-frame check says why when it fails

`hung-shell` failed once on #75, a docs-only pull request. The boot history was right
(`a/committed/none b/trial/fallback a/committed/none a/committed/none`), and the trial failed
on `flow-shell.service` as it should. But the serial log held the line
`flow-compositor: first frame on the display; ready` 2 times, not 3. The same scenario passed
on `main` and on #72, #73 and #74 the same day.

Why one boot's line is missing is **not known**. The `x86-serial-logs` artifact could not be
read from the cloud session: its storage host is blocked by that session's proxy. So nothing
is weakened here: the count stays at 3. The failure now prints, into the job log itself,
every boot's kernel command line (`morph.slot=`) and every shell line, in order. The next
time it happens, the job log shows which boot lacked the line and what that boot said
instead. #75's failed job was re-run once, to see whether it repeats.
