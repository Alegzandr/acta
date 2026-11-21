"""Build every Acta app icon file from the geometry in this script.

    python scripts/icon/build_icons.py            (from the repository root)

Needs Python 3.10+ with `resvg-py` and `Pillow` (scripts/icon/requirements.txt). Writes, under assets/icons/:
  acta.svg, acta-macos.svg, acta-symbolic.svg      masters (hand-written geometry, below)
  acta.png                                         512 x 512 (Linux desktop entry, Velopack, updater window)
  acta.ico                                         16 20 24 32 40 48 64 256 (256 PNG-compressed)
  acta.icns                                        full macOS iconset, 16@1x .. 512@2x, PNG entries
  hicolor/<n>x<n>/apps/acta.png, hicolor/scalable/apps/acta.svg, hicolor/symbolic/apps/acta-symbolic.svg

The mark: an A on a 64-unit grid whose crossbar is a ledger rule. The inner bar touches the left leg and stops
2 units short of the right leg; the outer run starts 2 units past the right leg and leaves the plate. Every leg edge
has slope 0.4, so the bar meets the legs at exact tenths. 16, 20, 22, 24 and 32 px are hand-hinted from the tables
below (every pixel decided, glyph pixels fully opaque); other sizes are downscales of the vector.
"""
import io, os, struct, sys
import resvg_py
from PIL import Image

PLATE, INK, ACC = "#42413D", "#E4E3DF", "#72B5A2"
OUT = os.path.join("assets", "icons")

# ---------- vector masters (64 grid) ----------
A = "M10 52L26 12H34L50 52H40L30 27L20 52Z"
BAR_IN = "M26.4 36H31.6L34.8 44H23.2Z"
RUN = "M45.6 36H64V44H48.8Z"
A_AND_BAR = "M10 52L26 12H34L50 52H40L30 27L26.4 36H31.6L34.8 44H23.2L20 52Z"   # one colour: one polygon
PLATE_NOTCHED = ("M14 0H50A14 14 0 0 1 64 14V36H45.6L48.8 44H64V50A14 14 0 0 1 50 64H14"
                 "A14 14 0 0 1 0 50V14A14 14 0 0 1 14 0Z")

WIN_SVG = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 64 64">
  <rect width="64" height="64" rx="14" fill="{PLATE}"/>
  <path fill="{INK}" d="{A}"/>
  <path fill="{ACC}" d="{BAR_IN}"/>
  <path fill="{ACC}" d="{RUN}"/>
</svg>
"""

SYMBOLIC_SVG = f"""<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 64 64">
  <path fill="currentColor" fill-rule="evenodd" d="{PLATE_NOTCHED}{A_AND_BAR}"/>
</svg>
"""

# macOS: Apple's 824 px plate in a 1024 canvas, continuous-curvature corner, r = 185.4. Corner constants: the widely
# published reconstruction of the continuous rounded rect (corner extent 1.52866483 r). No shadow.
K = [(-1.52866483, 0), (-1.08849299, 0), (-0.86840701, 0), (-0.63149399, 0.07491100),
     (-0.37282401, 0.16905900), (-0.16905900, 0.37282401), (-0.07491100, 0.63149399),
     (0, 0.86840701), (0, 1.08849299), (0, 1.52866483)]


def squircle(x0, y0, x1, y1, r):
    n = lambda v: f"{v:.2f}".rstrip("0").rstrip(".")
    corners = [((x1, y0), (1, 0), (0, 1)), ((x1, y1), (0, 1), (-1, 0)),
               ((x0, y1), (-1, 0), (0, -1)), ((x0, y0), (0, -1), (1, 0))]
    d = ""
    for (cx, cy), u, v in corners:
        pts = [(cx + (p * u[0] + q * v[0]) * r, cy + (p * u[1] + q * v[1]) * r) for p, q in K]
        d += ("M" if not d else "L") + f"{n(pts[0][0])} {n(pts[0][1])}"
        for i in (1, 4, 7):
            d += "C" + " ".join(f"{n(x)} {n(y)}" for x, y in pts[i:i + 3])
    return d + "Z"


def mac_svg():
    plate = squircle(100, 100, 924, 924, 185.4)
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <defs><clipPath id="plate"><path d="{plate}"/></clipPath></defs>
  <path fill="{PLATE}" d="{plate}"/>
  <g clip-path="url(#plate)"><g transform="translate(100 100) scale(12.875)">
    <path fill="{INK}" d="{A}"/>
    <path fill="{ACC}" d="{BAR_IN}"/>
    <path fill="{ACC}" d="{RUN}"/>
  </g></g>
</svg>
"""


# ---------- hand hinting ----------
# Rows and columns from 0. T: apex row. L: left leg's outer column on each row from T to the baseline. The right leg
# mirrors it (right column = M - left column). Legs are vertical on the bar rows and on one guard row above and
# below, so both bar segments are true rectangles and no bar pixel touches a leg, even diagonally.
# Knockouts are exactly 1 px: inner bar ends 2 columns before the right leg, outer run starts 2 after it.
HINTS = {
    16: dict(T=2, L=[7, 6, 6, 5, 5, 4, 3, 3, 3, 3, 2, 2], M=15, w=2, bar=(9, 10), rad=3),
    20: dict(T=4, L=[9, 8, 8, 7, 7, 6, 5, 5, 5, 5, 4, 4], M=19, w=3, bar=(11, 12), rad=4),
    22: dict(T=4, L=[10, 9, 9, 8, 8, 7, 7, 6, 6, 6, 6, 5, 5, 4], M=21, w=3, bar=(12, 13), rad=5),
    24: dict(T=4, L=[10, 9, 9, 8, 8, 7, 7, 6, 5, 5, 5, 5, 5, 5, 4, 4], M=22, w=4, bar=(13, 15), rad=5),
    32: dict(T=6, L=[13, 12, 12, 11, 11, 10, 10, 9, 9, 8, 8, 7, 7, 7, 7, 7, 7, 6, 6, 5], M=29, w=5, bar=(18, 21),
             rad=7),
}


def hinted_glyph(n):
    """(x, y) -> 'ink' | 'acc' for the hinted size n."""
    h, px = HINTS[n], {}
    for i, l in enumerate(h["L"]):
        y = h["T"] + i
        for x in list(range(l, l + h["w"])) + list(range(h["M"] - l - h["w"] + 1, h["M"] - l + 1)):
            if l <= x <= h["M"] - l:
                px[(x, y)] = "ink"
    for y in range(h["bar"][0], h["bar"][1] + 1):
        l = h["L"][y - h["T"]]
        right_leg = h["M"] - l - h["w"] + 1
        for x in range(l + h["w"], right_leg - 1):
            px[(x, y)] = "acc"
        for x in range(h["M"] - l + 2, n):
            px[(x, y)] = "acc"
    return px


def coverage(n, rad, ss=8):
    cov = [[0.0] * n for _ in range(n)]
    for y in range(n):
        for x in range(n):
            k = 0
            for sy in range(ss):
                for sx in range(ss):
                    px, py = x + (sx + .5) / ss, y + (sy + .5) / ss
                    cx, cy = min(max(px, rad), n - rad), min(max(py, rad), n - rad)
                    k += (px - cx) ** 2 + (py - cy) ** 2 <= rad * rad
            cov[y][x] = k / ss / ss
    return cov


def rgb(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))


def hinted(n, symbolic=None):
    """Hand-hinted RGBA image. symbolic: an RGB colour; the plate in that colour with the glyph knocked out."""
    g, cov = hinted_glyph(n), coverage(n, HINTS[n]["rad"])
    im = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    for y in range(n):
        for x in range(n):
            if cov[y][x] <= 0:
                continue
            if symbolic:
                if (x, y) not in g:
                    im.putpixel((x, y), symbolic + (round(255 * cov[y][x]),))
            else:
                k = g.get((x, y))
                c = rgb(INK) if k == "ink" else rgb(ACC) if k == "acc" else rgb(PLATE)
                im.putpixel((x, y), c + (255 if k else round(255 * cov[y][x]),))
    return im


def vector(svg, n):
    data = resvg_py.svg_to_bytes(svg_string=svg, width=n, height=n)
    return Image.open(io.BytesIO(bytes(data))).convert("RGBA")


def icon(n):
    return hinted(n) if n in HINTS else vector(WIN_SVG, n)


# ---------- containers ----------
def png_bytes(im):
    b = io.BytesIO(); im.save(b, "PNG", optimize=True); return b.getvalue()


def bmp_entry(im):
    """32-bit BGRA DIB with an all-zero AND mask, as Windows expects for small ICO entries."""
    n = im.width
    rows = b"".join(bytes(c for p in (im.getpixel((x, y)) for x in range(n)) for c in (p[2], p[1], p[0], p[3]))
                    for y in reversed(range(n)))
    mask = b"\0" * (((n + 31) // 32) * 4 * n)
    header = struct.pack("<IiiHHIIiiII", 40, n, 2 * n, 1, 32, 0, len(rows) + len(mask), 0, 0, 0, 0)
    return header + rows + mask


def write_ico(path, sizes):
    blobs = [png_bytes(icon(n)) if n >= 256 else bmp_entry(icon(n)) for n in sizes]
    out = struct.pack("<HHH", 0, 1, len(sizes))
    off = 6 + 16 * len(sizes)
    for n, b in zip(sizes, blobs):
        out += struct.pack("<BBBBHHII", n % 256, n % 256, 0, 0, 1, 32, len(b), off)
        off += len(b)
    open(path, "wb").write(out + b"".join(blobs))


ICNS_TYPES = [("icp4", 16), ("icp5", 32), ("ic11", 32), ("icp6", 64), ("ic12", 64), ("ic07", 128),
              ("ic08", 256), ("ic13", 256), ("ic09", 512), ("ic14", 512), ("ic10", 1024)]


def write_icns(path):
    svg, chunks = mac_svg(), b""
    cache = {}
    for t, n in ICNS_TYPES:
        if n not in cache:
            cache[n] = png_bytes(vector(svg, n))
        chunks += t.encode() + struct.pack(">I", 8 + len(cache[n])) + cache[n]
    open(path, "wb").write(b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks)


def main():
    os.makedirs(OUT, exist_ok=True)
    w = lambda p, s: open(os.path.join(OUT, p), "w", encoding="utf-8", newline="\n").write(s)
    w("acta.svg", WIN_SVG); w("acta-macos.svg", mac_svg()); w("acta-symbolic.svg", SYMBOLIC_SVG)
    vector(WIN_SVG, 512).save(os.path.join(OUT, "acta.png"), optimize=True)
    write_ico(os.path.join(OUT, "acta.ico"), [16, 20, 24, 32, 40, 48, 64, 256])
    write_icns(os.path.join(OUT, "acta.icns"))
    for n in (16, 22, 24, 32, 48, 64, 128, 256):
        d = os.path.join(OUT, "hicolor", f"{n}x{n}", "apps"); os.makedirs(d, exist_ok=True)
        icon(n).save(os.path.join(d, "acta.png"), optimize=True)
    for sub, name, s in (("scalable", "acta.svg", WIN_SVG), ("symbolic", "acta-symbolic.svg", SYMBOLIC_SVG)):
        d = os.path.join(OUT, "hicolor", sub, "apps"); os.makedirs(d, exist_ok=True)
        open(os.path.join(d, name), "w", encoding="utf-8", newline="\n").write(s)
    print("wrote", OUT)


if __name__ == "__main__":
    sys.exit(main())
