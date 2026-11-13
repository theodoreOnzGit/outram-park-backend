# Subchannel solver: crossflow and turbulent mixing (gh:#1078)

> **Research, education and V&V only. AI draft, unreviewed.**
> Date: 2026-11-01. Toolchain: Rust 1.99.1, `--release`.
> Code: `crates/tampines/src/subchannel/` (`mod.rs`, `solver.rs`, `tests.rs`).

## What is being verified

`tampines::subchannel` is a steady-state, single-phase, constant-property
(incompressible) subchannel solver. It follows the open COBRA-type formulation
(Todreas & Kazimi, *Nuclear Systems* Vol. I/II, the subchannel chapter; Rowe,
COBRA-IIIC, BNWL-1695, 1973). Neither book is in the kovan corpus, so section
and equation numbers are not re-checked. The parity target named by gh:#1078,
RELAP-7 Theory Manual §6.15 (Eqs 408–411), is private and was **not read**.

For each subchannel the solver balances mass, axial momentum and energy per
axial cell. Each gap carries a lateral-momentum equation for the diversion
crossflow `w`, driven by the pressure difference, with gap loss coefficient
`K` and axial convection of lateral momentum. Turbulent mixing is
`w' = β s Ḡ`. It carries energy, and momentum through the factor `f_T`
(default 1). It is **on by default**, and `without_turbulent_mixing()` ablates
it. Wall friction is the RELAP5 four-region closure,
`tampines::closures::wall_friction`. The whole system is solved by Newton:
a finite-difference Jacobian coloured on its block band, PETIR's sparse LU,
and backtracking.

Gaps that are named and not modelled: no two-phase flow, no
temperature-dependent density (so no buoyancy or thermal acceleration), no
spacer-grid losses, no rod conduction or wall temperature.

## Common inputs

| Quantity | Value | Source |
|---|---|---|
| Subchannel | PWR square lattice interior, pitch 12.6 mm, rod 9.5 mm | geometry |
| `A` | `p² − πd²/4 = 8.788e-5 m²` | derived |
| `D_h` | `4A/(πd) = 11.78 mm` | derived |
| gap `s`, `l` | 3.1 mm, 12.6 mm | `p − d`, `p` |
| `ρ, c_p, μ` | 997 kg/m³, 4180 J/(kg K), 8.9e-4 Pa s | round textbook water near 300 K |
| `G_0`, `ṁ_0` | 3500 kg/(m² s), 0.3076 kg/s (`u_0 = 3.51 m/s`, `Re = 46317`) | chosen |
| `β` | 0.02 (`TYPICAL_MIXING_COEFFICIENT_BETA`) | mid-range of the ~0.005–0.05 literature span; not re-checked |
| `K` | 0.5 | order-of-magnitude choice. It does not enter at first order in test 2 |
| outlet | 1e5 Pa, uniform | chosen |

No input was tuned against any result.

## Results

Each prediction was written in the test's doc comment before the first run.

| # | Test | Prediction | Measured | Verdict |
|---|---|---|---|---|
| 1 | Two identical channels, equal inlet, 20 kW/m each, `L = 2 m`, 50 cells | `w = 0`; `ΔP` and `T_out` match single-channel hand values to 1e-12 | `ΔP = 41745.078675 Pa` vs hand 41745.078675 (−3.5e-16); `ΔT = 31.112607588 K` (2.3e-15); `max\|w\|/w_ref = 5.2e-14` | held |
| 2 | Unequal inlet `ṁ_0(1 ± 0.02)`, unheated, `L = 3 m`, 300 cells | mass to 1e-12; slow decay rate within 3 % of the linear-analysis root; profile within 3 %; outlet split < 0.02 | **first prediction missed** (below); after correction: rate +0.11 % (`f_T = 1`), +0.35 % (`f_T = 0`); profile −1.0 … −2.6 %; mass 1.8e-16 | held after a corrected analysis |
| 3 | Mixing only: channel 0 heated 20 kW/m over the first 2 m of 4 m, 200 cells | decay rate within 1e-4 of `λ = 2βs/A`; `ΔT(2 m)` within 1e-4 | `λ = 1.41105009`, measured 1.41114375 (+6.64e-5, the predicted `(λΔz)²/12`); `ΔT(2 m)` +1.2e-5; `w = 8e-22` | held |
| 3a | Ablation: `without_turbulent_mixing()` | heated rise `q'(L/2)/(ṁc_p)` to 1e-12; the unheated channel unchanged | as predicted (assertions pass) | held |
| 4 | Three channels, flows 1.1/1.0/0.85 `ṁ_0`, inlet 300/305/295 K, 25/15/5 kW/m, rough walls, `f_T = 0.8` | energy and mass to 1e-11 | energy 0.0 (exact in `f64` here), mass 1.2e-16, `max\|w\| = 0.0856 kg/(m s)`; outlet flows 0.9847/0.9834/0.9819 `ṁ_0` | held |

Newton converges in 2–4 iterations to a scaled residual ≤ 7.3e-16 in every run.

### Test 2 in detail, including the miss

Let `δ = (ṁ_1 − ṁ_2)/2`. Linearise the model's continuum equations about
`ṁ_0`: take `u* ≈ u_0`, note that `K w|w|` is second order, and let
`c = dF/dṁ` be the slope of the friction pressure gradient. The result is

```text
u_0 δ''' − (2 u_0 s/(l A)) δ' − (2 c_eff s/l) δ = 0,
δ(0) = ε ṁ_0,   δ'(0) = 0,   δ''(L) = 0   (uniform outlet pressure).
```

This has three real modes: fast inlet and outlet modes at `|r| ≈ 75 m⁻¹`, and
a slow redistribution mode `r_slow ≈ −c_eff A/u_0`. In the slow mode,
acceleration balances the friction difference at nearly equal pressure. The
reference is the exact three-mode solution fitted to the three boundary
conditions.

**The miss.** The first analysis took `c_eff = c = 6.40e4 Pa m⁻¹ (kg/s)⁻¹`.
It treated all mixing terms as second order. The run measured
`r = −3.023 m⁻¹` against the predicted `−1.604 m⁻¹` (+88 %), and the profile
missed by 30–94 %. The cause was in the analysis, not the code: `w' = β s Ḡ`
is O(1), so the momentum exchange `f_T w' (u_1 − u_2)` is **first order** in
`δ`. It adds `2 f_T w'/(ρA²) = 5.64e4` to `c`. No model input and no
tolerance was changed. The corrected analysis was made falsifiable: with
`f_T = 0` the rate must return to the original root.

| `f_T` | `c_eff` | `r_slow` (m⁻¹) | measured | rel | `δ/δ_0` at z = 0.25, 0.5, 1, 1.5, 2 m (analytic) | outlet split (analytic) |
|---|---|---|---|---|---|---|
| 1 | 1.2041e5 | −3.0192 | −3.0225 | +1.1e-3 | 0.4823 (0.4903), 0.2256 (0.2305), 0.04966 (0.05094), 0.01097 (0.01126), 0.002422 (0.002488) | 1.18e-4 (1.21e-4) |
| 0 | 6.4046e4 | −1.6040 | −1.6095 | +3.5e-3 | 0.6779 (0.6845), 0.4519 (0.4584), 0.2016 (0.2056), 0.09021 (0.09218), 0.04041 (0.04134) | 8.12e-3 (8.31e-3) |

**How to read it.** The decay rate agrees to 0.1–0.35 %. The profile lags the
linear solution by a near-constant 1–2.6 %. That offset is set in the inlet
region, where `w` peaks and the neglected second-order terms (donor `u*`,
`K w|w|`) are largest. It sits inside the 3 % bound but uses most of it.
Physically, crossflow equalises a 2 % flow imbalance within about
`1/|r_slow| ≈ 0.33 m` with default mixing, and 0.62 m without the momentum
part of mixing.

## Which numbers to quote

- **Conservation (tests 2 and 4).** Quote these as round-off: ≤ 1.8e-16
  relative. The discrete balances are conservative and the exchange terms
  are antisymmetric.
- **Physics checks.** Test 1 checks single-channel `ΔP` and energy to
  round-off. Test 3 checks the mixing decay rate to the predicted
  discretisation error, 6.6e-5. Test 2 checks the redistribution rate to
  0.1–0.35 % against the **corrected** linear analysis. The first test 2
  prediction (+88 % miss) is recorded above and must not be quoted as a
  result.

## How to run

```bash
cargo test --release -p tampines --lib subchannel -- --nocapture
```
