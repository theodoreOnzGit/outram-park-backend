"""Measurement markers: connected components of the marker's fill colour.

Discrete markers (BSAF draws measurements as filled squares / triangles) are
not curves, and a column-wise curve tracer is the wrong instrument for them:
it returns one y per column across the marker's width, so one measurement
becomes ~20 'points'. Here each marker is one connected blob of pixels
nearest the marker fill colour (nearest-colour classification against the
curve palette, capped), and its position is reported as the blob's
bounding-box centre (a MATLAB marker is centred on its data point).

Overlapping markers merge into one blob. A blob whose size exceeds the
isolated-marker size is reported as a CLUSTER and handled by
`cluster_apexes` below: for triangles, each marker's apex is a local
minimum of the blob's top edge, and its centre is apex + h/2, h being the
isolated triangle's bounding-box height.

usage: blobs.py IMAGE R,G,B XL XR YT YB [PALETTE r,g,b ...]
  XL..YB: the region to search (plot area, legend excluded by the caller).
  PALETTE: the other curve colours, so a pixel is assigned to the marker
  only if the marker colour is its nearest reference.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python3 -I drops it
from calib import load

MAXD = int(os.environ.get('MAXD', '60'))
BACKGROUND = [(255, 255, 255)]

def classify(raw, w, h, ref, others, box):
    xl, xr, yt, yb = box
    pal = [ref] + others + BACKGROUND
    mask = set(); cache = {}
    for y in range(yt, yb + 1):
        for x in range(xl, xr + 1):
            i = (y * w + x) * 3
            k = (raw[i], raw[i+1], raw[i+2])
            hit = cache.get(k)
            if hit is None:
                ds = [sum((k[c] - p[c]) ** 2 for c in range(3)) for p in pal]
                j = min(range(len(pal)), key=ds.__getitem__)
                hit = (j == 0 and ds[0] <= MAXD * MAXD)
                cache[k] = hit
            if hit: mask.add((x, y))
    return mask

def components(mask):
    seen = set(); out = []
    for p in mask:
        if p in seen: continue
        stack = [p]; seen.add(p); comp = []
        while stack:
            x, y = stack.pop(); comp.append((x, y))
            for q in ((x+1,y),(x-1,y),(x,y+1),(x,y-1),(x+1,y+1),(x-1,y-1),(x+1,y-1),(x-1,y+1)):
                if q in mask and q not in seen:
                    seen.add(q); stack.append(q)
        out.append(comp)
    return out

def bbox(c):
    xs = [p[0] for p in c]; ys = [p[1] for p in c]
    return min(xs), max(xs), min(ys), max(ys)

def cluster_apexes(c, tri_w, tri_h):
    """Triangle centres inside a merged blob.

    Top edge t(x) = min y of the blob in column x. A triangle apex is a
    column where t(x) is the minimum over a window of +-tri_w/2 (strict on
    one side, so a flat top yields one apex). Returns (x, apex+h/2) pairs.
    This can MISS a marker hidden wholly beneath others, and that loss is
    unrecoverable from the raster.
    """
    top = {}
    for x, y in c:
        if x not in top or y < top[x]: top[x] = y
    xs = sorted(top)
    half = max(2, int(tri_w * 0.45))
    pts = []
    for x in xs:
        win = [top[u] for u in range(x - half, x + half + 1) if u in top]
        left = [top[u] for u in range(x - half, x) if u in top]
        if top[x] == min(win) and (not left or top[x] < min(left)):
            pts.append((x, top[x] + tri_h / 2.0))
    return pts

if __name__ == '__main__':
    path = sys.argv[1]
    ref = tuple(map(int, sys.argv[2].split(',')))
    box = tuple(map(int, sys.argv[3:7]))
    others = [tuple(map(int, a.split(','))) for a in sys.argv[7:]]
    w, h, raw = load(path)
    comps = [c for c in components(classify(raw, w, h, ref, others, box)) if len(c) >= 12]
    comps.sort(key=lambda c: bbox(c)[0])
    for c in comps:
        x0, x1, y0, y1 = bbox(c)
        print(f'  blob n={len(c):5d} bbox x[{x0},{x1}] y[{y0},{y1}] w={x1-x0+1} h={y1-y0+1} '
              f'centre ({(x0+x1)/2:.1f},{(y0+y1)/2:.1f})')
