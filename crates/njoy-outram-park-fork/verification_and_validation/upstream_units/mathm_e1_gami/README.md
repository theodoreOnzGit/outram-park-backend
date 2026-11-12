# `e1` and `gami` (NJOY2016 `mathm.f90`) against petir, gh:#1366

Research, education and V&V only. Upstream NJOY2016 (modified BSD-3, LANL); `mathm.f90` is identical in
2016.79 and 2016.80 (SHA-256 in `run.sh`), compiled unmodified, gfortran 13.3.0 -O2.

**Methodology.** `driver.f90` prints hex bits of `e1(u)` and `gami(a,u)` (a = 0.5, 1.5, 2.5) at 779 abscissae
u = 1e-6 * 10^(k/100), k = 0..769, plus 1, 4 (branch edges) and neighbours, 0.5, 25, 50. Port side:
`petir::expint_e1`, `petir::gamma_inc_lower` (the functions `acer/energy/madland_nix.rs` calls, a = 1.5 only).
Test: `tests/mathm_e1_gami_vs_njoy2016_units.rs`.

**Prediction (before running):** <= 1e-14 relative, not bit-identical (GSL against SLATEC).

**Results.**

| function | bit-identical | worst relative | at u |
|---|---|---|---|
| `e1` | 686/779 | 4.16e-16 | 0.347 |
| `gami(0.5, u)` | 25/779 | 1.77e-15 | 2.4e-5 |
| `gami(1.5, u)` (used by ACER) | 75/779 | 3.75e-15 | 2.45e-5 |
| `gami(2.5, u)` | 37/779 | **1.06e-14** | 1.45e-6 |

`e1` and `gami(1.5)` meet the prediction. `gami(2.5)` misses 1e-14 by 6 %; the port does not use that `a`. Probable
cause (untested): the port forms `Gamma(a)*P(a,u)` with `exp(lngamma(a))`, which SLATEC does with `exp(dlngam(a)+a*log(x))`
and Tricomi's `dgamit`. The tests gate `e1` at 1e-15 and `gami(1.5)` at 1e-14; the other two `a` are pinned at the measured values.
