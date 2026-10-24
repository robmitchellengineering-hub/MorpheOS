# 2026-10-23 · DeepSeek · fix the flaky /home check in the wipe-in-machine scenario

The `images` x86 job's `wipe-in-machine` scenario loses the 24 `seed-*.txt` files' ext4
inode metadata across the emergency reboot: `sync` commits the journal transaction but does
not force the journal *checkpoint* (the copy of the inode table back to its on-disk home),
so the inode updates can still be journal-only when `echo b > /proc/sysrq-trigger` resets the
machine, and the post-run check reports `./seed-1.txt: FAILED open or read`. `seed_home`
(images/x86-qemu/overlay-recovery-test/usr/lib/morpheos/test-hook) now does
`sync; sleep 2; sync` after recording the checksums, giving jbd2's checkpoint thread time to
write the inode table and then flushing it. The wipe and recovery logic is untouched.

**Not proven here:** this is a timing heuristic, not a deterministic checkpoint — the only
deterministic trigger is unmount (`jbd2_journal_flush`), which is out of scope for a
`seed_home` edit. The change needs a real `images` workflow run (or the mesh QEMU node) to
show the /home check passing; `bash -n`/`sh -n` only prove the script is syntactically valid.
Also on the record: an earlier "unmount /persist" attempt was reported among the failed
hypotheses, but /persist has `/home` and `/var/lib/strata` bind-mounted on it, so a bare
`umount /persist` would have failed busy and never checkpointed — worth confirming before
treating it as counter-evidence.
