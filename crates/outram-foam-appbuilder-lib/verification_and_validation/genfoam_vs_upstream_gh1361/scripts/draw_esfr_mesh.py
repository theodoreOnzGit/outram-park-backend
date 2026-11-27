#!/usr/bin/env python3
"""gh:#1911 / gh:#1361 round 71: draw the 3-D fluid mesh of the shipped 3D_SmallESFR (every cell coloured by its cellZone).

usage: python3 -I draw_esfr_mesh.py <polyMesh dir (points/faces/owner/neighbour/cellZones, .gz allowed)> <out dir>

Draws what the solver reads (the polyMesh), not what the dictionaries say: each cell is the convex hull of ITS OWN vertices projected onto
the slice plane, kept when the cell straddles the plane, filled by its cellZone. Writes SVGs: esfr_fluid_axial_xz.svg (plane y = y_mid),
esfr_fluid_axial_yz.svg (plane x = x_mid) and esfr_fluid_radial.svg (four planes z = fractions of the height). Prints the extents, the
cell count per zone, the cells in no zone, and the cells drawn per slice. No numpy/PIL here: pure python; the SVG is rasterised to PNG by
headless Chromium (see README round 71).
"""
import gzip, os, re, sys

def rd(path):
    for p in (path, path + '.gz'):
        if os.path.exists(p):
            return (gzip.open(p, 'rt') if p.endswith('.gz') else open(p)).read()
    raise FileNotFoundError(path)

def body(t):
    return re.sub(r'/\*.*?\*/', '', t, flags=re.S)

def list_body(t):
    m = re.search(r'\n(\d+)\s*\n?\(', t)
    s = t[m.end():]
    return s[:s.index('\n)')]

d, out = sys.argv[1], sys.argv[2]
pts = [tuple(float(v) for v in x.split()) for x in re.findall(r'\(([^()]+)\)', list_body(body(rd(os.path.join(d, 'points')))))]
faces = [[int(v) for v in x.split()] for x in re.findall(r'\d+\(([^()]+)\)', list_body(body(rd(os.path.join(d, 'faces')))))]
owner = [int(v) for v in list_body(body(rd(os.path.join(d, 'owner')))).split()]
neigh = [int(v) for v in list_body(body(rd(os.path.join(d, 'neighbour')))).split()]
ncell = max(owner) + 1
cellpts = [set() for _ in range(ncell)]
for f, o in enumerate(owner):
    cellpts[o].update(faces[f])
for f, n in enumerate(neigh):
    cellpts[n].update(faces[f])
zt = body(rd(os.path.join(d, 'cellZones')))
zone_of = [None] * ncell
zones = []
for m in re.finditer(r'\n\s*(\w+)\s*\{\s*type cellZone;\s*cellLabels\s+List<label>\s*(\d+)\s*\(([^)]*)\)', zt):
    zones.append(m.group(1))
    for c in map(int, m.group(3).split()):
        zone_of[c] = m.group(1)
xs = [p[0] for p in pts]; ys = [p[1] for p in pts]; zs = [p[2] for p in pts]
box = (min(xs), max(xs), min(ys), max(ys), min(zs), max(zs))
print('nPoints %d nCells %d' % (len(pts), ncell))
print('extent x %.4f..%.4f y %.4f..%.4f z %.4f..%.4f m' % box)
for z in zones:
    print('zone %-16s %d cells' % (z, sum(1 for c in zone_of if c == z)))
print('cells in no zone: %d' % sum(1 for c in zone_of if c is None))
COL = {'axialReflector': '#8da0cb', 'controlRod': '#e41a1c', 'radialReflector': '#66c2a5', 'innerCore': '#fc8d62',
       'follower': '#a6d854', 'outerCore': '#ffd92f', 'diagrid': '#999999', None: '#000000'}

def hull(p):
    p = sorted(set(p))
    if len(p) < 3:
        return p
    def cr(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lo = []
    for q in p:
        while len(lo) >= 2 and cr(lo[-2], lo[-1], q) <= 0:
            lo.pop()
        lo.append(q)
    up = []
    for q in reversed(p):
        while len(up) >= 2 and cr(up[-2], up[-1], q) <= 0:
            up.pop()
        up.append(q)
    return lo[:-1] + up[:-1]

def slice_polys(axis, val):
    """cells straddling coordinate `axis` == val, as (zone, hull of the other two coordinates)."""
    keep = [i for i in range(3) if i != axis]
    res = []
    for c in range(ncell):
        v = [pts[i] for i in cellpts[c]]
        lo = min(q[axis] for q in v); hi = max(q[axis] for q in v)
        if lo <= val < hi:
            res.append((zone_of[c], hull([(q[keep[0]], q[keep[1]]) for q in v])))
    return res

def panel(polys, ox, oy, w, h, ext, title):
    (u0, u1, v0, v1) = ext
    s = min(w / (u1 - u0), h / (v1 - v0))
    g = ['<text x="%g" y="%g" font-size="14" font-family="sans-serif">%s</text>' % (ox, oy - 6, title)]
    for z, poly in polys:
        pp = ' '.join('%.2f,%.2f' % (ox + (a - u0) * s, oy + (v1 - b) * s) for a, b in poly)
        g.append('<polygon points="%s" fill="%s" stroke="#222" stroke-width="0.2"/>' % (pp, COL.get(z, '#000')))
    g.append('<rect x="%g" y="%g" width="%g" height="%g" fill="none" stroke="#000"/>' % (ox, oy, (u1 - u0) * s, (v1 - v0) * s))
    return g, (u1 - u0) * s, (v1 - v0) * s

def legend(x, y):
    g = []
    for i, z in enumerate(zones):
        g.append('<rect x="%g" y="%g" width="14" height="14" fill="%s" stroke="#000"/><text x="%g" y="%g" font-size="13" font-family="sans-serif">%s</text>'
                 % (x, y + 20 * i, COL.get(z, '#000'), x + 20, y + 12 + 20 * i, z))
    return g

def write(name, W, H, items):
    with open(os.path.join(out, name), 'w') as f:
        f.write('<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" viewBox="0 0 %d %d"><rect width="100%%" height="100%%" fill="white"/>%s</svg>\n'
                % (W, H, W, H, ''.join(items)))

os.makedirs(out, exist_ok=True)
xm = 0.5 * (box[0] + box[1]); ym = 0.5 * (box[2] + box[3])
for axis_name, axis, val, ext, fn in [
        ('plane y = %.3f m (x-z)' % ym, 1, ym, (box[0], box[1], box[4], box[5]), 'esfr_fluid_axial_xz.svg'),
        ('plane x = %.3f m (y-z)' % xm, 0, xm, (box[2], box[3], box[4], box[5]), 'esfr_fluid_axial_yz.svg')]:
    polys = slice_polys(axis, val)
    g, w, h = panel(polys, 40, 40, 560, 900, ext, 'fluidRegion, ' + axis_name + ', %d cells cut' % len(polys))
    print(fn, 'cells drawn', len(polys))
    write(fn, int(w) + 280, int(h) + 80, g + legend(int(w) + 70, 60))
items = []
fr = [0.04, 0.30, 0.50, 0.85]
for k, fz in enumerate(fr):
    zc = box[4] + fz * (box[5] - box[4])
    polys = slice_polys(2, zc)
    g, w, h = panel(polys, 40 + (k % 2) * 560, 40 + (k // 2) * 560, 520, 520, (box[0], box[1], box[2], box[3]),
                    'radial slice z = %.3f m (%.0f %% of the height), %d cells cut' % (zc, 100 * fz, len(polys)))
    print('radial z=%.3f cells drawn %d zones %s' % (zc, len(polys), sorted(set(str(z) for z, _ in polys))))
    items += g
write('esfr_fluid_radial.svg', 1160, 1150, items + legend(900, 1060 - 20 * len(zones) + 60))
