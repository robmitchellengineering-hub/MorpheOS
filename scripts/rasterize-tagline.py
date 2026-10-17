#!/usr/bin/env python3
"""Rasterize design/logo-tagline.svg to a raw ARGB raster (macOS only).

Writes crates/flow-compositor/src/logo-tagline.argb: W x H pixels, 4 bytes each in
A, R, G, B order. The wordmark is "Morphe" in the brand green gradient and "OS" in
white, on the brand background #05070d (the SVG bakes that background in), so the raster
is opaque and the compositor just blits it onto the same background.

WHY QLMANAGE. Unlike the geomesh mark (pure line geometry, rasterized by
rasterize-logo-mark.py), the wordmark is text, and there is no font renderer in the
compositor. This uses macOS QuickLook (qlmanage) — the only SVG-to-raster tool on this
machine — once, here, and commits the bytes; the build embeds them with `include_bytes!`.
QuickLook thumbnails to a square and pads with white, so this crops the #05070d content
band and halves it (a clean 2x box filter) to 400x120.

Run from the repository root, on macOS:  python3 scripts/rasterize-wordmark.py
"""
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path

TARGET_W, TARGET_H = 400, 30  # the SVG is 800x240; halved
BRAND_BLACK = (5, 7, 13)       # #05070d, the SVG's background rect


def decode_png(path):
    data = Path(path).read_bytes()
    i = 8
    w = h = ct = None
    idat = b""
    while i < len(data):
        (ln,) = struct.unpack(">I", data[i : i + 4])
        typ = data[i + 4 : i + 8]
        chunk = data[i + 8 : i + 8 + ln]
        if typ == b"IHDR":
            w, h, _bd, ct = struct.unpack(">IIBB", chunk[:10])
        elif typ == b"IDAT":
            idat += chunk
        elif typ == b"IEND":
            break
        i += 12 + ln
    raw = zlib.decompress(idat)
    ch = {0: 1, 2: 3, 4: 2, 6: 4}[ct]
    stride = w * ch

    def paeth(a, b, c):
        p = a + b - c
        pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
        return a if pa <= pb and pa <= pc else (b if pb <= pc else c)

    rows = []
    pos = 0
    prev = [0] * stride
    for _ in range(h):
        f = raw[pos]
        pos += 1
        line = list(raw[pos : pos + stride])
        pos += stride
        cur = [0] * stride
        for x in range(stride):
            a = cur[x - ch] if x >= ch else 0
            b = prev[x]
            c = prev[x - ch] if x >= ch else 0
            if f == 0:
                v = line[x]
            elif f == 1:
                v = (line[x] + a) & 0xFF
            elif f == 2:
                v = (line[x] + b) & 0xFF
            elif f == 3:
                v = (line[x] + (a + b) // 2) & 0xFF
            else:
                v = (line[x] + paeth(a, b, c)) & 0xFF
            cur[x] = v
        rows.append(cur)
        prev = cur

    def px(x, y):
        r = rows[y]
        if ch == 4:
            return tuple(r[x * 4 : x * 4 + 4])
        return tuple(r[x * 3 : x * 3 + 3])

    return w, h, px


def render_svg(svg, size=800):
    """Render an SVG to a PNG with QuickLook; return the PNG path."""
    tmp = Path(tempfile.mkdtemp())
    (tmp / svg.name).write_bytes(Path(svg).read_bytes())
    subprocess.run(
        ["qlmanage", "-t", "-s", str(size), "-o", str(tmp), str(tmp / svg.name)],
        check=True,
        capture_output=True,
    )
    return tmp / (svg.name + ".png")


def main():
    svg = Path("design/logo-tagline.svg")
    png = render_svg(svg)
    w, h, px = decode_png(png)

    # Crop the #05070d content band (the SVG's background rect, 800x240). QuickLook put
    # white padding below it; the band is the rows that hold the brand background.
    band = [y for y in range(h) if any(px(x, y)[:3] == BRAND_BLACK for x in range(w))]
    y0, y1 = min(band), max(band)
    # The SVG's rect is exactly 240 units; keep that many rows for the right aspect.
    y1 = y0 + 60 - 1

    out = bytearray()
    for ty in range(TARGET_H):
        for tx in range(TARGET_W):
            # 2x box filter: average the 2x2 source pixels for each output pixel.
            sx = tx * 2
            sy = y0 + ty * 2
            rs = gs = bs = 0
            for dy in range(2):
                for dx in range(2):
                    p = px(sx + dx, sy + dy)[:3]
                    rs += p[0]
                    gs += p[1]
                    bs += p[2]
            r, g, b = rs // 4, gs // 4, bs // 4
            # Key out the #05070d background so the compositor draws only the text and
            # lets what is behind it show through. The background is (5,7,13); every text
            # pixel (even the gradient's dark end) has some channel well above 20, and the
            # anti-aliased edge keeps its colour, which is correct over the same background.
            a = 255 if (r >= 20 or g >= 20 or b >= 20) else 0
            out += bytes((a, r, g, b))

    Path("crates/flow-compositor/src/logo-tagline.argb").write_bytes(out)
    print(
        f"wrote crates/flow-compositor/src/logo-tagline.argb: "
        f"{TARGET_W}x{TARGET_H} ({len(out)} bytes), cropped rows {y0}..{y1}"
    )


if __name__ == "__main__":
    main()
