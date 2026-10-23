# 2026-10-23 · DeepSeek · S3 auto-update — PR 2: the decision function and the fetcher

S3 step 3.2 and 3.3 (the second of its four PRs). PR 1 (release numbers + anti-rollback) is
already merged.

- `morph-update/src/decision.rs`: `decide(phase, running, server, tried, trial_armed)` — a pure
  function that returns fetch-or-skip. Every refusal is a `Skip` (never an error): a failed
  timer unit is an unhealthy boot. A table of tests covers every leaf.
- `morph-update/src/fetch.rs`: `fetch(base, release, dir, trusted)` downloads the manifest and
  its signature first, opens them (signature + release checked without hashing the large
  files), then downloads root.img/vmlinuz/initrd.img and hashes them. A server that serves a
  different release than it advertised is refused. `ureq` (rustls) is the client; the test
  server speaks plain HTTP.
- `tests/fetch.rs`: a one-shot local HTTP server proves the end-to-end fetch, and that a
  mis-advertised release is refused.

Next (PR 3): the `autoupdate` image (network base, the timer + drop-in), `build-payload.sh dir`,
and `test-autoupdate.sh good`; then PR 4: `bad` and `tampered`.
