# 2026-10-28 · DeepSeek · the two things that made the first real-hardware boot unusable

From the stick, not from memory. `morpheos-report/002-MacBookPro9-1-a/` on the first boot of the
x86 image on real hardware — an Apple **MacBookPro9,1** (mid-2012 15", i7-3615QM, dual GPU: Intel
HD 4000 on `card0`, NVIDIA GT 650M on `card1` with the internal panel `card1-LVDS-1`, Wi-Fi
**BCM4331** on `b43`, Ethernet BCM57765 on `tg3`, no missing firmware).

Rob, at the machine: *"the wifi is connected but firefox isnt loading anything"*, then *"nothing
else is working firefox is gone and i have 2 wifi windows"*.

## 1. Read-only `/var/lib` and `/etc` versus services that must write

The boot's own errors:

```
tailscaled: Failed to set up special execution directory in /var/lib: Read-only file system   (×3, failed to start)
systemd-tmpfiles: /var/lib/dbus/machine-id, /root, /var/log/journal — Read-only file system
```

The root is mounted `ro` on purpose. Saved Wi-Fi networks were already redirected to `/persist`
(`conf.d/30-saved-networks-on-persist.conf`), which proves the pattern — but three things that need
writable state were never given it:

- **`/etc/resolv.conf`** is written by NetworkManager on every join, and `/etc` is read-only. That
  is the best explanation of "connected, nothing loads": the join succeeds, the nameservers are
  never written. (Stated as the leading hypothesis, not a proven cause — see what would settle it.)
- **`/var/lib/NetworkManager`** — runtime state (device state, DHCP leases).
- **`/var/lib/tailscale`** — the node key; `tailscaled` could not start at all.

Fixed by the same pattern the image already uses: `tmpfs` for the two `/var/lib` directories, and
`/etc/resolv.conf` as a **symlink to `/run/NetworkManager/resolv.conf`** with
`conf.d/40-resolv-on-run.conf` (`dns=default`, `rc-manager=symlink`). `build.sh` asserts the
symlink is a symlink, because a plain file left in place would look right and silently never update.

## 2. Two Wi-Fi windows — two starters, one program

`flow-wifi.service` starts a picker when `nm-online` fails at boot. The shell starts **its own** on
the bar button or Super+W, and `Server::open_wifi()` only refuses when *it* has already spawned one
(`if !self.launched.is_empty()`). Neither knows about the other, so both run.

Fixed in the program rather than in either starter: `flow-wifi` now checks `/proc` for another
`flow-wifi` and exits **0** with a line on stdout, so the loser of the race is not a failed unit.
A `/proc` scan rather than a lock file: a lock left behind by a crash would silence the picker for
the rest of the boot, and no cleanup path is more trustworthy than the kernel's own process list.

## Verified here

| check | result |
|---|---|
| `cargo check -p flow-wifi --target x86_64-unknown-linux-gnu` | **passes** — the Linux-only code is actually compiled (a macOS `cargo check` compiles none of it: AGENTS.md H16) |
| `cargo fmt --all --check` | clean (it found one diff, which is now applied) |
| `bash -n images/x86-qemu/build.sh` | clean |

**Not verified:** a booted image. That needs the `images` build and a flash, and only then does the
next stick answer the resolver question — with `net-live.txt`, `ip route` and `/etc/resolv.conf`
written by the capture this branch's predecessor (#160) added.
