"""Measured data on Fig 2.1 (markers) -> data-unit CSVs in WORK/clean/.

  Fig 2.1a  two orange-red squares (operator RPV readings): markers.py
            square mode, marker 27 x 27 px (measured on the unoccluded
            plot markers' bounding boxes).
  Fig 2.1b  teal triangles with a light-green rim, ~44 x 34 px including
            the rim (measured on the isolated marker at 10.3 h: rim apex
            row 434, rim base row 468, columns 694-737). They overlap and
            are crossed by curves, so markers.py envelope mode is used.
Calibration constants are the tick fits from axes.py (README, Verification).
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
