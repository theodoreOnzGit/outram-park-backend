#!/usr/bin/env python3
"""Draw the geometry of the gh:#1529 cases from the ASSEMBLED upstream meshes (the
polyMesh written by blockMesh) and the cylinder polygons of the Rust test. Output:
../images/geometry.svg. Usage: python3 -I draw_geometry.py"""
import math, os, re

W = "/home/user/upstream-build/gh1529"
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "images")
os.makedirs(OUT, exist_ok=True)
COL = {"xmin": "#e07b39", "xmax": "#c9a227", "ymin": "#4a90c2", "ymax": "#2f6f9f",
       "zmin": "#b03a2e", "zmax": "#3b7d3b"}


def read_list(path, kind):
    t = open(path).read()
    t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
    t = re.sub(r"//.*", "", t)
    t = t[t.index("}") + 1:]  # drop FoamFile
    return t


def points(case):
    t = read_list(f"{W}/{case}/constant/polyMesh/points", "p")
    return [tuple(map(float, m.split())) for m in re.findall(r"\(([^()]+)\)", t)][1:] if False else \
        [tuple(map(float, m.split())) for m in re.findall(r"\(([-0-9.e+ ]+)\)", t)]


def faces(case):
    t = read_list(f"{W}/{case}/constant/polyMesh/faces", "f")
    return [list(map(int, m.split())) for m in re.findall(r"\d+\(([\d ]+)\)", t)]


def patches(case):
    t = read_list(f"{W}/{case}/constant/polyMesh/boundary", "b")
    out = []
    for m in re.finditer(r"(\w+)\s*\{[^}]*?nFaces\s+(\d+);\s*startFace\s+(\d+);", t, flags=re.S):
        out.append((m.group(1), int(m.group(2)), int(m.group(3))))
    return out


def proj(p, s):
    x, y, z = p
    # oblique projection; z scaled to show the thin plate cases
    return (s * (x + 0.45 * y), -s * (z + 0.45 * y) * 1.0)


def panel(case, ox, oy, title, s=170, zscale=1.0):
    pts, fcs, pat = points(case), faces(case), patches(case)
    svg = [f'<text x="{ox}" y="{oy - 150}" font-size="13" fill="#222">{title}</text>']
    polys = []
    for name, n, start in pat:
        for f in fcs[start:start + n]:
            cx = sum(pts[i][0] for i in f) / len(f)
            cy = sum(pts[i][1] for i in f) / len(f)
            cz = sum(pts[i][2] for i in f) / len(f)
            depth = cy  # draw far (large y) first
            q = [proj((pts[i][0], pts[i][1], pts[i][2] * zscale), s) for i in f]
            polys.append((-depth, name, q))
    polys.sort(key=lambda t: t[0])
    for _, name, q in polys:
        d = " ".join(f"{ox + a:.1f},{oy + b:.1f}" for a, b in q)
        svg.append(f'<polygon points="{d}" fill="{COL[name]}" fill-opacity="0.55" stroke="#222" stroke-width="0.6"/>')
    return svg


def cyl(ox, oy, n, r1=0.5, r2=1.0, s=120):
    svg = [f'<text x="{ox - 130}" y="{oy - 135}" font-size="13" fill="#222">concentric cylinders, N = {n} facets (inner 0.5 m, outer 1.0 m)</text>']
    for r, c in ((r2, "#3b7d3b"), (r1, "#b03a2e")):
        p = [(ox + s * r * math.cos(2 * math.pi * k / n), oy - s * r * math.sin(2 * math.pi * k / n)) for k in range(n)]
        svg.append('<polygon points="' + " ".join(f"{a:.1f},{b:.1f}" for a, b in p) + f'" fill="none" stroke="{c}" stroke-width="2"/>')
    return svg


svg = ['<svg xmlns="http://www.w3.org/2000/svg" width="1100" height="560" viewBox="0 0 1100 560" font-family="sans-serif">',
       '<rect width="1100" height="560" fill="white"/>']
svg += panel("cubeA", 40, 200, "cubeA: unit cube, 4x4x4 cells, 96 boundary faces", 150)
svg += panel("cubeB", 330, 200, "cubeB: same mesh, agglomerated to 24 coarse faces (faces drawn fine)", 150)
svg += panel("plate4", 40, 470, "plate4: 1 x 1 x 0.25 m, 8x8x1 cells (zmin 1000 K, zmax 500 K, sides 0 K)", 150, 1.0)
svg += panel("plate16", 400, 470, "plate16: 1 x 1 x 0.0625 m", 150, 1.0)
svg += cyl(930, 230, 16)
leg = 640
for i, (n, c) in enumerate(COL.items()):
    svg.append(f'<rect x="{leg}" y="{20 + 16 * i}" width="12" height="12" fill="{c}" fill-opacity="0.55" stroke="#222"/><text x="{leg + 18}" y="{31 + 16 * i}" font-size="12">{n}</text>')
svg.append('<text x="700" y="40" font-size="12" fill="#222">Patches of the blockMesh cases (all in group viewFactorWall).</text>')
svg.append('<text x="700" y="56" font-size="12" fill="#222">Drawn from constant/polyMesh of the upstream cases.</text>')
svg.append('</svg>')
open(os.path.join(OUT, "geometry.svg"), "w").write("\n".join(svg))
print("wrote", os.path.join(OUT, "geometry.svg"))
