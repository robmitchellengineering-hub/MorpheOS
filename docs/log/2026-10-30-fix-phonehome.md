# 2026-10-30 · DeepSeek · the machine sends its own report, so nobody has to carry it

Rob, after five builds spent guessing: *"Not setting up feed back loops for data."* Correct, and it is
the root of the night. A flashed image refused its own SSH key, its hardware report knew why — and the
report lived on the stick that was **inside the machine**. The data existed; there was no path from the
machine to the operator.

## What this adds

- **`/efi/morpheos-phonehome.txt`** — one line, a URL. **Absent by default**: no machine phones home unless
  someone with the stick in hand writes that file. No service is opened, no credential is carried.
- **`morph-phonehome.service`** — `After=morph-hwreport.service network-online.target`, `ConditionPathExists`
  that file, `ExecStart=-` (a diagnostic must never fail a trial boot, E11). It posts the newest report's
  `summary.txt`, `ssh.txt`, `net-live.txt`, `net.txt` and `journal-errors.txt` — the answers first, then the
  files that explain a refusal — as one small text body, capped at 60 KB per file.
- **`build.sh`** enables it and asserts both files are in the image, so a missing loop fails the build
  rather than being discovered on a machine.

## Why it would have changed that night

The refusal reason (`sshd -T`, the modes, the key fingerprint, the daemon's own lines) would have arrived in
the operator's chat on the first boot, instead of waiting for a stick to be carried across a room. Five
builds become one.

## Verified here, end to end

A receiver that accepts POST, and the real script pointed at it:

```
phonehome: sending 193 bytes to http://127.0.0.1:8124/ingest
phonehome: sent
--- received ---
machine: Macs-iMac.local
report: 003-TestMachine-a
===== summary.txt =====
===== ssh.txt =====
-- sshd -T --
permitrootlogin without-password
```

| check | result |
|---|---|
| no `/efi/morpheos-phonehome.txt` | "phone-home is off for this machine", exit 0 |
| a URL present, receiver accepting POST | **sent**, and the body arrived with `summary.txt` + `ssh.txt` |
| `bash -n` on the script and `build.sh` | clean |

**Not verified:** a real boot sending to a real receiver — that needs the next flash. The receiver here was
local and is the same code path.
