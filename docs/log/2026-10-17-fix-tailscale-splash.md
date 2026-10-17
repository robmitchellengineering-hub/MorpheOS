# 2026-10-17 · DeepSeek · Tailscale on the image + software-render the launch animation

Two image changes that make the reference laptop reachable and its boot animation visible,
batched on one fix/ branch (neither changes a doc-20 task status):

1. **Tailscale in the `plain` image.** `tailscale` joins `BROWSER_PKGS` from Tailscale's own
   apt repo (key fetched at build time, same signed-by pattern as the Mozilla repo). Only the
   `plain` scenario enables `tailscaled` + `morpheos-tailscale-up`; the browser/wifi test
   images carry the package but not the unit. `morpheos-tailscale-up` runs `tailscale up
   --hostname=morpheos-2012 --accept-dns=false`, tees the one-time login URL to
   `/persist/tailscale-login.txt`, and waits (no timeout) for approval. This is the reach layer
   the existing `scripts/oneclick-node.sh` needs; once the machine is on the tailnet, the node
   install is that script over SSH — nothing re-invented.

2. **The launch animation on the NVIDIA chip.** On the MacBookPro9,1 Firefox's WebGL gets no GL
   context from nouveau, so the animation flashes and the 8s fallback hands over to the shell.
   `flow-browser.service` now exports `LIBGL_ALWAYS_SOFTWARE=1` (Mesa llvmpipe), and
   `firefox-user.js` adds `webgl.force-enabled=true`, so the geomesh renders on the CPU.

CI is the proof: `check.sh` and the QEMU scenarios are Linux-only (AGENTS.md H16).
