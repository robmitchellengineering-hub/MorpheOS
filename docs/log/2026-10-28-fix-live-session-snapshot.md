# 2026-10-28 · DeepSeek · the stick records the live session, not just the boot

The first boot of the x86 image on real hardware (a 2012 Mac Pro, 2026-10-27) came back with a
session that did not work, and **the stick could not say why**:

> "i see 2 wifi windows on the screen it says the wifi is connected but firefix isnt loading any thing"
> "nothing else is working firefox is gone and i have 2 wifi windows"

Everything worth knowing was missing from the report:

- `net.txt` was read straight from `/sys/class/net`, so it knew the interface, its state, its MAC
  and its driver — and nothing about **routes, nameservers or what NetworkManager thought**. A
  link that says `up` and reaches nothing is byte-identical in sysfs to a good one.
- `journal-errors.txt` is `journalctl -b -p err`, i.e. **errors only, at boot**.
- **The journal is in RAM** (no `Storage=persistent` anywhere in the image), so the shell's,
  the picker's and the browser's own messages died with the power.
- Nothing recorded **which processes were running**, so "two Wi-Fi windows" — one picker from
  `flow-wifi.service` and one from the shell's own `open_wifi()` — left no trace at all.

## What changed

1. **`hwreport` now writes two more files**, best effort like the rest of it:
   `net-live.txt` (`ip -br a`, `ip route`, `/etc/resolv.conf`, `nmcli con show --active`,
   `nmcli device status`) and `processes.txt` (every process, so a duplicate binary is visible).
2. **`morph-observe` — a new timer**, `OnBootSec=20s` then `OnUnitInactiveSec=20s`, appending a
   snapshot to `/efi/morpheos-report/live-<machine>.txt`: interfaces, routes, nameservers, active
   connections, device status, the `flow-*`/`firefox` process list, failed units, and the last 40
   journal lines for `flow-shell`/`flow-wifi`/`NetworkManager`/`morph-banner`. Bounded at 512 KiB
   (it keeps the newest half), `sync`ed every time, because the stick can be pulled out at any
   moment.
3. **`build.sh` enables the timer for the hardware image** and **asserts the three files are in
   the image** — enabling is a symlink, which succeeds even when the unit it points at is absent,
   and then the stick is silent with nothing saying why.

**The safety rule that shapes all of it:** a failed unit makes the boot unhealthy, and an
unhealthy trial is rolled back (doc 20, E11). So the unit's `ExecStart` carries a `-`, the script
is best effort throughout and exits 0 always. A diagnostic must never be able to undo an update.

## Verified here, not on a VM

| check | result |
|---|---|
| `bash -n` on `build.sh`, `hwreport`, `observe` | all clean |
| the repo's own `scripts/verify-hwreport.sh` | 5/5 ok — the firmware section still reads correctly |
| `observe` with a normal environment | exit 0, file written, every section present |
| `observe` with **no tools at all** (`PATH=/nonexistent`) | exit 0 — best effort, as designed |
| size cap | 614,400 → 262,144 bytes, exit 0 |

**Not verified:** the unit actually running in a booted image (that needs the next `images` build
and a stick). The 2012 Mac Pro's own evidence still has to come from the console for this boot,
because this change is not in the image he is holding.
