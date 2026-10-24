# 2026-10-24 · DeepSeek · S3 auto-update — PR 4: bad and tampered

S3's fourth pull request: the two refusal scenarios `test-autoupdate.sh` promised in PR 3
(`good` landed there). Verdicts are read off the boot history and the server's request log,
never the serial console (AGENTS.md H8); the console is a trigger for when to stop the VM.

- `images/x86-qemu/build-payload.sh`: two new `dir`-shaped kinds. `bad` writes a
  `morph-broken.service` unit (`Type=oneshot ExecStart=/bin/false`) and its
  `multi-user.target.wants` symlink into the payload's root image with `debugfs` (the same
  shape as `build.sh`'s `unhealthy` scenario, but into an image file), so the trial's health
  check sees a failed unit and falls back. `tampered` flips one byte of `root.img` at offset
  1 MiB **after** signing (XOR `0xff`, so the byte is guaranteed to change — H3): the manifest
  and its signature still describe the original image, so the machine's signature check passes
  and its digest check (`Payload::check`) refuses it, having downloaded it. Both leave the
  payload as a directory for the HTTPS server.
- `images/x86-qemu/overlay-autoupdate/usr/lib/morpheos/test-hook`: step 1 no longer reboots
  unconditionally. It reboots only when `morph-boot status` says slot B is the default (the
  `good` commit-stuck proof); on a fallback it stays up, and the `*` step powers off only when
  B is the default (doc 20, S3 note 2). This keeps `bad`/`tampered` running for the harness to
  stop, and stops `good`'s walk from gaining a fourth boot.
- `images/x86-qemu/test-autoupdate.sh`: `good` unchanged. `bad` boots in the background, waits
  for the fallback decision then three skipped timer runs (each prints "already failed a
  trial" — a trigger only), stops the VM through QMP, and asserts history
  `a/committed/none b/trial/fallback a/committed/none`, default A, the trial failing on
  `morph-broken.service`, and `root.img` downloaded exactly once (no second download over the
  skipped runs). `tampered` waits for "the payload was refused", stops the VM, and asserts
  history `a/committed/none` (no trial), slot B with no marker and no `.morph-incomplete`, and
  `root.img` WAS downloaded (so the refusal is the digest check, not a failed fetch).
- Wired into `scripts/test-images.sh` (scenarios group) and `.github/workflows/images.yml`
  (three invocations: good, bad, tampered); the serial log is now per-scenario
  (`serial-autoupdate-<scenario>.log`) so the workflow's artifact keeps all three.

**Verified here (macOS, no QEMU/mtools/e2fsprogs):** `bash -n` on every touched shell script
(build-payload.sh, the test-hook, test-autoupdate.sh, test-images.sh); the embedded Python
(the QMP stop and the server) compiles — `scripts/verify-embedded-python.sh` cannot run on
macOS bash 3.2 (it uses `mapfile`), so I ran its extraction + `py_compile` step directly, with
the same result. **Not verified here:** the VM scenarios themselves — they need the `images`
workflow (QEMU on Linux). The task stays `open` in doc 20 until that run is green.

**Known gap (not this PR's fix, from PR 3):** `build-payload.sh` seals the payload with
`release 2` but the copied slot-A root still says `/usr/lib/morpheos/release` = 1. None of the
three scenarios reads slot B's release file, so it does not affect the assertions, but a real
payload must carry the release its manifest claims.
