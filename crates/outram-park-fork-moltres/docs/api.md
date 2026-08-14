# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `outram_park_fork_moltres`

# outram-park-fork-moltres

Circulating-fuel molten-salt-reactor (MSR) multiphysics on the
outram-foam **finite-volume** layer — the physics formulation of the
LGPL-2.1 [Moltres](https://github.com/arfc/moltres) code (multigroup
neutron diffusion + delayed-neutron precursor drift + salt heat
transfer), deliberately reimplemented on
[`outram_foam_basic_lib`]'s `FvMesh`/`fvm` operators instead of
MOOSE/PETSc finite elements. Not affiliated with the Moltres/ARFC
project.

> **⚠️ Untrusted AI-assisted draft — pending human V&V.** All physics
> here is verified only against analytic/limiting cases by automated
> tests (each test documents its methodology and measured results);
> no human review, no validation against MSRE benchmark data yet. Not
> for nuclear facility operation, reactor control, safety-critical, or
> licensing decisions (see the workspace `RESPONSIBLE_USE.md`).

## What it models (first pass)

- [`diffusion`] — static-fuel multigroup neutron-diffusion k-eigenvalue
  (power iteration over `fvm::laplacian + fvm::sp` group systems).
- [`precursors`] — delayed-neutron precursor **advection–decay drift**
  `dC_i/dt + div(u C_i) - div(D_C grad C_i) = beta_i/k S_f - lambda_i
  C_i`: the defining circulating-fuel physics.
- [`circulating`] — the coupled flux + drifting-precursor eigenvalue on
  a closed loop: reactivity falls with loop speed as precursors decay
  outside the core (the classic MSRE circulation loss).
- [`thermal`] — reduced slug-flow salt temperature + heat exchanger +
  linear cross-section temperature feedback, Picard-coupled.
- [`ring_mesh`] — the closed 1-D loop mesh (periodic topology via a
  ring of internal faces; no cyclic boundary machinery needed).
- [`materials`] — SI multigroup cross-section records and their
  materialisation to per-cell fields.

**Prescribed flow only:** the salt velocity is an input (rigid loop
circulation), not solved — full CFD coupling is the appbuilder/GeN-Foam
path. **Steady eigenvalue only** for the coupled system (precursor
transients exist as [`precursors::PrecursorDrift::step`], but there is
no coupled flux transient yet). Units are **SI (metres)** throughout;
see [`materials`] for cm → m conversion of standard reactor-physics
tables.

## Verification summary (measured 2026-08-04, release build)

| Check | Reference | Measured result |
|---|---|---|
| 1-group bare-slab k | analytic `nuSigma_f/(Sigma_a + D B^2)` | rel. err `6.3e-6` |
| 2-group bare-slab k | analytic two-group formula | rel. err `9.5e-7` |
| Zero-flow precursors | algebraic equilibrium `beta S_f/(k lambda)` | rel. err `2.3e-16` |
| Loop precursor balance | production = decay on closed loop | imbalance `<= 8.6e-11` |
| u = 0 circulating solver | equals static solver | `dk = 2.2e-16` |
| Circulation reactivity loss | monotone, `< beta`, MSRE-order | 151–388 pcm over 0.15–2.4 m/s (287 pcm at the MSRE-like nominal 0.6 m/s) |
| Loop energy balance | HX removal = deposited power; slug-flow `dT` | imbalance `1.1e-8`; `dT` matches analytic to 0.03 % |
| Feedback sign | k falls / T rises with power | monotone at 0.5/4/8 MW, ~170 pcm/MW |

(Each check records full methodology and the measured numbers in the
corresponding test's doc comment.)

## Modules

## Module `circulating`

Circulating-fuel k-eigenvalue: multigroup diffusion coupled to advected
delayed-neutron precursors — **the** MSRE effect.

The coupled steady eigenvalue system solved here is, per energy group
`g` and precursor family `i`:

```text
  -div(D_g grad phi_g) + Sigma_{r,g} phi_g
      = chi_{p,g} (1-beta)/k * S_f + chi_{d,g} S_d + S_{s,g}

  div(u C_i) - div(D_C grad C_i) + lambda_i C_i = beta_i/k * S_f
```

with `S_f = sum_g nuSigma_{f,g} phi_g`, `S_d = sum_i lambda_i C_i`,
`S_{s,g}` the in-scatter, and `u` the prescribed fuel-salt loop velocity.
When `u = 0` the precursor balance is the algebraic equilibrium and the
system reduces **exactly** to [`crate::diffusion::StaticDiffusion`]
(verified in tests). When `u > 0`, precursors drift out of the core and
decay in the external loop where their delayed neutrons cannot sustain
the chain reaction: `k_eff` (and hence the effective delayed fraction)
drops with loop speed. The reactivity difference
`rho(0) - rho(u)` is the classic "reactivity loss due to fuel
circulation" measured on the MSRE (~0.2 % dk/k at nominal flow).

Outer iteration per step: (1) solve every precursor family's steady
drift equation against the lagged fission source, (2) solve every
group's diffusion equation with prompt + delayed + in-scatter sources,
(3) update `k` from the fission-production integral ratio and
renormalise. Temperature feedback enters through
[`CirculatingFuelSolver::set_temperature`] (reduced linear
`dSigma_r/dT` model, see [`crate::materials`]).

Units: flux `1/(m^2 s)` (normalised), `C_i` `1/m^3`, `u` `m/s`
(`flow` as face flux `m^3/s`), `k_eff` dimensionless.

```rust
pub mod circulating { /* ... */ }
```

### Types

#### Struct `CirculatingFuelSolver`

Coupled flux + advected-precursor k-eigenvalue model (see module docs).

Build with [`CirculatingFuelSolver::new`], optionally apply a
temperature field with [`Self::set_temperature`], then call
[`Self::solve`]. Converged results stay in [`Self::flux`],
[`Self::precursors`], and [`Self::k_eff`] (flux normalised so the
fission-production integral is 1).

```rust
pub struct CirculatingFuelSolver {
    pub flux: Vec<crate::materials::NeutronFluxField>,
    pub precursors: Vec<crate::materials::PrecursorField>,
    pub k_eff: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `flux` | `Vec<crate::materials::NeutronFluxField>` | Group flux fields `phi_g` (`1/(m^2 s)`, normalised). |
| `precursors` | `Vec<crate::materials::PrecursorField>` | Precursor concentration fields `C_i` (`1/m^3`), one per family;<br>consistent with `flux` and `k_eff` after [`Self::solve`]. |
| `k_eff` | `f64` | Latest `k_eff` (dimensionless); 1.0 before the first solve. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(xs: XsFields, families: Vec<DelayedFamily>, flow: FaceFluxField, precursor_diffusion: f64, flux_boundary: &[BoundaryCondition<f64>], settings: EigenSettings) -> Result<Self, MoltresError> { /* ... */ }
  ```
  Build a circulating-fuel model.

- ```rust
  pub fn set_temperature(self: &mut Self, temperature: &TemperatureField, t_ref: f64) -> Result<(), MoltresError> { /* ... */ }
  ```
  Apply the reduced linear temperature feedback: recompute the

- ```rust
  pub fn solve(self: &mut Self) -> Result<EigenReport, MoltresError> { /* ... */ }
  ```
  Run the coupled outer iteration to convergence (methodology in the

- ```rust
  pub fn power_density_shape(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Un-normalised power-density shape `sum_g kappaSigma_{f,g} phi_g`

- ```rust
  pub fn xs(self: &Self) -> &XsFields { /* ... */ }
  ```
  The materialised cross sections this model was built from.

- ```rust
  pub fn beta_total(self: &Self) -> f64 { /* ... */ }
  ```
  Total delayed fraction `beta` of the configured families

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> CirculatingFuelSolver { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `diffusion`

Static-fuel multigroup neutron-diffusion k-eigenvalue solver.

Solves, for each energy group `g` on an [`FvMesh`], the steady multigroup
diffusion equation with the fission source scaled by `1/k`:

```text
  -div(D_g grad phi_g) + Sigma_{r,g} phi_g
      = chi_{t,g}/k * sum_g' nuSigma_{f,g'} phi_g'  +  sum_{g'!=g} Sigma_{g'->g} phi_g'
```

with the **total** spectrum `chi_{t,g} = chi_{p,g}(1-beta) + chi_{d,g}
beta` — i.e. the delayed-neutron precursors are taken at their zero-flow
equilibrium `C_i = beta_i S_f / (k lambda_i)`, so their decay source
collapses into the fission spectrum. This is the correct limit for
**static fuel** (`u = 0`); the flow-dependent solver in
[`crate::circulating`] replaces the equilibrium by the advected precursor
balance and reduces to this solver as `u -> 0`.

The eigenvalue is found by outer **power iteration**: each outer step
lags the fission source, solves every group's loss system
`fvm::laplacian(D_g) + fvm::sp(Sigma_r)` (symmetric positive definite —
solved with warm-started conjugate gradients), updates
`k <- k * F_new / F_old` from the fission-production integral `F`, and
renormalises the flux to `F = 1`.

Units: flux `1/(m^2 s)` (amplitude arbitrary up to normalisation),
`D` in `m`, all `Sigma` in `1/m`, `k_eff` dimensionless.

```rust
pub mod diffusion { /* ... */ }
```

### Types

#### Struct `EigenSettings`

Convergence controls for an outer power iteration (static or
circulating).

Defaults: `k` tolerance `1e-8`, flux tolerance `1e-7`, 5000 outer
iterations, inner linear solves to `1e-12` within 20000 iterations.
The generous outer budget matters for the **circulating** solver: the
delayed-neutron coupling through slowly-decaying, advected precursors
gives the outer fixed-point map a contraction ratio near 1 (measured
~0.99 per iteration on the MSRE-like test loop), so 1000–2000 outer
iterations are routinely needed — they are cheap (one CG + a few
Gauss-Seidel solves each). No Aitken/Chebyshev acceleration yet
(deferred, as in the workspace's GeN-Foam port).

```rust
pub struct EigenSettings {
    pub k_tolerance: f64,
    pub flux_tolerance: f64,
    pub max_outer_iterations: usize,
    pub linear: outram_foam_basic_lib::prelude::SolverSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_tolerance` | `f64` | Outer convergence tolerance on the relative change of `k_eff`<br>(dimensionless). |
| `flux_tolerance` | `f64` | Outer convergence tolerance on the relative L2 change of the group<br>fluxes (dimensionless). |
| `max_outer_iterations` | `usize` | Maximum outer (power) iterations before<br>[`MoltresError::NotConverged`]. |
| `linear` | `outram_foam_basic_lib::prelude::SolverSettings` | Inner linear-solver settings (per group / per precursor family). |

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
    fn clone(self: &Self) -> EigenSettings { /* ... */ }
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

- **Freeze**
- **From**
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
#### Struct `EigenReport`

Outcome of a converged (or abandoned) k-eigenvalue iteration.

```rust
pub struct EigenReport {
    pub k_eff: f64,
    pub outer_iterations: usize,
    pub k_residual: f64,
    pub flux_residual: f64,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | Effective multiplication factor `k_eff` (dimensionless). |
| `outer_iterations` | `usize` | Outer power iterations performed. |
| `k_residual` | `f64` | Final relative change in `k_eff` (dimensionless). |
| `flux_residual` | `f64` | Final relative L2 change in the flux (dimensionless). |
| `converged` | `bool` | True if both residuals met their tolerances. |

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
    fn clone(self: &Self) -> EigenReport { /* ... */ }
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

- **Freeze**
- **From**
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
#### Struct `StaticDiffusion`

Static-fuel multigroup diffusion model on an `FvMesh` (see the module
docs for the equations). Build with [`StaticDiffusion::new`], then call
[`StaticDiffusion::solve`]; the converged flux shape stays in
[`StaticDiffusion::flux`] (normalised so the fission-production integral
is 1 — scale to a target power afterwards if needed).

```rust
pub struct StaticDiffusion {
    pub flux: Vec<crate::materials::NeutronFluxField>,
    pub k_eff: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `flux` | `Vec<crate::materials::NeutronFluxField>` | Group flux fields `phi_g`, `1/(m^2 s)` up to normalisation; seeded<br>uniform, overwritten by [`Self::solve`]. |
| `k_eff` | `f64` | Latest `k_eff` (dimensionless); 1.0 before the first solve. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(xs: XsFields, families: &[DelayedFamily], flux_boundary: &[BoundaryCondition<f64>], settings: EigenSettings) -> Result<Self, MoltresError> { /* ... */ }
  ```
  Build a static diffusion model.

- ```rust
  pub fn solve(self: &mut Self) -> Result<EigenReport, MoltresError> { /* ... */ }
  ```
  Run the outer power iteration to convergence (see module docs for

- ```rust
  pub fn xs(self: &Self) -> &XsFields { /* ... */ }
  ```
  The materialised cross sections this model was built from.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> StaticDiffusion { /* ... */ }
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

#### Function `reactivity`

**Attributes:**

- `MustUse { reason: None }`

Static reactivity `rho = (k - 1)/k` of a multiplication factor
(dimensionless; multiply by `1e5` for pcm).

```rust
pub fn reactivity(k_eff: f64) -> f64 { /* ... */ }
```

## Module `error`

Error type shared by every solver in this crate.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `MoltresError`

Everything that can go wrong while building or running an MSR model.

Construction errors (`InvalidMaterial`, `SizeMismatch`, `InvalidMesh`)
indicate a caller mistake and are raised before any physics runs;
runtime errors (`NoFissionSource`, `NotConverged`, `LinearSolveFailed`)
indicate the configured problem has no computable answer within the
requested tolerances.

```rust
pub enum MoltresError {
    InvalidMaterial(String),
    SizeMismatch {
        what: &'static str,
        expected: usize,
        got: usize,
    },
    InvalidMesh(outram_foam_basic_lib::prelude::MeshError),
    NoFissionSource,
    NotConverged {
        outer_iterations: usize,
        k_residual: f64,
        flux_residual: f64,
    },
    LinearSolveFailed {
        field: String,
        residual: f64,
        iterations: usize,
    },
}
```

##### Variants

###### `InvalidMaterial`

A material record is internally inconsistent (wrong vector length,
negative cross section, fission spectrum not normalised, ...).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `SizeMismatch`

Two coupled arrays that must agree in length do not.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | What was being checked (e.g. `"zone_of_cell"`). |
| `expected` | `usize` | The length the mesh / group structure requires. |
| `got` | `usize` | The length actually supplied. |

###### `InvalidMesh`

The finite-volume mesh failed validation (from `outram-foam-basic-lib`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_foam_basic_lib::prelude::MeshError` |  |

###### `NoFissionSource`

The initial flux produces zero fission neutrons, so a k-eigenvalue is
undefined (non-multiplying configuration).

###### `NotConverged`

The outer (power) iteration exhausted its iteration budget.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `outer_iterations` | `usize` | Outer iterations performed before giving up. |
| `k_residual` | `f64` | Last relative change in `k_eff` (dimensionless). |
| `flux_residual` | `f64` | Last relative L2 change in the flux (dimensionless). |

###### `LinearSolveFailed`

An inner linear solve failed to reach its tolerance.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `String` | Name of the field being solved (e.g. `"precursor2"`). |
| `residual` | `f64` | Final normalised residual (dimensionless). |
| `iterations` | `usize` | Iterations performed. |

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
    fn from(source: MeshError) -> Self { /* ... */ }
    ```

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
## Module `materials`

Multigroup cross-section data and its materialisation onto an `FvMesh`.

Mirrors the Moltres material-property system (`GenericMoltresMaterial`'s
`diffcoef` / `remxs` / `nsf` / `chi_p` / `chi_d` / `gtransfxs` vectors) as
plain Rust structs, with one **crucial unit change**: everything here is
**SI (metres)**, because the `outram-foam-basic-lib` meshes this crate
builds on are in metres. Reactor-physics tables are usually in
centimetres — convert before constructing an [`MsrMaterial`]
(`Sigma[1/m] = 100 * Sigma[1/cm]`, `D[m] = D[cm] / 100`).

Temperature feedback is a **reduced linear model**: only the removal
(absorption + out-scatter) cross section carries a temperature derivative
`d Sigma_r / dT` (Moltres' `d_remxs_d_temp`), applied as
`Sigma_r(T) = Sigma_r(T_ref) + (dSigma_r/dT) (T - T_ref)`. Moltres itself
interpolates every group constant from tabulated `T` points; the linear
single-coefficient form is the documented first-pass simplification.

```rust
pub mod materials { /* ... */ }
```

### Types

#### Type Alias `NeutronFluxField`

Scalar neutron flux field, one value per cell. Units: `1/(m^2 s)`
(multiply by `1e-4` for the conventional `1/(cm^2 s)`).

```rust
pub type NeutronFluxField = outram_foam_basic_lib::prelude::VolScalarField;
```

#### Type Alias `PrecursorField`

Delayed-neutron precursor concentration field, one value per cell.
Units: `1/m^3`.

```rust
pub type PrecursorField = outram_foam_basic_lib::prelude::VolScalarField;
```

#### Type Alias `TemperatureField`

Fuel-salt temperature field, one value per cell. Units: `K`.

```rust
pub type TemperatureField = outram_foam_basic_lib::prelude::VolScalarField;
```

#### Type Alias `FaceFluxField`

Face volumetric flow flux `u . A_f`, one value per internal face.
Units: `m^3/s`. Positive = flow from face owner to face neighbour.

```rust
pub type FaceFluxField = outram_foam_basic_lib::prelude::SurfaceScalarField;
```

#### Struct `DelayedFamily`

One delayed-neutron precursor family.

Assumed uniform over the whole (well-mixed) fuel salt, which is why it is
a plain pair of numbers rather than a per-cell field (Moltres'
`beta_eff` / `decay_constant` material vectors, restricted to a single
fuel material).

```rust
pub struct DelayedFamily {
    pub beta: f64,
    pub lambda: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `beta` | `f64` | Delayed fraction `beta_i` of this family (dimensionless, typically<br>`1e-4 ..= 3e-3` per family; the 6-family total for U-235 is ~0.0065). |
| `lambda` | `f64` | Decay constant `lambda_i` in `1/s` (typically `0.01 ..= 3.0`). |

##### Implementations

###### Methods

- ```rust
  pub fn keepin_u235() -> Vec<DelayedFamily> { /* ... */ }
  ```
  The classic 6-family delayed-neutron data for thermal fission of

- ```rust
  pub fn total_beta(families: &[DelayedFamily]) -> f64 { /* ... */ }
  ```
  Sum of `beta_i` over a family list (the total delayed fraction

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DelayedFamily { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &DelayedFamily) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MsrMaterial`

Multigroup neutron-diffusion constants for one material zone.

All vectors are indexed by energy group `g = 0 .. G-1`, ordered from the
**highest** energy group to the lowest (the Moltres/Serpent convention).
**Units are SI (metres)** throughout — see the module docs for cm → m
conversion.

```rust
pub struct MsrMaterial {
    pub name: String,
    pub diffusion: Vec<f64>,
    pub sigma_removal: Vec<f64>,
    pub nu_sigma_f: Vec<f64>,
    pub chi_prompt: Vec<f64>,
    pub chi_delayed: Vec<f64>,
    pub scattering: Vec<Vec<f64>>,
    pub sigma_power: Vec<f64>,
    pub d_sigma_removal_d_temp: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable zone name (diagnostics only). |
| `diffusion` | `Vec<f64>` | Diffusion coefficient `D_g` in `m` (typically `0.003 ..= 0.03` m,<br>i.e. 0.3–3 cm). Moltres `diffcoef`. |
| `sigma_removal` | `Vec<f64>` | Removal cross section `Sigma_{r,g} = Sigma_{a,g} + sum_{g' != g}<br>Sigma_{g->g'}` in `1/m` (absorption **plus out-scatter**; the<br>in-scatter *into* `g` is handled separately via `scattering`).<br>Moltres `remxs`. |
| `nu_sigma_f` | `Vec<f64>` | Fission-production cross section `nu Sigma_{f,g}` in `1/m`<br>(zero in non-fuel zones). Moltres `nsf`. |
| `chi_prompt` | `Vec<f64>` | Prompt fission spectrum `chi_{p,g}` (dimensionless; sums to 1 over<br>groups in fissile zones, all-zero allowed in non-fuel zones).<br>Moltres `chi_p`. |
| `chi_delayed` | `Vec<f64>` | Delayed fission spectrum `chi_{d,g}` (dimensionless; sums to 1 in<br>fissile zones, all-zero allowed elsewhere). Moltres `chi_d`. |
| `scattering` | `Vec<Vec<f64>>` | Scattering matrix `scattering[g_from][g_to] = Sigma_{g_from -><br>g_to}` in `1/m`, with **zero diagonal** (within-group scattering is<br>already excluded from `sigma_removal`'s out-scatter sum). Moltres<br>`gtransfxs` off-diagonals. |
| `sigma_power` | `Vec<f64>` | Fission power conversion `kappa Sigma_{f,g}` in `J/m`, so the local<br>power density is `q''' = sum_g kappaSigma_{f,g} phi_g` in `W/m^3`.<br>(`kappa ~ 3.2e-11 J` per fission.) Zero in non-fuel zones. |
| `d_sigma_removal_d_temp` | `Vec<f64>` | Reduced linear temperature-feedback coefficient<br>`d Sigma_{r,g} / dT` in `1/(m K)` (Moltres `d_remxs_d_temp`;<br>positive = heating adds absorption = negative reactivity feedback). |

##### Implementations

###### Methods

- ```rust
  pub fn non_fuel</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, diffusion: Vec<f64>, sigma_removal: Vec<f64>) -> Self { /* ... */ }
  ```
  A non-multiplying material with every cross section zero except the

- ```rust
  pub fn validate(self: &Self, g: usize) -> Result<(), MoltresError> { /* ... */ }
  ```
  Check internal consistency against an expected group count `g`:

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MsrMaterial { /* ... */ }
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
    fn eq(self: &Self, other: &MsrMaterial) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `XsFields`

Cross sections materialised as per-cell fields on one `FvMesh`, ready for
finite-volume assembly. Built once by [`XsFields::materialize`]; the
per-zone [`MsrMaterial`] data is broadcast to cells through a
`zone_of_cell` map.

The diffusion coefficient is additionally interpolated to mesh faces
(**linear** face interpolation via `fvc::interpolate`; harmonic averaging
at strong material discontinuities is a documented future refinement)
because `fvm::laplacian` consumes a face field.

```rust
pub struct XsFields {
    pub energy_groups: usize,
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub diffusion_face: Vec<outram_foam_basic_lib::prelude::SurfaceScalarField>,
    pub sigma_removal: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub nu_sigma_f: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub chi_prompt: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub chi_delayed: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub scattering: Vec<Vec<outram_foam_basic_lib::prelude::VolScalarField>>,
    pub sigma_power: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub d_sigma_removal_d_temp: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `energy_groups` | `usize` | Number of energy groups `G` (>= 1). |
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` | The mesh the fields live on. |
| `diffusion_face` | `Vec<outram_foam_basic_lib::prelude::SurfaceScalarField>` | Face-interpolated diffusion coefficient `D_g` per group, `m`. |
| `sigma_removal` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Removal cross section `Sigma_{r,g}` per group at the reference<br>temperature, `1/m`. |
| `nu_sigma_f` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Fission production `nu Sigma_{f,g}` per group, `1/m`. |
| `chi_prompt` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Prompt spectrum `chi_{p,g}` per group, dimensionless. |
| `chi_delayed` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Delayed spectrum `chi_{d,g}` per group, dimensionless. |
| `scattering` | `Vec<Vec<outram_foam_basic_lib::prelude::VolScalarField>>` | Scattering transfer `Sigma_{g_from->g_to}` as `scattering[from][to]`,<br>`1/m` (zero diagonal). |
| `sigma_power` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Power conversion `kappa Sigma_{f,g}` per group, `J/m`. |
| `d_sigma_removal_d_temp` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Linear feedback coefficient `d Sigma_{r,g}/dT` per group, `1/(m K)`. |

##### Implementations

###### Methods

- ```rust
  pub fn materialize(mesh: Arc<FvMesh>, zone_of_cell: &[usize], materials: &[MsrMaterial]) -> Result<Self, MoltresError> { /* ... */ }
  ```
  Broadcast per-zone materials onto the mesh.

- ```rust
  pub fn sigma_removal_at(self: &Self, temperature: &TemperatureField, t_ref: f64) -> Result<Vec<VolScalarField>, MoltresError> { /* ... */ }
  ```
  Removal cross sections adjusted for a temperature field with the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> XsFields { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `precursors`

Delayed-neutron precursor advection–decay ("drift") transport.

For each precursor family `i` the concentration `C_i` (`1/m^3`) obeys

```text
  dC_i/dt + div(u C_i) - div(D_C grad C_i) = beta_i/k * S_f - lambda_i C_i
```

with `S_f = sum_g nuSigma_{f,g} phi_g` the fission-neutron production
(`1/(m^3 s)`), `u` the fuel-salt velocity, and `D_C` a small (numerical /
turbulent) precursor diffusivity in `m^2/s`. **The advection term is the
defining MSRE physics**: precursors born in the core are carried out into
the external loop by the flowing salt and decay there, where their
delayed neutrons are useless — so reactivity depends on the loop
velocity. The `1/k` factor on the production term is the k-eigenvalue
convention (pass `k = 1` for physical transients).

Both a steady solve (for eigenvalue outer iterations) and a
backward-Euler transient step are provided. Spatial discretisation is
first-order upwind for advection (`fvm::div`) and Gauss-orthogonal for
diffusion (`fvm::laplacian`); the asymmetric system is solved with
Gauss-Seidel, which converges because decay plus upwinding keep the
matrix diagonally dominant.

```rust
pub mod precursors { /* ... */ }
```

### Types

#### Struct `PrecursorDrift`

Advection–decay transport of the delayed-neutron precursor families on a
fixed flow field. Construct once with [`PrecursorDrift::new`], then call
[`PrecursorDrift::solve_steady`] (eigenvalue outer loops) or
[`PrecursorDrift::step`] (transients).

```rust
pub struct PrecursorDrift {
    pub families: Vec<crate::materials::DelayedFamily>,
    pub flow: crate::materials::FaceFluxField,
    pub diffusion: f64,
    pub linear: outram_foam_basic_lib::prelude::SolverSettings,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `families` | `Vec<crate::materials::DelayedFamily>` | Delayed families (`beta_i` dimensionless, `lambda_i` in `1/s`). |
| `flow` | `crate::materials::FaceFluxField` | Face volumetric flow flux `u . A_f` (`m^3/s`); positive =<br>owner-to-neighbour. Zero everywhere = static fuel. |
| `diffusion` | `f64` | Precursor diffusivity `D_C` (`m^2/s`, uniform, >= 0; typically a<br>small numerical/turbulent value like `1e-4` — molecular diffusion of<br>precursor nuclides is negligible, but a nonzero value smooths the<br>upwind advection front, mirroring Moltres' artificial-diffusion<br>stabilisation). |
| `linear` | `outram_foam_basic_lib::prelude::SolverSettings` | Linear-solver settings for each family's Gauss-Seidel solve. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, families: Vec<DelayedFamily>, flow: FaceFluxField, diffusion: f64, linear: SolverSettings) -> Result<Self, MoltresError> { /* ... */ }
  ```
  Build a precursor transport model.

- ```rust
  pub fn solve_steady(self: &Self, fission_source: &VolScalarField, k_eff: f64) -> Result<Vec<PrecursorField>, MoltresError> { /* ... */ }
  ```
  Steady advection–decay balance for every family:

- ```rust
  pub fn step(self: &Self, previous: &[PrecursorField], fission_source: &VolScalarField, k_eff: f64, dt: f64) -> Result<Vec<PrecursorField>, MoltresError> { /* ... */ }
  ```
  One backward-Euler step of length `dt` (s):

- ```rust
  pub fn delayed_source(self: &Self, precursors: &[PrecursorField]) -> VolScalarField { /* ... */ }
  ```
  Delayed-neutron volumetric source `S_d[c] = sum_i lambda_i C_i[c]`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PrecursorDrift { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `ring_mesh`

A closed 1-D "ring" finite-volume mesh for the MSR primary loop.

The circulating-fuel effect needs a **periodic** 1-D domain: fuel salt
leaves the top of the core, travels around the external loop (pump + heat
exchanger), and re-enters the core bottom. `outram-foam-basic-lib`'s
matrix assembly has no cyclic boundary-patch coupling, so instead the
loop is built as a mesh whose face topology **is** a ring: `n` cells,
`n` internal faces, face `i` joining cell `i` to cell `i+1` and the last
face joining cell `n-1` back to cell `0`. There are **no boundary
patches at all** — every finite-volume operator sees a purely internal,
periodic domain, which is exactly the physics of a closed loop.

To keep the cell-centre distances that `fvm::laplacian` uses consistent
at the wrap-around face, the cells are laid out on a **circle** in the
x-y plane (radius `R = L / 2 pi`). Every adjacent-cell distance is then
the same chord length `2 R sin(pi/n)`, which underestimates the arc
spacing `dx = L/n` by a uniform relative `O((pi/n)^2 / 6)` (`< 1e-4` for
`n >= 130`) — a documented, mesh-refinement-vanishing geometric bias.
Cell volumes use the exact arc measure `dx * A`.

```rust
pub mod ring_mesh { /* ... */ }
```

### Types

#### Struct `RingMesh`

A closed 1-D loop mesh plus its loop-level metadata.

Construct with [`RingMesh::new`]; the underlying [`FvMesh`] (shared
`Arc`) is in `mesh`. Cell `i` spans arc length
`[i dx, (i+1) dx)` measured from an arbitrary loop origin, so zone maps
(core vs external loop) are most naturally written against
[`RingMesh::arc_centre`].

```rust
pub struct RingMesh {
    pub mesh: std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>,
    pub circumference: f64,
    pub flow_area: f64,
    pub n_cells: usize,
    pub dx: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>` | The underlying finite-volume mesh (no boundary patches; `n` cells,<br>`n` internal faces, face `n-1` wraps from cell `n-1` to cell `0`). |
| `circumference` | `f64` | Loop circumference `L` in `m` (total salt path length). |
| `flow_area` | `f64` | Flow cross-sectional area `A` in `m^2` (uniform). |
| `n_cells` | `usize` | Number of cells `n` (>= 3). |
| `dx` | `f64` | Arc-length cell spacing `dx = L/n` in `m`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(circumference: f64, flow_area: f64, n_cells: usize) -> Result<Self, MoltresError> { /* ... */ }
  ```
  Build a closed loop of `n_cells` cells with total path length

- ```rust
  pub fn arc_centre(self: &Self, cell: usize) -> f64 { /* ... */ }
  ```
  Arc-length coordinate of cell `i`'s centre, `s_i = (i + 1/2) dx` in

- ```rust
  pub fn uniform_flux(self: &Self, speed: f64) -> FaceFluxField { /* ... */ }
  ```
  Face volumetric flux for a rigid-loop circulation at salt speed

- ```rust
  pub fn two_zone_map(self: &Self, core_length: f64) -> Vec<usize> { /* ... */ }
  ```
  Two-zone map: zone `0` ("core") for cells whose arc centre lies in

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RingMesh { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `thermal`

Reduced fuel-salt thermal model and the power/temperature-feedback
coupling loop.

**Deliberately reduced first pass — not CFD.** The salt moves as a rigid
slug at the prescribed loop speed (same [`FaceFluxField`] the precursors
use); the steady temperature `T` (`K`) on the closed loop obeys

```text
  rho c_p div(u T) - div(k_T grad T) + h_v m_HX (T - T_HX) = q'''
```

where `q'''` (`W/m^3`) is the fission heat deposited in the core
(Moltres `FissionHeatSource`, normalised to a target total power),
`h_v` (`W/(m^3 K)`) is a volumetric heat-exchanger conductance active
only on the HX section mask `m_HX` (Moltres `ConvectiveHeatExchanger`),
and `T_HX` (`K`) is the secondary-side temperature. Momentum, buoyancy,
turbulence, and conjugate structures are all out of scope here — the
full-CFD path lives in `outram-foam-appbuilder-lib` / GeN-Foam.

[`CoupledMsrSolver`] closes the multiphysics loop: eigenvalue → power
shape scaled to target power → temperature → linear cross-section
feedback → eigenvalue …, Picard-iterated with under-relaxation.

```rust
pub mod thermal { /* ... */ }
```

### Types

#### Struct `SaltThermalConfig`

Configuration of the reduced salt thermal model. All properties uniform
over the loop (well-mixed salt, single phase).

```rust
pub struct SaltThermalConfig {
    pub rho_cp: f64,
    pub conductivity: f64,
    pub hx_conductance: f64,
    pub hx_temperature: f64,
    pub hx_mask: Vec<bool>,
    pub linear: outram_foam_basic_lib::prelude::SolverSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rho_cp` | `f64` | Volumetric heat capacity `rho c_p` in `J/(m^3 K)` (molten fluoride<br>salts: ~4e6). |
| `conductivity` | `f64` | Thermal conductivity `k_T` in `W/(m K)` (salts: ~1; nearly<br>irrelevant next to advection at loop Peclet numbers, kept for<br>completeness and for the `u = 0` limit). |
| `hx_conductance` | `f64` | Volumetric heat-exchanger conductance `h_v` in `W/(m^3 K)`,<br>applied only on HX cells. |
| `hx_temperature` | `f64` | Secondary-side (coolant) temperature `T_HX` in `K`. |
| `hx_mask` | `Vec<bool>` | Per-cell HX mask: `true` where the heat exchanger removes heat.<br>Length must equal `mesh.n_cells`. |
| `linear` | `outram_foam_basic_lib::prelude::SolverSettings` | Linear-solver settings for the (asymmetric) temperature solve. |

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
    fn clone(self: &Self) -> SaltThermalConfig { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SaltThermalModel`

Steady fuel-salt temperature model on a loop mesh (see module docs).

```rust
pub struct SaltThermalModel {
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
  pub fn new(mesh: Arc<FvMesh>, config: SaltThermalConfig) -> Result<Self, MoltresError> { /* ... */ }
  ```
  Build the model; validates the HX mask length and property signs.

- ```rust
  pub fn solve_steady(self: &Self, flow: &FaceFluxField, heat_source: &VolScalarField) -> Result<(TemperatureField, SolverPerformance), MoltresError> { /* ... */ }
  ```
  Solve the steady temperature for a given loop flow and heat source.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SaltThermalModel { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CoupledReport`

Result of a converged neutronics–thermal Picard iteration.

```rust
pub struct CoupledReport {
    pub eigen: crate::diffusion::EigenReport,
    pub temperature: crate::materials::TemperatureField,
    pub heat_source: outram_foam_basic_lib::prelude::VolScalarField,
    pub picard_iterations: usize,
    pub temperature_residual: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `eigen` | `crate::diffusion::EigenReport` | The final eigenvalue report (at the converged temperature field). |
| `temperature` | `crate::materials::TemperatureField` | Converged salt temperature (`K`). |
| `heat_source` | `outram_foam_basic_lib::prelude::VolScalarField` | Heat source scaled to the target power (`W/m^3`). |
| `picard_iterations` | `usize` | Picard (outer multiphysics) iterations performed. |
| `temperature_residual` | `f64` | Final max-norm temperature change between Picard iterates (`K`). |

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
    fn clone(self: &Self) -> CoupledReport { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CoupledMsrSolver`

Picard-coupled circulating-fuel neutronics + salt temperature +
cross-section feedback (see module docs). The neutronics and thermal
models must share one mesh and one flow field.

```rust
pub struct CoupledMsrSolver {
    pub neutronics: crate::circulating::CirculatingFuelSolver,
    pub thermal: SaltThermalModel,
    pub flow: crate::materials::FaceFluxField,
    pub target_power: f64,
    pub t_ref: f64,
    pub relaxation: f64,
    pub max_picard_iterations: usize,
    pub temperature_tolerance: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `neutronics` | `crate::circulating::CirculatingFuelSolver` | Circulating-fuel neutronics (owned; query `neutronics.k_eff`,<br>`.flux`, `.precursors` after solving). |
| `thermal` | `SaltThermalModel` | Reduced salt thermal model. |
| `flow` | `crate::materials::FaceFluxField` | Loop flow shared by both physics (`m^3/s` per face). |
| `target_power` | `f64` | Target total fission power in `W` (the flux is rescaled so<br>`int q''' dV` equals this). |
| `t_ref` | `f64` | Reference temperature `T_ref` (`K`) of the cross-section data. |
| `relaxation` | `f64` | Picard under-relaxation factor in `(0, 1]` (0.7 is robust). |
| `max_picard_iterations` | `usize` | Max Picard iterations. |
| `temperature_tolerance` | `f64` | Convergence tolerance on the max temperature change (`K`). |

##### Implementations

###### Methods

- ```rust
  pub fn solve(self: &mut Self) -> Result<CoupledReport, MoltresError> { /* ... */ }
  ```
  Run the Picard loop: eigenvalue → power → temperature → feedback →

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `prelude`

Convenience re-exports of the crate's main types.

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `CirculatingFuelSolver`

```rust
pub use crate::circulating::CirculatingFuelSolver;
```

#### Re-export `reactivity`

```rust
pub use crate::diffusion::reactivity;
```

#### Re-export `EigenReport`

```rust
pub use crate::diffusion::EigenReport;
```

#### Re-export `EigenSettings`

```rust
pub use crate::diffusion::EigenSettings;
```

#### Re-export `StaticDiffusion`

```rust
pub use crate::diffusion::StaticDiffusion;
```

#### Re-export `MoltresError`

```rust
pub use crate::error::MoltresError;
```

#### Re-export `DelayedFamily`

```rust
pub use crate::materials::DelayedFamily;
```

#### Re-export `FaceFluxField`

```rust
pub use crate::materials::FaceFluxField;
```

#### Re-export `MsrMaterial`

```rust
pub use crate::materials::MsrMaterial;
```

#### Re-export `NeutronFluxField`

```rust
pub use crate::materials::NeutronFluxField;
```

#### Re-export `PrecursorField`

```rust
pub use crate::materials::PrecursorField;
```

#### Re-export `TemperatureField`

```rust
pub use crate::materials::TemperatureField;
```

#### Re-export `XsFields`

```rust
pub use crate::materials::XsFields;
```

#### Re-export `PrecursorDrift`

```rust
pub use crate::precursors::PrecursorDrift;
```

#### Re-export `RingMesh`

```rust
pub use crate::ring_mesh::RingMesh;
```

#### Re-export `CoupledMsrSolver`

```rust
pub use crate::thermal::CoupledMsrSolver;
```

#### Re-export `CoupledReport`

```rust
pub use crate::thermal::CoupledReport;
```

#### Re-export `SaltThermalConfig`

```rust
pub use crate::thermal::SaltThermalConfig;
```

#### Re-export `SaltThermalModel`

```rust
pub use crate::thermal::SaltThermalModel;
```

