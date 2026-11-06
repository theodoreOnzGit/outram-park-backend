"""Independent checks for Figs 2.3a and 2.6 (added 2026-10-29), in the style
of checks_unit1.py: the digitised curves against what the report states in
TEXT (pp.19-22), against the eye-read bands file, and against OTHER digitised
figures of the same report (Figs 2.1b, 2.3b, 2.5b), which were traced from
different rasters and so can disagree. Prints every comparison; nothing here
adjusts data.

usage: checks_unit1_late.py DIR    (a folder holding every fig*.csv)
"""
import csv, os, sys
D = sys.argv[1]
CODES = ['CIEMAT', 'IAE', 'IBRAE', 'IRSN', 'JAEA', 'KAERI', 'NRA', 'SNL', 'VTT']

def load(fig, s):
    pts = []
    for r in csv.reader(open(os.path.join(D, f'fig{fig}-{s.lower()}.csv'))):
        try: pts.append((float(r[0]), float(r[1])) + tuple(r[2:]))
        except (ValueError, IndexError): pass
    return sorted(pts)

def at(pts, t, tol):
    c = [p[1] for p in pts if abs(p[0] - t) <= tol]
    return sum(c) / len(c) if c else None

f = lambda v, d=3: '-' if v is None else f'{v:.{d}f}'

m = [p for p in load('2.3a', 'measurement') if 'suspect' not in p[2]]
up = [p for p in m if p[2] == 'upper']; lo = [p for p in m if p[2] == 'lower']
print(f'== Fig 2.3a measured envelope: {len(m)} usable points ({len(up)} upper, {len(lo)} lower), '
      f'{sum("suspect" in p[2] for p in load("2.3a", "measurement"))} suspect')
print('== Text p.19: "a pressure rise in the PCV until 50 hours, when the trend reversed and decreased for more than'
      ' 200 hours until it sharply started increasing again at 270 hours"')
seg = [p for p in m if 30 <= p[0] <= 100]
pk = max(seg, key=lambda p: p[1])
print(f'   largest centre 30-100 h: {pk[1]:.3f} MPa at {pk[0]:.1f} h ({pk[2]})')
seg = [p for p in m if 100 <= p[0] <= 272]
mn = min(seg, key=lambda p: p[1])
print(f'   smallest centre 100-272 h: {mn[1]:.3f} MPa at {mn[0]:.1f} h ({mn[2]}); '
      f'no marker at all between {max(p[0] for p in m if p[0] < 120):.1f} and {min(p[0] for p in m if p[0] > 120):.1f} h')
base = [p[1] for p in up if 240 <= p[0] <= 270]
b = max(base)
rise = next(p for p in up if p[0] > 262 and p[1] > b + 0.03)
print(f'   upper envelope 240-270 h at most {b:.3f} MPa; first upper point > {b + 0.03:.3f} MPa at {rise[0]:.1f} h ({rise[1]:.3f} MPa)')
pk3 = max((p for p in m if 270 <= p[0] <= 340), key=lambda p: p[1])
print(f'   peak after 270 h: {pk3[1]:.3f} MPa at {pk3[0]:.1f} h ({pk3[2]})')
print('== Bands 1.5 (eye-read): "a fall from ~0.8 MPa to ~0.15-0.2 MPa by 250 h, a rise to ~0.4 MPa near 300 h, then a slow decline"')
top = max(m, key=lambda p: p[1])
print(f'   largest centre overall {top[1]:.3f} MPa at {top[0]:.1f} h (marker apex {top[1] + 17.5 / 819.1:.3f} MPa)')
w = [p[1] for p in m if 230 <= p[0] <= 270]
print(f'   centres 230-270 h: {min(w):.3f}-{max(w):.3f} MPa')
e = [p for p in m if p[0] >= 480]
print(f'   centres from 480 h: {min(p[1] for p in e):.3f}-{max(p[1] for p in e):.3f} MPa')
print('== Cross-figure: Fig 2.1b (0-23 h raster) and Fig 2.3a (23-500 h raster) plot the same measured series')
b1 = [p for p in load('2.1b', 'measurement') if 'suspect' not in p[2] and p[0] >= 21.5]
a1 = [p for p in m if p[0] <= 30 and p[2] == 'upper']
print(f'   Fig 2.1b upper centres 21.5-23.3 h: {", ".join(f"{p[1]:.3f}" for p in b1 if p[2] == "upper")} MPa')
print(f'   Fig 2.3a upper centres <= 30 h:     {", ".join(f"{p[1]:.3f}@{p[0]:.1f}h" for p in a1)} MPa')
print('== Cross-figure: Fig 2.3b injection from ~270 h vs Fig 2.3a code pressure rise after 270 h')
for s in CODES:
    pts = load('2.3a', s)
    v0 = at(pts, 265, 3); late = [p[1] for p in pts if 272 <= p[0] <= 330]
    inj = None
    if os.path.exists(os.path.join(D, f'fig2.3b-{s.lower()}.csv')):
        q = [p for p in load('2.3b', s) if 250 <= p[0] <= 330]
        inj = max(q, key=lambda p: p[1]) if q else None
    print(f'   {s:6s} P(265 h) {f(v0)}  max 272-330 h {f(max(late) if late else None)}  rise {f(max(late) - v0 if late and v0 else None)} MPa;'
          f'  Fig 2.3b max 250-330 h: ' + (f'{inj[1]:.2f} kg/s at {inj[0]:.0f} h' if inj else 'no trace'))
print('== Text p.21: "At 35 hours, 10-80%, roughly, of Cs and I was estimated to have been scrubbed into the S/C water"')
for fig, el in (('2.6c', 'Cs'), ('2.6d', 'I')):
    vals = []
    for s in CODES:
        pts = load(fig, s); v = at(pts, 34.8, 0.25); vals.append((s, v))
    print(f'   {el} at 34.55-35 h: ' + ', '.join(f'{s} {f(v)}' for s, v in vals))
    vv = [v for _, v in vals if v is not None]
    print(f'   {el}: range {min(vv):.3f}-{max(vv):.3f}; {sum(0.10 <= v <= 0.80 for v in vv)} of {len(vv)} within 0.10-0.80')
print('== Bands 1.8 (eye-read): "Cs deposited on D/W structures ~0.02-0.7 (JAEA highest)" (Fig 2.6a)')
for fig, el in (('2.6a', 'Cs'), ('2.6b', 'I')):
    for s in CODES:
        pts = load(fig, s); pk = max(pts, key=lambda p: p[1])
        print(f'   {el} {s:6s} max {pk[1]:.3f} at {pk[0]:6.1f} h; at 495-500 h {f(at(pts, 497.5, 2.5))}; last traced {pts[-1][1]:.3f} at {pts[-1][0]:.1f} h')
print('== Text p.21: "some remobilisation was noted in some calculations": codes whose D/W deposit falls below half its maximum')
for fig, el in (('2.6a', 'Cs'), ('2.6b', 'I')):
    out = []
    for s in CODES:
        pts = load(fig, s); pk = max(pts, key=lambda p: p[1])
        after = [p for p in pts if p[0] > pk[0] and p[1] < 0.5 * pk[1]]
        if after: out.append(f'{s} (below half by {after[0][0]:.0f} h)')
    print(f'   {el}: ' + ', '.join(out))
print('== Mass balance across figures: deposited on D/W (2.6b) + dissolved in S/C (2.6d) <= released from fuel (2.5b), iodine, at 14-15 h')
print('   (release from fuel never decreases, so the largest traced 2.5b value up to 15 h is a LOWER bound on the release at 15 h;')
print('    2.6b is read as the mean over 13-16 h, because its pixel is 0.42 h wide and the curves still rise there)')
for s in CODES:
    a = at(load('2.6b', s), 14.5, 1.5); c = at(load('2.6d', s), 14.5, 0.5)
    rel = [p[1] for p in load('2.5b', s) if p[0] <= 15.0]
    r = max(rel) if rel else None
    tot = (a or 0) + (c or 0)
    if a is None or c is None: ok = 'incomplete'
    elif tot <= r: ok = 'consistent (below the release lower bound)'
    elif tot <= 1.0: ok = 'not decidable (above the traced lower bound, below 1)'
    else: ok = 'INCONSISTENT (> 1)'
    print(f'   {s:6s} D/W {f(a)} + S/C {f(c)} = {tot:.3f}  vs released >= {f(r)}  {ok}')
print('== Mass balance: D/W deposit + S/C water <= 1 at 35 h')
for fig1, fig2, el in (('2.6a', '2.6c', 'Cs'), ('2.6b', '2.6d', 'I')):
    for s in CODES:
        a = at(load(fig1, s), 35, 2.0); c = at(load(fig2, s), 34.8, 0.25)
        print(f'   {el} {s:6s} D/W {f(a)} + S/C {f(c)} = {f((a or 0) + (c or 0)) if a is not None and c is not None else "-"}')
print('== Bands 1.5 (eye-read): "Over 0-500 h the measured trace is dense and well reproduced": code means vs the measured envelope in two windows')
for a, b in ((170, 260), (330, 420)):
    mm = [p[1] for p in m if a <= p[0] <= b]
    lo_, hi_ = min(mm), max(mm)
    n_in, n_vis = 0, 0
    row = []
    for s in CODES:
        v = [p[1] for p in load('2.3a', s) if a <= p[0] <= b]
        if len(v) < 5: row.append(f'{s} -'); continue
        cm = sum(v) / len(v); n_vis += 1; ok = lo_ - 0.02 <= cm <= hi_ + 0.02; n_in += ok
        row.append(f'{s} {cm:.3f}{"" if ok else "*"}')
    print(f'   {a}-{b} h: measured centres {lo_:.3f}-{hi_:.3f} MPa (n={len(mm)}); code means: ' + ', '.join(row))
    print(f'   {n_in} of {n_vis} visible codes within the measured range +-0.02 MPa (* = outside)')
