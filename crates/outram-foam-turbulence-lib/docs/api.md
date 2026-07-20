# Crate Documentation

**Version:** 0.1.0

**Format Version:** 60

# Module `outram_foam_turbulence_lib`

**This is OUTRAM PARK's independent Rust translation of selected
OpenFOAM® turbulence-model algorithms — it is not the official
OpenFOAM® software and is not affiliated with, endorsed by, or
sanctioned by OpenCFD Ltd. or the ESI Group.** OpenFOAM® is a registered
trademark of OpenCFD Limited. See `TRADEMARKS.md` (this crate's
directory, mirrored from the workspace root) for the full attribution
and non-affiliation notice.

# Overview

Pure-Rust translation of the OpenFOAM turbulence-closure library: RAS
(Reynolds-Averaged Simulation) and LES (Large-Eddy Simulation) models that
supply the turbulent-stress and effective-viscosity terms a momentum solver
needs. Every model implements the [`traits::TurbulenceModel`] trait; dispatch
is static (generics), never `dyn`.

# Implementation status (read before depending on a model)

Only **k-ω SST is implemented and unit-tested**. The other closures are
scaffolds — the struct and its coefficients exist, but their trait methods
`todo!()`-panic if called. Constructing them is safe; driving them is not.

| Module | Model | Status |
|---|---|---|
| [`k_omega_sst`] | Menter (1994) k-ω SST | Implemented + unit-tested |
| [`laminar`] | No-op laminar | Partial — `div_dev_rho_reff` is `todo!()` |
| [`k_epsilon`] | Jones & Launder (1972) k-ε | Scaffold — trait methods `todo!()` |
| [`k_omega`] | Wilcox (1988) k-ω | Scaffold — trait methods `todo!()` |
| [`spalart_allmaras`] | Spalart-Allmaras (1992) | Scaffold — trait methods `todo!()` |
| [`les`] | Smagorinsky (1963) LES | Scaffold — trait methods `todo!()` |

[`wall_functions`] provides standalone log-law helpers (`y_plus`, `u_tau`,
`nu_t_wall`); they are not yet wired into any model as boundary conditions.
See `README.md` ("Limitations") for the full scope/validation caveats.

## Modules

## Module `error`

Error type for turbulence-model construction and transport solves.

A single [`TurbulenceError`] enum covers the failure modes the closures can
report (field-shape mismatches, use-before-init, and non-physical negative
turbulence quantities).

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `TurbulenceError`

Failure modes reported by the turbulence closures.

```rust
pub enum TurbulenceError {
    FieldSizeMismatch(String),
    NotInitialised,
    NegativeField {
        field: &'static str,
        value: f64,
    },
}
```

##### Variants

###### `FieldSizeMismatch`

A field passed in does not match the mesh cell count (or another field's
length). The payload describes the mismatch.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NotInitialised`

A model method was called before the model's state was initialised.

###### `NegativeField`

A turbulence quantity (e.g. k, ε, ω, ν_t) went negative, which is
non-physical. `field` names the quantity; `value` is the offending value.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `&'static str` | Name of the turbulence field that went negative (e.g. `"k"`). |
| `value` | `f64` | The offending (negative) value. |

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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
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
## Module `k_epsilon`

Standard k-ε RAS model (Jones & Launder 1972) — **scaffold only**.

The [`KEpsilon`] struct, its transport fields, and its model constants exist,
but the transport solve is not implemented: `correct`, `div_dev_rho_reff`,
`alpha_eff`, and `mu_eff_field` are `todo!()` stubs that panic if called.
Only `nu_t()` (which returns the zero-initialised field) is callable.

```rust
pub mod k_epsilon { /* ... */ }
```

### Types

#### Struct `KEpsilon`

Standard two-equation k-ε turbulence model (Jones & Launder 1972).

**Scaffold only** — every trait method except `nu_t()` is a `todo!()` that
panics if called. The struct and coefficients document the intended model:

C++ source: `src/TurbulenceModels/turbulenceModels/RAS/kEpsilon/`

Transport equations:
  ∂k/∂t + ∇·(Uk) − ∇·((ν + ν_t/σ_k)∇k) = G − ε
  ∂ε/∂t + ∇·(Uε) − ∇·((ν + ν_t/σ_ε)∇ε) = C1ε·(ε/k)·G − C2ε·(ε²/k)
  ν_t = Cμ · k² / ε

```rust
pub struct KEpsilon {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub k: outram_foam_basic_lib::prelude::VolScalarField,
    pub epsilon: outram_foam_basic_lib::prelude::VolScalarField,
    pub nu_t: outram_foam_basic_lib::prelude::VolScalarField,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` |  |
| `k` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinetic energy [m²/s²] |
| `epsilon` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent dissipation rate [m²/s³] |
| `nu_t` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinematic viscosity ν_t = Cμ k²/ε [m²/s] |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Standard Jones-Launder coefficients.

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
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TurbulenceModel**
  - ```rust
    fn div_dev_rho_reff(self: &Self, _u: &VolVectorField) -> FvVectorMatrix { /* ... */ }
    ```

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```

  - ```rust
    fn nu_t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_eff(self: &Self, _alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu_eff_field(self: &Self, _mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `k_omega`

Standard k-ω RAS model (Wilcox 1988) — **scaffold only**.

The [`KOmega`] struct, its transport fields, and its model constants exist,
but the transport solve is not implemented: `correct`, `div_dev_rho_reff`,
`alpha_eff`, and `mu_eff_field` are `todo!()` stubs that panic if called.
Only `nu_t()` (which returns the zero-initialised field) is callable.

```rust
pub mod k_omega { /* ... */ }
```

### Types

#### Struct `KOmega`

Standard two-equation k-ω turbulence model (Wilcox 1988).

**Scaffold only** — every trait method except `nu_t()` is a `todo!()` that
panics if called. The struct and coefficients document the intended model:

C++ source: `src/TurbulenceModels/turbulenceModels/RAS/kOmega/`

Transport equations:
  ∂k/∂t + ∇·(Uk) − ∇·((ν + σ_k ν_t)∇k) = G − β* k ω
  ∂ω/∂t + ∇·(Uω) − ∇·((ν + σ_ω ν_t)∇ω) = α (ω/k) G − β ω²
  ν_t = k / ω

```rust
pub struct KOmega {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub k: outram_foam_basic_lib::prelude::VolScalarField,
    pub omega: outram_foam_basic_lib::prelude::VolScalarField,
    pub nu_t: outram_foam_basic_lib::prelude::VolScalarField,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` |  |
| `k` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinetic energy [m²/s²] |
| `omega` | `outram_foam_basic_lib::prelude::VolScalarField` | Specific dissipation rate ω [1/s] |
| `nu_t` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinematic viscosity ν_t = k/ω [m²/s] |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Wilcox 1988 coefficients.

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
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TurbulenceModel**
  - ```rust
    fn div_dev_rho_reff(self: &Self, _u: &VolVectorField) -> FvVectorMatrix { /* ... */ }
    ```

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```

  - ```rust
    fn nu_t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_eff(self: &Self, _alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu_eff_field(self: &Self, _mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `k_omega_sst`

Menter (1994) k-ω SST RAS turbulence model.

Mirrors `src/TurbulenceModels/.../RAS/kOmegaSST/`. Blends k-ω (inner
boundary layer, F1 = 1) with transformed k-ε (free stream, F1 = 0) and
applies the Bradshaw stress limiter via F2.

## State the model needs beyond the trait

`TurbulenceModel::correct(&mut self)` takes no arguments, but a real model
needs the live velocity `U`, face flux `phi`, molecular viscosity `ν`, the
wall-distance field, and the time step. As in OpenFOAM (where the model holds
references to those fields), this struct stores them: set `u`, `phi`, `nu`,
`dt` each time step before calling `correct()`. The wall-distance field `y`
is computed once from the `Wall` patches at construction.

## Scope / validation

The constitutive relations (νt stress limiter, F1/F2 blending, production)
and the k/ω transport assembly are implemented and unit-tested. End-to-end
CFD validation (the NACA0012 aerofoil) additionally needs a working
`RhoPimpleFoam` and turbulence wall-function boundary conditions, both
tracked in `outram-foam-appbuilder-lib/TODO.md`.

```rust
pub mod k_omega_sst { /* ... */ }
```

### Types

#### Struct `KOmegaSST`

Menter k-ω SST turbulence model (1994).

```rust
pub struct KOmegaSST {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub k: outram_foam_basic_lib::prelude::VolScalarField,
    pub omega: outram_foam_basic_lib::prelude::VolScalarField,
    pub nu_t: outram_foam_basic_lib::prelude::VolScalarField,
    pub u: outram_foam_basic_lib::prelude::VolVectorField,
    pub phi: outram_foam_basic_lib::prelude::SurfaceScalarField,
    pub nu: outram_foam_basic_lib::prelude::VolScalarField,
    pub dt: f64,
    pub prt: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` |  |
| `k` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinetic energy k [m²/s²]. |
| `omega` | `outram_foam_basic_lib::prelude::VolScalarField` | Specific dissipation rate ω [1/s]. |
| `nu_t` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinematic viscosity ν_t [m²/s], from the Bradshaw-limited<br>ν_t = a1·k / max(a1·ω, |S|·F2). |
| `u` | `outram_foam_basic_lib::prelude::VolVectorField` | Velocity field — set by the solver each step before `correct()`. |
| `phi` | `outram_foam_basic_lib::prelude::SurfaceScalarField` | Face volumetric flux φ = U·Sf — set by the solver each step. |
| `nu` | `outram_foam_basic_lib::prelude::VolScalarField` | Molecular kinematic viscosity ν. |
| `dt` | `f64` | Time step [s]. |
| `prt` | `f64` | Turbulent Prandtl number for `alpha_eff`. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Construct a k-ω SST model over `mesh` with Menter (1994) coefficients.

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
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TurbulenceModel**
  - ```rust
    fn div_dev_rho_reff(self: &Self, u: &VolVectorField) -> FvVectorMatrix { /* ... */ }
    ```

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```

  - ```rust
    fn nu_t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_eff(self: &Self, alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu_eff_field(self: &Self, mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `SIGMA_K1`

k-diffusion coefficient σ_k for the inner (k-ω) set.

```rust
pub const SIGMA_K1: f64 = 0.85;
```

#### Constant `SIGMA_K2`

k-diffusion coefficient σ_k for the outer (transformed k-ε) set.

```rust
pub const SIGMA_K2: f64 = 1.00;
```

#### Constant `SIGMA_W1`

ω-diffusion coefficient σ_ω for the inner (k-ω) set.

```rust
pub const SIGMA_W1: f64 = 0.50;
```

#### Constant `SIGMA_W2`

ω-diffusion coefficient σ_ω for the outer (transformed k-ε) set.

```rust
pub const SIGMA_W2: f64 = 0.856;
```

#### Constant `BETA1`

ω-destruction coefficient β for the inner (k-ω) set.

```rust
pub const BETA1: f64 = 0.075;
```

#### Constant `BETA2`

ω-destruction coefficient β for the outer (transformed k-ε) set.

```rust
pub const BETA2: f64 = 0.0828;
```

#### Constant `BETA_STAR`

k-destruction coefficient β* (Cμ), shared by both sets.

```rust
pub const BETA_STAR: f64 = 0.09;
```

#### Constant `KAPPA`

von Kármán constant κ.

```rust
pub const KAPPA: f64 = 0.41;
```

#### Constant `A1`

Bradshaw stress-limiter coefficient a1 (in ν_t = a1·k / max(a1·ω, |S|·F2)).

```rust
pub const A1: f64 = 0.31;
```

## Module `laminar`

No-op laminar "turbulence" closure — zero turbulent viscosity everywhere.

**Partial.** `correct` (a no-op), `nu_t`, `alpha_eff`, and `mu_eff_field`
work, but [`LaminarModel::div_dev_rho_reff`] — the momentum stress term — is
still a `todo!()` and will panic if called, so the laminar closure cannot yet
be driven end-to-end through the trait.

```rust
pub mod laminar { /* ... */ }
```

### Types

#### Struct `LaminarModel`

No-op turbulence model — laminar flow, zero turbulent stresses.

Physically ν_t ≡ 0 (no turbulent viscosity), so the effective viscosity and
thermal diffusivity equal their molecular values.

**Partial implementation:** `div_dev_rho_reff` is a `todo!()` stub that
panics if called (see the module docs).

C++ source: `src/TurbulenceModels/turbulenceModels/laminar/laminar.H`

```rust
pub struct LaminarModel {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` |  |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, nu: VolScalarField) -> Self { /* ... */ }
  ```
  Construct a laminar closure over `mesh` with molecular kinematic

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
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TurbulenceModel**
  - ```rust
    fn div_dev_rho_reff(self: &Self, _u: &VolVectorField) -> FvVectorMatrix { /* ... */ }
    ```
    Not implemented — panics (`todo!()`). The laminar momentum stress term

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```
    No-op — laminar model has no transport equations to solve.

  - ```rust
    fn nu_t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_eff(self: &Self, alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu_eff_field(self: &Self, mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `les`

Large-Eddy Simulation (LES) sub-grid-scale closures.

Currently holds only the [`Smagorinsky`] model, which is a **scaffold** (its
trait methods `todo!()`-panic — see [`smagorinsky`]).

```rust
pub mod les { /* ... */ }
```

### Modules

## Module `smagorinsky`

Smagorinsky (1963) LES sub-grid-scale model — **scaffold only**.

The [`Smagorinsky`] struct and its constant exist, but the sub-grid
viscosity update is not implemented: `correct`, `div_dev_rho_reff`,
`alpha_eff`, and `mu_eff_field` are `todo!()` stubs that panic if called.
Only `nu_t()` (which returns the zero-initialised ν_sgs field) is callable.

```rust
pub mod smagorinsky { /* ... */ }
```

### Types

#### Struct `Smagorinsky`

Smagorinsky LES sub-grid scale model (1963).

**Scaffold only** — every trait method except `nu_t()` is a `todo!()` that
panics if called. The struct documents the intended model:

C++ source: `src/TurbulenceModels/LES/Smagorinsky/`

Sub-grid viscosity:  ν_sgs = (Cs·Δ)² · |S|
  where Cs ≈ 0.17 is the Smagorinsky constant,
        Δ  = (cell_volume)^(1/3) is the filter width (grid scale),
        |S| = sqrt(2 · symm(∇U) : symm(∇U)) is the strain-rate magnitude.

```rust
pub struct Smagorinsky {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub nu_sgs: outram_foam_basic_lib::prelude::VolScalarField,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` |  |
| `nu_sgs` | `outram_foam_basic_lib::prelude::VolScalarField` | Sub-grid-scale kinematic viscosity ν_sgs [m²/s]. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Construct a Smagorinsky model over `mesh` with the default constant

- ```rust
  pub fn with_cs(self: Self, cs: f64) -> Self { /* ... */ }
  ```
  Builder override for the Smagorinsky constant Cs (dimensionless).

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
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TurbulenceModel**
  - ```rust
    fn div_dev_rho_reff(self: &Self, _u: &VolVectorField) -> FvVectorMatrix { /* ... */ }
    ```

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```

  - ```rust
    fn nu_t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_eff(self: &Self, _alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu_eff_field(self: &Self, _mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `Smagorinsky`

```rust
pub use smagorinsky::Smagorinsky;
```

## Module `prelude`

Convenience re-exports: `use outram_foam_turbulence_lib::prelude::*;` brings
the trait, the error type, every model struct, and the wall-function helpers
into scope. Note that only [`KOmegaSST`] is a working model — the other
structs are scaffolds whose trait methods `todo!()`-panic (see the
crate-level status table).

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `TurbulenceError`

```rust
pub use crate::error::TurbulenceError;
```

#### Re-export `TurbulenceModel`

```rust
pub use crate::traits::TurbulenceModel;
```

#### Re-export `KEpsilon`

```rust
pub use crate::k_epsilon::KEpsilon;
```

#### Re-export `KOmega`

```rust
pub use crate::k_omega::KOmega;
```

#### Re-export `KOmegaSST`

```rust
pub use crate::k_omega_sst::KOmegaSST;
```

#### Re-export `LaminarModel`

```rust
pub use crate::laminar::LaminarModel;
```

#### Re-export `Smagorinsky`

```rust
pub use crate::les::Smagorinsky;
```

#### Re-export `SpalartAllmaras`

```rust
pub use crate::spalart_allmaras::SpalartAllmaras;
```

#### Re-export `nu_t_wall`

```rust
pub use crate::wall_functions::nu_t_wall;
```

#### Re-export `y_plus`

```rust
pub use crate::wall_functions::y_plus;
```

## Module `spalart_allmaras`

Spalart-Allmaras one-equation RAS model (1992) — **scaffold only**.

The [`SpalartAllmaras`] struct and its model constants exist, but the ν̃
transport solve is not implemented: `correct`, `div_dev_rho_reff`,
`alpha_eff`, and `mu_eff_field` are `todo!()` stubs that panic if called.
Only `nu_t()` (which returns the zero-initialised field) is callable.

```rust
pub mod spalart_allmaras { /* ... */ }
```

### Types

#### Struct `SpalartAllmaras`

Spalart-Allmaras one-equation turbulence model (1992).
Common in aerospace applications (external aerodynamics, aerofoils).

**Scaffold only** — every trait method except `nu_t()` is a `todo!()` that
panics if called. The struct and coefficients document the intended model:

C++ source: `src/TurbulenceModels/turbulenceModels/RAS/SpalartAllmaras/`

Single transport equation for the modified viscosity ν̃:
  ∂ν̃/∂t + U·∇ν̃ = Cb1·S̃·ν̃ + (1/σ)∇·((ν+ν̃)∇ν̃) + Cb2/σ·|∇ν̃|² − Cw1·fw·(ν̃/d)²
  ν_t = ν̃ · fv1    where fv1 = χ³/(χ³ + Cv1³),  χ = ν̃/ν

```rust
pub struct SpalartAllmaras {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub nu_tilde: outram_foam_basic_lib::prelude::VolScalarField,
    pub nu_t: outram_foam_basic_lib::prelude::VolScalarField,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` |  |
| `nu_tilde` | `outram_foam_basic_lib::prelude::VolScalarField` | Working variable ν̃ [m²/s] — NOT equal to ν_t directly. |
| `nu_t` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinematic viscosity ν_t = ν̃ · fv1 [m²/s]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```

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
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TurbulenceModel**
  - ```rust
    fn div_dev_rho_reff(self: &Self, _u: &VolVectorField) -> FvVectorMatrix { /* ... */ }
    ```

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```

  - ```rust
    fn nu_t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_eff(self: &Self, _alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu_eff_field(self: &Self, _mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `CB1`

Production coefficient Cb1.

```rust
pub const CB1: f64 = 0.1355;
```

#### Constant `CB2`

Diffusion coefficient Cb2.

```rust
pub const CB2: f64 = 0.622;
```

#### Constant `CV1`

Viscous-function coefficient Cv1 (in fv1 = χ³/(χ³ + Cv1³)).

```rust
pub const CV1: f64 = 7.1;
```

#### Constant `SIGMA`

Turbulent Prandtl-like diffusion constant σ.

```rust
pub const SIGMA: f64 = _;
```

#### Constant `KAPPA`

von Kármán constant κ.

```rust
pub const KAPPA: f64 = 0.41;
```

#### Constant `CW1`

Wall-destruction coefficient Cw1 = Cb1/κ² + (1 + Cb2)/σ (≈ 3.239).

```rust
pub const CW1: f64 = _;
```

#### Constant `CW2`

Wall-destruction coefficient Cw2.

```rust
pub const CW2: f64 = 0.3;
```

#### Constant `CW3`

Wall-destruction coefficient Cw3.

```rust
pub const CW3: f64 = 2.0;
```

## Module `traits`

The [`TurbulenceModel`] trait — the common contract every RAS/LES closure
in this crate implements.

The trait is a compile-time contract, not a dispatch mechanism: solvers hold
a concrete model type (or an enum over the models) and call it through
generics, so there is no `dyn` overhead. Only k-ω SST implements every method
for real today; the other models satisfy the trait but `todo!()`-panic in the
unimplemented methods (see the crate-level status table).

```rust
pub mod traits { /* ... */ }
```

### Traits

#### Trait `TurbulenceModel`

Common interface for all RAS and LES turbulence models.

Mirrors `Foam::compressible::turbulenceModel` and its incompressible
counterpart. Use static dispatch (generics) — not `dyn TurbulenceModel` —
to match C++ template zero-overhead composition.

```rust
pub trait TurbulenceModel {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `div_dev_rho_reff`: Assemble the turbulent deviatoric stress divergence term for the
- `correct`: Recompute turbulence transport fields (k, ε/ω, ν_t/μ_t) by solving
- `nu_t`: Turbulent kinematic viscosity field ν_t (incompressible) or μ_t/ρ
- `alpha_eff`: Effective thermal diffusivity field: α_eff = α + α_t.
- `mu_eff_field`: Effective dynamic viscosity field: μ_eff = μ + μ_t.

##### Implementations

This trait is implemented for the following types:

- `KEpsilon`
- `KOmega`
- `KOmegaSST`
- `LaminarModel`
- `Smagorinsky`
- `SpalartAllmaras`

## Module `wall_functions`

Wall-function utilities for RAS turbulence models.

C++ source: `src/TurbulenceModels/turbulenceModels/RAS/derivedFvPatchFields/`

Used when the mesh is too coarse to resolve the viscous sublayer (y⁺ > ~11).
These are **standalone helper functions**, not yet wired into any model as
patch boundary conditions, and are untested. The log-law constants
(κ = 0.41, E = 9.8, y⁺_lam = 11) are hard-coded.

```rust
pub mod wall_functions { /* ... */ }
```

### Functions

#### Function `y_plus`

Dimensionless wall distance y⁺ = u_τ · y / ν.

# Arguments
* `y`     — wall-normal distance from wall to cell centre [m]
* `u_tau` — friction velocity [m/s]
* `nu`    — kinematic viscosity [m²/s]

```rust
pub fn y_plus(y: f64, u_tau: f64, nu: f64) -> f64 { /* ... */ }
```

#### Function `u_tau`

Friction velocity u_τ from log-law iteration.

Solves  U⁺ = (1/κ) ln(E·y⁺)  for u_τ = U_wall / U⁺
using a Newton iteration.

# Arguments
* `u_wall` — tangential velocity at wall cell centre [m/s]
* `y`      — wall-normal distance [m]
* `nu`     — kinematic viscosity [m²/s]

```rust
pub fn u_tau(u_wall: f64, y: f64, nu: f64) -> f64 { /* ... */ }
```

#### Function `nu_t_wall`

Turbulent kinematic viscosity at the wall cell (nutWallFunction).

Returns ν_t such that the log-law is satisfied:
  ν_t = ν · (κ · y⁺ / ln(E · y⁺) − 1)  for y⁺ > y⁺_lam
  ν_t = 0                                  for y⁺ ≤ y⁺_lam (viscous sublayer)

```rust
pub fn nu_t_wall(y_p: f64, nu: f64) -> f64 { /* ... */ }
```

