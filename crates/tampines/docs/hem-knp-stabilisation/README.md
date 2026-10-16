# Stable HEM two-phase pipe solver on petir numerics: plan (gh:#896)

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation, reactor control, licensing, or safety-critical decisions. See `RESPONSIBLE_USE.md`.

**Date:** 2026-10-16. **Status: a plan, awaiting maintainer decisions D1–D4.** No solver code has been changed.

**Method.** A planning fleet of three slices. Each slice's full report sits here unedited:

| File | Slice |
|---|---|
| [`01-instability-diagnosis.md`](01-instability-diagnosis.md) | Why Edwards fails. Upstream OpenFOAM-10 `rhoCentralFoam` / `rhoPimpleFoam` (commit `89f925dc`) was read for this |
| [`02-petir-coverage-audit.md`](02-petir-coverage-audit.md) | Which platform `libm` calls remain, what petir lacks, and a cross-platform determinism test |
| [`03-api-and-crate-placement.md`](03-api-and-crate-placement.md) | Dependency graph, the three KNP copies, the proposed `tampines` API, and the test plan |
| [`runs/edwards_pimple_vanleer_30us_2026-10-16.log`](runs/edwards_pimple_vanleer_30us_2026-10-16.log) | The observed failure |

## What the diagnosis changed about the problem

**The instability is not in KNP.**

- `TampinesSteamArray` is a pressure-based `rhoPimpleFoam` port. KNP enters
  only as the opt-in deferred-correction dissipation `SolverMode::HybridAllMach`.
- The reproduced failure is in the **default Pimple mode, with KNP off**:
  `EDW_HE_SCHEME=vanleer`, 30 µs, release. Per gh:#406 the hybrid fails the
  same way.

The failure sequence, from the log:

| t | Event |
|---|---|
| 0.066 s | The break cell flashes |
| | Cell 22's pressure solve goes negative (−18.8 kPa while still liquid) and is clamped |
| | Its explicit continuity is asked to remove **5× the mass the cell holds** in one step (ρ_old = 1.36 → −5.47 kg/m³) |
| | The break pressure rises to **8.4 MPa, above the 7 MPa initial pressure** |
| 0.123 s | The `(p,h)` guard panics at `ph_flash_eqm/mod.rs:901` on a state far outside the domain |

The panic is the guard firing on a state that is already non-physical. It is
not gh:#342's 17.7 mK band, which `boundary_273_15_repro` still reproduces.

**What upstream does that the port does not**, from reading OpenFOAM-10:

1. **Adaptive Δt from a Courant number** (`compressibleCourantNo.H`,
   `setDeltaT.H`). The port uses a fixed Δt and computes no Courant number
   anywhere.
2. **Implicit energy convection** (`fvm::div(phi, he)` in `EEqn.H`). The port is
   explicit, and an explicit update stops being a convex combination once a
   cell's outflow exceeds its content.
3. **The kinetic-energy terms** `fvc::ddt(rho,K) + fvc::div(phi,K)`. These are
   missing, and the omissions register has no row for them.
4. **`thermo.correct()` once per outer corrector,** with a linear ρ update inside
   the pressure equation. That is the port's non-default
   `PsiRefresh::OncePerOuterCorrector`, which the port had already measured as
   *more* accurate.
5. **The `transonic` pressure-equation branch.** The port lacks it.

The KNP hybrid itself also differs from upstream. It adds its flux after the
pressure equation has converged, with no wave-speed Courant check. It applies
no energy dissipation and skips boundary faces.

## Proposed sequence

### Phase A: make the existing solver stable, testing each hypothesis with its prediction

**A1. Courant number and adaptive Δt**, ported from upstream `setDeltaT.H`.
Under `Fixed` Δt, an outflow Courant number above 1 returns a named error.

- *Prediction, stated before any run:* the 0–0.06 s plateau is essentially
  unchanged (Co ≈ 0.003 there). The drained-cell holds and the 8.4 MPa spike
  disappear.
- *Untested:* whether A1 alone removes the 0.123 s panic.

**A2. Implicit upwind energy convection by default.** This is **reuse**:
`outram-park-fork-coolprop` already has `EnergyBalanceMode::Implicit`. Also
register the missing kinetic-energy (K) terms.

- *Prediction:* the van Leer `he` blow-up disappears. The diagonal stays
  positive even when the continuity density goes negative.

**A3. `correct_thermo` uses the checked `try_*` flash** and returns `Err`. This
replaces a panic with an error; it adds no stability.

**A4. Ablation.** Run A1 and A2 together, then each alone. Count
`pressure_bound_events` and `drained_hold_events`. **Re-measure every Edwards
number** (current pins: plateau 359.0 psia, GS-1 RMSE 42.8 psia, Linear scheme)
and record every move, including unflattering ones.

### Phase B: one KNP kernel, and a HEM solver exported from `tampines`

**B1. One EOS-free KNP flux kernel** in
`outram_foam_basic_lib::fv_operators::fvc::central_upwind`. Every crate in the
chain already depends on basic-lib, so this adds no dependency edge.

- The steam-tables and CoolProp `face_flux_impl` are **code-identical**.
  Appbuilder's `RhoCentralFoam` is the same formula, written for total energy.
- Re-point all three copies, using bit-identical gates (M3, M4).

**B2. Public equilibrium sound speeds.**
`TampinesSteamTableCV::get_speed_of_sound` returns the **frozen Wood–Wallis**
speed. The equilibrium HEM speeds are `pub(crate)`, so a wrapper written today
would silently use the wrong physics.

**B3. `HemEquationOfState { Iapws97, CoolProp(..), IdealGas{..} }`** in
`tampines`, with a `(ρ,e) → p` inversion. That inversion is one `petir::roots`
Brent solve over each backend's `(p,h)` flash. No `(ρ,e)` flash exists anywhere
today.

**B4. `tampines::multiphase_1d::hem::HemPipe`**, re-exported as
`tampines::hem::HemPipe`. Its API is sketched in 03 §3.

- Its building blocks are reused: `Pipe1d`, `TampinesFluid`, `TampinesError`
  and the existing `critical_flow` dispatcher.
- It is uom at the boundary, with adaptive Courant by default.
- It returns errors, never clamps.

**B5. Tests in `tampines`:**

- an ideal-gas Sod run against appbuilder and against Toro's exact solution;
- positivity, and a named `CourantViolation`;
- mass and total-energy conservation to round-off between two walls;
- a pinned test that the default uses the equilibrium sound speed;
- a **semi-analytical HEM water double rarefaction** whose plateau falls inside
  the dome, built from one quadrature and one root with petir. Its candidate
  citations still need reading.

**B6. Edwards stays in `tampines-steam-tables`** as the critical-flow validation
case, calling `HemPipe` from `tests/edwards_blowdown.rs`.

- This needs a **path-only dev-dependency back-edge**
  `tampines-steam-tables → tampines` (D2).
- A toy workspace showed that integration tests work and `src/` unit tests
  would not, so Edwards must stay in `tests/`.
- The historical PIMPLE Edwards numbers are kept as recorded, not overwritten.

### Phase C: petir numerics (runs alongside A and B)

**S0. Determinism harness first, with a negative control.**

- Golden FNV hashes, one sub-hash per routine. Cover an IF97 sweep, a
  200-step Edwards run and CoolProp sweeps.
- Targets: linux, windows, macOS-aarch64 and `wasm32-wasip1` CI.
- Record the hashes **before** migrating, so the test is shown to be able to
  fail.
- Also trace the untraced ~9e-16 wasm-vs-native Sod difference. `sqrt` cannot
  cause it, because it is correctly rounded on every target.

**S1. Extend petir's `RealMath`** with `r_powi`, `r_log10`, `r_cbrt`, trig and
the rest. These are all `libm` delegations. Also fix petir's self-contradiction
on `powi`, where `mathf.rs` and `real.rs` disagree.

**S2. `tampines-steam-tables`**, which is already mostly on petir: about 258
IF97 `powi` sites and 1 `atan` remain on the hot path.

**S3. `outram-park-fork-coolprop`**, which is not migrated at all: about 300
hot-path sites. Benchmark it, because petir `powf` is 1.4–1.6× slower than the
platform's.

**S4–S7.** Deduplicate the ~15k-line `openfoam_source` copies onto basic-lib.
Then migrate basic-lib, re-lifting petir's nine verbatim files in the same
change. Then decide root-finder provenance and sweep the off-path modules.

**On `outram-foam-basic-lib` and the hot path.** The solvers use only its mesh
and primitive types, which call nothing but `sqrt`, so it is **not on the
transcendental hot path**. Its migration is for consistency, not for this
solver's determinism.

## Decisions for the maintainer

- **D1. Scheme.** Do we keep the pressure-based PIMPLE path (Phase A), build a
  density-based KNP on `[ρ, ρu, ρE]` (Phase B), or do both?
  - The plan as written does both: A stabilises what the components use today,
    and B gives the exported HEM solver.
  - `TampinesSteamArray` and `OPCPFluidArray` must stay either way, because
    components, examples and `htgr_sim_v1` use them.
- **D2. The back-edge.** Do you approve the path-only dev-dependency
  `tampines-steam-tables → tampines` so that Edwards stays where it is? The
  fallback is an Edwards-HEM test inside `tampines`.
- **D3. Time integrator.** petir has no SSP-RK, and its rules forbid AI-written
  numerics. The options are forward Euler (as upstream `rhoCentralFoam` uses),
  porting an SSP-RK2 from a named upstream, or granting an exception.
- **D4. CoolProp in the dome.** `state_hrho` refuses two-phase states. The new
  `(ρ,e)` root over `state_ph` would cover them, but nothing has verified it.
- **D5. Defaults that move validated numbers.** Do you want implicit energy
  convection (A2) and `PsiRefresh::OncePerOuterCorrector` as the defaults? Both
  restore upstream behaviour, and both require re-measuring Edwards.
- **D6. Extending petir.** May petir's `RealMath` gain the `libm`-delegated
  methods (S1)? `mathf.rs` deliberately left them out ("opt in knowingly").
