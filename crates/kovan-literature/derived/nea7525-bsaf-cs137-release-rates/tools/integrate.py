"""Independent check: integrate each digitised release-rate curve and compare
against the integral release the report tabulates (Table 5.1, transcribed by
hand from a different page).

The curve comes from pixels; the target comes from a table. Nothing links
them, so agreement is evidence the trace found the right curve. Trapezoidal
in rate (Bq/hr) against time (hours).
"""
import csv, os, sys

TARGET = {   # Table 5.1, PBq of Cs-137
 ('5.1','SNL MELCOR'):2.8, ('5.1','IAE SAMPSON'):4.4, ('5.1','IRSN ASTEC'):0.1,
 ('5.1','KAERI MELCOR'):18.9, ('5.1','CIEMAT MELCOR'):5.6,
 ('5.2','SNL MELCOR'):12.3, ('5.2','IAE SAMPSON'):0.0, ('5.2','IRSN ASTEC'):0.1,
 ('5.2','KAERI MELCOR'):5.8, ('5.2','CNL MELCOR'):8.1, ('5.2','CRIEPI MAAP'):6.7,
 ('5.3','SNL MELCOR'):10.6, ('5.3','IAE SAMPSON'):1.1, ('5.3','IRSN ASTEC'):43.1,
 ('5.3','KAERI MELCOR'):10.9, ('5.3','CRIEPI MAAP'):0.8, ('5.3','PSI MELCOR'):0.2,
 ('5.4','SNL MELCOR'):25.7, ('5.4','GRS Backwards Calculation'):13.1,
 ('5.4','IAE SAMPSON'):5.5, ('5.4','IRSN ASTEC'):43.2, ('5.4','KAERI MELCOR'):35.5,
}

def integral_pbq(path, max_gap_days=0.5):
    pts = []
    with open(path) as f:
        for row in csv.reader(f):
            try:
                x, y = float(row[0]), float(row[1])
            except (ValueError, IndexError):
                continue
            if y > 0:
                pts.append((x, y))
    pts.sort()
    total = 0.0
    for i in range(len(pts) - 1):
        (x0, y0), (x1, y1) = pts[i], pts[i+1]
        dt = x1 - x0
        if dt <= 0 or dt > max_gap_days:   # do not bridge a real gap
            continue
        total += 0.5 * (y0 + y1) * dt * 24.0
    return total / 1e15

if __name__ == '__main__':
    print(f"{'FIG':5s} {'SERIES':27s} {'DIGITISED':>10s} {'TABLE 5.1':>10s} {'RATIO':>7s}")
    rows = []
    for fig, series in sorted(TARGET):
        slug = ('fig' + fig + '-' + series).lower().replace(' ', '-')
        slug = ''.join(c for c in slug if c.isalnum() or c in '.-')
        path = os.path.join('clean', slug + '.csv')
        if not os.path.exists(path):
            print(f'{fig:5s} {series:27s}  MISSING {path}'); continue
        got = integral_pbq(path)
        want = TARGET[(fig, series)]
        ratio = (got / want) if want > 0 else float('nan')
        flag = ''
        if want > 0:
            if 0.5 <= ratio <= 2.0: flag = 'ok'
            elif 0.2 <= ratio <= 5.0: flag = 'fair'
            else: flag = '** CHECK **'
        print(f'{fig:5s} {series:27s} {got:10.2f} {want:10.2f} {ratio:7.2f}  {flag}')
        if want > 0: rows.append(ratio)
    rows.sort()
    if rows:
        print(f'\nratios: median {rows[len(rows)//2]:.2f}, '
              f'{sum(1 for r in rows if 0.5<=r<=2.0)}/{len(rows)} within a factor of 2')
