# Crate Documentation

**Version:** 0.0.4

**Format Version:** 61

# Module `raffles`

# RAFFLES — Risk Analysis Framework For Learning & Ensemble Simulation

An independent pure-Rust port of the uncertainty-quantification (UQ) and
risk-analysis core of [RAVEN](https://github.com/idaholab/raven), the
probabilistic risk-analysis / UQ framework developed by Idaho National
Laboratory. Upstream is Apache-2.0; RAFFLES is AGPL-3.0-only (was GPL-3.0-only until 2026-10-20, gh:#973). That direction
is **one-way** — see [Licensing](#licensing) below.

**Owner: Adolphus Lye.** They chose the RAFFLES backronym and this crate is
theirs to steer; changes of direction are their call. See the crate
`CLAUDE.md`.

## Status — PARTLY IMPLEMENTED, NO HUMAN V&V

This crate is no longer the empty scaffold its first commits described.
Every module below — [`distributions`], [`samplers`], [`sensitivity`],
[`bayesian`], [`distance`], [`abc`], [`imprecise`], [`model_selection`],
[`scram`], [`gnn`], [`surrogate`] and [`fidelity`] (the typed
high-fidelity-to-surrogate interface, gh:#1194) — carries working, unit-tested code
whose verification methodology and measured results are recorded in the doc
comments of the tests themselves.

Two carry less than their names suggest and say so in their own docs:
[`surrogate`] has polynomial regression and a `burn`-backed neural
regressor but no Gaussian process and no polynomial chaos, and [`scram`]
has no preprocessor, so a model whose variable ordering matters is at the
mercy of a first-appearance heuristic.

**None of it has been through human V&V.** Everything here is AI-assisted
draft material under the workspace `RESPONSIBLE_USE.md` rules until the
maintainer and the crate owner have reviewed it — so do not describe any
part of this crate as validated, and read "verified" as "checked against a
reference by an automated test", which is what it is.

## What belongs in this crate

The *statistical machinery* for running ensembles of simulations and
reasoning about the spread of their answers:

- **[`distributions`]** — probability distributions: densities, cumulative
  distribution functions, inverse CDFs, analytic moments.
- **[`samplers`]** — strategies that turn distributions into a concrete set
  of sample points: Monte Carlo, Latin hypercube, grid / stratified.
- **[`sensitivity`]** — importance measures computed from an existing
  sample set: Sobol variance decomposition, correlation coefficients.
- **[`distance`]** — statistical distances between two sample sets:
  Euclidean on summaries, Bhattacharyya, Hellinger, Jensen-Shannon,
  Bray-Curtis and the 1-Wasserstein area metric.
- **[`model_selection`]** — comparing competing models by their evidence:
  Bayes factors, posterior model probabilities, the Kass-Raftery scale.
- **[`imprecise`]** — imprecise probability: intervals, probability boxes,
  Clopper-Pearson confidence boxes, and coherent-system reliability with or
  without a dependence assumption.
- **[`abc`]** — Approximate Bayesian Computation: inference when the model
  can be run but no likelihood can be written down.
- **[`bayesian`]** — Bayesian model updating: priors, likelihoods, MCMC
  moves, and the transitional samplers (TMCMC, TEMCMC) that produce both a
  posterior sample and the evidence.
- **[`scram`]** — fault trees: build one, generate its minimal cut sets,
  derive its prime implicants, quantify the top-event probability by cut
  sets or by a binary decision diagram, and rank the basic events by the
  five standard importance measures. Coherent and non-coherent, though on
  a non-coherent tree cut sets are conservative where the prime implicants
  and the BDD are exact.
- **[`gnn`]** — graph neural networks for physics: message-passing
  topology, the physics-guided bound on message-passing iterations, and
  (behind the `burn` feature) the network itself.
- **[`surrogate`]** — reduced-order models fitted to a sample set and
  evaluated in place of the expensive simulation.

## What does NOT belong in this crate

- **Physics.** RAFFLES holds no reactor, thermal-hydraulic, neutronic or
  chemistry model. It samples inputs and consumes outputs; the physics
  lives in the other Outram Park crates.
- **Simulation drivers, job scheduling, file/XML input parsing, plotting,
  databases.** RAVEN is a whole workflow application; RAFFLES ports only
  its statistical core. A caller drives their own runs and hands RAFFLES
  arrays of numbers. ~~[`scram`] holds to the same line: it takes a fault
  tree a caller has built in Rust, never a SCRAM input model.~~
  **CORRECTED 2026-09-22** — [`scram`] no longer holds to that line: the
  workspace maintainer directed that everything of SCRAM except its GUI be
  translated, and [`scram::mef`] reads SCRAM's own Model Exchange Format
  input models. Building a tree in Rust is still supported and is still
  what the rest of the module takes. The line does hold for the
  RAVEN-derived modules, where no such direction was given.
- **Optimisation.** RAVEN's optimisers (gradient descent, genetic
  algorithms, Bayesian optimisation) are out of scope unless the crate
  owner decides otherwise.
- **Anything Android-hostile.** No system BLAS/LAPACK, no C or Fortran
  toolchain, no GUI. The crate must build natively on Termux
  (`aarch64-linux-android`). If dense linear algebra becomes necessary,
  prefer the pure-Rust `faer` already in the workspace, and target-gate
  anything BLAS-backed off Android in the same change.

## Units

RAFFLES quantities are dimensionless by nature — probabilities, quantiles,
variance fractions, correlation coefficients, sample counts — so `uom` is
deliberately not used here. Sample *values* are plain `f64` in whatever
units the caller's model uses; RAFFLES never interprets them physically.
Where a doc comment gives a range it is an ordinary numeric range, e.g. a
probability in `[0, 1]` or a Sobol index in `[0, 1]`.

## Design rules that bind every module here

RAVEN is deeply inheritance-based (`Sampler` -> `ForwardSampler` ->
`MonteCarlo`, and so on). That structure must **not** be transcribed into
Rust as trait objects. Per the workspace design rules:

- **Enum dispatch, never `Box<dyn Trait>` / `&dyn Trait` / `Arc<dyn Trait>`.**
  The set of distributions and samplers is closed and known at compile
  time, so each family becomes one enum with a variant per concrete model.
  A trait may still be used as a compiler-enforced contract on the concrete
  structs — just not for runtime dispatch.
- **No `Box<T>`** — own by value, or share with `Arc<T>`.
- **No lifetime parameters** on structs, traits or impls — own the data, or
  share it with `Arc<T>`.

## Verification

Nothing here is "done" until it is checked against something that is known
independently: analytic moments for a distribution, the published Sobol
indices of the Ishigami function for a sensitivity estimator, a published
test problem for anything else. The workspace V&V rule requires both the
*methodology* and the measured *results* to be written down. See
`CLAUDE.md` in this crate.

## Licensing

Upstream RAVEN is **Apache-2.0**; RAFFLES is ~~**GPL-3.0-only**~~
**AGPL-3.0-only** (2026-10-20, gh:#973), a combined work whose GPL-3.0 ports
keep GPL-3.0 (`LICENSE.GPL-3.0`). Apache-2.0
code may be taken into an (A)GPLv3 work, but (A)GPLv3 code may **not** be taken
into an Apache-2.0 work. Code therefore flows RAVEN -> RAFFLES and never
RAFFLES -> RAVEN. Do not contribute RAFFLES code upstream. Full provenance,
the required Battelle Energy Alliance / Idaho National Laboratory
attribution, and the verbatim upstream licence text are in the crate's
`NOTICE`, `LICENSE-APACHE-RAVEN` and `NOTICE-RAVEN`.

**RAVEN is not the only upstream, and the others are not Apache-2.0.**
[`scram`] derives from [SCRAM](https://github.com/rakhimov/scram) and parts
of [`bayesian`] and [`gnn`] from other projects, all **GPL-3.0** (those
files keep it, combined with the AGPL crate under §13), so none
carries the one-way constraint above. Check which upstream a file comes
from before writing an attribution header; the `NOTICE` lists all of them.

## Intended use

Education, research, capability building and V&V only. Despite the name,
RAFFLES is **not** for nuclear facility operation, reactor control,
licensing decisions, probabilistic safety assessment of a real facility,
safety-critical decision-making or emergency response.

## Scoping

The port scope — which RAVEN capabilities are in, which are out, and in
what order — is written up in `docs/raven-port-scoping.md` at the workspace
root.

## Modules

## Module `abc`

Approximate Bayesian Computation — inference when there is no likelihood.

# The problem it solves

Bayesian updating needs `p(D | theta)`. A great many engineering models
cannot supply one: the model is a black box, or its output is stochastic in
a way nobody has written down, or the thing being matched is a *scatter of
measurements* rather than a point with a known error distribution. What such
a model can always do is **run**.

ABC replaces the likelihood with a simulation and a distance:

1. propose `theta`,
2. run the model at `theta` to get a synthetic sample,
3. measure how far that sample is from the observed one, with one of
   [`crate::distance::DistanceMetric`],
4. turn that distance into a likelihood with a kernel of width `epsilon`.

As `epsilon` shrinks the approximation approaches the true posterior and the
computation gets harder. That trade-off is the whole method, and
[`AbcKernel::epsilon`] is where it is made explicit.

# Two ways to run it

- [`rejection_abc`] — draw from the prior, simulate, keep the close ones.
  Transparent, embarrassingly parallel, and hopeless when the prior is
  broad: the acceptance rate falls off a cliff. Use it as a reference and a
  sanity check, which is exactly what this module uses it for.
- [`abc_ln_likelihood`] with [`crate::bayesian::temcmc`] — build an
  approximate log-likelihood and hand it to the transitional sampler. This
  is the distance-based stochastic model updating framework of the recent
  literature (Bhattacharyya-, Hellinger- and Jensen–Shannon-based variants
  are just different choices of [`crate::distance::DistanceMetric`]), and it
  is what makes the method usable on a real problem.

# Draw as many samples as you have observations

**The simulator should return about as many rows as the observed data
has.** This looks like a performance detail and is not: it decides whether
the posterior's *width* means anything.

A distribution-to-distribution distance compares the model's output
distribution with the **empirical** distribution of the data, and treats
that empirical distribution as if it were the truth. Simulate far more
points than you observed and the model side becomes essentially exact, so
the distance is minimised by whichever parameter best matches the
particular sampling noise in your handful of measurements. Shrink `epsilon`
and the posterior concentrates on that minimiser — reporting a confidence
the data cannot support.

Measured on this module's own conjugate test problem (12 observations of
`N(2.5, 1)`, exact posterior sd 0.288195):

| simulated rows | tolerance | posterior sd | verdict |
|---|---|---|---|
| 200 | 0.1 | 0.118810 | 2.4x **too tight** — over-confident |
| 12 | 0.2 | 0.366628 | 1.27x wider than exact — the honest direction |

~~With the sample sizes matched, tightening the tolerance walks the posterior
sd down towards the exact value and stops there rather than sailing past it:
0.5677 at `epsilon = 0.5`, 0.3464 at 0.2, 0.3110 at 0.05, against the exact
0.288195. That saturation is what a correctly-posed ABC problem looks like.~~
**CORRECTED 2026-10-21 (gh:#985)** — that was a single seed. Over seeds
1..=30 the mean posterior sd is 0.5910 +/- 0.0028 (SE) at `epsilon = 0.5`,
0.3589 +/- 0.0026 at 0.2 and 0.2621 +/- 0.0096 at 0.05: tightening the
tolerance does shrink the posterior, but at 0.05 it goes on **past** the
exact 0.288195 (2.7 SE below it), so it does not saturate there and small
tolerances are over-confident on average even with matched sample sizes.

Both rows of that table are pinned by tests in this module, so the failure
mode cannot quietly come back.

# Frozen randomness, and why it is not optional

A stochastic simulator makes the ABC likelihood stochastic too: call it
twice at the same `theta` and you get two different numbers. Dropping such a
function into a Metropolis acceptance ratio silently changes what the chain
converges to — it becomes a pseudo-marginal method, valid only under
conditions most users never check, and a chain that lands on a lucky draw
can stick there indefinitely.

This module therefore **derives the simulator's seed from `theta` itself**,
so the approximate likelihood is an honest deterministic function: the same
parameter vector always produces the same synthetic sample and the same
distance. The cost is that the noise becomes a fixed, rough surface rather
than fresh noise each call; the benefit is that the sampler is doing what
its convergence theory says it is doing. [`AbcLikelihood::simulations`] sets
how many model runs are averaged into each evaluation, which is the knob for
smoothing that surface.

# References

- M. A. Beaumont, W. Zhang and D. J. Balding (2002). Approximate Bayesian
  computation in population genetics. *Genetics, 162*(4), 2025–2035. doi:
  [10.1093/genetics/162.4.2025](https://doi.org/10.1093/genetics/162.4.2025)
- A. Lye, S. Ferson and S. Xiao (2024). Comparison between distance
  functions for Approximate Bayesian Computation towards stochastic model
  updating and model validation under limited data. *ASCE-ASME Journal of
  Risk and Uncertainty in Engineering Systems Part A: Civil Engineering,
  10*, 03124001. doi:
  [10.1061/AJRUA6.RUENG-1223](https://doi.org/10.1061/AJRUA6.RUENG-1223)
- S. Bi, M. Broggi and M. Beer (2019). The role of the Bhattacharyya
  distance in stochastic model updating. *Mechanical Systems and Signal
  Processing, 117*, 437–452. doi:
  [10.1016/j.ymssp.2018.08.017](https://doi.org/10.1016/j.ymssp.2018.08.017)

Independent implementations from the published definitions — see the
provenance note in [`crate::bayesian`].

```rust
pub mod abc { /* ... */ }
```

### Types

#### Enum `AbcKernel`

How a distance is turned into an approximate log-likelihood.

All three are the standard ABC kernels. They differ in how sharply they
punish a model run that misses, and — more importantly for a sampler —
in whether they can return `-inf`.

```rust
pub enum AbcKernel {
    Uniform {
        epsilon: f64,
    },
    Gaussian {
        epsilon: f64,
    },
    Epanechnikov {
        epsilon: f64,
    },
}
```

##### Variants

###### `Uniform`

The textbook ABC indicator: `ln L = 0` if `distance <= epsilon`, and
`-inf` otherwise.

The definition every ABC derivation starts from, and the worst choice
for a gradient-free sampler: the approximate likelihood is flat inside
the ball and impossible outside it, so there is nothing to climb. A
transitional sampler whose whole initial population is outside the ball
cannot start. Use it with [`rejection_abc`], not with MCMC.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `epsilon` | `f64` | Tolerance radius, strictly positive, in the distance's own units. |

###### `Gaussian`

Gaussian kernel: `ln L = -0.5 * (distance / epsilon)^2`.

Never `-inf`, so there is always a gradient to follow back towards the
data. This is the kernel the distance-based stochastic model updating
frameworks use, and the default choice here.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `epsilon` | `f64` | Kernel width, strictly positive, in the distance's own units. |

###### `Epanechnikov`

Epanechnikov kernel: `ln L = ln(1 - (distance / epsilon)^2)` inside the
ball, `-inf` outside.

The minimum-variance kernel in the classical density-estimation sense,
and a middle ground: it falls off smoothly like the Gaussian but has the
Uniform's hard cut-off. Same starting problem as `Uniform` if the
initial population is all outside the ball.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `epsilon` | `f64` | Tolerance radius, strictly positive, in the distance's own units. |

##### Implementations

###### Methods

- ```rust
  pub fn epsilon(self: &Self) -> f64 { /* ... */ }
  ```
  The kernel's width or tolerance.

- ```rust
  pub fn has_hard_cutoff(self: &Self) -> bool { /* ... */ }
  ```
  Whether this kernel can return [`f64::NEG_INFINITY`] for a finite

- ```rust
  pub fn ln_likelihood(self: &Self, distance: f64) -> Result<f64> { /* ... */ }
  ```
  Converts a distance into an approximate natural log-likelihood.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AbcLikelihood`

Everything needed to turn a simulator into an approximate log-likelihood.

Built once and handed to [`abc_ln_likelihood`], which produces the closure a
sampler consumes.

```rust
pub struct AbcLikelihood {
    pub observed: Vec<Vec<f64>>,
    pub metric: crate::distance::DistanceMetric,
    pub kernel: AbcKernel,
    pub simulations: usize,
    pub seed: i64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `observed` | `Vec<Vec<f64>>` | The observed data: rows of equal width, one row per measurement. |
| `metric` | `crate::distance::DistanceMetric` | How the synthetic sample is compared with [`observed`](Self::observed). |
| `kernel` | `AbcKernel` | How the resulting distance becomes a log-likelihood. |
| `simulations` | `usize` | How many independent model runs are pooled into each evaluation.<br><br>One is enough when the simulator itself returns a whole sample. Raise it<br>when the simulator returns a single realisation per call, or to smooth<br>the frozen-noise surface described in the module documentation, at<br>proportional cost. |
| `seed` | `i64` | Base seed for the simulator. Mixed with a hash of the parameter vector,<br>so a given `theta` always gets the same simulator stream — see the<br>module documentation on frozen randomness. |

##### Implementations

###### Methods

- ```rust
  pub fn new(observed: Vec<Vec<f64>>, metric: DistanceMetric, kernel: AbcKernel, simulations: usize, seed: i64) -> Result<Self> { /* ... */ }
  ```
  Builds and validates the configuration.

- ```rust
  pub fn width(self: &Self) -> usize { /* ... */ }
  ```
  Width of one observation row.

- ```rust
  pub fn simulator_seed(self: &Self, theta: &[f64], run: usize) -> u64 { /* ... */ }
  ```
  The simulator stream seed for a given parameter vector.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `RejectionAbcResult`

The result of a rejection-ABC run.

```rust
pub struct RejectionAbcResult {
    pub samples: Vec<Vec<f64>>,
    pub distances: Vec<f64>,
    pub proposals: usize,
    pub acceptance_rate: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `samples` | `Vec<Vec<f64>>` | Accepted parameter vectors — the approximate posterior sample. |
| `distances` | `Vec<f64>` | The distance achieved by each accepted vector, aligned with<br>[`samples`](Self::samples). |
| `proposals` | `usize` | Proposals drawn from the prior, accepted or not. |
| `acceptance_rate` | `f64` | Fraction of proposals accepted, in `[0, 1]`.<br><br>The number that decides whether rejection ABC was the right tool. Below<br>about 1 % the answer is no: move to [`abc_ln_likelihood`] with a<br>transitional sampler. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `abc_ln_likelihood`

Builds an approximate log-likelihood closure from a simulator.

`simulator` takes a parameter vector and a mutable LCG stream state, and
returns a sample of model outputs with the same row width as the observed
data. The returned closure has the `Fn(&[f64]) -> f64` shape that
[`crate::bayesian::tmcmc`] and [`crate::bayesian::temcmc`] consume, so the
whole distance-based stochastic model updating framework is:

```no_run
# use raffles::abc::{abc_ln_likelihood, AbcKernel, AbcLikelihood};
# use raffles::bayesian::{temcmc, IndependentPrior, TransitionalConfig};
# use raffles::distance::DistanceMetric;
# fn run(prior: &IndependentPrior, observed: Vec<Vec<f64>>) -> raffles::Result<()> {
let abc = AbcLikelihood::new(
    observed,
    DistanceMetric::JensenShannon { bins: None },
    AbcKernel::Gaussian { epsilon: 0.05 },
    1,
    20_260_916,
)?;
let ln_likelihood = abc_ln_likelihood(abc, |theta, seed| {
    // run your model at `theta`, using `seed` for any randomness
    # let _ = (theta, seed);
    vec![vec![0.0]]
});
let config = TransitionalConfig::temcmc_defaults(2_000, 20_260_916)?;
let posterior = temcmc(prior, ln_likelihood, &config)?;
# let _ = posterior;
# Ok(())
# }
```

A simulator whose output rows are the wrong width, or which returns no rows
at all, yields [`f64::NEG_INFINITY`] for that parameter vector — the
sampler treats it as an impossible point rather than failing the run, which
is the right behaviour for a model that legitimately has no solution in part
of the parameter space.

```rust
pub fn abc_ln_likelihood<S>(config: AbcLikelihood, simulator: S) -> impl Fn(&[f64]) -> f64
where
    S: Fn(&[f64], &mut u64) -> Vec<Vec<f64>> { /* ... */ }
```

#### Function `rejection_abc`

Plain rejection ABC: draw from the prior, simulate, keep the close ones.

Every proposal is independent, so the result is an exact sample from the
ABC posterior for the given tolerance — no burn-in, no autocorrelation, no
convergence question. That makes it the right *reference* against which to
check an MCMC-based ABC run, which is what this module's tests use it for,
even though its acceptance rate makes it impractical on a real problem.

The tolerance comes from `config.kernel.epsilon()`, and acceptance is the
kernel's own rule: a `Gaussian` kernel accepts stochastically with
probability `exp(ln L)`, while `Uniform` and `Epanechnikov` have their hard
cut-offs.

# Errors

[`RafflesError::InvalidParameter`] if `proposals` is zero, or if the prior
and the kernel are inconsistent in a way the constructors already reject.

```rust
pub fn rejection_abc<S>(prior: &crate::bayesian::IndependentPrior, config: &AbcLikelihood, simulator: S, proposals: usize, seed: i64) -> crate::Result<RejectionAbcResult>
where
    S: Fn(&[f64], &mut u64) -> Vec<Vec<f64>> { /* ... */ }
```

## Module `bayesian`

Bayesian model updating — priors, likelihoods and the samplers that turn
them into a posterior.

# What this module is for

Given a prior belief about a model's parameters and a likelihood that says
how well a parameter vector explains measured data, produce samples from
the posterior

```text
p(theta | D)  =  p(D | theta) * p(theta) / p(D)
```

and an estimate of the evidence `p(D)` (the normalising constant, also
called the marginal likelihood). The evidence is what lets two competing
models be compared, so a sampler that produces it is worth more here than
one that only produces posterior samples.

# Provenance — written from the published papers, under a GPL-3.0 grant

**Read this before adding anything to this module.**

The algorithms here were requested by way of Adolphus Lye's MATLAB and R
repositories (workspace issue #158). **Adolphus Lye is the copyright
holder of all seven of them and has granted GPL-3.0**, stated directly to
the workspace maintainer on 2026-09-09 and reaffirmed on 2026-09-16. That
is the same licence as this crate, so there is no compatibility question
and no one-way constraint of the kind that applies to the RAVEN port — a
code-level port from any of the seven is permitted.

One provenance gap remains, and it is recorded rather than glossed: six of
the seven repositories carry **no `LICENSE` file**, so a third party
reading this repository cannot confirm the grant from the sources
themselves. That is a checkability gap, not a legal one. It is tracked as
`op-dwqw.1`, the grant is recorded in this crate's `NOTICE`, and the fix is
one commit per repository by the author.

**What is in this module was nonetheless written from the published
papers, not from that MATLAB**, and that is the honest provenance of these
particular files — the grant arrived first, but the papers were the
specification actually used. Every item therefore cites the paper that
defines its algorithm, with a DOI, and **carries no "ported from"
attribution header, because nothing was taken**. [`case_studies`] is the
exception: it *is* a port, from `Bayesian-Model-Updating-Tutorials`, and
carries the header accordingly.

The rule for anything added from here on:

- **A file genuinely derived from one of the seven repositories carries the
  attribution header** in `crates/raffles/CLAUDE.md` — upstream repo, file,
  commit, copyright holder, licence — naming GPL-3.0 and this grant.
- **A file written from a paper does not**, and cites the DOI instead.
  Do not attach a "ported from" header to work that was not ported.
- Reading the upstream MATLAB/R to **cross-check** an existing independent
  implementation is now open, and is the more valuable use of it: a
  disagreement between the Rust and the author's own code is a finding,
  and neither being able to look was a real limitation on the V&V here.

# That cross-check has been done — read it before changing this module

`TEMCMCsampler.m`, `TMCMCsampler.m` and `EMCMCsampler.m` were run under GNU
Octave and compared against this module on problems with closed-form
answers. Eleven differences were found, four of them checked by tests in
[`transitional`]. Full methodology, every patch made to the upstream to get
it running, and all measured numbers:
**`crates/raffles/docs/cross-check-against-upstream.md`**.

The three results that should change how you read this module:

- **TEMCMC agrees** with his to under one standard error on the posterior
  mean, posterior standard deviation and log evidence, over 5 seeds each.
  The evidence increment agrees to 8.9e-16.
- **This crate has no adaptive proposal scaling and his does** (finding
  F3). That is a real gap, and whether to close it is the crate owner's
  decision, not an implementer's — it would change sampler behaviour and
  invalidate every recorded V&V number here.
- **A single-seed V&V number is worth much less than it looks.** The
  posterior standard deviation recorded on
  `temcmc_matches_the_conjugate_normal_posterior` is 2.4 % above the closed
  form; over 5 seeds the same configuration gives +0.14 %. The rest of this
  module's single-run numbers deserve the same scepticism.

# What is here

- [`IndependentPrior`] — a prior over several parameters, each with its own
  marginal from [`crate::distributions`], independent of the others.
- [`mcmc`] — the two Markov-chain moves everything else is built on:
  random-walk Metropolis–Hastings, and the affine-invariant ensemble
  ("stretch") move.
- [`transitional`] — Transitional MCMC (TMCMC) and Transitional Ensemble
  MCMC (TEMCMC): tempered sequential samplers that walk the posterior in
  stages and return the log-evidence as a by-product.
- [`case_studies`] — small problems with exactly known answers, ported from
  the one repository in that set that carries a licence. Includes a bimodal
  posterior whose two modes are computable in closed form, which is the
  failure a moment-based check cannot see.
- [`koh`] — Kennedy–O'Hagan calibration with a GP model discrepancy,
  translated from LANL's SEPIA (BSD-3, gh:#1180) and sampled with
  [`transitional`]; differential-tested against SEPIA.

# Conventions used throughout

- **Everything is in natural logarithms.** A likelihood evaluated at a
  badly-fitting parameter vector underflows `f64` long before the sampler
  is finished with it, so the API takes and returns `ln L(theta)`, never
  `L(theta)`. A log-likelihood of [`f64::NEG_INFINITY`] is legal and means
  "impossible"; a `NaN` is not, and is treated as impossible with the
  diagnostic counter [`SamplerDiagnostics::non_finite_likelihoods`]
  incremented so it cannot pass silently.
- **The caller supplies the likelihood as a closure**, `Fn(&[f64]) -> f64`,
  taking the parameter vector and returning its natural log-likelihood.
  Generic parameters rather than `Box<dyn Fn>` — the workspace design rules
  forbid trait objects, and a generic is also faster and keeps the closure's
  captured state on the caller's side.
- **Randomness is explicit.** Every sampler takes a `seed: i64` and uses
  [`crate::samplers::stream_seed`] over the workspace's OpenMC 64-bit LCG.
  The same seed and the same configuration produce a byte-identical result.
  There is no thread-local RNG and no time-seeded default anywhere here.
- **Parameters are plain `f64` vectors** in whatever units the caller's
  model uses; RAFFLES never interprets them physically. See the crate-level
  note on units.

# Verification

Each sampler's own doc comment states the methodology and the measured
results of its verification tests, as the workspace V&V rule requires. The
backbone test is the **conjugate Normal–Normal problem**, where the
posterior mean, the posterior variance *and* the evidence all have closed
forms — so a sampler can be checked on all three at once rather than on
posterior moments alone. See [`transitional`] for the numbers.

None of this has been through human V&V. It is AI-assisted draft material
under the workspace `RESPONSIBLE_USE.md` rules until the maintainer and the
crate owner review it.

```rust
pub mod bayesian { /* ... */ }
```

### Modules

## Module `case_studies`

Case studies from the Bayesian model-updating tutorials — small problems
with exactly known answers.

# Why these are worth having

Every sampler in [`super`] is verified against a conjugate Normal–Normal
problem, which is the right backbone test but is also the easiest possible
posterior: unimodal, symmetric, and in one dimension. These two case studies
are harder in specific, useful ways, and both still have answers that can be
written down rather than estimated:

- [`spring_force`] — a linear static spring-mass system. The posterior over
  the stiffness is exactly Gaussian, so mean and variance are checkable in
  closed form. It is the sanity case.
- [`two_by_two_eigenvalues`] — the eigenvalues of a 2x2 structural matrix.
  Its posterior is **bimodal**: two distinct parameter vectors produce the
  same pair of eigenvalues, and both are computable exactly by solving a
  quadratic. A sampler that finds only one of them has failed in a way a
  moment-based check would not notice.

The second is the one that earns its place. Collapsing onto a single mode is
the characteristic failure of a poorly-mixing sampler, and it looks like
success from every summary statistic.

```rust
pub mod case_studies { /* ... */ }
```

### Functions

#### Function `spring_force`

Force in a 1-degree-of-freedom linear spring: `F = -k * x`.

- `stiffness` — `k`, in newtons per metre. Physically positive, though
  nothing here enforces it: a sampler exploring a prior that allows negative
  values should get an answer rather than a panic.
- `displacement` — `x`, in metres.

Returns the force in newtons. The sign convention is the upstream's: the
restoring force opposes the displacement.

```rust
pub fn spring_force(stiffness: f64, displacement: f64) -> f64 { /* ... */ }
```

#### Function `two_by_two_eigenvalues`

Eigenvalues of the 2x2 structural matrix parameterised by `theta`.

The closed form the upstream uses:

```text
lambda_1 = 0.5 * ((t1 + 2 t2) + sqrt(t1^2 + 4 t2^2))
lambda_2 = 0.5 * ((t1 + 2 t2) - sqrt(t1^2 + 4 t2^2))
```

Returns `(lambda_1, lambda_2)` with `lambda_1 >= lambda_2` by construction.

# The bimodality, and where it comes from

The map is two-to-one almost everywhere. Writing `s = lambda_1 + lambda_2`
and `d = lambda_1 - lambda_2`, the parameters satisfy

```text
t1 + 2 t2 = s        (a line)
t1^2 + 4 t2^2 = d^2  (an ellipse)
```

and a line generally cuts an ellipse twice. So a measured eigenvalue pair is
explained equally well by **two** parameter vectors, and the posterior has
two modes. [`eigenvalue_partner`] computes the other one.

This is not a defect of the model — it is genuine non-identifiability, the
thing Bayesian model updating is supposed to *reveal* rather than average
over. A point estimate of `theta` here is meaningless on its own.

# Panics

Never. A negative discriminant is impossible: `t1^2 + 4 t2^2` is a sum of
squares.

```rust
pub fn two_by_two_eigenvalues(theta: &[f64]) -> (f64, f64) { /* ... */ }
```

#### Function `eigenvalue_partner`

The *other* parameter vector that produces the same eigenvalues as `theta`.

Solves the line-meets-ellipse system in [`two_by_two_eigenvalues`] for its
second root. Returns `None` when the line is tangent to the ellipse, which
is the measure-zero case where the two modes coincide and the problem is
identifiable after all.

Used by this module's tests to state exactly where the second posterior mode
must be, rather than looking for "some other cluster".

```rust
pub fn eigenvalue_partner(theta: &[f64]) -> Option<Vec<f64>> { /* ... */ }
```

## Module `koh`

**Kennedy–O'Hagan calibration with model discrepancy** (gh:#1180),
translated from SEPIA's scalar-output model (the GPMSA formulation of
Higdon et al. 2004, 2008):

```text
y_obs(x) = η(x, θ) + δ(x) + ε,   η ~ GP over (x, t) fitted to simulator runs,
                                 δ ~ GP over x,  ε ~ N(0, 1/λ_Os)
```

(Kennedy & O'Hagan 2001, *J. R. Stat. Soc. B* 63:425.)

- [`KohData`]: SEPIA's transforms (unit cube, standardised y).
- [`KohModel`]: [`KohForm`] × [`KohPriors`]; [`KohModel::log_lik`],
  [`KohModel::log_prior`], [`KohModel::log_post`], [`KohModel::predict`]
  (the conditional Gaussian of (δ, η) at new x), [`EmulatorFactor`] for a
  fixed emulator.
- [`KohModel::sample`]: raffles' TMCMC on SEPIA's posterior (not a port).

**Upstream findings that shape this module** (V&V record): SEPIA's own
univariate setup cannot carry δ (only its likelihood and its dense
predictor still have the GPMSA branch), and that dense predictor's
scalar + δ branch places one block wrongly; the port follows upstream's
own comment there.

The typed route from a `ParametricModel` and a `Dataset` is
[`crate::fidelity::koh`]. V&V:
`crates/raffles/verification_and_validation/koh_calibration_gh1180.md`,
differential tests `tests/koh_sepia_gh1180.rs`.

```rust
pub mod koh { /* ... */ }
```

### Re-exports

#### Re-export `pairwise_sum`

```rust
pub use data::pairwise_sum;
```

#### Re-export `KohData`

```rust
pub use data::KohData;
```

#### Re-export `cov_cross`

```rust
pub use model::cov_cross;
```

#### Re-export `cov_self`

```rust
pub use model::cov_self;
```

#### Re-export `EmulatorFactor`

```rust
pub use model::EmulatorFactor;
```

#### Re-export `KohForm`

```rust
pub use model::KohForm;
```

#### Re-export `KohModel`

```rust
pub use model::KohModel;
```

#### Re-export `KohParams`

```rust
pub use model::KohParams;
```

#### Re-export `KohPredictive`

```rust
pub use model::KohPredictive;
```

#### Re-export `KohPriors`

```rust
pub use model::KohPriors;
```

#### Re-export `ParamPrior`

```rust
pub use model::ParamPrior;
```

#### Re-export `PriorDist`

```rust
pub use model::PriorDist;
```

#### Re-export `KohFree`

```rust
pub use sample::KohFree;
```

#### Re-export `KohPosterior`

```rust
pub use sample::KohPosterior;
```

#### Re-export `KohSampleConfig`

```rust
pub use sample::KohSampleConfig;
```

## Module `mcmc`

Markov-chain moves: random-walk Metropolis–Hastings and the
affine-invariant ensemble ("stretch") move.

Both kernels here are *moves*, not samplers: each advances a state (or a
population of states) one step against a target the caller supplies as a
log-density. [`super::transitional`] builds the actual samplers on top of
them. Keeping the move separate from the tempering schedule is what makes
TMCMC and TEMCMC one algorithm with two kernels rather than two algorithms.

# The target

Every function here takes the target as `ln_target: Fn(&[f64]) -> f64`,
the natural log of a density known only up to a constant. That is all a
Metropolis acceptance ratio needs, and it is what a tempered posterior
`ln p(theta) + beta * ln L(theta)` naturally is.

`-inf` means "impossible" and is always rejected. `NaN` from the caller's
target is treated as `-inf`: see [`super::eval_ln_likelihood`] for why that
policy exists and how it is counted.

# Why both kernels

**Metropolis–Hastings** with a multivariate-normal proposal is the
classical choice and is what Ching and Chen's TMCMC specifies. Its weakness
is that its performance depends entirely on the proposal covariance
matching the target's shape — on an anisotropic or strongly correlated
target with a poorly-scaled proposal it crawls.

**The affine-invariant ensemble move** (Goodman and Weare, 2010) removes
that tuning problem: a population of walkers proposes moves along the lines
joining pairs of walkers, so the proposal inherits the target's own shape.
Its performance is *invariant* under any affine transformation of the
parameter space, which is exactly the property an engineering model-updating
problem needs, where one parameter may be a stiffness of order 1e9 and the
next a damping ratio of order 1e-3.

# References

- J. Goodman and J. Weare (2010). Ensemble samplers with affine invariance.
  *Communications in Applied Mathematics and Computational Science, 5*(1),
  65–80. doi: [10.2140/camcos.2010.5.65](https://doi.org/10.2140/camcos.2010.5.65)
- W. K. Hastings (1970). Monte Carlo sampling methods using Markov chains
  and their applications. *Biometrika, 57*(1), 97–109. doi:
  [10.1093/biomet/57.1.97](https://doi.org/10.1093/biomet/57.1.97)
- A. Lye, A. Cicirello and E. Patelli (2021). Sampling methods for solving
  Bayesian model updating problems: A tutorial. *Mechanical Systems and
  Signal Processing, 159*, 107760. doi:
  [10.1016/j.ymssp.2021.107760](https://doi.org/10.1016/j.ymssp.2021.107760)

These are independent implementations from the published algorithms. See
the provenance note in [`super`] for why that matters here.

```rust
pub mod mcmc { /* ... */ }
```

### Types

#### Struct `ChainState`

One state of a Markov chain: a parameter vector and the target's log-density
there.

Carrying the log-density with the point is not an optimisation detail, it is
what makes the chain correct in the presence of an expensive likelihood:
re-evaluating the target at the current point every step would double the
cost and, for a stochastic likelihood, would also quietly change the
invariant distribution.

```rust
pub struct ChainState {
    pub theta: Vec<f64>,
    pub ln_target: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `theta` | `Vec<f64>` | The parameter vector, in the order fixed by the prior. |
| `ln_target` | `f64` | Natural log of the target density at `theta`, up to the target's own<br>unknown constant. May be [`f64::NEG_INFINITY`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new<T>(theta: Vec<f64>, ln_target: &T) -> Self
where
    T: Fn(&[f64]) -> f64 { /* ... */ }
  ```
  Builds a state by evaluating `ln_target` at `theta`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MetropolisHastings`

Random-walk Metropolis–Hastings with a multivariate-normal proposal.

The proposal is `theta* ~ N(theta, Sigma)` with `Sigma` supplied as its
lower Cholesky factor, so the caller (in practice the TMCMC stage loop)
controls the scale and the correlation of the step. Because the proposal is
symmetric, the Hastings ratio reduces to the target ratio.

# Why the Cholesky factor and not the covariance

A sampler takes thousands of steps from one covariance matrix. Factorising
once per stage instead of once per step is the obvious saving, but the real
reason is correctness: the factorisation is where a non-positive-definite
covariance is *detected*, and detecting it once per stage (where it can be
regularised and counted) is far better than discovering it in the middle of
a chain.

```rust
pub struct MetropolisHastings {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn from_cholesky(cholesky: Vec<Vec<f64>>) -> Result<Self> { /* ... */ }
  ```
  Builds the kernel from the lower Cholesky factor of the proposal

- ```rust
  pub fn dimension(self: &Self) -> usize { /* ... */ }
  ```
  Number of parameters the kernel proposes in.

- ```rust
  pub fn step<T>(self: &Self, state: &mut ChainState, ln_target: &T, seed: &mut u64) -> bool
where
    T: Fn(&[f64]) -> f64 { /* ... */ }
  ```
  Advances `state` one Metropolis–Hastings step, in place.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `EnsembleMove`

The affine-invariant ensemble ("stretch") move of Goodman and Weare (2010).

A population of `n` walkers explores the target together. To move walker
`k`, a partner `j != k` is drawn from the *complementary* half of the
population and the proposal is

```text
Y = X_j + z * (X_k - X_j),     z ~ g(z) on [1/a, a],  g(z) proportional to 1/sqrt(z)
```

accepted with probability `min(1, z^(d-1) * q(Y) / q(X_k))`. Because the
proposal is built only from differences of walker positions, the whole
kernel commutes with any affine map of the parameter space: rescaling a
parameter from metres to millimetres, or rotating a correlated pair, leaves
the sampler's behaviour unchanged. That is the property the name refers to,
and it is why this kernel needs no proposal covariance and no tuning.

# The complementary-halves split matters

Updating a walker using a partner from the *same* half that is itself being
updated in the same sweep breaks detailed balance. The population is
therefore split in two: the first half is updated using partners from the
second, then the second using the (already updated) first. Goodman and
Weare's paper and the `emcee` implementation both do this; it is not an
optimisation, and removing it silently biases the answer.

# Population size

`n` must be at least `2 * d + 2` for the ensemble to span the parameter
space with a complementary half of size at least `d + 1`. A smaller
population confines the walkers to a lower-dimensional affine subspace of
the parameter space, permanently — this is the classic failure mode of
ensemble samplers and it fails *silently*, producing a plausible-looking
chain that never explores some directions. [`EnsembleMove::new`] rejects it
rather than letting it happen.

```rust
pub struct EnsembleMove {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: f64) -> Result<Self> { /* ... */ }
  ```
  Builds the move with stretch parameter `a`.

- ```rust
  pub fn a(self: &Self) -> f64 { /* ... */ }
  ```
  The stretch parameter.

- ```rust
  pub fn minimum_population(dimension: usize) -> usize { /* ... */ }
  ```
  Smallest population size that can explore `dimension` parameters.

- ```rust
  pub fn sweep<T>(self: &Self, population: &mut [ChainState], ln_target: &T, seed: &mut u64) -> Result<usize>
where
    T: Fn(&[f64]) -> f64 { /* ... */ }
  ```
  Advances an entire population one sweep, in place.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `raven_mcmc`

**RAVEN's Metropolis and adaptive Metropolis samplers** (gh:#1193, the
RAVEN route for calibration, gh:#1180).

A translation of `Samplers/MCMC/`: a random-walk Metropolis chain whose
proposal scale is tuned on the fly with PyMC3's acceptance table, and
Haario-style adaptive Metropolis, whose Gaussian proposal covariance
follows the chain.

# How this relates to [`super::MetropolisHastings`]

The existing kernel ([`super::mcmc`]) is written from the papers, and with
a fixed scale and unbounded priors it is the same Markov kernel as RAVEN's
Metropolis with a diagonal Cholesky factor `0.2 σ_i s`. What differs:

| | `MetropolisHastings::step` | RAVEN (`raven_mcmc`) |
|---|---|---|
| proposal | `θ + L z`, any `L` | `θ + s · d`, `d_i ~ N(0, 0.2 σ_i)` (Metropolis) or `d ~ N(0, λΣ)` (adaptive) |
| scale | fixed | tuned every `tune_interval` steps (PyMC3 table); adaptive: Robbins–Monro `λ` and ensemble covariance |
| target | one `ln_target` | prior and likelihood kept separate; the log-ratio is a sum of per-parameter `ln pdf` differences |
| outside the prior's support | proposal rejected (`-inf`) | proposal **clamped onto the bound** and evaluated there |
| NaN log-ratio | rejected | `min(0, NaN) = 0`: **accepted** (Python `min`) |
| the first step | — | step 1 evaluates the initial point; acceptance rate counts it as accepted |

So `raven_mcmc` does not reuse `MetropolisHastings`: the clamp, the
separate prior and likelihood, the tuning and the replayable draws are
what make it RAVEN's, and bit-identity with RAVEN needs RAVEN's operation
order. The prior is the crate's [`IndependentPrior`].

# V&V

Record: `crates/raffles/verification_and_validation/raven_routes_gh1193.md`.
Against RAVEN itself (`Draws::Replay` on RAVEN's recorded draws) and
against closed-form posteriors (`Draws::Lcg`): see the tests below.

```rust
pub mod raven_mcmc { /* ... */ }
```

### Types

#### Enum `RavenMcmcKind`

Which RAVEN sampler.

```rust
pub enum RavenMcmcKind {
    Metropolis,
    AdaptiveMetropolis {
        adaptive_interval: usize,
    },
}
```

##### Variants

###### `Metropolis`

`Samplers/MCMC/Metropolis.py`: one independent Gaussian proposal per
parameter, `N(0, 0.2 σ_prior)`, times the tuned scale.

###### `AdaptiveMetropolis`

`Samplers/MCMC/AdaptiveMetropolis.py`: one multivariate Gaussian
proposal `N(0, λ Σ)`, `λ` and `Σ` adapted after the burn-in.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `adaptive_interval` | `usize` | Steps between covariance updates (RAVEN's `<adaptiveInterval>`,<br>default 20). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `LikelihoodForm`

What the likelihood closure returns (RAVEN's `<likelihood log="...">`).

```rust
pub enum LikelihoodForm {
    Log,
    Linear,
}
```

##### Variants

###### `Log`

The natural log of the likelihood (`log="True"`).

###### `Linear`

The likelihood itself (RAVEN's default, `log="False"`); its log is
taken when the ratio is formed.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `BoundHandling`

What to do with a proposal outside a prior's support.

```rust
pub enum BoundHandling {
    ClampAsRaven,
    Reject,
}
```

##### Variants

###### `ClampAsRaven`

RAVEN's rule (`Metropolis.py:137-142`, `AdaptiveMetropolis.py:274-285`):
move it onto the bound and evaluate it there. This leaves an atom on
the bound, so the chain does not sample the posterior when the
posterior has mass near a bound (V&V record, A3).

###### `Reject`

Reject the proposal (the Metropolis rule). Not in RAVEN.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `RavenMcmcConfig`

Configuration, with RAVEN's names and defaults.

```rust
pub struct RavenMcmcConfig {
    pub kind: RavenMcmcKind,
    pub limit: usize,
    pub burn_in: usize,
    pub tune: bool,
    pub tune_interval: usize,
    pub initial: Option<Vec<f64>>,
    pub likelihood: LikelihoodForm,
    pub bounds: BoundHandling,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `RavenMcmcKind` | Which sampler. |
| `limit` | `usize` | Total steps, the initial point included (`<limit>`). |
| `burn_in` | `usize` | Leading steps left out of [`RavenMcmcResult::samples`] (`<burnIn>`,<br>default 0). For adaptive Metropolis it is also where the scale tuning<br>stops and the adaptation starts. |
| `tune` | `bool` | Tune the proposal scale (`<tune>`, default true). |
| `tune_interval` | `usize` | Steps between scale tunings (`<tuneInterval>`, default 100). |
| `initial` | `Option<Vec<f64>>` | The initial point (`<initial>`); `None` draws it from the prior. |
| `likelihood` | `LikelihoodForm` | What the likelihood closure returns. |
| `bounds` | `BoundHandling` | What happens at a prior's bounds. |

##### Implementations

###### Methods

- ```rust
  pub fn metropolis(limit: usize) -> Self { /* ... */ }
  ```
  RAVEN's `Metropolis` defaults: burn-in 0, tuning on every 100 steps,

- ```rust
  pub fn adaptive_metropolis(limit: usize) -> Self { /* ... */ }
  ```
  RAVEN's `AdaptiveMetropolis` defaults: as [`metropolis`](Self::metropolis),

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Draws`

Where the randomness comes from.

```rust
pub enum Draws {
    Lcg {
        seed: u64,
    },
    Replay {
        increments: Vec<Vec<f64>>,
        uniforms: Vec<f64>,
    },
}
```

##### Variants

###### `Lcg`

The workspace LCG (`petir::rng::lcg`), from this seed: one standard
normal per parameter for the proposal, then one uniform for the
acceptance, every step after the first.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `seed` | `u64` | Stream state. |

###### `Replay`

Recorded draws, consumed in order: for step `k = 2..=limit`,
`increments[k - 2]` is the proposal draw `d` (before the scale) and
`uniforms[k - 2]` the acceptance uniform. This is how the port is
compared with RAVEN; the initial point must then be given.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `increments` | `Vec<Vec<f64>>` | Proposal draws, one vector of length `d` per step after the first. |
| `uniforms` | `Vec<f64>` | Acceptance uniforms, one per step after the first. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `RavenStep`

One step of the chain, as RAVEN's loop leaves it.

```rust
pub struct RavenStep {
    pub proposed: Vec<f64>,
    pub current: Vec<f64>,
    pub alpha: Option<f64>,
    pub accepted: bool,
    pub accept_rate: f64,
    pub scaling: f64,
    pub lambda: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `proposed` | `Vec<f64>` | The evaluated point (after the clamp). |
| `current` | `Vec<f64>` | The chain's state after the decision. |
| `alpha` | `Option<f64>` | `min(0, log ratio)`; `None` at step 1. |
| `accepted` | `bool` | Whether the proposal was accepted (false at step 1, as RAVEN's<br>`_accepted` starts). |
| `accept_rate` | `f64` | RAVEN's running acceptance rate, `(1 + accepted) / counter`. |
| `scaling` | `f64` | The proposal scale after this step's tuning. |
| `lambda` | `Option<f64>` | Adaptive Metropolis's `λ` after this step's update. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `RavenMcmcResult`

A finished chain.

```rust
pub struct RavenMcmcResult {
    pub steps: Vec<RavenStep>,
    pub burn_in: usize,
    pub ensemble_mean: Option<Vec<f64>>,
    pub ensemble_cov: Option<Vec<Vec<f64>>>,
    pub nan_log_ratios: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `steps` | `Vec<RavenStep>` | Every step, the first included. |
| `burn_in` | `usize` | The burn-in the run used. |
| `ensemble_mean` | `Option<Vec<f64>>` | Adaptive Metropolis's final ensemble mean. |
| `ensemble_cov` | `Option<Vec<Vec<f64>>>` | Adaptive Metropolis's final ensemble covariance (row-major, `d × d`). |
| `nan_log_ratios` | `usize` | Steps whose log ratio was NaN and which RAVEN's `min(0, ·)` therefore<br>accepted. Non-zero means the likelihood or prior has a hole. |

##### Implementations

###### Methods

- ```rust
  pub fn samples(self: &Self) -> Vec<Vec<f64>> { /* ... */ }
  ```
  The chain's states after the burn-in: steps with `counter > burn_in`,

- ```rust
  pub fn accept_rate(self: &Self) -> f64 { /* ... */ }
  ```
  Final acceptance rate.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `tune_scaling`

**Attributes:**

- `MustUse { reason: None }`

PyMC3's scale-tuning table, as RAVEN copies it (`MCMC.tuneScalingParam`).

```rust
pub fn tune_scaling(scale: f64, accept_rate: f64) -> f64 { /* ... */ }
```

#### Function `raven_mcmc`

Runs RAVEN's Metropolis or adaptive Metropolis on `prior` and the
caller's `likelihood` (in the form `config.likelihood` says).

# Errors

[`RafflesError::InvalidParameter`] for an invalid configuration;
[`RafflesError::DimensionMismatch`] for an initial point or a replayed
draw of the wrong length, or a replay that runs out;
[`RafflesError::InvalidParameter`] for a replay without an initial point,
or an adaptive proposal covariance that is not positive definite (LCG
route only).

```rust
pub fn raven_mcmc<L>(prior: &super::IndependentPrior, config: &RavenMcmcConfig, likelihood: &L, draws: Draws) -> crate::Result<RavenMcmcResult>
where
    L: Fn(&[f64]) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `RAVEN_OPTIMAL_ACCEPTANCE`

RAVEN's optimum acceptance rate for adaptive Metropolis
(`AdaptiveMetropolis._optAlpha`).

```rust
pub const RAVEN_OPTIMAL_ACCEPTANCE: f64 = 0.234;
```

#### Constant `RAVEN_DEFAULT_PROPOSAL_FRACTION`

RAVEN's default proposal standard deviation, as a fraction of the prior's
untruncated standard deviation (`MCMC._stdProposalDefault`).

```rust
pub const RAVEN_DEFAULT_PROPOSAL_FRACTION: f64 = 0.2;
```

## Module `transitional`

Transitional MCMC (TMCMC) and Transitional Ensemble MCMC (TEMCMC).

# The idea

Sampling a posterior directly is hard when the likelihood is sharp compared
with the prior: a chain started from the prior spends its life in the tails
and may never find the mass. Transitional samplers avoid that by walking
there in stages, through a sequence of intermediate distributions

```text
p_j(theta)  proportional to  p(theta) * L(theta)^beta_j,
0 = beta_0 < beta_1 < ... < beta_m = 1
```

The first stage is the prior, the last is the posterior, and each stage is
close enough to the next that importance-resampling plus a short MCMC run
carries the population across. The tempering exponents are **not** chosen by
the user: each `beta_{j+1}` is solved for so that the coefficient of
variation of the importance weights hits a target (1.0 by default), which is
what makes the method robust across problems of very different sharpness.

The by-product that makes this worth the machinery: the normalising constants
of the successive stages multiply to the **evidence** `p(D)`, so a single run
yields both the posterior sample and the marginal likelihood needed for model
comparison. A plain MCMC chain gives only the former.

# The two samplers

[`tmcmc`] and [`temcmc`] are the *same* algorithm with a different move
inside the stage:

| | stage move | needs tuning? |
|---|---|---|
| [`tmcmc`] | random-walk Metropolis–Hastings, proposal covariance scaled from the weighted sample covariance | the scale factor, and it matters |
| [`temcmc`] | affine-invariant ensemble ("stretch") move | no |

That is why they share one driver and one [`TransitionKernel`] enum rather
than being two functions with duplicated tempering logic: the tempering *is*
the algorithm, and the kernel is a choice within it.

# References

- J. Ching and Y.-C. Chen (2007). Transitional Markov Chain Monte Carlo
  method for Bayesian model updating, model class selection, and model
  averaging. *Journal of Engineering Mechanics, 133*(7), 816–832. doi:
  [10.1061/(ASCE)0733-9399(2007)133:7(816)](https://doi.org/10.1061/(ASCE)0733-9399(2007)133:7(816))
- A. Lye, A. Cicirello and E. Patelli (2022). An efficient and robust
  sampler for Bayesian inference: Transitional Ensemble Markov Chain Monte
  Carlo. *Mechanical Systems and Signal Processing, 167*, 108471. doi:
  [10.1016/j.ymssp.2021.108471](https://doi.org/10.1016/j.ymssp.2021.108471)
- A. Lye, A. Cicirello and E. Patelli (2021). Sampling methods for solving
  Bayesian model updating problems: A tutorial. *Mechanical Systems and
  Signal Processing, 159*, 107760. doi:
  [10.1016/j.ymssp.2021.107760](https://doi.org/10.1016/j.ymssp.2021.107760)

Independent implementations from those papers — see the provenance note in
[`super`].

```rust
pub mod transitional { /* ... */ }
```

### Types

#### Enum `TransitionKernel`

Which Markov-chain move a transitional sampler uses inside each stage.

Enum dispatch rather than a trait object, per the workspace design rules —
and it earns its keep here beyond that rule: the two variants carry
genuinely different tuning knobs, and a `match` at the one place the move is
made keeps them from leaking into the tempering code.

```rust
pub enum TransitionKernel {
    MetropolisHastings {
        scale: f64,
    },
    AffineInvariantEnsemble {
        a: f64,
    },
}
```

##### Variants

###### `MetropolisHastings`

Ching and Chen's original: random-walk Metropolis–Hastings whose
proposal covariance is `scale^2` times the weighted covariance of the
current stage's population.

`scale` is the paper's `beta` (renamed here because `beta` is already
the tempering exponent). The paper's value is 0.2, and the result is
sensitive to it: too small and the chains do not move, too large and
they are rejected.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `scale` | `f64` | Proposal scale factor, strictly positive. Ching and Chen use 0.2. |

###### `AffineInvariantEnsemble`

Lye, Cicirello and Patelli's variant: the affine-invariant ensemble
stretch move, which needs no proposal covariance at all.

`a` is the stretch parameter, conventionally 2.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Stretch parameter, strictly greater than 1. |

##### Implementations

###### Methods

- ```rust
  pub fn metropolis_hastings() -> Self { /* ... */ }
  ```
  Ching and Chen's Metropolis–Hastings kernel at the published scale 0.2.

- ```rust
  pub fn affine_invariant_ensemble() -> Self { /* ... */ }
  ```
  The affine-invariant ensemble kernel at the conventional `a = 2`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `TemperingCriterion`

The rule that decides how far each tempering stage may step.

# Why there is a choice here at all

The tempering schedule is the part of TMCMC that makes it robust, and the
rule that sets it is a modelling decision rather than a constant. Both
variants below answer the same question — "how far can `beta` move before
the importance weights degenerate?" — with different measures of
degeneration.

# The two are the same rule in different clothes

Worth knowing before choosing between them. For weights with mean `m` and
population variance `v`, the Kish effective sample size satisfies

```text
ESS = (sum w)^2 / sum w^2 = n / (1 + CoV^2)
```

exactly — so the two criteria are reparametrisations of each other under
`fraction = 1 / (1 + target^2)`, and **at their published defaults
(`target = 1`, `fraction = 0.5`) they are the identical rule**. The
comparison test in this module finds identical schedules for that reason,
and a separate test checks the identity numerically (agreement to 4e-15).

That does not make the alternative pointless: `fraction` is the more
interpretable dial, since "half my samples are still doing work" says
something a practitioner can act on and "the coefficient of variation of the
weights is 1" does not. It does mean that switching between them at the
published defaults will not change an answer, and anyone reporting a
difference between the two should look for it elsewhere.

One convention to note: [`weight_cov`] divides by `n`, not `n - 1`. A
definition using the unbiased denominator differs by `n / (n - 1)` inside
the square — negligible at usable population sizes, but stated rather than
assumed.

```rust
pub enum TemperingCriterion {
    WeightCoefficientOfVariation {
        target: f64,
    },
    EffectiveSampleSize {
        fraction: f64,
    },
}
```

##### Variants

###### `WeightCoefficientOfVariation`

Ching and Chen's original rule: step until the **coefficient of
variation** of the stage weights reaches `target`, conventionally 1.0.

Cheap and well-tested. Its weakness is that the coefficient of variation
is a moment of the weights, so a handful of enormous weights and a
broadly healthy population can produce the same value.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `target` | `f64` | Target coefficient of variation, strictly positive. Ching and Chen<br>use 1.0. Smaller means more, smaller stages: more robust and more<br>expensive. |

###### `EffectiveSampleSize`

The TMCMC-II rule of Lye and Marino (2023): step until the **effective
sample size** falls to `fraction` of the population, conventionally
one half.

The effective sample size `(sum w)^2 / sum w^2` measures directly how
many of the `N` samples are actually doing work, which is the quantity
a practitioner cares about, and it is bounded in `(0, N]` rather than
unbounded above. The published motivation for preferring it is that it
gives a more interpretable and better-behaved schedule than a moment
ratio.

Reference: A. Lye and L. Marino (2023). An investigation into an
alternative transition criterion of the Transitional Markov Chain Monte
Carlo method for Bayesian model updating. *Proceedings of the 33rd
European Safety and Reliability Conference*. doi:
[10.3850/978-981-18-8071-1_P331-cd](https://doi.org/10.3850/978-981-18-8071-1_P331-cd)

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `fraction` | `f64` | Fraction of the population the effective sample size may fall to,<br>in `(0, 1)`. Lye and Marino use 0.5. |

##### Implementations

###### Methods

- ```rust
  pub fn weight_cov() -> Self { /* ... */ }
  ```
  Ching and Chen's original criterion at the published target of 1.0.

- ```rust
  pub fn effective_sample_size() -> Self { /* ... */ }
  ```
  The TMCMC-II criterion at the published half-population target.

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Checks the criterion's own parameter.

- ```rust
  pub fn is_step_too_large(self: &Self, ln_l: &[f64], d_beta: f64) -> bool { /* ... */ }
  ```
  Whether a tempering step of `d_beta` would degenerate the weights past

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TransitionalConfig`

Configuration shared by both transitional samplers.

Defaults are the published ones where a paper gives a value, and are stated
in each field's documentation. [`TransitionalConfig::new`] is the only
constructor that validates, so a configuration that exists is a
configuration that can run.

```rust
pub struct TransitionalConfig {
    pub population: usize,
    pub criterion: TemperingCriterion,
    pub chain_length: usize,
    pub max_stages: usize,
    pub seed: i64,
    pub kernel: TransitionKernel,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `population` | `usize` | Number of samples carried through every stage, `N`.<br><br>This is the whole computational budget knob: the sampler evaluates the<br>likelihood `N` times per stage plus `N * chain_length` times for the<br>moves. For the ensemble kernel it is also the walker count, and must be<br>at least `2 * d + 2`. |
| `criterion` | `TemperingCriterion` | How far each tempering stage is allowed to step — the rule that decides<br>`beta_{j+1}`.<br><br>See [`TemperingCriterion`]; the two variants are the original TMCMC rule<br>and the TMCMC-II rule. |
| `chain_length` | `usize` | Number of MCMC moves applied per sample per stage.<br><br>1 is the classical TMCMC choice — resampling plus one move per sample,<br>with the decorrelation coming from the many stages. For the ensemble<br>kernel this counts *sweeps* over the whole population. |
| `max_stages` | `usize` | Hard cap on the number of stages, as a guard against a pathological<br>likelihood that makes the tempering crawl.<br><br>Reaching it is an error, not a truncated answer: a run that stopped at<br>`beta < 1` has not sampled the posterior at all, and returning it as if<br>it had is exactly the kind of silent wrongness the workspace rules<br>forbid. |
| `seed` | `i64` | Master seed for the run. The same seed with the same configuration<br>reproduces the run exactly. |
| `kernel` | `TransitionKernel` | Which move to use inside each stage. |

##### Implementations

###### Methods

- ```rust
  pub fn new(population: usize, criterion: TemperingCriterion, chain_length: usize, max_stages: usize, seed: i64, kernel: TransitionKernel) -> Result<Self> { /* ... */ }
  ```
  Builds and validates a configuration.

- ```rust
  pub fn tmcmc_defaults(population: usize, seed: i64) -> Result<Self> { /* ... */ }
  ```
  Ching and Chen's published defaults with the Metropolis–Hastings

- ```rust
  pub fn temcmc_defaults(population: usize, seed: i64) -> Result<Self> { /* ... */ }
  ```
  The same tempering defaults with the affine-invariant ensemble kernel.

- ```rust
  pub fn tmcmc_ii_defaults(population: usize, seed: i64) -> Result<Self> { /* ... */ }
  ```
  TMCMC-II defaults: the effective-sample-size criterion at half the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `StageReport`

What one tempering stage did.

Reported per stage because the *shape* of a run is what tells a reader
whether to trust it: a healthy run has tens of stages with acceptance rates
in the tens of percent and effective sample sizes that stay a decent
fraction of the population. One stage that jumps `beta` from 0.01 to 1.0
with an effective sample size of 3 has technically finished and has
sampled nothing.

```rust
pub struct StageReport {
    pub beta: f64,
    pub ln_evidence_increment: f64,
    pub acceptance_rate: f64,
    pub effective_sample_size: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `beta` | `f64` | Tempering exponent reached at the end of this stage, in `(0, 1]`. |
| `ln_evidence_increment` | `f64` | Natural log of this stage's contribution to the evidence. |
| `acceptance_rate` | `f64` | Fraction of proposed moves accepted during this stage, in `[0, 1]`. |
| `effective_sample_size` | `f64` | Kish effective sample size of the importance weights, `1 / sum(w^2)`<br>for normalised weights, in `(0, population]`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TransitionalResult`

The result of a transitional sampling run.

```rust
pub struct TransitionalResult {
    pub samples: Vec<Vec<f64>>,
    pub ln_likelihoods: Vec<f64>,
    pub ln_evidence: f64,
    pub stages: Vec<StageReport>,
    pub diagnostics: super::SamplerDiagnostics,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `samples` | `Vec<Vec<f64>>` | Posterior samples, one row per sample, `population` rows of `d`<br>parameters each. |
| `ln_likelihoods` | `Vec<f64>` | Natural log-likelihood at each returned sample, aligned with<br>[`samples`](Self::samples). |
| `ln_evidence` | `f64` | Natural log of the estimated evidence `p(D)`.<br><br>The product of the per-stage normalising constants. This is the number<br>to use for model comparison; exponentiating it is usually a mistake,<br>since it underflows for any realistic data set. |
| `stages` | `Vec<StageReport>` | One entry per tempering stage, in order. |
| `diagnostics` | `super::SamplerDiagnostics` | Numerical trouble met and handled during the run. |

##### Implementations

###### Methods

- ```rust
  pub fn posterior_mean(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Sample mean of each parameter — the posterior mean estimate.

- ```rust
  pub fn posterior_std_dev(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Unbiased sample standard deviation of each parameter.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `tmcmc`

Runs Transitional MCMC (Ching and Chen, 2007).

Convenience wrapper over [`run_transitional`] with the
[`TransitionKernel::MetropolisHastings`] kernel forced, so a caller who
wants "the TMCMC from the paper" cannot accidentally get the ensemble
variant. Any other kernel in `config` is overridden.

# Verification

**Methodology.** The conjugate Normal–Normal problem, where all three
outputs have closed forms. Data `y_i` drawn from `N(mu_true, sigma^2)` with
`sigma` known; prior `mu ~ N(mu_0, tau^2)`. Then the posterior is
`N(mu_n, tau_n^2)` with

```text
tau_n^2 = 1 / (1/tau^2 + n/sigma^2)
mu_n    = tau_n^2 * (mu_0/tau^2 + n*ybar/sigma^2)
```

and the evidence `p(D)` is available in closed form too (the marginal of a
Gaussian mean under a Gaussian prior). The sampler is checked on the
posterior mean, the posterior standard deviation **and** the log-evidence —
the last being the one a posterior-moments-only test would miss.

**Results** are recorded in the test module of this file
(`tmcmc_matches_the_conjugate_normal_posterior`), which prints the measured
values on `cargo test -- --nocapture`.

```rust
pub fn tmcmc<L>(prior: &super::IndependentPrior, ln_likelihood: L, config: &TransitionalConfig) -> crate::Result<TransitionalResult>
where
    L: Fn(&[f64]) -> f64 { /* ... */ }
```

#### Function `temcmc`

Runs Transitional Ensemble MCMC (Lye, Cicirello and Patelli, 2022).

Convenience wrapper over [`run_transitional`] with the
[`TransitionKernel::AffineInvariantEnsemble`] kernel forced. Any other
kernel in `config` is overridden.

The practical difference from [`tmcmc`]: no proposal scale to choose, and
performance that does not degrade when parameters differ wildly in
magnitude — see the affine-invariance test in [`super::mcmc`].

# Verification

Same conjugate Normal–Normal methodology as [`tmcmc`]; measured values in
`temcmc_matches_the_conjugate_normal_posterior` in this file's test module.

```rust
pub fn temcmc<L>(prior: &super::IndependentPrior, ln_likelihood: L, config: &TransitionalConfig) -> crate::Result<TransitionalResult>
where
    L: Fn(&[f64]) -> f64 { /* ... */ }
```

#### Function `run_transitional`

The shared transitional driver: tempering schedule, resampling, evidence,
and whichever stage move [`TransitionalConfig::kernel`] names.

Prefer [`tmcmc`] or [`temcmc`] unless you are deliberately mixing.

# Errors

- [`RafflesError::InvalidParameter`] if the ensemble kernel is chosen with
  a population below `2 * d + 2` (it would silently confine the walkers to
  a subspace), or if every initial prior draw has zero likelihood — that
  means the prior and the data do not overlap at all, and no amount of
  tempering fixes it.
- [`RafflesError::InvalidParameter`] if the run hits
  [`TransitionalConfig::max_stages`] before `beta` reaches 1. The partial
  population is *not* returned: it is not a posterior sample.

```rust
pub fn run_transitional<L>(prior: &super::IndependentPrior, ln_likelihood: L, config: &TransitionalConfig) -> crate::Result<TransitionalResult>
where
    L: Fn(&[f64]) -> f64 { /* ... */ }
```

### Types

#### Struct `IndependentPrior`

A prior over `d` parameters whose marginals are mutually independent.

This is the prior shape that covers nearly every Bayesian model-updating
problem in the engineering literature: each parameter gets its own
distribution (a `Uniform` range from a physical bound, a `LogNormal` for a
positive stiffness, a `Gamma` for a noise precision) and no correlation is
asserted between them a priori. Correlation is what the *posterior* is
expected to discover.

A correlated prior is deliberately not offered yet: the samplers here only
ever need [`ln_pdf`](Self::ln_pdf) and [`sample`](Self::sample), so adding a
`Prior` enum with a multivariate-normal variant later is a pure extension
and needs no change at the call sites.

# Example

```
use raffles::bayesian::IndependentPrior;
use raffles::distributions::{Distribution, Normal, Uniform};

let prior = IndependentPrior::new(vec![
    Distribution::Uniform(Uniform::new(0.0, 10.0)?),
    Distribution::Normal(Normal::new(1.0, 0.5)?),
])?;

assert_eq!(prior.dimension(), 2);
// Inside the support: a finite log-density.
assert!(prior.ln_pdf(&[5.0, 1.0]).is_finite());
// Outside it: impossible, not an error.
assert_eq!(prior.ln_pdf(&[-1.0, 1.0]), f64::NEG_INFINITY);
# Ok::<(), raffles::RafflesError>(())
```

```rust
pub struct IndependentPrior {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(marginals: Vec<Distribution>) -> Result<Self> { /* ... */ }
  ```
  Builds a prior from one marginal distribution per parameter.

- ```rust
  pub fn dimension(self: &Self) -> usize { /* ... */ }
  ```
  Number of parameters `d`, i.e. the length of every parameter vector

- ```rust
  pub fn marginals(self: &Self) -> &[Distribution] { /* ... */ }
  ```
  The marginal distributions, in parameter order.

- ```rust
  pub fn ln_pdf(self: &Self, theta: &[f64]) -> f64 { /* ... */ }
  ```
  Natural log of the joint prior density at `theta`.

- ```rust
  pub fn sample(self: &Self, seed: &mut u64) -> Vec<f64> { /* ... */ }
  ```
  Draws one parameter vector from the prior, advancing `seed` in place.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SamplerDiagnostics`

Counters a sampler keeps about numerical trouble it met and handled.

These are **not** errors: every one of them describes something a sampler
can legitimately encounter and continue past. They are reported so that a
run which "worked" but leaned heavily on a fallback cannot look identical
to one that did not.

```rust
pub struct SamplerDiagnostics {
    pub non_finite_likelihoods: usize,
    pub covariance_regularisations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `non_finite_likelihoods` | `usize` | Number of likelihood evaluations that returned `NaN`.<br><br>Treated as [`f64::NEG_INFINITY`] (impossible) so the chain keeps<br>moving. A non-zero count means the caller's likelihood has a hole in<br>it — a `0/0`, a `ln` of a negative, an unguarded `sqrt` — and the<br>posterior should not be trusted until that is found. |
| `covariance_regularisations` | `usize` | Number of times a stage's proposal covariance was not positive definite<br>and had to be regularised by inflating its diagonal.<br><br>Happens when the resampled population has collapsed onto fewer distinct<br>points than there are parameters. A large count means the tempering is<br>advancing too fast for the population size. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `EnsembleMove`

```rust
pub use mcmc::EnsembleMove;
```

#### Re-export `MetropolisHastings`

```rust
pub use mcmc::MetropolisHastings;
```

#### Re-export `raven_mcmc`

```rust
pub use raven_mcmc::raven_mcmc;
```

#### Re-export `BoundHandling`

```rust
pub use raven_mcmc::BoundHandling;
```

#### Re-export `Draws`

```rust
pub use raven_mcmc::Draws;
```

#### Re-export `LikelihoodForm`

```rust
pub use raven_mcmc::LikelihoodForm;
```

#### Re-export `RavenMcmcConfig`

```rust
pub use raven_mcmc::RavenMcmcConfig;
```

#### Re-export `RavenMcmcKind`

```rust
pub use raven_mcmc::RavenMcmcKind;
```

#### Re-export `RavenMcmcResult`

```rust
pub use raven_mcmc::RavenMcmcResult;
```

#### Re-export `RavenStep`

```rust
pub use raven_mcmc::RavenStep;
```

#### Re-export `temcmc`

```rust
pub use transitional::temcmc;
```

#### Re-export `tmcmc`

```rust
pub use transitional::tmcmc;
```

#### Re-export `StageReport`

```rust
pub use transitional::StageReport;
```

#### Re-export `TemperingCriterion`

```rust
pub use transitional::TemperingCriterion;
```

#### Re-export `TransitionKernel`

```rust
pub use transitional::TransitionKernel;
```

#### Re-export `TransitionalConfig`

```rust
pub use transitional::TransitionalConfig;
```

#### Re-export `TransitionalResult`

```rust
pub use transitional::TransitionalResult;
```

## Module `distance`

Statistical distances between two *sample sets*.

# What these are for

Classical Bayesian model updating needs a likelihood: a closed-form
statement of how probable the observed data are under a parameter vector.
Plenty of engineering problems cannot supply one — the model is a black box,
the data are a handful of scattered measurements, or the quantity being
matched is itself a *distribution* rather than a point. What can always be
done is to run the model, get a sample of its output, and ask **how far that
sample is from the measured one**.

That is what this module provides, and it is the engine of
[Approximate Bayesian Computation](crate::abc) and of the distance-based
stochastic model updating frameworks: pick a distance, and a small distance
becomes a large likelihood.

# Choosing one

The choice is not cosmetic. Lye, Ferson and Xiao (2024) compare exactly
these functions on the same problems and find they disagree about which
parameters are identifiable — so the metric is a modelling decision that
belongs in the write-up, not a default to be taken silently.

| Metric | Sees | True metric? | Bounded? |
|---|---|---|---|
| [`DistanceMetric::Euclidean`] | first two moments only | only with `standardise: false` | no |
| [`DistanceMetric::Bhattacharyya`] | the whole (binned) shape | no (fails the triangle inequality) | no — infinite for disjoint samples |
| [`DistanceMetric::Hellinger`] | the whole (binned) shape | yes | yes, `[0, 1]` |
| [`DistanceMetric::JensenShannon`] | the whole (binned) shape | its square root is | yes, `[0, ln 2]` nats |
| [`DistanceMetric::BrayCurtis`] | the whole (binned) shape | no | yes, `[0, 1]` |
| [`DistanceMetric::Wasserstein1`] | the whole CDF, unbinned | yes | no |

Two practical consequences of that table:

- **[`Bhattacharyya`](DistanceMetric::Bhattacharyya) goes infinite** when
  the two binned samples share no occupied bin. In an ABC likelihood that
  is a hard zero, which is a legitimate answer but will stall a sampler
  whose entire initial population is disjoint from the data. The bounded
  metrics degrade gracefully instead.
- **[`Wasserstein1`](DistanceMetric::Wasserstein1) needs no binning**, so it
  has no bin-count parameter to get wrong, and it is the only one here that
  keeps working sensibly when the samples are tiny. It is the "area metric"
  of the probability-bounds literature.

# Binning

Four of the six compare histograms, which means a bin count and a shared
grid. Both are decided from the **pooled** sample so that neither set gets
a grid that flatters it, and the default count follows the square-root rule
(`ceil(sqrt(n_pooled))`, clamped to `[2, 128]`) unless the caller names one.

In more than one dimension the grid is the Cartesian product of the
per-dimension bins, so the bin count grows as `bins^d`. That is the
curse of dimensionality and it is not hidden: past three or four
dimensions nearly every bin is empty and the binned metrics stop
discriminating. Use [`Wasserstein1`](DistanceMetric::Wasserstein1) or
[`Euclidean`](DistanceMetric::Euclidean) there, or reduce to summary
statistics first. [`DistanceMetric::bin_budget`] reports the grid size a
configuration would build so a caller can check before paying for it.

# References

- A. Lye, S. Ferson and S. Xiao (2024). Comparison between distance
  functions for Approximate Bayesian Computation towards stochastic model
  updating and model validation under limited data. *ASCE-ASME Journal of
  Risk and Uncertainty in Engineering Systems Part A: Civil Engineering,
  10*, 03124001. doi:
  [10.1061/AJRUA6.RUENG-1223](https://doi.org/10.1061/AJRUA6.RUENG-1223)
- S. Bi, M. Broggi and M. Beer (2019). The role of the Bhattacharyya
  distance in stochastic model updating. *Mechanical Systems and Signal
  Processing, 117*, 437–452. doi:
  [10.1016/j.ymssp.2018.08.017](https://doi.org/10.1016/j.ymssp.2018.08.017)
- J. Lin (1991). Divergence measures based on the Shannon entropy. *IEEE
  Transactions on Information Theory, 37*(1), 145–151. doi:
  [10.1109/18.61115](https://doi.org/10.1109/18.61115)
- S. Ferson, W. L. Oberkampf and L. Ginzburg (2008). Model validation and
  predictive capability for the thermal challenge problem. *Computer Methods
  in Applied Mechanics and Engineering, 197*(29–32), 2408–2430. doi:
  [10.1016/j.cma.2007.07.030](https://doi.org/10.1016/j.cma.2007.07.030)
  — the area metric.

Independent implementations from the published definitions; see the
provenance note in [`crate::bayesian`] for why that is stated explicitly.

```rust
pub mod distance { /* ... */ }
```

### Types

#### Enum `DistanceMetric`

A statistical distance between two sample sets.

Every variant is evaluated by [`DistanceMetric::distance`] on two sample
sets given as rows of equal width: `a[i][j]` is the `j`-th coordinate of the
`i`-th sample. One-dimensional data are rows of length 1.

Enum dispatch rather than a trait object, per the workspace design rules.
Here it also buys exhaustiveness where it matters: adding a metric forces
every `match` — including the one that decides whether a metric is bounded —
to say what the new one does.

```rust
pub enum DistanceMetric {
    Euclidean {
        standardise: bool,
    },
    Bhattacharyya {
        bins: Option<usize>,
    },
    Hellinger {
        bins: Option<usize>,
    },
    JensenShannon {
        bins: Option<usize>,
    },
    BrayCurtis {
        bins: Option<usize>,
    },
    Wasserstein1,
}
```

##### Variants

###### `Euclidean`

Euclidean distance between the two samples' summary statistics: the
per-dimension means and standard deviations, stacked into one vector.

The cheapest and the blindest. Two samples with the same mean and
variance but opposite skew are at distance zero from each other, so this
metric cannot see a bimodality that the data do show. It is included
because it is the classical ABC choice and the baseline the others are
compared against in the literature.

# `standardise` is a real choice, not a convenience

With `standardise: true` each coordinate's contribution is divided by
the pooled standard deviation **of the pair being compared**, so a
parameter in metres does not drown out one in millimetres. That is what
you want for a multi-output ABC problem — and it costs the triangle
inequality, because a distance whose scaling depends on which two
samples are in front of it is not a metric. This is not a subtlety that
was reasoned about in advance: the property test in this module caught
it, measuring `d(a, c) = 3.000899` against
`d(a, b) + d(b, c) = 2.999664` on three normal samples at means 0, 1
and 3.

With `standardise: false` the raw summary vectors are compared, which
*is* a true metric — and is the right choice when the outputs already
share a unit, or when the metric property is being relied on.

[`DistanceMetric::is_true_metric`] reports which of the two you have.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `standardise` | `bool` | Divide each coordinate by the pooled standard deviation of the pair<br>(unit-robust, not a metric) or compare raw summaries (a metric). |

###### `Bhattacharyya`

Bhattacharyya distance `-ln(sum_i sqrt(p_i q_i))` over the binned
histograms.

The distance-based stochastic-model-updating workhorse (Bi, Broggi and
Beer, 2019). Sensitive to the whole shape, not just the moments.

**It is not a metric** — it violates the triangle inequality — and it is
**unbounded**: when the two binned samples share no occupied bin the
Bhattacharyya coefficient is 0 and the distance is `+inf`. Both facts
are properties of the definition, not of this implementation, and both
are tested.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `bins` | `Option<usize>` | Bins per dimension, or `None` for the square-root rule. |

###### `Hellinger`

Hellinger distance `sqrt(1 - sum_i sqrt(p_i q_i))` over the binned
histograms.

The bounded, true-metric relative of [`Bhattacharyya`](Self::Bhattacharyya):
same Bhattacharyya coefficient underneath, but mapped into `[0, 1]` and
satisfying the triangle inequality. 0 means the histograms coincide,
1 means they are disjoint. This is the distance function of the
Hellinger-distance stochastic model updating framework.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `bins` | `Option<usize>` | Bins per dimension, or `None` for the square-root rule. |

###### `JensenShannon`

Jensen–Shannon divergence in nats over the binned histograms:
`0.5 KL(P || M) + 0.5 KL(Q || M)` with `M = (P + Q) / 2`.

Bounded above by `ln 2` (0.693147 nats), attained exactly when the two
samples are disjoint — which is what makes it well-behaved as an ABC
distance where the Bhattacharyya distance blows up. Its **square root**
is a true metric (the Jensen–Shannon distance); the divergence itself is
not, and this variant returns the divergence, as the papers using it do.

This is the distance function of the entropy-based affine-invariant
stochastic model updating framework.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `bins` | `Option<usize>` | Bins per dimension, or `None` for the square-root rule. |

###### `BrayCurtis`

Bray–Curtis dissimilarity over the binned histograms:
`sum_i |p_i - q_i| / sum_i (p_i + q_i)`, which for normalised
histograms is `0.5 * sum_i |p_i - q_i|` — the total variation distance.

Bounded in `[0, 1]`, cheap, and interpretable as the fraction of
probability mass that would have to be moved. Not a metric in the strict
sense used here only because it is a dissimilarity on compositions; on
normalised histograms it coincides with total variation, which is.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `bins` | `Option<usize>` | Bins per dimension, or `None` for the square-root rule. |

###### `Wasserstein1`

1-Wasserstein distance, summed over dimensions — the **area metric**.

For each coordinate, the area between the two empirical CDFs, computed
exactly from the order statistics with no binning at all. That makes it
the right choice for very small samples, where any histogram is mostly
noise, and it is why the probability-bounds literature (Ferson,
Oberkampf and Ginzburg, 2008) uses it for model validation.

Multivariate data are handled by summing the per-coordinate distances,
i.e. it compares the **marginals** and is blind to the dependence
structure between them. That is the standard "area metric" convention
in this literature, and it is stated here because the alternative — a
genuine multivariate optimal-transport distance — is a different and far
more expensive object.

##### Implementations

###### Methods

- ```rust
  pub fn is_bounded(self: &Self) -> bool { /* ... */ }
  ```
  Whether the metric's value is bounded above.

- ```rust
  pub fn is_true_metric(self: &Self) -> bool { /* ... */ }
  ```
  Whether the metric satisfies the triangle inequality, and is therefore a

- ```rust
  pub fn bins_for(self: &Self, n_pooled: usize) -> Option<usize> { /* ... */ }
  ```
  Bins per dimension this metric would use for a pooled sample of size

- ```rust
  pub fn bin_budget(self: &Self, n_pooled: usize, dimension: usize) -> Option<usize> { /* ... */ }
  ```
  Total number of histogram cells this metric would build for `n_pooled`

- ```rust
  pub fn distance(self: &Self, a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<f64> { /* ... */ }
  ```
  Distance between two sample sets.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `distributions`

Continuous probability distributions — densities, CDFs, inverse CDFs and
analytic moments.

Eight continuous distributions are implemented: [`Uniform`], [`Normal`],
[`LogNormal`], [`Triangular`], [`Exponential`], [`Weibull`], [`Gamma`] and
[`Beta`]. Each is a small `Copy` struct built through a validating `new`,
and all eight are collected in the [`Distribution`] enum, which dispatches
by `match`. [`Truncated`] renormalises any of them onto a sub-interval of
its support.

## Units

Distributions here are over **plain `f64` in whatever unit the caller's
uncertain parameter carries**. A `Normal` over a temperature and a `Normal`
over a reactivity are the same mathematics, so `uom` is deliberately not
used: the unit belongs to the caller's parameter definition, not to the
distribution. Probabilities, quantile arguments and CDF values are
dimensionless and lie in `[0, 1]`; densities carry the reciprocal of the
variate's unit; means carry the variate's unit and variances its square.

## Randomness lives elsewhere — on purpose

[`ContinuousDistribution1D::sample`] takes a **uniform deviate `u` in
`[0, 1]`** and returns `ppf(u)`. It does not take an RNG, and this module
contains no randomness at all. That is a deliberate design choice, not an
omission:

- Seeding, stream splitting and reproducibility are the sampler's problem.
  [`crate::samplers`] owns them, so a Latin-hypercube or grid design can
  choose *where* in `[0, 1]` to evaluate and reuse every distribution here
  unchanged.
- Every function in this module is a deterministic function of its
  arguments, so every test is an exact numerical assertion rather than a
  statistical one.

## Errors, never panics

Constructors and [`ContinuousDistribution1D::ppf`] return
[`crate::Result`]. Invalid parameters (a non-positive scale, an apex outside
its bounds, a probability outside `[0, 1]`, a non-finite argument) come back
as [`crate::RafflesError::InvalidParameter`]. No public entry point in this
module panics on caller input.

`pdf` and `cdf` are total functions of `f64` and return `0.0` outside the
support, so they need no `Result`. Where a density is genuinely unbounded —
[`Gamma`] with shape `alpha < 1` at its lower endpoint, [`Beta`] with
`alpha < 1` at `low` or `beta < 1` at `high`, [`Weibull`] with `k < 1` at
`low` — `pdf` returns `f64::INFINITY`, which is the correct limit.

## Verification

Verified against closed-form mathematics, not against upstream gold files
(those are RNG-stream dependent — see `docs/raven-port-scoping.md` §7). The
test module at the bottom of this file records methodology *and* measured
results for: analytic moments recovered by quadrature of the density, the
`cdf(ppf(p)) == p` and `ppf(cdf(x)) == x` round trips, unit total mass,
published reference quantiles (standard normal, chi-square, incomplete
beta), inverse-transform sampling reproducing the CDF, distribution
identities (`Gamma(1, b) == Exponential(b)`, `Beta(1, 1) == Uniform(0, 1)`,
`Weibull(1, l) == Exponential(1/l)`), and closed-form truncated-normal
moments.

**This is AI-assisted draft material and has had no human V&V review.** Do
not describe it as validated.

## Provenance

Ported from RAVEN (Apache-2.0); see the attribution header at the top of
this file for the upstream files, commit and the full list of structural
changes.

```rust
pub mod distributions { /* ... */ }
```

### Modules

## Module `seeded`

Seed-driven generic samplers (uniform, Box-Muller normal, exponential) on
the workspace LCG — moved from `outram-mc-libs` 2026-10-02 (GitHub #500).
Not RAVEN-derived; see the module's own header.
**Seed-driven generic samplers** on the workspace LCG
([`petir::rng::lcg`]): a uniform on `[low, high)`, a standard normal by
Box-Muller, and an exponential by inverse CDF.

# How these relate to the rest of [`crate::distributions`]

The RAVEN-derived distributions sample by **inverse CDF from a supplied
uniform** (`Distribution::sample(u)`), so the caller owns the stream. These
take the LCG **seed** and advance it themselves, which is what a Monte
Carlo transport history does. They are not a second implementation of the
same thing: `sample_normal` is Box-Muller (two draws per deviate), not the
inverse normal CDF, and changing either into the other would change every
stream that consumes them. Both shapes now live in one module tree, and
there is one copy of each.

# Exactly the stream they always drew

Moved byte-for-byte from `outram_mc_libs::rng::distributions`, which
re-exports them, so `outram-mc-libs`, `boon-lay` and `nee_soon` call sites
see the same numbers. `ln` is always `petir::real::ln` (as it was through
outram-mc's `RealMath::r_ln`). `cos` is [`petir::real::cos`] in every build
(~~the platform `f64::cos` unless an off-by-default `deterministic-math`
feature was on~~ removed 2026-10-20, gh:#985: deterministic maths is not a
choice). Physics samplers (`maxwell`, `watt`,
`isotropic_direction`) stay in `outram-mc-libs`.

```rust
pub mod seeded { /* ... */ }
```

### Functions

#### Function `uniform`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

A uniform deviate on `[low, high)`, one LCG draw.

```rust
pub fn uniform(seed: &mut u64, low: f64, high: f64) -> f64 { /* ... */ }
```

#### Function `sample_normal`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

A standard normal deviate `N(0, 1)` by the Box-Muller transform: two draws
`u1`, `u2`, returning `sqrt(-2 ln u1) · cos(2π u2)`. For `N(μ, σ²)` use
`μ + σ · sample_normal(seed)`.

`u1` is clamped to the smallest positive `f64` so `ln(0)` cannot occur.

```rust
pub fn sample_normal(seed: &mut u64) -> f64 { /* ... */ }
```

#### Function `sample_normal_3d`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

A 3-vector of independent `N(0, σ²)` deviates (three [`sample_normal`]
calls, x then y then z).

```rust
pub fn sample_normal_3d(seed: &mut u64, sigma: f64) -> (f64, f64, f64) { /* ... */ }
```

#### Function `sample_exp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

An exponential deviate with rate `λ` (mean `1/λ`) by inverse CDF,
`-ln(u) / λ`, one draw.

```rust
pub fn sample_exp(seed: &mut u64, rate: f64) -> f64 { /* ... */ }
```

### Types

#### Struct `Uniform`

Uniform distribution on the closed interval `[lower, upper]`.

Constant density `1 / (upper - lower)` over the interval and zero outside.
The maximum-entropy choice when only a physical range is known — e.g. a
manufacturing tolerance quoted as a plus/minus band with no preferred value
inside it.

Upstream: `Distributions1D.BasicUniformDistribution`, which builds
`scipy.stats.uniform(lowerBound, upperBound - lowerBound)`.

```rust
pub struct Uniform {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(lower: f64, upper: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a uniform distribution on `[lower, upper]`.

- ```rust
  pub fn lower(self: &Self) -> f64 { /* ... */ }
  ```
  Lower bound of the interval, in the variate's unit.

- ```rust
  pub fn upper(self: &Self) -> f64 { /* ... */ }
  ```
  Upper bound of the interval, in the variate's unit.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Normal`

Normal (Gaussian) distribution with mean `mu` and standard deviation
`sigma`.

Support is the whole real line, so it is the wrong model for a quantity that
cannot go negative (a temperature difference, a flow rate, a burnup); use
[`LogNormal`], [`Gamma`] or a [`Truncated`] normal for those. The usual
choice for measurement error and for a manufacturing parameter quoted as
"nominal plus/minus one sigma".

Upstream: `Distributions1D.BasicNormalDistribution` over
`scipy.stats.norm(mean, sd)`.

```rust
pub struct Normal {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mu: f64, sigma: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a normal distribution.

- ```rust
  pub fn mu(self: &Self) -> f64 { /* ... */ }
  ```
  Mean of the distribution, in the variate's unit.

- ```rust
  pub fn sigma(self: &Self) -> f64 { /* ... */ }
  ```
  Standard deviation, in the variate's unit.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `LogNormal`

Log-normal distribution: `X = low + exp(Y)` with `Y ~ Normal(mu, sigma)`.

**`mu` and `sigma` describe the underlying normal `Y`, not `X`.** This is
RAVEN's parameterisation and the usual one, but it is the single easiest
thing to get wrong: `E[X] = low + exp(mu + sigma^2 / 2)`, which is not
`mu`. The support is `(low, +inf)`, so this is the natural model for a
strictly positive quantity known to within a multiplicative factor — a
thermal conductivity, a heat-transfer coefficient, a failure rate.

Upstream: `Distributions1D.LogNormal` plus its wrapper
`BasicLogNormalDistribution`, which are hand-implemented rather than
delegated to `scipy.stats` precisely because of this shift parameter.

```rust
pub struct LogNormal {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mu: f64, sigma: f64, low: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a log-normal distribution.

- ```rust
  pub fn mu(self: &Self) -> f64 { /* ... */ }
  ```
  Mean of the underlying normal `ln(X - low)`.

- ```rust
  pub fn sigma(self: &Self) -> f64 { /* ... */ }
  ```
  Standard deviation of the underlying normal `ln(X - low)`.

- ```rust
  pub fn low(self: &Self) -> f64 { /* ... */ }
  ```
  Location shift: the (excluded) infimum of the support, in the variate's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```
    Variance of the log-normal.

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Triangular`

Triangular distribution on `[lower, upper]` peaking at `apex`.

The standard "expert elicitation" distribution: the smallest credible value,
the largest, and the most likely one, with a linear density between them. It
is bounded on both sides, which is often the honest statement of what is
known about an engineering parameter.

Upstream: `Distributions1D.BasicTriangularDistribution`, which converts to
SciPy's `triang(c, loc, scale)` with `c = (apex - lower) / (upper - lower)`.

```rust
pub struct Triangular {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(lower: f64, apex: f64, upper: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a triangular distribution.

- ```rust
  pub fn lower(self: &Self) -> f64 { /* ... */ }
  ```
  Lower bound, in the variate's unit.

- ```rust
  pub fn apex(self: &Self) -> f64 { /* ... */ }
  ```
  Mode (most likely value), in the variate's unit.

- ```rust
  pub fn upper(self: &Self) -> f64 { /* ... */ }
  ```
  Upper bound, in the variate's unit.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Exponential`

Exponential distribution with **rate** `lambda`, shifted so its support is
`[low, +inf)`.

Density `lambda * exp(-lambda * (x - low))`. The memoryless waiting-time
distribution: time to the next event of a Poisson process, time to failure
of a component with a constant hazard rate.

**`lambda` is a rate, not a mean.** The mean is `low + 1 / lambda`. Upstream
makes the same choice and converts on the way into SciPy
(`scipy.stats.expon(loc, 1 / lmbda)` in
`Distributions1D.BasicExponentialDistribution`).

```rust
pub struct Exponential {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(lambda: f64, low: f64) -> Result<Self> { /* ... */ }
  ```
  Builds an exponential distribution.

- ```rust
  pub fn lambda(self: &Self) -> f64 { /* ... */ }
  ```
  Rate parameter, in reciprocal units of the variate.

- ```rust
  pub fn low(self: &Self) -> f64 { /* ... */ }
  ```
  Location shift: the lower bound of the support, in the variate's unit.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Weibull`

Weibull distribution with shape `k` and **scale** `lambda`, shifted so its
support is `[low, +inf)`.

Density `(k / lambda) * ((x - low) / lambda)^(k-1) *
exp(-((x - low) / lambda)^k)`. The standard reliability / time-to-failure
model, because the hazard rate `k/lambda * ((x-low)/lambda)^(k-1)` is
decreasing for `k < 1` (infant mortality), constant for `k = 1` (reduces
exactly to [`Exponential`] with rate `1 / lambda`) and increasing for
`k > 1` (wear-out). Also used for brittle-fracture strength distributions,
where `k` is the Weibull modulus.

Upstream: `Distributions1D.BasicWeibullDistribution` over
`scipy.stats.weibull_min(k, low, lmbda)`.

```rust
pub struct Weibull {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(k: f64, lambda: f64, low: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a Weibull distribution.

- ```rust
  pub fn k(self: &Self) -> f64 { /* ... */ }
  ```
  Shape parameter (Weibull modulus), dimensionless.

- ```rust
  pub fn lambda(self: &Self) -> f64 { /* ... */ }
  ```
  Scale parameter, in the variate's unit.

- ```rust
  pub fn low(self: &Self) -> f64 { /* ... */ }
  ```
  Location shift: the lower bound of the support, in the variate's unit.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Gamma`

Gamma distribution with shape `alpha` and **rate** `beta`, shifted so its
support is `[low, +inf)`.

Density `beta^alpha * (x-low)^(alpha-1) * exp(-beta (x-low)) / Gamma(alpha)`.
The waiting time until the `alpha`-th event of a Poisson process, and the
usual flexible model for a strictly positive quantity with a right-skewed
spread. Special cases: `alpha = 1` is exactly [`Exponential`] with the same
rate; `alpha = nu/2, beta = 1/2, low = 0` is chi-square with `nu` degrees of
freedom.

**`beta` is a RATE, and this is the parameterisation trap RAVEN inherits.**
The scale is `1 / beta`, and `E[X] = low + alpha / beta`. Upstream's
`Distributions.py` takes `alpha`/`beta` from the user and constructs
`BasicGammaDistribution(self.alpha, 1.0 / self.beta, self.low)` — i.e. it
converts the rate to a scale on the way in. RAFFLES keeps the rate in the
public API and does the conversion internally, so callers coming from RAVEN
input decks pass the same numbers.

```rust
pub struct Gamma {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(alpha: f64, beta: f64, low: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a gamma distribution.

- ```rust
  pub fn alpha(self: &Self) -> f64 { /* ... */ }
  ```
  Shape parameter, dimensionless.

- ```rust
  pub fn beta(self: &Self) -> f64 { /* ... */ }
  ```
  Rate parameter, in reciprocal units of the variate. The scale is its

- ```rust
  pub fn low(self: &Self) -> f64 { /* ... */ }
  ```
  Location shift: the lower bound of the support, in the variate's unit.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Beta`

Beta distribution with shapes `alpha` and `beta`, rescaled from the standard
`(0, 1)` interval onto `[low, high]`.

With `z = (x - low) / (high - low)`, the density is
`z^(alpha-1) * (1-z)^(beta-1) / (B(alpha, beta) * (high - low))`. The
flexible bounded distribution: `alpha = beta = 1` is exactly
[`Uniform`]`(low, high)`, `alpha = beta > 1` is a symmetric hump,
`alpha != beta` skews it, and `alpha, beta < 1` puts the mass at the two
ends. Standard for a bounded fraction — a void fraction, a burnup fraction,
an efficiency.

Upstream: `Distributions1D.BasicBetaDistribution` over
`scipy.stats.beta(alpha, beta, low, scale)`, with `Distributions.py` passing
`scale = high - low`. RAFFLES takes `low`/`high` directly, since that is what
a caller actually knows.

```rust
pub struct Beta {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(alpha: f64, beta: f64, low: f64, high: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a beta distribution on `[low, high]`.

- ```rust
  pub fn alpha(self: &Self) -> f64 { /* ... */ }
  ```
  First shape parameter, dimensionless.

- ```rust
  pub fn beta(self: &Self) -> f64 { /* ... */ }
  ```
  Second shape parameter, dimensionless.

- ```rust
  pub fn low(self: &Self) -> f64 { /* ... */ }
  ```
  Lower bound of the support, in the variate's unit.

- ```rust
  pub fn high(self: &Self) -> f64 { /* ... */ }
  ```
  Upper bound of the support, in the variate's unit.

- ```rust
  pub fn scale(self: &Self) -> f64 { /* ... */ }
  ```
  Width of the support, `high - low`, in the variate's unit. This is

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Distribution`

A univariate continuous probability distribution — the dispatch point for
every distribution RAFFLES knows.

This is an **enum, not a trait object**, per the workspace design rules.
RAVEN's `Distribution` class hierarchy and its XML-name-driven `Factory` are
replaced by a closed set of variants: a caller constructs the variant they
want in Rust, and adding a distribution later is a compile error at every
`match` that forgot it rather than a silent runtime fallthrough. There is no
heap allocation — the enum is the size of its largest variant and is `Copy`.

Every variant delegates to the concrete struct's
[`ContinuousDistribution1D`] implementation, so the semantics, units and
valid parameter ranges are exactly those documented on each struct.

```
use raffles::distributions::{ContinuousDistribution1D, Distribution, Normal};

let d = Distribution::Normal(Normal::new(650.0, 12.0)?);
// A quantile of a coolant temperature, in whatever unit the caller used.
let hot = d.ppf(0.95)?;
assert!(hot > d.mean());
# Ok::<(), raffles::RafflesError>(())
```

```rust
pub enum Distribution {
    Uniform(Uniform),
    Normal(Normal),
    LogNormal(LogNormal),
    Triangular(Triangular),
    Exponential(Exponential),
    Weibull(Weibull),
    Gamma(Gamma),
    Beta(Beta),
}
```

##### Variants

###### `Uniform`

Constant density on a closed interval — see [`Uniform`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Uniform` |  |

###### `Normal`

Gaussian on the whole real line — see [`Normal`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Normal` |  |

###### `LogNormal`

`low + exp(Normal)`, strictly positive above `low` — see [`LogNormal`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LogNormal` |  |

###### `Triangular`

Bounded, piecewise-linear, expert-elicitation shape — see
[`Triangular`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Triangular` |  |

###### `Exponential`

Memoryless waiting time with a constant rate — see [`Exponential`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Exponential` |  |

###### `Weibull`

Reliability / time-to-failure with a monotone hazard — see [`Weibull`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Weibull` |  |

###### `Gamma`

Right-skewed positive quantity, shape plus rate — see [`Gamma`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Gamma` |  |

###### `Beta`

Flexible bounded distribution on `[low, high]` — see [`Beta`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Beta` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Truncated`

Any [`Distribution`] restricted to `[lower, upper]` and renormalised so its
density still integrates to one.

The mass outside the window is not discarded but redistributed:

- `pdf_trunc(x) = pdf(x) / (F(upper) - F(lower))` for `x` inside the window,
  zero outside;
- `cdf_trunc(x) = (F(x) - F(lower)) / (F(upper) - F(lower))`;
- `ppf_trunc(p) = F^-1(F(lower) + p * (F(upper) - F(lower)))`.

This is upstream's renormalisation from `Distributions1D.ContinuousDistribution`,
lifted out of the base class into its own type so that an untruncated
distribution costs nothing and the [`Distribution`] enum stays
non-recursive (hence no `Box`).

**Moments are numerical, not closed form.** Unlike upstream — whose
`untrMean`/`untrStdDev` return the *untruncated* moments and are therefore
wrong for a truncated variable — [`mean`](ContinuousDistribution1D::mean)
and [`variance`](ContinuousDistribution1D::variance) here integrate
`ppf_trunc` over `(0, 1)` by graded composite Gauss-Legendre quadrature. See
the verification tests for the measured accuracy against the closed-form
truncated normal. They are exact to quadrature error only, and cost roughly
500 quantile evaluations per call, so cache the result rather than calling
them in a loop.

```
use raffles::distributions::{ContinuousDistribution1D, Distribution, Normal, Truncated};

// A normally distributed positive quantity, truncated at zero.
let base = Distribution::Normal(Normal::new(1.0, 2.0)?);
let t = Truncated::new(base, 0.0, f64::INFINITY)?;
assert_eq!(t.cdf(0.0), 0.0);
assert!(t.mean() > base.mean()); // clipping the left tail pulls the mean up
# Ok::<(), raffles::RafflesError>(())
```

```rust
pub struct Truncated {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(base: Distribution, lower: f64, upper: f64) -> Result<Self> { /* ... */ }
  ```
  Restricts `base` to `[lower, upper]`.

- ```rust
  pub fn base(self: &Self) -> Distribution { /* ... */ }
  ```
  The untruncated distribution this was built from.

- ```rust
  pub fn lower(self: &Self) -> f64 { /* ... */ }
  ```
  Lower truncation bound, in the variate's unit.

- ```rust
  pub fn upper(self: &Self) -> f64 { /* ... */ }
  ```
  Upper truncation bound, in the variate's unit.

- ```rust
  pub fn retained_mass(self: &Self) -> f64 { /* ... */ }
  ```
  Probability mass of the base distribution inside the truncation window,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContinuousDistribution1D**
  - ```rust
    fn pdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn cdf(self: &Self, x: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn ppf(self: &Self, p: f64) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn mean(self: &Self) -> f64 { /* ... */ }
    ```
    Mean of the truncated variable, by quadrature of `ppf_trunc` over

  - ```rust
    fn variance(self: &Self) -> f64 { /* ... */ }
    ```
    Variance of the truncated variable, by quadrature of

  - ```rust
    fn support(self: &Self) -> (f64, f64) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `ContinuousDistribution1D`

Compiler-enforced contract that every concrete continuous distribution in
this module satisfies.

This trait exists so the compiler checks that each distribution really does
provide a density, a CDF, a quantile function and its analytic moments. It
is **never** used for runtime dispatch — there is no `Box<dyn
ContinuousDistribution1D>` anywhere, and there must not be. Dispatch over a
heterogeneous set of distributions goes through the [`Distribution`] enum,
which implements this same trait by `match`.

All quantities are plain `f64` in the caller's own units: `x` and the return
values of [`ppf`](Self::ppf), [`sample`](Self::sample) and
[`mean`](Self::mean) carry the variate's unit, [`pdf`](Self::pdf) carries its
reciprocal, [`variance`](Self::variance) its square, and `p` and the return
of [`cdf`](Self::cdf) are dimensionless probabilities in `[0, 1]`.

```rust
pub trait ContinuousDistribution1D {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `pdf`: Probability density at `x`, in reciprocal units of the variate.
- `cdf`: Cumulative probability `P(X <= x)`, in `[0, 1]`.
- `ppf`: Inverse CDF (percent-point / quantile function).
- `mean`: Analytic mean `E[X]`, in the variate's unit.
- `variance`: Analytic variance `Var[X]`, in the variate's unit squared.
- `support`: Closed interval `(lower, upper)` over which the density can be non-zero.

##### Provided Methods

- ```rust
  fn std_dev(self: &Self) -> f64 { /* ... */ }
  ```
  Analytic standard deviation `sqrt(Var[X])`, in the variate's unit.

- ```rust
  fn sample(self: &Self, u: f64) -> Result<f64> { /* ... */ }
  ```
  Draws a variate by the inverse-transform method from a caller-supplied

##### Implementations

This trait is implemented for the following types:

- `Uniform`
- `Normal`
- `LogNormal`
- `Triangular`
- `Exponential`
- `Weibull`
- `Gamma`
- `Beta`
- `Distribution`
- `Truncated`

## Module `estimators`

**Basic estimators** — the generic statistics a Monte Carlo code needs:
sample mean and standard error, seed pooling, Shannon entropy over counts,
running-sum (batch) uncertainty and batch prediction, and first-order error
propagation.

# Why these live in RAFFLES

The rule from GitHub issue #500: **the maths on numbers lives here; deciding
what is counted stays with the physics code.** `outram-mc-libs` still
chooses the active generations, bins the fission bank on its mesh, decides
which tally a trigger watches and which tallies are combined — and calls
these functions for the arithmetic. Before the move the sample mean/stderr
existed in four identical private copies in outram-mc's drivers; it now
exists once.

# Bit-identity is a requirement, not a nicety

Every function here reproduces its outram-mc original **bit for bit**: the
same summation order, the same `(n - 1)` placement, the same `sqrt` of a
quotient rather than a quotient of `sqrt`s. Two functions that compute "the
same" statistic are therefore kept apart where their rounding differs:
[`mean_and_stderr`] returns `sqrt(var / n)` and [`pooled`] returns
`sqrt(var) / sqrt(n)`, which are not bit-identical. Merging them would move
recorded results. `outram-mc-libs/tests/stats_move_fingerprints.rs` pins
k-eff runs through all of them.

# Status

No human V&V, like the rest of this crate. The formulas are the textbook
ones and each carries a unit test against a hand-computed value.

```rust
pub mod estimators { /* ... */ }
```

### Modules

## Module `seed_consistency`

Seed-ensemble consistency: `χ²/dof` of seed values against their internal
`σ`, a fixed 95 % band, leave-one-out outlier flags (GitHub #494).
**Seed-ensemble consistency** — is the scatter of `N` independent Monte
Carlo runs what each run's own internal `σ` says it should be?

# What this answers

A Monte Carlo code reports, for one run, a value `x_i` and an internal
standard error `σ_i`. Run the same case with `N` independent seeds and you
can check that `σ_i` is honest: if it is, the values scatter about their
common mean with exactly that spread, and

```text
χ² = Σ (x_i − x̄_w)² / σ_i²        x̄_w = Σ(x_i/σ_i²) / Σ(1/σ_i²)
```

is distributed as chi-square with `N − 1` degrees of freedom. So
`χ²/dof ≈ 1` is consistency; `χ²/dof` above its upper 97.5 % point means
the runs scatter **more** than their internal `σ` admits (the classic cause
in power iteration: cycle-to-cycle correlation makes the per-run `σ` too
small — GitHub #495); below the lower 2.5 % point means they scatter
**less**, which points at seeds that are not independent (the `op-rbo`
stream-correlation defect is the precedent).

# The band is fixed in advance

Two-sided 95 %: `[χ²_{0.025}(dof), χ²_{0.975}(dof)] / dof`, from the exact
chi-square quantiles (the regularised incomplete gamma in
[`crate::distributions`]), not from a normal approximation. Nothing here
takes a band from the caller, so the criterion cannot be moved after a
result is seen.

# Outlier seeds

Each seed is compared with the inverse-variance mean of the **others**
(leave-one-out, so a wild seed cannot pull the reference towards itself):

```text
z_i = (x_i − x̄_{−i}) / sqrt(σ_i² + 1/Σ_{j≠i} σ_j⁻²)
```

and flagged when `|z_i|` exceeds the two-sided Bonferroni threshold
`Φ⁻¹(1 − 0.025/N)` — a 5 % family-wise false-alarm rate over all `N` seeds,
again fixed in advance. A flag is a prompt to look at that seed, **never a
licence to drop it**: dropping a seed after seeing it is choosing the data.

# References

- Particle Data Group, *Review of Particle Physics*, "Statistics" chapter
  (weighted averages and the `χ²/dof` scale-factor test).
- C. E. Bonferroni (1936); any text on multiple comparisons.

```rust
pub mod seed_consistency { /* ... */ }
```

### Types

#### Enum `ConsistencyVerdict`

Direction of a seed-ensemble inconsistency, judged against the fixed
two-sided 95 % `χ²/dof` band.

```rust
pub enum ConsistencyVerdict {
    Consistent,
    OverDispersed,
    UnderDispersed,
}
```

##### Variants

###### `Consistent`

`χ²/dof` inside its 95 % band: the seed-to-seed scatter agrees with the
runs' own internal `σ`.

###### `OverDispersed`

Above the band: the runs scatter **more** than their internal `σ`
says. The internal `σ` under-states the uncertainty (in power
iteration, usually inter-cycle correlation — GitHub #495).

###### `UnderDispersed`

Below the band: the runs scatter **less** than their internal `σ`
says — seeds that are not independent, or a `σ` that over-states.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SeedOutlier`

One seed flagged by the leave-one-out outlier test.

```rust
pub struct SeedOutlier {
    pub index: usize,
    pub value: f64,
    pub z: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Position of the seed in the input slice. |
| `value` | `f64` | The seed's value, in the caller's unit. |
| `z` | `f64` | Its leave-one-out z-score (dimensionless). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SeedConsistency`

Everything [`seed_consistency`] measures about an ensemble.

Values carry the caller's unit (pcm, `k`, a tally unit); the ratios and
`χ²` are dimensionless.

```rust
pub struct SeedConsistency {
    pub n: usize,
    pub mean: f64,
    pub sd: f64,
    pub sem: f64,
    pub weighted_mean: f64,
    pub weighted_sem: f64,
    pub rms_internal_sigma: f64,
    pub spread_ratio: f64,
    pub chi2: f64,
    pub dof: usize,
    pub chi2_per_dof: f64,
    pub band_95: (f64, f64),
    pub p_value_upper: f64,
    pub verdict: ConsistencyVerdict,
    pub outlier_threshold: f64,
    pub outliers: Vec<SeedOutlier>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `usize` | Number of seeds. |
| `mean` | `f64` | Unweighted pooled mean, sample sd and standard error, exactly<br>[`super::pooled`] — the numbers this workspace's ensembles have always<br>quoted. |
| `sd` | `f64` | Seed-to-seed sample standard deviation (what ONE run scatters by). |
| `sem` | `f64` | `sd / √n`, the uncertainty on [`Self::mean`]. |
| `weighted_mean` | `f64` | Inverse-variance weighted mean `x̄_w`, the reference for `χ²`. |
| `weighted_sem` | `f64` | `1/sqrt(Σ σ_i⁻²)`: the uncertainty on `x̄_w` *if* the internal `σ`<br>are honest. Compare with [`Self::sem`]. |
| `rms_internal_sigma` | `f64` | Root-mean-square internal `σ`, `sqrt(mean σ_i²)`. |
| `spread_ratio` | `f64` | `sd / rms_internal_sigma` — `≈ 1` when the internal `σ` is honest. |
| `chi2` | `f64` | `Σ (x_i − x̄_w)² / σ_i²`. |
| `dof` | `usize` | `n − 1`. |
| `chi2_per_dof` | `f64` | `chi2 / dof`. |
| `band_95` | `(f64, f64)` | The fixed two-sided 95 % band on `χ²/dof` for this `dof`. |
| `p_value_upper` | `f64` | Upper-tail probability `P(χ²_dof ≥ chi2)`. |
| `verdict` | `ConsistencyVerdict` | Where `chi2_per_dof` falls relative to `band_95`. |
| `outlier_threshold` | `f64` | The two-sided Bonferroni `|z|` threshold used for outliers. |
| `outliers` | `Vec<SeedOutlier>` | Seeds whose leave-one-out `|z|` exceeds `outlier_threshold`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GroupMeanScatter`

Scatter of **disjoint group means** against the prediction `sd/√m`.

The `1/√N` law is not something a single ensemble can show by quoting
`sd/√N` — that is the formula, not a measurement. What *can* be measured
is whether means of `m` seeds really scatter by `sd/√m`: split the seeds
(in order) into `n_groups = ⌊N/m⌋` disjoint groups of `m`, take each
group's mean, and compare the sample sd of those means with `sd/√m`. If
the seeds are independent the ratio is `1` within
`ratio_sigma ≈ 1/sqrt(2(n_groups − 1))` (the large-sample standard error of
a sample sd, relative). Seeds that share structure push it away from 1.

```rust
pub struct GroupMeanScatter {
    pub group_size: usize,
    pub n_groups: usize,
    pub observed: f64,
    pub predicted: f64,
    pub ratio: f64,
    pub ratio_sigma: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `group_size` | `usize` | Seeds per group, `m`. |
| `n_groups` | `usize` | Number of disjoint groups used (leftover seeds at the end are unused). |
| `observed` | `f64` | Sample sd of the group means. |
| `predicted` | `f64` | `sd / √m` with `sd` the seed-to-seed sd over the seeds used. |
| `ratio` | `f64` | `observed / predicted`. |
| `ratio_sigma` | `f64` | Approximate 1σ on `ratio`, `1/sqrt(2(n_groups − 1))`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `seed_consistency`

Check an ensemble of independent runs against their own internal `σ`.

`values[i]` and `sigmas[i]` are seed `i`'s estimate and its internal 1σ
standard error, in one unit. See the module docs for the statistic, the
fixed 95 % band and the outlier rule.

# Errors

- [`RafflesError::DimensionMismatch`] if the slices differ in length.
- [`RafflesError::InvalidParameter`] for fewer than two seeds, a
  non-finite value, or a `σ` that is not strictly positive and finite (a
  zero `σ` would give that seed infinite weight and an infinite `χ²`).

```
use raffles::estimators::seed_consistency::{seed_consistency, ConsistencyVerdict};
let v = [0.0, 1.0, -1.0, 0.5, -0.5];
let s = [1.0; 5];
let c = seed_consistency(&v, &s).unwrap();
assert_eq!(c.dof, 4);
assert!((c.chi2 - 2.5).abs() < 1e-12);
assert_eq!(c.verdict, ConsistencyVerdict::Consistent);
```

```rust
pub fn seed_consistency(values: &[f64], sigmas: &[f64]) -> crate::Result<SeedConsistency> { /* ... */ }
```

#### Function `chi2_per_dof_band`

Two-sided band on `χ²/dof` at confidence `level` (e.g. `0.95`), from the
exact chi-square quantiles: `[χ²_{(1−level)/2}, χ²_{(1+level)/2}] / dof`.

`dof = 0` returns `(NaN, NaN)`; there is no distribution.

```rust
pub fn chi2_per_dof_band(dof: usize, level: f64) -> (f64, f64) { /* ... */ }
```

#### Function `group_mean_scatter`

See [`GroupMeanScatter`]. Returns `None` unless `group_size ≥ 1` and at
least three groups fit (two groups give a ratio with no usable error).

```rust
pub fn group_mean_scatter(values: &[f64], group_size: usize) -> Option<GroupMeanScatter> { /* ... */ }
```

## Module `autocorrelation`

Autocorrelation-corrected standard errors for a correlated sequence: batch
means, integrated autocorrelation time, effective sample size (GitHub #495).
**Standard errors for correlated sequences** — batch means, the integrated
autocorrelation time and the effective sample size.

# The problem

The textbook standard error `s/√n` assumes the `n` values are independent.
A Markov chain's successive states are not, and neither are successive
generations of a Monte Carlo power iteration: generation `g + 1`'s fission
source *is* generation `g`'s fission sites. Positive correlation makes the
true uncertainty of the mean larger than `s/√n` by `sqrt(τ_int)`, where

```text
τ_int = 1 + 2 Σ_{k≥1} ρ_k          (ρ_k: lag-k autocorrelation)
```

In a loosely coupled core (a pebble bed, a large lattice) `τ_int` can be
several, and the naive `σ` under-states the truth by that square root.

# Two estimators, reported side by side with the naive one

- **Non-overlapping batch means.** Cut the sequence into `a` consecutive
  batches of `b`, take each batch's mean, and quote `sd(batch means)/√a`.
  If `b` is long compared with the correlation length the batch means are
  nearly independent, so this is honest without estimating any `ρ_k`. The
  default batch size is `b = ⌊√n⌋` (Flegal & Jones 2010, the standard
  consistent choice), **fixed in advance** — choosing `b` after seeing
  which one gives the flattering `σ` is exactly the tuning the workspace
  forbids. When `n` is not a multiple of `b`, the **leading** remainder is
  dropped: the earliest values are the ones most likely to still carry a
  start-up transient.
- **Integrated autocorrelation time** with Sokal's automatic window:
  `τ(M) = 1 + 2 Σ_{k=1..M} ρ_k`, taking the smallest `M ≥ c·τ(M)`, with
  `c = 5` (Sokal 1997), again fixed. Then `ESS = n/τ` and
  `σ_ESS = s/√ESS`.

**Neither replaces the naive `σ`.** [`CorrelatedMean`] carries all three,
so a caller cannot report the corrected one while the naive one silently
vanishes, or vice versa.

# Limits stated up front

Both corrected estimators are themselves noisy: a batch-means `σ` from `a`
batches has a relative uncertainty of about `1/sqrt(2(a − 1))` — `±21 %`
at the 12 batches of a 120-generation run. With too few values neither can
see a correlation longer than the run, so a short run can report "no
correlation" for a source that simply has not had time to show it.

# References

- J. M. Flegal and G. L. Jones (2010). Batch means and spectral variance
  estimators in Markov chain Monte Carlo. *Annals of Statistics, 38*(2),
  1034–1070. doi:10.1214/09-AOS735
- A. D. Sokal (1997). Monte Carlo methods in statistical mechanics:
  foundations and new algorithms. In *Functional Integration*, Springer,
  131–192. doi:10.1007/978-1-4899-0319-8_6
- T. Ueki, F. B. Brown, D. K. Parsons and J. S. Warsa (2004).
  Autocorrelation and dominance ratio in Monte Carlo criticality
  calculations. *Nuclear Science and Engineering, 145*, 279–290 — the
  power-iteration motivation.

```rust
pub mod autocorrelation { /* ... */ }
```

### Types

#### Struct `AutocorrelationTime`

Integrated autocorrelation time by Sokal's automatic window.

```rust
pub struct AutocorrelationTime {
    pub tau_int: f64,
    pub window: usize,
    pub window_converged: bool,
    pub effective_sample_size: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tau_int` | `f64` | `τ_int = 1 + 2 Σ_{k=1..window} ρ_k`. |
| `window` | `usize` | The window `M` the sum was truncated at. |
| `window_converged` | `bool` | `false` when no `M < n/2` satisfied `M ≥ c·τ(M)`: the correlation<br>length is comparable to the run, and `tau_int` is a **lower bound**. |
| `effective_sample_size` | `f64` | `n / τ_int`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BatchMeans`

Non-overlapping batch means.

```rust
pub struct BatchMeans {
    pub batch_size: usize,
    pub n_batches: usize,
    pub dropped_leading: usize,
    pub mean: f64,
    pub sem: f64,
    pub sem_relative_uncertainty: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `batch_size` | `usize` | Values per batch, `b`. |
| `n_batches` | `usize` | Number of batches, `a`. |
| `dropped_leading` | `usize` | Leading values dropped so that `a·b` values remain. |
| `mean` | `f64` | Mean of the values used (equal to the mean of the batch means). |
| `sem` | `f64` | `sd(batch means) / √a`. |
| `sem_relative_uncertainty` | `f64` | Approximate relative 1σ on `sem` itself, `1/sqrt(2(a − 1))`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CorrelatedMean`

The mean of a correlated sequence with its naive and corrected standard
errors, side by side.

`naive_sem` is [`super::mean_and_stderr`] exactly — the number every
existing report in this workspace quotes. The two corrected ones are
additions; nothing here replaces it.

```rust
pub struct CorrelatedMean {
    pub n: usize,
    pub mean: f64,
    pub naive_sem: f64,
    pub batch: Option<BatchMeans>,
    pub tau: Option<AutocorrelationTime>,
    pub ess_sem: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `usize` | Number of values. |
| `mean` | `f64` | Plain mean of all values. |
| `naive_sem` | `f64` | Textbook `s/√n` (assumes independence). |
| `batch` | `Option<BatchMeans>` | Batch-means estimate at the default batch size, if at least two<br>batches fit. |
| `tau` | `Option<AutocorrelationTime>` | Autocorrelation-time estimate, if computable. |
| `ess_sem` | `Option<f64>` | `s / sqrt(ESS)`, the `τ`-corrected standard error. |

##### Implementations

###### Methods

- ```rust
  pub fn estimate(x: &[f64]) -> Self { /* ... */ }
  ```
  Estimate everything from `x`, with the default batch size.

- ```rust
  pub fn estimate_with_batch_size(x: &[f64], batch_size: usize) -> Self { /* ... */ }
  ```
  As [`Self::estimate`] with an explicit batch size — for a study of the

- ```rust
  pub fn batch_inflation(self: &Self) -> Option<f64> { /* ... */ }
  ```
  `batch.sem / naive_sem` — how much the naive `σ` under-states, by the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `autocorrelation`

Lag-`k` sample autocorrelations `ρ_0 = 1, ρ_1, …, ρ_{max_lag}`.

The standard biased estimator (divide every lag's sum by `n`, not
`n − k`), which keeps the sequence positive semi-definite. Returns an
empty vector for fewer than two values or a constant sequence (zero
variance has no autocorrelation to speak of). `max_lag` is clamped to
`n − 1`.

```rust
pub fn autocorrelation(x: &[f64], max_lag: usize) -> Vec<f64> { /* ... */ }
```

#### Function `integrated_autocorrelation_time`

See [`AutocorrelationTime`]. `None` for fewer than four values or a
constant sequence.

`τ_int` below 1 (anti-correlation) is reported as measured, not clamped:
it means the naive `σ` *over*-states, and hiding that would be choosing
the instrument after the result.

```rust
pub fn integrated_autocorrelation_time(x: &[f64]) -> Option<AutocorrelationTime> { /* ... */ }
```

#### Function `batch_means`

See [`BatchMeans`]. `None` when `batch_size` is zero or fewer than two
batches fit.

```rust
pub fn batch_means(x: &[f64], batch_size: usize) -> Option<BatchMeans> { /* ... */ }
```

#### Function `default_batch_size`

The default batch size `⌊√n⌋` (Flegal & Jones 2010), fixed in advance.

```rust
pub fn default_batch_size(n: usize) -> usize { /* ... */ }
```

### Constants and Statics

#### Constant `SOKAL_WINDOW_C`

Sokal's window constant `c`: the window `M` is the smallest with
`M ≥ c·τ(M)`. Fixed in advance at Sokal's recommended `5`.

```rust
pub const SOKAL_WINDOW_C: f64 = 5.0;
```

## Module `normality`

Sample skewness, excess kurtosis and the D'Agostino–Pearson normality test
(GitHub #944).
**Sample skewness, excess kurtosis and a normality test** — is a window of a
trace (a `k_eff` or Shannon-entropy history, say) plausibly Gaussian?

Generic statistics on a slice of numbers. Deciding which trace to test, and
what to do with the verdict, stays with the caller (the Auto `k_eff`
watcher of #927 asks for normality *and* stationarity, the latter from
[`super::stationarity`]).

# What is computed

With central moments `m_j = Σ(x − x̄)^j / n`:

- **Moment skewness** `g1 = m3 / m2^{3/2}` and **moment excess kurtosis**
  `g2 = m4 / m2² − 3`. Both are biased in small samples (`E[g2] =
  −6/(n+1)` for a normal population: about −0.19 at `n = 30`).
- **Bias-adjusted** `G1 = g1 √(n(n−1)) / (n−2)` and
  `G2 = ((n+1) g2 + 6)(n−1) / ((n−2)(n−3))` (the SAS/SPSS/Excel and
  SciPy `bias=False` estimators; Joanes & Gill 1998).
- **Standard errors under normality** of `g1` and `g2`: the *exact*
  variances `Var(g1) = 6(n−2)/((n+1)(n+3))` and
  `Var(g2) = 24n(n−2)(n−3)/((n+1)²(n+3)(n+5))`, which are also the
  variances the normality test below standardises by. They are standard
  errors *of the statistic if the data were normal* — the yardstick for
  "is this skew large?", not a confidence interval for a non-normal
  population's skew.

# Which normality test, and why (windows of 30–50)

The **D'Agostino–Pearson omnibus test** `K² = Z(g1)² + Z(g2)²`, where
`Z(g1)` is D'Agostino's (1970) normalising transformation of the skewness
and `Z(g2)` the Anscombe–Glynn (1983) transformation of the kurtosis. Under
normality `K²` is approximately `χ²` with 2 degrees of freedom, whose upper
tail is exactly `p = exp(−K²/2)`.

Jarque–Bera (`n/6 · (g1² + g2²/4)`) was **not** chosen: its `χ²₂` reference
law is asymptotic and is poor at `n` of a few tens (the skewness and
kurtosis converge to normality slowly, and `g2` is itself strongly skewed
at small `n`), which is why Jarque–Bera is normally used with simulated
critical values at small `n`. The D'Agostino transforms exist precisely to
make `Z(g1)` and `Z(g2)` standard normal at modest `n`: the skewness
transform is specified for `n ≥ 8` and the kurtosis transform for `n ≥ 20`
(the `n ≥ 20` floor is enforced here). The size of the test at `n = 30`
and `n = 50` is measured in the tests below rather than assumed.

**Measured size (2026-10-17): mildly liberal.** At nominal `alpha = 0.05`
the rejection rate on N(0,1) windows is about **0.057** (SciPy 1.18.1,
200 000 windows: 0.0568 at n = 20, 0.0576 at n = 30, 0.0572 at n = 50,
se 0.0005; this crate's own generator, 20 000 windows: 0.0570 at n = 30,
0.0530 at n = 50, se 0.0015). The `chi^2_2` law is an approximation, so
`p_value` is slightly too small; treat `alpha = 0.05` as roughly 0.057.

# Limits

- `K²` is an omnibus **moment** test: it detects skew and tail weight, not,
  for example, bimodality that preserves the first four moments, and it
  has little power against mild non-normality at `n = 30`. "Passed" means
  "no evidence against", never "is Gaussian".
- It assumes **independent** values. A correlated trace (power-iteration
  `k_eff`) violates that; the test's size is then not the nominal one.
  Thin or batch the trace first ([`super::autocorrelation`]) or read the
  result accordingly.
- A constant window has zero variance and no defined skewness: an `Err`.

# References

**Not yet verified against the literature corpus** — none of these is in
`reactor-literature` as of 2026-10-17 (searched by author name; the
submodule is also empty in agent worktrees), and the bibliographic details
below are given as the implementation's source of the formulas, to be
checked against the papers by a human:

- R. B. D'Agostino (1970). Transformation to normality of the null
  distribution of `g1`. *Biometrika* 57(3), 679–681.
- F. J. Anscombe and W. J. Glynn (1983). Distribution of the kurtosis
  statistic `b2` for normal samples. *Biometrika* 70(1), 227–234.
- R. B. D'Agostino and E. S. Pearson (1973). Tests for departure from
  normality. Empirical results for the distributions of `b2` and `√b1`.
  *Biometrika* 60(3), 613–622 (the omnibus `K²`).
- D. N. Joanes and C. A. Gill (1998). Comparing measures of sample skewness
  and kurtosis. *The Statistician* 47(1), 183–189 (`G1`, `G2`).

Cross-checked numerically against SciPy 1.18.1 `skew`, `kurtosis`,
`skewtest`, `kurtosistest`, `normaltest`; see the test docs.

# Status

No human V&V, like the rest of this crate.

```rust
pub mod normality { /* ... */ }
```

### Types

#### Struct `SampleMoments`

Sample skewness and excess kurtosis with their null standard errors.

```rust
pub struct SampleMoments {
    pub n: usize,
    pub mean: f64,
    pub skewness: f64,
    pub excess_kurtosis: f64,
    pub skewness_adjusted: f64,
    pub excess_kurtosis_adjusted: f64,
    pub se_skewness: f64,
    pub se_excess_kurtosis: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `usize` | Sample size. |
| `mean` | `f64` | Sample mean. |
| `skewness` | `f64` | Moment skewness `g1 = m3 / m2^{3/2}`. |
| `excess_kurtosis` | `f64` | Moment excess kurtosis `g2 = m4 / m2² − 3`. |
| `skewness_adjusted` | `f64` | Bias-adjusted skewness `G1`. |
| `excess_kurtosis_adjusted` | `f64` | Bias-adjusted excess kurtosis `G2`. |
| `se_skewness` | `f64` | Standard error of `g1` if the data were normal. |
| `se_excess_kurtosis` | `f64` | Standard error of `g2` if the data were normal. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `NormalityTest`

Result of the D'Agostino–Pearson omnibus normality test.

```rust
pub struct NormalityTest {
    pub n: usize,
    pub z_skewness: f64,
    pub z_kurtosis: f64,
    pub k2: f64,
    pub p_value: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `usize` | Sample size. |
| `z_skewness` | `f64` | Normalised skewness statistic `Z(g1)`, ~`N(0,1)` under normality. |
| `z_kurtosis` | `f64` | Normalised kurtosis statistic `Z(g2)`, ~`N(0,1)` under normality. |
| `k2` | `f64` | Omnibus `K² = Z(g1)² + Z(g2)²`, ~`χ²₂` under normality. |
| `p_value` | `f64` | Upper-tail p-value `exp(−K²/2)`. |

##### Implementations

###### Methods

- ```rust
  pub fn rejects_at(self: &Self, alpha: f64) -> Result<bool> { /* ... */ }
  ```
  `true` when normality is rejected at significance level `alpha`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `moments`

Sample skewness and excess kurtosis of `x`, with null standard errors.

Needs `n ≥ 4` finite, non-constant values ([`MIN_N_MOMENTS`]). See the
module docs for the definitions.

```
let m = raffles::estimators::normality::moments(&[1.0, 2.0, 3.0, 4.0, 10.0]).unwrap();
assert!((m.skewness - 36.0 / 31.622776601683793).abs() < 1e-12);
assert!((m.excess_kurtosis - (-0.212)).abs() < 1e-12);
```

```rust
pub fn moments(x: &[f64]) -> crate::Result<SampleMoments> { /* ... */ }
```

#### Function `dagostino_pearson`

D'Agostino–Pearson omnibus normality test on `x`.

Needs `n ≥ 20` finite, non-constant values ([`MIN_N_NORMALITY_TEST`]).
`p_value = exp(−K²/2)`; reject normality when it falls below the chosen
level (see [`NormalityTest::rejects_at`]). See the module docs for the
choice of test, its references and its limits.

```
let x: Vec<f64> = (1..=30).map(|i| (i * i) as f64).collect(); // right-skewed
let t = raffles::estimators::normality::dagostino_pearson(&x).unwrap();
assert!((t.p_value - 0.145278851325707).abs() < 1e-9); // SciPy normaltest
```

```rust
pub fn dagostino_pearson(x: &[f64]) -> crate::Result<NormalityTest> { /* ... */ }
```

### Constants and Statics

#### Constant `MIN_N_NORMALITY_TEST`

Minimum sample size of [`dagostino_pearson`]: the Anscombe–Glynn kurtosis
transform is specified for `n ≥ 20`.

```rust
pub const MIN_N_NORMALITY_TEST: usize = 20;
```

#### Constant `MIN_N_MOMENTS`

Minimum sample size for the bias-adjusted `G2` and its variance (`n − 3`
appears in a denominator), and so for [`moments`].

```rust
pub const MIN_N_MOMENTS: usize = 4;
```

## Module `stationarity`

Stationarity diagnostics on a trace: MSER truncation, a Geweke two-window
test, a single change-point posterior (GitHub #496).
**Stationarity diagnostics for a trace** — has a sequence stopped
drifting, and from where on?

Generic output-analysis tools for the warm-up ("initial transient")
problem. A Monte Carlo power iteration is the motivating case — its
Shannon-entropy and `k` traces relax from the initial guess before they
fluctuate about a fixed mode — but nothing here knows that: it takes a
slice of numbers. Deciding *which* trace, and what to do with the answer,
stays with the caller.

# Three independent pieces of evidence

- **MSER-m truncation** (White 1997; White, Cobb & Spratt 2000). Batch the
  trace into means of `m` (default `m = 5`, the published MSER-5), and pick
  the truncation `d` minimising
  `MSER(d) = Σ_{j>d} (Z_j − Z̄_d)² / (k − d)²`. It balances the bias of
  keeping transient values against the variance of discarding good ones.
  **If the minimum lies in the second half of the trace** the rule has
  found no stationary tail long enough to trust, and the trace is flagged
  as not converged — the published rule's own failure signal.
- **Geweke's two-window test** (Geweke 1992). Compare the mean of the first
  10 % of a (truncated) trace with the last 50 %, as a z-score, with each
  window's standard error from **batch means** ([`super::autocorrelation`])
  rather than the naive `s/√n` — the windows are autocorrelated, and the
  naive error would turn ordinary correlated wander into "drift". Geweke's
  original uses a spectral density at zero; batch means estimates the same
  quantity. `|z| > 1.96` is drift at 95 %, fixed in advance.
- **A single change-point in the mean**, Bayesian. On the batch means
  (batching suppresses the autocorrelation the model ignores), with flat
  priors on the two segment means and a Jeffreys prior on a common
  variance, the posterior over the change location `τ` is exact:
  `p(τ | Z) ∝ (n₁ n₂)^{-1/2} · S_τ^{-(n−2)/2}`, `S_τ` the within-segment
  sum of squares. Whether there is a change at all is judged by BIC
  (`ln Z ≈ ln L̂ − (p/2) ln n`) between "one mean" and "two means plus a
  location", and reported on the Kass–Raftery scale through
  [`crate::model_selection`].

# What none of these can do

They cannot see a transient longer than the trace, and a slowly drifting
trace with a long correlation time can look stationary to every one of
them. Passing is evidence, not proof; the reference for an inactive-cycle
count is still a much longer run.

# References

- K. P. White Jr. (1997). An effective truncation heuristic for bias
  reduction in simulation output. *Simulation, 69*(6), 323–334.
- K. P. White Jr., M. J. Cobb and S. C. Spratt (2000). A comparison of five
  steady-state truncation heuristics for simulation. *Proc. 2000 Winter
  Simulation Conference*, 755–760. doi:10.1109/WSC.2000.899870
- J. Geweke (1992). Evaluating the accuracy of sampling-based approaches to
  the calculation of posterior moments. In *Bayesian Statistics 4*, Oxford
  University Press, 169–193.
- R. E. Kass and A. E. Raftery (1995). Bayes factors. *JASA, 90*(430),
  773–795.

```rust
pub mod stationarity { /* ... */ }
```

### Types

#### Struct `MserTruncation`

Result of the MSER-m truncation rule.

```rust
pub struct MserTruncation {
    pub batch_size: usize,
    pub truncation: usize,
    pub truncation_batches: usize,
    pub n_batches: usize,
    pub statistic: Vec<f64>,
    pub in_first_half: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `batch_size` | `usize` | Batch size `m` used. |
| `truncation` | `usize` | Optimal truncation in **original samples** (`d* · m`). |
| `truncation_batches` | `usize` | Optimal truncation in batches, `d*`. |
| `n_batches` | `usize` | Number of batches `k`. |
| `statistic` | `Vec<f64>` | `MSER(d)` for every `d` evaluated (`0..=k−2`). |
| `in_first_half` | `bool` | `true` when `d* ≤ k/2`: a stationary tail at least half the trace long<br>was found. `false` is the rule's own "not converged" signal. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GewekeTest`

Result of Geweke's two-window test.

```rust
pub struct GewekeTest {
    pub mean_first: f64,
    pub mean_last: f64,
    pub sem_first: f64,
    pub sem_last: f64,
    pub z: f64,
    pub batch_errors: bool,
    pub drift: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mean_first` | `f64` | Mean of the first window. |
| `mean_last` | `f64` | Mean of the last window. |
| `sem_first` | `f64` | Standard error of each window's mean (batch means where at least two<br>batches fit, otherwise naive — see `batch_errors`). |
| `sem_last` | `f64` | See `sem_first`. |
| `z` | `f64` | `(mean_first − mean_last) / sqrt(sem_first² + sem_last²)`. |
| `batch_errors` | `bool` | `true` when both errors came from batch means. `false` means at least<br>one window was too short for that and fell back to the naive `s/√n`,<br>which under-states the error of a correlated window — read `z` as an<br>upper bound on its true size then. |
| `drift` | `bool` | `|z| > GEWEKE_Z_CRIT`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ChangePoint`

Posterior over a single change-point in the mean, plus a BIC verdict on
whether there is one.

```rust
pub struct ChangePoint {
    pub batch_size: usize,
    pub posterior: Vec<f64>,
    pub first_tau: usize,
    pub map_tau: usize,
    pub map_sample: usize,
    pub segment_means: (f64, f64),
    pub ln_bayes_factor_change: f64,
    pub strength: crate::model_selection::EvidenceStrength,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `batch_size` | `usize` | Batch size the trace was reduced by before the analysis. |
| `posterior` | `Vec<f64>` | `posterior[i]` is `p(τ = first_tau + i | Z)`: the first `τ` batch<br>means form segment one. Sums to 1. |
| `first_tau` | `usize` | Smallest `τ` considered (each segment needs two batch means). |
| `map_tau` | `usize` | Posterior mode, in batch means. |
| `map_sample` | `usize` | `map_tau` in original samples. |
| `segment_means` | `(f64, f64)` | Mean of segment one and segment two at the mode. |
| `ln_bayes_factor_change` | `f64` | `ln B` of "change" against "no change" by BIC. Positive favours a change. |
| `strength` | `crate::model_selection::EvidenceStrength` | Kass–Raftery reading of `|ln B|`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `mser_truncation`

MSER-`batch_size` on `x`. Batches from the **start** (the trailing
remainder is dropped), so a truncation counts samples from the first
value. `None` with fewer than four batches.

```rust
pub fn mser_truncation(x: &[f64], batch_size: usize) -> Option<MserTruncation> { /* ... */ }
```

#### Function `geweke`

Geweke's test on `x` with the fixed [`GEWEKE_WINDOWS`]. `None` when either
window has fewer than two values.

```rust
pub fn geweke(x: &[f64]) -> Option<GewekeTest> { /* ... */ }
```

#### Function `change_point`

Single change-point analysis of `x`, reduced to batch means of
`batch_size` first. `None` with fewer than six batch means, or a trace
with no spread at all.

```rust
pub fn change_point(x: &[f64], batch_size: usize) -> Option<ChangePoint> { /* ... */ }
```

### Constants and Statics

#### Constant `MSER_BATCH`

MSER batch size, fixed at the published MSER-5.

```rust
pub const MSER_BATCH: usize = 5;
```

#### Constant `GEWEKE_WINDOWS`

Geweke window fractions (first, last), fixed at Geweke's 10 % / 50 %.

```rust
pub const GEWEKE_WINDOWS: (f64, f64) = _;
```

#### Constant `GEWEKE_Z_CRIT`

Two-sided 95 % critical `|z|` for the Geweke test.

```rust
pub const GEWEKE_Z_CRIT: f64 = 1.959_963_984_540_054;
```

### Types

#### Struct `BinStats`

Running sums for one quantity across realisations (batches).

**Moved from** `outram_mc_libs::tally::trigger::BinStats`, which re-exports
it.

```rust
pub struct BinStats {
    pub sum: f64,
    pub sum_sq: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sum` | `f64` | Sum of the per-realisation values. |
| `sum_sq` | `f64` | Sum of their squares. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `mean_and_stderr`

Sample mean and **standard error of the mean** (1σ) of `x`.

```text
mean = Σx / n
var  = Σ(x - mean)² / (n - 1)
sem  = sqrt(var / n)
```

Returns `(0, 0)` for an empty slice and `(mean, 0)` for one sample — a
single realisation has no measurable spread.

**Moved from** `outram-mc-libs`, where four identical private copies
(`physics/keff.rs`, `physics/transport_csg.rs`, `physics/physics_mg.rs`,
`pebble_beds/keff_delta.rs`) computed the eigenvalue estimate over the
active generations. The four were diffed before merging and were
character-identical.

```
let (m, s) = raffles::estimators::mean_and_stderr(&[1.0, 2.0, 3.0]);
assert_eq!(m, 2.0);
assert!((s - (1.0_f64 / 3.0).sqrt()).abs() < 1e-15);
```

```rust
pub fn mean_and_stderr(x: &[f64]) -> (f64, f64) { /* ... */ }
```

#### Function `pooled`

Pool independent samples — typically one result per RNG seed — into
`(mean, sd, sem)`.

- **`sd`** is the sample standard deviation (`n - 1`): the spread of one
  realisation, i.e. what a single reproduction will give.
- **`sem = sd / √n`** is the uncertainty on the pooled mean; quote it when
  comparing with a benchmark or another code.

Returns `(NaN, 0, 0)` for no samples and `(x, 0, 0)` for one.

**Moved from** `outram_mc_libs::vv::pooled`, which re-exports this. Note
`sem` is `sd / sqrt(n)`, not [`mean_and_stderr`]'s `sqrt(var / n)`: equal in
exact arithmetic, different in the last bit, and kept so.

```
let (mean, sd, sem) = raffles::estimators::pooled(&[100.0, 200.0, 300.0]);
assert!((mean - 200.0).abs() < 1e-9);
assert!((sd - 100.0).abs() < 1e-9);
assert!((sem - 100.0 / 3.0_f64.sqrt()).abs() < 1e-9);
```

```rust
pub fn pooled(x: &[f64]) -> (f64, f64, f64) { /* ... */ }
```

#### Function `shannon_entropy_bits`

Shannon entropy, **in bits**, of the distribution given by non-negative
`counts` (weights per bin): `H = -Σ p log₂ p` with `p = c / Σc`, empty bins
contributing nothing.

Ranges from 0 (everything in one bin) to `log₂(bins)` (uniform). Returns
`None` when the total is not positive — there is no distribution to take
an entropy of (OpenMC divides unconditionally and yields NaN there).

**Moved from** `outram_mc_libs::tally::mesh::RegularMesh::shannon_entropy`
(a port of OpenMC `src/mesh.cpp`'s entropy, base 2 as upstream's
`std::log2`), which still bins the fission bank and calls this.

```
let h = raffles::estimators::shannon_entropy_bits(&[1.0, 1.0, 1.0, 1.0]).unwrap();
assert_eq!(h, 2.0);
assert_eq!(raffles::estimators::shannon_entropy_bits(&[0.0, 0.0]), None);
```

```rust
pub fn shannon_entropy_bits(counts: &[f64]) -> Option<f64> { /* ... */ }
```

#### Function `bin_uncertainty`

Mean, standard deviation **of the mean**, and relative error over `n`
realisations, from running sums:

```text
mean    = sum / n
std_dev = sqrt( max(0, sum_sq / n - mean²) / (n - 1) )
rel_err = std_dev / |mean|
```

The `(n - 1)` is outside the parenthesis, so this is the uncertainty on the
mean, not the spread of the realisations. The variance is clamped at 0 so
a cancelled variance reads as zero rather than NaN.

Returns `None` for `n < 2` (no estimate) or a mean of exactly zero ("no
contributions", which OpenMC signals with a `(-1, -1)` sentinel and which
must not be confused with "converged to zero").

**Moved from** `outram_mc_libs::tally::trigger::bin_uncertainty` (port of
OpenMC `get_tally_uncertainty`, `src/tallies/trigger.cpp:30`), which
re-exports it; the trigger *policy* (metrics, thresholds, which bins) stays
in outram-mc.

```rust
pub fn bin_uncertainty(stats: BinStats, n: usize) -> Option<(f64, f64, f64)> { /* ... */ }
```

#### Function `predict_batches`

Predicted total batches for an uncertainty that is `ratio` times its
target, assuming variance falls as `1/N`:

```text
n_pred = floor(n_active * ratio²) + n_inactive + 1,  n_active = current - n_inactive
```

`None` for a non-finite ratio (no basis for an estimate).

**Moved from** `outram_mc_libs::tally::trigger::predict_batches` (OpenMC
`src/tallies/trigger.cpp:209-215`), which re-exports it.

```rust
pub fn predict_batches(current_batch: usize, n_inactive: usize, ratio: f64) -> Option<usize> { /* ... */ }
```

#### Function `sigma_sum`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

First-order, **uncorrelated** error propagation for `a ± b`:
`σ = sqrt(σa² + σb²)`.

**Moved from** `outram_mc_libs::tally::arithmetic` (`DerivedTally::add` /
`sub`, after `openmc/tally.py`). Two operands that are in fact correlated
(`a + a`) get the uncorrelated answer, by design.

```rust
pub fn sigma_sum(sa: f64, sb: f64) -> f64 { /* ... */ }
```

#### Function `product_with_sigma`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

`a · b` and its first-order uncorrelated σ, `|a·b| · sqrt((σa/a)² + (σb/b)²)`.

**Moved from** `outram_mc_libs::tally::arithmetic::DerivedTally::mul`.

```rust
pub fn product_with_sigma(a: f64, sa: f64, b: f64, sb: f64) -> (f64, f64) { /* ... */ }
```

#### Function `quotient_with_sigma`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

`a / b` and its first-order uncorrelated σ, `|a/b| · sqrt((σa/a)² + (σb/b)²)`.
A zero `b` gives a non-finite value; guarding against it is the caller's job.

**Moved from** `outram_mc_libs::tally::arithmetic::DerivedTally::div`.

```rust
pub fn quotient_with_sigma(a: f64, sa: f64, b: f64, sb: f64) -> (f64, f64) { /* ... */ }
```

#### Function `sum_with_sigma`

Sum of values and the quadrature sum of their σ, `(Σv, sqrt(Σσ²))`.

**Moved from** `outram_mc_libs::tally::arithmetic::DerivedTally::sum`
(`openmc.Tally.summation`).

```rust
pub fn sum_with_sigma(values: &[f64], sigmas: &[f64]) -> (f64, f64) { /* ... */ }
```

## Module `fidelity`

**One common interface from a high-fidelity model to a surrogate**
(gh:#1194, epic gh:#1177): traits for the caller's models, enums for the
routes RAFFLES provides, and `uom` quantities at the edge.

# The pieces

| Item | Job |
|---|---|
| [`Quantities`], [`quantities!`](crate::quantities), [`Channel`], [`SiQuantity`] | a record of named `uom` quantities (an input `x`, an output `y`, a parameter vector `θ`) mapped one to one onto SI base values |
| [`ParameterSpace<Q>`] | a bound or a prior per channel, as a RAFFLES [`Distribution`](crate::distributions::Distribution), stated in uom |
| [`HighFidelityModel`] | `y = f(x)`, implemented by the caller (CFD, Monte Carlo, a toy function) |
| [`ParametricModel`] | `y = f_LF(x, θ)`, the object of calibration (Kennedy–O'Hagan gh:#1180, DUD gh:#1192) |
| [`Dataset<I, O>`], [`MultiFidelityDataset`] | runs with provenance, a [`Fidelity`] tag and a [`Split`] fixed at construction |
| [`SurrogateSpec`], [`Surrogate<I, O>`] | what to fit, and the fitted model: `Polynomial` (one fit per output), `Neural` behind the `burn` feature, since gh:#1193 RAVEN's `PolynomialChaos` and `Kriging { route: RavenSklearn }` (with a σ), since gh:#1190 SMT's `Kriging { route: Smt }` and `CoKriging` (MFK, fitted by `Surrogate::fit_multi` on a `MultiFidelityDataset`) |
| [`Prediction<O>`] | `mean: O`, `std_dev: Option<O>` (the uncertainty keeps the output's units) |
| [`ValidationReport<O>`] | held-out RMSE and max error per output in output units, LOO RMSE on the training rows, 2σ coverage when a σ exists |
| [`CorrectedModel<L>`], [`ThetaEstimate`], [`discrepancy_dataset`] | `y_LF(x, θ) + δ(x)`; since gh:#1180 `ThetaEstimate::Posterior` |
| [`koh::KohCalibration`], [`koh::KohSpec`] | Kennedy–O'Hagan calibration of a [`ParametricModel`] against a [`Dataset`]'s training rows, with a GP discrepancy (SEPIA's formulation, gh:#1180) |
| [`build`], [`sample`], [`BuildPlan`], [`HeldOut`] | sample `hf` over a [`Sampler`](crate::samplers::Sampler) design, split, fit, validate |
| [`StateSpaceModel`], [`Assimilator`], [`Gaussian`], [`AssimilationStep`] | `x_{k+1} = f(x_k)`, `z_k = h(x_k)`, implemented by the caller; FilterPy's EKF / UKF / EnKF ([`crate::filtering`]) run on it, typed (gh:#1191) |

# Design rules it follows

- **Traits between crates, enums within.** [`HighFidelityModel`] and
  [`ParametricModel`] are implemented in the physics crate and consumed
  here generically (`M: HighFidelityModel`), never as `dyn` (root
  `AGENTS.md`, "Crate-level interfaces may follow SOLID principles"). The
  routes RAFFLES provides are the closed enum [`Surrogate`]. No `dyn`, no
  `Box`, no lifetimes; `PhantomData` carries `I` and `O`.
- **uom at the edge, SI `f64` inside.** Every public signature that carries
  a physical quantity is typed; the numerics (polynomial, network, the
  ports still to come) run on SI base values, which is what keeps the
  differential tests of the ports against their upstream unchanged. The
  round trip is exact (`quantities` module tests).
- **The held-out split is fixed before fitting**, in the [`Dataset`] or
  the [`BuildPlan`], and no fit reads a held-out row: the test
  `poisoned_held_out_outputs_do_not_change_the_fit` pins it.
- **Routes are added with their ports, not as placeholders.** ~~Kriging and
  co-kriging (SMT, gh:#1190) become [`SurrogateSpec`] / [`Surrogate`]
  variants when they land~~ (landed 2026-11-04: `KrigingRoute::Smt`,
  `SurrogateSpec::CoKriging`; ~~polynomial chaos (RAVEN, gh:#1193)~~ landed
  2026-11-04 with RAVEN's kriging route); ~~a sibling
  `StateSpaceModel` trait comes with the Kalman ports (gh:#1191)~~
  landed 2026-11-04 with them: [`StateSpaceModel`] and [`Assimilator`].

# Scope note

[`build`] runs the caller's model, which the crate's earlier scope ("the
caller runs their own model and hands RAFFLES arrays of numbers") did not
do. It does so only through the caller's own trait implementation, with
no physics here; the maintainer directed the interface (gh:#1194).

# V&V

Record: `crates/raffles/verification_and_validation/fidelity_interface_gh1194.md`
(predictions written before the runs). The first real implementer is the
digital twin engine's double-pipe pair
(`outram_park_digital_twin_engine::multifidelity::double_pipe::fidelity`).

```rust
pub mod fidelity { /* ... */ }
```

### Modules

## Module `koh`

**Kennedy–O'Hagan calibration of a [`ParametricModel`] against a
[`Dataset`]** (gh:#1180): `y_HF(x) = y_LF(x, θ) + δ(x) + ε` with a GP
emulator of `y_LF` and a GP prior on δ, through SEPIA's scalar model
([`crate::bayesian::koh`]).

[`KohCalibration::calibrate`]:

1. **Simulator design:** at each caller-given design x, `runs_per_input`
   points of a seeded Latin hypercube over the calibrated θ coordinates,
   each mapped to a θ record (uncalibrated channels at
   [`KohSpec::theta_fixed`]) and run through the low-fidelity model.
   Running the low-fidelity model at a held-out x reads no held-out
   output.
2. **Field data:** the dataset's **training rows only**.
3. **Emulator hyperparameters** by the highest-posterior particle of a
   TMCMC run on SEPIA's simulation-only posterior, then
4. **the KOH posterior** over (θ, β_V, λ_Vz, λ_Os) by TMCMC with the
   emulator held there (KOH 2001 §4.5's plug-in).

Each coordinate may be taken on a log scale ([`Scale`]); the output must
be a single-channel record. Predictions ([`KohCalibration::predict`])
are mixtures over posterior samples of SEPIA's conditional Gaussians, with
δ's part reported beside the total.

```rust
pub mod koh { /* ... */ }
```

### Types

#### Enum `Scale`

The coordinate a channel is calibrated, emulated or modelled on.

```rust
pub enum Scale {
    Linear,
    Log,
}
```

##### Variants

###### `Linear`

The SI value itself.

###### `Log`

The natural log of the SI value (which must be positive).

##### Implementations

###### Methods

- ```rust
  pub fn to_coord(self: Self, si: f64) -> f64 { /* ... */ }
  ```
  SI value to the coordinate.

- ```rust
  pub fn to_si(self: Self, c: f64) -> f64 { /* ... */ }
  ```
  The coordinate back to an SI value.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ThetaPrior`

A prior on a calibrated θ coordinate, truncated to the design range.

```rust
pub enum ThetaPrior {
    Uniform,
    Normal {
        mean: f64,
        sd: f64,
    },
}
```

##### Variants

###### `Uniform`

Flat on the range (SEPIA's default N(0.5, 10²) on the unit scale).

###### `Normal`

Normal on the coordinate (ln of the SI value for [`Scale::Log`], the
SI value for [`Scale::Linear`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mean` | `f64` | Mean, in coordinate units. |
| `sd` | `f64` | Standard deviation, in coordinate units. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CalibratedParameter`

One calibrated θ channel.

```rust
pub struct CalibratedParameter {
    pub name: String,
    pub scale: Scale,
    pub prior: ThetaPrior,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The θ channel's name. |
| `scale` | `Scale` | Its coordinate. |
| `prior` | `ThetaPrior` | Its prior on that coordinate. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ObservationError`

A Gamma prior on the observation precision, stated as the error it
implies: `λ_Os ~ Gamma(shape, mean 1/σ²)`, σ the observation error on
the output coordinate divided by SEPIA's output standard deviation.

```rust
pub struct ObservationError {
    pub sd: f64,
    pub shape: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sd` | `f64` | The error's standard deviation on the output coordinate. |
| `shape` | `f64` | The Gamma shape. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `KohSpec`

What to calibrate, and how.

```rust
pub struct KohSpec<T: Quantities> {
    pub calibrated: Vec<CalibratedParameter>,
    pub theta_lower: T,
    pub theta_upper: T,
    pub theta_fixed: T,
    pub input_scales: Vec<Scale>,
    pub output_scale: Scale,
    pub runs_per_input: usize,
    pub design_seed: i64,
    pub observation_error: Option<ObservationError>,
    pub form: crate::bayesian::koh::KohForm,
    pub emulator_sampler: crate::bayesian::koh::KohSampleConfig,
    pub calibration_sampler: crate::bayesian::koh::KohSampleConfig,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `calibrated` | `Vec<CalibratedParameter>` | The calibrated θ channels. |
| `theta_lower` | `T` | The design range's lower end (only calibrated channels are read). |
| `theta_upper` | `T` | The design range's upper end. |
| `theta_fixed` | `T` | Every channel's value when not calibrated, and the **derived value**<br>an ablation resets a calibrated channel to. |
| `input_scales` | `Vec<Scale>` | The coordinate of each input channel. |
| `output_scale` | `Scale` | The output coordinate. |
| `runs_per_input` | `usize` | Low-fidelity runs per design input. |
| `design_seed` | `i64` | The design's master seed (input `i` uses `seed + i`). |
| `observation_error` | `Option<ObservationError>` | The observation-precision prior; `None` keeps SEPIA's Gamma(1, 1e-3). |
| `form` | `crate::bayesian::koh::KohForm` | With or without δ. |
| `emulator_sampler` | `crate::bayesian::koh::KohSampleConfig` | TMCMC for the emulator's hyperparameters (all free, simulation only). |
| `calibration_sampler` | `crate::bayesian::koh::KohSampleConfig` | TMCMC for the calibration (`free` is overridden to `CalibrationOnly`). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `KohPrediction`

The predictive distribution at one x.

```rust
pub struct KohPrediction<O: Quantities> {
    pub median: O,
    pub lower95: O,
    pub upper95: O,
    pub mean: f64,
    pub sd: f64,
    pub delta_mean: f64,
    pub delta_sd: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `median` | `O` | The mixture median, in output units. |
| `lower95` | `O` | The 2.5 % quantile. |
| `upper95` | `O` | The 97.5 % quantile. |
| `mean` | `f64` | Mixture mean on the output coordinate. |
| `sd` | `f64` | Mixture standard deviation on the output coordinate. |
| `delta_mean` | `f64` | δ's posterior mean on the output coordinate (0 without δ). |
| `delta_sd` | `f64` | δ's posterior standard deviation on the output coordinate. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `KohCalibration`

A finished calibration.

```rust
pub struct KohCalibration<L: ParametricModel> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn calibrate(lf: L, data: &Dataset<<L as >::Input, <L as >::Output>, design: &[<L as >::Input], spec: KohSpec<<L as >::Theta>) -> Result<Self> { /* ... */ }
  ```
  Runs the calibration (module docs).

- ```rust
  pub fn recalibrate(self: &Self, form: KohForm) -> Result<Self>
where
    L: Clone { /* ... */ }
  ```
  The same data and emulator, calibrated again as `form` (e.g. without

- ```rust
  pub fn model(self: &Self) -> &KohModel { /* ... */ }
  ```
  The SEPIA model (data, priors).

- ```rust
  pub fn emulator(self: &Self) -> &KohParams { /* ... */ }
  ```
  The emulator's plug-in hyperparameters.

- ```rust
  pub fn posterior(self: &Self) -> &KohPosterior { /* ... */ }
  ```
  The raw posterior.

- ```rust
  pub fn lf(self: &Self) -> &L { /* ... */ }
  ```
  The low-fidelity model.

- ```rust
  pub fn theta_coordinates(self: &Self) -> Vec<Vec<f64>> { /* ... */ }
  ```
  Posterior samples of the calibrated θ coordinates, one row each.

- ```rust
  pub fn theta_estimate(self: &Self) -> Result<ThetaEstimate<<L as >::Theta>> { /* ... */ }
  ```
  The typed posterior of θ (uncalibrated channels at their fixed

- ```rust
  pub fn predict(self: &Self, xs: &[<L as >::Input], stride: usize, observation_error: bool, reset: Option<&str>) -> Result<Vec<KohPrediction<<L as >::Output>>> { /* ... */ }
  ```
  The predictive at each x, a mixture over every `stride`-th posterior

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `build`

```rust
pub use build::build;
```

#### Re-export `sample`

```rust
pub use build::sample;
```

#### Re-export `BuildPlan`

```rust
pub use build::BuildPlan;
```

#### Re-export `HeldOut`

```rust
pub use build::HeldOut;
```

#### Re-export `Dataset`

```rust
pub use dataset::Dataset;
```

#### Re-export `Fidelity`

```rust
pub use dataset::Fidelity;
```

#### Re-export `MultiFidelityDataset`

```rust
pub use dataset::MultiFidelityDataset;
```

#### Re-export `SiRows`

```rust
pub use dataset::SiRows;
```

#### Re-export `Split`

```rust
pub use dataset::Split;
```

#### Re-export `discrepancy_dataset`

```rust
pub use models::discrepancy_dataset;
```

#### Re-export `CorrectedModel`

```rust
pub use models::CorrectedModel;
```

#### Re-export `HighFidelityModel`

```rust
pub use models::HighFidelityModel;
```

#### Re-export `ParametricModel`

```rust
pub use models::ParametricModel;
```

#### Re-export `ThetaEstimate`

```rust
pub use models::ThetaEstimate;
```

#### Re-export `check_width`

```rust
pub use quantities::check_width;
```

#### Re-export `Channel`

```rust
pub use quantities::Channel;
```

#### Re-export `Quantities`

```rust
pub use quantities::Quantities;
```

#### Re-export `SiQuantity`

```rust
pub use quantities::SiQuantity;
```

#### Re-export `ParameterSpace`

```rust
pub use space::ParameterSpace;
```

#### Re-export `AssimilationStep`

```rust
pub use state_space::AssimilationStep;
```

#### Re-export `Assimilator`

```rust
pub use state_space::Assimilator;
```

#### Re-export `Gaussian`

```rust
pub use state_space::Gaussian;
```

#### Re-export `StateSpaceModel`

```rust
pub use state_space::StateSpaceModel;
```

#### Re-export `KrigingRoute`

```rust
pub use surrogate::KrigingRoute;
```

#### Re-export `Prediction`

```rust
pub use surrogate::Prediction;
```

#### Re-export `Surrogate`

```rust
pub use surrogate::Surrogate;
```

#### Re-export `SurrogateSpec`

```rust
pub use surrogate::SurrogateSpec;
```

#### Re-export `ValidationReport`

```rust
pub use surrogate::ValidationReport;
```

## Module `filtering`

**State estimation: FilterPy's Kalman filters** (gh:#1191).

Ported from FilterPy (R. Labbe, <https://github.com/rlabbe/filterpy>,
commit `5df4e095fac0`, 2.0.0, MIT; `LICENSE-MIT-FILTERPY`, crate `NOTICE`):

| Item | Upstream |
|---|---|
| [`KalmanFilter`], [`rts_smoother`] | `kalman/kalman_filter.py`: predict, Joseph-form update, batch filter, RTS |
| [`ExtendedKalmanFilter`] | `kalman/EKF.py` |
| [`MerweScaledSigmaPoints`] | `kalman/sigma_points.py` |
| [`UnscentedKalmanFilter`], [`unscented_transform`] | `kalman/UKF.py`, `kalman/unscented_transform.py` |
| [`EnsembleKalmanFilter`], [`outer_product_sum`] | `kalman/ensemble_kalman_filter.py`, `common/helpers.py` |
| [`q_discrete_white_noise`], [`q_continuous_white_noise`], [`order_by_derivative`] | `common/discretization.py` |

Written here, not ported: [`Matrix`] and the kernels of
[`numpy_ops`], which round as numpy/OpenBLAS do (measured, so the
filters are bit-identical with FilterPy inside the measured envelope);
[`gaussian_draws`] (the EnKF's own draws on the workspace LCG);
[`nees`] and [`nis`].

The typed route for a caller's model is
[`crate::fidelity::StateSpaceModel`] with [`crate::fidelity::Assimilator`].

**Synthetic data only** (`RESPONSIBLE_USE.md`): these filters are for
twin experiments, teaching and V&V, never for estimating the state of a
real plant.

V&V: `crates/raffles/verification_and_validation/filterpy_gh1191.md`,
tests `tests/filterpy_gh1191.rs`.

```rust
pub mod filtering { /* ... */ }
```

### Modules

## Module `numpy_ops`

**A small dense matrix and numpy's rounding order** (gh:#1191).

FilterPy is numpy: every `dot` is an OpenBLAS gemm, gemv or ddot, `inv`
is LAPACK `dgesv`, scipy's `cholesky` is `dpotrf`. For the filters to be
bit-identical with FilterPy, each operation here rounds exactly as that
kernel does. The orders were measured, not read from a library
(`verification_and_validation/filterpy_gh1191.md`, section 2.3):

| numpy call | here | order | measured envelope |
|---|---|---|---|
| `dot(A, B)`, both matrices | [`Matrix::dot`] | one FMA chain over k per element | any size tried (≤ 5×5, 3×7·7×3) |
| `dot(A, x)`, A with ≥ 2 rows | [`Matrix::dot_vec`] | 4-lane FMA blocks reduced `(l0+l2)+(l1+l3)`, then a k mod 4 tail | k ≤ 7, or rows a multiple of 4 |
| `dot(a, x)`, one row | [`ddot`] | one FMA chain | k ≤ 15 |
| `dot(w, S)`, 1-D w | [`vec_dot`] | pairs of FMAs in fours, then a chain | S with 2 or 3 columns |
| `linalg.inv` | [`Matrix::inv`] | left-looking LU, trsm with reciprocal diagonals in power-of-two blocks | n ≤ 5 |
| `scipy.linalg.cholesky` (upper) | [`Matrix::cholesky_upper`] | `dpotf2` with the gemv's −1 folded into x | n ≤ 4 |
| `mean(axis=0)`, `sum(axis=0)` | [`mean_rows`], [`sum_matrices`] | sequential over rows; numpy's **pairwise** sum when each row is a single element (the axis is then contiguous) | rows of 1, 2, 3 and stacks of 1×1, 2×1, 1×2, 3×3 (found when D7 first missed) |

Outside an envelope the kernels keep the same structure and are correct
linear algebra, but are **not claimed bit-identical** to numpy: the
OpenBLAS kernels there (gemv with 4x2 / 4x1 blocks at k ≥ 8, ddot at
k ≥ 16, gemv-N with ≥ 4 columns, `inv` beyond 5×5) were not measured
or did not match. The order is also **CPU-dispatch dependent**: the
references were made on SkylakeX.

`f64::mul_add` is a correctly rounded fused multiply-add on every target
(a hardware FMA, or libm's software `fma`), so the results do not depend
on the build's target features.

```rust
pub mod numpy_ops { /* ... */ }
```

### Types

#### Struct `Matrix`

A dense row-major matrix of `f64`.

```rust
pub struct Matrix {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(rows: usize, cols: usize) -> Self { /* ... */ }
  ```
  A `rows × cols` matrix of zeros.

- ```rust
  pub fn identity(n: usize) -> Self { /* ... */ }
  ```
  The `n × n` identity.

- ```rust
  pub fn from_row_major(rows: usize, cols: usize, data: Vec<f64>) -> Result<Self> { /* ... */ }
  ```
  From row-major data.

- ```rust
  pub fn from_rows(rows: &[Vec<f64>]) -> Result<Self> { /* ... */ }
  ```
  From rows, which must all have the same length (at least one row).

- ```rust
  pub fn column(v: &[f64]) -> Self { /* ... */ }
  ```
  A column vector (`n × 1`), FilterPy's 2-D `x`.

- ```rust
  pub fn diagonal(d: &[f64]) -> Self { /* ... */ }
  ```
  A diagonal matrix.

- ```rust
  pub fn rows(self: &Self) -> usize { /* ... */ }
  ```
  Number of rows.

- ```rust
  pub fn cols(self: &Self) -> usize { /* ... */ }
  ```
  Number of columns.

- ```rust
  pub fn get(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```
  Element `(i, j)`.

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Set element `(i, j)`.

- ```rust
  pub fn row(self: &Self, i: usize) -> &[f64] { /* ... */ }
  ```
  Row `i` as a slice.

- ```rust
  pub fn set_row(self: &mut Self, i: usize, v: &[f64]) -> Result<()> { /* ... */ }
  ```
  Replace row `i`.

- ```rust
  pub fn col(self: &Self, j: usize) -> Vec<f64> { /* ... */ }
  ```
  Column `j`, copied.

- ```rust
  pub fn as_slice(self: &Self) -> &[f64] { /* ... */ }
  ```
  The row-major data.

- ```rust
  pub fn to_rows(self: &Self) -> Vec<Vec<f64>> { /* ... */ }
  ```
  As nested rows.

- ```rust
  pub fn diag(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The main diagonal.

- ```rust
  pub fn t(self: &Self) -> Self { /* ... */ }
  ```
  The transpose.

- ```rust
  pub fn add(self: &Self, other: &Self) -> Result<Self> { /* ... */ }
  ```
  Elementwise `self + other`.

- ```rust
  pub fn sub(self: &Self, other: &Self) -> Result<Self> { /* ... */ }
  ```
  Elementwise `self − other`.

- ```rust
  pub fn scale(self: &Self, s: f64) -> Self { /* ... */ }
  ```
  `s * self` elementwise (numpy's scalar times array).

- ```rust
  pub fn div_scalar(self: &Self, s: f64) -> Self { /* ... */ }
  ```
  `self / s` elementwise.

- ```rust
  pub fn dot(self: &Self, other: &Self) -> Result<Self> { /* ... */ }
  ```
  numpy's `dot(self, other)` for two 2-D arrays, in numpy's dispatch

- ```rust
  pub fn dot_vec(self: &Self, x: &[f64]) -> Result<Vec<f64>> { /* ... */ }
  ```
  numpy's `dot(self, x)` with x 1-D or a column: ddot for one row,

- ```rust
  pub fn inv(self: &Self) -> Result<Self> { /* ... */ }
  ```
  numpy's `linalg.inv` (LAPACK `dgesv` on the identity, OpenBLAS's

- ```rust
  pub fn cholesky_upper(self: &Self) -> Result<Self> { /* ... */ }
  ```
  scipy's `linalg.cholesky(self)` (upper, `U` with `Uᵀ U = self`),

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `ddot`

**Attributes:**

- `MustUse { reason: None }`

numpy's `dot` of two 1-D arrays (OpenBLAS ddot): one FMA chain from 0.
Measured bit-identical for k ≤ 15 (the vector kernel above that is not
reproduced).

# Panics
If the lengths differ (a caller bug; every caller checks first).

```rust
pub fn ddot(a: &[f64], b: &[f64]) -> f64 { /* ... */ }
```

#### Function `vec_dot`

numpy's `dot(w, s)` with 1-D `w` (k) and `s` (k × m): ddot for one
column, otherwise OpenBLAS gemv-N's per-column order (pairs of FMAs in
fours, then a chain); measured for m = 2, 3.

# Errors
[`RafflesError::DimensionMismatch`] if `w.len() != s.rows()`.

```rust
pub fn vec_dot(w: &[f64], s: &Matrix) -> crate::Result<Vec<f64>> { /* ... */ }
```

#### Function `mean_rows`

numpy's `mean(rows, axis=0)`: a sequential sum over rows, then ÷ N; for
a one-column matrix numpy's pairwise sum
([`crate::bayesian::koh::pairwise_sum`], reused), because that axis is
contiguous.

# Errors
[`RafflesError::DimensionMismatch`] on an empty matrix.

```rust
pub fn mean_rows(m: &Matrix) -> crate::Result<Vec<f64>> { /* ... */ }
```

#### Function `sum_matrices`

numpy's `sum(stack, axis=0)` over a stack of equal-shaped matrices:
sequential, or numpy's pairwise sum for a stack of 1×1 matrices.

# Errors
On an empty stack or a shape mismatch.

```rust
pub fn sum_matrices(stack: &[Matrix]) -> crate::Result<Matrix> { /* ... */ }
```

#### Function `outer`

**Attributes:**

- `MustUse { reason: None }`

numpy's `outer(a, b)` (exact products).

```rust
pub fn outer(a: &[f64], b: &[f64]) -> Matrix { /* ... */ }
```

### Re-exports

#### Re-export `nees`

```rust
pub use consistency::nees;
```

#### Re-export `nis`

```rust
pub use consistency::nis;
```

#### Re-export `order_by_derivative`

```rust
pub use discretization::order_by_derivative;
```

#### Re-export `q_continuous_white_noise`

```rust
pub use discretization::q_continuous_white_noise;
```

#### Re-export `q_discrete_white_noise`

```rust
pub use discretization::q_discrete_white_noise;
```

#### Re-export `ExtendedKalmanFilter`

```rust
pub use ekf::ExtendedKalmanFilter;
```

#### Re-export `gaussian_draws`

```rust
pub use enkf::gaussian_draws;
```

#### Re-export `outer_product_sum`

```rust
pub use enkf::outer_product_sum;
```

#### Re-export `EnsembleKalmanFilter`

```rust
pub use enkf::EnsembleKalmanFilter;
```

#### Re-export `rts_smoother`

```rust
pub use kalman::rts_smoother;
```

#### Re-export `BatchOutput`

```rust
pub use kalman::BatchOutput;
```

#### Re-export `KalmanFilter`

```rust
pub use kalman::KalmanFilter;
```

#### Re-export `RtsOutput`

```rust
pub use kalman::RtsOutput;
```

#### Re-export `Matrix`

```rust
pub use numpy_ops::Matrix;
```

#### Re-export `MerweScaledSigmaPoints`

```rust
pub use sigma_points::MerweScaledSigmaPoints;
```

#### Re-export `unscented_transform`

```rust
pub use ukf::unscented_transform;
```

#### Re-export `UnscentedKalmanFilter`

```rust
pub use ukf::UnscentedKalmanFilter;
```

## Module `gnn`

Graph neural networks for physics: message passing, and the physics-guided
bound on how much of it is enough.

# Why this is in RAFFLES

A message-passing graph network trained on simulation output is a
**surrogate model** — the thing [`crate::surrogate`] is for, and the thing a
UQ campaign needs when the real model is too expensive to sample thousands
of times. The intended consumers are the workspace's particle and transport
codes: granular DEM (`outram-park-fork-liggghts`), Monte Carlo transport
(`outram-mc-libs`) and the TRISO/Lagrangian work in `boon-lay`, all of which
already have a natural graph structure — contacts, cells, particles.

# The part worth having first

[`bound`] — the physics-guided lower bound on message-passing iterations —
needs **no neural network and no `burn`**, and is useful on its own. It
answers a question that is otherwise settled by hyperparameter search: how
many message-passing steps does this PDE on this mesh actually require? Get
it wrong downwards and the network **under-reaches**: its prediction cannot
physically depend on the data that determine the answer, so it fails in
rollout no matter how well it trains.

[`graph`] holds the topology that bound is computed from — radius and
contact graphs, hop distances, diameters, receptive fields, disjoint-union
batching — and is likewise `burn`-free. ~~`mc_geometry` builds that
topology from an `outram-mc-libs` CSG geometry~~ **CORRECTED 2026-10-02**:
the CSG adapter (`cell_adjacency_graph`) moved to
`outram_blender::gnn_graph`, behind that crate's `gnn-graph` feature, by
maintainer decision (GitHub issue #486: `outram-blender` owns geometry
description). RAFFLES no longer depends on `outram-mc-libs` at all. The
matching bridge for granular DEM lives in `outram-park-fork-liggghts`
behind its `gnn` feature. Both adapters live with the geometry because
the dependency only runs one way: geometry crates depend on RAFFLES, never
the reverse.

[`mpnn`] is the network itself and [`training`] is its training loop and
autoregressive rollout; both are behind the crate's `burn` feature.
[`dataset`] reads the upstream's own `.pt` trajectory files and needs no
`burn` at all, so a caller can measure a published dataset's mesh and its
reach requirement without building a tensor library.

# Provenance

Unlike the rest of this crate, [`mpnn`] **is a port**: the upstream
Physics-guided-MPNN repository is GPL-3.0, the same licence as this
workspace, so its model code could be translated directly. That file carries
the attribution header the crate's `CLAUDE.md` requires.

[`bound`]'s formulas are *not* transcribed — they are not in the upstream
code, which fixes its iteration count by configuration — and are derived
there from the principles the paper states, with the derivation written out.
See that module for exactly how far that claim goes.

# Reference

- L. Tesan and M. M. Iparraguirre et al. (2025). On the under-reaching
  phenomenon in message passing neural PDE solvers: revisiting the CFL
  condition. arXiv:2507.08861. <https://arxiv.org/abs/2507.08861>
- T. Pfaff, M. Fortunato, A. Sanchez-Gonzalez and P. W. Battaglia (2021).
  Learning mesh-based simulation with graph networks. *ICLR 2021*.
  arXiv:2010.03409 — MeshGraphNet, the encoder–processor–decoder
  architecture the upstream builds on.

# Status

No human V&V. The reach property of the network is tested directly (see
[`mpnn`]), the bound is tested against hand-computed lattice diameters and
CFL numbers, and [`training`] runs an end-to-end Poisson experiment against
exact solutions. That experiment demonstrates the penalty for under-reaching
badly; it does NOT resolve whether reaching the bound exactly matters, and
the test says so with its measurements. Nothing here has been trained on or
validated against a published PDE dataset — though [`dataset`] now reads
those datasets, and the measurements it takes from all five of the
upstream's are recorded in its tests, including one place where the bound
computed here differs from the number the upstream's README states.

```rust
pub mod gnn { /* ... */ }
```

### Modules

## Module `bound`

The physics-guided lower bound on message-passing iterations.

# The problem: under-reaching

A message-passing graph network moves information one hop per iteration. Run
`M` iterations and a node's prediction can depend only on nodes within `M`
hops of it — no further, ever, at any amount of training. If the physics
says a node's next state depends on something further away than that, the
network is structurally unable to represent the answer. It will train, the
loss will fall, and the rollout will be wrong.

Tesan and Iparraguirre (2025) name this **under-reaching** and make the
point that `M` is therefore not a hyperparameter to tune — it is a quantity
the PDE and the mesh *determine*, in the same way and for the same reason
that the CFL condition determines a stable explicit time step.

# The two bounds

[`PdeClass`] chooses which applies, because the two families of PDE
propagate information in categorically different ways:

- **Hyperbolic** (waves, advection, elastic contact). Information travels at
  a finite speed `c`. Over one integration step it covers `c * dt`, so the
  network's reach `M * h` — `M` hops of characteristic edge length `h` —
  must cover it:

  ```text
  M >= c * dt / h
  ```

  which is the CFL number, read as a requirement on message passing rather
  than on the time step.

- **Parabolic and elliptic** (diffusion, heat conduction, Poisson). The
  governing equation propagates information across the whole domain
  instantaneously — a Poisson solve has no finite signal speed at all — so
  the only sufficient reach is the entire graph:

  ```text
  M >= graph diameter in hops
  ```

# Provenance of the formulas

The *idea* — that these bounds exist and that they explain observed rollout
failures — is Tesan and Iparraguirre's, and their repository is GPL-3.0, so
a code-level port would be permitted. The two expressions above are not
transcribed from that repository, because they do not appear in it: the
published code fixes its iteration count by configuration and the bounds
live in the paper. They are derived here from the principles the paper's
abstract states, and each derivation is written out above so a reader can
check the reasoning rather than trust a constant.

**This matters for how much weight to put on the numbers.** Where the paper
gives a sharper constant — a safety factor, a different characteristic
length, a correction for the encoder's own reach — this module's bound will
differ from it. Treat these as the elementary reach requirement, which is
necessary, rather than as a reproduction of the paper's precise statement.

# Reference

- L. Tesan and M. M. Iparraguirre et al. (2025). On the under-reaching
  phenomenon in message passing neural PDE solvers: revisiting the CFL
  condition. arXiv:2507.08861.
  <https://arxiv.org/abs/2507.08861>

```rust
pub mod bound { /* ... */ }
```

### Types

#### Enum `PdeClass`

Which family a PDE belongs to, and therefore how information propagates
through it.

The classification is the standard one for second-order PDEs and it is the
only thing the bound needs to know about the physics — which is what makes
the bound usable before any training has been done.

```rust
pub enum PdeClass {
    Hyperbolic {
        signal_speed: f64,
        time_step: f64,
    },
    Parabolic,
    Elliptic,
}
```

##### Variants

###### `Hyperbolic`

Finite propagation speed: waves, advection, elastic contact,
compressible flow. Information travels along characteristics at speed
`c`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `signal_speed` | `f64` | Characteristic signal speed, in the mesh's length unit per unit of<br>the integration step's time unit. For a wave equation this is the<br>wave speed; for advection, the advection velocity; for elastic<br>contact, the sound speed in the material. |
| `time_step` | `f64` | The integration time step the network advances per forward pass, in<br>the same time unit. |

###### `Parabolic`

Infinite propagation speed: heat conduction, diffusion. A disturbance
anywhere changes the solution everywhere immediately, however slightly.

###### `Elliptic`

No time derivative at all: Poisson, steady-state elasticity. The
solution at a point depends on the whole domain's boundary conditions
simultaneously.

##### Implementations

###### Methods

- ```rust
  pub fn has_finite_signal_speed(self: &Self) -> bool { /* ... */ }
  ```
  Whether the class propagates information at a finite speed.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `IterationBound`

The verdict on a chosen number of message-passing iterations.

```rust
pub struct IterationBound {
    pub required: usize,
    pub configured: Option<usize>,
    pub hop_length: f64,
    pub reach: Option<f64>,
    pub required_distance: Option<f64>,
    pub rationale: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `required` | `usize` | The smallest number of message-passing iterations the physics admits. |
| `configured` | `Option<usize>` | What the model is actually configured to use, if the caller supplied it. |
| `hop_length` | `f64` | Physical distance one message hop covers, in the mesh's length unit. |
| `reach` | `Option<f64>` | Physical distance the configured number of iterations reaches, if known. |
| `required_distance` | `Option<f64>` | Distance the physics requires be reached in one integration step, for a<br>hyperbolic problem; `None` for the classes with infinite signal speed,<br>where the requirement is topological rather than metric. |
| `rationale` | `String` | Which rule produced `required`, in words, for a report or a log line. |

##### Implementations

###### Methods

- ```rust
  pub fn is_satisfied(self: &Self) -> Option<bool> { /* ... */ }
  ```
  Whether the configured iteration count satisfies the bound.

- ```rust
  pub fn shortfall(self: &Self) -> Option<usize> { /* ... */ }
  ```
  How many iterations are missing, or zero when the bound is met.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `physics_guided_lower_bound`

The physics-guided lower bound on message-passing iterations, for a given
PDE class and mesh.

`hop_length` is the physical distance one message hop covers — in a radius
graph, the connectivity radius; in a mesh graph, the characteristic edge
length. Using the *mean* edge length rather than the shortest is optimistic:
the bound is only as good as the worst hop along the path the information
has to travel, so pass the smaller number when in doubt.

`configured` is the model's chosen iteration count, if there is one; passing
it fills in [`IterationBound::is_satisfied`].

# Errors

[`RafflesError::InvalidParameter`] if `hop_length` is not strictly positive,
if a hyperbolic class has a non-positive signal speed or time step, or if
the graph is disconnected and the class needs its diameter — an
infinite-speed problem on a disconnected mesh has *no* sufficient iteration
count, and reporting a finite one would be wrong rather than conservative.

```rust
pub fn physics_guided_lower_bound(class: PdeClass, graph: &super::graph::Graph, hop_length: f64, configured: Option<usize>) -> crate::Result<IterationBound> { /* ... */ }
```

## Module `dataset`

Reading the trajectory files the Physics-guided-MPNN experiments ship.

# Why this exists rather than a conversion script

The upstream's datasets — heat diffusion, waves, and the elliptic Poisson
problem — are the only data on which its central claim can actually be
tested. Reaching them from Rust means reading `torch.save` archives, and
the alternatives were worse: a one-off Python converter would make the
result unreproducible from this repository, and committing converted copies
of someone else's data would redistribute it without need.

No `burn` required: this module produces plain `f64` and `usize` data, so a
caller can inspect a dataset, build its [`Graph`] and compute its
physics-guided bound without compiling a tensor library.

# What a `torch.save` archive is

An uncompressed ZIP containing:

- `<name>/data.pkl` — a Python pickle, protocol 2, describing the object
  graph. Tensors appear in it as calls to `torch._utils._rebuild_tensor_v2`
  with a *persistent id* naming a storage, plus an offset, a shape and a
  **stride**.
- `<name>/data/<key>` — the raw bytes of each storage, little-endian.

**The strides are not decoration.** The upstream's `edge_index` is a
`[2, N]` tensor with stride `(1, 2)` — a transposed view of a contiguous
`[N, 2]` buffer — so a reader that assumes row-major contiguity silently
scrambles every edge in the graph and produces a plausible-looking,
completely wrong topology. This module honours strides, and a test pins
that specific case.

# The restricted pickle reader

Pickle is a stack machine that can, in general, call arbitrary Python. This
reader implements **only** the opcodes `torch.save` actually emits for these
files and rejects everything else, so it cannot be induced to do anything
but build data. `GLOBAL` records a name; it never resolves or calls
anything, and the only "call" honoured by `REDUCE` is the tensor rebuild.
Feeding it a hostile pickle gets an error, not execution.

That restriction is also why this is not a general PyTorch loader and should
not grow into one. If a file outside this format needs reading, the answer
is to convert it, not to widen the opcode set.

```rust
pub mod dataset { /* ... */ }
```

### Types

#### Enum `Dtype`

The element type of a stored tensor.

```rust
pub enum Dtype {
    F32,
    I64,
    F64,
}
```

##### Variants

###### `F32`

32-bit float — PyTorch's `FloatStorage`.

###### `I64`

64-bit signed integer — PyTorch's `LongStorage`.

###### `F64`

64-bit float — PyTorch's `DoubleStorage`.

Present because the upstream's plastic-collision dataset uses it while
the others use `FloatStorage`. That was found by running this reader
against the real files rather than against its own synthetic fixtures,
which is the argument for doing both.

##### Implementations

###### Methods

- ```rust
  pub fn size(self: &Self) -> usize { /* ... */ }
  ```
  Bytes per element.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Tensor`

One tensor read out of an archive, already de-strided into row-major order.

```rust
pub struct Tensor {
    pub shape: Vec<usize>,
    pub dtype: Dtype,
    pub values: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `shape` | `Vec<usize>` | Dimensions, outermost first. |
| `dtype` | `Dtype` | Element type as stored. |
| `values` | `Vec<f64>` | Values in row-major order, widened to `f64`.<br><br>Widening is lossless for `f32` and for any `i64` below 2^53, which<br>covers every index and every field value in these files. A tensor that<br>would lose precision is rejected at read time rather than quietly<br>rounded — see [`TorchArchive::tensor`]. |

##### Implementations

###### Methods

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Total number of elements.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the tensor holds no elements.

- ```rust
  pub fn rows(self: &Self) -> Result<Vec<Vec<f64>>> { /* ... */ }
  ```
  The tensor as rows, given its shape is `[rows, columns]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TorchArchive`

A parsed `torch.save` archive: every named tensor it contains, in the order
the pickle named them.

The upstream's files hold a *list* of graph objects — one per time step of a
trajectory — each carrying the same field names. Fields therefore repeat,
and [`TorchArchive::field_series`] collects one field across the whole
trajectory.

```rust
pub struct TorchArchive {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn from_bytes(bytes: &[u8]) -> Result<Self> { /* ... */ }
  ```
  Parses an archive from its bytes.

- ```rust
  pub fn fields(self: &Self) -> &[(String, Tensor)] { /* ... */ }
  ```
  Every `(name, tensor)` pair, in pickle order.

- ```rust
  pub fn field_names(self: &Self) -> Vec<&str> { /* ... */ }
  ```
  The distinct field names present.

- ```rust
  pub fn field_series(self: &Self, name: &str) -> Vec<&Tensor> { /* ... */ }
  ```
  Every tensor stored under `name`, in order — one per trajectory step.

- ```rust
  pub fn tensor(self: &Self, name: &str) -> Result<&Tensor> { /* ... */ }
  ```
  The first tensor stored under `name`.

- ```rust
  pub fn graph_from_faces(self: &Self, node_count: usize) -> Result<Graph> { /* ... */ }
  ```
  Builds the graph from a `face` tensor rather than an `edge_index`.

- ```rust
  pub fn graph(self: &Self, node_count: usize) -> Result<Graph> { /* ... */ }
  ```
  Builds the message-passing graph from the archive's `edge_index` field.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `graph`

Graph topology: the mesh or particle connectivity a message-passing network
runs on, and the reach questions that can be asked of it.

Nothing here involves machine learning or `burn`. A [`Graph`] is a plain
adjacency structure, and every function in this file is an ordinary
combinatorial computation — which is deliberate, because the physics-guided
bound in [`super::bound`] is *about* this topology and a caller who only
wants that bound should not have to pull in a tensor library to get it.

```rust
pub mod graph { /* ... */ }
```

### Types

#### Struct `Graph`

An undirected graph over `n` nodes, stored as a directed edge list with
both directions present.

# Why both directions

Message passing is directional: a message flows from a sender to a
receiver. An undirected mesh edge is therefore two messages, and storing it
as two directed edges means the aggregation step needs no special case.
[`Graph::from_undirected_edges`] does the doubling; [`Graph::new`] takes the
directed list as given, for a genuinely directed problem.

# Invariants

Every endpoint index is below [`node_count`](Self::node_count). A graph that
exists has no dangling edge, so the message-passing loop needs no bounds
check in its inner loop.

```rust
pub struct Graph {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(node_count: usize, senders: Vec<usize>, receivers: Vec<usize>) -> Result<Self> { /* ... */ }
  ```
  Builds a graph from a directed edge list.

- ```rust
  pub fn from_undirected_edges(node_count: usize, edges: &[(usize, usize)]) -> Result<Self> { /* ... */ }
  ```
  Builds a graph from undirected edges, storing each one in both

- ```rust
  pub fn radius_graph(points: &[Vec<f64>], radius: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a **radius graph**: every pair of points closer than `radius` is

- ```rust
  pub fn node_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of nodes.

- ```rust
  pub fn edge_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of directed edges — twice the undirected count for a graph built

- ```rust
  pub fn senders(self: &Self) -> &[usize] { /* ... */ }
  ```
  Sender index of each directed edge.

- ```rust
  pub fn receivers(self: &Self) -> &[usize] { /* ... */ }
  ```
  Receiver index of each directed edge.

- ```rust
  pub fn adjacency(self: &Self) -> Vec<Vec<usize>> { /* ... */ }
  ```
  Adjacency lists, one per node, listing the nodes it can send to.

- ```rust
  pub fn hop_distances(self: &Self, source: usize) -> Result<Vec<usize>> { /* ... */ }
  ```
  Hop distance from `source` to every node, with [`usize::MAX`] for nodes

- ```rust
  pub fn eccentricity(self: &Self, source: usize) -> Result<usize> { /* ... */ }
  ```
  Greatest hop distance from `source` to any node it can reach.

- ```rust
  pub fn is_connected(self: &Self) -> bool { /* ... */ }
  ```
  Whether every node can reach every other.

- ```rust
  pub fn diameter(self: &Self) -> usize { /* ... */ }
  ```
  The graph's **diameter** in hops: the greatest hop distance between any

- ```rust
  pub fn diameter_estimate(self: &Self) -> usize { /* ... */ }
  ```
  A cheap **lower bound** on the diameter, by double sweep.

- ```rust
  pub fn contact_graph(centres: &[Vec<f64>], radii: &[f64], skin: f64) -> Result<Self> { /* ... */ }
  ```
  A **contact graph** over spheres: two spheres are connected when the gap

- ```rust
  pub fn repeat(self: &Self, copies: usize) -> Result<Self> { /* ... */ }
  ```
  `copies` disjoint copies of this graph, as one graph.

- ```rust
  pub fn receptive_field(self: &Self, node: usize, iterations: usize) -> Result<Vec<usize>> { /* ... */ }
  ```
  The **receptive field** of a node after `iterations` message-passing

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `physics_guided_lower_bound`

```rust
pub use bound::physics_guided_lower_bound;
```

#### Re-export `IterationBound`

```rust
pub use bound::IterationBound;
```

#### Re-export `PdeClass`

```rust
pub use bound::PdeClass;
```

#### Re-export `TorchArchive`

```rust
pub use dataset::TorchArchive;
```

#### Re-export `Graph`

```rust
pub use graph::Graph;
```

## Module `imprecise`

Imprecise probability — intervals, probability boxes, confidence boxes, and
the reliability of coherent systems under limited data.

# Why a distribution is sometimes the wrong object

Fit a distribution to four failure observations and you get a curve with no
visible caveat attached. The curve is a fiction: four points do not pin down
a distribution, and the analysis downstream will treat it as if they had.

Imprecise probability keeps the ignorance in the object instead of
discarding it. A **probability box** ([`Pbox`]) is a pair of CDFs that
bracket the unknown true one; a **confidence box** ([`cbox_binomial`]) is a
p-box built so that its cuts *are* confidence intervals, at every level at
once. Propagate those through a system model and the answer comes out as
bounds that state what the data support, rather than a single number that
does not.

# What is here

- [`Interval`] — closed real intervals with the arithmetic the structure
  functions below need.
- [`Pbox`] — a p-box on a bounded range, stored as lower and upper CDFs on a
  shared grid. Construction from a precise distribution, from an interval,
  and from explicit bounds; cuts, CDF bounds, and an enclosure check.
- [`cbox_binomial`] — the Clopper–Pearson confidence box for a rate observed
  as `k` successes in `n` trials. The central object of the
  "computing with confidence" line of work.
- [`SystemStructure`] and [`EventDependence`] — series, parallel and
  k-out-of-n reliability, evaluated either under independence of component
  failures or with **no dependence assumption at all** (Fréchet bounds).

# Two different dependence questions, kept apart

This is where imprecise reliability analyses most often go wrong, so the API
separates them:

1. **Dependence between component failure *events*.** Do two pumps fail
   independently, or does a common cause link them? This decides the
   *formula*: a product for independence, Fréchet bounds when nothing is
   assumed. That is [`EventDependence`].
2. **Dependence between the *uncertainties* in the component
   reliabilities.** Two components' reliabilities may each be known only to
   a confidence box; are those two states of knowledge related? This decides
   how the boxes are *combined*.

[`SystemStructure::reliability_pbox`] answers (1) as the caller specifies
and (2) by combining the component boxes **at matched confidence level** —
the comonotone case, which is what the confidence-box literature reports as
"generalised confidence bounds", and which this module states rather than
leaves implicit. General unknown-dependence p-box convolution is *not*
implemented here; see [`SystemStructure::reliability_pbox`] for what that would
take. ~~General unknown-dependence p-box convolution is not implemented~~
**CORRECTED 2026-10-20 (gh:#987)** — it now exists, in
[`crate::scram::pba`] (pba.r's Fréchet convolution on this module's
[`Pbox`] ladder, plus `and`/`or`/`andI`/`orI`), and
[`crate::scram::pbox_tree`] applies it to a whole fault tree. The
matched-level combination below is unchanged.

# References

- A. Lye, W. Vechgama, M. Sallak, S. Destercke, S. Ferson and S. Xiao
  (2024). Advances in the reliability analysis of coherent systems under
  limited data with confidence boxes. *ASCE-ASME Journal of Risk and
  Uncertainty in Engineering Systems Part A: Civil Engineering, 11*,
  04024074. doi:
  [10.1061/AJRUA6.RUENG-1380](https://doi.org/10.1061/AJRUA6.RUENG-1380)
- C. J. Clopper and E. S. Pearson (1934). The use of confidence or fiducial
  limits illustrated in the case of the binomial. *Biometrika, 26*(4),
  404–413. doi: [10.1093/biomet/26.4.404](https://doi.org/10.1093/biomet/26.4.404)
- S. Ferson, V. Kreinovich, L. Ginzburg, D. S. Myers and K. Sentz (2003).
  Constructing probability boxes and Dempster-Shafer structures. Sandia
  National Laboratories, SAND2002-4015. doi:
  [10.2172/809606](https://doi.org/10.2172/809606)
- M. Fréchet (1935). Généralisations du théorème des probabilités totales.
  *Fundamenta Mathematicae, 25*, 379–387.
- S. Ferson, R. B. Nelsen, J. Hajagos, D. J. Berleant, J. Zhang, W. T.
  Tucker, L. R. Ginzburg and W. L. Oberkampf (2004). Dependence in
  probabilistic modeling, Dempster-Shafer theory, and probability bounds
  analysis. Sandia National Laboratories, SAND2004-3072. doi:
  [10.2172/919189](https://doi.org/10.2172/919189)

Independent implementations from the published definitions — see the
provenance note in [`crate::bayesian`].

```rust
pub mod imprecise { /* ... */ }
```

### Types

#### Struct `Interval`

A closed real interval `[lower, upper]`.

The arithmetic here is the standard interval arithmetic, restricted to the
operations the reliability structure functions need. Every operation is
*rigorous*: the result encloses every value the true operands could produce.

```rust
pub struct Interval {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(lower: f64, upper: f64) -> Result<Self> { /* ... */ }
  ```
  Builds `[lower, upper]`.

- ```rust
  pub fn point(value: f64) -> Result<Self> { /* ... */ }
  ```
  A degenerate interval containing one value.

- ```rust
  pub fn lower(self: &Self) -> f64 { /* ... */ }
  ```
  The lower bound.

- ```rust
  pub fn upper(self: &Self) -> f64 { /* ... */ }
  ```
  The upper bound.

- ```rust
  pub fn width(self: &Self) -> f64 { /* ... */ }
  ```
  Width `upper - lower`, a measure of how much is not known.

- ```rust
  pub fn contains(self: &Self, value: f64) -> bool { /* ... */ }
  ```
  Whether `value` lies in the closed interval.

- ```rust
  pub fn encloses(self: &Self, other: &Interval) -> bool { /* ... */ }
  ```
  Whether this interval encloses `other` entirely.

- ```rust
  pub fn add(self: &Self, other: &Interval) -> Interval { /* ... */ }
  ```
  Interval sum.

- ```rust
  pub fn multiply(self: &Self, other: &Interval) -> Interval { /* ... */ }
  ```
  Interval product.

- ```rust
  pub fn complement(self: &Self) -> Interval { /* ... */ }
  ```
  The complement `1 - x`, which for a probability interval swaps and

- ```rust
  pub fn min(self: &Self, other: &Interval) -> Interval { /* ... */ }
  ```
  Element-wise minimum with another interval.

- ```rust
  pub fn max(self: &Self, other: &Interval) -> Interval { /* ... */ }
  ```
  Element-wise maximum with another interval.

- ```rust
  pub fn clamp_to_unit(self: &Self) -> Interval { /* ... */ }
  ```
  Clamps both bounds into `[0, 1]`, for quantities that are probabilities.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `EventDependence`

How component failure events depend on one another.

This chooses the *structure function*, not the arithmetic: see the module
documentation on keeping the two dependence questions apart.

```rust
pub enum EventDependence {
    Independent,
    Unknown,
}
```

##### Variants

###### `Independent`

Components fail independently — the textbook assumption.

Series reliability is the product of component reliabilities; parallel
is one minus the product of the unreliabilities. Convenient, and wrong
whenever a common cause exists — which in a real plant it usually does.

###### `Unknown`

**Nothing is assumed** about how failures are related.

The result is the Fréchet bound: the tightest interval that holds for
*every* possible dependence structure, from perfect positive to perfect
negative. For a series system of `n` components,
`[max(0, sum(R_i) - (n-1)), min(R_i)]`; for a parallel system,
`[max(R_i), min(1, sum(R_i))]`.

These are wide — that is the honest cost of not knowing — and they are
*guaranteed*, which the independence answer is not. A designer who
assumes independence is making a claim about the plant; choosing this
variant is declining to.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SystemStructure`

The reliability structure of a coherent system.

"Coherent" in the reliability sense: the system is monotone (repairing a
component never hurts) and every component matters. That monotonicity is
what makes bound propagation valid — applying the structure function to the
component bounds gives the system bounds, with no search needed.

```rust
pub enum SystemStructure {
    Series,
    Parallel,
    KOutOfN {
        k: usize,
    },
}
```

##### Variants

###### `Series`

All components must work: the system is as weak as its weakest part.

###### `Parallel`

Any one component suffices: full redundancy.

###### `KOutOfN`

At least `k` of the `n` components must work.

Covers the middle ground that series and parallel do not — a 2-out-of-3
voting logic, or a pump train where two of four suffice.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `k` | `usize` | How many components must work; `1` is parallel and `n` is series. |

##### Implementations

###### Methods

- ```rust
  pub fn reliability_interval(self: &Self, components: &[Interval], dependence: EventDependence) -> Result<Interval> { /* ... */ }
  ```
  System reliability from *interval-valued* component reliabilities.

- ```rust
  pub fn reliability_pbox(self: &Self, components: &[Pbox], dependence: EventDependence) -> Result<Pbox> { /* ... */ }
  ```
  System reliability as a p-box, from component reliability p-boxes.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Pbox`

A probability box: a pair of CDFs that bracket an unknown distribution.

# Representation

Stored as **quantile bounds at a fixed ladder of probability levels**:
`levels` equally-spaced levels, and at each one a lower and an upper
quantile. This is the representation the confidence-box literature works in
— a cut at level `alpha` is exactly a confidence interval at that level —
and it makes the operations this module needs into interval arithmetic on
matched cuts.

The alternative representation, CDF bounds on a value grid, is better for
convolution-style p-box arithmetic and is not what is stored here. Nothing
prevents adding it later; nothing in this module needs it.

# Invariants

Both quantile sequences are non-decreasing, and `lower[i] <= upper[i]` at
every level. [`Pbox::from_quantile_bounds`] enforces both; a p-box that
exists is a valid one.

```rust
pub struct Pbox {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn from_quantile_bounds(lower: Vec<f64>, upper: Vec<f64>) -> Result<Self> { /* ... */ }
  ```
  Builds a p-box from explicit quantile bounds.

- ```rust
  pub fn from_distribution<D>(distribution: &D, levels: usize) -> Result<Self>
where
    D: ContinuousDistribution1D { /* ... */ }
  ```
  A **precise** p-box: zero width everywhere, from a known distribution.

- ```rust
  pub fn from_interval(interval: Interval, levels: usize) -> Result<Self> { /* ... */ }
  ```
  The **vacuous** p-box on an interval: everything that is known is that

- ```rust
  pub fn levels(self: &Self) -> usize { /* ... */ }
  ```
  Number of probability levels in the ladder.

- ```rust
  pub fn level_probability(self: &Self, level: usize) -> f64 { /* ... */ }
  ```
  The probability level at index `level`, in `[0, 1]`.

- ```rust
  pub fn quantile_interval(self: &Self, level: usize) -> Result<Interval> { /* ... */ }
  ```
  The quantile bounds at a level index — the interval the quantity's

- ```rust
  pub fn cut(self: &Self, alpha: f64) -> Result<Interval> { /* ... */ }
  ```
  The two-sided interval between probability levels `alpha / 2` and

- ```rust
  pub fn cdf_bounds(self: &Self, x: f64) -> (f64, f64) { /* ... */ }
  ```
  Bounds on the CDF at `x`: `(lower, upper)` with `lower <= P(X <= x) <=

- ```rust
  pub fn mean_interval(self: &Self) -> Interval { /* ... */ }
  ```
  The interval of means consistent with this p-box.

- ```rust
  pub fn encloses_distribution<D>(self: &Self, distribution: &D) -> Result<bool>
where
    D: ContinuousDistribution1D { /* ... */ }
  ```
  Whether this p-box encloses a given distribution at every level — the

- ```rust
  pub fn lower_quantiles(self: &Self) -> &[f64] { /* ... */ }
  ```
  The lower quantile sequence, tracing the upper CDF.

- ```rust
  pub fn upper_quantiles(self: &Self) -> &[f64] { /* ... */ }
  ```
  The upper quantile sequence, tracing the lower CDF.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `cbox_binomial`

The Clopper–Pearson **confidence box** for a rate observed as `k` successes
in `n` trials.

# What a confidence box is

An ordinary confidence interval answers one question: "at 95 %, where is the
rate?" A confidence box answers it at *every* level simultaneously. Its
bounds are

```text
lower CDF:  Beta(k,     n - k + 1)
upper CDF:  Beta(k + 1, n - k)
```

so that cutting it at `alpha` reproduces exactly the Clopper–Pearson
interval at `1 - alpha`. The degenerate parameters at `k = 0` and `k = n`
are handled as the literature does: the missing side becomes the point 0 or
1 respectively, which is why a c-box from zero failures in `n` trials gives
`[0, something]` rather than an undefined answer — the case that matters
most in reliability, and the one a naive `k / n` point estimate answers with
a confident and useless zero.

# Why this object and not a posterior

A c-box makes no prior assumption. With three failures in ten trials a
Bayesian answer depends on a prior nobody can defend from three failures;
the c-box's cuts are frequentist confidence statements that hold whatever
the truth is. Under limited data that difference is the entire argument of
the "computing with confidence" line of work.

# Errors

[`RafflesError::InvalidParameter`] if `n` is zero, if `k > n`, or if
`levels` is below 2.

```rust
pub fn cbox_binomial(k: usize, n: usize, levels: usize) -> crate::Result<Pbox> { /* ... */ }
```

## Module `model_selection`

Bayesian model selection: comparing competing models by their evidence.

# What this is for

Parameter estimation asks "given this model, what are its parameters?".
Model selection asks the prior question: **which model?** A stiffer spring
or a nonlinear one; a Weibull failure law or a lognormal; a two-parameter
creep model or a four-parameter one. The Bayesian answer is the ratio of the
models' evidences — their marginal likelihoods — which the transitional
samplers in [`crate::bayesian`] already produce as a by-product of sampling.

# The Occam factor is not an add-on

The evidence integrates the likelihood over the *whole prior*, so a model
with more parameters, or vaguer priors on them, pays for the parameter space
it does not use. That penalty is automatic: there is no AIC-style correction
term to remember, and none to get wrong. A four-parameter model must fit
enough better to repay the volume it spends.

[`crate::bayesian::transitional`] has a test measuring exactly this: widening
a prior tenfold lowered the log-evidence by 2.23 nats against a closed-form
2.18.

# What this module will not do for you

- **It cannot rescue a bad evidence estimate.** The Bayes factor inherits
  every error in the two log-evidences, and a sampler that has not converged
  produces a confident, wrong one. Compare the stage reports of the two runs
  before trusting their ratio.
- **It says nothing about whether either model is any good.** A Bayes factor
  of 1000 means one model is far better than the other; both may still be
  hopeless. That is a posterior-predictive question, not a model-selection
  one.
- **The interpretation scale is a convention, not a result.** See
  [`EvidenceStrength`].

# References

- H. Jeffreys (1961). *Theory of Probability*, 3rd edition. Oxford
  University Press — the original interpretation scale.
- R. E. Kass and A. E. Raftery (1995). Bayes factors. *Journal of the
  American Statistical Association, 90*(430), 773–795. doi:
  [10.1080/01621459.1995.10476572](https://doi.org/10.1080/01621459.1995.10476572)
  — the scale used here, and the standard modern reference.
- J. Ching and Y.-C. Chen (2007). Transitional Markov Chain Monte Carlo
  method for Bayesian model updating, model class selection, and model
  averaging. *Journal of Engineering Mechanics, 133*(7), 816–832. doi:
  [10.1061/(ASCE)0733-9399(2007)133:7(816)](https://doi.org/10.1061/(ASCE)0733-9399(2007)133:7(816))
  — model class selection from TMCMC evidence, which is what this module
  consumes.

```rust
pub mod model_selection { /* ... */ }
```

### Types

#### Enum `EvidenceStrength`

How strongly a Bayes factor favours one model over another, on the
Kass–Raftery scale.

**This is a convention.** The thresholds are conventional reading aids, not
results: nothing changes about the evidence at `2 * ln B = 6`. They are
offered because a bare number invites over-reading in both directions, and
because a report that says "strong" is easier to argue with than one that
says "3.7".

```rust
pub enum EvidenceStrength {
    NotWorthMoreThanABareMention,
    Positive,
    Strong,
    VeryStrong,
}
```

##### Variants

###### `NotWorthMoreThanABareMention`

`2 ln B` below 2: the data barely distinguish the models.

###### `Positive`

`2 ln B` in `[2, 6)`.

###### `Strong`

`2 ln B` in `[6, 10)`.

###### `VeryStrong`

`2 ln B` at or above 10.

##### Implementations

###### Methods

- ```rust
  pub fn from_ln_bayes_factor(ln_bayes_factor: f64) -> Self { /* ... */ }
  ```
  Classifies a natural-log Bayes factor on the Kass–Raftery scale, which

- ```rust
  pub fn as_str(self: &Self) -> &'static str { /* ... */ }
  ```
  The conventional phrase, for a report line.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ModelEvidence`

One candidate model in a comparison: a name and its log-evidence.

```rust
pub struct ModelEvidence {
    pub name: String,
    pub ln_evidence: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | How the model is referred to in the report. |
| `ln_evidence` | `f64` | Natural log of the model's evidence `p(D | M)`, as returned by<br>[`crate::bayesian::TransitionalResult::ln_evidence`].<br><br>The log, always: a realistic data set drives the evidence itself far<br>below `f64::MIN_POSITIVE`, so a comparison written in terms of the raw<br>evidence silently becomes `0 / 0`. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, ln_evidence: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a candidate.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ModelComparison`

A ranked comparison of several models.

```rust
pub struct ModelComparison {
    pub ranked: Vec<ModelEvidence>,
    pub probabilities: Vec<f64>,
    pub ln_bayes_factor_over_runner_up: Option<f64>,
    pub strength: Option<EvidenceStrength>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ranked` | `Vec<ModelEvidence>` | The candidates, ordered best first. |
| `probabilities` | `Vec<f64>` | Posterior probability of each entry in [`ranked`](Self::ranked), under<br>equal prior model probabilities. |
| `ln_bayes_factor_over_runner_up` | `Option<f64>` | Natural-log Bayes factor of the best model against the runner-up, or<br>`None` when only one model was supplied. |
| `strength` | `Option<EvidenceStrength>` | How strong that margin is, on the Kass–Raftery scale. |

##### Implementations

###### Methods

- ```rust
  pub fn best(self: &Self) -> &ModelEvidence { /* ... */ }
  ```
  The winning model.

- ```rust
  pub fn summary(self: &Self) -> String { /* ... */ }
  ```
  A one-line summary for a report or a log.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `ln_bayes_factor`

The natural-log Bayes factor of model `a` against model `b`.

`ln B = ln p(D | M_a) - ln p(D | M_b)`. Positive favours `a`.

Returned as a log for the same reason the inputs are logs: `exp` of it
overflows or underflows for any comparison decisive enough to be
interesting.

```rust
pub fn ln_bayes_factor(a: &ModelEvidence, b: &ModelEvidence) -> f64 { /* ... */ }
```

#### Function `posterior_model_probabilities`

Posterior probability of each model, given equal prior probabilities.

A softmax over the log-evidences, computed with the usual maximum shift so
that a set of log-evidences around -15 000 — entirely ordinary for a few
hundred observations — does not underflow to a vector of zeros.

The result is aligned with the input and sums to 1.

# The equal-prior assumption is doing work

Equal prior model probabilities is a choice, and in a safety case it is
often the wrong one: a model that contradicts established physics should not
start level with one that does not. Where priors differ, add
`ln p(M_i)` to each log-evidence before calling this — the arithmetic is the
same, and doing it at the call site keeps the assumption visible.

# Errors

[`RafflesError::InvalidParameter`] if `models` is empty.

```rust
pub fn posterior_model_probabilities(models: &[ModelEvidence]) -> crate::Result<Vec<f64>> { /* ... */ }
```

#### Function `compare_models`

Ranks models by evidence and reports the margin.

# Errors

[`RafflesError::InvalidParameter`] if `models` is empty.

```rust
pub fn compare_models(models: &[ModelEvidence]) -> crate::Result<ModelComparison> { /* ... */ }
```

## Module `openda`

**OpenDA's data-assimilation and calibration algorithms** (gh:#1192).

Ported from OpenDA (OpenDA Association, Java,
<https://github.com/OpenDA-Association/OpenDA>, commit `e51565c2c8de`,
LGPL-3.0-or-later; `LICENSE-LGPL-3.0-OPENDA`, crate `NOTICE`), the
algorithms and not the Java framework (no XML configuration, no model
wrappers):

| Item | Upstream |
|---|---|
| [`run_sequential`], [`SequentialFilter`] | `AbstractSequentialAlgorithm`, `AbstractSequentialEnsembleAlgorithm` |
| [`enkf_update`], [`enkf_analysis`], [`Localization`], [`zhang_weights`] | `EnKF`, `EnsembleVectors`, `AutoLocalizationZhang2011` |
| [`ensr_update`], [`ensr_analysis`] | `EnSR` |
| [`particle_filter_update`], [`residual_resampling`] | `ParticleFilter` (residual resampling only, as upstream) |
| [`steady_state_analysis`], [`SteadyStateGain`] | `SteadyStateFilter` |
| [`Dud`] | `BaseDudCoreOptimizer`, `DudCoreOptimizer` |
| [`Simplex`] | `SimplexCoreOptimizer` |
| [`Powell`] | `PowellCoreOptimizer` |
| [`WeightedLeastSquares`], [`SimulationPredictor`] | `SimulationKwadraticCostFunction` (OpenDA's weighted cost) |
| [`SimpleCost`], [`RosenbrockCost`], [`simple_least_squares`] | OpenDA's test cost functions |
| [`StochVector`] | `StochVector`, `SqrtCovariance` (noise and uncertainty) |
| [`SimpleStochModel`], [`Dynamics`], [`Observations`] | `SimpleStochModelInstance` with the oscillator and Lorenz test models; `CsvStochObserver` |
| [`linalg`] | `org.openda.utils.Matrix`, `VectorDouble` |
| [`jama`] | JAMA 1.0.2's SVD (public domain), which OpenDA's `inverse`/`sqrt`/`rightSolve` use |

Written here, not ported: [`DrawSource`] (replays OpenDA's recorded draws
for differential tests, or draws from the workspace LCG); the
[`OdaStochModel`], [`CostFunction`] and [`Predictor`] seams (traits
consumed generically, after OpenDA's interfaces). The typed route for a
caller's [`crate::fidelity::StateSpaceModel`] is
[`crate::fidelity::Assimilator::OpenDaEnsemble`].

The kernels round as Java does (sequential IEEE, no FMA), so on the same
inputs and draws the port is **bit-identical** with OpenDA; upstream
quirks are ported faithfully and listed in each file's header and in
`verification_and_validation/openda_gh1192.md` §3.

**Synthetic data only** (`RESPONSIBLE_USE.md`): twin experiments,
teaching and V&V, never the state of a real plant.

V&V: `crates/raffles/verification_and_validation/openda_gh1192.md`,
tests `tests/openda_gh1192.rs`.

```rust
pub mod openda { /* ... */ }
```

### Modules

## Module `cost`

**OpenDA's cost functions**, including the weighted least-squares cost
its calibration algorithms minimise (gh:#1192).

```rust
pub mod cost { /* ... */ }
```

### Types

#### Struct `CostHistory`

Every evaluation, in order, and the best so far (OpenDA's `allPars`,
`allCosts`, `allPreds`, `pMin`, `fMin`, `predMin`).

```rust
pub struct CostHistory {
    pub parameters: Vec<Vec<f64>>,
    pub costs: Vec<f64>,
    pub predictions: Vec<Vec<f64>>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `parameters` | `Vec<Vec<f64>>` | Evaluated parameters. |
| `costs` | `Vec<f64>` | Their costs. |
| `predictions` | `Vec<Vec<f64>>` | Their predictions (least-squares costs only). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn record(self: &mut Self, p: &[f64], cost: f64, pred: Option<&[f64]>) { /* ... */ }
  ```
  Record one evaluation; the best is replaced only on a strict

- ```rust
  pub fn optimum(self: &Self) -> Option<(f64, &[f64])> { /* ... */ }
  ```
  `getOptimalCost()`, `getOptimalParameters()`.

- ```rust
  pub fn optimal_predictions(self: &Self) -> Option<&[f64]> { /* ... */ }
  ```
  The predictions at the optimum.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `LeastSquaresView`

What a least-squares cost exposes to the optimisers.

```rust
pub struct LeastSquaresView {
    pub observations: super::noise::StochVector,
    pub parameters: super::noise::StochVector,
    pub background: bool,
    pub last_predictions: Option<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `observations` | `super::noise::StochVector` | Observations and their (independent) errors. |
| `parameters` | `super::noise::StochVector` | The parameters' prior uncertainty (search directions, background). |
| `background` | `bool` | Whether the background term is in the cost. |
| `last_predictions` | `Option<Vec<f64>>` | Predictions of the last evaluation. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SimpleCost`

OpenDA's `SimpleCostFunction`: `Σ ((p_i − 1)/w_i)²`, `w = (1, 2)`.

```rust
pub struct SimpleCost {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **CostFunction**
  - ```rust
    fn evaluate(self: &mut Self, p: &[f64]) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn history(self: &Self) -> &CostHistory { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `RosenbrockCost`

OpenDA's `RosenbrockCostFunction` (factor 100, pairs of parameters).

```rust
pub struct RosenbrockCost {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **CostFunction**
  - ```rust
    fn evaluate(self: &mut Self, p: &[f64]) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn history(self: &Self) -> &CostHistory { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `IdentityPredictor`

The identity model `y = p` of `SimpleLeastSquaresCostFunction`.

```rust
pub struct IdentityPredictor;
```

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Predictor**
  - ```rust
    fn predict(self: &Self, p: &[f64]) -> Result<Vec<f64>> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SimulationPredictor`

`SimulationKwadraticCostFunction`'s model run for an [`OdaStochModel`]:
a fresh copy of the template, `setParameters(p)`, observations
announced, run to the horizon's end without noise, then observed.

```rust
pub struct SimulationPredictor<M> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(template: M, observations: &Observations) -> Result<Self> { /* ... */ }
  ```
  The observations kept are those in the model's time horizon

- ```rust
  pub fn observations(self: &Self) -> &Observations { /* ... */ }
  ```
  The observations used.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Predictor**
  - ```rust
    fn predict(self: &Self, p: &[f64]) -> Result<Vec<f64>> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `WeightedLeastSquares`

OpenDA's weighted cost `J = f·‖(y − h(p))/σ‖² + [f·‖L_p⁻¹(p − p̄)‖²]`,
`f = factor` (0.5 in OpenDA), each `‖·‖²` as `Math.pow(norm2, 2)`.

```rust
pub struct WeightedLeastSquares<P> {
    pub factor: f64,
    pub background: bool,
    pub background_first: bool,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `factor` | `f64` | The factor on both terms. |
| `background` | `bool` | Add the background term. |
| `background_first` | `bool` | Add Jb before Jo (`SimpleLeastSquaresCostFunction`'s order). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(predictor: P, observations: StochVector, parameters: StochVector, factor: f64, background: bool) -> Self { /* ... */ }
  ```
  From a predictor, observed values with σ, and the parameter prior.

- ```rust
  pub fn predictor(self: &Self) -> &P { /* ... */ }
  ```
  The predictor.

- ```rust
  pub fn simulation(template: M, observations: &Observations, background: bool) -> Result<Self> { /* ... */ }
  ```
  `new SimulationKwadraticCostFunction(factory, observer)` with

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **CostFunction**
  - ```rust
    fn evaluate(self: &mut Self, p: &[f64]) -> Result<f64> { /* ... */ }
    ```

  - ```rust
    fn history(self: &Self) -> &CostHistory { /* ... */ }
    ```

  - ```rust
    fn least_squares(self: &Self) -> Option<LeastSquaresView> { /* ... */ }
    ```

  - ```rust
    fn multiplication_factor(self: &Self) -> f64 { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `CostFunction`

OpenDA's `ICostFunction`, consumed generically by [`super::simplex`],
[`super::powell`] and [`super::dud`].

```rust
pub trait CostFunction {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `evaluate`: The cost at `p`, recorded in [`CostFunction::history`].
- `history`: Every evaluation so far.

##### Provided Methods

- ```rust
  fn least_squares(self: &Self) -> Option<LeastSquaresView> { /* ... */ }
  ```
  `Some` for a least-squares cost (Java's `instanceof`).

- ```rust
  fn multiplication_factor(self: &Self) -> f64 { /* ... */ }
  ```
  `getMultiplicationFactor()`.

##### Implementations

This trait is implemented for the following types:

- `SimpleCost`
- `RosenbrockCost`
- `WeightedLeastSquares<P>` with <P: Predictor>

#### Trait `Predictor`

A caller's model for the weighted least-squares cost: predictions of
the observations at parameters `p`.

```rust
pub trait Predictor {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `predict`: The predictions.

##### Implementations

This trait is implemented for the following types:

- `IdentityPredictor`
- `SimulationPredictor<M>` with <M: OdaStochModel>

### Functions

#### Function `simple_least_squares`

OpenDA's `SimpleLeastSquaresCostFunction`: `y = p`, observations
(1, 1) ± (1, 2), parameter prior (0, 0) ± (1, 2), factor 1.

# Errors
Never (fixed, consistent shapes).

```rust
pub fn simple_least_squares(background: bool) -> crate::Result<WeightedLeastSquares<IdentityPredictor>> { /* ... */ }
```

## Module `dud`

**OpenDA's DUD** ("doesn't use derivatives"), a secant Gauss–Newton for
weighted least squares (gh:#1192).

```rust
pub mod dud { /* ... */ }
```

### Types

#### Struct `Dud`

OpenDA's `DudCoreOptimizer` (unconstrained).

```rust
pub struct Dud {
    pub initial_step: f64,
    pub max_iterations: usize,
    pub max_inner_iterations: usize,
    pub max_total_evaluations: Option<usize>,
    pub inner_scale_fac: f64,
    pub min_inner_negative_look: usize,
    pub max_step: f64,
    pub abs_tol: f64,
    pub rel_tol: f64,
    pub rel_tol_lin_cost: f64,
    pub compute_error_estimate: bool,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `initial_step` | `f64` | `initialStep` (1.0): the search steps are this times the prior<br>uncertainty's columns. |
| `max_iterations` | `usize` | `maxit` (3). |
| `max_inner_iterations` | `usize` | `maxInnerIter` (6). |
| `max_total_evaluations` | `Option<usize>` | `maxTotalEvaluations` (None = OpenDA's −1, unlimited). |
| `inner_scale_fac` | `f64` | `innerScaleFac` (0.5). Its sign flips during backtracking. |
| `min_inner_negative_look` | `usize` | `minInnerNegativeLook` (3). |
| `max_step` | `f64` | `maxStep` (10), in the basis of the stored parameter differences. |
| `abs_tol` | `f64` | `absTol` (0.01). |
| `rel_tol` | `f64` | `relTol` (0.01). |
| `rel_tol_lin_cost` | `f64` | `relTolLinCost` (0.01). |
| `compute_error_estimate` | `bool` | `computeErrorEstimate` (true). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn initialize<C: CostFunction>(self: &mut Self, f: &mut C, p_init: &[f64]) -> Result<()> { /* ... */ }
  ```
  `initialize(pInit)`: `pInit` and `pInit + initial_step·L_j` for each

- ```rust
  pub fn current(self: &Self) -> (&[Vec<f64>], &[f64]) { /* ... */ }
  ```
  The stored points, best first.

- ```rust
  pub fn iterations(self: &Self) -> usize { /* ... */ }
  ```
  Outer iterations done.

- ```rust
  pub fn std_values(self: &Self) -> Option<&[f64]> { /* ... */ }
  ```
  The linearised standard deviations of the parameters at the last

- ```rust
  pub fn sqrt_covariance(self: &Self) -> Option<&Rows> { /* ... */ }
  ```
  The linearised square root of the parameters' covariance

- ```rust
  pub fn optimize<C: CostFunction>(self: &mut Self, f: &mut C) -> Result<()> { /* ... */ }
  ```
  `optimize()`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `enkf`

**OpenDA's ensemble Kalman filter** with its localisation options
(gh:#1192).

```rust
pub mod enkf { /* ... */ }
```

### Types

#### Enum `Localization`

OpenDA's `localization` options for the EnKF.

```rust
pub enum Localization {
    None,
    Hamill,
    AutoZhang {
        n_bootstrap: usize,
        sigma_alpha2: f64,
    },
}
```

##### Variants

###### `None`

No localisation.

###### `Hamill`

Hamill et al. (2001): Schur product of each gain column with the
model's mask ([`OdaStochModel::observed_localization`]).

###### `AutoZhang`

Zhang & Oliver (2011): weights from bootstrap gains.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `n_bootstrap` | `usize` | Bootstrap gains (OpenDA: 50). |
| `sigma_alpha2` | `f64` | σ²_α (OpenDA: 0.36, the paper's optimum). |

##### Implementations

###### Methods

- ```rust
  pub fn auto_zhang() -> Self { /* ... */ }
  ```
  OpenDA's `autoZhang` settings.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `LocalizationInput`

The localisation an [`enkf_update`] applies, with its input.

```rust
pub enum LocalizationInput {
    None,
    Hamill(Vec<Vec<f64>>),
    AutoZhang {
        n_bootstrap: usize,
        sigma_alpha2: f64,
    },
}
```

##### Variants

###### `None`

None.

###### `Hamill`

Hamill: one mask per observation, each the state's length.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Vec<f64>>` |  |

###### `AutoZhang`

Zhang & Oliver, with OpenDA's settings.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `n_bootstrap` | `usize` | Bootstrap gains. |
| `sigma_alpha2` | `f64` | σ²_α. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `EnsembleVectors`

OpenDA's `EnsembleVectors`: deviations and mean.

```rust
pub struct EnsembleVectors {
    pub ensemble: Vec<Vec<f64>>,
    pub mean: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ensemble` | `Vec<Vec<f64>>` | Members with the mean removed. |
| `mean` | `Vec<f64>` | The mean. |

##### Implementations

###### Methods

- ```rust
  pub fn from_members(members: Vec<Vec<f64>>) -> Self { /* ... */ }
  ```
  `getEnsembleVectorsState`/`Prediction`: `ensembleAverage`, then

- ```rust
  pub fn as_openda_constructor(members: Vec<Vec<f64>>) -> Self { /* ... */ }
  ```
  `new EnsembleVectors(IVector[])`: the mean of all members, removed

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `EnkfUpdate`

One EnKF analysis on vectors.

```rust
pub struct EnkfUpdate {
    pub deltas: Vec<Vec<f64>>,
    pub gain_pre: Vec<Vec<f64>>,
    pub gain_post: Vec<Vec<f64>>,
    pub innovation_covariance: super::jama::Rows,
    pub predicted_mean: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `deltas` | `Vec<Vec<f64>>` | The increment to add to each member (`axpyOnState(1.0, ·)`). |
| `gain_pre` | `Vec<Vec<f64>>` | Gain columns, one per observation, before localisation. |
| `gain_post` | `Vec<Vec<f64>>` | Gain columns after localisation. |
| `innovation_covariance` | `super::jama::Rows` | `D = PRED·PREDᵀ/(q−1) + R`. |
| `predicted_mean` | `Vec<f64>` | The forecast mean of the predictions. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `compute_gain`

`computeGainMatrix`: the gain columns `K_i = Σ_j E(j,i)·X_j`, with
`E = PREDᵀ·inv(D)`, the state deviations scaled by `1/√(q−1)` and
back by `√(q−1)` **in place**, as OpenDA does.

# Errors
Shapes, or the SVD inverse's.

```rust
pub fn compute_gain(obs_std: &[f64], preds: &EnsembleVectors, states: &mut EnsembleVectors) -> crate::Result<(Vec<Vec<f64>>, super::jama::Rows)> { /* ... */ }
```

#### Function `zhang_weights`

Zhang & Oliver's weights (`AutoLocalizationZhang2011`), ported as
written: each bootstrap member is the drawn deviation **added to
itself**, and the bootstrap ensemble is built with
[`EnsembleVectors::as_openda_constructor`].

# Errors
From [`compute_gain`], the draws, or a negative C² (OpenDA throws).

```rust
pub fn zhang_weights(obs_std: &[f64], preds: &EnsembleVectors, states: &EnsembleVectors, n_bootstrap: usize, sigma_alpha2: f64, draws: &mut super::noise::DrawSource) -> crate::Result<Vec<Vec<f64>>> { /* ... */ }
```

#### Function `enkf_update`

OpenDA's EnKF analysis on vectors: `states` and `preds` are the
members' forecast states and predictions of `obs`. Gain, localisation,
then each member's increment from perturbed observations (one draw per
observation per member, in member order).

# Errors
Shapes, the inverse, or the draws.

```rust
pub fn enkf_update(states: Vec<Vec<f64>>, preds: Vec<Vec<f64>>, obs: &super::models::Observations, localization: &LocalizationInput, draws: &mut super::noise::DrawSource) -> crate::Result<EnkfUpdate> { /* ... */ }
```

#### Function `move_main_to_mean`

`updateMainModel` (also EnSR's and the particle filter's): `main +=
1.0·(mean − main)`, the mean of the members' current states.

```rust
pub fn move_main_to_mean<M: OdaStochModel>(members: &[M], main: &mut M) { /* ... */ }
```

#### Function `enkf_analysis`

OpenDA's `EnKF.analysis` on model members: gathers states and
predictions, runs [`enkf_update`] (Hamill's mask from member 0),
applies the increments, and moves `main` to the analysis mean.

# Errors
From the models or [`enkf_update`].

```rust
pub fn enkf_analysis<M: OdaStochModel>(members: &mut [M], main: &mut M, obs: &super::models::Observations, localization: Localization, draws: &mut super::noise::DrawSource) -> crate::Result<EnkfUpdate> { /* ... */ }
```

## Module `ensr`

**OpenDA's ensemble square-root filter, particle filter and steady-state
filter** (gh:#1192).

```rust
pub mod ensr { /* ... */ }
```

### Types

#### Struct `Resampling`

The particle filter's normalised weights and residual-resampling index
(`index[i]` is the member member `i` becomes a copy of).

```rust
pub struct Resampling {
    pub weights: Vec<f64>,
    pub index: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `weights` | `Vec<f64>` | Normalised weights. |
| `index` | `Vec<usize>` | Parent of each new member. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SteadyStateGain`

A stored gain for the steady-state filter.

```rust
pub struct SteadyStateGain {
    pub columns: Vec<(f64, Vec<f64>)>,
    pub skip_factor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `columns` | `Vec<(f64, Vec<f64>)>` | `(observation index, gain column)`. |
| `skip_factor` | `f64` | `skipAssimilationStandardDeviationFactor` (OpenDA: +∞, none skipped). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `ensr_update`

EnSR (Sakov 2006, symmetric transform) on vectors: OpenDA's increment
`xChange = x̄_a − x_i + √(q−1)·(L_f·T)_i` for each member.

# Errors
Shapes, or the SVDs'.

```rust
pub fn ensr_update(states: &[Vec<f64>], preds: &[Vec<f64>], obs: &super::models::Observations) -> crate::Result<Vec<Vec<f64>>> { /* ... */ }
```

#### Function `ensr_analysis`

OpenDA's `EnSR.analysis` on model members.

# Errors
From the models or [`ensr_update`].

```rust
pub fn ensr_analysis<M: OdaStochModel>(members: &mut [M], main: &mut M, obs: &super::models::Observations) -> crate::Result<()> { /* ... */ }
```

#### Function `residual_resampling`

Residual resampling (`ParticleFilter.residualResampling`): `⌊N w_i⌋`
copies each, the rest drawn from the residual weights.

# Errors
The draws'.

```rust
pub fn residual_resampling(weights: &[f64], draws: &mut super::noise::DrawSource) -> crate::Result<Vec<usize>> { /* ... */ }
```

#### Function `particle_filter_update`

The particle filter's weights (`evaluatePDF` of each prediction,
normalised by their sum) and resampling.

# Errors
Shapes or the draws'.

```rust
pub fn particle_filter_update(preds: &[Vec<f64>], obs: &super::models::Observations, draws: &mut super::noise::DrawSource) -> crate::Result<Resampling> { /* ... */ }
```

#### Function `particle_filter_analysis`

OpenDA's `ParticleFilter.analysis` on model members: members are
replaced by copies of their parents (`restoreInternalState`).

# Errors
From the models or [`particle_filter_update`].

```rust
pub fn particle_filter_analysis<M: OdaStochModel>(members: &mut [M], main: &mut M, obs: &super::models::Observations, draws: &mut super::noise::DrawSource) -> crate::Result<Resampling> { /* ... */ }
```

#### Function `steady_state_analysis`

`SteadyStateFilter.analysis`: `main += Σ_i (y_i − h_i(main))·K_i`,
skipping observations with `|innovation| > skip·σ`.

# Errors
An observation without a gain column (OpenDA throws), or the model's.

```rust
pub fn steady_state_analysis<M: OdaStochModel>(gain: &SteadyStateGain, main: &mut M, obs: &super::models::Observations) -> crate::Result<()> { /* ... */ }
```

## Module `jama`

**JAMA's singular value decomposition**, in JAMA's rounding order
(gh:#1192). Matrices are `Vec<Vec<f64>>`, Java's `double[][]`.

```rust
pub mod jama { /* ... */ }
```

### Types

#### Type Alias `Rows`

A Java `double[][]`.

```rust
pub type Rows = Vec<Vec<f64>>;
```

#### Struct `JamaSvd`

The decomposition `A = U·diag(s)·Vᵀ` of an `m × n` matrix, `m ≥ n`.

```rust
pub struct JamaSvd {
    pub u: Rows,
    pub s: Vec<f64>,
    pub v: Rows,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `u` | `Rows` | `m × n` left singular vectors (JAMA's `getU()` for `m ≥ n`). |
| `s` | `Vec<f64>` | The `n` singular values, decreasing. |
| `v` | `Rows` | `n × n` right singular vectors. |

##### Implementations

###### Methods

- ```rust
  pub fn s_matrix(self: &Self) -> Rows { /* ... */ }
  ```
  JAMA's `getS()`: the `n × n` diagonal matrix of `s`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `hypot`

**Attributes:**

- `MustUse { reason: None }`

JAMA's `Maths.hypot`: `sqrt(a² + b²)` without under/overflow.

```rust
pub fn hypot(a: f64, b: f64) -> f64 { /* ... */ }
```

#### Function `svd`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"too_many_lines\" },\nkind: Allow},\nLintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"many_single_char_names\" },\nkind: Allow},\nLintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"needless_range_loop\" },\nkind: Allow}])]")`

JAMA's `SingularValueDecomposition(A)`.

# Errors
[`RafflesError::DimensionMismatch`] for `m < n`, an empty or ragged `a`.

```rust
pub fn svd(arg: &[Vec<f64>]) -> crate::Result<JamaSvd> { /* ... */ }
```

#### Function `times`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"needless_range_loop\" },\nkind: Allow}])]")`

JAMA's `Matrix.times`: column of `B` copied, then one chain per element.

# Errors
[`RafflesError::DimensionMismatch`] if the inner dimensions differ.

```rust
pub fn times(a: &[Vec<f64>], b: &[Vec<f64>]) -> crate::Result<Rows> { /* ... */ }
```

#### Function `transpose`

**Attributes:**

- `MustUse { reason: None }`

JAMA's `Matrix.transpose` (exact).

```rust
pub fn transpose(a: &[Vec<f64>]) -> Rows { /* ... */ }
```

## Module `linalg`

**OpenDA's `Matrix` and `Vector` kernels**, in Java's rounding order
(gh:#1192).

```rust
pub mod linalg { /* ... */ }
```

### Functions

#### Function `zeros`

**Attributes:**

- `MustUse { reason: None }`

`m × n` zeros.

```rust
pub fn zeros(m: usize, n: usize) -> super::jama::Rows { /* ... */ }
```

#### Function `eye`

**Attributes:**

- `MustUse { reason: None }`

`Matrix.eye(n)`.

```rust
pub fn eye(n: usize) -> super::jama::Rows { /* ... */ }
```

#### Function `diag_matrix`

**Attributes:**

- `MustUse { reason: None }`

`Matrix.diag(IVector)` / `Matrix(double[])`: the diagonal matrix of `v`.

```rust
pub fn diag_matrix(v: &[f64]) -> super::jama::Rows { /* ... */ }
```

#### Function `diag`

**Attributes:**

- `MustUse { reason: None }`

`Matrix.diag()`: the diagonal of `a`.

```rust
pub fn diag(a: &[Vec<f64>]) -> Vec<f64> { /* ... */ }
```

#### Function `from_columns`

**Attributes:**

- `MustUse { reason: None }`

`new Matrix(IVector[])`: each vector is a **column**.

```rust
pub fn from_columns(v: &[Vec<f64>]) -> super::jama::Rows { /* ... */ }
```

#### Function `columns`

**Attributes:**

- `MustUse { reason: None }`

`Matrix.asVectorArray()`: the columns.

```rust
pub fn columns(a: &[Vec<f64>]) -> Vec<Vec<f64>> { /* ... */ }
```

#### Function `scale_matrix`

`Matrix.scale(f)`: `a(i,j) = a(i,j)·f`.

```rust
pub fn scale_matrix(a: &mut [Vec<f64>], f: f64) { /* ... */ }
```

#### Function `concat_vertical`

**Attributes:**

- `MustUse { reason: None }`

`Matrix.concatVertical(a, b)`.

```rust
pub fn concat_vertical(a: &[Vec<f64>], b: &[Vec<f64>]) -> super::jama::Rows { /* ... */ }
```

#### Function `multiply`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"many_single_char_names\" },\nkind: Allow}])]")`

`Matrix.multiply(facAB, A, B, facThis, transa, transb)`: `C = facThis·C +
facAB·op(A)·op(B)`, element by element: `C *= facThis`, one sum chain,
then `C += facAB·sum`.

# Errors
[`RafflesError::DimensionMismatch`] on incompatible shapes.

```rust
pub fn multiply(c: &mut [Vec<f64>], fac_ab: f64, a: &[Vec<f64>], b: &[Vec<f64>], fac_c: f64, transa: bool, transb: bool) -> crate::Result<()> { /* ... */ }
```

#### Function `mult`

`Matrix.mult(A, B, ta, tb)`: a zero result, then [`multiply`] with factors 1.

# Errors
As [`multiply`].

```rust
pub fn mult(a: &[Vec<f64>], b: &[Vec<f64>], ta: bool, tb: bool) -> crate::Result<super::jama::Rows> { /* ... */ }
```

#### Function `right_multiply`

`Matrix.rightMultiply(beta, v, alpha, x)`: `x = alpha·x + beta·L·v`, in
OpenDA's order `temp = alpha·x(i); temp += (beta·L(i,j))·v(j)`. Like
OpenDA, nothing is done on a shape mismatch.

```rust
pub fn right_multiply(l: &[Vec<f64>], beta: f64, v: &[f64], alpha: f64, x: &mut [f64]) { /* ... */ }
```

#### Function `inverse`

`Matrix.inverse()`: JAMA SVD, `V·diag(1/s)·Uᵀ` with `|s| ≤ 1e-7` zeroed.

# Errors
From the SVD (an empty or wide matrix).

```rust
pub fn inverse(a: &[Vec<f64>]) -> crate::Result<super::jama::Rows> { /* ... */ }
```

#### Function `sqrt`

`Matrix.sqrt()`: `U·diag(√s)·Vᵀ` with `|s| < 1e-7` zeroed.

# Errors
From the SVD, or a non-square matrix.

```rust
pub fn sqrt(a: &[Vec<f64>]) -> crate::Result<super::jama::Rows> { /* ... */ }
```

#### Function `svd`

`Matrix.svd()`: `[U, S, V]`, a wide matrix transposed first (and `U`,
`V` swapped back).

# Errors
From the SVD.

```rust
pub fn svd(a: &[Vec<f64>]) -> crate::Result<(super::jama::Rows, super::jama::Rows, super::jama::Rows)> { /* ... */ }
```

#### Function `right_solve`

`Matrix.rightSolve(v, x)`: the least-squares `x` of `L·x = v`, as OpenDA
computes it: `xᵀ = vᵀ·U·diag(1/s)·Vᵀ` (1e-7 cut-off), through
`Jama.Matrix.times`.

# Errors
From the SVD (OpenDA throws for `m < n` too), or `v` of the wrong length.

```rust
pub fn right_solve(l: &[Vec<f64>], v: &[f64]) -> crate::Result<Vec<f64>> { /* ... */ }
```

#### Function `axpy`

`VectorDouble.axpy(alpha, x)`: `y(i) = y(i) + alpha·x(i)`.

```rust
pub fn axpy(y: &mut [f64], alpha: f64, x: &[f64]) { /* ... */ }
```

#### Function `scale`

`VectorDouble.scale(alpha)`: `y(i) = alpha·y(i)`.

```rust
pub fn scale(y: &mut [f64], alpha: f64) { /* ... */ }
```

#### Function `norm2`

**Attributes:**

- `MustUse { reason: None }`

`VectorDouble.norm2()`: `√(Σ y(i)²)`, one chain.

```rust
pub fn norm2(y: &[f64]) -> f64 { /* ... */ }
```

#### Function `norm2_squared`

**Attributes:**

- `MustUse { reason: None }`

`Math.pow(norm2(y), 2.0)`, which OpenDA's costs use and which equals
`n·n` on every sample measured (gh:#1192 record, finding 3).

```rust
pub fn norm2_squared(y: &[f64]) -> f64 { /* ... */ }
```

#### Function `pointwise_multiply`

`VectorDouble.pointwiseMultiply`.

```rust
pub fn pointwise_multiply(y: &mut [f64], x: &[f64]) { /* ... */ }
```

#### Function `pointwise_divide`

`VectorDouble.pointwiseDivide`.

```rust
pub fn pointwise_divide(y: &mut [f64], x: &[f64]) { /* ... */ }
```

#### Function `ensemble_average`

**Attributes:**

- `MustUse { reason: None }`

`ensembleAverage`: member 0, plus each later member (`axpy(1.0, ·)`),
then `scale(1.0/n)`.

```rust
pub fn ensemble_average(v: &[Vec<f64>]) -> Vec<f64> { /* ... */ }
```

#### Function `remove_average`

`removeAverage`: `axpy(-1.0, mean)` on each member.

```rust
pub fn remove_average(v: &mut [Vec<f64>], mean: &[f64]) { /* ... */ }
```

## Module `models`

**OpenDA's test models** (oscillator, Lorenz-63), the observer, and the
model seam the sequential algorithms consume (gh:#1192).

```rust
pub mod models { /* ... */ }
```

### Types

#### Struct `Observations`

Observations in OpenDA's `CsvStochObserver` column form, independent
Gaussian errors.

```rust
pub struct Observations {
    pub time: Vec<f64>,
    pub index: Vec<f64>,
    pub value: Vec<f64>,
    pub std: Vec<f64>,
    pub transform: Option<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time` | `Vec<f64>` | Observation time, model time units. |
| `index` | `Vec<f64>` | State index observed (OpenDA's `index` column). |
| `value` | `Vec<f64>` | Observed value. |
| `std` | `Vec<f64>` | Standard deviation. |
| `transform` | `Option<Vec<f64>>` | OpenDA's `transform` column: `2` observes the square of the state. |

##### Implementations

###### Methods

- ```rust
  pub fn new(time: Vec<f64>, index: Vec<f64>, value: Vec<f64>, std: Vec<f64>, transform: Option<Vec<f64>>) -> Result<Self> { /* ... */ }
  ```
  From columns.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of observations.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  True if none.

- ```rust
  pub fn select_span(self: &Self, begin: f64, end: f64) -> Self { /* ... */ }
  ```
  `createSelection(span)`: the observations whose time stamp lies in

- ```rust
  pub fn times(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The distinct observation times, sorted (`getTimes`).

- ```rust
  pub fn uncertainty(self: &Self) -> Result<StochVector> { /* ... */ }
  ```
  The observation error as a `StochVector`.

- ```rust
  pub fn realization(self: &Self, draws: &mut DrawSource) -> Result<Vec<f64>> { /* ... */ }
  ```
  `getRealizations()`: one perturbed observation vector.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Dynamics`

The right-hand side of a [`SimpleStochModel`].

```rust
pub enum Dynamics {
    Oscillator {
        t_damp: f64,
        omega: f64,
    },
    Lorenz63 {
        sigma: f64,
        rho: f64,
        beta: f64,
    },
}
```

##### Variants

###### `Oscillator`

`dx/dt = u`, `du/dt = −ω²x − (2/t_damp)u`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_damp` | `f64` | Damping time scale \[s\]. |
| `omega` | `f64` | Angular frequency \[rad/s\]. |

###### `Lorenz63`

Lorenz (1963).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `sigma` | `f64` | σ. |
| `rho` | `f64` | ρ. |
| `beta` | `f64` | β. |

##### Implementations

###### Methods

- ```rust
  pub fn oscillator() -> Self { /* ... */ }
  ```
  OpenDA's oscillator defaults: t_damp 8, ω = 0.25·2π.

- ```rust
  pub fn lorenz63() -> Self { /* ... */ }
  ```
  OpenDA's Lorenz defaults: σ 10, ρ 28, β 8/3.

- ```rust
  pub fn parameters(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The uncertain parameters, in OpenDA's `stochParNames` order.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SimpleModelConfig`

The XML configuration of OpenDA's simple models, as a struct: `None`
keeps the model's default (OpenDA's `Initialize`).

```rust
pub struct SimpleModelConfig {
    pub timespan: Option<[f64; 3]>,
    pub parameters: Option<Vec<f64>>,
    pub parameter_std: Option<Vec<f64>>,
    pub system_noise: Option<(Vec<f64>, Vec<f64>)>,
    pub initial_state: Option<Vec<f64>>,
    pub initial_state_std: Option<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `timespan` | `Option<[f64; 3]>` | `simulationTimespan`: `[t_start, t_step, t_stop]`. |
| `parameters` | `Option<Vec<f64>>` | `parameters` (all of them, in `stochParNames` order). |
| `parameter_std` | `Option<Vec<f64>>` | `parameterUncertainty`: standard deviations (the mean is the<br>parameters' values at that point). |
| `system_noise` | `Option<(Vec<f64>, Vec<f64>)>` | `systemNoise`: `(mean, std)`. |
| `initial_state` | `Option<Vec<f64>>` | `initialState`. |
| `initial_state_std` | `Option<Vec<f64>>` | `initialStateUncertainty` (only read with `initial_state`, as OpenDA). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SimpleStochModel`

OpenDA's `SimpleStochModelInstance`: RK4 with step `t_step`, optional
system noise `√t_step·w` after each step, and stored states for
observations.

```rust
pub struct SimpleStochModel {
    pub dynamics: Dynamics,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dynamics` | `Dynamics` | The right-hand side and its parameters. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(dynamics: Dynamics, config: &SimpleModelConfig) -> Result<Self> { /* ... */ }
  ```
  A model with `dynamics`' defaults overridden by `config`.

- ```rust
  pub fn with_localization_mask(self: Self, mask: Vec<f64>) -> Self { /* ... */ }
  ```
  Replace Hamill's mask (all ones by default) by the same `mask` for

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **OdaStochModel**
  - ```rust
    fn state(self: &Self) -> Vec<f64> { /* ... */ }
    ```

  - ```rust
    fn axpy_on_state(self: &mut Self, alpha: f64, v: &[f64]) { /* ... */ }
    ```

  - ```rust
    fn current_time(self: &Self) -> f64 { /* ... */ }
    ```

  - ```rust
    fn time_horizon(self: &Self) -> (f64, f64) { /* ... */ }
    ```

  - ```rust
    fn compute(self: &mut Self, target: f64, draws: &mut DrawSource) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn announce_observed_values(self: &mut Self, _obs: &Observations) { /* ... */ }
    ```

  - ```rust
    fn observed_values(self: &Self, obs: &Observations) -> Result<Vec<f64>> { /* ... */ }
    ```

  - ```rust
    fn state_uncertainty(self: &Self) -> Option<StochVector> { /* ... */ }
    ```

  - ```rust
    fn parameter_uncertainty(self: &Self) -> Option<StochVector> { /* ... */ }
    ```

  - ```rust
    fn set_parameters(self: &mut Self, p: &[f64]) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn set_automatic_noise(self: &mut Self, on: bool) { /* ... */ }
    ```

  - ```rust
    fn observed_localization(self: &Self, obs: &Observations) -> Vec<Vec<f64>> { /* ... */ }
    ```

  - ```rust
    fn restore_from(self: &mut Self, saved: &Self) { /* ... */ }
    ```
    OpenDA restores state, time and step and **stops storing** (it clears

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `OdaStochModel`

The model seam OpenDA's sequential algorithms use (after
`IStochModelInstance`), consumed generically. RAFFLES implements it for
its test models ([`SimpleStochModel`]) and for a
[`crate::fidelity::StateSpaceModel`]; a downstream crate may implement it
for its own model.

```rust
pub trait OdaStochModel: Clone {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `state`: The state vector.
- `axpy_on_state`: `x = x + alpha·v`.
- `current_time`: Current model time.
- `time_horizon`: Start and end of the time horizon.
- `compute`: Advance to `target`, drawing system noise from `draws` when automatic
- `announce_observed_values`: Start storing states for `obs` (OpenDA's announceObservedValues).
- `observed_values`: The model's prediction of `obs`.
- `state_uncertainty`: The initial-state uncertainty, if any.
- `parameter_uncertainty`: The parameter uncertainty, if any.
- `set_parameters`: Set the uncertain parameters.
- `set_automatic_noise`: Switch automatic system noise on or off.

##### Provided Methods

- ```rust
  fn observed_localization(self: &Self, obs: &Observations) -> Vec<Vec<f64>> { /* ... */ }
  ```
  Hamill's localisation mask, one state-length vector per observation

- ```rust
  fn restore_from(self: &mut Self, saved: &Self) { /* ... */ }
  ```
  `restoreInternalState(saved)`: become a copy of `saved`'s state.

##### Implementations

This trait is implemented for the following types:

- `SimpleStochModel`

### Functions

#### Function `java_round`

**Attributes:**

- `MustUse { reason: None }`

Java's `Math.round(double)` for the values a step count takes:
`⌊x + ½⌋` evaluated exactly.

```rust
pub fn java_round(x: f64) -> i64 { /* ... */ }
```

### Constants and Statics

#### Constant `TIME_PRECISION`

OpenDA's time precision, one second in days: `1.0/24.0/60.0/60.0`.

```rust
pub const TIME_PRECISION: f64 = _;
```

## Module `noise`

**OpenDA's Gaussian uncertainty** (`StochVector`) and the source of its
random draws (gh:#1192).

```rust
pub mod noise { /* ... */ }
```

### Types

#### Enum `DrawSource`

Where OpenDA's random numbers come from. **Written here, not ported.**

```rust
pub enum DrawSource {
    Injected {
        gaussians: Vec<f64>,
        uniforms: Vec<f64>,
        ints: Vec<usize>,
        cursor: [usize; 3],
    },
    Lcg {
        seed: u64,
    },
}
```

##### Variants

###### `Injected`

Replay recorded upstream draws, each stream in order. A stream that
runs out is an error (the port consumed more than OpenDA did).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `gaussians` | `Vec<f64>` | `nextGaussian()` of `StochVector`'s generator. |
| `uniforms` | `Vec<f64>` | `nextDouble()` of the particle filter's generator. |
| `ints` | `Vec<usize>` | `nextInt(n)` of the Zhang localisation's generator. |
| `cursor` | `[usize; 3]` | Next unread index of each stream. |

###### `Lcg`

The workspace LCG at this state: Box–Muller normals, `prn` uniforms,
`⌊n·u⌋` integers.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `seed` | `u64` | LCG state. |

##### Implementations

###### Methods

- ```rust
  pub fn injected(gaussians: Vec<f64>, uniforms: Vec<f64>, ints: Vec<usize>) -> Self { /* ... */ }
  ```
  Replay recorded draws.

- ```rust
  pub fn lcg(seed: u64) -> Self { /* ... */ }
  ```
  The workspace LCG.

- ```rust
  pub fn gaussian(self: &mut Self) -> Result<f64> { /* ... */ }
  ```
  A standard normal.

- ```rust
  pub fn uniform(self: &mut Self) -> Result<f64> { /* ... */ }
  ```
  A uniform on `[0, 1)`.

- ```rust
  pub fn int_below(self: &mut Self, n: usize) -> Result<usize> { /* ... */ }
  ```
  An integer in `0..n`.

- ```rust
  pub fn consumed(self: &Self) -> Option<[usize; 3]> { /* ... */ }
  ```
  How many draws of each stream were consumed (injected only).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `StochVector`

OpenDA's `StochVector`: a Gaussian vector, independent (mean and
standard deviations) or correlated (mean and a square root `L` of the
covariance, `P = L·Lᵀ`).

```rust
pub struct StochVector {
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
    pub correlated: Option<(super::jama::Rows, super::jama::Rows)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mean` | `Vec<f64>` | Mean. |
| `std` | `Vec<f64>` | Standard deviations (√ of the covariance's diagonal when correlated). |
| `correlated` | `Option<(super::jama::Rows, super::jama::Rows)>` | `Some((P, L))` when correlated. |

##### Implementations

###### Methods

- ```rust
  pub fn independent(mean: Vec<f64>, std: Vec<f64>) -> Result<Self> { /* ... */ }
  ```
  Independent elements (`StochVector(mean, std)`).

- ```rust
  pub fn correlated(mean: Vec<f64>, covariance: Rows, is_square_root: bool) -> Result<Self> { /* ... */ }
  ```
  Correlated (`StochVector(mean, covariance, isSquareRoot)`): from `P`

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of elements.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  True for a zero-length vector.

- ```rust
  pub fn realization(self: &Self, draws: &mut DrawSource) -> Result<Vec<f64>> { /* ... */ }
  ```
  `createRealization()`: one standard normal per element, in order;

- ```rust
  pub fn evaluate_pdf(self: &Self, tv: &[f64]) -> Result<f64> { /* ... */ }
  ```
  `evaluatePdf(tv)`, **as OpenDA computes it**. Correlated: the

- ```rust
  pub fn sqrt_solve(self: &Self, v: &[f64]) -> Result<Vec<f64>> { /* ... */ }
  ```
  `getSqrtCovariance().rightSolve(v, x)` (= `leftSolve` here): `v/σ`

- ```rust
  pub fn sqrt_matrix(self: &Self) -> Rows { /* ... */ }
  ```
  `getSqrtCovariance().asMatrix()`: `diag(σ)` or `L`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `powell`

**OpenDA's Powell minimiser** with its bracketing and Brent line search
(gh:#1192).

```rust
pub mod powell { /* ... */ }
```

### Types

#### Struct `Powell`

OpenDA's `PowellCoreOptimizer`.

```rust
pub struct Powell {
    pub max_iterations: usize,
    pub max_brent: usize,
    pub max_bracket: usize,
    pub rel_tol: f64,
    pub abs_tol: f64,
    pub rel_tol_brent: f64,
    pub abs_tol_brent: f64,
    pub bracket_first_try: f64,
    pub limit: f64,
    pub tiny: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iterations` | `usize` | `maxitPowell` (200). |
| `max_brent` | `usize` | `maxitBrent` (200). |
| `max_bracket` | `usize` | `maxitBracket` (20). |
| `rel_tol` | `f64` | `relTolPowell` (0.01). |
| `abs_tol` | `f64` | `absTolPowell` (0.01). |
| `rel_tol_brent` | `f64` | `relTolBrent` (0.01). |
| `abs_tol_brent` | `f64` | `absTolBrent` (0.001). |
| `bracket_first_try` | `f64` | `bracketFirstTry` (1.0). |
| `limit` | `f64` | `limit` (100). |
| `tiny` | `f64` | `tiny` (1e-15). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn initialize<C: CostFunction>(self: &mut Self, f: &mut C, p_init: &[f64], width: f64) -> Result<()> { /* ... */ }
  ```
  `initialize(pInit, width)`: directions `width·e_{n−1}`, …,

- ```rust
  pub fn initialize_with_directions<C: CostFunction>(self: &mut Self, f: &mut C, p_init: &[f64], directions: Vec<Vec<f64>>) -> Result<()> { /* ... */ }
  ```
  `initialize(pInit, searchInit)`.

- ```rust
  pub fn optimum(self: &Self) -> (&[f64], f64) { /* ... */ }
  ```
  The best point and cost the optimiser has accepted.

- ```rust
  pub fn directions(self: &Self) -> &[Vec<f64>] { /* ... */ }
  ```
  The current search directions.

- ```rust
  pub fn iterations(self: &Self) -> usize { /* ... */ }
  ```
  Outer iterations done.

- ```rust
  pub fn optimize<C: CostFunction>(self: &mut Self, f: &mut C) -> Result<()> { /* ... */ }
  ```
  `optimize()`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `sequential`

**OpenDA's sequential assimilation loop** and the filter enum (gh:#1192).

```rust
pub mod sequential { /* ... */ }
```

### Types

#### Struct `EnsembleOptions`

OpenDA's `ensembleModel@stochInit/stochParameter/stochForcing`.

```rust
pub struct EnsembleOptions {
    pub stoch_init: bool,
    pub stoch_parameter: bool,
    pub stoch_forcing: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stoch_init` | `bool` | Add a realisation of the state uncertainty to each member. |
| `stoch_parameter` | `bool` | Draw each member's parameters from their uncertainty. |
| `stoch_forcing` | `bool` | Members add system noise while computing. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    OpenDA's defaults: init and forcing on, parameters off.

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SequentialFilter`

The sequential filters RAFFLES ports from OpenDA.

```rust
pub enum SequentialFilter {
    EnKf {
        ensemble_size: usize,
        localization: super::enkf::Localization,
        options: EnsembleOptions,
    },
    EnSr {
        ensemble_size: usize,
        options: EnsembleOptions,
    },
    ParticleFilter {
        ensemble_size: usize,
        options: EnsembleOptions,
    },
    SteadyState(super::ensr::SteadyStateGain),
}
```

##### Variants

###### `EnKf`

`EnKF`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ensemble_size` | `usize` | Members. |
| `localization` | `super::enkf::Localization` | Localisation option. |
| `options` | `EnsembleOptions` | Ensemble generation. |

###### `EnSr`

`EnSR`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ensemble_size` | `usize` | Members. |
| `options` | `EnsembleOptions` | Ensemble generation. |

###### `ParticleFilter`

`ParticleFilter` (residual resampling).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ensemble_size` | `usize` | Particles. |
| `options` | `EnsembleOptions` | Ensemble generation. |

###### `SteadyState`

`SteadyStateFilter`: a stored gain on the main model, no ensemble.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::ensr::SteadyStateGain` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SequentialStep`

One analysis time.

```rust
pub struct SequentialStep {
    pub time: f64,
    pub observations: super::models::Observations,
    pub forecast: Vec<Vec<f64>>,
    pub analysis: Vec<Vec<f64>>,
    pub main_after: Vec<f64>,
    pub gain: Option<(Vec<Vec<f64>>, Vec<Vec<f64>>)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time` | `f64` | The analysis time. |
| `observations` | `super::models::Observations` | The observations assimilated. |
| `forecast` | `Vec<Vec<f64>>` | Member states before the analysis (the main model for the<br>steady-state filter). |
| `analysis` | `Vec<Vec<f64>>` | Member states after it. |
| `main_after` | `Vec<f64>` | The main model's state after it. |
| `gain` | `Option<(Vec<Vec<f64>>, Vec<Vec<f64>>)>` | EnKF only: gain columns before and after localisation. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SequentialRun`

A whole run.

```rust
pub struct SequentialRun<M> {
    pub steps: Vec<SequentialStep>,
    pub main: M,
    pub members: Vec<M>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `steps` | `Vec<SequentialStep>` | Each analysis. |
| `main` | `M` | The main model at the end (OpenDA's current state after the last<br>analysis and the final forecast). |
| `members` | `Vec<M>` | The members at the end. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `run_sequential`

Run `filter` on `template` (a fresh model) against `obs`: OpenDA's
`setStochComponents`, then `next()` at each distinct observation time
inside the model's horizon, then the final forecast.

# Errors
From the models, the analyses or the draws.

```rust
pub fn run_sequential<M: OdaStochModel>(template: &M, obs: &super::models::Observations, filter: &SequentialFilter, draws: &mut super::noise::DrawSource) -> crate::Result<SequentialRun<M>> { /* ... */ }
```

## Module `simplex`

**OpenDA's Nelder–Mead simplex** (gh:#1192).

```rust
pub mod simplex { /* ... */ }
```

### Types

#### Struct `Simplex`

OpenDA's `SimplexCoreOptimizer`.

```rust
pub struct Simplex {
    pub max_iterations: usize,
    pub abs_tol: f64,
    pub rel_tol: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iterations` | `usize` | `maxitSimplex` (100). |
| `abs_tol` | `f64` | `absTolSimplex` (0.01). |
| `rel_tol` | `f64` | `relTolSimplex` (0.01). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn initialize<C: CostFunction>(self: &mut Self, f: &mut C, p_init: &[f64], width: f64) -> Result<()> { /* ... */ }
  ```
  `initialize(pInit, width)`: `pInit` and `pInit + width·e_i`.

- ```rust
  pub fn current(self: &Self) -> (&[Vec<f64>], &[f64]) { /* ... */ }
  ```
  The vertices and their costs.

- ```rust
  pub fn iterations(self: &Self) -> usize { /* ... */ }
  ```
  Outer iterations done.

- ```rust
  pub fn std_values(self: &Self) -> Option<&[f64]> { /* ... */ }
  ```
  The least-squares standard deviations OpenDA estimates from the

- ```rust
  pub fn optimize<C: CostFunction>(self: &mut Self, f: &mut C) -> Result<()> { /* ... */ }
  ```
  `optimize()`: iterate until a stopping test holds.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `sorted_index`

**Attributes:**

- `MustUse { reason: None }`

Indices that sort `v` ascending, stably (`Collections.sort` with
`Double.compare`).

```rust
pub fn sorted_index(v: &[f64]) -> Vec<usize> { /* ... */ }
```

### Re-exports

#### Re-export `simple_least_squares`

```rust
pub use cost::simple_least_squares;
```

#### Re-export `CostFunction`

```rust
pub use cost::CostFunction;
```

#### Re-export `CostHistory`

```rust
pub use cost::CostHistory;
```

#### Re-export `IdentityPredictor`

```rust
pub use cost::IdentityPredictor;
```

#### Re-export `LeastSquaresView`

```rust
pub use cost::LeastSquaresView;
```

#### Re-export `Predictor`

```rust
pub use cost::Predictor;
```

#### Re-export `RosenbrockCost`

```rust
pub use cost::RosenbrockCost;
```

#### Re-export `SimpleCost`

```rust
pub use cost::SimpleCost;
```

#### Re-export `SimulationPredictor`

```rust
pub use cost::SimulationPredictor;
```

#### Re-export `WeightedLeastSquares`

```rust
pub use cost::WeightedLeastSquares;
```

#### Re-export `Dud`

```rust
pub use dud::Dud;
```

#### Re-export `compute_gain`

```rust
pub use enkf::compute_gain;
```

#### Re-export `enkf_analysis`

```rust
pub use enkf::enkf_analysis;
```

#### Re-export `enkf_update`

```rust
pub use enkf::enkf_update;
```

#### Re-export `zhang_weights`

```rust
pub use enkf::zhang_weights;
```

#### Re-export `EnkfUpdate`

```rust
pub use enkf::EnkfUpdate;
```

#### Re-export `EnsembleVectors`

```rust
pub use enkf::EnsembleVectors;
```

#### Re-export `Localization`

```rust
pub use enkf::Localization;
```

#### Re-export `LocalizationInput`

```rust
pub use enkf::LocalizationInput;
```

#### Re-export `ensr_analysis`

```rust
pub use ensr::ensr_analysis;
```

#### Re-export `ensr_update`

```rust
pub use ensr::ensr_update;
```

#### Re-export `particle_filter_analysis`

```rust
pub use ensr::particle_filter_analysis;
```

#### Re-export `particle_filter_update`

```rust
pub use ensr::particle_filter_update;
```

#### Re-export `residual_resampling`

```rust
pub use ensr::residual_resampling;
```

#### Re-export `steady_state_analysis`

```rust
pub use ensr::steady_state_analysis;
```

#### Re-export `Resampling`

```rust
pub use ensr::Resampling;
```

#### Re-export `SteadyStateGain`

```rust
pub use ensr::SteadyStateGain;
```

#### Re-export `Dynamics`

```rust
pub use models::Dynamics;
```

#### Re-export `Observations`

```rust
pub use models::Observations;
```

#### Re-export `OdaStochModel`

```rust
pub use models::OdaStochModel;
```

#### Re-export `SimpleModelConfig`

```rust
pub use models::SimpleModelConfig;
```

#### Re-export `SimpleStochModel`

```rust
pub use models::SimpleStochModel;
```

#### Re-export `DrawSource`

```rust
pub use noise::DrawSource;
```

#### Re-export `StochVector`

```rust
pub use noise::StochVector;
```

#### Re-export `Powell`

```rust
pub use powell::Powell;
```

#### Re-export `run_sequential`

```rust
pub use sequential::run_sequential;
```

#### Re-export `EnsembleOptions`

```rust
pub use sequential::EnsembleOptions;
```

#### Re-export `SequentialFilter`

```rust
pub use sequential::SequentialFilter;
```

#### Re-export `SequentialRun`

```rust
pub use sequential::SequentialRun;
```

#### Re-export `SequentialStep`

```rust
pub use sequential::SequentialStep;
```

#### Re-export `Simplex`

```rust
pub use simplex::Simplex;
```

## Module `samplers`

Samplers — strategies that turn a dimension count into a set of design
points in the unit hypercube.

A *sampler* answers one question: **which points in input space should the
caller evaluate their model at?** It does not evaluate the model, does not
know what the inputs physically mean, and — by the deliberate design choice
below — does not know their probability distributions either.

# Output contract — unit uniforms, not distribution draws

**Every sampler here returns numbers in `[0, 1)`.** A design is a
`Vec<Vec<f64>>` of shape *(samples x dimensions)*: `design[i][j]` is the
coordinate of sample `i` along dimension `j`, and it is a **cumulative
probability**, not a physical value.

The caller maps each coordinate through the inverse CDF of whatever
distribution that dimension carries:

```text
let design = sampler.generate(master_seed);          // uniforms in [0, 1)
for row in &design {
    let temperature = temperature_dist.sample(row[0]); // K
    let power       = power_dist.sample(row[1]);       // W
    // ... evaluate the caller's own model here ...
}
```

This is the single most important design decision in the module, and it is
deliberate on three counts:

1. **Correctness is preserved.** Inverse-CDF (probability-integral)
   transformation of a uniform gives an exact draw from the target
   distribution, and it is monotone — so a Latin hypercube's stratification
   and a grid's tensor structure survive the mapping unchanged. RAVEN
   relies on the same identity: its CDF-space grids are recast through
   `ppf` at the last moment.
2. **The two modules stay independent.** [`crate::samplers`] has no
   dependency on [`crate::distributions`], so a sampler can be verified on
   its own — the stratification property below is exact and needs no
   distribution at all.
3. **It is what the mathematics actually is.** Latin hypercube sampling and
   grid sampling are defined on the unit hypercube; the distribution is a
   change of variables applied afterwards.

The one thing this contract does *not* cover is a distribution whose
dimensions are correlated (RAVEN's multivariate normal with a PCA
transform). That needs a joint inverse transform and is out of scope here.

# What is in this module

- [`MonteCarlo`] — independent uniform draws. RAVEN's
  `Samplers/MonteCarlo.py`.
- [`LatinHypercube`] — one draw per equiprobable stratum per dimension,
  randomly paired across dimensions. **RAVEN calls this `Stratified`**
  (`Samplers/Stratified.py`); the name difference is worth remembering when
  reading upstream.
- [`GridSampler`] — full-factorial sampling on a tensor product of
  per-dimension CDF levels. RAVEN's `Samplers/Grid.py`.
- [`stream_seed`] — derives independent generator streams from one master
  seed, for callers running replicates or parallel workers.

[`Sampler`] is the enum that dispatches between them. There is no
`Box<dyn Sampler>`: the set of strategies is closed and known at compile
time, so adding a variant is a compile error at every `match` that forgot
it. [`SamplingDesign`] is a compiler-enforced contract on the concrete
structs, never a dispatch mechanism.

# A note on Sobol

Three different things share the name and none of them is in this module:

- RAVEN's `Samplers/Sobol.py` is a **sparse-grid (HDMR) decomposition** used
  to build a surrogate. It is not a sampling design in the sense used here.
- The **Sobol sensitivity indices** are a variance decomposition computed
  from an existing sample set — [`crate::sensitivity`].
- The **Sobol low-discrepancy sequence** is a quasi-Monte-Carlo point set.
  RAFFLES has no such sequence, and RAVEN does not contain one either.

# Not implemented

- **Per-point probability weights.** RAVEN carries a weight per design
  point (`ProbabilityWeight-<var>`) so that downstream statistics can be
  computed on a non-equiprobable design. Those weights are analytically
  simple here (`1/n` per Monte Carlo point, `1/n` per Latin hypercube
  stratum, and the cell probability for a grid), but they are not produced
  yet because [`crate::sensitivity`] has no weighted estimator to consume
  them.
- **Value-space grids.** A grid specified in physical units rather than in
  CDF space needs the distribution's support, which this module does not
  see. Out of scope under the output contract above.
- **Correlated / multivariate designs**, factorial and response-surface
  designs, and every adaptive or model-in-the-loop sampler.

# The RNG — reused, not reinvented

**RAFFLES ships no generator of its own.** Sampling draws from
`petir::rng::lcg`, the workspace's port of OpenMC's 64-bit linear
congruential generator (`src/random_lcg.cpp`). ~~It is reached through
`outram_mc_libs::rng::lcg`.~~ **CORRECTED 2026-10-02**: the generator moved
from `outram-mc-libs` into `petir` (the workspace's `no_std` numerics
crate), unchanged bit for bit, and RAFFLES now depends on `petir` instead
of on the transport crate. That edge had to go: with `outram-mc-libs` taking
on `outram-park-fork-liggghts` for DEM pebble beds, it closed the cycle
`outram-mc-libs -> liggghts -> raffles -> outram-mc-libs`. Three reasons it is the right
choice here rather than a fresh PRNG or a new `rand` dependency:

- **One generator per workspace.** `docs/raven-port-scoping.md` (section 10,
  question 1) records that Outram Park has no `rand` crate; adding one is
  the maintainer's decision. Reusing the generator that already exists
  avoids both a new third-party dependency and a duplicate hand-rolled PRNG.
- **Jump-ahead gives genuinely independent streams.** `future_seed(n, seed)`
  advances the LCG `n` steps in `O(log n)`, so each sampled dimension can be
  given a starting seed a full stride away from its neighbours' — the
  streams provably do not overlap. That is OpenMC's reproducible-parallel
  Monte Carlo design, and it is what makes the dimensions of a design
  statistically independent. It is also already tested where it lives, in
  `petir::rng::lcg` (formerly `outram-mc-libs`).
- **Android-clean.** `petir` is `no_std` with no GPU dependency in its
  default build, so `cargo check -p raffles --all-targets --target
  aarch64-linux-android` stays clean. RAFFLES follows `outram-mc-libs`'
  gating convention if it ever needs something Android-hostile.

`petir::rng::lcg::init_seed` is deliberately **not** used — see
[`stream_seed`] for why, and for what this module does instead.

# Reproducibility

**Seeding is explicit and mandatory.** Every `generate` call takes a
`master_seed: i64`; there is no "seed from the clock" path, because an
unreproducible design is not a usable experiment. The same master seed and
the same sampler give a **bitwise-identical** design, on every platform,
forever: the LCG is wrapping integer arithmetic and the uniform conversion
is an exact scaling of a 52-bit integer.

Designs are **not** stream-compatible with RAVEN, and that is intentional.
Reproducing upstream's byte-for-byte sample dumps would require matching
NumPy's PCG64 stream *and* RAVEN's exact draw ordering; upstream's gold CSVs
are therefore explicitly written off as verification oracles (see
`docs/raven-port-scoping.md`, section 7). The verification below rests on
structural and statistical properties instead, which are stronger.

# Verification

See the `tests` module at the bottom of this file. Every test carries its
methodology and its measured result. In summary, measured 2026-08-06:

- Latin hypercube stratification holds exactly (one point per stratum per
  dimension) for every design checked.
- Grid point counts and coordinates match the tensor product exactly.
- All coordinates from all three samplers lie in `[0, 1)`.
- A fixed master seed reproduces a design bit-for-bit; different seeds
  differ.
- Per-dimension streams are uncorrelated.
- The Monte Carlo sample mean approaches `0.5` and the error decays as
  `N^{-1/2}`.

None of this is validation, and none of it has been through human review.

```rust
pub mod samplers { /* ... */ }
```

### Types

#### Struct `MonteCarlo`

Plain Monte Carlo: independent uniform draws in every dimension.

The design is `sample_count * dimensions` independent draws from `U[0, 1)`.
After the caller maps them through inverse CDFs, the rows are independent
draws from the joint input distribution (assuming independent inputs — see
the module doc on correlation).

The estimator error of any quantity computed from the design decays as
`N^{-1/2}` independently of `dimensions`, which is Monte Carlo's defining
property and the reason it survives in high dimension where a grid cannot.

Ported from RAVEN's `Samplers/MonteCarlo.py`. Upstream's `samplingType`
option (uniform sampling between the distribution's own bounds, weighted by
a CDF difference) is not ported: it needs the distribution's support, which
this module deliberately does not see.

# Example

```
use raffles::samplers::{MonteCarlo, SamplingDesign};

let mc = MonteCarlo::new(1000, 3).unwrap();
let design = mc.generate(42);
assert_eq!(design.len(), 1000);
assert_eq!(design[0].len(), 3);
```

```rust
pub struct MonteCarlo {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(samples: usize, dimensions: usize) -> Result<Self> { /* ... */ }
  ```
  Creates a Monte Carlo design of `samples` points over `dimensions`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **SamplingDesign**
  - ```rust
    fn dimensions(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn sample_count(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn generate(self: &Self, master_seed: i64) -> Vec<Vec<f64>> { /* ... */ }
    ```

- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `LatinHypercube`

Latin hypercube sampling — **RAVEN calls this `Stratified`**.

With `n` samples, each dimension's `[0, 1)` range is cut into `n`
equiprobable strata `[k/n, (k+1)/n)`. Exactly one point falls in each
stratum of each dimension; which stratum a given sample occupies is chosen
by an independent random permutation per dimension, so the strata are
randomly paired across dimensions. Within its stratum, the coordinate is
drawn uniformly:

```text
design[i][j] = (permutation_j[i] + u) / n,    u ~ U[0, 1)
```

# Why use it

The stratification removes the clustering and gaps that independent Monte
Carlo draws produce by chance, so for an integrand with a strong additive
(main-effect) component the variance of the estimate is lower than plain
Monte Carlo at the same `n`. It buys nothing for a purely interactive
integrand, and it does not change the `N^{-1/2}` asymptotic rate.

# The exact property

One point per stratum per dimension is a **deterministic** property of the
construction, not a statistical tendency, and it is asserted as such in the
verification below. It is the sharpest available test of this sampler.

Ported from RAVEN's `Samplers/Stratified.py`. Upstream builds the strata as
a `GridEntity` and permits unequal, user-supplied stratum boundaries; this
port fixes them equiprobable, which is the standard and the overwhelmingly
common use. Upstream's multivariate-normal / global-grid path is not
ported.

# Example

```
use raffles::samplers::{LatinHypercube, SamplingDesign};

let lhs = LatinHypercube::new(10, 2).unwrap();
let design = lhs.generate(7);

// Each dimension has exactly one point in each of the 10 strata.
let mut occupied = [false; 10];
for row in &design {
    let stratum = (row[0] * 10.0) as usize;
    assert!(!occupied[stratum]);
    occupied[stratum] = true;
}
```

```rust
pub struct LatinHypercube {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(samples: usize, dimensions: usize) -> Result<Self> { /* ... */ }
  ```
  Creates a Latin hypercube design of `samples` points over `dimensions`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **SamplingDesign**
  - ```rust
    fn dimensions(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn sample_count(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn generate(self: &Self, master_seed: i64) -> Vec<Vec<f64>> { /* ... */ }
    ```

- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GridSampler`

Full-factorial sampling on a tensor product of per-dimension CDF levels.

Each dimension carries a list of cumulative-probability levels in `[0, 1)`.
The design is every combination of one level from each dimension, so the
point count is the product of the per-dimension level counts — it grows
exponentially in `dimensions` and is the reason grid sampling is only
practical in low dimension.

# Ordering

Points come out in **odometer order with the last dimension varying
fastest**, which is row-major / C order. For levels `[[0.0, 0.5], [0.1,
0.9]]` the design is, in order:

```text
(0.0, 0.1)  (0.0, 0.9)  (0.5, 0.1)  (0.5, 0.9)
```

The order is part of the contract — it is what makes a fixed design
comparable across runs — and it is asserted in the verification below.

# Determinism

A grid uses no randomness at all, so
[`generate`](SamplingDesign::generate) ignores its `master_seed` and two
different seeds give the identical design.

Ported from RAVEN's `Samplers/Grid.py` with the grid construction from
`GridEntities.py`. Both of upstream's constructions are available —
`custom` as [`with_levels`](Self::with_levels), `equal` as
[`equally_spaced`](Self::equally_spaced). Upstream's value-space grids,
global grids shared across correlated variables, and refinement machinery
are not ported.

# Example

```
use raffles::samplers::{GridSampler, SamplingDesign};

// 3 levels on each of 2 dimensions -> 9 points.
let grid = GridSampler::equally_spaced(2, 2, 0.1, 0.9).unwrap();
assert_eq!(grid.sample_count(), 9);

let design = grid.generate(1);
assert_eq!(design[0], vec![0.1, 0.1]);
assert_eq!(design[8], vec![0.9, 0.9]);
```

```rust
pub struct GridSampler {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn with_levels(levels: Vec<Vec<f64>>) -> Result<Self> { /* ... */ }
  ```
  Builds a grid from explicit per-dimension levels — RAVEN's `custom`

- ```rust
  pub fn equally_spaced(dimensions: usize, steps: usize, lower: f64, upper: f64) -> Result<Self> { /* ... */ }
  ```
  Builds a grid with the same equally spaced levels on every dimension —

- ```rust
  pub fn equally_spaced_as_raven(dimensions: usize, steps: usize, lower: f64, upper: f64) -> Result<Self> { /* ... */ }
  ```
  Builds the same grid as [`equally_spaced`](Self::equally_spaced) but

- ```rust
  pub fn levels(self: &Self) -> &[Vec<f64>] { /* ... */ }
  ```
  The cumulative-probability levels along each dimension, as supplied.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **SamplingDesign**
  - ```rust
    fn dimensions(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn sample_count(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn generate(self: &Self, _master_seed: i64) -> Vec<Vec<f64>> { /* ... */ }
    ```

- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Sampler`

A design-of-experiments strategy over a set of uncertain inputs.

Dispatch is by `match` on this enum, never through a trait object: the set
of strategies is closed and known at compile time, so adding a variant makes
every site that forgot to handle it a compile error rather than a silent
runtime fallthrough. This replaces RAVEN's `Samplers/Factory.py`, which maps
XML type strings to classes at run time.

Every variant produces a design of cumulative probabilities in `[0, 1)`;
see the module documentation for the output contract.

# Example

```
use raffles::samplers::{LatinHypercube, MonteCarlo, Sampler};

let strategies = [
    Sampler::MonteCarlo(MonteCarlo::new(64, 3).unwrap()),
    Sampler::LatinHypercube(LatinHypercube::new(64, 3).unwrap()),
];

for strategy in &strategies {
    let design = strategy.generate(2026);
    assert_eq!(design.len(), 64);
    assert!(design.iter().flatten().all(|u| (0.0..1.0).contains(u)));
}
```

```rust
pub enum Sampler {
    MonteCarlo(MonteCarlo),
    LatinHypercube(LatinHypercube),
    Grid(GridSampler),
}
```

##### Variants

###### `MonteCarlo`

Independent uniform draws — see [`MonteCarlo`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `MonteCarlo` |  |

###### `LatinHypercube`

One point per equiprobable stratum per dimension — see
[`LatinHypercube`]. RAVEN names this sampler `Stratified`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LatinHypercube` |  |

###### `Grid`

Full-factorial tensor grid of CDF levels — see [`GridSampler`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `GridSampler` |  |

##### Implementations

###### Methods

- ```rust
  pub fn dimensions(self: &Self) -> usize { /* ... */ }
  ```
  Number of input dimensions the design spans.

- ```rust
  pub fn sample_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of design points this strategy will produce.

- ```rust
  pub fn generate(self: &Self, master_seed: i64) -> Vec<Vec<f64>> { /* ... */ }
  ```
  Produces the design: `sample_count()` rows of `dimensions()`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `SamplingDesign`

Compiler-enforced contract on every concrete sampling strategy.

This trait exists so the compiler checks that each strategy really does
report its shape and produce a design. It is **not** a dispatch mechanism —
per the workspace design rules there is no `Box<dyn SamplingDesign>`;
dispatch goes through the [`Sampler`] enum.

```rust
pub trait SamplingDesign {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `dimensions`: Number of input dimensions the design spans. Always at least 1.
- `sample_count`: Number of design points the strategy will produce. Always at least 1,
- `generate`: Produces the design: a `Vec` of [`sample_count`](Self::sample_count)

##### Implementations

This trait is implemented for the following types:

- `MonteCarlo`
- `LatinHypercube`
- `GridSampler`

### Functions

#### Function `stream_seed`

Derives the starting seed of an independent generator stream from a master
seed.

Stream `k` starts `k * DEFAULT_STRIDE` LCG steps after the master seed,
where `DEFAULT_STRIDE = 152_917` is OpenMC's per-particle stride. Two
streams therefore do not overlap as long as neither consumes more than
`DEFAULT_STRIDE` draws — which is exactly the guarantee OpenMC relies on to
make parallel Monte Carlo reproducible independent of thread count.

Use this to give replicates or parallel workers their own generators while
keeping the whole computation reproducible from one master seed. The
samplers in this module use the same mechanism internally, one stream per
sampled dimension, with a stride widened past `DEFAULT_STRIDE` whenever a
design needs more draws than that.

`master_seed` may be any `i64` and `stream` any index; there are no bad
values.

# Why not `petir::rng::lcg::init_seed`

~~That helper computes `future_seed(id + offset, future_seed(DEFAULT_STRIDE,
master))`, i.e. consecutive `id`s land **one LCG step apart**, not one
stride apart. [...] The discrepancy is in `outram-mc-libs`, not here, and is
reported rather than patched from this crate.~~ **CORRECTED 2026-10-02** —
stale since `op-rbo` (2026-08-06): `init_seed` now computes
`future_seed(id * DEFAULT_STRIDE, master_seed + offset)` with wrapping
arithmetic, exactly OpenMC's `src/random_lcg.cpp:60`, so it too gives
non-overlapping streams (checked by reading `petir::rng::lcg::init_seed`
and its test `init_seed_consecutive_ids_are_one_full_stride_apart`). This
function still calls `future_seed` directly, now as a preference rather
than a workaround: it lets a design widen the stride past
`DEFAULT_STRIDE`, which `init_seed` pins.

# Example

```
use raffles::samplers::stream_seed;

let a = stream_seed(2026, 0);
let b = stream_seed(2026, 1);
assert_ne!(a, b);
assert_eq!(a, stream_seed(2026, 0)); // reproducible
```

```rust
pub fn stream_seed(master_seed: i64, stream: usize) -> u64 { /* ... */ }
```

## Module `scram`

# SCRAM port — fault-tree quantification

A translation of the probabilistic-risk-analysis core of
[SCRAM](https://github.com/rakhimov/scram): taking the **minimal cut sets**
of a fault tree and turning them into a top-event probability and a ranking
of which basic events matter.

## What a cut set is, for a reader who has not met one

A **fault tree** is a Boolean expression for how a system fails, written
over *basic events* — a pump not starting, a valve stuck shut. A **cut set**
is a set of basic events whose simultaneous occurrence is sufficient to
cause the top event. It is **minimal** when removing any member stops it
being sufficient.

The minimal cut sets are the complete qualitative answer: the system fails
exactly when at least one of them occurs. The quantitative questions — how
likely, and which events drive it — follow from them.

## The route through this module

1. [`fault_tree::FaultTreeBuilder`] — describe the tree in names.
2. [`mocus::minimal_cut_sets`] — generate the cut sets, or
   [`zbdd::minimal_cut_sets`] when the model is big enough that top-down
   expansion gives up.
3. [`top_event_probability`] — quantify.
4. [`importance_factors`] — rank the basic events.

**If all you want is the probability, skip to [`bdd::Bdd`].** It evaluates
the tree's Boolean function directly, so it needs no cut sets, has no
`2^n` ceiling, and on a non-coherent tree gives the *true* value where cut
sets can only bound it. Cut sets remain the answer to "how does it fail",
which no single probability can give.

Steps 3 and 4 are **ports** of SCRAM and carry its attribution headers.
Step 2 is **not**: SCRAM generates cut sets with a ZBDD over a
heavily-preprocessed Boolean graph, which is the larger part of the
upstream and is not ported. [`mocus`] is the classical top-down expansion
from the published literature instead, verified *against* SCRAM's reported
products rather than translated from its code — see that module.

A caller who already has cut sets from elsewhere can skip straight to
step 3; [`CutSet`] does not care where they came from.

**Non-coherent trees are handled, and there the answer you ask for
matters.** Where a `not`, `nand`, `nor` or `xor` appears, a component
*working* can contribute to the top event. Minimal cut sets then discard
that information and become **conservative** — quantifying them bounds the
probability from above. Two things recover the exact answer:
[`bdd::Bdd::probability`] for the probability itself, and
[`zbdd::prime_implicants`] for the combinations, which keep the
complemented literals. Measured on the fixture's small non-coherent model:
cut sets sum to `0.80`, prime implicants to `0.54`, and the truth is
`0.5032`.

**Reading SCRAM's own input models.** [`mef`] takes a Model Exchange
Format document to a [`fault_tree::FaultTreeModel`], evaluating the
basic-event expressions through [`expression`] on the way. It handles
`<define-component>`'s private namespaces, `<xi:include>`, and all eleven
MEF connectives, and it **refuses** rather than skips anything it does not
read.

**Common-cause failure groups are applied by default.** A model that
declares one has said its components are coupled, and [`ccf`] rewrites the
tree accordingly — each member becomes a proxy gate over the shared-failure
events it belongs to. Upstream does this only under `scram --ccf`;
[`mef::MefModel::without_ccf`] is the explicit ablation, and both paths are
verified against the corresponding SCRAM run. On
`TwoTrain/common_cause` the difference is `0.0622587` against `0.0361` —
ignoring a declared group is not a small approximation.

~~**What is still absent:** everything SCRAM does around this core — XML
input models, … and the expression library~~ **CORRECTED 2026-09-22** —
both landed, and on 2026-09-22 so did the seven random deviates and the
four common-cause-failure models ([`ccf`]) and the substitutions
([`substitution`]), and sampling with the uncertainty analysis over it
([`uncertainty`]) and the alignments ([`alignment`]). What is still
absent: event trees, the
reporter, and — in the analysis itself — the preprocessor, whose absence is
a cost in diagram size rather than in answers.

**Uncertainty analysis.** [`uncertainty::analyse`] runs upstream's Monte
Carlo: draw every random deviate afresh each trial, quantify, and report
the distribution of the answers. Because the random stream differs from
upstream's, its verification is statistical rather than exact — the one
place in this module where that is so, and `scram_uncertainty` states the
confidence and checks the samplers a second way, against closed-form
moments.

## Confidence-box basic events (gh:#987)

[`pba`] and [`pbox_tree`] evaluate a [`FaultTree`] whose basic events are
**confidence boxes** or p-boxes rather than numbers, gate by gate, with the
dependence assumption chosen explicitly: [`pbox_tree::evaluate_pbox_tree`]
(independent or Fréchet unknown dependence, per gate) and
[`pbox_tree::evaluate_cbox_edges`] (the left-edge / right-edge construction
of Lye et al. 2024 for repeated events). Ports of Adolphus Lye's
"Computing with Confidence" case studies and Scott Ferson's pba.r;
verified against the original R scripts run with R 4.6.1
(`crates/raffles/docs/computing-with-confidence-verification.md`). AI-assisted draft.

## Where this sits relative to the rest of the crate

[`crate::imprecise::SystemStructure`] also computes system reliability, and
the two are **not** duplicates:

| | `imprecise::SystemStructure` | this module |
|---|---|---|
| input | component reliabilities | minimal cut sets + event probabilities |
| structure | series, parallel, k-out-of-n | arbitrary coherent fault tree |
| numbers | **interval-valued** | point-valued |
| question | reliability under dependence assumptions | top-event probability and event importance |

Reach for `imprecise` when the structure is simple and the inputs are
bounds; reach for this when the structure is a real fault tree.

## Verification

Every function here is checked against **upstream SCRAM compiled and run**,
not against a reading of its source. The oracle harness, the exact upstream
commit, the one build patch that was needed, and the measured agreement are
recorded in `crates/raffles/docs/scram-port-verification.md`.

```rust
pub mod scram { /* ... */ }
```

### Modules

## Module `alignment`

Alignments — the same model analysed in several operating configurations.

A plant is not in one state all year. It runs normally most of the time,
with one pump out for maintenance some of it, and the risk is different in
each. An **alignment** names those configurations, gives each a fraction of
the mission time, and says which house events are set in it.

# What a phase changes

Two things, and upstream's `RiskAnalysis::RunAnalysis` does both before
running the whole analysis again:

1. the **mission time** becomes `time_fraction x mission_time`, so every
   `<exponential>` and `<periodic-test>` is evaluated over that phase's
   duration;
2. the phase's `<set-house-event>` instructions flip the named house
   events, which prunes whole branches of the tree.

There is **no single answer** for a model with an alignment: upstream
produces one result set per phase, tagged with the alignment and phase
names, and so does this. [`super::mef::MefModel::in_phase`] returns the
model as it stands in one phase; the caller loops.

The fractions must sum to 1 — upstream checks it to `1e-4` — so a caller
wanting a single number can combine the per-phase results by those
fractions. That combination is the analyst's to make, and neither upstream
nor this port makes it for them.

```rust
pub mod alignment { /* ... */ }
```

### Types

#### Struct `Phase`

One operating configuration within an [`Alignment`].

```rust
pub struct Phase {
    pub name: String,
    pub time_fraction: f64,
    pub set_house_events: Vec<(String, bool)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Its name, as the report tags the phase's results. |
| `time_fraction` | `f64` | The fraction of the mission time spent in it, in `(0, 1]`. |
| `set_house_events` | `Vec<(String, bool)>` | `<set-house-event>` instructions: `(house-event id, state)`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Alignment`

A set of phases covering the whole mission time.

```rust
pub struct Alignment {
    pub name: String,
    pub phases: Vec<Phase>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Its id. |
| `phases` | `Vec<Phase>` | Its phases, in declaration order. |

##### Implementations

###### Methods

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Upstream's `Phase::Phase` domain check and `Alignment::Validate`.

- ```rust
  pub fn phase(self: &Self, name: &str) -> Option<&Phase> { /* ... */ }
  ```
  The phase of that name.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `bdd`

Binary decision diagrams — the **exact** top-event probability, at any
scale.

[`super::probability::Approximation::Exact`] computes the same quantity by
inclusion-exclusion over the minimal cut sets, which is `2^n` in their
number and refuses past
[`super::probability::EXACT_CUT_SET_LIMIT`]. This module has no such limit:
it evaluates the fault tree's Boolean function directly, so it scales to
models where enumerating cut sets is hopeless, and it needs no cut sets at
all.

**For a non-coherent tree the two are not even the same number.** Minimal
cut sets are conservative — deleting the complemented literals discards the
requirement that a component be *working* — so quantifying them gives an
upper bound. The BDD evaluates the real function. On the fixture's small
non-coherent model that is `0.5032` here against `0.6220` from the cut
sets, and `0.5032` is the right answer.

# Variable ordering

A BDD's *size* depends heavily on the order its variables are tested in;
its *value* does not. This module orders variables by first appearance in a
depth-first walk from the top gate, which keeps related events adjacent and
is what makes the fixture's largest model tractable. Finding a good order
in general is NP-hard, and upstream spends its preprocessor on the problem;
nothing here does. A tree that blows up under this order would need that
work, and [`Bdd::node_count`] is how you would see it happening.

# Example

```
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
use raffles::scram::bdd::Bdd;

let mut b = FaultTreeBuilder::new();
b.basic_event("ValveOne", 0.5).unwrap();
b.basic_event("PumpOne", 0.7).unwrap();
b.basic_event("ValveTwo", 0.5).unwrap();
b.basic_event("PumpTwo", 0.7).unwrap();
b.gate("TrainOne", Connective::Or, &["ValveOne", "PumpOne"]).unwrap();
b.gate("TrainTwo", Connective::Or, &["ValveTwo", "PumpTwo"]).unwrap();
b.gate("TopEvent", Connective::And, &["TrainOne", "TrainTwo"]).unwrap();
let model = b.build("TopEvent").unwrap();

let bdd = Bdd::build(model.tree()).unwrap();
let p = bdd.probability(model.probabilities()).unwrap();
assert!((p - 0.7225).abs() < 1e-12);
```

```rust
pub mod bdd { /* ... */ }
```

### Types

#### Type Alias `NodeId`

A node in the diagram: `0` is the constant **false**, `1` the constant
**true**, and anything above indexes [`Bdd::nodes`].

```rust
pub type NodeId = usize;
```

#### Struct `Ite`

One if-then-else node: test `variable`, follow `high` when it occurs and
`low` when it does not.

```rust
pub struct Ite {
    pub order: usize,
    pub high: NodeId,
    pub low: NodeId,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `order` | `usize` | Position of the tested variable in the diagram's own ordering, **not**<br>a basic-event index. [`Bdd::basic_event`] converts. |
| `high` | `NodeId` | Followed when the variable occurs. |
| `low` | `NodeId` | Followed when it does not. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Bdd`

A reduced, ordered binary decision diagram of a fault tree's Boolean
function.

```rust
pub struct Bdd {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn variable_order(tree: &FaultTree) -> Vec<usize> { /* ... */ }
  ```
  Builds the diagram of a fault tree's Boolean function.

- ```rust
  pub fn build(tree: &FaultTree) -> Result<Self> { /* ... */ }
  ```

- ```rust
  pub fn node_count(self: &Self) -> usize { /* ... */ }
  ```
  How many if-then-else nodes the diagram holds.

- ```rust
  pub fn root(self: &Self) -> NodeId { /* ... */ }
  ```
  The root node.

- ```rust
  pub fn is_never(self: &Self) -> bool { /* ... */ }
  ```
  Whether the function is the constant false — a tree that cannot fail.

- ```rust
  pub fn is_always(self: &Self) -> bool { /* ... */ }
  ```
  Whether the function is the constant true — a tree that always fails.

- ```rust
  pub fn basic_event(self: &Self, order: usize) -> usize { /* ... */ }
  ```
  The basic event tested at ordering position `order`.

- ```rust
  pub fn variable_count(self: &Self) -> usize { /* ... */ }
  ```
  How many variables the diagram orders.

- ```rust
  pub fn conjoin(self: &mut Self, a: NodeId, b: NodeId) -> Result<NodeId> { /* ... */ }
  ```
  The conjunction of two nodes, extending the diagram as needed.

- ```rust
  pub fn ite(self: &Self, node: NodeId) -> Option<Ite> { /* ... */ }
  ```
  The if-then-else at `node`, or `None` at a terminal.

- ```rust
  pub fn probability(self: &Self, event_probabilities: &[f64]) -> Result<f64> { /* ... */ }
  ```
  Exact probability that the top event occurs.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `ZERO`

The constant-false terminal.

```rust
pub const ZERO: NodeId = 0;
```

#### Constant `ONE`

The constant-true terminal.

```rust
pub const ONE: NodeId = 1;
```

#### Constant `NODE_LIMIT`

Ceiling on how many distinct nodes may be created.

A BDD is exponential in the worst case, and on a bad variable order an
ordinary-looking model reaches that worst case. Hitting this returns an
error naming the limit rather than exhausting memory, for the same reason
[`super::mocus::EXPANSION_LIMIT`] exists: a diagnosable failure beats a
disappearing process.

```rust
pub const NODE_LIMIT: usize = 20_000_000;
```

## Module `ccf`

Common-cause failure groups.

A fault tree that treats two pumps as independent understates the risk:
one flood, one maintenance error or one bad batch fails both. A CCF group
says "these members share a cause", gives the probability of a member
failing at all, and gives the fractions belonging to each size of
simultaneous failure.

# What applying a group does to the tree

Upstream's `CcfGroup::ApplyModel` rewrites the tree rather than adjusting
numbers, and this port does the same:

1. every member basic event becomes a **proxy gate of the same name**;
2. for each level `k` and each `k`-subset of the members, a new basic
   event `[A B …]` is created with that level's probability;
3. each proxy gate becomes an `or` over every CCF event it belongs to.

So a two-member group replaces `PumpOne` with
`or([PumpOne], [PumpOne PumpTwo])`, and the shared failure appears in the
cut sets as a single event — which is exactly what makes it show up as an
order-1 product where the independent model gave order 2.

# Applied by default, ablated explicitly

Upstream applies CCF groups only under `scram --ccf`. **This port applies
them by default**, following the workspace rule that physics the data
supplies is applied unless a caller explicitly ablates it: a model that
declares a CCF group has said its components are coupled, and a run that
silently ignores that reports a risk it has been told is wrong.
[`super::mef::MefModel::without_ccf`] is the visible ablation, and both
paths are verified against the corresponding SCRAM run.

```rust
pub mod ccf { /* ... */ }
```

### Types

#### Enum `CcfModel`

Which common-cause model a group uses.

Upstream's four `CcfGroup` subclasses. Each differs in what its factors
mean and therefore in `CalculateProbabilities`; they share everything
else.

```rust
pub enum CcfModel {
    BetaFactor,
    Mgl,
    AlphaFactor,
    PhiFactor,
}
```

##### Variants

###### `BetaFactor`

`beta-factor`: if the common cause occurs, **all** members fail.
One factor, at the group's own size.

###### `Mgl`

`MGL`, the multiple-Greek-letter model: factor `k` is the fraction of
failures of `k` or more given that `k - 1` failed. Factors start at
level 2.

###### `AlphaFactor`

`alpha-factor`: factor `k` is the fraction of failures involving
exactly `k` members.

###### `PhiFactor`

`phi-factor`: the fractions are given directly, `Q_k = phi_k Q`.

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  The MEF attribute value.

- ```rust
  pub fn parse(s: &str) -> Result<Self> { /* ... */ }
  ```
  Reads the MEF attribute value.

- ```rust
  pub fn min_level(self: Self, members: usize) -> usize { /* ... */ }
  ```
  Upstream's `min_level()`: the lowest factor level the model takes.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CcfGroup`

A common-cause failure group.

```rust
pub struct CcfGroup {
    pub name: String,
    pub model: CcfModel,
    pub members: Vec<String>,
    pub distribution: super::expression::Expression,
    pub factors: Vec<(usize, super::expression::Expression)>,
    pub base_path: String,
    pub private: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The group's id, as [`super::mef`] resolves names. |
| `model` | `CcfModel` | Which model the factors belong to. |
| `members` | `Vec<String>` | Member basic-event ids, in declaration order. The order decides which<br>subsets are formed and in what order, so it is not incidental. |
| `distribution` | `super::expression::Expression` | The probability of a member failing at all — upstream's<br>`distribution_`, which also becomes each member's own probability<br>before the model is applied. |
| `factors` | `Vec<(usize, super::expression::Expression)>` | `(level, factor)`, dense from [`CcfModel::min_level`] upwards. |
| `base_path` | `String` | The dotted path CCF events are named in, empty at model level. |
| `private` | `bool` | Whether the group is private, so its CCF events take their full path<br>as their id. |

##### Implementations

###### Methods

- ```rust
  pub fn validate(self: &Self, parameters: &Parameters, mission_time: f64) -> Result<()> { /* ... */ }
  ```
  Upstream's `CcfGroup::Validate`.

- ```rust
  pub fn probabilities(self: &Self) -> Result<Vec<(usize, Expression)>> { /* ... */ }
  ```
  Upstream's `CalculateProbabilities`, one arm per model.

- ```rust
  pub fn events(self: &Self) -> Result<Vec<CcfEvent>> { /* ... */ }
  ```
  The CCF events this group creates, and which members each one couples.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CcfEvent`

One generated common-cause basic event.

```rust
pub struct CcfEvent {
    pub id: String,
    pub group: String,
    pub level: usize,
    pub members: Vec<String>,
    pub probability: super::expression::Expression,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | The id it is registered under — `[A B]`, or the full path if the<br>group is private. |
| `group` | `String` | The group that created it. |
| `level` | `usize` | How many members fail together. |
| `members` | `Vec<String>` | The member ids it couples. |
| `probability` | `super::expression::Expression` | Its probability. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `proxy_arguments`

The proxy-gate arguments a set of CCF events implies: member id -> the
events that couple it.

Upstream builds this while generating the events; separating it keeps
[`CcfGroup`] free of the tree types.

```rust
pub fn proxy_arguments(events: &[CcfEvent]) -> std::collections::HashMap<String, Vec<String>> { /* ... */ }
```

## Module `expression`

Expressions — how a basic event's probability is *computed* rather than
stated.

A fault tree's leaves are rarely given as bare numbers. A component with a
failure rate and a mission time has an **exponential** probability; one
that is tested periodically and repaired has a probability that saws up
and down with the test interval. The Model Exchange Format writes these as
expression trees, and this module evaluates them.

Two of the fixture's models need this to be read at all: `HIPPS` defines
every basic event by `<periodic-test>` or `<GLM>`, and its probabilities
appear nowhere in the input as literals — SCRAM computes them. Reproducing
those exact values is the verification, and
`tests/scram_expressions.rs` does it.

# Random deviates

The seven `<*-deviate>` and `<histogram>` elements are here too. Each has
two faces upstream: `value()` is the distribution's **mean**, which is
what an ordinary SCRAM run computes and prints, and `DoSample()` draws
from it, which only the uncertainty analysis calls.
[`Expression::evaluate`] is the first and [`Expression::sample`] the
second; [`super::uncertainty`] is what consumes the latter.

Their arrival is also what gives [`Expression::interval`] a purpose: a
normal deviate with a comfortable mean has a domain reaching six sigma
below it, and upstream rejects such an argument on the *domain* half of
its validation even though the value passes.

# Example

```
use raffles::scram::expression::Expression;

// A component with failure rate 1e-6/h over a 1000 h mission.
let e = Expression::exponential(
    Expression::float(1e-6),
    Expression::float(1000.0),
);
let p = e.value().unwrap();
assert!((p - (1.0 - (-1e-3f64).r_exp())).abs() < 1e-15);
```

```rust
pub mod expression { /* ... */ }
```

### Types

#### Type Alias `Parameters`

A named parameter's value, resolved when an expression is evaluated.

Upstream models parameters as first-class `Parameter` expressions in a
graph with cycle detection (`src/parameter.{h,cc}`, `src/cycle.h`). Here a
parameter is a *name* looked up in this table, which keeps [`Expression`]
a tree rather than a graph — the cycle it could otherwise form is
unrepresentable instead of detected.

```rust
pub type Parameters = std::collections::HashMap<String, f64>;
```

#### Enum `Expression`

How a quantity is computed.

One enum rather than upstream's class hierarchy, per the workspace
no-trait-objects rule. Arguments are `Arc`-shared because the MEF lets one
parameter feed several expressions, and because the rules forbid `Box`.

```rust
pub enum Expression {
    Float(f64),
    Bool(bool),
    MissionTime,
    Parameter(String),
    Exponential {
        lambda: alloc::sync::Arc<Expression>,
        time: alloc::sync::Arc<Expression>,
    },
    Glm {
        gamma: alloc::sync::Arc<Expression>,
        lambda: alloc::sync::Arc<Expression>,
        mu: alloc::sync::Arc<Expression>,
        time: alloc::sync::Arc<Expression>,
    },
    Weibull {
        alpha: alloc::sync::Arc<Expression>,
        beta: alloc::sync::Arc<Expression>,
        t0: alloc::sync::Arc<Expression>,
        time: alloc::sync::Arc<Expression>,
    },
    PeriodicTestInstantRepair {
        lambda: alloc::sync::Arc<Expression>,
        tau: alloc::sync::Arc<Expression>,
        theta: alloc::sync::Arc<Expression>,
        time: alloc::sync::Arc<Expression>,
    },
    PeriodicTestInstantTest {
        lambda: alloc::sync::Arc<Expression>,
        mu: alloc::sync::Arc<Expression>,
        tau: alloc::sync::Arc<Expression>,
        theta: alloc::sync::Arc<Expression>,
        time: alloc::sync::Arc<Expression>,
    },
    Add(Vec<Expression>),
    Sub(Vec<Expression>),
    Mul(Vec<Expression>),
    Div(Vec<Expression>),
    Neg(alloc::sync::Arc<Expression>),
    Abs(alloc::sync::Arc<Expression>),
    Min(Vec<Expression>),
    Max(Vec<Expression>),
    Mean(Vec<Expression>),
    Exp(alloc::sync::Arc<Expression>),
    Log(alloc::sync::Arc<Expression>),
    Log10(alloc::sync::Arc<Expression>),
    Pow(alloc::sync::Arc<Expression>, alloc::sync::Arc<Expression>),
    Sqrt(alloc::sync::Arc<Expression>),
    Mod(alloc::sync::Arc<Expression>, alloc::sync::Arc<Expression>),
    Trunc(alloc::sync::Arc<Expression>),
    Round(alloc::sync::Arc<Expression>),
    Floor(alloc::sync::Arc<Expression>),
    Ceil(alloc::sync::Arc<Expression>),
    Ite {
        condition: alloc::sync::Arc<Expression>,
        consequent: alloc::sync::Arc<Expression>,
        alternate: alloc::sync::Arc<Expression>,
    },
    Not(alloc::sync::Arc<Expression>),
    And(Vec<Expression>),
    Or(Vec<Expression>),
    Eq(alloc::sync::Arc<Expression>, alloc::sync::Arc<Expression>),
    Lt(alloc::sync::Arc<Expression>, alloc::sync::Arc<Expression>),
    Gt(alloc::sync::Arc<Expression>, alloc::sync::Arc<Expression>),
    UniformDeviate {
        min: alloc::sync::Arc<Expression>,
        max: alloc::sync::Arc<Expression>,
    },
    NormalDeviate {
        mean: alloc::sync::Arc<Expression>,
        sigma: alloc::sync::Arc<Expression>,
    },
    LognormalDeviate {
        mean: alloc::sync::Arc<Expression>,
        ef: alloc::sync::Arc<Expression>,
        level: alloc::sync::Arc<Expression>,
    },
    LognormalDeviateNormal {
        mu: alloc::sync::Arc<Expression>,
        sigma: alloc::sync::Arc<Expression>,
    },
    GammaDeviate {
        k: alloc::sync::Arc<Expression>,
        theta: alloc::sync::Arc<Expression>,
    },
    BetaDeviate {
        alpha: alloc::sync::Arc<Expression>,
        beta: alloc::sync::Arc<Expression>,
    },
    Histogram {
        boundaries: Vec<Expression>,
        weights: Vec<Expression>,
    },
}
```

##### Variants

###### `Float`

A literal number. Upstream `ConstantExpression`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Bool`

A literal boolean, evaluating to 1 or 0. Upstream `ConstantExpression`
with a `bool`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `bool` |  |

###### `MissionTime`

`<system-mission-time/>` — the model's mission time.

###### `Parameter`

A named parameter, resolved against a [`Parameters`] table.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Exponential`

`1 - exp(-lambda * t)`. Upstream `Exponential`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `lambda` | `alloc::sync::Arc<Expression>` | Failure rate, per hour. |
| `time` | `alloc::sync::Arc<Expression>` | Mission time, hours. |

###### `Glm`

The GLM (failure-on-demand plus repairable) model. Upstream `Glm`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `gamma` | `alloc::sync::Arc<Expression>` | Probability of failure on demand. |
| `lambda` | `alloc::sync::Arc<Expression>` | Failure rate, per hour. |
| `mu` | `alloc::sync::Arc<Expression>` | Repair rate, per hour. |
| `time` | `alloc::sync::Arc<Expression>` | Mission time, hours. |

###### `Weibull`

The Weibull failure model. Upstream `Weibull`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `alpha` | `alloc::sync::Arc<Expression>` | Scale parameter. |
| `beta` | `alloc::sync::Arc<Expression>` | Shape parameter. |
| `t0` | `alloc::sync::Arc<Expression>` | Time shift before failures can begin. |
| `time` | `alloc::sync::Arc<Expression>` | Mission time, hours. |

###### `PeriodicTestInstantRepair`

Periodic testing with **instant repair** — upstream's four-argument
`PeriodicTest`, its `InstantRepair` flavour.

The component is tested every `tau` hours starting at `theta`, and a
discovered failure is repaired instantly, so the probability resets at
every test. This is the flavour the fixture's `HIPPS` uses.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `lambda` | `alloc::sync::Arc<Expression>` | Failure rate while functioning, per hour. |
| `tau` | `alloc::sync::Arc<Expression>` | Time between tests, hours. |
| `theta` | `alloc::sync::Arc<Expression>` | Time before the first test, hours. |
| `time` | `alloc::sync::Arc<Expression>` | Mission time, hours. |

###### `PeriodicTestInstantTest`

Periodic testing with an **instant test** and a repair rate —
upstream's five-argument `PeriodicTest`, its `InstantTest` flavour.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `lambda` | `alloc::sync::Arc<Expression>` | Failure rate while functioning, per hour. |
| `mu` | `alloc::sync::Arc<Expression>` | Repair rate, per hour. |
| `tau` | `alloc::sync::Arc<Expression>` | Time between tests, hours. |
| `theta` | `alloc::sync::Arc<Expression>` | Time before the first test, hours. |
| `time` | `alloc::sync::Arc<Expression>` | Mission time, hours. |

###### `Add`

Sum of the arguments. Upstream `Add`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Sub`

First argument minus the rest. Upstream `Sub`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Mul`

Product of the arguments. Upstream `Mul`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Div`

First argument divided by the rest. Upstream `Div`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Neg`

Arithmetic negation. Upstream `Neg`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Abs`

Absolute value. Upstream `Abs`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Min`

Smallest argument. Upstream `Min`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Max`

Largest argument. Upstream `Max`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Mean`

Arithmetic mean of the arguments. Upstream `Mean`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Exp`

`e` raised to the argument. Upstream `Exp`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Log`

Natural logarithm. Upstream `Log`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Log10`

Base-10 logarithm. Upstream `Log10`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Pow`

First argument raised to the second. Upstream `Pow`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |
| 1 | `alloc::sync::Arc<Expression>` |  |

###### `Sqrt`

Square root. Upstream `Sqrt`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Mod`

Remainder of integer division. Upstream `Mod`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |
| 1 | `alloc::sync::Arc<Expression>` |  |

###### `Trunc`

Round towards zero. Upstream `Trunc` / `Integer`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Round`

Round to the nearest integer. Upstream `Round`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Floor`

Round down. Upstream `Floor`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Ceil`

Round up. Upstream `Ceil`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `Ite`

`if condition then consequent else alternate`. Upstream `Ite`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `condition` | `alloc::sync::Arc<Expression>` | Evaluated as a boolean: non-zero is true. |
| `consequent` | `alloc::sync::Arc<Expression>` | Taken when the condition holds. |
| `alternate` | `alloc::sync::Arc<Expression>` | Taken otherwise. |

###### `Not`

Logical negation: 1 when the argument is zero, else 0. Upstream `Not`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |

###### `And`

All arguments non-zero. Upstream `And`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Or`

Any argument non-zero. Upstream `Or`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Expression>` |  |

###### `Eq`

Equality. Upstream `Eq`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |
| 1 | `alloc::sync::Arc<Expression>` |  |

###### `Lt`

Strictly less than. Upstream `Lt`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |
| 1 | `alloc::sync::Arc<Expression>` |  |

###### `Gt`

Strictly greater than. Upstream `Gt`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `alloc::sync::Arc<Expression>` |  |
| 1 | `alloc::sync::Arc<Expression>` |  |

###### `UniformDeviate`

`<uniform-deviate>`. Upstream `UniformDeviate`.

Its deterministic value is the midpoint, which is the distribution's
mean.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `min` | `alloc::sync::Arc<Expression>` | Lower bound. |
| `max` | `alloc::sync::Arc<Expression>` | Upper bound. |

###### `NormalDeviate`

`<normal-deviate>`. Upstream `NormalDeviate`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mean` | `alloc::sync::Arc<Expression>` | Mean. |
| `sigma` | `alloc::sync::Arc<Expression>` | Standard deviation. |

###### `LognormalDeviate`

The three-argument `<lognormal-deviate>` — upstream's `Logarithmic`
flavour, parametrised by the distribution's **own** mean and an error
factor at a confidence level.

`EF = exp(z_level * sigma)`, so `sigma = ln(EF) / z_level`, and
`mu = ln(mean) - sigma^2 / 2`. This is the flavour `SmallTree` and
`BSCU` use.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mean` | `alloc::sync::Arc<Expression>` | Mean of the log-normal distribution (not of the underlying normal). |
| `ef` | `alloc::sync::Arc<Expression>` | Error factor. |
| `level` | `alloc::sync::Arc<Expression>` | Confidence level the error factor is quoted at, in `(0, 1)`. |

###### `LognormalDeviateNormal`

The two-argument `<lognormal-deviate>` — upstream's `Normal` flavour,
parametrised by the **underlying normal's** mean and standard
deviation.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mu` | `alloc::sync::Arc<Expression>` | Location parameter: mean of `ln X`. |
| `sigma` | `alloc::sync::Arc<Expression>` | Scale parameter: standard deviation of `ln X`. |

###### `GammaDeviate`

`<gamma-deviate>`. Upstream `GammaDeviate`, shape and **scale**.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `k` | `alloc::sync::Arc<Expression>` | Shape parameter. |
| `theta` | `alloc::sync::Arc<Expression>` | Scale parameter. |

###### `BetaDeviate`

`<beta-deviate>`. Upstream `BetaDeviate`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `alpha` | `alloc::sync::Arc<Expression>` | First shape parameter. |
| `beta` | `alloc::sync::Arc<Expression>` | Second shape parameter. |

###### `Histogram`

`<histogram>`. Upstream `Histogram`: a piecewise-constant density.

`boundaries` has one more entry than `weights`: the first is the lower
bound of the first bin, and each later one closes a bin. Upstream keeps
them in one argument list split at the midpoint; here they are two
fields, which makes the length invariant representable.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `boundaries` | `Vec<Expression>` | Bin boundaries, strictly increasing, `weights.len() + 1` of them. |
| `weights` | `Vec<Expression>` | Positive weight of each bin. |

##### Implementations

###### Methods

- ```rust
  pub fn float(value: f64) -> Self { /* ... */ }
  ```
  A literal number.

- ```rust
  pub fn exponential(lambda: Expression, time: Expression) -> Self { /* ... */ }
  ```
  `1 - exp(-lambda * t)`.

- ```rust
  pub fn glm(gamma: Expression, lambda: Expression, mu: Expression, time: Expression) -> Self { /* ... */ }
  ```
  The GLM model: failure on demand `gamma`, failure rate `lambda`,

- ```rust
  pub fn weibull(alpha: Expression, beta: Expression, t0: Expression, time: Expression) -> Self { /* ... */ }
  ```
  The Weibull model.

- ```rust
  pub fn periodic_test_instant_repair(lambda: Expression, tau: Expression, theta: Expression, time: Expression) -> Self { /* ... */ }
  ```
  Periodic testing with instant repair — the four-argument form.

- ```rust
  pub fn periodic_test_instant_test(lambda: Expression, mu: Expression, tau: Expression, theta: Expression, time: Expression) -> Self { /* ... */ }
  ```
  Periodic testing with an instant test and a repair rate — the

- ```rust
  pub fn value(self: &Self) -> Result<f64> { /* ... */ }
  ```
  Evaluates the expression with no parameters and the default mission

- ```rust
  pub fn evaluate(self: &Self, parameters: &Parameters, mission_time: f64) -> Result<f64> { /* ... */ }
  ```
  Evaluates the expression.

- ```rust
  pub fn validate(self: &Self, parameters: &Parameters, mission_time: f64) -> Result<()> { /* ... */ }
  ```
  Upstream's `Validate()`: the domain checks each formula requires.

- ```rust
  pub fn uniform_deviate(min: Expression, max: Expression) -> Self { /* ... */ }
  ```
  `<uniform-deviate>`.

- ```rust
  pub fn normal_deviate(mean: Expression, sigma: Expression) -> Self { /* ... */ }
  ```
  `<normal-deviate>`.

- ```rust
  pub fn lognormal_deviate(mean: Expression, ef: Expression, level: Expression) -> Self { /* ... */ }
  ```
  The three-argument `<lognormal-deviate>`: mean, error factor, level.

- ```rust
  pub fn lognormal_deviate_normal(mu: Expression, sigma: Expression) -> Self { /* ... */ }
  ```
  The two-argument `<lognormal-deviate>`: the underlying normal's

- ```rust
  pub fn gamma_deviate(k: Expression, theta: Expression) -> Self { /* ... */ }
  ```
  `<gamma-deviate>`: shape and scale.

- ```rust
  pub fn beta_deviate(alpha: Expression, beta: Expression) -> Self { /* ... */ }
  ```
  `<beta-deviate>`.

- ```rust
  pub fn is_deviate(self: &Self) -> bool { /* ... */ }
  ```
  Whether the expression's value deviates from its mean — upstream's

- ```rust
  pub fn interval(self: &Self, parameters: &Parameters, mission_time: f64) -> Result<Interval> { /* ... */ }
  ```
  The domain of values this expression can take — upstream's

- ```rust
  pub fn sample(self: &Self, parameters: &Parameters, mission_time: f64, seed: &mut u64) -> Result<f64> { /* ... */ }
  ```
  Draws one value — upstream's `Expression::Sample`.

- ```rust
  pub fn is_deviate_through(self: &Self, definitions: &HashMap<String, Expression>) -> bool { /* ... */ }
  ```
  Whether a random deviate is reachable from this expression, **looking

- ```rust
  pub fn sample_memoised(self: &Self, definitions: &HashMap<String, Expression>, values: &Parameters, round: &mut Parameters, mission_time: f64, seed: &mut u64) -> Result<f64> { /* ... */ }
  ```
  One trial's draw of this expression **in upstream's order** — the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Interval`

A value's domain, for validation only — upstream's `Interval`.

Upstream is `boost::icl::continuous_interval<double>`, and the only
operations it uses are the four constructors, `lower`/`upper`, `contains`
and `within`. Those are what this carries; nothing here is a general
interval-arithmetic type.

**What it is for.** Upstream validates an argument twice: its *value* must
be in range, and its *sample domain* must be too
(`EnsureNonNegative`/`EnsurePositive`/`EnsureWithin` in `expression.cc`).
The second check only bites when a random deviate is involved — a normal
deviate used as a failure rate has a domain reaching six sigma below its
mean, and upstream rejects it however comfortable the mean looks.

```rust
pub struct Interval {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn closed(lower: f64, upper: f64) -> Self { /* ... */ }
  ```
  `[lower, upper]`.

- ```rust
  pub fn left_open(lower: f64, upper: f64) -> Self { /* ... */ }
  ```
  `(lower, upper]`.

- ```rust
  pub fn point(value: f64) -> Self { /* ... */ }
  ```
  The degenerate interval `[value, value]`, which is upstream's default

- ```rust
  pub fn lower(self: &Self) -> f64 { /* ... */ }
  ```
  The lower bound.

- ```rust
  pub fn upper(self: &Self) -> f64 { /* ... */ }
  ```
  The upper bound.

- ```rust
  pub fn contains(self: &Self, value: f64) -> bool { /* ... */ }
  ```
  Whether `value` lies in the interval — upstream's `Contains`.

- ```rust
  pub fn within(self: &Self, other: &Interval) -> bool { /* ... */ }
  ```
  Whether this interval lies entirely inside `other` — upstream's

- ```rust
  pub fn is_non_negative(self: &Self) -> bool { /* ... */ }
  ```
  Upstream's `IsNonNegative`: **the lower bound alone**, open or not.

- ```rust
  pub fn is_positive(self: &Self) -> bool { /* ... */ }
  ```
  Upstream's `IsPositive`: non-negative and not containing zero.

- ```rust
  pub fn is_probability(self: &Self) -> bool { /* ... */ }
  ```
  Upstream's `IsProbability`: within `[0, 1]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut core::fmt::Formatter<''_>) -> core::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `ensure_probability`

Upstream `EnsureProbability` (`src/expression.cc`).

Two checks, not one: the expression's **value** must be a probability, and
so must its whole [`Interval`]. The second only bites when a random deviate
is involved, and it is the reason [`Expression::interval`] exists.

# Errors

[`RafflesError::InvalidParameter`] naming which of the two failed.

```rust
pub fn ensure_probability(expression: &Expression, parameters: &Parameters, mission_time: f64, what: &str) -> crate::Result<()> { /* ... */ }
```

#### Function `resolve_parameter_values`

Every parameter's **value** — upstream's `Parameter::value()`, which is
its expression's value with each deviate at its mean, never a sample.

Parameters may name other parameters, so this resolves them in rounds until
nothing new resolves. The result does not depend on the map's iteration
order: each value is a deterministic function of the values it names.

# Errors

[`RafflesError::InvalidParameter`] naming the parameters that never
resolve: one that names an undefined parameter, or a cycle, which upstream
rejects when it validates the model.

```rust
pub fn resolve_parameter_values(definitions: &std::collections::HashMap<String, Expression>, mission_time: f64) -> crate::Result<Parameters> { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_MISSION_TIME`

SCRAM's default mission time, in hours.

Upstream `Settings::mission_time_`; a model that does not set one gets
this, and `<system-mission-time/>` resolves to it.

```rust
pub const DEFAULT_MISSION_TIME: f64 = 8760.0;
```

## Module `fault_tree`

The fault tree itself — gates, their logic, and what feeds them.

A fault tree is a Boolean expression written upside down: the **top event**
is the system failure being studied, and it is decomposed through gates
until the leaves are **basic events** whose probabilities are known.

Build one with [`FaultTreeBuilder`], which works in names and hands back
both the tree and the basic-event probability vector that
[`super::probability`] and [`super::importance`] expect — the indices line
up by construction, which is the mistake that would otherwise be easiest to
make.

```
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};

// Two redundant trains, each of which fails if its valve or its pump does.
let mut b = FaultTreeBuilder::new();
b.basic_event("ValveOne", 0.5).unwrap();
b.basic_event("PumpOne", 0.7).unwrap();
b.basic_event("ValveTwo", 0.5).unwrap();
b.basic_event("PumpTwo", 0.7).unwrap();
b.gate("TrainOne", Connective::Or, &["ValveOne", "PumpOne"]).unwrap();
b.gate("TrainTwo", Connective::Or, &["ValveTwo", "PumpTwo"]).unwrap();
b.gate("TopEvent", Connective::And, &["TrainOne", "TrainTwo"]).unwrap();
let model = b.build("TopEvent").unwrap();

assert_eq!(model.probabilities(), &[0.5, 0.7, 0.5, 0.7]);
```

```rust
pub mod fault_tree { /* ... */ }
```

### Types

#### Enum `Connective`

The Boolean logic a gate applies to its arguments.

Mirrors upstream SCRAM's `pdag.h` `Connective` enum. All eight are
analysed; the four negating ones make the tree **non-coherent**, which
changes what its cut sets mean — see [`Connective::is_coherent`].

```rust
pub enum Connective {
    And,
    Or,
    Atleast {
        min: usize,
    },
    Xor,
    Not,
    Nand,
    Nor,
    Null,
}
```

##### Variants

###### `And`

All arguments must occur. Upstream `kAnd`.

###### `Or`

Any one argument suffices. Upstream `kOr`.

###### `Atleast`

At least `min` of the arguments must occur — the K-of-N, voting or
combination gate. Upstream `kAtleast`, whose threshold lives in a
separate `min_number` field; folding it into the variant makes an
at-least gate without a threshold unrepresentable.

`min` must satisfy `1 <= min <= args.len()`. `min == 1` is an
[`Connective::Or`] and `min == args.len()` is an [`Connective::And`];
both are accepted rather than rewritten, because upstream accepts them
and rewriting would make a round trip lossy.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `min` | `usize` | How many arguments must occur. |

###### `Xor`

Exactly one of two arguments. Upstream `kXor`. **Non-coherent.**

###### `Not`

Negation of a single argument. Upstream `kNot`. **Non-coherent.**

###### `Nand`

Negation of [`Connective::And`]. Upstream `kNand`. **Non-coherent.**

###### `Nor`

Negation of [`Connective::Or`]. Upstream `kNor`. **Non-coherent.**

###### `Null`

Pass-through of a single argument, with no logic. Upstream `kNull`.

Not the empty set — it exists because the Model Exchange Format lets a
gate stand for another event (a "transfer" symbol), and dropping it
would change the tree's shape.

##### Implementations

###### Methods

- ```rust
  pub fn is_coherent(self: &Self) -> bool { /* ... */ }
  ```
  Whether this connective is **coherent** — monotone, so that a basic

- ```rust
  pub fn as_str(self: &Self) -> &'static str { /* ... */ }
  ```
  Upstream's own name for this connective, as it appears in a SCRAM input

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Arg`

What feeds a gate: another gate, a basic event, or a constant.

The first two carry an index, not a name — gate indices into
[`FaultTree::gates`], basic-event indices into the probability slice.
[`FaultTreeBuilder`] does the name resolution so a caller need not.

```rust
pub enum Arg {
    Gate(usize),
    BasicEvent(usize),
    Constant(bool),
}
```

##### Variants

###### `Gate`

An intermediate gate, by index.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `BasicEvent`

A basic event, by index into the probability slice.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Constant`

A **house event** — a condition fixed for the analysis rather than
sampled, which the Model Exchange Format writes as
`<constant value="true"/>`.

House events model the configuration a study is run in: a valve lined
up or not, a timer reset or not. They have no probability and never
appear in a cut set; they only decide which parts of the tree are
live. `ThreeMotor`'s three timer-reset conditions are the fixture's
example, and all three are `true`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `bool` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Gate`

One gate: a connective and the arguments it applies to.

```rust
pub struct Gate {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn connective(self: &Self) -> Connective { /* ... */ }
  ```
  The Boolean logic this gate applies.

- ```rust
  pub fn args(self: &Self) -> &[Arg] { /* ... */ }
  ```
  What feeds this gate, in the order it was declared.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FaultTree`

A validated fault tree.

Constructing one guarantees, once and for all, that every index is in
range, every gate's arity suits its connective, and the graph is acyclic.
[`super::mocus`] therefore does not re-check any of that and cannot loop
forever on a cyclic tree.

```rust
pub struct FaultTree {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn gates(self: &Self) -> &[Gate] { /* ... */ }
  ```
  Every gate, indexed as [`Arg::Gate`] refers to them.

- ```rust
  pub fn basic_event_count(self: &Self) -> usize { /* ... */ }
  ```
  How many distinct basic events the tree refers to.

- ```rust
  pub fn has_house_events(self: &Self) -> bool { /* ... */ }
  ```
  Whether any gate is fed by a [`Arg::Constant`] house event.

- ```rust
  pub fn top(self: &Self) -> usize { /* ... */ }
  ```
  Index of the top-event gate.

- ```rust
  pub fn is_coherent(self: &Self) -> bool { /* ... */ }
  ```
  Whether every gate is coherent, so the tree has no complemented

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FaultTreeModel`

A fault tree, its basic-event probabilities, and the names they came from.

This is what [`FaultTreeBuilder::build`] returns. The probability vector is
indexed exactly as [`Arg::BasicEvent`] and [`super::probability::CutSet`]
members are, so it can be handed straight to
[`super::probability::top_event_probability`].

```rust
pub struct FaultTreeModel {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn tree(self: &Self) -> &FaultTree { /* ... */ }
  ```
  The validated tree.

- ```rust
  pub fn probabilities(self: &Self) -> &[f64] { /* ... */ }
  ```
  Basic-event probabilities, indexed as cut-set members are.

- ```rust
  pub fn basic_event_names(self: &Self) -> &[String] { /* ... */ }
  ```
  Basic-event names, in index order.

- ```rust
  pub fn gate_names(self: &Self) -> &[String] { /* ... */ }
  ```
  Gate names, in index order.

- ```rust
  pub fn basic_event_index(self: &Self, name: &str) -> Option<usize> { /* ... */ }
  ```
  Index of a basic event by name, or `None` if the tree has no such

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FaultTreeBuilder`

Builds a [`FaultTreeModel`] from named gates and basic events.

Declaration order does not matter: a gate may name arguments that have not
been declared yet, and everything is resolved and validated in
[`FaultTreeBuilder::build`]. That is deliberate — a fault tree is normally
written top-down, and requiring bottom-up declaration would make it
tedious to transcribe one.

A name that is never declared as either a gate or a basic event is an
error, not an implicit basic event. Silently inventing a leaf is how a
typo becomes a wrong answer.

```rust
pub struct FaultTreeBuilder {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  A builder with no gates and no basic events.

- ```rust
  pub fn basic_event(self: &mut Self, name: &str, probability: f64) -> Result<usize> { /* ... */ }
  ```
  Declares a basic event and its probability of occurrence.

- ```rust
  pub fn house_event(self: &mut Self, name: &str, value: bool) -> Result<()> { /* ... */ }
  ```
  Declares a **house event** — a condition fixed for the analysis.

- ```rust
  pub fn gate(self: &mut Self, name: &str, connective: Connective, args: &[&str]) -> Result<usize> { /* ... */ }
  ```
  Declares a gate, its connective, and the names of its arguments.

- ```rust
  pub fn build(self: Self, top: &str) -> Result<FaultTreeModel> { /* ... */ }
  ```
  Resolves every name, validates the structure, and returns the model.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `importance`

Which basic events matter — the five standard importance measures.

A top-event probability says how likely failure is. It does not say what to
fix. These five measures rank the basic events, and they rank them
*differently* because they ask different questions — see
[`ImportanceFactors`].

```rust
pub mod importance { /* ... */ }
```

### Types

#### Struct `ImportanceFactors`

The five importance measures for one basic event, plus its occurrence
count.

All five are dimensionless. They are **not** interchangeable rankings: a
component can be top by one measure and unremarkable by another, which is
the reason PRA reports all of them rather than picking one.

```rust
pub struct ImportanceFactors {
    pub occurrence: usize,
    pub mif: f64,
    pub cif: f64,
    pub dif: f64,
    pub raw: f64,
    pub rrw: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `occurrence` | `usize` | How many cut sets contain this event.<br><br>A purely structural count — it ignores probabilities entirely. Upstream<br>calls it `occurrence`.<br><br>**Zero when the factors came from [`importance_factors_from_bdd`]**,<br>which never builds cut sets and so has nothing to count. Reported as<br>zero rather than guessed at; use [`importance_factors`], or<br>[`super::zbdd::minimal_cut_sets`], if the count is what you need. |
| `mif` | `f64` | **Birnbaum marginal importance factor** — `P(top | event) - P(top | not<br>event)`.<br><br>The sensitivity of the top-event probability to this event: how much<br>the answer moves between the event being certain and impossible. It<br>does **not** depend on the event's own probability, which is why a very<br>reliable component can still have a large MIF. |
| `cif` | `f64` | **Critical importance factor** — `p * MIF / p_total`.<br><br>The fraction of top-event probability attributable to this event being<br>critical. Unlike MIF this *does* weight by the event's own probability,<br>so it answers "where is the risk actually coming from". Upstream:<br>`imp.cif = p_var * imp.mif / p_total;` |
| `dif` | `f64` | **Fussell-Vesely diagnosis importance factor** — `p * RAW`.<br><br>Upstream computes it exactly this way:<br>`imp.dif = p_var * imp.raw;`. Note this is SCRAM's definition and is<br>ported as such; other PRA codes define Fussell-Vesely as the fraction<br>of top-event probability from cut sets containing the event, which is<br>numerically different. **If you are comparing against another tool,<br>check which definition it uses before concluding anything disagrees.** |
| `raw` | `f64` | **Risk achievement worth** — `1 + (1 - p) * MIF / p_total`.<br><br>How much worse the top event gets if this event is made certain. A<br>large RAW marks a component whose continued reliability is load-bearing<br>— the argument for maintaining it. Upstream:<br>`imp.raw = 1 + (1 - p_var) * imp.mif / p_total;` |
| `rrw` | `f64` | **Risk reduction worth** — `p_total / (p_total - p * MIF)`.<br><br>How much better the top event gets if this event is made impossible.<br>A large RRW marks a component worth improving — the argument for<br>investing in it. RRW is bounded below by 1 by construction.<br><br>**This is the one field that deliberately disagrees with upstream, and<br>only at the singularity.** The denominator vanishes exactly when the<br>event lies in *every* cut set — a single point of failure — because<br>then `P(top | not event) = 0`, removing the event removes all risk, and<br>the ratio diverges. This port returns [`f64::INFINITY`], which states<br>that. SCRAM guards the division with exact float equality<br>(`if (p_total != p_var * imp.mif)`) and, when it holds, leaves `rrw` at<br>its zero-initialised value — so **upstream reports `RRW = 0`**, a value<br>RRW cannot otherwise take.<br><br>The guard is also why this port cannot copy it: upstream lands on the<br>singular point exactly only because its BDD traversal happens to<br>produce bit-identical values. Computing the same quantity another way<br>lands a few ulp off, falls through `!=`, and divides by ~1e-19. That<br>was measured, not hypothesised — `Theatre/theatre` `Mains_Fail` gave<br>**-4.77e15** before the tolerance guard replaced the equality test.<br>See [`SINGULARITY_TOLERANCE`] and<br>`tests/scram_oracle_suite.rs::rrw_diverges_from_upstream_only_at_the_singularity`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `importance_factors`

Computes the five importance measures for one basic event.

`event` indexes into `event_probabilities`, as cut-set members do.
`approximation` selects how the top-event probability is quantified;
**use [`Approximation::Exact`] to reproduce upstream SCRAM**, whose
importance analysis runs on the exact BDD value.

# Errors

[`RafflesError::InvalidParameter`] if `event` is out of range, if the cut
sets or probabilities are invalid (see
[`super::probability::cut_set_probability`]), or if the total probability
is zero — every factor divides by it, so the ranking is undefined rather
than infinite.

```rust
pub fn importance_factors(event: usize, cut_sets: &[super::probability::CutSet], event_probabilities: &[f64], approximation: super::probability::Approximation) -> crate::Result<ImportanceFactors> { /* ... */ }
```

#### Function `importance_factors_from_bdd`

Computes the five importance measures from a **binary decision diagram**.

Same measures as [`importance_factors`], same derived formulas — but the
three probabilities they rest on come from
[`super::bdd::Bdd::probability`] rather than from inclusion-exclusion over
cut sets. That makes three differences, and the third is the reason this
exists:

1. **No cut-set ceiling.** [`importance_factors`] inherits
   [`super::probability::EXACT_CUT_SET_LIMIT`]; this does not.
2. **No cut sets needed at all**, so a model whose cut sets are
   intractable can still be ranked.
3. **On a non-coherent tree it is the RIGHT ANSWER, and the cut-set
   version is not.** Minimal cut sets there are conservative, and every
   factor derived from them inherits that. Measured on the fixture's small
   non-coherent model, event `a`: this gives the Birnbaum factor
   `0.432`, matching SCRAM exactly, where [`importance_factors`] over the
   same tree's cut sets gives `0.420`. Upstream computes importance from
   its BDD for precisely this reason.

On a **coherent** tree the two agree exactly, which is asserted in
`tests/scram_bdd_oracle.rs`.

`event` is a basic-event index, as for [`importance_factors`].

# Errors

[`RafflesError::InvalidParameter`] if `event` is out of range, if a
probability is invalid, or if the top-event probability is zero — every
factor divides by it.

# Example

```
use raffles::scram::bdd::Bdd;
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
use raffles::scram::importance::importance_factors_from_bdd;

// `a AND NOT b` OR `c` -- non-coherent, so cut sets would be conservative.
let mut b = FaultTreeBuilder::new();
b.basic_event("a", 0.1).unwrap();
b.basic_event("b", 0.2).unwrap();
b.basic_event("c", 0.3).unwrap();
b.gate("NotB", Connective::Not, &["b"]).unwrap();
b.gate("Left", Connective::And, &["a", "NotB"]).unwrap();
b.gate("Top", Connective::Or, &["Left", "c"]).unwrap();
let model = b.build("Top").unwrap();

let bdd = Bdd::build(model.tree()).unwrap();
let f = importance_factors_from_bdd(0, &bdd, model.probabilities()).unwrap();
// P(top | a) - P(top | not a) = (0.8 + 0.3 - 0.24) - 0.3 = 0.56
assert!((f.mif - 0.56).abs() < 1e-12);
```

```rust
pub fn importance_factors_from_bdd(event: usize, bdd: &super::bdd::Bdd, event_probabilities: &[f64]) -> crate::Result<ImportanceFactors> { /* ... */ }
```

### Constants and Statics

#### Constant `SINGULARITY_TOLERANCE`

Relative width of the band around the RRW singularity treated as exactly
singular.

`RRW = p_total / (p_total - p * MIF)` diverges when the event sits in every
cut set. Upstream tests that denominator for exact equality with zero;
this port cannot, because it computes both quantities by a different route
and lands within a few ulp rather than on the point. `1e-12` is far wider
than the ulp-scale noise and far narrower than any physically meaningful
denominator.

```rust
pub const SINGULARITY_TOLERANCE: f64 = 1e-12;
```

## Module `mef`

Reading SCRAM's Model Exchange Format.

Until this existed, upstream's models reached the tests through a shell
script that scraped a flat subset of the XML, and two things were out of
reach because of it: `<xi:include>`, and `<define-component>`'s private
namespaces. The second is why `ThreeMotor/three_motor` — the one model in
the fixture that no analysis route could cover — was excluded: it declares
`E1` twice, once at the top and once inside a private component, and a
flat parser silently merges them into a wrong tree.

# What this reads

`<define-fault-tree>` and `<define-component>`, `<define-gate>`,
`<define-basic-event>`, `<define-house-event>`, `<define-parameter>`,
`<define-CCF-group>`, `<define-substitution>`, `<define-alignment>`,
`<model-data>`, all eleven MEF connectives, event references (`<event>`,
`<gate>`, `<basic-event>`, `<house-event>`, and `<event type="…">`),
`<not>` and `<constant>` arguments, and the expression elements
[`super::expression`] evaluates.

**Formulas do not nest**, and that is a property of the format rather than
a limit of this reader: upstream's grammar (`share/input.rng`) lets a
connective take only event references, `<not>` around one event, or a
`<constant>`. See [`Formula`].

# What it does not

Event trees and extern functions.
**An element it does not recognise is an error**, never a skip — a model
using one would otherwise be read as a smaller, different model that
happens to parse, which is the failure this whole port exists to avoid.

The remaining expression elements — the trigonometric functions,
`<switch>`, the test-event conditions and `<extern-function>` calls — are
refused for the same reason. None appears in any of upstream's own input
models.

There is also **no schema validation**. Upstream validates against a
RelaxNG grammar before looking at anything; this checks only what it needs
to build the model. A document SCRAM would reject may be read here.

```rust
pub mod mef { /* ... */ }
```

### Types

#### Struct `Element`

A parsed XML element: name, attributes, children, and any text.

A deliberately small DOM. The pull parser gives events; the MEF walk wants
to look at a subtree more than once (an expression's arity decides which
formula it is), so the events are collected first.

```rust
pub struct Element {
    pub name: String,
    pub attributes: std::collections::HashMap<String, String>,
    pub children: Vec<Element>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Local name, with any namespace prefix stripped. |
| `attributes` | `std::collections::HashMap<String, String>` | Attributes, by local name. |
| `children` | `Vec<Element>` | Child elements, in document order. |

##### Implementations

###### Methods

- ```rust
  pub fn attr(self: &Self, name: &str) -> Option<&str> { /* ... */ }
  ```
  The value of an attribute, or `None`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `MefConnective`

The Model Exchange Format's connective set — upstream's `mef::Connective`.

The first eight are the analysis connectives, in upstream's own order, and
map one-to-one onto [`Connective`]. The last three are, in upstream's
words, "rarely used connectives specific to the MEF": they have no PDAG
representation and `Pdag::ConstructComplexGate` rewrites each into the
first eight. This port keeps them as read and applies the same rewrite in
[`MefModel::fault_tree`], so a model using them builds the tree upstream
would have built.

```rust
pub enum MefConnective {
    And,
    Or,
    Atleast {
        min: usize,
    },
    Xor,
    Not,
    Nand,
    Nor,
    Null,
    Iff,
    Imply,
    Cardinality {
        min: usize,
        max: usize,
    },
}
```

##### Variants

###### `And`

`<and>`.

###### `Or`

`<or>`.

###### `Atleast`

`<atleast min="k">` — the K-of-N vote.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `min` | `usize` | How many arguments must occur. |

###### `Xor`

`<xor>`, two arguments.

###### `Not`

`<not>`, one argument.

###### `Nand`

`<nand>`.

###### `Nor`

`<nor>`.

###### `Null`

A pass-through: a gate that names another event directly.

###### `Iff`

`<iff>`, two arguments — equality.

###### `Imply`

`<imply>`, two arguments — `a` implies `b`.

###### `Cardinality`

`<cardinality min="j" max="k">` — between `j` and `k` arguments occur.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `min` | `usize` | Fewest arguments that may occur. |
| `max` | `usize` | Most arguments that may occur. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Formula`

A gate's Boolean formula.

**MEF formulas do not nest.** Upstream's RelaxNG grammar (`share/input.rng`,
`define name="formula"`) admits a connective whose arguments are each an
`argument`, and an `argument` is an event reference, a `<not>` around one
event reference, or a `<constant>` — never another connective. An earlier
revision of this reader carried machinery for arbitrarily nested formulas
and said so in its module doc; that was **wrong about the format**, and the
claim is corrected here rather than left standing. What does exist is the
`<not>` wrapper, which upstream records as a *complement flag on the
argument* and this port lowers into an anonymous negating gate.

```rust
pub struct Formula {
    pub connective: MefConnective,
    pub args: Vec<FormulaArg>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `connective` | `MefConnective` | The connective. |
| `args` | `Vec<FormulaArg>` | Its arguments, in document order. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `FormulaArg`

One argument of a [`Formula`] — upstream's `Formula::Arg`.

```rust
pub enum FormulaArg {
    Event {
        name: String,
        complement: bool,
    },
    Constant(bool),
}
```

##### Variants

###### `Event`

A reference to a declared event, by resolved id, optionally negated by
a `<not>` wrapper.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The `Id::id()` of the declaration this reference resolves to. |
| `complement` | `bool` | Whether a `<not>` wrapped it. |

###### `Constant`

`<constant value="true|false"/>`. Upstream substitutes its
`HouseEvent::kTrue` / `kFalse` singletons.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `bool` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MefModel`

A parsed MEF model.

```rust
pub struct MefModel {
    pub name: Option<String>,
    pub mission_time: f64,
    pub parameters: std::collections::HashMap<String, super::expression::Expression>,
    pub basic_events: std::collections::HashMap<String, super::expression::Expression>,
    pub house_events: std::collections::HashMap<String, bool>,
    pub gates: std::collections::HashMap<String, Formula>,
    pub gate_order: Vec<String>,
    pub ccf_groups: Vec<super::ccf::CcfGroup>,
    pub alignments: Vec<super::alignment::Alignment>,
    pub substitutions: Vec<super::substitution::Substitution>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `Option<String>` | The `name` attribute of `<opsa-mef>`, if any. |
| `mission_time` | `f64` | Mission time, defaulting to [`DEFAULT_MISSION_TIME`]. |
| `parameters` | `std::collections::HashMap<String, super::expression::Expression>` | Parameter expressions, by resolved name. |
| `basic_events` | `std::collections::HashMap<String, super::expression::Expression>` | Basic-event expressions, by resolved name. |
| `house_events` | `std::collections::HashMap<String, bool>` | House-event constants, by resolved name. |
| `gates` | `std::collections::HashMap<String, Formula>` | Gate formulas, by resolved name. |
| `gate_order` | `Vec<String>` | Declaration order of the gates, so a model reads back deterministically. |
| `ccf_groups` | `Vec<super::ccf::CcfGroup>` | Common-cause failure groups, in declaration order.<br><br>[`MefModel::fault_tree`] applies them; [`MefModel::without_ccf`] is<br>the explicit ablation. |
| `alignments` | `Vec<super::alignment::Alignment>` | Alignments, in declaration order.<br><br>A model with an alignment has **no single answer**: it is analysed once<br>per phase. [`MefModel::in_phase`] returns the model as it stands in one. |
| `substitutions` | `Vec<super::substitution::Substitution>` | Substitutions, in declaration order.<br><br>The **declarative** ones are applied by [`MefModel::fault_tree`], as a<br>tree rewrite. The **non-declarative** ones are a pass over the<br>generated products, which the caller runs with<br>[`MefModel::product_substitutions`] and<br>[`super::substitution::apply_to_products`]. |

##### Implementations

###### Methods

- ```rust
  pub fn from_file(path: &Path) -> Result<Self> { /* ... */ }
  ```
  Reads a model from a file, resolving `<xi:include>` relative to it.

- ```rust
  pub fn from_element(root: &Element) -> Result<Self> { /* ... */ }
  ```
  Reads a model from an already-parsed `<opsa-mef>` element.

- ```rust
  pub fn resolved_parameters(self: &Self) -> Result<Parameters> { /* ... */ }
  ```
  Every parameter's value, resolved by repeated passes.

- ```rust
  pub fn without_ccf(self: &Self) -> MefModel { /* ... */ }
  ```
  This model with its common-cause groups **dropped** — the explicit

- ```rust
  pub fn apply_ccf(self: &Self) -> Result<MefModel> { /* ... */ }
  ```
  This model with every common-cause group applied — upstream's

- ```rust
  pub fn apply_declarative_substitutions(self: &Self, top: &str) -> Result<MefModel> { /* ... */ }
  ```
  This model with its **declarative** substitutions applied to the tree

- ```rust
  pub fn product_substitutions(self: &Self, tree: &FaultTreeModel) -> Result<Vec<ProductSubstitution>> { /* ... */ }
  ```
  The **non-declarative** substitutions, in the basic-event indices of

- ```rust
  pub fn basic_event_expressions(self: &Self, tree: &FaultTreeModel) -> Result<Vec<Expression>> { /* ... */ }
  ```
  The **expression** behind each basic event of `tree`, in its index

- ```rust
  pub fn in_phase(self: &Self, alignment: &str, phase: &str) -> Result<MefModel> { /* ... */ }
  ```
  This model as it stands in one phase of one alignment — upstream's

- ```rust
  pub fn phases(self: &Self) -> Vec<(&str, &str)> { /* ... */ }
  ```
  Every `(alignment, phase)` pair the model declares, in declaration

- ```rust
  pub fn fault_tree(self: &Self, top: &str) -> Result<FaultTreeModel> { /* ... */ }
  ```
  Flattens the model into a [`FaultTreeModel`] rooted at `top`.

- ```rust
  pub fn top_gates(self: &Self) -> Vec<String> { /* ... */ }
  ```
  Every gate no other gate references — the candidate top events.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `parse_xml`

**Attributes:**

- `Other("#[attr = CfgTrace([NameValue { name: \"feature\", value: Some(\"std\"), span: crates/raffles/src/scram/mef.rs:152:7: 152:22 (#0) }])]")`

Parses an XML document into [`Element`]s.

Namespace prefixes are stripped: MEF uses one vocabulary and the only
prefixed element in upstream's suite is `<xi:include>`, which is resolved
before this sees it.

```rust
pub fn parse_xml(source: &str) -> crate::Result<Element> { /* ... */ }
```

### Constants and Statics

#### Constant `CONSTANT_TRUE`

The reserved house event standing in for `<constant value="true"/>`.

Angle brackets cannot occur in a MEF name, so this can never collide with a
declaration.

```rust
pub const CONSTANT_TRUE: &str = "<constant:true>";
```

#### Constant `CONSTANT_FALSE`

The reserved house event standing in for `<constant value="false"/>`.

```rust
pub const CONSTANT_FALSE: &str = "<constant:false>";
```

## Module `mocus`

Minimal cut sets from a fault tree — the MOCUS algorithm.

**This is NOT a port of SCRAM's `src/mocus.cc`, and it carries no
attribution header for that reason.** Upstream's MOCUS is a 108-line driver
over a ZBDD `CutSetContainer` operating on a `Pdag` that a 2,400-line
preprocessor has already rewritten; porting it means porting ZBDD, the
Boolean graph and the preprocessor — roughly 8,000 lines — and none of that
is here. What this module implements is the **classical top-down MOCUS
expansion** from the published literature:

- J. B. Fussell and W. E. Vesely, "A new methodology for obtaining cut sets
  for fault trees", *Transactions of the American Nuclear Society* **15**
  (1972), 262-263.
- A. Rauzy, "New algorithms for fault trees analysis", *Reliability
  Engineering & System Safety* **40**(3) (1993), 203-211,
  [doi:10.1016/0951-8320(93)90060-C](https://doi.org/10.1016/0951-8320(93)90060-C)
  — for why a BDD/ZBDD formulation supersedes this one on large models.

It is nonetheless **verified against upstream**: the cut sets it produces
are compared, set for set, against the products SCRAM reports for its own
input models. That comparison is worth more than a translation would be,
precisely because the two algorithms are unrelated — see
`crates/raffles/docs/scram-port-verification.md` and
`tests/scram_mocus_oracle.rs`.

# What it does and does not handle

**All eight connectives**, coherent and not. A negated gate is expanded
through its De Morgan dual, so complemented literals appear during
expansion; a partial set containing both a literal and its complement is
impossible and is dropped. At the end the negative literals are deleted and
the result minimised by absorption — which is what upstream does too, in
ZBDD form: `Zbdd::EliminateComplement` OR-merges the two branches of a
negative-index node (deleting the literal) and `Zbdd::Minimize` absorbs.

**Minimal cut sets of a non-coherent tree are CONSERVATIVE**, and that is a
property of the definition, not of this implementation. Deleting negative
literals discards the information that some failure combinations require a
component to be *working*, so the cut sets describe a function that is
everywhere at least as large as the real one. Quantifying them gives an
**upper bound** on the top-event probability, not the probability. Use
prime implicants if you need the exact function — upstream has them behind
`--prime-implicants`, and they are not ported.

**It is exponential in the worst case**, which is why Rauzy's paper exists.
[`minimal_cut_sets`] takes an order limit for that reason, and it is the
same knob upstream calls `limit_order`.

**[`super::zbdd::minimal_cut_sets`] computes the same answer and is not
exponential in the same way.** It is the one to reach for on anything
sizeable: on the fixture's `Aralia/das9601` this module exhausts its
five-million-state ceiling at every order limit and the ZBDD finishes in
under a second. This module is kept because it is an unrelated second
route to the same sets — the two are checked against each other — and
because it is far easier to follow when a disagreement has to be
diagnosed.

**Upstream has a probability cut-off setting, and it does nothing.**
`Settings::cut_off_` defaults to `1e-8`, is settable from the CLI
(`--cut-off`) and from a project file, and is range-validated on the way
in — but its getter `Settings::cut_off()` has **no callers anywhere in
SCRAM 0.16.2**, so no product is ever discarded by probability. Checked by
running it, not only by reading: `--cut-off 0.5` on `Aralia/chinese`, whose
top-event probability is `1.17e-3`, leaves all 392 products in place and
the total unchanged.

So there is nothing to port here, and truncating by order only is not a
divergence from upstream. Stated because the opposite is the natural
assumption from reading `settings.h`.

```rust
pub mod mocus { /* ... */ }
```

### Functions

#### Function `minimal_cut_sets`

Generates the minimal cut sets of a fault tree.

A **cut set** is a set of basic events whose joint occurrence causes the
top event; it is **minimal** if no proper subset of it is also a cut set.
The minimal cut sets are the complete enumeration of how the system fails,
and are what [`super::probability::top_event_probability`] and
[`super::importance::importance_factors`] consume.

`limit_order` discards any cut set with more than that many basic events;
pass [`DEFAULT_LIMIT_ORDER`] to match upstream SCRAM's default. Truncation
is a real approximation — the discarded sets contribute probability — so
lower it deliberately, not for speed alone.

The returned cut sets are sorted by order and then lexicographically by
member index, so two runs on the same tree compare equal.

# Errors

- [`RafflesError::InvalidParameter`] if `limit_order` is zero, or if the
  expansion exceeds [`EXPANSION_LIMIT`] intermediate states.

# Example

```
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
use raffles::scram::mocus::{minimal_cut_sets, DEFAULT_LIMIT_ORDER};

let mut b = FaultTreeBuilder::new();
b.basic_event("ValveOne", 0.5).unwrap();
b.basic_event("PumpOne", 0.7).unwrap();
b.basic_event("ValveTwo", 0.5).unwrap();
b.basic_event("PumpTwo", 0.7).unwrap();
b.gate("TrainOne", Connective::Or, &["ValveOne", "PumpOne"]).unwrap();
b.gate("TrainTwo", Connective::Or, &["ValveTwo", "PumpTwo"]).unwrap();
b.gate("TopEvent", Connective::And, &["TrainOne", "TrainTwo"]).unwrap();
let model = b.build("TopEvent").unwrap();

// Two redundant trains: every failure needs one event from each.
let cut_sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
assert_eq!(cut_sets.len(), 4);
assert!(cut_sets.iter().all(|c| c.order() == 2));
```

```rust
pub fn minimal_cut_sets(tree: &super::fault_tree::FaultTree, limit_order: usize) -> crate::Result<Vec<super::probability::CutSet>> { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_LIMIT_ORDER`

Default cap on the **order** of a generated cut set — how many basic events
it may contain.

20, matching upstream SCRAM's `Settings::limit_order_` default, so a
comparison against SCRAM's reported products is like for like unless the
caller changes both.

```rust
pub const DEFAULT_LIMIT_ORDER: usize = 20;
```

#### Constant `EXPANSION_LIMIT`

Ceiling on how many partially-expanded cut sets may be alive at once.

MOCUS is exponential, and on a real PRA model it does not merely get slow —
it exhausts memory. Hitting this returns an error naming the limit rather
than letting the process be killed, which is the difference between a
diagnosable failure and a disappearing job.

```rust
pub const EXPANSION_LIMIT: usize = 5_000_000;
```

## Module `pba`

# pba.r operations on RAFFLES' [`Pbox`]

The p-box logic needed to propagate **confidence boxes** through Boolean
gates: [`kn`] (the binomial c-box as pba.r builds it), the dependence-free
[`and_unknown`] / [`or_unknown`] (Fréchet bounds on the *uncertain
probabilities*), and the independence versions [`and_independent`] /
[`or_independent`].

## Two dependence questions, again

The Boolean gate has a dependence between the **events** (do `A` and `B`
fail together?), and the p-boxes carry a dependence between the
**uncertainties** in `P(A)` and `P(B)`. pba.r's `and`/`or` make no
assumption about either: `and(a, b)` is the envelope of
`max(a + b - 1, 0)` and `min(a, b)`, each combined with Fréchet
convolution. `andI`/`orI` assume both are independent. This is the
convention of Lye et al. (2024) and the scripts; it is *not* the
matched-confidence-level (comonotone) combination of
[`crate::imprecise::SystemStructure::reliability_pbox`].

## Fréchet convolution on a quantile ladder

For a non-decreasing `op` (sum, min, max) and operands with quantile
bounds `lower`, `upper` on the ladder `p_i = i/n`:

```text
upper_z[i] = min over a + b = n + i of  op(upper_x[a], upper_y[b])
lower_z[m] = max over a + b = m     of  op(lower_x[a], lower_y[b])
```

which is the Williamson–Downs bound restricted to the ladder, exactly what
`frechetconv.pbox` loops over. Restricting to grid pairs keeps it a valid
(possibly not the tightest) bound.

No transcendental functions are used here, so nothing is routed through
PETIR (gh:#985): the code is `+ - * min max` and a sort.

## References

- S. Ferson, pba.r, <https://github.com/ScottFerson/pba.r> (GPL-3.0).
- A. Lye, W. Vechgama, M. Sallak, S. Destercke, S. Ferson, S. Xiao (2024),
  *ASCE-ASME J. Risk Uncertainty Eng. Syst. A* 11, 04024074,
  doi:10.1061/AJRUA6/RUENG-1380.
- R. C. Williamson and T. Downs (1990). Probabilistic arithmetic I.
  *Int. J. Approximate Reasoning* 4, 89-158.

```rust
pub mod pba { /* ... */ }
```

### Types

#### Enum `MonotoneOp`

The monotone binary operations the Fréchet convolution is used with.

```rust
pub enum MonotoneOp {
    Add,
    Min,
    Max,
}
```

##### Variants

###### `Add`

`x + y`

###### `Min`

`min(x, y)` — pba.r's `pmin`

###### `Max`

`max(x, y)` — pba.r's `pmax`

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `point`

A precise (zero-width) p-box holding one number — pba.r's `pbox(x)`.

```rust
pub fn point(value: f64, levels: usize) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `clamp_pbox`

Clamps both quantile series into `[lo, hi]`.

```rust
pub fn clamp_pbox(a: &crate::imprecise::Pbox, lo: f64, hi: f64) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `kn_exact`

The Clopper-Pearson c-box with the **exact support ends**, no `1e-6` clamp:
the two edge distributions pba.r's `Sbeta(k, n-k+1)` and
`Sbeta(k+1, n-k)` give (what the Case study 4 script calls `c_L` and
`c_R`, built with `beta()` and so unclamped).

# Errors

As [`cbox_binomial`].

```rust
pub fn kn_exact(k: usize, n: usize, levels: usize) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `kn`

The binomial confidence box exactly as pba.r's `KN(k, n)` builds it.

**Is it [`cbox_binomial`]?** In the interior, yes: the same
`Beta(k, n-k+1)` / `Beta(k+1, n-k)` quantile ladders. Two differences
were found by reading `KN` and are reproduced here: (1) pba.r takes the
*exact* support ends (`qbeta(0) = 0`, `qbeta(1) = 1`) where
[`cbox_binomial`] evaluates the quantile at `1e-9` and `1 - 1e-9`, which
for `Beta(23, 2)` is about `0.3`, not `0`; and (2) pba.r clamps the
result to `[1e-6, 1 - 1e-6]`. This is [`kn_exact`] followed by that clamp.
Existing `cbox_binomial` is unchanged.

# Errors

As [`cbox_binomial`].

```rust
pub fn kn(k: usize, n: usize, levels: usize) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `complement`

`1 - a` — pba.r's `complement.pbox`. Exact, no dependence question.

```rust
pub fn complement(a: &crate::imprecise::Pbox) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `envelope`

pba.r's `env.pbox`: the smallest p-box containing both.

```rust
pub fn envelope(a: &crate::imprecise::Pbox, b: &crate::imprecise::Pbox) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `frechet_convolve`

pba.r's `frechetconv.pbox(x, y, op)` for a non-decreasing `op`: bounds on
`op(X, Y)` with **no assumption about the dependence** between `X` and `Y`.

# Errors

[`RafflesError::DimensionMismatch`] if the ladders differ.

```rust
pub fn frechet_convolve(x: &crate::imprecise::Pbox, y: &crate::imprecise::Pbox, op: MonotoneOp) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `independent_product`

Product of **independent** non-negative p-boxes — pba.r's
`conv.pbox(x, y, '*')`: all `n x n` cell products, sorted, every `n`-th
kept.

# Errors

[`RafflesError::DimensionMismatch`] for differing ladders;
[`RafflesError::InvalidParameter`] if an operand can be negative (the
sorted-cell construction is only monotone for non-negative factors).

```rust
pub fn independent_product(x: &crate::imprecise::Pbox, y: &crate::imprecise::Pbox) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `and_independent`

pba.r's `andI`: both events independent, and independent uncertainties.

```rust
pub fn and_independent(a: &crate::imprecise::Pbox, b: &crate::imprecise::Pbox) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `or_independent`

pba.r's `orI`: `1 - (1 - a)(1 - b)` with independent operands.

```rust
pub fn or_independent(a: &crate::imprecise::Pbox, b: &crate::imprecise::Pbox) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `and_unknown`

pba.r's `and`: no assumption on dependence. The envelope of the Fréchet
lower bound `max(a + b - 1, 0)` and the upper bound `min(a, b)`.

```rust
pub fn and_unknown(a: &crate::imprecise::Pbox, b: &crate::imprecise::Pbox) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

#### Function `or_unknown`

pba.r's `or`: no assumption on dependence. The envelope of the Fréchet
lower bound `max(a, b)` and upper bound `min(a + b, 1)`.

```rust
pub fn or_unknown(a: &crate::imprecise::Pbox, b: &crate::imprecise::Pbox) -> crate::Result<crate::imprecise::Pbox> { /* ... */ }
```

### Constants and Statics

#### Constant `PBA_BZERO`

pba.r's `Bzero`: `KN` clamps its result to `[1e-6, 1 - 1e-6]` "otherwise
beta distributions touch zero which is inconvenient in arithmetical
operations" (pba BETTER.r, near `km`/`KN`).

```rust
pub const PBA_BZERO: f64 = 1.0e-6;
```

## Module `pbox_tree`

# Fault trees whose basic events are confidence boxes

[`evaluate_pbox_tree`] takes a [`FaultTree`] and one [`Pbox`] per basic
event (usually a c-box from [`super::pba::kn`]) and propagates them
**gate by gate**, with the dependence assumption chosen explicitly for each
gate ([`GateDependence`]). The tree's structure is the SCRAM
[`FaultTree`]; its point probabilities are ignored.

## What "gate by gate" costs you

A gate combines its arguments as if they were separate quantities. If a
basic event (or a sub-gate) feeds the tree through more than one path, the
independent combination double-counts it. So:

- under [`EventDependence::Unknown`] a repeated event is **safe**: Fréchet
  bounds hold for every joint distribution, including "the same event
  twice";
- under [`EventDependence::Independent`] a repeated event is **refused**
  ([`RepeatedEvents::Reject`], the default). [`RepeatedEvents::IndependentCopies`]
  evaluates it anyway, treating each occurrence as an independent copy.
  That is what the "Boolean function" route of the Case study 3 script
  computes, and it is **not** the structure-function answer.

[`evaluate_cbox_edges`] is the other route for repeated events: Lye et al.
(2024) evaluate the tree once with every basic event replaced by its
left edge and once by its right edge and take the envelope (Case study 4,
`e1f = env.pbox(e1f_L, e1f_R)`). It applies to coherent trees and to
c-boxes, whose edges are distributions; it is not justified for an
arbitrary p-box.

`Xor` is refused (not monotone in the probabilities). `Atleast` is
supported under unknown dependence only (as the OR of the AND of every
`min`-subset, at most [`MAX_ATLEAST_SUBSETS`] subsets), because the subsets
share events.

```rust
pub mod pbox_tree { /* ... */ }
```

### Types

#### Enum `GateDependence`

Which dependence assumption each gate uses.

```rust
pub enum GateDependence {
    Uniform(crate::imprecise::EventDependence),
    PerGate(Vec<crate::imprecise::EventDependence>),
}
```

##### Variants

###### `Uniform`

The same assumption at every gate.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::imprecise::EventDependence` |  |

###### `PerGate`

One assumption per gate, indexed as [`FaultTree::gates`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<crate::imprecise::EventDependence>` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `RepeatedEvents`

What to do when a basic event or sub-gate feeds the tree twice.

```rust
pub enum RepeatedEvents {
    Reject,
    IndependentCopies,
}
```

##### Variants

###### `Reject`

Refuse if any gate would combine a repeated event as independent.

###### `IndependentCopies`

Treat every occurrence as an independent copy (the script's Boolean
route). Knowingly approximate.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PboxTreeOptions`

Options for [`evaluate_pbox_tree`].

```rust
pub struct PboxTreeOptions {
    pub dependence: GateDependence,
    pub repeated: RepeatedEvents,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dependence` | `GateDependence` | Dependence assumption per gate. |
| `repeated` | `RepeatedEvents` | Repeated-event policy. |

##### Implementations

###### Methods

- ```rust
  pub fn uniform(dependence: EventDependence) -> Self { /* ... */ }
  ```
  The same dependence everywhere, repeated events rejected.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PboxTreeResult`

A p-box for every gate of the tree.

```rust
pub struct PboxTreeResult {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn top(self: &Self) -> &Pbox { /* ... */ }
  ```
  The p-box of the top event.

- ```rust
  pub fn gate(self: &Self, index: usize) -> Option<&Pbox> { /* ... */ }
  ```
  The p-box of gate `index` (indexed as [`FaultTree::gates`]).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `evaluate_pbox_tree`

Propagates one p-box per basic event through the tree, gate by gate.

`basic` is indexed as [`Arg::BasicEvent`]; all p-boxes share a ladder.
Gates not reachable from the top are `None` in no result: only reachable
gates are evaluated, and [`PboxTreeResult::gate`] returns `None` for the
rest.

# Errors

[`RafflesError::DimensionMismatch`] if `basic` has the wrong length or
ladders differ; [`RafflesError::InvalidParameter`] for a refused
connective, a refused repeated event, or a p-box outside `[0, 1]`.

```rust
pub fn evaluate_pbox_tree(tree: &super::fault_tree::FaultTree, basic: &[crate::imprecise::Pbox], options: &PboxTreeOptions) -> crate::Result<PboxTreeResult> { /* ... */ }
```

#### Function `evaluate_cbox_edges`

The Lye et al. (2024) edge construction for **confidence boxes**: evaluate
the tree with every basic event replaced by its left edge, again with its
right edge, and take the envelope at every gate (Case study 4:
`e1f = env.pbox(e1f_L, e1f_R)`).

Valid for coherent trees whose inputs are c-boxes. Repeated events are
allowed with [`EventDependence::Unknown`] gates.

# Errors

As [`evaluate_pbox_tree`], and [`RafflesError::InvalidParameter`] if the
tree is not coherent.

```rust
pub fn evaluate_cbox_edges(tree: &super::fault_tree::FaultTree, basic: &[crate::imprecise::Pbox], options: &PboxTreeOptions) -> crate::Result<PboxTreeResult> { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_ATLEAST_SUBSETS`

Upper limit on the number of `min`-subsets an unknown-dependence
`Atleast` gate expands into.

```rust
pub const MAX_ATLEAST_SUBSETS: usize = 200;
```

## Module `probability`

Top-event probability from minimal cut sets.

Three quantifications, and the difference between them matters more than
the arithmetic does — see [`Approximation`].

```rust
pub mod probability { /* ... */ }
```

### Types

#### Struct `CutSet`

A minimal cut set: the basic events that together cause the top event.

Members are **indices into the basic-event probability slice** passed
alongside, not probabilities themselves. An empty cut set is rejected: in
fault-tree semantics it would mean the top event occurs unconditionally,
which is a malformed tree rather than a probability-1 answer.

Upstream represents a cut set as `std::vector<int>` of *signed* variable
indices, where a negative index is a complement. SCRAM asserts
`member > 0` in the probability path — complemented literals never reach
it — so this type carries unsigned indices and the assertion becomes
unrepresentable rather than checked.

```rust
pub struct CutSet {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(members: &[usize]) -> Result<Self> { /* ... */ }
  ```
  Builds a cut set from basic-event indices.

- ```rust
  pub fn members(self: &Self) -> &[usize] { /* ... */ }
  ```
  The basic-event indices, ascending and deduplicated.

- ```rust
  pub fn order(self: &Self) -> usize { /* ... */ }
  ```
  How many basic events are in this cut set — its **order**.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Approximation`

How to combine cut-set probabilities into a top-event probability.

The three differ in how they treat the overlap between cut sets — the
chance that two of them occur at once. Both approximations are **upper
bounds** for a coherent tree, and both are standard in PRA practice.

```rust
pub enum Approximation {
    Exact,
    RareEvent,
    Mcub,
}
```

##### Variants

###### `Exact`

Inclusion-exclusion over the cut sets: the exact probability of their
union, for independent basic events.

**Cost is `2^n - 1` terms in the number of cut sets `n`**, so this is
for small trees and for checking the approximations, not for a full
plant model. [`top_event_probability`] refuses more than
[`EXACT_CUT_SET_LIMIT`] cut sets rather than hanging.

**Prefer [`super::bdd::Bdd::probability`] for anything larger.** It
computes the same quantity from the tree directly, with no cut-set
ceiling — and on a **non-coherent** tree it computes a *different*,
truer quantity, because minimal cut sets there are conservative. This
variant is kept because it is an independent second route to the same
number on a coherent tree, and the two are checked against each
other.

###### `RareEvent`

Rare-event approximation: the sum of the cut-set probabilities,
clamped to 1.

Ignores every overlap, so it over-counts. Good when the basic-event
probabilities are small — the regime the name refers to — and
increasingly poor as they rise. The clamp is upstream's:
`return sum > 1 ? 1 : sum;`.

###### `Mcub`

Min-cut-upper-bound: `1 - prod(1 - p_i)` over the cut sets.

Treats the cut sets as independent, which they are not when they share
basic events. Tighter than [`Approximation::RareEvent`] and never
exceeds 1 by construction, so it needs no clamp.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `cut_set_probability`

Probability that one cut set occurs — the product of its members'
probabilities.

Assumes the basic events are **independent**. This is upstream's
`CutSetProbabilityCalculator::Calculate`.

# Errors

[`RafflesError::InvalidParameter`] if a member index is out of range for
`event_probabilities`, or if any referenced probability is outside `[0, 1]`
or not finite.

```rust
pub fn cut_set_probability(cut_set: &CutSet, event_probabilities: &[f64]) -> crate::Result<f64> { /* ... */ }
```

#### Function `top_event_probability`

Top-event probability from the minimal cut sets.

`event_probabilities[i]` is the probability of basic event `i`; cut-set
members index into it. Basic events are assumed **independent**.

# Errors

[`RafflesError::InvalidParameter`] if `cut_sets` is empty, if any member
index or probability is invalid (see [`cut_set_probability`]), or if
[`Approximation::Exact`] is asked for more than [`EXACT_CUT_SET_LIMIT`] cut
sets.

```rust
pub fn top_event_probability(cut_sets: &[CutSet], event_probabilities: &[f64], approximation: Approximation) -> crate::Result<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `EXACT_CUT_SET_LIMIT`

The largest number of cut sets [`Approximation::Exact`] will accept.

Inclusion-exclusion is `2^n - 1` terms; at 20 cut sets that is about a
million, which is a second or so, and every step beyond doubles it.

```rust
pub const EXACT_CUT_SET_LIMIT: usize = 20;
```

## Module `substitution`

Substitutions — delete terms, recovery rules and exchange events.

A fault tree says how a system fails. A substitution says something the
tree cannot: that two events are mutually exclusive, that a recovery action
applies when a particular combination occurs, or that one event stands in
for another under some condition.

# Two kinds, applied in two different places

A substitution with **no `<source>`** is *declarative*: it is a statement
about the world, and upstream applies it to the **graph** as a logical
implication. `hypothesis -> target` becomes `or(not hypothesis, target)`,
and the analysed root becomes `and(root, that)`. A `false` target — a
delete term — has no target to imply, so the conjunct is just
`not hypothesis`.

A substitution **with** a `<source>` is *non-declarative*: it is a
post-processing rule, and upstream applies it to the **products**. For
every product containing the whole hypothesis, the source events are
removed and the target added; the result is then re-minimised. That is
[`apply_to_products`].

**Upstream refuses an exact analysis with non-declarative substitutions**
— "Non-declarative substitutions do not apply to exact analyses." The
substituted product list is no longer the minimal cut sets of any Boolean
function, so inclusion-exclusion over it means nothing; only the
rare-event and MCUB approximations, which treat the products as a list,
stay meaningful. That is measured, not inferred: the compiled binary
refuses `--probability` on `TwoTrain/nondeclarative_substitutions.xml` and
runs it under `--rare-event`.

```rust
pub mod substitution { /* ... */ }
```

### Types

#### Enum `SubstitutionType`

The "traditional" substitution types of the MEF.

Upstream treats these as a *classification* rather than as behaviour: the
`type` attribute is optional, and `Substitution::type()` infers which one a
substitution amounts to from its shape. Nothing in the analysis branches on
it.

```rust
pub enum SubstitutionType {
    DeleteTerms,
    RecoveryRule,
    ExchangeEvent,
}
```

##### Variants

###### `DeleteTerms`

`delete-terms`: the hypothesis is impossible.

###### `RecoveryRule`

`recovery-rule`: the hypothesis implies the target.

###### `ExchangeEvent`

`exchange-event`: one source event is replaced by the target.

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  The MEF attribute value — upstream's `kSubstitutionTypeToString`.

- ```rust
  pub fn parse(s: &str) -> Result<Self> { /* ... */ }
  ```
  Reads the MEF attribute value.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SubstitutionTarget`

What a substitution puts in place of its source.

```rust
pub enum SubstitutionTarget {
    Event(String),
    Constant(bool),
}
```

##### Variants

###### `Event`

A basic event, by resolved id.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Constant`

`<constant value="true|false"/>`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `bool` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Substitution`

A substitution.

```rust
pub struct Substitution {
    pub name: String,
    pub declared_type: Option<SubstitutionType>,
    pub hypothesis: super::mef::Formula,
    pub target: SubstitutionTarget,
    pub source: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Its id. |
| `declared_type` | `Option<SubstitutionType>` | The `type` attribute, if the model gave one. Optional in the grammar. |
| `hypothesis` | `super::mef::Formula` | A simple Boolean formula over basic events only. |
| `target` | `SubstitutionTarget` | What replaces the source, or what the hypothesis implies. |
| `source` | `Vec<String>` | The events replaced, by resolved id. **Empty means declarative.** |

##### Implementations

###### Methods

- ```rust
  pub fn declarative(self: &Self) -> bool { /* ... */ }
  ```
  Upstream's `declarative()`: no source events.

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Upstream's `Substitution::Validate`.

- ```rust
  pub fn inferred_type(self: &Self) -> Option<SubstitutionType> { /* ... */ }
  ```
  Upstream's `Substitution::type()`: which "traditional" type this

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ProductSubstitution`

A non-declarative substitution, in basic-event indices — upstream's
`Pdag::Substitution`.

Upstream splits an `or` hypothesis into one of these per argument, so that
every entry's hypothesis is a plain conjunction to test against a product.

```rust
pub struct ProductSubstitution {
    pub hypothesis: Vec<usize>,
    pub source: Vec<usize>,
    pub target: Option<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hypothesis` | `Vec<usize>` | Every one of these must be in a product for the rule to fire. |
| `source` | `Vec<usize>` | Events removed from the product. |
| `target` | `Option<usize>` | The event added, if any. `None` is upstream's index `0`, which it uses<br>for a constant target. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `apply_to_products`

Upstream's `Zbdd::ApplySubstitutions`, as a pass over products.

For every product, each rule whose whole hypothesis is present removes its
source events and adds its target; the rewritten products are then
re-minimised, since a substitution can make one product a superset of
another.

Upstream does this inside the ZBDD, where `Apply<kAnd>`/`Apply<kOr>`
rebuild the set and `Minimize` subsumes; here the products are already a
list, so the same operations are set operations on it. The **result** is
what is checked, against SCRAM's own reported products.

# Errors

[`RafflesError::InvalidParameter`] if a substitution empties a product
entirely, which would make the top event unconditional — upstream's
`Apply<kAnd>` over no arguments is the `Base` set, and a product list
containing it is the UNITY case the generators here refuse by name.

```rust
pub fn apply_to_products(products: &[super::probability::CutSet], substitutions: &[ProductSubstitution]) -> crate::Result<Vec<super::probability::CutSet>> { /* ... */ }
```

## Module `uncertainty`

Uncertainty analysis — what the top-event probability's **distribution**
looks like, not just its mean.

A basic event defined by a `<lognormal-deviate>` is not a number, it is a
distribution. Evaluating it at its mean — which is what every other part of
this module does, and what an ordinary SCRAM run does — gives one number
and says nothing about the spread. This runs the analysis many times,
drawing every deviate afresh each round, and reports the distribution of
the answers.

# The loop

Upstream's `UncertaintyAnalysis::Analyze`, in three steps:

1. gather the basic events whose expression reaches a deviate
   ([`Expression::is_deviate_through`], which follows parameter names);
2. per trial, `Reset()` and `Sample()` each of them, **clamping to
   `[0, 1]`** — upstream's `prob > 1 ? 1 : prob < 0 ? 0 : prob`, which
   matters because a normal deviate reaches outside the unit interval;
3. quantify the top event with those probabilities, and collect.

**Parameters are sampled once per trial, not once per use.** Upstream
memoises a draw per expression object per round, so a parameter feeding
several basic events contributes the same draw to all of them. ~~Here a
parameter is a name in a table, so the table is sampled first and every
event evaluated against it — the same thing by construction rather than by
bookkeeping.~~ **CORRECTED 2026-11-17 (gh:#1611):** sampling the table
first was NOT the same thing. It drew the parameters in the table's
`HashMap` order, which std seeds per process, so one shared stream gave
each parameter a different draw on every run with the same seed; it also
drew deviate parameters no event uses. Now a parameter is drawn the way
upstream draws it: lazily, the first time a deviate event's expression
reaches it, events in index order and arguments depth first, memoised for
the rest of the trial ([`Expression::sample_memoised`]). A deviate's own
arguments read their **value**, as upstream's `mean_.value()` does, not
this trial's sample.

# What this can and cannot be checked against

SCRAM's `--uncertainty` reports the mean, sigma, error factor, confidence
interval and quantiles. Those are the oracle. But **the random streams
differ** — upstream draws from one static `std::mt19937`, this from
`petir::rng::lcg` — so the two runs cannot agree sample for
sample, only in distribution. The verification is therefore statistical,
and `tests/scram_uncertainty.rs` says at what confidence.

The quantiles differ for a second reason: upstream estimates them
**online** with `boost::accumulators`' `extended_p_square_quantile`, which
is an approximation that never stores the samples, while this sorts the
samples and takes the exact order statistic. Both are defensible; they are
not the same number.

```rust
pub mod uncertainty { /* ... */ }
```

### Types

#### Struct `UncertaintySettings`

How a Monte Carlo run is configured — upstream's `Settings` fields that
uncertainty analysis reads.

```rust
pub struct UncertaintySettings {
    pub trials: usize,
    pub quantiles: usize,
    pub bins: usize,
    pub seed: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `trials` | `usize` | How many trials to run. |
| `quantiles` | `usize` | How many quantiles to report. |
| `bins` | `usize` | How many histogram bins to report. |
| `seed` | `u64` | The generator's seed. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `UncertaintyResult`

The result of a Monte Carlo run — upstream's `UncertaintyAnalysis`
accessors.

```rust
pub struct UncertaintyResult {
    pub mean: f64,
    pub sigma: f64,
    pub error_factor: f64,
    pub confidence_interval: (f64, f64),
    pub quantiles: Vec<f64>,
    pub distribution: Vec<(f64, f64)>,
    pub samples: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mean` | `f64` | Mean of the sampled top-event probabilities. |
| `sigma` | `f64` | Standard deviation, with upstream's `n / (n - 1)` correction. |
| `error_factor` | `f64` | `exp(1.96 * sigma)` — upstream's 95 % error factor.<br><br>Note this is upstream's formula as written. It is the error factor of a<br>*log-normal* with that log-scale, so it is only the usual ratio measure<br>when the distribution is log-normal; on any other shape it is a number<br>computed from sigma rather than a property of the samples. Ported as<br>upstream wrote it. |
| `confidence_interval` | `(f64, f64)` | 95 % confidence interval **of the mean**: `mean ± 1.96 sigma / sqrt(n)`. |
| `quantiles` | `Vec<f64>` | Quantiles at `1/q, 2/q, …, 1`. |
| `distribution` | `Vec<(f64, f64)>` | The histogram, as `(lower bound, fraction of samples)` pairs.<br><br>**This is not upstream's binning.** Upstream reports the same<br>*quantity* — its `<bin value=…>` is a fraction, and the twenty of them<br>sum to 1 — but the edges come from `boost::accumulators`' density<br>accumulator, whose range is its own and is neither `[min, max]` nor<br>documented as anything reproducible. On `SmallTree` its first edge is<br>`0.0021956` and its width `0.0100408`, which is not<br>`(max - min) / 20`. These bins are equal-width over `[min, max]`, and<br>`scram_uncertainty` compares the mean, sigma and quantiles rather than<br>this. |
| `samples` | `Vec<f64>` | Every sampled top-event probability, in trial order. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `analyse`

Runs the Monte Carlo — upstream's `UncertaintyAnalysis::Analyze`.

`quantify` turns one trial's basic-event probabilities into a top-event
probability. Which quantification it uses is the caller's choice, exactly
as it is upstream: the BDD, inclusion-exclusion over cut sets, or an
approximation. It is called once per trial, so its cost sets the run's.

`parameters` are the model's parameter **expressions**. ~~sampled once per
trial before the basic events are — upstream's memoisation, by
construction.~~ **CORRECTED 2026-11-17 (gh:#1611):** each is drawn at most
once per trial, lazily, when a deviate basic event first reaches it — the
order upstream's memoised `Sample()` gives, which does not depend on the
map's iteration order. A fixed `settings.seed` therefore gives the same
samples on every run.

# Errors

[`RafflesError::InvalidParameter`] if `settings.trials` is below 2 (the
variance correction divides by `n - 1`), if a sample or a quantification
fails, if a parameter does not resolve, or if no basic event reaches a
deviate — a Monte Carlo over a
deterministic model reports a spread of zero and is almost always a
mistake, so it is refused rather than answered.

```rust
pub fn analyse<Q>(model: &super::fault_tree::FaultTreeModel, expressions: &[super::expression::Expression], parameters: &std::collections::HashMap<String, super::expression::Expression>, mission_time: f64, settings: UncertaintySettings, quantify: Q) -> crate::Result<UncertaintyResult>
where
    Q: FnMut(&[f64]) -> crate::Result<f64> { /* ... */ }
```

#### Function `statistics`

Upstream's `CalculateStatistics`, over an already-collected sample.

Separated so that the statistics can be checked against closed forms
without running a Monte Carlo, which is what
`scram_uncertainty::the_statistics_are_upstreams_formulas` does.

# Panics

Never: `analyse` has already refused fewer than two trials, and this is
only reachable through it or through a test that supplies its own samples
with at least two.

```rust
pub fn statistics(samples: Vec<f64>, settings: UncertaintySettings) -> UncertaintyResult { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_TRIALS`

Upstream `Settings::num_trials_`.

```rust
pub const DEFAULT_TRIALS: usize = 1_000;
```

#### Constant `DEFAULT_QUANTILES`

Upstream `Settings::num_quantiles_`.

```rust
pub const DEFAULT_QUANTILES: usize = 20;
```

#### Constant `DEFAULT_BINS`

Upstream `Settings::num_bins_`.

```rust
pub const DEFAULT_BINS: usize = 20;
```

#### Constant `DEFAULT_SEED`

Upstream `Settings::seed_`.

```rust
pub const DEFAULT_SEED: u64 = 0;
```

## Module `zbdd`

Zero-suppressed decision diagrams — minimal cut sets **at scale**.

[`super::mocus`] generates cut sets by the classical top-down expansion,
which is exponential and gives up on a real model:
`reference-data/scram`'s `Aralia/das9601` exhausts its five-million-state
ceiling at every order limit. This module gets the same answer from the
[`super::bdd::Bdd`] instead, where the work is proportional to the diagram
rather than to the number of intermediate sets — which is the whole reason
upstream reaches for a ZBDD, and the reason Rauzy's 1993 paper exists.

A **zero-suppressed** diagram represents a *family of sets* rather than a
Boolean function, and its reduction rule is the one that matters here: a
node whose "present" branch is empty is dropped, so a variable absent from
every set in the family costs nothing. Cut sets are sparse — a model with
108 basic events has cut sets of order 9 — which is exactly the shape that
rule is for.

# Coherent and non-coherent

The conversion keeps the variables taken on the "occurs" branch of each
path to `true`, and drops the rest. For a **coherent** tree that is exactly
the minimal cut sets. For a **non-coherent** one it discards the
requirement that some component be *working*, so the result is
conservative in the same way [`super::mocus`]'s is, and for the same
reason — upstream's ordinary path does the same.

**[`prime_implicants`] is the exact alternative**, keeping the complemented
literals, and is upstream's `--prime-implicants`. It costs more: the
consensus term adds a third recursive call at every node, and upstream
itself does not finish it on the fixture's largest model.

# Example

```
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
use raffles::scram::zbdd::minimal_cut_sets;

let mut b = FaultTreeBuilder::new();
b.basic_event("ValveOne", 0.5).unwrap();
b.basic_event("PumpOne", 0.7).unwrap();
b.basic_event("ValveTwo", 0.5).unwrap();
b.basic_event("PumpTwo", 0.7).unwrap();
b.gate("TrainOne", Connective::Or, &["ValveOne", "PumpOne"]).unwrap();
b.gate("TrainTwo", Connective::Or, &["ValveTwo", "PumpTwo"]).unwrap();
b.gate("TopEvent", Connective::And, &["TrainOne", "TrainTwo"]).unwrap();
let model = b.build("TopEvent").unwrap();

let cut_sets = minimal_cut_sets(model.tree(), None).unwrap();
assert_eq!(cut_sets.len(), 4);
assert!(cut_sets.iter().all(|c| c.order() == 2));
```

```rust
pub mod zbdd { /* ... */ }
```

### Types

#### Type Alias `SetId`

A node in the family diagram: `0` is the **empty family** (no sets at all),
`1` the family holding just the **empty set**, and anything above indexes
the builder's node table.

```rust
pub type SetId = usize;
```

#### Struct `PrimeImplicant`

A **prime implicant**: a minimal condition sufficient for the top event,
recording both what must fail and what must hold.

This is what a minimal cut set becomes once complemented literals are kept.
On a **coherent** tree the two coincide and [`negative`](Self::negative) is
always empty — no component working can ever help cause failure. On a
**non-coherent** tree they differ, and the difference is the whole point:
cut sets discard the negative literals and so describe a strictly larger
function, while prime implicants describe the real one.

Members are indices into the basic-event probability slice, as
[`CutSet`]'s are.

```rust
pub struct PrimeImplicant {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn positive(self: &Self) -> &[usize] { /* ... */ }
  ```
  Basic events that must **occur**, ascending.

- ```rust
  pub fn negative(self: &Self) -> &[usize] { /* ... */ }
  ```
  Basic events that must **not** occur, ascending.

- ```rust
  pub fn order(self: &Self) -> usize { /* ... */ }
  ```
  Total number of literals — upstream reports this as the product's

- ```rust
  pub fn probability(self: &Self, event_probabilities: &[f64]) -> Result<f64> { /* ... */ }
  ```
  Probability that this implicant holds, for independent basic events.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `minimal_cut_sets`

The minimal cut sets of a fault tree, by way of its BDD.

Equivalent to [`super::mocus::minimal_cut_sets`] and enormously more
scalable: `mocus` enumerates intermediate sets, this walks a diagram. On
`Aralia/das9601` — 288 gates, non-coherent — `mocus` gives up and this
does not.

`limit_order` discards cut sets above that order, as upstream's
`limit_order` setting does; `None` keeps all of them. Note the truncation
happens when the family is materialised, so unlike `mocus` it does not
reduce the work — it is for trimming the answer, not for making a hard
model tractable.

Returned cut sets are sorted by order then by member index, matching
`mocus`, so the two can be compared directly.

# Errors

[`RafflesError::InvalidParameter`] if the [`Bdd`] or the family diagram
exceeds its node limit.

# A non-coherent tree's answer is conservative

See the module doc: the complemented literals are dropped, exactly as in
`mocus` and in upstream's ordinary (non-prime-implicant) path.

```rust
pub fn minimal_cut_sets(tree: &super::fault_tree::FaultTree, limit_order: Option<usize>) -> crate::Result<Vec<super::probability::CutSet>> { /* ... */ }
```

#### Function `from_bdd`

The minimal cut sets of an already-built diagram.

Use this rather than [`minimal_cut_sets`] when the [`Bdd`] is also wanted
for its probability — building it twice is the expensive half.

# Errors

[`RafflesError::InvalidParameter`] if the family diagram exceeds
[`NODE_LIMIT`].

```rust
pub fn from_bdd(bdd: &super::bdd::Bdd, limit_order: Option<usize>) -> crate::Result<Vec<super::probability::CutSet>> { /* ... */ }
```

#### Function `count_minimal_cut_sets`

How many minimal cut sets a tree has, without materialising them.

The count comes off the diagram in time proportional to its size, so a
model with millions of cut sets can be counted even where listing them is
hopeless. Untruncated — an order limit is a property of the listing, not of
the family.

# Errors

[`RafflesError::InvalidParameter`] if either diagram exceeds its node
limit.

```rust
pub fn count_minimal_cut_sets(tree: &super::fault_tree::FaultTree) -> crate::Result<u128> { /* ... */ }
```

#### Function `prime_implicants`

The prime implicants of a fault tree.

Where [`minimal_cut_sets`] discards complemented literals — making its
answer conservative on a non-coherent tree — this keeps them, so the result
describes the tree's function **exactly**. On a coherent tree the two are
the same sets and every implicant's [`PrimeImplicant::negative`] is empty.

Returned implicants are sorted by order, then positive members, then
negative, so two runs compare equal.

# Cost

Substantially more than cut sets, because the consensus term adds a third
recursive call at every node. **Upstream is no faster**: `scram
--prime-implicants` does not finish within five minutes on the fixture's
`Aralia/das9601`, where its minimal cut sets take under a second. Expect
this to be usable on small and medium trees only.

# Errors

[`RafflesError::InvalidParameter`] if either diagram exceeds its node
limit.

# Example

```
use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
use raffles::scram::zbdd::prime_implicants;

// `a AND NOT b` -- failure needs `a` to fail AND `b` to be working.
let mut b = FaultTreeBuilder::new();
b.basic_event("a", 0.1).unwrap();
b.basic_event("b", 0.2).unwrap();
b.gate("NotB", Connective::Not, &["b"]).unwrap();
b.gate("Top", Connective::And, &["a", "NotB"]).unwrap();
let model = b.build("Top").unwrap();

let pis = prime_implicants(model.tree()).unwrap();
assert_eq!(pis.len(), 1);
assert_eq!(pis[0].positive(), &[0]);   // a must occur
assert_eq!(pis[0].negative(), &[1]);   // b must not
assert!((pis[0].probability(model.probabilities()).unwrap() - 0.08).abs() < 1e-12);
```

```rust
pub fn prime_implicants(tree: &super::fault_tree::FaultTree) -> crate::Result<Vec<PrimeImplicant>> { /* ... */ }
```

#### Function `minimal_cut_sets_from_graph`

The minimal cut sets of a fault tree, **without building a BDD**.

This is upstream's other route: `Zbdd::ConvertGraph` folds the gate graph
bottom-up with ZBDD `Apply`, then `EliminateComplements` deletes the
complemented literals and `Minimize` absorbs. It is the path
`Zbdd(const Gate&, const Settings&)` takes, and the one upstream's MOCUS
drives.

[`minimal_cut_sets`] answers the same question through a BDD. Keeping both
is the point: they share the `Minimize`/`Subsume` tail and nothing else,
so agreeing is evidence. ~~`tests/scram_graph_zbdd.rs`~~ **CORRECTED
2026-09-22** — there is no such file and there never was; the checks live
in `tests/scram_bdd_oracle.rs`
(`the_graph_zbdd_route_matches_scram_and_the_bdd_route`), which
compares this route against each other route, against [`super::mocus`],
and against SCRAM's own products.

`limit_order` discards cut sets above that order, applied when the family
is listed; `None` keeps all of them.

# Errors

[`RafflesError::InvalidParameter`] if the diagram exceeds [`NODE_LIMIT`],
or if the tree's minimal cut sets are the empty set (see
[`super::mocus`]'s unity error).

```rust
pub fn minimal_cut_sets_from_graph(tree: &super::fault_tree::FaultTree, limit_order: Option<usize>) -> crate::Result<Vec<super::probability::CutSet>> { /* ... */ }
```

### Constants and Statics

#### Constant `EMPTY`

The family containing no sets — a function that cannot be satisfied.

```rust
pub const EMPTY: SetId = 0;
```

#### Constant `BASE`

The family containing exactly the empty set.

Not the same as [`EMPTY`], and the difference is the usual first
confusion: `EMPTY` has no members, `BASE` has one member which happens to
have no elements.

```rust
pub const BASE: SetId = 1;
```

#### Constant `NODE_LIMIT`

Ceiling on how many distinct family nodes may be created.

Same purpose as [`super::bdd::NODE_LIMIT`]: fail with a name rather than
exhaust memory.

```rust
pub const NODE_LIMIT: usize = 20_000_000;
```

### Re-exports

#### Re-export `Bdd`

```rust
pub use bdd::Bdd;
```

#### Re-export `Expression`

```rust
pub use expression::Expression;
```

#### Re-export `MefModel`

```rust
pub use mef::MefModel;
```

#### Re-export `Arg`

```rust
pub use fault_tree::Arg;
```

#### Re-export `Connective`

```rust
pub use fault_tree::Connective;
```

#### Re-export `FaultTree`

```rust
pub use fault_tree::FaultTree;
```

#### Re-export `FaultTreeBuilder`

```rust
pub use fault_tree::FaultTreeBuilder;
```

#### Re-export `FaultTreeModel`

```rust
pub use fault_tree::FaultTreeModel;
```

#### Re-export `Gate`

```rust
pub use fault_tree::Gate;
```

#### Re-export `importance_factors`

```rust
pub use importance::importance_factors;
```

#### Re-export `importance_factors_from_bdd`

```rust
pub use importance::importance_factors_from_bdd;
```

#### Re-export `ImportanceFactors`

```rust
pub use importance::ImportanceFactors;
```

#### Re-export `minimal_cut_sets`

```rust
pub use mocus::minimal_cut_sets;
```

#### Re-export `count_minimal_cut_sets`

```rust
pub use zbdd::count_minimal_cut_sets;
```

#### Re-export `minimal_cut_sets_from_graph`

```rust
pub use zbdd::minimal_cut_sets_from_graph;
```

#### Re-export `prime_implicants`

```rust
pub use zbdd::prime_implicants;
```

#### Re-export `PrimeImplicant`

```rust
pub use zbdd::PrimeImplicant;
```

#### Re-export `cut_set_probability`

```rust
pub use probability::cut_set_probability;
```

#### Re-export `top_event_probability`

```rust
pub use probability::top_event_probability;
```

#### Re-export `Approximation`

```rust
pub use probability::Approximation;
```

#### Re-export `CutSet`

```rust
pub use probability::CutSet;
```

## Module `sensitivity`

Sensitivity analysis — Sobol variance decomposition and correlation measures.

Importance measures computed **from an existing sample set**: a matrix of
input points and the corresponding model outputs. Nothing here evaluates a
model, generates a random design, or knows what the numbers mean physically.
The caller runs their own model and hands RAFFLES arrays of `f64`.

## What is implemented

- [`sobol_indices`] — first-order `S_i` and total-effect `S_Ti` variance
  indices, estimated from a Saltelli-style A / B / A_B^(i) sample.
- [`SobolSampleLayout`] — the sample layout that estimator requires: how
  many model evaluations `k` inputs and `n` base samples cost, where each
  block sits in the output vector, and [`SobolSampleLayout::build_design`]
  to assemble the design matrix from two independent base matrices.
- [`pearson_correlation`], [`spearman_correlation`],
  [`input_output_correlations`], [`CorrelationKind`] — cheap linear and rank
  correlation measures, useful alongside the variance-based indices.
- [`sample_mean`], [`sample_variance`], [`average_ranks`] — the ensemble
  statistics the measures above are built on, exposed because they are
  useful on their own.

Everything returns [`crate::Result`]; no function in this module panics on
caller-supplied data.

## What does NOT belong here

- **Generating the design.** [`crate::samplers`] does that. This module
  consumes an already-evaluated sample. The one exception is
  [`SobolSampleLayout::build_design`], because the A-B-A_B^(i) construction
  is part of the *estimator*, not a general-purpose sampling strategy.
- **Surrogate construction.** Sobol indices read analytically off the
  coefficients of a polynomial-chaos expansion are a [`crate::surrogate`]
  capability that this module may later consume; the surrogate itself is not
  built here.
- **Plotting, reporting, file output.**

## Design

Estimators are free functions over slices returning owned results. The one
variant family — which correlation coefficient to compute — is the
[`CorrelationKind`] enum, dispatched by `match`, never
`Box<dyn SensitivityMeasure>`. No type here carries a lifetime parameter.

## The Sobol estimator, stated explicitly

For a model `f` of `k` independent inputs, let `A` and `B` be two
independent `n x k` sample matrices drawn from the input distribution, and
let `A_B^(i)` be `A` with its `i`-th column replaced by the `i`-th column of
`B`. Write `y_A = f(A)`, `y_B = f(B)`, `y_AB_i = f(A_B^(i))`, each of length
`n`. This module uses:

- **Total variance** — the unbiased sample variance of the `2n` values
  `{y_A, y_B}` pooled:

  `V = (1 / (2n - 1)) * sum over the pooled sample of (y - ybar)^2`

- **First-order index** (Saltelli's form of the Sobol'/Homma–Saltelli
  estimator):

  ~~`V_i = (1/n) * sum_j y_B[j] * (y_AB_i[j] - y_A[j])`,  `S_i = V_i / V`~~

  `V_i = (1/n) * sum_j (y_B[j] - ybar) * (y_AB_i[j] - y_A[j])`,  `S_i = V_i / V`

  **CORRECTED 2026-10-07 (gh:#584): `y_B` is centred on the pooled mean
  `ybar` above.** Subtracting a constant changes nothing in expectation —
  `y_AB_i` and `y_A` are identically distributed, so `E[y_AB_i - y_A] = 0`
  — but the uncentred product's sampling error scales with `ybar / sqrt(V)`.
  On `outram-mc-libs`' `sobol_design_on_an_additive_model` (`ybar/sd ~ 34`,
  `n = 512`) the uncentred first-order sum came out at `-1.437` against an
  exact `1`; a 2000-seed replica of that design put the uncentred sum at
  mean `1.07`, sd `2.17`, and the centred one at mean `1.000`, sd `0.058`
  (gh:#584, 2026-10-06 audit). SALib's `sobol.analyze` standardises the
  outputs before the same estimator for the same reason. The Jansen
  total-effect estimator below is a difference of outputs and is unchanged.

- **Total-effect index** (Jansen's estimator, the one Saltelli et al. (2010)
  recommend for `S_Ti`):

  `V_Ti = (1/(2n)) * sum_j (y_A[j] - y_AB_i[j])^2`,  `S_Ti = V_Ti / V`

Both indices are dimensionless. In exact arithmetic `S_i` lies in `[0, 1]`,
`S_Ti` lies in `[0, 1]`, `S_i <= S_Ti`, the `S_i` sum to at most 1 (equality
only for a purely additive model), and the `S_Ti` sum to at least 1. **A
finite-sample estimate can violate all of these**, and a small negative
`S_i` is the normal signature of an index that is truly zero. This module
deliberately does **not** clamp the estimates — a clamped index hides
exactly the "my sample is too small" signal the caller needs to see.

References for the estimator formulas:

- I. M. Sobol', *Global sensitivity indices for nonlinear mathematical
  models and their Monte Carlo estimates*, Mathematics and Computers in
  Simulation **55** (2001) 271–280.
- T. Homma and A. Saltelli, *Importance measures in global sensitivity
  analysis of nonlinear models*, Reliability Engineering and System Safety
  **52** (1996) 1–17.
- M. J. W. Jansen, *Analysis of variance designs for model output*,
  Computer Physics Communications **117** (1999) 35–43.
- A. Saltelli, P. Annoni, I. Azzini, F. Campolongo, M. Ratto and
  S. Tarantola, *Variance based sensitivity analysis of model output. Design
  and estimator for the total sensitivity index*, Computer Physics
  Communications **181** (2010) 259–270.

These bibliographic details are given as they are conventionally cited in
the sensitivity-analysis literature; they have **not** been checked against
the publications themselves and must be verified before appearing in any
published V&V write-up.

## Verification — status

The estimator is checked against closed-form indices in the `tests` module
at the bottom of this file. Measured 2026-08-06; see each test's doc comment
for methodology and the numbers actually produced.

| Gate | Reference | Achieved |
|---|---|---|
| Sudret polynomial, `N = 3` | `S_i = 25/91`, `S_Ti = 36/91` exactly | max abs error ~~`2.681e-4`~~ `2.079e-4` (centred `y_B`, 2026-10-07) on `S_i`, `3.266e-4` on `S_Ti` at `n = 65536` |
| Ishigami function | `S = (0.313905, 0.442411, 0)`, `S_T = (0.557589, 0.442411, 0.243684)` | max abs error ~~`3.723e-4`~~ `3.788e-4` (centred `y_B`, 2026-10-07) on `S`, `5.105e-5` on `S_T` at `n = 65536` |
| Additive linear model | `S_i = S_Ti = c_i^2 / sum(c^2)`, `sum S_i = 1` | max abs error on `S` ~~`2.493e-4`~~ `8.956e-5` (centred `y_B`, 2026-10-07); `sum S_i =` ~~`0.999441`~~ `1.000128` |
| Pearson / Spearman | exact constructions (`+1`, `-1`, `0`, known `r = 0.6`) | machine precision |

**Still open, not claimed:** the Sobol g-function gate named in the crate
`CLAUDE.md` verification table is *not* implemented here. It is an
8-input case, so the deterministic 16-dimensional Halton design the other
gates use degrades badly (high-index Halton dimensions correlate), and a
flaky or quietly-wrong gate is worse than a missing one. It needs a proper
low-discrepancy or scrambled sequence, which is [`crate::samplers`]' job.

No part of this module has been through **human** review, and nothing here
is validated — these are verification gates only ("is it implemented
correctly?"), not evidence that any of it represents physical reality.

## Provenance — read before adding to this file

**No RAVEN code has been ported into this module.** It is an independent
implementation of published algorithms, so per the crate `CLAUDE.md` it
carries no upstream attribution header. Checked against RAVEN `devel` at
commit `01216937967c38ee287859270c035c8eca906dc6` (accessed 2026-08-06):

- RAVEN has **no** Saltelli-style Monte Carlo Sobol estimator. Its Sobol
  indices are computed *analytically* from polynomial-chaos coefficients in
  `ravenframework/SupervisedLearning/GaussPolynomialRom.py`
  (`getSensitivities`, line 613). That is a [`crate::surrogate`] capability,
  not this one. The estimator above comes from the papers cited earlier.
- RAVEN's Pearson and Spearman counterparts live in
  `ravenframework/Models/PostProcessors/BasicStatistics.py` (`corrCoeff`,
  line 1401; `spearmanCorrelation`, line 1518). Those are *probability-
  weighted* estimators built on `numpy`/`xarray`. This module implements the
  unweighted textbook definitions directly and is not a translation of them.
  Weighted variants, if wanted later, would be the port.

**LICENCE HAZARD — keep this warning in place.** The upstream area adjacent
to sensitivity analysis is where RAVEN vendors third-party **BSD** code that
is *not* covered by RAVEN's Apache-2.0 grant:

- **AMSC** — Copyright 2014 University of Utah, Scientific Computing and
  Imaging Institute (3-clause BSD).
- **NGL** — Copyright 2012 Carlos D. Correa (2-clause BSD).

They sit in `src/AMSC/` and reach the framework through
`Models/PostProcessors/TopologicalDecomposition.py`,
`SupervisedLearning/MSR.py` and — note the name —
`ravenframework/UI/SensitivityView.py`. None of those was read or used here,
and nothing in this file derives from them. **Anything derived from AMSC or
NGL needs the BSD attribution header, not the Apache-2.0 one.** If a file
you are about to port traces back to either, stop and ask rather than
guessing at the header. See the crate `NOTICE` and `NOTICE-RAVEN`.

```rust
pub mod sensitivity { /* ... */ }
```

### Types

#### Enum `CorrelationKind`

Which correlation coefficient to compute.

Enum dispatch, per the workspace design rules — never a trait object. Both
variants produce a dimensionless coefficient in `[-1, 1]`.

```rust
pub enum CorrelationKind {
    Pearson,
    Spearman,
}
```

##### Variants

###### `Pearson`

Pearson product-moment correlation: measures **linear** association.
`+1`/`-1` only for an exactly affine relationship, and it is *not*
preserved by a non-linear monotone transform of either variable.

###### `Spearman`

Spearman rank correlation: Pearson's coefficient applied to the average
ranks of each variable. Measures **monotone** association, so it is
exactly preserved by any strictly monotone transform, linear or not.

##### Implementations

###### Methods

- ```rust
  pub fn correlation(self: &Self, x: &[f64], y: &[f64]) -> Result<f64> { /* ... */ }
  ```
  Computes whichever coefficient this variant names, for the paired

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SobolSampleLayout`

The sample layout the Sobol estimator requires — **get this wrong and the
indices are silently wrong**, which is why it is a type rather than a
convention in a doc comment.

For `k` inputs and `n` base samples the estimator needs two independent
`n x k` sample matrices `A` and `B`, plus the `k` mixed matrices `A_B^(i)`
(`A` with its `i`-th column replaced by `B`'s `i`-th column). That is
**`n * (k + 2)` model evaluations**, stacked in this fixed block order:

| Block | Rows | Accessor |
|---|---|---|
| `A` | `0 .. n` | [`block_a`](Self::block_a) |
| `B` | `n .. 2n` | [`block_b`](Self::block_b) |
| `A_B^(0)` | `2n .. 3n` | [`block_ab`](Self::block_ab) |
| … | … | … |
| `A_B^(k-1)` | `(k+1)n .. (k+2)n` | [`block_ab`](Self::block_ab) |

Both the design matrix built by [`build_design`](Self::build_design) and the
output vector consumed by [`sobol_indices`] use exactly this order.

```
use raffles::sensitivity::SobolSampleLayout;

// 3 inputs, 1024 base samples
let layout = SobolSampleLayout::new(3, 1024).unwrap();
assert_eq!(layout.model_evaluations(), 1024 * 5);
assert_eq!(layout.block_a(), 0..1024);
assert_eq!(layout.block_b(), 1024..2048);
assert_eq!(layout.block_ab(0).unwrap(), 2048..3072);
```

```rust
pub struct SobolSampleLayout {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(inputs: usize, base_samples: usize) -> Result<Self> { /* ... */ }
  ```
  Describes a Sobol design over `inputs` input variables with

- ```rust
  pub fn inputs(self: &Self) -> usize { /* ... */ }
  ```
  Number of input variables, `k`.

- ```rust
  pub fn base_samples(self: &Self) -> usize { /* ... */ }
  ```
  Number of base samples per matrix, `n`.

- ```rust
  pub fn model_evaluations(self: &Self) -> usize { /* ... */ }
  ```
  Total model evaluations this design costs: `n * (k + 2)`.

- ```rust
  pub fn block_a(self: &Self) -> Range<usize> { /* ... */ }
  ```
  Row range of the `A` block within the stacked design / output vector.

- ```rust
  pub fn block_b(self: &Self) -> Range<usize> { /* ... */ }
  ```
  Row range of the `B` block within the stacked design / output vector.

- ```rust
  pub fn block_ab(self: &Self, i: usize) -> Result<Range<usize>> { /* ... */ }
  ```
  Row range of the `A_B^(i)` block — `A` with column `i` taken from `B`.

- ```rust
  pub fn build_design(self: &Self, a: &[f64], b: &[f64]) -> Result<Vec<f64>> { /* ... */ }
  ```
  Assembles the full stacked design matrix from two independent base

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SobolIndices`

Variance-based sensitivity indices estimated from one Sobol sample.

All indices are dimensionless fractions of the output variance. See the
module docs for the exact estimators and for why the values are **not**
clamped to `[0, 1]`.

```rust
pub struct SobolIndices {
    pub first_order: Vec<f64>,
    pub total_effect: Vec<f64>,
    pub mean: f64,
    pub total_variance: f64,
    pub layout: SobolSampleLayout,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `first_order` | `Vec<f64>` | First-order index `S_i` per input, in input order: the fraction of<br>output variance explained by input `i` **alone**, averaging over all<br>the others. Exact range `[0, 1]`; the estimates sum to at most 1, with<br>equality only for a purely additive model. |
| `total_effect` | `Vec<f64>` | Total-effect index `S_Ti` per input, in input order: the fraction of<br>output variance explained by input `i` alone **plus every interaction<br>it takes part in**. Exact range `[0, 1]`, with `S_Ti >= S_i`; the<br>estimates sum to at least 1. `S_Ti` near zero is the criterion for<br>fixing an input at a nominal value. |
| `mean` | `f64` | Sample mean of the pooled `A` and `B` outputs (`2n` values), in the<br>caller's output units. |
| `total_variance` | `f64` | Unbiased sample variance of the pooled `A` and `B` outputs, the `V`<br>every index above is divided by. Squared output units, non-negative. |
| `layout` | `SobolSampleLayout` | The layout the estimate was computed against — carried so a caller can<br>report `n` and the evaluation count alongside the numbers. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `sample_mean`

Arithmetic mean of a sample.

`x` is a sample of a scalar quantity in whatever units the caller's model
uses; RAFFLES never interprets them. The result carries those same units.

# Errors

[`RafflesError::DimensionMismatch`] if `x` is empty (at least one sample is
required).

```rust
pub fn sample_mean(x: &[f64]) -> crate::Result<f64> { /* ... */ }
```

#### Function `sample_variance`

Unbiased (Bessel-corrected, `n - 1` denominator) sample variance.

Non-negative, in the square of the caller's output units. Use this rather
than the biased `n` form when the sample is being used to *estimate* a
population variance, which is what every measure in this module does.

# Errors

[`RafflesError::DimensionMismatch`] if `x` has fewer than two elements — the
unbiased variance of a single point is undefined.

```rust
pub fn sample_variance(x: &[f64]) -> crate::Result<f64> { /* ... */ }
```

#### Function `average_ranks`

Ranks of `x` in ascending order, `1`-based, with **ties resolved by
averaging** — the convention Spearman's rank correlation assumes.

The returned vector has the same length and ordering as `x`: element `j` is
the rank of `x[j]`. Ranks lie in `[1, n]` and always sum to `n(n + 1) / 2`.
Three tied values occupying ranks 4, 5 and 6 each receive `5.0`.

Non-finite inputs are not rejected: `NaN` sorts to the end (via
[`f64::total_cmp`], which cannot panic) and compares unequal to itself, so
each `NaN` receives its own rank. Results in the presence of `NaN` are
well-defined but not statistically meaningful.

# Errors

[`RafflesError::DimensionMismatch`] if `x` is empty.

```rust
pub fn average_ranks(x: &[f64]) -> crate::Result<Vec<f64>> { /* ... */ }
```

#### Function `pearson_correlation`

Pearson product-moment correlation coefficient between two paired samples.

`x[j]` and `y[j]` are the two quantities observed at sample `j`. The result
is dimensionless and lies in `[-1, 1]`: `+1` for a perfectly increasing
affine relationship, `-1` for a perfectly decreasing one, `0` for no
*linear* association (which is not the same as independence — see the
symmetric-parabola case in this module's tests).

Computed as `cov(x, y) / (sd(x) * sd(y))` with the unbiased `n - 1`
denominator throughout; the correction cancels, so the biased form gives the
identical coefficient.

# Errors

- [`RafflesError::DimensionMismatch`] if the two samples differ in length,
  or if fewer than two points are supplied.
- [`RafflesError::InvalidParameter`] if either sample has zero variance —
  a constant variable has no correlation with anything, and returning `NaN`
  silently would hide that.

```rust
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> crate::Result<f64> { /* ... */ }
```

#### Function `spearman_correlation`

Spearman rank correlation coefficient between two paired samples.

Pearson's coefficient applied to the average ranks (see [`average_ranks`]),
so it is dimensionless, lies in `[-1, 1]`, and is **invariant under any
strictly monotone transform** of either variable — `+1` for any increasing
relationship whether or not it is linear.

# Errors

- [`RafflesError::DimensionMismatch`] if the two samples differ in length,
  or if fewer than two points are supplied.
- [`RafflesError::InvalidParameter`] if either sample is entirely tied (all
  ranks equal), which makes the coefficient undefined.

```rust
pub fn spearman_correlation(x: &[f64], y: &[f64]) -> crate::Result<f64> { /* ... */ }
```

#### Function `input_output_correlations`

Correlation of every input column against a single scalar output.

A cheap first look at which inputs matter, and the natural companion to
[`sobol_indices`]: it costs one already-evaluated sample rather than the
`n * (k + 2)` evaluations the Sobol estimator needs, but it only sees
linear ([`CorrelationKind::Pearson`]) or monotone
([`CorrelationKind::Spearman`]) association, and is blind to interactions.

- `inputs_row_major` — the `n x k` input sample, **row-major**: sample `j`'s
  value for input `i` is at `inputs_row_major[j * k + i]`.
- `inputs` — `k`, the number of input variables.
- `outputs` — the `n` model outputs, `outputs[j]` matching sample row `j`.

Returns `k` coefficients, each in `[-1, 1]`, in input order.

# Errors

- [`RafflesError::InvalidParameter`] if `inputs` is zero.
- [`RafflesError::DimensionMismatch`] if `inputs_row_major.len()` is not
  `outputs.len() * inputs`, or if fewer than two samples are supplied.
- Whatever [`CorrelationKind::correlation`] returns for a degenerate column.

```rust
pub fn input_output_correlations(inputs_row_major: &[f64], inputs: usize, outputs: &[f64], kind: CorrelationKind) -> crate::Result<Vec<f64>> { /* ... */ }
```

#### Function `sobol_indices`

Estimates first-order and total-effect Sobol indices from an evaluated
Saltelli-style sample.

`outputs` holds the scalar model output for every row of the design
described by `layout`, **in that layout's block order** — build it with
[`SobolSampleLayout::build_design`] and evaluate row by row, or lay it out
yourself using [`SobolSampleLayout::block_a`],
[`SobolSampleLayout::block_b`] and [`SobolSampleLayout::block_ab`]. Its
length must be exactly [`SobolSampleLayout::model_evaluations`].

The estimator assumes the `k` inputs are **mutually independent**; the
variance decomposition it inverts does not hold for correlated inputs, and
this function cannot detect the violation.

# Errors

- [`RafflesError::DimensionMismatch`] if `outputs.len()` does not match the
  layout.
- [`RafflesError::InvalidParameter`] if the pooled output variance is zero
  (a constant model has no sensitivity structure to report, and dividing by
  it would return `NaN` indices that look like real answers).

```rust
pub fn sobol_indices(layout: SobolSampleLayout, outputs: &[f64]) -> crate::Result<SobolIndices> { /* ... */ }
```

## Module `surrogate`

Surrogate models — cheap reduced-order stand-ins for an expensive model.

## What is implemented

- **[`polynomial`]** — multivariate polynomial regression by least squares,
  with automatic input rescaling and optional ridge regularisation. Start
  here: it fits in closed form, it reproduces a polynomial exactly, and its
  coefficients mean something.
- **[`neural`]** — a `burn`-trained neural-network regressor, behind the
  crate's `burn` feature. For responses a bounded-degree polynomial cannot
  express.

- **[`validation`]** — leave-one-out cross-validation and jackknife+
  prediction intervals for [`PolynomialSurrogate`] (added 2026-10-03,
  GitHub #499).
- **[`MultiOutputPolynomial`]** — one [`PolynomialSurrogate`] per output
  column, each bit-identical to the scalar fit (added 2026-11-04, gh:#1194).

**The typed, common interface to these routes is [`crate::fidelity`]**
(gh:#1194): `uom` records in and out, a `Dataset` with its held-out split
fixed at construction, `SurrogateSpec` / `Surrogate<I, O>` over the routes
here, and a `ValidationReport` on the held-out rows. New routes (kriging,
polynomial chaos, co-kriging) land here as `f64` implementations and as a
variant there.

~~Gaussian process regression / kriging and the sparse-grid polynomial chaos
route are **not** implemented.~~ **CORRECTED 2026-11-04 (gh:#1193)** — both
now are, as RAVEN ports: [`polynomial_chaos`] (RAVEN's `GaussPolynomialRom`
with its index sets, Smolyak/tensor grids and SciPy's Gauss rules) and
[`gaussian_process`] (RAVEN's `GaussianProcessRegressor` interface over
scikit-learn's numerics). ~~HDMR and the SMT kriging route (gh:#1190) are
not.~~ **CORRECTED 2026-11-04 (gh:#1190)** — [`smt`] ports SMT's KRG,
KPLS, KPLSK and MFK co-kriging; HDMR is still not ported. ~~Neither is the cross-validation
machinery;~~ **CORRECTED 2026-10-03** — leave-one-out cross-validation is
now in [`validation`] (polynomial surrogates only; k-fold and the neural
surrogate are not covered). The fitted models still report `r_squared` and
`rmse` on whatever data they are given, and the tests here demonstrate why
that must be held-out data.

## Scope — what belongs here

Models fitted to a sample set (inputs and the model outputs at those
inputs) and then evaluated in place of re-running the expensive simulation
— RAVEN's `SupervisedLearning` / ROM layer:

- Polynomial chaos expansions, including the sparse-grid collocation route.
- Gaussian process regression / kriging.
- Linear and polynomial regression models.
- The cross-validation machinery needed to say whether a fit is any good.

## What would NOT belong here

- The sample design the surrogate is fitted to ([`crate::samplers`]).
- Sensitivity measures ([`crate::sensitivity`]) — though a polynomial chaos
  expansion yields Sobol indices directly from its coefficients, so the two
  modules will interact once both exist.
- Any physics model. A surrogate here approximates a caller's black box.

## Before starting work here

Surrogate fitting is where the temptation to add a BLAS/LAPACK dependency
appears. Do not. The workspace Android/Termux rule is hard: prefer the
pure-Rust `faer` already in the root `[workspace.dependencies]`, and if
something BLAS-backed is genuinely unavoidable, declare it under
`[target.'cfg(not(target_os = "android"))'.dependencies]` in the same
change and note it in the README.

## Design

Enum dispatch, as everywhere else in this crate — never
`Box<dyn Surrogate>`. No lifetime parameters. The dispatch enum is
[`crate::fidelity::Surrogate`].

## Verification requirement

A surrogate is not done until it is checked against a function whose exact
answer is known: a polynomial the expansion should reproduce to machine
precision at the right order, and a published test problem (Ishigami,
Sobol g-function, or a standard regression benchmark) with reported error
metrics. Record the methodology and the measured errors.

## Provenance

~~No RAVEN code has been ported into this module.~~ **CORRECTED
2026-11-04** — `polynomial_chaos/` and `gaussian_process.rs` are RAVEN
ports (gh:#1193), with SciPy, NumPy and scikit-learn (BSD-3-Clause) numerics
beside them. Each derived file carries the attribution header shown in the crate `CLAUDE.md`, naming
the upstream file under `ravenframework/SupervisedLearning/`, the commit,
the copyright holder and the licence.

```rust
pub mod surrogate { /* ... */ }
```

### Modules

## Module `gaussian_process`

**Gaussian-process regression, RAVEN's route**: RAVEN's
`GaussianProcessRegressor` ROM (its kernel menu and defaults) over
scikit-learn's numerics (gh:#1193). The SMT kriging port (gh:#1190) is
the second route; the two are to be cross-checked there.

RAVEN's defaults, which [`GpConfig::raven_defaults`] reproduces: no kernel
given (so sklearn's `1.0 (fixed) * RBF(1.0, fixed)`, which leaves
**nothing to optimise**), `alpha = 1e-10`, `normalize_y = false`,
L-BFGS-B, no restarts. Pick a kernel with [`GpKernel::raven`] to get one
whose hyperparameters are fitted.

The leave-one-out residuals use the closed form of Rasmussen and
Williams (2006), *Gaussian Processes for Machine Learning*, eq. 5.12, at
the fitted hyperparameters (an independent addition, not from either
upstream).

# V&V

Record: `crates/raffles/verification_and_validation/raven_routes_gh1193.md`;
differential tests in `tests/raven_routes_gh1193.rs`.

```rust
pub mod gaussian_process { /* ... */ }
```

### Types

#### Struct `Hyper`

One scalar hyperparameter: its value, and its bounds (`None` = fixed).

```rust
pub struct Hyper {
    pub value: f64,
    pub bounds: Option<(f64, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `value` | `f64` | The value (not its log). |
| `bounds` | `Option<(f64, f64)>` | `(lower, upper)`, or `None` for a fixed hyperparameter. |

##### Implementations

###### Methods

- ```rust
  pub fn free(value: f64) -> Self { /* ... */ }
  ```
  sklearn's default: value with bounds `(1e-5, 1e5)`.

- ```rust
  pub fn fixed(value: f64) -> Self { /* ... */ }
  ```
  A fixed hyperparameter.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `BaseKernel`

The stationary part of the kernel.

```rust
pub enum BaseKernel {
    Rbf,
    Matern12,
    Matern32,
    Matern52,
}
```

##### Variants

###### `Rbf`

Squared exponential, `exp(−d²/2)` on scaled distances.

###### `Matern12`

Matérn ν = 1/2.

###### `Matern32`

Matérn ν = 3/2.

###### `Matern52`

Matérn ν = 5/2 (RAVEN's `Matern`).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `RavenKernel`

RAVEN's kernel names for regression (`pickKernel`).

```rust
pub enum RavenKernel {
    Rbf,
    Matern,
    ConstantRbf,
    ConstantMatern,
    RbfWhite,
    ConstantRbfWhite,
}
```

##### Variants

###### `Rbf`

`RBF`: `RBF(length_scale=1 or ones(d))`.

###### `Matern`

`Matern`: `Matern(length_scale, nu=2.5)`.

###### `ConstantRbf`

`Constant*RBF`.

###### `ConstantMatern`

`Constant*Matern`.

###### `RbfWhite`

`RBF+WhiteNoise`.

###### `ConstantRbfWhite`

`Constant*RBF+WhiteNoise`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GpKernel`

`[amplitude ×] base(length scales) [+ white noise]`.

```rust
pub struct GpKernel {
    pub amplitude: Option<Hyper>,
    pub base: BaseKernel,
    pub length_scale: Vec<f64>,
    pub length_scale_bounds: Option<(f64, f64)>,
    pub white: Option<Hyper>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `amplitude` | `Option<Hyper>` | A `ConstantKernel` factor; `None` for none. |
| `base` | `BaseKernel` | The stationary kernel. |
| `length_scale` | `Vec<f64>` | One length scale (isotropic) or one per input (anisotropic). |
| `length_scale_bounds` | `Option<(f64, f64)>` | Bounds shared by every length scale; `None` = fixed. |
| `white` | `Option<Hyper>` | A `WhiteKernel` term; `None` for none. |

##### Implementations

###### Methods

- ```rust
  pub fn raven_default() -> Self { /* ... */ }
  ```
  sklearn's default when RAVEN passes no kernel:

- ```rust
  pub fn raven(name: RavenKernel, d: usize, anisotropic: bool) -> Self { /* ... */ }
  ```
  RAVEN's `pickKernel`/`customKernel` for `name`, on `d` inputs, with

- ```rust
  pub fn theta(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The free hyperparameters, `ln` of each, in sklearn's order.

- ```rust
  pub fn bounds(self: &Self) -> Vec<(f64, f64)> { /* ... */ }
  ```
  Bounds of [`theta`](Self::theta), in log space.

- ```rust
  pub fn with_theta(self: &Self, theta: &[f64]) -> Result<Self> { /* ... */ }
  ```
  A copy with the free hyperparameters set from `theta` (log space,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `GpOptimizer`

How the hyperparameters are chosen.

```rust
pub enum GpOptimizer {
    MaximiseLml,
    None,
}
```

##### Variants

###### `MaximiseLml`

Maximise the log marginal likelihood (RAVEN's `fmin_l_bfgs_b`, here a
bounded BFGS; see the module notes).

###### `None`

Keep the kernel's hyperparameters (RAVEN's `optimizer = None`).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GpConfig`

RAVEN's `GaussianProcessRegressor` settings.

```rust
pub struct GpConfig {
    pub kernel: GpKernel,
    pub alpha: f64,
    pub normalize_y: bool,
    pub optimizer: GpOptimizer,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kernel` | `GpKernel` | The kernel and its starting hyperparameters. |
| `alpha` | `f64` | Added to the kernel diagonal in the fit (`<alpha>`, default 1e-10). |
| `normalize_y` | `bool` | Standardise the targets (`<normalize_y>`, default false). |
| `optimizer` | `GpOptimizer` | `<optimizer>`. |

##### Implementations

###### Methods

- ```rust
  pub fn raven_defaults() -> Self { /* ... */ }
  ```
  RAVEN's defaults: [`GpKernel::raven_default`], alpha 1e-10, no target

- ```rust
  pub fn raven_with_kernel(kernel: GpKernel) -> Self { /* ... */ }
  ```
  RAVEN's defaults with a named kernel.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GaussianProcess`

A fitted single-output Gaussian process.

```rust
pub struct GaussianProcess {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn fit(config: &GpConfig, x: &[Vec<f64>], y: &[f64]) -> Result<Self> { /* ... */ }
  ```
  Fits one output.

- ```rust
  pub fn kernel(self: &Self) -> &GpKernel { /* ... */ }
  ```
  The fitted kernel.

- ```rust
  pub fn log_marginal_likelihood(self: &Self) -> f64 { /* ... */ }
  ```
  The log marginal likelihood at the fitted hyperparameters

- ```rust
  pub fn log_marginal_likelihood_at(self: &Self, theta: &[f64]) -> Result<Option<(f64, Vec<f64>)>> { /* ... */ }
  ```
  The LML and its gradient at another `theta` (log space), on the

- ```rust
  pub fn predict(self: &Self, x: &[f64]) -> Result<(f64, f64)> { /* ... */ }
  ```
  Predictive mean and standard deviation at `x`.

- ```rust
  pub fn loo_residuals(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Leave-one-out residuals `y_i − μ_{−i}` at the fitted hyperparameters,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `SKLEARN_DEFAULT_BOUNDS`

sklearn's default hyperparameter bounds, `(1e-5, 1e5)`.

```rust
pub const SKLEARN_DEFAULT_BOUNDS: (f64, f64) = _;
```

## Module `polynomial`

Polynomial regression surrogates, fitted by least squares.

# What this is

A multivariate polynomial of bounded total degree, fitted to a sample of
(input, output) pairs and evaluated in place of the expensive model. The
oldest and least fashionable surrogate, and often the right one: it is
cheap, it is deterministic, it extrapolates predictably badly (which is
better than extrapolating unpredictably badly), and its coefficients mean
something.

It is also the foundation of a polynomial chaos expansion — the same fit in
an orthogonal basis — which is why RAVEN's ROM layer starts here.

# The basis

All monomials of total degree at most `degree` in `d` inputs, in graded
lexicographic order, starting with the constant. The number of them is
`C(d + degree, degree)`, which [`PolynomialSurrogate::basis_size`] reports:
4 inputs at degree 3 is 35 terms, at degree 5 it is 126, and at degree 8 it
is 495. That growth is why a caller should ask before fitting rather than
after.

# Solving

Normal equations with a Cholesky factorisation, plus optional Tikhonov
ridge regularisation on the diagonal. Not the most numerically refined
choice — a QR or SVD of the design matrix is better conditioned — and the
reason it is used anyway is stated honestly in
[`PolynomialSurrogate::fit`]: it keeps the crate free of a linear-algebra
dependency, and the condition-number cost is bounded by rescaling the inputs
to `[-1, 1]` first, which this module does automatically.

# References

- N. Wiener (1938). The homogeneous chaos. *American Journal of
  Mathematics, 60*(4), 897–936. doi:
  [10.2307/2371268](https://doi.org/10.2307/2371268)
- D. Xiu and G. E. Karniadakis (2002). The Wiener–Askey polynomial chaos for
  stochastic differential equations. *SIAM Journal on Scientific Computing,
  24*(2), 619–644. doi:
  [10.1137/S1064827501387826](https://doi.org/10.1137/S1064827501387826)

```rust
pub mod polynomial { /* ... */ }
```

### Types

#### Struct `PolynomialSurrogate`

A fitted multivariate polynomial surrogate.

Built by [`PolynomialSurrogate::fit`] and evaluated by
[`PolynomialSurrogate::predict`].

```rust
pub struct PolynomialSurrogate {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn basis_size(inputs: usize, degree: usize) -> usize { /* ... */ }
  ```
  Number of basis terms for `inputs` variables at total degree `degree`:

- ```rust
  pub fn fit(inputs: &[Vec<f64>], outputs: &[f64], degree: usize, ridge: f64) -> Result<Self> { /* ... */ }
  ```
  Fits a polynomial of total degree `degree` to `(inputs, outputs)` by

- ```rust
  pub fn predict(self: &Self, x: &[f64]) -> Result<f64> { /* ... */ }
  ```
  Evaluates the fitted polynomial at one input vector.

- ```rust
  pub fn degree(self: &Self) -> usize { /* ... */ }
  ```
  Total degree of the fitted basis.

- ```rust
  pub fn terms(self: &Self) -> usize { /* ... */ }
  ```
  Number of basis terms, and therefore of fitted coefficients.

- ```rust
  pub fn coefficients(self: &Self) -> &[f64] { /* ... */ }
  ```
  The fitted coefficients, aligned with [`basis`](Self::basis).

- ```rust
  pub fn basis(self: &Self) -> &[Vec<usize>] { /* ... */ }
  ```
  The exponent vector of each basis term.

- ```rust
  pub fn r_squared(self: &Self, inputs: &[Vec<f64>], outputs: &[f64]) -> Result<f64> { /* ... */ }
  ```
  Coefficient of determination `R^2` on a data set.

- ```rust
  pub fn rmse(self: &Self, inputs: &[Vec<f64>], outputs: &[f64]) -> Result<f64> { /* ... */ }
  ```
  Root-mean-square prediction error on a data set.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MultiOutputPolynomial`

A multi-output polynomial surrogate: **one independent
[`PolynomialSurrogate`] per output column**, all at the same degree and
ridge (added 2026-11-04 for `crate::fidelity`, gh:#1194).

One fit per output, rather than a shared multi-right-hand-side solve, so
each column's fit is bit-identical to the scalar fit of that column: the
test `each_output_is_the_scalar_fit` pins it.

```rust
pub struct MultiOutputPolynomial {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn fit(inputs: &[Vec<f64>], outputs: &[Vec<f64>], degree: usize, ridge: f64) -> Result<Self> { /* ... */ }
  ```
  Fits one polynomial per column of `outputs` (rows of equal width).

- ```rust
  pub fn predict(self: &Self, x: &[f64]) -> Result<Vec<f64>> { /* ... */ }
  ```
  Evaluates every output's polynomial at `x`.

- ```rust
  pub fn outputs(self: &Self) -> &[PolynomialSurrogate] { /* ... */ }
  ```
  The per-output fits, in column order.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `polynomial_chaos`

**Polynomial chaos: RAVEN's `GaussPolynomialRom`** (gh:#1193), with its
index sets, Smolyak and tensor quadrature grids and orthonormal
Legendre/Hermite polynomials ([`grid`]), and the SciPy Gauss rules and
special functions it delegates to ([`special`]).

The expansion is a **projection**, not a regression: each coefficient is
the quadrature of `y · Φ_k` on RAVEN's grid, so the training data must be
the model at the grid's points ([`PolynomialChaosSpec::grid`]). Mean,
variance and Sobol indices follow from the coefficients alone.

# How this relates to [`super::PolynomialSurrogate`]

The existing polynomial route is a least-squares fit in monomials on any
design; this one is RAVEN's spectral projection in orthonormal
polynomials on a quadrature design. Neither replaces the other; both are
exact for a polynomial inside their basis (the V&V record's A4).

# V&V

Record: `crates/raffles/verification_and_validation/raven_routes_gh1193.md`;
differential tests in `tests/raven_routes_gh1193.rs`.

```rust
pub mod polynomial_chaos { /* ... */ }
```

### Modules

## Module `grid`

RAVEN's index sets, quadrature sparse grids and orthonormal polynomials
for polynomial chaos.

```rust
pub mod grid { /* ... */ }
```

### Types

#### Enum `PceVariable`

One input of a polynomial chaos expansion, with RAVEN's default
quadrature and polynomial family for it.

```rust
pub enum PceVariable {
    Uniform {
        lower: f64,
        upper: f64,
    },
    Normal {
        mean: f64,
        std: f64,
    },
}
```

##### Variants

###### `Uniform`

`Uniform(lower, upper)`: Gauss–Legendre points, Legendre polynomials.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `lower` | `f64` | Lower bound. |
| `upper` | `f64` | Upper bound. |

###### `Normal`

Untruncated `Normal(mean, std)`: Gauss–Hermite (probabilists') points,
Hermite polynomials.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mean` | `f64` | Mean. |
| `std` | `f64` | Standard deviation. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `IndexSetKind`

Which index set (`<IndexSet>`).

```rust
pub enum IndexSetKind {
    TensorProduct,
    TotalDegree,
    HyperbolicCross,
}
```

##### Variants

###### `TensorProduct`

`max_j(p_j w_j) <= L · mean(w)`.

###### `TotalDegree`

`sum_j p_j w_j <= L · mean(w)`.

###### `HyperbolicCross`

`prod_j (p_j + 1)^(w_j) <= (L + 1)^(mean(w))`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SparseGridKind`

Which grid (`<SparseGrid>`).

```rust
pub enum SparseGridKind {
    Smolyak,
    Tensor,
}
```

##### Variants

###### `Smolyak`

Smolyak's combination of tensor rules (RAVEN's default).

###### `Tensor`

The full tensor grid at each dimension's largest order.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `QuadratureGrid`

A quadrature grid: points in the variables' own space, and weights, in
RAVEN's insertion order.

```rust
pub struct QuadratureGrid {
    pub points: Vec<Vec<f64>>,
    pub weights: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<Vec<f64>>` | Points, one row per point. |
| `weights` | `Vec<f64>` | Weights (on the quadrature measures, before `measureNorm`). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `index_set`

**Attributes:**

- `MustUse { reason: None }`

`IndexSet.initialize` + the subclass rule + `generateMultiIndex`.
`importance` is RAVEN's per-variable `weight` (default 1).

```rust
pub fn index_set(kind: IndexSetKind, order: usize, importance: &[f64]) -> Vec<Vec<usize>> { /* ... */ }
```

#### Function `quadrature_grid`

`SmolyakSparseGrid.initialize` / `TensorGrid.initialize`.

# Errors

A Gauss rule out of range (more than 150 points in a dimension).

```rust
pub fn quadrature_grid(vars: &[PceVariable], kind: IndexSetKind, set: &[Vec<usize>], grid: SparseGridKind) -> crate::Result<QuadratureGrid> { /* ... */ }
```

## Module `special`

The special functions and Gauss rules RAVEN takes from SciPy, ported in
SciPy's operation order so the differential test can compare bits.

```rust
pub mod special { /* ... */ }
```

### Types

#### Enum `GaussRule`

Which Gauss rule.

```rust
pub enum GaussRule {
    Legendre,
    HermiteNorm,
}
```

##### Variants

###### `Legendre`

`roots_legendre`: weight 1 on [−1, 1], weights sum to 2.

###### `HermiteNorm`

`roots_hermitenorm`: weight `exp(−x²/2)`, weights sum to `√(2π)`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `eval_legendre`

**Attributes:**

- `MustUse { reason: None }`

`scipy.special.eval_legendre(n, x)` for integer `n` (`eval_legendre_l`).

```rust
pub fn eval_legendre(n: usize, x: f64) -> f64 { /* ... */ }
```

#### Function `eval_hermitenorm`

**Attributes:**

- `MustUse { reason: None }`

`scipy.special.eval_hermitenorm(n, x)`: probabilists' Hermite `He_n`.

```rust
pub fn eval_hermitenorm(n: usize, x: f64) -> f64 { /* ... */ }
```

#### Function `numpy_sum`

**Attributes:**

- `MustUse { reason: None }`

NumPy's sum of a contiguous `f64` array: the first element plus the
pairwise sum of the rest (the add reduction of `loops_utils.h.src`).

```rust
pub fn numpy_sum(a: &[f64]) -> f64 { /* ... */ }
```

#### Function `numpy_around`

**Attributes:**

- `MustUse { reason: None }`

`np.around(x, decimals)` for `decimals > 0`: multiply, round half to
even, divide.

```rust
pub fn numpy_around(x: f64, decimals: i32) -> f64 { /* ... */ }
```

#### Function `gauss_rule`

`scipy.special.roots_legendre(n)` / `roots_hermitenorm(n)`, `1 <= n <=
150`: Golub–Welsch eigenvalues, one Newton step, scipy's log-normalised
weights, symmetrisation, then the weights scaled to `mu0`.

# Errors

[`RafflesError::InvalidParameter`] for `n` outside `1..=150`.

```rust
pub fn gauss_rule(rule: GaussRule, n: usize) -> crate::Result<(Vec<f64>, Vec<f64>)> { /* ... */ }
```

### Types

#### Struct `PolynomialChaosSpec`

What to build: the inputs, the index set, the order and the grid.

```rust
pub struct PolynomialChaosSpec {
    pub variables: Vec<PceVariable>,
    pub index_set: IndexSetKind,
    pub order: usize,
    pub grid: SparseGridKind,
    pub importance: Option<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `variables` | `Vec<PceVariable>` | One entry per input, in input order. |
| `index_set` | `IndexSetKind` | `<IndexSet>`. |
| `order` | `usize` | `<PolynomialOrder>` (at least 1, RAVEN's own check). |
| `grid` | `SparseGridKind` | `<SparseGrid>` (RAVEN's default: Smolyak). |
| `importance` | `Option<Vec<f64>>` | Per-input importance weights (RAVEN's `<Interpolation weight=...>`,<br>default 1); `None` for all ones. |

##### Implementations

###### Methods

- ```rust
  pub fn total_degree(variables: Vec<PceVariable>, order: usize) -> Self { /* ... */ }
  ```
  A Smolyak, total-degree, isotropic spec (RAVEN's defaults apart from

- ```rust
  pub fn index_points(self: &Self) -> Result<Vec<Vec<usize>>> { /* ... */ }
  ```
  The index set: the multi-indices of the polynomial terms, in RAVEN's

- ```rust
  pub fn grid(self: &Self) -> Result<QuadratureGrid> { /* ... */ }
  ```
  The quadrature grid the model must be run on, in RAVEN's order.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PolynomialChaos`

A trained expansion, one coefficient vector per output.

```rust
pub struct PolynomialChaos {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn fit(spec: &PolynomialChaosSpec, x: &[Vec<f64>], y: &[Vec<f64>]) -> Result<Self> { /* ... */ }
  ```
  Trains on rows `x` / outputs `y` that contain every grid point

- ```rust
  pub fn spec(self: &Self) -> &PolynomialChaosSpec { /* ... */ }
  ```
  The spec it was built from.

- ```rust
  pub fn index_set(self: &Self) -> &[Vec<usize>] { /* ... */ }
  ```
  The multi-indices, aligned with each output's coefficients.

- ```rust
  pub fn coefficients(self: &Self, k: usize) -> &[f64] { /* ... */ }
  ```
  The coefficients of output `k`, aligned with [`index_set`](Self::index_set).

- ```rust
  pub fn outputs(self: &Self) -> usize { /* ... */ }
  ```
  Number of outputs.

- ```rust
  pub fn predict(self: &Self, x: &[f64]) -> Result<Vec<f64>> { /* ... */ }
  ```
  `__evaluateLocal__`: every output at `x`.

- ```rust
  pub fn mean(self: &Self, k: usize) -> f64 { /* ... */ }
  ```
  `__mean__`: the coefficient of the constant term.

- ```rust
  pub fn variance(self: &Self, k: usize) -> f64 { /* ... */ }
  ```
  `__variance__`: `Σ c² − mean²`.

- ```rust
  pub fn sobol(self: &Self, k: usize) -> Vec<(Vec<usize>, f64, f64)> { /* ... */ }
  ```
  `getSensitivities`: Sobol indices and partial variances per subset of

- ```rust
  pub fn total_sobol(self: &Self, k: usize) -> Vec<(Vec<usize>, f64)> { /* ... */ }
  ```
  `getTotalSensitivities`: for each subset, the sum of the Sobol

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `GRID_MATCH_TOLERANCE`

RAVEN's tolerance for matching a training row to a grid point
(`GaussPolynomialRom._train`, `distance_upper_bound=1e-9`).

```rust
pub const GRID_MATCH_TOLERANCE: f64 = 1e-9;
```

### Re-exports

#### Re-export `index_set`

```rust
pub use grid::index_set;
```

#### Re-export `quadrature_grid`

```rust
pub use grid::quadrature_grid;
```

#### Re-export `IndexSetKind`

```rust
pub use grid::IndexSetKind;
```

#### Re-export `PceVariable`

```rust
pub use grid::PceVariable;
```

#### Re-export `QuadratureGrid`

```rust
pub use grid::QuadratureGrid;
```

#### Re-export `SparseGridKind`

```rust
pub use grid::SparseGridKind;
```

## Module `smt`

**SMT's kriging family** (gh:#1190, epic gh:#1177): the second kriging
route beside RAVEN/sklearn's ([`crate::surrogate::gaussian_process`]).

| SMT | Here |
|---|---|
| `KRG` | [`SmtKriging`] with [`SmtPls::None`] |
| `KPLS` | [`SmtKriging`] with [`SmtPls::Kpls`] (PLS from scikit-learn, [`pls`]) |
| `KPLSK` | [`SmtKriging`] with [`SmtPls::Kplsk`] |
| `MFK` | [`Mfk`] |

SMT's defaults are [`SmtKrgConfig::smt_defaults`] and
[`MfkConfig::smt_defaults`]: `squar_exp`, constant trend, nugget `100 ε`,
θ0 `1e-2`, θ bounds `[1e-6, 20]`, X and y standardised with the sample
std, 10-point multistart. **The optimiser is not SMT's** (TNC is not
ported): it is this crate's bounded BFGS on SMT's own objective, so fits
agree at optima, not along paths. Typed access goes through
`fidelity::SurrogateSpec::Kriging { route: KrigingRoute::Smt(..) }` and
`SurrogateSpec::CoKriging`.

# V&V

Record: `crates/raffles/verification_and_validation/smt_kriging_gh1190.md`
(predictions committed before this code); differential tests against SMT
in `tests/smt_kriging_gh1190.rs`.

```rust
pub mod smt { /* ... */ }
```

### Modules

## Module `core`

The numerics every SMT kriging route shares: kernels, regression bases,
standardisation, the reduced likelihood and its gradient, and the
multistart optimiser over log10 θ.

```rust
pub mod core { /* ... */ }
```

### Types

#### Enum `SmtCorrelation`

SMT's correlation functions (`corr`). Each is a product of 1-D kernels on
the componentwise distances `d_i` (already `|Δx_i|^power`, or their PLS
projection).

```rust
pub enum SmtCorrelation {
    SquarExp,
    AbsExp,
    Matern52,
    Matern32,
}
```

##### Variants

###### `SquarExp`

`squar_exp`, SMT's default: `exp(−Σ θ_i d_i)`, `d_i = Δx_i²`.

###### `AbsExp`

`abs_exp`: `exp(−Σ θ_i d_i)`, `d_i = |Δx_i|`.

###### `Matern52`

`matern52`: `Π (1 + √5 l_i + 5/3 l_i²) exp(−√5 Σ l_i)`, `l_i = θ_i |Δx_i|`.

###### `Matern32`

`matern32`: `Π (1 + √3 l_i) exp(−√3 Σ l_i)`, `l_i = θ_i |Δx_i|`.

##### Implementations

###### Methods

- ```rust
  pub fn power(self: Self) -> f64 { /* ... */ }
  ```
  SMT's `_pow_exp_power`: 2 for `squar_exp`, 1 otherwise.

- ```rust
  pub fn value(self: Self, theta: &[f64], d: &[f64]) -> f64 { /* ... */ }
  ```
  The correlation of one pair, from its componentwise distances `d`.

- ```rust
  pub fn derivative(self: Self, theta: &[f64], d: &[f64], k: usize, r: f64) -> f64 { /* ... */ }
  ```
  `∂r/∂θ_k` given `r` (SMT's `corr(d, grad_ind=k)`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SmtRegression`

SMT's regression bases (`poly`, and MFK's `rho_regr`).

```rust
pub enum SmtRegression {
    Constant,
    Linear,
    Quadratic,
}
```

##### Variants

###### `Constant`

`[1]`: ordinary kriging.

###### `Linear`

`[1, x_1, …, x_n]`.

###### `Quadratic`

`[1, x_1, …, x_n, x_1 x_1, x_1 x_2, …, x_n x_n]` (upper triangle, row by row).

##### Implementations

###### Methods

- ```rust
  pub fn basis(self: Self, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  The basis at a (standardised) point.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Projection`

How componentwise distances are formed.

```rust
pub enum Projection {
    Full,
    Pls(Vec<Vec<f64>>),
}
```

##### Variants

###### `Full`

`|Δx_i|^power` in every input dimension (KRG, MFK, KPLSK's second pass).

###### `Pls`

KPLS: `Δx² · coeff²` (squar_exp) or `|Δx| · |coeff|` (otherwise), one
column per PLS component; `coeff` is `nx × n_comp`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Vec<f64>>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn distances(self: &Self, corr: SmtCorrelation, dx: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  The componentwise distances of one difference vector `dx`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Pairs`

The training pairs `(i, j)`, `i < j`, in SMT's order, and their
componentwise distances.

```rust
pub struct Pairs {
    pub ij: Vec<(usize, usize)>,
    pub d: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ij` | `Vec<(usize, usize)>` | Indices. |
| `d` | `Vec<Vec<f64>>` | Distances, one row per pair. |

##### Implementations

###### Methods

- ```rust
  pub fn new(x: &[Vec<f64>], corr: SmtCorrelation, proj: &Projection) -> Self { /* ... */ }
  ```
  `cross_distances` then the componentwise transform.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Level`

One level's training problem: standardised inputs' pair distances, the
regression matrix `F`, the standardised outputs, and how many of `F`'s
columns are the regression (`p`) and the autoregressive part (`q`).

```rust
pub struct Level {
    pub pairs: Pairs,
    pub f: Vec<Vec<f64>>,
    pub y: Vec<f64>,
    pub p: usize,
    pub q: usize,
    pub y_std: f64,
    pub nugget: f64,
    pub corr: SmtCorrelation,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pairs` | `Pairs` | Pair distances of the standardised inputs. |
| `f` | `Vec<Vec<f64>>` | `n × (q + p)`. |
| `y` | `Vec<f64>` | Standardised outputs. |
| `p` | `usize` | Regression columns. |
| `q` | `usize` | Autoregressive (ρ) columns, 0 for a single-fidelity model. |
| `y_std` | `f64` | SMT's `y_std` (scales σ² back to output units). |
| `nugget` | `f64` | The nugget. |
| `corr` | `SmtCorrelation` | The correlation. |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, theta: &[f64]) -> Result<Option<(f64, LevelPar)>> { /* ... */ }
  ```
  `_reduced_likelihood_function`: the reduced log10 likelihood and the

- ```rust
  pub fn gradient(self: &Self, theta: &[f64]) -> Result<Option<(f64, Vec<f64>)>> { /* ... */ }
  ```
  `_reduced_likelihood_gradient`: `∂ rlf / ∂ θ` (linear θ), with SMT's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `LevelPar`

What `LikelihoodEvaluator.evaluate` puts in `par`.

```rust
pub struct LevelPar {
    pub sigma2: f64,
    pub beta: Vec<f64>,
    pub gamma: Vec<f64>,
    pub c: Vec<Vec<f64>>,
    pub ft: Vec<Vec<f64>>,
    pub q: Vec<Vec<f64>>,
    pub g: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sigma2` | `f64` | Process variance, **in output units** (SMT multiplies by `y_std²`). |
| `beta` | `Vec<f64>` | GLS weights. |
| `gamma` | `Vec<f64>` | `R⁻¹ (y − F β)`. |
| `c` | `Vec<Vec<f64>>` | Lower Cholesky factor of `R`. |
| `ft` | `Vec<Vec<f64>>` | `C⁻¹ F`, `n × (q + p)`. |
| `q` | `Vec<Vec<f64>>` | `Q` of `Ft = Q G`. |
| `g` | `Vec<Vec<f64>>` | `G` of `Ft = Q G`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SmtOptimizer`

How the hyperparameters are chosen.

```rust
pub enum SmtOptimizer {
    MaximiseLikelihood,
    None,
}
```

##### Variants

###### `MaximiseLikelihood`

Maximise the reduced likelihood over log10 θ within the bounds, from
every start (SMT's TNC multistart; here this crate's bounded BFGS).

###### `None`

Keep θ0 exactly (SMT's `hyper_opt = "NoOp"`, except that SMT returns
`10**log10(θ0)`, which can differ from θ0 by an ulp).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SmtStarts`

Starting points of the multistart, in log10 θ.

```rust
pub enum SmtStarts {
    Default {
        n_start: usize,
        seed: i64,
    },
    Explicit(Vec<Vec<f64>>),
}
```

##### Variants

###### `Default`

SMT's arrangement: log10 θ0, one uniform draw in the log10 bounds, and
an `n_start`-point Latin hypercube, drawn from this crate's
`petir::rng::lcg` streams (`seed`), not numpy's PCG64: the points are
not SMT's.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `n_start` | `usize` | SMT's `n_start` (10). |
| `seed` | `i64` | Master seed of the two designs. |

###### `Explicit`

Given starts (log10 θ rows), e.g. the ones SMT recorded. For KPLSK,
rows whose length is the input dimension are used in the second pass
(after the projected θ0), the others in the first.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<Vec<f64>>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn smt_default() -> Self { /* ... */ }
  ```
  SMT's default: `n_start = 10`, here with seed 41 (SMT's numpy seed,

- ```rust
  pub fn rows(self: &Self, theta0_log: &[f64], log_bounds: (f64, f64), lhs: bool) -> Result<Vec<Vec<f64>>> { /* ... */ }
  ```
  The start rows for `m` hyperparameters, `theta0_log` first; `lhs`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `standardization`

**Attributes:**

- `MustUse { reason: None }`

`standardization`: column means and sample stds (`ddof = 1`) of X, a std
below 100 ε replaced by 1; the mean and sample std of y, a zero std
replaced by 1. Returns `(x_offset, x_scale, y_mean, y_std)`.

```rust
pub fn standardization(x: &[Vec<f64>], y: &[f64]) -> (Vec<f64>, Vec<f64>, f64, f64) { /* ... */ }
```

#### Function `maximise`

The multistart: from every start, the bounded BFGS on `−rlf(10^t)` with
SMT's gradient by the chain rule; the best end point (lowest `−rlf`, the
first on a tie, as SMT's `result["fun"] < best["fun"]`). Starts whose
likelihood is `−inf` are skipped. `None` if every start fails.

# Errors
[`Level::evaluate`]'s ill-conditioned-F error.

```rust
pub fn maximise(level: &Level, starts: &[Vec<f64>], log_bounds: (f64, f64)) -> crate::Result<Option<Vec<f64>>> { /* ... */ }
```

## Module `krg`

**SMT's KRG, KPLS and KPLSK** (gh:#1190): ordinary / universal kriging
with SMT's kernels, trends, nugget, θ bounds and normalisation, and its
partial-least-squares reductions. One fitted model per output.

```rust
pub mod krg { /* ... */ }
```

### Types

#### Enum `SmtPls`

Which partial-least-squares reduction, if any.

```rust
pub enum SmtPls {
    None,
    Kpls {
        n_comp: usize,
    },
    Kplsk {
        n_comp: usize,
    },
}
```

##### Variants

###### `None`

KRG: one θ per input.

###### `Kpls`

KPLS: one θ per PLS component (`squar_exp` or `abs_exp`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `n_comp` | `usize` | Number of components (SMT's `n_comp`, default 1). |

###### `Kplsk`

KPLSK: KPLS's optimum projected to one θ per input, then a full KRG
optimisation from it (`squar_exp` only).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `n_comp` | `usize` | Number of components of the first pass. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `TrendVariance`

The trend term of the predictive variance.

```rust
pub enum TrendVariance {
    AsSmt,
    Exact,
}
```

##### Variants

###### `AsSmt`

SMT's KRG: `u = diag(G)⁻¹ (Ftᵀ rt − f)`, exact for a constant trend,
not for `linear` / `quadratic` (V&V record, finding 1).

###### `Exact`

`u = G⁻ᵀ (Ftᵀ rt − f)`, the universal-kriging variance.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SmtKrgConfig`

SMT's KRG-family options.

```rust
pub struct SmtKrgConfig {
    pub corr: super::core::SmtCorrelation,
    pub poly: super::core::SmtRegression,
    pub theta0: Vec<f64>,
    pub theta_bounds: (f64, f64),
    pub nugget: f64,
    pub optimizer: super::core::SmtOptimizer,
    pub starts: super::core::SmtStarts,
    pub pls: SmtPls,
    pub trend_variance: TrendVariance,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `corr` | `super::core::SmtCorrelation` | `corr` (default `squar_exp`). |
| `poly` | `super::core::SmtRegression` | `poly` (default `constant`). |
| `theta0` | `Vec<f64>` | `theta0` (default `[1e-2]`, broadcast); with [`SmtOptimizer::None`]<br>the θ used, exactly. |
| `theta_bounds` | `(f64, f64)` | `theta_bounds` (default `[1e-6, 20]`). |
| `nugget` | `f64` | `nugget` (default `100 ε`). |
| `optimizer` | `super::core::SmtOptimizer` | Optimise or keep θ0. |
| `starts` | `super::core::SmtStarts` | Multistart points. |
| `pls` | `SmtPls` | KRG, KPLS or KPLSK. |
| `trend_variance` | `TrendVariance` | The variance's trend term. |

##### Implementations

###### Methods

- ```rust
  pub fn smt_defaults() -> Self { /* ... */ }
  ```
  SMT's `KRG()` defaults.

- ```rust
  pub fn at_theta(theta: Vec<f64>) -> Self { /* ... */ }
  ```
  The defaults at a fixed θ (SMT's `hyper_opt = "NoOp"`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SmtKriging`

A fitted SMT kriging model of one output.

```rust
pub struct SmtKriging {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn fit(config: &SmtKrgConfig, x: &[Vec<f64>], y: &[f64]) -> Result<Self> { /* ... */ }
  ```
  Fits one output `y` on inputs `x` (`n × nx`), as SMT's `train()`.

- ```rust
  pub fn theta(self: &Self) -> &[f64] { /* ... */ }
  ```
  The hyperparameters (SMT's `optimal_theta`).

- ```rust
  pub fn reduced_likelihood(self: &Self) -> f64 { /* ... */ }
  ```
  The reduced log10 likelihood at [`theta`](Self::theta)

- ```rust
  pub fn gls(self: &Self) -> (f64, &[f64], &[f64]) { /* ... */ }
  ```
  σ² in output units, β, γ (`optimal_par`).

- ```rust
  pub fn standardisation(self: &Self) -> (&[f64], &[f64], f64, f64) { /* ... */ }
  ```
  `(X_offset, X_scale, y_mean, y_std)`.

- ```rust
  pub fn coeff_pls(self: &Self) -> Option<&[Vec<f64>]> { /* ... */ }
  ```
  KPLS's `coeff_pls` (`|x_rotations_|`), if a PLS route.

- ```rust
  pub fn predict(self: &Self, x: &[f64]) -> Result<(f64, f64)> { /* ... */ }
  ```
  The predictive mean and variance (SMT's `predict_values`,

- ```rust
  pub fn reduced_likelihood_gradient_at(self: &Self, theta: &[f64]) -> Result<Option<(f64, Vec<f64>)>> { /* ... */ }
  ```
  The reduced likelihood and its gradient at another θ, on the training

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `linalg`

Dense helpers on `Vec<Vec<f64>>` rows: triangular solves, a Householder
QR, a symmetric eigenvalue routine for condition estimates, and a small
inverse.

```rust
pub mod linalg { /* ... */ }
```

### Functions

#### Function `forward`

**Attributes:**

- `MustUse { reason: None }`

Solves `L z = b` for lower-triangular `L`.

```rust
pub fn forward(l: &[Vec<f64>], b: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `backward_lt`

**Attributes:**

- `MustUse { reason: None }`

Solves `Lᵀ x = z` for lower-triangular `L`.

```rust
pub fn backward_lt(l: &[Vec<f64>], z: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `backward_upper`

**Attributes:**

- `MustUse { reason: None }`

Solves `U x = b` for upper-triangular `U`.

```rust
pub fn backward_upper(u: &[Vec<f64>], b: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `forward_ut`

**Attributes:**

- `MustUse { reason: None }`

Solves `Uᵀ x = b` for upper-triangular `U` (a lower solve).

```rust
pub fn forward_ut(u: &[Vec<f64>], b: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `dot`

**Attributes:**

- `MustUse { reason: None }`

`Σ a_i b_i`, in index order.

```rust
pub fn dot(a: &[f64], b: &[f64]) -> f64 { /* ... */ }
```

#### Function `columns`

**Attributes:**

- `MustUse { reason: None }`

Columns of an `n × p` row-major matrix.

```rust
pub fn columns(a: &[Vec<f64>]) -> Vec<Vec<f64>> { /* ... */ }
```

#### Function `householder_qr`

**Attributes:**

- `MustUse { reason: None }`

Economic QR of an `n × p` matrix (`n ≥ p`) by Householder reflections:
`(Q, G)` with `Q` `n × p` (orthonormal columns, row-major) and `G` `p × p`
upper triangular, `A = Q G`. scipy's `qr(mode="economic")`.

```rust
pub fn householder_qr(a: &[Vec<f64>]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) { /* ... */ }
```

#### Function `symmetric_eigenvalues`

**Attributes:**

- `MustUse { reason: None }`

Eigenvalues of a small symmetric matrix (cyclic Jacobi), unordered.

```rust
pub fn symmetric_eigenvalues(a: &[Vec<f64>]) -> Vec<f64> { /* ... */ }
```

#### Function `singular_value_ratio`

**Attributes:**

- `MustUse { reason: None }`

Ratio of the smallest to the largest singular value of `a` (`n × p`),
from the eigenvalues of `aᵀa`: SMT's `sv[-1] / sv[0]` threshold test. Not
an accurate condition number near 1e-8 (squaring), which the thresholds
it feeds (1e-10 on `G`, 1e15 on `F`) do not need to resolve finely.

```rust
pub fn singular_value_ratio(a: &[Vec<f64>]) -> f64 { /* ... */ }
```

#### Function `inverse`

**Attributes:**

- `MustUse { reason: None }`

Inverse of a small square matrix by Gauss–Jordan with partial pivoting;
`None` if singular. Stands in for scipy's `pinv` on a full-rank matrix
(where the two agree) and for `linalg.inv`.

```rust
pub fn inverse(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> { /* ... */ }
```

## Module `mfk`

**SMT's MFK co-kriging** (gh:#1190, fulfilling gh:#1181's machinery):
the recursive autoregressive multi-fidelity model of Le Gratiet (2013),
level by level from the lowest fidelity.

```rust
pub mod mfk { /* ... */ }
```

### Types

#### Struct `MfkConfig`

SMT's MFK options.

```rust
pub struct MfkConfig {
    pub corr: super::core::SmtCorrelation,
    pub poly: super::core::SmtRegression,
    pub rho_regr: super::core::SmtRegression,
    pub theta0: Vec<Vec<f64>>,
    pub theta_bounds: (f64, f64),
    pub nugget: f64,
    pub optimizer: super::core::SmtOptimizer,
    pub starts: Vec<super::core::SmtStarts>,
    pub propagate_uncertainty: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `corr` | `super::core::SmtCorrelation` | `corr` (default `squar_exp`). |
| `poly` | `super::core::SmtRegression` | `poly` (default `constant`). |
| `rho_regr` | `super::core::SmtRegression` | `rho_regr` (default `constant`). |
| `theta0` | `Vec<Vec<f64>>` | θ0 per level (one row, broadcast to every level, or one per level;<br>each row length 1 or `nx`); default `[[1e-2]]`. |
| `theta_bounds` | `(f64, f64)` | `theta_bounds` (default `[1e-6, 20]`). |
| `nugget` | `f64` | `nugget` (default `100 ε`). |
| `optimizer` | `super::core::SmtOptimizer` | Optimise or keep θ0. |
| `starts` | `Vec<super::core::SmtStarts>` | Starts per level (one entry, used for every level with the seed<br>offset by the level index, or one per level). |
| `propagate_uncertainty` | `bool` | `propagate_uncertainty` (default true). |

##### Implementations

###### Methods

- ```rust
  pub fn smt_defaults() -> Self { /* ... */ }
  ```
  SMT's `MFK()` defaults.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Mfk`

A fitted MFK model of one output.

```rust
pub struct Mfk {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn fit(config: &MfkConfig, levels: &[(Vec<Vec<f64>>, Vec<f64>)]) -> Result<Self> { /* ... */ }
  ```
  Fits the levels `(x_l, y_l)`, lowest fidelity first, as SMT's MFK

- ```rust
  pub fn level_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of levels.

- ```rust
  pub fn level(self: &Self, lvl: usize) -> Option<(&[f64], f64, f64, &[f64], &[f64])> { /* ... */ }
  ```
  Level `lvl`'s θ, reduced log10 likelihood, σ² (output units), β, γ.

- ```rust
  pub fn standardisation(self: &Self) -> (&[f64], &[f64], f64, f64) { /* ... */ }
  ```
  `(X_offset, X_scale, y_mean, y_std)` over all levels concatenated.

- ```rust
  pub fn predict_all_levels(self: &Self, x: &[f64]) -> Result<(f64, Vec<f64>, Vec<f64>)> { /* ... */ }
  ```
  The predictive mean at the highest level, and the variance of every

- ```rust
  pub fn predict(self: &Self, x: &[f64]) -> Result<(f64, f64)> { /* ... */ }
  ```
  The highest level's predictive mean and variance (SMT's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `pls`

`PLSRegression(n_comp).fit(X, y).x_rotations_` for a single output.

```rust
pub mod pls { /* ... */ }
```

### Functions

#### Function `x_rotations`

sklearn's `x_rotations_` (`nx × n_comp`) of `PLSRegression(n_comp)`
fitted to `x` (`n × nx`) and one output `y`.

# Errors
`n_comp` above `min(n, nx)` (sklearn's bound), fewer than two rows, or a
singular `Pᵀ W`.

```rust
pub fn x_rotations(x: &[Vec<f64>], y: &[f64], n_comp: usize) -> crate::Result<Vec<Vec<f64>>> { /* ... */ }
```

### Re-exports

#### Re-export `SmtCorrelation`

```rust
pub use self::core::SmtCorrelation;
```

#### Re-export `SmtOptimizer`

```rust
pub use self::core::SmtOptimizer;
```

#### Re-export `SmtRegression`

```rust
pub use self::core::SmtRegression;
```

#### Re-export `SmtStarts`

```rust
pub use self::core::SmtStarts;
```

#### Re-export `SmtKriging`

```rust
pub use krg::SmtKriging;
```

#### Re-export `SmtKrgConfig`

```rust
pub use krg::SmtKrgConfig;
```

#### Re-export `SmtPls`

```rust
pub use krg::SmtPls;
```

#### Re-export `TrendVariance`

```rust
pub use krg::TrendVariance;
```

#### Re-export `Mfk`

```rust
pub use mfk::Mfk;
```

#### Re-export `MfkConfig`

```rust
pub use mfk::MfkConfig;
```

## Module `validation`

**Leave-one-out cross-validation and jackknife+ prediction intervals** for
[`PolynomialSurrogate`].

# Why held-out error, not `R²` on the fit

A surrogate's error on the points it was fitted to measures how well it
*memorised* them. The question a sweep needs answered is how wrong it is
**between** them. Leave-one-out (LOO) refits `n` times, each time without
one point, and records the error at the point left out — every residual is
a genuine prediction.

# Prediction intervals: jackknife+

For a new input `x`, with `μ_{−i}` the model fitted without point `i` and
`R_i = |y_i − μ_{−i}(x_i)|` its LOO residual, the jackknife+ interval at
miscoverage `α` is

```text
[ q⁻_α { μ_{−i}(x) − R_i },  q⁺_α { μ_{−i}(x) + R_i } ]
```

where `q⁺_α` is the `⌈(1 − α)(n + 1)⌉`-th smallest value and `q⁻_α` the
`⌊α(n + 1)⌋`-th smallest. Barber, Candès, Ramdas and Tibshirani (2021)
prove coverage of at least `1 − 2α` for exchangeable data **with no
assumption on the model** — which is the point: the guarantee does not
depend on the polynomial being the right shape. When `n` is too small for
the order statistic to exist the bound is infinite, and is returned as
such rather than invented.

# What the interval covers

The data `y_i` here are Monte Carlo results, each carrying its own
statistical noise. The interval therefore covers **a new noisy run** at
`x`, not the noise-free truth, and it can be no narrower than the run-to-
run scatter. Callers that need a statement about the noise-free mean must
say so and handle it themselves.

# Reference

R. F. Barber, E. J. Candès, A. Ramdas and R. J. Tibshirani (2021).
Predictive inference with the jackknife+. *Annals of Statistics, 49*(1),
486–507. doi:10.1214/20-AOS1965

```rust
pub mod validation { /* ... */ }
```

### Types

#### Struct `LeaveOneOut`

Leave-one-out models and residuals for a polynomial surrogate.

```rust
pub struct LeaveOneOut {
    pub full: super::PolynomialSurrogate,
    pub models: Vec<super::PolynomialSurrogate>,
    pub residuals: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `full` | `super::PolynomialSurrogate` | The fit to all points — the model to evaluate for a point estimate. |
| `models` | `Vec<super::PolynomialSurrogate>` | `models[i]` was fitted without point `i`. |
| `residuals` | `Vec<f64>` | `residuals[i] = y_i − models[i](x_i)` (signed). |

##### Implementations

###### Methods

- ```rust
  pub fn fit(inputs: &[Vec<f64>], outputs: &[f64], degree: usize, ridge: f64) -> Result<Self> { /* ... */ }
  ```
  Fit the full model and the `n` leave-one-out models.

- ```rust
  pub fn rmse(self: &Self) -> f64 { /* ... */ }
  ```
  Root-mean-square LOO residual.

- ```rust
  pub fn chi2_per_point(self: &Self, sigmas: &[f64]) -> Result<f64> { /* ... */ }
  ```
  `Σ (r_i / σ_i)² / n` — the LOO residuals in units of each point's own

- ```rust
  pub fn jackknife_plus(self: &Self, x: &[f64], alpha: f64) -> Result<(f64, f64)> { /* ... */ }
  ```
  Jackknife+ interval at `x` with miscoverage `alpha` (coverage at least

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `GaussianProcess`

```rust
pub use gaussian_process::GaussianProcess;
```

#### Re-export `GpConfig`

```rust
pub use gaussian_process::GpConfig;
```

#### Re-export `GpKernel`

```rust
pub use gaussian_process::GpKernel;
```

#### Re-export `GpOptimizer`

```rust
pub use gaussian_process::GpOptimizer;
```

#### Re-export `RavenKernel`

```rust
pub use gaussian_process::RavenKernel;
```

#### Re-export `MultiOutputPolynomial`

```rust
pub use polynomial::MultiOutputPolynomial;
```

#### Re-export `PolynomialSurrogate`

```rust
pub use polynomial::PolynomialSurrogate;
```

#### Re-export `PolynomialChaos`

```rust
pub use polynomial_chaos::PolynomialChaos;
```

#### Re-export `PolynomialChaosSpec`

```rust
pub use polynomial_chaos::PolynomialChaosSpec;
```

#### Re-export `Mfk`

```rust
pub use smt::Mfk;
```

#### Re-export `MfkConfig`

```rust
pub use smt::MfkConfig;
```

#### Re-export `SmtKriging`

```rust
pub use smt::SmtKriging;
```

#### Re-export `SmtKrgConfig`

```rust
pub use smt::SmtKrgConfig;
```

## Types

### Enum `RafflesError`

Errors produced by RAFFLES.

Deliberately small: these are the two failure shapes every planned module
needs (a caller-supplied parameter that cannot describe a valid
distribution or design, and a shape mismatch between arrays). Variants are
added as real code lands — this enum is scaffold, not a finished taxonomy.

No variant here signals "unimplemented". The unimplemented parts of RAFFLES
have no public entry point at all, so a caller cannot reach one by
accident.

```rust
pub enum RafflesError {
    InvalidParameter {
        parameter: String,
        value: f64,
        reason: String,
    },
    DimensionMismatch {
        expected: usize,
        found: usize,
    },
    ModelEvaluation {
        model: String,
        reason: String,
    },
}
```

#### Variants

##### `InvalidParameter`

A caller-supplied parameter is outside the range the model admits —
for example a negative standard deviation, or a probability outside
`[0, 1]`.

`parameter` names the offending argument as the caller wrote it,
`value` is what was passed, and `reason` states the constraint that was
violated.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `parameter` | `String` | Name of the offending parameter, as it appears in the public API. |
| `value` | `f64` | The value that was rejected. |
| `reason` | `String` | The constraint it violated, phrased for a human reader. |

##### `DimensionMismatch`

Two arrays that had to agree in length or dimension did not — for
example a sample matrix whose column count does not match the number of
input variables.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `expected` | `usize` | The length or dimension that was required. |
| `found` | `usize` | The length or dimension actually supplied. |

##### `ModelEvaluation`

A caller's model, evaluated through [`fidelity::HighFidelityModel`]
or [`fidelity::ParametricModel`], failed (added 2026-11-04,
gh:#1194). The implementing crate maps its own error into this.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `model` | `String` | Which model, as its implementer names it. |
| `reason` | `String` | Its own error message. |

#### Implementations

##### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Move**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Type Alias `Result`

Convenient alias for a fallible RAFFLES result.

```rust
pub type Result<T> = core::result::Result<T, RafflesError>;
```

## Macros

### Macro `quantities`

**Attributes:**

- `MacroExport`

Defines a struct of uom fields and derives [`Quantities`] for it.

Attributes (derives, doc comments) pass through to the struct and its
fields; the struct must be `Clone` (derive it). Every field type must be a
`uom::si::Quantity<_, _, f64>`, e.g. `uom::si::f64::Length` or a named
alias of one.

```
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::millimeter;
use uom::si::thermodynamic_temperature::degree_celsius;
use raffles::fidelity::Quantities;

raffles::quantities! {
    /// A pipe at a temperature.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Pipe {
        /// Bore.
        pub diameter: Length,
        /// Wall temperature (absolute).
        pub wall: ThermodynamicTemperature,
    }
}

let p = Pipe {
    diameter: Length::new::<millimeter>(25.0),
    wall: ThermodynamicTemperature::new::<degree_celsius>(80.0),
};
let si = p.to_si(); // metres and kelvin
assert_eq!(Pipe::from_si(&si)?, p);
assert_eq!(Pipe::channels()[1].unit, "K");
# Ok::<(), raffles::RafflesError>(())
```

```rust
pub macro_rules! quantities {
    /* macro_rules! quantities {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $( $(#[$fmeta:meta])* $fvis:vis $field:ident : $ty:ty ),+ $(,)?
        }
    ) => { ... };
} */
}
```

