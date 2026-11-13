# HEM pipe solver (`hem::HemPipe`) vs exact Riemann solutions, a semi-analytical HEM rarefaction, and Edwards–O'Brien

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-10-16 (UTC), by running the tests named below in release mode.
**Crate version / commit:** `tampines-steam-tables` 0.2.9 on base `79b63cdbe` (gh #902, epic #896), AI-written, not yet human-reviewed.

## What is under test

`tampines_steam_tables::hem::HemPipe`: an explicit, density-based finite-volume
solver for the 1-D homogeneous-equilibrium (HEM) Euler equations on
`W = [ρ, ρu, ρE]`.

- KNP central-upwind flux: the shared basic-lib kernel `fvc::central_upwind`.
- Reconstruction as upstream OpenFOAM-10 `rhoCentralFoam.C`: `ρ`, `ρU`,
  `rPsi = p/ρ`, `e` and `c` each limited with van Leer.
- Courant number `centralCourantNo.H` and adaptive Δt `setDeltaT.H`, default
  central Courant 0.2.
- Time integration: `petir::ssprk` SSP22 by default.
- Closure: the compile-time trait `HemEquationOfState`. `Iapws97Hem` recovers
  `p` from `(ρ, e)` with a bracketed Brent root over the IF97 `(p,h)` flash and
  uses the equilibrium sound speed `w_ph_eqm_hem`. `IdealGasHem` is a perfect
  gas. `tampines` adds `CoolPropHem`.

Deviations from upstream are listed in the module doc of `src/hem/pipe.rs`:
a non-lagged Courant number, `rPsi = p/ρ`, the equilibrium `c`, inviscid and
frictionless, and errors instead of bounds.

## Methodology and results, case by case

Every pass criterion below was fixed **before** the case was first run. Where
a criterion was missed, the miss is reported here and in the test, and is not
hidden.

### 1. Ideal-gas Sod vs the exact Riemann solution and Sod (1978) Table II

`tests/hem_pipe_verification.rs::ideal_gas_sod_matches_the_exact_riemann_solution`.

**Methodology.** appbuilder's Sod case:
- air, γ = 1.4, R = 287.1 J/(kg·K);
- 100 cells on [−5, 5] m;
- ρ = 1 | 0.125 kg/m³, p = 1e5 | 1e4 Pa, transmissive ends;
- t = 0.2·t0 = 6.32456e-3 s.

The exact solution is Toro (2009) ch. 4. Its code is copied verbatim from
appbuilder into `tests/common/toro_exact.rs`; no dependency was added. It is
evaluated at all 100 cell centres. Norms are relative to the field peak
(1 kg/m³, 293.3 m/s, 1e5 Pa), as in appbuilder's `RESULTS.md`.

**Registered criterion:** whole-field L2 ≤ 5 % of peak for ρ, u and p.

**Results:**

| configuration | steps | L2 ρ | L2 u | L2 p | L∞ u |
|---|---|---|---|---|---|
| default (adaptive Co 0.2, SSP22, van Leer), 100 cells | 109 | 1.53 % | **5.56 %** | 1.25 % | 51.9 % |
| fixed Δt = 1e-6 s, forward Euler (appbuilder's settings) | 6325 | 1.51 % | 5.35 % | 1.20 % | — |
| default, 200 cells | 219 | 1.03 % | 3.99 % | 0.80 % | — |
| default, 400 cells (diagnostic run, not in the test) | — | — | 2.98 % | — | — |
| appbuilder `RhoCentralFoam`, 100 cells (its RESULTS.md, 2026-07-17) | 6325 | 1.41 % | 5.17 % | 1.06 % | 48.0 % |

Sod Table II faithful stations (x/L = 0.1, 0.2, 0.5, 0.6, 0.8, 0.9), worst
error relative to peak: ρ 0.12 %, u 0.15 %, p 0.04 %. appbuilder's are
0.43 / 0.96 / 0.43 %.

**Interpretation, and the miss.**
- ρ and p meet the registered bar. **u misses it: 5.56 % > 5 %.**
- The miss is one cell. The shock cell's 51.9 % alone contributes about 5.2 %
  to a 100-cell L2. Away from the shock (|x − 3.5 m| > 0.3 m), u's L2 is
  1.21 / 0.62 / 0.32 % at 100 / 200 / 400 cells: first-order convergence, as
  expected of a shock-capturing scheme near a discontinuity.
- appbuilder's own velocity L2 (5.17 %) is also above 5 %. Its test asserts
  the Table II faithful-station error, not a whole-field L2. Its `CLAUDE.md`
  claimed "L2 within 5 % per variable"; that claim is corrected in this change.

The test asserts:
- ρ and p whole-field L2 ≤ 5 %;
- the Table II faithful stations ≤ 5 % of peak in all three variables (appbuilder's actual bar);
- that u's L2 falls when the mesh is doubled.

It prints the u miss. The 5 % u bar was **not** loosened.

### 2. Positivity: Toro test 2 (strong double rarefaction)

`toro_test_2_double_rarefaction_stays_positive`.

**Methodology.** Toro 2009 Table 4.1 test 2:
- ρ = 1, u = ∓2, p = 0.4, γ = 1.4, exact p* = 0.00189;
- 100 cells on [0, 1], transmissive ends, to t = 0.15;
- stepped one step at a time.

**Registered criterion:** under the default numerics, every step succeeds,
and p > 0 and ρ > 0 after every step.

**Result: MISSED with the default van Leer reconstruction.** At t = 0.06215
the cell beside the centre reaches a specific internal energy of
`e = −2.18e-5 J/kg` (exact ≈ 0.24). The step returns
`HemError::EquationOfState`, the state is rolled back, and nothing is clamped.
Sweep (2026-10-16, run with `run_until`):

| Courant | integrator | van Leer | upwind (1st order) |
|---|---|---|---|
| 0.2 | SSP22 | fails (t = 0.062 when stepped singly) | **passes**, 150 steps, min p 7.44e-3 |
| 0.1 | SSP22 | fails, t = 0.0123 | passes |
| 0.05 | SSP22 | fails, t = 0.0111 | passes |
| 0.025 | SSP22 | fails, t = 0.0111 | passes |
| 0.1 | forward Euler | fails, t = 0.0079 | passes |
| 0.2 | SSP33 | fails, t = 0.0093 | passes |

So the failure is **not** a time-step problem. The cause is the
reconstruction taken from upstream: ρ, ρU and e are limited independently, so
the face states do not average back to the cell's ρE. Near vacuum, where
E ≈ ½ρu², the update of E − ½ρu² is then not sign-preserving. Upstream has no
positivity limiter either. **Open item:** a positivity-preserving
reconstruction (new numerics, outside gh #902).

The first-order run's centre pressure is 0.00744, against the exact 0.00189.
That over-prediction is expected of a first-order scheme near vacuum.

The test asserts what is true and can still fail:
- the upwind run stays positive throughout;
- the van Leer run either finishes positive or stops with a named
  `EquationOfState`/`Unphysical` error, leaving a finite, positive state.

### 3. Fixed Δt above the stability limit

`a_fixed_step_that_is_too_large_is_refused`.

**Methodology.** Sod with `Fixed(1e-3 s)`.

**Result:** `CourantViolation { courant: 1.8708, limit: 0.5 }`, returned before
the step; the state is unchanged (snapshot equal). `Fixed(1e-6 s)` succeeds.

### 4. Conservation between two walls on IF97 water, half subcooled, half two-phase

`two_walls_conserve_mass_and_total_energy_to_round_off_on_if97`.

**Methodology.**
- 1 m, 40 cells, `Wall` at both ends.
- Left half: 2 MPa, 450 K subcooled liquid.
- Right half: 1 MPa, 1500 kJ/kg two-phase (quality ≈ 0.37).
- 1000 adaptive steps.

**Registered criterion:** |ΔΣρV|/ΣρV and |ΔΣρEV|/ΣρEV ≤ 1e-12 (plan 03 §4.1).

**Result (pass):**
- after 1000 steps, t = 2.8295e-2 s;
- |ΔM|/M = **1.7e-15** and |ΔE|/E = **8.5e-16**;
- minimum pressure 622.6 kPa;
- every step closed through IF97 with no error.

### 5. Pinned defaults, and the equilibrium sound speed

`the_defaults_are_pinned_and_if97_uses_the_equilibrium_sound_speed`.

The test pins:
- `HemNumerics::default()` = adaptive central Courant 0.2 (`max_dt` 1 ms), van Leer, SSP22;
- `HemPipe` = `HemPipe<Iapws97Hem>`.

At 1 MPa, 1500 kJ/kg the sound speeds agree:
- closure c = `w_ph_eqm_hem` = Kieffer eq. 28 = **264.223 m/s**;
- the equilibrium isentrope's own finite difference also gives 264.223 m/s;
- frozen Wood–Wallis is 503.076 m/s.

The speed a solver cell reports is the same equilibrium speed.

**Correction:** the first version also asserted `c_eqm < ½ c_frozen`. That
factor was an unchecked guess, and the measured ratio is 0.525. It was
replaced by `c_eqm < c_frozen`, which follows from thermodynamic stability. The
identity checks beside it all passed.

### 6. HEM water double rarefaction vs a semi-analytical solution (plan 03 §4.2)

`hem_water_double_rarefaction_semi_analytical_prediction` (the prediction) and
`hem_water_double_rarefaction_matches_the_semi_analytical_plateaus` (the run).

**Methodology.**
- Subcooled water at 5 MPa, 450 K; u = ∓5 m/s either side of the centre.
- Exact HEM solution: the Riemann invariant u(p) = −u0 + ∫_p^{p0} dp/(ρc) along
  the isentrope s0, with the **equilibrium** c, and u(p*) = 0 by symmetry.
- Construction (`tests/common/hem_rarefaction.rs`):
  - the bubble pressure, found by bisection on the IF97 flash's own
    Region 1 / Region 4 switch;
  - two `petir::integration::qags` quadratures, split at the sound-speed jump;
  - one Brent root for p*.
- The quadrature tolerance is 1e-7, because the IF97 backward `(p,s)`
  correlations make the integrand smooth only to about 1e-6; `qags` reports
  `RoundOff` at 1e-10.

**Prediction, computed and written into the test before the solver was run on
the case:**

| quantity | predicted |
|---|---|
| bubble pressure on the isentrope, p_b | 0.920004 MPa |
| saturated-liquid plateau velocity, u_b | −1.7495 m/s |
| centre pressure p* (inside the dome) | 0.908004 MPa |
| centre density ρ* | 726.20 kg/m³ (saturated liquid: 887) |
| equilibrium speed at p*, c* | 9.37 m/s |
| wave speeds (left half) | head −1417.9, liquid tail −1403.2, two-phase fan −9.51 to −9.37 m/s |

**Run.** 1 m, 100 cells, transmissive ends, IF97, default numerics, to
t = 20 ms.

**Registered criteria:**
- plateau (0.30 ≤ |x| ≤ 0.45 m): p within 1 % of p_b, |u| within 5 % of |u_b|;
- centre (|x| ≤ 0.1 m): p within 0.25·(p_b − p*) = 3.0 kPa of p*, and ρ within 5 % of ρ*.

**Results (pass), 419 steps, 4.3 s:**

| | measured | predicted | difference |
|---|---|---|---|
| plateau p | 0.919977 MPa | 0.920004 MPa | −0.003 % |
| plateau \|u\| | 1.7486 m/s | 1.7495 m/s | −0.05 % |
| centre p | 0.908000 MPa | 0.908004 MPa | −4 Pa (gate 3000 Pa) |
| centre ρ | 726.06 kg/m³ | 726.20 kg/m³ | −0.02 % |

**Interpretation.** The HEM closure carries the flashing fan correctly through
the solver: an equilibrium state inside the dome (quality ≈ 1e-3), 12 kPa below
the bubble pressure. The transition between the centre state and the plateau
is smeared over about 5 cells.

One detail matters for reading the step count. In the numerical solution, the
"saturated-liquid" plateau cells sit at a quality of about 5e-7, just inside
the dome, so their equilibrium sound speed is about 8 m/s, not the liquid
1400 m/s. In exact HEM the speed is discontinuous at x = 0 and both values
bound the plateau's wave speeds. That is why 20 ms took only 419 steps.

**Citations.** The Riemann-invariant construction is standard (Toro 2009,
ch. 4). The real-EOS wave-curve references listed in plan 03 §4.2 (Menikoff &
Plohr 1989; Müller & Voss 2006; Saurel, Petitpas & Abgrall 2008) are **not
verified**: none is in the kovan corpus, none was read, and none is cited as
support.

### 7. Edwards–O'Brien blowdown against GS-1

`tests/hem_pipe_edwards.rs::edwards_obrien_blowdown_on_hem_pipe`.

**Methodology.**
- Case data (B-T-3271 Table 1, Hendrie profile, digitised GS-1 data) is
  copied unchanged from `tests/edwards_blowdown.rs` into
  `tests/common/edwards_case.rs`.
- Geometry: L = 4.096 m, D = 0.073 m, 24 uniform cells.
- Initial state: 7 MPa with the Hendrie temperature profile.
- Boundaries: `Wall` at x = 0. At x = L, `ChokedBreak` to 0.1 MPa, its area
  fraction ramped 0 → 0.87 over 1 ms. The break flux is this crate's HEM
  critical-flow dispatcher at the break cell's stagnation state.
- Numerics: default, inviscid.

No numerical prediction was written down before this run; that is a gap in
this record.

**Registered criteria:**
- the run completes 0.6 s;
- `|M(t) + M_out(t) − M(0)| ≤ 1e-12 M(0)`.

The GS-1 RMSE and the plateau are reported, not gated. Nothing was tuned to
them.

**Results (pass on both gates):**

| | HemPipe, 24 cells | HemPipe, 96 cells (`EDW_HEM_CELLS=96`) | PIMPLE record (`tests/edwards_blowdown.rs`, Linear he, 24 cells, 30 µs) |
|---|---|---|---|
| completes 0.6 s | ~~yes, 4014 steps, 20.6 s~~ | ~~yes, 12602 steps, 175 s~~ | yes |
| GS-1 plateau, mean 0.02–0.06 s | ~~364.9 psia~~ | ~~372.1 psia~~ | ~~359.0 psia~~ |
| GS-1 RMSE vs data, 16 points, 0–0.30 s | ~~44.1 psia~~ | ~~45.2 psia~~ | ~~42.8 psia~~ |
| mass conservation incl. break outflow, max rel. | ~~4.2e-15~~ | ~~3.4e-15~~ | — |


**CORRECTED 2026-10-16 (re-measured on the merged tree, release).** The table
above was first recorded from an earlier state of the #902 code. Re-running the
committed code (`3037f2036`, and again after the #900 and #903 S2/S3 merges,
with identical output) gives:

| | HemPipe, 24 cells | HemPipe, 96 cells | PIMPLE after #900 (Linear he, 24 cells, 30 µs) |
|---|---|---|---|
| completes 0.6 s | yes, 4025 steps, 16.3 s | yes, 12518 steps, 146 s | yes |
| GS-1 plateau, mean 0.02–0.06 s | **368.0 psia** (was 364.9) | **372.4 psia** (was 372.1) | 360.5 psia (was 359.0, before #900) |
| GS-1 RMSE vs data | **44.2 psia** (was 44.1) | **45.1 psia** (was 45.2) | 17.2 psia (was 42.8, before #900) |
| mass conservation, max rel. | 5.2e-15 (was 4.2e-15) | 4.9e-15 (was 3.4e-15) | — |

The struck values above are kept for the record. Since #900, the stabilised
PIMPLE solver is **2.6× better** than HemPipe in GS-1 RMSE (gh:#918).

**Re-measured 2026-11-02 (gh:#1111), release, 24 cells.** The subcooled
critical-flow solver behind the break boundary changed (§12). Before = HEAD
`90ecc3373` with the old solver files; after = the §12 solver. Only the three
solver files differ between the two runs.

| | HemPipe before | HemPipe after | PIMPLE before | PIMPLE after |
|---|---|---|---|---|
| steps to 0.6 s | 4270 | 4224 | 20 000 (fixed Δt) | 20 000 |
| GS-1 plateau | 367.6 psia | **363.8 psia** | 360.5 psia | **364.0 psia** |
| GS-1 RMSE | 42.8 psia | **42.7 psia** | 17.2 psia | **20.5 psia** |
| peak break flow | 48.90 kg/s | **81.51 kg/s** | 98.2 lbm/s (44.5 kg/s) | 101.0 lbm/s (45.8 kg/s) |
| mass conservation, max rel. | 4.5e-15 | 4.4e-15 | — | — |

- **The "before" HemPipe column already differed** from the 2026-10-16 row
  above (368.0 psia / 44.2 psia / 4025 steps). That row was stale on HEAD
  before this change, cause not traced. The struck-and-corrected rows above
  remain the history.
- **HemPipe's peak break flow rose 67 %** (48.9 → 81.5 kg/s); PIMPLE's rose
  2.9 %. ~~The likely reason, not checked: HemPipe's break cell passes
  through near-saturated states where the old `ρ_f c_2φ` branch returned
  the low sonic-kink flux. When the peak occurs was not traced.~~
  **Traced 2026-11-02 (§12, "Tracing").** The 81.5 kg/s is one 1-ms sample
  at t = 0.043 s. In it, a single break evaluation saw recompressed liquid
  at 5.03 MPa, produced when the break cell's void collapsed. The sustained
  break flow is unchanged within a few %.
- The GS-1 comparison quantities moved by ≤ 3.8 psia on both solvers.
- Both tests pass.

GS-1 pressure (psia), 24 cells. **NOT RE-CHECKED 2026-10-16:** this series came from the same earlier code state as the struck table values, and the test does not print it, so it was not regenerated. Treat it as illustrative only:

```csv
t_s,hem_pipe,data
0.000,1015.3,985.0
0.020,336.9,350.8
0.040,375.2,364.0
0.060,381.2,367.4
0.080,383.4,343.0
0.100,376.4,312.2
0.120,359.1,298.2
0.140,339.8,296.0
0.160,344.8,288.7
0.180,340.2,288.4
0.200,332.4,289.0
0.220,327.7,283.1
0.240,309.8,271.4
0.260,284.5,253.0
0.280,267.4,228.0
0.300,257.8,190.1
```

**Interpretation.**
- The plateau falls inside the experimental 350–367 psia band, and the result
  is close to the PIMPLE record on both measures.
- From 0.08 s on, the HEM pipe depressurises too slowly: it is 20–70 psia
  above the data. That is the known direction of an equilibrium model's error
  for a subcooled blowdown (no thermal non-equilibrium, equilibrium critical
  flow). It was not tuned.
- The PIMPLE case reaches its numbers only with the unbounded `Linear`
  enthalpy scheme (gh:#406). The density-based solver completes with the
  bounded van Leer default and no clamps, which is what Phase B was for.
- Past about 0.55 s, GS-1 falls below the 0.1 MPa back pressure (11.4 psia at
  0.6 s). The break model carries no backflow, so the break then acts as a
  closed end. Recorded as a model limitation.
- The PIMPLE numbers are quoted from that file and are unchanged.

### 8. CoolProp closure (in `tampines`, `tests/hem_coolprop.rs`)

**(ρ, e) inside the dome (decision D4): works.**
- Case: water at 1 MPa, h = 800 / 1500 / 2500 kJ/kg, so x = 0.0186 / 0.366 / 0.862.
- `CoolPropHem` closes `(p, h)` with the Maxwell VLE. `(ρ, e)` → p by the same
  bracketed Brent root (`pressure_from_rho_e`) returns 1 MPa to **5.7e-9 –
  6.4e-9** relative, with or without a hint.
- That figure is the tolerance of coolprop's own `saturation_temperature`
  bisection (1e-8·p), not of the root. The quality also round-trips to 1e-8.
- Against IF97 at the same `(p, h)`: ρ −0.34 / −0.02 / −0.01 %,
  T −0.0076 K, c_eq +0.32 / +0.02 / +0.01 %. That last figure is an independent
  cross-check of the IF97 Kieffer equilibrium speed against CoolProp's
  finite-difference HEM speed.
- Cost: 40–75 ms per `(ρ, e)` closure; one hinted call took 3.5 s because its
  hint sat in the liquid region the flash refuses.

~~**Compressed-liquid water: does NOT work, a pre-existing coolprop defect.**~~ **CORRECTED 2026-11-13** (gh:#1349 fixed `density_pt`; measured results in `crates/tampines/verification_and_validation/hem_coolprop_liquid_water_gh1349.md`: 2 MPa / 350 K and 1.3 MPa / 455 K now close to the liquid and agree with IF97 to 1.3e-5 and 1.5e-5 in density; the liquid shock tube agrees to 0.15 % of the jump). The 2026-10-16 record of the defect follows, struck through:
- `outram_park_fork_coolprop::flash::density_pt` starts its Newton from the
  ideal-gas density.
- `state_pt(350 K, 2 MPa)` returns ρ = 322.0 kg/m³ (the critical density) with
  h ≈ −6.4e18 J/kg.
- `state_pt(455 K, 1.3 MPa)` returns a metastable vapour; `state_ph` inherits
  both.
- `CoolPropHem` now rejects any single-phase root inside the saturation dome
  at its own temperature, as an `EquationOfState` error
  (`coolprop_refuses_compressed_liquid_water_instead_of_returning_a_wrong_root`).
- Upstream CoolProp's `solver_rho_Tp` starts a liquid from the
  saturated-liquid ancillary density. This was **not verified** against the
  upstream source in this change. **CORRECTED 2026-11-13:** gh:#1349's fix does this (RESULTS.md defect 1 there).

**Single-phase water shock tube, IF97 vs CoolProp (cross-code, not
validation).** ~~Because of the liquid defect this is run in superheated steam~~ (the liquid case is now restored, see above; the steam case stays):
- 500 K, 0.2 | 0.1 MPa, 40 cells, t = 0.5 ms;
- 36 steps each; star pressure ~~0.139940 MPa (IF97) vs 0.139938 (CoolProp)~~ **re-measured 2026-11-13: 0.140066 MPa (IF97) vs 0.139938 (CoolProp)**;
- max |Δp| = ~~8.7 Pa = **0.009 %**~~ ~~**290.2 Pa = 0.290 %** of the jump (the CoolProp side did not move; the IF97 side did, from a change to the IF97 solver since 2026-10-16 that this change did not investigate, still inside the test's 5 % bound)~~ **EXPLAINED 2026-11-13 (gh:#1346):** the move came from commit 1211cc9de (2026-11-01, gh:#1061), which made wall friction a default of `HemPipe`. `Iapws97Hem` supplies a viscosity, so friction acts (cell 20: u = 144.5 m/s, rho = 0.659 kg/m3, mu = 1.58e-5 Pa s, Re = 2.1e5, D_h = 35.7 mm); the CoolProp closure supplies none and is frictionless. The first differing cells are 19 and 20, in step 1 (second SSP stage, where u is first nonzero); by step 2, 8 cells differ. With `without_wall_friction` the IF97 tube reproduces **0.139940 MPa, p19 = 140376.365 Pa, p21 = 140104.104 Pa, exactly as at 3037f2036**, and max |dp| vs CoolProp is **8.7 Pa = 0.009 %** again (quote this like-for-like figure; 290.2 Pa compares friction against no friction). The 7 backward-equation coefficients fixed by 7be8a7d24 (gh:#1346) are not involved: 1211cc9de already gives 0.140066 and the ablated HEAD gives 0.139940. Bisect (star pressure at the tested commits): 3037f2036 0.139940, d9d81741e 0.139940, 2c9b7a02e 0.139940, 1211cc9de 0.140066, c8182db14 0.140066, 3008d6027 0.140066, 831b1ae1a 0.140066, HEAD 0.140066;
- CoolProp took 28 s, IF97 0.12 s.

### 9. `Iapws97Hem::break_flow` for subcooled liquid at low Δp (gh:#1102, 2026-11-02)

> Research, education and V&V only. AI draft, unreviewed.

**Defect.** `break_flow` returned `Err("h0 − h_b < 0")` for 300 K liquid at
2.31 → 1.50, 2.0 → 1.5 and 1.2 → 1.0 bar. Called at 3 → 1 bar it did not
error, but it returned 14 987 kg/(m² s), 25 % below Bernoulli. The choke
check was not the problem: the dispatcher's p* ≈ 3540 Pa ≈ p_sat(300 K) lies
below every back pressure, so the unchoked branch ran. There it formed
`h0 − h(p_b, s0)` from `s0 = s(p0, T_ph(p0, h0))` and
`h_b = h(p_b, T_ps(p_b, s0))`. Each passes through an IF97 backward equation
(consistency |ΔT| ≤ 25 mK, so up to ≈ 100 J/kg of `c_p ΔT`), and the true
drop `∫v dp ≈ v Δp` is only 20–200 J/kg here. Measured before the fix:
`Δh` = −6.8, −38.1, −68.8 and +113.1 J/kg, against `v Δp` ≈ 81, 50, 20 and
200 J/kg.

**Prediction, written before the fix.** Compute the liquid isentrope from the
Region 1 forward (Gibbs) equations only, with a Newton solve on
`s_1(T, p_b) = s0` at `T0 = T_ph(p0, h0)`. Then `Δh` is the exact `∫v dp`,
positive, and `G → √(2ρΔp)`, with a relative deviation of order `κ_T Δp`
(≈ 1e-4 at 2 bar in 300 K water). Predicted for 2.31 → 1.50 bar:
G ≈ 12 700 kg/(m² s).

**Fix.** A new private `Iapws97Hem::liquid_isentropic_drop` (in
`src/hem/iapws97.rs`) handles a Region 1 stagnation whose isentrope is still
liquid at `p_b` (`p_b > p_sat(T_b)`, 273.15 ≤ T_b ≤ 623.15 K). If the Newton
solve fails it returns an error, never a default. Other states keep the
`(p, s)` flash route. The stagnation enthalpy used is `h_1(T0, p0)`, which
differs from the given `h0` by the backward `T(p, h)` consistency error.

**Methodology and pass criteria** (tests in `src/hem/tests.rs`, each criterion
fixed before the run):

| Test | Criterion | Measured 2026-11-02 |
|---|---|---|
| `gh1102_cold_liquid_low_dp_break_flow_is_unchoked_bernoulli`: 300 K, the four Δp above | `Ok`, unchoked, `abs(G/√(2ρ0Δp) − 1) ≤ 2 κ_T Δp` (κ_T = 4.5e-10 1/Pa) | all `Ok`, unchoked. 2.31 → 1.50 bar: G = 12 706 kg/(m² s), as predicted. Worst deviation 6.7e-5 (3 → 1 bar, bound 1.8e-4), scaling as −3.3e-10 Δp/Pa |
| `gh1102_break_flow_tends_to_bernoulli_as_dp_vanishes`: 300 K / 2.31 bar, 370 K / 1.2 bar, 500 K / 7 MPa; Δp = 0, 1, 10, 100 Pa | `G(0) = 0`; `abs(G/G_B − 1) ≤ 1e-6` (round-off of Δh at 1 Pa plus κ_T Δp) | worst 7.1e-8 (300 K, 1 Pa) |
| `gh1102_choked_transition_is_continuous_for_edwards_liquid`: 500 K / 7 MPa, `p_b = p*(1 ± 1e-9)`, plus a 201-point sweep `p0 → p*/2` | `abs(G(p*⁺)/G*(p*⁻) − 1) ≤ c_p·25 mK/Δh` = 2.2e-2; G non-decreasing as `p_b` falls | −8.1e-3 (85 583 against 86 283 kg/(m² s)); monotone |
| `gh1102_choked_transition_is_continuous_for_cold_liquid` (`#[ignore]`, gh:#1111): same at 300 K / 2.31 bar | same | **FAILS.** `G(p*⁺)` = 21 291 and `G*` = 16 387 kg/(m² s), so G drops 23 % as `p_b` crosses `p*`. The ratio bound (0.46) is too wide to see it; the monotonicity assertion catches it |

**Interpretation.** The unchoked branch for subcooled liquid is now
Bernoulli-exact to within compressibility, continuous to Δp = 1 Pa, and
continuous with the choked flux at the Edwards stagnation state. The
remaining miss is on the **choked** side for cold low-pressure liquid:
`get_critical_pressure_and_mass_flux_subcooled_liquid_ph` still builds its
energy objective through the same backward-equation round trip, so its G* is
about 23 % low at 300 K / 2.31 bar. Changing that solver re-measures the
Zaloudek, Moody and Marviken V&V, so it is filed as gh:#1111 rather than
done here. **Quote** the unchoked numbers above. Do not quote the 300 K
choked G* as a validated value.

### 10. gh:#1111: the subcooled critical solver's cold-liquid G*, diagnosed and measured; fix NOT applied (2026-11-02)

> Research, education and V&V only. AI draft, unreviewed.

**Status: diagnosed, fix measured, fix reverted.** The fix moves one asserted
Moody point outside its tolerance (below). The brief said to stop and report
in that case rather than force it, so the solver is unchanged and the
`gh1102_..._cold_liquid` test stays `#[ignore]`d.

**Cause (measured).** The defect is narrower than "backward-equation round
trips". `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` takes
`s0 = s_ph_eqm(p0, h0) = s_1(T_ph(p0, h0), p0)`. That is the entropy of the
state at the backward-equation temperature `T0`. The kinetic energy is then
`h0 − h(p, s0)` with the **given** `h0`, not `h_1(T0, p0)`. The two differ by
the backward `T(p, h)` consistency error, and the in-dome `h(p, s0)` the
search uses is a forward-equation, quality-weighted value. Measured with a
temporary diagnostic (release, removed afterwards):

| stagnation | `T_ph(p0,h0) − T0` | `h_1(T0,p0) − h0`, J/kg | `h0 − h(p_b,s0)`: given `h0` → consistent | G* now → consistent, kg/(m² s) |
|---|---|---|---|---|
| 300 K / 2.31 bar | +22.3 mK | +93.0 | 135.2 → 228.2 | 16 387 → 21 291 |
| 370 K / 1.2 bar | +0.73 mK | +3.1 | 27.6 → 30.7 | 7 137 → 7 524 |
| 500 K / 7 MPa (Edwards) | −10.1 mK | −46.6 | 5 370 → 5 324 | 86 283 → 85 908 |

All three take the bubble-point branch (`p* = p_bubble`). `s0`, `p_bubble`,
the sonic map and the quality at a given `p` do not change.

**Fix tried.** For a Region 1 stagnation (`T0 ≤ 623.15 K`), use
`h0 := h_1(T_ph(p0, h0), p0)` throughout the solver. This is the same choice
`Iapws97Hem::liquid_isentropic_drop` makes (§9), so the two meet at `p*`. The
patch is about 30 lines in `stagnation_point_outside_vle_ph_dome_multiphase.rs`.

**Predictions, written before the fix was run** (the session's predictions
file). P1: at 300 K, G* goes to 21 291, the transition is continuous to < 1e-6
and the cold test passes. P2: Edwards G* goes to 85 908, rel −8.1e-3 → ≈ −3.8e-3.
P3: at 370 K, G* rises 5.4 %. P4: Zaloudek sonic-kink points unchanged,
energy-branch points move < 1 %. P5: Moody asserted points byte-identical,
on the belief that only in-dome points are asserted. P6: Marviken means move
< 0.5 %. P7: generic-dispatcher Region 1 points move < 1 %.

**Outcomes** (lib run filtered to `converging_diverging_nozzles`, `hem::`,
`fhr_sim`; 154 tests before, 153 pass and 1 fails after):

| prediction | outcome |
|---|---|
| P1 | **held.** G* = 21 291.03; rel = +4.8e-7; the sweep is monotone; the cold test **passes** with the fix |
| P2 | **held.** G* = 85 907.8; rel = −3.78e-3 (was −8.11e-3), bound 2.2e-2 |
| P3 | **held.** 7 137 → 7 524 |
| P4 | **partly wrong in size.** The x_t = 0.05 subcooled points at 1000/1500/2000 psia moved by more than 1 %: p* errors −0.99/−1.11/+1.39 % → −0.10/−0.12/+0.07 % (all better), G errors −5.0/−6.7/−0.6 % → −4.1/−5.7/−2.0 % (two better, one worse). x_t = 0.10 at 1500 psia: p* +0.13 → −0.06 %, G −2.44 → −2.65 %. All Zaloudek tests still pass |
| P5 | **wrong.** The Moody tests also assert the deeply subcooled Region 1 points (`[DEEP]`, tol 0.08 in log10). Every isobar moved slightly. **`isobar_pref_0_50` FAILS** at h/h_ref = 2.2549 (sub_ratio 5.03): err 0.005 → **0.120**. Same isobar, h/h_ref = 0.4902: err 0.069 → 0.003 (the "worst observed" figure in `MOODY_DEEP_LOG10_TOL`'s doc). The other DEEP errors on this isobar went 0.024/0.006/0.008/0.005/0.041 → 0.015/0.008/0.005/0.023/0.033 |
| P6 | **wrong.** Test 24 was unchanged. Test 23: 14 of 29 points (p ≥ 4075 kPa) fell about 20 %, mean dev +5.9 % → −5.3 %, mean |dev| 12.6 → 10.7 %, max 23.1 % unchanged. It still passes its ±15 % mean / ±25 % point band. **Mechanism:** the −115 J/kg stagnation shift moves the quality at the energy maximum from 0.0249 to 0.0302, across the solver's `x < 0.03` sonic-kink switch, and the solver jumps branch. The switch is a knife edge: a 0.01 % change in `h0` changes G by 20 % |
| P7 | the generic-dispatcher tests pass; `CHOKE_PROBE` subcooled (5 MPa) G 95 150.6 → 95 323.5 (+0.18 %) |
| P8, added before running: the Moody 0.25 isobar's skipped DEEP point (err 0.170, "IF97 isentrope diverges from Bernoulli") becomes ≈ 18 330, err ≈ 0.012 | **held, better.** G = 18 628.8 against Moody's 18 842.8, err **0.005**. The documented 0.170 "open question" is this defect, not IF97 physics |

**Interpretation.** The consistent stagnation state is the right equation
of state: it removes a 23 % defect at 300 K and closes the long-standing 0.170
Moody 0.25 miss to 0.005. It also halves the Edwards transition step and
brings the Zaloudek subcooled choke pressures to ≈ 0.1 %. But the old Moody
0.50 / h = 2.2549 agreement (err 0.005) was a compensating error. At that
point the available `Δh` is ≈ 121 J/kg, the given-`h0` error removed 53 J/kg
of it, and the consistent HEM (= Bernoulli, `v Δp` = 121 J/kg) sits 0.12 in
log10 above Moody's graph-read value. The point is the steep, near-threshold
end of the DEEP branch (sub_ratio 5.03 against `DEEP_SUBCOOLING_RATIO` = 5.0,
and 6.71 when computed with the consistent `h0`). The fix also exposes the
`x < 0.03` branch switch as discontinuous in `h0` (Marviken 23). **Not done,
for the maintainer to decide:** whether to land the fix and record the Moody
0.50 / 2.2549 point as a known miss, or to address the branch switches first.
Tolerances were not touched. **Quote nothing from the patched solver as
validated**; the numbers in this section are measurements of an unapplied
patch.

### 11. gh:#1111 stage 2: principled branch rule + consistent state, measured; NOT applied (2026-11-02)

> Research, education and V&V only. AI draft, unreviewed.

**Status: measured, then reverted.** Maintainer decision after §10: fix the
branch switches first. I tried two physically defined rules. Both remove the
knife edges, and both break validations, so per the brief I stopped. The
solver is unchanged and the gh1102 cold test stays `#[ignore]`d. No
tolerance was touched. The patch is held by the session; its design is
described here.

**What was changed in the patch.**
1. **Consistent stagnation state.** For a Region 1 stagnation, `T0` comes
   from Newton on the forward equation `h_1(T0, p0) = h0`, starting at the
   backward `T_ph`. The given `h0` is kept and `s0 = s_1(T0, p0)`. §10's
   variant instead replaced `h0` by `h_1(T_ph, p0)`; this one does not shift
   the stagnation enthalpy. `Iapws97Hem::liquid_isentropic_drop` used the
   same `T0`.
2. **The `x < 0.03` switch, the sonic map and the `DEEP_SUBCOOLING_RATIO`
   escape were removed.** `G(p) = ρ√(2(h0 − h))` is evaluated below
   `p_bubble` from forward saturation properties at
   `x = (s0 − s_f)/(s_g − s_f)`. At `p_bubble` itself it uses `ρ_f`, `h_f`.
   A 160-point geometric scan below `p_bubble` selects the maximum, and the
   golden section refines it within the grid neighbours.
3. **Two selection rules were measured:**
   - **(A) first local maximum** going down from `p0`: the converging-nozzle
     argument that `G = ṁ/A` must rise monotonically to the throat;
   - **(B) global maximum**: the maximum-discharge principle that defines
     Moody's and Zaloudek's HEM curves, and the one the old code applied
     before its switches.

   Both reduce to the kink (`G* = ρ_f u_b`) when `G` falls immediately below
   `p_bubble`, i.e. `u_b > c_2φ`. (B)'s `G*` is a maximum of continuous
   functions, so it is continuous in `h0` even where `p*` jumps.

**Predictions** (written before each run, held in the session's predictions
file):
- Q1: the largest step in `G*` per 1 J/kg of `h0` is < 0.5 %.
- Q2: at 300 K the kink binds, the cold test passes and rel < 1e-6.
- Q3: Edwards |rel| < 1e-3.
- Q4: Zaloudek x_t ≥ 0.05 passes, with the x_t ≈ 0 low-pressure points at
  risk.
- Q5: Moody 0.50 / 2.2549 still fails (≈ 0.12).
- Q6: Marviken 24 moves to near bare-HEM, so its "budget not met" assertion
  trips.

**Outcomes, rule (A) first maximum:**
- 300 K rel = +4.2e-8 and Edwards rel = −5.5e-9 (Q2, Q3 held).
- **Zaloudek x_t = 0.05 at 1000 psia fails**: `p*` = 8502 kPa against
  6895 kPa, G −41 %. For that state `G` has a local maximum at the kink and
  a higher interior maximum at the Zaloudek throat; the reference is the
  global maximum. x_t = 0.10 at 1500 psia fails the same way.
- Rule (A) therefore contradicts the reference definition. I did not pursue
  it further.

**Outcomes, rule (B) global maximum** (filtered lib run: 148 pass, 5 fail):

| check | before (committed) | rule (B) |
|---|---|---|
| Q1 continuity, worst `|ΔG/G|` per 1 J/kg over ±300 J/kg | Marviken 4075 kPa: a 20 % jump for a −115 J/kg shift (§10) | Marviken 4075 kPa 3.6e-6; Moody 0.50/2.2549 7.8e-6; 0.50/2.3137 1.1e-5; 300 K 2.7e-7 (**held**) |
| gh1102 cold (300 K) | ignored, G drops 23 % | rel = 4.2e-8, monotone, **passes** |
| Edwards transition | rel −8.1e-3 | rel −5.5e-9 |
| hem Δp → 0 Bernoulli | 7.1e-8 | failed at 3.2e-6 / −7.8e-6 until the test's reference density was corrected (below); in the patch |
| Zaloudek x_t = 0.05 subcooled (1000/1500/2000 psia) | dp −0.99/−1.11/+1.39 %, dG −5.0/−6.7/−0.6 % | dp −0.09/−0.10/+0.03 %, dG −4.1/−5.7/−2.1 %, pass |
| Zaloudek x_t = 0.10 at 1500 psia | dp +0.13 %, dG −2.44 % | dp −0.07 %, dG −2.65 %, pass |
| Zaloudek in-dome, superheated, backward buckets | | byte-identical |
| **Zaloudek bubble-point curve (x_t = 1e-4)**, subcooled and generic dispatcher | pass | **FAILS.** Per-point (`diagnose_bubble_point_artifact`): dGlg +0.197 / +0.134 / +0.029 at 5/10/15 psia; dP −2.7/−4.0/−4.3/−3.4/−2.1 % at 20/30/50/75/100 psia, with G within 0.017; from 150 psia up both within tolerance |
| Moody DEEP points, all isobars | max err per isobar ≤ 0.069 | unchanged or better on every isobar (e.g. 0.50 h = 0.4902: 0.069 → 0.003; 6.00: max 0.013 → 0.008), **except 0.50 / h = 2.2549: 0.005 → 0.120, FAIL** (Q5 held). That test aborts there, so the 0.50 isobar's R4 points were not re-measured; the other isobars' R4 points are byte-identical |
| Marviken 23 | mean 12.6 %, max 23.1 %, pass | mean 13.9 %, signed −13.9 %, **max 31.5 % > 25 %, FAIL**. The worst points (4797–4974 kPa, 1.4–3.6 K local subcooling) are −27 to −31 %. The bare-HEM characterisation in the same file also has max 28.2 % on test 23 |
| Marviken 24 | mean 48.6 %, signed −48.5 %, 31/40 outside the band ("not validated") | mean **10.4 %**, signed −9.4 %, max 25.6 %, **1/40 outside**: now inside the 15 % mean budget. The test's assertion that the budget is NOT met trips (Q6 held) |

**Why the bubble-point curve breaks: a finding about the fixture, not
tuned away.** The x_t = 1e-4 fixture sets `h0 = h_t + u_t²/2` with `u_t`
from `mass_flux_ps_eqm_throat`. That instrument evaluates the sound speed
at quality 1e-4. At 5 psia, `v_g/v_f` = 4472, so `ρ_t ≈ 0.69 ρ_f` at
x = 1e-4. With the same velocity, `G` at the bubble point (`ρ_f u_b`) is
higher than at the fixture's throat. The fixture's throat is therefore not
the maximum of `G` along its own isentrope, and any energy-consistent
maximum-flow rule lands on the kink. The old `ρ_f c_2φ` branch reproduced
the fixture because it used the same instrument and ignored `h0`; it did not
use an energy-consistent maximum. HEM evaluated at the throat still
reproduces Zaloudek to ±0.04 (`thr_dGlg`). At 20–100 psia the maximum of
`G` is flat: G is within 0.017 in log10, while `p*` sits 2–4 % deeper.

**The hem test's reference density (instrument fix included in the
patch).** `liquid_stagnation` in `src/hem/tests.rs` took ρ0 from
`Iapws97Hem::state_p_t(..).density`. `Iapws97Hem` forms that from `(p, h)`
through the backward `T(p, h)`: 996.6101 kg/m³ at "300 K", which is the
density at 300.022 K, against ρ_1(300 K) = 996.6162. The old solver shared
that error, so the Δp → 0 check agreed with it. With a forward-consistent
solver the check measured the reference instead. The patch uses `ρ_1(T0, p0)`.
**Recorded separately:** `Iapws97Hem::state_p_t` returns a density that is
not `ρ(p, T)` at the given `T` (6e-6 relative at 300 K, 1.6e-5 at 500 K). It
is not changed here. **Checked 2026-11-02 (stage 3): not a defect.** It is a
documented design choice. `state_p_t` routes through `(p, h)` "so ρ and e
are consistent with the (ρ, e) inversion" (module doc, "Consistency of the
initial state"). It stays as is.

**Interpretation.** A continuous, energy-consistent rule is available. Rule
(B) improves the 300 K, Edwards, Zaloudek x_t ≥ 0.05 and most Moody
numbers, and it turns Marviken 24 from a 48.6 % miss into a 10.4 % one.
Three validations then disagree with it:
- the Zaloudek x_t = 1e-4 fixture at ≤ 100 psia, which is built with an
  energy-inconsistent sonic definition;
- Moody 0.50 / 2.2549 (+0.12 in log10, a steep graph-read point);
- Marviken 23's per-point band (31.5 %). Bare HEM is at 28.2 % there too.

Under the maximum-flow definition that both references use, these are
honest misses of equilibrium HEM. They are not solver branch defects. **For
the maintainer:** whether to land rule (B) and record these as known misses
(changing the Zaloudek bubble-curve fixture's definition, the Moody point
and Marviken 23 to characterisations, and Marviken 24 to a validation), or
to keep the committed solver. Nothing here may be quoted as validated.

### 12. gh:#1111 stage 3: rule (B) LANDED, misses recorded by name (2026-11-02)

> Research, education and V&V only. AI draft, unreviewed.

**Maintainer decision:** land rule (B) from §11 and record the misses
honestly. **Applied.**
- The subcooled critical solver is the continuous global maximum of `G` on
  a forward-consistent stagnation state.
- `Iapws97Hem`'s unchoked liquid branch uses the same `T0`.
- No tolerance was changed.

**Predictions** (written before running; held in the session's predictions
file):
- R1: the new Zaloudek saturated-liquid fixture gives `p* = p_t` within
  0.5 %.
- R2: `G_calc` is the fixture's `ρ_f c_2φ(0⁺)`.
- R3: every point passes, with 5–15 psia the risk.
- Moody 0.25 in full mode passes.

R1–R3 held. **The Moody 0.25 prediction was wrong** (below).

**1. Zaloudek saturated-liquid fixture, redefined.**
`zaloudek_mass_flux_hom_eqm::hem_stagnation_for_saturated_liquid_throat`
defines it:
- the throat sits on the saturated-liquid line (x_t = 0);
- `s0 = s_1(T_sat, p_t)`;
- `u_t = c_2φ(0⁺)` from a one-sided `(∂v/∂p)_s` on the dome side, using
  forward equations;
- `h0 = h_f + u_t²/2`;
- `(p0, T0)` comes from a 2D forward Newton solve.

This is the HEM stagnation whose own isentrope has its maximum of `G` at
the throat, which is what an HEM reference throat means. It is defined
independently of the comparison. The old fixture (x_t = 1e-4, sound speed
at 1e-4) did not satisfy this at low pressure. Its numbers under the new
solver are in §11 and at the test.
- **Result:** all 16 Region 1 points pass at the unchanged tolerances, in
  both the subcooled test and the generic-dispatcher test.
- `p*` equals `p_t` exactly, except −0.02 % at 100–300 psia (a flat
  maximum).
- G/G_ref − 1: −7.8, +3.1, +5.9, +5.3, +6.2, +5.8, +3.1, −1.2, +4.1, +3.7,
  +4.1, −3.2, +0.2, −5.8, −6.6, −3.2 % (5 … 2000 psia).
- 3000 psia (Region 3 saturated liquid) keeps the old map: −0.29 % on `p*`
  in the generic test.

**2. Zaloudek, other buckets.**
- x_t = 0.05 subcooled at 1000/1500/2000 psia: `p*` −0.09/−0.10/+0.03 %,
  G −4.1/−5.7/−2.1 %.
- x_t = 0.10 at 1500 psia: `p*` −0.07 %, G −2.65 %.
- The in-dome, superheated and backward buckets are byte-identical.

**3. Moody.**
- Every isobar's deep points are equal or better (§11 table).
- **0.50 / h = 2.2549 is excluded by name** in `MOODY_KNOWN_MISSES`,
  measured err 0.120 against 0.08. It is printed and asserted to remain
  outside the tolerance.
- The rest of the 0.50 isobar asserts. Its R4 points, unmeasured in §11
  because the test aborted, all pass.
- **Moody 0.25 was tried in full mode** (its old 0.170 reason is gone):
  - 6 deep points pass (≤ 0.039);
  - **1.8627 misses at 0.156 and 1.9020 at 0.252**. These are the steep end
    of the deep branch, the same signature as 0.50 / 2.2549.
  - The isobar was kept in-dome-only rather than adding two more
    exclusions, and the numbers are recorded at the test.

**4. Marviken.**
- **Test 23: VALIDATED with 5 named misses.**
  - Mean 13.9 % (≤ 15 %), signed −13.9 %. The worst point outside the named
    misses is 19.3 %.
  - The 5 points outside 25 % are named in `MARVIKEN_23_KNOWN_MISSES`:
    4797.7 kPa −26.9 %, 4877.2 −31.5 %, 4891.6 −25.5 %, 4942.2 −31.3 %,
    4974.7 −30.6 %. They stay in the mean and are asserted to stay outside.
  - They are the top-of-envelope points where the measurement itself jumps
    ±13–30 % between neighbours. The bare-HEM characterisation also exceeds
    the band there (28.2 %).
  - **Changed verdict:** before, 29/29 were inside, but the bias was +5.9 %
    from the knife-edge branch.
- **Test 24: promoted to VALIDATED.**
  - Mean 10.4 %, signed −9.4 %.
  - 39 of 40 points are inside 25 % (worst 24.7 %). The one outside is
    named in `MARVIKEN_24_KNOWN_MISSES`: 3193.6 kPa, −25.6 %.
  - The miss-pinning test is now a validation gate at its stated
    15 %/25 % budget.
  - **Changed verdict:** NOT VALIDATED (48.6 %) → VALIDATED.

**5. HEM continuity tests.**
- gh1102 cold test: **un-ignored**, passes, rel 4.2e-8.
- Edwards transition: rel −5.5e-9.
- Δp → 0 Bernoulli, with the reference density corrected (§11): worst
  4.4e-7 (370 K, 1 Pa), within 1e-6.

**6. Determinism goldens.**
- `edw_pimple_*_{10,50,200}` moved and were regenerated, as recorded in
  `determinism_golden_moves.md`.
- Step 1 and Part A are unchanged.

**7. Edwards GS-1** (`tests/edwards_blowdown.rs`, Linear defaults): passes.
Measured on the same tree with only the three solver files swapped (release):
- break-flow peak 98.2 → 101.0 lbm/s;
- GS-1 plateau 360.5 → 364.0 psia;
- **GS-1 RMSE 17.2 → 20.5 psia (worse)**;
- `p @ t_end` 14.2 → 13.7 psia;
- 0 bound events and 0 holds, before and after.

~~Why the RMSE rose was not traced. A plausible reading, not checked: the
break cell is near-saturated during the transient, where the old
`ρ_f c_2φ` branch fired.~~ **Traced 2026-11-02** (see "Tracing" below). The
rise is one data point, t = 0.020 s. It follows from the disc-opening break
flux, which the old solver clipped for deeply subcooled liquid. **HemPipe** (`tests/hem_pipe_edwards.rs`) also moved: plateau 367.6 →
363.8 psia, RMSE 42.8 → 42.7 psia, and peak break flow **48.9 → 81.5
kg/s**. See the re-measurement table in §7.

**Integration tests** (`cargo test --release -j 2 -p tampines-steam-tables
--tests`, 2026-11-02): every target passes. The lib run gives 1040 passed,
0 failed, 14 ignored.

#### Tracing the two Edwards movements (2026-11-02)

Hypotheses were written with a predicted sign before each measurement, in
the session's predictions file. All runs are release builds on the same
tree. "Old" means only the three solver files are swapped back to
`90ecc3373`. The probes were temporary prints, removed afterwards; no code
was changed by this tracing.

**HemPipe peak break flow, 48.9 → 81.5 kg/s.**
- **H1, units or area** (prediction: if HemPipe's kg/s and PIMPLE's lbm/s
  or break areas differed, the whole series would be scaled). **Refuted as
  a defect.**
  - Both solvers use `0.87 · π/4 · 0.073²` = 3.64e-3 m².
  - HemPipe's sustained break flow is 40–48 kg/s before and after. PIMPLE's
    is 101 lbm/s = 45.8 kg/s.
  - The 81.5 is one 1-ms sample (t = 0.043 s, neighbours 47.8 and 44.9).
    The old run had a smaller one-sample spike, 48.9 kg/s at 0.048 s.
  - Both coincide with the break cell's void collapsing to ≈ 0 and
    re-flashing. (In this mesh GS-1's cell is the break cell.)
- **H2, a G* jump at the saturated-liquid line between the in-dome and
  subcooled solvers** (prediction: the new subcooled G* exceeds the in-dome
  G* as δ → 0). **Refuted.** The new dispatcher is continuous across the
  line: at 2.6 MPa, 13 196.0 at `h_f − 1 J/kg` against 13 195.9 at
  `h_f + 1`; the same to ≤ 1e-5 at 0.5, 2, 3 and 5 MPa.
- **H3, recompressed liquid at void collapse** (prediction: the stiff
  liquid EOS briefly recompresses the break cell well above `p_sat`; the new
  solver returns the Bernoulli kink flux there and the old one returned the
  clipped `ρ_f c_2φ`). **Held.**
  - New: one break call at p = 5.03 MPa, ρ 833.9, u 24.6 (with
    `p_sat` ≈ 2.6 MPa) returned G = 63 460 kg/(m² s). That single step
    accounts for the excess, ≈ 0.026 kg out in that millisecond.
  - Old: the same kind of states (3.67 and 3.37 MPa, ρ ≈ 833, u ≈ 25)
    returned 16 014 and 16 151, the sonic-kink clip.
- **Verdict.** For the state it is given, the new flux is the HEM answer: a
  5 MPa liquid with `p_sat` = 2.6 MPa discharges at the Bernoulli flux, the
  branch Marviken 24 validates. The state itself is a one-step
  void-collapse recompression in a 24-cell density-based scheme. The old
  solver happened to mask it.
- **The "peak" metric is a single-sample maximum and should not be
  quoted.** The sustained level (≈ 45–48 kg/s on the plateau) is the
  number to quote.

**PIMPLE GS-1 RMSE, 17.2 → 20.5 psia.**
- **H4** (prediction: the in-dome solver is unchanged, so the break flows
  diverge first at a Region 1 break-cell state, with after ≥ before there).
  **Held.**
- Per-step break evaluations over the 1 ms disc opening (break cell = GS-1
  cell), old → new:

  | t (ms) | old: p (MPa) → G | new: p (MPa) → G |
  |---|---|---|
  | 0.06 | 6.60 → 82 845 | 6.60 → 82 493 |
  | 0.09 | 5.86 → **15 610** | 5.87 → 74 678 |
  | 0.12 | | 4.90 → 62 992 |
  | 0.15 | | 3.87 → 47 430 |
  | 0.18 | | 2.98 → 27 712 |
  | 0.21 | | two-phase at 2.49 MPa, 12 605 |
  | 0.09 to 0.39 | 5.86 → 2.56, constant **15 610** | |
  | 0.42 | two-phase at 2.10 MPa, 9 539 | |

  - The old 82 845 → 15 610 drop at 5.86 MPa is the `DEEP_SUBCOOLING_RATIO`
    knife edge.
  - The old 15 610 is the sonic-kink clip on liquid 0.3–3.3 MPa above
    `p_sat`, the same defect as Marviken 24's 48 % deficit.
  - The new values are continuous and Bernoulli-consistent: at 3.87 MPa,
    `√(2 ρ Δp)` = 47 300.
- **Consequence.**
  - The old run's break cell flashed deeper: 2.10 against 2.49 MPa, with
    `p_sat(T)` = 2.53–2.56 MPa.
  - The new run discharges more in the first 10 ms: 0.439 against 0.346 kg
    (+27 %). By 30 ms the two are equal (1.190 against 1.173 kg).
  - GS-1 dips deeper at 20 ms: 305.2 against 344.0 psia, data 350.8.
- **The RMSE rise is that one point.** Without t = 0.020 s, the 15-point
  RMSE is 17.65 (old) against 17.60 psia (new). Per point, new against old
  error:
  - better at 0.16, 0.22, 0.26 and 0.28 s;
  - worse at 0.04–0.14, 0.18 and 0.30 s.
- **Verdict.**
  - The new break flux over the disc opening is the energy-consistent HEM
    value. The old one was the documented clip.
  - The deeper 20 ms dip is what equilibrium HEM gives once the flux is
    right. Whether it is more physical cannot be read from one point:
    equilibrium HEM does not model the delayed flashing that sets the real
    pressure undershoot.
  - No defect was found, so no code was changed.

**Which numbers to quote:** the §12 numbers. The §10/§11 figures describe
unapplied patches or the old fixture. For Edwards, quote the plateau and
RMSE and the sustained break flow; the single-sample peak break flow
should not be quoted.

## References

```bibtex
@book{toro2009riemann,
  title = {Riemann Solvers and Numerical Methods for Fluid Dynamics: A Practical Introduction},
  author = {Toro, Eleuterio F.}, edition = {3rd}, year = {2009}, publisher = {Springer},
  note = {Ch. 4, exact Riemann solver; Table 4.1 tests 1 (Sod) and 2}
}
@article{sod1978survey,
  title = {A survey of several finite difference methods for systems of nonlinear hyperbolic conservation laws},
  author = {Sod, Gary A.}, journal = {Journal of Computational Physics}, volume = {27}, number = {1},
  pages = {1--31}, year = {1978}, doi = {10.1016/0021-9991(78)90023-2}, note = {Table II}
}
@article{kurganov2001semidiscrete,
  title = {Semidiscrete central-upwind schemes for hyperbolic conservation laws and Hamilton--Jacobi equations},
  author = {Kurganov, A. and Noelle, S. and Petrova, G.}, journal = {SIAM J. Sci. Comput.},
  volume = {23}, number = {3}, pages = {707--740}, year = {2001}
}
@article{kieffer1977sound,
  title = {Sound speed in liquid-gas mixtures: water-air and water-steam},
  author = {Kieffer, S. W.}, journal = {J. Geophys. Res.}, volume = {82}, number = {20},
  pages = {2895--2904}, year = {1977}, note = {eq. 28, used by w_ph_eqm_hem}
}
@techreport{tomlinson_btm3271,
  title = {Edwards--O'Brien blowdown benchmark, RELAP5-3D nodalisation},
  author = {Tomlinson and Aumiller}, number = {B-T-3271},
  note = {Table 1 geometry and Hendrie profile; Figure 3 GS-1 data (digitised); as cited in tests/edwards_blowdown.rs}
}
```

The upstream code is OpenFOAM-10, commit `89f925dc`:
`applications/solvers/compressible/rhoCentralFoam/{rhoCentralFoam.C, centralCourantNo.H, setDeltaT.H}`.
It was read for this port. The SSP Runge–Kutta methods are `petir::ssprk`, a
port of PyClaw (gh #903).

## Reproduce

```bash
cargo test --release -p tampines-steam-tables --test hem_pipe_verification -- --nocapture
cargo test --release -p tampines-steam-tables --test hem_pipe_edwards -- --nocapture   # ~21 s
EDW_HEM_CELLS=96 cargo test --release -p tampines-steam-tables --test hem_pipe_edwards -- --nocapture   # ~3 min
cargo test --release -p tampines --test hem_coolprop -- --nocapture   # ~30 s
cargo test --release -p tampines-steam-tables --lib gh1102 -- --nocapture --include-ignored   # §9; the ignored one fails (gh:#1111)
```

The Edwards test also writes the gitignored
`verification_and_validation/hem_pipe_edwards_gs1.csv`: the full GS-1 series at
1 ms.
