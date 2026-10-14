# 2026-10-14 · Claude Code · A replace is timed from its first half, so the rewind covers it

The second handoff rehearsal saw `strata`'s break-it test fail once during a full
`check.sh` run on a busy machine. It passed 15 times out of 15 alone, and passed on the next
full run. After the rewind, one file was left over: `2026/clients/invoice-1.txt.locked`, the
ciphertext half of the first "replace" in the attack (in sorted order). Its original,
`invoice-1.txt`, had been restored.

**The gap.** A "replace" is two changes: the ciphertext is written, and the original is
deleted. The detector counted it as one hit, timed by whichever half it saw second. The
rewind point is the burst's first hit minus one second. Version times are when the watcher
*recorded* each change, so under load the two halves can be more than a second apart. The
rewind point then falls between them, and the first ciphertext was written "before" it and
is kept.

**The fix** (`crates/strata/src/detect.rs`):

- A replace's hit takes the earlier of its two times.
- The burst's first time is the minimum over the run, because hits are no longer strictly
  in time order.
- An alert already raised moves its rewind point earlier, and is re-issued, if a later hit
  turns out to have started earlier.

**Evidence.** `a_replace_counts_from_its_first_half_so_the_rewind_covers_it` puts 3 s
between the halves, in both orders. It failed on the old code and passes now. The break-it
test passed 80 of 80 in parallel stress runs with the fix.

**Stated plainly:** I could not reproduce the original flake. The old code also passed 128
of 128 stress runs (CPU load, and synchronous-write load). So the claim is that the failure
seen fits this gap exactly, and that the gap is closed. It is not that this flake can no
longer happen for some other reason.
