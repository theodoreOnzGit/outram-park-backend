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
| completes 0.6 s | yes, 4014 steps, 20.6 s | yes, 12602 steps, 175 s | yes |
| GS-1 plateau, mean 0.02–0.06 s | 364.9 psia | 372.1 psia | 359.0 psia |
| GS-1 RMSE vs data, 16 points, 0–0.30 s | 44.1 psia | 45.2 psia | 42.8 psia |
| mass conservation incl. break outflow, max rel. | 4.2e-15 | 3.4e-15 | — |

GS-1 pressure (psia), 24 cells:

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

**Compressed-liquid water: does NOT work, a pre-existing coolprop defect.**
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
  upstream source in this change.

**Single-phase water shock tube, IF97 vs CoolProp (cross-code, not
validation).** Because of the liquid defect this is run in superheated steam:
- 500 K, 0.2 | 0.1 MPa, 40 cells, t = 0.5 ms;
- 36 steps each; star pressure 0.139940 MPa (IF97) vs 0.139938 (CoolProp);
- max |Δp| = 8.7 Pa = **0.009 %** of the jump;
- CoolProp took 28 s, IF97 0.12 s.

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
```

The Edwards test also writes the gitignored
`verification_and_validation/hem_pipe_edwards_gs1.csv`: the full GS-1 series at
1 ms.
