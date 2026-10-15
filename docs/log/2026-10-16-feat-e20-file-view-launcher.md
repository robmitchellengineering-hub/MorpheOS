# 2026-10-16 · DeepSeek · E20 built: the file view opens from the bar

The launcher half of E20: a Files button on the shell's bar (left of the Wi-Fi button) and
Super+E, both opening the file view, mirroring the Wi-Fi button and Super+W. The button's
geometry is a pure function (`files_button`, unit-tested in wm.rs); the compositor wiring is
Linux-only (headless.rs: `Shortcut::Files`, `open_files`, the click, the bar drawing), proven by
CI.

Next: the headless test that clicks the Files button and Super+E and asserts the file view
window appears.
