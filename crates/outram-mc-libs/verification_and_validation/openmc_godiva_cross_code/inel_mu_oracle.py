#!/usr/bin/env python3
"""OpenMC's own inelastic-level angular data as an oracle for the Rust port.

Reads the SAME ACE file OpenMC transports with -- produced by the same NJOY2016
build from the same ENDF/B-VIII.0 tape -- and reports the mean CM cosine of each
discrete inelastic level at a set of incident energies.

THE NEAREST-POINT TRAP, which this study hit three times: OpenMC's
AngleDistribution::sample picks tabulated table i or i+1 with probability r, so
its EXPECTATION is the linear interpolation between the two. Reading the nearest
tabulated energy instead is NOT what OpenMC does and disagrees with it. This
script interpolates.
"""
import sys, numpy as np, openmc.data

def mubar_of(dist):
    """Mean cosine of one openmc Univariate angular distribution."""
    if isinstance(dist, openmc.stats.Uniform):
        return 0.0
    if isinstance(dist, openmc.stats.Tabular):
        x, p = np.asarray(dist.x, float), np.asarray(dist.p, float)
        if dist.interpolation == 'histogram':
            # p is constant on [x_i, x_i+1); mean of mu over each bin
            w = np.diff(x) * p[:-1]
            m = np.diff(x) * p[:-1] * 0.5 * (x[:-1] + x[1:])
            return m.sum() / w.sum()
        w = np.trapezoid(p, x)
        return np.trapezoid(p * x, x) / w
    if isinstance(dist, openmc.stats.Mixture):
        return sum(pr * mubar_of(d) for pr, d in zip(dist.probability, dist.distribution))
    raise TypeError(f"unhandled angular distribution {type(dist)}")

def level_mubar(nuc, mt, e_ev):
    """Interpolated <mu_cm> for reaction mt at incident energy e_ev [eV]."""
    rx = nuc.reactions[mt]
    ang = rx.products[0].distribution[0].angle
    if ang is None:
        return 0.0
    eg = np.asarray(ang.energy, float)
    mus = [mubar_of(d) for d in ang.mu]
    if e_ev <= eg[0]:
        return mus[0]
    if e_ev >= eg[-1]:
        return mus[-1]
    i = int(np.searchsorted(eg, e_ev) - 1)
    r = (e_ev - eg[i]) / (eg[i + 1] - eg[i])
    return (1.0 - r) * mus[i] + r * mus[i + 1]   # the EXPECTATION of OpenMC's draw

if __name__ == "__main__":
    ace, label = sys.argv[1], sys.argv[2]
    energies = [float(x) for x in sys.argv[3:]] or [1.0e6, 2.0e6, 5.0e6, 1.4e7]
    nuc = openmc.data.IncidentNeutron.from_ace(ace)
    print(f"# {label}: <mu_cm> per discrete inelastic level, from OpenMC's ACE")
    print("mt," + ",".join(f"{e:.6e}" for e in energies))
    for mt in range(51, 91):
        if mt not in nuc.reactions:
            continue
        vals = [level_mubar(nuc, mt, e) for e in energies]
        print(f"{mt}," + ",".join(f"{v:+.6f}" for v in vals))
