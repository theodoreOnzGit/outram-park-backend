#!/usr/bin/env python3
"""Closed-form expected <E'> of the unit-base interpolated MT=91 law, OFF-GRID.

WHY
---
`tests/mt91_transfer_vs_openmc.rs` showed our MT=91 f0(E->E') matches OpenMC's
on every tabulated incident row to 7 significant figures. But a row-by-row
comparison is structurally blind to the INTER-ROW rule: at an incident energy
between two tabulated rows, the sampled spectrum is built by unit-base
interpolation, and a defect in that construction (wrong branch probability,
wrong envelope, wrong bin lookup) leaves every row identical while moving the
sampled spectrum. Godiva's neutrons overwhelmingly arrive off-grid.

This is the remaining lead-1 for `op-os8x`, and it needs no transport.

THE CONSTRUCTION, AND ITS EXACT MEAN
------------------------------------
Unit-base interpolation between rows i and i+1 at factor r:

  envelope   E_1 = E[i,1] + r*(E[i+1,1] - E[i,1])
             E_k = E[i,k] + r*(E[i+1,k] - E[i,k])
  branch     draw from row i+1 with probability r, else from row i
  scale      E' = E_1 + (E_s - E[l,1]) * (E_k - E_1) / (E[l,k] - E[l,1])

The scaling is affine in the sampled value, so the expectation passes straight
through it and the mean has a closed form in each row's own mean and endpoints:

  <E'> = (1-r) * [E_1 + (m_i   - E[i,1])   * (E_k-E_1)/(E[i,k]  -E[i,1])]
       +   r   * [E_1 + (m_i+1 - E[i+1,1]) * (E_k-E_1)/(E[i+1,k]-E[i+1,1])]

So no sampler is re-implemented on either side: the reference is OpenMC's own
tabulated rows put through the published algorithm in closed form, and the Rust
test compares its SAMPLED mean against it. A disagreement is a defect in our
inter-row construction; agreement excludes it.

OUTPUT
------
`mt91_interrow_oracle.csv`: e_in_ev, expected_mean_eout_ev, r, e_lo_ev, e_hi_ev
"""
import csv
import numpy as np
import openmc.data

HDF5 = "work/U238.h5"
OUT = "mt91_interrow_oracle.csv"



def _exact_mean(x, p):
    """Exact mean of a lin-lin tabulated pdf.

    CORRECTION, 2026-09-16. This was `np.trapezoid(x*p, x) / np.trapezoid(p, x)`.
    The denominator is fine -- `p` is linear between points, so the trapezoid
    rule integrates it EXACTLY. The numerator is not: `x*p(x)` is QUADRATIC on
    each bin, and the trapezoid rule is exact only for linear integrands. The
    error per bin is -h^3 * m / 6 with `m` the pdf slope, so it grows with bin
    width and with how steeply the pdf falls -- which on this law means it grows
    with incident energy.

    Measured on U-238 MT=91: the trapezoid mean is +0.089 % off at 1.945 MeV,
    -0.259 % at 2.4 MeV and -1.632 % at 3.0 MeV. Using it as a reference made a
    CORRECT sampler look like it carried a growing bias, and very nearly put a
    false defect on the op-os8x record. The sampler agrees with the exact mean
    to a flat -0.05 % at every energy.
    """
    x = np.asarray(x, float)
    p = np.asarray(p, float)
    a, b = x[:-1], x[1:]
    pa, pb = p[:-1], p[1:]
    h = b - a
    ok = h > 0
    a, b, pa, pb, h = a[ok], b[ok], pa[ok], pb[ok], h[ok]
    m = (pb - pa) / h
    num = np.sum(pa * (b**2 - a**2) / 2.0
                 + m * ((b**3 - a**3) / 3.0 - a * (b**2 - a**2) / 2.0))
    den = np.sum(0.5 * h * (pa + pb))        # exact for a linear p
    return float(num / den)


def row_mean(dist):
    return _exact_mean(dist.x, dist.p)


def main():
    u238 = openmc.data.IncidentNeutron.from_hdf5(HDF5)
    d = u238.reactions[91].products[0].distribution[0]
    e_in = np.asarray(d.energy, float)

    # Probe energies deliberately BETWEEN tabulated rows, spread across the
    # 1.9-3.0 MeV band where op-os8x's excess flux sits. Each is checked to be
    # strictly interior to its bracket, so none accidentally lands on a row.
    probes = [1.97e6, 2.05e6, 2.15e6, 2.3e6, 2.45e6, 2.62e6, 2.9e6, 3.05e6]

    rows = []
    for e in probes:
        i = int(np.searchsorted(e_in, e) - 1)
        assert 0 <= i < len(e_in) - 1, e
        lo, hi = float(e_in[i]), float(e_in[i + 1])
        assert lo < e < hi, f"probe {e} is not strictly between rows ({lo}, {hi})"
        r = (e - lo) / (hi - lo)

        di, di1 = d.energy_out[i], d.energy_out[i + 1]
        xi, xi1 = np.asarray(di.x, float), np.asarray(di1.x, float)
        e_i_1, e_i_k = float(xi[0]), float(xi[-1])
        e_i1_1, e_i1_k = float(xi1[0]), float(xi1[-1])
        m_i, m_i1 = row_mean(di), row_mean(di1)

        e_1 = e_i_1 + r * (e_i1_1 - e_i_1)
        e_k = e_i_k + r * (e_i1_k - e_i_k)
        span = e_k - e_1
        term_i = e_1 + (m_i - e_i_1) * span / (e_i_k - e_i_1)
        term_i1 = e_1 + (m_i1 - e_i1_1) * span / (e_i1_k - e_i1_1)
        expected = (1.0 - r) * term_i + r * term_i1

        rows.append((f"{e:.6e}", f"{expected:.6e}", f"{r:.6f}",
                     f"{lo:.6e}", f"{hi:.6e}"))

    with open(OUT, "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["e_in_ev", "expected_mean_eout_ev", "r", "e_lo_ev", "e_hi_ev"])
        w.writerows(rows)

    print(f"OpenMC {openmc.__version__}: U-238 MT=91 unit-base expected means -> {OUT}")
    for r_ in rows:
        print(f"  E_in {r_[0]}  r={r_[2]}  bracket [{r_[3]}, {r_[4]}]  <E'>exp {r_[1]}")


if __name__ == "__main__":
    main()
