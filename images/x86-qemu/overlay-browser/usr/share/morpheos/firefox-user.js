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
