#!/usr/bin/env python3
"""OpenMC's BAND-AVERAGED cross sections in the two bands `op-os8x` lives in.

WHY THIS EXISTS
---------------
`op-os8x` is +0.42 % excess flux at 1.9-3.0 MeV against 1.2-1.9 % deficits at
67-174 keV. The cross sections were "excluded" by `compare_xs.py`, which reports
two things: the FLUX-WEIGHTED difference over the whole range (<=0.06 %) and the
worst POINTWISE difference (large, and expected, since two independently thinned
grids never line up inside a resonance).

Neither is the quantity that sets the flux in a band. A flux-weighted average
over the whole spectrum is a MEAN, and a mean cannot see a redistribution -- the
same blind spot that hid chi's shape behind chi's mean until it was measured
band by band. And a worst-pointwise number is dominated by grid alignment, so it
is deliberately not chased and cannot exclude anything either.

What governs the flux in a band is the BAND-AVERAGED macroscopic cross section:
a 1 % difference in mean sigma_t across 67-174 keV produces about a 1 % flux
difference there, which is the size of the residual.

So this computes, per band, the lethargy-averaged microscopic cross section
    <sigma>_band = int sigma(E) dE/E / int dE/E
which weights each decade equally and does not need a flux -- deliberately, so
the comparison cannot be contaminated by the very flux difference it is meant to
explain.

USAGE
-----
    python3 band_xs_oracle.py          # writes band_xs_oracle.csv
"""
import csv
import os
import sys

import numpy as np
import openmc.data

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.path.join(HERE, "work")
T = "294K"

# The two residual bands, plus neighbours so a difference can be located rather
# than only detected.
BANDS = [
    (2.0e4, 6.7e4),
    (6.7e4, 1.74e5),   # the flux DEFICIT band
    (1.74e5, 6.0e5),
    (6.0e5, 1.9e6),
    (1.9e6, 3.0e6),    # the flux EXCESS band
    (3.0e6, 6.0e6),
]

# Only the channels the transport kernel partitions on, per compare_xs.py's own
# reasoning; plus their sum, which is what a mean free path actually sees.
MTS = {"elastic": 2, "fission": 18, "capture": 102}

NUCLIDES = [("U235", "U235.h5"), ("U238", "U238.h5")]

NSUB = 4000  # log-spaced sub-samples per band


def band_average(xs_fn, lo, hi, n=NSUB):
    """<sigma> over [lo, hi] weighted by dE/E (flat in lethargy)."""
    ln = np.linspace(np.log(lo), np.log(hi), n)
    e = np.exp(ln)
    s = xs_fn(e)
    # trapezoid in ln E == dE/E weighting
    num = np.trapezoid(s, ln) if hasattr(np, "trapezoid") else np.trapz(s, ln)
    return num / (ln[-1] - ln[0])


def main():
    rows = []
    for name, fn in NUCLIDES:
        path = os.path.join(WORK, fn)
        if not os.path.exists(path):
            print(f"missing {path}", file=sys.stderr)
            return 1
        nuc = openmc.data.IncidentNeutron.from_hdf5(path)
        print(f"\n{name}: band-averaged microscopic cross sections (barn)")
        header = f"{'band lo':>10} {'band hi':>10}"
        for k in list(MTS) + ["inelastic", "sum"]:
            header += f" {k:>12}"
        print(header)
        for lo, hi in BANDS:
            vals = {}
            for label, mt in MTS.items():
                if mt in nuc.reactions:
                    xs = nuc.reactions[mt].xs[T]
                    vals[label] = band_average(xs, lo, hi)
                else:
                    vals[label] = 0.0
            # inelastic: sum MT=51..91 (MT=4 is not present on every ACE file)
            inel = 0.0
            for mt in list(range(51, 92)):
                if mt in nuc.reactions:
                    inel += band_average(nuc.reactions[mt].xs[T], lo, hi)
            vals["inelastic"] = inel
            vals["sum"] = sum(vals[k] for k in list(MTS) + ["inelastic"])
            line = f"{lo:10.3e} {hi:10.3e}"
            for k in list(MTS) + ["inelastic", "sum"]:
                line += f" {vals[k]:12.6f}"
            print(line)
            for k, v in vals.items():
                rows.append(
                    {
                        "nuclide": name,
                        "band_lo_ev": f"{lo:.6e}",
                        "band_hi_ev": f"{hi:.6e}",
                        "channel": k,
                        "sigma_barn": f"{v:.9e}",
                    }
                )
    out = os.path.join(HERE, "band_xs_oracle.csv")
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)
    print(f"\nwrote {out} ({len(rows)} rows)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
