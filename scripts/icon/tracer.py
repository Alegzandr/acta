"""Analytic renderer (the source of truth of the app icon) for the Acta Companion icon: ivory planet, tilted brass tube ring, verdigris moon, ink plate.

Orthographic camera looking along -z; image coordinates x right, y down, z toward the viewer; lengths in units of
the plate width W (0..1). numpy + Pillow only.
"""
import math
import numpy as np
from PIL import Image, ImageFilter

HEX = lambda h: np.array([int(h[i:i + 2], 16) for i in (1, 3, 5)], dtype=np.float64) / 255.0

SPEC = dict(
    planet_c=(0.45, 0.55), planet_r=0.19,
    ring_R=0.34, ring_t=0.009, ring_phi=18.0, ring_ratio=0.31,      # tube radius = thickness / 2
    moon_c=(0.805, 0.225), moon_r=0.075,
    light=(135.0, 45.0),                                           # azimuth (deg, 135 = upper left), elevation
    shadow_off=(0.02, 0.03), shadow_blur=0.04, shadow_alpha=0.35,
    ring_shadow_drop=0.02,
    plate_top="#272A2C", plate_bottom="#1E2123", plate_n=5.0,
    planet_lit="#EDE6D6", planet_dark="#9C927E",
    ring_lit="#E6CB8E", ring_base="#B8955A", ring_dark="#6E5530",
    moon_lit="#A9D6C8", moon_base="#72B5A2", moon_mottle="#3F7466",
)

# ---------- noise ----------
_rng = np.random.default_rng(7)
_PERM = _rng.permutation(256)
_VALS = _rng.random(256)


def _hash(ix, iy, iz):
    return _VALS[_PERM[(_PERM[(_PERM[ix & 255] + iy) & 255] + iz) & 255]]


def vnoise(p):
    i = np.floor(p).astype(np.int64)
    f = p - i
    f = f * f * (3 - 2 * f)
    out = 0
    for dx in (0, 1):
        for dy in (0, 1):
            for dz in (0, 1):
                w = (f[..., 0] if dx else 1 - f[..., 0]) * (f[..., 1] if dy else 1 - f[..., 1]) * (f[..., 2] if dz else 1 - f[..., 2])
                out = out + w * _hash(i[..., 0] + dx, i[..., 1] + dy, i[..., 2] + dz)
    return out


def fbm(p, octaves=4):
    s, a, tot = 0, 1.0, 0
    for o in range(octaves):
        s = s + a * vnoise(p * (2 ** o))
        tot += a
        a *= 0.5
    return s / tot


def smooth(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0, 1)
    return t * t * (3 - 2 * t)


# ---------- geometry ----------
def ring_frame(s):
    phi = math.radians(s["ring_phi"])
    el = math.asin(s["ring_ratio"])
    u = np.array([math.cos(phi), -math.sin(phi), 0.0])                 # major axis, rising to the right
    w = np.array([math.sin(phi), math.cos(phi), 0.0])                  # in-screen perpendicular, pointing down
    v = w * math.sin(el) + np.array([0, 0, 1.0]) * math.cos(el)        # lower half comes toward the viewer
    return u, v


def ring_points(s, n=4000):
    u, v = ring_frame(s)
    th = np.linspace(0, 2 * math.pi, n, endpoint=False)
    c = np.array([*s["planet_c"], 0.0])
    return c + s["ring_R"] * (np.cos(th)[:, None] * u + np.sin(th)[:, None] * v)


def render(size, s=SPEC, ss=2, with_plate=True):
    N = size * ss
    px = (np.arange(N) + 0.5) / N
    X, Y = np.meshgrid(px, px)
    az, el = (math.radians(a) for a in s["light"])
    L = np.array([math.cos(el) * math.cos(az), -math.cos(el) * math.sin(az), math.sin(el)])
    L = L / np.linalg.norm(L)
    H = L + np.array([0, 0, 1.0]); H /= np.linalg.norm(H)

    zbuf = np.full((N, N), -np.inf)
    obj = np.zeros((N, N), np.int8)          # 0 plate, 1 planet, 2 ring, 3 moon
    nrm = np.zeros((N, N, 3))
    pos = np.zeros((N, N, 3))

    def sphere(c, r, oid):
        d2 = (X - c[0]) ** 2 + (Y - c[1]) ** 2
        m = d2 < r * r
        z = np.sqrt(np.maximum(r * r - d2, 0))
        upd = m & (z > zbuf)
        zbuf[upd] = z[upd]; obj[upd] = oid
        nrm[upd] = np.stack([(X - c[0]) / r, (Y - c[1]) / r, z / r], -1)[upd]
        pos[upd] = np.stack([X - c[0], Y - c[1], z], -1)[upd] / r
        return m

    planet_full = sphere(s["planet_c"], s["planet_r"], 1)
    moon_full = sphere(s["moon_c"], s["moon_r"], 3)

    # ring: a swept sphere along the tilted circle, z-buffered stamp by stamp
    t = s["ring_t"]
    ring_full = np.zeros((N, N), bool)
    rad = int(math.ceil(t * N)) + 1
    for p in ring_points(s, n=max(2000, int(8 * math.pi * s["ring_R"] * N))):
        cx, cy = p[0] * N, p[1] * N
        x0, x1 = max(int(cx) - rad, 0), min(int(cx) + rad + 1, N)
        y0, y1 = max(int(cy) - rad, 0), min(int(cy) + rad + 1, N)
        if x0 >= x1 or y0 >= y1:
            continue
        sx, sy = X[y0:y1, x0:x1] - p[0], Y[y0:y1, x0:x1] - p[1]
        d2 = sx * sx + sy * sy
        m = d2 < t * t
        dz = np.sqrt(np.maximum(t * t - d2, 0))
        z = p[2] + dz
        zb = zbuf[y0:y1, x0:x1]
        upd = m & (z > zb)
        ring_full[y0:y1, x0:x1] |= m
        zb[upd] = z[upd]
        obj[y0:y1, x0:x1][upd] = 2
        nn = np.stack([sx / t, sy / t, dz / t], -1)
        sub = nrm[y0:y1, x0:x1]; sub[upd] = nn[upd]

    lam = np.clip((nrm * L).sum(-1), 0, 1)
    spec = np.clip((nrm * H).sum(-1), 0, 1)
    col = np.zeros((N, N, 3))

    # planet: matte ivory with faint pores; ring shadow dropped below the front arc
    m = obj == 1
    tone = smooth(0.0, 0.95, lam)[..., None]
    pc = HEX(s["planet_dark"]) * (1 - tone) + HEX(s["planet_lit"]) * tone
    pores = 1 + 0.022 * (fbm(pos * 38.0, 3) - 0.5)
    pc = pc * pores[..., None] + 0.06 * (spec ** 24)[..., None]
    ring_front = (obj == 2) & (zbuf > 0)
    shadow_src = Image.fromarray((ring_front * 255).astype(np.uint8))
    drop = int(round(s["ring_shadow_drop"] * N))
    sh = Image.new("L", (N, N), 0); sh.paste(shadow_src, (int(round(0.006 * N)), drop))
    sh = np.asarray(sh.filter(ImageFilter.GaussianBlur(0.004 * N)), np.float64) / 255.0
    pc = pc * (1 - 0.42 * sh)[..., None]
    col[m] = pc[m]

    # ring: brass tube
    m = obj == 2
    lt = lam[..., None]
    rc = np.where(lt < 0.5, HEX(s["ring_dark"]) + (HEX(s["ring_base"]) - HEX(s["ring_dark"])) * (lt / 0.5),
                  HEX(s["ring_base"]) + (HEX(s["ring_lit"]) - HEX(s["ring_base"])) * ((lt - 0.5) / 0.5))
    rc = rc + 0.45 * (spec ** 40)[..., None] * np.array([1.0, 0.95, 0.82])
    col[m] = rc[m]

    # moon: mottled verdigris
    m = obj == 3
    mott = smooth(0.45, 0.62, fbm(pos * 4.2 + 3.1, 4))[..., None]
    base = HEX(s["moon_base"]) * (1 - mott) + HEX(s["moon_mottle"]) * mott
    lit = HEX(s["moon_lit"]) * (1 - mott) + HEX(s["moon_mottle"]) * 1.25 * mott
    tone = smooth(0.0, 1.0, lam)[..., None]
    mc = np.where(tone < 0.5, base * (0.5 + tone), base + (lit - base) * ((tone - 0.5) * 2))
    mc = mc + 0.22 * (spec ** 30)[..., None]
    col[m] = mc[m]

    # plate: vertical gradient, fine grain, drop shadow of everything to the lower right
    g = Y[..., None]
    plate = HEX(s["plate_top"]) * (1 - g) + HEX(s["plate_bottom"]) * g
    plate = plate * (1 + 0.012 * (np.random.default_rng(N).random((N, N)) - 0.5))[..., None]   # grain, seeded by size
    union = Image.fromarray(((planet_full | moon_full | ring_full) * 255).astype(np.uint8))
    so = Image.new("L", (N, N), 0)
    so.paste(union, (int(round(s["shadow_off"][0] * N)), int(round(s["shadow_off"][1] * N))))
    so = np.asarray(so.filter(ImageFilter.GaussianBlur(s["shadow_blur"] * N / 2)), np.float64) / 255.0
    plate = plate * (1 - s["shadow_alpha"] * so)[..., None]
    m = obj == 0
    col[m] = plate[m]

    rgb = Image.fromarray((np.clip(col, 0, 1) * 255 + 0.5).astype(np.uint8), "RGB")
    alpha = None
    if with_plate:
        nx, ny = np.abs(X * 2 - 1), np.abs(Y * 2 - 1)
        a = (nx ** s["plate_n"] + ny ** s["plate_n"]) <= 1.0
        alpha = Image.fromarray((a * 255).astype(np.uint8))
    im = rgb.convert("RGBA")
    if alpha is not None:
        im.putalpha(alpha)
    if ss > 1:
        im = im.reduce(ss)
    return im, dict(planet=planet_full, moon=moon_full, ring=ring_full, N=N)
