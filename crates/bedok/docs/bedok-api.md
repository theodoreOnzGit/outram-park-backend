# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `bedok`

# BEDOK — systems-level multiphysics coupling

Three-dimensional nodal-diffusion neutronics coupled to thermal hydraulics,
at the fidelity band **above one-dimensional neutronics and below CFD**.
For CFD-fidelity multiphysics coupling see GeN-Foam in
`outram-foam-appbuilder-lib`; for neutronics-and-nuclear-data integration
see `nee_soon`.

## Provenance

This crate is a Rust translation of a MATLAB implementation by **Than Yan
Ren** (Singapore Nuclear Research and Safety Institute). Permission to
translate and to publish the result as open source under OUTRAM PARK was
given by the author and approved at project-lead level; see
`docs/bedok-port-scoping.md` §6.

The original code was unfinished when it was handed over. Gaps are
translated **as they are** and marked in place; completing them is a
separate, separately documented step. See `docs/bedok-port-scoping.md` §1.0
for why that separation matters.

## The two paths

- [`reference`] — the faithful translation. Structure, iteration order and
  convergence logic follow the MATLAB line for line. Not idiomatic, not
  optimised, deliberately.
- [`substituted`] — the same physics rebuilt on OUTRAM PARK libraries. Every
  component here must reproduce [`reference`] on the benchmark suite before
  it is accepted, and nothing may be improved before it has passed parity.

Both paths coexist so parity tests can call them in the same process.

## Verification status

**Unverified.** No part of this crate has been validated. Reference
fixtures under `tests/fixtures/` record what Yan Ren's implementation
produces; agreement with them shows the translation is faithful, which is
not the same as being correct. Benchmark comparison against the published
IAEA-3D and NEACRP results is a separate check.

Not for nuclear facility operation, reactor control, licensing, or
safety-critical decisions.

## Modules

## Module `error`

Error type for BEDOK.

```rust
pub mod error { /* ... */ }
```

### Types

#### Type Alias `Result`

Result alias used throughout the crate.

```rust
pub type Result<T> = std::result::Result<T, BedokError>;
```

#### Enum `BedokError`

Everything that can go wrong in a BEDOK solve or fixture load.

```rust
pub enum BedokError {
    EmptyGrid {
        nx: usize,
        ny: usize,
        nz: usize,
        ngroups: usize,
    },
    IndexOutOfRange {
        idx: usize,
        len: usize,
    },
    Fixture {
        path: String,
        reason: String,
    },
    NotConverged {
        what: &'static str,
        iterations: usize,
        residual: f64,
    },
    Io(std::io::Error),
}
```

##### Variants

###### `EmptyGrid`

A grid dimension was zero.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Nodes in x. |
| `ny` | `usize` | Nodes in y. |
| `nz` | `usize` | Nodes in z. |
| `ngroups` | `usize` | Energy groups. |

###### `IndexOutOfRange`

A flat state-vector index was outside the grid.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `idx` | `usize` | The offending index. |
| `len` | `usize` | Valid length. |

###### `Fixture`

A reference fixture could not be read or did not have the expected shape.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `path` | `String` | Fixture path. |
| `reason` | `String` | What was wrong with it. |

###### `NotConverged`

An iterative solve failed to converge.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which solve. |
| `iterations` | `usize` | Iterations taken. |
| `residual` | `f64` | Residual reached. |

###### `Io`

Underlying I/O failure.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::io::Error` |  |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
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
    fn from(source: std::io::Error) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: BedokError) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

## Module `reference`

Stage 1 — the faithful translation of Than Yan Ren's MATLAB.

# Translation rules

These are not style preferences; they are what makes the translation
checkable. See `docs/bedok-port-scoping.md` §1.

- **Do not re-architect.** Keep the solver structure, iteration order and
  convergence logic as they are in the MATLAB.
- **Do not optimise.** A faster formulation that reorders floating-point
  accumulation defeats the purpose of a reference.
- **Do not substitute** OUTRAM PARK libraries here. The one decided
  exception is IAPWS-IF97, which comes from `tampines-steam-tables` rather
  than being ported from the third-party MATLAB file.
- **Do not fix what looks wrong.** Record it in the doc comment where it
  occurs and leave the behaviour alone.

# Naming

Every ported item carries a descriptive Rust name, and names its MATLAB
origin in the doc comment: the `.m` file and the original function. The
MATLAB names (`calc_a1234_expansionxyz`, `makegradDxyz`) are provenance,
not API.

```rust
pub mod reference { /* ... */ }
```

### Modules

## Module `cases`

Benchmark case constructors — geometry, materials, cross sections and
thermal-hydraulic boundary conditions for the cases BEDOK is verified
against.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | `iaea3ds.m`, `neacrpa2.m`, `neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1.m`, `neacrpd1t.m`, `geom2dxycase1.m`, plus the utilities `geometry_ends3d.m`, `handle2dcoords.m`, `handle3dcoords.m`, `convert_grid3d.m`, `convertindexc2d.m`, `convertsparseformat2d.m`, `convertsparsekey3d.m`, `fixinfnan.m`, `fixnegativematrix.m`, `calc_relpower3d.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# What belongs here

Everything that *describes* a benchmark: node counts and dimensions,
material maps, cross-section tables and their feedback derivatives,
fuel-pin geometry, coolant inlet conditions, control-rod bank layouts, and
the transient forcing. Nothing that *solves* anything — the nodal-diffusion
kernels live in `reference::nodal`, the channel and rod models in
`reference::th`, and the drivers that call both in `reference::coupling`.

The index/sparse utilities in [`sparse`] and the map-scanning helpers in
[`geometry`] are here because that is where the MATLAB keeps them and
because they are consumed while a case is being built; they are otherwise
independent of any particular case.

# The cases

| Constructor | MATLAB | Grid | Groups | Feedback | Transient |
|---|---|---|---|---|---|
| [`iaea_3d`] | `iaea3ds.m` | 17 × 17 × **19** | 2 | none | no |
| [`neacrp_a2`] | `neacrpa2.m` | 17 × 17 × 18 | 2 | boron, fuel T, coolant T, coolant density, rods | no |
| [`neacrp_a2_transient`] | `neacrpa2t.m` | 17 × 17 × 18 | 2 | as A2 | rod ejection, 5 s |
| [`neacrp_a1_transient`] | `neacrpa1t.m` | 17 × 17 × 18 | 2 | as A2 | rod ejection at hot zero power, 5 s |
| [`neacrp_d1()`] | `neacrpd1.m` | 17 × 17 × **14** | 2 | fuel T, coolant density | no |
| [`neacrp_d1_transient`] | `neacrpd1t.m` | 17 × 17 × **14** | 2 | as D1 | inlet cold water, 20 s |
| [`geom2d_xy_case1`] | `geom2dxycase1.m` | user × user × 1 | 1 | none | no |

# Read the grid back from the case

Three constructors overwrite the node counts the caller asked for:
`iaea3ds.m` forces `maxiz = 19` (it appends a top axial reflector plane),
`neacrpd1.m` forces `maxiz = 14`, and both force `maxix = maxiy = 17`.
[`BuiltCase::grid`] is the authority on the shape of the state vector;
whatever went in is not.

# Faithfulness

Per `docs/bedok-port-scoping.md` §1.0 the reference is translated **as it
is**, including the parts that are unfinished or wrong. Each such place
carries a doc comment saying so, and none of them is repaired here. Grep
for "Unfinished in the reference" and "Questionable in the reference" to
enumerate them.

```rust
pub mod cases { /* ... */ }
```

### Modules

## Module `csv_maps`

Assembly-composition and control-rod-bank maps, and MATLAB `readmatrix`
semantics.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source | the ten `*.csv` inputs of the BEDOK MATLAB snapshot, read by `readmatrix` in `iaea3ds.m`, `neacrpa2.m` and `neacrpd1.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# What these files are

All ten CSVs are **inputs**, not reference outputs. Nine of them are
17 × 17 maps over the modelled core quadrant/octant; each entry is a
*material index* into the case's cross-section tables (or `0` for "outside
the core", which the solver excludes from the unknowns). The tenth,
`NEACRPD1_COL.csv`, is a 14 × 10 table mapping *(axial level, radial column
type)* to a material index — the BWR case composes its 3-D material map from
the radial map plus this axial column table.

`NEACRPA2_CRODBANKS.csv` is different in kind: its entries are **control-rod
bank numbers** (`0` = no rod at that radial position), indexing
`geometry.crod`.

# Two MATLAB behaviours that must be reproduced exactly

1. **`readmatrix('IAEA3DS_1')` appends `.csv`** to a name with no extension.
   Reproduced here by naming the files through [`CompositionMap`] rather
   than by string.
2. **Every file carries a UTF-8 byte-order mark** (`EF BB BF`). MATLAB
   tolerates it; a naive reader parses the first field as `NaN`. [`parse`]
   strips it.

The files are embedded with `include_str!`, so nothing is read from disk at
run time and the maps travel with the compiled library.

```rust
pub mod csv_maps { /* ... */ }
```

### Types

#### Enum `CompositionMap`

One of the ten embedded input maps.

The variant *is* the filename in the MATLAB, so a case constructor never
spells a path and the `.csv`-appending behaviour of `readmatrix` cannot be
got wrong. Enum dispatch, per the workspace Rust rules — there is no
mechanism here for loading an arbitrary file.

```rust
pub enum CompositionMap {
    Iaea3dsBottomReflector,
    Iaea3dsLowerFuel,
    Iaea3dsUpperFuel,
    Iaea3dsTopReflector,
    NeacrpA2AxialReflector,
    NeacrpA2LowerFuel,
    NeacrpA2MainFuel,
    NeacrpA2ControlRodBanks,
    NeacrpD1RadialColumns,
    NeacrpD1ColumnTable,
}
```

##### Variants

###### `Iaea3dsBottomReflector`

IAEA-3D axial level 1: the bottom axial reflector. `readmatrix('IAEA3DS_1')`.

###### `Iaea3dsLowerFuel`

IAEA-3D axial levels 2–14: the lower fuelled region. `readmatrix('IAEA3DS_2')`.

###### `Iaea3dsUpperFuel`

IAEA-3D axial levels 15–18: the upper fuelled region, where the partly
inserted rods sit. `readmatrix('IAEA3DS_3')`.

###### `Iaea3dsTopReflector`

IAEA-3D axial level 19: the top axial reflector. `readmatrix('IAEA3DS_4')`.

###### `NeacrpA2AxialReflector`

NEACRP PWR axial reflector plane (levels 1 and 18). `readmatrix('NEACRPA2_1')`.

###### `NeacrpA2LowerFuel`

NEACRP PWR axial level 2 (the bottom fuelled plane). `readmatrix('NEACRPA2_2')`.

###### `NeacrpA2MainFuel`

NEACRP PWR axial levels 3–17 (the bulk of the fuel). `readmatrix('NEACRPA2_3')`.

###### `NeacrpA2ControlRodBanks`

NEACRP PWR control-rod bank numbers per radial position.
`readmatrix('NEACRPA2_CRODBANKS')`.

###### `NeacrpD1RadialColumns`

NEACRP BWR radial map: per radial position, which *column type*
(1–10, `0` = outside the core). `readmatrix('NEACRPD1_1')`.

###### `NeacrpD1ColumnTable`

NEACRP BWR axial column table: material index by (axial level 1–14,
column type 1–10). `readmatrix('NEACRPD1_COL')`.

##### Implementations

###### Methods

- ```rust
  pub const fn matlab_name(self: Self) -> &'static str { /* ... */ }
  ```
  The MATLAB name this map is loaded under, without the `.csv` extension

- ```rust
  pub fn load(self: Self) -> Result<NumericMatrix> { /* ... */ }
  ```
  Parse this map into a dense numeric matrix.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CompositionMap { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CompositionMap) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `NumericMatrix`

A dense matrix of `f64`, the shape MATLAB's `readmatrix` returns.

Entries are dimensionless: material indices, column-type indices or
control-rod bank numbers depending on the file. Stored row-major.

```rust
pub struct NumericMatrix {
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
  pub const fn rows(self: &Self) -> usize { /* ... */ }
  ```
  Number of rows. MATLAB `size(M,1)`.

- ```rust
  pub const fn cols(self: &Self) -> usize { /* ... */ }
  ```
  Number of columns. MATLAB `size(M,2)`.

- ```rust
  pub fn at(self: &Self, row: usize, col: usize) -> f64 { /* ... */ }
  ```
  Entry at **0-based** `(row, col)`.

- ```rust
  pub fn at_matlab(self: &Self, row: usize, col: usize) -> f64 { /* ... */ }
  ```
  Entry at **1-based** `(row, col)`, i.e. MATLAB `M(row, col)`.

- ```rust
  pub fn index_at_matlab(self: &Self, row: usize, col: usize) -> Result<usize> { /* ... */ }
  ```
  Entry at 1-based `(row, col)` as a material / bank index.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NumericMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NumericMatrix) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `parse`

Parse CSV text the way MATLAB's `readmatrix` does for these files.

Strips a leading UTF-8 byte-order mark, accepts CRLF line endings, and
requires the result to be rectangular.

`name` is used only in error messages.

# Errors

[`BedokError::Fixture`] if a field does not parse as a number, if rows have
differing lengths, or if the text holds no rows.

```rust
pub fn parse(name: &str, text: &str) -> crate::error::Result<NumericMatrix> { /* ... */ }
```

## Module `fuel`

Fuel-pin radial geometry and the material property correlations attached to
it.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | the `geometry.fuel` block of `neacrpa2.m` / `neacrpa2t.m` / `neacrpa1t.m` / `neacrpd1.m`, and the `geometry.fuel.rhocp` block of the transient cases |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# Units

Radii and lengths \[cm\], areas \[cm²\], volumes per unit length \[cm²\],
thermal conductivity \[W/cm/K\], gap conductance \[W/cm²/K\], volumetric
heat capacity \[J/cm³/K\], temperature \[K\].

```rust
pub mod fuel { /* ... */ }
```

### Types

#### Enum `RodRegion`

Which of the three radial materials a fuel-pin node is.

MATLAB `geometry.fuel.whichk`, an array of `1` (fuel), `0` (gap) and `2`
(cladding). The numbering is not an ordering: it is an index into the
`geometry.fuel.tcon` cell array, with the gap handled by a separate branch
that reads `tcon{end}`.

```rust
pub enum RodRegion {
    Fuel,
    Gap,
    Clad,
}
```

##### Variants

###### `Fuel`

Fuel pellet. MATLAB `whichk == 1`.

###### `Gap`

Pellet–cladding gap, modelled as a conductance rather than a
conducting solid. MATLAB `whichk == 0`.

###### `Clad`

Cladding. MATLAB `whichk == 2`.

##### Implementations

###### Methods

- ```rust
  pub const fn matlab_code(self: Self) -> usize { /* ... */ }
  ```
  The MATLAB `whichk` code for this region.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RodRegion { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RodRegion) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `ThermalConductivity`

A thermal-conductivity correlation, or the gap conductance.

MATLAB stores these as anonymous function handles in the
`geometry.fuel.tcon` cell array. Function handles have no place in the
port — the workspace Rust rules forbid trait objects and boxed closures —
so the closed set of correlations is an enum, dispatched by `match`.

# Units differ between variants

[`UraniumDioxide`](Self::UraniumDioxide) and [`Zircaloy`](Self::Zircaloy)
return a conductivity \[W/cm/K\]; [`GapConductance`](Self::GapConductance)
returns a **conductance** \[W/cm²/K\]. That mismatch is in the reference:
`fuelrodheat_1dcylnd.m` multiplies `tcon{end}` by a radius to recover a
conductivity-like quantity (`kplus = tcon{end}*Ctr(ir+1)`). Recorded, not
repaired.

```rust
pub enum ThermalConductivity {
    UraniumDioxide,
    Zircaloy,
    GapConductance(f64),
}
```

##### Variants

###### `UraniumDioxide`

UO₂ pellet: `(1.05 + 2150/(T - 73.15))/100` \[W/cm/K\].

Valid for `T > 73.15 K`; it is singular at `T = 73.15 K` and negative
below, neither of which a reactor calculation reaches. Both NEACRP
cases use this same correlation.

###### `Zircaloy`

Zircaloy cladding:
`(7.51 + 2.09e-2 T - 1.45e-5 T² + 7.67e-9 T³)/100` \[W/cm/K\].

###### `GapConductance`

Constant pellet–cladding gap conductance \[W/cm²/K\].

`1` for the NEACRP PWR cases, `0.35` for the BWR case — both taken
from the benchmark specification.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: Self, t: f64) -> f64 { /* ... */ }
  ```
  Evaluate at temperature `t` \[K\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermalConductivity { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermalConductivity) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `VolumetricHeatCapacity`

A volumetric-heat-capacity correlation, `rho*cp` \[J/cm³/K\].

MATLAB `geometry.fuel.rhocp`, set only by the transient cases
(`neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1t.m` — all three use the same two
correlations). Indexed like `tcon`: entry 1 is fuel, entry 2 cladding.

```rust
pub enum VolumetricHeatCapacity {
    UraniumDioxide,
    Zircaloy,
}
```

##### Variants

###### `UraniumDioxide`

UO₂ pellet:
`10.412*(1 - 0.01248)*(162.3 + 0.3038 T - 2.391e-4 T² + 6.404e-8 T³)/1000`
\[J/cm³/K\].

10.412 g/cm³ is the undished UO₂ density, reduced by the 1.248 % pellet
dishing; the bracket is the specific heat \[J/kg/K\] from NEACRP-L-335
§2.7, and the `/1000` converts g·J/(kg·cm³·K) to J/cm³/K.

###### `Zircaloy`

Zircaloy cladding: `6.6*(252.54 + 0.11474 T)/1000` \[J/cm³/K\], with
6.6 g/cm³ the Zircaloy-4 density.

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: Self, t: f64) -> f64 { /* ... */ }
  ```
  Evaluate at temperature `t` \[K\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> VolumetricHeatCapacity { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &VolumetricHeatCapacity) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `FuelGeometry`

Radial geometry of one fuel pin, plus its subchannel.

MATLAB `geometry.fuel`. Built identically by all four NEACRP case
constructors; only the dimensions and the gap conductance differ.

```rust
pub struct FuelGeometry {
    pub fuel_radius: f64,
    pub gap_thickness: f64,
    pub clad_thickness: f64,
    pub pitch: f64,
    pub doppler_alpha: f64,
    pub outer_radius: f64,
    pub node_thickness: Vec<f64>,
    pub node_center: Vec<f64>,
    pub node_area: Vec<f64>,
    pub region: Vec<RodRegion>,
    pub subchannel_area: f64,
    pub hydraulic_diameter: f64,
    pub conductivity: [ThermalConductivity; 3],
    pub heat_capacity: Vec<VolumetricHeatCapacity>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_radius` | `f64` | Pellet radius \[cm\]. MATLAB `geometry.fuel.fuelrad`. |
| `gap_thickness` | `f64` | Radial gap thickness \[cm\]. MATLAB `geometry.fuel.fuelgap`. |
| `clad_thickness` | `f64` | Cladding thickness \[cm\]. MATLAB `geometry.fuel.clad`. |
| `pitch` | `f64` | Rod pitch \[cm\]. MATLAB `geometry.fuel.pitch`. |
| `doppler_alpha` | `f64` | Weight on the pellet surface temperature in the effective Doppler<br>temperature \[dimensionless\]. MATLAB `geometry.fuel.doppleralpha`;<br>`0.7` in both cases. |
| `outer_radius` | `f64` | Outer rod radius, `fuel + gap + clad` \[cm\]. MATLAB<br>`geometry.fuel.Rtot`. |
| `node_thickness` | `Vec<f64>` | Radial thickness of each node \[cm\], innermost first. MATLAB<br>`geometry.fuel.Lr`. |
| `node_center` | `Vec<f64>` | Outer-edge-referenced centre radius of each node \[cm\], computed as<br>`sum(Lr(1:ir)) - 0.5*Lr(ir)`. MATLAB `geometry.fuel.Ctr`. |
| `node_area` | `Vec<f64>` | Per-node cross-sectional area \[cm²\] (a volume per unit rod length).<br>MATLAB `geometry.fuel.Vi`.<br><br># Unfinished in the reference — this is wrong as written<br><br>The MATLAB computes, for `i >= 2`,<br><br>```text<br>rminus = sum(geometry.fuel.Lr(i-1));   % a scalar: just Lr(i-1)<br>rplus  = sum(geometry.fuel.Lr(i));     % a scalar: just Lr(i)<br>Vi(i)  = pi*(rplus^2 - rminus^2);<br>```<br><br>`sum` of a single element is that element, so these are node<br>*thicknesses*, not cumulative radii — almost certainly a typo for<br>`sum(Lr(1:i-1))` and `sum(Lr(1:i))`. Because every pellet node has the<br>same thickness, the consequence is that **`Vi(2:fueln)` is exactly<br>zero** and the gap/clad entries are meaningless. `Vi(1)` is correct.<br><br>Reproduced exactly as written, per `docs/bedok-port-scoping.md` §1.0:<br>repairing it here would make a later disagreement with the benchmark<br>impossible to attribute. [`node_area_corrected`](Self::node_area_corrected)<br>provides the annulus areas the formula was evidently reaching for, for<br>comparison only — nothing in the reference path uses it. |
| `region` | `Vec<RodRegion>` | Material of each radial node, innermost first. MATLAB<br>`geometry.fuel.whichk`. |
| `subchannel_area` | `f64` | Coolant flow area per rod \[cm²\], `pitch² - pi*Rtot²`. MATLAB<br>`geometry.fuel.subarea`. |
| `hydraulic_diameter` | `f64` | Subchannel hydraulic diameter \[cm\]. MATLAB `geometry.fuel.hydia`,<br>computed as `4*subarea/(2*pi*Rtot + 4*pitch - 8*Rtot)`.<br><br># Questionable in the reference<br><br>The usual wetted perimeter of a square-pitch subchannel is the rod<br>circumference alone, `2*pi*Rtot`. The extra `4*pitch - 8*Rtot` adds the<br>square cell's perimeter minus the rod's projected width, which has no<br>standard justification for an interior subchannel. Recorded, not<br>changed. |
| `conductivity` | `[ThermalConductivity; 3]` | Thermal conductivity per `whichk` code, in the MATLAB's cell order:<br>entry 0 is fuel (`tcon{1}`), entry 1 cladding (`tcon{2}`), entry 2 the<br>gap conductance (`tcon{3}`, read as `tcon{end}`). |
| `heat_capacity` | `Vec<VolumetricHeatCapacity>` | Volumetric heat capacity, entry 0 fuel and entry 1 cladding. MATLAB<br>`geometry.fuel.rhocp`. Empty for a steady-only case, which does not set<br>the field at all. |

##### Implementations

###### Methods

- ```rust
  pub fn build(discretisation: FuelDiscretisation, fuel_radius: f64, gap_thickness: f64, clad_thickness: f64, pitch: f64, doppler_alpha: f64, gap_conductance: f64) -> Self { /* ... */ }
  ```
  Build the radial mesh and derived quantities from the pin dimensions.

- ```rust
  pub fn with_transient_heat_capacity(self: &mut Self) { /* ... */ }
  ```
  Attach the transient volumetric heat capacities.

- ```rust
  pub fn node_area_corrected(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The annulus areas `pi*(r_i² - r_{i-1}²)` the reference's `Vi` formula

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FuelGeometry) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `geom2d_xy`

A one-group 2-D x–y test case: a square of UO₂ in a moderator box.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source file | `geom2dxycase1.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# What it is for

Not a benchmark — a smoke test. One energy group, two materials, no
feedback, no thermal hydraulics, and a stated answer (`k_eff = 0.487` at
the default dimensions) so a broken solver shows up immediately. It is the
only 2-D case in the snapshot; `main_exec_diff3d.m` keeps its call
commented out.

# Two ways it differs from every 3-D case

- **Different boundary field names.** It sets `geometry.left`, `.right`,
  `.top`, `.bottom` instead of `.xmin` … `.zmax`. They are mapped here as
  left → `x_min`, right → `x_max`, bottom → `y_min`, top → `y_max`, with
  the two z faces given the same condition. The 2-D `geometry` is not
  interchangeable with a 3-D one in the MATLAB either.
- **No `geometry_ends3d` call**, so there are no fuelled-extent arrays.
  The solvers test `isfield(geometry,'xlows')` and fall back to the full
  range, which is correct here because no node is void.

# Representation as a degenerate 3-D grid

[`Grid`] has no 2-D form, so the case is
built with `nz = 1`. `z_total` is set to `0.0` because the MATLAB defines
no `geometry.Ztot`; node volumes are consequently **areas** \[cm²\], which
is exactly what `geometry.Vi` holds in the 2-D MATLAB (its own comment
calls it "area of each cell").

```rust
pub mod geom2d_xy { /* ... */ }
```

### Functions

#### Function `geom2d_xy_case1`

Build the 2-D x–y smoke-test case.

Rust translation of `geom2dxycase1.m`.

A `Lux` × `Luy` = 8 × 8 cm square of UO₂ centred in a 24 × 24 cm box of
moderator, one energy group, vacuum on all four sides. The source header
states `k_eff = 0.487` at these dimensions.

# Cross sections

Both materials have `Sigma_t = 5 /cm` and `Sigma_s = 0.9 * Sigma_t`; the
fuel additionally has `nu*Sigma_f = 0.05 * Sigma_t`. Absorption is
implicit in the total, as in `iaea3ds.m`.

# Questionable in the reference

`constants.chi = [1; 1]` is a **column** of length 2 — one entry per
*material* — where the 3-D cases build a `materials × G` matrix. With
`G = 1` the two readings coincide numerically, so nothing is wrong at
runtime; it is reproduced here as the `materials × G` form the rest of the
code expects, and flagged because a second energy group would break it.

The file also writes `params.nu`, `params.chi` and `params.frac_p`
alongside the `constants` struct — fields no other case sets and no solver
reads. Recorded, not carried into [`CaseParams`].

# Errors

[`crate::error::BedokError::EmptyGrid`] if the requested radial node counts
are zero.

```rust
pub fn geom2d_xy_case1(input: &super::params::CaseParams) -> crate::error::Result<super::BuiltCase> { /* ... */ }
```

## Module `geometry`

Case geometry: boundary conditions, cell centres, the fuelled-region
extents, and the utilities that derive them.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | `geometry_ends3d.m`, `convert_grid3d.m`, `calc_relpower3d.m`, and the `geometry` struct built by `iaea3ds.m` / `neacrpa2.m` / `neacrpd1.m` / `geom2dxycase1.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# Units

All lengths are **centimetres** and all volumes **cubic centimetres** — the
units the benchmark specifications and the MATLAB use throughout. `uom`
types are deliberately not used in the reference translation, so the
arithmetic stays line-for-line comparable with the original.

```rust
pub mod geometry { /* ... */ }
```

### Types

#### Enum `Boundary`

An outer boundary condition on one face of the domain.

MATLAB stores these as strings on `geometry.xmin` … `geometry.zmax`.

```rust
pub enum Boundary {
    Reflective,
    Vacuum,
    ZeroFlux,
}
```

##### Variants

###### `Reflective`

Zero net current: a symmetry plane. MATLAB `'reflective'`.

Both quadrant/octant cases use this on the two inner faces.

###### `Vacuum`

Zero incoming partial current, i.e. the usual extrapolated-boundary
condition. MATLAB `'vacuum'`. Used by IAEA-3D on all outer faces.

###### `ZeroFlux`

Flux forced to zero at the face. MATLAB `'zeroflux'`. Used by both
NEACRP cases on all outer faces.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Boundary { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Boundary) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Boundaries`

The six outer boundary conditions of a case.

# Note on the 2-D case

`geom2dxycase1.m` names its boundaries `left` / `right` / `top` / `bottom`
rather than `xmin` … `ymax`. They are mapped here as
left → `x_min`, right → `x_max`, bottom → `y_min`, top → `y_max`, and the
two z faces are filled with the same condition. The rename is recorded
because it means the 2-D case's `geometry` is **not** interchangeable with
a 3-D one in the MATLAB either.

```rust
pub struct Boundaries {
    pub x_min: Boundary,
    pub x_max: Boundary,
    pub y_min: Boundary,
    pub y_max: Boundary,
    pub z_min: Boundary,
    pub z_max: Boundary,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x_min` | `Boundary` | Condition at `x = 0`. MATLAB `geometry.xmin`. |
| `x_max` | `Boundary` | Condition at `x = Xtot`. MATLAB `geometry.xmax`. |
| `y_min` | `Boundary` | Condition at `y = 0`. MATLAB `geometry.ymin`. |
| `y_max` | `Boundary` | Condition at `y = Ytot`. MATLAB `geometry.ymax`. |
| `z_min` | `Boundary` | Condition at `z = 0`. MATLAB `geometry.zmin`. |
| `z_max` | `Boundary` | Condition at `z = Ztot`. MATLAB `geometry.zmax`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Boundaries { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Boundaries) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `GridScale`

Node-refinement factors relative to each case's native mesh.

MATLAB `xscale`, `yscale`, `zscale`, computed as `int64(maxix/17)` and so
on. They let a case be run on a refined grid: the composition maps stay
17 × 17 and are sampled with `ceil(ix/maxix*17)`.

`neacrpa2.m` and `neacrpd1.m` store these on `geometry`; `iaea3ds.m`
computes them but does not store them. They are stored uniformly here.

```rust
pub struct GridScale {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `usize` | `maxix / 17`, rounded. MATLAB `xscale`. |
| `y` | `usize` | `maxiy / 17`, rounded. MATLAB `yscale`. |
| `z` | `usize` | `maxiz / (native axial levels)`, rounded — 19 for IAEA-3D, 18 for the<br>PWR cases, 14 for the BWR case. MATLAB `zscale`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GridScale { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GridScale) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `DomainEnds`

The first and last **fuelled** node along each axis, per transverse
position.

MATLAB `geometry.xlows` / `xhis` / `ylows` / `yhis` / `zlows` / `zhis`,
built by `geometry_ends3d.m`. The nodal solver uses them to skip the
void (`whichsigma == 0`) region outside the core outline, so that a
quadrant map with a stepped radial boundary does not spend unknowns on
nodes that are not there.

# Index convention

Values here are **0-based node indices**, one less than the MATLAB's. The
accessors take 0-based transverse coordinates.

```rust
pub struct DomainEnds {
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
  pub fn x_low(self: &Self, iy: usize, iz: usize) -> usize { /* ... */ }
  ```
  First fuelled `ix` at `(iy, iz)`. MATLAB `geometry.xlows(iy,iz) - 1`.

- ```rust
  pub fn x_high(self: &Self, iy: usize, iz: usize) -> usize { /* ... */ }
  ```
  Last fuelled `ix` at `(iy, iz)`. MATLAB `geometry.xhis(iy,iz) - 1`.

- ```rust
  pub fn y_low(self: &Self, ix: usize, iz: usize) -> usize { /* ... */ }
  ```
  First fuelled `iy` at `(ix, iz)`. MATLAB `geometry.ylows(ix,iz) - 1`.

- ```rust
  pub fn y_high(self: &Self, ix: usize, iz: usize) -> usize { /* ... */ }
  ```
  Last fuelled `iy` at `(ix, iz)`. MATLAB `geometry.yhis(ix,iz) - 1`.

- ```rust
  pub fn z_low(self: &Self, ix: usize, iy: usize) -> usize { /* ... */ }
  ```
  First fuelled `iz` at `(ix, iy)`. MATLAB `geometry.zlows(ix,iy) - 1`.

- ```rust
  pub fn z_high(self: &Self, ix: usize, iy: usize) -> usize { /* ... */ }
  ```
  Last fuelled `iz` at `(ix, iy)`. MATLAB `geometry.zhis(ix,iy) - 1`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DomainEnds { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DomainEnds) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `GridKey`

A compaction of the full state vector down to only the fuelled nodes.

Rust translation of `convert_grid3d.m`. `key[full_index]` is the compacted
(1-based) position of that unknown, or `0` if the node is outside the core;
`reverse_key[compacted - 1]` is the full 1-based index it came from.

The 1-based values are kept because the sparse-matrix rewiring that
consumes them (`convertsparsekey3d.m`) tests `key(i) == 0` to mean
"dropped", which a 0-based key could not express.

```rust
pub struct GridKey {
    pub key: Vec<usize>,
    pub reverse_key: Vec<usize>,
    pub kept: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `Vec<usize>` | `key(idx)`, one entry per full state slot: the 1-based compacted index,<br>or `0` for a node outside the core. |
| `reverse_key` | `Vec<usize>` | `reversekey(counter)`: the 1-based full index each compacted unknown<br>came from. Entries beyond [`len`](Self::len) are `0`. |
| `kept` | `usize` | Number of unknowns kept. |

##### Implementations

###### Methods

- ```rust
  pub const fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of unknowns after compaction. MATLAB's final `counter`.

- ```rust
  pub const fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether every node was dropped — an all-void material map.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GridKey { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GridKey) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CaseGeometry`

Everything a case constructor puts on its `geometry` struct.

The node counts, extents, per-node lengths, volumes and material map live
in [`Geometry`], which is shared with the solver; this type carries the
case-specific remainder.

```rust
pub struct CaseGeometry {
    pub base: crate::reference::grid::Geometry,
    pub scale: GridScale,
    pub centers: Vec<[f64; 3]>,
    pub boundaries: Boundaries,
    pub ends: Option<DomainEnds>,
    pub fuel: Option<super::fuel::FuelGeometry>,
    pub control_rods: Option<ControlRodConfig>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `base` | `crate::reference::grid::Geometry` | Grid, extents, per-node lengths/volumes, and the flattened material<br>map.<br><br>**Note on `lx` / `ly` / `lz`:** these are filled **per spatial node**<br>(length `grid.nodes()`), matching MATLAB `geometry.Lx` etc., which the<br>solvers index with the full node index. The doc comment on<br>[`Geometry::lx`] describes them as "one per x index"; the per-node form<br>is what the reference actually builds and what downstream indexing<br>requires. Recorded here rather than changed — `grid.rs` is outside this<br>module's ownership. |
| `scale` | `GridScale` | Refinement factors relative to the case's native mesh. MATLAB<br>`geometry.xscale` / `yscale` / `zscale`. |
| `centers` | `Vec<[f64; 3]>` | Centre of each spatial node, `[x, y, z]` \[cm\], flattened with the<br>same rule as `base.which_sigma`. MATLAB `geometry.Ctr`. |
| `boundaries` | `Boundaries` | The six outer boundary conditions. |
| `ends` | `Option<DomainEnds>` | Fuelled extents per axis. `None` for `geom2dxycase1`, which does not<br>call `geometry_ends3d`; the solvers test `isfield(geometry,'xlows')`<br>and fall back to the full range. |
| `fuel` | `Option<super::fuel::FuelGeometry>` | Fuel-pin geometry and material correlations. `None` for the two cases<br>without thermal hydraulics. |
| `control_rods` | `Option<ControlRodConfig>` | Control-rod bank layout and positions. `None` where the case defines no<br>rods. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CaseGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `ControlRodConfig`

Control-rod bank geometry and the current bank positions.

MATLAB `geometry.crodn` / `crodbtm` / `crodstep` / `crodmaxstep` /
`crodtop` / `crodbanks` / `crod`, set by the NEACRP PWR cases.

```rust
pub struct ControlRodConfig {
    pub bank_count: usize,
    pub bottom: f64,
    pub step: f64,
    pub max_steps: f64,
    pub top: f64,
    pub banks: super::csv_maps::NumericMatrix,
    pub positions: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bank_count` | `usize` | Number of banks. MATLAB `geometry.crodn`; `7` for the PWR cases. |
| `bottom` | `f64` | Axial height of a fully inserted rod tip above the bottom of the model<br>\[cm\]. MATLAB `geometry.crodbtm`. |
| `step` | `f64` | Axial travel per withdrawal step \[cm\]. MATLAB `geometry.crodstep`. |
| `max_steps` | `f64` | Steps from fully inserted to fully withdrawn \[steps\]. MATLAB<br>`geometry.crodmaxstep`. |
| `top` | `f64` | Tip height at full withdrawal, `bottom + step*max_steps` \[cm\].<br>MATLAB `geometry.crodtop`. |
| `banks` | `super::csv_maps::NumericMatrix` | Bank number at each radial position, `0` = no rod. MATLAB<br>`geometry.crodbanks`, read from `NEACRPA2_CRODBANKS.csv`. |
| `positions` | `Vec<f64>` | Current position of each bank \[withdrawal steps\], `0` = fully<br>inserted. MATLAB `geometry.crod`. |

##### Implementations

###### Methods

- ```rust
  pub fn tip_heights(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Tip height of each bank above the bottom of the model \[cm\].

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ControlRodConfig { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ControlRodConfig) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `matlab_int64_scale`

MATLAB's `int64(x)` on a positive ratio: round half **away from zero**.

Rust's `as usize` truncates and `f64::round` rounds half away from zero,
which is what MATLAB does — so this is `round`, not `trunc`. Spelled out
because the difference decides the grid on a non-integer refinement.

# Errors

[`BedokError::EmptyGrid`] if the result is zero. The MATLAB has no such
guard: a zero scale makes its `for iz = 1:zscale` loops empty and its
`Zlengths(ceil(iz/zscale))` a division by zero. Rejecting it is an error
path, not a change to any computed value.

```rust
pub fn matlab_int64_scale(requested: usize, native: usize, grid: crate::reference::grid::Grid) -> crate::error::Result<usize> { /* ... */ }
```

#### Function `geometry_ends_3d`

Scan the material map for the fuelled extent along each axis.

Rust translation of `geometry_ends3d.m`.

For each transverse position the MATLAB walks the third index from 1
upwards, records the first node whose material is non-zero as the low end,
and the node **before** the first zero encountered *after* that as the high
end. Defaults are the full range (`1` and `maxi`).

# Two consequences of that rule, recorded not repaired

- A line that is **entirely** outside the core (all zeros) keeps the
  defaults, so it reports the *whole* axis as fuelled. Downstream code that
  trusts `xlows`/`xhis` without also checking `whichsigma` will therefore
  see phantom nodes on such a line.
- Only the **first** contiguous run is found. A line whose material returns
  to non-zero after a gap has the second run silently dropped. Neither
  benchmark has such a line, so this never bites in the ported cases.

`which_sigma` is indexed as the flattened spatial map, `ix*ny*nz + iy*nz +
iz` — the same rule as [`Grid::index`](crate::reference::grid::Grid::index)
with `g = 0`.

# Errors

[`BedokError::Fixture`] if `which_sigma` is not `grid.nodes()` long.

```rust
pub fn geometry_ends_3d(grid: crate::reference::grid::Grid, which_sigma: &[usize]) -> crate::error::Result<DomainEnds> { /* ... */ }
```

#### Function `convert_grid_3d`

Build the compaction key over the fuelled nodes.

Rust translation of `convert_grid3d.m`.

# Unfinished in the reference

The `Nc /= 0` branch computes its index as
`(G+Nc-1)*energyindexstep + …` inside a loop over `nn = 1:Nc`, so every
extra unknown at a node is given the *same* index — plainly a typo for
`(G+nn-1)*…`. `params.Nc` is `0` in every case in the snapshot, so the
branch never runs. Reproduced as written, per
`docs/bedok-port-scoping.md` §1.0.

# Errors

[`BedokError::Fixture`] if `which_sigma` is not `grid.nodes()` long.

```rust
pub fn convert_grid_3d(grid: crate::reference::grid::Grid, num_extra_unknowns: usize, which_sigma: &[usize]) -> crate::error::Result<GridKey> { /* ... */ }
```

#### Function `calc_relative_power_3d`

Collapse a power-density state vector to a normalised radial power map.

Rust translation of `calc_relpower3d.m`.

Sums the per-group power density into a single spatial field, integrates it
over `z`, then scales so that the **mean over the non-zero entries** is 1.
The result is the assembly-wise relative power, the quantity the IAEA-3D
and NEACRP benchmarks tabulate.

Returned row-major over `(ix, iy)`, length `nx*ny` \[dimensionless\].

# Note

The normalisation divides by `nnz(pwrdensxy)` — the count of radial
positions with non-zero power — so it is a mean over *fuelled* positions,
not over all of them. A position that happens to integrate to exactly zero
while being inside the core would be excluded; that cannot occur for a
converged flux.

# Errors

[`BedokError::Fixture`] if `power_density` is neither `grid.nodes()` nor
`grid.state_len()` long, or if the total power is zero (the scaling would
be `0/0`).

```rust
pub fn calc_relative_power_3d(grid: crate::reference::grid::Grid, power_density: &[f64]) -> crate::error::Result<Vec<f64>> { /* ... */ }
```

## Module `iaea3d`

IAEA-3D PWR benchmark — the two-group, no-feedback steady case.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source file | `iaea3ds.m` (function `iaea3ds`) |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# The case

The IAEA 3-D PWR benchmark: a quarter core, 17 × 17 radial nodes of 10 cm
and 19 axial nodes of 20 cm, two energy groups, five materials, and **no
thermal-hydraulic feedback of any kind** — no boron, no Doppler, no
moderator density. That makes it the cleanest possible exercise of the
neutronics path alone, which is why it is the first case ported.

Reference eigenvalues quoted in the source header: PARCS `1.029096`,
ADPRES `1.029082`.

```rust
pub mod iaea3d { /* ... */ }
```

### Functions

#### Function `iaea_3d`

Build the IAEA-3D case.

Rust translation of `iaea3ds.m`.

# The grid is overwritten, and that matters

The first three statements of `iaea3ds.m` are

```text
params.maxix = 17;
params.maxiy = 17;
params.maxiz = 19;
```

**unconditionally**, discarding whatever the caller asked for.
`main_exec_diff3d.m` requests `maxiz = 18`; this case runs on 19 axial
nodes, the extra one being the top axial reflector plane read from
`IAEA3DS_4.csv`. The returned grid is therefore 17 × 17 × 19 = 5,491 nodes,
10,982 state entries at two groups — always read the grid back from the
returned [`CaseParams`], never from the request.

A side effect of the same forcing: `xscale`, `yscale` and `zscale` are
computed *after* it, so they are always 1 and the mesh-refinement machinery
(`ceil(ix/maxix*17)` sampling of the composition maps) is dead code here.
It is ported anyway, because the NEACRP cases do not force the grid and
genuinely use it.

# Materials

| Index | Material |
|---|---|
| 1 | Outer fuel |
| 2 | Inner fuel |
| 3 | Inner fuel + control rod |
| 4 | Reflector |
| 5 | Reflector + control rod |

# Axial layout

| Axial nodes (1-based) | Composition map |
|---|---|
| 1 | `IAEA3DS_1` — bottom axial reflector |
| 2 … 14 | `IAEA3DS_2` — lower fuel |
| 15 … 18 | `IAEA3DS_3` — upper fuel, rodded region |
| 19 | `IAEA3DS_4` — top axial reflector |

# Cross sections

Given directly as `tot` (total/removal), `f` (`nu*Sigma_f`) and `s`
(scattering). `sigmavalues.a` and `sigmavalues.fp` are **not defined** —
`makesigmadfxyz.m` substitutes zeros for `fp`, and absorption is implicit
in the total minus the scattering rows. Units \[1/cm\].

# Errors

Propagates CSV-parse and grid-construction failures. In practice these can
only fire if the embedded composition maps are corrupted.

```rust
pub fn iaea_3d(input: &super::params::CaseParams) -> crate::error::Result<super::BuiltCase> { /* ... */ }
```

## Module `neacrp_a`

NEACRP-L-335 PWR rod-ejection benchmark — cases A2 (steady and transient)
and A1 (transient, hot zero power).

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | `neacrpa2.m`, `neacrpa2t.m`, `neacrpa1t.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
| Benchmark | NEACRP 3-D LWR Core Transient Benchmark, NEA/NSC/DOC(93)25 (NEACRP-L-335 Rev. 1), 1991 |

# The case

A PWR core octant with rotational symmetry, 17 × 17 radial nodes of
10.803 cm and 18 axial nodes of specified thickness, two energy groups,
eleven materials, seven control-rod banks, and full cross-section feedback
on boron, fuel temperature, coolant temperature and coolant density.

- **A2** ejects the central control assembly from 100 steps to fully
  withdrawn in 0.1 s, at **full power**.
- **A1** does the same from **fully inserted**, at **hot zero power**
  (2775 W core, 286 °C inlet), where the ejected worth is around one dollar
  and the response is a prompt-critical power spike.

# Why the three constructors share one body

`neacrpa2t.m` is a *copy* of `neacrpa2.m` with a handful of assignments
changed and a transient block appended; `neacrpa1t.m` is in turn a copy of
`neacrpa2t.m`. Every difference is a plain overwrite of a leaf value —
nothing downstream in the constructor reads `params.boron`,
`params.fueltempavg`, `th.powratio` or `geometry.crod` — so applying the
deltas *after* building A2 gives bit-identical results to running the
copied file top to bottom, while keeping the three-way relationship visible
instead of triplicating 500 lines. Each delta is named in the doc comment
of the constructor that applies it.

```rust
pub mod neacrp_a { /* ... */ }
```

### Modules

## Module `tables`

NEACRP-L-335 PWR two-group cross-section tables and their feedback
derivatives.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source file | the `sigmavalues` blocks of `neacrpa2.m` (reproduced verbatim in `neacrpa2t.m` and `neacrpa1t.m`) |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
| Benchmark | NEACRP 3-D LWR Core Transient Benchmark, NEA/NSC/DOC(93)25 (NEACRP-L-335 Rev. 1), 1991 — public, citable, clean under `DATA_POLICY.md` |

# Layout

Each table is an 11-row array of `[group 1, group 2]` values, in the same
order the MATLAB assigns them in its `(1:6,:)` and `(7:11,:)` slabs, so a
reader can diff a row against the source line. The scattering matrix is
given by its down-scatter column alone; the within-group diagonal is then
closed on `total - absorption - out-scatter`, exactly as the MATLAB's
trailing two lines do.

# Materials

| Index | Composition |
|---|---|
| 1 | Axial reflector |
| 2 | Radial reflector |
| 3 | Radial reflector, re-entrant corner |
| 4 | 2.1 w/o fuel |
| 5 | 2.6 w/o fuel |
| 6 | 3.1 w/o fuel |
| 7 | 2.6 w/o + 12 burnable absorber rods |
| 8 | 2.6 w/o + 16 burnable absorber rods |
| 9 | 2.6 w/o + 20 burnable absorber rods |
| 10 | 3.1 w/o + 12 burnable absorber rods |
| 11 | 3.1 w/o + 16 burnable absorber rods |

# Units

Cross sections \[1/cm\]; `kappa_fission` \[J/cm\]. A derivative table
carries those divided by ppm, K or g/cm³ as its variable requires.

```rust
pub mod tables { /* ... */ }
```

### Functions

#### Function `base_sigmas`

Base cross sections at the reference state.

MATLAB `sigmavalues.tot` / `.f` / `.fp` / `.a` / `.s` of `neacrpa2.m`.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn base_sigmas() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

#### Function `boron_derivatives`

Boron-concentration derivatives \[per ppm\], referenced to 1200.2 ppm.

MATLAB `sigmavalues.boron`.

# A variant left in the source

`neacrpa2.m` carries a commented-out alternative for
`sigmavalues.boron.tot(1:6,:)` that zeroes the group-2 entry of the two
reflector materials (`0 7.76184E-04` → `0 0`). The **active** line, with
the reflector entries present, is the one ported. Recorded because a
disagreement with the benchmark's boron worth would make that switch the
first thing to try.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn boron_derivatives() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

#### Function `fuel_temperature_derivatives`

Fuel-temperature (Doppler) derivatives \[per K\], referenced to 891.45 K.

MATLAB `sigmavalues.fueltemp`. The three reflector materials have no fuel,
so their derivatives are zero.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn fuel_temperature_derivatives() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

#### Function `coolant_temperature_derivatives`

Coolant-temperature derivatives \[per K\], referenced to 579.75 K.

MATLAB `sigmavalues.cooltemp`.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn coolant_temperature_derivatives() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

#### Function `coolant_density_derivatives`

Coolant-density derivatives \[per g/cm³\], referenced to 0.7125 g/cm³.

MATLAB `sigmavalues.coolden`. The axial reflector (material 1) has a
density derivative because it is water; the two radial reflector materials
do not.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn coolant_density_derivatives() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

#### Function `control_rod_increments`

Cross-section increments for a fully inserted control rod \[1/cm per unit
rodded fraction\].

MATLAB `sigmavalues.crod`. The increments are the same for every material
except 6 (3.1 w/o fuel), which has its own set. `sigmavalupd3d_handler.m`
applies them against a reference of `0`, scaled by the fraction of the node
the rod occupies.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn control_rod_increments() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

### Constants and Statics

#### Constant `MATERIALS`

Number of materials in the NEACRP PWR tables.

```rust
pub const MATERIALS: usize = 11;
```

#### Constant `GROUPS`

Number of energy groups.

```rust
pub const GROUPS: usize = 2;
```

### Functions

#### Function `neacrp_a2`

Build the NEACRP PWR case A2 at steady state.

Rust translation of `neacrpa2.m`.

# Grid

Unlike `iaea3ds.m` and `neacrpd1.m`, this case does **not** overwrite the
requested node counts — the three `params.maxi*` assignments are commented
out in the source, labelled "Recommended". It therefore runs on whatever
the driver asked for, and the refinement factors
`xscale = round(maxix/17)`, `zscale = round(maxiz/18)` are live: the
composition maps are sampled with `ceil(ix/maxix*17)` and each native axial
plane is split into `zscale` nodes of `Zlengths(k)/zscale`.

The energy-group count *is* overwritten, to 2.

# Boron

`params.boron = 1000` ppm, labelled "initial concentration" — this is
**not** the critical boron concentration. The two transient variants
replace it with values found by a boron search (1139.01 ppm for A2,
551.31 ppm for A1); the benchmark's own PANTHER values are 1160.6 and
567.7 ppm respectively.

# Errors

- [`BedokError::EmptyGrid`] if the requested grid is coarser than the
  native mesh, which would make a refinement factor zero.
- [`BedokError::Fixture`] if the requested axial node count is not a whole
  multiple of `zscale` covering all 18 native planes — the MATLAB indexes
  `Zlengths(ceil(iz/zscale))` and errors out of bounds in that case.

```rust
pub fn neacrp_a2(input: &super::params::CaseParams) -> crate::error::Result<super::BuiltCase> { /* ... */ }
```

#### Function `neacrp_a2_transient`

Build the NEACRP PWR case A2 rod-ejection transient.

Rust translation of `neacrpa2t.m`.

Identical to [`neacrp_a2`] for the steady state; the file differs from
`neacrpa2.m` in exactly these places, and each is applied here:

| MATLAB | Value | Why |
|---|---|---|
| `params.tend` | 5 s | transient window |
| `params.tgrid` | `[0:0.0025:0.2, 0.2:0.01:1, 1:0.05:5, 5]` | fine over the spike |
| `params.outprefix` | `neacrpa2t` | history CSV prefix |
| `params.boron` | 1139.01 ppm | critical boron **for this code** (coupled `k_eff` 1.000005); the benchmark's PANTHER value is 1160.6 ppm |
| `params.velocities` | `[0.28e8, 0.44e6]` cm/s | Table 2.1 |
| `params.beta_dnp` | `0.0076 * [0.034, 0.200, 0.183, 0.404, 0.145, 0.034]` | Table 2.2 |
| `params.lambda_dnp` | `[0.0128, 0.0318, 0.1190, 0.3181, 1.4027, 3.9286]` 1/s | Table 2.2 |
| `geometry.fuel.rhocp` | UO₂ + Zircaloy | §2.7 |
| `geometry.crodeject` | 1 | the central CA |
| `geometry.crodejectto` | 228 steps | fully withdrawn |
| `params.ejectduration` | 0.1 s | Figure 3.2 |

# The time grid repeats its junctions

`[0:0.0025:0.2, 0.2:0.01:1, …]` contains `0.2` twice, `1` twice and `5`
twice (the last from the explicit `params.tend` at the end). Reproduced
rather than deduplicated — see
[`TransientSchedule::has_duplicate_times`].

# Errors

As [`neacrp_a2`].

```rust
pub fn neacrp_a2_transient(input: &super::params::CaseParams) -> crate::error::Result<super::BuiltCase> { /* ... */ }
```

#### Function `neacrp_a1_transient`

Build the NEACRP PWR case A1 rod-ejection transient at hot zero power.

Rust translation of `neacrpa1t.m`, which is `neacrpa2t.m` with these
changes:

| MATLAB | A2 transient | A1 transient |
|---|---|---|
| `params.tgrid` | `[0:0.0025:0.2, …]` | `[0:0.001:0.6, 0.6:0.005:1, 1:0.025:5, 5]` |
| `params.outprefix` | `neacrpa2t` | `neacrpa1t` |
| `th.powratio` | 1 | 1e-6 (2775 W core = 693.75 W per quarter) |
| `params.boron` | 1139.01 ppm | 551.31 ppm (benchmark PANTHER value 567.7 ppm) |
| `params.fueltempavg` | 891.19 K | 559.15 K — at HZP the fuel is in equilibrium with the coolant |
| `geometry.crod` | `[100 200 100 200 200 200 200]` | `[0 0 0 228 0 0 0]` — Figure 3.1: banks 1,2,3,5,6,7 fully inserted, bank 4 fully withdrawn |

The inlet temperature is unchanged: the HZP specification's 286 °C is the
same 559.15 K that case A2 already uses.

# Note from the source

The 1 ms time grid over 0–0.6 s is there because the ejected worth is
around one dollar at HZP, so the power spike is super-prompt-critical and
spans several decades. The source also warns that at, say, 1000 ppm the
core is roughly 4200 pcm subcritical and the transient stops being a
prompt excursion — i.e. the boron value above is load-bearing.

# Errors

As [`neacrp_a2`].

```rust
pub fn neacrp_a1_transient(input: &super::params::CaseParams) -> crate::error::Result<super::BuiltCase> { /* ... */ }
```

## Module `neacrp_d1`

NEACRP-L-335 BWR case D1 — steady state and the inlet cold-water-injection
transient.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | `neacrpd1.m`, `neacrpd1t.m` (driven by `run_neacrpd1t.m`) |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
| Benchmark | NEACRP 3-D LWR Core Transient Benchmark, NEA/NSC/DOC(93)25 (NEACRP-L-335 Rev. 1), 1991 |

# The case

A BWR core quadrant, 17 × 17 radial nodes of half an assembly pitch
(15.24 cm) and 14 axial nodes of 30.48 cm, two energy groups, nineteen
materials. Feedback on fuel temperature and coolant density only — the
void feedback is the physics of interest.

The transient doubles the inlet subcooling with a 2.5 s time constant over
20 s, at constant flow and with no rod motion: colder water raises the
coolant density, which adds reactivity, which raises power.

# A naming discrepancy in the source

`neacrpd1.m` is headed "BWR NEACRP BENCHMARK - Case D2" while the function,
the data files and `neacrpd1t.m` all say D1. The transient file's header is
specific and consistent (spec §6.2, Figure 6.1, Tables 5.1/5.2), so the
comment in `neacrpd1.m` appears to be a stale copy. Recorded, not resolved.

```rust
pub mod neacrp_d1 { /* ... */ }
```

### Modules

## Module `tables`

NEACRP-L-335 BWR two-group cross-section tables and their feedback
derivatives.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source file | the `sigmavalues` blocks of `neacrpd1.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
| Benchmark | NEACRP 3-D LWR Core Transient Benchmark, NEA/NSC/DOC(93)25 (NEACRP-L-335 Rev. 1), 1991 |

# Nineteen materials, unlabelled

Unlike the PWR case, `neacrpd1.m` gives **no comment block naming the
materials**. What can be read off the data and the axial column table
(`NEACRPD1_COL.csv`): material 1 is the bottom reflector plane and
material 4 the top, neither of which fissions; material 19 is the radial
reflector (column 10 at every axial level), also non-fissioning; the
remaining sixteen are fuelled compositions varying with void history and
burnable-absorber loading. Recorded as an observation, not an authority —
the reference does not say.

# What this case does *not* have

- **No `sigmavalues.fp`.** The array is allocated and every assignment to
  it is commented out, so `kappa*Sigma_f` is identically zero.
  `neacrpd1t.m` rebuilds it from `nu*Sigma_f` because the transient power
  normalisation divides by it; the steady solver never reads it.
- **No boron feedback**, even though `neacrpd1.m` sets
  `params.boron = 1000`.
- **No coolant-*temperature* feedback** — only fuel temperature and coolant
  density. In a BWR the density (void) feedback dominates, so this is a
  defensible modelling choice rather than plainly an omission, but it is
  an asymmetry with the PWR case worth knowing about.
- **No control-rod cross sections.** The section header is present and the
  body is empty.

All four are recorded, not repaired — see `docs/bedok-port-scoping.md`
§1.0.

# Units

Cross sections \[1/cm\]. Derivative tables carry those per K (fuel
temperature) or per g/cm³ (coolant density).

```rust
pub mod tables { /* ... */ }
```

### Functions

#### Function `base_sigmas`

Base cross sections at the reference state.

MATLAB `sigmavalues.tot` / `.f` / `.a` / `.s` of `neacrpd1.m`.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn base_sigmas() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

#### Function `fuel_temperature_derivatives`

Fuel-temperature (Doppler) derivatives \[per K\], referenced to 573.15 K.

MATLAB `sigmavalues.fueltemp` of `neacrpd1.m`.

Every **group-1 total** derivative is zero while the group-1 *absorption*
derivative is positive: the Doppler broadening shows up as resonance
capture that is exactly offset in the total by the closing identity, so the
within-group scattering diagonal absorbs it. That is a property of the
data as supplied, not of the port.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn fuel_temperature_derivatives() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

#### Function `coolant_density_derivatives`

Coolant-density derivatives \[per g/cm³\], referenced to 0.55 g/cm³.

MATLAB `sigmavalues.coolden` of `neacrpd1.m`. This is the dominant
feedback of a BWR: it is what couples the void distribution back into the
neutronics.

# Errors

Cannot fail in practice: the internal assembly step only errors when the
absorption table is missing, and it is always supplied here.

```rust
pub fn coolant_density_derivatives() -> crate::error::Result<crate::reference::cases::sigmas::SigmaSet> { /* ... */ }
```

### Constants and Statics

#### Constant `MATERIALS`

Number of materials in the NEACRP BWR tables.

```rust
pub const MATERIALS: usize = 19;
```

#### Constant `GROUPS`

Number of energy groups.

```rust
pub const GROUPS: usize = 2;
```

### Functions

#### Function `neacrp_d1`

Build the NEACRP BWR case D1 at steady state.

Rust translation of `neacrpd1.m`.

# The grid is overwritten

Like `iaea3ds.m` and unlike the PWR cases, the first three statements force

```text
params.maxix = 17;  params.maxiy = 17;  params.maxiz = 14;
```

`run_neacrpd1t.m` requests `maxiz = 18` and gets 14. The axial count is
fixed by the data: `NEACRPD1_COL.csv` has exactly 14 rows, one per axial
level. Read the grid back from the returned [`CaseParams`].

# How the material map is built

Two files rather than four bands. `NEACRPD1_1.csv` gives a *column type*
(1–10, `0` = outside the core) at each radial position; `NEACRPD1_COL.csv`
gives the material index for each (axial level, column type) pair. A
position with column type `0` is left at material `0` for its whole height.

# The inlet temperature comes from a steam-table flash

The MATLAB computes it through three IAPWS-IF97 calls — saturation
temperature at 6.7 MPa, the saturated-liquid enthalpy there, then a
`(p,h)` flash 46.52 kJ/kg below it. Per `docs/bedok-port-scoping.md` §3
`IAPWS_IF97.m` is **not** ported and `tampines-steam-tables` is substituted;
that is the one substitution allowed inside the reference path, and it
happens here, in
[`CoolantInletTemperature::evaluate_kelvin`](super::th::CoolantInletTemperature::evaluate_kelvin).
The stored inlet keeps the *specification* — pressure and subcooling — so
the substitution stays visible.

`params.cooltempavg`, which the MATLAB sets to that same inlet temperature,
is filled from the evaluated flash.

# Unfinished in the reference

- **`th.flowrate` is assigned twice.** The first assignment
  (`13000000/157/400.78`) is dead; the live value is
  `70000/(30.48² - 221*pi*0.715²)`. Both are ported as one field with the
  live value, and the dead line is noted here rather than in code.
- **`params.boron = 1000` with no boron feedback table.** The steady
  coupled driver reaches `sigmavalupd3d_handler.m`, which tests
  `isfield(sigmavaluesref,'boron')` and skips the update, so the number has
  no effect. It is carried through anyway.
- **No coolant-temperature feedback and no control-rod cross sections.**
  See the [`tables`] module docs.

# Errors

- [`crate::error::BedokError::EmptyGrid`] if a refinement factor would be
  zero.
- [`crate::error::BedokError::Fixture`] if a composition map entry is out of
  range.

```rust
pub fn neacrp_d1(input: &super::params::CaseParams) -> crate::error::Result<super::BuiltCase> { /* ... */ }
```

#### Function `neacrp_d1_transient`

Build the NEACRP BWR case D1 inlet cold-water-injection transient.

Rust translation of `neacrpd1t.m`, which calls `neacrpd1.m` and then adds:

| MATLAB | Value | Source |
|---|---|---|
| `sigmavalues.fp` | `sigmavalues.f * 3.20e-11` | Table 5.1 prompt energy release |
| `sigmavalues.fueltemp.fp`, `.coolden.fp` | the same scaling of their `f` | |
| `params.tend` | 20 s | §6.2 |
| `params.tgrid` | `[0:0.025:2, 2:0.05:6, 6:0.1:12, 12:0.2:20]` | |
| `params.th_model` | `'hem'` | homogeneous equilibrium, steady *and* transient |
| `params.velocities` | `[1/3.57e-8, 1/2.27e-6]` cm/s | Table 5.1 |
| `params.beta_dnp` | `[0.00026, 0.00152, 0.00139, 0.00307, 0.00110, 0.00026]` | Table 5.2 |
| `params.lambda_dnp` | `[0.013, 0.032, 0.119, 0.318, 1.403, 3.929]` 1/s | Table 5.2 |
| `geometry.fuel.rhocp` | UO₂ + Zircaloy | §5.7 |
| `geometry.crodeject` | 0 — **no rod motion** | §6.2 |
| `th.inlettemp_t` | `46.52*(2 - exp(-0.4 t))` kJ/kg subcooling | Figure 6.1 |

# The kappa-fission workaround, in the reference's own words

`neacrpd1.m` leaves `sigmavalues.fp` identically zero because the steady
solver derives power from the fission source and never reads it. The
transient normalisation `P/P0` *does* read it, and would compute `0/0`. The
source therefore rebuilds it as `fp = E0 * nu*Sigma_f / nu` with
`E0 = 3.20e-11 J/fission` and `nu = 1` as encoded, noting that under a
composition-uniform `nu` the **ratio** `P/P0` is exact because the `E0`
scale cancels. That is a repair made by the reference itself, so it is
ported as written — it is not this translation adding one.

# Why the model is forced to HEM

From the source header: the transient chain
(`th_solvertimexyz` → `singleflow1devaptime`) is a homogeneous-equilibrium
enthalpy march, so the *initial steady state* must run the same model. The
two-fluid steady solver would hand the HEM transient a slip-void density
mismatch, i.e. a spurious reactivity step at `t = 0`.

# Errors

As [`neacrp_d1`].

```rust
pub fn neacrp_d1_transient(input: &super::params::CaseParams) -> crate::error::Result<super::BuiltCase> { /* ... */ }
```

## Module `params`

The `params` struct a case constructor takes in and hands back, and the
transient/kinetics data the transient cases attach to it.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | the `params` fields set by `main_exec_diff3d.m`, `run_neacrpd1t.m`, `iaea3ds.m`, `neacrpa2.m`, `neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1.m`, `neacrpd1t.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# The in-out convention, and why it matters

Every MATLAB case constructor has the signature
`[params, …] = case(params)`: the caller passes a user set-up struct and
gets a *modified* one back. Some cases overwrite the grid the caller asked
for — `iaea3ds.m` forces 17 × 17 × **19**, `neacrpd1.m` forces
17 × 17 × **14** — so **the grid must always be read back from the returned
params, never from what was requested.** The Rust constructors keep that
shape (`&CaseParams` in, a new [`CaseParams`] out) precisely so the mistake
is hard to make.

```rust
pub mod params { /* ... */ }
```

### Types

#### Enum `ThermalHydraulicModel`

Which thermal-hydraulic model the coupled solver should run.

MATLAB `params.th_model`, a string, set only by `neacrpd1t.m`
(`params.th_model='hem'`). Absent means "the case's default path".

```rust
pub enum ThermalHydraulicModel {
    HomogeneousEquilibrium,
}
```

##### Variants

###### `HomogeneousEquilibrium`

Homogeneous-equilibrium enthalpy march
(`singleflow1devap` / `singleflow1devaptime`). MATLAB `'hem'`.

`neacrpd1t.m` selects it for both the steady state and the transient so
that the two use the *same* model: seeding the HEM transient from the
two-fluid steady solver would hand it a slip-void density mismatch,
i.e. a spurious reactivity step at `t = 0`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermalHydraulicModel { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermalHydraulicModel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `FuelDiscretisation`

Radial discretisation of the fuel pin, for the 1-D cylindrical conduction
model.

MATLAB `params.fuel`. Node counts, not lengths — the radii live in
[`FuelGeometry`](super::fuel::FuelGeometry).

```rust
pub struct FuelDiscretisation {
    pub gap_nodes: usize,
    pub clad_nodes: usize,
    pub fuel_nodes: usize,
    pub total_nodes: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gap_nodes` | `usize` | Radial nodes across the pellet–clad gap. MATLAB `params.fuel.gapn`;<br>`1` in every case. |
| `clad_nodes` | `usize` | Radial nodes across the cladding. MATLAB `params.fuel.cladn`; `1` in<br>every case. |
| `fuel_nodes` | `usize` | Radial nodes across the fuel pellet. MATLAB `params.fuel.fueln`; `20`<br>in every case. |
| `total_nodes` | `usize` | Total radial nodes, `fuel + gap + clad`. MATLAB `params.fuel.maxir`. |

##### Implementations

###### Methods

- ```rust
  pub const fn neacrp_default() -> Self { /* ... */ }
  ```
  The discretisation both NEACRP cases use: 20 pellet nodes, 1 gap node,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelDiscretisation { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FuelDiscretisation) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `KineticsData`

Prompt-neutron speeds and delayed-neutron precursor data for the transient
solver.

MATLAB `params.velocities`, `params.beta_dnp`, `params.lambda_dnp`, set by
`neacrpa2t.m`, `neacrpa1t.m` and `neacrpd1t.m`. Absent in the steady cases.

```rust
pub struct KineticsData {
    pub velocities: Vec<f64>,
    pub beta: Vec<f64>,
    pub lambda: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `velocities` | `Vec<f64>` | Prompt neutron speed per energy group \[cm/s\]. MATLAB<br>`params.velocities`.<br><br>The PWR cases give speeds directly (`0.28e8`, `0.44e6`); the BWR case<br>gives them as reciprocals of the Table 5.1 inverse velocities<br>(`1/3.57e-8`, `1/2.27e-6`). |
| `beta` | `Vec<f64>` | Delayed-neutron fraction per precursor group \[dimensionless\]. MATLAB<br>`params.beta_dnp`. Six groups, summing to 0.0076. |
| `lambda` | `Vec<f64>` | Precursor decay constant per group \[1/s\]. MATLAB `params.lambda_dnp`. |

##### Implementations

###### Methods

- ```rust
  pub fn total_beta(self: &Self) -> f64 { /* ... */ }
  ```
  Total delayed-neutron fraction, `sum(beta)` \[dimensionless\].

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> KineticsData { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &KineticsData) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `RodEjection`

A control-rod bank ejection, the forcing of the two PWR transient cases.

MATLAB splits this across two structs: `geometry.crodeject` (which bank),
`geometry.crodejectto` (final position) and `params.ejectduration` (how
long). It is kept together here; the doc comments name both origins.

```rust
pub struct RodEjection {
    pub bank: usize,
    pub target_steps: f64,
    pub duration: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bank` | `usize` | Index into `geometry.crod` of the ejected bank, **1-based** as in the<br>MATLAB. MATLAB `geometry.crodeject`; `1` (the central CA) in both PWR<br>transients.<br><br>`neacrpd1t.m` sets `geometry.crodeject = 0` meaning *no rod motion*;<br>that case is represented by `rod_ejection: None` rather than by a zero<br>here. |
| `target_steps` | `f64` | Final bank position \[withdrawal steps\]. MATLAB<br>`geometry.crodejectto`; `228` = fully withdrawn. |
| `duration` | `f64` | Ejection time \[s\]. MATLAB `params.ejectduration`; `0.1` s, and the<br>benchmark states it is independent of insertion depth. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RodEjection { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RodEjection) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `TransientSchedule`

The time window and output grid of a transient run.

MATLAB `params.tend`, `params.tgrid`, `params.outprefix`, plus the rod
motion. Present only on the transient cases.

```rust
pub struct TransientSchedule {
    pub t_end: f64,
    pub time_grid: Vec<f64>,
    pub output_prefix: String,
    pub rod_ejection: Option<RodEjection>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_end` | `f64` | End of the transient \[s\]. MATLAB `params.tend`. |
| `time_grid` | `Vec<f64>` | Output/step times \[s\], ascending, starting at 0. MATLAB<br>`params.tgrid`.<br><br>Built in the MATLAB by concatenating uniform ranges, which **repeats<br>the junction times** — e.g. `[0:0.0025:0.2, 0.2:0.01:1, …]` contains<br>`0.2` twice. The duplicates are reproduced here rather than removed:<br>they are what the reference solver actually steps over. See<br>[`has_duplicate_times`](Self::has_duplicate_times). |
| `output_prefix` | `String` | Prefix for the solver's history CSVs. MATLAB `params.outprefix`. |
| `rod_ejection` | `Option<RodEjection>` | The control-rod ejection driving the transient, if any. `None` for the<br>BWR cold-water-injection case, whose forcing is a coolant inlet<br>condition instead. |

##### Implementations

###### Methods

- ```rust
  pub fn has_duplicate_times(self: &Self) -> bool { /* ... */ }
  ```
  Whether the time grid repeats a time, as the MATLAB concatenations do.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TransientSchedule { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TransientSchedule) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CaseParams`

The `params` struct: solver controls, grid shape, and the state feedback
variables' initial averages.

# Units

Temperatures are kelvin, boron is ppm by weight, coolant density is
g/cm³ — the units the NEACRP cross-section feedback tables are expressed
against.

```rust
pub struct CaseParams {
    pub grid: crate::reference::grid::Grid,
    pub max_num_cycles: usize,
    pub nodal_update: usize,
    pub stop: usize,
    pub verbosity: u8,
    pub plot_figures: bool,
    pub plot_3d: bool,
    pub debug_dump: bool,
    pub num_extra_unknowns: usize,
    pub prompt_fraction: Option<f64>,
    pub jfnk_preconditioner: bool,
    pub jfnk_relaxation: f64,
    pub jfnk_verbosity: u8,
    pub boron_ppm: Option<f64>,
    pub fuel_temperature_average: Option<f64>,
    pub coolant_temperature_average: Option<f64>,
    pub coolant_density_average: Option<f64>,
    pub fuel: Option<FuelDiscretisation>,
    pub transient: Option<TransientSchedule>,
    pub kinetics: Option<KineticsData>,
    pub thermal_hydraulic_model: Option<ThermalHydraulicModel>,
    pub steady_state_file: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `crate::reference::grid::Grid` | Node counts and energy-group count: MATLAB `params.maxix`,<br>`params.maxiy`, `params.maxiz` and `params.G` in one place.<br><br>**Read this back from the constructed case.** Three of the four case<br>constructors overwrite what the caller asked for. |
| `max_num_cycles` | `usize` | Maximum outer (power-iteration) cycles. MATLAB<br>`params.max_num_cycles`. |
| `nodal_update` | `usize` | Cycles between semi-analytic nodal corrections; `0` selects the<br>solver's default. MATLAB `params.nodalupd`. |
| `stop` | `usize` | Force a stop after this many cycles; `0` disables. MATLAB<br>`params.stop`. |
| `verbosity` | `u8` | Verbosity level. MATLAB `params.verb`. |
| `plot_figures` | `bool` | Whether the driver draws figures. MATLAB `params.plotfig` (0/1). |
| `plot_3d` | `bool` | Whether the driver draws the 3-D power plot. MATLAB `params.plot3d`. |
| `debug_dump` | `bool` | Whether the solver dumps debug state. MATLAB `params.debugdump`. |
| `num_extra_unknowns` | `usize` | Number of extra (precursor) unknowns carried alongside the flux per<br>node. MATLAB `params.Nc`; **`0` in every case in the snapshot**, so the<br>`Nc /= 0` branches of the index utilities are untested dead code. |
| `prompt_fraction` | `Option<f64>` | Prompt fission fraction \[dimensionless\]. MATLAB `params.frac_p`; `1`<br>wherever it is set (`iaea3ds.m`, `geom2dxycase1.m`).<br><br>The NEACRP cases never set it — `params.frac_p` is simply absent there,<br>represented as `None`. |
| `jfnk_preconditioner` | `bool` | Whether the solver preconditions its JFNK iterations. MATLAB<br>`params.jfnkprecon`. |
| `jfnk_relaxation` | `f64` | JFNK under-relaxation factor \[dimensionless\]. MATLAB<br>`params.jfnkrel`. |
| `jfnk_verbosity` | `u8` | JFNK verbosity. MATLAB `params.jfnkverb`. |
| `boron_ppm` | `Option<f64>` | Boron concentration \[ppm\]. MATLAB `params.boron`. |
| `fuel_temperature_average` | `Option<f64>` | Core-average fuel temperature used to initialise the Doppler feedback<br>\[K\]. MATLAB `params.fueltempavg`. |
| `coolant_temperature_average` | `Option<f64>` | Core-average coolant temperature used to initialise the moderator<br>temperature feedback \[K\]. MATLAB `params.cooltempavg`. |
| `coolant_density_average` | `Option<f64>` | Core-average coolant density used to initialise the density feedback<br>\[g/cm³\]. MATLAB `params.cooldenavg`. |
| `fuel` | `Option<FuelDiscretisation>` | Fuel-pin radial discretisation, when the case runs thermal hydraulics.<br>MATLAB `params.fuel`. |
| `transient` | `Option<TransientSchedule>` | Transient window, time grid and rod motion. MATLAB `params.tend` /<br>`tgrid` / `outprefix` (+ the ejection fields). `None` for a steady case. |
| `kinetics` | `Option<KineticsData>` | Prompt velocities and delayed-neutron data. MATLAB<br>`params.velocities` / `beta_dnp` / `lambda_dnp`. `None` for a steady<br>case. |
| `thermal_hydraulic_model` | `Option<ThermalHydraulicModel>` | Thermal-hydraulic model override. MATLAB `params.th_model`. |
| `steady_state_file` | `Option<String>` | Path the transient driver caches its converged steady state in. MATLAB<br>`params.steadyfile`, set by `run_neacrpd1t.m`. |

##### Implementations

###### Methods

- ```rust
  pub fn main_exec_defaults() -> Self { /* ... */ }
  ```
  The user set-up block of `main_exec_diff3d.m`, verbatim.

- ```rust
  pub fn run_neacrpd1t_defaults() -> Self { /* ... */ }
  ```
  The user set-up block of `run_neacrpd1t.m`.

- ```rust
  pub const fn coords_3d(self: &Self) -> (usize, usize, usize) { /* ... */ }
  ```
  Node counts `(maxix, maxiy, maxiz)`. MATLAB `handle3dcoords(params)`.

- ```rust
  pub const fn coords_2d(self: &Self) -> (usize, usize) { /* ... */ }
  ```
  Node counts `(maxi1, maxi2)` for a 2-D case. MATLAB

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CaseParams { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CaseParams) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `colon`

**Attributes:**

- `MustUse { reason: None }`

Build the MATLAB expression `a:step:b` — a closed range that stops at or
before `b`.

Used by the transient cases to reproduce `params.tgrid` exactly. Note
MATLAB's colon operator accumulates as `a + k*step`, which is what is done
here, so the rounding matches.

# Panics

If `step` is not strictly positive.

```rust
pub fn colon(a: f64, step: f64, b: f64) -> Vec<f64> { /* ... */ }
```

## Module `sigmas`

Cross-section tables and the feedback-derivative tables the coupled solver
interpolates them with.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | the `sigmavalues` and `constants` blocks of `iaea3ds.m`, `neacrpa2.m`, `neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1.m`, `neacrpd1t.m`, `geom2dxycase1.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# Units

Macroscopic cross sections are per centimetre \[1/cm\]; `kappa_fission` is
an energy release times a cross section \[J/cm\]. A feedback table holds the
**derivative** of each of those with respect to its state variable, so its
units are the cross section's divided by ppm, K or g/cm³ as appropriate.

# Group index convention for scattering

MATLAB writes the scattering table as `sigmavalues.s(material, gt, g)` and
reads it in `makesigmadfxyz.m` as "from group `g` into group `gt`" — the
**second** index is the destination. The rows the case files assign,
`s(m,:,:) = [s_11 s_12; s_21 s_22]`, therefore put the down-scatter cross
section at `s(m,2,1)`. [`ScatterTable`] keeps that ordering.

```rust
pub mod sigmas { /* ... */ }
```

### Types

#### Struct `ScatterTable`

Group-to-group scattering cross sections, per material \[1/cm\].

MATLAB `sigmavalues.s`, a `(materials × G × G)` array indexed
`s(material, to, from)`.

```rust
pub struct ScatterTable {
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
  pub fn zeros(materials: usize, ngroups: usize) -> Self { /* ... */ }
  ```
  An all-zero table for `materials` materials and `ngroups` groups.

- ```rust
  pub const fn materials(self: &Self) -> usize { /* ... */ }
  ```
  Number of materials the table covers.

- ```rust
  pub const fn ngroups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups.

- ```rust
  pub fn get(self: &Self, material: usize, to: usize, from: usize) -> f64 { /* ... */ }
  ```
  Cross section for scattering **from** group `from` **into** group `to`,

- ```rust
  pub fn set(self: &mut Self, material: usize, to: usize, from: usize, value: f64) { /* ... */ }
  ```
  Set the cross section for `from` → `to` \[1/cm\], all indices 0-based.

- ```rust
  pub fn set_block_2x2(self: &mut Self, material: usize, block: [[f64; 2]; 2]) { /* ... */ }
  ```
  Assign a whole material's 2 × 2 block in MATLAB source order.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ScatterTable { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ScatterTable) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `SigmaSet`

One complete set of macroscopic cross sections, or of their derivatives
with respect to a feedback variable.

MATLAB spreads these over `sigmavalues.tot` / `.f` / `.fp` / `.a` / `.s`,
and repeats the same five fields inside each feedback sub-struct
(`sigmavalues.boron.tot`, `sigmavalues.fueltemp.tot`, …). One type serves
both roles here, exactly as it does there.

# Absent fields

A MATLAB struct may simply lack a field — `iaea3ds.m` defines no
`sigmavalues.a` and no `sigmavalues.fp`, and `makesigmadfxyz.m` tests
`isfield(sigmavalues,'fp')` and substitutes zeros. An **empty** `Vec` here
means the same thing: the field was not set. Use
[`absorption_or_zero`](Self::absorption_or_zero) /
[`kappa_fission_or_zero`](Self::kappa_fission_or_zero) to get the
zero-filled form.

```rust
pub struct SigmaSet {
    pub total: Vec<Vec<f64>>,
    pub nu_fission: Vec<Vec<f64>>,
    pub kappa_fission: Vec<Vec<f64>>,
    pub absorption: Vec<Vec<f64>>,
    pub scatter: ScatterTable,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `Vec<Vec<f64>>` | Total (removal) cross section per material and group \[1/cm\]. MATLAB<br>`sigmavalues.tot`; indexed `[material][group]`, both 0-based. |
| `nu_fission` | `Vec<Vec<f64>>` | Fission production cross section, `nu*Sigma_f` \[1/cm\]. MATLAB<br>`sigmavalues.f`. The `nu` of the benchmarks is folded in already —<br>`sigmavalues.nu` is all ones. |
| `kappa_fission` | `Vec<Vec<f64>>` | Fission energy-release cross section, `kappa*Sigma_f` \[J/cm\]. MATLAB<br>`sigmavalues.fp`. Empty where the case does not define it. |
| `absorption` | `Vec<Vec<f64>>` | Absorption cross section \[1/cm\]. MATLAB `sigmavalues.a`. Empty where<br>the case does not define it. |
| `scatter` | `ScatterTable` | Group-to-group scattering \[1/cm\]. MATLAB `sigmavalues.s`. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(materials: usize, ngroups: usize) -> Self { /* ... */ }
  ```
  An all-zero set for `materials` materials and `ngroups` groups, with

- ```rust
  pub fn materials(self: &Self) -> usize { /* ... */ }
  ```
  Number of materials.

- ```rust
  pub fn ngroups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups.

- ```rust
  pub fn absorption_or_zero(self: &Self) -> Vec<Vec<f64>> { /* ... */ }
  ```
  Absorption, or zeros if the case left the field unset.

- ```rust
  pub fn kappa_fission_or_zero(self: &Self) -> Vec<Vec<f64>> { /* ... */ }
  ```
  `kappa*Sigma_f`, or zeros if the case left the field unset — the

- ```rust
  pub fn close_self_scatter(self: &mut Self) -> Result<()> { /* ... */ }
  ```
  Fill the within-group scattering diagonal from the total and absorption

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SigmaSet { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SigmaSet) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `FeedbackTable`

The derivative of every cross section with respect to one feedback
variable, plus the state that variable is referenced to.

MATLAB `sigmavalues.boron`, `.fueltemp`, `.cooltemp`, `.coolden`, `.crod`.
`sigmavalupd3d.m` applies them as
`sigma <- sigma + d(sigma)/dx * (x - x_ref)` per node.

```rust
pub struct FeedbackTable {
    pub reference: Option<f64>,
    pub derivative: SigmaSet,
    pub update_mask: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reference` | `Option<f64>` | The state value the base cross sections were generated at: boron<br>\[ppm\], temperature \[K\] or density \[g/cm³\]. MATLAB<br>`sigmavalues.<var>.ref`.<br><br>`None` for the control-rod table, which the case files leave unset —<br>`sigmavalup3d_handler.m` assigns `sigmavaluesref.crod.ref = 0` before<br>use, because the "state" there is a rodded *fraction* running from 0<br>to 1. |
| `derivative` | `SigmaSet` | The derivatives themselves, one value per material and group. |
| `update_mask` | `Vec<f64>` | Per-spatial-node mask: `1` where this feedback is applied, `0` where it<br>is not. MATLAB `sigmavalues.<var>.upd`, flattened<br>`ix*ny*nz + iy*nz + iz`.<br><br>Every case sets it to "the node is fissile", i.e. `sum(f(m,:)) > 0`,<br>then reuses the same mask for the other feedbacks. Empty where the<br>MATLAB does not define the field (the control-rod table). |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FeedbackTable { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FeedbackTable) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `SigmaValues`

The complete `sigmavalues` struct of a case.

```rust
pub struct SigmaValues {
    pub base: SigmaSet,
    pub nu: Vec<Vec<f64>>,
    pub chi: Vec<Vec<f64>>,
    pub boron: Option<FeedbackTable>,
    pub fuel_temperature: Option<FeedbackTable>,
    pub coolant_temperature: Option<FeedbackTable>,
    pub coolant_density: Option<FeedbackTable>,
    pub control_rod: Option<FeedbackTable>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `base` | `SigmaSet` | The base cross sections, at the reference state. |
| `nu` | `Vec<Vec<f64>>` | Average neutrons per fission per material and group<br>\[dimensionless\]. MATLAB `sigmavalues.nu`, copied from<br>`constants.nu`; all ones in every case, because `sigmavalues.f`<br>already carries `nu*Sigma_f`. |
| `chi` | `Vec<Vec<f64>>` | Fission emission spectrum per material and group \[dimensionless\].<br>MATLAB `sigmavalues.chi`, copied from `constants.chi`; all fission<br>neutrons are born in group 1 in every case. |
| `boron` | `Option<FeedbackTable>` | Boron-concentration feedback \[per ppm\]. MATLAB `sigmavalues.boron`. |
| `fuel_temperature` | `Option<FeedbackTable>` | Fuel-temperature (Doppler) feedback \[per K\]. MATLAB<br>`sigmavalues.fueltemp`. |
| `coolant_temperature` | `Option<FeedbackTable>` | Coolant-temperature feedback \[per K\]. MATLAB<br>`sigmavalues.cooltemp`. |
| `coolant_density` | `Option<FeedbackTable>` | Coolant-density feedback \[per g/cm³\]. MATLAB<br>`sigmavalues.coolden`. |
| `control_rod` | `Option<FeedbackTable>` | Fully-inserted-control-rod increment \[1/cm per unit rodded<br>fraction\]. MATLAB `sigmavalues.crod`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SigmaValues { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SigmaValues) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CaseConstants`

The `constants` struct: the fission spectrum, neutron yield, and prompt
fraction.

MATLAB copies `constants.chi` and `constants.nu` straight into
`sigmavalues`, so the two are always equal; both are kept so the port has
the same shape as the original.

```rust
pub struct CaseConstants {
    pub chi: Vec<Vec<f64>>,
    pub nu: Vec<Vec<f64>>,
    pub frac_p: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `chi` | `Vec<Vec<f64>>` | Fission emission spectrum per material and group \[dimensionless\].<br>MATLAB `constants.chi`. |
| `nu` | `Vec<Vec<f64>>` | Neutron yield per fission, per material and group \[dimensionless\].<br>MATLAB `constants.nu`. |
| `frac_p` | `Option<f64>` | Prompt fission fraction \[dimensionless\]. MATLAB `constants.frac_p`.<br><br>`None` for the NEACRP cases, which never set it — a MATLAB struct with<br>the field simply absent. |

##### Implementations

###### Methods

- ```rust
  pub fn fast_group_birth(materials: usize, ngroups: usize, frac_p: Option<f64>) -> Self { /* ... */ }
  ```
  The spectrum every 3-D case uses: all fission neutrons born in group 1,

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CaseConstants { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CaseConstants) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `fissile_node_mask`

Build the fissile-node mask a feedback table is applied over.

Rust translation of the loop every NEACRP case writes as

```text
for ix … for iy … for iz
    if whichsigma(ix,iy,iz)==0, continue, end
    if sum(sigmavalues.f(whichsigma(ix,iy,iz),:))>0
        upd(idx)=1;
    end
```

i.e. **1 at every node whose material produces fission neutrons, 0
elsewhere**. `which_sigma` holds 1-based material indices flattened
`ix*ny*nz + iy*nz + iz`; `nu_fission` is `[material][group]`.

# Errors

[`BedokError::Fixture`] if a node names a material outside `nu_fission`.

```rust
pub fn fissile_node_mask(which_sigma: &[usize], nu_fission: &[Vec<f64>]) -> crate::error::Result<Vec<f64>> { /* ... */ }
```

#### Function `assign_rows`

Copy a MATLAB literal of the form `[a b; c d; …]` into `[material][group]`
rows, starting at 1-based material `first_material`.

The case files assign their tables in slabs
(`sigmavalues.tot(1:6,:) = […]; sigmavalues.tot(7:11,:) = […]`) so that the
port can be diffed against the MATLAB line by line. This helper preserves
that structure.

# Errors

[`BedokError::Fixture`] if the rows do not fit inside `table`, or if a row
has the wrong number of groups.

```rust
pub fn assign_rows(table: &mut [Vec<f64>], first_material: usize, rows: &[[f64; 2]]) -> crate::error::Result<()> { /* ... */ }
```

## Module `sparse`

Sparse-index bookkeeping and the small numeric guards.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | `convertindexc2d.m`, `convertsparseformat2d.m`, `convertsparsekey3d.m`, `fixnegativematrix.m`, `fixinfnan.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# Representation

MATLAB manipulates these matrices exclusively through the pair
`[i,j,v] = find(mat)` and `sparse(i,j,v,m,n)`, i.e. as coordinate triplets.
[`CooMatrix`] is that same triplet form, so each ported function is a
line-by-line reading of its original rather than a translation into some
other sparse format.

Row and column indices are stored **1-based**, as MATLAB's are. That is not
a stylistic choice: `convertsparsekey3d.m` uses `key(i) == 0` to mean "this
unknown was dropped", which a 0-based index cannot express.

```rust
pub mod sparse { /* ... */ }
```

### Types

#### Struct `CooMatrix`

A sparse matrix as coordinate triplets, with MATLAB's 1-based indices.

Duplicate `(row, col)` pairs are permitted and are summed on assembly, as
MATLAB's `sparse` does; none of the ported functions produces them.

```rust
pub struct CooMatrix {
    pub rows: usize,
    pub cols: usize,
    pub row_index: Vec<usize>,
    pub col_index: Vec<usize>,
    pub values: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rows` | `usize` | Number of rows. |
| `cols` | `usize` | Number of columns. |
| `row_index` | `Vec<usize>` | 1-based row index of each stored entry. MATLAB `i` from `find`. |
| `col_index` | `Vec<usize>` | 1-based column index of each stored entry. MATLAB `j`. |
| `values` | `Vec<f64>` | Stored values. MATLAB `v`. |

##### Implementations

###### Methods

- ```rust
  pub const fn empty(rows: usize, cols: usize) -> Self { /* ... */ }
  ```
  An empty `rows` × `cols` matrix.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of stored entries. MATLAB `nnz(mat)`, except that an explicitly

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the matrix stores no entries.

- ```rust
  pub fn push(self: &mut Self, row: usize, col: usize, value: f64) -> Result<()> { /* ... */ }
  ```
  Append an entry at **1-based** `(row, col)`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CooMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CooMatrix) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `TwoDIndexMode`

Which unknown ordering a 2-D sparse operator is expressed in.

MATLAB passes these as the bare integers `frommode` / `tomode`.

```rust
pub enum TwoDIndexMode {
    Nodal,
    HalfIndex,
}
```

##### Variants

###### `Nodal`

Full node indices only: `g*maxi1*maxi2 + ix*maxi2 + iy`. MATLAB mode 1.

###### `HalfIndex`

Diamond-difference half indices, on a `(2*maxi1+1) × (2*maxi2+1)` mesh
that carries cell centres *and* faces. MATLAB mode 2.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TwoDIndexMode { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TwoDIndexMode) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `TwoDIndexParams`

The 2-D grid shape the index conversion needs.

MATLAB reads these straight off `params` inside `convertindexc2d.m`.

```rust
pub struct TwoDIndexParams {
    pub ngroups: usize,
    pub maxi1: usize,
    pub maxi2: usize,
    pub num_extra_unknowns: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ngroups` | `usize` | Energy groups. MATLAB `params.G`. |
| `maxi1` | `usize` | Nodes along the first axis. MATLAB `params.maxi1`. |
| `maxi2` | `usize` | Nodes along the second axis. MATLAB `params.maxi2`. |
| `num_extra_unknowns` | `usize` | Extra unknowns per node. MATLAB `params.Nc`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TwoDIndexParams { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TwoDIndexParams) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `NonFiniteFill`

What to substitute for a non-finite entry.

MATLAB selects between the two by whether `fixinfnan` was given a second
argument at all — `varargin` is tested with `isempty`, and its value is
never read.

```rust
pub enum NonFiniteFill {
    Zero,
    SmallestMagnitude,
}
```

##### Variants

###### `Zero`

Replace with zero. MATLAB `fixinfnan(v)`.

###### `SmallestMagnitude`

Replace with the smallest magnitude in the vector. MATLAB
`fixinfnan(v, anything)`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NonFiniteFill { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NonFiniteFill) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `convert_index_c2d`

Convert a vector of 2-D sparse indices between the nodal and half-index
orderings.

Rust translation of `convertindexc2d.m`. Indices are 1-based in and out.

# Not used by any ported case

This is part of the legacy 2-D path; the 3-D cases use
[`convert_sparse_key_3d`] and the key from
[`convert_grid_3d`](super::geometry::convert_grid_3d) instead. It is ported
for completeness, and because `convertsparseformat2d.m` calls it.

# Unfinished in the reference

- `convertindexc2d.m` reads `params.maxi1` / `params.maxi2` **directly**,
  while its only caller `convertsparseformat2d.m` obtains the same two
  numbers through `handle2dcoords`. A `params` carrying `maxix`/`maxiy`
  (as every case does) therefore satisfies the caller and errors in the
  callee. Recorded, not repaired: [`TwoDIndexParams`] makes the requirement
  explicit instead.
- `philenf1` and `philenf2` are computed and never used.
- **The half-index → nodal branch does not work.** It computes
  `ix = ceil(mod(v-1, energystep2)/xstep2)/2` and
  `iy = (mod(mod(v-1, energystep2), xstep2)+1)/2`, and for *every* index the
  forward direction produces, at least one of the two is a half-integer —
  so the reconstructed nodal index is never an integer. Round-tripping `1`
  through the forward map and back gives `2.5`, not `1`. In MATLAB that
  value reaches `sparse`, which rejects a non-integer subscript. The
  arithmetic is reproduced in floating point so the defect is visible
  rather than hidden by an integer cast, and the non-integer result is
  returned as an error. See the test
  `half_index_to_nodal_is_broken_in_the_reference`.

# Errors

[`BedokError::IndexOutOfRange`] if a converted index is not a positive
integer, which is the reference's silent failure made explicit.

```rust
pub fn convert_index_c2d(params: TwoDIndexParams, indices: &[usize], from: TwoDIndexMode, to: TwoDIndexMode) -> crate::error::Result<Vec<usize>> { /* ... */ }
```

#### Function `convert_sparse_format_2d`

Re-index a 2-D sparse operator between the two orderings.

Rust translation of `convertsparseformat2d.m`. Only the row and column
indices are converted; the values are carried across unchanged — the
MATLAB has a commented-out line that converted the *values* too, which
would have been meaningless.

# Errors

Propagates [`convert_index_c2d`]'s errors.

```rust
pub fn convert_sparse_format_2d(params: TwoDIndexParams, matrix: &CooMatrix, from: TwoDIndexMode, to: TwoDIndexMode) -> crate::error::Result<CooMatrix> { /* ... */ }
```

#### Function `convert_sparse_key_3d`

Compact a 3-D sparse operator onto the fuelled unknowns.

Rust translation of `convertsparsekey3d.m`. `key` is
[`GridKey::key`](super::geometry::GridKey::key): the new 1-based index of
each old unknown, or `0` for one that is dropped.

An entry whose **row** is a dropped unknown is skipped only when it is the
identity element placed there to keep the matrix non-singular — MATLAB's
`key(i(k))==0 && i(k)==j(k) && v(k)==1`. Any *other* entry on a dropped row
is kept and then indexed with `key == 0`, which MATLAB's `sparse` rejects.
The reference prints a diagnostic block (`k`, `i(k)`, `j(k)`, `v(k)` and a
decoded `ix,iy,iz`) and then fails on the `sparse` call.

# Behavioural difference, stated plainly

The port does **not** print that diagnostic — a library must not write to
stdout — and returns [`BedokError::IndexOutOfRange`] where the MATLAB would
have errored inside `sparse`. Both paths fail on the same inputs; only the
message differs. The decoded coordinates in the reference's diagnostic are
hard-coded to a 19 × 17 × 17 grid (`rem(i-1,19)`), so they are wrong for
any other case — recorded, not fixed, since the block is unreachable except
on the way to an error.

# Errors

[`BedokError::IndexOutOfRange`] if a kept entry maps to a dropped unknown,
or if `key` is shorter than the matrix.

```rust
pub fn convert_sparse_key_3d(matrix: &CooMatrix, key: &[usize], new_len: usize) -> crate::error::Result<CooMatrix> { /* ... */ }
```

#### Function `fix_negative_matrix`

**Attributes:**

- `MustUse { reason: None }`

Zero every negative entry of a sparse matrix.

Rust translation of `fixnegativematrix.m`. Used after a cross-section
feedback update has been applied, to stop an extrapolated derivative from
driving a cross section below zero.

The MATLAB assigns `mat(i,j) = 0`, which *removes* the entry from the
sparse structure; here the entry is retained with a zero value. The
difference is invisible to every arithmetic use and avoids a reallocation.

```rust
pub fn fix_negative_matrix(matrix: &CooMatrix) -> CooMatrix { /* ... */ }
```

#### Function `fix_inf_nan`

**Attributes:**

- `MustUse { reason: None }`

Replace `Inf`, `-Inf` and `NaN` entries of a vector.

Rust translation of `fixinfnan.m`.

# A subtlety in [`SmallestMagnitude`](NonFiniteFill::SmallestMagnitude)

The MATLAB computes `min(abs(vector))` over the vector *including* its
non-finite entries. `min` skips `NaN`, so those do no harm, but `+Inf`
entries do participate — harmlessly, since `Inf` can only be the minimum if
every entry is `Inf`, in which case the substitution is `Inf` and nothing is
fixed. The source comment claims the minimum is "over remaining finite
vals", which is true only by that accident. Reproduced as written.

If the vector holds no finite entries at all, the fill is `Inf` (or `NaN`
for an all-`NaN` vector, since `min` of an empty selection is `NaN`); the
port returns the vector unchanged in the all-`NaN` case, matching MATLAB's
`min([]) = []` assignment failure being avoided by the `any(mask)` guard
only when there is something to fix.

```rust
pub fn fix_inf_nan(vector: &[f64], fill: NonFiniteFill) -> Vec<f64> { /* ... */ }
```

## Module `th`

The `th` struct: core power, coolant inlet conditions and channel geometry
counts.

# Provenance

| | |
|---|---|
| Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
| Source files | the `T-H input` block of `neacrpa2.m` / `neacrpa2t.m` / `neacrpa1t.m` / `neacrpd1.m`, and the inlet forcing of `neacrpd1t.m` |
| Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
| Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |

# Units

Power \[W\], pressure \[MPa\], temperature \[K\], mass flux \[g/s/cm²\],
specific enthalpy \[kJ/kg\] (numerically equal to the code's J/g).

```rust
pub mod th { /* ... */ }
```

### Types

#### Enum `FlowDirection`

Direction of coolant flow through the core.

MATLAB `th.flowdir`, `+1` for upwards and `-1` for downwards. Both NEACRP
cases are upflow.

```rust
pub enum FlowDirection {
    Upward,
    Downward,
}
```

##### Variants

###### `Upward`

Inlet at the bottom of the model. MATLAB `th.flowdir = 1`.

###### `Downward`

Inlet at the top of the model. MATLAB `th.flowdir = -1`.

##### Implementations

###### Methods

- ```rust
  pub const fn sign(self: Self) -> f64 { /* ... */ }
  ```
  The MATLAB sign, `+1` or `-1`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FlowDirection { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FlowDirection) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `CoolantInletTemperature`

How the coolant inlet temperature is specified.

MATLAB writes `th.coolant.inlettemp` as a number in the PWR cases, but the
BWR case computes it from a **subcooling below saturation**:

```text
tsat = IAPWS_IF97('Tsat_p', 6.7);
hsat = IAPWS_IF97('h1_pT',  6.7, tsat);
th.coolant.inlettemp = IAPWS_IF97('T_ph', 6.7, hsat - 46.52);
```

# Why the specification is kept rather than only its value

`docs/bedok-port-scoping.md` §3 decides that `IAPWS_IF97.m` is **not**
ported and that steam properties come from `tampines-steam-tables`, through
`reference::th::steam`. The flash *is* evaluated — see
[`evaluate_kelvin`](Self::evaluate_kelvin) — but the pressure and enthalpy
deficit are what the benchmark actually specifies, so they are what is
stored. That also makes the one allowed substitution visible at the point
it happens instead of baked into a literal.

```rust
pub enum CoolantInletTemperature {
    Fixed(f64),
    SubcooledBelowSaturation {
        pressure_mpa: f64,
        enthalpy_deficit_kj_per_kg: f64,
    },
}
```

##### Variants

###### `Fixed`

A temperature stated directly \[K\]. MATLAB
`th.coolant.inlettemp = 559.15`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `SubcooledBelowSaturation`

Subcooled by a stated enthalpy below the saturated-liquid enthalpy at
the system pressure.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure_mpa` | `f64` | System pressure \[MPa\]. |
| `enthalpy_deficit_kj_per_kg` | `f64` | Enthalpy below saturated liquid \[kJ/kg\]; `46.52` for NEACRP D1. |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate_kelvin(self: Self) -> f64 { /* ... */ }
  ```
  The inlet temperature \[K\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoolantInletTemperature { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoolantInletTemperature) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `ColdWaterInjection`

The NEACRP D1 inlet cold-water-injection forcing.

Rust translation of `th.inlettemp_t` in `neacrpd1t.m`. NEACRP-L-335
Figure 6.1 doubles the inlet subcooling with a 2.5 s time constant:

```text
dH(t) = 46.52 * (2 - exp(-0.4 t))   kJ/kg
```

so `dH(0) = 46.52`, exactly the steady inlet of `neacrpd1.m` — the forcing
is continuous at `t = 0` — rising to `93.04 kJ/kg`. The core pressure and
the inlet mass flow are constant throughout, and there is no rod motion.

This type supplies the enthalpy history the MATLAB feeds to
`IAPWS_IF97('T_ph', …)`; [`inlet_at`](Self::inlet_at) turns a time into the
[`CoolantInletTemperature`] whose
[`evaluate_kelvin`](CoolantInletTemperature::evaluate_kelvin) performs the
flash.

```rust
pub struct ColdWaterInjection {
    pub pressure_mpa: f64,
    pub steady_deficit_kj_per_kg: f64,
    pub asymptotic_multiple: f64,
    pub rate_per_second: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure_mpa` | `f64` | Core pressure, constant through the transient \[MPa\]. |
| `steady_deficit_kj_per_kg` | `f64` | Steady-state subcooling \[kJ/kg\]; the `46.52` of Figure 6.1. |
| `asymptotic_multiple` | `f64` | Asymptotic multiple of the steady subcooling \[dimensionless\]; `2`. |
| `rate_per_second` | `f64` | Exponential rate \[1/s\]; `0.4`, i.e. a 2.5 s time constant. |

##### Implementations

###### Methods

- ```rust
  pub const fn neacrp_d1() -> Self { /* ... */ }
  ```
  The Figure 6.1 forcing of NEACRP D1.

- ```rust
  pub fn enthalpy_deficit_kj_per_kg(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Inlet enthalpy deficit below saturated liquid at time `t` \[s\], in

- ```rust
  pub fn inlet_at(self: &Self, t: f64) -> CoolantInletTemperature { /* ... */ }
  ```
  The inlet condition at time `t` \[s\], in the form the steam-table

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ColdWaterInjection { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ColdWaterInjection) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CoolantInlet`

Coolant inlet state.

MATLAB `th.coolant`.

```rust
pub struct CoolantInlet {
    pub pressure_mpa: f64,
    pub temperature: CoolantInletTemperature,
    pub inlet_void: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure_mpa` | `f64` | System pressure \[MPa\]. MATLAB `th.coolant.inletpress`; 15.5 MPa for<br>the PWR cases, 6.7 MPa for the BWR case. |
| `temperature` | `CoolantInletTemperature` | Inlet temperature, stated or specified as a subcooling.<br>MATLAB `th.coolant.inlettemp`. |
| `inlet_void` | `f64` | Inlet volumetric gas fraction \[dimensionless\]. MATLAB<br>`th.coolant.inletvoid`.<br><br>Both cases set `1e-14` rather than zero: the two-phase closures divide<br>by the void fraction, so a hard zero would be a division by zero. That<br>is a numerical guard in the reference, not a physical statement. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoolantInlet { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoolantInlet) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `ThermalHydraulics`

The `th` struct: everything the thermal-hydraulic solver needs that is not
geometry.

```rust
pub struct ThermalHydraulics {
    pub max_power_watt: f64,
    pub power_ratio: f64,
    pub coolant_heat_fraction: f64,
    pub coolant: CoolantInlet,
    pub mass_flux_g_per_s_cm2: f64,
    pub flow_direction: FlowDirection,
    pub fuel_pins_per_node: f64,
    pub guide_tubes_per_node: f64,
    pub inlet_forcing: Option<ColdWaterInjection>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_power_watt` | `f64` | Rated thermal power of the modelled sector \[W\]. MATLAB `th.maxpow`.<br><br>The PWR cases model a core quarter and give `693.75e6 W`; the BWR case<br>gives `1800e6/4 W`. |
| `power_ratio` | `f64` | Fraction of rated power the case runs at \[dimensionless\]. MATLAB<br>`th.powratio`; `1` at full power, `1e-6` for the hot-zero-power case A1. |
| `coolant_heat_fraction` | `f64` | Fraction of the fission energy deposited directly in the coolant<br>\[dimensionless\]. MATLAB `th.coolheatfrac`; `0.019` in every case. |
| `coolant` | `CoolantInlet` | Inlet state. |
| `mass_flux_g_per_s_cm2` | `f64` | Area-averaged coolant mass flux \[g/s/cm²\]. MATLAB `th.flowrate`. |
| `flow_direction` | `FlowDirection` | Flow direction. MATLAB `th.flowdir`. |
| `fuel_pins_per_node` | `f64` | Fuel pins per radial node, after dividing by the radial refinement<br>factors \[dimensionless count\]. MATLAB `th.nfuelpin`, which is<br>assigned as the per-assembly count and then divided by<br>`xscale*yscale`. |
| `guide_tubes_per_node` | `f64` | Guide tubes per radial node \[dimensionless count\]. MATLAB<br>`th.gtube`; `25` in both cases.<br><br># Unfinished in the reference<br><br>Unlike `th.nfuelpin`, this is **not** divided by the refinement<br>factors, so on a refined grid the guide-tube count per node stays at<br>the whole-assembly value while the pin count shrinks. Recorded, not<br>fixed. It has no effect at the native 17 × 17 mesh, where both scales<br>are 1. |
| `inlet_forcing` | `Option<ColdWaterInjection>` | Time-dependent inlet forcing, if the case has one. MATLAB<br>`th.inlettemp_t`, set only by `neacrpd1t.m`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermalHydraulics { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermalHydraulics) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Types

#### Struct `BuiltCase`

Everything a MATLAB case constructor returns, in one value.

The MATLAB signature is
`[params, geometry, th, constants, whichsigma, sigmavalues] = case(params)`
(or the same without `th` for the two cases with no thermal hydraulics).
`whichsigma` is not a separate field here because the MATLAB also writes it
onto `geometry` and the two are always the same array; it is reached
through [`which_sigma`](Self::which_sigma).

```rust
pub struct BuiltCase {
    pub params: CaseParams,
    pub geometry: CaseGeometry,
    pub constants: CaseConstants,
    pub sigmas: SigmaValues,
    pub th: Option<ThermalHydraulics>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `params` | `CaseParams` | Solver controls and the **authoritative** grid. MATLAB `params`. |
| `geometry` | `CaseGeometry` | Dimensions, material map, boundary conditions, fuel pin and rods.<br>MATLAB `geometry`. |
| `constants` | `CaseConstants` | Fission spectrum, neutron yield and prompt fraction. MATLAB<br>`constants`. |
| `sigmas` | `SigmaValues` | Cross sections and their feedback derivatives. MATLAB `sigmavalues`. |
| `th` | `Option<ThermalHydraulics>` | Thermal-hydraulic boundary conditions. MATLAB `th`; absent for the two<br>neutronics-only cases. |

##### Implementations

###### Methods

- ```rust
  pub const fn grid(self: &Self) -> Grid { /* ... */ }
  ```
  The grid the case was actually built on.

- ```rust
  pub fn which_sigma(self: &Self) -> &[usize] { /* ... */ }
  ```
  Material index per spatial node, 1-based as in the MATLAB, `0` outside

- ```rust
  pub fn material_at(self: &Self, ix: usize, iy: usize, iz: usize) -> usize { /* ... */ }
  ```
  Material index at 0-based `(ix, iy, iz)`; `0` means the node is outside

- ```rust
  pub fn active_nodes(self: &Self) -> usize { /* ... */ }
  ```
  Number of spatial nodes inside the modelled core, i.e. with a non-zero

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BuiltCase { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

### Re-exports

#### Re-export `geom2d_xy_case1`

```rust
pub use geom2d_xy::geom2d_xy_case1;
```

#### Re-export `CaseGeometry`

```rust
pub use geometry::CaseGeometry;
```

#### Re-export `iaea_3d`

```rust
pub use iaea3d::iaea_3d;
```

#### Re-export `neacrp_a1_transient`

```rust
pub use neacrp_a::neacrp_a1_transient;
```

#### Re-export `neacrp_a2`

```rust
pub use neacrp_a::neacrp_a2;
```

#### Re-export `neacrp_a2_transient`

```rust
pub use neacrp_a::neacrp_a2_transient;
```

#### Re-export `neacrp_d1`

```rust
pub use neacrp_d1::neacrp_d1;
```

#### Re-export `neacrp_d1_transient`

```rust
pub use neacrp_d1::neacrp_d1_transient;
```

#### Re-export `CaseParams`

```rust
pub use params::CaseParams;
```

#### Re-export `CaseConstants`

```rust
pub use sigmas::CaseConstants;
```

#### Re-export `SigmaValues`

```rust
pub use sigmas::SigmaValues;
```

#### Re-export `ThermalHydraulics`

```rust
pub use th::ThermalHydraulics;
```

## Module `coupling`

Coupled neutronics/thermal-hydraulics drivers, cross-section feedback, and
the critical-boron search.

# Provenance

Translated from Than Yan Ren's (SNRSI) BEDOK MATLAB snapshot
(`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`, received 2026-08-05).
Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute. Translated with permission; see `docs/bedok-port-scoping.md` §6.

| This module | MATLAB source |
|---|---|
| [`steady`] | `thdiffusion_solverxyz.m` |
| [`transient`] | `thdiffusion_solvertimexyz.m` |
| [`cross_section_feedback`] | `sigmavalupd3d.m`, `sigmavalupd3d_handler.m` |
| [`critical_boron`] | `criticalboron_xyz.m` |
| [`sparse`] | MATLAB built-in sparse syntax (`\`, `decomposition`, `spdiags`) |
| [`seam`] | *provisional* — the `nodal/` and `th/` interfaces these drivers call |

# What "coupling" means here

Neutronics and thermal hydraulics are coupled by **Picard iteration**, not
by a monolithic Newton solve. The neutronics produces a power distribution;
the T-H turns it into fuel and coolant temperatures and a coolant density;
[`cross_section_feedback`] turns those back into cross sections; and the
cycle repeats. The fields that carry the feedback (coolant density, Doppler
temperature, average fuel temperature, wall heat flux) are **under-relaxed**
on every pass, because the undamped cycle oscillates between cold/dense and
boiling/void states in a BWR.

The steady driver ([`steady::solve_coupled_steady`]) runs that cycle to
convergence. The transient driver ([`transient::solve_coupled_transient`])
starts from it, re-equilibrates the operator it will actually time-step, and
then marches the multigroup diffusion equation with six delayed-neutron
precursor families, one T-H step per time step. The boron search
([`critical_boron::search_critical_boron`]) wraps a guarded secant around
static eigensolves at a frozen T-H state, then refines boron, flux and
feedback together.

# No Jacobian-free Newton-Krylov solver exists in the snapshot

Recorded here because the project's scoping document describes the transient
driver as "JFNK-preconditioned", and the case scripts set
`params.jfnkprecon`, `params.jfnkrel` and `params.jfnkverb`
(`main_exec_diff3d.m:19-21`, `run_neacrpd1t.m:11`), with `params.ptc` and
`params.jfnk_max_iter` documented at `main_exec_diff3d.m:50-61`.

**No file in the snapshot reads any of those five controls.** The JFNK
solver they belong to is `driftflux_solverstatic1d.m`, which is *not in the
snapshot*, together with `driftflux_eqnstatic1d5.m`, `enthmix_forward.m`,
`enthmix_invert.m` and `bwrchfhottest.m`. The transient driver translated
here is a **linear implicit-Euler / exponential-transform time integration
with a direct sparse solve per step and Picard feedback coupling** — there
is no Newton iteration, no Krylov solver and no preconditioner anywhere in
it. Nothing has been invented to fill the gap; see
`docs/bedok-port-scoping.md` §1.0 on why gaps are recorded rather than
completed during translation.

# Status

**Unverified.** Nothing here has been run against a benchmark, and the
`nodal/` and `th/` calls it makes are [`todo!`] stubs at the time of
writing (see [`seam`]). Not for nuclear facility operation, reactor
control, licensing, or safety-critical decisions.

```rust
pub mod coupling { /* ... */ }
```

### Modules

## Module `critical_boron`

Critical boron concentration search for the coupled steady state.

# Provenance

Translated from `criticalboron_xyz.m` in Than Yan Ren's (SNRSI) BEDOK
MATLAB snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`, received
2026-08-05). Original author: **Than Yan Ren**, Singapore Nuclear Research
and Safety Institute. Translated with permission; see
`docs/bedok-port-scoping.md` §6.

# Why the search is built this way

Yan Ren's own note on the 2026-06 rewrite, kept because it explains a design
that otherwise looks over-elaborate: the previous implementation wrapped a
secant iteration around full **cold-started** coupled solves, one per boron
iterate. That cold-start T-H Picard *"can go chaotic at off-nominal boron
(keff transients into the hundreds)"* and either trips the solver's
not-converging exit — returning a garbage `k_eff` that poisons the secant,
with boron observed diverging past 1e5 ppm — or settles into a spurious
coupled state.

The rewrite never cold-starts the T-H away from the starting boron:

- **Phase 0** — one coupled steady solve at the starting boron, with a
  robust bootstrap ([`eigensolve_cold`]) if the standard solver diverges
  from its cold start.
- **Phase 1** — a guarded secant on *static* eigenvalue solves at the frozen
  Phase-0 T-H state. Cheap, and it measures the boron worth slope.
- **Phase 2** — a warm-started coupled loop: per outer iteration one static
  eigensolve, a boron correction using the measured slope, and one
  under-relaxed static T-H update, converging boron, flux and feedback
  together.

# Two eigensolvers, and why both are needed

[`eigensolve_at_boron`] delegates to the production SA-nodal eigensolver
warm-started from the running flux. [`eigensolve_cold`] instead builds the
nodal correction from the incoming flux, **freezes** it, and runs a
self-normalising power iteration. Yan Ren verified both halves of the
reason:

- the production solver's *continuous* nodal updates use the still-bad
  mid-iteration flux on a cold start and diverge (`k_eff → 5e4`) on a
  heavily rodded configuration; freezing them via `params.nodalupd` does
  stabilise it, **but**
- the production solver builds its *initial* nodal correction from a
  hardcoded flat flux, so a frozen call returns a ~25 pcm-biased, flatter
  seed — which then destabilised a near-critical Phase-1 warm solve
  (`k_eff → 377`).

[`eigensolve_cold`] is stable cold *and* returns an accurate seed, which the
production solver cannot be made to do through its parameters.

```rust
pub mod critical_boron { /* ... */ }
```

### Types

#### Struct `CriticalBoronOutput`

Result of a critical-boron search — the MATLAB `output` struct of
`criticalboron_xyz.m`.

```rust
pub struct CriticalBoronOutput {
    pub boron: f64,
    pub k_eff: f64,
    pub boron_history: Vec<f64>,
    pub k_eff_history: Vec<f64>,
    pub slope_pcm_per_ppm: f64,
    pub scalar_flux: Vec<f64>,
    pub fission_source: Vec<f64>,
    pub pwrdens: Vec<f64>,
    pub th: super::seam::ThermalState,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `boron` | `f64` | Critical boron concentration \[ppm\]. |
| `k_eff` | `f64` | Multiplication factor at that concentration \[-\]. |
| `boron_history` | `Vec<f64>` | Boron iterates \[ppm\], Phase 1 followed by Phase 2. |
| `k_eff_history` | `Vec<f64>` | `k_eff` at each iterate \[-\]. |
| `slope_pcm_per_ppm` | `f64` | Measured boron worth \[pcm/ppm\] — negative for a PWR. |
| `scalar_flux` | `Vec<f64>` | Converged scalar flux, `state_len` entries. |
| `fission_source` | `Vec<f64>` | Fission source `sigma_f * phi`, `state_len` entries. |
| `pwrdens` | `Vec<f64>` | Node power density `fission_source .* Vi`, `state_len` entries. |
| `th` | `super::seam::ThermalState` | Coupled thermal-hydraulic state at critical boron. |
| `converged` | `bool` | Whether both the `k_eff` and the fuel-temperature criteria were met. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CriticalBoronOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `BoronEigenSolution`

What an eigensolve inside the search returns.

```rust
pub struct BoronEigenSolution {
    pub k_eff: f64,
    pub flux: Vec<f64>,
    pub fission_source: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | Multiplication factor \[-\]. |
| `flux` | `Vec<f64>` | Converged flux, `state_len` entries. |
| `fission_source` | `Vec<f64>` | Fission source `sigma_f * phi`, `state_len` entries. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoronEigenSolution { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `ColdEigenSolution`

The same, plus the nodal terms the cold bootstrap carries between calls.

```rust
pub struct ColdEigenSolution {
    pub k_eff: f64,
    pub flux: Vec<f64>,
    pub fission_source: Vec<f64>,
    pub nodal_terms: super::seam::NodalTerms,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | Multiplication factor \[-\]. |
| `flux` | `Vec<f64>` | Converged flux, `state_len` entries. |
| `fission_source` | `Vec<f64>` | Fission source `sigma_f * phi`, `state_len` entries. |
| `nodal_terms` | `super::seam::NodalTerms` | Nodal terms, warm-started into the next bootstrap iteration. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ColdEigenSolution { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

### Functions

#### Function `search_critical_boron`

**Attributes:**

- `Other("#[allow(clippy::too_many_lines)]")`

Search for the boron concentration that makes the coupled steady state
critical.

MATLAB `criticalboron_xyz(geometry, params, th, sigmavalues, whichsigma,
initial_k_eff)`.

# Arguments

- `initial_k_eff` — MATLAB `varargin{1}`; 1.0 when [`None`].

Controls read from `params`: [`crit_tol`](CaseParams::crit_tol) (default
1e-5), [`fuel_temp_tol`](CaseParams::fuel_temp_tol) (default 0.5 K),
[`th_relax`](CaseParams::th_relax) (default 0.5), and
[`boron`](CaseParams::boron) as the starting concentration.

# Deviations from the MATLAB

- **No steady-state cache.** `params.steadyfile` names a `.mat` file; there
  is no `.mat` support here, so Phase 0 always solves.
- **No progress printing.** The same iterates are in
  [`boron_history`](CriticalBoronOutput::boron_history) and
  [`k_eff_history`](CriticalBoronOutput::k_eff_history).
- The MATLAB's two `warning()` calls (a discarded bad cache, and a bootstrap
  that has not converged) have no output channel here; the second is visible
  as [`converged`](CriticalBoronOutput::converged) being false.

# Errors

[`CouplingError::EigenvalueOutOfRange`] if any eigensolve leaves the sane
band — the MATLAB `criticalboron_xyz:badeig` and `:badboot` errors — plus
any sparse or feedback failure.

# Panics

Through the [`seam`] stubs until `nodal/` and `th/` land.

```rust
pub fn search_critical_boron(geometry: &super::seam::CoreGeometry, params: &super::seam::CaseParams, th_in: &super::seam::ThermalState, sigma_values: &super::seam::SigmaValues, which_sigma: &super::seam::MaterialMap, initial_k_eff: Option<f64>) -> super::error::Result<CriticalBoronOutput> { /* ... */ }
```

#### Function `eigensolve_at_boron`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Static eigenvalue at a given boron and **frozen** T-H state, warm-started
from the incoming flux.

MATLAB local function `eigsolveboron`. Updates the cross sections for this
boron and T-H state (boron plus Doppler/coolant feedback through
[`update_cross_sections`]), then delegates the eigenvalue solve to the
production SA-nodal eigensolver. Using the same eigensolver as the Phase-0
coupled solve and the transient solver keeps the reported `k_eff` consistent
across the whole search.

# Precondition

**Only safe from a warm flux.** The production solver's continuous nodal
updates act on the flux at every update; from a flat cold flux they diverge,
which is why the Phase-0 bootstrap uses [`eigensolve_cold`] instead.

# Forced parameters

`params.boron` is set to `boron`, `params.plotfig` to 0 (suppressing the
solver's per-call diagnostic figure — no-op here, as nothing plots), and
`params.innertol` to [`SEARCH_INNER_TOL`].

# Errors

Propagates cross-section feedback failures.

# Panics

Through the [`seam`] stub until `nodal/` lands.

```rust
pub fn eigensolve_at_boron(params: &super::seam::CaseParams, geometry: &super::seam::CoreGeometry, sigma_values_ref: &super::seam::SigmaValues, which_sigma_ref: &super::seam::MaterialMap, th: &super::seam::ThermalState, flux: &[f64], k_eff: f64, boron: f64) -> super::error::Result<BoronEigenSolution> { /* ... */ }
```

#### Function `eigensolve_cold`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Robust cold-start eigenvalue solve — Phase 0 bootstrap only.

MATLAB local function `eigsolvecold`. Builds the operator and the SA-nodal
correction from the **incoming** flux with
[`COLD_NODAL_REFINEMENTS`] refinements, **freezes** the correction, and runs
a self-normalising power iteration on a single cached sparse LU
factorisation:

```text
phi_new  = M \ (fs / k)                      (fixinfnan'd)
fs_new   = sigma_f * phi_new
k_new    = k * ‖fs_new‖₁ / ‖fs‖₁
scale so that sum(fs_new) matches its initial value
```

converged when the fission-source residual is below
[`COLD_FISSION_SOURCE_TOL`] and the `k_eff` residual below
[`COLD_K_EFF_TOL`].

`nodal_terms` is carried across calls so the correction warm-starts as the
bootstrap's flux and T-H converge.

# Note

Unlike [`eigensolve_at_boron`], this does **not** set `params.innertol` —
it does not call the production solver at all.

# Errors

Propagates cross-section feedback failures and
[`CouplingError::Singular`] if the frozen operator cannot be factorised.

# Panics

Through the [`seam`] stubs until `nodal/` lands.

```rust
pub fn eigensolve_cold(params: &super::seam::CaseParams, geometry: &super::seam::CoreGeometry, sigma_values_ref: &super::seam::SigmaValues, which_sigma_ref: &super::seam::MaterialMap, th: &super::seam::ThermalState, flux: &[f64], k_eff: f64, boron: f64, nodal_terms: &super::seam::NodalTerms) -> super::error::Result<ColdEigenSolution> { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_CRIT_TOL`

Default `|k_eff - 1|` tolerance of the critical state \[-\].

```rust
pub const DEFAULT_CRIT_TOL: f64 = 1.0e-5;
```

#### Constant `DEFAULT_BORON_WORTH_SLOPE`

Secant seed for the boron worth, `dk/db` \[1/ppm\] — a typical PWR value.

```rust
pub const DEFAULT_BORON_WORTH_SLOPE: f64 = -9.0e-5;
```

#### Constant `MAX_COUPLED_ITERATIONS`

Phase-2 outer-iteration cap.

```rust
pub const MAX_COUPLED_ITERATIONS: usize = 40;
```

#### Constant `MAX_SECANT_ITERATIONS`

Phase-1 secant-iteration cap.

```rust
pub const MAX_SECANT_ITERATIONS: usize = 12;
```

#### Constant `SECANT_TOL`

Phase-1 secant convergence tolerance on `|k_eff - 1|` \[-\].

Tighter than [`DEFAULT_CRIT_TOL`] because the frozen-T-H eigensolves are
cheap and the slope measurement wants a clean bracket.

```rust
pub const SECANT_TOL: f64 = 2.0e-6;
```

#### Constant `SANE_K_EFF`

Sane-range guard on `k_eff` in Phases 1 and 2 \[-\].

```rust
pub const SANE_K_EFF: (f64, f64) = _;
```

#### Constant `SANE_K_EFF_BOOTSTRAP`

Sane-range guard on `k_eff` during the Phase-0 bootstrap \[-\], deliberately
wider than [`SANE_K_EFF`].

```rust
pub const SANE_K_EFF_BOOTSTRAP: (f64, f64) = _;
```

#### Constant `MAX_BOOTSTRAP_ITERATIONS`

Phase-0 bootstrap iteration cap.

```rust
pub const MAX_BOOTSTRAP_ITERATIONS: usize = 30;
```

#### Constant `SEARCH_INNER_TOL`

Inner tolerance forced on the Phase-1/2 eigensolves \[-\].

Tight, for a sub-ppm-accurate critical `k_eff`.

```rust
pub const SEARCH_INNER_TOL: f64 = 1.0e-8;
```

#### Constant `COLD_NODAL_REFINEMENTS`

Nodal-correction refinements in [`eigensolve_cold`].

```rust
pub const COLD_NODAL_REFINEMENTS: usize = 3;
```

#### Constant `COLD_MAX_POWER_ITERATIONS`

Power-iteration cap in [`eigensolve_cold`].

```rust
pub const COLD_MAX_POWER_ITERATIONS: usize = 8000;
```

#### Constant `COLD_FISSION_SOURCE_TOL`

Fission-source tolerance of the cold power iteration \[-\].

```rust
pub const COLD_FISSION_SOURCE_TOL: f64 = 1.0e-8;
```

#### Constant `COLD_K_EFF_TOL`

`k_eff` tolerance of the cold power iteration \[-\].

```rust
pub const COLD_K_EFF_TOL: f64 = 1.0e-9;
```

## Module `cross_section_feedback`

Cross-section feedback — how the thermal-hydraulic state moves the
cross sections.

# Provenance

Translated from `sigmavalupd3d.m` and `sigmavalupd3d_handler.m` in Than Yan
Ren's (SNRSI) BEDOK MATLAB snapshot (`BEDOKfiles.zip`, sha256
`e45cd6f57be2087c…`, received 2026-08-05). Original author: **Than Yan
Ren**, Singapore Nuclear Research and Safety Institute. Translated with
permission; see `docs/bedok-port-scoping.md` §6.

# What the feedback model is

Every cross section is a **linear function of each feedback variable about
a tabulated reference point**, and the channels compose by summation:

```text
Sigma(node) = Sigma_ref(composition) + sum_channels  dSigma/dx * (x(node)^m - x_ref^m)
```

with `m = 1` for boron, moderator temperature, coolant temperature, coolant
density and rod fraction, and `m = 0.5` for the Doppler fuel temperature —
the usual square-root Doppler law. The channels are applied in a fixed
order, each consuming the previous channel's output, so the composition is
sequential rather than parallel; that ordering is preserved exactly.

# The table-compaction side effect

[`apply_feedback_channel`] does two things at once, and the second is easy
to miss: it applies one feedback channel **and** it rewrites the material
map. On the way in, `which_sigma` holds composition ids and the tables have
one row per composition; on the way out, `which_sigma` holds a row index
into a table with one row **per non-void node**. Every later channel
therefore reads per-node rows, while the derivative tables stay indexed by
composition throughout (they come from `which_sigma_ref`, never from the
running map).

```rust
pub mod cross_section_feedback { /* ... */ }
```

### Types

#### Enum `FeedbackVariable`

Values of one feedback variable — a scalar applied everywhere, or one value
per spatial node.

MATLAB writes this as `if isscalar(currval); currval = currval*ones(es,1);`;
enum dispatch makes the two cases explicit instead.

```rust
pub enum FeedbackVariable {
    Uniform(f64),
    PerNode(Vec<f64>),
}
```

##### Variants

###### `Uniform`

One value for the whole core — how boron enters \[ppm\].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `PerNode`

One value per spatial node, `grid.nodes()` entries, indexed
`Grid::index(0, ix, iy, iz)`. Temperatures \[K\], densities \[g/cm³\],
rod fractions \[-\].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<f64>` |  |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FeedbackVariable { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

### Functions

#### Function `real_power`

**Attributes:**

- `MustUse { reason: None }`

`real(x^m)` with MATLAB's complex-power semantics.

MATLAB evaluates `x^m` for negative real `x` and non-integer `m` in the
complex plane, `x^m = |x|^m * exp(i*m*pi)`, and `real()` then keeps
`|x|^m * cos(m*pi)`. For `m = 1` that is `x` itself; for the Doppler
`m = 0.5` a negative temperature collapses to (numerically) zero rather
than producing NaN, which is why the MATLAB wraps every power in `real()`.

Reproduced rather than replaced by `x.powf(m)`, which would return NaN for
a negative base and change the behaviour on any node that goes unphysical.

```rust
pub fn real_power(x: f64, m: f64) -> f64 { /* ... */ }
```

#### Function `apply_feedback_channel`

Apply one feedback channel and compact the cross-section table to one row
per node.

MATLAB `sigmavalupd3d.m`:
`[sigmavalues, whichsigma] = sigmavalupd3d(params, sigmavaluesold,
whichsigmaold, whichsigmaref, deltasigmavalues, currval, m)`.

# Arguments

- `sigma_values_old` — cross sections to perturb. On the first channel these
  are the case's per-composition tables; afterwards they are the previous
  channel's per-node table.
- `which_sigma_old` — the map that indexes `sigma_values_old`: composition
  ids on the first channel, per-node row indices afterwards.
- `which_sigma_ref` — the case's composition map, which always indexes
  `delta`.
- `delta` — derivatives with respect to this channel's variable, one row per
  composition.
- `current_value` — the variable's present value.
- `exponent` — the `m` of `x^m`; MATLAB `varargin{1}`, default 1.

# Returns

The perturbed cross sections (one row per non-void node, counted in
`ix, iy, iz` order) and the rewritten material map.

# Note on the feedback tables

The returned [`SigmaValues`] has **empty**
[`feedback`](SigmaValues::feedback) tables, matching the MATLAB: the
function builds a fresh `sigmavalues` struct and never copies the
`.boron` / `.fueltemp` / … sub-structs across. The handler is unaffected
because it always tests and reads them on the *reference* struct.

# Errors

[`super::error::CouplingError::NotANumber`] from the `pauseonnan` guards,
with the MATLAB's column semantics (see [`pause_on_nan`]).

```rust
pub fn apply_feedback_channel(params: &super::seam::CaseParams, sigma_values_old: &super::seam::SigmaValues, which_sigma_old: &super::seam::MaterialMap, which_sigma_ref: &super::seam::MaterialMap, delta: &super::seam::FeedbackTable, current_value: &FeedbackVariable, exponent: f64) -> super::error::Result<(super::seam::SigmaValues, super::seam::MaterialMap)> { /* ... */ }
```

#### Function `control_rod_fraction`

Fraction of each node covered by an inserted control rod \[-\], `0` (clear)
to `1` (fully rodded).

The control-rod part of MATLAB `sigmavalupd3d_handler.m` (lines 40–75).
Bank tip position is `crodbtm + crod*crodstep` \[cm\] measured from the
bottom of the column; nodes entirely above the tip are fully rodded, the
node straddling the tip gets the partial fraction, and nodes below it are
clear.

# Unfinished in the MATLAB — uninitialised `rodlvl`

The search for the straddling node,

```text
for iz=1:maxiz
    if sum(Lz(idx+1:idx+iz))>rodpos(rod); rodlvl=iz; break; end
end
```

**never assigns `rodlvl` when the bank tip sits at or above the top of the
column** (a fully withdrawn bank, exactly the end state of a rod-ejection
transient). MATLAB then either errors with an undefined variable — if this
is the first rodded column — or, far worse, silently reuses the `rodlvl`
**left over from the previous `(ix,iy)` column**, producing a rod fraction
for a rod that is not there.

Translated as-is: `last_rod_level` carries across columns exactly as the
MATLAB workspace variable does, and a first-column occurrence returns
[`super::error::CouplingError::MissingCaseData`] where MATLAB would raise
its undefined-variable error. **Not fixed** — see
`docs/bedok-port-scoping.md` §1.0.

# Deviation — the `rodfrac.csv` dump

`sigmavalupd3d_handler.m:71` calls `writematrix(rodfrac,'rodfrac.csv')`
**unconditionally**, inside the feedback update — so the MATLAB rewrites
that file on every cross-section update, including once per Picard pass per
transient time step. It is a leftover debug dump, not an output; no file is
written here.

# Errors

[`super::error::CouplingError::MissingCaseData`] if the straddling-node
search fails before any column has ever set a level.

```rust
pub fn control_rod_fraction(params: &super::seam::CaseParams, geometry: &super::seam::CoreGeometry) -> super::error::Result<Vec<f64>> { /* ... */ }
```

#### Function `update_cross_sections`

Apply every feedback channel the case defines, in the MATLAB's order.

MATLAB `sigmavalupd3d_handler.m`:
`[sigmavalues, whichsigma] = sigmavalupd3d_handler(params, geometry,
sigmavaluesref, whichsigmaref, th)`.

# Order — and why it is load-bearing

Channels are applied strictly in this sequence, each consuming the previous
one's output: **boron → Doppler fuel temperature → moderator temperature →
coolant temperature → coolant density → control rods**. Because each
perturbation is linear about the *reference* value and the derivative tables
are always read at the composition id, the sum is order-independent in exact
arithmetic — but the floating-point accumulation is not, and neither are the
clamps applied afterwards. The order is preserved.

# Post-processing clamps

After all channels:

1. Negative fission (`f`), power (`fp`) and scattering entries are zeroed.
2. Any composition whose absorption `Sigma_tot - sum_to Sigma_s(:,g)` has
   gone negative has its **within-group scattering** reduced by the
   shortfall, forcing the absorption to exactly zero.

Neither `tot` nor the within-group scattering is clamped directly, which is
what step 2 exists to compensate for.

# Arguments

- `sigma_values_ref` — the case's per-composition tables **and** the
  derivative tables. Never modified.
- `which_sigma_ref` — the case's composition map. Never modified.
- `th` — the thermal-hydraulic state the feedback is evaluated at.

# Errors

[`super::error::CouplingError::NotANumber`] from the `pauseonnan` guards;
[`super::error::CouplingError::MissingCaseData`] if a `modtemp` channel is
declared but `th.mod_temp` is absent, or from
[`control_rod_fraction`].

```rust
pub fn update_cross_sections(params: &super::seam::CaseParams, geometry: &super::seam::CoreGeometry, sigma_values_ref: &super::seam::SigmaValues, which_sigma_ref: &super::seam::MaterialMap, th: &super::seam::ThermalState) -> super::error::Result<(super::seam::SigmaValues, super::seam::MaterialMap)> { /* ... */ }
```

## Module `error`

Failures the coupled drivers can report.

# Why this is not [`crate::BedokError`]

The crate-level error enum does not yet carry the sparse-linear-algebra and
coupled-convergence cases the drivers need, and the reference translation is
being written by several hands at once, so this module keeps its additions
local rather than racing another author for `src/error.rs`. Merging
[`CouplingError`] into [`crate::BedokError`] is a one-way conversion away
(every variant here is either new or a wrapper) and is a deliberate
follow-up, not an oversight.

# Provenance

Support code for the translation of Than Yan Ren's (SNRSI) BEDOK MATLAB
snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`). The MATLAB has no
error type: it calls `error()` / `warning()` inline (`pauseonnan.m`,
`criticalboron_xyz:badeig`, `thdiffusion_solvertimexyz:diverged`). Each
variant below names the MATLAB identifier it stands in for.

```rust
pub mod error { /* ... */ }
```

### Types

#### Type Alias `Result`

Result alias for the coupled drivers.

```rust
pub type Result<T> = std::result::Result<T, CouplingError>;
```

#### Enum `CouplingError`

Everything the coupled neutronics/thermal-hydraulics drivers can fail on.

```rust
pub enum CouplingError {
    SparseAssembly {
        reason: String,
    },
    Singular {
        reason: String,
    },
    NotANumber {
        field: &'static str,
    },
    EigenvalueOutOfRange {
        k_eff: f64,
        boron: f64,
    },
    NoTimeData,
    MissingCaseData {
        what: &'static str,
    },
    Bedok(crate::error::BedokError),
    Io(std::io::Error),
}
```

##### Variants

###### `SparseAssembly`

A sparse operator could not be assembled — shape mismatch between terms,
index overflow, or allocation failure.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `reason` | `String` | What went wrong. |

###### `Singular`

The sparse LU factorisation failed: the operator is structurally or
numerically singular.

In BEDOK this normally means the diffusion operator has an empty row —
an all-void plane, or a `whichsigma` map that disagrees with the
geometry.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `reason` | `String` | What the factorisation reported. |

###### `NotANumber`

A field held a NaN or a complex value where the MATLAB `pauseonnan.m`
would have raised `'NaN occured'`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `&'static str` | Which field tripped the guard. |

###### `EigenvalueOutOfRange`

An eigenvalue came back outside the physically sane band the critical
boron search insists on — MATLAB `criticalboron_xyz:badeig` and
`criticalboron_xyz:badboot`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | The offending eigenvalue \[-\]. |
| `boron` | `f64` | Boron concentration it was computed at \[ppm\]. |

###### `NoTimeData`

The transient case supplied neither an end time nor a time grid —
MATLAB `thdiffusion_solvertimexyz:notimedata`.

###### `MissingCaseData`

Something the case must supply was missing.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which field. |

###### `Bedok`

A crate-level failure — grid indexing, fixture loading, I/O.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::error::BedokError` |  |

###### `Io`

Writing a CSV output failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::io::Error` |  |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
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
    fn from(source: BedokError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: std::io::Error) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

## Module `seam`

The interfaces the coupled drivers call into — **provisional**.

# Provenance

Translated alongside Than Yan Ren's (SNRSI) BEDOK MATLAB snapshot
(`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`). Original author: **Than Yan
Ren**, Singapore Nuclear Research and Safety Institute. Translated with
permission; see `docs/bedok-port-scoping.md` §6.

# Why this module exists, and what must happen to it

The coupled drivers in this directory are the *callers* of the semi-analytic
nodal solver and the thermal-hydraulics solver. Those two live in
[`crate::reference::nodal`] and [`crate::reference::th`] and were being
written at the same time as this module, so the call sites here are declared
against the shapes the MATLAB actually passes, and every function body is
[`todo!`].

**Nothing in this module is a design proposal.** Each item names the MATLAB
function it stands for and the module that will own it. When `nodal/` and
`th/` land, the declarations here are deleted and the drivers import theirs;
the only work is reconciling names and field layouts. The state types
(`CaseParams`, `CoreGeometry`, `ThermalState`, `SigmaValues`) are shared by
all three directories and will end up in one place — most naturally next to
[`crate::reference::grid`], which already owns [`Grid`] and
[`crate::reference::grid::Geometry`].

# Conventions used throughout

- **State vectors** (`philen = nodes * ngroups` entries) are indexed through
  [`Grid::index`]. Never index them by hand.
- **Per-node fields** (`es = nodes` entries) use the same rule with `g = 0`.
- **Per-`(ix,iy)` maps** (rod banks, `zhis`) are indexed `ix*ny + iy`.
- **Lengths are centimetres, temperatures kelvin, densities g/cm³, power
  watts, time seconds, boron ppm** — the units the MATLAB and the benchmark
  specifications both use. `uom` types are deliberately absent from the
  reference path so the arithmetic stays line-for-line comparable
  (`docs/bedok-port-scoping.md` §7).

```rust
pub mod seam { /* ... */ }
```

### Types

#### Struct `CaseParams`

Run controls and case data — the MATLAB `params` struct.

MATLAB reads this struct with `isfield`, so every optional control is an
[`Option`] here and the defaults are applied at the point of use, exactly
where the MATLAB applies them.

# Ownership

Will be owned by the case layer ([`crate::reference::cases`]) once it
lands; the fields below are only those the coupling layer reads.

```rust
pub struct CaseParams {
    pub grid: crate::reference::grid::Grid,
    pub n_components: usize,
    pub fuel_max_ir: usize,
    pub fuel_n: usize,
    pub boron: f64,
    pub fuel_temp_avg_init: f64,
    pub cool_temp_avg_init: f64,
    pub cool_den_avg_init: f64,
    pub fuel_temp_tol: Option<f64>,
    pub flux_tol: Option<f64>,
    pub th_max_iter: Option<usize>,
    pub th_relax: Option<f64>,
    pub inexact_inner: Option<f64>,
    pub inexact_eta: Option<f64>,
    pub inner_tol: Option<f64>,
    pub crit_tol: Option<f64>,
    pub t_end: Option<f64>,
    pub t_grid: Option<Vec<f64>>,
    pub time_picard: Option<usize>,
    pub nodal_upd_time: Option<usize>,
    pub time_scheme: Option<KineticsScheme>,
    pub freq_iter: Option<usize>,
    pub freq_mode: Option<FrequencyMode>,
    pub out_prefix: Option<String>,
    pub velocities: Vec<f64>,
    pub beta_dnp: Vec<f64>,
    pub lambda_dnp: Vec<f64>,
    pub eject_duration: Option<f64>,
    pub steady_file: Option<String>,
    pub debug_dump: bool,
    pub output_dir: Option<String>,
    pub jfnk_precon: Option<f64>,
    pub jfnk_rel: Option<f64>,
    pub jfnk_verb: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `crate::reference::grid::Grid` | Node grid and energy-group count. MATLAB `params.maxix/maxiy/maxiz/G`<br>via `handle3dcoords`. |
| `n_components` | `usize` | Extra state components beyond the `G` flux groups. MATLAB `params.Nc`<br>(absent → 0). All four benchmark cases set it to zero; it lengthens the<br>operators to `philenf = philen + Nc*es`. |
| `fuel_max_ir` | `usize` | Radial solution rings in the fuel rod. MATLAB `params.fuel.maxir`. |
| `fuel_n` | `usize` | Radial rings inside the fuel pellet proper. MATLAB `params.fuel.fueln`. |
| `boron` | `f64` | Boron concentration \[ppm\] the cross-section feedback is evaluated at.<br>MATLAB `params.boron`. |
| `fuel_temp_avg_init` | `f64` | Initial flat fuel temperature \[K\]. MATLAB `params.fueltempavg`. |
| `cool_temp_avg_init` | `f64` | Initial flat coolant temperature \[K\]. MATLAB `params.cooltempavg`. |
| `cool_den_avg_init` | `f64` | Initial flat coolant density \[g/cm³\]. MATLAB `params.cooldenavg`. |
| `fuel_temp_tol` | `Option<f64>` | Fuel-temperature convergence tolerance \[K\] for the coupled outer loop.<br>MATLAB `params.fueltemptol`; default 0.5 K. |
| `flux_tol` | `Option<f64>` | Outer fission-source / `k_eff` tolerance \[-\]. MATLAB `params.fluxtol`;<br>default 1e-4. |
| `th_max_iter` | `Option<usize>` | Cap on coupled outer iterations. MATLAB `params.thmaxiter`; default 50. |
| `th_relax` | `Option<f64>` | Picard under-relaxation factor for the T-H feedback fields, `0 < w <= 1`.<br>MATLAB `params.threlax`; default 0.5. |
| `inexact_inner` | `Option<f64>` | Set to `Some(0.0)` to disable the Eisenstat-Walker-style inexact inner<br>tolerance. MATLAB `params.inexactinner`. |
| `inexact_eta` | `Option<f64>` | Forcing factor of the inexact inner schedule. MATLAB `params.inexacteta`;<br>default 1e-3. |
| `inner_tol` | `Option<f64>` | Inner eigenvalue-solve tolerance handed to the nodal solver. MATLAB<br>`params.innertol` — written by the coupling layer, read by<br>`sanodaldiffusion_solverxyz`. |
| `crit_tol` | `Option<f64>` | `k_eff` tolerance of the critical-boron search. MATLAB `params.crittol`;<br>default 1e-5. |
| `t_end` | `Option<f64>` | End of the transient \[s\]. MATLAB `params.tend`. |
| `t_grid` | `Option<Vec<f64>>` | Time points of the transient \[s\]. MATLAB `params.tgrid`. |
| `time_picard` | `Option<usize>` | Feedback Picard passes per time step. MATLAB `params.timepicard`;<br>default 1. |
| `nodal_upd_time` | `Option<usize>` | Update the SA-nodal correction every N time steps; 0 freezes it at the<br>steady state. MATLAB `params.nodalupdtime`; default 1. |
| `time_scheme` | `Option<KineticsScheme>` | Kinetics scheme. MATLAB `params.timescheme`; default<br>[`KineticsScheme::ExponentialTransform`]. |
| `freq_iter` | `Option<usize>` | Flux solves per time step for the exponential-transform scheme: one<br>predictor plus `freq_iter - 1` frequency correctors. MATLAB<br>`params.freqiter`; default 2, floored at 1. |
| `freq_mode` | `Option<FrequencyMode>` | Whether the exponential-transform frequencies are per-group-global or<br>per-node. MATLAB `params.freqmode`; default<br>[`FrequencyMode::Global`]. |
| `out_prefix` | `Option<String>` | Prefix of the transient output CSV files. MATLAB `params.outprefix`;<br>default `"neacrpa2t"`. |
| `velocities` | `Vec<f64>` | Prompt neutron group velocities \[cm/s\]. MATLAB `params.velocities`. |
| `beta_dnp` | `Vec<f64>` | Delayed-neutron fractions, one per precursor family \[-\]. MATLAB<br>`params.beta_dnp`. |
| `lambda_dnp` | `Vec<f64>` | Delayed-neutron decay constants \[1/s\]. MATLAB `params.lambda_dnp`. |
| `eject_duration` | `Option<f64>` | Control-assembly ejection duration \[s\]. MATLAB<br>`params.ejectduration`; required only when the case ejects a bank. |
| `steady_file` | `Option<String>` | Path of the `.mat`-equivalent steady-state cache. MATLAB<br>`params.steadyfile`.<br><br># Not implemented in the port<br><br>The MATLAB `load`/`save` of a `.mat` file has no translation here, so<br>the field is carried for round-tripping the case data and **not acted<br>on**: [`super::transient::solve_coupled_transient`] and<br>[`super::critical_boron::search_critical_boron`] always run the steady<br>solve. Reinstating the cache means choosing a serialisation format,<br>which is a decision for the crate, not for the translation. |
| `debug_dump` | `bool` | Write the MATLAB debug CSV dumps. MATLAB `params.debugdump`.<br><br>Carried but **not acted on** by the coupling layer, which writes no<br>files at all (see [`super::steady::solve_coupled_steady`]). The nodal<br>and T-H layers have their own `debugdump` blocks. |
| `output_dir` | `Option<String>` | Directory a caller may write the returned histories into. Has no MATLAB<br>counterpart — MATLAB writes into the working directory — and nothing in<br>the coupling layer reads it. Present so that a future CSV writer has a<br>place to be told where to put its output rather than defaulting to the<br>caller's working directory. |
| `jfnk_precon` | `Option<f64>` | JFNK preconditioner switch. MATLAB `params.jfnkprecon`.<br><br># Dead control in the snapshot<br><br>`main_exec_diff3d.m:19-21` and `run_neacrpd1t.m:11` set<br>`params.jfnkprecon`, `params.jfnkrel` and `params.jfnkverb`, and<br>`main_exec_diff3d.m:54-61` documents `params.ptc` and<br>`params.jfnk_max_iter` as controls of `driftflux_solverstatic1d.m`.<br>**No file in the snapshot reads any of them, and<br>`driftflux_solverstatic1d.m` is not in the snapshot at all.** The<br>Jacobian-free Newton-Krylov solver those controls belong to is<br>therefore missing upstream, not omitted here. Carried as a field so the<br>case data round-trips and so the gap is recorded where a reader meets<br>it; nothing in this crate reads it either. |
| `jfnk_rel` | `Option<f64>` | JFNK relaxation factor. MATLAB `params.jfnkrel`. Dead control — see<br>[`jfnk_precon`](Self::jfnk_precon). |
| `jfnk_verb` | `Option<f64>` | JFNK verbosity. MATLAB `params.jfnkverb`. Dead control — see<br>[`jfnk_precon`](Self::jfnk_precon). |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CaseParams { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Enum `KineticsScheme`

Time-integration scheme for the flux and the delayed-neutron precursors.

MATLAB `params.timescheme`. Enum dispatch rather than the MATLAB's integer
switch, so a new scheme forces every match site to be revisited.

```rust
pub enum KineticsScheme {
    ExponentialTransform,
    ImplicitEuler,
}
```

##### Variants

###### `ExponentialTransform`

`timescheme = 1` (default). Exponential-transform implicit Euler for the
flux with per-node or per-group frequencies, and analytic precursor
integration assuming a linearly varying transformed fission source over
the step — the scheme of the nodal program Ants (A. Rintala,
U. Lauranto, *Ann. Nucl. Energy* **190** (2023) 109868, Eqs. (3)–(13)).

###### `ImplicitEuler`

`timescheme = 0`. Plain first-order implicit Euler for both flux and
precursors; the legacy scheme.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> KineticsScheme { /* ... */ }
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
    fn default() -> KineticsScheme { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &KineticsScheme) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `FrequencyMode`

How the exponential-transform frequencies are computed.

MATLAB `params.freqmode`.

```rust
pub enum FrequencyMode {
    Global,
    Node,
}
```

##### Variants

###### `Global`

`'global'` (default): one amplitude frequency per energy group, uniform
in space, taken from the volume-integrated group flux. Robust for
super-prompt rod ejections.

###### `Node`

`'node'`: per-node, per-group frequencies as written in the Ants paper.
Yan Ren records this as **unstable in super-prompt HZP rod ejections** —
node-wise frequency noise near the ejected channel feeds back through
the nearly singular prompt operator.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FrequencyMode { /* ... */ }
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
    fn default() -> FrequencyMode { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FrequencyMode) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CoreGeometry`

Core geometry as the coupled drivers need it — the MATLAB `geometry` struct.

Wraps [`Geometry`] (node sizes and volumes, already owned by
[`crate::reference::grid`]) and adds the fuel-rod, control-rod and
axial-block data the coupling layer reads.

# Ownership

Provisional. [`crate::reference::cases`] builds these; `nodal/` and `th/`
read them. Expect the extra fields below to be folded into
[`Geometry`] or into a case-layer type.

```rust
pub struct CoreGeometry {
    pub base: crate::reference::grid::Geometry,
    pub fuel: FuelGeometry,
    pub crod: Vec<f64>,
    pub crod_banks: Vec<usize>,
    pub crod_btm: f64,
    pub crod_step: f64,
    pub crod_eject: Option<usize>,
    pub crod_eject_to: Option<f64>,
    pub zlows: Vec<usize>,
    pub zhis: Vec<usize>,
    pub zscale: usize,
    pub nodal_coeffs: NodalCoefficients,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `base` | `crate::reference::grid::Geometry` | Node sizes, volumes and the grid. MATLAB `geometry.Lx/Ly/Lz/Vi`.<br><br># Length of `lz` — a discrepancy to reconcile<br><br>The coupling layer indexes `base.lz` as **one value per spatial node**<br>(`Grid::index(0, ix, iy, iz)`), because that is what the MATLAB does:<br>`sigmavalupd3d_handler.m:57` sums `Lz(idx+1 : idx+iz)` off a flat node<br>offset, and `neacrpa2t.m:56` fills `geometry.Lz` with<br>`maxix*maxiy*maxiz` entries. [`Geometry::lz`] currently documents itself<br>as "one per z index"; the two must be made to agree before either is<br>trusted, and the MATLAB is the authority. |
| `fuel` | `FuelGeometry` | Fuel-rod radial geometry. MATLAB `geometry.fuel`. |
| `crod` | `Vec<f64>` | Control-bank positions \[steps withdrawn\], one per bank. MATLAB<br>`geometry.crod`. Mutated during the transient as the bank moves. |
| `crod_banks` | `Vec<usize>` | Which control bank covers each `(ix,iy)` column, 0 for none; indexed<br>`ix*ny + iy`. MATLAB `geometry.crodbanks`. |
| `crod_btm` | `f64` | Axial position of a fully inserted bank tip \[cm\]. MATLAB<br>`geometry.crodbtm`. |
| `crod_step` | `f64` | Length of one control-rod step \[cm\]. MATLAB `geometry.crodstep`. |
| `crod_eject` | `Option<usize>` | Index of the ejected bank into [`crod`](Self::crod), **1-based as in the<br>MATLAB**; 0 or [`None`] means the case has no rod motion (NEACRP D1).<br>MATLAB `geometry.crodeject`. |
| `crod_eject_to` | `Option<f64>` | Final position of the ejected bank \[steps\]. MATLAB<br>`geometry.crodejectto`. |
| `zlows` | `Vec<usize>` | Lowest fuel-bearing axial node of each column, **1-based**; indexed<br>`ix*ny + iy`. MATLAB `geometry.zlows`. |
| `zhis` | `Vec<usize>` | Highest fuel-bearing axial node of each column, **1-based**; indexed<br>`ix*ny + iy`. MATLAB `geometry.zhis`. The transient uses it to find each<br>channel's outlet node. |
| `zscale` | `usize` | Mesh layers per axial benchmark block. MATLAB `geometry.zscale`. |
| `nodal_coeffs` | `NodalCoefficients` | Semi-analytic nodal coefficients `A,B,E,F,G,H`, rebuilt whenever the<br>operators are. MATLAB `geometry.nodalcoeffs` (from `calc_ABEFGHxyz`). |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoreGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `FuelGeometry`

Radial fuel-rod geometry — the MATLAB `geometry.fuel` struct.

```rust
pub struct FuelGeometry {
    pub which_k: Vec<usize>,
    pub ctr: Vec<f64>,
    pub fuel_rad: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `which_k` | `Vec<usize>` | Conductivity-material tag of each radial ring: 0 = gap, 1 = fuel,<br>2 = cladding. MATLAB `geometry.fuel.whichk`, length<br>[`CaseParams::fuel_max_ir`]. |
| `ctr` | `Vec<f64>` | Radius of each ring centre \[cm\]. MATLAB `geometry.fuel.Ctr`. |
| `fuel_rad` | `f64` | Fuel pellet radius \[cm\]. MATLAB `geometry.fuel.fuelrad`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `ThermalState`

Thermal-hydraulic state of the core — the MATLAB `th` struct.

# Ownership

Provisional; [`crate::reference::th`] will own this. Only the fields the
coupling layer reads or writes are declared. The T-H solver carries several
more (`th.coolant.enth`, `press`, `alphag`, `vm`, `ldens`, `gdens`,
`quality`, `th.linpwrdens`) that the CHF call needs.

```rust
pub struct ThermalState {
    pub fuel_temp_avg: Vec<f64>,
    pub fuel_temp_doppler: Vec<f64>,
    pub fuel_temp: Vec<f64>,
    pub n_solution_ids: usize,
    pub mod_temp: Option<Vec<f64>>,
    pub coolant: CoolantState,
    pub heat_flux: Vec<f64>,
    pub power_ratio: f64,
    pub max_power: f64,
    pub n_fuel_pins: f64,
    pub coolant_heat_fraction: f64,
    pub flow_rate: f64,
    pub flow_dir: f64,
    pub inlet_temp_schedule: Option<InletTemperatureSchedule>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_temp_avg` | `Vec<f64>` | Volume-average fuel temperature per node \[K\]. MATLAB `th.fueltempavg`.<br>Length `nodes`. |
| `fuel_temp_doppler` | `Vec<f64>` | Effective Doppler fuel temperature per node \[K\], the quantity the<br>cross-section feedback uses (with a square-root law). MATLAB<br>`th.fueltempdoppler`. Length `nodes`. |
| `fuel_temp` | `Vec<f64>` | Radial fuel-rod temperature profile \[K\], `nodes * n_solution_ids`,<br>indexed `node * n_solution_ids + id`. MATLAB `th.fueltemp`. |
| `n_solution_ids` | `usize` | Number of radial solution ids per node — `maxir` plus one node per<br>material interface. MATLAB `maxid`, computed in `thdiffusion_solverxyz`. |
| `mod_temp` | `Option<Vec<f64>>` | Moderator temperature per node \[K\], used only when the case supplies a<br>`modtemp` feedback table. MATLAB `th.modtemp`. |
| `coolant` | `CoolantState` | Coolant state. |
| `heat_flux` | `Vec<f64>` | Wall heat flux per node \[W/cm²\]. MATLAB `th.heatflux`. Length `nodes`. |
| `power_ratio` | `f64` | Core power relative to [`max_power`](Self::max_power) \[-\]. MATLAB<br>`th.powratio`; the transient rescales it every step. |
| `max_power` | `f64` | Rated core thermal power \[W\]. MATLAB `th.maxpow`. |
| `n_fuel_pins` | `f64` | Fuel pins per node. MATLAB `th.nfuelpin`. |
| `coolant_heat_fraction` | `f64` | Fraction of fission energy deposited directly in the coolant \[-\].<br>MATLAB `th.coolheatfrac`. |
| `flow_rate` | `f64` | Area-averaged coolant mass flux \[g/s/cm²\]. MATLAB `th.flowrate`. |
| `flow_dir` | `f64` | Flow direction: `+1` upwards, `-1` downwards. MATLAB `th.flowdir`. |
| `inlet_temp_schedule` | `Option<InletTemperatureSchedule>` | Time-dependent inlet-temperature forcing. MATLAB `th.inlettemp_t`, a<br>function handle the case supplies; absent for cases with a fixed inlet. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermalState { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `CoolantState`

Coolant channel state — the MATLAB `th.coolant` struct.

```rust
pub struct CoolantState {
    pub temps: Vec<f64>,
    pub dens: Vec<f64>,
    pub inlet_temp: f64,
    pub inlet_press: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temps` | `Vec<f64>` | Coolant temperature per node \[K\]. MATLAB `th.coolant.temps`. |
| `dens` | `Vec<f64>` | Coolant density per node \[g/cm³\]. MATLAB `th.coolant.dens`. |
| `inlet_temp` | `f64` | Channel inlet temperature \[K\]. MATLAB `th.coolant.inlettemp`. The<br>transient overwrites it each step from<br>[`ThermalState::inlet_temp_schedule`]. |
| `inlet_press` | `f64` | Channel inlet pressure \[MPa\]. MATLAB `th.coolant.inletpress`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoolantState { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Enum `InletTemperatureSchedule`

Prescribed inlet-temperature history — the MATLAB `th.inlettemp_t` handle.

Enum dispatch, per the workspace rule against trait objects: the set of
forcings is closed and known from the benchmark cases.

# Ownership

Provisional; the case layer defines the forcings, so the variants will grow
there. Only the D1 forcing exists in the snapshot.

```rust
pub enum InletTemperatureSchedule {
    Constant,
    NeacrpD1ColdWater {
        inlet_pressure: f64,
        saturated_liquid_enthalpy: f64,
    },
}
```

##### Variants

###### `Constant`

No time dependence — the inlet stays at
[`CoolantState::inlet_temp`].

###### `NeacrpD1ColdWater`

NEACRP D1 cold-water injection, `neacrpd1t.m` (spec Fig. 6.1): the
inlet subcooling doubles with a 2.5 s time constant,
`dH(t) = 46.52*(2 - exp(-0.4 t))` kJ/kg below the saturated-liquid
enthalpy at the constant core pressure, converted to a temperature with
an IAPWS-IF97 `(p,h)` flash.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `inlet_pressure` | `f64` | Constant core pressure \[MPa\]. MATLAB `th.coolant.inletpress`. |
| `saturated_liquid_enthalpy` | `f64` | Saturated-liquid enthalpy at that pressure \[kJ/kg\]. MATLAB<br>`hsat0 = IAPWS_IF97('h1_pT', Pin, IAPWS_IF97('Tsat_p', Pin))`. |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, t: f64, constant_inlet_temp: f64) -> f64 { /* ... */ }
  ```
  Inlet temperature \[K\] at time `t` \[s\].

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> InletTemperatureSchedule { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `MaterialMap`

Which composition (or, after a feedback pass, which compacted table row)
each spatial node uses — the MATLAB `whichsigma` array.

Zero means "no material": a void node outside the core, skipped by every
loop that walks this map.

# The two meanings, and why they share a type

The MATLAB overloads this array, and the translation keeps the overload
because the feedback chain depends on it (see
[`super::cross_section_feedback`]):

- As handed in by the case (`whichsigmaref`), entries are **composition
  ids**, 1-based, indexing the benchmark's material tables.
- As returned by a feedback update, entries are **row indices into the
  compacted per-node table**, 1-based, counting non-void nodes in
  `ix, iy, iz` order.

```rust
pub struct MaterialMap {
    pub grid: crate::reference::grid::Grid,
    pub ids: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `crate::reference::grid::Grid` | The grid it is defined on. |
| `ids` | `Vec<usize>` | One entry per spatial node, indexed `Grid::index(0, ix, iy, iz)`. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(grid: Grid) -> Self { /* ... */ }
  ```
  A map of `grid.nodes()` entries, all void.

- ```rust
  pub fn at(self: &Self, ix: usize, iy: usize, iz: usize) -> usize { /* ... */ }
  ```
  The id at `(ix, iy, iz)`, all 0-based; 0 means void.

- ```rust
  pub fn set(self: &mut Self, ix: usize, iy: usize, iz: usize, id: usize) { /* ... */ }
  ```
  Set the id at `(ix, iy, iz)`, all 0-based.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MaterialMap { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `SigmaValues`

Multigroup cross sections plus the feedback derivative tables — the MATLAB
`sigmavalues` struct.

Rows are compositions on the way in (`sigmavaluesref`) and per-node table
entries on the way out of a feedback pass. Row count is
[`n_rows`](Self::n_rows).

# Units

`tot`, `f` and `s` are macroscopic cross sections \[1/cm\]; `f` is
`nu*Sigma_f`. `fp` is `kappa*Sigma_f` \[J/cm\], the power-producing
operator. `nu` \[-\] and `chi` \[-\] are the neutron yield and fission
spectrum.

```rust
pub struct SigmaValues {
    pub ngroups: usize,
    pub tot: Vec<f64>,
    pub f: Vec<f64>,
    pub fp: Vec<f64>,
    pub s: Vec<f64>,
    pub nu: Vec<f64>,
    pub chi: Vec<f64>,
    pub feedback: FeedbackTables,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ngroups` | `usize` | Energy groups. |
| `tot` | `Vec<f64>` | Total cross section, `row*ngroups + g` \[1/cm\]. |
| `f` | `Vec<f64>` | Fission production `nu*Sigma_f`, `row*ngroups + g` \[1/cm\]. |
| `fp` | `Vec<f64>` | Power production `kappa*Sigma_f`, `row*ngroups + g` \[J/cm\]. |
| `s` | `Vec<f64>` | Scattering matrix, `row*ngroups*ngroups + to*ngroups + from` \[1/cm\].<br><br>The index order follows the MATLAB `sigmavalues.s(w, to, from)`, which<br>`sigmavalupd3d_handler.m:93` fixes by computing the absorption as<br>`tot(w,g) - sum(s(w,:,g))` — a sum over destinations at fixed source<br>group `g`. |
| `nu` | `Vec<f64>` | Neutron yield per fission, `row*ngroups + g` \[-\]. |
| `chi` | `Vec<f64>` | Fission spectrum, `row*ngroups + g` \[-\]. |
| `feedback` | `FeedbackTables` | The feedback derivative tables the case supplies. |

##### Implementations

###### Methods

- ```rust
  pub fn n_rows(self: &Self) -> usize { /* ... */ }
  ```
  Number of table rows — compositions, or nodes after a feedback pass.

- ```rust
  pub fn scattering(self: &Self, row: usize, to: usize, from: usize) -> f64 { /* ... */ }
  ```
  Scattering entry `(row, to, from)`, all 0-based \[1/cm\].

- ```rust
  pub fn scattering_mut(self: &mut Self, row: usize, to: usize, from: usize) -> &mut f64 { /* ... */ }
  ```
  Mutable scattering entry `(row, to, from)`, all 0-based \[1/cm\].

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SigmaValues { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `FeedbackTables`

The set of feedback channels a case defines — the optional sub-structs of
the MATLAB `sigmavalues` (`sigmavalues.boron`, `.fueltemp`, …).

`None` is the MATLAB `isfield(...) == false`: the channel is simply not
applied.

```rust
pub struct FeedbackTables {
    pub boron: Option<FeedbackTable>,
    pub fuel_temp: Option<FeedbackTable>,
    pub mod_temp: Option<FeedbackTable>,
    pub cool_temp: Option<FeedbackTable>,
    pub cool_den: Option<FeedbackTable>,
    pub crod: Option<FeedbackTable>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `boron` | `Option<FeedbackTable>` | Boron concentration \[ppm\], linear. |
| `fuel_temp` | `Option<FeedbackTable>` | Doppler fuel temperature \[K\], square-root law (`m = 0.5`). |
| `mod_temp` | `Option<FeedbackTable>` | Moderator temperature \[K\], linear. |
| `cool_temp` | `Option<FeedbackTable>` | Coolant temperature \[K\], linear. |
| `cool_den` | `Option<FeedbackTable>` | Coolant density \[g/cm³\], linear. |
| `crod` | `Option<FeedbackTable>` | Control-rod insertion fraction \[-\], linear about zero. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FeedbackTables { /* ... */ }
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
    fn default() -> FeedbackTables { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `FeedbackTable`

Cross-section derivatives with respect to one feedback variable — the MATLAB
`deltasigmavalues` struct.

Rows are **composition ids**, always: the derivative tables are never
compacted, so `sigmavalupd3d.m` indexes them with `whichsigmaref` while it
indexes the base values with `whichsigmaold`.

```rust
pub struct FeedbackTable {
    pub ngroups: usize,
    pub reference_value: f64,
    pub tot: Vec<f64>,
    pub f: Vec<f64>,
    pub fp: Vec<f64>,
    pub s: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ngroups` | `usize` | Energy groups. |
| `reference_value` | `f64` | Value of the feedback variable the base cross sections were tabulated<br>at. MATLAB `deltasigmavalues.ref` (`ref` is a Rust keyword). |
| `tot` | `Vec<f64>` | d(total)/d(variable), `row*ngroups + g`. |
| `f` | `Vec<f64>` | d(nu*Sigma_f)/d(variable), `row*ngroups + g`. |
| `fp` | `Vec<f64>` | d(kappa*Sigma_f)/d(variable), `row*ngroups + g`. |
| `s` | `Vec<f64>` | d(scattering)/d(variable), `row*ngroups*ngroups + to*ngroups + from`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FeedbackTable { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `SigmaOperators`

The assembled multigroup operators — the MATLAB `sigma` struct from
`makesigmadfxyz.m`.

All four are square, of side
`philenf = grid.state_len() + n_components*grid.nodes()`.

# Ownership

[`crate::reference::nodal`] builds these.

```rust
pub struct SigmaOperators {
    pub tot: super::sparse::SparseMatrix,
    pub s: super::sparse::SparseMatrix,
    pub f: super::sparse::SparseMatrix,
    pub fp: super::sparse::SparseMatrix,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tot` | `super::sparse::SparseMatrix` | Total-removal operator (diagonal) \[1/cm × cm³\]. MATLAB `sigma.tot`. |
| `s` | `super::sparse::SparseMatrix` | Scattering operator. MATLAB `sigma.s`. |
| `f` | `super::sparse::SparseMatrix` | Fission production operator `chi * nu*Sigma_f`. MATLAB `sigma.f`. |
| `fp` | `super::sparse::SparseMatrix` | Power production operator `kappa*Sigma_f`. MATLAB `sigma.fp`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SigmaOperators { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `DiffusionCoefficients`

Diffusion coefficients per node and group \[cm\] — the MATLAB `DiffD` array
from `calcdiffvalues3d.m`.

# Ownership

[`crate::reference::nodal`]. Opaque to the coupling layer, which only passes
it between nodal calls.

```rust
pub struct DiffusionCoefficients {
    pub values: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `values` | `Vec<f64>` | `nx*ny*nz*ngroups` values, indexed `Grid::index(g, ix, iy, iz)`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DiffusionCoefficients { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `GradientTerms`

Interface-current terms produced alongside the diffusion operator — the
MATLAB `gradterms` from `makegradDxyz.m`.

# Ownership

[`crate::reference::nodal`]. Opaque here; passed straight into
[`calc_semi_analytic_nodal`].

```rust
pub struct GradientTerms {
    pub placeholder: (),
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `placeholder` | `()` | Placeholder for the nodal layer's own representation. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GradientTerms { /* ... */ }
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
    fn default() -> GradientTerms { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `NodalCoefficients`

Semi-analytic nodal coefficients `A, B, E, F, G, H` — the MATLAB
`calc_ABEFGHxyz.m` output, stored as `geometry.nodalcoeffs`.

# Ownership

[`crate::reference::nodal`]. Opaque here.

```rust
pub struct NodalCoefficients {
    pub placeholder: (),
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `placeholder` | `()` | Placeholder for the nodal layer's own representation. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodalCoefficients { /* ... */ }
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
    fn default() -> NodalCoefficients { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `NodalTerms`

The six per-node transverse-leakage / expansion terms carried across nodal
updates — the MATLAB `nodalterms`, a `philen x 6` array.

Warm-starting the nodal correction from the previous update is why this is
threaded through the drivers rather than rebuilt.

```rust
pub struct NodalTerms {
    pub values: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `values` | `Vec<f64>` | `state_len * 6` values, indexed `state_index*6 + term`. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(state_len: usize) -> Self { /* ... */ }
  ```
  All-zero terms, the MATLAB `zeros(philen,6)` cold start.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodalTerms { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `DiffusionSolution`

What the SA-nodal eigenvalue solver returns — the MATLAB
`sanodaldiffusion_solverxyz` output struct.

```rust
pub struct DiffusionSolution {
    pub k_eff: f64,
    pub scalar_flux: Vec<f64>,
    pub fission_source: Vec<f64>,
    pub pwrdens: Vec<f64>,
    pub residual: f64,
    pub k_eff_residual: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | Multiplication factor \[-\]. MATLAB `output.k_eff`. |
| `scalar_flux` | `Vec<f64>` | Converged scalar flux \[n/cm²/s, to an arbitrary normalisation\],<br>`state_len` entries.<br><br># Representation note<br><br>MATLAB returns `output.scalar_flux` as a **matrix** whose extra columns<br>hold the fission-source extrapolation history; every consumer in the<br>snapshot reads only `scalar_flux(:,1)` (the coupling layer explicitly,<br>`main_exec_diff3d.m` implicitly through linear indexing). Only that<br>first column is carried here. |
| `fission_source` | `Vec<f64>` | Fission source `sigma.f * phi`, `state_len` entries. |
| `pwrdens` | `Vec<f64>` | Node power density `fission_source .* Vi`, `state_len` entries. |
| `residual` | `f64` | Final fission-source residual \[-\]. MATLAB `output.residual`. |
| `k_eff_residual` | `f64` | Final `k_eff` residual \[-\]. MATLAB `output.k_eff_residual`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DiffusionSolution { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `ChfResult`

Critical-heat-flux result — the MATLAB `w3chfhottest.m` output.

# Ownership

[`crate::reference::th`]. Opaque here: `thdiffusion_solverxyz.m:191`
computes it and **never uses it** — it is not placed in the output struct,
so the call is currently dead. Translated as-is.

```rust
pub struct ChfResult {
    pub placeholder: (),
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `placeholder` | `()` | Placeholder for the thermal-hydraulics layer's own representation. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ChfResult { /* ... */ }
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
    fn default() -> ChfResult { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

### Functions

#### Function `make_sigma_operators`

**Attributes:**

- `MustUse { reason: None }`

Assemble the multigroup operators from tabulated cross sections.

MATLAB `makesigmadfxyz.m`, called as `makesigmadfxyz(params, sigmavalues,
whichsigma, 1)` — mode 1, full indices only.

# Ownership

**[`crate::reference::nodal`] owns this.** Declared here only so the
coupled drivers compile ahead of it.

# Panics

Always — the body is [`todo!`].

```rust
pub fn make_sigma_operators(params: &CaseParams, sigma_values: &SigmaValues, which_sigma: &MaterialMap) -> SigmaOperators { /* ... */ }
```

#### Function `calc_diffusion_coefficients`

**Attributes:**

- `MustUse { reason: None }`

Diffusion coefficients from the total cross sections.

MATLAB `calcdiffvalues3d.m`, default mode 1:
`D = mode / ((2*mode + 1) * Sigma_tot)`.

# Ownership

**[`crate::reference::nodal`] owns this.**

# Panics

Always — the body is [`todo!`].

```rust
pub fn calc_diffusion_coefficients(params: &CaseParams, sigma_tot: &[f64], which_sigma: &MaterialMap) -> DiffusionCoefficients { /* ... */ }
```

#### Function `make_gradient_diffusion_operator`

**Attributes:**

- `MustUse { reason: None }`

Finite-difference diffusion operator and the interface-current terms.

MATLAB `makegradDxyz.m`, returning `[gradD, gradterms]`.

# Ownership

**[`crate::reference::nodal`] owns this.**

# Panics

Always — the body is [`todo!`].

```rust
pub fn make_gradient_diffusion_operator(geometry: &CoreGeometry, params: &CaseParams, diffusion: &DiffusionCoefficients, which_sigma: &MaterialMap) -> (super::sparse::SparseMatrix, GradientTerms) { /* ... */ }
```

#### Function `calc_nodal_coefficients`

**Attributes:**

- `MustUse { reason: None }`

Semi-analytic nodal coefficients `A, B, E, F, G, H`.

MATLAB `calc_ABEFGHxyz.m`, stored by the caller into
[`CoreGeometry::nodal_coeffs`].

# Ownership

**[`crate::reference::nodal`] owns this.**

# Panics

Always — the body is [`todo!`].

```rust
pub fn calc_nodal_coefficients(params: &CaseParams, geometry: &CoreGeometry, sigma: &SigmaOperators, diffusion: &DiffusionCoefficients) -> NodalCoefficients { /* ... */ }
```

#### Function `calc_semi_analytic_nodal`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

One refinement of the semi-analytic nodal correction at a fixed flux.

MATLAB `calc_sanodalxyz.m`, returning `[nodal, nodalterms]`. Reads
`geometry.nodalcoeffs`, so [`calc_nodal_coefficients`] must have been
stored into the geometry first.

# Ownership

**[`crate::reference::nodal`] owns this.**

# Panics

Always — the body is [`todo!`].

```rust
pub fn calc_semi_analytic_nodal(params: &CaseParams, geometry: &CoreGeometry, flux: &[f64], sigma: &SigmaOperators, diffusion: &DiffusionCoefficients, gradient_terms: &GradientTerms, nodal_terms_old: &NodalTerms, k_eff: f64) -> (super::sparse::SparseMatrix, NodalTerms) { /* ... */ }
```

#### Function `solve_sanodal_eigenvalue`

**Attributes:**

- `MustUse { reason: None }`

The SA-nodal `k`-eigenvalue solve — the production eigensolver.

MATLAB `sanodaldiffusion_solverxyz(geometry, params, sigmavalues,
whichsigma, initial_k_eff, initflux)`. `warm_flux` is `varargin{2}`: a
previously converged flux used to seed the source iteration instead of a
flat guess. `params.innertol`, when set, replaces the default 1e-6 inner
tolerance.

# Ownership

**[`crate::reference::nodal`] owns this.**

# Panics

Always — the body is [`todo!`].

```rust
pub fn solve_sanodal_eigenvalue(geometry: &CoreGeometry, params: &CaseParams, sigma_values: &SigmaValues, which_sigma: &MaterialMap, initial_k_eff: f64, warm_flux: Option<&[f64]>) -> DiffusionSolution { /* ... */ }
```

#### Function `solve_thermal_hydraulics_steady`

**Attributes:**

- `MustUse { reason: None }`

One steady thermal-hydraulics update at a given power distribution.

MATLAB `th_solverxyz(params, geometry, th, whichsigma, pwrdens)`. Marches
the coolant enthalpy up each channel and solves the 1-D cylindrical fuel-rod
conduction, returning the updated state. `pwrdens` is the node power
\[W per node\], `state_len` entries.

# Ownership

**[`crate::reference::th`] owns this.**

# Panics

Always — the body is [`todo!`].

```rust
pub fn solve_thermal_hydraulics_steady(params: &CaseParams, geometry: &CoreGeometry, th: &ThermalState, which_sigma: &MaterialMap, pwrdens: &[f64]) -> ThermalState { /* ... */ }
```

#### Function `solve_thermal_hydraulics_transient`

**Attributes:**

- `MustUse { reason: None }`

One implicit-Euler time step of the thermal hydraulics.

MATLAB `th_solvertimexyz(params, geometry, th, whichsigma, pwrdens, thold,
dt)`. `th` is the current iterate (its `heatflux` feeds the coolant energy
source and its `powratio` must already carry the current relative power);
`th_old` is the converged state of the previous **time step**, supplying the
capacity terms; `dt` is the step \[s\].

# Ownership

**[`crate::reference::th`] owns this.**

# Panics

Always — the body is [`todo!`].

```rust
pub fn solve_thermal_hydraulics_transient(params: &CaseParams, geometry: &CoreGeometry, th: &ThermalState, which_sigma: &MaterialMap, pwrdens: &[f64], th_old: &ThermalState, dt: f64) -> ThermalState { /* ... */ }
```

#### Function `w3_chf_hottest_channel`

**Attributes:**

- `MustUse { reason: None }`

W-3 critical heat flux evaluated on the hottest channel.

MATLAB `w3chfhottest.m`.

# Ownership

**[`crate::reference::th`] owns this.**

# Known defect in the MATLAB

`w3chfhottest.m:22` sets `highy = ix` instead of `highy = iy` when it
records the hottest channel, so the search always returns a diagonal
column. Recorded, not fixed — see `docs/bedok-port-scoping.md` §1.0.

# Panics

Always — the body is [`todo!`].

```rust
pub fn w3_chf_hottest_channel(params: &CaseParams, geometry: &CoreGeometry, th: &ThermalState) -> ChfResult { /* ... */ }
```

#### Function `replicate_per_group`

**Attributes:**

- `MustUse { reason: None }`

Replicate a per-node field across every energy group — MATLAB
`repmat(geometry.Vi, G, 1)`.

The result has `grid.state_len()` entries, with entry
`Grid::index(g, ix, iy, iz)` holding the node's value for every `g`. Used
to turn node volumes \[cm³\] into the `ViG` vector that converts a fission
source into a node power.

# Panics

If `per_node.len()` is not `grid.nodes()`.

```rust
pub fn replicate_per_group(grid: &crate::reference::grid::Grid, per_node: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `fix_negative`

Zero every negative entry — MATLAB `fixnegativematrix.m`.

# Faithful quirk

The MATLAB operates on the result of `find(mat)`, i.e. only on **stored
non-zero** entries. For a dense MATLAB array that is every non-zero value,
which is what this reproduces: exact zeros are left alone (they are already
non-negative) and every negative value becomes zero.

```rust
pub fn fix_negative(values: &mut [f64]) { /* ... */ }
```

#### Function `pause_on_nan`

The MATLAB `pauseonnan.m` guard, with its column semantics preserved.

# Faithful quirk — this guard is weaker than it looks

`pauseonnan.m` is `if any(isnan(input)) ... error(...)`. On a **matrix**,
MATLAB's `any` reduces down columns and returns a row vector, and an `if`
on a vector is true only when **every** element is non-zero. So for a
2-D input the guard fires only when *every column* contains at least one
NaN — a single NaN, or a whole NaN row, passes silently. That behaviour is
reproduced here rather than tightened: the cross-section arrays it guards
are all 2-D or 3-D.

`data` is row-major with `ncols` columns, matching the storage in
[`SigmaValues`].

# Errors

[`super::error::CouplingError::NotANumber`] under the condition above.

```rust
pub fn pause_on_nan(field: &'static str, data: &[f64], ncols: usize) -> super::error::Result<()> { /* ... */ }
```

## Module `sparse`

Sparse linear algebra used by the coupled drivers.

# Provenance

Support code for the translation of Than Yan Ren's (SNRSI) BEDOK MATLAB
snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`). It has no MATLAB
counterpart of its own: it supplies the operations MATLAB provides as
built-in syntax — sparse `+`/`-`, `A*x`, `spdiags`, `\` and
`decomposition()` — so that the ported drivers read the same way the
original does.

# What the MATLAB operators mean here

| MATLAB | Rust |
|---|---|
| `A*x` (sparse × dense vector) | [`spmv`] |
| `A+B`, `A-B`, `c*A` | [`linear_combination`] |
| `A + spdiags(d,0,n,n)` | [`add_diagonal`] |
| `A*spdiags(d,0,n,n)` | [`scale_columns`] |
| `decomposition(A)` then `A\b` | [`SparseLu`] |
| `A\b` (one-shot) | [`SparseLu::factorise`] then [`SparseLu::solve`] |

**`\` is a direct sparse LU here, deliberately.** MATLAB's backslash on a
sparse unsymmetric matrix runs UMFPACK — a direct factorisation, not an
iterative solve — and `decomposition(A)` caches that factorisation for
reuse across right-hand sides. Substituting a Krylov method would change
the answers by its own tolerance and is out of scope for the reference
translation (`docs/bedok-port-scoping.md` §1, stage 1).

```rust
pub mod sparse { /* ... */ }
```

### Types

#### Type Alias `SparseMatrix`

A sparse operator over the state vector, in compressed-sparse-column form.

All BEDOK operators (`gradD`, the nodal correction, `sigma.tot`, `sigma.s`,
`sigma.f`, `sigma.fp`) are square and of side `philenf` — the full state
length including any extra components, in units of inverse centimetres
times centimetres cubed as the discretised balance equation leaves them.

```rust
pub type SparseMatrix = faer::sparse::SparseColMat<usize, f64>;
```

#### Struct `SparseLu`

A cached sparse LU factorisation — the MATLAB `decomposition(A)`.

Holding the factorisation and calling [`solve`](Self::solve) repeatedly
reproduces MATLAB's `dM = decomposition(M); x = dM\b;` exactly: one
factorisation, many triangular solves. Results are identical to a fresh
`M\b` per right-hand side, so the reuse is purely a cost saving.

# Note on pivoting

`faer`'s sparse LU uses partial (row) pivoting, as UMFPACK does. Fill-in
ordering differs between the two libraries, so the rounding of the solve is
not bit-identical to MATLAB's — expected, and the reason parity tolerances
are set physically rather than at machine epsilon
(`docs/bedok-port-scoping.md` §5).

```rust
pub struct SparseLu {
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
  pub fn factorise(a: &SparseMatrix) -> Result<Self> { /* ... */ }
  ```
  Factorise `a`, which must be square.

- ```rust
  pub fn solve(self: &Self, b: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Solve `A x = b` with the cached factorisation — the MATLAB `dM\b`.

- ```rust
  pub const fn order(self: &Self) -> usize { /* ... */ }
  ```
  Order of the factorised matrix.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `spmv`

**Attributes:**

- `MustUse { reason: None }`

Matrix-vector product `a * x`, the MATLAB `A*x`.

# Panics

If `x.len()` differs from the column count of `a`. A length mismatch means
the state-vector convention has been broken somewhere upstream, which is
exactly the failure mode `grid.rs` exists to prevent, so it is loud.

```rust
pub fn spmv(a: &SparseMatrix, x: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `linear_combination`

Linear combination `sum_i coefficient_i * matrix_i`, the MATLAB `A+B-C`.

Structurally-zero entries stay absent; duplicate entries are summed. Every
term must have the same shape.

# Errors

[`CouplingError::SparseAssembly`] if the term list is empty, if the terms
disagree in shape, or if assembly fails.

```rust
pub fn linear_combination(terms: &[(f64, &SparseMatrix)]) -> super::error::Result<SparseMatrix> { /* ... */ }
```

#### Function `add_diagonal`

`a + spdiags(d, 0, n, n)` — add a diagonal to a square operator.

Used for the time-derivative term `spdiags(invv*(omega+1/dt),0,…) + M` of
the transient flux solve. `d` carries units of inverse velocity per second,
i.e. inverse centimetres, matching the removal terms already in `a`.

# Errors

[`CouplingError::SparseAssembly`] if `a` is not square or `d` has the wrong
length.

```rust
pub fn add_diagonal(a: &SparseMatrix, d: &[f64]) -> super::error::Result<SparseMatrix> { /* ... */ }
```

#### Function `scale_columns`

`a * spdiags(d, 0, n, n)` — scale column `j` of `a` by `d[j]`.

This is the "delayed production of the new flux moves into the system
matrix" step of the exponential-transform scheme: right-multiplying the
fission operator by a diagonal scales each column, i.e. each source node's
contribution, by its own precursor weight.

# Errors

[`CouplingError::SparseAssembly`] if `d.len()` differs from the column count.

```rust
pub fn scale_columns(a: &SparseMatrix, d: &[f64]) -> super::error::Result<SparseMatrix> { /* ... */ }
```

#### Function `diagonal_matrix`

A square diagonal operator, the MATLAB `spdiags(d, 0, n, n)`.

# Errors

[`CouplingError::SparseAssembly`] if assembly fails.

```rust
pub fn diagonal_matrix(d: &[f64]) -> super::error::Result<SparseMatrix> { /* ... */ }
```

#### Function `from_triplets`

Assemble a sparse matrix from triplets, summing duplicates.

# Errors

[`CouplingError::SparseAssembly`] if `faer` rejects the triplet list (index
overflow or allocation failure).

```rust
pub fn from_triplets(nrows: usize, ncols: usize, triplets: &[faer::sparse::Triplet<usize, usize, f64>]) -> super::error::Result<SparseMatrix> { /* ... */ }
```

#### Function `fix_inf_nan`

Replace non-finite entries with zero — MATLAB `fixinfnan.m` (default mode).

Yan Ren applies this to every flux solve so that a blown-up node does not
poison the whole vector through the subsequent norms. The "special mode"
(`fixinfnan(v, anything)`, which substitutes `min(abs(v))` instead) is not
used anywhere the coupling calls it, so only the default is translated.

# Note

This silently converts divergence into a zero flux. It is translated as-is;
it is a symptom-suppressor, not a fix, and any solve that needs it has
already failed.

```rust
pub fn fix_inf_nan(v: &mut [f64]) { /* ... */ }
```

#### Function `sum`

**Attributes:**

- `MustUse { reason: None }`

Sum of a vector, MATLAB `sum(v)`.

```rust
pub fn sum(v: &[f64]) -> f64 { /* ... */ }
```

#### Function `norm2`

**Attributes:**

- `MustUse { reason: None }`

Euclidean norm, MATLAB `norm(v)` / `norm(v,2)`.

```rust
pub fn norm2(v: &[f64]) -> f64 { /* ... */ }
```

#### Function `norm1`

**Attributes:**

- `MustUse { reason: None }`

Sum of absolute values, MATLAB `norm(v,1)`.

```rust
pub fn norm1(v: &[f64]) -> f64 { /* ... */ }
```

#### Function `max_abs_difference`

**Attributes:**

- `MustUse { reason: None }`

Largest absolute difference between two vectors, MATLAB
`max(abs(a-b))` — the fuel-temperature convergence measure, in kelvin.

# Panics

If the two vectors have different lengths.

```rust
pub fn max_abs_difference(a: &[f64], b: &[f64]) -> f64 { /* ... */ }
```

#### Function `under_relax`

Picard under-relaxation `(1-w)*old + w*new`, applied in place to `new`.

`w` is the relaxation factor: `w = 1` is no damping (take the new value),
`w -> 0` freezes the field. Yan Ren defaults it to 0.5 for the
neutronics/T-H feedback fields.

# Panics

If the two vectors have different lengths.

```rust
pub fn under_relax(new: &mut [f64], old: &[f64], w: f64) { /* ... */ }
```

## Module `steady`

Steady coupled neutronics/thermal-hydraulics solve.

# Provenance

Translated from `thdiffusion_solverxyz.m` in Than Yan Ren's (SNRSI) BEDOK
MATLAB snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`, received
2026-08-05). Original author: **Than Yan Ren**, Singapore Nuclear Research
and Safety Institute. Translated with permission; see
`docs/bedok-port-scoping.md` §6.

```rust
pub mod steady { /* ... */ }
```

### Types

#### Struct `SteadyOutput`

Result of a steady coupled solve — the MATLAB `output` struct of
`thdiffusion_solverxyz.m`.

```rust
pub struct SteadyOutput {
    pub k_eff: f64,
    pub residual: f64,
    pub k_eff_residual: f64,
    pub fuel_temp_residual: f64,
    pub fuel_temp_residual_history: Vec<f64>,
    pub k_eff_history: Vec<f64>,
    pub scalar_flux: Vec<f64>,
    pub fission_source: Vec<f64>,
    pub pwrdens: Vec<f64>,
    pub th: super::seam::ThermalState,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | Converged multiplication factor \[-\]. |
| `residual` | `f64` | Final fission-source residual \[-\], `‖Δfs‖₂/‖fs‖₂`. |
| `k_eff_residual` | `f64` | Final `k_eff` residual \[-\], `|Δk|/k`. |
| `fuel_temp_residual` | `f64` | Final fuel-temperature change \[K\], `max|Δ T_fuel,avg|`. |
| `fuel_temp_residual_history` | `Vec<f64>` | Per-iteration fuel-temperature change history \[K\]. Entries for<br>iterations where no T-H update ran are [`f64::INFINITY`], as in the<br>MATLAB's `inf(maxiter,1)` preallocation. |
| `k_eff_history` | `Vec<f64>` | Per-iteration `k_eff` history \[-\], starting with the initial guess. |
| `scalar_flux` | `Vec<f64>` | Converged scalar flux, renormalised so the fission-source integral<br>matches its initial value. `state_len` entries. |
| `fission_source` | `Vec<f64>` | Fission source at the same normalisation, `state_len` entries. |
| `pwrdens` | `Vec<f64>` | Node power density, `fission_source .* Vi`, `state_len` entries. |
| `th` | `super::seam::ThermalState` | Converged thermal-hydraulic state. |
| `converged` | `bool` | `false` if the loop bailed out through the not-converging guard —<br>`k_eff` non-positive or NaN, or the outer-iteration cap passed.<br><br>Has no MATLAB counterpart: the original prints<br>`" T-H interation stopped, not converging"` and returns the same struct<br>either way, so a caller cannot tell. Recorded here because<br>[`critical_boron`](super::critical_boron) has to re-derive it from the<br>returned `k_eff`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SteadyOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

### Functions

#### Function `fuel_solution_id_count`

**Attributes:**

- `MustUse { reason: None }`

Number of radial solution ids in the fuel rod — MATLAB `maxid`.

`maxir` rings plus one extra node at every material interface, found by
counting transitions to and from `whichk == 0` (the gap) along the radius.

# Panics

If `which_k` is shorter than `fuel_max_ir`.

```rust
pub fn fuel_solution_id_count(params: &super::seam::CaseParams, geometry: &super::seam::CoreGeometry) -> usize { /* ... */ }
```

#### Function `flat_initial_thermal_state`

**Attributes:**

- `MustUse { reason: None }`

Flat starting thermal-hydraulic state — the MATLAB "Set up initial T-H"
block.

Every node starts at the case's average fuel temperature, average coolant
temperature and average coolant density, with zero wall heat flux. The
non-field members of `th` (flow rate, rated power, pin count) are carried
through untouched.

```rust
pub fn flat_initial_thermal_state(params: &super::seam::CaseParams, geometry: &super::seam::CoreGeometry, th: &super::seam::ThermalState) -> super::seam::ThermalState { /* ... */ }
```

#### Function `solve_coupled_steady`

Solve the coupled steady state.

MATLAB `thdiffusion_solverxyz(geometry, params, th, sigmavalues,
whichsigma, initial_k_eff)`.

# The iteration

One Picard cycle per outer iteration:

1. Update the cross sections at the current T-H state
   ([`update_cross_sections`]) — skipped on the first pass, which uses the
   update already done before the loop.
2. Pick an inexact inner tolerance from how far the outer loop still is
   (see [`inexact_inner_tolerance`]).
3. Solve the `k`-eigenvalue problem, **warm-started** from the previous
   outer iteration's flux and `k_eff`.
4. Measure the fission-source and `k_eff` residuals.
5. Take one steady T-H step and **under-relax** the four fields that carry
   the feedback: coolant density, Doppler temperature, average fuel
   temperature and wall heat flux.

The loop exits when all three of the fission-source residual, the `k_eff`
residual and the fuel-temperature change are below tolerance; or bails out
if `k_eff` goes non-positive or NaN, or the iteration cap is passed.

# Arguments

- `initial_k_eff` — MATLAB `varargin{1}`; 1.0 when the caller passes
  [`None`].

# Normalisation

On exit the flux and fission source are scaled so the fission-source
integral equals its value at the *initial flat flux*
(`init_norm = sum(sigma_f * ones)`), the MATLAB's
"fission source integration = 1" convention.

# Deviation — file output

The MATLAB ends with seven `writematrix` calls that dump `k_eff`, the
residual histories, the flux, the fission source and the power density into
the working directory unconditionally. A library function writing into a
caller's working directory is not acceptable here, so the same data is
returned in [`SteadyOutput`] instead and no file is written. Numerically
nothing changes.

# Deviation — progress printing

The MATLAB's per-iteration `fprintf` lines are not reproduced; the same
numbers are in [`SteadyOutput::k_eff_history`],
[`SteadyOutput::fuel_temp_residual_history`] and the residual fields.

# Errors

Propagates the cross-section feedback's `pauseonnan` guards and any sparse
failure.

# Panics

Through the [`seam`] stubs until `nodal/` and `th/` land.

```rust
pub fn solve_coupled_steady(geometry: &super::seam::CoreGeometry, params: &super::seam::CaseParams, th: &super::seam::ThermalState, sigma_values: &super::seam::SigmaValues, which_sigma: &super::seam::MaterialMap, initial_k_eff: Option<f64>) -> super::error::Result<SteadyOutput> { /* ... */ }
```

#### Function `inexact_inner_tolerance`

**Attributes:**

- `MustUse { reason: None }`

Inexact inner-solve tolerance for the next eigenvalue solve \[-\].

MATLAB `thdiffusion_solverxyz.m:118-134`, an Eisenstat-Walker-style forcing
schedule. While the outer neutronics/T-H loop is far from converged, an
over-tight inner solve is wasted work because the cross sections change
again next pass; so

```text
innertol = clamp(eta * max(fission-source residual, k_eff residual),
                 1e-6, 1e-3)
```

Yan Ren's reasoning, which is a physics point rather than a performance
one: *"A loose inner solve does not merely blur the final keff readout — it
biases the coupled FIXED POINT through the feedback (loose flux → wrong
power → wrong fuel temp → wrong Doppler)."* `eta = 0.001` makes the schedule
self-tighten to the 1e-6 floor in the tail, killing the power-shape jitter
and the fuel-temperature limit cycle it caused.

Returns [`None`] — leaving the inner solver at its own 1e-6 default — when
`params.inexactinner == 0`.

```rust
pub fn inexact_inner_tolerance(params: &super::seam::CaseParams, fission_source_residual: f64, k_eff_residual: f64) -> Option<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_FUEL_TEMP_TOL`

Default fuel-temperature convergence tolerance \[K\].

Yan Ren's note, kept verbatim because it is a physics judgement and not a
tuning knob: *"relaxed from 0.01 K; a max-norm fuel temperature criterion
that tight is unrealistic for a coupled BWR steady state — the hot nodes
limit-cycle ~1 K"*.

```rust
pub const DEFAULT_FUEL_TEMP_TOL: f64 = 0.5;
```

#### Constant `DEFAULT_FLUX_TOL`

Default outer fission-source / `k_eff` tolerance \[-\].

Yan Ren's note: *"Relaxed from 1e-5: even exact inner solves floor the
outer fission-source residual near ~1e-4 (a tiny residual Picard cycle), so
1e-5 is unreachable."*

```rust
pub const DEFAULT_FLUX_TOL: f64 = 1.0e-4;
```

#### Constant `DEFAULT_MAX_OUTER_ITERATIONS`

Default cap on coupled outer iterations.

```rust
pub const DEFAULT_MAX_OUTER_ITERATIONS: usize = 50;
```

#### Constant `DEFAULT_TH_RELAX`

Default Picard under-relaxation factor for the T-H feedback fields \[-\].

```rust
pub const DEFAULT_TH_RELAX: f64 = 0.5;
```

#### Constant `INNER_TOL_FLOOR`

Floor of the inexact inner-solve tolerance schedule \[-\].

```rust
pub const INNER_TOL_FLOOR: f64 = 1.0e-6;
```

#### Constant `INNER_TOL_CAP`

Cap of the inexact inner-solve tolerance schedule \[-\].

```rust
pub const INNER_TOL_CAP: f64 = 1.0e-3;
```

#### Constant `DEFAULT_INEXACT_ETA`

Default forcing factor of the inexact inner-solve schedule \[-\].

```rust
pub const DEFAULT_INEXACT_ETA: f64 = 0.001;
```

## Module `transient`

Transient coupled neutronics/thermal-hydraulics solve.

# Provenance

Translated from `thdiffusion_solvertimexyz.m` in Than Yan Ren's (SNRSI)
BEDOK MATLAB snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`,
received 2026-08-05). Original author: **Than Yan Ren**, Singapore Nuclear
Research and Safety Institute. Translated with permission; see
`docs/bedok-port-scoping.md` §6.

# The method, in three phases

**Phase 1 — initial steady state.** The static coupled solver
([`super::steady::solve_coupled_steady`]) is run to convergence, and the
transient fission operator is then divided by the resulting `k_eff` so the
initial state is *exactly* critical. That stands in for the critical-boron
search the benchmark performs to the same effect.

**Phase 2 — rebuild and re-equilibrate.** The diffusion operator is rebuilt
at the converged steady state and the flux and `k_eff` are re-equilibrated
on it with a short power iteration, so the transient starts from an exact
equilibrium of *the operator actually used in the time stepping* rather than
of whatever the eigensolver last held.

**Phase 3 — time integration.** The multigroup diffusion equation with six
delayed-neutron precursor families, the prescribed control-assembly
ejection, and one transient T-H step per time step.

# Two kinetics schemes

[`KineticsScheme::ExponentialTransform`] (the default) is the scheme of the
nodal program *Ants* — A. Rintala and U. Lauranto, *Ann. Nucl. Energy* **190**
(2023) 109868, Eqs. (3)–(13): implicit Euler on an exponentially transformed
flux, with the precursors integrated analytically under the assumption that
the transformed fission source varies linearly over the step. The
frequencies are iterated **within** the step — a predictor pass at
`omega = 0`, then `freq_iter - 1` correctors using the newest flux of the
current step, which is the remark under the paper's Eq. (4). Yan Ren records
that extrapolating the frequencies from the *previous* step instead proved
unstable against the lagged T-H feedback (a growing two-step power
oscillation), so it is not used.

[`KineticsScheme::ImplicitEuler`] is the legacy first-order scheme with the
precursors eliminated analytically per step.

# There is no Newton-Krylov solve here

Each time step is a **linear** system solved directly (sparse LU). The
feedback is closed by Picard passes (`params.timepicard`), not by a Newton
iteration. See the note in [`super`] on the snapshot's dead `params.jfnk*`
controls.

```rust
pub mod transient { /* ... */ }
```

### Types

#### Struct `TransientOutput`

Result of a transient coupled solve — the MATLAB `output` struct of
`thdiffusion_solvertimexyz.m`.

The `C1`–`C6` labels are the reported quantities of NEACRP-L-335 section 4C.

```rust
pub struct TransientOutput {
    pub k_eff: f64,
    pub steady: super::steady::SteadyOutput,
    pub th: super::seam::ThermalState,
    pub time: Vec<f64>,
    pub relative_power: Vec<f64>,
    pub avg_fuel_temp: Vec<f64>,
    pub max_fuel_temp: Vec<f64>,
    pub coolant_outlet_temp: Vec<f64>,
    pub radial_c5_z6: Vec<f64>,
    pub radial_c5_z13: Vec<f64>,
    pub radial_c6_z6: Vec<f64>,
    pub radial_c6_z13: Vec<f64>,
    pub t_power_max: f64,
    pub relative_power_max: f64,
    pub rod_position: Vec<f64>,
    pub scalar_flux_final: Vec<f64>,
    pub pwrdens_final: Vec<f64>,
    pub precursors_final: Vec<Vec<f64>>,
    pub time_scheme: super::seam::KineticsScheme,
    pub diverged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | Initial (re-equilibrated) multiplication factor \[-\]. |
| `steady` | `super::steady::SteadyOutput` | The Phase-1 steady state, returned whole. |
| `th` | `super::seam::ThermalState` | Final transient thermal-hydraulic state. |
| `time` | `Vec<f64>` | Time points \[s\], truncated at the divergence guard if it tripped. |
| `relative_power` | `Vec<f64>` | **C1** — core power relative to its steady value \[-\]. |
| `avg_fuel_temp` | `Vec<f64>` | **C2** — core-average fuel temperature \[K\]. |
| `max_fuel_temp` | `Vec<f64>` | **C3** — maximum fuel temperature \[K\]. |
| `coolant_outlet_temp` | `Vec<f64>` | **C4** — core-average coolant outlet temperature \[K\]. |
| `radial_c5_z6` | `Vec<f64>` | **C5-1** — radial power map at active-core axial layer 6, at the time of<br>the power maximum, normalised to a peak of 1. `nx*ny`, indexed<br>`ix*ny + iy`. |
| `radial_c5_z13` | `Vec<f64>` | **C5-2** — the same at active-core axial layer 13. |
| `radial_c6_z6` | `Vec<f64>` | **C6-1** — radial power map at layer 6 at the final time. |
| `radial_c6_z13` | `Vec<f64>` | **C6-2** — the same at layer 13. |
| `t_power_max` | `f64` | Time of the power maximum \[s\]. |
| `relative_power_max` | `f64` | Peak relative power \[-\]. |
| `rod_position` | `Vec<f64>` | Ejected-bank position per time step \[steps withdrawn\]. |
| `scalar_flux_final` | `Vec<f64>` | Final scalar flux, `philenf` entries. |
| `pwrdens_final` | `Vec<f64>` | Final group-collapsed node power \[W\], `nodes` entries. |
| `precursors_final` | `Vec<Vec<f64>>` | Final precursor concentrations, `n_families` vectors of `philenf`<br>entries each. |
| `time_scheme` | `super::seam::KineticsScheme` | Which kinetics scheme was used. |
| `diverged` | `bool` | Whether the divergence guard tripped. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TransientOutput { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

### Functions

#### Function `solve_coupled_transient`

**Attributes:**

- `Other("#[allow(clippy::too_many_lines)]")`

Solve the coupled transient.

MATLAB `thdiffusion_solvertimexyz(geometry, params, th, sigmavalues,
whichsigma, initial_k_eff)`.

# Case data the transient needs

From `params`: [`velocities`](CaseParams::velocities) \[cm/s\],
[`beta_dnp`](CaseParams::beta_dnp) \[-\],
[`lambda_dnp`](CaseParams::lambda_dnp) \[1/s\],
[`t_end`](CaseParams::t_end) and/or [`t_grid`](CaseParams::t_grid) \[s\],
and [`eject_duration`](CaseParams::eject_duration) \[s\] when a bank moves.
From `geometry`: [`crod_eject`](CoreGeometry::crod_eject) and
[`crod_eject_to`](CoreGeometry::crod_eject_to).

# Time grid

`[0, tgrid…, tend]`, rounded to 1 µs and deduplicated so overlapping range
endpoints cannot produce a near-zero step, then truncated at `tend`.

# Deviations from the MATLAB

- **No file output.** The MATLAB writes `<prefix>_C1toC4_history.csv` and
  four `C5`/`C6` matrices, and optionally a JPEG plot. Everything in them is
  returned in [`TransientOutput`] instead.
- **No steady-state cache.** `params.steadyfile` names a `.mat` file the
  MATLAB loads if present and writes otherwise. There is no `.mat` support
  here, so the steady solve always runs;
  [`CaseParams::steady_file`] is carried but not acted on.
- **No progress printing.**
- **No per-step wall times.** The MATLAB records `output.steptime` (and
  prints a mean) purely as a performance diagnostic; it is not reproduced.

None of the four changes a number.

# Errors

[`CouplingError::NoTimeData`] if the case sets neither `tend` nor `tgrid`;
[`CouplingError::MissingCaseData`] if a bank ejection is declared without an
ejection duration or target; plus any sparse or feedback failure.

# Panics

Through the [`seam`] stubs until `nodal/` and `th/` land.

```rust
pub fn solve_coupled_transient(geometry: &super::seam::CoreGeometry, params: &super::seam::CaseParams, th_in: &super::seam::ThermalState, sigma_values: &super::seam::SigmaValues, which_sigma: &super::seam::MaterialMap, initial_k_eff: Option<f64>) -> super::error::Result<TransientOutput> { /* ... */ }
```

#### Function `diffusion_operator`

`M = gradD + nodal + sigma.tot - sigma.s`, the static diffusion operator.

# Errors

[`CouplingError::SparseAssembly`] if the four terms disagree in shape.

```rust
pub fn diffusion_operator(grad_d: &super::sparse::SparseMatrix, nodal: &super::sparse::SparseMatrix, sigma_tot: &super::sparse::SparseMatrix, sigma_s: &super::sparse::SparseMatrix) -> super::error::Result<super::sparse::SparseMatrix> { /* ... */ }
```

#### Function `build_time_grid`

**Attributes:**

- `MustUse { reason: None }`

Build the transient time grid \[s\].

MATLAB: `tgrid = [0 params.tgrid(:).' tend]`, then rounded to 1 µs and
deduplicated (`unique(round(tgrid*1e6))/1e6`) so overlapping range endpoints
cannot produce a near-zero step, then truncated at `tend`. With no case
grid, a uniform 10 ms grid over `0..tend` is used.

`unique` also **sorts**, so a case grid supplied out of order is silently
reordered — matching the MATLAB.

```rust
pub fn build_time_grid(case_grid: Option<&[f64]>, t_end: f64) -> Vec<f64> { /* ... */ }
```

#### Function `g_exp_0`

**Attributes:**

- `MustUse { reason: None }`

`g0(x) = (exp(x) - 1 - x)/x²`, with the series fallback near `x = 0`.

MATLAB local function `gexp0`. The series `1/2 + x/6 + x²/24` is used for
`|x| < 1e-4`, where the direct form loses all its significant digits to
cancellation.

```rust
pub fn g_exp_0(x: f64) -> f64 { /* ... */ }
```

#### Function `g_exp_1`

**Attributes:**

- `MustUse { reason: None }`

`g1(x) = (x - 1 + exp(-x))/x²`, with the series fallback near `x = 0`.

MATLAB local function `gexp1`. Series `1/2 - x/6 + x²/24` for `|x| < 1e-4`.

```rust
pub fn g_exp_1(x: f64) -> f64 { /* ... */ }
```

#### Function `node_frequencies`

**Attributes:**

- `MustUse { reason: None }`

Per-node, per-group exponential-transform frequencies \[1/s\].

MATLAB local function `expfreq`, the Ants paper's Eq. (4):
`omega = ln(phi(t_n)/phi(t_{n-1}))/dt`. Zero wherever either flux is
non-positive or non-finite, or the node is void (`invv == 0`).

# Stability warning carried from the MATLAB

Yan Ren records this mode as **unstable in super-prompt rod ejections**:
node-wise frequency noise near the ejected channel feeds back through the
nearly singular prompt operator. [`global_group_frequencies`] is the
default for that reason.

```rust
pub fn node_frequencies(phi_new: &[f64], phi_old: &[f64], dt: f64, inv_v: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `global_group_frequencies`

**Attributes:**

- `MustUse { reason: None }`

Per-group **global** amplitude frequencies \[1/s\], uniform in space.

The default mode. Taken from the volume-integrated group flux, so it
captures the stiff exponential amplitude rise of a super-prompt excursion
exactly while carrying no spatial noise. Zero on void nodes (`invv == 0`),
and left at zero for any group whose integrated flux is non-positive or
non-finite at either end of the step.

```rust
pub fn global_group_frequencies(grid: &crate::reference::grid::Grid, philenf: usize, phi: &[f64], phi_old: &[f64], vi_per_group: &[f64], inv_v: &[f64], dt: f64) -> Vec<f64> { /* ... */ }
```

#### Function `fuel_node_mask`

**Attributes:**

- `MustUse { reason: None }`

Fuel-node mask: 1 where the node is fuel, 0 elsewhere.

MATLAB: `whichsigmaref(ix,iy,iz) >= 4`, with the comment *"compositions 4-11
are fuel in the NEACRP composition map"*.

# Case-specific constant

The threshold 4 is **hard-coded to the NEACRP composition numbering** and is
not derived from the case data. Any case whose fuel compositions are not
numbered 4 and above gets a wrong mask, and hence wrong C2/C3/C4 outputs,
silently. Recorded, not fixed.

```rust
pub fn fuel_node_mask(grid: &crate::reference::grid::Grid, which_sigma_ref: &super::seam::MaterialMap) -> Vec<f64> { /* ... */ }
```

#### Function `channel_outlet_indices`

**Attributes:**

- `MustUse { reason: None }`

Spatial indices of every fuel-bearing channel's outlet node.

MATLAB: the top node of every column that contains any fuel, taken as
`geometry.zhis(ix,iy)` — the highest fuel-bearing axial node, **not** the
top of the mesh.

```rust
pub fn channel_outlet_indices(grid: &crate::reference::grid::Grid, fuel_mask: &[f64], zhis: &[usize]) -> Vec<usize> { /* ... */ }
```

#### Function `fuel_radial_weights`

**Attributes:**

- `MustUse { reason: None }`

Radial volume weights for the in-rod fuel-temperature average \[-\].

MATLAB: solution ids `1..fueln` are the fuel rings up to their centres and
id `fueln+1` is the fuel surface node covering `[Ctr(fueln), fuelrad]`, so
the weights are annular areas normalised by the pellet area:

```text
w(1)       = Ctr(1)^2 / R^2
w(i)       = (Ctr(i)^2 - Ctr(i-1)^2) / R^2      for 2 <= i <= fueln
w(fueln+1) = (R^2 - Ctr(fueln)^2) / R^2
```

Returns `fueln + 1` weights.

```rust
pub fn fuel_radial_weights(params: &super::seam::CaseParams, geometry: &super::seam::CoreGeometry) -> Vec<f64> { /* ... */ }
```

#### Function `core_average_fuel_temperature`

**Attributes:**

- `MustUse { reason: None }`

Fuel-volume-weighted core-average fuel temperature \[K\] — output **C2**.

MATLAB `calcavgfuel`: the radial average over the pellet
(`fueltemp(:,1:fueln+1) * wrad`) weighted by the fuel-node volume.

# Panics

If `th.fuel_temp` is shorter than `nodes * radial_weights.len()`.

```rust
pub fn core_average_fuel_temperature(th: &super::seam::ThermalState, radial_weights: &[f64], fuel_volume: &[f64]) -> f64 { /* ... */ }
```

#### Function `maximum_fuel_temperature`

**Attributes:**

- `MustUse { reason: None }`

Maximum fuel temperature over the pellet of any fuel node \[K\] — output
**C3**.

MATLAB `calcmaxfuel`: `max(max(fueltemp(fuelmask==1, 1:fueln+1)))`.

```rust
pub fn maximum_fuel_temperature(th: &super::seam::ThermalState, fuel_mask: &[f64], n_radial: usize) -> f64 { /* ... */ }
```

#### Function `average_coolant_outlet_temperature`

**Attributes:**

- `MustUse { reason: None }`

Mean coolant temperature over the channel outlet nodes \[K\] — output
**C4**.

MATLAB `calccoolout`: `mean(th.coolant.temps(outletidx))`. An unweighted
mean over channels, not a flow-weighted mixed-mean outlet temperature.

```rust
pub fn average_coolant_outlet_temperature(th: &super::seam::ThermalState, outlet_index: &[usize]) -> f64 { /* ... */ }
```

#### Function `collapse_power_over_groups`

**Attributes:**

- `MustUse { reason: None }`

Sum a state-vector power over energy groups, leaving one value per node.

MATLAB `collapsepow = @(pwr) sum(reshape(pwr, es, G), 2)`.

```rust
pub fn collapse_power_over_groups(grid: &crate::reference::grid::Grid, power: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `radial_map_layer`

**Attributes:**

- `MustUse { reason: None }`

Radial (x-y) power map of an **active-core** axial layer, `nx*ny` entries
indexed `ix*ny + iy`.

MATLAB local function `radialmaplayer`. The axial blocks of the NEACRP model
are: block 1 the lower reflector, blocks 2–17 the active core, block 18 the
upper reflector, each spanning `zscale` mesh layers. Active layer `L`
therefore spans global mesh layers `L*zscale+1 … (L+1)*zscale`, which is why
the offset is `L*zscale` and not `(L-1)*zscale`.

# Panics

If `layer` and `z_scale` address a mesh layer beyond `grid.nz` — which the
MATLAB would also do, as an index error.

```rust
pub fn radial_map_layer(grid: &crate::reference::grid::Grid, node_power: &[f64], layer: usize, z_scale: usize) -> Vec<f64> { /* ... */ }
```

#### Function `normalise_to_peak`

Scale a map so its peak is 1 — the MATLAB `radC5_z6/max(radC5_z6(:))`.

A non-positive or non-finite peak leaves the map untouched rather than
filling it with NaN; MATLAB would divide anyway. Recorded as a deliberate
difference in a degenerate case that produces no meaningful output either
way.

```rust
pub fn normalise_to_peak(map: &mut [f64]) { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_OUT_PREFIX`

Default output-file prefix, MATLAB `params.outprefix`.

```rust
pub const DEFAULT_OUT_PREFIX: &str = "neacrpa2t";
```

#### Constant `DEFAULT_FREQ_ITER`

Default number of flux solves per step under the exponential transform:
one predictor plus one frequency corrector.

```rust
pub const DEFAULT_FREQ_ITER: usize = 2;
```

#### Constant `PHASE2_NODAL_REFINEMENTS`

Refinement passes of the nodal correction at the fixed converged steady flux
in Phase 2 — the MATLAB's initial call plus four more.

```rust
pub const PHASE2_NODAL_REFINEMENTS: usize = 4;
```

#### Constant `PHASE2_MAX_POWER_ITERATIONS`

Maximum power iterations in the Phase-2 re-equilibration.

Yan Ren's note: *"heavily rodded cores have a high dominance ratio — allow
many cheap triangular-solve iterations rather than exiting unconverged"*.

```rust
pub const PHASE2_MAX_POWER_ITERATIONS: usize = 5000;
```

#### Constant `PHASE2_TOL`

Fission-source and `k_eff` tolerance of the Phase-2 re-equilibration \[-\].

```rust
pub const PHASE2_TOL: f64 = 1.0e-9;
```

#### Constant `DIVERGENCE_POWER_RATIO`

Divergence guard on the relative power \[-\].

Deliberately far above any physical excursion: hot-zero-power cases start
at `P0 ~ kW`, so `P/P0 ~ 1e6` is real physics, and only `> 1e12` is taken
as a blown-up solution.

```rust
pub const DIVERGENCE_POWER_RATIO: f64 = 1.0e12;
```

#### Constant `OMEGA_DT_MIN`

Lower clamp on the per-step exponent `omega*dt` \[-\].

A **physics** bound rather than overflow protection: keeping
`omega*dt >= -0.9` also keeps the transformed time-derivative coefficient
`omega + 1/dt` positive.

```rust
pub const OMEGA_DT_MIN: f64 = -0.9;
```

#### Constant `OMEGA_DT_MAX`

Upper clamp on the per-step exponent `omega*dt` \[-\], i.e. at most a
factor `e^2 ≈ 7.4` growth per step. Keeps the transform effective for the
global mode while bounding pathological extrapolation.

```rust
pub const OMEGA_DT_MAX: f64 = 2.0;
```

### Re-exports

#### Re-export `search_critical_boron`

```rust
pub use critical_boron::search_critical_boron;
```

#### Re-export `CriticalBoronOutput`

```rust
pub use critical_boron::CriticalBoronOutput;
```

#### Re-export `CouplingError`

```rust
pub use error::CouplingError;
```

#### Re-export `Result`

```rust
pub use error::Result;
```

#### Re-export `solve_coupled_steady`

```rust
pub use steady::solve_coupled_steady;
```

#### Re-export `SteadyOutput`

```rust
pub use steady::SteadyOutput;
```

#### Re-export `solve_coupled_transient`

```rust
pub use transient::solve_coupled_transient;
```

#### Re-export `TransientOutput`

```rust
pub use transient::TransientOutput;
```

## Module `fixtures`

Loading the reference fixtures captured from Yan Ren's MATLAB.

# What a fixture is, and what it is not

These files record what Yan Ren's implementation produced when it was run
under GNU Octave — see `tests/fixtures/<case>/PROVENANCE.md` for the
interpreter, the shims applied, and the capture date. They pin the
*reference*, not the *truth*: agreement with a fixture shows the Rust
translation is faithful, which is a different claim from being correct.
Comparison against the published IAEA-3D benchmark values is a separate
check (`docs/bedok-port-scoping.md` §4).

# Two tiers of fixture, and why

**Reduced, committed** — under `tests/fixtures/<case>/`, about 20 kB. These
are the quantities the IAEA-3D benchmark itself reports, and they are what a
routine parity gate compares:

| File | Shape | Loader |
|---|---|---|
| `k_eff.csv` | scalar | [`load_scalar`] |
| `final_residuals.csv` | 1 × 2 | [`load_row`] |
| `radial_power_map.csv` | 17 × 17 | [`load_matrix`] |
| `axial_power_profile.csv` | 19 × 1 | [`load_matrix`] |

The reduced files are **plain matrices with no index columns** — 17 rows of
17 comma-separated values, and 19 rows of one value respectively.

**Full node-level, not committed** — about 1.4 MB of text under
`collaboration/bedok-full-fixtures/<case>/` (gitignored, regenerable in
~77 s with the command in [`REGENERATE_FULL_FIXTURES`]). These are
`power_density.csv`, `fission_source.csv` and `scalar_flux.csv`, and they
only matter once a parity failure has to be pinned to a specific node.
A fresh clone will not have them, so anything that reads them must check
[`full_fixtures_available`] first and skip with a message rather than fail.

# Indexed-field format

The full fields have no header row and are laid out as

```text
g,ix,iy,iz,value[,value...]
```

with **1-based** MATLAB indices in the first four columns. Values are
written at `%.17g`, so every entry round-trips through an IEEE double
exactly — a fixture comparison is therefore never limited by the file
format.

The explicit index columns exist so the port never infers the flattening
order. **Every loader here routes those indices through
[`Grid::index_from_matlab`]**, the single place the 1-based → 0-based
conversion is allowed to happen. Nothing in this module subtracts one by
hand, and neither should any caller: a silent off-by-one in the index
convention permutes the reactor without crashing anything, which is the
failure mode the [`grid`](crate::reference::grid) module exists to prevent.

Row order within a file is *not* trusted. Each row is placed at the flat
index its own coordinates dictate, and the loader reports any slot written
twice or left empty.

# Repo-local paths

[`fixture_dir`] and [`full_fixture_dir`] resolve against
`CARGO_MANIFEST_DIR`, so they only work in a checkout of this repository.
The crate is `publish = false` and the fixtures live under `tests/`, so
this is deliberate rather than a limitation to work around.

```rust
pub mod fixtures { /* ... */ }
```

### Types

#### Struct `Iaea3dReduced`

The committed reduced IAEA-3D reference quantities.

This is what a routine parity gate compares against: the eigenvalue, the
residuals the reference stopped at, and the two power shapes the IAEA-3D
benchmark itself reports. Present in every clone.

```rust
pub struct Iaea3dReduced {
    pub grid: crate::reference::grid::Grid,
    pub k_eff: f64,
    pub fission_source_residual: f64,
    pub k_eff_residual: f64,
    pub radial_power_map: Vec<f64>,
    pub axial_power_profile: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `crate::reference::grid::Grid` | The grid the fixtures were captured on. See [`iaea3d_grid`]. |
| `k_eff` | `f64` | Converged multiplication factor \[-\]. Matches [`IAEA3D_K_EFF`]. |
| `fission_source_residual` | `f64` | Final fission-source residual the MATLAB stopped at \[-\].<br><br>Column 1 of `final_residuals.csv`; 9.611040e-07 as captured. The fields<br>are therefore only determined to about this level — a tolerance tighter<br>than the reference's own convergence criterion is not meaningful. |
| `k_eff_residual` | `f64` | Final `k_eff` residual the MATLAB stopped at \[-\].<br><br>Column 2 of `final_residuals.csv`; 9.272337e-10 as captured. This sets<br>the floor on how tightly *any* faithful translation can be expected to<br>reproduce `k_eff`: the reference itself is converged only this far. |
| `radial_power_map` | `Vec<f64>` | Radial power map \[-\], `nx * ny` entries, **row-major with `ix` as the<br>row index**: entry `(ix, iy)` sits at `ix * ny + iy`.<br><br>Power summed over `z` and over both energy groups, then normalised so<br>the mean over *powered* (non-zero) nodes is exactly 1. The 112 unpowered<br>reflector positions of the 289 are exact zeros.<br><br># Orientation caveat<br><br>The captured map is symmetric under transposition — IAEA-3D is<br>quadrant-symmetric — so the data cannot distinguish "row = `ix`" from<br>"row = `iy`". Row-is-`ix` is assumed here because that is what MATLAB's<br>`writematrix` of a `[maxix × maxiy]` array produces. Any future<br>asymmetric case must re-check this rather than inherit the assumption. |
| `axial_power_profile` | `Vec<f64>` | Axial power profile \[-\], `nz` entries indexed by `iz`.<br><br>Power summed over `x` and `y` and over both groups, normalised the same<br>way: mean 1 over the 17 powered planes, with the two reflector planes<br>exactly zero. |

##### Implementations

###### Methods

- ```rust
  pub fn load() -> Result<Self> { /* ... */ }
  ```
  Loads the reduced fixtures from the in-repo directory returned by

- ```rust
  pub fn load_from<P: AsRef<Path>>(dir: P) -> Result<Self> { /* ... */ }
  ```
  Loads the reduced fixtures from an explicit directory.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Iaea3dReduced { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Struct `Iaea3dFullFields`

The uncommitted full node-level IAEA-3D reference fields.

Field vectors are all `grid.state_len()` long (10,982 entries: 5,491 nodes
× 2 groups) and share the 0-based flat ordering documented on
[`Grid::index`]. Loading these is opt-in: they are regenerable, gitignored,
and only needed to localise a parity failure to a node.

```rust
pub struct Iaea3dFullFields {
    pub grid: crate::reference::grid::Grid,
    pub power_density: Vec<f64>,
    pub fission_source: Vec<f64>,
    pub scalar_flux: Vec<f64>,
    pub scalar_flux_iterates: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `crate::reference::grid::Grid` | The grid the fixtures were captured on. |
| `power_density` | `Vec<f64>` | Nodal power density, MATLAB `pwrdens`.<br><br>Units follow the MATLAB, which carries the benchmark's own<br>normalisation rather than an SI-typed quantity; the reference path<br>deliberately does not use `uom`, so that its arithmetic stays<br>line-for-line comparable with the original (see<br>[`Geometry`](crate::reference::grid::Geometry)). |
| `fission_source` | `Vec<f64>` | Nodal fission source \[-\], MATLAB `fissionsource`. |
| `scalar_flux` | `Vec<f64>` | Converged scalar flux \[-\], column 1 of `scalar_flux.csv`. |
| `scalar_flux_iterates` | `Vec<Vec<f64>>` | The four retained flux iterates, columns 2–5 of `scalar_flux.csv`.<br><br>Kept because the MATLAB's fission-source extrapolation path consumes<br>them, so a translation of that path can be checked iterate by iterate<br>rather than only at convergence. |

##### Implementations

###### Methods

- ```rust
  pub fn try_load() -> Result<Option<Self>> { /* ... */ }
  ```
  Loads the full fields if they are present, returning `Ok(None)` if they

- ```rust
  pub fn load_from<P: AsRef<Path>>(dir: P) -> Result<Self> { /* ... */ }
  ```
  Loads the full fields from an explicit directory, failing if absent.

- ```rust
  pub fn radial_power_map(self: &Self) -> Result<Vec<f64>> { /* ... */ }
  ```
  Reduces [`Self::power_density`] to the radial map shape of

- ```rust
  pub fn axial_power_profile(self: &Self) -> Result<Vec<f64>> { /* ... */ }
  ```
  Reduces [`Self::power_density`] to the axial profile shape of

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Iaea3dFullFields { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

### Functions

#### Function `iaea3d_grid`

The node grid the IAEA-3D fixtures were captured on.

17 × 17 × 19 nodes in 2 energy groups: 5,491 nodes and 10,982 state
entries. Note `nz = 19`, not the 18 nodes the case input requests — the
MATLAB case constructor appends an axial reflector plane.

# Errors

Cannot fail in practice; the signature is fallible only because
[`Grid::new`] is.

```rust
pub fn iaea3d_grid() -> crate::error::Result<crate::reference::grid::Grid> { /* ... */ }
```

#### Function `fixture_dir`

**Attributes:**

- `MustUse { reason: None }`

Absolute path of the **committed reduced** fixture directory for `case`,
e.g. [`IAEA3D`].

Resolves to `<crate root>/tests/fixtures/<case>`. Repo-local; see the
module docs.

```rust
pub fn fixture_dir(case: &str) -> std::path::PathBuf { /* ... */ }
```

#### Function `full_fixture_dir`

**Attributes:**

- `MustUse { reason: None }`

Absolute path of the **uncommitted full node-level** fixture directory for
`case`.

Resolves to `<workspace root>/collaboration/bedok-full-fixtures/<case>`,
unless [`FULL_FIXTURE_DIR_ENV`] is set, in which case that path is used as
the parent directory instead. The directory is gitignored and may well not
exist — check [`full_fixtures_available`] before reading from it.

```rust
pub fn full_fixture_dir(case: &str) -> std::path::PathBuf { /* ... */ }
```

#### Function `full_fixtures_available`

**Attributes:**

- `MustUse { reason: None }`

Whether the full node-level fixtures for `case` are present on this
machine.

Checks for the directory and for every file [`Iaea3dFullFields`] reads, so
a partial regeneration counts as absent rather than failing halfway through
a comparison.

```rust
pub fn full_fixtures_available(case: &str) -> bool { /* ... */ }
```

#### Function `load_scalar`

Reads a fixture holding exactly one number on one line, such as
`k_eff.csv`.

# Errors

[`BedokError::Fixture`] if the file is missing, empty, holds more than one
row or column, or the value does not parse as a float.

```rust
pub fn load_scalar<P: AsRef<std::path::Path>>(path: P) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `load_row`

Reads a fixture holding one row of numbers, such as `final_residuals.csv`.

# Errors

[`BedokError::Fixture`] if the file is missing, empty, holds more than one
row, or any value fails to parse.

```rust
pub fn load_row<P: AsRef<std::path::Path>>(path: P) -> crate::error::Result<Vec<f64>> { /* ... */ }
```

#### Function `load_matrix`

Reads a plain matrix CSV — no index columns — into a **row-major** flat
vector of length `rows * cols`.

This is the shape of the committed reduced fixtures:
`radial_power_map.csv` is `load_matrix(path, 17, 17)` and
`axial_power_profile.csv` is `load_matrix(path, 19, 1)`.

Row-major means entry `(r, c)` sits at `r * cols + c`. For the radial map
that is `ix * ny + iy` — see [`Iaea3dReduced::radial_power_map`] for the
orientation convention and its one unresolved ambiguity.

# Errors

[`BedokError::Fixture`] if the row count differs from `rows`, any row has a
column count differing from `cols`, or any entry fails to parse.

```rust
pub fn load_matrix<P: AsRef<std::path::Path>>(path: P, rows: usize, cols: usize) -> crate::error::Result<Vec<f64>> { /* ... */ }
```

#### Function `load_field`

Reads a single-valued indexed field CSV (`g,ix,iy,iz,value`) into a flat
vector in this crate's 0-based ordering.

The returned vector has length [`Grid::state_len`], with entry `i` holding
the value whose MATLAB coordinates map to flat index `i` under
[`Grid::index_from_matlab`].

# Errors

[`BedokError::Fixture`] if the row count differs from `grid.state_len()`,
a row has the wrong number of columns, an index is out of range for `grid`,
two rows claim the same node, or any field fails to parse.

```rust
pub fn load_field<P: AsRef<std::path::Path>>(path: P, grid: &crate::reference::grid::Grid) -> crate::error::Result<Vec<f64>> { /* ... */ }
```

#### Function `load_field_columns`

Reads a multi-column indexed field CSV (`g,ix,iy,iz,v1,…,vN`) into
`columns` flat vectors, each in this crate's 0-based ordering.

`scalar_flux.csv` is the case in point: pass
[`SCALAR_FLUX_COLUMNS`] and take element 0 for the converged flux,
elements 1–4 for the retained iterates.

# Errors

As [`load_field`], plus [`BedokError::Fixture`] if `columns` is zero or a
row does not carry exactly `4 + columns` fields.

```rust
pub fn load_field_columns<P: AsRef<std::path::Path>>(path: P, grid: &crate::reference::grid::Grid, columns: usize) -> crate::error::Result<Vec<Vec<f64>>> { /* ... */ }
```

#### Function `radial_power_map`

Collapses a full node-level power field to the committed radial map.

Sums over `z` and over all energy groups, then normalises so the mean over
non-zero entries is exactly 1. Returns `nx * ny` entries, row-major with
`ix` as the row index.

# Why an unweighted sum

No node-volume weighting is applied, because the capture script does not
apply any. That is verified rather than assumed: reducing the captured
`power_density.csv` this way reproduces the committed
`radial_power_map.csv` to 0.0 absolute difference — bit-exact — and the
axial profile to 1.3e-15. The IAEA-3D mesh is uniform, so the distinction
would not show up here anyway; a non-uniform case must re-derive it.

# Errors

[`BedokError::Fixture`] if `power` is not `grid.state_len()` long, or if
every entry is zero (nothing to normalise against).

```rust
pub fn radial_power_map(power: &[f64], grid: &crate::reference::grid::Grid) -> crate::error::Result<Vec<f64>> { /* ... */ }
```

#### Function `axial_power_profile`

Collapses a full node-level power field to the committed axial profile.

Sums over `x`, `y` and all energy groups, then normalises so the mean over
non-zero entries is exactly 1. Returns `nz` entries indexed by `iz`. See
[`radial_power_map`] on the absence of volume weighting.

# Errors

[`BedokError::Fixture`] if `power` is not `grid.state_len()` long, or if
every entry is zero.

```rust
pub fn axial_power_profile(power: &[f64], grid: &crate::reference::grid::Grid) -> crate::error::Result<Vec<f64>> { /* ... */ }
```

### Constants and Statics

#### Constant `IAEA3D`

Fixture directory name for the IAEA-3D steady-state case.

```rust
pub const IAEA3D: &str = "iaea3d";
```

#### Constant `IAEA3D_K_EFF`

`k_eff` recorded in `tests/fixtures/iaea3d/k_eff.csv`.

Yan Ren's converged eigenvalue for IAEA-3D — `1.0290842762` to the ten
figures quoted in `PROVENANCE.md`, carried here at the full `%.17g`
precision of the file. This is the *reference* value, not the published
benchmark value; see the module docs on the difference.

```rust
pub const IAEA3D_K_EFF: f64 = 1.0290842761799579;
```

#### Constant `SCALAR_FLUX_COLUMNS`

Number of value columns in the full `scalar_flux.csv`.

Column 1 is the converged flux; columns 2–5 are retained iterates consumed
by the MATLAB's fission-source extrapolation path.

```rust
pub const SCALAR_FLUX_COLUMNS: usize = 5;
```

#### Constant `REGENERATE_FULL_FIXTURES`

Shell command that regenerates the uncommitted full node-level fixtures.

Quoted verbatim from `tests/fixtures/iaea3d/PROVENANCE.md` so that a test
which skips for want of those files can name the exact fix.

```rust
pub const REGENERATE_FULL_FIXTURES: &str = "cd collaboration/BEDOKfiles && octave --no-gui --quiet --eval \"addpath(\'../octave-shims\'); addpath(\'.\'); capture_iaea3ds\"";
```

#### Constant `FULL_FIXTURE_DIR_ENV`

Environment variable that overrides where the full fixtures are looked for.

Unset in normal use. Exists so the full fields can be regenerated somewhere
other than the default gitignored directory without editing code.

```rust
pub const FULL_FIXTURE_DIR_ENV: &str = "BEDOK_FULL_FIXTURES";
```

## Module `grid`

Grid geometry and the state-vector index convention.

# Why this module exists first

Every field in BEDOK — flux, fission source, power density, cross sections —
is stored as a flat vector indexed by a single rule. Getting that rule wrong
does not crash anything: it silently permutes the reactor, and the solver
converges happily to a wrong answer. It is the highest-risk single line in
the whole translation, so it is pinned here, once, with tests.

# The convention

Taken verbatim from Yan Ren's `main_exec_diff3d.m:176`, which is the
authoritative statement of how the MATLAB indexes its state vectors:

```text
idx = (g-1)*maxix*maxiy*maxiz + (ix-1)*maxiy*maxiz + (iy-1)*maxiz + iz
```

That is **group-major**, then `ix`, then `iy`, with `iz` varying fastest.
The MATLAB indices are 1-based; this port is 0-based, so the same rule
becomes:

```text
idx = g*nodes + ix*(ny*nz) + iy*nz + iz
```

with `nodes = nx*ny*nz`. The reference fixtures under
`tests/fixtures/*/` carry explicit `g,ix,iy,iz` columns using the **1-based**
MATLAB values, precisely so that this conversion is checked rather than
assumed.

```rust
pub mod grid { /* ... */ }
```

### Types

#### Struct `Grid`

The node grid a case is discretised on.

Holds only the counts. Physical dimensions live in [`Geometry`].

# Note on `nz`

A case constructor may *change* the axial node count relative to what the
user requested — `iaea3ds` raises `maxiz` from 18 to 19 when it appends the
axial reflector. Always read the grid back from the built case rather than
from the input parameters.

```rust
pub struct Grid {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub ngroups: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx` | `usize` | Number of nodes in x. MATLAB `params.maxix`. |
| `ny` | `usize` | Number of nodes in y. MATLAB `params.maxiy`. |
| `nz` | `usize` | Number of nodes in z. MATLAB `params.maxiz`. |
| `ngroups` | `usize` | Number of energy groups. MATLAB `params.G`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(nx: usize, ny: usize, nz: usize, ngroups: usize) -> Result<Self> { /* ... */ }
  ```
  A grid with `nx` × `ny` × `nz` nodes and `ngroups` energy groups.

- ```rust
  pub const fn nodes(self: &Self) -> usize { /* ... */ }
  ```
  Number of spatial nodes, `nx*ny*nz`, ignoring energy groups.

- ```rust
  pub const fn state_len(self: &Self) -> usize { /* ... */ }
  ```
  Length of a full state vector, `nodes * ngroups`.

- ```rust
  pub fn index(self: &Self, g: usize, ix: usize, iy: usize, iz: usize) -> usize { /* ... */ }
  ```
  Flat index of node `(ix, iy, iz)` in group `g`, all **0-based**.

- ```rust
  pub fn unindex(self: &Self, idx: usize) -> Result<(usize, usize, usize, usize)> { /* ... */ }
  ```
  Inverse of [`index`](Self::index): flat index back to `(g, ix, iy, iz)`,

- ```rust
  pub fn index_from_matlab(self: &Self, g: usize, ix: usize, iy: usize, iz: usize) -> Result<usize> { /* ... */ }
  ```
  Flat index from the **1-based** indices used by the MATLAB and written

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Grid { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Grid) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `Geometry`

Physical extents and per-node dimensions of a case.

Mirrors Yan Ren's `geometry` struct. All lengths are in centimetres, the
unit the benchmark specifications and the MATLAB both use; `uom` types are
deliberately **not** used inside the reference translation, so that the
arithmetic stays line-for-line comparable with the original. The
`substituted` path is where typed quantities belong.

```rust
pub struct Geometry {
    pub grid: Grid,
    pub x_total: f64,
    pub y_total: f64,
    pub z_total: f64,
    pub lx: Vec<f64>,
    pub ly: Vec<f64>,
    pub lz: Vec<f64>,
    pub volume: Vec<f64>,
    pub which_sigma: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `Grid` | The node grid. |
| `x_total` | `f64` | Total core extent in x \[cm\]. MATLAB `geometry.Xtot`. |
| `y_total` | `f64` | Total core extent in y \[cm\]. MATLAB `geometry.Ytot`. |
| `z_total` | `f64` | Total core extent in z \[cm\]. MATLAB `geometry.Ztot`. |
| `lx` | `Vec<f64>` | Node width in x \[cm\], **one entry per spatial node** (length<br>[`Grid::nodes`]). MATLAB `geometry.Lx`.<br><br>Index with `ix*(ny*nz) + iy*nz + iz`, i.e. [`Grid::index`] with `g = 0`.<br><br># Not one per axis index<br><br>An earlier version of this doc comment said "one per x index". That was<br>wrong, and it was caught independently by three separate ports of the<br>MATLAB — see `neacrpa2.m:43`, `sigmavalupd3d_handler.m:57` and<br>`driftflux6_solverstatic3d.m:63`, all of which index these per node. The<br>distinction matters because an axially varying mesh is exactly the case<br>a per-axis reading would silently get wrong. |
| `ly` | `Vec<f64>` | Node width in y \[cm\], one entry per spatial node. MATLAB<br>`geometry.Ly`. Indexed as [`lx`](Self::lx). |
| `lz` | `Vec<f64>` | Node height in z \[cm\], one entry per spatial node. MATLAB<br>`geometry.Lz`. Indexed as [`lx`](Self::lx). |
| `volume` | `Vec<f64>` | Node volume \[cm³\], one per spatial node. MATLAB `geometry.Vi`. |
| `which_sigma` | `Vec<usize>` | Material index per spatial node, 1-based as in the MATLAB.<br>MATLAB `geometry.whichsigma`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Geometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

## Module `nodal`

Semi-analytic nodal method (SANM) — the neutronics core.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Fourteen MATLAB files, ~3,857 lines, are translated here. Each Rust module
names its `.m` source in its own header; the map is:

| MATLAB file | Rust module |
|---|---|
| `sanodaldiffusion_solverxyz.m` | [`sanm_solver`] |
| `diffusion_solverxyz.m` | [`finite_difference_solver`] |
| `calc_sanodalxyz.m` | [`nodal_correction`] |
| `calc_a1234_expansionxyz.m` | [`expansion`] |
| `calc_a1_expansionxyz.m` | [`first_moment`] |
| `calc_ABEFGHxyz.m` | [`nodal_coefficients`] |
| `calc_bucklingxyz.m` | [`buckling`] |
| `calc_transleakagexyz.m` | [`transverse_leakage`] |
| `calc_1sttransleakagexyz.m`, `calc_2ndtransleakagexyz.m` | [`leakage_moments`] |
| `makegradDxyz.m` | [`gradient_diffusion`] |
| `makesigmadfxyz.m`, `calcdiffvalues3d.m` | [`cross_sections`] |
| `fiss_src_extrapolatexyz.m` | [`fission_source`] |
| *(the `geometry`/`params` structs, `handle3dcoords.m`)* | [`geometry`] |
| *(MATLAB sparse built-ins, `\`, `decomposition`)* | [`sparse`] |
| *(the `scalar_flux` history matrix)* | [`flux_history`] |

# What the method is

Coarse-mesh nodal diffusion. The core is divided into assembly-sized nodes
— 20 cm across in the benchmarks, far too coarse for finite difference to be
accurate. Rather than refine the mesh, the semi-analytic nodal method solves
the one-dimensional diffusion equation *analytically* within each node
along each axis, treating leakage in the other two directions as a known
transverse source fitted by a parabola. The resulting surface currents are
folded back into a finite-difference-shaped operator as a **correction** to
the coupling coefficients, so the global solve stays a sparse linear system
of the same size and sparsity as plain finite difference.

The data flow of one nodal update, which is also the module dependency
order:

```text
cross_sections ---> gradient_diffusion ---+
       |                                  |
       +---> nodal_coefficients ----------+
                                          v
           transverse_leakage ---> leakage_moments ---> buckling
                                          |                 |
                                          +--> first_moment<+
                                                   |
                                             expansion (A1..A4)
                                                   |
                                           nodal_correction
                                                   |
                                              sanm_solver
```

# Entry points

- [`sanm_solver::solve`] — the nodal `k`-eigenvalue solve.
- [`finite_difference_solver::solve`] — the same problem without the nodal
  correction, as a cross-check.

Everything else exists so those two can be read, tested and debugged a
stage at a time.

# Faithfulness and its consequences

This is a stage-1 translation: structure, iteration order and convergence
logic follow the MATLAB, and nothing that looked wrong was repaired. The
places where the reference is unfinished, fragile or self-inconsistent are
recorded in the doc comment of the item where they occur, under a heading
that says so. The ones a reader should know about before trusting a result:

- **`Nc > 0` does not work**, in the MATLAB or here — see
  [`geometry::NodalParams::n_precursor_groups`].
- **A direction with a single node is not a supported mesh**: the boundary
  blocks index a neighbour outside the grid. Two nodes per direction is the
  minimum.
- **The near-zero-flux guard in [`nodal_correction`] silently falls back to
  finite difference**, on a threshold relative to the global flux maximum.
- **A reflective outer face over a node with no material** makes an
  outer-face system exactly singular; the reference produces `Inf`/`NaN`
  there, and so does this port — see [`first_moment::assemble`].
- **The `1e6` substitution for a zero diffusion coefficient** in
  [`expansion::assemble`] is a magic guard, not a physical value.

# Verification status

**Unverified against the benchmarks.** The unit tests here check formulas,
index maps and a handful of analytically-known limits (an infinite-medium
`k_inf`, conservation on a reflective block). They do not establish that the
method reproduces IAEA-3D or NEACRP, and no comparison against Yan Ren's own
results has been made — see `docs/bedok-port-scoping.md` §4 for why
"reproduces Yan Ren's results" must not be claimed.

```rust
pub mod nodal { /* ... */ }
```

### Modules

## Module `buckling`

The node-wise buckling operators.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `calc_bucklingxyz.m` (function `calc_bucklingxyz`).

```rust
pub mod buckling { /* ... */ }
```

### Types

#### Struct `Buckling`

The three directional buckling operators, one per coordinate direction.

Each is `philen` square and **dimensionless**: it is the net removal
operator `sigma_tot - sigma_s - sigma_f/k_eff` \[cm⁻¹\] scaled by
`0.25*L²/D` \[cm\], i.e. the squared optical half-width of the node in that
direction. Rows and columns are state indices; the only nonzero columns of
row `idx` are the `G` group indices at `idx`'s own spatial node, so each
operator is block-diagonal in space with `G`×`G` blocks.

```rust
pub struct Buckling {
    pub x: super::sparse::SparseMatrix,
    pub y: super::sparse::SparseMatrix,
    pub z: super::sparse::SparseMatrix,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `super::sparse::SparseMatrix` | x-direction buckling operator \[dimensionless\]. |
| `y` | `super::sparse::SparseMatrix` | y-direction buckling operator \[dimensionless\]. |
| `z` | `super::sparse::SparseMatrix` | z-direction buckling operator \[dimensionless\]. |

##### Implementations

###### Methods

- ```rust
  pub fn axis(self: &Self, axis: Axis) -> &SparseMatrix { /* ... */ }
  ```
  The operator along `axis`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Buckling { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Buckling) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `assemble`

**Attributes:**

- `MustUse { reason: None }`

Builds the three buckling operators — `calc_bucklingxyz.m`.

Entry `(idx, c)` of the x operator is

```text
(sigma_tot - sigma_s - sigma_f/k_eff)(idx, c) * 0.25 * Lx(idx)^2 / D(idx)
```

and likewise for y and z, with the element order kept as the MATLAB writes
it (`Bt*0.25 .* L .* L ./ D`) so the rounding matches.

`k_eff` is the current eigenvalue estimate \[dimensionless\], typically
within a few percent of 1. `diffusion` is the flat diffusion-coefficient
state vector \[cm\]; nodes where its group-1 entry is zero are skipped
entirely, leaving the operators empty there.

# Caching omitted

The MATLAB caches the `k_eff`-independent part in `persistent` storage,
keyed on a fingerprint of the inputs, and rebuilds only when the
fingerprint changes. That is a pure speed optimisation with no effect on
results, and it is not reproduced — a `persistent` cache shared across
unrelated cases is also a correctness hazard this port has no reason to
inherit.

# Panics

If `diffusion.len()` differs from the neutronics state length.

```rust
pub fn assemble(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, sigma: &super::cross_sections::CrossSectionOperators, diffusion: &[f64], k_eff: f64) -> Buckling { /* ... */ }
```

## Module `cross_sections`

Cross-section operators and diffusion coefficients.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Sources: `makesigmadfxyz.m` (function `makesigmadfxyz`) and
`calcdiffvalues3d.m` (function `calcdiffvalues3d`).

```rust
pub mod cross_sections { /* ... */ }
```

### Types

#### Struct `MaterialCrossSections`

Per-material multigroup cross-section data — Yan Ren's `sigmavalues` struct.

Materials are indexed **0-based here** but referred to by the **1-based**
index stored in `which_sigma`, exactly as in the MATLAB, where
`whichsigma == 0` means "no material at this node". Convert with `m - 1`.

# Units

- `total`, `fission`, `fission_prompt`, `scatter`: macroscopic cross
  sections \[cm⁻¹\].
- `nu`: neutrons per fission \[dimensionless\], typically 2.4–2.9.
- `chi`: fission spectrum \[dimensionless\], summing to 1 over groups.

```rust
pub struct MaterialCrossSections {
    pub total: Vec<Vec<f64>>,
    pub fission: Vec<Vec<f64>>,
    pub fission_prompt: Vec<Vec<f64>>,
    pub scatter: Vec<Vec<Vec<f64>>>,
    pub nu: Vec<Vec<f64>>,
    pub chi: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `Vec<Vec<f64>>` | `sigmavalues.tot(m,g)` — total (here: removal-bearing) cross section,<br>indexed `[material][group]` \[cm⁻¹\]. |
| `fission` | `Vec<Vec<f64>>` | `sigmavalues.f(m,g)` — fission cross section, `[material][group]`<br>\[cm⁻¹\]. |
| `fission_prompt` | `Vec<Vec<f64>>` | `sigmavalues.fp(m,g)` — prompt-fission cross section,<br>`[material][group]` \[cm⁻¹\].<br><br>May be left empty, reproducing the MATLAB's<br>`if ~isfield(sigmavalues,'fp'), sigmavalues.fp=zeros(...)` default. |
| `scatter` | `Vec<Vec<Vec<f64>>>` | `sigmavalues.s(m,gt,g)` — scattering from group `g` **into** group `gt`,<br>indexed `[material][to_group][from_group]` \[cm⁻¹\]. |
| `nu` | `Vec<Vec<f64>>` | `sigmavalues.nu(m,g)` — neutrons per fission, `[material][group]`<br>\[dimensionless\].<br><br># Note on the MATLAB's two ways of reading this<br><br>`makesigmadfxyz.m` reads `nu` twice with different index counts:<br>`nu(whichsigma(...))` (a *linear* index) when filling `sigma.nu`, and<br>`nu(whichsigma(...),g)` (a 2-D index) when filling `sigma.f`. Under<br>MATLAB's column-major linear indexing the first resolves to<br>`nu(material, 1)`, so the port uses `nu[m][0]` there and `nu[m][g]` in<br>the fission operator. Both readings are reproduced, not reconciled. |
| `chi` | `Vec<Vec<f64>>` | `sigmavalues.chi(m,g)` — fission spectrum, `[material][group]`<br>\[dimensionless\]. |

##### Implementations

###### Methods

- ```rust
  pub fn prompt_fission(self: &Self, m: usize, g: usize) -> f64 { /* ... */ }
  ```
  Prompt-fission cross section of material `m` (0-based) in group `g`,

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MaterialCrossSections { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MaterialCrossSections) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CrossSectionOperators`

The assembled cross-section operators — Yan Ren's `sigma` struct.

Every matrix is `philenf` × `philenf` (see
[`NodalParams::philenf`](super::geometry::NodalParams::philenf)) and carries
units of cm⁻¹. Rows and columns are state-vector indices, so a scattering
entry at `(idx_to, idx_from)` couples two energy groups at the *same*
spatial node — every operator here is block-diagonal in space.

```rust
pub struct CrossSectionOperators {
    pub total: super::sparse::SparseMatrix,
    pub fission: super::sparse::SparseMatrix,
    pub fission_prompt: super::sparse::SparseMatrix,
    pub fission_bare: super::sparse::SparseMatrix,
    pub scatter: super::sparse::SparseMatrix,
    pub scatter_self: super::sparse::SparseMatrix,
    pub nu: Vec<f64>,
    pub chi: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `super::sparse::SparseMatrix` | `sigma.tot` — diagonal total cross section \[cm⁻¹\]. |
| `fission` | `super::sparse::SparseMatrix` | `sigma.f` — the full fission-production operator<br>`chi(gt) * nu(g) * sigma_f(g)` \[cm⁻¹\], mapping the flux in group `g`<br>to production in group `gt`. |
| `fission_prompt` | `super::sparse::SparseMatrix` | `sigma.fp` — the same operator built from the *prompt* fission cross<br>section, `chi(gt) * sigma_fp(g)` \[cm⁻¹\].<br><br># Unfinished in the reference<br><br>Note the asymmetry with `fission`: `sigma.fp` omits the `nu` factor that<br>`sigma.f` includes. Whether `sigmavalues.fp` is meant to already contain<br>`nu * beta` is not stated anywhere in the snapshot. Translated as<br>written. |
| `fission_bare` | `super::sparse::SparseMatrix` | `sigma.fb` — the "bare" diagonal fission cross section `sigma_f(g)`,<br>with no `nu` and no `chi` \[cm⁻¹\]. |
| `scatter` | `super::sparse::SparseMatrix` | `sigma.s` — the full group-to-group scattering operator \[cm⁻¹\],<br>including within-group scattering on the diagonal. |
| `scatter_self` | `super::sparse::SparseMatrix` | `sigma.sd` — only the within-group (diagonal) part of `scatter`<br>\[cm⁻¹\]. |
| `nu` | `Vec<f64>` | `sigma.nu` — neutrons per fission at each state index<br>\[dimensionless\]; zero outside the core. |
| `chi` | `Vec<f64>` | `sigma.chi` — the fission spectrum as a `G` × `philen` table flattened<br>row-major, so entry `(gt, idx)` lives at `gt * philen + idx`<br>\[dimensionless\].<br><br>Note this is `philen`-wide, not `philenf`-wide, exactly as the MATLAB's<br>`schi=zeros(G,philen)`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CrossSectionOperators { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CrossSectionOperators) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `assemble_operators`

**Attributes:**

- `MustUse { reason: None }`

Assembles the cross-section operators — `makesigmadfxyz.m`, `mode == 1`.

Walks every node with a nonzero material index and, for each energy group,
writes the diagonal total/fission/self-scatter entries and the off-diagonal
fission-production and scattering entries.

`which_sigma` is the **1-based** material index per spatial node, indexed by
`ix*ny*nz + iy*nz + iz`; `0` means the node is outside the core and is
skipped entirely, leaving every operator zero there.

# Mode 2 is not ported

The MATLAB supports a second index convention (`mode == 2`) that carries
half-indices for a 2-D layout. Every call site in the SANM path passes
`mode = 1`, so only that is translated. Recording the reason it is not worth
resurrecting: mode 2's node loop reads
`for ix=m:m:m*maxix, for iy=m:m:m*maxiy, for iz=m:m:maxiz` — the `iz` bound
is missing its `m*` factor, so with `m = 2` the loop covers only the first
`maxiz/2` axial nodes. That is a defect in the reference, left unfixed here
per the translation rules.

# Panics

If `which_sigma.len()` differs from the node count, or a material index
exceeds the supplied cross-section tables.

```rust
pub fn assemble_operators(params: &super::geometry::NodalParams, values: &MaterialCrossSections, which_sigma: &[usize]) -> CrossSectionOperators { /* ... */ }
```

#### Function `diffusion_coefficients`

**Attributes:**

- `MustUse { reason: None }`

Diffusion coefficients per node and group \[cm\] — `calcdiffvalues3d.m`.

Computes `D = mode / ((2*mode + 1) * sigma_tot)`. With the default
`mode = 1` this is the standard `D = 1/(3 sigma_tr)` of P1 diffusion theory,
with `sigmavalues.tot` playing the part of the transport cross section;
higher `mode` values reproduce the alternative definitions the MATLAB
comment alludes to ("diffusion coefficients based on different
definitions") without documenting them further.

Nodes with `which_sigma == 0` are left at exactly `0.0`. Downstream code
keys "is this node in the core?" off `D == 0`, so that zero is load-bearing,
not just an initialiser.

The result is a flat state vector of length `G * nodes`, indexed by
[`Grid::index`] — the same layout the MATLAB reaches by
`reshape(permute(diffvalues,[3 2 1 4]), philen, 1)`.

# Valid ranges

`sigma_tot` must be strictly positive for any in-core material; a zero
entry yields an infinite `D`, which the MATLAB also produces and does not
guard against.

# Panics

If `which_sigma.len()` differs from `grid.nodes()`.

```rust
pub fn diffusion_coefficients(grid: crate::reference::grid::Grid, material_total: &[Vec<f64>], which_sigma: &[usize], mode: f64) -> Vec<f64> { /* ... */ }
```

## Module `expansion`

The full `A1`–`A4` semi-analytic expansion for one flux iterate.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `calc_a1234_expansionxyz.m` (function `calc_a1234_expansionxyz`).

```rust
pub mod expansion { /* ... */ }
```

### Types

#### Struct `Expansion`

The four expansion coefficient sets of the semi-analytic nodal method.

All four carry the units of the flux, neutrons cm⁻² s⁻¹. `A1` and `A3` are
the odd (surface) orders and carry a `*_first` variant for the low outer
face; `A2` and `A4` are the even (node-interior) orders.

```rust
pub struct Expansion {
    pub first_order: super::first_moment::OddExpansion,
    pub second_order: super::geometry::DirectionVectors,
    pub third_order: super::first_moment::OddExpansion,
    pub fourth_order: super::geometry::DirectionVectors,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `first_order` | `super::first_moment::OddExpansion` | `A1` — first order, from the face-continuity systems. |
| `second_order` | `super::geometry::DirectionVectors` | `A2` — second order, from the node-wise buckling systems. |
| `third_order` | `super::first_moment::OddExpansion` | `A3` — third order, algebraic in `A1`. |
| `fourth_order` | `super::geometry::DirectionVectors` | `A4` — fourth order, algebraic in `A2`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Expansion { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Expansion) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `assemble`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Computes `A1`–`A4` for the given flux and eigenvalue —
`calc_a1234_expansionxyz.m`.

The order is fixed by data dependence and is preserved exactly:

1. the zeroth transverse-leakage moment, from the current flux and the
   *previous* iteration's nodal correction terms;
2. the buckling operators at the current `k_eff`;
3. the first and second leakage moments;
4. `A2`, from a direct sparse solve of `diag(E)*Buck + 3I`;
5. `A4 = B * (Buck*A2 + L2)`;
6. `A1`, via [`first_moment::assemble`];
7. `A3 = A * (Buck*A1 + L1)`, and the same for the `*_first` variants.

`flux` is the current scalar flux \[neutrons cm⁻² s⁻¹\], `k_eff` the current
eigenvalue \[dimensionless\], `diffusion` the flat diffusion-coefficient
state vector \[cm\].

# The `1e6` guard, recorded

Before forming the transverse source the MATLAB replaces every zero
diffusion coefficient with `1e6` — a magic number whose only purpose is to
make the subsequent division finite (`diffvaluesDfix(diffvaluesDfix==0) =
1000000; %prevent division by 0 later`). It is not a physical value and it
leaks into `Ssource` at out-of-core nodes, where the source is then
`~1e-6` times the leakage rather than zero. Reproduced verbatim.

# Panics

If the `A2` system cannot be factorised, or any input length is wrong.

```rust
pub fn assemble(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, flux: &[f64], sigma: &super::cross_sections::CrossSectionOperators, diffusion: &[f64], grad_terms: &super::geometry::FaceTerms, previous_nodal_terms: &super::geometry::FaceTerms, k_eff: f64) -> Expansion { /* ... */ }
```

## Module `finite_difference_solver`

The finite-difference fallback `k`-eigenvalue solver.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `diffusion_solverxyz.m` (function `diffusion_solverxyz`).

This is the same power iteration as [`super::sanm_solver`] with the nodal
correction removed — plain coarse-mesh finite difference. It is useful as a
cross-check on the nodal path: the two must agree as the mesh is refined,
and disagree in a characteristic way on a coarse one.

```rust
pub mod finite_difference_solver { /* ... */ }
```

### Types

#### Struct `FiniteDifferenceSolution`

The result of the finite-difference source iteration.

```rust
pub struct FiniteDifferenceSolution {
    pub k_eff: f64,
    pub fission_source_residual: f64,
    pub k_eff_residual: f64,
    pub scalar_flux: Vec<f64>,
    pub fission_source: Vec<f64>,
    pub power_density: Vec<f64>,
    pub iterations: usize,
    pub termination: super::sanm_solver::Termination,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | Converged multiplication factor \[dimensionless\]. |
| `fission_source_residual` | `f64` | Final relative fission-source residual \[dimensionless\]. |
| `k_eff_residual` | `f64` | Final relative `k_eff` residual \[dimensionless\]. |
| `scalar_flux` | `Vec<f64>` | Converged scalar flux \[neutrons cm⁻² s⁻¹\], normalised so that the<br>fission-source 1-norm equals its initial value. |
| `fission_source` | `Vec<f64>` | Fission source \[neutrons cm⁻³ s⁻¹\]. |
| `power_density` | `Vec<f64>` | Node power density \[neutrons s⁻¹ per node\]. |
| `iterations` | `usize` | Source iterations performed. |
| `termination` | `super::sanm_solver::Termination` | Why the loop ended. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FiniteDifferenceSolution { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FiniteDifferenceSolution) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `solve`

**Attributes:**

- `MustUse { reason: None }`

Solves the `k`-eigenvalue problem by coarse-mesh finite difference —
`diffusion_solverxyz.m`.

The iteration is

```text
(gradD + sigma_tot - sigma_sd) phi = (1/k) F phi + (sigma_s - sigma_sd) phi
```

i.e. within-group scattering is kept on the left and group-to-group
scattering is lagged on the right. The left-hand operator does not change,
so it is factorised once.

# Differences from the nodal solver worth knowing about

- **The normalisation is applied every iteration**, not once at the end, and
  uses the fission source's **1-norm** where the nodal solver uses its plain
  sum. Both are the reference's own choices.
- **There is no flux history and no fission-source extrapolation.**
- On a non-converging exit the returned flux is the *previous* iterate, not
  the one that triggered the break: the MATLAB assigns
  `scalar_flux = scalar_flux_l_plus` only after the break test. Preserved.

# Not ported

- The unconditional `writematrix` diagnostic dumps and the `plotfig`
  surface plot.
- The `keychange` compaction branch, which renumbers the state vector to
  skip empty grid space via `convert_grid3d` / `convertsparsekey3d`. It is
  guarded by a hard-coded `keychange=0` in the reference and so is dead
  code there.
- The `philenf >= sizethresh` GMRES branch, dead for the same reason as in
  [`super::sanm_solver`].

# Panics

If the left-hand operator cannot be factorised, or if `Nc > 0` makes the
operator shapes disagree.

```rust
pub fn solve(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, values: &super::cross_sections::MaterialCrossSections, which_sigma: &[usize], initial_k_eff: f64) -> FiniteDifferenceSolution { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_ITERATIONS`

Hard iteration ceiling — MATLAB `maxiter=10000`.

```rust
pub const MAX_ITERATIONS: usize = 10_000;
```

#### Constant `TOLERANCE`

Convergence tolerance — MATLAB `diffusion.tol = 1E-6`.

Unlike the nodal solver, this one has no `params.innertol` override.

```rust
pub const TOLERANCE: f64 = 1e-6;
```

## Module `first_moment`

The first-order (`A1`) coefficients of the semi-analytic expansion.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `calc_a1_expansionxyz.m` (function `calc_a1_expansionxyz`).

This is where continuity is imposed. Every interior face gets a `2G`×`2G`
system enforcing current continuity (the first `G` rows) and
discontinuity-factor-weighted flux continuity (the second `G` rows) across
that face; every outer face gets a `G`×`G` system enforcing the boundary
condition. The unknowns are the two adjacent nodes' first-order expansion
coefficients, of which only the low node's are kept.

```rust
pub mod first_moment { /* ... */ }
```

### Types

#### Struct `OddExpansion`

An odd-order expansion coefficient set — MATLAB's `A1` and `A3` structs,
which share this shape.

Same units as the flux (neutrons cm⁻² s⁻¹). Each `x`/`y`/`z` entry belongs
to the face on the **high** side of its node; the `*_first` entries exist
only at the low-boundary node of each line, where the outer face has no
partner to share a coefficient with. The even orders `A2` and `A4` carry no
`*_first` variant and are plain [`DirectionVectors`].

```rust
pub struct OddExpansion {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub x_first: Vec<f64>,
    pub y_first: Vec<f64>,
    pub z_first: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Vec<f64>` | Coefficient on the high-x face of each node. MATLAB `A1.x`. |
| `y` | `Vec<f64>` | Coefficient on the high-y face of each node. MATLAB `A1.y`. |
| `z` | `Vec<f64>` | Coefficient on the high-z face of each node. MATLAB `A1.z`. |
| `x_first` | `Vec<f64>` | Coefficient on the low-x outer face. MATLAB `A1.xfirst`; zero except at<br>the low-boundary node of each x line. |
| `y_first` | `Vec<f64>` | Coefficient on the low-y outer face. MATLAB `A1.yfirst`. |
| `z_first` | `Vec<f64>` | Coefficient on the low-z outer face. MATLAB `A1.zfirst`. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(n: usize) -> Self { /* ... */ }
  ```
  All six vectors zeroed, for a state vector of length `n`.

- ```rust
  pub fn axis(self: &Self, axis: Axis) -> &[f64] { /* ... */ }
  ```
  The high-face coefficients along `axis`.

- ```rust
  pub fn axis_first(self: &Self, axis: Axis) -> &[f64] { /* ... */ }
  ```
  The low-outer-face coefficients along `axis`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OddExpansion { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &OddExpansion) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `assemble`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `Other("#[allow(clippy::needless_range_loop)]")`
- `MustUse { reason: None }`

Solves for the first-order expansion coefficients —
`calc_a1_expansionxyz.m`.

# Interior faces

For the face between low node `l` and high node `h`, with `d = 2D/L`
\[cm⁻¹\], `B` the within-node `G`×`G` buckling block \[dimensionless\], and
`f` the low/high assembly discontinuity factors, the `2G`×`2G` system is

```text
[ -d_l F_l B_l - d_l I     d_h F_h B_h + d_h I ] [ a_l ]   [  b_l + b'_h  ]
[  f_l A_l B_l + f_l I     f_h A_h B_h + f_h I ] [ a_h ] = [  b2'_h - b2_l ]
```

The top block is current continuity, the bottom flux continuity across the
discontinuity factors. Only `a_l` is retained.

# Outer faces

A `G`×`G` system per node, whose form depends on the boundary condition; see
the reference for the three cases. The low-boundary result is stored in the
`*_first` vectors, the high-boundary result overwrites the corresponding
`x`/`y`/`z` entry.

# Unfinished / fragile in the reference, recorded not repaired

- **The outer-face system is assembled even for nodes with no material.**
  The `if diffvalues(idx)==0, continue` guard skips filling the rows but not
  the diagonal adjustment that follows, and the solve always runs. With the
  default unit discontinuity factors the resulting matrix is `±I`, so the
  answer is harmless; with a reflective outer face the diagonal term is
  `diffvec`, which is zero there, making the system exactly singular. The
  MATLAB then warns and produces `Inf`/`NaN`; this port produces the same
  non-finite values (see [`solve_dense_in_place`]).
- **A comment mislabels the x high-boundary block as `%zhi node`.** Cosmetic
  only; the arithmetic underneath is the x one.

# Panics

If any input vector is not of the neutronics state length, or if a line has
a single node so that a boundary block reaches past the end of the grid.

```rust
pub fn assemble(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, flux: &[f64], second_order: &super::geometry::DirectionVectors, fourth_order: &super::geometry::DirectionVectors, leakage_first: &super::geometry::DirectionVectors, diffusion: &[f64], buckling: &super::buckling::Buckling) -> OddExpansion { /* ... */ }
```

## Module `fission_source`

Fission-source extrapolation for source-iteration acceleration.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `fiss_src_extrapolatexyz.m` (function `fiss_src_extrapolatexyz`).

The method is the fixed-weight extrapolation of B. R. Bandini, *A
three-dimensional transient neutronics routine for the TRAC-PF1 reactor
thermal hydraulic computer code*, PhD thesis, Pennsylvania State University,
1990, p. 51 — cited in the MATLAB header. The dominance ratio is estimated
from successive fission-source differences and turned into an extrapolation
weight `w = d/(1-d)`.

```rust
pub mod fission_source { /* ... */ }
```

### Types

#### Enum `ExtrapolationOutcome`

What the extrapolation decided to do, for the caller's diagnostics.

```rust
pub enum ExtrapolationOutcome {
    Applied,
    ZeroDenominator,
    NotAsymptotic,
}
```

##### Variants

###### `Applied`

The flux and fission source were extrapolated.

###### `ZeroDenominator`

Skipped: a dominance-ratio denominator was exactly zero, which happens
when the fission source is stagnant (typically the first few
iterations).

###### `NotAsymptotic`

Skipped: the two successive weight estimates disagreed by more than
10%, so the iteration is not yet in its asymptotic regime.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExtrapolationOutcome { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExtrapolationOutcome) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `extrapolate`

**Attributes:**

- `MustUse { reason: None }`

Extrapolates the flux and fission source — `fiss_src_extrapolatexyz.m`.

Reads the four newest columns of `history`, forms their fission sources, and
estimates the dominance ratio \[dimensionless, physically in `(0,1)`\] as

```text
d      = ||fs   - fs_1|| / ||fs_1 - fs_2||
d_prev = ||fs_1 - fs_2|| / ||fs_2 - fs_3||
```

Both are clamped to `[0, 0.99]`, converted to weights `w = d/(1-d)`, and
capped at `w = 5`. If the two weights agree to within 10% the current
iterate is extrapolated in place:

```text
fs      <- fs  + w*(fs  - fs_1)
phi(:,1) <- phi + w*(phi - phi_1)
```

Returns the (possibly extrapolated) fission source and what was decided.
The history's current column is updated in place; older columns are left
alone.

# Guards, and whose they are

The clamping, the `w <= 5` cap, the zero-denominator check and the `+1e-14`
in the agreement test are **all Yan Ren's own additions**, each with a
comment explaining the failure it prevents (`domir >= 1` producing a
negative or infinite weight; division by a near-zero weight). They are
reproduced exactly, including the asymmetry that `w` is capped but the
agreement test then compares the capped values, so two very different
uncapped ratios can be judged "asymptotic" once both saturate at 5.

# Panics

If the history has fewer than four columns, or its column length does not
match the fission operator.

```rust
pub fn extrapolate(fission_operator: &super::sparse::SparseMatrix, history: &mut super::flux_history::FluxHistory) -> (Vec<f64>, ExtrapolationOutcome) { /* ... */ }
```

## Module `flux_history`

The rolling flux history the source iteration keeps for acceleration.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: the `scalar_flux` matrix of `sanodaldiffusion_solverxyz.m`, whose
comment reads "history of 5 for acceleration schemes, can increase in
needed", and its consumer `fiss_src_extrapolatexyz.m`.

```rust
pub mod flux_history { /* ... */ }
```

### Types

#### Struct `FluxHistory`

A fixed-depth history of scalar-flux iterates, newest first.

Column 0 is the current iterate; column `j` is the iterate from `j` source
iterations ago. Every column is a full state vector of length `philenf`, in
neutrons cm⁻² s⁻¹ up to the arbitrary normalisation the eigenvalue problem
leaves free.

The default depth is 5, matching the MATLAB's `ones(philenf,5)`. Only the
first four columns are read (by the fission-source extrapolation); the
fifth is carried but unused, exactly as in the reference.

```rust
pub struct FluxHistory {
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
  pub fn filled(len: usize, depth: usize, value: f64) -> Self { /* ... */ }
  ```
  A history of `depth` identical columns, each `len` entries of `value` —

- ```rust
  pub fn broadcast(flux: &[f64], depth: usize) -> Self { /* ... */ }
  ```
  A history whose every column is a copy of `flux` — the MATLAB's

- ```rust
  pub fn from_columns(columns: Vec<Vec<f64>>) -> Self { /* ... */ }
  ```
  A history built from explicit columns, newest first.

- ```rust
  pub fn depth(self: &Self) -> usize { /* ... */ }
  ```
  Number of columns held.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Length of each column, i.e. the state-vector length.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the state vectors are empty.

- ```rust
  pub fn current(self: &Self) -> &[f64] { /* ... */ }
  ```
  The current iterate — MATLAB `scalar_flux(:,1)`.

- ```rust
  pub fn column(self: &Self, j: usize) -> &[f64] { /* ... */ }
  ```
  Column `j`, counting back from the current iterate at `j = 0`.

- ```rust
  pub fn set_current(self: &mut Self, flux: Vec<f64>) { /* ... */ }
  ```
  Overwrites the current iterate without shifting the history.

- ```rust
  pub fn push(self: &mut Self, flux: Vec<f64>) { /* ... */ }
  ```
  Shifts every column back one place and installs `flux` as the current

- ```rust
  pub fn scale(self: &mut Self, factor: f64) { /* ... */ }
  ```
  Multiplies every column by `factor` — the final renormalisation

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FluxHistory { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FluxHistory) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `geometry`

Geometry, boundary conditions and per-face bookkeeping for the SANM path.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

This file has no single `.m` counterpart. It gives Rust types to the pieces
of Yan Ren's `geometry` and `params` structs that every ported nodal file
reads, and to the `handle3dcoords.m` coordinate lookup:

- `geometry.Lx/Ly/Lz/Vi` and the `geometry.{x,y,z}{min,max}` boundary-
  condition strings ⇒ [`NodalGeometry`] and [`BoundaryCondition`].
- `geometry.{x,y,z}lows` / `{x,y,z}his`, the per-column active index range
  used to skip out-of-core nodes ⇒ [`ActiveRange`].
- `geometry.adf`, the `philen`×6 assembly-discontinuity-factor table, and
  the `philen`×6 `gradterms` / `nodalterms` tables ⇒ [`FaceTerms`].
- `geometry.nodalcoeffs` ⇒ [`NodalCoefficients`] (built in
  [`super::nodal_coefficients`]).
- `params.G` / `params.maxi{x,y,z}` / `params.Nc` ⇒ [`NodalParams`] plus
  [`Grid`](crate::reference::grid::Grid).

# Units

All lengths are centimetres and all volumes cubic centimetres, the units the
benchmark specifications and the MATLAB both use. `uom` types are
deliberately not used inside the reference translation, so the arithmetic
stays line-for-line comparable with the original.

```rust
pub mod geometry { /* ... */ }
```

### Types

#### Enum `Face`

Which face of a node a per-face quantity belongs to.

The MATLAB stores these as columns 1–6 of the `philen`×6 arrays `gradterms`,
`nodalterms` and `geometry.adf`. [`Face::column`] is the 0-based column, so
`Face::XMinus.column() == 0` is MATLAB column 1.

```rust
pub enum Face {
    XMinus,
    XPlus,
    YMinus,
    YPlus,
    ZMinus,
    ZPlus,
}
```

##### Variants

###### `XMinus`

Low-x face. MATLAB column 1.

###### `XPlus`

High-x face. MATLAB column 2.

###### `YMinus`

Low-y face. MATLAB column 3.

###### `YPlus`

High-y face. MATLAB column 4.

###### `ZMinus`

Low-z face. MATLAB column 5.

###### `ZPlus`

High-z face. MATLAB column 6.

##### Implementations

###### Methods

- ```rust
  pub const fn column(self: Self) -> usize { /* ... */ }
  ```
  The 0-based column this face occupies in a [`FaceTerms`] table.

- ```rust
  pub const fn minus(axis: Axis) -> Self { /* ... */ }
  ```
  The low-side face of `axis`.

- ```rust
  pub const fn plus(axis: Axis) -> Self { /* ... */ }
  ```
  The high-side face of `axis`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Face { /* ... */ }
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

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Face) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `Axis`

Which coordinate direction a quantity is taken along.

```rust
pub enum Axis {
    X,
    Y,
    Z,
}
```

##### Variants

###### `X`

x.

###### `Y`

y.

###### `Z`

z.

##### Implementations

###### Methods

- ```rust
  pub const fn node_count(self: Self, grid: Grid) -> usize { /* ... */ }
  ```
  Number of nodes along this axis.

- ```rust
  pub const fn line_counts(self: Self, grid: Grid) -> (usize, usize) { /* ... */ }
  ```
  Extents of the two indices that identify a line parallel to this axis.

- ```rust
  pub const fn coords(self: Self, k1: usize, k2: usize, pos: usize) -> (usize, usize, usize) { /* ... */ }
  ```
  The `(ix, iy, iz)` of the node at `pos` along this axis on line

- ```rust
  pub const fn stride(self: Self, grid: Grid) -> usize { /* ... */ }
  ```
  The state-vector stride to the next node along this axis: `1` in z,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Axis { /* ... */ }
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

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Axis) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `BoundaryCondition`

The outer-boundary condition on one face of the core.

Translates the MATLAB string fields `geometry.xmin`, `geometry.xmax`,
`geometry.ymin`, `geometry.ymax`, `geometry.zmin`, `geometry.zmax`, whose
only recognised values are `'vacuum'`, `'reflective'` and `'zeroflux'`.

# Unfinished in the reference

The MATLAB `switch` statements have no `otherwise` branch. An unrecognised
string therefore silently leaves the coefficient at whatever it was
initialised to — usually zero — rather than raising an error. The enum makes
that class of typo unrepresentable; the behaviour for the three real values
is unchanged.

```rust
pub enum BoundaryCondition {
    Vacuum,
    Reflective,
    ZeroFlux,
}
```

##### Variants

###### `Vacuum`

Zero incoming partial current (`'vacuum'`).

###### `Reflective`

Zero net current (`'reflective'`).

###### `ZeroFlux`

Zero flux at the surface (`'zeroflux'`).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoundaryCondition { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BoundaryCondition) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `BoundaryConditions`

The six outer-boundary conditions of a case.

```rust
pub struct BoundaryConditions {
    pub x_min: BoundaryCondition,
    pub x_max: BoundaryCondition,
    pub y_min: BoundaryCondition,
    pub y_max: BoundaryCondition,
    pub z_min: BoundaryCondition,
    pub z_max: BoundaryCondition,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x_min` | `BoundaryCondition` | `geometry.xmin`. |
| `x_max` | `BoundaryCondition` | `geometry.xmax`. |
| `y_min` | `BoundaryCondition` | `geometry.ymin`. |
| `y_max` | `BoundaryCondition` | `geometry.ymax`. |
| `z_min` | `BoundaryCondition` | `geometry.zmin`. |
| `z_max` | `BoundaryCondition` | `geometry.zmax`. |

##### Implementations

###### Methods

- ```rust
  pub const fn uniform(bc: BoundaryCondition) -> Self { /* ... */ }
  ```
  All six faces set to the same condition.

- ```rust
  pub const fn low(self: &Self, axis: Axis) -> BoundaryCondition { /* ... */ }
  ```
  The low-side condition along `axis`.

- ```rust
  pub const fn high(self: &Self, axis: Axis) -> BoundaryCondition { /* ... */ }
  ```
  The high-side condition along `axis`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoundaryConditions { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BoundaryConditions) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `ActiveRange`

The first and last in-core node index along one axis, for every line of
nodes parallel to that axis.

Translates the MATLAB pairs `geometry.zlows`/`geometry.zhis` (indexed by
`ix,iy`), `geometry.ylows`/`geometry.yhis` (indexed by `ix,iz`) and
`geometry.xlows`/`geometry.xhis` (indexed by `iy,iz`). Where the MATLAB
falls back to `ones(...)` / `maxi*(ones(...))` when the field is absent,
[`ActiveRange::full`] does the same.

Indices stored here are **0-based**; the MATLAB values are 1-based.

```rust
pub struct ActiveRange {
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
  pub fn full(first_len: usize, second_len: usize, n: usize) -> Self { /* ... */ }
  ```
  Every line spans the whole axis: low `0`, high `n-1`.

- ```rust
  pub fn new(first_len: usize, second_len: usize, lows: Vec<usize>, highs: Vec<usize>) -> Self { /* ... */ }
  ```
  Explicit per-line bounds, both **0-based** and inclusive.

- ```rust
  pub fn low(self: &Self, first: usize, second: usize) -> usize { /* ... */ }
  ```
  First in-core index on the line keyed by `(first, second)`.

- ```rust
  pub fn high(self: &Self, first: usize, second: usize) -> usize { /* ... */ }
  ```
  Last in-core index on the line keyed by `(first, second)`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ActiveRange { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ActiveRange) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `FaceTerms`

A `philen`×6 table of per-face values, one row per state-vector index.

Three MATLAB arrays share this shape and column order and so share this
type: `gradterms` (finite-difference coupling coefficients from
`makegradDxyz.m`, dimensionless once divided by a node width),
`nodalterms` (the nodal correction coefficients from `calc_sanodalxyz.m`,
same units), and `geometry.adf` (assembly discontinuity factors,
dimensionless, default 1).

```rust
pub struct FaceTerms {
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
  pub fn zeros(rows: usize) -> Self { /* ... */ }
  ```
  A table of `rows` rows, every entry zero — MATLAB `zeros(philen,6)`.

- ```rust
  pub fn ones(rows: usize) -> Self { /* ... */ }
  ```
  A table of `rows` rows, every entry one — MATLAB `ones(philen,6)`, the

- ```rust
  pub const fn rows(self: &Self) -> usize { /* ... */ }
  ```
  Number of rows (state-vector length).

- ```rust
  pub fn get(self: &Self, idx: usize, face: Face) -> f64 { /* ... */ }
  ```
  Value at state index `idx` on `face`.

- ```rust
  pub fn set(self: &mut Self, idx: usize, face: Face, value: f64) { /* ... */ }
  ```
  Overwrites the value at state index `idx` on `face`.

- ```rust
  pub fn scale(self: &mut Self, factor: f64) { /* ... */ }
  ```
  Multiplies every entry by `factor` — MATLAB `gradterms=2*gradterms`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FaceTerms { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FaceTerms) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `DirectionVectors`

A quantity carried once per coordinate direction, each a full state vector.

The MATLAB writes these as structs with `.x`, `.y` and `.z` fields —
`Leakage`, `Ssource`, `A2`, `A4`, `diffvec`, `bdummy` and friends. The
physical quantity depends on which one: transverse leakages are in
neutrons cm⁻³ s⁻¹, the expansion coefficients `A2`/`A4` are in the same
units as the flux, and `diffvec` is in cm⁻¹.

```rust
pub struct DirectionVectors {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Vec<f64>` | x-direction values, one per state index. |
| `y` | `Vec<f64>` | y-direction values, one per state index. |
| `z` | `Vec<f64>` | z-direction values, one per state index. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(n: usize) -> Self { /* ... */ }
  ```
  Three zero vectors of length `n`.

- ```rust
  pub fn axis(self: &Self, axis: Axis) -> &[f64] { /* ... */ }
  ```
  The component along `axis`.

- ```rust
  pub fn axis_mut(self: &mut Self, axis: Axis) -> &mut Vec<f64> { /* ... */ }
  ```
  The component along `axis`, mutably.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DirectionVectors { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DirectionVectors) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `NodalParams`

Solver-shape parameters — the fields of Yan Ren's `params` struct that the
nodal path reads.

The node counts and group count live in [`Grid`]; this carries what is left.

```rust
pub struct NodalParams {
    pub grid: crate::reference::grid::Grid,
    pub n_precursor_groups: usize,
    pub inner_tolerance: f64,
    pub nodal_update_interval: usize,
    pub fission_extrapolation_interval: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `crate::reference::grid::Grid` | The node grid and energy-group count. MATLAB `params.maxi{x,y,z}`,<br>`params.G`. |
| `n_precursor_groups` | `usize` | Number of delayed-neutron precursor groups appended to the state vector.<br>MATLAB `params.Nc`, defaulting to 0 when the field is absent.<br><br># Unfinished in the reference<br><br>`Nc > 0` does not work in the MATLAB. `makegradDxyz.m` and<br>`makesigmadfxyz.m` build `(G+Nc)*nodes`-square operators, but<br>`calc_sanodalxyz.m` returns a `G*nodes`-square one and<br>`calc_transleakagexyz.m` multiplies a `G*nodes`-wide operator by the<br>`(G+Nc)*nodes`-long flux — both of which raise a dimension error in<br>MATLAB. Translated as-is: the corresponding Rust operations panic on the<br>same mismatch. Only `Nc == 0` is reachable. |
| `inner_tolerance` | `f64` | Source-iteration convergence tolerance on both the fission-source<br>residual and the `k_eff` residual. MATLAB `diffusion.tol`, overridden by<br>`params.innertol` when that is set and positive. |
| `nodal_update_interval` | `usize` | Source iterations between rebuilds of the nodal correction matrix.<br>MATLAB `nodalupd`, default `ceil((maxix+maxiy+maxiz)/10)`, overridden by<br>a nonzero `params.nodalupd`.<br><br># An interval of 1 destabilises the iteration<br><br>The MATLAB comment reads: "Smaller values reduce the lag between the<br>flux shape and the nodal correction matrix, improving stability at the<br>cost of extra factorisations." **In this port the opposite is observed<br>at an interval of exactly 1**, where the correction is rebuilt from the<br>flux that was just computed from it. On a homogeneous leaking cube<br>(20 cm nodes, one group, `k_inf = 1`) the source iteration then fails to<br>settle and hits the 5000-iteration ceiling, while an interval of 2 or<br>more converges to within 1×10⁻³ of the finite-difference answer at every<br>mesh size tried (3³ to 11³). See the tests in<br>[`super::sanm_solver`].<br><br>This matters because the default `ceil((nx+ny+nz)/10)` **is** 1 for any<br>mesh with `nx+ny+nz <= 10`. Every benchmark in the snapshot is far<br>larger (IAEA-3D gives 6), so the reference never hits it in anger.<br><br>Recorded, not repaired, per the translation rules. It has **not** been<br>confirmed that the MATLAB behaves the same way — the reference has not<br>been run (`docs/bedok-port-scoping.md` §4), so this is a property of the<br>port, and whether it is also a property of the original is open. |
| `fission_extrapolation_interval` | `usize` | Source iterations between fission-source extrapolations. MATLAB `fsexp`,<br>default 5, overridden by a nonzero `params.fsexp`. |

##### Implementations

###### Methods

- ```rust
  pub fn with_matlab_defaults(grid: Grid) -> Self { /* ... */ }
  ```
  Parameters with the MATLAB defaults for a given grid.

- ```rust
  pub const fn philen(self: &Self) -> usize { /* ... */ }
  ```
  Length of a neutronics state vector, `G*nodes`. MATLAB `philen`.

- ```rust
  pub const fn philenf(self: &Self) -> usize { /* ... */ }
  ```
  Length of the full state vector including precursors,

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodalParams { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodalParams) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `NodalGeometry`

Node dimensions, boundary conditions and the in-core index ranges the SANM
path needs — Yan Ren's `geometry` struct, minus the fields only the
thermal-hydraulic side reads.

`lx`, `ly`, `lz` and `volume` are indexed by **spatial** node index
(`ix*ny*nz + iy*nz + iz`), matching the MATLAB's `geometry.Lx` etc. before
the `repmat(...,G,1)` lift to full state length.

```rust
pub struct NodalGeometry {
    pub lx: Vec<f64>,
    pub ly: Vec<f64>,
    pub lz: Vec<f64>,
    pub volume: Vec<f64>,
    pub which_sigma: Vec<usize>,
    pub boundaries: BoundaryConditions,
    pub x_range: ActiveRange,
    pub y_range: ActiveRange,
    pub z_range: ActiveRange,
    pub adf: FaceTerms,
    pub nodal_coefficients: NodalCoefficients,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lx` | `Vec<f64>` | Node width in x \[cm\], one per spatial node. MATLAB `geometry.Lx`. |
| `ly` | `Vec<f64>` | Node width in y \[cm\], one per spatial node. MATLAB `geometry.Ly`. |
| `lz` | `Vec<f64>` | Node height in z \[cm\], one per spatial node. MATLAB `geometry.Lz`. |
| `volume` | `Vec<f64>` | Node volume \[cm³\], one per spatial node. MATLAB `geometry.Vi`. |
| `which_sigma` | `Vec<usize>` | Material index per spatial node, **1-based**, `0` meaning "no material<br>here". MATLAB `geometry.whichsigma` / the `whichsigma` argument. |
| `boundaries` | `BoundaryConditions` | Outer-boundary conditions. |
| `x_range` | `ActiveRange` | In-core `ix` range for each `(iy, iz)` line. MATLAB<br>`geometry.xlows`/`xhis`. |
| `y_range` | `ActiveRange` | In-core `iy` range for each `(ix, iz)` line. MATLAB<br>`geometry.ylows`/`yhis`. |
| `z_range` | `ActiveRange` | In-core `iz` range for each `(ix, iy)` line. MATLAB<br>`geometry.zlows`/`zhis`. |
| `adf` | `FaceTerms` | Assembly discontinuity factors, `philen`×6, dimensionless. MATLAB<br>`geometry.adf`; unity everywhere when the field is absent. |
| `nodal_coefficients` | `NodalCoefficients` | Semi-analytic expansion coefficients, filled by<br>[`super::nodal_coefficients::assemble`]. MATLAB `geometry.nodalcoeffs`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(grid: Grid, lx: Vec<f64>, ly: Vec<f64>, lz: Vec<f64>, which_sigma: Vec<usize>, boundaries: BoundaryConditions) -> Self { /* ... */ }
  ```
  A uniform-mesh geometry with whole-axis in-core ranges, unity ADFs and

- ```rust
  pub const fn range(self: &Self, axis: Axis) -> &ActiveRange { /* ... */ }
  ```
  The in-core index range along `axis`.

- ```rust
  pub fn width(self: &Self, axis: Axis, node: usize) -> f64 { /* ... */ }
  ```
  Node width along `axis` at spatial node `node` \[cm\].

- ```rust
  pub fn width_state_vector(self: &Self, axis: Axis, grid: Grid) -> Vec<f64> { /* ... */ }
  ```
  Node widths along `axis` lifted to full state length by repeating the

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodalGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodalGeometry) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `NodalCoefficients`

The `A`, `B`, `E`, `F`, `G`, `H` coefficients of the semi-analytic nodal
expansion, one full state vector per coefficient per direction.

All six are dimensionless functions of the node's optical half-width
`alpha = 0.5*L*sqrt(sigma_r/D)`; see [`super::nodal_coefficients`] for the
formulas and for what happens as `alpha -> 0`. MATLAB
`geometry.nodalcoeffs.{Aa,Bb,Ee,Ff,Gg,Hh}`.

```rust
pub struct NodalCoefficients {
    pub aa: DirectionVectors,
    pub bb: DirectionVectors,
    pub ee: DirectionVectors,
    pub ff: DirectionVectors,
    pub gg: DirectionVectors,
    pub hh: DirectionVectors,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `aa` | `DirectionVectors` | `Aa` — multiplies the first leakage moment in the odd expansion. |
| `bb` | `DirectionVectors` | `Bb` — multiplies the second leakage moment in the even expansion. |
| `ee` | `DirectionVectors` | `Ee` — the even-mode buckling weight. |
| `ff` | `DirectionVectors` | `Ff` — the odd-mode current weight. |
| `gg` | `DirectionVectors` | `Gg` — the fourth-order surface-flux weight. |
| `hh` | `DirectionVectors` | `Hh` — the third-order surface-current weight. |

##### Implementations

###### Methods

- ```rust
  pub fn zeros(n: usize) -> Self { /* ... */ }
  ```
  All six coefficients zeroed, for a state vector of length `n`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodalCoefficients { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodalCoefficients) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `gradient_diffusion`

The finite-difference leakage operator and its per-face coupling terms.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `makegradDxyz.m` (function `makegradDxyz`).

```rust
pub mod gradient_diffusion { /* ... */ }
```

### Types

#### Struct `GradientDiffusion`

The finite-difference leakage operator and the face-coupling terms the
nodal correction is built on top of.

```rust
pub struct GradientDiffusion {
    pub operator: super::sparse::SparseMatrix,
    pub face_terms: super::geometry::FaceTerms,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `operator` | `super::sparse::SparseMatrix` | `gradD` — the assembled leakage operator \[cm⁻¹\], `philenf` square.<br><br>Its diagonal starts as the identity and is overwritten in-core, so rows<br>for nodes outside the core keep a `1` and the operator stays<br>nonsingular there. |
| `face_terms` | `super::geometry::FaceTerms` | `gradterms` — the six per-face coupled diffusion coefficients<br>`2*Dtilde` \[cm\] used by the transverse-leakage and nodal-correction<br>stages.<br><br>The MATLAB doubles the whole table on the last line<br>(`gradterms=2*gradterms; %check this (seems correct)`); that comment is<br>the author's, and the doubling is reproduced without judgement. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GradientDiffusion { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GradientDiffusion) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `assemble`

**Attributes:**

- `MustUse { reason: None }`

Builds the finite-difference leakage operator — `makegradDxyz.m`.

For each interior face the coupled diffusion coefficient is

```text
Dtilde+ = 0.5*(h + h+) * D*D+ / (h*D + h+*D+) / L
```

with `h = L/2` the node half-width \[cm\], `D` the diffusion coefficient
\[cm\], and the result contributing `Dtilde+/h+` to the diagonal and
`-Dtilde+/h+` to the off-diagonal. Outer faces substitute a
boundary-condition-dependent `Dtilde`:

| Condition | Outer `Dtilde` |
|---|---|
| `Vacuum` | `0.5*D / (2*D + 0.5*L)` |
| `Reflective` | `0` |
| `ZeroFlux` | `D / L` |

Directions are swept z, then y, then x. **The z sweep assigns the diagonal;
the y and x sweeps accumulate onto it.** That asymmetry is in the original
and is preserved — it is why z must be swept first.

`diffusion` is the flat diffusion-coefficient state vector from
[`super::cross_sections::diffusion_coefficients`] \[cm\]; `which_sigma` is
the 1-based material index per spatial node, `0` skipping the node.

# Not ported

The `tomode ~= 1` branch calls `convertsparseformat2d`, which converts to
the 2-D half-index layout. No SANM call site uses it, so only `tomode = 1`
is translated.

# Panics

If a boundary node sits at the very edge of the grid so that its `+1`
neighbour does not exist — the MATLAB indexes out of bounds and errors in
exactly the same situation.

```rust
pub fn assemble(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, diffusion: &[f64], which_sigma: &[usize]) -> GradientDiffusion { /* ... */ }
```

#### Function `neighbour_stride`

**Attributes:**

- `MustUse { reason: None }`

The neighbour stride along `axis` in the flat state vector.

`+1` in z, `+nz` in y, `+ny*nz` in x — the MATLAB's `1`, `maxiz` and
`xstep`.

```rust
pub const fn neighbour_stride(axis: super::geometry::Axis, grid: crate::reference::grid::Grid) -> usize { /* ... */ }
```

## Module `leakage_moments`

The first and second transverse-leakage moments.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Sources: `calc_1sttransleakagexyz.m` (function `calc_1sttransleakagexyz`)
and `calc_2ndtransleakagexyz.m` (function `calc_2ndtransleakagexyz`).

Both files implement the classic quadratic transverse-leakage fit: the
transverse leakage seen by the one-dimensional solution along one axis is
approximated by the parabola through the node-average leakages of the node
and its two neighbours along that axis, and these two routines return the
parabola's first and second moments.

```rust
pub mod leakage_moments { /* ... */ }
```

### Types

#### Enum `Moment`

Which moment of the transverse-leakage parabola to evaluate.

The two share their loop structure, their node-width ratios and their
scaling; they differ only in the interior numerator and in what happens on
a non-reflective outer face.

```rust
pub enum Moment {
    First,
    Second,
}
```

##### Variants

###### `First`

The first (odd) moment — `calc_1sttransleakagexyz.m`.

###### `Second`

The second (even) moment — `calc_2ndtransleakagexyz.m`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Moment { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Moment) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `first_moment`

**Attributes:**

- `MustUse { reason: None }`

The first transverse-leakage moment — `calc_1sttransleakagexyz.m`.

See [`moment`] for the formulas. Result units follow the input: with a
zeroth-moment leakage in neutrons cm⁻³ s⁻¹ and `D` in cm, the moment is in
neutrons cm⁻² s⁻¹, the same units as the flux.

```rust
pub fn first_moment(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, zeroth: &super::geometry::DirectionVectors, diffusion: &[f64]) -> super::geometry::DirectionVectors { /* ... */ }
```

#### Function `second_moment`

**Attributes:**

- `MustUse { reason: None }`

The second transverse-leakage moment — `calc_2ndtransleakagexyz.m`.

See [`moment`] for the formulas and units.

```rust
pub fn second_moment(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, zeroth: &super::geometry::DirectionVectors, diffusion: &[f64]) -> super::geometry::DirectionVectors { /* ... */ }
```

#### Function `moment`

**Attributes:**

- `MustUse { reason: None }`

The shared body of the two moment routines.

For direction `d` the transverse source is the sum of the *other* two
directions' zeroth leakage moments: `S_x = L_y + L_z`, `S_y = L_x + L_z`,
`S_z = L_x + L_y`.

With `t+ = L(i+1)/L(i)` and `t- = L(i-1)/L(i)` the neighbour width ratios
\[dimensionless\] and `h = 2*(t+ + 1)*(t- + 1)*(t- + t+ + 1)`, the interior
value is

```text
First:  [ (t-+1)(2t-+1)(S(i+1) - S(i)) + (t++1)(2t++1)(S(i) - S(i-1)) ] / h
Second: [ (t-+1)        (S(i+1) - S(i)) + (t++1)        (S(i-1) - S(i)) ] / h
```

scaled in both cases by `0.25 * L(i)^2 / D(i)` \[cm²/cm = cm\].

On an outer face, with `h = 4*(t+1)*(t+2)`:

| | `Vacuum` / `ZeroFlux` | `Reflective` |
|---|---|---|
| First, low face | `(S(i+1) - S(i)) / (t+ + 1)` | `6*(S(i+1) - S(i)) / h` |
| First, high face | `(S(i) - S(i-1)) / (t- + 1)` | `6*(S(i) - S(i-1)) / h` |
| Second, low face | left at zero | `2*(S(i+1) - S(i)) / h` |
| Second, high face | left at zero | `2*(S(i-1) - S(i)) / h` |

Nodes with a zero diffusion coefficient are skipped and stay zero.

# Recorded asymmetry

The second moment's interior numerator uses `S(i-1) - S(i)` where the first
uses `S(i) - S(i-1)`, and its high-face reflective term likewise flips.
That is a genuine even/odd distinction, not a sign slip, and is reproduced
verbatim.

# Panics

If `diffusion.len()` differs from the neutronics state length, or if a
line's `low` and `high` coincide, which makes the boundary blocks index a
neighbour outside the grid. The MATLAB fails in the same place.

```rust
pub fn moment(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, zeroth: &super::geometry::DirectionVectors, diffusion: &[f64], which: Moment) -> super::geometry::DirectionVectors { /* ... */ }
```

## Module `nodal_coefficients`

The `A`, `B`, `E`, `F`, `G`, `H` coefficients of the semi-analytic
expansion.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `calc_ABEFGHxyz.m` (functions `calc_ABEFGHxyz` and `abefgh`).

```rust
pub mod nodal_coefficients { /* ... */ }
```

### Functions

#### Function `expansion_coefficients`

**Attributes:**

- `MustUse { reason: None }`

The six coefficients as a function of the node's optical half-width.

`alpha = 0.5 * L * sqrt(sigma_r / D)` \[dimensionless\], with `L` the node
width \[cm\], `sigma_r` the removal cross section \[cm⁻¹\] and `D` the
diffusion coefficient \[cm\]. Returned in the order `(A, B, E, F, G, H)`,
all dimensionless.

The expressions, verbatim from the MATLAB's inner `abefgh` function:

```text
ms = 3*(cosh(a)/a - sinh(a)/a^2)
mc = 5*(sinh(a)/a - 3*cosh(a)/a^2 + 3*sinh(a)/a^3)
A  = (sinh(a) - ms) / (a^2 * ms)
B  = (cosh(a) - sinh(a)/a - mc) / (a^2 * mc)
E  = sinh(a)/(a*mc) - 3/a^2
F  = (a*cosh(a) - ms) / (a^2 * ms)
G  = (a*sinh(a) - 3*mc) / (cosh(a) - sinh(a)/a - mc)
H  = (a*cosh(a) - ms) / (sinh(a) - ms)
```

# Valid range

`alpha` must be strictly positive and not so small that the `sinh`/`cosh`
cancellations lose all significance. Every expression is a ratio of
differences that individually vanish as `alpha -> 0`, so the accuracy
degrades long before `alpha` reaches zero, and at exactly `alpha = 0` the
result is `NaN`. **The reference has no small-`alpha` series expansion and
no guard.** Recorded, not fixed: adding one would change results everywhere
the mesh is optically thin.

```rust
pub fn expansion_coefficients(alpha: f64) -> (f64, f64, f64, f64, f64, f64) { /* ... */ }
```

#### Function `assemble`

**Attributes:**

- `MustUse { reason: None }`

Builds `geometry.nodalcoeffs` — `calc_ABEFGHxyz.m`.

For each in-core node and group, forms `r = sqrt(sigma_r / D)` \[cm⁻¹\] from
the removal cross section on the diagonal of `sigma.tot - sigma.s`, and
evaluates [`expansion_coefficients`] at `0.5 * r * L` once per direction.
Nodes with `D == 0` — i.e. outside the core — keep all six coefficients at
zero.

# Divergence from MATLAB on a negative removal cross section

MATLAB's `sqrt` of a negative number returns a complex value, and the whole
coefficient set (and every matrix built from it downstream) silently becomes
complex. Rust's `f64::sqrt` returns `NaN` instead, which propagates and
eventually trips the solver's non-finite guard. This is a genuine
behavioural difference, recorded here rather than papered over; it can only
be reached with a data set whose within-group scattering exceeds its total
cross section, which is unphysical.

# Panics

If `diffusion.len()` differs from the neutronics state length.

```rust
pub fn assemble(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, sigma_total: &super::sparse::SparseMatrix, sigma_scatter: &super::sparse::SparseMatrix, diffusion: &[f64]) -> super::geometry::NodalCoefficients { /* ... */ }
```

## Module `nodal_correction`

The semi-analytic nodal correction operator.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `calc_sanodalxyz.m` (function `calc_sanodalxyz`).

This is the heart of the method. The semi-analytic expansion gives a surface
current on each face; dividing it by the flux turns it into a *correction*
to the finite-difference coupling coefficient, which is then assembled into
an operator added alongside `gradD`. The result is a coarse-mesh
finite-difference scheme whose coefficients reproduce the analytic
one-dimensional solution — the standard nodal-equivalence trick.

```rust
pub mod nodal_correction { /* ... */ }
```

### Types

#### Struct `NodalCorrection`

The nodal correction operator and the face terms it was built from.

```rust
pub struct NodalCorrection {
    pub operator: super::sparse::SparseMatrix,
    pub face_terms: super::geometry::FaceTerms,
    pub expansion: super::expansion::Expansion,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `operator` | `super::sparse::SparseMatrix` | The correction operator \[cm⁻¹\], `philen` square, added to `gradD` in<br>the source iteration. MATLAB `nodal`.<br><br># Recorded dimension mismatch<br><br>This is `philen` square while `gradD` and the cross-section operators<br>are `philenf` square. With `Nc == 0` those coincide; with `Nc > 0` the<br>MATLAB's `gradD+nodal+sigma.tot-sigma.s` raises a dimension error, and<br>so does this port. See<br>[`NodalParams::n_precursor_groups`](super::geometry::NodalParams::n_precursor_groups). |
| `face_terms` | `super::geometry::FaceTerms` | The per-face nodal correction coefficients \[cm\]. MATLAB<br>`nodalterms`; fed back into the next iteration's transverse-leakage<br>calculation. |
| `expansion` | `super::expansion::Expansion` | The `A1`–`A4` expansion this correction was derived from, kept because<br>the caller occasionally wants to inspect it. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodalCorrection { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodalCorrection) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `assemble`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Builds the nodal correction — `calc_sanodalxyz.m`.

For each face the surface current implied by the expansion is

```text
J+ = (2D/L) * ( A1 + 3*A2 + H*A3 + G*A4 )        (high face)
J- = (2D/L) * ( A1first - 3*A2 + H*A3first - G*A4 )  (low outer face)
```

and the correction coefficient on an interior face is

```text
n+ = ( g+ * (phi - phi+) + J+ ) / (phi + phi+)
```

with `g+` the finite-difference face term. On an outer face the flux of the
single adjacent node is used instead of the sum. The correction of a face is
then shared with the neighbour: `n-(i+1) = n+(i)`.

The operator row for node `i` is

```text
(i, i)        += ( n- - n+ ) / L(i)
(i, i+stride)  = -n+ / L(i+stride)
(i, i-stride)  =  n- / L(i-stride)
```

summed over the three directions, with z sweeping first and y and x
accumulating onto the diagonal it created.

# The near-zero flux guard, and what it hides

Dividing by the flux is only meaningful where the flux is not near zero, so
the reference skips the update when `|phi| <= 1e-8 * max|phi|` (or when the
two-node sum is), **leaving the correction at zero** — that is, falling back
to plain finite difference at that face. The guard is Yan Ren's own, added
with the comment "the nodal expansion is ill-conditioned (near-zero or
sign-cancelling flux)". Two consequences worth stating plainly:

- The threshold is relative to the **global** flux maximum, so in a case
  with a strong axial or radial gradient a whole region can silently drop to
  finite difference.
- The skipped entry keeps whatever it held from a previous sweep — for the
  shared `n-(i+1) = n+(i)` assignment, which is *not* inside the guard, the
  neighbour still receives the (possibly stale) value.

Left exactly as found.

# Panics

If the y or x sweep reaches a node the z sweep did not create a diagonal
entry for. That happens when the per-line active ranges disagree about which
nodes are in-core; the MATLAB indexes `nodalele(0)` and errors in the same
situation.

```rust
pub fn assemble(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, flux: &[f64], sigma: &super::cross_sections::CrossSectionOperators, diffusion: &[f64], grad_terms: &super::geometry::FaceTerms, previous_nodal_terms: &super::geometry::FaceTerms, k_eff: f64) -> NodalCorrection { /* ... */ }
```

## Module `sanm_solver`

The semi-analytic nodal `k`-eigenvalue solver.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `sanodaldiffusion_solverxyz.m` (function
`sanodaldiffusion_solverxyz`).

```rust
pub mod sanm_solver { /* ... */ }
```

### Types

#### Enum `Termination`

Why the source iteration stopped.

```rust
pub enum Termination {
    Converged,
    Stopped,
}
```

##### Variants

###### `Converged`

Both the fission-source residual and the `k_eff` residual fell below the
tolerance.

###### `Stopped`

`k_eff` went non-positive or `NaN`, or the iteration ceiling was
reached. The MATLAB prints "Source interation stopped, not converging",
dumps the flux to `scalar_fluxerr.csv`, and returns the values it has —
it does **not** raise an error, and neither does this port.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Termination { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Termination) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `DiffusionSolution`

The result of a source iteration.

```rust
pub struct DiffusionSolution {
    pub k_eff: f64,
    pub fission_source_residual: f64,
    pub k_eff_residual: f64,
    pub scalar_flux: super::flux_history::FluxHistory,
    pub fission_source: Vec<f64>,
    pub power_density: Vec<f64>,
    pub iterations: usize,
    pub termination: Termination,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | Converged multiplication factor \[dimensionless\]. MATLAB<br>`output.k_eff`. |
| `fission_source_residual` | `f64` | Final relative fission-source residual \[dimensionless\]. MATLAB<br>`output.residual`. |
| `k_eff_residual` | `f64` | Final relative `k_eff` residual \[dimensionless\]. MATLAB<br>`output.k_eff_residual`. |
| `scalar_flux` | `super::flux_history::FluxHistory` | The flux history, renormalised so the fission-source integral matches<br>its initial value. MATLAB `output.scalar_flux`. |
| `fission_source` | `Vec<f64>` | Fission source per unit volume \[neutrons cm⁻³ s⁻¹\]. MATLAB<br>`output.fission_source`. |
| `power_density` | `Vec<f64>` | Node power density, `fission_source * node volume`<br>\[neutrons s⁻¹ per node\]. MATLAB `output.pwrdens`. |
| `iterations` | `usize` | Source iterations performed. MATLAB's `iteration-1` in the printout. |
| `termination` | `Termination` | Why the loop ended. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DiffusionSolution { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DiffusionSolution) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `solve`

**Attributes:**

- `Other("#[allow(clippy::manual_is_multiple_of)]")`
- `MustUse { reason: None }`

Solves the `k`-eigenvalue problem with the semi-analytic nodal method —
`sanodaldiffusion_solverxyz.m`.

# What it does

A power (source) iteration on

```text
(gradD + nodal + sigma_tot - sigma_s) phi = (1/k) F phi
```

solved each pass with a **direct** sparse LU factorisation, refactorising
only when the nodal correction is rebuilt. `k_eff` is updated by the ratio
of successive fission-source 1-norms, and every
[`NodalParams::fission_extrapolation_interval`] iterations the flux and
fission source are extrapolated (see [`super::fission_source`]).

Convergence requires **both** the relative fission-source residual and the
relative `k_eff` residual to fall below
[`NodalParams::inner_tolerance`].

# Stability caveat

A [`NodalParams::nodal_update_interval`] of 1 — the MATLAB default whenever
`nx+ny+nz <= 10` — makes the iteration unstable in this port. Read that
field's documentation before using a small mesh.

# Arguments

- `geometry` — node dimensions, boundary conditions and discontinuity
  factors. Its `nodal_coefficients` field is (re)computed here, exactly as
  the MATLAB assigns `geometry.nodalcoeffs` inside the solver.
- `values` — per-material multigroup cross sections \[cm⁻¹\].
- `which_sigma` — 1-based material index per spatial node, `0` for none.
- `initial_k_eff` — starting eigenvalue \[dimensionless\]; the MATLAB
  default is 1.
- `initial_flux` — optional warm start. If its depth matches the history
  depth the columns are taken as-is; otherwise the current column is
  broadcast, mirroring the reference's two-branch warm start.

# Normalisation

On exit the flux and fission source are scaled so that the **sum** (not the
1-norm) of the fission source equals its value on the first iterate. The
MATLAB comment reads "CURRENT NORMALIZATION: fission source intergration =
1", which is not what the code does — it preserves the initial integral,
whatever that was. Recorded, not changed.

# Not ported

- The `params.debugdump` CSV writes and the `params.plotfig` surface plot.
  The MATLAB itself calls these "pure side effects [that] do not affect the
  solver's return values".
- The `philenf >= sizethresh` branch, which switches to preconditioned
  GMRES. `sizethresh` is 5×10⁷, far above any state vector the benchmarks
  produce, so it is dead in every case in the snapshot — and substituting an
  iterative solver would be a stage-2 change (`docs/bedok-port-scoping.md`
  §5), not a translation.
- The commented-out Wielandt shift. `weilandtfactor` is set and never used;
  the author's note says it "does not seem to work".

# Panics

If the LHS cannot be factorised, or if `Nc > 0` makes the operator shapes
disagree (see
[`NodalParams::n_precursor_groups`](super::geometry::NodalParams::n_precursor_groups)).

```rust
pub fn solve(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, values: &super::cross_sections::MaterialCrossSections, which_sigma: &[usize], initial_k_eff: f64, initial_flux: Option<&super::flux_history::FluxHistory>) -> DiffusionSolution { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_ITERATIONS`

Hard iteration ceiling — MATLAB `maxiter=5000`.

```rust
pub const MAX_ITERATIONS: usize = 5000;
```

## Module `sparse`

Sparse and small-dense linear algebra for the reference nodal solvers.

# Provenance

Original author of the BEDOK MATLAB implementation this crate translates:
**Than Yan Ren**, Singapore Nuclear Research and Safety Institute (SNRSI).
Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`. Translated under the
permission recorded in `docs/bedok-port-scoping.md` §6.

This particular file is **not** a translation of one `.m` file. It supplies
the MATLAB built-ins the ported nodal files lean on, in the order they are
used across `makegradDxyz.m`, `makesigmadfxyz.m`, `calc_bucklingxyz.m`,
`calc_transleakagexyz.m`, `calc_a1_expansionxyz.m`,
`calc_a1234_expansionxyz.m`, `calc_sanodalxyz.m`,
`sanodaldiffusion_solverxyz.m` and `diffusion_solverxyz.m`:

| MATLAB | Here |
|---|---|
| `sparse(i,j,v,m,n)` | [`SparseMatrix::from_triplets`] |
| `A*x` | [`SparseMatrix::mul_vec`] |
| `A+B`, `A-B` | [`SparseMatrix::add`], [`SparseMatrix::sub`] |
| `spdiags(d,0,n,n)*A` | [`SparseMatrix::scale_rows`] |
| `speye(n)` | [`SparseMatrix::identity`] |
| `A(i,j)` on a sparse `A` | [`SparseMatrix::get`] |
| `decomposition(A)` then `dA\b` | [`SparseLu`] (faer's direct sparse LU) |
| `A\b` on a small dense `A`, `pagemldivide` | [`solve_dense_in_place`] |

# Deviations from MATLAB, recorded rather than hidden

- **Explicit zeros are kept.** MATLAB's `sparse()` discards numerically zero
  entries; [`SparseMatrix::from_triplets`] stores them. No computed value
  changes, but the stored sparsity pattern is a superset of MATLAB's, which
  can shift the pivots the LU picks and hence the last few digits of a
  solve. Recorded, not repaired: dropping zeros would make some assembled
  operators structurally singular that MATLAB happens to survive.
- **The direct solver is faer's sparse LU with partial pivoting**, not
  UMFPACK. Same algorithm class, different pivot order, so results agree to
  solver tolerance rather than bit for bit.

Nothing here is verified against Yan Ren's implementation — see the crate
docs on verification status.

```rust
pub mod sparse { /* ... */ }
```

### Types

#### Struct `SparseMatrix`

A real sparse matrix in compressed-sparse-column form.

Holds no physical quantity of its own — it is whatever operator the caller
assembled (a leakage operator in cm⁻¹, a cross-section operator in cm⁻¹, a
dimensionless buckling operator). Sizes are node/group state-vector lengths,
so a square operator is `state_len` × `state_len` in the sense of
[`Grid::state_len`](crate::reference::grid::Grid::state_len).

Row indices within a column are stored ascending, and duplicate
`(row, col)` triplets are summed at construction, matching MATLAB's
`sparse(i,j,v,m,n)`.

```rust
pub struct SparseMatrix {
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
  pub fn from_triplets(nrows: usize, ncols: usize, triplets: &[(usize, usize, f64)]) -> Self { /* ... */ }
  ```
  Assembles an `nrows` × `ncols` matrix from `(row, col, value)` triplets,

- ```rust
  pub fn identity(n: usize, factor: f64) -> Self { /* ... */ }
  ```
  The `n` × `n` identity scaled by `factor` — MATLAB `factor*speye(n)`.

- ```rust
  pub const fn nrows(self: &Self) -> usize { /* ... */ }
  ```
  Number of rows.

- ```rust
  pub const fn ncols(self: &Self) -> usize { /* ... */ }
  ```
  Number of columns.

- ```rust
  pub fn stored_entries(self: &Self) -> usize { /* ... */ }
  ```
  Number of stored entries, including any explicit zeros.

- ```rust
  pub fn get(self: &Self, row: usize, col: usize) -> f64 { /* ... */ }
  ```
  Entry `(row, col)`, or `0.0` if not stored — MATLAB `A(row,col)`.

- ```rust
  pub fn diagonal(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The main diagonal as a dense vector — MATLAB `full(diag(A))`.

- ```rust
  pub fn mul_vec(self: &Self, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  `A * x` — MATLAB `A*x`.

- ```rust
  pub fn add(self: &Self, other: &Self) -> Self { /* ... */ }
  ```
  `A + B` — MATLAB `A+B`.

- ```rust
  pub fn sub(self: &Self, other: &Self) -> Self { /* ... */ }
  ```
  `A - B` — MATLAB `A-B`.

- ```rust
  pub fn scale_rows(self: &Self, d: &[f64]) -> Self { /* ... */ }
  ```
  Scales row `i` by `d[i]` — MATLAB `spdiags(d,0,n,n)*A`.

- ```rust
  pub fn lu(self: &Self) -> Option<SparseLu> { /* ... */ }
  ```
  Factorises with a **direct** sparse LU — MATLAB `decomposition(A)`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SparseMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SparseMatrix) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `SparseLu`

A cached direct sparse LU factorisation — MATLAB's `decomposition` object.

Holds no physical quantity; it solves `A x = b` for whatever operator `A`
was factorised.

```rust
pub struct SparseLu {
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
  pub fn solve(self: &Self, rhs: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Solves `A x = b` — MATLAB `dA\b`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut core::fmt::Formatter<''_>) -> core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `solve_dense_in_place`

Solves the small dense system `a * x = b` in place — MATLAB `A\b` for a
square dense `A`, and one page of `pagemldivide`.

`a` is `n` × `n` in **row-major** order; `b` is the length-`n` right-hand
side and receives the solution.

Gaussian elimination with partial pivoting, the same algorithm MATLAB's
`mldivide` selects for a general square dense matrix.

# Singular systems

A zero pivot is **not** treated as an error: the elimination proceeds and
produces `Inf`/`NaN`, which is what MATLAB does (with a warning) and what
the reference relies on propagating. `calc_a1_expansionxyz.m` reaches this
case for a reflective outer face over a node with zero diffusion
coefficient — see [`super::first_moment`].

# Panics

If `a.len() != n*n` or `b.len() != n`.

```rust
pub fn solve_dense_in_place(a: &mut [f64], n: usize, b: &mut [f64]) { /* ... */ }
```

## Module `transverse_leakage`

The zeroth transverse-leakage moment.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.

Source: `calc_transleakagexyz.m` (function `calc_transleakagexyz`).

```rust
pub mod transverse_leakage { /* ... */ }
```

### Functions

#### Function `zeroth_moment`

**Attributes:**

- `MustUse { reason: None }`

Node-average leakage out of each face pair — `calc_transleakagexyz.m`.

Builds a three-point leakage operator per direction and applies it to the
flux, returning `L_x*phi`, `L_y*phi` and `L_z*phi` in neutrons cm⁻³ s⁻¹ if
the flux is in neutrons cm⁻² s⁻¹.

The interior row for state index `i` is

```text
 diag  = ( g- + g+ + n- - n+ ) / L(i)
 plus  = -( g+ + n+ ) / L(i + stride)
 minus = -( g- - n- ) / L(i - stride)
```

with `g±` the finite-difference face terms from
[`super::gradient_diffusion`] and `n±` the nodal corrections from
[`super::nodal_correction`], both \[cm\]. Note that the neighbour
coefficients are divided by the **neighbour's** width, not the node's — that
asymmetry is in the reference and is preserved.

On an outer face the diagonal keeps the full four-term form for `Vacuum` and
`ZeroFlux` but drops to a single face for `Reflective`, while the
neighbour coefficient is unchanged. Nodes whose group-1 diffusion
coefficient is zero are skipped, contributing nothing.

# Panics

If the flux length does not match the operator width. That mismatch is
reachable in the reference only through `Nc > 0`, which does not work there
either — see [`NodalParams::n_precursor_groups`].

```rust
pub fn zeroth_moment(params: &super::geometry::NodalParams, geometry: &super::geometry::NodalGeometry, flux: &[f64], diffusion: &[f64], grad_terms: &super::geometry::FaceTerms, nodal_terms: &super::geometry::FaceTerms) -> super::geometry::DirectionVectors { /* ... */ }
```

### Re-exports

#### Re-export `FiniteDifferenceSolution`

```rust
pub use finite_difference_solver::FiniteDifferenceSolution;
```

#### Re-export `FluxHistory`

```rust
pub use flux_history::FluxHistory;
```

#### Re-export `ActiveRange`

```rust
pub use geometry::ActiveRange;
```

#### Re-export `Axis`

```rust
pub use geometry::Axis;
```

#### Re-export `BoundaryCondition`

```rust
pub use geometry::BoundaryCondition;
```

#### Re-export `BoundaryConditions`

```rust
pub use geometry::BoundaryConditions;
```

#### Re-export `Face`

```rust
pub use geometry::Face;
```

#### Re-export `FaceTerms`

```rust
pub use geometry::FaceTerms;
```

#### Re-export `NodalGeometry`

```rust
pub use geometry::NodalGeometry;
```

#### Re-export `NodalParams`

```rust
pub use geometry::NodalParams;
```

#### Re-export `DiffusionSolution`

```rust
pub use sanm_solver::DiffusionSolution;
```

#### Re-export `Termination`

```rust
pub use sanm_solver::Termination;
```

## Module `th`

Thermal hydraulics — faithful translation of Than Yan Ren's MATLAB.

# Provenance

Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
Institute (SNRSI). Translated from the handed-over MATLAB snapshot
`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`, received 2026-08-05.
Permission to translate and to publish as open source under OUTRAM PARK was
given by the author and approved at project-lead level; see
`docs/bedok-port-scoping.md` §6.

# What is in here

| Rust module | MATLAB source |
|---|---|
| [`solver`] | `th_solverxyz.m` |
| [`solver_time`] | `th_solvertimexyz.m` |
| [`single_flow_evap`] | `singleflow1devap.m` |
| [`single_flow_evap_time`] | `singleflow1devaptime.m` |
| [`drift_flux_3d`] | `driftflux6_solverstatic3d.m` |
| [`fuel_rod`] | `fuelrodheat_1dcylnd.m` |
| [`fuel_rod_time`] | `fuelrodheattime_1dcylnd.m` |
| [`w3_chf`] | `w3chf.m`, `w3chfhottest.m` |
| [`steam`] | *substituted* — `tampines-steam-tables`, not `IAPWS_IF97.m` |
| [`linalg`] | *supporting* — replaces MATLAB's sparse `\` |

# Units

The MATLAB mixes unit systems and this translation keeps them **exactly as
they are**, because changing them would change the floating-point
arithmetic. Every public item states its units explicitly. The conventions
throughout are:

- length **cm**, area **cm²**, volume **cm³**
- temperature **K** (never °C)
- pressure **MPa**
- specific enthalpy **kJ/kg** (= J/g)
- density **g/cm³**, mass flux **g/(s·cm²)**, velocity **cm/s**
- power **W**, linear power density **W/cm**, volumetric power **W/cm³**
- heat flux **W/cm²**, heat transfer coefficient **W/(cm²·K)**
- thermal conductivity **W/(cm·K)**, gap conductance **W/(cm²·K)**
- volumetric heat capacity **J/(cm³·K)**, kinematic viscosity **cm²/s**

# Known gaps in the upstream snapshot

Recorded here rather than repaired, per `docs/bedok-port-scoping.md` §1.0.
Each is also flagged at the point in the code where it occurs.

1. **`driftflux6_solverstatic1d.m` is missing from the snapshot.**
   `driftflux6_solverstatic3d.m` calls it at its line 157 and nothing else
   in the snapshot defines it. See [`drift_flux_3d`]. The single-phase
   homogeneous-equilibrium path ([`single_flow_evap`]) is complete and is
   what the benchmark cases actually exercise.
2. **`fuelrodheat_1dcylnd` indexes past its own matrix** for any rod layout
   with no material→gap transition (e.g. an all-fuel rod). See
   [`fuel_rod::solve_static`].
3. **A material→material interface with no gap between them assembles no
   conduction coefficient.** See [`fuel_rod::solve_static`].
4. **The gap ring becomes an orphan row** fixed at `T = 1 K`. See
   [`fuel_rod::solve_static`].
5. **`w3chfhottest.m` sets `highy = ix`** where it means `iy`. See
   [`w3_chf::hottest_channel`].
6. **`w3chf.m`'s `enthshift` is not the inlet enthalpy** the W-3
   correlation calls for, and carries a stray factor of ½. See
   [`w3_chf::critical_heat_flux`].

# Verification status

**Unverified against the reference.** The MATLAB was not run (there is no
MATLAB or Octave on the build machine, and the snapshot ships no golden
outputs), so nothing here may be described as "reproducing Yan Ren's
results". The unit tests in this module check internal consistency and
hand-worked correlation values only.

```rust
pub mod th { /* ... */ }
```

### Modules

## Module `drift_flux_3d`

Multichannel wrapper for the staggered six-equation two-fluid solver.

# Provenance

Translated from `driftflux6_solverstatic3d.m` by **Than Yan Ren**
(Singapore Nuclear Research and Safety Institute), BEDOK snapshot sha256
`e45cd6f57be2087c…`, received 2026-08-05.

# THE SINGLE-CHANNEL SOLVER IS MISSING FROM THE SNAPSHOT

**`driftflux6_solverstatic3d.m` calls `driftflux6_solverstatic1d` at its
line 157. No file in the snapshot defines it.** Yan Ren handed the code
over unfinished and that kernel was never written — there is nothing to
translate, and per `docs/bedok-port-scoping.md` §1.0 nothing is invented to
fill the gap. [`solve_single_channel_static`] is the named hole; it returns
[`ThError::MissingUpstreamSource`] and calling it is the only way to reach
that error.

Everything *around* the missing call is translated faithfully — the inlet
state, the warm-start bookkeeping, the previous-state defaults for
unpowered columns, and the whole derived-field recovery tail — so that when
the kernel is eventually written the wrapper it plugs into is already
here and already reviewed.

**Use [`super::single_flow_evap`] instead.** The homogeneous-equilibrium
path is complete, and it is what the benchmark cases run
(`neacrpd1t.m` sets `params.th_model = 'hem'`).

# Deviation from the MATLAB, and why

The MATLAB wraps each channel solve in `try/catch` and, on failure, keeps
that channel's previous state and continues (`driftflux6_solverstatic3d.m`
lines 165-168) — the surrounding Picard under-relaxation is expected to
absorb one stale channel-cycle. Reproducing that here would mean **every**
channel failing silently, the recovery tail running on inlet-state
defaults, and the function returning a plausible-looking converged result
built from nothing. [`solve_static`] therefore propagates the error instead
of swallowing it. This is a deliberate, documented departure from faithful
translation, made because the faithful path is unreachable: no channel can
ever succeed while the kernel is absent.

# Also not translated

- **The `parfor` channel sharding.** `params.stag6_par` / `stag6_nworkers`
  select a MATLAB process pool. The channels are independent, so this
  changes throughput and not results; the translation is serial.
- **The `evalc` log suppression** around the channel call, and the `verb`
  progress line. Diagnostics only.

```rust
pub mod drift_flux_3d { /* ... */ }
```

### Types

#### Struct `SingleChannelSolution`

The primary six-equation state one channel solve returns.

MATLAB `[Pc, Ac, VLc, VGc, TLc, TGc, Ust, qr, rel, stp, warm, fail]` out of
the local `stag6_channel` helper. Each vector is `nz` long, bottom node
first.

```rust
pub struct SingleChannelSolution {
    pub pressure: Vec<f64>,
    pub void_fraction: Vec<f64>,
    pub liquid_velocity: Vec<f64>,
    pub gas_velocity: Vec<f64>,
    pub liquid_temperature: Vec<f64>,
    pub gas_temperature: Vec<f64>,
    pub staggered_state: Vec<f64>,
    pub relative_error: f64,
    pub steps: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `Vec<f64>` | Pressure \[MPa\] at each axial node. |
| `void_fraction` | `Vec<f64>` | Void fraction \[-\] at each axial node. |
| `liquid_velocity` | `Vec<f64>` | Liquid velocity \[cm/s\] at each axial node. |
| `gas_velocity` | `Vec<f64>` | Vapour velocity \[cm/s\] at each axial node. |
| `liquid_temperature` | `Vec<f64>` | Liquid temperature \[K\] at each axial node. |
| `gas_temperature` | `Vec<f64>` | Vapour temperature \[K\] at each axial node. |
| `staggered_state` | `Vec<f64>` | Staggered state vector, `6*nz` long, kept as the next warm start.<br>MATLAB `r.Ustag`. |
| `relative_error` | `f64` | Relative residual reached \[-\]. MATLAB `r.relerr`. |
| `steps` | `usize` | JFNK steps taken. MATLAB `r.nsteps`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SingleChannelSolution { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SingleChannelSolution) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `solve_single_channel_static`

The missing single-channel staggered six-equation solver.

MATLAB `driftflux6_solverstatic1d(pc, gch, thch, pwch)` — **a file that
does not exist in the handed-over snapshot**. It is named here so the gap
is a findable, typed item rather than a comment, and so that whoever writes
the kernel has the call signature the wrapper expects.

From the wrapper's own use of the result, the kernel is expected to return
per-node pressure \[MPa\], void fraction \[-\], liquid and vapour velocity
\[cm/s\], liquid and vapour temperature \[K\], a `6*nz` staggered state
vector for warm starting, a relative residual and a step count. Nothing
else about it can be established from the snapshot, and nothing is assumed.

# Errors

Always [`ThError::MissingUpstreamSource`].

```rust
pub fn solve_single_channel_static(_params: &super::ThermalHydraulicParams, _geometry: &super::ThGeometry, _th: &super::ThermalHydraulicState, _channel_power_density: &[f64]) -> super::ThResult<SingleChannelSolution> { /* ... */ }
```

#### Function `solve_static`

Solve every fuelled channel with the six-equation two-fluid model and
recover the derived thermodynamic fields over the whole domain.

MATLAB `driftflux6_solverstatic3d(params, geometry, th, pwrdens)`.

# Arguments

- `params` — grid and solver knobs.
- `geometry` — axial node heights \[cm\] and per-channel extents.
- `th` — thermal-hydraulic state, updated in place.
- `power_density` — \[-\] L1-normalised, group-collapsed nodal power,
  `grid.nodes()` long. A channel is "fuelled" if any node of its axial
  column is non-zero.

# Errors

- [`ThError::MissingUpstreamSource`] as soon as the first fuelled channel
  needs [`solve_single_channel_static`] — which is to say, always, for any
  case with power in it. See this module's header.
- [`ThError::LengthMismatch`] if an input vector is not `grid.nodes()` long.

```rust
pub fn solve_static(params: &super::ThermalHydraulicParams, geometry: &super::ThGeometry, th: &mut super::ThermalHydraulicState, power_density: &[f64]) -> super::ThResult<()> { /* ... */ }
```

## Module `fuel_rod`

Steady 1-D cylindrical fuel-rod conduction.

# Provenance

Translated from `fuelrodheat_1dcylnd.m` by **Than Yan Ren** (Singapore
Nuclear Research and Safety Institute), BEDOK snapshot sha256
`e45cd6f57be2087c…`, received 2026-08-05. Faithful translation: the node
layout, the harmonic-mean conduction coefficients and the assembly order
are as in the original. Nothing is repaired.

# The physics

Steady radial conduction in a fuel pin,

```text
(1/r) d/dr ( k(T) r dT/dr ) + q''' = 0
```

integrated over annular control volumes and **divided through by `2*pi`**
(the MATLAB says so at its line 4; every coefficient below carries that
factor). The pellet is `fueln` rings, then a gas gap carrying a
*conductance*, then the cladding. A convective boundary condition closes
the outermost node.

# The node layout

One extra *surface* node is inserted at each material↔gap interface, so the
matrix is `maxid = maxir + surfcount` on a side — 24 for the NEACRP layout
(20 fuel rings + gap + clad). For that layout the solution vector reads:

| index (0-based) | what |
|---|---|
| 0 | pellet centreline ring |
| 1..=19 | remaining pellet rings |
| 20 | pellet outer **surface** |
| 21 | the gap ring — an orphan row, see below |
| 22 | cladding inner **surface** |
| 23 | cladding outer surface, where the coolant BC is applied |

The Doppler temperature the cross-section feedback uses is built from
indices `0` and `fueln` (`th_solverxyz.m:190`, `fueltemp(idx,fueln+1)` in
1-based terms) — the centreline and the pellet surface.

```rust
pub mod fuel_rod { /* ... */ }
```

### Functions

#### Function `solve_static`

Solve the steady radial temperature profile of one fuel pin.

MATLAB `fuelrodheat_1dcylnd(params, geometry, temps, pwr, bc, modtemp)`.

# Arguments

- `params` — radial node counts; `params.max_ir` must equal
  `geometry.which_k.len()`.
- `geometry` — radial mesh and material properties.
- `temperatures` — \[K\] the temperatures the **temperature-dependent
  conductivities are evaluated at**, i.e. the previous Picard iterate.
  Length `maxid` (see the module note on layout), *not* `maxir`.
- `volumetric_power` — \[W/cm³\] fission power density in the pellet,
  MATLAB `pwr`. Zero outside fuel. Typical PWR values are 200–600 W/cm³.
- `boundary_coefficient` — \[W/(cm·K)\] the convective boundary coefficient
  `hcoeff * Rtot`, MATLAB `bc`.
- `coolant_temperature` — \[K\] the coolant sink temperature, MATLAB
  `modtemp`.

# Returns

The radial temperature profile \[K\], length `maxid`, centreline first.

# Unfinished-code gaps carried over from the MATLAB

These are **recorded, not repaired**, per `docs/bedok-port-scoping.md`
§1.0.

1. **The gap ring is an orphan row fixed at `T = 1 K`.** When the loop
   reaches a gap ring it writes `bvec(id) = 1` and leaves that row's
   diagonal at the `1` the identity initialisation put there, so the row
   reads `T = 1 K`. No other row references that column: the pellet-surface
   row connects *across* the gap directly to the cladding-inner-surface row
   (`laplccol(counter) = id+2`). The 1 K value is physically meaningless.
   It survives because `th_solverxyz.m:185` clamps the whole profile to
   `[coolant temperature, tmaxfuel]` immediately afterwards, and because
   neither the Doppler temperature nor the wall heat flux reads that index.

2. **A layout with no material→gap transition indexes past the matrix.**
   With no gap, `surfcount = 0` so `maxid = maxir`, and the `ir == maxir`
   branch still writes column `id+1 = maxid+1` and reads `temps(id+1)`.
   In MATLAB that is a hard error from `sparse`. Here it is
   [`ThError::UnsupportedRodLayout`], raised before the write.

3. **A material→material interface with no gap between them assembles no
   conduction coefficient.** If ring `ir` and ring `ir+1` are different
   *conducting* materials, the loop sets `surf = 1` and re-enters with the
   same `ir`; the `surf == 1` branch then only does anything when
   `whichk(ir+1) == 0`. For a direct fuel→clad interface it therefore adds
   no off-diagonal at all and leaves `kplus` at its previous value, so the
   diagonal becomes `2*kplus_previous` and the two sides are not coupled.
   The benchmark geometries always place a gap between pellet and cladding,
   so this path is never exercised — but it is wrong, and it is left wrong.

4. **NaN in the solution is not an error here.** The MATLAB prints the
   matrix and continues; the caller (`th_solverxyz.m:194`) is what detects
   and substitutes. This function likewise returns NaN rather than failing.

# Errors

- [`ThError::LengthMismatch`] if `temperatures` is shorter than `maxid`, or
  `params.max_ir` disagrees with the geometry.
- [`ThError::UnsupportedRodLayout`] for the layouts described above.
- [`ThError::SingularMatrix`] if the assembled operator cannot be
  factorised.

```rust
pub fn solve_static(params: &super::FuelRodParams, geometry: &super::FuelRodGeometry, temperatures: &[f64], volumetric_power: f64, boundary_coefficient: f64, coolant_temperature: f64) -> super::ThResult<Vec<f64>> { /* ... */ }
```

## Module `fuel_rod_time`

Transient 1-D cylindrical fuel-rod conduction — one implicit-Euler step.

# Provenance

Translated from `fuelrodheattime_1dcylnd.m` by **Than Yan Ren** (Singapore
Nuclear Research and Safety Institute), BEDOK snapshot sha256
`e45cd6f57be2087c…`, received 2026-08-05. Faithful translation; nothing is
repaired.

# The physics

```text
rho*cp dT/dt = (1/r) d/dr ( k(T) r dT/dr ) + q'''
```

discretised exactly as the steady solver
([`super::fuel_rod::solve_static`]) — same node layout, same harmonic-mean
conduction coefficients, same boundary treatment, same division by `2*pi` —
with one heat-capacity term added per solution node:

```text
cap_id = rho*cp(T_old,id) * (r_cur^2 - r_prev^2) / 2 / dt      [W/(cm*K)]
```

added to the diagonal, and `cap_id * T_old,id` added to the source.
`[r_prev, r_cur]` is the radial interval that solution node represents.

The scheme is **semi-implicit**: conductivity is evaluated at the current
Picard iterate `temperatures` and heat capacity at the previous time step
`old_temperatures`. Gap and surface nodes carry no heat capacity.

Every unfinished-code gap listed on [`super::fuel_rod::solve_static`]
applies here too, unchanged — the two files are near-duplicates upstream.

```rust
pub mod fuel_rod_time { /* ... */ }
```

### Functions

#### Function `solve_transient`

Advance one fuel pin through a single implicit-Euler time step.

MATLAB `fuelrodheattime_1dcylnd(params, geometry, temps, tempsold, pwr, bc,
modtemp, dt)`.

# Arguments

- `params` — radial node counts; `params.max_ir` must match the geometry.
- `geometry` — radial mesh, conductivities and volumetric heat capacities.
- `temperatures` — \[K\] the current Picard iterate, used to evaluate the
  **conductivities**. Length `maxid`.
- `old_temperatures` — \[K\] the converged profile of the previous **time
  step**, used for the capacity terms and to evaluate `rho*cp`.
  Length `maxid`.
- `volumetric_power` — \[W/cm³\] pellet fission power density, MATLAB `pwr`.
- `boundary_coefficient` — \[W/(cm·K)\] convective boundary coefficient
  `hcoeff * Rtot`, MATLAB `bc`.
- `coolant_temperature` — \[K\] coolant sink temperature, MATLAB `modtemp`.
- `time_step` — \[s\] the step size `dt`. Must be strictly positive; the
  capacity terms divide by it.

# Returns

The radial temperature profile \[K\] at the end of the step, length `maxid`.

# Errors

- [`ThError::LengthMismatch`] if either temperature vector is shorter than
  `maxid`, or `params.max_ir` disagrees with the geometry.
- [`ThError::UnsupportedRodLayout`] for the layouts the upstream MATLAB
  cannot assemble — see [`super::fuel_rod::solve_static`].
- [`ThError::SingularMatrix`] if the assembled operator cannot be
  factorised.

# Panics

If `time_step` is not strictly positive. The MATLAB would silently produce
`Inf`/`NaN` capacity terms; a zero or negative step is a caller bug, so it
is caught here rather than propagated as a poisoned temperature field.

```rust
pub fn solve_transient(params: &super::FuelRodParams, geometry: &super::FuelRodGeometry, temperatures: &[f64], old_temperatures: &[f64], volumetric_power: f64, boundary_coefficient: f64, coolant_temperature: f64, time_step: f64) -> super::ThResult<Vec<f64>> { /* ... */ }
```

## Module `linalg`

Small dense linear solver standing in for MATLAB's sparse backslash.

# Provenance

Supporting code for the translation of Than Yan Ren's (SNRSI) MATLAB,
snapshot sha256 `e45cd6f57be2087c…`. It has no `.m` counterpart: it
replaces the `laplc \ bvec` line that closes both
`fuelrodheat_1dcylnd.m` and `fuelrodheattime_1dcylnd.m`.

# Why a dense solve

MATLAB assembles the rod-conduction operator with `sparse(...)` and solves
it with `\`, which for a square sparse matrix runs UMFPACK's LU with
partial pivoting. The matrix is `maxid x maxid` — **24 x 24** for the
NEACRP layout — so sparsity buys nothing, and a dense LU with partial
pivoting is the same algorithm on the same data. Keeping it in-crate also
keeps the factorisation deterministic and inspectable, which matters for a
reference implementation whose job is to be diffed against.

Row ordering, pivoting rule and elimination order are the only things that
can move the last bits of the answer; they are documented at
[`solve_dense_lu`].

```rust
pub mod linalg { /* ... */ }
```

### Types

#### Struct `DenseMatrix`

A dense square matrix in row-major order, assembled from triplets.

Mirrors MATLAB's `sparse(rows, cols, values, n, n)`: **duplicate
`(row, col)` entries are summed**, not overwritten. The rod-conduction
assembly relies on that behaviour for its diagonal.

```rust
pub struct DenseMatrix {
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
  pub fn zeros(order: usize) -> Self { /* ... */ }
  ```
  An `order` x `order` matrix of zeros.

- ```rust
  pub const fn order(self: &Self) -> usize { /* ... */ }
  ```
  Side length of the matrix.

- ```rust
  pub fn accumulate(self: &mut Self, row: usize, col: usize, value: f64) { /* ... */ }
  ```
  Add `value` to entry `(row, col)`, both 0-based.

- ```rust
  pub fn set(self: &mut Self, row: usize, col: usize, value: f64) { /* ... */ }
  ```
  Overwrite entry `(row, col)` with `value`, both 0-based.

- ```rust
  pub fn get(self: &Self, row: usize, col: usize) -> f64 { /* ... */ }
  ```
  Entry `(row, col)`, both 0-based.

- ```rust
  pub fn multiply(self: &Self, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Matrix-vector product `A * x`.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DenseMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DenseMatrix) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `solve_dense_lu`

Solve `a * x = b` by Gaussian elimination with partial pivoting.

The pivot at step `k` is the row at or below `k` with the largest absolute
entry in column `k`; ties keep the earlier row. Elimination proceeds
column by column, then back substitution runs from the last row upward.
This is the same algorithm MATLAB's `\` uses for a general square system,
so results agree to within the usual reordering-of-additions noise.

# Arguments

- `a` — the system matrix; consumed, since it is factorised in place.
- `b` — right-hand side, length `a.order()`.
- `what` — a label carried into the error, for diagnostics.

# Returns

The solution vector `x`, length `a.order()`.

# Errors

[`ThError::SingularMatrix`] if the largest available pivot in some column
is exactly zero. A `NaN` pivot is **not** treated as singular: the MATLAB
lets NaN propagate into `results` and its callers test for it afterwards
(`if any(isnan(results))`), so this does too.

# Panics

If `b.len() != a.order()`.

```rust
pub fn solve_dense_lu(a: DenseMatrix, b: &[f64], what: &'static str) -> super::ThResult<Vec<f64>> { /* ... */ }
```

## Module `single_flow_evap`

Steady homogeneous-equilibrium channel model with boiling.

# Provenance

Translated from `singleflow1devap.m` by **Than Yan Ren** (Singapore Nuclear
Research and Safety Institute), BEDOK snapshot sha256
`e45cd6f57be2087c…`, received 2026-08-05. Faithful translation. The IAPWS
calls go through [`super::steam`] rather than the third-party
`IAPWS_IF97.m`; that is the one substitution `docs/bedok-port-scoping.md`
§3 allows inside the reference.

# What it does — the MATLAB's own two stages

**Stage 1 — enthalpy march.** March the mixture enthalpy up (or down) each
channel from a pure energy balance at constant pressure,

```text
dh/dz = q'_wall / (G*A)
```

evaluated as a half-node march: the first active node takes half of its own
enthalpy rise, each subsequent node takes half of its predecessor's rise
plus half of its own.

**Stage 2 — invert the enthalpy.** Turn the mixture enthalpy into void
fraction, temperature and quality at the (constant) channel pressure `P`:

| condition | temperature | void |
|---|---|---|
| `h < hL(P)` | `T(P,h)` — subcooled liquid | 0 |
| `hL <= h <= hV(P)` | `Tsat(P)` — saturated mixture | drift-flux relation |
| `h > hV(P)` | `T(P,h)` — superheated vapour | 1 |

with the Zuber-Findlay void-quality closure

```text
alpha = x / [ C0*(x + (1-x)*rho_g/rho_l) + rho_g*Vgj/G ]
Vgj   = sqrt(2) * ( (rho_l - rho_g)*g*sigma / rho_l^2 )^0.25
```

`C0 = 1.2` by default (round tube), overridable; setting
[`ThermalHydraulicParams::homogeneous_evaporation`] forces the homogeneous
limit `C0 = 1`, `Vgj = 0`.

# Assumptions and limits

- **Constant pressure along the channel** — no pressure drop is computed.
- **Thermal equilibrium** — one mixture enthalpy, no subcooled boiling and
  no interfacial heat transfer.
- **Constant mass flux** — `th.flow_rate` is not updated.
- The MATLAB describes this as "a cheap, robust initial condition for
  `driftflux_solverstatic1d`". Because the six-equation solver is missing
  from the snapshot (see [`super::drift_flux_3d`]), it is in practice the
  channel model the benchmark cases run on.

# Notes on the original

- The comment at `singleflow1devap.m:105` says "steam at 900 K" but the code
  evaluates the enthalpy ceiling at **1050 K**. Translated as written.
- `inlett`, `flowdir` and `heatflux` are read into locals whether or not
  they are used; `Rtot` is used only in the wall-heat term. No behaviour
  depends on this.

```rust
pub mod single_flow_evap { /* ... */ }
```

### Functions

#### Function `solve_static`

Solve every channel's steady enthalpy march and invert it into a
two-phase state.

MATLAB `singleflow1devap(params, geometry, th, pwrdens)`.

# Arguments

- `params` — grid and void-closure knobs.
- `geometry` — axial node heights \[cm\] and per-channel axial extents.
- `th` — thermal-hydraulic state, updated in place. On entry
  `th.heat_flux` \[W/cm²\] must hold the wall heat flux of the previous
  Picard pass; on exit `th.coolant` carries the whole channel state.
- `power_density` — \[-\] the L1-normalised, group-collapsed nodal power
  distribution, `grid.nodes()` long, as produced by
  [`super::solver::solve_static`].

# Errors

- [`ThError::LengthMismatch`] if `power_density`, `geometry.axial_height`
  or `th.heat_flux` is not `grid.nodes()` long.

```rust
pub fn solve_static(params: &super::ThermalHydraulicParams, geometry: &super::ThGeometry, th: &mut super::ThermalHydraulicState, power_density: &[f64]) -> super::ThResult<()> { /* ... */ }
```

#### Function `check_coolant_for_nan`

Run the MATLAB `pauseonnan` checks the channel model's caller performs.

# Errors

[`ThError::NotANumber`] naming the first field that contains a NaN.

```rust
pub fn check_coolant_for_nan(th: &super::ThermalHydraulicState) -> super::ThResult<()> { /* ... */ }
```

## Module `single_flow_evap_time`

Transient homogeneous-equilibrium channel model — one implicit-Euler step.

# Provenance

Translated from `singleflow1devaptime.m` by **Than Yan Ren** (Singapore
Nuclear Research and Safety Institute), BEDOK snapshot sha256
`e45cd6f57be2087c…`, received 2026-08-05. Faithful translation. IAPWS calls
go through [`super::steam`]; see `docs/bedok-port-scoping.md` §3.

# The scheme

One implicit-Euler step of the 1-D single-pressure coolant energy equation
per channel,

```text
rho*A dh/dt + W dh/dz = q'_wall
```

marched **on the cell faces** with the cell-centred enthalpy taken as the
average of its two faces:

```text
W*(hf_i - hf_{i-1}) + cap_i*(hc_i - hc_i_old) = q_i
cap_i = rho_old * A * Lz / dt          [g/s]
hc_i  = (hf_{i-1} + hf_i) / 2          [kJ/kg]
```

solved node by node for `hf_i`. As `dt -> inf` the capacity term vanishes
and the scheme reduces exactly to the steady half-node march of
[`super::single_flow_evap`], which is what makes a transient consistent
with its own `t = 0` steady state.

Stage 2 — inverting the mixture enthalpy into void fraction, temperature
and quality — is *identical* to the steady model (the MATLAB says so at its
line 94) and is shared with it, see
[`super::single_flow_evap::invert_mixture_enthalpy`].

# Assumptions

Mass flow rate and channel pressure are **held constant** through the
transient. The MATLAB notes this is right for the NEACRP PWR cases
(constant inlet flow, constant 155 bar core pressure) and says nothing
about cases where it is not.

```rust
pub mod single_flow_evap_time { /* ... */ }
```

### Functions

#### Function `solve_transient`

Advance every channel's coolant enthalpy through one time step.

MATLAB `singleflow1devaptime(params, geometry, th, pwrdens, thold, dt)`.

# Arguments

- `params` — grid and void-closure knobs.
- `geometry` — axial node heights \[cm\] and per-channel axial extents.
- `th` — current thermal-hydraulic iterate, updated in place.
  `th.heat_flux` \[W/cm²\] must carry the wall heat flux of the previous
  step or Picard pass, and `th.power_ratio` the current relative core power.
- `power_density` — \[-\] L1-normalised, group-collapsed nodal power,
  `grid.nodes()` long.
- `previous_step` — the **converged** state of the previous time step. Only
  `previous_step.coolant.enthalpy` \[kJ/kg\] and
  `previous_step.coolant.density` \[g/cm³\] are read, for the capacity
  terms.
- `time_step` — \[s\] the step size `dt`.

# Errors

[`super::ThError::LengthMismatch`] if any input vector is not
`grid.nodes()` long.

# Panics

If `time_step` is not strictly positive — the capacity terms divide by it.

```rust
pub fn solve_transient(params: &super::ThermalHydraulicParams, geometry: &super::ThGeometry, th: &mut super::ThermalHydraulicState, power_density: &[f64], previous_step: &super::ThermalHydraulicState, time_step: f64) -> super::ThResult<()> { /* ... */ }
```

## Module `solver`

Steady thermal-hydraulic solver — the entry point the coupled Picard loop
calls.

# Provenance

Translated from `th_solverxyz.m` by **Than Yan Ren** (Singapore Nuclear
Research and Safety Institute), BEDOK snapshot sha256
`e45cd6f57be2087c…`, received 2026-08-05. Faithful translation; nothing is
repaired.

# What it does, in order

1. **Normalise and collapse the power.** L1-normalise the full `G*es`
   power-density vector, then sum the energy groups into the first `es`
   entries so everything downstream works on a single spatial field.
2. **Solve the coolant channels** — [`super::single_flow_evap`] when
   `params.channel_model` is
   [`ChannelModel::HomogeneousEquilibrium`](super::ChannelModel::HomogeneousEquilibrium),
   otherwise [`super::drift_flux_3d`] (which cannot run; see that module).
3. **Build the heat transfer coefficient** from a Dittus-Boelter
   correlation on the recovered coolant transport properties.
4. **Solve each fuelled node's rod conduction**
   ([`super::fuel_rod::solve_static`]), clamp the profile, and form the
   Doppler temperature and the wall heat flux.
5. **Check for NaN** and fail rather than pass poison into the neutronics.

# Not translated

- **The `params.debugdump` CSV dumps** (`th_solverxyz.m` lines 93-96 and
  215-241). Diagnostics with no effect on any returned value; they write
  `pwrdens.csv`, `hcoeff.csv`, `fueltemp.csv` and a dozen others into the
  working directory.
- **Dead locals.** The MATLAB computes `Vi = repmat(geometry.Vi, G, 1)`,
  `subflow = flowrate*subarea`, `Lx`, `Ly`, `Lr`, `maxir` and `whichg` and
  never uses them.
- **`th.coolant.temps = temps` and `th.coolant.dens = dens`** at the end are
  no-ops: both were read out of `th.coolant` a few lines earlier and never
  modified.

```rust
pub mod solver { /* ... */ }
```

### Functions

#### Function `solve_static`

Run one steady thermal-hydraulic solve over the whole core.

MATLAB `th_solverxyz(params, geometry, th, whichsigma, pwrdens)`.

# Arguments

- `params` — grid, fuel node counts, clamp and channel-model selection.
- `geometry` — axial node heights \[cm\], channel extents, fuel-pin radial
  geometry.
- `th` — thermal-hydraulic state, updated in place. `th.power_ratio` must
  already carry the current relative core power and `th.heat_flux`
  \[W/cm²\] the wall heat flux of the previous Picard pass.
- `which_sigma` — material index per spatial node, `grid.nodes()` long, in
  the MATLAB's 1-based material numbering where **`0` means "no material"**
  (a reflector or out-of-core node). A channel whose lowest active node has
  `which_sigma == 0` is skipped entirely.
- `power_density` — \[-\] nodal fission power density, the **full
  `grid.state_len()`** vector across all energy groups. It is normalised
  and collapsed inside; the caller's copy is not modified.

# Returns

Nothing; `th` carries the result: `coolant` (temperature, density,
enthalpy, void, transport properties), `heat_flux` \[W/cm²\],
`fuel_temperature` \[K\], `fuel_temperature_doppler` \[K\],
`fuel_temperature_average` \[K\] and `linear_power_density` \[W/cm\].

# Errors

- [`ThError::LengthMismatch`] if any input vector has the wrong length.
- [`ThError::NotANumber`] if the coolant enthalpy, temperature, density,
  fuel temperature, wall heat flux or Doppler temperature ends up
  containing a NaN — the translation of MATLAB `pauseonnan`.
- [`ThError::MissingUpstreamSource`] if `params.channel_model` selects the
  two-fluid path; see [`super::drift_flux_3d`].
- Anything [`super::fuel_rod::solve_static`] can raise.

# Behaviour on a NaN rod solve

If the rod conduction returns NaN at a node, the MATLAB emits a warning,
substitutes the local coolant temperature (or `params.cooltempavg` when
that is not finite either) into the whole radial profile, zeroes that node's
wall heat flux, and carries on. That is reproduced, warning included — it
is printed to stderr.

```rust
pub fn solve_static(params: &super::ThermalHydraulicParams, geometry: &super::ThGeometry, th: &mut super::ThermalHydraulicState, which_sigma: &[usize], power_density: &[f64]) -> super::ThResult<()> { /* ... */ }
```

#### Function `normalise_and_collapse`

**Attributes:**

- `MustUse { reason: None }`

L1-normalise a full `G*es` power-density vector and sum the energy groups
into a single `es`-long spatial field.

MATLAB `th_solverxyz.m:84-90`:

```text
pwrdens = pwrdens/norm(pwrdens,1);
for g = 2:G
    pwrdens(1:es) = pwrdens(1:es) + pwrdens((g-1)*es+1 : g*es);
end
pwrdens = pwrdens(1:es);
```

The group-major layout this relies on is the one pinned in
[`crate::reference::grid`].

# Arguments

- `power_density` — \[arbitrary, normalised away\] length `state_len()`.

# Returns

A `nodes()`-long field summing to 1 (up to sign cancellations, since the
normalisation uses the L1 norm of the *unsummed* vector).

```rust
pub fn normalise_and_collapse(grid: &crate::reference::grid::Grid, power_density: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `coolant_density`

**Attributes:**

- `MustUse { reason: None }`

Bring the mixture density into the form the cross-section feedback wants.

The MATLAB simply reads `th.coolant.dens`; this exists so the coupling
layer has a documented accessor rather than reaching into the struct.

# Returns

Coolant mixture density \[g/cm³\] per spatial node.

```rust
pub fn coolant_density(th: &super::ThermalHydraulicState) -> &[f64] { /* ... */ }
```

#### Function `channel_saturation_temperature`

**Attributes:**

- `MustUse { reason: None }`

Saturation temperature \[K\] of the channel pressure, for callers that need
to know where the two-phase region starts.

# Arguments

- `th` — read for `th.coolant.inlet_pressure` \[MPa\].

```rust
pub fn channel_saturation_temperature(th: &super::ThermalHydraulicState) -> f64 { /* ... */ }
```

## Module `solver_time`

Transient thermal-hydraulic solver — one implicit-Euler step of the
coupled channel + rod system.

# Provenance

Translated from `th_solvertimexyz.m` by **Than Yan Ren** (Singapore Nuclear
Research and Safety Institute), BEDOK snapshot sha256
`e45cd6f57be2087c…`, received 2026-08-05. Faithful translation; nothing is
repaired.

# Relationship to the steady solver

Structurally identical to [`super::solver::solve_static`], with the two
steady kernels swapped for their transient counterparts:

| steady | transient |
|---|---|
| [`super::single_flow_evap::solve_static`] | [`super::single_flow_evap_time::solve_transient`] |
| [`super::fuel_rod::solve_static`] | [`super::fuel_rod_time::solve_transient`] |

There is **no channel-model choice here**: the transient always marches the
homogeneous-equilibrium model. That is why `th_solverxyz.m` offers
`params.th_model = 'hem'` at all — a transient needs its `t = 0` steady
state from the same model, or the density mismatch shows up as a spurious
reactivity step at `t = 0`.

# Not translated

The `params.debugdump` CSV dumps (`th_solvertimexyz.m` lines 149-161) and
the same dead locals the steady solver carries. See
[`super::solver`].

```rust
pub mod solver_time { /* ... */ }
```

### Functions

#### Function `solve_transient`

Advance the whole core's thermal hydraulics through one time step.

MATLAB `th_solvertimexyz(params, geometry, th, whichsigma, pwrdens, thold,
dt)`.

# Arguments

- `params` — grid, fuel node counts and the fuel-temperature clamp.
  `params.channel_model` is **ignored**: the transient always uses the
  homogeneous-equilibrium channel march.
- `geometry` — axial node heights \[cm\], channel extents, fuel-pin radial
  geometry (including the volumetric heat capacities the transient needs).
- `th` — the current T-H iterate, updated in place. `th.heat_flux`
  \[W/cm²\] feeds the coolant energy source as the wall flux of the
  previous step or Picard pass, and `th.power_ratio` must already carry the
  current relative core power.
- `which_sigma` — material index per spatial node, `grid.nodes()` long,
  `0` meaning "no material".
- `power_density` — \[-\] nodal fission power, the full
  `grid.state_len()` vector; normalised and group-collapsed inside.
- `previous_step` — the **converged** T-H state of the previous time step,
  supplying the capacity terms for both the coolant and the rods.
- `time_step` — \[s\] the step size `dt`.

# Errors

As [`super::solver::solve_static`], plus anything
[`super::fuel_rod_time::solve_transient`] can raise.

# Panics

If `time_step` is not strictly positive.

```rust
pub fn solve_transient(params: &super::ThermalHydraulicParams, geometry: &super::ThGeometry, th: &mut super::ThermalHydraulicState, which_sigma: &[usize], power_density: &[f64], previous_step: &super::ThermalHydraulicState, time_step: f64) -> super::ThResult<()> { /* ... */ }
```

## Module `steam`

IAPWS-IF97 water/steam properties — **the one substitution allowed inside
the reference translation**.

# Provenance

The MATLAB calls `IAPWS_IF97.m`, which is third-party
(`Copyright (c) 2013 Mark Mifofski`) and is **not** ported; see
`docs/bedok-port-scoping.md` §3. This module is a thin adapter that answers
the same entry points out of the workspace's own
[`tampines_steam_tables`] crate, which implements IAPWS-IF97 in Rust.

Everything else in `src/reference/th/` is translated from Than Yan Ren's
(SNRSI) MATLAB, snapshot sha256 `e45cd6f57be2087c…`.

# Units — the whole point of this module

`tampines-steam-tables` is strictly SI (Pa, K, J/kg, m³/kg, W/(m·K), Pa·s)
and `uom`-typed. The MATLAB `IAPWS_IF97` wrapper uses the XSteam
convention: **MPa, K, kJ/kg, m³/kg, kJ/(kg·K), W/(m·K), Pa·s**, all bare
`f64`. Every function here takes and returns the *MATLAB* convention, so
the ported solvers read exactly like the original. The unit conversion
happens here and nowhere else.

# Why this is a parity risk, and what to check

Substituting an IF97 implementation *inside* the reference means every
downstream comparison silently inherits any disagreement. The gate for it
is not implementation-against-implementation but **both against the
published IAPWS-IF97 verification tables**, over the pressure/enthalpy
envelope the four benchmark cases exercise (PWR ~15.5 MPa, BWR ~6.7 MPa
plus the two-phase region). That check is `tampines-steam-tables`' own
responsibility and is **not** performed by this module.

# The saturation-line dispatch hazard

The MATLAB repeatedly evaluates liquid properties at
`min(temps, Tsat - 2*eps)`. As
[`MATLAB_EPS`](super::MATLAB_EPS) documents, `Tsat - 2*eps` is a **no-op**
in `f64` at reactor temperatures, so those calls land exactly on the
saturation temperature. `tampines-steam-tables`' `(T,p)` region dispatch
resolves such a point by comparing `p` against `p_sat(T)`; whether it lands
in region 1 or region 2 then depends on the last bit of the
`T_sat(p) -> p_sat(T)` round trip. [`thermal_conductivity_pt`] and
[`dynamic_viscosity_pt`] therefore go through the generic dispatch (as the
MATLAB does) but are guarded against the out-of-envelope panic; the
explicitly-regioned entry points ([`enthalpy_region1_pt`] and friends)
force their region and are immune.

```rust
pub mod steam { /* ... */ }
```

### Functions

#### Function `saturation_temperature`

**Attributes:**

- `MustUse { reason: None }`

Saturation temperature \[K\] at `pressure_mpa` \[MPa\].

MATLAB `IAPWS_IF97('Tsat_p', p)`. Valid from the triple point
(611.657 Pa) to the critical pressure (22.064 MPa).

```rust
pub fn saturation_temperature(pressure_mpa: f64) -> f64 { /* ... */ }
```

#### Function `enthalpy_region1_pt`

**Attributes:**

- `MustUse { reason: None }`

Region-1 (subcooled liquid) specific enthalpy \[kJ/kg\] at
`pressure_mpa` \[MPa\] and `temperature_kelvin` \[K\].

MATLAB `IAPWS_IF97('h1_pT', p, T)`. The region-1 equation is *forced*, not
dispatched, exactly as in the MATLAB. Valid 273.15–623.15 K up to 100 MPa.

```rust
pub fn enthalpy_region1_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `enthalpy_region2_pt`

**Attributes:**

- `MustUse { reason: None }`

Region-2 (vapour) specific enthalpy \[kJ/kg\] at `pressure_mpa` \[MPa\] and
`temperature_kelvin` \[K\].

MATLAB `IAPWS_IF97('h2_pT', p, T)`. Valid to 1073.15 K.

```rust
pub fn enthalpy_region2_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `specific_volume_region1_pt`

**Attributes:**

- `MustUse { reason: None }`

Region-1 specific volume \[m³/kg\] at `pressure_mpa` \[MPa\] and
`temperature_kelvin` \[K\].

MATLAB `IAPWS_IF97('v1_pT', p, T)`. Note the unit: the MATLAB keeps
specific volume in SI m³/kg even though everything around it is cgs, and
converts with `1/v/1000` to get g/cm³.

```rust
pub fn specific_volume_region1_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `specific_volume_region2_pt`

**Attributes:**

- `MustUse { reason: None }`

Region-2 specific volume \[m³/kg\] at `pressure_mpa` \[MPa\] and
`temperature_kelvin` \[K\]. MATLAB `IAPWS_IF97('v2_pT', p, T)`.

```rust
pub fn specific_volume_region2_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `isobaric_heat_capacity_region1_pt`

**Attributes:**

- `MustUse { reason: None }`

Region-1 isobaric specific heat \[kJ/(kg·K)\] at `pressure_mpa` \[MPa\] and
`temperature_kelvin` \[K\]. MATLAB `IAPWS_IF97('cp1_pT', p, T)`.

```rust
pub fn isobaric_heat_capacity_region1_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `saturated_liquid_enthalpy`

**Attributes:**

- `MustUse { reason: None }`

Saturated-liquid specific enthalpy \[kJ/kg\] at `pressure_mpa` \[MPa\].

MATLAB `IAPWS_IF97('hL_p', p)`, evaluated as the region-1 equation on the
saturation line.

```rust
pub fn saturated_liquid_enthalpy(pressure_mpa: f64) -> f64 { /* ... */ }
```

#### Function `saturated_vapour_enthalpy`

**Attributes:**

- `MustUse { reason: None }`

Saturated-vapour specific enthalpy \[kJ/kg\] at `pressure_mpa` \[MPa\].

MATLAB `IAPWS_IF97('hV_p', p)`, evaluated as the region-2 equation on the
saturation line.

```rust
pub fn saturated_vapour_enthalpy(pressure_mpa: f64) -> f64 { /* ... */ }
```

#### Function `saturated_vapour_specific_volume`

**Attributes:**

- `MustUse { reason: None }`

Saturated-vapour specific volume \[m³/kg\] at `pressure_mpa` \[MPa\].
MATLAB `IAPWS_IF97('vV_p', p)`.

```rust
pub fn saturated_vapour_specific_volume(pressure_mpa: f64) -> f64 { /* ... */ }
```

#### Function `temperature_ph`

**Attributes:**

- `MustUse { reason: None }`

Temperature \[K\] from pressure \[MPa\] and specific enthalpy \[kJ/kg\].

MATLAB `IAPWS_IF97('T_ph', p, h)` — the backward `(p,h)` flash, dispatched
across regions 1–4. In the two-phase region it returns `Tsat(p)`.

Returns `NaN` rather than panicking when the state is outside the IF97
backward-equation envelope. The MATLAB returns `NaN` there too, and
`singleflow1devap.m:117` explicitly guards against it
(`temps(~isfinite(temps)) = Tsat`).

```rust
pub fn temperature_ph(pressure_mpa: f64, enthalpy_kj_per_kg: f64) -> f64 { /* ... */ }
```

#### Function `thermal_conductivity_pt`

**Attributes:**

- `MustUse { reason: None }`

Thermal conductivity \[W/(m·K)\] at `pressure_mpa` \[MPa\] and
`temperature_kelvin` \[K\].

MATLAB `IAPWS_IF97('k_pT', p, T)`. The MATLAB divides the result by 100 to
reach W/(cm·K); this function keeps the SI value so the call site reads the
same as the original.

Returns `NaN` outside the IF97 forward envelope
(273.15–1073.15 K, 0 < p ≤ 100 MPa) instead of panicking — see the module
note on the saturation-line dispatch hazard.

```rust
pub fn thermal_conductivity_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 { /* ... */ }
```

#### Function `dynamic_viscosity_pt`

**Attributes:**

- `MustUse { reason: None }`

Dynamic viscosity \[Pa·s\] at `pressure_mpa` \[MPa\] and
`temperature_kelvin` \[K\]. MATLAB `IAPWS_IF97('mu_pT', p, T)`.

Returns `NaN` outside the IF97 forward envelope instead of panicking.

```rust
pub fn dynamic_viscosity_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 { /* ... */ }
```

## Module `w3_chf`

W-3 critical heat flux correlation and departure-from-nucleate-boiling
ratio.

# Provenance

Translated from `w3chf.m` and `w3chfhottest.m` by **Than Yan Ren**
(Singapore Nuclear Research and Safety Institute), BEDOK snapshot sha256
`e45cd6f57be2087c…`, received 2026-08-05. Faithful translation; the two
defects noted below are recorded, not repaired.

# The correlation

Tong's W-3 correlation for critical heat flux in a uniformly heated
channel, written as a product of four factors:

```text
q_CHF = K1 * K2 * K3 * K4 / 10                       [W/cm^2]

K1 = (2.022 - 0.06238 p) + (0.1722 - 0.01427 p) exp[(18.177 - 0.5987 p) X]
K2 = (0.1484 - 1.596 X + 0.1729 X|X|) * 2.326 * G * 10 + 3271
K3 = (1.157 - 0.869 X) * (0.2664 + 0.8357 exp(-124.1 De/100))
K4 = 0.8258 + 0.0003413 (h_f - h_in)
```

The published W-3 constants assume psia, `lb/(hr·ft²)`, inches and Btu/lb;
the constants above are the same correlation rewritten for **MPa,
g/(s·cm²), cm and kJ/kg**, giving `K1*K2*K3*K4` in kW/m² and hence the
final `/10` to reach W/cm². (For example `0.0004302 psia⁻¹ × 145.038 =
0.06238 MPa⁻¹`, and `0.000794 (Btu/lb)⁻¹ ÷ 2.326 = 0.0003413 (kJ/kg)⁻¹`.)

# Validity of W-3

The published range is roughly 5.5–16 MPa, mass flux
136–6800 kg/(m²·s) (13.6–680 g/(s·cm²)), quality −0.15 to +0.15, hydraulic
diameter 0.5–1.8 cm, and inlet subcooling below about 660 kJ/kg. Neither
the MATLAB nor this translation checks any of that — the correlation is
evaluated wherever it is called.

# Defects carried over from the MATLAB

Recorded, not repaired, per `docs/bedok-port-scoping.md` §1.0. See
[`critical_heat_flux`] and [`hottest_channel`].

# Not translated

`w3chf.m` ends with three unconditional `writematrix` calls dumping
`chf.csv`, `dnbr.csv` and `chfheatflux.csv` into the working directory.
Those are diagnostics with no effect on any returned value and are not
translated.

```rust
pub mod w3_chf { /* ... */ }
```

### Types

#### Struct `CriticalHeatFluxResult`

Critical heat flux and DNB ratio over the nodes the correlation was
evaluated on. MATLAB `chf` struct.

```rust
pub struct CriticalHeatFluxResult {
    pub critical_heat_flux: Vec<f64>,
    pub dnbr: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `critical_heat_flux` | `Vec<f64>` | Predicted critical heat flux \[W/cm²\] at each node. MATLAB `chf.chf`. |
| `dnbr` | `Vec<f64>` | Departure-from-nucleate-boiling ratio \[-\], `q_CHF / q_wall`, with<br>non-finite entries (zero wall heat flux) replaced by zero.<br>MATLAB `chf.dnbr`. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CriticalHeatFluxResult { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CriticalHeatFluxResult) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `w3_correlation`

**Attributes:**

- `MustUse { reason: None }`

The W-3 correlation evaluated at one point.

Factored out of the vectorised MATLAB expression so it can be checked
against a hand-worked value; the arithmetic is identical.

# Arguments

- `pressure_mpa` — \[MPa\] local coolant pressure.
- `quality` — \[-\] local equilibrium steam quality; may be negative
  (subcooled) in the published correlation, although this code path is fed
  the 0–1 clamped quality by the channel model.
- `mass_flux` — \[g/(s·cm²)\] local mixture mass flux `rho_m * v_m`.
- `hydraulic_diameter_cm` — \[cm\] subchannel hydraulic diameter.
- `subcooling` — \[kJ/kg\] `h_f(p) - h_in`, the enthalpy rise still
  available to saturation.

# Returns

Critical heat flux in **W/cm²**.

```rust
pub fn w3_correlation(pressure_mpa: f64, quality: f64, mass_flux: f64, hydraulic_diameter_cm: f64, subcooling: f64) -> f64 { /* ... */ }
```

#### Function `critical_heat_flux`

**Attributes:**

- `MustUse { reason: None }`

Predict the critical heat flux and DNB ratio at every node of `th`.

MATLAB `w3chf(geometry, th)`.

# Arguments

- `geometry` — only `geometry.fuel.hydraulic_diameter` \[cm\] is used.
  (The MATLAB also reads `subarea` and defines a gravity constant; neither
  is used, and neither is translated.)
- `th` — the coolant state. Reads `heat_flux` \[W/cm²\], `coolant.pressure`
  \[MPa\], `coolant.void_fraction`, `coolant.mixture_velocity` \[cm/s\],
  `coolant.liquid_density` and `coolant.gas_density` \[g/cm³\],
  `coolant.enthalpy` \[kJ/kg\], `coolant.quality`,
  `coolant.inlet_temperature` \[K\] and `coolant.inlet_pressure` \[MPa\].

# Returns

One [`CriticalHeatFluxResult`] entry per node of the state passed in.

# Defects carried over from the MATLAB

1. **`enthshift` is not the inlet enthalpy the W-3 `K4` factor calls for.**
   W-3's `K4` uses `h_f - h_in` with `h_in` the **channel inlet**
   enthalpy. The MATLAB instead builds
   `enthshift(i) = (0.5*enth(i) + 0.5*enth(i-1))/2` — a *local* two-node
   average, halved again by a stray outer `/2`. Only `enthshift(1)` is the
   inlet enthalpy. The halving alone roughly doubles the apparent
   subcooling and so inflates `K4`.
2. **The `i-1` walk runs over the flat node index, not along a channel.**
   Because `iz` varies fastest in the state vector, `enth(i-1)` is the node
   below within a channel, but at every channel boundary it is the *top of
   the previous channel*. The first node of each channel therefore mixes
   two unrelated channels' enthalpies.

Both are left exactly as written.

```rust
pub fn critical_heat_flux(geometry: &super::ThGeometry, th: &super::ThermalHydraulicState) -> CriticalHeatFluxResult { /* ... */ }
```

#### Function `hottest_channel`

**Attributes:**

- `MustUse { reason: None }`

Evaluate the W-3 correlation over the axially hottest channel only.

MATLAB `w3chfhottest(params, geometry, th)`. The "hottest" channel is the
one whose **whole axial column** of wall heat flux sums highest.

# Arguments

- `grid` — the node grid, for the channel sweep.
- `geometry` — as [`critical_heat_flux`].
- `th` — the full-core coolant state.

# Returns

A [`CriticalHeatFluxResult`] with `grid.nz` entries — one per axial node of
the selected channel, bottom first.

# Defect carried over from the MATLAB

**`w3chfhottest.m:21` sets `highy = ix`, not `iy`.** The `y` index of the
hottest channel is therefore overwritten with its `x` index, so the channel
actually evaluated is `(ix, ix)` — the diagonal — rather than the one whose
heat flux was measured. For a symmetric quarter-core layout the two often
coincide, which is presumably why it went unnoticed. Reproduced verbatim.

Note also that the MATLAB slices `th` but leaves `th.coolant.inletpress`
and `inlettemp` at their full-core values, which is correct — they are
scalars.

```rust
pub fn hottest_channel(grid: &crate::reference::grid::Grid, geometry: &super::ThGeometry, th: &super::ThermalHydraulicState) -> CriticalHeatFluxResult { /* ... */ }
```

### Types

#### Type Alias `ThResult`

Result alias for the thermal-hydraulics reference translation.

```rust
pub type ThResult<T> = std::result::Result<T, ThError>;
```

#### Enum `ThError`

Everything the ported thermal hydraulics can fail with.

This is deliberately a module-local error type rather than a new
[`crate::BedokError`] variant: `src/error.rs` is outside this module's
ownership. A `From<ThError> for BedokError` bridge should be added when the
coupling layer lands.

```rust
pub enum ThError {
    NotANumber {
        field: &'static str,
        index: usize,
    },
    MissingUpstreamSource {
        missing: &'static str,
        caller: &'static str,
    },
    LengthMismatch {
        what: &'static str,
        expected: usize,
        got: usize,
    },
    SingularMatrix {
        what: &'static str,
        pivot: usize,
    },
    UnsupportedRodLayout {
        reason: &'static str,
    },
}
```

##### Variants

###### `NotANumber`

A field came out of the solve containing NaN.

This is the translation of the MATLAB `pauseonnan` helper, which calls
`error('NaN occured')`. The MATLAB also rejects complex values; that
check has no Rust counterpart because every quantity here is `f64`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `&'static str` | Which field tripped the check. |
| `index` | `usize` | Flat spatial-node index of the first offending entry. |

###### `MissingUpstreamSource`

A MATLAB source file the snapshot depends on was not in the snapshot.

Yan Ren handed the code over unfinished; this is not a translation
oversight. Nothing is invented to fill the gap.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `missing` | `&'static str` | The `.m` file that does not exist. |
| `caller` | `&'static str` | The `.m` file that calls it. |

###### `LengthMismatch`

An input vector was not the length the grid implies.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which input. |
| `expected` | `usize` | Length implied by the grid. |
| `got` | `usize` | Length supplied. |

###### `SingularMatrix`

A linear system could not be factorised.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | Which solve. |
| `pivot` | `usize` | Zero pivot position (0-based). |

###### `UnsupportedRodLayout`

A fuel-rod radial layout the upstream MATLAB cannot assemble.

Not a Rust limitation — see [`fuel_rod::solve_static`] for the exact
out-of-range write in the original.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `reason` | `&'static str` | What about the layout the MATLAB cannot handle. |

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
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

#### Enum `FlowDirection`

Direction of coolant flow along the channel. MATLAB `th.flowdir`.

```rust
pub enum FlowDirection {
    Upward,
    Downward,
}
```

##### Variants

###### `Upward`

MATLAB `th.flowdir = 1` — inlet at the bottom of the channel.

###### `Downward`

MATLAB `th.flowdir = -1` — inlet at the top of the channel.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FlowDirection { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FlowDirection) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `ChannelModel`

Which channel model [`solver::solve_static`] dispatches to.

MATLAB `params.th_model`. The default in `th_solverxyz.m` is the two-fluid
path; `'hem'` selects the homogeneous-equilibrium enthalpy march. The
MATLAB comment records *why* the choice matters: `th_solvertimexyz` always
marches the HEM model, so a transient run needs its `t = 0` steady state
from the same model, or the density mismatch injects a spurious reactivity
step at `t = 0`.

```rust
pub enum ChannelModel {
    TwoFluid,
    HomogeneousEquilibrium,
}
```

##### Variants

###### `TwoFluid`

MATLAB default — `driftflux6_solverstatic3d`. **Not usable**: its
single-channel kernel is absent from the snapshot, see
[`drift_flux_3d`].

###### `HomogeneousEquilibrium`

MATLAB `params.th_model = 'hem'` — `singleflow1devap`. Complete, and
what the benchmark cases use.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ChannelModel { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ChannelModel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `RodMaterial`

Material class of one radial ring of the fuel-pin conduction mesh.

MATLAB `geometry.fuel.whichk`, which stores `1` for fuel, `0` for the
gas gap and `2` for cladding, and uses that value to index the `tcon` cell
array of conductivity function handles.

```rust
pub enum RodMaterial {
    Fuel,
    Gap,
    Clad,
}
```

##### Variants

###### `Fuel`

UO₂ fuel pellet. MATLAB `whichk == 1`.

###### `Gap`

Fuel-cladding gas gap. MATLAB `whichk == 0`. Carries a *conductance*
(W/(cm²·K)), not a conductivity, and no heat capacity.

###### `Clad`

Cladding. MATLAB `whichk == 2`.

##### Implementations

###### Methods

- ```rust
  pub const fn matlab_which_k(self: Self) -> usize { /* ... */ }
  ```
  The MATLAB `whichk` integer for this material.

- ```rust
  pub const fn is_fuel(self: Self) -> bool { /* ... */ }
  ```
  Whether this ring generates fission power. MATLAB `whichf = (whichk == 1)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RodMaterial { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RodMaterial) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `ThermalConductivityModel`

Temperature-dependent thermal conductivity of a rod material, in W/(cm·K).

The MATLAB stores these as anonymous function handles in
`geometry.fuel.tcon{...}`. Workspace rules forbid trait objects and boxed
closures, so the closed set of correlations the benchmark cases use is an
enum instead.

```rust
pub enum ThermalConductivityModel {
    Uo2Neacrp,
    ZircaloyNeacrp,
    Constant(f64),
}
```

##### Variants

###### `Uo2Neacrp`

UO₂ as used by the NEACRP cases:
`k(T) = (1.05 + 2150/(T - 73.15))/100` W/(cm·K), `T` in K.

Valid for solid UO₂ roughly 300–3000 K. Singular at `T = 73.15 K`,
which no reactor state reaches.

###### `ZircaloyNeacrp`

Zircaloy cladding as used by the NEACRP cases:
`k(T) = (7.51 + 2.09e-2 T - 1.45e-5 T² + 7.67e-9 T³)/100` W/(cm·K),
`T` in K. Valid roughly 300–1500 K.

###### `Constant`

A temperature-independent conductivity in W/(cm·K).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, temperature_kelvin: f64) -> f64 { /* ... */ }
  ```
  Thermal conductivity in W/(cm·K) at temperature `temperature_kelvin` \[K\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermalConductivityModel { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermalConductivityModel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `VolumetricHeatCapacityModel`

Temperature-dependent volumetric heat capacity `rho*cp`, in J/(cm³·K).

MATLAB `geometry.fuel.rhocp{...}`, used only by the transient rod solver.

```rust
pub enum VolumetricHeatCapacityModel {
    Uo2Neacrp,
    ZircaloyNeacrp,
    Constant(f64),
}
```

##### Variants

###### `Uo2Neacrp`

UO₂ as used by `neacrpa2t.m`:
`10.412*(1 - 0.01248)*(162.3 + 0.3038 T - 2.391e-4 T² + 6.404e-8 T³)/1000`
J/(cm³·K), `T` in K. The leading factor is the theoretical density
(g/cm³) reduced for porosity.

###### `ZircaloyNeacrp`

Zircaloy as used by `neacrpa2t.m`:
`6.6*(252.54 + 0.11474 T)/1000` J/(cm³·K), `T` in K.

###### `Constant`

A temperature-independent volumetric heat capacity in J/(cm³·K).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, temperature_kelvin: f64) -> f64 { /* ... */ }
  ```
  Volumetric heat capacity in J/(cm³·K) at `temperature_kelvin` \[K\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> VolumetricHeatCapacityModel { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &VolumetricHeatCapacityModel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `FuelRodParams`

Radial node counts of the fuel-pin conduction mesh. MATLAB `params.fuel`.

```rust
pub struct FuelRodParams {
    pub fuel_rings: usize,
    pub gap_rings: usize,
    pub clad_rings: usize,
    pub max_ir: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_rings` | `usize` | Radial rings inside the pellet. MATLAB `params.fuel.fueln`. |
| `gap_rings` | `usize` | Radial rings across the gas gap. MATLAB `params.fuel.gapn`. |
| `clad_rings` | `usize` | Radial rings across the cladding. MATLAB `params.fuel.cladn`. |
| `max_ir` | `usize` | Total rings, `fuel_rings + gap_rings + clad_rings`.<br>MATLAB `params.fuel.maxir`. |

##### Implementations

###### Methods

- ```rust
  pub const fn new(fuel_rings: usize, gap_rings: usize, clad_rings: usize) -> Self { /* ... */ }
  ```
  The conventional layout: `fuel_rings` pellet rings, then the gap, then

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelRodParams { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FuelRodParams) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `FuelRodGeometry`

Radial geometry and material properties of one fuel pin.
MATLAB `geometry.fuel`.

All lengths in cm. The rod is treated as radially one-dimensional; the
integrated heat equation is divided through by `2*pi`, which is why the
"volumes" below are per unit rod length.

```rust
pub struct FuelRodGeometry {
    pub fuel_radius: f64,
    pub gap_thickness: f64,
    pub clad_thickness: f64,
    pub outer_radius: f64,
    pub pitch: f64,
    pub doppler_alpha: f64,
    pub ring_thickness: Vec<f64>,
    pub ring_centre_radius: Vec<f64>,
    pub ring_area: Vec<f64>,
    pub which_k: Vec<RodMaterial>,
    pub subchannel_area: f64,
    pub hydraulic_diameter: f64,
    pub fuel_conductivity: ThermalConductivityModel,
    pub clad_conductivity: ThermalConductivityModel,
    pub gap_conductance: f64,
    pub fuel_heat_capacity: VolumetricHeatCapacityModel,
    pub clad_heat_capacity: VolumetricHeatCapacityModel,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_radius` | `f64` | Pellet outer radius \[cm\]. MATLAB `geometry.fuel.fuelrad`. |
| `gap_thickness` | `f64` | Radial gap thickness \[cm\]. MATLAB `geometry.fuel.fuelgap`. |
| `clad_thickness` | `f64` | Cladding thickness \[cm\]. MATLAB `geometry.fuel.clad`. |
| `outer_radius` | `f64` | Rod outer radius \[cm\], `fuel_radius + gap_thickness + clad_thickness`.<br>MATLAB `geometry.fuel.Rtot`. |
| `pitch` | `f64` | Square lattice pitch \[cm\]. MATLAB `geometry.fuel.pitch`. |
| `doppler_alpha` | `f64` | Doppler weighting `alpha` \[-\], in `T_doppler = (1-alpha) T_centre +<br>alpha T_surface`. Typically 0.7. MATLAB `geometry.fuel.doppleralpha`. |
| `ring_thickness` | `Vec<f64>` | Radial thickness of each ring \[cm\], length `max_ir`.<br>MATLAB `geometry.fuel.Lr`. |
| `ring_centre_radius` | `Vec<f64>` | Radius of each ring's centre \[cm\], length `max_ir`.<br>MATLAB `geometry.fuel.Ctr`. |
| `ring_area` | `Vec<f64>` | Cross-sectional area of each ring \[cm²\] (MATLAB calls it a volume,<br>`geometry.fuel.Vi`, because the rod is per unit length). |
| `which_k` | `Vec<RodMaterial>` | Material of each ring, length `max_ir`. MATLAB `geometry.fuel.whichk`. |
| `subchannel_area` | `f64` | Coolant flow area per pin \[cm²\], `pitch² - pi*outer_radius²`.<br>MATLAB `geometry.fuel.subarea`. |
| `hydraulic_diameter` | `f64` | Subchannel hydraulic diameter \[cm\]. MATLAB `geometry.fuel.hydia`. |
| `fuel_conductivity` | `ThermalConductivityModel` | Pellet conductivity. MATLAB `geometry.fuel.tcon{1}`. |
| `clad_conductivity` | `ThermalConductivityModel` | Cladding conductivity. MATLAB `geometry.fuel.tcon{2}`. |
| `gap_conductance` | `f64` | Gap **conductance** \[W/(cm²·K)\], not a conductivity.<br>MATLAB `geometry.fuel.tcon{end}` (the NEACRP benchmark value is 1.0). |
| `fuel_heat_capacity` | `VolumetricHeatCapacityModel` | Pellet volumetric heat capacity. MATLAB `geometry.fuel.rhocp{1}`.<br>Transient solve only. |
| `clad_heat_capacity` | `VolumetricHeatCapacityModel` | Cladding volumetric heat capacity. MATLAB `geometry.fuel.rhocp{2}`.<br>Transient solve only. |

##### Implementations

###### Methods

- ```rust
  pub fn conductivity(self: &Self, material: RodMaterial) -> Option<ThermalConductivityModel> { /* ... */ }
  ```
  Conductivity model of a conducting ring.

- ```rust
  pub fn heat_capacity(self: &Self, material: RodMaterial) -> Option<VolumetricHeatCapacityModel> { /* ... */ }
  ```
  Volumetric heat capacity model of a conducting ring, J/(cm³·K).

- ```rust
  pub fn cumulative_radius(self: &Self, ring: usize) -> f64 { /* ... */ }
  ```
  Cumulative outer radius of ring `ring` \[cm\], 0-based.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelRodGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FuelRodGeometry) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `ThGeometry`

Axial geometry the thermal hydraulics reads. MATLAB `geometry` (the parts
`th_solverxyz` and friends touch).

# Note on `axial_height`

MATLAB `geometry.Lz` is a **full state-length column vector** — one entry
per spatial node, not one per `iz`. `neacrpa2.m:43` builds it as
`zeros(maxix*maxiy*maxiz,1)` and `driftflux6_solverstatic3d.m:63` reshapes
it to `(maxiz, nch)`. This struct keeps that shape.

```rust
pub struct ThGeometry {
    pub axial_height: Vec<f64>,
    pub z_low: Vec<usize>,
    pub z_high: Vec<usize>,
    pub fuel: FuelRodGeometry,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `axial_height` | `Vec<f64>` | Axial node height \[cm\], one per spatial node (`grid.nodes()` long).<br>MATLAB `geometry.Lz`. |
| `z_low` | `Vec<usize>` | Lowest active axial node of each `(ix, iy)` channel, **0-based and<br>inclusive**. MATLAB `geometry.zlows`, which is 1-based; the conversion<br>happens once, here. Indexed `ix*ny + iy`. |
| `z_high` | `Vec<usize>` | Highest active axial node of each channel, 0-based inclusive.<br>MATLAB `geometry.zhis`. Indexed `ix*ny + iy`. |
| `fuel` | `FuelRodGeometry` | Fuel-pin radial geometry. MATLAB `geometry.fuel`. |

##### Implementations

###### Methods

- ```rust
  pub const fn channel_index(grid: &Grid, ix: usize, iy: usize) -> usize { /* ... */ }
  ```
  Index into [`z_low`](Self::z_low) / [`z_high`](Self::z_high) for the

- ```rust
  pub fn with_full_axial_extent(grid: &Grid, axial_height: Vec<f64>, fuel: FuelRodGeometry) -> Self { /* ... */ }
  ```
  A geometry whose every channel spans the full axial extent — the

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThGeometry) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `CoolantState`

Coolant state over the whole core. MATLAB `th.coolant`.

Every vector is `grid.nodes()` long and indexed by
[`Grid::index`](crate::reference::grid::Grid::index) with group `0`.

```rust
pub struct CoolantState {
    pub inlet_temperature: f64,
    pub inlet_pressure: f64,
    pub inlet_void: f64,
    pub enthalpy: Vec<f64>,
    pub enthalpy_face: Vec<f64>,
    pub temperature: Vec<f64>,
    pub void_fraction: Vec<f64>,
    pub quality: Vec<f64>,
    pub pressure: Vec<f64>,
    pub density: Vec<f64>,
    pub liquid_density: Vec<f64>,
    pub gas_density: Vec<f64>,
    pub mixture_velocity: Vec<f64>,
    pub thermal_conductivity: Vec<f64>,
    pub prandtl: Vec<f64>,
    pub kinematic_viscosity: Vec<f64>,
    pub liquid_velocity: Vec<f64>,
    pub gas_velocity: Vec<f64>,
    pub liquid_temperature: Vec<f64>,
    pub gas_temperature: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inlet_temperature` | `f64` | Channel inlet temperature \[K\]. MATLAB `th.coolant.inlettemp`. |
| `inlet_pressure` | `f64` | Channel pressure \[MPa\], held constant along the channel by the HEM<br>model. MATLAB `th.coolant.inletpress`. |
| `inlet_void` | `f64` | Volumetric inlet gas fraction \[-\]. MATLAB `th.coolant.inletvoid`. |
| `enthalpy` | `Vec<f64>` | Cell-centred mixture specific enthalpy \[kJ/kg\]. MATLAB `.enth`. |
| `enthalpy_face` | `Vec<f64>` | Cell-**face** enthalpy \[kJ/kg\] from the transient march.<br>MATLAB `.enthface`; the steady march does not set it. |
| `temperature` | `Vec<f64>` | Mixture temperature \[K\] — `Tsat(p)` in the two-phase region.<br>MATLAB `.temps`. |
| `void_fraction` | `Vec<f64>` | Void fraction \[-\], 0 to 1. MATLAB `.alphag`. |
| `quality` | `Vec<f64>` | Equilibrium steam quality \[-\], clamped to 0–1. MATLAB `.quality`. |
| `pressure` | `Vec<f64>` | Nodal pressure \[MPa\]. MATLAB `.press`. |
| `density` | `Vec<f64>` | Mixture density \[g/cm³\]. MATLAB `.dens`. |
| `liquid_density` | `Vec<f64>` | Saturated/subcooled liquid density \[g/cm³\]. MATLAB `.ldens`. |
| `gas_density` | `Vec<f64>` | Vapour density \[g/cm³\]. MATLAB `.gdens`. |
| `mixture_velocity` | `Vec<f64>` | Mixture velocity \[cm/s\]. MATLAB `.vm`. |
| `thermal_conductivity` | `Vec<f64>` | Liquid thermal conductivity \[W/(cm·K)\]. MATLAB `.tcon`. |
| `prandtl` | `Vec<f64>` | Liquid Prandtl number \[-\]. MATLAB `.pran`. |
| `kinematic_viscosity` | `Vec<f64>` | Liquid kinematic viscosity \[cm²/s\]. MATLAB `.kvis`. |
| `liquid_velocity` | `Vec<f64>` | Liquid velocity \[cm/s\] — six-equation model only. MATLAB `.vliq`. |
| `gas_velocity` | `Vec<f64>` | Vapour velocity \[cm/s\] — six-equation model only. MATLAB `.vgas`. |
| `liquid_temperature` | `Vec<f64>` | Liquid temperature \[K\] — six-equation model only. MATLAB `.tempsliq`. |
| `gas_temperature` | `Vec<f64>` | Vapour temperature \[K\] — six-equation model only. MATLAB `.tempsgas`. |

##### Implementations

###### Methods

- ```rust
  pub fn uniform(nodes: usize, inlet_temperature: f64, inlet_pressure: f64, inlet_void: f64, temperature: f64, density: f64) -> Self { /* ... */ }
  ```
  A uniform initial coolant state over `nodes` spatial nodes.

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoolantState { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoolantState) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `ThermalHydraulicState`

The whole thermal-hydraulic state passed through the coupled Picard loop.
MATLAB `th`.

```rust
pub struct ThermalHydraulicState {
    pub max_power: f64,
    pub power_ratio: f64,
    pub n_fuel_pins: f64,
    pub coolant_heat_fraction: f64,
    pub flow_rate: Vec<f64>,
    pub flow_direction: FlowDirection,
    pub heat_flux: Vec<f64>,
    pub radial_nodes: usize,
    pub fuel_temperature: Vec<f64>,
    pub fuel_temperature_average: Vec<f64>,
    pub fuel_temperature_doppler: Vec<f64>,
    pub linear_power_density: Vec<f64>,
    pub coolant: CoolantState,
    pub stag6_u_stag: Vec<f64>,
    pub stag6_q_ref: Vec<f64>,
    pub stag6_rel_err: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_power` | `f64` | Core thermal power at 100 % \[W\]. MATLAB `th.maxpow`. |
| `power_ratio` | `f64` | Current relative core power \[-\]. MATLAB `th.powratio`. |
| `n_fuel_pins` | `f64` | Fuel pins per node \[-\] (a real number, since cases scale by symmetry).<br>MATLAB `th.nfuelpin`. |
| `coolant_heat_fraction` | `f64` | Fraction of fission energy deposited directly in the coolant \[-\].<br>MATLAB `th.coolheatfrac`; 0.019 in the NEACRP cases. |
| `flow_rate` | `Vec<f64>` | Coolant mass flux \[g/(s·cm²)\], one per spatial node. MATLAB<br>`th.flowrate`, which may be a scalar; expand it with<br>[`uniform_flow_rate`](Self::uniform_flow_rate). |
| `flow_direction` | `FlowDirection` | Flow direction. MATLAB `th.flowdir`. |
| `heat_flux` | `Vec<f64>` | Wall heat flux at the rod surface \[W/cm²\], one per spatial node.<br>MATLAB `th.heatflux`. |
| `radial_nodes` | `usize` | Radial solution nodes per rod, MATLAB `maxid`. See<br>[`radial_solution_nodes`]. |
| `fuel_temperature` | `Vec<f64>` | Rod radial temperature profiles \[K\], `nodes * radial_nodes` entries in<br>row-major order (node-major, radial index fastest). MATLAB `th.fueltemp`,<br>an `es x maxid` matrix. |
| `fuel_temperature_average` | `Vec<f64>` | Node-average fuel temperature \[K\]. MATLAB `th.fueltempavg`. Note the<br>MATLAB sets it equal to the Doppler temperature rather than computing a<br>volume average — the volume-average line is commented out in<br>`th_solverxyz.m:189`. |
| `fuel_temperature_doppler` | `Vec<f64>` | Doppler-weighted fuel temperature \[K\] used by the cross-section<br>feedback. MATLAB `th.fueltempdoppler`. |
| `linear_power_density` | `Vec<f64>` | Linear power density \[W/cm\] per node. MATLAB `th.linpwrdens`. |
| `coolant` | `CoolantState` | Coolant fields. |
| `stag6_u_stag` | `Vec<f64>` | Warm-start state vector of the six-equation staggered solver,<br>`6*nz x n_channels` in column-major order. MATLAB `th.stag6_Ustag`.<br>Unused while the single-channel kernel is missing. |
| `stag6_q_ref` | `Vec<f64>` | Wall heat flux the warm start was taken at \[W/cm²\], `nz x n_channels`<br>column-major. MATLAB `th.stag6_qref`. |
| `stag6_rel_err` | `Vec<f64>` | Per-channel relative residual of the last six-equation solve \[-\],<br>`NaN` where never solved. MATLAB `th.stag6_relerr`. |

##### Implementations

###### Methods

- ```rust
  pub fn fuel_temperature_row(self: &Self, node: usize) -> &[f64] { /* ... */ }
  ```
  The rod radial temperature profile \[K\] of spatial node `node`.

- ```rust
  pub fn fuel_temperature_row_mut(self: &mut Self, node: usize) -> &mut [f64] { /* ... */ }
  ```
  Mutable view of [`fuel_temperature_row`](Self::fuel_temperature_row).

- ```rust
  pub fn uniform_flow_rate(self: &mut Self, mass_flux: f64, nodes: usize) { /* ... */ }
  ```
  Set a spatially uniform coolant mass flux \[g/(s·cm²)\].

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermalHydraulicState { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermalHydraulicState) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Struct `ThermalHydraulicParams`

Solver-level knobs the thermal hydraulics reads out of MATLAB `params`.

```rust
pub struct ThermalHydraulicParams {
    pub grid: crate::reference::grid::Grid,
    pub fuel: FuelRodParams,
    pub max_fuel_temperature: f64,
    pub coolant_average_temperature: f64,
    pub channel_model: ChannelModel,
    pub evaporation_c0: f64,
    pub homogeneous_evaporation: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `crate::reference::grid::Grid` | The node grid and group count. MATLAB `params.maxix/maxiy/maxiz/G`. |
| `fuel` | `FuelRodParams` | Fuel-pin radial node counts. MATLAB `params.fuel`. |
| `max_fuel_temperature` | `f64` | Ceiling of the fuel-temperature clamp \[K\]. MATLAB `params.tmaxfuel`,<br>default 3100 K (the UO₂ melting point). |
| `coolant_average_temperature` | `f64` | Fallback coolant temperature \[K\] substituted when the rod solve<br>returns NaN and the local coolant temperature is not finite either.<br>MATLAB `params.cooltempavg`. |
| `channel_model` | `ChannelModel` | Which channel model the static solver uses. MATLAB `params.th_model`. |
| `evaporation_c0` | `f64` | Zuber-Findlay distribution parameter `C0` \[-\] of the void-quality<br>closure. MATLAB `params.evap_C0`, default 1.2. |
| `homogeneous_evaporation` | `bool` | Force the homogeneous limit (`C0 = 1`, `Vgj = 0`).<br>MATLAB `params.evap_homog == 1`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(grid: Grid, fuel: FuelRodParams, coolant_average_temperature: f64) -> Self { /* ... */ }
  ```
  Parameters with the MATLAB defaults: 3100 K fuel clamp, `C0 = 1.2`,

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermalHydraulicParams { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermalHydraulicParams) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Functions

#### Function `radial_solution_nodes`

**Attributes:**

- `MustUse { reason: None }`

Number of *solution* nodes in the rod-conduction matrix, MATLAB `maxid`.

The conduction mesh inserts one extra node at every material↔gap interface,
so `maxid = maxir + surfcount` where `surfcount` counts transitions between
a conducting ring and the gap in either direction. Translated from the
identical loop that appears in `fuelrodheat_1dcylnd.m`,
`fuelrodheattime_1dcylnd.m` and `thdiffusion_solverxyz.m`.

For the NEACRP layout (20 fuel rings, 1 gap, 1 clad) this is `22 + 2 = 24`.

```rust
pub fn radial_solution_nodes(which_k: &[RodMaterial]) -> usize { /* ... */ }
```

#### Function `matlab_real_powf`

**Attributes:**

- `MustUse { reason: None }`

MATLAB's `real(x^p)` for real `x` and `p`.

MATLAB raises a **negative** real base to a fractional power in the complex
plane and `real()` then takes the real part, so `real((-x)^p)` is
`|x|^p * cos(p*pi)` — a finite, generally non-zero number. Rust's
`f64::powf` returns `NaN` for the same inputs. The difference is not
cosmetic: `th_solverxyz.m:149` wraps both `pran^0.4` and `reynolds^0.8` in
`real()` precisely because those arguments can go negative when a property
flash misbehaves, and a `NaN` there would propagate into the fuel
temperature and trip the `pauseonnan` guard.

# Arguments

- `base` — any real number, including negatives, infinities and `NaN`.
- `exponent` — the real exponent.

# Returns

`base.powf(exponent)` when `base >= 0` or `base` is `NaN`; otherwise
`(-base).powf(exponent) * cos(exponent*pi)`.

```rust
pub fn matlab_real_powf(base: f64, exponent: f64) -> f64 { /* ... */ }
```

#### Function `pause_on_nan`

Return `Err(ThError::NotANumber)` if `values` contains any NaN.

Translation of `pauseonnan.m`, which prints the offending array and calls
`error('NaN occured')`. The MATLAB also errors on complex input; that arm
has no counterpart here.

# Errors

[`ThError::NotANumber`] naming `field` and the first offending index.

```rust
pub fn pause_on_nan(field: &'static str, values: &[f64]) -> ThResult<()> { /* ... */ }
```

#### Function `fix_inf_nan`

**Attributes:**

- `MustUse { reason: None }`

Replace every non-finite entry (`+/-Inf`, `NaN`) with zero.

Translation of `fixinfnan.m` in its default mode. The MATLAB's optional
second mode — substituting `min(abs(vector))` — is not used by any file in
this module's scope and is not translated.

```rust
pub fn fix_inf_nan(values: &[f64]) -> Vec<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `MATLAB_EPS`

MATLAB `eps` — the double-precision machine epsilon, 2.220446049250313e-16.

Used verbatim wherever the MATLAB writes `max(x, eps)` or `Tsat - 2*eps`.

# A translation hazard worth knowing about

`Tsat - 2*eps` is a **no-op** at reactor temperatures: at `Tsat ≈ 618 K` one
unit in the last place is about `1.1e-13`, three orders of magnitude larger
than `2*eps`. The MATLAB's intent ("nudge just below saturation so the
liquid branch is selected") is therefore not achieved in the original
either. The translation reproduces the arithmetic exactly rather than
repairing it; see [`steam`] for how the region dispatch copes.

```rust
pub const MATLAB_EPS: f64 = f64::EPSILON;
```

### Re-exports

#### Re-export `Geometry`

```rust
pub use grid::Geometry;
```

#### Re-export `Grid`

```rust
pub use grid::Grid;
```

## Module `substituted`

Stage 2 — the same physics rebuilt on OUTRAM PARK libraries.

**No physics lives here yet.** What lives here is the *seam*: the set of
swappable components, the enum each one dispatches through, and the parity
state each is in. The seam exists before the implementations so that a
substitution arrives as one new enum variant with a parity gate attached,
rather than as a fork of the solver.

# The rule that governs this module

No component is accepted here until it reproduces [`crate::reference`] on
the benchmark suite to a stated tolerance, and **no component is improved
before it has passed parity**. A substitution that changes results *and*
claims to be better cannot be distinguished from one that is simply wrong.

[`Component`] enumerates the substitutions planned in
`docs/bedok-port-scoping.md` §5, and [`Component::parity_status`] records
where each one stands. That is deliberately data rather than prose: a test
walks [`Component::ALL`] and fails if the map here drifts from the scoping
document, and adding a substitution without stating its parity status will
not compile.

# Dispatch

Each component module defines an enum whose variants are the available
implementations — [`channel_flow::ChannelFlowKernel`] and friends. Per the
workspace Rust rules there are **no trait objects**: the set of physics
implementations is closed and known at compile time, so adding one forces
every `match` to handle it, and a missed case is a compile error rather
than a runtime surprise. The shape every kernel follows is:

```text
match kernel {
    Kernel::Reference   => reference_path(...),
    Kernel::Substitute  => substituted_path(...),
}
```

# A note on the linear-solver substitution

The reference path uses a **direct** sparse LU (`faer`), because that is
what MATLAB's `\` and `decomposition()` do, and stage 1 must match it.
`outram-foam-basic-lib` offers **iterative** solvers only — conjugate
gradient, GAMG, Gauss-Seidel. Swapping one in is therefore a real
substitution with a real question behind it: does an iterative solve reach
the same `k_eff` as a direct factorisation, and at what cost? Note also
that the diffusion LHS (`gradD + nodal + sigma.tot - sigma.s`) is
**non-symmetric** because of down-scattering, so conjugate gradient does
not apply to it unmodified. This is the substitution most likely to break
bit-level agreement while being perfectly correct, which is why parity
tolerances are set physically rather than at machine epsilon.

```rust
pub mod substituted { /* ... */ }
```

### Modules

## Module `channel_flow`

Substitution seam for single-phase channel flow with evaporation.

Reference origin: `singleflow1devap.m` / `singleflow1devaptime.m` — a 1-D
single-phase channel with evaporation, steady and transient. Proposed
substitute: `tuas_boussinesq_solver` (fluid-array machinery) composed
through `tampines`.

**No implementation here yet.** [`ChannelFlowKernel::Tuas`] names the
substitution and carries its parity state; the physics arrives with the
gate, not before.

```rust
pub mod channel_flow { /* ... */ }
```

### Types

#### Enum `ChannelFlowKernel`

Which implementation performs the single-phase channel-flow solve.

Enum dispatch, not a trait object: the set of channel-flow implementations
is closed, so adding one is a compile error at every `match` that has not
been updated.

```rust
pub enum ChannelFlowKernel {
    Reference,
    Tuas,
}
```

##### Variants

###### `Reference`

The stage-1 faithful translation in `reference::th`.

The default, and the oracle every substitution is measured against.

###### `Tuas`

`tuas_boussinesq_solver` / `tampines` standing in for it.

**Not implemented.** Selecting it today only records an intent; there is
no code behind it and it has not passed a parity gate. The open physics
question is whether TUAS's fluid-array formulation reproduces the
MATLAB's evaporation treatment, which is where the two models most
plainly differ.

##### Implementations

###### Methods

- ```rust
  pub const fn implementation(self: &Self) -> Implementation { /* ... */ }
  ```
  Which of the two paths a call on this kernel would take.

- ```rust
  pub const fn is_accepted(self: &Self) -> bool { /* ... */ }
  ```
  Whether this kernel may be used in a solve.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ChannelFlowKernel { /* ... */ }
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
    fn default() -> ChannelFlowKernel { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ChannelFlowKernel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `chf`

Substitution seam for critical heat flux.

Reference origin: `w3chf.m` / `w3chfhottest.m` — the W-3 correlation, and a
hot-channel variant of it. Two candidate substitutes exist in the
workspace, `outram-foam-multiphase::chf` and `outram-foam-appbuilder-lib`'s
`closures::heat_transfer::chf`; **which of them actually implements W-3**
is the open question, and a substitution that quietly swaps in a different
correlation is not a parity failure but a physics change.

**No implementation here yet.**

```rust
pub mod chf { /* ... */ }
```

### Types

#### Enum `ChfKernel`

Which implementation evaluates the critical heat flux.

```rust
pub enum ChfKernel {
    Reference,
    OutramFoamMultiphase,
    AppbuilderClosure,
}
```

##### Variants

###### `Reference`

The stage-1 faithful translation of W-3 in `reference::th`.

###### `OutramFoamMultiphase`

`outram-foam-multiphase::chf` standing in for it.

**Not implemented.** Confirm it is W-3 before gating it.

###### `AppbuilderClosure`

`outram-foam-appbuilder-lib` `closures::heat_transfer::chf` standing in
for it.

**Not implemented.** Confirm it is W-3 before gating it.

##### Implementations

###### Methods

- ```rust
  pub const fn implementation(self: &Self) -> Implementation { /* ... */ }
  ```
  Which of the two paths a call on this kernel would take.

- ```rust
  pub const fn is_accepted(self: &Self) -> bool { /* ... */ }
  ```
  Whether this kernel may be used in a solve. See

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ChfKernel { /* ... */ }
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
    fn default() -> ChfKernel { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ChfKernel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `cross_sections`

Substitution seam for cross-section data and its feedback update.

Reference origin: `sigmavalupd3d.m`, plus the two-group data each benchmark
case carries. Proposed substitute: `njoy-outram-park-fork`.

This is explicitly a **later** step. The benchmarks supply their own
two-group sets, and those sets are part of the benchmark specification: a
solve using cross sections generated from evaluated nuclear data is no
longer solving the same problem, so it cannot be compared to the reference
as a parity check. What `njoy-outram-park-fork` substitutes for is the
*feedback interpolation* — how cross sections vary with fuel temperature,
moderator density and boron — not the benchmark data itself.

**No implementation here yet.**

```rust
pub mod cross_sections { /* ... */ }
```

### Types

#### Enum `CrossSectionSource`

Where cross sections and their feedback derivatives come from.

```rust
pub enum CrossSectionSource {
    Reference,
    Njoy,
}
```

##### Variants

###### `Reference`

The stage-1 faithful translation: benchmark-supplied two-group data
with the MATLAB's feedback update.

###### `Njoy`

`njoy-outram-park-fork` standing in for it.

**Not implemented.** See the module note on why this cannot be a
straight parity comparison against a benchmark-data solve.

##### Implementations

###### Methods

- ```rust
  pub const fn implementation(self: &Self) -> Implementation { /* ... */ }
  ```
  Which of the two paths a call on this source would take.

- ```rust
  pub const fn is_accepted(self: &Self) -> bool { /* ... */ }
  ```
  Whether this source may be used in a solve. See

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CrossSectionSource { /* ... */ }
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
    fn default() -> CrossSectionSource { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CrossSectionSource) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `drift_flux`

Substitution seam for six-equation drift-flux two-phase flow.

Reference origin: `driftflux6_solverstatic3d.m`. Proposed substitute:
`outram-foam-multiphase::drift_flux`, which exists but whose fidelity match
to Yan Ren's formulation is unverified — the two may not close the same set
of six equations, which is the first thing a parity attempt must establish.

**No implementation here yet.**

```rust
pub mod drift_flux { /* ... */ }
```

### Types

#### Enum `DriftFluxKernel`

Which implementation performs the drift-flux two-phase solve.

```rust
pub enum DriftFluxKernel {
    Reference,
    OutramFoamMultiphase,
}
```

##### Variants

###### `Reference`

The stage-1 faithful translation in `reference::th`.

###### `OutramFoamMultiphase`

`outram-foam-multiphase::drift_flux` standing in for it.

**Not implemented.** Selecting it today only records an intent.

##### Implementations

###### Methods

- ```rust
  pub const fn implementation(self: &Self) -> Implementation { /* ... */ }
  ```
  Which of the two paths a call on this kernel would take.

- ```rust
  pub const fn is_accepted(self: &Self) -> bool { /* ... */ }
  ```
  Whether this kernel may be used in a solve. See

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DriftFluxKernel { /* ... */ }
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
    fn default() -> DriftFluxKernel { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DriftFluxKernel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `fuel_rod`

Substitution seam for one-dimensional cylindrical fuel-rod conduction.

Reference origin: `fuelrodheat_1dcylnd.m` / `fuelrodheat_1dcylndtime.m` —
steady and transient radial conduction in a fuel rod. Two candidate
substitutes: `outram-park-fork-offbeat`, which is much the richer model
(eigenstrain, gap conductance, fission-gas release), and TUAS's
`one_d_solid_structure`, which is closer in scope to the original.

Richer is not automatically better for a parity gate: OFFBEAT models effects
the reference does not, so agreement is only expected where those effects
are switched off. That has to be arranged deliberately rather than hoped
for.

**No implementation here yet.**

```rust
pub mod fuel_rod { /* ... */ }
```

### Types

#### Enum `FuelRodKernel`

Which implementation solves radial conduction in the fuel rod.

```rust
pub enum FuelRodKernel {
    Reference,
    Offbeat,
    TuasSolidStructure,
}
```

##### Variants

###### `Reference`

The stage-1 faithful translation in `reference::th`.

###### `Offbeat`

`outram-park-fork-offbeat` standing in for it.

**Not implemented.** The richer model; see the module note on what that
costs a parity comparison.

###### `TuasSolidStructure`

TUAS `one_d_solid_structure` standing in for it.

**Not implemented.** Closer in scope to the original.

##### Implementations

###### Methods

- ```rust
  pub const fn implementation(self: &Self) -> Implementation { /* ... */ }
  ```
  Which of the two paths a call on this kernel would take.

- ```rust
  pub const fn is_accepted(self: &Self) -> bool { /* ... */ }
  ```
  Whether this kernel may be used in a solve. See

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelRodKernel { /* ... */ }
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
    fn default() -> FuelRodKernel { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FuelRodKernel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `kinetics`

Substitution seam for delayed-neutron kinetics in the transient path.

Reference origin: the delayed-neutron precursor treatment inside
`thdiffusion_solvertimexyz.m`. Proposed substitute: `teh-o-prke`.

The substitution is not a like-for-like swap and should not be described as
one. The reference carries **spatially resolved** precursor concentrations
alongside the flux; `teh-o-prke` solves **point** reactor kinetics. They
coincide only under a shape assumption, so this gate is as much about
stating that assumption as about measuring a difference.

**No implementation here yet.**

```rust
pub mod kinetics { /* ... */ }
```

### Types

#### Enum `KineticsKernel`

Which implementation advances the delayed-neutron precursors.

```rust
pub enum KineticsKernel {
    Reference,
    TehOPrke,
}
```

##### Variants

###### `Reference`

The stage-1 faithful translation in `reference::coupling`.

Spatially resolved precursors, as in the MATLAB.

###### `TehOPrke`

`teh-o-prke` point kinetics standing in for it.

**Not implemented.** See the module note: this is a fidelity change, not
only an implementation change.

##### Implementations

###### Methods

- ```rust
  pub const fn implementation(self: &Self) -> Implementation { /* ... */ }
  ```
  Which of the two paths a call on this kernel would take.

- ```rust
  pub const fn is_accepted(self: &Self) -> bool { /* ... */ }
  ```
  Whether this kernel may be used in a solve. See

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> KineticsKernel { /* ... */ }
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
    fn default() -> KineticsKernel { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &KineticsKernel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Module `linear_solver`

Substitution seam for the sparse linear solve behind the diffusion
equation.

Reference origin: MATLAB's `\`, `decomposition()` and `gmres`. The stage-1
translation uses a **direct** sparse LU (`faer`) because that is what those
do, and stage 1 must match them.

Proposed substitute: `outram-foam-basic-lib`'s `ldu_matrix` and `krylov`,
which offer **iterative** solvers only — conjugate gradient, GAMG,
Gauss-Seidel.

# Why this row is the awkward one

Two things make it so, and both are properties of the problem rather than
of anyone's code:

- **A direct factorisation and an iterative solve do not agree bit for
  bit.** The iterative answer is only as converged as its own tolerance, so
  the parity tolerance for this component has to be set from the physics —
  how much `k_eff` movement is acceptable — not from machine epsilon. This
  is the substitution most likely to move results while being entirely
  correct.
- **The diffusion left-hand side is non-symmetric.** `gradD + nodal +
  sigma.tot - sigma.s` picks up asymmetry from down-scattering, so
  conjugate gradient does not apply to it unmodified. A substitution must
  choose a method that tolerates that, and say which.

**No implementation here yet.**

```rust
pub mod linear_solver { /* ... */ }
```

### Types

#### Enum `LinearSolverKernel`

Which linear solver factorises or iterates the diffusion system.

```rust
pub enum LinearSolverKernel {
    Reference,
    OutramFoamKrylov,
}
```

##### Variants

###### `Reference`

The stage-1 direct sparse LU (`faer`), matching MATLAB's `\`.

###### `OutramFoamKrylov`

`outram-foam-basic-lib` `ldu_matrix` / `krylov` standing in for it.

**Not implemented.** Iterative; see the module note on the tolerance and
symmetry consequences.

##### Implementations

###### Methods

- ```rust
  pub const fn implementation(self: &Self) -> Implementation { /* ... */ }
  ```
  Which of the two paths a call on this kernel would take.

- ```rust
  pub const fn is_accepted(self: &Self) -> bool { /* ... */ }
  ```
  Whether this kernel may be used in a solve. See

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LinearSolverKernel { /* ... */ }
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
    fn default() -> LinearSolverKernel { /* ... */ }
    ```

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LinearSolverKernel) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

### Types

#### Enum `Implementation`

Which implementation a running solve is dispatching to.

Every component kernel maps onto one of these, so a solve can report the
path it actually took without the reporting code knowing which component it
is looking at.

```rust
pub enum Implementation {
    Reference,
    Substituted,
}
```

##### Variants

###### `Reference`

The stage-1 faithful translation in [`crate::reference`].

###### `Substituted`

An OUTRAM PARK library standing in for it.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Implementation { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Implementation) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `ParityStatus`

How far a component has got through its parity gate.

Recorded as data so the substitution map cannot quietly claim more than has
been measured. Per the workspace V&V rule, a `Passed` variant must carry the
measured number and the date it was measured — not merely the word "passed".

```rust
pub enum ParityStatus {
    NotStarted,
    AwaitingGate,
    Passed {
        max_relative_difference: f64,
        measured: &'static str,
    },
    Failed {
        max_relative_difference: f64,
        measured: &'static str,
    },
}
```

##### Variants

###### `NotStarted`

No substituted implementation has been written.

###### `AwaitingGate`

An implementation exists but has not been run against the reference.

###### `Passed`

Measured against the reference and within the stated tolerance.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `max_relative_difference` | `f64` | Largest relative difference measured against the reference \[-\]. |
| `measured` | `&'static str` | ISO date the measurement was taken, e.g. `"2026-08-05"`. |

###### `Failed`

Measured against the reference and outside the stated tolerance.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `max_relative_difference` | `f64` | Largest relative difference measured against the reference \[-\]. |
| `measured` | `&'static str` | ISO date the measurement was taken. |

##### Implementations

###### Methods

- ```rust
  pub const fn is_accepted(self: &Self) -> bool { /* ... */ }
  ```
  Whether this component may be used in a substituted solve.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ParityStatus { /* ... */ }
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

- **DistributionExt**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ParityStatus) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

#### Enum `Component`

A component of the solve that stage 2 plans to replace.

The variants are exactly the rows of the substitution map in
`docs/bedok-port-scoping.md` §5. Adding a row there means adding a variant
here, which forces every method below to account for it.

```rust
pub enum Component {
    ChannelFlow,
    DriftFlux,
    CriticalHeatFlux,
    FuelRod,
    Kinetics,
    CrossSections,
    LinearSolver,
}
```

##### Variants

###### `ChannelFlow`

Single-phase channel flow with evaporation.

###### `DriftFlux`

Six-equation drift-flux two-phase flow.

###### `CriticalHeatFlux`

Critical heat flux (W-3 correlation).

###### `FuelRod`

One-dimensional cylindrical fuel-rod conduction.

###### `Kinetics`

Delayed-neutron kinetics in the transient path.

###### `CrossSections`

Cross-section data and its feedback update.

###### `LinearSolver`

Sparse linear algebra behind the diffusion solve.

##### Implementations

###### Methods

- ```rust
  pub const fn matlab_origin(self: &Self) -> &'static str { /* ... */ }
  ```
  The MATLAB routines this component is translated from.

- ```rust
  pub const fn substitute(self: &Self) -> &'static str { /* ... */ }
  ```
  The OUTRAM PARK crate (or crates) proposed to stand in for it.

- ```rust
  pub const fn parity_status(self: &Self) -> ParityStatus { /* ... */ }
  ```
  Where this component stands against its parity gate.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Component { /* ... */ }
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

- **DistributionExt**
- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Component) -> bool { /* ... */ }
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
- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

## Re-exports

### Re-export `BedokError`

```rust
pub use error::BedokError;
```

### Re-export `Result`

```rust
pub use error::Result;
```

