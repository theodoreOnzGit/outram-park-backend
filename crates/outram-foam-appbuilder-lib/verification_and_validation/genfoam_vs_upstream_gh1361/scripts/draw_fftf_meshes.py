#!/usr/bin/env python3
"""gh:#1870 / gh:#1361 round 62: draw the meshes of the shipped 2D_FFTF (every region, every cell coloured by its cellZone).

usage: python3 -I draw_fftf_meshes.py <case dir holding constant/<region>/polyMesh> <out dir> [region mesh dir overrides...]

Draws what the solver reads, not what the dictionaries say: each cell is the polygon of ITS OWN front-patch face (the wedge/empty patch
of the 2-D extrusion) in the x-z plane, filled by the cellZone the cell belongs to (cellZones file), with the zone names as a legend
and the bounding box in metres. Also prints per region: nCells, nCells in a zone, cells in no zone, and the x-z extent.
Writes <out dir>/fftf_<region>_zones.svg (an SVG image; this machine has no working PIL or rsvg to rasterise).
"""
import gzip, os, re, sys

def rd(path):
    for p in (path, path + '.gz'):
        if os.path.exists(p):
            return (gzip.open(p, 'rt') if p.endswith('.gz') else open(p)).read()
    raise FileNotFoundError(path)

def body(t):
    t = re.sub(r'/\*.*?\*/', '', t, flags=re.S)
    return t

def int_list(t):
    m = re.search(r'\n(\d+)\s*\n?\(', t)
    n = int(m.group(1)); s = t[m.end():]
    return n, s[:s.index('\n)')]

def read_points(d):
    t = body(rd(os.path.join(d, 'points')))
    n, b = int_list(t)
    return [tuple(float(v) for v in x.split()) for x in re.findall(r'\(([^()]+)\)', b)]

def read_faces(d):
    t = body(rd(os.path.join(d, 'faces')))
    n, b = int_list(t)
    return [[int(v) for v in x.split()] for x in re.findall(r'\d+\(([^()]+)\)', b)]

def read_owner(d):
    t = body(rd(os.path.join(d, 'owner')))
    n, b = int_list(t)
    return [int(v) for v in b.split()]

def read_boundary(d):
    t = body(rd(os.path.join(d, 'boundary')))
    out = []
    for m in re.finditer(r'\n\s{4}(\w+)\s*\n\s{4}\{(.*?)\n\s{4}\}', t, flags=re.S):
        nf = int(re.search(r'nFaces\s+(\d+)', m.group(2)).group(1))
        sf = int(re.search(r'startFace\s+(\d+)', m.group(2)).group(1))
        ty = re.search(r'type\s+(\w+)', m.group(2)).group(1)
        out.append((m.group(1), ty, sf, nf))
    return out

def read_zones(d):
    t = body(rd(os.path.join(d, 'cellZones')))
    zs = []
    for m in re.finditer(r'\n\s*(\w+)\s*\n\s*\{\s*type\s+cellZone;\s*cellLabels\s+List<label>\s*(\d+)\s*\(([^)]*)\)', t, flags=re.S):
        zs.append((m.group(1), [int(v) for v in m.group(3).split()]))
    return zs

PAL = [(230,25,75),(60,180,75),(255,225,25),(0,130,200),(245,130,48),(145,30,180),(70,240,240),(240,50,230),(210,245,60),(250,190,212),
       (0,128,128),(220,190,255),(170,110,40),(255,250,200),(128,0,0),(170,255,195),(128,128,0),(255,215,180),(0,0,128),(128,128,128),
       (90,90,90),(200,120,120),(120,200,120),(120,120,200),(200,200,120),(30,30,30),(160,60,110),(60,160,110),(110,60,160)]

def draw(region, d, out):
    pts = read_points(d); faces = read_faces(d); owner = read_owner(d); bnd = read_boundary(d); zones = read_zones(d)
    ncell = max(owner) + 1
    front = [b for b in bnd if b[1] in ('wedge', 'empty')]
    # the front patch is the first wedge/empty patch (the back one is its mirror image)
    name, ty, sf, nf = front[0]
    extra = [b for b in bnd if b[0].endswith('FrontAndBack') or b[0] == 'Front']
    zone_of = [-1] * ncell
    for zi, (zn, cl) in enumerate(zones):
        for c in cl:
            zone_of[c] = zi
    xs = [p[0] for p in pts]; zs_ = [p[2] for p in pts]
    x0, x1, z0, z1 = min(xs), max(xs), min(zs_), max(zs_)
    W = 1500; sc = W / max(x1 - x0, 1e-9); H = int((z1 - z0) * sc) + 1
    rows = (len(zones) + 2) // 3
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W+40}" height="{H+60+22*rows}"><rect width="100%" height="100%" fill="white"/>']
    svg.append(f'<text x="22" y="14" font-size="13" font-family="sans-serif">{region}: {ncell} cells, {len(zones)} cellZones, patch {name} ({nf} faces); x {x0:.3f}..{x1:.3f} m, z {z0:.3f}..{z1:.3f} m</text>')
    drawn = set(owner[f] for f in range(sf, sf + nf))
    for f in list(range(sf, sf + nf)) + [f for b in extra for f in range(b[2], b[2] + b[3]) if owner[f] not in drawn and not drawn.add(owner[f])]:
        c = owner[f]; col = PAL[zone_of[c] % len(PAL)] if zone_of[c] >= 0 else (0, 0, 0)
        poly = ' '.join(f'{(pts[v][0]-x0)*sc+20:.2f},{(z1-pts[v][2])*sc+30:.2f}' for v in faces[f])
        svg.append(f'<polygon points="{poly}" fill="rgb{col}" stroke="#282828" stroke-width="0.3"/>')
    ly = H + 45
    for zi, (zn, cl) in enumerate(zones):
        cx = 20 + (zi % 3) * ((W + 40) // 3); cy = ly + (zi // 3) * 22
        svg.append(f'<rect x="{cx}" y="{cy}" width="14" height="14" fill="rgb{PAL[zi % len(PAL)]}" stroke="black"/><text x="{cx+20}" y="{cy+12}" font-size="12" font-family="sans-serif">{zn} ({len(cl)})</text>')
    svg.append('</svg>')
    sp = os.path.join(out, f'fftf_{region}_zones.svg'); open(sp, 'w').write('\n'.join(svg))
    nz = sum(1 for z in zone_of if z < 0)
    over = ncell and sum(len(cl) for _, cl in zones) - (ncell - nz)
    print(f'{region}: cells drawn {len(drawn)}, nCells {ncell}, front patch {name} {nf} faces, cellZones {len(zones)}, cells in no zone {nz}, zone-membership overlap {over}, x {x0:.4f}..{x1:.4f}, z {z0:.4f}..{z1:.4f}')
    for zn, cl in zones:
        print(f'  {zn}: {len(cl)}')

if __name__ == '__main__':
    case, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    for region in ('fluidRegion', 'neutroRegion'):
        draw(region, os.path.join(case, 'constant', region, 'polyMesh'), out)
