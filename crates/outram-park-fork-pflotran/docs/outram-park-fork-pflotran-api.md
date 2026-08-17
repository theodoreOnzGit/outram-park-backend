# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `outram_park_fork_pflotran`

# outram-park-fork-pflotran

An independent, pure-Rust fork / translation of
[PFLOTRAN](https://www.pflotran.org) — the US-DOE national-lab subsurface
**flow and reactive-transport** simulator (Fortran + PETSc, massively
parallel) — rebuilt to OUTRAM PARK's design rules: enum dispatch (no trait
objects), `uom`-typed API boundaries, a pure-Rust solver (no PETSc FFI, no
MPI in v1), and an Android-buildable library.

> **⚠️ VERIFICATION-ONLY — no validation, no human V&V yet.** Multiple modes
> are implemented and unit/verification-tested, but NONE has been validated
> against published PFLOTRAN reference cases (all validation is bead-tracked
> and deferred: op-v6s.9.x/.10.1/.11.1/.12.1/.13.1). Do not treat any output
> as validated. Implemented so far, each **verified** against closed-form /
> manufactured references only:
> - **RICHARDS** variably-saturated flow ([`flow::RichardsSimulation`]) — MMS 2nd-order.
> - **Solute transport** ([`transport`]) + **TH heat transport** ([`energy`]) — closed-form advection–diffusion/conduction.
> - **Aqueous geochemistry** ([`geochemistry`]) + **kinetics** ([`kinetics`]) + **reactive transport** ([`reactive_transport`]).
> - **Two-phase (air–water) multiphase flow** ([`multiphase`]) on the block
>   solver ([`solver::block`]).
>
> **Independent fork, not the official PFLOTRAN.** "PFLOTRAN" names only the
> upstream work this crate derives from; nothing here is endorsed by or
> affiliated with the PFLOTRAN development team or the national laboratories
> (LANL, PNNL, ORNL, LBNL, SNL). See `NOTICE` and the workspace
> `TRADEMARKS.md`.
>
> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. No human V&V has been performed. Not for
> nuclear facility operation, reactor control, safety-critical analysis, or
> licensing decisions — this is for education, research, and V&V only.

## v1 scope — the vertical slice (bead op-v6s.2)

The first end-to-end target is deliberately narrow, so a real physics result
can be validated before breadth is added:

- **Flow mode:** RICHARDS — variably-saturated single-phase groundwater flow.
- **Grid:** structured Cartesian finite volume, two-point flux.
- **Solver:** serial pure-Rust Newton-Krylov (no PETSc, no MPI).
- **I/O:** a minimal card-based ASCII input-deck subset; CSV/VTK output.

Explicitly **out of v1**: unstructured grids, MPI / distributed solves,
HDF5, multiphase (GENERAL) flow, energy transport (TH), solute transport,
and reactive geochemistry (GIRT). Those are later beads (op-v6s.10..op-v6s.14).

## Module map — what belongs where

| Module | PFLOTRAN analogue | Status |
|---|---|---|
| [`units`] | dimensional quantities used throughout | **real** — named `uom` type aliases (a human hovers `Pressure`, not a raw `Quantity`) |
| [`error`] | error handling | **real** — the crate [`error::PflotranError`] enum |
| [`flow`] | `pm_*` process-model / flow-mode polymorphism | **working (verification-only)** — [`flow::FlowMode`] + [`flow::RichardsSimulation`]: RICHARDS residual/Jacobian + adaptive timestep (bead op-v6s.8) |
| [`grid`] | `discretization` / `grid` structured FV | **real** — structured Cartesian FV, two-point flux (bead op-v6s.5) |
| [`solver`] | PETSc SNES/KSP replacement | **real** — scalar + block ([`solver::block`]) Newton–Krylov over foam-basic-lib `krylov` (beads op-v6s.4, op-v6s.4.1) |
| [`properties`] | EOS + characteristic curves | **real** — EOS, van Genuchten/Brooks–Corey/Haverkamp curves, thermal properties (beads op-v6s.7, op-v6s.10) |
| [`io`] | input-deck cards + output | **real (AI-designed subset)** — card-deck + CSV/VTK (bead op-v6s.6) |
| [`transport`] | conservative solute transport | **working (verification-only)** — advection–diffusion, coupled to a RICHARDS flow field (bead op-v6s.11) |
| [`energy`] | TH heat transport | **working (verification-only)** — advection–conduction of temperature, one-way coupled to flow (bead op-v6s.10) |
| [`geochemistry`] | aqueous speciation | **working (verification-only)** — equilibrium speciation (GIRT core; bead op-v6s.12) |
| [`kinetics`] | mineral kinetics | **working (verification-only)** — TST precipitation/dissolution on a foam ODE solver (bead op-v6s.12) |
| [`reactive_transport`] | GIRT reactive transport | **working (verification-only)** — SNIA transport↔geochemistry coupling (bead op-v6s.12) |
| [`multiphase`] | GENERAL multiphase flow | **working (verification-only)** — two-phase air–water on the block solver (bead op-v6s.13) |
| [`general_mode`] | GENERAL air–water–energy | **working (verification-only)** — non-isothermal nb=3 (p_l, s_l, T) on the block solver; T couples back through ρ_l(T)/μ_l(T) (bead op-v6s.15.5) |
| [`thermal_convection`] | two-way buoyancy TH | **working (verification-only)** — density-driven porous convection; conductive limit, Rayleigh formula, and Horton–Rogers–Lapwood onset across 4π² all verified via energy-row equilibration + adaptive sub-stepping (beads op-v6s.15.6, op-3tt) |
| [`activity`] | aqueous activity models | **real** — Debye–Hückel / Davies coefficients (bead op-v6s.15.1) |
| [`sorption`] | sorption + ion exchange | **real** — Kd/Langmuir/Freundlich isotherms + Gaines–Thomas exchange (bead op-v6s.15.2) |
| [`surface_complexation`] | pH-dependent sorption | **real** — amphoteric protonation + metal binding with NEM/CCM/diffuse-layer electrostatics (bead op-gg7) |
| [`decay`] | radioactive decay | **real** — Bateman decay chains + ingrowth (bead op-v6s.15.3) |
| [`eos_real`] | real fluid EOS | **real** — IAPWS-IF97 liquid water via `tampines-steam-tables` (bead op-v6s.15.7) |
| [`eos_co2_brine`] | CO2 + brine EOS | **real (approximate)** — Redlich–Kwong CO2 + Batzle–Wang NaCl brine density/viscosity (bead op-1y6) |
| [`microbial`] | microbial reactions | **real** — Monod/dual-Monod biodegradation on a foam ODE solver (bead op-v6s.15.4) |
| [`wells`] | wells + advanced BCs | **real** — Peaceman well index + hydrostatic/seepage/time-varying BCs (bead op-v6s.15.12) |
| [`deck`] | real PFLOTRAN input deck | **real (subset)** — genuine PFLOTRAN keyword-block syntax, Fortran D-exponent floats (bead op-v6s.15.10) |
| [`decomposition`] | MPI domain decomposition | **working (verification-only)** — 1-D partition + halo exchange + distributed CG (all-reduce dot + halo matvec) over `outram-park-mpi`; matches serial (beads op-v6s.15.9, op-57m) |
| [`hdf5_io`] | HDF5 snapshot output | **real (AI-designed layout)** — structured-grid solution snapshots via pure-Rust `hdf5-pure`; write/read round-trip (bead op-v6s.15.11) |
| [`pitzer`] | high-ionic-strength activity | **real** — Pitzer ion-interaction virial model for brines (25 °C, binary salts) (bead op-s1h) |
| [`unstructured`] | unstructured FV grid | **real** — polyhedral cell/face connectivity + two-point-flux (TPFA) transmissibility (bead op-v6s.15.8) |

Modules op-v6s.15.1/.2/.3/.4/.7/.10/.12 above are standalone building blocks
(upstream-parity gaps); the sorption/decay pieces are wired into
[`transport`], while the others are self-contained and not yet wired into the
flow/transport/geochemistry hot loops.

## Design rules (workspace mandate)

- **Enum dispatch, no trait objects.** Flow modes, EOS forms, and solver
  kinds are enums matched exhaustively — see [`flow::FlowMode`]. A trait may
  still act as a compiler-checked contract on each concrete mode, but never
  as `Box<dyn _>` runtime dispatch.
- **`uom` at API boundaries.** Every physical quantity crossing a public
  boundary is a [`units`] alias, so units are checked at compile time.
- **Pure Rust, Android-safe.** No PETSc, no MPI, no system BLAS, no C/Fortran
  toolchain in the library build.

## Modules

## Module `activity`

Aqueous activity-coefficient models — bead op-v6s.15.1.

Corrects molar concentration to thermodynamic **activity** for dissolved
ions, replacing the ideal (`gamma = 1`) assumption used by the v1
geochemistry speciation core (see the [`crate::geochemistry`] module). The
activity of species `i` is

```text
a_i = gamma_i * c_i
```

where `c_i` is the free concentration in **mol/L** and `gamma_i` its
(dimensionless) activity coefficient. In the dilute limit `I -> 0` every
model returns `gamma -> 1`, recovering the ideal case.

## Ionic strength

All charged-species corrections are driven by the ionic strength

```text
I = 0.5 * sum_i c_i * z_i^2      [mol/L]
```

with `c_i` in mol/L and `z_i` the (signed, integer-or-real) charge number.

## Models (all at ~25 degrees C)

With Debye–Hückel constants at 25 degrees C in water,
`A ≈ 0.5085` and `B ≈ 0.3281` (units `1 / (Angstrom * sqrt(mol/L))`):

- **[`ActivityModel::Ideal`]** — `gamma = 1` for every species.
- **[`ActivityModel::DebyeHuckel`]** — extended Debye–Hückel with an optional
  b-dot linear term:

  ```text
  log10 gamma_i = -A z_i^2 sqrt(I) / (1 + B a_i sqrt(I)) + bdot * I
  ```

  where `a_i` is the ion-size (a0) parameter in Angstrom and `bdot` the
  optional linear term (`bdot = 0` gives plain extended Debye–Hückel;
  `bdot ≈ 0.041` is the classic Helgeson NaCl B-dot value). Usable to higher
  ionic strength than Davies when a fitted `bdot` is supplied.
- **[`ActivityModel::Davies`]** — parameterless, valid to `I ~ 0.5 mol/L`:

  ```text
  log10 gamma_i = -A z_i^2 ( sqrt(I) / (1 + sqrt(I)) - 0.3 I )
  ```

## Human-review flags (simplifications)

- **25 degrees C only.** `A` and `B` are fixed at their 25 degrees C water
  values; there is no temperature dependence. Using these at other
  temperatures is a modelling error until temperature-dependent constants
  are added.
- **Common ion-size parameter.** [`ActivityModel::DebyeHuckel`] carries a
  single `a_i` applied to every ion, rather than per-species a0 values.
- **Neutral-species simplification.** For the charged models a species with
  `z = 0` is assigned `gamma = 1` (the setchenow / "salting-out" correction
  for neutral aqueous species is not modelled).

## Provenance

Standard aqueous-geochemistry activity models as implemented in PFLOTRAN and
PHREEQC. References: Debye & Hückel (1923); C. W. Davies, *Ion Association*
(Butterworths, 1962); Helgeson (1969) for the B-dot extension; Parkhurst &
Appelo, *Description of Input and Examples for PHREEQC Version 3*, USGS
Techniques and Methods 6-A43 (2013). These are open, published models.

Enum dispatch, no trait objects (workspace rule).

```rust
pub mod activity { /* ... */ }
```

### Types

#### Enum `ActivityModel`

Aqueous activity-coefficient model. Relates free concentration to activity
by `a_i = gamma_i * c_i`, with `c_i` in mol/L.

The model set is closed and dispatched by `match` (no trait objects). All
variants are evaluated at ~25 degrees C; see the module header for the
physics, constants, and human-review simplifications.

```rust
pub enum ActivityModel {
    Ideal,
    DebyeHuckel {
        ion_size_angstrom: f64,
        bdot: f64,
    },
    Davies,
}
```

##### Variants

###### `Ideal`

Ideal solution: `gamma = 1` for every species, at any ionic strength.

###### `DebyeHuckel`

Extended Debye–Hückel with a single common ion-size parameter
(`ion_size_angstrom`, the a0 in Angstrom) and an optional b-dot linear
term (`bdot`, units `1 / (mol/L)`). Set `bdot = 0` for plain extended
Debye–Hückel.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ion_size_angstrom` | `f64` | Common ion-size (a0) parameter in Angstrom, applied to every ion.<br>Typical values are ~3–9 Angstrom (e.g. ~4 for Na+, ~9 for H+). |
| `bdot` | `f64` | Optional b-dot linear term in `1 / (mol/L)`; `0.0` gives plain<br>extended Debye–Hückel. |

###### `Davies`

Davies equation — parameterless, reasonable to ionic strength
`I ~ 0.5 mol/L`.

##### Implementations

###### Methods

- ```rust
  pub fn ionic_strength(charges: &[f64], concentrations: &[f64]) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Ionic strength `I = 0.5 * sum_i c_i * z_i^2` in mol/L.

- ```rust
  pub fn log10_gamma(self: &Self, charge: f64, ionic_strength: f64) -> f64 { /* ... */ }
  ```
  `log10(gamma_i)` for an ion of the given (signed, integer-or-real)

- ```rust
  pub fn gamma(self: &Self, charge: f64, ionic_strength: f64) -> f64 { /* ... */ }
  ```
  Activity coefficient `gamma = 10^{log10_gamma}`.

- ```rust
  pub fn a_constant() -> f64 { /* ... */ }
  ```
  The Debye–Hückel `A` constant used by the charged models

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ActivityModel { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ActivityModel) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `deck`

Parser for **genuine PFLOTRAN input-deck syntax** (a supported subset).

Unlike the [`crate::io`] module — whose grammar is an AI-invented "lite"
format that is *not* compatible with real PFLOTRAN — this module parses the
actual keyword/block syntax used by the upstream PFLOTRAN simulator
(documentation.pflotran.org). It is a step toward real-deck compatibility.

This parser is intentionally self-contained: it depends only on
[`crate::error::PflotranError`] and the standard library, and it produces its
own plain [`PflotranDeck`] struct (it does **not** reuse the `io` module's
[`crate::io::InputDeck`] spec types).

# Supported subset — flag clearly what is parsed

Real PFLOTRAN decks are large; this v1 parser targets a *representative,
honest subset*. It is **not** a full PFLOTRAN reader. Supported blocks and
cards:

| Block | Cards parsed | Notes |
|---|---|---|
| `SIMULATION` | `SIMULATION_TYPE`, `PROCESS_MODELS` → `SUBSURFACE_FLOW` → `MODE` | only `MODE` is captured; other cards are skipped leniently |
| `GRID` | `TYPE`, `NXYZ`, `BOUNDS` | structured Cartesian only |
| `MATERIAL_PROPERTY <name>` | `ID`, `POROSITY`, `PERMEABILITY` → `PERM_ISO` | isotropic permeability only |
| `CHARACTERISTIC_CURVES <name>` | `SATURATION_FUNCTION <type>` → `ALPHA`, `M`, `LIQUID_RESIDUAL_SATURATION` | |
| `TIME` | `FINAL_TIME`, `INITIAL_TIMESTEP_SIZE`, `MAXIMUM_TIMESTEP_SIZE` | value + unit suffix, normalised to seconds |
| `SUBSURFACE` / `END_SUBSURFACE` | (markers) | consumed and ignored |
| `SKIP` / `NOSKIP` | (markers) | consumed and ignored (v1: no region skipping) |

# NOT yet parsed (human-review flags)

Everything else in the real format, including: unstructured / `DXYZ` grids,
anisotropic / tensor permeability, `REGION` / `OBSERVATION` /
`BOUNDARY_CONDITION` / `INITIAL_CONDITION` / `STRATA` cards, `FLOW_CONDITION`
and `TRANSPORT_CONDITION`, `CHEMISTRY`, `DATASET` / `HDF5`, `EXTERNAL_FILE`
inclusion, `REFERENCE_*` cards, output/`SNAPSHOT_FILE` controls, and true
`SKIP`/`NOSKIP` region skipping. Inside a supported block an *unknown card is
rejected* (with line context) rather than silently ignored, so the parser
never pretends to understand more than it does. This is untrusted
AI-generated draft material until a human reviews it against real PFLOTRAN
decks, per the workspace `RESPONSIBLE_USE.md` rule.

# Syntax features handled

- **Fortran D-exponent floats** — `1.d-12` == `1.0e-12`, `0.d0` == `0.0`,
  `-3.5D2` == `-350.0` (see [`parse_fortran_float`]).
- **Time unit suffixes** — `s`/`m`/`h`/`d`/`y` on `TIME` cards, normalised to
  seconds (see [`time_to_seconds`]); a year is 365.25 days.
- **Comments** — `#` and `!` to end of line.
- **Named blocks** — `MATERIAL_PROPERTY soil1`, `CHARACTERISTIC_CURVES cc1`.
- **Nested blocks** closed by `/`; top-level blocks closed by `END` (either
  terminator is accepted where a block ends).
- **Case-insensitive keywords**; blank lines; stray `:` tokens ignored.

```rust
pub mod deck { /* ... */ }
```

### Types

#### Struct `PflotranDeck`

A parsed (subset of a) real PFLOTRAN input deck.

Produced by [`parse_pflotran_deck`]. Only the [supported subset](self) of
cards is represented; unsupported cards cause a parse error rather than being
dropped, so a successfully parsed deck reflects exactly what the file said
about the fields below.

```rust
pub struct PflotranDeck {
    pub simulation_mode: FlowMode,
    pub grid: GridBlock,
    pub materials: Vec<MaterialProperty>,
    pub characteristic_curves: Vec<CharacteristicCurveBlock>,
    pub time: TimeBlock,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `simulation_mode` | `FlowMode` | Flow mode from `SIMULATION` > `PROCESS_MODELS` > `SUBSURFACE_FLOW` ><br>`MODE` (e.g. `RICHARDS`). |
| `grid` | `GridBlock` | Structured grid dimensions and physical bounds (`GRID` block). |
| `materials` | `Vec<MaterialProperty>` | Zero or more `MATERIAL_PROPERTY` blocks, in file order. |
| `characteristic_curves` | `Vec<CharacteristicCurveBlock>` | Zero or more `CHARACTERISTIC_CURVES` blocks, in file order. |
| `time` | `TimeBlock` | Time-stepping controls (`TIME` block), all normalised to seconds. |

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
    fn clone(self: &Self) -> PflotranDeck { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PflotranDeck) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `FlowMode`

The subsurface-flow process model selected by the `MODE` card.

`RICHARDS`/`TH`/`GENERAL` are the common PFLOTRAN flow modes; any other
keyword is preserved verbatim in [`FlowMode::Unknown`] so the caller can see
what the deck asked for without the parser silently discarding it.

```rust
pub enum FlowMode {
    Richards,
    Th,
    General,
    Unknown(String),
}
```

##### Variants

###### `Richards`

`RICHARDS` — variably-saturated single-phase flow.

###### `Th`

`TH` — thermo-hydraulic (flow + heat).

###### `General`

`GENERAL` — multiphase / multicomponent flow.

###### `Unknown`

Any other `MODE` keyword, stored verbatim (original case preserved).

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FlowMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FlowMode) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GridBlock`

The `GRID` block: a structured Cartesian mesh and its physical extent.

`nx`/`ny`/`nz` come from the `NXYZ` card (cell counts along each axis).
`bounds_min`/`bounds_max` come from the `BOUNDS` sub-block as
`[x, y, z]` corner coordinates in **metres**.

```rust
pub struct GridBlock {
    pub structured: bool,
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub bounds_min: [f64; 3],
    pub bounds_max: [f64; 3],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `structured` | `bool` | `true` when `TYPE STRUCTURED` was given (or defaulted). Only structured<br>grids are supported in v1. |
| `nx` | `usize` | Cell count along x, from `NXYZ`. |
| `ny` | `usize` | Cell count along y, from `NXYZ`. |
| `nz` | `usize` | Cell count along z, from `NXYZ`. |
| `bounds_min` | `[f64; 3]` | Lower `[x, y, z]` corner of the domain, in metres (first `BOUNDS` line). |
| `bounds_max` | `[f64; 3]` | Upper `[x, y, z]` corner of the domain, in metres (second `BOUNDS` line). |

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
    fn clone(self: &Self) -> GridBlock { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GridBlock) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `MaterialProperty`

A `MATERIAL_PROPERTY <name>` block (isotropic subset).

```rust
pub struct MaterialProperty {
    pub name: String,
    pub id: usize,
    pub porosity: f64,
    pub permeability_iso: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The material name that followed the `MATERIAL_PROPERTY` keyword. |
| `id` | `usize` | Integer material `ID`. |
| `porosity` | `f64` | `POROSITY`, dimensionless. |
| `permeability_iso` | `f64` | Isotropic intrinsic permeability from `PERMEABILITY` > `PERM_ISO`, in<br>m^2. |

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
    fn clone(self: &Self) -> MaterialProperty { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MaterialProperty) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `CharacteristicCurveBlock`

A `CHARACTERISTIC_CURVES <name>` block (saturation-function subset).

Captures the `SATURATION_FUNCTION` type keyword (e.g. `VAN_GENUCHTEN`) and
its three parameters. Relative-permeability sub-blocks are not parsed in v1.

```rust
pub struct CharacteristicCurveBlock {
    pub name: String,
    pub saturation_function: String,
    pub alpha: f64,
    pub m: f64,
    pub liquid_residual_saturation: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The curve-set name that followed the `CHARACTERISTIC_CURVES` keyword. |
| `saturation_function` | `String` | The `SATURATION_FUNCTION` model keyword, verbatim (e.g. `VAN_GENUCHTEN`). |
| `alpha` | `f64` | `ALPHA`, the van-Genuchten air-entry parameter, in 1/Pa. |
| `m` | `f64` | `M`, the van-Genuchten exponent, dimensionless. |
| `liquid_residual_saturation` | `f64` | `LIQUID_RESIDUAL_SATURATION`, dimensionless. |

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
    fn clone(self: &Self) -> CharacteristicCurveBlock { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CharacteristicCurveBlock) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TimeBlock`

The `TIME` block, with every value normalised to **seconds**.

Each source card carries a value plus a unit suffix (`s`/`m`/`h`/`d`/`y`);
[`time_to_seconds`] converts them, so the fields here are always in SI
seconds regardless of the deck's chosen units.

```rust
pub struct TimeBlock {
    pub final_time_s: f64,
    pub initial_dt_s: f64,
    pub max_dt_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `final_time_s` | `f64` | `FINAL_TIME`, in seconds. |
| `initial_dt_s` | `f64` | `INITIAL_TIMESTEP_SIZE`, in seconds. |
| `max_dt_s` | `f64` | `MAXIMUM_TIMESTEP_SIZE`, in seconds. |

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
    fn clone(self: &Self) -> TimeBlock { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TimeBlock) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `parse_pflotran_deck`

Parse a real-PFLOTRAN-syntax deck (the [supported subset](self)).

Returns a fully-populated [`PflotranDeck`] on success. On failure returns
[`PflotranError::InvalidInput`] carrying line context and, where useful, the
offending keyword.

Required blocks: `SIMULATION` (with a `MODE` card), `GRID`, and `TIME`; their
absence is an error. `MATERIAL_PROPERTY` and `CHARACTERISTIC_CURVES` are
optional and may repeat.

```rust
pub fn parse_pflotran_deck(input: &str) -> Result<PflotranDeck, crate::error::PflotranError> { /* ... */ }
```

#### Function `parse_fortran_float`

Parse a single Fortran-style float token.

PFLOTRAN decks are written by/for Fortran and use the `D`/`d` exponent marker
(`1.d-12`, `0.d0`, `-3.5D2`). This accepts that plus ordinary `e`/`E`
exponents (`1.0e3`) and plain decimals (`2.5`). The `D`/`d` marker is
translated to `e` and the result parsed as an [`f64`].

Returns [`PflotranError::InvalidInput`] for tokens that are not valid
numbers.

# Examples

```
use outram_park_fork_pflotran::deck::parse_fortran_float;
assert_eq!(parse_fortran_float("1.d-12").unwrap(), 1.0e-12);
assert_eq!(parse_fortran_float("0.d0").unwrap(), 0.0);
assert_eq!(parse_fortran_float("-3.5D2").unwrap(), -350.0);
assert_eq!(parse_fortran_float("1.0e3").unwrap(), 1000.0);
assert!(parse_fortran_float("nonsense").is_err());
```

```rust
pub fn parse_fortran_float(token: &str) -> Result<f64, crate::error::PflotranError> { /* ... */ }
```

#### Function `time_to_seconds`

Convert a PFLOTRAN time value plus a unit suffix to seconds.

Recognised units (case-insensitive): `s` (second), `m` (minute), `h` (hour),
`d` (day), `y` (year = 365.25 days). Common long forms (`sec`, `min`, `hr`,
`day`, `yr`, and plurals) are also accepted. Unknown units are rejected.

# Examples

```
use outram_park_fork_pflotran::deck::time_to_seconds;
assert_eq!(time_to_seconds(1.0, "d").unwrap(), 86_400.0);
assert_eq!(time_to_seconds(2.0, "h").unwrap(), 7_200.0);
assert_eq!(time_to_seconds(5.0, "s").unwrap(), 5.0);
assert!(time_to_seconds(1.0, "furlong").is_err());
```

```rust
pub fn time_to_seconds(value: f64, unit: &str) -> Result<f64, crate::error::PflotranError> { /* ... */ }
```

## Module `decomposition`

MPI-style domain decomposition and halo exchange (bead op-v6s.15.9).

PFLOTRAN scales out by partitioning the grid across MPI ranks and exchanging a
one-cell **halo** (ghost layer) between neighbouring subdomains each iteration,
so every rank can evaluate its stencil using up-to-date neighbour values. This
module provides that pattern for a **structured 1-D chain of cells**, driven by
the pure-Rust [`outram_park_mpi`] transport — the first slice of distributed
scale-out.

# What is here

- [`Decomposition1D`] — a balanced contiguous partition of `n_global` cells
  across the ranks of a communicator: each rank's cell range and its left/right
  neighbour ranks.
- [`exchange_halo`] — one nearest-neighbour halo swap: send each subdomain's
  edge cell to its neighbour and receive the neighbour's edge into a ghost slot.
- [`jacobi_smooth_distributed`] — a worked example: distributed Jacobi
  relaxation of the 1-D Laplace problem with Dirichlet ends, which reproduces
  the serial result exactly (see the module tests). This stands in for the real
  stencil kernels (diffusion, conduction, the transport operator) that the halo
  exchange will feed.

# Scope / human-review flags

**Verification-only, untrusted AI draft** (workspace `RESPONSIBLE_USE.md`).
This is the halo-exchange foundation, **not** a fully MPI-parallel Newton
solve: the implicit RICHARDS/transport Jacobian is still assembled and solved
serially per rank. Decomposing the global linear solve (distributed
matrix-vector products + a parallel Krylov method) is a follow-up. The
partition is 1-D (structured slabs); unstructured / multi-dimensional
partitioning is future work.

```rust
pub mod decomposition { /* ... */ }
```

### Modules

## Module `cartesian2d`

2-D Cartesian domain decomposition and distributed solve (bead op-gj5).

Extends the 1-D [`super`] decomposition to a **2-D process grid** built on the
MPI Cartesian topology ([`outram_park_mpi::CartesianComm`]): the global
`nx × ny` cell grid is tiled into rectangular blocks, one per rank, and each
rank finds its four face neighbours with [`cart_shift`](outram_park_mpi::CartesianComm::shift).
A 5-point-stencil halo exchange ([`exchange_halo_2d`]) swaps edge rows/columns,
and a distributed conjugate gradient ([`distributed_cg_2d`]) — halo-exchanged
matvec plus [`all_reduce`](outram_park_mpi::Communicator)-reduced dot products
(reusing [`super::krylov::distributed_dot`]) — solves the 2-D shifted-Poisson
system to the **same answer as a serial solve at any process-grid shape**
(checked in the module tests).

# Scope / human-review flags

Verification-only, untrusted AI draft. This demonstrates 2-D distributed
partitioning + solve on a model SPD operator; 3-D, unstructured meshes,
preconditioning, and using it as the linear stage of pflotran's real Newton
solve remain op-gj5 follow-ups. Blocks are balanced per axis; no
reorder-for-locality.

```rust
pub mod cartesian2d { /* ... */ }
```

### Types

#### Struct `Decomposition2D`

A 2-D rectangular block decomposition of an `nx_global × ny_global` cell grid
over a Cartesian process grid.

Local fields are stored row-major within the block: local cell `(ix, iy)` is at
index `iy * lx + ix`, for `ix in 0..lx`, `iy in 0..ly`.

```rust
pub struct Decomposition2D {
    pub nx_global: usize,
    pub ny_global: usize,
    pub cart: outram_park_mpi::CartesianComm,
    pub x0: usize,
    pub y0: usize,
    pub lx: usize,
    pub ly: usize,
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub down: Option<i32>,
    pub up: Option<i32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nx_global` | `usize` | Global cell counts. |
| `ny_global` | `usize` | Global cell counts. |
| `cart` | `outram_park_mpi::CartesianComm` | The 2-D Cartesian communicator (`dims = [px, py]`). |
| `x0` | `usize` | Global x offset of this block's first column. |
| `y0` | `usize` | Global y offset of this block's first row. |
| `lx` | `usize` | Local block width (x) and height (y). |
| `ly` | `usize` | Local block width (x) and height (y). |
| `left` | `Option<i32>` | Neighbour ranks (in the Cartesian comm): `-x`, `+x`, `-y`, `+y`; `None` at a<br>physical domain edge. |
| `right` | `Option<i32>` |  |
| `down` | `Option<i32>` |  |
| `up` | `Option<i32>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(nx_global: usize, ny_global: usize, cart: CartesianComm) -> Self { /* ... */ }
  ```
  Build the 2-D block decomposition for this rank from the global grid size

- ```rust
  pub fn local_len(self: &Self) -> usize { /* ... */ }
  ```
  Number of local cells (`lx * ly`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Halo2D`

Ghost rows/columns received from the four face neighbours (empty where the
block touches a physical domain edge — a zero Dirichlet ghost).

```rust
pub struct Halo2D {
    pub left: Vec<f64>,
    pub right: Vec<f64>,
    pub down: Vec<f64>,
    pub up: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `left` | `Vec<f64>` | Left neighbour's rightmost column (length `ly`), else empty. |
| `right` | `Vec<f64>` | Right neighbour's leftmost column (length `ly`), else empty. |
| `down` | `Vec<f64>` | Down neighbour's top row (length `lx`), else empty. |
| `up` | `Vec<f64>` | Up neighbour's bottom row (length `lx`), else empty. |

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
    fn clone(self: &Self) -> Halo2D { /* ... */ }
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
    fn default() -> Halo2D { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `exchange_halo_2d`

Exchange the four edge rows/columns of `u` with the block's face neighbours for
a 5-point stencil.

# Errors
Propagates any transport error.

```rust
pub fn exchange_halo_2d(decomp: &Decomposition2D, u: &[f64]) -> outram_park_mpi::MpiResult<Halo2D> { /* ... */ }
```

#### Function `poisson_matvec_2d`

Apply the 2-D shifted-Poisson operator `A = (4+shift) I - L` (5-point stencil)
to the distributed field `u`, returning `A u` on the local block. Ghosts at a
physical domain edge are `0` (homogeneous Dirichlet); `shift > 0` makes `A` SPD.

# Errors
Propagates any halo-exchange transport error.

```rust
pub fn poisson_matvec_2d(decomp: &Decomposition2D, u: &[f64], shift: f64) -> outram_park_mpi::MpiResult<Vec<f64>> { /* ... */ }
```

#### Function `distributed_cg_2d`

Solve the 2-D shifted-Poisson system `A u = b` by distributed conjugate
gradient over the block decomposition, starting from `u = 0`. Returns this
rank's solution block and the iteration count.

# Errors
Propagates any transport error from the matvec or reduced dot products.

```rust
pub fn distributed_cg_2d(decomp: &Decomposition2D, b: &[f64], shift: f64, tol: f64, max_iter: usize) -> outram_park_mpi::MpiResult<(Vec<f64>, usize)> { /* ... */ }
```

#### Function `serial_poisson_2d_cg`

Serial reference: the 2-D shifted-Poisson CG on one rank over the whole
`nx × ny` grid (row-major `iy*nx + ix`), the oracle for [`distributed_cg_2d`].

```rust
pub fn serial_poisson_2d_cg(nx: usize, ny: usize, b: &[f64], shift: f64, tol: f64, max_iter: usize) -> (Vec<f64>, usize) { /* ... */ }
```

## Module `krylov`

Distributed conjugate-gradient solve over an MPI decomposition (bead op-57m).

This is the parallel-Krylov core of MPI scale-out: it solves a symmetric
positive-definite linear system `A u = b` across ranks using only two
distributed primitives —

- a **halo-exchanged matrix-vector product** ([`poisson_matvec`]): each rank
  applies the stencil to its slab using ghost values from its neighbours (via
  [`super::exchange_halo`]); and
- a **globally-reduced inner product** ([`distributed_dot`]): each rank sums
  its local contribution, then an [`all_reduce`](outram_park_mpi::Communicator::all_reduce)
  forms the global dot product.

With those two operations the conjugate-gradient iteration ([`distributed_cg`])
is otherwise identical to the serial one, and produces the **same solution
regardless of rank count** — the module tests check this against
[`serial_cg`] across several partitions.

The model operator is the 1-D shifted Poisson / Helmholtz stencil
`A u_i = (2 + shift) u_i - u_{i-1} - u_{i+1}` with homogeneous Dirichlet ghosts
(`u_{-1} = u_N = 0`). For `shift > 0` it is symmetric positive-definite and
well-conditioned, so CG converges — it stands in for the SPD systems a real
implicit discretisation produces.

# Scope / human-review flags

**Verification-only, untrusted AI draft.** This demonstrates a *distributed
linear solve* (parallel CG); wiring it as the linear stage of pflotran's actual
Newton solve on the real Jacobian, preconditioning, and 2-D/3-D (Cartesian) or
unstructured partitioning remain follow-ups under op-57m.

```rust
pub mod krylov { /* ... */ }
```

### Functions

#### Function `poisson_matvec`

Apply the 1-D shifted-Poisson operator `A = (2+shift) I - L` to the distributed
vector `u` (this rank's slab), returning `A u` for the local cells.

Ghost values come from a halo exchange; at a physical domain end the ghost is
`0` (homogeneous Dirichlet). `shift >= 0`; `shift > 0` makes `A` SPD.

# Errors
Propagates any halo-exchange transport error.

```rust
pub fn poisson_matvec(comm: &outram_park_mpi::Communicator, decomp: &super::Decomposition1D, u: &[f64], shift: f64) -> outram_park_mpi::MpiResult<Vec<f64>> { /* ... */ }
```

#### Function `distributed_dot`

Global inner product of two distributed vectors: local dot, then an
`all_reduce(Sum)` so every rank gets the same global value.

# Errors
Propagates any collective transport error.

```rust
pub fn distributed_dot(comm: &outram_park_mpi::Communicator, a: &[f64], b: &[f64]) -> outram_park_mpi::MpiResult<f64> { /* ... */ }
```

#### Function `distributed_cg`

Solve `A u = b` for the shifted-Poisson operator by **distributed conjugate
gradient**, starting from `u = 0`. Returns this rank's solution slab and the
iteration count.

Convergence is on the global residual 2-norm (`sqrt(r·r) < tol`). Every rank
must call this collectively with matching `shift`/`tol`/`max_iter`.

# Errors
Propagates any transport error from the matvec or the reduced dot products.

```rust
pub fn distributed_cg(comm: &outram_park_mpi::Communicator, decomp: &super::Decomposition1D, b: &[f64], shift: f64, tol: f64, max_iter: usize) -> outram_park_mpi::MpiResult<(Vec<f64>, usize)> { /* ... */ }
```

#### Function `distributed_cg_with`

Distributed conjugate gradient for an **arbitrary** SPD operator, given as a
matvec closure `matvec(v) -> A v` that applies the distributed operator to this
rank's slab (performing its own halo exchange). Starts from `x = 0`; returns
this rank's solution slab and the iteration count.

This is the reusable core — [`distributed_cg`] is the shifted-Poisson
specialisation, and [`super::operator::DiffusionOperator1D`] drives it with a
real variable-coefficient operator. The generic parameter is a monomorphised
closure, not a trait object, per the workspace design rules.

Convergence is on the global residual 2-norm (`sqrt(r·r) < tol`); the dot
products are [`distributed_dot`]s so every rank agrees. Every rank must call
this collectively with a matching `matvec`, `tol`, and `max_iter`.

# Errors
Propagates any transport error from `matvec` or the reduced dot products.

```rust
pub fn distributed_cg_with<F>(comm: &outram_park_mpi::Communicator, b: &[f64], matvec: F, tol: f64, max_iter: usize) -> outram_park_mpi::MpiResult<(Vec<f64>, usize)>
where
    F: Fn(&[f64]) -> outram_park_mpi::MpiResult<Vec<f64>> { /* ... */ }
```

#### Function `distributed_pcg_with`

Distributed **preconditioned** conjugate gradient for an arbitrary SPD operator.

Like [`distributed_cg_with`] but applies a preconditioner `precond(r) -> M⁻¹ r`
each iteration (`M ≈ A`, SPD). For a Jacobi/diagonal preconditioner `precond`
is purely local (no communication) and markedly accelerates convergence on a
badly-scaled (e.g. heterogeneous-coefficient) system. Both `matvec` and
`precond` are monomorphised closures, not trait objects.

# Errors
Propagates any transport error from `matvec`, `precond`, or the reduced dots.

```rust
pub fn distributed_pcg_with<F, P>(comm: &outram_park_mpi::Communicator, b: &[f64], matvec: F, precond: P, tol: f64, max_iter: usize) -> outram_park_mpi::MpiResult<(Vec<f64>, usize)>
where
    F: Fn(&[f64]) -> outram_park_mpi::MpiResult<Vec<f64>>,
    P: Fn(&[f64]) -> outram_park_mpi::MpiResult<Vec<f64>> { /* ... */ }
```

#### Function `distributed_bicgstab_with`

Distributed **BiCGStab** for an arbitrary (possibly **non-symmetric**) operator
— the method the advection-dominated transport Jacobian needs, where CG does
not apply.

Like [`distributed_cg_with`] the operator is a monomorphised matvec closure and
all inner products are [`distributed_dot`]s; the stabilised bi-conjugate-
gradient recurrence is otherwise the textbook one. Starts from `x = 0`;
converges on the global residual 2-norm. A breakdown (`rho` or `omega` → 0)
stops the iteration and returns the best `x` so far.

# Errors
Propagates any transport error from `matvec` or the reduced dot products.

```rust
pub fn distributed_bicgstab_with<F>(comm: &outram_park_mpi::Communicator, b: &[f64], matvec: F, tol: f64, max_iter: usize) -> outram_park_mpi::MpiResult<(Vec<f64>, usize)>
where
    F: Fn(&[f64]) -> outram_park_mpi::MpiResult<Vec<f64>> { /* ... */ }
```

#### Function `serial_cg`

Serial reference: the same shifted-Poisson CG solved on one rank over the whole
`n_global`-cell system, the correctness oracle for [`distributed_cg`].

```rust
pub fn serial_cg(n_global: usize, b: &[f64], shift: f64, tol: f64, max_iter: usize) -> (Vec<f64>, usize) { /* ... */ }
```

## Module `ldu`

Distributed solve of a **real assembled `LduMatrix`** (bead op-gj5).

The previous op-gj5 slices ([`super::operator`]) drive the distributed CG from
a re-implemented stencil. This module closes the loop to pflotran's *actual*
linear-algebra type: it assembles the genuine
[`outram_foam_basic_lib::ldu_matrix::LduMatrix`] — the face-addressed sparse
matrix the RICHARDS/transport/energy solvers build and hand to the serial
Krylov backend — and then solves it **distributed** across MPI ranks.

[`assemble_diffusion_ldu`] performs a real face-based diffusion assembly
(per-connection geometric transmissibility × harmonic-mean conductivity, plus a
diagonal Helmholtz shift for SPD-ness) into an `LduMatrix`.
[`DistributedLduMatrix1D`] extracts this rank's rows from that matrix over a
1-D cell partition and provides a halo-exchanged distributed matrix-vector
product; [`DistributedLduMatrix1D::solve`] drives
[`super::krylov::distributed_cg_with`]. The module tests check the distributed
SpMV against [`LduMatrix::multiply`] (the real serial product) cell-for-cell,
and the distributed solution against a serial CG on the same matrix.

# Scope / human-review flags

Verification-only, untrusted AI draft. The distributed matrix is currently
*extracted* from a globally-assembled `LduMatrix` (each rank sees the whole
assembly) — a demo simplification; assembling only the local rows per rank, and
using this inside pflotran's real Newton loop on the non-symmetric transport
Jacobian (which needs a distributed BiCGStab, not CG), remain the op-gj5
follow-ups. 1-D structured partition only.

```rust
pub mod ldu { /* ... */ }
```

### Types

#### Struct `DistributedLduMatrix1D`

A row-distributed view of a real [`LduMatrix`] over a 1-D cell partition.

Holds this rank's owned rows as a diagonal plus the two tridiagonal
off-diagonals (`west` = coupling to cell `c-1`, `east` = coupling to `c+1`),
extracted from the assembled matrix. Cross-rank couplings at the slab edges are
resolved by a halo exchange during the matvec.

```rust
pub struct DistributedLduMatrix1D {
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
  pub fn from_rows(decomp: &Decomposition1D, diag: Vec<f64>, west: Vec<f64>, east: Vec<f64>) -> Result<Self, crate::error::PflotranError> { /* ... */ }
  ```
  Build directly from this rank's tridiagonal rows: `diag[i]` = `A_ii`,

- ```rust
  pub fn from_global(decomp: &Decomposition1D, ldu: &LduMatrix) -> Self { /* ... */ }
  ```
  Extract this rank's rows (per `decomp`) from a globally-assembled 1-D

- ```rust
  pub fn assemble_diffusion_local(comm: &Communicator, decomp: &Decomposition1D, k_local: &[f64], geom: f64, shift: f64) -> MpiResult<Self> { /* ... */ }
  ```
  Assemble this rank's rows **locally** — without ever forming the global

- ```rust
  pub fn matvec(self: &Self, comm: &Communicator, x: &[f64]) -> MpiResult<Vec<f64>> { /* ... */ }
  ```
  Distributed matrix-vector product `A x` on this rank's slab, exchanging the

- ```rust
  pub fn solve(self: &Self, comm: &Communicator, b: &[f64], tol: f64, max_iter: usize) -> MpiResult<(Vec<f64>, usize)> { /* ... */ }
  ```
  Solve `A u = b` for the assembled matrix by distributed conjugate gradient.

- ```rust
  pub fn solve_bicgstab(self: &Self, comm: &Communicator, b: &[f64], tol: f64, max_iter: usize) -> MpiResult<(Vec<f64>, usize)> { /* ... */ }
  ```
  Solve `A u = b` by distributed **BiCGStab** — for a **non-symmetric**

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DistributedLduMatrix1D { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `assemble_diffusion_ldu`

Assemble the real face-addressed diffusion matrix for `grid` with per-cell
conductivity `k` and a diagonal Helmholtz `shift` (`> 0` ⇒ SPD).

This is a genuine pflotran-style assembly into
[`outram_foam_basic_lib::ldu_matrix::LduMatrix`]: each internal connection `f`
contributes a symmetric transmissibility `T_f = A_f/d_f · harmonic(k_o, k_n)` to
the diagonal of both cells and to `upper[f]`/`lower[f]`. Face index `f` matches
`grid.connections()[f]` by construction.

# Panics
Debug-asserts `k.len() == grid.n_cells()`.

```rust
pub fn assemble_diffusion_ldu(grid: &crate::grid::CartesianGrid, k: &[f64], shift: f64) -> outram_foam_basic_lib::ldu_matrix::LduMatrix { /* ... */ }
```

#### Function `assemble_advection_diffusion_ldu`

Assemble a **non-symmetric** advection–diffusion `LduMatrix`: the diffusion
assembly of [`assemble_diffusion_ldu`] plus an upwind advection term for a
constant `velocity` (m/s, `+x`).

Upwinding makes `upper[f] ≠ lower[f]` (the matrix is non-symmetric), so the
resulting system must be solved with BiCGStab, not CG — this is the shape of
the real transport Jacobian. The upwind stencil mirrors the `transport`
module's assembly.

```rust
pub fn assemble_advection_diffusion_ldu(grid: &crate::grid::CartesianGrid, k: &[f64], velocity: f64, shift: f64) -> outram_foam_basic_lib::ldu_matrix::LduMatrix { /* ... */ }
```

#### Function `serial_ldu_cg`

Serial CG on a real [`LduMatrix`] using its own [`LduMatrix::multiply`] — the
correctness oracle for the distributed solve.

```rust
pub fn serial_ldu_cg(ldu: &outram_foam_basic_lib::ldu_matrix::LduMatrix, b: &[f64], tol: f64, max_iter: usize) -> (Vec<f64>, usize) { /* ... */ }
```

#### Function `serial_ldu_bicgstab`

Serial BiCGStab on a real [`LduMatrix`] via [`LduMatrix::multiply`] — the
oracle for the distributed non-symmetric solve.

```rust
pub fn serial_ldu_bicgstab(ldu: &outram_foam_basic_lib::ldu_matrix::LduMatrix, b: &[f64], tol: f64, max_iter: usize) -> (Vec<f64>, usize) { /* ... */ }
```

## Module `newton`

Distributed Newton solver for nonlinear systems (bead op-gj5).

The RICHARDS / GENERAL flow modes are **nonlinear** (van Genuchten curves,
upstream-weighted mobility, a pressure-dependent EOS), so distributing them
needs more than a single linear solve: a Newton loop whose residual norm is a
global reduction and whose Jacobian is solved distributed each iteration. This
module provides that outer loop, [`distributed_newton`], built on the
distributed BiCGStab and `all_reduce` primitives.

Given a residual closure `F(x)` (this rank's owned-cell residual, doing its own
halo exchange) and a Jacobian-rows closure returning the local tridiagonal
`(diag, west, east)`, each iteration:

1. evaluate `F(x)` and its global 2-norm (`sqrt(all_reduce(F·F))`);
2. assemble the distributed Jacobian ([`super::ldu::DistributedLduMatrix1D::from_rows`]);
3. solve `J·δx = −F` with distributed BiCGStab;
4. update `x ← x + δx`.

The module test drives a genuinely nonlinear 1-D reaction–diffusion problem and
confirms the distributed Newton reproduces a serial Newton solve cell-for-cell.

# Scope / human-review flags

Verification-only, untrusted AI draft. This is the distributed nonlinear-solve
*framework*; wiring pflotran's exact RICHARDS residual (van Genuchten
saturation/rel-perm + EOS + upstream mobility + gravity) and Jacobian into it —
the last op-gj5 step — requires faithfully replicating those curves per rank
and is a follow-up. Plain Newton (no line search / damping); 1-D tridiagonal.

```rust
pub mod newton { /* ... */ }
```

### Functions

#### Function `distributed_newton`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Solve a nonlinear system `F(x) = 0` by distributed Newton, starting from `x0`.

- `residual(x) -> F` returns this rank's owned-cell residual slab.
- `jac_rows(x) -> (diag, west, east)` returns this rank's local tridiagonal
  Jacobian rows (`west[i]` = ∂F_i/∂x_{i-1}, `east[i]` = ∂F_i/∂x_{i+1}), each
  length `decomp.local_len`, with `0` for off-domain couplings.

Converges when the global residual 2-norm falls below `abs_tol`. Returns the
solution slab and the Newton iteration count.

# Errors
Propagates any transport error from the closures or the distributed linear
solve; [`MpiError::InvalidArgument`] if a closure returns the wrong length.

```rust
pub fn distributed_newton<R, J>(comm: &outram_park_mpi::Communicator, decomp: &super::Decomposition1D, x: Vec<f64>, residual: R, jac_rows: J, abs_tol: f64, max_newton: usize, lin_tol: f64, lin_max: usize) -> outram_park_mpi::MpiResult<(Vec<f64>, usize)>
where
    R: Fn(&[f64]) -> outram_park_mpi::MpiResult<Vec<f64>>,
    J: Fn(&[f64]) -> outram_park_mpi::MpiResult<(Vec<f64>, Vec<f64>, Vec<f64>)> { /* ... */ }
```

## Module `operator`

A distributed **variable-coefficient** diffusion operator (bead op-gj5).

Where [`super::krylov`] demonstrates the distributed solve on a constant-
coefficient toy (the shifted Poisson stencil), this module drives the *same*
generic distributed CG ([`super::krylov::distributed_cg_with`]) with a **real
heterogeneous operator**: 1-D steady diffusion with a spatially-varying
conductivity `k(x)` and harmonic-mean face transmissibilities —

`A u_i = shift·u_i + T^-_i (u_i − u_{i-1}) + T^+_i (u_i − u_{i+1})`,
`T = 2 k_i k_j / (k_i + k_j)` on an interior face, homogeneous Dirichlet (a
zero ghost with `T = k_i`) at a physical domain end.

This is the operator a constant-density steady Darcy / heat-conduction problem
produces on a 1-D grid with heterogeneous permeability — the shape of a real
pflotran Jacobian block, not a toy. The point is that the distributed solve
reproduces the serial one **for a non-trivial, position-dependent matrix**: the
per-rank ghost *coefficients* (not just the field) are halo-exchanged once at
construction, and the field is exchanged each matvec.

# Scope / human-review flags

Verification-only, untrusted AI draft. Symmetric (harmonic-mean) coefficients →
SPD, so plain CG applies. Wiring this as the linear stage inside pflotran's
actual Newton loop on the assembled `LduMatrix` Jacobian, a distributed
preconditioner, and 2-D/3-D/unstructured coefficient operators remain the
op-gj5 follow-ups.

```rust
pub mod operator { /* ... */ }
```

### Types

#### Struct `DiffusionOperator1D`

A distributed 1-D variable-coefficient diffusion operator over a
[`Decomposition1D`]. Holds this rank's conductivities plus its neighbours' edge
conductivities (halo-exchanged once), so a matvec needs only a field halo.

```rust
pub struct DiffusionOperator1D {
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
  pub fn new(comm: &Communicator, decomp: Decomposition1D, k: Vec<f64>, shift: f64) -> MpiResult<Self> { /* ... */ }
  ```
  Build the operator: store `k` (this rank's per-cell conductivity) and

- ```rust
  pub fn matvec(self: &Self, comm: &Communicator, u: &[f64]) -> MpiResult<Vec<f64>> { /* ... */ }
  ```
  Apply the operator to the distributed field `u` (this rank's slab),

- ```rust
  pub fn diagonal(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The operator's diagonal `A_ii` per local cell — the sum of this cell's face

- ```rust
  pub fn solve(self: &Self, comm: &Communicator, b: &[f64], tol: f64, max_iter: usize) -> MpiResult<(Vec<f64>, usize)> { /* ... */ }
  ```
  Solve `A u = b` for this operator by distributed conjugate gradient

- ```rust
  pub fn solve_jacobi_pcg(self: &Self, comm: &Communicator, b: &[f64], tol: f64, max_iter: usize) -> MpiResult<(Vec<f64>, usize)> { /* ... */ }
  ```
  Solve `A u = b` by **Jacobi-preconditioned** distributed CG (via

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DiffusionOperator1D { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `serial_diffusion_matvec`

Serial reference matvec for the variable-coefficient operator over the whole
`n_global`-cell grid — the correctness oracle.

```rust
pub fn serial_diffusion_matvec(n_global: usize, k: &[f64], u: &[f64], shift: f64) -> Vec<f64> { /* ... */ }
```

#### Function `serial_diffusion_cg`

Serial CG reference solving the variable-coefficient system on one rank.

```rust
pub fn serial_diffusion_cg(n_global: usize, k: &[f64], b: &[f64], shift: f64, tol: f64, max_iter: usize) -> (Vec<f64>, usize) { /* ... */ }
```

## Module `transport`

Distributed solute-transport timestep (bead op-gj5).

This is the orchestration slice: a **working distributed implicit transport
timestep** that reproduces the serial [`crate::transport::SoluteTransport`]
step, but with the linear system assembled per-rank and solved with the
distributed BiCGStab ([`super::ldu`]) over an MPI decomposition.

Each backward-Euler step assembles the same operator the serial module does for
a uniform 1-D flow — accumulation `(θ+ρ_bK_d)V/Δt`, upwind advection, and
symmetric dispersion — as a distributed tridiagonal matrix, forms the RHS
`acc·cₒₗₐ`, and solves `A c = b` with distributed BiCGStab (the operator is
non-symmetric because of upwinding). The module test runs several timesteps
and checks the distributed concentration field against the real serial
`SoluteTransport` cell-for-cell.

# Scope / human-review flags

Verification-only, untrusted AI draft. Uniform 1-D flow (constant face flux,
water content, dispersion), Upwind advection (no deferred-correction TVD),
closed ends (zero boundary flux, no Dirichlet BC). It demonstrates the
distributed solver as the linear stage of a real transport timestep;
non-uniform flow, TVD, Dirichlet boundaries, and the RICHARDS/energy timesteps
are the remaining op-gj5 follow-ups.

```rust
pub mod transport { /* ... */ }
```

### Types

#### Struct `DistributedTransport1D`

A distributed 1-D implicit solute-transport stepper for **general (non-uniform)
flow**.

Holds, per owned cell, the retarded storage `(θ+ρ_bK_d)V` and the volumetric
flux `q` and dispersion coupling `d = D_face·θ_face·(A_f/d_f)` of its west and
east internal faces (`0` where there is no such face at a domain end). Each
[`step`](Self::step) advances the concentration one backward-Euler step via a
distributed BiCGStab solve. Build directly with [`new`](Self::new), for uniform
flow with [`uniform`](Self::uniform), or from a global flow field with
[`from_global_flow`](Self::from_global_flow).

```rust
pub struct DistributedTransport1D {
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
  pub fn new(decomp: Decomposition1D, storage_v: Vec<f64>, west_q: Vec<f64>, east_q: Vec<f64>, west_d: Vec<f64>, east_d: Vec<f64>, dt: f64) -> Self { /* ... */ }
  ```
  Build from explicit per-owned-cell arrays (all length `decomp.local_len`).

- ```rust
  pub fn with_flux_limiter(self: Self, limiter: FluxLimiter, uniform_flux: f64) -> Self { /* ... */ }
  ```
  Enable a TVD deferred-correction advection scheme with uniform `+x` face

- ```rust
  pub fn with_ends(self: Self, left: EndCondition, right: EndCondition) -> Self { /* ... */ }
  ```
  Set the boundary conditions at the two global domain ends (default

- ```rust
  pub fn uniform(decomp: Decomposition1D, storage_cell: f64, q: f64, d: f64, dt: f64) -> Self { /* ... */ }
  ```
  Convenience constructor for **uniform** flow: constant face flux `q`,

- ```rust
  pub fn from_global_flow(decomp: &Decomposition1D, water_content: &[f64], face_flux: &[f64], molecular_diffusion: f64, longitudinal_dispersivity: f64, area: f64, geom: f64, storage_v: Vec<f64>, dt: f64) -> Self { /* ... */ }
  ```
  Build this rank's stepper from a **global** uniform-grid flow field: the

- ```rust
  pub fn from_energy_flow(decomp: &Decomposition1D, water_content: &[f64], face_flux: &[f64], rho_cw: f64, rock_heat_capacity: f64, effective_conductivity: f64, geom: f64, cell_volume: f64, dt: f64) -> Self { /* ... */ }
  ```
  Build a distributed **energy (heat) transport** stepper from a global flow

- ```rust
  pub fn step(self: &Self, comm: &Communicator, c: &[f64], tol: f64, max_iter: usize) -> MpiResult<Vec<f64>> { /* ... */ }
  ```
  Advance the concentration `c` (this rank's slab) one implicit timestep,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `EndCondition`

The condition on a global domain-end boundary face, matching the serial
`SoluteTransport` boundary assembly.

```rust
pub enum EndCondition {
    Closed,
    Flux(f64),
    Dirichlet {
        flux: f64,
        dispersion: f64,
        concentration: f64,
    },
}
```

##### Variants

###### `Closed`

No boundary flux and no concentration (a closed end).

###### `Flux`

A boundary flux but no fixed concentration — the serial default: the flux
is added to the diagonal (advective outflow / zero-concentration inflow).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Dirichlet`

A Dirichlet concentration: advection upwinded by the flux sign plus an
always-on dispersive coupling to `concentration`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `flux` | `f64` | Boundary-face volumetric flux (`+` = outflow, `-` = inflow), m³/s. |
| `dispersion` | `f64` | Dispersive coupling `D_face·θ·(A_bnd/d_bnd)`. |
| `concentration` | `f64` | Boundary solute concentration `c_bc`. |

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
    fn clone(self: &Self) -> EndCondition { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &EndCondition) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Types

#### Struct `Decomposition1D`

A balanced contiguous 1-D partition of `n_global` cells over a communicator.

Rank `r` of `p` owns a contiguous block of cells; blocks differ in length by at
most one when `p` does not divide `n_global`. Each rank knows its global cell
offset, its local length, and the ranks of its left/right neighbours (or
`None` at a physical domain end).

```rust
pub struct Decomposition1D {
    pub n_global: usize,
    pub n_ranks: i32,
    pub rank: i32,
    pub start: usize,
    pub local_len: usize,
    pub left: Option<i32>,
    pub right: Option<i32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_global` | `usize` | Total number of cells in the global domain. |
| `n_ranks` | `i32` | Number of ranks the domain is split over. |
| `rank` | `i32` | This rank's id in the communicator. |
| `start` | `usize` | Global index of this rank's first (leftmost) owned cell. |
| `local_len` | `usize` | Number of cells this rank owns. |
| `left` | `Option<i32>` | Left-neighbour rank, or `None` if this subdomain touches the low domain end. |
| `right` | `Option<i32>` | Right-neighbour rank, or `None` if this subdomain touches the high domain end. |

##### Implementations

###### Methods

- ```rust
  pub fn new(n_global: usize, comm: &Communicator) -> Self { /* ... */ }
  ```
  Build the partition for this rank from the global cell count and its

- ```rust
  pub fn global_index(self: &Self, local: usize) -> usize { /* ... */ }
  ```
  Global cell index of this rank's `local`-th owned cell.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Decomposition1D { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Halo`

The ghost values a rank receives from its neighbours in a halo exchange.

`left`/`right` are the neighbour's adjacent edge cell value, or `None` when
this subdomain touches the corresponding physical domain end (where a boundary
condition, not a neighbour value, applies).

```rust
pub struct Halo {
    pub left: Option<f64>,
    pub right: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `left` | `Option<f64>` | Value of the left neighbour's rightmost cell (`None` at the low domain end). |
| `right` | `Option<f64>` | Value of the right neighbour's leftmost cell (`None` at the high domain end). |

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
    fn clone(self: &Self) -> Halo { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Halo) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `exchange_halo`

Perform one nearest-neighbour halo exchange for the subdomain field `local`.

Each rank sends its leftmost owned cell to its left neighbour and its rightmost
to its right neighbour, and receives the neighbours' adjacent edge cells into a
[`Halo`]. Sends are posted before receives; because the transport buffers
eagerly, a full neighbour chain exchanges without deadlock.

# Errors
Propagates any [`outram_park_mpi`] transport error.

# Panics
Does not panic; an empty `local` slab is only valid when the rank has no
neighbours (a degenerate single-rank-with-zero-cells case), handled by sending
nothing.

```rust
pub fn exchange_halo(comm: &outram_park_mpi::Communicator, decomp: &Decomposition1D, local: &[f64]) -> outram_park_mpi::MpiResult<Halo> { /* ... */ }
```

#### Function `jacobi_smooth_distributed`

Distributed Jacobi relaxation of the 1-D Laplace problem `u'' = 0` on
`[0, n_global)` with Dirichlet ends `u[0] = left_bc`, `u[n_global-1] = right_bc`,
run for `iterations` sweeps over a [`Decomposition1D`].

Each sweep: exchange the halo, then set every interior cell to the average of
its two neighbours (using ghost values at subdomain edges and the fixed BC at
the true domain ends). Returns this rank's converged local slab. Because Jacobi
is order-independent within a sweep, the distributed result is **bit-identical**
to the serial computation ([`jacobi_smooth_serial`]) — the module tests assert
this across several rank counts.

# Errors
Propagates any halo-exchange transport error.

```rust
pub fn jacobi_smooth_distributed(comm: &outram_park_mpi::Communicator, decomp: &Decomposition1D, left_bc: f64, right_bc: f64, iterations: usize) -> outram_park_mpi::MpiResult<Vec<f64>> { /* ... */ }
```

#### Function `jacobi_smooth_serial`

The serial reference for [`jacobi_smooth_distributed`]: the same Jacobi sweeps
over the whole `n_global`-cell array on one rank, used as the correctness oracle.

```rust
pub fn jacobi_smooth_serial(n_global: usize, left_bc: f64, right_bc: f64, iterations: usize) -> Vec<f64> { /* ... */ }
```

## Module `energy`

TH energy transport (v1) — bead op-v6s.10.

Heat transport for the thermal-hydraulic (TH) flow mode: conservation of
energy for the cell-centred temperature `T` (K), advected by a frozen Darcy
flow field (from a RICHARDS solve) and conducted through the combined
fluid + rock continuum. This module **mirrors** the conservative
solute-transport module ([`crate::transport`]) — same grid, same `LduMatrix`
assembly, same one-shot BiCGStab + ILU(0) Krylov solve, same test style —
and differs only in the physics: the accumulation term is a volumetric heat
capacity (fluid + rock), the advected quantity is the fluid heat-capacity
rate `rho_w c_w q`, and the "diffusion" is an effective thermal conduction.

v1 is **one-way (weakly) coupled**: the flow field is frozen and the energy
balance is solved on top of it. Buoyancy / temperature-dependent viscosity
feedback on the flow is deferred.

# Governing equation

With volumetric heat capacity `C_v` (J/m^3/K), Darcy volumetric face flux `q`
(m^3/s), water density `rho_w` (kg/m^3), water specific heat `c_w`
(J/kg/K), and effective thermal conductivity `kappa_eff` (W/m/K):

```text
d/dt(C_v * T) + div(rho_w * c_w * q * T) - div(kappa_eff * grad T) = 0
```

With `C_v`, `q`, and `kappa_eff` frozen, this is **linear** in `T`: one
backward-Euler step assembles a single system `A T = b` and solves it once
(BiCGStab + ILU(0)) — no Newton iteration.

# Coefficient definitions (v1 assumptions)

- **Volumetric heat capacity** (per cell `i`):
  `C_v_i = theta_w_i * rho_w * c_w + (1 - phi) * rho_r * c_r` (J/m^3/K),
  where `theta_w_i = water_content[i]` is the volumetric water content, `phi`
  the porosity, and `rho_r`/`c_r` the rock (solid-grain) density / specific
  heat. The fluid part scales with the *actual* water content (so a partially
  saturated cell stores less heat in its fluid); the solid part uses the
  fixed solid fraction `1 - phi`. Homogeneous (single scalar `phi`) in v1.
- **Effective thermal conductivity** (scalar, homogeneous):
  `kappa_eff = phi * kappa_w + (1 - phi) * kappa_r` (W/m/K) — a
  **volume-weighted arithmetic mean** split by *porosity*, i.e. a
  fully-saturated (pore space entirely water) approximation. This is
  deliberately independent of `theta_w`: partially-saturated pores (air
  lowering the pore conductivity) and geometric mean / series-parallel mixing
  models are deferred. The advective heat term is unaffected by this choice.

# Discretisation

Cell-centred finite volume, implicit (backward) Euler in time:

- **Accumulation** `C_v_i V_i / dt (T_i - T_i^old)` — diagonal heat-capacity
  term.
- **Advection** — first-order **upwind** on the face heat-capacity rate
  `w = rho_w c_w q_f` (W/K): the face temperature is taken from the upstream
  cell. Upwind is unconditionally monotone (the assembled matrix is an
  M-matrix) at the cost of numerical diffusion of order `|v| dx / 2`; a
  higher-order / TVD scheme is deferred. Note `w` uses `rho_w c_w q` and does
  **not** re-weight by water content — it is the heat carried by the moving
  water whose volumetric flux is `q`.
- **Conduction** — symmetric two-point flux `d = kappa_eff * area / distance`
  (W/K) using the grid's geometric transmissibility.

# Boundary conditions

- **Default (no BC on a face location):** advective outflow only, with a zero
  conductive gradient. An unspecified face uses the interior temperature (a
  zero-gradient / interior-upwind condition); at an inflow face this carries
  the interior temperature back in, so specify a Dirichlet condition where an
  inlet temperature matters.
- **[`EnergyBoundaryKind::DirichletTemperature`]:** a fixed boundary
  temperature `T_bc` (K). The advective part is upwinded by the boundary flux
  sign (inflow carries `T_bc` in; outflow carries the interior temperature
  out) and the conductive part **always** couples the near-boundary cell to
  `T_bc` across the half-cell distance. Applying the conductive coupling
  regardless of flux sign makes this a genuine fixed-temperature boundary at
  an inflow, outflow, or zero-flux face — the same modelling choice the
  transport module made for `InflowConcentration`, and what the analytical
  verification tests (steady advection–conduction and a pure-conduction
  linear profile) rely on. See the note in [`EnergyTransport::step`].

# Units

Temperature `T` is kelvin (K); volumetric fluxes are m^3/s; water content is
dimensionless; densities kg/m^3; specific heats J/(kg·K); conductivities
W/(m·K); heat capacity `C_v` J/(m^3·K); energies J; time steps seconds. The
API uses plain `f64` (not `uom`) because the energy balance and its Krylov
solve mix quantities of differing dimension; callers apply units at the
case-setup layer.

```rust
pub mod energy { /* ... */ }
```

### Types

#### Struct `ThermalParameters`

Thermal parameters for the TH energy balance (SI units).

All fields are positive physical constants of the fluid (water) and the rock
(solid grains) plus the (homogeneous) porosity. They enter two derived
quantities used by [`EnergyTransport`]:

- the per-cell volumetric heat capacity
  `C_v = theta_w rho_w c_w + (1 - phi) rho_r c_r` (J/m^3/K), and
- the effective thermal conductivity
  `kappa_eff = phi kappa_w + (1 - phi) kappa_r` (W/m/K),

as documented on the [module][crate::energy]. Homogeneous (single scalar per
field) in v1.

```rust
pub struct ThermalParameters {
    pub water_density: f64,
    pub water_specific_heat: f64,
    pub water_conductivity: f64,
    pub rock_density: f64,
    pub rock_specific_heat: f64,
    pub rock_conductivity: f64,
    pub porosity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `water_density` | `f64` | Water density `rho_w`, kg/m^3. Must be finite and `> 0`. |
| `water_specific_heat` | `f64` | Water specific heat capacity `c_w`, J/(kg·K). Must be finite and `> 0`. |
| `water_conductivity` | `f64` | Water thermal conductivity `kappa_w`, W/(m·K). Must be finite and `> 0`. |
| `rock_density` | `f64` | Rock (solid-grain) density `rho_r`, kg/m^3. Must be finite and `> 0`. |
| `rock_specific_heat` | `f64` | Rock specific heat capacity `c_r`, J/(kg·K). Must be finite and `> 0`. |
| `rock_conductivity` | `f64` | Rock thermal conductivity `kappa_r`, W/(m·K). Must be finite and `> 0`. |
| `porosity` | `f64` | Porosity `phi`, dimensionless, strictly in the open interval `(0, 1)`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(water_density: f64, water_specific_heat: f64, water_conductivity: f64, rock_density: f64, rock_specific_heat: f64, rock_conductivity: f64, porosity: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build and validate a set of thermal parameters.

- ```rust
  pub fn water_rock_defaults() -> Self { /* ... */ }
  ```
  Representative liquid-water + generic-rock parameters at ~25 °C.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `EnergyBoundaryKind`

The kind of energy (temperature) boundary condition applied at a face
location.

Enum dispatch (no trait objects), per the workspace design rules. Extended by
adding variants; every `match` on it is then checked for exhaustiveness.

```rust
pub enum EnergyBoundaryKind {
    DirichletTemperature(f64),
}
```

##### Variants

###### `DirichletTemperature`

A fixed boundary temperature `T_bc` (K). Advection is upwinded by the
boundary flux sign; the conductive flux always couples the near-boundary
cell to `T_bc` across the half-cell distance, so this acts as a genuine
fixed-temperature (Dirichlet) boundary at an inflow, outflow, or zero-flux
face.

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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `EnergyBoundaryCondition`

An energy boundary condition bound to one of the six domain-box face
locations.

```rust
pub struct EnergyBoundaryCondition {
    pub location: crate::grid::BoundaryLocation,
    pub kind: EnergyBoundaryKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `location` | `crate::grid::BoundaryLocation` | Which exterior face location this condition applies to. |
| `kind` | `EnergyBoundaryKind` | The condition to impose there. |

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `EnergyTransport`

One implicit-Euler heat-transport step over a frozen flow field.

Holds the grid, the frozen [`FlowField`] (reused verbatim from the transport
module), the [`ThermalParameters`], the temperature boundary conditions, the
time step `dt` (s), and the previous-time temperature `T_old` (K).
[`EnergyTransport::step`] assembles and solves the linear system for the
next-time temperature; repeated stepping (with
[`EnergyTransport::set_previous`] between steps, or by feeding the returned
field back) advances the temperature in time or drives it to steady state.

```rust
pub struct EnergyTransport {
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
  pub fn new(grid: CartesianGrid, flow: FlowField, thermal: ThermalParameters, boundary: Vec<EnergyBoundaryCondition>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a heat-transport stepper from a grid, a frozen flow field, thermal

- ```rust
  pub fn set_timestep(self: &mut Self, dt: f64) { /* ... */ }
  ```
  Set the time step `dt` (seconds). Validated (must be positive and finite)

- ```rust
  pub fn set_previous(self: &mut Self, t_old: &[f64]) { /* ... */ }
  ```
  Set the previous-time temperature `T_old` (K), one value per cell.

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of grid cells (the size of the linear system).

- ```rust
  pub fn step(self: &mut Self, t: &mut Vec<f64>) -> Result<KrylovResult, PflotranError> { /* ... */ }
  ```
  Assemble and solve `A T = b` for the next-time temperature, writing the

- ```rust
  pub fn total_energy(self: &Self, t: &[f64]) -> f64 { /* ... */ }
  ```
  Total thermal energy relative to 0 K, `sum_i V_i C_v_i T_i` (J).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `decay`

Radioactive decay chains with daughter ingrowth (bead op-v6s.15.3).

This module advances a set of coupled radionuclide inventories forward in
time under first-order radioactive decay and the ingrowth of daughter
nuclides. It is the decay-source term used by radionuclide transport in the
OUTRAM PARK nuclear focus, mirroring PFLOTRAN's radioactive-decay reaction
(`reaction_sandbox_radioactive_decay` / the `RADIOACTIVE_DECAY` block).

# Physical model

Each nuclide `i` decays with a **decay constant**

```text
lambda_i = ln(2) / half_life_i          (units: 1/s)
```

A *stable* nuclide has an infinite half-life and hence `lambda_i = 0`.
Nuclides are coupled by branching: a parent `p` decaying produces daughter
`i` with **branching fraction** `b_{p,i}` (moles of `i` created per mole of
`p` that decays; the fractions over all daughters of `p` sum to at most 1,
the remainder being decay to nuclides not tracked in this network). The
governing linear ODE system for the atom densities / concentrations `C_i`
(any consistent amount unit — atoms, mol, mol/L; the equations are linear
and unit-agnostic) is

```text
dC_i/dt = -lambda_i C_i + sum_{p -> i} b_{p,i} lambda_p C_p .
```

Collecting the coefficients into the **decay matrix** `A` (`A[i][i] =
-lambda_i`, `A[i][p] = b_{p,i} lambda_p`), this is `dC/dt = A C`, whose exact
solution over a step `dt` is the matrix exponential

```text
C(t + dt) = exp(A dt) C(t) .
```

## Atom conservation

Column `p` of `A` sums to `-lambda_p (1 - sum_i b_{p,i})`. When every parent
decays entirely within the tracked network (`sum_i b_{p,i} = 1`), every
column sums to zero, `exp(A dt)` is column-stochastic, and the total amount
`sum_i C_i` is conserved exactly (decay just moves atoms down the chain into
the stable end member). With branching loss (`sum_i b_{p,i} < 1`) the lost
fraction leaves the network and the total decreases accordingly — both cases
are handled by the same matrix exponential, so the book-keeping is automatic.

# Numerics — general-chain integration

[`DecayChain::decay`] advances *any* network (linear, branching, or
converging) by forming `exp(A dt)` with **scaling-and-squaring driven by a
truncated Taylor series** (the workhorse "method 3" of Moler & Van Loan,
*Nineteen Dubious Ways to Compute the Exponential of a Matrix*, SIAM Review
45(1) 2003). The scaled matrix `M = A dt / 2^s` is chosen with `s` large
enough that its infinity-norm is `<= 0.5`; the Taylor series
`exp(M) = sum_k M^k / k!` then converges rapidly (terminated once a term is
negligible relative to the running sum, capped at 30 terms), and `exp(A dt)`
is recovered by squaring `s` times. For a norm `<= 0.5` and 30 Taylor terms
the truncation error is far below `f64` round-off, so the method is accurate
to near machine precision (`~1e-12` relative or better) across the stiff
range of decay constants a real chain spans; the unit tests confirm
agreement with the closed-form Bateman solution to `< 1e-6`.

[`DecayChain::bateman_linear`] provides the classic **Bateman (1910)**
closed form for the special case of a single linear chain
`0 -> 1 -> ... -> N-1` with unit branching, as an independent analytical
cross-check of the general integrator.

# Provenance

- Decay-chain / ingrowth theory: H. Bateman, "The solution of a system of
  differential equations occurring in the theory of radioactive
  transformations", *Proc. Cambridge Philos. Soc.* **15** (1910) 423–427.
- Matrix-exponential method: C. Moler & C. Van Loan, SIAM Review **45**(1)
  (2003) 3–49.
- Modelled on PFLOTRAN's radioactive-decay reaction (US-DOE national-lab
  subsurface reactive-transport code; this crate is an independent fork —
  see the crate root and `NOTICE`).

# Status

**Verification-only, untrusted AI-generated draft** (per the workspace
`RESPONSIBLE_USE.md`). Verified against the analytical Bateman solution and
conservation laws in the unit tests below; **no human V&V** has been
performed and it has **not** been validated against a published PFLOTRAN
reference case. Not for nuclear facility operation, safeguards analysis, or
any safety-critical or licensing use — education, research, and V&V only.

```rust
pub mod decay { /* ... */ }
```

### Types

#### Struct `Nuclide`

One nuclide in a [`DecayChain`]: its half-life and the branchings to its
daughters.

The half-life is given in **seconds** and must be strictly positive; use
[`f64::INFINITY`] for a stable nuclide (decay constant `0`). Each entry of
`daughters` names a daughter by its index into the [`DecayChain`]'s nuclide
list together with the branching fraction (moles of daughter produced per
mole of this nuclide that decays). The branching fractions must lie in
`[0, 1]` and sum to at most `1` (any shortfall represents decay to nuclides
outside the tracked network).

```rust
pub struct Nuclide {
    pub name: String,
    pub half_life_seconds: f64,
    pub daughters: Vec<(usize, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable nuclide label (e.g. `"U-238"`). Not interpreted by the<br>solver; carried for reporting and debugging. |
| `half_life_seconds` | `f64` | Half-life in **seconds** (`> 0`; use [`f64::INFINITY`] for a stable<br>nuclide, giving a decay constant of `0`). |
| `daughters` | `Vec<(usize, f64)>` | Branchings to daughter nuclides: `(daughter index, branching fraction)`.<br>Indices must be valid entries of the owning [`DecayChain`]; fractions<br>must be in `[0, 1]` and sum to at most `1`. |

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
    fn clone(self: &Self) -> Nuclide { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `DecayChain`

A decay network of `N` nuclides identified by their index (`0..N`).

Construct with [`DecayChain::new`], which validates the topology and
pre-computes each decay constant. Advance an inventory in place with
[`DecayChain::decay`], or evaluate the closed-form [`DecayChain::bateman_linear`]
for a linear chain.

See the [module documentation](self) for the physical model and numerics.

```rust
pub struct DecayChain {
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
  pub fn new(nuclides: Vec<Nuclide>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a decay chain from its nuclides, validating the topology.

- ```rust
  pub fn n_nuclides(self: &Self) -> usize { /* ... */ }
  ```
  Number of nuclides `N` in the chain.

- ```rust
  pub fn decay_constant(self: &Self, i: usize) -> f64 { /* ... */ }
  ```
  Decay constant `lambda_i = ln(2) / half_life_i` in `1/s` for nuclide `i`

- ```rust
  pub fn nuclide(self: &Self, i: usize) -> &Nuclide { /* ... */ }
  ```
  Nuclide `i` (its name, half-life, and daughter branchings).

- ```rust
  pub fn decay(self: &Self, concentrations: &mut [f64], dt: f64) -> Result<(), PflotranError> { /* ... */ }
  ```
  Advance an inventory over `dt` seconds **in place**, applying radioactive

- ```rust
  pub fn bateman_linear(self: &Self, head_initial: f64, t: f64) -> Result<Vec<f64>, PflotranError> { /* ... */ }
  ```
  Bateman closed-form solution for a **linear** chain

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DecayChain { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `eos_co2_brine`

Real-fluid equations of state for **supercritical/gaseous CO2** and **NaCl
brine** — the fluid pair needed for GENERAL-mode, geologic-CO2-sequestration
(GCS) simulations (bead op-1y6).

This module extends [`crate::eos_real`] (IAPWS-IF97 pure liquid water) toward
the two-fluid, two-component system a CO2-storage model needs: a CO2-rich
non-wetting phase and an NaCl-brine aqueous phase. It provides density and
viscosity for each, as plain-`f64` SI (or documented per-method) inputs.

# ⚠️ ACCURACY WARNING — READ THIS FIRST ⚠️

**The CO2 density here uses the Redlich-Kwong (RK) cubic EOS, which is only a
rough, low-order approximation to the Span & Wagner (1996) reference EOS that
NIST/REFPROP and the real PFLOTRAN CO2 modes use.** In the dense
near-critical / supercritical region that matters most for GCS, **RK
under-predicts CO2 density by ~10-15%** (measured: ~-14% at 40 °C, 10 MPa,
see the tests). Do **not** treat RK CO2 density as quantitatively reliable
for storage-capacity, buoyancy, or plume-migration estimates. It is a
placeholder with the correct qualitative behaviour (monotonic in P and T,
ideal-gas limit) until a Span-Wagner Helmholtz-EOS port lands.

**The CO2 viscosity is the Fenghour-Wakeham-Vesovic (1998) *zero-density*
(dilute-gas) term only** — it depends on temperature alone and **omits the
excess (density-dependent) contribution entirely**, so it materially
under-predicts the viscosity of dense supercritical CO2. Use it only near the
dilute-gas limit; flag every dense-phase use as approximate.

The brine density/viscosity use the Batzle & Wang (1992) correlations, which
are empirical fits valid over the stated ranges (below) and are the same
order of fidelity used in exploration geophysics — reasonable, but still an
empirical correlation, not a reference EOS.

Per the workspace `RESPONSIBLE_USE.md`, **everything here is untrusted
AI-generated draft** until a human reviews it and validates it against
published CO2-brine reference data. No human V&V has been performed.

# Units at the API boundary

Following the PFLOTRAN-fork convention, the public methods take and return
plain `f64`, but the **unit differs per fluid** to match each correlation's
natural units — read each signature:

- [`Co2Properties`] takes **temperature in kelvin (K)** and **pressure in
  pascal (Pa)**; returns density in kg/m^3, viscosity in Pa.s, `Z`
  dimensionless.
- [`Brine`] takes **temperature in degrees Celsius (°C)** and **pressure in
  megapascal (MPa)** — the native units of the Batzle-Wang correlation —
  and returns density in kg/m^3, viscosity in Pa.s.

# Valid ranges (inputs outside these are rejected, not extrapolated)

- **CO2 (RK):** `T > 0 K`, `0 < P <= 500 MPa`, both finite. RK is a global
  cubic so it *returns* a value anywhere in that box, but its accuracy is
  only "order-of-magnitude to ~15%" as warned above, worst near the critical
  point (Tc = 304.13 K, Pc = 7.3773 MPa).
- **Brine (Batzle-Wang):** `0 °C <= T <= 350 °C`, `0 < P <= 100 MPa`, NaCl
  mass fraction `0 <= w <= 0.26` (0.26 ≈ halite saturation near ambient).
  The correlation was fit over roughly 20-350 °C, up to ~100 MPa, and NaCl
  up to saturation.

# Provenance / references

- Redlich, O. & Kwong, J.N.S. (1949). *On the Thermodynamics of Solutions.
  V. An Equation of State. Fugacities of Gaseous Solutions.* Chem. Rev.
  44(1), 233-244. — the cubic EOS used for CO2 density.
- Span, R. & Wagner, W. (1996). *A New Equation of State for Carbon Dioxide
  Covering the Fluid Region from the Triple-Point Temperature to 1100 K at
  Pressures up to 800 MPa.* J. Phys. Chem. Ref. Data 25(6), 1509-1596. —
  the **reference** CO2 EOS that RK here only *approximates* (RK is not this).
- Fenghour, A., Wakeham, W.A. & Vesovic, V. (1998). *The Viscosity of Carbon
  Dioxide.* J. Phys. Chem. Ref. Data 27(1), 31-44. — CO2 viscosity; only the
  zero-density term is used here.
- Batzle, M. & Wang, Z. (1992). *Seismic properties of pore fluids.*
  Geophysics 57(11), 1396-1408. — brine density and brine viscosity
  correlations (Eqs. 27a, 27b and Eq. 29 of that paper).

Higher-fidelity brine viscosity alternatives (Kestin et al. 1981; Mao & Duan
2009) and CO2 solubility in brine (Duan & Sun 2003) are noted as follow-up
work (see the module TODO in [`co2`]/[`brine`]); they are **not** implemented
here.

```rust
pub mod eos_co2_brine { /* ... */ }
```

### Modules

## Module `brine`

NaCl-brine (aqueous-phase) density and viscosity via the **Batzle & Wang
(1992)** empirical correlations.

# Physics

## Density — Batzle-Wang (1992), Eqs. 27a/27b

First the (Batzle-Wang) pure-water density `rho_w` (g/cm^3) as a function of
temperature `T` (°C) and pressure `P` (MPa):

```text
rho_w = 1 + 1e-6 * ( -80 T - 3.3 T^2 + 0.00175 T^3 + 489 P - 2 T P
                     + 0.016 T^2 P - 1.3e-5 T^3 P - 0.333 P^2 - 0.002 T P^2 )
```

then the brine density with NaCl mass fraction `w` (0..1):

```text
rho_b = rho_w + w * ( 0.668 + 0.44 w
        + 1e-6 * ( 300 P - 2400 P w
                   + T ( 80 + 3 T - 3300 w - 13 P + 47 P w ) ) )
```

The result (g/cm^3) is multiplied by 1000 to return kg/m^3. At fixed `T, P`,
`rho_b` rises monotonically with `w`, so brine is always denser than pure
water at the same state — the buoyancy contrast that drives CO2 override in
GCS.

## Viscosity — Batzle-Wang (1992), Eq. 29

```text
eta = 0.1 + 0.333 w + (1.65 + 91.9 w^3)
      * exp( -( 0.42 (w^0.8 - 0.17)^2 + 0.045 ) * T^0.8 )    [centipoise]
```

with `w` the NaCl mass fraction and `T` in °C. The result (cP) is multiplied
by 1e-3 to return Pa.s. Viscosity rises with salinity and falls with
temperature.

# Accuracy / scope

Batzle-Wang are empirical fits (exploration-geophysics fidelity), valid
roughly over `T` in 20-350 °C, `P` up to ~100 MPa, and NaCl up to
saturation. They are reasonable but are **not** a reference EOS. Near ambient
the pure-water term gives ~0.997 g/cm^3 at 20 °C (vs IAPWS 0.9982) — a ~0.1%
low bias inherent to the fit. Higher-fidelity brine viscosity models
(Kestin et al. 1981; Mao & Duan 2009) are noted as follow-up but not
implemented here.

Untrusted AI-generated draft (`RESPONSIBLE_USE.md`); see the parent module
[`crate::eos_co2_brine`] for the full warning and provenance.

```rust
pub mod brine { /* ... */ }
```

### Types

#### Struct `Brine`

NaCl brine (aqueous phase) with a fixed NaCl mass fraction, using the
Batzle & Wang (1992) density and viscosity correlations.

The salinity is stored as [`Brine::nacl_mass_fraction`] (mass fraction, not
molality — dimensionless, `0 <= w <= 0.26`). Because the field is public,
the salinity range is validated inside each method rather than at
construction. See the module documentation for the correlations, units
(`T` in °C, `P` in MPa), valid ranges, and accuracy notes.

```rust
pub struct Brine {
    pub nacl_mass_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nacl_mass_fraction` | `f64` | NaCl mass fraction (dimensionless), valid `0 <= w <= 0.26`<br>(0.26 ≈ halite saturation near ambient). `0.0` is pure water; seawater<br>is ≈ `0.035`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(nacl_mass_fraction: f64) -> Self { /* ... */ }
  ```
  Construct a brine with the given NaCl mass fraction (dimensionless).

- ```rust
  pub fn density(self: &Self, temperature_c: f64, pressure_mpa: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Brine density (kg/m^3) at temperature `temperature_c` (°C) and pressure

- ```rust
  pub fn viscosity(self: &Self, temperature_c: f64, pressure_mpa: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Brine dynamic viscosity (Pa.s) at temperature `temperature_c` (°C).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Brine { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Brine) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `co2`

CO2 real-fluid properties via the **Redlich-Kwong (RK) cubic EOS**
(density, compressibility) plus the **Fenghour-Wakeham-Vesovic (1998)
dilute-gas viscosity** term.

# Physics

## Density — Redlich-Kwong cubic EOS

The RK EOS (Redlich & Kwong, 1949) in pressure-explicit form is

```text
P = R T / (v - b)  -  a / ( sqrt(T) * v * (v + b) )
```

with the CO2-specific constants derived from the critical point
(Tc = 304.13 K, Pc = 7.3773 MPa):

```text
a = 0.42748 * R^2 * Tc^2.5 / Pc
b = 0.08664 * R  * Tc      / Pc
```

Non-dimensionalising with `A = a P / (R^2 T^2.5)` and `B = b P / (R T)`, the
molar volume is replaced by the compressibility factor `Z = P v / (R T)`,
which satisfies the cubic

```text
Z^3 - Z^2 + (A - B - B^2) Z - A B = 0.
```

The **largest real root** is taken as the gas/supercritical compressibility
(the smallest positive root, when three exist, is the liquid branch). Density
then follows from the ideal-gas-corrected relation

```text
rho = P M / (Z R T),   M = 0.04401 kg/mol (CO2 molar mass).
```

**RK is a documented approximation to the Span & Wagner (1996) reference
EOS**, not a substitute for it. It captures the right *shape* (ideal-gas
limit `Z -> 1` as `P -> 0`, `rho` rising with `P`, falling with `T`) but is
quantitatively poor in the dense near-critical region: it under-predicts CO2
density there by ~10-15% (see the tests, which assert against NIST/Span-Wagner
values *with the RK deviation stated honestly*). The acentric factor
`omega = 0.22394` for CO2 is noted for readers who wish to upgrade to
Redlich-Kwong-Soave (RKS) or Peng-Robinson; **plain RK does not use it**, so
it is not a constant here.

## Viscosity — Fenghour-Wakeham-Vesovic (1998), dilute-gas term only

The zero-density (dilute-gas) viscosity is

```text
eta_0(T) = 1.00697 * sqrt(T) / G*(T*)      [microPa.s]
ln G*(T*) = sum_{i=0..4} a_i (ln T*)^i,   T* = T / (epsilon/k),  epsilon/k = 251.196 K
```

with `a = [0.235156, -0.491266, 0.05211155, 0.05347906, -0.01537102]`. This
is **only the density-independent term**: the full FWV correlation adds an
excess viscosity `Delta eta(rho, T)` (and a small critical enhancement) that
is **omitted here**. Consequently [`Co2Properties::viscosity`] is a function
of temperature alone and materially under-predicts dense supercritical CO2
viscosity — use it only near the dilute limit and treat all dense-phase
output as approximate.

# Follow-up (not implemented)

- Replace RK density with a Span-Wagner Helmholtz-energy EOS port.
- Add the FWV excess-viscosity term for dense-phase accuracy.

See the parent module [`crate::eos_co2_brine`] for the full accuracy warning
and provenance block. Untrusted AI-generated draft (`RESPONSIBLE_USE.md`).

```rust
pub mod co2 { /* ... */ }
```

### Types

#### Struct `Co2Properties`

CO2 real-fluid properties via the Redlich-Kwong cubic EOS (density,
compressibility) and the Fenghour-Wakeham-Vesovic dilute-gas viscosity.

**Stateless** — all methods are associated functions taking `(T [K], P [Pa])`
directly; there is nothing to construct. See the module documentation for the
physics, valid ranges, and the (important) accuracy limitations: RK
under-predicts dense CO2 density by ~10-15%, and the viscosity is a
temperature-only dilute-gas term.

```rust
pub struct Co2Properties;
```

##### Implementations

###### Methods

- ```rust
  pub fn compressibility(temperature_k: f64, pressure_pa: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Compressibility factor `Z = P v / (R T)` (dimensionless) at temperature

- ```rust
  pub fn density(temperature_k: f64, pressure_pa: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  CO2 mass density (kg/m^3) at temperature `temperature_k` (K) and pressure

- ```rust
  pub fn viscosity(temperature_k: f64, pressure_pa: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  CO2 dynamic viscosity (Pa.s) at temperature `temperature_k` (K).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Co2Properties { /* ... */ }
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
    fn default() -> Co2Properties { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Co2Properties) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Re-exports

#### Re-export `Brine`

```rust
pub use brine::Brine;
```

#### Re-export `Co2Properties`

```rust
pub use co2::Co2Properties;
```

## Module `eos_real`

Real IAPWS-IF97 liquid-water equation of state — a higher-fidelity
alternative to the slightly-compressible correlations in
[`crate::properties`] (bead op-v6s.15.7).

# What this module provides

[`IapwsWater`] wraps the workspace `tampines-steam-tables` crate (an in-house
IAPWS-IF97 industrial-formulation implementation) and exposes liquid-water
**density**, **dynamic viscosity**, **specific enthalpy**, **specific
internal energy**, and **isobaric specific heat** as functions of pressure
and temperature. It is a drop-in, higher-fidelity replacement for
[`crate::properties::LiquidWaterEos`], which uses a single
slightly-compressible exponential density law and a constant viscosity.

# Units — SI throughout, plain `f64` at the boundary

Unlike the `uom`-typed `tampines-steam-tables` API this module talks to, the
public methods here take and return **plain `f64` SI values** so downstream
PFLOTRAN solver code (which assembles residuals/Jacobians in `f64`) can use
them directly. The `uom` conversions happen internally. Specifically:

- pressure `p` — pascal (Pa)
- temperature `T` — kelvin (K)
- density — kilogram per cubic metre (kg/m^3)
- dynamic viscosity — pascal-second (Pa.s)
- specific enthalpy / internal energy — joule per kilogram (J/kg)
- isobaric specific heat `c_p` — joule per kilogram-kelvin (J/(kg.K))

# Valid region — IAPWS-IF97 Region 1 (subcooled/compressed liquid) only

v1 covers **single-phase liquid water only**, i.e. IAPWS-IF97 **Region 1**:

- temperature `273.15 K <= T <= 623.15 K` (0 to ~350 °C), and
- pressure `p_sat(T) < p <= 100 MPa`, i.e. strictly above the saturation
  pressure at `T` (compressed/subcooled liquid) and up to 100 MPa.

A `(p, T)` state that is on or below the saturation line (vapour or the
two-phase dome), above 623.15 K (Regions 2/3/5), or above 100 MPa is
**rejected** with [`PflotranError::InvalidInput`] rather than silently
returning an out-of-region extrapolation. Two-phase, vapour, near-critical,
and supercritical states are out of scope for this v1 liquid EOS.

# Underlying `tampines-steam-tables` calls

All property evaluations route through the crate's single-phase `(T,p)`
forward-flash functions in
`tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm`:
`v_tp_eqm_single_phase` (specific volume -> density), `h_tp_eqm_single_phase`
(enthalpy), `u_tp_eqm_single_phase` (internal energy), and
`cp_tp_eqm_single_phase` (isobaric heat capacity); plus
`mu_tp_eqm_single_phase` from that crate's `dynamic_viscosity` module
(re-exported through `pt_flash_eqm`). The saturation-line guard uses
`tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4`.

# Verification status

Untrusted AI-generated draft until a human reviews it (workspace
`RESPONSIBLE_USE.md`). The `tampines-steam-tables` crate is itself verified
against the International Steam Tables (Kretzschmar & Wagner, 2019); this
wrapper only re-expresses those calls in `f64` SI and adds the Region-1
input guard. The tests below assert the wrapper is wired correctly against
reference IAPWS liquid-water values — they are not a re-validation of IF97.

```rust
pub mod eos_real { /* ... */ }
```

### Types

#### Struct `IapwsWater`

Real IAPWS-IF97 liquid-water properties, wrapping `tampines-steam-tables`.

A drop-in, higher-fidelity alternative to
[`crate::properties::LiquidWaterEos`] (which uses simple
slightly-compressible correlations). SI units throughout; all methods take
plain `f64` pressure (Pa) and temperature (K) and return plain `f64` SI
results, converting to/from `uom` internally.

The evaluator is **stateless** — it holds no configuration — so a single
[`IapwsWater::new`] (or [`IapwsWater::default`]) instance can be shared and
called freely. All methods validate that the requested `(p, T)` lies in
IAPWS-IF97 Region 1 (single-phase liquid) and return
[`PflotranError::InvalidInput`] otherwise; see the module documentation for
the exact region.

```rust
pub struct IapwsWater {
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  Construct the (stateless) IAPWS-IF97 liquid-water evaluator.

- ```rust
  pub fn density(self: &Self, pressure_pa: f64, temperature_k: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Liquid density (kg/m^3) at pressure `p` (Pa) and temperature `T` (K).

- ```rust
  pub fn viscosity(self: &Self, pressure_pa: f64, temperature_k: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Dynamic viscosity (Pa.s) at pressure `p` (Pa) and temperature `T` (K).

- ```rust
  pub fn enthalpy(self: &Self, pressure_pa: f64, temperature_k: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Specific enthalpy (J/kg) at pressure `p` (Pa) and temperature `T` (K).

- ```rust
  pub fn internal_energy(self: &Self, pressure_pa: f64, temperature_k: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Specific internal energy (J/kg) at pressure `p` (Pa) and temperature

- ```rust
  pub fn specific_heat(self: &Self, pressure_pa: f64, temperature_k: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Isobaric specific heat `c_p` (J/(kg.K)) at pressure `p` (Pa) and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> IapwsWater { /* ... */ }
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
    fn default() -> IapwsWater { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &IapwsWater) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `error`

Error type for `outram-park-fork-pflotran`.

A single [`PflotranError`] enum covers the crate's failure modes. During the
scaffold phase the most common variant is [`PflotranError::NotImplemented`]:
entry points whose physics is not yet translated return it, so a caller can
never mistake an unfinished stub for a real result.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `PflotranError`

**Attributes:**

- `NonExhaustive`

Errors returned by `outram-park-fork-pflotran`.

This enum is expected to grow as flow modes, the solver, and I/O land
(beads op-v6s.4..op-v6s.8). New variants are added rather than overloading a
generic string, so callers can `match` on the specific failure.

```rust
pub enum PflotranError {
    NotImplemented(&'static str),
    InvalidInput(String),
    Convergence(String),
    Io(String),
}
```

##### Variants

###### `NotImplemented`

The requested functionality is a documented scaffold and has no real
implementation yet. Carries a short label naming the missing piece
(e.g. `"RICHARDS flow-mode solve"`).

Returning this — instead of `todo!()`/`unimplemented!()` panics or a
fabricated value — is how the scaffold stays honest per the workspace
`RESPONSIBLE_USE.md` rule.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `InvalidInput`

A supplied physical input was outside its valid range or otherwise
inconsistent (e.g. a negative porosity, a saturation above 1).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Convergence`

A nonlinear (Newton) or linear (Krylov) solve failed to converge within
its iteration/tolerance budget.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Io`

A file read/write or on-disk-format error — e.g. an HDF5 snapshot that
cannot be written, opened, or parsed ([`crate::hdf5_io`]).

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

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `flow`

Flow-mode dispatch — the enum that stands in for PFLOTRAN's process-model
(`pm_*`) polymorphism.

PFLOTRAN selects a *mode* (RICHARDS, TH, GENERAL, ...) that defines the
governing equations, primary unknowns, and residual/Jacobian assembly.
Upstream this is C-style polymorphism over a base process-model type; here it
is a closed [`FlowMode`] enum matched exhaustively, per the workspace
"no trait objects — use enums for dispatch" rule. Adding a mode is then a
compile error at every `match` until it is handled.

v1 implements the [`richards`] mode end-to-end: a [`FlowMode::Richards`] owns
a running [`RichardsSimulation`] and [`FlowMode::step`]/[`FlowMode::run`]
advance it in time.

> **Untrusted AI draft, verification-only.** The RICHARDS physics has been
> verified (analytical steady state + method of manufactured solutions), not
> validated against published PFLOTRAN reference cases. See the crate README
> "Bookkeeping status" and `docs/ai-directed-decisions.md`.

```rust
pub mod flow { /* ... */ }
```

### Modules

## Module `richards`

RICHARDS flow mode — variably-saturated single-phase groundwater flow.

Solves the liquid-phase mass-conservation (Richards') equation

$$ \frac{\partial}{\partial t}\left(\phi\, S_l\, \rho_l\right) + \nabla \cdot \left(\rho_l\, \mathbf{q}_l\right) = Q_l $$

with the Darcy flux

$$ \mathbf{q}_l = -\frac{k\, k_{rl}}{\mu_l}\left(\nabla p_l - \rho_l\, \mathbf{g}\right) $$

discretised in space by a **cell-centred two-point flux** finite volume on a
structured Cartesian [`grid`](crate::grid), and in time by **backward
(implicit) Euler**. The single primary unknown per cell is the liquid
pressure `p_l` (Pa). The nonlinear system `F(p) = 0` at each timestep is
handed to the crate's [`NewtonSolver`](crate::solver::NewtonSolver), whose
linear step uses `outram-foam-basic-lib`'s asymmetric Krylov solvers.

Mobility (`k_rl`) is **upstream-weighted** on each face — the physically
correct choice for advective transport, and the reason the Jacobian is
nonsymmetric (hence BiCGStab / GMRES rather than CG).

## Sign & reference conventions

- `z` is elevation (upward positive); gravity magnitude `g >= 0` acts in `-z`.
- Flow potential on a connection: `Phi = p + rho_face * g * z`.
- Capillary pressure `p_c = p_gas - p_l`, with a fixed reference gas pressure
  `p_gas` (default atmospheric). `p_c <= 0` means fully saturated.
- Residual `F_i` is a mass rate (kg/s); positive face/boundary flux leaves
  the cell. Unspecified boundaries are **no-flow** (natural zero Neumann).

> **Untrusted AI draft — verification only.** See `docs/ai-directed-decisions.md`
> (esp. D6, D8) and the crate README "Bookkeeping status". No validation
> against published PFLOTRAN results has been done.

```rust
pub mod richards { /* ... */ }
```

### Types

#### Enum `BoundaryConditionKind`

What is prescribed on one exterior boundary location.

Pressures are in pascal (Pa); Neumann fluxes are a Darcy velocity in metres
per second (m/s), **positive = inflow into the domain**.

```rust
pub enum BoundaryConditionKind {
    DirichletPressure(f64),
    NeumannFlux(f64),
}
```

##### Variants

###### `DirichletPressure`

Fixed liquid pressure `p` (Pa) on the boundary (first-type / Dirichlet).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `NeumannFlux`

Fixed normal Darcy flux `q` (m/s), positive pointing **into** the
domain (second-type / Neumann). `q = 0` is a no-flow wall.

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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BoundaryCondition`

A boundary condition applied to one of the six Cartesian domain faces.

```rust
pub struct BoundaryCondition {
    pub location: crate::grid::BoundaryLocation,
    pub kind: BoundaryConditionKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `location` | `crate::grid::BoundaryLocation` | Which exterior face of the logical box this applies to. |
| `kind` | `BoundaryConditionKind` | What is prescribed there. |

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `RichardsProblem`

The RICHARDS nonlinear system for a single implicit-Euler timestep.

Owns the grid, fluid EOS, characteristic curves, and homogeneous material
data for the v1 slice (isotropic permeability, uniform porosity). Implements
[`NonlinearSystem`] so it can be driven by [`NewtonSolver`]. The Jacobian is
assembled **numerically** (local finite differences of the residual over the
two-point stencil) — this is exact-to-round-off, matches the residual by
construction, and side-steps the singular analytical `dk_r/dS` slope of the
van Genuchten–Mualem model near full saturation (decision D8/D6).

```rust
pub struct RichardsProblem {
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
  pub fn new(grid: CartesianGrid, eos: LiquidWaterEos, curves: CharacteristicCurves, porosity: f64, permeability: f64, gravity: f64, reference_gas_pressure: f64, boundary: Vec<BoundaryCondition>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a RICHARDS problem.

- ```rust
  pub fn set_timestep(self: &mut Self, dt: f64) { /* ... */ }
  ```
  Set the implicit-Euler timestep `dt` (s) for the next assembly. Must be

- ```rust
  pub fn set_previous(self: &mut Self, p_old: &[f64]) { /* ... */ }
  ```
  Set the previous-time-level pressure field (Pa), length `n_cells`.

- ```rust
  pub fn set_source(self: &mut Self, source: &[f64]) { /* ... */ }
  ```
  Set a per-cell volumetric mass source (kg/s), positive = injection.

- ```rust
  pub fn grid(self: &Self) -> &CartesianGrid { /* ... */ }
  ```
  Read access to the underlying grid.

- ```rust
  pub fn total_mass(self: &Self, p: &[f64]) -> f64 { /* ... */ }
  ```
  Total fluid mass in the domain (kg) for a pressure field `p`:

- ```rust
  pub fn flow_field(self: &Self, p: &[f64]) -> crate::transport::FlowField { /* ... */ }
  ```
  Export the [`FlowField`](crate::transport::FlowField) (volumetric Darcy

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **NonlinearSystem**
  - ```rust
    fn n_dof(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn ldu_addressing(self: &Self) -> (Vec<usize>, Vec<usize>) { /* ... */ }
    ```

  - ```rust
    fn assemble_residual(self: &mut Self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> { /* ... */ }
    ```

  - ```rust
    fn assemble_jacobian(self: &mut Self, x: &[f64], jac: &mut LduMatrix) -> Result<(), PflotranError> { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TimeControls`

Timestep controller for a transient RICHARDS run.

All times in seconds. `dt` grows by `growth_factor` after a converged step
(capped at `max_dt`) and is cut by `cut_factor` on a failed nonlinear solve,
aborting if it would fall below `min_dt`.

```rust
pub struct TimeControls {
    pub final_time: f64,
    pub initial_dt: f64,
    pub max_dt: f64,
    pub min_dt: f64,
    pub growth_factor: f64,
    pub cut_factor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `final_time` | `f64` | Simulation end time (s). |
| `initial_dt` | `f64` | Initial timestep (s). |
| `max_dt` | `f64` | Maximum timestep (s). |
| `min_dt` | `f64` | Minimum timestep (s) before the run gives up. |
| `growth_factor` | `f64` | Multiplicative growth factor on a converged step (> 1). |
| `cut_factor` | `f64` | Multiplicative cut factor on a failed step (in (0,1)). |

##### Implementations

###### Methods

- ```rust
  pub fn from_time_spec(final_time: f64, initial_dt: f64, max_dt: f64) -> Self { /* ... */ }
  ```
  Controls from a parsed deck's `TIME` card, with sensible growth/cut

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `StepReport`

Outcome of a single [`RichardsSimulation::step`].

```rust
pub struct StepReport {
    pub time: f64,
    pub dt: f64,
    pub newton_iterations: usize,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time` | `f64` | Simulation time (s) reached at the end of this step. |
| `dt` | `f64` | The timestep size (s) actually taken (after any cutting). |
| `newton_iterations` | `usize` | Newton iterations used by the accepted solve. |
| `converged` | `bool` | Whether the step converged (always true on `Ok`). |

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
    fn clone(self: &Self) -> StepReport { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `RunReport`

Summary of a full [`RichardsSimulation::run`].

```rust
pub struct RunReport {
    pub steps: usize,
    pub final_time: f64,
    pub total_newton_iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `steps` | `usize` | Number of accepted timesteps. |
| `final_time` | `f64` | Final simulation time reached (s). |
| `total_newton_iterations` | `usize` | Total Newton iterations across all accepted steps. |

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
    fn clone(self: &Self) -> RunReport { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `RichardsSimulation`

A transient RICHARDS simulation: a [`RichardsProblem`] plus a
[`NewtonSolver`], time controls, and the evolving pressure field.

```rust
pub struct RichardsSimulation {
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
  pub fn new(problem: RichardsProblem, newton: NewtonConfig, time: TimeControls, initial_pressure: f64) -> Self { /* ... */ }
  ```
  Assemble a simulation from a problem, Newton configuration, time

- ```rust
  pub fn from_input_deck(deck: &InputDeck) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a runnable simulation directly from a parsed input deck, wiring the

- ```rust
  pub fn pressure(self: &Self) -> &[f64] { /* ... */ }
  ```
  Current liquid-pressure field (Pa), length `n_cells`.

- ```rust
  pub fn saturation(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Current liquid-saturation field `S_l` (dimensionless), length `n_cells`.

- ```rust
  pub fn current_time(self: &Self) -> f64 { /* ... */ }
  ```
  Simulation time reached so far (s).

- ```rust
  pub fn problem(self: &Self) -> &RichardsProblem { /* ... */ }
  ```
  The underlying problem (read-only), e.g. for grid access.

- ```rust
  pub fn total_mass(self: &Self) -> f64 { /* ... */ }
  ```
  Total fluid mass currently in the domain (kg). See

- ```rust
  pub fn flow_field(self: &Self) -> crate::transport::FlowField { /* ... */ }
  ```
  The current [`FlowField`](crate::transport::FlowField) (Darcy face fluxes

- ```rust
  pub fn step(self: &mut Self) -> Result<StepReport, PflotranError> { /* ... */ }
  ```
  Advance by one adaptive timestep. Cuts `dt` and retries on a failed

- ```rust
  pub fn run(self: &mut Self) -> Result<RunReport, PflotranError> { /* ... */ }
  ```
  Run to the configured final time, returning the run summary.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Constants and Statics

#### Constant `STANDARD_GRAVITY`

Standard gravitational acceleration (m/s^2), the default for a simulation.

```rust
pub const STANDARD_GRAVITY: f64 = 9.806_65;
```

#### Constant `ATMOSPHERIC_PRESSURE`

Standard atmospheric pressure (Pa), the default reference gas pressure.

```rust
pub const ATMOSPHERIC_PRESSURE: f64 = 101_325.0;
```

### Types

#### Enum `FlowMode`

**Attributes:**

- `NonExhaustive`

A closed set of subsurface flow modes.

v1 implements [`FlowMode::Richards`] (variably-saturated single-phase flow).
TH, GENERAL (multiphase), and transport modes are added as later variants
(beads op-v6s.10, op-v6s.13, op-v6s.11); the `#[non_exhaustive]` marker keeps
downstream `match`es forward-compatible.

```rust
pub enum FlowMode {
    Richards(RichardsSimulation),
}
```

##### Variants

###### `Richards`

Variably-saturated single-phase groundwater flow (Richards' equation),
carrying its live time-stepping simulation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RichardsSimulation` |  |

##### Implementations

###### Methods

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-readable name of the active flow mode (e.g. `"RICHARDS"`).

- ```rust
  pub fn step(self: &mut Self) -> crate::Result<StepReport> { /* ... */ }
  ```
  Advance the flow solution by one (adaptive) timestep, returning the

- ```rust
  pub fn run(self: &mut Self) -> crate::Result<RunReport> { /* ... */ }
  ```
  Run the simulation to its configured final time, returning the run

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Re-exports

#### Re-export `BoundaryCondition`

```rust
pub use richards::BoundaryCondition;
```

#### Re-export `BoundaryConditionKind`

```rust
pub use richards::BoundaryConditionKind;
```

#### Re-export `RichardsProblem`

```rust
pub use richards::RichardsProblem;
```

#### Re-export `RichardsSimulation`

```rust
pub use richards::RichardsSimulation;
```

#### Re-export `RunReport`

```rust
pub use richards::RunReport;
```

#### Re-export `StepReport`

```rust
pub use richards::StepReport;
```

#### Re-export `TimeControls`

```rust
pub use richards::TimeControls;
```

## Module `general_mode`

GENERAL mode — non-isothermal air–water–energy 3-phase-DOF flow (bead
op-v6s.15.5).

This module is the **non-isothermal extension** of the isothermal two-phase
air–water solver in [`crate::multiphase`]. Where [`crate::multiphase`] carries
**two** primary unknowns per cell — liquid pressure `P_l` (Pa) and liquid
saturation `S_l` (dimensionless) — and solves two coupled mass balances, this
module **adds temperature `T` (K) as a third unknown**, giving a coupled
**three-DOF** (`nb = 3`) air–water–energy system: two phase mass balances plus
one bulk (fluid + rock) energy balance. It is a deliberately *simplified*
rendering of PFLOTRAN's GENERAL flow mode.

The physics reuses the two-phase closure from [`crate::multiphase`] verbatim
in spirit (capillary pressure by retention-curve inversion, upstream-weighted
per-phase mobilities, a quadratic-Corey gas relative permeability) and adds
**temperature-dependent liquid properties** — density `rho_l(P_l, T)` and
viscosity `mu_l(T)` from [`ThermalWaterProperties`] — so that temperature
genuinely feeds back into the flow, not just rides along it.

# Governing equations (three per cell, backward-Euler in time)

Discretised by a cell-centred **two-point flux** finite volume in space and
**backward (implicit) Euler** in time. Per cell:

**1. Water (liquid) mass**

$$ \frac{\partial}{\partial t}\!\left(\phi\, \rho_l\, S_l\right) + \nabla\cdot\!\left(\rho_l\, \lambda_l\,(\nabla P_l - \rho_l\, \mathbf{g})\right) = 0, \qquad \lambda_l = \frac{k\, k_{rl}(S_l)}{\mu_l(T)} $$

**2. Air (gas) mass**

$$ \frac{\partial}{\partial t}\!\left(\phi\, \rho_g\, S_g\right) + \nabla\cdot\!\left(\rho_g\, \lambda_g\,(\nabla P_g - \rho_g\, \mathbf{g})\right) = 0, \qquad S_g = 1 - S_l, \quad P_g = P_l + P_c(S_l) $$

**3. Energy (bulk fluid + rock)**

$$ \frac{\partial}{\partial t}\!\left[\phi\,(\rho_l S_l u_l + \rho_g S_g u_g) + (1-\phi)\, \rho_r c_r (T - T_{\text{ref}})\right] + \nabla\cdot\!\left(\rho_l h_l \lambda_l \mathbf{f}_l + \rho_g h_g \lambda_g \mathbf{f}_g\right) - \nabla\cdot\!\left(\lambda_{\text{bulk}}\, \nabla T\right) = 0 $$

where `f_l`, `f_g` are the per-phase Darcy driving terms
`(grad P_p - rho_p g)`, `u_p`/`h_p` are the phase specific internal
energy / enthalpy, and `lambda_bulk` is the effective bulk thermal
conductivity. The energy advection is carried by **both** phase mass fluxes
(upstream-weighted on the phase enthalpy) and conduction by `lambda_bulk`.

# Temperature coupling (why `T` is not inert)

Temperature couples **back into the flow** through the temperature-dependent
liquid properties supplied by [`ThermalWaterProperties`]:
- `rho_l(P_l, T)` — density falls with `T` (thermal expansion), changing both
  the accumulation term and the gravity head;
- `mu_l(T)` — viscosity falls with `T`, *raising* the liquid mobility
  `lambda_l` and so speeding liquid flow in hot regions.
Gas properties (`rho_g`, `mu_g`) are constant in this v1.

# Simplifications — flags for human review (this is an untrusted AI draft)

Per the workspace `RESPONSIBLE_USE.md`, this is **untrusted AI-generated draft
material, verification-only**, not validated against any published PFLOTRAN
GENERAL-mode reference. It is a *simplified* GENERAL mode. Real GENERAL-mode
features deliberately **omitted** here:
- **No component partitioning between phases** — the air and water components
  do not dissolve/evaporate into each other; there is one air component in the
  gas and one water component in the liquid, each confined to its phase.
- **No phase appearance / disappearance handling** beyond clamping `S_l` into
  `[S_r, 1]` for the property closures; there is no primary-variable switching.
- **No latent heat / phase change** — no boiling, condensation, or evaporation
  enthalpy.
- **Ideal / constant-`c_p` energy** — internal energy and enthalpy are
  simplified to `u = h = c_p (T - T_ref)` with a constant per-phase `c_p` and a
  fixed reference temperature `T_ref` (25 °C by default). No real-gas EOS.
- **Constant gas density and viscosity** (constant-density air).
- **Bulk conductivity** is a porosity-weighted arithmetic mean
  `lambda_bulk = phi k_l + (1 - phi) k_r` (fully-saturated approximation,
  independent of `S_l`), matching [`crate::energy`].

# State-vector layout

All state vectors are interleaved, length `3 * n_cells`: cell `c` occupies
`[3c] = P_l` (Pa), `[3c + 1] = S_l` (dimensionless), `[3c + 2] = T` (K),
matching the block solver's `nb = 3` convention. Where `S_l` feeds the
property closures it is clamped to `[S_r, 1]`; the accumulation terms use the
raw `S_l`/`T` so the discrete balances stay exact.

# Sign & reference conventions

- `z` is elevation (upward positive); gravity magnitude `g >= 0` acts in `-z`.
- Per-face phase potential `Phi_p = P_p + rho_p g z`; a positive face/boundary
  flux **leaves** the cell.
- Unspecified boundary faces are **no-flow + adiabatic** (the natural default).

# Jacobian

The `3x3`-block Jacobian is assembled **numerically** — local finite
differences of the three residual equations with respect to the three local
unknowns over the two-point stencil (see [`GeneralFlow::assemble_jacobian`]).
This matches the residual by construction and side-steps fragile analytic
derivatives through the upstream switch, the capillary-pressure inversion, and
the temperature-dependent property closures — the standard tractable choice
for a strongly coupled `nb = 3` system.

# Provenance

PFLOTRAN GENERAL mode (documentation.pflotran.org; Lichtner et al.,
*PFLOTRAN Theory Guide*) and the standard black-oil / compositional
multiphase-with-energy formulation. Independent pure-Rust reimplementation,
not the official PFLOTRAN and not endorsed by its authors.

> **Untrusted AI draft — verification only.** Not for facility operation or
> any safety-critical use.

```rust
pub mod general_mode { /* ... */ }
```

### Types

#### Struct `GeneralFluids`

Air–water–energy fluid + rock properties for the GENERAL mode.

Bundles everything the three governing equations need beyond the geometry and
the characteristic curves:
- **Liquid water** as a temperature-dependent [`ThermalWaterProperties`]
  (density `rho_l(P_l, T)`, viscosity `mu_l(T)`, specific heat `c_pl`);
- **Gas (air)** as a *constant*-property phase (`rho_g`, `mu_g`, `c_pg`);
- **Rock** thermal properties [`RockThermalProperties`] (`rho_r`, `c_r`,
  conductivity `k_r`) for the solid part of the bulk energy storage and
  conduction;
- the **enthalpy reference temperature** `T_ref` (K) at which the simplified
  internal energy / enthalpy `u = h = c_p (T - T_ref)` is zero.

The liquid specific heat `c_pl` and liquid thermal conductivity `k_l` are read
from the embedded [`ThermalWaterProperties`]; only the *gas* specific heat is
stored separately (the gas is otherwise constant-property).

```rust
pub struct GeneralFluids {
    pub water: crate::properties::ThermalWaterProperties,
    pub gas_density: f64,
    pub gas_viscosity: f64,
    pub gas_specific_heat: f64,
    pub rock: crate::properties::RockThermalProperties,
    pub reference_temperature: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `water` | `crate::properties::ThermalWaterProperties` | Temperature-dependent liquid-water properties: `rho_l(P_l, T)`, `mu_l(T)`,<br>liquid specific heat `c_pl` (J/(kg·K)) and conductivity `k_l` (W/(m·K)). |
| `gas_density` | `f64` | Gas (air) mass density `rho_g`, kg/m^3, constant. Strictly positive. |
| `gas_viscosity` | `f64` | Gas (air) dynamic viscosity `mu_g`, Pa·s, constant. Strictly positive. |
| `gas_specific_heat` | `f64` | Gas (air) specific heat at constant pressure `c_pg`, J/(kg·K). Strictly<br>positive. Enters the gas internal energy / enthalpy `u_g = h_g = c_pg<br>(T - T_ref)`. |
| `rock` | `crate::properties::RockThermalProperties` | Solid-matrix (rock) thermal properties: density `rho_r`, specific heat<br>`c_r`, conductivity `k_r`. |
| `reference_temperature` | `f64` | Enthalpy / internal-energy reference temperature `T_ref`, K. The simplified<br>per-phase internal energy and enthalpy are `u = h = c_p (T - T_ref)`. |

##### Implementations

###### Methods

- ```rust
  pub fn air_water() -> Self { /* ... */ }
  ```
  Representative **air–water at ~20–25 °C** defaults: liquid water from

- ```rust
  pub fn new(water: ThermalWaterProperties, gas_density: f64, gas_viscosity: f64, gas_specific_heat: f64, rock: RockThermalProperties, reference_temperature: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a validated air–water–energy fluid/rock bundle.

- ```rust
  pub fn liquid_specific_heat(self: &Self) -> f64 { /* ... */ }
  ```
  Liquid specific heat `c_pl` (J/(kg·K)), read from the embedded thermal

- ```rust
  pub fn liquid_density(self: &Self, p_l: f64, t: f64) -> f64 { /* ... */ }
  ```
  Liquid mass density `rho_l(P_l, T)` (kg/m^3) at liquid pressure `p_l` (Pa)

- ```rust
  pub fn liquid_viscosity(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Liquid dynamic viscosity `mu_l(T)` (Pa·s) at temperature `t` (K). Uses

- ```rust
  pub fn liquid_enthalpy(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Liquid specific internal energy / enthalpy `u_l = h_l = c_pl (T - T_ref)`

- ```rust
  pub fn gas_enthalpy(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Gas specific internal energy / enthalpy `u_g = h_g = c_pg (T - T_ref)`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> GeneralFluids { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GeneralFluids) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `GeneralBoundaryKind`

What is prescribed on one exterior boundary location for GENERAL-mode flow.

Enum dispatch (no trait objects) per the workspace design rules. Unspecified
faces default to [`GeneralBoundaryKind::NoFlow`].

```rust
pub enum GeneralBoundaryKind {
    Dirichlet {
        liquid_pressure: f64,
        liquid_saturation: f64,
        temperature: f64,
    },
    NoFlow,
}
```

##### Variants

###### `Dirichlet`

Fixed liquid pressure (Pa), liquid saturation (dimensionless in `[S_r, 1]`)
**and** temperature (K) at the boundary. The ghost state's gas pressure
follows the capillary closure `P_g = P_l + P_c(S_l)`; both phases can
exchange mass across the face, advecting enthalpy, and the conductive term
couples the near-boundary cell to `T_bc` across the half-cell distance
(a genuine fixed-temperature boundary regardless of flow direction — the
same modelling choice [`crate::energy`] makes).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `liquid_pressure` | `f64` | Prescribed liquid pressure `P_l` at the boundary, Pa. |
| `liquid_saturation` | `f64` | Prescribed liquid saturation `S_l` at the boundary, dimensionless in<br>`[S_r, 1]`. |
| `temperature` | `f64` | Prescribed temperature `T` at the boundary, K. |

###### `NoFlow`

No-flow **and adiabatic** (zero normal flux for both phases and zero
conductive heat flux). The natural default.

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
    fn clone(self: &Self) -> GeneralBoundaryKind { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GeneralBoundaryKind) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GeneralBoundaryCondition`

A GENERAL-mode boundary condition applied to one of the six Cartesian faces.

```rust
pub struct GeneralBoundaryCondition {
    pub location: crate::grid::BoundaryLocation,
    pub kind: GeneralBoundaryKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `location` | `crate::grid::BoundaryLocation` | Which exterior face of the logical box this applies to. |
| `kind` | `GeneralBoundaryKind` | What is prescribed there. |

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
    fn clone(self: &Self) -> GeneralBoundaryCondition { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GeneralBoundaryCondition) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GeneralFlow`

GENERAL-mode 3-DOF (`P_l`, `S_l`, `T`) non-isothermal air–water problem for a
single backward-Euler timestep.

Owns an `Arc<CartesianGrid>` (shared, read-only topology — no lifetimes, per
the workspace rules), the characteristic curves, the [`GeneralFluids`] bundle,
and homogeneous material data (isotropic permeability, uniform porosity).
Implements [`BlockNonlinearSystem`] with `dof_per_cell = 3`, so it is driven
by the generic [`BlockNewtonSolver`].

# Energy-residual scaling

The bulk energy residual (W-scale, dominated by `rho_r c_r`) is divided by a
fixed numerical scale `energy_scale = rho_r c_r` before it enters the block
system, so the three residual components are of comparable magnitude and the
Newton residual norm is not dominated by the energy equation. This is a
*numerical* row scaling: it does not change the root (a scaled residual driven
to zero is the same solution) and the reported total-energy diagnostic is
computed unscaled.

```rust
pub struct GeneralFlow {
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
  pub fn new(grid: Arc<CartesianGrid>, fluids: GeneralFluids, curves: CharacteristicCurves, porosity: f64, permeability: f64, gravity: f64, boundary: Vec<GeneralBoundaryCondition>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a GENERAL-mode air–water–energy flow problem.

- ```rust
  pub fn set_timestep(self: &mut Self, dt: f64) { /* ... */ }
  ```
  Set the implicit-Euler timestep `dt` (s) for the next assembly. Must be

- ```rust
  pub fn set_previous(self: &mut Self, state: &[f64]) { /* ... */ }
  ```
  Set the previous-time-level state (interleaved `[P_l, S_l, T, …]`, length

- ```rust
  pub fn grid(self: &Self) -> &CartesianGrid { /* ... */ }
  ```
  Read access to the shared underlying grid.

- ```rust
  pub fn fluids(self: &Self) -> &GeneralFluids { /* ... */ }
  ```
  The [`GeneralFluids`] bundle (read-only).

- ```rust
  pub fn total_water_mass(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Total **water** mass in the domain (kg) for an interleaved state `x`:

- ```rust
  pub fn total_gas_mass(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Total **gas** mass in the domain (kg) for an interleaved state `x`:

- ```rust
  pub fn total_energy(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Total **bulk energy** in the domain (J) relative to the `T_ref` datum, for

- ```rust
  pub fn capillary_pressure(self: &Self, s_l: f64) -> f64 { /* ... */ }
  ```
  Capillary pressure `P_c(S_l)` (Pa), by numerically inverting the retention

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **BlockNonlinearSystem**
  - ```rust
    fn n_cells(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn dof_per_cell(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn ldu_addressing(self: &Self) -> (Vec<usize>, Vec<usize>) { /* ... */ }
    ```

  - ```rust
    fn assemble_residual(self: &mut Self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> { /* ... */ }
    ```

  - ```rust
    fn assemble_jacobian(self: &mut Self, x: &[f64], jac: &mut BlockLduMatrix) -> Result<(), PflotranError> { /* ... */ }
    ```
    Assemble the `3x3`-block Jacobian **numerically** by local finite

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GeneralSimulation`

A transient GENERAL-mode air–water–energy simulation driven by the block
Newton solver.

Holds the [`GeneralFlow`] problem, a [`BlockNewtonSolver`], the evolving
interleaved state (`[P_l, S_l, T]` per cell, length `3 * n_cells`), and the
current time. Each [`step`](Self::step) sets the timestep and previous state
on the problem, then solves the coupled `nb = 3` block nonlinear system.

```rust
pub struct GeneralSimulation {
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
  pub fn new(problem: GeneralFlow, initial: Vec<f64>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Assemble a simulation from a problem and a full interleaved initial state

- ```rust
  pub fn step(self: &mut Self, dt: f64) -> Result<BlockNewtonReport, PflotranError> { /* ... */ }
  ```
  Advance one backward-Euler step of size `dt` (s), returning the block

- ```rust
  pub fn time(self: &Self) -> f64 { /* ... */ }
  ```
  Current simulation time (s).

- ```rust
  pub fn liquid_pressure(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Current liquid-pressure field `P_l` (Pa), de-interleaved, length `n_cells`.

- ```rust
  pub fn liquid_saturation(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Current liquid-saturation field `S_l` (dimensionless), de-interleaved,

- ```rust
  pub fn temperature(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Current temperature field `T` (K), de-interleaved, length `n_cells`.

- ```rust
  pub fn problem(self: &Self) -> &GeneralFlow { /* ... */ }
  ```
  The underlying problem (read-only), e.g. for grid access or the

- ```rust
  pub fn state(self: &Self) -> &[f64] { /* ... */ }
  ```
  The current interleaved state vector `[P_l, S_l, T]` per cell (length

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Constants and Statics

#### Constant `STANDARD_GRAVITY`

Standard gravitational acceleration (m/s^2) — the conventional default for a
GENERAL-mode run (gravity acting in `-z`). Matches
[`crate::multiphase::STANDARD_GRAVITY`].

```rust
pub const STANDARD_GRAVITY: f64 = 9.806_65;
```

## Module `geochemistry`

Aqueous geochemistry (v1) — bead op-v6s.12.

Equilibrium aqueous **speciation** — the reaction-network core for reactive
transport (GIRT). Given the total concentration of each PRIMARY (component)
species, it finds the free primary and secondary species concentrations that
simultaneously satisfy the law of mass action and per-component mass balance,
by a Newton solve on log-concentration. Concentrations are in mol/L.

See [`ReactionNetwork`] (build + [`ReactionNetwork::speciate`]) and its
result [`Speciation`]. The full physical model, the exact Newton residual and
Jacobian, and the v1 simplifications (ideal activities `gamma = 1`; no mineral
phases, no kinetics, no explicit charge-balance constraint, no water activity)
are documented on the [`network`] module.

**Deferred to follow-up beads:** Debye–Hückel / Davies activity corrections,
mineral precipitation/dissolution with solubility products, kinetic reaction
rates, and coupling to the transport solver. Enum dispatch, no trait objects.

```rust
pub mod geochemistry { /* ... */ }
```

### Modules

## Module `network`

Equilibrium aqueous speciation — the reaction-network core.

This module implements the classic PHREEQC/PFLOTRAN component-based
speciation solve: given the *total* concentration of each PRIMARY (component)
species, find the free (uncomplexed) concentration of each primary such that
every SECONDARY species is in equilibrium (law of mass action) and every
component's mass balance is satisfied.

# Physical model (v1)

A system has `Nc` primary species and `Ns` secondary species. Concentrations
are molar, **mol/L**. Each secondary species `i` is formed from the primaries
by an equilibrium reaction with stoichiometry `nu[i][j]` (moles of primary
`j` consumed to make one mole of secondary `i`) and a base-10 equilibrium
constant `log10_k[i]`:

- **Mass action** (secondary concentration):
  `C_sec[i] = 10^{log10_k[i]} * prod_j C_prim[j]^{nu[i][j]}`.
- **Mass balance** (total of component `j`):
  `T[j] = C_prim[j] + sum_i nu[i][j] * C_sec[i]`.

# Simplifications (flagged for human review)

- **Ideal activities** — activity coefficients `gamma = 1`, so activity
  equals concentration. Debye–Hückel / Davies corrections are **deferred**
  to a follow-up (they multiply each species activity and change the
  mass-action product; the Newton skeleton here is unchanged by adding them).
- **No mineral phases** — precipitation/dissolution and the associated
  solubility-product constraints are not modelled here (later bead).
- **No kinetic reactions** — only instantaneous equilibrium.
- **No explicit charge-balance constraint** — the caller supplies the total
  of every component (including any proton/charge component) directly. The
  solve enforces mass action + mass balance on those totals; it does not add
  an electroneutrality equation. A caller wanting a charge-balanced state
  must choose totals that are themselves charge-consistent (see the
  symmetric weak-acid verification case in the tests).
- **No water/H2O activity** — the solvent is not treated as a component.

# Numerics

The solve is Newton's method in `x_j = ln C_prim[j]`, which keeps every
primary concentration strictly positive for any real `x`. See
[`ReactionNetwork::speciate`] for the exact residual and Jacobian.

```rust
pub mod network { /* ... */ }
```

### Types

#### Struct `ReactionNetwork`

An equilibrium aqueous reaction network: `Nc` primary + `Ns` secondary species.

Concentrations are in mol/L; equilibrium constants are dimensionless (ideal
activities, so activity equals concentration in v1). Secondary species `i`
has a `log10_k` and a stoichiometry row, and its concentration follows the
law of mass action
`C_sec[i] = 10^{log10_k[i]} * prod_j C_prim[j]^{stoich[i][j]}`.

Construct with [`ReactionNetwork::new`] (which validates the shapes) and
solve a state with [`ReactionNetwork::speciate`].

```rust
pub struct ReactionNetwork {
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
  pub fn new(primary: Vec<String>, secondary: Vec<String>, stoich: Vec<Vec<f64>>, log10_k: Vec<f64>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a reaction network from species names, a stoichiometry matrix, and

- ```rust
  pub fn n_primary(self: &Self) -> usize { /* ... */ }
  ```
  Number of primary (component) species, `Nc`.

- ```rust
  pub fn n_secondary(self: &Self) -> usize { /* ... */ }
  ```
  Number of secondary (equilibrium) species, `Ns`.

- ```rust
  pub fn primary_names(self: &Self) -> &[String] { /* ... */ }
  ```
  Names of the primary (component) species, in column order.

- ```rust
  pub fn secondary_names(self: &Self) -> &[String] { /* ... */ }
  ```
  Names of the secondary (equilibrium) species, in row order.

- ```rust
  pub fn speciate(self: &Self, totals: &[f64], initial: Option<&[f64]>) -> Result<Speciation, PflotranError> { /* ... */ }
  ```
  Solve for the free primary concentrations that satisfy mass action and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ReactionNetwork { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Speciation`

Result of an equilibrium speciation solve.

All concentrations are in mol/L and strictly positive. `primary[j]` is the
free (uncomplexed) concentration of primary species `j`; `secondary[i]` is
the equilibrium concentration of secondary species `i`; `iterations` is the
number of Newton iterations taken to reach the tolerance.

```rust
pub struct Speciation {
    pub primary: Vec<f64>,
    pub secondary: Vec<f64>,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `primary` | `Vec<f64>` | Free primary (component) concentrations, mol/L, length `Nc`. |
| `secondary` | `Vec<f64>` | Secondary species concentrations, mol/L, length `Ns`. |
| `iterations` | `usize` | Newton iterations taken to converge. |

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
    fn clone(self: &Self) -> Speciation { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Re-exports

#### Re-export `ReactionNetwork`

```rust
pub use network::ReactionNetwork;
```

#### Re-export `Speciation`

```rust
pub use network::Speciation;
```

## Module `hdf5_io`

HDF5 snapshot I/O for structured-grid solution fields (bead op-v6s.15.11).

PFLOTRAN writes its solution to **HDF5** — the portable, self-describing binary
format visualisation tools (VisIt, ParaView) read via XDMF. This module writes
and reads a structured-grid snapshot — the grid geometry plus one or more named
cell fields at a given time — using the workspace's **pure-Rust `hdf5-pure`**
backend (no system `libhdf5`, no C toolchain), so pflotran stays
Android-buildable.

# On-disk layout (AI-designed, PFLOTRAN-*inspired*)

A snapshot file contains, at the root:

- `grid_dimensions` — `i32[3]` = `[nx, ny, nz]`
- `x_coords` / `y_coords` / `z_coords` — `f64` cell-centre coordinates per axis
- `time` — `f64[1]`, seconds
- one `f64` dataset `field_<k>` per cell field (length `nx*ny*nz`, in the
  grid's x-fastest cell order), with the field's name carried in the root
  string attribute `field_name_<k>` and the count in `n_fields`

This is a clean, round-trippable layout, **not** a byte-for-byte reproduction
of PFLOTRAN's exact HDF5 schema (its `Coordinates` / `Time:` group naming and
the companion `.xmf` descriptor). Reading a real PFLOTRAN output file, and
emitting the matching XDMF, are follow-ups — flagged here and in the crate
docs.

# Scope / human-review flags

Verification-only, untrusted AI draft (workspace `RESPONSIBLE_USE.md`). Native-
endian `f64`/`i32`; structured grids only; single time level per file. Verified
by a write→read round-trip (see the module tests), not against PFLOTRAN gold
files.

```rust
pub mod hdf5_io { /* ... */ }
```

### Types

#### Struct `Hdf5Snapshot`

A single structured-grid solution snapshot: the grid geometry, a time stamp,
and one or more named cell fields.

Field values are stored per cell in the grid's x-fastest order
(`index = i + nx*(j + ny*k)`), matching [`CartesianGrid`]. Every field has
length `nx*ny*nz`.

```rust
pub struct Hdf5Snapshot {
    pub dims: [usize; 3],
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub time: f64,
    pub fields: Vec<(String, Vec<f64>)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dims` | `[usize; 3]` | Grid cell counts `[nx, ny, nz]`. |
| `x` | `Vec<f64>` | Cell-centre x coordinates, length `nx` (metres). |
| `y` | `Vec<f64>` | Cell-centre y coordinates, length `ny` (metres). |
| `z` | `Vec<f64>` | Cell-centre z coordinates, length `nz` (metres). |
| `time` | `f64` | Snapshot time (seconds). |
| `fields` | `Vec<(String, Vec<f64>)>` | Named cell fields, each of length `nx*ny*nz` (e.g. `("Pressure", …)`). |

##### Implementations

###### Methods

- ```rust
  pub fn from_grid_fields(grid: &CartesianGrid, time: f64, fields: Vec<(String, Vec<f64>)>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Assemble a snapshot from a [`CartesianGrid`], a time, and named cell fields.

- ```rust
  pub fn to_bytes(self: &Self) -> Result<Vec<u8>, PflotranError> { /* ... */ }
  ```
  Serialise this snapshot to in-memory HDF5 bytes.

- ```rust
  pub fn write<P: AsRef<std::path::Path>>(self: &Self, path: P) -> Result<(), PflotranError> { /* ... */ }
  ```
  Write this snapshot to an HDF5 file at `path`.

- ```rust
  pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Parse a snapshot from in-memory HDF5 bytes.

- ```rust
  pub fn read<P: AsRef<std::path::Path>>(path: P) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Read a snapshot from an HDF5 file at `path`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Hdf5Snapshot { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Hdf5Snapshot) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-park-fork-pflotran/src/lib.rs:110:11: 110:32 (#0) }, crates/outram-park-fork-pflotran/src/lib.rs:110:10: 110:33 (#0))])]")`

Optional wgpu GPU acceleration — compiled only off Android (the workspace GPU
rule keeps GPU deps out of the Android library build); CPU is the trusted path.
Optional **wgpu GPU acceleration** for batched property evaluation (op-v6s.14).

This is a *demonstrator* GPU addon following the workspace GPU rules and the
`outram-blender` precedent:

1. **Android-gated.** The whole module is compiled only under
   `cfg(not(target_os = "android"))` (wired in `lib.rs`); the Android build
   never sees `wgpu`, keeping the library headless-buildable there.
2. **CPU is the trusted path; GPU is acceleration only.** The `f64` CPU
   reference ([`van_genuchten_se_cpu`]) is authoritative. The GPU kernel runs
   in `f32` and is an *approximation* — [`van_genuchten_se_best_effort`]
   probes for a device and silently falls back to the CPU when there is no
   adapter (headless CI, no `/dev/dri`) or the submit fails.

The kernel evaluates the van Genuchten effective saturation
`Se(p_c) = (1 + (alpha p_c)^n)^{-m}` (`Se = 1` for `p_c <= 0`) over an array of
capillary pressures — a pure element-wise map, the shape best suited to a GPU
compute dispatch. Wiring this into the solver's per-cell property evaluation
is a follow-up; the CPU (rayon) path remains the default trusted route.

> **GPU path unverified in this environment.** Developed where no GPU adapter
> was available, so only the CPU fallback was exercised here. The GPU dispatch
> must be validated on a GPU-equipped host before it is trusted (bead op-v6s.14).

```rust
pub mod gpu { /* ... */ }
```

### Types

#### Struct `GpuContext`

A ready wgpu device + queue for headless compute.

```rust
pub struct GpuContext {
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `GpuError`

A recoverable GPU failure — the caller should fall back to the CPU path.

```rust
pub enum GpuError {
    Poll(String),
    Map(String),
    MapCallbackMissing,
}
```

##### Variants

###### `Poll`

The device could not be polled to completion (device lost).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Map`

The readback buffer could not be mapped.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `MapCallbackMissing`

The map callback never fired.

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
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `probe`

Probe for a GPU adapter and open a device. Returns `None` when there is no
usable adapter — a **normal, expected** outcome on headless CI / no-GPU hosts,
not an error; the caller then uses the CPU path.

```rust
pub fn probe() -> Option<GpuContext> { /* ... */ }
```

#### Function `van_genuchten_se_cpu`

The **trusted** `f64` CPU reference: van Genuchten effective saturation over a
batch of capillary pressures (Pa). `Se = 1` for `p_c <= 0`; result clamped to
`[0, 1]`.

```rust
pub fn van_genuchten_se_cpu(alpha: f64, n: f64, m: f64, pc: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `try_van_genuchten_se_gpu`

Evaluate van Genuchten `Se(pc)` on the GPU (`f32`). Fallible — the caller
should fall back to [`van_genuchten_se_cpu`] on `Err`. An empty input returns
an empty `Vec` without touching the GPU.

```rust
pub fn try_van_genuchten_se_gpu(ctx: &GpuContext, alpha: f64, n: f64, m: f64, pc: &[f64]) -> Result<Vec<f64>, GpuError> { /* ... */ }
```

#### Function `van_genuchten_se_best_effort`

Evaluate van Genuchten `Se(pc)` using the GPU when available, otherwise the
CPU. Never fails: falls back to [`van_genuchten_se_cpu`] when there is no GPU
adapter or the GPU submit errors. GPU results are `f32`-precision.

```rust
pub fn van_genuchten_se_best_effort(alpha: f64, n: f64, m: f64, pc: &[f64]) -> Vec<f64> { /* ... */ }
```

## Module `grid`

Structured Cartesian finite-volume grid (v1) — bead op-v6s.5.

A logically-rectangular `nx * ny * nz` Cartesian mesh with the geometry a
two-point flux approximation (TPFA) needs: cell volumes and centroids,
internal cell-to-cell [`Connection`]s (shared face area, centroid distance,
and the purely geometric transmissibility `area / distance`), and exterior
[`BoundaryFace`]s for applying Dirichlet/Neumann boundary conditions.

# Cell ordering (x-fastest)

Cells are numbered with the x index varying fastest, then y, then z:

```text
index = i + nx * (j + ny * k)
```

[`CartesianGrid::cell_index`] and [`CartesianGrid::cell_ijk`] convert between
the linear index and the `(i, j, k)` logical triple in both directions.

# Connection ordering (matches the LDU addressing)

[`CartesianGrid::connections`] and [`CartesianGrid::ldu_addressing`] share one
canonical face order: cells are visited in linear order, and for each cell its
`+x`, `+y`, then `+z` internal faces are emitted (when the neighbour exists).
Face `f` in the LDU addressing therefore corresponds exactly to
`connections()[f]`, so a caller can assemble an
`outram_foam_basic_lib` `LduMatrix` and index its off-diagonal coefficients by
the same `f`.

Every internal connection has `owner < neighbour`: the `+x`/`+y`/`+z`
neighbour always has the larger linear index, so the lower-index cell is the
owner by construction.

# Units

All lengths are metres (m), all areas m^2, all volumes m^3. The geometric
transmissibility has units of metres (m) — it is `area / distance`, and must
be multiplied by a mobility (`k_r / mu`) and an intrinsic permeability (m^2)
by the physics layer to obtain a true transmissibility.

# Scope

Structured Cartesian only; unstructured grids are deferred to a later bead.
Grid spacings are stored per axis, so both uniform
([`CartesianGrid::uniform`]) and rectilinear
([`CartesianGrid::rectilinear`]) meshes are supported by the same type.

```rust
pub mod grid { /* ... */ }
```

### Types

#### Enum `Axis`

Cartesian axis. Selects which permeability-tensor component (and which
direction's spacing) a connection or boundary face is aligned with.

```rust
pub enum Axis {
    X,
    Y,
    Z,
}
```

##### Variants

###### `X`

The x axis (index `i`, varies fastest in the cell ordering).

###### `Y`

The y axis (index `j`).

###### `Z`

The z axis (index `k`, varies slowest).

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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `BoundaryLocation`

One of the six faces of the logical box, naming where a boundary condition is
applied. `Min`/`Max` refer to the low and high ends of each axis
(`XMin` is the `i == 0` face, `XMax` the `i == nx - 1` face, and so on).

```rust
pub enum BoundaryLocation {
    XMin,
    XMax,
    YMin,
    YMax,
    ZMin,
    ZMax,
}
```

##### Variants

###### `XMin`

The `i == 0` exterior face (low-x end).

###### `XMax`

The `i == nx - 1` exterior face (high-x end).

###### `YMin`

The `j == 0` exterior face (low-y end).

###### `YMax`

The `j == ny - 1` exterior face (high-y end).

###### `ZMin`

The `k == 0` exterior face (low-z end).

###### `ZMax`

The `k == nz - 1` exterior face (high-z end).

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
    fn clone(self: &Self) -> BoundaryLocation { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BoundaryLocation) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Connection`

An internal connection between two adjacent cells (`owner < neighbour`),
carrying the geometry a two-point flux approximation needs.

The face is shared by exactly the two cells; `area` is its area (m^2),
`distance` is the straight-line owner-centroid to neighbour-centroid distance
(m), and `geometric_transmissibility` is `area / distance` (m). Multiply the
latter by phase mobility (`k_r / mu`) and intrinsic permeability (m^2) in the
physics layer to obtain a flux coefficient.

```rust
pub struct Connection {
    pub owner: usize,
    pub neighbour: usize,
    pub area: f64,
    pub distance: f64,
    pub geometric_transmissibility: f64,
    pub axis: Axis,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `owner` | `usize` | Linear index of the owner cell (the one with the smaller linear index). |
| `neighbour` | `usize` | Linear index of the neighbour cell (the larger linear index). |
| `area` | `f64` | Shared face area, m^2. Product of the two cell widths on the axes<br>orthogonal to [`Connection::axis`]. |
| `distance` | `f64` | Owner-centroid to neighbour-centroid distance, m. Equals half the owner's<br>width plus half the neighbour's width along [`Connection::axis`]. |
| `geometric_transmissibility` | `f64` | Purely geometric transmissibility, `area / distance`, in metres (m).<br>Mobility- and permeability-free; scale it in the physics layer. |
| `axis` | `Axis` | Axis the connection is normal to (the flux direction). |

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
    fn clone(self: &Self) -> Connection { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Connection) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BoundaryFace`

A boundary face of a single cell, for Dirichlet or Neumann boundary
conditions on the domain exterior.

`distance` is the cell-centroid to boundary-face distance (m) — half the
cell's width along [`BoundaryFace::axis`] — which a Dirichlet condition uses
as the one-sided gradient length.

```rust
pub struct BoundaryFace {
    pub cell: usize,
    pub location: BoundaryLocation,
    pub area: f64,
    pub distance: f64,
    pub axis: Axis,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell` | `usize` | Linear index of the cell owning this exterior face. |
| `location` | `BoundaryLocation` | Which of the six box faces this is. |
| `area` | `f64` | Face area, m^2. Product of the two cell widths on the axes orthogonal to<br>[`BoundaryFace::axis`]. |
| `distance` | `f64` | Cell-centroid to boundary-face distance, m — half the cell width along<br>[`BoundaryFace::axis`]. |
| `axis` | `Axis` | Axis the boundary face is normal to. |

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
    fn clone(self: &Self) -> BoundaryFace { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BoundaryFace) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `CartesianGrid`

A logically-rectangular `nx * ny * nz` Cartesian finite-volume grid.

Cell ordering is x-fastest: `index = i + nx * (j + ny * k)`. Per-axis
spacings (metres) are stored, so a single instance can be uniform or
rectilinear. Cell centroids, internal [`Connection`]s, and exterior
[`BoundaryFace`]s are precomputed at construction and returned by reference.

```rust
pub struct CartesianGrid {
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
  pub fn uniform(nx: usize, ny: usize, nz: usize, dx: GeoLength, dy: GeoLength, dz: GeoLength) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a uniform grid from per-axis cell counts and uniform spacings

- ```rust
  pub fn rectilinear(dx: Vec<f64>, dy: Vec<f64>, dz: Vec<f64>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a rectilinear grid from per-axis spacing arrays (metres).

- ```rust
  pub fn dims(self: &Self) -> (usize, usize, usize) { /* ... */ }
  ```
  Cell counts per axis as `(nx, ny, nz)`.

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Total number of cells, `nx * ny * nz`.

- ```rust
  pub fn cell_index(self: &Self, i: usize, j: usize, k: usize) -> usize { /* ... */ }
  ```
  Linear index of cell `(i, j, k)` using x-fastest ordering

- ```rust
  pub fn cell_ijk(self: &Self, cell: usize) -> (usize, usize, usize) { /* ... */ }
  ```
  Logical `(i, j, k)` triple of a linear cell index (inverse of

- ```rust
  pub fn cell_volume(self: &Self, cell: usize) -> f64 { /* ... */ }
  ```
  Volume of a cell, m^3 — `dx[i] * dy[j] * dz[k]`.

- ```rust
  pub fn cell_center(self: &Self, cell: usize) -> [f64; 3] { /* ... */ }
  ```
  Centroid of a cell as `[x, y, z]` in metres.

- ```rust
  pub fn connections(self: &Self) -> &[Connection] { /* ... */ }
  ```
  All internal cell-to-cell connections, in the canonical face order (see

- ```rust
  pub fn boundary_faces(self: &Self) -> &[BoundaryFace] { /* ... */ }
  ```
  All exterior boundary faces, one per cell face lying on the domain

- ```rust
  pub fn ldu_addressing(self: &Self) -> (Vec<usize>, Vec<usize>) { /* ... */ }
  ```
  The `(owner, neighbour)` index arrays for constructing an

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> CartesianGrid { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `io`

Input-deck I/O and output writers (v1) — bead op-v6s.6.

A minimal **card-based ASCII input-deck subset** (PFLOTRAN-*style* blocks)
plus CSV / legacy-VTK output writers for the v1 RICHARDS vertical slice.
HDF5 is deferred behind a future feature gate.

# AI-designed subset — needs human review

The deck grammar here is a small subset **invented for this crate**. It is
inspired by PFLOTRAN's card/block input style but is **not** the real
PFLOTRAN input-deck syntax and is not compatible with it. Per the workspace
`RESPONSIBLE_USE.md` rule, this is untrusted AI-generated draft material: a
human must review it against genuine PFLOTRAN input decks before any
fidelity claim is made. The full grammar is documented on [`parse_deck`].

# Decoupling

The parser returns plain, self-contained [`spec`] structs built from
primitives (`usize` / `f64` / `String` / `Vec`). It does **not** depend on
the [`crate::grid`], [`crate::properties`], or [`crate::solver`] modules —
the RICHARDS driver maps a parsed [`InputDeck`] onto those. The writers take
primitive slices, not foreign field types, so this module is fully testable
without a filesystem or the physics side.

# Units

Lengths in metres, pressures in pascals (Pa), times in seconds (s),
permeability in m^2, `alpha` in 1/Pa, Darcy fluxes in m/s.

```rust
pub mod io { /* ... */ }
```

### Re-exports

#### Re-export `parse_deck`

```rust
pub use parse::parse_deck;
```

#### Re-export `BoundaryConditionSpec`

```rust
pub use spec::BoundaryConditionSpec;
```

#### Re-export `BoundaryKindSpec`

```rust
pub use spec::BoundaryKindSpec;
```

#### Re-export `BoundaryLocationSpec`

```rust
pub use spec::BoundaryLocationSpec;
```

#### Re-export `CurveModel`

```rust
pub use spec::CurveModel;
```

#### Re-export `CurveSpec`

```rust
pub use spec::CurveSpec;
```

#### Re-export `GridSpec`

```rust
pub use spec::GridSpec;
```

#### Re-export `InputDeck`

```rust
pub use spec::InputDeck;
```

#### Re-export `MaterialSpec`

```rust
pub use spec::MaterialSpec;
```

#### Re-export `TimeSpec`

```rust
pub use spec::TimeSpec;
```

#### Re-export `write_results_csv`

```rust
pub use writers::write_results_csv;
```

#### Re-export `write_vtk_structured`

```rust
pub use writers::write_vtk_structured;
```

## Module `kinetics`

Kinetic mineral precipitation / dissolution (v1) — bead op-v6s.12.

Extends the equilibrium [`crate::geochemistry`] speciation core toward
reactive-transport geochemistry (GIRT) by adding *kinetic* mineral phases:
solids that dissolve into, or precipitate out of, the aqueous solution at a
finite rate governed by **transition-state theory (TST)** rather than
instantaneous equilibrium. The aqueous complexation itself is still solved
at equilibrium (via [`ReactionNetwork::speciate`]); only the mineral–water
exchange is rate-limited.

# Physical model

A kinetic mineral `M_i` exchanges with the aqueous primary (component)
species by the dissolution reaction

```text
M_i  <->  sum_j nu[i][j] * primary_j
```

where `nu[i][j]` (`stoich[j]` on [`KineticMineral`]) is the number of moles
of primary component `j` **released into solution per mole of mineral
dissolved**, and the mineral has a base-10 solubility product
`log10_ksp[i]`.

Given the free primary concentrations `C_prim` obtained from a speciation
solve (ideal activities inherited from [`crate::geochemistry`], so activity
equals concentration in mol/L), define:

- **Ion activity product** `Q_i = prod_j C_prim[j]^{nu[i][j]}`.
- **Solubility product** `Ksp_i = 10^{log10_ksp[i]}`.
- **Saturation ratio** `Omega_i = Q_i / Ksp_i`.
- **Saturation index** `SI_i = log10(Omega_i) = log10(Q_i) - log10_ksp[i]`.

The transition-state-theory rate (moles of mineral reacting per unit time)
is

```text
R_i = k_i * A_i * (1 - Omega_i)
```

with `k_i` the rate constant (mol/(m^2 s)) and `A_i` the reactive surface
area (m^2). The sign convention:

- `Omega_i < 1` (undersaturated) => `R_i > 0` => **dissolution** (mineral
  moles fall, aqueous totals rise).
- `Omega_i > 1` (supersaturated) => `R_i < 0` => **precipitation** (mineral
  moles rise, aqueous totals fall).
- `Omega_i = 1` (saturated) => `R_i = 0` (equilibrium; the rate law relaxes
  the system toward `Omega -> 1`).

The effect on the state is

```text
d(total_j)/dt = sum_i nu[i][j] * R_i        (aqueous component totals)
d(m_i)/dt     = -R_i                        (mineral moles)
```

which conserves mass exactly: every mole released into the totals is a mole
removed from a mineral, weighted by `nu`.

## No-mineral clamp (documented behaviour)

A mineral that is **absent** (`m_i <= 0`) cannot dissolve — there is no
solid to consume. So whenever `m_i <= 0` and the raw TST rate is a
*dissolution* rate (`R_i > 0`, i.e. `Omega_i < 1`), the rate is clamped to
`0`. Precipitation from an absent phase (`R_i < 0`, `Omega_i > 1`) is still
permitted, so a supersaturated solution can nucleate a new mineral from
zero. (This is the physically-correct reading of the clamp; a bare "cannot
dissolve what isn't there" rule, consistent with the `no_mineral_clamp`
test.) No secondary-nucleation kinetics or induction time is modelled —
precipitation begins immediately once `Omega > 1`.

# Numerics — coupled ODE over a timestep

The coupled system for the state vector
`y = [total_0 .. total_{Nc-1}, m_0 .. m_{Nm-1}]` is integrated over a
macro-timestep `dt` with the foam-basic-lib **RKF45** adaptive explicit
Runge-Kutta-Fehlberg 4(5) solver ([`outram_foam_basic_lib::ode::Rkf45`]).

**Why RKF45 (explicit) and not Rosenbrock23 (stiff):** the right-hand side
is evaluated by running a full Newton **speciation** each call, so the
Jacobian `d(dy/dt)/dy` that a Rosenbrock/W-method needs would have to
differentiate through the implicit speciation solve — no closed form is
available, and a finite-difference Jacobian would cost `Nc+1` speciations
per Jacobian. RKF45 needs no Jacobian and its adaptive controller shrinks
the step automatically when the kinetics stiffen near equilibrium. For
very fast rate constants (relaxation time `<< dt`) a stiff integrator with
a hand-coded or frozen-activity Jacobian would be more efficient; that is a
documented follow-up, not needed for the moderate rates modelled here.

**Live vs frozen activities:** this implementation **re-speciates on every
RHS evaluation** (live activities), warm-started from the previous free
primary vector. This is the accurate choice — the ion activity product
`Q_i` tracks the true aqueous state as it evolves — at the cost of one
Newton speciation per RHS call. A cheaper *frozen-activity* alternative
(speciate once per macro-step and hold `C_prim` fixed inside the ODE) is a
valid simplification but is **not** used here.

# Simplifications (flagged for human review)

- **Ideal activities** (`gamma = 1`) inherited from
  [`crate::geochemistry`]; no Debye–Hückel / Davies correction.
- **Single lumped surface-area model** — `A_i` is a constant per mineral; no
  evolving grain-size / specific-surface-area or reactive-site model, and no
  dependence of `A_i` on the current mineral amount.
- **No nucleation kinetics** — precipitation is governed by the same TST law
  with no induction time or critical supersaturation.
- **No pH- or catalysis-dependent rate laws, no far-from-equilibrium
  (`|1 - Omega|^n`) exponents** — the affinity term is the linear
  `(1 - Omega)`.

Per the workspace `RESPONSIBLE_USE.md`, this is untrusted AI-generated draft
material until a human reviews it; no validation against published
reactive-transport benchmarks has been performed.

```rust
pub mod kinetics { /* ... */ }
```

### Types

#### Struct `KineticMineral`

A kinetic mineral: its dissolution stoichiometry into the aqueous primary
components, its solubility product, its rate constant, and its reactive
surface area.

The dissolution reaction is `M <-> sum_j stoich[j] * primary_j`, so
`stoich[j]` is the moles of primary component `j` released per mole of
mineral dissolved (length `Nc`, the number of primary species in the
associated [`ReactionNetwork`]). Values may be non-integer; a negative entry
would mean a component is *consumed* on dissolution, which is unusual but
not forbidden.

```rust
pub struct KineticMineral {
    pub name: String,
    pub stoich: Vec<f64>,
    pub log10_ksp: f64,
    pub rate_constant: f64,
    pub surface_area: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-facing mineral label (e.g. `"Calcite"`). |
| `stoich` | `Vec<f64>` | Dissolution stoichiometry, length `Nc`: moles of primary `j` released<br>per mole of mineral dissolved. |
| `log10_ksp` | `f64` | Base-10 solubility product, `log10(Ksp)` (dimensionless, ideal<br>activities). `Ksp = 10^{log10_ksp}`. |
| `rate_constant` | `f64` | Rate constant `k`, mol/(m^2 s). Must be finite and non-negative. |
| `surface_area` | `f64` | Reactive surface area `A`, m^2. Must be finite and non-negative. |

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
    fn clone(self: &Self) -> KineticMineral { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `KineticSystem`

A set of kinetic minerals reacting with an aqueous [`ReactionNetwork`].

Build with [`KineticSystem::new`] (which validates every mineral's
stoichiometry length against the network's primary-species count) and drive
a state forward in time with [`KineticSystem::react`]. The saturation index
of any mineral at a given free-primary state is available from
[`KineticSystem::saturation_index`].

The network and mineral set are shared via [`Arc`] (workspace rule: no
lifetime parameters, no `Box`), so the per-`react` ODE system can hold cheap
handles to them without borrowing `self`.

```rust
pub struct KineticSystem {
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
  pub fn new(network: ReactionNetwork, minerals: Vec<KineticMineral>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a kinetic system from an aqueous reaction network and a set of

- ```rust
  pub fn n_minerals(self: &Self) -> usize { /* ... */ }
  ```
  The number of kinetic minerals, `Nm`.

- ```rust
  pub fn n_primary(self: &Self) -> usize { /* ... */ }
  ```
  The number of aqueous primary components, `Nc`.

- ```rust
  pub fn network(self: &Self) -> &ReactionNetwork { /* ... */ }
  ```
  The underlying equilibrium aqueous reaction network.

- ```rust
  pub fn minerals(self: &Self) -> &[KineticMineral] { /* ... */ }
  ```
  The kinetic mineral phases, in index order.

- ```rust
  pub fn saturation_index(self: &Self, mineral: usize, primary: &[f64]) -> f64 { /* ... */ }
  ```
  Saturation index `SI = log10(Q / Ksp)` of mineral `mineral` at the given

- ```rust
  pub fn react(self: &Self, totals: &mut Vec<f64>, mineral_moles: &mut Vec<f64>, dt: f64) -> Result<usize, PflotranError> { /* ... */ }
  ```
  Integrate the aqueous component totals and mineral amounts forward by

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> KineticSystem { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `microbial`

Microbially-mediated (biodegradation) kinetic reactions — bead op-v6s.15.4.

Adds **microbial** kinetic reactions to the reactive-transport stack: an
electron **donor** is oxidised using an electron **acceptor**, the reaction
catalysed by living **biomass** `B`, at a rate governed by a **Monod**
(dual-Monod) saturation law rather than by mass-action equilibrium. This is
the rate model used by PFLOTRAN's `MICROBIAL_REACTION` block and the
standard textbook description of substrate-limited microbial growth.

# Physical model

## Monod / dual-Monod rate law

A single microbial reaction `r` degrades a donor species `d` with an
acceptor species `a`, mediated by a biomass pool `B_r`. Its instantaneous
rate (mol of reaction turnover per litre per second) is

```text
R_r = k_max,r * B_r * [ S_d / (K_d + S_d) ] * [ S_a / (K_a + S_a) ]
                    * prod_j [ I_j / (I_j + C_j) ]
```

where

- `k_max,r` is the maximum specific reaction rate, `1/s` (per unit biomass);
- `B_r` is the biomass concentration of the pool driving reaction `r`
  (mol/L, or any biomass unit consistent with `k_max` and `yield`);
- `S_d = C[donor]` and `S_a = C[acceptor]` are the donor / acceptor aqueous
  concentrations (mol/L);
- `K_d`, `K_a` are the donor / acceptor **half-saturation** constants
  (mol/L) — the substrate concentration at which the corresponding Monod
  factor reaches `1/2`;
- each optional **inhibition** factor `I_j / (I_j + C_j)` (non-competitive /
  Haldane-type inhibition) suppresses the rate as the concentration `C_j` of
  inhibitor species `j` rises above its inhibition constant `I_j` (mol/L).

Each Monod factor `S / (K + S)` runs monotonically from `0` (at `S = 0`) to
`1` (as `S >> K`), so the product is bounded in `[0, 1]` and the rate is
bounded by `k_max * B`. The **dual** structure means the reaction is limited
by whichever substrate is scarce relative to its half-saturation constant.

## Effect on the state

Aqueous species change by the reaction stoichiometry `nu_r[i]` (mol of
species `i` per mol of reaction turnover; consumed species carry `nu < 0`,
produced species `nu > 0`); biomass grows in proportion to turnover and
decays by first-order endogenous respiration:

```text
dC_i/dt = sum_r nu_r[i] * R_r          (aqueous species i)
dB_r/dt = yield_r * R_r - k_decay,r * B_r   (biomass pool r)
```

with `yield_r` the biomass yield (biomass produced per unit reaction
turnover) and `k_decay,r` the first-order biomass decay constant (`1/s`).
When substrate is ample the growth term `yield * R` dominates and biomass
rises; when substrate is exhausted `R -> 0` and biomass decays exponentially
as `B_r(t) = B_r(0) * exp(-k_decay,r * t)`.

The aqueous-species update is stoichiometrically exact: for a single
reaction, `Delta C_i / nu_i` is the same integrated turnover for every
species `i`, so elemental balances implied by `nu` are conserved to solver
precision.

# Numerics — coupled ODE over a timestep

[`MicrobialSystem::react`] integrates the coupled state
`y = [C_0 .. C_{Ns-1}, B_0 .. B_{Nr-1}]` over a macro-timestep `dt` with the
foam-basic-lib **RKF45** adaptive explicit Runge-Kutta-Fehlberg 4(5) solver
([`outram_foam_basic_lib::ode::Rkf45`]), the same integrator the mineral
[`crate::kinetics`] module uses. The right-hand side is closed-form
arithmetic (no inner equilibrium solve), so an explicit adaptive method is
appropriate; its controller shrinks the step automatically when fast growth
or decay stiffens the system. Concentrations and biomass are floored to
zero inside each Monod / inhibition factor so that a transient adaptive-stage
extrapolation slightly below zero cannot produce a spurious negative or
non-finite rate.

# Simplifications (flagged for human review)

- **One biomass pool per reaction.** Each reaction has its own scalar
  biomass concentration; there is no shared microbial community, no
  competition between reactions for a common biomass, and no mobile-vs-
  attached partitioning.
- **No thermodynamic factor.** The rate has no `(1 - Q/K)` /
  `(1 - exp(Delta G / RT))` far-from-equilibrium term — turnover does not
  stop when the reaction approaches thermodynamic equilibrium. Only substrate
  availability and inhibition limit the rate.
- **No pH, temperature, or nutrient dependence** beyond the explicit donor,
  acceptor, and inhibitor Monod factors listed on the reaction.
- **Ideal (concentration = activity)** aqueous phase, matching the rest of
  this crate's geochemistry.

Per the workspace `RESPONSIBLE_USE.md`, this is untrusted AI-generated draft
material until a human reviews it; no validation against a published
biodegradation benchmark has been performed.

# Provenance

- J. Monod, "The growth of bacterial cultures", *Annual Review of
  Microbiology* **3**, 371–394 (1949) — the single-substrate saturation
  rate law `R = R_max * S / (K + S)`.
- The dual-Monod (multiplicative donor × acceptor) extension and the
  non-competitive inhibition factor `I / (I + C)` are the standard forms used
  in subsurface biodegradation reactive-transport codes.
- PFLOTRAN `MICROBIAL_REACTION` (Monod / inhibition / biomass) — the
  reference implementation this module mirrors. See P. C. Lichtner et al.,
  *PFLOTRAN User Manual*.

```rust
pub mod microbial { /* ... */ }
```

### Types

#### Struct `MonodReaction`

A single Monod (dual-Monod) microbial reaction with optional inhibition and
a biomass catalyst.

The reaction degrades the electron [`donor`](Self::donor) species using the
electron [`acceptor`](Self::acceptor) species, at the rate

```text
R = k_max * B * [S_d/(K_d + S_d)] * [S_a/(K_a + S_a)] * prod_j I_j/(I_j + C_j)
```

(see the [module header](crate::microbial) for the full physics and units).
Species indices refer to positions in the host [`MicrobialSystem`]'s aqueous
species vector.

```rust
pub struct MonodReaction {
    pub name: String,
    pub donor: usize,
    pub acceptor: usize,
    pub k_max: f64,
    pub half_sat_donor: f64,
    pub half_sat_acceptor: f64,
    pub stoichiometry: Vec<f64>,
    pub inhibition: Vec<(usize, f64)>,
    pub biomass_yield: f64,
    pub biomass_decay: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-facing reaction label (e.g. `"aerobic acetate oxidation"`). |
| `donor` | `usize` | Index of the electron-donor species, `0 <= donor < n_species`. |
| `acceptor` | `usize` | Index of the electron-acceptor species, `0 <= acceptor < n_species`. |
| `k_max` | `f64` | Maximum specific reaction rate `k_max`, `1/s` (turnover per unit<br>biomass). Must be finite and non-negative. |
| `half_sat_donor` | `f64` | Donor half-saturation constant `K_d`, mol/L. Must be finite and strictly<br>positive (it appears in the denominator `K_d + S_d`). |
| `half_sat_acceptor` | `f64` | Acceptor half-saturation constant `K_a`, mol/L. Must be finite and<br>strictly positive. |
| `stoichiometry` | `Vec<f64>` | Reaction stoichiometry `nu_i` per aqueous species (length<br>`n_species`): mol of species `i` per mol of reaction turnover. Consumed<br>species carry `nu_i < 0`, produced species `nu_i > 0`. Every entry must<br>be finite. |
| `inhibition` | `Vec<(usize, f64)>` | Optional non-competitive inhibition terms `(species index j,<br>inhibition constant I_j)`. Each contributes a factor<br>`I_j / (I_j + C_j)` to the rate, suppressing it as inhibitor `j`<br>accumulates. Each `I_j` must be finite and strictly positive; each index<br>must be a valid species. |
| `biomass_yield` | `f64` | Biomass yield: biomass produced per unit reaction turnover (`dB/dt`<br>growth term is `yield * R`). Must be finite. Typically non-negative;<br>a negative yield is unusual but not forbidden. |
| `biomass_decay` | `f64` | First-order biomass decay constant `k_decay`, `1/s` (endogenous<br>respiration). Must be finite and non-negative. |

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
    fn clone(self: &Self) -> MonodReaction { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `MicrobialSystem`

A microbial reaction system: a set of [`MonodReaction`]s acting on `Ns`
aqueous species plus one biomass pool per reaction.

Build with [`MicrobialSystem::new`] (which validates every reaction's
indices, stoichiometry length, and parameter positivity) and advance a
state forward with [`MicrobialSystem::react`]. The instantaneous rate of any
reaction at a given state is available from [`MicrobialSystem::rate`].

The reaction set is shared via [`Arc`] (workspace rule: no lifetime
parameters, no `Box`) so the per-`react` ODE system can hold a cheap handle
without borrowing `self`.

```rust
pub struct MicrobialSystem {
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
  pub fn new(n_species: usize, reactions: Vec<MonodReaction>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a microbial system from an aqueous species count and a set of

- ```rust
  pub fn n_species(self: &Self) -> usize { /* ... */ }
  ```
  The number of aqueous species `Ns`.

- ```rust
  pub fn n_reactions(self: &Self) -> usize { /* ... */ }
  ```
  The number of microbial reactions `Nr` (= the number of biomass pools).

- ```rust
  pub fn reactions(self: &Self) -> &[MonodReaction] { /* ... */ }
  ```
  The microbial reactions, in index order.

- ```rust
  pub fn rate(self: &Self, reaction: usize, concentrations: &[f64], biomass: &[f64]) -> f64 { /* ... */ }
  ```
  Instantaneous rate `R_r` (mol/(L·s)) of reaction `reaction` at the given

- ```rust
  pub fn react(self: &Self, concentrations: &mut [f64], biomass: &mut [f64], dt: f64) -> Result<usize, PflotranError> { /* ... */ }
  ```
  Integrate the aqueous concentrations and biomass pools forward by `dt`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MicrobialSystem { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `multiphase`

Two-phase immiscible (air–water) isothermal flow — bead op-v6s.13, the first
cut toward the GENERAL multiphase mode.

This module is the **coupled, two-unknowns-per-cell analogue** of the scalar
RICHARDS mode ([`crate::flow`]). Where RICHARDS carries one unknown per cell
(liquid pressure) and is solved on the scalar Newton–Krylov layer, two-phase
flow carries **two** primary unknowns per cell — liquid pressure `P_l` (Pa)
and liquid saturation `S_l` (dimensionless) — and is assembled onto the block
multi-DOF solver [`crate::solver::block`] with `nb = 2` DOF per cell.

# Governing equations

Two mass-conservation equations per cell, discretised by a cell-centred
**two-point flux** finite volume in space and **backward (implicit) Euler**
in time:

$$ \frac{\partial}{\partial t}\left(\phi\, S_l\, \rho_l\right) + \nabla\cdot\left(\rho_l\, \mathbf{q}_l\right) = 0, \qquad \mathbf{q}_l = -\frac{k\, k_{rl}(S_l)}{\mu_l}\left(\nabla P_l - \rho_l\,\mathbf{g}\right) $$

$$ \frac{\partial}{\partial t}\left(\phi\, S_g\, \rho_g\right) + \nabla\cdot\left(\rho_g\, \mathbf{q}_g\right) = 0, \qquad \mathbf{q}_g = -\frac{k\, k_{rg}(S_g)}{\mu_g}\left(\nabla P_g - \rho_g\,\mathbf{g}\right) $$

with the saturation and capillary closures

$$ S_g = 1 - S_l, \qquad P_g = P_l + P_c(S_l), \qquad S_e = \frac{S_l - S_r}{1 - S_r} . $$

Face mobility (`k_{rl}` / `k_{rg}`) is **upstream-weighted** per phase, as in
RICHARDS — the physically correct choice for advective transport and the
reason the block Jacobian is nonsymmetric.

# Constitutive closures

- **Liquid relative permeability** `k_{rl} = relative_permeability(S_e)` from
  the supplied [`CharacteristicCurves`] (van Genuchten / Brooks–Corey /
  Haverkamp).
- **Gas relative permeability** uses a documented closed-form **quadratic
  Corey** non-wetting curve with **zero residual gas saturation**:

  $$ k_{rg}(S_e) = (1 - S_e)^2 . $$

  This is `1` at `S_e = 0` (fully gas), `0` at `S_e = 1` (fully liquid), and
  monotone. It is computed directly from `S_e` and does **not** come from the
  `CharacteristicCurves` (whose relative permeability is the wetting phase's).
- **Capillary pressure** `P_c(S_l)` is obtained by **numerically inverting**
  the retention curve: `P_c` is the pressure at which
  `CharacteristicCurves::effective_saturation(P_c) = S_e`. Because
  `effective_saturation` is monotone non-increasing in `P_c`, the inverse is
  found by exponential bracketing + bisection (see
  [`TwoPhaseFlow::capillary_pressure`]). `S_e >= 1` maps to `P_c = 0`
  (saturated); the target `S_e` is floored at `1e-9` so the suction stays
  finite at the residual limit.

# Simplifications (v1) — flags for human review

- **Constant phase densities and viscosities.** Both liquid and gas are
  treated as incompressible with constant [`TwoPhaseFluids`] properties. Gas
  is therefore **constant density** — ideal-gas / slightly-compressible gas
  is a documented follow-up. This keeps the accumulation term linear in the
  unknowns and the residual well-scaled.
- **Isothermal.** No energy equation; adding temperature as a third unknown
  (the full air–water–energy GENERAL mode) is deferred.
- **Verification-only, not validated.** No comparison to published PFLOTRAN
  two-phase results has been done; the tests here are self-contained physical
  sanity checks (stationarity, phase-mass conservation, bounded imbibition).

# State-vector layout

All state vectors are interleaved, length `2 * n_cells`: cell `c` occupies
`[2c] = P_l` (Pa) and `[2c + 1] = S_l` (dimensionless), matching the block
solver's `nb = 2` convention. Where `S_l` feeds the property closures it is
clamped to `[S_r, 1]` to keep `S_e`, `k_r`, and `P_c` finite; the
accumulation term uses the raw `S_l` so the discrete mass balance stays exact.

# Sign & reference conventions

- `z` is elevation (upward positive); gravity magnitude `g >= 0` acts in `-z`.
- Per-face potential for phase `p`: `Phi_p = P_p + rho_p * g * z`.
- A residual component is a phase mass rate (kg/s); a positive face/boundary
  flux **leaves** the cell. Unspecified boundary faces are **no-flow**.

> **Untrusted AI draft — verification only**, per the workspace
> `RESPONSIBLE_USE.md`. Not for facility operation or any safety-critical use.

```rust
pub mod multiphase { /* ... */ }
```

### Types

#### Struct `TwoPhaseFluids`

A pair of immiscible fluids (wetting liquid + non-wetting gas) with constant
SI properties.

All four fields must be strictly positive and finite. Densities are in
kg/m^3 and viscosities in Pa·s. For v1 the properties are **constant** (the
liquid is incompressible and the gas is treated as constant density); see the
module header for the simplifications this entails.

```rust
pub struct TwoPhaseFluids {
    pub liquid_density: f64,
    pub liquid_viscosity: f64,
    pub gas_density: f64,
    pub gas_viscosity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `liquid_density` | `f64` | Liquid (wetting) mass density `rho_l`, kg/m^3. Strictly positive. |
| `liquid_viscosity` | `f64` | Liquid (wetting) dynamic viscosity `mu_l`, Pa·s. Strictly positive. |
| `gas_density` | `f64` | Gas (non-wetting) mass density `rho_g`, kg/m^3. Strictly positive. |
| `gas_viscosity` | `f64` | Gas (non-wetting) dynamic viscosity `mu_g`, Pa·s. Strictly positive. |

##### Implementations

###### Methods

- ```rust
  pub fn air_water() -> Self { /* ... */ }
  ```
  Representative **air–water at ~20 °C** properties: liquid water

- ```rust
  pub fn new(liquid_density: f64, liquid_viscosity: f64, gas_density: f64, gas_viscosity: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a validated fluid pair.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TwoPhaseFluids { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TwoPhaseFluids) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `TwoPhaseBoundaryKind`

What is prescribed on one exterior boundary location for two-phase flow.

Unspecified faces default to [`TwoPhaseBoundaryKind::NoFlow`].

```rust
pub enum TwoPhaseBoundaryKind {
    Dirichlet {
        liquid_pressure: f64,
        liquid_saturation: f64,
    },
    NoFlow,
}
```

##### Variants

###### `Dirichlet`

Fixed liquid pressure (Pa) and liquid saturation (dimensionless) at the
boundary. The ghost state's gas pressure follows the capillary closure
`P_g = P_l + P_c(S_l)`, so both phases can exchange mass across the face.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `liquid_pressure` | `f64` | Prescribed liquid pressure `P_l` at the boundary, Pa. |
| `liquid_saturation` | `f64` | Prescribed liquid saturation `S_l` at the boundary, dimensionless in<br>`[S_r, 1]`. |

###### `NoFlow`

No-flow (zero normal flux for both phases). The natural default.

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
    fn clone(self: &Self) -> TwoPhaseBoundaryKind { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TwoPhaseBoundaryKind) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TwoPhaseBoundaryCondition`

A two-phase boundary condition applied to one of the six Cartesian faces.

```rust
pub struct TwoPhaseBoundaryCondition {
    pub location: crate::grid::BoundaryLocation,
    pub kind: TwoPhaseBoundaryKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `location` | `crate::grid::BoundaryLocation` | Which exterior face of the logical box this applies to. |
| `kind` | `TwoPhaseBoundaryKind` | What is prescribed there. |

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
    fn clone(self: &Self) -> TwoPhaseBoundaryCondition { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TwoPhaseBoundaryCondition) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TwoPhaseFlow`

The two-phase immiscible flow problem for a single backward-Euler timestep.

Owns the grid, characteristic curves, fluid pair, and homogeneous material
data (isotropic permeability, uniform porosity). Implements
[`BlockNonlinearSystem`] with `dof_per_cell = 2`, so it is driven by the
generic [`BlockNewtonSolver`]. The block Jacobian is assembled **numerically**
— local finite differences of the two residual equations with respect to the
two local unknowns over the two-point stencil — which matches the residual by
construction and side-steps fragile analytic derivatives through the upstream
switch and the capillary-inversion closure.

```rust
pub struct TwoPhaseFlow {
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
  pub fn new(grid: CartesianGrid, curves: CharacteristicCurves, fluids: TwoPhaseFluids, porosity: f64, permeability: f64, gravity: f64, boundary: Vec<TwoPhaseBoundaryCondition>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a two-phase flow problem.

- ```rust
  pub fn set_timestep(self: &mut Self, dt: f64) { /* ... */ }
  ```
  Set the implicit-Euler timestep `dt` (s) for the next assembly. Must be

- ```rust
  pub fn set_previous(self: &mut Self, state: &[f64]) { /* ... */ }
  ```
  Set the previous-time-level state (interleaved `[P_l0, S_l0, P_l1, S_l1,

- ```rust
  pub fn grid(self: &Self) -> &CartesianGrid { /* ... */ }
  ```
  Read access to the underlying grid.

- ```rust
  pub fn total_water_mass(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Total **water** mass in the domain (kg) for an interleaved state `x`:

- ```rust
  pub fn total_gas_mass(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Total **gas** mass in the domain (kg) for an interleaved state `x`:

- ```rust
  pub fn capillary_pressure(self: &Self, s_l: f64) -> f64 { /* ... */ }
  ```
  Capillary pressure `P_c(S_l)` (Pa), obtained by numerically inverting the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **BlockNonlinearSystem**
  - ```rust
    fn n_cells(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn dof_per_cell(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn ldu_addressing(self: &Self) -> (Vec<usize>, Vec<usize>) { /* ... */ }
    ```

  - ```rust
    fn assemble_residual(self: &mut Self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> { /* ... */ }
    ```

  - ```rust
    fn assemble_jacobian(self: &mut Self, x: &[f64], jac: &mut BlockLduMatrix) -> Result<(), PflotranError> { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TwoPhaseSimulation`

A transient two-phase immiscible simulation driven by the block Newton solver.

Holds the [`TwoPhaseFlow`] problem, a [`BlockNewtonSolver`], the evolving
interleaved state (`[P_l, S_l]` per cell, length `2 * n_cells`), and the
current time. Each [`step`](Self::step) sets the timestep and previous state
on the problem, then solves the coupled block nonlinear system.

```rust
pub struct TwoPhaseSimulation {
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
  pub fn new(problem: TwoPhaseFlow, config: BlockNewtonConfig, initial_liquid_pressure: f64, initial_liquid_saturation: f64) -> Self { /* ... */ }
  ```
  Assemble a simulation from a problem, a block-Newton configuration, and a

- ```rust
  pub fn step(self: &mut Self, dt: f64) -> Result<BlockNewtonReport, PflotranError> { /* ... */ }
  ```
  Advance one backward-Euler step of size `dt` (s), returning the block

- ```rust
  pub fn time(self: &Self) -> f64 { /* ... */ }
  ```
  Current simulation time (s).

- ```rust
  pub fn liquid_pressure(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Current liquid-pressure field `P_l` (Pa), de-interleaved, length

- ```rust
  pub fn liquid_saturation(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Current liquid-saturation field `S_l` (dimensionless), de-interleaved,

- ```rust
  pub fn problem(self: &Self) -> &TwoPhaseFlow { /* ... */ }
  ```
  The underlying problem (read-only), e.g. for grid access or the

- ```rust
  pub fn state(self: &Self) -> &[f64] { /* ... */ }
  ```
  The current interleaved state vector `[P_l, S_l]` per cell (length

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Constants and Statics

#### Constant `STANDARD_GRAVITY`

Standard gravitational acceleration (m/s^2) — the conventional default for a
two-phase run (gravity acting in `-z`).

```rust
pub const STANDARD_GRAVITY: f64 = 9.806_65;
```

## Module `pitzer`

Pitzer ion-interaction (virial) activity-coefficient model — bead op-s1h.

Extends the [`crate::activity`] module (Ideal / Debye–Hückel / Davies), which
is only trustworthy to ionic strength `I ~ 0.5 molal`, to the
high-ionic-strength brines encountered in geochemistry and repository
near-field chemistry. The Pitzer specific-ion-interaction model reproduces
measured mean activity and osmotic coefficients up to and beyond `6 molal`
for many electrolytes.

Where the Debye–Hückel family treats all ion–ion interactions through a
single ionic-strength term, Pitzer adds an explicit, empirically-fitted
**virial expansion** in molality: a second-virial pair term (`beta0`,
`beta1`) and a third-virial term (`C_phi`) per binary electrolyte. Those
parameters are what carry the model to high concentration.

## Scope of this implementation

This module implements the **binary (single-salt) 1:1 and 2:1 electrolyte**
case only — one cation, one anion, at **25 degrees C**. It does **not** yet
implement the mixing terms (`theta_ij`, `psi_ijk`) needed for multi-salt
brines, higher-order electrostatic terms, or any temperature dependence.
See the human-review flags below.

## Molality convention

Pitzer's equations are written on a **molality** basis (mol of solute per kg
of solvent water), not molarity. For a salt `M_{nu_M} X_{nu_X}` that fully
dissociates at stoichiometric molality `m`, the ionic strength is

```text
I = 0.5 * ( nu_M * m * z_M^2 + nu_X * m * z_X^2 )      [mol/kg]
```

e.g. `I = m` for a 1:1 salt (NaCl) and `I = 3 m` for a 2:1 salt (CaCl2).

## Equations (25 degrees C)

**Debye–Hückel osmotic function** (the long-range electrostatic term):

```text
f^gamma = -A_phi * [ sqrt(I) / (1 + b*sqrt(I)) + (2/b) * ln(1 + b*sqrt(I)) ]
f^phi   = -A_phi *   sqrt(I) / (1 + b*sqrt(I))
```

with `b = 1.2 kg^0.5 mol^-0.5` (Pitzer's universal constant) and the
Debye–Hückel–Pitzer slope [`A_PHI_25C`] `≈ 0.391 kg^0.5 mol^-0.5` at
25 degrees C in water.

**Second-virial `B` terms** (short-range pair interaction), with
`x = alpha*sqrt(I)` and `alpha = 2.0 kg^0.5 mol^-0.5` for 1:1 and 2:1
electrolytes:

```text
B^phi_MX   = beta0 + beta1 * exp(-alpha*sqrt(I))
B^gamma_MX = 2*beta0 + (2*beta1 / (alpha^2 * I))
             * [ 1 - (1 + alpha*sqrt(I) - 0.5*alpha^2*I) * exp(-alpha*sqrt(I)) ]
```

(`B^gamma_MX -> 2*beta0 + 2*beta1` as `I -> 0`, evaluated analytically here
to avoid the `0/0` at `I = 0`.)

**Mean activity coefficient** of the binary salt, `nu = nu_M + nu_X`,
`C^gamma_MX = 1.5 * C_phi`:

```text
ln(gamma_pm) = |z_M z_X| * f^gamma
             + m * (2*nu_M*nu_X / nu)          * B^gamma_MX
             + m^2 * (2*(nu_M*nu_X)^1.5 / nu)  * C^gamma_MX
```

**Osmotic coefficient**:

```text
phi = 1 + |z_M z_X| * f^phi
        + m * (2*nu_M*nu_X / nu)         * B^phi_MX
        + m^2 * (2*(nu_M*nu_X)^1.5 / nu) * C_phi
```

## Verification (against published tabulations)

Methodology: evaluate `mean_activity_coefficient` / `osmotic_coefficient`
for NaCl and CaCl2 with the Pitzer & Mayorga (1973) parameters and compare
to their tabulated `gamma_pm` / `phi`. Results measured from this
implementation on 2026-07-23 (see `tests` at the foot of this file):

| Quantity                      | This code | Published | Source                     |
|-------------------------------|-----------|-----------|----------------------------|
| NaCl gamma_pm, m = 0.1        | 0.7771    | ~0.778    | Pitzer & Mayorga 1973      |
| NaCl gamma_pm, m = 1.0        | 0.6561    | ~0.657    | Pitzer & Mayorga 1973      |
| NaCl gamma_pm, m = 3.0        | 0.7139    | ~0.714    | Robinson & Stokes / P&M    |
| NaCl phi,      m = 1.0        | 0.9361    | ~0.936    | Pitzer & Mayorga 1973      |
| CaCl2 gamma_pm upturn m1→m3   | 0.50→1.47 | rises     | Pitzer & Mayorga 1973      |

Agreement is within `~0.001` of the published values across the range — the
high-molality validity the Debye–Hückel/Davies models lack.

## Human-review flags (this is untrusted AI-generated draft material)

Per `RESPONSIBLE_USE.md`, treat this as a draft pending human inspection,
licence-provenance review, and independent verification. Specific
limitations:

- **25 degrees C only.** [`A_PHI_25C`] and the `beta`/`C_phi` parameters are
  fixed at 25 degrees C. There is no temperature dependence; using this at
  other temperatures is a modelling error.
- **Binary (single-salt) only.** No cation–cation / anion–anion mixing
  terms (`theta_ij`), no triplet terms (`psi_ijk`). A real multi-salt brine
  needs those; this handles one MX salt in water.
- **1:1 and 2:1 electrolytes.** `alpha` is fixed at `2.0`; 2:2 electrolytes
  (which use a two-term `beta1`/`beta2` form with `alpha1 = 1.4`,
  `alpha2 = 12`) are **not** modelled.
- **Fully-dissociated salt assumed.** No ion pairing / association.

## Provenance

Pitzer, K. S. (1973), "Thermodynamics of electrolytes. I. Theoretical basis
and general equations", *J. Phys. Chem.* **77**(2), 268–277. Pitzer, K. S.
& Mayorga, G. (1973), "Thermodynamics of electrolytes. II. Activity and
osmotic coefficients for strong electrolytes with one or both ions
univalent", *J. Phys. Chem.* **77**(19), 2300–2308 (source of the NaCl and
CaCl2 parameters and the `b = 1.2`, `alpha = 2.0` constants). Cross-check
tabulations: Robinson, R. A. & Stokes, R. H., *Electrolyte Solutions*, 2nd
ed. (Butterworths, 1959). These are open, published models.

Enum-free by design (a single struct of parameters); no trait objects, no
`Box`, no lifetimes (workspace rules).

```rust
pub mod pitzer { /* ... */ }
```

### Types

#### Struct `PitzerBinaryParams`

Pitzer ion-interaction virial parameters for a single **binary** electrolyte
(one cation, one anion — e.g. NaCl, CaCl2) at 25 degrees C.

The `beta0`, `beta1`, and `c_phi` fields are the empirically-fitted virial
coefficients; the charge and stoichiometry fields describe the salt's
dissociation `M_{nu_cation} X_{nu_anion} -> nu_cation M^{z_cation} +
nu_anion X^{z_anion}`.

# Units

`beta0`, `beta1`, and `c_phi` are dimensionless in the sense that they carry
the `kg/mol` powers implied by the molality expansion (they are the values
tabulated by Pitzer & Mayorga 1973). Charges are signed charge numbers;
stoichiometric coefficients are counts.

# Valid range

Constructed for 1:1 and 2:1 electrolytes at 25 degrees C. Evaluation methods
are trustworthy to high ionic strength (`> 6 molal` for NaCl) — the regime
the Debye–Hückel/Davies models in [`crate::activity`] cannot reach.

```rust
pub struct PitzerBinaryParams {
    pub beta0: f64,
    pub beta1: f64,
    pub c_phi: f64,
    pub z_cation: f64,
    pub z_anion: f64,
    pub nu_cation: f64,
    pub nu_anion: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `beta0` | `f64` | Second-virial `beta^(0)_MX` coefficient (kg/mol). |
| `beta1` | `f64` | Second-virial `beta^(1)_MX` coefficient (kg/mol) — the ionic-strength-<br>dependent part, gated by the `g(alpha*sqrt(I))` shape function. |
| `c_phi` | `f64` | Third-virial `C^phi_MX` coefficient (kg^2/mol^2). Note `C^gamma = 1.5 *<br>C_phi` is used in the activity-coefficient expression. |
| `z_cation` | `f64` | Signed charge number of the cation (e.g. `+1.0` for Na+, `+2.0` for<br>Ca2+). |
| `z_anion` | `f64` | Signed charge number of the anion (e.g. `-1.0` for Cl-). |
| `nu_cation` | `f64` | Stoichiometric coefficient of the cation (`nu_M`, e.g. `1.0` for NaCl,<br>`1.0` for CaCl2). |
| `nu_anion` | `f64` | Stoichiometric coefficient of the anion (`nu_X`, e.g. `1.0` for NaCl,<br>`2.0` for CaCl2). |

##### Implementations

###### Methods

- ```rust
  pub fn nacl() -> Self { /* ... */ }
  ```
  Published **NaCl** parameters (Pitzer & Mayorga 1973, Table I):

- ```rust
  pub fn cacl2() -> Self { /* ... */ }
  ```
  Published **CaCl2** parameters (Pitzer & Mayorga 1973, Table I):

- ```rust
  pub fn ionic_strength(self: &Self, molality: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Ionic strength `I = 0.5 * (nu_M * m * z_M^2 + nu_X * m * z_X^2)` in

- ```rust
  pub fn mean_activity_coefficient(self: &Self, molality: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Mean activity coefficient `gamma_pm` of the salt at stoichiometric

- ```rust
  pub fn osmotic_coefficient(self: &Self, molality: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Osmotic coefficient `phi` of the solvent at stoichiometric molality `m`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PitzerBinaryParams { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PitzerBinaryParams) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Constants and Statics

#### Constant `A_PHI_25C`

Debye–Hückel–Pitzer osmotic-coefficient slope `A_phi` at 25 degrees C in
water, in units of `kg^0.5 mol^-0.5`.

This is the Pitzer form of the Debye–Hückel limiting slope (note it is
**not** the same constant as the base-10 `A ≈ 0.509` used by
[`crate::activity`]; that one multiplies `log10 gamma`, this one appears in
natural-log osmotic form). The commonly quoted value is `0.3915` at
25 degrees C (Pitzer & Mayorga 1973); `0.391` is used here and reproduces
the published `gamma_pm` / `phi` tables to `~0.001`.

```rust
pub const A_PHI_25C: f64 = 0.391;
```

## Module `properties`

Fluid & material properties (v1) — bead op-v6s.7.

The constitutive closures the RICHARDS flow slice needs, `uom`-typed at the
public boundary and pure `f64` (SI base units) inside:

- [`LiquidWaterEos`] — a slightly-compressible, constant-viscosity liquid
  equation of state giving density `rho(p)`, its pressure derivative, and a
  constant viscosity. See [`eos`].
- [`CharacteristicCurves`] — the variably-saturated retention `S_e(p_c)` and
  liquid relative-permeability `k_r(S_e)` curves, dispatched by enum over the
  [`VanGenuchten`] (van Genuchten–Mualem) and [`BrooksCorey`]
  (Brooks–Corey–Burdine) model families. See [`curves`].
- [`ThermalWaterProperties`] and [`RockThermalProperties`] — the
  temperature-dependent liquid-water and solid-matrix thermal closures the
  **TH** (thermal-hydraulic) flow mode needs for its energy balance:
  `rho(p, T)`, `mu(T)`, and the `c`/`k` transport coefficients (bead
  op-v6s.10). Verification-only correlations, not IAPWS-IF97. See [`thermal`].

# Conventions (shared by the curves)

- Capillary pressure `p_c = p_gas - p_liq`; `p_c >= 0` unsaturated,
  `p_c <= 0` fully saturated.
- Effective saturation `S_e = (S_l - S_r)/(1 - S_r)` in `[0, 1]`; convert to
  liquid saturation with `S_l = S_r + (1 - S_r) S_e` using
  [`CharacteristicCurves::residual_saturation`].
- Outputs are clamped (`S_e`, `k_r` to `[0, 1]`); saturated/residual limits
  are handled without `NaN`. The one genuinely singular slope
  (van Genuchten–Mualem `dk_r/dS_e -> +inf` as `S_e -> 1`) is documented on
  [`VanGenuchten`] and returns `f64::INFINITY`, never `NaN`.

# Design notes for reviewers

- **Relative-permeability model pairing.** van Genuchten retention is paired
  with **Mualem** (1976) pore-connectivity `k_r`; Brooks–Corey retention with
  **Burdine** `k_r`. These are the classical, self-consistent pairings and
  match PFLOTRAN's defaults; deviating (e.g. VG+Burdine) is possible upstream
  but not offered here in v1.
- **Untrusted AI-generated draft** until a human reviews it (workspace
  `RESPONSIBLE_USE.md`): no human V&V yet. The analytical derivatives are
  unit-tested against central finite differences, but the curves have not
  been validated against a published PFLOTRAN reference case.

```rust
pub mod properties { /* ... */ }
```

### Re-exports

#### Re-export `BrooksCorey`

```rust
pub use curves::BrooksCorey;
```

#### Re-export `CharacteristicCurves`

```rust
pub use curves::CharacteristicCurves;
```

#### Re-export `Haverkamp`

```rust
pub use curves::Haverkamp;
```

#### Re-export `VanGenuchten`

```rust
pub use curves::VanGenuchten;
```

#### Re-export `LiquidWaterEos`

```rust
pub use eos::LiquidWaterEos;
```

#### Re-export `RockThermalProperties`

```rust
pub use thermal::RockThermalProperties;
```

#### Re-export `ThermalWaterProperties`

```rust
pub use thermal::ThermalWaterProperties;
```

## Module `reactive_transport`

Reactive transport (v1) — operator-split (SNIA) coupling of solute
[`transport`](crate::transport) and equilibrium
[`geochemistry`](crate::geochemistry). Bead op-v6s.12.

Multi-component reactive transport by **sequential non-iterative operator
splitting** (SNIA). Each time step advances `Nc` aqueous component *total*
concentrations in two decoupled sub-steps:

1. **Transport** — each component's total concentration is advected and
   dispersed by the same frozen flow field, exactly as in the single-solute
   [`crate::transport`] solver. Because the advection–dispersion operator
   depends only on the flow field (not on the species), the same implicit-
   Euler matrix `A` is assembled **once** and reused: for component `k` we
   solve `A * total_k = b_k`, changing only the right-hand side `b_k` (its
   accumulation term uses component `k`'s old totals, and its boundary term
   uses component `k`'s inflow total).
2. **Reaction** — each cell is then re-speciated to instantaneous chemical
   equilibrium with [`ReactionNetwork::speciate`], redistributing each
   component's transported total over its free primary and its secondary
   (complexed) species. Equilibrium speciation conserves every component
   total by construction (mass balance), so the reaction sub-step changes the
   *speciation* of a cell but not its transported totals.

# Governing form

For component `k` with total aqueous concentration `T_k` (mol/L), water
content `theta_w`, Darcy flux `q`, and effective dispersion `D`:

```text
d/dt(theta_w T_k) + div(q T_k) - div(theta_w D grad T_k) = R_k
```

SNIA solves the transport terms (left side, `R_k = 0`) implicitly for the new
`T_k`, then applies the reaction operator (equilibrium re-speciation) as a
cell-local correction. The two are not iterated to convergence within a step.

# Assumptions and simplifications (flagged for human review)

- **SNIA splitting error is O(dt).** Because transport and reaction are
  applied sequentially without sub-iteration, the operator-splitting error is
  first order in the time step. Sharp reaction fronts require a small `dt`;
  this is a documented accuracy limit, not a bug. A Strang-split (O(dt^2)) or
  a global-implicit (GIRT) variant is deferred.
- **All species are aqueous and mobile.** Every component total is
  transported by the *same* advection–dispersion operator with the *same*
  dispersion coefficient — i.e. all mass is dissolved and moves with the
  water. Immobile phases (sorbed species, minerals) and species-specific
  diffusion coefficients are **not** modelled; adding minerals would make
  part of a component's total immobile and is deferred with the geochemistry
  mineral phases.
- **Equilibrium chemistry only**, inheriting every simplification of
  [`crate::geochemistry`] (ideal activities `gamma = 1`, no kinetics, no
  charge-balance constraint, no water activity).
- **First-order upwind advection**, inheriting the numerical cross-wind
  diffusion `~ |v| dx / 2` of the [`crate::transport`] scheme.

# Units

Concentrations (component totals, free primaries, secondaries) are mol/L;
volumetric fluxes are m^3/s; water content is dimensionless; the dispersion
coefficient is m^2/s; component "mass" reported by
[`ReactiveTransport::component_mass`] is in mol (a volume-integrated amount,
`sum_i V_i theta_w_i T_i`); time steps are seconds. Plain `f64` is used (not
`uom`) for the same reason as [`crate::transport`]: the solve mixes
quantities of differing dimension and callers apply units at case setup.

Enum dispatch throughout, no trait objects, per the workspace design rules.

```rust
pub mod reactive_transport { /* ... */ }
```

### Types

#### Struct `ReactiveBoundaryCondition`

A reactive (multi-component) boundary condition bound to one of the six
domain-box face locations.

It fixes the inflow **total** concentration of every component at that face.
The advective part is upwinded by the boundary flux sign (inflow carries the
specified totals into the domain; outflow carries the interior totals out) and
the dispersive part couples the near-boundary cell to the specified totals
across the half-cell distance, so — like
[`crate::transport::TransportBoundaryKind::InflowConcentration`] — this acts
as a Dirichlet condition on each component total at an inflow or an outflow
face. A face location with no [`ReactiveBoundaryCondition`] uses the default
advective-outflow / zero-gradient behaviour for every component.

```rust
pub struct ReactiveBoundaryCondition {
    pub location: crate::grid::BoundaryLocation,
    pub inflow_totals: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `location` | `crate::grid::BoundaryLocation` | Which exterior face location this condition applies to. |
| `inflow_totals` | `Vec<f64>` | The fixed inflow total concentration of each component, mol/L. Length must<br>equal the number of components `Nc = network.n_primary()`; column `k`<br>is component `k`'s boundary total. |

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ReactiveTransport`

One SNIA reactive-transport stepper over a frozen flow field.

Holds the grid, the frozen [`FlowField`], the scalar dispersion parameters
(`molecular_diffusion` and `longitudinal_dispersivity`, shared by every
component), the [`ReactionNetwork`], the per-location boundary totals, the
time step `dt` (s), the current component totals `totals[cell][component]`
(mol/L), and the last equilibrium free-primary concentrations
`last_primary[cell][component]` (mol/L, used to warm-start each cell's Newton
speciation). [`ReactiveTransport::step`] transports every component total then
re-speciates every cell.

```rust
pub struct ReactiveTransport {
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
  pub fn new(grid: CartesianGrid, flow: FlowField, molecular_diffusion: f64, longitudinal_dispersivity: f64, network: ReactionNetwork, boundary: Vec<ReactiveBoundaryCondition>, initial_totals: Vec<Vec<f64>>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a reactive-transport stepper.

- ```rust
  pub fn set_timestep(self: &mut Self, dt: f64) { /* ... */ }
  ```
  Set the time step `dt` (seconds). Validated (must be positive and finite)

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of grid cells (the size of each per-component linear system).

- ```rust
  pub fn n_components(self: &Self) -> usize { /* ... */ }
  ```
  Number of transported components `Nc` (the reaction network's primary

- ```rust
  pub fn step(self: &mut Self) -> Result<(), PflotranError> { /* ... */ }
  ```
  Advance one SNIA step: transport every component total through the shared

- ```rust
  pub fn totals(self: &Self, component: usize) -> Vec<f64> { /* ... */ }
  ```
  Current total concentration of one component across all cells (mol/L),

- ```rust
  pub fn speciate_cell(self: &Self, cell: usize) -> Result<Speciation, PflotranError> { /* ... */ }
  ```
  Solve the equilibrium speciation of one cell from its current component

- ```rust
  pub fn component_mass(self: &Self, component: usize) -> f64 { /* ... */ }
  ```
  Total amount of one component over the whole domain (mol),

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `solver`

Newton–Krylov nonlinear solver layer (v1, KEYSTONE) — bead op-v6s.4.

A serial, pure-Rust Newton driver for a nonlinear algebraic system
`F(x) = 0` (the discretized residual of a flow mode). It is **generic over
the system** via the [`NonlinearSystem`] contract — static dispatch, never a
trait object — and delegates each linear Newton step `J·dx = -F` to
`outram-foam-basic-lib`'s [`krylov`](outram_foam_basic_lib::krylov)
asymmetric solvers (BiCGStab / GMRES) wrapped by an ILU(0) / Jacobi /
identity preconditioner. No PETSc, no MPI.

# Algorithm

Given an initial guess `x` the driver iterates:

1. Assemble the residual `F(x)`; its 2-norm `||F||₂` measures how far `x` is
   from a root. The first iteration's norm is cached as `||F₀||₂`.
2. **Converged** when `||F||₂ < abs_tol` OR `||F||₂ < rel_tol·||F₀||₂`.
3. Otherwise assemble the Jacobian `J = dF/dx` into a fixed-sparsity
   [`LduMatrix`], build the chosen preconditioner from `J`, and solve
   `J·dx = -F` with the chosen Krylov method.
4. **Line search (backtracking / Armijo):** try the full step `λ = 1`; if the
   residual norm is not sufficiently reduced, halve `λ` up to
   [`NewtonConfig::max_backtracks`] times, then update `x ← x + λ·dx`.

The Jacobian sparsity (owner/neighbour face addressing) is **fixed** across
the whole solve; only the coefficient values change per iteration, so the
[`LduMatrix`] is allocated once and refilled in place.

# Convergence & honesty

If the loop exhausts [`NewtonConfig::max_iterations`] without meeting the
tolerance it returns [`PflotranError::Convergence`] carrying the last norm —
it never reports a non-root as a solution. A NaN/Inf appearing in the
residual or the Newton step is likewise reported as a `Convergence` failure
rather than propagated silently.

```rust
pub mod solver { /* ... */ }
```

### Modules

## Module `block`

Block multi-DOF Newton–Krylov solver layer — bead op-v6s.4.1.

The scalar [`crate::solver`] handles one unknown per cell over
`outram-foam-basic-lib`'s scalar `LduMatrix`. Coupled multiphase physics
(e.g. the GENERAL mode: gas pressure + saturation + temperature) instead
carries `nb` unknowns per cell, with an `nb×nb` coupling block on every cell
and on every face. A scalar `LduMatrix` cannot represent that per-face
`nb×nb` coupling, so this module provides a self-contained block-sparse
layer built on the grid's face connectivity.

# What lives here

- [`BlockLduMatrix`] — a block-sparse matrix over owner/neighbour face
  addressing: an `nb×nb` diagonal block per cell and two `nb×nb` off-diagonal
  blocks per internal face (owner→neighbour in `upper`, neighbour→owner in
  `lower`).
- [`BlockJacobiPreconditioner`] — inverts each `nb×nb` diagonal block with a
  dense LU (`outram-foam-basic-lib`'s `SquareMatrix`) for use as a
  block-diagonal preconditioner.
- [`block_bicgstab`] — preconditioned BiCGStab on the flattened block system,
  using [`BlockLduMatrix::multiply`] as the sparse matrix–vector product.
- [`BlockNonlinearSystem`] — the compiler-contract for a block nonlinear
  system `F(x) = 0`, consumed via generics (static dispatch, never a trait
  object).
- [`BlockNewtonSolver`] — the Newton driver that mirrors the scalar
  [`crate::solver::NewtonSolver`] structure at the block level: assemble
  residual, converge-test (abs/rel), assemble block Jacobian, block-Jacobi
  preconditioned BiCGStab for `J·dx = -F`, Armijo backtracking line search,
  update.

# State-vector layout

All state vectors are length `n_cells * nb`. Cell `c`'s degrees of freedom
occupy the contiguous slice `[c*nb .. c*nb + nb]`. Blocks are stored
**row-major**: block entry `(i, j)` (row `i`, column `j`, both in `0..nb`) is
at offset `i*nb + j` within that block's `nb*nb`-length span.

# Units

As in the scalar layer, `x`, the residual, and the Jacobian entries are
dimensionless numeric coefficients — any `uom` unit bookkeeping belongs to
the flow mode that assembles them, not to this generic block layer.

```rust
pub mod block { /* ... */ }
```

### Types

#### Struct `BlockLduMatrix`

Block-sparse matrix over grid face connectivity; `nb` unknowns per cell.

The sparsity mirrors an OpenFOAM-style LDU addressing but with dense `nb×nb`
blocks instead of scalar coefficients:

- one `nb×nb` **diagonal** block per cell — cell `c`'s block occupies
  `diag[c*nb*nb .. (c+1)*nb*nb]`, row-major;
- two `nb×nb` **off-diagonal** blocks per internal face `f` — the
  owner→neighbour coupling (`∂F_owner/∂x_neighbour`) in
  `upper[f*nb*nb ..]`, and the neighbour→owner coupling
  (`∂F_neighbour/∂x_owner`) in `lower[f*nb*nb ..]`.

Every face satisfies `owner[f] < neighbour[f]`. Within a block, entry
`(i, j)` sits at flat offset `i*nb + j`.

A state vector `x` acted on by this matrix has length `n_cells * nb`, with
cell `c`'s unknowns at `x[c*nb .. c*nb + nb]`.

```rust
pub struct BlockLduMatrix {
    pub n_cells: usize,
    pub nb: usize,
    pub n_internal_faces: usize,
    pub diag: Vec<f64>,
    pub upper: Vec<f64>,
    pub lower: Vec<f64>,
    pub owner: Vec<usize>,
    pub neighbour: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_cells` | `usize` | Number of cells (equations groups); the state length is `n_cells * nb`. |
| `nb` | `usize` | Unknowns per cell — the block dimension `nb`. |
| `n_internal_faces` | `usize` | Number of internal faces; the length of `owner` / `neighbour` and the<br>count of `upper` / `lower` blocks. |
| `diag` | `Vec<f64>` | Diagonal blocks, `n_cells` blocks of `nb*nb` row-major entries each. |
| `upper` | `Vec<f64>` | Owner→neighbour off-diagonal blocks, `n_internal_faces` blocks of<br>`nb*nb` entries each. `upper[f]` block holds `∂F_owner/∂x_neighbour`. |
| `lower` | `Vec<f64>` | Neighbour→owner off-diagonal blocks, `n_internal_faces` blocks of<br>`nb*nb` entries each. `lower[f]` block holds `∂F_neighbour/∂x_owner`. |
| `owner` | `Vec<usize>` | Face owner cell indices, `owner[f] < neighbour[f]`. |
| `neighbour` | `Vec<usize>` | Face neighbour cell indices. |

##### Implementations

###### Methods

- ```rust
  pub fn new(n_cells: usize, nb: usize, owner: Vec<usize>, neighbour: Vec<usize>) -> Self { /* ... */ }
  ```
  Allocate a zero-filled block matrix for `n_cells` cells, `nb` unknowns

- ```rust
  pub fn zero(self: &mut Self) { /* ... */ }
  ```
  Zero all coefficient blocks (`diag`, `upper`, `lower`) in place, keeping

- ```rust
  pub fn multiply(self: &Self, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Sparse block matrix–vector product `y = A·x`.

- ```rust
  pub fn add_diag(self: &mut Self, cell: usize, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Accumulate `v` into diagonal-block entry `(i, j)` of `cell` — i.e.

- ```rust
  pub fn add_upper(self: &mut Self, face: usize, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Accumulate `v` into owner→neighbour (`upper`) block entry `(i, j)` of

- ```rust
  pub fn add_lower(self: &mut Self, face: usize, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Accumulate `v` into neighbour→owner (`lower`) block entry `(i, j)` of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BlockJacobiPreconditioner`

Block-Jacobi preconditioner: the inverse of each `nb×nb` diagonal block.

Applying it solves the block-diagonal system `D·z = r` cell-by-cell, using a
precomputed dense inverse of each diagonal block. A diagonal block that is
singular (its LU factorization fails) falls back to the identity block for
that cell, so `apply` never produces NaN from a failed factorization; the
preconditioner is then merely weaker there, not wrong.

```rust
pub struct BlockJacobiPreconditioner {
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
  pub fn new(a: &BlockLduMatrix) -> Self { /* ... */ }
  ```
  Factor each `nb×nb` diagonal block of `a` and store its inverse.

- ```rust
  pub fn apply(self: &Self, r: &[f64], z: &mut [f64]) { /* ... */ }
  ```
  Apply the preconditioner: `z = M^{-1} r`, a block-diagonal solve.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BlockNewtonConfig`

Configuration for the block Newton–Krylov solver.

Convergence is declared when the residual 2-norm falls below `abs_tol`, or
below `rel_tol` times the initial residual norm. See
[`BlockNewtonConfig::default`] for the shipped defaults.

```rust
pub struct BlockNewtonConfig {
    pub max_iterations: usize,
    pub abs_tol: f64,
    pub rel_tol: f64,
    pub linear_tolerance: f64,
    pub linear_max_iter: usize,
    pub max_backtracks: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iterations` | `usize` | Maximum Newton iterations before giving up with<br>[`PflotranError::Convergence`]. |
| `abs_tol` | `f64` | Absolute convergence tolerance: converged when `||F||₂ < abs_tol`. |
| `rel_tol` | `f64` | Relative convergence tolerance: converged when<br>`||F||₂ < rel_tol · ||F₀||₂`. |
| `linear_tolerance` | `f64` | Relative-residual tolerance passed to the inner [`block_bicgstab`] solve. |
| `linear_max_iter` | `usize` | Maximum iterations for the inner [`block_bicgstab`] solve. |
| `max_backtracks` | `usize` | Maximum backtracking line-search halvings per Newton step. `0` disables<br>the line search (pure full-step Newton, `λ = 1`). |

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
- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    Defaults: 50 Newton iterations, `abs_tol = 1e-9`, `rel_tol = 1e-8`,

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BlockNewtonReport`

Outcome of a [`BlockNewtonSolver::solve`] call.

Returned by value on success; on failure the same information is summarized
inside the [`PflotranError::Convergence`] message.

```rust
pub struct BlockNewtonReport {
    pub iterations: usize,
    pub converged: bool,
    pub final_residual_norm: f64,
    pub residual_history: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Number of Newton iterations performed (residual evaluations at which the<br>convergence test was applied). |
| `converged` | `bool` | Whether the tolerance was met. Always `true` in a successfully returned<br>report — a `false` outcome is surfaced as an `Err` instead. |
| `final_residual_norm` | `f64` | The residual 2-norm `||F||₂` at exit. |
| `residual_history` | `Vec<f64>` | The residual 2-norm at each Newton iteration, in order. |

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
    fn clone(self: &Self) -> BlockNewtonReport { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BlockNewtonSolver`

Serial pure-Rust block Newton–Krylov nonlinear solver.

Construct with [`BlockNewtonSolver::new`] and drive a
[`BlockNonlinearSystem`] to a root with [`BlockNewtonSolver::solve`]. The
driver mirrors the scalar [`crate::solver::NewtonSolver`] structure at the
block level: assemble `F`, converge-test (abs/rel), assemble the block
Jacobian, build a [`BlockJacobiPreconditioner`], solve `J·dx = -F` with
[`block_bicgstab`], then take a backtracking/Armijo line-search step.

```rust
pub struct BlockNewtonSolver {
    pub config: BlockNewtonConfig,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `config` | `BlockNewtonConfig` | The Newton / line-search / linear-solver configuration. |

##### Implementations

###### Methods

- ```rust
  pub fn new(config: BlockNewtonConfig) -> Self { /* ... */ }
  ```
  Create a block Newton solver with the given configuration.

- ```rust
  pub fn solve<S: BlockNonlinearSystem>(self: &Self, sys: &mut S, x: &mut [f64]) -> Result<BlockNewtonReport, PflotranError> { /* ... */ }
  ```
  Solve `F(x) = 0`, updating `x` in place with the converged root.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Traits

#### Trait `BlockNonlinearSystem`

Compiler-enforced contract for a block nonlinear system `F(x) = 0`.

Consumed via generics ([`BlockNewtonSolver::solve`] takes
`S: BlockNonlinearSystem`), so dispatch is **static** — never a trait object
(`Box<dyn _>` / `&dyn _`), per the workspace design rules.

The state vector `x` has length `n_cells * dof_per_cell`, with cell `c`'s
unknowns at `x[c*nb .. c*nb + nb]` (`nb = dof_per_cell`). The Jacobian
sparsity — the `(owner, neighbour)` face addressing — is **fixed for the
entire solve**; only the block coefficient values change per Newton
iteration, so the [`BlockLduMatrix`] is allocated once.

```rust
pub trait BlockNonlinearSystem {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `n_cells`: Number of cells; the state length is `n_cells() * dof_per_cell()`.
- `dof_per_cell`: Unknowns per cell — the block dimension `nb`. Constant across the solve.
- `ldu_addressing`: The fixed internal-face addressing as `(owner, neighbour)` arrays, one
- `assemble_residual`: Assemble the residual `F(x)` into `out` (length
- `assemble_jacobian`: Assemble the block Jacobian `dF/dx` into `jac` (already sized to the

##### Implementations

This trait is implemented for the following types:

- `GeneralFlow`
- `TwoPhaseFlow`
- `ThermalConvection`

### Functions

#### Function `block_bicgstab`

Left-preconditioned BiCGStab on the flattened block system `A·x = b`.

Solves the block-sparse linear system with [`BlockLduMatrix::multiply`] as
the sparse matrix–vector product and the [`BlockJacobiPreconditioner`] as a
(right-applied here, via the standard `p̂ = M^{-1} p` / `ŝ = M^{-1} s`
scheme) preconditioner. Operates purely on flat `f64` slices using the
`outram-foam-basic-lib` BLAS-1 [`vecops`](outram_foam_basic_lib::krylov::vecops)
helpers.

# Arguments

- `a` — the block system matrix.
- `b` — right-hand side, length `n_cells * nb`.
- `x0` — optional initial guess (length `n_cells * nb`); `None` starts from
  zero.
- `precond` — the block-Jacobi preconditioner (usually built from `a`).
- `tolerance` — relative-residual stopping tolerance `||b - A·x|| / ||b||`.
- `max_iter` — maximum BiCGStab iterations.

# Returns

`(solution, iterations, final_relative_residual, converged)`. The
`final_relative_residual` is recomputed as the *true* `||b - A·x|| / ||b||`
(not the recurrence residual). A zero right-hand side short-circuits to the
zero solution with `converged = true`. On a BiCGStab breakdown
(`rho ≈ 0` or `omega ≈ 0`) the iteration stops and returns the
best-residual iterate seen so far with `converged = false`.

```rust
pub fn block_bicgstab(a: &BlockLduMatrix, b: &[f64], x0: Option<&[f64]>, precond: &BlockJacobiPreconditioner, tolerance: f64, max_iter: usize) -> (Vec<f64>, usize, f64, bool) { /* ... */ }
```

### Types

#### Enum `LinearSolverKind`

Which Krylov method from [`outram_foam_basic_lib::krylov`] solves the linear
Newton step `J·dx = -F`.

Enum-dispatched (no trait objects): the driver matches on this to call
[`krylov::bicgstab`] or [`krylov::gmres`].

```rust
pub enum LinearSolverKind {
    BiCGStab,
    Gmres,
}
```

##### Variants

###### `BiCGStab`

Stabilized bi-conjugate gradient — cheap per iteration, no restart
parameter; the default for these nonsymmetric flow Jacobians.

###### `Gmres`

Restarted GMRES — more robust for strongly nonsymmetric / stiff systems,
at the cost of storing the restart-length Krylov basis.

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
    fn clone(self: &Self) -> LinearSolverKind { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LinearSolverKind) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `PreconditionerKind`

Which preconditioner wraps the Krylov solve, built afresh from the current
Jacobian each Newton iteration.

Enum-dispatched (no trait objects): the driver matches on this to construct
the corresponding [`Preconditioner`].

```rust
pub enum PreconditionerKind {
    Identity,
    Jacobi,
    Ilu0,
}
```

##### Variants

###### `Identity`

No preconditioning ([`Preconditioner::identity`]) — for testing or
already-well-conditioned systems.

###### `Jacobi`

Diagonal (Jacobi) scaling ([`Preconditioner::jacobi`]) — cheapest useful
choice.

###### `Ilu0`

Zero-fill incomplete-LU ([`Preconditioner::ilu0`]) — the strongest of the
three and the default; good for diffusion-dominated Jacobians.

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
    fn clone(self: &Self) -> PreconditionerKind { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PreconditionerKind) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `NewtonConfig`

Newton solver configuration.

See [`NewtonConfig::default`] for the shipped defaults. Convergence is
declared when the residual 2-norm falls below `abs_tol`, or below
`rel_tol` times the initial residual norm.

```rust
pub struct NewtonConfig {
    pub max_iterations: usize,
    pub abs_tol: f64,
    pub rel_tol: f64,
    pub linear: LinearSolverKind,
    pub preconditioner: PreconditionerKind,
    pub linear_settings: outram_foam_basic_lib::krylov::KrylovSettings,
    pub max_backtracks: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iterations` | `usize` | Maximum number of Newton iterations before giving up with<br>[`PflotranError::Convergence`]. |
| `abs_tol` | `f64` | Absolute convergence tolerance: converged when `||F||₂ < abs_tol`. |
| `rel_tol` | `f64` | Relative convergence tolerance: converged when<br>`||F||₂ < rel_tol · ||F₀||₂`, where `||F₀||₂` is the initial residual<br>norm. |
| `linear` | `LinearSolverKind` | Which Krylov method solves each linear step. |
| `preconditioner` | `PreconditionerKind` | Which preconditioner wraps the Krylov solve. |
| `linear_settings` | `outram_foam_basic_lib::krylov::KrylovSettings` | Settings (tolerance / max_iter / restart) passed to the inner Krylov<br>solve. |
| `max_backtracks` | `usize` | Maximum backtracking line-search halvings per Newton step. `0` disables<br>the line search entirely (pure full-step Newton, `λ = 1`). |

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
- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    Defaults: 50 Newton iterations, `abs_tol = 1e-9`, `rel_tol = 1e-8`,

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `NewtonReport`

Outcome of a [`NewtonSolver::solve`] call.

Returned by value on success; on failure the same information is summarized
inside the [`PflotranError::Convergence`] message.

```rust
pub struct NewtonReport {
    pub iterations: usize,
    pub converged: bool,
    pub final_residual_norm: f64,
    pub residual_history: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Number of Newton iterations performed (residual evaluations at which the<br>convergence test was applied). |
| `converged` | `bool` | Whether the tolerance was met. Always `true` in a successfully returned<br>report — a `false` outcome is surfaced as an `Err` instead. |
| `final_residual_norm` | `f64` | The residual 2-norm `||F||₂` at exit. |
| `residual_history` | `Vec<f64>` | The residual 2-norm at each iteration, in order — useful for plotting the<br>(near-quadratic) Newton convergence tail. |

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
    fn clone(self: &Self) -> NewtonReport { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `NewtonSolver`

Serial pure-Rust Newton–Krylov nonlinear solver.

Construct with [`NewtonSolver::new`] and drive a [`NonlinearSystem`] to a
root with [`NewtonSolver::solve`]. The [`config`](Self::config) is public so
callers may inspect or tweak it between solves.

```rust
pub struct NewtonSolver {
    pub config: NewtonConfig,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `config` | `NewtonConfig` | The Newton / line-search / linear-solver configuration. |

##### Implementations

###### Methods

- ```rust
  pub fn new(config: NewtonConfig) -> Self { /* ... */ }
  ```
  Create a solver with the given configuration.

- ```rust
  pub fn solve<S: NonlinearSystem>(self: &Self, sys: &mut S, x: &mut [f64]) -> Result<NewtonReport, PflotranError> { /* ... */ }
  ```
  Solve `F(x) = 0`, updating `x` in place with the converged root.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Traits

#### Trait `NonlinearSystem`

Compiler-enforced contract for a nonlinear system `F(x) = 0` solved by
Newton's method.

This trait is consumed via generics ([`NewtonSolver::solve`] takes
`S: NonlinearSystem`), so dispatch is **static** — it is never used as a
trait object (`Box<dyn _>` / `&dyn _`), per the workspace design rules.

# Sparsity contract

The Jacobian sparsity — the `(owner, neighbour)` face addressing returned by
[`ldu_addressing`](Self::ldu_addressing) — is **fixed for the entire solve**.
Only the coefficient *values* (filled by
[`assemble_jacobian`](Self::assemble_jacobian)) change from one Newton
iteration to the next. The driver relies on this to allocate the
[`LduMatrix`] once.

# Units

`x`, the residual, and the Jacobian entries are dimensionless numeric
coefficients here — any `uom` unit bookkeeping belongs to the flow mode that
assembles them, not to this generic contract.

```rust
pub trait NonlinearSystem {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `n_dof`: Number of degrees of freedom — the length of `x`, of the residual, and
- `ldu_addressing`: The fixed Jacobian sparsity as `(owner, neighbour)` arrays: one entry per
- `assemble_residual`: Assemble the residual `F(x)` into `out` (length [`n_dof`](Self::n_dof)).
- `assemble_jacobian`: Assemble the Jacobian `dF/dx` into `jac`, which is already sized to the

##### Implementations

This trait is implemented for the following types:

- `RichardsProblem`

### Re-exports

#### Re-export `block_bicgstab`

```rust
pub use block::block_bicgstab;
```

#### Re-export `BlockJacobiPreconditioner`

```rust
pub use block::BlockJacobiPreconditioner;
```

#### Re-export `BlockLduMatrix`

```rust
pub use block::BlockLduMatrix;
```

#### Re-export `BlockNewtonConfig`

```rust
pub use block::BlockNewtonConfig;
```

#### Re-export `BlockNewtonReport`

```rust
pub use block::BlockNewtonReport;
```

#### Re-export `BlockNewtonSolver`

```rust
pub use block::BlockNewtonSolver;
```

#### Re-export `BlockNonlinearSystem`

```rust
pub use block::BlockNonlinearSystem;
```

## Module `sorption`

Equilibrium sorption models for reactive transport (bead op-v6s.15.2).

Sorption is the reversible partitioning of a dissolved species between the
aqueous phase and the solid (mineral / soil) surface. In a reactive-transport
model it is the dominant control on how much *slower* a reactive solute moves
than the water carrying it — captured by the **retardation factor** `R`. This
module provides the two standard equilibrium families:

1. **Isotherms** ([`SorptionIsotherm`]) — a single-solute algebraic relation
   between the sorbed concentration `s` (mol per kg of dry solid) and the
   aqueous concentration `c` (mol per litre of solution):
   - **Linear (Kd):**   `s = Kd · c`                          (`Kd` in L/kg)
   - **Langmuir:**       `s = s_max · K·c / (1 + K·c)`         (`s_max` mol/kg, `K` L/mol)
   - **Freundlich:**     `s = Kf · c^n`,  `n ∈ (0, 1]`         (`Kf` mol/kg per (mol/L)^n)

2. **Ion exchange** ([`IonExchange`]) — multi-cation competition for a fixed
   number of negatively-charged exchange sites, using the **Gaines–Thomas**
   convention (activities of the sorbed species are their *equivalent
   fractions*). Given the aqueous concentrations of `N ≥ 2` competing cations
   it returns their equivalent fractions `beta_i` on the exchanger.

## Retardation

For a linearising sorption relation the one-dimensional advection–dispersion
equation for the total (aqueous + sorbed) mass carries an effective velocity
`v / R`, with

```text
    R = 1 + (rho_b / theta_w) · ds/dc
```

where `rho_b` is the dry **bulk density** of the porous medium (kg solid per
litre of bulk volume) and `theta_w` is the volumetric **water content**
(dimensionless, litre water per litre bulk). For the linear isotherm `ds/dc`
is the constant `Kd`, so `R` is constant; for Langmuir/Freundlich `ds/dc`
depends on `c`, so `R` is concentration-dependent (a nonlinear front).
See [`SorptionIsotherm::retardation`].

## Units summary

| Symbol   | Meaning                         | Unit            |
|----------|---------------------------------|-----------------|
| `c`      | aqueous concentration           | mol/L           |
| `s`      | sorbed concentration            | mol/kg solid    |
| `Kd`     | linear distribution coefficient | L/kg            |
| `s_max`  | Langmuir sorption capacity      | mol/kg solid    |
| `K`      | Langmuir affinity               | L/mol           |
| `Kf`     | Freundlich coefficient          | mol/kg /(mol/L)^n |
| `n`      | Freundlich exponent             | dimensionless, (0,1] |
| `rho_b`  | dry bulk density                | kg/L (bulk)     |
| `theta_w`| volumetric water content        | dimensionless   |
| `CEC`    | cation-exchange capacity        | eq/kg solid     |
| `beta_i` | exchanger equivalent fraction   | dimensionless   |

## Design (workspace mandate)

Enum dispatch, no trait objects, no `Box`, no lifetime parameters. The
ion-exchange equilibrium is closed with a single scalar site-activity term
and solved by a safeguarded (bisection-bracketed) **scalar Newton** iteration
— no dense linear algebra is needed.

## Scope and human-review flags

- **Equilibrium only.** Local instantaneous equilibrium is assumed; there is
  no kinetic (rate-limited) sorption here.
- **Ideal activities.** Aqueous *concentrations* are used directly as
  activities (activity coefficients `gamma = 1`), consistent with the v1
  [`crate::geochemistry`] speciation core. Debye–Hückel / Davies corrections
  are deferred.
- **Surface complexation is deferred** — no diffuse-layer / constant-
  capacitance / triple-layer electrostatic surface-complexation model is
  implemented in this module.
- **Untrusted AI-generated draft** until a human reviews it, per the workspace
  `RESPONSIBLE_USE.md`. Verification-only: the tests below check closed-form
  limits and finite-difference derivatives, not published PFLOTRAN cases.

## Provenance

Standard sorption / ion-exchange theory as used in PFLOTRAN's reactive-
transport (GIRT) module:
- I. Langmuir, "The adsorption of gases on plane surfaces of glass, mica and
  platinum," J. Am. Chem. Soc. 40 (1918) 1361.
- H. Freundlich, "Über die Adsorption in Lösungen," Z. Phys. Chem. 57 (1906) 385.
- G. L. Gaines & H. C. Thomas, "Adsorption Studies on Clay Minerals. II. A
  Formulation of the Thermodynamics of Exchange Adsorption," J. Chem. Phys.
  21 (1953) 714.
- C. A. J. Appelo & D. Postma, *Geochemistry, Groundwater and Pollution*,
  2nd ed., Balkema (2005), ch. 6 (ion exchange, Gaines–Thomas convention).
- G. E. Hammond, P. C. Lichtner & R. T. Mills, "Evaluating the performance of
  parallel subsurface simulators: An illustrative example with PFLOTRAN,"
  Water Resour. Res. 50 (2014) 208.

```rust
pub mod sorption { /* ... */ }
```

### Types

#### Enum `SorptionIsotherm`

Equilibrium sorption isotherm: sorbed `s` (mol/kg solid) versus aqueous `c`
(mol/L).

All three forms are single-solute, closed-form relations. Construct
[`SorptionIsotherm::Linear`] directly, or use the validating constructors
[`SorptionIsotherm::new_langmuir`] / [`SorptionIsotherm::new_freundlich`] for
the nonlinear forms, which reject non-physical parameters.

# Assumptions
- Local equilibrium; concentrations are used directly as activities
  (`gamma = 1`).
- `c >= 0`. Negative inputs are clamped to `0` internally so no `NaN` escapes
  from the fractional power in [`SorptionIsotherm::Freundlich`].

```rust
pub enum SorptionIsotherm {
    Linear {
        kd: f64,
    },
    Langmuir {
        s_max: f64,
        k: f64,
    },
    Freundlich {
        kf: f64,
        n: f64,
    },
}
```

##### Variants

###### `Linear`

Linear (constant-`Kd`) isotherm, `s = Kd · c`.

- `kd`: distribution coefficient, L/kg (i.e. mol/kg sorbed per mol/L aqueous).
  Physically `kd >= 0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `kd` | `f64` | Linear distribution coefficient `Kd`, in L/kg. |

###### `Langmuir`

Langmuir isotherm, `s = s_max · K·c / (1 + K·c)`.

Saturates at the monolayer capacity `s_max` as `c -> inf`. Build with
[`SorptionIsotherm::new_langmuir`] to enforce `s_max > 0`, `k > 0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `s_max` | `f64` | Maximum (monolayer) sorption capacity `s_max`, in mol/kg solid. |
| `k` | `f64` | Langmuir affinity constant `K`, in L/mol. |

###### `Freundlich`

Freundlich isotherm, `s = Kf · c^n`, with `n ∈ (0, 1]`.

Empirical power law for energetically heterogeneous surfaces. `n = 1`
reduces exactly to the linear isotherm with `Kd = Kf`. Build with
[`SorptionIsotherm::new_freundlich`] to enforce `kf > 0` and `n ∈ (0, 1]`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `kf` | `f64` | Freundlich coefficient `Kf`, in mol/kg per (mol/L)^n. |
| `n` | `f64` | Freundlich exponent `n`, dimensionless, in `(0, 1]`. |

##### Implementations

###### Methods

- ```rust
  pub fn sorbed(self: &Self, c: f64) -> f64 { /* ... */ }
  ```
  Sorbed concentration `s(c)` in mol/kg for aqueous `c` in mol/L (`c >= 0`).

- ```rust
  pub fn d_sorbed_d_c(self: &Self, c: f64) -> f64 { /* ... */ }
  ```
  Analytic derivative `ds/dc` (units: (mol/kg)/(mol/L) = L/kg).

- ```rust
  pub fn retardation(self: &Self, c: f64, bulk_density: f64, water_content: f64) -> f64 { /* ... */ }
  ```
  Retardation factor `R = 1 + (rho_b / theta_w) · ds/dc`.

- ```rust
  pub fn new_langmuir(s_max: f64, k: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Construct a validated [`SorptionIsotherm::Langmuir`].

- ```rust
  pub fn new_freundlich(kf: f64, n: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Construct a validated [`SorptionIsotherm::Freundlich`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SorptionIsotherm { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SorptionIsotherm) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `IonExchange`

Gaines–Thomas cation exchange on an exchanger of capacity `cec` (eq/kg).

Models `N ≥ 2` cations competing for a fixed pool of negatively-charged
exchange sites. The state of the exchanger is described by the **equivalent
fractions** `beta_i` (fraction of the total charge-equivalents held by cation
`i`), which are dimensionless and sum to 1.

# Mass-action model

Each cation `i` has charge `z_i` and a Gaines–Thomas half-reaction relating
it to the pool of exchange sites. Closing the system with a single common
**site-activity term** `mu` (a positive scalar Lagrange-like multiplier that
enforces `sum_i beta_i = 1`) gives the equivalent fractions in the compact form

```text
    beta_i = K_i · a_i · mu^(z_i)          with  sum_i beta_i = 1,
```

where `a_i` is the aqueous concentration (mol/L, used as activity) and
`K_i = 10^(log10_selectivity_i)` is cation `i`'s selectivity coefficient.
With this closure the *pairwise* Gaines–Thomas selectivity coefficient
between cations `i` and `j`,

```text
    K_ij = (beta_i^(z_j) · a_j^(z_i)) / (beta_j^(z_i) · a_i^(z_j)) = K_i^(z_j) / K_j^(z_i),
```

is a constant independent of the aqueous composition, as Gaines–Thomas
requires. By convention one cation (typically the reference / index 0) is
assigned `log10_selectivity = 0` (`K = 1`); the others are relative to it.

# Solve method

Because every term `K_i·a_i·mu^(z_i)` is positive and strictly increasing in
`mu > 0`, the closure `g(mu) = sum_i K_i·a_i·mu^(z_i) - 1 = 0` has a unique
positive root. It is found by a **safeguarded scalar Newton** iteration: the
root is first bracketed (`g(0) = -1 < 0`; the upper bound is found by
doubling `mu` until `g > 0`), then Newton steps are taken with a bisection
fallback whenever a step would leave the bracket. This is a robust 1-D solve
— no dense linear algebra is required.

# Sorbed amounts

The sorbed concentration of cation `i` in mol/kg is recovered from its
equivalent fraction as `q_i = beta_i · cec / z_i`.

# Scope
Equilibrium only; ideal activities (`gamma = 1`); no surface complexation.

```rust
pub struct IonExchange {
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
  pub fn new(cec: f64, charges: Vec<f64>, log10_selectivity: Vec<f64>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a Gaines–Thomas exchanger.

- ```rust
  pub fn n_cations(self: &Self) -> usize { /* ... */ }
  ```
  Number of competing cations `N`.

- ```rust
  pub fn cec(self: &Self) -> f64 { /* ... */ }
  ```
  Cation-exchange capacity `CEC`, in eq/kg solid.

- ```rust
  pub fn exchange_fractions(self: &Self, aqueous: &[f64]) -> Result<Vec<f64>, PflotranError> { /* ... */ }
  ```
  Equilibrium exchanger equivalent fractions `beta_i` in equilibrium with

- ```rust
  pub fn sorbed_amounts(self: &Self, aqueous: &[f64]) -> Result<Vec<f64>, PflotranError> { /* ... */ }
  ```
  Sorbed amounts `q_i = beta_i · CEC / z_i` (mol/kg solid) in equilibrium

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> IonExchange { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &IonExchange) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `surface_complexation`

Electrostatic surface-complexation sorption models (bead op-gg7).

This module extends the equilibrium sorption family in [`crate::sorption`]
(Kd / Langmuir / Freundlich isotherms and Gaines–Thomas ion exchange) with
**pH-dependent surface complexation**. Mineral surfaces such as **hydrous
ferric oxide (HFO)** expose amphoteric hydroxyl sites `>SOH` that protonate,
deprotonate, and bind aqueous metal cations. Because the reactions consume or
release `H+`, the amount of metal sorbed is a strong function of pH — the
characteristic sigmoidal *sorption edge* that isotherms cannot reproduce.

## Mass-action core (non-electrostatic model, NEM)

Three surface reactions act on a single amphoteric site type. Square brackets
denote **surface-species concentrations in mol per litre of solution**; `{H+}`
and `{M}` are aqueous **activities** (here taken equal to molarity, `gamma =
1`, matching the v1 [`crate::geochemistry`] core):

```text
  protonation    >SOH + H+  <=> >SOH2+          K_a1 = [>SOH2+] / ([>SOH]·{H+})
  deprotonation  >SOH       <=> >SO- + H+        K_a2 = [>SO-]·{H+} / [>SOH]
  metal binding  >SOH + M^z+ <=> >SOM^(z-1) + H+ K_M  = [>SOM]·{H+} / ([>SOH]·{M})
```

with the **site balance** (total site density conserved)

```text
  T = [>SOH] + [>SOH2+] + [>SO-] + [>SOM].
```

Every surface species is proportional to `[>SOH]`, so for the NEM the balance
is a single *linear* equation with the closed form

```text
  [>SOH] = T / (1 + K_a1·{H+} + K_a2/{H+} + K_M·{M}/{H+}),
```
and no iteration is needed.

## Electrostatic correction (CCM and DLM)

When surface charge develops, moving an ion of charge `z_i` through the
near-surface potential `psi` (volts) costs coulombic work, so each intrinsic
constant is corrected by a **Boltzmann factor**. Writing the change in the
*surface species'* charge across a reaction as `Δz`, the apparent
(conditional) constant is `K_app = K_int · exp(-Δz·F·psi/(R·T))`. Introducing
the master factor

```text
  P = exp(-F·psi/(R·T)),
```
the three surface-species ratios (relative to `[>SOH]`) become

```text
  [>SOH2+]/[>SOH] = K_a1·{H+}·P^(+1)      (Δz = +1)
  [>SO-]  /[>SOH] = K_a2/{H+}·P^(-1)      (Δz = -1)
  [>SOM]  /[>SOH] = K_M·{M}/{H+}·P^(z-1)  (Δz = z-1)
```

The **surface charge density** `sigma` (C/m^2) follows from the net surface
charge per litre and the surface area per litre `A·Cs`:

```text
  Q      = [>SOH2+] - [>SO-] + (z-1)·[>SOM]        (mol charge / L)
  sigma  = Q·F / (A·Cs)                            (C/m^2)
```
where `A` is the specific surface area (m^2/g) and `Cs` the solid
concentration (g/L), so `A·Cs` has units m^2 per litre of solution.

Two electrostatic closures relate `sigma` and `psi`, solved self-consistently:

- **Constant-capacitance model (CCM):** `sigma = C · psi`, i.e. `psi =
  sigma/C`, with the capacitance `C` in F/m^2. Appropriate for high, constant
  ionic strength.
- **Diffuse-layer model (DLM):** the Gouy–Chapman relation, in the form used
  by Dzombak & Morel (1990) for a symmetric 1:1 electrolyte at 25 °C,
  ```text
    sigma = 0.1174 · sqrt(I) · sinh(F·psi/(2·R·T)),
  ```
  where `I` is the ionic strength in mol/L. The prefactor `0.1174` C/m^2 is
  `sqrt(8·R·T·epsilon·epsilon0·1000)` evaluated at `T = 298.15 K` with the
  permittivity of water, so it is only valid at 25 °C.

## Solver and convergence handling

For the NEM the speciation is the closed form above (`psi = 0`). For CCM and
DLM the residual

```text
  f(psi) = sigma(psi) - closure(psi)
```
is **strictly monotone decreasing** in `psi`: `sigma(psi)` decreases as `psi`
rises (a more positive potential drives off positive charge), while both
closures (`C·psi` and the `sinh` term) increase. A monotone residual has a
unique root, which we find by **bracketing then bisection** — expand a small
symmetric interval about `psi = 0` until the residual changes sign, then
bisect. Bisection cannot diverge once a bracket exists, so it is used in
preference to a Newton step that could overshoot the stiff exponential. If a
bracket cannot be found inside a physical bound (±5 V, far outside any real
surface potential), a [`PflotranError::Convergence`] is returned rather than a
panic. The search bound also keeps every exponent finite, so no `NaN`/`inf`
escapes.

## Point of zero charge

Ignoring metal, the surface is neutral when `[>SOH2+] = [>SO-]`, i.e.
`K_a1·{H+} = K_a2/{H+}`, giving `{H+} = sqrt(K_a2/K_a1)` and

```text
  pH_pzc = 0.5·(log10 K_a1 - log10 K_a2).
```
With the sign convention here (`log_k_a2` is the deprotonation constant, a
*negative* number for an acidic site) this equals the midpoint of the two
`pKa`-style magnitudes. See [`SurfaceSite::point_of_zero_charge_ph`].

## Units summary

| Symbol   | Meaning                              | Unit           |
|----------|--------------------------------------|----------------|
| `{H+}`   | proton activity (`10^-pH`)           | mol/L          |
| `{M}`    | aqueous metal activity               | mol/L          |
| `[>S…]`  | surface-species concentration        | mol/L          |
| `T`      | total site density                   | mol/L          |
| `K_a1`   | protonation constant                 | L/mol          |
| `K_a2`   | deprotonation constant               | mol/L          |
| `K_M`    | metal-binding constant               | dimensionless* |
| `A`      | specific surface area                | m^2/g          |
| `Cs`     | solid concentration                  | g/L            |
| `C`      | CCM capacitance                      | F/m^2          |
| `I`      | ionic strength (DLM)                 | mol/L          |
| `psi`    | surface potential                    | V              |
| `sigma`  | surface charge density               | C/m^2          |

(*`K_M` as written groups `{M}` and `{H+}` so it is dimensionless for a
`1:1` proton exchange; the exact dimensionality depends on the reaction
stoichiometry chosen.)

## Scope and human-review flags

- **Single site type, single metal.** One amphoteric `>SOH` site and one
  metal complex are modelled. Multi-site (strong/weak HFO) and competitive
  multi-metal systems are **not** implemented here.
- **25 °C only.** The Faraday/`RT` factor and the DLM `0.1174` prefactor are
  evaluated at `T = 298.15 K`. No temperature dependence.
- **Equilibrium, ideal activities.** Local instantaneous equilibrium;
  concentrations used directly as activities (`gamma = 1`). No triple-layer
  model, no basic-Stern layer.
- **Untrusted AI-generated draft** until a human reviews it, per the
  workspace `RESPONSIBLE_USE.md`. The tests below check physical *limits*
  (charge sign vs pH, edge shape, mass conservation), **not** published
  PFLOTRAN / FITEQL reference numbers — this is verification, not validation.

## Provenance

- D. A. Dzombak & F. M. M. Morel, *Surface Complexation Modeling: Hydrous
  Ferric Oxide*, Wiley-Interscience (1990) — the diffuse-layer model,
  the `0.1174` C/m^2 prefactor, and the HFO reaction set.
- J. A. Davis & D. B. Kent, "Surface complexation modeling in aqueous
  geochemistry," *Reviews in Mineralogy* 23 (1990) 177 — CCM/DLM/TLM overview.
- G. Sposito, *The Surface Chemistry of Natural Particles*, Oxford (2004).

```rust
pub mod surface_complexation { /* ... */ }
```

### Types

#### Enum `SurfaceComplexationModel`

Choice of electrostatic closure for the surface-complexation solve.

Enum dispatch (no trait objects) per the workspace design rules. The physics
difference is only in how surface charge density `sigma` relates to surface
potential `psi`; the mass-action core is shared.

```rust
pub enum SurfaceComplexationModel {
    NonElectrostatic,
    ConstantCapacitance {
        capacitance: f64,
    },
    DiffuseLayer {
        ionic_strength: f64,
    },
}
```

##### Variants

###### `NonElectrostatic`

Non-electrostatic model — no coulombic correction (`psi = 0`, `P = 1`).

The intrinsic constants act directly; the site balance is the linear
closed form. Appropriate as a first approximation or at conditions where
electrostatics are negligible.

###### `ConstantCapacitance`

Constant-capacitance model: `sigma = C · psi`.

Suited to high, roughly constant ionic strength. `capacitance` is `C` in
**F/m^2** and must be strictly positive.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `capacitance` | `f64` | Surface capacitance `C`, in F/m^2 (`> 0`). |

###### `DiffuseLayer`

Diffuse-layer (Gouy–Chapman) model:
`sigma = 0.1174·sqrt(I)·sinh(F·psi/(2·R·T))` at 25 °C.

`ionic_strength` is `I` in **mol/L** and must be strictly positive.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ionic_strength` | `f64` | Bulk ionic strength `I`, in mol/L (`> 0`). |

##### Implementations

###### Methods

- ```rust
  pub fn validate(self: &Self) -> Result<(), PflotranError> { /* ... */ }
  ```
  Validate the model parameters.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SurfaceComplexationModel { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SurfaceComplexationModel) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SurfaceSite`

A single amphoteric surface-site type with its protonation constants,
site density, and the surface metrics needed for the electrostatic models.

All fields are public so a caller can build one literally; the validating
constructor [`SurfaceSite::new`] is preferred as it rejects non-physical
values, and [`SurfaceSite::speciate`] re-validates defensively.

# Units and assumptions
- `log_k_a1`, `log_k_a2` are base-10 logs of the intrinsic protonation /
  deprotonation constants. `log_k_a2` is typically **negative** (acidic site).
- `site_density` is the **total** `>SOH`-equivalent site concentration in
  **mol per litre of solution** (`> 0`).
- `specific_surface_area` (m^2/g) and `solid_concentration` (g/L) enter only
  through their product `A·Cs` (m^2/L), used to convert net surface charge to
  `sigma`. They are required (`> 0`) for CCM/DLM but ignored for the NEM.

```rust
pub struct SurfaceSite {
    pub log_k_a1: f64,
    pub log_k_a2: f64,
    pub site_density: f64,
    pub specific_surface_area: f64,
    pub solid_concentration: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `log_k_a1` | `f64` | `log10 K_a1` for `>SOH + H+ = >SOH2+` (protonation; usually positive). |
| `log_k_a2` | `f64` | `log10 K_a2` for `>SOH = >SO- + H+` (deprotonation; usually negative). |
| `site_density` | `f64` | Total site density `T`, in mol sites per litre of solution (`> 0`). |
| `specific_surface_area` | `f64` | Specific surface area `A`, in m^2/g (used for `sigma`; see module docs). |
| `solid_concentration` | `f64` | Solid concentration `Cs`, in g/L. |

##### Implementations

###### Methods

- ```rust
  pub fn new(log_k_a1: f64, log_k_a2: f64, site_density: f64, specific_surface_area: f64, solid_concentration: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Construct and validate a [`SurfaceSite`].

- ```rust
  pub fn point_of_zero_charge_ph(self: &Self) -> f64 { /* ... */ }
  ```
  Analytical point of zero charge (pH), `0.5·(log_k_a1 - log_k_a2)`.

- ```rust
  pub fn speciate(self: &Self, ph: f64, metal_molarity: f64, model: SurfaceComplexationModel, complex: &SurfaceComplex) -> Result<SurfaceSpeciation, PflotranError> { /* ... */ }
  ```
  Solve the equilibrium surface speciation at a given pH and aqueous metal

- ```rust
  pub fn sorption_edge(self: &Self, ph_values: &[f64], metal_molarity: f64, model: SurfaceComplexationModel, complex: &SurfaceComplex) -> Result<Vec<(f64, f64)>, PflotranError> { /* ... */ }
  ```
  Compute a **sorption edge**: the fraction of metal sorbed at each pH in

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SurfaceSite { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SurfaceSite) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SurfaceComplex`

A surface metal complex `>SOH + M^z+ = >SOM^(z-1) + H+`.

`log_k` is `log10 K_M` of the intrinsic binding constant, and `metal_charge`
is the aqueous cation charge `z` (`> 0`), which sets both the product complex
charge `z-1` and the coulombic exponent in the electrostatic models.

```rust
pub struct SurfaceComplex {
    pub log_k: f64,
    pub metal_charge: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `log_k` | `f64` | `log10 K_M` of the intrinsic metal-binding constant. |
| `metal_charge` | `f64` | Aqueous metal cation charge `z` (`> 0`, e.g. `2.0` for `Zn^2+`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(log_k: f64, metal_charge: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Construct and validate a [`SurfaceComplex`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SurfaceComplex { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SurfaceComplex) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SurfaceSpeciation`

The equilibrium surface speciation at a given pH and aqueous metal activity.

All four species concentrations are in **mol per litre of solution** and, by
construction, sum to the site density `T` (mass conservation). The surface
potential is in volts (`0` for the non-electrostatic model).

```rust
pub struct SurfaceSpeciation {
    pub soh: f64,
    pub soh2: f64,
    pub so_minus: f64,
    pub som: f64,
    pub surface_potential: f64,
    pub fraction_metal_sorbed: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `soh` | `f64` | Neutral site `[>SOH]`, mol/L. |
| `soh2` | `f64` | Protonated site `[>SOH2+]`, mol/L (positive charge). |
| `so_minus` | `f64` | Deprotonated site `[>SO-]`, mol/L (negative charge). |
| `som` | `f64` | Metal-bound site `[>SOM^(z-1)]`, mol/L. |
| `surface_potential` | `f64` | Self-consistent surface potential `psi`, in volts (`0` for the NEM). |
| `fraction_metal_sorbed` | `f64` | Fraction of total metal (`aqueous + sorbed`) held on the surface,<br>`[>SOM] / ({M} + [>SOM])`, dimensionless in `[0, 1]`. |

##### Implementations

###### Methods

- ```rust
  pub fn net_surface_charge(self: &Self, metal_charge: f64) -> f64 { /* ... */ }
  ```
  Net surface charge, in **mol of charge per litre**:

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SurfaceSpeciation { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SurfaceSpeciation) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `thermal_convection`

Two-way buoyancy-coupled thermal-hydraulic flow — density-driven porous-medium
convection (bead op-v6s.15.6).

This module closes the coupling loop that the one-way [`crate::energy`] module
leaves open. In [`crate::energy`], a *frozen* Darcy flow field advects heat but
the temperature never feeds back into the flow. Here the fluid density depends
on temperature, so a temperature field creates buoyancy forces that **drive**
the flow, which in turn advects heat — the fully coupled thermal-hydraulic
convection problem (natural convection in a saturated porous medium).

Two primary unknowns are carried per cell — liquid pressure `p` (Pa) and
temperature `T` (K) — and the pair is assembled onto the block multi-DOF Newton
solver [`crate::solver::block`] with `nb = 2` DOF per cell, exactly mirroring
the structure of the two-phase module [`crate::multiphase`].

# Governing equations

Fully liquid-saturated porous medium, cell-centred two-point-flux finite volume
in space, backward (implicit) Euler in time.

**Mass (flow), slightly-compressible Darcy with thermal buoyancy:**

$$ \frac{\partial}{\partial t}\left(\phi\, \rho\, c_p\, p\right) + \nabla\cdot\left(\rho\, \mathbf{q}\right) = 0, \qquad \mathbf{q} = -\frac{k}{\mu}\left(\nabla p - \rho\,\mathbf{g}\right) $$

**Energy (bulk fluid + rock):**

$$ \frac{\partial}{\partial t}\left[\left(\rho_f c_f \phi + \rho_r c_r (1-\phi)\right) T\right] + \nabla\cdot\left(\rho_f c_f\, \mathbf{q}\, T\right) - \nabla\cdot\left(\lambda\, \nabla T\right) = 0 $$

The **two-way coupling** lives entirely in the density `\rho(T)`: it appears in
the buoyancy (gravity) term `\rho\,\mathbf{g}` of the Darcy flux, so a lateral
temperature (hence density) contrast produces an unbalanced buoyancy force and
drives flow. That flow's volumetric flux `\mathbf{q}` is the same flux that
advects heat in the energy equation. Set the thermal-expansion coefficient
`\beta = 0` and the density becomes uniform, the buoyancy term becomes an exact
hydrostatic balance, the flow vanishes, and the problem degenerates to pure
conduction — which is exactly the isolation test used below.

# Density law (Boussinesq)

A documented **linear Boussinesq** law is used:

$$ \rho(T) = \rho_0 \left(1 - \beta\,(T - T_{ref})\right) $$

with `\rho_0` the reference density at `T_{ref}` and `\beta` the volumetric
thermal-expansion coefficient (1/K). The Boussinesq approximation retains this
density variation in the **buoyancy term** (the physical driver of convection)
and treats density as the constant `\rho_0` in the accumulation heat-capacity
products, which keeps those terms linear. `\beta` for liquid water near 20 °C is
`~2.1e-4` 1/K (see [`crate::properties::thermal::ThermalWaterProperties`], whose
`thermal_expansion` field carries the same value). We use the explicit
Boussinesq law rather than the exponential
[`ThermalWaterProperties::density`](crate::properties::thermal::ThermalWaterProperties::density)
because it makes the `\beta = 0` isolation and the Horton–Rogers–Lapwood
Rayleigh-number verification exact and transparent.

# Face flux (two-point, mean-density buoyancy)

For an internal face between owner `o` and neighbour `n`, with geometric
transmissibility `T_geom = area / distance` (m):

```text
rho_face = 0.5 * (rho(T_o) + rho(T_n))            # arithmetic-mean face density
dphi     = (p_o - p_n) + rho_face * g * (z_o - z_n)
q_vol    = (k / mu) * T_geom * dphi               # volumetric flux, m^3/s, +owner->neighbour
m_face   = rho_face * q_vol                       # mass flux, kg/s
e_adv    = c_f * T_upwind * m_face                # advected enthalpy, W (T_upwind by sign of q_vol)
e_cond   = lambda * T_geom * (T_o - T_n)          # conductive flux, W
```

The buoyancy term uses the **arithmetic-mean face density** (the standard,
stable variant that avoids the upstream-density circularity) while the advected
heat is upwind-weighted on temperature (first-order upwind, monotone). Elevation
`z` is `cell_center(c)[2]` (upward positive); buoyancy is nonzero only where
`z_o != z_n`, i.e. on vertical (`Axis::Z`) faces of a Cartesian grid, but it is
computed from the centroid elevations so the expression is general.

# Jacobian

The `nb = 2` block Jacobian is assembled **numerically** by finite-differencing
the two-equation cell residual with respect to the two local unknowns over the
two-point stencil, exactly as [`crate::multiphase`] does. This matches the
residual by construction and side-steps fragile analytic derivatives through the
buoyancy term and the upwind switch.

# Pressure null space

With an all-impermeable (no-flow / insulated) box the mass equation is a
pure-Neumann pressure problem: the pressure is defined only up to an additive
constant. A small fluid compressibility `c` (the `\rho c_p p` storage term,
`fluid_compressibility` in [`ConvectionParameters`]) regularises this and gives
the pressure a fast transient; the additive-constant freedom is harmless because
velocities and energies depend only on pressure *gradients*.

# Simplifications (v1) — flags for human review

- **Boussinesq, single-phase, fully saturated.** Density varies only through the
  linear law above; viscosity, specific heats and conductivity are constants.
- **Verification-only, not validated.** No comparison against published PFLOTRAN
  convection results has been made. The tests are self-contained physical checks
  (pure-conduction isolation, buoyancy onset, Rayleigh-number threshold).
- **Coarse-grid Rayleigh threshold is qualitative.** The Horton–Rogers–Lapwood
  critical value `Ra_c = 4\pi^2 \approx 39.478` is exact for a continuous square
  cell; on the coarse meshes used here only the *qualitative* onset across
  `4\pi^2` is demonstrated (subcritical stays conductive, supercritical
  convects). Quantitative `Ra_c` on a coarse mesh is approximate. The
  non-convecting cases sit at a small numerical conductive floor
  (~1e-7 m/s here, ~2% of the convecting velocity) rather than exactly zero —
  the arithmetic-mean face density in the buoyancy term does not perfectly
  cancel the discrete hydrostatic gradient; a finer mesh lowers it.
- **Convecting-regime solve.** The strongly-coupled convecting state is reached
  by two mechanisms: the energy conservation equation is **row-equilibrated**
  (divided by the volumetric heat capacity) so it is comparable in magnitude to
  the mass equation for the block-Jacobi-preconditioned linear solve, and the
  driver ([`ThermalConvectionSimulation::step_adaptive`]) takes **adaptive
  backward-Euler sub-steps**, halving on a failed Newton solve. Without both,
  the near-elliptic pressure system stalls at large steps.

# Provenance

Lapwood, E. R. (1948), "Convection of a fluid in a porous medium",
*Proc. Camb. Phil. Soc.* **44**, 508–521; Horton, C. W. & Rogers, F. T. (1945),
"Convection currents in a porous medium", *J. Appl. Phys.* **16**, 367; Nield,
D. A. & Bejan, A., *Convection in Porous Media* (the Horton–Rogers–Lapwood
problem and `Ra_c = 4\pi^2`). Boussinesq buoyancy approximation.

> **Untrusted AI draft — verification only**, per the workspace
> `RESPONSIBLE_USE.md`. Not for facility operation or any safety-critical use.

```rust
pub mod thermal_convection { /* ... */ }
```

### Types

#### Struct `ConvectionParameters`

Bulk thermal + hydraulic parameters for the coupled convection problem (SI
units).

Groups the porous-medium hydraulics (permeability, porosity, fluid viscosity
and compressibility), the linear Boussinesq density law (`reference_density`
`\rho_0`, `thermal_expansion` `\beta`, `reference_temperature` `T_{ref}`), and
the bulk energy properties (fluid specific heat, rock volumetric heat capacity,
bulk thermal conductivity), plus gravity. All are homogeneous (single scalar
per field) in this verification model.

Construct via [`ConvectionParameters::new`] (validated) or
[`ConvectionParameters::water_saturated`] (a water-in-rock default). Fields are
public for read access and literal construction, but bypassing `new` skips
validation.

```rust
pub struct ConvectionParameters {
    pub permeability: f64,
    pub porosity: f64,
    pub fluid_viscosity: f64,
    pub fluid_compressibility: f64,
    pub reference_density: f64,
    pub thermal_expansion: f64,
    pub reference_temperature: f64,
    pub fluid_specific_heat: f64,
    pub rock_volumetric_heat_capacity: f64,
    pub bulk_conductivity: f64,
    pub gravity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `permeability` | `f64` | Intrinsic (absolute) isotropic permeability `k`, m^2. Strictly positive. |
| `porosity` | `f64` | Porosity `\phi`, dimensionless, strictly in the open interval `(0, 1)`. |
| `fluid_viscosity` | `f64` | Fluid dynamic viscosity `\mu`, Pa·s (constant). Strictly positive. |
| `fluid_compressibility` | `f64` | Fluid isothermal compressibility `c`, 1/Pa. Non-negative. Enters only the<br>pressure storage (accumulation) term; a small value regularises the<br>pure-Neumann pressure null space of an impermeable box (see module header). |
| `reference_density` | `f64` | Boussinesq reference fluid density `\rho_0` at `reference_temperature`,<br>kg/m^3. Strictly positive. |
| `thermal_expansion` | `f64` | Boussinesq volumetric thermal-expansion coefficient `\beta`, 1/K.<br>Non-negative; `0` disables buoyancy (density becomes uniform). |
| `reference_temperature` | `f64` | Boussinesq reference temperature `T_{ref}` (K), where `\rho = \rho_0`.<br>Strictly positive (absolute). |
| `fluid_specific_heat` | `f64` | Fluid specific heat `c_f`, J/(kg·K) (constant). Strictly positive. |
| `rock_volumetric_heat_capacity` | `f64` | Rock volumetric heat capacity `\rho_r c_r`, J/(m^3·K) (constant, the solid<br>grains' storage per unit bulk volume of solid). Strictly positive. |
| `bulk_conductivity` | `f64` | Bulk (fluid + rock) thermal conductivity `\lambda`, W/(m·K) (constant).<br>Strictly positive. |
| `gravity` | `f64` | Gravitational acceleration magnitude `g`, m/s^2, acting in `-z`. Non-negative<br>(pass `0` to disable gravity/buoyancy entirely). |

##### Implementations

###### Methods

- ```rust
  pub fn water_saturated() -> Self { /* ... */ }
  ```
  A representative **water-saturated generic rock** parameter set near ~20 °C:

- ```rust
  pub fn new(permeability: f64, porosity: f64, fluid_viscosity: f64, fluid_compressibility: f64, reference_density: f64, thermal_expansion: f64, reference_temperature: f64, fluid_specific_heat: f64, rock_volumetric_heat_capacity: f64, bulk_conductivity: f64, gravity: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build and validate a full parameter set from raw SI values.

- ```rust
  pub fn fluid_density(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Boussinesq fluid density `\rho(T) = \rho_0 (1 - \beta (T - T_{ref}))`,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ConvectionParameters { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ConvectionParameters) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `ConvectionBoundaryKind`

What is prescribed on one exterior boundary location.

Unspecified faces default to [`ConvectionBoundaryKind::Insulated`] (impermeable
to flow and adiabatic to heat).

```rust
pub enum ConvectionBoundaryKind {
    Insulated,
    FixedTemperature(f64),
    Dirichlet {
        pressure: f64,
        temperature: f64,
    },
}
```

##### Variants

###### `Insulated`

Impermeable to flow **and** insulated (zero conductive heat flux, zero mass
flux). The natural default for a closed convection cell's sidewalls.

###### `FixedTemperature`

Impermeable to flow, but a **fixed temperature** `T_bc` (K): heat conducts
across the half-cell distance to the boundary while no mass crosses. This is
the isothermal top/bottom of the Horton–Rogers–Lapwood layer.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Dirichlet`

Open boundary: a fixed pressure (Pa) and temperature (K). Mass and heat may
both cross via a ghost state at the boundary-face centroid (advective +
conductive), the same two-point flux as an internal face.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | Prescribed pressure `p_bc` at the boundary, Pa. |
| `temperature` | `f64` | Prescribed temperature `T_bc` at the boundary, K. |

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
    fn clone(self: &Self) -> ConvectionBoundaryKind { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ConvectionBoundaryKind) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ConvectionBoundaryCondition`

A boundary condition applied to one of the six Cartesian box faces.

```rust
pub struct ConvectionBoundaryCondition {
    pub location: crate::grid::BoundaryLocation,
    pub kind: ConvectionBoundaryKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `location` | `crate::grid::BoundaryLocation` | Which exterior face of the logical box this applies to. |
| `kind` | `ConvectionBoundaryKind` | What is prescribed there. |

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
    fn clone(self: &Self) -> ConvectionBoundaryCondition { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ConvectionBoundaryCondition) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ThermalConvection`

A two-way (buoyancy) coupled thermal-hydraulic flow problem for a single
backward-Euler timestep, `nb = 2` (pressure, temperature).

Owns the grid (shared by [`Arc`]), the [`ConvectionParameters`], the timestep,
the previous-time-level state, and the per-face boundary conditions. Implements
[`BlockNonlinearSystem`] with `dof_per_cell = 2`, so it is driven by the generic
[`BlockNewtonSolver`]. The block Jacobian is assembled **numerically** (local
finite differences of the two residual equations over the two-point stencil).

State vectors are interleaved, length `2 * n_cells`: cell `c` occupies
`[2c] = p` (Pa) and `[2c + 1] = T` (K).

```rust
pub struct ThermalConvection {
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
  pub fn new(grid: Arc<CartesianGrid>, params: ConvectionParameters, boundary: Vec<ConvectionBoundaryCondition>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a coupled convection problem.

- ```rust
  pub fn set_timestep(self: &mut Self, dt: f64) { /* ... */ }
  ```
  Set the implicit-Euler timestep `dt` (s) for the next assembly.

- ```rust
  pub fn set_previous(self: &mut Self, state: &[f64]) { /* ... */ }
  ```
  Set the previous-time-level state (interleaved `[p0, T0, p1, T1, …]`, length

- ```rust
  pub fn grid(self: &Self) -> &CartesianGrid { /* ... */ }
  ```
  Read access to the underlying grid.

- ```rust
  pub fn params(self: &Self) -> &ConvectionParameters { /* ... */ }
  ```
  The bulk thermal + hydraulic parameters.

- ```rust
  pub fn total_energy(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Total sensible heat content of the domain (J) for an interleaved state `x`:

- ```rust
  pub fn peak_velocity(self: &Self, x: &[f64]) -> f64 { /* ... */ }
  ```
  Peak Darcy (specific-discharge) velocity magnitude over all cells (m/s) — a

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **BlockNonlinearSystem**
  - ```rust
    fn n_cells(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn dof_per_cell(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn ldu_addressing(self: &Self) -> (Vec<usize>, Vec<usize>) { /* ... */ }
    ```

  - ```rust
    fn assemble_residual(self: &mut Self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> { /* ... */ }
    ```

  - ```rust
    fn assemble_jacobian(self: &mut Self, x: &[f64], jac: &mut BlockLduMatrix) -> Result<(), PflotranError> { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `AdaptiveStepReport`

A transient buoyancy-coupled convection simulation driven by the block Newton
solver.

Outcome of one [`ThermalConvectionSimulation::step_adaptive`] call — how the
requested interval was covered by adaptive backward-Euler sub-steps.

Adaptive sub-stepping is the robustness mechanism for the strongly-convecting
regime: a large target step into a vigorously convecting state can leave the
block Newton solve without a basin of convergence, so on a failed solve the
sub-step is halved (down to `min_dt`) and retried, and after each accepted
sub-step the next attempt is grown back toward the target. The physics is
unchanged — only the path taken through time adapts.

```rust
pub struct AdaptiveStepReport {
    pub substeps: usize,
    pub cuts: usize,
    pub min_substep_dt: f64,
    pub max_newton_iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `substeps` | `usize` | Number of accepted backward-Euler sub-steps that tiled the interval. |
| `cuts` | `usize` | Number of timestep reductions (halvings) taken along the way. |
| `min_substep_dt` | `f64` | The smallest accepted sub-step (s) — a diagnostic of how hard the interval was. |
| `max_newton_iterations` | `usize` | The largest Newton iteration count over the accepted sub-steps. |

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
    fn clone(self: &Self) -> AdaptiveStepReport { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AdaptiveStepReport) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ThermalConvectionSimulation`

Holds the [`ThermalConvection`] problem, a [`BlockNewtonSolver`], the evolving
interleaved state (`[p, T]` per cell, length `2 * n_cells`), and the current
time. Each [`step`](Self::step) sets the timestep and previous state on the
problem, then solves the coupled `nb = 2` block nonlinear system.

```rust
pub struct ThermalConvectionSimulation {
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
  pub fn new(problem: ThermalConvection, config: BlockNewtonConfig, initial: Vec<f64>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Assemble a simulation from a problem, a block-Newton configuration, and an

- ```rust
  pub fn step(self: &mut Self, dt: f64) -> Result<BlockNewtonReport, PflotranError> { /* ... */ }
  ```
  Advance one backward-Euler step of size `dt` (s), returning the block Newton

- ```rust
  pub fn step_adaptive(self: &mut Self, dt: f64, min_dt: f64) -> Result<AdaptiveStepReport, PflotranError> { /* ... */ }
  ```
  Advance a target interval `dt` (s) by **adaptive backward-Euler sub-steps**,

- ```rust
  pub fn time(self: &Self) -> f64 { /* ... */ }
  ```
  Current simulation time (s).

- ```rust
  pub fn temperature(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Current temperature field `T` (K), de-interleaved, length `n_cells`.

- ```rust
  pub fn pressure(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Current pressure field `p` (Pa), de-interleaved, length `n_cells`.

- ```rust
  pub fn peak_velocity(self: &Self) -> f64 { /* ... */ }
  ```
  Peak Darcy velocity magnitude (m/s) at the current state — see

- ```rust
  pub fn total_energy(self: &Self) -> f64 { /* ... */ }
  ```
  Total sensible heat content (J) at the current state — see

- ```rust
  pub fn problem(self: &Self) -> &ThermalConvection { /* ... */ }
  ```
  The underlying problem (read-only), e.g. for grid or parameter access.

- ```rust
  pub fn state(self: &Self) -> &[f64] { /* ... */ }
  ```
  The current interleaved state vector `[p, T]` per cell (length

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `rayleigh_number`

Rayleigh number for a saturated porous layer,

$$ Ra = \frac{\rho_0\, g\, \beta\, \Delta T\, k\, H}{\mu\, \alpha_m}, \qquad \alpha_m = \frac{\lambda}{\rho_f c_f}, $$

where `\alpha_m` is the effective thermal diffusivity of the medium (using the
fluid heat capacity `\rho_f c_f = \rho_0 c_f`). `delta_t` is the top-to-bottom
temperature difference `\Delta T` (K) and `height` is the layer thickness `H`
(m). Onset of Horton–Rogers–Lapwood convection occurs at
[`CRITICAL_RAYLEIGH_NUMBER`] `= 4\pi^2`.

# Panics

Does not panic; if the parameters make `\alpha_m = 0` (impossible for a valid
[`ConvectionParameters`], whose conductivity and heat capacity are positive) the
result would be non-finite.

```rust
pub fn rayleigh_number(params: &ConvectionParameters, delta_t: f64, height: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `STANDARD_GRAVITY`

Standard gravitational acceleration (m/s^2) — the conventional default for a
convection run (gravity acting in `-z`, with `z` elevation-positive).

```rust
pub const STANDARD_GRAVITY: f64 = 9.806_65;
```

#### Constant `CRITICAL_RAYLEIGH_NUMBER`

The Horton–Rogers–Lapwood critical Rayleigh number `Ra_c = 4\pi^2 \approx
39.478` for onset of convection in a horizontal saturated porous layer heated
uniformly from below with impermeable, isothermal top and bottom boundaries.

Below `Ra_c` the motionless conductive state is linearly stable (perturbations
decay); above it the conductive state is unstable and convection cells grow.
Reference: Lapwood (1948); Nield & Bejan, *Convection in Porous Media*.

```rust
pub const CRITICAL_RAYLEIGH_NUMBER: f64 = _;
```

## Module `transport`

Conservative (non-reactive) solute transport (v1) — bead op-v6s.11.

Advection–diffusion–dispersion of a **passive scalar** — a single solute
concentration `c` (mass of solute per unit fluid volume, kg/m^3) — on the
structured Cartesian grid ([`crate::grid::CartesianGrid`]), advected by a
frozen flow field from a RICHARDS solve and spread by molecular diffusion
plus velocity-driven longitudinal dispersion. No chemistry: the solute does
not react, sorb, decay, or feed back on the flow. Reactive geochemistry is
deferred to bead op-v6s.12.

# Governing equation

For volumetric water content `theta_w = phi * S_l` (dimensionless), Darcy
volumetric face flux `q` (m^3/s), and effective diffusion/dispersion
coefficient `D` (m^2/s):

```text
d/dt(theta_w * c) + div(q * c) - div(theta_w * D * grad c) = 0
```

With `theta_w`, `q`, and `D` frozen from the flow solve, this is **linear**
in `c`. One backward-Euler step therefore assembles a single linear system
`A c = b` and solves it once with the foam-basic-lib Krylov backend
(BiCGStab + ILU(0)) — no Newton iteration.

# Discretisation

Cell-centred finite volume, implicit (backward) Euler in time:

- **Accumulation** `theta_w_i V_i / dt (c_i - c_i^old)` — diagonal mass term.
- **Advection** — first-order **upwind**: the face concentration is taken from
  the upstream cell. Upwind is unconditionally monotone (the assembled matrix
  is an M-matrix), so a passive scalar stays within its physical bounds with
  no over/undershoot, at the cost of numerical (cross-wind) diffusion of order
  `|v| dx / 2`. A TVD / higher-order scheme is deferred.
- **Dispersion** — symmetric two-point flux using the grid's geometric
  transmissibility `area / distance`, with a face-averaged water content and a
  velocity-dependent coefficient `D_face = D_mol + alpha_L |v_darcy|` (see
  [`DispersionModel`]).

# Boundary conditions

- **Default (no BC on a face location):** advective outflow only, with a zero
  dispersive gradient. An unspecified inflow face uses the interior
  concentration (a zero-gradient inflow) — document this if it matters for a
  case.
- **[`TransportBoundaryKind::InflowConcentration`]:** a fixed boundary
  concentration `c_bc`. The advective part is upwinded (inflow carries `c_bc`
  into the domain; outflow carries the interior concentration out) and the
  dispersive part always couples the near-boundary cell to `c_bc` across the
  half-cell distance. Applying the dispersive coupling on **both** inflow and
  outflow makes this a genuine Dirichlet concentration usable at either end of
  a domain, which the analytical verification tests (steady advection–
  diffusion and pure-diffusion linear profile) rely on. See the note in
  [`SoluteTransport::step`].

# Units

Concentration `c` is kg/m^3; volumetric fluxes are m^3/s; water content is
dimensionless; `D` is m^2/s; masses are kg; time steps are seconds. The API
uses plain `f64` (not `uom`) because the solute field and its Krylov solve mix
quantities of differing dimension; callers apply units at the case-setup layer.

```rust
pub mod transport { /* ... */ }
```

### Types

#### Struct `FlowField`

A frozen flow field feeding the solute-transport step, as produced by a flow
(RICHARDS) solve.

Face indexing matches the grid exactly: `face_flux[f]` corresponds to
`grid.connections()[f]` and to face `f` of `grid.ldu_addressing()`;
`boundary_flux[b]` corresponds to `grid.boundary_faces()[b]`;
`water_content[i]` is cell `i`. All volumetric fluxes are in m^3/s.

The three vectors must have lengths `grid.connections().len()`,
`grid.boundary_faces().len()`, and `grid.n_cells()` respectively;
[`SoluteTransport::new`] validates this.

```rust
pub struct FlowField {
    pub face_flux: Vec<f64>,
    pub boundary_flux: Vec<f64>,
    pub water_content: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `face_flux` | `Vec<f64>` | Signed Darcy volumetric flux per **internal** face, m^3/s. Positive means<br>flow from `owner` to `neighbour` (i.e. in the `+x`/`+y`/`+z` face<br>direction). Length `grid.connections().len()`. |
| `boundary_flux` | `Vec<f64>` | Signed Darcy volumetric flux per **boundary** face, m^3/s. Positive means<br>**outflow** (leaving the domain); negative means inflow. Length<br>`grid.boundary_faces().len()`. |
| `water_content` | `Vec<f64>` | Volumetric water content `theta_w = phi * S_l` per cell, dimensionless in<br>`[0, 1]`. Length `grid.n_cells()`. Frozen from the flow solve. |

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `DispersionModel`

Effective diffusion/dispersion model: `D_face = molecular_diffusion +
longitudinal_dispersivity * |v_darcy|`.

`v_darcy` is the Darcy velocity magnitude across a face (`|q| / area`, m/s),
so the second term is mechanical (velocity-driven) longitudinal dispersion and
the first is velocity-independent molecular diffusion. Transverse dispersion
is not modelled in v1.

```rust
pub struct DispersionModel {
    pub molecular_diffusion: f64,
    pub longitudinal_dispersivity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `molecular_diffusion` | `f64` | Molecular diffusion coefficient, m^2/s. Must be finite and `>= 0`. |
| `longitudinal_dispersivity` | `f64` | Longitudinal dispersivity `alpha_L`, m. Must be finite and `>= 0`.<br>Multiplies the Darcy velocity magnitude to give the dispersive part of the<br>face coefficient. |

##### Implementations

###### Methods

- ```rust
  pub fn new(molecular_diffusion: f64, longitudinal_dispersivity: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Construct a dispersion model from a molecular diffusion coefficient

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `TransportBoundaryKind`

The kind of solute boundary condition applied at a face location.

Enum dispatch (no trait objects), per the workspace design rules. Extended by
adding variants; every `match` on it is then checked for exhaustiveness.

```rust
pub enum TransportBoundaryKind {
    InflowConcentration(f64),
}
```

##### Variants

###### `InflowConcentration`

A fixed boundary solute concentration `c_bc` (kg/m^3). Advection is
upwinded by the boundary flux sign; the dispersive flux always couples the
near-boundary cell to `c_bc` across the half-cell distance, so this acts as
a Dirichlet concentration at either an inflow or an outflow face.

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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TransportBoundaryCondition`

A solute boundary condition bound to one of the six domain-box face locations.

```rust
pub struct TransportBoundaryCondition {
    pub location: crate::grid::BoundaryLocation,
    pub kind: TransportBoundaryKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `location` | `crate::grid::BoundaryLocation` | Which exterior face location this condition applies to. |
| `kind` | `TransportBoundaryKind` | The condition to impose there. |

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SoluteTransport`

One implicit-Euler solute-transport step over a frozen flow field.

Holds the grid, the frozen [`FlowField`], the [`DispersionModel`], the solute
boundary conditions, the time step `dt` (s), and the previous-time
concentration `c_old` (kg/m^3). [`SoluteTransport::step`] assembles and solves
the linear system for the next-time concentration; repeated stepping (with
[`SoluteTransport::set_previous`] between steps, or by feeding the returned
field back) advances the solute in time or drives it to steady state.

```rust
pub struct SoluteTransport {
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
  pub fn new(grid: CartesianGrid, flow: FlowField, dispersion: DispersionModel, boundary: Vec<TransportBoundaryCondition>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build a solute-transport stepper from a grid, a frozen flow field, a

- ```rust
  pub fn set_flux_limiter(self: &mut Self, limiter: FluxLimiter) { /* ... */ }
  ```
  Select the advection scheme. [`FluxLimiter::Upwind`] (default) is

- ```rust
  pub fn set_decay_constant(self: &mut Self, lambda: f64) -> Result<(), PflotranError> { /* ... */ }
  ```
  Set the first-order radioactive-decay constant `lambda` (1/s) of the

- ```rust
  pub fn set_linear_sorption(self: &mut Self, isotherm: crate::sorption::SorptionIsotherm, bulk_density: f64) -> Result<(), PflotranError> { /* ... */ }
  ```
  Apply **linear** equilibrium sorption (constant retardation) to the solute,

- ```rust
  pub fn set_timestep(self: &mut Self, dt: f64) { /* ... */ }
  ```
  Set the time step `dt` (seconds). Validated (must be positive and finite)

- ```rust
  pub fn set_previous(self: &mut Self, c_old: &[f64]) { /* ... */ }
  ```
  Set the previous-time concentration `c_old` (kg/m^3), one value per cell.

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of grid cells (the size of the linear system).

- ```rust
  pub fn step(self: &mut Self, c: &mut Vec<f64>) -> Result<KrylovResult, PflotranError> { /* ... */ }
  ```
  Assemble and solve `A c = b` for the next-time concentration, writing the

- ```rust
  pub fn total_mass(self: &Self, c: &[f64]) -> f64 { /* ... */ }
  ```
  Total solute mass `sum_i V_i * theta_w_i * c_i` (kg).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `units`

Named `uom` type aliases for the physical quantities used across the crate.

The workspace "human interface layer" rule requires that a developer hovering
a symbol in rust-analyzer sees a *meaningful* name — `Permeability`, not a
raw `Quantity<ISQ<...>, SI<f64>, f64>`. These aliases give each subsurface
quantity a name and spell out its physical meaning and SI unit, even though
`uom` already enforces dimensions at compile time.

All aliases are `f64`-backed `uom::si::f64` quantities and are `Copy`.

## Note on dimensionless quantities

Saturation and porosity are physically dimensionless fractions in `[0, 1]`.
They alias `uom`'s [`Ratio`], which carries no unit; the valid-range
constraint is documented here and must be enforced by the code that consumes
them (a future validated newtype may wrap them — bead op-v6s.7).

```rust
pub mod units { /* ... */ }
```

### Types

#### Type Alias `FluidPressure`

Fluid (or phase) pressure. SI unit: pascal (Pa).

In RICHARDS flow the primary unknown is liquid pressure; capillary pressure
`p_c = p_gas - p_liq` sets the saturation via the characteristic curves.

```rust
pub type FluidPressure = uom::si::f64::Pressure;
```

#### Type Alias `CapillaryPressure`

Capillary pressure `p_c = p_nonwetting - p_wetting`. SI unit: pascal (Pa).

Non-negative for a wetting liquid; drives the retention (characteristic)
curve `S_l(p_c)`.

```rust
pub type CapillaryPressure = uom::si::f64::Pressure;
```

#### Type Alias `Saturation`

Liquid saturation `S_l` — the fraction of pore volume occupied by the liquid
phase. Dimensionless, valid range `[0, 1]` (aliases [`Ratio`]).

```rust
pub type Saturation = uom::si::f64::Ratio;
```

#### Type Alias `Porosity`

Porosity `phi` — the fraction of bulk volume that is pore space.
Dimensionless, valid range `[0, 1]` (aliases [`Ratio`]).

```rust
pub type Porosity = uom::si::f64::Ratio;
```

#### Type Alias `RelativePermeability`

Relative permeability `k_r` — the saturation-dependent factor in `[0, 1]`
scaling intrinsic permeability for a given phase. Dimensionless
(aliases [`Ratio`]).

```rust
pub type RelativePermeability = uom::si::f64::Ratio;
```

#### Type Alias `Permeability`

Intrinsic (absolute) permeability `k` of the porous medium. SI unit: square
metre (m^2). (1 darcy is approximately 9.869233e-13 m^2.)

Aliases [`Area`] because permeability has dimensions of length squared.

```rust
pub type Permeability = uom::si::f64::Area;
```

#### Type Alias `DarcyVelocity`

Darcy velocity (specific discharge) `q` — volumetric flux per unit bulk
cross-sectional area. SI unit: metre per second (m/s). Aliases [`Velocity`].

```rust
pub type DarcyVelocity = uom::si::f64::Velocity;
```

#### Type Alias `FluidDensity`

Fluid mass density `rho`. SI unit: kilogram per cubic metre (kg/m^3).

```rust
pub type FluidDensity = uom::si::f64::MassDensity;
```

#### Type Alias `FluidViscosity`

Fluid dynamic viscosity `mu`. SI unit: pascal-second (Pa.s).

```rust
pub type FluidViscosity = uom::si::f64::DynamicViscosity;
```

#### Type Alias `FluidTemperature`

Fluid temperature `T`. SI unit: kelvin (K). Used by the (planned) TH flow
mode and by temperature-dependent property correlations.

```rust
pub type FluidTemperature = uom::si::f64::ThermodynamicTemperature;
```

#### Type Alias `GeoLength`

A geometric length (cell size, depth, elevation). SI unit: metre (m).

```rust
pub type GeoLength = uom::si::f64::Length;
```

## Module `unstructured`

Unstructured polyhedral finite-volume grid with explicit connectivity
(bead op-v6s.15.8).

The [`crate::grid`] module provides a structured Cartesian mesh only. This
module is its unstructured counterpart: a grid of **arbitrary polyhedral
cells** whose topology is given explicitly as a list of faces, rather than
implied by a logical `(i, j, k)` indexing. It mirrors PFLOTRAN's real
implicit-unstructured (`UGRID`) discretization, where each cell carries a
centroid and volume and each face records the two cells (or one cell + the
exterior) it separates plus the geometry a flux stencil needs.

# What this layer provides — and what it does not

This is the **connectivity + geometry** layer only, plus the classic
**two-point flux approximation (TPFA)** geometric transmissibility. It does
**not** provide:

- mesh-file readers (Exodus/HDF5/PFLOTRAN `.ugi`/`.h5`) — cells and faces
  must be supplied explicitly to [`UnstructuredGrid::from_faces`];
- multi-point flux (MPFA) or any consistent scheme for non-K-orthogonal
  meshes — TPFA only (see the caveat below);
- the physics (mobility, permeability, boundary conditions) — the stored
  transmissibility is a *purely geometric* factor.

# Two-point flux approximation (TPFA) transmissibility

For an internal face of area `A` shared by cells `a` and `b` with centroids
`c_a`, `c_b`, unit face normal `n`, this module stores the geometric
transmissibility

```text
  T_geom = A * |n · (c_b - c_a)| / |c_b - c_a|^2       [m]
```

(`transmissibility_geom` on [`UnstructuredConnection`]). This is the standard
TPFA form: it projects the cell-to-cell vector onto the face normal, so it
reduces exactly to the familiar `A / d` (with `d = |c_b - c_a|`) when the
connection is **K-orthogonal** — i.e. the line joining the two centroids is
parallel to the face normal, as on a Cartesian or a Voronoi (PEBI) mesh. The
absolute value makes the result independent of whether `n` was supplied
pointing from `a` to `b` or the reverse; a well-posed TPFA transmissibility
is always positive. Multiply `T_geom` by phase mobility (`k_r / mu`) and
intrinsic permeability (m^2) in the physics layer to obtain a true flux
coefficient.

## K-orthogonality caveat (a real limitation, not a bug)

TPFA is **only consistent** (convergent to the true solution) when the mesh
is K-orthogonal with respect to the permeability tensor. On a distorted mesh,
or with a full/anisotropic permeability tensor whose principal axes are not
aligned with the cell-connection directions, TPFA introduces an `O(1)`
discretization error that does **not** vanish under refinement — the scheme
is inconsistent there. Handling those cases needs a multi-point flux (MPFA)
or mimetic/mixed scheme, which is out of scope for this module. See
Aziz & Settari, *Petroleum Reservoir Simulation* (1979); Eymard, Gallouët &
Herbin, *Finite Volume Methods* (Handbook of Numerical Analysis, 2000);
and LeVeque, *Finite Volume Methods for Hyperbolic Problems* (2002).

# Units

Lengths are metres (m), areas m^2, volumes m^3. `transmissibility_geom` has
units of metres (m), being `area / distance`.

# Relationship to the structured grid

Field names and accessors deliberately track [`crate::grid`]: an
[`UnstructuredConnection`] plays the role of [`crate::grid::Connection`]
(two cell ids + area + geometric transmissibility), an
[`UnstructuredBoundaryFace`] the role of [`crate::grid::BoundaryFace`], and
[`UnstructuredGrid`] exposes `n_cells` / `cell_volume` / `connections` /
`boundary_faces` / `ldu_addressing` with the same meanings. This keeps the
two grids unify-able later, while remaining a separate self-contained type.
[`UnstructuredGrid::uniform_column`] builds a 1-D column that reproduces the
structured grid's geometry exactly, so an unstructured result can be
cross-checked cell-for-cell against the Cartesian one.

# Provenance

PFLOTRAN implicit-unstructured (`UGRID`) discretization + the standard TPFA
two-point flux stencil. **Untrusted AI-generated draft** per the workspace
`RESPONSIBLE_USE.md`: geometry/connectivity + TPFA only, no MPFA and no mesh
readers yet. No human V&V has been performed; not for facility operation,
reactor control, or licensing.

```rust
pub mod unstructured { /* ... */ }
```

### Types

#### Struct `CellId`

An index into a grid's cell array.

Topology links are stored as plain `usize` cell indices (never Rust
references), so the grid needs no lifetime parameters. This newtype is the
typed spelling of such an index for public signatures that want to be
explicit that a `usize` names a cell.

```rust
pub struct CellId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Methods

- ```rust
  pub fn index(self: Self) -> usize { /* ... */ }
  ```
  The underlying array index of this cell.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> CellId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

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

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &CellId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CellId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &CellId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `UnstructuredConnection`

A single internal connection between two unstructured cells, carrying the
geometry a two-point flux approximation (TPFA) needs.

The face is shared by exactly the two distinct cells `cell_a` and `cell_b`
(both array indices). `area` is the shared face area (m^2), and
`transmissibility_geom` is the purely geometric TPFA factor
`A * |n · (c_b - c_a)| / |c_b - c_a|^2` in metres (m) — mobility- and
permeability-free. See the module docs for the formula and its
K-orthogonality limitation.

```rust
pub struct UnstructuredConnection {
    pub cell_a: usize,
    pub cell_b: usize,
    pub area: f64,
    pub transmissibility_geom: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell_a` | `usize` | Array index of the first cell of the connection. |
| `cell_b` | `usize` | Array index of the second cell of the connection (distinct from<br>`cell_a`). |
| `area` | `f64` | Shared face area, m^2. |
| `transmissibility_geom` | `f64` | Purely geometric TPFA transmissibility factor `A / d`, in metres (m):<br>`area * |n · (c_b - c_a)| / |c_b - c_a|^2`. Multiply by mobility<br>(`k_r / mu`) and intrinsic permeability (m^2) in the physics layer. |

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
    fn clone(self: &Self) -> UnstructuredConnection { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &UnstructuredConnection) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `UnstructuredBoundaryFace`

A boundary face of a single unstructured cell — a face touching the domain
exterior — for applying Dirichlet or Neumann boundary conditions.

It has exactly one owner cell (`cell`, an array index), an area (m^2), an
outward unit `normal`, and a face `centroid` (`[x, y, z]`, metres).

```rust
pub struct UnstructuredBoundaryFace {
    pub cell: usize,
    pub area: f64,
    pub normal: [f64; 3],
    pub centroid: [f64; 3],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell` | `usize` | Array index of the single cell owning this exterior face. |
| `area` | `f64` | Face area, m^2. |
| `normal` | `[f64; 3]` | Outward unit normal `[nx, ny, nz]` (normalised at construction). |
| `centroid` | `[f64; 3]` | Face centroid `[x, y, z]`, metres. |

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
    fn clone(self: &Self) -> UnstructuredBoundaryFace { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &UnstructuredBoundaryFace) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `UnstructuredGrid`

An unstructured polyhedral finite-volume grid with explicit connectivity.

Each cell has a centroid (`[x, y, z]`, metres) and a volume (m^3). Internal
[`UnstructuredConnection`]s and exterior [`UnstructuredBoundaryFace`]s are
precomputed at construction from an explicit face list and returned by
reference. Unlike [`crate::grid::CartesianGrid`], there is no logical
`(i, j, k)` structure — topology is whatever the supplied faces describe.

```rust
pub struct UnstructuredGrid {
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
  pub fn from_faces(centroids: Vec<[f64; 3]>, volumes: Vec<f64>, faces: Vec<(usize, Option<usize>, f64, [f64; 3], [f64; 3])>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build an unstructured grid from explicit cell geometry and a face list.

- ```rust
  pub fn uniform_column(n: usize, length: f64, area: f64) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Build an unstructured grid reproducing a uniform 1-D column of `n` cells

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Total number of cells.

- ```rust
  pub fn cell_volume(self: &Self, cell: usize) -> f64 { /* ... */ }
  ```
  Volume of a cell, m^3.

- ```rust
  pub fn cell_centroid(self: &Self, cell: usize) -> [f64; 3] { /* ... */ }
  ```
  Centroid of a cell as `[x, y, z]` in metres.

- ```rust
  pub fn connections(self: &Self) -> &[UnstructuredConnection] { /* ... */ }
  ```
  All internal cell-to-cell connections, in the order the internal faces

- ```rust
  pub fn boundary_faces(self: &Self) -> &[UnstructuredBoundaryFace] { /* ... */ }
  ```
  All exterior boundary faces, in the order they were supplied to

- ```rust
  pub fn ldu_addressing(self: &Self) -> (Vec<usize>, Vec<usize>) { /* ... */ }
  ```
  The `(lower, upper)` index arrays for assembling an LDU / sparse matrix

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> UnstructuredGrid { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `wells`

Well models and advanced source/sink + boundary conditions (bead op-v6s.15.12).

This module supplies the parts of a subsurface-flow model that inject or
withdraw fluid at points/columns inside the domain (**wells** and
**source/sink** terms) and the pressure/flux conditions imposed on its
exterior (**advanced boundary conditions**). All quantities are SI and the
sign convention throughout is **positive = fluid entering the cell**
(injection), **negative = fluid leaving** (production/outflow).

## Peaceman vertical-well model

A well whose radius `r_w` (typically centimetres) is far smaller than a grid
cell (metres) cannot be resolved by the pressure the cell carries — the
numerical cell pressure is a *volume average*, not the pressure at the well
sandface. Peaceman (1978, 1983) showed that for a vertical well centred in a
Cartesian cell the steady radial-flow solution recovers the correct rate if
the cell pressure is interpreted as the pressure at an **equivalent radius**
`r_eq`, and the well is coupled to the cell through a **well index** `WI`:

```text
    WI = 2*pi * k * h / ( ln(r_eq / r_w) + skin )
```

- `k`  — cell permeability, m^2 (isotropic form here; see review flags).
- `h`  — perforated thickness of the cell, m (the cell's `dz` for a vertical
         well fully perforating the layer).
- `r_w`— wellbore radius, m.
- `skin` — dimensionless near-well damage/stimulation factor (`+` damage
         reduces `WI`, `-` stimulation increases it).

The **isotropic** equivalent radius (used by [`PeacemanWell::well_index`]) is

```text
    r_eq = 0.14 * sqrt(dx^2 + dy^2)
```

which is the isotropic special case (`kx = ky`) of Peaceman's anisotropic
formula

```text
    r_eq = 0.28 * sqrt( sqrt(ky/kx)*dx^2 + sqrt(kx/ky)*dy^2 )
                 / ( (ky/kx)^0.25 + (kx/ky)^0.25 ).
```

The volumetric flow the well drives into a perforated cell is

```text
    q_well = WI * lambda * (p_bh - p_cell)          [m^3/s]
```

where `lambda = k_r / mu` is the fluid **mobility** (relative permeability
over dynamic viscosity, units 1/(Pa*s)) and `p_bh` is the wellbore
**bottom-hole pressure** (Pa). With this sign, `p_bh > p_cell` gives `q > 0`
(injection) and `p_bh < p_cell` gives `q < 0` (production). A
**rate-controlled** well instead distributes a prescribed total volumetric
rate across its perforated cells in proportion to each cell's `WI` (the
larger the connection, the larger its share).

## Advanced boundary conditions

[`AdvancedBc`] collects three state/space/time-dependent conditions common to
PFLOTRAN `FLOW_CONDITION` cards:

- **Hydrostatic** — a Dirichlet pressure varying with elevation about a
  datum, `p(z) = p_datum + rho*g*(z_datum - z)`; pressure increases downward.
- **Seepage face** — a *state-dependent switch*: the face conducts (outflow
  permitted) only where the cell pressure exceeds atmospheric; otherwise it is
  a no-flow boundary. See [`AdvancedBc::value`] for the exact encoding.
- **Time-varying** — a tabulated `value(t)` with linear interpolation and
  flat clamping outside the table range (a piecewise-linear forcing).

## Units summary

| Symbol      | Meaning                          | Unit        |
|-------------|----------------------------------|-------------|
| `k`         | permeability                     | m^2         |
| `dx,dy,dz`  | cell dimensions                  | m           |
| `h`         | perforated thickness             | m           |
| `r_w`       | wellbore radius                  | m           |
| `r_eq`      | Peaceman equivalent radius       | m           |
| `skin`      | near-well skin factor            | dimensionless |
| `WI`        | well index                       | m^3 (i.e. m^2 * m) |
| `lambda`    | fluid mobility `k_r/mu`          | 1/(Pa*s)    |
| `p_bh`      | bottom-hole pressure             | Pa          |
| `p_cell`    | cell (grid-block) pressure       | Pa          |
| `q`         | volumetric source/sink rate      | m^3/s       |
| `rho`       | fluid density                    | kg/m^3      |
| `g`         | gravitational acceleration       | m/s^2       |

## Design (workspace mandate)

Enum dispatch, no trait objects, no `Box`, no lifetime parameters, pure
Rust with no external dependencies (Android-clean). Cell geometry is read
from [`crate::grid::CartesianGrid`] through its public accessors only.

## Provenance

- Peaceman, D. W. (1978), "Interpretation of Well-Block Pressures in
  Numerical Reservoir Simulation", *SPE Journal* **18**(3), 183–194.
- Peaceman, D. W. (1983), "Interpretation of Well-Block Pressures in
  Numerical Reservoir Simulation With Nonsquare Grid Blocks and Anisotropic
  Permeability", *SPE Journal* **23**(3), 531–543.
- Hydrostatic and seepage-face flow conditions follow the standard PFLOTRAN
  `FLOW_CONDITION` formulation (PFLOTRAN theory/user guide,
  documentation.pflotran.org).

## Human-review flags (untrusted AI draft, no human V&V)

- **Single-phase.** Mobility `lambda` is a single scalar; there is no
  multiphase relative-permeability upwinding.
- **Vertical well only.** The `WI` uses `h = dz` and the vertical-well
  equivalent radius; horizontal/deviated wells are not modelled.
- **Isotropic-k well index.** [`PeacemanWell::well_index`] uses the
  `r_eq = 0.14*sqrt(dx^2+dy^2)` isotropic form; anisotropic `kx != ky`
  correction is documented above but not implemented.

```rust
pub mod wells { /* ... */ }
```

### Types

#### Enum `WellControl`

How a [`PeacemanWell`] is driven each timestep.

Both variants use the module sign convention: positive rate / positive
pressure difference means fluid **entering** the reservoir (injection).

```rust
pub enum WellControl {
    BottomHolePressure(f64),
    VolumetricRate(f64),
}
```

##### Variants

###### `BottomHolePressure`

Fixed bottom-hole (sandface) pressure `p_bh`, Pa. Each perforated cell's
rate is then `WI * lambda * (p_bh - p_cell)`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `VolumetricRate`

Fixed **total** volumetric rate across all perforations, m^3/s. Positive
injects, negative produces. The total is split among perforated cells in
proportion to their well index `WI`.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> WellControl { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &WellControl) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `PeacemanWell`

A Peaceman vertical well perforating one or more Cartesian cells.

The well couples a single wellbore to every cell in [`PeacemanWell::cells`]
through that cell's well index (see the module docs). Physical assumptions:
the well is **vertical**, **single-phase**, and each perforated cell uses the
**isotropic-k** equivalent radius `r_eq = 0.14*sqrt(dx^2 + dy^2)` with
perforated thickness `h = dz`.

Prefer [`PeacemanWell::new`] to construct one — it validates the geometry —
though the fields are public for inspection.

```rust
pub struct PeacemanWell {
    pub cells: Vec<usize>,
    pub radius: f64,
    pub skin: f64,
    pub control: WellControl,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cells` | `Vec<usize>` | Linear indices (into the grid) of the perforated cells. Must be non-empty<br>and each index valid for the grid it is evaluated against. Indices should<br>be distinct — a repeated cell would be counted twice in rate weighting. |
| `radius` | `f64` | Wellbore radius `r_w`, metres. Must be strictly positive and smaller than<br>the cell equivalent radius `r_eq` (otherwise `ln(r_eq/r_w) + skin <= 0`<br>and the well index is unphysical). |
| `skin` | `f64` | Dimensionless near-well skin factor. Positive = formation damage (lower<br>`WI`); negative = stimulation (higher `WI`). Must be finite. |
| `control` | `WellControl` | The well's operating control (fixed BHP or fixed total rate). |

##### Implementations

###### Methods

- ```rust
  pub fn new(cells: Vec<usize>, radius: f64, skin: f64, control: WellControl) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Construct and validate a Peaceman well.

- ```rust
  pub fn well_index(dx: f64, dy: f64, dz: f64, permeability: f64, r_w: f64, skin: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  Isotropic-k Peaceman well index for a single cell, m^3.

- ```rust
  pub fn cell_rates(self: &Self, grid: &CartesianGrid, permeability: f64, mobility: f64, cell_pressure: &[f64]) -> Result<Vec<f64>, PflotranError> { /* ... */ }
  ```
  Per-cell volumetric source/sink rates (m^3/s) this well imposes on the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PeacemanWell { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PeacemanWell) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `SourceSink`

A single per-cell source/sink term: a fixed volumetric or mass rate applied
to one grid cell.

Distinct from [`PeacemanWell`], which derives its rate from a pressure
difference and a well index; a `SourceSink` is a directly prescribed rate
(e.g. a recharge flux or a tracer injection). Sign convention: positive =
mass/fluid **entering** the cell.

```rust
pub enum SourceSink {
    Volumetric {
        cell: usize,
        rate: f64,
    },
    Mass {
        cell: usize,
        rate: f64,
    },
}
```

##### Variants

###### `Volumetric`

A prescribed **volumetric** rate at a cell, m^3/s (positive = injection).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `cell` | `usize` | Linear cell index the rate is applied to. |
| `rate` | `f64` | Volumetric rate, m^3/s. |

###### `Mass`

A prescribed **mass** rate at a cell, kg/s (positive = injection).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `cell` | `usize` | Linear cell index the rate is applied to. |
| `rate` | `f64` | Mass rate, kg/s. |

##### Implementations

###### Methods

- ```rust
  pub fn cell(self: &Self) -> usize { /* ... */ }
  ```
  The cell this term is applied to.

- ```rust
  pub fn volumetric_rate(self: &Self, density: f64) -> Result<f64, PflotranError> { /* ... */ }
  ```
  The **volumetric** rate (m^3/s) of this term, converting a mass rate with

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SourceSink { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SourceSink) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `AdvancedBc`

Advanced (state/space/time-dependent) boundary-condition value types.

Each variant is evaluated through [`AdvancedBc::value`], which returns the
boundary pressure (Pa) or forcing value to impose at a given elevation,
current cell pressure, and time. See the per-variant docs for the physics
and the seepage-face encoding.

```rust
pub enum AdvancedBc {
    Hydrostatic {
        pressure_datum: f64,
        z_datum: f64,
        density: f64,
        gravity: f64,
    },
    SeepageFace {
        atmospheric_pressure: f64,
    },
    TimeVarying {
        table: Vec<(f64, f64)>,
    },
}
```

##### Variants

###### `Hydrostatic`

Hydrostatic Dirichlet pressure about a datum:
`p(z) = pressure_datum + density*gravity*(z_datum - z)`.

Pressure increases downward (decreasing `z`). `pressure_datum` is the
pressure (Pa) at elevation `z_datum` (m); `density` is the fluid density
(kg/m^3) and `gravity` the gravitational acceleration (m/s^2, a positive
magnitude).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure_datum` | `f64` | Pressure at the datum elevation, Pa. |
| `z_datum` | `f64` | Datum elevation `z_datum`, m. |
| `density` | `f64` | Fluid density `rho`, kg/m^3. |
| `gravity` | `f64` | Gravitational acceleration `g`, m/s^2 (positive magnitude). |

###### `SeepageFace`

Seepage-face condition: outflow is permitted only where the cell pressure
exceeds `atmospheric_pressure`; elsewhere the face is a no-flow boundary.
A state-dependent switch — see [`AdvancedBc::value`] for the encoding.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `atmospheric_pressure` | `f64` | Atmospheric (reference) pressure `p_atm`, Pa. |

###### `TimeVarying`

Time-varying forcing given as a table of `(time_s, value)` pairs, with
linear interpolation between successive points and flat clamping outside
the tabulated time range. The table must be sorted by ascending time; an
empty table evaluates to `0.0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `table` | `Vec<(f64, f64)>` | `(time_s, value)` samples, sorted by ascending `time_s`. |

##### Implementations

###### Methods

- ```rust
  pub fn time_varying(table: Vec<(f64, f64)>) -> Result<Self, PflotranError> { /* ... */ }
  ```
  Construct and validate a [`AdvancedBc::TimeVarying`] table.

- ```rust
  pub fn value(self: &Self, z: f64, cell_pressure: f64, time: f64) -> f64 { /* ... */ }
  ```
  Boundary pressure / forcing value at elevation `z` (m), current

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> AdvancedBc { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AdvancedBc) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Types

### Type Alias `Result`

Convenience `Result` alias for the crate's fallible operations.

The error variant is always [`PflotranError`]; scaffold entry points that
are not implemented yet return [`PflotranError::NotImplemented`] rather than
panicking or returning a fabricated value.

```rust
pub type Result<T> = core::result::Result<T, PflotranError>;
```

## Re-exports

### Re-export `PflotranError`

```rust
pub use error::PflotranError;
```

### Re-export `FlowMode`

```rust
pub use flow::FlowMode;
```

### Re-export `RichardsSimulation`

```rust
pub use flow::RichardsSimulation;
```

