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

## Modules

## Module `error`

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `TurbulenceError`

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

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NotInitialised`

###### `NegativeField`

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `&'static str` |  |
| `value` | `f64` |  |

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
## Module `traits`

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

- `LaminarModel`
- `KEpsilon`
- `KOmega`
- `KOmegaSST`
- `SpalartAllmaras`
- `Smagorinsky`

## Module `laminar`

```rust
pub mod laminar { /* ... */ }
```

### Types

#### Struct `LaminarModel`

No-op turbulence model — laminar flow, zero turbulent stresses.

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
## Module `k_epsilon`

```rust
pub mod k_epsilon { /* ... */ }
```

### Types

#### Struct `KEpsilon`

Standard two-equation k-ε turbulence model (Jones & Launder 1972).

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
    fn alpha_eff(self: &Self, alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu_eff_field(self: &Self, mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `k_omega`

```rust
pub mod k_omega { /* ... */ }
```

### Types

#### Struct `KOmega`

Standard two-equation k-ω turbulence model (Wilcox 1988).

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
| `k` | `outram_foam_basic_lib::prelude::VolScalarField` |  |
| `omega` | `outram_foam_basic_lib::prelude::VolScalarField` |  |
| `nu_t` | `outram_foam_basic_lib::prelude::VolScalarField` |  |
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

```rust
pub const SIGMA_K1: f64 = 0.85;
```

#### Constant `SIGMA_K2`

```rust
pub const SIGMA_K2: f64 = 1.00;
```

#### Constant `SIGMA_W1`

```rust
pub const SIGMA_W1: f64 = 0.50;
```

#### Constant `SIGMA_W2`

```rust
pub const SIGMA_W2: f64 = 0.856;
```

#### Constant `BETA1`

```rust
pub const BETA1: f64 = 0.075;
```

#### Constant `BETA2`

```rust
pub const BETA2: f64 = 0.0828;
```

#### Constant `BETA_STAR`

```rust
pub const BETA_STAR: f64 = 0.09;
```

#### Constant `KAPPA`

```rust
pub const KAPPA: f64 = 0.41;
```

#### Constant `A1`

```rust
pub const A1: f64 = 0.31;
```

## Module `spalart_allmaras`

```rust
pub mod spalart_allmaras { /* ... */ }
```

### Types

#### Struct `SpalartAllmaras`

Spalart-Allmaras one-equation turbulence model (1992).
Common in aerospace applications (external aerodynamics, aerofoils).

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

```rust
pub const CB1: f64 = 0.1355;
```

#### Constant `CB2`

```rust
pub const CB2: f64 = 0.622;
```

#### Constant `CV1`

```rust
pub const CV1: f64 = 7.1;
```

#### Constant `SIGMA`

```rust
pub const SIGMA: f64 = _;
```

#### Constant `KAPPA`

```rust
pub const KAPPA: f64 = 0.41;
```

#### Constant `CW1`

```rust
pub const CW1: f64 = _;
```

#### Constant `CW2`

```rust
pub const CW2: f64 = 0.3;
```

#### Constant `CW3`

```rust
pub const CW3: f64 = 2.0;
```

## Module `les`

```rust
pub mod les { /* ... */ }
```

### Modules

## Module `smagorinsky`

```rust
pub mod smagorinsky { /* ... */ }
```

### Types

#### Struct `Smagorinsky`

Smagorinsky LES sub-grid scale model (1963).

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

- ```rust
  pub fn with_cs(self: Self, cs: f64) -> Self { /* ... */ }
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
### Re-exports

#### Re-export `Smagorinsky`

```rust
pub use smagorinsky::Smagorinsky;
```

## Module `wall_functions`

```rust
pub mod wall_functions { /* ... */ }
```

### Functions

#### Function `y_plus`

Wall function utilities for RAS turbulence models.

C++ source: `src/TurbulenceModels/turbulenceModels/RAS/derivedFvPatchFields/`

Used when the mesh is too coarse to resolve the viscous sublayer (y⁺ > ~11).
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

## Module `prelude`

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

#### Re-export `LaminarModel`

```rust
pub use crate::laminar::LaminarModel;
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

#### Re-export `SpalartAllmaras`

```rust
pub use crate::spalart_allmaras::SpalartAllmaras;
```

#### Re-export `Smagorinsky`

```rust
pub use crate::les::Smagorinsky;
```

#### Re-export `y_plus`

```rust
pub use crate::wall_functions::y_plus;
```

#### Re-export `nu_t_wall`

```rust
pub use crate::wall_functions::nu_t_wall;
```

