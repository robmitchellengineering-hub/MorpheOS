# 2026-10-15 · DeepSeek · E17 built: the file view lists a folder and opens a file

The window on the shell (doc 20, E17, open · parallel): `flow-files --dir <folder>` lists it
(folders first, then files), paints one row each in its own colour, and a click enters a
folder or opens a file with the desktop's default handler (`xdg-open`, the path single-quoted
so a space or a quote cannot run anything else). The listing and the click decision are pure
and unit-tested in `lib.rs` (5 tests); `tests/on_the_shell.rs` opens the real window on the
real compositor, asserts the listing and the open command, and reads the frame to see the
folder and file rows.

The window is a plain pointer-only Wayland client, the same handlers and buffer path as
`flow-wifi`, so it is Linux-only and CI proves it (the Mac cannot build `#[cfg(target_os =
"linux")]` code).

Next: E16's resize PR (#81) merges first (same check-first shape); then a bar button so a
person can reach the file view, and USB mounting (E18).
