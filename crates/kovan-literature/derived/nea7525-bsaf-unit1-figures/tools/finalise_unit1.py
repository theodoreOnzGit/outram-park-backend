"""Round to the precision a pixel supports and assemble the Unit 1 dataset.

One pixel is 0.0125-0.020 h in time on Figs 2.1, 2.2 and 2.5 (0.029-0.42 h
on Figs 2.3 and 2.6), so time is written to 3 decimals. In value, one pixel is 0.0098 MPa (Fig 2.1a), 0.0012 MPa (2.1b),
1.42 kg (2.2), 0.0011 (2.5) and 0.023 kg/s (2.3b): written to 3, 4, 1, 4
and 2 decimals (2.3b time: 0.4 h per pixel, still written to 3 decimals).
Measurement series keep their third 'basis' column (how each marker centre
was located; README, Method).

usage: finalise_unit1.py TSV CLEAN OUTDIR
"""
import csv, os, sys

DEC = {'2.1a': 3, '2.1b': 4, '2.2': 1, '2.5a': 4, '2.5b': 4, '2.3b': 2,
       # added 2026-10-29: one pixel is 0.0012 MPa (2.3a), and 0.0007, 0.0006,
       # 0.0009, 0.0008 in mass fraction (2.6a-d); time is 0.39-0.42 h per
       # pixel on 2.3a/2.6a/2.6b and 0.029 h on 2.6c/2.6d
       '2.3a': 4, '2.6a': 4, '2.6b': 4, '2.6c': 4, '2.6d': 4}
MEASURED = ('2.1a', '2.1b', '2.3a')   # figures that plot measured data
tsv, src, dst = sys.argv[1:4]
rows = list(csv.DictReader(open(tsv), delimiter='\t'))
series = []
for r in rows:
    series.append((r['fig'], r['panel'], r['series'], r['quantity']))
    if r['series'] == 'CIEMAT' and r['fig'] in MEASURED:
        series.append((r['fig'], r['panel'], 'measurement', r['quantity']))
os.makedirs(dst, exist_ok=True)
allrows = [('figure', 'series', 'time_h', 'quantity', 'value', 'basis')]
total = 0
for fig, panel, s, q in series:
    slug = f'fig{fig}-{s.lower()}'
    pts = []
    for r in csv.reader(open(os.path.join(src, slug + '.csv'))):
        try: pts.append((float(r[0]), float(r[1]), r[2] if len(r) > 2 else ''))
        except (ValueError, IndexError): pass
    pts.sort()
    d = DEC[fig]
    with open(os.path.join(dst, slug + '.csv'), 'w', newline='') as f:
        w = csv.writer(f)
        w.writerow(['time_h', q] + (['basis'] if s == 'measurement' else []))
        for t, v, b in pts:
            w.writerow([f'{t:.3f}', f'{v:.{d}f}'] + ([b] if s == 'measurement' else []))
            allrows.append((fig, s, f'{t:.3f}', q, f'{v:.{d}f}', b))
    total += len(pts)
    print(f'  {slug}: {len(pts)}')
with open(os.path.join(dst, 'all-series.csv'), 'w', newline='') as f:
    csv.writer(f).writerows(allrows)
print(f'TOTAL {total} points in {len(series)} series')
