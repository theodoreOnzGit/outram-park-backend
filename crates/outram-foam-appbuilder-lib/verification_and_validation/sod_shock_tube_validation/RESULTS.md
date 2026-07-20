<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
Part of Outram Park (outram-park-backend), outram-foam-appbuilder-lib.
-->

# Sod shock tube — RhoCentralFoam port vs the exact Riemann solution

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-07-17 (Asia/Singapore) — from the
`rho_central_foam_matches_sod_table_ii` test (release build).
**Crate / commit:** `outram-foam-appbuilder-lib` on `develop` (base `71b63e5`).

This record accompanies the plottable per-cell dataset
[`sod_shock_tube_profile_vs_exact_riemann.csv`](./sod_shock_tube_profile_vs_exact_riemann.csv)
in this folder. That CSV is **regenerated on every** `cargo test --release`
run of this crate's Sod test and is the file a reader plots directly. The
companion `verification_and_validation/README.md` and the parent V&V write-up
(`tests/sod_shock_tube_validation/README.md`) cover the Sod (1978) Table II
comparison; this file focuses on the whole-field comparison against the
analytic exact Riemann solution.

## Methodology

- **Solver under test:** the Rust port of OpenFOAM `rhoCentralFoam`
  (Kurganov–Tadmor central-upwind KNP flux, 2nd-order vanLeer MUSCL
  reconstruction), driven from `RhoCentralFoam` in this crate.
- **Case:** the committed 100-cell 1-D polyMesh, $x \in [-5, 5]$ m,
  $\Delta x = 0.1$ m, diaphragm at $x = 0$. Initial conditions (SI, air,
  $\gamma = 1.4$, $R = 287.1$ J/(kg·K)):
  $\rho_L = 1.0$ kg/m³, $p_L = 10^5$ Pa, $u_L = 0$;
  $\rho_R = 0.125$ kg/m³, $p_R = 10^4$ Pa, $u_R = 0$.
- **Time integration:** fixed $\Delta t = 10^{-6}$ s, run to Sod's canonical
  $\tau = 0.2$, i.e. $t = 0.2\,t_0 = 6.32456 \times 10^{-3}$ s
  (with $t_0 = L_0/u_0$, $L_0 = 10$ m, $u_0 = \sqrt{p_0/\rho_0} = 316.228$ m/s).
- **Reference:** the analytic exact Riemann solution (Toro 2013, ch. 4),
  computed on the fly by the `star_pressure` / `star_velocity` / `sample`
  functions in `main.rs` and self-checked in
  `exact_riemann_reproduces_sod_star_state` to $< 10^{-4}$ of the published
  Sod star state ($p^* = 0.30313$, $u^* = 0.92745$, $\rho^*_L = 0.42632$,
  $\rho^*_R = 0.26557$, shock speed $= 1.75216$, dimensionless).
- **Comparison:** the exact solution is evaluated at **every cell centre**
  (100 points), directly against the numerical profile — no interpolation onto
  the coarse 9-station table. Error norms are the discrete
  $L_2$ (root-mean-square over cells) and $L_\infty$ (max-absolute over cells),
  reported in SI units and normalised by the field peak
  ($\rho_{\text{peak}} = 1.0$ kg/m³, $u_{\text{peak}} = |u^*|\,u_0 = 293.3$ m/s,
  $p_{\text{peak}} = 10^5$ Pa).

## Reference

```bibtex
@book{toro2013riemann,
  title     = {Riemann solvers and numerical methods for fluid dynamics: a practical introduction},
  author    = {Toro, Eleuterio F.},
  edition   = {3rd},
  year      = {2013},
  publisher = {Springer Science \& Business Media}
}
@article{sod1978survey,
  title   = {A survey of several finite difference methods for systems of nonlinear hyperbolic conservation laws},
  author  = {Sod, Gary A.},
  journal = {Journal of Computational Physics},
  volume  = {27}, number = {1}, pages = {1--31}, year = {1978},
  doi     = {10.1016/0021-9991(78)90023-2}
}
```

## Results (measured 2026-07-17; 100 cells, $\Delta t = 10^{-6}$ s, vanLeer MUSCL)

Whole-field error of the port vs the exact Riemann solution, over all 100 cell
centres at $\tau = 0.2$:

| variable | $L_2$ (SI) | $L_2$ (rel. peak) | $L_\infty$ (SI) | $L_\infty$ (rel. peak) |
|---|---|---|---|---|
| density $\rho$  | $1.411\times10^{-2}$ kg/m³ | 1.41 % | $8.609\times10^{-2}$ kg/m³ | 8.61 % |
| velocity $u$    | $1.516\times10^{1}$ m/s    | 5.17 % | $1.409\times10^{2}$ m/s    | 48.04 % |
| pressure $p$    | $1.056\times10^{3}$ Pa     | 1.06 % | $7.448\times10^{3}$ Pa     | 7.45 % |

For context, the companion Table II comparison (asserted, faithful stations
only) gives worst relative errors of $p = 0.43\%$, $u = 0.96\%$,
$\rho = 0.43\%$ — see `tests/sod_shock_tube_validation/README.md`.

### Interpretation

- The **$L_2$ norms are small** (1–5 % of peak), the expected accuracy of a
  2nd-order shock-capturing central scheme on a coarse 100-cell mesh across a
  field that contains a rarefaction fan, a contact discontinuity, and a shock.
- The **$L_\infty$ norms are dominated by the one or two cells that straddle a
  discontinuity**, where the analytic solution jumps sharply and the numerical
  scheme necessarily smears it over ~1–2 cells. Concretely: the $L_\infty$
  velocity error (48 %) is the single cell at $x \approx 3.55$ m sitting on the
  **shock** (numerical $u \approx 141$ m/s while the exact right state is
  $u = 0$); the $L_\infty$ density error is the same shock plus the **contact**
  near $x \approx 1.85$ m. These are point values on a discontinuity, not a
  field-wide error — the profile is monotone (non-oscillatory) through each
  wave, which is the correct behaviour for this KT/vanLeer scheme.
- **Honest V&V finding:** the port reproduces the smooth regions and constant
  states to ~1 % and captures the wave *positions* correctly, but a 100-cell
  mesh cannot resolve the discontinuities to better than one cell — so the
  $L_\infty$ figures are inherently large and should be read as "worst single
  cell on a jump", not as a defect. A mesh-refinement study (not run here)
  would show $L_2$ converging and $L_\infty$ staying $O(1)$ at the jumps, the
  classic behaviour of a shock-capturing scheme.

## Data provenance

The reference is the closed-form Sod/Riemann solution (public literature:
Sod 1978; Toro 2013), computed inside the test — no external dataset. The CSV
is a machine-generated artefact of running the committed test on the committed
mesh and initial fields; regenerate with:

```bash
cargo test -p outram-foam-appbuilder-lib --release \
  --test sod_shock_tube_validation rho_central_foam_matches_sod_table_ii -- --nocapture
```
