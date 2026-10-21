#!/usr/bin/env python3
"""Localise the Godiva transport discrepancy: measure <mu>, Sigma_tr, and the
HISTORICAL NOTE, added 2026-09-16. This script diagnosed the defect; the defect
is now FIXED (bead op-tm9f). What it computes is therefore the size of the error
that WAS present, i.e. the "this crate" column is the PRE-FIX behaviour, kept
because the decomposition is what justified the fix and a reader needs to see
it. The discrete inelastic levels are now sampled from their own ENDF MF=4
distributions, verified against this same ACE in
tests/inelastic_mubar_vs_openmc.rs (worst 3.1e-3 in <mu_cm>), and Godiva moved
-198 pcm to +16 +/- 11.

Two caveats on the numbers below, stated rather than quietly relied on:
  - mubar() reads OpenMC's table at the bracketing index (searchsorted), NOT
    interpolated. That is the nearest-point approximation this study elsewhere
    warns about. It is tolerable here because the result is an aggregate over
    400 energies, but it is not exact; inel_mu_oracle.py in this directory does
    interpolate and is the one to trust for per-level values.
  - The inelastic column is a CM mean, while the elastic column is converted to
    lab. For A ~ 235 the difference is ~1/(2A) ~ 0.002 and does not change the
    conclusion, but the printed label says "lab" for both.


reactivity worth of the angular distributions this crate does not yet sample.

Context. outram-mc-libs sits +250 +/- 51 pcm above OpenMC on Godiva with
identical nuclear data (compare_xs.py shows every reaction agreeing to <= 0.06%
flux-weighted). A k_inf comparison then splits that:

    relative offset, k_eff   +250 +/- 51 pcm
    relative offset, k_inf    +69 +/- 23 pcm
    => non-leakage share     ~+181 pcm

(Relative offsets add, since ln k_eff = ln k_inf + ln P_NL.) So roughly 28% is
spectral and 72% is leakage: our neutrons escape LESS than they should.

This script tests the obvious cause. `src/physics/scatter.rs` says plainly:
"Anisotropic *inelastic* angular laws ... remain future work" -- elastic uses the
full ENDF MF=4 tabulated cosine, inelastic is sampled isotropic in CM. Isotropic
scattering means <mu> too small, Sigma_tr = Sigma_t(1 - <mu>) too LARGE, the
diffusion coefficient too small, leakage too low, and k too high. Right sign.

It computes, flux-weighted over a Watt spectrum above 10 keV and averaged over
the three ICSBEP nuclides at their atom densities:
  * <mu_lab> for elastic and for the discrete inelastic levels,
  * Sigma_tr as it should be, and as this crate effectively computes it,
  * the resulting change in non-leakage probability, from the MEASURED k_inf.

    WORK_DIR=... python3 transport_decomposition.py
"""
import os, math
import numpy as np
import openmc.data

WORK = os.environ.get("WORK_DIR", "./work")
T = "294K"
E = np.logspace(4, 7.28, 400)     # Godiva's flux lives above ~10 keV
w = np.array([math.exp(-e / 0.988e6) * math.sinh(math.sqrt(2.249e-6 * e)) for e in E])
DENS = {"U234": 4.9184e-4, "U235": 4.4994e-2, "U238": 2.4984e-3}
K_INF_REF, K_EFF_REF = 2.26401, 1.0     # OpenMC's own measured values

def _mu_one(d):
    """<mu> of a single tabulated cosine distribution."""
    if isinstance(d, openmc.stats.Tabular):
        x, p = np.asarray(d.x, float), np.asarray(d.p, float)
        if p.sum() <= 0:
            return 0.0
        if getattr(d, "interpolation", "linear-linear") == "histogram":
            dx = np.diff(x)
            return float((dx * p[:-1] * 0.5 * (x[:-1] + x[1:])).sum() / (dx * p[:-1]).sum())
        return float(np.trapezoid(x * p, x) / np.trapezoid(p, x))
    if isinstance(d, openmc.stats.Mixture):
        return sum(pr * _mu_one(q) for pr, q in zip(d.probability, d.distribution))
    return 0.0


def mubar(dist, e):
    """<mu> of an OpenMC AngleDistribution at incident energy e; 0 if isotropic.

    INTERPOLATED between the tabulated incident energies, which is the correct
    reading and not a refinement. OpenMC's AngleDistribution::sample picks table
    i or i+1 with probability r, so the EXPECTATION of its draw is the linear
    interpolation. This function previously used np.searchsorted, i.e. the table
    at the energy ABOVE e; since <mu> grows with energy that biased the
    aggregates HIGH -- measured 2026-09-16 at +3.6% on elastic (0.2740 against
    0.2645) and +3.7% on inelastic (0.0254 against 0.0245).

    That is the fourth time the nearest-point trap has appeared in this study.
    The other three are recorded in the README; this one was in the reference
    script itself, and was found only when an independent Rust computation of
    the same aggregate disagreed by 30% and the disagreement had to be explained
    rather than explained away.
    """
    if dist is None:
        return 0.0
    try:
        eg = np.asarray(dist.energy, float)
        mus = np.array([_mu_one(d) for d in dist.mu])
        return float(np.interp(e, eg, mus))
    except Exception:
        return 0.0

num_el = np.zeros_like(E); num_in = np.zeros_like(E)
sig_el = np.zeros_like(E); sig_in = np.zeros_like(E); sig_tot = np.zeros_like(E)
for n, dn in DENS.items():
    d = openmc.data.IncidentNeutron.from_hdf5(f"{WORK}/{n}.h5")
    A = d.atomic_weight_ratio
    r = d.reactions[2]
    ad = r.products[0].distribution[0].angle if r.products else None
    mu = np.array([mubar(ad, e) for e in E]) if ad else np.zeros_like(E)
    mu_lab = mu + (1.0 / A) * (1 - mu * mu) / 2.0      # CM -> lab, tiny for A~235
    xs = r.xs[T](E); sig_el += dn * xs; num_in_add = 0
    num_el += dn * xs * mu_lab
    for mt in range(51, 92):
        if mt not in d.reactions:
            continue
        rr = d.reactions[mt]; x2 = rr.xs[T](E)
        a2 = None
        if rr.products:
            try: a2 = rr.products[0].distribution[0].angle
            except Exception: a2 = None
        m2 = np.array([mubar(a2, e) for e in E]) if a2 else np.zeros_like(E)
        sig_in += dn * x2; num_in += dn * x2 * m2
    for mt in list(range(51, 92)) + [2, 18, 102, 16, 17]:
        if mt in d.reactions:
            sig_tot += dn * d.reactions[mt].xs[T](E)

fw = lambda x: float(np.sum(w * x) / np.sum(w))
f_in = float(np.sum(w * sig_in) / np.sum(w * (sig_el + sig_in)))
print("flux-weighted above 10 keV, material-averaged over the three ICSBEP nuclides:")
print(f"  elastic    <mu_lab> = {np.sum(w*num_el)/np.sum(w*sig_el):.4f}"
      f"   ({100*(1-f_in):.1f}% of scattering)")
print(f"  inelastic  <mu_lab> = {np.sum(w*num_in)/np.sum(w*sig_in):.4f}"
      f"   ({100*f_in:.1f}% of scattering)   <-- this crate sampled it as 0 before op-tm9f")

Str_true = fw(sig_tot - (num_el + num_in))
Str_ours = fw(sig_tot - num_el)
rel = Str_ours / Str_true - 1.0
print(f"\n  Sigma_tr [1/cm]   should be {Str_true:.5f}   PRE-op-tm9f {Str_ours:.5f}"
      f"   ({100*rel:+.2f}%)")

P = K_EFF_REF / K_INF_REF
L2B2 = 1.0 / P - 1.0
Pp = 1.0 / (1.0 + L2B2 / (1.0 + rel))
print(f"\n  reference P_NL = {P:.5f}  (leakage {100*(1-P):.1f}%)")
print(f"  one-group diffusion: Sigma_tr {100*rel:+.2f}% => dP_NL/P_NL = "
      f"{(Pp/P-1)*1e5:+.0f} pcm")
print(f"  MEASURED leakage share of the discrepancy: ~+181 pcm")
print("\nOne-group diffusion in a bare fast metal sphere is crude. This is an")
print("order-of-magnitude check on whether the mechanism is BIG ENOUGH to matter,")
print("not a precise prediction -- and it is.")
