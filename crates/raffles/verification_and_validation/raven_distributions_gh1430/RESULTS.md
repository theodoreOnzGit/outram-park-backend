# Results: raffles vs RAVEN distributions, BasicStatistics and Grid (gh:#1430)

Measured 2026-11-13, release build, by
`cargo test --release -p raffles --test raven_distributions_gh1430 --test raven_statistics_gh1430 --test raven_grid_gh1430 -- --nocapture`.
Reference versions: RAVEN `devel` @ `dcb75beaae888500402aa7edd0ba0d5d0f64ad05`
(2026-11-11), Python 3.11.18, numpy 1.26.4, scipy 1.14.1, xarray 2024.6.0.
Predictions are in `PREDICTIONS.md`, committed before each run. AI-assisted
draft; verification only.

## Case 1: 1-D distributions (18 cases: 12 untruncated, 6 truncated)

| quantity | bit-identical | worst | where |
|---|---|---|---|
| pdf (564 points) | 179 | 5.7e-15 relative | Beta(2,5), x = 0.982 |
| cdf (567 points) | 258 | 3.1e-15 **absolute** (8.2e-9 relative) | truncated LogNormal, 1e-8 above the lower bound |
| ppf (513 points) | 255 | 7.3e-9 relative; <= 26 units of 2^-53 in probability | truncated Normal, p = 1e-8 |
| mean and std (24) | 16 | 1.1e-15 relative | Weibull(1.8, 3) |

Per family: Uniform, Triangular, Exponential, Weibull and the Normal/LogNormal
pdf agree to a few ulp (Uniform fully bit-identical; Triangular pdf and ppf
bit-identical). Gamma and Beta ppf reach 3.1e-10 and 5.3e-12 relative at
p = 1 - 1e-8 (15 and 26 units of 2^-53 in probability).

Prediction scorecard: D1 (closed forms, <= 1e-15) held (worst 3.3e-16 on
Exponential, 0 on Uniform, 2e-16 Triangular cdf). D2 held (Normal pdf 2.9e-16,
cdf 3.8e-15, inside <= 1e-13). D3 held (Normal ppf 5.3e-16). D4 held. D5 held
except the Weibull pdf at x = 1.1e-4 (1.8e-15, 1.8x the predicted bound). **D6
missed**: Gamma ppf was predicted <= 1e-10 and measured 3.1e-10 (and Beta 5.3e-12
held); the miss is conditioning at p = 1 - 1e-8 (see below), not an
algorithmic error. D7 held, and the amplification is the 1/(F(hi)-F(lo)) plus
the cancellation described below. **D8 confirmed, and it is an upstream
defect**: RAVEN's `LogNormal.std()` returns `sqrt(Var) + low`
(`Distributions1D.py:303`); for `low = 1.5` RAVEN's value exceeds the true
standard deviation by exactly `low` (excess / low = 1.0000000000000002).
raffles does not copy it and is correct. D9 held (every endpoint value agreed;
`pdf` at a Weibull `k < 1` lower bound is `inf` on both sides).

### First divergent operation, per mismatch

* **Normal/LogNormal cdf and pdf.** Different `erf`/`erfc` implementations
  (scipy: Cephes `ndtr`; raffles: its own `special::erf`). Not a port of Cephes;
  differences are 1 to 15 ulp. Expected, below 1e-14.
* **Gamma, Beta cdf/ppf.** Different incomplete gamma/beta implementations
  (scipy's Boost-backed `gammainc`/`betainc` and `*inv` against raffles'
  series/continued fraction + Newton). cdf <= 5.2e-15 relative. The ppf
  differences, measured through raffles' own cdf, are at most 26 units of
  2^-53 in probability: near p = 1 - 1e-8 a probability is only resolvable to
  ~1e-8 of the tail mass, so both answers are consistent with the requested p
  to within that representation.
* **Truncated cdf and ppf near the lower bound.** `(F(x) - F(lo)) /
  (F(hi) - F(lo))` is a difference of nearly equal numbers when x is 1e-8 of the
  mass above `lo`: an absolute error of 1 ulp (~1e-16) in `F` becomes ~1e-8
  relative. RAVEN's formula is identical, so this is conditioning shared by
  both, and is why the cdf is pinned absolutely and the ppf by probability
  units.

No port defect was found in the distributions.

## Case 3: BasicStatistics (RAVEN `_runLocal`, 10^4 x 3, unbiased)

| quantity | n | bit-identical | worst relative |
|---|---|---|---|
| mean | 3 | 0 | 3.2e-15 |
| variance | 3 | 0 | 2.4e-15 |
| sigma | 3 | 0 | 1.2e-15 |
| covariance (RAVEN `cov_*`) | 9 | 0 | 4.6e-15 |
| Pearson | 9 | 1 | 5.4e-15 |
| Spearman | 9 | 7 | 1.6e-16 |

Scorecard: S1 **missed** (mean predicted <= 2e-16, measured 3.2e-15). S2 held
(<= 1e-14). S3 held (<= 1e-13). S4 not separately asserted (the test's
covariance diagonal uses the variance, as RAVEN does).

**First divergent operation: the summation order.** raffles sums left to
right; numpy sums pairwise (8 accumulators, 128-element leaves). Re-doing the
mean and the variance in numpy's pairwise order inside the test reproduces
RAVEN **3/3 bit-identical for both**, so the formulas (including the `n/(n-1)`
correction) are the same and only the sum order differs. The raffles
functions were not changed: `sample_mean`/`sample_variance` are in the path
`outram-mc-libs` pins to the bit (root and crate `AGENTS.md`), so adopting
pairwise summation is a decision for the maintainer (deferred, issue
comment). Left-to-right summation of 10^4 terms is also the less accurate
choice, so the change would be an improvement.

Not compared (no raffles function exists): skewness, kurtosis, median,
percentile, minimum, maximum, and the standard errors. A gap.

## Case 2: Grid construction (13 comparable `equal` cases, 4 `custom`)

| constructor | levels | bit-identical | worst absolute |
|---|---|---|---|
| `equally_spaced` (existing) | 1220 | 400 | 5.6e-14 |
| `equally_spaced_as_raven` (new, this change) | 1220 | **1220** | 0 |
| `with_levels` vs RAVEN `custom` | 11 (2 of 4 inputs refused by RAVEN) | 9 | 1.1e-16 |

Five more `equal` cases have `upper = 1.0`, which raffles refuses by design
(`ppf(1)` is infinite) and RAVEN accepts; they are asserted to be refused and
not compared.

Scorecard: G1 held. G2 **partly missed**: predicted ~1e-14, measured 5.6e-14
(the rounded step's error is up to 5e-15 and is multiplied by k, up to 29
here). G3 held (level counts equal in every case). G4 held and exceeded:
RAVEN's own bounds check **rejects 2 of the 4 custom grids** (inputs with
more than 14 decimals, whose rounded steps no longer sum to `upper` within its
1e-14 relative tolerance), where raffles keeps the levels exactly.

**Finding (a documentation defect, fixed here).** The `equally_spaced` doc
said the levels are "exactly as upstream's `numpy.linspace(lower, upper,
steps + 1)` does". RAVEN does not use linspace's values: it keeps the first
step, rounds it to 14 decimals (`GridEntities.py:307`), and rebuilds the
levels with `numpy.arange(lower, upper - step/2, step)` plus `upper`
(`:455-472`). The claim is struck through and corrected in place; the
RAVEN-exact levels are offered beside the plain ones as
`GridSampler::equally_spaced_as_raven` (bit-identical to RAVEN on all 1220
levels). `equally_spaced` is unchanged.

## Not run

* **Stratified / Latin hypercube** (case 2 in the issue). RAVEN draws one
  random permutation and one `random()` per stratum through numpy's MT19937;
  raffles draws through `petir::rng::lcg`, and `LatinHypercube::generate`
  takes a seed, not uniforms. There is no replay entry point, and the streams
  cannot agree. A replay hook (a `generate_from_uniforms`) is the extension
  that would allow it; not added.
* **Sobol indices** (case 4). RAVEN's `SobolIndices` is HDMR over
  `GaussPolynomialRom` and needs a model-evaluation loop; the HDMR ROM is
  already compared in gh:#1193, and the Saltelli estimator is checked against
  the Ishigami closed form. Different estimators; not run.
* **RAVEN's `crow` C++ distributions.** Retired from RAVEN's default path
  (numpy/scipy are the environment, `utils/randomUtils.py:37`); the compiled
  `crow_modules` were not built.
