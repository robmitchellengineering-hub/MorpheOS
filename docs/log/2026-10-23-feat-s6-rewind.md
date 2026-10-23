# 2026-10-23 · DeepSeek · S6 rewind — PR 1: the flow-timeline crate

The `flow-timeline` crate, the first half of S6 (doc 20): the model (`timeline.rs`) with the
rewind-point list, the two-step confirm (dry run, then the real rewind) and the result line,
all unit-tested with no Wayland in it; the backend (`backend.rs`) with a Strata backend
(`Strata::alerts`, `Strata::rewind` dry-run-then-real) and a fake, every Strata call behind the
trait so a non-root shell (E11) swaps in a broker without touching store permissions; the
Wayland window (`main.rs`, drawn with flow-shell's font the way flow-wifi is, with the rewind
off the draw loop and a progress bar while it runs, H17); and `tests/on_the_shell.rs`, which
drives the window on the headless compositor with screendump colour bands per step. The
timeline's colours are registered in flow-wifi's colour list (H20).

What is next: PR 2 — the VM scenario (`build.sh` `rewind`, `test-rewind.sh`, the `.path` unit
that opens the window on an alert, and the workflow wiring), which marks S6 done.
