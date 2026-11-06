"""Measured data on Figs 2.1 and 2.3a (markers) -> data-unit CSVs in WORK/clean/.

  Fig 2.1a  two orange-red squares (operator RPV readings): markers.py
            square mode, marker 27 x 27 px (measured on the unoccluded
            plot markers' bounding boxes).
  Fig 2.1b  teal triangles with a light-green rim, ~44 x 34 px including
            the rim (measured on the isolated marker at 10.3 h: rim apex
            row 434, rim base row 468, columns 694-737). They overlap and
            are crossed by curves, so markers.py envelope mode is used.
  Fig 2.3a  (added 2026-10-29) the same teal triangles over 0-500 h, smaller
            on this raster: 37 x 35 px including the rim (measured on the
            legend marker: rim apex row 74, rim base row 108, rim columns
            1060-1096; fill 32 x 32). Its rim is lighter and yellower than
            on Fig 2.1b (~120,235,70 and ~197,251,167), so those measured rim
            colours are the rim references here. Envelope mode, as Fig 2.1b.
Calibration constants are the tick fits from axes.py (README, Verification).
FIGS=2.3a (comma list) restricts the run, so the earlier figures' files are
not rewritten when the dataset is extended.
"""
import csv, io, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
imgdir, work = sys.argv[1], sys.argv[2]
P1 = '246,201,48 0,0,215 0,58,0 229,0,234 3,235,0 0,218,220 107,0,216 0,0,0 107,40,0'.split()
P2 = '242,201,47 0,0,212 0,65,1 229,0,224 0,234,0 0,221,216 107,0,220 0,0,0 106,36,0'.split()

def run(args, env):
    out = subprocess.run(['python3', '-I', os.path.join(HERE, 'markers.py'), *args],
                         capture_output=True, text=True, check=True, env=dict(os.environ, **env)).stdout
    return list(csv.DictReader(io.StringIO(out)))

os.makedirs(f'{work}/clean', exist_ok=True)
ONLY = [f for f in os.environ.get('FIGS', '').split(',') if f]
want = lambda f: not ONLY or f in ONLY

def envelope_csv(pts, binh, path, q):
    # Self-check: an UPPER envelope point lower than a LOWER one within the
    # same bin is impossible for a cloud of markers; it means the 'apex' was
    # a cut made by a crossing curve, not a marker tip. Such points are kept
    # but re-labelled 'upper-suspect' (not deleted, not moved).
    for q_ in pts:
        if q_[2] == 'upper' and any(o[2] == 'lower' and abs(o[0] - q_[0]) <= binh and o[1] > q_[1] + 0.005 for o in pts):
            q_[2] = 'upper-suspect'
    with open(path, 'w', newline='') as f:
        w = csv.writer(f); w.writerow(['time_h', q, 'basis'])
        for t, p, bas in pts:
            w.writerow([f'{t:.3f}', f'{p:.4f}', bas])

if want('2.3a'):
    P3 = '238,199,68 0,0,213 0,66,0 227,0,228 0,230,0 0,221,218 102,1,215 0,0,0 105,32,0'.split()
    c = run([f'{imgdir}/u1-022-003.png', 'envelope', '33,178,171+120,235,70+197,251,167', '37', '35',
             '140', '1440', '40', '950', '1025,1340,66,552', *P3], {'MINPX': '15', 'GAP': '3', 'MINFILL': '40'})
    pts = [[50 + (float(r['px_x']) - 238.84) * 450 / (1384.86 - 238.84),
            (954.27 - float(r['px_y'])) / (954.27 - 135.17), r['basis']] for r in c]
    # Operator audit (2026-10-29): every envelope point was inspected on the
    # private overlay. One 'lower' point lies below the marker cloud, on the
    # crossing of the IAE (blue) and JAEA (green) curves, whose blend mimics
    # teal fill over a pale-green rim (checked on a 5x zoom of the raster).
    # It is kept and labelled, not moved or deleted. A geometric test for
    # this (fill within 5 rows above a base) was tried first and rejected:
    # it also flagged 13 genuine bases, so it was not a usable instrument.
    for q_ in pts:
        if q_[2] == 'lower' and abs(q_[0] - 358.70) < 0.05:
            q_[2] = 'lower-suspect'
    # bin = W/2 = 18 px = 7.1 h on this axis
    envelope_csv(pts, 7.2, f'{work}/clean/fig2.3a-measurement.csv', 'drywell_pressure_mpa_abs')
    for t, p, bas in sorted(pts): print(f'2.3a  {t:7.2f} h  {p:6.4f} MPa  {bas}')
    print(f'2.3a  {len(pts)} envelope points')
if not want('2.1a'): sys.exit(0)
a = run([f'{imgdir}/u1-019-000.png', 'square', '255,65,7', '27', '27', '162', '985', '60', '945', '-', *P1], {})
with open(f'{work}/clean/fig2.1a-measurement.csv', 'w', newline='') as f:
    w = csv.writer(f); w.writerow(['time_h', 'rpv_pressure_mpa_abs', 'basis'])
    for r in a:
        t = (float(r['px_x']) - 159.60) * 20 / (1182.60 - 159.60)
        p = (947.35 - float(r['px_y'])) * 8 / (947.35 - 126.85)
        w.writerow([f'{t:.3f}', f'{p:.3f}', r['basis']]); print(f'2.1a  {t:6.3f} h  {p:6.3f} MPa  {r["basis"]}')
b = run([f'{imgdir}/u1-019-001.png', 'envelope', '33,178,171+102,230,5+200,250,140', '44', '34',
         '169', '1420', '40', '945', '1031,1342,450,936', *P2], {'MINPX': '15', 'GAP': '3', 'MINFILL': '40'})
# Self-check: an UPPER envelope point lower than a LOWER one within the same
# bin (W/2 = 22 px = 0.41 h) is impossible for a cloud of markers; it means
# the 'apex' was a cut made by a crossing curve, not a marker tip. Such
# points are kept but re-labelled 'upper-suspect' (not deleted, not moved).
pts = []
for r in b:
    t = (float(r['px_x']) - 166.40) * 20 / (1230.40 - 166.40)
    p = (949.14 - float(r['px_y'])) * 1 / (949.14 - 125.36)
    pts.append([t, p, r['basis']])
for q in pts:
    if q[2] == 'upper' and any(o[2] == 'lower' and abs(o[0] - q[0]) <= 0.42 and o[1] > q[1] + 0.005 for o in pts):
        q[2] = 'upper-suspect'
with open(f'{work}/clean/fig2.1b-measurement.csv', 'w', newline='') as f:
    w = csv.writer(f); w.writerow(['time_h', 'drywell_pressure_mpa_abs', 'basis'])
    for t, p, bas in pts:
        w.writerow([f'{t:.3f}', f'{p:.4f}', bas])
        print(f'2.1b  {t:6.3f} h  {p:6.4f} MPa  {bas}')
print(f'2.1b  {len(b)} envelope points')
