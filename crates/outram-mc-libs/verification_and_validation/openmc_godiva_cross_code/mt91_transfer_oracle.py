#!/usr/bin/env python3
"""Extract OpenMC's U-238 MT=91 outgoing-energy law as a comparison oracle.

WHY THIS EXISTS
---------------
`op-os8x` is localised to +0.88 % excess flux at 1.9-3.0 MeV against 1.4-1.9 %
deficits at 67-174 keV -- a deficit of down-scatter out of the MeV window. The
angular laws and the cross sections are excluded by measurement, leaving the
MT=91 continuum f0(E->E') SHAPE as the leading suspect.

A per-MT collision *rate* tally cannot discriminate it: rates are flux x sigma,
the cross sections already agree to <=0.06 % flux-weighted, so a rate comparison
would mostly restate the flux difference it is meant to explain. What
discriminates is the TRANSFER itself -- where an MT=91 collision at 2-3 MeV puts
the neutron. That is a data-level comparison needing no transport at all.

Both sides trace to the same ENDF/B-VIII.0 U-238 evaluation: this repo's
`reference-data/endf/n-092_U_238.endf` -> NJOY2016 -> ACE -> HDF5 (see
`build_data.py` / `make_ace.sh`), and `njoy-outram-park-fork` reads that same
tape's MF=6 directly. So any difference is a port defect, not a data difference.

OUTPUT
------
`mt91_transfer_oracle.csv`, committed, one row per incident-energy point of
OpenMC's own law in [1.5, 3.5] MeV:

    e_in_ev, mean_eout_ev, median_eout_ev, frac_below_300kev, n_points

`mean` is the first moment of the tabulated pdf, integrated EXACTLY for a
lin-lin table (see `_exact_mean` -- the trapezoid rule is wrong here and nearly
cost this study a false defect); `frac_below_300kev` is the
fraction of emitted neutrons landing under 300 keV, chosen because the 67-174
keV deficit is where op-os8x's missing flux went.
"""
import csv
import numpy as np
import openmc.data

HDF5 = "work/U238.h5"
BAND = (1.5e6, 3.5e6)
OUT = "mt91_transfer_oracle.csv"



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

def moments(dist):
    x = np.asarray(dist.x, dtype=float)
    p = np.asarray(dist.p, dtype=float)
    norm = np.trapezoid(p, x)
    if norm <= 0:
        return None
    mean = _exact_mean(x, p)
    cdf = np.concatenate([[0.0], np.cumsum(0.5 * np.diff(x) * (p[1:] + p[:-1]))]) / norm
    median = float(np.interp(0.5, cdf, x))
    below = float(np.interp(3.0e5, x, cdf))
    return mean, median, below, len(x)


def main():
    u238 = openmc.data.IncidentNeutron.from_hdf5(HDF5)
    dist = u238.reactions[91].products[0].distribution[0]
    e_in = np.asarray(dist.energy, dtype=float)
    rows = []
    for i, e in enumerate(e_in):
        if not (BAND[0] <= e <= BAND[1]):
            continue
        m = moments(dist.energy_out[i])
        if m is None:
            continue
        mean, median, below, n = m
        rows.append((f"{e:.6e}", f"{mean:.6e}", f"{median:.6e}", f"{below:.6f}", n))

    with open(OUT, "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["e_in_ev", "mean_eout_ev", "median_eout_ev", "frac_below_300kev", "n_points"])
        w.writerows(rows)

    print(f"OpenMC {openmc.__version__}: U-238 MT=91, {len(rows)} incident rows in "
          f"[{BAND[0]:.2e}, {BAND[1]:.2e}] eV -> {OUT}")
    for r in rows:
        print(f"  E_in {r[0]}  <E'> {r[1]}  med {r[2]}  P(E'<300keV) {r[3]}  ({r[4]} pts)")


if __name__ == "__main__":
    main()
