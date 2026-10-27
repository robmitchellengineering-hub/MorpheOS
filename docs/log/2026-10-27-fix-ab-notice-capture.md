# 2026-10-27 · DeepSeek · the notice screendump starts before the machine goes away

`main`'s required check "Try to Break It: A/B updates (x86 VM)" has been red since `8e8d039`,
repeatably (two runs, 2026-10-25 and 2026-10-27), on:

```
BrokenPipeError: [Errno 32] Broken pipe
  File "<stdin>", line 46, in <module>
  File "<stdin>", line 16, in cmd
```

Both scenarios that screendump the space notice — `test-full-disk.sh` and
`test-config-rollback.sh` — waited for `flow-notice: shown` in the serial log on a **1-second
poll**, then started Python, connected QMP and dumped the screen. The notice does not stay up
that long. From `serial-full-disk-2.log` (Measured, guest clock):

| event | guest time |
|---|---|
| `flow-notice: shown free space 0% …` | **4.124 s** |
| `reboot: Power down` | **4.937 s** |

So the notice is on screen for **0.81 s**, while the capture could not begin until ~1 s after it
appeared: the QMP socket was already closed, the first write raised `BrokenPipeError`, and
`set -e` turned a diagnostic into the job's exit status. Every other full-disk assertion printed
`ok` in that same run, which is what made it look like a product failure.

**Fix, both scripts:** the capture is started as soon as QEMU is launched, and it polls the
screen every 250 ms until the warning band is there, the machine goes away, or the timeout
expires. The serial-log wait is gone — the pixels are the evidence, which is what the check was
always meant to assert. A closed socket, an EOF mid-read, a socket that never appears and a
missing GPU all end in a **named verdict** printed by the capture and a `FAIL` line that says
whether the band was seen — never a traceback, and never a pass with the question unanswered.

**Verified here, without a VM** (the poller extracted from the script and run against a stand-in
QMP server):

| case | result |
|---|---|
| band appears on the 3rd dump | `3000 px of 1280x800 (frame 3)`, exit 0 |
| socket closes after 2 dumps | `best 0 px over 2 frames, the machine went away first`, exit 1 |
| socket closes at the handshake | `the QMP control socket closed during the handshake: …`, exit 1 |
| socket never appears | `the QMP socket never appeared`, exit 1 |

`bash -n` and `py_compile` clean on both scripts. **Not verified: the real VM.** The proof is
this pull request's own A/B job — that job *is* the harness, and no local machine here runs the
x86 image.

Note for the next person: `images.yml` runs the A/B job on every pull request with no path
filter (deliberately), so this is ~35 minutes before it reports.
