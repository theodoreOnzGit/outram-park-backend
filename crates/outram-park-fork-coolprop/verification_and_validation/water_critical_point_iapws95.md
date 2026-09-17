# Water critical-point pressure vs IAPWS-95's defining value

**Generated:** 2026-07-10T00:00:00Z (date of the `accumulate_non_analytic`
implementation and its verification test; exact time not logged at the time
of writing — see the git commit timestamp for `tests/non_analytic_critical_region.rs`
for a precise value going forward)
**Crate version / commit:** `outram-park-fork-coolprop` 0.0.1, workspace commit `fd4b8e8`

## Methodology

IAPWS-95 is *fit* so that Water's critical point is reproduced exactly: at
`T = T_c = 647.096 K`, `ρ = ρ_c = 322.0 kg/m³`, the equation of state must give
`p = p_c = 22 064 000 Pa` to the limits of double precision. This is a direct
code-vs-defining-value check of the residual Helmholtz EOS's non-analytic
critical-region term (`eos::ResidualTerm::NonAnalytic`, evaluated by
`eos::accumulate_non_analytic`) — before that term was implemented, this
exact point carried a ~1e-4 relative residual (see the crate's git history and
`tests/water_reference.rs`).

`T_c`, `ρ_c` and `p_c` are also exactly the branch point (`δ=1, τ=1`) the
non-analytic term's `θ`/`Δ`/`ψ` formulas must be offset away from (several
factors are formally `0/0` there) — so this is also a direct test of that
numerical guard.

Pass criterion: relative error `< 1e-9` (chosen well above machine epsilon's
floor, since IEEE 754 double-precision arithmetic through a ~50-term
Helmholtz sum accumulates some rounding; the actual result is far tighter, see
below).

## Reference

```bibtex
@article{wagner2002iapws,
  author  = {Wagner, W. and Pru{\ss}, A.},
  title   = {The {IAPWS} Formulation 1995 for the Thermodynamic Properties of
             Ordinary Water Substance for General and Scientific Use},
  journal = {Journal of Physical and Chemical Reference Data},
  volume  = {31},
  number  = {2},
  pages   = {387--535},
  year    = {2002},
  doi     = {10.1063/1.1461829},
  note    = {Critical parameters T\_c = 647.096 K, rho\_c = 322.0 kg/m^3,
             p\_c = 22.064 MPa: Table 13.1, p. 429. The EOS is fit to
             reproduce these exactly at the critical point by construction
             (Sec. 6.3, p. 407).}
}
```

## Results

```csv
quantity,computed,reference,units,rel_error
p_critical,22064000.00000115,22064000,Pa,5.2e-14
```

Prose interpretation: the computed critical pressure agrees with IAPWS-95's
defining value to `5.2e-14` relative error — essentially machine precision for
a computation chaining ~50 residual-Helmholtz terms plus the non-analytic
term's θ/Δ/ψ evaluation. This confirms both (a) `accumulate_non_analytic` is
implemented correctly (the term was a documented no-op before this work,
carrying a ~1e-4 residual at the critical point) and (b) the `δ=1`/`τ=1`
branch-point offset guard does not introduce a detectable error at the exact
point it exists to protect.

A companion check one degree below `T_c` on the critical isochore
(`T = 646.096 K`, same `ρ_c`) is not a *defining* IAPWS-95 value (no
independently-published reference number to check against at exactly that
point), so it's a monotonicity/sanity check only, not part of this V&V
record — see `tests/non_analytic_critical_region.rs::water_pressure_just_below_critical_isochore_is_sane`.

**Reproduce:** `cargo test --release -p outram-park-fork-coolprop --test non_analytic_critical_region -- --nocapture`
