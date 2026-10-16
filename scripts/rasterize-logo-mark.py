#!/usr/bin/env python3
"""Rasterize design/logo-1-mark.svg (the geomesh mark) to a raw ARGB raster.

Writes crates/flow-compositor/src/logo-mark.argb: SIZE x SIZE pixels, 4 bytes each in
A, R, G, B order. The mark's line segments (matrix green #00ff41, stroke-width 3) are
supersampled 4x4 per pixel for anti-aliasing, on a fully transparent background.

WHY A COMMITTED RASTER. The compositor draws solid rectangles only; it has no SVG or
image loader, and the build must not reach for the network. Rasterizing the mark once,
here, and committing the bytes lets the splash embed the brand mark with a plain
`include_bytes!` — the same rule as HANDOFF.md's "no network at build time".

Run from the repository root:  python3 scripts/rasterize-logo-mark.py
"""
import math
import xml.etree.ElementTree as ET

SIZE = 256          # output square raster, in pixels
CONTENT = 240       # the mark's bounding box spans this many pixels (so it is centred
                    # with a little transparent padding for the anti-aliased edge)
SS = 4              # supersampling per axis (4x4 sub-samples per pixel)
STROKE = 3.0        # the SVG's stroke-width, in its own units
HALF = STROKE / 2.0
COLOR = (0, 255, 65)  # #00ff41 — matrix green, the brand's voice

svg = ET.parse("design/logo-1-mark.svg")
segs = []
for line in svg.iter("{http://www.w3.org/2000/svg}line"):
    segs.append(tuple(float(line.get(a)) for a in ("x1", "y1", "x2", "y2")))

xs = [c for s in segs for c in (s[0], s[2])]
ys = [c for s in segs for c in (s[1], s[3])]
minx, maxx = min(xs), max(xs)
miny, maxy = min(ys), max(ys)
cx, cy = (minx + maxx) / 2.0, (miny + maxy) / 2.0
span = max(maxx - minx, maxy - miny)
scale = CONTENT / span


def to_raster(sx, sy):
    """The raster coordinates of an SVG point: the mark's centre lands on the raster's."""
    return ((sx - cx) * scale + SIZE / 2.0, (sy - cy) * scale + SIZE / 2.0)


# Each segment, with its raster-space bounding box (stroke half-width plus a sub-pixel
# margin), so only the pixels near a line are tested against it.
seg_boxes = []
for (x1, y1, x2, y2) in segs:
    rx1, ry1 = to_raster(x1, y1)
    rx2, ry2 = to_raster(x2, y2)
    margin = HALF * scale + 1.0
    seg_boxes.append(
        (
            min(rx1, rx2) - margin,
            min(ry1, ry2) - margin,
            max(rx1, rx2) + margin,
            max(ry1, ry2) + margin,
            x1, y1, x2, y2,
        )
    )


def dist_to_seg(px, py, x1, y1, x2, y2):
    """Distance from a point to a line segment (the clamped projection gives round caps)."""
    dx = x2 - x1
    dy = y2 - y1
    l2 = dx * dx + dy * dy
    if l2 == 0.0:
        return math.hypot(px - x1, py - y1)
    t = ((px - x1) * dx + (py - y1) * dy) / l2
    t = max(0.0, min(1.0, t))
    return math.hypot(px - (x1 + t * dx), py - (y1 + t * dy))


# Coverage counts per pixel: how many of its sub-samples fall inside a stroke. A pixel
# crossed by two overlapping segments just saturates at SS*SS, which is the correct
# "covered" fraction since every stroke is the same green.
counts = [[0] * SIZE for _ in range(SIZE)]
for (bminx, bminy, bmaxx, bmaxy, x1, y1, x2, y2) in seg_boxes:
    px0 = max(0, int(math.floor(bminx)))
    px1 = min(SIZE - 1, int(math.ceil(bmaxx)))
    py0 = max(0, int(math.floor(bminy)))
    py1 = min(SIZE - 1, int(math.ceil(bmaxy)))
    for py in range(py0, py1 + 1):
        for px in range(px0, px1 + 1):
            hit = 0
            for j in range(SS):
                sy = (py + (j + 0.5) / SS - SIZE / 2.0) / scale + cy
                for i in range(SS):
                    sx = (px + (i + 0.5) / SS - SIZE / 2.0) / scale + cx
                    if dist_to_seg(sx, sy, x1, y1, x2, y2) <= HALF:
                        hit += 1
            counts[py][px] += hit

out = bytearray()
for py in range(SIZE):
    for px in range(SIZE):
        coverage = min(counts[py][px], SS * SS) / (SS * SS)
        alpha = round(255.0 * coverage)
        out += bytes((alpha, COLOR[0], COLOR[1], COLOR[2]))

with open("crates/flow-compositor/src/logo-mark.argb", "wb") as f:
    f.write(out)

opaque = sum(1 for py in range(SIZE) for px in range(SIZE) if counts[py][px] >= SS * SS)
print(
    f"wrote crates/flow-compositor/src/logo-mark.argb: "
    f"{SIZE}x{SIZE} ({len(out)} bytes) from {len(segs)} segments; "
    f"{opaque} fully-opaque pixels"
)
