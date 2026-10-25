# 2026-10-25 · DeepSeek · fix S7 full-disk test assertions to match the retention policy

The guard works (prunes + survives ENOSPC + the notice shows). The failures were the test's
own expectations, which were stricter than doc-20 S7's "Done when".

1. `one version each` -> the bucket retention (`prunable`) keeps the newest + one per daily
   bucket, so the seed's five same-instant versions yield 2 per file (6 of 3 files). The
   assertion now checks "6 versions of 3 files" and 2 per file.
2. The free-space "says why not" branch greps `still below the floor`, but the guard prints
   `still below the 10% floor` — the grep now matches `still below the`.
3. The notice screendump raced the flow-notice client's first frame (H17) — it now
   re-screendumps up to 10 times until the WARN band shows.
