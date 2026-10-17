# 2026-10-17 · DeepSeek · Tap-to-click on the reference trackpad

Re-adds `config_tap_set_enabled(true)` in the compositor's `announce` (live.rs), beside the
pointer acceleration it already sets. libinput leaves tapping OFF by default, and the bcm5974
trackpad is physical-click-only without it. The earlier #112 regression was almost certainly the
nouveau blacklist (GPU), not this one-line input config — reverted together in #113 and the
blacklist re-added separately in #115, so this is now added back alone.

(Audio was NOT touched: PipeWire ships as user units, so a system-service enable would break;
audio needs the browser user's own session and is a Tuesday-with-logs task.)

CI is the proof (Linux-only, H16).
