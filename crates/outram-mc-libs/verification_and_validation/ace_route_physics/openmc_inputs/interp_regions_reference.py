#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0
#
# ACE nu and yield tables WITH NON-LIN-LIN INTERPOLATION REGIONS, constructed
# from NJOY2016 U-235 (ENDF/B-VIII.0, 293.6 K), and OpenMC's values of them,
# for GitHub #365 audit.
#
# Own work (outram-park-backend), GPL-3.0; OpenMC Python API (MIT) as a library.
#
# Every nu table in ENDF/B-VIII.0 is lin-lin (census 2026-09-29: 86 MT=452,
# 86 MT=456, 84 MT=455) and the only non-lin-lin neutron yield is F-19's
# histogram, which is constant. No real table exercises the regions, so
# this script appends new blocks to XSS and repoints the locators:
#   NU    : prompt+total form (first word -len). Both tables are U-235's
#           total grid split into five regions with INT = 1,2,3,4,5 in turn.
#           Prompt = total - 0.0165.
#   DNU   : U-235's delayed table on two regions, INT 4 (log-lin) then 1
#           (histogram).
#   BDD   : group 1's probability made energy dependent on U-235's DNU grid:
#           p1(E) = p1 * (1 + 0.5 x), x in [0, 1] along the grid, on two
#           regions (INT 5, then 3). The other groups are left as they are.
#   MT=5  : the TY = -101 yield TAB1 replaced by U-235's own yield on three
#           regions (INT 1, 5, 2).
# The physics is artificial; the FORMAT is ACE's.
#
# Reference values: OpenMC's C++ Tabulated1D::operator() (src/endf.cpp),
# re-implemented below (tab1d) because transport evaluates tables in C++.
# Python's Tabulated1D.__call__ is printed beside it as a cross-check.
# BDD: the expected group share is p_g(E) / sum_g p_g(E_first), which is
# outram's renormalisation. OpenMC's reader instead multiplies an energy-
# dependent p_g by nu_d on a union grid and re-tabulates the product lin-lin
# (reaction.py:338-347). That is an approximation of the product and is NOT
# the reference here; the V&V note records the difference.
# Output rows: <quantity>,<E eV>,<value>,<value if read lin-lin (the old behaviour)>
# Usage: openmc-env; python interp_regions_reference.py <U235.ace> <out.ace> <out.csv>

import sys
import math
import numpy as np
import openmc
import openmc.data as od
import openmc.data.ace as A


def tab1d(x, y, nbt, ints, e):
    """OpenMC C++ Tabulated1D::operator(), exactly (0-based nbt = NBT - 1)."""
    n = len(x)
    if e < x[0]:
        return y[0]
    if e > x[-1]:
        return y[-1]
    i = 0 if e == x[0] else int(np.searchsorted(x, e, side="left")) - 1
    i = min(max(i, 0), n - 2)
    law = ints[0] if len(ints) else 2
    for b, it in zip(nbt, ints):
        if i < b - 1:
            law = it
            break
    x0, x1, y0, y1 = x[i], x[i + 1], y[i], y[i + 1]
    if law == 1:
        return y0
    if law == 2:
        return y0 + (e - x0) / (x1 - x0) * (y1 - y0)
    if law == 3:
        return y0 + math.log(e / x0) / math.log(x1 / x0) * (y1 - y0)
    if law == 4:
        return y0 * math.exp((e - x0) / (x1 - x0) * math.log(y1 / y0))
    return y0 * math.exp(math.log(e / x0) / math.log(x1 / x0) * math.log(y1 / y0))


def tab_words(x_mev, y, nbt, ints):
    return [float(len(nbt))] + [float(b) for b in nbt] + [float(i) for i in ints] + \
        [float(len(x_mev))] + list(map(float, x_mev)) + list(map(float, y))


def read_tab(x, at):
    nr = int(x[at])
    nbt = [int(v) for v in x[at + 1:at + 1 + nr]]
    ints = [int(v) for v in x[at + 1 + nr:at + 1 + 2 * nr]]
    j = at + 1 + 2 * nr
    ne = int(x[j])
    return nbt, ints, np.array(x[j + 1:j + 1 + ne]), np.array(x[j + 1 + ne:j + 1 + 2 * ne]), j + 1 + 2 * ne


def split(n, k):
    """k region breakpoints (1-based last point of each) over n points."""
    return [int(round(n * (r + 1) / k)) for r in range(k - 1)] + [n]


def main(src, out_ace, out_csv):
    lines = open(src).read().splitlines()
    t = A.get_table(src)
    x = t.xss
    xss = list(x[1:])
    nxs = [int(v) for v in t.nxs[1:17]]
    jxs = [int(v) for v in t.jxs[1:33]]

    def append(words):
        loc = len(xss) + 1
        xss.extend(words)
        return loc

    # --- NU: total table of the original (prompt+total or single) ---
    nu_at = t.jxs[2]
    if x[nu_at] < 0:
        nu_at = nu_at + int(abs(x[nu_at])) + 1
    assert int(x[nu_at]) == 2
    _, _, ex, ey, _ = read_tab(x, nu_at + 1)
    nbt_nu = split(len(ex), 5)
    ints_nu = [1, 2, 3, 4, 5]
    total = [2.0] + tab_words(ex, ey, nbt_nu, ints_nu)
    prompt = [2.0] + tab_words(ex, ey - 0.0165, nbt_nu, ints_nu)
    loc = append([-float(len(prompt))] + prompt + total)
    jxs[1] = loc

    # --- DNU ---
    dnu = t.jxs[24]
    assert dnu > 0 and int(x[dnu]) == 2
    _, _, dx, dy, _ = read_tab(x, dnu + 1)
    nbt_d, ints_d = split(len(dx), 2), [4, 1]
    jxs[23] = append([2.0] + tab_words(dx, dy, nbt_d, ints_d))

    # --- BDD: group 1 energy dependent ---
    at = t.jxs[25]
    groups = []
    for g in range(int(t.nxs[8])):
        lam = x[at]
        nbt, ints, gx, gy, nxt = read_tab(x, at + 1)
        groups.append((lam, nbt, ints, gx, gy))
        at = nxt
    lam, _, _, _, gy0 = groups[0]
    p1 = float(gy0[0])
    frac = np.linspace(0.0, 1.0, len(dx))
    g1y = p1 * (1.0 + 0.5 * frac)
    nbt_g, ints_g = split(len(dx), 2), [5, 3]
    words = [lam] + tab_words(dx, g1y, nbt_g, ints_g)
    for lam_k, nbt, ints, gx, gy in groups[1:]:
        words += [lam_k] + tab_words(gx, gy, nbt, ints)
    jxs[24] = append(words)
    p_first_sum = g1y[0] + sum(float(g[4][0]) for g in groups[1:])

    # --- MT=5 yield ---
    mtr = [int(v) for v in x[t.jxs[3]:t.jxs[3] + int(t.nxs[4])]]
    i5 = mtr.index(5)
    ty = int(x[t.jxs[5] + i5])
    assert abs(ty) > 100
    yat = t.jxs[11] + abs(ty) - 101
    _, _, yx, yy, _ = read_tab(x, yat)
    nbt_y, ints_y = split(len(yx), 3), [1, 5, 2]
    yloc = append(tab_words(yx, yy, nbt_y, ints_y))
    new_ty = 100 + (yloc - t.jxs[11] + 1)
    xss[t.jxs[5] + i5 - 1] = float(-new_ty if ty < 0 else new_ty)

    nxs[0] = len(xss)
    ints_all = nxs + jxs
    with open(out_ace, "w") as f:
        f.write("\n".join(lines[:6]) + "\n")
        for r in range(6):
            f.write("".join(f"{v:9d}" for v in ints_all[8 * r:8 * r + 8]) + "\n")
        for k in range(0, len(xss), 4):
            f.write("".join(f"{v:20.11E}" for v in xss[k:k + 4]) + "\n")

    # --- OpenMC's reading, and the reference values ---
    d = od.IncidentNeutron.from_ace(out_ace)
    tot = d.reactions[18].derived_products[0]
    assert tot.emission_mode == "total"
    assert list(tot.yield_.interpolation) == ints_nu, tot.yield_.interpolation
    y5 = d.reactions[5].products[0].yield_
    assert list(y5.interpolation) == ints_y
    t2 = A.get_table(out_ace)
    dnu_tab = od.Tabulated1D.from_ace(t2, t2.jxs[24] + 1)
    assert list(dnu_tab.interpolation) == ints_d
    g1 = od.Tabulated1D.from_ace(t2, t2.jxs[25] + 1, convert_units=True)
    assert list(g1.interpolation) == ints_g

    def probe(xs_ev):
        """Energies inside every region, at interior grid points' midpoints,
        and outside both ends."""
        xs = np.asarray(xs_ev)
        mids = np.sqrt(xs[:-1] * xs[1:])
        pick = mids[np.linspace(0, len(mids) - 1, 12).astype(int)]
        return [xs[0] * 0.5] + list(pick) + [xs[-1] * 1.5]

    rows = []
    E = ex * 1e6
    for e in probe(E):
        v = tab1d(E, ey, nbt_nu, ints_nu, e)
        rows.append(("nu_total", e, v, float(tot.yield_(e)), tab1d(E, ey, [], [], e)))
    D = dx * 1e6
    for e in probe(D):
        rows.append(("nu_delayed", e, tab1d(D, dy, nbt_d, ints_d, e), float(dnu_tab(e)),
                     tab1d(D, dy, [], [], e)))
        pg = tab1d(D, g1y, nbt_g, ints_g, e) / p_first_sum
        rows.append(("group1_share", e, pg, float(g1(e)) / p_first_sum,
                     tab1d(D, g1y, [], [], e) / p_first_sum))
    Y = yx * 1e6
    for e in probe(Y):
        rows.append(("mt5_yield", e, tab1d(Y, yy, nbt_y, ints_y, e), float(y5(e)),
                     tab1d(Y, yy, [], [], e)))
    worst = 0.0
    for q, e, v, py, _ in rows:
        inside = e >= min(E[0], D[0], Y[0]) and e <= max(E[-1], D[-1], Y[-1])
        if inside and v != 0:
            worst = max(worst, abs(py - v) / abs(v))
    print("rows", len(rows), "worst |python - C++ rule| (inside)", worst)
    with open(out_csv, "w") as f:
        f.write("# generated by interp_regions_reference.py, openmc " + openmc.__version__ + "\n")
        for q, e, v, _, lin in rows:
            f.write(f"{q},{float(e)!r},{float(v)!r},{float(lin)!r}\n")


if __name__ == "__main__":
    main(*sys.argv[1:4])
