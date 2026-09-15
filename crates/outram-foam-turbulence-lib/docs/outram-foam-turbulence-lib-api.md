# Crate Documentation

**Version:** 0.1.2

**Format Version:** 61

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

Every closure now implements the full [`traits::TurbulenceModel`] contract
and is unit-tested; none are `todo!()` scaffolds. **Unit-tested is not
benchmark-validated** — the transport formulae, coefficients, and positivity
are checked against the upstream OpenFOAM source and analytical values, but
no model has yet been validated end-to-end against a published turbulence
benchmark, and all use zero-gradient (not wall-function) near-wall boundary
conditions. See each module's "Honest scope" section and `README.md`.

| Module | Model | Status |
|---|---|---|
| [`k_omega_sst`] | Menter (1994) k-ω SST | Implemented + unit-tested |
| [`laminar`] | No-op laminar (ν_t ≡ 0) | Implemented + unit-tested |
| [`k_epsilon`] | Jones & Launder (1972) k-ε | Implemented + unit-tested |
| [`k_omega`] | Wilcox k-ω | Implemented + unit-tested |
| [`spalart_allmaras`] | Spalart-Allmaras (1992) | Implemented + unit-tested |
| [`les`] | Smagorinsky (1963) LES | Implemented + unit-tested |

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

Standard k-ε RAS turbulence model (Launder & Spalding 1972/1974).

Pure-Rust port of OpenFOAM's `RASModels::kEpsilon`. The constitutive
relation (ν_t = C_μ·k²/ε) and the coupled k/ε transport solve are
implemented and unit-tested — this is no longer a scaffold.

## What is ported, and from where

Translated line-by-line from
`upstream_source/OpenFOAM/src/MomentumTransportModels/momentumTransportModels/RAS/kEpsilon/kEpsilon.C`
(OpenFOAM-dev, OpenFOAM Foundation):

- `correctNut()` / `boundEpsilon()` — `kEpsilon.C:41-55`:
  ν_t = C_μ·k²/ε, with ε floored at `C_μ·k²/(nutMaxCoeff·ν)` so ν_t never
  exceeds `nutMaxCoeff·ν` (keeps the ν_t division finite).
- Production `G = ν_t·(dev(twoSymm(∇U)) && ∇U)` — `kEpsilon.C:202-207`.
  For (near-)incompressible flow this reduces to `G = ν_t·|S|²` with
  `|S|² = 2·S:S`, `S = symm(∇U)` — identical to the k-ω SST production term.
- Dissipation (ε) equation — `kEpsilon.C:214-232`:
  ∂ε/∂t + ∇·(φε) − ∇·(D_ε ∇ε) = C1·(ε/k)·G − C2·ε²/k, `D_ε = ν + ν_t/σ_ε`.
- Kinetic-energy (k) equation — `kEpsilon.C:235-252`:
  ∂k/∂t + ∇·(φk) − ∇·(D_k ∇k) = G − ε, `D_k = ν + ν_t/σ_k`.
- Solve order: ε **first**, then k (`kEpsilon.C:230` before `kEpsilon.C:250`).
- `DkEff`/`DepsilonEff` — `kEpsilon.H:161-178`.
- Default coefficients C_μ=0.09, C1=1.44, C2=1.92, σ_k=1.0, σ_ε=1.3 —
  `kEpsilon.C:113-118`.

## State the model needs beyond the trait

`TurbulenceModel::correct(&mut self)` takes no arguments, but the transport
solve needs the live velocity `U`, the face flux `phi`, the molecular
viscosity `ν` and the time step `dt`. As in OpenFOAM (where the model holds
references to those fields) this struct stores them: set `u`, `phi`, `nu`,
`dt` each time step before calling `correct()`.

## Honest scope — what is NOT modelled

- **Wall functions.** OpenFOAM's standard k-ε is a high-Reynolds model that
  relies on `epsilonWallFunction`/`kqRWallFunction`/`nutkWallFunction`
  boundary conditions to set ε, k and ν_t in the first near-wall cell. This
  port uses zero-gradient boundaries for all turbulence fields (exactly as
  the k-ω SST port does) — there is no low-Re damping and no wall-function
  BC, so near-wall profiles are not validated. Suitable for verifying the
  interior transport algebra and coefficients, not for a wall-bounded V&V
  benchmark.
- **Compressibility / RDT.** The `divU` (rapid-distortion, C3) terms and the
  `alpha·rho` phase-fraction/density weighting are dropped: this is the
  single-phase incompressible reduction (α = ρ = 1, ∇·U ≈ 0). The C3 term is
  zero at the default coefficient anyway (`kEpsilon.C:116`).
- **Under-relaxation & `fvModels`/`fvConstraints`.** The `relax()` calls and
  external source/constraint hooks (`kEpsilon.C:227-229,248-249`) are not
  reproduced; the equations are solved as assembled.

```rust
pub mod k_epsilon { /* ... */ }
```

### Types

#### Struct `KEpsilon`

Standard two-equation k-ε turbulence model (Launder & Spalding 1972/1974).

Computes the turbulent (eddy) kinematic viscosity ν_t [m²/s] from a k
(turbulent kinetic energy, [m²/s²]) and ε (dissipation rate, [m²/s³])
transport pair, via ν_t = C_μ·k²/ε. Valid for fully-turbulent,
(near-)incompressible interior flow; see the module-level "Honest scope"
note for the wall-function and compressibility limitations.

Set [`Self::u`], [`Self::phi`], [`Self::nu`] and [`Self::dt`] each time step
before calling [`TurbulenceModel::correct`].

```rust
pub struct KEpsilon {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub k: outram_foam_basic_lib::prelude::VolScalarField,
    pub epsilon: outram_foam_basic_lib::prelude::VolScalarField,
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
| `epsilon` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent dissipation rate ε [m²/s³]. |
| `nu_t` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinematic viscosity ν_t = C_μ·k²/ε [m²/s]. |
| `u` | `outram_foam_basic_lib::prelude::VolVectorField` | Velocity field U [m/s] — set by the solver each step before `correct()`. |
| `phi` | `outram_foam_basic_lib::prelude::SurfaceScalarField` | Face volumetric flux φ = U·Sf [m³/s] — set by the solver each step. |
| `nu` | `outram_foam_basic_lib::prelude::VolScalarField` | Molecular kinematic viscosity ν [m²/s]. |
| `dt` | `f64` | Time step Δt [s]. |
| `prt` | `f64` | Turbulent Prandtl number Pr_t for `alpha_eff` (dimensionless). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Construct a standard k-ε model over `mesh` with Launder-Spalding

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `k_omega`

Standard high-Reynolds-number k-ω RAS turbulence model (Wilcox).

Pure-Rust port of OpenFOAM's `Foam::RASModels::kOmega`, mirroring
`upstream_source/OpenFOAM/src/MomentumTransportModels/momentumTransportModels/RAS/kOmega/kOmega.C`.
The two transport equations, the eddy-viscosity closure, and the momentum
stress divergence are implemented and unit-tested (see the `tests` module at
the bottom of this file). This is **not** a scaffold — every trait method is
a real solve.

## Model equations (incompressible form)

Eddy viscosity (`kOmega.C:50`, `correctNut`):
  ν_t = k / ω   [m²/s]

Production (`kOmega.C:199-203`): G = ν_t · (dev(twoSymm(∇U)) && ∇U). For the
incompressible, near-divergence-free case this reduces to G = ν_t·|S|² with
|S|² = 2·S:S, S = symm(∇U) — the same production form the k-ω SST port uses.

ω transport (`kOmega.C:210-221`):
  ∂ω/∂t + ∇·(φω) − ∇·(D_ω ∇ω) = γ·(ω/k)·G − β·ω²
k transport (`kOmega.C:232-243`):
  ∂k/∂t + ∇·(φk) − ∇·(D_k ∇k) = G − β*·k·ω
with effective diffusivities (`kOmega.H:148-165`):
  D_ω = ν + σ_ω·ν_t,   D_k = ν + σ_k·ν_t.

## Coefficients (Wilcox; `kOmega.H:38-48`, `kOmega.C:111-115`)

β* = 0.09, γ = 0.52, β = 0.072, σ_k (alphaK) = 0.5, σ_ω (alphaOmega) = 0.5.
All dimensionless.

## State the model needs beyond the trait

`TurbulenceModel::correct(&mut self)` takes no arguments, so — exactly as in
OpenFOAM, where the model holds references to the live fields — this struct
stores the velocity `u`, face flux `phi`, molecular viscosity `nu`, and time
step `dt`. Set them each time step before calling `correct()`.

## Honest scope — what is NOT modelled

- **Boundary conditions are zero-gradient only.** There is no ω wall
  function and no near-wall `boundaryFieldRef().updateCoeffs()` (`kOmega.C:207`)
  — this is the high-Re model core without the wall treatment OpenFOAM adds
  through its BC objects. Results near a solid wall are therefore not
  wall-function-accurate.
- **Compressibility / VOF terms dropped.** The `divU` `SuSp` corrections
  (`kOmega.C:217, 239`) and the α·ρ (phase-fraction / density) weighting are
  omitted; this is the constant-density incompressible reduction (α = ρ = 1).
- **`boundOmega`** (`kOmega.C:41-44`) is replaced by a simple clamp of k and
  ω to a `SMALL` positive floor after each solve, not the
  `k/(nutMaxCoeff·ν)` lower bound.
- **No relaxation, `fvModels`, or `fvConstraints`** source/constraint hooks.

Consequently this model is verified (units, formulae, positivity — see the
tests) but not yet validated against a published k-ω benchmark.

```rust
pub mod k_omega { /* ... */ }
```

### Types

#### Struct `KOmega`

Standard two-equation k-ω turbulence model (Wilcox, high-Re form).

Solves transport equations for the turbulent kinetic energy `k` [m²/s²] and
the specific dissipation rate `omega` [1/s], closing the eddy viscosity as
ν_t = k/ω [m²/s]. Port of `Foam::RASModels::kOmega` (`kOmega.C`).

```rust
pub struct KOmega {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub k: outram_foam_basic_lib::prelude::VolScalarField,
    pub omega: outram_foam_basic_lib::prelude::VolScalarField,
    pub nu_t: outram_foam_basic_lib::prelude::VolScalarField,
    pub u: outram_foam_basic_lib::prelude::VolVectorField,
    pub phi: outram_foam_basic_lib::prelude::SurfaceScalarField,
    pub nu: outram_foam_basic_lib::prelude::VolScalarField,
    pub dt: f64,
    pub prt: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` |  |
| `k` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinetic energy k [m²/s²]. |
| `omega` | `outram_foam_basic_lib::prelude::VolScalarField` | Specific dissipation rate ω [1/s]. |
| `nu_t` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinematic viscosity ν_t = k/ω [m²/s]. |
| `u` | `outram_foam_basic_lib::prelude::VolVectorField` | Velocity field U [m/s] — set by the solver each step before `correct()`. |
| `phi` | `outram_foam_basic_lib::prelude::SurfaceScalarField` | Face volumetric flux φ = U·Sf [m³/s] — set by the solver each step. |
| `nu` | `outram_foam_basic_lib::prelude::VolScalarField` | Molecular kinematic viscosity ν [m²/s]. |
| `dt` | `f64` | Time step [s]. |
| `prt` | `f64` | Turbulent Prandtl number Pr_t [-] for `alpha_eff` (default 0.85). |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Construct a standard k-ω model over `mesh` with Wilcox coefficients.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

#### Constant `BETA_STAR`

k-destruction coefficient β* (= C_μ) in the k-destruction term β*·k·ω.

```rust
pub const BETA_STAR: f64 = 0.09;
```

#### Constant `GAMMA`

ω-production coefficient γ (a.k.a. α / C_ω1) in γ·(ω/k)·G.

```rust
pub const GAMMA: f64 = 0.52;
```

#### Constant `BETA`

ω-destruction coefficient β in the ω-destruction term β·ω².

```rust
pub const BETA: f64 = 0.072;
```

#### Constant `ALPHA_K`

k-diffusion coefficient σ_k (OpenFOAM `alphaK`) in D_k = ν + σ_k·ν_t.

```rust
pub const ALPHA_K: f64 = 0.5;
```

#### Constant `ALPHA_OMEGA`

ω-diffusion coefficient σ_ω (OpenFOAM `alphaOmega`) in D_ω = ν + σ_ω·ν_t.

```rust
pub const ALPHA_OMEGA: f64 = 0.5;
```

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

Fully implemented: `correct` (a no-op), `nu_t` (a genuine zero field),
`alpha_eff`, `mu_eff_field`, and the momentum stress term
[`LaminarModel::div_dev_rho_reff`]. Because ν_t ≡ 0, the effective viscosity
reduces to the molecular value ν, and the stress divergence is exactly the
molecular viscous term

```text
  divDevReff(U) = −∇·(ν ∇U)  −  ∇·(ν · dev2(∇Uᵀ))
```

the implicit Laplacian plus the explicit transpose correction. This is the
`divDevTau`/`divDevReff` term shared by every OpenFOAM momentum-transport
model with `νt = 0`.

C++ source (the shared linear-viscous-stress term):
`src/MomentumTransportModels/momentumTransportModels/linearViscousStress/linearViscousStress.C:89-114`
(`DivDevTau` = `divDevTauCorr − fvm::laplacian(alphaRhoNuEff, U)`, with the
correction `−∇·(alphaRhoNuEff · dev2(T(grad U)))`); the laminar model itself
is `.../momentumTransportModels/laminar/laminar/laminar.H`.

```rust
pub mod laminar { /* ... */ }
```

### Types

#### Struct `LaminarModel`

No-op turbulence model — laminar flow, zero turbulent stresses.

Physically ν_t ≡ 0 (no turbulent viscosity), so the effective viscosity and
thermal diffusivity equal their molecular values. The momentum stress term
[`LaminarModel::div_dev_rho_reff`] is therefore the pure molecular viscous
divergence `−∇·(ν ∇U) − ∇·(ν·dev2(∇Uᵀ))`.

C++ source: `src/MomentumTransportModels/momentumTransportModels/laminar/laminar/laminar.H`
(constitutive stress term: `linearViscousStress.C:89-114`).

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
    fn div_dev_rho_reff(self: &Self, u: &VolVectorField) -> FvVectorMatrix { /* ... */ }
    ```
    Assemble the laminar (molecular) deviatoric stress divergence

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```
    No-op — laminar model has no transport equations to solve.

  - ```rust
    fn nu_t(self: &Self) -> &VolScalarField { /* ... */ }
    ```
    Turbulent kinematic viscosity ν_t [m²/s] — a genuine zero field for

  - ```rust
    fn alpha_eff(self: &Self, alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```
    Effective thermal diffusivity α_eff = α + α_t. For laminar flow α_t = 0,

  - ```rust
    fn mu_eff_field(self: &Self, mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```
    Effective dynamic viscosity μ_eff = μ + μ_t. For laminar flow μ_t = 0,

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

Smagorinsky (1963) LES sub-grid-scale (SGS) eddy-viscosity model.

Pure-Rust port of OpenFOAM's `LESModels::Smagorinsky`
(`src/MomentumTransportModels/momentumTransportModels/LES/Smagorinsky/`,
`Smagorinsky.C:39-67`, `Smagorinsky.H:38-59`) on top of
`LESModels::LESeddyViscosity` (`LESeddyViscosity.C:60-61` for the default
coefficients).

## What this computes

Unlike the textbook algebraic form `ν_sgs = (Cs·Δ)²·|S|`, OpenFOAM's
Smagorinsky is derived from the one-equation `kEqn` SGS model **assuming
local equilibrium** (production = dissipation). That gives a closed
algebraic estimate of the SGS kinetic energy `k` (m²/s²) from the resolved
strain, and then the eddy viscosity from `k` (`Smagorinsky.H:38-50`):

```text
  D  = symm(grad U)                              (resolved strain-rate tensor, 1/s)
  a  = Ce / Δ                                    (1/(m·s) after ×k^(3/2))
  b  = (2/3)·tr(D)                               (1/s; = 0 for divergence-free flow)
  c  = 2·Ck·Δ·(dev(D) : D)                       (m·(1/s²) = m/s²)
  √k = (-b + √(b² + 4·a·c)) / (2·a)              (m/s)  → k = (√k)²   (m²/s²)
  ν_sgs = Ck · Δ · √k                            (m²/s)
```

This is the exact quadratic root solved in `Smagorinsky.C:45-55` for the
`k(gradU)` protected member, followed by `correctNut()`
(`Smagorinsky.C:59-67`): `nut_ = Ck·delta·sqrt(k)`.

## Filter width Δ (the `cubeRootVol` delta)

The LES filter width is the local grid scale. This port uses the
`cubeRootVol` choice — `Δ = V^(1/3)` per cell, `V` the cell volume (m³), so
`Δ` has units of metres. OpenFOAM offers other `delta` models
(`smooth`, `maxDeltaxyz`, `vanDriest`, …) selected at runtime; see the
"Honest scope" note.

## Default coefficients (`LESeddyViscosity.C:60-61`)

* `Ck = 0.094` — SGS eddy-viscosity coefficient (dimensionless).
* `Ce = 1.048` — SGS dissipation coefficient (dimensionless).

These correspond to an effective Smagorinsky constant
`Cs = (Ck·√(Ck/Ce))^(1/2) ≈ 0.17` for isotropic turbulence, which is why
the model is still called "Smagorinsky".

## State the model needs beyond the trait

[`TurbulenceModel::correct`] takes no arguments, but the algebraic update
needs the live velocity field `U` and the molecular viscosity `ν`. As in
OpenFOAM (where the model holds references to those fields), this struct
stores them: set `u` and `nu` each time step before calling `correct()`.
Δ is a function of the (fixed) mesh and is precomputed once at construction.

Because the model is **algebraic**, `correct()` solves no transport PDE —
it simply recomputes `k_sgs` and `ν_sgs` from the current strain field. It
is therefore markedly simpler than the RAS transport models (k-ω SST etc.).

## Honest scope — what is NOT modelled

* **Only the `cubeRootVol` delta** (`Δ = V^(1/3)`) is implemented. No
  `maxDeltaxyz`, no `smooth`, and in particular **no van Driest near-wall
  damping** — so the raw model over-predicts `ν_sgs` in the viscous
  sublayer, exactly as unmodified OpenFOAM Smagorinsky does. Wall damping
  would require a wall-distance field and the `vanDriest` delta wrapper.
* The compressible / two-phase `alpha`, `rho`, `alphaRhoPhi` weighting from
  the templated base class is not carried; this is the incompressible
  (`ν`-based) specialisation, matching the rest of this crate.
* `read()` (runtime dictionary re-read of `Ck`/`Ce`) has no analogue —
  coefficients are named struct fields set in Rust.
* No end-to-end LES validation (decaying isotropic turbulence, channel
  flow) is performed here; the unit tests below verify the algebra and the
  Δ definition only. See the `## Bookkeeping status` gate in the crate
  README — the V&V axis is not human-cleared.

```rust
pub mod smagorinsky { /* ... */ }
```

### Types

#### Struct `Smagorinsky`

Smagorinsky (1963) LES sub-grid-scale eddy-viscosity model.

Algebraic model: `correct()` recomputes the SGS kinetic energy `k_sgs`
(m²/s²) and kinematic eddy viscosity `ν_sgs` (m²/s) from the resolved
strain `D = symm(∇U)` (1/s) and the per-cell filter width `Δ = V^(1/3)`
(m). No transport equation is solved.

Set [`Smagorinsky::u`] (velocity, m/s) and [`Smagorinsky::nu`] (molecular
kinematic viscosity, m²/s) before each `correct()`.

```rust
pub struct Smagorinsky {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub nu_sgs: outram_foam_basic_lib::prelude::VolScalarField,
    pub k_sgs: outram_foam_basic_lib::prelude::VolScalarField,
    pub u: outram_foam_basic_lib::prelude::VolVectorField,
    pub nu: outram_foam_basic_lib::prelude::VolScalarField,
    pub prt: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` |  |
| `nu_sgs` | `outram_foam_basic_lib::prelude::VolScalarField` | Sub-grid-scale kinematic viscosity ν_sgs [m²/s], = Ck·Δ·√k_sgs, ≥ 0. |
| `k_sgs` | `outram_foam_basic_lib::prelude::VolScalarField` | Sub-grid-scale kinetic energy k_sgs [m²/s²], the algebraic equilibrium<br>root of `a·k + b·√k − c = 0`, ≥ 0. |
| `u` | `outram_foam_basic_lib::prelude::VolVectorField` | Velocity field U [m/s] — set by the solver each step before `correct()`. |
| `nu` | `outram_foam_basic_lib::prelude::VolScalarField` | Molecular kinematic viscosity ν [m²/s]. |
| `prt` | `f64` | Turbulent Prandtl number Prt (dimensionless) for `alpha_eff` (default 0.85). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Construct a Smagorinsky model over `mesh` with the OpenFOAM default

- ```rust
  pub fn with_ck(self: Self, ck: f64) -> Self { /* ... */ }
  ```
  Builder override for the SGS eddy-viscosity coefficient Ck (dimensionless).

- ```rust
  pub fn with_ce(self: Self, ce: f64) -> Self { /* ... */ }
  ```
  Builder override for the SGS dissipation coefficient Ce (dimensionless).

- ```rust
  pub fn delta(self: &Self) -> &[f64] { /* ... */ }
  ```
  The per-cell `cubeRootVol` filter width Δ = V^(1/3) [m] (read-only).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    Turbulent deviatoric-stress divergence `∇·(−2 ν_eff dev(symm(∇U)))`.

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```
    Algebraic SGS update (no PDE solve). Ports `Smagorinsky.C:45-67`.

  - ```rust
    fn nu_t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_eff(self: &Self, alpha: &VolScalarField) -> VolScalarField { /* ... */ }
    ```
    Effective thermal diffusivity α_eff = α + α_t, α_t = ν_sgs / Prt [m²/s].

  - ```rust
    fn mu_eff_field(self: &Self, mu: &VolScalarField) -> VolScalarField { /* ... */ }
    ```
    Effective viscosity μ_eff = μ + μ_t (incompressible: ν + ν_sgs) [m²/s].

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `CK`

SGS eddy-viscosity coefficient Ck (dimensionless). Default from
`LESeddyViscosity.C:60`. Appears in `ν_sgs = Ck·Δ·√k` and in the
`k`-quadratic coefficient `c = 2·Ck·Δ·(dev(D):D)`.

```rust
pub const CK: f64 = 0.094;
```

#### Constant `CE`

SGS dissipation coefficient Ce (dimensionless). Default from
`LESeddyViscosity.C:61`. Appears in the `k`-quadratic coefficient
`a = Ce/Δ` (from `ε = Ce·k^(3/2)/Δ` under local equilibrium).

```rust
pub const CE: f64 = 1.048;
```

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

Spalart-Allmaras one-equation RAS turbulence model (Spalart & Allmaras 1992/1994).

A pure-Rust port of OpenFOAM's `SpalartAllmaras` eddy-viscosity model. It
solves a **single** transport equation for the Spalart-Allmaras working
variable ν̃ ("nuTilda", units m²/s) and recovers the turbulent kinematic
viscosity as `ν_t = ν̃ · fv1`. It is the standard low-cost closure for
attached external aerodynamics (aerofoils, wings, external flows).

## Transport equation (incompressible, α = ρ = 1)

```text
  ∂ν̃/∂t + ∇·(φ ν̃) − ∇·((ν+ν̃)/σ ∇ν̃) − Cb2/σ |∇ν̃|²
      = Cb1 · S̃ · ν̃  −  Cw1 · fw · (ν̃/y)²
```

with the auxiliary functions (all ported from the upstream `.C`):

```text
  χ   = ν̃/ν
  fv1 = χ³ / (χ³ + Cv1³)
  fv2 = 1 − χ / (1 + χ·fv1)
  Ω   = √2 · |skew(∇U)|                       (vorticity magnitude)
  S̃   = max(Ω + fv2·ν̃/(κ y)² ,  Cs·Ω)         (Spalart-limited modified vorticity)
  r   = min(ν̃ / (max(S̃, small)·(κ y)²), 10)
  g   = r + Cw2·(r⁶ − r)
  fw  = g · ((1 + Cw3⁶)/(g⁶ + Cw3⁶))^(1/6)
  ν_t = ν̃ · fv1
```

## Upstream provenance (line-by-line port)

`upstream_source/OpenFOAM/src/MomentumTransportModels/momentumTransportModels/RAS/SpalartAllmaras/SpalartAllmaras.C`:
- `chi()`   — `SpalartAllmaras.C:41-45`
- `fv1()`   — `SpalartAllmaras.C:48-56`
- `fv2()`   — `SpalartAllmaras.C:59-71`
- `Stilda()` (incl. Ω = √2·|skew(∇U)| and the `Cs·Ω` clip) — `SpalartAllmaras.C:74-100`
- `fw()` (r, g, fw) — `SpalartAllmaras.C:103-134`
- `correctNut()` (ν_t = ν̃·fv1) — `SpalartAllmaras.C:137-153`
- coefficient defaults & Cw1 = Cb1/κ² + (1+Cb2)/σ — `SpalartAllmaras.C:181-189`
- `DnuTildaEff()` = (ν̃+ν)/σ — `SpalartAllmaras.C:234-243`
- `correct()` ν̃ transport assembly — `SpalartAllmaras.C:294-343`

## State the model needs beyond the trait

`TurbulenceModel::correct(&mut self)` takes no arguments, but the model
needs the live velocity `U`, the face flux `phi`, the molecular viscosity
`ν`, the wall-distance field, and the time step. As in OpenFOAM (where the
model holds references to these), this struct stores them: set `u`, `phi`,
`nu`, and `dt` each time step before calling `correct()`. The wall-distance
field `y` is computed once at construction from the mesh `Wall` patches.

## Honest scope — what is NOT modelled

- **Trip terms omitted (as upstream).** OpenFOAM implements this model
  without the trip term, so the `ft2` term is not needed and is not
  included here either (see `SpalartAllmaras.H:38-39`). This is a faithful
  match to the upstream, not a simplification introduced by this port.
- **Wall boundary condition is zero-gradient, not the proper `nuTilda`
  wall function.** OpenFOAM applies a fixed-value ν̃ = 0 at walls (via the
  patch field) together with wall functions. Here every patch — walls
  included — uses a zero-gradient boundary via the `scalar_field` helper.
  Near-wall behaviour is therefore only approximate; this is adequate for
  the unit-level verification here but NOT validated against a wall-bounded
  benchmark (no law-of-the-wall / skin-friction validation has been run).
- **No `fvModels`/`fvConstraints` sources, no equation relaxation, no MRF.**
  The upstream `relax()`, `fvModels.source(...)`, and `fvConstraints`
  calls are dropped; only the bare transport + physics source/sink terms
  are assembled.
- **σ uses the exact rational 2/3 rather than the upstream literal 0.66666.**
  The difference (≈ 1e-5 relative) is numerically negligible; Cw1 is derived
  from it consistently.

AI-assisted port — treat as untrusted draft until human-reviewed. Verified
only at the unit level (see the inline test module); not validated against
an aerodynamic benchmark.

```rust
pub mod spalart_allmaras { /* ... */ }
```

### Types

#### Struct `SpalartAllmaras`

Spalart-Allmaras one-equation turbulence model (1992/1994).

Solves a single transport equation for the working variable ν̃ [m²/s] and
recovers ν_t = ν̃·fv1 [m²/s]. Common in aerospace applications (external
aerodynamics, aerofoils). Set `u`, `phi`, `nu`, `dt` before each
[`TurbulenceModel::correct`] call; the wall-distance `y` is fixed at
construction.

```rust
pub struct SpalartAllmaras {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub nu_tilde: outram_foam_basic_lib::prelude::VolScalarField,
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
| `nu_tilde` | `outram_foam_basic_lib::prelude::VolScalarField` | Spalart-Allmaras working variable ν̃ [m²/s] — NOT equal to ν_t directly. |
| `nu_t` | `outram_foam_basic_lib::prelude::VolScalarField` | Turbulent kinematic viscosity ν_t = ν̃ · fv1 [m²/s]. |
| `u` | `outram_foam_basic_lib::prelude::VolVectorField` | Velocity field U [m/s] — set by the solver each step before `correct()`. |
| `phi` | `outram_foam_basic_lib::prelude::SurfaceScalarField` | Face volumetric flux φ = U·Sf [m³/s] — set by the solver each step. |
| `nu` | `outram_foam_basic_lib::prelude::VolScalarField` | Molecular kinematic viscosity ν [m²/s]. |
| `dt` | `f64` | Time step Δt [s]. |
| `prt` | `f64` | Turbulent Prandtl number for `alpha_eff` (dimensionless). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Construct a Spalart-Allmaras model over `mesh` with the 1992/1994

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

Turbulent Prandtl-like diffusion constant σ (upstream `sigmaNut` = 0.66666).

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

#### Constant `CS`

Stilda limiter coefficient Cs — clips S̃ at Cs·Ω (Spalart's limiter).

```rust
pub const CS: f64 = 0.3;
```

## Module `traits`

The [`TurbulenceModel`] trait — the common contract every RAS/LES closure
in this crate implements.

The trait is a compile-time contract, not a dispatch mechanism: solvers hold
a concrete model type (or an enum over the models) and call it through
generics, so there is no `dyn` overhead. Every model in this crate (k-ω SST,
k-ε, Wilcox k-ω, Spalart-Allmaras, Smagorinsky LES, and laminar) implements
every method for real and is unit-tested — none `todo!()`-panic (see the
crate-level status table).

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

