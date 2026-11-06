"""Independent checks: the digitised curves against numbers the report states
in TEXT and TABLES (pp.17-21), which were transcribed separately and are not
inputs to the trace. Prints every comparison; nothing here adjusts data.

usage: checks_unit1.py CLEAN
"""
import csv, os, sys
C = sys.argv[1]
CODES = ['CIEMAT', 'IAE', 'IBRAE', 'IRSN', 'JAEA', 'KAERI', 'NRA', 'SNL', 'VTT']

def load(fig, s):
    p = os.path.join(C, f'fig{fig}-{s.lower()}.csv'); pts = []
    for r in csv.reader(open(p)):
        try: pts.append((float(r[0]), float(r[1])) + tuple(r[2:]))
        except (ValueError, IndexError): pass
    return sorted(pts)

def first(pts, cond, after=-1):
    for p in pts:
        if p[0] > after and cond(p[1]): return p[0]
    return None

def at(pts, t, tol=0.15):
    c = [p for p in pts if abs(p[0] - t) <= tol]
    return sum(p[1] for p in c) / len(c) if c else None

f = lambda v, d=2: '-' if v is None else f'{v:.{d}f}'
print('== Fig 2.1a measured RPV pressure (text p.17: two operator readings between 5 and 12 h)')
for p in load('2.1a', 'measurement'): print(f'   t = {p[0]:.3f} h   P = {p[1]:.3f} MPa  ({p[2]})')
print('== Table 2.1: MSL failure KAERI 5.0 h, SNL 6.1 h (immediate depressurisation)')
for s, tab in (('KAERI', 5.0), ('SNL', 6.1)):
    pts = load('2.1a', s)
    last_hi = max((p[0] for p in pts if p[1] > 7.0 and p[0] < 8), default=None)
    cross = first(pts, lambda v: v < 3.0, after=4.0)
    print(f'   {s:6s} last >7 MPa at {f(last_hi)} h, first <3 MPa at {f(cross)} h; table {tab} h')
print('== Table 2.2 / text p.18: CIEMAT 11.58 h, VTT 11.42 h lower-head failure "coincident with the largest measured PCV pressure at around 11.5 h"')
for s, tab in (('CIEMAT', 11.58), ('VTT', 11.42)):
    pts = load('2.1a', s)
    print(f'   {s:6s} RPV first <3 MPa after 10 h at {f(first(pts, lambda v: v < 3.0, after=10.0))} h; table {tab} h')
m = [p for p in load('2.1b', 'measurement') if p[2] != 'upper-suspect']
top = max(m, key=lambda p: p[1])
print(f'   largest measured D/W marker centre: {top[1]:.3f} MPa at {top[0]:.2f} h ({top[2]})')
print('== Text p.19: "data predicted by most participants between 15 hours and the PCV venting (~24 h) is consistent with the measurements"')
mm = [p[1] for p in m if 16 <= p[0] <= 23]
lo, hi = min(mm), max(mm); mean = sum(mm) / len(mm)
print(f'   measured marker centres 16-23 h: n={len(mm)}, {lo:.3f}-{hi:.3f} MPa, mean {mean:.3f}')
inside = 0
for s in CODES:
    v = [p[1] for p in load('2.1b', s) if 16 <= p[0] <= 23]
    if not v: print(f'   {s:6s} no visible trace 16-23 h'); continue
    cm = sum(v) / len(v); ok = lo - 0.02 <= cm <= hi + 0.02; inside += ok
    print(f'   {s:6s} mean {cm:.3f} MPa (n={len(v)})  diff {cm - mean:+.3f}  {"within" if ok else "OUTSIDE"} measured range +-0.02')
print(f'   {inside} of the codes with a visible 16-23 h trace sit within the measured range +-0.02 MPa')
print('== Bands file 1.5 (eye-read): D/W calculated peaks 0.8-1.0 MPa around 10-16 h; plateau 0.75-0.8 MPa by 20 h')
for s in CODES:
    pts = [p for p in load('2.1b', s) if p[0] <= 23]
    pk = max(pts, key=lambda p: p[1])
    print(f'   {s:6s} peak {pk[1]:.3f} MPa at {pk[0]:.2f} h;  at 20 h {f(at(pts, 20.0), 3)} MPa')
print('== Bands file 1.4 (eye-read): H2 at 25 h JAEA ~400, bulk 600-1000, IBRAE ~1310 kg; onset 3-5 h; complete by 10-15 h')
for s in CODES:
    pts = load('2.2', s); fin = sum(p[1] for p in pts if p[0] >= 24) / max(1, sum(1 for p in pts if p[0] >= 24))
    on = first(pts, lambda v: v > 20.0); c95 = first(pts, lambda v: v > 0.95 * fin)
    print(f'   {s:6s} plateau(24-25 h) {fin:7.1f} kg   >20 kg at {f(on)} h   95 % of plateau at {f(c95)} h')
print('== Text p.21: release from fuel reaches an asymptote "which in most cases exceeded 80%"; bands 1.8 onset 3-6 h')
for fig, el in (('2.5a', 'Xe'), ('2.5b', 'I')):
    n80 = 0
    for s in CODES:
        pts = load(fig, s); last = pts[-1]; n80 += last[1] > 0.8
        on = first(pts, lambda v: v > 0.05)
        print(f'   {el:2s} {s:6s} last traced {last[1]:.3f} at {last[0]:.2f} h   >0.05 at {f(on)} h')
    print(f'   {el}: {n80} of 9 last-traced values exceed 0.80')
print('== Text p.19 (Fig 2.3b): "they agree on an injection rate lower than 4.5 kg/s"; bands 1.6: "prior to [270 h] the injection is taken to be 0"')
for s in ['CIEMAT', 'IAE', 'IBRAE', 'IRSN', 'JAEA', 'NRA', 'VTT']:
    pts = load('2.3b', s)
    pre = [p for p in pts if p[0] < 268]; post = [p for p in pts if p[0] >= 275]   # 275, not 270: NRA's 18.4 -> 9.2 step sits at 270-273 h
    mpre = max(pre, key=lambda p: p[1]) if pre else None
    mpost = max(post, key=lambda p: p[1]) if post else None
    print(f'   {s:6s} max before 268 h: {f(mpre[1]) if mpre else "-"} kg/s' + (f' at {mpre[0]:.0f} h' if mpre else '') +
          f';  max from 275 h: {f(mpost[1]) if mpost else "-"} kg/s' + (f' at {mpost[0]:.0f} h' if mpost else ''))
