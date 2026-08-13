# Crate Documentation

**Version:** 0.1.1

**Format Version:** 60

# Module `outram_foam_appbuilder_lib`

**This is OUTRAM PARK's independent Rust translation of selected
OpenFOAM® solver-application algorithms — it is not the official
OpenFOAM® software and is not affiliated with, endorsed by, or
sanctioned by OpenCFD Ltd. or the ESI Group.** OpenFOAM® is a registered
trademark of OpenCFD Limited. See `TRADEMARKS.md` (this crate's
directory, mirrored from the workspace root) for the full attribution
and non-affiliation notice.

# `outram-foam-appbuilder-lib` — Layer 5: solver applications and case I/O

This crate is the **application layer** of the OUTRAM PARK OpenFOAM-in-Rust
stack. It sits on top of `outram-foam-basic-lib` (Layers 1–4: tensors,
fields, mesh, `fvm`/`fvc` operators, linear solvers) and
`outram-foam-turbulence-lib` (turbulence closures), and supplies the parts
those crates deliberately do not: the **time-advancement loops**, the
**case-file structures**, and the **multiphysics coupling drivers**.

```text
outram-foam-basic-lib        Layers 1–4  primitives, fields, mesh, FV operators
outram-foam-turbulence-lib   Layer 4     RAS/LES closures
           │
           ▼
outram-foam-appbuilder-lib   Layer 5     ← THIS CRATE
```

## Where to start

- [`solvers`] — one submodule per ported OpenFOAM application. Each owns its
  PISO/PIMPLE (or explicit) time loop. Construct one with `new(mesh, control,
  schemes, solution)`, set the field state, then call `step()` or `run()`.
- [`io`] — readers for `constant/polyMesh` and `0/<field>` files, plus typed
  `controlDict` / `fvSchemes` / `fvSolution` structs.
- [`turbulence`] — pick a closure for a solver run.
- [`prelude`] — one `use` that pulls in the commonly needed public items.
- `tutorials/` — runnable end-to-end cases; the intended entry point for a
  reader new to the crate.

## Maturity — read before depending on this

This is an early (0.1.0), in-progress crate and its surface is uneven:
some paths are validated against published benchmarks, others are
unexercised, and several are `todo!()`. The **`README.md` "Limitations"
section is the authoritative per-module status** and is deliberately
detailed. Two consequences bite immediately:

- **No OpenFOAM dictionary parsing.** [`io::control_dict::ControlDict::read`],
  [`io::fv_schemes::FvSchemes::read`] and
  [`io::fv_solution::FvSolution::read`] are `todo!()`. Configure a case by
  constructing the structs in Rust (`Default::default()` plus field
  assignment), not by reading `system/…` from disk.
- **No field output.** Every writer in [`io::output`] is `todo!()`, so a
  solver run leaves its results in memory only — read them off the solver's
  public field members.

Per the workspace `RESPONSIBLE_USE.md`, nothing here is for reactor
operation, control, licensing, or any safety-critical or operational use.

## Modules

## Module `error`

The crate's single error type, [`error::AppBuilderError`].
# `error` — the crate's single error type

Every fallible public function in `outram-foam-appbuilder-lib` returns
[`AppBuilderError`], so a caller matches one enum across case-file parsing
and the solver time loops rather than juggling a per-module error type.

Variants that carry a file path or line number always refer to the OpenFOAM
case file being read; variants carrying a residual or iteration count come
from a solver loop.

Note that the *unimplemented* parts of this crate (the `todo!()` dictionary
readers and field writers — see the crate-root docs) **panic** rather than
returning an error variant. `AppBuilderError` reports genuine runtime
failures, not missing features.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `AppBuilderError`

Errors returned by this crate's case I/O and solver-loop entry points.

Every fallible public function in `outram-foam-appbuilder-lib` reports
through this single enum, so a caller matches one error type across mesh/
dictionary parsing and the time-advancement loops.

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
    UnsupportedScheme {
        family: &'static str,
        scheme: String,
        reason: &'static str,
    },
}
```

##### Variants

###### `Io`

An OS-level I/O failure while reading a case file; `path` is the file
that could not be read and `source` is the underlying [`std::io::Error`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `path` | `std::path::PathBuf` |  |
| `source` | `std::io::Error` |  |

###### `Parse`

A syntactic error in an OpenFOAM dictionary or field file. `file` and
`line` locate the offending token (1-based line number) and `msg`
describes what was expected.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `file` | `String` |  |
| `line` | `usize` |  |
| `msg` | `String` |  |

###### `MissingKey`

A required dictionary entry was absent: `key` is the missing keyword and
`dict` names the dictionary (e.g. `controlDict`) it was expected in.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `&'static str` |  |
| `dict` | `&'static str` |  |

###### `Diverged`

The linear/nonlinear solve failed to converge: `iter` iterations were
taken and `residual` is the (dimensionless) residual reached at bail-out.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iter` | `usize` |  |
| `residual` | `f64` |  |

###### `TimeLimitReached`

The time loop reached its configured end time `t` (seconds). Returned as
a normal stop signal, not a physics failure.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t` | `f64` |  |

###### `UnsupportedScheme`

An `fvSchemes` selection was parsed and understood, but the solver layer
has no discretisation for it yet. `family` is the dictionary sub-entry
(e.g. `"ddtSchemes"`), `scheme` the requested keyword, and `reason` says
what is missing.

This is deliberately an error rather than a silent fallback to a default
scheme: a scheme selection that is quietly discarded reads as a promise
the solver does not keep.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `family` | `&'static str` |  |
| `scheme` | `String` |  |
| `reason` | `&'static str` |  |

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
## Module `genfoam`

GeN-Foam reactor-multiphysics port (neutronics + TH + thermo-mechanics).
See `docs/genfoam-port-plan.md` for the module map and translation order.
# `genfoam` — Rust port of GeN-Foam's reactor-multiphysics solver

GeN-Foam (Generalized Nuclear Foam) is an OpenFOAM-based multiphysics solver
for nuclear reactors, coupling **neutronics** (point-kinetics, multigroup
diffusion, SP3, SN), **thermal-hydraulics** (single- and two-phase), and
**thermo-mechanics** (thermal-expansion feedback) across multiple meshes.

This subtree is OUTRAM PARK's independent Rust translation of GeN-Foam
(upstream commit `652b3da`, GPL-3.0). It is kept cleanly separated from this
crate's hand-written OpenFOAM solver ports (`crate::solvers`) and case I/O
(`crate::io`).

## What belongs here

Reactor-physics models specific to GeN-Foam: the neutronics models and their
reactivity-feedback machinery, the multi-region coupling that maps fields
between the neutronics / TH / TM meshes, and the reactor-specific
thermal-hydraulics extensions. The generic FV building blocks these rest on
(tensors, `FvMesh`, fields, `SquareMatrix`, `fvm`/`fvc` operators) come from
[`outram_foam_basic_lib`] and are **not** re-ported here.

## What does not belong here

Cross-section generation and Monte Carlo transport. GeN-Foam's neutronics is
deterministic and self-contained; it reads GeN-Foam-format `nuclearData`
dictionaries. It does **not** depend on `njoy-outram-park-fork` or
`outram-mc-libs`.

## Port status

This is an incremental, multi-session port. See
`docs/genfoam-port-plan.md` for the full module map and translation order.
Each submodule's own `//!` header states its precise status; in summary:

- [`neutronics`] — point-kinetics (0-D), multigroup diffusion, SP3, and S_N
  discrete-ordinates eigenvalue/transient solvers are implemented, along
  with the shared cross-section ([`neutronics::xs`]) and flux/power
  ([`neutronics::state`]) data structures.
- [`thermo_mechanics`] — the linear-elastic thermal-stress constitutive core
  and the full displacement/heat field solve on the mechanics mesh are
  implemented.
- [`multi_region`] — the mesh-to-mesh mapping, coupling-field registry, and
  the tightly-coupled Picard outer iteration are implemented (with some
  scaffolded gaps noted in that module's header).
- [`thermal_hydraulics`] — the phase/structure field state, the one-phase
  porous solver driver, all six closure families, the TH boundary
  conditions, the diagnostic function objects and the bespoke hydrogen
  thermophysical package are implemented with unit tests. The two-phase
  (MULES) solver, `onePhaseLegacy` and the `nusselt_baffle` boundary
  condition remain unported. See that module's header for the per-sub-module
  breakdown and the full gap list.

The generic FV building blocks ([`common`]) round out the subtree.

```rust
pub mod genfoam { /* ... */ }
```

### Modules

## Module `common`

# `genfoam::common` — shared multiphysics utilities

Rust port of `GeN-Foam/src/classes/common` (~3.7k LOC): the base helpers the
neutronics, thermal-hydraulics, and thermo-mechanics regions build on. It is
the **foundation** the other `genfoam` modules code against, so its surface
is kept small, dimensionally honest, and thoroughly documented.

Generic FV building blocks (tensors, `FvMesh`, fields, `SquareMatrix`,
`fvm`/`fvc`) come from [`outram_foam_basic_lib`] and are **not** re-ported
here.

## Module map — what lives here

| Submodule | Ports | Role |
|---|---|---|
| [`time_profile`] | `common/timeProfile` | A simulation input as a function of time (external reactivity `ρ(t)`, source power `S(t)`, boron ramps): [`TimeProfile`]. |
| [`interpolate_table`] | `common/InterpolateTable` (scalar 1-D) | 1-D lookup table with step/linear method + error/clamp/extrapolate out-of-bounds policy and an integral: [`ScalarInterpolateTable`]. |
| [`rbf`] | `common/radialBasisFunctionInterpolation` | N-dimensional polyharmonic-spline radial-basis-function interpolation. Shared by the neutronics cross-section parametrisation ([`crate::genfoam::neutronics::xs`]) and the non-conformal mesh mapping ([`crate::genfoam::multi_region::rbf_mapping`]). |

Both are dimensionally honest: their **time abscissa** is a `uom`
[`Time`](uom::si::f64::Time), but a tabulated **ordinate is a raw `f64`**
because the same generic table serves reactivity (dimensionless), power (W),
and concentration (mol/m³) consumers — the consumer attaches the ordinate's
unit at its own boundary. Forcing one `uom` unit here would be physically
wrong. See each submodule's docs.

## Deferred / folded-into-caller (not ported here)

Per `docs/genfoam-port-plan.md`, the remaining `common/` files are either
mesh-topology helpers that only the (not-yet-ported) `multi_region` layer
needs, or thin things that fold into their caller:

- `common/solver` — the abstract run-time-selectable **region-solver base
  class** (the PIMPLE region-solver interface). This is Layer-5 solver-loop
  logic; it becomes the `NeutronicsModel` / TH-solver **enum dispatch** in
  those modules, not a `common` helper. **Deferred** to the TH port.
- `common/listOperation` — `stringify` list-to-`word` conversion for
  dictionary I/O. Folds into the case-I/O layer; **not needed** by the
  physics core.
- `common/latticeMap`, `common/mergeOrSplitBaffles` — mesh-topology /
  mesh-to-mesh helpers used only by multi-region coupling. **Deferred** to
  `genfoam::multi_region` (tracked under the appbuilder epic `op-p6p`).
- The 2-D/3-D `FieldField` `InterpolateTableGF` instantiations —
  spatial cross-section interpolation. **Deferred** to
  `genfoam::neutronics::xs`, which owns the group-XS containers.

See `docs/genfoam-port-plan.md` for the full translation order.

```rust
pub mod common { /* ... */ }
```

### Modules

## Module `interpolate_table`

# 1-D interpolation table with selectable method and out-of-bounds policy

[`ScalarInterpolateTable`] is a 1-D lookup table over strictly-ascending
abscissae `x` returning a scalar `y` — GeN-Foam's `scalarInterpolateTableGF`
(the `InterpolateTableGF<scalarField, scalar, scalar>` instantiation). It is
richer than [`interpolate_xy`](fn@outram_foam_basic_lib::interpolation::interpolate_xy),
which offers only linear interpolation with endpoint clamping, in two ways:

1. **Interpolation method** — [`InterpolationMethod::Linear`] *or*
   [`InterpolationMethod::Step`] (each tabulated value holds until the next
   knot; a staircase, used e.g. for banded/tabulated cross sections).
2. **Out-of-bounds policy** — [`OutOfBounds::Error`] (reject),
   [`OutOfBounds::Fixed`] (clamp to the endpoint, the `interpolate_xy`
   behaviour), or [`OutOfBounds::Extrapolation`] (linear extrapolation from
   the two nearest points).

It also provides an [`integral`](ScalarInterpolateTable::integral) with a
coordinate-power weight (cartesian/cylindrical/spherical) and the table-walk
helpers [`next_point`](ScalarInterpolateTable::next_point) /
[`last_point`](ScalarInterpolateTable::last_point).

## Dimension convention

Both axes are **raw `f64`**, because this is a generic numeric utility whose
axes carry different physics per use site (cross section vs temperature,
power vs radius, …); pinning a single `uom` unit would be physically wrong.
The consumer attaches units at its own boundary. (This matches the upstream
`scalar`-typed table and mirrors the same decision made for
[`TimeProfile`](super::time_profile::TimeProfile)'s ordinate.)

## Error handling — no panics

Faithful to the workspace rule against papering over non-convergence, every
fallible query returns a [`Result`]; nothing panics on bad input. Upstream
`FatalError`s (out-of-bounds under [`OutOfBounds::Error`], malformed tables)
become [`InterpolateTableError`] values the caller must handle.

## Example

```
use outram_foam_appbuilder_lib::genfoam::common::interpolate_table::{
    InterpolationMethod, OutOfBounds, ScalarInterpolateTable,
};

let table = ScalarInterpolateTable::new(
    vec![0.0, 1.0, 2.0],
    vec![0.0, 10.0, 30.0],
    InterpolationMethod::Linear,
    OutOfBounds::Fixed,
)
.unwrap();

assert_eq!(table.interpolate(0.5).unwrap(), 5.0);   // linear between knots
assert_eq!(table.interpolate(-1.0).unwrap(), 0.0);  // clamped to first value
assert_eq!(table.interpolate(9.0).unwrap(), 30.0);  // clamped to last value
```

```rust
pub mod interpolate_table { /* ... */ }
```

### Types

#### Enum `InterpolationMethod`

How a [`ScalarInterpolateTable`] interpolates between tabulated knots.

Maps to GeN-Foam's `InterpolateTableBaseGF::interpolationMethod`.

```rust
pub enum InterpolationMethod {
    Step,
    Linear,
}
```

##### Variants

###### `Step`

Each tabulated value applies from its knot until the next knot — a
staircase (upstream `STEP`). For `x` in `[x[i-1], x[i])` the value is
`y[i-1]`.

###### `Linear`

Linear interpolation between adjacent knots (upstream `LINEAR`).

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
    fn clone(self: &Self) -> InterpolationMethod { /* ... */ }
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
    fn eq(self: &Self, other: &InterpolationMethod) -> bool { /* ... */ }
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
#### Enum `OutOfBounds`

What a [`ScalarInterpolateTable`] does when queried outside its tabulated
range `[x[0], x[n-1]]`.

Maps to GeN-Foam's `InterpolateTableBaseGF::outofBoundsMehtod` (sic).

```rust
pub enum OutOfBounds {
    Error,
    Extrapolation,
    Fixed,
}
```

##### Variants

###### `Error`

Reject the query — [`ScalarInterpolateTable::interpolate`] returns
[`InterpolateTableError::OutOfBounds`] (upstream `ERROR`, a `FatalError`).

###### `Extrapolation`

Linearly extrapolate using the two nearest tabulated points
(upstream `EXTRAPOLATION`).

###### `Fixed`

Clamp to the nearest endpoint value (upstream `FIXED`; the same behaviour
as [`interpolate_xy`](fn@outram_foam_basic_lib::interpolation::interpolate_xy)).

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
    fn clone(self: &Self) -> OutOfBounds { /* ... */ }
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
    fn eq(self: &Self, other: &OutOfBounds) -> bool { /* ... */ }
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
#### Enum `InterpolateTableError`

Errors from building or querying a [`ScalarInterpolateTable`].

```rust
pub enum InterpolateTableError {
    LengthMismatch {
        xs: usize,
        ys: usize,
    },
    TooFewPoints(usize),
    NonAscendingAbscissae {
        index: usize,
        prev: f64,
        next: f64,
    },
    OutOfBounds {
        x: f64,
        lo: f64,
        hi: f64,
    },
}
```

##### Variants

###### `LengthMismatch`

The `x` and `y` vectors had different lengths.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `xs` | `usize` | Number of abscissae supplied. |
| `ys` | `usize` | Number of ordinate values supplied. |

###### `TooFewPoints`

A table needs at least two points to interpolate between.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `NonAscendingAbscissae`

The abscissa column was not strictly ascending. The reported index `i` is
the first offender (`x[i] <= x[i-1]`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Index of the first non-ascending entry. |
| `prev` | `f64` | The preceding abscissa. |
| `next` | `f64` | The offending abscissa. |

###### `OutOfBounds`

A query fell outside `[x[0], x[n-1]]` under the [`OutOfBounds::Error`]
policy.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` | The queried abscissa. |
| `lo` | `f64` | The table's lower bound `x[0]`. |
| `hi` | `f64` | The table's upper bound `x[n-1]`. |

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
    fn clone(self: &Self) -> InterpolateTableError { /* ... */ }
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
    fn from(source: InterpolateTableError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &InterpolateTableError) -> bool { /* ... */ }
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
#### Struct `ScalarInterpolateTable`

A 1-D interpolation table over strictly-ascending abscissae.

See the [module documentation](self) for the interpolation methods,
out-of-bounds policies, dimension convention (both axes are raw `f64`), and
the no-panic error policy. Build with [`new`](Self::new); query with
[`interpolate`](Self::interpolate).

```rust
pub struct ScalarInterpolateTable {
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
  pub fn new(xs: Vec<f64>, ys: Vec<f64>, method: InterpolationMethod, out_of_bounds: OutOfBounds) -> Result<Self, InterpolateTableError> { /* ... */ }
  ```
  Build a table from tabulated `(xs[i], ys[i])` points.

- ```rust
  pub fn method(self: &Self) -> InterpolationMethod { /* ... */ }
  ```
  The configured interpolation method.

- ```rust
  pub fn out_of_bounds(self: &Self) -> OutOfBounds { /* ... */ }
  ```
  The configured out-of-bounds policy.

- ```rust
  pub fn interpolate(self: &Self, x: f64) -> Result<f64, InterpolateTableError> { /* ... */ }
  ```
  Interpolate the table at `x`.

- ```rust
  pub fn integral(self: &Self, k: f64) -> f64 { /* ... */ }
  ```
  The integral of the tabulated data, `∫ y · d(x^k)`, with coordinate power

- ```rust
  pub fn next_point(self: &Self, x: f64) -> Result<f64, InterpolateTableError> { /* ... */ }
  ```
  The next tabulated abscissa strictly greater than `x` (the upstream

- ```rust
  pub fn last_point(self: &Self) -> f64 { /* ... */ }
  ```
  The last (largest) tabulated abscissa `xs[n-1]` (upstream `getLastPoint`).

- ```rust
  pub fn first_point(self: &Self) -> f64 { /* ... */ }
  ```
  The first (smallest) tabulated abscissa `xs[0]`.

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
    fn clone(self: &Self) -> ScalarInterpolateTable { /* ... */ }
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
    fn eq(self: &Self, other: &ScalarInterpolateTable) -> bool { /* ... */ }
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
## Module `rbf`

# Polyharmonic-spline radial-basis-function interpolation (N-dimensional)

This is the numerical kernel behind GeN-Foam's cross-section
parametrisation: given a handful of *reference states* (each a point in a
multi-parameter feedback space — fuel temperature, coolant density, axial /
radial expansion, ...) with a known cross-section value at each, it builds a
smooth interpolant that reproduces every state exactly and interpolates in
between.

## The interpolant

For `N` data points `c_i` in a `p`-dimensional parameter space with values
`sigma_i`, the polyharmonic spline is

```text
  f(x) = sum_i  w_i phi(||x - c_i||)  +  v_0  +  sum_k v_k x_k
```

with radial basis `phi` selected by `mode` (see
[`polyharmonic_spline_function`]) and a linear polynomial tail `v`. The
weights `(w, v)` solve the symmetric saddle-point system

```text
  [ A   B ] [ w ]   [ sigma ]
  [ B^T 0 ] [ v ] = [ 0     ]
```

where `A_{ij} = phi(||c_i - c_j||)`, and `B` stacks a column of ones with the
data coordinates. The orthogonality rows `B^T w = 0` make the polynomial tail
well-posed.

## Provenance

GeN-Foam keeps these functions in `common/radialBasisFunctionInterpolation`,
which the port plan maps to `genfoam::common` — hence this module's home.
It ports the two overloads the neutronics cross-section parametrisation and
the `multi_region` non-conformal RBF mapping both call (the N-dimensional
`solve` + evaluator, plus the shared basis function). It is a faithful
port of the upstream algorithm — a saddle-point solve via
`outram_foam_basic_lib::matrix::SquareMatrix` (Crout LU with partial
pivoting), the direct analogue of upstream's `LUscalarMatrix::inv`.

Both consumers —
[`crate::genfoam::neutronics::xs::nuclear_data_one_energy`] and
[`crate::genfoam::multi_region::rbf_mapping`] — share this single kernel.

```rust
pub mod rbf { /* ... */ }
```

### Functions

#### Function `polyharmonic_spline_function`

**Attributes:**

- `MustUse { reason: None }`

Polyharmonic radial basis function `phi(r)` evaluated from `r^2`.

`r_square` is the squared Euclidean distance `||x - c||^2` in parameter
space; `mode` selects the spline order (GeN-Foam
`polyharmonicSplineMode`, default `1`):

- `1`: `phi = r`            (linear — reproduces linear interpolation in 1-D)
- `2`: `phi = r^2 ln(r)`    (thin-plate spline)
- `3`: `phi = r^3`
- `4`: `phi = r^4 ln(r)`

Any other `mode` returns `0.0` (matching upstream's out-of-range fallback).
Note `phi(0) = 0` in every mode, so the diagonal of `A` is zero.

```rust
pub fn polyharmonic_spline_function(r_square: f64, mode: usize) -> f64 { /* ... */ }
```

#### Function `solve_polyharmonic_spline`

Solve for the polyharmonic-spline weights `[w | v]` of an N-dimensional data
set.

`x_list` is the transposed coordinate table: `x_list[param][data]` is the
value of parameter `param` at data point `data`. Every inner list must have
the same length `n` (the number of data points). `v_list` holds the `n`
data values `sigma_i`. `mode` selects the basis (see
[`polyharmonic_spline_function`]).

Returns the weight vector of length `n + p + 1` (`p = x_list.len()`), laid
out as `[w_0..w_{n-1}, v_0, v_1..v_p]` — the `w_i` multiply the radial terms,
`v_0` is the constant tail, and `v_1..v_p` the per-parameter linear tail —
exactly the layout [`polyharmonic_spline`] expects.

# Errors

Returns [`MatrixError::Singular`] if the saddle-point matrix is singular
(e.g. duplicate data points). The error is propagated, never defaulted.

# Panics

Panics if `x_list` is empty or its rows have unequal length; callers
(`nuclearDataOneEnergy::build`) guarantee a rectangular, non-empty table.

```rust
pub fn solve_polyharmonic_spline(x_list: &[Vec<f64>], v_list: &[f64], mode: usize) -> Result<Vec<f64>, outram_foam_basic_lib::matrix::MatrixError> { /* ... */ }
```

#### Function `polyharmonic_spline`

**Attributes:**

- `MustUse { reason: None }`

Evaluate the polyharmonic spline at an arbitrary parameter point `x_input`.

`weights` is the vector returned by [`solve_polyharmonic_spline`],
`x_list[param][data]` the same transposed coordinate table used to build it,
and `x_input[param]` the query point (length `p = x_list.len()`). `mode` must
match the one used at solve time.

Returns the interpolated scalar `f(x_input)`.

# Panics

Panics if `x_list` is empty; callers guarantee at least one data point.

```rust
pub fn polyharmonic_spline(weights: &[f64], x_list: &[Vec<f64>], x_input: &[f64], mode: usize) -> f64 { /* ... */ }
```

## Module `time_profile`

# Time profiles — time-tabulated scalar simulation inputs

A [`TimeProfile`] is the value of one simulation input as a function of
**time** — GeN-Foam's `timeProfile` (a thin wrapper over OpenFOAM's
`Function1<scalar>`). It is how a transient case prescribes a time-dependent
driver that is *not* computed from the fields, for example:

- the **external reactivity** `ρ(t)` injected into a point-kinetics run
  (a control-rod ramp, a step reactivity insertion) — dimensionless `Δk/k`,
- an **external neutron-source power** `S(t)` (watts),
- a **boron concentration** or coolant-inlet boundary value ramp.

## Dimension convention (read this before use)

The **abscissa is always time**, so it is typed as [`Time`] (seconds).

The **ordinate is a raw `f64`** whose physical meaning is fixed by the
*consumer*, not by this type. This mirrors the upstream `Function1<scalar>`,
which is deliberately dimension-agnostic: the same machinery tabulates
reactivity (dimensionless), source power (W), and concentration (mol/m³).
Attaching a single `uom` unit here would be physically wrong for a shared
helper. Consumers must document and attach the ordinate's unit at their own
boundary — e.g. point-kinetics reads a reactivity profile as a
[`uom::si::f64::Ratio`] and a source profile as a [`uom::si::f64::Power`].

## Start-time offset

Every profile carries a `start_time` offset `t_0` (default `0 s`, faithful to
GeN-Foam's `startTime`). A query at wall-clock time `t` is evaluated at the
*shifted* time `t − t_0`, so a table authored from `t = 0` can be delayed to
begin at `t_0` without re-tabulating it. Before `t_0` the table clamps to its
first value (see [`TimeProfile::value`]).

## What is intentionally not ported

GeN-Foam's `timeProfile` can also be driven by an **FMI** port
(`type fmi`), reading the value each step from a co-simulation
`commDataLayer`. That is live external-system coupling — out of scope for
this offline library and excluded by the workspace responsible-use policy —
so it is **not** ported. Only the self-contained table/constant profiles are.

## Example

```
use outram_foam_appbuilder_lib::genfoam::common::time_profile::TimeProfile;
use uom::si::f64::Time;
use uom::si::time::second;

// A 57.67 pcm (0.2 β) step reactivity that switches on at t = 100 s:
// the table is authored from t = 0, then delayed by the start-time offset.
let rho = TimeProfile::table(
    vec![Time::new::<second>(0.0), Time::new::<second>(1.0)],
    vec![0.0, 0.0005767131],
)
.unwrap()
.with_start_time(Time::new::<second>(100.0));

assert_eq!(rho.value(Time::new::<second>(50.0)), 0.0);          // before start
assert_eq!(rho.value(Time::new::<second>(100.0)), 0.0);         // at start (shifted t = 0)
assert_eq!(rho.value(Time::new::<second>(101.0)), 0.0005767131);// shifted t = 1 s
```

```rust
pub mod time_profile { /* ... */ }
```

### Types

#### Enum `TimeProfileError`

Errors returned when constructing a [`TimeProfile`] from a table.

```rust
pub enum TimeProfileError {
    LengthMismatch {
        times: usize,
        values: usize,
    },
    TooFewPoints(usize),
    NonAscendingTimes {
        index: usize,
        prev: f64,
        next: f64,
    },
}
```

##### Variants

###### `LengthMismatch`

The `times` and `values` vectors had different lengths.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `times` | `usize` | Number of abscissa (time) entries supplied. |
| `values` | `usize` | Number of ordinate (value) entries supplied. |

###### `TooFewPoints`

A table needs at least two points to interpolate between.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `NonAscendingTimes`

The abscissa (time) column was not strictly ascending. Interpolation
requires a strictly increasing time axis; equal or decreasing times are
rejected. The reported index `i` is the first offender (`times[i] <=
times[i-1]`), in seconds.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Index of the first non-ascending entry. |
| `prev` | `f64` | The preceding time, in seconds. |
| `next` | `f64` | The offending time, in seconds. |

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
    fn clone(self: &Self) -> TimeProfileError { /* ... */ }
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
    fn eq(self: &Self, other: &TimeProfileError) -> bool { /* ... */ }
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
#### Struct `TimeProfile`

A single simulation input tabulated against **time**.

See the [module documentation](self) for the dimension convention (abscissa
is [`Time`]; the ordinate is a raw `f64` whose unit the consumer fixes) and
the start-time offset semantics.

Construct one with [`TimeProfile::constant`] or [`TimeProfile::table`], then
optionally shift it with [`TimeProfile::with_start_time`]. Query it with
[`TimeProfile::value`].

```rust
pub struct TimeProfile {
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
  pub fn constant(value: f64) -> Self { /* ... */ }
  ```
  A profile that returns `value` at every time — the direct analogue of an

- ```rust
  pub fn table(times: Vec<Time>, values: Vec<f64>) -> Result<Self, TimeProfileError> { /* ... */ }
  ```
  A piecewise-linear profile through the tabulated points

- ```rust
  pub fn with_start_time(self: Self, start_time: Time) -> Self { /* ... */ }
  ```
  Set the start-time offset `t_0`: subsequent [`value`](Self::value) queries

- ```rust
  pub fn start_time(self: &Self) -> Time { /* ... */ }
  ```
  The start-time offset `t_0` as a [`Time`].

- ```rust
  pub fn valid(self: &Self) -> bool { /* ... */ }
  ```
  Whether this profile can be evaluated. Always `true` for the ported

- ```rust
  pub fn value(self: &Self, t: Time) -> f64 { /* ... */ }
  ```
  The value of the profile at wall-clock time `t`, evaluated at the shifted

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
    fn clone(self: &Self) -> TimeProfile { /* ... */ }
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
    fn eq(self: &Self, other: &TimeProfile) -> bool { /* ... */ }
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
### Re-exports

#### Re-export `InterpolateTableError`

```rust
pub use interpolate_table::InterpolateTableError;
```

#### Re-export `InterpolationMethod`

```rust
pub use interpolate_table::InterpolationMethod;
```

#### Re-export `OutOfBounds`

```rust
pub use interpolate_table::OutOfBounds;
```

#### Re-export `ScalarInterpolateTable`

```rust
pub use interpolate_table::ScalarInterpolateTable;
```

#### Re-export `TimeProfile`

```rust
pub use time_profile::TimeProfile;
```

#### Re-export `TimeProfileError`

```rust
pub use time_profile::TimeProfileError;
```

## Module `multi_region`

# `genfoam::multi_region` — multi-mesh coupling

Rust port of `GeN-Foam/src/classes/multiRegion` (~2.4k LOC): the layer that
couples the separate neutronics / thermal-hydraulics / thermo-mechanics
meshes — mapping fields between them (`meshToMesh` / radial-basis-function
interpolation), assembling the coupled feedback (fuel/coolant temperature,
density, power density, mesh displacement), and driving the outer coupling
iteration.

Generic FV building blocks come from [`outram_foam_basic_lib`]; the physics
models being coupled live in [`super::neutronics`],
[`super::thermo_mechanics`], and the thermal-hydraulics module.

## Module map — read this first

| Submodule | Ports (upstream) | Role |
|---|---|---|
| [`mesh_to_mesh`] | `meshHandler` `meshToMesh` addressing + `map`/`mapTgtToSrc` | Volumetric cell-to-cell field transfer ([`MeshToMesh`]): nearest-cell, inverse-distance, and (approximate) conservative cell-volume-weighted mapping between overlapping region meshes. |
| [`rbf_mapping`] | `meshHandler::interpolateAndMapFields` (polyharmonic-spline path) | Radial-basis-function mapping ([`RbfFieldMap`]) for **non-conformal** meshes that do not volume-overlap — fits a polyharmonic spline to scattered samples and evaluates it on the target mesh. |
| [`coupling_fields`] | `meshHandler` region registry + `mappings` / `mapAllFields` | The [`MeshHandler`]: region meshes, their named coupling fields, and the pairwise mappings; [`MeshHandler::interpolate_coupling_fields`] is `interpolateCouplingFields`. |
| [`outer_iteration`] | `multiPhysicsSolver::correctPhysics` | The tightly-coupled Picard loop ([`MultiPhysicsSolver`]) advancing the regions and exchanging feedback to convergence, enum-dispatched over [`RegionModel`] (no `dyn`). |
| [`mesh_region`] | region-solver dispatch → `diffusionNeutronics` / `thermalHydraulics` | The **mesh-based** [`RegionModel`] variants: [`MeshNeutronics`] drives the real multigroup-diffusion solver with cross-section temperature feedback; [`MeshThermalHydraulics`] is the per-cell energy-balance seam for the full TH solver. |
| [`reactivity_feedback`] | `pointKineticNeutronics` feedback assembly | The [`ReactivityFeedback`] layer: turns the mesh temperature / density feedback fields into a scalar reactivity `Δρ` (Doppler + expansion + coolant density), generalising the lumped `α(T−T_ref)` to spatial fields. |

**Start with an example**, not the API: the loop wired end-to-end (0-D
neutronics ↔ lumped thermal-hydraulics, using the already-ported
point-kinetics) lives in [`outer_iteration`]'s `tests` — it constructs a
[`MeshHandler`], registers two regions and their mappings, and drives
[`MultiPhysicsSolver::solve`] over a transient.

## Coupling fields exchanged

`TFuel`, `TStruct`, coolant density, `powerDensityNeutronics`, and `meshDisp`
— pulled onto the meshes that consume them; see [`coupling_fields`].

## RBF kernel reuse

[`rbf_mapping`] reuses the shared polyharmonic-spline kernel at
[`crate::genfoam::common::rbf`] rather than adding a second copy — the same
kernel also backs cross-section parametrisation in
[`crate::genfoam::neutronics::xs`]. See [`rbf_mapping`]'s docs.

## Scaffolded gaps (missing basic-lib mesh machinery)

Two pieces are intentionally scaffolded (documented interface + a degraded or
deferred implementation) because [`outram_foam_basic_lib`] does not yet expose
the geometry they need — tracked as sub-beads of `op-p6p.8`:

- **Exact conservative `meshToMesh` (`imCellVolumeWeight`).** Upstream
  integrates true polyhedral cell-overlap volumes (a supermesh intersection);
  basic-lib has cell centres/volumes but no mesh-intersection/clipping. The
  port uses nearest-cell addressing plus a global integral rescale — globally
  conservative, but not the exact local overlap distribution. Exact overlap
  awaits a basic-lib supermesh operator.
- **`deformMesh` / `movePoints`.** Applying `meshDisp` to actually move mesh
  points (upstream `deformMesh`) needs mutable mesh-point geometry on
  `FvMesh`, which basic-lib does not expose. The loop plumbs the displacement
  field through the coupling but does not yet move points.

Neither hack was applied to work around these — the mapping is honest about
its fidelity and the loop is honest about not moving points.

**Port status:** field mapping (volumetric + RBF), coupled-feedback assembly,
and the outer Picard loop are ported and verified end-to-end against both the
0-D neutronics ↔ lumped-TH coupling *and* the **mesh-based** path: the
[`MeshNeutronics`] variant drives the real multigroup-diffusion solver
([`crate::genfoam::neutronics::DiffusionNeutronics`]) with cross-section
(Doppler) temperature feedback through the [`reactivity_feedback`] layer, and
[`MeshThermalHydraulics`] closes the loop across non-conformal meshes (V&V:
power-density conservation across the `CellVolumeWeight` map and negative
Doppler feedback lowering `k_eff` — see [`mesh_region`]'s tests). The full
porous/two-phase TH solver (`op-p6p.7`) drops into the same
[`MeshThermalHydraulics`] seam when it lands; per-cell (rather than mean)
cross-section feedback awaits a neutronics-subtree API addition (tracked on
`op-p6p.8.4`). See `docs/genfoam-port-plan.md`.

```rust
pub mod multi_region { /* ... */ }
```

### Modules

## Module `coupling_fields`

# Coupled-feedback field assembly (`interpolateCouplingFields`)

Rust port of GeN-Foam's `meshHandler`: the registry of region meshes and the
`mappings` specification that pulls the coupled-feedback fields onto the
meshes that consume them. In a reactor multiphysics run the fields exchanged
each coupling step are:

| Field | Produced on | Consumed on |
|---|---|---|
| `TFuel` (fuel temperature) | thermal-hydraulics | neutronics (Doppler XS feedback) |
| `TStruct` (structure temperature) | thermal-hydraulics / thermo-mechanics | neutronics, thermo-mechanics |
| coolant density `rho` | thermal-hydraulics | neutronics (moderator/coolant feedback) |
| `powerDensityNeutronics` | neutronics | thermal-hydraulics (volumetric heat source) |
| `meshDisp` (mesh displacement) | thermo-mechanics | neutronics, thermal-hydraulics (geometry feedback) |

Each region lives on its own mesh; the [`MeshHandler`] holds those meshes,
the named fields on them, and the pairwise [`MeshToMesh`] mappings that move a
source field onto a target field. [`MeshHandler::interpolate_coupling_fields`]
executes the whole `mappings` spec in one call — the direct analogue of
upstream `mapAllFields`; [`MeshHandler::interpolate_these`] restricts it to a
named subset (upstream `mapTheseFields`, called inside the outer loop so only
the actively-iterated regions re-exchange).

Field values are unit-agnostic raw `f64` / [`Vector3`] cell arrays (matching
[`outram_foam_basic_lib`] fields); the physical `uom` interpretation is
attached where the values are *used* — see [`super::outer_iteration`].

```rust
pub mod coupling_fields { /* ... */ }
```

### Types

#### Enum `CouplingError`

Errors from building or running the coupling-field registry.

```rust
pub enum CouplingError {
    UnknownRegion(String),
    DuplicateRegion(String),
    MissingField {
        region: String,
        field: String,
    },
}
```

##### Variants

###### `UnknownRegion`

A region name was referenced that is not registered.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `DuplicateRegion`

A region with this name was already registered.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `MissingField`

A mapping referenced a field absent from a region.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `region` | `String` | The region the field was expected on. |
| `field` | `String` | The missing field's name. |

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
    fn clone(self: &Self) -> CouplingError { /* ... */ }
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

- **Eq**
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
    fn eq(self: &Self, other: &CouplingError) -> bool { /* ... */ }
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
#### Enum `FieldKind`

Whether a coupling field is scalar or vector valued.

```rust
pub enum FieldKind {
    Scalar,
    Vector,
}
```

##### Variants

###### `Scalar`

A [`VolScalarField`] (temperature, density, power density).

###### `Vector`

A [`VolVectorField`] (mesh displacement, velocity).

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
    fn clone(self: &Self) -> FieldKind { /* ... */ }
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
    fn eq(self: &Self, other: &FieldKind) -> bool { /* ... */ }
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
#### Struct `CouplingField`

One entry of a region-to-region mapping: which source field feeds which
target field, its kind, and how it combines with the target's contents.

Upstream this is one aligned `(sourceFields, targetFields)` pair in the
`mappings` sub-dictionary; the combine mode reflects upstream's `plusEqOp`
accumulation (use [`MapCombine::Accumulate`] when several source regions sum
into one target field, [`MapCombine::Replace`] for a single clean transfer).

```rust
pub struct CouplingField {
    pub source: String,
    pub target: String,
    pub kind: FieldKind,
    pub combine: super::mesh_to_mesh::MapCombine,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source` | `String` | Field name on the source region. |
| `target` | `String` | Field name on the target region. |
| `kind` | `FieldKind` | Scalar or vector. |
| `combine` | `super::mesh_to_mesh::MapCombine` | How the mapped value combines with the target's current value. |

##### Implementations

###### Methods

- ```rust
  pub fn scalar(source: &str, target: &str) -> Self { /* ... */ }
  ```
  A scalar `source → target` transfer that replaces the target value.

- ```rust
  pub fn vector(source: &str, target: &str) -> Self { /* ... */ }
  ```
  A vector `source → target` transfer that replaces the target value.

- ```rust
  pub fn accumulating(self: Self) -> Self { /* ... */ }
  ```
  Set the combine mode to [`MapCombine::Accumulate`] (upstream `plusEqOp`).

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
    fn clone(self: &Self) -> CouplingField { /* ... */ }
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
#### Struct `CouplingRegion`

A physics region: a named mesh and the coupling fields that live on it.

The neutronics, thermal-hydraulics and thermo-mechanics regions are each one
of these. It owns its mesh (`Arc<FvMesh>`, shared with the mappers) and its
coupling fields keyed by name.

```rust
pub struct CouplingRegion {
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
  pub fn new(name: &str, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Create an empty region named `name` on `mesh`.

- ```rust
  pub fn name(self: &Self) -> &str { /* ... */ }
  ```
  The region name.

- ```rust
  pub fn mesh(self: &Self) -> &Arc<FvMesh> { /* ... */ }
  ```
  The region mesh.

- ```rust
  pub fn insert_scalar(self: &mut Self, field: VolScalarField) { /* ... */ }
  ```
  Register a scalar coupling field (keyed by the field's own name).

- ```rust
  pub fn insert_vector(self: &mut Self, field: VolVectorField) { /* ... */ }
  ```
  Register a vector coupling field (keyed by the field's own name).

- ```rust
  pub fn scalar(self: &Self, name: &str) -> Option<&VolScalarField> { /* ... */ }
  ```
  Borrow a scalar field by name.

- ```rust
  pub fn scalar_mut(self: &mut Self, name: &str) -> Option<&mut VolScalarField> { /* ... */ }
  ```
  Mutably borrow a scalar field by name.

- ```rust
  pub fn vector(self: &Self, name: &str) -> Option<&VolVectorField> { /* ... */ }
  ```
  Borrow a vector field by name.

- ```rust
  pub fn vector_mut(self: &mut Self, name: &str) -> Option<&mut VolVectorField> { /* ... */ }
  ```
  Mutably borrow a vector field by name.

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
    fn clone(self: &Self) -> CouplingRegion { /* ... */ }
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
#### Struct `MeshHandler`

The multi-region field-mapping hub: region meshes, their coupling fields, and
the pairwise mappings between them.

Register regions with [`MeshHandler::add_region`], wire the `mappings` with
[`MeshHandler::add_mapping`], then call
[`MeshHandler::interpolate_coupling_fields`] each coupling step. This is the
Rust `meshHandler`.

```rust
pub struct MeshHandler {
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
  An empty handler.

- ```rust
  pub fn add_region(self: &mut Self, region: CouplingRegion) -> Result<(), CouplingError> { /* ... */ }
  ```
  Register a region. Its name must be unique.

- ```rust
  pub fn region(self: &Self, name: &str) -> Option<&CouplingRegion> { /* ... */ }
  ```
  Borrow a region by name.

- ```rust
  pub fn region_mut(self: &mut Self, name: &str) -> Option<&mut CouplingRegion> { /* ... */ }
  ```
  Mutably borrow a region by name.

- ```rust
  pub fn add_mapping(self: &mut Self, target: &str, source: &str, method: MappingMethod, fields: Vec<CouplingField>) -> Result<(), CouplingError> { /* ... */ }
  ```
  Register a mapping that transfers `fields` from region `source` onto

- ```rust
  pub fn interpolate_coupling_fields(self: &mut Self) -> Result<(), CouplingError> { /* ... */ }
  ```
  Execute *all* registered mappings — the coupled-feedback assembly

- ```rust
  pub fn interpolate_these(self: &mut Self, region_names: &[&str]) -> Result<(), CouplingError> { /* ... */ }
  ```
  Execute only the mappings whose *both* endpoints are in `region_names`

- ```rust
  pub fn scalar_integral(self: &Self, region: &str, field: &str) -> Result<f64, CouplingError> { /* ... */ }
  ```
  The volume integral `∑ V φ` of a named scalar field on a named region — a

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
    fn clone(self: &Self) -> MeshHandler { /* ... */ }
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
    fn default() -> MeshHandler { /* ... */ }
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
## Module `mesh_region`

# Mesh-based region models (spatial neutronics ↔ thermal-hydraulics)

The mesh-based [`RegionModel`](super::outer_iteration::RegionModel) variants
that drive the *spatial* physics solvers through the multi-region Picard loop
— the piece the `multiRegion` module was scaffolded for. Where the 0-D
[`LumpedNeutronics`](super::outer_iteration::LumpedNeutronics) /
[`LumpedThermal`](super::outer_iteration::LumpedThermal) carry one cell, these
carry a full [`FvMesh`] and exchange **per-cell** fields
(`powerDensityNeutronics`, `TFuel`) through the
[`MeshHandler`](super::coupling_fields::MeshHandler) mapping.

## [`MeshNeutronics`] — real mesh diffusion with temperature feedback

Wraps the ported [`DiffusionNeutronics`] multigroup-diffusion solver
(`op-p6p.6`). Each Picard sub-step it:

1. reads the mapped per-cell fuel temperature `TFuel` from its region;
2. collapses it to a single importance-weighted feedback temperature
   (via [`ReactivityFeedback::weighted_mean`]) and **re-materialises the cross
   sections at that temperature**, so the group constants carry the real
   Doppler dependence encoded in the `nuclearData` states (`op-p6p.10`);
3. re-solves the k-eigenvalue for the fundamental-mode flux/power shape;
4. renormalises to a fixed target total power and writes the per-cell
   volumetric power density `powerDensityNeutronics` back for the mapping.

The feedback is therefore the **physical** cross-section feedback GeN-Foam
uses (hot fuel → altered `nu Sigma_f` / `Sigma_r` → lower `k_eff`), not a
bolted-on reactivity offset. The residual it reports is the relative change in
`k_eff` between Picard iterates.

### Fidelity note — uniform vs. per-cell feedback point

[`DiffusionXsFields::materialize`](crate::genfoam::neutronics::diffusion::DiffusionXsFields::materialize)
takes **one** feedback vector for the whole mesh, so this model feeds it the
importance-weighted mean temperature — a *global* Doppler feedback. Upstream
evaluates the cross sections at each cell's *local* temperature. Per-cell
feedback needs a `materialize` variant that accepts a per-cell parameter
array; that is a neutronics-subtree API addition, tracked as a follow-up on
`op-p6p.8.4` (see the crate handoff). The global-feedback path is real and
sufficient to close the coupled loop and demonstrate negative feedback.

## [`MeshThermalHydraulics`] — minimal per-cell energy balance (seam)

A real but deliberately minimal mesh thermal model: a per-cell backward-Euler
fuel-energy balance `ρc_p dT/dt = q''' − (ρc_p/τ)(T − T_cool)` (the coolant
coupling is parametrised by a thermal time constant `τ` so every input is a
standard named `uom` alias). It reads the mapped `powerDensityNeutronics` and
writes `TFuel`, closing the loop across meshes.

**This is the integration seam for the full porous / two-phase
[`thermal_hydraulics`](crate::genfoam::thermal_hydraulics) solver
(`op-p6p.7`, in progress in the parallel fleet).** Its intended interface is
the same coupling contract — consume `powerDensityNeutronics`, produce
`TFuel` / `TStruct` / coolant density on its mesh — so when that solver lands
it drops into the same [`RegionModel`](super::outer_iteration::RegionModel)
seam. Until then this stand-in provides the temperature field the neutronics
feedback consumes. See the `TODO(op-p6p.7)` below.

```rust
pub mod mesh_region { /* ... */ }
```

### Types

#### Struct `MeshNeutronics`

Mesh-based neutron-diffusion region with cross-section temperature feedback.

Owns the multigroup cross sections (`Arc<CrossSectionData>`, re-materialised
every sub-step at the mapped feedback temperature), the mesh zoning and flux
boundary conditions, the solver settings, and the [`ReactivityFeedback`] used
both to collapse the temperature field to a feedback point and to report the
equivalent feedback reactivity for diagnostics.

```rust
pub struct MeshNeutronics {
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
  pub fn new(region_name: &str, mesh: Arc<FvMesh>, xs: Arc<CrossSectionData>, zone_of_cell: Vec<usize>, base_params: Vec<f64>, feedback_var: Option<usize>, flux_boundary: Vec<BoundaryCondition<f64>>, settings: DiffusionSettings, target_power: Power, feedback: ReactivityFeedback) -> Result<Self, CouplingLoopError> { /* ... */ }
  ```
  Build a mesh-diffusion region.

- ```rust
  pub fn k_eff(self: &Self) -> f64 { /* ... */ }
  ```
  The current effective multiplication factor `k_eff`.

- ```rust
  pub fn fission_power(self: &Self) -> Power { /* ... */ }
  ```
  The fixed total fission power the model renormalises to [W].

- ```rust
  pub fn model(self: &Self) -> &DiffusionNeutronics { /* ... */ }
  ```
  The diffusion model (flux / precursors / power density / `k_eff`).

- ```rust
  pub fn feedback_reactivity(self: &Self, region: &CouplingRegion) -> Result<uom::si::f64::Ratio, CouplingLoopError> { /* ... */ }
  ```
  The equivalent feedback reactivity `Δρ` the mapped fields imply, via the

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
    fn clone(self: &Self) -> MeshNeutronics { /* ... */ }
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
- **RegionKernel**
  - ```rust
    fn correct(self: &mut Self, region: &mut CouplingRegion, _dt: Time, _tight: bool) -> Result<f64, CouplingLoopError> { /* ... */ }
    ```

  - ```rust
    fn region_name(self: &Self) -> &str { /* ... */ }
    ```

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
#### Struct `MeshThermalHydraulics`

Mesh-based thermal-hydraulics region — a per-cell fuel-energy balance.

A real but minimal spatial thermal model closing the loop across meshes:
each cell solves the backward-Euler balance
`ρc_p dT/dt = q''' − (ρc_p/τ)(T − T_cool)`, reading the mapped per-cell
`powerDensityNeutronics` `q'''` [W/m³] and writing `TFuel` [K]. `τ` is the
fuel-to-coolant thermal time constant — expressing the coolant coupling as a
time constant keeps every input a standard named `uom` alias (no exotic
volumetric-conductance type). At steady state `T = T_cool + q''' τ/ρc_p`, so
deposited power heats the fuel above the coolant temperature.

TODO(op-p6p.7): replace this stand-in with the full porous single-/two-phase
[`thermal_hydraulics`](crate::genfoam::thermal_hydraulics) solver when it
lands. Its coupling contract is identical (consume `powerDensityNeutronics`,
produce `TFuel` / `TStruct` / coolant density on this mesh), so it slots into
the same [`RegionModel`](super::outer_iteration::RegionModel) seam.

```rust
pub struct MeshThermalHydraulics {
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
  pub fn new(region_name: &str, n_cells: usize, rho_cp: VolumetricHeatCapacity, tau: Time, t_coolant: ThermodynamicTemperature, t_initial: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a per-cell thermal region on a mesh of `n_cells` at a uniform

- ```rust
  pub fn temperatures(self: &Self) -> &[f64] { /* ... */ }
  ```
  The current per-cell fuel temperature [K].

- ```rust
  pub fn mean_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  The volume-mean fuel temperature.

- ```rust
  pub fn begin_step(self: &mut Self) { /* ... */ }
  ```
  Snapshot the temperature at the start of an outer time step.

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
    fn clone(self: &Self) -> MeshThermalHydraulics { /* ... */ }
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
- **RegionKernel**
  - ```rust
    fn correct(self: &mut Self, region: &mut CouplingRegion, dt: Time, _tight: bool) -> Result<f64, CouplingLoopError> { /* ... */ }
    ```

  - ```rust
    fn region_name(self: &Self) -> &str { /* ... */ }
    ```

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
## Module `mesh_to_mesh`

# Volumetric mesh-to-mesh field mapping

Rust port of the *volumetric* cell-to-cell field transfer at the heart of
GeN-Foam's `meshHandler` (`meshToMesh` addressing + `map` / `mapTgtToSrc`).
Where the neutronics, thermal-hydraulics and thermo-mechanics regions live on
**separate, spatially-overlapping meshes** covering the same physical volume,
this maps a cell field (temperature, density, power density, mesh
displacement) from a *source* region mesh onto a *target* region mesh.

This is distinct from [`outram_foam_basic_lib::mesh::RegionInterface`], which
couples two regions across a **shared boundary patch** (the
`chtMultiRegionFoam` conjugate-heat-transfer pattern). GeN-Foam's multi-region
coupling is **volume-overlap**, not face-shared: the whole neutronics mesh
overlaps the whole TH mesh, and every cell value is transferred.

## Methods ([`MappingMethod`])

For each target cell the mapper precomputes a short list of `(source cell,
weight)` contributions (the "addressing", built once at construction, exactly
as upstream builds a `meshToMesh` once and reuses it every step):

- [`MappingMethod::NearestCell`] — the value of the source cell whose centre
  is nearest the target cell centre. Exact for co-located (conformal) meshes,
  where the nearest source cell *is* the coincident cell.
- [`MappingMethod::InverseDistance`] — inverse-distance-squared blend of the
  `n` nearest source cells. Smooths across a refinement change.
- [`MappingMethod::CellVolumeWeight`] — upstream's default
  (`imCellVolumeWeight`). The *exact* upstream operator integrates the true
  polyhedral cell-overlap volumes (a supermesh intersection); basic-lib does
  not yet expose that geometry, so this port uses nearest-cell addressing
  plus a **global integral-conservation rescale** — a documented degraded
  mode that preserves the volume integral `∑ V φ` exactly (the property most
  coupling terms rely on) but not the local overlap distribution. Exact
  polyhedral overlap is tracked as a scaffolding bead (see the crate
  `multi_region` module docs).

## Units

The mapper is a linear operator on raw cell values, so it is unit-agnostic —
it carries the field's physical meaning without interpreting it, matching how
[`outram_foam_basic_lib`] fields (`VolScalarField` = `f64` per cell) are
unit-agnostic. The physical `uom` quantities live in the coupling layer that
*reads* the mapped fields ([`super::outer_iteration`]).

```rust
pub mod mesh_to_mesh { /* ... */ }
```

### Types

#### Enum `MappingMethod`

How a target cell draws its value from the source mesh.

Mirrors the interpolation-method choice GeN-Foam's `meshToMesh` constructor
takes (upstream hard-codes `imCellVolumeWeight`). See the module docs for the
per-variant fidelity.

```rust
pub enum MappingMethod {
    NearestCell,
    InverseDistance {
        n_neighbours: usize,
    },
    CellVolumeWeight,
}
```

##### Variants

###### `NearestCell`

Value of the single nearest source cell (by centre distance).

###### `InverseDistance`

Inverse-distance-squared blend of the `n_neighbours` nearest source cells.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `n_neighbours` | `usize` | Number of nearest source cells blended (must be `>= 1`). |

###### `CellVolumeWeight`

Cell-volume-weighted conservative transfer (upstream default). This port
approximates it as nearest-cell addressing plus a global integral rescale;
see the module docs.

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
    fn clone(self: &Self) -> MappingMethod { /* ... */ }
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
    fn eq(self: &Self, other: &MappingMethod) -> bool { /* ... */ }
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
#### Enum `MapCombine`

How mapped values combine with the existing target-field contents.

Upstream `meshHandler::map` uses `plusEqOp` — it *accumulates* onto the
target (so several source regions can sum into one feedback field). A single
clean transfer uses [`MapCombine::Replace`].

```rust
pub enum MapCombine {
    Replace,
    Accumulate,
}
```

##### Variants

###### `Replace`

Overwrite the target cell with the mapped value.

###### `Accumulate`

Add the mapped value onto the target cell (upstream `plusEqOp`).

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
    fn clone(self: &Self) -> MapCombine { /* ... */ }
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
    fn eq(self: &Self, other: &MapCombine) -> bool { /* ... */ }
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
#### Struct `MeshToMesh`

A precomputed cell-to-cell mapping from a source mesh onto a target mesh.

Build once with [`MeshToMesh::new`]; apply every coupling step with
[`MeshToMesh::map_scalar`] / [`MeshToMesh::map_vector`]. The addressing
(`weights`) is independent of the field, so it is cached — exactly as
GeN-Foam caches a `meshToMesh` object across time steps.

```rust
pub struct MeshToMesh {
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
  pub fn new(source: Arc<FvMesh>, target: Arc<FvMesh>, method: MappingMethod) -> Self { /* ... */ }
  ```
  Build the mapping addressing from `source` onto `target` with `method`.

- ```rust
  pub fn source(self: &Self) -> &Arc<FvMesh> { /* ... */ }
  ```
  The source mesh this mapping reads from.

- ```rust
  pub fn target(self: &Self) -> &Arc<FvMesh> { /* ... */ }
  ```
  The target mesh this mapping writes onto.

- ```rust
  pub fn method(self: &Self) -> MappingMethod { /* ... */ }
  ```
  The interpolation method in use.

- ```rust
  pub fn map_scalar(self: &Self, source_field: &VolScalarField, combine: MapCombine, target_field: &mut VolScalarField) { /* ... */ }
  ```
  Map a scalar cell field from the source mesh onto `target_field` (which

- ```rust
  pub fn map_vector(self: &Self, source_field: &VolVectorField, combine: MapCombine, target_field: &mut VolVectorField) { /* ... */ }
  ```
  Map a vector cell field (e.g. `meshDisp`, velocity) onto `target_field`.

- ```rust
  pub fn interpolate_scalar(self: &Self, source_field: &VolScalarField, name: &str) -> VolScalarField { /* ... */ }
  ```
  Interpolate a scalar field, returning a fresh field on the target mesh.

- ```rust
  pub fn integral(mesh: &FvMesh, field: &VolScalarField) -> f64 { /* ... */ }
  ```
  The volume integral `∑_c V_c φ_c` of a scalar field over a mesh.

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
    fn clone(self: &Self) -> MeshToMesh { /* ... */ }
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
## Module `outer_iteration`

# Outer coupling iteration (the tightly-coupled Picard loop)

Rust port of GeN-Foam's `multiPhysicsSolver::correctPhysics`: the
loose/tight-coupling loop that advances the physics regions (neutronics +
thermal-hydraulics + thermo-mechanics) and exchanges their feedback fields
through the [`MeshHandler`] until the inter-region residual converges.

## The loop (matching upstream)

```text
do {
    mapTheseFields(activeRegions)            // exchange feedback (MeshHandler)
    for each region:
        deformMesh()                          // apply meshDisp (deferred here)
        (iter>0 && correctOnlyEnergy)?        // loose vs tight sub-step
            correctTightlyCoupledPhysics() : correctPhysics()
    residual = max_region getResidual()
} while (residual > minResidual && iter < maxIterations)
```

## Enum dispatch, not `dyn`

Upstream holds a `PtrList<solver>` of polymorphic region solvers. Per the
workspace no-trait-object rule, the region models are a closed
[`RegionModel`] **enum**; adding a model forces every dispatch site to handle
it. The [`RegionKernel`] trait is a *compile-time contract* each model
satisfies (it is never used as `dyn`).

## What is realised vs. deferred

The loop dispatches to mesh-based **diffusion** neutronics
([`RegionModel::MeshNeutronics`], wrapping the ported
[`DiffusionNeutronics`](crate::genfoam::neutronics::DiffusionNeutronics)) and
to a per-cell thermal region ([`RegionModel::MeshThermalHydraulics`]). SP3,
S_N and the full porous thermal-hydraulics driver drop into the same
dispatch seam but are not yet wired as region models.

It also drives, using only 0-D code, a **point-kinetics ↔ lumped
thermal-hydraulics** coupling:
[`LumpedNeutronics`] wraps the ported
[`point_kinetics`](crate::genfoam::neutronics::point_kinetics) and consumes a
mapped fuel temperature as Doppler-feedback reactivity; [`LumpedThermal`]
closes the loop with a lumped fuel-energy balance. This exercises the whole
stack — [`MeshHandler`] mapping, feedback assembly, and this Picard loop —
against a real transient. When the mesh-based models land, they become new
[`RegionModel`] variants and the `match` arms flag every site to update.
[`RegionModel::Prescribed`] is the integration seam for a region whose fields
are set externally (a black-box or not-yet-ported model).

## Units

Physical quantities use `uom` ([`Power`], [`ThermodynamicTemperature`],
[`Time`], [`Reactivity`]). The convergence **residual** is a dimensionless,
normalised relative change (a pure `f64`) — it compares each region's
solution to its previous iterate, so it carries no physical unit.

```rust
pub mod outer_iteration { /* ... */ }
```

### Types

#### Type Alias `FeedbackCoefficient`

Doppler-style reactivity feedback coefficient `α` such that
`Δρ = −α · (T_fuel − T_ref)` (units 1/K; negative feedback is the physical
stabilising case).

```rust
pub type FeedbackCoefficient = uom::si::f64::TemperatureCoefficient;
```

#### Enum `CouplingLoopError`

Errors from the outer coupling loop.

```rust
pub enum CouplingLoopError {
    MissingField {
        region: String,
        field: String,
    },
    PointKinetics(crate::genfoam::neutronics::point_kinetics::PointKineticsError),
    Neutronics(String),
    NotConverged {
        residual: f64,
        iterations: usize,
    },
}
```

##### Variants

###### `MissingField`

A region model references a coupling field missing from its region.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `region` | `String` | The region. |
| `field` | `String` | The absent field. |

###### `PointKinetics`

The point-kinetics sub-solve failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::genfoam::neutronics::point_kinetics::PointKineticsError` |  |

###### `Neutronics`

A mesh-based neutronics sub-solve failed (cross-section materialisation or
eigenvalue/transient solve). The message is the underlying
`NeutronicsError`, kept as a `String` so this error stays `Clone`
([`crate::genfoam::neutronics::NeutronicsError`] wraps un-`Clone` RBF
diagnostics).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NotConverged`

The loop hit `max_iterations` without reaching `min_residual`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `residual` | `f64` | Residual at the final iteration. |
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CouplingLoopError { /* ... */ }
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

  - ```rust
    fn from(e: PointKineticsError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CouplingLoopError) -> bool { /* ... */ }
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
#### Struct `LumpedNeutronics`

0-D neutronics region: the ported point-kinetics core with Doppler feedback.

Each sub-step it reads `TFuel` [K] from its region, forms the reactivity
`ρ = ρ_ext − α (T_fuel − T_ref)`, steps point-kinetics over `dt`, and writes
the volumetric heat source `powerDensityNeutronics = P / V` [W/m³] back. The
residual is the relative change in fission power across the iterate.

```rust
pub struct LumpedNeutronics {
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
  pub fn new(region_name: &str, params: PointKineticsParameters, initial_power: Power, rho_ext: Reactivity, alpha: FeedbackCoefficient, t_ref: ThermodynamicTemperature, volume: f64) -> Self { /* ... */ }
  ```
  Build a 0-D neutronics region.

- ```rust
  pub fn fission_power(self: &Self) -> Power { /* ... */ }
  ```
  The current fission power.

- ```rust
  pub fn begin_step(self: &mut Self) { /* ... */ }
  ```
  Snapshot the full point-kinetics state at the start of an outer time step

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
    fn clone(self: &Self) -> LumpedNeutronics { /* ... */ }
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
- **RegionKernel**
  - ```rust
    fn correct(self: &mut Self, region: &mut CouplingRegion, dt: Time, _tight: bool) -> Result<f64, CouplingLoopError> { /* ... */ }
    ```

  - ```rust
    fn region_name(self: &Self) -> &str { /* ... */ }
    ```

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
#### Struct `LumpedThermal`

Lumped thermal-hydraulics region: a fuel-node energy balance closing the
coupling loop.

Each sub-step it reads `powerDensityNeutronics` [W/m³], forms the deposited
power `P = q''' · V`, advances the lumped fuel temperature by backward Euler
`C dT/dt = P − UA (T − T_cool)`, and writes `TFuel` [K] back. The residual is
the relative change in temperature across the iterate.

```rust
pub struct LumpedThermal {
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
  pub fn new(region_name: &str, heat_capacity: HeatCapacity, conductance: ThermalConductance, t_coolant: ThermodynamicTemperature, t_initial: ThermodynamicTemperature, volume: f64) -> Self { /* ... */ }
  ```
  Build a lumped-TH region with an initial fuel temperature.

- ```rust
  pub fn fuel_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  The current fuel temperature.

- ```rust
  pub fn begin_step(self: &mut Self) { /* ... */ }
  ```
  Snapshot the temperature at the start of an outer time step.

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
    fn clone(self: &Self) -> LumpedThermal { /* ... */ }
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
- **RegionKernel**
  - ```rust
    fn correct(self: &mut Self, region: &mut CouplingRegion, dt: Time, _tight: bool) -> Result<f64, CouplingLoopError> { /* ... */ }
    ```

  - ```rust
    fn region_name(self: &Self) -> &str { /* ... */ }
    ```

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
#### Struct `PrescribedRegion`

A region whose coupling fields are set externally — the integration seam for
a black-box or not-yet-ported physics model. It advances nothing and reports
a zero residual (it never blocks convergence).

```rust
pub struct PrescribedRegion {
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
  pub fn new(region_name: &str) -> Self { /* ... */ }
  ```
  Build a prescribed region by name.

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
    fn clone(self: &Self) -> PrescribedRegion { /* ... */ }
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
- **RegionKernel**
  - ```rust
    fn correct(self: &mut Self, _region: &mut CouplingRegion, _dt: Time, _tight: bool) -> Result<f64, CouplingLoopError> { /* ... */ }
    ```

  - ```rust
    fn region_name(self: &Self) -> &str { /* ... */ }
    ```

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
#### Enum `RegionModel`

**Attributes:**

- `Other("#[allow(clippy::large_enum_variant)]")`

The closed set of coupled region models (enum dispatch, no `dyn`).

Adding a mesh-based neutronics or thermal-hydraulics model later means adding
a variant here — every `match` on [`RegionModel`] then fails to compile until
it handles the new case (exhaustiveness).

```rust
pub enum RegionModel {
    Neutronics(LumpedNeutronics),
    ThermalHydraulics(LumpedThermal),
    Prescribed(PrescribedRegion),
    MeshNeutronics(super::mesh_region::MeshNeutronics),
    MeshThermalHydraulics(super::mesh_region::MeshThermalHydraulics),
}
```

##### Variants

###### `Neutronics`

0-D point-kinetics neutronics with Doppler feedback.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LumpedNeutronics` |  |

###### `ThermalHydraulics`

Lumped fuel-node thermal-hydraulics.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LumpedThermal` |  |

###### `Prescribed`

Externally-driven / not-yet-ported region.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PrescribedRegion` |  |

###### `MeshNeutronics`

Mesh-based multigroup-diffusion neutronics with cross-section temperature
feedback (wraps [`DiffusionNeutronics`](crate::genfoam::neutronics::DiffusionNeutronics)).
The `DiffusionNeutronics` it carries is far larger than the lumped
variants, which clippy's `large_enum_variant` flags; the standard `Box`
fix is banned by the workspace rules, so the lint is allowed on this enum.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::mesh_region::MeshNeutronics` |  |

###### `MeshThermalHydraulics`

Mesh-based per-cell thermal-hydraulics (the integration seam for the full
[`thermal_hydraulics`](crate::genfoam::thermal_hydraulics) solver).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::mesh_region::MeshThermalHydraulics` |  |

##### Implementations

###### Methods

- ```rust
  pub fn begin_step(self: &mut Self) { /* ... */ }
  ```
  Snapshot each stateful model's start-of-step baseline before the Picard

- ```rust
  pub fn correct(self: &mut Self, region: &mut CouplingRegion, dt: Time, tight: bool) -> Result<f64, CouplingLoopError> { /* ... */ }
  ```
  Dispatch [`RegionKernel::correct`].

- ```rust
  pub fn region_name(self: &Self) -> &str { /* ... */ }
  ```
  Dispatch [`RegionKernel::region_name`].

- ```rust
  pub fn correct_only_energy(self: &Self) -> bool { /* ... */ }
  ```
  Whether this model participates in the tight (energy-only) inner sub-step

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
    fn clone(self: &Self) -> RegionModel { /* ... */ }
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
#### Struct `MultiPhysicsSolver`

The multi-physics outer coupling loop (`multiPhysicsSolver`).

Owns the [`MeshHandler`] (region meshes + coupling fields + mappings) and the
[`RegionModel`]s, plus the loop controls. Call [`MultiPhysicsSolver::solve`]
once per physical time step to drive the regions to a converged coupled state.

```rust
pub struct MultiPhysicsSolver {
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
  pub fn new(handler: MeshHandler, models: Vec<RegionModel>, min_residual: f64, max_iterations: usize) -> Self { /* ... */ }
  ```
  Build the loop from a wired [`MeshHandler`] and its region models.

- ```rust
  pub fn handler(self: &Self) -> &MeshHandler { /* ... */ }
  ```
  Borrow the mesh handler (to read converged coupling fields).

- ```rust
  pub fn model(self: &Self, i: usize) -> Option<&RegionModel> { /* ... */ }
  ```
  Borrow a region model by index (models are stored in construction order).

- ```rust
  pub fn residual_history(self: &Self) -> &[f64] { /* ... */ }
  ```
  The residual at each iterate of the last [`Self::solve`].

- ```rust
  pub fn solve(self: &mut Self, dt: Time) -> Result<usize, CouplingLoopError> { /* ... */ }
  ```
  Advance one physical time step of `dt`, running the Picard loop to

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
    fn clone(self: &Self) -> MultiPhysicsSolver { /* ... */ }
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
### Traits

#### Trait `RegionKernel`

Compile-time contract every [`RegionModel`] variant satisfies (never used as
a trait object — dispatch is through the enum).

A region model reads its input coupling fields from its [`CouplingRegion`],
advances one Picard sub-step, writes its output coupling fields back, and
returns a normalised residual (relative change from the previous iterate).

```rust
pub trait RegionKernel {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `correct`: Advance one outer-loop sub-step over `dt`.
- `region_name`: The region name this model owns (its mesh/fields in the [`MeshHandler`]).

##### Implementations

This trait is implemented for the following types:

- `MeshNeutronics`
- `MeshThermalHydraulics`
- `LumpedNeutronics`
- `LumpedThermal`
- `PrescribedRegion`

### Functions

#### Function `lumped_cell_mesh`

**Attributes:**

- `MustUse { reason: None }`

A single-cell mesh — the trivial "0-D region" carrier for a lumped model, so
it plugs into the mesh-based [`MeshHandler`] like any spatial region.

`volume` is the region's physical volume [m³]; the lumped fields (fuel
temperature, power density) live on the one cell.

```rust
pub fn lumped_cell_mesh(volume: f64) -> std::sync::Arc<outram_foam_basic_lib::mesh::fv_mesh::FvMesh> { /* ... */ }
```

## Module `rbf_mapping`

# Radial-basis-function field mapping for non-conformal meshes

Rust port of GeN-Foam's `meshHandler::interpolateAndMapFields`
polyharmonic-spline path: when a field is only known at a **scattered set of
sample points** (e.g. an axial power profile sampled at a handful of
elevations, or values read off cells of a source region that does *not*
volume-overlap the target), a smooth radial-basis-function (RBF) interpolant
is fit to the samples and evaluated at every target cell centre.

This is the fallback for the volumetric [`super::mesh_to_mesh::MeshToMesh`]
when the source and target meshes are geometrically dissimilar (different
dimensionality, no cell overlap) — exactly the case upstream's commented
`interpolateAndMapFields` covers with `polyharmonicSpline` / `kriging`.

## RBF kernel reuse

The polyharmonic-spline solve/evaluate kernel is **not** re-implemented here.
It is reused from [`crate::genfoam::common::rbf`], the single shared home for
the port of upstream's `common/radialBasisFunctionInterpolation` (the same
kernel also backs cross-section parametrisation in
[`crate::genfoam::neutronics::xs`]). This module depends only on the two free
functions [`rbf::solve_polyharmonic_spline`] and [`rbf::polyharmonic_spline`].
**No second RBF copy is introduced.**

## Units

Sample coordinates are metres (mesh cell centres are geometric positions).
Sample *values* are unit-agnostic raw `f64`, matching
[`outram_foam_basic_lib`] scalar fields — the physical `uom` meaning is
attached by the consumer that reads the mapped field.

```rust
pub mod rbf_mapping { /* ... */ }
```

### Types

#### Enum `PolyharmonicMode`

The polyharmonic-spline basis order (GeN-Foam `polyharmonicSplineMode`).

Selects the radial basis `φ(r)`; see
[`rbf::polyharmonic_spline_function`]. [`PolyharmonicMode::Linear`] (the
upstream default, `mode = 1`) reproduces linear fields exactly and is the
safe choice for reactor axial/radial profiles.

```rust
pub enum PolyharmonicMode {
    Linear,
    ThinPlate,
    Cubic,
    ThinPlate4,
}
```

##### Variants

###### `Linear`

`φ = r` — linear; reproduces linear functions exactly (upstream default).

###### `ThinPlate`

`φ = r² ln r` — thin-plate spline.

###### `Cubic`

`φ = r³`.

###### `ThinPlate4`

`φ = r⁴ ln r`.

##### Implementations

###### Methods

- ```rust
  pub fn as_mode(self: Self) -> usize { /* ... */ }
  ```
  The upstream integer `polyharmonicSplineMode` this corresponds to.

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
    fn clone(self: &Self) -> PolyharmonicMode { /* ... */ }
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
    fn eq(self: &Self, other: &PolyharmonicMode) -> bool { /* ... */ }
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
#### Enum `RbfMapError`

Errors from building or applying an [`RbfFieldMap`].

```rust
pub enum RbfMapError {
    EmptyOrMismatched {
        n_points: usize,
        n_values: usize,
    },
    Singular,
}
```

##### Variants

###### `EmptyOrMismatched`

Fewer than one sample point, or a sample-count/value-count mismatch.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `n_points` | `usize` | Number of sample coordinates supplied. |
| `n_values` | `usize` | Number of sample values supplied. |

###### `Singular`

The RBF saddle-point system was singular (e.g. duplicate sample points).

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
    fn clone(self: &Self) -> RbfMapError { /* ... */ }
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

- **Eq**
- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(_: MatrixError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RbfMapError) -> bool { /* ... */ }
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
#### Struct `RbfFieldMap`

A polyharmonic-spline interpolant fit to scattered 3-D sample points, ready
to evaluate at arbitrary positions or map onto a whole target mesh.

Built once from `(sample_points, values)`; the weights are cached so mapping
onto many cells is cheap. This is the RBF analogue of a cached
[`super::mesh_to_mesh::MeshToMesh`] addressing.

```rust
pub struct RbfFieldMap {
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
  pub fn build(sample_points: &[Vector3], values: &[f64], mode: PolyharmonicMode) -> Result<Self, RbfMapError> { /* ... */ }
  ```
  Fit the polyharmonic spline to `sample_points` with the given `values`.

- ```rust
  pub fn evaluate(self: &Self, point: Vector3) -> f64 { /* ... */ }
  ```
  Evaluate the interpolant at an arbitrary position (metres).

- ```rust
  pub fn map_onto(self: &Self, target: Arc<FvMesh>, name: &str) -> VolScalarField { /* ... */ }
  ```
  Evaluate the interpolant at every cell centre of `target`, returning a

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
    fn clone(self: &Self) -> RbfFieldMap { /* ... */ }
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
## Module `reactivity_feedback`

# Reactivity-feedback layer (temperature / density → reactivity)

Rust port of the reactivity-feedback assembly GeN-Foam's `pointKinetics`
neutronics performs each step: it turns the thermal-hydraulic / mechanical
**feedback fields** on the mesh (fuel temperature, structure temperature,
coolant density, axial expansion) into a single scalar reactivity `Δρ` that
perturbs the neutronics. This is the layer that closes the
neutronics ↔ thermal-hydraulics loop.

It generalises the 0-D Doppler feedback already demonstrated in
[`super::outer_iteration`] (`ρ = ρ_ext − α (T_fuel − T_ref)`, a single
lumped temperature) to **spatial mesh fields**: each feedback term
contributes a *local* reactivity density per cell, and the total `Δρ` is the
importance-weighted volume average of those local contributions (first-order
perturbation theory: the reactivity worth of a local perturbation is weighted
by the local neutron importance).

## The feedback sum

For a set of feedback terms `t` (Doppler, expansion, coolant density), with a
per-cell importance weight `w_c` (cell volume, optionally times a supplied
worth field such as `phi^2`):

```text
  Δρ = ( Σ_c w_c Σ_t contribution_t(field_t[c]) ) / ( Σ_c w_c )
```

Each term's contribution is written so a **positive** coefficient is the
physically stabilising (negative-feedback) case, matching
[`super::outer_iteration::LumpedNeutronics`]:

- [`FeedbackTerm::Doppler`] — `−α (T − T_ref)` (linear) or
  `−α T_ref ln(T/T_ref)` (logarithmic, the classic Doppler broadening law;
  equal to the linear form to first order in `ΔT`).
- [`FeedbackTerm::Expansion`] — `−α (T_struct − T_ref)` (axial fuel / clad
  expansion, temperature-driven).
- [`FeedbackTerm::Density`] — `−α (ρ − ρ_ref)/ρ_ref` (coolant density / void;
  coefficient is reactivity per unit *fractional* density change).

## Reduction to the lumped case (V&V anchor)

For a **uniform** field and a volume weight, the importance-weighted average
collapses to the single-term lumped expression, so a mesh with a spatially
constant `T_fuel` reproduces `Δρ = −α (T_fuel − T_ref)` exactly — the bridge
between this layer and the verified 0-D path.

## Units

Coefficients and references cross the API as named `uom` aliases
([`FeedbackCoefficient`] = 1/K, [`ThermodynamicTemperature`],
[`MassDensity`]); the returned [`Reactivity`] is `uom`'s dimensionless
[`Ratio`]. The per-cell reduction is done in raw `f64` internally (never at
the API boundary).

```rust
pub mod reactivity_feedback { /* ... */ }
```

### Types

#### Enum `DopplerLaw`

The temperature dependence of the Doppler reactivity term.

Both laws use the same [`FeedbackCoefficient`] `α` (1/K) and agree to first
order in `ΔT`; they differ only in how the term grows for large excursions.

```rust
pub enum DopplerLaw {
    Linear,
    Logarithmic,
}
```

##### Variants

###### `Linear`

`−α (T − T_ref)` — linear in the temperature rise.

###### `Logarithmic`

`−α T_ref ln(T/T_ref)` — the logarithmic Doppler-broadening law (the
resonance-integral `A + B ln T` form), scaled by `T_ref` so `α` keeps its
1/K units and the two laws coincide as `T → T_ref`.

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
    fn clone(self: &Self) -> DopplerLaw { /* ... */ }
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
    fn eq(self: &Self, other: &DopplerLaw) -> bool { /* ... */ }
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
#### Enum `ImportanceWeight`

Per-cell importance weight used to average local reactivity contributions.

First-order perturbation theory weights a local perturbation's reactivity
worth by the local neutron importance (`phi ϕ†`); with the fundamental-mode
approximation `ϕ† ∝ ϕ` that is `phi^2`. Absent an adjoint solve, a plain
volume weight is the honest default (every cell equally important).

```rust
pub enum ImportanceWeight {
    Volume,
    Field(String),
}
```

##### Variants

###### `Volume`

Weight each cell by its volume `V_c` only (uniform importance).

###### `Field`

Weight each cell by `V_c · f_c`, where `f_c` is the named scalar field
(e.g. a one-group flux or `phi^2` worth field) read from the region.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

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
    fn clone(self: &Self) -> ImportanceWeight { /* ... */ }
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
#### Enum `FeedbackTerm`

One additive feedback contribution (enum dispatch, no `dyn`).

Each variant names the region scalar field it reads, its coefficient, and its
reference point. `contribution` returns the **local** reactivity `Δρ_c` (a
dimensionless `f64`) for the field value in one cell; a positive coefficient
gives negative (stabilising) feedback under a positive deviation.

```rust
pub enum FeedbackTerm {
    Doppler {
        field: String,
        coeff: super::outer_iteration::FeedbackCoefficient,
        t_ref: uom::si::f64::ThermodynamicTemperature,
        law: DopplerLaw,
    },
    Expansion {
        field: String,
        coeff: super::outer_iteration::FeedbackCoefficient,
        t_ref: uom::si::f64::ThermodynamicTemperature,
    },
    Density {
        field: String,
        coeff: uom::si::f64::Ratio,
        rho_ref: uom::si::f64::MassDensity,
    },
}
```

##### Variants

###### `Doppler`

Fuel-temperature (Doppler) feedback, `−α (T − T_ref)` or the log law.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `String` | Name of the fuel-temperature field (K) in the region. |
| `coeff` | `super::outer_iteration::FeedbackCoefficient` | Feedback coefficient `α` (1/K); positive ⇒ stabilising. |
| `t_ref` | `uom::si::f64::ThermodynamicTemperature` | Reference temperature `T_ref` (K). |
| `law` | `DopplerLaw` | Temperature dependence. |

###### `Expansion`

Structure-temperature / thermal-expansion feedback, `−α (T − T_ref)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `String` | Name of the structure-temperature field (K) in the region. |
| `coeff` | `super::outer_iteration::FeedbackCoefficient` | Feedback coefficient `α` (1/K); positive ⇒ stabilising. |
| `t_ref` | `uom::si::f64::ThermodynamicTemperature` | Reference temperature `T_ref` (K). |

###### `Density`

Coolant-density / void feedback, `−α (ρ − ρ_ref)/ρ_ref` (coefficient is
reactivity per unit fractional density change).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `String` | Name of the coolant-density field (kg/m³) in the region. |
| `coeff` | `uom::si::f64::Ratio` | Reactivity per unit fractional density change (dimensionless<br>`Ratio`). Positive ⇒ losing coolant (density drop) adds positive<br>reactivity is the *destabilising* case, so a positive coefficient<br>here means density *rise* subtracts reactivity. |
| `rho_ref` | `uom::si::f64::MassDensity` | Reference density `ρ_ref` (kg/m³). |

##### Implementations

###### Methods

- ```rust
  pub fn field_name(self: &Self) -> &str { /* ... */ }
  ```
  The name of the region field this term reads.

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
    fn clone(self: &Self) -> FeedbackTerm { /* ... */ }
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
#### Struct `ReactivityFeedback`

The reactivity-feedback assembly: a set of [`FeedbackTerm`]s and the
importance weighting used to reduce their per-cell contributions to one
scalar `Δρ`.

Build with [`Self::new`] (empty, volume-weighted) and add terms with the
`with_*` builders, then call [`Self::reactivity`] against a
[`CouplingRegion`] carrying the feedback fields.

```rust
pub struct ReactivityFeedback {
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
  A feedback assembly with no terms and a volume importance weight.

- ```rust
  pub fn with_weight(self: Self, weight: ImportanceWeight) -> Self { /* ... */ }
  ```
  Set the per-cell importance weight (default [`ImportanceWeight::Volume`]).

- ```rust
  pub fn with_doppler(self: Self, field: &str, coeff: FeedbackCoefficient, t_ref: ThermodynamicTemperature, law: DopplerLaw) -> Self { /* ... */ }
  ```
  Add a Doppler (fuel-temperature) term.

- ```rust
  pub fn with_expansion(self: Self, field: &str, coeff: FeedbackCoefficient, t_ref: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Add a structure-temperature / expansion term.

- ```rust
  pub fn with_density(self: Self, field: &str, coeff: Ratio, rho_ref: MassDensity) -> Self { /* ... */ }
  ```
  Add a coolant-density / void term.

- ```rust
  pub fn terms(self: &Self) -> &[FeedbackTerm] { /* ... */ }
  ```
  Borrow the configured terms (for diagnostics / dispatch checks).

- ```rust
  pub fn reactivity(self: &Self, region: &CouplingRegion) -> Result<Reactivity, CouplingLoopError> { /* ... */ }
  ```
  The total feedback reactivity `Δρ` for the feedback fields on `region`.

- ```rust
  pub fn weighted_mean(self: &Self, region: &CouplingRegion, field: &str) -> Result<f64, CouplingLoopError> { /* ... */ }
  ```
  Importance-weighted mean of a named scalar field over the region — the

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
    fn clone(self: &Self) -> ReactivityFeedback { /* ... */ }
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
### Re-exports

#### Re-export `CouplingError`

```rust
pub use coupling_fields::CouplingError;
```

#### Re-export `CouplingField`

```rust
pub use coupling_fields::CouplingField;
```

#### Re-export `CouplingRegion`

```rust
pub use coupling_fields::CouplingRegion;
```

#### Re-export `FieldKind`

```rust
pub use coupling_fields::FieldKind;
```

#### Re-export `MeshHandler`

```rust
pub use coupling_fields::MeshHandler;
```

#### Re-export `MeshNeutronics`

```rust
pub use mesh_region::MeshNeutronics;
```

#### Re-export `MeshThermalHydraulics`

```rust
pub use mesh_region::MeshThermalHydraulics;
```

#### Re-export `MapCombine`

```rust
pub use mesh_to_mesh::MapCombine;
```

#### Re-export `MappingMethod`

```rust
pub use mesh_to_mesh::MappingMethod;
```

#### Re-export `MeshToMesh`

```rust
pub use mesh_to_mesh::MeshToMesh;
```

#### Re-export `lumped_cell_mesh`

```rust
pub use outer_iteration::lumped_cell_mesh;
```

#### Re-export `CouplingLoopError`

```rust
pub use outer_iteration::CouplingLoopError;
```

#### Re-export `FeedbackCoefficient`

```rust
pub use outer_iteration::FeedbackCoefficient;
```

#### Re-export `LumpedNeutronics`

```rust
pub use outer_iteration::LumpedNeutronics;
```

#### Re-export `LumpedThermal`

```rust
pub use outer_iteration::LumpedThermal;
```

#### Re-export `MultiPhysicsSolver`

```rust
pub use outer_iteration::MultiPhysicsSolver;
```

#### Re-export `PrescribedRegion`

```rust
pub use outer_iteration::PrescribedRegion;
```

#### Re-export `RegionKernel`

```rust
pub use outer_iteration::RegionKernel;
```

#### Re-export `RegionModel`

```rust
pub use outer_iteration::RegionModel;
```

#### Re-export `PolyharmonicMode`

```rust
pub use rbf_mapping::PolyharmonicMode;
```

#### Re-export `RbfFieldMap`

```rust
pub use rbf_mapping::RbfFieldMap;
```

#### Re-export `RbfMapError`

```rust
pub use rbf_mapping::RbfMapError;
```

#### Re-export `DopplerLaw`

```rust
pub use reactivity_feedback::DopplerLaw;
```

#### Re-export `FeedbackTerm`

```rust
pub use reactivity_feedback::FeedbackTerm;
```

#### Re-export `ImportanceWeight`

```rust
pub use reactivity_feedback::ImportanceWeight;
```

#### Re-export `ReactivityFeedback`

```rust
pub use reactivity_feedback::ReactivityFeedback;
```

## Module `neutronics`

# `genfoam::neutronics` — deterministic reactor neutronics

Rust port of GeN-Foam's `src/classes/neutronics/`. GeN-Foam offers several
run-time-selectable neutronics models; upstream they share the abstract
`Foam::neutronics` base class. In this port the model set is a **closed enum**
(per the workspace no-`dyn`-dispatch rule) rather than a virtual base — adding
a model forces every `match` site to handle it.

## What belongs here

- [`point_kinetics`] — 0-D point-kinetics (implemented).
- [`diffusion`] — multigroup neutron diffusion, k-eigenvalue + transient
  (implemented).
- [`sp3`] — simplified-P3 transport, eigenvalue + transient (implemented).
- [`sn`] — discrete-ordinates (S_N) transport, **eigenvalue only**
  (implemented; the transient `step` is deferred, unlike `diffusion` and
  `sp3`). Each model also exposes a lightweight state-only constructor
  (`::new`) that allocates flux state without cross sections and therefore
  cannot solve — build with `with_cross_sections` to obtain a working model.
- [`state`] — the shared spatial flux / power / precursor / power-density
  state that the spatial models read and write.
- [`xs`] — the multigroup cross-section (`XS`) data structures.

Cross sections are read from GeN-Foam `nuclearData` dictionaries; this module
does **not** pull data from the NJOY / Monte Carlo crates.

## Model dispatch — [`NeutronicsModel`]

GeN-Foam selects a neutronics model at run time through the abstract
`Foam::neutronics` base class. Here the closed set of models is a
[`NeutronicsModel`] enum (per the workspace no-`dyn` rule): adding a variant
forces every `match` to handle it. The shared, model-agnostic surface —
[`NeutronicsModel::power`], [`NeutronicsModel::k_eff`],
[`NeutronicsModel::kind`] — is what a coupling/driver layer calls without
knowing which model it holds.

```rust
pub mod neutronics { /* ... */ }
```

### Modules

## Module `diffusion`

# Multigroup neutron-diffusion solver

Rust port of GeN-Foam's `diffusionNeutronics`. Assembles and solves the
multigroup neutron-diffusion equations on an
[`outram_foam_basic_lib::mesh::FvMesh`] with basic-lib's finite-volume
operators ([`fvm::laplacian`](outram_foam_basic_lib::fv_operators::fvm::laplacian),
[`fvm::sp`](outram_foam_basic_lib::fv_operators::fvm::sp),
[`fvm::ddt`](outram_foam_basic_lib::fv_operators::fvm::ddt)) and its LDU
linear solvers. Two forms are provided:

- **k-eigenvalue** ([`DiffusionNeutronics::solve_eigenvalue`]) — outer
  power iteration for the fundamental-mode flux shape and `k_eff` of a
  source-free reactor.
- **transient** ([`DiffusionNeutronics::step`]) — one backward-Euler time
  step of the coupled flux + delayed-neutron-precursor system at a fixed,
  externally-imposed `k_eff` (typically the eigenvalue from a preceding
  `solve_eigenvalue`, so a null transient holds steady).

## The equations (from `diffusionNeutronics.H`)

For each energy group `g` (removal `Sigma_{r,g}` already includes
out-scatter; the in-scatter from other groups is an explicit source):

```text
  (1/v_g) d(phi_g)/dt = div(D_g grad phi_g) - Sigma_{r,g} phi_g
                        + chi_{p,g}/k (1-beta) S_n + chi_{d,g} S_d + S_{s,g}
```

with `S_n = sum_h nu Sigma_{f,h} phi_h` (fission production),
`S_d = sum_k lambda_k C_k` (delayed source), and
`S_{s,g} = sum_{h != g} Sigma_{h->g} phi_h` (in-scatter). For the eigenvalue
form the time derivative drops and the whole fission source is scaled by
`1/k`; the precursors sit at their equilibrium
`C_k = beta_k S_n / (k lambda_k)`, so prompt + delayed collapse to the full
spectrum `chi_g = chi_{p,g}(1-beta) + chi_{d,g} beta` times `S_n/k`.

## FV sign convention

basic-lib's [`fvm::laplacian`](outram_foam_basic_lib::fv_operators::fvm::laplacian)
assembles a **positive-definite** matrix for
`-div(D grad phi)` (positive diagonal), so the diffusion + removal loss
operator is `fvm::laplacian(D_face, phi) + fvm::sp(Sigma_r, phi)` — both
positive-diagonal, hence diagonally dominant and Gauss-Seidel-friendly. See
the crate's `pimple_foam` docs for the sign-convention discussion.

## Scope / deferrals

Cross sections are materialised once at a fixed feedback point (see
[`fields::DiffusionXsFields`]); live thermal-hydraulic feedback, control-rod
motion, discontinuity-factor adjustment, liquid-fuel precursor advection,
Aitken acceleration, and the implicit/integral transient predictors are
deferred with the coupling layer. Boundary conditions are whatever the
caller sets on the flux fields (vacuum = `FixedValue(0)`, reflective =
`ZeroGradient`).

```rust
pub mod diffusion { /* ... */ }
```

### Modules

## Module `fields`

# Mesh-materialised multigroup cross-section fields

GeN-Foam's `diffusionNeutronics` stores its cross sections as
`PtrList<volScalarField>` — one field per energy group, filled cell-by-cell
from each cell's material zone (`setNeutronicsVariables.H`). This module is
the Rust analogue: it "bakes" the mesh-free per-zone
[`CrossSectionData`](crate::genfoam::neutronics::xs::CrossSectionData) onto
the mesh via a `cell -> zone` map, producing the
[`outram_foam_basic_lib::fields::VolScalarField`] bundle the FV assembly
consumes.

## Scope: a single fixed feedback point (feedback fields deferred)

Materialisation happens at **one** feedback state supplied as
`raw_parameters` (fuel/coolant temperature, density, ...). Every cross
section is evaluated there, so the fields are *spatially* heterogeneous
(different zones give different constants) but the feedback state itself is
uniform and static. Per-cell feedback driven by live thermal-hydraulics
fields — GeN-Foam's `xs_.correct(TFuel_, ...)` each time step — needs the
TH / multi-region coupling layer and is deferred (see the `neutronics` epic
and the `xs` module docs). For a benchmark whose `nuclearData` declares no
feedback variables, pass `raw_parameters = &[]` and every value equals the
`reference` state.

```rust
pub mod fields { /* ... */ }
```

### Types

#### Struct `DiffusionXsFields`

The full group-wise cross-section field set for a diffusion solve, baked
onto a mesh.

Every `Vec<VolScalarField>` is indexed by energy group `g` in the same order
as [`CrossSectionData`]. All fields carry the base-SI units documented on
[`CrossSectionData`]'s typed accessors (`D` in metres, cross sections in
`m^-1`, `sigma_pow` in `J/m`, inverse velocity in `s/m`).

```rust
pub struct DiffusionXsFields {
    pub energy_groups: usize,
    pub prec_groups: usize,
    pub d: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub d_face: Vec<outram_foam_basic_lib::prelude::SurfaceScalarField>,
    pub nu_sigma_f: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub sigma_pow: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub sigma_removal: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub chi_prompt: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub chi_delayed: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub inv_velocity: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub scattering: Vec<Vec<outram_foam_basic_lib::prelude::VolScalarField>>,
    pub beta: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub beta_tot: outram_foam_basic_lib::prelude::VolScalarField,
    pub lambda: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `energy_groups` | `usize` | Number of energy groups `G`. |
| `prec_groups` | `usize` | Number of delayed-neutron precursor groups `N`. |
| `d` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Diffusion coefficient `D_g` per group (m). |
| `d_face` | `Vec<outram_foam_basic_lib::prelude::SurfaceScalarField>` | `D_g` interpolated to faces, ready for<br>[`fvm::laplacian`](outram_foam_basic_lib::fv_operators::fvm::laplacian). |
| `nu_sigma_f` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Fission-neutron production `nu Sigma_{f,g}` per group (m^-1). |
| `sigma_pow` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Fission energy-release cross section `kappa Sigma_{f,g}` per group (J/m). |
| `sigma_removal` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Removal cross section `Sigma_{r,g}` per group (m^-1). |
| `chi_prompt` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Prompt fission spectrum `chi_{p,g}` per group (dimensionless). |
| `chi_delayed` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Delayed fission spectrum `chi_{d,g}` per group (dimensionless). |
| `inv_velocity` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Inverse neutron speed `1/v_g` per group (s/m). |
| `scattering` | `Vec<Vec<outram_foam_basic_lib::prelude::VolScalarField>>` | Scattering `Sigma_{s, j->i}` (moment 0), indexed `[from j][into i]`<br>(m^-1). The diagonal `j == i` (self-scatter) is present but ignored by<br>the in-scatter source, matching GeN-Foam's `energyJ != energyI` guard. |
| `beta` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Per-group delayed fraction `beta_k` per precursor group (dimensionless). |
| `beta_tot` | `outram_foam_basic_lib::prelude::VolScalarField` | Total delayed fraction `beta_tot` (dimensionless). |
| `lambda` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Precursor decay constant `lambda_k` per precursor group (s^-1). |

##### Implementations

###### Methods

- ```rust
  pub fn materialize(mesh: &Arc<FvMesh>, xs: &CrossSectionData, zone_of_cell: &[usize], raw_parameters: &[f64]) -> Result<Self, XsError> { /* ... */ }
  ```
  Materialise the cross sections onto `mesh` at the feedback point

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
    fn clone(self: &Self) -> DiffusionXsFields { /* ... */ }
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
### Types

#### Struct `DiffusionSettings`

Convergence and linear-solver controls for a diffusion solve.

Defaults: `1e-8` outer `k_eff` tolerance, `1e-6` outer flux tolerance, 500
outer (power) iterations, and a `1e-9` / 2000-iteration inner Gauss-Seidel
linear solve per group.

```rust
pub struct DiffusionSettings {
    pub k_tolerance: f64,
    pub flux_tolerance: f64,
    pub max_outer_iterations: usize,
    pub linear: outram_foam_basic_lib::prelude::SolverSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_tolerance` | `f64` | Outer-loop convergence tolerance on the relative change in `k_eff`. |
| `flux_tolerance` | `f64` | Outer-loop convergence tolerance on the relative L2 change in the flux. |
| `max_outer_iterations` | `usize` | Maximum number of outer power iterations. |
| `linear` | `outram_foam_basic_lib::prelude::SolverSettings` | Inner linear-solver settings (one group solve). |

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
    fn clone(self: &Self) -> DiffusionSettings { /* ... */ }
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
#### Struct `EigenvalueReport`

Outcome of an eigenvalue power iteration.

```rust
pub struct EigenvalueReport {
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
| `k_eff` | `f64` | Converged effective multiplication factor. |
| `outer_iterations` | `usize` | Number of outer power iterations performed. |
| `k_residual` | `f64` | Final relative change in `k_eff` between the last two outer iterations. |
| `flux_residual` | `f64` | Final relative L2 change in the flux between the last two outer<br>iterations. |
| `converged` | `bool` | Whether both residuals fell below their tolerances. |

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
    fn clone(self: &Self) -> EigenvalueReport { /* ... */ }
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
#### Struct `DiffusionNeutronics`

A multigroup neutron-diffusion model on an `FvMesh`.

Owns the shared [`NeutronicsState`] (flux / precursors / power density /
`k_eff`) and the mesh-materialised cross sections
([`DiffusionXsFields`]). Build with [`Self::new`], then call
[`Self::solve_eigenvalue`] or [`Self::step`].

```rust
pub struct DiffusionNeutronics {
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
  pub fn solve_eigenvalue(self: &mut Self) -> Result<EigenvalueReport, NeutronicsError> { /* ... */ }
  ```
  Solve the multigroup k-eigenvalue problem by outer power iteration.

- ```rust
  pub fn step(self: &mut Self, dt: Time) -> Result<(), NeutronicsError> { /* ... */ }
  ```
  Advance the coupled flux + precursor system by one implicit

- ```rust
  pub fn new(mesh: Arc<FvMesh>, xs: &CrossSectionData, zone_of_cell: &[usize], raw_parameters: &[f64], flux_boundary: &[BoundaryCondition<f64>], settings: DiffusionSettings) -> Result<Self, NeutronicsError> { /* ... */ }
  ```
  Build a diffusion model on `mesh` from cross sections `xs`, a

- ```rust
  pub fn state(self: &Self) -> &NeutronicsState { /* ... */ }
  ```
  The shared neutronics state (flux, precursors, power density, `k_eff`).

- ```rust
  pub fn xs_fields(self: &Self) -> &DiffusionXsFields { /* ... */ }
  ```
  The materialised cross-section fields.

- ```rust
  pub fn settings(self: &Self) -> &DiffusionSettings { /* ... */ }
  ```
  The convergence / solver settings.

- ```rust
  pub fn energy_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups `G`.

- ```rust
  pub fn prec_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of delayed-neutron precursor groups `N`.

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
    fn clone(self: &Self) -> DiffusionNeutronics { /* ... */ }
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
### Re-exports

#### Re-export `DiffusionXsFields`

```rust
pub use fields::DiffusionXsFields;
```

## Module `point_kinetics`

# Point-kinetics neutronics (0-D)

The lumped-parameter ("point") reactor-kinetics model: the reactor is treated
as a single point whose fission power `P(t)` and delayed-neutron precursor
"powers" `C_i(t)` evolve in time under a prescribed total reactivity. This is
the fastest neutronics option in GeN-Foam and the physics core that its
spatial models (diffusion/SP3/SN) reduce to when only the amplitude is
tracked.

## The equations

With `N` delayed-neutron precursor groups, prompt-neutron generation time
`Λ` (seconds), total reactivity `ρ` (dimensionless, `Δk/k`), optional
subcritical index `ζ` (dimensionless), and optional external source power
`S` (watts):

```text
  dP/dt   = (ρ − ζ − β)/Λ · P  +  Σ_i λ_i C_i  +  S/Λ
  dC_i/dt = β_i/Λ · P  −  λ_i C_i          for i = 1 … N
```

Here `β_i` is the effective delayed-neutron fraction of group `i`, `β = Σ β_i`
the total delayed fraction, and `λ_i` the decay constant of group `i` (`1/s`).
Following GeN-Foam, the precursor variables `C_i` are carried in **power units
(W)**, not neutron density, so that they scale directly with `P`.

## Numerical scheme — faithful to GeN-Foam

GeN-Foam solves the system fully implicitly (backward Euler) as one dense
linear system per time step (`include/solvePointKinetics.H`). The unknown
vector is `x = [P, C_1, …, C_N, S]` of length `n = N + 2` (the `+2` is the
fission power and a trivial row that carries the external source into the
power balance). With `Δt` the time step and a superscript `old` denoting the
previous step's values, the assembled system `A x = B` is

```text
  row 0   (power):    [1/Δt − (ρ−ζ−β)/Λ] P  − Σ_i λ_i C_i  − (1/Λ) S = P^old / Δt
  row i   (group i):  −(β_i/Λ) P  +  [1/Δt + λ_i] C_i               = C_i^old / Δt
  row n−1 (source):   [1/Δt] S                                       = S^old / Δt
```

The dense solve uses [`outram_foam_basic_lib::matrix::SquareMatrix`] (Crout
LU with partial pivoting), the direct analogue of GeN-Foam's
`Foam::scalarSquareMatrix` + `solve()`.

## Scope of this port

This module ports the **reactivity-driven 0-D ODE core** only. The full
GeN-Foam `pointKineticNeutronics` class additionally computes `ρ` from
temperature/density feedback fields on an `fvMesh` (Doppler, fuel/clad/coolant
/structure), GEM and control-rod-driveline reactivity, an external neutron
source with power-monitoring modulation, FMU inputs, and a liquid-fuel
precursor-advection variant. Those all require the mesh / thermal-hydraulics /
multi-region layers and are **deferred** (see `docs/genfoam-port-plan.md`).
Here, the total reactivity `ρ` and the external source power `S` are inputs
the caller supplies each step.

## Example

```
use outram_foam_appbuilder_lib::genfoam::neutronics::point_kinetics::{
    PointKineticsParameters, PointKineticsState,
};
use uom::si::f64::{Power, Ratio, Time, Frequency};
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::frequency::hertz;

// One delayed group, purely illustrative constants.
let params = PointKineticsParameters::new(
    vec![Ratio::new::<ratio>(0.0065)],      // β_1
    vec![Frequency::new::<hertz>(0.08)],    // λ_1  [1/s]
    Time::new::<second>(1.0e-4),            // Λ    [s]
)
.unwrap();

// Start critical: precursors at their equilibrium level for 1 MW.
let mut state = PointKineticsState::new_equilibrium(&params, Power::new::<watt>(1.0e6));

// Advance one 1 ms step at zero reactivity, no external source.
state
    .step(
        &params,
        Time::new::<second>(1.0e-3),
        Ratio::new::<ratio>(0.0),
        Power::new::<watt>(0.0),
    )
    .unwrap();

// At exact equilibrium and ρ = 0 the power is unchanged.
assert!((state.fission_power().get::<watt>() - 1.0e6).abs() < 1.0);
```

```rust
pub mod point_kinetics { /* ... */ }
```

### Types

#### Type Alias `Reactivity`

Total reactivity `ρ` or a delayed-neutron fraction `β` — a dimensionless
ratio (`Δk/k`). Named alias over `uom`'s [`Ratio`] for human readability.

```rust
pub type Reactivity = uom::si::f64::Ratio;
```

#### Type Alias `DecayConstant`

Delayed-neutron precursor decay constant `λ_i` — inverse time (`1/s`). Named
alias over `uom`'s [`Frequency`] (both are `s⁻¹`).

```rust
pub type DecayConstant = uom::si::f64::Frequency;
```

#### Type Alias `PromptGenerationTime`

Prompt-neutron generation time `Λ` — a [`Time`] (seconds). Named alias for
readability at call sites.

```rust
pub type PromptGenerationTime = uom::si::f64::Time;
```

#### Enum `PointKineticsError`

Errors from constructing or advancing a point-kinetics model.

```rust
pub enum PointKineticsError {
    MismatchedGroups {
        betas: usize,
        lambdas: usize,
    },
    NoDelayedGroups,
    NonPositivePromptGenerationTime(f64),
    NonPositiveDecayConstant {
        group: usize,
        value: f64,
    },
    NonPositiveTimeStep(f64),
    SingularSystem {
        col: usize,
    },
}
```

##### Variants

###### `MismatchedGroups`

The delayed-fraction and decay-constant lists have different lengths;
each precursor group needs exactly one `β_i` and one `λ_i`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `betas` | `usize` | Number of `β_i` values supplied. |
| `lambdas` | `usize` | Number of `λ_i` values supplied. |

###### `NoDelayedGroups`

No delayed groups were supplied; the model needs at least one.

###### `NonPositivePromptGenerationTime`

The prompt-neutron generation time `Λ` was not strictly positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `NonPositiveDecayConstant`

A decay constant `λ_i` was not strictly positive.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `group` | `usize` | Zero-based group index of the offending `λ_i`. |
| `value` | `f64` | The offending value in `1/s`. |

###### `NonPositiveTimeStep`

The requested time step was not strictly positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `SingularSystem`

The implicit linear system was singular (degenerate parameters, e.g. a
`Λ` or `Δt` that produced a zero pivot).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `col` | `usize` | Column at which the LU decomposition found a zero pivot. |

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
    fn clone(self: &Self) -> PointKineticsError { /* ... */ }
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
    fn from(e: PointKineticsError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(e: MatrixError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: PointKineticsError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PointKineticsError) -> bool { /* ... */ }
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
#### Struct `PointKineticsParameters`

Time-invariant kinetics parameters of a reactor: the delayed-neutron data and
the prompt-generation time.

All fields carry `uom` dimensioned quantities. Construct with [`Self::new`]
(subcritical index `ζ` defaults to zero) or [`Self::with_subcritical_index`].

# Valid ranges / assumptions

- At least one delayed group; `β_i > 0` for a physical fraction (not checked
  beyond non-emptiness), `λ_i > 0`.
- `Λ > 0`. Typical values: `Λ ≈ 10⁻⁴ s` (thermal reactors) to `10⁻⁷ s` (fast).
- `β_i` are effective fractions; their sum `β = Σ β_i` is a few ×10⁻³.

```rust
pub struct PointKineticsParameters {
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
  pub fn new(delayed_fractions: Vec<Reactivity>, decay_constants: Vec<DecayConstant>, prompt_generation_time: PromptGenerationTime) -> Result<Self, PointKineticsError> { /* ... */ }
  ```
  Build kinetics parameters with subcritical index `ζ = 0`.

- ```rust
  pub fn with_subcritical_index(delayed_fractions: Vec<Reactivity>, decay_constants: Vec<DecayConstant>, prompt_generation_time: PromptGenerationTime, subcritical_index: Reactivity) -> Result<Self, PointKineticsError> { /* ... */ }
  ```
  Build kinetics parameters with an explicit subcritical index `ζ`.

- ```rust
  pub fn delayed_group_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of delayed-neutron precursor groups `N`.

- ```rust
  pub fn delayed_fractions(self: &Self) -> &[Reactivity] { /* ... */ }
  ```
  The effective delayed-neutron fractions `β_i` (dimensionless), one per

- ```rust
  pub fn decay_constants(self: &Self) -> &[DecayConstant] { /* ... */ }
  ```
  The decay constants `λ_i` (`1/s`), one per group, in group order.

- ```rust
  pub fn prompt_generation_time(self: &Self) -> PromptGenerationTime { /* ... */ }
  ```
  The prompt-neutron generation time `Λ` (seconds).

- ```rust
  pub fn subcritical_index(self: &Self) -> Reactivity { /* ... */ }
  ```
  The subcritical index `ζ` (dimensionless).

- ```rust
  pub fn total_delayed_fraction(self: &Self) -> Reactivity { /* ... */ }
  ```
  Total effective delayed-neutron fraction `β = Σ_i β_i` (dimensionless).

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
    fn clone(self: &Self) -> PointKineticsParameters { /* ... */ }
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
    fn eq(self: &Self, other: &PointKineticsParameters) -> bool { /* ... */ }
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
#### Struct `PointKineticsState`

Instantaneous state of the point-kinetics model: fission power and the
per-group precursor "powers", plus the external source power carried into the
implicit solve.

All quantities are [`Power`] (watts). The precursor `C_i` are carried in power
units (GeN-Foam convention) so they scale directly with the fission power.

```rust
pub struct PointKineticsState {
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
  pub fn new_equilibrium(params: &PointKineticsParameters, fission_power: Power) -> Self { /* ... */ }
  ```
  Construct the **critical steady state** for a given fission power.

- ```rust
  pub fn new(fission_power: Power, precursor_powers: Vec<Power>) -> Self { /* ... */ }
  ```
  Construct a state from explicit fission power and precursor powers.

- ```rust
  pub fn fission_power(self: &Self) -> Power { /* ... */ }
  ```
  Current fission power `P` (watts).

- ```rust
  pub fn precursor_powers(self: &Self) -> &[Power] { /* ... */ }
  ```
  Current precursor powers `C_i` (watts), in group order.

- ```rust
  pub fn external_source_power(self: &Self) -> Power { /* ... */ }
  ```
  External source power `S` (watts) from the most recent [`Self::step`].

- ```rust
  pub fn step(self: &mut Self, params: &PointKineticsParameters, dt: Time, reactivity: Reactivity, external_source_power: Power) -> Result<(), PointKineticsError> { /* ... */ }
  ```
  Advance the state by one implicit (backward-Euler) time step.

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
    fn clone(self: &Self) -> PointKineticsState { /* ... */ }
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
    fn eq(self: &Self, other: &PointKineticsState) -> bool { /* ... */ }
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
## Module `sn`

# Discrete-ordinates (S_N) transport

Rust port of GeN-Foam's `SNNeutronics`. Where [`super::diffusion`] replaces
the angular dependence of the transport equation with Fick's law, the S_N
method keeps it: it solves the transport equation directly on a finite set
of discrete directions (an angular [`quadrature`] set), so it captures
strong flux anisotropy — streaming through voids, deep gradients near strong
absorbers — that diffusion / SP3 smear out. It is the highest-fidelity
deterministic option in the suite.

## The discrete-ordinates equations (from `SNNeutronics.H`)

For energy group `i` and direction `Omega_j` (unit vector, weight `w_j`),
the steady (eigenvalue) angular flux `psi_{i,j}` obeys

```text
  div(Omega_j psi_{i,j}) + Sigma_{t,i} psi_{i,j} = q_i / (4*pi)
```

with the isotropic source (moment 0)

```text
  q_i = chi_i / k sum_{i'} nu Sigma_{f,i'} phi_{i'}
        + sum_{i'} Sigma_{s, i' -> i} phi_{i'} ,
```

where `phi_i = sum_j w_j psi_{i,j}` is the scalar flux and `sum_j w_j =
4*pi` (see [`quadrature`]). The total cross section
`Sigma_{t,i} = Sigma_{r,i} + Sigma_{s, i -> i}` is reconstructed from the
(self-scatter-excluded) removal cross section the shared
[`super::xs`] data stores plus the group's own moment-0 self-scatter — the
scattering source `sum_{i'}` then runs over **all** groups including `i`.
The `div` term is discretised with basic-lib's first-order **upwind**
convection operator
[`fvm::div`](outram_foam_basic_lib::fv_operators::fvm::div) on the face flux
`Omega_j . Sf`, which is the finite-volume transport sweep; the collision
term is [`fvm::sp`](outram_foam_basic_lib::fv_operators::fvm::sp) and the
source is [`fvm::su`](outram_foam_basic_lib::fv_operators::fvm::su). The
resulting per-direction matrix is asymmetric, so it is solved with the
Gauss-Seidel LDU solver (not CG).

## Iteration structure

- **inner** ([`sweep`]) — a scattering-source iteration for one group:
  evaluate the isotropic source at the current scalar flux, solve every
  direction, re-sum the scalar flux, repeat until it stops changing.
- **outer** ([`eigenvalue`]) — the k-eigenvalue power iteration on the
  fission source, identical in structure to the diffusion solver
  (`k <- k F_new / F_old`, renormalise `F = 1`).

## Scope / deferrals

Isotropic (P0) scattering only — the anisotropic Legendre-moment source
(`SNNeutronics`'s `legendreMatrices_`, moments `>= 1`) is deferred, matching
how the [`super::diffusion`] port uses only moment 0. Transients, liquid-fuel
precursor advection, Aitken acceleration, and live thermal-hydraulic feedback
are likewise deferred with the coupling layer. Reflective boundaries are the
`ZeroGradient` flux BC; a vacuum edge is `FixedValue(0)` (zero incoming
angular flux, exact for the upwind operator).

## Two ways to build an `SnNeutronics`

- [`SnNeutronics::new`] — a **scaffold** with an allocated
  [`NeutronicsState`] but no cross sections. [`Self::solve_eigenvalue`] on it
  returns [`NeutronicsError::ModelNotImplemented`]; it exists so the shared
  [`super::NeutronicsModel::Sn`] surface (`power`, `k_eff`, `kind`) works.
- [`SnNeutronics::with_cross_sections`] — the **working** model:
  materialises the cross sections, builds the quadrature, and lets
  [`Self::solve_eigenvalue`] run the real transport eigenvalue solve.

```rust
pub mod sn { /* ... */ }
```

### Modules

## Module `quadrature`

# Angular quadrature — level-symmetric discrete ordinates (S_N)

The discrete-ordinates method replaces the continuous angular variable
`Omega` of the transport equation with a finite set of directions
`Omega_j` (unit vectors) each carrying a weight `w_j`, so an angular
integral becomes a weighted sum:

```text
  int_{4*pi} f(Omega) dOmega  ~=  sum_j w_j f(Omega_j) ,   sum_j w_j = 4*pi .
```

This module builds the **level-symmetric** (`LQ_N`) sets S2, S4 and S6 —
the sets whose direction cosines are invariant under the 48 rotations /
reflections of the octahedral group, so the quadrature is rotationally
unbiased. A set of order `N` has `N (N + 2)` directions in total
(`N (N + 2) / 8` per octant), replicated across the eight sign octants of
`(Omega_x, Omega_y, Omega_z)` exactly as GeN-Foam's `readQuadratureSet.H`
does.

## Weight normalisation

The weights returned by [`AngularQuadrature`] sum to `4*pi`, i.e. they
integrate the constant `f = 1` over the unit sphere to its exact value
`4*pi`. The scalar flux is then `phi = sum_j w_j psi_j` and an isotropic
volumetric source `S` contributes `S / (4*pi)` to each direction's
transport equation. (GeN-Foam instead normalises the weights to sum to `1`
and drops the `1 / (4*pi)`; the two conventions give identical fluxes.)

## 1-D meshes

On a 1-D slab mesh (`create_one_d_mesh`) only the x-normal faces exist, so
the streaming term sees only `Omega_j . Sf = Omega_{j,x} * A`. The 3-D
level-symmetric set therefore acts as an (over-resolved) 1-D set in
`mu = Omega_x`; the scalar-flux sum still integrates the angular flux
correctly because the weights are complete on the sphere.

```rust
pub mod quadrature { /* ... */ }
```

### Types

#### Enum `QuadratureOrder`

The order `N` of a level-symmetric S_N discrete-ordinates set.

Enum dispatch (per the workspace no-`dyn` rule): the closed set of supported
quadrature orders. A higher order resolves the angular flux with more
directions (`N (N + 2)` total) and reduces "ray effects", at proportionally
higher cost. S2 is the coarsest (8 directions, equivalent to diffusion-like
angular resolution); S6 (48 directions) is accurate enough for the smooth
fluxes of the verification benchmarks here.

```rust
pub enum QuadratureOrder {
    S2,
    S4,
    S6,
}
```

##### Variants

###### `S2`

S2 — 8 directions (1 per octant).

###### `S4`

S4 — 24 directions (3 per octant).

###### `S6`

S6 — 48 directions (6 per octant).

##### Implementations

###### Methods

- ```rust
  pub fn n(self: Self) -> usize { /* ... */ }
  ```
  The numeric order `N` (2, 4, 6).

- ```rust
  pub fn direction_count(self: Self) -> usize { /* ... */ }
  ```
  Total number of discrete directions `N (N + 2)`.

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
    fn clone(self: &Self) -> QuadratureOrder { /* ... */ }
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
    fn eq(self: &Self, other: &QuadratureOrder) -> bool { /* ... */ }
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
#### Struct `AngularQuadrature`

A level-symmetric discrete-ordinates quadrature set: unit direction vectors
`Omega_j` and their weights `w_j` (summing to `4*pi`).

Build with [`AngularQuadrature::level_symmetric`]. The directions and
weights are read-only after construction.

```rust
pub struct AngularQuadrature {
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
  pub fn level_symmetric(order: QuadratureOrder) -> Self { /* ... */ }
  ```
  Construct the level-symmetric set of the requested order.

- ```rust
  pub fn order(self: &Self) -> QuadratureOrder { /* ... */ }
  ```
  The quadrature order.

- ```rust
  pub fn direction_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of discrete directions `M = N (N + 2)`.

- ```rust
  pub fn directions(self: &Self) -> &[Vector3] { /* ... */ }
  ```
  The unit direction vectors `Omega_j`.

- ```rust
  pub fn weights(self: &Self) -> &[f64] { /* ... */ }
  ```
  The direction weights `w_j` (sum to `4*pi`).

- ```rust
  pub fn weight_sum(self: &Self) -> f64 { /* ... */ }
  ```
  The sum of all weights (`4*pi` for a consistent set) — the V&V invariant.

- ```rust
  pub fn face_phi(self: &Self, mesh: &Arc<FvMesh>, direction: usize) -> SurfaceScalarField { /* ... */ }
  ```
  Build the face-flux field `facePhi_j = Omega_j . Sf` for direction

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
    fn clone(self: &Self) -> AngularQuadrature { /* ... */ }
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
### Types

#### Struct `SnSettings`

Convergence and linear-solver controls for an S_N solve.

Defaults: `1e-8` outer `k_eff` tolerance, `1e-6` outer flux tolerance, 500
outer (power) iterations; up to 50 inner scattering-source iterations per
group with a `1e-7` relative flux tolerance; and a `1e-9` / 2000-iteration
inner Gauss-Seidel linear solve per direction.

```rust
pub struct SnSettings {
    pub k_tolerance: f64,
    pub flux_tolerance: f64,
    pub max_outer_iterations: usize,
    pub max_inner_iterations: usize,
    pub inner_tolerance: f64,
    pub linear: outram_foam_basic_lib::prelude::SolverSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_tolerance` | `f64` | Outer-loop convergence tolerance on the relative change in `k_eff`. |
| `flux_tolerance` | `f64` | Outer-loop convergence tolerance on the relative L2 change in the flux. |
| `max_outer_iterations` | `usize` | Maximum number of outer power iterations. |
| `max_inner_iterations` | `usize` | Maximum number of inner scattering-source iterations per group per outer<br>iteration. |
| `inner_tolerance` | `f64` | Relative-L2 convergence tolerance on a group's scalar flux between inner<br>scattering-source iterations. |
| `linear` | `outram_foam_basic_lib::prelude::SolverSettings` | Inner linear-solver settings (one direction solve). |

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
    fn clone(self: &Self) -> SnSettings { /* ... */ }
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
#### Struct `SnNeutronics`

A discrete-ordinates (S_N) transport model on an `FvMesh`.

Build a **working** model with [`Self::with_cross_sections`] and run
[`Self::solve_eigenvalue`]; a bare [`Self::new`] is a scaffold whose
`solve_eigenvalue` returns [`NeutronicsError::ModelNotImplemented`] (see the
module docs).

```rust
pub struct SnNeutronics {
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
  pub fn new(mesh: Arc<FvMesh>, energy_groups: usize, prec_groups: usize) -> Self { /* ... */ }
  ```
  Allocate an S_N **scaffold** with `energy_groups` scalar-flux fields and

- ```rust
  pub fn with_cross_sections(mesh: Arc<FvMesh>, xs: &CrossSectionData, zone_of_cell: &[usize], raw_parameters: &[f64], flux_boundary: &[BoundaryCondition<f64>], order: QuadratureOrder, settings: SnSettings) -> Result<Self, NeutronicsError> { /* ... */ }
  ```
  Build a **working** S_N model on `mesh` from cross sections `xs`, a

- ```rust
  pub fn state(self: &Self) -> &NeutronicsState { /* ... */ }
  ```
  The shared neutronics state (flux, precursors, power density, `k_eff`).

- ```rust
  pub fn settings(self: &Self) -> &SnSettings { /* ... */ }
  ```
  The convergence / solver settings.

- ```rust
  pub fn quadrature(self: &Self) -> Option<&AngularQuadrature> { /* ... */ }
  ```
  The angular quadrature, if this is a working model.

- ```rust
  pub fn energy_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups `G`.

- ```rust
  pub fn solve_eigenvalue(self: &mut Self) -> Result<crate::genfoam::neutronics::EigenvalueReport, NeutronicsError> { /* ... */ }
  ```
  Solve the S_N k-eigenvalue problem by outer power iteration.

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
    fn clone(self: &Self) -> SnNeutronics { /* ... */ }
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
### Re-exports

#### Re-export `AngularQuadrature`

```rust
pub use quadrature::AngularQuadrature;
```

#### Re-export `QuadratureOrder`

```rust
pub use quadrature::QuadratureOrder;
```

## Module `sp3`

# Simplified-P3 (SP3) neutron-transport solver

Rust port of GeN-Foam's `SP3Neutronics`. SP3 augments the multigroup
neutron-diffusion machinery ([`crate::genfoam::neutronics::diffusion`]) with
a **second angular moment** of the flux, recovering part of the transport
anisotropy that diffusion theory drops. It is more accurate than diffusion in
optically thin or strongly heterogeneous regions, and — this is the key
verification property — it *reduces to diffusion* wherever the flux is
near-isotropic.

## The two coupled SP3 moment equations (from `fluxEqSP3.H`)

SP3 carries, per energy group `g`, two moment fields:

- the **0th composite moment** `Phi0_g = phi0_g + 2 phi2_g`
  (GeN-Foam `fluxStar_`), and
- the **2nd moment** `phi2_g` (GeN-Foam `fluxStar2_`),

from which the physical scalar flux is reconstructed as
`phi0_g = Phi0_g - 2 phi2_g` (GeN-Foam `flux_ = fluxStar_ - 2 fluxStar2_`,
discontinuity factor 1). The steady (eigenvalue) equations are

```text
  -div(D0_g grad Phi0_g) + Sigma_{r,g} Phi0_g = Q_g + 2 Sigma_{r,g} phi2_g
  -div(D2_g grad phi2_g) + A2_g phi2_g        = (2/3)(Sigma_{r,g} Phi0_g - Q_g)
```

with `D0_g = D_g` the ordinary diffusion coefficient,
`D2_g = (3/7)/Sigma_{t,g}`, `A2_g = (5/3) Sigma_{t,g} + (4/3) Sigma_{r,g}`,
`Sigma_{t,g} = Sigma_{r,g} + Sigma_{s,g->g}`, and the same fission +
in-scatter source as diffusion,
`Q_g = chi_g/k * S_n + S_{s,g}`. The removal in the first equation acts on
the *physical* flux — `Sigma_r Phi0 - 2 Sigma_r phi2 = Sigma_r phi0` — so with
`phi2 -> 0` the first equation is exactly the diffusion equation. In an
infinite medium `phi2` is identically zero, so SP3's `k_inf` equals
diffusion's `k_inf = nu Sigma_f / Sigma_a` exactly (see the V&V tests).

## Two forms

- **k-eigenvalue** ([`Sp3Neutronics::solve_eigenvalue`]) — outer power
  iteration over the coupled moment system (`solveNeutronicsSP3.H` with
  `eigenvalueNeutronics_ = true`, so the time terms drop).
- **transient** ([`Sp3Neutronics::step`]) — one backward-Euler step of the
  coupled moments + delayed precursors at a frozen `k_eff`.

## Boundary conditions

The caller's per-patch boundary condition is applied to **both** moment
fields (`Phi0` and `phi2`): `FixedValue(0)` on both gives the zero-flux
(Mark) vacuum edge `phi0 = 0`; `ZeroGradient` on both gives a reflective
edge. GeN-Foam's Marshak/albedo SP3 boundary (`albedoSP3FvPatchField`), which
couples the two moments at the surface, is a documented deferral — the
zero-flux edge is the standard choice for the homogeneous verification cases
here.

## Scope / deferrals

Same fixed-feedback-point cross sections as diffusion (see
[`fields::Sp3XsFields`]); live TH feedback, discontinuity-factor adjustment,
liquid-fuel precursor advection, Aitken acceleration, the implicit/integral
transient predictors, and the albedo SP3 boundary are deferred with the
coupling layer.

```rust
pub mod sp3 { /* ... */ }
```

### Modules

## Module `fields`

# Mesh-materialised SP3 cross-section fields

The SP3 second-moment equation needs two group constants that the plain
diffusion field set does not carry:

- the **total** cross section `Sigma_t = Sigma_r + Sigma_{s,g->g}` (removal
  plus self-scatter — GeN-Foam writes it as
  `sigmaRemoval[g] + sigmaFromTo[g][g]`), and
- the **second-moment diffusion coefficient** `D_2 = (3/7)/Sigma_t`
  (GeN-Foam `Dalbedo_ = 1/(sigmaRemoval+sigmaFromTo)*3/7` in `fluxEqSP3.H`),
- the **second-moment removal coefficient**
  `A_2 = (5/3) Sigma_t + (4/3) Sigma_r` (the `fvm::Sp` diagonal of the second
  SP3 equation).

Everything else (the 0th-moment diffusion coefficient `D_0 = D`, the removal
`Sigma_r`, the fission / scattering / precursor data) is exactly the diffusion
field set, so this module **wraps**
[`DiffusionXsFields`](crate::genfoam::neutronics::diffusion::DiffusionXsFields)
and adds only the SP3-specific derived fields. It re-uses the diffusion
materialisation verbatim (single fixed feedback point; live feedback is the
same documented deferral as diffusion).

```rust
pub mod fields { /* ... */ }
```

### Types

#### Struct `Sp3XsFields`

The full SP3 cross-section field set on a mesh: the diffusion field set plus
the three second-moment derived fields.

All base-SI units follow [`DiffusionXsFields`]; the added fields are
`sigma_total` in `m^-1`, `d2` (and its face interpolant `d2_face`) in metres,
and `second_moment_removal` in `m^-1`.

```rust
pub struct Sp3XsFields {
    pub base: crate::genfoam::neutronics::diffusion::DiffusionXsFields,
    pub sigma_total: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub d2: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
    pub d2_face: Vec<outram_foam_basic_lib::prelude::SurfaceScalarField>,
    pub second_moment_removal: Vec<outram_foam_basic_lib::prelude::VolScalarField>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `base` | `crate::genfoam::neutronics::diffusion::DiffusionXsFields` | The shared diffusion field set (`D_0`, `Sigma_r`, fission, scattering,<br>precursors, ...). SP3's 0th-moment equation uses these directly. |
| `sigma_total` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Total cross section `Sigma_t = Sigma_r + Sigma_{s,g->g}` per group<br>(`m^-1`). GeN-Foam `sigmaRemoval[g] + sigmaFromTo[g][g]`. |
| `d2` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Second-moment diffusion coefficient `D_2 = (3/7)/Sigma_t` per group (m). |
| `d2_face` | `Vec<outram_foam_basic_lib::prelude::SurfaceScalarField>` | `D_2` interpolated to faces, ready for<br>[`fvm::laplacian`](outram_foam_basic_lib::fv_operators::fvm::laplacian). |
| `second_moment_removal` | `Vec<outram_foam_basic_lib::prelude::VolScalarField>` | Second-moment removal coefficient<br>`A_2 = (5/3) Sigma_t + (4/3) Sigma_r` per group (`m^-1`) — the diagonal<br>of the second SP3 equation. |

##### Implementations

###### Methods

- ```rust
  pub fn materialize(mesh: &Arc<FvMesh>, xs: &CrossSectionData, zone_of_cell: &[usize], raw_parameters: &[f64]) -> Result<Self, XsError> { /* ... */ }
  ```
  Materialise the SP3 cross sections onto `mesh` at the feedback point

- ```rust
  pub fn energy_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups `G`.

- ```rust
  pub fn prec_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of delayed-neutron precursor groups `N`.

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
    fn clone(self: &Self) -> Sp3XsFields { /* ... */ }
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
### Types

#### Struct `Sp3Settings`

Convergence and linear-solver controls for an SP3 solve.

Defaults match [`crate::genfoam::neutronics::DiffusionSettings`]: `1e-8`
outer `k_eff` tolerance, `1e-6` outer flux tolerance, 500 outer (power)
iterations, and a `1e-9` / 2000-iteration inner CG linear solve per moment
equation.

```rust
pub struct Sp3Settings {
    pub k_tolerance: f64,
    pub flux_tolerance: f64,
    pub max_outer_iterations: usize,
    pub linear: outram_foam_basic_lib::prelude::SolverSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_tolerance` | `f64` | Outer-loop convergence tolerance on the relative change in `k_eff`. |
| `flux_tolerance` | `f64` | Outer-loop convergence tolerance on the relative L2 change in the flux. |
| `max_outer_iterations` | `usize` | Maximum number of outer power iterations. |
| `linear` | `outram_foam_basic_lib::prelude::SolverSettings` | Inner linear-solver settings (one moment-equation solve). |

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
    fn clone(self: &Self) -> Sp3Settings { /* ... */ }
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
#### Struct `Sp3Neutronics`

A simplified-P3 (SP3) transport model on an `FvMesh`.

Owns the shared [`NeutronicsState`] (physical flux `phi0` / precursors /
power density / `k_eff`), the SP3 moment fields (`Phi0` and `phi2` per
group), and — once built with [`Self::with_cross_sections`] — the
mesh-materialised SP3 cross sections ([`Sp3XsFields`]).

Two ways to construct:

- [`Self::new`] — a **state-only scaffold** (no cross sections). It exists so
  [`crate::genfoam::neutronics::NeutronicsModel::Sp3`] can hold a value with
  the shared surface (`power`, `k_eff`) working; [`Self::solve_eigenvalue`]
  on such a value returns [`NeutronicsError::ModelNotImplemented`].
- [`Self::with_cross_sections`] — a **solvable** model with cross sections,
  ready for [`Self::solve_eigenvalue`] / [`Self::step`].

```rust
pub struct Sp3Neutronics {
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
  pub fn solve_eigenvalue(self: &mut Self) -> Result<EigenvalueReport, NeutronicsError> { /* ... */ }
  ```
  Solve the SP3 k-eigenvalue problem by outer power iteration.

- ```rust
  pub fn step(self: &mut Self, dt: Time) -> Result<(), NeutronicsError> { /* ... */ }
  ```
  Advance the coupled SP3 moment + precursor system by one implicit

- ```rust
  pub fn new(mesh: Arc<FvMesh>, energy_groups: usize, prec_groups: usize) -> Self { /* ... */ }
  ```
  Allocate a **state-only SP3 scaffold** with `energy_groups` flux fields

- ```rust
  pub fn with_cross_sections(mesh: Arc<FvMesh>, xs: &CrossSectionData, zone_of_cell: &[usize], raw_parameters: &[f64], flux_boundary: &[BoundaryCondition<f64>], settings: Sp3Settings) -> Result<Self, NeutronicsError> { /* ... */ }
  ```
  Build a **solvable** SP3 model on `mesh` from cross sections `xs`, a

- ```rust
  pub fn state(self: &Self) -> &NeutronicsState { /* ... */ }
  ```
  The shared neutronics state (physical flux, precursors, power density,

- ```rust
  pub fn xs_fields(self: &Self) -> Option<&Sp3XsFields> { /* ... */ }
  ```
  The materialised SP3 cross-section fields, if this is a solvable model

- ```rust
  pub fn settings(self: &Self) -> &Sp3Settings { /* ... */ }
  ```
  The convergence / solver settings.

- ```rust
  pub fn flux_star(self: &Self) -> &[VolScalarField] { /* ... */ }
  ```
  The per-group 0th composite moment `Phi0_g = phi0_g + 2 phi2_g`

- ```rust
  pub fn flux_star2(self: &Self) -> &[VolScalarField] { /* ... */ }
  ```
  The per-group 2nd moment `phi2_g` (GeN-Foam `fluxStar2_`), read-only.

- ```rust
  pub fn energy_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups `G`.

- ```rust
  pub fn prec_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of delayed-neutron precursor groups `N`.

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
    fn clone(self: &Self) -> Sp3Neutronics { /* ... */ }
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
### Re-exports

#### Re-export `Sp3XsFields`

```rust
pub use fields::Sp3XsFields;
```

## Module `state`

# Shared spatial neutronics state

The mesh-based state every *spatial* deterministic neutronics model
(diffusion / SP3 / SN) reads and writes: the per-group scalar flux, the
per-group delayed-neutron precursor concentration, the fuel power-density
field, the collapsed one-group flux, and the two scalars the eigenvalue /
transient loops carry — the effective multiplication factor `k_eff` and the
integrated total fission power.

This is the Rust analogue of GeN-Foam's `Foam::neutronics` abstract base
(`keff_`, `powerDensity_`, `oneGroupFlux_`) plus the `flux_` / `prec_`
`PtrList`s that `diffusionNeutronics` adds. The 0-D
[`super::point_kinetics`] model does **not** use this state — it has no
mesh — so the shared state lives here rather than being forced onto every
[`super::NeutronicsModel`] variant.

## Field units (basic-lib fields are bare `f64`, unitful by convention)

`outram_foam_basic_lib`'s [`VolScalarField`] stores plain `f64` per cell, so
the physical unit of each field is a documented convention rather than a
`uom` type. The scalar *summary* quantities that cross the public API
([`Self::k_eff`], [`Self::total_power`]) are returned as named `uom`
aliases.

| Field | Symbol | Unit |
|---|---|---|
| [`Self::flux`] (per group) | `phi_g` | `1/(m^2 s)` (scalar group flux) |
| [`Self::precursors`] (per group) | `C_k` | `1/m^3` (precursor number density) |
| [`Self::power_density`] | `q'''` | `W/m^3` |
| [`Self::one_group_flux`] | `sum_g phi_g` | `1/(m^2 s)` |

```rust
pub mod state { /* ... */ }
```

### Types

#### Type Alias `NeutronMultiplicationFactor`

Effective neutron multiplication factor `k_eff` — dimensionless. Named alias
over `uom`'s [`Ratio`]; `k_eff = 1` is exact criticality, `> 1` supercritical.

```rust
pub type NeutronMultiplicationFactor = uom::si::f64::Ratio;
```

#### Struct `NeutronicsState`

The shared spatial neutronics state: per-group flux and precursors, the
power-density field, the one-group collapsed flux, `k_eff`, and the total
fission power.

Construct with [`Self::new`] (uniform unit flux, zero precursors,
`k_eff = 1`); a spatial model then overwrites the fields as it iterates.
The flux / precursor vectors are indexed by energy / precursor group in the
same order as the [`super::xs::CrossSectionData`] group constants.

```rust
pub struct NeutronicsState {
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
  pub fn new(mesh: Arc<FvMesh>, energy_groups: usize, prec_groups: usize) -> Self { /* ... */ }
  ```
  Allocate the state for `energy_groups` flux fields and `prec_groups`

- ```rust
  pub fn mesh(self: &Self) -> &Arc<FvMesh> { /* ... */ }
  ```
  The mesh all fields are defined on.

- ```rust
  pub fn energy_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups `G`.

- ```rust
  pub fn prec_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of delayed-neutron precursor groups `N`.

- ```rust
  pub fn flux(self: &Self) -> &[VolScalarField] { /* ... */ }
  ```
  The per-group scalar flux fields `phi_g` (`1/(m^2 s)`), read-only.

- ```rust
  pub fn flux_mut(self: &mut Self) -> &mut Vec<VolScalarField> { /* ... */ }
  ```
  The per-group flux fields, mutable (for a solver to overwrite in place).

- ```rust
  pub fn precursors(self: &Self) -> &[VolScalarField] { /* ... */ }
  ```
  The per-group precursor concentration fields `C_k` (`1/m^3`), read-only.

- ```rust
  pub fn precursors_mut(self: &mut Self) -> &mut Vec<VolScalarField> { /* ... */ }
  ```
  The per-group precursor fields, mutable.

- ```rust
  pub fn power_density(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  The fuel power-density field `q'''` (`W/m^3`), read-only.

- ```rust
  pub fn power_density_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
  The fuel power-density field, mutable.

- ```rust
  pub fn one_group_flux(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  The energy-collapsed one-group flux `sum_g phi_g` (`1/(m^2 s)`).

- ```rust
  pub fn one_group_flux_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
  The one-group flux field, mutable.

- ```rust
  pub fn k_eff(self: &Self) -> NeutronMultiplicationFactor { /* ... */ }
  ```
  The effective multiplication factor `k_eff` (dimensionless).

- ```rust
  pub fn set_k_eff(self: &mut Self, k_eff: NeutronMultiplicationFactor) { /* ... */ }
  ```
  Overwrite `k_eff`.

- ```rust
  pub fn k_eff_raw(self: &Self) -> f64 { /* ... */ }
  ```
  The raw `k_eff` value (dimensionless `f64`) — the numerical working copy

- ```rust
  pub fn set_k_eff_raw(self: &mut Self, k_eff: f64) { /* ... */ }
  ```
  Overwrite the raw `k_eff` value.

- ```rust
  pub fn total_power(self: &Self) -> Power { /* ... */ }
  ```
  The domain-integrated total fission power `P` (watts).

- ```rust
  pub fn set_total_power_raw(self: &mut Self, watts: f64) { /* ... */ }
  ```
  Overwrite the total power (raw watts).

- ```rust
  pub fn total_power_raw(self: &Self) -> f64 { /* ... */ }
  ```
  The raw total-power value (watts, `f64`).

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
    fn clone(self: &Self) -> NeutronicsState { /* ... */ }
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
## Module `xs`

# `genfoam::neutronics::xs` — deterministic multigroup cross-section data

Rust port of `GeN-Foam/src/classes/neutronics/XS` — the cross-section data
model the deterministic neutronics solvers (diffusion / SP3 / SN) consume.
It stores per-material-zone multigroup constants and interpolates them to the
current reactor feedback conditions (fuel/coolant temperature, coolant
density, expansion, ...).

Cross sections are read from GeN-Foam `nuclearData` dictionaries and are
**deterministic and self-contained**: this module does not depend on
`njoy-outram-park-fork` or `outram-mc-libs`.

## What lives here

- [`CrossSectionData`] — the top-level store: energy/precursor group counts,
  the ordered feedback variables ([`XsVariable`]), and one
  [`ZoneNuclearData`] per material zone. Built from a parsed
  [`NuclearDataInput`] and queried for typed [`GroupConstants`] /
  [`PrecursorConstants`] / a scattering [`SquareMatrix`].
- [`ZoneNuclearData`] — one zone's full group-wise data and constants.
- [`NuclearDataOneEnergy`] — one interpolated scalar value across states.
  Its polyharmonic-spline RBF kernel lives in
  [`crate::genfoam::common::rbf`] (shared with `multi_region`).
- [`units`] — named `uom` aliases (`MacroscopicCrossSection`,
  `DiffusionCoefficient`, `InverseVelocity`, ...).
- [`input`] — the plain-data mirror of the `nuclearData` dictionary.

## The parametrisation, end to end

A `nuclearData` file lists reactor *states*: a mandatory `reference` state
plus perturbed states (`Tfuel1200K`, `TfuelAndRhoHot`, ...). Each state gives
the cross sections at a point in feedback-parameter space. This module fits a
polyharmonic spline through those points per (zone, energy group, quantity),
so a solver can ask for the cross sections at *arbitrary* feedback conditions
via [`CrossSectionData::evaluate_group_constants`]. Each variable's raw value
is passed through its [`variables::VariableLaw`] (`lin` / `log` / `sqrt`)
first, so the fit is done in a linearising space (e.g. `ln(T_fuel)` for
fast-spectrum Doppler).

## Scope of this port — data model only

This is the **mesh-free data layer**: the zone-level interpolators and their
evaluation at a feedback point. GeN-Foam additionally *materialises* these
onto an `fvMesh` (filling one `volScalarField` per group by looping over the
cells of each `cellZone`), and handles control-rod driveline motion and
discontinuity-factor flux adjustment — all of which need the mesh /
multi-region layers and so are not implemented *in this module*. Mesh
materialisation has since landed next door, in
[`DiffusionXsFields`](crate::genfoam::neutronics::diffusion::DiffusionXsFields)
and its SP3 counterpart. **Control-rod driveline motion and
discontinuity-factor adjustment remain deferred** (tracked in beads under
the `neutronics` epic). The evaluation methods below are exactly the per-cell `.get(...)` call
GeN-Foam performs in `setNeutronicsVariables.H`, lifted out of the cell loop.

## Dependency on `genfoam::common`

The polyharmonic-spline RBF kernel this module fits with lives in
[`crate::genfoam::common::rbf`], shared with
[`crate::genfoam::multi_region::rbf_mapping`] so there is a single
implementation of the algorithm.

```rust
pub mod xs { /* ... */ }
```

### Modules

## Module `error`

Error type for building and querying the cross-section data model.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `XsError`

Failure while constructing or evaluating a [`super::CrossSectionData`].

These correspond to the `FatalError` checks GeN-Foam performs while reading
a `nuclearData` dictionary, plus the numerical failure of the RBF solve.
Errors are always propagated — never papered over with a default value.

```rust
pub enum XsError {
    NoStates,
    ReferenceStateNotFirst {
        found: String,
    },
    MissingReferenceVariable {
        name: String,
    },
    UnknownVariableLaw {
        name: String,
        law: String,
    },
    LengthMismatch {
        field: &'static str,
        zone: String,
        found: usize,
        expected: usize,
    },
    MissingZoneConstants {
        zone: String,
    },
    IndexOutOfRange {
        what: &'static str,
        index: usize,
        count: usize,
    },
    SingularRbfMatrix {
        detail: String,
    },
}
```

##### Variants

###### `NoStates`

The `states` list was empty. At least the mandatory `reference` state is
required.

###### `ReferenceStateNotFirst`

The first state was not named `reference`. GeN-Foam requires the
reference state first so its values seed every interpolator.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `found` | `String` | The name actually found in first position. |

###### `MissingReferenceVariable`

A feedback variable declared in `xsVariables` had no value in the
`reference` state (its reference value is mandatory).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The offending variable name. |

###### `UnknownVariableLaw`

An `xsVariables` law keyword was not one of `lin` / `log` / `sqrt`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The variable whose law failed to parse. |
| `law` | `String` | The unrecognised law keyword. |

###### `LengthMismatch`

A per-group or per-precursor data array had the wrong length.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `&'static str` | Which array (e.g. `"nuSigmaEff"`, `"Beta"`, `"scatteringMatrixP0"`). |
| `zone` | `String` | The zone the array belongs to. |
| `found` | `usize` | Length that was supplied. |
| `expected` | `usize` | Length that was required. |

###### `MissingZoneConstants`

The reference state did not define the constant data (IV, Beta, lambda,
discFactor, integralFlux, ...) for one of its zones.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `zone` | `String` | The zone lacking constants. |

###### `IndexOutOfRange`

A zone or energy-group index passed to an accessor was out of range.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `what` | `&'static str` | What was indexed (`"zone"`, `"energy group"`, `"Legendre moment"`). |
| `index` | `usize` | The offending index. |
| `count` | `usize` | The number of available items. |

###### `SingularRbfMatrix`

The polyharmonic-spline saddle-point solve failed (singular matrix,
typically duplicate reference-state coordinates).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `detail` | `String` | Detail from the underlying dense solver. |

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
    fn from(source: xs::XsError) -> Self { /* ... */ }
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
## Module `group_constants`

# Typed group-constant and precursor views

These are the `uom`-typed values a deterministic multigroup solver
(diffusion / SP3 / SN) reads out of the cross-section store after
interpolation. They sit at the boundary where the unit-free interpolation
core hands data to physics code, so every field is a named, dimension-checked
quantity from [`super::units`].

The scattering matrix is returned separately (as an
`outram_foam_basic_lib::matrix::SquareMatrix`) by
[`super::CrossSectionData::scattering_matrix`], because its size
(`energy_groups x energy_groups`) and per-Legendre-moment indexing make a
dense matrix the natural container.

```rust
pub mod group_constants { /* ... */ }
```

### Types

#### Struct `GroupConstants`

The multigroup constants for **one energy group** in one material zone at one
feedback state.

Every field carries its physical dimension; see [`super::units`] for the base
SI units. Assembled by [`super::CrossSectionData::evaluate_group_constants`]
(feedback-interpolated) or
[`super::CrossSectionData::reference_group_constants`] (reference values).

```rust
pub struct GroupConstants {
    pub diffusion_coefficient: crate::genfoam::neutronics::xs::units::DiffusionCoefficient,
    pub nu_sigma_f: crate::genfoam::neutronics::xs::units::MacroscopicCrossSection,
    pub sigma_pow: crate::genfoam::neutronics::xs::units::EnergyReleaseCrossSection,
    pub sigma_removal: crate::genfoam::neutronics::xs::units::MacroscopicCrossSection,
    pub chi_prompt: crate::genfoam::neutronics::xs::units::Dimensionless,
    pub chi_delayed: crate::genfoam::neutronics::xs::units::Dimensionless,
    pub inverse_velocity: crate::genfoam::neutronics::xs::units::InverseVelocity,
    pub disc_factor: crate::genfoam::neutronics::xs::units::Dimensionless,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `diffusion_coefficient` | `crate::genfoam::neutronics::xs::units::DiffusionCoefficient` | Diffusion coefficient `D_g` (m). |
| `nu_sigma_f` | `crate::genfoam::neutronics::xs::units::MacroscopicCrossSection` | Fission-neutron production cross section `nu Sigma_{f,g}` (m^-1). |
| `sigma_pow` | `crate::genfoam::neutronics::xs::units::EnergyReleaseCrossSection` | Fission energy-release cross section `kappa Sigma_{f,g}` (J/m). |
| `sigma_removal` | `crate::genfoam::neutronics::xs::units::MacroscopicCrossSection` | Removal cross section `Sigma_{r,g}` (m^-1):<br>absorption + out-scatter from group `g`. |
| `chi_prompt` | `crate::genfoam::neutronics::xs::units::Dimensionless` | Prompt fission emission spectrum `chi_{p,g}` (dimensionless). |
| `chi_delayed` | `crate::genfoam::neutronics::xs::units::Dimensionless` | Delayed fission emission spectrum `chi_{d,g}` (dimensionless). |
| `inverse_velocity` | `crate::genfoam::neutronics::xs::units::InverseVelocity` | Inverse neutron speed `1/v_g` (s/m). |
| `disc_factor` | `crate::genfoam::neutronics::xs::units::Dimensionless` | Discontinuity factor `gamma_g` (dimensionless). |

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
    fn clone(self: &Self) -> GroupConstants { /* ... */ }
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
#### Struct `PrecursorConstants`

The delayed-neutron precursor constants for one material zone.

Precursor data are feedback-independent in GeN-Foam (read once from the
reference state), so there is a single value per zone rather than one per
feedback point. `beta` and `lambda` have one entry per precursor group.

```rust
pub struct PrecursorConstants {
    pub beta: Vec<crate::genfoam::neutronics::xs::units::Dimensionless>,
    pub beta_tot: crate::genfoam::neutronics::xs::units::Dimensionless,
    pub lambda: Vec<crate::genfoam::neutronics::xs::units::PrecursorDecayConstant>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `beta` | `Vec<crate::genfoam::neutronics::xs::units::Dimensionless>` | Per-group delayed-neutron fractions `beta_k` (dimensionless). |
| `beta_tot` | `crate::genfoam::neutronics::xs::units::Dimensionless` | Total delayed-neutron fraction `beta_tot = sum_k beta_k` (dimensionless). |
| `lambda` | `Vec<crate::genfoam::neutronics::xs::units::PrecursorDecayConstant>` | Per-group precursor decay constants `lambda_k` (s^-1). |

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
    fn clone(self: &Self) -> PrecursorConstants { /* ... */ }
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
## Module `input`

# Plain-data mirror of the `nuclearData` dictionary

GeN-Foam reads its cross sections from an OpenFOAM `IOdictionary` named
`nuclearData` (a `states` list, each with a `zones` list). This crate keeps
**dictionary parsing** in the `io` layer; the structs here are the parsed,
in-memory representation that [`super::CrossSectionData::from_input`]
consumes. They are deterministic and self-contained — no mesh, no OpenFOAM
I/O, no NJOY/Monte-Carlo data.

## Schema (mirrors the upstream `nuclearData` file)

- [`NuclearDataInput`] — top-level scalars, the `xsVariables` list, and the
  ordered `states`.
- [`StateInput`] — one reactor state (`reference` first), its perturbed
  feedback-parameter values, and its per-zone data.
- [`ZoneStateInput`] — the group-wise **variable** cross sections for one
  zone in one state, plus (reference state only) the zone's constant data.
- [`ZoneConstantsInput`] — the feedback-independent per-zone constants
  (`IV`, `Beta`, `lambda`, `discFactor`, ...), read from the reference state.

All cross sections are in MKSA (per-metre) units, matching the `nuclearData`
convention.

```rust
pub mod input { /* ... */ }
```

### Types

#### Struct `NuclearDataInput`

Top-level parsed `nuclearData` dictionary.

```rust
pub struct NuclearDataInput {
    pub energy_groups: usize,
    pub prec_groups: usize,
    pub legendre_moments: usize,
    pub poly_spline_mode: usize,
    pub fast_neutrons: bool,
    pub xs_variables: Vec<crate::genfoam::neutronics::xs::variables::XsVariable>,
    pub do_not_parametrize: Vec<usize>,
    pub states: Vec<StateInput>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `energy_groups` | `usize` | Number of energy groups `G`. |
| `prec_groups` | `usize` | Number of delayed-neutron precursor groups. |
| `legendre_moments` | `usize` | Number of stored Legendre scattering moments, `1 + legendreMoments`<br>(GeN-Foam always stores at least the `P0` moment, hence the `+1`; supply<br>the already-incremented count here). |
| `poly_spline_mode` | `usize` | Polyharmonic-spline mode (basis order), GeN-Foam `polyharmonicSplineMode`<br>(default `1`). |
| `fast_neutrons` | `bool` | Fast-spectrum flag (`fastNeutrons`). Retained as metadata; in current<br>GeN-Foam the Doppler transform is chosen per variable via its<br>[`super::variables::VariableLaw`] (`log` vs `sqrt`), not by this flag. |
| `xs_variables` | `Vec<crate::genfoam::neutronics::xs::variables::XsVariable>` | Ordered feedback variables (`xsVariables`). Their order fixes the layout<br>of every parameter vector used thereafter. |
| `do_not_parametrize` | `Vec<usize>` | Energy-group indices excluded from parametrisation (`doNotParametrize`):<br>these groups always return their reference cross sections. |
| `states` | `Vec<StateInput>` | Reactor states in dictionary order; `states[0]` must be `reference`. |

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
    fn clone(self: &Self) -> NuclearDataInput { /* ... */ }
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
#### Struct `StateInput`

One reactor state (`reference` or a perturbed state).

```rust
pub struct StateInput {
    pub name: String,
    pub parameters: std::collections::BTreeMap<String, f64>,
    pub zones: Vec<ZoneStateInput>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | State name; the first state must be `"reference"`. |
| `parameters` | `std::collections::BTreeMap<String, f64>` | Perturbed feedback-parameter values, keyed by variable name (raw physical<br>value, *before* any transform law). Missing variables default to the<br>reference-state value. |
| `zones` | `Vec<ZoneStateInput>` | Per-zone data for this state. |

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
    fn clone(self: &Self) -> StateInput { /* ... */ }
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
#### Struct `ZoneStateInput`

The cross-section data for one material zone within one state.

```rust
pub struct ZoneStateInput {
    pub name: String,
    pub d: Vec<f64>,
    pub nu_sigma_eff: Vec<f64>,
    pub sigma_pow: Vec<f64>,
    pub sigma_removal: Vec<f64>,
    pub chi_prompt: Vec<f64>,
    pub chi_delayed: Vec<f64>,
    pub scattering: Vec<Vec<Vec<f64>>>,
    pub constants: Option<ZoneConstantsInput>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Cell-zone name (must match a zone declared in the reference state). |
| `d` | `Vec<f64>` | Diffusion coefficient `D_g`, length `energy_groups` (m). |
| `nu_sigma_eff` | `Vec<f64>` | Fission-neutron production `nu Sigma_{f,g}`, length `energy_groups`<br>(m^-1). |
| `sigma_pow` | `Vec<f64>` | Fission energy release `kappa Sigma_{f,g}`, length `energy_groups`<br>(J/m). |
| `sigma_removal` | `Vec<f64>` | Removal cross section `Sigma_{r,g}`, length `energy_groups` (m^-1). |
| `chi_prompt` | `Vec<f64>` | Prompt spectrum `chi_{p,g}`, length `energy_groups` (dimensionless). |
| `chi_delayed` | `Vec<f64>` | Delayed spectrum `chi_{d,g}`, length `energy_groups` (dimensionless). |
| `scattering` | `Vec<Vec<Vec<f64>>>` | Scattering matrices, one per Legendre moment. `scattering[moment][j][i]`<br>is `Sigma_{s, j -> i}` for moment `moment`; each matrix is<br>`energy_groups x energy_groups` and there must be `legendre_moments` of<br>them (`scatteringMatrixP0`, `scatteringMatrixP1`, ...). |
| `constants` | `Option<ZoneConstantsInput>` | Feedback-independent constants for this zone. **Required in the reference<br>state**, ignored (may be `None`) in perturbed states. |

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
    fn clone(self: &Self) -> ZoneStateInput { /* ... */ }
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
#### Struct `ZoneConstantsInput`

The feedback-independent constant data for one zone, read from the reference
state (GeN-Foam reads these only in the reference-zone pass).

```rust
pub struct ZoneConstantsInput {
    pub fuel_fraction: f64,
    pub secondary_power_volume_fraction: f64,
    pub fraction_to_secondary_power: f64,
    pub df_adjust: bool,
    pub iv: Vec<f64>,
    pub disc_factor: Vec<f64>,
    pub integral_flux: Vec<f64>,
    pub beta: Vec<f64>,
    pub lambda: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_fraction` | `f64` | Fuel volume fraction `alpha_fuel` per lattice volume (default `1`). |
| `secondary_power_volume_fraction` | `f64` | Volume fraction of secondary power-producing structure (default `1`). |
| `fraction_to_secondary_power` | `f64` | Fraction of total power going to the secondary structure (default `0`). |
| `df_adjust` | `bool` | Whether discontinuity factors are adjusted for this zone (`dfAdjust`,<br>default `true`). |
| `iv` | `Vec<f64>` | Inverse velocity `1/v_g`, length `energy_groups` (s/m). |
| `disc_factor` | `Vec<f64>` | Discontinuity factors `gamma_g`, length `energy_groups` (dimensionless). |
| `integral_flux` | `Vec<f64>` | Integral fluxes `Phi_g` used to adapt discontinuity factors, length<br>`energy_groups`. |
| `beta` | `Vec<f64>` | Delayed-neutron fractions `beta_k`, length `prec_groups` (dimensionless). |
| `lambda` | `Vec<f64>` | Precursor decay constants `lambda_k`, length `prec_groups` (s^-1). |

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
    fn clone(self: &Self) -> ZoneConstantsInput { /* ... */ }
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
    The GeN-Foam defaults for the scalar factors, with empty group/precursor

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
## Module `nuclear_data_one_energy`

# `NuclearDataOneEnergy` — one interpolated cross-section value

The atom of GeN-Foam's cross-section store: it holds one scalar nuclear-data
value (e.g. `nu Sigma_f` for a single energy group in a single material
zone) sampled across every perturbed reactor state, and interpolates it to
arbitrary feedback conditions with a polyharmonic-spline RBF
([`crate::genfoam::common::rbf`]).

A [`super::zone_data::ZoneNuclearData`] owns a grid of these — one per
energy group per quantity, plus the full scattering matrix.

## Reduced-parameter handling

Only parameters that actually **vary** across the supplied states can be
interpolated over; a parameter that is identical in every state would make
the RBF saddle-point matrix singular. [`NuclearDataOneEnergy::build`]
therefore drops constant parameters, keeping a reduced coordinate table.
If fewer than two data points exist, or no parameter varies, the value is
treated as constant and [`NuclearDataOneEnergy::get`] returns the reference
(first) value — faithfully mirroring upstream `nuclearDataOneEnergy`.

```rust
pub mod nuclear_data_one_energy { /* ... */ }
```

### Types

#### Struct `NuclearDataOneEnergy`

Interpolator for a single scalar nuclear-data value across perturbed states.

All values are bare `f64` in the quantity's own base-SI units; physical
dimensions are re-attached at the [`super::group_constants`] boundary. The
parameter vectors passed to [`Self::add_data`] and [`Self::get`] are already
**transformed** (each variable's [`super::variables::VariableLaw`] applied)
and aligned to the owning [`super::CrossSectionData`]'s variable order.

```rust
pub struct NuclearDataOneEnergy {
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
  pub fn new(poly_spline_mode: usize, n_parameters: usize) -> Self { /* ... */ }
  ```
  Create an empty interpolator for `n_parameters` feedback variables using

- ```rust
  pub fn add_data(self: &mut Self, value: f64, parameters: &[f64]) { /* ... */ }
  ```
  Append one data point: the value `value` observed at the (already

- ```rust
  pub fn build(self: &mut Self) -> Result<(), XsError> { /* ... */ }
  ```
  Assemble the interpolant from the accumulated data points.

- ```rust
  pub fn get_ref(self: &Self) -> f64 { /* ... */ }
  ```
  The reference (first-registered) value, ignoring all feedback.

- ```rust
  pub fn get(self: &Self, parameters: &[f64], is_parametrize: bool) -> f64 { /* ... */ }
  ```
  Interpolate the value at the (already transformed) parameter point

- ```rust
  pub fn data_len(self: &Self) -> usize { /* ... */ }
  ```
  Number of data points (perturbed states) registered.

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
    fn clone(self: &Self) -> NuclearDataOneEnergy { /* ... */ }
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
## Module `units`

# Named `uom` aliases for multigroup cross-section quantities

GeN-Foam's `nuclearData` header states that all nuclear data are provided in
**MKSA units, not in cm** — i.e. macroscopic cross sections are per **metre**
(`m^-1`), lengths in metres. This module gives each group constant a named,
dimension-checked [`uom`] type so a reader hovering in their editor sees
`MacroscopicCrossSection`, not a raw `Quantity<ISQ<...>>`.

## Why the interpolation core is unit-free

The radial-basis-function interpolator ([`crate::genfoam::common::rbf`]) and the
per-quantity store ([`super::nuclear_data_one_energy`]) operate on bare
`f64`. They are dimension-agnostic numerical primitives: a single
interpolator maps a **heterogeneous** parameter vector — fuel temperature
(K), coolant density (kg/m^3), axial/radial expansion (dimensionless) — onto
one scalar of whatever physical dimension the quantity happens to have. No
single `uom` type can describe that mixed input space, exactly as
`outram_foam_basic_lib`'s `SquareMatrix` and `interpolate_xy` are unit-free.
Physical units are re-attached at the typed accessor boundary
([`super::group_constants`]), which is where a solver meets the data.

## Convention for the helper constructors

Each `fn` below takes a value already expressed in **base SI** and wraps it
in the corresponding `uom` type. The base SI units are spelled out in each
doc comment. Read a value back with the quantity's `.value` field, which is
likewise the base-SI magnitude.

```rust
pub mod units { /* ... */ }
```

### Types

#### Type Alias `DiffusionCoefficient`

Diffusion coefficient `D_g` of energy group `g` — **base SI: metre (m)**.

Multigroup diffusion theory writes the current as `J_g = -D_g grad(phi_g)`;
with `phi_g` a group flux (`m^-2 s^-1`) the coefficient carries a length so
that `D_g grad(phi_g)` has flux-per-area units. Aliased to [`uom`]'s
`Length`.

```rust
pub type DiffusionCoefficient = uom::si::f64::Length;
```

#### Type Alias `MacroscopicCrossSection`

A macroscopic cross section `Sigma` — **base SI: per metre (m^-1)**.

Used for the removal cross section `Sigma_{r,g}`, the fission-neutron
production cross section `nu Sigma_{f,g}`, and each scattering-matrix entry
`Sigma_{s, g->g'}`. Macroscopic (already multiplied by number density), so
the natural dimension is inverse length. Aliased to [`uom`]'s
`LinearNumberDensity` (`L^-1`).

```rust
pub type MacroscopicCrossSection = uom::si::f64::LinearNumberDensity;
```

#### Type Alias `EnergyReleaseCrossSection`

Fission energy-release cross section `kappa Sigma_{f,g}` —
**base SI: joule per metre (J/m = kg m s^-2)**.

Its product with a scalar group flux `phi_g` (`m^-2 s^-1`, neutrons treated
as a dimensionless count) yields a volumetric power density
(`W/m^3 = kg m^-1 s^-3`), hence dimension `L M T^-2`.

```rust
pub type EnergyReleaseCrossSection = uom::si::Quantity<uom::si::ISQ<uom::typenum::P1, uom::typenum::P1, uom::typenum::N2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

#### Type Alias `InverseVelocity`

Inverse neutron speed `1/v_g` of energy group `g` —
**base SI: second per metre (s/m = L^-1 T)**.

The time-dependent multigroup balance carries `(1/v_g) d(phi_g)/dt`; the
inverse speed sets the group's kinetic time scale. There is no standard
named `uom` quantity for `s/m`, so it is defined here from the ISQ base.

```rust
pub type InverseVelocity = uom::si::Quantity<uom::si::ISQ<uom::typenum::N1, uom::typenum::Z0, uom::typenum::P1, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

#### Type Alias `PrecursorDecayConstant`

Delayed-neutron precursor decay constant `lambda_k` —
**base SI: per second (s^-1)**.

Group `k`'s precursors decay as `exp(-lambda_k t)`. Dimensionally a
frequency, so aliased to [`uom`]'s `Frequency`.

```rust
pub type PrecursorDecayConstant = uom::si::f64::Frequency;
```

#### Type Alias `Dimensionless`

A dimensionless nuclear-data ratio — the fission spectra `chi_{p,g}` /
`chi_{d,g}`, delayed fractions `beta_k` / `beta_tot`, and discontinuity
factors `gamma_g`. Aliased to [`uom`]'s `Ratio`.

```rust
pub type Dimensionless = uom::si::f64::Ratio;
```

### Functions

#### Function `diffusion_coefficient`

**Attributes:**

- `MustUse { reason: None }`

Wrap a base-SI value (`m`) as a [`DiffusionCoefficient`].

```rust
pub const fn diffusion_coefficient(metres: f64) -> DiffusionCoefficient { /* ... */ }
```

#### Function `macroscopic_cross_section`

**Attributes:**

- `MustUse { reason: None }`

Wrap a base-SI value (`m^-1`) as a [`MacroscopicCrossSection`].

```rust
pub const fn macroscopic_cross_section(per_metre: f64) -> MacroscopicCrossSection { /* ... */ }
```

#### Function `energy_release_cross_section`

**Attributes:**

- `MustUse { reason: None }`

Wrap a base-SI value (`J/m`) as an [`EnergyReleaseCrossSection`].

```rust
pub const fn energy_release_cross_section(joule_per_metre: f64) -> EnergyReleaseCrossSection { /* ... */ }
```

#### Function `inverse_velocity`

**Attributes:**

- `MustUse { reason: None }`

Wrap a base-SI value (`s/m`) as an [`InverseVelocity`].

```rust
pub const fn inverse_velocity(second_per_metre: f64) -> InverseVelocity { /* ... */ }
```

#### Function `precursor_decay_constant`

**Attributes:**

- `MustUse { reason: None }`

Wrap a base-SI value (`s^-1`) as a [`PrecursorDecayConstant`].

```rust
pub const fn precursor_decay_constant(per_second: f64) -> PrecursorDecayConstant { /* ... */ }
```

#### Function `dimensionless`

**Attributes:**

- `MustUse { reason: None }`

Wrap a bare ratio as a [`Dimensionless`] quantity.

```rust
pub const fn dimensionless(ratio: f64) -> Dimensionless { /* ... */ }
```

## Module `variables`

# Feedback parameters (`xsVariables`) and their transform laws

A GeN-Foam `nuclearData` file declares an `xsVariables` sub-dictionary that
names the feedback parameters the cross sections depend on and, for each, a
**transform law** applied to the raw physical value *before* it enters the
polyharmonic-spline interpolation. Choosing the law linearises the physical
dependence so a low-order spline fits well:

- `lin`  — identity; use for quantities the XS vary linearly in
  (coolant density, expansion).
- `log`  — natural log; the classic **fast-spectrum Doppler** law, where
  resonance absorption varies as `ln(T_fuel)`.
- `sqrt` — square root; the **thermal-spectrum Doppler** law, `sqrt(T_fuel)`.

The same law is applied both to the stored reference-state coordinates and to
the query point at evaluation time, so the interpolation is carried out
entirely in transformed space (see [`super::CrossSectionData`]).

```rust
pub mod variables { /* ... */ }
```

### Types

#### Enum `VariableLaw`

The transform applied to a feedback parameter before interpolation.

Parsed from the `xsVariables` law keyword; see the module documentation for
the physical meaning of each.

```rust
pub enum VariableLaw {
    Linear,
    Log,
    Sqrt,
}
```

##### Variants

###### `Linear`

`lin` — identity, `x -> x`.

###### `Log`

`log` — natural logarithm, `x -> ln(x)` (fast-spectrum Doppler).

###### `Sqrt`

`sqrt` — square root, `x -> sqrt(x)` (thermal-spectrum Doppler).

##### Implementations

###### Methods

- ```rust
  pub fn from_keyword(keyword: &str) -> Option<Self> { /* ... */ }
  ```
  Parse a GeN-Foam law keyword (`"lin"`, `"log"`, `"sqrt"`).

- ```rust
  pub fn keyword(self: Self) -> &'static str { /* ... */ }
  ```
  The GeN-Foam keyword string for this law.

- ```rust
  pub fn transform(self: Self, value: f64) -> f64 { /* ... */ }
  ```
  Apply the transform to a raw physical parameter value.

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
    fn clone(self: &Self) -> VariableLaw { /* ... */ }
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
    fn eq(self: &Self, other: &VariableLaw) -> bool { /* ... */ }
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
#### Struct `XsVariable`

One feedback parameter declared in the `xsVariables` dictionary.

The order of these within [`super::CrossSectionData`] defines the layout of
every parameter vector passed to the interpolators and to the public
evaluation methods.

```rust
pub struct XsVariable {
    pub name: String,
    pub law: VariableLaw,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Variable name as it appears in the `nuclearData` state dictionaries<br>(e.g. `"Tfuel"`, `"rhoCool"`, `"axExp"`). |
| `law` | `VariableLaw` | Transform law applied to this variable's value before interpolation. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, law: VariableLaw) -> Self { /* ... */ }
  ```
  Construct a feedback variable from its name and law.

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
    fn clone(self: &Self) -> XsVariable { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &XsVariable) -> bool { /* ... */ }
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
## Module `zone_data`

# `ZoneNuclearData` — one material zone's full cross-section set

Ports the per-`cellZone` data lists of GeN-Foam's `XS` class: for a single
material region it holds one [`NuclearDataOneEnergy`] interpolator per energy
group for each variable quantity (`D`, `nu Sigma_f`, `kappa Sigma_f`,
`Sigma_r`, `chi_p`, `chi_d`), the full per-Legendre-moment scattering matrix
of interpolators, and the feedback-independent constants (`1/v`, `beta`,
`lambda`, discontinuity factors, fuel/secondary-power fractions).

Everything here is bare `f64` in base-SI units; the `uom` wrapping happens in
[`super::CrossSectionData`]. Zones are assembled from the reference state,
fed one data point per reactor state, then [`ZoneNuclearData::build`] solves
every interpolator.

```rust
pub mod zone_data { /* ... */ }
```

### Types

#### Struct `ZoneNuclearData`

A single-material zone's group-wise cross sections and precursor constants.

```rust
pub struct ZoneNuclearData {
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
  pub fn new_reference(zone: &ZoneStateInput, energy_groups: usize, prec_groups: usize, legendre_moments: usize, poly_spline_mode: usize, n_parameters: usize) -> Result<Self, XsError> { /* ... */ }
  ```
  Build a zone's empty interpolator grid and its constants from the

- ```rust
  pub fn add_state_data(self: &mut Self, zone: &ZoneStateInput, parameters: &[f64]) -> Result<(), XsError> { /* ... */ }
  ```
  Register one reactor state's variable cross sections as a data point,

- ```rust
  pub fn build(self: &mut Self) -> Result<(), XsError> { /* ... */ }
  ```
  Solve every interpolator in this zone.

- ```rust
  pub fn name(self: &Self) -> &str { /* ... */ }
  ```
  The zone's cell-zone name.

- ```rust
  pub fn energy_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups.

- ```rust
  pub fn prec_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of precursor groups.

- ```rust
  pub fn legendre_moments(self: &Self) -> usize { /* ... */ }
  ```
  Number of stored Legendre scattering moments.

- ```rust
  pub fn fuel_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Fuel volume fraction `alpha_fuel`.

- ```rust
  pub fn secondary_power_volume_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Volume fraction of secondary power-producing structure.

- ```rust
  pub fn fraction_to_secondary_power(self: &Self) -> f64 { /* ... */ }
  ```
  Fraction of total power going to the secondary structure.

- ```rust
  pub fn df_adjust(self: &Self) -> bool { /* ... */ }
  ```
  Whether this zone's discontinuity factors are adjusted at runtime.

- ```rust
  pub fn diffusion_coefficient(self: &Self, group: usize, parameters: &[f64], parametrize: bool) -> f64 { /* ... */ }
  ```
  Interpolated diffusion coefficient `D_g` (m) at `parameters`.

- ```rust
  pub fn nu_sigma_f(self: &Self, group: usize, parameters: &[f64], parametrize: bool) -> f64 { /* ... */ }
  ```
  Interpolated `nu Sigma_{f,g}` (m^-1) at `parameters`.

- ```rust
  pub fn sigma_pow(self: &Self, group: usize, parameters: &[f64], parametrize: bool) -> f64 { /* ... */ }
  ```
  Interpolated `kappa Sigma_{f,g}` (J/m) at `parameters`.

- ```rust
  pub fn sigma_removal(self: &Self, group: usize, parameters: &[f64], parametrize: bool) -> f64 { /* ... */ }
  ```
  Interpolated removal cross section `Sigma_{r,g}` (m^-1) at `parameters`.

- ```rust
  pub fn chi_prompt(self: &Self, group: usize, parameters: &[f64], parametrize: bool) -> f64 { /* ... */ }
  ```
  Interpolated prompt spectrum `chi_{p,g}` at `parameters`.

- ```rust
  pub fn chi_delayed(self: &Self, group: usize, parameters: &[f64], parametrize: bool) -> f64 { /* ... */ }
  ```
  Interpolated delayed spectrum `chi_{d,g}` at `parameters`.

- ```rust
  pub fn scattering(self: &Self, moment: usize, energy_j: usize, energy_i: usize, parameters: &[f64], parametrize: bool) -> f64 { /* ... */ }
  ```
  Interpolated scattering `Sigma_{s, j->i}` (m^-1) for Legendre `moment`.

- ```rust
  pub fn diffusion_coefficient_ref(self: &Self, group: usize) -> f64 { /* ... */ }
  ```
  Reference (feedback-free) value of a group quantity via a selector.

- ```rust
  pub fn inverse_velocity(self: &Self, group: usize) -> f64 { /* ... */ }
  ```
  Constant inverse velocity `1/v_g` (s/m).

- ```rust
  pub fn disc_factor(self: &Self, group: usize) -> f64 { /* ... */ }
  ```
  Constant discontinuity factor `gamma_g`.

- ```rust
  pub fn integral_flux(self: &Self, group: usize) -> f64 { /* ... */ }
  ```
  Constant integral flux `Phi_g`.

- ```rust
  pub fn beta(self: &Self, prec: usize) -> f64 { /* ... */ }
  ```
  Constant delayed-neutron fraction `beta_k`.

- ```rust
  pub fn beta_tot(self: &Self) -> f64 { /* ... */ }
  ```
  Total delayed-neutron fraction `beta_tot`.

- ```rust
  pub fn lambda(self: &Self, prec: usize) -> f64 { /* ... */ }
  ```
  Constant precursor decay constant `lambda_k` (s^-1).

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
    fn clone(self: &Self) -> ZoneNuclearData { /* ... */ }
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
### Types

#### Struct `CrossSectionData`

The deterministic multigroup cross-section store for a whole reactor.

Holds one [`ZoneNuclearData`] per material zone (in reference-state order),
the global group counts, and the ordered feedback variables. Construct it
with [`CrossSectionData::from_input`], then evaluate group constants at any
feedback state.

All public evaluation methods return `uom`-typed quantities; the parameter
vectors they take are **raw physical** values (K, kg/m^3, ...) in the order
of [`CrossSectionData::variable_names`] — the transform laws are applied
internally.

```rust
pub struct CrossSectionData {
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
  pub fn from_input(input: &NuclearDataInput) -> Result<Self, XsError> { /* ... */ }
  ```
  Build the cross-section store from a parsed `nuclearData` dictionary.

- ```rust
  pub fn transform_parameters(self: &Self, raw_parameters: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Transform a caller-supplied vector of **raw physical** parameter values

- ```rust
  pub fn energy_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups `G`.

- ```rust
  pub fn prec_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of delayed-neutron precursor groups.

- ```rust
  pub fn legendre_moments(self: &Self) -> usize { /* ... */ }
  ```
  Number of stored Legendre scattering moments (`1 + legendreMoments`).

- ```rust
  pub fn fast_neutrons(self: &Self) -> bool { /* ... */ }
  ```
  Fast-spectrum flag as read from the dictionary (metadata only).

- ```rust
  pub fn variable_names(self: &Self) -> Vec<&str> { /* ... */ }
  ```
  The feedback variables in order; the layout of every parameter vector.

- ```rust
  pub fn variables(self: &Self) -> &[XsVariable] { /* ... */ }
  ```
  The feedback variables (name + law) in order.

- ```rust
  pub fn zone_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of material zones.

- ```rust
  pub fn zone_index(self: &Self, name: &str) -> Option<usize> { /* ... */ }
  ```
  Look up a zone's index by name.

- ```rust
  pub fn zone(self: &Self, index: usize) -> Option<&ZoneNuclearData> { /* ... */ }
  ```
  Borrow a zone by index.

- ```rust
  pub fn evaluate_group_constants(self: &Self, zone: usize, group: usize, raw_parameters: &[f64]) -> Result<GroupConstants, XsError> { /* ... */ }
  ```
  Feedback-interpolated [`GroupConstants`] for zone `zone`, energy group

- ```rust
  pub fn reference_group_constants(self: &Self, zone: usize, group: usize) -> Result<GroupConstants, XsError> { /* ... */ }
  ```
  The reference (feedback-free) [`GroupConstants`] for zone `zone`, group

- ```rust
  pub fn scattering_matrix(self: &Self, zone: usize, moment: usize, raw_parameters: &[f64]) -> Result<SquareMatrix, XsError> { /* ... */ }
  ```
  The feedback-interpolated scattering matrix `Sigma_{s, j->i}` (m^-1) for

- ```rust
  pub fn precursor_constants(self: &Self, zone: usize) -> Result<PrecursorConstants, XsError> { /* ... */ }
  ```
  The delayed-neutron precursor constants for zone `zone`

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
    fn clone(self: &Self) -> CrossSectionData { /* ... */ }
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
### Re-exports

#### Re-export `XsError`

```rust
pub use error::XsError;
```

#### Re-export `GroupConstants`

```rust
pub use group_constants::GroupConstants;
```

#### Re-export `PrecursorConstants`

```rust
pub use group_constants::PrecursorConstants;
```

#### Re-export `NuclearDataInput`

```rust
pub use input::NuclearDataInput;
```

#### Re-export `StateInput`

```rust
pub use input::StateInput;
```

#### Re-export `ZoneConstantsInput`

```rust
pub use input::ZoneConstantsInput;
```

#### Re-export `ZoneStateInput`

```rust
pub use input::ZoneStateInput;
```

#### Re-export `NuclearDataOneEnergy`

```rust
pub use nuclear_data_one_energy::NuclearDataOneEnergy;
```

#### Re-export `VariableLaw`

```rust
pub use variables::VariableLaw;
```

#### Re-export `XsVariable`

```rust
pub use variables::XsVariable;
```

#### Re-export `ZoneNuclearData`

```rust
pub use zone_data::ZoneNuclearData;
```

### Types

#### Enum `NeutronicsModelKind`

Which neutronics model a [`NeutronicsModel`] holds. Returned by
[`NeutronicsModel::kind`] and used to name an unimplemented model in
[`NeutronicsError::ModelNotImplemented`].

```rust
pub enum NeutronicsModelKind {
    PointKinetics,
    Diffusion,
    Sp3,
    Sn,
}
```

##### Variants

###### `PointKinetics`

0-D point kinetics.

###### `Diffusion`

Multigroup neutron diffusion.

###### `Sp3`

Simplified P3 (SP3) transport (eigenvalue + transient).

###### `Sn`

Discrete-ordinates (SN) transport (eigenvalue only).

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
    fn clone(self: &Self) -> NeutronicsModelKind { /* ... */ }
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
    fn eq(self: &Self, other: &NeutronicsModelKind) -> bool { /* ... */ }
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
#### Enum `NeutronicsError`

Errors from constructing or advancing a neutronics model.

Not [`Clone`] — it wraps [`xs::XsError`], which owns un-`Clone` RBF-solve
diagnostics.

```rust
pub enum NeutronicsError {
    Xs(xs::XsError),
    PointKinetics(point_kinetics::PointKineticsError),
    PatchCountMismatch {
        given: usize,
        patches: usize,
    },
    NoFissionSource,
    NotConverged {
        outer_iterations: usize,
        k_residual: f64,
        flux_residual: f64,
    },
    NonPositiveTimeStep(f64),
    ModelNotImplemented(NeutronicsModelKind),
}
```

##### Variants

###### `Xs`

A cross-section materialisation / evaluation failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `xs::XsError` |  |

###### `PointKinetics`

A point-kinetics sub-solve failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `point_kinetics::PointKineticsError` |  |

###### `PatchCountMismatch`

The per-patch flux boundary list did not have one entry per mesh patch.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `given` | `usize` | Number of boundary conditions supplied. |
| `patches` | `usize` | Number of mesh patches. |

###### `NoFissionSource`

The initial fission-neutron production was zero, so `k_eff` is undefined
(a non-multiplying configuration).

###### `NotConverged`

The eigenvalue power iteration did not converge within the outer-iteration
budget.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `outer_iterations` | `usize` | Outer iterations performed. |
| `k_residual` | `f64` | Final relative change in `k_eff`. |
| `flux_residual` | `f64` | Final relative L2 change in the flux. |

###### `NonPositiveTimeStep`

The transient time step was not strictly positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `ModelNotImplemented`

The model was built with the state-only `::new` constructor, so it holds
flux state but no cross sections and cannot solve. Rebuild it with
`with_cross_sections` to obtain a working model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NeutronicsModelKind` |  |

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
    fn from(source: xs::XsError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: PointKineticsError) -> Self { /* ... */ }
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
#### Struct `PointKineticsModel`

A 0-D point-kinetics model wrapped as a [`NeutronicsModel`] variant.

Composes the already-ported [`PointKineticsParameters`] and
[`PointKineticsState`] into one owned unit so it can sit in the
[`NeutronicsModel`] enum alongside the spatial models. Unlike the spatial
models it has no [`NeutronicsState`] / mesh — point kinetics tracks only the
scalar amplitude.

```rust
pub struct PointKineticsModel {
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
  pub fn new(params: PointKineticsParameters, state: PointKineticsState) -> Self { /* ... */ }
  ```
  Build a point-kinetics model from its kinetics parameters and an initial

- ```rust
  pub fn params(self: &Self) -> &PointKineticsParameters { /* ... */ }
  ```
  The kinetics parameters.

- ```rust
  pub fn state(self: &Self) -> &PointKineticsState { /* ... */ }
  ```
  The current 0-D state (fission power + precursor powers).

- ```rust
  pub fn step(self: &mut Self, dt: Time, reactivity: Reactivity, external_source_power: Power) -> Result<(), NeutronicsError> { /* ... */ }
  ```
  Advance one implicit time step under total reactivity `reactivity` and

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
    fn clone(self: &Self) -> PointKineticsModel { /* ... */ }
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
    fn eq(self: &Self, other: &PointKineticsModel) -> bool { /* ... */ }
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
#### Enum `NeutronicsModel`

**Attributes:**

- `Other("#[allow(clippy::large_enum_variant)]")`

The closed set of deterministic neutronics models (enum dispatch, no
`dyn`).

A driver holds a `NeutronicsModel` and calls the shared surface
([`Self::power`], [`Self::k_eff`], [`Self::kind`]) without a compile-time
dependency on which model it is. To *run* a model, match it out and call the
model-specific method (`DiffusionNeutronics::solve_eigenvalue` / `::step`,
`PointKineticsModel::step`) — those interfaces differ (0-D vs. mesh-based)
too much to share one signature honestly.

The variants differ in size (the mesh-based `Diffusion` model is much larger
than the 0-D `PointKinetics` one), which clippy's `large_enum_variant` would
flag — the standard fix is to `Box` the big variant, but `Box<T>` is banned
by the workspace design rules, so the lint is allowed here instead. A
`NeutronicsModel` is created once per reactor region and lives for the whole
run, so the size asymmetry costs nothing in practice.

```rust
pub enum NeutronicsModel {
    PointKinetics(PointKineticsModel),
    Diffusion(DiffusionNeutronics),
    Sp3(Sp3Neutronics),
    Sn(SnNeutronics),
}
```

##### Variants

###### `PointKinetics`

0-D point kinetics.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PointKineticsModel` |  |

###### `Diffusion`

Multigroup neutron diffusion.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `DiffusionNeutronics` |  |

###### `Sp3`

Simplified P3 transport (eigenvalue + transient).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Sp3Neutronics` |  |

###### `Sn`

Discrete-ordinates transport (eigenvalue only).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SnNeutronics` |  |

##### Implementations

###### Methods

- ```rust
  pub fn kind(self: &Self) -> NeutronicsModelKind { /* ... */ }
  ```
  Which model this is.

- ```rust
  pub fn power(self: &Self) -> Power { /* ... */ }
  ```
  The current total fission power `P` (watts).

- ```rust
  pub fn k_eff(self: &Self) -> Option<NeutronMultiplicationFactor> { /* ... */ }
  ```
  The effective multiplication factor `k_eff`, if the model tracks one.

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
    fn clone(self: &Self) -> NeutronicsModel { /* ... */ }
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
### Re-exports

#### Re-export `DiffusionNeutronics`

```rust
pub use diffusion::DiffusionNeutronics;
```

#### Re-export `DiffusionSettings`

```rust
pub use diffusion::DiffusionSettings;
```

#### Re-export `EigenvalueReport`

```rust
pub use diffusion::EigenvalueReport;
```

#### Re-export `SnNeutronics`

```rust
pub use sn::SnNeutronics;
```

#### Re-export `Sp3Neutronics`

```rust
pub use sp3::Sp3Neutronics;
```

#### Re-export `NeutronMultiplicationFactor`

```rust
pub use state::NeutronMultiplicationFactor;
```

#### Re-export `NeutronicsState`

```rust
pub use state::NeutronicsState;
```

## Module `thermal_hydraulics`

# `genfoam::thermal_hydraulics` — reactor thermal-hydraulics

Rust port of `GeN-Foam/src/classes/thermalHydraulics` (~65k LOC — by far the
largest GeN-Foam module): the single- and two-phase reactor thermal-hydraulics
(porous-medium momentum/energy for the core, sub-channel and pin models, the
fluid/structure heat-transfer closures, and the turbulence/friction
correlations) that supplies the temperature/density feedback to the
neutronics.

Generic FV building blocks (fields, `fvm`/`fvc` operators, matrices) come from
[`outram_foam_basic_lib`]; turbulence closures from
[`outram_foam_turbulence_lib`]. This module ports GeN-Foam's reactor-specific
TH extensions on top of those, NOT the generic FV machinery.

**Port in progress — this is a large, multi-slice sub-effort.** See
`docs/genfoam-port-plan.md` (the "thermalHydraulics breakdown" section) for
the sub-module map, translation order, and per-sub-module beads
(`op-p6p.7.1` … `op-p6p.7.14`).

## Sub-module map

Every sub-module below is ported and carries unit tests against published
correlation values or closed-form results. What remains unported is listed
under "Known gaps".

- [`units`] — named `uom` aliases (`ReynoldsNumber`, `DarcyFrictionFactor`,
  `HeatTransferCoefficient`, `HeatFlux`). Implemented.
- [`closures`] — the `physicsModels/` correlation leaves: `fs_drag`,
  `ff_drag`, `heat_transfer`, `phase_change`, `interfacial` and
  `turbulence`. All six families are implemented with their own `tests`
  modules; [`closures::fs_drag`] additionally carries an analytic
  verification (laminar `f·Re → 64`).
- [`phase`] / [`structure`] — fluid-phase and solid-structure field state,
  including the power/heat-exchanger/pump structure models. Implemented.
- [`solver`] — the porous solver drivers. [`solver::one_phase`] (UEqn/pEqn/
  EEqn) is implemented; see "Known gaps" for its property limitation.
- [`boundary_conditions`] — `blackbody_radiation`, `velocity_rundown` and
  `time_field_table` implemented.
- [`function_objects`] — post-processing diagnostics (mass flow, pressure
  drop, bulk temperature, field diffs). Implemented.
- [`thermophysical`] — the bespoke dissociating-hydrogen (H/H₂) property
  package: EOS, thermodynamics, viscosity, conductivity. Implemented.

## Known gaps

- **The two-phase (MULES) solver is not implemented**, nor is
  `onePhaseLegacy`. Only [`solver::one_phase`] exists.
- [`solver::one_phase`] runs on **constant fluid properties** (`he = Cp·T`,
  fixed-surface-temperature structure coupling): [`thermophysical`] is
  ported but not yet wired in as the driver's fluid package.
- `boundary_conditions::nusselt_baffle` is a **stub** — every method is
  `unimplemented!()` (cross-patch implicit coupling is not supported).
- [`closures::turbulence`] ports the closure *algebra* only; the k/ε
  transport equations and `correctNut` orchestration are deferred (the
  generic single-phase machinery lives in `outram-foam-turbulence-lib`).
- The correlation leaves are **unit-tested, not system-validated** — they
  have not been exercised inside a converged multiphysics run.
- The great majority of upstream `thermalHydraulics` (~65k LOC) is still
  unported; what exists here is the closure/field/one-phase-driver
  foundation.

```rust
pub mod thermal_hydraulics { /* ... */ }
```

### Modules

## Module `boundary_conditions`

# `genfoam::thermal_hydraulics::boundary_conditions` — GeN-Foam TH boundary conditions

GeN-Foam-specific thermal-hydraulics boundary conditions layered on top of
basic-lib's generic BC set. Generic OpenFOAM BC machinery (fixedValue,
zeroGradient, and the rest of basic-lib's BC set) does **not** belong
here — only the GeN-Foam-specific closures below it.

Ports upstream `src/classes/thermalHydraulics/src/boundaryConditions/**`
(commit 652b3da). Each is a **physical-value closure**: a plain struct
plus methods computing boundary values from `uom`-dimensioned inputs
(temperatures, time, per-face slices), not a full `fvPatchField` — the
per-face mesh loop and "wire this into the solver" plumbing belong to the
porous-solver bead (op-p6p.7.11).

## Module map

| Submodule | Ports | Status |
|---|---|---|
| [`blackbody_radiation`] | `blackBodyRadiation` | Ported — [`BlackBodyRadiationBc`] |
| [`velocity_rundown`] | `velocityRundown` | Ported — [`VelocityRundownBc`] |
| [`time_field_table`] | `timeFieldTable` | Ported — [`TimeFieldTable`] |
| [`nusselt_baffle`] | `NusseltThermalBaffle1D` | **Scaffold only** — [`NusseltThermalBaffle1DBc`]; every method is `unimplemented!()`. See the module doc for why (cross-patch implicit coupling) and bead op-p6p.7.13 for the follow-up. |

```rust
pub mod boundary_conditions { /* ... */ }
```

### Modules

## Module `blackbody_radiation`

# `blackbody_radiation` — Stefan-Boltzmann radiative wall temperature

Port of GeN-Foam's `blackBodyRadiationFvPatchScalarField`: a simplified
black-body radiation condition for a wall exposed to an ambient
(radiation-sink) temperature `T_a`. The face temperature is found from the
Stefan-Boltzmann law

```text
T_face = ( q'' / (sigma * epsilon) + T_a^4 )^(1/4)
```

where the driving heat flux `q''` is itself approximated with a one-sided
Fourier (conduction) estimate between the cell-centre temperature and the
(previous) face temperature:

```text
q'' = | kappa * (T_cell - T_face) / d |
```

`d` is the cell-centre-to-face distance and `kappa` the wall thermal
conductivity. Upstream calls both steps back-to-back inside
`updateCoeffs()` every outer iteration (using the *previous* iteration's
face temperature in the Fourier estimate), which is a fixed-point update,
not a closed-form solve — [`BlackBodyRadiationBc::update_face_temperature`]
reproduces exactly that one step; a caller iterates it to convergence the
same way the upstream outer loop does.

## What this is (and is not)

This is a **physical-value closure** — a plain struct plus methods over
`uom`-dimensioned scalars, not a full `fvPatchField`. The per-face loop,
mesh-face distance lookup, and "wire this into the enthalpy equation"
plumbing are the porous-solver's job; this module only reproduces the
per-face temperature update, which is self-contained and independently
verifiable against the Stefan-Boltzmann closed form.

## Stefan-Boltzmann constant

`sigma = 5.670374419e-8 W / (m^2 K^4)` is the current (2019 SI
redefinition) exact value, given the base SI units are now themselves
exactly defined constants (CODATA 2018). Upstream reads this from
OpenFOAM's `constant::physicoChemical::sigma`; the numeric value matches to
the digits OpenFOAM prints. `uom` has no named quantity for `W/(m^2 K^4)`
(a temperature-to-the-fourth composite), so `sigma` and the `T^4` algebra
are carried as raw `f64` in SI units internally — the same pattern used
elsewhere in this crate (e.g. [`super::super::structure::heat_source`]) for
composite expressions with no natural `uom` type; all public signatures
stay `uom`-dimensioned.

```rust
pub mod blackbody_radiation { /* ... */ }
```

### Types

#### Struct `BlackBodyRadiationBc`

A black-body radiative wall boundary condition.

Port of `blackBodyRadiationFvPatchScalarField`: given the wall's
emissivity, thermal conductivity, and the ambient (radiation-sink)
temperature, computes the face temperature implied by a Fourier-law
estimate of the wall heat flux via the Stefan-Boltzmann law.

Construct with [`BlackBodyRadiationBc::new`]; advance one outer-iteration
step with [`BlackBodyRadiationBc::update_face_temperature`], or call the
two half-steps ([`fourier_heat_flux`](Self::fourier_heat_flux),
[`face_temperature_from_flux`](Self::face_temperature_from_flux))
independently.

```rust
pub struct BlackBodyRadiationBc {
    pub emissivity: uom::si::f64::Ratio,
    pub kappa: uom::si::f64::ThermalConductivity,
    pub ambient_temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `emissivity` | `uom::si::f64::Ratio` | Surface emissivity `epsilon`, dimensionless, `0..=1` (1 = ideal black<br>body). |
| `kappa` | `uom::si::f64::ThermalConductivity` | Wall thermal conductivity `kappa` used in the Fourier heat-flux<br>estimate. |
| `ambient_temperature` | `uom::si::f64::ThermodynamicTemperature` | Ambient (radiation-sink) temperature `T_a`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(emissivity: Ratio, kappa: ThermalConductivity, ambient_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a black-body radiation BC from its three dictionary entries

- ```rust
  pub fn fourier_heat_flux(self: &Self, face_temperature: ThermodynamicTemperature, cell_temperature: ThermodynamicTemperature, cell_to_face_distance: Length) -> HeatFlux { /* ... */ }
  ```
  The one-sided Fourier (conduction) heat-flux estimate between the

- ```rust
  pub fn face_temperature_from_flux(self: &Self, heat_flux: HeatFlux) -> ThermodynamicTemperature { /* ... */ }
  ```
  The Stefan-Boltzmann face temperature implied by a heat flux `q''`:

- ```rust
  pub fn update_face_temperature(self: &Self, current_face_temperature: ThermodynamicTemperature, cell_temperature: ThermodynamicTemperature, cell_to_face_distance: Length) -> ThermodynamicTemperature { /* ... */ }
  ```
  One outer-iteration update step, mirroring upstream's `updateCoeffs()`

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
    fn clone(self: &Self) -> BlackBodyRadiationBc { /* ... */ }
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
    fn eq(self: &Self, other: &BlackBodyRadiationBc) -> bool { /* ... */ }
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
## Module `nusselt_baffle`

# `nusselt_baffle` — 1-D through-wall conduction coupled baffle (SCAFFOLD ONLY)

**Not implemented.** This module only declares the public data shape of
GeN-Foam's `NusseltThermalBaffle1DFvPatchScalarField` so the rest of the
`boundary_conditions` module tree can reference its types; every method
that would compute a physical result is `unimplemented!()`. See bead
op-p6p.7.13 (follow-up) for the full port.

## What upstream does (for the next porter's context)

Two GeN-Foam patches (a "master" and a "slave", named via the `samplePatch`
entry) sit on opposite faces of a **thin** wall — thin meaning the wall's
own thermal inertia is neglected, so its two surface temperatures are set
by an **instantaneous heat-flux balance** rather than a transient
conduction solve:

```text
h_master * (T_fluid,master - T_wall,master)
    = kappa_wall / thickness * (T_wall,master - T_wall,slave)
    = h_slave  * (T_wall,slave  - T_fluid,slave)
```

`h_master`/`h_slave` are convective heat-transfer coefficients from a
Nusselt-number correlation of the form

```text
Nu = const + coeff * Re^expRe * Pr^expPr
```

with per-side `(const, coeff, expRe, expPr)` — the slave side may omit any
of the four and inherit the master's value (upstream: "If not provided,
defaults to master value").

The update is **implicit**: upstream's `updateCoeffs()` on the master patch
also reaches into the slave patch's `valueFraction`/`refValue`/`refGradient`
(a `mixed` boundary condition) to solve the three-way balance above as one
coupled system, and — in the two-phase case — additionally couples across
*both* fluid phases' temperature fields simultaneously. That
cross-patch/cross-phase coupling is genuinely solver-shaped (it needs
mutable access to another patch's BC state inside one patch's coefficient
update), which is why this is scaffolded rather than fully ported here: it
does not fit this module's "plain struct + pure function over dimensioned
scalars" contract without first deciding how the porous-solver bead
(op-p6p.7.11) represents inter-patch coupling.

## Not fabricated here

No formula below is evaluated — the structs only carry the dictionary
entries upstream reads (`thickness`, `kappa`, `const`/`coeff`/`expRe`/
`expPr` per side) and the method signatures document, but do not compute,
the flux balance above. Every method body is `unimplemented!()`.

```rust
pub mod nusselt_baffle { /* ... */ }
```

### Types

#### Enum `BaffleSide`

Which side of the baffle (master or slave patch) a query refers to.

Mirrors upstream's `owner()` distinction between the patch that owns the
dictionary's `thickness`/`kappa` entries (the master, `samplePatch`
pointing at the slave) and the patch that reads them from its neighbour.

```rust
pub enum BaffleSide {
    Master,
    Slave,
}
```

##### Variants

###### `Master`

The patch that declares `thickness` and `kappa` directly.

###### `Slave`

The patch that reads `thickness`/`kappa` from its `samplePatch` (the
master).

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
    fn clone(self: &Self) -> BaffleSide { /* ... */ }
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
    fn eq(self: &Self, other: &BaffleSide) -> bool { /* ... */ }
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
#### Struct `NusseltCorrelationCoefficients`

Nusselt-number correlation coefficients, `Nu = const + coeff * Re^expRe *
Pr^expPr` — one instance per baffle side.

Mirrors the upstream `const`, `coeff`, `expRe`, `expPr` dictionary entries
verbatim. All four are dimensionless (the correlation itself is
dimensionless; `Re` and `Pr` are dimensionless inputs).

```rust
pub struct NusseltCorrelationCoefficients {
    pub const_term: f64,
    pub coeff: f64,
    pub exp_reynolds: f64,
    pub exp_prandtl: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `const_term` | `f64` | The additive constant term in the Nusselt correlation. |
| `coeff` | `f64` | The multiplicative coefficient on `Re^expRe * Pr^expPr`. |
| `exp_reynolds` | `f64` | The Reynolds-number exponent. |
| `exp_prandtl` | `f64` | The Prandtl-number exponent. |

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
    fn clone(self: &Self) -> NusseltCorrelationCoefficients { /* ... */ }
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
    fn eq(self: &Self, other: &NusseltCorrelationCoefficients) -> bool { /* ... */ }
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
#### Struct `NusseltThermalBaffle1DBc`

Placeholder for the (not yet ported) 1-D through-wall conduction baffle.

See the [module documentation](self) for why this is a data-only skeleton:
every method that would evaluate the coupled flux balance is
`unimplemented!()`. Do not call the methods below expecting a result —
they exist only to fix the public API shape ahead of the real port (bead
op-p6p.7.13 follow-up).

```rust
pub struct NusseltThermalBaffle1DBc {
    pub thickness: uom::si::f64::Length,
    pub wall_conductivity: uom::si::f64::ThermalConductivity,
    pub master_correlation: NusseltCorrelationCoefficients,
    pub slave_correlation: NusseltCorrelationCoefficients,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `thickness` | `uom::si::f64::Length` | Baffle (wall) thickness, upstream `thickness`. |
| `wall_conductivity` | `uom::si::f64::ThermalConductivity` | Baffle (wall) thermal conductivity, upstream `kappa`. |
| `master_correlation` | `NusseltCorrelationCoefficients` | Nusselt correlation coefficients for the master-side convective<br>coupling. |
| `slave_correlation` | `NusseltCorrelationCoefficients` | Nusselt correlation coefficients for the slave-side convective<br>coupling (defaults to `master_correlation` upstream when the<br>dictionary omits them — that default-inheritance is case-I/O and is<br>the caller's responsibility here, not this type's). |

##### Implementations

###### Methods

- ```rust
  pub fn new(thickness: Length, wall_conductivity: ThermalConductivity, master_correlation: NusseltCorrelationCoefficients, slave_correlation: NusseltCorrelationCoefficients) -> Self { /* ... */ }
  ```
  Build a baffle BC from its dictionary entries. Pure data assembly — no

- ```rust
  pub fn convective_htc(self: &Self, _side: BaffleSide, _reynolds: Ratio, _prandtl: Ratio, _fluid_conductivity: ThermalConductivity, _hydraulic_diameter: Length) -> HeatTransferCoefficient { /* ... */ }
  ```
  The convective heat-transfer coefficient on one side of the baffle,

- ```rust
  pub fn coupled_wall_temperatures(self: &Self, _master_fluid_temperature: ThermodynamicTemperature, _slave_fluid_temperature: ThermodynamicTemperature, _master_htc: HeatTransferCoefficient, _slave_htc: HeatTransferCoefficient) -> (ThermodynamicTemperature, ThermodynamicTemperature) { /* ... */ }
  ```
  The coupled master/slave wall-face temperatures solving the

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
    fn clone(self: &Self) -> NusseltThermalBaffle1DBc { /* ... */ }
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
    fn eq(self: &Self, other: &NusseltThermalBaffle1DBc) -> bool { /* ... */ }
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
## Module `time_field_table`

# `time_field_table` — tabulated time-varying per-face scalar boundary

Port of GeN-Foam's `timeFieldTableFvPatchScalarField`: a boundary
condition whose value is a **whole per-face field**, tabulated against
time and linearly interpolated —

```text
table
(
    (t0 (v0_0 v1_0 v2_0 ...))
    ...
    (tN (v0_N v1_N v2_N ...))
);
```

`ti` is a tabulated time and `vj_i` the value at face `j` for that time
row. For `t < t0` the first row's field applies unchanged; for `t > tN`
the last row's field applies unchanged; in between, adjacent rows are
linearly interpolated **face-by-face** with a single shared interpolation
coefficient (all faces cross their `(t_i, t_{i+1})` bracket together, since
they share one time axis).

Unlike [`super::super::super::common::TimeProfile`] (one scalar value,
ordinate unit fixed by the consumer) this is the **per-face** analogue —
GeN-Foam's `timeFieldTableFvPatchScalarField` doc notes plainly there was
no ready-made OpenFOAM `Table` class for a `scalarField`-valued table, so
it hand-rolled this one; the same gap exists here, so [`TimeFieldTable`]
fills it.

## Implementation: reuses [`ScalarInterpolateTable`]

Rather than re-deriving the clamp/interpolate arithmetic, [`TimeFieldTable`]
builds one [`ScalarInterpolateTable`] per face column (all sharing the same
abscissa — the tabulated times) with
[`InterpolationMethod::Linear`] and [`OutOfBounds::Fixed`] (clamp to the
nearest tabulated row), which is exactly upstream's `t < t0` /
`t > tN` / linear-in-between behaviour. This is a straight reuse of an
already-verified building block (see
`crate::genfoam::common::interpolate_table`'s own tests), not a new
numerical implementation.

## Dimension convention

Like [`TimeProfile`](super::super::super::common::TimeProfile), the
abscissa (time) is [`Time`](uom::si::f64::Time) and each face's ordinate is
a raw `f64` whose physical unit the consumer fixes — the same generic
table serves any scalar boundary field (a temperature ramp, a heat-flux
schedule, …).

## What this is (and is not)

A **physical-value closure** over a `Vec<f64>` per query time — not a full
`fvPatchField`. Reading the dictionary's nested `(t (v0 v1 ...))` list
syntax and writing it back out is case I/O, out of scope here; this module
only reproduces the lookup-and-interpolate behaviour once the table has
been parsed into `(times, values)`.

A table needs at least **two** time rows to interpolate between (the same
floor [`ScalarInterpolateTable`] enforces); a single-row table is a
constant field and does not need this machinery.

```rust
pub mod time_field_table { /* ... */ }
```

### Types

#### Enum `TimeFieldTableError`

Errors constructing a [`TimeFieldTable`].

```rust
pub enum TimeFieldTableError {
    LengthMismatch {
        times: usize,
        rows: usize,
    },
    InconsistentRowLength {
        index: usize,
        expected: usize,
        got: usize,
    },
    Table(crate::genfoam::common::interpolate_table::InterpolateTableError),
}
```

##### Variants

###### `LengthMismatch`

The `times` and `values` (row) vectors had different lengths.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `times` | `usize` | Number of tabulated time entries. |
| `rows` | `usize` | Number of value rows supplied. |

###### `InconsistentRowLength`

A value row had a different number of faces than row 0.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Index of the offending row. |
| `expected` | `usize` | Face count established by row 0. |
| `got` | `usize` | Face count actually found in this row. |

###### `Table`

The per-face interpolation table could not be built — wraps the
underlying [`InterpolateTableError`] (too few time points, or the
times were not strictly ascending). Since every face shares the same
time axis, this only ever originates from the shared `times` column.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::genfoam::common::interpolate_table::InterpolateTableError` |  |

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
    fn clone(self: &Self) -> TimeFieldTableError { /* ... */ }
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
    fn from(source: InterpolateTableError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TimeFieldTableError) -> bool { /* ... */ }
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
#### Struct `TimeFieldTable`

A per-face scalar boundary field tabulated against time.

See the [module documentation](self) for the table layout, the reuse of
[`ScalarInterpolateTable`] for the interior arithmetic, and the dimension
convention (abscissa is [`Time`]; each face's ordinate is a raw `f64` the
consumer's unit). Build with [`TimeFieldTable::new`]; query with
[`TimeFieldTable::value`].

```rust
pub struct TimeFieldTable {
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
  pub fn new(times: Vec<Time>, values: Vec<Vec<f64>>) -> Result<Self, TimeFieldTableError> { /* ... */ }
  ```
  Build a table from tabulated rows `(times[i], values[i])`, where

- ```rust
  pub fn n_faces(self: &Self) -> usize { /* ... */ }
  ```
  The number of faces this table covers.

- ```rust
  pub fn value(self: &Self, t: Time) -> Vec<f64> { /* ... */ }
  ```
  The per-face field at time `t`: linear interpolation between the

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
    fn clone(self: &Self) -> TimeFieldTable { /* ... */ }
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
    fn eq(self: &Self, other: &TimeFieldTable) -> bool { /* ... */ }
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
## Module `velocity_rundown`

# `velocity_rundown` — pump/flow coastdown velocity boundary condition

Port of GeN-Foam's `velocityRundownFvPatchVectorField`: a velocity inlet
whose value follows a power-law coastdown/run-up profile once a trip time
`t_0` has elapsed —

```text
U(t) = U0                              for t <= t0
U(t) = U0 * (A + B*(t - t0))^C         for t >  t0
```

`U0` is the reference (pre-trip, steady) velocity vector; `A`, `B`, `C` are
the upstream dictionary entries `const`, `coeff`, `exp`. This reproduces a
**power-law** rundown, not an exponential one — e.g. a pump coasting down
under `C < 0` slows towards zero as `(1 + B*(t-t0))^C` grows without bound
in the denominator sense; a linear ramp is `C = 1`.

## Reading back a restarted case (not ported)

Upstream's dictionary constructor also *un-rundowns* a `value` entry read
from a restart file back to `U0` (dividing by the same factor evaluated at
the restart time), because OpenFOAM restart files store the boundary
field's last-evaluated value rather than the original reference velocity.
That is restart-file I/O plumbing, not physics — this port only exposes
the run-time evaluation `U(t)` given `U0` directly; a caller that already
has `U0` (from the case setup, not a restart file) does not need it.

## What this is (and is not)

A **physical-value closure**: a plain struct plus a method computing the
instantaneous velocity vector as a function of time, not a full
`fvPatchField`. The reference velocity is carried as a basic-lib
[`Vector3`] rather than a `uom` vector type — matching the precedent in
[`super::super::structure::pump::Pump`], whose doc explains why: the
underlying `Foam::volVectorField`/`surfaceVectorField` this ultimately
feeds is itself raw `f64` per component, so `uom` typing stops at the
scalar coefficients (`A`, `C` dimensionless; `B` and `t0` carry time
dimensions) and the vector magnitude is scaled by the dimensionless
rundown factor.

```rust
pub mod velocity_rundown { /* ... */ }
```

### Types

#### Struct `VelocityRundownBc`

A pump/flow coastdown (power-law rundown) velocity boundary condition.

Port of `velocityRundownFvPatchVectorField`. See the [module
documentation](self) for the `U(t) = U0 * (A + B*(t-t0))^C` profile.
Construct with [`VelocityRundownBc::new`]; query with
[`VelocityRundownBc::rundown_factor`] (the dimensionless scale factor
alone) or [`VelocityRundownBc::velocity`] (applied to a reference
velocity vector).

```rust
pub struct VelocityRundownBc {
    pub const_a: uom::si::f64::Ratio,
    pub coeff_b: uom::si::f64::Frequency,
    pub exp_c: f64,
    pub start_time: uom::si::f64::Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `const_a` | `uom::si::f64::Ratio` | The dictionary `const` entry `A`, dimensionless. |
| `coeff_b` | `uom::si::f64::Frequency` | The dictionary `coeff` entry `B` — **base SI: 1/s** (`Hz`). Multiplies<br>the elapsed time `t - t0` so the sum `A + B*(t-t0)` stays<br>dimensionless before being raised to the power `C`. |
| `exp_c` | `f64` | The dictionary `exp` entry `C`, dimensionless exponent (can be<br>negative for a decaying rundown, e.g. `C < 0`, or positive for a<br>ramp-up). |
| `start_time` | `uom::si::f64::Time` | The trip/start time `t0`. The profile is held at the reference value<br>for `t <= t0` and evaluates the power law for `t > t0`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(const_a: Ratio, coeff_b: Frequency, exp_c: f64, start_time: Time) -> Self { /* ... */ }
  ```
  Build a rundown BC from its four dictionary entries (`const`, `coeff`,

- ```rust
  pub fn rundown_factor(self: &Self, t: Time) -> Ratio { /* ... */ }
  ```
  The dimensionless rundown factor `f(t)`:

- ```rust
  pub fn velocity(self: &Self, reference_velocity: Vector3, t: Time) -> Vector3 { /* ... */ }
  ```
  The instantaneous velocity `U(t) = U0 * f(t)`, given the reference

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
    fn clone(self: &Self) -> VelocityRundownBc { /* ... */ }
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
    fn eq(self: &Self, other: &VelocityRundownBc) -> bool { /* ... */ }
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
### Re-exports

#### Re-export `BlackBodyRadiationBc`

```rust
pub use blackbody_radiation::BlackBodyRadiationBc;
```

#### Re-export `BaffleSide`

```rust
pub use nusselt_baffle::BaffleSide;
```

#### Re-export `NusseltCorrelationCoefficients`

```rust
pub use nusselt_baffle::NusseltCorrelationCoefficients;
```

#### Re-export `NusseltThermalBaffle1DBc`

```rust
pub use nusselt_baffle::NusseltThermalBaffle1DBc;
```

#### Re-export `TimeFieldTable`

```rust
pub use time_field_table::TimeFieldTable;
```

#### Re-export `TimeFieldTableError`

```rust
pub use time_field_table::TimeFieldTableError;
```

#### Re-export `VelocityRundownBc`

```rust
pub use velocity_rundown::VelocityRundownBc;
```

## Module `closures`

# `genfoam::thermal_hydraulics::closures` — TH closure correlations

Rust port of GeN-Foam's `src/classes/thermalHydraulics/src/physicsModels/`
(~35k LOC, the single largest part of the module). These are the algebraic
**leaves** of the thermal-hydraulics model: small, self-contained functions
of local field values (Reynolds number, void fraction, quality, …) that feed
the porous momentum, energy, and phase-transport equations in
[`super::solver`].

Upstream each correlation family is an OpenFOAM `runTimeSelectionTable` of a
virtual base class. Per the workspace no-`dyn`-dispatch rule, each family is
translated to a **closed enum** with one variant per correlation and a
`match`-based dispatch method — adding a correlation forces every dispatch
site to handle it.

## Sub-modules

All six families are implemented, each with its own `tests` module checking
the correlations against published values or closed-form limits. They are
**unit-tested, not system-validated** — none has been exercised inside a
converged multiphysics run.

- [`fs_drag`] — fluid-structure (wall) Darcy friction-factor correlations.
  Implemented; additionally **verified** against the analytic laminar limit
  `f·Re → 64`.
- [`ff_drag`] — fluid-fluid (interfacial) drag correlations.
- [`heat_transfer`] — fluid-structure and fluid-fluid heat-transfer
  coefficients, plus critical heat flux.
- [`phase_change`] — saturation properties and phase-change source terms.
- [`interfacial`] — interfacial area, bubble/droplet diameter, virtual mass,
  and the flow-regime map.
- [`turbulence`] — the two-phase/porous turbulence **closure algebra**. The
  k/ε transport equations and `correctNut` orchestration are deferred; see
  that module's header for the precise deferral list.

See `docs/genfoam-port-plan.md` for the translation order and per-family
tracking beads.

```rust
pub mod closures { /* ... */ }
```

### Modules

## Module `fs_drag`

# `fs_drag` — fluid-structure wall-friction factor correlations

Port of GeN-Foam's `FSDragCoefficientModels` family. Each model maps a local
[`ReynoldsNumber`] to a **Darcy friction factor** ([`DarcyFrictionFactor`]),
the `f` in the channel pressure gradient

```text
dp/dx = - f * rho * |U| * U / (2 * D_h)
```

In GeN-Foam this scalar is one component of the anisotropic fluid-structure
drag tensor `Kd` assembled per cell in the porous momentum equation
(`UEqn`); assembling `Kd` from these factors belongs to the solver bead
(op-p6p.7.11) and is out of scope here. This module ports the **correlations
themselves**, which are pure algebra in `Re` plus fixed geometric
coefficients — no mesh or field state — and are therefore independently
verifiable against published values and analytical limits.

## Model set (closed enum, no `dyn` dispatch)

| Variant | Upstream | Intended flow geometry |
|---|---|---|
| [`FsWallFriction::Churchill`] | `Churchill` | Smooth/rough pipe, all-`Re` (Churchill 1977) |
| [`FsWallFriction::Colebrook`] | `Colebrook` | Power-law fit `f = (a·log10 Re + b)^c` |
| [`FsWallFriction::ReynoldsPower`] | `ReynoldsPower` | Generic power law `f = a·Re^b + c` |
| [`FsWallFriction::Rehme`] | `Rehme` | Wire-wrapped rod bundle (Rehme 1973) |
| [`FsWallFriction::Engel`] | `Engel` | Wire-wrapped rod bundle (Engel et al. 1979) |
| [`FsWallFriction::ModifiedEngel`] | `modifiedEngel` | Modified Engel bundle |
| [`FsWallFriction::BaxiDalleDonne`] | `BaxiDalleDonne` | Wire-wrapped bundle (Baxi & Dalle Donne) |
| [`FsWallFriction::NoKazimi`] | `NoKazimi` | Wire-wrapped bundle (No & Kazimi) |

The bundle correlations ([`FsWallFriction::Rehme`], `BaxiDalleDonne`,
`NoKazimi`) precompute fixed coefficients from the wire-wrap geometry; use
the `*_from_geometry` constructors to reproduce GeN-Foam's derivation exactly.

## References

- S.W. Churchill, "Friction-factor equation spans all fluid-flow regimes",
  *Chem. Eng.* 84(24), 1977, pp. 91-92.
- K. Rehme, "Pressure drop correlations for fuel element spacers",
  *Nucl. Technol.* 17, 1973, pp. 15-23.
- Y.S. Tang, R.D. Coffield, R.A. Markley / F.C. Engel, R.A. Markley,
  A.A. Bishop, "Laminar, transition, and turbulent parallel flow pressure
  drop across wire-wrap-spaced rod bundles", *Nucl. Sci. Eng.* 69, 1979.

```rust
pub mod fs_drag { /* ... */ }
```

### Types

#### Enum `FsWallFriction`

Fluid-structure (wall) Darcy friction-factor correlation.

Closed enum port of GeN-Foam's `FSDragCoefficientModel` run-time-selectable
family. Evaluate with [`FsWallFriction::friction_factor`], which takes the
local [`ReynoldsNumber`] and returns the [`DarcyFrictionFactor`].

Each variant carries the parameters GeN-Foam reads from its `dragModel`
dictionary (or, for the bundle models, the coefficients derived once from the
wire-wrap geometry — see the `*_from_geometry` constructors).

```rust
pub enum FsWallFriction {
    Churchill {
        surface_roughness: f64,
    },
    Colebrook {
        coeff: f64,
        constant: f64,
        exp: f64,
    },
    ReynoldsPower {
        coeff: f64,
        exp: f64,
        constant: f64,
    },
    Rehme {
        a: f64,
        b1: f64,
        b2: f64,
    },
    Engel,
    ModifiedEngel,
    BaxiDalleDonne {
        a: f64,
        b: f64,
        c: f64,
    },
    NoKazimi {
        a: f64,
        b: f64,
        c: f64,
    },
}
```

##### Variants

###### `Churchill`

Churchill (1977) all-regime correlation. `surface_roughness` is the
relative roughness `epsilon = e / D_h` (dimensionless); use `0.0` for a
hydraulically smooth wall.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `surface_roughness` | `f64` | Relative surface roughness `epsilon = e / D_h` (dimensionless). |

###### `Colebrook`

Power-law-in-`log10(Re)` fit `f = (coeff · log10(Re) + constant)^exp`.
(GeN-Foam names this model `Colebrook`; it is an explicit fit, not the
implicit Colebrook-White equation.)

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `coeff` | `f64` | Multiplier on `log10(Re)`. |
| `constant` | `f64` | Additive constant inside the power. |
| `exp` | `f64` | Outer exponent. |

###### `ReynoldsPower`

Generic power law `f = coeff · Re^exp + constant`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `coeff` | `f64` | Leading coefficient. |
| `exp` | `f64` | Reynolds-number exponent (typically negative). |
| `constant` | `f64` | Additive constant. |

###### `Rehme`

Rehme (1973) wire-wrapped-bundle correlation
`f = a · (b1 / Re + b2 / Re^0.133)`. Build with
[`FsWallFriction::rehme_from_geometry`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Wetted-perimeter split factor `a` (dimensionless). |
| `b1` | `f64` | Laminar-branch coefficient `b1`. |
| `b2` | `f64` | Turbulent-branch coefficient `b2`. |

###### `Engel`

Engel et al. (1979) wire-wrapped-bundle correlation with a
laminar/turbulent blend over `400 < Re < 5000`. No parameters.

###### `ModifiedEngel`

Modified Engel bundle correlation (different laminar/turbulent constants),
same `400 < Re < 5000` blend. No parameters.

###### `BaxiDalleDonne`

Baxi & Dalle Donne wire-wrapped-bundle correlation, blended over
`400 < Re < 5000`. Build with
[`FsWallFriction::baxi_dalle_donne_from_geometry`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Laminar-branch coefficient `a`. |
| `b` | `f64` | Turbulent-branch coefficient `b`. |
| `c` | `f64` | Turbulent-branch coefficient `c`. |

###### `NoKazimi`

No & Kazimi wire-wrapped-bundle correlation, blended over
`400 < Re < 2600`. Build with
[`FsWallFriction::no_kazimi_from_geometry`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Laminar-branch coefficient `a`. |
| `b` | `f64` | Turbulent-branch coefficient `b`. |
| `c` | `f64` | Turbulent-branch coefficient `c`. |

##### Implementations

###### Methods

- ```rust
  pub fn friction_factor(self: &Self, re: ReynoldsNumber) -> DarcyFrictionFactor { /* ... */ }
  ```
  Evaluate the Darcy friction factor at the given Reynolds number.

- ```rust
  pub fn rehme_from_geometry(number_of_pins: f64, pin_diameter: f64, wire_diameter: f64, wire_lead_length: f64, wetted_wrap_perimeter: f64) -> Self { /* ... */ }
  ```
  Build a [`FsWallFriction::Rehme`] from wire-wrapped-bundle geometry,

- ```rust
  pub fn baxi_dalle_donne_from_geometry(pin_diameter: f64, wire_diameter: f64, wire_lead_length: f64) -> Self { /* ... */ }
  ```
  Build a [`FsWallFriction::BaxiDalleDonne`] from wire-wrapped-bundle

- ```rust
  pub fn no_kazimi_from_geometry(pin_diameter: f64, wire_diameter: f64, wire_lead_length: f64) -> Self { /* ... */ }
  ```
  Build a [`FsWallFriction::NoKazimi`] from wire-wrapped-bundle geometry

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
    fn clone(self: &Self) -> FsWallFriction { /* ... */ }
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
    fn eq(self: &Self, other: &FsWallFriction) -> bool { /* ... */ }
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
## Module `ff_drag`

# `closures::ff_drag` — fluid-fluid interfacial drag & two-phase multipliers

Rust port of GeN-Foam's `physicsModels/dragModels/{FFDragCoefficientModels,
twoPhaseDragMultiplierModels}`. Two independent, closed enum families (no
`dyn` dispatch):

- [`interfacial::FfDragCoefficient`] — interfacial drag between the two
  fluid phases (Wallis, SchillerNaumann, Bestion/BestionTRACE, Autruffe,
  NoKazimi), evaluated on an [`interfacial::FfInterfacialState`].
- [`multipliers::TwoPhaseDragMultiplier`] — two-phase friction multipliers
  applied on top of a single-phase (fluid-structure) drag
  (LockhartMartinelli, ChenKalish, Kaiser74/88, KottowskiSavatteri,
  LottesFlinn, LottesFlinnNguyen, constant).

Belongs here: the pure-algebra fluid-fluid momentum-coupling correlations.
Does **not** belong here: the fluid-structure wall friction
([`super::fs_drag`]), heat transfer, or assembling the drag tensor fields
from these coefficients (mesh/field state — the solver's job, bead
op-p6p.7.11).

Tracked by bead op-p6p.7.5; see `docs/genfoam-port-plan.md`.

```rust
pub mod ff_drag { /* ... */ }
```

### Re-exports

#### Re-export `FfDragCoefficient`

```rust
pub use interfacial::FfDragCoefficient;
```

#### Re-export `FfInterfacialState`

```rust
pub use interfacial::FfInterfacialState;
```

#### Re-export `TwoPhaseDragMultiplier`

```rust
pub use multipliers::TwoPhaseDragMultiplier;
```

## Module `heat_transfer`

# `closures::heat_transfer` — fluid-structure & fluid-fluid heat transfer

Rust port of GeN-Foam's `physicsModels/heatTransferModels/**` — the largest
closure family in the workspace by upstream line count. It covers wall
(fluid-structure, "FS") and interfacial (fluid-fluid, "FF") heat-transfer
coefficients: single-phase forced convection, pool boiling, and a
multi-regime boiling dispatcher that blends them, plus the sub-models that
feed it (critical heat flux, Leidenfrost temperature, onset-of-nucleate-
boiling temperature, flow-enhancement / suppression factors, sub-cooled
boiling fraction).

Belongs here: wall/interfacial **heat-transfer coefficients and heat
fluxes** as pure functions of already-known local state (temperatures,
pressure, Reynolds/Prandtl numbers, fluid properties). Does **not** belong
here: phase-change mass-transfer rates ([`super::phase_change`]) — see
"Scope boundary" below for the one place this matters.

## Sub-modules

- [`fs_htc`] — fluid-structure forced-convection Nusselt correlations
  (Dittus-Boelter-form `Nu = A + B Re^C Pr^D`, with an optional wall-
  temperature-ratio term or a wall-resistance combination) and pool-boiling
  correlations (Shah, Gorenflo).
- [`boiling`] — the multi-regime boiling dispatcher (condensation /
  single-phase / nucleate-boiling superposition, TRACE-style) and the
  `superpositionNucleateBoiling` htc combiner.
- [`chf`] — critical-heat-flux, Leidenfrost temperature, onset-of-
  nucleate-boiling temperature, and the flow-enhancement / suppression /
  sub-cooled-boiling-fraction sub-models that feed [`boiling`].
- [`ff_htc`] — fluid-fluid (interfacial) forced-convection Nusselt
  correlation.

## Scope boundary: sub-cooled boiling mass transfer

Upstream's `multiRegimeBoiling::value()` also writes a wall mass-transfer
source term (`dmdtW`, the sub-cooled-boiling vapour-generation rate) into a
*mutable field owned by the phase-change model* as a side effect of
computing the htc — a real `fUcK eNcApSuLaTi1o0N` moment even by upstream's
own code-comment admission (see `subCooledBoilingFractionModel.H`).
[`boiling::multi_regime_boiling_htc`] instead **returns** the two
ingredients that feed that term (`nucleate_boiling_heat_flux`,
`forced_convection_heat_flux`) as plain values; the caller combines them
with a [`chf::SubcooledBoilingFraction`] and writes the resulting mass rate
wherever the phase-change coupling (a different bead) puts it. No mutable
shared state crosses this module boundary.

## Local `uom` aliases

[`super::units`] (this crate's wired `thermal_hydraulics::units` module)
already provides [`HeatTransferCoefficient`](crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient),
[`HeatFlux`](crate::genfoam::thermal_hydraulics::units::HeatFlux), and
[`ReynoldsNumber`](crate::genfoam::thermal_hydraulics::units::ReynoldsNumber);
all sub-modules here use those. This module additionally needs a few named
quantities that module does not yet define — [`PrandtlNumber`] and
[`LatentHeat`] below. There is also a *second*, sibling
`thermal_hydraulics::thermophysical::units` module in this crate with
overlapping candidates ([`MassDensity`](uom::si::f64::MassDensity),
[`ThermalConductivity`](uom::si::f64::ThermalConductivity), …). That
sibling module is now wired in (`thermophysical::units` exists and exports
its own `PrandtlNumber`), so the two aliases below are a genuine
duplication rather than a workaround; folding them — and the sibling's —
into `thermal_hydraulics::units` is an open follow-up.

All other quantities used across this family (temperatures, pressures,
lengths, densities, …) are `uom`'s own already-named `f64` quantity types
(`ThermodynamicTemperature`, `TemperatureInterval`, `Pressure`, `Length`,
`MassDensity`, `ThermalConductivity`, `DynamicViscosity`, `Velocity`,
`SurfaceTension`), imported directly per file — matching the convention
already used elsewhere in this crate (e.g. `genfoam::thermo_mechanics::mesh_solve`).

## What is deferred (not ported; omitted rather than half-stubbed)

- **`multiRegimeBoilingTRACE`, `multiRegimeBoilingTRACECHF`,
  `multiRegimeBoilingVapourTRACE`** — TRACE-specific variants layered on
  top of the base `multiRegimeBoiling` dispatcher with additional CHF/
  post-CHF wiring this port does not include (see next point).
- **`NusseltWallAndHfromFMU`** — couples to an external FMU (Functional
  Mock-up Unit) co-simulation; out of scope for a pure-algebra port.
- **CHF `lookUpTableCHF`** — a 3-D (pressure, mass-flux, quality) table
  interpolation over externally-supplied tabulated data
  (`InterpolateTablesGF`/`interpolation2DTable` infrastructure); porting it
  faithfully needs that table-interpolation machinery (out of scope here)
  and real published table data to verify against. [`chf::CriticalHeatFlux`]
  only ports `constantCHF`.
- **Post-CHF `CachardLiquid`/`CachardVapour`** (inverted-annular-flow film
  models) — both compute a vapour-film thickness from a term
  `pow(pi/max(DRi,1e-6),2)` where, reading the constructor,
  `pi` is bound to `p_[celli]`, the cell **pressure** field (`"p"` in the
  OpenFOAM registry) — not the rod pitch, despite the physically sensible
  reading (a pitch-to-diameter ratio) being what the surrounding
  dimensionless bracket `1 + alpha*(...)` requires, and despite a sibling
  model in the same family (`lookUpTableCHF`) taking an explicit
  `PitchToDiameter` dictionary parameter for exactly this purpose. Divided
  by a diameter, `pressure / DR` is not dimensionless — `uom` correctly
  refuses to compile the literal expression (`1.0 + Ratio::something`
  requires the something to be dimensionless), which is exactly the class
  of bug this port's type system exists to catch. Resolving which field
  was actually intended needs the upstream `structure`/`FSPair` classes,
  which are out of this bead's scope, so both models are omitted rather
  than guessed at.
- **`multiRegimeBoiling`'s post-CHF branch** — upstream's own `value()`
  hardcodes `TCHFi = 1e69` with the comment "needs dedicated model for its
  setting", i.e. the branch is unreachable and undocumented even upstream.
  [`boiling::multi_regime_boiling_htc`] matches this (no post-CHF branch)
  rather than inventing one.
- **FF `NoKazimi`** — see [`ff_htc`]'s module doc for why (an undocumented
  `fluid::operator[]` use whose semantics cannot be confirmed from the
  available headers).

Everything else named in bead op-p6p.7.6 is ported and V&V'd in
[`tests`].

```rust
pub mod heat_transfer { /* ... */ }
```

### Modules

## Module `boiling`

# `boiling` — nucleate-boiling superposition & the multi-regime dispatcher

Two functions, both faithful ports of GeN-Foam's boiling-htc combiners:

- [`superposition_nucleate_boiling_htc`] — the simple weighted sum
  `h = h_FC*F + h_PB*S` (upstream `superpositionNucleateBoiling`), for
  flows that transition from single-phase convection straight into
  nucleate boiling with no sub-cooled-boiling region in between.
- [`multi_regime_boiling_htc`] — the fuller TRACE-philosophy regime map
  (upstream `multiRegimeBoiling`): condensation / optional film
  condensation below `T_sat`, plain forced convection between `T_sat` and
  the onset-of-nucleate-boiling temperature, and Chen-style flux- or
  htc-superposition nucleate boiling above it.

**Not ported here** (see the parent module doc for the full rationale):
the TRACE CHF/post-CHF variants, and the `dmdtW` sub-cooled-boiling
mass-transfer side effect — [`multi_regime_boiling_htc`] returns the two
heat-flux ingredients that side effect needs
([`MultiRegimeBoilingResult::nucleate_boiling_heat_flux`],
[`MultiRegimeBoilingResult::forced_convection_heat_flux`]) instead of
writing to shared mutable state.

## Two upstream quirks reproduced verbatim (not silently fixed)

Per this repository's rule against silently changing ported physics,
[`multi_regime_boiling_htc`] reproduces two literal upstream expressions
exactly, each flagged with a `NOTE(port):` comment at the call site:

1. The film-condensation blend fraction in the `T_wall < T_sat` branch is
   upstream's literal `f = 0.9 - alpha/0.1` (division binds tighter than
   subtraction in C++, same as Rust), not the evidently-intended
   `(0.9 - alpha)/0.1` — the former leaves `[0, 1]` for any
   `alpha in (0.8, 0.9)` except very close to `0.89`.
2. The temperature-difference floor in the flux-superposition branch is
   upstream's literal `dT = (dT >= 0) ? max(dT,1e-3) : min(-dT,-1e-3)`;
   since `-dT > 0` whenever `dT < 0`, the `else` arm always evaluates to
   exactly `-1e-3` regardless of `|dT|` (almost certainly meant
   `-max(-dT, 1e-3)`).

Both are pure scalar arithmetic (no `uom` dimensional-consistency check
applies), so — unlike the two dimensionally-broken formulas documented in
[`super::chf`] that `uom` refuses to compile — reproducing them verbatim
was a choice, not a compiler requirement. It was made because "the
equation is wrong, not the tolerance" cuts both ways: an AI-assisted port
silently "improving" upstream physics without sign-off is exactly the
failure mode this workspace's guardrails exist to prevent.

```rust
pub mod boiling { /* ... */ }
```

### Types

#### Struct `MultiRegimeBoilingInputs`

Inputs to [`multi_regime_boiling_htc`]. Mirrors GeN-Foam's
`multiRegimeBoiling` dictionary, whose several sub-models are all
optional (`this->found("...")`) — the `Option` fields here play the same
role.

```rust
pub struct MultiRegimeBoilingInputs {
    pub t_wall: uom::si::f64::ThermodynamicTemperature,
    pub t_fluid: uom::si::f64::ThermodynamicTemperature,
    pub t_sat: uom::si::f64::ThermodynamicTemperature,
    pub vapour_void_fraction: uom::si::f64::Ratio,
    pub htc_forced_convection_enhanced: crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient,
    pub htc_pool_boiling: crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient,
    pub htc_film_condensation: Option<crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient>,
    pub suppression_factor: Option<uom::si::f64::Ratio>,
    pub t_onset_nucleate_boiling: Option<uom::si::f64::ThermodynamicTemperature>,
    pub htc_pool_boiling_at_onset: Option<crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient>,
    pub superposition_exponent: f64,
    pub heat_flux_superposition: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_wall` | `uom::si::f64::ThermodynamicTemperature` | Wall (structure) temperature `T_wall`. |
| `t_fluid` | `uom::si::f64::ThermodynamicTemperature` | Bulk fluid temperature `T_fluid`. |
| `t_sat` | `uom::si::f64::ThermodynamicTemperature` | Saturation temperature `T_sat` at the local pressure. |
| `vapour_void_fraction` | `uom::si::f64::Ratio` | Vapour void fraction `alpha_v = 1 - alpha_liquid,normalized`. |
| `htc_forced_convection_enhanced` | `crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient` | Two-phase forced-convection htc, **already** multiplied by the flow-<br>enhancement factor `F` (upstream's cached `htc2pFCi_ =<br>htcFCPtr_->value(celli) * FPtr_->value(celli)`). |
| `htc_pool_boiling` | `crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient` | Pool-boiling htc evaluated at `(T_wall, T_sat)` (upstream<br>`htcPBPtr_->value(celli)`). |
| `htc_film_condensation` | `Option<crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient>` | Film-condensation htc, if a `filmCondensationModel` was configured<br>(`None` skips straight to `htc_forced_convection_enhanced` below<br>`T_sat`, matching upstream's `htcCndPtr_.valid()` guard). |
| `suppression_factor` | `Option<uom::si::f64::Ratio>` | Suppression factor `S`, if a `suppressionFactorModel` was configured. |
| `t_onset_nucleate_boiling` | `Option<uom::si::f64::ThermodynamicTemperature>` | Onset-of-nucleate-boiling temperature, if a `nucleateBoilingOnsetModel`<br>was configured. `None` uses `T_sat` in its place, matching upstream's<br>`(TONBPtr_.valid()) ? TONBPtr_->value(...) : Tsati`. |
| `htc_pool_boiling_at_onset` | `Option<crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient>` | Pool-boiling htc evaluated at `(T_onset_nucleate_boiling, T_sat)`<br>instead of `(T_wall, T_sat)` — only read in flux-superposition mode<br>when a suppression factor is **not** configured but an ONB<br>temperature **is** (upstream's `qBIi` "boiling-inception" heat-flux<br>term, which avoids a discontinuity at the boiling onset). Pass `None`<br>to fall back to plain `qFC^n + qPB^n` superposition in that case. |
| `superposition_exponent` | `f64` | Superposition exponent `n` (upstream dictionary entry<br>`superpositionExponent`; TRACE-philosophy codes typically use `n` in<br>`[1, 4]`, with higher `n` sharpening the transition between regimes). |
| `heat_flux_superposition` | `bool` | `true` superposes **heat fluxes** then divides by `deltaT` to recover<br>an htc (upstream's `heatFluxSuperposition = true`, the TRACE<br>approach); `false` superposes **heat-transfer coefficients** directly. |

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
    fn clone(self: &Self) -> MultiRegimeBoilingInputs { /* ... */ }
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
#### Struct `MultiRegimeBoilingResult`

Output of [`multi_regime_boiling_htc`].

```rust
pub struct MultiRegimeBoilingResult {
    pub htc: crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient,
    pub nucleate_boiling_heat_flux: Option<crate::genfoam::thermal_hydraulics::units::HeatFlux>,
    pub forced_convection_heat_flux: Option<crate::genfoam::thermal_hydraulics::units::HeatFlux>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `htc` | `crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient` | The dispatched heat-transfer coefficient. |
| `nucleate_boiling_heat_flux` | `Option<crate::genfoam::thermal_hydraulics::units::HeatFlux>` | Nucleate-boiling heat flux `q_NB`, the ingredient<br>[`super::chf::SubcooledBoilingFraction`] needs to compute the wall vapour-<br>generation rate during sub-cooled boiling (upstream's `qNBi`, fed to<br>its `setDmdtW` side effect). `Some` only in the flux-superposition<br>nucleate-boiling branch (`heat_flux_superposition == true` and<br>`T_wall >= T_onset_nucleate_boiling >= T_sat`); `None` everywhere<br>else, including the htc-superposition nucleate-boiling branch — see<br>the field doc on why that branch cannot supply a faithful value. |
| `forced_convection_heat_flux` | `Option<crate::genfoam::thermal_hydraulics::units::HeatFlux>` | Enhanced forced-convection heat flux `q_FC = h_2pFC * (T_wall -<br>T_fluid)`, populated alongside `nucleate_boiling_heat_flux`. |

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
    fn clone(self: &Self) -> MultiRegimeBoilingResult { /* ... */ }
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

#### Function `superposition_nucleate_boiling_htc`

**Attributes:**

- `MustUse { reason: None }`

`h = h_FC * F + h_PB * S` — the simplest boiling-htc combination, for
flows with no sub-cooled-boiling region between single-phase convection
and nucleate boiling (upstream `superpositionNucleateBoiling`).

```rust
pub fn superposition_nucleate_boiling_htc(htc_forced_convection: crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient, flow_enhancement_factor: uom::si::f64::Ratio, htc_pool_boiling: crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient, suppression_factor: uom::si::f64::Ratio) -> crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient { /* ... */ }
```

#### Function `multi_regime_boiling_htc`

**Attributes:**

- `MustUse { reason: None }`

Dispatch the boiling regime and return its heat-transfer coefficient.

Faithful port of `multiRegimeBoiling::value()`'s regime map, minus the
post-CHF branch (upstream itself hardcodes `TCHFi = 1e69`, i.e.
unreachable — "needs dedicated model for its setting", per its own
comment) and the `dmdtW` mass-transfer side effect (see the module doc).

Regime map, matching upstream exactly:
1. `T_wall < T_sat`: either plain enhanced forced convection, or — if a
   film-condensation model is configured and `alpha_vapour > 0.8` — a
   blend into (or full) film condensation as `alpha_vapour -> 0.9`.
2. `T_sat <= T_wall < T_onset_nucleate_boiling`: plain enhanced forced
   convection (nothing "special" happens yet).
3. `T_wall >= T_onset_nucleate_boiling`: nucleate/sub-cooled boiling,
   combining `htc_forced_convection_enhanced` and `htc_pool_boiling` via
   `superposition_exponent`-power superposition, optionally moderated by
   `suppression_factor` or `htc_pool_boiling_at_onset`.

```rust
pub fn multi_regime_boiling_htc(inputs: MultiRegimeBoilingInputs) -> MultiRegimeBoilingResult { /* ... */ }
```

## Module `chf`

# `chf` — critical-heat-flux and boiling sub-models

The sub-models [`boiling::multi_regime_boiling_htc`](super::boiling::multi_regime_boiling_htc)
composes, each a closed enum:

| Enum | Upstream classes | Role |
|---|---|---|
| [`CriticalHeatFlux`] | `constantCHF` | Critical heat flux (departure from nucleate boiling) |
| [`LeidenfrostTemperature`] | `GroeneveldStewart` | Minimum film-boiling ("rewetting") temperature |
| [`OnsetOfNucleateBoilingTemperature`] | `Basu` | Wall temperature at which nucleate boiling begins |
| [`FlowEnhancementFactor`] | `Chen`, `RezkallahSims`, `COBRA_TF` | Enhances forced convection under two-phase flow |
| [`SuppressionFactor`] | `Chen`, `COBRA_TF` | Suppresses pool boiling as flow becomes annular |
| [`SubcooledBoilingFraction`] | `constant`, `SahaZuber` | Fraction of the sub-cooled pool-boiling heat flux that nets vapour |

## Shared helper

[`two_phase_mixture_reynolds_number`] is used by **two** upstream models
that independently compute the identical formula:
`flowEnhancementFactorModels::COBRA_TF` and `suppressionFactorModels::Chen`.
Both need a "mixture" Reynolds number assembled from *both* phases' local
density, velocity, viscosity, and void fraction — this port factors that
shared computation into one function instead of duplicating it.

## Velocity as a scalar (dimensional note)

Upstream forms the mixture Reynolds number from the **vector** sum
`mag(alpha1*rho1*U1 + alpha2*rho2*U2)` (`U()` is a `volVectorField`, not
the scalar `magU()` used elsewhere in the same classes). This crate has no
shared vector/tensor field type in scope for this closure family (the rest
of `closures::*`, e.g. `ReynoldsNumber` in [`fs_drag`](super::super::fs_drag),
is already scalarised at the API boundary), so
[`two_phase_mixture_reynolds_number`] takes each phase's velocity as a
**signed scalar** (its component along the flow direction) and computes
`abs(alpha1*rho1*u1 + alpha2*rho2*u2)`. This is exact for co-linear
(1-D channel) flow — the common case for these porous-channel closures —
and is the same scalar reduction the rest of this crate already applies to
upstream's tensor-porous-media quantities.

## Deferred (see the parent module doc for the full list + rationale)

CHF `lookUpTableCHF` (3-D table interpolation) and the post-CHF
`CachardLiquid`/`CachardVapour` models (dimensionally-inconsistent upstream
formula, `uom` correctly refuses to compile it) are **not** ported here.

## References

- R.T. Lahey Jr. and F.J. Moody, *The Thermal-Hydraulics of a Boiling
  Water Nuclear Reactor*, ANS, 1993 (critical heat flux background).
- D.C. Groeneveld and J.C. Stewart, "The minimum film boiling temperature
  for water during film boiling collapse," *NUREG/CP-0022*, 1982.
- J.C. Chen, "Correlation for boiling heat transfer to saturated fluids in
  convective flow," *Ind. Eng. Chem. Process Des. Dev.* 5(3), 1966.
- K.S. Rezkallah and G.E. Sims, "An Examination of Correlations of Mean
  Heat Transfer Coefficients in Two-Phase Two-Component Flow in Vertical
  Tubes," *AIChE Symposium Series* 83(257), 1987.
- COBRA-TF (Coolant Boiling in Rod Arrays - Two Fluid) code manual,
  Pennsylvania State University / NRC, for the `COBRA_TF` flow-enhancement
  and suppression correlations.
- N. Basu, G.R. Warrier, V.K. Dhir, "Onset of Nucleate Boiling and Active
  Nucleation Site Density During Subcooled Flow Boiling," *J. Heat
  Transfer* 124, 2002, pp. 717-728.
- P. Saha and N. Zuber, "Point of net vapor generation and vapor void
  fraction in subcooled boiling," *Proc. 5th Int. Heat Transfer Conf.*,
  Tokyo, 1974, paper B4.7 (the published `Δ T_sub` correlation this port
  reproduces — see [`SubcooledBoilingFraction::SahaZuber`]'s doc for why
  it deviates from upstream's literal C++).

```rust
pub mod chf { /* ... */ }
```

### Types

#### Enum `CriticalHeatFlux`

Critical heat flux (CHF) — the wall heat flux at which nucleate boiling
gives way to film boiling (departure from nucleate boiling / dryout).

Only upstream's `constantCHF` is ported; `lookUpTableCHF` (3-D pressure /
mass-flux / quality table interpolation) is deferred — see the module doc.

```rust
pub enum CriticalHeatFlux {
    Constant(crate::genfoam::thermal_hydraulics::units::HeatFlux),
}
```

##### Variants

###### `Constant`

A fixed CHF value, read directly from phase properties upstream
(`constantCHF`'s `value` dictionary entry).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::genfoam::thermal_hydraulics::units::HeatFlux` |  |

##### Implementations

###### Methods

- ```rust
  pub fn value(self: &Self) -> HeatFlux { /* ... */ }
  ```
  The critical heat flux.

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
    fn clone(self: &Self) -> CriticalHeatFlux { /* ... */ }
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
    fn eq(self: &Self, other: &CriticalHeatFlux) -> bool { /* ... */ }
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
#### Enum `LeidenfrostTemperature`

Leidenfrost (minimum film-boiling / rewetting) temperature.

```rust
pub enum LeidenfrostTemperature {
    GroeneveldStewart {
        critical_pressure: uom::si::f64::Pressure,
    },
}
```

##### Variants

###### `GroeneveldStewart`

Groeneveld & Stewart (1982) correlation, as ported by TRACE and by
GeN-Foam from it. Valid for `p < 9 MPa`; upstream notes it "lack[s]
the term due to quality" (i.e. no flow-quality dependence is modelled)
and linearly ramps to `T_sat` at the critical pressure above 9 MPa so
the temperature is still defined (if not validated) there.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `critical_pressure` | `uom::si::f64::Pressure` | Fluid critical pressure (Pa), used only above 9 MPa for the ramp. |

##### Implementations

###### Methods

- ```rust
  pub fn temperature(self: &Self, pressure: Pressure, t_sat: ThermodynamicTemperature) -> ThermodynamicTemperature { /* ... */ }
  ```
  The Leidenfrost temperature `T_min`. `t_sat` is the local saturation

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
    fn clone(self: &Self) -> LeidenfrostTemperature { /* ... */ }
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
    fn eq(self: &Self, other: &LeidenfrostTemperature) -> bool { /* ... */ }
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
#### Enum `OnsetOfNucleateBoilingTemperature`

Onset-of-nucleate-boiling (ONB) wall temperature — the wall superheat at
which the first vapour bubbles nucleate, below which flow boiling behaves
as single-phase forced convection.

```rust
pub enum OnsetOfNucleateBoilingTemperature {
    Basu {
        surface_tension: uom::si::f64::SurfaceTension,
        contact_angle: uom::si::f64::Angle,
    },
}
```

##### Variants

###### `Basu`

Basu, Warrier & Dhir (2002) correlation, in terms of the local
forced-convection heat-transfer coefficient (see
[`OnsetOfNucleateBoilingTemperature::temperature`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `surface_tension` | `uom::si::f64::SurfaceTension` | Liquid surface tension `sigma` (N/m). |
| `contact_angle` | `uom::si::f64::Angle` | Liquid-structure contact angle (degrees). |

##### Implementations

###### Methods

- ```rust
  pub fn temperature(self: &Self, htc_forced_convection_enhanced: HeatTransferCoefficient, t_fluid: ThermodynamicTemperature, t_sat: ThermodynamicTemperature, other_phase_density: MassDensity, latent_heat: LatentHeat, thermal_conductivity: ThermalConductivity) -> ThermodynamicTemperature { /* ... */ }
  ```
  The ONB temperature `T_ONB`.

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
    fn clone(self: &Self) -> OnsetOfNucleateBoilingTemperature { /* ... */ }
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
    fn eq(self: &Self, other: &OnsetOfNucleateBoilingTemperature) -> bool { /* ... */ }
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
#### Enum `FlowEnhancementFactor`

Two-phase flow-enhancement factor `F` — multiplies the single-phase
forced-convection htc to account for bubble-induced turbulence and the
velocity increase from vapour generation.

Evaluate with [`FlowEnhancementFactor::value`], which takes a
[`FlowEnhancementInputs`] carrying every driving quantity any variant
might need (only the fields the active variant actually reads are used —
see each variant's doc).

```rust
pub enum FlowEnhancementFactor {
    Chen {
        max_value: uom::si::f64::Ratio,
    },
    RezkallahSims {
        exponent: f64,
        max_value: uom::si::f64::Ratio,
    },
    CobraTf {
        max_value: uom::si::f64::Ratio,
    },
}
```

##### Variants

###### `Chen`

Chen (1966), keyed to the inverse Lockhart-Martinelli parameter
`1/X_tt`. Uses [`FlowEnhancementInputs::inverse_lockhart_martinelli`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `max_value` | `uom::si::f64::Ratio` | Maximum allowed value of `F` (upstream `maxValue_`, a<br>`flowEnhancementFactorModel` base-class dictionary entry). |

###### `RezkallahSims`

Rezkallah & Sims (1987), keyed to the liquid void fraction; TRACE's
numerically-preferred default over Chen at low pressure/flow (see the
upstream doc comment, citing NRC ML120060218, pp. 263-267). Uses
[`FlowEnhancementInputs::liquid_void_fraction`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `exponent` | `f64` | Correlation exponent (upstream dictionary entry `exp`). |
| `max_value` | `uom::si::f64::Ratio` | Maximum allowed value of `F`. |

###### `CobraTf`

COBRA-TF, keyed to the ratio of a two-phase "mixture" Reynolds number
to the single-phase Reynolds number (see
[`two_phase_mixture_reynolds_number`]). Uses
[`FlowEnhancementInputs::mixture_reynolds_number`] and
[`FlowEnhancementInputs::single_phase_reynolds_number`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `max_value` | `uom::si::f64::Ratio` | Maximum allowed value of `F`. |

##### Implementations

###### Methods

- ```rust
  pub fn value(self: &Self, inputs: FlowEnhancementInputs) -> Ratio { /* ... */ }
  ```
  Evaluate `F`.

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
    fn clone(self: &Self) -> FlowEnhancementFactor { /* ... */ }
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
    fn eq(self: &Self, other: &FlowEnhancementFactor) -> bool { /* ... */ }
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
#### Struct `FlowEnhancementInputs`

Every driving quantity [`FlowEnhancementFactor::value`] might need; the
active variant reads only the field(s) documented on it.

```rust
pub struct FlowEnhancementInputs {
    pub inverse_lockhart_martinelli: uom::si::f64::Ratio,
    pub liquid_void_fraction: uom::si::f64::Ratio,
    pub mixture_reynolds_number: crate::genfoam::thermal_hydraulics::units::ReynoldsNumber,
    pub single_phase_reynolds_number: crate::genfoam::thermal_hydraulics::units::ReynoldsNumber,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inverse_lockhart_martinelli` | `uom::si::f64::Ratio` | Inverse Lockhart-Martinelli parameter `1/X_tt` (dimensionless). Used<br>by [`FlowEnhancementFactor::Chen`]. |
| `liquid_void_fraction` | `uom::si::f64::Ratio` | Liquid void fraction `alpha_l` (dimensionless). Used by<br>[`FlowEnhancementFactor::RezkallahSims`]. |
| `mixture_reynolds_number` | `crate::genfoam::thermal_hydraulics::units::ReynoldsNumber` | Two-phase mixture Reynolds number (see<br>[`two_phase_mixture_reynolds_number`]). Used by<br>[`FlowEnhancementFactor::CobraTf`]. |
| `single_phase_reynolds_number` | `crate::genfoam::thermal_hydraulics::units::ReynoldsNumber` | Single-phase Reynolds number of the phase this factor multiplies<br>(upstream `pair_.Re()`). Used by [`FlowEnhancementFactor::CobraTf`]. |

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
    fn clone(self: &Self) -> FlowEnhancementInputs { /* ... */ }
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
#### Enum `SuppressionFactor`

Pool-boiling suppression factor `S` — multiplies the pool-boiling htc to
"turn it off" as the flow regime transitions towards annular / film
boiling and pool boiling becomes physically implausible.

Evaluate with [`SuppressionFactor::value`], which takes a
[`SuppressionInputs`]; as with [`FlowEnhancementFactor`], only the
field(s) the active variant reads are used.

```rust
pub enum SuppressionFactor {
    Chen,
    CobraTf,
}
```

##### Variants

###### `Chen`

Chen (1966), `S = 1 / (1 + 2.53e-6 * Re_mix^1.17)`, using the same
two-phase mixture Reynolds number as
[`FlowEnhancementFactor::CobraTf`] (see
[`two_phase_mixture_reynolds_number`]).

###### `CobraTf`

COBRA-TF, a linear temperature-ratio clamp
`S = clamp((T_fluid - T_sat) / (T_wall - T_sat), 0, 1)`.

##### Implementations

###### Methods

- ```rust
  pub fn value(self: &Self, inputs: SuppressionInputs) -> Ratio { /* ... */ }
  ```
  Evaluate `S`.

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
    fn clone(self: &Self) -> SuppressionFactor { /* ... */ }
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
    fn eq(self: &Self, other: &SuppressionFactor) -> bool { /* ... */ }
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
#### Struct `SuppressionInputs`

Every driving quantity [`SuppressionFactor::value`] might need.

```rust
pub struct SuppressionInputs {
    pub mixture_reynolds_number: crate::genfoam::thermal_hydraulics::units::ReynoldsNumber,
    pub t_fluid: uom::si::f64::ThermodynamicTemperature,
    pub t_wall: uom::si::f64::ThermodynamicTemperature,
    pub t_sat: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mixture_reynolds_number` | `crate::genfoam::thermal_hydraulics::units::ReynoldsNumber` | Two-phase mixture Reynolds number. Used by [`SuppressionFactor::Chen`]. |
| `t_fluid` | `uom::si::f64::ThermodynamicTemperature` | Bulk fluid temperature. Used by [`SuppressionFactor::CobraTf`]. |
| `t_wall` | `uom::si::f64::ThermodynamicTemperature` | Wall temperature. Used by [`SuppressionFactor::CobraTf`]. |
| `t_sat` | `uom::si::f64::ThermodynamicTemperature` | Saturation temperature. Used by [`SuppressionFactor::CobraTf`]. |

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
    fn clone(self: &Self) -> SuppressionInputs { /* ... */ }
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
#### Enum `SubcooledBoilingFraction`

Fraction of the "would-be" pool-boiling heat flux that actually results in
net vapour generation during sub-cooled (bulk liquid below `T_sat`)
boiling — `0` means all of it re-condenses into the sub-cooled bulk, `1`
means all of it nets vapour.

```rust
pub enum SubcooledBoilingFraction {
    Constant {
        value: uom::si::f64::Ratio,
    },
    SahaZuber,
}
```

##### Variants

###### `Constant`

A fixed fraction, independent of the local heat flux.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `value` | `uom::si::f64::Ratio` | The fixed fraction (upstream dictionary entry `value`). |

###### `SahaZuber`

Saha & Zuber (1974) point-of-net-vapour-generation correlation.

##### Implementations

###### Methods

- ```rust
  pub fn fraction(self: &Self, heat_flux: HeatFlux, thermal_conductivity: ThermalConductivity, hydraulic_diameter: Length, re: ReynoldsNumber, pr: PrandtlNumber, t_sat: ThermodynamicTemperature, t_fluid: ThermodynamicTemperature) -> Ratio { /* ... */ }
  ```
  Evaluate the fraction. `heat_flux` is the local wall heat flux driving

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
    fn clone(self: &Self) -> SubcooledBoilingFraction { /* ... */ }
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
    fn eq(self: &Self, other: &SubcooledBoilingFraction) -> bool { /* ... */ }
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
### Functions

#### Function `two_phase_mixture_reynolds_number`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Two-phase "mixture" Reynolds number shared by
[`FlowEnhancementFactor::CobraTf`] and [`SuppressionFactor::Chen`]:
`Re_mix = max(|alpha_1*rho_1*u_1 + alpha_2*rho_2*u_2| * D_h /
(alpha_1*mu_1 + alpha_2*mu_2), 10)` — see the module doc for the
vector-to-scalar velocity note.

Takes both phases' void fraction, density, velocity, and viscosity plus the
hydraulic diameter (nine dimensioned inputs) — this is the mixture Reynolds
number's intrinsic argument list, so `too_many_arguments` is allowed here.

```rust
pub fn two_phase_mixture_reynolds_number(alpha_1: uom::si::f64::Ratio, rho_1: uom::si::f64::MassDensity, u_1: uom::si::f64::Velocity, mu_1: uom::si::f64::DynamicViscosity, alpha_2: uom::si::f64::Ratio, rho_2: uom::si::f64::MassDensity, u_2: uom::si::f64::Velocity, mu_2: uom::si::f64::DynamicViscosity, hydraulic_diameter: uom::si::f64::Length) -> crate::genfoam::thermal_hydraulics::units::ReynoldsNumber { /* ... */ }
```

#### Function `contact_angle_from_degrees`

**Attributes:**

- `MustUse { reason: None }`

Convenience: build a [`SurfaceTension`]/[`Angle`] pair's contact angle
from degrees, matching upstream's dictionary entry
(`dict.get<scalar>("contactAngle")` in degrees, converted internally).

```rust
pub fn contact_angle_from_degrees(degrees: f64) -> uom::si::f64::Angle { /* ... */ }
```

## Module `ff_htc`

# `ff_htc` — fluid-fluid (interfacial) forced-convection HTC correlation

[`FfForcedConvectionHtc`] ports upstream's `FFHeatTransferCoefficientModels::Nusselt`
— the interfacial-transfer analogue of [`super::fs_htc::FsForcedConvectionHtc::Nusselt`],
same `Nu = A + B*Re^C*Pr^D` form (no wall-temperature-ratio term; there is
no "wall" at a fluid-fluid interface), `h = Nu*k/D_h`, evaluated with the
**dispersed**-phase Reynolds number and hydraulic diameter and the
**continuous**-phase Prandtl number and thermal conductivity (matching
upstream's `pair.PrContinuous()`/`pair.DhDispersed()` field selection —
the two phases play asymmetric roles at an interface, unlike a
fluid-structure pair).

## Deferred: `NoKazimi`

Upstream's `FFHeatTransferCoefficientModels::NoKazimi` (interfacial
condensation/evaporation htc, taking the lesser of a continuum
correlation and a kinetic-theory/molecular-effusion limit) is **not**
ported. Its continuum-limit term multiplies by `bulkFluid_[celli]` — a
`fluid::operator[]` call whose return semantics are not documented in
`FFHeatTransferCoefficientModel.H`/`NoKazimiFFHeatTransferCoefficient.H`
and are not derivable from either header (the void-fraction-weighted
`normalized()` accessor is a *separate*, already-used member,
`alpha_(bulkFluid_.normalized())`, so `operator[]` is not simply that).
Confirming its meaning needs the upstream `fluid` class implementation,
which is out of this bead's file scope (it belongs to
[`super::super::super::phase`], and per the task brief this port does not
reach into sibling phase modules). Porting a physics correlation on a
guessed interpretation would risk exactly the "fabricated result" this
workspace's compliance rules prohibit, so it is omitted rather than
half-stubbed. Tracked for follow-up once the `phase`/`fluid` port exposes
(and documents) that accessor.

```rust
pub mod ff_htc { /* ... */ }
```

### Types

#### Enum `FfForcedConvectionHtc`

Fluid-fluid (interfacial) forced-convection heat-transfer coefficient.

```rust
pub enum FfForcedConvectionHtc {
    Nusselt {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
    },
}
```

##### Variants

###### `Nusselt`

`Nu = A + B * Re^C * Pr^D`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Additive constant `A`. |
| `b` | `f64` | Leading coefficient `B`. `0.0` skips the `Re`/`Pr` term (`Nu = A`). |
| `c` | `f64` | Reynolds-number exponent `C`. |
| `d` | `f64` | Prandtl-number exponent `D`. |

##### Implementations

###### Methods

- ```rust
  pub fn heat_transfer_coefficient(self: &Self, re: ReynoldsNumber, pr: PrandtlNumber, k: ThermalConductivity, d_h: Length) -> HeatTransferCoefficient { /* ... */ }
  ```
  Evaluate `h = Nu * k / D_h`. `re`, `pr` are the dispersed-phase

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
    fn clone(self: &Self) -> FfForcedConvectionHtc { /* ... */ }
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
    fn eq(self: &Self, other: &FfForcedConvectionHtc) -> bool { /* ... */ }
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
## Module `fs_htc`

# `fs_htc` — fluid-structure forced-convection & pool-boiling HTC correlations

Two closed-enum families, each a faithful port of one GeN-Foam
`FSHeatTransferCoefficientModel` sub-family:

| Enum | Upstream classes | Physical role |
|---|---|---|
| [`FsForcedConvectionHtc`] | `Nusselt`, `NusseltAndWall` | Single-/two-phase forced convection between the fluid and the wall |
| [`PoolBoilingHtc`] | `Shah`, `Gorenflo` | Saturated pool-boiling heat transfer at the wall |

Both families are ultimately Nusselt-number correlations
`Nu = A + B * Re^C * Pr^D * (...)`; the conversion to a heat-transfer
coefficient is always `h = Nu * k / D_h` (`k` the fluid thermal
conductivity, `D_h` the hydraulic diameter) — the classic definition of
the Nusselt number. Each `impl` documents exactly which formula it
evaluates so the conversion is visible at the call site, not buried.

## References

- Dittus, F.W. and Boelter, L.M.K., *University of California Publications
  in Engineering*, Vol. 2, 1930 (the archetype this family generalises).
- Walton, C.M., "Thermal-hydraulic loss coefficients for CANDU fuel
  bundles", 1992, eq. (10) "Wolf-McCarthy II" (the wall/fluid
  temperature-ratio example in `Nusselt`'s upstream doc comment).
- N.Z. Shah, pool-boiling correlation for liquid metals (sodium, Na-K),
  as used by GeN-Foam's `Shah` model.
- D. Gorenflo, "Pool Boiling," *VDI-Heat Atlas*, Sect. Ha, VDI-Verlag,
  Dusseldorf, 1993, via J.G. Collier and J.R. Thome, *Convective Boiling
  and Condensation*, 3rd ed., pp. 155-158, Oxford University Press, 1994.

```rust
pub mod fs_htc { /* ... */ }
```

### Types

#### Enum `FsForcedConvectionHtc`

Fluid-structure forced-convection heat-transfer coefficient.

Both variants are Nusselt-number correlations of the form
`Nu = A + B * Re^C * Pr^D`, converted to `h = Nu * k / D_h`. They differ in
what happens after that:

- [`FsForcedConvectionHtc::Nusselt`] optionally multiplies `Nu` by a
  wall/fluid temperature-ratio term `(T_w / T_f)^E` before the conversion
  (upstream `Nusselt`, `TypeName("NusseltReynoldsPrandtlPower")`).
- [`FsForcedConvectionHtc::NusseltAndWall`] has no temperature-ratio term,
  but instead combines the resulting fluid-side `h` **in series** with a
  fixed wall-resistance coefficient `H_wall`
  (`1/H = 1/(Nu*k/D_h) + 1/H_wall`), modelling e.g. a cladding oxide layer
  or contact resistance (upstream `NusseltAndWall`).

Evaluate with [`FsForcedConvectionHtc::heat_transfer_coefficient`].

```rust
pub enum FsForcedConvectionHtc {
    Nusselt {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        e: f64,
    },
    NusseltAndWall {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        h_wall: crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient,
    },
}
```

##### Variants

###### `Nusselt`

`Nu = A + B * Re^C * Pr^D * (T_w / T_f)^E`. `wall_fluid_temperatures`
is read only when `e != 0.0`; pass `None` (or `e = 0.0`) to omit the
temperature-ratio term entirely, matching upstream's `E_` defaulting to
`0` and its `Twall_[celli] > 0.0` guard.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Additive (fully-developed-laminar-limit) constant `A`. |
| `b` | `f64` | Leading coefficient `B` on the `Re^C Pr^D` term. If `0.0`, the<br>whole `Re`/`Pr`/temperature-ratio term is skipped (matches<br>upstream's `B_ != 0` guard) and `Nu = A`. |
| `c` | `f64` | Reynolds-number exponent `C`. |
| `d` | `f64` | Prandtl-number exponent `D`. |
| `e` | `f64` | Wall/fluid temperature-ratio exponent `E` (`0.0` disables the<br>term). |

###### `NusseltAndWall`

`Nu = A + B * Re^C * Pr^D`, combined in series with a fixed
wall-resistance coefficient `h_wall` via `h = h_fluid*h_wall /
(h_fluid + h_wall)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Additive constant `A`. |
| `b` | `f64` | Leading coefficient `B`. `0.0` skips the `Re`/`Pr` term (`Nu = A`). |
| `c` | `f64` | Reynolds-number exponent `C`. |
| `d` | `f64` | Prandtl-number exponent `D`. |
| `h_wall` | `crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient` | Fixed wall-resistance heat-transfer coefficient `H_wall`<br>(upstream dictionary entry `addH`). |

##### Implementations

###### Methods

- ```rust
  pub fn heat_transfer_coefficient(self: &Self, re: ReynoldsNumber, pr: PrandtlNumber, k: ThermalConductivity, d_h: Length, wall_fluid_temperatures: Option<(ThermodynamicTemperature, ThermodynamicTemperature)>) -> HeatTransferCoefficient { /* ... */ }
  ```
  Evaluate the forced-convection heat-transfer coefficient.

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
    fn clone(self: &Self) -> FsForcedConvectionHtc { /* ... */ }
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
    fn eq(self: &Self, other: &FsForcedConvectionHtc) -> bool { /* ... */ }
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
#### Enum `PoolBoilingHtc`

Saturated pool-boiling heat-transfer coefficient.

Both variants have the closed form `h_PB = C(pR) * q''^n * pR^m` for some
pressure-dependent coefficients, but `q''` is itself `h_PB * deltaT` — the
heat flux depends on the coefficient being solved for. Upstream breaks
this circularity by assuming `h ~ h_PB` (pool boiling dominates the total
wall heat transfer) and solving `h_PB = (C * deltaT^n * pR^m)^(1/(1-n))`
directly in terms of the wall/reference temperature difference — this is
[`PoolBoilingHtc::heat_transfer_coefficient_from_delta_t`]. Both models
also support skipping that assumption when the heat flux is already known
from elsewhere (upstream's `useExplicitHeatFlux_` dictionary option) —
[`PoolBoilingHtc::heat_transfer_coefficient_from_heat_flux`] evaluates the
original (non-circular) `h_PB = C * q''^n * pR^m` directly. Reduced
pressure `pR = p / p_crit` uses a **fixed** critical pressure per variant
(matching upstream, which hardcodes it rather than reading it from a
dictionary): `p_crit = 35 MPa` for [`PoolBoilingHtc::Shah`] ("specific to
Sodium" per upstream) and `p_crit = 22.09 MPa` for
[`PoolBoilingHtc::Gorenflo`] ("specific to Water").

Both methods return `0 W/(m^2 K)` when the driving quantity (`deltaT` or
`q''`) is non-positive, matching upstream's `if (deltaT > 0.0) ... else
return 0.0;` guard.

```rust
pub enum PoolBoilingHtc {
    Shah,
    Gorenflo {
        surface_roughness: uom::si::f64::Length,
    },
}
```

##### Variants

###### `Shah`

Shah pool-boiling correlation, `p_crit = 35 MPa` (sodium). No
configurable parameters — all coefficients (`C`, `m`, `n`) are fixed
constants upstream, linearly blended across `pR in (5e-4, 1.5e-3)` to
avoid the discontinuity Shah's original piecewise `pR = 1e-3` cutoff
would otherwise introduce.

###### `Gorenflo`

Gorenflo (1993) pool-boiling correlation, `p_crit = 22.09 MPa`
(water).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `surface_roughness` | `uom::si::f64::Length` | Absolute surface roughness `R` (metres). Upstream dictionary entry<br>`absoluteSurfaceRoughness`, defaulting to the reference roughness<br>`R0 = 4e-7 m` (i.e. `roughness_factor = (R/R0)^0.133 = 1`). |

##### Implementations

###### Methods

- ```rust
  pub fn heat_transfer_coefficient_from_delta_t(self: &Self, delta_t: TemperatureInterval, pressure: Pressure) -> HeatTransferCoefficient { /* ... */ }
  ```
  `h_PB = (C(pR) * deltaT^n * pR^m)^(1/(1-n))` for [`PoolBoilingHtc::Shah`],

- ```rust
  pub fn heat_transfer_coefficient_from_heat_flux(self: &Self, heat_flux: HeatFlux, pressure: Pressure) -> HeatTransferCoefficient { /* ... */ }
  ```
  The non-circular form `h_PB = C(pR) * q''^n * pR^m` (Shah) or

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
    fn clone(self: &Self) -> PoolBoilingHtc { /* ... */ }
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
    fn eq(self: &Self, other: &PoolBoilingHtc) -> bool { /* ... */ }
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

#### Type Alias `PrandtlNumber`

Prandtl number `Pr = c_p * mu / k` — **dimensionless**.

Local alias (see the module doc's "Local `uom` aliases" section) of
[`uom`]'s [`Ratio`]. Used throughout [`fs_htc`], [`ff_htc`], and [`chf`]
alongside [`crate::genfoam::thermal_hydraulics::units::ReynoldsNumber`] as
the second independent variable of the Nusselt-form forced-convection
correlations.

```rust
pub type PrandtlNumber = uom::si::f64::Ratio;
```

#### Type Alias `LatentHeat`

Latent heat of vaporization `L = h_g - h_f` — **base SI: J / kg**.

Local alias (see the module doc) of [`uom`]'s
[`AvailableEnergy`](uom::si::f64::AvailableEnergy), the quantity `uom`
uses for mass-specific energy. Used by [`chf::OnsetOfNucleateBoilingTemperature`].

```rust
pub type LatentHeat = uom::si::f64::AvailableEnergy;
```

## Module `interfacial`

# `closures::interfacial` — two-phase geometry & regime closures

Rust port of GeN-Foam's `physicsModels/{interfacialAreaModels,
fluidDiameterModels, virtualMassModels, dispersionModels,
contactPartitionModels, regimeMapModels}`. These are the **geometric and
topological** closures of the two-fluid model — how much interface area
exists, how big is a bubble/droplet/film, how strongly does an accelerating
phase drag its added mass along, how does turbulence spread a dispersed
phase, what fraction of a wall does each phase touch, and which named flow
regime applies at a given local state. Belongs here: two-phase interface
*geometry* and regime *selection*. Does **not** belong here: the drag,
heat-transfer, or phase-change *values* themselves (those are `ff_drag`,
`fs_drag` (sibling module), `heat_transfer`, `phase_change`) — this module
supplies the multipliers and switches those other closures consume.

## Sub-modules (each a closed enum/struct, `match`-dispatched, no `dyn`)

| Module | Port of | Public type(s) |
|---|---|---|
| [`area`] | `interfacialAreaModels/{spherical,annular,NoKazimi,Schor}` | [`area::InterfacialArea`] |
| [`diameter`] | `fluidDiameterModels/{isomolarBubble,isothermalBubble,pipeFilm,WallisFilm}` | [`diameter::BubbleDiameter`], [`diameter::FilmDiameter`] |
| [`virtual_mass`] | `virtualMassModels/virtualMassCoefficientModels` (`constant` only — see module docs) | [`virtual_mass::VirtualMassCoefficient`] |
| [`dispersion`] | `dispersionModels/constant` | [`dispersion::TurbulentDispersion`] |
| [`contact_partition`] | `contactPartitionModels/{complementary,linear}` | [`contact_partition::ContactPartition`] |
| [`regime_map`] | `regimeMapModels/oneParameter` (`twoParameters` deferred — see module docs) | [`regime_map::RegimeMap1D`] |
| [`units`] | (no upstream equivalent) | Local `uom` aliases: `units::InterfacialAreaConcentration`, `units::FluidDiameter` |

Each sub-module's own doc comment carries the full methodology, the
upstream `.H`/`.C` provenance, and (where a simplification from upstream's
OpenFOAM mesh/registry machinery to a pure closure was necessary — e.g.
`contactPartitionModels::complementary`'s registry lookup, or
`regimeMapModels::twoParameters`'s point-in-polygon engine) an explicit note
of what changed and why. See `tests.rs` for the V&V methodology and results
(measured 2026-07-15).

Tracked by bead op-p6p.7.8; see `docs/genfoam-port-plan.md`.

```rust
pub mod interfacial { /* ... */ }
```

### Modules

## Module `area`

# `interfacial::area` — interfacial area concentration models

Port of GeN-Foam's `interfacialAreaModels` family: given the local raw
(unnormalized) phase fractions of a dispersed/continuous fluid-fluid pair,
return the interfacial area concentration `a_i` (interface area per unit
mixture volume, [`InterfacialAreaConcentration`]) — the multiplier every
fluid-fluid drag and heat-transfer closure scales by.

## Model set (closed enum, no `dyn` dispatch)

| Variant | Upstream | Geometry assumed |
|---|---|---|
| [`InterfacialArea::Spherical`] | `spherical` | Dispersed phase is spherical bubbles/droplets of diameter `D` |
| [`InterfacialArea::Annular`] | `annular` | Dispersed phase is a cylindrical (annular) core/film |
| [`InterfacialArea::NoKazimi`] | `NoKazimi` | Vapour in a triangular-pitch wire-wrapped rod bundle |
| [`InterfacialArea::Schor`] | `Schor` | Vapour in a triangular-pitch rod bundle, 3-region blend |

## A modelling note: "raw" vs "pair-normalized" phase fraction

Upstream, `spherical`/`annular` read `pair.alphaDispersed()`/
`pair.alphaContinuous()` (both **raw**, i.e. fractions of the *whole* mesh
cell, which in general may host more than two fluids) and internally form
`a = alphaDispersed / (alphaDispersed + alphaContinuous)` — GeN-Foam calls
this the pair-*normalized* fraction. `NoKazimi`/`Schor` instead read the raw
vapour fraction directly (`vapour_[celli]`) and separately fetch
`vapour_.normalized()[celli]`, GeN-Foam's own name for exactly that same
`a`.

This port unifies the four models on one signature,
[`InterfacialArea::area_concentration`], which takes the pair's two **raw**
fractions (`alpha_dispersed`, `alpha_continuous`) and derives the
pair-normalized fraction internally as `alpha_dispersed / (alpha_dispersed +
alpha_continuous)` for every variant — i.e. it assumes the pair's two fluids
are the only phases present in the cell (no third non-interacting phase). A
call site with a genuine three-phase mixture must pre-normalize
`alpha_continuous` to exclude the third phase before calling. This
assumption is explicit and testable (see `tests.rs`); it is not a
fabrication of new physics, only a simplification of GeN-Foam's general
multi-fluid registry lookup into a pure two-argument function.

`dispersed_diameter` is used only by [`InterfacialArea::Spherical`] /
[`InterfacialArea::Annular`] (upstream reads it per-cell from a
[`super::diameter::BubbleDiameter`]/[`super::diameter::FilmDiameter`]
closure); [`InterfacialArea::NoKazimi`] / [`InterfacialArea::Schor`] carry
their own **fixed** pin diameter as a variant field (rod-bundle geometry is
a case constant, not a per-cell field) and ignore the argument.

## References

- N. Zuber, "On the dispersed two-phase flow in the laminar flow regime,"
  *Chem. Eng. Sci.* 19(11), 1964 — spherical `a_i = 6*alpha/D` limit.
- H.C. No & M.S. Kazimi correlation for rod-bundle interfacial area (as
  implemented upstream; no public citation given in the GeN-Foam source).
- Schor et al. rod-bundle interfacial area correlation with a three-region
  (low/transition/high void-fraction) blend (as implemented upstream).

```rust
pub mod area { /* ... */ }
```

### Types

#### Enum `InterfacialArea`

Interfacial area concentration `a_i` correlation.

Closed enum port of GeN-Foam's `interfacialAreaModel` run-time-selectable
family. Evaluate with [`InterfacialArea::area_concentration`]. See the
module docs for the shared "raw vs. pair-normalized fraction" convention and
why `dispersed_diameter` is ignored by the rod-bundle variants.

```rust
pub enum InterfacialArea {
    Spherical {
        cutoff_alpha: f64,
    },
    Annular {
        cutoff_alpha: f64,
    },
    NoKazimi {
        a_const: f64,
        pin_diameter_m: f64,
    },
    Schor {
        pin_diameter_m: f64,
        a_const: f64,
        cutoff_alpha: f64,
        min_area_at_large_alpha: f64,
        ia1: f64,
        ia2: f64,
    },
}
```

##### Variants

###### `Spherical`

Spherical bubble/droplet geometry: `a_i = 6*alpha/D` below `cutoff_alpha`,
tapering to zero at `alpha = 1` above it (packing limit — spheres cannot
fill space, so upstream forces `a_i -> 0` as the dispersed fraction
approaches unity). The two branches are algebraically continuous at
`alpha = cutoff_alpha` (the `(1-a)/(1-cutoff)` taper factor is exactly
`1` there) — see `tests.rs`'s `spherical_cutoff_branch_is_continuous`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `cutoff_alpha` | `f64` | Void/quality fraction above which the tapering correction applies<br>(upstream default `0.9`). |

###### `Annular`

Cylindrical (annular) core/film geometry: `a_i = 4*alpha/D` below
`cutoff_alpha`, with a `(1 - sqrt(alpha))` taper above it.

**Known upstream discontinuity, faithfully reproduced.** Unlike
[`InterfacialArea::Spherical`]'s `(1-a)/(1-cutoff)` taper (which is
exactly `1` at `a = cutoff_alpha`, so its two branches meet), annular's
`(1-sqrt(a))/(1-cutoff)` taper does **not** reduce to `1` at
`a = cutoff_alpha` — the value jumps by a factor of exactly
`1/(1+sqrt(cutoff_alpha))` (about `0.513`, i.e. an abrupt ~49% drop, at
the upstream default `cutoff_alpha = 0.9`) when crossing from the
dilute branch into the tapered branch. This was verified against the
upstream `.C` source (`annularInterfacialArea.C`) character-for-character
— it is not a transcription error in this port, and is not "fixed"
here per the workspace's guardrail against altering verified upstream
behaviour without human sign-off. See `tests.rs`'s
`annular_cutoff_branch_has_a_documented_jump_discontinuity` for the
exact reproduction and a flag for human follow-up.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `cutoff_alpha` | `f64` | Void/quality fraction above which the tapering correction applies<br>(upstream default `0.9`). |

###### `NoKazimi`

No & Kazimi wire-wrapped triangular-pitch rod-bundle correlation.
Build with [`InterfacialArea::no_kazimi_from_geometry`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a_const` | `f64` | Precomputed geometry constant `A = 4*pi / (D*(2*sqrt(3)*(P/D)^2 - pi))`, 1/m. |
| `pin_diameter_m` | `f64` | Pin (rod) diameter, m. |

###### `Schor`

Schor rod-bundle correlation with a three-region blend over the
pair-normalized vapour fraction: `alpha_n < 0.55` (dilute), `0.55 <=
alpha_n < 0.65` (linear transition), `alpha_n >= 0.65` (dense, tapered
toward `cutoff_alpha`). Build with [`InterfacialArea::schor_from_geometry`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pin_diameter_m` | `f64` | Pin diameter, m. |
| `a_const` | `f64` | Precomputed `A = 2*sqrt(3)*(P/D)^2` geometry constant (dimensionless). |
| `cutoff_alpha` | `f64` | Void fraction above which the dense-region taper applies (upstream<br>default `0.957`). |
| `min_area_at_large_alpha` | `f64` | Minimum allowable `a_i` at large void fraction, 1/m (upstream<br>default `0.0`). |
| `ia1` | `f64` | Precomputed interpolation value at `alpha_n = 0.55`, 1/m. |
| `ia2` | `f64` | Precomputed interpolation value at `alpha_n = 0.65`, 1/m. |

##### Implementations

###### Methods

- ```rust
  pub fn area_concentration(self: &Self, alpha_dispersed: Ratio, alpha_continuous: Ratio, dispersed_diameter: FluidDiameter) -> InterfacialAreaConcentration { /* ... */ }
  ```
  Evaluate the interfacial area concentration for the given pair state.

- ```rust
  pub fn no_kazimi_from_geometry(pin_diameter_m: f64, pin_pitch_m: f64) -> Self { /* ... */ }
  ```
  Build [`InterfacialArea::NoKazimi`] from rod-bundle geometry, reproducing

- ```rust
  pub fn schor_from_geometry(pin_diameter_m: f64, pin_pitch_m: f64, cutoff_alpha: f64, min_area_at_large_alpha: f64) -> Self { /* ... */ }
  ```
  Build [`InterfacialArea::Schor`] from rod-bundle geometry and the two

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
    fn clone(self: &Self) -> InterfacialArea { /* ... */ }
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
    fn eq(self: &Self, other: &InterfacialArea) -> bool { /* ... */ }
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
## Module `contact_partition`

# `interfacial::contact_partition` — fluid-structure wall-contact partition

Port of GeN-Foam's `contactPartitionModel` family: the fraction of a
structure's wetted wall area attributed to a given fluid phase (e.g. what
fraction of a rod's surface is "wetted by liquid" vs. "in contact with
vapour film"), used to split fluid-structure drag/heat-transfer between the
phases of a boiling or film-flow scenario.

## Model set (closed enum, no `dyn` dispatch)

| Variant | Upstream | Contact fraction |
|---|---|---|
| [`ContactPartition::Linear`] | `linear` | Equal to the phase's own (pair-)normalized void fraction |
| [`ContactPartition::Complementary`] | `complementary` | `1 -` the other phase's contact fraction |

## The `complementary` registry lookup, faithfully simplified

Upstream's `complementary::value()` does not compute anything itself — on
first call it searches the mesh's `objectRegistry` for *the other*
`contactPartitionModel` instance registered for the same structure (there
are always exactly two fluids in an `FSPair`, so exactly one is `linear` and
the other `complementary`) and returns `1 - thatModel->value(celli)`. That
registry lookup is OpenFOAM mesh-object-registry plumbing, not algebra, and
is out of scope for a pure closure (the solver bead that owns the fluid
registry is the natural place to wire "find my complementary pair's
partition model"). This port keeps the physics — `1 - other` — and takes the
other model's already-evaluated fraction as an argument
([`ContactPartition::Complementary`]'s `value` takes `other_fraction`
instead of re-deriving it), which is exact for any registry wiring a caller
chooses.

```rust
pub mod contact_partition { /* ... */ }
```

### Types

#### Enum `ContactPartition`

Fluid-structure wall-contact partition fraction — dimensionless, `0..=1`.

Closed enum port of GeN-Foam's `contactPartitionModel` family. Evaluate
with [`ContactPartition::value`]. See the module docs for the
`complementary` registry-lookup simplification.

```rust
pub enum ContactPartition {
    Linear,
    Complementary,
}
```

##### Variants

###### `Linear`

Contact fraction equal to the phase's own (pair-)normalized void
fraction `alphaN` (i.e. the structure's wall is assumed wetted in exact
proportion to how much of the fluid volume this phase occupies).

###### `Complementary`

Contact fraction `1 -` the complementary phase's fraction (see the
module docs for why this takes the other fraction as an input rather
than a mesh-registry lookup).

##### Implementations

###### Methods

- ```rust
  pub fn value(self: &Self, input: Ratio) -> Ratio { /* ... */ }
  ```
  Evaluate the contact partition fraction.

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
    fn clone(self: &Self) -> ContactPartition { /* ... */ }
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
    fn eq(self: &Self, other: &ContactPartition) -> bool { /* ... */ }
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
## Module `diameter`

# `interfacial::diameter` — bubble/droplet and film characteristic diameter

Port of GeN-Foam's `fluidDiameterModels` family, split into **two** closed
enums (rather than one) because the four upstream models fall into two
physically distinct groups that do not share an input signature:

| Enum | Upstream models | Driven by |
|---|---|---|
| [`BubbleDiameter`] | `isomolarBubble`, `isothermalBubble` | Local pressure (and, for `isomolarBubble`, temperature) via the ideal-gas law |
| [`FilmDiameter`] | `pipeFilm`, `WallisFilm` | The phase's own (pair-)normalized void fraction and the structure's hydraulic diameter |

Both return the same physical quantity, the dispersed phase's characteristic
diameter ([`FluidDiameter`]) — the `Dh` field GeN-Foam feeds into
[`super::area::InterfacialArea::area_concentration`]'s `dispersed_diameter`
argument.

## `BubbleDiameter` — ideal-gas bubble/droplet resizing

Both variants assume a fixed reference diameter `d0` at a reference state
`(p0[, T0])` and rescale the diameter as the bubble/droplet expands or
contracts isomolarly (`pV/T = const`, i.e. constant moles, no mass
exchange) or isothermally (`pV = const` at fixed `T`, i.e. constant mass at
constant temperature). Both are cube-root ideal-gas volume scalings —
`isothermalBubble` is exactly `isomolarBubble` with the `T/T0` factor
dropped.

## `FilmDiameter` — annular-flow film thickness

Both variants estimate a film thickness from the phase's own
(pair-)normalized void fraction `alphaN` and the structure's hydraulic
diameter `Dhs` — `pipeFilm` assumes a film conformal to a circular pipe wall
(`d = (1 - sqrt(1-alphaN)) * Dhs`, from the pipe cross-section area balance);
`WallisFilm` assumes a linear film-thickness/void-fraction relationship
calibrated against the Wallis interfacial-drag coefficient (`d = 0.25 *
alphaN * Dhs`; see TRACE theory manual eqns. 4-37/4-38,
<https://www.nrc.gov/docs/ML1200/ML120060218.pdf>, pp. 135-136).

```rust
pub mod diameter { /* ... */ }
```

### Types

#### Enum `BubbleDiameter`

Ideal-gas bubble/droplet diameter closure — rescales a reference diameter
`d0` (at reference state `p0`[, `T0`]) with local pressure (and, for
[`BubbleDiameter::IsomolarBubble`], temperature).

```rust
pub enum BubbleDiameter {
    IsomolarBubble {
        d0_m: f64,
        p0_pa: f64,
        t0_k: f64,
    },
    IsothermalBubble {
        d0_m: f64,
        p0_pa: f64,
    },
}
```

##### Variants

###### `IsomolarBubble`

Constant-moles (isomolar) ideal-gas rescaling:
`d = d0 * cbrt((p0 * T) / (p * T0))`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `d0_m` | `f64` | Reference diameter, m. |
| `p0_pa` | `f64` | Reference pressure, Pa. |
| `t0_k` | `f64` | Reference temperature, K. |

###### `IsothermalBubble`

Constant-mass, constant-temperature (isothermal) ideal-gas rescaling:
`d = d0 * cbrt(p0 / p)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `d0_m` | `f64` | Reference diameter, m. |
| `p0_pa` | `f64` | Reference pressure, Pa. |

##### Implementations

###### Methods

- ```rust
  pub fn diameter(self: &Self, pressure: Pressure, temperature: ThermodynamicTemperature) -> FluidDiameter { /* ... */ }
  ```
  Evaluate the diameter at the given local `(pressure, temperature)`.

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
    fn clone(self: &Self) -> BubbleDiameter { /* ... */ }
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
    fn eq(self: &Self, other: &BubbleDiameter) -> bool { /* ... */ }
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
#### Enum `FilmDiameter`

Annular-flow film-thickness closure — estimates a film thickness from the
phase's own (pair-)normalized void fraction and the structure's hydraulic
diameter.

```rust
pub enum FilmDiameter {
    PipeFilm {
        residual_alpha: f64,
    },
    WallisFilm {
        residual_alpha: f64,
    },
}
```

##### Variants

###### `PipeFilm`

Film conformal to a circular pipe wall:
`d = (1 - sqrt(1 - max(alphaN, residual_alpha))) * Dhs`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `residual_alpha` | `f64` | Minimum allowable normalized void fraction, avoiding a null film<br>thickness at `alphaN = 0` (upstream default `1e-2`). |

###### `WallisFilm`

Wallis interfacial-drag-calibrated linear film thickness:
`d = 0.25 * max(alphaN, residual_alpha) * Dhs`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `residual_alpha` | `f64` | Minimum allowable normalized void fraction (upstream default `1e-2`). |

##### Implementations

###### Methods

- ```rust
  pub fn diameter(self: &Self, alpha_normalized: Ratio, structure_hydraulic_diameter: FluidDiameter) -> FluidDiameter { /* ... */ }
  ```
  Evaluate the film thickness given the phase's (pair-)normalized void

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
    fn clone(self: &Self) -> FilmDiameter { /* ... */ }
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
    fn eq(self: &Self, other: &FilmDiameter) -> bool { /* ... */ }
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
## Module `dispersion`

# `interfacial::dispersion` — turbulent dispersion coefficient

Port of GeN-Foam's `dispersionModel` run-time-selectable family. Only one
concrete model ships upstream (`dispersionModels::constant`), so this is
(currently) a single-variant closed enum, same situation as
[`super::virtual_mass::VirtualMassCoefficient`].

## The fluid1-vs-fluid2 sign resolution

Upstream's constructor resolves *which* fixed value to store at
dictionary-read time, not at evaluation time:

```text
value_ = (pair.fluid1().name() == dispersedPhaseName) ? dict["value"] : 1 - dict["value"]
```

i.e. the dictionary always specifies the coefficient *for the named
dispersed phase*; if the pair's `fluid1` happens to be that phase the raw
value is kept, otherwise its complement is stored (the model always reports
"dispersion of `fluid1`", per `dispersionModel::value()`'s doc comment:
"Return dispersion value of fluid1_ (ALWAYS fluid1)"). This is a
dictionary-parse-time decision (which fluid was named), not per-cell
physics, so it is reproduced as a constructor argument
([`TurbulentDispersion::constant_for_fluid1`]) rather than deferred into the
evaluation call.

```rust
pub mod dispersion { /* ... */ }
```

### Types

#### Enum `TurbulentDispersion`

Turbulent dispersion coefficient for `fluid1` of an `FFPair` —
dimensionless (upstream's `dispersionModel::value()` always reports the
coefficient for `fluid1`; the complementary coefficient for `fluid2` is
`1 - value`, computed by the caller if needed).

Closed enum port of GeN-Foam's `dispersionModel` family. See the module
docs for why [`TurbulentDispersion::Constant`] is currently the only
variant, and for the fluid1/fluid2 resolution convention.

```rust
pub enum TurbulentDispersion {
    Constant {
        value_for_fluid1: f64,
    },
}
```

##### Variants

###### `Constant`

A fixed dispersion coefficient for `fluid1`, resolved once from the
case dictionary (see [`TurbulentDispersion::constant_for_fluid1`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `value_for_fluid1` | `f64` | The coefficient value for `fluid1` (dimensionless). |

##### Implementations

###### Methods

- ```rust
  pub fn constant_for_fluid1(dict_value: Ratio, dispersed_phase_is_fluid1: bool) -> Self { /* ... */ }
  ```
  Build a [`TurbulentDispersion::Constant`] from the dictionary-declared

- ```rust
  pub fn value_for_fluid1(self: &Self) -> Ratio { /* ... */ }
  ```
  The (already-resolved) dispersion coefficient for `fluid1` of the pair.

- ```rust
  pub fn value_for_fluid2(self: &Self) -> Ratio { /* ... */ }
  ```
  The complementary dispersion coefficient for `fluid2` of the pair

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
    fn clone(self: &Self) -> TurbulentDispersion { /* ... */ }
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
    fn eq(self: &Self, other: &TurbulentDispersion) -> bool { /* ... */ }
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
## Module `regime_map`

# `interfacial::regime_map` — one-parameter flow-regime map

Port of GeN-Foam's `regimeMapModels::oneParameter`: given a single scalar
parameter (e.g. void fraction, quality, superficial mass flux), classify it
into one or more named flow regimes with interpolation weights that sum to
`1` — the mechanism GeN-Foam uses to blend between correlations (drag,
interfacial area, ...) that only apply within a specific regime, avoiding a
discontinuous jump at the regime boundary.

## Algorithm (faithful to upstream `oneParameter::correct()`)

1. At construction, each named regime is given a `[lower, upper]` window on
   the parameter axis. Windows are sorted ascending by `lower`. Where one
   regime's `upper` does not exactly meet the next regime's `lower`, an
   **anonymous interpolation gap** is inserted between them — the region
   across which the two neighbouring regimes are blended.
2. At evaluation, the parameter is binned into whichever named window
   contains it (weight `1.0`), or, if it falls in a gap, blended between the
   gap's two neighbouring regimes using [`RegimeInterpolationMode::Linear`]
   (weights proportional to distance from each boundary) or
   [`RegimeInterpolationMode::Quadratic`] (a smootherstep-like blend, zero
   slope at both gap endpoints).

This port keeps upstream's exact half-open window semantics: a named window
`[t0, t1)` claims a parameter value `p` when `t1 - p > 0` and `p - t0 >= 0`
(so `p == t0` belongs to the window, `p == t1` does not); an interpolation
gap `(t0, t1]` claims `p` when `t1 - p >= 0` and `p - t0 > 0` (the opposite
half-open sense).

**Known sharp edge, faithfully reproduced, not "fixed":** because the two
half-open senses are opposite, they do not compose cleanly at every
junction. At the point where a regime *ends and a gap begins* (`p ==` that
regime's `upper`), neither the regime nor the gap claims `p` — the returned
weights sum to `0.0` there (a dropped, measure-zero point). At the point
where a gap *ends and the next regime begins* (`p ==` that regime's
`lower`), **both** the gap (with weight `0.0` for the entering regime, so
effectively a no-op contribution) **and** the regime itself claim `p`, so
that regime's total weight is `2.0` at that single point, not `1.0` — see
`tests.rs`'s `gap_junction_double_counts_the_entering_regime` regression
test. Both are genuine properties of the upstream algorithm (reproduced
bit-for-bit here, not introduced by this port); in practice they only
affect a set of parameter values of measure zero and are invisible to any
caller that does not probe an exact threshold value, but they are
documented rather than silently "cleaned up" (see the workspace's
guardrail against altering verified behaviour without human sign-off).

**The two outermost regimes are unbounded, regardless of their declared
window.** Only the *inner*-facing edge of the lowest and highest regime is
ever compared against a neighbour (to decide whether a gap is needed); the
outer-facing edge is always replaced by the `+-`[`OUTER_BOUND_SENTINEL`]
sentinel. So a regime declared `[0.0, 0.3]` that happens to be the lowest
window in the map in fact claims every `parameter <= 0.3` down to
`-`[`OUTER_BOUND_SENTINEL`] (not just down to `0.0`), and likewise the
highest window extends up to `+`[`OUTER_BOUND_SENTINEL`] regardless of its
declared upper edge. This is deliberate upstream (every real parameter
value must classify into *some* regime, so the two extremal regimes
extrapolate outward rather than leaving the map's tails unclassified), not
an artefact of this port — see `tests.rs`'s
`outermost_regimes_extend_to_the_sentinel_not_their_declared_edge` test.

The `+-1e69` numeric sentinels upstream uses for the outermost open bounds
are reproduced exactly (as [`OUTER_BOUND_SENTINEL`]) rather than replaced
with `f64::INFINITY`. That substitution was tried and rejected during
development: the per-window evaluation divides by the window's width `dt`,
and the outermost window's width is `t1 - t0` with one side infinite —
`(INFINITY - p) / INFINITY` is the IEEE-754 indeterminate form `inf/inf =
NaN`, not `1.0`. A large *finite* sentinel keeps `dt` finite (about `1e69`)
so the ratio evaluates normally; this is exactly why upstream picked a
numeric sentinel over an literal unbounded value in the first place, and is
preserved here rather than "improved" (see `tests.rs`'s
`outermost_window_sentinel_does_not_yield_nan` regression test).

## Deferred: `regimeMapModels::twoParameters`

GeN-Foam's `twoParameters` regime map (`regimeMapModels/twoParameters/{twoParameters,
regimeBoundary2D,regimeDomain2D}.{H,C}`, ~1150 lines combined) classifies a
`(parameter1, parameter2)` point against a set of named polygonal regions in
parameter space (`regimeDomain2D`: point-in-polygon tests for both convex and
concave polygons, shared-boundary detection for cross-region interpolation
bands, bounding-box acceleration). This is a general-purpose 2D
computational-geometry engine, not a closed-form algebraic closure like the
rest of this module — porting it faithfully is a substantially larger,
self-contained task (candidate for a dedicated follow-up bead, and a natural
fit to share with any future mesh/geometry utilities elsewhere in the
workspace rather than duplicate). **Deferred**, not attempted here, to keep
this bead's output reviewable; [`RegimeMap1D`] (the `oneParameter` port) is
unaffected and stands alone.

## Also deferred: the `templatedModels`/`byRegime` dispatch layer

`regimeMapModelTemplates.C`'s `constructModels`/`interpolateValue` and
`templatedModels/byRegime/*` are the C++ machinery that, given a regime map
and one sub-model dictionary per named regime, builds a list of
run-time-selected sub-models and blends their values with the regime
weights. That is generic multi-model dispatch across *any* closure family
(drag, heat transfer, interfacial area, ...), which is exactly the kind of
indirection the workspace's no-`dyn` rule steers away from; the solver bead
that owns per-pair model selection is better placed to wire "evaluate this
closure enum's variant selected by [`RegimeMap1D::regime_weights`]" directly
against the concrete closure enums in this module, rather than have this
module reach into every other closure family's types. **Deferred.**

```rust
pub mod regime_map { /* ... */ }
```

### Types

#### Enum `RegimeInterpolationMode`

Interpolation shape used to blend two neighbouring regimes across a gap.

Closed enum port of GeN-Foam's `oneParameter::interpolationMode`.

```rust
pub enum RegimeInterpolationMode {
    Linear,
    Quadratic,
}
```

##### Variants

###### `Linear`

Weights vary linearly with distance from each boundary of the gap.

###### `Quadratic`

A smootherstep-like blend: zero slope at both gap endpoints, matching
upstream's `interpolationMode::quadratic` branch exactly (see
[`RegimeMap1D::regime_weights`]'s implementation for the precise
piecewise form — it is not a plain quadratic `t^2` ease, and is
reproduced bit-for-bit rather than replaced with a "cleaner" curve).

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
    fn clone(self: &Self) -> RegimeInterpolationMode { /* ... */ }
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
    fn eq(self: &Self, other: &RegimeInterpolationMode) -> bool { /* ... */ }
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
#### Struct `RegimeBound`

One named regime's window `[lower, upper]` on the parameter axis, as read
from GeN-Foam's `regimeBounds { "name" (lower upper); ... }` dictionary
entry. `lower`/`upper` may be given in either order — [`RegimeMap1D::new`]
normalizes them, matching upstream's own swap-if-reversed step.

```rust
pub struct RegimeBound {
    pub name: String,
    pub lower: f64,
    pub upper: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The regime's name (must be unique within the map). |
| `lower` | `f64` | One edge of the window. |
| `upper` | `f64` | The other edge of the window. |

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
    fn clone(self: &Self) -> RegimeBound { /* ... */ }
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
    fn eq(self: &Self, other: &RegimeBound) -> bool { /* ... */ }
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
#### Struct `RegimeMap1D`

A one-parameter flow-regime map: classifies a scalar parameter into named
regimes with interpolation weights summing to `1`.

Closed-form (no `dyn`, no mesh/field access) port of GeN-Foam's
`regimeMapModels::oneParameter`. Build with [`RegimeMap1D::new`]; evaluate
per-cell with [`RegimeMap1D::regime_weights`]. See the module docs for the
algorithm and the exact half-open boundary convention.

```rust
pub struct RegimeMap1D {
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
  pub fn new(regimes: Vec<RegimeBound>, interpolation_mode: RegimeInterpolationMode) -> Self { /* ... */ }
  ```
  Build a regime map from its (unsorted, possibly edge-reversed) named

- ```rust
  pub fn regime_weights(self: &Self, parameter: f64) -> Vec<(String, f64)> { /* ... */ }
  ```
  Classify `parameter` into its regime(s), returning `(regime_name,

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
    fn clone(self: &Self) -> RegimeMap1D { /* ... */ }
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
    fn eq(self: &Self, other: &RegimeMap1D) -> bool { /* ... */ }
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
## Module `units`

# Named `uom` aliases local to `closures::interfacial`

[`crate::genfoam::thermal_hydraulics::units`] (the shared
`thermal_hydraulics::units` module) only defines the aliases the
already-ported [`crate::genfoam::thermal_hydraulics::closures::fs_drag`]
family needs (`ReynoldsNumber`, `DarcyFrictionFactor`,
`HeatTransferCoefficient`, `HeatFlux`). This module adds the two names the
two-phase geometry/regime closures need that are not there yet:

- [`InterfacialAreaConcentration`] — interfacial area per unit mixture volume
  `a_i` (1/m). The one genuinely new *dimension* this closure family
  introduces (`uom`'s [`ReciprocalLength`](uom::si::f64::ReciprocalLength)).
- [`FluidDiameter`] — the bubble/droplet/film characteristic diameter (m). An
  alias of `uom`'s plain [`Length`](uom::si::f64::Length); named separately so
  a call site cannot confuse it with, say, a hydraulic or pin diameter that
  happens to also be a `Length`.

`virtualMassModels`/`dispersionModels`/`contactPartitionModels` all reduce to
plain dimensionless fractions or coefficients, so they reuse `uom`'s
[`Ratio`](uom::si::f64::Ratio) directly (same convention as `ReynoldsNumber`
in the shared module) — no new alias needed for those.

**Candidate for promotion to the shared `units.rs`:** [`InterfacialAreaConcentration`]
is likely to recur once the fluid-fluid drag/heat-transfer closures (bead
op-p6p.7.9/.7.10) are ported, since `a_i` is their shared multiplier. Left
local here per the op-p6p.7.8 task scope (touching the shared file is out of
this bead's lane); flagged in the bead's hand-off report for whoever ports
`ff_drag`/`heat_transfer` next to hoist it if it turns out to be shared.

```rust
pub mod units { /* ... */ }
```

### Types

#### Type Alias `InterfacialAreaConcentration`

Interfacial area concentration `a_i` — **base SI: 1/m** (area of the
fluid-fluid interface per unit volume of the two-fluid mixture).

Returned by every [`super::area::InterfacialArea`] variant. Aliases `uom`'s
[`ReciprocalLength`](uom::si::f64::ReciprocalLength) (dimension `L^-1`, which
is exactly `a_i`'s dimension: interface area `[L^2]` per mixture volume `[L^3]`).

```rust
pub type InterfacialAreaConcentration = uom::si::f64::ReciprocalLength;
```

#### Type Alias `FluidDiameter`

A characteristic fluid diameter — bubble, droplet, or film thickness —
**base SI: m**.

Returned by every [`super::diameter::BubbleDiameter`] and
[`super::diameter::FilmDiameter`] variant, and consumed as the
`dispersed_diameter` argument of [`super::area::InterfacialArea::spherical`]/
[`super::area::InterfacialArea::annular`]. Aliases `uom`'s
[`Length`](uom::si::f64::Length).

```rust
pub type FluidDiameter = uom::si::f64::Length;
```

### Functions

#### Function `interfacial_area_concentration`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Build an [`InterfacialAreaConcentration`] from a bare (1/m) magnitude.

```rust
pub fn interfacial_area_concentration(value_per_metre: f64) -> InterfacialAreaConcentration { /* ... */ }
```

#### Function `fluid_diameter`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Build a [`FluidDiameter`] from a bare metre magnitude.

```rust
pub fn fluid_diameter(value_m: f64) -> FluidDiameter { /* ... */ }
```

## Module `virtual_mass`

# `interfacial::virtual_mass` — virtual (added) mass coefficient

Port of GeN-Foam's `virtualMassCoefficientModel` run-time-selectable family.

**Why this is a one-variant enum.** `virtualMassCoefficientModel.H`/`.C`
declare only the abstract base and its `runTimeSelectionTable` — no concrete
subclass ships in `virtualMassModels/`. The only instantiation that actually
exists in the upstream tree is the generic `constantModel<scalar,
virtualMassCoefficientModel>` template, registered under the name
`"constant"` in `templatedModels/constant/constantModels.C` and used by both
shipped tutorials (`Tutorials/featureCases/{1D_boiling,2D_KNS37-L22}/constant/
fluidRegion/phaseProperties`, e.g. `virtualMassCoefficientModel { type
constant; value 0.1; }`). `constantModel::value()` is trivial — it returns
the dictionary-read constant unchanged for every cell — so this port is that
one real, dictionary-driven model, not a fabricated placeholder. A future
non-constant `Cvm` correlation (e.g. Zuber & Findlay's slip-flow form) would
be a new closed-enum variant, not a change to this one.

`virtualMass::correct()` itself (upstream `virtualMassModels/virtualMass.{H,C}`)
assembles the two-fluid added-mass **force** as an `fvVectorMatrix` from
`Vm`, the phase velocities, and their fluxes — that is porous-momentum-solver
wiring (mesh fields, `fvm::ddt`/`fvm::div`), not a pure closure, and belongs
to the solver bead (op-p6p.7.11) once it exists. Only the scalar `Cvm(alpha)`
coefficient closure is ported here.

```rust
pub mod virtual_mass { /* ... */ }
```

### Types

#### Enum `VirtualMassCoefficient`

Virtual (added) mass coefficient `Cvm` — dimensionless.

Closed enum port of GeN-Foam's `virtualMassCoefficientModel` family.
Evaluate with [`VirtualMassCoefficient::coefficient`]. See the module docs
for why [`VirtualMassCoefficient::Constant`] is currently the only variant.

```rust
pub enum VirtualMassCoefficient {
    Constant {
        cvm: f64,
    },
}
```

##### Variants

###### `Constant`

A fixed coefficient read once from the case dictionary (upstream
`virtualMassCoefficientModel { type constant; value <Cvm>; }`; a
theoretical sphere in inviscid potential flow has `Cvm = 0.5`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `cvm` | `f64` | The fixed coefficient value (dimensionless). |

##### Implementations

###### Methods

- ```rust
  pub fn constant(cvm: Ratio) -> Self { /* ... */ }
  ```
  Build a [`VirtualMassCoefficient::Constant`] from a dimensionless `Cvm`.

- ```rust
  pub fn coefficient(self: &Self) -> Ratio { /* ... */ }
  ```
  Evaluate the coefficient. Takes no per-cell field arguments: every

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
    fn clone(self: &Self) -> VirtualMassCoefficient { /* ... */ }
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
    fn eq(self: &Self, other: &VirtualMassCoefficient) -> bool { /* ... */ }
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
## Module `phase_change`

# `closures::phase_change` — evaporation/condensation source models

Rust port of GeN-Foam's `physicsModels/phaseChangeModels/**`: the saturation
models (constant temperature, water, waterTRACE, Browning-Potter),
latent-heat models (Fink-Leibowitz, water, from-thermophysical-properties),
and the phase-change mass-transfer-rate models (heat-driven, forced
constant). Together these supply the interfacial mass source term `dmdt`
(kg / (m^3 s)) that the two-phase solver's continuity and energy equations
consume. Belongs here: the phase-change rate and its saturation/latent-heat
inputs, as pure algebra in temperature/pressure/enthalpy. Does **not**
belong here: the interfacial heat-transfer coefficients and area density
that produce the heat fluxes consumed by [`rate::PhaseChangeRateModel`]
(that is `super::heat_transfer`, out of scope for this module — see
"Deferred" below), and the enthalpy-consistency (`adjust`) correction that
reads the two phases' live thermodynamic state (out of scope: no sibling
thermo dependency per this port's mandate).

## Model set (three closed enums, no `dyn` dispatch)

| Family | Type | Variants | Upstream |
|---|---|---|---|
| Saturation | [`SaturationModel`] | `ConstantTemperature`, `Water`, `WaterTrace`, `BrowningPotter` | `saturationModels/{constantTemperature,water,waterTRACE,BrowningPotter}` |
| Latent heat | [`LatentHeatModel`] | `FinkLeibowitz`, `Water`, `FromThermophysicalProperties` | `latentHeatModels/{FinkLeibowitz,water,fromThermophysicalProperties}` |
| Phase-change rate | [`PhaseChangeRateModel`] | `HeatDrivenConductionLimited`, `HeatDrivenTwoPhaseDriven`, `HeatDrivenOnePhaseDriven`, `HeatDrivenMixedDriven`, `ForcedConstant` | `heatDriven/`, `forcedConstant/` |

Each family lives in its own file: [`saturation`], [`latent_heat`], [`rate`].
[`tests`] holds the cross-family verification & validation suite.

## Local `uom` aliases

GeN-Foam's phase-change classes pass bare `Foam::scalar`s between the
saturation model, the latent-heat model, and the mass-transfer-rate model.
This module gives the recurring quantities named, dimension-checked `uom`
types. `uom` 0.38 has no built-in named quantity for the phase-change rate
or the saturation-pressure/temperature slope's dimensions, so both are
*composed* from existing `uom` quantities via `core::ops::Div`'s associated
`Output` type — `uom`'s `si` system implements cross-dimension arithmetic
generically (`Quantity<Dl,..> / Quantity<Dr,..> -> Quantity<Dl-Dr,..>`), so
this is fully dimension-checked, just without a named unit table:

- [`LatentHeat`] — alias for [`uom::si::f64::AvailableEnergy`] (J/kg).
- [`InterfacialHeatFlux`] — alias for
  [`uom::si::f64::VolumetricPowerDensity`] (W/m^3), the volumetric
  interfacial heat flux `iA * htc * (T - T_interface)` that drives
  [`rate::PhaseChangeRateModel::HeatDrivenConductionLimited`] and friends.
- [`PhaseChangeRate`] — composed as `InterfacialHeatFlux / LatentHeat`;
  dimension L^-3 M T^-1, i.e. **kg / (m^3 s)**, GeN-Foam's `dmdt`.
- [`SaturationPressureSlope`] — composed as `Pressure / ThermodynamicTemperature`;
  **Pa / K**, GeN-Foam's `dPsat/dT` (`valuePSatPrime`).

These four aliases arguably belong in the shared `units.rs` module once a
second closure family needs them (see the port report for op-p6p.7.7).

```rust
pub mod phase_change { /* ... */ }
```

### Modules

## Module `latent_heat`

Specific latent heat of vaporization `h_fg(T)`.

Port of GeN-Foam's `latentHeatModel` run-time-selectable family. Every
[`LatentHeatModel`] correlation returns a **magnitude** (always physically
positive): GeN-Foam's `LSign_` (+1 if fluid1 is the liquid, -1 otherwise)
is a property of the fluid-fluid pair being modelled, not of the
correlation itself, so applying it is left to the caller — the same
separation [`super::rate`] uses for which phase is "driving".

**Deferred:** upstream's `latentHeatModel::adjust()` (an optional
enthalpy-consistency correction, enabled via the `adjust` dictionary key,
that nudges `L` by the difference between a phase's *current* enthalpy and
its *saturation* enthalpy so evaporation/condensation stays energy
conservative under the solver's explicit mass-transfer discretisation) is
**not** ported here. It needs live per-cell enthalpy fields from both
phases' thermodynamic packages and the sign of the local `dmdt` field —
solver/thermo-package state this module's mandate excludes (see the
`phase_change` module docs). Apply it, if needed, at the call site once
that state is available.

## Reference

- `FinkLeibowitz`: N.P. Fink, L. Leibowitz, "Thermodynamic and Transport
  Properties of Sodium Liquid and Vapor", ANL/RE-95/2, 1995,
  <https://www.ne.anl.gov/eda/ANL-RE-95-2.pdf>.
- `water`: fit (`L = A + B*log(C-T)`) to NIST water saturation data,
  <https://www.nist.gov/system/files/documents/srd/NISTIR5078-Tab1.pdf>.

```rust
pub mod latent_heat { /* ... */ }
```

### Types

#### Enum `LatentHeatModel`

Specific latent heat of vaporization correlation.

Closed enum port of GeN-Foam's `latentHeatModel` family. Evaluate with
[`LatentHeatModel::latent_heat`], which takes both the local temperature
and the two phases' specific enthalpies even though a given variant may
only use one input set — mirrors upstream, where every model has access
to the interfacial temperature and (via the phase-change model) both
fluids' thermodynamic state regardless of which it reads.

```rust
pub enum LatentHeatModel {
    FinkLeibowitz,
    Water,
    FromThermophysicalProperties,
}
```

##### Variants

###### `FinkLeibowitz`

Fink & Leibowitz (1995) correlation for liquid sodium, valid
(extrapolated with clamping, matching upstream) for `T` in `[371, 2500] K`.

###### `Water`

Fit to NIST water saturation data, `L = A + B*log(C-T)`, valid
(clamped, matching upstream) for `T` in `[273.16, 680] K`. Performs
reasonably to about 638 K even though water's true critical
temperature is 647.096 K (`C = 681.718 K` is the fit's own asymptote,
not the physical critical point — see upstream source comment).

###### `FromThermophysicalProperties`

`L = h_vapour - h_liquid`: the difference of the two phases' specific
enthalpies (upstream: their enthalpies of formation, `Hf`, from each
phase's `thermophysicalProperties`). The caller supplies both — this
module has no dependency on a thermo package.

##### Implementations

###### Methods

- ```rust
  pub fn latent_heat(self: &Self, t: ThermodynamicTemperature, h_vapour: LatentHeat, h_liquid: LatentHeat) -> LatentHeat { /* ... */ }
  ```
  Evaluate the specific latent heat of vaporization.

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
    fn clone(self: &Self) -> LatentHeatModel { /* ... */ }
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
    fn eq(self: &Self, other: &LatentHeatModel) -> bool { /* ... */ }
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
## Module `rate`

Phase-change mass-transfer-rate models: `dmdt` as a function of
interfacial heat fluxes (or a prescribed constant).

Port of GeN-Foam's `phaseChangeModel::correctInterfacialDmdt()` family
(`heatDrivenPhaseChange`, `forcedConstantPhaseChange`). Upstream computes
the two per-phase interfacial heat fluxes as
`q1 = i_A * htc1 * (T1 - T_interface)` and
`q2 = i_A * htc2 * (T2 - T_interface)`, then evaluates one of several mode
formulas on `q1/L` and `q2/L`. This port takes `q1`/`q2` pre-multiplied
(see [`super::InterfacialHeatFlux`]) so it stays independent of the
interfacial-heat-transfer-coefficient and interfacial-area-density
closures (`super::heat_transfer`) — the caller assembles `q1`/`q2` and
supplies the [`super::LatentHeat`] from [`super::LatentHeatModel`].

**Deferred** (solver-level bookkeeping, out of scope for a pure closure —
see the `phase_change` module docs): `phaseChangeModel::correct()`'s
wall-boiling split (`dmdtW_`), the adaptive mass-transfer limiter
(`limitMassTransfer`), interfacial-area flooring (`limitInterfacialArea`),
interfacial-temperature relaxation (`correctInterfacialTemperature`), and
the energy-conservative `heSources_` bookkeeping that feeds the two
phases' enthalpy equations. `forcedConstantPhaseChange`'s upstream
`cellZones`-based region masking (`dmdt_[celli] = value` only inside named
mesh zones) is mesh/solver state and is likewise out of scope — this port
exposes the constant rate itself; zone masking is a call-site concern.

```rust
pub mod rate { /* ... */ }
```

### Types

#### Enum `PhaseChangeRateModel`

Phase-change (evaporation/condensation) volumetric mass-transfer-rate model.

Closed enum port of GeN-Foam's `heatDrivenPhaseChange::mode` (four
variants, `heatDriven/heatDrivenPhaseChange.H`) plus
`forcedConstantPhaseChange` (`forcedConstant/forcedConstantPhaseChange.H`).
Evaluate with [`PhaseChangeRateModel::mass_transfer_rate`].

```rust
pub enum PhaseChangeRateModel {
    HeatDrivenConductionLimited,
    HeatDrivenTwoPhaseDriven,
    HeatDrivenOnePhaseDriven {
        driving_is_phase1: bool,
    },
    HeatDrivenMixedDriven {
        driving_is_phase1: bool,
    },
    ForcedConstant {
        rate: super::PhaseChangeRate,
    },
}
```

##### Variants

###### `HeatDrivenConductionLimited`

`dmdt = (q1 + q2) / L`: mass transfer conserves total interfacial
energy transfer even if the two phases' heat fluxes to the interface
don't individually balance (the TRACE approach). Can be unstable
when the phases' volumetric heat capacities differ by orders of
magnitude (upstream notes sodium's ~2000x liquid/vapour density ratio
as the failure case) — upstream `mode::conductionLimited`.

###### `HeatDrivenTwoPhaseDriven`

`dmdt = max(q1, 0)/L + min(q2, 0)/L`: evaporation is driven only by
phase-1 superheat (`q1 > 0`), condensation only by phase-2 undercooling
(`q2 < 0`) — no need to know which phase is liquid vs. vapour, that
information lives in the sign of `L` at the call site (see
[`super::LatentHeatModel`] docs). Upstream `mode::twoPhaseDriven`.

###### `HeatDrivenOnePhaseDriven`

`dmdt = q1/L` if `driving_is_phase1`, else `q2/L`: both evaporation and
condensation are driven uniquely by one phase's heat flux. Upstream
`mode::onePhaseDriven` (`drivingPhase` dictionary key resolved here to
a bool since this port has no phase-name registry).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `driving_is_phase1` | `bool` | `true` selects phase 1's heat flux (`q1`) as the sole driver;<br>`false` selects phase 2's (`q2`). |

###### `HeatDrivenMixedDriven`

Counterpart to [`PhaseChangeRateModel::HeatDrivenTwoPhaseDriven`]: one
phase drives both evaporation and condensation, the other only
contributes when it reinforces (i.e. only its condensing/undercooling
branch, via `neg_part`, is added). `dmdt = q1/L + min(q2,0)/L` if
`driving_is_phase1`, else `dmdt = max(q1,0)/L + q2/L`. Upstream
`mode::mixedDriven`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `driving_is_phase1` | `bool` | Same convention as [`PhaseChangeRateModel::HeatDrivenOnePhaseDriven`]. |

###### `ForcedConstant`

A prescribed constant rate, independent of the interfacial heat
fluxes and latent heat (both ignored). Upstream
`forcedConstantPhaseChange` (dictionary key `value`); the upstream
`regions` cellZone mask that restricts where this rate is applied is
solver/mesh state, out of scope here (see module docs).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `rate` | `super::PhaseChangeRate` | The prescribed volumetric mass-transfer rate. |

##### Implementations

###### Methods

- ```rust
  pub fn mass_transfer_rate(self: &Self, q1: InterfacialHeatFlux, q2: InterfacialHeatFlux, l: LatentHeat) -> PhaseChangeRate { /* ... */ }
  ```
  Evaluate the volumetric phase-change mass-transfer rate `dmdt`.

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
    fn clone(self: &Self) -> PhaseChangeRateModel { /* ... */ }
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
    fn eq(self: &Self, other: &PhaseChangeRateModel) -> bool { /* ... */ }
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
## Module `saturation`

Saturation-curve correlations: `T_sat(p)`, `p_sat(T)`, `ln(p_sat(T))`, and
`dp_sat/dT`.

Port of GeN-Foam's `saturationModel` run-time-selectable family. Every
[`SaturationModel`] method takes both the local temperature *and* pressure
even when a given correlation only uses one of them — this mirrors
upstream, where every model has both `iT_`/`p_` fields available regardless
of which it actually reads (see each variant's docs for which argument is
live).

## References

- `water`: NIST water saturation data, 0.01-350 degC,
  <https://www.nist.gov/system/files/documents/srd/NISTIR5078-Tab1.pdf>.
- `waterTRACE`: TRACE V5.0 Theory Manual,
  <https://www.nrc.gov/docs/ML1200/ML120060218.pdf>.
- `BrowningPotter`: Browning & Potter correlation, ANL-RE-95/2,
  <https://www.ne.anl.gov/eda/ANL-RE-95-2.pdf>.

```rust
pub mod saturation { /* ... */ }
```

### Types

#### Enum `SaturationModel`

Saturation temperature/pressure correlation.

Closed enum port of GeN-Foam's `saturationModel` family. Evaluate with
[`SaturationModel::t_sat`] (`T_sat(p)`), [`SaturationModel::p_sat`]
(`p_sat(T)`), [`SaturationModel::ln_p_sat`] (`ln(p_sat(T))`), and
[`SaturationModel::p_sat_prime`] (`dp_sat/dT`).

```rust
pub enum SaturationModel {
    ConstantTemperature {
        t_sat: uom::si::f64::ThermodynamicTemperature,
    },
    Water,
    WaterTrace,
    BrowningPotter,
}
```

##### Variants

###### `ConstantTemperature`

Fixed saturation temperature, independent of local pressure.

Faithful quirk: upstream's `valuePSat` does **not** compute a pressure
from `TSat_` — it returns the local system pressure field verbatim
(`return p_[celli];`), i.e. this model treats "the current cell
pressure" as automatically saturated. `p_sat`/`ln_p_sat` below
reproduce that pass-through; `p_sat_prime` is always zero.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_sat` | `uom::si::f64::ThermodynamicTemperature` | The fixed saturation temperature (upstream `TSat_`, dictionary key `value`). |

###### `Water`

Water, 0.01-350 degC, `p_sat = A * (T - B)^C` fit to NIST tabulated
data (A, B, C hard-coded from the upstream fit).

**Known fit accuracy pitfall** (measured, not a port bug — see
`tests.rs`): the fit is a good round-trip inverse of itself, but as an
absolute correlation it deviates further from the true saturation
curve than its "NIST fit" description suggests — `T_sat(101325 Pa)`
measures 378.59 K (+1.46% vs. the true 373.15 K normal boiling point),
and `p_sat(373.15 K)` measures 80908 Pa (-20% vs. 101325 Pa). This is
the upstream correlation's own coefficient set, ported verbatim.

###### `WaterTrace`

Water, TRACE-theory-manual piecewise fit (4 branches in `T` for
`p_sat`, 4 branches in `p` for `T_sat`, boundaries at approximately
370.4 K, 609.6 K, 647.3 K and 90.56 kPa, 13.97 MPa, 22.12 MPa
respectively).

**Known upstream discrepancy** (faithfully ported, not fixed — see
`tests.rs`): the branch-2 (370.4251 K - 609.62463 K) `p_sat(T)` formula
`ps = BB * ((T-DB)/AB)^(1/CB)` and the branch-2 `T_sat(p)` formula
`Ts = AB * (BB*p)^CB + DB` are **not** algebraic inverses of each other
(the former multiplies by `BB = 1e-5` where consistency with the
latter would require dividing by it) — `p_sat(373.15 K)` in this branch
evaluates to about `1.0e-5 Pa`, nowhere near the true ~101325 Pa,
while `T_sat(101325 Pa)` in the same branch correctly recovers
`373.35 K`. This crate ports the upstream formula as written in both
directions; do not "fix" `p_sat`'s branch 2 to match `T_sat`'s inverse
without upstream confirmation this is in fact a transcription bug.

###### `BrowningPotter`

Browning & Potter correlation (salt/other-fluid fit), ANL-RE-95/2.
`ln(p_sat) = A + B/T + C*ln(T)`, not analytically invertible in `T`, so
`T_sat(p)` is upstream's documented second-order-polynomial
approximation of the inverse (round-trips to within ~0.04% — see
`tests.rs`).

##### Implementations

###### Methods

- ```rust
  pub fn t_sat(self: &Self, p: Pressure) -> ThermodynamicTemperature { /* ... */ }
  ```
  Saturation temperature `T_sat(p)`.

- ```rust
  pub fn p_sat(self: &Self, t: ThermodynamicTemperature, p: Pressure) -> Pressure { /* ... */ }
  ```
  Saturation pressure `p_sat(T)`.

- ```rust
  pub fn ln_p_sat(self: &Self, t: ThermodynamicTemperature, p: Pressure) -> f64 { /* ... */ }
  ```
  Natural log of the saturation pressure, `ln(p_sat(T) / 1 Pa)`.

- ```rust
  pub fn p_sat_prime(self: &Self, t: ThermodynamicTemperature, p: Pressure) -> SaturationPressureSlope { /* ... */ }
  ```
  Saturation-curve slope `dp_sat/dT`.

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
    fn clone(self: &Self) -> SaturationModel { /* ... */ }
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
    fn eq(self: &Self, other: &SaturationModel) -> bool { /* ... */ }
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

#### Type Alias `LatentHeat`

Specific latent heat of vaporization `h_fg` — **base SI: J / kg**.

The energy absorbed per unit mass converting liquid to vapour at
saturation. Always physically positive; the sign convention for which
phase is gaining/losing mass (GeN-Foam's `LSign_`) is a property of the
fluid-fluid pair, not of the correlation, and is applied by the caller
(see the [`latent_heat`] module doc). Aliased to
[`uom`]'s [`AvailableEnergy`](uom::si::f64::AvailableEnergy).

```rust
pub type LatentHeat = uom::si::f64::AvailableEnergy;
```

#### Type Alias `InterfacialHeatFlux`

Volumetric interfacial heat flux `q = i_A * htc * (T - T_interface)` —
**base SI: W / m^3**.

The rate of heat conducted from a phase's bulk to the fluid-fluid
interface per unit mixture volume, i.e. interfacial area density
(`1/m`) times heat-transfer coefficient (`W/(m^2 K)`) times the
bulk-to-interface temperature difference. [`rate::PhaseChangeRateModel`]'s
heat-driven variants take this pre-multiplied quantity as input rather
than the interfacial HTC and area separately, so this module stays
independent of `super::heat_transfer`. Aliased to [`uom`]'s
[`VolumetricPowerDensity`](uom::si::f64::VolumetricPowerDensity).

```rust
pub type InterfacialHeatFlux = uom::si::f64::VolumetricPowerDensity;
```

#### Type Alias `PhaseChangeRate`

Volumetric phase-change (evaporation/condensation) mass-transfer rate —
**base SI: kg / (m^3 s)**.

GeN-Foam's `dmdt`: positive means mass transferring from fluid1 to fluid2
per the pair's `LSign_` convention. Composed as
[`InterfacialHeatFlux`] / [`LatentHeat`] (dimension L^-3 M T^-1); build one
directly from a bare `kg/(m^3 s)` magnitude with [`phase_change_rate`], and
read one back with [`phase_change_rate_value`].

```rust
pub type PhaseChangeRate = <InterfacialHeatFlux as Div<LatentHeat>>::Output;
```

#### Type Alias `SaturationPressureSlope`

Saturation-curve slope `dP_sat/dT` — **base SI: Pa / K**.

GeN-Foam's `valuePSatPrime`. Composed as `Pressure / ThermodynamicTemperature`
(dimension M L^-1 T^-2 Theta^-1); see [`saturation::SaturationModel::p_sat_prime`].

```rust
pub type SaturationPressureSlope = <uom::si::f64::Pressure as Div<uom::si::f64::ThermodynamicTemperature>>::Output;
```

### Functions

#### Function `phase_change_rate`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Build a [`PhaseChangeRate`] from a bare magnitude in kg / (m^3 s).

`uom` has no named unit table for this composed dimension, so the value is
carried through as `InterfacialHeatFlux(x W/m^3) / LatentHeat(1 J/kg)`:
dividing by the base-unit magnitude `1.0` leaves `x` unchanged (both
`watt_per_cubic_meter` and `joule_per_kilogram` are `uom`'s coefficient-1
base units for their quantities), so the resulting `.value` is exactly `x`
in kg / (m^3 s).

```rust
pub fn phase_change_rate(value_kg_per_m3_per_s: f64) -> PhaseChangeRate { /* ... */ }
```

#### Function `phase_change_rate_value`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Read a [`PhaseChangeRate`] back as a bare `f64` in kg / (m^3 s).

Valid because `PhaseChangeRate`'s composed dimension has no `uom` unit
table, so its public `value` field already holds the SI base-unit
magnitude (kg / (m^3 s) has coefficient 1 against `uom`'s SI base).

```rust
pub fn phase_change_rate_value(rate: PhaseChangeRate) -> f64 { /* ... */ }
```

### Re-exports

#### Re-export `LatentHeatModel`

```rust
pub use latent_heat::LatentHeatModel;
```

#### Re-export `PhaseChangeRateModel`

```rust
pub use rate::PhaseChangeRateModel;
```

#### Re-export `SaturationModel`

```rust
pub use saturation::SaturationModel;
```

## Module `turbulence`

# `closures::turbulence` — two-phase / porous turbulence closures

Rust port of GeN-Foam's `physicsModels/turbulenceModels/{LaheyKEpsilon,
mixtureKEpsilon,porousKEpsilon,porousKEpsilon2PhaseCorrected}`. Each
upstream class is a full OpenFOAM `RASModel` that builds and solves
`fvScalarMatrix` transport equations for `k` and `epsilon` on mesh fields —
machinery this crate does not have and, per the port plan, does not
re-implement here (the generic single-phase k-epsilon transport equation
itself is reused from `outram_foam_turbulence_lib`). **This module ports
only the porous- and two-phase-specific closure terms**: the small,
self-contained pieces of algebra that modify or add to the generic
production/dissipation/eddy-viscosity terms. See the "Ported vs. deferred"
section below for the exact boundary.

## Sub-modules

| Sub-module | Upstream class | Reference |
|---|---|---|
| [`lahey_k_epsilon`] | `LaheyKEpsilon` | Lahey Jr., R.T. (2005), *Nucl. Eng. Des.* 235(10), 1043-1060 |
| [`mixture_k_epsilon`] | `mixtureKEpsilon` | Behzadi, Issa & Rusche (2004), *Chem. Eng. Sci.* 59(4), 759-770; bubble term from Lahey (2005) |
| [`porous_k_epsilon`] | `porousKEpsilon`, `porousKEpsilon2PhaseCorrected` | GeN-Foam-original (no external reference) |
| [`units`] | — | local named `uom` aliases for this sub-module |

`multiphaseCompressibleTurbulenceModels.C` (the upstream
`addToRunTimeSelectionTable`/`makeTurbulenceModelTypes` registration
boilerplate) has no algebraic content and is intentionally not ported.

## Ported vs. deferred

**Ported** (pure algebra: local scalar in, local scalar out, no mesh/field
state):
- [`drag_coefficient_from_kd`] (below) — the `Cd()` inversion shared
  verbatim by `LaheyKEpsilon` and `mixtureKEpsilon`.
- Lahey: the bubble-induced eddy-viscosity addend, the bubble-induced
  production term `bubbleG`, the gas-phase-transfer relaxation rate, and
  the resulting `k`/`epsilon` production-rate compositions. See
  [`lahey_k_epsilon`].
- mixtureKEpsilon: the dispersed-phase turbulent-response coefficient
  `Ct2`, the virtual-mass-corrected effective gas density, the mixture
  density and mass-weighted `k`/`epsilon` mixing, the (dimensionally
  distinct) mixture `bubbleG`, and the resulting production-rate
  compositions. See [`mixture_k_epsilon`].
- porousKEpsilon / porousKEpsilon2PhaseCorrected: the turbulence-intensity
  correlation for the porous-zone equilibrium `k`, the mixing-length
  equilibrium `epsilon`, the relaxation-toward-equilibrium coefficient, and
  the `nut` stabilisation addend. See [`porous_k_epsilon`].

**Deferred to the solver-integration bead** (needs mesh/field machinery
this crate does not have — `volScalarField`, `fvm::div`/`fvm::laplacian`,
`fvScalarMatrix` assembly, boundary-condition correction):
- The generic single-phase k-epsilon production `G =
  nut*(dev(twoSymm(gradU)) && gradU)` and the `k`/`epsilon` transport
  equations themselves (`fvm::ddt` + `fvm::div` + `fvm::laplacian` ==
  production − dissipation + these closure source terms). Reused from
  `outram_foam_turbulence_lib`.
- `correctNut()`'s field-level orchestration (looking up phase/pair
  objects from the mesh registry, calling `correctBoundaryConditions()`,
  `fv::options` correction).
- `porousKEpsilon`'s constructor-time per-region dictionary parsing that
  paints `convergenceLength`/`turbulenceIntensityCoeff`/etc. onto cell
  zones — a mesh/structure concern, not closure algebra. This module takes
  those already-resolved per-cell coefficients as plain `f64` struct
  fields instead.
- `porousKEpsilon::kSource`/`epsilonSource` are, upstream, literally empty
  `fvScalarMatrix`s (zero-dimensioned placeholders with no algebraic
  content) — there is nothing to port for either porous model.
- `mixtureKEpsilon::correct()`'s phase-averaging orchestration (looking up
  the sibling phase's turbulence model from the mesh registry, `mixFlux`,
  `mixU` on face-interpolated `surfaceScalarField`s, the final
  `kl = Cc2*km` / `kg = Ct2*kl` back-substitution) — all mesh/field state.

Port status: the closure algebra above is implemented and unit-tested under
bead op-p6p.7.9; the solver-integration items listed immediately above
remain deferred. See `docs/genfoam-port-plan.md`.

```rust
pub mod turbulence { /* ... */ }
```

### Modules

## Module `lahey_k_epsilon`

# `lahey_k_epsilon` — bubble-induced turbulence (Lahey, 2005)

Port of GeN-Foam's `LaheyKEpsilon`: a continuous-liquid-phase k-epsilon
model with an added bubble-induced-turbulence source, valid for dispersed
gas / continuous liquid systems (per the upstream class doc: applying it to
continuous-gas systems runs but is not physically validated, since the
source terms hard-code the gas `Dh` and liquid `rho`).

Reference: Lahey Jr., R. T. (2005). *The simulation of multidimensional
multiphase flows.* Nuclear Engineering and Design, 235(10), 1043-1060.

## What this module ports

[`LaheyBubbleClosure`] carries the four upstream model coefficients
(`Cp`, `Cmub`, `C3`, `alphaInversion`; `Cmu`/`C1`/`C2`/`sigmak`/`sigmaEps`
belong to the reused generic k-epsilon and are not duplicated here) and
exposes:

- [`LaheyBubbleClosure::bubble_induced_eddy_viscosity`] — the additive
  `nut` term from `correctNut()` (the generic `Cmu*k^2/epsilon` term is
  reused from `outram_foam_turbulence_lib`, not ported here).
- [`LaheyBubbleClosure::bubble_induced_production`] — `bubbleG()`.
- [`LaheyBubbleClosure::phase_transfer_rate`] — `phaseTransferCoeff()`,
  with the `rho` factor stripped out (see its doc for why).
- [`LaheyBubbleClosure::k_production_rate`] /
  [`LaheyBubbleClosure::epsilon_production_rate`] — the explicit parts of
  `kSource()`/`epsilonSource()`, recast as **specific** (per-unit-mass)
  rates so they compose with [`bubble_induced_production`]. The `alpha*rho`
  volumetric weighting and the `fvm::Sp`/`fvm::Su` matrix assembly are
  solver-integration concerns (see the parent module doc).

`Cd()` is shared with `mixtureKEpsilon` and lives at
[`super::drag_coefficient_from_kd`].

```rust
pub mod lahey_k_epsilon { /* ... */ }
```

### Types

#### Struct `LaheyBubbleClosure`

Bubble-induced-turbulence closure coefficients (Lahey, 2005).

Upstream defaults (from `LaheyKEpsilonCoeffs`): `cp = 0.25`,
`cmub = 0.6`, `c3 = -0.33` (upstream default; the constructor falls back to
`C2` — usually `1.92` — if `C3` is not set in the dictionary, but `-0.33`
is Lahey's own published value and the one documented in the class header),
`alpha_inversion = 0.3`.

```rust
pub struct LaheyBubbleClosure {
    pub cp: f64,
    pub cmub: f64,
    pub c3: f64,
    pub alpha_inversion: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cp` | `f64` | Bubble-induced-production coefficient `Cp` (dimensionless). |
| `cmub` | `f64` | Bubble-induced-viscosity coefficient `Cmub` (dimensionless). |
| `c3` | `f64` | Epsilon-production coefficient `C3` for the bubble-induced source<br>(dimensionless; Lahey's value is `-0.33`, note the sign). |
| `alpha_inversion` | `f64` | Void-fraction threshold `alphaInversion` above which the gas phase is<br>treated as continuous (no phase-transfer relaxation applied above it;<br>dimensionless, `0..=1`). |

##### Implementations

###### Methods

- ```rust
  pub const fn new_default() -> Self { /* ... */ }
  ```
  Upstream default coefficients (`LaheyKEpsilonCoeffs` defaults).

- ```rust
  pub fn drag_coefficient(kd: VolumetricRelaxationCoefficient, dh_gas: HydraulicDiameter, rho_liquid: MassDensity, alpha_liquid: VoidFraction, alpha_gas: VoidFraction, relative_velocity: RelativeVelocity) -> DragCoefficient { /* ... */ }
  ```
  Invert `Cd` from the interfacial friction coefficient `Kd`. Thin

- ```rust
  pub fn bubble_induced_eddy_viscosity(self: &Self, dh_gas: HydraulicDiameter, alpha_gas: VoidFraction, relative_velocity: RelativeVelocity) -> EddyViscosity { /* ... */ }
  ```
  The bubble-induced addend to the turbulent (eddy) viscosity.

- ```rust
  pub fn bubble_induced_production(self: &Self, drag_coefficient: DragCoefficient, alpha_gas: VoidFraction, relative_velocity: RelativeVelocity, dh_gas: HydraulicDiameter) -> TurbulentDissipationRate { /* ... */ }
  ```
  Bubble-induced turbulent-kinetic-energy production `bubbleG`.

- ```rust
  pub fn phase_transfer_rate(self: &Self, alpha_gas: VoidFraction, gas_k: TurbulentKineticEnergy, gas_epsilon: TurbulentDissipationRate, timestep: Time) -> Frequency { /* ... */ }
  ```
  Gas-to-liquid turbulence phase-transfer relaxation rate.

- ```rust
  pub fn k_production_rate(self: &Self, alpha_gas: VoidFraction, bubble_induced_production: TurbulentDissipationRate, phase_transfer_rate: Frequency, gas_k: TurbulentKineticEnergy, local_k: TurbulentKineticEnergy) -> TurbulentDissipationRate { /* ... */ }
  ```
  Net specific (per-unit-mass) rate of change of `k` from the bubble and

- ```rust
  pub fn epsilon_production_rate(self: &Self, alpha_gas: VoidFraction, epsilon: TurbulentDissipationRate, k: TurbulentKineticEnergy, bubble_induced_production: TurbulentDissipationRate, phase_transfer_rate: Frequency, gas_epsilon: TurbulentDissipationRate) -> TurbulentDissipationRateOfChange { /* ... */ }
  ```
  Net rate of change of `epsilon` from the bubble and phase-transfer

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
    fn clone(self: &Self) -> LaheyBubbleClosure { /* ... */ }
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
    fn eq(self: &Self, other: &LaheyBubbleClosure) -> bool { /* ... */ }
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
## Module `mixture_k_epsilon`

# `mixture_k_epsilon` — mixture-model two-phase turbulence

Port of GeN-Foam's `mixtureKEpsilon`: a shared-`k`/`epsilon` mixture
turbulence model for dispersed gas-liquid systems, based on Behzadi, Issa
& Rusche (2004) with an effective gas density (virtual-mass-corrected) and
a Lahey-style bubble-induced production term.

References:
- Behzadi, A., Issa, R. I., & Rusche, H. (2004). *Modelling of dispersed
  bubble and droplet flow at high phase fractions.* Chemical Engineering
  Science, 59(4), 759-770.
- Lahey Jr., R. T. (2005). *The simulation of multidimensional multiphase
  flows.* Nuclear Engineering and Design, 235(10), 1043-1060 (bubble term).

## What this module ports

[`MixtureKEpsilonClosure`] carries `Cmu`, `Cp`, `C3` and exposes:

- [`MixtureKEpsilonClosure::dispersion_coefficient`] — `Ct2()`, the
  dispersed-phase turbulent-response coefficient (how closely the gas
  phase's turbulence tracks the liquid's).
- [`MixtureKEpsilonClosure::effective_gas_density`] — `rhogEff()`.
- [`MixtureKEpsilonClosure::mixture_density`] — `rhom()`.
- [`MixtureKEpsilonClosure::mix_k`] / [`MixtureKEpsilonClosure::mix_epsilon`]
  — the two instantiations of `mix()`.
- [`MixtureKEpsilonClosure::bubble_induced_production`] — `bubbleG()`
  (dimensionally distinct from the Lahey model's — see
  [`super::units::MixtureBubbleProduction`]).
- [`MixtureKEpsilonClosure::k_production_rate`] /
  [`MixtureKEpsilonClosure::epsilon_production_rate`] — `kSource()`/
  `epsilonSource()`, which upstream are already plain explicit sources
  (`fvm::Su`), so these need no further decomposition.

`Cd()` is shared with `LaheyKEpsilon` and lives at
[`super::drag_coefficient_from_kd`]. `rholEff()` upstream is a trivial
passthrough (`return liquid().rho();`) — not ported as its own function;
callers pass the liquid density directly wherever `rholEff` would appear.
`mixFlux`/`mixU`/`Cc2` and the `correct()` phase-averaging orchestration
operate on `surfaceScalarField`s / look up sibling phase objects from the
mesh registry — deferred (see the parent module doc).

```rust
pub mod mixture_k_epsilon { /* ... */ }
```

### Types

#### Struct `MixtureKEpsilonClosure`

Mixture k-epsilon closure coefficients (Behzadi/Issa/Rusche + Lahey bubble
term).

Upstream defaults (`mixtureKEpsilonCoeffs`): `cmu = 0.09`, `cp = 0.25`
(from the base `LaheyKEpsilonCoeffs`-style default, reused here), `c3`
defaults to `C2` (typically `1.92`) unless overridden. `C1`/`C2`/
`sigmak`/`sigmaEps` belong to the reused generic k-epsilon transport
equation and are not duplicated here.

```rust
pub struct MixtureKEpsilonClosure {
    pub cmu: f64,
    pub cp: f64,
    pub c3: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cmu` | `f64` | Standard k-epsilon eddy-viscosity coefficient `Cmu` (dimensionless),<br>used here only inside the dispersion-coefficient formula. |
| `cp` | `f64` | Bubble-induced-production coefficient `Cp` (dimensionless). |
| `c3` | `f64` | Epsilon-production coefficient `C3` for the bubble-induced source<br>(dimensionless). |

##### Implementations

###### Methods

- ```rust
  pub const fn new_default() -> Self { /* ... */ }
  ```
  Upstream default coefficients (`mixtureKEpsilonCoeffs` defaults, with

- ```rust
  pub fn drag_coefficient(kd: VolumetricRelaxationCoefficient, dh_gas: HydraulicDiameter, rho_liquid: MassDensity, alpha_liquid: VoidFraction, alpha_gas: VoidFraction, relative_velocity: RelativeVelocity) -> DragCoefficient { /* ... */ }
  ```
  Invert `Cd` from the interfacial friction coefficient `Kd`. Thin

- ```rust
  pub fn dispersion_coefficient(self: &Self, kd: VolumetricRelaxationCoefficient, rho_liquid: MassDensity, rho_gas: MassDensity, liquid_k: TurbulentKineticEnergy, liquid_epsilon: TurbulentDissipationRate, alpha_gas: VoidFraction) -> Ratio { /* ... */ }
  ```
  Dispersed-phase turbulent-response coefficient `Ct2`.

- ```rust
  pub fn effective_gas_density(rho_gas: MassDensity, virtual_mass_coefficient: Ratio, rho_liquid: MassDensity) -> MassDensity { /* ... */ }
  ```
  Virtual-mass-corrected effective gas density `rhogEff = rho_g +

- ```rust
  pub fn mixture_density(alpha_liquid: VoidFraction, rho_liquid_eff: MassDensity, alpha_gas: VoidFraction, rho_gas_eff: MassDensity) -> MassDensity { /* ... */ }
  ```
  Mixture density `rhom = alpha_l*rholEff + alpha_g*rhogEff`.

- ```rust
  pub fn mix_k(alpha_liquid: VoidFraction, rho_liquid_eff: MassDensity, liquid_k: TurbulentKineticEnergy, alpha_gas: VoidFraction, rho_gas_eff: MassDensity, gas_k: TurbulentKineticEnergy) -> TurbulentKineticEnergy { /* ... */ }
  ```
  Mass-weighted mixture turbulent kinetic energy.

- ```rust
  pub fn mix_epsilon(alpha_liquid: VoidFraction, rho_liquid_eff: MassDensity, liquid_epsilon: TurbulentDissipationRate, alpha_gas: VoidFraction, rho_gas_eff: MassDensity, gas_epsilon: TurbulentDissipationRate) -> TurbulentDissipationRate { /* ... */ }
  ```
  Mass-weighted mixture turbulent dissipation rate.

- ```rust
  pub fn bubble_induced_production(self: &Self, alpha_liquid: VoidFraction, rho_liquid: MassDensity, drag_coefficient: DragCoefficient, alpha_gas: VoidFraction, relative_velocity: RelativeVelocity, dh_gas: HydraulicDiameter) -> MixtureBubbleProduction { /* ... */ }
  ```
  Mixture-model bubble-induced production term `bubbleG`.

- ```rust
  pub fn k_production_rate(bubble_induced_production: MixtureBubbleProduction, rho_mixture: MassDensity) -> TurbulentDissipationRate { /* ... */ }
  ```
  Specific mixture-`k` production rate — upstream `kSource() =

- ```rust
  pub fn epsilon_production_rate(self: &Self, epsilon_mixture: TurbulentDissipationRate, k_mixture: TurbulentKineticEnergy, bubble_induced_production: MixtureBubbleProduction, rho_mixture: MassDensity) -> TurbulentDissipationRateOfChange { /* ... */ }
  ```
  Specific mixture-`epsilon` production rate — upstream `epsilonSource()

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
    fn clone(self: &Self) -> MixtureKEpsilonClosure { /* ... */ }
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
    fn eq(self: &Self, other: &MixtureKEpsilonClosure) -> bool { /* ... */ }
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
## Module `porous_k_epsilon`

# `porous_k_epsilon` — porous-medium `k`/`epsilon` relaxation

Port of GeN-Foam's `porousKEpsilon` and `porousKEpsilon2PhaseCorrected`.
Both force `k` and `epsilon` to relax toward an **equilibrium** value
inside porous zones (fuel-pin bundles, spacer grids — anywhere sub-mesh
structure exists), because the mesh is too coarse to resolve the actual
turbulence generated by that structure. The equilibrium values come from a
turbulence-intensity/length-scale correlation, not from solving a transport
equation. `porousKEpsilon2PhaseCorrected` differs only in adding a
void-fraction-dependent term to the turbulence-intensity correlation.

GeN-Foam-original models (no external literature reference beyond the
standard turbulence-intensity/mixing-length correlation form
`I = A*Re^B`, `L = C*Dh`).

## What this module ports

[`PorousKEpsilonClosure`] carries the porous-relaxation coefficients and
exposes:

- [`PorousKEpsilonClosure::equilibrium_k`] — the turbulence-intensity
  correlation for the porous-zone equilibrium `k`.
- [`PorousKEpsilonClosure::equilibrium_epsilon`] — the mixing-length
  correlation for the porous-zone equilibrium `epsilon`.
- [`PorousKEpsilonClosure::relaxation_coefficient`] — the
  relaxation-toward-equilibrium coefficient `alphaRhoConv`.
- [`PorousKEpsilonClosure::nut_stabilization`] — the optional `nut`
  stabilisation addend (`DhStruct`/`laminarReStruct`).

[`PorousKEpsilon2PhaseClosure`] wraps a [`PorousKEpsilonClosure`] (adding
the `turbulenceIntensityAlphaCoeff` term `D`) and overrides only
`equilibrium_k`; `equilibrium_epsilon`, `relaxation_coefficient`, and
`nut_stabilization` are identical between the two upstream classes, so
[`PorousKEpsilon2PhaseClosure`] delegates to its embedded base rather than
duplicating them (composition, not inheritance — no trait objects).

**`kSource`/`epsilonSource` are not ported**: upstream, both are literally
empty placeholder `fvScalarMatrix`s (`new fvScalarMatrix(k_, <dims>)` with
no coefficients set) for *both* `porousKEpsilon` and
`porousKEpsilon2PhaseCorrected` — there is no algebra there to port. The
actual porous relaxation happens via `alphaRhoConv` directly inside
`correct()`'s `epsEqn`/`kEqn` assembly (`- fvm::Sp(alphaRhoConv, k) +
alphaRhoConv*equilibriumK`), which is
[`relaxation_coefficient`](PorousKEpsilonClosure::relaxation_coefficient)
plus [`equilibrium_k`](PorousKEpsilonClosure::equilibrium_k) — both ported
above.

```rust
pub mod porous_k_epsilon { /* ... */ }
```

### Types

#### Struct `PorousKEpsilonClosure`

Porous-medium `k`/`epsilon` relaxation-to-equilibrium closure
(`porousKEpsilon`).

Fields correspond to the upstream per-region dictionary entries
(`porousKEpsilonProperties.<region>`); upstream reads and paints these onto
cell zones at construction time (a mesh/structure concern — see the parent
module doc) — here they are plain already-resolved per-cell coefficients.

```rust
pub struct PorousKEpsilonClosure {
    pub cmu: f64,
    pub turbulence_intensity_coeff: f64,
    pub turbulence_intensity_exp: f64,
    pub turbulence_length_scale_coeff: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cmu` | `f64` | Standard k-epsilon eddy-viscosity coefficient `Cmu` (dimensionless,<br>upstream default `0.09`), used here only inside<br>[`equilibrium_epsilon`](Self::equilibrium_epsilon) as `Cmu^0.75`. |
| `turbulence_intensity_coeff` | `f64` | Turbulence-intensity correlation coefficient `A` in `I_t =<br>A*Re^B` (dimensionless). |
| `turbulence_intensity_exp` | `f64` | Turbulence-intensity correlation exponent `B` in `I_t = A*Re^B`<br>(dimensionless, typically negative). |
| `turbulence_length_scale_coeff` | `f64` | Turbulence length-scale correlation coefficient `C` in `L_t = C*Dh`<br>(dimensionless). |

##### Implementations

###### Methods

- ```rust
  pub fn equilibrium_k(self: &Self, speed: Velocity, reynolds_number: ReynoldsNumber) -> TurbulentKineticEnergy { /* ... */ }
  ```
  Porous-zone equilibrium turbulent kinetic energy.

- ```rust
  pub fn equilibrium_epsilon(self: &Self, equilibrium_k: TurbulentKineticEnergy, structure_dh: HydraulicDiameter) -> TurbulentDissipationRate { /* ... */ }
  ```
  Porous-zone equilibrium turbulent dissipation rate.

- ```rust
  pub fn relaxation_coefficient(alpha: VoidFraction, rho: MassDensity, speed: Velocity, convergence_length: HydraulicDiameter) -> VolumetricRelaxationCoefficient { /* ... */ }
  ```
  Relaxation-toward-equilibrium coefficient `alphaRhoConv`.

- ```rust
  pub fn nut_stabilization(speed: Velocity, dh_struct: HydraulicDiameter, laminar_re_struct: f64) -> EddyViscosity { /* ... */ }
  ```
  Optional `nut` stabilisation addend, applied inside porous cells when

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
    fn clone(self: &Self) -> PorousKEpsilonClosure { /* ... */ }
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
    fn eq(self: &Self, other: &PorousKEpsilonClosure) -> bool { /* ... */ }
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
#### Struct `PorousKEpsilon2PhaseClosure`

Two-phase-corrected porous-medium `k`/`epsilon` relaxation closure
(`porousKEpsilon2PhaseCorrected`).

Identical to [`PorousKEpsilonClosure`] except the turbulence-intensity
correlation gains an additive void-fraction term `D*(1 - alpha_relative)`.
Wraps (rather than duplicates) a [`PorousKEpsilonClosure`] for the other
three methods.

```rust
pub struct PorousKEpsilon2PhaseClosure {
    pub base: PorousKEpsilonClosure,
    pub turbulence_intensity_alpha_coeff: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `base` | `PorousKEpsilonClosure` | The shared porous-relaxation coefficients and methods<br>(`equilibrium_epsilon`, `relaxation_coefficient`,<br>`nut_stabilization`, and the `A`/`B` turbulence-intensity terms). |
| `turbulence_intensity_alpha_coeff` | `f64` | Void-fraction turbulence-intensity coefficient `D` in `I_t =<br>A*Re^B + D*(1 - alpha_relative)` (dimensionless). |

##### Implementations

###### Methods

- ```rust
  pub fn equilibrium_k(self: &Self, speed: Velocity, reynolds_number: ReynoldsNumber, relative_alpha: VoidFraction) -> TurbulentKineticEnergy { /* ... */ }
  ```
  Porous-zone equilibrium turbulent kinetic energy, two-phase-corrected.

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
    fn clone(self: &Self) -> PorousKEpsilon2PhaseClosure { /* ... */ }
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
    fn eq(self: &Self, other: &PorousKEpsilon2PhaseClosure) -> bool { /* ... */ }
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
## Module `units`

# Named `uom` aliases for the two-phase / porous turbulence closures

[`super`]'s bubble-induced and porous-relaxation closures combine void
fractions, slip velocities, and the turbulence pair `(k, epsilon)` into a
handful of quantities `uom` does not all have off-the-shelf names for. This
module gives each one a named, dimension-checked type so a reader hovering
in their editor sees `EddyViscosity` or `TurbulentKineticEnergy`, not a raw
`Quantity<...>`.

**Local to this sub-module.** [`ReynoldsNumber`](super::super::super::units::ReynoldsNumber)
is reused from the already-wired-in
[`thermal_hydraulics::units`](super::super::super::units) module. The
quantities below were defined fresh here during the initial port, when the
sibling `phase` / `thermophysical` sub-modules were not yet wired into the
tree. Those modules are **now** live, so [`VoidFraction`] here is a
near-exact duplicate of
[`phase::phase_base::VolumeFraction`](super::super::super::phase::phase_base::VolumeFraction)
(both `uom` `Ratio`). Unifying them — re-export one from the other, or hoist
both to the shared `thermal_hydraulics::units` — is a small follow-up tracked
in beads (see the appbuilder epic `op-p6p`), deliberately left out of the
integration pass to avoid churning verified turbulence closures.

## Standard quantities (aliases of existing `uom` types)

| Alias | `uom` type | Base SI | Physical meaning |
|---|---|---|---|
| [`VoidFraction`] | `Ratio` | – | phase volume fraction `alpha`, `0..=1` |
| [`RelativeVelocity`] | `Velocity` | m/s | interfacial slip speed `\|U_c - U_d\|` |
| [`HydraulicDiameter`] | `Length` | m | dispersed-phase or porous-structure `Dh` |
| [`EddyViscosity`] | `KinematicViscosity` | m^2/s | turbulent (eddy) viscosity `nu_t` |
| [`TurbulentKineticEnergy`] | `AvailableEnergy` | m^2/s^2 | `k` |
| [`TurbulentDissipationRate`] | `SpecificPower` | m^2/s^3 | `epsilon`, and `dk/dt`-shaped rates |
| [`DragCoefficient`] | `Ratio` | – | fluid-fluid drag coefficient `Cd` |

## Composite quantities (no built-in `uom` name)

| Alias | Base SI | Physical meaning |
|---|---|---|
| [`VolumetricRelaxationCoefficient`] | kg/(m^3 s) | `Kd` (interfacial friction) and `alpha*rho/tau` (porous relaxation) |
| [`MixtureBubbleProduction`] | kg/(m s^3) | mixture-model bubble-induced production `bubbleG` (dimensionally distinct from the Lahey model's — see [`super::mixture_k_epsilon`]) |
| [`TurbulentDissipationRateOfChange`] | m^2/s^4 | `d(epsilon)/dt`-shaped production terms |

```rust
pub mod units { /* ... */ }
```

### Types

#### Type Alias `VoidFraction`

Phase **void (volume) fraction** `alpha` — **dimensionless** (`0..=1`).

Aliased to [`uom`]'s [`Ratio`](uom::si::f64::Ratio). See the module doc for
why this duplicates (for now) `phase::phase_base::VolumeFraction`.

```rust
pub type VoidFraction = uom::si::f64::Ratio;
```

#### Type Alias `RelativeVelocity`

Interfacial **relative (slip) velocity magnitude** `\|U_c - U_d\|` —
**base SI: m/s**. Aliased to [`uom`]'s [`Velocity`](uom::si::f64::Velocity).

```rust
pub type RelativeVelocity = uom::si::f64::Velocity;
```

#### Type Alias `HydraulicDiameter`

A **hydraulic diameter** — **base SI: m**. Used both for the dispersed
(bubble/droplet) phase's `Dh` and the porous structure's `Dh`. Aliased to
[`uom`]'s [`Length`](uom::si::f64::Length).

```rust
pub type HydraulicDiameter = uom::si::f64::Length;
```

#### Type Alias `EddyViscosity`

**Eddy (turbulent) viscosity** `nu_t` — **base SI: m^2/s**. Aliased to
[`uom`]'s [`KinematicViscosity`](uom::si::f64::KinematicViscosity).

```rust
pub type EddyViscosity = uom::si::f64::KinematicViscosity;
```

#### Type Alias `TurbulentKineticEnergy`

**Turbulent kinetic energy** `k` — **base SI: m^2/s^2 (J/kg)**. `uom` has
no quantity named "turbulent kinetic energy"; dimensionally it is a
mass-specific energy, so this aliases [`uom`]'s
[`AvailableEnergy`](uom::si::f64::AvailableEnergy) (the quantity `uom` uses
for `J/kg`).

```rust
pub type TurbulentKineticEnergy = uom::si::f64::AvailableEnergy;
```

#### Type Alias `TurbulentDissipationRate`

**Turbulent dissipation rate** `epsilon` — **base SI: m^2/s^3 (W/kg)**.
Also used for any `dk/dt`-shaped rate (dimensionally identical: energy per
unit mass per unit time). Aliased to [`uom`]'s
[`SpecificPower`](uom::si::f64::SpecificPower).

```rust
pub type TurbulentDissipationRate = uom::si::f64::SpecificPower;
```

#### Type Alias `DragCoefficient`

Fluid-fluid **drag coefficient** `Cd` — **dimensionless**. Aliased to
[`uom`]'s [`Ratio`](uom::si::f64::Ratio).

```rust
pub type DragCoefficient = uom::si::f64::Ratio;
```

#### Type Alias `VolumetricRelaxationCoefficient`

A **volumetric relaxation coefficient** — **base SI: kg/(m^3 s)**
(`M L^-3 T^-1`).

GeN-Foam's fluid-fluid interfacial friction coefficient `Kd` (from
`F_drag/V = Kd * (U_c - U_d)`, so `Kd` has dimension
`(force/volume)/velocity = kg/(m^3 s)`) and the porous k-epsilon models'
`alpha*rho*(\|U\|/convergenceLength)` relaxation coefficient share this
exact dimension — both are "a density divided by a time scale". There is
no standard named `uom` quantity for it, so it is defined here from the
ISQ base, following the precedent in
`outram_foam_basic_lib::thermophysics::quantities::Compressibility` and
`genfoam::neutronics::xs::units`.

```rust
pub type VolumetricRelaxationCoefficient = uom::si::Quantity<uom::si::ISQ<uom::typenum::N3, uom::typenum::P1, uom::typenum::N1, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

#### Type Alias `MixtureBubbleProduction`

The **mixture-model bubble-induced production term** `bubbleG` —
**base SI: kg/(m s^3)** (`M L^-1 T^-3`).

GeN-Foam's `mixtureKEpsilon::bubbleG()` carries an extra `liquid()*rho_l`
factor the Lahey model's `bubbleG()` does not (the upstream source comments
this explicitly: "Differs from the Lahey model as it has this extra term
(which also makes them dimensionally different)"). The Lahey model's
`bubbleG` is a genuine specific (per-unit-mass) production rate — aliased
to [`TurbulentDissipationRate`] — while this one is not, hence the separate
composite type.

```rust
pub type MixtureBubbleProduction = uom::si::Quantity<uom::si::ISQ<uom::typenum::N1, uom::typenum::P1, uom::typenum::N3, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

#### Type Alias `TurbulentDissipationRateOfChange`

The **rate of change of the turbulent dissipation rate**, `d(epsilon)/dt`
— **base SI: m^2/s^4** (`L^2 T^-4`).

`epsilon` itself has dimension m^2/s^3 ([`TurbulentDissipationRate`]); its
own production/relaxation terms (as they appear on the RHS of the
`epsilon` transport equation) therefore carry one more inverse-time power.
No standard named `uom` quantity exists for this; defined here from the
ISQ base.

```rust
pub type TurbulentDissipationRateOfChange = uom::si::Quantity<uom::si::ISQ<uom::typenum::P2, uom::typenum::Z0, uom::typenum::N4, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

### Functions

#### Function `volumetric_relaxation_coefficient`

**Attributes:**

- `MustUse { reason: None }`

Wrap a base-SI value (`kg/(m^3 s)`) as a [`VolumetricRelaxationCoefficient`].

```rust
pub const fn volumetric_relaxation_coefficient(kilogram_per_cubic_metre_second: f64) -> VolumetricRelaxationCoefficient { /* ... */ }
```

#### Function `mixture_bubble_production`

**Attributes:**

- `MustUse { reason: None }`

Wrap a base-SI value (`kg/(m s^3)`) as a [`MixtureBubbleProduction`].

```rust
pub const fn mixture_bubble_production(kilogram_per_metre_second_cubed: f64) -> MixtureBubbleProduction { /* ... */ }
```

#### Function `dissipation_rate_of_change`

**Attributes:**

- `MustUse { reason: None }`

Wrap a base-SI value (`m^2/s^4`) as a [`TurbulentDissipationRateOfChange`].

```rust
pub const fn dissipation_rate_of_change(square_metre_per_second_quartic: f64) -> TurbulentDissipationRateOfChange { /* ... */ }
```

### Functions

#### Function `drag_coefficient_from_kd`

**Attributes:**

- `MustUse { reason: None }`

Invert GeN-Foam's fluid-fluid drag coefficient `Cd` from the interfacial
friction coefficient `Kd`.

Shared verbatim by `LaheyKEpsilon::Cd()` and `mixtureKEpsilon::Cd()`
(identical upstream formula in both classes). `Kd` is defined (per the
upstream doc comment on both classes) by

```text
Kd = 0.5 * (alpha_c*alpha_d)/(alpha_c+alpha_d) * (rho_c/Dh_d) * |U_c-U_d| * Cd
```

with `c`/`d` the continuous/dispersed phase; this function solves that
relation for `Cd` given the already-computed `Kd` (produced elsewhere by
the fluid-fluid drag closures, out of scope here — see
`closures::ff_drag`, bead op-p6p.7.5).

**Deliberate deviation from upstream:** when `alpha_continuous +
alpha_dispersed == 0` (a degenerate cell with neither phase present), the
C++ `(li*gi)/(li+gi)` evaluates `0.0/0.0 = NaN`; this port instead treats
that fraction as `0.0` before applying the `max(..., 1e-3 m/s)` floor
(matching the physically sensible limit — no phase present, no drag —
rather than propagating a `NaN`). This is a numerical-robustness guard on
an unreachable-in-practice edge case, not a tolerance change.

```rust
pub fn drag_coefficient_from_kd(kd: units::VolumetricRelaxationCoefficient, dh_dispersed: units::HydraulicDiameter, rho_continuous: uom::si::f64::MassDensity, alpha_continuous: units::VoidFraction, alpha_dispersed: units::VoidFraction, relative_velocity: units::RelativeVelocity) -> units::DragCoefficient { /* ... */ }
```

### Re-exports

#### Re-export `LaheyBubbleClosure`

```rust
pub use lahey_k_epsilon::LaheyBubbleClosure;
```

#### Re-export `MixtureKEpsilonClosure`

```rust
pub use mixture_k_epsilon::MixtureKEpsilonClosure;
```

#### Re-export `PorousKEpsilon2PhaseClosure`

```rust
pub use porous_k_epsilon::PorousKEpsilon2PhaseClosure;
```

#### Re-export `PorousKEpsilonClosure`

```rust
pub use porous_k_epsilon::PorousKEpsilonClosure;
```

## Module `function_objects`

# `genfoam::thermal_hydraulics::function_objects` — TH post-processing diagnostics

Run-time post-processing and diagnostic function objects, ported from
upstream `src/classes/thermalHydraulics/src/functionObjects/**`. These are
**pure post-processing/monitoring hooks over solver fields — no physics is
computed here.** No solver, closure, or run-loop logic belongs in this
module; every function here is a stateless reduction over a
`VolField`/`SurfaceField`/`FvMesh` snapshot passed in by the caller (the
eventual solver driver decides *when* and *how often* to call these — that
wiring is out of scope here, per bead op-p6p.7.14).

## Sub-module map

| Module | Upstream function object | What it computes |
|---|---|---|
| [`mass_flow`] | `massFlow` | Total mass flow rate through a patch/face set: `mdot = sum\|alphaRhoPhi\|` |
| [`pressure_drop`] | `pressureDrop` | Area-weighted-average pressure difference between two patches (or a patch vs. a reference) |
| [`t_bulk`] | `TBulk` | Flow-weighted (bulk/mixing-cup) temperature over a patch |
| [`patch_scalar_value`] | `patchScalarFieldValue` | Raw per-face patch values, plus a selectable reduction (sum/average/min/max/integral) |
| [`field_diff_extents`] | `fieldDiffExtents` | Spatial bounding-box extents of where one field exceeds a mask field (scalar only) |
| [`stop_if_max_field_diff`] | `stopIfMaxFieldDiff` | Stop-criterion decision: `max_cell(field1 - field2) > 0` |
| [`field_integral`] | `fieldIntegralToFMU` | Volume integral `sum(field * V)` (the FMU co-simulation export itself is out of scope) |

Every module documents exactly where it is a literal port vs. a
documented simplification/generalisation relative to the upstream C++ —
see each module's doc comment before assuming 1:1 behavioural parity.

## Design notes

- **No `dyn`/trait-object dispatch.** Where upstream uses a `regionType`
  enum + `switch`, this port uses a plain closed Rust enum
  ([`mass_flow::MassFlowRegion`], [`patch_scalar_value::PatchReduceOp`]).
- **`uom` at the API boundary.** Every function returning a genuine
  physical quantity (mass rate, pressure, temperature, area) returns a
  named `uom::si::f64` type, not a bare `f64`. The two exceptions —
  [`patch_scalar_value::reduce_patch_scalar_field`] and the
  [`field_integral`] functions — operate on a field of *generic*,
  caller-defined physical meaning (any scalar field), so a single named
  `uom` quantity would misrepresent them; both document this explicitly.
- **Panics over silent fallback**, per this crate's guardrails: an
  out-of-range patch index, a field/mesh size mismatch, or an empty
  reduction domain panics rather than returning a default value.

```rust
pub mod function_objects { /* ... */ }
```

### Modules

## Module `field_diff_extents`

`fieldDiffExtents` — spatial bounding-box extents of where one field
exceeds a "mask" field, at a single instant.

Port of `functionObjects::fieldDiffExtents`
(`fieldDiffExtents.C`/`.H`/`fieldDiffExtentsTemplates.C`), **scalar fields
only** — upstream is templated over `scalar`/`vector`/`sphericalTensor`/
`symmTensor`/`tensor`; this port implements only the `scalar`
instantiation, the common case for TH monitoring fields like `T`/`p`.

**Despite the "Diff" in the name, this does not compare a field to its own
value at a previous timestep.** It compares **two different fields**
(`field` vs `maskField`) at the *same* instant, cell-by-cell / face-by-face:

```text
mask_i = true   if field_i > maskField_i
       = false  otherwise
extents = bounding box of { C_i - C0 : mask_i }
```

where `C_i` is a cell centre (internal field) or patch face centre
(boundary), and `C0` is a user-chosen reference position (upstream's
`referencePosition`, default the origin).

Note the comparison is **unsigned, no `mag()`** — this matches upstream's
**scalar specialisation** of `calcMask`
(`return pos((field/oneField) - (maskField/oneMaskField));` in
`fieldDiffExtents.C`), which differs from the generic vector/tensor
template in `fieldDiffExtentsTemplates.C`
(`return pos(mag(field/oneField) - mag(maskField/oneMaskField));`). Since
this port only implements the scalar case, only the unsigned form applies.

If no cell/face satisfies the mask, upstream falls back to a degenerate
box at the reference position (`extents.add(point::zero)` after `C0`
subtraction) — [`internal_extents`] and [`patch_extents`] mirror that
fallback exactly, so an "empty" result is a well-defined zero-size box
rather than an unbounded/inverted one.

```rust
pub mod field_diff_extents { /* ... */ }
```

### Types

#### Struct `Extents`

Axis-aligned bounding box, `min`/`max` corner. Mirrors upstream's
`Foam::boundBox`.

```rust
pub struct Extents {
    pub min: outram_foam_basic_lib::primitives::Vector3,
    pub max: outram_foam_basic_lib::primitives::Vector3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `min` | `outram_foam_basic_lib::primitives::Vector3` |  |
| `max` | `outram_foam_basic_lib::primitives::Vector3` |  |

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
    fn clone(self: &Self) -> Extents { /* ... */ }
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
    fn eq(self: &Self, other: &Extents) -> bool { /* ... */ }
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
### Functions

#### Function `cell_mask`

Cell-wise (internal field) diff mask: `true` where `field_i > mask_field_i`.

Matches upstream's **scalar specialisation** of `calcMask` — unsigned
difference, no `mag()` (see module docs for why this differs from the
generic vector/tensor template).

# Panics
If `field.internal.len() != mask_field.internal.len()`.

```rust
pub fn cell_mask(field: &outram_foam_basic_lib::fields::VolScalarField, mask_field: &outram_foam_basic_lib::fields::VolScalarField) -> Vec<bool> { /* ... */ }
```

#### Function `patch_mask`

Patch-face diff mask on boundary patch `patch_index`: `true` where
`field_i > mask_field_i` at that face. Same semantics as [`cell_mask`], on
a patch's boundary values instead of the internal field.

# Panics
If the two fields' face counts on `patch_index` disagree, or `patch_index`
is out of range for either field's boundary.

```rust
pub fn patch_mask(field: &outram_foam_basic_lib::fields::VolScalarField, mask_field: &outram_foam_basic_lib::fields::VolScalarField, patch_index: usize) -> Vec<bool> { /* ... */ }
```

#### Function `internal_extents`

Bounding-box extents of `{ C_i - reference_position : mask[i] }` over cell
centres.

If no cell satisfies the mask, returns a degenerate box at the origin of
the reference-shifted frame (`min == max == Vector3::ZERO`) — mirrors
upstream's `extents.add(point::zero)` fallback.

# Panics
If `mask.len() != mesh.n_cells`.

```rust
pub fn internal_extents(mesh: &outram_foam_basic_lib::mesh::FvMesh, mask: &[bool], reference_position: outram_foam_basic_lib::primitives::Vector3) -> Extents { /* ... */ }
```

#### Function `patch_extents`

Bounding-box extents of `{ Cf_i - reference_position : mask[i] }` over the
face centres of boundary patch `patch_index`. Same empty-mask fallback as
[`internal_extents`].

# Panics
If `mask.len()` does not equal `mesh.patches[patch_index].size`, or
`patch_index` is out of range for `mesh.patches`.

```rust
pub fn patch_extents(mesh: &outram_foam_basic_lib::mesh::FvMesh, patch_index: usize, mask: &[bool], reference_position: outram_foam_basic_lib::primitives::Vector3) -> Extents { /* ... */ }
```

## Module `field_integral`

`fieldIntegralToFMU` — volume integral `sum_cells(field_c * V_c)`.

Port of `functionObjects::fieldIntegralToFMU`'s core computation
(`fieldIntegralToFMU.C`/`.H`: `result = gSum(fieldZone * volZone)`).

// TODO(genfoam): the FMU/co-simulation export plumbing
// (`commDataLayer::storeObj`/`getObj`, `externalIOObject`) is
// deliberately **not** ported — it is a network/process-boundary
// concern (FMI-standard scalar exchange with an external co-simulation
// master) with no analog in this crate. Only the volume-integral
// computation itself is ported; wiring its result out to an FMU is a
// separate, later concern (outside `outram-foam-appbuilder-lib`).

The physical unit of the result is `[field units] * m^3` — generic to
whatever quantity the caller integrates, so this returns a plain `f64`
rather than a named `uom` quantity (there is no single named quantity that
covers every field this can be called with). Upstream's own usage example
integrates a field named `heatFlux.structure` into a `hxPowerOut` FMU
scalar; that only comes out as a power if `heatFlux.structure` is actually
a volumetric power density (W/m^3), not a surface flux (W/m^2) despite the
name — that unit choice is the caller's responsibility, not this
function's.

```rust
pub mod field_integral { /* ... */ }
```

### Functions

#### Function `volume_integral_over_cells`

Volume integral of `field` over an explicit list of cell indices
(upstream's `cellZone`).

`sum_{c in cells}(field[c] * V[c])`.

# Panics
If any index in `cells` is out of range for `mesh.cell_volumes` /
`field.internal`.

```rust
pub fn volume_integral_over_cells(mesh: &outram_foam_basic_lib::mesh::FvMesh, field: &outram_foam_basic_lib::fields::VolScalarField, cells: &[usize]) -> f64 { /* ... */ }
```

#### Function `volume_integral`

Volume integral of `field` over the whole domain (all cells).

`sum_c(field[c] * V[c])`.

# Panics
If `field.internal.len() != mesh.n_cells`.

```rust
pub fn volume_integral(mesh: &outram_foam_basic_lib::mesh::FvMesh, field: &outram_foam_basic_lib::fields::VolScalarField) -> f64 { /* ... */ }
```

## Module `mass_flow`

`massFlow` — total mass flow rate through a boundary patch or an
arbitrary set of faces.

Port of `functionObjects::massFlow` (`massFlow.C`/`.H`). Upstream sums the
magnitude of an already area-integrated mass-flux face field
(`alphaRhoPhi`, OpenFOAM's `phi` convention: `rho_f * U_f . Sf`, units
kg/s per face) over the faces of a patch, faceSet, or faceZone:

```text
mdot = sum_faces |alphaRhoPhi_f|
```

Because `alphaRhoPhi` is already area-integrated, no separate multiply by
face area is needed here (contrast [`super::pressure_drop`] and
[`super::t_bulk`], which area-weight a *non*-integrated field).

Upstream's `faceSet`/`faceZone` region kinds both reduce to "an arbitrary
list of face indices" (`labelList faces_`) — this port collapses them into
[`MassFlowRegion::Faces`]. Upstream also supports an optional
`scaleFactor` (default 1.0) applied to the final result; that isn't
modelled as a parameter here since [`MassRate`] is a `uom` quantity and
already supports `mdot * k` for a plain `f64` scale `k`.

```rust
pub mod mass_flow { /* ... */ }
```

### Types

#### Enum `MassFlowRegion`

Region a [`total_mass_flow`] (or [`region_area`]) integral is evaluated
over.

Mirrors upstream `massFlow::regionType` (`patch` / `faceSet` / `faceZone`);
the latter two both reduce to an explicit face-index list here, since
basic-lib has no named face-set/face-zone object — the caller resolves the
face list itself.

```rust
pub enum MassFlowRegion {
    Patch(usize),
    Faces(Vec<usize>),
}
```

##### Variants

###### `Patch`

A named boundary patch, by index into [`FvMesh::patches`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Faces`

An arbitrary list of global face indices (internal or boundary),
mirroring upstream `faceSet`/`faceZone`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<usize>` |  |

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
    fn clone(self: &Self) -> MassFlowRegion { /* ... */ }
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

#### Function `total_mass_flow`

Total mass flow rate `mdot = sum_faces |alphaRhoPhi_f|` through `region`.

`alpha_rho_phi` must be the already area-integrated mass-flux face field
(`rho_f * U_f . Sf`, kg/s per face — OpenFOAM's `phi` convention),
evaluated on the same mesh `region` refers into. No separate `&FvMesh`
argument is needed: `alpha_rho_phi` already carries its own `Arc<FvMesh>`
(used internally by [`SurfaceScalarField::face_value`] for the `Faces`
case), and the `Patch` case only ever touches `alpha_rho_phi.boundary`.
Faces contribute by magnitude, matching upstream's
`mag(alphaRhoPhip[i])` — so flow direction does not cancel across faces of
a patch with mixed in/outflow, exactly as in GeN-Foam.

# Panics
If `region` names a patch index out of range for `alpha_rho_phi.boundary`,
or a face index out of range for the field's mesh.

```rust
pub fn total_mass_flow(alpha_rho_phi: &outram_foam_basic_lib::fields::SurfaceScalarField, region: &MassFlowRegion) -> uom::si::f64::MassRate { /* ... */ }
```

#### Function `region_area`

Total area of `region`'s faces — the `S` diagnostic upstream logs
alongside `mdot` (`"... massFlow = mDot kg/s over S m2"`).

# Panics
Same conditions as [`total_mass_flow`].

```rust
pub fn region_area(mesh: &outram_foam_basic_lib::mesh::FvMesh, region: &MassFlowRegion) -> uom::si::f64::Area { /* ... */ }
```

## Module `patch_scalar_value`

`patchScalarFieldValue` — per-face values of a scalar field on a boundary
patch, plus a selectable reduction over that patch.

Literal upstream `functionObjects::patchScalarFieldValue`
(`patchScalarFieldValue.C`/`.H`) does **no reduction at all** — its
`write()` just dumps the raw per-face boundary values as an OpenFOAM list
(`"( t ( v0 v1 ... vn ))"`). [`patch_field_values`] ports that literally.

This crate's porting brief additionally calls for a **selectable
reduction** (sum / average / min / max / integral) over the patch,
dispatched through a closed enum rather than upstream's raw dump —
[`PatchReduceOp`] and [`reduce_patch_scalar_field`] provide that. This is a
deliberate generalisation beyond upstream's `write()`, not a literal port
of it — documented here so the two are not confused.

```rust
pub mod patch_scalar_value { /* ... */ }
```

### Types

#### Enum `PatchReduceOp`

Reduction applied over a boundary patch's face values by
[`reduce_patch_scalar_field`]. Closed enum — no `dyn` dispatch.

```rust
pub enum PatchReduceOp {
    Sum,
    Average,
    Min,
    Max,
    Integral,
}
```

##### Variants

###### `Sum`

Unweighted sum of face values: `sum(v)`.

###### `Average`

Area-weighted average: `sum(v * |Sf|) / sum(|Sf|)`.

###### `Min`

Minimum face value.

###### `Max`

Maximum face value.

###### `Integral`

Area integral: `sum(v * |Sf|)`.

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
    fn clone(self: &Self) -> PatchReduceOp { /* ... */ }
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
    fn eq(self: &Self, other: &PatchReduceOp) -> bool { /* ... */ }
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
### Functions

#### Function `patch_field_values`

Raw per-face boundary values of `field` on `patch_index`, in face order.

Literal port of upstream `patchScalarFieldValue::write()`'s output list
(minus the `"( t ( ... ))"` text framing, which is a file-format concern
out of scope for a pure computation).

# Panics
If `patch_index` is out of range for `field.boundary`.

```rust
pub fn patch_field_values(field: &outram_foam_basic_lib::fields::VolScalarField, patch_index: usize) -> Vec<f64> { /* ... */ }
```

#### Function `reduce_patch_scalar_field`

Reduce `field`'s values over boundary patch `patch_index` by `op`.

See [`PatchReduceOp`] for the exact formula per variant.

# Panics
If `patch_index` is out of range for `mesh.patches` / `field.boundary`, or
(`Min`/`Max`) the patch has zero faces (fold over an empty iterator
returns `+-infinity`, which is intentionally left un-guarded — an empty
patch is a caller configuration error, not a runtime condition to paper
over silently).

```rust
pub fn reduce_patch_scalar_field(mesh: &outram_foam_basic_lib::mesh::FvMesh, field: &outram_foam_basic_lib::fields::VolScalarField, patch_index: usize, op: PatchReduceOp) -> f64 { /* ... */ }
```

## Module `pressure_drop`

`pressureDrop` — area-weighted-average pressure difference between two
boundary patches (or a patch average vs. a fixed reference pressure).

Port of `functionObjects::pressureDrop` (`pressureDrop.C`/`.H`), **patch
region kind only**. Upstream additionally supports a `faceSet` region kind
by face-interpolating the cell pressure field (`fvc::interpolate(p)`) onto
an arbitrary internal-face subset; that needs an interpolation weight
scheme for arbitrary faces, which basic-lib does not expose as a
general-purpose helper outside the `fvm`/`fvc` matrix-assembly path. That
variant is intentionally out of scope for this port.
// TODO(genfoam): add a `Faces` region kind once a standalone
// face-interpolation helper exists for an arbitrary face subset.

Upstream:
```text
p1 = sum_faces(p_f * |Sf_f|) / sum_faces(|Sf_f|)    (region 1, "upstream")
p2 = sum_faces(p_f * |Sf_f|) / sum_faces(|Sf_f|)    (region 2, "downstream")
deltaP = p1 - p2
```
For a patch region, `p_f` is the boundary patch field value
(`p.boundaryField()[patchID]`), i.e. already a per-face boundary value —
no interpolation needed, unlike the `faceSet` case above.

```rust
pub mod pressure_drop { /* ... */ }
```

### Functions

#### Function `patch_average_pressure`

Area-weighted average of `p` over boundary patch `patch_index`.

`p_avg = sum_faces(p_f * |Sf_f|) / sum_faces(|Sf_f|)` — the same
area-weighted average upstream computes for each side of a `pressureDrop`
function object (`pressureDrop::write()`, `regionType::patch` branch).

# Panics
If `patch_index` is out of range for `mesh.patches` / `p.boundary`, or the
patch has zero total area (division by zero).

```rust
pub fn patch_average_pressure(mesh: &outram_foam_basic_lib::mesh::FvMesh, p: &outram_foam_basic_lib::fields::VolScalarField, patch_index: usize) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `pressure_drop`

Pressure drop `p1 - p2` between two boundary patches.

`upstream_patch`/`downstream_patch` are patch indices into `mesh.patches`;
`p1`/`p2` are each computed by [`patch_average_pressure`]. Matches
upstream `pressureDrop::write()`'s `deltaP = p1 - p2` for the
`patch`/`patch` region-type combination.

# Panics
Same conditions as [`patch_average_pressure`], for either patch.

```rust
pub fn pressure_drop(mesh: &outram_foam_basic_lib::mesh::FvMesh, p: &outram_foam_basic_lib::fields::VolScalarField, upstream_patch: usize, downstream_patch: usize) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `pressure_drop_vs_reference`

Pressure drop between a patch's area-weighted average and a fixed
reference pressure (`p1 - p_ref`).

This is the "patch avg vs reference" mode named in this crate's porting
brief; it is not a literal upstream mode (upstream's `pressureDrop`
function object always compares two regions), but a direct specialisation
of the same area-weighted average with region 2 replaced by a supplied
constant.

```rust
pub fn pressure_drop_vs_reference(mesh: &outram_foam_basic_lib::mesh::FvMesh, p: &outram_foam_basic_lib::fields::VolScalarField, patch_index: usize, p_reference: uom::si::f64::Pressure) -> uom::si::f64::Pressure { /* ... */ }
```

## Module `stop_if_max_field_diff`

`stopIfMaxFieldDiff` — stop-criterion decision: true once
`max_cell(field1 - field2) > 0`.

Port of `functionObjects::stopIfMaxFieldDiff` (`.C`/`.H`). Upstream's
`write()` calls `time.writeAndEnd()` directly when the criterion trips;
**this port only computes the decision** ([`StopDecision`]) — wiring a
stop signal into an actual run/timestep loop is a solver-driver concern,
deliberately out of scope for a post-processing function object (per the
porting brief: "return a bool/enum decision; don't wire into any run
loop"). The caller decides what to do with [`StopDecision::stop`].

Compares only the **internal** field, matching upstream's
`max(field1-field2)` (an OpenFOAM `Foam::max` reduction over a
`GeometricField`'s internal field).

```rust
pub mod stop_if_max_field_diff { /* ... */ }
```

### Types

#### Struct `StopDecision`

Outcome of evaluating the `stopIfMaxFieldDiff` criterion.

```rust
pub struct StopDecision {
    pub stop: bool,
    pub max_diff: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stop` | `bool` | `true` once `max_diff > 0.0` — upstream's trigger condition. |
| `max_diff` | `f64` | `max_c (field1[c] - field2[c])` over all internal cells. |

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
    fn clone(self: &Self) -> StopDecision { /* ... */ }
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
    fn eq(self: &Self, other: &StopDecision) -> bool { /* ... */ }
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
### Functions

#### Function `evaluate_stop_if_max_field_diff`

Evaluate the `stopIfMaxFieldDiff` criterion for `field1`/`field2`.

Matches upstream `stopIfMaxFieldDiff::write()`:
`if (max(field1-field2).value() > 0.0) { ... stop ... }`.

# Panics
If `field1.internal.len() != field2.internal.len()`, or either field has
zero cells (the max of an empty field is undefined).

```rust
pub fn evaluate_stop_if_max_field_diff(field1: &outram_foam_basic_lib::fields::VolScalarField, field2: &outram_foam_basic_lib::fields::VolScalarField) -> StopDecision { /* ... */ }
```

## Module `t_bulk`

`TBulk` — flow-weighted (bulk / mixing-cup) temperature over a boundary
patch.

Port of `functionObjects::TBulk` (`TBulk.C`/`.H`), **patch region kind
only** — see [`super::pressure_drop`] for why the `faceSet`/`faceZone`
variants (which upstream serves via `fvc::interpolate`) are out of scope.

```text
T_bulk = sum_faces(|alphaRhoPhi_f| * Cp_f * T_f) / sum_faces(|alphaRhoPhi_f| * Cp_f)
```
the enthalpy-flux-weighted mean temperature — physically, the temperature
a perfectly-mixed downstream plenum would settle to. `Cp` weights both the
numerator and denominator to the same power, so its absolute scale/units
cancel in the ratio; only self-consistency between the two sums matters.
Upstream takes `thermo.Cp()`, a `volScalarField` with real `J/(kg K)`
units — any per-cell heat-capacity field works here as long as it's the
same field used to weight both sums (which [`bulk_temperature`] guarantees
by construction). The denominator is floored at `1e-9`, exactly matching
upstream's `Tb = hDot/max(hDotByT, 1e-9)`, to avoid a NaN when the patch
carries no flow.

```rust
pub mod t_bulk { /* ... */ }
```

### Functions

#### Function `bulk_temperature`

Flow-weighted (bulk) temperature over boundary patch `patch_index`.

`temperature` and `specific_heat` are per-cell fields; their **boundary**
values on `patch_index` are used (upstream's `T.boundaryField()[patchID]`
/ `Cp.boundaryField()[patchID]`). `alpha_rho_phi` is the area-integrated
mass-flux face field (see [`super::mass_flow::total_mass_flow`]) — its
boundary values on the same patch supply the flow weight, by magnitude
(matching upstream's `mag(alphaRhoPhip[i])`).

No mesh geometry (face areas) is needed here: unlike
[`super::pressure_drop::patch_average_pressure`], the weighting is already
carried by the area-integrated mass flux, not a raw field needing a
separate `|Sf|` multiply.

# Panics
If `patch_index` is out of range for any of the three fields' boundary
vectors, or the three patches disagree in face count (they must all
describe the same boundary patch of the same mesh).

```rust
pub fn bulk_temperature(patch_index: usize, temperature: &outram_foam_basic_lib::fields::VolScalarField, specific_heat: &outram_foam_basic_lib::fields::VolScalarField, alpha_rho_phi: &outram_foam_basic_lib::fields::SurfaceScalarField) -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

## Module `phase`

# `genfoam::thermal_hydraulics::phase` — fluid-phase field state and phase pairs

The **fluid-side field-state foundation** of GeN-Foam's porous-medium
two-fluid thermal-hydraulics. A porous cell carries, simultaneously, one or
more `fluid` phases (each a volume fraction `alpha`, a velocity, an enthalpy,
…) and an unresolved solid `structure` occupying the complementary volume.
This module owns the **fluid** side of that state; the solid `structure`
lives in [`super::structure`] and the correlation leaves in
[`super::closures`].

Ports upstream
`src/classes/thermalHydraulics/src/phaseModels/{phaseBase, fluid}` plus the
kinematic core of `physicsModels/phasePairs/{FSPair, FFPair}`.

## Module map

| Sub-module | Provides | Upstream |
|---|---|---|
| [`phase_base`] | [`PhaseBase`] — the shared `alpha` field + name + residual fraction every phase embeds; the [`VolumeFraction`] alias | `phaseBase.{C,H}` |
| [`fluid`] | [`Fluid`] — the per-cell fluid field-state bag; [`StateOfMatter`] dispatch | `fluid/fluid.{C,H}` |
| [`phase_pair`] | Pairwise [`fs_reynolds`] / [`ff_reynolds`] / [`ff_relative_velocity_magnitude`] — the dimensionless numbers the closures read | `phasePairs/{FSPair,FFPair}.C` |

## What lives here vs. elsewhere

This module is deliberately **state, not physics**: [`Fluid`] holds fields
and exposes them; the thermo package ([`super::thermophysical`]), turbulence
([`super::closures::turbulence`]), drag/heat-transfer closures, and the
porous solver ([`super::solver`]) update those fields by mutable reference.
The [`phase_pair`] helpers port only the *kinematic* dimensionless numbers
(Reynolds, relative velocity); the drag tensor `Kd` and heat-transfer
coefficient assembly that the full upstream `FSPair`/`FFPair` also perform
are closure/solver responsibilities and are **not** here.

## Dispatch: no trait objects

Per the workspace no-`dyn` rule, phase classification is a closed enum
([`StateOfMatter`]) dispatched by value, not runtime polymorphism. The
fluid-vs-structure distinction that upstream expresses through the
`phaseBase` base class is handled by the solver holding a [`Fluid`] and a
structure directly (both embed a [`PhaseBase`]); a unifying `Phase` enum, if
one is wanted, belongs at the solver level once [`super::structure`] lands,
since it must name both concrete types.

```rust
pub mod phase { /* ... */ }
```

### Modules

## Module `fluid`

# `fluid` — the fluid-phase field-state container

Port of GeN-Foam's `Foam::fluid`. Upstream describes this class as one that
*"mainly acts as a variable placeholder, as little functionality is deferred
to the class itself"* — it is the **per-cell field-state bag** the porous
momentum / energy / pressure solvers and every closure correlation read from
and write to. This Rust port keeps exactly that role: [`Fluid`] owns the
phase fields and exposes them; the physics (thermo, turbulence, drag, heat
transfer) lives in the `thermophysical`, `turbulence`, and `closures`
modules and updates these fields by reference.

## What it owns

- **Kinematics** — velocity `U` and its magnitude `magU`; the superficial
  volumetric flux `phi`, the phase volumetric flux `alphaPhi`, the phase mass
  flux `alphaRhoPhi`, and the cell-centred mass flux `alphaRhoMagU`; the
  dilation term `dgdt`; and the solver-normalised volume fraction
  `normalized`.
- **Thermophysics placeholders** — density `rho`, temperature `T`, specific
  enthalpy `h`, thermal conductivity `kappa`, specific heat `Cp`, dynamic
  viscosity `mu`, Prandtl number `Pr`. As upstream, these are *placeholders*
  the thermo package recomputes and writes into each step (see the field-unit
  table below).
- **Geometry** — the characteristic (hydraulic) diameter `Dh`.
- **Two-phase quantities** — flow quality `X`, Lockhart–Martinelli parameter
  `XLM`, and the dispersion marker; trivial (single-phase) unless a two-phase
  solver drives them.
- **Coupling / diagnostics** — the neutronics-to-fluid power density, the
  continuity error and its time-integral, and the `thermoResidualAlpha`
  markers that switch the energy equation off in near-empty cells.

## Field units (basic-lib fields are bare `f64`, unitful by convention)

[`outram_foam_basic_lib`]'s [`VolScalarField`] / [`VolVectorField`] /
[`SurfaceScalarField`] store plain `f64` per cell (or face), so each field's
physical unit is a documented convention, matching the precedent set by
`genfoam::neutronics`'s state. Scalar summaries that cross the public API
([`Fluid::thermo_residual_alpha`], [`Fluid::cumulative_continuity_error`])
are returned as named [`uom`] types.

| Field | Symbol | Unit |
|---|---|---|
| [`Fluid::alpha`] | `alpha` | dimensionless (`0..=1`) |
| [`Fluid::velocity`] | `U` | `m/s` (vector) |
| [`Fluid::mag_velocity`] | `\|U\|` | `m/s` |
| [`Fluid::phi`] | `phi` | `m^3/s` (superficial volumetric face flux) |
| [`Fluid::alpha_phi`] | `alphaPhi` | `m^3/s` (phase volumetric face flux) |
| [`Fluid::alpha_rho_phi`] | `alphaRhoPhi` | `kg/s` (phase mass face flux) |
| [`Fluid::alpha_rho_mag_u`] | `alphaRhoMagU` | `kg/(m^2 s)` (cell mass flux) |
| [`Fluid::dgdt`] | `dgdt` | `1/s` (dilation) |
| [`Fluid::normalized`] | `alpha_norm` | dimensionless |
| [`Fluid::density`] | `rho` | `kg/m^3` |
| [`Fluid::temperature`] | `T` | `K` |
| [`Fluid::enthalpy`] | `h` | `J/kg` (specific enthalpy) |
| [`Fluid::kappa`] | `kappa` | `W/(m K)` (thermal conductivity) |
| [`Fluid::cp`] | `Cp` | `J/(kg K)` (specific heat) |
| [`Fluid::mu`] | `mu` | `Pa s` (dynamic viscosity) |
| [`Fluid::prandtl`] | `Pr` | dimensionless |
| [`Fluid::hydraulic_diameter`] | `Dh` | `m` |
| [`Fluid::power_density`] | `q'''` | `W/m^3` |
| [`Fluid::continuity_error`] | `contErr` | `kg/(m^3 s)` |
| [`Fluid::flow_quality`] | `X` | dimensionless |
| [`Fluid::lockhart_martinelli`] | `XLM` | dimensionless |
| [`Fluid::dispersion`] | — | dimensionless (`0` continuous … `1` dispersed) |

```rust
pub mod fluid { /* ... */ }
```

### Types

#### Enum `StateOfMatter`

The physical state of matter of a [`Fluid`] phase.

Closed-enum port of GeN-Foam's `fluid::stateOfMatter`. Dispatched by value
(no trait objects) via [`Self::is_liquid`] / [`Self::is_gas`]; several
closures branch on it (e.g. boiling models only fire for a `Gas` companion
to a `Liquid`).

```rust
pub enum StateOfMatter {
    Undetermined,
    Liquid,
    Gas,
}
```

##### Variants

###### `Undetermined`

Not classified — the single-phase default (`onePhase` runs).

###### `Liquid`

Liquid phase.

###### `Gas`

Gas / vapour phase.

##### Implementations

###### Methods

- ```rust
  pub fn is_liquid(self: Self) -> bool { /* ... */ }
  ```
  `true` if this phase is explicitly the [`StateOfMatter::Liquid`].

- ```rust
  pub fn is_gas(self: Self) -> bool { /* ... */ }
  ```
  `true` if this phase is explicitly the [`StateOfMatter::Gas`].

- ```rust
  pub fn name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream dictionary keyword for this state

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
    fn clone(self: &Self) -> StateOfMatter { /* ... */ }
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
    fn default() -> StateOfMatter { /* ... */ }
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
    fn eq(self: &Self, other: &StateOfMatter) -> bool { /* ... */ }
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
#### Struct `Fluid`

A generic fluid phase: the per-cell field-state container the porous
thermal-hydraulics solvers and closures operate on.

Construct with [`Fluid::new`], which allocates every field on the shared
mesh with GeN-Foam's initial values (`alpha = 0`, `U = 0`, `Dh = SMALL`,
`XLM = 1e4`, markers = 0). A solver or thermo package then overwrites the
fields as it iterates; the `*_mut` accessors expose them for that.

All fields are defined on one shared [`FvMesh`] (`Arc`), reachable through
[`Fluid::mesh`]. The phase volume fraction and name live in the embedded
[`PhaseBase`].

```rust
pub struct Fluid {
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
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(mesh: Arc<FvMesh>, name: impl Into<String>, state_of_matter: StateOfMatter) -> Self { /* ... */ }
  ```
  Allocate a fluid phase named `name` on `mesh` with the given

- ```rust
  pub fn name(self: &Self) -> &str { /* ... */ }
  ```
  The phase name.

- ```rust
  pub fn mesh(self: &Self) -> &Arc<FvMesh> { /* ... */ }
  ```
  The mesh every field of this phase is defined on.

- ```rust
  pub fn base(self: &Self) -> &PhaseBase { /* ... */ }
  ```
  The embedded [`PhaseBase`] (volume fraction, name, residual alpha).

- ```rust
  pub fn base_mut(self: &mut Self) -> &mut PhaseBase { /* ... */ }
  ```
  Mutable access to the embedded [`PhaseBase`].

- ```rust
  pub fn alpha(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  The phase volume-fraction field `alpha` (dimensionless, `0..=1`).

- ```rust
  pub fn alpha_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
  Mutable access to the phase volume-fraction field `alpha`.

- ```rust
  pub fn state_of_matter(self: &Self) -> StateOfMatter { /* ... */ }
  ```
  This phase's state of matter (liquid / gas / undetermined).

- ```rust
  pub fn is_liquid(self: &Self) -> bool { /* ... */ }
  ```
  `true` if this phase is the liquid; forwards to [`StateOfMatter::is_liquid`].

- ```rust
  pub fn is_gas(self: &Self) -> bool { /* ... */ }
  ```
  `true` if this phase is the gas; forwards to [`StateOfMatter::is_gas`].

- ```rust
  pub fn velocity(self: &Self) -> &VolVectorField { /* ... */ }
  ```
  Velocity `U` (`m/s`, vector).

- ```rust
  pub fn velocity_mut(self: &mut Self) -> &mut VolVectorField { /* ... */ }
  ```
   Velocity `U` (`m/s`, vector).

- ```rust
  pub fn mag_velocity(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Velocity magnitude `|U|` (`m/s`).

- ```rust
  pub fn mag_velocity_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Velocity magnitude `|U|` (`m/s`).

- ```rust
  pub fn phi(self: &Self) -> &SurfaceScalarField { /* ... */ }
  ```
  Superficial volumetric face flux `phi` (`m^3/s`).

- ```rust
  pub fn phi_mut(self: &mut Self) -> &mut SurfaceScalarField { /* ... */ }
  ```
   Superficial volumetric face flux `phi` (`m^3/s`).

- ```rust
  pub fn alpha_phi(self: &Self) -> &SurfaceScalarField { /* ... */ }
  ```
  Phase volumetric face flux `alphaPhi` (`m^3/s`).

- ```rust
  pub fn alpha_phi_mut(self: &mut Self) -> &mut SurfaceScalarField { /* ... */ }
  ```
   Phase volumetric face flux `alphaPhi` (`m^3/s`).

- ```rust
  pub fn alpha_rho_phi(self: &Self) -> &SurfaceScalarField { /* ... */ }
  ```
  Phase mass face flux `alphaRhoPhi` (`kg/s`).

- ```rust
  pub fn alpha_rho_phi_mut(self: &mut Self) -> &mut SurfaceScalarField { /* ... */ }
  ```
   Phase mass face flux `alphaRhoPhi` (`kg/s`).

- ```rust
  pub fn alpha_rho_mag_u(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Cell-centred mass flux `alphaRhoMagU` (`kg/(m^2 s)`); regime-map placeholder.

- ```rust
  pub fn alpha_rho_mag_u_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Cell-centred mass flux `alphaRhoMagU` (`kg/(m^2 s)`); regime-map placeholder.

- ```rust
  pub fn dgdt(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Dilation term `dgdt` (`1/s`) — the compressible part of continuity.

- ```rust
  pub fn dgdt_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Dilation term `dgdt` (`1/s`) — the compressible part of continuity.

- ```rust
  pub fn normalized(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Solver-normalised volume fraction (dimensionless).

- ```rust
  pub fn normalized_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Solver-normalised volume fraction (dimensionless).

- ```rust
  pub fn density(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Density `rho` (`kg/m^3`). Placeholder updated by the thermo package.

- ```rust
  pub fn density_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Density `rho` (`kg/m^3`). Placeholder updated by the thermo package.

- ```rust
  pub fn temperature(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Temperature `T` (`K`). Placeholder updated by the thermo package.

- ```rust
  pub fn temperature_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Temperature `T` (`K`). Placeholder updated by the thermo package.

- ```rust
  pub fn enthalpy(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Specific enthalpy `h` (`J/kg`). Placeholder updated by the thermo package.

- ```rust
  pub fn enthalpy_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Specific enthalpy `h` (`J/kg`). Placeholder updated by the thermo package.

- ```rust
  pub fn kappa(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Thermal conductivity `kappa` (`W/(m K)`). Placeholder updated by thermo.

- ```rust
  pub fn kappa_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Thermal conductivity `kappa` (`W/(m K)`). Placeholder updated by thermo.

- ```rust
  pub fn cp(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Specific heat `Cp` (`J/(kg K)`). Placeholder updated by thermo.

- ```rust
  pub fn cp_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Specific heat `Cp` (`J/(kg K)`). Placeholder updated by thermo.

- ```rust
  pub fn mu(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Dynamic viscosity `mu` (`Pa s`). Placeholder updated by thermo.

- ```rust
  pub fn mu_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Dynamic viscosity `mu` (`Pa s`). Placeholder updated by thermo.

- ```rust
  pub fn prandtl(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Prandtl number `Pr` (dimensionless). Placeholder updated by thermo.

- ```rust
  pub fn prandtl_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Prandtl number `Pr` (dimensionless). Placeholder updated by thermo.

- ```rust
  pub fn hydraulic_diameter(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Characteristic (hydraulic) diameter `Dh` (`m`).

- ```rust
  pub fn hydraulic_diameter_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Characteristic (hydraulic) diameter `Dh` (`m`).

- ```rust
  pub fn flow_quality(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Flow quality `X` (dimensionless); non-trivial only in two-phase.

- ```rust
  pub fn flow_quality_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Flow quality `X` (dimensionless); non-trivial only in two-phase.

- ```rust
  pub fn lockhart_martinelli(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Lockhart–Martinelli parameter `XLM` (dimensionless).

- ```rust
  pub fn lockhart_martinelli_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Lockhart–Martinelli parameter `XLM` (dimensionless).

- ```rust
  pub fn dispersion(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Dispersion marker (`0` continuous … `1` dispersed), dimensionless.

- ```rust
  pub fn dispersion_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Dispersion marker (`0` continuous … `1` dispersed), dimensionless.

- ```rust
  pub fn power_density(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Neutronics-projected power density `q'''` (`W/m^3`) deposited in the fluid.

- ```rust
  pub fn power_density_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Neutronics-projected power density `q'''` (`W/m^3`) deposited in the fluid.

- ```rust
  pub fn continuity_error(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Continuity error `contErr` (`kg/(m^3 s)`).

- ```rust
  pub fn continuity_error_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Continuity error `contErr` (`kg/(m^3 s)`).

- ```rust
  pub fn above_thermo_residual_alpha(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Marker: `1` where `alpha_norm > thermoResidualAlpha`, else `0`.

- ```rust
  pub fn above_thermo_residual_alpha_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Marker: `1` where `alpha_norm > thermoResidualAlpha`, else `0`.

- ```rust
  pub fn below_thermo_residual_alpha(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Marker: `1` where `alpha_norm <= thermoResidualAlpha`, else `0`.

- ```rust
  pub fn below_thermo_residual_alpha_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Marker: `1` where `alpha_norm <= thermoResidualAlpha`, else `0`.

- ```rust
  pub fn reference_density(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  Reference density `rho0` (`kg/m^3`), used only under the Boussinesq EOS.

- ```rust
  pub fn reference_density_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
   Reference density `rho0` (`kg/m^3`), used only under the Boussinesq EOS.

- ```rust
  pub fn min_xlm(self: &Self) -> f64 { /* ... */ }
  ```
  Minimum allowable Lockhart–Martinelli parameter (dimensionless).

- ```rust
  pub fn max_xlm(self: &Self) -> f64 { /* ... */ }
  ```
  Maximum allowable Lockhart–Martinelli parameter (dimensionless).

- ```rust
  pub fn thermo_residual_alpha(self: &Self) -> VolumeFraction { /* ... */ }
  ```
  The `thermoResidualAlpha` threshold (dimensionless). Below it the fluid

- ```rust
  pub fn set_thermo_residual_alpha(self: &mut Self, value: VolumeFraction) { /* ... */ }
  ```
  Overwrite the `thermoResidualAlpha` threshold.

- ```rust
  pub fn is_boussinesq(self: &Self) -> bool { /* ... */ }
  ```
  Whether this phase uses the Boussinesq equation of state.

- ```rust
  pub fn set_boussinesq(self: &mut Self, value: bool) { /* ... */ }
  ```
  Set the Boussinesq flag (set by the thermo package after reading the EOS).

- ```rust
  pub fn rho(self: &Self, variable_if_boussinesq: bool) -> &VolScalarField { /* ... */ }
  ```
  The Boussinesq-sensitive density.

- ```rust
  pub fn cumulative_continuity_error(self: &Self) -> Mass { /* ... */ }
  ```
  Domain-integrated continuity error `cumulContErr` (`kg`).

- ```rust
  pub fn set_cumulative_continuity_error(self: &mut Self, value: Mass) { /* ... */ }
  ```
  Overwrite the integrated continuity error.

- ```rust
  pub fn correct_mag_velocity(self: &mut Self) { /* ... */ }
  ```
  Recompute the velocity magnitude field: `magU = |U|` per cell.

- ```rust
  pub fn correct_alpha_rho_mag_u(self: &mut Self) { /* ... */ }
  ```
  Recompute the cell-centred mass flux `alphaRhoMagU = alpha * rho * magU`.

- ```rust
  pub fn correct_thermo_residual_markers(self: &mut Self) { /* ... */ }
  ```
  Recompute the thermo-residual marker fields from `normalized` and the

- ```rust
  pub fn nu(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Kinematic viscosity `nu = mu / rho` (`m^2/s`) as a fresh field.

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
    fn clone(self: &Self) -> Fluid { /* ... */ }
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
## Module `phase_base`

# `phase_base` — shared per-phase field foundation

Rust analogue of GeN-Foam's `Foam::phaseBase`, the abstract base every
phase (the [`super::fluid::Fluid`] and the solid `structure`) inherits from.
Upstream `phaseBase` **is** a `volScalarField` — the phase **volume
fraction** `alpha` — plus three scalars: the phase name, a reference to the
mesh, and the *residual* volume fraction used to stabilise the phase
momentum equation as `alpha -> 0`.

Here that is a plain composition ([`PhaseBase`] owns the `alpha`
[`VolScalarField`]) rather than inheritance, since Rust has no field-type
inheritance and the workspace forbids trait-object dispatch. A [`Fluid`]
(and, later, a solid structure) embeds a `PhaseBase` and forwards to it.

[`Fluid`]: super::fluid::Fluid

## Units

[`outram_foam_basic_lib`]'s [`VolScalarField`] stores bare `f64` per cell, so
the `alpha` field is dimensionless-by-convention (a volume fraction, `0..=1`).
The scalar [`PhaseBase::residual_alpha`] that crosses the public API is a
[`uom`]-typed [`VolumeFraction`] so a reader hovering in their editor sees a
dimensionless ratio, not a bare number.

```rust
pub mod phase_base { /* ... */ }
```

### Types

#### Type Alias `VolumeFraction`

Phase **volume fraction** `alpha` — **dimensionless** (`0..=1`).

The fraction of a cell's volume occupied by this phase. Named alias over
[`uom`]'s [`Ratio`]; used for the scalar volume-fraction parameters that
cross the public API ([`PhaseBase::residual_alpha`],
[`super::fluid::Fluid::thermo_residual_alpha`]). The per-cell `alpha`
**field** itself is a bare-`f64` [`VolScalarField`] (basic-lib convention),
documented as a volume fraction.

```rust
pub type VolumeFraction = uom::si::f64::Ratio;
```

#### Struct `PhaseBase`

The shared per-phase state common to the fluid and structure phases.

Port of `Foam::phaseBase`. Owns the phase **volume-fraction field**
`alpha` (one dimensionless value per cell), the phase **name**, and the
**residual volume fraction** used to keep the phase momentum equation
well-posed as `alpha -> 0`.

The mesh is shared read-only (`Arc<FvMesh>`); the `alpha` field carries its
own clone of that `Arc`, matching the rest of the field state in
[`super::fluid::Fluid`].

```rust
pub struct PhaseBase {
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
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(mesh: Arc<FvMesh>, name: impl Into<String>) -> Self { /* ... */ }
  ```
  Allocate a phase base on `mesh` with a uniform `alpha` of zero and the

- ```rust
  pub fn name(self: &Self) -> &str { /* ... */ }
  ```
  The phase name.

- ```rust
  pub fn mesh(self: &Self) -> &Arc<FvMesh> { /* ... */ }
  ```
  The mesh all fields of this phase are defined on.

- ```rust
  pub fn alpha(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  The phase volume-fraction field `alpha` (dimensionless, `0..=1`).

- ```rust
  pub fn alpha_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
  ```
  Mutable access to the phase volume-fraction field `alpha`.

- ```rust
  pub fn residual_alpha(self: &Self) -> VolumeFraction { /* ... */ }
  ```
  The residual volume fraction (dimensionless), the floor that keeps the

- ```rust
  pub fn set_residual_alpha(self: &mut Self, value: VolumeFraction) { /* ... */ }
  ```
  Overwrite the residual volume fraction.

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
    fn clone(self: &Self) -> PhaseBase { /* ... */ }
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

#### Function `volume_fraction`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Build a [`VolumeFraction`] from a plain (dimensionless) magnitude.

```rust
pub fn volume_fraction(value: f64) -> VolumeFraction { /* ... */ }
```

## Module `phase_pair`

# `phase_pair` — pairwise dimensionless numbers (Reynolds, relative velocity)

Port of the **kinematic core** of GeN-Foam's `FSPair` (fluid–structure) and
`FFPair` (fluid–fluid) classes: the pairwise **Reynolds number** and the
fluid–fluid **relative-velocity magnitude**. As the upstream `FSPair`
documentation puts it, a Reynolds number *"cannot be defined for a fluid by
itself, it needs to be geometrically constrained in a domain (so that a
hydraulic diameter can be defined)"* — hence it lives on the phase **pair**,
not the [`Fluid`] itself.

[`Fluid`]: super::fluid::Fluid

## Scope: kinematics only, no closure tables

The full upstream `FSPair`/`FFPair` also assemble the drag tensor `Kd`, the
heat-transfer coefficient `htc`, the contact-partition fraction, interfacial
area, virtual mass, etc. — every one of which is a *closure* (owned by the
`closures` sub-modules) or *solver* assembly step. Those are **out of scope
here**; this module ports only the phase-state-derived dimensionless numbers
the closures consume as inputs. Each is a pure per-cell function of already-
assembled fields, so it is independently verifiable against a hand
calculation.

## Definitions

Fluid–structure isotropic Reynolds number (`FSPair::correct`):

```text
Re_FS = max( alpha_norm * |U| * D_h / nu ,  Re_min )
```

Fluid–fluid relative velocity and Reynolds number (`FFPair::correct`):

```text
|U_r| = |U_1 - U_2|
Re_FF = max( |U_r| * D_h,disp / nu_cont ,  Re_min )
```

where `alpha_norm` is the [`Fluid::normalized`] volume fraction, `nu = mu/rho`
the kinematic viscosity ([`Fluid::nu`]), `D_h` the hydraulic diameter, and
`D_h,disp` / `nu_cont` the dispersed-phase diameter / continuous-phase
kinematic viscosity that `FFPair` blends from the two fluids.

[`Fluid::normalized`]: super::fluid::Fluid::normalized
[`Fluid::nu`]: super::fluid::Fluid::nu

```rust
pub mod phase_pair { /* ... */ }
```

### Functions

#### Function `fs_reynolds`

**Attributes:**

- `MustUse { reason: None }`

Fluid–structure isotropic Reynolds number field
`Re = max(alpha_norm * |U| * D_h / nu, Re_min)`.

Port of `FSPair::correct`'s `Re_ = fluid.normalized()*mag(U)*Dh/nu` followed
by `Re_ = max(Re_, minRe_)`. All four input fields are per-cell bare `f64`
(see the [`super::fluid`] unit table); `min_re` is a [`uom`]-typed
[`ReynoldsNumber`] floor. The result is a fresh [`VolScalarField`] named
`Re` on the same mesh as `normalized_alpha`.

# Panics
Debug-asserts every input shares the mesh cell count of `normalized_alpha`.

```rust
pub fn fs_reynolds(normalized_alpha: &outram_foam_basic_lib::fields::vol_field::VolScalarField, mag_u: &outram_foam_basic_lib::fields::vol_field::VolScalarField, dh: &outram_foam_basic_lib::fields::vol_field::VolScalarField, nu: &outram_foam_basic_lib::fields::vol_field::VolScalarField, min_re: crate::genfoam::thermal_hydraulics::units::ReynoldsNumber) -> outram_foam_basic_lib::fields::vol_field::VolScalarField { /* ... */ }
```

#### Function `ff_relative_velocity_magnitude`

**Attributes:**

- `MustUse { reason: None }`

Fluid–fluid relative-velocity magnitude field `|U_r| = |U_1 - U_2|` (`m/s`).

Port of `FFPair::correct`'s `magUr_ = mag(U1 - U2)`. Result is a fresh
[`VolScalarField`] named `magUr` on the same mesh as `u1`.

# Panics
Debug-asserts `u1` and `u2` share a cell count.

```rust
pub fn ff_relative_velocity_magnitude(u1: &outram_foam_basic_lib::fields::vol_field::VolVectorField, u2: &outram_foam_basic_lib::fields::vol_field::VolVectorField) -> outram_foam_basic_lib::fields::vol_field::VolScalarField { /* ... */ }
```

#### Function `ff_reynolds`

**Attributes:**

- `MustUse { reason: None }`

Fluid–fluid Reynolds number field
`Re = max(|U_r| * D_h,disp / nu_cont, Re_min)`.

Port of `FFPair::correct`'s
`Re_ = max(magUr_*DhDispersed_/nuContinuous_, minRe_)`. `mag_ur` is typically
the output of [`ff_relative_velocity_magnitude`]; `dh_dispersed` is the
dispersed-phase hydraulic diameter and `nu_continuous` the continuous-phase
kinematic viscosity (`m^2/s`). Result is a fresh [`VolScalarField`] named
`Re` on the same mesh as `mag_ur`.

# Panics
Debug-asserts every input shares the cell count of `mag_ur`.

```rust
pub fn ff_reynolds(mag_ur: &outram_foam_basic_lib::fields::vol_field::VolScalarField, dh_dispersed: &outram_foam_basic_lib::fields::vol_field::VolScalarField, nu_continuous: &outram_foam_basic_lib::fields::vol_field::VolScalarField, min_re: crate::genfoam::thermal_hydraulics::units::ReynoldsNumber) -> outram_foam_basic_lib::fields::vol_field::VolScalarField { /* ... */ }
```

### Re-exports

#### Re-export `Fluid`

```rust
pub use fluid::Fluid;
```

#### Re-export `StateOfMatter`

```rust
pub use fluid::StateOfMatter;
```

#### Re-export `volume_fraction`

```rust
pub use phase_base::volume_fraction;
```

#### Re-export `PhaseBase`

```rust
pub use phase_base::PhaseBase;
```

#### Re-export `VolumeFraction`

```rust
pub use phase_base::VolumeFraction;
```

#### Re-export `ff_relative_velocity_magnitude`

```rust
pub use phase_pair::ff_relative_velocity_magnitude;
```

#### Re-export `ff_reynolds`

```rust
pub use phase_pair::ff_reynolds;
```

#### Re-export `fs_reynolds`

```rust
pub use phase_pair::fs_reynolds;
```

## Module `solver`

# `genfoam::thermal_hydraulics::solver` — porous TH equation drivers

The one-phase and (planned) two-phase thermal-hydraulics solver drivers: the
porous momentum equation (`UEqn`, with an anisotropic drag tensor `Kd`
assembled from fluid-structure friction closures plus tortuosity-modified
turbulent diffusion), the porous energy equation (`EEqn`, coupled to the
structure via the heat-transfer coefficient), the PIMPLE pressure equation
(`pEqn`), and — for two-phase — MULES-limited `alpha` transport. It reuses
basic-lib `fvm`/`fvc` assembly and the crate's existing PIMPLE scaffolding
(`src/solvers/rho_pimple_foam.rs`). The closure correlations
([`super::closures`]) and the field state ([`super::phase`]/[`super::structure`])
do NOT belong here — this module wires them into the equation loops.

Ports upstream `src/classes/thermalHydraulics/solvers/**` (the `onePhase`,
`onePhaseLegacy`, and `twoPhase` top-level solver drivers).

## Module map

| Sub-module | Provides | Upstream | Status |
|---|---|---|---|
| [`porous_drag`] | [`PorousDrag`] — the isotropic `Kd` drag-coefficient assembly from the wall-friction closure | `physicsModels/dragModels/FSDragFactor` | **Ported + V&V** |
| [`one_phase`] | [`OnePhaseSolver`] — porous `UEqn`/`pEqn`/`EEqn` driver | `solvers/onePhase/**` | **Ported + V&V** (constant-property slice; see its docs) |

## Dispatch: no trait objects

Per the workspace no-`dyn` rule the solver family is a closed enum
([`ThermalHydraulicsSolver`]) dispatched by value. Only [`OnePhaseSolver`] is
implemented so far; `onePhaseLegacy` and the two-phase (`twoPhase`, MULES
`alpha` transport + two-phase `pEqn`) drivers are tracked in beads
op-p6p.7.11 (follow-up) and op-p6p.7.12 and will each add a variant, at which
point every `match` on this enum becomes a compile error until updated — the
exhaustiveness the rule buys.

```rust
pub mod solver { /* ... */ }
```

### Modules

## Module `one_phase`

# `solver::one_phase` — porous single-phase thermal-hydraulics driver

Rust port of GeN-Foam's `solvers/onePhase` — the single-fluid, one-stationary-
structure porous-medium Eulerian solver. It marches, per time step, a
**porous momentum** predictor/corrector and a **porous energy** equation
coupled to the solid structure:

- **`UEqn`** (`UEqn_1p.H`): `ddt(alpha rho, U) + div(alphaRhoPhi, U)
  - laplacian(alpha rho nuEff, U) + Kd . U = -grad(p) + body forces`, where
  the anisotropic drag `Kd` is assembled by [`super::porous_drag`] from the
  fluid-structure friction closure. Here only the **isotropic** `Kd` is
  wired (`Kd = Kd * I`), so the drag term is the implicit `fvm::sp(Kd, U)`
  in upstream's `Sp((1/3) tr(Kd), U) + (dev(Kd) & U)` (the deviatoric part
  vanishes for isotropic `Kd`).
- **`pEqn`** (`pEqn_1p.H`): an incompressible-porous PIMPLE pressure
  corrector (`rAU`/`HbyA`/flux reconstruction) built on basic-lib operators,
  reusing the proven structure of [`crate::solvers::rho_pimple_foam`].
- **`EEqn`** (`EEqn_1p.H`): `ddt(alpha rho, he) + div(alphaRhoPhi, he)
  - laplacian(alpha alphaEff, he) = q_struct + q'''`, with the structure
  heat exchange linearised semi-implicitly (a `Su`/`Sp` pair) exactly as
  `structure::linearizedSemiImplicitHeatSource`.

## Honest scope

This driver runs with **constant fluid properties** supplied on the
[`Fluid`] fields (`rho`, `mu`, `Cp`, `kappa`) and treats the enthalpy as
`he = Cp * T` about `T = 0`. The reason is wiring, not absence: the bespoke
hydrogen thermophysical package in [`super::super::thermophysical`] *is*
ported (EOS, thermodynamics, viscosity, conductivity), but this driver does
not yet call it as its `he <-> T` / `rho(p,T)` package. That
is sufficient for the incompressible porous momentum/energy physics that is
the ported slice's purpose (friction pressure drop, structure heat coupling)
and is what the V&V exercises. The structure side is wired as a
fixed-surface-temperature convective coupling (a field-wide analogue of
GeN-Foam's `fixedTemperature` structure model); driving a full
[`super::super::structure::StructureCell`] fuel-pin conduction field is the
next slice (bead op-p6p.7.11 follow-up).

## Wiring of the consumed models

| Consumed | Where it enters |
|---|---|
| [`Fluid`] field state (`U`, `phi`, `rho`, `he`, `alpha`, `Dh`, `mu`) | owned by the solver, read/written each step |
| [`super::porous_drag::PorousDrag`] ([`super::super::closures::fs_drag`]) | `Kd` assembly → `UEqn` drag term |
| structure coupling fields (`wall_htc`, `wall_area`, `wall_temperature`) | `EEqn` linearised heat source |

```rust
pub mod one_phase { /* ... */ }
```

### Types

#### Struct `OnePhaseSolver`

Porous single-phase thermal-hydraulics solver (`onePhase`).

Owns one [`Fluid`] phase, the pressure field, the porous-drag closure, and
the structure convective-coupling fields, and advances them with
[`OnePhaseSolver::step`]. Construct with [`OnePhaseSolver::new`] and set the
initial/boundary fields through the public accessors before stepping.

```rust
pub struct OnePhaseSolver {
    pub fluid: crate::genfoam::thermal_hydraulics::phase::fluid::Fluid,
    pub p: VolScalarField,
    pub drag: super::porous_drag::PorousDrag,
    pub nu_eff: VolScalarField,
    pub alpha_eff: VolScalarField,
    pub cp: VolScalarField,
    pub wall_htc: VolScalarField,
    pub wall_area: VolScalarField,
    pub wall_temperature: VolScalarField,
    pub body_force: Vector3,
    pub solve_pressure: bool,
    pub p_ref_cell: usize,
    pub p_ref_value: f64,
    pub n_outer: usize,
    pub n_inner: usize,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fluid` | `crate::genfoam::thermal_hydraulics::phase::fluid::Fluid` | The single fluid phase (velocity, flux, thermo placeholders, `alpha`). |
| `p` | `VolScalarField` | Static pressure `p` [Pa]. |
| `drag` | `super::porous_drag::PorousDrag` | Fluid-structure porous drag closure (assembles `Kd`). |
| `nu_eff` | `VolScalarField` | Effective kinematic viscosity `nuEff` [m^2/s] for the momentum diffusion<br>term (molecular + turbulent). Constant-property placeholder. |
| `alpha_eff` | `VolScalarField` | Effective thermal diffusivity `alphaEff = kappa/Cp` [kg/(m s)] for the<br>energy diffusion term. |
| `cp` | `VolScalarField` | Constant specific heat `Cp` [J/(kg K)] relating `he = Cp * T`. |
| `wall_htc` | `VolScalarField` | Structure surface heat-transfer coefficient `h` [W/(m^2 K)]. |
| `wall_area` | `VolScalarField` | Structure interfacial area density `a_v` [1/m]. |
| `wall_temperature` | `VolScalarField` | Structure surface temperature `T_wall` [K] (fixed-temperature coupling). |
| `body_force` | `Vector3` | Explicit body force per unit volume `f_body` [N/m^3] (e.g. an imposed<br>pressure gradient or a gravity-driven head), added to the momentum RHS. |
| `solve_pressure` | `bool` | Whether to run the pressure corrector. For a spatially uniform, gradient-<br>free driving (the drag/energy V&V), the pressure field is inert and this<br>can be `false` to isolate the momentum/energy physics. |
| `p_ref_cell` | `usize` | Reference cell for the (singular, incompressible) pressure system. |
| `p_ref_value` | `f64` | Reference pressure value [Pa] pinned at `p_ref_cell`. |
| `n_outer` | `usize` | PIMPLE outer correctors. |
| `n_inner` | `usize` | PISO inner (pressure) correctors. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, fluid: Fluid, drag: PorousDrag) -> Self { /* ... */ }
  ```
  Allocate a one-phase solver on `mesh` with the given fluid and drag

- ```rust
  pub fn mesh(self: &Self) -> &Arc<FvMesh> { /* ... */ }
  ```
  The mesh this solver runs on.

- ```rust
  pub fn temperature(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Fluid temperature field `T = he / Cp` [K], recomputed from the current

- ```rust
  pub fn step(self: &mut Self, dt: f64) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the coupled porous momentum + energy system one time step `dt`

- ```rust
  pub fn correct_fluid_mechanics(self: &mut Self, dt: f64) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Solve the porous momentum equation (and, if enabled, the pressure

- ```rust
  pub fn correct_energy(self: &mut Self, dt: f64) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Solve the porous energy equation, updating the fluid enthalpy.

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
## Module `porous_drag`

# `solver::porous_drag` — fluid-structure drag-tensor assembly

Assembles the porous-medium **fluid-structure drag coefficient** `Kd` that
the momentum equation ([`super::one_phase`]) sinks velocity through. This is
the field-wide side of GeN-Foam's `FSPair::Kd()`: the [`super::super::closures::fs_drag`]
family gives the *scalar* Darcy friction factor `f(Re)` per cell; this module
turns that into the per-cell drag coefficient the solver assembles into
`UEqn`.

## The isotropic drag coefficient

Port of `FSDragFactor::correctField` (isotropic branch). For each cell the
diagonal drag coefficient is

```text
Kd = 0.5 / Dh * (1 - alpha_s) * rho * max(|U|, minMagU) * f(Re)
```

where `alpha_s` is the **structure** volume fraction (so `1 - alpha_s` is the
open/fluid fraction `alpha_f`), `Dh` the hydraulic diameter, `rho` the fluid
density, `|U|` the velocity magnitude, and `f` the Darcy friction factor from
the wall-friction closure. The momentum sink assembled from it is `Kd * U`
(an implicit `fvm::sp` term), i.e. the Darcy-Weisbach body force
`f/Dh * rho |U| U / 2` scaled by the open fraction.

Only the **isotropic** (`Kd = Kd * I`) case is ported here; the anisotropic
(localX/Y/Z) branch of `FSDragFactor` is a documented follow-up.

## Units

`Kd` has dimensions `kg / (m^3 s)` (a volumetric drag coefficient): `Kd * U`
is a force per unit volume `N/m^3 = Pa/m`, balancing a pressure gradient. It
is stored in a bare-`f64` [`VolScalarField`] following the basic-lib field
convention; the scalar tuning parameters that cross the public API carry
their [`uom`] types.

```rust
pub mod porous_drag { /* ... */ }
```

### Types

#### Struct `PorousDrag`

Configuration of the isotropic fluid-structure porous drag.

Holds the wall-friction closure ([`FsWallFriction`], the correlation that
maps a Reynolds number to a Darcy friction factor) plus the two numerical
floors upstream applies while assembling `Kd`: a minimum velocity magnitude
`minMagU` (keeps the drag finite as `U -> 0`) and a minimum Reynolds number
(keeps `f(Re)` off its `Re -> 0` singularity).

```rust
pub struct PorousDrag {
    pub friction: crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction,
    pub min_mag_u: f64,
    pub min_reynolds: crate::genfoam::thermal_hydraulics::units::ReynoldsNumber,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `friction` | `crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction` | The fluid-structure wall-friction correlation `f(Re)`. |
| `min_mag_u` | `f64` | Minimum velocity magnitude `minMagU` [m/s] used in the drag coefficient<br>(upstream `PIMPLE/minMagU`, default `0`). A small positive value avoids<br>a vanishing drag at stagnation. |
| `min_reynolds` | `crate::genfoam::thermal_hydraulics::units::ReynoldsNumber` | Reynolds-number floor for the friction-factor evaluation (dimensionless).<br>Upstream typically clamps at `Re = 1` to avoid the `1/Re` laminar<br>singularity; default here `1.0`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(friction: FsWallFriction) -> Self { /* ... */ }
  ```
  Build a porous drag from a wall-friction closure, with `minMagU = 0` and

- ```rust
  pub fn with_min_mag_u(self: Self, min_mag_u: f64) -> Self { /* ... */ }
  ```
  Set the minimum velocity magnitude `minMagU` [m/s].

- ```rust
  pub fn assemble_kd(self: &Self, fluid: &Fluid) -> VolScalarField { /* ... */ }
  ```
  Assemble the isotropic drag-coefficient field `Kd` [kg/(m^3 s)] from the

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
    fn clone(self: &Self) -> PorousDrag { /* ... */ }
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
### Types

#### Enum `ThermalHydraulicsSolver`

Closed-enum dispatch over the porous thermal-hydraulics solver drivers.

A [`ThermalHydraulicsSolver`] wraps one concrete driver; [`Self::step`]
forwards to it. Adding the two-phase driver (bead op-p6p.7.12) means adding a
`TwoPhase` variant here — every existing `match` then fails to compile until
it handles the new case, which is exactly why this is an enum and not a
`dyn` trait object.

```rust
pub enum ThermalHydraulicsSolver {
    OnePhase(OnePhaseSolver),
}
```

##### Variants

###### `OnePhase`

Single-fluid + one stationary structure porous Eulerian solver
(`solvers/onePhase`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `OnePhaseSolver` |  |

##### Implementations

###### Methods

- ```rust
  pub fn step(self: &mut Self, dt: f64) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the wrapped driver one time step of `dt` seconds.

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
### Re-exports

#### Re-export `OnePhaseSolver`

```rust
pub use one_phase::OnePhaseSolver;
```

#### Re-export `PorousDrag`

```rust
pub use porous_drag::PorousDrag;
```

## Module `structure`

# `genfoam::thermal_hydraulics::structure` — mesh-unresolved solid structure

The **solid side of the porous two-field formulation**: the fuel pins,
cladding, and grid hardware that occupy the complementary volume of each
porous cell but are not resolved by the mesh. This module owns the
structure's *thermal* state (surface temperature, stored heat) and its power
source, and it exchanges heat with the fluid phase through the per-cell
convective closure. It does **not** own the fluid-phase field state (that is
the [`phase`](super::phase) module) nor the correlation leaves (those are
[`closures`](super::closures)).

Ported upstream:
`src/classes/thermalHydraulics/src/phaseModels/structureModels/**`.

## Module map

| Submodule | Ports | What it provides |
|---|---|---|
| [`units`] | — | Named `uom` aliases: [`PowerDensity`], [`VolumetricHeatCapacity`], [`InterfacialAreaDensity`], [`WallConductance`] (+ re-exported [`HeatTransferCoefficient`], [`HeatFlux`]) |
| [`power_model`] | `powerModel/{fixedPower,fixedTemperature}` | [`PowerModel`] enum — the structure's internal power source and its lumped surface-temperature update |
| [`heat_source`] | `structure.C` heat terms | Explicit fluid ⟷ structure source, wall heat flux, the inert [`PassiveStructure`] |
| [`pump`] | `pump` | [`Pump`] momentum source with time scaling |
| [`heat_exchanger`] | `heatExchanger` | [`HeatExchanger`] two-sided wall-temperature solve |
| [`power_off`] | `powerOffCriterionModels/{timer,fieldValue}` | [`PowerOffCriterion`] SCRAM criteria |

[`StructureCell`] (below) is the small aggregate that wires a power model,
an optional passive sub-structure, an optional pump, and an optional
power-off criterion into a single per-cell object and advances them together
— the direct, self-contained analogue of one cell's worth of
`structure::correct` + `structure::explicitHeatSource`.

## Design boundary — what belongs to the porous-solver bead

Everything here is **per-cell / mesh-free algebra plus owned lumped state**.
The genuinely mesh-coupled machinery of GeN-Foam's `structure` — the
anisotropic tortuosity / local-frame rotation tensors, the mesh-to-mesh
heat-exchanger mapping, the semi-implicit `fvm::Sp` enthalpy-source matrix,
and the assembly of these per-cell quantities into the porous `UEqn`/`EEqn`
— is deferred to the porous one-phase solver bead (op-p6p.7.11), mirroring
how [`closures::fs_drag`](super::closures::fs_drag) ports the friction
correlations but leaves the drag-tensor assembly to the solver. The
higher-fidelity radial-conduction pin/pebble models and the FMU couplings are
separate follow-ups (see [`power_model`]).

```rust
pub mod structure { /* ... */ }
```

### Modules

## Module `heat_exchanger`

# `heat_exchanger` — two-sided wall coupling of primary and secondary fluids

Port of the **flux-conservation wall-temperature solve** at the heart of
GeN-Foam's `heatExchanger::correct`. A heat exchanger couples a primary and a
secondary fluid region across a conducting tube wall of series conductance
`H_w = k_wall / t_wall` ([`WallConductance`]). GeN-Foam requires the heat
flux through the wall to equal the convective flux into each fluid, giving a
2×2 linear system for the two wall-surface temperatures `T_p`, `T_s`:

```text
T_s = A_p*T_p - B_p ,   T_p = A_s*T_s - B_s
```

with, on each side `i` (`p` or `s`),

```text
A_i = (H_w + h_sum_i) / H_w        (dimensionless)
B_i = ht_sum_i / H_w               (temperature)
```

where `h_sum_i`/`ht_sum_i` are that side's fluid-coupling sums (the same
`H`/`HT` pair used everywhere else in this module — see
[`super::heat_source`]). The closed-form solution is

```text
T_p = (B_p*A_s + B_s) / (A_p*A_s - 1)
T_s = (B_s*A_p + B_p) / (A_s*A_p - 1)
```

guarded against the degenerate `A_p*A_s = 1` case (both convective
coefficients zero ⇒ no heat exchanged) exactly as upstream, with a `1e-69`
floor on the denominator.

## Scope

[`hx_wall_temperature`] ports this per-cell algebra, which is self-contained
and hand-verifiable. GeN-Foam additionally handles the **mesh-to-mesh
mapping** between (possibly non-conformal) primary and secondary meshes to
find the "opposite" cell's `h_sum`/`ht_sum`; that mapping is inseparable from
the finite-volume mesh machinery and belongs to the solver bead
(op-p6p.7.11). Here the caller supplies both sides' already-mapped coupling
sums.

```rust
pub mod heat_exchanger { /* ... */ }
```

### Types

#### Struct `HeatExchanger`

A heat-exchanger wall: its series conductance.

Carries the tube-wall conductance `H_w = k_wall / t_wall`. Evaluate the
wall-surface temperatures with [`HeatExchanger::wall_temperature`], which
wraps [`hx_wall_temperature`].

```rust
pub struct HeatExchanger {
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
  pub fn new(wall_conductance: WallConductance) -> Self { /* ... */ }
  ```
  Build a heat exchanger from its wall conductance `H_w = k_wall/t_wall`.

- ```rust
  pub fn wall_conductance(self: &Self) -> WallConductance { /* ... */ }
  ```
  Wall conductance `H_w` [W/(m^2 K)].

- ```rust
  pub fn wall_temperature(self: &Self, h_sum_primary: HeatTransferCoefficient, ht_sum_primary: HeatFlux, h_sum_secondary: HeatTransferCoefficient, ht_sum_secondary: HeatFlux) -> ThermodynamicTemperature { /* ... */ }
  ```
  The **primary-side** wall-surface temperature `T_p` [K] for this cell.

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
    fn clone(self: &Self) -> HeatExchanger { /* ... */ }
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
    fn eq(self: &Self, other: &HeatExchanger) -> bool { /* ... */ }
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
### Functions

#### Function `hx_wall_temperature`

**Attributes:**

- `MustUse { reason: None }`

Primary-side heat-exchanger wall-surface temperature `T_p` [K].

Port of the primary loop of `heatExchanger::correct`:
`T_p = (B_p*A_s + B_s) / max(A_p*A_s - 1, 1e-69)` (see the [module
documentation](self) for the derivation and the `A_i`, `B_i` definitions).
The `1e-69` floor reproduces upstream's guard against the no-heat-transfer
degeneracy where both convective coefficients vanish.

```rust
pub fn hx_wall_temperature(wall_conductance: super::units::WallConductance, h_sum_primary: super::units::HeatTransferCoefficient, ht_sum_primary: super::units::HeatFlux, h_sum_secondary: super::units::HeatTransferCoefficient, ht_sum_secondary: super::units::HeatFlux) -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

## Module `heat_source`

# `heat_source` — fluid ⟷ structure convective heat coupling

Port of the per-cell heat-exchange algebra in GeN-Foam's `structure.C`: the
**explicit** volumetric heat source the structure feeds to the fluid energy
equation, the diagnostic wall heat flux, and the lumped energy balance of the
**passive** (inert, non-powered) sub-structure.

Throughout, the fluid coupling arrives as the two per-cell sums GeN-Foam
passes into `structure::correct`:

```text
h_sum  = sum_j frac_j * htc_j          [W/(m^2 K)]   (== H  upstream)
ht_sum = sum_j frac_j * htc_j * T_j    [W/m^2]       (== HT upstream)
```

For a single-phase solver these collapse to `h_sum = htc`,
`ht_sum = htc * T_fluid`; the two-phase generalisation (a
fluid-partition-weighted sum over phases) uses the *same* formulas, which is
why the port takes the pre-assembled sums rather than individual phases.

## What is here vs. what is the solver's job

The **explicit** source `Q = a_v*(ht_sum - h_sum*T_struct)` is pure per-cell
algebra and is ported and hand-verified here. GeN-Foam also offers a
**semi-implicit** linearisation (`linearizedSemiImplicitHeatSource`) that
returns an `fvScalarMatrix` — a `(Su, Sp)` pair assembled into the enthalpy
`EEqn` with `fvm::Sp`. That matrix assembly is inseparable from the finite-
volume energy-equation machinery and therefore belongs to the porous-solver
bead (op-p6p.7.11), exactly as the anisotropic drag-tensor `Kd` assembly does
for [`super::super::closures::fs_drag`]. It is intentionally **not** ported
here; the explicit source below is the coupling term a lumped or
operator-split integrator needs.

```rust
pub mod heat_source { /* ... */ }
```

### Types

#### Struct `PassiveStructure`

The inert (passive) sub-structure of a porous cell.

GeN-Foam's `structure` carries, alongside the active power regions, an inert
sub-structure (grid, cladding, unheated hardware) with its own volume
fraction, interfacial area, and thermal inertia. It produces no power but
still stores heat and exchanges it convectively with the fluid; its surface
temperature `Tpas` evolves by the *same* lumped backward-Euler balance as a
[`super::power_model::FixedPower`] with zero source.

```rust
pub struct PassiveStructure {
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
  pub fn new(interfacial_area: InterfacialAreaDensity, alpha_rho_cp: VolumetricHeatCapacity, initial_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a passive sub-structure.

- ```rust
  pub fn surface_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  The current passive surface temperature [K].

- ```rust
  pub fn interfacial_area(self: &Self) -> InterfacialAreaDensity { /* ... */ }
  ```
  Interfacial-area density `a_v` [1/m].

- ```rust
  pub fn correct(self: &mut Self, h_sum: HeatTransferCoefficient, ht_sum: HeatFlux, dt: uom::si::f64::Time) -> ThermodynamicTemperature { /* ... */ }
  ```
  Advance the passive surface temperature one backward-Euler step.

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
    fn clone(self: &Self) -> PassiveStructure { /* ... */ }
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
    fn eq(self: &Self, other: &PassiveStructure) -> bool { /* ... */ }
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
### Functions

#### Function `explicit_heat_source`

**Attributes:**

- `MustUse { reason: None }`

Explicit fluid-structure heat source for one cell — **W / m^3**.

Port of the per-cell term in `structure::explicitHeatSource`:

```text
Q = a_v * (ht_sum - h_sum * T_struct)
  = a_v * h_sum * (T_fluid - T_struct)     (single phase)
```

Sign convention (matching the fluid energy equation): `Q` is the power per
unit volume delivered **to the fluid**. It is positive when the fluid is
colder than the structure (`ht_sum > h_sum*T_struct`, i.e. the structure
heats the fluid) and negative when the fluid is hotter (a heat exchanger /
cold structure removing energy).

# Parameters
- `interfacial_area` — `a_v` [1/m] of the exchanging structure region.
- `h_sum` — the coupling coefficient [W/(m^2 K)].
- `ht_sum` — the coupling `h*T` product [W/m^2].
- `t_struct` — the structure surface temperature [K].

```rust
pub fn explicit_heat_source(interfacial_area: super::units::InterfacialAreaDensity, h_sum: super::units::HeatTransferCoefficient, ht_sum: super::units::HeatFlux, t_struct: uom::si::f64::ThermodynamicTemperature) -> super::units::PowerDensity { /* ... */ }
```

#### Function `wall_heat_flux`

**Attributes:**

- `MustUse { reason: None }`

Diagnostic wall heat flux for one cell — **W / m^2**.

Port of `heatFlux_[celli] = H*Twall - HT` in `structure::correct`
(`Twall` = the structure surface temperature). Positive means heat flows
**from the structure to the fluid** across the wall. Used by GeN-Foam mostly
for post-processing (and by the Shah pool-boiling closure).

```text
q'' = h_sum * T_wall - ht_sum
```

```rust
pub fn wall_heat_flux(h_sum: super::units::HeatTransferCoefficient, ht_sum: super::units::HeatFlux, t_wall: uom::si::f64::ThermodynamicTemperature) -> super::units::HeatFlux { /* ... */ }
```

## Module `power_model`

# `power_model` — the solid-structure energy source / surface temperature

Port of GeN-Foam's `powerModel` family. A power model describes the
**thermal state of the mesh-unresolved solid structure** in a porous cell:
it owns the structure surface temperature and, each timestep, updates that
temperature from (a) the internal power it produces and (b) the convective
heat it exchanges with the surrounding fluid. That surface temperature is
then what the fluid-structure heat closure sees (see [`super::heat_source`]).

## Model set (closed enum, no `dyn` dispatch)

| Variant | Upstream | Physics |
|---|---|---|
| [`PowerModel::FixedPower`] | `fixedPower` | Prescribed volumetric power; surface temperature from a transient **lumped** energy balance |
| [`PowerModel::FixedTemperature`] | `fixedTemperature` | Prescribed surface temperature (constant or time-scaled); no energy balance |

The higher-fidelity pin/pebble/FMU models upstream
(`heatedPin`, `nuclearFuelPin`, `interpolatedNuclearFuelPin`,
`nuclearSteadyStatePebble`, `nuclearFuelFMU`, `fixedTemperatureFMU`) solve a
**radial (1-D) conduction** problem inside the pin/pebble each timestep and
are *not* ported here — that radial-conduction machinery is a substantial
follow-up (its own bead) and the FMU variants couple to an external
functional-mock-up unit that is out of scope for this crate. The two lumped
models above are the self-contained, hand-verifiable core of the family.

## Lumped energy balance (the `fixedPower` core)

GeN-Foam solves, cell by cell, a **backward-Euler** update of the structure
surface temperature `T` (`fixedPower::correct`). Writing `C = alpha*rho*Cp`
(the [`VolumetricHeatCapacity`]), `a_v` the [`InterfacialAreaDensity`],
`q'''` the [`PowerDensity`], `alpha` the power-region volume fraction, and
the fluid coupling as the two sums

```text
h_sum  = sum_j frac_j * htc_j          [W/(m^2 K)]   (== H  upstream)
ht_sum = sum_j frac_j * htc_j * T_j    [W/m^2]       (== HT upstream)
```

the update is

```text
           a_v*ht_sum + alpha*q''' + (C/dt)*T_old
  T_new = ---------------------------------------
                  C/dt + a_v*h_sum
```

This is the discretised form of
`d/dt (C*T) = alpha*q''' + a_v*(ht_sum - h_sum*T)`, i.e. thermal-inertia
storage on the left, internal power plus net convective exchange on the
right. In the steady limit (`dt -> inf`) it reduces to the algebraic balance
`alpha*q''' = a_v*(h_sum*T - ht_sum)`, the case exercised by the V&V test.

## Time dependence

Both models accept an optional [`TimeProfile`] read as a **scaling factor**
relative to the initial value (GeN-Foam's `powerTimeProfile` /
`temperatureTimeProfile`): the instantaneous value is
`value_initial * profile(t)`. Upstream expresses this as the algebraically
equivalent ratio recurrence `v(t) = v(t-dt)*profile(t)/profile(t-dt)`
(`powerUpdate`); here we evaluate the closed form directly against the stored
initial value, which avoids the recurrence's division-by-zero edge when a
profile passes through zero.

```rust
pub mod power_model { /* ... */ }
```

### Types

#### Struct `FixedPower`

A `fixedPower` structure: prescribed internal power, transient lumped
surface temperature.

Owns the structure surface temperature as evolving state; call
[`FixedPower::correct`] once per timestep with the current fluid coupling to
advance it. Construct with [`FixedPower::new`] (optionally chaining
[`FixedPower::with_power_profile`]).

```rust
pub struct FixedPower {
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
  pub fn new(alpha: Ratio, interfacial_area: InterfacialAreaDensity, power_density_initial: PowerDensity, alpha_rho_cp: VolumetricHeatCapacity, initial_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a fixed-power model at an initial surface temperature.

- ```rust
  pub fn with_power_profile(self: Self, profile: TimeProfile) -> Self { /* ... */ }
  ```
  Attach a power-vs-time scaling profile (a factor relative to the initial

- ```rust
  pub fn power_density(self: &Self, t: Time) -> PowerDensity { /* ... */ }
  ```
  The instantaneous volumetric power density at time `t`.

- ```rust
  pub fn surface_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  The current structure surface temperature [K].

- ```rust
  pub fn interfacial_area(self: &Self) -> InterfacialAreaDensity { /* ... */ }
  ```
  Interfacial-area density `a_v` [1/m] of this power region.

- ```rust
  pub fn correct(self: &mut Self, h_sum: HeatTransferCoefficient, ht_sum: HeatFlux, dt: Time, t: Time) -> ThermodynamicTemperature { /* ... */ }
  ```
  Advance the structure surface temperature by one backward-Euler step.

- ```rust
  pub fn power_off(self: &mut Self) { /* ... */ }
  ```
  Zero the power (GeN-Foam's `powerOff`): the internal source is removed but

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
    fn clone(self: &Self) -> FixedPower { /* ... */ }
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
    fn eq(self: &Self, other: &FixedPower) -> bool { /* ... */ }
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
#### Struct `FixedTemperature`

A `fixedTemperature` structure: prescribed surface temperature, no energy
balance.

The structure surface is simply held at a prescribed temperature (constant,
or scaled by a [`TimeProfile`]). Useful for a boundary-like structure whose
temperature is imposed rather than computed.

```rust
pub struct FixedTemperature {
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
  pub fn new(interfacial_area: InterfacialAreaDensity, temperature_initial: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a fixed-temperature model.

- ```rust
  pub fn with_temperature_profile(self: Self, profile: TimeProfile) -> Self { /* ... */ }
  ```
  Attach a temperature-vs-time scaling profile (a factor relative to the

- ```rust
  pub fn surface_temperature_at(self: &Self, t: Time) -> ThermodynamicTemperature { /* ... */ }
  ```
  The imposed surface temperature at time `t`

- ```rust
  pub fn interfacial_area(self: &Self) -> InterfacialAreaDensity { /* ... */ }
  ```
  Interfacial-area density `a_v` [1/m].

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
    fn clone(self: &Self) -> FixedTemperature { /* ... */ }
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
    fn eq(self: &Self, other: &FixedTemperature) -> bool { /* ... */ }
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
#### Enum `PowerModel`

Run-time-selectable structure power model — closed enum, no `dyn` dispatch.

Port of the concrete, self-contained members of GeN-Foam's `powerModel`
run-time-selection table. Dispatch the surface-temperature update through
[`PowerModel::correct`] and read the result through
[`PowerModel::surface_temperature`].

```rust
pub enum PowerModel {
    FixedPower(FixedPower),
    FixedTemperature(FixedTemperature),
}
```

##### Variants

###### `FixedPower`

Prescribed internal power; transient lumped surface temperature.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FixedPower` |  |

###### `FixedTemperature`

Prescribed surface temperature; no energy balance.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FixedTemperature` |  |

##### Implementations

###### Methods

- ```rust
  pub fn correct(self: &mut Self, h_sum: HeatTransferCoefficient, ht_sum: HeatFlux, dt: Time, t: Time) -> ThermodynamicTemperature { /* ... */ }
  ```
  Advance the model by one timestep and return the new structure surface

- ```rust
  pub fn surface_temperature(self: &Self, t: Time) -> ThermodynamicTemperature { /* ... */ }
  ```
  The current structure surface temperature at time `t`.

- ```rust
  pub fn interfacial_area(self: &Self) -> InterfacialAreaDensity { /* ... */ }
  ```
  Interfacial-area density `a_v` [1/m] of this power region.

- ```rust
  pub fn power_off(self: &mut Self) { /* ... */ }
  ```
  Turn the internal power off (no-op for a fixed-temperature model).

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
    fn clone(self: &Self) -> PowerModel { /* ... */ }
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
    fn eq(self: &Self, other: &PowerModel) -> bool { /* ... */ }
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
## Module `power_off`

# `power_off` — criteria that scram the structure power

Port of GeN-Foam's `powerOffCriterionModel` family. Each timestep the
structure evaluates its criterion; once satisfied, every power model's
internal source is zeroed ([`super::power_model::PowerModel::power_off`]) —
the software analogue of a reactor trip / SCRAM in the educational model.

## Criteria (closed enum, no `dyn` dispatch)

| Variant | Upstream | Fires when |
|---|---|---|
| [`PowerOffCriterion::Timer`] | `timer` | The simulation time reaches a set instant |
| [`PowerOffCriterion::FieldValue`] | `fieldValue` | A monitored field's max/min crosses a threshold (with an optional confirmation delay) |

Both are evaluated through [`PowerOffCriterion::check`]. The `fieldValue`
criterion carries a small latch (`time0`) so the optional `time_delay`
confirmation window is measured from first crossing, faithfully to upstream.

```rust
pub mod power_off { /* ... */ }
```

### Types

#### Enum `FieldReduction`

Which extremum of the monitored field the `fieldValue` criterion reduces to.

Port of GeN-Foam's `fieldValue::fieldOp` (`max` / `min`).

```rust
pub enum FieldReduction {
    Max,
    Min,
}
```

##### Variants

###### `Max`

Use the field maximum over the domain.

###### `Min`

Use the field minimum over the domain.

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
    fn clone(self: &Self) -> FieldReduction { /* ... */ }
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
    fn eq(self: &Self, other: &FieldReduction) -> bool { /* ... */ }
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
#### Enum `ThresholdDirection`

The direction of the threshold test for the `fieldValue` criterion.

Port of GeN-Foam's `fieldValue::criterion`
(`valueAboveThreshold` / `valueBelowThreshold`).

```rust
pub enum ThresholdDirection {
    Above,
    Below,
}
```

##### Variants

###### `Above`

Fire when the reduced value is at or above the threshold.

###### `Below`

Fire when the reduced value is at or below the threshold.

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
    fn clone(self: &Self) -> ThresholdDirection { /* ... */ }
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
    fn eq(self: &Self, other: &ThresholdDirection) -> bool { /* ... */ }
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
#### Enum `PowerOffCriterion`

A run-time-selectable power-off (SCRAM) criterion — closed enum, no `dyn`.

```rust
pub enum PowerOffCriterion {
    Timer {
        trigger_time: uom::si::f64::Time,
    },
    FieldValue {
        reduction: FieldReduction,
        direction: ThresholdDirection,
        threshold: f64,
        time_delay: uom::si::f64::Time,
        first_crossing: Option<uom::si::f64::Time>,
    },
}
```

##### Variants

###### `Timer`

Fire once the simulation time reaches `trigger_time`
(port of `powerOffCriterionModels::timer`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `trigger_time` | `uom::si::f64::Time` | The instant at which power is turned off. |

###### `FieldValue`

Fire when a monitored scalar field's extremum crosses a threshold
(port of `powerOffCriterionModels::fieldValue`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `reduction` | `FieldReduction` | Whether to test the field maximum or minimum. |
| `direction` | `ThresholdDirection` | Whether the trip is on crossing above or below the threshold. |
| `threshold` | `f64` | The threshold value, in the monitored field's own units. |
| `time_delay` | `uom::si::f64::Time` | Optional confirmation delay: the criterion must stay satisfied from<br>first crossing until `time0 + time_delay` before it fires. Zero ⇒ no<br>delay. |
| `first_crossing` | `Option<uom::si::f64::Time>` | Latch: the time of first crossing, set on the first satisfied check. |

##### Implementations

###### Methods

- ```rust
  pub fn timer(trigger_time: Time) -> Self { /* ... */ }
  ```
  A timer criterion firing at `trigger_time`.

- ```rust
  pub fn field_value(reduction: FieldReduction, direction: ThresholdDirection, threshold: f64) -> Self { /* ... */ }
  ```
  A field-value criterion (no confirmation delay).

- ```rust
  pub fn with_time_delay(self: Self, delay: Time) -> Self { /* ... */ }
  ```
  Attach a confirmation delay to a [`PowerOffCriterion::FieldValue`]

- ```rust
  pub fn check(self: &mut Self, t: Time, field_extremum: f64) -> bool { /* ... */ }
  ```
  Evaluate the criterion at simulation time `t`.

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
    fn clone(self: &Self) -> PowerOffCriterion { /* ... */ }
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
    fn eq(self: &Self, other: &PowerOffCriterion) -> bool { /* ... */ }
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
## Module `pump`

# `pump` — structure-borne momentum source

Port of GeN-Foam's `pump`. A pump is a body-force term the structure adds to
the porous momentum equation over the cells it occupies: a fixed source
**vector**, optionally scaled in time by a [`TimeProfile`] (e.g. a coast-down
curve on loss of power).

```text
momentum_source(t) = base_source * profile(t)
```

The source vector has the dimension of the momentum-equation source density,
**N / m^3** (equivalently Pa/m — a force per unit volume). It is carried as a
basic-lib [`Vector3`] to match the raw-`f64` convention of the finite-volume
vector fields it is ultimately written into (`Foam::volVectorField`); the
unit is documented rather than type-enforced because those fields are
themselves untyped. Assembling this source into the momentum equation is the
porous-solver bead's job (op-p6p.7.11); this type only reproduces the pump's
value-and-time-scaling behaviour, which is self-contained and verifiable.

The upstream FMU-multiplier coupling (`pumpMultiplierNameFromFMU`) is not
ported — it reads a scalar from an external functional-mock-up unit, out of
scope for this crate.

```rust
pub mod pump { /* ... */ }
```

### Types

#### Struct `Pump`

A structure momentum source (pump).

Construct with [`Pump::new`]; optionally attach a time scaling with
[`Pump::with_time_profile`]. Query the instantaneous source with
[`Pump::momentum_source`].

```rust
pub struct Pump {
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
  pub fn new(base_source: Vector3) -> Self { /* ... */ }
  ```
  Build a pump with a constant momentum-source vector [N/m^3].

- ```rust
  pub fn with_time_profile(self: Self, profile: TimeProfile) -> Self { /* ... */ }
  ```
  Attach a momentum-source time-scaling profile (a factor relative to the

- ```rust
  pub fn time_dependent(self: &Self) -> bool { /* ... */ }
  ```
  Whether this pump's source varies in time.

- ```rust
  pub fn momentum_source(self: &Self, t: Time) -> Vector3 { /* ... */ }
  ```
  The momentum-source vector [N/m^3] at time `t`.

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
    fn clone(self: &Self) -> Pump { /* ... */ }
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
    fn eq(self: &Self, other: &Pump) -> bool { /* ... */ }
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
## Module `units`

# Named `uom` aliases for the solid-structure (power-model) quantities

GeN-Foam's `structure` / `powerModel` classes carry their per-cell state as
bare `Foam::volScalarField`s whose physical meaning lives only in comments
(`powerDensity_`, `alphaRhoCp_`, `iA_`, …). This module gives those recurring
quantities named, dimension-checked [`uom`] types so a reader hovering in
their editor sees [`PowerDensity`], not a raw `Quantity<...>`.

The convective-coupling quantities the fluid side hands to the structure —
the effective heat-transfer coefficient and the enthalpy-weighted `h*T`
product — reuse the parent
[`thermal_hydraulics::units`](super::super::units) aliases
([`HeatTransferCoefficient`], [`HeatFlux`]); they are re-exported here so a
caller working in `structure` finds the whole vocabulary in one place.

All quantities are SI.

```rust
pub mod units { /* ... */ }
```

### Types

#### Type Alias `PowerDensity`

Volumetric power (heat-source) density `q'''` — **base SI: W / m^3**.

The fission (or generic internal) power deposited per unit total cell volume,
i.e. GeN-Foam's `powerDensity_`. In a porous cell only the structure volume
fraction produces power, so the *effective* source in the energy balance is
`alpha * q'''` (see [`super::power_model`]). Aliased to [`uom`]'s
[`VolumetricPowerDensity`](uom::si::f64::VolumetricPowerDensity).

```rust
pub type PowerDensity = uom::si::f64::VolumetricPowerDensity;
```

#### Type Alias `VolumetricHeatCapacity`

Volumetric heat capacity `rho * Cp` (optionally `alpha`-weighted) —
**base SI: J / (m^3 K)**.

The lumped thermal inertia of the solid structure per unit total cell
volume: GeN-Foam's `alphaRhoCp_` (`alpha * rho * Cp`). Governs how fast the
structure surface temperature responds in the transient lumped energy
balance. Aliased to [`uom`]'s
[`VolumetricHeatCapacity`](uom::si::f64::VolumetricHeatCapacity).

```rust
pub type VolumetricHeatCapacity = uom::si::f64::VolumetricHeatCapacity;
```

#### Type Alias `InterfacialAreaDensity`

Interfacial-area density `a_v` (wetted surface per unit volume) —
**base SI: 1 / m** (`m^2 / m^3`).

GeN-Foam's `iA_` / `iAact_` / `iApas_`: the structure-to-fluid heat-transfer
surface area per unit total cell volume. Multiplying a surface heat flux
`q''` [W/m^2] by this density yields a volumetric heat source [W/m^3].
Aliased to [`uom`]'s [`ReciprocalLength`](uom::si::f64::ReciprocalLength).

```rust
pub type InterfacialAreaDensity = uom::si::f64::ReciprocalLength;
```

#### Type Alias `WallConductance`

Heat-exchanger wall conductance `H_w = k_wall / t_wall` —
**base SI: W / (m^2 K)**.

The series conductance of the heat-exchanger tube wall (thermal conductivity
divided by wall thickness): GeN-Foam's `Hw_`. Same dimension as a
convective heat-transfer coefficient. Aliased to [`uom`]'s
[`HeatTransfer`](uom::si::f64::HeatTransfer).

```rust
pub type WallConductance = uom::si::f64::HeatTransfer;
```

### Re-exports

#### Re-export `HeatFlux`

```rust
pub use super::super::units::HeatFlux;
```

#### Re-export `HeatTransferCoefficient`

```rust
pub use super::super::units::HeatTransferCoefficient;
```

### Types

#### Struct `StructureThermalState`

The thermal state produced by advancing a [`StructureCell`] one timestep.

All quantities are for the single porous cell that the [`StructureCell`]
represents.

```rust
pub struct StructureThermalState {
    pub active_surface_temperature: uom::si::f64::ThermodynamicTemperature,
    pub passive_surface_temperature: Option<uom::si::f64::ThermodynamicTemperature>,
    pub wall_temperature: uom::si::f64::ThermodynamicTemperature,
    pub heat_source_to_fluid: PowerDensity,
    pub wall_heat_flux: HeatFlux,
    pub powered_off: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `active_surface_temperature` | `uom::si::f64::ThermodynamicTemperature` | Active (powered) structure surface temperature `T_act` [K]. |
| `passive_surface_temperature` | `Option<uom::si::f64::ThermodynamicTemperature>` | Passive sub-structure surface temperature `T_pas` [K], if a passive<br>sub-structure is present. |
| `wall_temperature` | `uom::si::f64::ThermodynamicTemperature` | Wall temperature `T_wall` [K] = the active surface temperature (matching<br>GeN-Foam's `Twall = Tact`). |
| `heat_source_to_fluid` | `PowerDensity` | Total explicit heat source exchanged with the fluid [W/m^3] — the sum of<br>the active and passive convective terms, in GeN-Foam's<br>`a_v*(ht_sum - h_sum*T_struct)` convention. Its magnitude is the power<br>per unit volume crossing the wall. |
| `wall_heat_flux` | `HeatFlux` | Diagnostic wall heat flux `q''` [W/m^2] (positive = structure to fluid). |
| `powered_off` | `bool` | Whether the power-off (SCRAM) criterion has fired. |

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
    fn clone(self: &Self) -> StructureThermalState { /* ... */ }
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
    fn eq(self: &Self, other: &StructureThermalState) -> bool { /* ... */ }
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
#### Struct `StructureCell`

One porous cell's worth of mesh-unresolved solid structure.

Aggregates the structure's active [`PowerModel`], an optional inert
[`PassiveStructure`], an optional [`Pump`] momentum source, and an optional
[`PowerOffCriterion`] SCRAM trigger, and advances them together with
[`StructureCell::correct`] — the per-cell analogue of `structure::correct`
followed by `structure::explicitHeatSource`.

This is deliberately lumped and mesh-free: it is the unit that is directly
hand-verifiable (energy balance) and that a lumped or operator-split
integrator can drive. The full field-wide assembly over a real mesh is the
porous-solver bead's responsibility (see the [module documentation](self)).

```rust
pub struct StructureCell {
    pub power_model: PowerModel,
    pub passive: Option<PassiveStructure>,
    pub pump: Option<Pump>,
    pub power_off: Option<PowerOffCriterion>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `power_model` | `PowerModel` | The active power model (fission / internal source + surface temperature). |
| `passive` | `Option<PassiveStructure>` | Optional inert sub-structure sharing the cell. |
| `pump` | `Option<Pump>` | Optional pump momentum source over this cell. |
| `power_off` | `Option<PowerOffCriterion>` | Optional power-off (SCRAM) criterion. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(power_model: PowerModel) -> Self { /* ... */ }
  ```
  Build a structure cell from just an active power model. Attach the

- ```rust
  pub fn with_passive(self: Self, passive: PassiveStructure) -> Self { /* ... */ }
  ```
  Attach an inert passive sub-structure.

- ```rust
  pub fn with_pump(self: Self, pump: Pump) -> Self { /* ... */ }
  ```
  Attach a pump momentum source.

- ```rust
  pub fn with_power_off(self: Self, criterion: PowerOffCriterion) -> Self { /* ... */ }
  ```
  Attach a power-off (SCRAM) criterion.

- ```rust
  pub fn momentum_source(self: &Self, t: Time) -> Vector3 { /* ... */ }
  ```
  This cell's pump momentum source [N/m^3] at time `t` (zero if no pump).

- ```rust
  pub fn correct(self: &mut Self, h_sum: HeatTransferCoefficient, ht_sum: HeatFlux, dt: Time, t: Time, scram_field_extremum: f64) -> StructureThermalState { /* ... */ }
  ```
  Advance the structure one timestep and return its thermal state.

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
    fn clone(self: &Self) -> StructureCell { /* ... */ }
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
    fn eq(self: &Self, other: &StructureCell) -> bool { /* ... */ }
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
### Re-exports

#### Re-export `hx_wall_temperature`

```rust
pub use heat_exchanger::hx_wall_temperature;
```

#### Re-export `HeatExchanger`

```rust
pub use heat_exchanger::HeatExchanger;
```

#### Re-export `explicit_heat_source`

```rust
pub use heat_source::explicit_heat_source;
```

#### Re-export `wall_heat_flux`

```rust
pub use heat_source::wall_heat_flux;
```

#### Re-export `PassiveStructure`

```rust
pub use heat_source::PassiveStructure;
```

#### Re-export `FixedPower`

```rust
pub use power_model::FixedPower;
```

#### Re-export `FixedTemperature`

```rust
pub use power_model::FixedTemperature;
```

#### Re-export `PowerModel`

```rust
pub use power_model::PowerModel;
```

#### Re-export `FieldReduction`

```rust
pub use power_off::FieldReduction;
```

#### Re-export `PowerOffCriterion`

```rust
pub use power_off::PowerOffCriterion;
```

#### Re-export `ThresholdDirection`

```rust
pub use power_off::ThresholdDirection;
```

#### Re-export `Pump`

```rust
pub use pump::Pump;
```

#### Re-export `HeatFlux`

```rust
pub use units::HeatFlux;
```

#### Re-export `HeatTransferCoefficient`

```rust
pub use units::HeatTransferCoefficient;
```

#### Re-export `InterfacialAreaDensity`

```rust
pub use units::InterfacialAreaDensity;
```

#### Re-export `PowerDensity`

```rust
pub use units::PowerDensity;
```

#### Re-export `VolumetricHeatCapacity`

```rust
pub use units::VolumetricHeatCapacity;
```

#### Re-export `WallConductance`

```rust
pub use units::WallConductance;
```

## Module `thermophysical`

# `genfoam::thermal_hydraulics::thermophysical` — bespoke GeN-Foam fluid property packages

GeN-Foam's `thermophysicalProperties/` sub-tree defines fluid-property
packages that are not already covered by
[`tampines_steam_tables`](https://docs.rs/tampines-steam-tables) (steam/water)
or `outram-foam-basic-lib`'s thermo layer. As of upstream commit `652b3da`
there is exactly one such package: **dissociating hydrogen** (`H2`,
GeN-Foam's bespoke H/H2 property model for nuclear-thermal-propulsion-style
high-temperature hydrogen coolant). See [`hydrogen`] for the full
implementation and provenance notes.

## Public surface — [`BespokeFluidPropertyModel`]

Following this crate's "no trait objects — use enums for dispatch" rule
(mirroring
[`closures::fs_drag::FsWallFriction`](crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction)),
the entire public surface of this module is the closed
[`BespokeFluidPropertyModel`] enum. It currently has one variant,
[`BespokeFluidPropertyModel::Hydrogen`]; adding a second bespoke fluid
package later means adding a variant here and a sibling module next to
[`hydrogen`], not introducing a trait object.

Every evaluator takes the fluid state as `(p, T)` — pressure and
thermodynamic temperature — matching upstream's `H2::method(p, T)`
signatures, and returns a named `uom` quantity from [`units`].

## What this module does *not* cover

`src/thermophysicalProperties/` is only 3 upstream files (1,415 LOC) out of
the ~65k-LOC `thermalHydraulics` module (see `docs/genfoam-port-plan.md`,
"thermalHydraulics breakdown"). Reading/writing GeN-Foam's `constant/`
dictionary format to select a fluid model at run time is an I/O concern
that belongs with [`io`](crate::io), not here — this module's job is the
physics evaluators, addressed by picking an enum variant in Rust code.

```rust
pub mod thermophysical { /* ... */ }
```

### Modules

## Module `units`

# Named `uom` aliases for bespoke thermophysical-property quantities

[`super`]'s bespoke fluid-property models (currently only hydrogen) take a
`(p, T)` state and return several distinct physical quantities. `uom`
already gives most of these clear names ([`MassDensity`], [`DynamicViscosity`],
[`ThermalConductivity`], [`SpecificHeatCapacity`]); this module re-exports
them under one roof and adds the two names `uom` does not distinguish on its
own: **specific enthalpy** (`uom`'s [`AvailableEnergy`](uom::si::f64::AvailableEnergy))
and **specific entropy**, which is dimensionally identical to specific heat
capacity (`J / (kg K)`) but a different physical quantity, so a bare
`SpecificHeatCapacity` return type at a `entropy()` call site would mislead a
reader. Two dimensionless fractions ([`MoleFraction`], [`MassFraction`]) get
the same treatment as the sibling [`units`](super::super::units) module's
`ReynoldsNumber`/`DarcyFrictionFactor` aliases of [`Ratio`](uom::si::f64::Ratio).

All quantities are SI (`p` in Pa, `T` in K).

```rust
pub mod units { /* ... */ }
```

### Types

#### Type Alias `MassDensity`

Mass density `rho` — **base SI: kg / m^3**.

```rust
pub type MassDensity = uom::si::f64::MassDensity;
```

#### Type Alias `SpecificHeatCapacity`

Isobaric specific heat capacity `Cp` — **base SI: J / (kg K)**.

```rust
pub type SpecificHeatCapacity = uom::si::f64::SpecificHeatCapacity;
```

#### Type Alias `SpecificEnthalpy`

Specific enthalpy `h` — **base SI: J / kg**. Aliases `uom`'s
[`AvailableEnergy`](uom::si::f64::AvailableEnergy), the quantity `uom` uses
for mass-specific energy.

```rust
pub type SpecificEnthalpy = uom::si::f64::AvailableEnergy;
```

#### Type Alias `SpecificEntropy`

Specific entropy `s` — **base SI: J / (kg K)**. Dimensionally identical to
[`SpecificHeatCapacity`] (both are energy / mass / temperature), so `uom`
has no distinct quantity for it; this alias exists purely so a function
signature returning entropy does not read as if it returns a heat capacity.

```rust
pub type SpecificEntropy = uom::si::f64::SpecificHeatCapacity;
```

#### Type Alias `DynamicViscosity`

Dynamic viscosity `mu` — **base SI: Pa s**.

```rust
pub type DynamicViscosity = uom::si::f64::DynamicViscosity;
```

#### Type Alias `ThermalConductivity`

Thermal conductivity `kappa` — **base SI: W / (m K)**.

```rust
pub type ThermalConductivity = uom::si::f64::ThermalConductivity;
```

#### Type Alias `MoleFraction`

A mole fraction (e.g. atomic-hydrogen mole fraction `x_H`) —
**dimensionless**. Aliases [`uom`]'s [`Ratio`](uom::si::f64::Ratio).

```rust
pub type MoleFraction = uom::si::f64::Ratio;
```

#### Type Alias `MassFraction`

A mass fraction (e.g. atomic-hydrogen mass fraction `w_H`) —
**dimensionless**. Aliases [`uom`]'s [`Ratio`](uom::si::f64::Ratio).

```rust
pub type MassFraction = uom::si::f64::Ratio;
```

#### Type Alias `SpecificVolume`

Specific volume (e.g. a second virial coefficient `B`) — **base SI:
m^3 / kg**. Aliases [`uom`]'s [`SpecificVolume`](uom::si::f64::SpecificVolume).

```rust
pub type SpecificVolume = uom::si::f64::SpecificVolume;
```

#### Type Alias `PrandtlNumber`

A Prandtl number `Pr = Cp mu / kappa` — **dimensionless**. Aliases
[`uom`]'s [`Ratio`](uom::si::f64::Ratio).

```rust
pub type PrandtlNumber = uom::si::f64::Ratio;
```

### Types

#### Enum `BespokeFluidPropertyModel`

A bespoke (non-steam-table) fluid thermophysical-property model.

Closed enum port of GeN-Foam's `thermophysicalProperties`
runtime-selectable family — see the [module documentation](self) for why
this is the crate's entire public surface for bespoke fluid properties.
Evaluate with the methods below, each taking pressure `p` and
thermodynamic temperature `T`.

```rust
pub enum BespokeFluidPropertyModel {
    Hydrogen,
}
```

##### Variants

###### `Hydrogen`

Dissociating hydrogen (H <-> H2 thermal equilibrium), GeN-Foam's `H2`
class. See [`hydrogen`] for the correlations and their provenance.

##### Implementations

###### Methods

- ```rust
  pub fn density(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
  ```
  Mass density `rho`. `p` in Pa, `T` in K; returns kg/m^3.

- ```rust
  pub fn specific_heat_capacity(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
  ```
  Isobaric specific heat capacity `Cp`. `p` in Pa, `T` in K; returns

- ```rust
  pub fn specific_heat_capacity_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
  ```
  Isochoric specific heat capacity `Cv`. `p` in Pa, `T` in K; returns

- ```rust
  pub fn dynamic_viscosity(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity { /* ... */ }
  ```
  Dynamic viscosity `mu`. `p` in Pa, `T` in K; returns Pa s.

- ```rust
  pub fn thermal_conductivity(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity { /* ... */ }
  ```
  Thermal conductivity `kappa`. `p` in Pa, `T` in K; returns W/(m K).

- ```rust
  pub fn specific_enthalpy(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificEnthalpy { /* ... */ }
  ```
  Specific enthalpy `h`. `p` in Pa, `T` in K; returns J/kg.

- ```rust
  pub fn specific_entropy(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificEntropy { /* ... */ }
  ```
  Specific entropy `s`. `p` in Pa, `T` in K; returns J/(kg K).

- ```rust
  pub fn prandtl_number(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> PrandtlNumber { /* ... */ }
  ```
  Prandtl number `Pr = Cp mu / kappa`. `p` in Pa, `T` in K; dimensionless.

- ```rust
  pub fn dissociated_mole_fraction(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MoleFraction { /* ... */ }
  ```
  Mole fraction of a dissociated/minority species (for [`Hydrogen`](Self::Hydrogen),

- ```rust
  pub fn dissociated_mass_fraction(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassFraction { /* ... */ }
  ```
  Mass fraction of a dissociated/minority species (for [`Hydrogen`](Self::Hydrogen),

- ```rust
  pub fn second_virial_coefficient(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificVolume { /* ... */ }
  ```
  Second virial coefficient `B` of the real-gas equation of state. `p`

- ```rust
  pub fn kappa_over_cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity { /* ... */ }
  ```
  `kappa / Cp` (upstream `H2::alphah`). See

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
    fn clone(self: &Self) -> BespokeFluidPropertyModel { /* ... */ }
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
    fn default() -> BespokeFluidPropertyModel { /* ... */ }
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
    fn eq(self: &Self, other: &BespokeFluidPropertyModel) -> bool { /* ... */ }
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
## Module `units`

# Named `uom` aliases for thermal-hydraulics quantities

GeN-Foam's thermal-hydraulics closures return bare `Foam::scalar`s whose
physical meaning is only documented in comments. This module gives the
recurring TH quantities named, dimension-checked [`uom`] types so a reader
hovering in their editor sees `DarcyFrictionFactor`, not a raw
`Quantity<...>`. Each alias below is the type a closure method takes or
returns.

All quantities are SI. Two of them ([`ReynoldsNumber`], [`DarcyFrictionFactor`])
are dimensionless and alias [`uom`]'s [`Ratio`](uom::si::f64::Ratio); the
others carry genuine dimensions.

## Why the correlation cores still take a plain `Ratio`

A friction-factor correlation is a pure map `Re -> f`, both dimensionless.
Wrapping them in `Ratio` keeps the *call site* self-documenting (you cannot
pass a temperature where a Reynolds number is wanted) without inventing a
bespoke newtype the compiler could not check.

```rust
pub mod units { /* ... */ }
```

### Types

#### Type Alias `ReynoldsNumber`

Reynolds number `Re = rho * |U| * D_h / mu` — **dimensionless**.

The single independent variable of the fluid-structure wall-friction
correlations in [`super::closures::fs_drag`]. GeN-Foam forms it per cell from
the local fluid density, superficial velocity magnitude, hydraulic diameter,
and dynamic viscosity; here it is passed in already assembled. Aliased to
[`uom`]'s [`Ratio`].

```rust
pub type ReynoldsNumber = uom::si::f64::Ratio;
```

#### Type Alias `DarcyFrictionFactor`

Darcy(-Weisbach) friction factor `f` — **dimensionless**.

The quantity returned by the fluid-structure wall-friction correlations. In
the laminar limit it reduces to `f = C / Re` (circular pipe `C = 64`,
wire-wrapped rod bundles `C ~ 99..110`). The pressure gradient along a
channel is `dp/dx = -f * (rho * U^2) / (2 * D_h)`. Aliased to [`uom`]'s
[`Ratio`].

Note: this is the **Darcy** factor (the `64/Re` convention), four times the
Fanning factor. GeN-Foam's `FSDragCoefficientModel::value()` returns this
Darcy form (its laminar branch is `64/Re`, `99/Re`, etc.).

```rust
pub type DarcyFrictionFactor = uom::si::f64::Ratio;
```

#### Type Alias `HeatTransferCoefficient`

Convective heat-transfer coefficient `h` — **base SI: W / (m^2 K)**.

Used by the fluid-structure and fluid-fluid heat-transfer closures
(`Nu = h * D_h / k`). Aliased to [`uom`]'s
[`HeatTransfer`](uom::si::f64::HeatTransfer).

```rust
pub type HeatTransferCoefficient = uom::si::f64::HeatTransfer;
```

#### Type Alias `HeatFlux`

Surface heat flux `q''` — **base SI: W / m^2**.

The wall heat flux exchanged between the fluid and the structure; the target
of the critical-heat-flux (CHF) closures. Aliased to [`uom`]'s
[`HeatFluxDensity`](uom::si::f64::HeatFluxDensity).

```rust
pub type HeatFlux = uom::si::f64::HeatFluxDensity;
```

### Functions

#### Function `reynolds_number`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Build a [`ReynoldsNumber`] from a plain (dimensionless) magnitude.

Convenience for call sites that have already computed `Re` as an `f64`.

```rust
pub fn reynolds_number(value: f64) -> ReynoldsNumber { /* ... */ }
```

#### Function `friction_factor_value`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`
- `MustUse { reason: None }`

Read a [`DarcyFrictionFactor`] back as a plain (dimensionless) `f64`.

```rust
pub fn friction_factor_value(f: DarcyFrictionFactor) -> f64 { /* ... */ }
```

## Module `thermo_mechanics`

# `genfoam::thermo_mechanics` — thermal-expansion / thermo-mechanics feedback

Rust port of `GeN-Foam/src/classes/thermoMechanics`: the linear-elastic,
small-strain thermal-expansion model that feeds geometry/density changes back
into the neutronics and thermal-hydraulics regions. Upstream it is a
transient segregated finite-volume solver derived from OpenFOAM's
`solidDisplacementFoam`, generalised to multiple material zones
(`legacyThermoMechanics`).

## What this module contains

The self-contained, analytically-verifiable physics core of the model:

- [`material`] — the per-zone isotropic linear-elastic material card
  ([`ElasticMaterial`]): `E`, `ν`, `α`, `ρ`, `c_p`, `k`, and the derived Lamé
  parameters `μ`, `λ`, bulk term `3K`, and thermal diffusivity `DT`.
- [`stress`] — the Hooke's-law thermal-stress constitutive relation:
  `σ = 2μ ε + λ tr(ε) I − 3K α ΔT I`, plus the von Mises equivalent stress.
- [`feedback`] — the geometry feedback the model *produces*: the axial
  fuel/control-rod thermal expansion (`u = ∫ α ΔT dz`) and the assembly of
  the coupled mesh-displacement vector `meshDisp` mapped onto the other
  meshes.

These are wired together by [`LegacyThermoMechanics`] and dispatched through
the [`ThermoMechanicsModel`] enum (no `dyn`, per workspace rules).

- [`mesh_solve`] — the full finite-volume field solve on the mechanics
  `FvMesh`: the segregated displacement-momentum equation (`DEqn`) and the
  structure heat-diffusion equation (`TEqn`) from
  `include/solveThermalMechanics.H`, plus the coupled `meshDisp` assembly.
  The entry point is [`MechanicsMeshSolver`]. It consumes the constitutive
  kernel above cell-by-cell.

## Field solve — now implemented on the mechanics mesh

The full displacement *field* solve on an `fvMesh`
(`include/solveThermalMechanics.H`) and the structure heat-diffusion solve
live in [`mesh_solve`], built on the finite-volume operators
[`outram_foam_basic_lib`] now exposes:

- vector-field gradient `∇u → VolTensorField` (`fvc::grad_vec`) and
  tensor-field divergence `∇·σ → VolVectorField` (`fvc::div_tensor`),
- `fvm::d2dt2` (implicit second time derivative) for the inertial term,
- the scalar `fvm::laplacian` / vector `fvm::laplacian_vec` diffusion
  operators and `fvc::grad` for the explicit thermal load.

The constitutive law and feedback relations ported in [`stress`]/[`feedback`]
are exactly the physics the FV assembly evaluates pointwise, and also stand
alone against analytical benchmarks (see the `#[test]` V&V cases in each
submodule, and the coupled field-solve V&V in [`mesh_solve`]).

## Still deferred

An anisotropic stiffness (needs a tensor-coefficient Laplacian), a traction
(`σ·n = 0`) boundary condition on `D`, non-orthogonal gradient correction,
and the multi-region mapping of the 1-D axial fuel/CR expansion onto a shared
mesh — see the limitations note in [`mesh_solve`].

## Interface expected from `genfoam::common` (ported in parallel)

This module currently needs nothing from `genfoam::common` at compile time.
The mesh solve ([`mesh_solve`]) and the multi-region coupling will need,
from `common` / the wider port:

- a material-zone map (cell → [`ElasticMaterial`]) built from the
  `thermoMechanicalProperties`/`materials` dictionary (upstream reads it via
  `cellZones`),
- the `meshToMesh` field-mapping used by `interpolateCouplingFields`
  (`multiRegion`) to pull `TFuel`, `TStruct`, and `powerDensityNeutronics`
  onto the mechanics mesh and push `meshDisp` back, and
- `movePoints`-style mesh deformation on `FvMesh` (upstream `deformMesh`).

See `docs/genfoam-port-plan.md` for the module map and translation order.

```rust
pub mod thermo_mechanics { /* ... */ }
```

### Modules

## Module `feedback`

# Thermal-expansion geometry feedback

The geometry/density feedback that thermo-mechanics produces for the
neutronics and thermal-hydraulics regions: the axial thermal expansion of
the fuel and control-rod columns, and the assembly of the mesh-displacement
vector `meshDisp` that GeN-Foam maps back onto the other meshes to move
points (`deformMesh`) and shift cross-sections.

## Axial fuel / control-rod expansion (the reactor-physics feedback)

GeN-Foam solves two 1-D scalar transport equations along the pin direction
`ê` (`globalOptions/pinDirection`) for the accumulated axial displacement of
the fuel and control-rod stacks (`include/solveThermalMechanics.H`):

```text
  div(ê·Sf, fuelDisp) = α_fuel (T_fuel − T_fuelRef)
  div(−ê·Sf, CRDisp)  = −α_CR   (T_struct − T_CRRef)
```

On a 1-D column of cells stacked along `ê` this upwind divergence is exactly
a cumulative integral of the local thermal strain: the displacement at the
outlet face of cell `i` is `Σ_{j≤i} α ΔT_j Δz_j`. Physically it is the
free axial thermal growth of a bar, `u(z) = ∫₀^z α (T(z') − T_ref) dz'`.
Axial fuel expansion lengthens the active fuel column (a negative,
stabilising reactivity feedback in fast reactors); control-rod-stack
expansion inserts the rods further. Both are the geometry changes this
module feeds to neutronics.

## Mesh-displacement assembly

The vector actually mapped to the other meshes is (upstream):

```text
  meshDisp = (disp − (disp·ê) ê) + (fuelDisp + CRDisp) ê
```

i.e. the radial (transverse-to-pin) part of the elastic displacement plus the
axial fuel/CR expansion along `ê`. The axial component of the 2-D/3-D elastic
displacement is *removed* and replaced by the physically-resolved 1-D axial
expansion, so that spurious axial variation of the plane solve does not leak
into the coupled geometry. See [`assemble_mesh_displacement`].

```rust
pub mod feedback { /* ... */ }
```

### Functions

#### Function `free_axial_expansion`

Free axial thermal expansion of a uniformly-heated bar, `Δu = α ΔT L`.

The closed-form limit of the axial-expansion transport equation for a bar of
length `L` at a uniform temperature rise `ΔT` above its stress-free
reference. This is the total elongation of the fuel (or control-rod) stack.

# Parameters

- `alpha` — linear thermal-expansion coefficient `α` (1/K).
- `delta_t` — uniform temperature rise `ΔT = T − T_ref` (kelvin interval).
- `length` — undeformed bar length `L` (m).

# Returns

The axial elongation `Δu` as a [`Displacement`] (metres).

```rust
pub fn free_axial_expansion(alpha: super::material::ThermalExpansionCoeff, delta_t: uom::si::f64::TemperatureInterval, length: uom::si::f64::Length) -> super::material::Displacement { /* ... */ }
```

#### Function `axial_expansion_profile`

Cumulative axial-displacement profile of a stack of cells along the pin.

Ports the upwind `div(ê·Sf, fuelDisp) = α (T − T_ref)` solve on a 1-D column:
the returned value `u[i]` is the axial displacement of the **outlet face** of
cell `i`, i.e. the running sum `Σ_{j≤i} α ΔT_j Δz_j` of per-cell thermal
growth. The inlet face (base of the stack) is anchored at zero displacement.

# Parameters

- `alpha` — expansion coefficient `α` (1/K), assumed uniform along the stack.
- `delta_t` — per-cell temperature rise `ΔT_i = T_i − T_ref` (kelvin
  interval), ordered from the anchored base to the free tip.
- `cell_height` — per-cell height `Δz_i` (m); same length/order as `delta_t`.

# Returns

A `Vec` of cumulative axial [`Displacement`]s, one per cell (outlet-face
value). Empty inputs yield an empty vector. If the two slices differ in
length the shorter is used.

```rust
pub fn axial_expansion_profile(alpha: super::material::ThermalExpansionCoeff, delta_t: &[uom::si::f64::TemperatureInterval], cell_height: &[uom::si::f64::Length]) -> Vec<super::material::Displacement> { /* ... */ }
```

#### Function `assemble_mesh_displacement`

Assemble the coupled mesh-displacement vector `meshDisp` (metres).

`meshDisp = (disp − (disp·ê) ê) + axial ê`: the transverse-to-pin part of the
elastic displacement `disp` plus the axially-resolved fuel/control-rod
expansion `axial` along the (normalised) pin direction `ê`. This is the
vector GeN-Foam maps onto the neutronics/TH meshes and passes to
`movePoints`.

# Parameters

- `disp` — the elastic displacement vector at the point, in **metres**
  (`outram-foam-basic-lib` has no `uom` vector type; unit is documented).
- `axial` — the axial fuel+CR expansion `fuelDisp + CRDisp` along `ê`
  ([`Displacement`], metres).
- `pin_direction` — the pin orientation `ê`; normalised internally (a zero
  vector yields just the axial term dropped and `disp` returned unchanged).

# Returns

The mesh-displacement vector in **metres** (a [`Vector3`]).

```rust
pub fn assemble_mesh_displacement(disp: outram_foam_basic_lib::primitives::Vector3, axial: super::material::Displacement, pin_direction: outram_foam_basic_lib::primitives::Vector3) -> outram_foam_basic_lib::primitives::Vector3 { /* ... */ }
```

## Module `material`

# Linear-elastic, thermally-expanding material

Per-material-zone constitutive data for GeN-Foam's `legacyThermoMechanics`
model, ported from the property block of its constructor
(`legacyThermoMechanics.C`). Each mesh cell zone in GeN-Foam carries one such
isotropic linear-elastic material: density, Young's modulus, Poisson's ratio,
specific heat, conductivity, and a linear thermal-expansion coefficient.

## Derived elastic moduli

From Young's modulus `E` and Poisson's ratio `ν` this computes the Lamé
parameters and the bulk-modulus term used by the stress solve:

```text
  μ  = E / (2 (1 + ν))                         (shear modulus)
  λ  = ν E / ((1 + ν)(1 − 2ν))                 (Lamé's first parameter, plane strain)
  3K = E / (1 − 2ν)                            (bulk-modulus term,     plane strain)
```

For the plane-stress rheology option GeN-Foam substitutes
`λ = ν E / ((1 + ν)(1 − ν))` and `3K = E / (1 − ν)` (see [`RheologyOption`]).

It also derives the thermal diffusivity `DT = k / (ρ c_p)` used by the
structure heat-diffusion solve.

## Units note vs. upstream

GeN-Foam divides its displacement equation through by density, so upstream
`E_`, `mu_`, `lambda_`, `sigmaD_` are **specific** (per-unit-mass, `m²/s²`)
and the physical stress is recovered as `rho·sigmaD`. This port keeps every
modulus in **physical** SI units (pascals) throughout; the two formulations
are algebraically identical (see [`super::stress`]).

```rust
pub mod material { /* ... */ }
```

### Types

#### Type Alias `YoungsModulus`

Young's modulus `E` — an elastic stiffness, a [`Pressure`] (pascals, Pa).
Named alias for readability at call sites.

```rust
pub type YoungsModulus = uom::si::f64::Pressure;
```

#### Type Alias `ShearModulus`

Shear modulus `μ` (Lamé's second parameter) — a [`Pressure`] (Pa).

```rust
pub type ShearModulus = uom::si::f64::Pressure;
```

#### Type Alias `LameLambda`

Lamé's first parameter `λ` — a [`Pressure`] (Pa).

```rust
pub type LameLambda = uom::si::f64::Pressure;
```

#### Type Alias `BulkModulusTerm`

Bulk-modulus term `3K` used in the thermal-stress relation — a [`Pressure`]
(Pa). Equals `E / (1 − 2ν)` (plane strain) or `E / (1 − ν)` (plane stress).

```rust
pub type BulkModulusTerm = uom::si::f64::Pressure;
```

#### Type Alias `Stress`

A mechanical stress component — a [`Pressure`] (Pa).

```rust
pub type Stress = uom::si::f64::Pressure;
```

#### Type Alias `ThermalExpansionCoeff`

Linear thermal-expansion coefficient `α` — inverse temperature (`1/K`).
Named alias over `uom`'s [`TemperatureCoefficient`].

```rust
pub type ThermalExpansionCoeff = uom::si::f64::TemperatureCoefficient;
```

#### Type Alias `ThermalDiffusivity`

Thermal diffusivity `DT = k / (ρ c_p)` — [`DiffusionCoefficient`] (`m²/s`).

```rust
pub type ThermalDiffusivity = uom::si::f64::DiffusionCoefficient;
```

#### Type Alias `Displacement`

A displacement / elongation — a [`Length`] (metres). Named alias for the
mesh-displacement and thermal-expansion outputs of this module.

```rust
pub type Displacement = uom::si::f64::Length;
```

#### Enum `RheologyOption`

Which 2-D idealisation the Lamé parameters `λ` and `3K` are formed under.

Ported from `legacyThermoMechanics`'s `planeStress_` switch
(`rheologyOption/planeStress` in the `thermoMechanicalProperties`
dictionary). In a full 3-D analysis the distinction is moot and
[`RheologyOption::PlaneStrain`] (the GeN-Foam default) is used.

```rust
pub enum RheologyOption {
    PlaneStrain,
    PlaneStress,
}
```

##### Variants

###### `PlaneStrain`

Plane strain (out-of-plane strain assumed zero) — GeN-Foam's default.
`λ = ν E / ((1+ν)(1−2ν))`, `3K = E / (1−2ν)`.

###### `PlaneStress`

Plane stress (out-of-plane stress assumed zero) — for thin bodies.
`λ = ν E / ((1+ν)(1−ν))`, `3K = E / (1−ν)`.

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
    fn clone(self: &Self) -> RheologyOption { /* ... */ }
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
    fn default() -> RheologyOption { /* ... */ }
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
    fn eq(self: &Self, other: &RheologyOption) -> bool { /* ... */ }
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
#### Enum `MaterialError`

Errors from constructing a [`ElasticMaterial`].

```rust
pub enum MaterialError {
    NonPositiveYoungsModulus(f64),
    NonPositiveDensity(f64),
    NonPositiveSpecificHeat(f64),
    PoissonRatioOutOfRange(f64),
}
```

##### Variants

###### `NonPositiveYoungsModulus`

Young's modulus was not strictly positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `NonPositiveDensity`

Density was not strictly positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `NonPositiveSpecificHeat`

Specific heat capacity was not strictly positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `PoissonRatioOutOfRange`

Poisson's ratio was outside the thermodynamically admissible range
`−1 < ν < 1/2` for an isotropic material. At `ν = 1/2` the material is
incompressible and `3K → ∞`, so the derived moduli diverge.

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
    fn clone(self: &Self) -> MaterialError { /* ... */ }
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
    fn eq(self: &Self, other: &MaterialError) -> bool { /* ... */ }
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
#### Struct `ElasticMaterial`

An isotropic linear-elastic material with linear thermal expansion — the
per-zone material card of GeN-Foam's `legacyThermoMechanics`.

All fields carry `uom` dimensioned quantities. Construct with [`Self::new`],
which validates the ranges and is the only way to build one.

# Valid ranges / assumptions

- `E > 0` (pascals). Typical structural metals: `E ≈ 200 GPa` (steel).
- `−1 < ν < 1/2` (dimensionless). Typical metals `ν ≈ 0.3`.
- `ρ > 0` (kg/m³), `c_p > 0` (J/(kg·K)).
- `α ≥ 0` typically (1/K); metals `α ≈ 1–2 × 10⁻⁵ K⁻¹`.
- `k ≥ 0` (W/(m·K)).

Isotropy and small-strain linear elasticity are assumed (as in the upstream
model); no plasticity, creep, or anisotropy is represented.

```rust
pub struct ElasticMaterial {
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
  pub fn new(youngs_modulus: YoungsModulus, poisson_ratio: Ratio, thermal_expansion: ThermalExpansionCoeff, density: MassDensity, specific_heat: SpecificHeatCapacity, conductivity: ThermalConductivity) -> Result<Self, MaterialError> { /* ... */ }
  ```
  Build a material from its physical properties, validating ranges.

- ```rust
  pub fn youngs_modulus(self: &Self) -> YoungsModulus { /* ... */ }
  ```
  Young's modulus `E` (Pa).

- ```rust
  pub fn poisson_ratio(self: &Self) -> Ratio { /* ... */ }
  ```
  Poisson's ratio `ν` (dimensionless).

- ```rust
  pub fn thermal_expansion(self: &Self) -> ThermalExpansionCoeff { /* ... */ }
  ```
  Linear thermal-expansion coefficient `α` (1/K).

- ```rust
  pub fn density(self: &Self) -> MassDensity { /* ... */ }
  ```
  Mass density `ρ` (kg/m³).

- ```rust
  pub fn specific_heat(self: &Self) -> SpecificHeatCapacity { /* ... */ }
  ```
  Specific heat capacity `c_p` (J/(kg·K)).

- ```rust
  pub fn conductivity(self: &Self) -> ThermalConductivity { /* ... */ }
  ```
  Thermal conductivity `k` (W/(m·K)).

- ```rust
  pub fn shear_modulus(self: &Self) -> ShearModulus { /* ... */ }
  ```
  Shear modulus `μ = E / (2 (1 + ν))` (Pa) — Lamé's second parameter.

- ```rust
  pub fn lame_lambda(self: &Self, rheology: RheologyOption) -> LameLambda { /* ... */ }
  ```
  Lamé's first parameter `λ` (Pa) under the given [`RheologyOption`].

- ```rust
  pub fn bulk_modulus_three_k(self: &Self, rheology: RheologyOption) -> BulkModulusTerm { /* ... */ }
  ```
  Bulk-modulus term `3K` (Pa) under the given [`RheologyOption`].

- ```rust
  pub fn thermal_diffusivity(self: &Self) -> ThermalDiffusivity { /* ... */ }
  ```
  Thermal diffusivity `DT = k / (ρ c_p)` (m²/s) for the structure

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
    fn clone(self: &Self) -> ElasticMaterial { /* ... */ }
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
    fn eq(self: &Self, other: &ElasticMaterial) -> bool { /* ... */ }
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
## Module `mesh_solve`

# Displacement + structure-heat field solve on the mechanics `FvMesh`

The finite-volume, segregated field solve of GeN-Foam's `legacyThermoMechanics`
model — the mesh-level counterpart of the pointwise constitutive kernel in
[`super::stress`] / [`super::material`]. Ported from
`include/solveThermalMechanics.H`, whose two solves are:

- **`TEqn`** — the structure heat-diffusion equation for the temperature field
  `TStruct` on the mechanics mesh (feeds `ΔT` into the thermal stress), and
- **`DEqn`** — the small-strain linear-elastic displacement-momentum equation
  for the displacement field `D`, in the `solidDisplacementFoam`
  compact-normal-stress segregated form.

## The displacement equation (`DEqn`)

The momentum balance of a linear-elastic solid is `∇·σ = ρ ∂²D/∂t²` with the
thermo-elastic Cauchy stress

```text
  σ = μ (∇D + ∇Dᵀ) + λ tr(∇D) I − 3K α (T − T_ref) I
    = 2μ ε + λ tr(ε) I − 3K α ΔT I
```

(`ε = symm(∇D)`; `μ`, `λ` the Lamé parameters; `3K = 3λ + 2μ` the bulk-modulus
term; `α` the linear thermal-expansion coefficient). Splitting the elastic
divergence into an implicit Laplacian on `D` plus an explicit correction
(OpenFOAM's *compact normal stress* form),

```text
  ∇·σ = ∇·[(2μ+λ) ∇D]                      (implicit, fvm::laplacian)
      + ∇·[σ_D − (2μ+λ) ∇D]                (explicit, divSigmaExp = fvc::div)
      − ∇(3K α ΔT)                          (explicit thermal load, fvc::grad)
```

with `σ_D = μ twoSymm(∇D) + λ tr(∇D) I`. The assembled system solved each
outer (segregated) corrector is

```text
  ρ ∂²D/∂t² − ∇·[(2μ+λ)∇D] = divSigmaExp − ∇(3K α ΔT)
```

and the explicit `divSigmaExp` is refreshed from the freshly-solved `D`,
iterating to convergence. In the **quasi-static** form the inertial
`ρ ∂²D/∂t²` term is dropped (a pure elliptic equilibrium solve, `∇·σ = 0`).

## Operators consumed from `outram_foam_basic_lib`

`fvm::laplacian_vec`, `fvm::d2dt2_coeff`, `fvc::grad`, `fvc::grad_vec`,
`fvc::div_tensor`, and the scalar `fvm::laplacian` for `TEqn`.

## Known limitations (inherited from the basic-lib operator layer)

- **Scalar-coefficient Laplacian only.** `fvm::laplacian_vec` takes an
  isotropic scalar `2μ+λ`; an anisotropic stiffness tensor is not expressible
  and would need a new operator.
- **Single Gauss gradient, no non-orthogonal correction.** `fvc::grad_vec`
  sets a *zero-gradient* output boundary, so the explicit `divSigmaExp`
  correction carries no boundary-gradient information; near a boundary it is
  approximate. On a 1-D column the correction is identically zero (its
  non-zero entries are transverse normal stresses with no transverse
  gradient), so the implicit Laplacian is exact there — which is what the V&V
  cases below exploit.
- **Constant-Δt `d2dt2`.** The transient inertial term assumes a fixed
  timestep and a non-moving mesh.

A traction (Neumann) boundary condition on `D` that imposes a non-zero stress
(e.g. a stress-free surface under thermal load, `σ·n = 0 ⇒ ∂D/∂n ≠ 0`) is not
yet available — the field boundary layer offers only Dirichlet (`FixedValue`)
and zero-gradient. The stress-free free-expansion case is therefore reached
here through a non-uniform temperature field with Dirichlet ends rather than a
traction surface (see the V&V tests). A `tractionDisplacement` boundary
condition is a basic-lib follow-up.

## What this does not include

The separate 1-D axial fuel / control-rod expansion solve (`fuelDispEqn` /
`CRDispEqn`) and its column-to-mesh mapping remain in [`super::feedback`] as
the closed-form cumulative integral; wiring those onto a shared multi-region
mesh belongs to `genfoam::multi_region`. The [`MechanicsMeshSolver`] exposes
[`MechanicsMeshSolver::mesh_displacement`] to assemble the coupled `meshDisp`
per cell via [`super::assemble_mesh_displacement`] once an axial-expansion
field is supplied.

```rust
pub mod mesh_solve { /* ... */ }
```

### Types

#### Struct `DisplacementReport`

Outcome of a segregated [`MechanicsMeshSolver::solve_displacement`] run.

The residual is the maximum per-cell change in the displacement field between
the last two outer correctors (metres); `converged` is `true` when that fell
below the configured tolerance before the corrector budget was spent.

```rust
pub struct DisplacementReport {
    pub n_correctors: usize,
    pub final_change: f64,
    pub converged: bool,
    pub max_displacement: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_correctors` | `usize` | Number of outer (segregated) correctors actually performed. |
| `final_change` | `f64` | Maximum per-cell displacement change on the final corrector (m). |
| `converged` | `bool` | Whether the segregated loop converged within the corrector budget. |
| `max_displacement` | `f64` | Peak displacement magnitude in the solved field (m). |

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
    fn clone(self: &Self) -> DisplacementReport { /* ... */ }
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
    fn eq(self: &Self, other: &DisplacementReport) -> bool { /* ... */ }
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
#### Struct `MechanicsMeshSolver`

Segregated finite-volume solver for `legacyThermoMechanics` on one mechanics
[`FvMesh`]: the displacement field `D` and the structure temperature
`TStruct`, with the thermo-elastic stress `σ` as output.

The solver owns its fields so that boundary conditions (set once on the
supplied `disp`/`t_struct` fields) persist across the outer correctors and
across time steps — only the internal cell values are updated in place.

# Units of the owned fields (raw SI, per the FV operator layer)

`outram_foam_basic_lib`'s field layer is un-`uom`'d, so field element values
are plain `f64` in SI base units: displacement `D` in **metres**, temperature
`TStruct` in **kelvin** (absolute), stress `σ` in **pascals**. The public
*scalar* configuration (`t_struct_ref`, `dt`) and summary getters use `uom`.

# Example

```no_run
# use std::sync::Arc;
# use outram_foam_basic_lib::mesh::FvMesh;
# use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
# use outram_foam_basic_lib::ldu_matrix::SolverSettings;
# use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::{
#     ElasticMaterial, LegacyThermoMechanics, RheologyOption, YoungsModulus, ThermalExpansionCoeff,
#     MechanicsMeshSolver,
# };
# use uom::si::f64::{
#     MassDensity, Ratio, SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature, Time,
# };
# use uom::si::{
#     mass_density::kilogram_per_cubic_meter, pressure::gigapascal, ratio::ratio,
#     specific_heat_capacity::joule_per_kilogram_kelvin, temperature_coefficient::per_kelvin,
#     thermal_conductivity::watt_per_meter_kelvin, thermodynamic_temperature::kelvin, time::second,
# };
# fn build(mesh: Arc<FvMesh>, disp: VolVectorField, t: VolScalarField) {
let steel = ElasticMaterial::new(
    YoungsModulus::new::<gigapascal>(200.0), Ratio::new::<ratio>(0.3),
    ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
    MassDensity::new::<kilogram_per_cubic_meter>(7850.0),
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
    ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
).unwrap();
let model = LegacyThermoMechanics::new(steel, RheologyOption::PlaneStrain);
let mut solver = MechanicsMeshSolver::new(
    model, ThermodynamicTemperature::new::<kelvin>(300.0),
    Time::new::<second>(1.0), SolverSettings::default(), disp, t,
);
solver.solve_temperature(None);
let report = solver.solve_displacement(true); // quasi-static
# let _ = report;
# }
```

```rust
pub struct MechanicsMeshSolver {
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
  pub fn new(model: LegacyThermoMechanics, t_struct_ref: ThermodynamicTemperature, dt: Time, settings: SolverSettings, disp: VolVectorField, t_struct: VolScalarField) -> Self { /* ... */ }
  ```
  Build a solver on the mesh carried by `disp`.

- ```rust
  pub fn set_corrector_control(self: &mut Self, n_correctors: usize, tol: f64) { /* ... */ }
  ```
  Override the segregated outer-corrector budget and convergence tolerance

- ```rust
  pub fn displacement(self: &Self) -> &VolVectorField { /* ... */ }
  ```
  The solved displacement field `D` (m).

- ```rust
  pub fn temperature(self: &Self) -> &VolScalarField { /* ... */ }
  ```
  The structure temperature field `TStruct` (K, absolute).

- ```rust
  pub fn stress(self: &Self) -> &VolSymmTensorField { /* ... */ }
  ```
  The thermo-elastic stress field `σ` (Pa) from the most recent

- ```rust
  pub fn solve_temperature(self: &mut Self, power_density: Option<&VolScalarField>) -> SolverPerformance { /* ... */ }
  ```
  Solve the steady structure heat-diffusion equation `−∇·(k ∇TStruct) = q'''`.

- ```rust
  pub fn solve_displacement(self: &mut Self, quasi_static: bool) -> DisplacementReport { /* ... */ }
  ```
  Solve the linear-elastic displacement equation `∇·σ = ρ ∂²D/∂t²` on the

- ```rust
  pub fn advance_time(self: &mut Self) { /* ... */ }
  ```
  Rotate the displacement history for the next transient step

- ```rust
  pub fn mesh_displacement(self: &Self, pin_direction: Vector3, axial: &VolScalarField) -> VolVectorField { /* ... */ }
  ```
  Assemble the coupled mesh-displacement field `meshDisp` (m) per cell.

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
    fn clone(self: &Self) -> MechanicsMeshSolver { /* ... */ }
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
## Module `stress`

# Linear-elastic thermal-stress constitutive law

The pointwise Hooke's-law constitutive relation of GeN-Foam's
`legacyThermoMechanics` — the tensor algebra that turns a displacement
gradient (small-strain tensor) and a temperature rise into a Cauchy stress
tensor and its von Mises equivalent. Ported from `include/calculateStress.H`
and the `sigmaD_` update in `include/solveThermalMechanics.H`.

## The relations

With small-strain tensor `ε = symm(∇u)` (dimensionless), shear modulus `μ`,
Lamé parameter `λ`, bulk-modulus term `3K`, expansion coefficient `α`, and
temperature rise above the stress-free reference `ΔT = T − T_ref`:

```text
  σ_D = 2μ ε + λ tr(ε) I              (isothermal deviatoric-plus-volumetric stress)
  σ   = σ_D − 3K α ΔT I               (full stress, thermal contraction removed)
  σ_eq = sqrt( (3/2) dev(σ) : dev(σ) )   (von Mises equivalent stress)
```

Upstream carries `σ_D` and `3Kα` in density-normalised (specific) units and
multiplies by `ρ` in `calculateStress.H` to recover physical stress:
`sigma = rho·sigmaD − I·rho·threeKalpha·(TStruct − TStructRef)`. This port
works in **physical pascals** directly, which is algebraically identical.

## Tensor representation

Stress and strain are symmetric second-order tensors. There is no `uom`
tensor type, so tensors use [`outram_foam_basic_lib`]'s dimensionless
[`SymmTensor`] with the physical unit documented in each signature (strain is
dimensionless; the returned stress tensor is in **pascals**). Scalar outputs
([`von_mises_stress`], [`hydrostatic_stress`]) are returned as `uom`
[`Pressure`] so the public boundary stays dimensioned.

```rust
pub mod stress { /* ... */ }
```

### Functions

#### Function `deviatoric_plus_volumetric_stress`

Isothermal stress tensor `σ_D = 2μ ε + λ tr(ε) I` (pascals).

The mechanical (non-thermal) part of the Cauchy stress for a small-strain
linear-elastic isotropic solid. Upstream:
`sigmaD_ = mu_*twoSymm(gradD) + lambda_*(I*tr(gradD))`, where
`twoSymm(∇u) = ∇u + ∇uᵀ = 2ε`.

# Parameters

- `strain` — small-strain tensor `ε = symm(∇u)` (dimensionless).
- `mu` — shear modulus `μ` (Pa).
- `lambda` — Lamé's first parameter `λ` (Pa).

# Returns

The stress tensor `σ_D` in **pascals** (as a [`SymmTensor`]).

```rust
pub fn deviatoric_plus_volumetric_stress(strain: outram_foam_basic_lib::primitives::SymmTensor, mu: super::material::ShearModulus, lambda: super::material::LameLambda) -> outram_foam_basic_lib::primitives::SymmTensor { /* ... */ }
```

#### Function `thermal_stress`

Full Cauchy stress tensor including the thermal term (pascals).

`σ = σ_D − 3K α ΔT I`, i.e. the isothermal stress with the isotropic
thermal-expansion contribution subtracted (upstream `calculateStress.H`:
`sigma = rho·sigmaD − I·rho·threeKalpha·(TStruct − TStructRef)`).

# Parameters

- `strain` — small-strain tensor `ε` (dimensionless).
- `mu`, `lambda` — Lamé parameters (Pa).
- `three_k` — bulk-modulus term `3K` (Pa).
- `alpha` — linear thermal-expansion coefficient `α` (1/K).
- `delta_t` — temperature rise `ΔT = T − T_ref` above the stress-free
  reference (kelvin interval).

# Returns

The full stress tensor `σ` in **pascals** (as a [`SymmTensor`]).

```rust
pub fn thermal_stress(strain: outram_foam_basic_lib::primitives::SymmTensor, mu: super::material::ShearModulus, lambda: super::material::LameLambda, three_k: super::material::BulkModulusTerm, alpha: super::material::ThermalExpansionCoeff, delta_t: uom::si::f64::TemperatureInterval) -> outram_foam_basic_lib::primitives::SymmTensor { /* ... */ }
```

#### Function `von_mises_stress`

von Mises equivalent stress `σ_eq = sqrt((3/2) dev(σ) : dev(σ))` (Pa).

Upstream: `sigmaEq = sqrt((3/2)*magSqr(dev(sigma)))`. The double-inner
product `dev(σ):dev(σ)` is `SymmTensor::mag_sqr` of the deviator.

# Parameters

- `sigma` — a stress tensor whose components are in **pascals**.

```rust
pub fn von_mises_stress(sigma: outram_foam_basic_lib::primitives::SymmTensor) -> super::material::Stress { /* ... */ }
```

#### Function `hydrostatic_stress`

Hydrostatic (mean) stress `σ_m = tr(σ)/3` (Pa).

Positive in tension. For a purely thermal, fully-constrained state this is
the whole stress (the deviator vanishes and `σ_eq = 0`).

# Parameters

- `sigma` — a stress tensor whose components are in **pascals**.

```rust
pub fn hydrostatic_stress(sigma: outram_foam_basic_lib::primitives::SymmTensor) -> super::material::Stress { /* ... */ }
```

### Types

#### Struct `LegacyThermoMechanics`

The linear-elastic thermo-mechanics model (`legacyThermoMechanics`) for a
single material zone: an [`ElasticMaterial`] plus its rheology idealisation.

This bundles the constitutive law and feedback relations of [`stress`] and
[`feedback`] against one material, so a caller supplies only kinematic /
thermal state (strain, temperature rise, geometry) at each query. It is the
per-zone constitutive kernel that [`mesh_solve`] evaluates cell-by-cell.

# Example

```
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::{
    ElasticMaterial, LegacyThermoMechanics, RheologyOption, YoungsModulus, ThermalExpansionCoeff,
};
use uom::si::f64::{
    Length, MassDensity, Ratio, SpecificHeatCapacity, TemperatureInterval, ThermalConductivity,
};
use uom::si::{
    length::meter, mass_density::kilogram_per_cubic_meter, pressure::gigapascal, ratio::ratio,
    specific_heat_capacity::joule_per_kilogram_kelvin, temperature_coefficient::per_kelvin,
    temperature_interval::kelvin, thermal_conductivity::watt_per_meter_kelvin,
};

let steel = ElasticMaterial::new(
    YoungsModulus::new::<gigapascal>(200.0),
    Ratio::new::<ratio>(0.3),
    ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
    MassDensity::new::<kilogram_per_cubic_meter>(7850.0),
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
    ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
)
.unwrap();
let model = LegacyThermoMechanics::new(steel, RheologyOption::PlaneStrain);

// A 2 m fuel rod heated 400 K above reference grows axially:
let dz = model.free_axial_expansion(
    TemperatureInterval::new::<kelvin>(400.0),
    Length::new::<meter>(2.0),
);
use uom::si::length::millimeter;
assert!((dz.get::<millimeter>() - 9.6).abs() < 1e-9); // α ΔT L = 1.2e-5·400·2 = 9.6 mm
```

```rust
pub struct LegacyThermoMechanics {
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
  pub fn new(material: ElasticMaterial, rheology: RheologyOption) -> Self { /* ... */ }
  ```
  Build the model from a material and a rheology idealisation.

- ```rust
  pub fn material(self: &Self) -> ElasticMaterial { /* ... */ }
  ```
  The underlying material card.

- ```rust
  pub fn rheology(self: &Self) -> RheologyOption { /* ... */ }
  ```
  The rheology idealisation (plane strain / plane stress).

- ```rust
  pub fn stress(self: &Self, strain: SymmTensor, delta_t: TemperatureInterval) -> SymmTensor { /* ... */ }
  ```
  Full Cauchy stress tensor `σ` (pascals) for a small-strain state `ε` at a

- ```rust
  pub fn von_mises(self: &Self, strain: SymmTensor, delta_t: TemperatureInterval) -> Stress { /* ... */ }
  ```
  von Mises equivalent stress `σ_eq` (Pa) for a small-strain state `ε` at a

- ```rust
  pub fn free_axial_expansion(self: &Self, delta_t: TemperatureInterval, length: Length) -> Displacement { /* ... */ }
  ```
  Free axial thermal expansion `α ΔT L` of a uniformly-heated bar of length

- ```rust
  pub fn axial_expansion_profile(self: &Self, delta_t: &[TemperatureInterval], cell_height: &[Length]) -> Vec<Displacement> { /* ... */ }
  ```
  Cumulative axial-displacement profile of a stack of cells at per-cell

- ```rust
  pub fn mesh_displacement(self: &Self, disp: Vector3, axial: Displacement, pin_direction: Vector3) -> Vector3 { /* ... */ }
  ```
  Assemble the coupled mesh-displacement vector `meshDisp` (metres) from an

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
    fn clone(self: &Self) -> LegacyThermoMechanics { /* ... */ }
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
    fn eq(self: &Self, other: &LegacyThermoMechanics) -> bool { /* ... */ }
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
#### Enum `ThermoMechanicsModel`

The set of thermo-mechanics models, dispatched by `enum` (no trait objects,
per workspace rules).

GeN-Foam selects between `legacyThermoMechanics` (the linear-elastic model
ported here) and `extendedThermoMechanics` (an OFFBEAT-based fuel-performance
model) at run time. Only the legacy model is ported; the extended model is a
separate, much larger subsystem and is **deferred**. The enum exists so that
adding it later forces every dispatch site to handle it (exhaustiveness).

```rust
pub enum ThermoMechanicsModel {
    Legacy(LegacyThermoMechanics),
}
```

##### Variants

###### `Legacy`

Linear-elastic multi-zone model (`legacyThermoMechanics`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LegacyThermoMechanics` |  |

##### Implementations

###### Methods

- ```rust
  pub fn stress(self: &Self, strain: SymmTensor, delta_t: TemperatureInterval) -> SymmTensor { /* ... */ }
  ```
  Full Cauchy stress tensor `σ` (Pa) for the given strain and temperature

- ```rust
  pub fn von_mises(self: &Self, strain: SymmTensor, delta_t: TemperatureInterval) -> Stress { /* ... */ }
  ```
  von Mises equivalent stress `σ_eq` (Pa), dispatched to the active model.

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
    fn clone(self: &Self) -> ThermoMechanicsModel { /* ... */ }
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
    fn eq(self: &Self, other: &ThermoMechanicsModel) -> bool { /* ... */ }
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
### Re-exports

#### Re-export `assemble_mesh_displacement`

```rust
pub use feedback::assemble_mesh_displacement;
```

#### Re-export `axial_expansion_profile`

```rust
pub use feedback::axial_expansion_profile;
```

#### Re-export `free_axial_expansion`

```rust
pub use feedback::free_axial_expansion;
```

#### Re-export `BulkModulusTerm`

```rust
pub use material::BulkModulusTerm;
```

#### Re-export `Displacement`

```rust
pub use material::Displacement;
```

#### Re-export `ElasticMaterial`

```rust
pub use material::ElasticMaterial;
```

#### Re-export `LameLambda`

```rust
pub use material::LameLambda;
```

#### Re-export `MaterialError`

```rust
pub use material::MaterialError;
```

#### Re-export `RheologyOption`

```rust
pub use material::RheologyOption;
```

#### Re-export `ShearModulus`

```rust
pub use material::ShearModulus;
```

#### Re-export `Stress`

```rust
pub use material::Stress;
```

#### Re-export `ThermalDiffusivity`

```rust
pub use material::ThermalDiffusivity;
```

#### Re-export `ThermalExpansionCoeff`

```rust
pub use material::ThermalExpansionCoeff;
```

#### Re-export `YoungsModulus`

```rust
pub use material::YoungsModulus;
```

#### Re-export `DisplacementReport`

```rust
pub use mesh_solve::DisplacementReport;
```

#### Re-export `MechanicsMeshSolver`

```rust
pub use mesh_solve::MechanicsMeshSolver;
```

#### Re-export `deviatoric_plus_volumetric_stress`

```rust
pub use stress::deviatoric_plus_volumetric_stress;
```

#### Re-export `hydrostatic_stress`

```rust
pub use stress::hydrostatic_stress;
```

#### Re-export `thermal_stress`

```rust
pub use stress::thermal_stress;
```

#### Re-export `von_mises_stress`

```rust
pub use stress::von_mises_stress;
```

## Module `io`

OpenFOAM case input/output — polyMesh and field readers, typed
`controlDict`/`fvSchemes`/`fvSolution`, and (unimplemented) field writers.
# `io` — OpenFOAM case input/output

Purpose-built Rust parsers and writers for the OpenFOAM ASCII case files, so
a case can be read into typed structs (invalid keys become `Result` errors,
not silent runtime fallbacks) and results written back out. No C++/FFI is
used — see `poly_mesh`'s header for the rationale.

## What actually reads and writes today

**Only the mesh and field readers work.** The three `system/` dictionary
parsers and every writer are `todo!()` and panic if called, so a case is
configured by constructing the structs in Rust and its results are read off
the solver's public fields rather than from disk.

| Module | Covers | Status |
|---|---|---|
| [`poly_mesh`] | `constant/polyMesh` (points/faces/owner/neighbour) | **Implemented** |
| [`field_reader`] | `0/<field>` internal fields, scalar and vector | **Implemented** |
| [`control_dict`] | `system/controlDict` time + write control | struct only; `read` is `todo!()` |
| [`fv_schemes`] | `system/fvSchemes` ddt/grad/div/laplacian choices | struct only; `read` is `todo!()` |
| [`fv_solution`] | `system/fvSolution` linear-solver + PIMPLE controls | struct only; `read` is `todo!()` |
| [`output`] | OpenFOAM-ASCII and legacy-VTK field writers | **all `todo!()`** |

The "struct only" rows are still useful: the dictionaries they model are
typed enums, so a scheme or solver selection that OpenFOAM would accept
silently and misinterpret is instead a compile error here.

```rust
pub mod io { /* ... */ }
```

### Modules

## Module `control_dict`

Typed equivalent of OpenFOAM's `system/controlDict` — the time-loop and
output control dictionary. [`ControlDict`] replaces the free-form text
dictionary with a struct whose start/stop/write controls are enums, so an
invalid selection cannot silently fall back to a default.

**Status: the struct exists, the on-disk parser does not.**
[`ControlDict::read`] is `todo!()`. Build a case with
`ControlDict::default()` and field assignment.

**Only three fields currently drive anything.** The solver loops in
[`crate::solvers`] read [`ControlDict::start`], [`ControlDict::stop`] and
[`ControlDict::delta_t`]. The write-control, `adjustTimeStep` and
`runTimeModifiable` fields are carried faithfully for a future output layer
but are **not consulted by any solver today** — each is flagged below.

```rust
pub mod control_dict { /* ... */ }
```

### Types

#### Struct `ControlDict`

The contents of an OpenFOAM `system/controlDict`, as a typed struct.

Construct with [`ControlDict::default`] and assign the fields you need;
[`ControlDict::read`] (parsing the file from disk) is not implemented.

See the module documentation for which fields the solvers actually honour.

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
| `application` | `String` | Name of the OpenFOAM application the case was written for (e.g.<br>`"pimpleFoam"`). Informational only — this crate never dispatches on it. |
| `start` | `StartControl` | Where the run starts. See [`StartControl`]. |
| `stop` | `StopControl` | Where the run stops. See [`StopControl`]. |
| `delta_t` | `f64` | Fixed time step Δt in seconds. Must be > 0. Because `adjust_time_step` is not<br>implemented (below), this is the step size for the whole run. |
| `write_control` | `WriteControl` | How often results should be written. See [`WriteControl`].<br><br>**Not honoured** — [`crate::io::output`] has no working writers, so no<br>solver writes anything to disk regardless of this value. |
| `write_interval` | `f64` | Interval used by `write_control` — steps for<br>[`WriteControl::TimeStep`], seconds otherwise. **Not honoured** (see<br>`write_control`). |
| `purge_write` | `usize` | Number of old time directories to retain (`0` = keep all).<br>**Not honoured** (see `write_control`). |
| `write_format` | `WriteFormat` | ASCII or binary field output. **Not honoured** (see `write_control`). |
| `write_precision` | `usize` | Significant figures for ASCII output. **Not honoured** (see<br>`write_control`). |
| `run_time_modifiable` | `bool` | Whether OpenFOAM would re-read the dictionary each step.<br>**Not honoured** — this crate holds the struct in memory and never<br>re-reads it. |
| `adjust_time_step` | `bool` | Whether the time step should adapt to `max_co` / `max_delta_t`.<br><br>**Not implemented.** There is no adaptive-Δt path in this crate: the<br>solver loops step at the fixed [`ControlDict::delta_t`] whatever this is<br>set to. Setting it `true` changes nothing — pick a Δt that satisfies<br>your Courant limit yourself. |
| `max_co` | `f64` | Target maximum Courant number for adaptive stepping [-].<br>**Not honoured** (see `adjust_time_step`). |
| `max_delta_t` | `f64` | Ceiling on the adaptive time step, in seconds.<br>**Not honoured** (see `adjust_time_step`). |

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

Where a run begins — OpenFOAM's `startFrom`.

Only [`StartControl::StartTime`] is acted on; the loops in
[`crate::solvers`] treat the other two as `t = 0`, because selecting a time
directory on disk needs field *reading per time step*, which this crate does
not do.

```rust
pub enum StartControl {
    StartTime(f64),
    LatestTime,
    FirstTime,
}
```

##### Variants

###### `StartTime`

Begin at this time, in seconds (`startFrom startTime`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `LatestTime`

Begin at the newest time directory present (`startFrom latestTime`).
**Treated as `t = 0`.**

###### `FirstTime`

Begin at the earliest time directory present (`startFrom firstTime`).
**Treated as `t = 0`.**

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

Where a run ends — OpenFOAM's `stopAt`.

Only [`StopControl::EndTime`] is acted on. The solver `run()` loops return
immediately (`Ok(())`, zero steps taken) for every other variant, because
each of them is defined in terms of a write that this crate cannot perform.

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

Stop once the simulated time reaches this value, in seconds (`stopAt endTime`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `WriteNow`

Stop and write immediately. **Runs zero steps** (see the enum docs).

###### `NoWriteNow`

Stop immediately without writing. **Runs zero steps.**

###### `NextWrite`

Stop at the next scheduled write. **Runs zero steps.**

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

How often results are written — OpenFOAM's `writeControl`.

**No variant is honoured**: [`crate::io::output`]'s writers are `todo!()`,
so a solver run produces no files. The enum exists so a case description is
complete and so the output layer, once written, has its selection already
typed.

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

Write every N time steps.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `RunTime`

Write every N seconds of *simulated* time.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `AdjustableRunTime`

Write every N seconds of simulated time, adjusting Δt to land exactly on
the write instants.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `CpuTime`

Write every N seconds of CPU time.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `ClockTime`

Write every N seconds of wall-clock time.

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

Field-file encoding — OpenFOAM's `writeFormat`. **Not honoured**; see
[`WriteControl`].

```rust
pub enum WriteFormat {
    Ascii,
    Binary,
}
```

##### Variants

###### `Ascii`

Human-readable ASCII.

###### `Binary`

Binary (smaller and faster, not diffable).

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

Parser for OpenFOAM's `system/fvSchemes` — the per-operator numerical scheme
selection dictionary. Each scheme family (ddt, grad, div, laplacian, snGrad,
interpolation) is a typed enum on [`FvSchemes`], so rust-analyzer surfaces
every valid option on hover and an unknown scheme is a `Result` error.

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
  Parse a `system/fvSchemes` file from disk.

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
    The defaults describe **what the solvers in this crate actually do** when

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

Parser for OpenFOAM's `system/fvSolution` — the linear-solver, PIMPLE/PISO
outer-loop, and under-relaxation control dictionary. Produces the typed
[`FvSolution`], with per-field [`LinearSolverConfig`] and the
[`PimpleControl`] loop parameters.

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
  Parse a `system/fvSolution` file from disk.

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

Writers for simulation results — OpenFOAM ASCII field files and legacy VTK
for ParaView.

**Status: all writers in this module are unimplemented scaffolds
(`todo!`)** — the signatures and intended file layouts are fixed, but
calling any of them currently panics.

```rust
pub mod output { /* ... */ }
```

### Functions

#### Function `write_scalar_field`

Write a scalar field to `<time_dir>/<field_name>` in OpenFOAM ASCII format.

**Not yet implemented — calling this panics (`todo!`).** The intended output
follows the standard OpenFOAM field file layout:
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

**Not yet implemented — calling this panics (`todo!`).**

```rust
pub fn write_vector_field(time_dir: &std::path::Path, field: &outram_foam_basic_lib::prelude::VolVectorField, dimensions: &str) -> Result<(), crate::error::AppBuilderError> { /* ... */ }
```

#### Function `write_vtk`

Write a legacy VTK unstructured grid file for ParaView.

**Not yet implemented — calling this panics (`todo!`).** When implemented it
will include mesh geometry and all provided scalar fields.

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

Re-exports of the crate's commonly used public items, for
`use outram_foam_appbuilder_lib::prelude::*;`.

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

#### Re-export `DecayConstant`

```rust
pub use crate::genfoam::neutronics::point_kinetics::DecayConstant;
```

#### Re-export `PointKineticsError`

```rust
pub use crate::genfoam::neutronics::point_kinetics::PointKineticsError;
```

#### Re-export `PointKineticsParameters`

```rust
pub use crate::genfoam::neutronics::point_kinetics::PointKineticsParameters;
```

#### Re-export `PointKineticsState`

```rust
pub use crate::genfoam::neutronics::point_kinetics::PointKineticsState;
```

#### Re-export `PromptGenerationTime`

```rust
pub use crate::genfoam::neutronics::point_kinetics::PromptGenerationTime;
```

#### Re-export `Reactivity`

```rust
pub use crate::genfoam::neutronics::point_kinetics::Reactivity;
```

#### Re-export `CrossSectionData`

```rust
pub use crate::genfoam::neutronics::xs::CrossSectionData;
```

#### Re-export `DiffusionNeutronics`

```rust
pub use crate::genfoam::neutronics::DiffusionNeutronics;
```

#### Re-export `NeutronicsModel`

```rust
pub use crate::genfoam::neutronics::NeutronicsModel;
```

#### Re-export `NeutronicsState`

```rust
pub use crate::genfoam::neutronics::NeutronicsState;
```

#### Re-export `LegacyThermoMechanics`

```rust
pub use crate::genfoam::thermo_mechanics::LegacyThermoMechanics;
```

#### Re-export `MechanicsMeshSolver`

```rust
pub use crate::genfoam::thermo_mechanics::MechanicsMeshSolver;
```

#### Re-export `MeshHandler`

```rust
pub use crate::genfoam::multi_region::MeshHandler;
```

#### Re-export `MultiPhysicsSolver`

```rust
pub use crate::genfoam::multi_region::MultiPhysicsSolver;
```

#### Re-export `Fluid`

```rust
pub use crate::genfoam::thermal_hydraulics::phase::Fluid;
```

#### Re-export `StateOfMatter`

```rust
pub use crate::genfoam::thermal_hydraulics::phase::StateOfMatter;
```

#### Re-export `HeatExchanger`

```rust
pub use crate::genfoam::thermal_hydraulics::structure::HeatExchanger;
```

#### Re-export `PowerModel`

```rust
pub use crate::genfoam::thermal_hydraulics::structure::PowerModel;
```

#### Re-export `HrmFoam`

```rust
pub use crate::solvers::hrm_foam::HrmFoam;
```

#### Re-export `HrmModelConfig`

```rust
pub use crate::solvers::hrm_foam::HrmModelConfig;
```

#### Re-export `MeltFoam`

```rust
pub use crate::solvers::melt_foam::MeltFoam;
```

#### Re-export `PimpleFoam`

```rust
pub use crate::solvers::pimple_foam::PimpleFoam;
```

#### Re-export `PressureSolver`

```rust
pub use crate::solvers::pimple_foam::PressureSolver;
```

#### Re-export `build_default`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::build_default as build_reacting_two_phase_euler;
```

#### Re-export `InterfacialHeatTransfer`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::InterfacialHeatTransfer;
```

#### Re-export `InterfacialMassTransfer`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::InterfacialMassTransfer;
```

#### Re-export `PhaseSelector`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::PhaseSelector;
```

#### Re-export `PhaseSpecies`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::PhaseSpecies;
```

#### Re-export `PhaseThermo`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::PhaseThermo;
```

#### Re-export `ReactingTwoPhaseEulerFoam`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::ReactingTwoPhaseEulerFoam;
```

#### Re-export `ReactionMechanism`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::ReactionMechanism;
```

#### Re-export `ReactionSource`

```rust
pub use crate::solvers::reacting_two_phase_euler_foam::ReactionSource;
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

#### Re-export `TurbulenceClosure`

```rust
pub use crate::turbulence::TurbulenceClosure;
```

## Module `solvers`

The ported OpenFOAM solver applications and their time-advancement loops.
# `solvers` — Layer-5 OpenFOAM solver-application ports

Each submodule is a self-contained Rust port of one OpenFOAM solver
application: it owns its time-advancement loop, assembles and solves the
governing equations each step, and enforces boundary conditions through
[`outram_foam_basic_lib`]'s `FvMesh`/`FvPatch` (never re-implementing BC
logic — see `bc_util`).

| Submodule | Ports | Regime |
|---|---|---|
| [`pimple_foam`] | pimpleFoam | Incompressible transient PIMPLE |
| [`melt_foam`] | pimpleFoam + `solidificationMelting` fvModel | Incompressible buoyant PIMPLE with phase change |
| [`rho_pimple_foam`] | rhoPimpleFoam | Compressible transient PIMPLE |
| [`sonic_foam`] | sonicFoam | Transonic/supersonic compressible |
| [`rho_central_foam`] | rhoCentralFoam | Density-based central-upwind (Kurganov-Tadmor) |
| [`hrm_foam`] | HRMFoam | Homogeneous relaxation two-phase flashing flow |
| [`reacting_two_phase_euler_foam`] | reactingTwoPhaseEulerFoam | Two-fluid Euler-Euler with reacting mass/heat transfer |

`bc_util` is a crate-internal helper for capturing and re-applying patch
boundary conditions around a field solve.

```rust
pub mod solvers { /* ... */ }
```

### Modules

## Module `hrm_foam`

# `hrm_foam` — Homogeneous Relaxation Model solver (HRMFoam)

Rust port of the HRMFoam flashing-flow solver: a compressible two-phase model
in which the vapour mass fraction relaxes toward its equilibrium value over a
finite relaxation time θ, using the Downar-Zapolski (1996) correlation. Used
for rapid depressurisation / flashing (e.g. blowdown) where mechanical
equilibrium holds but thermodynamic equilibrium lags.

The model constants ([`THETA_0`], [`DZ_A`], [`DZ_B`]) and their runtime
overrides ([`HrmModelConfig`]) are defined here; [`HrmFoam`] owns the time
loop.

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
| `model` | `HrmModelConfig` | The Downar-Zapolski relaxation-model constants this run uses.<br><br>Set through [`HrmFoam::with_model_config`]; [`HrmFoam::new`] installs<br>[`HrmModelConfig::default`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```
  Build a Homogeneous Relaxation Model two-phase flashing-flow solver on

- ```rust
  pub fn with_model_config(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution, model: HrmModelConfig) -> Self { /* ... */ }
  ```
  Build an HRM solver with explicit relaxation-model constants.

- ```rust
  pub fn relaxation_time(psi: f64, x: f64) -> f64 { /* ... */ }
  ```
  Downar-Zapolski relaxation time τ at a single point.

- ```rust
  pub fn relaxation_time_with_config(self: &Self, psi: f64, x: f64) -> f64 { /* ... */ }
  ```
  The Downar-Zapolski (1996) relaxation time θ, in seconds, evaluated with

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance one time step.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the solver from the `controlDict` start time to its end time,

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

## Module `melt_foam`

# meltFoam — incompressible buoyant PIMPLE with phase change

## What belongs here, and what does not

This module holds the **Layer-5 solver loop** that melting needs and that
[`pimple_foam`](super::pimple_foam) does not provide: a temperature
transport equation, and the wiring that lets an
[`FvModels`] collection contribute to *both* the momentum and the energy
equation of the same timestep.

The phase-change physics itself does **not** belong here — it lives in
`outram_foam_basic_lib::fv_options::SolidificationMelting`, which owns the
liquid fraction, the latent heat, the Darcy drag and the Boussinesq buoyancy.
This module only assembles equations and calls that model at the right points
in the timestep. Adding a mushy-zone correlation or a new drag law here would
be a layering mistake.

There is no upstream application called `meltFoam`, and this module does not
claim to be a port of one. Upstream runs this physics by attaching the
`solidificationMelting` **fvModel** to an existing buoyant solver through a
runtime dictionary; because this crate's solvers are Rust structs rather than
runtime-assembled dictionaries, the same composition has to be written out as
a named solver. The individual equations are upstream's — the PISO loop from
`applications/modules/incompressibleFluid/`, the energy equation from
`applications/modules/fluid/thermophysicalPredictor.C` — but their assembly
into one struct is this crate's, not a transcription of any single upstream
file.

## Governing equations

With kinematic pressure `p = p/ρ` \[m²/s²\], exactly as pimpleFoam:

`∂U/∂t + ∇·(UU) − ∇·(ν∇U) = −∇p + S_U`

`∇·U = 0`

`∂T/∂t + ∇·(φT) − ∇·(α_th ∇T) = S_T`

`S_U` and `S_T` are supplied entirely by the attached [`FvModels`]. For a
melting problem `S_U` is the Carman-Kozeny Darcy drag plus the Boussinesq
buoyancy, and `S_T` is the latent heat of fusion.

## The kinematic-units trap (upstream dimensional quirk — reproduced, not fixed)

Upstream's `solidificationMelting::addSup` has two momentum overloads, and
**the incompressible one simply calls the compressible one**:

```text
void addSup(const volVectorField& U, fvMatrix<vector>& eqn) const
{
    ...
    const scalar S  = -Cu_*sqr(1.0 - alpha1c)/(pow3(alpha1c) + q_);
    const vector Sb = rhoRef_*g*beta_*deltaT_[i];
    Sp[celli] += Vc*S;
    Su[celli] += Vc*Sb;
}
void addSup(const volScalarField& rho, const volVectorField& U,
            fvMatrix<vector>& eqn) const
{
    addSup(U, eqn);          // <-- identical coefficients, density ignored
}
```

Those coefficients are dimensionally consistent only with a **force-form**
(density-weighted) momentum equation: `Vc*Sb` carries \[N\] and `Vc*S`
carries \[kg/s\]. A kinematic momentum equation — the one pimpleFoam and this
solver assemble — needs \[m⁴/s²\] and \[m³/s\] respectively, i.e. both terms
divided by density.

Upstream does not divide. It offers no separate kinematic form and no
dimension check on this path, so a user attaching the model to an
incompressible solver silently gets both terms scaled by ρ unless they
compensate through the coefficients.

**This port reproduces the upstream behaviour rather than correcting it**,
per the workspace rule on upstream defects. The compensation is therefore the
caller's, and it is mechanical:

- set `reference_density = 1.0` (not the material density), and
- give `darcy_coefficient` in **kinematic** units \[1/s\], i.e. the
  literature `C_u` \[kg/(m³·s)\] divided by ρ.

[`MeltFoam::boussinesq_coefficients`] performs exactly that conversion from
physical inputs, so a caller never has to remember it. Reach for it rather
than filling
`SolidificationMeltingCoefficients` in by hand for this solver.

## Why `rho` is a field of ones

The temperature equation above is per unit volume with no ρ, matching
upstream's `addSup(he, eqn)` overload, which passes `geometricOneField()`.
This solver therefore hands [`FvModels::add_source_scalar`] a uniform field
of 1.0 — not the material density. Passing the real density would multiply
the latent-heat source by ρ (a factor of ~6000 for gallium) and freeze the
melt front in place.

```rust
pub mod melt_foam { /* ... */ }
```

### Types

#### Struct `MeltFoam`

Incompressible transient buoyant PIMPLE solver with phase change.

Solves the equations in the module documentation: a kinematic-pressure
PISO/PIMPLE velocity-pressure coupling, plus a temperature transport
equation, with an [`FvModels`] collection contributing to both.

# Units

Strict SI. `u` \[m/s\], `p` **kinematic** \[m²/s²\] (not Pa), `t` \[K\],
`phi` \[m³/s\], `nu` \[m²/s\], `alpha_thermal` \[m²/s\].

# Typical use

Build with [`new`](Self::new), set the fields and their boundary conditions,
attach a `SolidificationMelting` model built from
[`boussinesq_coefficients`](Self::boussinesq_coefficients), then call
[`step`](Self::step) in a loop or [`run`](Self::run) once.

```rust
pub struct MeltFoam {
    pub mesh: std::sync::Arc<FvMesh>,
    pub control: crate::io::control_dict::ControlDict,
    pub schemes: crate::io::fv_schemes::FvSchemes,
    pub solution: crate::io::fv_solution::FvSolution,
    pub u: VolVectorField,
    pub p: VolScalarField,
    pub t: VolScalarField,
    pub phi: SurfaceScalarField,
    pub nu: VolScalarField,
    pub alpha_thermal: VolScalarField,
    pub fv_models: FvModels,
    pub pressure_solver: crate::solvers::pimple_foam::PressureSolver,
    pub temperature_solver: SolverSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<FvMesh>` | The mesh, shared read-only. |
| `control` | `crate::io::control_dict::ControlDict` | Time control — start, stop and timestep. |
| `schemes` | `crate::io::fv_schemes::FvSchemes` | Discretisation schemes. |
| `solution` | `crate::io::fv_solution::FvSolution` | Linear-solver and PIMPLE-loop settings. |
| `u` | `VolVectorField` | Velocity \[m/s\]. |
| `p` | `VolScalarField` | Kinematic pressure `p/ρ` \[m²/s²\]. |
| `t` | `VolScalarField` | Temperature \[K\]. |
| `phi` | `SurfaceScalarField` | Face volumetric flux `φ = U·Sf` \[m³/s\]. |
| `nu` | `VolScalarField` | Kinematic viscosity `ν` \[m²/s\]. For gallium ~3.2e-7. |
| `alpha_thermal` | `VolScalarField` | Thermal diffusivity `α_th = k/(ρ·Cp)` \[m²/s\]. For liquid gallium<br>~1.3e-5, i.e. roughly 40x the momentum diffusivity — the low Prandtl<br>number that makes this problem convection-dominated. |
| `fv_models` | `FvModels` | Optional equation sources. A melting case attaches exactly one<br>`SolidificationMelting` model here. |
| `pressure_solver` | `crate::solvers::pimple_foam::PressureSolver` | Linear solver for the pressure Poisson equation. |
| `temperature_solver` | `SolverSettings` | Linear-solver settings for the **temperature** equation.<br><br># Why this is separate, and why the default is so tight<br><br>Defaults to `tolerance = 1e-12`, far tighter than<br>[`SolverSettings::default`]'s `1e-7`. That is not caution for its own<br>sake — it is a measured requirement.<br><br>The enthalpy-porosity scheme conserves energy *exactly* at the discrete<br>level: summing the temperature equation over all cells and all steps<br>telescopes to `Σ V·(Cp·ΔT + L·Δα) = Cp·Σ dt·(wall flux)`, with every<br>internal-face term cancelling. The only leak is the linear solve's own<br>residual, and a melting run is *long* — thousands to tens of thousands of<br>steps — so a per-step residual that is negligible in a short run<br>accumulates into a visible energy drift.<br><br>Measured on the 1-D Stefan case in this crate's `melting_vv_cases`<br>integration test (400 cells, dt = 0.01 s, 10 000 steps, 2026-08-05):<br><br>| T-solve tolerance | Δ(enthalpy) | ∫ wall heat | Imbalance |<br>|---|---|---|---|<br>| `1e-7` (the generic default) | 2111.019463 J/m² | 2130.665578 J/m² | -19.646 J/m², **-0.9221 %** |<br>| `1e-12` (this default) | 2128.217773 J/m² | 2128.218044 J/m² | -2.7108e-4 J/m², **-1.27e-5 %** |<br>| `1e-14` | — | 2128.218016 J/m² | -1.9647e-6 J/m², **-9.23e-8 %** |<br><br>A 0.9 % energy loss would be indistinguishable from a physics error while<br>being purely numerical, which is exactly the kind of drift that makes a<br>melting result untrustworthy. Loosen this only with a re-run of that<br>energy-balance check. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```
  Build a solver with zeroed fields on `mesh`.

- ```rust
  pub fn boussinesq_coefficients(solidus: f64, liquidus: f64, latent_heat: f64, specific_heat: f64, density: f64, thermal_expansion: f64, darcy_coefficient_force: f64) -> SolidificationMeltingCoefficients { /* ... */ }
  ```
  Build phase-change coefficients already converted to the **kinematic**

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the solution by one timestep.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Run from the start time to the end time in `control`.

- ```rust
  pub fn liquid_fraction(self: &Self) -> Option<&[f64]> { /* ... */ }
  ```
  The liquid fraction \[-\] of the first attached

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
    pub u_old_old: Option<VolVectorField>,
    pub turbulence: crate::turbulence::TurbulenceClosure,
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
| `u_old_old` | `Option<VolVectorField>` | Velocity at the time level *before* `u_old`, i.e. U^{n−2} [m/s].<br><br>Maintained automatically by [`PimpleFoam::step`] and needed only by the<br>second-order [`crate::io::fv_schemes::DdtScheme::Backward`] time scheme.<br>`None` before the first step has completed, where backward differencing<br>degenerates to Euler. |
| `turbulence` | `crate::turbulence::TurbulenceClosure` | Turbulence closure (default [`TurbulenceClosure::Laminar`]).<br><br>Selecting a RAS/LES model replaces the molecular viscous term in the<br>momentum predictor with `divDevReff(U) = −∇·(ν_eff ∇U) − ∇·(ν_eff<br>dev2(∇Uᵀ))`, ν_eff = ν + ν_t, and advances the model's transport<br>equations once per time step after the pressure correctors — OpenFOAM's<br>`turbulence->correct()` position. Set it directly, e.g.<br><br>```ignore<br>solver.turbulence = TurbulenceClosure::k_omega_sst(mesh.clone());<br>solver.turbulence.set_k_omega_uniform(1.0e-2, 100.0); // k [m²/s²], ω [1/s]<br>```<br><br>**The default is laminar and stays laminar unless you change it**, so an<br>existing run is unaffected. See [`crate::turbulence`] for the honest<br>scope limits — in particular, the closures use zero-gradient (not<br>wall-function) near-wall boundary conditions. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```
  Build an incompressible PIMPLE solver on `mesh`, with every field

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the solution by one time step using the PIMPLE algorithm.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the solver from the `controlDict` start time to its end time,

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
## Module `reacting_two_phase_euler_foam`

# reactingTwoPhaseEulerFoam — reacting two-phase Euler-Euler solver (app layer)

Application-layer translation of OpenFOAM's reacting two-phase Euler-Euler
solver. In the upstream we track (OpenFOAM-dev, the Foundation), the old
`reactingTwoPhaseEulerFoam` executable has been folded into the
runtime-selectable **`multiphaseEuler`** solver module
(`applications/modules/multiphaseEuler`, run via `foamRun -solver
multiphaseEuler`). This module ports that solver's **loop structure** and
its reacting-two-phase physics onto the pure-Rust finite-volume stack.

## What it composes vs. what it adds

Per the workspace **Layer-5 rule** (solver-loop logic lives in the
application crate; the finite-volume/physics building blocks live in the
libraries), this solver:

- **composes** the hydrodynamic core from [`outram_foam_multiphase`]:
  [`TwoFluidPimple`] already provides the two phase-momentum predictors, the
  shared-pressure PISO corrector (Rhie-Chow coupled), the interphase
  [`DragModel`], and dispersed volume-fraction transport
  (`α_d + α_c = 1`); and
- **adds** the reacting-Euler application layer that `multiphaseEuler` layers
  on top of the flow solve: per-phase **energy equations** with
  **interfacial heat transfer** ([`InterfacialHeatTransfer`]), **interfacial
  mass transfer / phase change** ([`InterfacialMassTransfer`]) with
  latent-heat coupling, and a **reaction heat source**
  ([`ReactionSource`]).

## Solver loop (mirrors `multiphaseEuler.C`)

Each [`solve_timestep`](ReactingTwoPhaseEulerFoam::solve_timestep) runs, in
order:

1. **pre-predictor + momentum-predictor + pressure-corrector** — delegated to
   [`TwoFluidPimple::solve_timestep`] (`α`-transport, both `UEqns`, the shared
   `pEqn`, flux/velocity correction).
2. **thermophysical-predictor** — a `nEnergyCorrectors` loop
   ([`thermophysical_predictor`](ReactingTwoPhaseEulerFoam::thermophysical_predictor))
   that assembles and solves both phase enthalpy equations with the
   one-resistance interfacial heat exchange and any reaction/latent sources,
   exactly the role of `multiphaseEuler`'s `energyPredictor()` /
   `thermophysicalPredictor()`.
3. **phase-change update** — an operator-split application of the interfacial
   mass-transfer rate `ṁ` to the volume fractions and the phase enthalpies
   (latent heat).

## Energy equation (per phase `k`, conservative enthalpy form)

Solving specific enthalpy `he_k = Cp_k · T_k` `[J/kg]`, mirroring
`rhoPimpleFoam`'s `heEqn` extended with the `α_k` weighting and interfacial
source (`multiphaseEuler` `thermophysicalPredictor.C:88`
`phase.heEqn() == heatTransfer[phase.name()]`):

$$ \frac{\partial(\alpha_k \rho_k he_k)}{\partial t} + \nabla\cdot(\alpha_k \rho_k \phi_k he_k) - \nabla\cdot\left(\frac{\alpha_k \kappa_k}{Cp_k}\nabla he_k\right) = K\,(T_o - T_k) + \dot q_{rxn,k} + \dot q_{lat,k} $$

with `K` `[W/(m³·K)]` the interfacial volumetric heat-transfer coefficient,
`T_o` the *other* phase's temperature, `q̇_rxn` the reaction heat, and
`q̇_lat` the latent-heat coupling. The interfacial term is split
**implicit in the own phase** (`K/Cp_k` on the diagonal) and **explicit in
the other phase** (`K·T_o` in the source), so at convergence of the corrector
loop the exchange is symmetric (`+K(T_o−T_k)` and `+K(T_k−T_o)` on the two
phases) and total thermal energy is conserved.

> **⚠️ Unverified until validated — foundation.** Verification-tested only
> (formula exactness, thermal-equilibration limit, energy conservation,
> source-term response). **No benchmark/experimental validation** — that is
> the human V&V step (workspace `RESPONSIBLE_USE.md`). Not for nuclear
> facility operation, reactor control, safety-critical, or licensing
> decisions. Independent OUTRAM PARK fork, not the official OpenFOAM software
> (see `TRADEMARKS.md`).

### Honest scope (what this foundation does *not* yet do)

- **Constant per-phase `Cp`, `κ`** (perfect-caloric closure `he = Cp·T`).
  Composition-resolved chemistry is available but reduced: an optional
  single-phase multicomponent [`PhaseSpecies`] transported with the phase
  mass flux + Fickian diffusion, plus a single global first-order Arrhenius
  [`ReactionMechanism`] (`fuel → product`, heat-release into that phase). A
  prescribed [`ReactionSource`] heat term is also kept for
  composition-free cases. Multi-step mechanisms, per-species properties /
  diffusivities, reversible/multi-phase reactions, and the full
  `phaseSystem` kinetics remain future work.
- **One-resistance** interfacial heat transfer (no interface-temperature
  two-resistance solve), **operator-split** phase change (the `ṁ` source is
  not folded into the implicit `α`-transport matrix), and no population
  balance, MRF, LTS, or buoyant `p_rgh` split.
- Enum dispatch (no `dyn`), no lifetime parameters, `uom` at the public
  boundary, documented raw `f64` (SI) in the inner assembly loops — per the
  workspace `CLAUDE.md`.

```rust
pub mod reacting_two_phase_euler_foam { /* ... */ }
```

### Types

#### Struct `PhaseThermo`

Constant-property thermal state carried for one phase: its specific enthalpy
field `he_k = Cp_k·T_k` `[J/kg]` plus the constant caloric properties needed
to close the energy equation.

The temperature is recovered as `T_k = he_k / Cp_k` (reference `0 K`); only
enthalpy *differences* are physically meaningful in this foundation, which is
all the interfacial exchange, latent heat, and reaction terms depend on.

```rust
pub struct PhaseThermo {
    pub he: VolScalarField,
    pub he_old: VolScalarField,
    pub cp: f64,
    pub kappa: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `he` | `VolScalarField` | Specific enthalpy `he_k` `[J/kg]`, the solved energy variable. |
| `he_old` | `VolScalarField` | Old-time specific enthalpy for the `∂/∂t` term. |
| `cp` | `f64` | Isobaric specific heat capacity `Cp_k` `[J/(kg·K)]` (constant, > 0). |
| `kappa` | `f64` | Thermal conductivity `κ_k` `[W/(m·K)]` (constant, ≥ 0). |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(mesh: Arc<FvMesh>, name: impl Into<String>, cp: SpecificHeatCapacity, kappa: ThermalConductivity, t0: ThermodynamicTemperature) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Build a phase thermal state from constant properties and a uniform

- ```rust
  pub fn temperature(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Temperature field `T_k = he_k / Cp_k` `[K]` (zero-gradient boundaries).

- ```rust
  pub fn mean_temperature(self: &Self) -> f64 { /* ... */ }
  ```
  Volume-averaged temperature `[K]` — a diagnostic used by the tests and by

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
#### Enum `InterfacialHeatTransfer`

Interfacial **heat-transfer** closure supplying the volumetric coefficient
`K` `[W/(m³·K)]` in the per-phase energy source `K·(T_o − T_k)`.

Ports the dispersed-sphere family of `heatTransferModels`
(`multiphaseEuler/phaseSystem/interfacialModels/heatTransferModels/`). The
coefficient follows the ported form
(`RanzMarshall.C`: `K = 6·max(α_d,α_res)·κ_c·Nu/d²`), with the Nusselt
number set by the variant:

- [`Spherical`](Self::Spherical) — `Nu = 2` (pure conduction limit of a
  sphere; `sphericalHeatTransfer`).
- [`RanzMarshall`](Self::RanzMarshall) — `Nu = 2 + 0.6·√Re·∛Pr`
  (`RanzMarshall.C:56`), the standard forced-convection correlation.
- [`ConstantNu`](Self::ConstantNu) — a prescribed constant Nusselt number.

Enum dispatch (not `dyn`) per the workspace design rules.

```rust
pub enum InterfacialHeatTransfer {
    Spherical,
    RanzMarshall,
    ConstantNu(f64),
}
```

##### Variants

###### `Spherical`

Conduction-limit sphere, `Nu = 2`.

###### `RanzMarshall`

Ranz-Marshall forced-convection correlation `Nu = 2 + 0.6·√Re·∛Pr`.

###### `ConstantNu`

Prescribed constant Nusselt number `Nu`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn nusselt(self: &Self, re: f64, pr: f64) -> f64 { /* ... */ }
  ```
  Nusselt number for a single cell given the local slip Reynolds number

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
    fn clone(self: &Self) -> InterfacialHeatTransfer { /* ... */ }
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
#### Enum `InterfacialMassTransfer`

Interfacial **mass-transfer** (phase-change) closure supplying the
volumetric rate `ṁ` `[kg/(m³·s)]` transferred **from the dispersed phase to
the continuous phase** (positive `ṁ` = dispersed evaporating/dissolving into
the continuous carrier; negative = condensation onto the dispersed phase).

Applied **operator-split** after the energy predictor: the volume fractions
are updated by `Δα_d = −ṁ·Δt/ρ_d` and the released/absorbed latent heat
`L·ṁ` is added to the continuous phase enthalpy (see
[`ReactingTwoPhaseEulerFoam::latent_heat`]). This is a foundation stand-in
for the phaseSystem's implicit mass-transfer coupling.

Enum dispatch (not `dyn`).

```rust
pub enum InterfacialMassTransfer {
    None,
    ConstantRate(f64),
}
```

##### Variants

###### `None`

No phase change (`ṁ = 0`).

###### `ConstantRate`

A prescribed spatially-uniform rate `ṁ` `[kg/(m³·s)]` (dispersed →
continuous). For controlled tests and cases where the rate is supplied
externally.

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
    fn clone(self: &Self) -> InterfacialMassTransfer { /* ... */ }
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
#### Enum `PhaseSelector`

Which phase a volumetric source is deposited into.

```rust
pub enum PhaseSelector {
    Dispersed,
    Continuous,
}
```

##### Variants

###### `Dispersed`

The dispersed phase `d`.

###### `Continuous`

The continuous phase `c`.

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
    fn clone(self: &Self) -> PhaseSelector { /* ... */ }
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
    fn eq(self: &Self, other: &PhaseSelector) -> bool { /* ... */ }
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
#### Enum `ReactionSource`

**Reaction heat source** — a prescribed volumetric heat-release rate added to
one phase's energy equation. Stands in for composition-resolved finite-rate
chemistry (not modelled at this foundation stage; see the module "Honest
scope").

Enum dispatch (not `dyn`).

```rust
pub enum ReactionSource {
    None,
    VolumetricHeat {
        phase: PhaseSelector,
        q: f64,
    },
}
```

##### Variants

###### `None`

No reaction.

###### `VolumetricHeat`

Uniform volumetric heat release `q̇` `[W/m³]` deposited into the selected
phase.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `phase` | `PhaseSelector` | Phase the heat is released into. |
| `q` | `f64` | Volumetric heat-release rate `q̇` `[W/m³]` (positive = heating). |

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
    fn clone(self: &Self) -> ReactionSource { /* ... */ }
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
#### Struct `PhaseSpecies`

Multicomponent **composition** carried inside one phase: a set of species
mass fractions `Y_i` `[-]` transported with that phase's mass flux plus a
single Fickian diffusivity. This is the composition the finite-rate
[`ReactionMechanism`] acts on — the "reacting" content the prescribed
[`ReactionSource`] heat term stands in for when no composition is modelled.

Mirrors `multiphaseEuler`'s per-phase `Y` fields (`thermophysicalPredictor.C`
`compositionPredictor()` — `phase.YiEqn(Y[i]) == …`), reduced to a single
phase with a shared constant diffusivity.

```rust
pub struct PhaseSpecies {
    pub phase: PhaseSelector,
    pub names: Vec<String>,
    pub y: Vec<VolScalarField>,
    pub y_old: Vec<VolScalarField>,
    pub diffusivity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `phase` | `PhaseSelector` | Which phase carries this composition. |
| `names` | `Vec<String>` | Species names, one per transported mass fraction. |
| `y` | `Vec<VolScalarField>` | Species mass-fraction fields `Y_i` `[-]`, `Σ_i Y_i = 1`. |
| `y_old` | `Vec<VolScalarField>` | Old-time mass fractions for the `∂/∂t` term. |
| `diffusivity` | `f64` | Fickian mass diffusivity `D` `[m²/s]` (constant, ≥ 0), shared by all<br>species. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, phase: PhaseSelector, names: Vec<String>, y0: &[f64], diffusivity: f64) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Build a composition from species names and uniform initial mass

- ```rust
  pub fn mass_fraction(self: &Self, i: usize) -> &VolScalarField { /* ... */ }
  ```
  Mass-fraction field of species `i` `[-]`.

- ```rust
  pub fn mean_mass_fraction(self: &Self, i: usize) -> f64 { /* ... */ }
  ```
  Volume-averaged mass fraction of species `i` `[-]` (diagnostic).

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
#### Enum `ReactionMechanism`

Finite-rate homogeneous **reaction mechanism** acting on a [`PhaseSpecies`]
composition and releasing heat into that phase's energy equation.

Enum dispatch (not `dyn`).

```rust
pub enum ReactionMechanism {
    None,
    Arrhenius {
        fuel: usize,
        product: usize,
        a_pre: f64,
        e_act: f64,
        delta_h: f64,
    },
}
```

##### Variants

###### `None`

No reaction (species are transported inertly).

###### `Arrhenius`

A single global irreversible reaction `fuel → product` with first-order
Arrhenius kinetics. The volumetric fuel consumption rate is

```text
ω = A · exp(−Ea/(R·T)) · ρ · Y_fuel     [kg/(m³·s)]
```

(`A` pre-exponential `[1/s]`, `Ea` activation energy `[J/mol]`). Species
sources are `−ω` (fuel) and `+ω` (product); the heat release is
`q̇ = ΔH · ω` `[W/m³]` deposited into the reacting phase's enthalpy.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `fuel` | `usize` | Index of the fuel species in [`PhaseSpecies::y`]. |
| `product` | `usize` | Index of the product species in [`PhaseSpecies::y`]. |
| `a_pre` | `f64` | Pre-exponential factor `A` `[1/s]`. |
| `e_act` | `f64` | Activation energy `Ea` `[J/mol]`. |
| `delta_h` | `f64` | Heat of reaction `ΔH` per unit fuel mass `[J/kg]` (positive =<br>exothermic). |

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
    fn clone(self: &Self) -> ReactionMechanism { /* ... */ }
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
#### Struct `ReactingTwoPhaseEulerFoam`

Reacting two-phase Euler-Euler solver (application layer) — see the module
documentation for the algorithm, provenance, and honest scope.

```rust
pub struct ReactingTwoPhaseEulerFoam {
    pub hydro: outram_foam_multiphase::two_fluid_pimple::TwoFluidPimple,
    pub dispersed_thermo: PhaseThermo,
    pub continuous_thermo: PhaseThermo,
    pub heat_transfer: InterfacialHeatTransfer,
    pub mass_transfer: InterfacialMassTransfer,
    pub reaction: ReactionSource,
    pub species: Option<PhaseSpecies>,
    pub reaction_mechanism: ReactionMechanism,
    pub latent_heat: f64,
    pub residual_alpha: f64,
    pub n_energy_correctors: usize,
    pub control: crate::io::control_dict::ControlDict,
    pub schemes: crate::io::fv_schemes::FvSchemes,
    pub solution: crate::io::fv_solution::FvSolution,
    pub time: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hydro` | `outram_foam_multiphase::two_fluid_pimple::TwoFluidPimple` | Hydrodynamic core (two phase momenta + shared pressure + `α`-transport +<br>drag), composed from [`outram_foam_multiphase`]. |
| `dispersed_thermo` | `PhaseThermo` | Thermal state of the dispersed phase. |
| `continuous_thermo` | `PhaseThermo` | Thermal state of the continuous phase. |
| `heat_transfer` | `InterfacialHeatTransfer` | Interfacial heat-transfer closure. |
| `mass_transfer` | `InterfacialMassTransfer` | Interfacial mass-transfer (phase-change) closure. |
| `reaction` | `ReactionSource` | Prescribed reaction heat source (composition-free stand-in). |
| `species` | `Option<PhaseSpecies>` | Optional composition-resolved species transported in one phase. When set<br>(with a non-[`None`](ReactionMechanism::None)<br>[`reaction_mechanism`](Self::reaction_mechanism)) the solver runs a<br>composition predictor before the energy predictor, exactly as<br>`multiphaseEuler` does. |
| `reaction_mechanism` | `ReactionMechanism` | Finite-rate reaction mechanism acting on [`species`](Self::species);<br>releases heat into that phase's enthalpy equation. |
| `latent_heat` | `f64` | Latent heat of the dispersed→continuous phase change `L` `[J/kg]`<br>(absorbed on evaporation `ṁ > 0`). Only used when<br>[`mass_transfer`](Self::mass_transfer) is active. |
| `residual_alpha` | `f64` | Residual volume-fraction floor `α_res` `[-]` guarding the `max(α,α_res)`<br>in the interfacial coefficient and the per-phase energy denominators<br>(mirrors OpenFOAM's `residualAlpha`). Must be `> 0`. |
| `n_energy_correctors` | `usize` | Number of energy correctors per time step (`nEnergyCorrectors`, ≥ 1). |
| `control` | `crate::io::control_dict::ControlDict` | Time-control dictionary. |
| `schemes` | `crate::io::fv_schemes::FvSchemes` | Discretisation schemes. |
| `solution` | `crate::io::fv_solution::FvSolution` | Linear-solver / PIMPLE control. |
| `time` | `f64` | Current simulation time `[s]`. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(hydro: TwoFluidPimple, dispersed_thermo: PhaseThermo, continuous_thermo: PhaseThermo, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Result<Self, MultiphaseError> { /* ... */ }
  ```
  Assemble a reacting two-phase Euler solver from a hydrodynamic core and

- ```rust
  pub fn dispersed_temperature(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Dispersed-phase temperature field `[K]`.

- ```rust
  pub fn continuous_temperature(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Continuous-phase temperature field `[K]`.

- ```rust
  pub fn solve_timestep(self: &mut Self, dt: f64) -> Result<(), MultiphaseError> { /* ... */ }
  ```
  Advance the coupled flow, energy, and phase-change state by one time step

- ```rust
  pub fn thermophysical_predictor(self: &mut Self, dt: f64) -> Result<(), MultiphaseError> { /* ... */ }
  ```
  Energy predictor loop — assemble and solve both phase enthalpy equations

- ```rust
  pub fn composition_predictor(self: &mut Self, dt: f64) -> Result<(), MultiphaseError> { /* ... */ }
  ```
  Composition predictor — advance the [`species`](Self::species) mass

- ```rust
  pub fn apply_mass_transfer(self: &mut Self, dt: f64) -> Result<(), MultiphaseError> { /* ... */ }
  ```
  Operator-split interfacial mass transfer: update the volume fractions by

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Run the transient loop from the control dictionary's start time to its end

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
### Functions

#### Function `build_default`

Convenience constructor: a [`DragModel`]-coupled hydrodynamic core plus the
two thermal states, assembled into a ready-to-step solver with default
controls. Kept out of `new` so the primary constructor stays I/O-driven.

```rust
pub fn build_default(system: outram_foam_multiphase::two_fluid::TwoFluidSystem, drag: outram_foam_multiphase::two_fluid::DragModel, dispersed_thermo: PhaseThermo, continuous_thermo: PhaseThermo) -> Result<ReactingTwoPhaseEulerFoam, outram_foam_multiphase::MultiphaseError> { /* ... */ }
```

## Module `rho_central_foam`

# `rho_central_foam` — density-based compressible solver (rhoCentralFoam)

Rust port of rhoCentralFoam: a density-based, explicit compressible solver
using the Kurganov-Noelle-Petrova (KNP) central-upwind flux with 2nd-order
van Leer MUSCL reconstruction. It advances the conserved compressible fields
(ρ, ρU, ρE) and is well suited to shock-dominated high-speed flow (the Sod
shock-tube validation case exercises this solver). See [`RhoCentralFoam`].

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
  Build a density-based central-upwind (Kurganov-Noelle-Petrova) compressible

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  One explicit KNP time step.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the solver from the `controlDict` start time to its end time,

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

# `rho_pimple_foam` — compressible transient PIMPLE solver (rhoPimpleFoam)

Rust port of rhoPimpleFoam: the compressible counterpart of pimpleFoam,
solving continuity, momentum, and (enthalpy-form) energy with a
compressibility-consistent pressure equation. Suited to subsonic compressible
transient flow. See [`RhoPimpleFoam`] for the governing equations and time
loop; a companion stability primer lives in this module's `docs/`.

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
    pub turbulence: crate::turbulence::TurbulenceClosure,
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
| `turbulence` | `crate::turbulence::TurbulenceClosure` | Turbulence closure (default [`TurbulenceClosure::Laminar`]).<br><br>Selecting a RAS/LES model makes the momentum viscous term use the<br>effective **dynamic** viscosity μ_eff = μ + ρ ν_t [Pa·s] and the energy<br>equation use α_eff = α + ρ ν_t / Pr_t [kg/(m·s)], and advances the<br>model's transport equations once per time step after the pressure<br>correctors.<br><br>The closures in `outram-foam-turbulence-lib` are formulated<br>**kinematically**, so this solver feeds them ν = μ/ρ and the<br>**volumetric** flux φ/ρ_f, and converts ν_t back with μ_t = ρ ν_t. That<br>mapping is the constant-density approximation to OpenFOAM's compressible<br>`fvm::div(alphaRhoPhi, k)` form — exact only where ρ is uniform. See<br>[`crate::turbulence`] for the full scope limits.<br><br>**The default is laminar**, so an existing run is unaffected. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, control: ControlDict, schemes: FvSchemes, solution: FvSolution) -> Self { /* ... */ }
  ```
  Build a compressible PIMPLE solver on `mesh`, with every field allocated

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance one time step with compressible PIMPLE.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the solver from the `controlDict` start time to its end time,

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
## Module `schemes`

Applying `fvSchemes` selections (ddt / div) to an assembled equation.
See [`schemes::ddt_vec_scheme`] and [`schemes::div_vec_scheme`].
# Applying `fvSchemes` selections to the assembled equations

[`crate::io::fv_schemes::FvSchemes`] parses OpenFOAM's scheme dictionary into
typed enums. This module is what makes those enums *do* something: it turns a
[`DdtScheme`] or [`DivScheme`] into the corresponding discretisation on a
momentum equation.

Before this module existed, every solver in this crate stored an `FvSchemes`
and then ignored it — the time derivative was hardwired to Euler and
convection to first-order upwind, whatever the dictionary said. A scheme
selection that is silently discarded is worse than none, because it reads as
a promise.

## What is implemented, and what returns an error

Unimplemented schemes return
[`AppBuilderError::UnsupportedScheme`](crate::error::AppBuilderError::UnsupportedScheme)
rather than falling back to a default. A silent fallback would reintroduce
exactly the failure mode this module exists to remove.

| Family | Implemented | Returns `UnsupportedScheme` |
|---|---|---|
| ddt | `Euler`, `Backward`, `SteadyState` | `CrankNicolson`, `LocalEuler` |
| div | `GaussUpwind`, `GaussLinear` | `GaussLinearUpwind`, `GaussVanLeer`, `GaussMUSCL`, `GaussLimitedLinear` |

The limited/TVD `div` schemes need a face limiter driven by a reconstructed
upwind gradient; `outram-foam-basic-lib` provides the limiter functions
(`limiters::FluxLimiter`) but not the face-`r` reconstruction for a vector
field, so they are declared unsupported rather than approximated.

```rust
pub mod schemes { /* ... */ }
```

### Functions

#### Function `ddt_vec_scheme`

Assemble the transient term `∂U/∂t` of a vector transport equation according
to `scheme`.

The returned [`FvVectorMatrix`] is *volume-integrated*: its diagonal carries
`V/Δt`-like coefficients [m³/s] and its source `V·U/Δt` [m⁴/s²], matching
`fvm::ddt_vec` and the rest of this crate's assembly convention.

# Arguments

* `scheme`     — the `ddtSchemes` selection.
* `u`          — the velocity field being solved for, U^n [m/s].
* `u_old`      — U at the previous time level, U^{n−1} [m/s].
* `u_old_old`  — U at the level before that, U^{n−2} [m/s]. Required by
  [`DdtScheme::Backward`]; pass `None` on the first step of a run, where this
  function falls back to Euler for that step (the standard second-order
  backward start-up, since U^{n−2} does not exist yet).
* `dt`         — time step Δt [s], must be > 0.
* `mesh`       — the mesh.

# Schemes

* [`DdtScheme::Euler`] — first-order implicit Euler, `(U^n − U^{n−1})/Δt`.
  Bounded and unconditionally stable; the default.
* [`DdtScheme::Backward`] — second-order backward differencing,
  `(3U^n − 4U^{n−1} + U^{n−2})/(2Δt)`. Diagonal coefficient `1.5 V/Δt`,
  source `V(2U^{n−1} − 0.5U^{n−2})/Δt`. More accurate but less bounded; can
  ring on a coarse mesh at high Courant number.

  **Known limitation — `Backward` is wired, not verified as second order.**
  [`crate::solvers::pimple_foam::PimpleFoam`]'s PISO loop adds a Rhie–Chow
  flux correction `rAU_f · fvc::ddt_corr(U_old, φ_old, Δt)`, and
  `outram-foam-basic-lib`'s `ddt_corr` implements only the **Euler** form.
  Selecting `Backward` changes `rAU = V/A` (the diagonal becomes `1.5 V/Δt`
  instead of `V/Δt`) without changing `ddt_corr`, so the two are
  inconsistent by a ratio tending to 1.5 as Δt → 0. Measured consequence
  (`tests/fv_scheme_selection.rs`, 2026-08-07): the Euler and Backward
  lid-driven-cavity runs converge to steady states differing by 1.0e-2 to
  2.9e-2 m/s (1–3 % of U_lid), and that difference *grows* as Δt is
  refined — the signature of an inconsistency, not of truncation error.
  Fixing it needs OpenFOAM's `backwardDdtScheme::fvcDdtPhiCorr`, which
  belongs in `outram-foam-basic-lib`. Until then, treat `Backward` as
  available and benchmark-neutral (it scores marginally better than Euler
  against Ghia 1982 at every Δt tested), but not as a verified second-order
  time integration.
* [`DdtScheme::SteadyState`] — the term is dropped entirely (a zero matrix),
  for steady solvers.

# Errors

[`AppBuilderError::UnsupportedScheme`] for `CrankNicolson` (needs the stored
off-centred `ddt0` field OpenFOAM keeps between steps) and `LocalEuler`
(needs a per-cell local time-step field).

```rust
pub fn ddt_vec_scheme(scheme: &crate::io::fv_schemes::DdtScheme, u: &VolVectorField, u_old: &VolVectorField, u_old_old: Option<&VolVectorField>, dt: f64, mesh: std::sync::Arc<FvMesh>) -> Result<FvVectorMatrix, crate::error::AppBuilderError> { /* ... */ }
```

#### Function `div_vec_scheme`

Assemble the convection term `∇·(φ U)` of a vector transport equation
according to `scheme`.

# Arguments

* `scheme` — the `divSchemes` selection for `div(phi,U)`.
* `phi`    — face flux φ = U·S_f [m³/s] (volumetric) or ρU·S_f [kg/s]
  (mass); the scheme is agnostic, the units follow `phi`.
* `u`      — the transported velocity field [m/s].
* `mesh`   — the mesh.

# Schemes

* [`DivScheme::GaussUpwind`] — first-order upwind, `fvm::div_vec` unmodified.
  Unconditionally bounded, strongly diffusive on a coarse mesh.
* [`DivScheme::GaussLinear`] — second-order central differencing, assembled
  by **deferred correction**: the implicit matrix stays the (diagonally
  dominant) upwind operator, and the difference between the linear and upwind
  face values is added explicitly to the source,

  ```text
    D_f = φ_f (U_f^linear − U_f^upwind)
    source[owner]     −= D_f
    source[neighbour] += D_f
  ```

  This is the standard Khosla–Rubin treatment: it recovers second-order
  accuracy at convergence while keeping the matrix M-like. It is **not**
  bounded — central differencing on a convection-dominated coarse mesh can
  produce over/undershoots.

  Boundary faces are left to `fvm::div_vec`: on a zero-gradient patch the
  linear and upwind face values coincide (both equal the owner value), and on
  a fixed-value patch `fvm::div_vec` already uses the prescribed value, so
  there is no correction to apply either way.

# Errors

[`AppBuilderError::UnsupportedScheme`] for the limited/TVD schemes
(`linearUpwind`, `vanLeer`, `MUSCL`, `limitedLinear`) — see the module
documentation.

```rust
pub fn div_vec_scheme(scheme: &crate::io::fv_schemes::DivScheme, phi: &SurfaceScalarField, u: &VolVectorField, mesh: std::sync::Arc<FvMesh>) -> Result<FvVectorMatrix, crate::error::AppBuilderError> { /* ... */ }
```

## Module `sonic_foam`

# `sonic_foam` — transonic/supersonic compressible solver (sonicFoam)

Rust port of sonicFoam: a pressure-based compressible solver for trans- and
supersonic flow of a single-phase gas, using the compressibility ψ = ρ/p as
the primary thermodynamic closure. See [`SonicFoam`] for the pressure
equation and the current explicit-convection limitation.

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
  Build a transonic/supersonic ψ-based compressible solver on `mesh`, with

- ```rust
  pub fn step(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance one time step.

- ```rust
  pub fn run(self: &mut Self) -> Result<(), AppBuilderError> { /* ... */ }
  ```
  Advance the solver from the `controlDict` start time to its end time,

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
## Module `turbulence`

Turbulence-closure selection for the solver loops — the Layer-5 adapter over
`outram-foam-turbulence-lib`. See [`turbulence::TurbulenceClosure`].
# Turbulence-closure selection for the solver loops (Layer 5 adapter)

This module is the **bridge** between `outram-foam-turbulence-lib` (Layer 4 —
the RAS/LES closures themselves) and the solver loops in
[`crate::solvers`] (Layer 5 — PIMPLE/PISO time advancement).

## What belongs here and what does not

*Here:* selecting which closure a run uses, pushing the solver's live fields
into the closure once per outer iteration, calling `correct()` at the right
point in the PIMPLE loop, and converting between the closure's
**kinematic** view (ν, ν_t in m²/s) and a compressible solver's **dynamic**
view (μ, μ_t in Pa·s).

*Not here:* the turbulence transport equations themselves. Those live in
`outram-foam-turbulence-lib` and are not duplicated.

## Dispatch is by enum, never `dyn`

[`TurbulenceClosure`] is a plain enum, as the workspace design rules require.
Adding a model is a compile-time forcing function: every `match` in this file
must gain a new arm or the crate does not build. There is no `Box<dyn
TurbulenceModel>` anywhere.

## How a solver uses it

```text
for each outer (PIMPLE) iteration:
    turbulence.sync_inputs(&U, &phi_volumetric, &nu, dt);   // push live state
    UEqn = ddt + div + turbulence.div_dev_reff(&U, &nu);    // turbulent stress
    ... momentum predictor, PISO pressure correctors ...
end outer loop
turbulence.sync_inputs(&U, &phi_volumetric, &nu, dt);       // corrected state
turbulence.correct();                                       // advance k, ω/ε, ν_t
```

`correct()` is deliberately called **after** the pressure correctors, exactly
as OpenFOAM's `turbulence->correct()` sits at the bottom of the PIMPLE loop.

## Honest scope — read before trusting a turbulent result

- The closures use **zero-gradient near-wall boundary conditions**, not wall
  functions. `outram-foam-turbulence-lib` ships `wall_functions::{y_plus,
  u_tau, nu_t_wall}` as standalone helpers that are **not** wired in as patch
  boundary conditions, by that crate's own admission. A wall-bounded RAS run
  therefore does **not** reproduce the log law and **must not** be compared
  against a friction-factor correlation and called validated.
- What *is* verified here (see the tests at the bottom of this file) is the
  **coupling**: that the momentum equation actually picks up ν_t, and that a
  closure advanced inside the PIMPLE loop reproduces the analytic solution of
  its own transport equations for a case with no walls and no shear.
- No model in this stack has been validated end-to-end against a published
  turbulence benchmark. Do not describe one as validated.

```rust
pub mod turbulence { /* ... */ }
```

### Types

#### Enum `TurbulenceClosure`

Which turbulence closure a solver run uses.

Enum dispatch, not a trait object — see the module documentation. Each
non-laminar variant owns the concrete model struct from
`outram-foam-turbulence-lib`, so its transport fields (k, ω, ε, ν̃, ν_t) are
reachable for inspection after a run, e.g.
`if let TurbulenceClosure::KOmegaSST(m) = &solver.turbulence { &m.k }`.

# Units

Every model in this enum is formulated **kinematically**: ν and ν_t are in
m²/s, k in m²/s², ω in 1/s, ε in m²/s³. A compressible solver holding dynamic
viscosity μ [Pa·s] must convert with [`TurbulenceClosure::mu_eff`], which
applies μ_t = ρ ν_t.

# Default

[`TurbulenceClosure::Laminar`] — a run that does not opt in to a model keeps
exactly the molecular viscous term the solver assembled before this module
existed, so no pre-existing result changes.

```rust
pub enum TurbulenceClosure {
    Laminar,
    KOmegaSST(KOmegaSST),
    KEpsilon(KEpsilon),
    KOmega(KOmega),
    SpalartAllmaras(SpalartAllmaras),
    Smagorinsky(Smagorinsky),
}
```

##### Variants

###### `Laminar`

No turbulence closure: ν_t ≡ 0 and ν_eff = ν.

The momentum stress term reduces to the plain implicit molecular
Laplacian `−∇·(ν ∇U)`. This variant deliberately **omits** the explicit
transpose correction `−∇·(ν dev2(∇Uᵀ))` that
`outram_foam_turbulence_lib::laminar::LaminarModel` adds: that term
vanishes identically for a divergence-free constant-ν flow, and omitting
it keeps this variant bit-for-bit identical to the viscous term the
solvers used before turbulence was wired in.

###### `KOmegaSST`

Menter (1994) k-ω SST RAS model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `KOmegaSST` |  |

###### `KEpsilon`

Jones & Launder (1972) standard k-ε RAS model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `KEpsilon` |  |

###### `KOmega`

Wilcox k-ω RAS model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `KOmega` |  |

###### `SpalartAllmaras`

Spalart-Allmaras (1992) one-equation RAS model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SpalartAllmaras` |  |

###### `Smagorinsky`

Smagorinsky (1963) LES sub-grid-scale model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Smagorinsky` |  |

##### Implementations

###### Methods

- ```rust
  pub fn k_omega_sst(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Menter k-ω SST over `mesh`, with the Menter (1994) coefficients.

- ```rust
  pub fn k_epsilon(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Standard k-ε over `mesh` (Jones & Launder 1972 coefficients).

- ```rust
  pub fn k_omega(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Wilcox k-ω over `mesh`.

- ```rust
  pub fn spalart_allmaras(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Spalart-Allmaras one-equation model over `mesh`.

- ```rust
  pub fn smagorinsky(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Smagorinsky LES over `mesh` (`Ck = 0.094`, `Ce = 1.048`, cubeRootVol Δ).

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-readable model name, matching the OpenFOAM `simulationType` /

- ```rust
  pub fn is_laminar(self: &Self) -> bool { /* ... */ }
  ```
  `true` when no turbulence transport is solved (ν_t ≡ 0).

- ```rust
  pub fn sync_inputs(self: &mut Self, u: &VolVectorField, phi: &SurfaceScalarField, nu: &VolScalarField, dt: f64) { /* ... */ }
  ```
  Push the solver's live state into the closure.

- ```rust
  pub fn correct(self: &mut Self) { /* ... */ }
  ```
  Advance the turbulence transport equations by one time step.

- ```rust
  pub fn div_dev_reff(self: &Self, u: &VolVectorField, nu: &VolScalarField) -> FvVectorMatrix { /* ... */ }
  ```
  Assemble the momentum stress term for the velocity field `u`.

- ```rust
  pub fn nu_t(self: &Self) -> Option<&VolScalarField> { /* ... */ }
  ```
  Turbulent kinematic viscosity ν_t [m²/s], or `None` for a laminar run.

- ```rust
  pub fn nu_eff(self: &Self, nu: &VolScalarField) -> VolScalarField { /* ... */ }
  ```
  Effective kinematic viscosity ν_eff = ν + ν_t [m²/s], per cell.

- ```rust
  pub fn mu_eff(self: &Self, mu: &VolScalarField, rho: &VolScalarField) -> VolScalarField { /* ... */ }
  ```
  Effective **dynamic** viscosity μ_eff = μ + ρ ν_t [Pa·s], per cell.

- ```rust
  pub fn alpha_eff_compressible(self: &Self, alpha: &VolScalarField, rho: &VolScalarField) -> VolScalarField { /* ... */ }
  ```
  Effective thermal diffusivity α_eff = α + α_t [kg/(m·s)] for a

- ```rust
  pub fn turbulent_prandtl(self: &Self) -> f64 { /* ... */ }
  ```
  Turbulent Prandtl number Pr_t (dimensionless) of the active model;

- ```rust
  pub fn set_k_omega_uniform(self: &mut Self, k: f64, scale: f64) -> bool { /* ... */ }
  ```
  Set uniform turbulence transport fields on a two-equation model.

- ```rust
  pub fn volumetric_flux(mass_flux: &SurfaceScalarField, rho: &VolScalarField) -> SurfaceScalarField { /* ... */ }
  ```
  Convert a compressible solver's **mass** flux φ_m = ρ U·S_f [kg/s] into

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
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> TurbulenceClosure { /* ... */ }
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
