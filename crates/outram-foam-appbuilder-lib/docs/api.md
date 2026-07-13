# Crate Documentation

**Version:** 0.1.0

**Format Version:** 60

# Module `outram_foam_appbuilder_lib`

**This is OUTRAM PARK's independent Rust translation of selected
OpenFOAM® solver-application algorithms — it is not the official
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

#### Enum `AppBuilderError`

```rust
pub enum AppBuilderError {
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    Parse {
        file: String,
        line: usize,
        msg: String,
    },
    MissingKey {
        key: &'static str,
        dict: &'static str,
    },
    Diverged {
        iter: usize,
        residual: f64,
    },
    TimeLimitReached {
        t: f64,
    },
}
```

##### Variants

###### `Io`

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `path` | `std::path::PathBuf` |  |
| `source` | `std::io::Error` |  |

###### `Parse`

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `file` | `String` |  |
| `line` | `usize` |  |
| `msg` | `String` |  |

###### `MissingKey`

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `&'static str` |  |
| `dict` | `&'static str` |  |

###### `Diverged`

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iter` | `usize` |  |
| `residual` | `f64` |  |

###### `TimeLimitReached`

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t` | `f64` |  |

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
## Module `io`

input and output

```rust
pub mod io { /* ... */ }
```

### Modules

## Module `control_dict`

```rust
pub mod control_dict { /* ... */ }
```

### Types

#### Struct `ControlDict`

Parsed contents of an OpenFOAM `system/controlDict` file.

```rust
pub struct ControlDict {
    pub application: String,
    pub start: StartControl,
    pub stop: StopControl,
    pub delta_t: f64,
    pub write_control: WriteControl,
    pub write_interval: f64,
    pub purge_write: usize,
    pub write_format: WriteFormat,
    pub write_precision: usize,
    pub run_time_modifiable: bool,
    pub adjust_time_step: bool,
    pub max_co: f64,
    pub max_delta_t: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `application` | `String` |  |
| `start` | `StartControl` |  |
| `stop` | `StopControl` |  |
| `delta_t` | `f64` |  |
| `write_control` | `WriteControl` |  |
| `write_interval` | `f64` |  |
| `purge_write` | `usize` |  |
| `write_format` | `WriteFormat` |  |
| `write_precision` | `usize` |  |
| `run_time_modifiable` | `bool` |  |
| `adjust_time_step` | `bool` |  |
| `max_co` | `f64` |  |
| `max_delta_t` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn read(path: &Path) -> Result<Self, AppBuilderError> { /* ... */ }
  ```
  Parse a `controlDict` file from disk.

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
    fn clone(self: &Self) -> ControlDict { /* ... */ }
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
#### Enum `StartControl`

```rust
pub enum StartControl {
    StartTime(f64),
    LatestTime,
    FirstTime,
}
```

##### Variants

###### `StartTime`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `LatestTime`

###### `FirstTime`

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
    fn clone(self: &Self) -> StartControl { /* ... */ }
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
    fn eq(self: &Self, other: &StartControl) -> bool { /* ... */ }
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
#### Enum `StopControl`

```rust
pub enum StopControl {
    EndTime(f64),
    WriteNow,
    NoWriteNow,
    NextWrite,
}
```

##### Variants

###### `EndTime`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `WriteNow`

###### `NoWriteNow`

###### `NextWrite`

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
    fn clone(self: &Self) -> StopControl { /* ... */ }
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
    fn eq(self: &Self, other: &StopControl) -> bool { /* ... */ }
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
#### Enum `WriteControl`

```rust
pub enum WriteControl {
    TimeStep(usize),
    RunTime(f64),
    AdjustableRunTime(f64),
    CpuTime(f64),
    ClockTime(f64),
}
```

##### Variants

###### `TimeStep`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `RunTime`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `AdjustableRunTime`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `CpuTime`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `ClockTime`

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> WriteControl { /* ... */ }
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
    fn eq(self: &Self, other: &WriteControl) -> bool { /* ... */ }
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
#### Enum `WriteFormat`

```rust
pub enum WriteFormat {
    Ascii,
    Binary,
}
```

##### Variants

###### `Ascii`

###### `Binary`

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
    fn clone(self: &Self) -> WriteFormat { /* ... */ }
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
    fn eq(self: &Self, other: &WriteFormat) -> bool { /* ... */ }
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
## Module `field_reader`

Readers for OpenFOAM field files (volScalarField, volVectorField).

Supports both `uniform` and `nonuniform List<...>` internal fields.

```rust
pub mod field_reader { /* ... */ }
```

### Functions

#### Function `read_vol_vector_field`

Read the `internalField` of a `volVectorField` file.

Handles:
- `internalField uniform (x y z);`
- `internalField nonuniform List<vector> N\n(\n(x y z)\n...\n);`

```rust
pub fn read_vol_vector_field(path: &std::path::Path, n_cells: usize) -> Result<Vec<outram_foam_basic_lib::prelude::Vector3>, crate::error::AppBuilderError> { /* ... */ }
```

#### Function `read_vol_scalar_field`

Read the `internalField` of a `volScalarField` file.

Handles:
- `internalField uniform <value>;`
- `internalField nonuniform List<scalar> N\n(\n<value>\n...\n);`

```rust
pub fn read_vol_scalar_field(path: &std::path::Path, n_cells: usize) -> Result<Vec<f64>, crate::error::AppBuilderError> { /* ... */ }
```

#### Function `read_vol_scalar_field_full`

Read a complete `volScalarField` (internal + boundary) bound to `mesh`.

```rust
pub fn read_vol_scalar_field_full(path: &std::path::Path, mesh: &std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>) -> Result<outram_foam_basic_lib::prelude::VolScalarField, crate::error::AppBuilderError> { /* ... */ }
```

#### Function `read_vol_vector_field_full`

Read a complete `volVectorField` (internal + boundary) bound to `mesh`.

```rust
pub fn read_vol_vector_field_full(path: &std::path::Path, mesh: &std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>) -> Result<outram_foam_basic_lib::prelude::VolVectorField, crate::error::AppBuilderError> { /* ... */ }
```

## Module `fv_schemes`

```rust
pub mod fv_schemes { /* ... */ }
```

### Types

#### Struct `FvSchemes`

Parsed `system/fvSchemes` — numerical scheme selection for each operator.

```rust
pub struct FvSchemes {
    pub ddt: DdtScheme,
    pub default_grad: GradScheme,
    pub default_div: DivScheme,
    pub default_laplacian: LaplacianScheme,
    pub default_sn_grad: SnGradScheme,
    pub default_interpolation: InterpolationScheme,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ddt` | `DdtScheme` |  |
| `default_grad` | `GradScheme` |  |
| `default_div` | `DivScheme` |  |
| `default_laplacian` | `LaplacianScheme` |  |
| `default_sn_grad` | `SnGradScheme` |  |
| `default_interpolation` | `InterpolationScheme` |  |

##### Implementations

###### Methods

- ```rust
  pub fn read(path: &Path) -> Result<Self, AppBuilderError> { /* ... */ }
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
    fn clone(self: &Self) -> FvSchemes { /* ... */ }
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
#### Enum `DdtScheme`

Time-stepping scheme (ddtSchemes).

```rust
pub enum DdtScheme {
    Euler,
    Backward,
    CrankNicolson(f64),
    LocalEuler,
    SteadyState,
}
```

##### Variants

###### `Euler`

###### `Backward`

###### `CrankNicolson`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `LocalEuler`

###### `SteadyState`

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
    fn clone(self: &Self) -> DdtScheme { /* ... */ }
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
    fn eq(self: &Self, other: &DdtScheme) -> bool { /* ... */ }
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
#### Enum `GradScheme`

Gradient scheme (gradSchemes).

```rust
pub enum GradScheme {
    GaussLinear,
    LeastSquares,
    FourthOrder,
}
```

##### Variants

###### `GaussLinear`

###### `LeastSquares`

###### `FourthOrder`

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
    fn clone(self: &Self) -> GradScheme { /* ... */ }
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
    fn eq(self: &Self, other: &GradScheme) -> bool { /* ... */ }
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
#### Enum `DivScheme`

Divergence / convection scheme (divSchemes).

```rust
pub enum DivScheme {
    GaussLinear,
    GaussUpwind,
    GaussLinearUpwind(String),
    GaussVanLeer,
    GaussMUSCL,
    GaussLimitedLinear(f64),
}
```

##### Variants

###### `GaussLinear`

###### `GaussUpwind`

###### `GaussLinearUpwind`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `GaussVanLeer`

###### `GaussMUSCL`

###### `GaussLimitedLinear`

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> DivScheme { /* ... */ }
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
    fn eq(self: &Self, other: &DivScheme) -> bool { /* ... */ }
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
#### Enum `LaplacianScheme`

Laplacian scheme (laplacianSchemes).

```rust
pub enum LaplacianScheme {
    GaussLinearCorrected,
    GaussLinearUncorrected,
    GaussLinearLimited(f64),
}
```

##### Variants

###### `GaussLinearCorrected`

###### `GaussLinearUncorrected`

###### `GaussLinearLimited`

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LaplacianScheme { /* ... */ }
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
    fn eq(self: &Self, other: &LaplacianScheme) -> bool { /* ... */ }
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
#### Enum `SnGradScheme`

Surface-normal gradient scheme (snGradSchemes).

```rust
pub enum SnGradScheme {
    Corrected,
    Uncorrected,
    Limited(f64),
}
```

##### Variants

###### `Corrected`

###### `Uncorrected`

###### `Limited`

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SnGradScheme { /* ... */ }
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
    fn eq(self: &Self, other: &SnGradScheme) -> bool { /* ... */ }
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
#### Enum `InterpolationScheme`

Face interpolation scheme (interpolationSchemes).

```rust
pub enum InterpolationScheme {
    Linear,
    Upwind(String),
    Harmonic,
}
```

##### Variants

###### `Linear`

###### `Upwind`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Harmonic`

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
    fn clone(self: &Self) -> InterpolationScheme { /* ... */ }
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
    fn eq(self: &Self, other: &InterpolationScheme) -> bool { /* ... */ }
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
## Module `fv_solution`

```rust
pub mod fv_solution { /* ... */ }
```

### Types

#### Struct `FvSolution`

Parsed `system/fvSolution`.

```rust
pub struct FvSolution {
    pub solvers: std::collections::HashMap<String, LinearSolverConfig>,
    pub pimple: PimpleControl,
    pub relaxation_fields: std::collections::HashMap<String, f64>,
    pub relaxation_equations: std::collections::HashMap<String, f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `solvers` | `std::collections::HashMap<String, LinearSolverConfig>` | Per-field linear solver configuration, keyed by field name. |
| `pimple` | `PimpleControl` | PIMPLE / PISO outer-loop control parameters. |
| `relaxation_fields` | `std::collections::HashMap<String, f64>` | Under-relaxation factors, keyed by field name. |
| `relaxation_equations` | `std::collections::HashMap<String, f64>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn read(path: &Path) -> Result<Self, AppBuilderError> { /* ... */ }
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
    fn clone(self: &Self) -> FvSolution { /* ... */ }
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
#### Struct `LinearSolverConfig`

Linear solver configuration for a single field (fvSolution::solvers.<field>).

```rust
pub struct LinearSolverConfig {
    pub solver: LinearSolverType,
    pub preconditioner: Option<String>,
    pub tolerance: f64,
    pub rel_tol: f64,
    pub max_iter: usize,
    pub smoother: Option<String>,
    pub n_sweep: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `solver` | `LinearSolverType` |  |
| `preconditioner` | `Option<String>` |  |
| `tolerance` | `f64` |  |
| `rel_tol` | `f64` |  |
| `max_iter` | `usize` |  |
| `smoother` | `Option<String>` |  |
| `n_sweep` | `usize` |  |

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
    fn clone(self: &Self) -> LinearSolverConfig { /* ... */ }
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
#### Enum `LinearSolverType`

Linear solver algorithm.

```rust
pub enum LinearSolverType {
    Pcg,
    PbicgStab,
    Gamg,
    GaussSeidel,
    Diagonal,
    SmoothSolver,
}
```

##### Variants

###### `Pcg`

Preconditioned Conjugate Gradient (symmetric systems, e.g. pressure).

###### `PbicgStab`

Preconditioned Bi-Conjugate Gradient Stabilised (asymmetric, e.g. U, T).

###### `Gamg`

Generalised Algebraic Multi-Grid (large pressure systems).

###### `GaussSeidel`

Gauss-Seidel (smoother or stand-alone for simple problems).

###### `Diagonal`

Diagonal preconditioner only.

###### `SmoothSolver`

Smooth solver (iterative, for symmetric).

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
    fn clone(self: &Self) -> LinearSolverType { /* ... */ }
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
    fn eq(self: &Self, other: &LinearSolverType) -> bool { /* ... */ }
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
#### Struct `PimpleControl`

PIMPLE / PISO outer-corrector loop control.

```rust
pub struct PimpleControl {
    pub n_outer_correctors: usize,
    pub n_correctors: usize,
    pub n_non_orthogonal_correctors: usize,
    pub consistent: bool,
    pub correct_phi: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_outer_correctors` | `usize` | Number of outer PIMPLE correctors (1 = PISO). |
| `n_correctors` | `usize` | Number of inner pressure correctors per outer corrector. |
| `n_non_orthogonal_correctors` | `usize` | Non-orthogonal correctors for mesh skewness compensation. |
| `consistent` | `bool` | Use consistent formulation (avoids rAU cell-size dependency). |
| `correct_phi` | `bool` | Turbulence corrector at end of each outer loop. |

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
    fn clone(self: &Self) -> PimpleControl { /* ... */ }
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
## Module `output`

```rust
pub mod output { /* ... */ }
```

### Functions

#### Function `write_scalar_field`

Write a scalar field to `<time_dir>/<field_name>` in OpenFOAM ASCII format.

The output follows the standard OpenFOAM field file layout:
```text
FoamFile { version 2.0; format ascii; class volScalarField; object p; }
dimensions [kg m-1 s-2];
internalField nonuniform List<scalar> N ( v0 v1 … vN-1 );
boundaryField { … }
```

```rust
pub fn write_scalar_field(time_dir: &std::path::Path, field: &outram_foam_basic_lib::prelude::VolScalarField, dimensions: &str) -> Result<(), crate::error::AppBuilderError> { /* ... */ }
```

#### Function `write_vector_field`

Write a vector field to `<time_dir>/<field_name>` in OpenFOAM ASCII format.

```rust
pub fn write_vector_field(time_dir: &std::path::Path, field: &outram_foam_basic_lib::prelude::VolVectorField, dimensions: &str) -> Result<(), crate::error::AppBuilderError> { /* ... */ }
```

#### Function `write_vtk`

Write a legacy VTK unstructured grid file for ParaView.

Includes mesh geometry and all provided scalar fields.

```rust
pub fn write_vtk(out_path: &std::path::Path, mesh_points: &[[f64; 3]], scalar_fields: &[(&str, &outram_foam_basic_lib::prelude::VolScalarField)]) -> Result<(), crate::error::AppBuilderError> { /* ... */ }
```

## Module `poly_mesh`

Reader for OpenFOAM `constant/polyMesh/` directories.

## Why a custom parser, not the OpenFOAM C++ reader?

OpenFOAM's own file reader (`ISstream`, `IOobject`, `IOdictionary`) is a
deeply-templated C++ library with a runtime type registry.  There is no
stable C API to wrap with bindgen, and the template depth makes even
generating Rust FFI shims impractical.  No mature Rust crate exists for
OpenFOAM ASCII format either.  The ASCII format itself is straightforward
enough (FoamFile header + N-element list) that a purpose-built 400-line
Rust parser is far simpler than attempting C++ interop.

```rust
pub mod poly_mesh { /* ... */ }
```

### Functions

#### Function `read_poly_mesh`

Read an OpenFOAM `constant/polyMesh/` directory and return an `FvMesh`.

Reads `points`, `faces`, `owner`, `neighbour`, and `boundary`, then
derives all geometric quantities (face centres, face area vectors, cell
centres, cell volumes) via pyramid decomposition — the same algorithm used
by `primitiveMesh::makeFaceCentresAndAreas` and
`primitiveMesh::makeCellCentresAndVols` in OpenFOAM's C++ source.

```rust
pub fn read_poly_mesh(poly_mesh_dir: &std::path::Path) -> Result<std::sync::Arc<outram_foam_basic_lib::prelude::FvMesh>, crate::error::AppBuilderError> { /* ... */ }
```

#### Function `parse_points`

Parse the `points` file.  Each entry is `(x y z)`.

```rust
pub fn parse_points(text: &str, file: &str) -> Result<Vec<[f64; 3]>, crate::error::AppBuilderError> { /* ... */ }
```

#### Function `parse_faces`

Parse the `faces` file.  Each entry is `N(v0 v1 … vN-1)`.

The leading digit(s) before `(` give the vertex count; they are redundant
with the actual number of tokens inside `( )` but we verify consistency.

```rust
pub fn parse_faces(text: &str, file: &str) -> Result<Vec<Vec<usize>>, crate::error::AppBuilderError> { /* ... */ }
```

#### Function `parse_index_list`

Parse the `owner` or `neighbour` file: one non-negative integer per entry.

```rust
pub fn parse_index_list(text: &str, file: &str) -> Result<Vec<usize>, crate::error::AppBuilderError> { /* ... */ }
```

#### Function `parse_boundary`

Parse the `boundary` file into a list of `BoundaryPatch`.

```rust
pub fn parse_boundary(text: &str, file: &str) -> Result<Vec<outram_foam_basic_lib::prelude::BoundaryPatch>, crate::error::AppBuilderError> { /* ... */ }
```

## Module `prelude`

for users to import

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `AppBuilderError`

```rust
pub use crate::error::AppBuilderError;
```

#### Re-export `ControlDict`

```rust
pub use crate::io::control_dict::ControlDict;
```

#### Re-export `StartControl`

```rust
pub use crate::io::control_dict::StartControl;
```

#### Re-export `StopControl`

```rust
pub use crate::io::control_dict::StopControl;
```

#### Re-export `WriteControl`

```rust
pub use crate::io::control_dict::WriteControl;
```

#### Re-export `WriteFormat`

```rust
pub use crate::io::control_dict::WriteFormat;
```

#### Re-export `DdtScheme`

```rust
pub use crate::io::fv_schemes::DdtScheme;
```

#### Re-export `DivScheme`

```rust
pub use crate::io::fv_schemes::DivScheme;
```

#### Re-export `FvSchemes`

```rust
pub use crate::io::fv_schemes::FvSchemes;
```

#### Re-export `GradScheme`

```rust
pub use crate::io::fv_schemes::GradScheme;
```

#### Re-export `LaplacianScheme`

```rust
pub use crate::io::fv_schemes::LaplacianScheme;
```

#### Re-export `SnGradScheme`

```rust
pub use crate::io::fv_schemes::SnGradScheme;
```

#### Re-export `FvSolution`

```rust
pub use crate::io::fv_solution::FvSolution;
```

#### Re-export `LinearSolverConfig`

```rust
pub use crate::io::fv_solution::LinearSolverConfig;
```

#### Re-export `LinearSolverType`

```rust
pub use crate::io::fv_solution::LinearSolverType;
```

#### Re-export `PimpleControl`

```rust
pub use crate::io::fv_solution::PimpleControl;
```

#### Re-export `write_scalar_field`

```rust
pub use crate::io::output::write_scalar_field;
```

#### Re-export `read_poly_mesh`

```rust
pub use crate::io::poly_mesh::read_poly_mesh;
```

#### Re-export `HrmFoam`

```rust
pub use crate::solvers::hrm_foam::HrmFoam;
```

#### Re-export `HrmModelConfig`

```rust
pub use crate::solvers::hrm_foam::HrmModelConfig;
```

#### Re-export `PimpleFoam`

```rust
pub use crate::solvers::pimple_foam::PimpleFoam;
```

#### Re-export `PressureSolver`

```rust
pub use crate::solvers::pimple_foam::PressureSolver;
```

#### Re-export `RhoCentralFoam`

```rust
pub use crate::solvers::rho_central_foam::RhoCentralFoam;
```

#### Re-export `RhoPimpleFoam`

```rust
pub use crate::solvers::rho_pimple_foam::RhoPimpleFoam;
```

#### Re-export `SonicFoam`

```rust
pub use crate::solvers::sonic_foam::SonicFoam;
```

## Module `solvers`

your solvers are here!

```rust
pub mod solvers { /* ... */ }
```

### Modules

## Module `hrm_foam`

```rust
pub mod hrm_foam { /* ... */ }
```

### Types

#### Struct `HrmModelConfig`

Runtime controls translated from HRMFoam's thermophysical/SigmaY dictionaries.

```rust
pub struct HrmModelConfig {
    pub theta_0: f64,
    pub pressure_undershoot_exp: f64,
    pub liquid_fraction_exp: f64,
    pub theta_floor: f64,
    pub rho_min: f64,
    pub solve_gas_fraction: bool,
    pub adiabatic: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `theta_0` | `f64` | Relaxation time pre-factor θ₀ [s]. |
| `pressure_undershoot_exp` | `f64` | Pressure undershoot exponent in the Downar-Zapolski correlation. |
| `liquid_fraction_exp` | `f64` | Liquid fraction exponent in the Downar-Zapolski correlation. |
| `theta_floor` | `f64` | Lower bound on relaxation time [s]. |
| `rho_min` | `f64` | Lower bound on density [kg/m³]. |
| `solve_gas_fraction` | `bool` | Enable the non-condensable gas mass-fraction equation. |
| `adiabatic` | `bool` | Skip the enthalpy equation, matching HRMFoam's `adiabatic` switch. |

##### Implementations

###### Methods

- ```rust
  pub fn relaxation_time(self: Self, psi: f64, x: f64) -> f64 { /* ... */ }
  ```
  Downar-Zapolski relaxation time τ for the configured constants.

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
    fn clone(self: &Self) -> HrmModelConfig { /* ... */ }
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
#### Struct `HrmFoam`

Homogeneous Relaxation Model (HRM) two-phase flashing flow solver.

The HRM assumes mechanical and thermal equilibrium between phases but
allows thermodynamic non-equilibrium via a finite relaxation time τ
toward the equilibrium dryness fraction x_eq(p, h).

Downar-Zapolski (1996) relaxation time:
  τ = θ₀ · ψ^a · (1 − x)^b
where ψ = (p_sat − p) / p_sat is the pressure undershoot (dimensionless).

Transport equations:
  ∂ρ/∂t  + ∇·(ρU)     = 0
  ∂(ρU)/∂t + ∇·(ρUU)  = −∇p
  ∂(ρh)/∂t + ∇·(ρhU)  = dp/dt
  ∂(ρx)/∂t + ∇·(ρxU)  = ρ · (x_eq − x) / τ   ← HRM relaxation source
  ∂(ρy)/∂t + ∇·(ρyU)  = ∇·(D∇y)              ← gas mass fraction

The equilibrium quality x_eq(p, h) is supplied externally (e.g. via
TAMPINES steam tables).  Call `set_x_eq` each time step before `step()`.

C++ source: `../HRMFoam/` (sibling directory, outside this workspace)

```rust
pub struct HrmFoam {
    pub mesh: std::sync::Arc<FvMesh>,
    pub control: crate::io::control_dict::ControlDict,
    pub schemes: crate::io::fv_schemes::FvSchemes,
    pub solution: crate::io::fv_solution::FvSolution,
    pub u: VolVectorField,
    pub p: VolScalarField,
    pub rho: VolScalarField,
    pub h: VolScalarField,
    pub x: VolScalarField,
    pub y: VolScalarField,
    pub x_eq: VolScalarField,
    pub mu: VolScalarField,
    pub alpha_h: VolScalarField,
    pub gas_diffusivity: VolScalarField,
    pub p_sat: VolScalarField,
    pub phi: SurfaceScalarField,
    pub model: HrmModelConfig,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<FvMesh>` |  |
| `control` | `crate::io::control_dict::ControlDict` |  |
| `schemes` | `crate::io::fv_schemes::FvSchemes` |  |
| `solution` | `crate::io::fv_solution::FvSolution` |  |
| `u` | `VolVectorField` | Velocity field [m/s] |
| `p` | `VolScalarField` | Pressure [Pa] |
| `rho` | `VolScalarField` | Mixture density [kg/m³] |
| `h` | `VolScalarField` | Mixture specific enthalpy [J/kg] |
| `x` | `VolScalarField` | Vapour dryness fraction x ∈ [0, 1] |
| `y` | `VolScalarField` | Non-condensable gas mass fraction y ∈ [0, 1] |
| `x_eq` | `VolScalarField` | Equilibrium quality x_eq(p, h) — updated by caller each time step |
| `mu` | `VolScalarField` | Dynamic viscosity μ [Pa·s] |
| `alpha_h` | `VolScalarField` | Effective thermal diffusivity αh [kg/(m·s)] |
| `gas_diffusivity` | `VolScalarField` | Effective gas diffusivity D [kg/(m·s)] |
| `p_sat` | `VolScalarField` | Saturation pressure p_sat [Pa] — updated by caller each time step |
| `phi` | `SurfaceScalarField` | Mass flux φ = ρ U·Sf [kg/s] |
| `model` | `HrmModelConfig` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```

- ```rust
  pub fn with_model_config(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution, model: HrmModelConfig) -> Self { /* ... */ }
  ```

- ```rust
  pub fn relaxation_time(psi: f64, x: f64) -> f64 { /* ... */ }
  ```
  Downar-Zapolski relaxation time τ at a single point.

- ```rust
  pub fn relaxation_time_with_config(self: &Self, psi: f64, x: f64) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance one time step.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
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

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `THETA_0`

Relaxation time pre-factor θ₀ [s]

```rust
pub const THETA_0: f64 = 3.84e-7;
```

#### Constant `DZ_A`

Pressure undershoot exponent a

```rust
pub const DZ_A: f64 = -0.54;
```

#### Constant `DZ_B`

Quality exponent b

```rust
pub const DZ_B: f64 = -0.05;
```

## Module `pimple_foam`

# pimpleFoam / icoFoam — incompressible PISO/PIMPLE solver

Rust port of OpenFOAM's incompressible PISO/PIMPLE algorithm. With
`nOuterCorrectors = 1` (no SIMPLE outer loop) pimpleFoam reduces to **pure
PISO**, i.e. it is structurally identical to **icoFoam**; the lid-driven
cavity tutorial (`tutorials/pimple_foam_cavity.rs`) validates this port
against icoFoam-generated reference fields.

## Kinematic pressure and the pressure reference (closed-domain note)

Like icoFoam and incompressible pimpleFoam, `p` here is **kinematic
pressure** `p/ρ` with units m²/s², *not* Pa. The momentum equation carries
`−∇p` (kinematic) directly, and density never appears.

Both OpenFOAM solvers pin a pressure **reference cell** for a closed domain.
The cavity has walls on every boundary, so the pressure boundary condition is
zero-gradient everywhere → the pressure Poisson equation is pure-Neumann and
singular (its solution is unique only up to an additive constant, and the net
boundary flux must vanish for a solution to exist at all). OpenFOAM fixes the
constant with `setRefCell`:

```text
// icoFoam/createFields.H
label  pRefCell  = 0;
scalar pRefValue = 0.0;
setRefCell(p, mesh.solutionDict().subDict("PISO"), pRefCell, pRefValue);
//   ... later, in the pressure corrector:
pEqn.setReference(pRefCell, pRefValue);

// pimpleFoam/createFields.H — IDENTICAL mechanism
setRefCell(p, pimple.dict(), pRefCell, pRefValue);
```

So yes — pimpleFoam does exactly what icoFoam does here. This port mirrors it
with `p_eqn.set_reference(0, 0.0)` in the inner corrector loop.

## Original OpenFOAM source (icoFoam.C — the clean PISO reference)

```text
// Momentum predictor
fvVectorMatrix UEqn
(
    fvm::ddt(U)
  + fvm::div(phi, U)
  - fvm::laplacian(nu, U)        // NOTE the minus sign — see below
);
if (piso.momentumPredictor())
{
    solve(UEqn == -fvc::grad(p));
}

// --- PISO loop
while (piso.correct())   // (5) the WHOLE block re-runs each corrector,
{                        //     re-evaluating UEqn.H() from the latest U
    volScalarField rAU(1.0/UEqn.A());
    volVectorField HbyA(constrainHbyA(rAU*UEqn.H(), U, p));     // (3)
    surfaceScalarField phiHbyA
    (
        "phiHbyA",
        fvc::flux(HbyA)
      + fvc::interpolate(rAU)*fvc::ddtCorr(U, phi)              // ddtCorr
    );
    adjustPhi(phiHbyA, U, p);                                  // (3)
    constrainPressure(p, U, phiHbyA, rAU);
    while (piso.correctNonOrthogonal())
    {
        fvScalarMatrix pEqn
        (
            fvm::laplacian(rAU, p) == fvc::div(phiHbyA)         // (2)
        );
        pEqn.setReference(pRefCell, pRefValue);
        pEqn.solve(...);                                       // (4) GAMG/PCG
        if (piso.finalNonOrthogonalIter())
            phi = phiHbyA - pEqn.flux();
    }
    U = HbyA - rAU*fvc::grad(p);
    U.correctBoundaryConditions();                            // (1)
}
```

## How this port differs from the original, and why

**Root cause of the sign flips (changes 0a/0b): `outram-foam-basic-lib`'s
`fvm::laplacian` uses the *opposite* diagonal-sign convention to OpenFOAM's.**
OpenFOAM assembles `fvm::laplacian(Γ, φ)` with a *negative* diagonal
(`diag = −Σcoeff`), i.e. the matrix represents `+∇·(Γ∇φ)` exactly as written
in the equation. This port assembles it *positive-definite*
(`diag = +Σcoeff`), i.e. its matrix represents `−∇·(Γ∇φ)`. So the port's
Laplacian matrix is the **negation** of OpenFOAM's. Every sign change below
follows from this one fact; the discretised physics is identical.

0a. **Momentum viscous term: `+ fvm::laplacian_vec` (OpenFOAM: `−`).**
    The momentum LHS viscous term is `−ν∇²U = −∇·(ν∇U)`. OpenFOAM writes it as
    `− fvm::laplacian(nu, U)` because its Laplacian matrix is `+∇·(ν∇U)`.
    This port's Laplacian is already `−∇·(ν∇U)`, so it is **added**.
    Subtracting it (copying OpenFOAM's sign literally) negates the diffusion
    diagonal: the matrix diagonal goes negative (V/dt − Σcoeff < 0), `rAU =
    V/A` explodes to ~1e23, and the very first solve produces ~1e130. This
    was the first bug found.

0b. **Pressure source: negated divergence (OpenFOAM: `== fvc::div(phiHbyA)`).**
    OpenFOAM solves `fvm::laplacian(rAU, p) == fvc::div(phiHbyA)` with its
    negative-diagonal Laplacian `L_OF`. This port's Laplacian is `L = −L_OF`,
    so the *same* equation is `L·p = −div(phiHbyA)`. Equivalently: with the
    positive-definite operator the discrete divergence of the corrector flux
    `−rAUf·snGrad(p)` is `−(L·p)`, and zeroing the corrected divergence
    requires `L·p = −div(phiHbyA)`. Using `+div` flips the sign of `p`, so the
    corrector pumps divergence *in* and the run blows up over a few steps.

1. **`correct_bcs` / `correct_bcs_vec` (OpenFOAM: `U.correctBoundaryConditions()`).**
   OpenFOAM fields carry their boundary-condition objects, so re-evaluating
   them is a method call. In this port `solve()` and field arithmetic rebuild
   output fields with *zero-gradient* boundaries — the prescribed BC *type*
   (e.g. the moving-wall lid) is lost. The BC template is therefore captured
   at the top of each step and re-applied after every field update, exactly
   where OpenFOAM calls `correctBoundaryConditions()`.

2. **Pressure reference** — `p_eqn.set_reference(0, 0.0)` = `pEqn.setReference(
   pRefCell, pRefValue)` (see the closed-domain note above). Unchanged in
   intent from OpenFOAM.

3. **constrainHbyA / adjustPhi (boundary flux of phiHbyA).**
   OpenFOAM wraps `HbyA` in `constrainHbyA(...)` and calls `adjustPhi(...)` so
   that on fixed-velocity walls the boundary flux of `phiHbyA` is the
   *prescribed* `U_BC·Sf` (= 0 through a no-penetration wall). This port
   originally took `fvc::flux` of the zero-gradient `HbyA` extrapolation,
   which leaks a spurious flux through the walls, breaks the closed-domain
   compatibility condition `Σ source = 0`, and makes the pinned Poisson solve
   ramp the pressure ~6× every step. The fix sets the boundary flux to
   `U_BC·Sf` — the constrainHbyA equivalent for this BC set.

4. **Pressure linear solver: PCG (`solve_cg`), not Gauss-Seidel.**
   OpenFOAM solves the pressure with GAMG/PCG (chosen in `fvSolution`); it
   would never use Gauss-Seidel on a Poisson system. This port's
   `FvMatrix::solve` defaults to Gauss-Seidel, which needed ~22 000 iterations
   (and often did not converge within the cap) on the 400-cell cavity. The
   pressure matrix is symmetric SPD, so it is solved with `solve_cg` (PCG)
   instead — ~130 iterations, ~170× faster. A purely-performance change, but
   a *correctness* one in practice: an under-solved pEqn leaves residual
   divergence that accumulates and destabilises the run.

5. **PISO corrector loop — `H(U)` re-evaluated every pass (the stability fix).**
   OpenFOAM's `while (piso.correct())` re-runs the *entire* `rAU`/`HbyA`/
   `phiHbyA`/`pEqn`/`U`-update sequence each corrector, so `UEqn.H()` is
   recomputed from the velocity updated by the previous corrector — that is
   the iteration that converges the pressure–velocity coupling. An earlier
   version of this port computed `HbyA` once and merely re-solved the *same*
   pressure system `nCorrectors` times (updating neither `H(U)` nor `U`
   between passes), which collapses to a single corrector and capped stability
   at Co ≈ 0.1. With the loop restructured to match OpenFOAM, the cavity is
   stable at icoFoam's `dt = 5e-3` (Co ≈ 0.85).

6. **`fvc::ddtCorr` Rhie–Chow flux correction — now included.**
   `phiHbyA += fvc::interpolate(rAU)*fvc::ddtCorr(U, phi)`, with OpenFOAM's
   `fvcDdtPhiCoeff` limiter (`coeff = 1 − min(|phiCorr|/(|phi|+SMALL), 1)`).
   It couples the face flux to its own old-time value to suppress
   pressure–velocity (checkerboard) decoupling. `ddtCorr` uses the time-old
   `U`/`phi` (constant across the inner correctors). See
   `outram_foam_basic_lib::fv_operators::fvc::ddt_corr`.

```rust
pub mod pimple_foam { /* ... */ }
```

### Types

#### Enum `PressureSolver`

Linear solver used for the pressure Poisson equation.

The pressure equation is symmetric SPD and elliptic. Both options warm-start
from the previous time step's pressure field:

- [`Pcg`](PressureSolver::Pcg) — DIC-preconditioned conjugate gradient.
  Iteration count grows with the mesh (∝ √κ), but each iteration is cheap.
- [`Gamg`](PressureSolver::Gamg) — algebraic multigrid. Near
  mesh-independent (a few V-cycles); the better choice on fine meshes.

```rust
pub enum PressureSolver {
    Pcg,
    Gamg,
}
```

##### Variants

###### `Pcg`

DIC-preconditioned conjugate gradient (`FvMatrix::solve_cg_with_guess`).

###### `Gamg`

Algebraic multigrid (`FvMatrix::solve_gamg_with_guess`).

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
    fn clone(self: &Self) -> PressureSolver { /* ... */ }
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
    fn default() -> PressureSolver { /* ... */ }
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
    fn eq(self: &Self, other: &PressureSolver) -> bool { /* ... */ }
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
#### Struct `PimpleFoam`

Incompressible transient PIMPLE/PISO solver.

Solves:
  ∂U/∂t + ∇·(UU) − ν∇²U = −∇p    (p here is kinematic: p/ρ, units m²/s²)
  ∇·U = 0

Outer PIMPLE loop → momentum predictor → inner PISO pressure correctors.

C++ solver: `applications/solvers/incompressible/pimpleFoam/` (and the
equivalent `icoFoam` for `nOuterCorrectors = 1`).

See the **module-level documentation** for the kinematic-pressure /
pressure-reference discussion, the original OpenFOAM source, and a
point-by-point justification of where (and why) this port's signs and
solver choices differ from the C++ original.

```rust
pub struct PimpleFoam {
    pub mesh: std::sync::Arc<FvMesh>,
    pub control: crate::io::control_dict::ControlDict,
    pub schemes: crate::io::fv_schemes::FvSchemes,
    pub solution: crate::io::fv_solution::FvSolution,
    pub u: VolVectorField,
    pub p: VolScalarField,
    pub phi: SurfaceScalarField,
    pub nu: VolScalarField,
    pub pressure_solver: PressureSolver,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<FvMesh>` |  |
| `control` | `crate::io::control_dict::ControlDict` |  |
| `schemes` | `crate::io::fv_schemes::FvSchemes` |  |
| `solution` | `crate::io::fv_solution::FvSolution` |  |
| `u` | `VolVectorField` | Velocity field [m/s] |
| `p` | `VolScalarField` | Kinematic pressure field p/ρ [m²/s²] |
| `phi` | `SurfaceScalarField` | Face volumetric flux φ = U·Sf [m³/s] |
| `nu` | `VolScalarField` | Kinematic viscosity ν [m²/s] |
| `pressure_solver` | `PressureSolver` | Linear solver for the pressure Poisson equation (default: PCG). |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the solution by one time step using the PIMPLE algorithm.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
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

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `rho_central_foam`

```rust
pub mod rho_central_foam { /* ... */ }
```

### Types

#### Struct `RhoCentralFoam`

Density-based central-upwind compressible solver — rhoCentralFoam.

Implements the **Kurganov-Noelle-Petrova (KNP)** scheme for the Euler
equations.  All convective terms are treated **explicitly** — no matrix
solve for transport.  Only suitable for time-accurate problems with
CFL ≤ 1.

Governing equations (conservation form):
  ∂W/∂t + ∇·F(W) = 0
  W = [ρ, ρU, ρE]ᵀ,  E = e + ½|U|²,  p = (γ−1)ρe  (calorically perfect gas)

KNP flux at face f (Kurganov, Noelle & Petrova, SIAM J. Sci. Comp. 2001):
  F_KNP = (a_R·F_L − a_L·F_R + a_L·a_R·(W_R − W_L)) / (a_R − a_L)
  a_L = min(U_n,L − c_L,  U_n,R − c_R,  0)
  a_R = max(U_n,L + c_L,  U_n,R + c_R,  0)

The left/right face states (`L`, `R`) are **2nd-order vanLeer MUSCL
reconstructions** of ρ, U and e — the owner-biased (`pos`) and
neighbour-biased (`neg`) face values from `fvc::reconstruct_pos_neg`,
matching OpenFOAM rhoCentralFoam's `interpolate(field, pos/neg)`. Using the
raw cell values instead would make the scheme first-order.

C++ solver: `applications/solvers/compressible/rhoCentralFoam/`

```rust
pub struct RhoCentralFoam {
    pub mesh: std::sync::Arc<FvMesh>,
    pub control: crate::io::control_dict::ControlDict,
    pub schemes: crate::io::fv_schemes::FvSchemes,
    pub solution: crate::io::fv_solution::FvSolution,
    pub u: VolVectorField,
    pub p: VolScalarField,
    pub rho: VolScalarField,
    pub e: VolScalarField,
    pub psi_limit: f64,
    pub phi: SurfaceScalarField,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<FvMesh>` |  |
| `control` | `crate::io::control_dict::ControlDict` |  |
| `schemes` | `crate::io::fv_schemes::FvSchemes` |  |
| `solution` | `crate::io::fv_solution::FvSolution` |  |
| `u` | `VolVectorField` | Velocity field [m/s] |
| `p` | `VolScalarField` | Pressure [Pa] |
| `rho` | `VolScalarField` | Density [kg/m³] |
| `e` | `VolScalarField` | Specific internal energy e [J/kg] |
| `psi_limit` | `f64` | Co-volume limiter (unused for calorically-perfect gas; kept for API compatibility). |
| `phi` | `SurfaceScalarField` | Mass flux output [kg/s] |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  One explicit KNP time step.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
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

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `rho_pimple_foam`

```rust
pub mod rho_pimple_foam { /* ... */ }
```

### Types

#### Struct `RhoPimpleFoam`

Compressible transient PIMPLE solver — rhoPimpleFoam.

Solves:
  ∂ρ/∂t  + ∇·(ρU)     = 0          (continuity)
  ∂(ρU)/∂t + ∇·(ρUU)  = −∇p + ∇·τ  (momentum)
  ∂(ρh)/∂t + ∇·(ρUh)  = dp/dt      (energy, h-form, adiabatic closure)
  ρ = ψ·p                            (EOS approximation)

Pressure equation includes the compressibility term ψ·∂p/∂t so that the
system is consistent with the linearised continuity equation.

C++ solver: `applications/solvers/compressible/rhoPimpleFoam/`

```rust
pub struct RhoPimpleFoam {
    pub mesh: std::sync::Arc<FvMesh>,
    pub control: crate::io::control_dict::ControlDict,
    pub schemes: crate::io::fv_schemes::FvSchemes,
    pub solution: crate::io::fv_solution::FvSolution,
    pub u: VolVectorField,
    pub p: VolScalarField,
    pub rho: VolScalarField,
    pub t: VolScalarField,
    pub he: VolScalarField,
    pub mu: VolScalarField,
    pub alpha_h: VolScalarField,
    pub psi: VolScalarField,
    pub phi: SurfaceScalarField,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<FvMesh>` |  |
| `control` | `crate::io::control_dict::ControlDict` |  |
| `schemes` | `crate::io::fv_schemes::FvSchemes` |  |
| `solution` | `crate::io::fv_solution::FvSolution` |  |
| `u` | `VolVectorField` | Velocity field [m/s] |
| `p` | `VolScalarField` | Pressure field [Pa] |
| `rho` | `VolScalarField` | Density field [kg/m³] |
| `t` | `VolScalarField` | Temperature field [K] |
| `he` | `VolScalarField` | Specific enthalpy [J/kg] |
| `mu` | `VolScalarField` | Dynamic viscosity μ [Pa·s] |
| `alpha_h` | `VolScalarField` | Effective thermal diffusivity αh = κ/Cp [kg/(m·s)] |
| `psi` | `VolScalarField` | Compressibility ψ = ∂ρ/∂p|_T = ρ/p [s²/m²] |
| `phi` | `SurfaceScalarField` | Mass flux φ = ρ U·Sf [kg/s] |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance one time step with compressible PIMPLE.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
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

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `sonic_foam`

```rust
pub mod sonic_foam { /* ... */ }
```

### Types

#### Struct `SonicFoam`

Transonic/supersonic compressible solver — sonicFoam.

Uses the compressibility ψ = ρ/p as the primary thermodynamic closure.
The pressure equation is:

  ∂(ψp)/∂t + ∇·(ψ_d p) − ∇·(ρ·rAU·∇p) = 0

where ψ_d = ψ·U is the "density" face velocity field.  The `fvm::div`
implicit scalar-convection operator is not yet in this library, so the
convective term ∇·(ψ_d p) is treated explicitly via `fvc::div`.

C++ solver: `applications/solvers/compressible/sonicFoam/`

```rust
pub struct SonicFoam {
    pub mesh: std::sync::Arc<FvMesh>,
    pub control: crate::io::control_dict::ControlDict,
    pub schemes: crate::io::fv_schemes::FvSchemes,
    pub solution: crate::io::fv_solution::FvSolution,
    pub u: VolVectorField,
    pub p: VolScalarField,
    pub rho: VolScalarField,
    pub e: VolScalarField,
    pub psi: VolScalarField,
    pub mu: VolScalarField,
    pub phi: SurfaceScalarField,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<FvMesh>` |  |
| `control` | `crate::io::control_dict::ControlDict` |  |
| `schemes` | `crate::io::fv_schemes::FvSchemes` |  |
| `solution` | `crate::io::fv_solution::FvSolution` |  |
| `u` | `VolVectorField` | Velocity field [m/s] |
| `p` | `VolScalarField` | Pressure [Pa] |
| `rho` | `VolScalarField` | Density [kg/m³] |
| `e` | `VolScalarField` | Specific internal energy e [J/kg] |
| `psi` | `VolScalarField` | Compressibility ψ = ρ/p [s²/m²] |
| `mu` | `VolScalarField` | Dynamic viscosity μ [Pa·s] |
| `phi` | `SurfaceScalarField` | Mass flux φ = ρ U·Sf [kg/s] |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance one time step.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
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

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
