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

## Status — SCAFFOLD ONLY

Nothing is implemented. Every module below is an empty, documented
placeholder that states its own scope. No distribution, sampler, estimator
or surrogate exists yet, and none of this has been through human V&V. Do
not describe any part of this crate as working, verified or validated.

## What belongs in this crate

The *statistical machinery* for running ensembles of simulations and
reasoning about the spread of their answers:

- **[`distributions`]** — probability distributions: densities, cumulative
  distribution functions, inverse CDFs, analytic moments.
- **[`samplers`]** — strategies that turn distributions into a concrete set
  of sample points: Monte Carlo, Latin hypercube, grid / stratified.
- **[`sensitivity`]** — importance measures computed from an existing
  sample set: Sobol variance decomposition, correlation coefficients.
- **[`surrogate`]** — reduced-order models fitted to a sample set and
  evaluated in place of the expensive simulation.

## What does NOT belong in this crate

- **Physics.** RAFFLES holds no reactor, thermal-hydraulic, neutronic or
  chemistry model. It samples inputs and consumes outputs; the physics
  lives in the other Outram Park crates.
- **Simulation drivers, job scheduling, file/XML input parsing, plotting,
  databases.** RAVEN is a whole workflow application; RAFFLES ports only
  its statistical core. A caller drives their own runs and hands RAFFLES
  arrays of numbers.
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
    fn clone(self: &Self) -> Uniform { /* ... */ }
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
    fn eq(self: &Self, other: &Uniform) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Normal { /* ... */ }
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
    fn eq(self: &Self, other: &Normal) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> LogNormal { /* ... */ }
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
    fn eq(self: &Self, other: &LogNormal) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Triangular { /* ... */ }
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
    fn eq(self: &Self, other: &Triangular) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Exponential { /* ... */ }
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
    fn eq(self: &Self, other: &Exponential) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Weibull { /* ... */ }
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
    fn eq(self: &Self, other: &Weibull) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Gamma { /* ... */ }
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
    fn eq(self: &Self, other: &Gamma) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Beta { /* ... */ }
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
    fn eq(self: &Self, other: &Beta) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Distribution { /* ... */ }
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
    fn eq(self: &Self, other: &Distribution) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Truncated { /* ... */ }
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
    fn eq(self: &Self, other: &Truncated) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> MonteCarlo { /* ... */ }
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
    fn eq(self: &Self, other: &MonteCarlo) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> LatinHypercube { /* ... */ }
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
    fn eq(self: &Self, other: &LatinHypercube) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> GridSampler { /* ... */ }
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
    fn eq(self: &Self, other: &GridSampler) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> Sampler { /* ... */ }
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
    fn eq(self: &Self, other: &Sampler) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> CorrelationKind { /* ... */ }
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
    fn eq(self: &Self, other: &CorrelationKind) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> SobolSampleLayout { /* ... */ }
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
    fn eq(self: &Self, other: &SobolSampleLayout) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> SobolIndices { /* ... */ }
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
    fn eq(self: &Self, other: &SobolIndices) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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

**UNIMPLEMENTED, AND NO WORK IS SCHEDULED.** This module is a placeholder
for a planned capability. It is here so the crate's intended shape is
visible, not because anything is in progress. Nothing is exported from it,
so there is no public path a caller can reach.

## Scope — what would belong here

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
    fn clone(self: &Self) -> RafflesError { /* ... */ }
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
    fn eq(self: &Self, other: &RafflesError) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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

