# 2026-10-16 · DeepSeek · E14: Firefox comes online when the Wi-Fi picker joins

The fix: Firefox starts at boot with no network, so its `network.manage-offline-status`
(default true) flips it into "Work Offline" and it never clears that when NetworkManager
brings Wi-Fi up — Rob saw the offline page persist until a manual reload. A seeded
`user.js` (`firefox-user.js`, copied idempotently by `flow-browser.service` into the
profile before Firefox starts) sets `network.manage-offline-status` false and clears any
residual `browser.offline`. The `wifi` image now carries Firefox, and `test-wifi.sh`
navigates it to a page the simulated access point serves (`http://192.168.77.1/`) and
asserts its colour on screen with no reload step — the first navigation after the join
must just work.

Opening E14 (`proposed` → `done`) is Rob's call, recorded here.

The QEMU scenario is CI-validated only: macOS cannot run `scripts/check.sh` or the image
build (AGENTS.md H16), so CI's `rust` and `images` jobs are the proof.

Rob on E14: Fix Firefox bug (2026-10-16)
