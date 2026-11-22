# teh-o-prke vs GeN-Foam: feedback sum and PRKE step (gh:#1439, epic #1336)

> Research, education and V&V only. Not for facility operation, licensing or any
> safety decision (`RESPONSIBLE_USE.md`). AI draft, unreviewed.

**Run 2026-11-13.** Upstream: GeN-Foam `652b3da` (GPL-3.0, EPFL) on ESI OpenFOAM v2506,
the unmodified `solvers::pointKineticNeutronics` class from `libNeutronicsSolvers.so`.
Predictions: [`PREDICTIONS.md`](PREDICTIONS.md), committed before any run.
Test: `tests/genfoam_pointkinetics_vs_upstream.rs` (6 tests, all pass, numbers pinned).
Reproduce: `scripts/build_and_run_pkdump.sh` (a second run reproduced
`refs/pkdump.ref.txt` byte for byte).

## Methodology

`upstream_driver/pkDump/pkDump.C` derives from the real class (its members are `protected`)
and does not modify upstream:

* **F (feedback algebra, fixed inputs).** A member function `#include`s upstream's own
  `include/correctReactivity.H` verbatim, with the temperatures and density as locals, so
  only the feedback algebra runs (no mesh averaging). Four cases: F1 = the coefficient set of
  `Tutorials/reactorCases/2D_FFTF/.../nuclearData` (Doppler 5e-3, fuel -4e-6, clad -1e-6,
  coolant density 1.5e-5, structure -1e-5, driveline 7e-5 m/K and its 7-point rod-worth map),
  400 steps of a sinusoidal history including a fuel temperature below `SMALL` and one exactly
  at the reference; F1D = the same, rod-worth map reversed; F2 and F2D = a dyadic set
  (driveline coefficient 0.5 m/K, map knots at multiples of 1/8 m) with the driveline
  expansion placed exactly on the knots and on both map ends, and inside and outside the map.
  830 rows.
* **K (kinetics step).** The real `correctPhysics()` on the FFTF `neutroRegion` mesh (a real
  `fvMesh`, `Time`, 8 delayed groups, `Lambda = 5e-7 s`), 300 steps with `dt` swept
  1e-3 to 1e-1 s, feedback fields set per step. Each step prints upstream's total reactivity,
  `dt`, and the state before and after. This crate then restarts from **upstream's** state
  before each step, so error does not accumulate and every step is its own comparison.
* Inputs are printed with 17 significant digits, so the Rust side parses back the same `f64`.
  Driveline: `L = 1 m`, `alpha = c_dl`, so `L alpha` is upstream's `c_dl` exactly.

Not run, as the issue suggests, "GeN-Foam's own pointKinetics tutorial": the tutorials
(`2D_FFTF`, `1D_MSR_pointKinetics`) need the coupled fluid region and hours of run time,
and the comparison that matters here (the algebra) does not need them. The ESFR case the issue
names (`3D_SmallESFR` point-kinetics variant) does not exist in this tree:
`3D_SmallESFR_old` carries no `feedbackCoeff*` keys; `2D_FFTF` is the fast-reactor
point-kinetics tutorial.

## Results

### Feedback terms (F), this crate with `RodWorthOutOfRange::UpstreamZero`

| case | rows | linear terms (fuel, clad, coolant T, coolant rho) | structure | driveline | Doppler | total |
|---|---:|---|---|---|---|---|
| F1 | 400 | 400/400 bit-identical, each | 399 bit-identical + 1 sign-of-zero | 400/400 | **399/400, worst 2 ulp** | 400/400 |
| F1D | 400 | 400/400 each | 400 sign-of-zero only | 400/400 | **399/400, worst 2 ulp** | 400/400 |
| F2 | 15 | 15/15 each | 15 sign-of-zero only | 15/15 | 15/15 | 15/15 |
| F2D | 15 | 15/15 each | 15 sign-of-zero only | 15/15 | 15/15 | 15/15 |

* **Every term is equal as a number on all 830 rows except one Doppler row per history.**
  The totals are bit-identical on all 830 (the one Doppler difference is absorbed in the sum).
* **Doppler mismatch, localised.** One row (`T = 912.7070374357462 K`, ratio `0.9127...`):
  this crate `4.5670164460333637e-4`, upstream `4.567016446033365e-4`, 2 ulp. First divergent
  operation: `ln`. PETIR `r_ln(x)` differs from glibc `ln(x)` by **1 ulp** at that ratio (1 of
  400 ratios; std `ln` reproduces upstream's term exactly), and `-K * ln` carries that into
  2 ulp of the term. The 1-ulp libm difference is the accepted class (maintainer 2026-10-20) and
  is pinned (`petir_ln_is_the_doppler_mismatch`); the 2-ulp term is a consequence of the
  multiply, not a second error.
* **Sign of zero (radial structure).** Where `T_struct = T_ref`, upstream's term is `-0`
  (`coeff * (+0)` with a negative coefficient) and this crate's is `+0`: the radial-expansion
  fold starts from `Ratio::ZERO`, and `+0 + -0 = +0`. Equal as numbers, 0 ulp, harmless in the
  total (bit-identical on every row). Counted separately and pinned, not changed.
* **Driveline** is bit-identical on all 830 rows in `UpstreamZero` mode, including both map
  orientations, knots, both map ends and outside the map (zero).
* **Default `Error` policy** (maintainer decision 2026-11-10), 830 rows: 742 bit-identical to
  upstream, 2 at the end upstream excludes (upstream 0, here the tabulated end value: the
  documented difference), 86 outside the closed range (here `Err`, upstream 0).

### PRKE step (K), one backward-Euler step from upstream's state, 300 steps, 8 groups

| route | `P` bit-identical | worst rel. `P` | `C_i` bit-identical | worst rel. `C_i` |
|---|---:|---|---:|---|
| `PointKinetics::step_implicit` (default, unchanged) | 6 / 300 | 1.4e-12 (step 51) | 761 / 2400 | 6.4e-15 |
| upstream's unscaled rows + this crate's LU | 5 / 300 | 1.2e-12 (step 151) | 896 / 2400 | 4.8e-15 |
| upstream's unscaled rows + OpenFOAM `solve()` (translated) | **300 / 300** | 0 | **2400 / 2400** | 0 |
| **`PointKinetics::step_implicit_genfoam_form`** (new) | **300 / 300** | 0 | **2400 / 2400** | 0 |

Initial precursor state (`(beta_i P)/(lambda_i Lambda)` upstream, `set_critical_equilibrium`
here): 8 / 8 bit-identical.

**First divergent operation.** Not the matrix assembly (my prediction P5 named the row scaling
by `dt`, and was wrong about the cause): the second row shows that upstream's exact rows with
this crate's solver are still 5/300. It is the **solver**. GeN-Foam's `solvePointKinetics.H`
calls `solve(x, A, B)`, which in ESI v2506
(`src/OpenFOAM/matrices/scalarMatrices/scalarMatricesTemplates.C`) is **Gaussian elimination
with partial pivoting and no row scaling**, not `LUsolve`. `src/matrix.rs` lifts
`LUDecompose`/`LUBacksubstitute` (Crout LU with implicit scaling). Two algorithms, same answer to
about 1e-12 on this stiff system (`dt/Lambda` up to 2e5), different bits. Translating `solve()`
(`SquareMatrix::solve_gaussian_openfoam`) and upstream's rows
(`step_implicit_genfoam_form`) gives 300/300 and 2400/2400.

`step_implicit` stays the default: it is this crate's original scaled form and is
pinned by `six_group_bit_identity_pin.rs`; the GeN-Foam form is an additional method, so
nothing existing changed. It also takes the subcritical index `zeta` and an external source
power, which `step_implicit` has no argument for.

## Predictions scored

| # | predicted | observed | verdict |
|---|---|---|---|
| P1 | linear terms 100 % bit-identical | 100 % (structure: equal as numbers, sign of zero differs) | right, with the zero-sign caveat I did not foresee |
| P2 | Doppler >= 99 % identical, worst 1 ulp | 99.75 %, **worst 2 ulp** (1 ulp in `ln`) | count right, ulp bound **wrong** |
| P3 | driveline 100 % with `UpstreamZero`; `Error` differs only at the excluded end and outside | right (742 / 2 / 86) | right |
| P4 | total bit-identical wherever every term is | bit-identical on all 830 | right (stronger) |
| P5 | step not bit-identical (below 50 %), `P` <= 1e-10, `C` <= 1e-11; cause: row scaling | 6/300 and 761/2400, 1.4e-12, 6.4e-15; **cause is the solver, not the scaling** | numbers right, **mechanism wrong** |
| P6 | initial precursors bit-identical | 8/8 | right |
| P7 | appbuilder closer to upstream than this crate | appbuilder has upstream's rows but this same LU: **equal to row 2 above**, no closer (static reading, see below) | **wrong** |

## Two ports of one upstream: do they agree? (duplication finding)

Read, not run (this task runs `teh-o-prke` tests only; the appbuilder port is not merged
with this crate, per the issue).

| | `teh-o-prke` | `outram-foam-appbuilder-lib` `genfoam` |
|---|---|---|
| step rows | scaled by `dt`, no `zeta`, source as a rate | upstream's rows exactly, with `zeta` and the source row |
| solver | Crout LU (inlined copy of the basic-lib type) | the same LU (basic-lib) |
| vs upstream (this run) | 6/300 bit-identical (default), 300/300 with the new method | by the measured row 2: 5/300, worst 1.2e-12 |
| Doppler | `-K_D ln(max(T,SMALL)/max(Tref,SMALL))`, upstream's | `-alpha T_ref ln(T/T_ref)`, a different parametrisation, no `SMALL` floor |
| other terms | fuel, clad, coolant T, coolant density, structure (one or many), driveline rod-worth map | `reactivity_feedback.rs`: Doppler, expansion, density over mesh fields with an importance weight; no driveline map; coefficients negated |

They **do not agree bit for bit with each other**, and neither is a copy of the other:
the appbuilder step is closer in assembly and this crate is closer in solver only after the
change made here. The mesh-integral half of `computeFeedbackFieldValues.H` (flux-squared
weighting) lives only in the appbuilder. **Recommendation, not done (deferred to the
maintainer):** make the appbuilder step call a translation of `solve()` (the basic-lib
`SquareMatrix` would need `solve_gaussian_openfoam`, the same code, so the inlined-copy rule
in `src/matrix.rs` applies) and take the driveline, Doppler-floor and rod-worth terms from this
crate's `fast_reactor` instead of re-deriving them. Merging the two is a design decision and
was not attempted.

## Gap survey (GeN-Foam `pointKinetics/`, updated)

| upstream item | status |
|---|---|
| `correctReactivity.H` feedback sum (Doppler, fuel, clad, coolant T, coolant density, structure, driveline) | ported (`fast_reactor.rs`), bit-identical but for PETIR `ln` (1 ulp) |
| `calcDrivelineReactivity` | ported (`RodWorthMap`), bit-identical in `UpstreamZero` |
| `solvePointKinetics.H` step (rows, `solve()`) | **now ported** (`step_implicit_genfoam_form`, `solve_gaussian_openfoam`); was the only real mismatch |
| thermal-spectrum Doppler `coeff (sqrt(T) - sqrt(Tref))` (`fastNeutrons false`) | **not ported**: `FastDopplerTerm` is the fast form only. Was not in the issue's survey ("no gaps"), found in `correctReactivity.H` |
| `TStructMech` term | not ported (documented in `fast_reactor.rs`: same linear form as a radial structure term) |
| `precEquilibriumReactivity`, `externalReactivityTimeProfile`, `boronReactivityTimeProfile` | not ported (caller adds them; no equivalent) |
| `calcGEMLevelAndReactivity` (gas expansion module) | not ported; upstream's own comment says GEM "does NOT work in GeN-Foam 2.0" |
| external-source modes (`transient`, `powerMonitoring`), `setBeamParameters` | not ported; `zeta` and `S` are in `step_implicit_genfoam_form` as arguments only |
| decay-power profile, `power_ = fission + decay` | partly: `point_reactor` has its own decay-heat model, not upstream's profile |
| `solvePointKineticsLiquidFuel.H` (precursor advection) | not ported here; `circulating.rs` is a different model (core/loop transit), not a translation |
| `computeFeedbackFieldValues.H` (flux-squared-weighted averages) | not ported here (appbuilder has a mesh version); a mesh integral |
| `maxDeltaT()` power-variation step control | not ported |

Findings carried to the issue: the "no gaps to file" line in the issue body is superseded by
the thermal-spectrum Doppler form, the liquid-fuel step and the source modes above; the
issue's case 2 (ESFR variant) names a case that does not exist in this tree.

## Doc claims corrected in this change

* `outram-foam-appbuilder-lib` `genfoam/neutronics/point_kinetics/mod.rs` (and its
  `docs/*-api.md` mirror, edited by hand to match, not regenerated): "the direct analogue
  of GeN-Foam's `scalarSquareMatrix` + `solve()`" struck through; it is Crout LU, upstream is
  Gaussian elimination.

## What remains

* The 2-ulp Doppler row is a PETIR `r_ln` 1-ulp difference (PETIR is another crate); recorded,
  not fixed.
* Liquid-fuel and thermal-Doppler paths, GEM, boron and external-source modes (above).
* The appbuilder step still uses the LU; changing it is outside this task's scope
  (its tests were not run).
* A coupled-tutorial run (full `GeN-Foam` on `2D_FFTF` against this crate in a loop) was not
  done: needs the TH region, and the algebra comparison above isolates the same operations.
