# 2026-10-16 · DeepSeek · E20 done: the file view opens from the bar

E20 work (`feat/e20-file-view-launcher`) merged without flipping its status — the plan still
said `open` though the feature and its tests are on `main`. This marks E20 done and adds the
evidence: the bar's Files button geometry (`wm.rs`), the bar-button / Super+E dispatch
(`headless.rs`), and the headless test that clicks the button and asserts Super+E opens the
same single view (`flow-files/tests/on_the_shell.rs`).

No Rob decision here: `open` → `done` with evidence is a builder action, not a `proposed` move.
