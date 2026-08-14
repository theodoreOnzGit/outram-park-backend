# Crate Documentation

**Version:** 0.2.0

**Format Version:** 60

# Module `chem_eng_real_time_process_control_simulator`

## Modules

## Module `alpha_nightly`

```rust
pub mod alpha_nightly { /* ... */ }
```

### Modules

## Module `prelude`

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `errors`

```rust
pub use super::errors;
```

#### Re-export `super::transfer_fn_wrapper_and_enums::*`

```rust
pub use super::transfer_fn_wrapper_and_enums::*;
```

#### Re-export `super::controllers::*`

```rust
pub use super::controllers::*;
```

## Module `controllers`

```rust
pub mod controllers { /* ... */ }
```

### Modules

## Module `integral_controller`

```rust
pub mod integral_controller { /* ... */ }
```

### Types

#### Struct `IntegralController`

Integral controller with transfer function

G(s) = K_c / (tau_I s) exp(-cs)

the controller has two main parts,
firstly, a delay function

and the integral ramp response function

```rust
pub struct IntegralController {
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
  pub fn new(controller_gain: Ratio, integral_time: Time) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```
  integral controller in the form:

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> IntegralController { /* ... */ }
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
    returns 1/s

- **Freeze**
- **From**
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
    fn into(self: Self) -> AnalogController { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &IntegralController) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &IntegralController) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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

- **TransferFnTraits**
  - ```rust
    fn set_dead_time(self: &mut Self, dead_time: Time) { /* ... */ }
    ```

  - ```rust
    fn set_user_input_and_calc(self: &mut Self, user_input: Ratio, time_of_input: Time) -> Result<Ratio, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn spawn_writer(self: &mut Self, name: String) -> Result<csv::Writer<std::fs::File>, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn csv_write_values(self: &mut Self, wtr: &mut csv::Writer<std::fs::File>, time: Time, input: Ratio, output: Ratio) -> Result<(), ChemEngProcessControlSimulatorError> { /* ... */ }
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
#### Struct `RampResponseRealTime`

Ramp response for integral controller
able to take in a time varying input

Transfer function is G(s) = 1/s

```rust
pub struct RampResponseRealTime {
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
  pub fn new(integral_time: Time, controller_gain: Ratio) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```

- ```rust
  pub fn set_user_input_and_calc(self: &mut Self, user_input: Ratio, time_of_input: Time) -> Result<Ratio, ChemEngProcessControlSimulatorError> { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RampResponseRealTime { /* ... */ }
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
    returns G(s) = 1/s

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &RampResponseRealTime) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &RampResponseRealTime) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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
#### Struct `RampResponse`

Ramp response for integral controller,
This is because the integral of a step function is
a ramp response

Allows for a user defined start time where the ramp
response switches on


The response is:

y(t) = u (t - t_start) * a1 * K * (t - t_start)


```rust
pub struct RampResponse {
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
  pub fn new(gradient_gain: Frequency, start_time: Time, user_input: Ratio, current_time: Time) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```
  constructor

- ```rust
  pub fn calculate_response(self: &mut Self, simulation_time: Time) -> Ratio { /* ... */ }
  ```
  calculates the current value of the ramp response

- ```rust
  pub fn is_started_for_1s(self: &Self) -> bool { /* ... */ }
  ```
  checks if ramp function is past its dead time

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RampResponse { /* ... */ }
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
    fn eq(self: &Self, other: &RampResponse) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &RampResponse) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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
### Types

#### Enum `AnalogController`

generic enum for a Continuous Time Controller

```rust
pub enum AnalogController {
    PIDFiltered(ProportionalController, IntegralController, FilteredDerivativeController),
    PI(ProportionalController, IntegralController),
    P(ProportionalController),
    PDFiltered(ProportionalController, FilteredDerivativeController),
    IntegralStandalone(IntegralController),
    DerivativeFilteredStandalone(FilteredDerivativeController),
}
```

##### Variants

###### `PIDFiltered`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ProportionalController` |  |
| 1 | `IntegralController` |  |
| 2 | `FilteredDerivativeController` |  |

###### `PI`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ProportionalController` |  |
| 1 | `IntegralController` |  |

###### `P`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ProportionalController` |  |

###### `PDFiltered`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ProportionalController` |  |
| 1 | `FilteredDerivativeController` |  |

###### `IntegralStandalone`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `IntegralController` |  |

###### `DerivativeFilteredStandalone`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FilteredDerivativeController` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new_pi_controller(controller_gain: Ratio, integral_time: Time) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```

- ```rust
  pub fn new_filtered_pid_controller(controller_gain: Ratio, integral_time: Time, derivative_time: Time, alpha: Ratio) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```

- ```rust
  pub fn new_filtered_pd_controller(controller_gain: Ratio, derivative_time: Time, alpha: Ratio) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AnalogController { /* ... */ }
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
    fn into(self: Self) -> AnalogController { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> AnalogController { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> AnalogController { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AnalogController) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &AnalogController) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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

- **TransferFnTraits**
  - ```rust
    fn set_dead_time(self: &mut Self, dead_time: uom::si::f64::Time) { /* ... */ }
    ```

  - ```rust
    fn set_user_input_and_calc(self: &mut Self, user_input: uom::si::f64::Ratio, time_of_input: uom::si::f64::Time) -> Result<uom::si::f64::Ratio, super::errors::ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn spawn_writer(self: &mut Self, name: String) -> Result<csv::Writer<std::fs::File>, super::errors::ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn csv_write_values(self: &mut Self, wtr: &mut csv::Writer<std::fs::File>, time: uom::si::f64::Time, input: uom::si::f64::Ratio, output: uom::si::f64::Ratio) -> Result<(), super::errors::ChemEngProcessControlSimulatorError> { /* ... */ }
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
### Re-exports

#### Re-export `IntegralController`

```rust
pub use self::integral_controller::IntegralController;
```

#### Re-export `ProportionalController`

```rust
pub use self::proportional_controller::ProportionalController;
```

#### Re-export `FilteredDerivativeController`

```rust
pub use self::filtered_derivative_controller::FilteredDerivativeController;
```

## Module `errors`

```rust
pub mod errors { /* ... */ }
```

### Types

#### Enum `ChemEngProcessControlSimulatorError`

Master Error type of this crate

```rust
pub enum ChemEngProcessControlSimulatorError {
    GenericStringError(String),
    UnstableDampingFactorForStableTransferFunction,
    WrongTransferFnType,
    CsvError(csv::Error),
}
```

##### Variants

###### `GenericStringError`

it's a generic error which is a placeholder since I used
so many string errors

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `UnstableDampingFactorForStableTransferFunction`

when transfer function is unstable when it should be
stable

###### `WrongTransferFnType`

###### `CsvError`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `csv::Error` |  |

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
    fn from(csv_error: csv::Error) -> Self { /* ... */ }
    ```

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
## Module `transfer_fn_wrapper_and_enums`

```rust
pub mod transfer_fn_wrapper_and_enums { /* ... */ }
```

### Modules

## Module `generic_second_order`

```rust
pub mod generic_second_order { /* ... */ }
```

### Types

#### Enum `TransferFnSecondOrder`

an enum describing generic second order systems,
only stable systems are implemented so far

you are meant to put in:

G(s) =

a1 s^2 + b1 s + c1
------------------
a2 s^2 + b2 s + c2

```rust
pub enum TransferFnSecondOrder {
    StableUnderdamped(crate::alpha_nightly::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes, crate::alpha_nightly::stable_transfer_functions::decaying_sinusoid::DecayingSinusoid, crate::alpha_nightly::stable_transfer_functions::decaying_sinusoid::DecayingSinusoid),
    StableCriticallydamped(crate::alpha_nightly::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes, crate::alpha_nightly::stable_transfer_functions::decaying_exponentials::DecayingSecondOrderExponential),
    StableOverdamped(crate::alpha_nightly::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes, crate::alpha_nightly::stable_transfer_functions::decaying_exponentials::DecayingSecondOrderExponential),
    Unstable,
    Undamped,
}
```

##### Variants

###### `StableUnderdamped`

this is arranged in the order
no_zero_transfer_fn,
cosine_term,
sine_term

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::alpha_nightly::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes` |  |
| 1 | `crate::alpha_nightly::stable_transfer_functions::decaying_sinusoid::DecayingSinusoid` |  |
| 2 | `crate::alpha_nightly::stable_transfer_functions::decaying_sinusoid::DecayingSinusoid` |  |

###### `StableCriticallydamped`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::alpha_nightly::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes` |  |
| 1 | `crate::alpha_nightly::stable_transfer_functions::decaying_exponentials::DecayingSecondOrderExponential` |  |

###### `StableOverdamped`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::alpha_nightly::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes` |  |
| 1 | `crate::alpha_nightly::stable_transfer_functions::decaying_exponentials::DecayingSecondOrderExponential` |  |

###### `Unstable`

###### `Undamped`

##### Implementations

###### Methods

- ```rust
  pub fn new(a1: uom::si::Quantity<uom::si::ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, uom::si::SI<f64>, f64>, b1: Time, c1: Ratio, a2: uom::si::Quantity<uom::si::ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, uom::si::SI<f64>, f64>, b2: Time, c2: Ratio) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```
  generic constructor based on polynomials

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TransferFnSecondOrder { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> TransferFn { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TransferFnSecondOrder) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TransferFnSecondOrder) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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

- **TransferFnTraits**
  - ```rust
    fn set_dead_time(self: &mut Self, dead_time: Time) { /* ... */ }
    ```

  - ```rust
    fn set_user_input_and_calc(self: &mut Self, user_input: Ratio, time: Time) -> Result<Ratio, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn spawn_writer(self: &mut Self, name: String) -> Result<Writer<std::fs::File>, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn csv_write_values(self: &mut Self, wtr: &mut Writer<std::fs::File>, time: Time, input: Ratio, output: Ratio) -> Result<(), ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(generic_transfer_function: TransferFn) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `generic_first_order`

```rust
pub mod generic_first_order { /* ... */ }
```

### Types

#### Enum `TransferFnFirstOrder`

an enum describing generic second order systems,
only stable systems are implemented so far

you are meant to put in:

G(s) =

a1 s + b1
----------
a2 s + b2

There are three kinds,

1. Stable
2. Undamped (constant value)
3. Unstable

Now this is actually a summation of two first order transfer
functions and some offset

let Kp = b1/b2
let tau_p = a2/b2

we get:

G(s) =

```text
    Kp          a1        /             1          \
----------- + ----------- |  1 -  ---------------- |
tau_p s + 1   b2 * taup   \        taup s + 1      /
```

The first term is taken care of by a
FirstOrderStableTransferFnNoZeroes,

the second term, is there due to the zeroes, therefore
it is take care of by
FirstOrderStableTransferFnForZeroes

```rust
pub enum TransferFnFirstOrder {
    Stable(crate::alpha_nightly::stable_transfer_functions::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes, crate::alpha_nightly::stable_transfer_functions::first_order_transfer_fn_with_zeroes::FirstOrderStableTransferFnForZeroes),
    Unstable,
    ConstantValueUndamped,
}
```

##### Variants

###### `Stable`

this is arranged in the order
no_zero_transfer_fn,
cosine_term,
sine_term

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::alpha_nightly::stable_transfer_functions::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes` |  |
| 1 | `crate::alpha_nightly::stable_transfer_functions::first_order_transfer_fn_with_zeroes::FirstOrderStableTransferFnForZeroes` |  |

###### `Unstable`

###### `ConstantValueUndamped`

##### Implementations

###### Methods

- ```rust
  pub fn new(a1: Time, b1: Ratio, a2: Time, b2: Ratio) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```
  generic constructor based on polynomials

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TransferFnFirstOrder { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> TransferFn { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TransferFnFirstOrder) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TransferFnFirstOrder) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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

- **TransferFnTraits**
  - ```rust
    fn set_dead_time(self: &mut Self, dead_time: Time) { /* ... */ }
    ```

  - ```rust
    fn set_user_input_and_calc(self: &mut Self, user_input: Ratio, time: Time) -> Result<Ratio, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn spawn_writer(self: &mut Self, name: String) -> Result<Writer<std::fs::File>, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn csv_write_values(self: &mut Self, wtr: &mut Writer<std::fs::File>, time: Time, input: Ratio, output: Ratio) -> Result<(), ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(generic_transfer_function: TransferFn) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Types

#### Enum `TransferFn`

generic enum for a Transfer Function

```rust
pub enum TransferFn {
    FirstOrder(TransferFnFirstOrder),
    SecondOrder(TransferFnSecondOrder),
}
```

##### Variants

###### `FirstOrder`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TransferFnFirstOrder` |  |

###### `SecondOrder`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TransferFnSecondOrder` |  |

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
    fn clone(self: &Self) -> TransferFn { /* ... */ }
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
    fn default() -> TransferFn { /* ... */ }
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
    fn into(self: Self) -> TransferFn { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> TransferFn { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TransferFn) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TransferFn) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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

- **TransferFnTraits**
  - ```rust
    fn set_dead_time(self: &mut Self, dead_time: Time) { /* ... */ }
    ```

  - ```rust
    fn set_user_input_and_calc(self: &mut Self, user_input: Ratio, time_of_input: Time) -> Result<Ratio, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn spawn_writer(self: &mut Self, name: String) -> Result<Writer<std::fs::File>, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn csv_write_values(self: &mut Self, wtr: &mut Writer<std::fs::File>, time: Time, input: Ratio, output: Ratio) -> Result<(), ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(generic_transfer_function: TransferFn) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(generic_transfer_function: TransferFn) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `TransferFnTraits`

```rust
pub trait TransferFnTraits {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `set_dead_time`
- `set_user_input_and_calc`
- `spawn_writer`
- `csv_write_values`

##### Implementations

This trait is implemented for the following types:

- `ProportionalController`
- `IntegralController`
- `FilteredDerivativeController`
- `AnalogController`
- `TransferFn`
- `TransferFnSecondOrder`
- `TransferFnFirstOrder`

### Re-exports

#### Re-export `TransferFnSecondOrder`

```rust
pub use generic_second_order::TransferFnSecondOrder;
```

#### Re-export `TransferFnFirstOrder`

```rust
pub use generic_first_order::TransferFnFirstOrder;
```

## Module `beta_testing`

```rust
pub mod beta_testing { /* ... */ }
```

### Modules

## Module `prelude`

```rust
pub mod prelude { /* ... */ }
```

## Module `errors`

```rust
pub mod errors { /* ... */ }
```

### Types

#### Enum `ChemEngProcessControlSimulatorError`

Master Error type of this crate

```rust
pub enum ChemEngProcessControlSimulatorError {
    GenericStringError(String),
    UnstableDampingFactorForStableTransferFunction,
    WrongTransferFnType,
    CsvError(csv::Error),
}
```

##### Variants

###### `GenericStringError`

it's a generic error which is a placeholder since I used
so many string errors

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `UnstableDampingFactorForStableTransferFunction`

when transfer function is unstable when it should be
stable

###### `WrongTransferFnType`

###### `CsvError`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `csv::Error` |  |

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
    fn from(csv_error: csv::Error) -> Self { /* ... */ }
    ```

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
## Module `transfer_fn_wrapper_and_enums`

```rust
pub mod transfer_fn_wrapper_and_enums { /* ... */ }
```

### Modules

## Module `generic_second_order`

```rust
pub mod generic_second_order { /* ... */ }
```

### Types

#### Enum `TransferFnSecondOrder`

an enum describing generic second order systems,
only stable systems are implemented so far

you are meant to put in:

G(s) =

a1 s^2 + b1 s + c1
------------------
a2 s^2 + b2 s + c2

```rust
pub enum TransferFnSecondOrder {
    StableUnderdamped(crate::beta_testing::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes, crate::beta_testing::stable_transfer_functions::decaying_sinusoid::DecayingSinusoid, crate::beta_testing::stable_transfer_functions::decaying_sinusoid::DecayingSinusoid),
    StableCriticallydamped,
    StableOverdamped,
    Unstable,
    Undamped,
}
```

##### Variants

###### `StableUnderdamped`

this is arranged in the order
no_zero_transfer_fn,
cosine_term,
sine_term

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::beta_testing::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes` |  |
| 1 | `crate::beta_testing::stable_transfer_functions::decaying_sinusoid::DecayingSinusoid` |  |
| 2 | `crate::beta_testing::stable_transfer_functions::decaying_sinusoid::DecayingSinusoid` |  |

###### `StableCriticallydamped`

###### `StableOverdamped`

###### `Unstable`

###### `Undamped`

##### Implementations

###### Methods

- ```rust
  pub fn new(a1: uom::si::Quantity<uom::si::ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, uom::si::SI<f64>, f64>, b1: Time, c1: Ratio, a2: uom::si::Quantity<uom::si::ISQ<Z0, Z0, P2, Z0, Z0, Z0, Z0>, uom::si::SI<f64>, f64>, b2: Time, c2: Ratio) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```
  generic constructor based on polynomials

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TransferFnSecondOrder { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> TransferFn { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TransferFnSecondOrder) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TransferFnSecondOrder) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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

- **TransferFnTraits**
  - ```rust
    fn set_dead_time(self: &mut Self, dead_time: Time) { /* ... */ }
    ```

  - ```rust
    fn set_user_input_and_calc(self: &mut Self, user_input: Ratio, time: Time) -> Result<Ratio, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn spawn_writer(self: &mut Self, name: String) -> Result<Writer<std::fs::File>, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn csv_write_values(self: &mut Self, wtr: &mut Writer<std::fs::File>, time: Time, input: Ratio, output: Ratio) -> Result<(), ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(generic_transfer_function: TransferFn) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `generic_first_order`

```rust
pub mod generic_first_order { /* ... */ }
```

### Types

#### Enum `TransferFnFirstOrder`

an enum describing generic second order systems,
only stable systems are implemented so far

you are meant to put in:

G(s) =

a1 s + b1
----------
a2 s + b2

There are three kinds,

1. Stable
2. Undamped (constant value)
3. Unstable

Now this is actually a summation of two first order transfer
functions and some offset

let Kp = b1/b2
let tau_p = a2/b2

we get:

G(s) =

```text
    Kp          a1        /             1          \
----------- + ----------- |  1 -  ---------------- |
tau_p s + 1   b2 * taup   \        taup s + 1      /
```

The first term is taken care of by a
FirstOrderStableTransferFnNoZeroes,

the second term, is there due to the zeroes, therefore
it is take care of by
FirstOrderStableTransferFnForZeroes

```rust
pub enum TransferFnFirstOrder {
    Stable(crate::beta_testing::stable_transfer_functions::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes, crate::beta_testing::stable_transfer_functions::first_order_transfer_fn_with_zeroes::FirstOrderStableTransferFnForZeroes),
    Unstable,
    ConstantValueUndamped,
}
```

##### Variants

###### `Stable`

this is arranged in the order
no_zero_transfer_fn,
cosine_term,
sine_term

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::beta_testing::stable_transfer_functions::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes` |  |
| 1 | `crate::beta_testing::stable_transfer_functions::first_order_transfer_fn_with_zeroes::FirstOrderStableTransferFnForZeroes` |  |

###### `Unstable`

###### `ConstantValueUndamped`

##### Implementations

###### Methods

- ```rust
  pub fn new(a1: Time, b1: Ratio, a2: Time, b2: Ratio) -> Result<Self, ChemEngProcessControlSimulatorError> { /* ... */ }
  ```
  generic constructor based on polynomials

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TransferFnFirstOrder { /* ... */ }
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

  - ```rust
    fn into(self: Self) -> TransferFn { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TransferFnFirstOrder) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TransferFnFirstOrder) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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

- **TransferFnTraits**
  - ```rust
    fn set_dead_time(self: &mut Self, dead_time: Time) { /* ... */ }
    ```

  - ```rust
    fn set_user_input_and_calc(self: &mut Self, user_input: Ratio, time: Time) -> Result<Ratio, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn spawn_writer(self: &mut Self, name: String) -> Result<Writer<std::fs::File>, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn csv_write_values(self: &mut Self, wtr: &mut Writer<std::fs::File>, time: Time, input: Ratio, output: Ratio) -> Result<(), ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(generic_transfer_function: TransferFn) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Types

#### Enum `TransferFn`

generic enum for a Transfer Function

```rust
pub enum TransferFn {
    FirstOrder(TransferFnFirstOrder),
    SecondOrder(TransferFnSecondOrder),
}
```

##### Variants

###### `FirstOrder`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TransferFnFirstOrder` |  |

###### `SecondOrder`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TransferFnSecondOrder` |  |

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
    fn clone(self: &Self) -> TransferFn { /* ... */ }
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
    fn default() -> TransferFn { /* ... */ }
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
    fn into(self: Self) -> TransferFn { /* ... */ }
    ```

  - ```rust
    fn into(self: Self) -> TransferFn { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TransferFn) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TransferFn) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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

- **TransferFnTraits**
  - ```rust
    fn set_dead_time(self: &mut Self, dead_time: Time) { /* ... */ }
    ```

  - ```rust
    fn set_user_input_and_calc(self: &mut Self, user_input: Ratio, time_of_input: Time) -> Result<Ratio, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn spawn_writer(self: &mut Self, name: String) -> Result<Writer<std::fs::File>, ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

  - ```rust
    fn csv_write_values(self: &mut Self, wtr: &mut Writer<std::fs::File>, time: Time, input: Ratio, output: Ratio) -> Result<(), ChemEngProcessControlSimulatorError> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(generic_transfer_function: TransferFn) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

  - ```rust
    fn try_from(generic_transfer_function: TransferFn) -> Result<Self, <Self as >::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `TransferFnTraits`

```rust
pub trait TransferFnTraits {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `set_dead_time`
- `set_user_input_and_calc`
- `spawn_writer`
- `csv_write_values`

##### Implementations

This trait is implemented for the following types:

- `TransferFn`
- `TransferFnSecondOrder`
- `TransferFnFirstOrder`

### Re-exports

#### Re-export `TransferFnSecondOrder`

```rust
pub use generic_second_order::TransferFnSecondOrder;
```

#### Re-export `TransferFnFirstOrder`

```rust
pub use generic_first_order::TransferFnFirstOrder;
```

## Module `z_domain`

z-domain (discrete-time) transfer functions and continuous <-> discrete
conversion, ported from the GNU Octave control package.

# What belongs here

- [`ContinuousTransferFn`] — a SISO continuous-time transfer function
  `G(s) = num(s)/den(s)` held as real polynomial coefficients
  (Octave `tf` equivalent, SISO only).
- [`DiscreteTransferFn`] — a SISO discrete-time transfer function
  `G(z^-1)` with a sample time, held in DSP form (ascending powers of
  `z^-1`, Octave `filt` equivalent), advanced sample-by-sample by an
  O(1) fixed-state recurrence.
- [`C2dMethod`] / [`D2cMethod`] and the conversions
  [`ContinuousTransferFn::to_discrete`] (Octave `c2d`) and
  [`DiscreteTransferFn::to_continuous`] (Octave `d2c`).

# What does NOT belong here

- MIMO systems, state-space models as a public surface, frequency-domain
  plotting, and the SLICOT numerical library. Upstream's `c2d` reaches
  MIMO/state-space generality through the BSD-3-licensed SLICOT kernels;
  this module deliberately stays SISO and order <= 2 for the methods
  that need eigenvalues (`Zoh`, `MatchedPoleZero`), because every block
  this crate ships (first-order lag, first-order with zero, second-order)
  is order <= 2 and a closed form exists there.
- Discrete Riccati/Lyapunov machinery (`dlqr`, `dare`, `dlyap`): those
  pull in SLICOT Riccati solvers and are tracked as a follow-up bead,
  not half-ported here.
- Dead time / transport delay: the continuous blocks in
  `stable_transfer_functions` handle dead time themselves; this layer
  converts the rational part only.

# Relation to the O(1) recurrence blocks

The `stable_transfer_functions` blocks advance by the zero-order-hold
(step-invariant) discrete equivalent specialised to their own structure.
[`C2dMethod::Zoh`] is the *same mathematics* in general form: converting
a first-order lag with `Zoh` and stepping the result reproduces
`FirstOrderStableTransferFnNoZeroes` sample-for-sample (this is verified
in `verification_tests.rs`). What this module adds beyond that block is
the other discretisation methods (`Tustin`, `TustinPrewarp`,
`MatchedPoleZero`), the inverse direction (`d2c`), and an explicit
coefficient-level representation you can inspect.

# Units (`uom`)

Sample times are `uom` [`Time`](uom::si::f64::Time) (seconds) and block
input/output signals are dimensionless [`Ratio`](uom::si::f64::Ratio),
matching the rest of the crate. **Polynomial coefficients are plain
`f64`**: the coefficient of `s^k` carries units of `s^k` (SI seconds
implied) and the coefficients of a z-polynomial are genuinely
dimensionless, so a single `uom` type cannot represent a coefficient
vector — forcing one would misstate the physics rather than protect it.
This is a documented, deliberate exception to the uom-everywhere rule.

```rust
pub mod z_domain { /* ... */ }
```

### Modules

## Module `continuous_tf`

SISO continuous-time transfer functions as polynomial coefficient
vectors — the minimal `tf` surface the `c2d`/`d2c` port needs.

```rust
pub mod continuous_tf { /* ... */ }
```

### Types

#### Struct `ContinuousTransferFn`

A SISO continuous-time transfer function

```text
          num(s)     num[0] + num[1] s + num[2] s^2 + ...
G(s)  =  --------  = ------------------------------------
          den(s)     den[0] + den[1] s + den[2] s^2 + ...
```

# Physical quantity

Maps a dimensionless input signal to a dimensionless output signal in the
Laplace domain — the same unit-agnostic convention as the crate's
time-domain blocks, which sit between scaled signals in a control loop.

# Units

Coefficients are stored **ascending in powers of `s`** and are plain
`f64`: the coefficient of `s^k` implicitly carries units of `seconds^k`
(SI). A single `uom` type cannot span a coefficient vector whose entries
all have different dimensions, so the vector is deliberately untyped and
the `uom`-typed constructors ([`Self::first_order`],
[`Self::second_order`]) are the recommended entry points.

Note the **ascending** convention differs from Octave's `tf`, which
lists coefficients in descending powers; the conversion is a `reverse`.

# Valid ranges and assumptions

- The denominator must not be the zero polynomial
  ([`ZDomainError::ZeroDenominator`]).
- Stability is *not* required or checked here: `d2c` of a discrete
  system can legitimately produce an unstable continuous model. The
  stability-guaranteed types live in `stable_transfer_functions`.

```rust
pub struct ContinuousTransferFn {
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
  pub fn new(num: Vec<f64>, den: Vec<f64>) -> Result<Self, ZDomainError> { /* ... */ }
  ```
  Builds `G(s) = num(s)/den(s)` from coefficient vectors in

- ```rust
  pub fn first_order(process_gain: Ratio, process_time: Time) -> Result<Self, ZDomainError> { /* ... */ }
  ```
  Builds the first-order lag `G(s) = K_p / (tau_p s + 1)` — the same

- ```rust
  pub fn second_order(process_gain: Ratio, process_time: Time, damping_ratio: Ratio) -> Result<Self, ZDomainError> { /* ... */ }
  ```
  Builds the stable second-order form used across this crate,

- ```rust
  pub fn numerator_ascending_s(self: &Self) -> &[f64] { /* ... */ }
  ```
  Numerator coefficients, ascending powers of `s` (`s^k` coefficient

- ```rust
  pub fn denominator_ascending_s(self: &Self) -> &[f64] { /* ... */ }
  ```
  Denominator coefficients, ascending powers of `s`.

- ```rust
  pub fn order(self: &Self) -> usize { /* ... */ }
  ```
  System order: the larger of numerator and denominator degree.

- ```rust
  pub fn steady_state_gain(self: &Self) -> f64 { /* ... */ }
  ```
  Steady-state (DC) gain `G(0) = num[0]/den[0]`, dimensionless.

- ```rust
  pub fn to_discrete(self: &Self, sample_time: Time, method: C2dMethod) -> Result<DiscreteTransferFn, ZDomainError> { /* ... */ }
  ```
  Converts this continuous-time transfer function into its

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ContinuousTransferFn { /* ... */ }
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
    fn eq(self: &Self, other: &ContinuousTransferFn) -> bool { /* ... */ }
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
## Module `conversion`

Continuous <-> discrete transfer-function conversion (`c2d` / `d2c`).

# Methods and their mathematics

- **Zero-order hold** ([`C2dMethod::Zoh`]) — the step-invariant
  transformation: exact at the sample instants whenever the input is
  held constant over each sample interval, which is precisely what a
  time-stepping simulator produces. This is the same mathematics as the
  O(1) recurrences in `stable_transfer_functions` (see that module's
  docs and bead `op-fm5`); here it is computed for a general SISO
  transfer function of order <= 2 via the companion-form state-space
  `x' = A x + B u`, `Phi = exp(A T)`,
  `Gamma = (integral_0^T exp(A s) ds) B`.
- **Tustin / bilinear** ([`C2dMethod::Tustin`]) — the substitution
  `s = (2/T) (z - 1)/(z + 1)` (the trapezoidal integration rule),
  applied directly to the numerator and denominator polynomials. Works
  for any order.
- **Tustin with prewarping** ([`C2dMethod::TustinPrewarp`]) — bilinear
  with `beta = w0 / tan(w0 T / 2)` in place of `2/T`, so the frequency
  response is exact at the angular frequency `w0` (upstream
  `inst/@ss/__c2d__.m` uses the identical `beta`).
- **Matched pole/zero** ([`C2dMethod::MatchedPoleZero`]) — maps every
  pole and finite zero through `z = exp(s T)`, fills the excess zeros at
  `z = -1` (all but one), and matches the gain at DC (or, if a pole or
  zero sits on the imaginary axis near DC, at the first clear frequency)
  — a direct port of the matched branch of `inst/@tf/__c2d__.m`.

`d2c` supports `Tustin`, `TustinPrewarp` (the inverse substitution
`z = (beta + s)/(beta - s)`) and `MatchedPoleZero` (`s = ln(z)/T`).
**`d2c` by zero-order hold is deliberately not ported**: upstream
computes it with a matrix logarithm (`logm`), a general dense-matrix
algorithm out of scope here — tracked as a follow-up bead rather than
half-implemented. The same applies to upstream's `foh` (first-order
hold) and `impulse` invariant methods on the `c2d` side.

# References

The zero-order-hold mathematics is the same textbook material cited in
`stable_transfer_functions/first_order_transfer_fn.rs` (Astrom &
Wittenmark; Seborg, Edgar, Mellichamp & Doyle; Franklin, Powell &
Workman; Ogata; Oppenheim & Schafer; Smith). As recorded there and in
bead `op-ia5j`, **those citations are unverified against physical
copies — no edition, year or page number is given, and none should be
added without checking.**

```rust
pub mod conversion { /* ... */ }
```

### Types

#### Enum `C2dMethod`

Continuous-to-discrete conversion method (`c2d`).

Enum dispatch per the workspace Rust design rules (no trait objects).
Upstream Octave method strings map as: `"zoh"`/`"std"` -> [`Self::Zoh`],
`"tustin"`/`"bilin"` -> [`Self::Tustin`], `"prewarp"` ->
[`Self::TustinPrewarp`], `"matched"` -> [`Self::MatchedPoleZero`].
Upstream's `"foh"` and `"impulse"` are not ported (see module docs).

```rust
pub enum C2dMethod {
    Zoh,
    Tustin,
    TustinPrewarp {
        prewarp_frequency: AngularVelocity,
    },
    MatchedPoleZero,
}
```

##### Variants

###### `Zoh`

Zero-order hold (step-invariant): exact at sample instants for
piecewise-constant input. Requires a proper transfer function of
order <= 2.

###### `Tustin`

Bilinear (trapezoidal) transformation, `s = (2/T)(z-1)/(z+1)`.
Any order.

###### `TustinPrewarp`

Bilinear transformation with frequency prewarping: the discrete
frequency response is exact at `prewarp_frequency`. Any order.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `prewarp_frequency` | `AngularVelocity` | Angular frequency `w0` at which the response is matched, in<br>rad/s (`uom` `AngularVelocity`). Must satisfy `0 < w0 < pi/T`. |

###### `MatchedPoleZero`

Matched pole/zero method (`z = exp(s T)` on poles and finite zeros,
excess zeros at `z = -1`, gain matched at DC). Requires order <= 2.

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
    fn clone(self: &Self) -> C2dMethod { /* ... */ }
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
    fn eq(self: &Self, other: &C2dMethod) -> bool { /* ... */ }
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
#### Enum `D2cMethod`

Discrete-to-continuous conversion method (`d2c`).

Upstream's `"zoh"` inverse needs a matrix logarithm and is deliberately
absent (see module docs and the follow-up bead).

```rust
pub enum D2cMethod {
    Tustin,
    TustinPrewarp {
        prewarp_frequency: AngularVelocity,
    },
    MatchedPoleZero,
}
```

##### Variants

###### `Tustin`

Inverse bilinear transformation, `z = (beta + s)/(beta - s)` with
`beta = 2/T`. Any order.

###### `TustinPrewarp`

Inverse bilinear transformation with prewarping at
`prewarp_frequency` (rad/s); `beta = w0 / tan(w0 T / 2)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `prewarp_frequency` | `AngularVelocity` | Angular frequency `w0` in rad/s; must satisfy `0 < w0 < pi/T`. |

###### `MatchedPoleZero`

Matched pole/zero method, `s = ln(z)/T`. Requires order <= 2 and no
pole or zero at `z = 0`.

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
    fn clone(self: &Self) -> D2cMethod { /* ... */ }
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
    fn eq(self: &Self, other: &D2cMethod) -> bool { /* ... */ }
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
## Module `discrete_tf`

SISO discrete-time transfer functions in DSP (`z^-1`) form, advanced by
an O(1) fixed-state recurrence — the `filt` surface of the Octave port.

The stepping recurrence is the direct-form-II-transposed realisation of
the difference equation, which carries exactly `max(deg num, deg den)`
state values — a fixed number. This deliberately matches the crate-wide
rule that a block must never accumulate a growing history of its inputs
(bead `op-fm5`).

```rust
pub mod discrete_tf { /* ... */ }
```

### Types

#### Struct `DiscreteTransferFn`

A SISO discrete-time transfer function with sample time `T`, stored in
DSP format (Octave `filt` convention — ascending powers of `z^-1`):

```text
            b0 + b1 z^-1 + b2 z^-2 + ...
G(z^-1) = --------------------------------,   a0 = 1 after normalisation
            a0 + a1 z^-1 + a2 z^-2 + ...
```

equivalent to the difference equation (with `u` the input samples and
`y` the output samples)

```text
y[n] = b0 u[n] + b1 u[n-1] + ... - a1 y[n-1] - a2 y[n-2] - ...
```

# Physical quantity

Maps a dimensionless input sample stream to a dimensionless output
sample stream (`uom` `Ratio` both ways), one sample per `sample_time`.
The z-polynomial coefficients are genuinely dimensionless, so they are
plain `f64` by design; the sample time is a physical `uom`
[`Time`](uom::si::f64::Time) in seconds.

# Valid ranges and assumptions

- `sample_time` must be strictly positive.
- The `z^0` denominator coefficient `a0` must be nonzero (otherwise the
  difference equation would need future inputs — an acausal system) and
  is normalised to 1 on construction.
- Samples must be fed at the fixed sample interval; unlike the
  continuous-time blocks in `stable_transfer_functions`, a discrete
  transfer function has no meaning between samples and cannot absorb an
  irregular timestep. If your simulator steps irregularly, keep using
  the continuous blocks.

```rust
pub struct DiscreteTransferFn {
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
  pub fn to_continuous(self: &Self, method: D2cMethod) -> Result<ContinuousTransferFn, ZDomainError> { /* ... */ }
  ```
  Converts this discrete-time transfer function back into a

- ```rust
  pub fn from_z_inverse_coefficients(num: Vec<f64>, den: Vec<f64>, sample_time: Time) -> Result<Self, ZDomainError> { /* ... */ }
  ```
  Builds a discrete transfer function from coefficients in **ascending

- ```rust
  pub fn from_z_descending_coefficients(num_descending_z: Vec<f64>, den_descending_z: Vec<f64>, sample_time: Time) -> Result<Self, ZDomainError> { /* ... */ }
  ```
  Builds a discrete transfer function from coefficients in

- ```rust
  pub fn advance_one_sample(self: &mut Self, input: Ratio) -> Ratio { /* ... */ }
  ```
  Advances the block by exactly one sample interval, applying the

- ```rust
  pub fn reset(self: &mut Self) { /* ... */ }
  ```
  Resets the internal state to zero (a block at rest with zero past

- ```rust
  pub fn numerator_z_inverse(self: &Self) -> &[f64] { /* ... */ }
  ```
  Numerator coefficients in ascending powers of `z^-1` (dimensionless;

- ```rust
  pub fn denominator_z_inverse(self: &Self) -> &[f64] { /* ... */ }
  ```
  Denominator coefficients in ascending powers of `z^-1`

- ```rust
  pub fn sample_time(self: &Self) -> Time { /* ... */ }
  ```
  The sample interval `T` (seconds).

- ```rust
  pub fn state_size(self: &Self) -> usize { /* ... */ }
  ```
  Number of state values the block carries. Fixed at construction

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DiscreteTransferFn { /* ... */ }
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
    fn eq(self: &Self, other: &DiscreteTransferFn) -> bool { /* ... */ }
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
### Types

#### Enum `ZDomainError`

Errors from z-domain construction and conversion.

This is deliberately separate from
[`ChemEngProcessControlSimulatorError`](crate::beta_testing::errors::ChemEngProcessControlSimulatorError)
so that the z-domain module stays self-contained and does not touch the
twinned error files shared with `alpha_nightly`.

```rust
pub enum ZDomainError {
    ZeroDenominator,
    NonPositiveTimeConstant,
    NonPositiveDampingRatio,
    NonPositiveSampleTime,
    AcausalSystem,
    UnsupportedOrder {
        order: usize,
    },
    ImproperTransferFunction,
    MatchedPoleZeroAtOrigin,
    NonFinitePoleOrZero,
    InvalidPrewarpFrequency,
}
```

##### Variants

###### `ZeroDenominator`

A denominator polynomial was empty or identically zero.

###### `NonPositiveTimeConstant`

A time constant that must be strictly positive (seconds) was not.

###### `NonPositiveDampingRatio`

A damping ratio that must be strictly positive (dimensionless) was not.

###### `NonPositiveSampleTime`

The sample time (seconds) must be strictly positive.

###### `AcausalSystem`

A discrete transfer function whose leading denominator coefficient
(the `z^0` term of the polynomial in `z^-1`) is zero describes an
acausal system and cannot be simulated forward in time.

###### `UnsupportedOrder`

The requested conversion method is only implemented for system order
<= 2 (`Zoh`, `MatchedPoleZero` need eigenvalues, which this crate
computes analytically). Use `Tustin`/`TustinPrewarp` for higher-order
systems, or see the follow-up bead for a general-order port.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `order` | `usize` | The offending system order (max of numerator and denominator degree). |

###### `ImproperTransferFunction`

`Zoh` discretisation requires a proper transfer function
(numerator degree <= denominator degree).

###### `MatchedPoleZeroAtOrigin`

Matched pole/zero `d2c` cannot map a discrete pole or zero at exactly
`z = 0`, because `ln(0)` diverges (mirrors the upstream Octave error).

###### `NonFinitePoleOrZero`

A matched-method pole or zero mapped to a non-finite value
(mirrors the upstream Octave error).

###### `InvalidPrewarpFrequency`

The prewarp frequency must satisfy `0 < w0 < pi / T` (below the
Nyquist angular frequency) for `tan(w0 T / 2)` to be positive and
finite.

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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ZDomainError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
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
### Re-exports

#### Re-export `ContinuousTransferFn`

```rust
pub use continuous_tf::ContinuousTransferFn;
```

#### Re-export `C2dMethod`

```rust
pub use conversion::C2dMethod;
```

#### Re-export `D2cMethod`

```rust
pub use conversion::D2cMethod;
```

#### Re-export `DiscreteTransferFn`

```rust
pub use discrete_tf::DiscreteTransferFn;
```

## Module `stable`

```rust
pub mod stable { /* ... */ }
```

### Modules

## Module `prelude`

```rust
pub mod prelude { /* ... */ }
```

