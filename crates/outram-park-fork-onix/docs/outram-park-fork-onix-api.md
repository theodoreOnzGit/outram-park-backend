# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `outram_park_fork_onix`

# outram-park-fork-onix

Independent pure-Rust fork/translation of ONIX (open-source depletion/burnup;
MIT upstream) — Bateman/CRAM depletion + fission-product inventory for the
MSRE digital twin. Not affiliated with the ONIX project.

> **⚠️ Untrusted AI-assisted draft — pending human V&V.** This first-pass
> port was produced with AI assistance and is untrusted draft material until
> a human reviews it (see the workspace `RESPONSIBLE_USE.md`). No human V&V.
> MSRE digital-twin epic `op-6w0`, bead `op-6w0.2`. Not for nuclear facility
> operation, reactor control, safety-critical, or licensing decisions.

## What this crate does (stand-alone / precomputed-input mode)

Given a set of nuclides, their decay data (decay constants + branching),
one-group (or few-group collapsed) neutron-reaction rates, and fission
yields, this crate assembles the depletion (Bateman) matrix `A` (units
`1/s`) and computes the depleted inventory `n(Δt) = exp(A·Δt)·n0` using the
**order-16 CRAM** (Chebyshev Rational Approximation Method) solver — the same
algorithm and coefficients ONIX uses in `onix/salameche/cram.py`.

It is a faithful port of ONIX's *depletion-math core* only. See "Scope" below
for what is deliberately **not** ported.

## Quick start

```
use outram_park_fork_onix::{
    DepletionSystem, DecayData, ReactionRates, FissionYields, Nuclide, DecayMode,
};

// Two-step decay chain A -> B -> C (C stable).
let a = Nuclide::new(50, 100, 0);
let b = Nuclide::new(51, 100, 0);
let c = Nuclide::new(52, 100, 0);

let mut sys = DepletionSystem::new();
sys.add_nuclide(a, DecayData::single_mode(1e-2, DecayMode::BetaMinus),
                ReactionRates::none(), FissionYields::empty()).unwrap();
sys.add_nuclide(b, DecayData::single_mode(1e-3, DecayMode::BetaMinus),
                ReactionRates::none(), FissionYields::empty()).unwrap();
sys.add_nuclide(c, DecayData::stable(),
                ReactionRates::none(), FissionYields::empty()).unwrap();

let n0 = sys.inventory_vector(&[(a, 1.0)]).unwrap();
let n = sys.deplete(&n0, 100.0).unwrap(); // deplete 100 s
// n[2] is the C inventory after 100 s.
# assert!(n[2] >= 0.0);
```

## Scope — what is and is NOT ported

**Ported:** nuclide identity ([`Nuclide`]), decay/transmutation channel
identity + daughter lookup ([`DecayMode`], [`ReactionChannel`]), burnup-matrix
assembly ([`DepletionSystem::build_matrix`]), the order-16 CRAM solver
([`cram::cram16`]), and a stand-alone single/multi-step driver
([`DepletionSystem`]).

**NOT ported (out of scope for this first pass):** the OpenMC coupling
(`onix/couple/`); ONIX's nuclide-data libraries (decay, cross section,
fission-yield files under `onix/data/`) — the caller supplies precomputed
data instead; the predictor-corrector / higher-order flux approximations
(`burn_substep_pc`, `burn_substep_pcME4`); the order-48 CRAM (ONIX itself
only ships order-16); the full input/sequence/reporting machinery
(`onix/input.py`, `onix/sequence.py`, `onix/system.py`, `onix/utils/`).

## Modules

## Module `chain`

Nuclide-chain bookkeeping: decay data, reaction rates, and fission yields.

These are the per-nuclide inputs the burnup-matrix assembly
([`crate::matrix`]) consumes. They mirror the data ONIX attaches to each
`passport` (nuclide record) — decay constants + branching, one-group
reaction rates, and fission-yield tables — but in a caller-supplied,
precomputed (stand-alone) form rather than read from ONIX's data libraries.

## Provenance (GPLv3 relicensing of MIT upstream)

Structure mirrors ONIX (open-source, MIT; commit `7328dc6`):
  * `onix/salameche/mat_builder.py:134` (`get_decay_mat` — total decay on
    the diagonal, partial decay constants off-diagonal),
  * `onix/salameche/mat_builder.py:5` (`get_xs_mat` — removal on the
    diagonal, production reaction rates off-diagonal, fission-yield term at
    lines 99–125), and
  * `onix/passport.py` (the per-nuclide `decay_a`, `current_xs`, `fy`
    records).

Independent Rust re-implementation; OUTRAM PARK fork relicenses under
**GPL-3.0-only** (MIT is GPL-3.0-compatible).

```rust
pub mod chain { /* ... */ }
```

### Types

#### Struct `DecayData`

Decay constant of one nuclide plus its branching among decay modes.

* `lambda_total` — total decay constant λ, **units `1/s`** (`λ = ln 2 / t½`).
  A stable nuclide has `lambda_total = 0.0`. Valid range `>= 0`.
* `branches` — `(mode, fraction)` pairs; each `fraction` is the branching
  ratio (dimensionless, in `[0, 1]`) of that mode. Physically the fractions
  sum to ~1, but this is **not** enforced (an incomplete data set may list
  only the tracked modes). The partial decay constant of a mode is
  `lambda_total * fraction` (units `1/s`).

This matches ONIX's `decay_a` dict, whose `'total decay'` entry is the
diagonal loss (`mat_builder.py:162`) and whose per-mode entries are the
off-diagonal production rates (`mat_builder.py:187`).

```rust
pub struct DecayData {
    pub lambda_total: f64,
    pub branches: Vec<(crate::reactions::DecayMode, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lambda_total` | `f64` | Total decay constant λ, units `1/s`. `0.0` ⇒ stable. |
| `branches` | `Vec<(crate::reactions::DecayMode, f64)>` | Branching ratios `(mode, fraction)`; `fraction` dimensionless in `[0,1]`. |

##### Implementations

###### Methods

- ```rust
  pub fn stable() -> Self { /* ... */ }
  ```
  A stable nuclide: zero decay constant, no branches.

- ```rust
  pub fn from_half_life(half_life_s: f64, branches: Vec<(DecayMode, f64)>) -> Self { /* ... */ }
  ```
  Build decay data from a half-life (seconds) and branching ratios.

- ```rust
  pub fn single_mode(lambda: f64, mode: DecayMode) -> Self { /* ... */ }
  ```
  Single-mode decay: total constant λ (`1/s`) with 100 % branching.

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
    fn clone(self: &Self) -> DecayData { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DecayData) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `ReactionRates`

One-group (or few-group collapsed) neutron-reaction rates for a nuclide.

Each entry is `(channel, rate)` where `rate` is the **reaction rate in
`1/s`** — i.e. microscopic cross section σ (barns) × `1e-24` (cm²/barn) ×
scalar flux φ (n·cm⁻²·s⁻¹), already collapsed to one group by the caller.
This is exactly ONIX's `A = B*1e-24*flux + C` construction
(`onix/salameche/burn.py:187`), except the caller supplies the finished
`1/s` rate rather than (σ, φ).

Use [`ReactionRates::from_xs_flux`] if you have barns + flux and want the
`1e-24` conversion done for you.

```rust
pub struct ReactionRates {
    pub channels: Vec<(crate::reactions::ReactionChannel, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `channels` | `Vec<(crate::reactions::ReactionChannel, f64)>` | `(channel, rate_per_second)` — `rate` in `1/s`. |

##### Implementations

###### Methods

- ```rust
  pub fn none() -> Self { /* ... */ }
  ```
  Empty reaction-rate set (nuclide sees no neutron flux / has no cross

- ```rust
  pub fn from_xs_flux(channels_barns: &[(ReactionChannel, f64)], flux: f64) -> Self { /* ... */ }
  ```
  Build rates from one-group cross sections (barns) and a scalar flux.

- ```rust
  pub fn total_removal(self: &Self) -> f64 { /* ... */ }
  ```
  Total removal rate (sum over all channels including fission), units

- ```rust
  pub fn fission_rate(self: &Self) -> f64 { /* ... */ }
  ```
  The fission rate for this nuclide (sum of any fission channels), `1/s`.

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
    fn clone(self: &Self) -> ReactionRates { /* ... */ }
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
    fn default() -> ReactionRates { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReactionRates) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `FissionYields`

Fission-yield table for one fissile parent.

`products` are `(product, yield_fraction)` where `yield_fraction` is the
number of that product produced **per fission** (dimensionless, atoms per
fission). ONIX stores yields in **percent** and multiplies by `1e-2`
(`mat_builder.py:121`); here the caller supplies the already-fractional
yield (atoms/fission), so no `1e-2` is applied. Cumulative or independent
yields may be used depending on how the chain is modelled — that choice is
the caller's.

```rust
pub struct FissionYields {
    pub products: Vec<(crate::nuclide::Nuclide, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `products` | `Vec<(crate::nuclide::Nuclide, f64)>` | `(product_nuclide, atoms_per_fission)`; `atoms_per_fission` dimensionless. |

##### Implementations

###### Methods

- ```rust
  pub fn empty() -> Self { /* ... */ }
  ```
  An empty yield table.

- ```rust
  pub fn from_percent(products_percent: &[(Nuclide, f64)]) -> Self { /* ... */ }
  ```
  Build a yield table from percent yields (ONIX's native units).

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
    fn clone(self: &Self) -> FissionYields { /* ... */ }
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
    fn default() -> FissionYields { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FissionYields) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
## Module `cram`

CRAM — the Chebyshev Rational Approximation Method matrix-exponential solver.

CRAM computes the action of the matrix exponential
`n(Δt) = exp(A·Δt) · n0` — the exact solution of the Bateman depletion
system `dn/dt = A·n` for a constant burnup matrix `A` (units `1/s`) over a
timestep `Δt` (units `s`). It is the standard high-accuracy depletion solver
(Pusa & Leppänen); ONIX uses the order-16 variant in
`onix/salameche/cram.py`.

## Method (order-16 partial-fraction / incomplete-pole form)

The rational approximation `r(x) ≈ exp(x)` of order 16 is written as a sum
over its 8 conjugate pole pairs:

```text
  exp(A·Δt)·n0 ≈ α0·n0 + 2·Re{ Σ_{k=1..8} α_k · (A·Δt − θ_k·I)^{-1} · n0 }
```

Each term is one **complex resolvent solve**: `(A·Δt − θ_k·I) y_k = α_k·n0`.
Taking twice the real part of the 8 upper-half-plane poles accounts for
their complex-conjugate partners. `α0` is the value of the approximation at
infinity. This is verbatim the algorithm in ONIX `CRAM16`
(`onix/salameche/cram.py:6–59`).

## Provenance (GPLv3 relicensing of MIT upstream)

The θ_k, α_k, and α0 coefficients below are copied **digit-for-digit** from
ONIX (open-source, MIT; commit `7328dc6`), `onix/salameche/cram.py`:
  * `theta` array — `cram.py:22–30`,
  * `alpha_0`      — `cram.py:32`,
  * `alpha` array  — `cram.py:34–42`,
  * solve loop     — `cram.py:44–59`.

ONIX in turn takes these from the CRAM literature (Pusa, "Rational
Approximations to the Matrix Exponential in Burnup Calculations", *Nucl.
Sci. Eng.* 169 (2011) 155–167). ONIX stores them at `complex256` precision;
we hold `f64`, which is the working precision of the solve. Independent Rust
re-implementation; OUTRAM PARK fork relicenses under **GPL-3.0-only** (MIT is
GPL-3.0-compatible).

```rust
pub mod cram { /* ... */ }
```

### Types

#### Enum `CramError`

Errors raised by the CRAM solver.

```rust
pub enum CramError {
    DimensionMismatch {
        expected: usize,
        got: usize,
    },
    SingularResolvent {
        column: usize,
    },
}
```

##### Variants

###### `DimensionMismatch`

The initial-inventory vector length did not equal the matrix dimension.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `expected` | `usize` | Matrix dimension `n`. |
| `got` | `usize` | Length of the supplied `n0`. |

###### `SingularResolvent`

A complex resolvent `(A·Δt − θ_k·I)` was singular to working precision —
Gaussian elimination found a zero pivot. This should not happen for a
physical depletion matrix (the θ_k sit off the real axis), so it usually
signals a malformed matrix.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `column` | `usize` | The column at which no nonzero pivot could be found. |

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

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: CramError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CramError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
### Functions

#### Function `cram16`

Order-16 CRAM: `n(Δt) = exp(A·Δt)·n0`.

* `a` — burnup matrix `A` (units `1/s`), see [`BurnupMatrix`].
* `dt` — timestep Δt in **seconds** (`>= 0`).
* `n0` — initial number densities (atoms, or atoms·cm⁻³); length must equal
  `a.dim()`.

Returns the depleted inventory `n(Δt)` in the same units as `n0`. Negative
entries (a known CRAM artefact for species that should be exactly zero) are
**not** clamped here — see [`clamp_nonnegative`] for the optional
physicality filter ONIX applies in `CRAM_density_check`
(`onix/salameche/cram.py:127`).

Algorithm ported from ONIX `CRAM16` (`onix/salameche/cram.py:6–59`).

```rust
pub fn cram16(a: &crate::matrix::BurnupMatrix, dt: f64, n0: &[f64]) -> Result<Vec<f64>, CramError> { /* ... */ }
```

#### Function `clamp_nonnegative`

Clamp physically-impossible values to zero, mirroring ONIX
`CRAM_density_check` (`onix/salameche/cram.py:127–163`).

CRAM produces small negative densities for species that should be exactly
zero (an artefact of the rational approximation); these have no physical
meaning. This filter sets any entry `< threshold` (including negatives) to
`0.0`. ONIX uses `threshold = 1e-24` atoms·cm⁻³ by default. Pass the
threshold in the same units as the inventory. Mutates `n` in place and
returns the count of entries zeroed.

```rust
pub fn clamp_nonnegative(n: &mut [f64], threshold: f64) -> usize { /* ... */ }
```

## Module `driver`

Stand-alone depletion driver — assemble the burnup matrix and deplete.

This is the precomputed-input (stand-alone) mode of ONIX: the caller
supplies decay data, one-group reaction rates, fission yields, and an
initial inventory; the driver assembles the burnup matrix `A` and advances
the inventory over one or more timesteps with [`crate::cram::cram16`]. There
is **no** neutron-transport / OpenMC coupling here — reaction rates are
taken as given (see the crate-level scope notes).

## Provenance (GPLv3 relicensing of MIT upstream)

Driver flow mirrors ONIX (open-source, MIT; commit `7328dc6`):
  * matrix assembly — `onix/salameche/mat_builder.py` (`get_xs_mat`,
    `get_decay_mat`, `get_initial_vect`),
  * `A = B·1e-24·φ + C`, `At = A·Δt`, `CRAM16(At, N)` — `onix/salameche/
    burn.py:187–194` (`burn_microstep`),
  * multi-step loop — `onix/salameche/burn.py:69` (`burn_cell`) over
    macrosteps.

Independent Rust re-implementation; OUTRAM PARK fork relicenses under
**GPL-3.0-only** (MIT is GPL-3.0-compatible).

```rust
pub mod driver { /* ... */ }
```

### Types

#### Type Alias `NuclideIndex`

A nuclide's index in the depletion vector (its row/column in the matrix).

```rust
pub type NuclideIndex = usize;
```

#### Enum `DepletionError`

Errors from building or running a [`DepletionSystem`].

```rust
pub enum DepletionError {
    DuplicateNuclide {
        zamid: crate::nuclide::ZamId,
    },
    UnknownNuclide {
        zamid: crate::nuclide::ZamId,
    },
    Cram(crate::cram::CramError),
}
```

##### Variants

###### `DuplicateNuclide`

The same nuclide was registered twice.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `zamid` | `crate::nuclide::ZamId` | The offending packed id. |

###### `UnknownNuclide`

A supplied initial-inventory map referenced a nuclide not in the system.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `zamid` | `crate::nuclide::ZamId` | The offending packed id. |

###### `Cram`

The CRAM solve failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::cram::CramError` |  |

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

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private18::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: CramError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DepletionError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Struct `DepletionSystem`

A stand-alone depletion system: a fixed set of nuclides plus their decay
data, reaction rates, and fission yields.

The nuclide set fixes the matrix ordering: nuclide registered `k`-th occupies
row/column `k`. Number densities are carried in whatever unit the caller uses
for the initial inventory (atoms, or atoms·cm⁻³); the driver is unit-agnostic
on the inventory as long as it is consistent.

Reaction rates are held **separately from decay data** so they can be
replaced between burnup steps (changing flux/spectrum) while the decay data
stays fixed — see [`DepletionSystem::set_reaction_rates`] and
[`DepletionSystem::deplete_multi`].

```rust
pub struct DepletionSystem {
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
  An empty system with no nuclides.

- ```rust
  pub fn add_nuclide(self: &mut Self, nuclide: Nuclide, decay: DecayData, rates: ReactionRates, fission_yields: FissionYields) -> Result<NuclideIndex, DepletionError> { /* ... */ }
  ```
  Register a nuclide with its decay data, reaction rates, and fission

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of nuclides in the system (matrix dimension).

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the system holds no nuclides.

- ```rust
  pub fn nuclides(self: &Self) -> &[Nuclide] { /* ... */ }
  ```
  The nuclides in matrix order.

- ```rust
  pub fn index_of(self: &Self, nuclide: Nuclide) -> Option<NuclideIndex> { /* ... */ }
  ```
  The index of a nuclide, or `None` if it is not tracked.

- ```rust
  pub fn set_reaction_rates(self: &mut Self, nuclide: Nuclide, rates: ReactionRates) -> Result<(), DepletionError> { /* ... */ }
  ```
  Replace the reaction rates of one nuclide (for a new burnup step).

- ```rust
  pub fn build_matrix(self: &Self) -> BurnupMatrix { /* ... */ }
  ```
  Assemble the burnup matrix `A` (units `1/s`) from the current data.

- ```rust
  pub fn inventory_vector(self: &Self, densities: &[(Nuclide, f64)]) -> Result<Vec<f64>, DepletionError> { /* ... */ }
  ```
  Build an initial-inventory vector from a `(nuclide, density)` map.

- ```rust
  pub fn deplete(self: &Self, n0: &[f64], dt: f64) -> Result<Vec<f64>, DepletionError> { /* ... */ }
  ```
  Deplete `n0` over a single timestep `dt` (seconds) via order-16 CRAM.

- ```rust
  pub fn deplete_multi(self: &Self, n0: &[f64], steps: &[f64]) -> Result<Vec<f64>, DepletionError> { /* ... */ }
  ```
  Multi-step depletion with a fixed matrix over each `dt` in `steps`.

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
    fn clone(self: &Self) -> DepletionSystem { /* ... */ }
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

- **RefUnwindSafe**
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `matrix`

The burnup / depletion matrix `A` (units `1/s`).

The Bateman depletion equation is `dn/dt = A·n`, where `n` is the vector of
nuclide number densities (atoms, or atoms·cm⁻³) and `A` is the burnup matrix
with units `1/s`. For nuclide `i`:

```text
  A[i][i] = -(λ_i + Σ_c r_{i,c})              (loss: decay + all reactions)
  A[i][j] =  (partial decay j→i) + (reaction rate j→i)   (gain from parent j)
```

This is exactly ONIX's `A = B·1e-24·φ + C` (`onix/salameche/burn.py:187`),
where `B` is the cross-section matrix (`get_xs_mat`) and `C` the decay matrix
(`get_decay_mat`). Here the reaction rates are supplied already in `1/s`.

The matrix is stored **dense, row-major**. Depletion matrices are sparse in
reality, but for the modest nuclide counts of a stand-alone chain (tens to a
few hundred) dense storage plus a dense complex solve in [`crate::cram`] is
simple, allocation-light, and pure Rust (no BLAS).

## Provenance (GPLv3 relicensing of MIT upstream)

Assembly logic ported from ONIX (open-source, MIT; commit `7328dc6`):
`onix/salameche/mat_builder.py` (`get_xs_mat` lines 5–127, `get_decay_mat`
lines 134–193) and `onix/salameche/burn.py:187` (the `B·1e-24·φ + C` sum).
Independent Rust re-implementation; OUTRAM PARK fork relicenses under
**GPL-3.0-only** (MIT is GPL-3.0-compatible).

```rust
pub mod matrix { /* ... */ }
```

### Types

#### Struct `BurnupMatrix`

A dense, row-major depletion matrix `A` with units `1/s`.

Index convention: `A[i][j]` is the rate at which parent `j` feeds nuclide
`i` (off-diagonal, `>= 0`); the diagonal `A[i][i]` is the total loss rate of
nuclide `i` (`<= 0`). Multiply by a timestep Δt (seconds) to get the
dimensionless matrix whose exponential action gives the depleted inventory.

```rust
pub struct BurnupMatrix {
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
  pub fn zeros(n: usize) -> Self { /* ... */ }
  ```
  An `n`×`n` zero matrix.

- ```rust
  pub fn dim(self: &Self) -> usize { /* ... */ }
  ```
  The dimension `n` (number of tracked nuclides).

- ```rust
  pub fn get(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```
  Read entry `A[i][j]` (units `1/s`). Panics if out of bounds.

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Set entry `A[i][j]` (units `1/s`). Panics if out of bounds.

- ```rust
  pub fn add(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Accumulate `v` into `A[i][j]` (units `1/s`). Panics if out of bounds.

- ```rust
  pub fn as_slice(self: &Self) -> &[f64] { /* ... */ }
  ```
  Borrow the raw row-major entries (units `1/s`).

- ```rust
  pub fn mul_vec(self: &Self, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Matrix–vector product `A·x` (units of the result: `1/s` × units of `x`).

- ```rust
  pub fn column_sums(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Column sums of `A` (units `1/s`), one per parent nuclide `j`.

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
    fn clone(self: &Self) -> BurnupMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BurnupMatrix) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
## Module `nuclide`

Nuclide identity — atomic number, mass number, and metastable state.

Physical quantity: a *nuclide* is one nuclear species, identified by its
proton number `Z` (dimensionless count), mass number `A = Z + N`
(dimensionless count of nucleons), and an integer *metastable state* index
`m` (0 = ground state, 1 = first isomer, …).

## Provenance (GPLv3 relicensing of MIT upstream)

Ported from the ONIX depletion code (open-source, MIT licensed):
  * upstream project: ONIX — <https://github.com/jlanversin/ONIX>
  * upstream commit:  `7328dc6`
  * source files:     `onix/utils/functions.py` (`zamid_to_name`,
    `name_to_zamid`, lines 272–325 — the `zzaaam = 10000*Z + 10*A + m`
    packing convention) and `onix/passport.py` (the `zamid`/`state`
    accessors).

This file is an independent Rust re-implementation of that convention. The
OUTRAM PARK fork relicenses the derived work under **GPL-3.0-only** (MIT is
GPL-3.0-compatible; the upstream MIT notice is preserved above).

```rust
pub mod nuclide { /* ... */ }
```

### Types

#### Struct `Nuclide`

A nuclide: proton number, mass number, and metastable-state index.

* `z` — proton number (atomic number). Valid range in practice `1..=118`.
* `a` — mass number (nucleons, protons + neutrons). Must satisfy `a >= z`.
* `m` — metastable-state index: `0` = ground state, `1` = first metastable
  isomer, and so on. Physically small (`0..=2` for essentially all data).

Units: all three fields are **dimensionless integer counts**.

```rust
pub struct Nuclide {
    pub z: u32,
    pub a: u32,
    pub m: u8,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `z` | `u32` | Proton number `Z` (atomic number), dimensionless count. |
| `a` | `u32` | Mass number `A` (nucleon count), dimensionless. Physically `a >= z`. |
| `m` | `u8` | Metastable-state index (0 = ground, 1 = first isomer, …), dimensionless. |

##### Implementations

###### Methods

- ```rust
  pub const fn new(z: u32, a: u32, m: u8) -> Self { /* ... */ }
  ```
  Construct a nuclide from `Z`, `A`, and metastable index `m`.

- ```rust
  pub const fn zamid(self: &Self) -> ZamId { /* ... */ }
  ```
  The ONIX packed id `10000*Z + 10*A + m` (dimensionless).

- ```rust
  pub const fn from_zamid(zamid: ZamId) -> Self { /* ... */ }
  ```
  Reconstruct a nuclide from its ONIX packed id.

- ```rust
  pub const fn neutron_number(self: &Self) -> Option<u32> { /* ... */ }
  ```
  Neutron number `N = A - Z` (dimensionless count).

- ```rust
  pub const fn is_physical(self: &Self) -> bool { /* ... */ }
  ```
  Basic physicality gate: `z >= 1` and `a >= z`.

- ```rust
  pub fn apply_delta(self: &Self, dz: i32, da: i32, dm: i32) -> Option<Nuclide> { /* ... */ }
  ```
  Apply signed `(dZ, dA, dm)` deltas, returning the product nuclide.

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
    fn clone(self: &Self) -> Nuclide { /* ... */ }
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Nuclide) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Nuclide) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Nuclide) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Type Alias `ZamId`

ONIX-style packed nuclide id: `zamid = 10000*Z + 10*A + m`.

This is the exact packing used by ONIX (`name_to_zamid`,
`onix/utils/functions.py:322`) but held as an integer instead of a string.
Units: dimensionless. Example: U-235 ground state → `922350`; Am-242m1 →
`952421`.

```rust
pub type ZamId = u64;
```

## Module `reactions`

Decay modes and neutron-induced reaction channels, with daughter lookup.

Each variant carries the `(dZ, dA, dm)` operation ONIX applies to a parent
nuclide's packed id to obtain the daughter. Enum dispatch is used throughout
(workspace rule: no trait objects) — the set of channels is closed and known
at compile time, so adding a variant forces every `match` to handle it.

## Provenance (GPLv3 relicensing of MIT upstream)

Ported from the ONIX depletion code (open-source, MIT licensed):
  * upstream project: ONIX — <https://github.com/jlanversin/ONIX>
  * upstream commit:  `7328dc6`
  * source file/lines: `onix/data/list_and_dict.py:244` (`xs_prod_fromS_toS`
    — the `(n,gamma)/(n,2n)/(n,3n)/(n,p)/(n,a)/(n,t)` delta triples) and
    `onix/data/list_and_dict.py:250` (`decay_prod_fromS_toS` — the
    `betaneg/betapos/alpha/neutron/proton` delta triples).

Independent Rust re-implementation; the OUTRAM PARK fork relicenses under
**GPL-3.0-only** (MIT is GPL-3.0-compatible; upstream MIT notice preserved).

```rust
pub mod reactions { /* ... */ }
```

### Types

#### Enum `DecayMode`

A radioactive decay mode.

Each mode maps a parent nuclide to a daughter via a fixed `(dZ, dA, dm)`
transformation. The associated *rate* of a mode is the partial decay
constant (units `1/s`, see [`crate::chain::DecayData`]); this enum only
encodes the identity transformation, not the rate.

Delta triples reproduce ONIX `decay_prod_fromS_toS`
(`onix/data/list_and_dict.py:250`).

```rust
pub enum DecayMode {
    BetaMinus,
    BetaPlus,
    Alpha,
    NeutronEmission,
    ProtonEmission,
    IsomericTransition,
}
```

##### Variants

###### `BetaMinus`

β⁻ decay: a neutron → proton, so `Z+1`, `A` unchanged. `[+1, 0, 0]`.

###### `BetaPlus`

β⁺ decay / electron capture: `Z-1`, `A` unchanged. `[-1, 0, 0]`.

###### `Alpha`

α decay: emit a ⁴He nucleus, so `Z-2`, `A-4`. `[-2, -4, 0]`.

###### `NeutronEmission`

Neutron emission: `A-1`, `Z` unchanged. `[0, -1, 0]`.

###### `ProtonEmission`

Proton emission: `Z-1`, `A-1`. `[-1, -1, 0]`.

###### `IsomericTransition`

Isomeric transition: same `Z`, same `A`, de-excite to ground (`m -> 0`).

ONIX handles metastable de-excitation through its state bookkeeping; here
we model the common ground-state landing (`dm` set so the daughter is
`m = 0`) via [`DecayMode::daughter`]'s special case.

##### Implementations

###### Methods

- ```rust
  pub const fn delta(self: &Self) -> (i32, i32, i32) { /* ... */ }
  ```
  The `(dZ, dA, dm)` delta this mode applies to a parent.

- ```rust
  pub fn daughter(self: &Self, parent: Nuclide) -> Option<Nuclide> { /* ... */ }
  ```
  The daughter nuclide produced by this decay mode, or `None` if the

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
    fn clone(self: &Self) -> DecayMode { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DecayMode) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Enum `ReactionChannel`

A neutron-induced reaction channel.

The transmutation channels each map parent → daughter through a fixed
`(dZ, dA, dm)` delta (ONIX `xs_prod_fromS_toS`,
`onix/data/list_and_dict.py:244`). [`ReactionChannel::Fission`] is special:
it has no single daughter — its products come from a fission-yield table
(see [`crate::chain::FissionYields`]).

The associated *rate* of a channel is the one-group (or few-group collapsed)
reaction rate in `1/s` (i.e. microscopic cross section in barns × `1e-24`
cm²/barn × scalar flux in n·cm⁻²·s⁻¹). This enum encodes only identity.

```rust
pub enum ReactionChannel {
    NGamma,
    N2n,
    N3n,
    Np,
    NAlpha,
    NT,
    Fission,
}
```

##### Variants

###### `NGamma`

Radiative capture `(n,γ)`: `A+1`, `Z` unchanged. `[0, +1, 0]`.

###### `N2n`

`(n,2n)`: net loss of one nucleon, `A-1`. `[0, -1, 0]`.

###### `N3n`

`(n,3n)`: net loss of two nucleons, `A-2`. `[0, -2, 0]`.

###### `Np`

`(n,p)`: `Z-1`, `A` unchanged (absorb n, emit p). `[-1, 0, 0]`.

###### `NAlpha`

`(n,α)`: `Z-2`, `A-3` (absorb n, emit ⁴He). `[-2, -3, 0]`.

###### `NT`

`(n,t)`: `Z-1`, `A-2` (absorb n, emit triton). `[-1, -2, 0]`.

###### `Fission`

Neutron-induced fission. No single daughter — products are drawn from a
fission-yield table. [`ReactionChannel::daughter`] returns `None`.

##### Implementations

###### Methods

- ```rust
  pub const fn delta(self: &Self) -> Option<(i32, i32, i32)> { /* ... */ }
  ```
  The `(dZ, dA, dm)` delta for the transmutation channels.

- ```rust
  pub const fn is_fission(self: &Self) -> bool { /* ... */ }
  ```
  Whether this channel is fission (its products come from a yield table).

- ```rust
  pub fn daughter(self: &Self, parent: Nuclide) -> Option<Nuclide> { /* ... */ }
  ```
  The transmutation daughter, or `None` for fission / unphysical results.

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
    fn clone(self: &Self) -> ReactionChannel { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReactionChannel) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
## Re-exports

### Re-export `DecayData`

```rust
pub use chain::DecayData;
```

### Re-export `FissionYields`

```rust
pub use chain::FissionYields;
```

### Re-export `ReactionRates`

```rust
pub use chain::ReactionRates;
```

### Re-export `clamp_nonnegative`

```rust
pub use cram::clamp_nonnegative;
```

### Re-export `cram16`

```rust
pub use cram::cram16;
```

### Re-export `CramError`

```rust
pub use cram::CramError;
```

### Re-export `DepletionError`

```rust
pub use driver::DepletionError;
```

### Re-export `DepletionSystem`

```rust
pub use driver::DepletionSystem;
```

### Re-export `NuclideIndex`

```rust
pub use driver::NuclideIndex;
```

### Re-export `BurnupMatrix`

```rust
pub use matrix::BurnupMatrix;
```

### Re-export `Nuclide`

```rust
pub use nuclide::Nuclide;
```

### Re-export `ZamId`

```rust
pub use nuclide::ZamId;
```

### Re-export `DecayMode`

```rust
pub use reactions::DecayMode;
```

### Re-export `ReactionChannel`

```rust
pub use reactions::ReactionChannel;
```

