# 2026-10-29 · DeepSeek · the geomesh was drawing a quarter of the design's edges

Rob, seeing the boot on the 2012 MacBook: *"the center geomesh has less sides than the original
shape it looks sparse and not good"*. Exact, and the cause is one number.

| | mesh |
|---|---|
| the design (`login.html:165`) | `new THREE.EdgesGeometry(new THREE.IcosahedronGeometry(GEOMESH_R, 1))` — the solid **subdivided once**: 42 vertices, **120 edges** |
| the software port (`splash.rs`) | `icosahedron_vertices()` + `icosahedron_edges()` — the bare solid: 12 vertices, **30 edges** |

So the boot splash drew a quarter of the design's edges. On a 1440×900 panel that reads as a sparse
cage rather than a mesh, which is what Rob saw.

## What changed

- **`subdivided_icosahedron()`** — the twenty faces are *derived*, not listed: in an icosahedron's
  1-skeleton a face is exactly a triple of vertices whose three pairwise distances are all the
  minimal edge length, so the face list cannot drift from `icosahedron_edges()`. Each face splits
  into four by its edge midpoints, each midpoint projected back onto the sphere (which is what
  Three.js does), giving 42 vertices and 120 edges.
- **The draw follows the mesh** instead of a hardcoded `30.0 * k`: the draw-in timing now derives
  from `edges.len()`, so it stays correct at any detail.
- **Four tests** pin it: 42/120, every vertex on the sphere, every edge the same length, no edge
  listed twice (a duplicate would draw one edge twice and finish the mesh early).

## Where the verification runs, and why not here

The test is a Rust build, and the mesh hook refused it on the Mac for the right reason: an idle node
can take it. Dispatched to `pacland-rog` — the Linux node, because this crate is Linux-only and a
Windows node would not exercise it.

**Not verified until that job returns.** The counts are arithmetic (twenty faces, nine edges each,
deduped to 120) but arithmetic is not a test run, and this repository's standard is the run.
