"""Measured data drawn as filled markers -> marker-centre pixel positions.

Markers are not curves; see blobs.py for why a column tracer is the wrong
instrument. The marker fill is isolated by nearest-colour classification
against every curve colour (capped, MAXD), and the curves are drawn ON TOP
of the markers in these figures, so a marker is often cut by a curve. The
known marker size is measured once from the least-occluded marker and then
used to place partly hidden markers from the edges that ARE adjacent to
paper (an edge next to a curve pixel may be an occlusion, not the marker's
real edge, and is not used).

  square  : fragments within one marker size are merged; on each axis the
            centre is the bbox centre if the extent matches the size,
            otherwise the paper-adjacent edge +- size/2.
  triangle: apex = local minimum of the cloud's top edge with paper above
            it and both flanks falling away; centre = apex + H/2 (basis
            'apex'). Flat bottom-edge runs with paper below that no apex
            accounts for give centre = base - H/2 (basis 'base'; a run
            longer than one marker is several merged markers and is
            sampled every W/2 with x uncertain by +-W/2, basis 'base-merged').

Centre = bounding-box centre of the filled marker (MATLAB centres markers
on their bounding box). The alternative, a triangle's centroid, sits H/6
lower; that is reported as a systematic uncertainty, not applied.

usage: markers.py IMAGE square|triangle|envelope R,G,B W H XL XR YT YB LEGEND(xl,xr,yt,yb|-) PALETTE...
prints CSV: px_x,px_y,basis
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python3 -I drops it
from calib import load
from blobs import classify, components, bbox

# the triangles carry a ~2-px light-green outline and every marker an
# anti-aliased rim, so 'paper beyond the edge' is tested GAP px out
GAP = int(os.environ.get('GAP', '5'))

def paper(raw, w, x, y):
    i = (y * w + x) * 3
    return min(raw[i], raw[i+1], raw[i+2]) > 225

def squares(mask, raw, w, W, H):
    comps = components(mask)
    boxes = [bbox(c) for c in comps if len(c) >= 6]
    merged = []
    for b in sorted(boxes):
        for k, m in enumerate(merged):
            u = (min(m[0], b[0]), max(m[1], b[1]), min(m[2], b[2]), max(m[3], b[3]))
            if u[1] - u[0] <= W + 2 and u[3] - u[2] <= H + 2:
                merged[k] = u; break
        else:
            merged.append(b)
    out = []
    for x0, x1, y0, y1 in merged:
        if (x1 - x0 + 1) * (y1 - y0 + 1) < 0.25 * W * H: continue
        def axis(a0, a1, size, lo_paper, hi_paper):
            if a1 - a0 + 1 >= size - 2: return (a0 + a1) / 2.0, 'full'
            if lo_paper and not hi_paper: return a0 + (size - 1) / 2.0, 'anchored-low'
            if hi_paper and not lo_paper: return a1 - (size - 1) / 2.0, 'anchored-high'
            return (a0 + a1) / 2.0, 'partial'
        mx = (x0 + x1) // 2; my = (y0 + y1) // 2
        cx, bx = axis(x0, x1, W, paper(raw, w, x0 - GAP, my), paper(raw, w, x1 + GAP, my))
        cy, by = axis(y0, y1, H, paper(raw, w, mx, y0 - GAP), paper(raw, w, mx, y1 + GAP))
        out.append((cx, cy, f'square-x:{bx}-y:{by}'))
    return out

def triangles(mask, raw, w, W, H, fill=None):
    top, bot = {}, {}
    for x, y in mask:
        top[x] = min(top.get(x, 1 << 30), y)
    out = []
    xs = sorted(top)
    # Where markers overlap (one every ~7 px on the 15-23 h plateau of
    # Fig 2.1b) and curves cross their lower parts, the flanks are hidden;
    # only the tips stand clear. An apex is therefore: the top edge's local
    # minimum within +-2 columns, paper GAP px above it, and at least 3 of
    # the 4 pixels below it inked (a tip has depth; a speck does not).
    # Apexes closer than 3 px are one marker.
    for x in xs:
        t = top[x]
        if not paper(raw, w, x, t - GAP): continue
        near = [top[u] for u in range(x - 2, x + 3) if u in top]
        if min(near) < t: continue
        if (x - 1) in top and top[x - 1] == t: continue      # one per flat top
        depth = sum(1 for k in range(1, 5) if (x, t + k) in mask or not paper(raw, w, x, t + k))
        if depth < 3: continue
        if out and x - out[-1][0] < 3: continue
        # a real tip has the marker FILL (first colour) within 12 rows below
        # it; anti-aliased blends of two curves that happen to classify as
        # the light rim colour do not
        if fill is not None and not any((u, t + k) in fill for u in range(x - 4, x + 5) for k in range(0, 13)):
            continue
        out.append((float(x), t + H / 2.0, 'apex'))
    # bottom edges: per column, the lowest fill pixel of every vertical run
    runs = {}
    for x, y in mask:
        if (x, y + 1) not in mask and paper(raw, w, x, y + GAP):
            runs.setdefault(y, []).append(x)
    for y, cols in sorted(runs.items()):
        cols.sort(); seg = [cols[0]]
        segs = []
        for c in cols[1:]:
            if c - seg[-1] <= 2: seg.append(c)
            else: segs.append(seg); seg = [c]
        segs.append(seg)
        for s in segs:
            if s[-1] - s[0] + 1 < 8: continue
            cy = y - H / 2.0
            claimed = any(abs(a[1] - cy) <= 3 and s[0] - W / 2 <= a[0] <= s[-1] + W / 2
                          for a in out if a[2] == 'apex')
            if claimed: continue
            if s[-1] - s[0] + 1 <= W + 4:
                out.append(((s[0] + s[-1]) / 2.0, cy, 'base'))
            else:
                x = s[0] + W / 2.0
                while x <= s[-1] - W / 2.0 + 1:
                    out.append((x, cy, 'base-merged')); x += W / 2.0
    return sorted(out)

def envelope(mask, fill, raw, w, W, H, minfill):
    """Upper and lower envelope of the marker-centre cloud, per bin of W/2 px.

    Used where markers overlap so densely that single markers cannot be
    separated (Fig 2.1b, 15-23 h). In each bin: the highest top edge with
    paper above it gives an upper marker centre (edge + H/2), the lowest
    bottom edge with paper below gives a lower one (edge - H/2). For a
    single unobstructed marker the two coincide, which is a self-check; a
    gap between them means markers at different heights share the bin.
    A bin needs >= minfill fill pixels: a partly hidden marker has ~100+,
    curve-blend specks a handful.
    """
    if not fill: return []
    xs = [p[0] for p in mask]
    lo_x, hi_x = min(xs), max(xs)
    step = max(4, W // 2)
    cols = {}
    for x, y in mask: cols.setdefault(x, []).append(y)
    out = []
    b0 = lo_x
    while b0 <= hi_x:
        b1 = b0 + step
        nfill = sum(1 for (x, y) in fill if b0 <= x < b1)
        if nfill >= minfill:
            tops, bots = [], []
            for x in range(b0, b1):
                ys = sorted(cols.get(x, []))
                for y in ys:
                    if (x, y - 1) not in mask and paper(raw, w, x, y - GAP) and \
                       any((x + u, y + k) in fill for u in range(-4, 5) for k in range(0, 13)):
                        tops.append((y, x))
                    if (x, y + 1) not in mask and paper(raw, w, x, y + GAP) and \
                       any((x + u, y - k) in fill for u in range(-4, 5) for k in range(0, 13)):
                        bots.append((y, x))
            if tops:
                t, x = min(tops)
                out.append((float(x), t + H / 2.0, 'upper'))
            if bots:
                b, x = max(bots)
                out.append((float(x), b - H / 2.0, 'lower'))
        b0 = b1
    return out

if __name__ == '__main__':
    path, kind = sys.argv[1], sys.argv[2]
    # R,G,B may be several colours joined by '+': the marker's fill AND its
    # outline (BSAF's triangles are teal with a light-green rim); each is
    # classified against the others and the curve palette, and the union is
    # the marker.
    refs = [tuple(map(int, c.split(','))) for c in sys.argv[3].split('+')]
    W, H = int(sys.argv[4]), int(sys.argv[5])
    box = tuple(map(int, sys.argv[6:10]))
    leg = None if sys.argv[10] == '-' else tuple(map(int, sys.argv[10].split(',')))
    others = [tuple(map(int, a.split(','))) for a in sys.argv[11:]]
    w, h, raw = load(path)
    mask = set(); fill = None
    for k, ref in enumerate(refs):
        got = classify(raw, w, h, ref, others + refs[:k] + refs[k+1:], box)
        if k == 0: fill = got
        mask |= got
    if leg:
        mask = {p for p in mask if not (leg[0] <= p[0] <= leg[1] and leg[2] <= p[1] <= leg[3])}
    # drop specks: anti-aliased blends of two curves can land nearest the
    # marker fill; a real marker fragment is at least MINPX pixels
    MINPX = int(os.environ.get('MINPX', '40'))
    mask = {p for c in components(mask) if len(c) >= MINPX for p in c}
    if kind == 'square': pts = squares(mask, raw, w, W, H)
    elif kind == 'envelope': pts = envelope(mask, fill, raw, w, W, H, int(os.environ.get('MINFILL', '40')))
    else: pts = triangles(mask, raw, w, W, H, fill)
    print('px_x,px_y,basis')
    for x, y, b in pts:
        print(f'{x:.1f},{y:.1f},{b}')
