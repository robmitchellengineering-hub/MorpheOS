> Persisted from the planning room 2026-11-08 so the plan is git-tracked, not lost.

# MorpheOS-Secure — session handoff

Written after the goal was marked blocked (round 44). Copy the prompt block below into a
new session to continue without re-deriving anything.

---

## Copy-paste prompt for a new session

```
Continue the MorpheOS-Secure goal. Repo: github.com/robmitchellengineering-hub/MorpheOS-Secure,
local checkout /Users/mac/code/MorpheOS-Secure-NEW. State: 20/20 gates green (20 check + 20 self-test).
Already done and merged: loader milestone 4 (a real Rust no_std program loads and runs — the "morph-program"
crate), the 5-call API surface (report, apply_certified, yield, send_udp, send_rtp), the preempt fix,
GICv3 (the Pi 5's GIC-600, gated behind the gic-v3 feature), and the AMT RTP envelope (RTP X=1 + RFC 8285
0xBEDE header extension).

NEXT RUNG (do this): port the AirMatrix engine to a no_std loadable program so its suite can run on the
Core. The AirMatrix codebase is at /Users/mac/code/airmatrix-morpheos (private, an 11-crate Rust workspace:
amt-core has rtp.rs/aes67.rs/ptp.rs/correction.rs/dedupe.rs; engine is amt-engine/amt-matrix/amt-dsp/
amt-meter). Bring amt-core + amt-engine down to no_std (no std I/O; use the Core's allocator), make its RTP
packet the loadable program's payload (the send_rtp syscall already emits the AMT-shaped RTP), then run
cargo test --workspace's RTP/transport tests on the Core. The Pi 5 board is a later rung and needs Rob's
hardware; GICv3 and the UART-address-from-device-tree path are already done.

RULES: builds/boots run on Linux nodes (studio-linux2 at 192.168.28.128 via the mesh SSH proxy through
robmi@100.86.50.6, key ~/.ssh/mesh-admin, user rob), never the Mac. Every claim needs a falsifier and its
evidence. The network gate reads the CAPTURE, not the console. Never touch the everyday MorpheOS repo. The
Core is never called a kernel. One engine runs the show; two are live backup. State plainly what is not done.
```

---

## Where the work actually stands (verified in the session)

**20 gates green, 20 self-tests green** — every claim carries a falsifier.

| rung | state |
|---|---|
| 1. Loader M4 — a real compiled program runs | done — `morph-program` (a `no_std` Rust crate, linked at `0x40200000`) loads and returns 66 |
| 2. API surface | done — 5 calls: report, apply_certified, yield, send_udp, **send_rtp** |
| 5. Preempt cleanup | done — `core::hint::spin_loop()` compiled to `isb`; replaced with `yield`, 12/12 |
| Pi 5 GIC-600 (GICv3) | done + gated — timer/preemption/context-switch green under `-machine gic-version=3` |
| Engine sends RTP | done — loaded program's own stream (SSRC `0xC0FFEE00`) verified on the capture |
| AMT "real RTP" envelope | done — RTP X=1 + RFC 8285 `0xBEDE` extension |

Also fixed this session: the `send_udp` wire proof, and the loader's multi-segment PT_LOAD + `.bss`
zeroing + multi-sector (>512-byte) loading.

Merged PRs: #73 send_udp wire, #74 known-bugs, #75 preempt fix, #76/#77/#78 GICv3, #79 gic3 gate,
#80 self-test sweep, #81 loader multi-segment/.bss, #82 loader multi-sector, #83 compiled program,
#84 send_rtp, #85 AMT RTP extension.

## The concrete blocker

1. **Pi 5 board not in hand** — item 3 (RP1 UART at `0x1F22010000`) and item 4's "real hardware"
   verification both need the physical board. GICv3 is done; the UART address is read from the device
   tree, so the software path is ready but unproven on metal.
2. **Full AirMatrix engine port** — a large multi-session effort. The RTP/AMT envelope and a placeholder
   engine are done; what remains is `amt-core`'s AES67/PTP/correction/dedupe plus the matrix/meter/engine
   crates down to `no_std`, then its suite run on the Core.

## Rules that matter (from the goal + AGENTS.md)

- Builds and boots run on Linux nodes, never the Mac.
- The everyday `MorpheOS` repo is never touched.
- The Core is never called a kernel.
- Every claim carries its falsifier and its evidence; no claim before its check is green.
- A gate that passes while the machine prints FAIL is a broken gate.
- The network gate reads the CAPTURE, not the console.
- Audio plugins remain deferred (Rob). M7b (Lean) is open and not pretendable.
- Two engines are live backup: "so if one dies the show never drops."

## Key files to read first in the next session

- `SYSAPI.md` — the `svc #0` ABI (x0 = syscall num, x1/x2 = args, result in frame[0]).
- `LOADER-PLAN.md` — loader milestones.
- `KNOWN-BUGS.md` — resolved bugs and root causes.
- `scripts/*.sh` — the gates; `scripts/all.sh` runs the sweep, `scripts/all.sh --self-test` the mutations.
- `core/src/{main.rs,elf.rs,rtp.rs,gic.rs,net.rs,sched.rs}` — the Core.
- `program/src/main.rs` — the placeholder engine.
