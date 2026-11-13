# Predictions: RAVEN 1-D distributions vs raffles (gh:#1430, case 1)

Committed BEFORE the reference generator was run. AI-assisted draft material;
verification only.

Upstream: `ravenframework/Distributions1D.py` (`Basic*Distribution`, which
wrap `scipy.stats` and add truncation) and `LogNormal` (RAVEN's own class,
numpy + `scipy.stats.norm`). The `crow` C++ bindings are retired in RAVEN
(the default stochastic environment is numpy), so scipy is the real upstream.

Instrument: `pdf`, `cdf`, `ppf` (RAVEN `inverseCdf`) on a grid per case, and
the untruncated mean / standard deviation; 12 untruncated + 6 truncated cases.

| # | Quantity | Prediction | Reason |
|---|---|---|---|
| D1 | Uniform, Triangular, Exponential pdf/cdf/ppf | bit-identical in most points, worst <= 1e-15 rel | closed forms; differences from operation order and `exp`/`ln` (PETIR vs glibc) only |
| D2 | Normal cdf, pdf | pdf <= 1e-15; cdf <= 1e-13 | scipy `ndtr` (Cephes erfc) vs the crate's own erf |
| D3 | Normal ppf | <= 1e-12 rel in the body, looser (<= 1e-9) at p = 1e-8 | scipy `ndtri` (Cephes) vs the crate's algorithm; not the same code |
| D4 | LogNormal | as Normal, plus the exp/ln | RAVEN's own formula for pdf; norm for cdf/ppf |
| D5 | Weibull | <= 1e-15 pdf/cdf/ppf | closed forms |
| D6 | Gamma, Beta cdf/ppf | <= 1e-10 rel | scipy special functions (Cephes/Boost) vs the crate's own series/CF + Newton; different implementations, not bit-identical |
| D7 | Truncated variants | inherit the base route's difference, amplified by 1/(F(hi)-F(lo)) | same renormalisation formula |
| D8 | mean / std of untruncated | bit-identical for closed forms; **RAVEN `LogNormal.std()` adds `low`** (`Distributions1D.py:295-303`), so a shifted LogNormal (low != 0) will DISAGREE by exactly `low` unless the crate copied that | read from source |
| D9 | Endpoint conventions (pdf at the bound, cdf <= xMin, ppf(0), ppf(1)) | identical | read from source |

## Case 3: RAVEN `BasicStatistics` vs `raffles::sensitivity` (appended before that run)

Instrument: RAVEN's real `BasicStatistics._runLocal` (xarray + numpy), unweighted,
`biased=False`, on a fixed 10^4 x 3 dataset built from exactly rounded IEEE
operations only (so both sides hold the same bits). raffles side:
`sample_mean`, `sample_variance`, `pearson_correlation`, `spearman_correlation`.
Read from source before the run: RAVEN skewness, kurtosis, median, minimum,
maximum and percentile exist; a grep of `crates/raffles/src` finds no
sample-skewness, sample-kurtosis, percentile or median function (only the
`normality` test's internal `z_from_*` helpers), so those are a **gap**, not a
comparison.

| # | Quantity | Prediction | Reason |
|---|---|---|---|
| S1 | mean | <= 2e-16 relative; probably NOT bit-identical | numpy sums pairwise; raffles' summation order is to be read |
| S2 | variance (unbiased) | <= 1e-14 relative | RAVEN: `(x-mean).var() * n/(n-1)`; numpy var is `mean(abs(x-mean)**2)` |
| S3 | Pearson, Spearman | <= 1e-13 | RAVEN's covariance goes through `np.dot` (BLAS) then divides by sqrt of the diagonal; summation order differs |
| S4 | RAVEN `covariance` diagonal | equals `variance` exactly | `np.fill_diagonal(cov, variance)` |
| S5 | skewness, kurtosis, median, percentile, min, max | not comparable | no raffles function; recorded as a gap and filed |

## Case 2: RAVEN Grid and Stratified construction vs `GridSampler` / `LatinHypercube` (appended before that run)

Read from source before the run (`GridEntities.py`, `Stratified.py`):

* **Grid, `construction="equal"`** (`GridEntities.py:351-356`, `:307`, `:455-472`):
  RAVEN builds `np.linspace(lower, upper, steps+1)`, then takes only its
  first step, ROUNDED TO 14 DECIMALS (`round(g[1]-g[0], 14)`), and rebuilds the
  levels as `np.concatenate((np.arange(lower, upper - 0.5*step, step),
  [upper]))`. raffles' `equally_spaced` computes `lower + (upper-lower)*k/steps`.
* **Grid, `construction="custom"`** (`:490-496`): levels are rebuilt as a
  running sum `round(prev,14) + round(step,14)`, not taken as given.
* **Stratified**: RAVEN draws one random permutation per dimension and
  `lower + (upper-lower)*random()` per stratum, through numpy's MT19937 and
  `utils.randomUtils`; raffles draws through `petir::rng::lcg`. The streams
  cannot match and raffles' `generate` takes a seed, not uniforms, so there is
  no replay entry point. Only the structural property (one point per stratum)
  is comparable and that is already a crate test. Not run.

| # | Quantity | Prediction | Reason |
|---|---|---|---|
| G1 | `equal` levels where the step is exactly representable to 14 decimals (e.g. 0.1..0.9 in 2 steps, 0..1 in 10) | same count; levels equal to <= 1 ulp-ish, not necessarily bit-identical | `arange` accumulates `lower + k*step` with the rounded step |
| G2 | `equal` levels where the step is NOT a 14-decimal number (e.g. 0..1 in 3, 0.1..0.9 in 7) | same count, levels differ by up to ~1e-14 (the rounded step times k) | `round(step, 14)` discards up to 5e-15 per step; times k |
| G3 | the level COUNT | equal for every case | `arange(lower, upper - step/2, step)` plus `upper` always yields `steps+1` while rounding error stays below half a step |
| G4 | `custom` levels | RAVEN's levels equal the input only to ~1e-14 (running 14-decimal sum), raffles' are exact; so they differ by <= ~1e-14 for inputs with more than 14 decimals and are identical for short decimals | `:490-496` |
