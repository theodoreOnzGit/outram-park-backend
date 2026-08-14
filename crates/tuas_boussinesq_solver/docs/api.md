# Crate Documentation

**Version:** 0.1.5

**Format Version:** 60

# Module `tuas_boussinesq_solver`

A Library which contains useful traits and methods for thermal
hydraulics calculations in salt loops


This crate has heavy reliance on units of measure (uom) released under
Apache 2.0 license. So you'll need to get used to unit safe calculations
with uom as well.


This library was initially developed for
use in my PhD thesis under supervision
of Professor Per F. Peterson. It a thermal hydraulics
library in Rust that is released under the GNU General Public License
v 3.0. This is partly due to the fact that some of the libraries
inherit from GeN-Foam and OpenFOAM, both licensed under GNU General
Public License v3.0.

As such, the entire library is released under GNU GPL v3.0. It is a strong
copyleft license which means you cannot use it in proprietary software.


License
   This is a thermal hydraulics library written
   in rust meant to help with the
   fluid mechanics and heat transfer aspects of the calculations
   for the Compact Integral Effects Tests (CIET) and hopefully
   Gen IV Reactors such as the Fluoride Salt cooled High Temperature
   Reactor (FHR)
     
   Copyright (C) 2022-2023  Theodore Kay Chen Ong, Singapore Nuclear
   Research and Safety Initiative, Per F. Peterson, University of
   California, Berkeley Thermal Hydraulics Laboratory

   tuas_boussinesq_solver is free software; you can
   redistribute it and/or modify it
   under the terms of the GNU General Public License as published by the
   Free Software Foundation; either version 2 of the License, or (at your
   option) any later version.

   tuas_boussinesq_solver is distributed in the hope
   that it will be useful, but WITHOUT
   ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
   FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
   for more details.

   This thermal hydraulics library
   contains some code copied from GeN-Foam, and OpenFOAM derivative.
   This offering is not approved or endorsed by the OpenFOAM Foundation nor
   OpenCFD Limited, producer and distributor of the OpenFOAM(R)software via
   www.openfoam.com, and owner of the OPENFOAM(R) and OpenCFD(R) trademarks.
   Nor is it endorsed by the authors and owners of GeN-Foam.

   You should have received a copy of the GNU General Public License
   along with this program.  If not, see <http://www.gnu.org/licenses/>.

© All rights reserved. Theodore Kay Chen Ong,
Singapore Nuclear Research and Safety Initiative,
Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Laboratory

Main author of the code: Theodore Kay Chen Ong, supervised by
Professor Per F. Peterson

Btw, I no affiliation with the Rust Foundation.


## Modules

## Module `tuas_lib_error`

provides error types for tuas_boussinesq_solver
Crate-wide error type ([`TuasLibError`]).

Every fallible operation in `tuas_boussinesq_solver` returns
`Result<_, TuasLibError>`. Variants cover array/dimension shape mismatches,
an empty mass-flowrate vector (so a Courant number cannot be formed),
thermophysical-property failures (including a temperature that falls
outside a property correlation's valid range), wrong heat-transfer
interaction / entity / material types, and a catch-all string error. A
`From<String>`/`Into<String>` bridge is provided for interop with the many
string-based error sites in the codebase.

```rust
pub mod tuas_lib_error { /* ... */ }
```

### Types

#### Enum `TuasLibError`

Master Error type of this crate

```rust
pub enum TuasLibError {
    ShapeMismatch(String),
    CourantMassFlowVectorEmpty,
    GenericStringError(String),
    NotImplementedForBoundaryConditions(String),
    TypeConversionErrorHeatTransferEntity,
    TypeConversionErrorMaterial,
    ThermophysicalPropertyTemperatureRangeError {
        material: String,
        temperature: uom::si::f64::ThermodynamicTemperature,
        lower_bound: uom::si::f64::ThermodynamicTemperature,
        upper_bound: uom::si::f64::ThermodynamicTemperature,
    },
    CorrelationRangeError {
        parameter: String,
        value: f64,
        lower_bound: f64,
        upper_bound: f64,
        units: String,
    },
    ThermophysicalPropertyError,
    WrongHeatTransferInteractionType,
}
```

##### Variants

###### `ShapeMismatch`

array shape / dimension mismatch (replaces the former ndarray-linalg LinalgError)

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `CourantMassFlowVectorEmpty`

empty mass flowrate vector error

this case is where the mass flowrate vector in a control
volume is empty,
so we can't calculate a courant number

###### `GenericStringError`

it's a generic error which is a placeholder since I used
so many string errors

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NotImplementedForBoundaryConditions`

error to indicate that function is not implemented for BC

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `TypeConversionErrorHeatTransferEntity`

error for type conversions for heat transfer entity

###### `TypeConversionErrorMaterial`

error for type conversions for material

###### `ThermophysicalPropertyTemperatureRangeError`

A temperature fell outside the valid range of a thermophysical-property
correlation.

The payload names **which** material was asked, at **what** temperature,
and **what** range it is tabulated over. Without those four facts the
error is unactionable: a bare "out of range" tells a caller nothing
about which of its many nodes failed, and that is exactly how an
`htgr_sim_v1` steam-generator failure once presented -- a panic naming
no material, no node, and no temperature.

Construct it with [`TuasLibError::temperature_out_of_range`] rather than
by hand, so the field order cannot be transposed.

# Fields

- `material` — the material's `Debug` rendering. It is a `String` and
  not a `Material` deliberately: this module is Layer 0 and
  `boussinesq_thermophysical_properties` is Layer 1, so holding the real
  type here would invert the crate's module layering.
- `temperature` — the temperature actually requested.
- `lower_bound` / `upper_bound` — the inclusive limits of the
  correlation's tabulated range.

All three temperatures are `uom`-typed; the message renders them in both
kelvin and degrees Celsius, because property tables in this crate are
written in kelvin while plant data is almost always quoted in Celsius.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `material` | `String` | `Debug` rendering of the material that was asked (see the variant<br>docs for why this is not a `Material`). |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | The temperature that was requested. |
| `lower_bound` | `uom::si::f64::ThermodynamicTemperature` | Inclusive lower limit of the correlation's tabulated range. |
| `upper_bound` | `uom::si::f64::ThermodynamicTemperature` | Inclusive upper limit of the correlation's tabulated range. |

###### `CorrelationRangeError`

An input other than temperature fell outside a correlation's validated
envelope.

Distinct from
[`TuasLibError::ThermophysicalPropertyTemperatureRangeError`], which is
specifically about a *temperature* against a *property table*. This one
covers any other bounded correlation input — neutron fluence, Reynolds
number, Prandtl number, a geometric ratio — where the quantity is not a
temperature and the bound is not a tabulated property range.

It exists because those checks were previously reported as temperature
range errors, which forced a caller reading the message to be told a
temperature had gone out of range when no temperature was involved.

# Fields

- `parameter` — the name of the quantity, as a reader would recognise it
  (e.g. `"fluence gam"`, `"Reynolds number"`).
- `value` — the value supplied.
- `lower_bound` / `upper_bound` — the inclusive limits of the validated
  envelope.
- `units` — the units `value` and the bounds are expressed in, spelled
  out for a human (e.g. `"10^25 n/m^2, E > 0.1 MeV"`, `"dimensionless"`).
  These are plain `f64` rather than `uom` quantities precisely because
  the variant is generic over quantity kinds, so the unit cannot be
  carried in the type; naming it here is the substitute.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `parameter` | `String` | Human-recognisable name of the out-of-range quantity. |
| `value` | `f64` | The value that was supplied. |
| `lower_bound` | `f64` | Inclusive lower limit of the validated envelope. |
| `upper_bound` | `f64` | Inclusive upper limit of the validated envelope. |
| `units` | `String` | Units of `value` and the bounds, spelled out for a human reader. |

###### `ThermophysicalPropertyError`

generic thermophysical property error

###### `WrongHeatTransferInteractionType`

wrong heat transfer interaction type

##### Implementations

###### Methods

- ```rust
  pub fn temperature_out_of_range</* synthetic */ impl core::fmt::Debug: core::fmt::Debug>(material: impl core::fmt::Debug, temperature: ThermodynamicTemperature, lower_bound: ThermodynamicTemperature, upper_bound: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Builds a [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`]

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn from(value: String) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> String { /* ... */ }
    ```

- **Read**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `matrix`

Pure-Rust dense LU solver (`SquareMatrix`), inlined from `outram-foam-basic-lib`
so this crate has no `outram-foam-basic-lib` dependency — see the module doc.
Pure-Rust dense LU solver — an **inlined copy** of
`outram-foam-basic-lib`'s `matrix::SquareMatrix`.

This is a *deliberate duplication*: `tuas_boussinesq_solver` needs only this small
self-contained LU solver from `outram-foam-basic-lib`, so the code is copied
here to remove the `outram-foam-basic-lib` path dependency entirely. That keeps
the inter-crate dependency graph acyclic when a future full `tampines`
(multiphase-flow) crate and `nee_soon` compose `teh-o-prke` and
`tuas_boussinesq_solver` together — decoupling is preferred over DRY here.
`teh-o-prke` carries an identical inlined copy for the same reason. If the
LU algorithm is ever changed, update all copies.

Pure `std` — no `ndarray`, no BLAS/LAPACK — so it also builds unchanged on
Android (see the workspace CLAUDE.md "Android portability" rule).

```rust
pub mod matrix { /* ... */ }
```

### Types

#### Enum `MatrixError`

Error type for `SquareMatrix::solve`.

```rust
pub enum MatrixError {
    Singular {
        col: usize,
    },
}
```

##### Variants

###### `Singular`

The matrix is exactly singular: the LU decomposition found a zero pivot
at the given column (the entire remaining column was zero).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `col` | `usize` | Column index whose pivot was zero. |

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
    fn clone(self: &Self) -> MatrixError { /* ... */ }
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
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MatrixError) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `SquareMatrix`

Row-major n×n dense matrix of `f64`. Maps to `Foam::scalarSquareMatrix`.

LU decomposition uses Crout's algorithm with scaled partial pivoting,
matching `Foam::LUDecompose(scalarSquareMatrix&, labelList&)`.

```rust
pub struct SquareMatrix {
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
  pub fn new(n: usize) -> Self { /* ... */ }
  ```
  Create an `n`x`n` matrix filled with zeros.

- ```rust
  pub fn n(self: &Self) -> usize { /* ... */ }
  ```
  Matrix order `n` (it is `n`x`n`).

- ```rust
  pub fn get(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```
  Element `(i, j)` (row-major, 0-indexed).

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Set element `(i, j)` to `v`.

- ```rust
  pub fn add(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Add `v` to element `(i, j)` (accumulate a coefficient).

- ```rust
  pub fn fill_zero(self: &mut Self) { /* ... */ }
  ```
  Reset every element to zero (reuse the allocation across assemblies).

- ```rust
  pub fn lu_decompose(self: &mut Self) -> Vec<usize> { /* ... */ }
  ```
  In-place LU decomposition with scaled partial pivoting.

- ```rust
  pub fn lu_back_substitute(self: &Self, pivot: &[usize], b: &mut Vec<f64>) { /* ... */ }
  ```
  Solve `LU·x = b` in-place (`b` is overwritten with the solution).

- ```rust
  pub fn solve(self: &Self, rhs: &[f64]) -> Result<Vec<f64>, MatrixError> { /* ... */ }
  ```
  Convenience: decompose a copy and solve `A·x = b`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SquareMatrix { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `prelude`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

prelude, for easy importing
Convenience re-export surface for `tuas_boussinesq_solver`.

Glob-import one of the submodules to pull in the crate's common public
types (control volumes, boundary conditions, materials, heat-transfer
entities and property helpers) without naming their full module paths.
`beta_testing` is the current, near-stable prelude; `alpha_nightly` is
reserved for unstable, in-development re-exports.

```rust
pub mod prelude { /* ... */ }
```

### Modules

## Module `alpha_nightly`

for code currently unstable and under development
Unstable, in-development prelude re-exports.

Reserved for experimental items whose API is not yet settled. It is
currently empty; use `prelude::beta_testing` for the near-stable public
import surface.

```rust
pub mod alpha_nightly { /* ... */ }
```

## Module `beta_testing`

for code currently moving towards a stable API and read for public testing
Near-stable prelude: the recommended `use
tuas_boussinesq_solver::prelude::beta_testing::*;` import surface.

Re-exports the crate's commonly used public items — the error type,
control volumes and heat-transfer entities (`SingleCVNode`, `FluidArray`,
`SolidColumn`, `HeatTransferEntity`), boundary conditions (`BCType`),
materials, a set of pre-built CIET components, thermophysical-property
helper functions, and the heat-transfer interaction enums and geometric
dimension newtypes.

```rust
pub mod beta_testing { /* ... */ }
```

### Re-exports

#### Re-export `TuasLibError`

thermal hydraulics library error

```rust
pub use crate::tuas_lib_error::TuasLibError;
```

#### Re-export `HeatTransferEntity`

heat transfer entities
Fluid arrays and solid arrays

```rust
pub use crate::pre_built_components::heat_transfer_entities::HeatTransferEntity;
```

#### Re-export `FluidArray`

```rust
pub use crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray;
```

#### Re-export `SolidColumn`

```rust
pub use crate::array_control_vol_and_fluid_component_collections::one_d_solid_array_with_lateral_coupling::SolidColumn;
```

#### Re-export `Material`

```rust
pub use crate::boussinesq_thermophysical_properties::Material;
```

#### Re-export `LiquidMaterial`

```rust
pub use crate::boussinesq_thermophysical_properties::LiquidMaterial;
```

#### Re-export `SolidMaterial`

```rust
pub use crate::boussinesq_thermophysical_properties::SolidMaterial;
```

#### Re-export `BCType`

```rust
pub use crate::boundary_conditions::BCType;
```

#### Re-export `SingleCVNode`

```rust
pub use crate::single_control_vol::SingleCVNode;
```

#### Re-export `HeaterTopBottomHead`

```rust
pub use crate::pre_built_components::ciet_heater_top_and_bottom_head_bare::HeaterTopBottomHead;
```

#### Re-export `StructuralSupport`

```rust
pub use crate::pre_built_components::ciet_struct_supports::StructuralSupport;
```

#### Re-export `InsulatedPorousMediaFluidComponent`

```rust
pub use crate::pre_built_components::insulated_porous_media_fluid_components::InsulatedPorousMediaFluidComponent;
```

#### Re-export `NonInsulatedPorousMediaFluidComponent`

```rust
pub use crate::pre_built_components::non_insulated_porous_media_fluid_components::NonInsulatedPorousMediaFluidComponent;
```

#### Re-export `link_heat_transfer_entity`

```rust
pub use crate::pre_built_components::heat_transfer_entities::preprocessing::link_heat_transfer_entity;
```

#### Re-export `try_get_mu_viscosity`

```rust
pub use crate::boussinesq_thermophysical_properties::dynamic_viscosity::try_get_mu_viscosity;
```

#### Re-export `try_get_prandtl`

```rust
pub use crate::boussinesq_thermophysical_properties::prandtl::try_get_prandtl;
```

#### Re-export `try_get_kappa_thermal_conductivity`

```rust
pub use crate::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;
```

#### Re-export `try_get_rho`

```rust
pub use crate::boussinesq_thermophysical_properties::density::try_get_rho;
```

#### Re-export `crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::*`

```rust
pub use crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::*;
```

#### Re-export `crate::control_volume_dimensions::*`

```rust
pub use crate::control_volume_dimensions::*;
```

## Module `boussinesq_thermophysical_properties`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

Module specifically for thermophysical properties
For liquids and solids with almost invariable density

This module contains a library of liquid and solid
thermophysical properties

```rust
pub mod boussinesq_thermophysical_properties { /* ... */ }
```

### Modules

## Module `density`

Density calculation

```rust
pub mod density { /* ... */ }
```

### Functions

#### Function `try_get_rho`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns a density given a material, temperature and pressure

example:

```rust
use uom::si::f64::*;
use uom::si::pressure::atmosphere;
use uom::si::thermodynamic_temperature::kelvin;
use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::density::try_get_rho;

use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::SolidMaterial::SteelSS304L;

use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::Material;

let steel = Material::Solid(SteelSS304L);
let temperature = ThermodynamicTemperature::new::<kelvin>(396.0);
let pressure = Pressure::new::<atmosphere>(1.0);

let density_result = try_get_rho(steel, temperature, pressure);


```

```rust
pub fn try_get_rho(material: super::Material, temperature: uom::si::f64::ThermodynamicTemperature, _pressure: uom::si::f64::Pressure) -> Result<uom::si::f64::MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `thermal_conductivity`

Thermal conductivity calculation

```rust
pub mod thermal_conductivity { /* ... */ }
```

### Functions

#### Function `try_get_kappa_thermal_conductivity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns thermal conductivity for a given material

```rust
use uom::si::f64::*;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
SolidMaterial::SteelSS304L;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
Material;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
thermal_conductivity::try_get_kappa_thermal_conductivity;

use uom::si::pressure::atmosphere;

let steel = Material::Solid(SteelSS304L);
let steel_temp = ThermodynamicTemperature::new::<kelvin>(350.0);
let pressure = Pressure::new::<atmosphere>(1.0);

// at 350K, we should expect thermal conductivity,
// 15.58 W/(m K)

let steel_thermal_cond: ThermalConductivity =
try_get_kappa_thermal_conductivity(steel, steel_temp, pressure).unwrap();

// Residuals from Graves et al. was about 3% at 350K for least
// squares regression. So 2.8% error is reasonable

approx::assert_relative_eq!(
    15.58,
    steel_thermal_cond.value,
    max_relative=0.028);

```

```rust
pub fn try_get_kappa_thermal_conductivity(material: super::Material, temperature: ThermodynamicTemperature, _pressure: Pressure) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `specific_heat_capacity`

SpecificHeatCapacity calculation

```rust
pub mod specific_heat_capacity { /* ... */ }
```

### Functions

#### Function `try_get_cp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns cp for a given material

```rust
use uom::si::f64::*;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
SolidMaterial::SteelSS304L;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
Material;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
specific_heat_capacity::try_get_cp;

use uom::si::pressure::atmosphere;

let steel = Material::Solid(SteelSS304L);
let steel_temp = ThermodynamicTemperature::new::<kelvin>(350.0);
let pressure = Pressure::new::<atmosphere>(1.0);

// at 350K, we should expect a specific heat capacity of
// approx 470 J/(kg K)

let steel_cp: SpecificHeatCapacity =
try_get_cp(steel, steel_temp, pressure).unwrap();


approx::assert_relative_eq!(
    470.0,
    steel_cp.value,
    max_relative=0.035);

```

```rust
pub fn try_get_cp(material: super::Material, temperature: ThermodynamicTemperature, _pressure: Pressure) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `dynamic_viscosity`

dynamic viscosity calculation

```rust
pub mod dynamic_viscosity { /* ... */ }
```

### Functions

#### Function `try_get_mu_viscosity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns a dynamic_viscosity given a material, temperature and pressure

example:

```rust
use uom::si::f64::*;
use uom::si::pressure::atmosphere;
use uom::si::thermodynamic_temperature::kelvin;
use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::dynamic_viscosity::try_get_mu_viscosity;

use tuas_boussinesq_solver::boussinesq_thermophysical_properties
::LiquidMaterial::DowthermA;

use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::Material;

let dowtherm_a = Material::Liquid(DowthermA);
let temperature = ThermodynamicTemperature::new::<kelvin>(350.0);
let pressure = Pressure::new::<atmosphere>(1.0);

let dynamic_viscosity_result =
try_get_mu_viscosity(dowtherm_a, temperature, pressure);

approx::assert_relative_eq!(
    0.001237,
    dynamic_viscosity_result.unwrap().value,
    max_relative=0.01);

```

```rust
pub fn try_get_mu_viscosity(material: super::Material, temperature: ThermodynamicTemperature, _pressure: Pressure) -> Result<DynamicViscosity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `specific_enthalpy`

specific enthalpy calculation
Specific-enthalpy lookups and their temperature inverse for all database
materials.

This module dispatches on `Material` (solid or liquid) to compute specific
enthalpy (J/kg) at a given temperature via `try_get_h`, and to recover the
temperature (K) from a specific enthalpy via `try_get_temperature_from_h`.
The enthalpy reference (h = 0 J/kg) is 0 degrees Celsius (273.15 K) for the
spline-based solids; each liquid correlation uses its own coded reference
(typically the lower bound of its validity range) as documented in
`liquid_database`. The pressure argument is accepted for interface
uniformity but is not used (these are incompressible-liquid / solid
correlations).

`enthalpy_data` holds the per-material enthalpy correlations;
`temperature_from_specific_enthalpy` holds the inverse (root-finding /
spline) maps.

```rust
pub mod specific_enthalpy { /* ... */ }
```

### Modules

## Module `enthalpy_data`

contains specific enthalpy data for all materials

```rust
pub mod enthalpy_data { /* ... */ }
```

### Functions

#### Function `try_get_h`

returns specific enthaply for a given material
specific_enthalpy is defined as 0 for 0 degree_celsius
for any material, that is 273.15 K

```rust
use uom::si::f64::*;
use uom::si::specific_heat_capacity::{joule_per_kilogram_kelvin,
joule_per_gram_degree_celsius};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::temperature_interval::degree_celsius;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
SolidMaterial::{SteelSS304L,Copper};
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
Material;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
specific_enthalpy::try_get_h;

use uom::si::pressure::atmosphere;

let steel = Material::Solid(SteelSS304L);
let steel_temp = ThermodynamicTemperature::new::<kelvin>(273.15);
let pressure = Pressure::new::<atmosphere>(1.0);

// enthalpy should be zero at 273.15 K

let steel_enthalpy_273_15_kelvin =
try_get_h(steel, steel_temp, pressure);

approx::assert_relative_eq!(
    0.0,
    steel_enthalpy_273_15_kelvin.unwrap().value,
    max_relative=0.045);

// we can also calculate enthalpy change of copper
// from 375K to 425K
let test_temperature_1 = ThermodynamicTemperature::new::
<kelvin>(375.0);
let test_temperature_2 = ThermodynamicTemperature::new::
<kelvin>(425.0);

let copper = Material::Solid(Copper);

let copper_enthalpy_change =
try_get_h(copper, test_temperature_2, pressure).unwrap()
- try_get_h(copper, test_temperature_1, pressure).unwrap();

// http://hyperphysics.phy-astr.gsu.edu/hbase/Tables/sphtt.html
// https://www.engineeringtoolbox.com/specific-heat-metals-d_152.html
// copper at 20C has heat capacity of
// 0.386 J/(g K)
// going to use this to estimate a ballpark figure to find enthalpy
// h = cp(T2 - T1)

// we can't usually subtract thermodynamic temperatures from each
// other, we need a termpature interval
//

let cp_copper_20_c =
SpecificHeatCapacity::new::<joule_per_gram_degree_celsius>(0.386);

let temperature_difference =
TemperatureInterval::new::<degree_celsius>(
test_temperature_2.value - test_temperature_1.value);

let specific_enthalpy_ballpark =
cp_copper_20_c * temperature_difference;

// the ballpark value is 19300 J/kg
approx::assert_relative_eq!(
    specific_enthalpy_ballpark.value,
    19300.0,
    max_relative=0.0001);

// it's less than 4% different from the ballpark value
// This means the copper enthalpy change should be quite reasonable

approx::assert_relative_eq!(
    specific_enthalpy_ballpark.value,
    copper_enthalpy_change.value,
    max_relative=0.04);

```

```rust
pub fn try_get_h(material: super::Material, temperature: ThermodynamicTemperature, _pressure: Pressure) -> Result<AvailableEnergy, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `try_get_temperature_from_h`

This function allows you to obtain ThermodynamicTemperature
from AvailableEnergy (a.k.a specific enthalpy) of a material
as long as we have the material in the database

example:
```rust
use uom::si::f64::*;
use uom::si::specific_heat_capacity::{joule_per_kilogram_kelvin,
joule_per_gram_degree_celsius};
use uom::si::thermodynamic_temperature::{kelvin,degree_celsius};
use uom::si::temperature_interval::degree_celsius as
interval_degree_celsius;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
SolidMaterial::{SteelSS304L,Copper};
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
Material;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
specific_enthalpy::try_get_h;

use uom::si::pressure::atmosphere;

use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
specific_enthalpy::try_get_temperature_from_h;


// let's get steel at 20 degree_celsius

let steel = Material::Solid(SteelSS304L);
let steel_temp = ThermodynamicTemperature::new::<degree_celsius>(20.0);
let atmospheric_pressure = Pressure::new::<atmosphere>(1.0);

let enthalpy_spline_zweibaum = try_get_h(
    steel,steel_temp,atmospheric_pressure).unwrap();

// now this enthalpy value is about
// 9050 J/kg +/- 0.5 J/kg
// the epsilon here is just round off error
// NOT measurement uncertainty or anything else
let round_off_error = 0.5;

approx::assert_abs_diff_eq!(
    enthalpy_spline_zweibaum.value,
    9050_f64,
    epsilon=round_off_error);

// let's use this enthalpy value to get a ThermodynamicTemperature

let steel_temperature_test =
try_get_temperature_from_h(steel,
enthalpy_spline_zweibaum,
atmospheric_pressure).unwrap();

// this should get back 20 degrees C with 0.001 degree_celsius of
// error at most
approx::assert_abs_diff_eq!(
    steel_temperature_test.get::<degree_celsius>(),
    20_f64,
    epsilon=0.001);
```

```rust
pub fn try_get_temperature_from_h(material: super::Material, material_enthalpy: AvailableEnergy, _pressure: Pressure) -> Result<ThermodynamicTemperature, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `thermal_diffusivity`

thermal diffusivity

```rust
pub mod thermal_diffusivity { /* ... */ }
```

### Functions

#### Function `try_get_alpha_thermal_diffusivity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates thermal diffusivity of a material
```rust
use uom::si::f64::*;
use uom::si::pressure::atmosphere;
use uom::si::thermodynamic_temperature::degree_celsius;
use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::thermal_diffusivity::try_get_alpha_thermal_diffusivity;
use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;

use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::SolidMaterial::SteelSS304L;

use tuas_boussinesq_solver::
boussinesq_thermophysical_properties::Material;

let steel = Material::Solid(SteelSS304L);
let temperature = ThermodynamicTemperature::new
::<degree_celsius>(80.0);
let pressure = Pressure::new::<atmosphere>(1.0);

let thermal_diffusivity_result = try_get_alpha_thermal_diffusivity(
steel, temperature, pressure).unwrap();
  
// thermal diffusivity of ss304L is approx 4.13e-6 m^2/s
approx::assert_relative_eq!(
thermal_diffusivity_result.value,
4.13e-6,
epsilon = 0.001);

// conductivity is approx 15.62 W/(m K)
let steel_thermal_cond: ThermalConductivity =
try_get_kappa_thermal_conductivity(steel, temperature, pressure).unwrap();

approx::assert_relative_eq!(
steel_thermal_cond.value,
15.62,
max_relative = 0.035);
```

```rust
pub fn try_get_alpha_thermal_diffusivity(material: super::Material, temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<DiffusionCoefficient, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `momentum_diffusivity`

momentum diffusivity (or kinematic viscosity)

```rust
pub mod momentum_diffusivity { /* ... */ }
```

### Functions

#### Function `try_get_nu_momentum_diffusivity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

gets kinematic viscosity

```rust
pub fn try_get_nu_momentum_diffusivity(material: super::Material, temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<DiffusionCoefficient, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `volumetric_heat_capacity`

volumetric_heat_capacity

```rust
pub mod volumetric_heat_capacity { /* ... */ }
```

### Functions

#### Function `try_get_rho_cp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates volumetric_heat_capacity of a material

```rust
pub fn try_get_rho_cp(material: super::Material, temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<VolumetricHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `prandtl`

prandtl number

```rust
pub mod prandtl { /* ... */ }
```

### Functions

#### Function `try_get_prandtl`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

provides the prandtl number

```rust
pub fn try_get_prandtl(material: super::Material, temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<Ratio, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `solid_material_surface_roughness`

surface roughness

```rust
pub mod solid_material_surface_roughness { /* ... */ }
```

## Module `temperature_ranges`

functions for temperature ranges
this gives the max or min temperatures for each material

```rust
pub mod temperature_ranges { /* ... */ }
```

## Module `liquid_database`

database for liquids
Liquid coolant thermophysical-property correlations.

Each submodule holds the temperature-dependent property correlations for
one liquid coolant in the TUAS database: density (kg/m^3), dynamic
viscosity (Pa·s), thermal conductivity (W/(m·K)), constant-pressure
specific heat capacity (J/(kg·K)), specific enthalpy (J/kg), and the
inverse enthalpy-to-temperature map. Every correlation is range-checked
against its coded validity window (in kelvin or degrees Celsius) and
returns a `uom`-typed quantity.

Fluids covered: Dowtherm A / Therminol VP-1 (`dowtherm_a`), HITEC nitrate
salt (`hitec_nitrate_salt`), YD-325 heat-transfer oil
(`yd_325_heat_transfer_oil`), FLiBe (`flibe`), and FLiNaK (`flinak`).
`custom_liquid_material` provides generic, user-supplied-correlation
helpers for a liquid not otherwise in the database.

```rust
pub mod liquid_database { /* ... */ }
```

### Modules

## Module `dowtherm_a`

property correlations for dowtherm_a,
also known as therminol vp1

```rust
pub mod dowtherm_a { /* ... */ }
```

### Functions

#### Function `get_dowtherm_a_density`

function to obtain dowtherm A density
given a temperature

```rust
pub fn get_dowtherm_a_density(fluid_temp: ThermodynamicTemperature) -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_dowtherm_a_viscosity`

function to obtain dowtherm A viscosity
given a temperature

```rust
pub fn get_dowtherm_a_viscosity(fluid_temp: ThermodynamicTemperature) -> Result<DynamicViscosity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_dowtherm_a_constant_pressure_specific_heat_capacity`

function to obtain dowtherm A specific heat capacity
given a temperature

```rust
pub fn get_dowtherm_a_constant_pressure_specific_heat_capacity(fluid_temp: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_dowtherm_a_thermal_conductivity`

function to obtain dowtherm A thermal conductivity
given a temperature

```rust
pub fn get_dowtherm_a_thermal_conductivity(fluid_temp: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_dowtherm_a_enthalpy`

function to obtain dowtherm A enthalpy
given a temperature


This is done via analytically integrating
the function for specific heat capacity of
dowtherm A

However,
the thing is that with enthalpy
we need a reference value
i take the reference value to be 0 J/kg enthalpy at 20C
integrating heat capacity with respect to T, we get

cp = 1518 + 2.82*T

H = 1518*T + 2.82/2.0*T^2 + C
at T = 20C,
H = 30924 + C
H = 0
C = -30924 (i used libre office to calculate this)

Example use:
```rust

use uom::si::f64::*;
use uom::si::thermodynamic_temperature::kelvin;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
liquid_database::dowtherm_a::get_dowtherm_a_enthalpy;


let temp1 = ThermodynamicTemperature::new::<kelvin>(303_f64);

let specific_enthalpy_1 =
get_dowtherm_a_enthalpy(temp1);


let expected_enthalpy: f64 =
1518_f64*30_f64 + 2.82/2.0*30_f64.powf(2_f64) - 30924_f64;

// the expected value is about 15885 J/kg

extern crate approx;
approx::assert_relative_eq!(expected_enthalpy, specific_enthalpy_1.unwrap().value,
max_relative=0.02);
```

```rust
pub fn get_dowtherm_a_enthalpy(fluid_temp: ThermodynamicTemperature) -> Result<AvailableEnergy, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_temperature_from_enthalpy`

function to obtain dowtherm A temperature
given a enthalpy


This is done via analytically integrating
the function for specific heat capacity of
dowtherm A

However,
the thing is that with enthalpy
we need a reference value
i take the reference value to be 0 J/kg enthalpy at 20C
integrating heat capacity with respect to T, we get

cp = 1518 + 2.82*T

H = 1518*T + 2.82/2.0*T^2 + C
at T = 20C,
H = 30924 + C
H = 0
C = -30924 (i used libre office to calculate this)

Once i have this correlation, i will use
an iterative root finding method to find the temperature

As of Oct 2022, it is bisection

Example:

```rust
use uom::si::f64::*;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::available_energy::joule_per_kilogram;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::
liquid_database::dowtherm_a::get_temperature_from_enthalpy;


let specific_enthalpy_1 = AvailableEnergy::new::
<joule_per_kilogram>(15885.0);

let temp_expected = ThermodynamicTemperature::new::
<kelvin>(303_f64);

let temp_acutal = get_temperature_from_enthalpy(
specific_enthalpy_1).unwrap();


extern crate approx;
approx::assert_relative_eq!(temp_expected.value,
temp_acutal.value,
max_relative=0.01);


```

```rust
pub fn get_temperature_from_enthalpy(fluid_enthalpy: AvailableEnergy) -> Result<ThermodynamicTemperature, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `range_check_dowtherm_a`

function checks if a fluid temperature falls in a range (20-180C)

If it falls outside this range, it will panic
or throw an error, and the program will not run

TODO: find a dowtherm a correlation with larger temperature range
of validity

```rust
pub fn range_check_dowtherm_a(fluid_temp: ThermodynamicTemperature) -> Result<bool, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `max_temp_dowtherm_a`

dowtherm a max temp

```rust
pub fn max_temp_dowtherm_a() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_dowtherm_a`

dowtherm a min temp

```rust
pub fn min_temp_dowtherm_a() -> ThermodynamicTemperature { /* ... */ }
```

## Module `hitec_nitrate_salt`

property correlations for hitec (a nitrate salt)
Du, Bao-Cun, et al. "Investigation on
heat transfer characteristics of molten salt in a
shell-and-tube heat exchanger." International Communications
in Heat and Mass Transfer 96 (2018): 61-68.

```rust
pub mod hitec_nitrate_salt { /* ... */ }
```

### Functions

#### Function `get_hitec_density`

function to obtain nitrate salt density
given a temperature

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.


rho (kg/m3) = 2280.22  - 0.733 T(K)

```rust
pub fn get_hitec_density(fluid_temp: ThermodynamicTemperature) -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_hitec_dynamic_viscosity`

function to obtain nitrate salt viscosity
given a temperature

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

mu Pa-s (T = 440 - 500 K)
= 0.93845
-0.54754 T(K)
+ 1.08225e-5 T(K)^2
- 7.2058e-9 T(K)^3

mu Pa-s (T = 500 - 800 K)
= 0.23816
- 1.2768e-3 T(K)
+ 2.6275e-6 T(K)^2
- 2.4331e-9 T(K)^3
+ 8.507e-13 T(K)^4

Bohlmann, E. G. (1972). HEAT TRANSFER SALT FOR HIGH TEMPERATURE
STEAM GENERATION (No. ORNL-TM-3777). Oak Ridge National
Lab.(ORNL), Oak Ridge, TN (United States).

given the complicated looking correlations, it's always good to
against data. I'm using Bohlman's data for HITEC salt in 1972
as comparison. Fig 6 on page 25 of the document shows a graph
of HITEC salt viscosity in centipoises against temperature in
Fahrenheit

Using graphreader, I got the following pieces of data for viscosity
in cP against temp in Fahrenheit (roughly, the curve axes were
tilted)


315.282,15.039
336.479,12.087
346.338,10.984
375.915,8.642
399.577,7.362
440.986,5.709
498.169,4.272
585.915,3.051
653.944,2.5
730.845,1.988
832.394,1.555
928.521,1.28

I can use a simple test to ascertain if the viscosity is close
to this value





```rust
pub fn get_hitec_dynamic_viscosity(fluid_temp: ThermodynamicTemperature) -> Result<DynamicViscosity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_hitec_constant_pressure_specific_heat_capacity`

function to obtain nitrate salt specific heat capacity
given a temperature
Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

cp (J/kg/K) = 1560.0
T in kelvin

Now, Sohal has a different correlation for cp

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and
thermochemical properties (No. INL/EXT-10-18297).
Idaho National Lab.(INL), Idaho Falls, ID (United States).

but I'm not going to consider that yet


```rust
pub fn get_hitec_constant_pressure_specific_heat_capacity(fluid_temp: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_hitec_thermal_conductivity`

function to obtain nitrate salt thermal conductivity
given a temperature

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

k (thermal conductivity in W/mK for T = 536-800 kelvin) =
0.7663 - 6.47e-4 T(K)

k (thermal conductivity in W/mK for T = 420-536 kelvin) =
2.2627 - 0.01176 T(K)
+ 2.551e-5 T(K)^2
- 1.863e-8 T(K)^3

T in kelvin

```rust
pub fn get_hitec_thermal_conductivity(fluid_temp: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_hitec_specific_enthalpy`

function to obtain nitrate salt specific enthalpy
given a temperature
Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

cp (J/kg/K) = 1560.0
T in kelvin

Manual integration with temperature yields:

h (J/kg) = 1560.0 T(K) + Constant

I can just adjust the enthalpy to be 0 J/kg at 440K

0 J/kg = 1560 * T_0 (K) + Constant
Constant = 0 - 1560 T_0 (K)
Constant = 0 - 1560 * 440
Constant = 0 - 686,400

h (J/kg) = 1560.0 T(K) - 686400
h (J/kg) = - 686400 + 1560.0 T(K)




```rust
pub fn get_hitec_specific_enthalpy(fluid_temp: ThermodynamicTemperature) -> Result<AvailableEnergy, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_temperature_from_enthalpy`

function to obtain nitrate salt temperature from specific enthalpy
Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.


Note that the enthalpy equation was derived from manual
integration of cp assuming 0 J/kg at 440K (the minimum temperature)

0 J/kg = 1560 * T_0 (K) + Constant
Constant = 0 - 1560 T_0 (K)
Constant = 0 - 1560 * 440
Constant = 0 - 686,400

h (J/kg) = 1560.0 T(K) - 686400
h (J/kg) = - 686400 + 1560.0 T(K)



```rust
pub fn get_temperature_from_enthalpy(fluid_enthalpy: AvailableEnergy) -> Result<ThermodynamicTemperature, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `range_check_hitec_salt`

function checks if a fluid temperature falls in a range

If it falls outside this range, it will panic
or throw an error, and the program will not run

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68./// Jana, S. S.,

From HITEC, the applicable range is 440K - 800 K,

In Du's paper, the viscosity correlation is applicable from 440 to 800K
while the rest of the properties are from 420-800K



```rust
pub fn range_check_hitec_salt(fluid_temp: ThermodynamicTemperature) -> Result<bool, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `max_temp_hitec`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

hitec max temp

```rust
pub fn max_temp_hitec() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_hitec`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

hitec min temp

```rust
pub fn min_temp_hitec() -> ThermodynamicTemperature { /* ... */ }
```

## Module `yd_325_heat_transfer_oil`

property correlations for YD-325 heat transfer oil
Du, Bao-Cun, et al. "Investigation on
heat transfer characteristics of molten salt in a
shell-and-tube heat exchanger." International Communications
in Heat and Mass Transfer 96 (2018): 61-68.


Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
An experimental study on the heat transfer performance of a prototype
molten-salt rod baffle heat exchanger for concentrated solar power.
Energy, 156, 63-72.

```rust
pub mod yd_325_heat_transfer_oil { /* ... */ }
```

### Functions

#### Function `get_yd325_density`

function to obtain yd_325_heat_transfer_oil density
given a temperature

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
An experimental study on the heat transfer performance of a prototype
molten-salt rod baffle heat exchanger for concentrated solar power.
Energy, 156, 63-72.

rho (kg/m3) = 1199.13  - 0.6311 T(K)

```rust
pub fn get_yd325_density(fluid_temp: ThermodynamicTemperature) -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_yd325_dynamic_viscosity`

function to obtain yd_325_heat_transfer_oil viscosity
given a temperature

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

mu Pa-s (T = 323-423 K)
= 0.33065
- 2.283e-3 T(K)
+ 5.2746e-6 T(K)^2
- 4.066e-9 T(K)^3

mu Pa-s (T = 423-523 K)
= 0.05989
- 3.452e-4 T(K)
+ 6.735e-7 T(K)^2
- 4.413e-10 T(K)^3






```rust
pub fn get_yd325_dynamic_viscosity(fluid_temp: ThermodynamicTemperature) -> Result<DynamicViscosity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_yd325_constant_pressure_specific_heat_capacity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

function to obtain yd_325_heat_transfer_oil specific heat capacity
given a temperature
Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
An experimental study on the heat transfer performance of a prototype
molten-salt rod baffle heat exchanger for concentrated solar power.
Energy, 156, 63-72.

cp (J/kg/K) = 776.0 + 3.40 T(K)
T in kelvin



```rust
pub fn get_yd325_constant_pressure_specific_heat_capacity(fluid_temp: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_yd325_thermal_conductivity`

function to obtain yd_325_heat_transfer_oil thermal conductivity
given a temperature

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
An experimental study on the heat transfer performance of a prototype
molten-salt rod baffle heat exchanger for concentrated solar power.
Energy, 156, 63-72.

lambda = 0.1416 - 6.68e-5 T(K)

T in kelvin

```rust
pub fn get_yd325_thermal_conductivity(fluid_temp: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_yd325_specific_enthalpy`

function to obtain yd_325_heat_transfer_oil specific enthalpy
given a temperature

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
An experimental study on the heat transfer performance of a prototype
molten-salt rod baffle heat exchanger for concentrated solar power.
Energy, 156, 63-72.

cp (J/kg/K) = 776.0 + 3.40 T(K)
T in kelvin

Manual integration with temperature yields:

h (J/kg) = 776.0 T(K) + 3.40 * 0.5 T(K)^2 + Constant

Now, I can just "cheat" and perform a definite integral
The reference temperature can be the lower bound temperature of
323K






```rust
pub fn get_yd325_specific_enthalpy(fluid_temp: ThermodynamicTemperature) -> Result<AvailableEnergy, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_temperature_from_enthalpy`

function to obtain YD-325 heat transfer oil temperature (K) from
specific enthalpy (J/kg)

Inverts `get_yd325_specific_enthalpy` by bisection over the correlation's
validity range 323 K - 523 K. Enthalpy below 0 J/kg (the h = 0 reference
at the 323 K lower bound) is out of range and panics.

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
An experimental study on the heat transfer performance of a prototype
molten-salt rod baffle heat exchanger for concentrated solar power.
Energy, 156, 63-72.




```rust
pub fn get_temperature_from_enthalpy(fluid_enthalpy: AvailableEnergy) -> Result<ThermodynamicTemperature, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `range_check_yd325_oil`

function checks if a fluid temperature falls in a range

If it falls outside this range, it will panic
or throw an error, and the program will not run

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68./// Jana, S. S.,

Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
An experimental study on the heat transfer performance of a prototype
molten-salt rod baffle heat exchanger for concentrated solar power.
Energy, 156, 63-72.

From YD-325, the applicable range is 323K - 523 K,

In Qiu's paper, the viscosity correlation is applicable from 323-523 K
while the rest of the properties are from 300-573 K



```rust
pub fn range_check_yd325_oil(fluid_temp: ThermodynamicTemperature) -> Result<bool, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `max_temp_yd325_oil`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

yd325_oil max temp

```rust
pub fn max_temp_yd325_oil() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_yd325_oil`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

yd325_oil min temp

```rust
pub fn min_temp_yd325_oil() -> ThermodynamicTemperature { /* ... */ }
```

## Module `flibe`

FLiBe,

Composition:
LiF 67 mol%
BeF2 33 mol%

Viscosity correlations for FLiBe may differ in composition slightly,
due to different compositions of FLiBe used for different ranges

Thermal conductivity is in the range of 1.1 W/(m K) in 873K to 1073K,
and Sohal's correlation was originally for 500-650 K
Which is strangely below the melting
point of flibe
but based on Romatoski's data, I found that Romatoski's data
could fit Sohal's correlation to within 10% error
even up to 1123 K in my PhD Thesis

Therefore I could use it in the whole temperature range from
500 - 1123K .


Ong, T. K. C. (2024). Digital Twins as Testbeds for
Iterative Simulated Neutronics Feedback Controller
Development (Doctoral dissertation, UC Berkeley).



Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

```rust
pub mod flibe { /* ... */ }
```

### Functions

#### Function `get_flibe_density`

function to obtain flibe salt density
given a temperature

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.
properties for a custom liquid material
not covered in the database

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).


rho (kg/m3) = 2415.6 - 0.49072 T(K)
Density correlation applies from melting point to critical point
732.2 - 4498.8 K

There is slight non-linearity for flibe density
but I'm ignoring that for now

```rust
pub fn get_flibe_density(fluid_temp: ThermodynamicTemperature) -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_flibe_dynamic_viscosity`

function to obtain flibe salt viscosity
given a temperature

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.
properties for a custom liquid material
not covered in the database

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

Romatoski writes that Gierszewski et al. had a correlation for
66 mol% LiF, 34 mol% BeF2 for dynamic_viscosity in cP, temperature
in kelvin

mu (cP) = 0.116 exp(3760/T(K))
Applicable from 600-1200 K

Beyond this range, there is no viscosity data for this same composition,
but Romatoski writes that Abe et al, had data for
66 mol% LiF, 34 mol% BeF2 for dynamic_viscosity in cP, temperature
in kelvin

mu (cP) = 0.07803 exp(4022/T(K))
Applicable from 812.5 - 1573 K


There is some discrepancy within the literature data,
but I suppose for this code,
Abe's correlation can work from 1200 - 1573 K


There will be obvious discontinuity at 1200K, but I'll leave it
for future patches

in totality, 600-1573 K is reasonable, but
freezing point is 732.2



```rust
pub fn get_flibe_dynamic_viscosity(fluid_temp: ThermodynamicTemperature) -> Result<DynamicViscosity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_flibe_constant_pressure_specific_heat_capacity`

function to obtain flibe salt specific heat capacity
given a temperature
Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.
properties for a custom liquid material
not covered in the database

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

It is quite invariant with temperature

values range from 2415.8 J/(kg K) to 2386 J/(kg K)

Lichtenstein had a cp value of 1860 J/(kg K), but this
lowered value was attributed to BeO impurities within the FLiBe

Lichtenstein, T., Rose, M. A., Krueger, J., Wu, E., &
Williamson, M. A. (2022). Thermochemical Property Measurements of
FLiNaK and FLiBe in FY 2020 (No. ANL/CFCT-20/37 Rev. 1).
Argonne National Lab.(ANL), Argonne, IL (United States).

It is more reasonable to take the 2386 J/(kg K) value as this had an
uncertainty of +/- 3%

the 2415.6 value had an uncertainty of about +/- 20%

The temperature range is for all fluid temperatures, from
732.2K all the way up to 4498.8K (ish), the triple point



```rust
pub fn get_flibe_constant_pressure_specific_heat_capacity(fluid_temp: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_flibe_thermal_conductivity`

function to obtain flibe salt thermal conductivity
given a temperature.
Data was obtained from the following publications

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

Thermal conductivity is in the range of 1.1 W/(m K) in 873K to 1073K,
and Sohal's correlation was originally for 500-650 K
Which is strangely below the melting
point of flibe
but based on Romatoski's data, I found that Romatoski's data
could fit Sohal's correlation to within 10% error
even up to 1123 K in my PhD Thesis

Therefore I could use it in the whole temperature range from
500 - 1123K .


Ong, T. K. C. (2024). Digital Twins as Testbeds for
Iterative Simulated Neutronics Feedback Controller
Development (Doctoral dissertation, UC Berkeley).

k (thermal conductivity in W/mK for T = 500-1123 kelvin) =
0.629697 + 0.0005 T[K]

For more than 1123 K, I'll just let the conductivity be
the value obtained at 1123 K, seeing how in Sohal, the value is
does not seem to vary greatly with temperature

T in kelvin

```rust
pub fn get_flibe_thermal_conductivity(fluid_temp: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_flibe_specific_enthalpy`

function to obtain flibe salt specific enthalpy
given a temperature

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

cp (J/kg/K) = 2389.0, T in kelvin

Manual integration with temperature yields:

h (J/kg) = 2389.0 T(K) + Constant

I can just adjust the enthalpy to be 0 J/kg at 732.2 K, which is
the low bound temperature for FLiBe

0.0 = 2389.0 * 732.2 + Constant


```rust
pub fn get_flibe_specific_enthalpy(fluid_temp: ThermodynamicTemperature) -> Result<AvailableEnergy, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_temperature_from_enthalpy`

function to obtain flibe salt temperature from specific enthalpy

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).


Note that the enthalpy equation was derived from manual
integration of cp assuming 0 J/kg at 732.2K (the minimum temperature)

h (J/kg) = 2389.0 T(K) + Constant

I can just adjust the enthalpy to be 0 J/kg at 732.2 K, which is
the low bound temperature for FLiBe

0.0 = 2389.0 * 732.2 + Constant



```rust
pub fn get_temperature_from_enthalpy(fluid_enthalpy: AvailableEnergy) -> Result<ThermodynamicTemperature, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `range_check_flibe_salt`

function checks if a fluid temperature falls in a range

If it falls outside this range, it will panic
or throw an error, and the program will not run

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

For FLiBe, the applicable range is 732.2K (melting point) - 1573 K.
I try to make the range as wide as possible because Gnielinski's correlation
requires corrections using wall temperature. These may be outside
the usual bulk temperatures of FLiBe.


thermal conductivity is extrapolated (constant till 1573 K, no data
exists there)
viscosity is all the way up to 732.2 K - 1573 K (Abe's correlation
forms the upper bound limit)




```rust
pub fn range_check_flibe_salt(fluid_temp: ThermodynamicTemperature) -> Result<bool, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `max_temp_flibe`

flibe max temp

```rust
pub fn max_temp_flibe() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_flibe`

flibe min temp

```rust
pub fn min_temp_flibe() -> ThermodynamicTemperature { /* ... */ }
```

## Module `flinak`

FLiNaK
46.5-11.5-42.0 mol% of LiF, NaF and KF respectively,
melting temperature is commonly in literature
454 C, though 462 C is a safer (more conservative) bet

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).




```rust
pub mod flinak { /* ... */ }
```

### Functions

#### Function `get_flinak_density`

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

using recommendation by Romatoski to use Janz and Tompkins correlation

rho (kg/m3) = 2579 - 0.624 T[K]

uncertainty is 2%
applicable from 940 - 1170 K
This is a major factor limiting the temperature range of the
correlations for FLiNaK as a whole


```rust
pub fn get_flinak_density(fluid_temp: ThermodynamicTemperature) -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_flinak_dynamic_viscosity`

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

using recommendation by Romatoski to use Cohen correlation
as he had experimental data points

mu = 0.04 exp(4170/T[K])

```rust
pub fn get_flinak_dynamic_viscosity(fluid_temp: ThermodynamicTemperature) -> Result<DynamicViscosity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_flinak_constant_pressure_specific_heat_capacity`

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

we are using Romatoski's recommended value of 1884 J/(kg K)
uncertainty (error bars) are 10%

```rust
pub fn get_flinak_constant_pressure_specific_heat_capacity(fluid_temp: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_flinak_thermal_conductivity`

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

we are using Smirnov correlation as recommended by Romatoski

```rust
pub fn get_flinak_thermal_conductivity(fluid_temp: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_flinak_specific_enthalpy`

returns flinak specific enthalpy

based on reference temperature at the minimum correlation temperature
of flinak (h = 0 J/kg at that point)



```rust
pub fn get_flinak_specific_enthalpy(fluid_temp: ThermodynamicTemperature) -> Result<AvailableEnergy, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_temperature_from_enthalpy`

returns flinak temperature from specific enthalpy

the specific enthalpy is
based on reference temperature at the minimum correlation temperature
of flinak (h = 0 J/kg at that point)



```rust
pub fn get_temperature_from_enthalpy(fluid_enthalpy: AvailableEnergy) -> Result<ThermodynamicTemperature, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `range_check_flinak_salt`

function checks if a fluid temperature falls in a range

If it falls outside this range, it will panic
or throw an error, and the program will not run

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.

For FLiNaK, the absolute lower bound is 462C, which is a melting point
estimate

The density correlation is in range 940 - 1170 K
about 666.85 C to 896.85 C

cp is across all temperature range 1884 J/(kg K)

the thermal conductivity is from about 773 to 1073 K


viscosity is over from 773-1173 K

From these, it seems that density and thermal conductivity correlations
limit the range of applicability

I'm not going to make effort to increase this range for the time being,
can be patched in future

most conservative range is density (940 - 1073 K)
666.85- 800C



```rust
pub fn range_check_flinak_salt(fluid_temp: ThermodynamicTemperature) -> Result<bool, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `max_temp_flinak`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

flinak max temp

```rust
pub fn max_temp_flinak() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_flinak`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

flinak min temp

```rust
pub fn min_temp_flinak() -> ThermodynamicTemperature { /* ... */ }
```

## Module `custom_liquid_material`

properties for a custom liquid material
not covered in the database
You'll need to define your own functions for this to work
User-defined liquid material property helpers.

These functions let a caller supply their own property correlations (as
`fn(ThermodynamicTemperature) -> ...` closures) for a liquid not otherwise
in the database. Each getter range-checks the requested temperature against
caller-supplied lower/upper bounds, then evaluates the supplied correlation
for density (kg/m^3), dynamic viscosity (Pa·s), constant-pressure specific
heat capacity (J/(kg·K)), or thermal conductivity (W/(m·K)). Specific
enthalpy (J/kg) is obtained by numerically integrating the supplied `cp`
correlation from the lower-bound temperature (taken as the h = 0 J/kg
reference), and temperature (K) is recovered from enthalpy by root-finding
that integral.

```rust
pub mod custom_liquid_material { /* ... */ }
```

### Functions

#### Function `get_custom_fluid_density`

function to obtain custom fluid density
given a temperature
and temperature bounds

```rust
pub fn get_custom_fluid_density(fluid_temp: ThermodynamicTemperature, density_function: fn(ThermodynamicTemperature) -> MassDensity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_fluid_viscosity`

function to obtain custom fluid viscosity
given a temperature

```rust
pub fn get_custom_fluid_viscosity(fluid_temp: ThermodynamicTemperature, viscosity_function: fn(ThermodynamicTemperature) -> DynamicViscosity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<DynamicViscosity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_fluid_constant_pressure_specific_heat_capacity`

function to obtain custom fluid specific heat capacity
given a temperature

```rust
pub fn get_custom_fluid_constant_pressure_specific_heat_capacity(fluid_temp: ThermodynamicTemperature, cp_function: fn(ThermodynamicTemperature) -> SpecificHeatCapacity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_fluid_thermal_conductivity`

function to obtain custom fluid thermal conductivity
given a temperature

```rust
pub fn get_custom_fluid_thermal_conductivity(fluid_temp: ThermodynamicTemperature, conductivity_function: fn(ThermodynamicTemperature) -> ThermalConductivity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_fluid_enthalpy`

function to obtain custom fluid enthalpy
given a temperature

Now, there are two ways of doing this,
firstly, allow the user to specify the enthalpy correlation
doing so saves calculation speed

the second way is to numerically integrate the cp value on behalf of
the user. It is slower on the calculation times but faster with
implementation

I suppose for the user end, I don't assume runtime
speed to be of the essence in this case in comparison to coding
speed and ease of use.

Therefore I will just use numerical integrals, so that the user need not
perform extra coding

```rust
pub fn get_custom_fluid_enthalpy(fluid_temp: ThermodynamicTemperature, cp_function: fn(ThermodynamicTemperature) -> SpecificHeatCapacity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<AvailableEnergy, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_fluid_temperature_from_enthalpy`

function to obtain custom fluid temperature
given a enthalpy

note that this is quite intensive calculation load
wise due to its iterative nature, use sparingly and with caution


```rust
pub fn get_custom_fluid_temperature_from_enthalpy(fluid_enthalpy: AvailableEnergy, cp_function: fn(ThermodynamicTemperature) -> SpecificHeatCapacity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<ThermodynamicTemperature, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `range_check_custom_fluid`

Checks that `fluid_temp` lies between the caller-supplied
`lower_bound_temperature` and `upper_bound_temperature`
(the comparison is done in degrees Celsius).

Unlike the fixed-range salt/oil checks in this database, the valid range
here is whatever the caller specifies via the two bound parameters.

Returns `Ok(true)` when in range; otherwise prints a diagnostic message
and returns `Err(TuasLibError::ThermophysicalPropertyTemperatureRangeError)`
(it does not panic).

```rust
pub fn range_check_custom_fluid(fluid_temp: ThermodynamicTemperature, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<bool, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `solid_database`

database for solids
# Solid material property database

Temperature-dependent thermophysical property correlations for the solid
materials used in the CIET / FHR thermal-hydraulics models: structural
metals, insulation, and heating-element candidates.

Each submodule bundles the correlations for one material — mass density
(kg/m^3), specific heat capacity (J/(kg·K)), thermal conductivity
(W/(m·K)), specific enthalpy (J/kg), surface roughness (m), and the inverse
specific-enthalpy -> temperature map — together with that material's coded
validity temperature range and its literature source. All inputs and
outputs are `uom` dimensioned quantities.

Active submodules: [`ss_304_l`] (SS-304L stainless steel, Zou/Zweibaum
lineage, 250-1000 K), [`ss_304_l_high_temp`] (the same alloy on the Kim
ANL-75-55 lineage, 300-1700 K, for HTGR work), [`copper`],
[`fiberglass`], [`pyrogel_hps`] (silica-aerogel insulation),
[`nuclear_graphite`] (HTR-10 / HTR-PM A3 pebble-matrix graphite and
IG-110 reflector graphite), and [`custom_solid_material`] (user-supplied
correlations). The `fecral` and `generic_heating_element` modules are
experimental scaffolding and are currently commented out (not part of the
build).

```rust
pub mod solid_database { /* ... */ }
```

### Modules

## Module `ss_304_l`

stainless steel 304L

```rust
pub mod ss_304_l { /* ... */ }
```

### Functions

#### Function `steel_304_l_spline_specific_heat_capacity_ciet_zweibaum`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns specific heat capacity of stainless steel 304L in J/(kg·K)

Cubic-spline interpolation of the Zou/Zweibaum tabulated values; valid for
temperatures from 250 K to 1000 K (returns a range error outside this).
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

```rust
pub fn steel_304_l_spline_specific_heat_capacity_ciet_zweibaum(temperature: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_304_l_libreoffice_spline_specific_heat_capacity_ciet_zweibaum`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns specific heat capacity of stainless steel 304L in J/(kg·K)

Evaluates a pre-fitted cubic polynomial in temperature (K); valid for
temperatures from 250 K to 1000 K (returns a range error outside this).
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

Instead of constructing a spline object on the spot and then deleting
it, I used Libreoffice Calc to construct a spline manually instead

```rust
pub fn steel_304_l_libreoffice_spline_specific_heat_capacity_ciet_zweibaum(temperature: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_ss_304_l_ornl_specific_heat_capacity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`


Graves, R. S., Kollie, T. G.,
McElroy, D. L., & Gilchrist, K. E. (1991). The
thermal conductivity of AISI 304L stainless steel.
International journal of thermophysics, 12, 409-415.

data taken from ORNL

It's only good for range of 300K to 700K

```rust
pub fn steel_ss_304_l_ornl_specific_heat_capacity(temperature: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_ss_304_l_ornl_thermal_conductivity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`


Graves, R. S., Kollie, T. G.,
McElroy, D. L., & Gilchrist, K. E. (1991). The
thermal conductivity of AISI 304L stainless steel.
International journal of thermophysics, 12, 409-415.

data taken from ORNL

It's only good for range of 300K to 700K

```rust
pub fn steel_ss_304_l_ornl_thermal_conductivity(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_304_l_spline_thermal_conductivity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns thermal conductivity of stainless steel 304L
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

```rust
pub fn steel_304_l_spline_thermal_conductivity(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_304_l_libreoffice_spline_thermal_conductivity_zweibaum`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns thermal conductivity of stainless steel 304L
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

I used libreoffice to construct the spline rather than use Rust's
inbuilt function, which is more computationally expensive

```rust
pub fn steel_304_l_libreoffice_spline_thermal_conductivity_zweibaum(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_ss_304_l_density`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

density ranges not quite given in original text
Zou, Ling, Rui Hu, and Anne Charpentier. SAM code validation
using the compact integral effects test (CIET) experimental data.
No. ANL/NSE-19/11.
Argonne National Lab.(ANL), Argonne, IL (United States), 2019.

```rust
pub fn steel_ss_304_l_density() -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_surf_roughness`

Value from: Perry's chemical Engineering handbook
8th edition Table 6-1
commercial steel or wrought iron
Perry, R. H., & DW, G. (2007).
Perry’s chemical engineers’ handbook,
8th illustrated ed. New York: McGraw-Hill.

```rust
pub fn steel_surf_roughness() -> Length { /* ... */ }
```

#### Function `steel_304_l_spline_specific_enthalpy_ciet_zweibaum`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns specific enthalpy of stainless steel 304L
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

```rust
pub fn steel_304_l_spline_specific_enthalpy_ciet_zweibaum(temperature: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

#### Function `steel_ss_304_l_ornl_specific_enthalpy_graves`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`


Graves, R. S., Kollie, T. G.,
McElroy, D. L., & Gilchrist, K. E. (1991). The
specific enthalpy of AISI 304L stainless steel.
International journal of thermophysics, 12, 409-415.

data taken from ORNL

It's only good for range of 300K to 700K

However, I analytically integrated it with wolfram alpha

```rust
pub fn steel_ss_304_l_ornl_specific_enthalpy_graves(temperature: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

#### Function `max_temp_ss_304l_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

ss_304l max temp

```rust
pub fn max_temp_ss_304l_zou_zweibaum_spline() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_ss_304l_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

ss_304l min temp

```rust
pub fn min_temp_ss_304l_zou_zweibaum_spline() -> ThermodynamicTemperature { /* ... */ }
```

## Module `ss_304_l_high_temp`

stainless steel 304L, high-temperature correlation set (300 K to 1700 K)

Kim, C. S. (1975). Thermophysical Properties of Stainless Steels.
ANL-75-55, Argonne National Laboratory. Open tier (US Government work,
public domain), catalogued in the KOVAN archive. Extends the envelope
beyond the 1000 K ceiling of the Zou/Zweibaum correlations in [`ss_304_l`],
for HTGR / HTR-10 component modelling. Does not replace [`ss_304_l`].
# Type 304L stainless steel, high-temperature correlation set (Kim, ANL-75-55)

Thermophysical property correlations for **AISI Type 304L austenitic
stainless steel** valid from **300 K to 1700 K** (26.85 degC to
1426.85 degC), dispatched from [`SolidMaterial::SteelSS304LHighTemp`].

## Why this exists alongside [`super::ss_304_l`]

This module does **not** replace [`super::ss_304_l`], and the two must not
be conflated:

| | [`super::ss_304_l`] ([`SolidMaterial::SteelSS304L`]) | this module ([`SolidMaterial::SteelSS304LHighTemp`]) |
|---|---|---|
| Lineage | Zou/Zweibaum splines (ANL/NSE-19/11), Graves et al. (ORNL) | Kim, ANL-75-55 (Argonne) |
| Range | 250 K - 1000 K | 300 K - 1700 K |
| Density | constant 8030 kg/m^3 | temperature-dependent, 7894 -> 7199 kg/m^3 |
| Validated against | CIET natural-circulation data, to ~6% | nothing measured in-workspace; see "Verification status" |

The Zou/Zweibaum set is the one the CIET regression tests are validated
against and it **must not be changed**. But its 1000 K (726.85 degC) ceiling
is too low for HTGR work: the HTR-10's published phase-1 core outlet is
700 degC (973.15 K), leaving only ~27 K of headroom before
[`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] is returned,
and the 900 degC phase-2 condition is unreachable. This module supplies the
wider envelope for those components; existing CIET models keep using
[`SolidMaterial::SteelSS304L`] and are unaffected.

## Source

Kim, C. S. (1975). *Thermophysical Properties of Stainless Steels*.
ANL-75-55, Argonne National Laboratory.
<https://www.osti.gov/servlets/purl/4152287>

US Government work, distribution unlimited, public domain. Catalogued in
this workspace's KOVAN archive (Open tier) at
`crates/kovan-literature/open/reports/kim1975-thermophysical-properties-stainless-steels.pdf`.

Only the **solid-region** Type 304L equations are implemented here:
Eq. (5) specific heat, Eq. (16) density, Eq. (28) thermal conductivity, and
Eq. (1) enthalpy (re-referenced to 273.15 K to match this crate's
convention). The liquid-region equations are recorded in
[`melting_and_liquid_region_notes`] for reference but are not evaluated by
any function here.

## Validity: measured versus extrapolated

**The whole 300-1700 K range is NOT measured data.** Kim fitted his
equations by least squares to experimental data and then *extrapolated*
them into the melting range (1670-1730 K), which he states explicitly:

- **Enthalpy / specific heat** — experimental data to **~1620 K** for
  Type 304L, "least-square techniques, and extrapolated to the melting
  range (1670-1730 K)" (report p. 2).
- **Thermal conductivity** — experimental data to **~1600 K**; "straight
  lines were drawn through these sets of data points and extended to the
  melting range" (report p. 15).
- **Density** — experimental data over **300-1600 K** (report p. 12,
  Table 6).

So: **300 K to 1600 K is backed by measured data; 1600 K to 1700 K is
Kim's own extrapolation.** Do not describe results above 1600 K as
measurement-backed. The upper bound is set at 1700 K — Kim's melting
temperature `T_m` — because the solid-region equations are meaningless
above it, not because the data extends that far.

## Verification status

These correlations are verified only against **Kim's own published tables**
(Tables 2, 7 and 10 of ANL-75-55) — that is, the implementation is checked
to reproduce the source, which is a *verification* result, not a
*validation* one. **No comparison against independent 304L measurements,
and no maintainer review, has been done.** Per `RESPONSIBLE_USE.md` this
is untrusted AI-assisted draft material until a human reviews it. See the
unit tests at the bottom of this file for the measured agreement.

```rust
pub mod ss_304_l_high_temp { /* ... */ }
```

### Functions

#### Function `max_temp_ss_304l_high_temp_kim`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the maximum temperature of the Kim ANL-75-55 Type 304L
solid-region correlations: **1700 K** (1426.85 degC).

This is Kim's melting temperature `T_m`. Note that only up to
[`MAX_MEASURED_DATA_TEMPERATURE_KELVIN`] (1600 K) is backed by experimental
data; 1600-1700 K is Kim's least-squares extrapolation into the melting
range (1670-1730 K). See the module documentation.

```rust
pub fn max_temp_ss_304l_high_temp_kim() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_ss_304l_high_temp_kim`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the minimum temperature of the Kim ANL-75-55 Type 304L
solid-region correlations: **300 K** (26.85 degC).

Below this, use [`SolidMaterial::SteelSS304L`], whose Zou/Zweibaum splines
reach down to 250 K.

```rust
pub fn min_temp_ss_304l_high_temp_kim() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `steel_304_l_high_temp_density_kim`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the **mass density** of Type 304L stainless steel, in kg/m^3.

Unlike [`super::ss_304_l::steel_ss_304_l_density`] (a constant
8030 kg/m^3), this is temperature-dependent, falling from about
7894 kg/m^3 at 300 K to about 7199 kg/m^3 at 1700 K.

# Correlation

Kim ANL-75-55, **Eq. (16)**, solid region:

```text
rho = 7.9841 - 2.6506e-4 * T - 1.1580e-7 * T^2      [g/cm^3], T in K
```

converted here to kg/m^3 by a factor of 1000.

# Arguments

* `temperature` — the steel temperature. Must lie between 300 K
  (26.85 degC) and 1700 K (1426.85 degC).

# Errors

Returns [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] if
`temperature` falls outside 300-1700 K.

# Validity

Fitted by least squares to experimental density data over 300-1600 K
(Kim's Table 6, experimental column) and extrapolated to 1700 K.

```rust
pub fn steel_304_l_high_temp_density_kim(temperature: ThermodynamicTemperature) -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_304_l_high_temp_specific_heat_capacity_kim`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the **constant-pressure specific heat capacity** of Type 304L
stainless steel, in J/(kg K).

Rises from about 510 J/(kg K) at 300 K to about 699 J/(kg K) at 1700 K.

# Correlation

Kim ANL-75-55, **Eq. (5)**, solid region:

```text
c_p = 0.1122 + 3.222e-5 * T        [cal/(g K)], T in K
```

converted here to J/(kg K) by a factor of 4184 (the thermochemical
calorie is exactly 4.184 J).

# Arguments

* `temperature` — the steel temperature. Must lie between 300 K
  (26.85 degC) and 1700 K (1426.85 degC).

# Errors

Returns [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] if
`temperature` falls outside 300-1700 K.

# Validity

Derived by Kim from enthalpy data measured to ~1620 K, then extrapolated
into the melting range. This is the **solid-region** `c_p` only; the liquid
value is a constant 0.190 cal/(g K) (795 J/(kg K)) and is not evaluated
here — see [`melting_and_liquid_region_notes`].

```rust
pub fn steel_304_l_high_temp_specific_heat_capacity_kim(temperature: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_304_l_high_temp_thermal_conductivity_kim`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the **thermal conductivity** of Type 304L stainless steel,
in W/(m K).

Rises linearly from about 12.97 W/(m K) at 300 K to about 35.62 W/(m K)
at 1700 K.

# Correlation

Kim ANL-75-55, **Eq. (28)**, solid region:

```text
k = 8.116e-2 + 1.618e-4 * T        [W/(cm K)], T in K
```

converted here to W/(m K) by a factor of 100.

# Arguments

* `temperature` — the steel temperature. Must lie between 300 K
  (26.85 degC) and 1700 K (1426.85 degC).

# Errors

Returns [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] if
`temperature` falls outside 300-1700 K.

# Validity

Kim drew straight lines through experimental conductivity data available
to ~1600 K and extended them to the melting range, so 1600-1700 K is
extrapolation.

```rust
pub fn steel_304_l_high_temp_thermal_conductivity_kim(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `steel_304_l_high_temp_specific_enthalpy_kim`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the **specific enthalpy** of Type 304L stainless steel, in J/kg,
taking h = 0 J/kg at 273.15 K (0 degrees Celsius).

# Correlation

The analytic integral of the Eq. (5) specific heat, which is Kim's
**Eq. (1)** re-referenced from his 298.15 K datum to this crate's 273.15 K
datum:

```text
h(T) = 0.1122 * (T - 273.15) + 1.611e-5 * (T^2 - 273.15^2)   [cal/g]
```

where 1.611e-5 is exactly half of the Eq. (5) slope 3.222e-5, as it must be
for the integral of a linear `c_p`. Converted here to J/kg by a factor of
4184. Kim's own published form,
`H_T - H_298.15 = -34.885 + 0.1122 T + 1.611e-5 T^2`, is the same curve
with a different additive constant.

# Arguments

* `temperature` — the steel temperature, nominally within 300-1700 K.

# Range behaviour

This function deliberately performs **no range check** and returns a bare
[`AvailableEnergy`] rather than a `Result`, matching the signature the
crate's [`specific_enthalpy`] dispatch requires of every solid. Because the
correlation is a plain quadratic it extrapolates smoothly and monotonically
outside 300-1700 K rather than failing — which is intentional here, since
solid-array energy solvers call this on intermediate iterates that may
briefly stray outside the range. Bounds are still enforced on the `c_p`,
density and conductivity paths. Values far outside 300-1700 K are
arithmetically valid but physically meaningless.

```rust
pub fn steel_304_l_high_temp_specific_enthalpy_kim(temperature: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

#### Function `steel_304_l_high_temp_temp_from_specific_enthalpy_kim`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the **temperature** of Type 304L stainless steel, in kelvin, given
its specific enthalpy in J/kg (referenced to h = 0 at 273.15 K).

This is the exact analytic inverse of
[`steel_304_l_high_temp_specific_enthalpy_kim`]. Because that enthalpy is a
quadratic in temperature, the inverse is the positive root of the quadratic
formula — no spline, no Brent-Dekker iteration, and no failure mode. That
makes it both cheaper and more robust than the root-finding inverses used
for the tabulated materials in this database.

# Derivation

With `a = 0.1122`, `b/2 = 1.611e-5` and `T_ref = 273.15`, solving
`h = a (T - T_ref) + (b/2)(T^2 - T_ref^2)` for `T` gives

```text
T = [ -a + sqrt( a^2 + 4 (b/2) ( a T_ref + (b/2) T_ref^2 + h ) ) ] / (2 (b/2))
```

taking the positive root, which is the physical branch for `T > 0`.

# Arguments

* `h_steel` — specific enthalpy in J/kg, with h = 0 at 273.15 K.

# Range behaviour

Performs no range check, matching the crate's solid enthalpy-inverse
dispatch signature (which returns a bare temperature, not a `Result`). The
discriminant stays positive for every enthalpy above roughly -0.9 MJ/kg,
far below any physically reachable value, so this does not produce NaN in
practice.

```rust
pub fn steel_304_l_high_temp_temp_from_specific_enthalpy_kim(h_steel: AvailableEnergy) -> ThermodynamicTemperature { /* ... */ }
```

#### Function `melting_and_liquid_region_notes`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Melting and liquid-region data from Kim ANL-75-55, recorded for reference.

**Nothing in this module evaluates these** — only the solid region is
implemented, and [`SolidMaterial::SteelSS304LHighTemp`] is bounded at
1700 K precisely so that the liquid region is never entered. This function
exists so the numbers are documented where a reader will meet them rather
than being lost in a commit message.

For Type 304L:

- Melting range **1670-1730 K**, with `T_m` = **1700 K**.
- Heat of fusion **64.0 cal/g** = **267.8 kJ/kg**.
- Liquid specific heat, constant, **0.190 cal/(g K)** = **795 J/(kg K)**
  (Kim's Eq. 9 enthalpy slope).
- Liquid density, Eq. (17):
  `rho = 7.5512 - 1.1167e-4 T - 1.5063e-7 T^2` [g/cm^3].
- Liquid thermal conductivity, Eq. (29):
  `k = 1.229e-1 + 3.248e-5 T` [W/(cm K)].

# OCR correction on Eq. (29) — deliberate, and verified

The scanned report renders the Eq. (29) slope as **`3.248e-3`**. That is an
OCR error in the superscript and **`3.248e-5` is correct**. Two independent
checks confirm it:

1. **Kim's own stated rule.** He writes that "at the melting points, the
   thermal conductivities of solid steels were reduced by half to give the
   values in the liquid state". Solid Eq. (28) at `T_m` = 1700 K gives
   0.356220 W/(cm K), half of which is **0.178110**. Eq. (29) with
   3.248e-5 gives **0.178116** — agreement to six significant figures.
   With 3.248e-3 it gives 5.6445, off by a factor of ~32.
2. **The parallel Type 316L equation.** Eq. (31) is
   `k = 1.241e-1 + 3.279e-5 T`, whose exponent survived OCR intact, and
   which satisfies the same halving rule at `T_m`.

Kim's own Table 10 also lists the 304L liquid conductivity at 1700 K as
0.1781 W/(cm K), matching the corrected form.

```rust
pub fn melting_and_liquid_region_notes() { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_MEASURED_DATA_TEMPERATURE_KELVIN`

Highest temperature backed by experimental data rather than by Kim's
extrapolation, in kelvin — 1600 K (1326.85 degC).

Taken as the most conservative of the three property data ranges: enthalpy
to ~1620 K, thermal conductivity to ~1600 K, density to ~1600 K. Above this
the correlations still evaluate (they are coded valid to 1700 K) but they
are the author's extrapolation into the melting range.

```rust
pub const MAX_MEASURED_DATA_TEMPERATURE_KELVIN: f64 = 1600.0;
```

## Module `copper`

copper

```rust
pub mod copper { /* ... */ }
```

### Functions

#### Function `copper_density`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

density ranges not quite given in original text
Zou, Ling, Rui Hu, and Anne Charpentier. SAM code validation
using the compact integral effects test (CIET) experimental data.
No. ANL/NSE-19/11.
Argonne National Lab.(ANL), Argonne, IL (United States), 2019.

```rust
pub fn copper_density() -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `copper_surf_roughness`

Arenales, M. R. M., Kumar, S.,
Kuo, L. S., & Chen, P. H. (2020).
Surface roughness variation effects on copper tubes in
pool boiling of water. International Journal of
Heat and Mass Transfer, 151, 119399.

```rust
pub fn copper_surf_roughness() -> Length { /* ... */ }
```

#### Function `copper_specific_heat_capacity_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns specific heat capacity of copper in J/(kg·K)

Cubic-spline interpolation of the Zou/Zweibaum tabulated values; valid for
temperatures from 200 K to 1000 K (returns a range error outside this).
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

```rust
pub fn copper_specific_heat_capacity_zou_zweibaum_spline(temperature: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `copper_thermal_conductivity_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns thermal conductivity of copper
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

```rust
pub fn copper_thermal_conductivity_zou_zweibaum_spline(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `copper_specific_enthalpy`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns specific enthalpy of copper
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

```rust
pub fn copper_specific_enthalpy(temperature: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

#### Function `max_temp_copper_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

copper max temp

```rust
pub fn max_temp_copper_zou_zweibaum_spline() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_copper_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

copper min temp

```rust
pub fn min_temp_copper_zou_zweibaum_spline() -> ThermodynamicTemperature { /* ... */ }
```

## Module `fiberglass`

fiberglass

```rust
pub mod fiberglass { /* ... */ }
```

### Functions

#### Function `fiberglass_density`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

density ranges not quite given in original text
Zou, Ling, Rui Hu, and Anne Charpentier. SAM code validation
using the compact integral effects test (CIET) experimental data.
No. ANL/NSE-19/11.
Argonne National Lab.(ANL), Argonne, IL (United States), 2019.

```rust
pub fn fiberglass_density() -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `fiberglass_surf_roughness`

Value from: Perry's chemical Engineering handbook
8th edition Table 6-1
generic value for drawn tubing
Perry, R. H., & DW, G. (2007).
Perry’s chemical engineers’ handbook,
8th illustrated ed. New York: McGraw-Hill.

```rust
pub fn fiberglass_surf_roughness() -> Length { /* ... */ }
```

#### Function `fiberglass_specific_heat_capacity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns specific heat capacity of fiberglass in J/(kg·K)

This is a temperature-independent constant of 844 J/(kg·K); the temperature
argument is ignored (no range check is applied).
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

```rust
pub fn fiberglass_specific_heat_capacity(_temperature: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `fiberglass_thermal_conductivity_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns thermal conductivity of fiberglass
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

```rust
pub fn fiberglass_thermal_conductivity_zou_zweibaum_spline(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `max_temp_fiberglass_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

fiberglass max temp

```rust
pub fn max_temp_fiberglass_zou_zweibaum_spline() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_fiberglass_zou_zweibaum_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

fiberglass min temp

```rust
pub fn min_temp_fiberglass_zou_zweibaum_spline() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `fiberglass_specific_enthalpy`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns specific enthalpy of fiberglass
cited from:
Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

specific enthalpy at 273.15 K = 0

cp = 844 J/(kg K)
hence
h = 844 * T - 844 * (273.15)

```rust
pub fn fiberglass_specific_enthalpy(temperature: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

## Module `custom_solid_material`

custom material for solid
# Custom (user-supplied) solid material properties

Helpers that let a caller plug in their own temperature-dependent property
correlations for a solid that is not in the built-in database. Each helper
takes the caller's correlation as a function pointer plus an explicit valid
temperature range (upper and lower bound as `uom`
`ThermodynamicTemperature`), range-checks the query temperature, and then
evaluates the correlation.

Provided: mass density (kg/m^3), constant-pressure specific heat capacity
(J/(kg·K)), thermal conductivity (W/(m·K)), specific enthalpy (J/kg,
obtained by numerically integrating the supplied cp from the lower bound),
and the inverse specific-enthalpy -> temperature map (solved by bisection).
All inputs and outputs are `uom` dimensioned quantities.

```rust
pub mod custom_solid_material { /* ... */ }
```

### Functions

#### Function `get_custom_solid_density`

function to obtain custom solid density
given a temperature
and temperature bounds

```rust
pub fn get_custom_solid_density(solid_temp: ThermodynamicTemperature, density_function: fn(ThermodynamicTemperature) -> MassDensity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_solid_constant_pressure_specific_heat_capacity`

function to obtain custom solid specific heat capacity
given a temperature

```rust
pub fn get_custom_solid_constant_pressure_specific_heat_capacity(solid_temp: ThermodynamicTemperature, cp_function: fn(ThermodynamicTemperature) -> SpecificHeatCapacity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_solid_thermal_conductivity`

function to obtain custom solid thermal conductivity
given a temperature

```rust
pub fn get_custom_solid_thermal_conductivity(solid_temp: ThermodynamicTemperature, conductivity_function: fn(ThermodynamicTemperature) -> ThermalConductivity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_solid_enthalpy`

function to obtain custom solid enthalpy
given a temperature

Now, there are two ways of doing this,
firstly, allow the user to specify the enthalpy correlation
doing so saves calculation speed

the second way is to numerically integrate the cp value on behalf of
the user. It is slower on the calculation times but faster with
implementation

I suppose for the user end, I don't assume runtime
speed to be of the essence in this case in comparison to coding
speed and ease of use.

Therefore I will just use numerical integrals, so that the user need not
perform extra coding

```rust
pub fn get_custom_solid_enthalpy(solid_temp: ThermodynamicTemperature, cp_function: fn(ThermodynamicTemperature) -> SpecificHeatCapacity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<AvailableEnergy, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_custom_solid_temperature_from_enthalpy`

function to obtain custom solid temperature
given a enthalpy

note that this is quite intensive calculation load
wise due to its iterative nature, use sparingly and with caution


```rust
pub fn get_custom_solid_temperature_from_enthalpy(solid_enthalpy: AvailableEnergy, cp_function: fn(ThermodynamicTemperature) -> SpecificHeatCapacity, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<ThermodynamicTemperature, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `range_check_custom_solid`

function checks if a solid temperature falls within the caller-supplied
range, i.e. between `lower_bound_temperature` and `upper_bound_temperature`
(both `uom` `ThermodynamicTemperature`; compared in degrees Celsius).

Returns `Ok(true)` if in range; if it falls outside, it prints a diagnostic
message and returns `Err(TuasLibError::ThermophysicalPropertyTemperatureRangeError)`.


```rust
pub fn range_check_custom_solid(solid_temp: ThermodynamicTemperature, upper_bound_temperature: ThermodynamicTemperature, lower_bound_temperature: ThermodynamicTemperature) -> anyhow::Result<bool, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `nuclear_graphite`

nuclear graphite (HTR-10 / HTR-PM A3 pebble matrix, and IG-110)

Correlations transcribed from the openly licensed Virtual Test Bed decks
vendored under `reference-data/virtual_test_bed/` (CC-BY-4.0, Open tier),
with cp cited to Butland & Maddison, J. Nucl. Mater. 49 (1973/74) 45-56.
# Nuclear graphite (HTR-10 / HTR-PM pebble matrix A3, and IG-110)

Thermophysical property correlations for two grades of nuclear graphite:

- **Matrix graphite, A3 grade** — the fuel-pebble matrix graphite of the
  HTR-10 / HTR-PM pebble-bed reactors
  ([`SolidMaterial::NuclearGraphiteMatrixA3`]).
- **IG-110** — the fine-grained isotropic reflector-grade graphite used in
  the HTTR and HTR-10 reflector structures
  ([`SolidMaterial::NuclearGraphiteIG110`]).

All correlations are transcribed from the openly licensed **Virtual Test
Bed (VTB)** input decks vendored in this workspace under
`reference-data/virtual_test_bed/` (CC-BY-4.0, Open tier), plus two Open
tier literature values for density. The exact deck file and line numbers
are cited on each function.

Both grades share one specific-heat-capacity table (Butland & Maddison):
nuclear graphite cp is treated as grade-insensitive because all grades are
polycrystalline graphite and cp is dominated by the phonon spectrum of
graphite itself, not by grade-specific porosity or grain structure.
Thermal conductivity, by contrast, is strongly grade- and
irradiation-dependent, so each grade has its own correlation, each with an
optional fast-neutron-fluence damage factor.

The enum arms of [`SolidMaterial`] dispatch to the **unirradiated /
zero-fluence** forms. The fluence-dependent free functions exist for
consumers (e.g. decay-heat / irradiated-core studies) that track fast
fluence themselves and want the degraded conductivity.

**None of these correlations has been checked against HTR-10 measurements
by the maintainer** — they are transcriptions of the VTB decks and cited
literature values, verified only against hand evaluations of the same
formulas (see the unit tests at the bottom of this file).

```rust
pub mod nuclear_graphite { /* ... */ }
```

### Functions

#### Function `nuclear_graphite_matrix_a3_density`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the mass density of A3-grade pebble matrix graphite,
1730 kg/m^3 (1.73 g/cm^3), treated as temperature-independent.

Source: IAEA-TECDOC-1382 ("Evaluation of high temperature gas cooled
reactor performance: benchmark analysis related to initial testing of the
HTTR and HTR-10"), Chapter 4 — HTR-10 fuel-element matrix graphite density
1.73 g/cm^3. Open tier (public IAEA benchmark document).

Note: the VTB HTR-PM pebble deck
(`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`,
line 193) embeds a matrix density of 1740 kg/m^3 inside its
conductivity Maxwell factor, and the VTB GPBR200 decks likewise use
1740 kg/m^3. The 0.6% difference from the 1730 kg/m^3 returned here is
far below the uncertainty of any downstream thermal calculation.

```rust
pub fn nuclear_graphite_matrix_a3_density() -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `nuclear_graphite_ig_110_density`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the mass density of IG-110 reflector-grade graphite,
1770 kg/m^3, treated as temperature-independent.

Source: the VTB HTTR deck
(`reference-data/virtual_test_bed/htgr/httr/steady_state_and_null_transient/fuel_elem_steady.i`,
line 340) which cites table 1.27 of NEA/NSC/DOC(2006)1 ("Evaluation of
High Temperature Gas Cooled Reactor Performance", OECD/NEA). Open tier
(CC-BY-4.0 VTB deck citing a public OECD/NEA benchmark document).

```rust
pub fn nuclear_graphite_ig_110_density() -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `nuclear_graphite_surf_roughness`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns a nominal surface roughness for machined nuclear graphite,
1.95 micrometres.

This is the same figure the in-workspace gFHR custom-graphite tutorial
uses (`src/lib/pre_built_components/insulated_pipes_and_fluid_components/tutorials/tutorial_6.rs`,
line 139, `gfhr_pipe_with_custom_graphite_material`), adopted here as the
in-workspace precedent. It is a **nominal machined-graphite figure, not a
measured HTR-10 value** — treat it as an order-of-magnitude estimate for
friction-factor purposes.

```rust
pub fn nuclear_graphite_surf_roughness() -> Length { /* ... */ }
```

#### Function `min_temp_nuclear_graphite`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Minimum temperature, 300 K, of the coded nuclear-graphite correlations
(both grades). This is the lowest node of the Butland & Maddison cp table
(see [`nuclear_graphite_specific_heat_capacity_butland_maddison_spline`]).

```rust
pub fn min_temp_nuclear_graphite() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `max_temp_nuclear_graphite`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Maximum temperature, 2000 K, of the coded nuclear-graphite correlations
(both grades). This is the highest node of the Butland & Maddison cp table
(see [`nuclear_graphite_specific_heat_capacity_butland_maddison_spline`]).

```rust
pub fn max_temp_nuclear_graphite() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `nuclear_graphite_specific_heat_capacity_butland_maddison_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the specific heat capacity, in J/(kg K), of nuclear graphite
(both A3 matrix and IG-110 grades) at the given temperature.

Cubic-spline interpolation of the 18-node G-348 graphite cp table (300 K
to 2000 K in 100 K steps) from the VTB HTTF SAM ring model deck
(`reference-data/virtual_test_bed/htgr/httf/sam_ring_model/HTTF-SS.i`,
lines 368-372, `cpgraphite` block), which cites **Butland, A. T. D. &
Maddison, R. J., "The specific heat of graphite: an evaluation of
measurements", J. Nucl. Mater. 49 (1973/74) 45-56**. Open tier
(CC-BY-4.0 VTB deck citing open literature).

**Grade-insensitivity assumption:** this one cp table serves both the A3
matrix and IG-110 enum variants. All nuclear graphite grades are
polycrystalline graphite, and cp is dominated by the phonon spectrum of
graphite itself rather than by grade-specific porosity/grain structure,
so per-grade cp differences are small compared to the correlation's own
uncertainty.

Valid range: 300 K to 2000 K; outside it, returns
`TuasLibError::ThermophysicalPropertyTemperatureRangeError`. (The
out-of-range debug print names the `NuclearGraphiteMatrixA3` variant
because the table is shared between both graphite variants.)

```rust
pub fn nuclear_graphite_specific_heat_capacity_butland_maddison_spline(temperature: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `nuclear_graphite_fluence_damage_factor`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the fast-neutron-fluence conductivity damage factor
(dimensionless) for nuclear graphite:

factor = 1 - 0.336*(1 - exp(-1.005*gam)) - 3.50e-2*gam

where `gam` is the fast-neutron fluence. **Unit interpretation:** `gam`
is interpreted as the fluence in units of 10^25 n/m^2 (E > 0.1 MeV).
This is an *interpretation, not a deck-stated fact* — the VTB HTR-PM deck
declares no unit for `gam`. It is supported by (a) the VTB GPBR200 decks
using `fast_neutron_fluence = 10e25` n/m^2 with graphite grade A3_3_1800,
and (b) the deck deriving `gam` from burnup with magnitudes of order 10,
consistent with fluences of order 10^26 n/m^2.

Source: the fluence factor of the `gmatrix_k` function in the VTB HTR-PM
pebble model
(`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`,
lines 193-198). CC-BY-4.0, Open tier. The deck names no upstream
literature source for this correlation.

Behaviour: exactly 1 at `gam = 0`; monotonically decreasing in `gam`.
The saturating exponential term levels off near 0.336 by `gam ~ 5`, after
which the linear `3.5e-2*gam` term dominates and drives the factor
through zero at `gam ~ 19` — which is unphysical (conductivity cannot be
negative). This function therefore returns
`TuasLibError::ThermophysicalPropertyTemperatureRangeError` for `gam`
outside [0, 15]: at `gam = 15` the factor is still physically positive
(measured 0.1390, see the unit test), leaving margin before the
unphysical zero crossing at `gam ~ 19.0`.

```rust
pub fn nuclear_graphite_fluence_damage_factor(fluence: Ratio) -> Result<Ratio, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the thermal conductivity, in W/(m K), of A3-grade pebble matrix
graphite at the given temperature and fast-neutron fluence.

Implements the `gmatrix_k` function of the VTB HTR-PM pebble model
(`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`,
lines 193-198; CC-BY-4.0, Open tier), with temperature `t` in kelvin:

k(t, gam) = 47.4 * (1 - 9.7556e-4*(t - 373.15)*exp(-6.036e-4*(t - 273.15)))
          * (1740/(2.2*(1700 - 1740) + 1740))
          * (1 - 0.336*(1 - exp(-1.005*gam)) - 3.50e-2*gam)

The three factors are: a temperature factor (equal to 1 at 373.15 K); a
constant density/Maxwell porosity factor 1740/1652 ~= 1.0533; and the
fluence damage factor (see
[`nuclear_graphite_fluence_damage_factor`], including the unit
interpretation of `gam` as fluence in 10^25 n/m^2, E > 0.1 MeV — an
interpretation, since the deck declares no unit). The correlation is
**transcribed from the VTB HTR-PM pebble model, which names no upstream
source** for it.

Valid ranges enforced: temperature 300 K to 2000 K (the deck states no
range; this range matches the sibling graphite cp table so all
nuclear-graphite properties share one coded validity window), and fluence
`gam` in [0, 15] (beyond which the damage factor heads to an unphysical
zero crossing at `gam ~ 19`). Out-of-range inputs return
`TuasLibError::ThermophysicalPropertyTemperatureRangeError`.

```rust
pub fn nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent(temperature: ThermodynamicTemperature, fluence: Ratio) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the thermal conductivity, in W/(m K), of **unirradiated**
(zero-fluence) A3-grade pebble matrix graphite at the given temperature.

This is [`nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent`]
evaluated at `gam = 0`, where the fluence damage factor is exactly 1 —
see that function for the correlation, its VTB HTR-PM source
(`pebble_triso.i` lines 193-198, CC-BY-4.0, Open tier; no upstream
source named in the deck), and the enforced 300 K to 2000 K range. The
[`SolidMaterial::NuclearGraphiteMatrixA3`] enum arm dispatches here.

```rust
pub fn nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `nuclear_graphite_ig_110_thermal_conductivity_unirradiated`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the thermal conductivity, in W/(m K), of **unirradiated** IG-110
reflector-grade graphite at the given temperature.

Implements the `IG110_k` quadratic of the VTB HTTR deck
(`reference-data/virtual_test_bed/htgr/httr/steady_state_and_null_transient/fuel_elem_steady.i`,
lines 301-306; CC-BY-4.0, Open tier), with temperature `t` in kelvin:

k(t) = 66.32 - 4.994e-2*t + 1.712e-5*t^2

The deck **names no upstream literature source** for this quadratic, and
**states no validity range**; this implementation enforces 300 K to
2000 K, consistent with the sibling nuclear-graphite cp table, so all
nuclear-graphite properties share one coded validity window.
Out-of-range temperatures return
`TuasLibError::ThermophysicalPropertyTemperatureRangeError`. The
[`SolidMaterial::NuclearGraphiteIG110`] enum arm dispatches here.

```rust
pub fn nuclear_graphite_ig_110_thermal_conductivity_unirradiated(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `nuclear_graphite_ig_110_thermal_conductivity_fluence_dependent`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the thermal conductivity, in W/(m K), of IG-110 reflector-grade
graphite at the given temperature and fast-neutron fluence, by applying
the A3 matrix-graphite fluence damage factor to the unirradiated IG-110
quadratic.

**Assumption of this implementation:** the saturating damage factor
(1 - 0.336*(1 - exp(-1.005*gam)) - 3.50e-2*gam) is taken from the VTB
HTR-PM pebble model, where it is applied to matrix/buffer/PyC materials —
**the VTB does not apply it to IG-110**. It is applied here because the
reflector dose-degradation path needs a fluence-degraded IG-110
conductivity and no better open correlation is vendored in this
workspace. Treat results at `gam > 0` as an engineering estimate only.

`gam` is interpreted as fluence in units of 10^25 n/m^2 (E > 0.1 MeV);
see [`nuclear_graphite_fluence_damage_factor`] for that interpretation
and for the enforced fluence range [0, 15]. Temperature range enforced:
300 K to 2000 K (via
[`nuclear_graphite_ig_110_thermal_conductivity_unirradiated`]).

```rust
pub fn nuclear_graphite_ig_110_thermal_conductivity_fluence_dependent(temperature: ThermodynamicTemperature, fluence: Ratio) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `nuclear_graphite_specific_enthalpy`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Returns the specific enthalpy, in J/kg, of nuclear graphite (both A3
matrix and IG-110 grades, which share one cp table) at the given
temperature, with h = 0 at 273.15 K.

Integrates the Butland & Maddison cp cubic spline (see
[`nuclear_graphite_specific_heat_capacity_butland_maddison_spline`];
VTB HTTF deck `HTTF-SS.i` lines 368-372, CC-BY-4.0, Open tier) from
273.15 K to the given temperature, per this database's house convention
(compare `copper_specific_enthalpy` /
`steel_304_l_spline_specific_enthalpy_ciet_zweibaum`).

**Below-table extrapolation note:** the cp table starts at 300 K but the
integration reference is 273.15 K, so the integral's first 26.85 K uses
the cubic spline's natural extrapolation below its lowest node. This only
shifts the (arbitrary) enthalpy datum by a constant; enthalpy
*differences* between any two temperatures at or above 300 K are
unaffected. User-facing dispatch keeps `min_temperature()` at 300 K, so
in-range calls never rely on extrapolated cp except through this shared
datum offset.

Like its siblings this function performs no range check of its own
(range enforcement happens in the cp/conductivity accessors and via
`min_temperature()`/`max_temperature()`).

```rust
pub fn nuclear_graphite_specific_enthalpy(temperature: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

## Module `pyrogel_hps`

pyrogel hps

This is an aerogel with silica fibres.

Most information comes from:

Kovács, Z., Csík, A., & Lakatos, Á. (2023).
Thermal stability investigations of different
aerogel insulation materials at elevated temperature.
Thermal Science and Engineering Progress, 42, 101906.

```rust
pub mod pyrogel_hps { /* ... */ }
```

### Functions

#### Function `pyrogel_hps_density`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Based on:
https://www.distributioninternational.com/ASSETS/DOCUMENTS/ITEMS/EN/PYBT10HA_SS.pdf


0.20 g/cc density (g/cc is grams per cubic centimeter)


```rust
pub fn pyrogel_hps_density() -> Result<MassDensity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `pyrogel_hps_surf_roughness`

For Pyrogel HPS specficially, I don't see any surface roughness
data in literature.


But since Pyrogel HPS is a silica aerogel, I'll use the silica
aerogel surface roughness as a ballpark estimate

Based on:
Mahadik, D. B., Venkateswara Rao, A., Parale, V. G., Kavale, M. S.,
Wagh, P. B., Ingale, S. V., & Gupta, S. C. (2011). Effect of surface
composition and roughness on the apparent surface free energy of
silica aerogel materials. Applied Physics Letters, 99(10).

Paper mentioned 1150–1450 nm

I'll just use 1500 nm as an estimate



```rust
pub fn pyrogel_hps_surf_roughness() -> Length { /* ... */ }
```

#### Function `pryogel_hps_specific_heat_capacity_rough_estimate`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Most information comes from:

Kovács, Z., Csík, A., & Lakatos, Á. (2023).
Thermal stability investigations of different
aerogel insulation materials at elevated temperature.
Thermal Science and Engineering Progress, 42, 101906.

work in progress though. still need to decipher the paper

Cassel, R. B. (2001). How Tzero™ Technology Improves DSC
Performance Part III: The Measurement of Specific Heat Capacity.
TA Instruments: New Castle, DE, USA.

for DSC:

dQ/dt (watts) = cp * beta * sample_mass
dQ/dt * 1/sample_mass (watts/gram) = cp * beta

beta is heating rate (kelvin or degC per minute)

Now, based on the dsc measurements, cp of around 1500 - 2200 J/(kg K)
can be expected after crystallisation. This is just a ballpark estimate

I'll just use 1700 J/(kg K) as a placeholder because thermal inertia
may not be superbly important to model now
But, 1698 was the value of cp both at 326 C, and 50C - 190C

So it seems to be a reasonable estimate for temperature beyond.

```rust
pub fn pryogel_hps_specific_heat_capacity_rough_estimate(temperature: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
```

#### Function `pyrogel_hps_specific_enthalpy`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

based on best estimate cp data, where cp = 1700 J/(kg K),
I programmed this such that
h = 1700 * T - 1700 * (273.15)

```rust
pub fn pyrogel_hps_specific_enthalpy(temperature: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
```

#### Function `pryogel_hps_specific_heat_capacity_spline_low_temp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Most information comes from:

Kovács, Z., Csík, A., & Lakatos, Á. (2023).
Thermal stability investigations of different
aerogel insulation materials at elevated temperature.
Thermal Science and Engineering Progress, 42, 101906.

work in progress though. still need to decipher the paper

Cassel, R. B. (2001). How Tzero™ Technology Improves DSC
Performance Part III: The Measurement of Specific Heat Capacity.
TA Instruments: New Castle, DE, USA.

for DSC:

dQ/dt (watts) = cp * beta * sample_mass
dQ/dt * 1/sample_mass (watts/gram) = cp * beta

beta is heating rate (kelvin or degC per minute)



```rust
pub fn pryogel_hps_specific_heat_capacity_spline_low_temp(temperature: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `ground_pyrogel_hps_dsc_spline_data`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Most information comes from:

Kovács, Z., Csík, A., & Lakatos, Á. (2023).
Thermal stability investigations of different
aerogel insulation materials at elevated temperature.
Thermal Science and Engineering Progress, 42, 101906.

Note that this pyrogel information is for ground pyrogel,
which then destroys the structure of the pyrogel and may change its
thermal conductivity. Moreover, crystallisation occurs, which changes
its heat capacity too.


```rust
pub fn ground_pyrogel_hps_dsc_spline_data(temperature: ThermodynamicTemperature) -> Result<SpecificPower, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `pyrogel_thermal_conductivity_commercial_factsheet_spline`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

returns thermal conductivity of pyrogel hps
cited from:
https://www.distributioninternational.com/ASSETS/DOCUMENTS/ITEMS/EN/PYBT10HA_SS.pdf

This is from aspen, tested with ASTM C177 at 2 psi compressive load

```rust
pub fn pyrogel_thermal_conductivity_commercial_factsheet_spline(temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `max_temp_pyrogel_hps`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

pyrogel_hps max temp

```rust
pub fn max_temp_pyrogel_hps() -> ThermodynamicTemperature { /* ... */ }
```

#### Function `min_temp_pyrogel_hps`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

pyrogel_hps min temp

```rust
pub fn min_temp_pyrogel_hps() -> ThermodynamicTemperature { /* ... */ }
```

### Types

#### Enum `Material`

basically,
insert this enum into a thermophysical property function
or something
then it will extract the
thermophysical property for you in unit safe method

```rust
pub enum Material {
    Solid(SolidMaterial),
    Liquid(LiquidMaterial),
}
```

##### Variants

###### `Solid`

Contains a list of selectable solids

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SolidMaterial` |  |

###### `Liquid`

Contains a list of selectable liquids

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LiquidMaterial` |  |

##### Implementations

###### Methods

- ```rust
  pub fn density(self: &Self, temperature: ThermodynamicTemperature, _pressure: Pressure) -> Result<MassDensity, TuasLibError> { /* ... */ }
  ```
  returns density of the material

- ```rust
  pub fn try_get_thermal_conductivity(self: &Self, temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, TuasLibError> { /* ... */ }
  ```
  allows you to get thermal conductivity straight from

- ```rust
  pub fn surface_roughness(self: &Self) -> Result<Length, TuasLibError> { /* ... */ }
  ```
  wrapper to help return surface roughness

- ```rust
  pub fn max_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  gives the maximum temperature for the correlations in the

- ```rust
  pub fn min_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  gives the minimum temperature (in kelvin) for the correlations in the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Material { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> Material { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> Material { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Material) -> bool { /* ... */ }
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

  - ```rust
    fn try_into(self: Self) -> Result<SolidMaterial, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<LiquidMaterial, <Self as >::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `SolidMaterial`

Contains a selection of solids with predefined material properties

```rust
pub enum SolidMaterial {
    SteelSS304L,
    SteelSS304LHighTemp,
    Copper,
    Fiberglass,
    PyrogelHPS,
    NuclearGraphiteMatrixA3,
    NuclearGraphiteIG110,
    CustomSolid((ThermodynamicTemperature, ThermodynamicTemperature), fn(ThermodynamicTemperature) -> SpecificHeatCapacity, fn(ThermodynamicTemperature) -> ThermalConductivity, fn(ThermodynamicTemperature) -> MassDensity, Length),
}
```

##### Variants

###### `SteelSS304L`

stainless steel 304 L,
material properties from
Graves, R. S., Kollie, T. G., McElroy, D. L.,
& Gilchrist, K. E. (1991). The thermal conductivity of
AISI 304L stainless steel. International journal of
thermophysics, 12, 409-415.

###### `SteelSS304LHighTemp`

Stainless steel 304L, **high-temperature correlation set**, valid from
300 K to 1700 K (26.85 degC to 1426.85 degC).

This is the same alloy as [`SolidMaterial::SteelSS304L`] but a
different, wider-ranging data lineage — use it for HTGR / HTR-10 work
where the 1000 K (726.85 degC) ceiling of the Zou/Zweibaum
correlations behind `SteelSS304L` is too low. Unlike `SteelSS304L`,
whose density is a constant 8030 kg/m^3, this variant's density is
temperature-dependent (7894 kg/m^3 at 300 K to 7199 kg/m^3 at 1700 K).

Properties from:
Kim, C. S. (1975). Thermophysical Properties of Stainless Steels.
ANL-75-55, Argonne National Laboratory —
specific heat Eq. (5), density Eq. (16), thermal conductivity
Eq. (28), solid region only.

**Only 300 K to 1600 K is backed by experimental data**; 1600 K to
1700 K is Kim's own least-squares extrapolation into the melting range
(1670-1730 K). See `solid_database::ss_304_l_high_temp` for the
correlations, the measured/extrapolated boundary, and the verification
results against Kim's published tables.

Existing CIET models should keep using [`SolidMaterial::SteelSS304L`]:
its correlations are what the CIET regression tests are validated
against, and this variant is not a drop-in substitute for them.

###### `Copper`

Copper material

###### `Fiberglass`

Fiberglass material

###### `PyrogelHPS`

Pyrogel HPS, or rather a best effort approximation of that given
available data

###### `NuclearGraphiteMatrixA3`

A3-grade nuclear matrix graphite — the fuel-pebble matrix of the
HTR-10 / HTR-PM pebble-bed reactors. Conductivity from the VTB
HTR-PM pebble model (zero-fluence form; CC-BY-4.0, Open tier),
cp from the Butland & Maddison (1973/74) graphite table, density
1730 kg/m^3 from IAEA-TECDOC-1382. Valid 300 K to 2000 K. See
`solid_database::nuclear_graphite` for the correlations, including
fluence-degraded conductivity variants not reachable from this enum.

###### `NuclearGraphiteIG110`

IG-110 fine-grained isotropic nuclear graphite — the HTTR / HTR-10
reflector grade. Conductivity from the VTB HTTR deck (unirradiated
quadratic; CC-BY-4.0, Open tier), cp from the Butland & Maddison
(1973/74) graphite table, density 1770 kg/m^3 per
NEA/NSC/DOC(2006)1 table 1.27. Valid 300 K to 2000 K. See
`solid_database::nuclear_graphite` for the correlations, including
fluence-degraded conductivity variants not reachable from this enum.

###### `CustomSolid`

Custom solid, for the user to decide the correlations himself
or herself

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `(ThermodynamicTemperature, ThermodynamicTemperature)` |  |
| 1 | `fn(ThermodynamicTemperature) -> SpecificHeatCapacity` |  |
| 2 | `fn(ThermodynamicTemperature) -> ThermalConductivity` |  |
| 3 | `fn(ThermodynamicTemperature) -> MassDensity` |  |
| 4 | `Length` |  |

##### Implementations

###### Methods

- ```rust
  pub fn try_get_thermal_conductivity(self: &Self, solid_temp: ThermodynamicTemperature) -> Result<ThermalConductivity, TuasLibError> { /* ... */ }
  ```
  returns the solid material's thermal conductivity, in W/(m K),

- ```rust
  pub fn try_get_cp(self: &Self, solid_temp: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, TuasLibError> { /* ... */ }
  ```
  wrapper that

- ```rust
  pub fn try_get_alpha_thermal_diffusivity(self: &Self, solid_temp: ThermodynamicTemperature, pressure: Pressure) -> Result<DiffusionCoefficient, TuasLibError> { /* ... */ }
  ```
  returns the solid material's thermal diffusivity

- ```rust
  pub fn surface_roughness(self: &Self) -> Result<Length, TuasLibError> { /* ... */ }
  ```
  returns surface roughness for various materials

- ```rust
  pub fn max_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  gives the maximum temperature for the correlations in the

- ```rust
  pub fn min_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  gives the minimum temperature (in kelvin) for the correlations in the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SolidMaterial { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> Material { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<SolidMaterial, <Self as >::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `LiquidMaterial`

Contains a selection of liquids with predefined material properties

```rust
pub enum LiquidMaterial {
    TherminolVP1,
    DowthermA,
    HITEC,
    YD325,
    FLiBe,
    FLiNaK,
    CustomLiquid((ThermodynamicTemperature, ThermodynamicTemperature), fn(ThermodynamicTemperature) -> SpecificHeatCapacity, fn(ThermodynamicTemperature) -> ThermalConductivity, fn(ThermodynamicTemperature) -> DynamicViscosity, fn(ThermodynamicTemperature) -> MassDensity),
}
```

##### Variants

###### `TherminolVP1`

therminol VP1

###### `DowthermA`

DowthermA, using the same correlations as TherminolVP1

###### `HITEC`

HITEC salt, 7 wt% sodium nitrate, 40 wt% sodium nitrite, 53 wt% potassium nitrate

###### `YD325`

YD-325 Synthetic Heat transfer oil
Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
An experimental study on the heat transfer performance of a prototype
molten-salt rod baffle heat exchanger for concentrated solar power.
Energy, 156, 63-72.

###### `FLiBe`


LiF - BeF2  in approx 67 mol% - 33 mol% combination
Data taken from:

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.
properties for a custom liquid material
not covered in the database

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

###### `FLiNaK`

46.5-11.5-42.0 mol% LiF, NaF, KF respectively
eutectic composition

Data taken from:

Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
for nuclear reactor applications: A review. Annals
of Nuclear Energy, 109, 635-647.
properties for a custom liquid material
not covered in the database

Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
Engineering database of liquid salt thermophysical and thermochemical
properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

###### `CustomLiquid`

Custom fluid, for the user to decide the correlations himself
or herself

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `(ThermodynamicTemperature, ThermodynamicTemperature)` |  |
| 1 | `fn(ThermodynamicTemperature) -> SpecificHeatCapacity` |  |
| 2 | `fn(ThermodynamicTemperature) -> ThermalConductivity` |  |
| 3 | `fn(ThermodynamicTemperature) -> DynamicViscosity` |  |
| 4 | `fn(ThermodynamicTemperature) -> MassDensity` |  |

##### Implementations

###### Methods

- ```rust
  pub fn try_get_density(self: &Self, fluid_temp: ThermodynamicTemperature) -> Result<MassDensity, TuasLibError> { /* ... */ }
  ```
  returns density of liquid material

- ```rust
  pub fn try_get_thermal_conductivity(self: &Self, fluid_temp: ThermodynamicTemperature) -> Result<ThermalConductivity, TuasLibError> { /* ... */ }
  ```
  returns the liquid thermal conductivity in a result enum

- ```rust
  pub fn try_get_cp(self: &Self, fluid_temp: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, TuasLibError> { /* ... */ }
  ```
  wrapper that

- ```rust
  pub fn try_get_dynamic_viscosity(self: &Self, fluid_temp: ThermodynamicTemperature) -> Result<DynamicViscosity, TuasLibError> { /* ... */ }
  ```
  obtains a result based on the dynamic viscosity of the material

- ```rust
  pub fn try_get_alpha_thermal_diffusivity(self: &Self, fluid_temp: ThermodynamicTemperature, pressure: Pressure) -> Result<DiffusionCoefficient, TuasLibError> { /* ... */ }
  ```
  returns the liquid material's thermal diffusivity

- ```rust
  pub fn try_get_nu_momentum_diffusivity(self: &Self, fluid_temp: ThermodynamicTemperature, pressure: Pressure) -> Result<DiffusionCoefficient, TuasLibError> { /* ... */ }
  ```
  returns the liquid material's momentum diffusivity (kinematic

- ```rust
  pub fn try_get_prandtl_liquid(self: &Self, temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  provides the prandtl number for a liquid material

- ```rust
  pub fn max_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  gives the maximum temperature for the correlations in the

- ```rust
  pub fn min_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  gives the minimum temperature (in kelvin) for the correlations in the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> LiquidMaterial { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> Material { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<LiquidMaterial, <Self as >::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `range_check`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

range check:
generic checker for whether a temperature value falls within
the specified temperature range
If it falls outside this range, return an error
or throw an error, and the program will not run

```rust
pub fn range_check(material: &Material, material_temperature: ThermodynamicTemperature, upper_temperature_limit: ThermodynamicTemperature, lower_temperature_limit: ThermodynamicTemperature) -> Result<bool, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `fluid_mechanics_correlations`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

Module for correlations of fluid mechanics
suitable for tuas_boussinesq_solver (single phase, negligble density changes
except for buoyancy)
License
   This file is part of a thermal hydraulics library written
   in rust meant to help with the
   fluid mechanics and heat transfer aspects of the calculations
   for the Compact Integral Effects Tests (CIET) and hopefully
   Gen IV Reactors such as the Fluoride Salt cooled High Temperature
   Reactor (FHR)
     
   Copyright (C) 2022-2023  Theodore Kay Chen Ong, Singapore Nuclear
   Research and Safety Initiative, Per F. Peterson, University of
   California, Berkeley Thermal Hydraulics Laboratory

   tuas_boussinesq_solver is free software; you can
   redistribute it and/or modify it
   under the terms of the GNU General Public License as published by the
   Free Software Foundation; either version 2 of the License, or (at your
   option) any later version.

   tuas_boussinesq_solver is distributed in the hope
   that it will be useful, but WITHOUT
   ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
   FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
   for more details.

   This thermal hydraulics library
   contains some code copied from GeN-Foam, and OpenFOAM derivative.
   This offering is not approved or endorsed by the OpenFOAM Foundation nor
   OpenCFD Limited, producer and distributor of the OpenFOAM(R)software via
   www.openfoam.com, and owner of the OPENFOAM(R) and OpenCFD(R) trademarks.
   Nor is it endorsed by the authors and owners of GeN-Foam.

   You should have received a copy of the GNU General Public License
   along with this program.  If not, see <http://www.gnu.org/licenses/>.

© All rights reserved. Theodore Kay Chen Ong,
Singapore Nuclear Research and Safety Initiative,
Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Laboratory

Main author of the code: Theodore Kay Chen Ong, supervised by
Professor Per F. Peterson

Btw, I have no affiliation with the Rust foundation.

# Fluid mechanics correlations

Dimensionless friction and flow correlations for 1D pipe/component
thermal-hydraulics. Contents:
- Churchill friction factor (`churchill_friction_factor`): Darcy / Fanning /
  Moody friction factor as a function of Reynolds number and relative
  roughness, valid across the laminar, transition and turbulent regimes.
- Custom fLDK form-loss correlations (`custom_fldk`) for user-supplied
  friction-factor and form-loss (K) functions.
- Non-dimensionalisation (`dimensionalisation`): Reynolds number and Bejan
  number (dimensionless pressure loss) conversions to and from `uom`
  quantities.
- Courant number (`courant_number`): dimensionless CFL / stability numbers.
- Pipe pressure-loss calculations (`pipe_calculations`).

The free functions defined directly in this module (`darcy`, `moody`,
`fldk`, `get_bejan_d`, `get_reynolds_number`) are thin wrappers over
`churchill_friction_factor` that operate on bare `f64` dimensionless inputs.

```rust
pub mod fluid_mechanics_correlations { /* ... */ }
```

### Modules

## Module `churchill_friction_factor`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

calculate darcy, fanning friction factor
using churchill correlation
Churchill (1977) friction-factor correlation and the Bejan-number
(dimensionless pressure loss) relations built on top of it.

All functions here take bare `f64` dimensionless inputs — Reynolds number,
relative roughness (`epsilon / D`), length-to-diameter ratio (`L/D`) and
form loss `K` — and return dimensionless `f64` results. The Churchill
correlation gives the Darcy (and Fanning/Moody) friction factor with a
single smooth expression valid across the laminar, transition and turbulent
regimes; the Reynolds number must be strictly positive (Re = 0 or Re < 0
panics).

```rust
pub mod churchill_friction_factor { /* ... */ }
```

### Functions

#### Function `darcy`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates darcy friction factor using churchill correlation

```rust
pub fn darcy(reynolds_number: f64, roughness_ratio: f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `moody`

calculates moody friction factor using churchill correlation
basically same as darcy

```rust
pub fn moody(reynolds_number: f64, roughness_ratio: f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `f_ldk`

calculates fLDK using churchill correlation
and a user defined form loss K value

```rust
pub fn f_ldk(reynolds_number: f64, roughness_ratio: f64, length_to_diameter_ratio: f64, k: f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_bejan_number_d`

calculates a nondimensional pressure loss (Be_D)
from the nondimensionalised flowrate (Re_D)

```rust
pub fn get_bejan_number_d(reynolds_number: f64, roughness_ratio: f64, length_to_diameter_ratio: f64, form_loss_k: f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_reynolds_from_bejan`

calculates Re given a Be_D

it is basically calculating nondimensionalised
flowrate from nondimensionalised pressure loss

```rust
pub fn get_reynolds_from_bejan(bejan_number_d: f64, roughness_ratio: f64, length_to_diameter: f64, form_loss_k: f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `custom_fldk`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

contains functions and/or structs
which help you calcualte a custom fLDK factor

ie

(f L/D +K )

f is the darcy firction factor
L/D is length to diameter ratio
K is the form loss
Form-loss (fLDK) correlations for components whose Darcy friction factor
and/or form-loss coefficient K are supplied by the caller as functions.

The dimensionless pressure-loss group is `fLDK = f * (L/D) + K`, and the
Bejan number (dimensionless pressure loss) is `Be_D = 0.5 * fLDK * Re^2`.
All inputs and outputs are bare `f64` dimensionless quantities (Reynolds
number, relative roughness, `L/D`). Reynolds number is recovered from a
Bejan number by bisection root-finding.

```rust
pub mod custom_fldk { /* ... */ }
```

### Functions

#### Function `custom_f_ldk`

this first function allows for custom fldk,
ie both friction factor and form loss k are user defined
<https://stackoverflow.com/questions/36390665/how-do-you-pass-a-rust-function-as-a-parameter>

```rust
pub fn custom_f_ldk(custom_darcy: &dyn Fn(f64, f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError>, reynolds_number: f64, roughness_ratio: f64, length_to_diameter_ratio: f64, custom_k: &dyn Fn(f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError>) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `custom_kpipe`

this is a special case of the fLDK component,
where we just specify a custom K but friction factor is based
on darcy friction factor

```rust
pub fn custom_kpipe(reynolds_number: f64, roughness_ratio: f64, length_to_diameter_ratio: f64, custom_k: &dyn Fn(f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError>) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `custom_kpipe_be_d`

This function is special,
not really used, as often
it assumes that the form losses, K for the pipe
take some functional form rather than staying constant


```rust
pub fn custom_kpipe_be_d(reynolds_number: f64, roughness_ratio: f64, length_to_diameter_ratio: f64, custom_k: &dyn Fn(f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError>) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `custom_f_ldk_be_d`

this functions calculates the bejan number using the
custom fLDK formula

```rust
pub fn custom_f_ldk_be_d(custom_darcy: &dyn Fn(f64, f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError>, reynolds_number: f64, roughness_ratio: f64, length_to_diameter_ratio: f64, custom_k: &dyn Fn(f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError>) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_reynolds`

this code allos us to get Reynold's number from a Bejan
number for a custom pipe.
i make no assumptions about the symmetry of flow
ie. i don't make assumptions about whether
the pipe exhibits the same pressure loss
in forwards and backwards flow,
that is up to the user to decide when
customDarcy and customK is put in

```rust
pub fn get_reynolds(custom_darcy: &'static dyn Fn(f64, f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError>, bejan_d: f64, roughness_ratio: f64, length_to_diameter: f64, custom_k: &'static dyn Fn(f64) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError>) -> anyhow::Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `dimensionalisation`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

contains functions and/or structs
which help you dimensionalise and nondimensionalise variables
eg Reynold's number
Conversions between dimensional (`uom`) fluid quantities and the
dimensionless groups used by the friction-factor correlations.

Computes the Reynolds number `Re = rho * u * D_H / mu` (from either velocity
or mass flowrate, and back to mass flowrate) and the Bejan number
`Be_D = Delta_P * rho * D_H^2 / mu^2` (dimensionless pressure loss), to and
from a `uom` `Pressure`. Physical inputs are `uom`-typed (density in kg/m^3,
velocity in m/s, hydraulic diameter in m, dynamic viscosity in Pa*s,
pressure in Pa); the dimensionless results are returned as `Ratio` (or a
bare `f64` where a Bejan number is taken as input).

```rust
pub mod dimensionalisation { /* ... */ }
```

### Functions

#### Function `calc_reynolds_from_velocity`

calculates reynolds number given a fluid average velocity

```rust
pub fn calc_reynolds_from_velocity(fluid_density: MassDensity, velocity: Velocity, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity) -> Ratio { /* ... */ }
```

#### Function `calc_reynolds_from_mass_rate`

calculates Re = mass_flow/area * D_H/mu

```rust
pub fn calc_reynolds_from_mass_rate(fluid_mass_flowrate: MassRate, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity) -> Ratio { /* ... */ }
```

#### Function `calc_reynolds_to_mass_rate`

converts Re to mass flowrate using
Re = mass_flow/area * D_H/mu

```rust
pub fn calc_reynolds_to_mass_rate(cross_sectional_area: Area, reynolds_number: Ratio, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity) -> MassRate { /* ... */ }
```

#### Function `calc_bejan_from_pressure`

calculates Bejan number from pressure

Be_D = Delta P * rho * D_H^2 / mu^2

```rust
pub fn calc_bejan_from_pressure(fluid_pressure: Pressure, hydraulic_diameter: Length, fluid_density: MassDensity, fluid_viscosity: DynamicViscosity) -> Ratio { /* ... */ }
```

#### Function `calc_bejan_to_pressure`

converts Bejan number to pressure
using:

Be_D = Delta P * rho * D_H^2 / mu^2

```rust
pub fn calc_bejan_to_pressure(bejan_d: f64, hydraulic_diameter: Length, fluid_density: MassDensity, fluid_viscosity: DynamicViscosity) -> Pressure { /* ... */ }
```

## Module `courant_number`

Courant Number Modules
Courant / stability numbers for explicit 1D and 3D thermal-hydraulic
marching schemes.

Provides the Courant-Friedrichs-Lewy (CFL) number for fluid advection
(a 1D form and OpenFOAM-style 3D control-volume forms), the Fourier number
for conduction, a Biot-Fourier product for convection, and an
enthalpy-transport Courant number. Each function takes `uom`-typed
quantities and returns the dimensionless number as `f64`, returning
`Err(value)` when the relevant stability limit is exceeded.

```rust
pub mod courant_number { /* ... */ }
```

### Functions

#### Function `get_fluid_courant_number_one_dimension`

calculates Courant-Friedrichs-Lewy number (CFL)
for 1D cells for fluid flow

formula is U * Delta T / Length

```rust
pub fn get_fluid_courant_number_one_dimension(fluid_velocity: Velocity, timestep: Time, cell_length_scale: Length) -> Result<f64, f64> { /* ... */ }
```

#### Function `courant_number_3d_openfoam_algorithm_velocity`

courant number 3D

calculates the CFL number for a 3D case

this is based on OpenFOAM's Courant number algorithm:

Co =  0.5 * timestep / volume *  
(summation (dot product of U and Area).abs())

Co =  0.5 * timestep / volume *  
(summation (dot product of U and normal vector).abs()*Area_magnitude)

this takes in three vectors, the volume of the control volume
and a timestep

This is for an arbitrarily shaped control volume with a number
of flat faces

the first vector specifies fluid velocity at each of these flat
faces

the second vector specifies the angle to the normal that these
velocities make with the area normals
we can define area normals as pointing inwards towards the
center of the cell,

but it doesn't really matter, we need to take the absolute value of
the dot product of the velocities with the normals anyhow


```rust
pub fn courant_number_3d_openfoam_algorithm_velocity(control_volume_volume: Volume, timestep: Time, face_area_vector: Vec<Area>, velocity_vector: Vec<Velocity>, angle_between_area_normals_and_velocity_vector: Vec<Angle>) -> Result<f64, f64> { /* ... */ }
```

#### Function `courant_number_3d_openfoam_algorithm_vol_flowrate`

similar algorithm based on volumetric flowrates in and out

```rust
pub fn courant_number_3d_openfoam_algorithm_vol_flowrate(control_volume_volume: Volume, timestep: Time, volume_flowrate_vector: Vec<VolumeRate>) -> Result<f64, f64> { /* ... */ }
```

#### Function `fourier_number_heat_conduction`

Courant number equivalent for conduction heat transfer

For conduction based heat transfer,
Courant number is just the fourier number
but the characteristic length is
the mesh length

Co = Fo
fourier number

will return an error if value is more than 0.25


```rust
pub fn fourier_number_heat_conduction(alpha_thermal_diffusivity: DiffusionCoefficient, timestep: Time, mesh_length: Length) -> Result<f64, f64> { /* ... */ }
```

#### Function `fourier_number_heat_convection`

Courant number equivalent for convection heat transfer
essentially calculates Co = Bi Fo

will return an error if value is more than 0.25


```rust
pub fn fourier_number_heat_convection(heat_transfer_coeffcient_for_external_fluid: HeatTransfer, control_volume_thermal_conductivity: ThermalConductivity, volume_cv_to_surface_area_cv_ratio: Length, alpha_thermal_diffusivity: DiffusionCoefficient, timestep: Time) -> Result<f64, f64> { /* ... */ }
```

#### Function `single_face_courant_number_enthalpy_flow`

Courant number for heat transport (ie mass flowrate)

calculate courant number for enthalpy transport based
on mass flowrates on one face

```rust
pub fn single_face_courant_number_enthalpy_flow(mass_flowrate: MassRate, control_volume_mass: Mass, timestep: Time) -> Result<f64, f64> { /* ... */ }
```

## Module `pipe_calculations`

pipe calculations

these are pre-built functions which make calculating mass flowrate
and pressure drop across pipes easier
Pre-built pipe pressure-loss / mass-flowrate calculations.

`pipe_calc_pressure_loss` maps a mass flowrate to the pressure loss across a
straight pipe (including a form-loss coefficient K), and
`pipe_calc_mass_flowrate` is its inverse. Both compose the Reynolds/Bejan
non-dimensionalisation with the Churchill friction factor, take `uom`-typed
pipe geometry and fluid properties, and handle reverse (negative) flow.

```rust
pub mod pipe_calculations { /* ... */ }
```

### Functions

#### Function `pipe_calc_pressure_loss`

a function calculates pressure
loss given a mass flowrate and pipe properties

```rust
pub fn pipe_calc_pressure_loss(fluid_mass_flowrate: MassRate, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, pipe_length: Length, absolute_roughness: Length, form_loss_k: f64) -> Result<Pressure, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `pipe_calc_mass_flowrate`

a function which calculates mass flowrate
given a pressure loss and pipe properties

```rust
pub fn pipe_calc_mass_flowrate(pressure_loss: Pressure, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, pipe_length: Length, absolute_roughness: Length, form_loss_k: f64) -> Result<MassRate, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

### Functions

#### Function `darcy`

This function calculates darcy friction factor
It takes in a Reynold's number and roughness ratio

and gives the darcy friction factor for laminar
turbulent, and transition regimes.

However, Re = 0 will not work!
```rust
let darcy_friction_factor =
    tuas_boussinesq_solver::
    fluid_mechanics_correlations::darcy(1800.0,0.0015).unwrap();

println!("{}", darcy_friction_factor);
```

```rust
pub fn darcy(reynolds_number: f64, roughness_ratio: f64) -> Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `moody`

This function calculates moody friction factor
It takes in a Reynold's number and roughness ratio

and gives the darcy friction factor for laminar
turbulent, and transition regimes.

It's basically the same as darcy friction factor

However, Re = 0 will not work!
```rust
let moody_friction_factor =
    tuas_boussinesq_solver::
    fluid_mechanics_correlations::moody(1800.0,0.0015).unwrap();

println!("{}", moody_friction_factor);
```

```rust
pub fn moody(reynolds_number: f64, roughness_ratio: f64) -> Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `fldk`

This function calculates the fldk

this is the

Be = 0.5 * Re^2 * (f * (L/D) + K)

the f is darcy friction factor

and the term in the brackets is fldk

you are to give a K value, L/D value, Re
and roughness ratio

However, Re = 0 will not work!
```rust
   let fldk =
       tuas_boussinesq_solver::
       fluid_mechanics_correlations::fldk(
           15000.0,0.00014,10.0,5.0).unwrap();

   println!("{}", fldk);
```

```rust
pub fn fldk(reynolds_number: f64, roughness_ratio: f64, length_to_diameter_ratio: f64, k: f64) -> Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_bejan_d`

This function calculates the bejan number

this is the


Be = (P * D^2)/(mu * nu)

P is pressure loss
D is hydraulic diameter
mu is dynamic viscosity
nu is kinematic viscosity

Be is the bejan number which is dimensionless

It is calculated using:
Be = 0.5 * Re^2 * (f * (L/D) + K)

the f is darcy friction factor

and the term in the brackets is fldk

you are to give a K value, L/D value, Re
and roughness ratio

Re = 0  and Re < 0 is supported,
this assumes that the component is symmetrical
in terms of pressure loss, which may usually
be the case for pipes anyhow



```rust
let bejan_d =
    tuas_boussinesq_solver::
    fluid_mechanics_correlations::get_bejan_d(
        0.00000000000001,0.00014,10.0,5.0).unwrap();

println!("{}", bejan_d);

let bejan_d =
    tuas_boussinesq_solver::
    fluid_mechanics_correlations::get_bejan_d(
        -5000.0,0.00014,10.0,5.0).unwrap();

println!("{}", bejan_d);

let bejan_d =
    tuas_boussinesq_solver::
    fluid_mechanics_correlations::get_bejan_d(
        0.0,0.00014,10.0,5.0).unwrap();

println!("{}", bejan_d);
```

```rust
pub fn get_bejan_d(reynolds_number: f64, roughness_ratio: f64, length_to_diameter_ratio: f64, k: f64) -> Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_reynolds_number`

This function calculates the Reynolds number given
a Bejan number.

Remember Bejan number is dimensionless pressure
drop

Be = (P * D^2)/(mu * nu)

P is pressure loss
D is hydraulic diameter
mu is dynamic viscosity
nu is kinematic viscosity

We implicitly solve for Re using:
Be = 0.5 * Re^2 * (f * (L/D) + K)

the f is darcy friction factor

and the term in the brackets is fldk

you are to give a K value, L/D value, Be
and roughness ratio

Re = 0  and Re < 0 is supported,
this assumes that the component is symmetrical
in terms of pressure loss, which may usually
be the case for pipes anyhow


In the following example, we get a bejan number calculated
first with Re = 5000.0
and then using that bejan number, we try and find the Re again
which should be about 5000.0

we use the approx package and ensure that the numbers are similar
to within 0.001 or 0.1% of each other

```rust

extern crate approx;
let bejan_d =
tuas_boussinesq_solver::fluid_mechanics_correlations::
    get_bejan_d(
        5000.0,0.00014,10.0,5.0).unwrap();

println!("{}", bejan_d);

let reynolds_number =
tuas_boussinesq_solver::fluid_mechanics_correlations::
    get_reynolds_number(
        bejan_d,0.00014,10.0,5.0).unwrap();

approx::assert_relative_eq!(reynolds_number, 5000.0,
max_relative = 0.001);
```


Note: why can't we just find Reynold's number from friction factor?

Note that in the laminar and turbulent region, a single Reynold's
number can have two different friction factor values.
Even in the transition region, there's probably a range of friction
factors where Re can have a third or fourth value
That's not good

Hence Reynold's number is not a function of friction factor unless
you restrict Re to a certain range

To get around this, we assume that pressure losses are a function
of Re and vice versa,

meaning to say each pressure loss value maps to a single Re
and therefore dimensionless pressure losses (Be) should also
map to a single Re.

Therefore, we must supply a Bejan number to get an Re value.


```rust
pub fn get_reynolds_number(bejan_d: f64, roughness_ratio: f64, length_to_diameter: f64, form_loss_k: f64) -> Result<f64, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `heat_transfer_correlations`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

Module for heat transfer correlations
suitable for tuas_boussinesq_solver (single phase, negligble density changes
except for buoyancy)
Heat-transfer correlations used by the TUAS Boussinesq solver.

This module groups the empirical and analytical relations that turn
geometry, fluid state, and flow conditions into heat-transfer quantities:

- [`nusselt_number_correlations`] — dimensionless Nusselt number `Nu`
  correlations for convection (pipe flow, packed beds), stating the
  laminar/turbulent regime and Reynolds/Prandtl validity of each.
- [`thermal_resistance`] — 1D conduction/convection thermal resistance
  (kelvin per watt) and conductance (watts per kelvin), returning heat
  flow (watts) for a given temperature difference.
- [`view_factors`] — dimensionless radiation view factors (concentric
  cylinders) for the clamshell radiative heater geometry.
- [`parallel_heat_exchangers`] — log-mean-temperature-difference (LMTD)
  based heat duty for parallel-piped heat exchangers.
- [`heat_transfer_interactions`] — dispatch layer that, given the shape of
  two control volumes (slab / cylinder / sphere) and an interaction type
  (conduction, convection, advection, radiation), computes the conductance
  or heat flow between them.

Anything that is not a heat-transfer correlation (fluid mechanics,
thermophysical properties, mesh/geometry primitives) belongs in the
sibling modules, not here.

```rust
pub mod heat_transfer_correlations { /* ... */ }
```

### Modules

## Module `nusselt_number_correlations`

correlations for convective heat transfer
The nusselt correlations class has calculates nusselt numbers
given a certain geometry

```rust
pub mod nusselt_number_correlations { /* ... */ }
```

### Modules

## Module `pipe_correlations`

These are nusselt correlations for pipes

```rust
pub mod pipe_correlations { /* ... */ }
```

### Functions

#### Function `nusselt_ciet_heater_v1_0`

A nusselt correlation for CIET heater v1.0

it returns Nu = 8.0
for Re < 2000.0

and returns Nu = 5.44 + 0.034*Re^(0.82)
for Re >= 2000.0
```rust
extern crate approx;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;


// for Re < 2000, return 8
let Re_laminar = 1500.0;

let Nu_laminar_test = pipe_correlations::nusselt_ciet_heater_v1_0(Re_laminar);

approx::assert_relative_eq!(8.0, Nu_laminar_test, max_relative=0.001);

// the following two tests are taken from table 3-1 of:
// <http://fhr.nuc.berkeley.edu/wp-content/uploads/2015/04/14-009_CIET-IRP-Final-Report.pdf>
// this is page 33 out of 103 for the document

// this test is accurate to within 1% of stated value

let Re_turbulent = 2768_f64;
let Nu_turbulent_test = pipe_correlations::
nusselt_ciet_heater_v1_0(Re_turbulent);

approx::assert_relative_eq!(28.0, Nu_turbulent_test, max_relative=0.01);

// this test is accurate to within 3% of stated value

let Re_turbulent_2 = 3932_f64;
let Nu_turbulent_test_2 = pipe_correlations::
nusselt_ciet_heater_v1_0(Re_turbulent_2);

approx::assert_relative_eq!(36.0, Nu_turbulent_test_2, max_relative=0.03);



```

Note that there is a discontinuity at Re = 2000
and that this is test bay data...
When heater was installed in CIET, there were different results


```rust
pub fn nusselt_ciet_heater_v1_0(reynolds_number: f64) -> f64 { /* ... */ }
```

#### Function `dittus_boelter_correlation`

 Dittus Boelter Correlation

 <https://www.e3s-conferences.org/articles/e3sconf/pdf/2017/01/e3sconf_wtiue2017_02008.pdf>


 Meant for turbulent flow
 Smooth surface tubes
 Heiss, J. F., & Coull, J. (1951). Nomograph of Dittus-Boelter
 equation for heating and cooling
 liquids. Industrial & Engineering Chemistry, 43(5), 1226-1229.


 <http://herve.lemonnier.sci.free.fr/TPF/NE/Winterton.pdf>

 The original paper is here

 Dittus, F. W., & Boelter, L. M. K. (1985). Heat transfer in
 automobile radiators of the tubular type. International
 communications in heat and mass transfer, 12(1), 3-22.

 The Dittus Boelter correlation has two forms,
 one for heating and one for cooling

 By heating I mean that the fluid is heated
 and heat is transfered from the tube walls to the
 heater

 And by cooling I mean that the fluid is cooled
 and the wall takes heat from the fluid

 ```rust
 extern crate approx;
 use tuas_boussinesq_solver::heat_transfer_correlations::
 nusselt_number_correlations::pipe_correlations;

 // here we have an example for heating
 // Re = 10000, Pr = 17


 let Re = 10000_f64;
 let Pr = 17_f64;

 let heating_ref_nu = 0.023 * Re.powf(0.8) * Pr.powf(0.4);

 let heating_test_bool = true;

 let mut test_Nu = pipe_correlations::dittus_boelter_correlation(Re, Pr,
 heating_test_bool);

 approx::assert_relative_eq!(heating_ref_nu, test_Nu,
 max_relative=0.01);

 // here we have an example for cooling
 // Re = 10000, Pr = 17

 let cooling_ref_nu = 0.023 * Re.powf(0.8) * Pr.powf(0.3);

 let cooling_test_bool = false;

 test_Nu = pipe_correlations::dittus_boelter_correlation(Re, Pr,
 cooling_test_bool);

 approx::assert_relative_eq!(cooling_ref_nu, test_Nu,
 max_relative=0.01);
 ```

<https://www.nuclear-power.com/nuclear-engineering/heat-transfer/convection-convective-heat-transfer/sieder-tate-equation/>

 Unfortunately, Dittus Boelter correlation is valid
 only for small to moderate temperature differences

 For larger temperature differences, use Sieder-Tate




```rust
pub fn dittus_boelter_correlation(reynolds_number: f64, prandtl_number: f64, heating: bool) -> f64 { /* ... */ }
```

#### Function `sieder_tate_correlation`

Sieder Tate Relationship

<https://www.e3s-conferences.org/articles/e3sconf/pdf/2017/01/e3sconf_wtiue2017_02008.pdf>

<https://www.nuclear-power.com/nuclear-engineering/heat-transfer/convection-convective-heat-transfer/sieder-tate-equation/>

Note that properties here are evaluated at Tavg (ie average bulk fluid
temperature)

For pipe or heat exchanger,
it could be

Tavg = (T_outlet + T_inlet)/2

the Re, Pr is generally evaluated at fluid temperature
whereas the fluid viscosity ratio is the ratio of viscosity at
the bulk fluid temperature to
fluid viscosity at wall temperature

Yang, X., Yang, X., Ding, J., Shao, Y., & Fan, H. (2012).
Numerical simulation study on the heat transfer
characteristics of the tube receiver of the
solar thermal power tower. Applied Energy, 90(1), 142-147.

viscosity_ratio = mu_f / mu_s

note that this ratio is a dynamic viscosity ratio, not
kinematic viscosity ratio


The range of applicability (from Perry's Handbook)
is
0.7 < Pr < 16700
and
4000 < Re_D <10000

and

0.0044 < viscosity_ratio <  9.75

The viscosity ratio bounds are estimated from the
the seider tate laminar heat transfer correlation,
i assumed they are of the same bounds. Did not check
however.

This is for fully developed turbulent flow only

viscosity_ratio = 5.0;


```rust

extern crate approx;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;

let Re = 8000_f64;
let Pr = 17_f64;

// the viscosity ratio is assumed to be 5

let viscosity_ratio = 5.0_f64;

let nu_f_reference = 0.027 * Re.powf(0.8)
* Pr.powf(1.0/3.0) *
viscosity_ratio.powf(0.14);

let test_nu = pipe_correlations::sieder_tate_correlation(
Re, Pr, viscosity_ratio);

approx::assert_relative_eq!(nu_f_reference, test_nu,
max_relative=0.01);

```



meant for turbulent flow

```rust
pub fn sieder_tate_correlation(reynolds_number: f64, prandtl_number: f64, viscosity_ratio_fluid_over_wall: f64) -> f64 { /* ... */ }
```

#### Function `gnielinski_correlation_liquids_fully_developed`

Gnielinski Equation for liquids


<https://www.e3s-conferences.org/articles/e3sconf/pdf/2017/01/e3sconf_wtiue2017_02008.pdf>

turbulent flow, all kinds of tubes

However, flow should be fully developed

```rust

extern crate approx;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;

let Re = 8000_f64;
let Pr_fluid = 17_f64;
let Pr_wall = 12_f64;
let darcy_friction_factor = 0.005_f64;

// let's now calculate the nusslet number

let prandtl_ratio = Pr_fluid/Pr_wall;

let darcy_ratio: f64 = darcy_friction_factor/8.0;

let numerator: f64 = darcy_ratio * (Re - 1000_f64) * Pr_fluid *
    prandtl_ratio.powf(0.11);
let denominator:f64 = 1_f64 + 12.7_f64 * darcy_ratio.powf(0.5) *
    (Pr_fluid.powf(2.0/3.0) - 1.0);



let nu_f_reference = numerator/denominator;

let test_nu = pipe_correlations::gnielinski_correlation_liquids_fully_developed(
Re,Pr_fluid, Pr_wall,darcy_friction_factor);
///
approx::assert_relative_eq!(nu_f_reference, test_nu,
max_relative=0.01);

```


```rust
pub fn gnielinski_correlation_liquids_fully_developed(reynolds_number: f64, prandtl_number_bulk_fluid: f64, prandtl_number_wall: f64, darcy_friction_factor: f64) -> f64 { /* ... */ }
```

#### Function `laminar_nusselt_uniform_heat_flux_fully_developed`

returns a nusselt number of 4.36,

This is an estimate for constant heat flux nusselt number
for fully developed thermal and velocity boundary layers


```rust
extern crate approx;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;

let nu_reference = 4.36_f64;
let Re = 1800_f64;
let nu_test = pipe_correlations::laminar_nusselt_uniform_heat_flux_fully_developed(
Re);


approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.01);



```

```rust
pub fn laminar_nusselt_uniform_heat_flux_fully_developed(reynolds_number: f64) -> f64 { /* ... */ }
```

#### Function `laminar_nusselt_uniform_wall_temperature_fully_developed`

returns a nusselt number of 3.66,

This is an estimate for constant wall temperature nusselt number
for fully developed thermal and velocity boundary layers

Re is measured at bulk temp
T_bulk = (T_in + T_out)/2

```rust
extern crate approx;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;

let nu_reference = 3.66_f64;
let Re = 1800_f64;
let nu_test = pipe_correlations::laminar_nusselt_uniform_wall_temperature_fully_developed(
Re);


approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.01);



```

```rust
pub fn laminar_nusselt_uniform_wall_temperature_fully_developed(reynolds_number: f64) -> f64 { /* ... */ }
```

#### Function `laminar_nusselt_uniform_wall_temperature_developing`

estimates Nusselt Number for developing flow
in laminar regime
for tubes
constant wall temperature

Re, Pr is measured at bulk temp
T_bulk = (T_in + T_out)/2

for fully developed flow, we need L/D to be about 20 or more

in Gnielinsiki's paper, when we have Pr = 0.7, we then have
and L/D about 1000, or D/L  = 0.001, then we can have
Nusselt number almost 3.66

For higher Prandtl numbers, the tendancy is for Nusselt numbers
to increase more especially due to influence of developing flow.

The second test case is for Pr about 70, which is the other extreme
case. From Gnielinski's paper, Nusselt number is about
8.0 for Re = 2000 and d/L = 0.001



```rust

extern crate approx;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;

let mut nu_reference = 3.66_f64;
let mut Re = 1000_f64;
let mut Pr = 0.7_f64;
let lengthToDiameterRatio = 1000_f64;

let mut nu_test = pipe_correlations::laminar_nusselt_uniform_wall_temperature_developing(
Re,
Pr,
lengthToDiameterRatio);



approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.01);

// this is the second part of the test

nu_reference = 8_f64;
Re = 2000_f64;
Pr = 70_f64;

nu_test = pipe_correlations::laminar_nusselt_uniform_wall_temperature_developing(
Re,
Pr,
lengthToDiameterRatio);


approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.05);

```





```rust
pub fn laminar_nusselt_uniform_wall_temperature_developing(reynolds_number: f64, prandtl_number: f64, length_to_diameter_ratio: f64) -> f64 { /* ... */ }
```

#### Function `laminar_nusselt_uniform_heat_flux_developing`

estimates Nusselt Number for developing flow
in laminar regime
for tubes
constant heat flux

Re, Pr is measured at bulk temp
T_bulk = (T_in + T_out)/2

note that wall temperatures are not required in this case

for fully developed flow, we need L/D to be about 20 or more

in Gnielinsiki's paper, when we have Pr = 0.7, we then have
and L/D about 1000, or D/L  = 0.001, then we can have
Nusselt number almost 3.66 for uniform wall temperature

Nu = 3.66 is the Nusselt number for uniform wall temperature
fully developed flow

Hence, for constant heat flux, to get a value of 4.36 which
is the value for fully developed flow, we need about Pr = 0.7
D/L = 0.001, or L/D = 1000, and Re about 1000-2300


There are some pieces of data available from Gnielinski's correlation
for Pr = 0.7
L/D = 10000,

The flow seems to also be fully developed here.

the value seems to be 4.36 no matter the Reynold's number

Now in CIET from Zweibaum's PhD thesis
, the parasitic heat losses in steady state heat
transfer were underestimated by about 75% in the priamry loop
and about 50% in the DRACS loop when using normal correlations
even for experiments thought to be steady state,

One possible contribution to this error is where Nusselt
numbers are underestimated in the laminar regime due to
flow development. Of course, there could be heat losses
due to instruments, connected heat structures and etc,
but a lower convective thermal resistance at the pipe wall
would increase heat transfer anyhow.

In an oversimplistic test, I take the fully developed flow
constant heat flux nusselt numer of 4.36, multiply that
by 1.75 to account for 75% underestimation, and compare that to a
typical nusselt number generated by this correlation
in the laminar regime.

A typical pipe in the CTAH loop has the following parameters:

long L/D ratio is about 87
the typical Pr at dowtherm A temp about 80C is
17 or 18 and Re =  200 therabout.

In test 4, we see that the 1.75 correction factor
applied to Nu = 4.36 is within 8% of the value generated by
this correlation when CIET parameters are used.

This looks promising of course. But we have not taken into
account conductive thermal resistance and insulation.

Nevertheless, it is promising to look into this as a potential
source of error.

```rust

extern crate approx;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;

let mut nu_reference = 4.36_f64;


// test 1

let mut Re = 1000_f64;
let mut Pr = 0.7_f64;
let mut lengthToDiameterRatio = 1000_f64;

let mut nu_test = pipe_correlations::laminar_nusselt_uniform_heat_flux_developing(
Re,
Pr,
lengthToDiameterRatio);



approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.01);

// test 2

Re = 1600_f64;
lengthToDiameterRatio = 10000_f64;

nu_test = pipe_correlations::laminar_nusselt_uniform_heat_flux_developing(
Re,
Pr,
lengthToDiameterRatio);

approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.01);

// test 3


Re = 2300_f64;
lengthToDiameterRatio = 10000_f64;

nu_test = pipe_correlations::laminar_nusselt_uniform_heat_flux_developing(
Re,
Pr,
lengthToDiameterRatio);

approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.01);


// test 4 (CIET prototypical test)

nu_reference = 4.36_f64 * 1.75;

Re = 200_f64;
lengthToDiameterRatio = 87_f64;
Pr = 18_f64;

nu_test = pipe_correlations::laminar_nusselt_uniform_heat_flux_developing(
Re,
Pr,
lengthToDiameterRatio);

approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.08);



```

For fully developed flow, multiple data points were available
for comparison. Otherwise, there were not as many data points.




```rust
pub fn laminar_nusselt_uniform_heat_flux_developing(reynolds_number: f64, prandtl_number: f64, length_to_diameter_ratio: f64) -> f64 { /* ... */ }
```

#### Function `gnielinski_turbulent_correlation_liquids_developing_bulk_fluid_prandtl`

estimates Nusselt Number for developing flow
in turbulent regime (Re > 4000)
for tubes
regardless of boundary conditions (constant heat flux, wall temp
mixed or anything else)


Re, Pr_fluid is measured at bulk temp
T_bulk = (T_in + T_out)/2

Pr_wall is liquid Pr at wall temperature

using gnielinski's data, we can get a Nu of 16
at Pr_fluid = 0.7, Re = 5000
Pr_wall = 0.7
d/L = 0.0001 or
L/D  = 10000

darcy friction factor at these conditions
Re = 5000, L/D = 10000 is calculated
for smooth tubes

darcy_friction_factor = 1.8 * log10 (Re) - 1.5

```rust

extern crate approx;
use tuas_boussinesq_solver::fluid_mechanics_correlations::
darcy;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;

let mut nu_reference = 16_f64;


// test 1

let mut Re = 5000_f64;
let mut Pr = 0.7_f64;
let mut Pr_wall = 0.7_f64;
let mut lengthToDiameterRatio = 10000_f64;

let mut darcy_friction_factor :f64 =
darcy(Re, 0.0).unwrap();

let mut nu_test = pipe_correlations::gnielinski_turbulent_correlation_liquids_developing_bulk_fluid_prandtl(
Re,
Pr,
Pr_wall,
darcy_friction_factor,
lengthToDiameterRatio);



approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.02);
```


```rust
pub fn gnielinski_turbulent_correlation_liquids_developing_bulk_fluid_prandtl(reynolds_number: f64, prandtl_number_bulk_fluid: f64, prandtl_number_wall: f64, darcy_friction_factor: f64, length_to_diameter_ratio: f64) -> f64 { /* ... */ }
```

#### Function `gnielinski_turbulent_correlation_liquids_developing`

estimates Nusselt Number for thermally developing flow
in turbulent regime (Re > 4000)
for tubes
regardless of boundary conditions
regardless of boundary conditions (constant heat flux, wall temp
mixed or anything else)


Re, Pr_fluid is measured at film temp
T_film = (T_bulk + T_wall)/2

Where:
T_bulk = (T_in + T_out)/2

For the correction factor,

(Pr_bulk/Pr_wall) is used.

You may choose to set Pr_bulk = Pr_film if you so wish, but there
is flexibility in this aspect


```rust
pub fn gnielinski_turbulent_correlation_liquids_developing(reynolds_number_film: f64, prandtl_number_bulk_fluid: f64, prandtl_number_film: f64, prandtl_number_wall: f64, darcy_friction_factor: f64, length_to_diameter_ratio: f64) -> f64 { /* ... */ }
```

#### Function `gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl`

Gnielinski correlation for developing
flow regimes (both thermally and hydrodynamically)
for pipe flows with liquids

and for turbulent, developing and lamianr regimes
uses uniform heat flux correlations in laminar regime

Gnielinski, V. (2013). On heat
transfer in tubes. International Journal
of Heat and Mass Transfer, 63, 134-140.

The reference test data is as follows:
at Pr_fluid = 0.7, Pr_wall = 0.7
Re = 3000,

d/L = 0.0001 (L/D = 10000)
Nu is approximately 8.2


```rust

extern crate approx;
use tuas_boussinesq_solver::fluid_mechanics_correlations::
darcy;
use tuas_boussinesq_solver::heat_transfer_correlations::
nusselt_number_correlations::pipe_correlations;

// test 1 (transition region)

let mut nu_reference = 8.2_f64;
let mut Re = 3000_f64;
let mut Pr = 0.7_f64;
let mut Pr_wall = 0.7_f64;
let mut lengthToDiameterRatio = 10000_f64;

let mut darcy_friction_factor :f64 =
darcy(Re, 0.0).unwrap();

let mut nu_test =
pipe_correlations::
gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl(
Re,
Pr,
Pr_wall,
darcy_friction_factor,
lengthToDiameterRatio);



approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.02);


// test 2 (turbulent regime)

let mut nu_reference = 16_f64;
let mut Re = 5000_f64;
let mut Pr = 0.7_f64;
let mut Pr_wall = 0.7_f64;
let mut lengthToDiameterRatio = 10000_f64;

let mut darcy_friction_factor :f64 =
darcy(Re, 0.0).unwrap();

let mut nu_test =
pipe_correlations::
gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl(
Re,
Pr,
Pr_wall,
darcy_friction_factor,
lengthToDiameterRatio);



approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.02);

// test 3 (laminar regime)

let mut nu_reference = 4.36;
let mut Re = 1000_f64;
let mut Pr = 0.7_f64;
let mut Pr_wall = 0.7_f64;
let mut lengthToDiameterRatio = 10000_f64;

let mut darcy_friction_factor :f64 =
darcy(Re, 0.0).unwrap();

let mut nu_test =
pipe_correlations::
gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl(
Re,
Pr,
Pr_wall,
darcy_friction_factor,
lengthToDiameterRatio);



approx::assert_relative_eq!(nu_reference, nu_test,
max_relative=0.02);
```

```rust
pub fn gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl(reynolds: f64, prandtl_number_fluid: f64, prandtl_number_wall: f64, darcy_friction_factor: f64, length_to_diameter_ratio: f64) -> f64 { /* ... */ }
```

#### Function `gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing`

Gnielinski correlation for developing
flow regimes (both thermally and hydrodynamically)
for pipe flows with liquids

and for turbulent, developing and lamianr regimes
uses uniform heat flux correlations in laminar regime

Gnielinski, V. (2013). On heat
transfer in tubes. International Journal
of Heat and Mass Transfer, 63, 134-140.

rather than use only the bulk and wall prandt number
for nusselt calculation,
a film prandtl number is also used here.
This film prandtl number will be used to calculate the nusselt
number in all regimes,

Whereas the bulk and wall prandtl number are only used in the
correction factor in the turbulent and transition regime.


```rust
pub fn gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing(reynolds_number_film: f64, prandtl_number_bulk_fluid: f64, prandtl_number_film: f64, prandtl_number_wall: f64, darcy_friction_factor: f64, length_to_diameter_ratio: f64) -> f64 { /* ... */ }
```

#### Function `custom_gnielinski_turbulent_nusselt_correlation`

from Du's paper

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

we have a generic Gnielinski type correlation,
empirically fitted to experimental data. This is in the form:

Nu = C (Re^m - 280.0) Pr^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25

Du did not mention which Pr to use
I'm going to assume this is Pr_film

Nu = C (Re^m - 280.0) Pr_film^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25

Technically this Pr is Pr(T_film) where
T_film = (T_wall + T_bulkfluid)/2

a simpler estimate is:
Pr_film = (Pr_wall + Pr_fluid)/2

However, the simplest is just to use Pr_bulk as Pr
this may underestimate Nusselt number, as Pr in the bulk fluid is
usually lower, but it may well work

anyway, I just forced the user to give another argument (Pr_film)
After some debugging however, i found this unnecessary.
Pr_film should equal Pr_bulk by default

For Du's Heat exchanger,
C = 0.04318,
m = 0.7797

No specific bounds are given

```rust
pub fn custom_gnielinski_turbulent_nusselt_correlation(correlation_coefficient_c: Ratio, reynolds_exponent_m: f64, prandtl_number_film: Ratio, prandtl_number_fluid: Ratio, prandtl_number_wall: Ratio, reynolds_number: Ratio, length_to_diameter_ratio: Ratio) -> Ratio { /* ... */ }
```

#### Function `custom_gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing`

from Du's paper

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in
a shell-and-tube heat exchanger. International Communications
in Heat and Mass Transfer, 96, 61-68.

we have a generic Gnielinski type correlation,
empirically fitted to experimental data. This is in the form:

Nu = C (Re^m - 280.0) Pr_f^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25

For Du's Heat exchanger,
C = 0.04318,
m = 0.7797


However, this does not cover the transition or laminar regimes,
I used Gnielinski correlation for developing
flow regimes (both thermally and hydrodynamically)
for pipe flows with liquids

and for turbulent, developing and lamianr regimes
uses uniform heat flux correlations in laminar regime

Gnielinski, V. (2013). On heat
transfer in tubes. International Journal
of Heat and Mass Transfer, 63, 134-140.

No specific bounds are given for Prandtl number or otherwise


the transition regime for pipes is around Re = 2300 - 4000
this is taken from the Re for transition in pipes

However, for transitions in tube bundles, we expect them
for around Re = 40-100

Takemoto, Y., Kawanishi, K., & Mizushima, J. (2010). Heat transfer
in the flow through a bundle of tubes and transitions of the flow.
International journal of heat and mass transfer, 53(23-24), 5411-5419.

I will use the Re from 40-100 as the transition regime
at Re of 40 and below, Nu is the same as for pipe lamniar flow

IT MAY NOT BE APPLICABLE IN THIS CASE, but its a decent estimate

darcy friction factor is not used for this case

```rust
pub fn custom_gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing(correlation_coefficient_c: Ratio, reynolds_exponent_m: f64, prandtl_number_film: Ratio, prandtl_number_fluid: Ratio, prandtl_number_wall: Ratio, reynolds_number: Ratio, length_to_diameter_ratio: Ratio) -> f64 { /* ... */ }
```

## Module `input_structs`

contains data types used for nusselt number correlation
enums

```rust
pub mod input_structs { /* ... */ }
```

### Types

#### Struct `NusseltPrandtlReynoldsData`

contains information Nusselt Prandtl Reynold's
correlation
usually in the form:

Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e

a is the constant
b is the reynolds_prandtl_coefficient
c is the reynolds_power,
d is the prandtl_power,
e is the prandtl_correction_factor_power

```rust
pub struct NusseltPrandtlReynoldsData {
    pub reynolds: Ratio,
    pub prandtl_bulk: Ratio,
    pub prandtl_wall: Ratio,
    pub constant: Ratio,
    pub reynolds_prandtl_coefficient: Ratio,
    pub reynolds_power: f64,
    pub prandtl_power: f64,
    pub prandtl_correction_factor_power: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reynolds` | `Ratio` | reynolds number input |
| `prandtl_bulk` | `Ratio` | bulk fluid prandtl number |
| `prandtl_wall` | `Ratio` | wall prandtl number based on wall tmeperature |
| `constant` | `Ratio` | a in<br>Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e |
| `reynolds_prandtl_coefficient` | `Ratio` | b in<br>Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e |
| `reynolds_power` | `f64` | c in<br>Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e |
| `prandtl_power` | `f64` | d in<br>Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e |
| `prandtl_correction_factor_power` | `f64` | power for prandtl number correction factor<br>e in<br>Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e |

##### Implementations

###### Methods

- ```rust
  pub fn custom_reynolds_prandtl(self: &Self) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  obtains nusselt based on:

- ```rust
  pub fn ciet_version_2_heater_uncorrected(self: &Self) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  obtains nusselt based on:

- ```rust
  pub fn ciet_version_2_heater_prandtl_corrected(self: &Self) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  ciet heater correlation for version 2,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NusseltPrandtlReynoldsData { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NusseltPrandtlReynoldsData) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `WakaoData`

Input data for the Wakao particle-to-fluid Nusselt number correlation
in a packed bed of spheres.

Both members are dimensionless (`uom` [`Ratio`]), and both are formed
on the **particle (pebble) diameter**.

# References

Wakao, N., & Funazkri, T. (1978). Effect
of fluid dispersion coefficients on particle-to-fluid mass
transfer coefficients in packed beds: correlation of
Sherwood numbers. Chemical Engineering Science, 33(10), 1375-1384.
(the mass-transfer / Sherwood form)

Wakao, N., Kaguei, S., & Funazkri, T. (1979). Effect of fluid
dispersion coefficients on particle-to-fluid heat transfer
coefficients in packed beds: correlation of Nusselt numbers.
Chemical Engineering Science, 34(3), 325-336.
DOI: 10.1016/0009-2509(79)85064-2
(the heat-transfer / Nusselt form, which is what [`WakaoData::get`]
evaluates)

```rust
pub struct WakaoData {
    pub reynolds: Ratio,
    pub prandtl_bulk: Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reynolds` | `Ratio` | Reynolds number, dimensionless.<br><br>Based on the **particle (sphere/pebble) diameter** `d` and the<br>**superficial** velocity `u` (volumetric flow divided by the<br>*empty* bed cross-section, not the interstitial velocity):<br><br>Re = rho u d / mu<br><br>Using interstitial rather than superficial velocity inflates Re<br>by 1/porosity (roughly a factor of 2.5 for a typical randomly<br>packed bed), so be explicit about which one is supplied. |
| `prandtl_bulk` | `Ratio` | Prandtl number of the fluid, dimensionless.<br><br>Pr = c_p mu / k. Either the bulk-fluid or the film Prandtl number<br>may be supplied; the correlation carries no wall-correction term,<br>so only one Prandtl number is used. |

##### Implementations

###### Methods

- ```rust
  pub fn get(self: &Self) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  Returns the particle-to-fluid Nusselt number for a packed bed of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> WakaoData { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &WakaoData) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `GnielinskiData`

contains data for gnielinski
correlation of various

```rust
pub struct GnielinskiData {
    pub reynolds: Ratio,
    pub prandtl_bulk: Ratio,
    pub prandtl_wall: Ratio,
    pub darcy_friction_factor: Ratio,
    pub length_to_diameter: Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reynolds` | `Ratio` | reynolds number based on hydraulic_diameter |
| `prandtl_bulk` | `Ratio` | bulk fluid prandtl number |
| `prandtl_wall` | `Ratio` | wall prandtl number based on wall temperature |
| `darcy_friction_factor` | `Ratio` | friction factor, set by user |
| `length_to_diameter` | `Ratio` | pipe length to diameter ratio |

##### Implementations

###### Methods

- ```rust
  pub fn get_nusselt_for_developing_flow_bulk_fluid_prandtl(self: &Self) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  Gnielinski correlation but for developing flows

- ```rust
  pub fn get_nusselt_for_developing_flow(self: &Self) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  Gnielinski correlation but for developing flows

- ```rust
  pub fn get_nusselt_for_custom_developing_flow_prandtl_film(self: &Self, correlation_coefficient_c: Ratio, reynolds_exponent_m: f64) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  Custom Gnielinski correlation but for developing flows

- ```rust
  pub fn get_nusselt_for_custom_developing_flow_prandtl_bulk(self: &Self, correlation_coefficient_c: Ratio, reynolds_exponent_m: f64) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  Custom Gnielinski correlation but for developing flows

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> GnielinskiData { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GnielinskiData) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `enums`

contains nusselt number enums used for input calculation

```rust
pub mod enums { /* ... */ }
```

### Types

#### Enum `NusseltCorrelation`

Contains a collection of nusselt number correlations for use

still under experimentation, so quite unstable

for now, you are supposed to construct a struct containing Pr and Re
and etc and fit them into the enum,

use the get method to obtain the Nusselt Number

```rust
pub enum NusseltCorrelation {
    PipeGnielinskiGeneric(super::input_structs::GnielinskiData),
    PipeGnielinskiCalibrated(super::input_structs::GnielinskiData, Ratio),
    PipeGnielinskiGenericPrandtlFilm(super::input_structs::GnielinskiData),
    CustomGnielinskiGenericPrandtlFilm(super::input_structs::GnielinskiData, Ratio, f64),
    CustomGnielinskiGenericPrandtlBulk(super::input_structs::GnielinskiData, Ratio, f64),
    PipeGnielinskiTurbulentPrandtlBulk(super::input_structs::GnielinskiData),
    Wakao(super::input_structs::WakaoData),
    ReynoldsPrandtl(super::input_structs::NusseltPrandtlReynoldsData),
    PipeConstantHeatFluxFullyDeveloped,
    PipeConstantTemperatureFullyDeveloped,
    CIETHeaterVersion2(super::input_structs::NusseltPrandtlReynoldsData),
    IdealNusseltOneBillion,
    FixedNusselt(Ratio),
}
```

##### Variants

###### `PipeGnielinskiGeneric`

pipe nusselt number using Gnielinski Correlation
for laminar, turbulent and transition region

laminar flow assumes constant heat flux

For this correlation, two prandtl numbers are used for Nusselt number
estimation
Pr_bulk and Pr_wall

of course, you may use your own Pr_film instead of Pr_bulk
and obtain your Nusselt number based on Pr_film, but the
correction factor will become

(Pr_film/Pr_wall)^0.11

for more fine grained control, please use another enum


Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::GnielinskiData` |  |

###### `PipeGnielinskiCalibrated`

calibrated
pipe nusselt number using Gnielinski Correlation
for laminar, turbulent and transition region. Allows you to
insert a multiplicative ratio to calibrate the Gnielinski
correlation

laminar flow assumes constant heat flux

For this correlation, two prandtl numbers are used for Nusselt number
estimation
Pr_bulk and Pr_wall

of course, you may use your own Pr_film instead of Pr_bulk
and obtain your Nusselt number based on Pr_film, but the
correction factor will become

(Pr_film/Pr_wall)^0.11

for more fine grained control, please use another enum

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::GnielinskiData` |  |
| 1 | `Ratio` |  |

###### `PipeGnielinskiGenericPrandtlFilm`

pipe nusselt number using Gnielinski Correlation
for laminar, turbulent and transition region

laminar flow assumes constant heat flux

For this correlation, three prandtl numbers are
used for Nusselt number estimation
Pr_bulk, Pr_film and Pr_wall

Pr_film is used for Nusselt number estimation in all regimes,
but Pr_bulk and Pr_wall are used only for the correction
factor in the turbulent and transition regime

Now, in the GnielinskiData object,
only the Pr_bulk and Pr_wall are provided
so Pr_film is estimated using

Pr_film = (Pr_bulk + Pr_wall)/2

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::GnielinskiData` |  |

###### `CustomGnielinskiGenericPrandtlFilm`

pipe nusselt number using custom Gnielinski correlation
for laminar, turbulent and transition region

laminar flow assumes constant heat flux  (Nu = 4.354)

Correlation be like:
Nu = C (Re^m - 280.0) Pr_film^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25
User must supply C and m

For low Re flows, Nu = 4.36 is used.
The transition regime is around Re = 40-100
This was totally random and arbitrary assuming that low Re
results in turbulent transition so to speak,
THESE MAY NOT BE APPLICABLE IN THIS CASE, so be careful

film prandtl numbers are used in this equation where
Pr_film = (Pr_bulk + Pr_wall)/2

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::GnielinskiData` |  |
| 1 | `Ratio` |  |
| 2 | `f64` |  |

###### `CustomGnielinskiGenericPrandtlBulk`

pipe nusselt number using custom Gnielinski correlation
for laminar, turbulent and transition region

laminar flow assumes constant heat flux  (Nu = 4.354)

Correlation be like:
Nu = C (Re^m - 280.0) Pr_f^0.4 ( 1.0 + (D_e/l)^(2/3) ) ( Pr_f / Pr_w )^0.25
User must supply C and m

For low Re flows, Nu = 4.36 is used.
The transition regime is around Re = 40-100
This was totally random and arbitrary assuming that low Re
results in turbulent transition so to speak,
THESE MAY NOT BE APPLICABLE IN THIS CASE, so be careful


Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::GnielinskiData` |  |
| 1 | `Ratio` |  |
| 2 | `f64` |  |

###### `PipeGnielinskiTurbulentPrandtlBulk`

nusselt number only for turbulent
flow in pipes
For this correlation, two prandtl numbers are used for Nusselt number
estimation
Pr_bulk and Pr_wall

of course, you may use your own Pr_film instead of Pr_bulk
and obtain your Nusselt number based on Pr_film, but the
correction factor will become

(Pr_film/Pr_wall)^0.11

for more fine grained control, please use another enum

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::GnielinskiData` |  |

###### `Wakao`

nusselt number for porous media
especially packed beds
based on Wakao Correlation:

Nu = 2 + 1.1 * Pr^(1/3) * Re^0.6

valid for roughly 15 <= Re <= 8500

note: both Nusselt and Reynolds numbers are based on the
particle (pebble) diameter, and Reynolds uses the superficial
velocity. See [`WakaoData::get`] for the full documentation,
including the note that this correlation had its Reynolds and
Prandtl exponents transposed before 2026-08-11 (bead `op-4542`).

Wakao, N., Kaguei, S., & Funazkri, T. (1979). Effect of fluid
dispersion coefficients on particle-to-fluid heat transfer
coefficients in packed beds: correlation of Nusselt numbers.
Chemical Engineering Science, 34(3), 325-336.
DOI: 10.1016/0009-2509(79)85064-2

Wakao, N., & Funazkri, T. (1978). Effect
of fluid dispersion coefficients on particle-to-fluid mass
transfer coefficients in packed beds: correlation of
Sherwood numbers. Chemical Engineering Science, 33(10), 1375-1384.

only one prandtl number is required here, so you can use
bulk fluid prandtl number or film prandtl number as you wish

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::WakaoData` |  |

###### `ReynoldsPrandtl`

generic reynolds prandtl power correlation
usually in the form:

Nu = a + b * Re^c * Pr^d (Pr/Pr_wall)^e

a is the constant
b is the reynolds_prandtl_coefficient
c is the reynolds_power,
d is the prandtl_power,
e is the prandtl_correction_factor_power


For this correlation, two prandtl numbers are used for Nusselt number
estimation
Pr and Pr_wall

for Pr, you may use your own Pr_film instead of Pr_bulk
and obtain your Nusselt number

just beware that if you use Pr_film, the correction factor becomes
(Pr_film/Pr_wall)^0.11
and if you use Pr_bulk, the correction factor becomes
(Pr_bulk/Pr_wall)^0.11

for more fine grained control, please use another enum

only one reynolds number is given, so it is up to you what
reynolds number you want to supply

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::NusseltPrandtlReynoldsData` |  |

###### `PipeConstantHeatFluxFullyDeveloped`

returns a nusselt number of 4.36 for fully developed
constant heat flux flow

###### `PipeConstantTemperatureFullyDeveloped`

returns a nusselt number of 3.66 for fully developed
constant temperature flow

###### `CIETHeaterVersion2`

ciet heater correlation for version 2,

Nu = 0.04179 * reynolds^0.836 * Pr_bulk^0.333
* (Pr_bulk/Pr_wall)^0.11

for Pr_bulk, you may use your own Pr_film instead of Pr_bulk
and obtain your Nusselt number

just beware that if you use Pr_film, the correction factor becomes
(Pr_film/Pr_wall)^0.11
and if you use Pr_bulk, the correction factor becomes
(Pr_bulk/Pr_wall)^0.11

for more fine grained control, please use another enum

or you may choose to ignore the correction factor completely
as I did in my dissertation

Ong, T. K. C. (2024). Digital Twins as Testbeds for
Iterative Simulated Neutronics Feedback Controller
Development (Doctoral dissertation, UC Berkeley).

only one reynolds number is given, so it is up to you what
reynolds number you want to supply

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::input_structs::NusseltPrandtlReynoldsData` |  |

###### `IdealNusseltOneBillion`

Ideal 1e9
Just returns a Nusselt number of 10^9
which may be suitable as an approximation for heat exchangers

###### `FixedNusselt`

Fixed nusselt number,

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Ratio` |  |

##### Implementations

###### Methods

- ```rust
  pub fn try_get_nusselt(self: &Self) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets the nusselt based on user choice of of correlation

- ```rust
  pub fn estimate_based_on_prandtl_darcy_and_reynolds_no_wall_correction(self: &Self, bulk_prandtl_number_input: Ratio, darcy_friction_factor: Ratio, reynolds_number_input: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets an estimate for nusselt number based on friction factor,

- ```rust
  pub fn estimate_based_on_prandtl_and_reynolds_no_wall_correction(self: &Self, bulk_prandtl_number_input: Ratio, reynolds_number_input: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets an estimate for the nusselt number based on user choice

- ```rust
  pub fn estimate_based_on_prandtl_reynolds_and_wall_correction(self: &Self, bulk_prandtl_number_input: Ratio, wall_prandtl_number_input: Ratio, reynolds_number_input: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets an estimate for the nusselt number based on user choice

- ```rust
  pub fn estimate_based_on_prandtl_darcy_and_reynolds_wall_correction(self: &Self, bulk_prandtl_number_input: Ratio, wall_prandtl_number_input: Ratio, darcy_friction_factor: Ratio, reynolds_number_input: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets an estimate for nusselt number based on friction factor,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NusseltCorrelation { /* ... */ }
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
    fn default() -> NusseltCorrelation { /* ... */ }
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
    fn eq(self: &Self, other: &NusseltCorrelation) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `tests`

tests to ensure correlations are working correctly

```rust
pub mod tests { /* ... */ }
```

## Module `thermal_resistance`

basic calculation functions for thermal resistance
this module  contains functions for thermal resistance


will probably need to cite later, [to be done]
take fourier's law for example

heat flux = - k dT/dx
Q/A  = -k dT/dx

the thermal resistance here is the ratio of
the driving force (dT) to the heat flow (Q)

in non differential form,
Q/A = -k (Delta T)/(Delta x)

-(Delta T)/Q = (Delta x)/(kA)

and for convection

heat flux (surface to fluid) = - h (T_fluid - T_surface)

Q/A = - (Delta T) h

(Delta T)/Q = 1/(hA)

The unit for thermal resistance here is kelvin per watt

For all intents and purposes however,
we want to find the heat transfer given a set temperature difference
and properties of the pipe and etc

hence, the output of the functions here will usually be power
given various inputs



```rust
pub mod thermal_resistance { /* ... */ }
```

### Functions

#### Function `subtract_two_thermodynamic_temperatures`

subtracts two thermodynamic temperatures from each
other to obtain a temperature interval

two values are supplied, t1 and t2

So i'm going to subtract 83F from 600K
83 F is 301.5 K approximately


```rust
extern crate approx;

use uom::si::{temperature_interval, thermodynamic_temperature};
use uom::si::f64::*;
use tuas_boussinesq_solver::heat_transfer_correlations::
thermal_resistance::subtract_two_thermodynamic_temperatures;

let t1 = ThermodynamicTemperature::new::
<thermodynamic_temperature::kelvin>(600_f64);

let t2 = ThermodynamicTemperature::new::
<thermodynamic_temperature::degree_fahrenheit>(83_f64);

let expected_temp_value = t1.value - t2.value;

let test_temp = subtract_two_thermodynamic_temperatures(
t1,t2);
approx::assert_relative_eq!(expected_temp_value, test_temp.value,
max_relative=0.001);

```




```rust
pub fn subtract_two_thermodynamic_temperatures(t1: ThermodynamicTemperature, t2: ThermodynamicTemperature) -> TemperatureInterval { /* ... */ }
```

#### Function `obtain_power_two_convection_two_conduction_thermal_resistance`

calcualtes heat flow using a thermal resistance model,
Q/A = - (Delta T) h

thermal resistance is:
(Delta T)/Q = 1/(hA)

we assume there are two convection thermal resistances to worry about
useful if we have a two fluid flow through
a single layer with some thermal resistance

useful if we have some pipe and insulation
and we have hot fluid in the pipe and we want to calculate heat loss
to the external environment

```rust
pub fn obtain_power_two_convection_two_conduction_thermal_resistance(temperature_of_heat_recipient: ThermodynamicTemperature, temperature_of_heat_source: ThermodynamicTemperature, average_surface_area_1: Area, heat_transfer_coefficient_1: HeatTransfer, average_surface_area_2: Area, heat_transfer_coefficient_2: HeatTransfer, average_thermal_conductivity_layer_1: ThermalConductivity, average_wall_surface_area_1: Area, length_of_wall_1: Length, average_thermal_conductivity_layer_2: ThermalConductivity, average_wall_surface_area_2: Area, length_of_wall_2: Length) -> Power { /* ... */ }
```

#### Function `obtain_power_one_convection_one_conduction_thermal_resistance`

calcualtes heat flow using a thermal resistance model,
Q/A = - (Delta T) h

thermal resistance is:
(Delta T)/Q = 1/(hA)

we assume there are one convection thermal resistances to worry about
useful if we want to place a thermal mass inside of the
thermal resistances

or if we want to calculate the maximum temperature of a heated pebble
or cylinder or block
a single layer with some thermal resistance

```rust
pub fn obtain_power_one_convection_one_conduction_thermal_resistance(temperature_of_heat_recipient: ThermodynamicTemperature, temperature_of_heat_source: ThermodynamicTemperature, average_surface_area_1: Area, heat_transfer_coefficient_1: HeatTransfer, average_thermal_conductivity: ThermalConductivity, average_surface_area: Area, length_of_wall: Length) -> Power { /* ... */ }
```

#### Function `obtain_power_two_convection_one_conduction_thermal_resistance`

calcualtes heat flow using a thermal resistance model,
Q/A = - (Delta T) h

thermal resistance is:
(Delta T)/Q = 1/(hA)

we assume there are two convection thermal resistances to worry about
useful if we have a two fluid flow through
a single layer with some thermal resistance

```rust
pub fn obtain_power_two_convection_one_conduction_thermal_resistance(temperature_of_heat_recipient: ThermodynamicTemperature, temperature_of_heat_source: ThermodynamicTemperature, average_surface_area_1: Area, heat_transfer_coefficient_1: HeatTransfer, average_surface_area_2: Area, heat_transfer_coefficient_2: HeatTransfer, average_thermal_conductivity: ThermalConductivity, average_surface_area_thermal_cond: Area, length_of_wall: Length) -> Power { /* ... */ }
```

#### Function `obtain_power_through_double_convection_thermal_resistance`

calcualtes heat flow using a thermal resistance model,
Q/A = - (Delta T) h

thermal resistance is:
(Delta T)/Q = 1/(hA)

we assume there are two convection thermal resistances to worry about
useful if we have a two fluid flow througha  diathermal wall

```rust
pub fn obtain_power_through_double_convection_thermal_resistance(temperature_of_heat_recipient: ThermodynamicTemperature, temperature_of_heat_source: ThermodynamicTemperature, average_surface_area_1: Area, heat_transfer_coefficient_1: HeatTransfer, average_surface_area_2: Area, heat_transfer_coefficient_2: HeatTransfer) -> Power { /* ... */ }
```

#### Function `obtain_power_through_single_convection_thermal_resistance`

calcualtes heat flow using a thermal resistance model,
Q/A = - (Delta T) h

thermal resistance is:
(Delta T)/Q = 1/(hA)

```rust
pub fn obtain_power_through_single_convection_thermal_resistance(temperature_of_heat_recipient: ThermodynamicTemperature, temperature_of_heat_source: ThermodynamicTemperature, average_surface_area: Area, heat_transfer_coefficient: HeatTransfer) -> Power { /* ... */ }
```

#### Function `obtain_power_through_two_layer_wall_thermal_resistance`

calcualtes heat flow using a thermal resistance model,
-(Delta T)/Q = (Delta x)/(kA)

assumes there are two layers in the 1D system

```rust
pub fn obtain_power_through_two_layer_wall_thermal_resistance(temperature_of_heat_recipient: ThermodynamicTemperature, temperature_of_heat_source: ThermodynamicTemperature, average_thermal_conductivity_layer_1: ThermalConductivity, average_thermal_conductivity_layer_2: ThermalConductivity, average_surface_area_1: Area, average_surface_area_2: Area, length_of_wall_1: Length, length_of_wall_2: Length) -> Power { /* ... */ }
```

#### Function `obtain_power_through_wall_thermal_resistance`

calcualtes heat flow using a thermal resistance model,
-(Delta T)/Q = (Delta x)/(kA)

```rust
pub fn obtain_power_through_wall_thermal_resistance(temperature_of_heat_recipient: ThermodynamicTemperature, temperature_of_heat_source: ThermodynamicTemperature, average_thermal_conductivity: ThermalConductivity, average_surface_area: Area, length_of_wall: Length) -> Power { /* ... */ }
```

#### Function `try_get_thermal_conductance_annular_cylinder`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

thermal resistance for cylindrical annular region
the String Error is temporary, probably need to refine in
later editions with a proper error type
// <https://web2.clarkson.edu/projects/subramanian/ch330/notes/Conduction%20in%20the%20Cylindrical%20Geometry.pdf>

Thermal conductance is just inverse of thermal resistance
it is
(2 * pi * L * K)/
ln(outer_radius/inner_radius)




```rust
pub fn try_get_thermal_conductance_annular_cylinder(inner_diameter: Length, outer_diameter: Length, cylinder_length: Length, k: ThermalConductivity) -> Result<ThermalConductance, String> { /* ... */ }
```

## Module `view_factors`

view factor functions for radiative heat transfer
Radiation view factors for enclosure heat-transfer geometries.

A view factor `F_(i-j)` is the dimensionless fraction (0 to 1) of
radiation leaving surface `i` that is intercepted by surface `j`. The
functions here compute the analytical view factors for a pair of coaxial
(concentric) cylinders and the annular end rings between them, for use in
the clamshell radiative heater model. Currently only the concentric
cylinder geometry is implemented; new enclosure geometries belong in their
own submodule here.

```rust
pub mod view_factors { /* ... */ }
```

### Modules

## Module `cocentric_cylinders`

this module contains functions for calculating view factors for
cocentric cylinders. This is for the clamshell radiative heater
still under construction

However, the view factors themselves have been tested to check
if they add up to one

```rust
pub mod cocentric_cylinders { /* ... */ }
```

### Functions

#### Function `outer_cylinder_self_view_factor`

F_(2-2) =  1 - 1/R - ((H^2 + 4 R^2)^0.5 -H)/(4 R) + 1/PI * B


Where B = 2/R atan (2 * sqrt(R^2 - 1)/H) - H/(2 R) * C

And C = D asin(E) - asin (F)

D = sqrt(4 R^2 + H^2)/H
E = (H^2 + 4(R^2 - 1) - 2 H^2/R^2)/(H^2  + 4 (R^2 - 1))

F = (R^2 - 2)/R^2

formula inspected ok 8:34pm 06 nov

```rust
pub fn outer_cylinder_self_view_factor(inner_diameter: Length, outer_diameter: Length, cylinder_height: Length) -> Ratio { /* ... */ }
```

#### Function `outer_cylinder_to_inner_cylinder_view_factor`

F_(2-1) = 1/R * ( 1 - B - 1/PI C )

C = D - E - F

D = acos (hsq_minus_rsq_plus_one/hsq_plus_rsq_minus_one)
E = e1 acos (e2)

e1 = sqrt( hsq_plus_rsq_plus_one^2 - 4.0 * rsq)/(2H)
e2 = (hsq_minus_rsq_plus_one)/(R * hsq_plus_rsq_minus_one)


hsq_minus_rsq_plus_one = H^2 - R^2 + 1
hsq_plus_rsq_minus_one = H^2 + R^2 - 1
hsq_plus_rsq_plus_one = H^2 + R^2 + 1

F = (hsq_minus_rsq_plus_one)/(2H) asin (1/R)

B = (hsq_plus_rsq_minus_one)/(4 H)


outer cylinder to inner cylinder view factor

visually inspected 8:39pm 06 nov 2024

```rust
pub fn outer_cylinder_to_inner_cylinder_view_factor(inner_diameter: Length, outer_diameter: Length, cylinder_height: Length) -> Ratio { /* ... */ }
```

#### Function `outer_cylinder_to_annular_end_ring_view_factor`

outer cylinder to annular end ring (enclosing space between
coaxial cylinders)


terms:
r_1 = inner_radius
r_2 = outer_radius

H = cylinder_height/r_2
R = r_1/r_2

X = sqrt(1-R^2)
Y = (R(1 - R^2 - H^2))/(1 - R^2 + H^2)

F_(1-2) = 1/PI * (A + B + C - D + E)


A =  R (atan(X/H) -  atan(2X/H))

B = H/4 * ( asin(2R^2 - 1) - asin (R))

C = X^2/(4 H) * (PI/2 + asin(R))

D = d1 * d2

d1 = sqrt( (1 + R^2 + H^2)^2 -  4 R^2) / (4H)
d2 = PI/2 + asin(Y)

E = e1 * e2

e1 = sqrt (4 + H^2)/4
e2 = PI/2 + asin(1 -  2 R^2 H^2 / (4 X^2 + H^2))

```rust
pub fn outer_cylinder_to_annular_end_ring_view_factor(inner_diameter: Length, outer_diameter: Length, cylinder_height: Length) -> Ratio { /* ... */ }
```

#### Function `inner_cylinder_to_outer_cylinder_view_factor`

using view factor algebra, compute inner cylinder to outer cylinder
view factor

A_inner (F_inner to outer) =  A_outer (F_outer to inner)

A_outer/A_inner = (PI D L)_outer/(PI D L)_inner

```rust
pub fn inner_cylinder_to_outer_cylinder_view_factor(inner_diameter: Length, outer_diameter: Length, cylinder_height: Length) -> Ratio { /* ... */ }
```

#### Function `inner_cylinder_to_annular_end_ring_view_factor`

inner surface cylinder to annular end


F(1-2) = V + 1/(2 pi) * [W - X * Y - Z]

V = B/(8RL)

W = acos(A/B)

X = 1/(2L) sqrt( (A+2)^2/R^2 - 4)

Y = acos( A * R / B)

Z = A/(2 R L) * asin(R)


F(1-2) A1 = F(2-1) A2



```rust
pub fn inner_cylinder_to_annular_end_ring_view_factor(inner_diameter: Length, outer_diameter: Length, cylinder_height: Length) -> Ratio { /* ... */ }
```

## Module `parallel_heat_exchangers`

calculations for parallel piped heat exchangers

```rust
pub mod parallel_heat_exchangers { /* ... */ }
```

### Functions

#### Function `log_mean_temperature_difference`

LMTD = (delta T in - delta T out) / (ln delta T in - ln delta T out)

note that reversing the order of delta T in and out doesn't really
matter, as long as both numerator and denominator are reversed
correctly

However, hot fluid temperatures and cold fluid temperature CANNOT
be mixed up, otherwise the logarithms will return an error

```rust
extern crate approx;
use tuas_boussinesq_solver::heat_transfer_correlations::
parallel_heat_exchangers::log_mean_temperature_difference;


use uom::si::{temperature_interval, thermodynamic_temperature};
use uom::si::f64::*;

let cold_fluid_temp_A = ThermodynamicTemperature::new::
<thermodynamic_temperature::degree_celsius>(21.0);

let cold_fluid_temp_B = ThermodynamicTemperature::new::
<thermodynamic_temperature::degree_celsius>(20.0);

let hot_fluid_temp_A = ThermodynamicTemperature::new::
<thermodynamic_temperature::degree_celsius>(48.0);

let hot_fluid_temp_B = ThermodynamicTemperature::new::
<thermodynamic_temperature::degree_celsius>(50.0);

let A_temperature_interval_value : f64 = hot_fluid_temp_A.value -
cold_fluid_temp_A.value;

let B_temperature_interval_value : f64 = hot_fluid_temp_B.value -
cold_fluid_temp_B.value;

let mut LMTD_value_expected =
(A_temperature_interval_value - B_temperature_interval_value)/
(A_temperature_interval_value.ln() -
B_temperature_interval_value.ln());

let LMTD_test = log_mean_temperature_difference(
cold_fluid_temp_A,
cold_fluid_temp_B,
hot_fluid_temp_A,
hot_fluid_temp_B).unwrap();


approx::assert_relative_eq!(LMTD_value_expected, LMTD_test.value,
max_relative=0.001);

// test 2 makes it more obvious

let mut LMTD_value_expected =
((48_f64 - 21_f64) - (50_f64-20.0))/
((48_f64-21_f64).ln() -
(50_f64-20.0).ln());

approx::assert_relative_eq!(LMTD_value_expected, LMTD_test.value,
max_relative=0.001);

```

```rust
pub fn log_mean_temperature_difference(temp_cold_fluid_a: ThermodynamicTemperature, temp_cold_fluid_b: ThermodynamicTemperature, temp_hot_fluid_a: ThermodynamicTemperature, temp_hot_fluid_b: ThermodynamicTemperature) -> Result<TemperatureInterval, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_lmtd_heat_flux_based_on_ambient_temp`

calculate overall heat flux power input based on lmtd

assuming a fixed surrounding temperature

calculates heat INPUT into fluid based on surrounding temperature
estimated fluid inlet and fluid outlet temperature

Q = U * A * LMTD

LMTD = (delta T in - delta T out) / (ln delta T in - ln delta T out)

U is overall_heat_transfer_coeff

A is the surface_area
The surface_area you use can be the surface area of the inner
or outer region of the pipe. BUT, the overall_heat_transfer_coeff
must be adjusted accordingly



```rust
pub fn calculate_lmtd_heat_flux_based_on_ambient_temp(overall_heat_transfer_coeff: HeatTransfer, ambient_temperature: ThermodynamicTemperature, fluid_temperature_in: ThermodynamicTemperature, fluid_temperature_out: ThermodynamicTemperature, surface_area: Area) -> Result<Power, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `heat_transfer_interactions`

heat transfer interactions between different shapes
of control volumes are calculated here
Heat-transfer interactions between two control volumes.

A heat-transfer interaction pairs a geometry (slab, cylinder, or sphere)
with a physical mechanism (conduction, convection, advection, or
radiation) and produces either a thermal conductance (watts per kelvin) or
a heat flow (watts) between the two control volumes.

Submodules:

- [`conductance`] — solid-solid and solid-fluid conduction/convection
  conductances (watts per kelvin), plus the radiation conductance helper.
- [`advection`] — fluid-fluid enthalpy transport (watts) driven by mass
  flow between control volumes.
- [`heat_transfer_geometry`] — enums/structs describing the geometry of an
  interaction (curved-surface fluid/solid arrangement, Cartesian layers).
- [`heat_transfer_interaction_enums`] — the [`HeatTransferInteractionType`]
  enum enumerating every supported interaction and its `(p, T)` → thermal
  conductance dispatch.

[`HeatTransferInteractionType`]: heat_transfer_interaction_enums::HeatTransferInteractionType

```rust
pub mod heat_transfer_interactions { /* ... */ }
```

### Modules

## Module `conductance`

for solid-solid and solid-fluid interaction, heat transfer can
be expressed in terms of thermal conductance or thermal
resistance

this module contains functions for these

```rust
pub mod conductance { /* ... */ }
```

### Functions

#### Function `get_conductance_single_cartesian_one_dimension`

Suppose we have two control volumes of the same materials and  
temperature and we put a 1D thermal resistance between them

we would need to return a thermal conductance based on a 1D
heat transfer model

conductance is watts per kelvin or
q = (kA)/dx * dT
conductance here is kA/dx
thermal resistance is 1/conductance

For a 1D case, the area is not defined, but I'm giving it a unit
area value of 1 meter squared specific to 1D calculations

Note that the control volume MUST have the same cross sectional
area so that it is consistent. Will need to be a 1D control
volume of sorts


```rust
pub fn get_conductance_single_cartesian_one_dimension(material: crate::boussinesq_thermophysical_properties::Material, material_temperature_1: ThermodynamicTemperature, material_temperature_2: ThermodynamicTemperature, material_pressure_1: Pressure, material_pressure_2: Pressure, thickness: XThicknessThermalConduction) -> Result<ThermalConductance, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_conductance_dual_cartesian_three_dimensions`

Suppose we have two control volumes of differing materials and  
temperature and we put a thermal resistance between them
in the x coordinate

we would need to return a thermal conductance based on a 1D
heat transfer model

conductance is watts per kelvin or
q = (kA)/dx * dT
conductance here is kA/dx
thermal resistance is 1/conductance




```rust
pub fn get_conductance_dual_cartesian_three_dimensions(material_1: crate::boussinesq_thermophysical_properties::Material, material_2: crate::boussinesq_thermophysical_properties::Material, material_temperature_1: ThermodynamicTemperature, material_temperature_2: ThermodynamicTemperature, material_pressure_1: Pressure, material_pressure_2: Pressure, xs_area: CrossSectionalArea, thickness_1: XThicknessThermalConduction, thickness_2: XThicknessThermalConduction) -> Result<ThermalConductance, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_conductance_cylindrical_radial_two_materials`

Suppose we have two control volumes of differing materials and  
temperature and we put a thermal resistance between them
in the cylindrical radial coordinate

we would need to return a thermal conductance based on a 1D
heat transfer model in the r coordinate


Now, it is important also to specify which control volume is
adjacent to the
the inner radius and which one is at the outer radius



```rust
pub fn get_conductance_cylindrical_radial_two_materials(material_inner_shell: crate::boussinesq_thermophysical_properties::Material, material_outer_shell: crate::boussinesq_thermophysical_properties::Material, material_temperature_inner_shell: ThermodynamicTemperature, material_temperature_outer_shell: ThermodynamicTemperature, material_pressure_inner_shell: Pressure, material_pressure_outer_shell: Pressure, id: InnerDiameterThermalConduction, inner_shell_thickness: RadialCylindricalThicknessThermalConduction, outer_shell_thickness: RadialCylindricalThicknessThermalConduction, l: CylinderLengthThermalConduction) -> Result<ThermalConductance, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `get_conductance_single_cylindrical_radial_solid_liquid`

Suppose we have two control volumes of differing materials and  
temperature

one control volume is a solid and the other is a fluid

now, we want to calculate thermal resistance between them

for fluids, the thermal resistance or conductance is quite
straightforward

from fluid to solid heat transfer,
Q = -hA (T_solid - T_fluid)

conductance here is hA (watts per kelvin); the resistance is 1/(hA)
where A is the curved surface area pi*D*L

for solid thermal resistance, we use the
obtain_thermal_conductance_annular_cylinder
function under common functions

that would need an inner diameter and an outer diameter

There are two cases here.

Firstly,
the fluid is an in the tube side of a heat exchanger or pipe,
hence the solid is considered on the outside

The surface area will be based on the inner diameter

Secondly, the fluid is on the outside of the cylindrical solid,
in this case, the surface area will be based on the outer diameter

you tell the solver which is which using an enum
CylindricalAndSphericalSolidFluidArrangement



```rust
pub fn get_conductance_single_cylindrical_radial_solid_liquid(solid: crate::boussinesq_thermophysical_properties::Material, solid_temperature: ThermodynamicTemperature, solid_pressure: Pressure, h: HeatTransfer, id: InnerDiameterThermalConduction, od: OuterDiameterThermalConduction, l: CylinderLengthThermalConduction, solid_liquid_arrangement: super::heat_transfer_geometry::CylindricalAndSphericalSolidFluidArrangement) -> Result<ThermalConductance, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `simple_radiation_conductance`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

H = sigma * coefficient * (T_hot^2 + T_cold^2)*(T_hot + T_cold)
where sigma is the stefan boltzmann constant
in W m^(-2) T^(-4)

the coefficient is in units of area, so provide it yourself
Stefan boltzmann constant
Modest, M. F., & Mazumder, S. (2021).
Radiative heat transfer. Academic press.
List of Symbols (page 32 of 2174)

5.670e-8 W m^(-2) K^(-4)

```rust
pub fn simple_radiation_conductance(area_coeff: Area, hot_temperature: ThermodynamicTemperature, cold_temperature: ThermodynamicTemperature) -> ThermalConductance { /* ... */ }
```

## Module `advection`

for fluid-fluid interaction, where fluid flows and carries
heat from one control volume to another, we call this advection
functions to calculate advection and placed in this module

```rust
pub mod advection { /* ... */ }
```

### Functions

#### Function `advection_heat_rate`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

now, advection is quite tricky because for conduction, the
heat transfer formula is for two control volumes cv_a and cv_b
can be as follows

(cv_a) --------------- (cv_b)
  
 T_a                    T_b

 Q_(ab) = -H(T_b - T_a)

Q_(ab) is heat transfer rate (watts) from a to b
H is conductance, not heat transfer coefficient
it has units of watts kelvin

For advection in contrast, it depends on flow

(cv_a) --------------- (cv_b)
  
 T_a                    T_b

For flow from a to b:
Q_(ab) = m h(T_a)

For flow from b to a
Q_(ab) = -m h(T_b)

Here, the enthalpy transfer only depends on one of the body's
temperature, which is directly dependent on mass flow

```rust
pub fn advection_heat_rate(mass_flow_from_a_to_b: MassRate, specific_enthalpy_of_a: AvailableEnergy, specific_enthalpy_of_b: AvailableEnergy) -> Result<Power, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `heat_transfer_geometry`

for heat transfer interactions between two control volumes,
there will be a certain geometry, this is usually a cylinder,
sphere or slab (straight line)

The enums and structs responsible for handling this information are
stored here

```rust
pub mod heat_transfer_geometry { /* ... */ }
```

### Types

#### Enum `CylindricalAndSphericalSolidFluidArrangement`

for a curved surface, be it cylindrical or spherical,
this enum indicates whether the fluid is on the inside (lower radius)
or on the outside (larger radius)

-----------------------------------------> r
fluid               ||                  solid


```rust
pub enum CylindricalAndSphericalSolidFluidArrangement {
    FluidOnInnerSurfaceOfSolidShell,
    FluidOnOuterSurfaceOfSolidShell,
}
```

##### Variants

###### `FluidOnInnerSurfaceOfSolidShell`

indicates that fluid in the inner side of a curved shell

-----------------------------------------> r
fluid               ||                  solid


###### `FluidOnOuterSurfaceOfSolidShell`

indicates that fluid is on the outer side (larger radius) of a
curved shell

-----------------------------------------> r
solid               ||                  fluid


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
    fn clone(self: &Self) -> CylindricalAndSphericalSolidFluidArrangement { /* ... */ }
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
    fn eq(self: &Self, other: &CylindricalAndSphericalSolidFluidArrangement) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `DataDualCartesianThermalConductanceThreeDimension`

here we have a struct for dual Cartesian Thermal conduction
in three dimensions
on

```rust
pub struct DataDualCartesianThermalConductanceThreeDimension {
    pub material_1: crate::boussinesq_thermophysical_properties::Material,
    pub material_2: crate::boussinesq_thermophysical_properties::Material,
    pub xs_area: CrossSectionalArea,
    pub thickness_1: XThicknessThermalConduction,
    pub thickness_2: XThicknessThermalConduction,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `material_1` | `crate::boussinesq_thermophysical_properties::Material` | material for first cv |
| `material_2` | `crate::boussinesq_thermophysical_properties::Material` | material for second cv |
| `xs_area` | `CrossSectionalArea` | cross sectional area at interface |
| `thickness_1` | `XThicknessThermalConduction` | thickness of first cv |
| `thickness_2` | `XThicknessThermalConduction` | thickness of second cv |

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
    fn clone(self: &Self) -> DataDualCartesianThermalConductanceThreeDimension { /* ... */ }
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
    fn eq(self: &Self, other: &DataDualCartesianThermalConductanceThreeDimension) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `heat_transfer_interaction_enums`

for heat transfer interactions, there are various types,
for example, it could be advection, conduction or a simple fixed
heat addition

these are represented in heat transfer enums which are stored here

```rust
pub mod heat_transfer_interaction_enums { /* ... */ }
```

### Types

#### Enum `HeatTransferInteractionType`

Contains possible heat transfer interactions between the nodes

```rust
pub enum HeatTransferInteractionType {
    UserSpecifiedThermalConductance(ThermalConductance),
    SingleCartesianThermalConductanceOneDimension(crate::boussinesq_thermophysical_properties::Material, XThicknessThermalConduction),
    DualCartesianThermalConductanceThreeDimension(DataDualCartesianThermalConductanceThreeDimension),
    DualCartesianThermalConductance((crate::boussinesq_thermophysical_properties::Material, XThicknessThermalConduction), (crate::boussinesq_thermophysical_properties::Material, XThicknessThermalConduction)),
    DualCylindricalThermalConductance((crate::boussinesq_thermophysical_properties::Material, RadialCylindricalThicknessThermalConduction), (crate::boussinesq_thermophysical_properties::Material, RadialCylindricalThicknessThermalConduction), (InnerDiameterThermalConduction, OuterDiameterThermalConduction, CylinderLengthThermalConduction)),
    CylindricalConductionConvectionLiquidOutside((crate::boussinesq_thermophysical_properties::Material, RadialCylindricalThicknessThermalConduction, ThermodynamicTemperature, Pressure), (HeatTransfer, OuterDiameterThermalConduction, CylinderLengthThermalConduction)),
    CylindricalConductionConvectionLiquidInside((crate::boussinesq_thermophysical_properties::Material, RadialCylindricalThicknessThermalConduction, ThermodynamicTemperature, Pressure), (HeatTransfer, InnerDiameterThermalConduction, CylinderLengthThermalConduction)),
    UserSpecifiedHeatAddition,
    UserSpecifiedHeatFluxCustomArea(Area),
    UserSpecifiedHeatFluxCylindricalOuterArea(CylinderLengthThermalConduction, OuterDiameterThermalConduction),
    UserSpecifiedHeatFluxCylindricalInnerArea(CylinderLengthThermalConduction, InnerDiameterThermalConduction),
    UserSpecifiedConvectionResistance(DataUserSpecifiedConvectionResistance),
    Advection(DataAdvection),
    SimpleRadiation(Area),
}
```

##### Variants

###### `UserSpecifiedThermalConductance`

The user specifies a thermal conductance between the nodes
in units of power/kelvin

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ThermalConductance` |  |

###### `SingleCartesianThermalConductanceOneDimension`

1D Cartesian Coordinates Thermal Resistance
We return a ThermalConductance because it's more convenient

basically have two control volumes, each node represents a control
volume

// ----------------------------
// |                          |
// *                          *
// |                          |
// ----------------------------
// cv_1                      cv_2

between them there is a thermal resistance
based on a q'' = k dT/dx

we have one material which determines conductivity
and then a length which determines the distance between
the two control volumes


Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::boussinesq_thermophysical_properties::Material` |  |
| 1 | `XThicknessThermalConduction` |  |

###### `DualCartesianThermalConductanceThreeDimension`

suppose there are two blocks with the same cross sectional
area, each of its own thickness and material makeup

this is DualCartesianThermalConductanceThreeDimension
we have three dimensional blocks, but the conduction is along
the thickness of the block, tube or cylinder

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `DataDualCartesianThermalConductanceThreeDimension` |  |

###### `DualCartesianThermalConductance`

1D Cartesian Coordinates Thermal Resistance, for solids only
We return a ThermalConductance because it's more convenient

basically have three control volumes

// -------------------------------------------------------
// |                          |                          |
// *                          *                          *
// |                          |                          |
// -------------------------------------------------------
// cv_1                      cv_2                     cv_3

between them there is a thermal resistance
based on a q'' = k dT/dx

we have two materials which determines conductivity
and then two lengths which determines the distance between
the two control volumes

Information must be passed in as a tuple,



Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `(crate::boussinesq_thermophysical_properties::Material, XThicknessThermalConduction)` |  |
| 1 | `(crate::boussinesq_thermophysical_properties::Material, XThicknessThermalConduction)` |  |

###### `DualCylindricalThermalConductance`

1D Cylindrical Coordinates Thermal Resistance
We return a ThermalConductance because it's more convenient

basically have three control volumes

// -------------------------------------------------------
// |                          |                          |
// *                          *                          *
// |                          |                          |
// -------------------------------------------------------
// cv_1                      cv_2                     cv_3

between them there is a thermal resistance
based on a q'' = k dT/dr

we have two materials which determines conductivity
and then two lengths which determines the distance between
the two control volumes

one also needs to determine the
inner diameter, outer diameter and length of the tube
  
the first material and thickness argument represents
cv_1 to cv_2 (the inner shell)

and the second entry pertains to the outer shell
cv_2 to cv_3, or the outer shell




Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `(crate::boussinesq_thermophysical_properties::Material, RadialCylindricalThicknessThermalConduction)` |  |
| 1 | `(crate::boussinesq_thermophysical_properties::Material, RadialCylindricalThicknessThermalConduction)` |  |
| 2 | `(InnerDiameterThermalConduction, OuterDiameterThermalConduction, CylinderLengthThermalConduction)` |  |

###### `CylindricalConductionConvectionLiquidOutside`

1D Cylindrical Coordinates Thermal Resistance
We return a ThermalConductance because it's more convenient

basically have three control volumes along the outer wall

-------------------------------------------------------> r
// ----------------------------
// |                          |                          
// * solid_cv_1               *                          *
// |                          |                         (T_f)
// ----------------------------
//                        solid_surface              Fluid_node

Where r is the radius
basically the liquid is on the outside (larger r)

between solid_cv_1 and the solid_surface
cv_2 there is a thermal resistance
based on a q'' = k dT/dr

between solid_surface and fluid_node, there is convection resistance
specified by a Nusselt Number so that we get a heat transfer
coefficient

For the conduction bit,
we have one material which determines conductivity
and then length which determines the distance between
the two control volumes

the thermal conductance is determined by
Thermal conductance
/// (2 * pi * L * K)/
ln(outer_radius/inner_radius)

using obtain_thermal_conductance_annular_cylinder
under common_functions


For convection, the heat flux from solid surface to fluid
is:

q = h A(T_s - T_f)

for hA
surface area is calculated by specifying an outer diameter
and a cylindrical axial length




Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `(crate::boussinesq_thermophysical_properties::Material, RadialCylindricalThicknessThermalConduction, ThermodynamicTemperature, Pressure)` |  |
| 1 | `(HeatTransfer, OuterDiameterThermalConduction, CylinderLengthThermalConduction)` |  |

###### `CylindricalConductionConvectionLiquidInside`

1D Cylindrical Coordinates Thermal Resistance
We return a ThermalConductance because it's more convenient

basically have three control volumes along the outer wall

-------------------------------------------------------> r
//                           ----------------------------
//                           |                          |                          
// *                         *         solid_cv_1       *                  
//                           |                          |                   
// fluid node                ----------------------------
// (T_f)                solid_surface

Where r is the radius
basically the liquid is on the inside (smaller r)

between solid_cv_1 and solid_surface
there is a thermal resistance
based on a q'' = k dT/dr

between solid_surface and fluid_node, there is convection resistance
specified by a Nusselt Number so that we get a heat transfer
coefficient

For the conduction bit,
we have one material which determines conductivity
and then length which determines the distance between
the two control volumes

the thermal conductance is determined by
Thermal conductance
/// (2 * pi * L * K)/
ln(outer_radius/inner_radius)

using obtain_thermal_conductance_annular_cylinder
under common_functions


For convection, the heat flux from solid surface to fluid
is:

q = h A(T_s - T_f)

for hA
surface area is calculated by specifying an outer diameter
and a cylindrical axial length




Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `(crate::boussinesq_thermophysical_properties::Material, RadialCylindricalThicknessThermalConduction, ThermodynamicTemperature, Pressure)` |  |
| 1 | `(HeatTransfer, InnerDiameterThermalConduction, CylinderLengthThermalConduction)` |  |

###### `UserSpecifiedHeatAddition`

The user Specifies a heat Addition for the BC
The uom type is Power

###### `UserSpecifiedHeatFluxCustomArea`

Use this enum to specify a constant heat flux
you will, of course, need to provide an area

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Area` |  |

###### `UserSpecifiedHeatFluxCylindricalOuterArea`

Use this enum to identify that you are
specifying a curved cylindrical surface area
on the outer surface of a cylinder

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `CylinderLengthThermalConduction` |  |
| 1 | `OuterDiameterThermalConduction` |  |

###### `UserSpecifiedHeatFluxCylindricalInnerArea`

Use this enum to identify that you are
specifying a curved cylindrical surface area
on the inner surface of a cylinder

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `CylinderLengthThermalConduction` |  |
| 1 | `InnerDiameterThermalConduction` |  |

###### `UserSpecifiedConvectionResistance`

For convection between solid and fluid,
the heat flux from solid surface to fluid
is:

q = h A(T_s - T_f)

this interaction calculates power based on a given h and A

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `DataUserSpecifiedConvectionResistance` |  |

###### `Advection`

For advection one would only specify the mass flowrate
from one control volume to another

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `DataAdvection` |  |

###### `SimpleRadiation`

for radiation heat transfer
The basic thing is that RHT power scales with
temperature

P = coefficient * (T_hot^4 - T_cold^4)

If we wanted to calculate a conductance, we would note that
P = h A (T_hot - T_cold)

h A = conductance
or we can use
H = conductance

H (T_hot - T_cold) = coefficient * (T_hot^4 - T_cold^4)

note that temperatures are necessarily in kelvin

decompose the power 4 relation
(a^2 - b^2) = (a+b)(a-b)

(T_hot^4 - T_cold^4) =
(T_hot^2 + T_cold^2)
(T_hot^2 - T_cold^2)

Decomposing again:
(T_hot^4 - T_cold^4) =
(T_hot^2 + T_cold^2)
(T_hot + T_cold)
(T_hot - T_cold)


H (T_hot - T_cold) =
coefficient *
(T_hot^2 + T_cold^2)
(T_hot + T_cold)
(T_hot - T_cold)

Therefore, conductance can be expressed as:


H = coefficient * (T_hot^2 + T_cold^2)*(T_hot + T_cold)

If one wants to be more precise with units,
then we should use:

P = sigma * coefficient * (T_hot^4 - T_cold^4)

where sigma is the stefan boltzmann constant
in W m^(-2) T^(-4)

H = sigma * coefficient * (T_hot^2 + T_cold^2)*(T_hot + T_cold)

the coefficient is in units of area, so provide it yourself
  

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Area` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new_advection_interaction(mass_flowrate: MassRate, fluid_density_heat_transfer_entity_1: MassDensity, fluid_density_heat_transfer_entity_2: MassDensity) -> Self { /* ... */ }
  ```
  constructs a new advection interaction so it's less

- ```rust
  pub fn get_thermal_conductance_based_on_interaction(self: &Self, temperature_1: ThermodynamicTemperature, temperature_2: ThermodynamicTemperature, pressure_1: Pressure, pressure_2: Pressure) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  based on the heat transfer interaction type,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> HeatTransferInteractionType { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> HeatTransferInteractionType { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeatTransferInteractionType) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(heat_transfer_interaction: HeatTransferInteractionType) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `DataUserSpecifiedConvectionResistance`

here we have a struct for simple convection resistance
in three dimensions
on

```rust
pub struct DataUserSpecifiedConvectionResistance {
    pub surf_area: SurfaceArea,
    pub heat_transfer_coeff: HeatTransfer,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `surf_area` | `SurfaceArea` | surface area for heat convection |
| `heat_transfer_coeff` | `HeatTransfer` | heat transfer coefficient in watts per square meter per kelvin |

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
    fn clone(self: &Self) -> DataUserSpecifiedConvectionResistance { /* ... */ }
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
    fn eq(self: &Self, other: &DataUserSpecifiedConvectionResistance) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `DataAdvection`

here we have a useful for necessary advection information

```rust
pub struct DataAdvection {
    pub mass_flowrate: MassRate,
    pub fluid_density_heat_transfer_entity_1: MassDensity,
    pub fluid_density_heat_transfer_entity_2: MassDensity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mass_flowrate` | `MassRate` | mass flowrate |
| `fluid_density_heat_transfer_entity_1` | `MassDensity` | fluid density of control volume on left<br><br>which means when you link control volumes or boundary<br>link(cv1, cv2, interaction)<br><br>the picture is like this<br><br>(cv1) ----> advection ---> (cv2)<br><br>cv1 is the left control volume<br>cv2 is the right control volume<br><br>now, the cv is not always a cv, it could be any heat<br>transfer entity |
| `fluid_density_heat_transfer_entity_2` | `MassDensity` | fluid density of control volume on right<br><br>which means when you link control volumes or boundary<br>link(cv1, cv2, interaction)<br><br>the picture is like this<br><br>(cv1) ----> advection ---> (cv2)<br><br>cv1 is the left control volume<br>cv2 is the right control volume<br>now, the cv is not always a cv, it could be any heat<br>transfer entity |

##### Implementations

###### Methods

- ```rust
  pub fn new_from_temperature_and_liquid_material(user_input_mass_flowrate: MassRate, fluid_material: LiquidMaterial, temperature_1: ThermodynamicTemperature, temperature_2: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructs an advection interaction by specifying

- ```rust
  pub fn new_from_heat_transfer_entity(user_input_mass_flowrate: MassRate, fluid_material: LiquidMaterial, hte_1: &mut HeatTransferEntity, hte_2: &mut HeatTransferEntity) -> Self { /* ... */ }
  ```
  constructs an advection interaction by specifying

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DataAdvection { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> HeatTransferInteractionType { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DataAdvection) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(heat_transfer_interaction: HeatTransferInteractionType) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Re-exports

#### Re-export `conductance::*`

```rust
pub use conductance::*;
```

#### Re-export `advection::*`

```rust
pub use advection::*;
```

## Module `control_volume_dimensions`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

specific dimensions for control volume construction
Geometric dimension newtypes for control-volume construction.

These are thin, `Copy` wrappers around `uom` `Length`/`Area` (SI: metres,
square metres) that give each geometric input a self-documenting name —
e.g. a wall thickness, a shell inner/outer diameter, a tube length, or a
cross-sectional/surface area. They exist so that heat-transfer interaction
enums and constructors read unambiguously and cannot silently swap, say, an
inner diameter for an outer one. Convert in and out with `From<Length>` /
`Into<Length>` (or `From<Area>` / `Into<Area>` for the area types).

```rust
pub mod control_volume_dimensions { /* ... */ }
```

### Types

#### Struct `XThicknessThermalConduction`

XThicknessThermalConduction is essentially a struct containing
one length describing a thickness in cartesian coordinates
for thermal conduction.

This type is meant for an input for various enums and functions
in this crate.

It is meant to guide the user so that they know what the
length inputs represents.

```rust

use uom::si::length::meter;
use uom::si::f64::*;
use tuas_boussinesq_solver::control_volume_dimensions
::XThicknessThermalConduction;

// let's say you have a thickness of 0.5 which you want to describe

let thickness_of_wall = Length::new::<meter>(0.5);

// we first need to convert it into an XThicknessThermalConduction
// type first
let wall_thickness_input = XThicknessThermalConduction::from(
thickness_of_wall);

// to convert it back into a length type, we use the into() method

let thickness_wall_for_calculation: Length =
wall_thickness_input.into();

// both these are the same
assert_eq!(thickness_of_wall, thickness_wall_for_calculation);
```



```rust
pub struct XThicknessThermalConduction {
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> XThicknessThermalConduction { /* ... */ }
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

  - ```rust
    fn from(thickness: Length) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> Length { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &XThicknessThermalConduction) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `RadialCylindricalThicknessThermalConduction`

This represents a thickness for radial conduction for
cylindrical shell layers

```rust
pub struct RadialCylindricalThicknessThermalConduction {
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RadialCylindricalThicknessThermalConduction { /* ... */ }
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

  - ```rust
    fn from(thickness: Length) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> Length { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RadialCylindricalThicknessThermalConduction) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `InnerDiameterThermalConduction`

This represents an inner diameter  for radial conduction
for spherical and cylindrical shell
layers

```rust
pub struct InnerDiameterThermalConduction {
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> InnerDiameterThermalConduction { /* ... */ }
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

  - ```rust
    fn from(od: Length) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> Length { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &InnerDiameterThermalConduction) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `OuterDiameterThermalConduction`

This represents an outer diameter
for radial conduction for spherical and Cylindrical shell
layers

```rust
pub struct OuterDiameterThermalConduction {
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OuterDiameterThermalConduction { /* ... */ }
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

  - ```rust
    fn from(od: Length) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> Length { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &OuterDiameterThermalConduction) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CylinderLengthThermalConduction`

This represents an tube length
ie. axial length for a cylindrical body

```rust
pub struct CylinderLengthThermalConduction {
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CylinderLengthThermalConduction { /* ... */ }
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

  - ```rust
    fn from(cylinder_length: Length) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> Length { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CylinderLengthThermalConduction) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CrossSectionalArea`

This represents an Cross Sectional Area
ie. axial length for a cylindrical body

```rust
pub struct CrossSectionalArea {
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CrossSectionalArea { /* ... */ }
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

  - ```rust
    fn from(xs_area: Area) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> Area { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CrossSectionalArea) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `SurfaceArea`

This represents an Surface Area

```rust
pub struct SurfaceArea {
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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SurfaceArea { /* ... */ }
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

  - ```rust
    fn from(surf_area: Area) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> Area { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SurfaceArea) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Constants and Statics

#### Constant `UNIT_AREA_SQ_METER_FOR_ONE_DIMENSIONAL_CALCS`

for 1D calculations, we need to calculate conductance as well,
but there is no area, hence, we have to use a unit area to calculate
the conductance

```rust
pub const UNIT_AREA_SQ_METER_FOR_ONE_DIMENSIONAL_CALCS: f64 = 1.0;
```

## Module `boundary_conditions`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

Module for boundary conditions
Thermal boundary conditions for the solver.

Defines [`BCType`], the closed set of boundary conditions a control volume
can be attached to: a fixed temperature, a fixed heat flux (per unit area),
or a fixed heat addition (power, where zero power means adiabatic). All
quantities are `uom`-typed.

```rust
pub mod boundary_conditions { /* ... */ }
```

### Types

#### Enum `BCType`

Contains all the types of Boundary Conditions (BCs) you can use

```rust
pub enum BCType {
    UserSpecifiedTemperature(ThermodynamicTemperature),
    UserSpecifiedHeatFlux(HeatFluxDensity),
    UserSpecifiedHeatAddition(Power),
}
```

##### Variants

###### `UserSpecifiedTemperature`

The user specifies a fixed temperature for the BC

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ThermodynamicTemperature` |  |

###### `UserSpecifiedHeatFlux`

The user specifies a heat flux for the BC
the uom type is heat flux density in power/area

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `HeatFluxDensity` |  |

###### `UserSpecifiedHeatAddition`

The user Specifies a heat Addition for the BC
The uom type is Power

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Power` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new_const_temperature(temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  creates a new constant temperature BC

- ```rust
  pub fn new_const_heat_flux(heat_flux: HeatFluxDensity) -> Self { /* ... */ }
  ```
  creates a new constant heat flux bc

- ```rust
  pub fn new_const_heat_addition(heat_addition: Power) -> Self { /* ... */ }
  ```
  creates a new constant heat addition bc

- ```rust
  pub fn new_adiabatic_bc() -> Self { /* ... */ }
  ```
  creates a new adiabatic BC (a heat-addition BC with zero power)

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> BCType { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BCType) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(hte: HeatTransferEntity) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `single_control_vol`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

Module for single control volumes (mainly for fluid control volumes,
but solid control volumes are set by setting flowrate to zero)

Single control volumes by default have functions which abstract away
the details of calculating heat transfer between different
single control volumes as well as between single control volumes and
different boundary conditions

This will abstract away some functionality of the following
modules, and is therefore dependent on these modules:

1. boussinesq_thermophysical_properties
2. fluid_mechanics_correlations
3. heat_transfer_correlations
4. control_volume_dimensions
5. boundary_conditions

By itself, it will NOT contain functions on how to interact with array
control volumes. This is to prevent overbloated hard to read code
Single control-volume (`SingleCVNode`) — the crate's core lumped thermal
node together with its node-to-node and node-to-boundary-condition
interactions.

A `SingleCVNode` stores a lumped state for one fixed control volume: its
specific enthalpy (J/kg), temperature (K), material, mass (kg), pressure
(Pa) and geometric volume (m^3). Its methods abstract away the
heat-transfer bookkeeping between adjacent control volumes and between a
control volume and a boundary condition — each interaction pushes a power
contribution (W) onto the node's enthalpy-rate vector and, where relevant,
a mesh-stability timestep limit (s), so that `advance_timestep` can march
the node forward one explicit Euler step.

By design this module deliberately does NOT hold logic for interacting
with array control volumes; that lives in
`array_control_vol_and_fluid_component_collections` to keep this code
readable. Submodules: `calculation` (advancing one timestep),
`preprocessing` (conduction / Courant / temperature-change timestep
limits), `interaction_between_two_cvs` and
`wrappers_for_heat_transfer_interaction` (node-to-node heat transfer), and
`boundary_condition_interactions` (node-to-boundary-condition heat/flow).

```rust
pub mod single_control_vol { /* ... */ }
```

### Modules

## Module `calculation`

calculation contains the advance timestep associated function


```rust
pub mod calculation { /* ... */ }
```

## Module `interaction_between_two_cvs`

this module
allows for calculating heat transfer values between two
single control volume objects
as well as the suitable timesteps which accompany these calculations

```rust
pub mod interaction_between_two_cvs { /* ... */ }
```

## Module `preprocessing`

contains functions to obtain timestep and other things

```rust
pub mod preprocessing { /* ... */ }
```

## Module `boundary_condition_interactions`

contains functions to help calculate heat transfer between control
volume and a boundary condition
Heat- and mass-transfer interactions between a single control volume and a
boundary condition (constant temperature, constant heat addition, or an
advective inflow/outflow).

Each interaction adds a power contribution (W) to the control volume's
enthalpy-rate vector and, for conduction against a boundary, may register a
mesh-stability timestep limit (s). Advective boundary interactions also
push a volumetric flowrate (m^3/s) used later for the Courant-number
timestep. Constant-temperature boundaries drive the control volume toward
the boundary temperature (K); constant-heat-addition boundaries inject a
fixed power (W).

```rust
pub mod boundary_condition_interactions { /* ... */ }
```

### Modules

## Module `advection_to_bcs`

for advection calculations with heat flux or heat addition BC,
the temperature of flows flowing in and out of the BC will be
determined by that of the control volume

it will be the same temperature as that of the control volume
at that current timestep

this will be quite similar to how OpenFOAM treats inflows and outflows
at zero gradient BCs

```rust
pub mod advection_to_bcs { /* ... */ }
```

## Module `conductance_to_bcs`

calculates a conductance interaction between the constant
temperature bc and cv

for conductance, orientation of bc and cv does not usually matter

```rust
pub mod conductance_to_bcs { /* ... */ }
```

## Module `constant_heat_addition_to_bcs`

calculates a conductance interaction between the constant
temperature bc and cv

for conductance, orientation of bc and cv does not usually matter

```rust
pub mod constant_heat_addition_to_bcs { /* ... */ }
```

### Functions

#### Function `calculate_constant_heat_addition_front_single_cv_back`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates the interaction between a heat addition BC and
a control volume

(single cv) ------------------ (heat addition bc)

the heat addition is at the front, the cv is at the back

```rust
pub fn calculate_constant_heat_addition_front_single_cv_back(control_vol: &mut crate::single_control_vol::SingleCVNode, heat_added_to_control_vol: Power, interaction: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_single_cv_front_constant_heat_addition_back`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates the interaction between a heat addition BC and
a control volume

(heat addition) ------------------ (single cv)

the single cv is at the front, the heat addition is at the back

```rust
pub fn calculate_single_cv_front_constant_heat_addition_back(heat_added_to_control_vol: Power, control_vol: &mut crate::single_control_vol::SingleCVNode, interaction: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `wrappers_for_heat_transfer_interaction`

heat transfer interaction wrappers

these functions help to abstract away some complexities
such as that of calculating
between conductance and advection interactions

```rust
pub mod wrappers_for_heat_transfer_interaction { /* ... */ }
```

## Module `tests`

tests for single control volume
for conjugate heat transfer and lumped heat capacitance
also has semi-infinite medium tests
Verification tests for `SingleCVNode` — lumped heat capacitance, automatic
timestep selection, conjugate heat transfer (CIET heater cases),
semi-infinite 1D transient conduction, and an adiabatic mixing joint. Each
submodule checks predicted temperatures (K / degC) against an analytical or
reference solution.

```rust
pub mod tests { /* ... */ }
```

### Types

#### Struct `SingleCVNode`

SingleCVNode (single control volume node) represents
the control volume with a fixed point

The idea for a SingleCVNode, is for it to contain information
about a control volume.

One can then connect these control volumes with other control
volumes and then specify the interaction or heat transfer between
adjacent Control Volumes CVs and Boundary Conditions BCs

The Control Volume is initiated with a temperature and material
type, this would help determine the control volume's specific
energy,
the mass of the system must also be specified

The changes can be pushed to a vector called the enthalpy
change vector

At the end of the timestep, the next_timestep_specific_enthalpy
is calculated by the current_timestep_control_volume_specific_enthalpy
plus the enthalpy changes in the vector

The temperature can then be calculated from the
next_timestep_specific_enthalpy




```rust
pub struct SingleCVNode {
    pub current_timestep_control_volume_specific_enthalpy: AvailableEnergy,
    pub next_timestep_specific_enthalpy: AvailableEnergy,
    pub rate_enthalpy_change_vector: Vec<Power>,
    pub mass_control_volume: Mass,
    pub material_control_volume: super::boussinesq_thermophysical_properties::Material,
    pub pressure_control_volume: Pressure,
    pub volume: Volume,
    pub max_timestep_vector: Vec<Time>,
    pub mesh_stability_lengthscale_vector: Vec<Length>,
    pub volumetric_flowrate_vector: Vec<VolumeRate>,
    pub temperature: ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `current_timestep_control_volume_specific_enthalpy` | `AvailableEnergy` | specific enthalpy at present timestep, set using<br>the temperature and material type |
| `next_timestep_specific_enthalpy` | `AvailableEnergy` | specific enthalpy at next timestep, used to calculate<br>temperature |
| `rate_enthalpy_change_vector` | `Vec<Power>` | contains rate of change of the specific enthalpy due to changes<br>once courant_number is determined, we would use the correct<br>timestep to multiply the power into an overall enthalpy change |
| `mass_control_volume` | `Mass` | control volume mass |
| `material_control_volume` | `super::boussinesq_thermophysical_properties::Material` | control volume material |
| `pressure_control_volume` | `Pressure` | control volume pressure |
| `volume` | `Volume` | volume of the control volume |
| `max_timestep_vector` | `Vec<Time>` | This vector is meant to house a list of maximum timesteps<br>and is meant for auto time stepping |
| `mesh_stability_lengthscale_vector` | `Vec<Length>` | This vector is meant to house a list of maximum timesteps<br>based on conduction only |
| `volumetric_flowrate_vector` | `Vec<VolumeRate>` | This vector houses a list of volumetric flowrates coming into<br>and out of the control volume<br>by convention, positive flowrates mean going into the<br>cv, negative flowrates mean flowing out of the cv |
| `temperature` | `ThermodynamicTemperature` | cv temperature<br>experimental: control volume temperature  <br>at current timestep |

##### Implementations

###### Methods

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  this function performs necessary calculations to move

- ```rust
  pub fn clear_vectors(self: &mut Self) -> Result<(), TuasLibError> { /* ... */ }
  ```
  clears all vectors for next timestep

- ```rust
  pub fn calculate_conductance_interaction_to_front_singular_cv_node(self: &mut Self, single_cv_2: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  this function calculates the conductance interaction between this

- ```rust
  pub fn calculate_advection_interaction_to_front_singular_cv_node(self: &mut Self, single_cv_2: &mut SingleCVNode, advection_data: DataAdvection) -> Result<(), TuasLibError> { /* ... */ }
  ```
  now, advection is quite tricky because the

- ```rust
  pub fn calculate_mesh_stability_timestep_for_two_single_cv_nodes(self: &mut Self, single_cv_2: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  calculates a suitable timescale when two single cv nodes interact

- ```rust
  pub fn calculate_conduction_timestep(self: &Self) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  this is a function to determine the relevant time scales

- ```rust
  pub fn calculate_courant_number_timestep(self: &mut Self, max_courant_number: Ratio) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  calculates timestep based on courant number

- ```rust
  pub fn append_conduction_mesh_stability_timestep(self: &mut Self, lengthscale: Length) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  appends timestep constrained to fourier number stability

- ```rust
  pub fn get_max_timestep(self: &mut Self, max_temperature_change: TemperatureInterval) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  compiles a list of time steps based on various criteria,

- ```rust
  pub fn calculate_cv_front_bc_back_advection_non_set_temperature(self: &mut Self, advection_data: DataAdvection) -> Result<(), TuasLibError> { /* ... */ }
  ```
  for advection calculations with heat flux or heat addition BC,

- ```rust
  pub fn calculate_bc_front_cv_back_advection_non_set_temperature(self: &mut Self, advection_data: DataAdvection) -> Result<(), TuasLibError> { /* ... */ }
  ```
  for advection calculations with heat flux or heat addition BC,

- ```rust
  pub fn calculate_cv_front_bc_back_advection_set_temperature(self: &mut Self, boundary_condition_temperature: ThermodynamicTemperature, advection_data: DataAdvection) -> Result<(), TuasLibError> { /* ... */ }
  ```
  for advection calculations with set temperature BCs

- ```rust
  pub fn calculate_bc_front_cv_back_advection_set_temperature(self: &mut Self, boundary_condition_temperature: ThermodynamicTemperature, advection_data: DataAdvection) -> Result<(), TuasLibError> { /* ... */ }
  ```
  for advection calculations with set temperature BCs

- ```rust
  pub fn calculate_single_cv_node_constant_temperature_conductance(self: &mut Self, boundary_condition_temperature: ThermodynamicTemperature, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  calculates a conductance interaction between the constant

- ```rust
  pub fn calculate_mesh_stability_conduction_timestep_for_single_node_and_bc(self: &mut Self, interaction: HeatTransferInteractionType) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  for conduction between a control volume and a boundary condition,

- ```rust
  pub fn calculate_between_two_singular_cv_nodes(single_cv_1: &mut SingleCVNode, single_cv_2: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  this is mostly a wrapper function

- ```rust
  pub fn calculate_conductance_interaction_between_two_singular_cv_nodes(single_cv_1: &mut SingleCVNode, single_cv_2: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  calculates the heat transfer between two single control volumes

- ```rust
  pub fn calculate_advection_interaction_between_two_singular_cv_nodes(single_cv_1: &mut SingleCVNode, single_cv_2: &mut SingleCVNode, advection_data: DataAdvection) -> Result<(), TuasLibError> { /* ... */ }
  ```
  calculates the heat transfer between two single control volumes

- ```rust
  pub fn calculate_single_cv_node_front_constant_temperature_back(boundary_condition_temperature: ThermodynamicTemperature, control_vol: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  calculates the heat transfer interaction between a single cv node

- ```rust
  pub fn calculate_constant_temperature_front_single_cv_back(control_vol: &mut SingleCVNode, boundary_condition_temperature: ThermodynamicTemperature, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  for connecting a bc to cv where

- ```rust
  pub fn new(cv_temperature: ThermodynamicTemperature, cv_material: Material, cv_mass: Mass, cv_volume: Volume) -> SingleCVNode { /* ... */ }
  ```
  to initiate the control volume, use this constructor,

- ```rust
  pub fn get_temperature_from_enthalpy_and_set(self: &mut Self) -> Result<ThermodynamicTemperature, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the control volume at the

- ```rust
  pub fn new_sphere(diameter: Length, material: Material, cv_temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<SingleCVNode, TuasLibError> { /* ... */ }
  ```
  this function constructs control volume based on spherical

- ```rust
  pub fn new_one_dimension_volume(length: Length, material: Material, cv_temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<SingleCVNode, TuasLibError> { /* ... */ }
  ```
  this function constructs 1d control volume based on spherical

- ```rust
  pub fn new_block(z: Length, width: Length, thickness: Length, material: Material, cv_temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<SingleCVNode, TuasLibError> { /* ... */ }
  ```
  this function constructs a block

- ```rust
  pub fn new_cylinder(z: Length, diameter: Length, material: Material, cv_temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<SingleCVNode, TuasLibError> { /* ... */ }
  ```
  this function constructs cylinder based on length

- ```rust
  pub fn new_cylindrical_shell(z: Length, id: InnerDiameterThermalConduction, od: OuterDiameterThermalConduction, material: Material, cv_temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<SingleCVNode, TuasLibError> { /* ... */ }
  ```
  this function constructs a cylindrical shell based on length

- ```rust
  pub fn new_odd_shaped_pipe(z: Length, cross_sectional_area: Area, material: Material, cv_temperature: ThermodynamicTemperature, pressure: Pressure) -> Result<SingleCVNode, TuasLibError> { /* ... */ }
  ```
  not all fluid elements are shaped like a cylinder

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SingleCVNode { /* ... */ }
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
    fn default() -> SingleCVNode { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(single_cv: SingleCVNode) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SingleCVNode) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(value: CVType) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<SingleCVNode, <Self as >::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `array_control_vol_and_fluid_component_collections`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

Module for array control volumes (mainly for fluid control volumes,
but solid control volumes are set by setting flowrate to zero)
suitable for tuas_boussinesq_solver (single phase, negligble density changes
except for buoyancy)

also contains code to help calculate pressure drop and mass flow rate
amongst multiple fluid components (eg. pipes) which are usually
represented by array control volumes
This will abstract away some functionality of the following
modules, and is therefore dependent on these modules:

1. boussinesq_thermophysical_properties
2. fluid_mechanics_correlations
3. heat_transfer_correlations
4. control_volume_dimensions
5. boundary_conditions
6. single_control_vol

By itself, it will NOT contain functions on how to interact with array
control volumes. This is to prevent overbloated hard to read code
Array control volumes and fluid-component collections.

This module groups the building blocks TUAS uses to model spatially
resolved (1D) thermal-hydraulic components and networks of them:

- Standalone fluid/solid node arrays (`standalone_fluid_nodes`,
  `standalone_solid_nodes`) — bare matrix solves over `SingleCVNode`
  structs, with no owning array-CV abstraction.
- Thermal-conductance array timestep solvers (`conductance_array_functions`)
  that advance the internal temperature profile (in K) of a lumped
  1D conductor given inner-node and outer-boundary conditions.
- Fully abstracted 1D array control volumes for solids and fluids
  (`one_dimension_cartesian_conducting_medium`,
  `one_d_solid_array_with_lateral_coupling`,
  `one_d_fluid_array_with_lateral_coupling`) that hide the matrix
  bookkeeping and can be coupled laterally (radially) to form 2D/3D
  lattices.
- Fluid-component collections (`fluid_component_collection`) for solving
  pressure drop (Pa) and mass flow rate (kg/s) across pipes arranged in
  series or parallel.

```rust
pub mod array_control_vol_and_fluid_component_collections { /* ... */ }
```

### Modules

## Module `standalone_fluid_nodes`

contains matrix calculations specific to fluid nodes
arranged in a 1D array

These are standalone, and not abstracted under an arrayCV
struct, they only use SingleCVNode structs and representative
arrays to represent an array of control volumes
Standalone single-material fluid-node arrays for high-Peclet-number flow.

These are one-dimensional arrays of fluid control volumes advanced without
axial conduction (advection-dominated, high Peclet number). Each array is
coupled radially to solid surfaces through a thermal-conductance array
(W/K per node) and terminated by a [`SingleCVNode`](crate::single_control_vol::SingleCVNode)
at each end so it can link to neighbouring control volumes; temperatures are
in kelvin.

What belongs here: the timestep advancers — `core_fluid_node` (fluid coupled
to a single surrounding solid, e.g. a pipe wall) and `shell_fluid_node`
(fluid coupled to both an inner and an outer solid) — and the shared
conductance-matrix / power-vector temperature solver in this file. Solid-only
node arrays live in the sibling `standalone_solid_nodes` module.

```rust
pub mod standalone_fluid_nodes { /* ... */ }
```

### Modules

## Module `core_fluid_node`

deals with fluid nodes in the core region

```rust
pub mod core_fluid_node { /* ... */ }
```

### Functions

#### Function `advance_timestep_fluid_node_array_pipe_high_peclet_number`

for high peclet number flows, we can advance timestep without
considering axial conduction

However, this fluid node array will be connected to a shell of some
sort, like a metallic pipe

There will be a conductance array which contains information of how
much thermal conductance connects the inner nodes to the outer
metallic pipe array

[solid]
T_back            T[0]           T[1]          T[n-1]         T_front

----------------fluid solid boundary with thermal resistance ---------

[fluid]
T_back            T[0]           T[1]          T[n-1]         T_front

At the back and front node, there will be a single_cv which is able
to link up to other cvs

You can also add heat volumetrically to the fluid as if it were
generating heat, but you can also set it to zero

This function is standalone, and is not really used inside the
array control volumes, but you are free to use it

```rust
pub fn advance_timestep_fluid_node_array_pipe_high_peclet_number(back_single_cv: &mut crate::single_control_vol::SingleCVNode, front_single_cv: &mut crate::single_control_vol::SingleCVNode, number_of_nodes: usize, dt: Time, total_volume: Volume, q: Power, last_timestep_temperature_solid: &mut Array1<ThermodynamicTemperature>, solid_fluid_conductance_array: &mut Array1<ThermalConductance>, last_timestep_temperature_fluid: &mut Array1<ThermodynamicTemperature>, mass_flowrate: MassRate, volume_fraction_array: &mut Array1<f64>, rho_cp: &mut Array1<VolumetricHeatCapacity>, q_fraction: &mut Array1<f64>) -> Result<Array1<ThermodynamicTemperature>, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `shell_fluid_node`

deals with fluid nodes as if they were in a shell region
that means they are exposed to an inner region and an outer region

```rust
pub mod shell_fluid_node { /* ... */ }
```

### Functions

#### Function `advance_timestep_fluid_shell_array_high_peclet_number`

for high peclet number flows, we can advance timestep without
considering axial conduction within the fluid

this fluid node array will be connected to two adjacent arrays,
they can be boundaries or pipes

There will be a conductance array which contains information of how
much thermal conductance connects the inner nodes to the outer
metallic pipe array

[solid]
T_back            T[0]           T[1]          T[n-1]         T_front

----------------fluid solid boundary with thermal resistance ---------

[fluid]
T_back            T[0]           T[1]          T[n-1]         T_front

----------------fluid solid boundary with thermal resistance ---------

[solid]
T_back            T[0]           T[1]          T[n-1]         T_front

At the back and front node, there will be a single_cv which is able
to link up to other cvs

You can also add heat volumetrically to the fluid as if it were
generating heat, but you can also set it to zero

This function is standalone, and is not really used inside the
array control volumes, but you are free to use it


```rust
pub fn advance_timestep_fluid_shell_array_high_peclet_number(back_single_cv: &mut crate::single_control_vol::SingleCVNode, front_single_cv: &mut crate::single_control_vol::SingleCVNode, number_of_nodes: usize, dt: Time, total_volume: Volume, q: Power, last_timestep_temperature_inner_side: &mut Array1<ThermodynamicTemperature>, inner_side_fluid_conductance_array: &mut Array1<ThermalConductance>, last_timestep_temperature_outer_side: &mut Array1<ThermodynamicTemperature>, outer_side_fluid_conductance_array: &mut Array1<ThermalConductance>, last_timestep_temperature_fluid: &mut Array1<ThermodynamicTemperature>, mass_flowrate: MassRate, volume_fraction_array: &mut Array1<f64>, rho_cp: &mut Array1<VolumetricHeatCapacity>, q_fraction: &mut Array1<f64>) -> Result<Array1<ThermodynamicTemperature>, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

### Functions

#### Function `solve_conductance_matrix_power_vector`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Solves for a temperature vector given a conductance matrix and power vector.

Uses the pure-Rust `SquareMatrix` LU solver inlined into this crate
(`crate::matrix`), so this path has no `outram-foam-basic-lib` / system BLAS
(OpenBLAS/Intel-MKL) dependency.

```rust
pub fn solve_conductance_matrix_power_vector(thermal_conductance_matrix: Array2<ThermalConductance>, power_vector: Array1<Power>) -> Result<Array1<ThermodynamicTemperature>, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `standalone_solid_nodes`

contains matrix calculations specific to solid nodes
these are meant to represent the "shell" of the pipe
or any kind of solid material in the pipe

These are standalone, and not abstracted under an arrayCV
struct, they only use SingleCVNode structs and representative
arrays to represent an array of control volumes
Standalone solid-node arrays with radial conduction only (no axial term).

These are one-dimensional arrays of solid control volumes (e.g. a pipe or
shell wall) advanced one timestep by an implicit energy balance that couples
each node radially to adjacent boundaries through thermal-conductance arrays
(W/K per node); axial conduction along the array is neglected. All node
temperatures are in kelvin and optional volumetric heat generation q is in W.

What belongs here: `shell_solid_node` (a solid node between an inner and an
outer boundary) and `core_solid_nodes` (a solid node with one boundary, the
other treated as adiabatic — implemented by delegating to the shell solver
with a zero-conductance side). The temperature solve reuses the
conductance-matrix solver from the sibling `standalone_fluid_nodes` module.

```rust
pub mod standalone_solid_nodes { /* ... */ }
```

### Modules

## Module `shell_solid_node`

code for solving solid nodes connected to two
adjacent boundaries:

an inner boundary
and an outer boundary

```rust
pub mod shell_solid_node { /* ... */ }
```

### Functions

#### Function `advance_timestep_solid_cylindrical_shell_node_no_axial_conduction`

for most pipe flows, we can consider radial conduction without
considering axial conduction

There will be a conductance array which contains information of how
much thermal conductance connects the inner nodes to the outer
metallic pipe array

[outer side]
T_back[0]         T[1]           T[1]          T[n-1](T_front)

----------------boundary with thermal resistance ---------


[solid]
T_back            T[0]           T[1]          T[n-1](T_front)

----------------boundary with thermal resistance ---------

[inner side]
T_back[0]         T[1]           T[1]          T[n-1](T_front)


You can also add heat volumetrically to the solid as if it were
generating heat, but you can also set it to zero

there is no axial conduction in this case, so the equations
are set up very simply



```rust
pub fn advance_timestep_solid_cylindrical_shell_node_no_axial_conduction(number_of_nodes: usize, dt: Time, total_volume: Volume, q: Power, last_timestep_temperature_inner_side: &mut Array1<ThermodynamicTemperature>, solid_inner_conductance_array: &mut Array1<ThermalConductance>, last_timestep_temperature_outer_side: &mut Array1<ThermodynamicTemperature>, solid_outer_conductance_array: &mut Array1<ThermalConductance>, last_timestep_temperature_solid: &mut Array1<ThermodynamicTemperature>, volume_fraction_array: &mut Array1<f64>, rho_cp: &mut Array1<VolumetricHeatCapacity>, q_fraction: &mut Array1<f64>) -> Result<Array1<ThermodynamicTemperature>, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `core_solid_nodes`

code for solving solid nodes connected to one adjacent boundary

```rust
pub mod core_solid_nodes { /* ... */ }
```

### Functions

#### Function `advance_timestep_solid_cylindrical_core_node_no_axial_conduction`

calculates solid cylindrical core without axial conduction
algorithm is simple, call on the shell_solid_node calculation
but make one side have zero heat conductance

```rust
pub fn advance_timestep_solid_cylindrical_core_node_no_axial_conduction(number_of_nodes: usize, dt: Time, total_volume: Volume, q: Power, last_timestep_temperature_adjacent_side: &mut Array1<ThermodynamicTemperature>, solid_adjacent_side_conductance_array: &mut Array1<ThermalConductance>, last_timestep_temperature_solid: &mut Array1<ThermodynamicTemperature>, volume_fraction_array: &mut Array1<f64>, rho_cp: &mut Array1<VolumetricHeatCapacity>, q_fraction: &mut Array1<f64>) -> Result<Array1<ThermodynamicTemperature>, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `conductance_array_functions`

Thermal-conductance array timestep solvers: advance the internal
temperature profile (in kelvin) of a lumped 1D conductor over one
timestep, solving an implicit conductance-matrix / power-vector system
given an inner temperature node and an outer cooling boundary condition.

```rust
pub mod conductance_array_functions { /* ... */ }
```

## Module `one_dimension_cartesian_conducting_medium`

contains a full struct which abstracts away calculation details

this is relevant for one dimension cartesian (x,y,z) coordinates
you can't really couple these arrays laterally though
One-dimensional Cartesian conducting-medium array control volume.

This module holds [`CartesianConduction1DArray`], an array control volume
that models pure conduction along a single Cartesian (x-direction) axis
through one homogeneous material. The medium is discretised into a chain of
finite-difference temperature nodes (all temperatures in kelvin) linked by
thermal resistors, with a [`SingleCVNode`] at each end so it can couple to
neighbouring control volumes.

What belongs here: the array type and its constructors (this file), plus the
per-timestep machinery split across the `preprocessing`, `calculation`, and
`postprocessing` submodules (conductance / volumetric-heat-capacity arrays,
the implicit-Euler temperature advance, and temperature retrieval). Transport
or two-dimensional / cylindrical conduction models do not belong here.

```rust
pub mod one_dimension_cartesian_conducting_medium { /* ... */ }
```

### Modules

## Module `postprocessing`

Functions or methods to retrieve temperature and other such
data from the array_cv

```rust
pub mod postprocessing { /* ... */ }
```

## Module `preprocessing`

Functions or methods to get timestep and other such quantiies
for calculations

helps to set up quantities used in calculation step

```rust
pub mod preprocessing { /* ... */ }
```

## Module `calculation`

Contains functions which advance the timestep
it's the bulk of calculation

```rust
pub mod calculation { /* ... */ }
```

### Types

#### Struct `CartesianConduction1DArray`

for 1D Cartesian Conduction array,
it is essentially an array control volume of one homogeneous
material

it is in Cartesian coordinates, basically, x direction only conduction

the structure is segregated into several smaller nodes using finite
difference methods

I'll use lumped_nuclear_structure_inspired_functions to calculate
new temperatures for this structure

the scheme used is the implicit Euler scheme to calculate new
temperatures. However, material properties are calculated using
current timestep temperatures rather than next timestep temperatures
therefore, it is more of a hybrid between the implicit and explicit
schemes.

the important methods are to advance timestep, and to update
material properties at every timestep

```rust
pub struct CartesianConduction1DArray {
    pub inner_single_cv: crate::single_control_vol::SingleCVNode,
    pub outer_single_cv: crate::single_control_vol::SingleCVNode,
    pub temperature_array_current_timestep: Array1<ThermodynamicTemperature>,
    pub material_control_volume: crate::boussinesq_thermophysical_properties::Material,
    pub pressure_control_volume: Pressure,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inner_single_cv` | `crate::single_control_vol::SingleCVNode` | represents the inner (lower r) control volume end<br>or back (lower x) control volume<br>to think of which is front and back, we think of coordinates<br>imagine a car or train cruising along in a positive x direction<br><br>//----------------------------------------------> x<br><br>//            (back --- train/car --- front)<br>//            lower x                 higher x<br> |
| `outer_single_cv` | `crate::single_control_vol::SingleCVNode` | represents the outer (higher r) control volume end<br>or front (higher x) control volume<br><br>to think of which is front and back, we think of coordinates<br>imagine a car or train cruising along in a positive x direction<br><br>//----------------------------------------------> x<br><br>//            (back --- train/car --- front)<br>//            lower x                 higher x<br> |
| `temperature_array_current_timestep` | `Array1<ThermodynamicTemperature>` | temperature array current timestep |
| `material_control_volume` | `crate::boussinesq_thermophysical_properties::Material` | control volume material |
| `pressure_control_volume` | `Pressure` | control volume pressure |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn get_temperature_vector(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  returns a clone of the temperature_array_current_timestep

- ```rust
  pub fn get_max_timestep(self: &mut Self, max_temperature_change: TemperatureInterval) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  gets the maximum timestep from the one dimensional

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  calculates the temperature array for the next timestep

- ```rust
  pub fn new(material: Material, initial_uniform_temperature: ThermodynamicTemperature, uniform_pressure: Pressure, inner_nodes: usize, total_length: Length) -> Result<Self, TuasLibError> { /* ... */ }
  ```
  constructs a new instance of the CartesianConduction1DArray

- ```rust
  pub fn get_bulk_temperature(self: &mut Self) -> Result<ThermodynamicTemperature, TuasLibError> { /* ... */ }
  ```
  gets bulk temperature of the array cv based on volume fraction

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> CartesianConduction1DArray { /* ... */ }
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
    fn default() -> CartesianConduction1DArray { /* ... */ }
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
    fn eq(self: &Self, other: &CartesianConduction1DArray) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `one_d_solid_array_with_lateral_coupling`

contains a full struct which abstracts away calculation details
1 dimensional solid arrays

this is relevant for one dimension cartesian (x,y,z) coordinates
except that you can couple these arrays laterally to form a 2D or
3D lattice
One-dimensional solid array control volume with lateral coupling.

This module defines the [`SolidColumn`] type: a 1D chain of solid
control-volume nodes (e.g. a pipe wall, rod, cladding, or structural
conductor) that transports heat by axial conduction along its length and
by lateral (radial) conduction to adjacent arrays. There is no advection
within a solid array.

The submodules split the type's behaviour by concern:
- `constructors` / `default` — build a `SolidColumn` (block, cylinder,
  cylindrical shell, or 1D unit-area volume) and its default state;
- `preprocessing` — stability timestep and bulk-temperature helpers used
  before a timestep;
- `lateral_connection` — register laterally adjacent temperature arrays
  (with thermal conductances) and volumetric power sources;
- `axial_connection` — link the array's back/front nodes to single CVs,
  boundary conditions, or other array CVs;
- `calculation` — assemble and solve the implicit-Euler conduction matrix
  to advance the node temperatures one timestep;
- `postprocessing` — read back the resulting temperature profile.

```rust
pub mod one_d_solid_array_with_lateral_coupling { /* ... */ }
```

### Modules

## Module `postprocessing`

Functions or methods to retrieve temperature and other such
data from the array_cv

```rust
pub mod postprocessing { /* ... */ }
```

## Module `preprocessing`

Functions or methods to get timestep and other such quantiies
for calculations

helps to set up quantities used in calculation step

```rust
pub mod preprocessing { /* ... */ }
```

## Module `calculation`

Contains functions which advance the timestep
it's the bulk of calculation

```rust
pub mod calculation { /* ... */ }
```

## Module `lateral_connection`

contains code to connect control volumes laterally,
in a cylindrical situation, it means radially

```rust
pub mod lateral_connection { /* ... */ }
```

## Module `axial_connection`

contains code to connect to other array cvs, other boundary conditions
or other single cvs
Axial (end-to-end) connections for a [`super::SolidColumn`].

These modules attach heat-transfer entities to the back or front boundary
node of the solid array: another single control volume, a boundary
condition (constant heat flux, constant heat rate, or constant
temperature), or another array control volume (solid column or fluid
array). Advection interactions are rejected — a solid array only conducts.
Heat flows through the shared end node, so the linking helpers reduce to
the single-CV pairwise interaction routines.

```rust
pub mod axial_connection { /* ... */ }
```

### Modules

## Module `interaction_with_single_cv`

the baseline for all interactions with other array cvs
is the interaction with single cvs and bcs
this module takes care of the interactions with single cvs

```rust
pub mod interaction_with_single_cv { /* ... */ }
```

## Module `interaction_with_bc`

the baseline for all interactions with other array cvs
is the interaction with single cvs and bcs
this module takes care of the interactions with boundary conditions
(constant heat flux, constant heat rate, constant temperature)

```rust
pub mod interaction_with_bc { /* ... */ }
```

## Module `interaction_with_array_cv`

this module takes care of the interactions with other array cvs
both solid arrays and fluid arrays

```rust
pub mod interaction_with_array_cv { /* ... */ }
```

## Module `default`

defaults

```rust
pub mod default { /* ... */ }
```

## Module `constructors`

constructors  

```rust
pub mod constructors { /* ... */ }
```

### Types

#### Struct `SolidColumn`

this is essentially a 1D pipe array containing two CVs
and two other laterally connected arrays
(it's essentially a generic solid array representing heat
structures with mainly axial conduction and radial conduction)

it can be used to represent rods, or cylindrical shells
in the latter case, the Column is hollow so to speak

Usually, these will be nested inside a heat transfer component
and then be used

Within this array, the implicit Euler Scheme is used

You must supply the number of nodes for the fluid array
Note that the front and back cv count as one node

```rust
pub struct SolidColumn {
    pub back_single_cv: crate::single_control_vol::SingleCVNode,
    pub front_single_cv: crate::single_control_vol::SingleCVNode,
    pub total_length: Length,
    pub material_control_volume: crate::boussinesq_thermophysical_properties::Material,
    pub pressure_control_volume: Pressure,
    pub lateral_adjacent_array_temperature_vector: Vec<Array1<ThermodynamicTemperature>>,
    pub lateral_adjacent_array_conductance_vector: Vec<Array1<ThermalConductance>>,
    pub q_vector: Vec<Power>,
    pub q_fraction_vector: Vec<Array1<f64>>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `back_single_cv` | `crate::single_control_vol::SingleCVNode` | represents the control volume at the back<br>imagine a car or train cruising along in a positive x direction<br><br>//----------------------------------------------> x<br><br>//            (back --- train/car --- front)<br>//            lower x                 higher x<br> |
| `front_single_cv` | `crate::single_control_vol::SingleCVNode` | represents the control volume at the front<br><br>to think of which is front and back, we think of coordinates<br>imagine a car or train cruising along in a positive x direction<br><br>//----------------------------------------------> x<br><br>//            (back --- train/car --- front)<br>//            lower x                 higher x<br> |
| `total_length` | `Length` | total length for the array |
| `material_control_volume` | `crate::boussinesq_thermophysical_properties::Material` | control volume material |
| `pressure_control_volume` | `Pressure` | control volume pressure |
| `lateral_adjacent_array_temperature_vector` | `Vec<Array1<ThermodynamicTemperature>>` | now solid arrays (columns) can be connected to solid arrays<br>or other fluid arrays adjacent to it radially<br><br>There will be no advection but there can<br>be thermal conductance shared between the nodes<br><br>hence, I only want to have a copy of the temperature<br>arrays radially adjacent to it<br><br>plus their thermal resistances<br>N is the array size, which is known at compile time |
| `lateral_adjacent_array_conductance_vector` | `Vec<Array1<ThermalConductance>>` | now solid arrays (columns) can be connected to solid arrays<br>or other fluid arrays adjacent to it radially<br><br>There will be no advection<br>but there can be thermal conductance shared between the nodes<br><br>hence, I only want to have a copy of the temperature<br>arrays radially adjacent to it<br><br>plus their thermal resistances<br>N is the array size, which is known at compile time |
| `q_vector` | `Vec<Power>` | solid arrays can also be connected to heat sources<br>or have specified volumetric heat sources |
| `q_fraction_vector` | `Vec<Array1<f64>>` | solid arrays should have their power distributed according<br>to their nodes |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn get_temperature_vector(self: &Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  obtains a clone of the temperature vector within the CV

- ```rust
  pub fn get_reverse_temperature_vector(self: &Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  obtains a clone of temperature vector, but in reverse format

- ```rust
  pub fn get_max_timestep(self: &mut Self, max_temperature_change: TemperatureInterval) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  gets the maximum timestep from the

- ```rust
  pub fn try_get_bulk_temperature(self: &mut Self) -> Result<ThermodynamicTemperature, TuasLibError> { /* ... */ }
  ```
  gets bulk temperature of the array cv based on volume fraction

- ```rust
  pub fn get_component_length(self: &Self) -> Length { /* ... */ }
  ```
  obtains length of the array

- ```rust
  pub fn get_component_xs_area(self: &Self) -> Area { /* ... */ }
  ```
  obtains cross sectional area of the array

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  advances timestep for the solid array column

- ```rust
  pub fn clear_vectors(self: &mut Self) -> Result<(), TuasLibError> { /* ... */ }
  ```
  clears all vectors for next timestep

- ```rust
  pub fn lateral_link_new_temperature_vector_avg_conductance(self: &mut Self, average_thermal_conductance: ThermalConductance, temperature_vec: Vec<ThermodynamicTemperature>) -> Result<(), TuasLibError> { /* ... */ }
  ```
  connects a laterally (radially) adjacent solid or fluid array to this

- ```rust
  pub fn lateral_link_new_power_vector(self: &mut Self, power_source: Power, q_fraction_arr: Array1<f64>) -> Result<(), TuasLibError> { /* ... */ }
  ```
  connects an adjacent solid or fluid node laterally

- ```rust
  pub fn link_single_cv_to_lower_side(self: &mut Self, single_cv_node_other: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches a single cv to the back,entrance,

- ```rust
  pub fn link_single_cv_to_higher_side(self: &mut Self, single_cv_node_other: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches a single cv to the exit,front,

- ```rust
  pub fn calculate_timestep_for_single_cv_to_front_of_array_cv(self: &mut Self, single_cv_node_other: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  calculates timestep for a single cv attached to the front of the

- ```rust
  pub fn calculate_timestep_for_single_cv_to_back_of_array_cv(self: &mut Self, single_cv_node_other: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  calculates timestep for a single cv attached to the back of the

- ```rust
  pub fn link_heat_flux_bc_to_front_of_this_cv(self: &mut Self, heat_flux_into_control_vol: HeatFluxDensity, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant heat flux BC to the front of this

- ```rust
  pub fn link_heat_flux_bc_to_back_of_this_cv(self: &mut Self, heat_flux_into_control_vol: HeatFluxDensity, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant heat flux BC to the back of this

- ```rust
  pub fn link_heat_addition_to_front_of_this_cv(self: &mut Self, heat_rate: Power, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant heat rate BC to the front of this

- ```rust
  pub fn link_heat_addition_to_back_of_this_cv(self: &mut Self, heat_rate: Power, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant heat rate BC to the back of this

- ```rust
  pub fn link_constant_temperature_to_front_of_this_cv(self: &mut Self, bc_temperature: ThermodynamicTemperature, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant temperature BC to the front of this

- ```rust
  pub fn link_constant_temperature_to_back_of_this_cv(self: &mut Self, bc_temperature: ThermodynamicTemperature, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant temperature BC to the front of this

- ```rust
  pub fn link_solid_column_to_the_front_of_this_solid_column(self: &mut Self, solid_column_other: &mut SolidColumn, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an array control volume to the front of this

- ```rust
  pub fn link_solid_column_to_the_back_of_this_solid_column(self: &mut Self, solid_column_other: &mut SolidColumn, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an array control volume to the back of this

- ```rust
  pub fn link_fluid_array_to_the_front_of_this_solid_column(self: &mut Self, fluid_array_other: &mut FluidArray, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches a fluid array control volume to the front of this

- ```rust
  pub fn link_fluid_array_to_the_back_of_this_solid_column(self: &mut Self, fluid_array_other: &mut FluidArray, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches a fluid array control volume to the back of this

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  generic constructor,

- ```rust
  pub fn new_block(length: Length, thickness: Length, width: Length, initial_temperature: ThermodynamicTemperature, initial_pressure: Pressure, solid_material: SolidMaterial, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  returns a solid in the shape of a block

- ```rust
  pub fn new_one_dimension_volume(length: Length, initial_temperature: ThermodynamicTemperature, initial_pressure: Pressure, solid_material: SolidMaterial, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  returns a one dimensioned volume

- ```rust
  pub fn new_cylinder(length: Length, diameter: Length, initial_temperature: ThermodynamicTemperature, initial_pressure: Pressure, solid_material: SolidMaterial, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  returns a solid in the shape of a cylinder

- ```rust
  pub fn new_cylindrical_shell(length: Length, inner_diameter: Length, outer_diameter: Length, initial_temperature: ThermodynamicTemperature, initial_pressure: Pressure, solid_material: SolidMaterial, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  returns a solid array in the shape of a cylindrical

- ```rust
  pub fn get_temperature_array(self: &Self) -> Result<Array1<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  obtains a clone of the temperature array in Array1 ndarray

- ```rust
  pub fn set_temperature_vector(self: &mut Self, temperature_vec: Vec<ThermodynamicTemperature>) -> Result<(), TuasLibError> { /* ... */ }
  ```
  sets the node temperature array (in kelvin) from a temperature

- ```rust
  pub fn set_temperature_array(self: &mut Self, temperature_arr: Array1<ThermodynamicTemperature>) -> Result<(), TuasLibError> { /* ... */ }
  ```
  sets the node temperature array from an `Array1` of

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  total number of temperature nodes in the solid array,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SolidColumn { /* ... */ }
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

  - ```rust
    fn from(solid_array: SolidColumn) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SolidColumn) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(value: CVType) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<SolidColumn, <Self as >::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `one_d_fluid_array_with_lateral_coupling`

contains a full struct which abstracts away calculation details
1 dimensional fluid arrays

this is relevant for one dimension cartesian (x,y,z) coordinates
except that you can couple these arrays laterally to form a 2D or
3D lattice
A 1D fluid control-volume array (`FluidArray`) with lateral (radial)
conduction coupling and axial advection.

This module owns the `FluidArray` type: a chain of fluid nodes discretising
a pipe or channel along its flow axis. The array carries axial advection
(mass flow in kg/s) and axial conduction between neighbouring nodes, and can
be coupled laterally (radially) to adjacent solid or fluid arrays through
shared thermal conductances (W/K) with no radial advection. It also acts as
a `FluidComponent`, providing pressure-loss / mass-flowrate relations via a
Darcy friction-factor correlation.

The node temperatures (K) are advanced with an implicit (backward) Euler
scheme; see the `calculation` submodule for the energy balance. Submodules
group the behaviour: `constructors`/`default` build arrays, `preprocessing`
computes timestep/Reynolds/Nusselt quantities, `postprocessing` reads out
temperature profiles, `lateral_connection` wires radial conductances and
power sources, `axial_connection` links neighbouring CVs/BCs at the front and
back, `fluid_component_calculation` holds the friction-loss correlations, and
`type_conversion` converts to/from `FluidComponent`.

```rust
pub mod one_d_fluid_array_with_lateral_coupling { /* ... */ }
```

### Modules

## Module `postprocessing`

Functions or methods to retrieve temperature and other such
data from the array_cv

```rust
pub mod postprocessing { /* ... */ }
```

## Module `preprocessing`

Functions or methods to get timestep and other such quantiies
for calculations

helps to set up quantities used in calculation step

```rust
pub mod preprocessing { /* ... */ }
```

## Module `calculation`

Contains functions which advance the timestep
it's the bulk of calculation

```rust
pub mod calculation { /* ... */ }
```

## Module `lateral_connection`

contains code to connect control volumes laterally,
in a cylindrical situation, it means radially

```rust
pub mod lateral_connection { /* ... */ }
```

## Module `axial_connection`

contains code to connect to other array cvs, other boundary conditions
or other single cvs
Axial (end-to-end) connections for a `FluidArray`.

These submodules attach heat-transfer entities to the front (higher
coordinate) or back (lower coordinate) faces of the array along its flow
axis: other single control volumes, boundary conditions (constant heat flux
in W/m^2, constant heat rate in W, or constant temperature in K), and other
array CVs (fluid arrays or solid columns). Axial links carry both advective
enthalpy transport (when mass flows across the face) and conduction; lateral
(radial) coupling lives in the sibling `lateral_connection` module instead.

```rust
pub mod axial_connection { /* ... */ }
```

### Modules

## Module `interaction_with_single_cv`

the baseline for all interactions with other array cvs
is the interaction with single cvs and bcs
this module takes care of the interactions with single cvs

```rust
pub mod interaction_with_single_cv { /* ... */ }
```

## Module `interaction_with_bc`

the baseline for all interactions with other array cvs
is the interaction with single cvs and bcs
this module takes care of the interactions with single cvs

```rust
pub mod interaction_with_bc { /* ... */ }
```

## Module `interaction_with_array_cv`

this module takes care of the interactions with other array cvs
both solid arrays and fluid arrays

```rust
pub mod interaction_with_array_cv { /* ... */ }
```

## Module `default`

defaults

```rust
pub mod default { /* ... */ }
```

## Module `constructors`

constructors  

```rust
pub mod constructors { /* ... */ }
```

## Module `fluid_component_calculation`

fluid component calculations
with the DimensionlessDarcyLossCorrelations
Fluid-component (pressure-loss / mass-flowrate) behaviour for a `FluidArray`.

This module holds the `DimensionlessDarcyLossCorrelations` enum, which
encodes the dimensionless friction/form-loss law of a component as a
function of Reynolds number: a pipe (Churchill Darcy friction factor times
L/D plus a form-loss K), a simple Reynolds power law (f_darcy = A + B Re^C),
or Ergun (packed bed, not yet implemented). From it, the code derives the
Bejan number, the pressure loss (Pa), and the Reynolds number from a given
pressure loss (root-finding on the Bejan/Reynolds relation).

It also implements the `FluidArray` getters/setters that make the array
behave as a fluid component: mass flowrate (kg/s), pressure loss (Pa),
cross-sectional area (m^2), hydraulic diameter (m, 4A/P), and
temperature-dependent fluid viscosity (Pa.s) and density (kg/m^3) evaluated
at the array bulk temperature, plus incline angle and internal pressure
source (e.g. a simulated pump).

```rust
pub mod fluid_component_calculation { /* ... */ }
```

### Modules

## Module `unit_test_dimensionless_darcy_loss_correlations`

unit tests for DimensionlessDarcyLossCorrelations

```rust
pub mod unit_test_dimensionless_darcy_loss_correlations { /* ... */ }
```

## Module `unit_test_mass_flowrate_and_pressure_change_dimensionless_darcy_loss`

unit tests for DimensionlessDarcyLossCorrelations get and set
mass flowrate and pressure change

```rust
pub mod unit_test_mass_flowrate_and_pressure_change_dimensionless_darcy_loss { /* ... */ }
```

### Types

#### Enum `DimensionlessDarcyLossCorrelations`

contains form loss or minor loss correlations for use

This will return a friction factor if one wishes it

```rust
pub enum DimensionlessDarcyLossCorrelations {
    Pipe(Ratio, Ratio, Ratio),
    SimpleReynoldsPower(Ratio, Ratio, f64),
    Ergun,
}
```

##### Variants

###### `Pipe`

standard pipe loss, must input
roughness ratio

and also a K ratio for generic form losses

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Ratio` |  |
| 1 | `Ratio` |  |
| 2 | `Ratio` |  |

###### `SimpleReynoldsPower`

Reynold's power correlation in the form
f_darcy = A + B Re^(C)

The first in the tuple is A,
the second is B, the third is C

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Ratio` |  |
| 1 | `Ratio` |  |
| 2 | `f64` |  |

###### `Ergun`

Ergun Equation
Ergun, S., & Orning, A. A. (1949). Fluid flow through
randomly packed columns and fluidized beds. Industrial
& Engineering Chemistry, 41(6), 1179-1184.

not done yet

##### Implementations

###### Methods

- ```rust
  pub fn new_pipe(pipe_length: Length, surface_roughness: Length, hydraulic_diameter: Length, form_loss: Ratio) -> Self { /* ... */ }
  ```
  creates a new pipe object

- ```rust
  pub fn new_simple_reynolds_power_component(a: Ratio, b: Ratio, c: f64) -> Self { /* ... */ }
  ```
  creates a new simple reynolds power correlation object

- ```rust
  pub fn fldk_based_on_darcy_friction_factor(self: &Self, reynolds_input: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets the (f L/D + K) based on reynolds number and

- ```rust
  pub fn darcy_friction_factor(self: &Self, reynolds_input: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets the darcy friction factor based on reynolds number and

- ```rust
  pub fn get_bejan_number_from_reynolds(self: &Self, reynolds_input: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  obtains bejan number given a reynolds number

- ```rust
  pub fn get_reynolds_number_from_bejan(self: &Self, bejan_input: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  obtains a reynolds number from a given bejan number

- ```rust
  pub fn get_pressure_loss_from_reynolds(self: &Self, reynolds_input: Ratio, hydraulic_diameter: Length, fluid_density: MassDensity, fluid_viscosity: DynamicViscosity) -> Result<Pressure, TuasLibError> { /* ... */ }
  ```
  pressure drop from Re

- ```rust
  pub fn get_reynolds_from_pressure_loss(self: &Self, pressure_loss_input: Pressure, hydraulic_diameter: Length, fluid_density: MassDensity, fluid_viscosity: DynamicViscosity) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  get Re from pressure drop

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DimensionlessDarcyLossCorrelations { /* ... */ }
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
    the default is just K = 1

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &DimensionlessDarcyLossCorrelations) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `type_conversion`

type conversion

```rust
pub mod type_conversion { /* ... */ }
```

### Types

#### Struct `FluidArray`

A 1D array of fluid control volumes discretising a pipe or channel along
its flow (axial) direction.

It contains a back single CV (lowest axial coordinate) and a front single
CV (highest axial coordinate), plus `inner_nodes` interior nodes between
them, and carries axial advection (mass flow in kg/s), axial conduction
between neighbouring nodes, and lateral (radial) conduction coupling to
adjacent solid or fluid arrays (thermal conductances in W/K, no radial
advection).

It can represent a cylindrical pipe, an annular channel, or an arbitrary
odd-shaped flow passage (see the constructors). Usually these are nested
inside a larger heat-transfer component and then used.

Node temperatures (K) are advanced with the implicit (backward) Euler
scheme.

You must supply the number of nodes for the fluid array. Note that the
front and back CVs each count as one node, so the total node count is
`inner_nodes + 2`.

```rust
pub struct FluidArray {
    pub back_single_cv: crate::single_control_vol::SingleCVNode,
    pub front_single_cv: crate::single_control_vol::SingleCVNode,
    pub material_control_volume: crate::boussinesq_thermophysical_properties::Material,
    pub pressure_control_volume: Pressure,
    pub fluid_component_loss_properties: self::fluid_component_calculation::DimensionlessDarcyLossCorrelations,
    pub nusselt_correlation: crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation,
    pub lateral_adjacent_array_temperature_vector: Vec<Array1<ThermodynamicTemperature>>,
    pub lateral_adjacent_array_conductance_vector: Vec<Array1<ThermalConductance>>,
    pub q_vector: Vec<Power>,
    pub q_fraction_vector: Vec<Array1<f64>>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `back_single_cv` | `crate::single_control_vol::SingleCVNode` | represents the control volume at the back<br>imagine a car or train cruising along in a positive x direction<br><br>//----------------------------------------------> x<br><br>//            (back --- train/car --- front)<br>//            lower x                 higher x<br> |
| `front_single_cv` | `crate::single_control_vol::SingleCVNode` | represents the control volume at the front<br><br>to think of which is front and back, we think of coordinates<br>imagine a car or train cruising along in a positive x direction<br><br>//----------------------------------------------> x<br><br>//            (back --- train/car --- front)<br>//            lower x                 higher x<br> |
| `material_control_volume` | `crate::boussinesq_thermophysical_properties::Material` | control volume material |
| `pressure_control_volume` | `Pressure` | control volume pressure |
| `fluid_component_loss_properties` | `self::fluid_component_calculation::DimensionlessDarcyLossCorrelations` | fluid component loss properties<br>be it for pipe or something else |
| `nusselt_correlation` | `crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation` | nusselt correlation |
| `lateral_adjacent_array_temperature_vector` | `Vec<Array1<ThermodynamicTemperature>>` | now fluid arrays can be connected to solid arrays<br>or other fluid arrays adjacent to it radially<br><br>There will be no advection in the radial direction,<br>but there can be thermal conductance shared between the nodes<br><br>hence, I only want to have a copy of the temperature<br>arrays radially adjacent to it<br><br>plus their thermal resistances<br>N is the array size, which is known at compile time |
| `lateral_adjacent_array_conductance_vector` | `Vec<Array1<ThermalConductance>>` | now fluid arrays can be connected to solid arrays<br>or other fluid arrays adjacent to it radially<br><br>There will be no advection in the radial direction,<br>but there can be thermal conductance shared between the nodes<br><br>hence, I only want to have a copy of the temperature<br>arrays radially adjacent to it<br><br>plus their thermal resistances<br>N is the array size, which is known at compile time |
| `q_vector` | `Vec<Power>` | fluid arrays can also be connected to heat sources<br>or have specified volumetric heat sources |
| `q_fraction_vector` | `Vec<Array1<f64>>` | fluid arrays should have their power distributed according<br>to their nodes |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn get_temperature_vector(self: &Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  obtains a clone of the temperature vector within the CV

- ```rust
  pub fn get_reverse_temperature_vector(self: &Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  obtains a clone of temperature vector, but in reverse format

- ```rust
  pub fn get_max_timestep(self: &mut Self, max_temperature_change: TemperatureInterval, mass_flowrate: MassRate) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  gets the maximum stable timestep (seconds) for this fluid array,

- ```rust
  pub fn try_get_bulk_temperature(self: &mut Self) -> Result<ThermodynamicTemperature, TuasLibError> { /* ... */ }
  ```
  gets bulk temperature of the array cv based on volume fraction

- ```rust
  pub fn get_reynolds(self: &mut Self, mass_flowrate: MassRate) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets the reynolds number for this fluid array

- ```rust
  pub fn get_nusselt(self: &mut Self, reynolds: Ratio, prandtl_bulk: Ratio, prandtl_wall: Ratio) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets the nusselt number based on reynolds number

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  advance_timestep in the array, using the mass flowrate set

- ```rust
  pub fn advance_timestep_with_mass_flowrate(self: &mut Self, timestep: Time, mass_flowrate: MassRate) -> Result<(), TuasLibError> { /* ... */ }
  ```
  advances timestep for the fluid array

- ```rust
  pub fn clear_vectors(self: &mut Self) -> Result<(), TuasLibError> { /* ... */ }
  ```
  clears all vectors for next timestep

- ```rust
  pub fn lateral_link_new_temperature_vector_avg_conductance(self: &mut Self, average_thermal_conductance: ThermalConductance, temperature_vec: Vec<ThermodynamicTemperature>) -> Result<(), TuasLibError> { /* ... */ }
  ```
  connects an adjacent solid or fluid node laterally

- ```rust
  pub fn lateral_link_new_power_vector(self: &mut Self, power_source: Power, q_fraction_arr: Array1<f64>) -> Result<(), TuasLibError> { /* ... */ }
  ```
  connects an adjacent solid or fluid node laterally

- ```rust
  pub fn link_single_cv_to_lower_side(self: &mut Self, single_cv_node_other: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches a single cv to the front,entrance,

- ```rust
  pub fn link_single_cv_to_higher_side(self: &mut Self, single_cv_node_other: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches a single cv to the exit, front,

- ```rust
  pub fn calculate_timestep_for_single_cv_to_front_of_array_cv(self: &mut Self, single_cv_node_other: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  calculates timestep for a single cv attached to the front of the

- ```rust
  pub fn calculate_timestep_for_single_cv_to_back_of_array_cv(self: &mut Self, single_cv_node_other: &mut SingleCVNode, interaction: HeatTransferInteractionType) -> Result<Time, TuasLibError> { /* ... */ }
  ```
  calculates timestep for a single cv attached to the back of the

- ```rust
  pub fn link_heat_flux_bc_to_front_of_this_cv(self: &mut Self, heat_flux_into_control_vol: HeatFluxDensity, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant heat flux BC to the front of this

- ```rust
  pub fn link_heat_flux_bc_to_back_of_this_cv(self: &mut Self, heat_flux_into_control_vol: HeatFluxDensity, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant heat flux BC to the back of this

- ```rust
  pub fn link_heat_addition_to_front_of_this_cv(self: &mut Self, heat_rate: Power, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant heat rate BC to the front of this

- ```rust
  pub fn link_heat_addition_to_back_of_this_cv(self: &mut Self, heat_rate: Power, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant heat rate BC to the front of this

- ```rust
  pub fn link_constant_temperature_to_front_of_this_cv(self: &mut Self, bc_temperature: ThermodynamicTemperature, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant temperature BC to the front of this

- ```rust
  pub fn link_constant_temperature_to_back_of_this_cv(self: &mut Self, bc_temperature: ThermodynamicTemperature, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an constant temperature BC to the front of this

- ```rust
  pub fn link_fluid_array_to_the_front_of_this_fluid_array(self: &mut Self, fluid_array_other: &mut FluidArray, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches another fluid array control volume to the front of this

- ```rust
  pub fn link_fluid_array_to_the_back_of_this_fluid_array(self: &mut Self, fluid_array_other: &mut FluidArray, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches another fluid array

- ```rust
  pub fn link_solid_column_to_the_front_of_this_fluid_array(self: &mut Self, solid_column_other: &mut SolidColumn, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an solid column array control volume to the front of this

- ```rust
  pub fn link_solid_column_to_the_back_of_this_fluid_array(self: &mut Self, solid_column_other: &mut SolidColumn, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  attaches an solid column

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  generic constructor,

- ```rust
  pub fn new_cylinder(length: Length, hydraulic_diameter: Length, initial_temperature: ThermodynamicTemperature, initial_pressure: Pressure, adjacent_solid_material: SolidMaterial, liquid_material: LiquidMaterial, pipe_form_loss: Ratio, user_specified_inner_nodes: usize, pipe_incline_angle: Angle) -> Self { /* ... */ }
  ```
  returns a fluid in the shape of a cylinder

- ```rust
  pub fn new_annular_cylinder(length: Length, inner_diameter: Length, outer_diameter: Length, initial_temperature: ThermodynamicTemperature, initial_pressure: Pressure, adjacent_solid_material: SolidMaterial, liquid_material: LiquidMaterial, pipe_form_loss: Ratio, user_specified_inner_nodes: usize, pipe_incline_angle: Angle) -> Self { /* ... */ }
  ```
  returns a fluid array cv in the shape of a cylindrical

- ```rust
  pub fn new_odd_shaped_pipe(length: Length, hydraulic_diameter: Length, cross_sectional_area: Area, initial_temperature: ThermodynamicTemperature, initial_pressure: Pressure, adjacent_solid_material: SolidMaterial, liquid_material: LiquidMaterial, pipe_form_loss: Ratio, user_specified_inner_nodes: usize, pipe_incline_angle: Angle) -> Self { /* ... */ }
  ```
  odd shaped pipe, where one defines an arbitrary flow area

- ```rust
  pub fn new_custom_component(length: Length, hydraulic_diameter: Length, cross_sectional_area: Area, initial_temperature: ThermodynamicTemperature, initial_pressure: Pressure, liquid_material: LiquidMaterial, a: Ratio, b: Ratio, c: f64, user_specified_inner_nodes: usize, pipe_incline_angle: Angle) -> Self { /* ... */ }
  ```
  creates a custom component where the user specifies

- ```rust
  pub fn get_mass_flowrate(self: &mut Self) -> MassRate { /* ... */ }
  ```
  gets mass flowrate for the fluid array

- ```rust
  pub fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
  ```
  sets the mass flowrate for the fluid array

- ```rust
  pub fn get_mass_flowrate_from_pressure_loss_immutable(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
  ```
  gets the mass flowrate for the fluid array

- ```rust
  pub fn get_pressure_loss(self: &mut Self) -> Pressure { /* ... */ }
  ```
  gets the pressure loss for the fluid array

- ```rust
  pub fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
  ```
  sets the pressure loss for the fluid array

- ```rust
  pub fn get_pressure_loss_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
  ```
  to get mass flowrate from pressure loss, we need to

- ```rust
  pub fn get_cross_sectional_area(self: &mut Self) -> Area { /* ... */ }
  ```
  gets cross sectional area using a mutable borrow

- ```rust
  pub fn get_cross_sectional_area_immutable(self: &Self) -> Area { /* ... */ }
  ```
  gets cross sectional area using an immutable borrow

- ```rust
  pub fn get_hydraulic_diameter(self: &mut Self) -> Length { /* ... */ }
  ```
  gets hydraulic diameter using a mutable borrow

- ```rust
  pub fn get_hydraulic_diameter_immutable(self: &Self) -> Length { /* ... */ }
  ```
  gets hydraulic diameter using an immutable borrow

- ```rust
  pub fn get_fluid_viscosity(self: &mut Self) -> DynamicViscosity { /* ... */ }
  ```
  gets fluid viscosity with a mutable borrow

- ```rust
  pub fn get_fluid_viscosity_immutable(self: &Self) -> DynamicViscosity { /* ... */ }
  ```
  gets fluid viscosity with a immutable borrow

- ```rust
  pub fn get_fluid_density(self: &mut Self) -> MassDensity { /* ... */ }
  ```
  gets fluid fluid density with a mutable borrow

- ```rust
  pub fn get_fluid_density_immutable(self: &Self) -> MassDensity { /* ... */ }
  ```
  gets fluid fluid density with a mutable borrow

- ```rust
  pub fn get_component_length(self: &mut Self) -> Length { /* ... */ }
  ```
  gets fluid array length

- ```rust
  pub fn get_component_length_immutable(self: &Self) -> Length { /* ... */ }
  ```
  gets fluid array length

- ```rust
  pub fn get_incline_angle(self: &mut Self) -> Angle { /* ... */ }
  ```
  gets incline angle (the angle at which it is inclined to

- ```rust
  pub fn get_incline_angle_immutable(self: &Self) -> Angle { /* ... */ }
  ```
  gets incline angle (the angle at which it is inclined to

- ```rust
  pub fn get_internal_pressure_source(self: &mut Self) -> Pressure { /* ... */ }
  ```
  gets the internal pressure source

- ```rust
  pub fn get_internal_pressure_source_immutable(self: &Self) -> Pressure { /* ... */ }
  ```
  gets the internal pressure source

- ```rust
  pub fn set_internal_pressure_source(self: &mut Self, internal_pressure: Pressure) { /* ... */ }
  ```
  sets the internal pressure source

- ```rust
  pub fn get_temperature_array(self: &Self) -> Result<Array1<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  obtains a clone of the temperature array in Array1 ndarray

- ```rust
  pub fn set_temperature_vector(self: &mut Self, temperature_vec: Vec<ThermodynamicTemperature>) -> Result<(), TuasLibError> { /* ... */ }
  ```
  sets the node temperature array (K) from a temperature vector.

- ```rust
  pub fn set_temperature_array(self: &mut Self, temperature_arr: Array1<ThermodynamicTemperature>) -> Result<(), TuasLibError> { /* ... */ }
  ```
  sets the node temperature array (K) from an `Array1` ndarray.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  length of the fluid array

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FluidArray { /* ... */ }
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

  - ```rust
    fn from(fluid_array: FluidArray) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FluidArray) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(value: FluidComponent) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(value: CVType) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<FluidArray, <Self as >::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `fluid_component_collection`

contains code for calculating pressure drop and mass flowrates over
pipes in series or parallel
Fluid component collections: hydraulic-network building blocks.

This module groups the abstractions used to compute mass flowrates
(kg/s) and pressure changes / pressure losses (Pa) for pipes, fittings,
and networks of them:

- [`fluid_component`] — the [`FluidComponent`] enum (a single fluid array,
  or a bundle of identical parallel fluid arrays / tubes).
- [`fluid_component_traits`] — the [`FluidComponentTrait`] contract plus the
  pipe/custom-component pressure-loss and pressure-change calculation traits.
- [`collection_series_and_parallel_functions`] — associated functions that
  combine a `Vec<FluidComponent>` in series or in parallel.
- [`fluid_component_collection`] — the [`FluidComponentCollection`] struct
  (a vector of components with a series/parallel orientation).
- [`super_collection_series_and_parallel_functions`] and
  [`fluid_component_super_collection`] — the same, one level up: a vector of
  collections (branches), used e.g. for multiple loops in parallel.
- [`tests_and_examples`] — worked examples showing how to assemble and solve
  these networks.

Pressure sign convention throughout: `pressure_change = -pressure_loss +
hydrostatic_pressure_change + internal_pressure_source`.

```rust
pub mod fluid_component_collection { /* ... */ }
```

### Modules

## Module `fluid_component`

License
   This file is part of tuas_boussinesq_solver, a partial library of the
   thermal hydraulics library written in rust meant to help with the
   fluid mechanics and heat transfer aspects of the calculations
     
   Copyright (C) 2022-2023  Theodore Kay Chen Ong, Singapore Nuclear
   Research and Safety Initiative, Per F. Peterson, University of
   California, Berkeley Thermal Hydraulics Laboratory

   tuas_boussinesq_solver is free software; you can redistribute it and/or modify it
   under the terms of the GNU General Public License as published by the
   Free Software Foundation; either version 2 of the License, or (at your
   option) any later version.

   tuas_boussinesq_solver is distributed in the hope that it will be useful, but WITHOUT
   ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
   FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
   for more details.

   This library is part of a thermal hydraulics library in rust
   and contains some code copied from GeN-Foam, and OpenFOAM derivative.
   This offering is not approved or endorsed by the OpenFOAM Foundation nor
   OpenCFD Limited, producer and distributor of the OpenFOAM(R)software via
   www.openfoam.com, and owner of the OPENFOAM(R) and OpenCFD(R) trademarks.
   Nor is it endorsed by the authors and owners of GeN-Foam.

   You should have received a copy of the GNU General Public License
   along with this program.  If not, see <http://www.gnu.org/licenses/>.

© All rights reserved. Theodore Kay Chen Ong,
Singapore Nuclear Research and Safety Initiative,
Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Laboratory

Main author of the code: Theodore Kay Chen Ong, supervised by
Professor Per F. Peterson
FluidComponents are pipes and fittings you can connect in parallel
such that you can calculate mass flowrate and pressure drop from them

These are usually array control volumes, but could be other components
as well

```rust
pub mod fluid_component { /* ... */ }
```

### Types

#### Enum `FluidComponent`

FluidComponents are pipes and fittings you can connect in parallel
such that you can calculate mass flowrate and pressure drop from them

```rust
pub enum FluidComponent {
    FluidArray(crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray),
    ParallelUniformFluidArray(crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray, u32),
}
```

##### Variants

###### `FluidArray`

these are arrays of control volumes connected in series

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray` |  |

###### `ParallelUniformFluidArray`

these are parallel arrays of fluid arrays (which themselves
are control volumes in series)

one fluid array represents one tube in this parallel array

to get the heat transfer overall, multiply by number of tubes
if given an overall mass flowrate and one wants to find
the mass flowrate through one tube, then divide by number
of tubes (the u32 value)

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray` |  |
| 1 | `u32` |  |

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
    fn clone(self: &Self) -> FluidComponent { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **FluidComponentTrait**
  - ```rust
    fn get_mass_flowrate(self: &mut Self) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_loss_immutable(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area(self: &mut Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area_immutable(self: &Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_at_ref_temperature(self: &mut Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_immutable_at_ref_temperature(self: &Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_at_ref_temperature(self: &mut Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_immutable_at_ref_temperature(self: &Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_component_length(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_component_length_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle(self: &mut Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle_immutable(self: &Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source_immutable(self: &Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_internal_pressure_source(self: &mut Self, internal_pressure: Pressure) { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FluidComponent) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(value: FluidComponent) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `fluid_component_traits`

contains a trait for use in fluid components and
collections of fluid components
This is because mass flowrate and pressure drop also need to be
calculated from collections of fluid components
Traits defining the fluid-component hydraulic contract.

Everything a pipe or fluid component must expose so that mass flowrate
(kg/s) and pressure change / pressure loss (Pa) can be computed lives here:

- [`FluidComponentTrait`] — get/set mass flowrate, pressure loss, geometry
  (cross-sectional area in m^2, hydraulic diameter in m, length in m,
  incline angle), fluid properties (density in kg/m^3, viscosity in Pa.s at
  a reference temperature), and internal pressure source (Pa); with default
  methods relating pressure change, pressure loss, and hydrostatic head.
- [`FluidComponentCollectionMethods`] — pressure change / loss and mass
  flowrate for a whole collection.
- [`FluidPipeCalcPressureLoss`] / [`FluidPipeCalcPressureChange`] — Churchill
  friction-factor pipe correlations (via Reynolds and Bejan numbers).
- [`FluidCustomComponentCalcPressureLoss`] /
  [`FluidCustomComponentCalcPressureChange`] — the same for components with a
  user-supplied custom Darcy-friction / form-loss correlation.

Sign convention: `pressure_change = -pressure_loss +
hydrostatic_pressure_change + internal_pressure_source`; gravity is taken as
9.81 m/s^2 and hydrostatic head as `rho * g * L * sin(incline_angle)`.

```rust
pub mod fluid_component_traits { /* ... */ }
```

### Modules

## Module `unit_test_fluid_component_traits`

unit tests for fluid_component traits

```rust
pub mod unit_test_fluid_component_traits { /* ... */ }
```

### Traits

#### Trait `FluidComponentTrait`

This is a generic fluid component trait,
which specifies that fluid components in general
should have the following properties accessed
via get and set methods

```rust
pub trait FluidComponentTrait {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `get_mass_flowrate`: gets the mass flowrate of the component
- `set_mass_flowrate`: sets the mass flowrate of the component
- `get_mass_flowrate_from_pressure_loss_immutable`: gets the mass flowrate of component given a
- `get_pressure_loss`: gets pressure loss
- `set_pressure_loss`: sets the pressure loss of the component
- `get_pressure_loss_immutable`: gets the pressure loss of component given a
- `get_cross_sectional_area`: gets cross sectional area
- `get_cross_sectional_area_immutable`: gets cross sectional area with immutable instance of self
- `get_hydraulic_diameter`: gets hydraulic diamter
- `get_hydraulic_diameter_immutable`: gets hydraulic diamter with immutable instance of self
- `get_fluid_viscosity_at_ref_temperature`: gets fluid viscosity at some user set reference temperature
- `get_fluid_viscosity_immutable_at_ref_temperature`:  gets fluid viscosity with an immutable instance of self
- `get_fluid_density_at_ref_temperature`:  gets fluid density
- `get_fluid_density_immutable_at_ref_temperature`:  gets fluid density with an immutable instance of self
- `get_component_length`: gets the component length
- `get_component_length_immutable`: gets the component length immutably
- `get_incline_angle`: gets the angle of incline for a pipe
- `get_incline_angle_immutable`: gets the incline angle of the pipe with immutable self
- `get_internal_pressure_source`: gets the pressure source for a fluid component
- `get_internal_pressure_source_immutable`: gets the pressure source for a fluid component
- `set_internal_pressure_source`: sets the internal pressure source for a pipe

##### Provided Methods

- ```rust
  fn get_mass_flowrate_from_pressure_change_immutable(self: &Self, pressure_change: Pressure) -> MassRate { /* ... */ }
  ```
  gets the mass flowrate of component given a

- ```rust
  fn get_pressure_change(self: &mut Self) -> Pressure { /* ... */ }
  ```
  gets pressure change for a pipe given

- ```rust
  fn get_pressure_change_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
  ```
  gets the pressure loss of component given a

- ```rust
  fn set_pressure_change(self: &mut Self, pressure_change: Pressure) { /* ... */ }
  ```
  sets the pressure change for the given pipe

- ```rust
  fn get_hydrostatic_pressure_change_at_ref_temperature(self: &mut Self) -> Pressure { /* ... */ }
  ```
  gets the hydrostatic pressure change

- ```rust
  fn get_hydrostatic_pressure_change_immutable_at_ref_temperature(self: &Self) -> Pressure { /* ... */ }
  ```
  gets the hydrostatic pressure change

##### Implementations

This trait is implemented for the following types:

- `FluidComponent`
- `super::NonInsulatedFluidComponent`
- `super::InsulatedFluidComponent`
- `super::NonInsulatedParallelFluidComponent`
- `super::HeaterTopBottomHead`
- `super::InsulatedPorousMediaFluidComponent`
- `super::NonInsulatedPorousMediaFluidComponent`

#### Trait `FluidComponentCollectionMethods`

contains methods to get pressure loss
and pressure change and mass flowrate based on
current state of the fluid component collection

```rust
pub trait FluidComponentCollectionMethods {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `get_pressure_change`: calculates pressure change when given a mass flowrate
- `get_mass_flowrate_from_pressure_change`: calculates mass flowrate from pressure change

##### Provided Methods

- ```rust
  fn get_pressure_loss(self: &Self, fluid_mass_flowrate: MassRate) -> Pressure { /* ... */ }
  ```
  calculates pressure loss when given a mass flowrate

- ```rust
  fn get_mass_flowrate_from_pressure_loss(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
  ```
  calculates mass flowrate from pressure loss

#### Trait `FluidPipeCalcPressureLoss`

provides generic methods to calculate mass flowrate
and pressure losses for pipes

see FluidComponent example for how to use

```rust
pub trait FluidPipeCalcPressureLoss {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `get_pipe_form_loss_k`: gets form loss k for a pipe
- `get_pipe_form_loss_k_immutable`: gets form loss k for a pipe
- `get_pipe_absolute_roughness`: gets absolute roughness for a pipe
- `get_pipe_absolute_roughness_immutable`: gets absolute roughness for a pipe

##### Provided Methods

- ```rust
  fn pipe_calc_pressure_loss(fluid_mass_flowrate: MassRate, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, pipe_length: Length, absolute_roughness: Length, form_loss_k: f64) -> Result<Pressure, TuasLibError> { /* ... */ }
  ```
  a function calculates pressure

- ```rust
  fn pipe_calc_mass_flowrate(pressure_loss: Pressure, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, pipe_length: Length, absolute_roughness: Length, form_loss_k: f64) -> Result<MassRate, TuasLibError> { /* ... */ }
  ```
  a function which calculates pressure

#### Trait `FluidPipeCalcPressureChange`

provides generic methods to calculate pressure change
to and from mass flowrate for an Inclined pipe
with some internal pressure source (eg. pump)



```rust
pub trait FluidPipeCalcPressureChange: FluidPipeCalcPressureLoss + FluidComponentTrait {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Provided Methods

- ```rust
  fn pipe_calc_pressure_change(fluid_mass_flowrate: MassRate, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, pipe_length: Length, absolute_roughness: Length, form_loss_k: f64, incline_angle: Angle, source_pressure: Pressure) -> Result<Pressure, TuasLibError> { /* ... */ }
  ```
  calculates a pressure change of the pipe

- ```rust
  fn pipe_calculate_mass_flowrate_from_pressure_change(pressure_change: Pressure, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, pipe_length: Length, absolute_roughness: Length, form_loss_k: f64, incline_angle: Angle, source_pressure: Pressure) -> Result<MassRate, TuasLibError> { /* ... */ }
  ```
  calculates a mass flowrate given a pressure change

- ```rust
  fn get_hydrostatic_pressure_change(pipe_length: Length, incline_angle: Angle, fluid_density: MassDensity) -> Pressure { /* ... */ }
  ```
  calculates hydrostatic pressure change

#### Trait `FluidCustomComponentCalcPressureLoss`

provides generic methods to calculate pressure
loss for a custom fluid component (with flow flowing
inside it)
given a custom darcy friction factor and
custom form loss correlation

```rust
pub trait FluidCustomComponentCalcPressureLoss {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `get_custom_loss_correlations`: returns the custom form loss factors
- `get_custom_loss_correlations_immutable`: returns the custom loss correlations
- `set_custom_loss_correlations`: sets the custom darcy friction factor function
- `get_custom_component_absolute_roughness`: gets the component absolute roughness for
- `get_custom_component_absolute_roughness_immutable`: gets the custom component absolute roughness

##### Provided Methods

- ```rust
  fn fluid_custom_component_calc_pressure_loss(fluid_mass_flowrate: MassRate, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, loss_correlation: DimensionlessDarcyLossCorrelations) -> Result<Pressure, TuasLibError> { /* ... */ }
  ```
  calculates pressure loss for a component given

- ```rust
  fn fluid_custom_component_calc_mass_flowrate_from_pressure_loss(pressure_loss: Pressure, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, loss_correlation: DimensionlessDarcyLossCorrelations) -> Result<MassRate, TuasLibError> { /* ... */ }
  ```
  calculates mass flowrate using input parameters

#### Trait `FluidCustomComponentCalcPressureChange`

Contains default implementations for calculating
mass flowrate from pressure change and vice versea

refer to examples in fluid_component_calculation
to see how its used

```rust
pub trait FluidCustomComponentCalcPressureChange: FluidCustomComponentCalcPressureLoss + FluidComponentTrait {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Provided Methods

- ```rust
  fn fluid_custom_component_calc_pressure_change(fluid_mass_flowrate: MassRate, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, component_length: Length, incline_angle: Angle, source_pressure: Pressure, loss_correlation: DimensionlessDarcyLossCorrelations) -> Result<Pressure, TuasLibError> { /* ... */ }
  ```
  calculates the pressure change for a custom

- ```rust
  fn fluid_custom_component_calc_mass_flowrate_from_pressure_change(pressure_change: Pressure, cross_sectional_area: Area, hydraulic_diameter: Length, fluid_viscosity: DynamicViscosity, fluid_density: MassDensity, component_length: Length, incline_angle: Angle, source_pressure: Pressure, loss_correlation: DimensionlessDarcyLossCorrelations) -> Result<MassRate, TuasLibError> { /* ... */ }
  ```
  calculates the mass flowrate given pressure change

- ```rust
  fn get_hydrostatic_pressure_change(pipe_length: Length, incline_angle: Angle, fluid_density: MassDensity) -> Pressure { /* ... */ }
  ```
  calculates hydrostatic pressure change

## Module `collection_series_and_parallel_functions`

contains functions which calculate mass flowrate and pressure drop
for components connected in series or parallel

```rust
pub mod collection_series_and_parallel_functions { /* ... */ }
```

### Traits

#### Trait `FluidComponentCollectionSeriesAssociatedFunctions`

contains associated functions which take a fluid component
vector and calculate mass flowrates and pressure changes
and losses from it

this assumes that all the components in the vector
are connected in series


note that the iterative methods of finding mass flowrate from pressure change
for this is EXPERIMENTAL,
convergence is not guaranteed.
Use at your own risk

```rust
pub trait FluidComponentCollectionSeriesAssociatedFunctions {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Provided Methods

- ```rust
  fn calculate_pressure_change_from_mass_flowrate(mass_flowrate: MassRate, fluid_component_vector: &Vec<FluidComponent>) -> Pressure { /* ... */ }
  ```
  calculates pressure change from mass flowrate

- ```rust
  fn calculate_mass_flowrate_from_pressure_change(pressure_change: Pressure, fluid_component_vector: &Vec<FluidComponent>) -> MassRate { /* ... */ }
  ```
  calculates mass flowrate from pressure change

##### Implementations

This trait is implemented for the following types:

- `FluidComponentCollection`

#### Trait `FluidComponentCollectionParallelAssociatedFunctions`

contains associated functions which take a fluid component
vector and calculate mass flowrates and pressure changes
and losses from it

this assumes that all the components in the vector
are connected in parallel

note that the iterative methods for finding pressure change across
the parallel branches given a mass flowrate are EXPERIMENTAL
use the associated functions at your own risk,

stability is not guarenteed

```rust
pub trait FluidComponentCollectionParallelAssociatedFunctions {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Provided Methods

- ```rust
  fn calculate_mass_flowrate_from_pressure_change(pressure_change: Pressure, fluid_component_vector: &Vec<FluidComponent>) -> MassRate { /* ... */ }
  ```
  calculates mass flowrate given a pressure change

- ```rust
  fn calculate_pressure_change_from_mass_flowrate(mass_flowrate: MassRate, fluid_component_vector: &Vec<FluidComponent>) -> Pressure { /* ... */ }
  ```
  calculates pressure change given a mass

- ```rust
  fn calculate_pressure_change_using_guessed_branch_mass_flowrate(guess_average_mass_flowrate: MassRate, user_specified_mass_flowrate: MassRate, fluid_component_vector: &Vec<FluidComponent>) -> Pressure { /* ... */ }
  ```
  calculates pressure change at user specified mass flowrate

- ```rust
  fn obtain_pressure_estimate_vector(mass_flowrate: MassRate, fluid_component_vector: &Vec<FluidComponent>) -> Vec<Pressure> { /* ... */ }
  ```
  This function takes a mass flowrate and applies it to each

- ```rust
  fn obtain_pressure_loss_estimate_vector(mass_flowrate: MassRate, fluid_component_vector: &Vec<FluidComponent>) -> Vec<Pressure> { /* ... */ }
  ```
  This function takes a mass flowrate and applies it to each

- ```rust
  fn obtain_maximum_pressure_from_vector(pressure_vector: &Vec<Pressure>) -> Pressure { /* ... */ }
  ```
  this function returns the maximum pressure change within

- ```rust
  fn obtain_minimum_pressure_from_vector(pressure_vector: &Vec<Pressure>) -> Pressure { /* ... */ }
  ```
  this function returns the minimum pressure change within

- ```rust
  fn obtain_average_pressure_from_vector(pressure_vector: &Vec<Pressure>) -> Pressure { /* ... */ }
  ```
  this function returns the arithmetic mean (average) pressure change

##### Implementations

This trait is implemented for the following types:

- `FluidComponentCollection`

## Module `super_collection_series_and_parallel_functions`

contains functions which calculate mass flowrate and pressure drop
for branches or fluid component collections connected in series or parallel

```rust
pub mod super_collection_series_and_parallel_functions { /* ... */ }
```

### Traits

#### Trait `FluidComponentSuperCollectionSeriesAssociatedFunctions`

contains associated functions which take a fluid component collection
vector and calculate mass flowrates and pressure changes
and losses from it

this assumes that all the components collections in the vector
are connected in series

note that the iterative methods of finding mass flowrate from pressure change
for this is EXPERIMENTAL,
convergence is not guaranteed.
Use at your own risk

```rust
pub trait FluidComponentSuperCollectionSeriesAssociatedFunctions {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Provided Methods

- ```rust
  fn calculate_pressure_change_from_mass_flowrate(mass_flowrate: MassRate, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> Pressure { /* ... */ }
  ```
  calculates pressure change from mass flowrate

- ```rust
  fn calculate_mass_flowrate_from_pressure_change(pressure_change: Pressure, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> MassRate { /* ... */ }
  ```
  calculates mass flowrate from pressure change

##### Implementations

This trait is implemented for the following types:

- `FluidComponentSuperCollection`

#### Trait `FluidComponentSuperCollectionParallelAssociatedFunctions`

contains associated functions which take a fluid component
vector and calculate mass flowrates and pressure changes
and losses from it

this assumes that all the components in the vector
are connected in parallel

note that the iterative methods for finding pressure change across
the parallel branches given a mass flowrate are EXPERIMENTAL
use the associated functions at your own risk,

stability is not guarenteed

```rust
pub trait FluidComponentSuperCollectionParallelAssociatedFunctions {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Provided Methods

- ```rust
  fn calculate_mass_flowrate_from_pressure_change(pressure_change: Pressure, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> MassRate { /* ... */ }
  ```
  calculates mass flowrate given a pressure change

- ```rust
  fn calculate_pressure_change_from_mass_flowrate(mass_flowrate: MassRate, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> Pressure { /* ... */ }
  ```
  calculates pressure change given a mass

- ```rust
  fn calculate_maximum_pressure_difference_between_branches(individual_branch_guess_average_mass_flowrate: MassRate, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> Pressure { /* ... */ }
  ```
  given a fixed flowrate through each branch,

- ```rust
  fn calculate_maximum_mass_flowrate_given_pressure_drop_across_each_branch(pressure_drop: Pressure, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> MassRate { /* ... */ }
  ```
  calculates maximum mass flowrate given a pressure drop across

- ```rust
  fn calculate_pressure_change_using_guessed_branch_mass_flowrate(individual_branch_guess_upper_bound_mass_flowrate: MassRate, user_specified_mass_flowrate: MassRate, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> Pressure { /* ... */ }
  ```
  calculates pressure change at user specified mass flowrate

- ```rust
  fn obtain_pressure_estimate_vector(mass_flowrate: MassRate, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> Vec<Pressure> { /* ... */ }
  ```
  This function takes a mass flowrate and applies it to each

- ```rust
  fn obtain_pressure_loss_estimate_vector(mass_flowrate: MassRate, fluid_component_collection_vector: &Vec<FluidComponentCollection>) -> Vec<Pressure> { /* ... */ }
  ```
  This function takes a mass flowrate and applies it to each

- ```rust
  fn obtain_maximum_pressure_from_vector(pressure_vector: &Vec<Pressure>) -> Pressure { /* ... */ }
  ```
  this function returns the maximum pressure change within

- ```rust
  fn obtain_minimum_pressure_from_vector(pressure_vector: &Vec<Pressure>) -> Pressure { /* ... */ }
  ```
  this function returns the minimum pressure change within

- ```rust
  fn obtain_average_pressure_from_vector(pressure_vector: &Vec<Pressure>) -> Pressure { /* ... */ }
  ```
  this function returns the arithmetic mean (average) pressure change

##### Implementations

This trait is implemented for the following types:

- `FluidComponentSuperCollection`

## Module `fluid_component_collection`

fluid component collections
these are vectors of fluid components

```rust
pub mod fluid_component_collection { /* ... */ }
```

### Types

#### Struct `FluidComponentCollection`

a fluid component collection,
which contains fluid components stored into a vector
and should contain some methods for CRUD operations

Create
Read
Update
Delete


```rust
pub struct FluidComponentCollection {
    pub components: Vec<super::fluid_component::FluidComponent>,
    pub orientation: FluidComponentCollectionOreintation,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `components` | `Vec<super::fluid_component::FluidComponent>` | this vector is the collection of fluid components |
| `orientation` | `FluidComponentCollectionOreintation` | this decides if the components are connected in series<br>or parallel |

##### Implementations

###### Methods

- ```rust
  pub fn get_immutable_fluid_component_vector(self: &Self) -> Vec<FluidComponent> { /* ... */ }
  ```
  returns a copy of the fluid component vector

- ```rust
  pub fn set_fluid_component_vector(self: &mut Self, fluid_component_vector: Vec<FluidComponent>) { /* ... */ }
  ```
  sets the fluid component vector to a specific value

- ```rust
  pub fn add_fluid_component(self: &mut Self, fluid_component: FluidComponent) { /* ... */ }
  ```
  adds a fluid component to the collection

- ```rust
  pub fn remove_fluid_component(self: &mut Self, component_index: usize) -> Result<(), TuasLibError> { /* ... */ }
  ```
  removes a fluid component by index from the collection

- ```rust
  pub fn get_fluid_component(self: &Self, component_index: usize) -> Result<FluidComponent, TuasLibError> { /* ... */ }
  ```
  returns read only a pointer of the fluid component

- ```rust
  pub fn update_fluid_component(self: &mut Self, component_index: usize, fluid_component: FluidComponent) { /* ... */ }
  ```
  updates the fluid component at the specified

- ```rust
  pub fn new_series_component_collection() -> Self { /* ... */ }
  ```
  new empty series component collection

- ```rust
  pub fn new_parallel_component_collection() -> Self { /* ... */ }
  ```
  new empty parallel component collection

- ```rust
  pub fn try_clone_and_add_component<T: TryInto<FluidComponent> + Clone + Debug>(self: &mut Self, component: &T) -> Result<(), TuasLibError>
where
    <T as TryInto<FluidComponent>>::Error: Debug { /* ... */ }
  ```
  clones anything that can be converted (try)into a FluidComponent

- ```rust
  pub fn clone_and_add_component<T: Into<FluidComponent> + Clone>(self: &mut Self, component: &T) { /* ... */ }
  ```
  clones anything that can be converted into a FluidComponent

- ```rust
  pub fn empty_vector(self: &mut Self) -> Result<(), TuasLibError> { /* ... */ }
  ```
  empties the vector

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FluidComponentCollection { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **FluidComponentCollectionMethods**
  - ```rust
    fn get_pressure_change(self: &Self, fluid_mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_change(self: &Self, pressure_change: Pressure) -> MassRate { /* ... */ }
    ```

- **FluidComponentCollectionParallelAssociatedFunctions**
- **FluidComponentCollectionSeriesAssociatedFunctions**
- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &FluidComponentCollection) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `FluidComponentCollectionOreintation`

tells you whether the components in FluidComponentCollection
or FluidComponentSuperCollection are connected in series or parallel

```rust
pub enum FluidComponentCollectionOreintation {
    Parallel,
    Series,
}
```

##### Variants

###### `Parallel`

fluid components are connected in parallel
(they share the same pressure change; mass flowrates add up)

###### `Series`

fluid components are connected in series
(they share the same mass flowrate; pressure changes add up)

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
    fn clone(self: &Self) -> FluidComponentCollectionOreintation { /* ... */ }
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
    fn eq(self: &Self, other: &FluidComponentCollectionOreintation) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Traits

#### Trait `FluidComponentCollectionMethods`

contains methods to get pressure loss
and pressure change and mass flowrate based on
current state of the fluid component collection

```rust
pub trait FluidComponentCollectionMethods {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `get_pressure_change`: calculates pressure change when given a mass flowrate
- `get_mass_flowrate_from_pressure_change`: calculates mass flowrate from pressure change

##### Provided Methods

- ```rust
  fn get_pressure_loss(self: &Self, fluid_mass_flowrate: MassRate) -> Pressure { /* ... */ }
  ```
  calculates pressure loss when given a mass flowrate

- ```rust
  fn get_mass_flowrate_from_pressure_loss(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
  ```
  calculates mass flowrate from pressure loss

##### Implementations

This trait is implemented for the following types:

- `FluidComponentCollection`
- `FluidComponentSuperCollection`

## Module `fluid_component_super_collection`

fluid component super collections
these are vectors of fluid component collections
usually used for calculating multiple branches in parallel

```rust
pub mod fluid_component_super_collection { /* ... */ }
```

### Types

#### Struct `FluidComponentSuperCollection`

A struct containing a vector of fluid component collections

```rust
pub struct FluidComponentSuperCollection {
    pub fluid_component_super_vector: Vec<FluidComponentCollection>,
    pub orientation: FluidComponentCollectionOreintation,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fluid_component_super_vector` | `Vec<FluidComponentCollection>` | this vector contains a collection of fluid component collections<br>usually, these are in series |
| `orientation` | `FluidComponentCollectionOreintation` | orientation of the fluid component collections<br>are these in series or parallel |

##### Implementations

###### Methods

- ```rust
  pub fn get_immutable_vector(self: &Self) -> Vec<FluidComponentCollection> { /* ... */ }
  ```
  returns a copy of the fluid component collection vector

- ```rust
  pub fn set_vector(self: &mut Self, fluid_component_super_vector: Vec<FluidComponentCollection>) { /* ... */ }
  ```
  sets the fluid component collection vector to a specific value

- ```rust
  pub fn add_collection_to_vector(self: &mut Self, fluid_component_super_vector: Vec<FluidComponentCollection>, fluid_component_vector: FluidComponentCollection) { /* ... */ }
  ```
  adds a fluid component collection to the super collection

- ```rust
  pub fn remove_collection_by_index(self: &mut Self, fluid_component_super_vector: Vec<FluidComponentCollection>, component_index: usize) { /* ... */ }
  ```
  removes a fluid component collection by index from the super collection

- ```rust
  pub fn get_collection_by_index(self: &mut Self, component_index: usize) -> FluidComponentCollection { /* ... */ }
  ```
  returns read only a pointer of the fluid component collection

- ```rust
  pub fn update_collection_by_index(self: &mut Self, component_index: usize, fluid_component_super_vector: Vec<FluidComponentCollection>, fluid_component_collection: FluidComponentCollection) { /* ... */ }
  ```
  updates the fluid component collection at the specified

- ```rust
  pub fn set_orientation_to_series(self: &mut Self) { /* ... */ }
  ```
  sets the orientation to series

- ```rust
  pub fn set_orientation_to_parallel(self: &mut Self) { /* ... */ }
  ```
  sets the orientation to parallel

- ```rust
  pub fn get_mass_flowrate_across_each_parallel_branch(self: &Self, pressure_change_across_each_branch: Pressure) -> Vec<MassRate> { /* ... */ }
  ```
  obtains a vector of mass flowrates that occur across each branch

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FluidComponentSuperCollection { /* ... */ }
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

- **FluidComponentCollectionMethods**
  - ```rust
    fn get_pressure_change(self: &Self, fluid_mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_change(self: &Self, pressure_change: Pressure) -> MassRate { /* ... */ }
    ```

- **FluidComponentSuperCollectionParallelAssociatedFunctions**
- **FluidComponentSuperCollectionSeriesAssociatedFunctions**
- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &FluidComponentSuperCollection) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `tests_and_examples`

some examples which show how to use the functionality of the fluid
mechanics correlation libraries
Worked examples and unit tests for the fluid-component collections.

Each submodule is a runnable, self-contained example (also a `#[test]`)
demonstrating how to assemble components and solve for mass flowrate (kg/s)
or pressure change (Pa):

- [`air_pipe_example`] — a single straight copper air pipe via
  `FluidPipeCalcPressureLoss`.
- [`water_pipe_example`] — an inclined water pipe with an internal pump
  (pressure source) via `FluidPipeCalcPressureChange`.
- [`coriolis_flowmeter_example`] — a component with a custom friction-factor
  correlation `(f_darcy L/D + K) = 18 + 93000/Re^1.35`.
- [`concurrency_and_multithreading_example`] — the same component moved into
  threads with `Arc`/`Mutex`.
- [`collection_fluid_components_in_series`] /
  [`collection_fluid_components_in_parallel`] — many pipes in a
  `FluidComponentCollection`.
- [`super_collection_fluid_components_in_parallel`] — parallel branches of
  series collections (a `FluidComponentSuperCollection`, as in CIET).

```rust
pub mod tests_and_examples { /* ... */ }
```

### Modules

## Module `air_pipe_example`

Example 1:

This example shows how to create a simple pipe
using the FluidComponent and FluidPipeCalcPressureLoss,
traits

this is by no means the best way to do it, but its a start
remember to use the relevant imports in the fluid component
tests

it is made of copper, 1m long, 2 in in diameter

This does not take inclined angles into consideration yet

```rust
pub mod air_pipe_example { /* ... */ }
```

## Module `water_pipe_example`

Example 2:

We saw previously how to create an air pipe
now we shall make a slanted water pipe
with some internal pressure source (as if it had a pump attached
to it)

we shall improve on how we can create the pipes
to do so, we shall use the FluidComponent trait and the
FluidPipeCalcPressureChange trait


```rust
pub mod water_pipe_example { /* ... */ }
```

## Module `coriolis_flowmeter_example`

Example 3,

suppose now we have a coriolis flowmeter
with a custom friction factor correlation

(f_darcy L/D + K) = 18 + 93000/Re^1.35

we shall use water to push flow through this coriolis flowmeter

also, the programming is rather tedious
because of lifetimes, but this is one example of how it can be done

```rust
pub mod coriolis_flowmeter_example { /* ... */ }
```

## Module `concurrency_and_multithreading_example`

Example 4


Testing if fluid component structs can be put into threads with move closures

```rust
pub mod concurrency_and_multithreading_example { /* ... */ }
```

## Module `collection_fluid_components_in_series`

Example 5

fluid components in series

```rust
pub mod collection_fluid_components_in_series { /* ... */ }
```

## Module `collection_fluid_components_in_parallel`

Example 6

fluid components in parallel

```rust
pub mod collection_fluid_components_in_parallel { /* ... */ }
```

## Module `super_collection_fluid_components_in_parallel`

Example 7

a colletion of fluid component collections is known
as a super collection.

for example, we have three branches of fluid components connected
in series
They are in turn connected in parallel for CIET.
To facilitate calculations here, we have super collections

```rust
pub mod super_collection_fluid_components_in_parallel { /* ... */ }
```

## Module `pre_built_components`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

Module for pre-built-components
suitable for tuas_boussinesq_solver (single phase, negligble density changes
except for buoyancy)

It's dependent on all the other modules within the tuas_boussinesq_solver

You don't want to write everything from scratch right?
Pre-built components — the human-facing builder API of the solver.

This module is the top-level map of ready-to-use thermal-hydraulic
components. Each submodule bundles a fluid array and/or a solid array
(control-volume collections) together with the pre-wired conductances,
boundary conditions and correlations needed to advance them in time, so a
user can assemble a loop without hand-building control volumes.

Conventions used throughout these components: temperatures in kelvin (K)
or degrees Celsius (degC), mass flow rate in kilograms per second (kg/s),
heat input / power in watts (W), pressure and pressure drop in pascals
(Pa), thermal conductance in watts per kelvin (W/K), and lengths / diameters
in metres (m). All public signatures carry `uom` dimensioned quantities.

Module map:
- [`heat_transfer_entities`] — the `HeatTransferEntity` abstraction that
  unifies single/arrayed control volumes and boundary conditions so they can
  be connected by a user-specified heat-transfer interaction.
- [`non_insulated_fluid_components`] — bare (uninsulated) pipes / fluid
  components that exchange heat with an ambient temperature boundary.
- [`insulated_pipes_and_fluid_components`] — pipes / fluid components with a
  single insulation layer.
- [`non_insulated_parallel_fluid_components`] — banks of parallel identical
  tubes (e.g. the tube side of a heat exchanger or a cooler).
- [`shell_and_tube_heat_exchanger`] — 1D shell-and-tube heat exchanger.
- [`one_d_solid_structure`] — a standalone 1D solid conduction structure
  (e.g. a hollow cylinder) with no internal fluid.
- [`ciet_struct_supports`], [`ciet_heater_top_and_bottom_head_bare`] — CIET
  structural-support and heater end-piece heat structures.
- [`insulated_porous_media_fluid_components`],
  [`non_insulated_porous_media_fluid_components`] — pipes packed with an
  internal solid (packed bed / annular insert / static mixer / CIET heater).
- [`ciet_isothermal_test_components`],
  [`ciet_steady_state_natural_circulation_test_components`],
  [`ciet_three_branch_plus_dracs`] — pre-assembled CIET loop components for
  the isothermal, natural-circulation and full three-branch + DRACS tests.
- [`uw_madison_flibe_loop_components`] — components for the UW Madison FLiBe
  natural-circulation loop.
- `gfhr_pipe_tests` — test-only gFHR pipe/branch flow checks.

```rust
pub mod pre_built_components { /* ... */ }
```

### Modules

## Module `heat_transfer_entities`

HeatTransferEntity module

For practical reasons, using different functions to connect
control volumes of various types (whether singleCV or arrayed control
volumes) can be quite cumbersome

To help the user connect these, I classify (and abstract) all control
volumes and boundary conditions as HeatTransferEntity objects.

The basic use is that HeatTransferEntity objects are connected to
each other by a user specified heat transfer interaction

Enum layer that unifies thermal control volumes (CVs) and boundary
conditions (BCs) behind a single [`HeatTransferEntity`] type so a solver
can hold, link, advance, and interrogate either kind through one API.

Module map:
- [`cv_types`] — the [`cv_types::CVType`] enum wrapping the control-volume
  variants (single node, fluid array, solid array) and their `From`/
  `TryFrom` conversions.
- [`bc_types`] — convenience constructors that build boundary-condition
  [`HeatTransferEntity`] values (constant temperature in K, heat flux in
  W/m^2, heat addition in W, adiabatic).
- [`preprocessing`] — sets up a heat-transfer problem: linking entities via
  heat-transfer interactions (single CV–BC and CV–CV thermal connections /
  conductance links in W/K), setting mass flowrates in kg/s, and computing
  mesh-stability timesteps in seconds.
- [`calculation`] — advances a control volume by one timestep (in seconds),
  converting accumulated enthalpy-change rates into the next-timestep state.
- [`postprocessing`] — extracts temperatures (K) and densities (kg/m^3)
  from an entity.
- [`type_conversion`] — `Into`/`TryFrom`/`TryInto` between the concrete CV
  and BC types and [`HeatTransferEntity`].
- [`conversion_to_data_advection`] — builds a `DataAdvection` interaction
  from two heat transfer entities.
- [`tests`] — mixing-joint and CIET-heater verification tests.

```rust
pub mod heat_transfer_entities { /* ... */ }
```

### Modules

## Module `cv_types`

all the types of Control volumes are represented in an enum
to abstract away the complications of connecting different types
of control volumes.

```rust
pub mod cv_types { /* ... */ }
```

### Types

#### Enum `CVType`

Contains Types of Control Volumes (CVs)

```rust
pub enum CVType {
    SingleCV(crate::single_control_vol::SingleCVNode),
    FluidArrayCV(crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray),
    SolidArrayCV(crate::array_control_vol_and_fluid_component_collections::one_d_solid_array_with_lateral_coupling::SolidColumn),
}
```

##### Variants

###### `SingleCV`

This CV is the most basic,  it can be represented by a single
point or node

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::single_control_vol::SingleCVNode` |  |

###### `FluidArrayCV`

Array CVs are collections of SingleCVs,
or discretised arrays of control volumes with SingleCVNodes
attached to either end
but do not require the
user to manually specify the connections between the SingleCVs
This is for fluid arrays, where there is advection through
the array

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray` |  |

###### `SolidArrayCV`

Array CVs are collections of SingleCVs,
or discretised arrays of control volumes with SingleCVNodes
attached to either end
but do not require the
user to manually specify the connections between the SingleCVs
This is for solid arrays, where there is no advection through
the array

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::array_control_vol_and_fluid_component_collections::one_d_solid_array_with_lateral_coupling::SolidColumn` |  |

##### Implementations

###### Methods

- ```rust
  pub fn get_material(self: &mut Self) -> Result<Material, TuasLibError> { /* ... */ }
  ```
  gets the material

- ```rust
  pub fn get_temperature_vector(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  obtains the temperature vector for all CVTypes

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> CVType { /* ... */ }
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

  - ```rust
    fn from(single_cv: SingleCVNode) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(fluid_array: FluidArray) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(solid_array: SolidColumn) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CVType) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(value: CVType) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(value: CVType) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(value: CVType) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(value: HeatTransferEntity) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `bc_types`

converts to and from boundary conditions

```rust
pub mod bc_types { /* ... */ }
```

## Module `preprocessing`

preprocessing

this module contains abstraction pertaining
to how to set up a heat transfer problem

This means setting up the timestep, mass flowrates and how
heat transfer entities are linked to each other via heat
transfer interactions
Preprocessing: wiring heat transfer entities together before a timestep.

This module holds the dispatch logic that links [`HeatTransferEntity`]
values (control volumes and boundary conditions) via a
[`HeatTransferInteractionType`], pushing the resulting enthalpy-change
rates (W) onto each control volume and computing mesh-stability timesteps
(seconds). It handles all four link permutations: CV–CV, BC–CV, CV–BC, and
the unsupported BC–BC (which returns an error).

Module map:
- [`HeatTransferEntity`] convenience linkers (`link_to_front`,
  `link_to_back`, `link`) and `try_set_flowrate_for_fluid_array` (kg/s).
- [`link_heat_transfer_entity`] — the top-level dispatcher that routes a
  pair of entities to the correct connection routine and mutates them.
- `calculate_*_serial` helpers — per-combination heat-flow and timestep
  (seconds) calculators for CV–CV and CV–BC pairs.
- [`try_get_thermal_conductance_based_on_interaction`] — maps an
  interaction enum to a thermal conductance (W/K).
- [`single_cv_and_bc_interactions`] and
  [`single_cv_single_cv_interactions`] — the leaf routines for single-node
  connections.

```rust
pub mod preprocessing { /* ... */ }
```

### Modules

## Module `single_cv_and_bc_interactions`

contains code for heat transfer interactions between SingleCV
and boundary conditions

```rust
pub mod single_cv_and_bc_interactions { /* ... */ }
```

### Functions

#### Function `calculate_single_cv_node_front_constant_temperature_back`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates interaction for a single cv and a constant temperature BC

```rust
pub fn calculate_single_cv_node_front_constant_temperature_back(boundary_condition_temperature: ThermodynamicTemperature, control_vol: &mut crate::single_control_vol::SingleCVNode, interaction: super::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_single_cv_front_heat_flux_back`

calculates the interaction between a heat flux BC and
a control volume

(heat flux bc) ------------------ (single cv)

the cv is at the front
heat addition is at the back

```rust
pub fn calculate_single_cv_front_heat_flux_back(heat_flux_into_control_vol: HeatFluxDensity, control_vol: &mut crate::single_control_vol::SingleCVNode, interaction: super::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_constant_heat_addition_front_single_cv_back`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates the interaction between a heat addition BC and
a control volume

(single cv) ------------------ (heat addition bc)

the heat addition is at the front, the cv is at the back

```rust
pub fn calculate_constant_heat_addition_front_single_cv_back(control_vol: &mut crate::single_control_vol::SingleCVNode, heat_added_to_control_vol: Power, interaction: super::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_single_cv_front_constant_heat_addition_back`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates the interaction between a heat addition BC and
a control volume

(heat addition) ------------------ (single cv)

the heat addition is at the front, the cv is at the back

```rust
pub fn calculate_single_cv_front_constant_heat_addition_back(heat_added_to_control_vol: Power, control_vol: &mut crate::single_control_vol::SingleCVNode, interaction: super::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_mesh_stability_conduction_timestep_for_single_node_and_bc`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

calculates the relevant timestep for stability based on mesh size
between control volume and boundary conditions

```rust
pub fn calculate_mesh_stability_conduction_timestep_for_single_node_and_bc(control_vol: &mut crate::single_control_vol::SingleCVNode, interaction: super::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<Time, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `single_cv_single_cv_interactions`

contains code for heat transfer interactions between SingleCV and
other SingleCVs

```rust
pub mod single_cv_single_cv_interactions { /* ... */ }
```

### Functions

#### Function `calculate_between_two_singular_cv_nodes`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

which calls other functions depending on whether the
heat transfer interaction is conductance based on advection based

```rust
pub fn calculate_between_two_singular_cv_nodes(single_cv_1: &mut crate::single_control_vol::SingleCVNode, single_cv_2: &mut crate::single_control_vol::SingleCVNode, interaction: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_advection_interaction_between_two_singular_cv_nodes`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

for advection flows between two SingleCVNode objects,
and specified advection information,

this updates the heat transfer vector in both singlecv nodes

```rust
pub fn calculate_advection_interaction_between_two_singular_cv_nodes(single_cv_1: &mut crate::single_control_vol::SingleCVNode, single_cv_2: &mut crate::single_control_vol::SingleCVNode, advection_data: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::DataAdvection) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_conductance_interaction_between_two_singular_cv_nodes`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

if two singleCV nodes have a conductance or thermal resistance between
them, their temperature differentials and conductance are used to
calculate the heat flow between them.

```rust
pub fn calculate_conductance_interaction_between_two_singular_cv_nodes(single_cv_1: &mut crate::single_control_vol::SingleCVNode, single_cv_2: &mut crate::single_control_vol::SingleCVNode, interaction: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

### Functions

#### Function `link_heat_transfer_entity`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

For this part, we determine how heat_transfer_entities interact
with each other by using a function
The function will take in a HeatTransferInteractionType enum
which you must first initiate

Then you need to supply two control volumes or more generally
heat_transfer_entities, which can consist of mix of control volumes
and boundary conditions

The function will then calculate the heat transfer between the two
control volumes, and either return a value or mutate the CV objects
using mutable borrows

```rust
pub fn link_heat_transfer_entity(entity_1: &mut super::HeatTransferEntity, entity_2: &mut super::HeatTransferEntity, interaction: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_timescales_for_heat_transfer_entity`

this function calculates relevant timescales when linking
two heat transfer entities

```rust
pub fn calculate_timescales_for_heat_transfer_entity(entity_1: &mut super::HeatTransferEntity, entity_2: &mut super::HeatTransferEntity, interaction: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<Time, crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_constant_temperature_front_single_cv_back`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

for connecting a bc to cv where

(cv) ---------- (constant temperature bc)

```rust
pub fn calculate_constant_temperature_front_single_cv_back(control_vol: &mut crate::single_control_vol::SingleCVNode, boundary_condition_temperature: ThermodynamicTemperature, interaction: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

#### Function `calculate_constant_heat_flux_front_single_cv_back`

calculates the interaction between a heat flux BC and
a control volume

(single cv) ------------------ (heat flux bc)

the heat addition is at the front, the cv is at the back

```rust
pub fn calculate_constant_heat_flux_front_single_cv_back(control_vol: &mut crate::single_control_vol::SingleCVNode, heat_flux_into_control_vol: HeatFluxDensity, interaction: crate::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType) -> Result<(), crate::tuas_lib_error::TuasLibError> { /* ... */ }
```

## Module `postprocessing`

postprocessing contains functions to obtain temperature profiles
of the HeatTransferEntity

```rust
pub mod postprocessing { /* ... */ }
```

## Module `calculation`

calculation modules deal mainly with advancing timestep

```rust
pub mod calculation { /* ... */ }
```

## Module `type_conversion`

type conversion
converts underlying nested enums into HeatTransferEntity objects

```rust
pub mod type_conversion { /* ... */ }
```

## Module `conversion_to_data_advection`

convert to data_advection
that is to say, you can construct a DataAdvection struct from
a HeatTransferEntity

```rust
pub mod conversion_to_data_advection { /* ... */ }
```

### Types

#### Enum `HeatTransferEntity`

Contains entities which transfer heat and interact with each
other

for example, control volumes and boundary conditions

```rust
pub enum HeatTransferEntity {
    ControlVolume(self::cv_types::CVType),
    BoundaryConditions(crate::boundary_conditions::BCType),
}
```

##### Variants

###### `ControlVolume`

Contains a list of ControlVolumeTypes

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `self::cv_types::CVType` |  |

###### `BoundaryConditions`

Contains a list of Boundary conditions

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::boundary_conditions::BCType` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new_const_temperature_bc(temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructors for BCs (convenience)

- ```rust
  pub fn new_const_heat_flux_bc(heat_flux: HeatFluxDensity) -> Self { /* ... */ }
  ```
  creates a new constant heat flux bc

- ```rust
  pub fn new_const_heat_addition(heat_addition: Power) -> Self { /* ... */ }
  ```
  creates a new constant heat addition bc

- ```rust
  pub fn new_adiabatic_bc() -> Self { /* ... */ }
  ```
  creates a new adiabatic BC

- ```rust
  pub fn link_to_front(self: &mut Self, other_hte: &mut HeatTransferEntity, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  wrapper for linking, makes it easier to link

- ```rust
  pub fn link_to_back(self: &mut Self, other_hte: &mut HeatTransferEntity, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  wrapper for linking, makes it easier to link

- ```rust
  pub fn link(entity: &mut HeatTransferEntity, other_hte: &mut HeatTransferEntity, interaction: HeatTransferInteractionType) -> Result<(), TuasLibError> { /* ... */ }
  ```
  wrapper for linking, makes it easier to link

- ```rust
  pub fn try_set_flowrate_for_fluid_array(self: &mut Self, mass_flowrate: MassRate) -> Result<(), TuasLibError> { /* ... */ }
  ```
  for fluid arrays, it is important to have a method

- ```rust
  pub fn temperature(entity: &mut HeatTransferEntity) -> Result<ThermodynamicTemperature, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the HeatTransferEntity

- ```rust
  pub fn try_get_bulk_temperature(self: &mut Self) -> Result<ThermodynamicTemperature, TuasLibError> { /* ... */ }
  ```
  gets bulk temperature of the heat transfer entity

- ```rust
  pub fn temperature_vector(entity: &mut HeatTransferEntity) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets a vector of temperatures

- ```rust
  pub fn get_temperature_vector(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets temperature vector of this HeatTransferEntity

- ```rust
  pub fn density_vector(entity: &mut HeatTransferEntity) -> Result<Vec<MassDensity>, TuasLibError> { /* ... */ }
  ```
  density vector

- ```rust
  pub fn advance_timestep(entity: &mut HeatTransferEntity, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  for control volumes, this method allows you to

- ```rust
  pub fn advance_timestep_mut_self(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  for control volumes, this method allows you to

- ```rust
  pub fn advance_timestep_mut_self_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a handle to advance the timestep

- ```rust
  pub fn set(self: &mut Self, user_input_hte: HeatTransferEntity) -> Result<(), TuasLibError> { /* ... */ }
  ```
  allows the user to override the heat transfer entity

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> HeatTransferEntity { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> HeatTransferEntity { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeatTransferEntity) -> bool { /* ... */ }
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

  - ```rust
    fn try_from(hte: HeatTransferEntity) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(value: HeatTransferEntity) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<FluidArray, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<SolidColumn, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_into(self: Self) -> Result<SingleCVNode, <Self as >::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `non_insulated_fluid_components`

for fluid flow through non insulated pipes and fluid components, these pipes will
be represented by control volumes laterally coupled to one
another.
Non-insulated fluid components — bare pipes and fluid components that
exchange heat directly with an ambient boundary.

This module provides [`NonInsulatedFluidComponent`], a fluid component with
no insulation layer: a fluid array (the flowing coolant) coupled to a solid
pipe shell, which in turn loses (or gains) heat to an ambient-temperature
boundary through a user-supplied heat-transfer coefficient. Because there is
no insulation, these are the right choice when heat exchange with the
surroundings is intended — e.g. coolers, heaters, or piping whose heat loss
is being tracked.

The fluid-to-shell coupling uses a Nusselt-number correlation (Gnielinski by
default; the CIET heater correlation for the heater builder). Pressure drop
is set by a Darcy friction / form-loss correlation.

Units: temperatures in kelvin (K) or degrees Celsius (degC), mass flow rate
in kilograms per second (kg/s), heat input / power in watts (W), pressure
and pressure drop in pascals (Pa), thermal conductance in watts per kelvin
(W/K), heat-transfer coefficient in watts per square metre kelvin
(W/(m^2 K)), and lengths / diameters in metres (m). All public signatures
carry `uom` dimensioned quantities.

Submodules:
- [`preprocessing`] — build the ambient and fluid-to-shell conductances and
  wire the lateral / axial connections; Reynolds-number helper.
- [`fluid_component`] — `FluidComponentTrait` impl (mass flow ↔ pressure loss).
- [`calculation`] — advance the component one timestep.
- [`postprocessing`] — read back the shell and fluid nodal temperature vectors.
- [`type_conversion`] — convert into a `FluidComponent`.
- [`calibration`] — override the fluid Nusselt correlation.

```rust
pub mod non_insulated_fluid_components { /* ... */ }
```

### Modules

## Module `preprocessing`

stuff such as conductances are calculated here

```rust
pub mod preprocessing { /* ... */ }
```

## Module `fluid_component`

implementations for the FluidComponent trait
are done here

```rust
pub mod fluid_component { /* ... */ }
```

## Module `calculation`

stuff for calculation is done here, ie, advancing timestep

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

postprocessing stuff, ie, get the temperature vectors
of both arrays of control volumes

```rust
pub mod postprocessing { /* ... */ }
```

## Module `type_conversion`

type conversion, such as into fluid component and such

```rust
pub mod type_conversion { /* ... */ }
```

## Module `calibration`

calibration, for calibrating thickness or nusselt correlation
(incomplete)

```rust
pub mod calibration { /* ... */ }
```

### Types

#### Struct `NonInsulatedFluidComponent`

The simplest component is a non insulated pipe

This is a simple pipe with a set hydraulic diameter and length

the standard assumption is that at each boundary of this pipe,
there is no conduction heat transfer in the axial direction
TODO: the nusselt number correlations for the shell and tube side
are not yet capable/tested of handling nusselt number correlations other
than Gnielinski type correlations


```rust
pub struct NonInsulatedFluidComponent {
    pub pipe_shell: super::heat_transfer_entities::HeatTransferEntity,
    pub pipe_fluid_array: super::heat_transfer_entities::HeatTransferEntity,
    pub ambient_temperature: ThermodynamicTemperature,
    pub heat_transfer_to_ambient: HeatTransfer,
    pub od: Length,
    pub id: Length,
    pub flow_area: Area,
    pub custom_component_loss_correlation: crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pipe_shell` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe shell which is<br>exposed to an ambient constant temperature boundary condition<br>This is because constant heat flux BCs are not common for pipes<br><br>only one radial layer of control volumes is used to simulate<br>the pipe shell |
| `pipe_fluid_array` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe fluid<br>which is coupled to the pipe shell via a Nusselt Number based<br>thermal resistance (usually Gnielinski correlation) |
| `ambient_temperature` | `ThermodynamicTemperature` | pipe ambient temperature |
| `heat_transfer_to_ambient` | `HeatTransfer` | pipe heat transfer coefficient to ambient |
| `od` | `Length` | pipe  outer diameter |
| `id` | `Length` | pipe inner diameter |
| `flow_area` | `Area` | flow area |
| `custom_component_loss_correlation` | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations` | loss correlation |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn lateral_and_miscellaneous_connections_no_wall_correction(self: &mut Self, mass_flowrate: MassRate, heater_power: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn lateral_and_miscellaneous_connections_wall_correction(self: &mut Self, mass_flowrate: MassRate, heater_power: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn lateral_and_miscellaneous_connections(self: &mut Self, mass_flowrate: MassRate, heater_power: Power, correct_prandtl_for_wall_temperatures: bool) -> Result<(), TuasLibError> { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn get_air_shell_nodal_shell_conductance(self: &mut Self, h_air_to_pipe_surf: HeatTransfer) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains air to pipe shell conductance

- ```rust
  pub fn get_fluid_array_node_pipe_shell_conductance(self: &mut Self, correct_prandtl_for_wall_temperatures: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains fluid to pipe  shell conductance

- ```rust
  pub fn get_reynolds_based_on_hydraulic_diameter_and_flow_area(self: &Self, mass_flowrate: MassRate, temperature: ThermodynamicTemperature) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets the reynolds number based on mass flworate and

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self, mass_flowrate: MassRate, heater_steady_state_power: Power) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  advances timestep for each HeatTransferEntity within the

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances timestep by spawning a thread

- ```rust
  pub fn pipe_shell_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the pipe shell array

- ```rust
  pub fn pipe_fluid_array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the pipe fluid array

- ```rust
  pub fn calibrate_nusselt_correlation_for_fluid_within_pipe(self: &mut Self, nusselt_correlation_user_set: NusseltCorrelation) { /* ... */ }
  ```
  allows user to set nusselt correlation for the

- ```rust
  pub fn new_bare_pipe(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, fluid_pressure: Pressure, solid_pressure: Pressure, flow_area: Area, incline_angle: Angle, form_loss: Ratio, id: Length, od: Length, pipe_length: Length, hydraulic_diameter: Length, surface_roughness: Length, pipe_shell_material: SolidMaterial, pipe_fluid: LiquidMaterial, htc_to_ambient: HeatTransfer, user_specified_inner_nodes: usize) -> NonInsulatedFluidComponent { /* ... */ }
  ```
  constructs a new pipe

- ```rust
  pub fn new_dewet_model_heater_v2_no_twisted_tape(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  constructs a new heater v2 based on de wet's model,

- ```rust
  pub fn new_custom_component(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, fluid_pressure: Pressure, solid_pressure: Pressure, flow_area: Area, incline_angle: Angle, form_loss: Ratio, reynolds_coefficient: Ratio, reynolds_power: f64, shell_id: Length, shell_od: Length, component_length: Length, hydraulic_diameter: Length, pipe_shell_material: SolidMaterial, pipe_fluid: LiquidMaterial, htc_to_ambient: HeatTransfer, user_specified_inner_nodes: usize) -> NonInsulatedFluidComponent { /* ... */ }
  ```
  constructs a new non-insulated custom fluid component

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NonInsulatedFluidComponent { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **FluidComponentTrait**
  - ```rust
    fn get_mass_flowrate(self: &mut Self) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_loss_immutable(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area(self: &mut Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area_immutable(self: &Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_at_ref_temperature(self: &mut Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_immutable_at_ref_temperature(self: &Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_at_ref_temperature(self: &mut Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_immutable_at_ref_temperature(self: &Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_component_length(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_component_length_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle(self: &mut Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle_immutable(self: &Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source_immutable(self: &Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_internal_pressure_source(self: &mut Self, internal_pressure: Pressure) { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NonInsulatedFluidComponent) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `insulated_pipes_and_fluid_components`

for fluid flow through insulated and fluid components with one layer
of insulation, these pipes will
be represented by control volumes laterally coupled to one
another.
Insulated pipes and fluid components.

This module provides [`InsulatedFluidComponent`], a pre-built
thermal-hydraulic component modelling an insulated pipe (or a generic
insulated fluid component). It couples three one-dimensional
control-volume arrays sharing the same set of axial nodes:

1. a fluid array (the flowing coolant) — [`InsulatedFluidComponent::pipe_fluid_array`],
2. a solid pipe shell wrapped around the fluid, and
3. a solid insulation layer wrapped around the shell.

Radially, heat flows fluid -> shell -> insulation -> ambient. The fluid
-> shell coupling uses a Nusselt-number thermal resistance (typically the
Gnielinski correlation); the outermost insulation node exchanges heat with
a constant ambient-temperature boundary condition through a
heat-transfer-coefficient-to-ambient (units W/(m^2 K)). Axial (along-pipe)
conduction within the solid arrays is modelled and is the main source of
the small departures from the analytical log-mean-temperature-difference
solution documented in the tests.

Units throughout follow `uom`: temperatures in kelvin (or degC),
mass flow in kg/s, power/heat in watts, pressure drop in pascals,
thermal conductance in W/K, and lengths/thicknesses in metres.

Submodule map:
- [`preprocessing`] — constructors and thermal-connection / conductance
  setup (lateral coupling between the arrays and to ambient).
- [`calculation`] — advancing the component by one timestep (conduction,
  including axial conduction).
- [`calibration`] — adjusting insulation thickness, ambient
  heat-transfer coefficient, and Nusselt calibration.
- [`fluid_component`] — the `FluidComponentTrait` implementation
  (pressure drop in Pa versus mass flow in kg/s).
- [`postprocessing`] — extracting the temperature arrays (in K).
- [`type_conversion`] — conversions such as `Into<FluidComponent>`.
- `tests` — verification/validation tests (private).
- `tutorials` — worked user-guide examples (private).

```rust
pub mod insulated_pipes_and_fluid_components { /* ... */ }
```

### Modules

## Module `preprocessing`

stuff such as conductances are calculated here

```rust
pub mod preprocessing { /* ... */ }
```

## Module `fluid_component`

implementations for the FluidComponent trait
are done here

```rust
pub mod fluid_component { /* ... */ }
```

## Module `calculation`

stuff for calculation is done here, ie, advancing timestep

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

postprocessing stuff, ie, get the temperature vectors
of both arrays of control volumes

```rust
pub mod postprocessing { /* ... */ }
```

## Module `type_conversion`

type conversions such as TryInto<FluidComponent>

```rust
pub mod type_conversion { /* ... */ }
```

## Module `calibration`

calibration functions for heat transfer coefficients to ambient
nusselt number and insulation thickness

```rust
pub mod calibration { /* ... */ }
```

### Types

#### Struct `InsulatedFluidComponent`

The simplest component is an insulated pipe

This is a simple pipe with a set hydraulic diameter and length

the standard assumption is that at each boundary of this pipe,
there is no conduction heat transfer in the axial direction

```rust
pub struct InsulatedFluidComponent {
    pub pipe_shell: super::heat_transfer_entities::HeatTransferEntity,
    pub pipe_fluid_array: super::heat_transfer_entities::HeatTransferEntity,
    pub insulation: super::heat_transfer_entities::HeatTransferEntity,
    pub ambient_temperature: ThermodynamicTemperature,
    pub heat_transfer_to_ambient: HeatTransfer,
    pub tube_od: Length,
    pub tube_id: Length,
    pub darcy_loss_correlation: crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pipe_shell` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe shell<br>only one radial layer of control volumes is used to simulate<br>the pipe shell<br><br>it is thermally coupled to insulation and to the fluid<br>in the pipe_fluid_array |
| `pipe_fluid_array` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe fluid<br>which is coupled to the pipe shell via a Nusselt Number based<br>thermal resistance (usually Gnielinski correlation) |
| `insulation` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe insulation<br><br>which is<br>exposed to an ambient constant temperature boundary condition<br>This is because constant heat flux BCs are not common for pipes<br>except for fully/ideally insulated pipes<br><br>this is coupled to the pipe_shell |
| `ambient_temperature` | `ThermodynamicTemperature` | pipe ambient temperature |
| `heat_transfer_to_ambient` | `HeatTransfer` | pipe heat transfer coefficient to ambient |
| `tube_od` | `Length` | pipe outer diameter (tube) |
| `tube_id` | `Length` | pipe inner diameter (tube) |
| `darcy_loss_correlation` | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations` | loss correlations |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn lateral_and_miscellaneous_connections_no_wall_correction(self: &mut Self, mass_flowrate: MassRate, heater_power: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  Wrapper fn:

- ```rust
  pub fn lateral_and_miscellaneous_connections_wall_correction(self: &mut Self, mass_flowrate: MassRate, heater_power: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  Wrapper fn:

- ```rust
  pub fn lateral_and_miscellaneous_connections(self: &mut Self, mass_flowrate: MassRate, heater_power: Power, correct_prandtl_for_wall_temperatures: bool) -> Result<(), TuasLibError> { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn get_ambient_surroundings_to_insulation_nodalised_thermal_conductance(self: &mut Self, h_air_to_pipe_surf: HeatTransfer) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains air to insulation shell conductance

- ```rust
  pub fn get_fluid_array_node_to_pipe_shell_conductance(self: &mut Self, correct_prandtl_for_wall_temperatures: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains fluid_array node to pipe_shell shell conductance

- ```rust
  pub fn get_reynolds_based_on_hydraulic_diameter_and_flow_area(self: &Self, mass_flowrate: MassRate, temperature: ThermodynamicTemperature) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets the reynolds number based on mass flworate and

- ```rust
  pub fn get_pipe_shell_to_insulation_nodal_conductance(self: &Self) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains pipe shell to insulation conductance

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self, mass_flowrate: MassRate, heater_steady_state_power: Power) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  advances the timestep for each HeatTransferEntity within this

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances the timestep by cloning this component, moving the clone

- ```rust
  pub fn pipe_shell_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the pipe shell array

- ```rust
  pub fn pipe_fluid_array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the pipe fluid array

- ```rust
  pub fn insulation_array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  insulation temperature array

- ```rust
  pub fn calibrate_insulation_thickness(self: &mut Self, insulation_thickness: Length) { /* ... */ }
  ```
  calibrates the insulation thickness of this pipe or component,

- ```rust
  pub fn get_insulation_thickness(self: &Self) -> Length { /* ... */ }
  ```
  gets the insulation thickness based on

- ```rust
  pub fn calibrate_heat_transfer_to_ambient(self: &mut Self, ambient_htc: HeatTransfer) { /* ... */ }
  ```
  calibrates the heat transfer coefficient to ambient

- ```rust
  pub fn try_calibrate_gnielinski_nusselt(self: &mut Self, calibration_ratio: Ratio) -> Result<(), TuasLibError> { /* ... */ }
  ```
  tries to calibrate the Gnielinski Nusselt number correlation of the

- ```rust
  pub fn new_insulated_pipe(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, fluid_pressure: Pressure, solid_pressure: Pressure, flow_area: Area, incline_angle: Angle, form_loss: Ratio, shell_id: Length, shell_od: Length, insulation_thickness: Length, pipe_length: Length, hydraulic_diameter: Length, pipe_shell_material: SolidMaterial, insulation_material: SolidMaterial, pipe_fluid: LiquidMaterial, htc_to_ambient: HeatTransfer, user_specified_inner_nodes: usize, surface_roughness: Length) -> InsulatedFluidComponent { /* ... */ }
  ```
  constructs a new insulated pipe

- ```rust
  pub fn new_custom_component(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, fluid_pressure: Pressure, solid_pressure: Pressure, flow_area: Area, incline_angle: Angle, form_loss: Ratio, reynolds_coefficient: Ratio, reynolds_power: f64, shell_id: Length, shell_od: Length, insulation_thickness: Length, component_length: Length, hydraulic_diameter: Length, pipe_shell_material: SolidMaterial, insulation_material: SolidMaterial, pipe_fluid: LiquidMaterial, htc_to_ambient: HeatTransfer, user_specified_inner_nodes: usize) -> InsulatedFluidComponent { /* ... */ }
  ```
  constructs a new insulated pipe

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> InsulatedFluidComponent { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **FluidComponentTrait**
  - ```rust
    fn get_mass_flowrate(self: &mut Self) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_loss_immutable(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area(self: &mut Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area_immutable(self: &Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_at_ref_temperature(self: &mut Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_immutable_at_ref_temperature(self: &Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_at_ref_temperature(self: &mut Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_immutable_at_ref_temperature(self: &Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_component_length(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_component_length_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle(self: &mut Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle_immutable(self: &Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source_immutable(self: &Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_internal_pressure_source(self: &mut Self, internal_pressure: Pressure) { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &InsulatedFluidComponent) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `non_insulated_parallel_fluid_components`

for fluid through through a series of parallel pipes
each with a uniform hydraulic diameter and length
usually used for heat exchangers
these are non insulated by default to maximise heat transfer rates

They are used to model the tube side of a heat exchanger (without
calculations for the shell side)

These are used in isolated DRACS loop calculations where the parallel
pipes are exposed to a boundary condition rather than a modelled tube
They can also be used to model coolers where parallel tubes are exposed
to a stream of colder air.

Non-insulated parallel fluid components: a bank of `n` identical,
uninsulated tubes/pipes in parallel (all sharing a common header) that
exchange heat with a constant-temperature ambient boundary.

Physically, a single representative tube is modelled with two coupled
control-volume arrays (fluid + pipe shell); the parallel bundle is
reproduced by scaling per-tube quantities by `number_of_tubes`. As a
`FluidComponent`, the bundle aggregates the parallel pressure drop (Pa)
against the total mass flow rate (kg/s) across all tubes. This is suited
to the tube side of a heat exchanger, or air-cooled pipes modelled as a
bundled array.

Module map:
- [`preprocessing`] — lateral/axial thermal connections and per-node
  conductances (W/K) between fluid, shell and ambient.
- [`calculation`] — advances the fluid and solid arrays one timestep,
  applying the parallel-tube (1/`number_of_tubes`) correction.
- [`fluid_component`] — the `FluidComponentTrait` impl aggregating
  pressure drop (Pa) vs total mass flow (kg/s) over the bundle.
- [`postprocessing`] — retrieves the fluid and shell temperature vectors
  (in kelvin).
- [`type_conversion`] — conversion into a `FluidComponent` enum variant.
- [`tests`] — verification tests for the parallel-tube treatment.

```rust
pub mod non_insulated_parallel_fluid_components { /* ... */ }
```

### Modules

## Module `preprocessing`

stuff such as conductances are calculated here

```rust
pub mod preprocessing { /* ... */ }
```

## Module `fluid_component`

implementations for the FluidComponent trait
are done here

```rust
pub mod fluid_component { /* ... */ }
```

## Module `calculation`

stuff for calculation is done here, ie, advancing timestep

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

postprocessing stuff, ie, get the temperature vectors
of both arrays of control volumes

```rust
pub mod postprocessing { /* ... */ }
```

## Module `type_conversion`

type conversion, such as into fluid component and such

```rust
pub mod type_conversion { /* ... */ }
```

## Module `tests`

verification tests for parallel tubing

```rust
pub mod tests { /* ... */ }
```

### Types

#### Struct `NonInsulatedParallelFluidComponent`

This is meant to simulate a parallel collection of non-insulated
pipes, exposed to some ambient temperature

this code is marked for change as we may use a separate
HeatTransferEntity struct to represent the parallel fluid arrays

This is good for the tube side of heat exchangers, or for air cooled
pipes modelled as bundled arrays

TODO: the nusselt number correlations for the shell and tube side
are not yet capable/tested of handling nusselt number correlations other
than Gnielinski type correlations


```rust
pub struct NonInsulatedParallelFluidComponent {
    pub pipe_shell: super::heat_transfer_entities::HeatTransferEntity,
    pub pipe_fluid_array: super::heat_transfer_entities::HeatTransferEntity,
    pub ambient_temperature: ThermodynamicTemperature,
    pub heat_transfer_to_ambient: HeatTransfer,
    pub single_tube_od: Length,
    pub single_tube_id: Length,
    pub single_tube_flow_area: Area,
    pub custom_component_loss_correlation: crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations,
    pub number_of_tubes: u32,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pipe_shell` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe shell which is<br>exposed to an ambient constant temperature boundary condition<br>This is because constant heat flux BCs are not common for pipes<br><br>only one radial layer of control volumes is used to simulate<br>the pipe shell |
| `pipe_fluid_array` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe fluid<br>which is coupled to the pipe shell via a Nusselt Number based<br>thermal resistance (usually Gnielinski correlation) |
| `ambient_temperature` | `ThermodynamicTemperature` | pipe ambient temperature |
| `heat_transfer_to_ambient` | `HeatTransfer` | pipe heat transfer coefficient to ambient |
| `single_tube_od` | `Length` | pipe outer diameter on a per tube bases |
| `single_tube_id` | `Length` | pipe inner diameter one a per tube basis |
| `single_tube_flow_area` | `Area` | flow area on a per tube basis |
| `custom_component_loss_correlation` | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations` | loss correlation on a per tube basis |
| `number_of_tubes` | `u32` | number of tubes in parallel<br>each pipe fluid array represents one tube only |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn lateral_and_miscellaneous_connections_no_wall_correction(self: &mut Self, mass_flowrate_over_all_tubes: MassRate, heater_power_over_all_tubes: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  wrapper function

- ```rust
  pub fn lateral_and_miscellaneous_connections_wall_correction(self: &mut Self, mass_flowrate_over_all_tubes: MassRate, heater_power_over_all_tubes: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  wrapper function

- ```rust
  pub fn lateral_and_miscellaneous_connections(self: &mut Self, mass_flowrate_over_all_tubes: MassRate, heater_power_over_all_tubes: Power, correct_prandtl_for_wall_temperatures: bool) -> Result<(), TuasLibError> { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn get_air_to_single_shell_nodal_shell_conductance(self: &mut Self, h_air_to_pipe_surf: HeatTransfer) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains air to pipe shell conductance

- ```rust
  pub fn get_single_tube_fluid_array_node_pipe_shell_conductance(self: &mut Self, correct_prandtl_for_wall_temperatures: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains fluid to pipe shell conductance

- ```rust
  pub fn get_reynolds_based_on_hydraulic_diameter_and_flow_area(self: &Self, mass_flowrate: MassRate, temperature: ThermodynamicTemperature) -> Result<Ratio, TuasLibError> { /* ... */ }
  ```
  gets the reynolds number based on mass flworate and

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self, mass_flowrate: MassRate, heater_steady_state_power: Power) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  advances timestep for each HeatTransferEntity within the

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances timestep by spawning a thread

- ```rust
  pub fn pipe_shell_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the pipe shell array

- ```rust
  pub fn pipe_fluid_array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the pipe fluid array

- ```rust
  pub fn new_bare_pipe_parallel_array(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, fluid_pressure: Pressure, solid_pressure: Pressure, flow_area: Area, incline_angle: Angle, form_loss: Ratio, id: Length, od: Length, pipe_length: Length, hydraulic_diameter: Length, surface_roughness: Length, pipe_shell_material: SolidMaterial, pipe_fluid: LiquidMaterial, htc_to_ambient: HeatTransfer, user_specified_inner_nodes: usize, number_of_parallel_tubes: u32) -> NonInsulatedParallelFluidComponent { /* ... */ }
  ```
  constructs a new pipe

- ```rust
  pub fn new_custom_component_parallel_array(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, fluid_pressure: Pressure, solid_pressure: Pressure, flow_area: Area, incline_angle: Angle, form_loss: Ratio, reynolds_coefficient: Ratio, reynolds_power: f64, shell_id: Length, shell_od: Length, component_length: Length, hydraulic_diameter: Length, pipe_shell_material: SolidMaterial, pipe_fluid: LiquidMaterial, htc_to_ambient: HeatTransfer, user_specified_inner_nodes: usize, number_of_parallel_tubes: u32) -> NonInsulatedParallelFluidComponent { /* ... */ }
  ```
  constructs a new non-insulated parallel bundle whose per-tube pressure

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NonInsulatedParallelFluidComponent { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **FluidComponentTrait**
  - ```rust
    fn get_mass_flowrate(self: &mut Self) -> MassRate { /* ... */ }
    ```
    for getting and setting mass flowrate, we multiply or divide by

  - ```rust
    fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_loss_immutable(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area(self: &mut Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area_immutable(self: &Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_at_ref_temperature(self: &mut Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_immutable_at_ref_temperature(self: &Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_at_ref_temperature(self: &mut Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_immutable_at_ref_temperature(self: &Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_component_length(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_component_length_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle(self: &mut Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle_immutable(self: &Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source_immutable(self: &Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_internal_pressure_source(self: &mut Self, internal_pressure: Pressure) { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NonInsulatedParallelFluidComponent) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `shell_and_tube_heat_exchanger`

This is code for 1D modelling of shell and tube heat exchangers

Single-pass, no-baffle, parallel-flow shell-and-tube heat exchanger.

This module models a bundle of parallel tubes (carrying the tube-side
fluid) running inside a shell (carrying the shell-side fluid), with a
solid tube-wall array coupling the two fluids and an optional outer
shell / insulation layer to ambient. The heat-transfer layout is:

tube fluid | inner tube wall | shell fluid | outer shell | (insulation) | ambient

The core type [`SimpleShellAndTubeHeatExchanger`] holds the control-volume
arrays and geometry; the submodules provide the behaviour:

- [`preprocessing`] — constructors and the lateral/axial thermal
  connections between the tube-side fluid, tube wall, shell-side fluid,
  outer shell and insulation (conductances in W/K).
- [`calculation`] — advancing the timestep (conduction, heat-transfer
  coefficients, parallel-tube treatment).
- [`calibration`] — tuning heat-transfer coefficients and geometry
  (thermal resistances in K/W, Nusselt numbers) against experimental data.
- [`fluid_component`] — `FluidComponent`-style accessors giving pressure
  drop (Pa) versus mass flow (kg/s) for the tube side and shell side.
- [`postprocessing`] — temperatures (K), heat duty (W), overall heat
  transfer coefficients (W/(m^2 K)) and heat-transfer areas (m^2).
- `type_conversion` — placeholder for `From`/`TryInto` conversions.
- [`tests`] — constructor, heat-transfer verification and the HITEC
  molten-salt to YD-325 oil validation sets (Du et al., 2018).

All physical quantities are `uom`-typed: temperatures in kelvin,
mass flow in kg/s, power/heat duty in watts, length in metres.

```rust
pub mod shell_and_tube_heat_exchanger { /* ... */ }
```

### Modules

## Module `preprocessing`

stuff such as conductances are calculated here

```rust
pub mod preprocessing { /* ... */ }
```

## Module `fluid_component`

implementations for the FluidComponent trait
are done here

unfortunately, we cannot treat this as a fluid component because
this is not a simple pipe

each fluid array must be treated as a fluid component in itself

```rust
pub mod fluid_component { /* ... */ }
```

## Module `calculation`

stuff for calculation is done here, ie, advancing timestep

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

postprocessing stuff, ie, get the temperature vectors
of both arrays of control volumes

```rust
pub mod postprocessing { /* ... */ }
```

## Module `type_conversion`

type conversion, such as into fluid component and such

```rust
pub mod type_conversion { /* ... */ }
```

## Module `calibration`

functions to help calibrate the shell and tube heat exchanger

```rust
pub mod calibration { /* ... */ }
```

## Module `tests`

verification and validation tests for parallel tubing
as well as constructors
Verification and validation tests for the shell-and-tube heat exchanger.

Covers constructors, simplified heat-transfer consistency checks, the
calibration helpers, and the Du et al. (2018) HITEC molten-salt to YD-325
oil validation sets. Each submodule groups one family of checks.

```rust
pub mod tests { /* ... */ }
```

### Modules

## Module `basic_postprocessing`

checks if basic things such as obtaining the overall
heat transfer coefficient and shell side area work okay

```rust
pub mod basic_postprocessing { /* ... */ }
```

## Module `heat_transfer_verification`

heat transfer verification
runs a series of simplified cases to check if heat
exchanger works correctly

```rust
pub mod heat_transfer_verification { /* ... */ }
```

## Module `hitec_molten_salt_to_yd325_du_heat_exchanger`

heat exchanger verification and validation
using Du's paper
Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P.
(2018). Investigation on heat transfer characteristics of
molten salt in a shell-and-tube heat exchanger. International
Communications in Heat and Mass Transfer, 96, 61-68.
Validation of the shell-and-tube heat exchanger against Du et al. (2018):
HITEC molten salt on the shell side transferring heat to YD-325 heat
transfer oil on the tube side, in a 19-tube single-pass exchanger.

Du, B. C., He, Y. L., Qiu, Y., Liang, Q., & Zhou, Y. P. (2018).
Investigation on heat transfer characteristics of molten salt in a
shell-and-tube heat exchanger. International Communications in Heat and
Mass Transfer, 96, 61-68.

Sets A/B/C sweep the salt volumetric flowrate (12.63 / 14.63 / 16.63
m^3/h) at roughly constant oil flow; the remaining submodules are
debugging cross-checks on dimensions, thermophysical properties and the
heat-transfer correlations.

```rust
pub mod hitec_molten_salt_to_yd325_du_heat_exchanger { /* ... */ }
```

### Modules

## Module `set_a`

shell and tube heat exchanger test set A,

This is where
salt volumetric flowrate is 12.63 m3/s
oil volumetic flowrate is 15.635 m3/s
temperatures of oil and salt are varied
from 74.49  - 90.41 C  (YD325 oil)
and 214.93 - 236.91 C (HITEC salt)
respectively

```rust
pub mod set_a { /* ... */ }
```

## Module `set_b`

shell and tube heat exchanger test set B,

This is where
salt volumetric flowrate is 14.63 m3/s
oil volumetic flowrate is 15.635 m3/s
temperatures of oil and salt are varied
from 74.49  - 90.41 C  (YD325 oil)
and 214.93 - 236.91 C (HITEC salt)
respectively

```rust
pub mod set_b { /* ... */ }
```

## Module `set_c`

shell and tube heat exchanger test set C,

This is where
salt volumetric flowrate is 16.63 m3/s
oil volumetic flowrate is 15.635 m3/s
temperatures of oil and salt are varied
from 74.49  - 90.41 C  (YD325 oil)
and 214.93 - 236.91 C (HITEC salt)
respectively

```rust
pub mod set_c { /* ... */ }
```

## Module `thermophsyical_property_checks`

in debugging, I suspected my prandtl number of hitec
salt was coded wrongly
This ensures things are coded correctly

```rust
pub mod thermophsyical_property_checks { /* ... */ }
```

## Module `dimension_checks`

in debugging, I suspected dimensions were
coded wrongly

```rust
pub mod dimension_checks { /* ... */ }
```

## Module `heat_transfer_correlations_checks`

in debugging, I suspected that heat transfer correlations
calculated the heat transfer coefficient wrongly
this is to ensure they are calculated right

```rust
pub mod heat_transfer_correlations_checks { /* ... */ }
```

## Module `constructor_tests`

constructor tests

```rust
pub mod constructor_tests { /* ... */ }
```

## Module `calibration_functions`

calibration function tests

```rust
pub mod calibration_functions { /* ... */ }
```

### Types

#### Struct `SimpleShellAndTubeHeatExchanger`

Single pass, no baffle, parrallel flow shell and tube
heat exchanger

Nusselt correlations can be customised to empirically fit other
correlations

The axial sides are adiabatic unless otherwise stated


```rust
pub struct SimpleShellAndTubeHeatExchanger {
    pub inner_pipe_shell_array_for_single_tube: crate::pre_built_components::heat_transfer_entities::HeatTransferEntity,
    pub tube_side_fluid_array_for_single_tube: crate::pre_built_components::heat_transfer_entities::HeatTransferEntity,
    pub shell_side_fluid_array: crate::pre_built_components::heat_transfer_entities::HeatTransferEntity,
    pub outer_shell: crate::pre_built_components::heat_transfer_entities::HeatTransferEntity,
    pub ambient_temperature: ThermodynamicTemperature,
    pub heat_transfer_to_ambient: HeatTransfer,
    pub insulation_array: crate::pre_built_components::heat_transfer_entities::HeatTransferEntity,
    pub heat_exchanger_has_insulation: bool,
    pub tube_side_od: Length,
    pub tube_side_id: Length,
    pub tube_side_flow_area: Area,
    pub tube_side_custom_component_loss_correlation: crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations,
    pub shell_side_custom_component_loss_correlation: crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations,
    pub number_of_tubes: u32,
    pub shell_side_id: Length,
    pub shell_side_od: Length,
    pub shell_side_flow_area: Area,
    pub shell_side_nusselt_correlation_to_tubes: crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation,
    pub shell_side_nusselt_correlation_parasitic: crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation,
    pub tube_side_nusselt_correlation: crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation,
    pub insulation_thickness: Length,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inner_pipe_shell_array_for_single_tube` | `crate::pre_built_components::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe shell which<br>contains the tube side fluid<br>it is exposed to the shell side fluid |
| `tube_side_fluid_array_for_single_tube` | `crate::pre_built_components::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe fluid<br>which is coupled to the pipe shell via a Nusselt Number based<br>thermal resistance (usually Gnielinski correlation) |
| `shell_side_fluid_array` | `crate::pre_built_components::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe fluid<br>which is coupled to the pipe shell via a Nusselt Number based<br>thermal resistance, this must be specified by the user |
| `outer_shell` | `crate::pre_built_components::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe shell which<br>contains the shell side fluid and tube bundle<br>it is exposed to the insulation, or ambient temperature<br>depending on whether the insulation is toggled on or off |
| `ambient_temperature` | `ThermodynamicTemperature` | ambient temperature that the shell and tube heat<br>exchanger is exposed to |
| `heat_transfer_to_ambient` | `HeatTransfer` | heat transfer coefficient to ambient<br>This provides thermal resistance between the surface of<br>the shell and tube heat exchanger<br>This could be the outer shell or insulation, depending on whether<br>insulation is toggled on or off<br><br> |
| `insulation_array` | `crate::pre_built_components::heat_transfer_entities::HeatTransferEntity` | insulation array covering the<br>outer_shell array if insulation is toggled on |
| `heat_exchanger_has_insulation` | `bool` | this option allows the user to toggle on or off insulation |
| `tube_side_od` | `Length` | representative<br>tube outer diameter on a per tube bases |
| `tube_side_id` | `Length` | representative<br>tube inner diameter one a per tube basis |
| `tube_side_flow_area` | `Area` | representative tube flow area on a per tube basis |
| `tube_side_custom_component_loss_correlation` | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations` | loss correlation on a per tube basis |
| `shell_side_custom_component_loss_correlation` | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations` | loss correlation for shell side |
| `number_of_tubes` | `u32` | number of tubes in parallel<br>each pipe fluid array represents one tube only |
| `shell_side_id` | `Length` | assuming the outer shell is circular, provide the internal diameter |
| `shell_side_od` | `Length` | assuming the outer shell is circular, provide the outer diameter |
| `shell_side_flow_area` | `Area` | allows for a custom flow area for the shell side |
| `shell_side_nusselt_correlation_to_tubes` | `crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation` | allows user to set custom nusselt correlation for shell side<br>fluid to tubes |
| `shell_side_nusselt_correlation_parasitic` | `crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation` | allows user to set custom nusselt correlation for shell side<br>fluid to shell |
| `tube_side_nusselt_correlation` | `crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation` | allows the user to set custom nusselt correlation<br>for tube side fluid to tube |
| `insulation_thickness` | `Length` | specifies an thickness for the insulation covering<br>the shell side |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn lateral_and_miscellaneous_connections(self: &mut Self, prandtl_wall_correction_setting: bool, tube_side_total_mass_flowrate: MassRate, shell_side_total_mass_flowrate: MassRate) -> Result<(), TuasLibError> { /* ... */ }
  ```
  The shell and tube heat exchanger has two configurations,

- ```rust
  pub fn get_air_to_outer_sthe_layer_nodal_conductance(self: &mut Self, h_air_to_pipe_surf: HeatTransfer) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains air to shell and tube heat exchanger (sthe)

- ```rust
  pub fn get_single_tube_side_fluid_array_node_to_inner_pipe_shell_nodal_conductance(self: &mut Self, correct_prandtl_for_wall_temperatures: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains tube side fluid to pipe shell conductance

- ```rust
  pub fn get_shell_side_fluid_to_single_inner_pipe_shell_nodal_conductance(self: &mut Self, correct_prandtl_for_wall_temperatures: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains shell side fluid to *single* pipe shell conductance

- ```rust
  pub fn get_shell_side_fluid_to_outer_pipe_shell_nodal_conductance(self: &mut Self, correct_prandtl_for_wall_temperatures: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  this calculates the conductance on a per node basis

- ```rust
  pub fn get_outer_pipe_shell_to_insulation_conductance(self: &Self) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains outer pipe shell to insulation conductance

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self, prandtl_wall_correction_setting: bool, tube_side_total_mass_flowrate: MassRate, shell_side_total_mass_flowrate: MassRate) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn calibrate_insulation_thickness(self: &mut Self, insulation_thickness: Length) { /* ... */ }
  ```
  calibrates the insulation thickness of this pipe or component,

- ```rust
  pub fn get_insulation_thickness(self: &Self) -> Length { /* ... */ }
  ```
  gets the insulation thickness based on

- ```rust
  pub fn get_clone_of_shell_side_fluid_component(self: &Self) -> FluidComponent { /* ... */ }
  ```
  clones the shell side fluid array, and converts it into a

- ```rust
  pub fn get_clone_of_tube_side_parallel_tube_fluid_component(self: &Self) -> FluidComponent { /* ... */ }
  ```
  clones the tube side fluid array and converts it into

- ```rust
  pub fn set_tube_side_total_mass_flowrate(self: &mut Self, mass_flowrate_over_all_tubes: MassRate) { /* ... */ }
  ```
  sets the tube side mass flowrate

- ```rust
  pub fn set_shell_side_total_mass_flowrate(self: &mut Self, mass_flowrate_through_shell: MassRate) { /* ... */ }
  ```
  sets the tube side mass flowrate

- ```rust
  pub fn get_tube_side_hydraulic_diameter_circular_tube(self: &Self) -> Length { /* ... */ }
  ```
  returns tube side hydraulic diameter

- ```rust
  pub fn get_shell_side_hydraulic_diameter(self: &Self) -> Length { /* ... */ }
  ```
  returns the shell side hydraulic diameter

- ```rust
  pub fn get_effective_length(self: &Self) -> Length { /* ... */ }
  ```
  returns the effective length of the single pass sthe

- ```rust
  pub fn get_shell_side_cross_sectional_area(self: &Self) -> Area { /* ... */ }
  ```
  returns the shell side cross sectional area

- ```rust
  pub fn get_shell_side_fluid_thermal_conductivity(self: &Self) -> ThermalConductivity { /* ... */ }
  ```
  returns shell side thermal conductivity

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  advances timestep for each HeatTransferEntity within the

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances timestep by spawning a thread

- ```rust
  pub fn inner_pipe_shell_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the inner pipe shell array

- ```rust
  pub fn inner_tube_fluid_array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the tube side fluid array

- ```rust
  pub fn shell_side_fluid_array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the shell side fluid array temperature

- ```rust
  pub fn shell_side_outer_tube_array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the shell side outer tube temperature

- ```rust
  pub fn insulation_array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  gets the temperature of the insulation

- ```rust
  pub fn overall_htc_based_on_conductance(self: &mut Self, correct_for_prandtl_wall_temperatures: bool, tube_side_total_mass_flowrate: MassRate, shell_side_total_mass_flowrate: MassRate) -> HeatTransfer { /* ... */ }
  ```
  provides overall heat transfer coeff using conductance

- ```rust
  pub fn overall_heat_transfer_coeff_u_shell_side(self: &Self, correct_for_prandtl_wall_temperatures: bool) -> Result<HeatTransfer, TuasLibError> { /* ... */ }
  ```
  provides the overall heat transfer coefficient based on the

- ```rust
  pub fn nusselt_tube_side(self: &Self) -> Ratio { /* ... */ }
  ```
  provides nusselt number for tube side

- ```rust
  pub fn reynolds_tube_side_single_tube(self: &Self) -> Ratio { /* ... */ }
  ```
  returns reynolds number for tube side for a single tube

- ```rust
  pub fn bulk_prandtl_number_tube_side(self: &Self) -> (Ratio, Ratio) { /* ... */ }
  ```
  bulk and wall prandtl number for tube side

- ```rust
  pub fn nusselt_number_shell_side_to_tubes(self: &Self) -> Ratio { /* ... */ }
  ```
  provides nusselt number for shell side to tubes

- ```rust
  pub fn reynolds_shell_side(self: &Self) -> Ratio { /* ... */ }
  ```
  provides reynolds number for shell side (both to tubes and

- ```rust
  pub fn bulk_prandtl_number_shell_side(self: &Self) -> Ratio { /* ... */ }
  ```
  provides bulk prandtl number for shell side

- ```rust
  pub fn wall_prandtl_number_shell_side_fluid_for_inner_tube(self: &Self) -> Ratio { /* ... */ }
  ```
  provides wall prandtl number based on inner tube temperature

- ```rust
  pub fn wall_prandtl_number_shell_side_fluid_for_outer_tube(self: &Self) -> Ratio { /* ... */ }
  ```
  provides wall prandtl number based on outer tube temperature

- ```rust
  pub fn nusselt_number_shell_side_parasitic(self: &Self) -> Ratio { /* ... */ }
  ```
  provides nusselt number to outer shell

- ```rust
  pub fn circular_tube_bundle_heat_transfer_area_shell_side(self: &Self) -> Area { /* ... */ }
  ```
  provides the tube bundle side heat transfer area

- ```rust
  pub fn parasitic_heat_transfer_area_shell_side(self: &Self) -> Area { /* ... */ }
  ```
  provides the parasitic (shell-to-ambient) heat transfer area, in m^2

- ```rust
  pub fn circular_tube_bundle_heat_transfer_area_tube_side(self: &Self) -> Area { /* ... */ }
  ```
  provides the tube bundle heat transfer area on the tube side, in m^2

- ```rust
  pub fn try_get_insulation_cylindrical_thermal_resistance(self: &Self) -> Result<ThermalResistance, TuasLibError> { /* ... */ }
  ```
  assuming sthe insulation is cylindrical,

- ```rust
  pub fn get_outer_shell_cylindrical_thermal_resistance(self: &Self) -> ThermalResistance { /* ... */ }
  ```
  assuming sthe outer shell is cylindrical,

- ```rust
  pub fn get_inner_tubes_cylindrical_thermal_resistance(self: &Self) -> ThermalResistance { /* ... */ }
  ```
  assuming sthe inner tubes are cylindrical parallel tubes,

- ```rust
  pub fn get_convective_thermal_resistance_to_ambient(self: &Self) -> ThermalResistance { /* ... */ }
  ```
  assuming the outer shell or insulation is cylindrical,

- ```rust
  pub fn get_inner_tubes_convective_thermal_resistance_based_on_wetted_perimeter(self: &Self) -> ThermalResistance { /* ... */ }
  ```
  get inner tube thermal resistance using wetted perimeter

- ```rust
  pub fn get_shell_side_convective_thermal_resistance_cylindrical(self: &Self) -> ThermalResistance { /* ... */ }
  ```
  get shell side thermal resistance assuming cylindrical tubing

- ```rust
  pub fn get_shell_side_parasitic_convective_thermal_resistance_cylindrical(self: &Self) -> ThermalResistance { /* ... */ }
  ```
  get shell side thermal resistance assuming cylindrical tubing

- ```rust
  pub fn get_shell_side_heat_rate_based_on_mass_flowrate(self: &Self, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, shell_mass_flowrate: MassRate) -> Power { /* ... */ }
  ```
  obtains shell side heat gain or loss based on

- ```rust
  pub fn get_shell_side_heat_rate_based_on_vol_flowrate(self: &Self, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, vol_flowrate: VolumeRate) -> Power { /* ... */ }
  ```
  obtains shell side heat gain or loss based on

- ```rust
  pub fn get_tube_side_heat_rate_based_on_mass_flowrate(self: &Self, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, tube_mass_flowrate: MassRate) -> Power { /* ... */ }
  ```
  obtains tube side heat gain or loss based on

- ```rust
  pub fn get_tube_side_heat_rate_based_on_vol_flowrate(self: &Self, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, vol_flowrate: VolumeRate) -> Power { /* ... */ }
  ```
  obtains tube side heat gain or loss based on

- ```rust
  pub fn get_ua_based_on_mass_flowrates_and_temperature_differences(self: &Self, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, tube_mass_flowrate: MassRate, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, shell_mass_flowrate: MassRate, is_counter_current: bool) -> ThermalConductance { /* ... */ }
  ```
  gets the overall thermal resistance for heat

- ```rust
  pub fn get_ua_based_on_heat_transfer_and_temperature_differences(heat_transfer_rate: Power, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, is_counter_current: bool) -> ThermalConductance { /* ... */ }
  ```
  gets the overall thermal resistance for heat

- ```rust
  pub fn obtain_shell_side_nusselt_number_based_on_expt_data(self: &Self, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, tube_mass_flowrate: MassRate, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, shell_mass_flowrate: MassRate, is_counter_current: bool) -> Ratio { /* ... */ }
  ```
  obtain shell side Nusselt number based on prevailing flowrates

- ```rust
  pub fn obtain_shell_side_nusselt_number_based_on_expt_data_and_heat_rate(self: &Self, sthe_heat_transfer_rate: Power, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, is_counter_current: bool) -> Ratio { /* ... */ }
  ```
  obtain shell side Nusselt number based on prevailing flowrates

- ```rust
  pub fn obtain_tube_side_nusselt_number_based_on_expt_data(self: &Self, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, tube_mass_flowrate: MassRate, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, shell_mass_flowrate: MassRate, is_counter_current: bool) -> Ratio { /* ... */ }
  ```
  obtain tube side nusselt number based on prevailing flowrates

- ```rust
  pub fn obtain_parasitic_nusselt_number_based_on_expt_data(self: &Self, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, tube_mass_flowrate: MassRate, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, shell_mass_flowrate: MassRate) -> Ratio { /* ... */ }
  ```
  obtain parasitic heat loss nusselt number

- ```rust
  pub fn obtain_parasitic_thermal_resistance_based_on_expt_data(self: &Self, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, tube_mass_flowrate: MassRate, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, shell_mass_flowrate: MassRate) -> ThermalResistance { /* ... */ }
  ```
  gets thermal resistance for parasitic heat losses

- ```rust
  pub fn obtain_parasitic_heat_loss_rate_based_on_expt_data(self: &Self, tube_inlet_temperature: ThermodynamicTemperature, tube_outlet_temeprature: ThermodynamicTemperature, tube_mass_flowrate: MassRate, shell_inlet_temperature: ThermodynamicTemperature, shell_outlet_temeprature: ThermodynamicTemperature, shell_mass_flowrate: MassRate) -> Power { /* ... */ }
  ```
  obtain parasitic heat losses based on expt data

- ```rust
  pub fn new_custom_circular_single_pass_sthe_with_insulation(number_of_tubes: u32, number_of_inner_nodes: usize, fluid_pressure: Pressure, solid_pressure: Pressure, tube_side_od: Length, tube_side_id: Length, tube_side_hydraulic_diameter: Length, tube_side_flow_area_single_tube: Area, shell_side_od: Length, shell_side_id: Length, shell_side_hydraulic_diameter: Length, shell_side_flow_area: Area, sthe_length: Length, tube_side_form_loss: Ratio, shell_side_form_loss: Ratio, insulation_thickness: Length, tube_side_incline_angle: Angle, shell_side_incline_angle: Angle, shell_side_liquid: LiquidMaterial, tube_side_liquid: LiquidMaterial, inner_tube_material: SolidMaterial, outer_tube_material: SolidMaterial, insulation_material: SolidMaterial, ambient_temperature: ThermodynamicTemperature, heat_transfer_to_ambient: HeatTransfer, tube_side_initial_temperature: ThermodynamicTemperature, shell_side_initial_temperature: ThermodynamicTemperature, shell_loss_correlations: DimensionlessDarcyLossCorrelations, tube_loss_correlations: DimensionlessDarcyLossCorrelations, tube_side_nusselt_correlation: NusseltCorrelation, shell_side_nusselt_correlation_to_tubes: NusseltCorrelation, shell_side_nusselt_correlation_to_outer_shell: NusseltCorrelation) -> SimpleShellAndTubeHeatExchanger { /* ... */ }
  ```
  heat exchanger constructor

- ```rust
  pub fn new_du_et_al_sthe() -> SimpleShellAndTubeHeatExchanger { /* ... */ }
  ```
  heat exchanger constructor

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SimpleShellAndTubeHeatExchanger { /* ... */ }
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
    fn eq(self: &Self, other: &SimpleShellAndTubeHeatExchanger) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `one_d_solid_structure`

represents one dimensional solid structure
One-dimensional solid conduction structure.

This module provides [`SolidStructure`], a standalone 1D solid heat
structure (for example a hollow cylinder) discretised into an axial column
of solid control volumes. Unlike the fluid-component builders, it carries no
internal fluid array — it is a pure conduction body that can be laterally
coupled to an ambient temperature boundary and/or fed a heat source, and
optionally used as an additional wall / structural mass in a loop model.

Units: lengths and diameters in metres (m), cross-sectional area in square
metres (m^2), temperatures in kelvin (K), pressure in pascals (Pa), power in
watts (W) and thermal conductance in watts per kelvin (W/K). All public
signatures use `uom` dimensioned quantities.

Submodules:
- [`preprocessing`] — build the lateral conductances and boundary-condition
  links (ambient-temperature and mixed power/ambient couplings).
- [`calculation`] — advance the structure one timestep.
- [`postprocessing`] — read back the nodal temperature vector.

```rust
pub mod one_d_solid_structure { /* ... */ }
```

### Modules

## Module `preprocessing`

stuff such as conductances are calculated here

```rust
pub mod preprocessing { /* ... */ }
```

## Module `calculation`

stuff for calculation is done here, ie, advancing timestep

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

postprocessing stuff, ie, get the temperature vectors
of both arrays of control volumes

```rust
pub mod postprocessing { /* ... */ }
```

### Types

#### Struct `SolidStructure`

A one-dimensional solid conduction structure.

Represents a solid body (typically a hollow cylinder) as a single radial
layer of solid control volumes discretised axially. There is no internal
fluid; heat enters through a lateral coupling to an ambient boundary and/or
a user-supplied power source, and conducts along the axial nodes.

The standard assumption is that each axial end boundary has no conduction
heat transfer in the axial direction (zero-power boundary condition) unless
the user links something else to it.

```rust
pub struct SolidStructure {
    pub solid_array: super::heat_transfer_entities::HeatTransferEntity,
    pub strucutre_length: Length,
    pub cross_sectional_area: Area,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `solid_array` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the solid body itself<br>(a single radial layer of solid control volumes)<br><br>it is laterally coupled to an ambient temperature boundary<br>and/or a heat source; it has no coupled fluid array |
| `strucutre_length` | `Length` | axial length of the structure, in metres (m) |
| `cross_sectional_area` | `Area` | cross-sectional area of the solid, in square metres (m^2) |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn link_mixed_boundary_condition_laterally(self: &mut Self, solid_array_to_ambient_nodal_conductance: ThermalConductance, ambient_temp: ThermodynamicTemperature, total_power_input_into_column: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn link_ambient_temperature_boundary_condition_laterally(self: &mut Self, solid_array_to_ambient_nodal_conductance: ThermalConductance, ambient_temp: ThermodynamicTemperature) -> Result<(), TuasLibError> { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn get_ambient_surroundings_to_cylinder_thermal_conductance(self: &mut Self, h_air_to_pipe_surf: HeatTransfer, cylinder_diameter: Length, ambient_temp: ThermodynamicTemperature) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains ambient (usually air) to structure conductance

- ```rust
  pub fn get_ambient_surroundings_to_hollow_cylinder_thermal_conductance(self: &mut Self, h_air_to_pipe_surf: HeatTransfer, cylinder_id: Length, cylinder_od: Length, ambient_temp: ThermodynamicTemperature) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains ambient (usually air) to structure conductance

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self, thermal_conductance_to_ambient: ThermalConductance, ambient_temp: ThermodynamicTemperature, power_input_into_column: Power) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> Result<(), TuasLibError> { /* ... */ }
  ```
  advances the solid structure by one timestep, marching the solid

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances timestep by spawning a thread

- ```rust
  pub fn array_temperature(self: &mut Self) -> Result<Vec<ThermodynamicTemperature>, TuasLibError> { /* ... */ }
  ```
  returns the nodal temperature vector (in kelvin, K) of the solid

- ```rust
  pub fn new_hollow_cylinder(initial_temperature: ThermodynamicTemperature, solid_pressure: Pressure, cross_sectional_area: Area, shell_id: Length, shell_od: Length, cylinder_length: Length, pipe_shell_material: SolidMaterial, user_specified_inner_nodes: usize) -> SolidStructure { /* ... */ }
  ```
  constructs a solid structure as a hollow cylinder.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SolidStructure { /* ... */ }
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
    fn eq(self: &Self, other: &SolidStructure) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `ciet_struct_supports`

represents the old CIET struct support codes based on
https://escholarship.org/uc/item/0362h3zf

Ong, T. K. C. (2024). Digital Twins as
Testbeds for Iterative Simulated Neutronics Feedback
Controller Development (Doctoral dissertation, UC Berkeley).
CIET structural supports modelled as lumped thermal masses.

A structural support (e.g. a steel mounting strut or bracket in the CIET
facility) is represented as a 1D solid array of control volumes that stores
heat (thermal inertia) and conducts axially, while losing heat laterally to
the surrounding air by natural convection. These supports are unheated;
their role is to act as parasitic-heat-loss / thermal-mass nodes coupled to
the environment (and, where wired up, to adjacent fluid or solid
components).

Physical quantities used throughout this module:
- temperatures in kelvin (`ThermodynamicTemperature`),
- heat-transfer coefficients in W/(m^2 K) (`HeatTransfer`),
- thermal conductances in W/K (`ThermalConductance`),
- lengths in metres (`Length`), areas in m^2 (`Area`).

Submodules split the work into preprocessing (building conductances and
lateral couplings), calculation (advancing the control volumes one
timestep), and postprocessing (reading back temperature profiles).

```rust
pub mod ciet_struct_supports { /* ... */ }
```

### Modules

## Module `preprocessing`

contains method implementations for obtaining conductances
between the different arrays, and also laterally coupling
the arrays to one another using a radial thermal resistance

```rust
pub mod preprocessing { /* ... */ }
```

## Module `calculation`

contains methods to help advance timesteps (ie update the
state of the control volumes after each timestep)

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

for postprocessing, one can obtain temperature profiles
of the component using the postprocessing modules

```rust
pub mod postprocessing { /* ... */ }
```

### Types

#### Struct `StructuralSupport`

A CIET structural support (e.g. a steel strut or bracket) treated as a
lumped thermal mass.

The support is modelled as a 1D solid array of control volumes that stores
heat and conducts axially, while losing heat laterally to the surrounding
air. It is unheated: it exists to account for thermal inertia and parasitic
heat loss to the environment rather than to add power.

Note: need to check for memory leaks

```rust
pub struct StructuralSupport {
    pub support_array: super::heat_transfer_entities::HeatTransferEntity,
    pub ambient_temperature: ThermodynamicTemperature,
    pub heat_transfer_to_air: HeatTransfer,
    pub total_lateral_surface_area: Area,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `support_array` | `super::heat_transfer_entities::HeatTransferEntity` | 1D array of control volumes that simulates the<br>conduction heat transfer and thermal inertia within<br>the structural support |
| `ambient_temperature` | `ThermodynamicTemperature` | representative ambient temperature around the structural<br>support, meant for calculating parasitic heat loss |
| `heat_transfer_to_air` | `HeatTransfer` | representative heat transfer coefficient to surroundings<br>around the structural<br>support, meant for calculating parasitic heat loss |
| `total_lateral_surface_area` | `Area` | representative surface area in contact with surroundings<br>around the structural<br>support, meant for calculating parasitic heat loss |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn lateral_and_miscellaneous_connections(self: &mut Self) { /* ... */ }
  ```
  connects the support's solid array laterally to the ambient air.

- ```rust
  pub fn get_air_to_steel_array_conductance(self: &mut Self, h_air_to_insulation_surf: HeatTransfer) -> ThermalConductance { /* ... */ }
  ```
  obtains air to steel shell conductance

- ```rust
  pub fn get_axial_node_to_bc_conductance(self: &mut Self) -> ThermalConductance { /* ... */ }
  ```
  obtains node to bc conductance

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves a clone of the entire structural-support

- ```rust
  pub fn _advance_timestep(self: &mut Self, timestep: Time) { /* ... */ }
  ```
  advances the timestep for the support's HeatTransferEntity

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances timestep by spawning a thread

- ```rust
  pub fn get_temperature_array(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  obtains the temperature profile (or array) of the structural

- ```rust
  pub fn new_steel_support_cylinder(component_length: Length, diameter: Length, initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructs a structural support made typically of steel

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> StructuralSupport { /* ... */ }
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
    fn eq(self: &Self, other: &StructuralSupport) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `ciet_heater_top_and_bottom_head_bare`

represents the CIET heater top and bottom head codes based on
https://escholarship.org/uc/item/0362h3zf

Ong, T. K. C. (2024). Digital Twins as
Testbeds for Iterative Simulated Neutronics Feedback
Controller Development (Doctoral dissertation, UC Berkeley).
CIET heater top and bottom head (bare, no insulation) components.

This module models the two unheated end sections of the CIET heater
(version 2) — the top head and the bottom head — as the
[`HeaterTopBottomHead`] control-volume component. Each head is a lumped
set of three coupled thermal masses (Therminol VP-1 fluid, SS304L steel
shell, and twisted tape) exchanging heat with the ambient air.

Units throughout follow `uom` SI conventions: temperatures in kelvin (K),
heat-transfer coefficients in W/(m^2 K), surface areas in m^2, lengths in
metres (m), and mass flow in kg/s. Submodules split the component into
preprocessing (construction / connections), calculation (timestep
advance), and postprocessing (temperature readout).

```rust
pub mod ciet_heater_top_and_bottom_head_bare { /* ... */ }
```

### Modules

## Module `preprocessing`

contains method implementations for obtaining conductances
between the different arrays, and also laterally coupling
the arrays to one another using a radial thermal resistance

```rust
pub mod preprocessing { /* ... */ }
```

## Module `fluid_entity`

contains method implementations for FluidComponentTrait
This means all the stuff about getting mass flowrate from pressure
and vice versa

```rust
pub mod fluid_entity { /* ... */ }
```

## Module `calculation`

contains methods to help advance timesteps (ie update the
state of the control volumes after each timestep)

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

for postprocessing, one can obtain temperature profiles
of the component using the postprocessing modules

```rust
pub mod postprocessing { /* ... */ }
```

### Types

#### Struct `HeaterTopBottomHead`

Models one of the CIET heater (version 2, bare/no insulation)
unheated end sections — either the top head or the bottom head.

The heater insulation was burnt off around 2018, after which many
frequency-response tests were run with the insulation removed; this
component therefore assumes no insulation but retains the twisted-tape
interior of heater version 2.

Unlike the heated-section component, this struct represents the top OR
bottom head (the unheated end regions of the heater), constructed via
[`HeaterTopBottomHead::new_top_head`] or
[`HeaterTopBottomHead::new_bottom_head`]. Each head is modelled as three
coupled thermal masses — the therminol (Therminol VP-1) fluid, the steel
(SS304L) shell, and the twisted tape — together with the ambient air state
used for heat loss.

```rust
pub struct HeaterTopBottomHead {
    pub twisted_tape_interior: super::heat_transfer_entities::HeatTransferEntity,
    pub steel_shell: super::heat_transfer_entities::HeatTransferEntity,
    pub therminol_array: super::heat_transfer_entities::HeatTransferEntity,
    pub ambient_temperature: ThermodynamicTemperature,
    pub heat_transfer_to_air: HeatTransfer,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `twisted_tape_interior` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>for the twisted tape in the heater top and bottom heads |
| `steel_shell` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>for the steel piping in the heater top and bottom heads |
| `therminol_array` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>for the therminol fluid in the heater top and bottom heads |
| `ambient_temperature` | `ThermodynamicTemperature` | ambient temperature of the surrounding air used to calculate heat loss<br>(a thermodynamic temperature, SI unit kelvin) |
| `heat_transfer_to_air` | `HeatTransfer` | heat transfer coefficient used to calculate heat loss to air<br>(SI unit watt per square metre per kelvin, W/(m^2 K)) |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn lateral_and_miscellaneous_connections(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn get_air_steel_shell_conductance(self: &mut Self, h_air_to_steel_surf: HeatTransfer) -> ThermalConductance { /* ... */ }
  ```
  obtains air to steel shell conductance

- ```rust
  pub fn get_therminol_node_steel_shell_conductance(self: &mut Self) -> ThermalConductance { /* ... */ }
  ```
  obtains therminol to steel shell conductance

- ```rust
  pub fn heater_v2_hydraulic_diameter_reynolds(self: &Self, mass_flowrate: MassRate, temperature: ThermodynamicTemperature) -> Ratio { /* ... */ }
  ```
  gets reynolds number based on top and bottom head

- ```rust
  pub fn get_therminol_node_twisted_tape_conductance(self: &Self) -> ThermalConductance { /* ... */ }
  ```
  obtains therminol to twisted tape conductance

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self, mass_flowrate: MassRate) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the entire heater object into the

- ```rust
  pub fn _advance_timestep(self: &mut Self, timestep: Time) { /* ... */ }
  ```
  advances the timestep (the `timestep` argument is the time increment,

- ```rust
  pub fn _advance_timestep_parallel_buggy(self: &mut Self, timestep: Time) { /* ... */ }
  ```
  advances the timestep (the `timestep` argument is the time increment,

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances timestep by spawning a thread

- ```rust
  pub fn steel_shell_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  provides an array of temperatures representing

- ```rust
  pub fn therminol_array_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  provides an array of temperatures representing

- ```rust
  pub fn twisted_tape_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  provides an array of temperatures representing

- ```rust
  pub fn new_top_head(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  synthesiszes a new heater top head

- ```rust
  pub fn _new_user_callibrated_top_head(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, h_to_air: HeatTransfer) -> Self { /* ... */ }
  ```
  synthesiszes a new user callibrated heater top head

- ```rust
  pub fn new_bottom_head(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  synthesiszes a new heater bottom head

- ```rust
  pub fn _new_user_callibrated_bottom_head(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, h_to_air: HeatTransfer) -> Self { /* ... */ }
  ```
  synthesiszes a new heater bottom head allowing the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> HeaterTopBottomHead { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **FluidComponentTrait**
  - ```rust
    fn get_mass_flowrate(self: &mut Self) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_loss_immutable(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area(self: &mut Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area_immutable(self: &Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_at_ref_temperature(self: &mut Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_immutable_at_ref_temperature(self: &Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_at_ref_temperature(self: &mut Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_immutable_at_ref_temperature(self: &Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_component_length(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_component_length_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle(self: &mut Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle_immutable(self: &Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source_immutable(self: &Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_internal_pressure_source(self: &mut Self, internal_pressure: Pressure) { /* ... */ }
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
    fn eq(self: &Self, other: &HeaterTopBottomHead) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `insulated_porous_media_fluid_components`

insulated porous media pipes are basically insulated pipes with
some things lodged inside. It could be a packed bed, or an annular inner pipe
Some of the code
represents the CIET static mixer codes based on
https://escholarship.org/uc/item/0362h3zf

Ong, T. K. C. (2024). Digital Twins as
Testbeds for Iterative Simulated Neutronics Feedback
Controller Development (Doctoral dissertation, UC Berkeley).
# Insulated porous-media fluid components

Pre-built components modelling a fluid flowing through a porous solid
matrix (a packed bed or internal structure such as CIET's MX-10 static
mixer, or the annular pipe inside CIET Heater v1) enclosed by a solid pipe
shell and an outer insulation layer that loses heat to ambient air.

Radial layout, from the centre outwards:
porous-media interior -> shell fluid -> pipe shell -> insulation -> ambient.

This module defines the `InsulatedPorousMediaFluidComponent` struct and its
constructors (annular pipe, CIET Heater v1 body/top/bottom heads, the
DeWet insulated Heater v2, and static mixers MX-10/MX-20/MX-21 plus their
adjacent pipes). Behaviour is split across submodules:
- `preprocessing` — thermal-connection setup and nodal conductances (W/K),
  with MX-10-specific helpers in `preprocessing::mx10`
- `calculation` — timestep advance / conduction updates
- `fluid_entity` — `FluidComponentTrait` (pressure drop in Pa vs mass flow in kg/s)
- `postprocessing` — nodal temperature profiles (K)
- `calibration` — insulation-thickness tuning of parasitic heat loss
- `type_conversion` — conversion into a `FluidComponent`
- `tests` — steady-state and transient validation against Zweibaum's data

```rust
pub mod insulated_porous_media_fluid_components { /* ... */ }
```

### Modules

## Module `preprocessing`

contains method implementations for obtaining conductances
between the different arrays, and also laterally coupling
the arrays to one another using a radial thermal resistance
Preprocessing for `InsulatedPorousMediaFluidComponent`.

Sets up the lateral (radial) thermal connections between the control-volume
arrays — porous-media interior, shell fluid, pipe shell and insulation —
computes the nodal thermal conductances (W/K) that couple them, and wires
the zero-power axial boundary conditions at each array end. MX-10 /
static-mixer-specific preprocessing lives in the `mx10` submodule.

```rust
pub mod preprocessing { /* ... */ }
```

### Modules

## Module `mx10`

contains preprocessing calcs specifc to mx10 and static
mixers
MX-10 / static-mixer-specific preprocessing for
`InsulatedPorousMediaFluidComponent`.

Provides the MX-10 lateral (radial) connection routine and the nodal
thermal-conductance (W/K) helpers tuned to the CIET MX-10 static-mixer
geometry: air-to-insulation, therminol-to-steel-shell and
steel-shell-to-fiberglass conductances, the MX-10 hydraulic-diameter
Reynolds-number helper, and a threaded lateral-connection variant.

```rust
pub mod mx10 { /* ... */ }
```

## Module `fluid_entity`

contains method implementations for FluidComponentTrait
This means all the stuff about getting mass flowrate from pressure
and vice versa
`FluidComponentTrait` implementation for
`InsulatedPorousMediaFluidComponent`.

Delegates the hydraulic behaviour — pressure drop (Pa) vs mass flow rate
(kg/s), hydraulic diameter (m), cross-sectional area (m^2) and fluid
properties — to the component's inner `pipe_fluid_array`, so the component
can be dropped into fluid-component collections and pressure-drop solvers.

```rust
pub mod fluid_entity { /* ... */ }
```

## Module `calculation`

contains methods to help advance timesteps (ie update the
state of the control volumes after each timestep)
Timestep advance for `InsulatedPorousMediaFluidComponent`.

Advances the state of each control-volume array (shell fluid, pipe shell,
insulation and porous-media interior) forward by one timestep — carrying
out the conduction/energy update — either serially or by spawning threads.

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

for postprocessing, one can obtain temperature profiles
of the component using the postprocessing modules
Postprocessing for `InsulatedPorousMediaFluidComponent`.

Extracts the nodal temperature profiles (K) of the pipe shell, shell fluid,
insulation and porous-media interior arrays, plus the total node count.

```rust
pub mod postprocessing { /* ... */ }
```

## Module `type_conversion`

conversion into other types
Type conversions for `InsulatedPorousMediaFluidComponent`.

Converts the component into a `FluidComponent` (via its inner fluid array)
so it can be stored in fluid-component collections and hydraulic networks.

```rust
pub mod type_conversion { /* ... */ }
```

## Module `calibration`

calibration
Insulation calibration for `InsulatedPorousMediaFluidComponent`.

Adjusts the insulation-layer thermal-conductance lengthscales (A/L, in m)
to tune parasitic heat loss to ambient by changing the effective insulation
thickness, without altering the component's thermal inertia.

```rust
pub mod calibration { /* ... */ }
```

### Types

#### Struct `InsulatedPorousMediaFluidComponent`

Fluid Components with Internals

This could be an insulated pipe with some twisted tape inside
For example, a static mixer

StaticMixer MX-10 is a classic example of what this class is meant for

However, it could also be used for CIET Heater v1.0 where it was insulated
and had an annular pipe inside it


```rust
pub struct InsulatedPorousMediaFluidComponent {
    pub insulation_array: super::heat_transfer_entities::HeatTransferEntity,
    pub interior_solid_array_for_porous_media: super::heat_transfer_entities::HeatTransferEntity,
    pub pipe_shell: super::heat_transfer_entities::HeatTransferEntity,
    pub pipe_fluid_array: super::heat_transfer_entities::HeatTransferEntity,
    pub ambient_temperature: ThermodynamicTemperature,
    pub heat_transfer_to_ambient: HeatTransfer,
    pub darcy_loss_correlation: crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations,
    pub thermal_conductance_lengthscale_pipe_shell_to_insulation_pipe_interface: Length,
    pub thermal_conductance_lengthscale_pipe_shell_to_fluid: Length,
    pub thermal_conductance_lengthscale_fluid_to_porous_media_internal: Length,
    pub thermal_conductance_lengthscale_insulation_to_insulation_pipe_interface: Length,
    pub thermal_conductance_lengthscale_insulation_to_ambient: Length,
    pub nusselt_correlation_fluid_to_pipe_shell: crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation,
    pub nusselt_correlation_lengthscale_fluid_to_pipe_shell: Length,
    pub convection_heat_transfer_area_insulation_to_ambient: Area,
    pub nusselt_correlation_fluid_to_porous_media_interior: crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation,
    pub nusselt_correlation_lengthscale_fluid_to_porous_media_interior: Length,
    pub convection_heat_transfer_area_fluid_to_pipe_shell: Area,
    pub convection_heat_transfer_area_fluid_to_interior: Area,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `insulation_array` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>for the insulation around the Insulated Porous media component<br>such as MX-10 |
| `interior_solid_array_for_porous_media` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>of heat generating or<br>non-heat generating components within the pipe<br>or fluid component<br><br>for example,<br>the twisted tape in the heated section of CIET's Heater |
| `pipe_shell` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>for the steel piping in MX-10 |
| `pipe_fluid_array` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>for the therminol fluid in MX-10 |
| `ambient_temperature` | `ThermodynamicTemperature` | ambient temperature of air used to calculate heat loss |
| `heat_transfer_to_ambient` | `HeatTransfer` | heat transfer coefficient used to calculate heat loss<br>to air |
| `darcy_loss_correlation` | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations` | loss correlations<br>for pipe losses |
| `thermal_conductance_lengthscale_pipe_shell_to_insulation_pipe_interface` | `Length` | thermal conductance lengthscale to ambient<br><br>for calculating thermal resistance, we need a length<br>scale<br><br>thermal conductance = (kA)/L<br><br>assuming 1D cartesian coordinates, you need to specify<br>a lengthscale for an appropraite thermal resistance.<br><br>This is not L, but rather A/L<br><br>to get thermal conductance just A/L * k<br>basically... |
| `thermal_conductance_lengthscale_pipe_shell_to_fluid` | `Length` | thermal conductance lengthscale from pipe to fluid<br><br>for calculating thermal resistance, we need a length<br>scale<br><br>thermal conductance = (kA)/L<br><br>assuming 1D cartesian coordinates, you need to specify<br>a lengthscale for an appropraite thermal resistance.<br><br>This is not L, but rather A/L<br><br>to get thermal conductance just A/L * k<br>basically... |
| `thermal_conductance_lengthscale_fluid_to_porous_media_internal` | `Length` | thermal conductance lengthscale from fluid to<br>porous media internal<br><br>for calculating thermal resistance, we need a length<br>scale<br><br>thermal conductance = (kA)/L<br><br>assuming 1D cartesian coordinates, you need to specify<br>a lengthscale for an appropraite thermal resistance.<br><br>This is not L, but rather A/L<br><br>to get thermal conductance just A/L * k<br>basically... |
| `thermal_conductance_lengthscale_insulation_to_insulation_pipe_interface` | `Length` | thermal conductance lengthscale from pipe shell to insulation<br><br><br>for calculating thermal resistance, we need a length<br>scale<br><br>thermal conductance = (kA)/L<br><br>assuming 1D cartesian coordinates, you need to specify<br>a lengthscale for an appropraite thermal resistance.<br><br>This is not L, but rather A/L<br><br>to get thermal conductance just A/L * k<br>basically... |
| `thermal_conductance_lengthscale_insulation_to_ambient` | `Length` | thermal conductance lengthscale from pipe shell to insulation<br><br><br>for calculating thermal resistance, we need a length<br>scale<br><br>thermal conductance = (kA)/L<br><br>assuming 1D cartesian coordinates, you need to specify<br>a lengthscale for an appropraite thermal resistance.<br><br>This is not L, but rather A/L<br><br>to get thermal conductance just A/L * k<br>basically... |
| `nusselt_correlation_fluid_to_pipe_shell` | `crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation` | nusselt correlation from fluid to pipe shell |
| `nusselt_correlation_lengthscale_fluid_to_pipe_shell` | `Length` | lengthscale for nusselt correlation to ambient<br>for pipes, the hydraulic diameter usually suffices |
| `convection_heat_transfer_area_insulation_to_ambient` | `Area` | convection heat transfer area to ambient<br>used to calculate conductance to ambient hA<br>conductance = h A |
| `nusselt_correlation_fluid_to_porous_media_interior` | `crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation` | nusselt correlation to porous media interior |
| `nusselt_correlation_lengthscale_fluid_to_porous_media_interior` | `Length` | lengthscale for nusselt correlation to porous_media_interior<br>for pipes, the hydraulic diameter usually suffices |
| `convection_heat_transfer_area_fluid_to_pipe_shell` | `Area` | convection heat transfer area to pipe<br>used to calculate conductance to pipe hA<br>conductance = h A |
| `convection_heat_transfer_area_fluid_to_interior` | `Area` | convection heat transfer area to interior<br>used to calculate conductance to interior hA<br>conductance = h A |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn lateral_and_miscellaneous_connections_mx10(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn get_air_insulation_shell_conductance_mx10(self: &mut Self, h_air_to_insulation_surf: HeatTransfer) -> ThermalConductance { /* ... */ }
  ```
  obtains the nodal thermal conductance (W/K) from ambient air to the

- ```rust
  pub fn get_therminol_node_steel_shell_conductance_mx10(self: &mut Self) -> ThermalConductance { /* ... */ }
  ```
  obtains therminol to steel shell conductance

- ```rust
  pub fn mx10_hydraulic_diameter_reynolds(self: &mut Self, mass_flowrate: MassRate, temperature: ThermodynamicTemperature) -> Ratio { /* ... */ }
  ```
  gets reynolds number based on MX-10 hydraulic diameter

- ```rust
  pub fn get_steel_to_fiberglass_conductance_mx10_nodal(self: &Self) -> ThermalConductance { /* ... */ }
  ```
  obtains the nodal thermal conductance (W/K) from the steel pipe shell to

- ```rust
  pub fn lateral_connection_thread_spawn_mx10(self: &Self, mass_flowrate: MassRate) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn lateral_and_miscellaneous_connections(self: &mut Self, prandtl_wall_correction_setting: bool, mass_flowrate: MassRate, shell_side_steady_state_power: Power, porous_media_side_steady_state_power: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  InsulatedPorousMediaFluidComponent config:

- ```rust
  pub fn get_ambient_to_insulation_nodal_conductance(self: &mut Self, heat_transfer_to_ambient: HeatTransfer) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains the nodal thermal conductance (W/K) from ambient air to the

- ```rust
  pub fn get_pipe_shell_to_insulation_nodal_conductance(self: &mut Self) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains the nodal thermal conductance (W/K) from the pipe shell to the

- ```rust
  pub fn get_pipe_shell_to_fluid_nodal_conductance(self: &Self, prandtl_wall_correction_setting: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  gets pipe shell to fluid nodalised conductance

- ```rust
  pub fn get_interior_to_fluid_nodal_conductance(self: &Self, prandtl_wall_correction_setting: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  gets nodalised conductance from porous media or twisted

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self, prandtl_wall_correction_setting: bool, mass_flowrate: MassRate, shell_side_steady_state_power: Power, porous_media_side_steady_state_power: Power) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) { /* ... */ }
  ```
  advances timestep for each HeatTransferEntity within the

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances timestep by spawning a thread

- ```rust
  pub fn _advance_timestep_parallel_buggy(self: &mut Self, timestep: Time) { /* ... */ }
  ```
  advances timestep for each HeatTransferEntity within the

- ```rust
  pub fn pipe_shell_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  gets the steel piping temperature of MX-10 in an array

- ```rust
  pub fn pipe_fluid_array_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  gets the fluid temperature of MX-10 in an array

- ```rust
  pub fn insulation_array_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  gets the insulation temperature in an array

- ```rust
  pub fn interior_solid_array_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  provides an array of temperatures representing

- ```rust
  pub fn number_of_nodes(self: &Self) -> usize { /* ... */ }
  ```
  returns the number of nodes in this InsulatedPorousMediaFluidComponent

- ```rust
  pub fn calibrate_insulation_thickness(self: &mut Self, pipe_length: Length, insulation_id: Length, insulation_thickness: Length) { /* ... */ }
  ```
  calibrates the insulation thickness of this pipe or component,

- ```rust
  pub fn new_annular_pipe(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, fluid_pressure: Pressure, solid_pressure: Pressure, pipe_shell_id: Length, pipe_length: Length, flow_area: Area, incline_angle: Angle, form_loss: Ratio, outer_pipe_thickness: Length, inner_pipe_id: Length, inner_pipe_od: Length, insulation_thickness: Length, insulation_material: SolidMaterial, pipe_shell_material: SolidMaterial, inner_annular_pipe_material: SolidMaterial, pipe_fluid_material: LiquidMaterial, htc_to_ambient: HeatTransfer, user_specificed_number_of_nodes: usize) -> Self { /* ... */ }
  ```
  constructs a new annular pipe with insulation

- ```rust
  pub fn new_ciet_heater_v1_with_annular_pipe(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  constructs the ciet heater v1 with inner annular pipe

- ```rust
  pub fn new_ciet_heater_v1_top_head(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  makes an insulated top head for the ciet v1 heater

- ```rust
  pub fn new_ciet_heater_v1_bottom_head(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  makes an insulated bottom head for the ciet v1 heater

- ```rust
  pub fn new_dewet_model_heater_v2_insulated(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  traditional callibrated heater constructor

- ```rust
  pub fn new_static_mixer_23_mx20(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructs the static mixer using the RELAP/SAM model

- ```rust
  pub fn new_static_mixer_25_mx21(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructs the static mixer using the RELAP/SAM model

- ```rust
  pub fn new_static_mixer_2_mx10(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructs the static mixer using the RELAP/SAM model

- ```rust
  pub fn new_static_mixer_pipe_2a_mx10(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructs the static mixer pipe using the RELAP/SAM model

- ```rust
  pub fn new_static_mixer_pipe_25a_mx21(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructs the static mixer pipe using the RELAP/SAM model

- ```rust
  pub fn new_static_mixer_pipe_23a_mx20(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  constructs the static mixer pipe using the RELAP/SAM model

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> InsulatedPorousMediaFluidComponent { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **FluidComponentTrait**
  - ```rust
    fn get_mass_flowrate(self: &mut Self) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_loss_immutable(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area(self: &mut Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area_immutable(self: &Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_at_ref_temperature(self: &mut Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_immutable_at_ref_temperature(self: &Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_at_ref_temperature(self: &mut Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_immutable_at_ref_temperature(self: &Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_component_length(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_component_length_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle(self: &mut Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle_immutable(self: &Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source_immutable(self: &Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_internal_pressure_source(self: &mut Self, internal_pressure: Pressure) { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &InsulatedPorousMediaFluidComponent) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `non_insulated_porous_media_fluid_components`

non insulated porous media pipes are basically non insulated pipes with
some things lodged inside. It could be a packed bed, or an annular inner pipe
Some of the code
this can be used to represent non insulated porous media pipes such as
such as the old CIET heater version 2 based on
https://escholarship.org/uc/item/0362h3zf

Ong, T. K. C. (2024). Digital Twins as
Testbeds for Iterative Simulated Neutronics Feedback
Controller Development (Doctoral dissertation, UC Berkeley).
Non-insulated porous-media fluid components: fluid flowing through a solid
matrix inside an uninsulated pipe that also exchanges heat with ambient.

The canonical instance is CIET's Heater version 2 heated section, whose
interior is a twisted-tape / perforated heating element treated as porous
media. Three coupled control-volume arrays are modelled: the fluid
(`pipe_fluid_array`), the outer pipe shell (`pipe_shell`), and the interior
solid matrix (`interior_solid_array_for_porous_media`). Radial coupling is
by nodal thermal conductances (W/K); the shell additionally loses heat to a
constant-temperature ambient. Note this component is the heated section
only — it excludes the heater top and bottom heads.

Module map:
- [`preprocessing`] — general lateral/axial connections and the nodal
  conductances (W/K) between fluid, shell, interior and ambient; its
  [`preprocessing::ciet_heater_v2`] submodule holds the CIET-heater-v2
  specific conductance builders.
- [`fluid_entity`] — the `FluidComponentTrait` impl (mass flow in kg/s,
  pressure drop in Pa, geometry).
- [`calculation`] — advances the three arrays one timestep.
- [`postprocessing`] — retrieves the fluid, shell and interior temperature
  vectors (in kelvin).
- [`type_conversion`] — conversion into a `FluidComponent` enum variant.
- [`tests`] — verification against De Wet's CIET heater-v2 data.

```rust
pub mod non_insulated_porous_media_fluid_components { /* ... */ }
```

### Modules

## Module `preprocessing`

contains method implementations for obtaining conductances
between the different arrays, and also laterally coupling
the arrays to one another using a radial thermal resistance
Preprocessing for the non-insulated porous-media fluid component:
establishes the thermal connections between its control-volume arrays and
computes the per-node thermal conductances (W/K).

This is where the fluid, pipe shell, interior porous-media solid and the
ambient boundary are laterally coupled for a given mass flow rate (kg/s)
and heater power (W), and where the ambient-to-shell, shell-to-fluid and
interior-to-fluid nodal conductances are derived from Nusselt-number
correlations plus solid-side conduction lengthscales. The
[`ciet_heater_v2`] submodule holds the CIET-heater-v2-specific variants of
these builders.

```rust
pub mod preprocessing { /* ... */ }
```

### Modules

## Module `ciet_heater_v2`

contains preprocessing functions specifically for
ciet heater v2

```rust
pub mod ciet_heater_v2 { /* ... */ }
```

## Module `fluid_entity`

contains method implementations for FluidComponentTrait
This means all the stuff about getting mass flowrate from pressure
and vice versa

```rust
pub mod fluid_entity { /* ... */ }
```

## Module `calculation`

contains methods to help advance timesteps (ie update the
state of the control volumes after each timestep)

```rust
pub mod calculation { /* ... */ }
```

## Module `postprocessing`

for postprocessing, one can obtain temperature profiles
of the component using the postprocessing modules
Postprocessing for the non-insulated porous-media fluid component:
extracts the nodal temperature vectors (in kelvin) of each of its three
control-volume arrays — the outer pipe shell, the fluid, and the interior
porous-media solid (e.g. the twisted tape in CIET's heated section).

```rust
pub mod postprocessing { /* ... */ }
```

## Module `type_conversion`

for converting into fluid components

```rust
pub mod type_conversion { /* ... */ }
```

## Module `tests`

tests for all ciet's heaters in

Ong, T. K. C. (2024). Digital Twins as Testbeds for
Iterative Simulated Neutronics Feedback Controller Development
(Doctoral dissertation, UC Berkeley).

The tuas_boussinesq_solver library was constructed with CIET
in mind

This is the compact integral effects test from the UC Berkeley
Thermal Hydraulics Lab
A Library which contains useful traits and methods for thermal
hydraulics calculations.


This crate has heavy reliance on units of measure (uom) released under
Apache 2.0 license. So you'll need to get used to unit safe calculations
with uom as well.


This library was initially developed for
use in my PhD thesis under supervision
of Professor Per F. Peterson. It a thermal hydraulics
library in Rust that is released under the GNU General Public License
v 3.0. This is partly due to the fact that some of the libraries
inherit from GeN-Foam and OpenFOAM, both licensed under GNU General
Public License v3.0.

As such, the entire library is released under GNU GPL v3.0. It is a strong
copyleft license which means you cannot use it in proprietary software.


License
   This is a thermal hydraulics library written
   in rust meant to help with the
   fluid mechanics and heat transfer aspects of the calculations
   for the Compact Integral Effects Tests (CIET) and hopefully
   Gen IV Reactors such as the Fluoride Salt cooled High Temperature
   Reactor (FHR)
     
   Copyright (C) 2022-2023  Theodore Kay Chen Ong, Singapore Nuclear
   Research and Safety Initiative, Per F. Peterson, University of
   California, Berkeley Thermal Hydraulics Laboratory

   tuas_boussinesq_solver is free software; you can
   redistribute it and/or modify it
   under the terms of the GNU General Public License as published by the
   Free Software Foundation; either version 2 of the License, or (at your
   option) any later version.

   tuas_boussinesq_solver is distributed in the hope
   that it will be useful, but WITHOUT
   ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
   FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
   for more details.

   This thermal hydraulics library
   contains some code copied from GeN-Foam, and OpenFOAM derivative.
   This offering is not approved or endorsed by the OpenFOAM Foundation nor
   OpenCFD Limited, producer and distributor of the OpenFOAM(R)software via
   www.openfoam.com, and owner of the OPENFOAM(R) and OpenCFD(R) trademarks.
   Nor is it endorsed by the authors and owners of GeN-Foam.

   You should have received a copy of the GNU General Public License
   along with this program.  If not, see <http://www.gnu.org/licenses/>.

© All rights reserved. Theodore Kay Chen Ong,
Singapore Nuclear Research and Safety Initiative,
Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Laboratory

Main author of the code: Theodore Kay Chen Ong, supervised by
Professor Per F. Peterson

Btw, I no affiliation with the Rust Foundation.
Verification and development tests for the non-insulated porous-media
fluid component, using CIET's Heater version 2 as the reference case.

Steady-state outlet temperatures are checked against De Wet's experimental
data (Ong 2024, PhD thesis), and regression tests confirm the generalised
porous-media component reproduces the original CIET-heater-v2 code path.

```rust
pub mod tests { /* ... */ }
```

### Types

#### Struct `NonInsulatedPorousMediaFluidComponent`

represents heater version 2 without insulation
This is because during 2018-ish, the heater insulation
got burnt off and a lot of frequency response tests were done
with insulation removed

Heater version 2 bare has no insulation
but it has a twisted tape interior


note that it only contains the heated section, not the top nor
bottom heads

note: the pressure drop correlations are not yet properly implemented
so it behaves like a pipe in terms of pressure drop
For now, I did not do anything special with it

```rust
pub struct NonInsulatedPorousMediaFluidComponent {
    pub interior_solid_array_for_porous_media: super::heat_transfer_entities::HeatTransferEntity,
    pub pipe_shell: super::heat_transfer_entities::HeatTransferEntity,
    pub pipe_fluid_array: super::heat_transfer_entities::HeatTransferEntity,
    pub ambient_temperature: ThermodynamicTemperature,
    pub heat_transfer_to_ambient: HeatTransfer,
    pub darcy_loss_correlation: crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations,
    pub solid_side_thermal_conductance_lengthscale_pipe_to_ambient: Length,
    pub solid_side_thermal_conductance_lengthscale_pipe_to_fluid: Length,
    pub solid_side_thermal_conductance_lengthscale_fluid_to_porous_media_internal: Length,
    pub nusselt_correlation_to_pipe_shell: crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation,
    pub nusselt_correlation_lengthscale_to_ambient: Length,
    pub convection_heat_transfer_area_to_ambient: Area,
    pub nusselt_correlation_to_porous_media_interior: crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation,
    pub nusselt_correlation_lengthscale_to_porous_media_interior: Length,
    pub convection_heat_transfer_area_to_pipe: Area,
    pub convection_heat_transfer_area_to_interior: Area,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `interior_solid_array_for_porous_media` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>of heat generating or<br>non-heat generating components within the pipe<br>or fluid component<br><br>for example,<br>the twisted tape in the heated section of CIET's Heater |
| `pipe_shell` | `super::heat_transfer_entities::HeatTransferEntity` | heat transfer entity representing control volumes<br>for the steel piping in the heated section of CIET's Heater |
| `pipe_fluid_array` | `super::heat_transfer_entities::HeatTransferEntity` | this HeatTransferEntity represents the pipe fluid<br>which is coupled to the pipe shell via a Nusselt Number based<br>thermal resistance (usually Gnielinski correlation)<br>But it is up to you to specify<br><br>heat transfer entity representing control volumes<br>for the therminol fluid in the heated section of CIET's Heater |
| `ambient_temperature` | `ThermodynamicTemperature` | <br>pipe heat transfer coefficient to ambient<br>eg.<br>ambient temperature of air used to calculate heat loss |
| `heat_transfer_to_ambient` | `HeatTransfer` | heat transfer coefficient used to calculate heat loss<br>to ambient, such as air air |
| `darcy_loss_correlation` | `crate::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations` | loss correlations |
| `solid_side_thermal_conductance_lengthscale_pipe_to_ambient` | `Length` | thermal conductance lengthscale to ambient<br><br>for calculating thermal resistance, we need a length<br>scale<br><br>thermal conductance = (kA)/L<br><br>assuming 1D cartesian coordinates, you need to specify<br>a lengthscale for an appropraite thermal resistance.<br><br>This is not L, but rather A/L<br><br>to get thermal conductance just A/L * k<br>basically... |
| `solid_side_thermal_conductance_lengthscale_pipe_to_fluid` | `Length` | thermal conductance lengthscale from pipe to fluid<br><br>for calculating thermal resistance, we need a length<br>scale<br><br>thermal conductance = (kA)/L<br><br>assuming 1D cartesian coordinates, you need to specify<br>a lengthscale for an appropraite thermal resistance.<br><br>This is not L, but rather A/L<br><br>to get thermal conductance just A/L * k<br>basically... |
| `solid_side_thermal_conductance_lengthscale_fluid_to_porous_media_internal` | `Length` | thermal conductance lengthscale from fluid to<br>porous media internal<br><br>for calculating thermal resistance, we need a length<br>scale<br><br>thermal conductance = (kA)/L<br><br>assuming 1D cartesian coordinates, you need to specify<br>a lengthscale for an appropraite thermal resistance.<br><br>This is not L, but rather A/L<br><br>to get thermal conductance just A/L * k<br>basically... |
| `nusselt_correlation_to_pipe_shell` | `crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation` | nusselt correlation to pipe shell (to ambient) |
| `nusselt_correlation_lengthscale_to_ambient` | `Length` | lengthscale for nusselt correlation to ambient<br>for pipes, the hydraulic diameter usually suffices |
| `convection_heat_transfer_area_to_ambient` | `Area` | convection heat transfer area to ambient<br>used to calculate conductance to ambient hA<br>conductance = h A |
| `nusselt_correlation_to_porous_media_interior` | `crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation` | nusselt correlation to porous media interior |
| `nusselt_correlation_lengthscale_to_porous_media_interior` | `Length` | lengthscale for nusselt correlation to porous_media_interior<br>for pipes, the hydraulic diameter usually suffices |
| `convection_heat_transfer_area_to_pipe` | `Area` | convection heat transfer area to pipe<br>used to calculate conductance to pipe hA<br>conductance = h A |
| `convection_heat_transfer_area_to_interior` | `Area` | convection heat transfer area to interior<br>used to calculate conductance to interior hA<br>conductance = h A |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn ciet_heater_v2_lateral_and_miscellaneous_connections(self: &mut Self, mass_flowrate: MassRate, heater_steady_state_power: Power) { /* ... */ }
  ```
  used to connect the arrays laterally

- ```rust
  pub fn ciet_heater_v2_get_air_steel_nodal_shell_conductance(self: &mut Self, h_air_to_steel_surf: HeatTransfer) -> ThermalConductance { /* ... */ }
  ```
  obtains air to steel shell conductance

- ```rust
  pub fn ciet_heater_v2_get_therminol_node_steel_shell_conductance(self: &mut Self) -> ThermalConductance { /* ... */ }
  ```
  obtains therminol to steel shell conductance

- ```rust
  pub fn heater_v2_hydraulic_diameter_reynolds(mass_flowrate: MassRate, temperature: ThermodynamicTemperature) -> Ratio { /* ... */ }
  ```
  obtains Reynolds number for the heater given a temperature and

- ```rust
  pub fn ciet_heater_v2_get_therminol_node_twisted_tape_conductance(self: &Self) -> ThermalConductance { /* ... */ }
  ```
  obtains therminol to twisted tape conductance

- ```rust
  pub fn ciet_heater_v2_lateral_connection_thread_spawn(self: &Self, mass_flowrate: MassRate, heater_steady_state_power: Power) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn lateral_and_miscellaneous_connections(self: &mut Self, prandtl_wall_correction_setting: bool, mass_flowrate: MassRate, shell_side_steady_state_power: Power, porous_media_side_steady_state_power: Power) -> Result<(), TuasLibError> { /* ... */ }
  ```
  NonInsulatedPorousMediaFluidComponent radial config (no insulation

- ```rust
  pub fn get_ambient_to_pipe_shell_nodal_conductance(self: &mut Self, heat_transfer_to_ambient: HeatTransfer) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  obtains the conductance from ambient to the pipe shell

- ```rust
  pub fn get_interior_to_fluid_nodal_conductance(self: &Self, prandtl_wall_correction_setting: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  gets nodalised conductance from porous media or twisted

- ```rust
  pub fn get_pipe_shell_to_fluid_nodal_conductance(self: &Self, prandtl_wall_correction_setting: bool) -> Result<ThermalConductance, TuasLibError> { /* ... */ }
  ```
  gets the pipe shell to fluid nodalised conductance

- ```rust
  pub fn lateral_connection_thread_spawn(self: &Self, prandtl_wall_correction_setting: bool, mass_flowrate: MassRate, shell_side_steady_state_power: Power, porous_media_side_steady_state_power: Power) -> JoinHandle<Self> { /* ... */ }
  ```
  spawns a thread and moves the clone of the entire heater object into the

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) { /* ... */ }
  ```
  advances timestep for each HeatTransferEntity within the

- ```rust
  pub fn advance_timestep_thread_spawn(self: &Self, timestep: Time) -> JoinHandle<Self> { /* ... */ }
  ```
  advances timestep by spawning a thread

- ```rust
  pub fn _advance_timestep_parallel_buggy(self: &mut Self, timestep: Time) { /* ... */ }
  ```
  advances timestep for each HeatTransferEntity within the

- ```rust
  pub fn pipe_shell_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  provides the nodal temperature vector (in kelvin) of the steel pipe

- ```rust
  pub fn pipe_fluid_array_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  provides the nodal temperature vector (in kelvin) of the fluid

- ```rust
  pub fn interior_solid_array_temperature(self: &mut Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  provides the nodal temperature vector (in kelvin) of the interior

- ```rust
  pub fn new_dewet_model_heater_v2(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  traditional callibrated heater constructor

- ```rust
  pub fn _new_six_watts_per_m2_kelvin_model_heater_v2_model(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, user_specified_inner_nodes: usize) -> Self { /* ... */ }
  ```
  traditional uncallibrated heater constructor

- ```rust
  pub fn ciet_heater_v2_generic_model(initial_temperature: ThermodynamicTemperature, ambient_temperature: ThermodynamicTemperature, user_specified_inner_nodes: usize, h_to_air: HeatTransfer) -> Self { /* ... */ }
  ```
  user uncallibrated heater constructor

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NonInsulatedPorousMediaFluidComponent { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **FluidComponentTrait**
  - ```rust
    fn get_mass_flowrate(self: &mut Self) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn set_mass_flowrate(self: &mut Self, mass_flowrate: MassRate) { /* ... */ }
    ```

  - ```rust
    fn get_mass_flowrate_from_pressure_loss_immutable(self: &Self, pressure_loss: Pressure) -> MassRate { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_pressure_loss(self: &mut Self, pressure_loss: Pressure) { /* ... */ }
    ```

  - ```rust
    fn get_pressure_loss_immutable(self: &Self, mass_flowrate: MassRate) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area(self: &mut Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_cross_sectional_area_immutable(self: &Self) -> Area { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_hydraulic_diameter_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_at_ref_temperature(self: &mut Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_viscosity_immutable_at_ref_temperature(self: &Self) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_at_ref_temperature(self: &mut Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_fluid_density_immutable_at_ref_temperature(self: &Self) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn get_component_length(self: &mut Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_component_length_immutable(self: &Self) -> Length { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle(self: &mut Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_incline_angle_immutable(self: &Self) -> Angle { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source(self: &mut Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn get_internal_pressure_source_immutable(self: &Self) -> Pressure { /* ... */ }
    ```

  - ```rust
    fn set_internal_pressure_source(self: &mut Self, internal_pressure: Pressure) { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> FluidComponent { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NonInsulatedPorousMediaFluidComponent) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `ciet_isothermal_test_components`

ciet components for pipes and valves for use in the isothermal test

Zweibaum, N. (2015). Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University
of California, Berkeley.

In my master's thesis, heat structure information was not included. However,
I shall include them in this round
Pre-built CIET components and branches for isothermal (constant-temperature)
hydraulic tests.

This module holds `new_*` builder functions that construct every pipe,
static mixer, flowmeter, pump, CTAH and DHX component of the Compact
Integral Effects Test (CIET) facility as either an
[`InsulatedFluidComponent`](super::insulated_pipes_and_fluid_components::InsulatedFluidComponent)
or a
[`NonInsulatedFluidComponent`](super::non_insulated_fluid_components::NonInsulatedFluidComponent).
Each builder takes an initial fluid temperature (a `uom`
`ThermodynamicTemperature`, i.e. kelvin/degC) and fills in the geometry
(hydraulic diameter and lengths in metres, flow area in square metres,
incline angle in degrees, form loss and surface roughness) from the
RELAP5-3D and SAM nodalisations of CIET.

Geometry parameters follow the CIET nodalisations reported by Zou, Hu &
Charpentier (SAM code validation, ANL/NSE-19/11, 2019) and Zweibaum's
thesis (UC Berkeley, 2015).

The submodules assemble these components into branches and run the
isothermal flow-rate vs pressure-drop verification tests:
[`ciet_branch_builders_isothermal`] wires the components into the heater,
CTAH and DHX branches, while [`isothermal_ctah_heater_branch_test`] and
[`isothermal_ctah_heater_dhx_branch_test`] verify the resulting mass
flow rates (kg/s) against the applied pressure change (Pa).

```rust
pub mod ciet_isothermal_test_components { /* ... */ }
```

### Modules

## Module `ciet_branch_builders_isothermal`

builds ciet branches for the isothermal test

```rust
pub mod ciet_branch_builders_isothermal { /* ... */ }
```

### Functions

#### Function `dhx_branch_builder_isothermal_test`

builds a dhx branch to simulate isothermal testing of ciet

```rust
pub fn dhx_branch_builder_isothermal_test(initial_temperature: ThermodynamicTemperature) -> crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_collection::FluidComponentCollection { /* ... */ }
```

#### Function `heater_branch_builder_isothermal_test`

builds a heater branch to simulate isothermal testing of ciet

```rust
pub fn heater_branch_builder_isothermal_test(initial_temperature: ThermodynamicTemperature) -> crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_collection::FluidComponentCollection { /* ... */ }
```

#### Function `ctah_branch_builder_isothermal_test`

builds the ctah branch to simulate isothermal testing of ciet
allows user to supply a pump pressure or loop pressure drop

```rust
pub fn ctah_branch_builder_isothermal_test(pump_pressure: Pressure, initial_temperature: ThermodynamicTemperature) -> crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_collection::FluidComponentCollection { /* ... */ }
```

### Functions

#### Function `new_pipe_6a`

creates a new pipe6a for CIET using the RELAP5-3D and SAM parameters
Pipe6a in Compact Integral Effects Test (CIET)
CTAH branch

It is a static mixer pipe
otherwise known as the static mixer pipe 6a

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_6a(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_static_mixer_41_label_6`

creates a new static mixer 41 (label 6) for CIET using the RELAP5-3D and SAM parameters
Component 6 in Compact Integral Effects Test (CIET)
CTAH branch  (also known as static mixer 41)

static mixer 41
label component 6
in Compact Integral Effects Test (CIET)
CTAH branch
static mixer 41 (MX-41) on CIET diagram
in the pump and CTAH branch
just before CTAH (AKA IHX)
from top to bottom

label 6 on diagram

It is a static mixer pipe
otherwise known as the static mixer pipe 6a

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_static_mixer_41_label_6(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_inactive_ctah_vertical`

creates a new ctah vertical for CIET using the RELAP5-3D and SAM parameters
in Compact Integral Effects Test (CIET)

this is inactive, so it behaves more like a pipe rather than a
heat exchanger

Vertical part of Coiled Tube Air Heater (CTAH)
label component 7a
in Compact Integral Effects Test (CIET)
CTAH branch

It is NOT insulated by the way


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.


```rust
pub fn new_inactive_ctah_vertical(initial_temperature: ThermodynamicTemperature) -> super::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_inactive_ctah_horizontal`

creates a new inactive CTAH horizontal section for CIET using the RELAP5-3D and SAM parameters
in Compact Integral Effects Test (CIET)

this is inactive, so it behaves more like a pipe rather than a
heat exchanger

Horizontal part of Coiled Tube Air Heater (CTAH)
label component 7b
in Compact Integral Effects Test (CIET)
CTAH branch
coiled tube air heater
has fldk = 400 + 52,000/Re

label is 7b
empirical data in page 48 on pdf viewer in Dr
Zweibaum thesis shows reverse flow has same
pressure drop characteristics as forward flow

It is NOT insulated by the way

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.


```rust
pub fn new_inactive_ctah_horizontal(initial_temperature: ThermodynamicTemperature) -> super::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_8a`

creates a new component for CIET using the RELAP5-3D and SAM parameters

It is a static mixer pipe
Static mixer pipe 8a
adjacent to MX-40 in the CTAH branch

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_8a(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_static_mixer_40_label_8`

creates a new component for CIET using the RELAP5-3D and SAM parameters

static mixer 40 (MX-40) on CIET diagram
just after CTAH (AKA IHX)
from top to bottom
label 8 on diagram

forced convection flow direction is same as top to bottom
has a fldk of 21+4000/Re

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_static_mixer_40_label_8(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_9`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe number 9 in CIET's CTAH branch

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_9(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_10`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe number 10 in CIET's CTAH branch

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_10(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_11`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe number 11 in CIET's CTAH branch

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_11(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_12`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe number 12 in CIET's CTAH branch

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_12(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_ctah_pump`

creates a new component for CIET using the RELAP5-3D and SAM parameters

ctah pump is a custom therminol component with
ie no friction factor losses
but it provides a source pressure

it is located between pipe 12 and 13

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_ctah_pump(initial_temperature: ThermodynamicTemperature) -> super::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_13`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe number 13 in CIET's CTAH branch
just after the pump
pipe 13 on the diagram in Nico Zweibaum nodalisation
probably some combination of V-42,
F-40 and F-41 on CIET diagram

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_13(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_14`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe number 14 in CIET's CTAH branch
pipe 14 on the diagram in Nico Zweibaum nodalisation
probably some combination of V-42,
F-40 and F-41 on CIET diagram
it is inclined 90 degrees upwards in direction
of flow

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also 90 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_14(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_flowmeter_40_14a`

creates a new component for CIET using the RELAP5-3D and SAM parameters

FM-40 Coriolis Flowmeter in CIET's CTAH branch
labelled 14a in simulation schmeatic

ctah line flowmeter 40
label 14a on simulation diagram
fldk = 18.0+93000/(Re^1.35)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_flowmeter_40_14a(initial_temperature: ThermodynamicTemperature) -> super::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_15`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe number 15 in CIET's CTAH branch

pipe 15 on the diagram in Nico Zweibaum nodalisation
probably corresponds of F30 on CIET's P&ID

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also
-49.36983 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_15(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_16`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe number 16 in CIET's CTAH branch

pipe 16 on the diagram in Nico Zweibaum nodalisation
probably corresponds of F30 on CIET's P&ID

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also
-49.36983 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_16(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_branch_17`

creates a new component for CIET using the RELAP5-3D and SAM parameters

Branch (or pipe 17) in CIET's CTAH branch

Approximations were made for this branch though,
technically branch 17a is part of CTAH branch
while 17b is part of the DHX branch,
I combined both for convenience
pipe 17 on the diagram in Nico Zweibaum nodalisation
probably corresponds of F30 on CIET's P&ID

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is 0 degrees


This is treated as a single pipe though

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_branch_17(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_branch_17b`

creates a new component for CIET using the RELAP5-3D and SAM parameters,
this is branch 17b within the DHX branch in the SAM model

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_branch_17b(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_branch_17a`

creates a new component for CIET using the RELAP5-3D and SAM parameters,
this is branch 17a within the CTAH branch in the SAM model

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_branch_17a(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_branch_5a`

creates a new component for CIET using the RELAP5-3D and SAM parameters
Branch 5a in the SAM model, using RELAP models for pipe length
and hydrualic diameter

located in the DHX branch

this is reverse order compared to table A-1 in
the Zweibaum nodalised relap model
pipe 5a on the diagram in SAM nodalisation
and from a top to bottom direction from pipe 5
to pipe 5, the incline angle is also
0 degrees
i add 180 degrees so that it is
properly reversed in
inclination angle from top to bottom

This is treated as a single pipe though

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_branch_5a(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_branch_5b`

creates a new component for CIET using the RELAP5-3D and SAM parameters
Branch 5b in the SAM model, using RELAP models for pipe length
and hydrualic diameter

located in the CTAH branch.
But for the purposes of hydrodynamic modelling and heat transfer
modelling, it is the same as the branch 5a

this is reverse order compared to table A-1 in
the Zweibaum nodalised relap model
pipe 5a on the diagram in SAM nodalisation
and from a top to bottom direction from pipe 5
to pipe 5, the incline angle is also
0 degrees
i add 180 degrees so that it is
properly reversed in
inclination angle from top to bottom

This is treated as a single pipe though

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_branch_5b(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_branch_5`

creates a new component for CIET using the RELAP5-3D and SAM parameters

Branch 5 in the Heater Branch (top to bottom perspective)

Approximations were made for this branch though,
technically branch 5a is part of DHX branch
while 5b is part of the DHX branch,
I combined both for convenience

This is treated as a single pipe though

Now I'd probably made a mistake putting branch 5 in
the heater branch, it's probably better put inside the
CTAH branch, (as of Oct 2022)
I'll probably put this in the CTAH branch in future

But for forced isothermal circulation tests with only
the heater branch and CTAH branch, it doesn't really matter
since there are only two branches

So no matter which branch you put branch or pipe 5 in,
it is still the same set of pipes in series
calculations will still be the same numerically


pipe 5 on the diagram in Nico Zweibaum nodalisation
and from a top to bottom direction from pipe 5
to pipe 5, the incline angle is also
0 degrees
i add 180 degrees so that it is
properly reversed in
inclination angle from top to bottom

This is treated as a single pipe though

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_branch_5(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_4`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe 4 within the heater branch
pipe 4 on the diagram in Nico Zweibaum nodalisation
probably corresponds of V11 and F12

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also
49.743387 +180.0 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_4(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_3_relap_model`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe3 within the heater branch
pipe 3 on the diagram in Nico Zweibaum nodalisation
probably corresponds of V11 and F12

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also
90.0 +180.0 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_3_relap_model(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_3_sam_model`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe3 within the heater branch
pipe 3 on the diagram in Nico Zweibaum nodalisation
probably corresponds of V11 and F12

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also
90.0 +180.0 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),

This uses the SAM parameters rather than the RELAP parameters
which uses 17.15 K value for pipe 3 rather than 3.15

```rust
pub fn new_pipe_3_sam_model(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_static_mixer_10_label_2`

creates a new component for CIET using the RELAP5-3D and SAM parameters

MX-10 within the heater branch
labelled as component 2


static mixer 10 (MX-10) on CIET diagram
just before the heater in the heater branch
from top to bottom
label 2 on diagram (fig A-1 on Nico Zweibaum thesis)
pg 125 on pdf viewer, pg 110 on printed page number on bottom right

though in reality flow goes from bottom to
top in forced convection
so from a flow perspective it is before the
heater


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_static_mixer_10_label_2(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_2a`

creates a new component for CIET using the RELAP5-3D and SAM parameters

static mixer pipe2a in heater branch

adjacent to MX-10
pipe 2a on the diagram in Nico Zweibaum nodalisation
probably corresponds of V11 and F12

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also
90.0 +180.0 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_2a(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_heater_top_head_1a`

creates a new component for CIET using the RELAP5-3D and SAM parameters

heater top head 1a of heater branch in CIET

heater top head
diagram label 1a

inclined at 90 degrees bottom to top
or 90 degrees + 180 top to bottom orientation

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_heater_top_head_1a(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_heated_section_version_1_label_1_without_inner_annular_pipe`

creates a new component for CIET using the RELAP5-3D and SAM parameters

This is the first version of CIET's heater

It is found in CIET's heater branch;
It has hydrodynamic losses similar to a pipe


this is the first version of the ciet heater
without any insert within the heater
the heater behaves like a pipe

inclined at 90 degrees bottom to top
or 90 degrees + 180 top to bottom orientation


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_heated_section_version_1_label_1_without_inner_annular_pipe(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_heater_bottom_head_1b`

creates a new component for CIET using the RELAP5-3D and SAM parameters

heater bottom head 1b within CIET's heater branch

heater top head
diagram label 1b

inclined at 90 degrees bottom to top
or 90 degrees + 180 top to bottom orientation

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_heater_bottom_head_1b(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_18`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe 18 within CIET's heater branch
pipe 18 on the diagram in Nico Zweibaum nodalisation

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also
-40.00520 +180.0 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_18(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_26`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe 26 in DHX Branch from Top to Bottom orientation

pipe 26 on the diagram in Nico Zweibaum nodalisation

and from a top to bottom direction from pipe 5
to pipe 17, the incline angle is also
-40.00520 +180.0 degrees

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_26(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_25a`

creates a new component for CIET using the RELAP5-3D and SAM parameters

Static mixer pipe 25a adjacent to MX-21
in DHX branch

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_25a(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_static_mixer_21_label_25`

creates a new component for CIET using the RELAP5-3D and SAM parameters

static mixer 21 (MX-21) on CIET diagram
in the DHX branch in primary loop
just before the DRACS heat exchanger
from top to bottom
label 25

in reality flow goes from bottom to
top in natural convection
also in the DRACS
loop there are flow diodes to make
it such that flow going from bottom to top
encounters more resistance

forced convection flow direction is same as top to bottom
has a fldk of 21+4000/Re

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_static_mixer_21_label_25(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_inactive_dhx_shell_side_heat_exchanger`

creates a new component for CIET using the RELAP5-3D and SAM parameters

this is the heat exchanger
in the DHX branch, labelled 24

It is shell side heat exchanger which allows
for heat to be transferred to natural circulation loop
or DRACS Loop
inclined at 90 degrees bottom to top
or 90 degrees + 180 top to bottom orientation

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_inactive_dhx_shell_side_heat_exchanger(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_23a`

creates a new component for CIET using the RELAP5-3D and SAM parameters

static mixer pipe 23a in DHX branch in CIET

otherwise known as the static mixer pipe
to MX-20

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_23a(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_static_mixer_20_label_23`

creates a new component for CIET using the RELAP5-3D and SAM parameters

static mixer 20 (MX-20) on CIET diagram
in the DRACS branch in primary loop
just after the DRACS heat exchanger
from top to bottom
label 23

in reality flow goes from bottom to
top in natural convection
also in the DRACS
loop there are flow diodes to make
it such that flow going from bottom to top
encounters more resistance

original angle is is 90 degrees
but i orientate from top to bottom

forced convection flow direction is same as top to bottom
has a fldk of 21+4000/Re

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_static_mixer_20_label_23(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_22_sam_model`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe 22 within DHX branch in CIEt


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_22_sam_model(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_22_relap_model`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe 22 within DHX branch in CIEt


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_22_relap_model(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_flowmeter_20_label_21a`

creates a new component for CIET using the RELAP5-3D and SAM parameters

FM-20 DHX branch flow coriolis flowmeter 20

natural convection heat exchanger in primary loop
diagram label is 21a

we use the convention of top of bypass branch to bottom
hence degree is -90

However in DHX, i also expect there to be
a check valve which only allows flow from top to bottom

That is the forward direction of flow for FM20,

ctah line flowmeter 40
label 14a on simulation diagram
fldk = 18.0+93000/(Re^1.35)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_flowmeter_20_label_21a(initial_temperature: ThermodynamicTemperature) -> super::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_21`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe 21 within CIET DHX loop


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_21(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_20`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe 20 within CIET DHX loop


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_20(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_19`

creates a new component for CIET using the RELAP5-3D and SAM parameters

pipe 19 within CIET DHX loop


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

```rust
pub fn new_pipe_19(initial_temperature: ThermodynamicTemperature) -> super::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

## Module `ciet_steady_state_natural_circulation_test_components`

ciet components for pipes and valves for use in the natural circulation
test. I attempt to reproduce some results in the following
publications:

Zweibaum, N. (2015). Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University
of California, Berkeley.

Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).
CIET steady-state natural-circulation test components and verification suites.

Bundles the pre-built CIET DRACS-loop component constructors
([`dracs_loop_components`]) with the steady-state natural-circulation
verification/validation test groups. These compare the TUAS-computed
natural-circulation loop mass flow rate (kg/s) against Scarlat's analytical
zero-heat-loss solution, the SAM code, and the CIET experimental data across
the three TCHX-outlet-temperature cases A (46 degC), B (35 degC) and
C (40 degC), for both the isolated DRACS loop and the DRACS loop coupled to
the DHX and heater branches.

```rust
pub mod ciet_steady_state_natural_circulation_test_components { /* ... */ }
```

### Modules

## Module `zero_parasitic_heat_loss_isolated_dracs_loop_tests`

For CIET natural circulation tests,

there are two sets of tests.

The first one isolates the DRACS

in this test, the DRACS loop with zero parasitic heat loss is
compared against the analytical solution by Scarlat for a
natural circulation loop with zero heat loss

Three sets of tests are performed.
One with TCHX outlet temperature of 46 C
Second with TCHX outlet temperature of 40 C
Third with TCHX outlet temperature of 35 C

These are enforced as part of the boundary conditions of the TCHX

In the original SAM publication

Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

I found it hard to distinguish what TCHX temperatures case A,
B and C were.

But there was another publication which shows which is test group
corresponds to which temperature:

Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021). Code validation of
SAM using natural-circulation experimental data from the compact
integral effects test (CIET) facility.
Nuclear Engineering and Design, 377, 111144.

According to table 2,

Case A has 7 tests and TCHX out temperature of 46 C
Case B has 9 tests and TCHX out temperature of 35 C
Case C has 9 tests and TCHX out temperature of 40 C

Table 3 also provides the data for these tests. These are included
in the module

For this set of tests, we do not worry about real-time calculations
just yet

SAM max error threshold is about 1%
that is (m_SAM - m_analytical)/m_analytical
Zero-parasitic-heat-loss isolated DRACS loop natural-circulation tests.

Drives the idealised isolated DRACS loop with zero parasitic heat loss (only
the TCHX rejects heat) and compares the steady-state loop mass flow rate
(kg/s) against Scarlat's analytical zero-heat-loss solution and the SAM
reference for the three TCHX-outlet cases: case A (319 K / 46 degC), case B
(308 K / 35 degC) and case C (313 K / 40 degC). Also holds the supporting
thermal-hydraulics, PID-controller and miscellaneous debugging tests, and
the mesh-refinement convergence study.

```rust
pub mod zero_parasitic_heat_loss_isolated_dracs_loop_tests { /* ... */ }
```

### Modules

## Module `case_a`

In the original SAM publication

Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

I found it hard to distinguish what TCHX temperatures case A,
B and C were.

But there was another publication which shows which is test group
corresponds to which temperature:

Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021). Code validation of
SAM using natural-circulation experimental data from the compact
integral effects test (CIET) facility.
Nuclear Engineering and Design, 377, 111144.

According to table 2,

Case A has 7 tests and TCHX out temperature of 46 C
Case B has 9 tests and TCHX out temperature of 35 C
Case C has 9 tests and TCHX out temperature of 40 C

Table 3 also provides the data



```rust
pub mod case_a { /* ... */ }
```

## Module `case_b`

In the original SAM publication

Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

I found it hard to distinguish what TCHX temperatures case A,
B and C were.

But there was another publication which shows which is test group
corresponds to which temperature:

Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021). Code validation of
SAM using natural-circulation experimental data from the compact
integral effects test (CIET) facility.
Nuclear Engineering and Design, 377, 111144.

According to table 2,

Case A has 7 tests and TCHX out temperature of 46 C
Case B has 9 tests and TCHX out temperature of 35 C
Case C has 9 tests and TCHX out temperature of 40 C

Table 3 also provides the data


```rust
pub mod case_b { /* ... */ }
```

## Module `case_c`

In the original SAM publication

Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).

I found it hard to distinguish what TCHX temperatures case A,
B and C were.

But there was another publication which shows which is test group
corresponds to which temperature:

Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021). Code validation of
SAM using natural-circulation experimental data from the compact
integral effects test (CIET) facility.
Nuclear Engineering and Design, 377, 111144.

According to table 2,

Case A has 7 tests and TCHX out temperature of 46 C
Case B has 9 tests and TCHX out temperature of 35 C
Case C has 9 tests and TCHX out temperature of 40 C

Table 3 also provides the data



```rust
pub mod case_c { /* ... */ }
```

## Module `debugging_thermal_hydraulics`

debugging tests for thermal hydraulics and fluid mechanics
functions to make natural circulation
testing easier

```rust
pub mod debugging_thermal_hydraulics { /* ... */ }
```

### Functions

#### Function `dracs_hot_branch_builder`

builds the hot branch of the DRACS loop (somewhat like the hot leg,
but with some other stuff)
this is pipe 30a all the way to 34
but I build the components from top down

```rust
pub fn dracs_hot_branch_builder(initial_temperature: ThermodynamicTemperature) -> crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_collection::FluidComponentCollection { /* ... */ }
```

#### Function `dracs_cold_branch_builder`

builds the cold branch of the DRACS loop (somewhat like the cold leg,
but with some other stuff)

```rust
pub fn dracs_cold_branch_builder(initial_temperature: ThermodynamicTemperature) -> crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_collection::FluidComponentCollection { /* ... */ }
```

## Module `debugging_pid_controller`

debugging tests for PID controller
functions to make natural circulation
testing easier

```rust
pub mod debugging_pid_controller { /* ... */ }
```

## Module `misc_debugging`

miscellaneous debugging tests
for other bugs I happened to find

```rust
pub mod misc_debugging { /* ... */ }
```

## Module `parasitic_heat_loss_regression_tests`

tests for parasitic heat loss regression and calibration tests
this is meant to callibrate CIET's model and also to test if
the wall correction is working properly
Regression and calibration tests for parasitic heat loss in the CIET
natural-circulation loops (DRACS loop and the coupled DRACS + primary loop),
benchmarked against Zou et al. (ANL/NSE-19/11) and Zweibaum's unpublished
CIET data (set C, heater powers ~841-2765 W).

The submodules progress through calibration stages:
- `wall_correction_isolated_dracs_loop_regression` - Gnielinski wall-correction
  factor `(Pr_f/Pr_wall)^0.11` on the isolated DRACS loop.
- `coupled_dracs_loop_ver_1_uncalibrated` - baseline coupled loop, no calibration.
- `coupled_dracs_loop_ver_2_calibrated` / `..._ver_3_calibrated` /
  `..._ver_6_calibrated` - successively calibrated STHE, insulation and
  heater-Nusselt settings.
- `dhx_sthe_calibration` - standalone DHX shell-and-tube heat exchanger (STHE)
  heat-transfer / insulation calibration.
- `primary_loop_parasitic_heat_loss_calibration` /
  `dracs_loop_parasitic_heat_loss_calibration` - per-leg insulation calibration.

Test quantities are natural-circulation mass flow rates (kg/s) in the DRACS
and primary loops, component temperatures (degC), and heater surface
temperatures (degC) at the given heater power (W).

```rust
pub mod parasitic_heat_loss_regression_tests { /* ... */ }
```

### Modules

## Module `wall_correction_isolated_dracs_loop_regression`

for gnielinski type correlations, there is a wall correction
factor which is something like (Pr_f/Pr_w)^0.11

For cooling situations, this lowers the heat transfer
coefficient. To test if this correction factor is working,
I compare the parasitic heat loss of the DRACS loop
without correction factors as a base case,
then switch it on to see if there is less parasitic
heat loss



```rust
pub mod wall_correction_isolated_dracs_loop_regression { /* ... */ }
```

## Module `coupled_dracs_loop_ver_1_uncalibrated`

version 1 of coupled DRACS loop
for version 1 of coupled DRACS loop

no calibration is done. heat exchanger correlations are Gnielinksi type
on the shell side.
There is parasitic heat loss through the heater when there should be
none. (See Zou's publication on nuclear engineering and design in 2021)
this serves as a baseline as to what kind of heat losses to expect
Version 1 (uncalibrated) coupled DRACS + primary loop regression tests.

The DHX shell-and-tube heat exchanger uses uncalibrated Gnielinski
correlations, so parasitic heat losses are un-tuned; these tests establish
the baseline over-prediction of the natural-circulation mass flow rates
(kg/s) in both loops against Zweibaum's CIET set-C data. Heater powers span
~841-2765 W with the TCHX outlet held at 40 degC.

```rust
pub mod coupled_dracs_loop_ver_1_uncalibrated { /* ... */ }
```

### Modules

## Module `regression_coupled_dracs_loop_version_1`

function to test uncalibrated
coupled dracs loop and compare with experimental data
this is more of a regression function, so I want to check the
output of the uncalibrated loop

the DHX here uses uncalibrated Gnielinski correlations
to estimate heat transfer coefficients

```rust
pub mod regression_coupled_dracs_loop_version_1 { /* ... */ }
```

## Module `validate_coupled_dracs_loop_version_1`

function to validate coupled DRACS loop to experimental data
within a given tolerance
version 1,
the DHX here uses uncalibrated Gnielinski correlations
to estimate heat transfer coefficients

```rust
pub mod validate_coupled_dracs_loop_version_1 { /* ... */ }
```

## Module `coupled_dracs_loop_ver_2_calibrated`

version 2 of coupled DRACS loop

for version 2, simple calibration is done
that is, STHE calibration and parasitic heat loss calibration over the loop
the vertical TCHX is not split into equal halves
Version 2 (calibrated) coupled DRACS + primary loop regression tests.

Version 2 applies a simple best-effort calibration: DHX shell-and-tube heat
exchanger (STHE) heat transfer plus per-leg parasitic-heat-loss (insulation
thickness) tuning over the loop. The vertical TCHX is NOT split into equal
halves here. Each set-C data point (C-1 to C-9, heater powers ~841-2765 W)
is checked for DRACS/primary natural-circulation mass flow rate (kg/s)
against experimental data, expecting ~8.5% over-prediction.

```rust
pub mod coupled_dracs_loop_ver_2_calibrated { /* ... */ }
```

### Modules

## Module `regression_coupled_dracs_loop_version_2`

function to test version 2 calibrated
coupled dracs loop and compare with experimental data
this is more of a regression function, so I want to check the
output of the calibrated loop


based on initial calibration with set c,
a best effort was made

for the pri loop
cold leg insulation thickness is 0.15 cm
hot leg insulation thickness is 0.24 cm

for the dracs loop
cold leg insulation thickness is 3cm
hot leg insulation thickness is 0.75 cm

for the DHX STHE,

shell side to tubes nusselt correction factor is 4.7
insulation thickness is 0.161 cm
shell side to ambient correction factor is 10.3
heat loss to ambient is 33.9 W/(m^2 K)

no changes made to tchx yet, I want to calibrate slowly

```rust
pub mod regression_coupled_dracs_loop_version_2 { /* ... */ }
```

## Module `coupled_dracs_loop_ver_3_calibrated`

version 3 of coupled DRACS loop

for version 3, simple calibration is done as with version 2,
but the vertical TCHX is split into two equal halves as was done in SAM,
only the bottom half will have the calibrated heat transfer coefficient.
The rest of the TCHX, the horizontal TCHX and 35b1, will be insulated.
Version 3 (calibrated) coupled DRACS + primary loop regression tests.

Version 3 uses the same STHE / insulation calibration as version 2, but the
vertical TCHX is split into two equal halves as in SAM: only the bottom half
carries the calibrated heat-transfer coefficient, while the rest of the TCHX
(the horizontal TCHX and 35b1) is insulated. Tests are `#[ignore]`d legacy
debugging runs that check DRACS/primary natural-circulation mass flow rates
(kg/s) for set-C data points (heater powers ~841-2765 W) against experimental
data, expecting ~8.5% over-prediction.

```rust
pub mod coupled_dracs_loop_ver_3_calibrated { /* ... */ }
```

### Modules

## Module `regression_coupled_dracs_loop_version_3`

function to test version 3 calibrated
coupled dracs loop and compare with experimental data
this is more of a regression function, so I want to check the
output of the calibrated loop


based on initial calibration with set c,
a best effort was made

for the pri loop
cold leg insulation thickness is 0.15 cm
hot leg insulation thickness is 0.24 cm

for the dracs loop
cold leg insulation thickness is 3cm
hot leg insulation thickness is 0.75 cm

for the DHX STHE,

shell side to tubes nusselt correction factor is 4.7
insulation thickness is 0.161 cm
shell side to ambient correction factor is 10.3
heat loss to ambient is 33.9 W/(m^2 K)

no changes made to tchx yet, I want to calibrate slowly

```rust
pub mod regression_coupled_dracs_loop_version_3 { /* ... */ }
```

## Module `coupled_dracs_loop_ver_6_calibrated`

Version 4 increases the K of pipe 22 to 45.95
Version 5 increases nusselt number of heater 5 times (deprecated now tho)



in this module, I want to calibrate the heater nusselt
number to a suitable value
dataset number,pri loop mass flowrate (kg/s),Heater (heat addition),Heater inlet (DegC),Average Surface T
C-1,0.02003,841.01916,50.5,86.80711,
C-2,0.02367,1158.68584,53.8,96.92176,
C-3,0.02635,1409.2231,56.7,105.23976,
C-4,0.02949,1736.10797,60.8,114.57434,
C-5,0.0319,2026.28588,64.1,122.82384,
C-6,0.03412,2288.8349,67.2,130.37845,
C-7,0.03562,2508.71169,70.6,138.12225,
C-8,0.03593,2685.83341,73.6,145.79877,
C-9,0.03547,2764.52664,76.5,153.29145,

This version attempts to calibrate the nusselt number of the heater
such that the surface temperature of the heater matches that of the
experimental data. Moreover, the heat transfer to ambient is now
set to zero

in SAM, the heat was added directly to the fluid. This is because
the heat addition values were back calculated from thermocouple data
so that the heat addition was a net to the fluid. Therefore, heater
surface temperatures were not even considered. If the surface temperatures
were simulated, then they would be lower than that of the fluid.

Here, I wanted to add a little realism. While there is zero heat loss
to the environment, the heat is added to the metal rather than to
the fluid. Hence, the metallic heater surface temperatures are elevated
to somewhat be closer to experimental data. This would be higher than
that of adding heat directly to the fluid. In this case, even a heater
surface temperature 10 K lower than that of experimental data would be
an improvement over the SAM model. Therefore, low temperature bounds
are still acceptable.

Since the increasing the Nusselt number between shell and fluid
decreases heater surface temperature, I'll just use a high bound
Nusselt number from this set because it is already a correction to
a non-existent or relatively low heater surface temperature from SAM.

Version 6 (calibrated) coupled DRACS + primary loop regression tests.

Building on the version 2/3 STHE and insulation calibration, version 6 also
calibrates the heater Nusselt-number correction factor so the simulated
metallic heater surface temperature (degC) matches the CIET set-C data, with
heat loss to ambient set to zero (heat is added to the metal, not directly
to the fluid, unlike the SAM back-calculated approach). Each set-C point
(C-1 to C-9, heater powers ~841-2765 W) asserts DRACS/primary natural-
circulation mass flow rates (kg/s) and the heater surface temperature (degC).
Tests are `#[ignore]`d legacy debugging runs.

```rust
pub mod coupled_dracs_loop_ver_6_calibrated { /* ... */ }
```

## Module `primary_loop_parasitic_heat_loss_calibration`

for the coupled dracs loop, we need to calibrate heat loss
through the primary loop
hot leg (from heater outlet to dhx shell inlet)
and cold leg
(from dhx shell outlet to heater inlet)

The data in csv format from Zweibaum's unpublished work in Th Lab
Archives is:

dataset number,pri loop mass flowrate (kg/s),Heater outlet (DegC),DHX shell top (DegC),DHX shell bottom (DegC),Heater inlet (DegC),
C-1,0.02003,75.22747,71.47752,53.60943,50.45784,
C-2,0.02367,82.41863,78.36713,57.13467,53.79036,
C-3,0.02635,87.78188,84.37342,59.82845,56.71891,
C-4,0.02949,94.71628,90.97595,63.9812,60.83029,
C-5,0.0319,100.37023,96.20228,67.05336,64.07406,
C-6,0.03412,105.25073,101.3375,69.85085,67.1654,
C-7,0.03562,110.34289,106.43149,73.21226,70.6215,
C-8,0.03593,115.52364,111.37615,76.13202,73.63344,
C-9,0.03547,119.96879,116.05003,79.02407,76.54479,

so that the inlet dhx shell temperature is equal to the set point,
That of the experimental data

repeat the same for the cold leg.

Parasitic-heat-loss calibration for the CIET primary (heater)
natural-circulation loop.

The submodules explore three ways to make the simulated primary loop shed
enough parasitic heat to match Zweibaum's steady-state natural-circulation
data (primary-loop mass flow rate in kg/s; heater and DHX-shell
temperatures in degC): tuning the pipe insulation thickness (in m/cm), the
ambient heat-transfer coefficient (in W/(m^2 K)), and the pipe Nusselt
number multiplier. Only the insulation-thickness route succeeded; the
ambient-htc and Nusselt-number routes are retained as documented failed
attempts (the thermal resistance is dominated by the insulation).

```rust
pub mod primary_loop_parasitic_heat_loss_calibration { /* ... */ }
```

### Modules

## Module `insulation_thickness_calibration`


This module's test attempted to tweak the insulation thickness
to ambient in order to obtain the correct dhx inlet temperature
unfortunately, the parasitic heat losses were not sufficient
to achieve this objective,

The next thing is to do as the RELAP and SAM model did,
which is to increase the overall heat transfer coefficient (U) rather
than just the wall side or ambient air side heat transfer coefficient.
Either by multiplying the heat transfer
area density by a certain amount (SAM) or applying a multiplicative
factor (page 40-41 of Zweibaum's thesis)


Zweibaum, N. (2015). Experimental validation of passive
safety system models: Application to design and optimization
of fluoride-salt-cooled, high-temperature reactors.
University of California, Berkeley.

previous tests indicated that increasing the heat transfer to ambient
or even the nusselt number did not significantly impact parasitic heat
losses. Hence, the way to calibrate parasitic heat loss is by decreasing
the insulation thickness from 0.0508 m to something less.

dataset number,pri loop mass flowrate (kg/s),Heater outlet (DegC),DHX shell top (DegC),DHX shell bottom (DegC),Heater inlet (DegC),
C-1,0.02003,75.22747,71.47752,53.60943,50.45784,
C-2,0.02367,82.41863,78.36713,57.13467,53.79036,
C-3,0.02635,87.78188,84.37342,59.82845,56.71891,
C-4,0.02949,94.71628,90.97595,63.9812,60.83029,
C-5,0.0319,100.37023,96.20228,67.05336,64.07406,
C-6,0.03412,105.25073,101.3375,69.85085,67.1654,
C-7,0.03562,110.34289,106.43149,73.21226,70.6215,
C-8,0.03593,115.52364,111.37615,76.13202,73.63344,
C-9,0.03547,119.96879,116.05003,79.02407,76.54479,
Primary-loop insulation-thickness calibration and validation tests.

These tests tune the pipe insulation thickness (in cm) of the primary-loop
hot leg (heater outlet -> DHX shell inlet) and cold leg (DHX shell bottom
outlet -> heater inlet) so the simulated parasitic heat loss reproduces
Zweibaum's steady-state temperatures (primary-loop mass flow rate in kg/s;
temperatures in degC). A calibrated thickness of about 0.24 cm was found
suitable across the hot-leg datasets.

```rust
pub mod insulation_thickness_calibration { /* ... */ }
```

### Modules

## Module `hot_leg_calibration`

This contains tests for the hot leg calibration of insulation
thickness,

a suitable calibrated thickness was found to be about 0.24 cm

dataset number,pri loop mass flowrate (kg/s),Heater outlet (DegC),DHX shell top (DegC),DHX shell bottom (DegC),Heater inlet (DegC),
C-1,0.02003,75.22747,71.47752,53.60943,50.45784,
C-2,0.02367,82.41863,78.36713,57.13467,53.79036,
C-3,0.02635,87.78188,84.37342,59.82845,56.71891,
C-4,0.02949,94.71628,90.97595,63.9812,60.83029,
C-5,0.0319,100.37023,96.20228,67.05336,64.07406,
C-6,0.03412,105.25073,101.3375,69.85085,67.1654,
C-7,0.03562,110.34289,106.43149,73.21226,70.6215,
C-8,0.03593,115.52364,111.37615,76.13202,73.63344,
C-9,0.03547,119.96879,116.05003,79.02407,76.54479,


```rust
pub mod hot_leg_calibration { /* ... */ }
```

## Module `cold_leg_validation`

This contains tests for the cold leg calibration of insulation
thickness,

a suitable calibrated thickness from the hot leg
was found to be about 0.24 cm

data from the dhx outlet to heater inlet is presented below:

dataset number,pri loop mass flowrate (kg/s),DHX shell bottom (DegC),Heater inlet (DegC),
C-1,0.02003,53.60943,50.45784,
C-2,0.02367,57.13467,53.79036,
C-3,0.02635,59.82845,56.71891,
C-4,0.02949,63.9812,60.83029,
C-5,0.0319,67.05336,64.07406,
C-6,0.03412,69.85085,67.1654,
C-7,0.03562,73.21226,70.6215,
C-8,0.03593,76.13202,73.63344,
C-9,0.03547,79.02407,76.54479,

```rust
pub mod cold_leg_validation { /* ... */ }
```

## Module `heat_transfer_to_ambient_calibration`

This module's test attempted to tweak the heat trasnfer coeffcient (htc)
to ambient in order to obtain the correct dhx inlet temperature
unfortunately, the parasitic heat losses were not sufficient
to achieve this objective,

The next thing is to do as the RELAP and SAM model did,
which is to reduce the convective thermal resistance between
fluid and wall. Either by multiplying the heat transfer
area density by a certain amount (SAM) or applying a multiplicative
factor (page 40-41 of Zweibaum's thesis). This module aims to do that

Zweibaum, N. (2015). Experimental validation of passive
safety system models: Application to design and optimization
of fluoride-salt-cooled, high-temperature reactors.
University of California, Berkeley.

anyhow, this also failed, the multiplicative regression also failed
dhx inlet temperature was still around 74 degrees C even at
multiplcation factors of about ~30 - 15000 (max time = 3000s, little or
no change)

dataset number,pri loop mass flowrate (kg/s),Heater outlet (DegC),DHX shell top (DegC),DHX shell bottom (DegC),Heater inlet (DegC),
C-1,0.02003,75.22747,71.47752,53.60943,50.45784,
C-2,0.02367,82.41863,78.36713,57.13467,53.79036,
C-3,0.02635,87.78188,84.37342,59.82845,56.71891,
C-4,0.02949,94.71628,90.97595,63.9812,60.83029,
C-5,0.0319,100.37023,96.20228,67.05336,64.07406,
C-6,0.03412,105.25073,101.3375,69.85085,67.1654,
C-7,0.03562,110.34289,106.43149,73.21226,70.6215,
C-8,0.03593,115.52364,111.37615,76.13202,73.63344,
C-9,0.03547,119.96879,116.05003,79.02407,76.54479,

```rust
pub mod heat_transfer_to_ambient_calibration { /* ... */ }
```

## Module `pipe_nusselt_number_calibration`

this module's test attempted to tweak the pipe nusselt number
in order to calibrate the parasitic heat losses
The problem is that even after adjusting the nusselt number up
by 20 times, there was no appreciable increase in parasitic
heat loss

I suspected that the parasitic heat loss was dominated by the
insulation thermal resistance. Tested this by tuning the nusselt
number up ~15000 times and the heat transfer to ambient up by
100 times.

It seems that Zweibaum's method and the method used in SAM for adjusting
up heat transfer coefficient is referring to overall heat transfer
coefficient rather than the heat transfer coefficient to air or to
the fluid in the tube

```rust
pub mod pipe_nusselt_number_calibration { /* ... */ }
```

## Module `dracs_loop_parasitic_heat_loss_calibration`

Zweibaum's unpublished data:
dataset number,dracs loop mass flowrate (kg/s),DHX tube top (outlet) (DegC),TCHX inlet (DegC),TCHX outlet(DegC),DHX tube bottom (DegC),
C-1,0.02686,53.00304,51.79332,40.42208,39.84713,
C-2,0.03055,55.30506,54.27495,40.25559,39.73516,
C-3,0.03345,56.82298,55.83001,39.74061,39.2569,
C-4,0.03649,59.44921,58.32055,40.25482,39.86112,
C-5,0.03869,61.31769,60.157,40.37106,40.01355,
C-6,0.04115,62.69342,61.72605,39.97878,39.53125,
C-7,0.04312,64.45658,63.45641,40.24987,39.8924,
C-8,0.04509,66.11271,65.13191,40.14256,39.91183,
C-9,0.04699,67.40722,66.51369,39.87633,39.64593,
Parasitic-heat-loss insulation-thickness calibration for the CIET DRACS
(Direct Reactor Auxiliary Cooling System) natural-circulation loop.

These calibration tests tune the pipe insulation thickness (in cm) of the
DRACS-loop components so the simulated parasitic heat loss reproduces
Zweibaum's steady-state natural-circulation temperature measurements
(DRACS-loop mass flow rate in kg/s; component temperatures in degC). The
two submodules split the loop by leg: the hot leg (DHX tube top outlet ->
TCHX inlet) and the cold leg (TCHX outlet -> DHX tube bottom inlet).

```rust
pub mod dracs_loop_parasitic_heat_loss_calibration { /* ... */ }
```

### Modules

## Module `hot_leg_calibration`

calibration strategy is to adjust insulation thickness
until correct parasitic heat loss is achieved

Zweibaum's unpublished data:
dataset number,dracs loop mass flowrate (kg/s),DHX tube top (outlet) (DegC),TCHX inlet (DegC),TCHX outlet(DegC),DHX tube bottom (DegC),
C-1,0.02686,53.00304,51.79332,40.42208,39.84713,
C-2,0.03055,55.30506,54.27495,40.25559,39.73516,
C-3,0.03345,56.82298,55.83001,39.74061,39.2569,
C-4,0.03649,59.44921,58.32055,40.25482,39.86112,
C-5,0.03869,61.31769,60.157,40.37106,40.01355,
C-6,0.04115,62.69342,61.72605,39.97878,39.53125,
C-7,0.04312,64.45658,63.45641,40.24987,39.8924,
C-8,0.04509,66.11271,65.13191,40.14256,39.91183,
C-9,0.04699,67.40722,66.51369,39.87633,39.64593,

```rust
pub mod hot_leg_calibration { /* ... */ }
```

## Module `cold_leg_calibration`

calibration strategy is to adjust insulation thickness
until correct parasitic heat loss is achieved

Zweibaum's unpublished data:
dataset number,dracs loop mass flowrate (kg/s),DHX tube top (outlet) (DegC),TCHX inlet (DegC),TCHX outlet(DegC),DHX tube bottom (DegC),
C-1,0.02686,53.00304,51.79332,40.42208,39.84713,
C-2,0.03055,55.30506,54.27495,40.25559,39.73516,
C-3,0.03345,56.82298,55.83001,39.74061,39.2569,
C-4,0.03649,59.44921,58.32055,40.25482,39.86112,
C-5,0.03869,61.31769,60.157,40.37106,40.01355,
C-6,0.04115,62.69342,61.72605,39.97878,39.53125,
C-7,0.04312,64.45658,63.45641,40.24987,39.8924,
C-8,0.04509,66.11271,65.13191,40.14256,39.91183,
C-9,0.04699,67.40722,66.51369,39.87633,39.64593,

```rust
pub mod cold_leg_calibration { /* ... */ }
```

## Module `dhx_sthe_calibration`

in this module, I want to calibrate dhx shell and tube heat exchanger (STHE)
heat transfer and calibration.

on page 13 of Zou's publication
Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),

Zou writes that the STHE for the DHX has an underestimated heat transfer
coefficient rather than an overestimated one as mentioned by Zweibaum,
Zou attributes this to a typo error as increased heat transfer area
densities were used.

Again set C is used to calibrate the DHX data

pri loop is shell side flowrate, dracs loop is tube side flowrate
dataset number,pri loop mass flowrate (kg/s),DRACS loop mass flowrate (kg/s),DHX shell top inlet (DegC),DHX tube bottom inlet(DegC),DHX shell bottom outlet (DegC),DHX tube top outlet (DegC),
C-1,0.02003,0.02686,71.47752,39.84713,53.60943,53.00304,
C-2,0.02367,0.03055,78.36713,39.73516,57.13467,55.30506,
C-3,0.02635,0.03345,84.37342,39.2569,59.82845,56.82298,
C-4,0.02949,0.03649,90.97595,39.86112,63.9812,59.44921,
C-5,0.0319,0.03869,96.20228,40.01355,67.05336,61.31769,
C-6,0.03412,0.04115,101.3375,39.53125,69.85085,62.69342,
C-7,0.03562,0.04312,106.43149,39.8924,73.21226,64.45658,
C-8,0.03593,0.04509,111.37615,39.91183,76.13202,66.11271,
C-9,0.03547,0.04699,116.05003,39.64593,79.02407,67.40722,

The calibration process was this:

(1) set insulation to about 0.15 cm, it was a value that worked for
other calibration
(2) calibrate nusselt number for shell and tube sides accordingly,
just use whatever value works
(3) take arithmetic average of calibrated nusselt numbers and check against
set A, B and C data for mass flowrate over the loop

this is a best effort estimate, I couldn't have one set of parameters to fit
everything, so each test set in dataset c has its own parameters
Standalone DHX shell-and-tube heat exchanger (STHE) calibration/regression
tests, decoupled from the full natural-circulation loop.

Each `dhx_regression_set_c*` test drives the DHX alone with the set-C
primary (shell-side) and DRACS (tube-side) mass flow rates (kg/s) and inlet
temperatures (degC) from Zweibaum's unpublished CIET data, then checks the
shell-side and tube-side outlet temperatures (degC) against the experimental
set point (within 0.5 K) and a tighter regression value (within 0.05 K).
Calibration knobs are the shell-side-to-tubes Nusselt correction factor, the
insulation thickness (cm), and the shell-side-to-ambient Nusselt correction
factor. Sets C-1 to C-8 use `calibration_version_2` (adds ambient heat-loss
tuning); set C-9 uses `calibration_version_1`. See Zou et al. (ANL/NSE-19/11).

```rust
pub mod dhx_sthe_calibration { /* ... */ }
```

### Modules

## Module `calibration_version_1`



in this module, I want to calibrate dhx shell and tube heat exchanger (STHE)
heat transfer and calibration.

on page 13 of Zou's publication
Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),

Zou writes that the STHE for the DHX has an underestimated heat transfer
coefficient rather than an overestimated one as mentioned by Zweibaum,
Zou attributes this to a typo error as increased heat transfer area
densities were used.

Again set C is used to calibrate the DHX data

Zweibaum's unpublished data:
pri loop is shell side flowrate, dracs loop is tube side flowrate
dataset number,pri loop mass flowrate (kg/s),DRACS loop mass flowrate (kg/s),DHX shell top inlet (DegC),DHX tube bottom inlet(DegC),DHX shell bottom outlet (DegC),DHX tube top outlet (DegC),
C-1,0.02003,0.02686,71.47752,39.84713,53.60943,53.00304,
C-2,0.02367,0.03055,78.36713,39.73516,57.13467,55.30506,
C-3,0.02635,0.03345,84.37342,39.2569,59.82845,56.82298,
C-4,0.02949,0.03649,90.97595,39.86112,63.9812,59.44921,
C-5,0.0319,0.03869,96.20228,40.01355,67.05336,61.31769,
C-6,0.03412,0.04115,101.3375,39.53125,69.85085,62.69342,
C-7,0.03562,0.04312,106.43149,39.8924,73.21226,64.45658,
C-8,0.03593,0.04509,111.37615,39.91183,76.13202,66.11271,
C-9,0.03547,0.04699,116.05003,39.64593,79.02407,67.40722,

To calibrate,

(1) first adjust the shell side to tubes nusselt number
until the tube side outlet temperature is correct,

(2) secondly, adjust the insulation thickness until the shell side
outlet temperature is correct

unfortunately, calibration version 1 is not able to account for the
voracious amount of parasitic heat loss from c5 to c7
I probably need to tweak the heat transfer to ambient as well

```rust
pub mod calibration_version_1 { /* ... */ }
```

## Module `calibration_version_2`



in this module, I want to calibrate dhx shell and tube heat exchanger (STHE)
heat transfer and calibration.

on page 13 of Zou's publication
Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),

Zou writes that the STHE for the DHX has an underestimated heat transfer
coefficient rather than an overestimated one as mentioned by Zweibaum,
Zou attributes this to a typo error as increased heat transfer area
densities were used.

Again set C is used to calibrate the DHX data

Zweibaum's unpublished data:
pri loop is shell side flowrate, dracs loop is tube side flowrate
dataset number,pri loop mass flowrate (kg/s),DRACS loop mass flowrate (kg/s),DHX shell top inlet (DegC),DHX tube bottom inlet(DegC),DHX shell bottom outlet (DegC),DHX tube top outlet (DegC),
C-1,0.02003,0.02686,71.47752,39.84713,53.60943,53.00304,
C-2,0.02367,0.03055,78.36713,39.73516,57.13467,55.30506,
C-3,0.02635,0.03345,84.37342,39.2569,59.82845,56.82298,
C-4,0.02949,0.03649,90.97595,39.86112,63.9812,59.44921,
C-5,0.0319,0.03869,96.20228,40.01355,67.05336,61.31769,
C-6,0.03412,0.04115,101.3375,39.53125,69.85085,62.69342,
C-7,0.03562,0.04312,106.43149,39.8924,73.21226,64.45658,
C-8,0.03593,0.04509,111.37615,39.91183,76.13202,66.11271,
C-9,0.03547,0.04699,116.05003,39.64593,79.02407,67.40722,

To calibrate,

(1) first adjust the shell side to tubes nusselt number
until the tube side outlet temperature is correct,

(2) secondly, adjust the insulation thickness until the shell side
outlet temperature is correct

version 2 additionally exposes a heat-transfer-to-ambient calibration
(`heat_loss_to_ambient_watts_per_m2_kelvin`), which version 1 lacked, so it
can account for the larger parasitic heat loss seen from c5 to c7 that
version 1 could not reproduce.

```rust
pub mod calibration_version_2 { /* ... */ }
```

## Module `coupled_dracs_loop_tests`

For validation, real tests were done for the dracs loop coupled
with the DHX and Heater branches in CIET

The relevant publications where the experimental data was pulled from
was:

Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021). Code validation of
SAM using natural-circulation experimental data from the compact
integral effects test (CIET) facility.
Nuclear Engineering and Design, 377, 111144.

According to table 2,

Case A has 7 tests and TCHX out temperature of 46 C
Case B has 9 tests and TCHX out temperature of 35 C
Case C has 9 tests and TCHX out temperature of 40 C

SAM max error threshold is about 6.76%
that is (m_SAM - m_experimental)/m_experimental


CIET coupled DRACS + primary-loop steady-state natural-circulation tests.

This module groups everything needed to run and verify the CIET compact
integral effects test (CIET) coupled natural-circulation steady states, in
which the primary (heater–DHX) loop is thermally coupled to the DRACS loop
through the DHX shell-and-tube heat exchanger. It contains:

- `dracs_loop_calc_functions_no_tchx_calibration` /
  `dracs_loop_calc_functions_sam_tchx_calibration` — per-timestep fluid-
  mechanics (natural-circulation mass flow rate, kg/s) and heat-transfer
  advance functions for the DRACS loop, without and with the SAM TCHX split.
- `pri_loop_calc_functions` — the same for the primary heater–DHX loop.
- `dhx_constructor` — builds the DHX shell-and-tube heat exchanger.
- `dataset_a` / `dataset_b` / `dataset_c` — the coupled regression/validation
  tests for CIET test sets A (TCHX out 46 degC), B (35 degC) and C (40 degC),
  asserting simulated DRACS and primary mass flow rates (kg/s) and component
  temperatures (degC) against the experimental / SAM values in Zou et al.
- `isolated_dracs_loop_resistance_calibration` — pipe-38 form-loss (K)
  calibration of the DRACS loop against the SAM solution.
- `sam_vs_tuas_vs_experiment_summary` / `sam_table4_flowrates_kg_per_s` — the
  SAM-vs-TUAS-vs-experiment comparison report and NED-2021 Table 4 data.

References throughout are Zou, Hu & Charpentier (2019, ANL/NSE-19/11) and
Zou, Hu, O'Grady & Hu (2021, Nuclear Engineering and Design 377, 111144).

```rust
pub mod coupled_dracs_loop_tests { /* ... */ }
```

### Modules

## Module `dracs_loop_calc_functions_no_tchx_calibration`

functions used for calculating the thermal hydraulics inside the DRACS
loop

mostly without tchx calibration, that is to say,
the vertical TCHX is not split into two parts as was done in SAM:

Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021). Code validation of
SAM using natural-circulation experimental data from the compact
integral effects test (CIET) facility.
Nuclear Engineering and Design, 377, 111144.


```rust
pub mod dracs_loop_calc_functions_no_tchx_calibration { /* ... */ }
```

### Functions

#### Function `get_abs_mass_flowrate_across_two_branches`

fluid mechanics bit
calculate the fluid mechanics for the two branches in parallel

In actual fact though, it is just one branch and we are getting
the mass flowrate through that branch,

can be used for DRACS
or the
DHX + Heater branch (both branches form one loop)

but its use is primarily for the DRACS branch

```rust
pub fn get_abs_mass_flowrate_across_two_branches(dracs_branches: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_super_collection::FluidComponentSuperCollection) -> MassRate { /* ... */ }
```

#### Function `coupled_dracs_fluid_mechanics_calc_abs_mass_rate_no_tchx_calibration`

fluid mechanics calcs, specific to the DRACS loop
note that this only works if the components are correct
obtains mass flowrate across the DRACS loop
gets the absolute flowrate across the hot branch

```rust
pub fn coupled_dracs_fluid_mechanics_calc_abs_mass_rate_no_tchx_calibration(pipe_34: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_heat_exchanger_30: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component::FluidComponent, dhx_tube_side_30a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) -> MassRate { /* ... */ }
```

#### Function `coupled_dracs_loop_link_up_components_no_tchx_calibration`

now the heat transfer for the DRACS loop
for a single timestep, given mass flowrate in a counter clockwise
fashion in the DRACS

you also must specify the heat transfer coefficient to ambient
which is assumed to be the same throughout the loop


for DHX, the flow convention is going from top to bottom for both
shell and tube. The code is written such that components are linked
in a clockwise fashion, so that flow goes from top to bottom
in the tube side of the DHX.

the mass_flowrate_counter_clockwise you provide will be converted
into a mass_flowrate_clockwise and used for calculation

```rust
pub fn coupled_dracs_loop_link_up_components_no_tchx_calibration(mass_flowrate_counter_clockwise: MassRate, tchx_heat_transfer_coeff: HeatTransfer, average_temperature_for_density_calcs: ThermodynamicTemperature, ambient_htc: HeatTransfer, pipe_34: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_sthe: &mut crate::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger, dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) { /* ... */ }
```

#### Function `dracs_loop_advance_timestep_except_dhx_no_tchx_calibration`

now the heat transfer for the DRACS loop
for a single timestep, given mass flowrate in a counter clockwise
fashion in the DRACS

you also must specify the heat transfer coefficient to ambient
which is assumed to be the same throughout the loop

```rust
pub fn dracs_loop_advance_timestep_except_dhx_no_tchx_calibration(timestep: Time, pipe_34: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) { /* ... */ }
```

#### Function `dracs_loop_dhx_tube_temperature_diagnostics`

these are temperature diagnostic
functions to check bulk and wall temperature before
and after the DHX tube side

before dhx tube: BT-60, WT-61 (not exactly sure where)
use pipe_30a
after dhx tube: BT-23, WT-22
use pipe_30b


```rust
pub fn dracs_loop_dhx_tube_temperature_diagnostics(dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, print_debug_results: bool) -> ((ThermodynamicTemperature, ThermodynamicTemperature), (ThermodynamicTemperature, ThermodynamicTemperature)) { /* ... */ }
```

#### Function `dracs_loop_tchx_temperature_diagnostics`

these are temperature diagnostic
functions to check bulk and wall temperature before and after
the tchx



```rust
pub fn dracs_loop_tchx_temperature_diagnostics(pipe_34: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_60_label_36: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, print_debug_results: bool) -> ((ThermodynamicTemperature, ThermodynamicTemperature), (ThermodynamicTemperature, ThermodynamicTemperature)) { /* ... */ }
```

## Module `dracs_loop_calc_functions_sam_tchx_calibration`

functions used for calculating the thermal hydraulics inside the DRACS
loop

mostly with tchx calibration, that is to say,
the vertical TCHX is split into two parts (35b-1 and 35b-2)
as was done in SAM:

Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021). Code validation of
SAM using natural-circulation experimental data from the compact
integral effects test (CIET) facility.
Nuclear Engineering and Design, 377, 111144.


```rust
pub mod dracs_loop_calc_functions_sam_tchx_calibration { /* ... */ }
```

### Functions

#### Function `get_abs_mass_flowrate_across_two_branches`

fluid mechanics bit
calculate the fluid mechanics for the two branches in parallel

In actual fact though, it is just one branch and we are getting
the mass flowrate through that branch,

can be used for DRACS
or the
DHX + Heater branch (both branches form one loop)

but its use is primarily for the DRACS branch

```rust
pub fn get_abs_mass_flowrate_across_two_branches(dracs_branches: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_super_collection::FluidComponentSuperCollection) -> MassRate { /* ... */ }
```

#### Function `coupled_dracs_fluid_mechanics_calc_abs_mass_rate_sam_tchx_calibration`

fluid mechanics calcs, specific to the DRACS loop
note that this only works if the components are correct
obtains mass flowrate across the DRACS loop
gets the absolute flowrate across the hot branch

```rust
pub fn coupled_dracs_fluid_mechanics_calc_abs_mass_rate_sam_tchx_calibration(pipe_34: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_heat_exchanger_30: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component::FluidComponent, dhx_tube_side_30a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_1: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_2: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) -> MassRate { /* ... */ }
```

#### Function `coupled_dracs_loop_link_up_components_sam_tchx_calibration`

now the heat transfer for the DRACS loop
for a single timestep, given mass flowrate in a counter clockwise
fashion in the DRACS

you also must specify the heat transfer coefficient to ambient
which is assumed to be the same throughout the loop


for DHX, the flow convention is going from top to bottom for both
shell and tube. The code is written such that components are linked
in a clockwise fashion, so that flow goes from top to bottom
in the tube side of the DHX.

the mass_flowrate_counter_clockwise you provide will be converted
into a mass_flowrate_clockwise and used for calculation

```rust
pub fn coupled_dracs_loop_link_up_components_sam_tchx_calibration(mass_flowrate_counter_clockwise: MassRate, tchx_heat_transfer_coeff: HeatTransfer, average_temperature_for_density_calcs: ThermodynamicTemperature, ambient_htc: HeatTransfer, pipe_34: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_sthe: &mut crate::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger, dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_1: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_2: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) { /* ... */ }
```

#### Function `dracs_loop_advance_timestep_except_dhx_sam_tchx_calibration`

now the heat transfer for the DRACS loop
for a single timestep, given mass flowrate in a counter clockwise
fashion in the DRACS

you also must specify the heat transfer coefficient to ambient
which is assumed to be the same throughout the loop

```rust
pub fn dracs_loop_advance_timestep_except_dhx_sam_tchx_calibration(timestep: Time, pipe_34: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_1: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_2: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) { /* ... */ }
```

#### Function `dracs_loop_dhx_tube_temperature_diagnostics`

these are temperature diagnostic
functions to check bulk and wall temperature before
and after the DHX tube side

before dhx tube: BT-60, WT-61 (not exactly sure where)
use pipe_30a
after dhx tube: BT-23, WT-22
use pipe_30b


```rust
pub fn dracs_loop_dhx_tube_temperature_diagnostics(dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, print_debug_results: bool) -> ((ThermodynamicTemperature, ThermodynamicTemperature), (ThermodynamicTemperature, ThermodynamicTemperature)) { /* ... */ }
```

## Module `pri_loop_calc_functions`

functions used for calculating the thermal hydraulics inside
the Heater and DHX branch
Note: heater v1.0 is used

```rust
pub mod pri_loop_calc_functions { /* ... */ }
```

### Functions

#### Function `get_abs_mass_flowrate_across_two_branches`

fluid mechanics bit
calculate the fluid mechanics for the two branches in parallel

In actual fact though, it is just one branch and we are getting
the mass flowrate through that branch,
can be used for
DHX + Heater branch (both branches form one loop)

```rust
pub fn get_abs_mass_flowrate_across_two_branches(dhx_and_heater_branches: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_super_collection::FluidComponentSuperCollection) -> MassRate { /* ... */ }
```

#### Function `coupled_dracs_pri_loop_branches_fluid_mechanics_calc_abs_mass_rate`

fluid mechanics calcs, specific to the primary (DHX plus Heater branch) loop
note that this only works if the components are correct
obtains mass flowrate across the primary (DHX plus Heater branch) loop
for hydrostatic pressure, all component angles are taken with
reference to the branching point (around 5a)

note: only Flowmeter is  non insulated, all other components
should be insulated

for hydrostatic pressure calculations, note that the angle
of the dhx shell side is going from top to bottom


```rust
pub fn coupled_dracs_pri_loop_branches_fluid_mechanics_calc_abs_mass_rate(pipe_4: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_3: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_2a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_10_label_2: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_top_head_1a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_version1_1: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_bottom_head_1b: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_18: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_26: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_25a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_21_label_25: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_shell_side_pipe_24: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component::FluidComponent, static_mixer_20_label_23: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_23a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_22: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_20_21a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_21: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_20: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_19: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17b: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) -> MassRate { /* ... */ }
```

#### Function `coupled_dracs_pri_loop_dhx_heater_link_up_components`

now the heat transfer for the DRACS loop
for a single timestep, given mass flowrate in a counter clockwise
fashion in the DRACS

you also must specify the heat transfer coefficient to ambient
which is assumed to be the same throughout the loop

flow goes downwards by default through the DHX
to facilitate this, components are linked in a counter clockwise
fashion in the primary loop

```rust
pub fn coupled_dracs_pri_loop_dhx_heater_link_up_components(mass_flowrate_counter_clockwise: MassRate, heat_rate_through_heater: Power, average_temperature_for_density_calcs: ThermodynamicTemperature, ambient_htc: HeatTransfer, pipe_4: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_3: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_2a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_10_label_2: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_top_head_1a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_version1_1: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_bottom_head_1b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_18: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_26: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_25a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_21_label_25: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_sthe: &mut crate::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger, static_mixer_20_label_23: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_23a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_22: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_20_21a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_21: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_20: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_19: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) { /* ... */ }
```

#### Function `pri_loop_advance_timestep_dhx_br_and_heater_br_except_dhx`

advances timestep for all components in primary loop except DHX

```rust
pub fn pri_loop_advance_timestep_dhx_br_and_heater_br_except_dhx(timestep: Time, pipe_4: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_3: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_2a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_10_label_2: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_top_head_1a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_version1_1: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_bottom_head_1b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_18: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_26: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_25a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_21_label_25: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_20_label_23: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_23a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_22: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_20_21a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_21: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_20: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_19: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) { /* ... */ }
```

#### Function `pri_loop_heater_temperature_diagnostics`

these are temperature diagnostic
functions to check bulk and wall temperature before
and after the heater

before heater: BT-11, WT-10
after heater and MX-10: BT-12, WT-13

so can take heater bottom head temperature (1b) at wall
and at bulk

I'm also using the bulk fluid temperature inside the static mixer
and its wall temperature
as a proxy for BT-12 and WT-12

```rust
pub fn pri_loop_heater_temperature_diagnostics(heater_bottom_head_1b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_10_label_2: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, print_debug_results: bool) -> ((ThermodynamicTemperature, ThermodynamicTemperature), (ThermodynamicTemperature, ThermodynamicTemperature)) { /* ... */ }
```

#### Function `pri_loop_dhx_shell_temperature_diagnostics`

these are temperature diagnostic
functions to check bulk and wall temperature before
and after the DHX shell side

before dhx shell: BT-21, WT-20 (just before MX-21)
use pipe_25a
after dhx shell and MX-20: BT-27, WT-26
use MX-20


```rust
pub fn pri_loop_dhx_shell_temperature_diagnostics(pipe_25a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_20_label_23: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, print_debug_results: bool) -> ((ThermodynamicTemperature, ThermodynamicTemperature), (ThermodynamicTemperature, ThermodynamicTemperature)) { /* ... */ }
```

## Module `dhx_constructor`

constructor for the dhx shell and tube heat exchanger
based on Zou's specifications

```rust
pub mod dhx_constructor { /* ... */ }
```

### Functions

#### Function `new_dhx_sthe_version_1`

constructs a new instance of the shell and tube
heat exchanger for the DHX based on Zou's specifications
for the flow area, hydraulic diameter and number of tubes
but Zweibaum's specifications for insulation thickness


the heat transfer coefficients are based on Gnielinski
correlation

Whereas hydrodynamically, the DHX shell and tube
sides are modelled as pipes with K values of 23.9 on
the shell side and 3.3 on the tube side
insulation thickness for DHX is 0.0508 m of fiberglass
DHX is made of copper tubing on the inside
and assumed to be copper on shell side as well

```rust
pub fn new_dhx_sthe_version_1(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger { /* ... */ }
```

#### Function `new_dhx_sthe_version_1_mesh_refined`

constructs a new instance of the shell and tube
heat exchanger for the DHX based on Zou's specifications
for the flow area, hydraulic diameter and number of tubes
but Zweibaum's specifications for insulation thickness


the heat transfer coefficients are based on Gnielinski
correlation

Whereas hydrodynamically, the DHX shell and tube
sides are modelled as pipes with K values of 23.9 on
the shell side and 3.3 on the tube side
insulation thickness for DHX is 0.0508 m of fiberglass
DHX is made of copper tubing on the inside
and assumed to be copper on shell side as well

This is the axially mesh-refined variant of [`new_dhx_sthe_version_1`]:
it is identical except that the tube and shell fluid arrays use 17 inner
nodes (19 total) instead of 9 inner nodes (11 total), for finer axial
resolution.

```rust
pub fn new_dhx_sthe_version_1_mesh_refined(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger { /* ... */ }
```

## Module `debugging`

debugging tests for functions to make natural circulation
testing easier

```rust
pub mod debugging { /* ... */ }
```

## Module `dracs_loop_components`

components for the DRACS loop

```rust
pub mod dracs_loop_components { /* ... */ }
```

### Functions

#### Function `new_pipe_34`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

pipe 34, the horizontal pipe just besides the NDHX, the
heat exchanger cooling the DRACS loop


```rust
pub fn new_pipe_34(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_33`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

pipe 33, the long vertical pipe just in the hot leg


```rust
pub fn new_pipe_33(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_32`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

pipe 32, the slanted pipe just in the hot leg next to the static mixer


```rust
pub fn new_pipe_32(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_static_mixer_61_label_31`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

static mixer pipe label 31, it is static mixer 61 on the P&ID for CIET



```rust
pub fn new_static_mixer_61_label_31(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_31a`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

static mixer pipe 31a, the slanted pipe just in the hot leg next to the static mixer


```rust
pub fn new_pipe_31a(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_dhx_tube_side_30b`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

DHX tube side (top head) 30b


```rust
pub fn new_dhx_tube_side_30b(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_isolated_dhx_tube_side_30`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

DHX tube side 30

Here is where the main heat exchange happens.
This one is for an isolated DHX.

Alternate code is needed for a coupled DHX


```rust
pub fn new_isolated_dhx_tube_side_30(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_isolated_dhx_tube_side_30_parallel_tubes`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

DHX tube side 30 with parallel tube modelling

Here is where the main heat exchange happens.
This one is for an isolated DHX.

Alternate code is needed for a coupled DHX


```rust
pub fn new_isolated_dhx_tube_side_30_parallel_tubes(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_parallel_fluid_components::NonInsulatedParallelFluidComponent { /* ... */ }
```

#### Function `new_dhx_tube_side_30a`

hot leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

DHX tube side (bottom head) 30a


```rust
pub fn new_dhx_tube_side_30a(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_ndhx_tchx_horizontal_35a`

cold leg of DRACS (or what I consider the cold branch)

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.


label 35a on RELAP model by Zweibaum
horizontal part of the TCHX or NDHX,
has the same loss correlations as the CTAH (horizontal)


```rust
pub fn new_ndhx_tchx_horizontal_35a(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_ndhx_tchx_vertical_35b`

cold leg of DRACS (or what I consider the cold branch)

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.


label 35b on RELAP model by Zweibaum
vertical part of the TCHX or NDHX,
has the same loss correlations as the CTAH (horizontal)


```rust
pub fn new_ndhx_tchx_vertical_35b(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_ndhx_tchx_vertical_35b_1`

cold leg of DRACS (or what I consider the cold branch)

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),

note that for coupled natural circulation dracs loop calibration
tchx pipe 35b is evenly split into 35b-1 and 35b-2
35b-1 is adiabatic towards the environment

label 35b-1 on SAM model by Zweibaum
vertical part of the TCHX or NDHX,
has the same loss correlations as the CTAH (horizontal)


```rust
pub fn new_ndhx_tchx_vertical_35b_1(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_ndhx_tchx_vertical_35b_2`

cold leg of DRACS (or what I consider the cold branch)

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),

note that for coupled natural circulation dracs loop calibration
tchx pipe 35b is evenly split into 35b-1 and 35b-2
35b-2 is not adiabatic towards the environment

label 35b-2 on SAM model by Zweibaum
vertical part of the TCHX or NDHX,
has the same loss correlations as the CTAH (horizontal)


```rust
pub fn new_ndhx_tchx_vertical_35b_2(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_36a`

cold leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

static mixer pipe 36a, the static mixer pipe next to the NDHX a.k.a TCHX


```rust
pub fn new_pipe_36a(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_static_mixer_60_label_36`

cold leg of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

static mixer MX-60 label 36


```rust
pub fn new_static_mixer_60_label_36(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_37`

cold leg (or branch) of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

pipe 37, a pipe next to MX-60


```rust
pub fn new_pipe_37(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_flowmeter_60_37a`

cold leg (or branch) of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

static flowmeter label 37a


```rust
pub fn new_flowmeter_60_37a(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_38`

cold leg (or branch) of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

pipe 38


```rust
pub fn new_pipe_38(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_38_sam_model`

cold leg (or branch) of DRACS — pipe 38, SAM-calibrated resistance variant

Identical geometry to [`new_pipe_38`] but carries the **SAM-matched form
loss K = 17.8** instead of the Zweibaum RELAP value K = 0.8.

# Physical quantity

Builds the insulated DRACS cold-leg pipe 38 (an [`InsulatedFluidComponent`])
used in the coupled natural-circulation loop. The form loss (dimensionless
K) sets this segment's minor pressure-loss contribution to the DRACS loop
hydraulic resistance.

# Why a separate constructor (provenance)

The RELAP5-3D value K = 0.8 in [`new_pipe_38`] gives too little DRACS loop
resistance, so the coupled loop over-predicts the DRACS natural-circulation
mass flow by +4.1…+5.5% vs the CIET experimental data (a documented bias;
see the module doc in `coupled_dracs_loop_tests`). Recalibrating pipe 38 to
K = 17.8 matches the SAM model: with it the isolated DRACS loop reproduces
SAM to <0.35% (worst-case error improves from +2.14% at K=0.8 to +0.14% at
K=17.8, validated in `isolated_dracs_loop_resistance_calibration`).

This mirrors the existing primary-loop precedent where
`new_pipe_3_sam_model` (K = 17.15) and `new_pipe_22_sam_model` (K = 45.95)
coexist beside their RELAP `new_pipe_3` / `new_pipe_22` counterparts.

# Where it is used (adopted 2026-07-15)

This is the DRACS cold-leg pipe-38 constructor used by the coupled A/B/C
natural-circulation regression tests (`dataset_a/b/c`), the educational-
simulator GUI/prototypes, and `isolated_dracs_loop_resistance_calibration`
(which validates K = 17.8 against the SAM isolated-DRACS reference, <0.35%).
The RELAP [`new_pipe_38`] (K = 0.8) is retained only for the legacy
uncalibrated `ver_1` and zero-parasitic references.

Adopting K = 17.8 in the coupled loop improves the mean DRACS agreement
(mean |error| 3.83% -> 2.76%) and fixes the documented mid/high-flow
over-prediction (e.g. A4 +5.4% -> +3.3%, B8 +7.5% -> +5.4%). It does *not*
help the two lowest-flow cases — B1 (655 W) -5.44% -> -6.62% and C1 (841 W)
-5.41% -> -6.80% — which already under-predict at K = 0.8; form loss scales
with velocity squared, so added resistance only deepens a low-flow
under-prediction it cannot lift. Those two carry a documented per-point
widened DRACS tolerance (see `dataset_b`/`dataset_c` and the
`coupled_dracs_loop_tests` module doc); the proper physics fix, a
velocity/Reynolds-dependent pipe-38 loss, is tracked as bead op-4wl.5.

# Assumptions / valid range

Same as [`new_pipe_38`]: Therminol VP-1 working fluid, SS-304L shell with
fiberglass insulation, 3-node SAM nodalization, initial temperature supplied
by the caller. Only the form loss differs.

# References

Zou, L., Hu, R., & Charpentier, A. (2019). SAM code validation using the
compact integral effects test (CIET) experimental data (No. ANL/NSE-19/11).
Argonne National Laboratory, IL.

Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021). Code validation of SAM using
natural-circulation experimental data from the compact integral effects
test (CIET) facility. Nuclear Engineering and Design, 377, 111144.

Zweibaum, Nicolas. Experimental validation of passive safety system models:
Application to design and optimization of fluoride-salt-cooled,
high-temperature reactors. University of California, Berkeley, 2015.

```rust
pub fn new_pipe_38_sam_model(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

#### Function `new_pipe_39`

cold leg (or branch) of DRACS

note that we will rotate these components by 180 degrees
for only the hot leg, as the DRACS loop in RELAP is programmed
in a counter clockwise fashion (see Nico Zweibaum's thesis)

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

pipe 39, bottom of cold leg


```rust
pub fn new_pipe_39(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent { /* ... */ }
```

## Module `uw_madison_flibe_loop_components`

From UW Madison FLiBe loop:


Britsch, K., Anderson, M., Brooks, P., &
Sridharan, K. (2019). Natural circulation
FLiBe loop overview. International Journal of
Heat and Mass Transfer, 134, 970-983.
Pre-built components for the University of Wisconsin-Madison FLiBe
(molten fluoride salt, LiF-BeF2) forced/natural-circulation loop.

The physical loop and its instrumentation are described in:
Britsch, K., Anderson, M., Brooks, P., & Sridharan, K. (2019).
Natural circulation FLiBe loop overview. International Journal of
Heat and Mass Transfer, 134, 970-983.

This module is organised as two successive modelling iterations of the
same 13-component loop (all pipes are 1 inch / 2.54 cm outer diameter with
a 3 mm wall, so the inner diameter is 2.54 cm - 2*3 mm; FLiBe is the working
fluid). Component lengths are in metres, incline angles in degrees:

- [`flibe_loop_iteration_one`] — first-cut loop: adiabatic pipe components,
  parasitic-heat-loss calibration against the reference test tables, and
  the fluid-mechanics / thermal-hydraulics calculation routines that link
  the components into a single circulating branch and advance it in time.
- [`flibe_loop_iteration_two`] — adds an explicit clamshell radiative
  heater sub-component to capture the heat added (and lost) by the loop's
  clamshell radiative heating elements, which iteration one could not
  represent.

Both iteration modules are currently gated behind `#[cfg(test)]` — they are
used from the loop's verification/simulation tests rather than the public
library surface.

```rust
pub mod uw_madison_flibe_loop_components { /* ... */ }
```

## Module `ciet_three_branch_plus_dracs`

Based on the natural circulation and isothermal loop,
I'm constructing CIET as a full loop now, inclusive of all 3
branches.

todo: will need to validate this loop for steady state,
natural circulation flow, and transient response (freq response
testing)


Zweibaum, N. (2015). Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University
of California, Berkeley.

Zou, L., Hu, R., & Charpentier, A. (2019). SAM code
validation using the compact integral effects test (CIET) experimental
data (No. ANL/NSE-19/11). Argonne National
Lab.(ANL), Argonne, IL (United States).
CIET three-branch primary loop plus the DRACS passive decay-heat
removal loop.

This module assembles the Compact Integral Effects Test (CIET)
facility as three primary-loop branches — the heater branch, the
CTAH (Coiled Tube Air Heater) branch used for forced circulation,
and the DHX (DRACS Heat Exchanger) branch — coupled to the DRACS
natural-circulation loop and its TCHX (Thermosyphon-Cooled Heat
eXchanger). It provides:

- [`components`] — the extra CIET components specific to the
  three-branch layout (the rest are reused from the isothermal and
  steady-state natural-circulation modules).
- [`solver_functions`] — the thermal-hydraulic link-up, fluid-flow
  and timestep-advance routines for the DRACS loop and the three
  primary-loop branches.
- [`ciet_educational_simulator_loop_prototypes`] — successive
  real-time educational simulator prototypes (versions 1-3).

Temperatures are in K / degC, mass flow in kg/s, power/heat transfer
in W, pressure drop in Pa, and lengths in m throughout (carried as
`uom` dimensioned quantities on the public component APIs).

```rust
pub mod ciet_three_branch_plus_dracs { /* ... */ }
```

### Modules

## Module `ciet_educational_simulator_loop_prototypes`

this version of ciet is optimised for real-time
simulation. It will not be validated, but it will be
fun to play with as a simulator. Useful for education and etc.

Also included here is some csv data for ciet's ctah loop forced circulation
transients
Real-time educational simulator prototypes for the CIET three-branch
plus DRACS loop.

Each `version_*` submodule is a successive iteration of the loop
solver used as a playable educational simulator (not a validated
model):

- [`version_1`] — mass flowrates solved serially; no CTAH PID
  control yet.
- [`version_2`] — adds CTAH (and TCHX) PID control, still single
  threaded.
- [`version_3`] — adds thread-based parallelism over version 2 to
  run the loop faster.

[`regression_tests`] guards that these changes still reproduce the
expected physics (e.g. the natural-circulation loop). Time is in
seconds, temperatures in degC, mass flow in kg/s and power in W in
these prototype drivers.

```rust
pub mod ciet_educational_simulator_loop_prototypes { /* ... */ }
```

### Modules

## Module `version_3`

version 3 adds parallelism to version 2,
hopefully to get it faster
Version 3 of the CIET three-branch educational simulator loop.

Version 3 keeps version 2's CTAH and TCHX PID control but solves the
branch mass flowrates on separate threads (`thread::spawn`) to run
faster than the serial version 2. It holds the version-3 driver
[`three_branch_ciet_ver3`] plus its steady-state, DHX-blocked and
reverse-diode test harnesses. Time is in seconds, temperatures in
degC, mass flow in kg/s and power in W.

```rust
pub mod version_3 { /* ... */ }
```

## Module `solver_functions`

CIET needs Thermo-hydraulic equations solved in TUAS

Writing them out explicitly in a procedural form is quite
cumbersome. It is much more concise to have these functions
here

Here there are functions for connecting the
heat transfer entities in:

- dracs loop
- pri loop DHX branch
- pri loop heater branch
- pri loop CTAH branch (forced circ)

Also I need to solve fluid flow in
- dracs loop
- pri loop, DHX, heater and CTAH branch

I would also need to be able to block flow in pri loop DHX
and CTAH branch as well, so as to isolate the loops.


```rust
pub mod solver_functions { /* ... */ }
```

### Functions

#### Function `pri_loop_three_branch_advance_timestep_except_dhx`

pri loop timestep advance for three loops
except dhx

```rust
pub fn pri_loop_three_branch_advance_timestep_except_dhx(timestep: Time, pipe_4: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_3: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_2a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_10_label_2: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_top_head_1a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_version1_1: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_bottom_head_1b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_18: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_26: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_25a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_21_label_25: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_20_label_23: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_23a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_22: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_20_21a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_21: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_20: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_19: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_41_label_6: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_6a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, ctah_vertical_label_7a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, ctah_horizontal_label_7b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_8a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_40_label_8: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_9: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_10: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_11: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_12: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, ctah_pump: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_13: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_14: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_40_14a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_15: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_16: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, top_mixing_node_5a_5b_4: &mut crate::prelude::beta_testing::HeatTransferEntity, bottom_mixing_node_17a_17b_18: &mut crate::prelude::beta_testing::HeatTransferEntity) { /* ... */ }
```

#### Function `ciet_pri_loop_three_branch_link_up_components`

heat transfer for pri loop, all three branch flowrates
required
now the heat transfer for the DRACS loop
for a single timestep, given mass flowrate in a counter clockwise
fashion in the DRACS

you also must specify the heat transfer coefficient to ambient
which is assumed to be the same throughout the loop

flow goes downwards by default through the DHX
to facilitate this, components are linked in a counter clockwise
fashion in the primary loop

todo: conduction between branches

```rust
pub fn ciet_pri_loop_three_branch_link_up_components(dhx_flow: MassRate, heater_flow: MassRate, ctah_flow: MassRate, heat_rate_through_heater: Power, average_temperature_for_density_calcs: ThermodynamicTemperature, ambient_htc: HeatTransfer, ctah_heat_transfer_coeff: HeatTransfer, pipe_4: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_3: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_2a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_10_label_2: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_top_head_1a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_version1_1: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_bottom_head_1b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_18: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_26: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_25a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_21_label_25: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_sthe: &mut crate::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger, static_mixer_20_label_23: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_23a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_22: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_20_21a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_21: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_20: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_19: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5b: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_41_label_6: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_6a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, ctah_vertical_label_7a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, ctah_horizontal_label_7b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_8a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_40_label_8: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_9: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_10: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_11: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_12: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, ctah_pump: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_13: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_14: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_40_14a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_15: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_16: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, top_mixing_node_5a_5b_4: &mut crate::prelude::beta_testing::HeatTransferEntity, bottom_mixing_node_17a_17b_18: &mut crate::prelude::beta_testing::HeatTransferEntity) { /* ... */ }
```

#### Function `get_abs_mass_flowrate_across_dracs_branches`

fluid mechanics bit for DRACS loop
calculate the fluid mechanics for the two branches in parallel
basically, mass flowrate

but its use is primarily for the DRACS branches in the DRACS
loop

```rust
pub fn get_abs_mass_flowrate_across_dracs_branches(dracs_branches: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_super_collection::FluidComponentSuperCollection) -> MassRate { /* ... */ }
```

#### Function `get_mass_flowrate_two_branches`

fluid mechanics bit for pri loop
calculate the fluid mechanics for the two branches in parallel
basically, mass flowrate


```rust
pub fn get_mass_flowrate_two_branches(dracs_branches: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_super_collection::FluidComponentSuperCollection) -> (MassRate, MassRate) { /* ... */ }
```

#### Function `get_mass_flowrate_vector_for_dhx_heater_and_ctah_branches`

fluid mechanics bit for primary loop
calculate fluid
calculate the fluid mechanics for the three branches in parallel
basically, mass flowrate

but its use is primarily for the DHX, Heater and CTAH branches


```rust
pub fn get_mass_flowrate_vector_for_dhx_heater_and_ctah_branches(pri_loop_branches: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component_super_collection::FluidComponentSuperCollection) -> (MassRate, MassRate, MassRate) { /* ... */ }
```

#### Function `three_branch_pri_loop_flowrates`

fluid mechanics calcs, specific to the primary loop
note that this only works if the components are correct
obtains mass flowrate across the primary loop
gets flowrate across dhx, heater and ctah branches, in that order
user must also specify a pump absolute pressure

(pressure drop) not using pump curves here yet


```rust
pub fn three_branch_pri_loop_flowrates(pump_pressure: Pressure, ctah_branch_blocked: bool, dhx_branch_blocked: bool, pipe_4: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_3: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_2a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_10_label_2: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_top_head_1a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_ver_1: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_bottom_head_1b: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_18: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_26: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_25a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_21_label_25: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_shell_side_pipe_24: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component::FluidComponent, static_mixer_20_label_23: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_23a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_22: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_20_21a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_21: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_20: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_19: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17b: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5b: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_41_label_6: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_6a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, ctah_vertical_label_7a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, ctah_horizontal_label_7b: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_8a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_40_label_8: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_9: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_10: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_11: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_12: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, ctah_pump: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_13: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_14: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_40_14a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_15: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_16: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) -> (MassRate, MassRate, MassRate) { /* ... */ }
```

#### Function `three_branch_pri_loop_flowrates_parallel`

fluid mechanics calcs, specific to the primary loop
note that this only works if the components are correct
obtains mass flowrate across the primary loop
gets flowrate across dhx, heater and ctah branches, in that order
user must also specify a pump absolute pressure

(pressure drop) not using pump curves here yet

now, for the diode version, I'll use a parallel thread to calculate
both the two branch and three branch version in parallel

So that when the three branch version solves, the two branch version
is also instantly available


```rust
pub fn three_branch_pri_loop_flowrates_parallel(pump_pressure: Pressure, ctah_branch_blocked: bool, dhx_branch_blocked: bool, pipe_4: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_3: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_2a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_10_label_2: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_top_head_1a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_ver_1: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, heater_bottom_head_1b: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_18: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_26: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_25a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_21_label_25: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_shell_side_pipe_24: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component::FluidComponent, static_mixer_20_label_23: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_23a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_22: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_20_21a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_21: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_20: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_19: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17b: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_5b: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_41_label_6: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_6a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, ctah_vertical_label_7a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, ctah_horizontal_label_7b: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_8a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_40_label_8: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_9: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_10: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_11: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_12: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, ctah_pump: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_13: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_14: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_40_14a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_15: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_16: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_17a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) -> (MassRate, MassRate, MassRate) { /* ... */ }
```

#### Function `coupled_dracs_fluid_mechanics_calc_abs_mass_rate_sam_tchx_calibration`

fluid mechanics calcs, specific to the DRACS loop
note that this only works if the components are correct
obtains mass flowrate across the DRACS loop
gets the absolute flowrate across the hot branch

```rust
pub fn coupled_dracs_fluid_mechanics_calc_abs_mass_rate_sam_tchx_calibration(pipe_34: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_heat_exchanger_30: &crate::array_control_vol_and_fluid_component_collections::fluid_component_collection::fluid_component::FluidComponent, dhx_tube_side_30a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_1: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_2: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) -> MassRate { /* ... */ }
```

#### Function `coupled_dracs_loop_link_up_components_sam_tchx_calibration`

now the heat transfer for the DRACS loop
for a single timestep, given mass flowrate in a counter clockwise
fashion in the DRACS

you also must specify the heat transfer coefficient to ambient
which is assumed to be the same throughout the loop


for DHX, the flow convention is going from top to bottom for both
shell and tube. The code is written such that components are linked
in a clockwise fashion, so that flow goes from top to bottom
in the tube side of the DHX.

the mass_flowrate_counter_clockwise you provide will be converted
into a mass_flowrate_clockwise and used for calculation

```rust
pub fn coupled_dracs_loop_link_up_components_sam_tchx_calibration(mass_flowrate_counter_clockwise: MassRate, tchx_heat_transfer_coeff: HeatTransfer, average_temperature_for_density_calcs: ThermodynamicTemperature, ambient_htc: HeatTransfer, pipe_34: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_sthe: &mut crate::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger, dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_1: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_2: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) { /* ... */ }
```

#### Function `dracs_loop_advance_timestep_except_dhx_sam_tchx_calibration`

now the heat transfer for the DRACS loop
for a single timestep, given mass flowrate in a counter clockwise
fashion in the DRACS

you also must specify the heat transfer coefficient to ambient
which is assumed to be the same throughout the loop

```rust
pub fn dracs_loop_advance_timestep_except_dhx_sam_tchx_calibration(timestep: Time, pipe_34: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_33: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_32: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_31a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, static_mixer_61_label_31: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_1: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, tchx_35b_2: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, static_mixer_60_label_36: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_36a: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_37: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, flowmeter_60_37a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, pipe_38: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent, pipe_39: &mut crate::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent) { /* ... */ }
```

#### Function `dracs_loop_dhx_tube_temperature_diagnostics`

these are temperature diagnostic
functions to check bulk and wall temperature before
and after the DHX tube side

before dhx tube: BT-60, WT-61 (not exactly sure where)
use pipe_30a
after dhx tube: BT-23, WT-22
use pipe_30b


```rust
pub fn dracs_loop_dhx_tube_temperature_diagnostics(dhx_tube_side_30a: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, dhx_tube_side_30b: &mut crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent, print_debug_results: bool) -> ((ThermodynamicTemperature, ThermodynamicTemperature), (ThermodynamicTemperature, ThermodynamicTemperature)) { /* ... */ }
```

## Module `components`

adds extra components specific to the three branch
simulation,
the other components were borrowed from the isothermal
test and steady state natural circulation modules

```rust
pub mod components { /* ... */ }
```

### Functions

#### Function `new_active_ctah_vertical`

creates a new ctah vertical for CIET using the RELAP5-3D and SAM parameters
in Compact Integral Effects Test (CIET)

this is inactive, so it behaves more like a pipe rather than a
heat exchanger

Vertical part of Coiled Tube Air Heater (CTAH)
label component 7a
in Compact Integral Effects Test (CIET)
CTAH branch

It is NOT insulated by the way


Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.

You'll need to supply your own heat transfer coefficient


```rust
pub fn new_active_ctah_vertical(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

#### Function `new_active_ctah_horizontal`

creates a new ctah vertical for CIET using the RELAP5-3D and SAM parameters
in Compact Integral Effects Test (CIET)

this is inactive, so it behaves more like a pipe rather than a
heat exchanger

Horizontal part of Coiled Tube Air Heater (CTAH)
label component 7b
in Compact Integral Effects Test (CIET)
CTAH branch
coiled tube air heater
has fldk = 400 + 52,000/Re

label is 7b
empirical data in page 48 on pdf viewer in Dr
Zweibaum thesis shows reverse flow has same
pressure drop characteristics as forward flow

It is NOT insulated by the way

Zou, Ling, Rui Hu, and Anne Charpentier. SAM code
validation using the compact integral effects test (CIET)
experimental data. No. ANL/NSE-19/11. Argonne National Lab.(ANL),


Zweibaum, Nicolas. Experimental validation of passive safety
system models: Application to design and optimization of
fluoride-salt-cooled, high-temperature reactors. University of
California, Berkeley, 2015.
Argonne, IL (United States), 2019.


```rust
pub fn new_active_ctah_horizontal(initial_temperature: ThermodynamicTemperature) -> crate::pre_built_components::non_insulated_fluid_components::NonInsulatedFluidComponent { /* ... */ }
```

