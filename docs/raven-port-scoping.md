# Scoping: porting RAVEN's uncertainty-quantification mathematics to Rust

Scoping document for `outram-park-fork-raven` — a Rust port of the
*mathematics* inside INL's RAVEN (Risk Analysis Virtual ENvironment):
probability distributions, sampling, statistics and sensitivity analysis,
polynomial-chaos surrogates and optimisation.

> **Intended use.** Education, research, capability building, and V&V only.
> RAVEN upstream is used at INL for probabilistic risk assessment; nothing in
> this port is for licensing, safety-critical decision-making, or operational
> deployment. See `RESPONSIBLE_USE.md`.
>
> **Status of this document.** This is a **scoping study, not a plan of record
> and not a port**. No crate exists, no code has been written. Every LOC figure
> and file path below was measured on 2026-08-06 against a scratch clone (see
> [§11 Provenance](#11-provenance)); nothing is estimated unless it says so.

## 0. Recommendation in one paragraph

**Start with distributions plus Monte Carlo and Latin hypercube sampling, and
nothing else.** It is the only tranche that is simultaneously self-contained
(no dependency on RAVEN's XML driver, entity factories, job handler, or data
objects), immediately useful on its own (every downstream UQ capability needs
it), and verifiable without running RAVEN at all — analytic moments,
closed-form CDF/PPF round-trips, and Kolmogorov–Smirnov against the
distribution's own CDF. Critically, RAVEN's 1-D distribution numerics were
**rewritten in pure Python over `scipy.stats` in 2023**, replacing the old
C++/Boost backend, so the port target is a small, readable, dependency-light
file rather than a SWIG-wrapped C++ layer. That single fact is what makes this
tranche cheap. Everything after it is materially more expensive, and the
honest framing of tranches 2 onward is in [§7](#7-tranches-and-their-verification-paths).

## 1. What RAVEN actually is

RAVEN is a **framework**, not a library. Its organising idea is an XML input
deck that instantiates entities (Samplers, Models, Distributions, Steps,
DataObjects, OutStreams) through factories, wires them together, and runs them
through a job handler that can drive external simulation codes. The
mathematics is real and good, but it is embedded in that machinery rather than
exposed as a callable numerical API.

This is the central scoping problem. A file-by-file translation of, say,
`Samplers/MonteCarlo.py` would produce Rust that manipulates
`self.inputInfo['ProbabilityWeight-' + key]` dictionaries — RAVEN's internal
transport format — and would be neither idiomatic nor usable. **The port must
re-express the mathematics against a Rust-native interface, using upstream as
the specification, not as a line-by-line source.**

### 1.1 Upstream totals — measured, not estimated

Python line counts, all `.py` files, at commit `01216937` (branch `devel`,
2026-07-14).

| Top-level directory | Files | Lines |
|---|---|---|
| `ravenframework/` — the framework | 547 | 130,852 |
| `tests/` | 476 | 33,427 |
| `scripts/` | 61 | 7,431 |
| `doc/` | 29 | 4,119 |
| `rook/` — the test runner | 13 | 3,396 |
| `developer_tools/` | 9 | 1,264 |
| `src/` — legacy C++/SWIG (Python side) | 4 | 1,284 |
| `plugins/` | 13 | 1,038 |

Repository size on disk after a full checkout: **828 MB** (GitHub reports 832,568 KB).
The bulk is not source — it is regression gold files, documentation images, and
git history.

Legacy C++ (`src/` and `crow/`), which is being retired — see [§2.1](#21-the-single-most-important-finding-the-c-backend-is-already-gone):

| Component | Lines |
|---|---|
| `src/distributions/*.cxx` | 2,633 |
| `crow/include/distributions/*.h` | 1,132 |
| `src/utilities/*.cxx` (ND interpolation, splines) | 1,810 |
| `src/AMSC/AMSC.cpp` (topological decomposition) | 1,633 |
| `src/crow_modules/*.i` (SWIG interfaces) | 58 |

### 1.2 `ravenframework/` subsystem breakdown — measured

| Subsystem | Files | Lines | In scope? |
|---|---|---|---|
| `SupervisedLearning/` | 118 | 24,906 | Partly — see [§3.5](#35-surrogate--reduced-order-models) |
| `Models/` (incl. `PostProcessors/`) | 51 | 17,111 | Only `BasicStatistics.py` |
| `CodeInterfaceClasses/` | 92 | 16,421 | **No** |
| `Optimizers/` | 45 | 9,551 | Partly, late |
| `Samplers/` | 23 | 9,358 | Partly — the maths, not the plumbing |
| `utils/` | 17 | 7,959 | Selectively |
| `TSA/` (time-series analysis) | 21 | 5,131 | **No** (this release) |
| `UI/` | 15 | 5,244 | **No** |
| `OutStreams/` | 19 | 3,999 | **No** |
| `contrib/` | 43 | 3,991 | **No** — vendored third-party |
| `DataObjects/` | 6 | 3,437 | **No** |
| `Metrics/` | 16 | 1,769 | A small slice, late |
| `Steps/` | 8 | 1,399 | **No** |
| `BaseClasses/` | 8 | 1,088 | **No** |
| `Runners/` | 9 | 981 | **No** |
| `Databases/` | 5 | 677 | **No** |
| `PluginBaseClasses/` | 7 | 480 | **No** |
| `CustomModes/`, `CustomDrivers/` | 7 | 1,117 | **No** |
| `CrossValidations/` | 4 | 239 | **No** — thin sklearn wrapper |
| `Decorators/` | 2 | 106 | **No** |

Loose top-level modules that matter:

| File | Lines | Note |
|---|---|---|
| `Distributions.py` | 4,194 | The distribution catalogue — but see [§3.1](#31-distributions-tranche-1) |
| `GridEntities.py` | 1,195 | Grid construction for grid/stratified samplers |
| `Distributions1D.py` | 854 | **The live 1-D numerics.** Pure Python over `scipy.stats` |
| `Quadratures.py` | 807 | Gauss rules, Clenshaw–Curtis, Smolyak sparse grid |
| `IndexSets.py` | 465 | Tensor-product, total-degree, hyperbolic-cross, adaptive |
| `OrthoPolynomials.py` | 421 | Legendre, Hermite, Laguerre, Jacobi |
| `DistributionsND.py` | 270 | Multivariate normal with PCA transform |
| `utils/randomUtils.py` | 565 | RNG abstraction and sampling primitives |
| `utils/mathUtils.py` | 1,280 | 55 functions, mixed quality — see [§3.6](#36-mathutils-cherry-pick-only) |

## 2. What is worth porting — findings against the task brief

All five candidate areas named in the brief **do** exist in RAVEN. Their
self-containedness varies enormously, and that variation should drive the
ordering.

### 2.1 The single most important finding: the C++ backend is already gone

RAVEN historically computed 1-D distributions in C++ (`crow`), wrapping
Boost.Math and exposing it to Python via SWIG. **That is no longer the live
path.** As of a 2023 rewrite (`Distributions1D.py`, header `Created on Oct 10,
2023`), every 1-D distribution is implemented in pure Python over
`scipy.stats`. Verified two ways:

- All 21 `self._distribution = ...` assignments for 1-D distributions in
  `Distributions.py` construct `Distributions1D.Basic*Distribution` objects.
  The only surviving `CrowDistribution1D` uses are the **N-dimensional**
  cases: `BasicMultiDimensionalInverseWeight` (line 3481), 
  `BasicMultiDimensionalCartesianSpline` (line 3661), and
  `BasicMultivariateNormal` (line 3927).
- `utils/randomUtils.py` line 33 states in a comment that the numpy and crow
  random environments produce identical output for the same seed, sets
  `stochasticEnv = 'numpy'`, and says **"crow will be removed"**.

**Consequence for the port.** Tranche 1 does not need to port C++, does not
need Boost, and does not need SWIG. It needs to reimplement roughly a dozen
`scipy.stats` distributions' PDF/CDF/PPF plus truncation renormalisation. That
is a well-understood, well-tested body of numerics with published reference
values, and it is why tranche 1 is the right starting point.

**Consequence for what we skip.** The N-D interpolated distributions
(`NDInverseWeight`, `NDCartesianSpline`) are still C++-backed and would drag in
a 1,810-line ND spline/interpolation layer. They are excluded from every
tranche below.

### 2.2 Samplers

| Sampler | File | Lines | Assessment |
|---|---|---|---|
| Monte Carlo | `Samplers/MonteCarlo.py` | 183 | **In, tranche 1.** The maths is `dist.rvs()` plus a probability-weight bookkeeping block |
| Stratified (Latin hypercube) | `Samplers/Stratified.py` | 280 | **In, tranche 1.** A random permutation per dimension over equiprobable strata |
| Grid | `Samplers/Grid.py` | 312 | **In, tranche 2.** Depends on `GridEntities.py` (1,195 lines), which is heavier than the sampler |
| Sparse-grid collocation | `Samplers/SparseGridCollocation.py` | 340 | **In, tranche 3.** Thin — the work is in `Quadratures.py` + `IndexSets.py` + `OrthoPolynomials.py` |
| Sobol (HDMR) | `Samplers/Sobol.py` | 237 | **In, tranche 3.** Depends on the above plus `GaussPolynomialRom` |
| Adaptive sparse grid | `Samplers/AdaptiveSparseGrid.py` | 684 | Tranche 4 at the earliest |
| Adaptive Sobol | `Samplers/AdaptiveSobol.py` | 937 | Tranche 4 at the earliest |
| Adaptive Monte Carlo | `Samplers/AdaptiveMonteCarlo.py` | 269 | Depends on `BasicStatistics` post-processor |
| Metropolis / adaptive Metropolis MCMC | `Samplers/MCMC/` | 1,094 | **In, tranche 5.** Not named in the brief but genuinely valuable for reactor inverse problems |
| Factorial design | `Samplers/FactorialDesign.py` | 178 | **Out.** Delegates entirely to `pyDOE3` |
| Response-surface design | `Samplers/ResponseSurfaceDesign.py` | 199 | **Out.** Delegates to `pyDOE3` |
| Dynamic event tree | `Samplers/DynamicEventTree.py` | 1,059 | **Out.** Requires the job handler and restartable external codes |
| Adaptive DET | `Samplers/AdaptiveDynamicEventTree.py` | 612 | **Out.** Same reason |
| Limit-surface search | `Samplers/LimitSurfaceSearch.py` | 848 | **Out** of this release. Needs a ROM plus the limit-surface post-processor |
| Custom / ensemble-forward | `CustomSampler.py`, `EnsembleForward.py` | 578 | **Out.** Pure orchestration |

**How much of a sampler file is mathematics?** Very little. Reading
`MonteCarlo.localGenerateInput` (lines 103–175): the actual sampling is one
call to `self.distDict[key].rvs()`, or for the uniform-sampling variant a
draw on $[\text{lower}, \text{upper}]$ followed by a CDF-difference weight.
The remaining ~60 lines manage
`variables2distributionsMapping`, `distributions2variablesMapping`,
`reducedDim`, comma-separated variable aliasing, and `inputInfo` population.
None of that survives contact with a Rust API.

The same holds for LHS. `Stratified.localInitialize` spends 45 of its lines on
grid-entity and multivariate-normal-transform bookkeeping; the LHS algorithm
itself is one line — `randomUtils.randomPermutation(range(pointByVar-1))` per
dimension.

**Read this as good news, not bad.** It means tranche 1's sampler work is
small. It also means the honest LOC-in / LOC-out ratio is nothing like 1:1,
and nobody should schedule this port by counting upstream lines.

### 2.3 Distributions

`Distributions.py` declares 26 classes. Continuous: Uniform, Normal, Gamma,
Beta, Triangular, Logistic, Laplace, Exponential, LogNormal, Weibull,
LogUniform, Custom1D. Discrete: Poisson, Binomial, Bernoulli, Geometric,
Categorical, UniformDiscrete, MarkovCategorical. N-dimensional:
`NDInverseWeight`, `NDCartesianSpline`, `MultivariateNormal`.

**42% of that file is not mathematics.** Measured by summing the line spans of
the XML/serialisation methods:

| Method | Lines |
|---|---|
| `getInputSpecification` | 633 |
| `_handleInput` | 550 |
| `getInitParams` | 314 |
| `_localSetState` | 211 |
| `initializeFromDict` | 40 |
| `__getstate__` / `__setstate__` | 28 |
| **Total** | **1,776 of 4,195** |

The real numerics live in `Distributions1D.py` (854 lines), which is small,
readable, and structured as a `ContinuousDistribution` / `DiscreteDistribution`
pair of base classes wrapping a `scipy.stats` frozen distribution with
truncation renormalisation:

$$\text{pdf}_{\text{trunc}}(x) = \frac{\text{pdf}(x)}{F(x_{\max}) - F(x_{\min})}, \quad x \in [x_{\min}, x_{\max}]$$

Two distributions — `LogNormal` and `Geometric` — are hand-implemented in
`Distributions1D.py` rather than delegated to `scipy.stats`, because RAVEN uses
a different parameterisation.

### 2.4 Sensitivity analysis

Split across three places:

- **`Models/PostProcessors/BasicStatistics.py`** (1,580 lines) — the workhorse.
  Offers 15 scalar metrics (`expectedValue`, `variance`, `sigma`, `median`,
  `percentile`, `skewness`, `kurtosis`, `variationCoefficient`, min/max,
  partial variances), 6 vector metrics (`sensitivity`, `covariance`,
  `pearson`, `spearman`, `NormalizedSensitivity`,
  `VarianceDependentSensitivity`), and — valuable and unusual — **standard
  errors** for 7 of them. Depends on `numpy`, `scipy.stats`, and `xarray`.
  The `xarray` dependency is presentational and does not need porting.
- **Sobol indices from polynomial chaos** —
  `SupervisedLearning/GaussPolynomialRom.py:613 getSensitivities()` computes
  Sobol indices *analytically* from the PCE coefficients, not by Monte Carlo
  estimation. This is the higher-value implementation, and it is cheap once the
  PCE exists.
- **`Models/PostProcessors/ImportanceRank.py`** (429 lines) — PCA-based
  importance ranking. Lower priority.

### 2.5 Surrogate / reduced-order models

`SupervisedLearning/` is 24,906 lines, and **most of it is not worth porting**:

| Component | Files | Lines | Assessment |
|---|---|---|---|
| `ScikitLearn/` wrappers | 73 | 7,984 | **Out.** Thin adapters around sklearn estimators. Porting these means porting sklearn |
| `KerasBase.py` + Keras classifiers/regressors | 4 | 2,745 | **Out.** TensorFlow |
| `ARMA.py` | 1 | 2,562 | **Out** this release. Time series |
| `ROMCollection.py` | 1 | 2,214 | **Out.** Orchestration (segmented/clustered/interpolated ROM assembly) |
| `DMD/` | 16 | 3,408 | **Out** this release. Would need SVD → BLAS |
| `GaussPolynomialRom.py` | 1 | 668 | **In, tranche 3.** Polynomial chaos. Deps: `numpy` + one `scipy.spatial` call |
| `HDMRRom.py` | 1 | 414 | **In, tranche 3.** Cut-HDMR / Sobol decomposition on top of the above |
| `MSR.py` | 1 | 640 | **Out.** Morse–Smale regression, needs the BSD-licensed AMSC C++ (1,633 lines) |
| `NDspline.py`, `NDinvDistWeight.py`, `NDinterpolatorRom.py` | 3 | 432 | **Out.** All three call the C++ `interpolationND` module |
| `PolyExponential.py`, `SyntheticHistory.py`, `MultiResolutionTSA.py`, `FeatureSelection/RFE.py` | 4 | 2,180 | **Out** this release |

**Gaussian process is not RAVEN's own.** `SupervisedLearning/ScikitLearn/GaussianProcess/GaussianProcessRegressor.py`
(401 lines) is a wrapper around `sklearn.gaussian_process`. RAVEN contains no
GP implementation to port. If the workspace wants GP surrogates, that is a
from-scratch implementation informed by the literature, **not** a RAVEN port —
and it should be scoped separately and honestly as such.

Net: of 24,906 lines in `SupervisedLearning/`, about **1,082** (GaussPolynomialRom +
HDMRRom) are genuine port candidates.

### 2.6 Optimisation

| Optimiser | Lines | Deps | Assessment |
|---|---|---|---|
| `GradientDescent.py` | 873 | numpy | **In, tranche 6.** Plus `gradients/` (FiniteDifference, CentralDifference, SPSA — 587 lines) and `stepManipulators/` (GradientHistory, ConjugateGradient — 984 lines) |
| `SimulatedAnnealing.py` | 779 | numpy, `pyDOE3.lhs` | **In, tranche 6.** Five cooling schedules; self-contained |
| `GeneticAlgorithm.py` | 1,484 | numpy, `scipy.special.comb`, xarray | **In, tranche 6**, but it is the largest single file in scope and pulls in 8 operator sub-packages (~1,300 lines) |
| `BayesianOptimizer.py` | 781 | **`scipy.optimize` + sklearn GP** | **Out.** Cannot be ported without first having a GP and a bounded optimiser. Revisit only after both exist |
| `RavenSampled.py` | 767 | — | Base class. Mostly orchestration; not ported directly |

`Optimizer(AdaptiveSampler)` — in RAVEN, **optimisers are samplers**. See
[§8](#8-design-rule-friction).

## 3. Detailed in-scope module notes

### 3.1 Distributions (tranche 1)

Port `Distributions1D.py`'s numerics, not `Distributions.py`'s catalogue class
hierarchy. The catalogue's job — parsing XML, validating parameters, tracking
truncation bounds — becomes Rust constructor arguments and a `Result`.

### 3.2 RNG (tranche 1, and a decision point)

`utils/randomUtils.py` wraps `numpy.random.Generator` (`NumpyRNG`, line 212).
NumPy's default bit generator is **PCG64**. The workspace has **no `rand`
crate at all** — `Cargo.toml` contains no `rand`, `rand_distr`, `rand_pcg`, or
`statrs`. What exists today is hand-rolled: `crates/boon-lay/.../oorandom_rng.rs`
(`OoRng64`, a PCG-family generator) and `crates/njoy-outram-park-fork/src/purr/mod.rs`
(`Rng`).

This is a real decision the maintainer must make, not something a port agent
should decide — see [§10](#10-open-questions-for-the-maintainer).

### 3.3 Grid entities (tranche 2)

`GridEntities.py` (1,195 lines) builds the value/CDF grids that Grid, Stratified
and the DET samplers consume. It supports both value-space and CDF-space grids
and global grids shared across correlated variables. It is larger than the
samplers that use it, and it is where the multivariate-normal PCA transform is
threaded through. Do not underestimate it.

### 3.4 Polynomial chaos (tranche 3)

Four upstream files form one coherent unit:

- `OrthoPolynomials.py` (421) — Legendre, Hermite (probabilists'), Laguerre,
  Jacobi, with norms and point-mapping to the standard domain.
- `Quadratures.py` (807) — `SparseGrid`, `TensorGrid`, `SmolyakSparseGrid`,
  and the matching quadrature sets, plus Clenshaw–Curtis and CDF-space
  variants.
- `IndexSets.py` (465) — `TensorProduct`, `TotalDegree`, `HyperbolicCross`,
  `Custom`, `AdaptiveSet`.
- `GaussPolynomialRom.py` (668) + `HDMRRom.py` (414) — the expansion itself,
  its moments, and analytic Sobol indices.

**This is the tranche with the largest hidden dependency.** Upstream delegates
the hard numerics to SciPy:

| Upstream call | What it needs in Rust |
|---|---|
| `scipy.special.eval_legendre`, `eval_hermitenorm`, `eval_genlaguerre`, `eval_jacobi` | Orthogonal polynomial evaluation — recurrence relations, straightforward |
| `scipy.special.orthogonal.p_roots`, `he_roots`, `la_roots`, `j_roots` | **Gauss quadrature nodes and weights.** The standard method is Golub–Welsch: symmetric tridiagonal eigenproblem |
| `scipy.fftpack.ifft` (Quadratures.py:724) | Inverse FFT, for Clenshaw–Curtis weights |
| `math.gamma` | Gamma function |

Golub–Welsch needs a symmetric tridiagonal eigensolver. **A general
`ndarray-linalg` dependency here would be Android-hostile and must be
avoided.** The correct approach is a self-contained implicit-QL-with-shifts
routine for symmetric tridiagonal matrices (the classic `TQLI`/`imtql2`
algorithm) — a few hundred lines of pure Rust, no BLAS, no LAPACK, Android-clean.
An alternative for Legendre and Hermite specifically is Newton iteration on the
polynomial with known asymptotic starting guesses, which avoids the eigenproblem
entirely. **Either way, this must not be resolved by reaching for BLAS.**

### 3.5 Basic statistics (tranche 2)

`BasicStatistics.py`'s scalar and vector metrics are ordinary weighted moment
and correlation calculations. The parts worth care:

- **Weighted** variants throughout — RAVEN carries per-sample probability
  weights from the sampler, so every statistic has a weighted form.
- **Standard errors** for 7 metrics. These are genuinely useful and are the
  part most worth getting right, because they are what makes a sampling result
  reportable.
- `spearman` requires rank transformation; `percentile` requires an
  interpolation convention that must be matched exactly or results will differ
  in the last digits.

### 3.6 `mathUtils` — cherry-pick only

55 functions, of wildly mixed relevance. Useful here: `normal`, `normalCdf`,
`partialDerivative`, `derivatives`, `relativeDiff`, `compareFloats`,
`numBinsDraconis`, `characterizeCDF`, `sampleCDF`, `sampleICDF`,
`gaussianize`, `degaussianize`. Not useful: the SVD/DMD helpers (BLAS-bound),
the numpy/list conversion helpers, `readVariableGroups` (XML), the type
predicates. **Do not port this file as a unit.** Take the ~12 functions the
in-scope tranches actually call.

## 4. What is explicitly NOT worth porting

Stated plainly, as the brief asks.

| Excluded | Lines | Why |
|---|---|---|
| **XML input driver** — `utils/InputData.py`, `InputTypes.py`, `xmlUtils.py`, plus every `getInputSpecification`/`_handleInput` in every entity | 2,191 in `utils/` alone, plus ~1,776 inside `Distributions.py` and comparable fractions elsewhere | Rust has types. An XML schema layer that exists to give a dynamically-typed language a validation story is dead weight in a language whose constructors already validate. This is the single largest exclusion and the main reason the port is far smaller than 130 kLOC |
| **Entity factories / plugin machinery** — `EntityFactoryBase.py`, `PluginManager.py`, `PluginBaseClasses/`, every `Factory.py` | ~1,200 | These exist to map XML strings to classes at runtime. Replaced by enums. See [§8](#8-design-rule-friction) |
| **External-code couplings** — `CodeInterfaceClasses/` | 16,421 | 26 interfaces (RELAP5, RELAP7, MELCOR, SCALE, SERPENT, PARCS, PHISICS, CobraTF, OpenFOAM, Dymola, MAAP5, SIMULATE3, Saphire, …). These write input decks and parse output files for specific external binaries. Some of those codes are export-controlled or licence-restricted; **the workspace should not be building couplings to them**, and per `RESPONSIBLE_USE.md` this is out of intended scope regardless |
| **MOOSE coupling** — `CodeInterfaceClasses/MooseBasedApp/` (10 files: MOOSE, BISON, CUBIT parsers and interfaces) | included above | Ties to a C++ framework the workspace does not use |
| **Job scheduling / parallelism** — `JobHandler.py`, `Runners/`, `CustomModes/`, `raven_qsub_command.py`, `RemoteNodeScripts/`, and the `ray` dependency | ~2,000 | Rust has `rayon` (already a workspace dependency). Porting a Python job handler built on `ray` and PBS/qsub would be actively harmful |
| **GUI** — `UI/` | 5,244 | PySide2/Qt5 + matplotlib. Also an Android-portability violation by construction |
| **Plotting / output streams** — `OutStreams/` | 3,999 | matplotlib. The workspace's plotting story is `outram-park-digital-twin-engine` |
| **DataObjects / Databases** — `DataObjects/`, `Databases/`, `h5py_interface_creator.py`, `CsvLoader.py` | ~4,300 | `xarray`/`pandas`/`h5py`/netCDF containers. Rust caller owns its own data |
| **`Steps/`, `Simulation.py`, `Driver.py`, `Application.py`** | ~2,500 | The XML-deck execution engine. This *is* the orchestration layer the brief says we do not need |
| **`contrib/`** | 3,991 | Vendored third-party Python |
| **`rook/`** | 3,396 | RAVEN's own test runner. We have `cargo test` |
| **`SupervisedLearning/ScikitLearn/`** | 7,984 | Wrappers, not implementations |
| **Keras/TensorFlow ROMs** | 2,745 | Would require a Rust deep-learning stack |
| **`MSR.py` + AMSC** | 640 Python + 1,633 C++ | Morse–Smale regression. Also the only BSD-licensed component (see [§9](#9-attribution-and-licence)) |
| **`TSA/`** | 5,131 | Time-series analysis (ARMA, VARMA, Fourier, wavelets, STL). Real value, but a different capability from UQ and depends on `statsmodels`. Scope separately if wanted |
| **`Metrics/`, `CrossValidations/`** | 2,008 | Predominantly `sklearn.metrics` / `scipy.spatial.distance` / `sklearn.model_selection` wrappers |
| **Dynamic event tree samplers** | 1,671 | Require restartable external codes driven by the job handler — the excluded layer |
| **`crow/` and `src/` C++** | ~7,200 | Being retired upstream ([§2.1](#21-the-single-most-important-finding-the-c-backend-is-already-gone)). Do not port a dead backend |

**Rough arithmetic.** Of `ravenframework`'s 130,852 lines, the in-scope
candidate set across all six tranches is on the order of **15,000–18,000 upstream
lines**, and after stripping XML boilerplate, factory machinery, and
`inputInfo` plumbing from those files, the mathematics being re-expressed is
smaller again. Do not read that as "cheap": the residue is dense numerics with
a high verification burden, which is where the real cost sits.

## 5. Dependency analysis

### 5.1 Upstream Python dependencies, by in-scope module

From `dependencies.xml`: numpy 1.26, scipy 1.12, scikit-learn 1.1, numba 0.61,
pandas, xarray, netcdf4 1.6, matplotlib 3.6, statsmodels 0.13, tensorflow 2.14,
h5py, lxml, psutil, pyDOE3, cloudpickle, ray, pyside2 (optional).

Usage counts across `ravenframework/` (files importing each): numpy 203,
sklearn 87, scipy 52, xarray 27, matplotlib 16, pandas 13, statsmodels 7,
tensorflow 4, ray 4, pyDOE 4, h5py 1.

| In-scope module | numpy | scipy | sklearn | other | Rust replacement |
|---|---|---|---|---|---|
| `Distributions1D.py` | yes | `scipy.stats` | no | — | Own PDF/CDF/PPF; `libm`-level special functions (erf, erfc, gamma, incomplete gamma, incomplete beta) |
| `DistributionsND.py` | yes | `scipy.stats` | no | — | MVN via Cholesky — **small dense Cholesky, hand-written, no BLAS** |
| `randomUtils.py` | `numpy.random` | no | no | — | `rand` + `rand_pcg`, or hand-rolled. See [§10](#10-open-questions-for-the-maintainer) |
| `Samplers/MonteCarlo.py` | yes | no | no | — | `ndarray` / plain `Vec` |
| `Samplers/Stratified.py` | yes | no | no | — | Fisher–Yates permutation |
| `Samplers/Grid.py`, `GridEntities.py` | yes | no | no | — | `ndarray` |
| `OrthoPolynomials.py` | yes | `scipy.special` | no | `math.gamma` | Recurrence relations + a gamma function |
| `Quadratures.py` | yes | `scipy.special.orthogonal`, `scipy.fftpack` | no | — | **Golub–Welsch or Newton iteration; a small real FFT.** See [§5.2](#52-the-blas-question) |
| `IndexSets.py` | yes | no | no | — | Pure combinatorics; trivial |
| `GaussPolynomialRom.py` | yes | `scipy.spatial` (one call) | no | — | `ndarray`; the `scipy.spatial` use is a nearest-point lookup, easily replaced |
| `HDMRRom.py` | yes | no | no | — | `ndarray` |
| `BasicStatistics.py` | yes | `scipy.stats` | no | **`xarray`** | `ndarray`; xarray is a container, not maths |
| `Optimizers/GradientDescent.py` | yes | no | no | — | `ndarray` |
| `Optimizers/SimulatedAnnealing.py` | yes | no | no | `pyDOE3.lhs` | Our own LHS from tranche 1 |
| `Optimizers/GeneticAlgorithm.py` | yes | `scipy.special.comb` | no | `xarray` | `ndarray`; `comb` is trivial |
| `Samplers/MCMC/` | yes | `scipy.stats` | no | — | Tranche-1 distributions |

### 5.2 The BLAS question — flagged as the brief requires

**No in-scope module requires BLAS/LAPACK, and none should acquire it.**
`ndarray-linalg` is Android-hostile and target-gated in this workspace; the
port must not introduce it. The three places where a careless implementation
would reach for it:

1. **Gauss quadrature nodes (Golub–Welsch)** — needs a symmetric *tridiagonal*
   eigensolver only. Implement `imtql2`-style implicit QL with shifts directly,
   or use Newton iteration on the polynomial. **Do not call a general
   `eig`/`eigh`.**
2. **Multivariate normal sampling** — needs a Cholesky factor of a small dense
   covariance matrix. Write the ~30-line Cholesky. **Do not call LAPACK
   `potrf`.**
3. **PCE coefficient solve** — RAVEN's PCE is *quadrature-based* (coefficients
   come from a sparse-grid integral), not regression-based, so there is **no
   least-squares solve** in `GaussPolynomialRom.py`. If someone later adds
   regression-based PCE, that is where a QR/SVD would sneak in, and it must be
   gated at that point.

Everything excluded from scope — DMD (SVD), sklearn ROMs, `MSR.py`,
`mathUtils`'s truncated-SVD helpers — is exactly the BLAS-hungry set. That
alignment is not a coincidence and is a further argument for the boundary drawn
here.

### 5.3 Android/Termux posture

The in-scope set is Android-clean by construction: no BLAS, no C/Fortran
toolchain, no windowing. The one item needing an explicit gate is any
`criterion` benchmark or example that gets added — per `CLAUDE.md`, examples
and benches are compiled by a native Termux build and are **not** exempt.
The proxy check is `cargo check -p outram-park-fork-raven --all-targets
--target aarch64-linux-android`; the authoritative check is a native Termux
build.

## 6. Proposed crate structure

Workspace convention (`docs/ecosystem-naming.md` and the members table in
`CLAUDE.md`) gives **`outram-park-fork-raven`**, GPL-3.0, described as an
independent fork, not affiliated with INL or Battelle Energy Alliance.

```
crates/outram-park-fork-raven/
  src/
    lib.rs                  //! module map; what belongs here and what does not
    distributions/
      mod.rs                //  enum Distribution — the dispatch point
      continuous/           //  uniform, normal, gamma, beta, triangular,
                            //  logistic, laplace, exponential, lognormal,
                            //  weibull, loguniform
      discrete/             //  poisson, binomial, bernoulli, geometric,
                            //  categorical, uniform_discrete
      truncation.rs         //  renormalisation shared by all continuous kinds
      special.rs            //  erf/erfc, gamma, incomplete gamma, incomplete beta
      multivariate.rs       //  MVN via hand-written Cholesky  (tranche 2)
    rng/
      mod.rs                //  enum RngKind + the sampling primitives
    sampling/
      mod.rs                //  enum Sampler
      monte_carlo.rs
      latin_hypercube.rs
      grid.rs               //  (tranche 2)
      grid_entities.rs      //  (tranche 2)
      sparse_grid.rs        //  (tranche 3)
      mcmc/                 //  (tranche 5)
    statistics/
      moments.rs            //  (tranche 2) weighted mean/var/skew/kurtosis
      standard_error.rs     //  (tranche 2)
      correlation.rs        //  (tranche 2) pearson, spearman, covariance
      sensitivity.rs        //  (tranche 2) regression + variance-dependent
    pce/                    //  (tranche 3)
      ortho_polynomials.rs
      quadrature.rs         //  Gauss rules + Clenshaw-Curtis, no BLAS
      index_sets.rs
      expansion.rs          //  the PCE itself, moments
      sobol.rs              //  analytic Sobol indices from PCE coefficients
      hdmr.rs
    optimization/           //  (tranche 6)
      mod.rs                //  enum Optimizer
      gradient_descent.rs
      simulated_annealing.rs
      genetic.rs
  examples/                 //  one runnable example per tranche
  docs/api.md               //  rustdoc mirror, per the bookkeeping rule
  README.md                 //  with the Bookkeeping status block, both axes ❌
```

**Deliberately absent:** any `input/`, `xml/`, `factory/`, `driver/`,
`job/`, `code_interface/`, or `outstream/` module. Their absence is the point.

## 7. Tranches and their verification paths

Each tranche states what it can be checked against. Per
`VERIFICATION_AND_VALIDATION.md` and the V&V documentation rule in `CLAUDE.md`,
**a tranche with no verification path is not scheduled.**

The reusable oracles, all already on disk in the upstream clone:

- **`doc/tests/analytic_tests.tex`** and its 12 included chapters (1,705 lines
  of LaTeX total) — closed-form derivations with numeric reference values.
  This is the highest-value verification artefact in the repository.
- **`tests/framework/AnalyticModels/`** — the corresponding Python models
  (`ishigami.py`, `sudret_sobol_poly.py`, `gFunction.py`, `attenuate.py`,
  `tensor_poly.py`, `poly_scgpc_gamma.py`, `parabolas.py`, `projectile.py`),
  plus 39 files in `optimizing/` (Rosenbrock, Beale, Goldstein–Price, Levi,
  Matyas, McCormick, Egg-holder, Mishra Bird, Townsend, Simionescu, ZDT).

**What is *not* a usable oracle:** `tests/framework/*/gold/*.csv`. Those are
RNG-stream-dependent sample dumps. Unless the port reproduces numpy's PCG64
stream bit-for-bit *and* RAVEN's exact draw ordering, they will not match, and
chasing them would be a trap. They are useful only under the "bit-exact"
answer to the RNG question in [§10](#10-open-questions-for-the-maintainer).

---

### Tranche 1 — Distributions + Monte Carlo + LHS **(recommended start)**

**Upstream scope:** `Distributions1D.py` (854), the parameterisation and
truncation logic from `Distributions.py` (of 4,195, roughly 2,400 after
removing XML boilerplate — and much of that remainder is per-distribution
repetition), `utils/randomUtils.py` (565), `Samplers/MonteCarlo.py` (183),
`Samplers/Stratified.py` (280).

**Size: moderate, and the smallest tranche here.** The distribution catalogue
is repetitive rather than deep — ~17 univariate distributions, each with
PDF/CDF/PPF/mean/variance plus truncation. The genuinely fiddly part is the
special-function layer: `erf`/`erfc` (normal), incomplete gamma (gamma,
Poisson), incomplete beta (beta, binomial), and their inverses for the PPFs.
That is a known-hard, known-solved body of numerics and should be budgeted
properly rather than waved through.

**Verification:**

1. **Analytic moments per distribution.** Every distribution has closed-form
   mean, variance, skewness and kurtosis. Check the implementation against
   them directly, and check the *truncated* moments by numerical quadrature of
   the truncated PDF.
2. **CDF/PPF round-trip.** $F^{-1}(F(x)) = x$ across the support, and
   $F(F^{-1}(p)) = p$ for $p \in (0,1)$, to a stated tolerance. Catches PPF
   solver bugs, which are the most common failure mode.
3. **PDF integrates to one.** Adaptive quadrature of the PDF over the support,
   truncated and untruncated.
4. **Kolmogorov–Smirnov, sampler against its own CDF.** Draw $n$ samples,
   compare the empirical CDF to the analytic one. Verifies `rvs()`.
5. **LHS stratification property — exact, not statistical.** For $n$ samples in
   $d$ dimensions, each dimension's samples must land one per stratum
   $[(i-1)/n,\ i/n)$. This is a deterministic property and should be asserted
   as one, not sampled.
6. **LHS variance reduction.** On a monotone integrand, LHS variance must be
   below plain MC variance at the same $n$, over repeated trials.
7. **The attenuation model** (`attenuate.py`, documented in
   `doc/tests/attenuate.tex`) — the exit strength of a monodirectional
   single-energy beam through $N$ absorbing sections, which is a genuinely
   reactor-flavoured test:

   $$u(Y) = \prod_{n=1}^{N} e^{-y_n/N}$$

   With all $y_n$ uniform on $[0,1]$, upstream derives and tabulates:

   $$\mathbb{E}[u] = \left[N\left(1 - e^{-1/N}\right)\right]^{N}$$

   $$\operatorname{var}[u] = \left[\frac{N}{2}\left(1 - e^{-2/N}\right)\right]^{N} - \left[N\left(1 - e^{-1/N}\right)\right]^{2N}$$

   with reference values $\mathbb{E} = 0.61927248698470190$,
   $\operatorname{var} = 0.01607798775751018$ at $N = 2$;
   $0.61287838657652779$ and $0.00787849640356994$ at $N = 4$;
   $0.61075635579491642$ and $0.00520852933409887$ at $N = 6$. A Monte Carlo
   estimate must converge on these at the expected $n^{-1/2}$ rate, and the
   convergence *rate* is itself the assertion.

**Why this tranche first.** It is self-contained (nothing upstream of it), it
is the dependency of every later tranche, it is verifiable entirely against
closed-form mathematics with zero benchmark-retrieval effort, and it is useful
the day it lands — the workspace has no distribution or sampling library at all
today.

---

### Tranche 2 — Grid sampling and basic statistics

**Upstream scope:** `Samplers/Grid.py` (312), `GridEntities.py` (1,195),
`Models/PostProcessors/BasicStatistics.py` (1,580), `DistributionsND.py` (270)
for the multivariate normal.

**Size: larger than tranche 1.** `GridEntities.py` is more intricate than it
looks (value-space vs CDF-space grids, global grids across correlated
variables), and `BasicStatistics.py` carries 21 metrics plus 7 standard errors,
each with a weighted variant.

**Verification:**

1. **Statistics against closed-form moments of tranche-1 distributions** —
   feed exact quantiles, check the estimator recovers the known moment.
2. **Weighted statistics against unweighted** with unit weights (identity
   check), and against hand-computed small cases with non-unit weights.
3. **Standard errors against their sampling distributions** — the SE of the
   mean must scale as $\sigma/\sqrt{n}$; verify empirically over many trials.
4. **Pearson and Spearman against constructed correlations** — generate data
   with a known correlation, recover it. Spearman must be exactly 1 for any
   monotone transform.
5. **Grid sampling** — exhaustiveness (every grid point visited exactly once)
   and correct probability weights (weights sum to 1 over the grid).
6. **MVN** — sample covariance converges to the specified covariance; the
   Cholesky factor satisfies $L L^{T} = \Sigma$ to machine precision.

---

### Tranche 3 — Polynomial chaos, sparse grids, and Sobol indices

**Upstream scope:** `OrthoPolynomials.py` (421), `Quadratures.py` (807),
`IndexSets.py` (465), `Samplers/SparseGridCollocation.py` (340),
`Samplers/Sobol.py` (237), `GaussPolynomialRom.py` (668), `HDMRRom.py` (414).

**Size: large, and the hardest tranche in this document.** It is where the
special-function and quadrature-node work lives (see
[§3.4](#34-polynomial-chaos-tranche-3)), where the no-BLAS constraint bites,
and where a wrong Gauss node quietly degrades every downstream number instead
of failing loudly. Budget it as the single biggest item, not as "PCE, one
crate module".

**Verification — this tranche has the best oracle in the whole port:**

1. **Orthogonal polynomials against their three-term recurrences and known
   values** — Legendre $P_n(1) = 1$, orthogonality integrals
   $\int P_m P_n \, w \, dx = \delta_{mn} \|P_n\|^2$ evaluated by the very
   quadrature being tested (a consistency check that catches node errors).
2. **Gauss quadrature exactness.** An $n$-point Gauss rule integrates
   polynomials of degree $\le 2n-1$ *exactly*. This is a hard, deterministic
   assertion to machine precision and is the strongest possible check on the
   node/weight computation. Assert it for every rule and every $n$ in range.
3. **Sparse-grid exactness** on the corresponding total-degree polynomial space.
4. **PCE moments against the attenuation model** — same analytic
   $\mathbb{E}$ and $\operatorname{var}$ as tranche 1, but now PCE should hit
   them far faster than MC. Upstream chose this model precisely because it is
   *hard* to represent with polynomials, so it also probes convergence honestly.
5. **Ishigami function** (`tests/framework/AnalyticModels/ishigami.py`, from
   Ishigami and Homma 1990, restated in Sudret 2008), the standard Sobol
   benchmark:

   $$u(Y) = \sin(x_1) + 7 \sin^{2}(x_2) + 0.1\, x_3^{4} \sin(x_1)$$

   with $x_1, x_2, x_3$ uniform on $[-\pi, \pi]$, $a = 7$, $b = 0.1$. Upstream
   records the analytic total variance $a^2/8 + b\pi^4/5 + b^2\pi^8/18 + 1/2 = 13.8446$,
   the partial variances $D_1 = 4.34589$, $D_2 = 6.125$, $D_3 = 0$,
   $D_{13} = 3.3737$, with $D_{12} = D_{23} = D_{123} = 0$, and the Sobol
   indices $S_1 = 0.3138$, $S_2 = 0.4424$, $S_3 = 0$, $S_{13} = 0.2436$, and
   $S_{12} = S_{23} = S_{123} = 0$. The zero indices are the sharpest test:
   they must come out numerically zero, not merely small.
6. **Sudret's polynomial** (`sudret_sobol_poly.py`, documented in
   `doc/tests/sobol_sens.tex`), which gives Sobol indices as *exact rationals*:

   $$u(Y) = \frac{1}{2^{N}} \prod_{n=1}^{N} \left(3 y_n^{2} + 1\right)$$

   with $y_n$ uniform on $[0,1]$. For $N = 3$: $S_1 = S_2 = S_3 = 25/91$,
   $S_{12} = S_{13} = S_{23} = 5/91$, $S_{123} = 1/91$, mean $1.0$, variance
   $0.728$. Because it is a polynomial, a sufficiently high-order PCE should
   reproduce these to **near machine precision**, not merely to a sampling
   tolerance — which makes it a verification test rather than a convergence
   study.
7. **Sobol's g-function** (`gFunction.py`, from Saltelli and Sobol 1995) with
   the upstream tuner set $a = \{1, 2, 5, 10, 20, 50, 100, 500\}$ — a
   published 8-dimensional case with closed-form indices, exercising the
   high-dimension path.
8. **`doc/tests/tensor_poly.tex`** and **`gamma_scgpc.tex`** — two further
   documented cases with derived reference values, for tensor-product
   polynomials and gamma-distributed sparse-grid collocation.

---

### Tranche 4 — Adaptive sparse grid and adaptive Sobol

**Upstream scope:** `AdaptiveSparseGrid.py` (684), `AdaptiveSobol.py` (937),
`IndexSets.AdaptiveSet`, `AdaptiveMonteCarlo.py` (269).

**Size: large.** Adaptive index-set growth with impact estimation and
convergence control. Genuinely intricate.

**Verification:** the same analytic cases as tranche 3, with the adaptive
result required to reach a stated tolerance in **fewer model evaluations** than
the equivalent static grid — the adaptivity claim itself becomes the assertion.
The Ishigami case is the natural one, since $S_3 = 0$ means a correct adaptive
scheme should spend almost nothing on $x_3$, which is directly observable.

---

### Tranche 5 — MCMC

**Upstream scope:** `Samplers/MCMC/MCMC.py` (477), `Metropolis.py` (200),
`AdaptiveMetropolis.py` (391).

**Size: moderate.** Not in the brief's list, but proposed because Bayesian
calibration against measured reactor data is a capability the workspace will
want, and it sits directly on tranche 1 with no new dependencies.

**Verification:** sample a target with a known closed form (a Gaussian, a
Gaussian mixture, a banana-shaped target) and check recovered moments;
Gelman–Rubin $\hat{R}$ convergence across chains; the detailed-balance property
of the acceptance rule asserted directly on the transition kernel for a small
discrete state space. **Conjugate Bayesian problems give exact posteriors** —
a Beta-Binomial or Normal-Normal conjugate pair yields an analytic posterior
the sampler must reproduce, which is the cleanest possible MCMC test.

---

### Tranche 6 — Optimisation

**Upstream scope:** `GradientDescent.py` (873) + `gradients/` (587) +
`stepManipulators/` (984) + `acceptanceConditions/` (171);
`SimulatedAnnealing.py` (779); `GeneticAlgorithm.py` (1,484) + operator
sub-packages (~1,300).

**Size: large, and it is the least urgent.** The workspace already has
optimisation-adjacent numerics elsewhere, and RAVEN's optimisers are the part
most tangled with `RavenSampled` (767 lines of sampler-shaped orchestration).
Placed last deliberately.

**Verification:** the 39 analytic functions in
`tests/framework/AnalyticModels/optimizing/`, all with published global optima —
Rosenbrock ($f = 0$ at $(1,1)$), Beale, Goldstein–Price, Levi, Matyas,
McCormick, Egg-holder, plus the constrained cases (Mishra Bird, Townsend,
Simionescu, Rosenbrock-with-disk, Rosenbrock-with-cubic) and the ZDT
multi-objective family with known Pareto fronts. `doc/tests/optimization_functions.tex`
(252 lines) documents them. Additionally: gradient approximators
(FiniteDifference, CentralDifference, SPSA) must be checked against **analytic
derivatives** of these same functions, which is a much sharper test than
end-to-end convergence.

**Explicitly deferred: `BayesianOptimizer.py`.** It requires a Gaussian process,
which RAVEN does not contain, plus `scipy.optimize` for acquisition-function
maximisation. Not portable from RAVEN. Revisit only if a GP is built separately.

---

### Ordering summary

| Tranche | Content | Relative size | Depends on |
|---|---|---|---|
| 1 | Distributions, RNG, MC, LHS | Moderate — smallest here | — |
| 2 | Grid sampling, basic statistics, MVN | Larger | 1 |
| 3 | PCE, sparse grids, Sobol | **Largest** | 1, 2 |
| 4 | Adaptive sparse grid / Sobol | Large | 3 |
| 5 | MCMC | Moderate | 1 |
| 6 | Optimisation | Large | 1 (2 for stochastic objectives) |

Tranches 5 and 6 depend only on tranche 1 and can run in parallel with 3 and 4
if the maintainer wants breadth over depth.

## 8. Design-rule friction

RAVEN is deeply object-oriented, and three specific patterns collide with the
workspace's Rust design rules.

### 8.1 Multiple inheritance — the sharpest collision

Rust enums have no multiple inheritance and no MRO. Measured upstream cases:

| Class | Bases |
|---|---|
| `AdaptiveMonteCarlo` | `AdaptiveSampler`, `MonteCarlo` |
| `AdaptiveSparseGrid` | `SparseGridCollocation`, `AdaptiveSampler` |
| `AdaptiveSobol` | `Sobol`, `AdaptiveSparseGrid` |
| `AdaptiveDynamicEventTree` | `DynamicEventTree`, `LimitSurfaceSearch` |
| `Sampler` (the root) | `metaclass_insert(ABCMeta, BaseEntity)`, `Assembler`, `InputDataUser` |

`AdaptiveSobol` therefore inherits from `Sobol` → `SparseGridCollocation` →
`Grid` → `Sampler`, *and* from `AdaptiveSparseGrid` → `AdaptiveSampler` →
`Sampler` — a diamond four levels deep. **This does not translate. It must be
redesigned.**

The resolution is composition: the "adaptive" axis is not a base class but a
**driver that owns a sampler and a convergence policy**. Concretely, instead of
`AdaptiveSparseGrid : (SparseGridCollocation, AdaptiveSampler)`, have an
`AdaptiveDriver` struct owning a `SparseGrid` plus an index-set growth policy.
That is a better design in Python too; upstream's inheritance here is
historical.

### 8.2 `Optimizer` is a subclass of `AdaptiveSampler`

In RAVEN, optimisation *is* adaptive sampling: `Optimizer(AdaptiveSampler)`,
and every concrete optimiser derives from `RavenSampled(Optimizer)`. This is a
coherent design given RAVEN's job-handler architecture — an optimiser proposes
points, the framework evaluates them asynchronously.

In Rust, with the orchestration layer excluded, this identification is
unnecessary and unhelpful. **Keep `Sampler` and `Optimizer` as separate enums.**
An optimiser that wants an initial design calls the LHS sampler; it does not
*inherit* from it.

### 8.3 Entity factories → enums

Every RAVEN subsystem has a `Factory.py` mapping XML type strings to classes at
runtime (`Samplers/Factory.py`, `Optimizers/Factory.py`,
`SupervisedLearning/Factory.py`, `Metrics/Factory.py`, plus five more inside
`Optimizers/`). This entire pattern disappears. The proposed shapes:

```rust
/// A univariate probability distribution over a real- or integer-valued
/// random variable, with optional truncation to a sub-interval of its support.
///
/// Every variant documents the physical meaning of its parameters and their
/// valid ranges. Truncation renormalises the density so it still integrates
/// to one over the truncated support.
pub enum Distribution {
    Uniform(Uniform),
    Normal(Normal),
    Gamma(Gamma),
    Beta(Beta),
    Triangular(Triangular),
    Logistic(Logistic),
    Laplace(Laplace),
    Exponential(Exponential),
    LogNormal(LogNormal),
    Weibull(Weibull),
    LogUniform(LogUniform),
    Poisson(Poisson),
    Binomial(Binomial),
    Bernoulli(Bernoulli),
    Geometric(Geometric),
    Categorical(Categorical),
    UniformDiscrete(UniformDiscrete),
}

/// Compiler-enforced contract every distribution must satisfy.
/// Used for checking, never for `dyn` dispatch.
pub trait Distribution1D {
    fn pdf(&self, x: f64) -> f64;
    fn cdf(&self, x: f64) -> f64;
    fn ppf(&self, p: f64) -> f64;
    fn mean(&self) -> f64;
    fn variance(&self) -> f64;
    fn sample(&self, rng: &mut RngKind) -> f64;
}

impl Distribution {
    pub fn pdf(&self, x: f64) -> f64 {
        match self {
            Self::Uniform(d) => d.pdf(x),
            Self::Normal(d)  => d.pdf(x),
            // ... exhaustive; a new variant is a compile error at every site
        }
    }
}
```

and for samplers:

```rust
/// A design-of-experiments strategy over a set of uncertain inputs.
pub enum Sampler {
    MonteCarlo(MonteCarlo),
    LatinHypercube(LatinHypercube),
    Grid(GridSampler),
    SparseGrid(SparseGridCollocation),
    Sobol(SobolSampler),
    Metropolis(Metropolis),
}
```

Adaptivity is composed on top, not inherited:

```rust
/// Drives a sampler to a convergence target, growing the design between
/// batches according to `policy`. Replaces RAVEN's `Adaptive*` inheritance
/// diamond with composition.
pub struct AdaptiveDriver {
    sampler: Sampler,
    policy: GrowthPolicy,
    convergence: ConvergenceCriteria,
}
```

### 8.4 Other rule interactions

- **No `Box<T>`, no lifetimes.** No in-scope module needs either. Distributions
  are small `Copy`-ish value types; samplers own their distribution list by
  value or behind `Arc` if shared.
- **`Arc<RwLock<T>>` for shared state.** Not needed in tranche 1; may become
  relevant if a parallel sampler shares an accumulator. Prefer `rayon`'s
  reduction over shared mutable state.
- **`uom`.** Distributions here are over *dimensionless* normalised quantities
  or over whatever unit the caller's uncertain parameter carries. **Do not
  force `uom` into the distribution layer** — a `Normal` over a temperature and
  a `Normal` over a reactivity are the same mathematics. The unit belongs to
  the caller's parameter definition, not to the distribution. This should be
  stated explicitly in the module docs so a reader does not think it was an
  oversight. *(Flagged for the maintainer in [§10](#10-open-questions-for-the-maintainer).)*
- **Human interface layer.** Every distribution's `///` must state the
  parameterisation used, because RAVEN, SciPy, Boost and most textbooks
  disagree — RAVEN's `Gamma` takes `alpha` and `beta` and constructs
  `BasicGammaDistribution(alpha, 1.0/beta, low)`, i.e. it passes a *scale*
  where it stores a *rate*. Getting this wrong is silent and this is exactly
  the class of confusion the human-interface rule exists to prevent.

## 9. Attribution and licence

### 9.1 Upstream licence — verified

RAVEN is **Apache License 2.0** (`LICENSE.txt`, verified 2026-08-06; GitHub's
API reports SPDX `Apache-2.0`). `NOTICE.txt` records:

> Copyright 2017 Battelle Energy Alliance, LLC — ALL RIGHTS RESERVED
>
> These data were produced by Office of Nuclear Energy of the U.S. Department
> of Energy under Contract No. DE-AC07-05ID14517 with the Department of Energy.

Every source file carries the standard Apache-2.0 header block.

### 9.2 Bundled third-party licences found in `NOTICE.txt` — flagged

`NOTICE.txt` discloses **two BSD-licensed components** vendored into RAVEN:

| Component | Licence | Copyright | Where |
|---|---|---|---|
| **AMSC** | 3-clause BSD | 2014 University of Utah, Scientific Computing and Imaging Institute | `src/AMSC/` (1,633 lines C++, 1,254 lines Python) |
| **NGL** | 2-clause BSD | 2012 Carlos D. Correa | used by AMSC |

Both are GPL-3.0-compatible, so they would not block a port — but **both are
out of scope anyway** (`MSR.py` and topological decomposition are excluded in
[§4](#4-what-is-explicitly-not-worth-porting)). Recorded here so a future
tranche does not pick them up without noticing they are *not* Apache-2.0 and
carry different attribution requirements.

Additionally, `crow/contrib/include/boost/` vendors Boost `predef` headers
(Boost Software License). Out of scope with the rest of `crow`.

### 9.3 Apache-2.0 into GPL-3.0 is one-way

Apache-2.0 is compatible with GPL-3.0 **in one direction only**: Apache-2.0
code may be incorporated into a GPL-3.0 work, and the combined work is
GPL-3.0. It is *not* compatible with GPL-2.0-only, and the flow does not
reverse.

**Concretely, for this port:**

- `outram-park-fork-raven` is **GPL-3.0**, like the rest of the workspace.
- **Nothing from this port can be contributed back to RAVEN upstream.** Once
  our derived work is GPL-3.0, INL cannot take it into an Apache-2.0 project.
  If the maintainer ever wants to contribute upstream, that contribution must
  be written independently and licensed Apache-2.0 — it cannot be a copy of
  our GPLv3 files. Nobody should discover this after writing the patch.
- Apache-2.0 §4 obligations survive into the derived work: retain copyright,
  patent, trademark and attribution notices; state significant changes; carry
  forward the relevant parts of `NOTICE.txt`.
- Apache-2.0 §6 forbids using the licensor's trademarks. The crate README must
  say, in the workspace's established wording, that it is an **independent
  fork, not official RAVEN, and not affiliated with Idaho National Laboratory
  or Battelle Energy Alliance**.

### 9.4 Provenance headers — the workspace obligation

Per `CLAUDE.md` and `RESEARCH_INTEGRITY_AND_PROVENANCE.md`, every ported file
keeps an attribution header naming upstream project, source file,
version/commit, copyright and licence — the pattern
`outram-park-fork-offbeat` and `outram-park-fork-coolprop` already use. **It
must not be stripped during refactors.** For this port:

```rust
// Ported from RAVEN (Risk Analysis Virtual ENvironment)
//   Upstream project: https://github.com/idaholab/raven
//   Source file:      ravenframework/Distributions1D.py
//   Version/commit:   01216937 (branch devel, 2026-07-14)
//   Copyright:        2017 Battelle Energy Alliance, LLC. All rights reserved.
//                     Produced under U.S. DOE Contract DE-AC07-05ID14517.
//   Upstream licence: Apache-2.0
//
// This file is part of OUTRAM PARK and is licensed GPL-3.0-or-later.
// Apache-2.0 permits incorporation into a GPL-3.0 work; the reverse does not
// hold, so this file cannot be contributed back to RAVEN upstream.
//
// Changes from upstream: <state them — Apache-2.0 section 4(b) requires this>
```

A top-level `NOTICE` in the crate should carry forward the Battelle copyright
and DOE contract statement verbatim.

### 9.5 Data-policy position

Everything named in this document is public: the RAVEN source, the analytic
test models, and the LaTeX derivations of their reference values. **No
restricted, proprietary, or operational data is involved**, and none should be
introduced. Note in particular that the excluded `CodeInterfaceClasses/` couple
to codes (SCALE, SERPENT, RELAP5, MELCOR, PARCS, SIMULATE3) whose *own*
distribution is often licence-restricted or export-controlled — a further
reason that exclusion is not merely a scoping convenience.

## 10. Open questions for the maintainer

Re-ask these before any port work begins.

1. **RNG strategy — must be settled before tranche 1.** Three options, with
   different consequences:
   - **(a) `rand` + `rand_pcg`.** Idiomatic, pure Rust, Android-clean, well
     tested. Adds a new workspace dependency where none exists today.
   - **(b) Hand-roll PCG64 to match numpy bit-for-bit.** Makes RAVEN's gold
     CSVs usable as regression oracles, but requires matching RAVEN's *draw
     ordering* too, which is fragile and couples us to upstream internals.
   - **(c) Reuse an existing in-workspace generator** — `boon-lay`'s `OoRng64`
     or `njoy`'s `purr::Rng`. Avoids a new dependency but neither was designed
     as a general-purpose statistical RNG and neither is currently shared.

   **The recommendation is (a)**, with the gold CSVs explicitly written off as
   oracles ([§7](#7-tranches-and-their-verification-paths)), because the
   analytic verification path is stronger anyway. But this is the maintainer's
   call, and it determines the tranche-1 test design.

2. **Does the workspace want a general-purpose `rand` dependency at all?**
   Related to (1) but broader: adding `rand` to `[workspace.dependencies]`
   affects more than this crate. It is Android-clean and has no BLAS, so there
   is no portability objection — but it is a policy question.

3. **`uom` in the distribution layer — confirm the position taken in
   [§8.4](#84-other-rule-interactions).** The proposal is that distributions
   stay dimensionless and the caller owns units. This departs from the
   workspace's usual `uom`-everywhere posture and should be an explicit
   decision, not a silent one.

4. **Is `outram-park-fork-raven` the right name?** RAVEN is a framework and we
   are porting its mathematics, not forking the framework. A name like
   `outram-park-uq` would describe the artefact more honestly, at the cost of
   breaking the `outram-park-fork-*` convention and obscuring the provenance.
   The convention argues for `-fork-`; accuracy argues against.

5. **Should tranche 3 (PCE) be split?** It is the largest tranche and contains
   two separable halves: the quadrature/orthogonal-polynomial machinery
   (verifiable on its own by the exactness property) and the PCE/Sobol layer
   on top. Splitting gives an earlier verified landing point at the cost of an
   extra integration boundary.

6. **Is time-series analysis (`TSA/`, 5,131 lines) wanted?** Excluded here as
   a different capability, but ARMA/Fourier/wavelet synthetic-history
   generation is directly relevant to the digital-twin work in
   `outram-park-digital-twin-engine`. If it is wanted, it needs its own
   scoping document — `statsmodels` is a heavier dependency to replace than
   anything in this port.

7. **Is a Gaussian process wanted, given RAVEN does not have one?**
   [§2.5](#25-surrogate--reduced-order-models) establishes that RAVEN's GP is a
   sklearn wrapper. If GP surrogates are a goal, that is a separate
   from-scratch project (and would unblock `BayesianOptimizer`). It should not
   be smuggled into a "RAVEN port".

8. **How much of `BasicStatistics`' standard-error machinery is wanted?** The
   7 standard errors are the most distinctive part of that module and the most
   effort per line. If the immediate need is point estimates only, tranche 2
   shrinks substantially.

## 11. Provenance

- **RAVEN source** — <https://github.com/idaholab/raven>. Apache-2.0
  (`LICENSE.txt` and per-file headers inspected 2026-08-06; GitHub API reports
  SPDX `Apache-2.0`). All file paths and line counts in this document were
  measured on a scratch shallow clone at commit
  `01216937967c38ee287859270c035c8eca906dc6` (branch `devel`, committed
  2026-07-14), taken 2026-08-06. Latest tagged release at that date:
  `RAVENv3.2` (2026-03-12). Repository size 828 MB on disk after checkout.
- **`NOTICE.txt`** — source of the Battelle copyright, the DOE contract number,
  the author list, and the disclosure of the bundled AMSC (University of Utah,
  BSD) and NGL (Carlos D. Correa, BSD) components recorded in
  [§9.2](#92-bundled-third-party-licences-found-in-noticetxt--flagged).
- **`dependencies.xml`** — source of the pinned Python dependency versions in
  [§5.1](#51-upstream-python-dependencies-by-in-scope-module).
- **Analytic reference values** — `doc/tests/attenuate.tex` (attenuation
  moments), `doc/tests/sobol_sens.tex` (Sudret indices), and
  `tests/framework/AnalyticModels/ishigami.py` (Ishigami partial variances and
  Sobol indices, attributed upstream to Ishigami and Homma 1990 and to Sudret,
  *Global Sensitivity Analysis*, 2008). The g-function is attributed upstream to
  Saltelli and Sobol, *Reliability Engineering and System Safety* **50** (1995)
  225–239. **These citations are reproduced from upstream's own comments and
  LaTeX; they have not been independently verified against the cited
  publications**, and must be checked before appearing in any published V&V
  write-up.
- The clone was made under `/tmp` and is **not** vendored into this repository.
  Per workspace policy, upstream stays outside the tree, read-only.
