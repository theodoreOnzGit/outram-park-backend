# Crate Documentation

**Version:** 0.1.4

**Format Version:** 61

# Module `teh_o_prke`

## Modules

## Module `zero_power_prke`

**Attributes:**

- `Other("#[warn(missing_docs)]")`

contains structs for the zero power point reactor kinetics equations

the SixGroupPRKE struct contains the code which performs solution of
the PRKE matrix with six precursor groups

but you must supply reactivity (or keff equivalently) as an input.

for real-time calculations, only thermal reactors are okay
because the neutron generation time is on the order of 10E-4 s
for fast reactors, neutron generation time is on the order of 10E-8
but home computers calculate on the order of 1E-5s per calculation or
1E-6s at best

Will probably need some other kind of method to calculate feedback


```rust
pub mod zero_power_prke { /* ... */ }
```

### Modules

## Module `six_group_precursor_prke`

six group PRKE struct

uses implicit calculation

```rust
pub mod six_group_precursor_prke { /* ... */ }
```

### Modules

## Module `six_group_constants`

contains six group delayed precursor decay constants and
delayed fraction

```rust
pub mod six_group_constants { /* ... */ }
```

### Types

#### Enum `FissioningNuclideType`

different nuclides or fuels have different delayed groups

```rust
pub enum FissioningNuclideType {
    U233,
    U235,
    Pu239,
}
```

##### Variants

###### `U233`

chooses the U233 group of delayed constants

###### `U235`

chooses the U235 group of delayed constants

###### `Pu239`

chooses the Pu239 group of delayed constants

##### Implementations

###### Methods

- ```rust
  pub fn get_decay_constant_array(self: &Self) -> [DecayConstant; 6] { /* ... */ }
  ```
  returns a new decay constant array based on nuclide

- ```rust
  pub fn get_delayed_fraction_array(self: &Self) -> [Ratio; 6] { /* ... */ }
  ```
  returns a delayed fraction array based on nuclide

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FissioningNuclideType { /* ... */ }
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
### Functions

#### Function `new_decay_constant_array`

produces a new decay constant array

```rust
pub fn new_decay_constant_array() -> [crate::zero_power_prke::six_group_precursor_prke::DecayConstant; 6] { /* ... */ }
```

#### Function `new_u233_delayed_neutron_fraction_array`

produces a new delayed fraction for u233

```rust
pub fn new_u233_delayed_neutron_fraction_array() -> [Ratio; 6] { /* ... */ }
```

#### Function `new_u235_delayed_neutron_fraction_array`

produces a new delayed fraction for u235

```rust
pub fn new_u235_delayed_neutron_fraction_array() -> [Ratio; 6] { /* ... */ }
```

#### Function `new_pu239_delayed_neutron_fraction_array`

produces a new delayed fraction for pu239

```rust
pub fn new_pu239_delayed_neutron_fraction_array() -> [Ratio; 6] { /* ... */ }
```

## Module `implicit_solver`

contains time stepping implicit solvers for SixGroupPRKE

```rust
pub mod implicit_solver { /* ... */ }
```

## Module `explicit_solver`

contains time stepping explicit solvers for SixGroupPRKE

```rust
pub mod explicit_solver { /* ... */ }
```

### Types

#### Type Alias `DecayConstant`

Decay Constant is essentially the same units as frequency

```rust
pub type DecayConstant = Frequency;
```

#### Struct `SixGroupPRKE`

SixGroupPRKE

```rust
pub struct SixGroupPRKE {
    pub decay_constant_array: [DecayConstant; 6],
    pub delayed_fraction_array: [Ratio; 6],
    pub delayed_group_mode: six_group_constants::FissioningNuclideType,
    pub precursor_and_neutron_pop_and_source_array: [VolumetricNumberDensity; 7],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `decay_constant_array` | `[DecayConstant; 6]` | contains an array for the various half lives<br>of the delayed precursors |
| `delayed_fraction_array` | `[Ratio; 6]` | contains delayed fraction arrays for the delayed precursors<br>this is different for u235, u233 and Pu239 |
| `delayed_group_mode` | `six_group_constants::FissioningNuclideType` | determines the set of delayed group constants based on your choice<br>of fissile isotope |
| `precursor_and_neutron_pop_and_source_array` | `[VolumetricNumberDensity; 7]` | precursor_and_neutron_pop_and_source_array |

##### Implementations

###### Methods

- ```rust
  pub fn solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(self: &mut Self, timestep: Time, reactivity: Ratio, neutron_generation_time: Time, background_source_rate: VolumetricNumberRate) -> Result<Array1<VolumetricNumberDensity>, TehOPrkeError> { /* ... */ }
  ```
  returns the next timestep neutron source vector

- ```rust
  pub fn construct_present_timestep_concentration_and_neutron_pop_vector(delayed_neutron_precursor_group_1_concentration: VolumetricNumberDensity, delayed_neutron_precursor_group_2_concentration: VolumetricNumberDensity, delayed_neutron_precursor_group_3_concentration: VolumetricNumberDensity, delayed_neutron_precursor_group_4_concentration: VolumetricNumberDensity, delayed_neutron_precursor_group_5_concentration: VolumetricNumberDensity, delayed_neutron_precursor_group_6_concentration: VolumetricNumberDensity, neutron_population_number_density: VolumetricNumberDensity, background_source_rate: VolumetricNumberRate, timestep: Time) -> Array1<VolumetricNumberDensity> { /* ... */ }
  ```
  constructs the vector for delayed neutron precursor concentration

- ```rust
  pub fn construct_coefficient_matrix(self: &Self, timestep: Time, reactivity: Ratio, neutron_generation_time: Time) -> Array2<Ratio> { /* ... */ }
  ```
  constructs the matrix required for

- ```rust
  pub fn solve_next_timestep_precursor_concentration_and_neutron_pop_vector_explicit(self: &mut Self, timestep: Time, reactivity: Ratio, neutron_generation_time: Time, background_source_rate: VolumetricNumberRate) -> Result<Array1<VolumetricNumberDensity>, TehOPrkeError> { /* ... */ }
  ```
  solves for the neutron population and precursor concentration

- ```rust
  pub fn get_current_neutron_population_density(self: &Self) -> VolumetricNumberDensity { /* ... */ }
  ```
  obtains current neutron population

- ```rust
  pub fn get_total_delayed_fraction(self: &Self) -> Ratio { /* ... */ }
  ```
  total delayed fraction

- ```rust
  pub fn get_keff_from_reactivity(reactivity: Ratio) -> Ratio { /* ... */ }
  ```
  enables you to convert reactivity into keff, useful for calculating

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SixGroupPRKE { /* ... */ }
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
## Module `matrix`

Pure-Rust dense LU solver (`SquareMatrix`), inlined from `outram-foam-basic-lib`
so this crate has no `outram-foam-basic-lib` dependency — see the module doc.
Pure-Rust dense LU solver — an **inlined copy** of
`outram-foam-basic-lib`'s `matrix::SquareMatrix`.

This is a *deliberate duplication*: `teh-o-prke` needs only this small
self-contained LU solver from `outram-foam-basic-lib`, so the code is copied
here to remove the `outram-foam-basic-lib` path dependency entirely. That keeps
the inter-crate dependency graph acyclic when a future full `tampines`
(multiphase-flow) crate and `nee_soon` compose `teh-o-prke` and
`tuas_boussinesq_solver` together — decoupling is preferred over DRY here.
`tuas_boussinesq_solver` carries an identical inlined copy for the same
reason. If the LU algorithm is ever changed, update all copies.

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

- **RefUnwindSafe**
- **Same**
- **Send**
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
## Module `fuel_temperature_feedback`

contains functions and structs for fuel temperature feedback

this is the simplest feedback mechanism
where rudimentary thermal hydraulics model is added.

```rust
pub mod fuel_temperature_feedback { /* ... */ }
```

### Types

#### Struct `SimpleFuelTemperatureFeedback`

a struct for calculating fuel temperature feedback
using a rather simple heat balance equations

m c_p (dT_fuel/dt) = -hA(T_fuel-T_surr) + fission_power_source

uses explicit time stepping for simplicity


```rust
pub struct SimpleFuelTemperatureFeedback {
    pub fuel_specific_heat_capacity: SpecificHeatCapacity,
    pub fuel_density: MassDensity,
    pub fuel_volume: Volume,
    pub fuel_temperature: ThermodynamicTemperature,
    pub convection_heat_trf_coeff: HeatTransfer,
    pub convection_heat_trf_area: Area,
    pub alpha_coefficient: Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_specific_heat_capacity` | `SpecificHeatCapacity` | rho V c_p = m c_p<br><br>let's do c_p |
| `fuel_density` | `MassDensity` | rho |
| `fuel_volume` | `Volume` | volume |
| `fuel_temperature` | `ThermodynamicTemperature` | T_fuel |
| `convection_heat_trf_coeff` | `HeatTransfer` | convection heat transfer coefficient |
| `convection_heat_trf_area` | `Area` | convection heat transfer area |
| `alpha_coefficient` | `Ratio` | fuel temperature feedback coefficient<br>can be expressed as alpha = -alpha_coefficient/sqrt(T(kelvin))<br><br>typically around 10^(-4) dimensionless |

##### Implementations

###### Methods

- ```rust
  pub fn set_fuel_temperature(self: &mut Self, temperature: ThermodynamicTemperature) -> Result<(), TehOPrkeError> { /* ... */ }
  ```
  set initial fuel temperature

- ```rust
  pub fn get_fuel_temperature(self: &Self) -> Result<ThermodynamicTemperature, TehOPrkeError> { /* ... */ }
  ```
  get current fuel temperature

- ```rust
  pub fn set_fuel_alpha_coefficient(self: &mut Self, alpha_coefficient: Ratio) -> Result<(), TehOPrkeError> { /* ... */ }
  ```
  set fuel alpha_coefficient

- ```rust
  pub fn add_fission_heat(self: &mut Self, fission_power: Power, timestep: Time) -> Result<(), TehOPrkeError> { /* ... */ }
  ```
  add fission heat

- ```rust
  pub fn remove_convection_heat(self: &mut Self, coolant_temperature: ThermodynamicTemperature, timestep: Time) -> Result<(), TehOPrkeError> { /* ... */ }
  ```
  remove heat due to convection

- ```rust
  pub fn obtain_fuel_temperature_delta_rho(self: &Self, reference_temperature: ThermodynamicTemperature) -> Result<Ratio, TehOPrkeError> { /* ... */ }
  ```
  obtain reactivity change compared to reference temperature

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
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

#### Function `obtain_fuel_temperature_feedback_coeff_thermal_spectrum`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

for thermal spectrum reactors,
the alpha  = d(rho)/dT

can be expressed as alpha = -alpha_coefficient/sqrt(T(kelvin))
(see lamarsh)

alpha_coefficient is some value, usually on the order of 1*10^(-4)


```rust
pub fn obtain_fuel_temperature_feedback_coeff_thermal_spectrum(alpha_coefficient: Ratio, temperature: ThermodynamicTemperature) -> Result<Ratio, crate::teh_o_prke_error::TehOPrkeError> { /* ... */ }
```

#### Function `obtain_fuel_temperature_reactivity_feedback_thermal_spectrum`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

for thermal spectrum reactors,
we can calculate reactivity based on alpha

can be expressed as d(rho)/dT = -alpha_coefficient/sqrt(T(kelvin))

alpha_coefficient is some value, usually on the order of 1*10^(-4)

now, if you want to have a reactivity with respect to some temperature,
we can analytically integrate:

rho - rho_ref = -alpha_coefficient * 2.0 (sqrt(T) - sqrt(T_ref))

we can define rho_ref as reactivity at some temperature T_ref

The function will return (rho - rho_ref), or delta_rho



```rust
pub fn obtain_fuel_temperature_reactivity_feedback_thermal_spectrum(alpha_coefficient: Ratio, temperature: ThermodynamicTemperature, reference_temperature: ThermodynamicTemperature) -> Result<Ratio, crate::teh_o_prke_error::TehOPrkeError> { /* ... */ }
```

## Module `control_rod_feedback`

contains functions and structs for control rod feedback

```rust
pub mod control_rod_feedback { /* ... */ }
```

### Functions

#### Function `obtain_rod_worth_cylinder`

based on Lamarsh's formula, obtain a rod worth for a cylinder
of height H, and an insertion length of x


rho (x) = rho (H) * [x/H - 1/ (2 pi) sin (2 pi x/H)]

of course x is necessarily less than or equal H

```rust
pub fn obtain_rod_worth_cylinder(cylinder_height: Length, insertion_length: Length, rod_worth: Ratio) -> Result<Ratio, crate::teh_o_prke_error::TehOPrkeError> { /* ... */ }
```

## Module `teh_o_prke_error`

error type for the crate

```rust
pub mod teh_o_prke_error { /* ... */ }
```

### Types

#### Enum `TehOPrkeError`

Master Error type of this crate

```rust
pub enum TehOPrkeError {
    ShapeMismatch(String),
    GenericStringError(String),
    NonNegativeFuelFeedbackCoefficient(f64),
    NonPositivePromptNeutronGenerationTime(f64),
    NonPositiveFuelHeatCapacity(f64),
    NonPositiveDelayedDecayConstant(f64),
}
```

##### Variants

###### `ShapeMismatch`

matrix solve error (e.g. singular coefficient matrix)

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `GenericStringError`

it's a generic error which is a placeholder since I used
so many string errors

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NonNegativeFuelFeedbackCoefficient`

[`crate::nordheim_fuchs`]'s exact timestepper requires a strictly
negative fuel feedback coefficient (alpha_f < 0, self-limiting
negative feedback) for its closed-form solution to stay
real-valued; a non-negative value describes a non-self-limiting
excursion this model does not support.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `NonPositivePromptNeutronGenerationTime`

[`crate::nordheim_fuchs`]'s prompt neutron generation time Lambda
must be strictly positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `NonPositiveFuelHeatCapacity`

[`crate::nordheim_fuchs`]'s lumped fuel heat capacity C_f must be
strictly positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `NonPositiveDelayedDecayConstant`

[`crate::delayed_neutron_layer`]'s per-group decay constant
`lambda_i` must be strictly positive (each precursor group is a
stable first-order lag of time constant `tau_i = 1/lambda_i`); a
non-positive `lambda_i` has no finite time constant.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

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
## Module `feedback_mechanisms`

contains code for various feedback mechanisms
this uses the six factor formula rather than simply adjusting reactivity

```rust
pub mod feedback_mechanisms { /* ... */ }
```

### Modules

## Module `fission_product_poisons`

fission product poisoning
includes but not limited to xenon-iodine 135 poisoning


```rust
pub mod fission_product_poisons { /* ... */ }
```

### Types

#### Struct `Xenon135Poisoning`

```rust
pub struct Xenon135Poisoning {
    pub iodine_135_num_density: VolumetricNumberDensity,
    pub xenon_135_num_density: VolumetricNumberDensity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `iodine_135_num_density` | `VolumetricNumberDensity` |  |
| `xenon_135_num_density` | `VolumetricNumberDensity` |  |

##### Implementations

###### Methods

- ```rust
  pub fn fp_yield_iodine_135_from_u235_thermal_fission() -> Ratio { /* ... */ }
  ```
  table 7.5

- ```rust
  pub fn fp_yield_xe_135_from_u235_thermal_fission() -> Ratio { /* ... */ }
  ```
  returns number of atoms per thermal fission of a nuclide

- ```rust
  pub fn fp_yield_iodine_135_from_u233_thermal_fission() -> Ratio { /* ... */ }
  ```
  table 7.5

- ```rust
  pub fn fp_yield_xe_135_from_u233_thermal_fission() -> Ratio { /* ... */ }
  ```
  table 7.5

- ```rust
  pub fn fp_yield_iodine_135_from_pu239_thermal_fission() -> Ratio { /* ... */ }
  ```
  table 7.5

- ```rust
  pub fn fp_yield_xe_135_from_pu239_thermal_fission() -> Ratio { /* ... */ }
  ```
  table 7.5

- ```rust
  pub fn iodine_135_decay_const() -> Frequency { /* ... */ }
  ```
  table 7.6

- ```rust
  pub fn xe_135_decay_const() -> Frequency { /* ... */ }
  ```
  table 7.6

- ```rust
  pub fn xe135_thermal_abs_xs() -> Area { /* ... */ }
  ```
  tentatively got from AI, but need to cite...

- ```rust
  pub fn calc_xe_135_and_return_num_density(self: &mut Self, timestep: Time, fission_rate: VolumetricNumberRate, fissioning_nuclide: FissioningNuclideType, thermal_neutron_conc: VolumetricNumberDensity) -> VolumetricNumberDensity { /* ... */ }
  ```
  (dX/dt) = gamma_X * fission rate + lambda_I * I -  lambda_X * X - sigma_aX * X *

- ```rust
  pub fn simplified_poison_concentration_feedback(poison_conc: MassConcentration) -> Ratio { /* ... */ }
  ```
  calculates a feedback based on poison concentration

- ```rust
  pub fn get_current_xe135_conc(self: &Self) -> MassConcentration { /* ... */ }
  ```

- ```rust
  pub fn gaseous_xe135_density_estimate() -> MassDensity { /* ... */ }
  ```
  gives xenon density estimate

- ```rust
  pub fn gaseous_xe135_molar_mass() -> MolarMass { /* ... */ }
  ```
  gives xenon135 molar weight

- ```rust
  pub fn u235_thermal_abs_xs() -> Area { /* ... */ }
  ```
  gives thermal absorption cross section est for u235

- ```rust
  pub fn u238_thermal_abs_xs() -> Area { /* ... */ }
  ```
  gives thermal absorption cross section est for u238

- ```rust
  pub fn uranium_number_density_est_in_uo2() -> VolumetricNumberDensity { /* ... */ }
  ```
  number density estimate for uranium in uo2

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Xenon135Poisoning { /* ... */ }
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
    returns a fresh core

- **Freeze**
- **From**
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
### Types

#### Struct `SixFactorFormulaFeedback`

six factor formula to calculate keff and
reactivity

keff = P_TNL * P_FNL * eta * f * p * epsilon

epsilon = fast fission factor
p = resonance escape probability
f  = thermal utilisation factor
eta = fuel reproduction factor

P_TNL = probability of thermal non leakage
P_FNL = probability of fast non leakage

```rust
pub struct SixFactorFormulaFeedback {
    pub p_tnl: Ratio,
    pub p_fnl: Ratio,
    pub epsilon: Ratio,
    pub p: Ratio,
    pub f: Ratio,
    pub eta: Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `p_tnl` | `Ratio` | thermal non leakage probability |
| `p_fnl` | `Ratio` | fast non leakage probability |
| `epsilon` | `Ratio` | fast fission factor |
| `p` | `Ratio` | resonance escape probability |
| `f` | `Ratio` | thermal utilisation factor |
| `eta` | `Ratio` | fuel reproduction factor |

##### Implementations

###### Methods

- ```rust
  pub fn calc_keff(self: &Self) -> Ratio { /* ... */ }
  ```
  calculates the k_eff given the

- ```rust
  pub fn calc_rho(self: &Self) -> Ratio { /* ... */ }
  ```
  calculates reactivity given the six factor

- ```rust
  pub fn fuel_temp_feedback(self: &mut Self, t: ThermodynamicTemperature, resonance_esc_feedback: fn(ThermodynamicTemperature) -> Ratio) { /* ... */ }
  ```
  fuel temperature feedback, should impact

- ```rust
  pub fn moderator_density_feedback(self: &mut Self, rho: MassDensity, mod_void_feedback: fn(MassDensity) -> Ratio, resonance_esc_feedback: fn(MassDensity) -> Ratio, thermal_non_leakage_feedback: fn(MassDensity) -> Ratio, fast_non_leakage_feedback: fn(MassDensity) -> Ratio) { /* ... */ }
  ```
  void (average density) feedback

- ```rust
  pub fn reflector_density_feedback(self: &mut Self, rho: MassDensity, mod_void_feedback: fn(MassDensity) -> Ratio, resonance_esc_feedback: fn(MassDensity) -> Ratio, thermal_non_leakage_feedback: fn(MassDensity) -> Ratio, fast_non_leakage_feedback: fn(MassDensity) -> Ratio) { /* ... */ }
  ```
  void (average density) feedback for reflector

- ```rust
  pub fn control_rod_feedback(self: &mut Self, rod_insertion_ratio: Ratio, ctrl_rod_feedback: fn(Ratio) -> Ratio) { /* ... */ }
  ```
  control rod feedback

- ```rust
  pub fn leakage_feedback(self: &mut Self, rho: MassDensity, thermal_non_leakage_feedback: fn(MassDensity) -> Ratio, fast_non_leakage_feedback: fn(MassDensity) -> Ratio) { /* ... */ }
  ```
  generic leakage feedback

- ```rust
  pub fn reactor_poison_feedback(self: &mut Self, reactor_poison_concentration: MassConcentration, reactor_poison_conc_feedback: fn(MassConcentration) -> Ratio) { /* ... */ }
  ```
  reactor poison feedback

- ```rust
  pub fn burnable_absorber_posion_feedback(self: &mut Self, burnable_poison_concentration: MassConcentration, poison_conc_feedback: fn(MassConcentration) -> Ratio) { /* ... */ }
  ```
  burnable absorber/poison feedback

- ```rust
  pub fn fuel_depletion_and_breeding_feedback(self: &mut Self, fuel_concentration: MassConcentration, eta_feedback: fn(MassConcentration) -> Ratio, fast_fission_factor_feedback: fn(MassConcentration) -> Ratio, resonance_esc_feedback: fn(MassConcentration) -> Ratio, thermal_utilisation_feedback: fn(MassConcentration) -> Ratio) { /* ... */ }
  ```
  fuel depletion and fuel breeding

- ```rust
  pub fn fuel_burnup_feedback(self: &mut Self, burnup: AvailableEnergy, eta_feedback: fn(AvailableEnergy) -> Ratio, fast_fission_factor_feedback: fn(AvailableEnergy) -> Ratio, resonance_esc_feedback: fn(AvailableEnergy) -> Ratio, thermal_utilisation_feedback: fn(AvailableEnergy) -> Ratio) { /* ... */ }
  ```
  burnup feedback

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SixFactorFormulaFeedback { /* ... */ }
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
## Module `decay_heat`

contains code for decay heat simulation
the user can have up to seven groups

# Fission-product decay heat (23-group, 1978 draft ANS Standard)

Decay heat is the power released by the radioactive decay of fission
products after fission itself has stopped. It is what makes a reactor
impossible to simply switch off, and it is the entire subject of a passive
decay-heat-removal safety case.

## What belongs in this module

The group-fit decay-heat model and its tabulated parameters, plus the
time-integration of those groups against a fission-power history. What does
**not** belong here: neutron kinetics (see [`crate::zero_power_prke`] and
friends), heat transfer, or any reactor-specific geometry.

## Data source

Parameters are Table 16 of

> Tobias, A., "Decay heat", *Progress in Nuclear Energy*, table titled
> "Parameters for fission product decay heat functions of 1978 draft ANS
> Standard (England *et al.*, 1978)", p. 78.

Tobias reproduces the fit of England *et al.* (1978) that forms the basis of
the 1978 proposed ANS Standard. Tobias records that the burst function is
reproduced by this 23-exponential sum "to within a few tenths of a percent,
for cooling times of up to 1e9 sec".

Three fissioning nuclides are tabulated — see [`FissioningNuclide`]. The
numerical parameters are physical measurements and are used here as facts;
the source publication itself is catalogued separately and is not
redistributed.

## Model

For a single fissile nuclide the decay-heat *burst* function, the power
released at time `t` seconds after a single fission, is Tobias eq. (32):

```text
m(t) = sum_{i=1..23} alpha_i * exp(-lambda_i * t)      [MeV / (fission . s)]
```

and the integral decay heat after an irradiation of `I` seconds at constant
fission rate is eq. (33):

```text
M(I,t) = sum_{i=1..23} (alpha_i / lambda_i) * exp(-lambda_i * t)
                       * (1 - exp(-lambda_i * I))      [MeV / fission]
```

Tobias notes that an infinite irradiation is represented in eq. (33) by
`I = 1e13 s`.

This module integrates the equivalent per-group differential form, which is
what a transient simulation needs because the fission power is not constant:

```text
dH_i/dt = alpha_i * F(t) - lambda_i * H_i        H_i has units of power
P_decay(t) = sum_i H_i(t)
```

where `F(t)` is the fission rate. A single fission at `t = 0` sets
`H_i = alpha_i` and then decays as `alpha_i * exp(-lambda_i t)`, recovering
eq. (32) exactly; holding `F` constant for `I` seconds and then decaying
recovers eq. (33) exactly. The two published forms are therefore special
cases of what is integrated here, which is the property the unit tests check.

## Why the update is analytic and not an explicit Euler step

**This matters, and it is why the previous placeholder could not have
worked.** The decay constants span
`lambda = 2.2138e+01` down to `1.5699e-14` per second — **fifteen orders of
magnitude**. The fastest group has a time constant of about 45 ms. An
explicit update would need a timestep below that for stability, so at the
0.1-1 s timesteps these simulators actually run, the fast groups would blow
up or oscillate.

[`DecayHeat::advance_timestep`] therefore integrates each group
**analytically** over the step, treating the fission power as constant
across it:

```text
H_i(t+dt) = H_i(t) * exp(-lambda_i * dt)
          + (alpha_i * F / lambda_i) * (1 - exp(-lambda_i * dt))
```

This is exact for a piecewise-constant fission power, unconditionally
stable at any timestep, and costs one `exp` per group per step.

## Status

**AI-assisted implementation, not yet human-reviewed** — see
`RESPONSIBLE_USE.md` and `VERIFICATION_AND_VALIDATION.md`. The unit tests in
this file verify the implementation against the source's own published
equations and against the total decay energy per fission; they do **not**
validate the model against a measured decay-heat transient.

```rust
pub mod decay_heat { /* ... */ }
```

### Types

#### Enum `FissioningNuclide`

The fissioning nuclide whose decay-heat parameters are in use.

The three cases tabulated by the 1978 draft ANS Standard. Real fuel is a
mixture, and Tobias eq. (34) sums over nuclides weighted by their fractional
fission rates; this enum selects one at a time, so a mixture must be handled
by summing several [`DecayHeat`] instances (see
[`DecayHeat::total_decay_heat_power`] and the module tests).

An enum rather than a trait object, per the workspace design rules: the set
of tabulated nuclides is closed and known at compile time, so adding one is
a compile error at every match site.

```rust
pub enum FissioningNuclide {
    U235Thermal,
    U238Fast,
    Pu239Thermal,
}
```

##### Variants

###### `U235Thermal`

Thermal fission of U-235. Derived from decay-heat functions in
England *et al.* (1978). Total decay energy 13.18 MeV/fission.

###### `U238Fast`

Fast fission of U-238. Tobias notes that for U-238 **only summation
results were used**, rather than the fitted-to-measurement route used
for the other two, so this column carries a different pedigree.
Total decay energy 16.24 MeV/fission.

###### `Pu239Thermal`

Thermal fission of Pu-239. Total decay energy 10.93 MeV/fission.

##### Implementations

###### Methods

- ```rust
  pub fn parameters(self: &Self) -> &'static [(f64, f64); 23] { /* ... */ }
  ```
  The 23 `(alpha_i, lambda_i)` pairs for this nuclide.

- ```rust
  pub fn total_decay_energy_per_fission(self: &Self) -> Energy { /* ... */ }
  ```
  Total decay energy released per fission over infinite time following a

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FissioningNuclide { /* ... */ }
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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FissioningNuclide) -> bool { /* ... */ }
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
#### Struct `DecayHeat`

Fission-product decay-heat state: one stored power per exponential group.

Construct with [`DecayHeat::new`] (all groups cold, as at first startup of
fresh fuel) or [`DecayHeat::new_at_equilibrium`] (groups saturated to an
infinite prior irradiation, which is the realistic starting point for a
shutdown transient). Advance with [`DecayHeat::advance_timestep`] and read
with [`DecayHeat::total_decay_heat_power`].

Owns its data by value; no lifetimes, no heap allocation.

```rust
pub struct DecayHeat {
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
  pub fn new(nuclide: FissioningNuclide) -> Self { /* ... */ }
  ```
  A cold decay-heat state for `nuclide`: no fission products present, so

- ```rust
  pub fn new_at_equilibrium(nuclide: FissioningNuclide, fission_power: Power) -> Self { /* ... */ }
  ```
  A decay-heat state saturated to an infinite prior irradiation at

- ```rust
  pub fn with_energy_per_fission(self: Self, energy_per_fission: Energy) -> Self { /* ... */ }
  ```
  Override the recoverable energy per fission (default

- ```rust
  pub fn nuclide(self: &Self) -> FissioningNuclide { /* ... */ }
  ```
  The nuclide whose parameters this state uses.

- ```rust
  pub fn advance_timestep(self: &mut Self, fission_power: Power, timestep: Time) { /* ... */ }
  ```
  Advance every decay-heat group by `timestep`, holding `fission_power`

- ```rust
  pub fn total_decay_heat_power(self: &Self) -> Power { /* ... */ }
  ```
  Total fission-product decay-heat power, the sum over all 23 groups.

- ```rust
  pub fn prompt_power_fraction(self: &Self) -> Ratio { /* ... */ }
  ```
  The fraction of fission power released *promptly*, i.e. everything

- ```rust
  pub fn group_power(self: &Self, group_index: usize) -> Option<Power> { /* ... */ }
  ```
  The stored power of a single group, for inspection and testing.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DecayHeat { /* ... */ }
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
    Thermal U-235, all groups cold.

- **Freeze**
- **From**
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
### Constants and Statics

#### Constant `DECAY_HEAT_GROUPS`

Number of exponential groups in the 1978 draft ANS Standard fit.

Fixed at 23 by the published fit (Tobias Table 16); it is not a tuning
parameter.

```rust
pub const DECAY_HEAT_GROUPS: usize = 23;
```

#### Constant `NOMINAL_ENERGY_PER_FISSION_MEV`

Recoverable energy released per fission, used to convert a fission **power**
into a fission **rate** so the group parameters (which are per fission) can
be applied.

200 MeV is the conventional round figure for thermal fission of U-235; the
true value depends on nuclide and on how much escaping neutrino energy is
excluded. Treating it as exactly 200 MeV introduces a systematic error of a
couple of percent in the absolute decay-heat level. Callers who need better
should use [`DecayHeat::with_energy_per_fission`].

```rust
pub const NOMINAL_ENERGY_PER_FISSION_MEV: f64 = 200.0;
```

## Module `time_stepping`

contains code for time stepping for prke
some algorithms copied from OpenFOAM


```rust
pub mod time_stepping { /* ... */ }
```

### Modules

## Module `openfoam_rfk45`

**Attributes:**

- `Other("#[allow(non_upper_case_globals)]")`

```rust
pub mod openfoam_rfk45 { /* ... */ }
```

### Types

#### Struct `RKF45`

**Attributes:**

- `Other("#[allow(non_snake_case)]")`

note: need a verification test too

```rust
pub struct RKF45 {
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
  pub fn solve(self: &mut Self, x0: f64, y0: Vec<f64>, dydx0: Vec<f64>, dx: f64, y: &mut Vec<f64>) { /* ... */ }
  ```

- ```rust
  pub fn solve_functional_prog_single_stepsize_no_stepsize_adjust</* synthetic */ impl Fn(f64, &Vec<f64>) -> Vec<f64>: Fn(f64, &Vec<f64>) -> Vec<f64>>(x0: f64, y0: Vec<f64>, dx: f64, user_defined_ode: impl Fn(f64, &Vec<f64>) -> Vec<f64>) -> Vec<f64> { /* ... */ }
  ```
  solves using a more functional programming

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RKF45 { /* ... */ }
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
## Module `openfoam_ode_system`

```rust
pub mod openfoam_ode_system { /* ... */ }
```

### Types

#### Struct `ODESystem`

**Attributes:**

- `Other("#[allow(non_snake_case)]")`

rust translation of the OpenFOAM ODE system

note that this is nested inside ODESolver struct

```rust
pub struct ODESystem {
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
  pub fn new(ode_system: fn(f64, &Vec<f64>) -> Vec<f64>) -> Self { /* ... */ }
  ```
  constructor for ODE system

- ```rust
  pub fn derivatives(self: &Self, x: f64, y: &Vec<f64>, dydx: &mut Vec<f64>) { /* ... */ }
  ```
  this evaluates a vector dydx based on a vector y and

- ```rust
  pub fn derivatives_with_fn</* synthetic */ impl Fn(f64, &Vec<f64>) -> Vec<f64>: Fn(f64, &Vec<f64>) -> Vec<f64>>(ode_system: impl Fn(f64, &Vec<f64>) -> Vec<f64>, x: f64, y: &Vec<f64>) -> Vec<f64> { /* ... */ }
  ```
  this evaluates a vector dydx based on a vector y and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ODESystem { /* ... */ }
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
## Module `openfoam_ode_solver`

```rust
pub mod openfoam_ode_solver { /* ... */ }
```

## Module `nordheim_fuchs`

analytical (closed-form) Nordheim-Fuchs exact timestepper for prompt
reactivity excursions with adiabatic fuel-temperature feedback -- a
real-time-friendly "Prompt Excursion Layer", distinct from (and much
cheaper than) the six-group precursor PRKE in [`zero_power_prke`].
Nordheim-Fuchs exact timestepper: an analytical, closed-form integrator
for prompt reactivity excursions with adiabatic fuel-temperature
feedback. See [`NordheimFuchsExactTimestepper`] for the physical model,
preconditions, and derivation.

This is deliberately **not** a general point-reactor-kinetics solver --
delayed-neutron precursor dynamics are neglected entirely (that remains
[`crate::zero_power_prke`]'s job). Its purpose is a real-time-friendly,
non-stiff educational/visualization model of the prompt self-limiting
mechanism (negative fuel-temperature feedback shutting down a prompt
excursion), for interactive reactor demos -- including on low-power
devices such as Android tablets -- where a standard ODE stepper's
`dt << Lambda` stability restriction (Lambda can be ~1e-8 s for
fast-spectrum systems) is prohibitively expensive for real-time frame
rates. It is **not** intended for licensing, reactor-protection, or
safety-grade transient analysis -- see
[`NordheimFuchsExactTimestepper`]'s "Limitations" section.

## References
- Rhoades, W. A., & Green, W. B. (1964). *SNAP Reactor Handbook --
  Transient Analysis* (NAA-SR-9368). Atomics International, a Division
  of North American Aviation, Inc. AEC Research and Development Report.
  <https://www.osti.gov/servlets/purl/4034094> (title page confirmed by
  direct retrieval 2026-07-14).
- U.S. NRC, ADAMS Accession No. ML21265A551,
  <https://www.nrc.gov/docs/ML2126/ML21265A551.pdf> -- cited by the
  project owner as a Nordheim-Fuchs reference. NRC's server returned
  "Access Denied" to automated retrieval during this session, so its
  title/author are not independently confirmed in this doc comment;
  treat the citation as owner-supplied pending manual review.
- Equation set, notation, and intended-use framing transcribed from a
  design scaffold the project owner supplied 2026-07-14 (generated with
  Microsoft Copilot).

```rust
pub mod nordheim_fuchs { /* ... */ }
```

### Types

#### Struct `NordheimFuchsExactTimestepper`

Analytical (closed-form) prompt-power / adiabatic-fuel-temperature
integrator -- the "Prompt Excursion Layer" beneath the delayed-neutron
PRKE precursor bank ([`crate::zero_power_prke`]) and a thermal-hydraulic
layer (TAMPINES / TUAS) in the recommended Outram Park architecture.

## Physical model / assumptions
- Prompt neutrons only; delayed-precursor dynamics neglected.
- Lumped adiabatic fuel heat capacity `C_f`, uniform fuel temperature
  `T_f`.
- Fuel-temperature feedback only: `rho_f = alpha_f * (T_f - T_f,ref)`.
- External reactivity `rho_ext` held constant over a timestep.
- Requires `alpha_f < 0` (negative feedback) for a stable, real-valued
  closed-form solution -- **positive-feedback excursions are
  intentionally out of scope** and rejected at construction (see
  [`Self::new`]) and re-checked (via `assert!`, matching this crate's
  panic-on-invalid-physics-state convention) at every [`Self::step`].

## Governing equations
```text
dP/dt       = P * (rho_ext + alpha_f*(T_f - T_f,ref) - beta) / Lambda
C_f dT_f/dt = P
```
where `P` = reactor power, `T_f` = lumped fuel temperature, `C_f` =
fuel heat capacity, `Lambda` = prompt neutron generation time, `beta` =
delayed neutron fraction, `rho_ext` = externally imposed reactivity,
`alpha_f` = fuel feedback coefficient.

Substituting the reactivity margin
`r = rho_ext - beta + alpha_f*(T_f - T_f,ref)` collapses the pair into
a single Riccati-type ODE, `dr/dt = (r^2 - gamma^2) / (2 Lambda)`, with
`gamma^2 = r_k^2 + 2 Lambda |alpha_f| P_k / C_f` (real-valued because
`alpha_f < 0`). `gamma` is a conserved quantity of this ODE for as long
as `rho_ext` (and the other coefficients) stay fixed -- see the
`gamma_invariant_is_conserved` test below.

The exact per-step update (closed form -- no numerical ODE integration
needed) is:
```text
r_{k+1}   = -gamma * tanh( gamma*dt/(2*Lambda) + atanh(-r_k/gamma) )
T_f,{k+1} = T_f,ref + (r_{k+1} - rho_ext + beta) / alpha_f
P_{k+1}   = C_f/(2*Lambda*|alpha_f|) * (gamma^2 - r_{k+1}^2)
```
This removes the `dt << Lambda` stability restriction a standard ODE
stepper would need, so the timestep can instead be chosen for UI frame
rate / thermal-hydraulic coupling cadence / mobile-device performance.

## What this model deliberately omits
Delayed-neutron dynamics (stays in [`crate::zero_power_prke`]),
licensing/safety-grade calculations, reactor-protection analysis,
spatial kinetics, fuel-melt / severe-accident modelling. It represents
only the prompt self-limiting mechanism via adiabatic fuel heating.

## Known numerical edge case
At the idealized zero-power point (`P_k == 0`, so `gamma == |r_k|`),
the closed-form phase term `atanh(-r_k/gamma) = atanh(+-1)` diverges --
this is a genuine feature of the exact solution (the P -> 0 startup
singularity), not a bug. [`Self::step`] clamps the `atanh` argument
just shy of +-1 to avoid propagating `inf`/`NaN`; real simulations
should seed a small nonzero `power` rather than starting at exactly
zero.

```rust
pub struct NordheimFuchsExactTimestepper {
    pub prompt_neutron_generation_time: uom::si::f64::Time,
    pub delayed_neutron_fraction: uom::si::f64::Ratio,
    pub fuel_heat_capacity: uom::si::f64::HeatCapacity,
    pub fuel_feedback_coefficient: uom::si::f64::TemperatureCoefficient,
    pub fuel_reference_temperature: uom::si::f64::ThermodynamicTemperature,
    pub fuel_temperature: uom::si::f64::ThermodynamicTemperature,
    pub power: uom::si::f64::Power,
    pub external_reactivity: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `prompt_neutron_generation_time` | `uom::si::f64::Time` | `Lambda`, prompt neutron generation time \[s\]. Must be > 0. |
| `delayed_neutron_fraction` | `uom::si::f64::Ratio` | `beta`, delayed neutron fraction (dimensionless). |
| `fuel_heat_capacity` | `uom::si::f64::HeatCapacity` | `C_f`, lumped (whole-core, not specific) fuel heat capacity \[J/K\].<br>Must be > 0. |
| `fuel_feedback_coefficient` | `uom::si::f64::TemperatureCoefficient` | `alpha_f`, fuel-temperature feedback coefficient \[K^-1\]. Must be<br>strictly negative (self-limiting feedback) -- see "Physical model"<br>above. |
| `fuel_reference_temperature` | `uom::si::f64::ThermodynamicTemperature` | `T_f,ref`, reference fuel temperature the feedback is measured<br>against \[K\]. |
| `fuel_temperature` | `uom::si::f64::ThermodynamicTemperature` | Current lumped fuel temperature `T_f` \[K\]. |
| `power` | `uom::si::f64::Power` | Current reactor power `P` \[W\]. |
| `external_reactivity` | `uom::si::f64::Ratio` | Current externally imposed reactivity `rho_ext` (dimensionless),<br>held constant over a call to [`Self::step`]. Set directly or via<br>[`Self::drive_external_reactivity_from_first_order_lag`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(prompt_neutron_generation_time: Time, delayed_neutron_fraction: Ratio, fuel_heat_capacity: HeatCapacity, fuel_feedback_coefficient: TemperatureCoefficient, fuel_reference_temperature: ThermodynamicTemperature, initial_fuel_temperature: ThermodynamicTemperature, initial_power: Power) -> Result<Self, TehOPrkeError> { /* ... */ }
  ```
  Constructs a new timestepper, validating the preconditions the

- ```rust
  pub fn set_external_reactivity(self: &mut Self, rho_ext: Ratio) { /* ... */ }
  ```
  Directly sets the external reactivity `rho_ext` used by the next

- ```rust
  pub fn drive_external_reactivity_from_first_order_lag(self: &mut Self, driver: &mut TransferFnFirstOrder, commanded_reactivity: Ratio, time: Time) -> Result<Ratio, TehOPrkeError> { /* ... */ }
  ```
  Drives `external_reactivity` from an external first-order lag (a

- ```rust
  pub fn reactivity_margin(self: &Self) -> Ratio { /* ... */ }
  ```
  The current reactivity margin

- ```rust
  pub fn step(self: &mut Self, dt: Time) { /* ... */ }
  ```
  Advances `power` and `fuel_temperature` by one timestep `dt` using

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NordheimFuchsExactTimestepper { /* ... */ }
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
    Illustrative, pedagogical parameter set -- **not** representative

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &NordheimFuchsExactTimestepper) -> bool { /* ... */ }
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
## Module `delayed_neutron_layer`

reusable "Delayed Neutron Layer" -- a reduced point-kinetics precursor
bank modelled as five first-order lags, one per delayed-neutron group.
Sits between the prompt-only [`nordheim_fuchs`] layer and a thermal-
hydraulics layer, restoring the delayed-neutron source that damps a
reactivity-power-temperature feedback loop.
Reusable **delayed-neutron layer** — a reduced point-kinetics precursor
bank: five delayed-neutron groups whose precursor concentrations are
advanced by **direct implicit (backward-Euler) time-stepping**, one
`O(1)`-cost, `O(1)`-memory update per timestep.

# What this module is for

The [`crate::nordheim_fuchs`] exact timestepper is a **prompt-only**
excursion model: it evolves reactor power and adiabatic fuel temperature
but carries **no delayed-neutron precursors**. Physically the delayed
neutrons are the reservoir that gives an operating reactor its inertia and
its long, controllable period: a chain that is *prompt* subcritical (net
reactivity `rho` below the delayed fraction `beta`) is held critical only
by neutrons emitted seconds-to-minutes later from decaying precursors.
Strip that reservoir out and a prompt-only model coupled to a thermal
feedback loop degenerates into a bang-bang relaxation oscillation — power
explodes whenever the prompt margin `rho - beta` goes positive and
collapses the instant it goes negative, driving the fuel temperature (hence
the reactivity) up and down without damping.

This layer restores the delayed-neutron reservoir while **keeping
Nordheim-Fuchs for the prompt response**. It sits between the
prompt-excursion layer and the thermal-hydraulics layer as a
*delayed-neutron source* in a Lie-split point-kinetics update (see
"How to couple it" below).

# The model it implements

Point reactor kinetics in reactor-power form (power `P`, prompt neutron
generation time `Lambda`, net reactivity `rho`, delayed fraction
`beta = sum_i beta_i`):

```text
  dP/dt   = (rho - beta)/Lambda * P + S ,   S = sum_i lambda_i C_i
  dC_i/dt = beta_i/Lambda * P - lambda_i C_i
```

The **prompt** part `(rho - beta)/Lambda * P` (with adiabatic
fuel-temperature feedback folded into `rho`) is exactly what
[`crate::nordheim_fuchs::NordheimFuchsExactTimestepper::step`] advances.
This layer owns the **delayed** part: it integrates the precursor
concentrations `C_i` and returns the source `S = sum_i lambda_i C_i`.

# How the precursors are integrated (implicit / backward-Euler)

Each group's precursor concentration `C_i` obeys
`dC_i/dt = (beta_i / Lambda) * P - lambda_i C_i`. This layer advances the
five `C_i` directly in time with a **backward-Euler (implicit) step**,
holding the reactor power `P` constant across the step. Discretising the
ODE implicitly (`dC_i/dt approx (C_i^{n+1} - C_i^n)/dt`, decay term
evaluated at the new time `n+1`):

```text
  (C_i^{n+1} - C_i^n)/dt = (beta_i/Lambda) * P - lambda_i C_i^{n+1}
```

which rearranges to the closed-form per-group update actually used in
[`DelayedNeutronLayer::advance`]:

```text
  C_i^{n+1} = ( C_i^n + dt * (beta_i/Lambda) * P ) / ( 1 + dt * lambda_i )
```

The total delayed source is then `S = sum_i lambda_i C_i^{n+1}`, and over
the timestep `dt` it injects a delayed power increment
`Delta P_delayed = S * dt` into the balance.

This is the **same implicit precursor stepping** the crate's coupled
[`crate::zero_power_prke::six_group_precursor_prke::SixGroupPRKE`] solver
uses (its `construct_coefficient_matrix` builds rows
`(1 + dt*lambda_i) C_i^{n+1} - (dt/Lambda) beta_i n^{n+1} = C_i^n`, i.e. the
same backward-Euler discretisation) — here decoupled from the prompt
neutron-population equation, because Nordheim-Fuchs owns the prompt term.
It is unconditionally stable at the always-on 1 ms GUI timestep and costs a
**fixed five multiply-adds per step with no history** — see the design note
below.

### Why direct integration replaced the transfer-function approach (op-e46.4)

Earlier revisions modelled each group as a
`chem_eng…::TransferFnFirstOrder` first-order lag
`(beta_i/Lambda)/(tau_i s + 1)`, `tau_i = 1/lambda_i`. That is analytically
exact for a piecewise-constant input, but the transfer function accumulates
**one superposed response term per input change** and only prunes it after
`20*tau`. In the always-on 1 ms real-time loop, with the slowest group's
`tau_1 approx 80.8 s`, its buffer grows to `~1.6M` entries before clearing,
and the per-step summation over that buffer is `O(n)` — so per-step cost
grew without bound (measured `~49 us -> ~1.8 ms/step` from step 1k to 40k,
blowing the 1 ms budget ~28 s in). Direct implicit stepping holds only the
five `C_i` as state: **`O(1)` time and `O(1)` memory per step, forever**,
with no growing `Vec` and no dependence on `TransferFnFirstOrder`.

At steady state the update fixed-point gives `C_i = (beta_i/Lambda) P /
lambda_i`, so `S_i = lambda_i C_i = (beta_i/Lambda) P`, hence
`S = (beta/Lambda) P` and the power equation forces `rho = 0` (delayed
critical) — the physically correct operating point, which the prompt-only
model could not reach (it sat at prompt critical, `rho = beta`, and rang).

This is a deliberately reduced (pedagogical / real-time-simulator) model,
**not** a full spatially-resolved kinetics solve. It is intended for
education, capability building, and V&V demonstrations — not for
licensing, safety, or operational analysis.

# Timestep selection (why 1 ms, not 25 microseconds)

The delayed-neutron precursors are **slow** compared with the 1 ms
real-time GUI timestep. Each group's half-life is
`t_half,i = ln(2) / lambda_i`; for the five-group thermal-U-235 set this
layer ships (see [`DelayedNeutronLayer::u235_five_group`]):

| i | `lambda_i` \[s^-1\] | `t_half,i` \[s\] | `t_half,i` at `dt = 1 ms` |
|---|---------------------|------------------|---------------------------|
| 1 | 0.012378            | 56.0             | ~56,000 steps             |
| 2 | 0.030137            | 23.0             | ~23,000 steps             |
| 3 | 0.111799            | 6.20             | ~6,200 steps              |
| 4 | 0.301369            | 2.30             | ~2,300 steps              |
| 5 | 1.633286            | 0.424 (merged)   | ~424 steps                |

Even the **fastest** group (the merged short-lived group, half-life
`~0.42 s`) is resolved by `~424` timesteps of 1 ms; the slowest spans tens
of thousands. Every precursor timescale is three-plus orders of magnitude
longer than 1 ms, so a 1 ms step samples the precursor dynamics with a very
large margin.

The **25 microsecond** timestep the earlier coupled solver used was **not**
set by the precursors at all — it was set by the fast **prompt** neutron
kinetics (prompt neutron generation time `Lambda ~ 2.31e-4 s`, and an
explicit prompt-power update needs `dt << Lambda` for stability). In
`fhr_sim_v2` the prompt term is owned entirely by the closed-form
[`crate::nordheim_fuchs::NordheimFuchsExactTimestepper`] (no `dt << Lambda`
restriction), and **this** layer integrates *only* the delayed precursors,
whose `0.42–56 s` timescale imposes no such fine-step requirement. Combined
with the unconditionally-stable implicit (backward-Euler) update above,
1 ms is more than adequate and removes the previous fine-timestep cost.

# How to couple it (Lie-split point kinetics)

Per timestep `dt`, with the prompt-excursion layer as the prompt
propagator:

1. set the prompt layer's power to the current total power and its
   reactivity/feedback, then advance it one step → prompt power `P_p`
   (this applies the `(rho - beta)/Lambda * P` term and adiabatic
   feedback);
2. `let dp_delayed = layer.advance(P_p, dt);` — updates the precursor
   lags and returns the delayed power increment `S * dt`;
3. total power `P = P_p + dp_delayed`; feed `P` back to the prompt layer
   for the next step and to the thermal-hydraulics layer.

The delayed increment keeps the reactor alive through the prompt-subcritical
operating regime and, because the `S_i` lag `P`, supplies the inertia that
damps the fuel-temperature feedback loop.

# Standard delayed-neutron data

[`DelayedNeutronLayer::u235_five_group`] bakes in a documented five-group
reduced set for thermal U-235 (see that constructor). For any other fuel or
group structure, build the layer from explicit `(beta_i, lambda_i)` pairs
with [`DelayedNeutronLayer::new`].

```rust
pub mod delayed_neutron_layer { /* ... */ }
```

### Types

#### Struct `DelayedNeutronLayer`

A reusable delayed-neutron layer: five precursor groups, integrated by
direct implicit (backward-Euler) time-stepping, that turn a prompt-only
kinetics model into proper point kinetics by supplying the delayed-neutron
source `S = sum_i lambda_i C_i`.

Construct it with [`DelayedNeutronLayer::u235_five_group`] (documented
thermal-U-235 data) or [`DelayedNeutronLayer::new`] (arbitrary
`(beta_i, lambda_i)` data), then call [`DelayedNeutronLayer::advance`] once
per timestep with the prompt power to get the delayed power increment. See
the module-level documentation for the model and the coupling recipe.

```rust
pub struct DelayedNeutronLayer {
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
  pub fn new(prompt_generation_time: Time, groups: [(Ratio, Frequency); 5]) -> Result<Self, TehOPrkeError> { /* ... */ }
  ```
  Builds a delayed-neutron layer from explicit per-group data.

- ```rust
  pub fn u235_five_group(prompt_generation_time: Time) -> Self { /* ... */ }
  ```
  Builds the layer with a **documented five-group reduced set for thermal

- ```rust
  pub fn total_delayed_neutron_fraction(self: &Self) -> Ratio { /* ... */ }
  ```
  Total delayed-neutron fraction `beta = sum_i beta_i` (dimensionless).

- ```rust
  pub fn prompt_generation_time(self: &Self) -> Time { /* ... */ }
  ```
  Prompt neutron generation time `Lambda` \[s\] this layer was built with.

- ```rust
  pub fn decay_constants(self: &Self) -> [Frequency; 5] { /* ... */ }
  ```
  The per-group decay constants `lambda_i` \[s^-1\], in construction order.

- ```rust
  pub fn delayed_fractions(self: &Self) -> [Ratio; 5] { /* ... */ }
  ```
  The per-group delayed-neutron fractions `beta_i` (dimensionless), in

- ```rust
  pub fn last_delayed_increment(self: &Self) -> Power { /* ... */ }
  ```
  The delayed power increment `S * dt` produced by the most recent

- ```rust
  pub fn advance(self: &mut Self, reactor_power: Power, dt: Time) -> Power { /* ... */ }
  ```
  Advances the precursor bank by one implicit (backward-Euler) timestep

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DelayedNeutronLayer { /* ... */ }
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
### Constants and Statics

#### Constant `NUM_DELAYED_GROUPS`

Number of delayed-neutron groups this layer models. Fixed at five precursor
groups (see the module documentation for why the six-group standard data is
reduced to five).

```rust
pub const NUM_DELAYED_GROUPS: usize = 5;
```

