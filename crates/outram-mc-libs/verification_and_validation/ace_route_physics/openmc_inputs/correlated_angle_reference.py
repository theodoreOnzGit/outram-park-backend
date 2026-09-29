#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0
#
# Exact moments of OpenMC's sampling of the ACE CORRELATED angle laws (law 61,
# law 44) and of the AND histogram forms (intt = 1, 32 equiprobable bins), for
# GitHub #365 audit.
#
# Own work (outram-park-backend), GPL-3.0; OpenMC Python API (MIT) as a library.
#
# What outram did before the audit, and what OpenMC does (src/secondary_correlated.cpp,
# src/secondary_kalbach.cpp, src/distribution.cpp):
#   law 61: row = lower edge k of the E' bin      | row k or k+1, whichever cdf edge
#                                                  |   is nearer r1 (lin-lin tables);
#                                                  |   k for histogram tables
#   cosine: linear-cdf inverse of the row          | Tabular::sample: quadratic for
#                                                  |   lin-lin, linear for histogram
#   law 44: r, a of row k                          | r, a interpolated to E' (raw, before
#                                                  |   the envelope scaling), lin-lin only
#   AND intt=1 / equiprobable: refused / lin-lin   | histogram Tabular
#
# The moments are exact for OpenMC's scheme: incident table i or i+1 with
# probability 1-r / r; for each E' bin [c_k, c_k1] the draw r1 is uniform, so
# the bin is split at the cdf midpoint (law 61) and each piece integrated with
# 48-point Gauss-Legendre in r1 (E'(r1) is the quadratic inverse, then the
# envelope scaling). The row's own mean cosine is closed form (Tabular mean;
# Kalbach r (coth a - 1/a)). Also printed: the same moments under outram's
# OLD scheme, to show each case discriminates.
#
# Constructed tables (no NJOY-built table in reach has these forms: every
# held AND and law-61 cosine row is intt = 2, and every law-44 table in reach
# is a histogram in E'):
#   U235_hist.ace  : NJOY2016 U-235 293.6 K with (a) every MT=91 law-61 cosine
#                    row turned into a histogram (pdf renormalised, cdf rebuilt
#                    for the histogram), (b) every MT=2 AND row with >= 11
#                    points replaced by 32 equiprobable bins cut from its own
#                    lin-lin cdf, (c) every MT=51 AND row turned into a histogram.
#   O16_kmlin.ace  : NJOY2016 O-16 293.6 K with every MT=91 law-44 table turned
#                    lin-lin in E' (pdf renormalised by trapezoid, cdf rebuilt).
# The physics of the constructed parts is artificial; the FORMAT is ACE's, and
# OpenMC's reader of the written file is the reference.
#
# Output rows: <case>,<mt>,<E eV>,<mean mu>,<mean mu*E'>,<old mean mu>,<old mean mu*E'>
#   (AND rows carry nan in the E' columns.)
# Usage: openmc-env; python correlated_angle_reference.py <U235.ace> <O16.ace> <out_dir> <out.csv>

import sys
import numpy as np
import openmc
import openmc.data as od
import openmc.data.ace as A

GX, GW = np.polynomial.legendre.leggauss(48)


def write_ace(src, xss, out):
    lines = open(src).read().splitlines()
    with open(out, "w") as f:
        f.write("\n".join(lines[:12]) + "\n")
        for k in range(0, len(xss), 4):
            f.write("".join(f"{v:20.11E}" for v in xss[k:k + 4]) + "\n")


def mtr_index(t, mt):
    x = t.xss
    mtr = [int(v) for v in x[t.jxs[3]:t.jxs[3] + int(t.nxs[4])]]
    return mtr.index(mt)


def law_block(t, mt):
    """1-based XSS index of the law data (IDAT) of reaction mt's first law."""
    x = t.xss
    i = mtr_index(t, mt)
    ldlw, dlw = t.jxs[10], t.jxs[11]
    loc = int(x[ldlw + i])
    law = int(x[dlw + loc - 1 + 1])
    idat = int(x[dlw + loc - 1 + 2])
    return law, dlw, dlw + idat - 1


def incident_locs(x, at):
    nreg = int(x[at]); at += 1 + 2 * nreg
    ne = int(x[at]); at += 1
    return [int(v) for v in x[at + ne:at + 2 * ne]]


def make_u235(src, out):
    t = A.get_table(src)
    x = t.xss
    xss = list(x[1:])  # 0-based copy: xss[j-1] is x[j]

    def get(j):
        return xss[j - 1]

    def put(j, v):
        xss[j - 1] = float(v)

    def to_histogram(d):
        n = int(get(d + 1))
        mu = np.array([get(d + 2 + j) for j in range(n)])
        p = np.array([get(d + 2 + n + j) for j in range(n)])
        h = np.diff(mu)
        area = np.sum(p[:-1] * h)
        p = p / area
        c = np.concatenate([[0.0], np.cumsum(p[:-1] * h)])
        c[-1] = 1.0
        put(d, 1)
        for j in range(n):
            put(d + 2 + n + j, p[j])
            put(d + 2 + 2 * n + j, c[j])

    # (a) MT=91 law-61 cosine rows -> histogram
    law, dlw, at = law_block(t, 91)
    assert law == 61
    seen = set()
    for L in incident_locs(x, at):
        d = dlw + L - 1
        n = int(x[d + 1])
        for j in range(n):
            lc = int(x[d + 2 + 3 * n + j])
            if lc != 0 and lc not in seen:
                seen.add(lc)
                to_histogram(dlw + abs(lc) - 1)

    land, andb = t.jxs[8], t.jxs[9]

    def and_rows(li):
        locb = int(x[land + li])
        assert locb > 0
        a = andb + locb - 1
        ne = int(x[a])
        return [int(v) for v in x[a + 1 + ne:a + 1 + 2 * ne]]

    # (b) elastic -> 32 equiprobable bins where the row has >= 11 points
    n_eq = 0
    for lc in and_rows(0):
        if lc >= 0:
            continue
        d = andb + abs(lc) - 1
        n = int(x[d + 1])
        if n < 11:
            continue
        mu = np.array(x[d + 2:d + 2 + n]); p = np.array(x[d + 2 + n:d + 2 + 2 * n])
        c = np.array(x[d + 2 + 2 * n:d + 2 + 3 * n])
        b = [-1.0]
        for q in np.arange(1, 32) / 32.0:
            k = int(np.clip(np.searchsorted(c, q, side="right") - 1, 0, n - 2))
            m = (p[k + 1] - p[k]) / (mu[k + 1] - mu[k])
            if m == 0:
                v = mu[k] + (q - c[k]) / p[k]
            else:
                v = mu[k] + (np.sqrt(max(0.0, p[k] ** 2 + 2 * m * (q - c[k]))) - p[k]) / m
            b.append(float(v))
        b.append(1.0)
        for j in range(33):
            put(d + j, b[j])
        n_eq += 1
    # flip the sign of the converted locators
    locb = int(x[land + 0]); a = andb + locb - 1; ne = int(x[a])
    for k in range(ne):
        lc = int(x[a + 1 + ne + k])
        if lc < 0 and int(x[andb + abs(lc) - 1 + 1]) >= 11:
            put(a + 1 + ne + k, -lc)

    # (c) MT=51 AND rows -> histogram
    for lc in and_rows(1 + mtr_index(t, 51)):
        if lc < 0:
            to_histogram(andb + abs(lc) - 1)
    write_ace(src, xss, out)
    return n_eq


def make_o16(src, out):
    t = A.get_table(src)
    x = t.xss
    xss = list(x[1:])
    law, dlw, at = law_block(t, 91)
    assert law == 44
    for L in incident_locs(x, at):
        d = dlw + L - 1
        assert int(x[d]) == 1
        n = int(x[d + 1])
        e = np.array(x[d + 2:d + 2 + n]); p = np.array(x[d + 2 + n:d + 2 + 2 * n])
        h = np.diff(e)
        area = np.sum(0.5 * (p[1:] + p[:-1]) * h)
        p = p / area
        c = np.concatenate([[0.0], np.cumsum(0.5 * (p[1:] + p[:-1]) * h)])
        c[-1] = 1.0
        xss[d - 1] = 2.0
        for j in range(n):
            xss[d + 2 + n + j - 1] = float(p[j])
            xss[d + 2 + 2 * n + j - 1] = float(c[j])
    write_ace(src, xss, out)


def tab_mean(t):
    x, p = np.asarray(t.x, float), np.asarray(t.p, float)
    h = np.diff(x)
    if t.interpolation == "histogram":
        return float(np.sum(0.5 * (x[1:] + x[:-1]) * p[:-1] * h))
    return float(np.sum(h * (x[:-1] * (2 * p[:-1] + p[1:]) + x[1:] * (p[:-1] + 2 * p[1:])) / 6.0))


def lincdf_mean(t):
    """Mean of outram's OLD cosine inverse: linear in cdf within each bin."""
    x, c = np.asarray(t.x, float), np.asarray(t.c, float)
    return float(np.sum(np.diff(c) * 0.5 * (x[1:] + x[:-1])))


def km_mean(r, a):
    return r * (1.0 / np.tanh(a) - 1.0 / a)


def e_raw(t, k, r1):
    x, p, c = t.x, t.p, t.c
    if t.interpolation == "histogram":
        return x[k] + (r1 - c[k]) / p[k] if p[k] > 0 else x[k]
    m = (p[k + 1] - p[k]) / (x[k + 1] - x[k])
    if m == 0:
        return x[k] + (r1 - c[k]) / p[k]
    return x[k] + (np.sqrt(np.maximum(0.0, p[k] ** 2 + 2 * m * (r1 - c[k]))) - p[k]) / m


def bracket(grid, e):
    g = np.asarray(grid)
    i = int(np.clip(np.searchsorted(g, e, side="right") - 1, 0, len(g) - 2))
    return i, float(np.clip((e - g[i]) / (g[i + 1] - g[i]), 0.0, 1.0))


def correlated_moments(dist, e):
    """(<mu>, <mu E'>) new and old, for law 61 or law 44."""
    km = type(dist).__name__ == "KalbachMann"
    i, r = bracket(dist.energy, e)
    A_, B_ = dist.energy_out[i], dist.energy_out[i + 1]
    e1 = A_.x[0] + r * (B_.x[0] - A_.x[0])
    ek = A_.x[-1] + r * (B_.x[-1] - A_.x[-1])
    new = np.zeros(2); old = np.zeros(2)
    for w, l in ((1 - r, i), (r, i + 1)):
        if w == 0:
            continue
        t = dist.energy_out[l]
        x, c = np.asarray(t.x), np.asarray(t.c)
        hist = t.interpolation == "histogram"
        scale = lambda ee: e1 + (ee - x[0]) * (ek - e1) / (x[-1] - x[0])
        if km:
            R = np.asarray(dist.precompound[l].y); S = np.asarray(dist.slope[l].y)
        else:
            mub = [tab_mean(m) for m in dist.mu[l]]
            mub_old = [lincdf_mean(m) for m in dist.mu[l]]
        for k in range(len(x) - 1):
            lo, hi = c[k], c[k + 1]
            if hi <= lo:
                continue
            mid = 0.5 * (lo + hi)
            pieces = [(lo, hi, k)] if (hist or km) else [(lo, mid, k), (mid, hi, k + 1)]
            for a0, b0, row in pieces:
                r1 = 0.5 * (b0 - a0) * GX + 0.5 * (b0 + a0)
                ww = 0.5 * (b0 - a0) * GW
                er = e_raw(t, k, r1)
                es = scale(er)
                if km:
                    if hist:
                        rr, aa = R[k], S[k]
                    else:
                        f = (er - x[k]) / (x[k + 1] - x[k])
                        rr, aa = R[k] + f * (R[k + 1] - R[k]), S[k] + f * (S[k + 1] - S[k])
                    mu = km_mean(rr, aa) * np.ones_like(r1)
                else:
                    mu = np.full_like(r1, mub[row])
                new += w * np.array([np.sum(ww * mu), np.sum(ww * mu * es)])
            # OLD outram scheme: the whole bin on row k, no r/a interpolation,
            # linear-cdf cosine inverse.
            r1 = 0.5 * (hi - lo) * GX + 0.5 * (hi + lo)
            ww = 0.5 * (hi - lo) * GW
            es = scale(e_raw(t, k, r1))
            muo = km_mean(R[k], S[k]) if km else mub_old[k]
            old += w * np.array([np.sum(ww) * muo, np.sum(ww * es) * muo])
    return new, old


def and_mean(ang, e):
    i, r = bracket(ang.energy, e)
    return sum(w * tab_mean(ang.mu[j]) for w, j in ((1 - r, i), (r, i + 1)) if w > 0)


def main(u235, o16, out_dir, out_csv):
    uh = f"{out_dir}/U235_hist.ace"
    ok = f"{out_dir}/O16_kmlin.ace"
    n_eq = make_u235(u235, uh)
    make_o16(o16, ok)
    print("elastic rows made equiprobable:", n_eq)
    rows = []
    cases = [("U235", u235, [(91, [1.0e6, 2.0e6, 5.0e6, 1.4e7]), (16, [1.4e7, 2.0e7])]),
             ("U235_hist", uh, [(91, [2.0e6, 5.0e6, 1.4e7])]),
             ("O16", o16, [(91, [1.2e7, 1.4e7, 2.0e7])]),
             ("O16_kmlin", ok, [(91, [1.2e7, 1.4e7, 2.0e7])])]
    for name, f, spec in cases:
        d = od.IncidentNeutron.from_ace(f)
        for mt, es in spec:
            dist = d.reactions[mt].products[0].distribution[0]
            if name == "U235_hist":
                assert dist.mu[5][3].interpolation == "histogram"
            if name == "O16_kmlin":
                assert dist.energy_out[3].interpolation == "linear-linear"
            for e in es:
                new, old = correlated_moments(dist, e)
                rows.append(f"{name},{mt},{e!r},{float(new[0])!r},{float(new[1])!r},{float(old[0])!r},{float(old[1])!r}")
                print(rows[-1])
    d = od.IncidentNeutron.from_ace(uh)
    el = d.reactions[2].products[0].distribution[0].angle
    assert any(m.interpolation == "histogram" and len(m.x) == 33 for m in el.mu)
    for e in [1.0e5, 1.0e6, 5.0e6]:
        rows.append(f"U235_hist,2,{e!r},{float(and_mean(el, e))!r},nan,nan,nan")
        print(rows[-1])
    a51 = d.reactions[51].products[0].distribution[0].angle
    assert all(m.interpolation == "histogram" for m in a51.mu if len(m.x) > 2)
    for e in [1.0e5, 1.0e6, 5.0e6]:
        rows.append(f"U235_hist,51,{e!r},{float(and_mean(a51, e))!r},nan,nan,nan")
        print(rows[-1])
    with open(out_csv, "w") as f:
        f.write("# generated by correlated_angle_reference.py, openmc " + openmc.__version__ + "\n")
        f.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main(*sys.argv[1:5])
