<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
Part of Outram Park (outram-park-backend), openfoam-appbuilder-lib.
-->

# Sod Shock Tube Validation — Rust `rhoCentralFoam` port vs Sod (1978) Table II

This directory holds the **validation** case for the Rust port of OpenFOAM
`rhoCentralFoam` (KNP central-upwind flux, 2nd-order vanLeer MUSCL). The port is
judged against the published benchmark profile in **Sod (1978) Table II**.

> The companion **tutorial** case
> (`tutorials/rho_central_foam_shock_tube.rs`) instead compares the Rust port
> against an OpenFOAM `rhoCentralFoam` reference run. This validation case
> compares the Rust port against Sod's published table.

- Methodology, exact-Riemann arbiter, and pass criteria: see `CLAUDE.md` in this
  directory and the module doc comment in `main.rs`.
- Numbers below were produced by the `rho_central_foam_matches_sod_table_ii` test
  (2026-07-06; 100 cells, `dt = 1e-6 s`, run to Sod's canonical τ = 0.2, i.e.
  `t = 6.3246e-3 s`). Regenerate with:

  ```bash
  cargo test -p openfoam-appbuilder-lib --test sod_shock_tube_validation \
    rho_central_foam_matches_sod_table_ii -- --nocapture
  ```

## Reference

Sod, G. A. (1978). *A Survey of Several Finite Difference Methods for Systems of
Nonlinear Hyperbolic Conservation Laws.* Journal of Computational Physics,
**27**(1), 1–31. DOI: [10.1016/0021-9991(78)90023-2](https://doi.org/10.1016/0021-9991(78)90023-2).

Open-access mirror (HAL): <https://hal.science/hal-01635155>.

Initial conditions (γ = 1.4), dimensionless: ρ_L = 1.0, P_L = 1.0, u_L = 0;
ρ_R = 0.125, P_R = 0.1, u_R = 0; diaphragm at x/L = 0.5; reference time τ = 0.2.

## Results — Rust port vs Table II (dimensionless, τ = 0.2)

Port values are the SI solution normalised by the scale factors (ρ₀ = 1 kg/m³,
u₀ = √(P₀/ρ₀) = 316.228 m/s, P₀ = 10⁵ Pa; `CLAUDE.md` §3.3), interpolated onto
the 9 Table II stations. The **exact** column is the analytic Riemann solution
(Toro ch. 4), used to flag whether each Table II station is *faithful* — i.e.
whether Table II's coarse 9-point sampling actually resolves the local profile.

| x/L | ρ port | ρ Table II | ρ exact | u port | u Table II | u exact | P port | P Table II | P exact | faithful |
|-----|--------|-----------|---------|--------|-----------|---------|--------|-----------|---------|----------|
| 0.1 | 0.9996 | 1.000 | 1.000 | 0.000 | 0.000 | 0.000 | 1.000 | 1.000 | 1.000 | ✅ |
| 0.2 | 0.9996 | 1.000 | 1.000 | 0.000 | 0.000 | 0.000 | 1.000 | 1.000 | 1.000 | ✅ |
| 0.3 | 0.8717 | 0.869 | 0.877 | 0.160 | 0.164 | 0.153 | 0.826 | 0.822 | 0.833 | ✅ |
| 0.4 | 0.6090 | 0.426 | 0.603 | 0.559 | 0.927 | 0.569 | 0.500 | 0.303 | 0.492 | ❌ fan |
| 0.5 | 0.4303 | 0.426 | 0.426 | 0.918 | 0.927 | 0.927 | 0.307 | 0.303 | 0.303 | ✅ |
| 0.6 | 0.4262 | 0.426 | 0.426 | 0.929 | 0.927 | 0.927 | 0.303 | 0.303 | 0.303 | ✅ |
| 0.7 | 0.2953 | 0.426 | 0.266 | 0.929 | 0.927 | 0.927 | 0.304 | 0.303 | 0.303 | ❌ contact |
| 0.8 | 0.2652 | 0.266 | 0.266 | 0.928 | 0.927 | 0.927 | 0.303 | 0.303 | 0.303 | ✅ |
| 0.9 | 0.1250 | 0.125 | 0.125 | 0.000 | 0.000 | 0.000 | 0.100 | 0.100 | 0.100 | ✅ |

### Same comparison in SI units (t = 6.3246×10⁻³ s)

| x [m] | ρ port [kg/m³] | ρ Table II | u port [m/s] | u Table II | P port [Pa] | P Table II |
|-------|----------------|-----------|--------------|-----------|-------------|-----------|
| −4.0 | 0.9996 | 1.0000 | 0.0 | 0.0 | 100000 | 100000 |
| −3.0 | 0.9996 | 1.0000 | 0.0 | 0.0 | 100000 | 100000 |
| −2.0 | 0.8717 | 0.8690 | 50.6 | 51.9 | 82561 | 82200 |
| −1.0 | 0.6090 | 0.4260 | 176.7 | 293.1 | 49972 | 30300 |
| 0.0 | 0.4303 | 0.4260 | 290.3 | 293.1 | 30727 | 30300 |
| 1.0 | 0.4262 | 0.4260 | 293.7 | 293.1 | 30266 | 30300 |
| 2.0 | 0.2953 | 0.4260 | 293.9 | 293.1 | 30352 | 30300 |
| 3.0 | 0.2652 | 0.2660 | 293.5 | 293.1 | 30321 | 30300 |
| 4.0 | 0.1250 | 0.1250 | 0.0 | 0.0 | 10000 | 10000 |

## Error summary

Worst relative error (normalised by field peak) over the **faithful** stations —
these are the ones asserted by the test:

| variable | worst faithful-point error | pass bound |
|----------|---------------------------|------------|
| pressure | 0.43 % | 5 % |
| velocity | 0.96 % | 5 % |
| density  | 0.43 % | 5 % |

Exact-Riemann arbiter self-check (`exact_riemann_reproduces_sod_star_state`):
P\* = 0.30313, u\* = 0.92745, ρ\*_L = 0.42632, ρ\*_R = 0.26557, shock speed =
1.75216 — all within < 10⁻⁴ of the analytic values.

## Why two stations are marked "not faithful"

Table II is a 9-point Glimm's random-choice solution and cannot resolve two
regions on this coarse grid:

- **x/L = 0.4** lies inside the **rarefaction fan** (which spans x/L ≈ 0.26–0.49
  at τ = 0.2). Table II reports the post-rarefaction star state (ρ = 0.426) there,
  but the true fan value is ρ = 0.603. The port gives ρ = 0.609 — it tracks the
  **exact** solution, not Table II.
- **x/L = 0.7** sits just past the **contact discontinuity** (at x/L ≈ 0.685).
  Table II still reads the left-star density (ρ = 0.426), while the exact
  right-star value is ρ = 0.266. The port gives ρ = 0.295 (one-cell contact
  smearing), again tracking the exact solution.

So at both unfaithful stations the discrepancy against Table II is Table II's own
9-point coarseness, not a defect in the port — which is exactly why the test uses
the analytic exact solution as an arbiter and asserts the port against Table II
only where Table II is a faithful sample.

## Interpretation

The Rust `rhoCentralFoam` port reproduces Sod's benchmark to **well under 1 %**
at every station Table II resolves cleanly, on a coarse 100-cell mesh — the
expected accuracy of a 2nd-order shock-capturing central scheme, with monotone
(non-oscillatory) captures of the rarefaction, contact, and shock.
