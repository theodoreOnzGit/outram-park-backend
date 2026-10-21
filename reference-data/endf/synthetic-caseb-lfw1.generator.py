#!/usr/bin/env python3
"""Generate a synthetic ENDF-6 tape whose only resonance range is an
unresolved **Case B** range (`LRU=2, LRF=1, LFW=1` — energy-dependent fission
widths on one shared grid).

Why this exists: Case B is the last unexercised path in this crate's
MF=2/MT=152 writer (`bn:op-12lu`). No evaluation in `reference-data/endf/`
uses it — screening every held tape finds exactly one `LRU=2/LRF=1` range
(Fe-58), and it is `LFW=0`, i.e. Case A. Rather than leave the path untested,
this builds a minimal tape NJOY2016 can process, so RECONR's own MF=2/MT=152
becomes the oracle. Same technique as `photoat-synthetic-Z6.generator.py`.

The parameters are invented and physically unremarkable; nothing here is a
measurement or is derived from any evaluation. What matters is that the tape
is format-legal and exercises the branches:

  * the fission-width grid seeds `eunr` (`rdf2u1`, reconr.f90:1370-1400);
  * one gap (1.1e4 -> 3.0e4 eV) exceeds `wide = 1.26`, so the `egridu` fill
    fires, and one (1.0e4 -> 1.1e4) does not, so it must NOT fire;
  * the last grid energy sits exactly at `EH`, so upstream's unguarded
    `enex = res(nloc+n+1)` read is never taken past the end of the list;
  * `L <= 1`, keeping clear of the `unfac` `l >= 3` defect (see `bn:op-33g1`);
  * `LSSF = 0`, so the MF=3 background addition is exercised too.

Usage:  python3 gen.py > synthetic-caseb-lfw1.endf
"""
import math, sys

MAT, ZA, AWR = 9998, 40090.0, 89.1320
EL, EH = 1.0e4, 1.0e5
ES = [1.0e4, 1.1e4, 3.0e4, 1.0e5]          # shared fission-width grid
SPI, AP, LSSF, NLS = 0.0, 0.68, 0, 2
# (L, [(D, AJ, AMUN, MUF, GN0, GG, [GF...])])
LSTATES = [
    (0, [(4.5e3, 0.5, 1.0, 1, 3.10, 0.240, [0.011, 0.013, 0.021, 0.037])]),
    (1, [(2.2e3, 0.5, 1.0, 1, 1.05, 0.220, [0.009, 0.010, 0.017, 0.030]),
         (1.5e3, 1.5, 1.0, 1, 0.72, 0.220, [0.007, 0.008, 0.014, 0.025])]),
]
# MF=3 backgrounds: (MT, [(E, xs)])
MF3 = [
    (1,   [(1.0e-5, 12.0), (1.0e4, 11.0), (1.0e5, 9.5), (2.0e7, 4.0)]),
    (2,   [(1.0e-5, 10.0), (1.0e4,  9.6), (1.0e5, 8.8), (2.0e7, 3.0)]),
    (18,  [(1.0e-5,  0.30), (1.0e4, 0.12), (1.0e5, 0.09), (2.0e7, 1.10)]),
    (102, [(1.0e-5,  1.70), (1.0e4, 1.28), (1.0e5, 0.61), (2.0e7, 0.02)]),
]

def ef(x):
    """One ENDF 11-character floating field."""
    if x == 0.0:
        return " 0.000000+0"
    sign = "-" if x < 0 else " "
    x = abs(x)
    e = int(math.floor(math.log10(x)))
    m = x / 10.0 ** e
    if float(f"{m:.6f}") >= 10.0:
        m /= 10.0
        e += 1
    es = f"{e:+d}"
    dec = 8 - len(es)                      # sign + leading digit + "." = 3 chars
    out = sign + f"{m:.{dec}f}" + es
    assert len(out) == 11, (out, x)
    return out

def ei(i):
    return f"{int(i):>11}"

class Tape:
    def __init__(self):
        self.lines, self.counts = [], {}
    def rec(self, fields, mf, mt):
        key = (mf, mt)
        self.counts[key] = self.counts.get(key, 0) + 1
        ns = self.counts[key] if mt != 0 else 99999
        self.lines.append("".join(fields) + f"{MAT:>4}{mf:>2}{mt:>3}{ns:>5}")
    def cont(self, c1, c2, l1, l2, n1, n2, mf, mt):
        self.rec([ef(c1), ef(c2), ei(l1), ei(l2), ei(n1), ei(n2)], mf, mt)
    def body(self, vals, mf, mt):
        for i in range(0, len(vals), 6):
            chunk = vals[i:i + 6]
            self.rec([ef(v) for v in chunk] + ["           "] * (6 - len(chunk)), mf, mt)
    def send(self, mf):
        self.rec([ef(0), ef(0), ei(0), ei(0), ei(0), ei(0)], mf, 0)
    def fend(self):
        self.rec([ef(0), ef(0), ei(0), ei(0), ei(0), ei(0)], 0, 0)

def build_file2():
    t = Tape()
    t.cont(ZA, AWR, 0, 0, 1, 0, 2, 151)                 # HEAD: NIS=1
    t.cont(ZA, 1.0, 0, 1, 1, 0, 2, 151)                 # isotope: LFW=1, NER=1
    t.cont(EL, EH, 2, 1, 0, 0, 2, 151)                  # range: LRU=2 LRF=1
    # LIST: SPI, AP, LSSF, 0, NE, NLS / ES(1..NE)
    t.cont(SPI, AP, LSSF, 0, len(ES), NLS, 2, 151)
    t.body(ES, 2, 151)
    for ll, js in LSTATES:
        t.cont(AWR, 0.0, ll, 0, len(js), 0, 2, 151)     # CONT: NJS in N1
        for (d, aj, amun, muf, gn0, gg, gf) in js:
            assert len(gf) == len(ES)
            # LIST: 0, 0, L, MUF, NE+6, 0 / D, AJ, AMUN, GN0, GG, 0, GF...
            t.cont(0.0, 0.0, ll, muf, len(ES) + 6, 0, 2, 151)
            t.body([d, aj, amun, gn0, gg, 0.0] + gf, 2, 151)
    t.send(2)
    return t

def build_file3():
    t = Tape()
    for mt, pairs in MF3:
        t.cont(ZA, AWR, 0, 0, 0, 0, 3, mt)              # HEAD
        t.cont(0.0, 0.0, 0, 0, 1, len(pairs), 3, mt)    # TAB1: 1 region
        t.rec([ei(len(pairs)), ei(2)] + ["           "] * 4, 3, mt)  # NBT, INT=lin-lin
        flat = [v for p in pairs for v in p]
        t.body(flat, 3, mt)
        t.send(3)
    return t

def main():
    f2, f3 = build_file2(), build_file3()
    # Dictionary: one entry per section, NC = record count excluding SEND.
    nc2 = f2.counts[(2, 151)]
    entries = [(2, 151, nc2, 0)]
    for mt, _ in MF3:
        entries.append((3, mt, f3.counts[(3, mt)], 0))
    text = [
        f" {'synthetic Case B (LRU=2, LRF=1, LFW=1) test material':<65}",
        f" {'generated by reference-data/endf/synthetic-caseb-lfw1.generator.py':<65}",
        f" {'invented parameters; not an evaluation, not measurement-derived':<65}",
    ]
    t = Tape()
    t.cont(ZA, AWR, 1, 0, 0, 6, 1, 451)                 # LRP=1, NFOR=6
    t.cont(0.0, 1.0, 0, 0, 0, 6, 1, 451)
    t.cont(1.0, 2.0e7, 0, 0, 10, 8, 1, 451)             # NSUB=10 (n), NVER=8
    t.cont(0.0, 0.0, 0, 0, len(text), len(entries), 1, 451)
    for line in text:
        t.rec([line[:66]], 1, 451)
    for (mf, mt, nc, mod) in entries:
        t.rec(["           ", "           ", ei(mf), ei(mt), ei(nc), ei(mod)], 1, 451)
    t.send(1)

    out = [f" {'synthetic Case B tape':<65}{1:>4}{0:>2}{0:>3}{0:>5}"]
    out += t.lines
    out.append(f"{ef(0)}{ef(0)}{ei(0)}{ei(0)}{ei(0)}{ei(0)}{MAT:>4}{0:>2}{0:>3}{0:>5}")
    out += f2.lines
    out.append(f"{ef(0)}{ef(0)}{ei(0)}{ei(0)}{ei(0)}{ei(0)}{MAT:>4}{0:>2}{0:>3}{0:>5}")
    out += f3.lines
    out.append(f"{ef(0)}{ef(0)}{ei(0)}{ei(0)}{ei(0)}{ei(0)}{MAT:>4}{0:>2}{0:>3}{0:>5}")
    out.append(f"{ef(0)}{ef(0)}{ei(0)}{ei(0)}{ei(0)}{ei(0)}{0:>4}{0:>2}{0:>3}{0:>5}")
    out.append(f"{ef(0)}{ef(0)}{ei(0)}{ei(0)}{ei(0)}{ei(0)}{-1:>4}{0:>2}{0:>3}{0:>5}")
    sys.stdout.write("\n".join(out) + "\n")

main()
