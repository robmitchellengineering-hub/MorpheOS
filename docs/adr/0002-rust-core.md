# 0002 · Rust for all core services

**Status:** Accepted, 2026-10-10

## Context
The core services run as PID 1, handle every byte of user data, and must fit a 512 MB
Pi as well as a server.

## Decision
`morphd`, Strata, the heal engine, capsules, the governor, the mesh and the shell are
written in Rust. eBPF is used for scheduler and policy hooks, and WASM/WASI for
third-party extensions. C or Zig only where firmware interfaces require it.

## Consequences
- Memory safety and no GC pauses, with one language from embedded to server.
- Morpheus (JavaScript/Node) logic is ported as logic, not linked as code.
