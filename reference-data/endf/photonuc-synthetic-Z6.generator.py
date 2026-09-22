"""Generate a SYNTHETIC ENDF-6 photonuclear (NSUB=0) tape for NJOY oracle runs.

NOT AN EVALUATION. Every cross section and emission spectrum here is an
analytic shape chosen to be smooth, positive and cheap; none of it is
measured or evaluated data and none of it should ever be used for physics.

## Why a synthetic tape

`reference-data/endf/` holds no photonuclear sublibrary tape, and the IAEA
NDS host this repository's `src/acquire.rs` downloads from
(`www-nds.iaea.org`) is blocked by the execution environment's network
policy, so one cannot be fetched.  A synthetic tape is the same device this
directory already uses for `photoat-synthetic-Z6.endf` and
`synthetic-caseb-lfw1.endf`: it is fed to **both** NJOY2016 and this port, so
it verifies that the two codes agree, which is exactly what a code-to-code
comparison needs.  It says nothing about physics and is not used for any.

## What it exercises

MF=1/451 with `NSUB = 0` (the flag `acephn` checks, `acepn.f90:127-129`),
MF=3/MT=5 as the LANL-style single non-elastic cross section that supplies the
ACE energy grid, and MF=6/MT=5 with four emitted particles -- neutron, proton,
alpha and photon -- each a LAW=1 (continuum energy-angle) subsection with
LANG=1, NA=0, so the per-particle IXSA blocks all get built.

Regenerate with:  python3 photonuc-synthetic-Z6.generator.py > photonuc-synthetic-Z6.endf
"""
import math, sys

MAT = 600
ZA = 6012.0
AWR = 11.8969
EMAX = 1.5e8          # photonuclear evaluations stop well below 1e10

def ff(x):
    """ENDF 11-character float."""
    if x == 0.0:
        return " 0.000000+0"
    s = '-' if x < 0 else ' '
    x = abs(x)
    e = int(math.floor(math.log10(x)))
    m = x / 10 ** e
    if round(m, 6) >= 10.0:
        m /= 10
        e += 1
    return f"{s}{m:.6f}{e:+d}" if abs(e) < 10 else f"{s}{m:.5f}{e:+d}"

def fi(n):
    return f"{n:11d}"

lines = []
seq = [0]

def emit(body, mf, mt):
    seq[0] += 1
    lines.append(f"{body:<66s}{MAT:4d}{mf:2d}{mt:3d}{seq[0]:5d}")

def cont(c1, c2, l1, l2, n1, n2, mf, mt):
    emit(ff(c1) + ff(c2) + fi(l1) + fi(l2) + fi(n1) + fi(n2), mf, mt)

def text(t, mf, mt):
    emit(t[:66], mf, mt)

BLANK = ' 0.000000+0 0.000000+0          0          0          0          0'

def send(mf):
    lines.append(f"{BLANK:66s}{MAT:4d}{mf:2d}{0:3d}{99999:5d}")
    seq[0] = 0

def fend():
    lines.append(f"{BLANK:66s}{MAT:4d}{0:2d}{0:3d}{0:5d}")
    seq[0] = 0

def mend():
    lines.append(f"{BLANK:66s}{0:4d}{0:2d}{0:3d}{0:5d}")

def tend():
    lines.append(f"{BLANK:66s}{-1:4d}{0:2d}{0:3d}{0:5d}")

def tab1(c1, c2, l1, l2, pairs, intlaw, mf, mt):
    cont(c1, c2, l1, l2, 1, len(pairs), mf, mt)
    emit(fi(len(pairs)) + fi(intlaw), mf, mt)
    flat = [v for p in pairs for v in p]
    for i in range(0, len(flat), 6):
        emit(''.join(ff(v) for v in flat[i:i + 6]), mf, mt)

def tab2(c1, c2, l1, l2, ne, intlaw, mf, mt):
    cont(c1, c2, l1, l2, 1, ne, mf, mt)
    emit(fi(ne) + fi(intlaw), mf, mt)

def listrec(c1, c2, l1, l2, n2, values, mf, mt):
    cont(c1, c2, l1, l2, len(values), n2, mf, mt)
    for i in range(0, len(values), 6):
        emit(''.join(ff(v) for v in values[i:i + 6]), mf, mt)

# ── the physics-shaped-but-not-physical data ────────────────────────────────
THRESH = 1.0e7        # 10 MeV: below this the non-elastic cross section is 0

def egrid():
    """A linear-interpolable grid: dense through the giant-dipole bump."""
    g = [1.0e5, 5.0e5, 1.0e6, 5.0e6, 8.0e6, 9.0e6, 9.9e6]
    e = THRESH
    while e < 3.0e7:
        g.append(e)
        e += 1.0e6
    e = 3.0e7
    while e < 1.0e8:
        g.append(e)
        e += 5.0e6
    g += [1.0e8, 1.2e8, EMAX]
    return sorted(set(g))

E = egrid()

def s_non(e):
    """A giant-dipole-resonance-shaped non-elastic cross section [barn]."""
    if e <= THRESH:
        return 0.0
    # Lorentzian peak at 23 MeV, 8 MeV wide, plus a flat quasi-deuteron tail.
    peak = 0.030 / (1.0 + ((e - 2.3e7) / 4.0e6) ** 2)
    tail = 0.004 * (1.0 - math.exp(-(e - THRESH) / 2.0e7))
    return peak + tail

non = [(e, s_non(e)) for e in E]

def emission(e_in, kind):
    """A normalised evaporation-like outgoing spectrum for one incident energy.

    `theta` is a nuclear-temperature-like width; the spectrum is
    `E' exp(-E'/theta)` truncated at the energy available, then normalised.
    """
    avail = max(e_in - THRESH, 1.0e5)
    theta = {'n': 1.5e6, 'p': 2.0e6, 'a': 2.5e6, 'g': 3.0e6}[kind] * (1.0 + avail / 5.0e7)
    n = 12
    pts = [avail * (i / (n - 1)) ** 1.5 for i in range(n)]
    pts[0] = 1.0e3
    vals = [(ep, (ep / theta) * math.exp(-ep / theta)) for ep in pts]
    area = sum((vals[i + 1][1] + vals[i][1]) * (vals[i + 1][0] - vals[i][0]) / 2
               for i in range(len(vals) - 1))
    return [(ep, v / area) for ep, v in vals]

# One emission table per incident energy above threshold.
EIN = [e for e in E if e > THRESH]

PARTICLES = [
    # (ZAP, AWP, yield-at-threshold, yield-at-EMAX, spectrum key)
    (1.0,    0.99862,  1.0, 2.4, 'n'),
    (1001.0, 0.99862,  0.0, 0.8, 'p'),
    (2004.0, 3.96713,  0.0, 0.3, 'a'),
    (0.0,    0.0,      0.6, 1.5, 'g'),
]

def yield_of(e, y0, y1):
    f = (e - THRESH) / (EMAX - THRESH)
    return y0 + (y1 - y0) * max(0.0, min(1.0, f))

# ── the tape ────────────────────────────────────────────────────────────────
lines.append(f"{'synthetic Z=6 photonuclear test tape (not an evaluation)':<66s}"
             f"{1:4d}{0:2d}{0:3d}{0:5d}")

# MF=1/451.  The third CONT's N1 is NSUB, and NSUB=0 is what makes this a
# photonuclear tape (acepn.f90:127-129 refuses anything else).
cont(ZA, AWR, 0, 0, 0, 0, 1, 451)          # HEAD: ZA, AWR, LRP, LFI, NLIB, NMOD
cont(0.0, 0.0, 0, 0, 0, 6, 1, 451)         # ELIS, STA, LIS, LISO, 0, NFOR=6
cont(0.0, EMAX, 0, 0, 0, 8, 1, 451)        # AWI=0 (photon), EMAX, LREL, 0, NSUB=0, NVER
cont(0.0, 0.0, 0, 0, 3, 3, 1, 451)         # TEMP, 0, LDRV, 0, NWD, NXC
text(" 6-C- 12 SYNTHETIC   PHOTONUCLEAR TEST DATA        OUTRAM-PARK", 1, 451)
text(" NOT AN EVALUATION: analytic shapes for NJOY2016 oracle runs.", 1, 451)
text(" NSUB=0 photonuclear; MF=3/MT=5 and MF=6/MT=5 only.", 1, 451)
for mf, mt, nc in ((1, 451, 7), (3, 5, 0), (6, 5, 0)):
    emit(' ' * 22 + fi(mf) + fi(mt) + fi(nc) + fi(0), 1, 451)
send(1)
fend()

# MF=3/MT=5 -- the non-elastic cross section, and the ACE energy grid.
cont(ZA, AWR, 0, 0, 0, 0, 3, 5)
tab1(0.0, 0.0, 0, 0, non, 2, 3, 5)
send(3)
fend()

# MF=6/MT=5 -- four emitted particles, each LAW=1 / LANG=1 / NA=0.
cont(ZA, AWR, 0, 1, len(PARTICLES), 0, 6, 5)   # LCT=1 (laboratory)
for zap, awp, y0, y1, kind in PARTICLES:
    ys = [(e, yield_of(e, y0, y1)) for e in E]
    tab1(zap, awp, 0, 1, ys, 2, 6, 5)          # LAW=1
    tab2(0.0, 0.0, 1, 2, len(EIN), 2, 6, 5)    # LANG=1, LEP=2
    for e_in in EIN:
        spec = emission(e_in, kind)
        vals = [v for pair in spec for v in pair]
        # ND=0 discrete lines, NA=0 angular orders, NW values, NEP points
        listrec(0.0, e_in, 0, 0, len(spec), vals, 6, 5)
send(6)
fend()
mend()
tend()

sys.stdout.write('\n'.join(lines) + '\n')
