// Firefox offline-mode fix (E14). At boot there is no network yet, so Firefox's
// network.manage-offline-status (default true) flips it into "Work Offline" and it does not
// clear when NetworkManager brings Wi-Fi up — the browser stays on its offline page until a
// manual reload. Never auto-enter offline mode, and clear any residual flag.
//
// NOTE (2026-10-17): no rendering/WebGL prefs belong here. The animation renders exactly as
// designed; the reference laptop's garbage frames are a GPU-selection bug (nouveau vs the
// Intel i915), fixed by blacklisting nouveau — not by changing how Firefox draws.
user_pref("network.manage-offline-status", false);
user_pref("browser.offline", false);

// AUTOPLAY, BECAUSE THE BOOTED IMAGE HAS NOBODY TO TAP. login.html starts the splash and its music
// together, and a browser stops audio until it has seen a gesture — at boot the first thing on the
// screen is that page, so the gesture never comes and the machine is silent. Firefox will play
// without one when the profile says so; measured need, not a preference (2026-10-29: "there is no
// audio on the splash screen when it boots").
user_pref("media.autoplay.default", 0);
user_pref("media.autoplay.blocking_policy", 0);
user_pref("media.autoplay.block-webaudio", false);
user_pref("media.autoplay.block-event.enabled", false);
