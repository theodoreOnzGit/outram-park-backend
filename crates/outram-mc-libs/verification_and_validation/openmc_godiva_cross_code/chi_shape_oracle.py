#!/usr/bin/env python3
"""Extract OpenMC's U-235 fission-spectrum SHAPE as a comparison oracle.

WHY THIS EXISTS
---------------
`op-os8x` is localised to +0.42 % excess flux at 1.9-3.0 MeV against 1.2-1.9 %
deficits at 67-174 keV. Every candidate on the study's list has now been
measured and excluded: the cross sections, both angular laws, the MT=91 transfer
table, the within-row CDF inversion, the inter-row unit-base rule, the CM->lab
transform, and channel branching.

chi was "cleared" too -- but only on its MEAN. `tests/fission_spectrum_vs_openmc.rs`
compares <E_out> and finds 0.008-0.018 %. A mean cannot see a redistribution
that moves probability from ~100 keV into the MeV window while conserving <E>,
because the 10+ MeV tail can compensate. That is precisely the residual's
signature, and chi is the largest single source of neutrons in a bare fast
assembly -- every neutron is born from it.

So this extracts the shape, not the mean: the fraction of fission neutrons born
in each of a set of energy bands, including the two bands the residual lives in.

Both sides trace to the same ENDF/B-VIII.0 U-235 evaluation: this repo's
`reference-data/endf/n-092_U_235-ENDF8.0.endf` -> NJOY2016 -> ACE -> HDF5, and
`njoy-outram-park-fork` reads that same tape's MF=5 directly. A difference is a
port defect, not a data difference.

USAGE
-----
    python3 chi_shape_oracle.py          # writes chi_shape_oracle.csv
"""
import csv
import os
import sys

import numpy as np
import openmc.data

HERE = os.path.dirname(os.path.abspath(__file__))
U235 = os.path.join(HERE, "work", "U235.h5")

# The bands `op-os8x` is localised to, plus a coarse spanning set so a
# redistribution anywhere shows up rather than only where it was expected.
BANDS = [
    (1.0e-3, 1.0e3),
    (1.0e3, 6.7e4),
    (6.7e4, 1.74e5),   # the DEFICIT band
    (1.74e5, 1.0e6),
    (1.0e6, 1.9e6),
    (1.9e6, 3.0e6),    # the EXCESS band
    (3.0e6, 6.0e6),
    (6.0e6, 1.0e7),
    (1.0e7, 3.0e7),
]

# Incident energies to report the shape at. Godiva's flux is fast, so the
# fission-inducing energies that matter are ~0.1-5 MeV.
INCIDENT = [1.0e3, 1.0e5, 1.0e6, 2.0e6, 5.0e6]


def chi_cdf_fractions(dist, e_in, bands):
    """Fraction of outgoing neutrons in each band, for one incident energy.

    `dist` is OpenMC's energy distribution for MT=18. The tabulated form is a
    `ContinuousTabular`: a list of per-incident-energy `Tabular` outgoing
    distributions plus the incident grid. The band fraction is integrated from
    the tabulated pdf directly rather than sampled, so the oracle carries no
    Monte Carlo noise of its own.
    """
    ee = np.asarray(dist.energy)
    i = np.searchsorted(ee, e_in) - 1
    i = max(0, min(i, len(ee) - 2))
    r = 0.0 if ee[i + 1] == ee[i] else (e_in - ee[i]) / (ee[i + 1] - ee[i])

    def band_fracs(tab):
        x = np.asarray(tab.x, dtype=float)
        p = np.asarray(tab.p, dtype=float)
        # Normalise: the stored pdf integrates to 1 already, but renormalise so
        # a truncated grid cannot bias the fractions.
        total = np.trapezoid(p, x) if hasattr(np, "trapezoid") else np.trapz(p, x)
        out = []
        for lo, hi in bands:
            m_lo, m_hi = max(lo, x[0]), min(hi, x[-1])
            if m_hi <= m_lo:
                out.append(0.0)
                continue
            # Integrate the piecewise-linear pdf on [m_lo, m_hi] exactly by
            # inserting the band edges into the grid.
            xs = np.unique(np.concatenate([x[(x > m_lo) & (x < m_hi)], [m_lo, m_hi]]))
            ps = np.interp(xs, x, p)
            seg = np.trapezoid(ps, xs) if hasattr(np, "trapezoid") else np.trapz(ps, xs)
            out.append(seg / total)
        return np.array(out)

    # OpenMC samples table i or i+1 with probability r, so the EXPECTATION of
    # its band fractions is the linear interpolation of the two. Reading the
    # nearest table instead is the trap this study hit three times.
    lo = band_fracs(dist.energy_out[i])
    hi = band_fracs(dist.energy_out[i + 1])
    return (1.0 - r) * lo + r * hi


def main():
    if not os.path.exists(U235):
        print(f"missing {U235}; run build_data.py first", file=sys.stderr)
        return 1
    u235 = openmc.data.IncidentNeutron.from_hdf5(U235)
    fission = u235.reactions[18]
    prod = fission.products[0]
    # `distribution[0]` is an UncorrelatedAngleEnergy wrapper; the chi table is
    # its `.energy` (a ContinuousTabular).
    dist = prod.distribution[0].energy
    print(f"U-235 MT=18 product 0 distribution: {type(dist).__name__}")

    rows = []
    for e_in in INCIDENT:
        fr = chi_cdf_fractions(dist, e_in, BANDS)
        for (lo, hi), f in zip(BANDS, fr):
            rows.append(
                {
                    "e_in_ev": f"{e_in:.6e}",
                    "band_lo_ev": f"{lo:.6e}",
                    "band_hi_ev": f"{hi:.6e}",
                    "fraction": f"{f:.9e}",
                }
            )
        print(
            f"  E_in = {e_in:9.3e} eV: "
            + " ".join(f"{f*100:6.3f}%" for f in fr)
            + f"   (sum {fr.sum():.6f})"
        )

    out = os.path.join(HERE, "chi_shape_oracle.csv")
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)
    print(f"wrote {out} ({len(rows)} rows)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
