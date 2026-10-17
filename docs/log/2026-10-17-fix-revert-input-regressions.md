# 2026-10-17 · DeepSeek · Revert two input/GPU regressions from #112

The #112 image made the reference trackpad WORSE (Rob: the pointer's sensor input is paused
and only "janking" moves it). Two new things in #112 could have caused it, so both are
reverted to the #111 behaviour that worked:

1. `live.rs` no longer calls `config_tap_set_enabled(true)` — tap-to-click is restored to the
   libinput default. It will be re-done with SSH access and real logs, not guessed.
2. The `blacklist-nouveau.conf` is removed — the hybrid Mac returns to its previous GPU
   selection. The native boot splash (Rob's clarified requirement) will be drawn by the
   compositor itself, so Firefox's WebGL GPU choice is no longer the launch-screen fix.

The tailscaled start fix from #112 is KEPT (it was correct and is the SSH path).
CI is the proof (Linux-only, H16).
