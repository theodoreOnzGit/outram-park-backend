# SPDX-License-Identifier: AGPL-3.0-only
"""gh:#1385 / gh:#1299: draw upstream's boundary-point distance to the sphere.

Reads the per-point dumps written by `boundary_surface_distance <polyMesh>
<stl> <dump>` (columns: point id, x, y, z, distance) for the sphere runs and
writes one SVG: distance (mm) against polar angle theta = acos(z/|p|), one
dot per boundary point, one colour per generator. No third-party modules.

Run:  python3 -I plot_sphere_distance_svg.py out.svg cartesianMesh=a.txt pMesh=b.txt tetMesh=c.txt
"""
import math
import sys

COLOURS = {"cartesianMesh": "#1f77b4", "pMesh": "#d62728", "tetMesh": "#2ca02c"}
W, H, L, R, T, B = 760, 420, 70, 20, 40, 50


def read(path):
    out = []
    with open(path) as f:
        for line in f:
            _, x, y, z, d = line.split()
            x, y, z, d = float(x), float(y), float(z), float(d)
            r = math.sqrt(x * x + y * y + z * z)
            out.append((math.degrees(math.acos(max(-1.0, min(1.0, z / r)))), d * 1e3))
    return out


def main():
    target = sys.argv[1]
    series = [(a.split("=")[0], read(a.split("=")[1])) for a in sys.argv[2:]]
    dmax = max(d for _, s in series for _, d in s)
    ymax = math.ceil(dmax * 2) / 2 or 1.0
    sx = lambda t: L + (W - L - R) * t / 180.0
    sy = lambda d: H - B - (H - T - B) * d / ymax
    o = ['<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" font-family="sans-serif" font-size="12">' % (W, H),
         '<rect width="100%" height="100%" fill="white"/>',
         '<text x="%d" y="20" font-size="14">Upstream cfMesh, sphere R 3 m, maxCellSize 0.5: boundary point distance to the STL</text>' % L]
    for t in range(0, 181, 30):
        o.append('<line x1="%.1f" y1="%d" x2="%.1f" y2="%d" stroke="#ddd"/>' % (sx(t), T, sx(t), H - B))
        o.append('<text x="%.1f" y="%d" text-anchor="middle">%d</text>' % (sx(t), H - B + 16, t))
    k = 0.0
    while k <= ymax + 1e-9:
        o.append('<line x1="%d" y1="%.1f" x2="%d" y2="%.1f" stroke="#ddd"/>' % (L, sy(k), W - R, sy(k)))
        o.append('<text x="%d" y="%.1f" text-anchor="end">%.1f</text>' % (L - 6, sy(k) + 4, k))
        k += 0.5
    o.append('<text x="%.1f" y="%d" text-anchor="middle">polar angle theta (deg), 0 = north pole</text>' % ((L + W - R) / 2, H - 12))
    o.append('<text x="16" y="%.1f" transform="rotate(-90 16 %.1f)" text-anchor="middle">distance to surface (mm)</text>' % ((T + H - B) / 2, (T + H - B) / 2))
    for i, (name, s) in enumerate(series):
        c = COLOURS.get(name, "#000")
        for t, d in s:
            o.append('<circle cx="%.1f" cy="%.1f" r="1.6" fill="%s" fill-opacity="0.6"/>' % (sx(t), sy(d), c))
        mx = max(d for _, d in s)
        o.append('<circle cx="%d" cy="%d" r="4" fill="%s"/><text x="%d" y="%d">%s: n=%d, max %.3f mm</text>' % (W - 250, T + 12 + 16 * i, c, W - 242, T + 16 + 16 * i, name, len(s), mx))
    o.append("</svg>")
    with open(target, "w") as f:
        f.write("\n".join(o))


if __name__ == "__main__":
    main()
