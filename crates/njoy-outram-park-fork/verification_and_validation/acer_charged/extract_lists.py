#!/usr/bin/env python3
"""Extract MF=6/MT=2 LAW=5 LIST records (charged-particle elastic, LTP/LIDP form)
from the NJOY2016 test-suite evaluations, for the ptlegc/coul differential test
(gh:#1485). Usage: extract_lists.py <NJOY2016/tests/resources> > lists.txt

Output, one block per picked record:
  label awp izai awt iza spi nwords
  c(1) ... c(nwords)            (the LIST record exactly as `ptlegc` receives it:
                                 C1 C2 L1 L2 N1 N2 then the data words)
The records picked per tape: the first, the middle and the last incident energy;
tapes whose LTP is 12 or more (tabulated: p + H-2, alpha + He-4) are skipped, as
`acecpe` skips `ptlegc` for them (`ltp.lt.12`, `acefc.f90:6557`).
`awp`, `izai`, `awt`, `iza`, `spi` are what `acecpe` passes (`acefc.f90:6553`):
AWP = AWI of MF=1/MT=451 (the projectile), IZAI = NSUB/10, AWT = the MF=6 head's
AWR, IZA = the target ZA, SPI = C1 of the LAW=5 TAB2 record.
"""
import sys, os

def fl(s):
    s = s.strip()
    if not s:
        return 0.0
    try:
        return float(s)
    except ValueError:
        for i in range(len(s) - 1, 0, -1):
            if s[i] in '+-' and s[i - 1] not in 'eE':
                return float(s[:i] + 'e' + s[i:])
        raise

def rec(line):
    f = [line[11 * k:11 * (k + 1)] for k in range(6)]
    return [fl(f[0]), fl(f[1]), int(f[2] or 0), int(f[3] or 0), int(f[4] or 0), int(f[5] or 0)]

def sec(lines, mf, mt):
    return [l for l in lines if l[70:72].strip() == str(mf) and l[72:75].strip() == str(mt)]

TAPES = [
    ("deck52_p_H1", "p-001_H_001-ENDF8.0.endf"),
    ("deck51_p_H2", "p-001_H_002-ENDF8.0.endf"),
    ("deck54_p_H3", "p-001_H_003-ENDF8.0.endf"),
    ("deck53_d_H2", "d-001_H_002-ENDF8.0.endf"),
    ("deck50_a_He4", "a-002_He_004-ENDF8.0.endf"),
]

def main(root):
    for label, fn in TAPES:
        lines = open(os.path.join(root, fn)).read().split('\n')
        m1 = sec(lines, 1, 451)
        za, awr1 = rec(m1[0])[0], rec(m1[0])[1]
        awi = rec(m1[2])[0]
        nsub = rec(m1[2])[4]
        izai = nsub // 10
        s = sec(lines, 6, 2)
        head = rec(s[0])
        awt = head[1]
        nk = head[4]
        y = rec(s[1])  # first TAB1 (the scattered particle, LAW=5)
        assert y[3] == 5, (fn, y)
        nr, np_ = y[4], y[5]
        k = 2 + (nr + 2) // 3 + (np_ + 2) // 3
        t2 = rec(s[k])
        spi = t2[0] if False else fl(s[k][0:11])
        ne = t2[5]
        lidp = t2[2]  # TAB2 L1: `acecpe` stores it in the LIST's L2 slot (`acefc.f90:6556`)
        k += 1 + (t2[4] + 2) // 3
        recs = []
        for _ in range(ne):
            h = rec(s[k])
            nw = h[4]
            vals = []
            for r in s[k + 1:k + 1 + (nw + 5) // 6]:
                for q in range(6):
                    vals.append(fl(r[11 * q:11 * (q + 1)]))
            vals = vals[:nw]
            recs.append((fl(s[k][0:11]), fl(s[k][11:22]), h[2], lidp, h[4], h[5], vals))
            k += 1 + (nw + 5) // 6
        if recs[0][2] >= 12:
            continue  # LTP >= 12 is tabulated: acecpe never calls ptlegc/coul for it
        pick = sorted(set([0, len(recs) // 2, len(recs) - 1]))
        for p in pick:
            c = recs[p]
            allw = [c[0], c[1], float(c[2]), float(c[3]), float(c[4]), float(c[5])] + c[6]
            print(label, repr(awi), izai, repr(awt), int(round(za)), repr(spi), len(allw))
            print(' '.join(repr(v) for v in allw))

main(sys.argv[1])
