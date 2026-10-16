// Firefox offline-mode fix (E14). At boot there is no network yet, so Firefox's
// network.manage-offline-status (default true) flips it into "Work Offline" and it does not
// clear when NetworkManager brings Wi-Fi up — the browser stays on its offline page until a
// manual reload. Never auto-enter offline mode, and clear any residual flag.
user_pref("network.manage-offline-status", false);
user_pref("browser.offline", false);
