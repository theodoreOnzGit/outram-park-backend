# TUAS off `peroxide` and `roots`, onto PETIR (gh:#989, gh:#991)

> Research, education and V&V only. Not for facility operation, licensing or
> safety decisions (see `RESPONSIBLE_USE.md`). AI-assisted work, not yet
> human-reviewed.

## What changed

| replaced | sites | now | adapter |
|---|---|---|---|
| `peroxide` `erfc` | 2 test files | `petir::specfunc::erfc` (GSL) | — |
| `peroxide` `integrate(.., GaussLegendre(20))` | custom liquid, custom solid | `petir::integration::gauss_legendre`, n = 20 | `numerical_methods::gauss_legendre_20` |
| `peroxide` `bisection!` / `BisectionMethod` (100 iterations, `\|f\| < 1e-8`) | Dowtherm A, FLiBe, FLiNaK, HITEC, YD-325, Churchill, custom fLDK, custom liquid, custom solid | `petir::roots::BracketingSolver` (GSL bisection) | `numerical_methods::bisection_residual` |
| `peroxide` `CubicSpline` `from_nodes` / `eval` / `integrate` | copper, SS-304L, fiberglass, Pyrogel HPS, nuclear graphite (19 tables) | `petir::interp::Interp` (GSL natural cubic spline) and the new `Interp::eval_integ` | `numerical_methods::NaturalCubicSpline` |
| `peroxide` `Algorithm::max/min` on `Vec<f64>` | parallel super-collection | the same fold, inlined | `fold_max` / `fold_min` |
| `peroxide` `c!` macro | property tables | a crate-local `c!` with the same comma form | `lib.rs` |
| `roots` `find_root_brent` / `find_root_regula_falsi` / `find_root_inverse_quadratic` with `SimpleConvergency` | solid T-from-h inversions, flow-network solvers, gFHR and CIET isothermal tests | `petir::roots::BracketingSolver` (GSL Brent, GSL false position) | `numerical_methods::{brent_root, false_position_root, bracketed_root}` |

`peroxide` and `roots` are gone from `Cargo.toml`, and so is the wasm
`getrandom` shim: after the change `cargo tree --target wasm32-unknown-unknown
-i getrandom@0.3.4` listed TUAS itself as the only dependant.

`src/lib/boussinesq_thermophysical_properties/solid_database/fecral.rs` and
`generic_heating_element.rs` still name `peroxide` and `roots`. Neither is in
the module tree (`solid_database/mod.rs` declares neither), and
`generic_heating_element.rs` would not compile if it were, so they were left
alone and are reported rather than converted blind.

## How each convergence test was matched (read from upstream source)

- **Bisection.** peroxide 0.41.2 `src/numerical/root.rs` lines 547-589: endpoint
  residual checks, then up to `max_iter` midpoints `c = (a+b)/2`, returning `c`
  as soon as `|f(c)| < tol`. GSL bisection probes the same midpoints and keeps
  the same half; it lacks only the residual test, which the adapter applies to
  each probe. Measured: **bit-identical** to peroxide on five pinned cases
  (`numerical_methods::tests::bisection_residual_reproduces_peroxide_bit_for_bit`,
  roots of `x^3 - 2x - 5` printed by peroxide before it was removed).
- **Brent / regula falsi / inverse quadratic.** `roots` 0.0.8 accepts when
  `|f| < eps` at an endpoint or iterate, or when the bracket is narrower than
  `eps`; `max_iter` caps it. The adapter keeps all three tests and the cap. The
  ALGORITHMS differ (GSL's Brent and false position, not `roots`' textbook
  Brent and Illinois regula falsi; `find_root_inverse_quadratic` has no GSL
  counterpart and maps to GSL Brent), so roots agree to `eps`, not bit for bit.
- **Gauss-Legendre 20.** Same rule, nodes and weights (PETIR's tables were
  ported from peroxide 0.41.2; the six corrected values are at orders 11 and
  12), same affine map and summation order.

## Out-of-range behaviour, site by site

peroxide's `CubicSpline::eval` extrapolated with the end polynomial, and its
`integrate` (`src/numerical/spline.rs` lines 671-690) silently **clipped** the
interval to the table, and returned **0** for a reversed interval whose limits
lie in different segments. PETIR refuses outside the table. Nothing is clamped
or extrapolated now; out of range is `TuasLibError::CorrelationRangeError`.

| function | table | range check before it | out of table reachable? | now |
|---|---|---|---|---|
| copper cp, k | 200-1000 K, 250-1000 K | identical range | no | error mapped (unreachable) |
| `copper_specific_enthalpy` | 200-1000 K, from 273.15 K | **none** (bare value) | yes, T < 200 or > 1000 K | **signature now `Result`**, error |
| SS-304L cp, k | 250-1000 K | identical | no | error mapped |
| `steel_304_l_spline_specific_enthalpy_ciet_zweibaum` | 250-1000 K, from 273.15 K | **none** | yes | **signature now `Result`**, error |
| fiberglass k | 250-600 K | identical | no | error mapped |
| Pyrogel k | 0-650 degC | identical | no | error mapped |
| Pyrogel cp low-T | 9.883-39.819 degC | **9.88**-39.819 degC | **yes, [9.88, 9.883) degC** | error |
| Pyrogel DSC | 35.426-326.294 degC | **35-327** degC | **yes, [35, 35.426) and (326.294, 327] degC** | error |
| graphite cp | 300-2000 K | identical | no | error mapped |
| `nuclear_graphite_specific_enthalpy` | 300-2000 K, from ~~**273.15 K**~~ **300 K** (since the maintainer decision below) | none | ~~**always: 273.15 K is below the table**~~ below 300 K or above 2000 K | **signature now `Result`**; ~~every call errors~~ errors only outside the table |
| T-from-h inversions (copper, steel, fiberglass, Pyrogel, graphite) | enthalpy at the table knots | none | yes, h outside the knot enthalpies | error (already `Result`); Brent failure that panicked (steel) or `unwrap`ped (copper, fiberglass, Pyrogel) now returns the error |

**FINDING — graphite** (**RESOLVED 2026-10-20**, see "Maintainer decisions"
below). The graphite cp table starts at 300 K but its enthalpy
was referenced to 273.15 K. peroxide integrated `[273.15, T]` by clipping it to
`[300, T]` without saying so (measured: its integral over `[273.15, 300]` was
exactly 0 J/kg), so graphite enthalpy has in fact always been referenced to
300 K. On PETIR every call is refused. Not fixed here; widening the table or
moving the reference is the maintainer's decision.

**FINDING — copper below 250 K.** peroxide returned exactly 0 J/kg for every
copper temperature in [200, 250) K (the reversed-interval bug above). PETIR
gives the integral: -26 973.11 J/kg at 200 K, -9 093.94 J/kg at 249 K.

## Round-off movement of the splines themselves

Measured 2026-10-20 before peroxide was removed, peroxide 0.41.2 against the
PETIR spline on 401 evenly spaced points per table (and the integral from the
reference temperature to each point where one is used):

| table | eval bit-identical | worst rel. diff | integral bit-identical | worst rel. diff |
|---|---|---|---|---|
| copper cp | 81/401 | 6.1e-16 | 111/364 | 5.3e-15 |
| copper k | 112/401 | 1.6e-14 | — | — |
| SS-304L cp | 64/401 | 1.0e-15 | 149/388 | 9.9e-15 |
| SS-304L k | 150/401 | 1.0e-15 | — | — |
| fiberglass k | 31/401 | 1.5e-15 | — | — |
| graphite cp | 29/401 | 4.4e-15 | 42/401 (from 300 K) | 2.9e-15 |
| Pyrogel k | 57/401 | 2.1e-15 | — | — |
| Pyrogel cp low-T | 171/401 | 2.6e-16 | — | — |

## Predictions written before the post-change suite run (2026-10-20 16:48 +08)

1. Liquid T-from-h (bisection), Churchill and custom fLDK Reynolds inversions,
   custom-material enthalpy: **no movement** (bit-identical adapters).
2. Spline properties: relative movement 1e-16 to 1e-14 (table above).
3. Brent and false position on GSL: each solve moves by less than its `eps`
   (1e-8 K for temperatures, 1e-8 kg/s or 1e-8 in residual units for
   flowrates). In the transient CIET and DRACS tests this compounds over
   thousands of steps; predicted movement of printed temperatures **below
   1e-6 K** and of mass flowrates **below 1e-7 relative**, sign unpredictable
   (it depends on which side each solve stops). No asserted V&V tolerance
   (0.2 degC heater outlet, 0.02 Nusselt, ~6 % natural circulation) should be
   approached.
4. Every test that evaluates nuclear graphite enthalpy fails with
   `CorrelationRangeError` (the graphite finding).
5. Steel and copper above 1000 K now error where peroxide clipped; predicted
   not to be reached by CIET (below ~ 420 K) or the gFHR pipe tests.

## Results (2026-10-20, x86_64 Linux, `--release`, long tests on)

Suite: `cargo test -p tuas_boussinesq_solver --lib --tests --release`
(test binaries run with `--test-threads` 5 before, 6 after; core budget).

| | passed | failed | ignored | wall time |
|---|---|---|---|---|
| base commit `38ce4d6c59` | 338 | 0 | 46 | 2877 s |
| after (this change, 5 new adapter tests) | 340 | **3** | 46 | 1205 s |

The three failures, each a finding and none loosened:

1. `nuclear_graphite_enthalpy_round_trip`: `CorrelationRangeError`, value
   273.15 K, table 300-2000 K. **Predicted** (prediction 4).
2. `dracs_natural_circ_thermal_hydraulics_regression_test_prototype_2`:
   pinned 0.010905593698135391 kg/s at `epsilon = 1e-7`, now
   0.010904398377899532 kg/s (-1.195e-6 kg/s, -1.1e-4 relative). **Not
   predicted**: prediction 3 said below 1e-7 relative. Attribution: tightening
   the flow-network solvers' `eps` from 1e-8/1e-9 to 1e-11 (scratch build, not
   committed) moves it again, to 0.010902201662272466 kg/s. The pin is tighter
   than the result's own sensitivity to root-finder tolerance.
3. `test_fhr_four_branch_solver_debug`: its debug path asks for `eps` = 1e-15,
   then 1e-12 Pa absolute on a pressure near -47001 Pa (one ulp = 7.3e-12 Pa).
   GSL's Brent and false position close the bracket to one ulp and stop there,
   with no endpoint residual below `eps`, so it reaches its
   `todo!("all root finding methods used not successful")`. On the base commit
   `roots`' inverse-quadratic solver passed through its residual test. **Not
   predicted.** The tolerance is below float resolution; not changed here.

### Movements of recorded V&V numbers (same tests run on both binaries)

| test | quantity | before | after | change |
|---|---|---|---|---|
| heater v1 steady state (maturity bar) | heater top-head exit | 86.8437030725699 degC | 86.8437030725699 degC | **0, bit-identical** |
| heater v1 transient step-down, Fig. 2.21 (bar) | heater top-head exit | 96.9271732918404 degC | 96.92717329184029 degC | -1.1e-13 K |
| heater v1 transient step-up, Fig. 2.21 (bar) | heater top-head exit | 97.81451482142683 degC | 97.81451482142688 degC | +5e-14 K |
| `tutorial_6` (custom graphite, GL20 + bisection) | outlet temperature | 1274.3878112984705 K | 1274.3878112984698 K | -7e-13 K (round-off; not bit-identical, so prediction 1 failed at round-off) |
| coupled DRACS set A1 | DRACS flow | 0.03414121757927805 kg/s | 0.03414122259728258 kg/s | +1.5e-7 rel. |
| | primary flow | 0.02767611317031267 kg/s | 0.02767611426455287 kg/s | +4.0e-8 rel. |
| | heater mean surface | 115.5736303006309 degC | 115.57363734823446 degC | +7.0e-6 K |
| coupled DRACS set B1 | DRACS flow | 0.02174745953183892 kg/s | 0.021747517661714625 kg/s | +2.7e-6 rel. |
| | primary flow | 0.018110326876947132 kg/s | 0.018110339045489327 kg/s | +6.7e-7 rel. |
| | heater mean surface | 70.05526595459622 degC | 70.05531604145011 degC | +5.0e-5 K |

Prediction 3 (temperatures below 1e-6 K, flows below 1e-7 relative in the
coupled loops) is **refuted** for the natural-circulation loops: they moved
up to 5e-5 K and 2.7e-6 relative. Every coupled DRACS set still passes its
experimental tolerance (6.1-6.2 %), and the heater bar did not move. The
size is consistent with the measured tolerance sensitivity above: the
coupled loops carry `eps`-level differences from every flow solve through
thousands of time steps of buoyancy feedback.

Only sets A1 and B1 were re-run pairwise for numbers; the other 25 coupled
sets passed in the full run but their printed values were not compared.

### Other checks

- PETIR's five build checks: `cargo test -p petir --release`, thumbv7em,
  wasm32, Android `--all-targets` and `--no-default-features` all build.
  `tests/verbatim_provenance.rs` fails on `linalg/square_matrix.rs` and
  `poly/cubic_eqn.rs`, whose `outram-foam-basic-lib` sources the gh:#985
  sweep changed to `r_*` calls. That is not part of this change.
- TUAS builds for `wasm32-unknown-unknown` (`--lib`) without the getrandom
  shim, and `cargo check --all-targets --target aarch64-linux-android` passes.
- Dependants: `tampines`, `tampines-steam-tables`, `teh-o-prke` and
  `outram-park-digital-twin-engine` pass `cargo check --lib --tests`, and so
  do digital-twin-engine's examples. `tampines`' `fhr_sim_v2` example fails
  to compile on `Quantity::r_exp/r_sin/r_cos`, which has nothing to do with TUAS.
- **Downstream consequence of the graphite finding:** `htgr_sim_v1`
  (`outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/decay_heat_removal.rs`)
  takes the reflector enthalpy of `NuclearGraphiteIG110` through `try_get_h`
  and panics on error, so it will panic at run time until the graphite
  reference is resolved. Not run (htgr validation re-runs are on hold).

## Maintainer decisions (2026-10-20) and the final run

The maintainer accepted the validation numbers above and decided the three
open items. All three were applied on the same day.

1. **Graphite: the datum is 300 K** (option a). The datum is now
   `GRAPHITE_ENTHALPY_REFERENCE_TEMPERATURE_KELVIN = 300.0`, the cp table's
   lowest knot. This is a documented datum, not a clamp: below 300 K and
   above 2000 K the function still errors. It reproduces what peroxide
   computed, which clipped [273.15, T] to [300, T]. The match is **within
   spline/quadrature round-off, not bit-identical**. Measured before peroxide
   was removed, the integral from 300 K to 401 points of the table was
   bit-identical at 42/401, with a worst relative difference of 2.9e-15.
   Results:
   - **`nuclear_graphite_enthalpy_round_trip` passes.** The worst round-trip
     error is 2.6147972675971687e-12 K, the same worst case as on the base
     commit.
   - **The htgr_sim_v1 reflector path no longer errors.** That path is
     `try_get_h(Material::Solid(NuclearGraphiteIG110), T, 101 325 Pa)`. Its
     own test, `the_reflector_and_rpv_capacities_are_derived`, could not be
     run: the htgr_sim_v1 *test* target does not compile on develop, failing
     on `r_sin`/`r_exp` imports in `app/state.rs`, `steam_generator.rs` and
     others (from the gh:#985 sweep, unrelated). So a scratch call was used
     instead (run, then removed). It gave:
     - h(300 K) = 0
     - h(600 K) = 327 307.27 J/kg
     - h(950 K) = 882 321.55 J/kg
     - h(2000 K) = 2 894 449.78 J/kg
     - 299 K and 2001 K: `CorrelationRangeError`
2. **`dracs_..._regression_test_prototype_2` is re-pinned.** The old pin was
   ~~0.010905593698135391 kg/s, epsilon 1e-7~~. The new pin is
   **0.010904398377899532 kg/s, epsilon 3e-6 kg/s**. The tolerance is the
   measured sensitivity to the flow solvers' tolerance (2.197e-6 kg/s,
   between the default eps and 1e-11), rounded up to one significant figure.
   The derivation is in the test's doc comment.
3. **`test_fhr_four_branch_solver_debug` now uses a relative tolerance.** The
   debugging solves use 4 × `f64::EPSILON` (8.9e-16), replacing ~~1e-15 then
   1e-12 Pa absolute~~. They go through the new
   `numerical_methods::relative_bracketed_root`, which is GSL's interval test
   with `epsabs = 0`. A one-ulp bracket has a relative width between
   EPSILON/2 and EPSILON, so the tolerance must exceed EPSILON to be
   reachable. Four times EPSILON accepts a two-ulp bracket. The derivation is
   at `PRESSURE_RELATIVE_TOLERANCE` in `multi_branch_solvers.rs`. The first
   Brent attempt now succeeds.

**Final run**, after merging `origin/develop` (#991, #992, #993, #995):
`cargo test -p tuas_boussinesq_solver --lib --tests --release`, long tests
on, `--test-threads=6`: **343 passed, 0 failed, 46 ignored**, in 826 s.
