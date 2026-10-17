# 2026-10-17 · DeepSeek · Tailscale joins with a pre-auth key (unattended)

The interactive login needs the console, and the 2012 Mac's console switch did not surface the
URL — so the join is made unattended. `morpheos-tailscale-up` now reads a pre-auth key from the
ESP (`/efi/morpheos-tailscale.key`, written at flash time, never committed to this public repo)
and passes it as `--auth-key`; without the file it falls back to the interactive URL. This keeps
the credential out of the public image artifact: it is injected into the flashed stick only.

CI is the proof (`check.sh` + QEMU scenarios are Linux-only, AGENTS.md H16).
