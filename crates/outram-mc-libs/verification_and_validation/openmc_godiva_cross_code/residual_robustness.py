#!/usr/bin/env python3
"""Is the `op-os8x` spectral residual statistically robust, or an artefact of a
small ensemble?

WHY THIS EXISTS
---------------
By 2026-09-17 every physical component of the comparison had been measured and
excluded: the cross sections (now BAND-AVERAGED, not just flux-weighted, to
1e-6), both angular laws, the MT=91 transfer table, the within-row CDF
inversion, the inter-row unit-base rule, the CM->lab transform, channel
branching, chi's mean and chi's SHAPE.

When every component of a difference agrees and the difference does not, the
next thing to doubt is the difference itself. `compare_spectrum.py` already uses
seed-to-seed spread as its uncertainty -- which is the right construction -- but
it does so with 8 seeds a side, and the standard deviation of a sample of 8
carries about 25 % uncertainty of its own. A "4.6 sigma" built on such an sd
could really be nearer 3.5, and a genuinely marginal effect could look solid.

This reads the committed statepoints directly (no transport, no `openmc`
executable needed) and reports the OpenMC side's own seed-to-seed scatter in the
two bands the residual lives in. If that scatter is comparable to the residual,
the residual is not established; if it is far smaller, it is.

RESULT (2026-09-17, 8 seeds, ptables off)
-----------------------------------------
    band            share      rel. sem     residual      sigma from this side
    67-174 keV      0.074951   0.232 %      -1.2..-1.9 %  5-8
    1.9-3.0 MeV     0.230661   0.078 %      +0.42 %       5.4

So the residual is ROBUST: the "small ensemble" explanation is not supported,
and `op-os8x` is a real difference between two codes whose every measured
component agrees to 1e-6.

USAGE
-----
    python3 residual_robustness.py
"""
import glob
import os
import sys

import numpy as np
import openmc

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.path.join(HERE, "work")

# (lo, hi, label, the recorded flux residual in that band)
BANDS = [
    (6.7e4, 1.74e5, "67-174 keV", "-1.2..-1.9 %"),
    (1.9e6, 3.0e6, "1.9-3.0 MeV", "+0.42 %"),
    (1.0e6, 3.0e6, "1.0-3.0 MeV", "(context)"),
]


def main():
    specs, edges = [], None
    for d in sorted(glob.glob(os.path.join(WORK, "run_ptables_off_seed*"))):
        p = os.path.join(d, "statepoint.160.h5")
        if not os.path.exists(p):
            continue
        sp = openmc.StatePoint(p)
        t = sp.get_tally(name="flux spectrum")
        m = t.mean.flatten()
        specs.append(m / m.sum())
        edges = t.find_filter(openmc.EnergyFilter).bins
    if len(specs) < 3:
        print(f"only {len(specs)} statepoints; need at least 3", file=sys.stderr)
        return 1
    specs = np.array(specs)
    lo = np.array([b[0] for b in edges])
    hi = np.array([b[1] for b in edges])

    print(f"OpenMC seed-to-seed scatter over {specs.shape[0]} seeds (normalised flux shares):")
    print(f"  {'band':>14} {'bins':>5} {'share':>10} {'sd':>10} {'rel sem':>9}  residual")
    for a, b, label, resid in BANDS:
        m = (hi > a) & (lo < b)  # overlap, since the 50-bin log grid has no edge at 1.9 MeV
        v = specs[:, m].sum(axis=1)
        sd = v.std(ddof=1)
        rel = sd / np.sqrt(len(v)) / v.mean() * 100
        print(
            f"  {label:>14} {m.sum():5d} {v.mean():10.6f} {sd:10.2e} {rel:8.3f}%  {resid}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
