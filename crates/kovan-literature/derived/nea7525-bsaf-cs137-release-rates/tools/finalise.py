"""Round to the precision a pixel actually supports and assemble the dataset.

One pixel is 8 decades / ~1000 px = 1.9% in rate and ~0.012 day in time, so
full float precision from the tracer is false precision. Rate is written to
4 significant figures, time to 4 decimals.
"""
import csv, os, sys

META = {}
for line in open(sys.argv[1]):
    f = line.rstrip('\n').split('\t')
    if f[0] == 'fig': continue
    META.setdefault(f[0], []).append((f[1], f[7]))     # fig -> [(unit, series)]

src, dstdir = sys.argv[2], sys.argv[3]
os.makedirs(dstdir, exist_ok=True)
combined = [('figure', 'unit', 'series', 'time_day', 'release_rate_bq_per_hr')]
total = 0
for fig in sorted(META):
    for unit, series in META[fig]:
        slug = ''.join(c for c in ('fig'+fig+'-'+series).lower().replace(' ', '-')
                       if c.isalnum() or c in '.-')
        p = os.path.join(src, slug + '.csv')
        pts = []
        if os.path.exists(p):
            for r in csv.reader(open(p)):
                try: x, y = float(r[0]), float(r[1])
                except (ValueError, IndexError): continue
                pts.append((round(x, 4), float(f'{y:.4g}')))
        pts.sort()
        if not pts:
            print(f'  {slug}: EMPTY (no curve in the figure) - not written')
            continue
        with open(os.path.join(dstdir, slug + '.csv'), 'w', newline='') as f:
            w = csv.writer(f)
            w.writerow(['time_day', 'release_rate_bq_per_hr'])
            for x, y in pts:
                w.writerow([f'{x:.4f}', f'{y:.4g}'])
        for x, y in pts:
            combined.append((fig, unit, series, f'{x:.4f}', f'{y:.4g}'))
        total += len(pts)
        print(f'  {slug}: {len(pts)} points')
with open(os.path.join(dstdir, 'all-series.csv'), 'w', newline='') as f:
    csv.writer(f).writerows(combined)
print(f'TOTAL {total} points across {len(combined)-1} rows')
