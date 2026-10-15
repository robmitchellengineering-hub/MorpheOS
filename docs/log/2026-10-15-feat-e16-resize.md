# 2026-10-15 · DeepSeek · E16: resize a window by its corner

Built resize: a drag on a window's corner resizes it — the corner follows the pointer, the
opposite corner stays put, and the client is told the new size (its `configure`); an edge drag
still moves. The decisions are in `wm.rs` (unit-tested), the compositor wires a corner press to
a resize and tells the client the new size, and a headless test proves it end to end.

Next: the file view (E17), then USB-mounting.
