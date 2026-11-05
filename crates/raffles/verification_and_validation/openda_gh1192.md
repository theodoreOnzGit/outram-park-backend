# gh:#1192: OpenDA's data-assimilation and calibration algorithms in `raffles`: differential V&V

**Status: AI-assisted draft, not reviewed by a human.** This record was
written in two stages: sections 1 to 4 (upstream, references, findings and
predictions) were committed **before any port code existed**; section 5
(results) was added after the port ran. Nothing in section 4 was edited after
the results were seen.

## 1. Upstream

| | |
|---|---|
| Project | OpenDA (open data assimilation toolbox), OpenDA Association |
| Repository | <https://github.com/OpenDA-Association/OpenDA> |
| Commit | `e51565c2c8def7d35f8b378a9ac118edbd98a94d` (2026-10-23 13:53 +0200), accessed 2026-11-04 |
| Licence | **LGPL-3.0-or-later**: `license.txt` is the verbatim GNU LGPL v3 (sha256 in §1.1), and every ported file's own header says "either version 3 of the License, or (at your option) any later version". Copyright "(c) 2012 OpenDA Association" (some files 2008, 2014, 2018) |
| Second upstream | **JAMA** 1.0.2 (NIST and The MathWorks), `Jama/SingularValueDecomposition.java`, `Jama/util/Maths.java`, `Jama/Matrix.times`: **public domain**. OpenDA's `Matrix.inverse()`, `sqrt()`, `svd()` and `rightSolve()` all go through JAMA's SVD, so its rounding order is part of OpenDA's |

### 1.1 Licence facts checked

- `license.txt` at the commit: GNU LESSER GENERAL PUBLIC LICENSE Version 3,
  29 June 2007; preserved verbatim as `crates/raffles/LICENSE-LGPL-3.0-OPENDA`.
- LGPL-3.0 is GPLv3 plus additional permissions (its §0 and the LGPL's own
  text), so LGPL-3.0-or-later code may be conveyed under GPLv3 and therefore
  combined into this AGPL-3.0-only crate (GPLv3 §13 / AGPLv3 §13), the same
  route the crate already takes for its GPL-3.0 ports. The ported files keep
  their LGPL-3.0-or-later notice in their header.
- JAMA's jar vendored by OpenDA (`core/java/resources/jama/Jama-1.0.2.jar`,
  sha256 `03f7d0c1…cd1fb8`) is not byte-identical to Maven Central's
  `gov.nist.math:jama:1.0.2` jar, so the sources were checked against the
  bytecode: after removing constant-pool indices, `javap -c -p` of
  `SingularValueDecomposition` and `util/Maths` is **identical** to the
  1.0.2 sources compiled with javac 21; `Matrix` differs only in
  javac-version artefacts (`invokevirtual` vs `invokespecial` for a private
  method; string concatenation), none in arithmetic.

## 2. How the references were made

`openda_gh1192/generate_references.sh` (recorded run 2026-11-04, OpenJDK
21.0.12.1, x86_64):

1. compiles OpenDA's `core`, `models`, `observers` and `algorithms` Java
   sources (448 files) with plain `javac` against the jars OpenDA vendors (no
   Ant or Maven was needed: it compiled without error);
2. compiles and runs `openda_gh1192/driver/.../Gh1192Driver.java`, which
   lives in OpenDA's `kalmanFilter` package and **adds no algorithm**: it
   - replaces the `java.util.Random` behind `StochVector` (state, forcing,
     parameter and observation noise), `ParticleFilter` (resampling) and
     `AutoLocalizationZhang2011` (bootstrap) by a recording subclass with the
     same seed, so every draw OpenDA consumed is written out;
   - captures the forecast ensemble, the gain before and after localisation,
     and the analysis ensemble at every analysis time (subclasses that call
     `super` and change no arithmetic);
   - reads each optimiser's full evaluation history (`allPars`, `allCosts`,
     `allPreds`) from the cost function.

**Provenance check, met:** the driver reproduces the value OpenDA's own
`KalmanFilterTest.testEnKF` asserts (N 100, seed 1234567890, through the
test's CSV round trip) **bit for bit**: `[-0.2315739613743379,
0.028205645374715847]`. The optimiser cases reproduce the values
`CoreOptimizerTest` asserts as strings (e.g. Simplex `[0.96148681640625,
0.9224853515625]`, Powell `[0.9999999999999999,1.0]`, DUD with background
`[0.5,0.5000000000000002]`).

Reference files (`openda_gh1192/references/`):

| file | contents |
|---|---|
| `openda_linalg.json` | JAMA SVD (U, s, V), OpenDA `svd`, `inverse`, `sqrt`, `rightSolve`, `A·Aᵀ` on 6 fixed matrices (2×2 to 5×3); `Math.pow(x, 2.0)` vs `x*x` and `Math.exp` vs `StrictMath.exp` on 200 000 samples; 2 000 `Math.exp` values |
| `openda_calibration.json` | Simplex, Powell and DUD on OpenDA's test cost functions (`SimpleCostFunction`, `RosenbrockCostFunction`, `SimpleLeastSquaresCostFunction` with and without the background term), and DUD, Simplex and Powell calibrating the oscillator's `(t_damp, omega)` through `SimulationKwadraticCostFunction` (OpenDA's weighted cost) on OpenDA's fixture `oscillatorCalibrationObservations.csv` (copied verbatim to `openda_gh1192/fixtures/`), with and without the background term: optimum, every evaluated parameter vector, cost and prediction, in order |
| `openda_sequential.json` | on the oscillator with `KalmanFilterTest`'s configuration and its noise-free observations (t = 0, 1, …, 10, σ 0.1): EnKF (N 10) with localisation none / Hamill / autoZhang, EnKF with stochastic parameters, EnSR (N 10), the particle filter (N 20), the steady-state filter (gain saved by the EnKF at t = 5); EnKF (N 10) on Lorenz-63 (x, y, z observed every 0.5, σ 1); per analysis time the forecast and analysis ensembles and gains; every Gaussian, uniform and bootstrap integer draw; 200 replicates each of EnKF, EnSR and PF at seeds 1–200 (statistical side) |

Hamill localisation needs a model-supplied mask, and OpenDA's oscillator
returns all ones (so Hamill would be a no-op). The driver's
`LocalisedOscillatorFactory` returns the fixed mask ρ = (1.0, 0.4) instead;
this is driver input, not OpenDA code.

## 3. Upstream findings (read before porting; to be ported faithfully)

1. **Java arithmetic is plain sequential IEEE**: no FMA contraction (the JLS
   forbids it), loops in source order. `Matrix.multiply` sums each element
   in one chain `sum += A(i,k)·B(k,j)` and then does `C *= facThis; C +=
   facAB·sum`; `rightMultiply` accumulates `alpha·x + Σ beta·L(i,j)·v(j)`
   left to right. Rust without `mul_add` reproduces this.
2. **`Matrix.inverse()` is not LU**: it is JAMA's SVD with singular values
   below `1e-7` (absolute) zeroed: `V·diag(1/s)·Uᵀ`. `sqrt()` is
   `U·diag(√s)·Vᵀ`, and `rightSolve` (DUD's least-squares step) is
   `(yᵀ U diag(1/s) Vᵀ)ᵀ`, through `Jama.Matrix.times` (one chain per
   element, column-copied). A "better" solver would change every number.
3. **`Math.pow(x, 2.0)` equals `x*x`** on all 200 000 samples (measured);
   the port writes `x*x`. **`Math.exp` (the HotSpot intrinsic) differs from
   `StrictMath.exp` (fdlibm) on 19 083 of 200 000 arguments** (9.5 %,
   measured): the particle filter's weights use `Math.exp`, so they are only
   as reproducible as `exp` is between the JVM and the Rust target's libm.
4. **EnKF** (`EnKF.java`): the gain is computed column by column,
   `K_i = Σ_j E(j,i)·X_j` with `E = PREDᵀ·inv(D)`, where the state
   deviations are first scaled by `1/√(q−1)` and afterwards scaled back by
   `√(q−1)`, **in place**. The round trip does not restore the deviations
   bit for bit; later uses see the rounded values. Each member's innovation
   is `obs.getRealizations() − pred_i' − pred_mean` (perturbed observations,
   one draw per observation per member, in member order).
5. **Localisation options** are `none`, `hamill` (Schur product of each
   gain column with a **model-supplied** mask `getObservedLocalization`) and
   `autoZhang` (Zhang & Oliver 2011, 50 bootstrap gains). In
   `AutoLocalizationZhang2011`:
   - each bootstrap member is `X_r + X_r`, i.e. **twice** the deviation (the
     commented-out lines added the mean);
   - `EnsembleVectors(…)` then removes the mean and sums the variance **from
     member 1**, skipping member 0 (`for (int i=1; …)`), so bootstrap member 0
     keeps the mean;
   - ρ = 1/(1 + (1 + 1/σ²_α)·C²) with σ²_α = 0.36, matching the paper's
     formula (the code comment says `1/σ²_α` without the 1 +);
   - the bootstrap draws `nextInt(n)` from its own static `Random`.
6. **EnSR** (`EnSR.java`) uses Sakov's symmetric transform: SVD of
   `I + ψᵀψ` with ψ = `R^{−1/2}·HL` (note **plus**, while the comment says
   minus; plus is the correct analysis form), `T = U·D^{−1/2}·Vᵀ`. No
   localisation.
7. **The particle filter is residual resampling only** (`samplingMethod`
   other than "residual…" throws). Weights are `StochVector.evaluatePdf` of
   the predictions, a product of `√(0.5/π)·exp(−t²/2)` divided by the product
   of σ. `drawFromDiscreteDistribution` returns index **0** when the uniform
   is not below any cumulative weight (possible only through rounding of the
   last cumulative sum). Members are copied by `restoreInternalState`, so a
   resampled member is an exact copy.
8. **Steady-state filter**: `x_a = x_f + Σ_i (y_i − h_i(x_f))·K_i` with a
   stored gain; columns matched to observations by `id:offset-in-seconds`;
   observations with |innovation| > `skipAssimilationStandardDeviationFactor`·σ
   are skipped. The XML gain storage is lossless here (same digits as the
   EnKF's gain at t = 5).
9. **Simplex** (`SimplexCoreOptimizer`): `initialize` evaluates the n + 1
   vertices but its `sortSimplexByCost` sorts only a **local copy** of the
   costs, so the first iteration runs on an **unsorted** simplex. The
   centroid is `Σ_all/n` (the worst vertex included), corrected inside the
   reflection coefficients. Ties are sorted stably (`Collections.sort`).
   The optimum reported is the best point the **cost function** has ever
   seen, not the simplex's.
10. **Powell** (`PowellCoreOptimizer`): Numerical Recipes' `powell` with a
    bracketing step and a Brent line search whose stopping test is on the
    change in f, not in x. `LineSearch` evaluates the base point again, and
    **rescales the search direction in place** by max(|α|, 0.1), so the
    stored directions change every call. The direction-replacement test
    uses `fExtrapolate > f_p` where NR uses `f_prev`.
11. **DUD** (`BaseDudCoreOptimizer`, `DudCoreOptimizer`): the linearised
    step solves `A·pStep = rhs` through the SVD `rightSolve`, in the basis of
    the stored parameter differences; the step is capped at `maxStep` in the
    ∞-norm; backtracking multiplies by `innerScaleFac` (0.5) and, at inner
    iteration `minInnerNegativeLook + 2`, flips its sign **on the object**, so
    every later outer iteration backtracks in the negative direction too.
    `isThereMoreToDo` returns true whenever an inner iteration happened and
    `imain < maxit`. Constraints (`add_constraints`, an active-set QP) are
    not ported (§6).
12. **The weighted cost** (`SimulationKwadraticCostFunction`):
    `J = factor·(‖(y − h(p))/σ‖₂)² + factor·(‖L_p⁻¹(p − p̄)‖₂)²` with
    `factor = 0.5`, each square taken as `Math.pow(norm2, 2.0)` (a square
    root then a square, not the raw sum). Parameters are clamped at 0 by the
    oscillator's `setParameters` (`max(value, 0)`).
13. Not an algorithm, recorded only: `CircularAdvectionInstance`'s
    Gaspari–Cohn half-width is `Math.sqrt(10 / 3)`, integer division, i.e.
    √3 rather than √(10/3).

### 3.1 Found while porting (after §4 was written; ported faithfully)

14. **`StochVector.evaluatePdf`, independent branch, multiplies by Πσ
    instead of dividing**: it divides `sqrtDetCov` by each σ and then
    returns `prob / sqrtDetCov`. The particle filter only uses ratios of
    weights with one σ vector, so its resampling is unaffected; a caller
    using the value as a density gets it wrong by Πσ².
15. **The weighted cost drops the observation at the model start time**:
    `createSelection(getTimeHorizon())` keeps times in `(t_start, t_stop]`
    (`Time.inSpan`'s strict lower bound), so the oscillator calibration
    uses 10 of the fixture's 11 observations (confirmed: 10 predictions per
    evaluation in the reference).
16. **The oscillator and Lorenz models redeclare `initialStateUncertainty`**,
    shadowing the field `getStateUncertainty()` returns: without an
    `initialStateUncertainty` configuration entry the model has **no** state
    uncertainty, whatever its constructor sets. Likewise a configured
    `parameters` entry without `parameterUncertainty` leaves the uncertainty
    centred on the constructor's defaults.
17. **The Simplex error estimate collapses for a small simplex**: it
    inverts AᵀA with `Matrix.inverse()`, whose singular-value cut-off is
    **absolute** (1e-7), so once the simplex has shrunk far enough the
    estimate becomes 0 (seen in a unit test at tolerances 1e-14; with
    OpenDA's defaults it is (1, 2) on the simple least-squares case, as the
    reference records).

## 4. Predictions (written before the port)

### Differential (bit-identical unless stated)

| id | case | prediction |
|---|---|---|
| D1 | JAMA SVD U, s, V on the 6 matrices | bit-identical |
| D2 | OpenDA `inverse`, `sqrt`, `svd`, `rightSolve`, `A·Aᵀ` | bit-identical |
| D3 | Rust `f64::exp` against the 2 000 `Math.exp` values | **not all identical**: at most 1 % differ, each by 1 ulp |
| D4 | Simplex on simple, Rosenbrock, simple LS: every evaluation (parameters and cost) and the optimum | bit-identical, same evaluation counts (25, 90, 22) |
| D5 | Powell on simple, Rosenbrock, simple with initial directions | bit-identical, same counts (47, 116, 230) |
| D6 | DUD on simple LS with and without background, incl. the error estimate | bit-identical |
| D7 | oscillator calibration, DUD / Simplex / Powell × background off / on, every evaluation and prediction | bit-identical |
| D8 | EnKF none / Hamill / autoZhang / stochastic parameters, OpenDA's draws injected: every forecast and analysis member at every time, the gains | bit-identical |
| D9 | EnSR, draws injected | bit-identical |
| D10 | particle filter, draws injected | states bit-identical (resampling indices identical); if D3's exp differences reach a weight, the weights may differ by an ulp but no index changes |
| D11 | steady-state filter with the stored gain | bit-identical |
| D12 | Lorenz-63 EnKF, draws injected | bit-identical |
| D13 | **statistical**, own draws (the workspace LCG, Box–Muller) vs OpenDA's 200 replicates, EnKF, EnSR (N 10), PF (N 20), final state: criterion fixed now: for each component, \|Δ mean\| / √(s²₁/200 + s²₂/200) < 3 and the sd ratio in [0.8, 1.25] | pass, all three |

### Analytic

| id | check | prediction |
|---|---|---|
| A1 | DUD recovers (a, b) of the linear model y = a + b·t (t = 0…9, noise-free, σ 0.1, start (0, 0), unit search steps) | to 1e-12 relative after its first outer iteration (a linear model makes the secant step exact) |
| A2 | Simplex, same problem, tolerances 1e-14, maxit 10 000 | within 1e-5 of (a, b) |
| A3 | Powell, same problem | within 1e-5 of (a, b) |
| A4 | EnKF → KF: the oscillator is linear (RK4 is a linear map F; noise √dt·w per substep gives Q_k), so the exact KF exists. Ensemble mean error \|x̄ − x_KF\|/σ_KF at t = 10, averaged over 10 seeds, for N = 25, 100, 400, 1600 | decreases monotonically and is < 0.1 at N 1600; ensemble variance within 10 % of the KF's at N 1600 |
| A5 | EnSR → KF, same | < 0.1 at N 1600 |

### Pilot (twin engine, separate record)

Written before the pilot code in
`crates/outram-park-digital-twin-engine/verification_and_validation/double_pipe_openda_gh1192.md`.

## 5. Results (2026-11-04)

Commands: `cargo test --release -p raffles --test openda_gh1192 -- --nocapture`
(all six tests, 70 s, dominated by A5's EnSR at N 400).

### Differential

| id | prediction | measured |
|---|---|---|
| D1 | JAMA SVD bit-identical | **bit-identical** (U, s, V of all 6 matrices) |
| D2 | OpenDA kernels bit-identical | **bit-identical**; D1 + D2 together **402/402** |
| D3 | ≤ 1 % of `exp` differ, by 1 ulp | **met**: 6 of 2 000 differ, each by 1 ulp (glibc `exp` vs the JVM's intrinsic) |
| D4 | Simplex bit-identical, 25 / 90 / 22 evaluations | **met**, same counts |
| D5 | Powell bit-identical, 47 / 116 / 230 | **met**, same counts |
| D6 | DUD bit-identical with the error estimate | **met** |
| D7 | oscillator calibration ×6 bit-identical (every parameter, cost and prediction) | **met**: DUD 8 / 4, Simplex 22 / 25, Powell 35 / 34 evaluations, no-background / background |
| D4–D7 total | | **3 330/3 330 bit-identical** |
| D8 | EnKF none / Hamill / Zhang / stochastic parameters, draws injected | **met**: every member, gain and main state |
| D9 | EnSR | **met** |
| D10 | PF states bit-identical | **met**; the D3 `exp` differences did not reach these weights |
| D11 | steady-state filter | **met** |
| D12 | Lorenz-63 EnKF | **met** |
| D8–D12 total | | **4 281/4 281 bit-identical**, and every recorded draw consumed, no more (4 130, 4 130, 4 130 + 5 500 integers, 4 150, 4 020, 8 040 + 53 uniforms, 0, 6 330) |
| D13 | statistical pass (criterion fixed first) | **pass, all six** components: |

| D13 | x[0] \|Δmean\|/se, sd ratio | x[1] \|Δmean\|/se, sd ratio |
|---|---|---|
| EnKF (N 10) | 0.97, 1.055 | 1.21, 1.105 |
| EnSR (N 10) | 0.38, 0.926 | 0.27, 1.149 |
| PF (N 20) | 0.35, 0.967 | 0.40, 1.051 |

**No residual needs a human judgement:** every deterministic comparison and
every injected-draw comparison is bit-identical, so the 1e-8 allowance was
not used.

### Analytic

| id | prediction | measured |
|---|---|---|
| A1 | DUD exact to 1e-12 at its first trial | **met**: max relative error 2.7e-15, 4 evaluations in all |
| A2 | Simplex within 1e-5 | **met**: 2.9e-9 after 69 iterations |
| A3 | Powell within 1e-5 | **MISSED**: stopped after 2 iterations at (0.579, 0.816), error 1.42 |
| A4 | EnKF → KF monotone, < 0.1 at N 1600, variance within 10 % | **met**: 0.145, 0.069, 0.054, **0.0165** σ_KF at N 25, 100, 400, 1600; variance ratio (0.998, 1.010) |
| A5 | EnSR < 0.1 at N 1600 | **not tested as predicted**: OpenDA's EnSR does a q×q SVD per analysis; 10 seeds at N 400 took 66 s, so N 1600 (≈ 64×) was not run. At N 25, 100, 400: 0.138, 0.056, 0.0525 σ_KF, reported without a gate |

**A3, the miss.** The exact KF at t = 10 is x = (−0.22801, −0.01798),
σ = (0.0956, 0.371). On the straight line, OpenDA's Powell (bit-identical
to OpenDA on the six D5/D7 cases, so taken to be OpenDA's behaviour; the
line problem itself was not run in Java) makes one good sweep, then in
iteration 2 both Brent line searches return their base point, the change
in J is exactly 0, and the outer test `|ΔJ| < absTol` stops it. **Post-hoc
diagnosis (run after the miss, not a prediction):** with Brent's tolerances
tightened from OpenDA's 0.01 / 0.001 to 1e-14, the same code reaches
(2.0, 0.49999999999999994) in 3 iterations. The cause is OpenDA's Brent
stopping on a relative change in f between successive trial points with no
x-tolerance. The test keeps no accuracy gate on A3; it pins only the
post-hoc check. Not changed in the port: it is upstream's behaviour.

### Reading the results

- On the same inputs and draws, the port reproduces OpenDA's sequential
  filters and optimisers exactly, including their quirks (§3), and its own
  draws reproduce OpenDA's statistics (D13).
- OpenDA's Powell with its default line-search tolerances can stall on an
  ill-conditioned quadratic; DUD is exact on a linear model; Simplex is
  slow but gets there.
- The EnKF's error falls by factors 2.1, 1.3 and 3.3 per four-fold N;
  1/√N would give 2 each. With 10 seeds per N the steps are noisy; the
  rate itself was not a prediction and is not claimed.

## 6. Not ported (with reason)

- the Java framework: XML configuration, `IStochModelFactory`/instance
  wrappers, result writers, restart files, timers, parallel model runs;
- DUD's constraint handling (`add_constraints`, an active-set QP);
  `SparseDud`; `DudEnKF`/`DudENSR` (iterative ensemble filters); the other
  optimisers in OpenDA (BFGS, conjugate gradient, LM, SCE, GLUE, DELSA);
- the EnKF's gain smoother in time (`timeRegularisationPerDay`) and
  restart of a smoothed gain; the steady-state filter's
  `estimateMissingObservations` (HK compensation) and gain replacement in
  time;
- the cost function's bias and standard-deviation removal, which are
  specific to per-station time series exchange items;
- stop criteria other than the optimisers' own tolerances.
