# PCICE semi-implicit finite-volume solver (gh:#1068)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic. The RELAP-7 Theory Manual §5.3 (Eqs
257–274; cited, not quoted, as the manual is not public) describes PCICE, the
pressure-corrected ICE method of Martineau and Berry. This is a clean-room
finite-volume implementation written from the open paper:

> R. C. Martineau and R. A. Berry, "The pressure-corrected ICE finite element
> method for compressible flows on unstructured meshes", *J. Comput. Phys.*
> 198 (2004) 659–685.

No RELAP source was read.

## What it does

`tampines::pcice::PcicePipe<E: HemEquationOfState>`, a 1-D cell-centred
finite-volume solver of the quasi-1-D compressible Euler equations on any
`HemEquationOfState` (ideal gas, IAPWS-IF97, CoolProp through `HemEos`). Per
step:

1. **Two-stage RK predictor** (Heun / SSP22) of mass, momentum and total
   energy with advective fluxes only (no pressure in the momentum flux, no
   pressure work in the energy flux). Faces are MUSCL-reconstructed (van Leer
   by default). The flux is central with Lax–Friedrichs dissipation at the
   **material** speed `max(|u_L|, |u_R|)`.
2. **Crank–Nicolson pressure target** `p^θ = ½(pⁿ + pⁿ⁺¹)`.
3. **Intermediate momentum and the corrected mass flux**
   `G_f = G*_f − ½Δt ∂_f p^θ`, using a compact face gradient (the Rhie–Chow-like
   coupling of a collocated grid). Momentum and total-energy fluxes are corrected
   with `G − G*`, and the pressure work `p^θ_f G/ρ_f` is added.
4. **Pressure-correction (Helmholtz) equation.** `pⁿ⁺¹ = p(ρ, ρE, ρu)` is
   linearised about the current iterate and solved as a tridiagonal system
   with `multiphase_1d::thomas_solve`. The EOS derivatives `(∂p/∂ρ)_e` and
   `(∂p/∂e)_ρ` are **forward finite differences of `state_rho_e`** (relative
   step 1e-6), because the trait has no derivatives. For an ideal gas at rest the
   operator reduces to the Crank–Nicolson wave operator `1 − (cΔt/2Δx)² Δ²`.
5. **Two correction passes.**
6. **Final conservative update** of mass, momentum and total energy with the
   final pressure. The new pressure comes from the EOS, so the carried state is
   thermodynamically consistent, and the residual left between the two is reported.

Time step: the material Courant number (`0.5` default), with an optional acoustic
cap. Boundaries: wall, fixed pressure, stagnation reservoir. Variable area
(quasi-1-D, with the wall pressure force) is implemented. It is verified only for
a duct at rest.

### Reused, searched first

- No PCICE existed in the workspace (grep `pcice`, `pressure-corrected`).
- **Reused:** `HemPipeGeometry` / `AreaProfile`, `HemInitialCondition`,
  `PrimitiveState`, `HemSnapshot` / `HemCellState`, `HemError`, the
  `HemEquationOfState` closures (`tampines_steam_tables::hem`);
  `outram_foam_basic_lib::limiters::FluxLimiter`; `multiphase_1d::thomas_solve`
  (tampines' own Thomas solver; `petir::linalg::tridiag` is symmetric-only);
  the Toro exact Riemann solver in
  `tampines-steam-tables/tests/common/toro_exact.rs`, which the test includes
  by `#[path]` rather than copying.
- **Re-written, cited:** the 1-D MUSCL face formula (`tampines_steam_tables`'
  `muscl_1d` is crate-private). It is the uniform-mesh form of
  `fvc::reconstruct_pos_neg`. The `PrimitiveState` closure (also private there).
- The PIMPLE/rhoPimple pressure equations (`outram-foam-appbuilder-lib`,
  `outram-foam-multiphase`) are 3-D, unstructured and enthalpy-based. They were
  read for structure and not reused.

### Reusable cases (for gh:#1069)

`tampines::pcice::cases`: `sod_ideal_gas(n)`, `low_mach_advection(n)` (with
`low_mach_advection_exact_density`), `water_shock_tube(n)` and
`nozzle_at_rest(n)`. Each is a `PciceCase` that builds a `PcicePipe`
(`build_pcice`) or an inviscid `HemPipe` on the same mesh (`build_hem_pipe`).

## Predictions (written before the first run, 2026-11-01)

1. **Sod** (100 cells, acoustic Courant 0.5): PCICE L1 `ρ` 1.5–3 %, `u` 2–5 %,
   `p` 1–3 % of peak. That is 1.2–3× worse than `HemPipe` (≈ 1 / 1.5 / 0.8 %)
   because the CN acoustics carry no upwind dissipation. Overshoot up to ~10 %
   of `u*`. L1 falls at 200 cells. Gate: `ρ ≤ 4`, `u ≤ 6`, `p ≤ 4 %`, and
   falling under refinement.
2. **Low Mach** (Mach 0.003, 100 cells, material Courant 0.5): 60 steps at
   acoustic Courant ≈ 175, stable. `p` and `u` uniform to round-off (gate
   1e-9). Density L1 1–4 % of the bump amplitude (gate 5 %), bump depth kept
   ≥ 90 %. Mass balance 1e-12. `HemPipe` takes ≈ 26 000 steps and
   gives 5–30 % L1.
3. **Water shock tube** (IF97, 440 K, 10 | 1 MPa): star pressure 5.4–5.6 MPa,
   within 1 % of `HemPipe`. L1 differences: `p` ≈ 1 % of the jump (gate 3 %),
   `u` 1–3 % of the plateau velocity (gate 5 %). Pressure residual ≤ 1e-6.
4. **Conservation**, two walls: mass and energy drift ≈ 1e-14 (gate 1e-12).
5. **Duct at rest** (2 : 1 : 2 area): `|u| < 1e-9 m/s`, `p` uniform to 1e-12.

## Results (2026-11-01, `tests/pcice.rs`, release)

| Case | Measured | Prediction |
|---|---|---|
| Sod, 100 cells, L1 % peak (ρ / u / p) | PCICE **0.694 / 1.332 / 0.560**; HemPipe 0.691 / 1.360 / 0.524 | gates met. **Missed favourably:** PCICE is level with HemPipe (within ±7 %), not 1.2–3× worse, and about half the predicted L1 |
| Sod, 200 cells | PCICE 0.370 / 0.668 / 0.282; HemPipe 0.359 / 0.681 / 0.261 | falls under refinement (first order, 1.9–2.0×): held |
| Sod overshoot `u_max/u*` | 1.059 (100), 1.063 (200) | ≤ 1.10: held |
| Sod, material Courant only (no acoustic cap), exploratory | **fails on step 1**: the fluid starts at rest, so Δt = max_dt = 1 ms (acoustic Courant ≈ 3.7 across the 10:1 jump), and the diaphragm cell's density goes to −3.5 kg/m³. The step is refused | not predicted |
| Low Mach steps / max acoustic Courant | 61 / **182.5** | held |
| Low Mach density L1 | **0.99 %** of amplitude (HemPipe 9.94 %, 26 564 steps) | held |
| Low Mach bump depth kept | 87.5 % | **missed** (≥ 90 %; ungated) |
| Low Mach max \|p/p0 − 1\| | 5.0e-11 | held |
| Low Mach max \|u − 1\| | **7.0e-9 m/s** | **MISSED** gate 1e-9 (see below) |
| Low Mach mass change − net inflow | 5.6e-16 relative | held |
| Water star pressure | 5.4889 (PCICE) vs 5.4685 MPa (HemPipe), 0.37 % | held |
| Water L1 difference | `p` 1.76 % of jump, `u` 3.30 % of plateau (3.46 m/s) | gates met. `p` above the ≈ 1 % prediction, `u` just above the 1–3 % prediction |
| Water pressure residual | 4.7e-12 | held |
| Conservation, Sod 742 steps | mass 1.1e-15, energy 0 | held |
| Conservation, water 582 steps | mass 4.4e-16, energy 1.8e-15 | held |
| Duct at rest, 100 steps | max \|u\| 9.1e-12 m/s, \|p/p0 − 1\| 2.5e-13 | held |

### The low-Mach velocity miss

The registered gate is not loosened. The test prints it as a MISS and does not
assert it. Diagnostics, added after the first run and printed by the test:

- **Not the pass residual.** With 1, 3 and 4 passes, `max|u − 1|` is 3.7e-9,
  8.5e-9 and 1.2e-8 m/s. The residual stays at 6–8e-12 every time.
- **The pressure noise is round-off that CN leaves undamped.** It is ≈ 3e-11
  relative everywhere. With `θ = 1` (backward Euler) it falls 17× to 2.9e-12,
  as expected of a neutral Crank–Nicolson acoustic mode.
- **The velocity drift is only partly explained.** With `θ = 1` it halves
  (3.0e-9 m/s), and it is concentrated on the bump's flanks (x ≈ 0.4–0.55 m,
  where the limiter switches). Its cause is **not identified**. At 7e-9
  relative it does not show in the density accuracy. It is an open finding.

### Interpretation

- At the acoustic Courant number that resolves a shock, PCICE is as accurate as
  the density-based KNP solver on Sod. This was not predicted. The material-speed
  dissipation plus the compact pressure coupling were enough.
- At low Mach number it does what it is for. It is stable at acoustic Courant
  182, takes 435× fewer steps than `HemPipe`, and is 10× more accurate on the
  advected contact, because its dissipation scales with `|u|`, not `|u| + c`.
- **Limit found:** a step set by the flow speed alone is unsafe when the flow
  starts from rest against a strong pressure jump (Sod, step 1). Use the
  acoustic cap or a small `max_dt` for such starts. Nothing is clamped: the
  step is refused.

## Limits

- Not here: friction, gravity, heat sources, choked breaks, junctions (all in
  `HemPipe`).
- Variable area is verified only at rest.
- Two-phase (flashing) water was not run. The forward-difference EOS
  derivatives straddle the saturation kink there, and that is untested.
- The predictor's energy correction and the reservoir boundary are first-order
  in the correction terms. The reservoir and fixed-pressure boundaries are
  exercised only by the low-Mach case.
