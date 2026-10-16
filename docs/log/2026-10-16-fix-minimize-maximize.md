# 2026-10-16 · DeepSeek · minimize and maximize window buttons

Rob reported on the 2012 MacBook Pro that the window minimize and maximize buttons do
nothing. The compositor ignored the `xdg_toplevel` requests. This wires them in: maximize
fills the screen minus the bar (restoring the prior geometry on un-maximize), minimize hides
the window (restored via the cycle shortcut — never unrecoverable). Pure wm logic with five
unit tests; the smithay handlers in headless.rs.
