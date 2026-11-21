"""Build every Acta app icon file from the scene in scripts/icon/tracer.py.

    python scripts/icon/build_icons.py            (from the repository root)

Needs Python 3.10+ with numpy, Pillow and resvg-py (scripts/icon/requirements.txt). Output is deterministic.

The icon: an ivory planet with a tilted brass ring passing in front of it, and a verdigris moon beyond the ring at
the upper right, on a dark ink plate. tracer.py renders it analytically (no generated pixels):
  - 1024 render, reduced with Lanczos for every size from 40 px up;
  - 16, 20, 22, 24 and 32 px rendered separately from the same scene with a thicker ring and a larger moon, so the
    ring stays at least 1 px of brass and the moon stays clear of it.
Windows and Linux: full-bleed superellipse plate (n = 5). macOS: the same scene on Apple's 824 px plate inside a
1024 canvas, with the continuous-curvature corner (r = 185.4) and no shadow.

Writes, under assets/icons/: acta.png (512), acta.ico (16 20 24 32 40 48 64 256), acta.icns (full iconset),
hicolor/<n>x<n>/apps/acta.png (16 22 24 32 48 64 128 256), hicolor/symbolic/apps/acta-symbolic.svg.
acta-symbolic.svg is hand-written (below) and copied into the hicolor tree.
"""
import io, os, struct, sys
import resvg_py
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tracer  # noqa: E402

OUT = os.path.join("assets", "icons")

# Small sizes: ring width in px, moon radius in plate widths.
SMALL = {16: (1.0, 0.094), 20: (1.1, 0.090), 22: (1.1, 0.088), 24: (1.15, 0.087), 32: (1.2, 0.085)}

# Monochrome silhouette on the same 64-unit layout: planet, ring (front arc knocked out of the planet, back arc
# hidden behind it with a gap), moon.
SYMBOLIC_SVG = """<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 64 64">
  <defs>
    <mask id="planet-cut"><rect width="64" height="64" fill="#fff"/>
      <path d="M7 35.2A21.8 6.7 0 0 0 50.6 35.2" transform="rotate(-18 28.8 35.2)" fill="none" stroke="#000" stroke-width="8"/>
    </mask>
    <mask id="back-cut"><rect width="64" height="64" fill="#fff"/><circle cx="28.8" cy="35.2" r="14.2" fill="#000"/></mask>
  </defs>
  <circle cx="28.8" cy="35.2" r="12.2" fill="currentColor" mask="url(#planet-cut)"/>
  <path d="M7 35.2A21.8 6.7 0 0 1 50.6 35.2" transform="rotate(-18 28.8 35.2)" fill="none" stroke="currentColor" stroke-width="3.5" mask="url(#back-cut)"/>
  <path d="M7 35.2A21.8 6.7 0 0 0 50.6 35.2" transform="rotate(-18 28.8 35.2)" fill="none" stroke="currentColor" stroke-width="3.5"/>
  <circle cx="51.5" cy="14.4" r="5.5" fill="currentColor"/>
</svg>
"""

# ---------- macOS plate ----------
K = [(-1.52866483, 0), (-1.08849299, 0), (-0.86840701, 0), (-0.63149399, 0.07491100),
     (-0.37282401, 0.16905900), (-0.16905900, 0.37282401), (-0.07491100, 0.63149399),
     (0, 0.86840701), (0, 1.08849299), (0, 1.52866483)]


def squircle(x0, y0, x1, y1, r):
    """Continuous-curvature rounded rect (the published reconstruction of Apple's corner)."""
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


def mac_mask(n):
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">'
           f'<path d="{squircle(100, 100, 924, 924, 185.4)}"/></svg>')
    data = resvg_py.svg_to_bytes(svg_string=svg, width=n, height=n)
    return Image.open(io.BytesIO(bytes(data))).convert("RGBA").getchannel("A")


# ---------- renders ----------
_cache = {}


def scene(n, plate):
    """The scene rendered at n x n; plate=True cuts the Windows/Linux superellipse."""
    key = (n, plate)
    if key not in _cache:
        if n in SMALL:
            px, mr = SMALL[n]
            s = dict(tracer.SPEC, ring_t=px / 2 / n, moon_r=mr)
            _cache[key] = tracer.render(n, s, ss=16, with_plate=plate)[0]
        elif n == 1024:
            _cache[key] = tracer.render(1024, tracer.SPEC, ss=2, with_plate=plate)[0]
        else:
            _cache[key] = scene(1024, plate).resize((n, n), Image.LANCZOS)
    return _cache[key]


def icon(n, mac=False):
    if not mac:
        return scene(n, True)
    # The art covers the plate plus one pixel all round, so the anti-aliased plate edge never shows the canvas.
    off = round(n * 100 / 1024) - 1
    side = n - 2 * off
    small_size = min(k for k in SMALL if k >= side) if side <= 32 else None
    art = scene(small_size or 1024, False).resize((side, side), Image.LANCZOS)
    canvas = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    canvas.paste(art, (off, off))
    canvas.putalpha(mac_mask(n))
    return canvas


# ---------- containers ----------
def png_bytes(im):
    # Truecolour RGBA: the render is smooth, so the whole set stays well under 1 MB without quantising
    # (a 256-colour palette visibly bands the planet).
    b = io.BytesIO(); im.save(b, "PNG", optimize=True); return b.getvalue()


def bmp_entry(im):
    """32-bit BGRA DIB with an all-zero AND mask, as Windows expects for small ICO entries."""
    n = im.width
    rows = b"".join(bytes(c for p in (im.getpixel((x, y)) for x in range(n)) for c in (p[2], p[1], p[0], p[3]))
                    for y in reversed(range(n)))
    mask_bits = b"\0" * (((n + 31) // 32) * 4 * n)
    header = struct.pack("<IiiHHIIiiII", 40, n, 2 * n, 1, 32, 0, len(rows) + len(mask_bits), 0, 0, 0, 0)
    return header + rows + mask_bits


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
    cache, chunks = {}, b""
    for t, n in ICNS_TYPES:
        if n not in cache:
            cache[n] = png_bytes(icon(n, mac=True))
        chunks += t.encode() + struct.pack(">I", 8 + len(cache[n])) + cache[n]
    open(path, "wb").write(b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks)


def main():
    def text(path, s):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        open(path, "w", encoding="utf-8", newline="\n").write(s)
    text(os.path.join(OUT, "acta-symbolic.svg"), SYMBOLIC_SVG)
    text(os.path.join(OUT, "hicolor", "symbolic", "apps", "acta-symbolic.svg"), SYMBOLIC_SVG)
    open(os.path.join(OUT, "acta.png"), "wb").write(png_bytes(icon(512)))
    write_ico(os.path.join(OUT, "acta.ico"), [16, 20, 24, 32, 40, 48, 64, 256])
    write_icns(os.path.join(OUT, "acta.icns"))
    for n in (16, 22, 24, 32, 48, 64, 128, 256):
        d = os.path.join(OUT, "hicolor", f"{n}x{n}", "apps"); os.makedirs(d, exist_ok=True)
        open(os.path.join(d, "acta.png"), "wb").write(png_bytes(icon(n)))
    print("wrote", OUT)


if __name__ == "__main__":
    sys.exit(main())
