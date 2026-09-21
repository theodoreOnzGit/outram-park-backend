# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `raffles`

# RAFFLES — Risk Analysis Framework For Learning & Ensemble Simulation

An independent pure-Rust port of the uncertainty-quantification (UQ) and
risk-analysis core of [RAVEN](https://github.com/idaholab/raven), the
probabilistic risk-analysis / UQ framework developed by Idaho National
Laboratory. Upstream is Apache-2.0; RAFFLES is GPL-3.0-only. That direction
is **one-way** — see [Licensing](#licensing) below.

**Owner: Adolphus Lye.** They chose the RAFFLES backronym and this crate is
theirs to steer; changes of direction are their call. See the crate
`CLAUDE.md`.

## Status — PARTLY IMPLEMENTED, NO HUMAN V&V

This crate is no longer the empty scaffold its first commits described.
Every module below — [`distributions`], [`samplers`], [`sensitivity`],
[`bayesian`], [`distance`], [`abc`], [`imprecise`], [`model_selection`],
[`scram`], [`gnn`] and [`surrogate`] — carries working, unit-tested code
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
  arrays of numbers. [`scram`] holds to the same line: it takes a fault
  tree a caller has built in Rust, never a SCRAM input model.
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

Upstream RAVEN is **Apache-2.0**; RAFFLES is **GPL-3.0-only**. Apache-2.0
code may be taken into a GPLv3 work, but GPLv3 code may **not** be taken
into an Apache-2.0 work. Code therefore flows RAVEN -> RAFFLES and never
RAFFLES -> RAVEN. Do not contribute RAFFLES code upstream. Full provenance,
the required Battelle Energy Alliance / Idaho National Laboratory
attribution, and the verbatim upstream licence text are in the crate's
`NOTICE`, `LICENSE-APACHE-RAVEN` and `NOTICE-RAVEN`.

**RAVEN is not the only upstream, and the others are not Apache-2.0.**
[`scram`] derives from [SCRAM](https://github.com/rakhimov/scram) and parts
of [`bayesian`] and [`gnn`] from other projects, all **GPL-3.0**, so none
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

With the sample sizes matched, tightening the tolerance walks the posterior
sd down towards the exact value and stops there rather than sailing past it:
0.5677 at `epsilon = 0.5`, 0.3464 at 0.2, 0.3110 at 0.05, against the exact
0.288195. That saturation is what a correctly-posed ABC problem looks like.

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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Re-exports

#### Re-export `EnsembleMove`

```rust
pub use mcmc::EnsembleMove;
```

#### Re-export `MetropolisHastings`

```rust
pub use mcmc::MetropolisHastings;
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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
batching — and is likewise `burn`-free. [`mc_geometry`] builds that topology
from an `outram-mc-libs` CSG geometry; the matching bridge for granular DEM
lives in `outram-park-fork-liggghts` behind its `gnn` feature, because the
dependency only runs one way.

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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `mc_geometry`

Building a message-passing graph from a Monte Carlo CSG geometry.

# Where this lives, and why

In `raffles`, not in `outram-mc-libs`. RAFFLES already depends on
`outram-mc-libs` for its random-number generator, so an adapter in the other
direction would make the two crates mutually dependent and neither would
compile. The DEM bridge went the other way — it lives in
`outram-park-fork-liggghts`, which nothing here depends on — and the
asymmetry is the dependency graph's, not a design preference.

# The graph

Nodes are **cells**; two cells are joined when they share a surface with
**opposite senses** — one on the inside, the other on the outside. That is
the standard cheap construction of a CSG adjacency graph, and it is what a
particle's possible next cell looks like from the current one.

## What it gets wrong, stated plainly

This is a **superset** of true geometric adjacency, in two ways:

- Two cells can reference the same surface with opposite senses and still
  not touch, because some *other* surface in one of their region
  expressions separates them. The graph will join them anyway.
- A cell defined by a union can have disconnected pieces, and this treats it
  as one node.

It is never a *subset*: two genuinely adjacent cells always share a surface
with opposite senses, so no real adjacency is missed. That direction is the
one that matters for a reach bound — an over-connected graph gives a
diameter that is too small, and therefore a bound that is too *low*, which
is the unsafe direction. **Treat a diameter computed from this graph as a
lower bound on the true one**, and say so in anything reported from it.

Exact adjacency needs surface-surface intersection tests against the full
region expressions, which is a real geometry kernel and is not what this
module is.

# What it is for

Two uses, both from the literature on learned variance reduction:

- **Importance and weight-window maps.** These are currently produced by a
  deterministic adjoint solve or by iterating Monte Carlo. A network over
  the cell graph is a candidate, and the cell graph is small and static so
  the training cost is dominated by generating targets, not by the graph.
- **Cell-wise response surrogates** — a predicted reaction rate or leakage
  per cell.

One caution worth stating up front: a Monte Carlo target carries a
statistical uncertainty, and a surrogate fitted to noisy targets inherits it
without reporting it. Any such surrogate needs its own uncertainty story
before it goes anywhere near a k-eff.

```rust
pub mod mc_geometry { /* ... */ }
```

### Functions

#### Function `cell_adjacency_graph`

Builds the cell-adjacency graph of a CSG geometry.

Two cells are joined when one references a surface on its inside and the
other references the same surface on its outside. See the module
documentation for exactly how this over-approximates true adjacency, and
why that direction is the unsafe one for a reach bound.

# Errors

[`RafflesError::InvalidParameter`] if `cells` is empty.

```rust
pub fn cell_adjacency_graph(cells: &[outram_mc_libs::geometry::cell::Cell]) -> crate::Result<super::graph::Graph> { /* ... */ }
```

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

#### Re-export `cell_adjacency_graph`

```rust
pub use mc_geometry::cell_adjacency_graph;
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
implemented; see [`SystemStructure::reliability_pbox`] for what that would
take.

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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
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

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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
`outram_mc_libs::rng::lcg`, the workspace's port of OpenMC's 64-bit linear
congruential generator (`src/random_lcg.cpp`). Three reasons it is the right
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
  statistically independent. It is also already tested upstream in
  `outram-mc-libs`.
- **Android-clean.** `outram-mc-libs` target-gates its wgpu/GPU paths off
  Android, so `cargo check -p raffles --all-targets --target
  aarch64-linux-android` stays clean. RAFFLES follows the same gating
  convention if it ever needs something Android-hostile.

`outram_mc_libs::rng::lcg::init_seed` is deliberately **not** used — see
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

# Why not `outram_mc_libs::rng::lcg::init_seed`

That helper computes `future_seed(id + offset, future_seed(DEFAULT_STRIDE,
master))`, i.e. consecutive `id`s land **one LCG step apart**, not one
stride apart. OpenMC's own `init_seed` (`src/random_lcg.cpp:60`) is
`future_seed(id * prn_stride, master_seed + offset)`. Using the workspace
helper for per-dimension streams would therefore make dimension `j+1`'s
draws a one-step shift of dimension `j`'s — near-perfectly correlated
dimensions, and a silently wrong design. This function calls `future_seed`
directly with OpenMC's `id * stride` semantics instead. The discrepancy is
in `outram-mc-libs`, not here, and is reported rather than patched from this
crate.

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

**What is still absent:** everything SCRAM does around this core — XML
input models, event trees, alignments, common-cause-failure groups,
substitutions and the expression library — plus, in the analysis itself,
the preprocessor
(upstream's `--prime-implicants`, which is what recovers the exact function
a non-coherent tree describes).

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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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
  pub fn build(tree: &FaultTree) -> Result<Self> { /* ... */ }
  ```
  Builds the diagram of a fault tree's Boolean function.

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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

Mirrors upstream SCRAM's `pdag.h` `Connective` enum. **Only the coherent
subset is supported by [`super::mocus`]** — the four negating variants are
representable so that a tree containing one can be built and *refused with
a clear message*, rather than being silently unrepresentable.

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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `Arg`

What feeds a gate: another gate, or a basic event.

Both carry an index, not a name — gate indices into [`FaultTree::gates`],
basic-event indices into the probability slice. [`FaultTreeBuilder`] does
the name resolution so a caller need not.

```rust
pub enum Arg {
    Gate(usize),
    BasicEvent(usize),
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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
| `occurrence` | `usize` | How many cut sets contain this event.<br><br>A purely structural count — it ignores probabilities entirely. Upstream<br>calls it `occurrence`. |
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
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

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

  `V_i = (1/n) * sum_j y_B[j] * (y_AB_i[j] - y_A[j])`,  `S_i = V_i / V`

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
| Sudret polynomial, `N = 3` | `S_i = 25/91`, `S_Ti = 36/91` exactly | max abs error `2.681e-4` on `S_i`, `3.266e-4` on `S_Ti` at `n = 65536` |
| Ishigami function | `S = (0.313905, 0.442411, 0)`, `S_T = (0.557589, 0.442411, 0.243684)` | max abs error `3.723e-4` on `S`, `5.105e-5` on `S_T` at `n = 65536` |
| Additive linear model | `S_i = S_Ti = c_i^2 / sum(c^2)`, `sum S_i = 1` | max abs error `2.493e-4`; `sum S_i = 0.999441` |
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
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

Gaussian process regression / kriging and the sparse-grid polynomial chaos
route are **not** implemented. Neither is the cross-validation machinery;
the fitted models report `r_squared` and `rmse` on whatever data they are
given, and the tests here demonstrate why that must be held-out data.

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
`Box<dyn Surrogate>`. No lifetime parameters.

## Verification requirement

A surrogate is not done until it is checked against a function whose exact
answer is known: a polynomial the expansion should reproduce to machine
precision at the right order, and a published test problem (Ishigami,
Sobol g-function, or a standard regression benchmark) with reported error
metrics. Record the methodology and the measured errors.

## Provenance

No RAVEN code has been ported into this module. When it is, each derived
file carries the attribution header shown in the crate `CLAUDE.md`, naming
the upstream file under `ravenframework/SupervisedLearning/`, the commit,
the copyright holder and the licence.

```rust
pub mod surrogate { /* ... */ }
```

### Modules

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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Re-exports

#### Re-export `PolynomialSurrogate`

```rust
pub use polynomial::PolynomialSurrogate;
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

- **CastableFrom**
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Type Alias `Result`

Convenient alias for a fallible RAFFLES result.

```rust
pub type Result<T> = core::result::Result<T, RafflesError>;
```

