# 2026-10-16 · DeepSeek · sound and SSH on the plain image

The MacBookPro9,1 boot had no sound and I could not reach it over the network. Both fixed:

- **Sound** — the image had no sound server (the `snd_hda_intel` driver was fine, no missing
  firmware, but Firefox had nowhere to output). Add `pipewire` + `pipewire-pulse` +
  `wireplumber` (+ `alsa-ucm-conf`) to the browser base.
- **SSH** — `openssh-server`, root login key-only with the mesh admin key baked into
  `/root/.ssh/authorized_keys`, and the service enabled, so the mesh can drive the machine
  over ethernet without a person at the console.
