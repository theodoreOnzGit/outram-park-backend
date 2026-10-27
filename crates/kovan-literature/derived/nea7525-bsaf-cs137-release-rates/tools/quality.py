"""Per-curve quality metrics: integral recovery and suspected crossing artefacts."""
import csv, math, os
from integrate import TARGET, integral_pbq

def jumps(path):
    pts = []
    for r in csv.reader(open(path)):
        try: pts.append((float(r[0]), float(r[1])))
        except (ValueError, IndexError): pass
    pts.sort()
    n = 0
    for i in range(1, len(pts) - 1):
        a, b, c = pts[i-1][1], pts[i][1], pts[i+1][1]
        if min(a, b, c) <= 0: continue
        d1 = abs(math.log10(b) - math.log10(a))
        d2 = abs(math.log10(c) - math.log10(b))
        if d1 > 0.6 and d2 > 0.6 and (math.log10(b)-math.log10(a))*(math.log10(c)-math.log10(b)) < 0:
            n += 1
    return len(pts), n

print(f"{'FIG':5s} {'SERIES':27s} {'N':>5s} {'INT':>7s} {'TBL':>6s} {'RATIO':>6s} {'SPIKES':>7s} GRADE")
for fig, series in sorted(TARGET):
    slug = ''.join(c for c in ('fig'+fig+'-'+series).lower().replace(' ','-') if c.isalnum() or c in '.-')
    p = os.path.join('clean', slug + '.csv')
    if not os.path.exists(p): continue
    n, j = jumps(p)
    got, want = integral_pbq(p), TARGET[(fig, series)]
    ratio = got/want if want > 0 else None
    if want == 0:
        grade = 'A (table says 0.0; trace empty)' if got < 0.05 else 'CHECK'
    elif 0.85 <= ratio <= 1.2 and j <= 2: grade = 'A'
    elif 0.7 <= ratio <= 1.5 and j <= 8:  grade = 'B'
    elif 0.3 <= ratio <= 3.0:             grade = 'C  under/over-reads'
    else:                                 grade = 'D  do not use for magnitude'
    rs = f'{ratio:6.2f}' if ratio is not None else '     -'
    print(f'{fig:5s} {series:27s} {n:5d} {got:7.2f} {want:6.2f} {rs} {j:7d}  {grade}')
