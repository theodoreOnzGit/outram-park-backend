# Predictions, written BEFORE any comparison run (gh:#1433, 2026-11-13)

Committed before `octave_driver.m` or the Rust test was run. Reference: GNU Octave
8.4.0 with `octave-control` **4.0.1** (Ubuntu apt). The port cites control 4.2.2+
commit f39e2625; `pkg install -forge control` was tried and failed (no route through
the proxy), so the reference is a *different, older* upstream version. Any
disagreement that traces to a 4.0.1 vs 4.2.2 change is reported as that, not as a
port defect.

What I read before predicting (source, not a run): `z_domain/{discrete_tf,conversion,
continuous_tf,polynomial}.rs`, and my recollection of liboctave `filter.cc`.

| # | claim | predicted outcome |
|---|---|---|
| P1 | `advance_one_sample` vs Octave builtin `filter(b,a,u)` (13 discrete systems, rand/step/impulse inputs, 40 samples) | NOT everywhere bit-identical. The port computes `(b*u + carry) - a*y`; liboctave computes `(carry - a*y) + b*u`. Different association, so a few ulp on cases with non-trivial coefficients. Bit-identical for the pure-integer d01 case. Worst relative difference (normwise) <= 1e-14 on the stable systems |
| P2 | localisation of P1 | an Octave-side loop replicating the port's order is bit-identical to the Rust port (all cases); an Octave-side loop replicating liboctave's order is bit-identical to the builtin `filter`. That proves operation order is the whole cause |
| P3 | a0 != 1 cases (d12, d13) | port multiplies by `1/a0`, Octave divides by `a0`: extra ulp-level difference on those two systems only |
| P4 | `lsim`, `step`, `impulse` (Octave goes tf -> state space -> recurrence) vs port | not bit-identical; normwise <= 1e-12 on stable systems, larger *relative-per-sample* near zero crossings (reported normwise, not per-sample). Unstable d10/d11 (poles to 1.15) compared normwise too |
| P5 | `c2d` Tustin, 1st/2nd order | within 1e-14 relative per coefficient; Octave goes via `ss` (SLICOT AB04MD), port is polynomial substitution |
| P6 | `c2d` ZOH, 1st/2nd order | within 1e-13 (Octave `expm` of the ss matrix vs closed form) |
| P7 | `c2d` prewarp | same as Tustin, 1e-14 |
| P8 | `c2d` matched | within 1e-14 |
| P9 | order > 2 with ZOH/matched (c08) | the port returns `UnsupportedOrder`, Octave succeeds: a counted **gap**, not a mismatch |
| P10 | integrator case c09 (`1/(s(1+2s))`) with ZOH | I do not know whether the port's closed form handles a pole at s = 0; flagged as the case most likely to reveal a defect |
| P11 | `dcgain` vs `steady_state_gain` | bit-identical (both `num[0]/den[0]`), `Inf` for c09 |
| P12 | `d2c` Tustin / prewarp / matched, orders 1-3 | within 1e-13 relative after normalising both sides by the leading denominator coefficient; Octave d2c ZOH is recorded but the port omits it (documented gap) |
| P13 | pole/zero, `bode`, `margin`, `nyquist` | no such API in the crate: not compared; gap rows |

Gap survey additions expected from a reading only: no `pole`/`zero`/`dcgain` for
discrete TFs, no `series`/`parallel`/`feedback` algebra, no MIMO, no `ss`, no
`foh`/`impulse` c2d, no `d2c` ZOH, no `lsim` on a vector in one call.

No bound above will be edited after the run. Any number outside these is reported
as observed, with its localisation.
