# `raffles::fidelity`: one typed interface from a high-fidelity model to a surrogate (gh:#1194)

Part of epic gh:#1177. AI-assisted draft material under `RESPONSIBLE_USE.md`:
no human has reviewed it. Verification only ("is the interface implemented
correctly?"); nothing here validates a physical model.

## Question

Does the typed layer (uom at the API, SI `f64` inside) change any number the
existing surrogates produce, does the held-out split stay out of every fit,
and what does the first real implementer, the double-pipe pair of gh:#1178,
look like through it?

## What was built, and what was reused

- **Reused, unchanged:** `surrogate::PolynomialSurrogate::fit` / `predict` /
  `rmse`, `surrogate::validation::LeaveOneOut` (the LOO RMSE),
  `surrogate::neural::train` (the `Neural` route), `samplers::Sampler` (the
  design), `distributions::Distribution` (bounds and priors).
- **Added:** `surrogate::polynomial::MultiOutputPolynomial` (one scalar fit
  per output, so each column is the scalar fit bit for bit), and the
  `fidelity` module: `Quantities` + `quantities!`, `Channel`, `SiQuantity`,
  `ParameterSpace`, `HighFidelityModel`, `ParametricModel`, `Dataset`,
  `MultiFidelityDataset`, `SurrogateSpec`, `Surrogate`, `Prediction`,
  `ValidationReport`, `CorrectedModel`, `ThetaEstimate`,
  `discrepancy_dataset`, `build`, `sample`, `BuildPlan`, `HeldOut`. One new
  error variant, `RafflesError::ModelEvaluation`.
- **Searched first:** no shared surrogate type, no unit layer and nothing
  describing the high-fidelity side existed in `raffles` (issue body); the
  twin engine's `multifidelity::double_pipe` already had uom structs for
  x, y and θ, so they were wrapped with `quantities!` rather than duplicated.

## Methodology

All in release mode, `cargo test --release -p raffles --lib`, unless stated.
Each test's doc comment carries the same methodology, its prediction and its
result.

| Test | What | Pass criterion |
|---|---|---|
| `fidelity::quantities::non_si_construction_round_trips_exactly` | a record built in mm, °C (absolute `ThermodynamicTemperature`), bar | `from_si(to_si(q)) == q`; `to_si` equals uom's own `get::<meter>()`, `get::<kelvin>()`, `get::<pascal>()` bit for bit |
| `fidelity::build::the_typed_path_reproduces_the_recorded_ishigami_fit_bit_for_bit` | the exact 600 + 2000 LCG draws of `surrogate::polynomial`'s Ishigami test (seed 4242), typed as `Angle` → `Ratio`, through `Dataset` → `Surrogate::fit` → `ValidationReport::of`, degrees 2, 4, 6, ridge 1e-10 | coefficients `==` the direct fit; held-out RMSE `to_bits` equal to `PolynomialSurrogate::rmse`; RMSE rounds to the values recorded on 2026-09-16 |
| `fidelity::build::build_matches_the_direct_fit_bit_for_bit` | `build` on a typed Ishigami `HighFidelityModel`, Monte Carlo design (2600 rows, seed 4242) through `U(−π, π)`, last 2000 held out, degree 6 | surrogate, held-out RMSE, max error and LOO RMSE bit-identical to direct calls on the same rows |
| `fidelity::build::poisoned_held_out_outputs_do_not_change_the_fit` | LHS, 200 rows, every 4th held out, degree 4; held-out outputs replaced by NaN, +1e300, −1e300 | the fit `==` the clean fit; the report shows the poison |
| `fidelity::models::a_discrepancy_surrogate_corrects_the_low_fidelity_model` | HF = 2a + a², LF = θa at θ = 2, δ degree 2 | corrected model exact at held-out rows; with no δ it is the LF model bit for bit |
| twin engine `multifidelity::double_pipe::fidelity` tests | see "Double pipe" below | as stated there |

## Predictions (written before the runs)

Ishigami and poisoning predictions are in the tests' doc comments, written
before the first run:

- **P1** typed path = direct fit, bit for bit, at all three degrees (the layer
  only copies `value`).
- **P2** `build` = direct calls, bit for bit; held-out RMSE near 0.6
  (within ±0.15 of the legacy draw's 0.6079); **LOO RMSE a little above the
  held-out RMSE** (LOO models see 599 points).
- **P3** poisoned held-out outputs leave the fit identical.
- **P4** the round trip is exact.

### Double pipe (predictions written 2026-11-04, before running either test)

The first real implementer is
`outram_park_digital_twin_engine::multifidelity::double_pipe::fidelity`:
`TuasOneD: ParametricModel` (the TUAS 1-D model, `one_d::try_evaluate`) and
`committed_dataset()`, the 44 committed CFD rows of gh:#1178 as a
`Dataset<DoublePipeInput, DoublePipeOutput>` with the stored split (36
training, 8 held-out) and `dataset::PROVENANCE`.

- **P5** `CorrectedModel { TuasOneD, θ nominal, no δ }` reproduces
  `one_d::evaluate(&x, &Theta::nominal())` bit for bit at the 8 held-out
  points (no arithmetic on the no-δ path).
- **P6** A polynomial surrogate of the CFD outputs fitted to the 36 training
  rows, in the raw inputs (Re_hot, Re_cold, k_wall, L), at two settings fixed
  now and not revisited: **degree 1, ridge 0** (5 terms, identifiable), and
  **degree 2, ridge 1e-8** (15 terms; on the 2-level k_wall and L factors the
  squares are collinear with the constant once scaled to [−1, 1], so the ridge
  is what makes it solvable, and the fit is linear in k_wall and L whatever
  the degree). Expected held-out errors, relative = RMSE / mean held-out
  value:
  - **UA:** 10–30 % at both degrees. Six of the eight held-out points sit at
    k_wall = 10 W/(m K), where UA has nearly saturated toward its k_wall = 100
    value, while a fit that is linear in k_wall between 1 and 100 puts it 9 %
    of the way along. The degree cannot fix this.
  - **q:** similar to UA, 5–20 %.
  - **effectiveness:** RMSE 0.03–0.08 (degree 1), 0.02–0.06 (degree 2).
  - **outlet temperatures:** RMSE 1–4 K (degree 1), 0.5–3 K (degree 2).
  - **Δp (both sides):** degree 1 20–40 % (laminar Δp ∝ Re L is a product,
    which a linear fit cannot express); degree 2 under 10 % (it carries the
    Re·L cross term).
  - **Nu (both sides):** 5–20 % at both degrees.
  - **LOO RMSE > held-out RMSE** for most outputs at degree 2: leaving out a
    corner of a 3×3×2×2 grid forces an extrapolation, while the held-out
    points are interior.

## Results (2026-11-04)

### Ishigami, round trip, poisoning

| | Prediction | Measured | Verdict |
|---|---|---|---|
| P1 | bit-identical | coefficients `==`, RMSE `to_bits` equal at degrees 2, 4, 6; RMSE 3.3331, 1.9718, 0.6079 (as recorded 2026-09-16); max abs error 12.4143, 8.2290, 4.5333 | met |
| P2 | bit-identical; RMSE 0.6 ± 0.15; LOO above held-out | bit-identical surrogate, held-out RMSE, max error, LOO RMSE; held-out RMSE 0.6218, max error 3.6508, **LOO RMSE 0.5526** | bits and RMSE met; **LOO-above-held-out missed** (LOO 11 % below). Both estimate the same error from different finite samples (600 LOO residuals, 2000 fresh points), and that spread allows it; nothing was changed in response |
| P3 | identical fit | identical for NaN, +1e300, −1e300; reports read NaN and > 1e299 | met |
| P4 | exact | exact (`to_bits`) on all five channels | met |
| discrepancy | exact to round-off | held-out error < 1e-12 m; no-δ path `==` | met |

### Double pipe

Run with `cargo test --release -p outram-park-digital-twin-engine --lib
double_pipe::fidelity -- --nocapture` (4 tests, 32 s).

**P5, met.** `CorrectedModel { TuasOneD, θ nominal, no δ }` equals
`one_d::evaluate(&x, &Theta::nominal())` bit for bit (`to_bits`) on all 9
output channels at all 8 held-out points.

**P6: the held-out report, as measured** (relative = held-out RMSE / mean
|y| over the 8 held-out rows; 36 training rows; LOO on the training rows):

| channel | prediction | degree 1, ridge 0: RMSE (rel.) / max / LOO | degree 2, ridge 1e-8: RMSE (rel.) / max / LOO | verdict |
|---|---|---|---|---|
| UA [W/K] | 10–30 % both | 0.1328 (11.1 %) / 0.1746 / 0.0859 | 0.5988 (50.0 %) / 0.7050 / 0.0525 | d1 met; d2 missed |
| Q [W] | 5–20 % | 7.349 (25.1 %) / 11.92 / 7.916 | 16.66 (56.8 %) / 20.75 / 4.969 | missed both |
| ε [-] | 0.03–0.08 / 0.02–0.06 | 0.0880 / 0.1555 / 0.1668 | 0.2525 / 0.3637 / 0.0909 | missed both (d1 narrowly) |
| T_hot,out [K] | 1–4 / 0.5–3 | 3.503 / 5.412 / 5.629 | 126.6 / 144.5 / 2.838 | d1 met; d2 missed |
| T_cold,out [K] | 1–4 / 0.5–3 | 3.489 / 5.553 / 7.027 | 125.5 / 145.1 / 2.399 | d1 met; d2 missed |
| Δp_hot [Pa] | 20–40 % / < 10 % | 0.0311 (11.3 %) / 0.0755 / 0.1323 | 0.1370 (49.8 %) / 0.1545 / 7.1e-6 | d1 better than predicted; d2 missed |
| Δp_cold [Pa] | 20–40 % / < 10 % | 0.0430 (12.8 %) / 0.1047 / 0.1806 | 0.1779 (53.0 %) / 0.2005 / 3.3e-7 | d1 better than predicted; d2 missed |
| Nu_hot [-] | 5–20 % | 0.2694 (5.2 %) / 0.3560 / 0.3985 | 1.999 (38.3 %) / 2.442 / 0.1809 | d1 met; d2 missed |
| Nu_cold [-] | 5–20 % | 0.1512 (2.3 %) / 0.2367 / 0.3058 | 2.580 (39.0 %) / 3.070 / 0.1615 | d1 better; d2 missed |
| LOO vs held-out at d2 | LOO > held-out | | LOO far **below** held-out on every channel | missed |

**Which numbers to quote:** degree 1's. It is the only setting whose basis is
identifiable on this design.

**Why degree 2 fails, and why its LOO hides it.** The training grid has only
two levels of k_wall (1, 100) and L (0.1, 0.3). Scaled to [−1, 1] by
`PolynomialSurrogate`, k² and L² are 1 at every training row: they are the
constant column again. The ridge (1e-8) makes the normal equations solvable by
choosing the minimum-norm split of the constant across the three columns, and
at an interior point (k² ≈ 0.67 at k_wall 10, L² = 0 at L 0.2) part of the
constant vanishes: hence 126 K of error on a 300–350 K outlet. Every LOO point
is a grid node, where the split is harmless, so degree 2's LOO looks
excellent. **On this design an in-grid LOO is not evidence for accuracy between
the nodes; only the held-out split, fixed before the CFD ran, exposed it.**
That is the held-out split doing its job. The prediction that degree 2 would
carry Δp ∝ Re·L to under 10 % missed because the predicted mechanism was
right but the collinear squares swamp it.

Nothing was tuned in response. What would address it (not done here, and to
be fixed before running, not after): a basis without the pure squares of
two-level factors, log-transformed inputs (the closure table of gh:#1178 is
log-multilinear for this reason), or a third level of k_wall and L in the
design.


## What this does not show

- ~~No Gaussian process, polynomial chaos or co-kriging route exists yet, so no
  route reports a standard deviation, and `coverage_2sigma` is exercised only
  by a unit test of its helper on hand-computed rows.~~ **CORRECTED
  2026-11-04:** RAVEN's polynomial chaos and GP routes landed with gh:#1193,
  and SMT's KRG/KPLS/KPLSK and MFK co-kriging with gh:#1190; the GP routes
  report a standard deviation and `coverage_2sigma` is exercised on real fits
  (`raven_routes_gh1193.md`, `smt_kriging_gh1190.md`).
- The neural route is plumbing-checked only:
  `fidelity::build::the_neural_route_runs_through_the_same_interface`
  (`--features burn`, LHS 300 rows, every 5th held out, 600 epochs, seed 7)
  gave held-out RMSE 0.6367 against the mean predictor's 3.5808 on
  2026-11-04; its only gate is beating the mean predictor. (The same
  `--features burn` run has one failure, pre-existing and unrelated:
  `gnn::training::tests::message_passing_reach_helps_until_optimisation_becomes_the_limit`,
  gh:#1197.)
- Android and wasm: `cargo check --release -p raffles --all-targets --target
  aarch64-linux-android` and `cargo check --release -p raffles --lib --target
  wasm32-unknown-unknown` were clean on 2026-11-04 (default features).
- Nothing here validates the double-pipe models: the CFD dataset is synthetic
  model output (gh:#1178's record), and the polynomial is a surrogate *of* the
  CFD, not of reality.
