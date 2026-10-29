"""Round-trip check and per-series quality metrics for the Unit 1 traces.

For every series in curves-unit1.tsv (traced CSVs in CLEAN/):
  RT   round-trip: reproject each point to a pixel with the same calibration
       and test whether the ORIGINAL figure is within TOL of the series'
       legend colour in the 3x3 neighbourhood. Partly circular (the tracer
       only accepted pixels within the same tolerance): it shows the
       calibration round-trips and nothing landed on axis/blank paper, NOT
       that the right curve was followed.
  COV  coverage: share of the pixel columns from the first traced point to
       the END OF THE AXIS (every code curve in these figures is drawn to
       the axis end) that carry a traced point within one step (2 px).
       (A first run measured to the LAST traced point instead; that scored a
       curve hidden under others for its whole tail, e.g. IAE on Fig 2.5a,
       as 1.00, so the denominator was changed - the stricter choice.) Low
       coverage = the curve is hidden under later-drawn curves there, which
       no tracer can recover from the raster.
  SPK  spikes: out-and-back excursions of 1-3 points larger than 8 % of
       the axis full scale (suspected crossing/blend artefacts; NOT removed).
Grades (fixed before looking at the numbers, mirroring the Cs-137 grades'
intent, with coverage standing in for the integral check these figures
lack): A  COV >= 0.85 and SPK <= 2;  B  COV >= 0.60 and SPK <= 6;  C  else.

usage: quality_unit1.py TSV IMGDIR CLEAN
"""
import csv, math, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python3 -I drops it
from calib import load, px

TOL = 30.0
FULL = {'2.1a': 8.0, '2.1b': 1.0, '2.2': 1400.0, '2.5a': 1.0, '2.5b': 1.0, '2.3b': 20.0}

def slug(fig, series):
    return f'fig{fig}-{series}'.lower().replace(' ', '-')

def main():
    tsv, imgdir, clean = sys.argv[1:4]
    rows = list(csv.DictReader(open(tsv), delimiter='\t'))
    cache = {}
    print(f"{'FIG':5s} {'SERIES':7s} {'N':>4s} {'T0':>6s} {'T1':>6s} {'COV':>5s} {'SPK':>3s} {'RT%':>6s} {'RTmed':>5s} GRADE")
    out = []
    for r in rows:
        fig = r['fig']
        if r['image'] not in cache: cache[r['image']] = load(os.path.join(imgdir, r['image']))
        w, h, raw = cache[r['image']]
        x0p, x0v, x1p, x1v = (float(r[k]) for k in ('xpx0', 'xval0', 'xpx1', 'xval1'))
        y0p, y0v, y1p, y1v = (float(r[k]) for k in ('ypx0', 'yval0', 'ypx1', 'yval1'))
        tox = lambda v: x0p + (v - x0v) * (x1p - x0p) / (x1v - x0v)
        toy = lambda v: y0p + (v - y0v) * (y1p - y0p) / (y1v - y0v)
        want = (int(r['r']), int(r['g']), int(r['b']))
        pts = []
        p = os.path.join(clean, slug(fig, r['series']) + '.csv')
        for row in csv.reader(open(p)):
            try: pts.append((float(row[0]), float(row[1])))
            except (ValueError, IndexError): pass
        pts.sort()
        ds = []
        for x, y in pts:
            xp, yp = int(round(tox(x))), int(round(toy(y)))
            best = min(math.dist(px(raw, w, xp + dx, yp + dy), want)
                       for dx in (-1, 0, 1) for dy in (-1, 0, 1))
            ds.append(best)
        ds.sort()
        rt = 100.0 * sum(d <= TOL for d in ds) / len(ds)
        cols = sorted(set(int(round(tox(x))) for x, _ in pts))
        # Figs 2.1, 2.2, 2.5: every code curve is drawn to the axis end
        # (checked by eye on the figure). Fig 2.3b: curves visibly stop
        # early (simulations ended: VTT ~120 h, JAEA/IRSN ~305 h, NRA ~380 h),
        # so there coverage runs to the last traced point.
        end = int(r['ax_right']) if fig != '2.3b' else cols[-1]
        span = end - cols[0] + 1
        covered = sum(1 for c in range(cols[0], end + 1)
                      if any(abs(c - k) <= 2 for k in cols[max(0, bisect(cols, c) - 1):bisect(cols, c) + 1]))
        cov = covered / span
        thr = 0.08 * FULL[fig]; spk = 0; i = 1
        while i < len(pts) - 1:
            base = pts[i-1][1]; hit = 0
            for k in (1, 2, 3):
                if i + k >= len(pts): break
                if abs(pts[i+k][1] - base) < thr / 2 and \
                   max(abs(pts[j][1] - base) for j in range(i, i + k)) > thr:
                    hit = k; break
            if hit: spk += 1; i += hit
            else: i += 1
        g = 'A' if cov >= 0.85 and spk <= 2 else ('B' if cov >= 0.60 and spk <= 6 else 'C')
        print(f'{fig:5s} {r["series"]:7s} {len(pts):4d} {pts[0][0]:6.2f} {pts[-1][0]:6.2f} {cov:5.2f} {spk:3d} {rt:6.1f} {ds[len(ds)//2]:5.1f} {g}')
        out.append((fig, r['series'], len(pts), cov, spk, rt, g))
    n = len(out)
    print(f'\nround-trip: {sum(o[5] == 100.0 for o in out)}/{n} series at 100 %, worst {min(o[5] for o in out):.1f} %')
    print('grades:', {g: sum(o[6] == g for o in out) for g in 'ABC'})

def bisect(a, v):
    lo, hi = 0, len(a)
    while lo < hi:
        m = (lo + hi) // 2
        if a[m] < v: lo = m + 1
        else: hi = m
    return lo

main()
