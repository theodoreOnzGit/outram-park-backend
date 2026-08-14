# Crate Documentation

**Version:** 0.0.1

**Format Version:** 60

# Module `tampines`

# TAMPINES

**T**hermal-hydraulic **A**rtificial-intelligence **M**ulti-**P**hase
**IN**tegrated **E**mulator **S**ystem.

TAMPINES is the **central thermal-hydraulic framework** of the OUTRAM PARK
suite. It owns all fluid flow, thermal-hydraulics, thermophysical
properties, heat transfer, balance-of-plant components, humid-air
psychrometrics, and multiphase thermal-hydraulics. It is distinct from
[`tampines_steam_tables`], which is only the IAPWS-IF97 property library
(one of the backends TAMPINES composes).

## What it composes

| Piece | Provided by | Role |
|---|---|---|
| Single-phase liquid thermal-hydraulics | [`tuas_boussinesq_solver`] | Boussinesq single-phase pipe/component flow |
| Compressible / two-phase properties | [`outram_park_fork_coolprop`] | CoolProp-derived thermophysical properties |
| IAPWS-IF97 steam/water properties | [`tampines_steam_tables`] | Steam-turbine and choked-flow equations |
| Finite-volume building blocks | [`outram_foam_basic_lib`] | Tensor algebra, ODE/polynomial solvers, FV operators |
| Process control | [`chem_eng_real_time_process_control_simulator`] | PID / transfer-function control loops |
| Equipment-model correlations | [`outram_park_fork_dwsim_libs`] | Pipe/valve/heat-exchanger/expander/pump sizing & rating equations |

## What belongs here / what does not

- **Belongs here:** fluid-flow and thermal-hydraulic component models
  (pipes, pumps, valves, heat exchangers, steam generators, turbines,
  condensers, cooling towers), balance-of-plant composition, humid-air
  psychrometrics, and multiphase thermal-hydraulics (HEM, drift-flux, CHF).
- **Does NOT belong here:** raw property-table equations (those live in
  `tampines-steam-tables` / `outram-park-fork-coolprop`), reactor physics
  (`teh-o-prke`, `outram-mc-libs`, `njoy-outram-park-fork`), or GUI /
  visualization code (`outram-park-digital-twin-gui`).

## Status

**Scaffold only.** This crate is being built out incrementally; see the
`op-dt3` epic in the workspace's beads issue tracker for the live module
plan and progress.

## Modules

## Module `balance_of_plant`

Balance-of-plant grouping.

Re-exports the BOP-relevant [`crate::components`] types under one module
for discoverability, plus system-level (multi-component) assembly types
like [`RankineCycle`]. This module adds no new leaf physics -- it is
composition only; each component's own (currently stubbed) method is
still where its physics lives.

[`crate::components::Pipe`] and [`crate::components::CoolingTower`] are
not re-exported here: `Pipe` is general-purpose (not BOP-specific), and
cooling towers have their own dedicated grouping, [`crate::cooling_tower`].

```rust
pub mod balance_of_plant { /* ... */ }
```

### Types

#### Struct `RankineCycle`

A basic Rankine-cycle assembly: steam generator, turbine, condenser, and
feedwater pump in series.

Grouping/composition only -- no whole-cycle calculation logic yet. Each
component's own method (e.g. [`Turbine::expand_to`], [`Condenser::condense`])
is still the entry point for that component's physics; a cycle-level
`run`/`step` that threads state between them is future work.

```rust
pub struct RankineCycle {
    pub steam_generator: SteamGenerator,
    pub turbine: Turbine,
    pub condenser: Condenser,
    pub feedwater_pump: Pump,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `steam_generator` | `SteamGenerator` | Boiler / steam generator. |
| `turbine` | `Turbine` | Steam turbine. |
| `condenser` | `Condenser` | Condenser. |
| `feedwater_pump` | `Pump` | Feedwater (condensate return) pump. |

##### Implementations

###### Methods

- ```rust
  pub fn new(steam_generator: SteamGenerator, turbine: Turbine, condenser: Condenser, feedwater_pump: Pump) -> Self { /* ... */ }
  ```
  Assemble a Rankine cycle from its four main components.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RankineCycle { /* ... */ }
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
    fn eq(self: &Self, other: &RankineCycle) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Re-exports

#### Re-export `Condenser`

```rust
pub use crate::components::Condenser;
```

#### Re-export `HeatExchanger`

```rust
pub use crate::components::HeatExchanger;
```

#### Re-export `Pump`

```rust
pub use crate::components::Pump;
```

#### Re-export `SteamGenerator`

```rust
pub use crate::components::SteamGenerator;
```

#### Re-export `Turbine`

```rust
pub use crate::components::Turbine;
```

#### Re-export `Valve`

```rust
pub use crate::components::Valve;
```

## Module `components`

Balance-of-plant component models.

Eight component structs, each composing existing backend types
([`crate::single_phase`], [`crate::compressible`], [`crate::hem`],
[`crate::humid_air`], and [`outram_park_fork_dwsim_libs`]'s equipment-
model correlations) rather than reimplementing physics. Method bodies
that need a real property-package/flash to actually run currently return
[`crate::TampinesError::NotYetImplemented`] -- the struct shapes and
field composition are the deliverable of this pass; wiring the method
bodies to real backend calls is tracked separately (see the workspace's
`op-dt3` epic).

```rust
pub mod components { /* ... */ }
```

### Modules

## Module `condenser`

Condenser component.

Wraps [`crate::hem::HemSteamCv`] directly, rather than any DWSIM-derived
model -- DWSIM has no dedicated condenser unit-op (its closest analog,
`Cooler.vb`, has no phase-change-specific treatment; see the workspace's
`op-dt3.10` bead notes). Condensation is naturally represented via
`HemSteamCv`'s own VLE-dome-aware quality state instead.

```rust
pub mod condenser { /* ... */ }
```

### Types

#### Struct `Condenser`

A condenser: an operating pressure and a target outlet quality (`0.0` =
saturated liquid, the typical target; a small positive value models
subcooling downstream instead).

```rust
pub struct Condenser {
    pub pressure: uom::si::f64::Pressure,
    pub target_outlet_quality: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `uom::si::f64::Pressure` | Condenser (shell-side steam) operating pressure. |
| `target_outlet_quality` | `f64` | Target outlet quality \[0, 1\] -- `0.0` for saturated-liquid outlet. |

##### Implementations

###### Methods

- ```rust
  pub fn new(pressure: Pressure, target_outlet_quality: f64) -> Self { /* ... */ }
  ```
  Construct a new condenser at the given operating pressure and target

- ```rust
  pub fn condense(self: &Self, _inlet: HemSteamCv) -> Result<HemSteamCv, TampinesError> { /* ... */ }
  ```
  Condense the given inlet steam state to this condenser's target

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Condenser { /* ... */ }
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
    fn eq(self: &Self, other: &Condenser) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `cooling_tower`

Cooling tower component.

DWSIM has no dedicated cooling-tower unit-op source (only a saved FOSSEE
flowsheet using existing generic blocks, not a unit-op class -- see the
workspace's `op-dt3.10` bead notes). This component instead wraps
[`crate::humid_air`], TAMPINES's own psychrometrics module; the
Merkel-method/NTU cooling-tower physics itself is new (no DWSIM
equivalent to port).

```rust
pub mod cooling_tower { /* ... */ }
```

### Types

#### Struct `CoolingTower`

A cooling tower: ambient air inlet state, circulating-water inlet
temperature and flow rate, and a target approach temperature (the
smallest achievable difference between the water outlet temperature and
the air's wet-bulb temperature).

```rust
pub struct CoolingTower {
    pub air_inlet: crate::humid_air::HumidAirState,
    pub water_inlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub water_flow_rate: uom::si::f64::VolumeRate,
    pub target_approach: uom::si::f64::TemperatureInterval,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `air_inlet` | `crate::humid_air::HumidAirState` | Ambient air inlet state. |
| `water_inlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | Circulating-water inlet temperature. |
| `water_flow_rate` | `uom::si::f64::VolumeRate` | Circulating-water volumetric flow rate. |
| `target_approach` | `uom::si::f64::TemperatureInterval` | Target approach temperature (`T_water,out - T_wet_bulb,air`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(air_inlet: HumidAirState, water_inlet_temperature: ThermodynamicTemperature, water_flow_rate: VolumeRate, target_approach: TemperatureInterval) -> Self { /* ... */ }
  ```
  Construct a new cooling tower with the given air inlet, water inlet

- ```rust
  pub fn evaluate(self: &Self) -> Result<(ThermodynamicTemperature, HumidAirState), TampinesError> { /* ... */ }
  ```
  Evaluate this cooling tower's water outlet temperature and air

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> CoolingTower { /* ... */ }
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
    fn eq(self: &Self, other: &CoolingTower) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `heat_exchanger`

Generic heat exchanger component.

Composes [`outram_park_fork_dwsim_libs::heat_exchanger`]'s LMTD/epsilon-NTU
building blocks behind a TAMPINES-native struct. For a two-phase
(boiling/condensing) secondary side, see [`crate::components::steam_generator`]
and [`crate::components::condenser`], which wrap this crate's own
[`crate::hem::HemSteamCv`] instead.

```rust
pub mod heat_exchanger { /* ... */ }
```

### Types

#### Struct `HeatExchanger`

A heat exchanger: flow arrangement, heat-transfer area, and overall
heat-transfer coefficient.

```rust
pub struct HeatExchanger {
    pub arrangement: outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement,
    pub area: uom::si::f64::Area,
    pub overall_coefficient: uom::si::f64::HeatTransfer,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `arrangement` | `outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement` | Co-current or counter-current flow arrangement. |
| `area` | `uom::si::f64::Area` | Heat-transfer area. |
| `overall_coefficient` | `uom::si::f64::HeatTransfer` | Overall heat-transfer coefficient `U`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(arrangement: FlowArrangement, area: Area, overall_coefficient: HeatTransfer) -> Self { /* ... */ }
  ```
  Construct a new heat exchanger with the given arrangement, area, and

- ```rust
  pub fn calculate(self: &Self, _t_hot_in: ThermodynamicTemperature, _t_cold_in: ThermodynamicTemperature) -> Result<(Power, ThermodynamicTemperature, ThermodynamicTemperature), TampinesError> { /* ... */ }
  ```
  Duty and outlet temperatures for the given inlet temperatures.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &HeatExchanger) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `pipe`

Pipe / pipeline component.

Composes either of TAMPINES's two single-phase flow backends
([`crate::single_phase::SinglePhaseFluidArray`] for lumped molten-salt/oil
loops, [`crate::compressible::CompressibleFluidArray`] for CoolProp-backed
compressible flow) behind one [`PipeBackend`] enum, plus the pipe
geometry a two-phase pressure-drop correlation
([`outram_park_fork_dwsim_libs::pipe::PipeFlowCorrelation`]) would need if
the flow becomes two-phase.

```rust
pub mod pipe { /* ... */ }
```

### Types

#### Enum `PipeBackend`

Which single-phase flow model backs a [`Pipe`].

Enum dispatch, not a trait object, per the workspace's mandatory
"no trait objects" Rust design rule.

```rust
pub enum PipeBackend {
    Lumped(crate::single_phase::SinglePhaseFluidArray),
    Compressible(crate::compressible::CompressibleFluidArray),
    SteamHem(tampines_steam_tables::openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray),
    InsulatedPipe(tuas_boussinesq_solver::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent),
}
```

##### Variants

###### `Lumped`

Lumped-parameter liquid flow (molten salt, thermal oil, ...) --
see [`SinglePhaseFluidArray`] for backend fluid coverage.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::single_phase::SinglePhaseFluidArray` |  |

###### `Compressible`

Compressible, CoolProp-backed flow (gas, near-critical, ...).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::compressible::CompressibleFluidArray` |  |

###### `SteamHem`

Homogeneous-equilibrium (HEM) steam/water flow, backed by
[`TampinesSteamArray`] and IAPWS-IF97 properties.

This is the two-phase steam/water path, and the intended BASELINE that
higher-fidelity two-phase models (drift-flux, two-fluid) are built on
and measured against — see workspace beads `op-dt3.18` and `op-dt3.19`.
Unlike [`Self::Lumped`] (single-phase liquid) and [`Self::Compressible`]
(single-phase compressible), this variant carries phase information, so
it is the one to reach for when the fluid may be wet.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines_steam_tables::openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray` |  |

###### `InsulatedPipe`

A TUAS **pre-built** insulated pipe: fluid array, metal pipe shell and
insulation, already thermally coupled to each other and to an ambient
boundary.

Prefer this over hand-assembling a [`Self::Lumped`] array and wiring
lateral links yourself — TUAS ships the coupling, and it is the only
variant that can report a **wall metal temperature** as well as a fluid
temperature, which is what a pipe's structural limit is judged against.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tuas_boussinesq_solver::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent` |  |

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
    fn clone(self: &Self) -> PipeBackend { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Pipe`

A pipe or pipeline segment: a flow backend plus the geometry a two-phase
correlation would need if the flow becomes two-phase.

```rust
pub struct Pipe {
    pub backend: PipeBackend,
    pub diameter: uom::si::f64::Length,
    pub length: uom::si::f64::Length,
    pub roughness: uom::si::f64::Length,
    pub inclination: uom::si::f64::Angle,
    pub two_phase_correlation: outram_park_fork_dwsim_libs::pipe::PipeFlowCorrelation,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `backend` | `PipeBackend` | The flow model backing this pipe. |
| `diameter` | `uom::si::f64::Length` | Pipe internal diameter. |
| `length` | `uom::si::f64::Length` | Pipe segment length. |
| `roughness` | `uom::si::f64::Length` | Absolute pipe-wall roughness. |
| `inclination` | `uom::si::f64::Angle` | Pipe inclination from horizontal, positive = uphill. |
| `two_phase_correlation` | `outram_park_fork_dwsim_libs::pipe::PipeFlowCorrelation` | Two-phase pressure-drop correlation to use if/when this pipe carries<br>two-phase flow (single-phase backends use their own native<br>pressure-drop calculation instead). |

##### Implementations

###### Methods

- ```rust
  pub fn new(backend: PipeBackend, diameter: Length, length: Length, roughness: Length, inclination: Angle) -> Self { /* ... */ }
  ```
  Construct a new pipe segment around the given flow `backend` and

- ```rust
  pub fn step(self: &mut Self, dt: Time) -> Result<(), TampinesError> { /* ... */ }
  ```
  Advance this pipe's flow state by one timestep `dt`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Pipe { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `pump`

Pump component.

Composes [`outram_park_fork_dwsim_libs::pump::modes`]'s already-working
calculation-mode algebra (`PumpSpecification`/`PumpInlet`/`evaluate`)
behind a TAMPINES-native struct.

```rust
pub mod pump { /* ... */ }
```

### Types

#### Struct `Pump`

A pump: one [`PumpSpecification`] (which quantity fixes its operating
point) plus an efficiency.

```rust
pub struct Pump {
    pub specification: outram_park_fork_dwsim_libs::pump::modes::PumpSpecification,
    pub efficiency: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `specification` | `outram_park_fork_dwsim_libs::pump::modes::PumpSpecification` | Which quantity specifies this pump's operating point. |
| `efficiency` | `uom::si::f64::Ratio` | Pump efficiency \[0, 1\]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(specification: PumpSpecification, efficiency: Ratio) -> Self { /* ... */ }
  ```
  Construct a new pump with the given operating-point specification and

- ```rust
  pub fn evaluate(self: &Self, _inlet: PumpInlet) -> Result<PumpResult, TampinesError> { /* ... */ }
  ```
  Evaluate this pump's outlet state for the given inlet.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Pump { /* ... */ }
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
    fn eq(self: &Self, other: &Pump) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `steam_generator`

Steam generator component (primary-to-secondary heat exchanger with
boiling on the secondary side).

DWSIM has no dedicated steam-generator/boiler unit-op; the closest analog
is its generic `HeatExchanger.vb` (see the workspace's `op-dt3.10` bead
notes). This component treats a steam generator as a specialized
two-phase heat exchanger: [`crate::components::heat_exchanger::HeatExchanger`]
for the primary-to-secondary coupling, [`crate::hem::HemSteamCv`] for the
boiling secondary-side steam/water state.

```rust
pub mod steam_generator { /* ... */ }
```

### Types

#### Struct `SteamGenerator`

A steam generator: the primary-to-secondary heat exchanger plus the
current secondary-side (steam/water) state.

```rust
pub struct SteamGenerator {
    pub heat_exchanger: crate::components::heat_exchanger::HeatExchanger,
    pub secondary_side: crate::hem::HemSteamCv,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `heat_exchanger` | `crate::components::heat_exchanger::HeatExchanger` | Primary-to-secondary heat-exchanger geometry/coefficient. |
| `secondary_side` | `crate::hem::HemSteamCv` | Current secondary-side steam/water state. |

##### Implementations

###### Methods

- ```rust
  pub fn new(heat_exchanger: HeatExchanger, secondary_side: HemSteamCv) -> Self { /* ... */ }
  ```
  Construct a new steam generator around the given heat exchanger and

- ```rust
  pub fn step(self: &mut Self, _primary_duty: Power) -> Result<(), TampinesError> { /* ... */ }
  ```
  Advance the secondary-side state given a primary-side heat duty.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SteamGenerator { /* ... */ }
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
    fn eq(self: &Self, other: &SteamGenerator) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `turbine`

Steam turbine component.

Composes [`crate::hem::HemSteamCv`] (the inlet/outlet steam state) with
[`outram_park_fork_dwsim_libs::expander::isentropic`]'s adiabatic/Schultz
polytropic turbine model.

```rust
pub mod turbine { /* ... */ }
```

### Types

#### Struct `Turbine`

A steam turbine: inlet state plus an efficiency specification.

```rust
pub struct Turbine {
    pub inlet: crate::hem::HemSteamCv,
    pub adiabatic_efficiency: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inlet` | `crate::hem::HemSteamCv` | Inlet steam state. |
| `adiabatic_efficiency` | `uom::si::f64::Ratio` | Adiabatic efficiency \[0, 1\] (this scaffold only represents the<br>adiabatic path; the polytropic path needs a flash-dependent<br>iteration -- see<br>[`outram_park_fork_dwsim_libs::expander::isentropic::solve_polytropic_efficiency`]). |

##### Implementations

###### Methods

- ```rust
  pub fn new(inlet: HemSteamCv, adiabatic_efficiency: Ratio) -> Self { /* ... */ }
  ```
  Construct a new turbine with the given inlet state and adiabatic

- ```rust
  pub fn expand_to(self: &Self, _outlet_pressure: Pressure) -> Result<HemSteamCv, TampinesError> { /* ... */ }
  ```
  Expand this turbine's inlet steam to the given outlet pressure,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Turbine { /* ... */ }
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
    fn eq(self: &Self, other: &Turbine) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `valve`

Control valve component.

Composes [`outram_park_fork_dwsim_libs::valve::iec_60534`]'s IEC 60534
sizing types behind a TAMPINES-native struct.

```rust
pub mod valve { /* ... */ }
```

### Types

#### Struct `Valve`

A control valve: a maximum flow coefficient, an opening-percentage-to-`Kv`
characteristic, and the current opening.

```rust
pub struct Valve {
    pub kv_max: outram_park_fork_dwsim_libs::valve::iec_60534::ValveFlowCoefficient,
    pub opening_characteristic: outram_park_fork_dwsim_libs::valve::iec_60534::OpeningCharacteristic,
    pub opening_percent: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kv_max` | `outram_park_fork_dwsim_libs::valve::iec_60534::ValveFlowCoefficient` | Fully-open flow coefficient. |
| `opening_characteristic` | `outram_park_fork_dwsim_libs::valve::iec_60534::OpeningCharacteristic` | Relationship between opening percentage and `Kv`. |
| `opening_percent` | `uom::si::f64::Ratio` | Current valve opening, \[0, 100\] percent. |

##### Implementations

###### Methods

- ```rust
  pub fn new(kv_max: ValveFlowCoefficient, opening_characteristic: OpeningCharacteristic, opening_percent: Ratio) -> Self { /* ... */ }
  ```
  Construct a new valve with the given maximum `Kv`, opening

- ```rust
  pub fn current_kv(self: &Self) -> ValveFlowCoefficient { /* ... */ }
  ```
  This valve's current (opening-adjusted) flow coefficient.

- ```rust
  pub fn mass_flow(self: &Self, _p1: Pressure, _p2: Pressure) -> Result<MassRate, TampinesError> { /* ... */ }
  ```
  Mass flow rate through this valve for the given upstream/downstream

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Valve { /* ... */ }
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
    fn eq(self: &Self, other: &Valve) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Re-exports

#### Re-export `Condenser`

```rust
pub use condenser::Condenser;
```

#### Re-export `CoolingTower`

```rust
pub use cooling_tower::CoolingTower;
```

#### Re-export `HeatExchanger`

```rust
pub use heat_exchanger::HeatExchanger;
```

#### Re-export `Pipe`

```rust
pub use pipe::Pipe;
```

#### Re-export `PipeBackend`

```rust
pub use pipe::PipeBackend;
```

#### Re-export `Pump`

```rust
pub use pump::Pump;
```

#### Re-export `SteamGenerator`

```rust
pub use steam_generator::SteamGenerator;
```

#### Re-export `Turbine`

```rust
pub use turbine::Turbine;
```

#### Re-export `Valve`

```rust
pub use valve::Valve;
```

## Module `compressible`

Compressible pipe flow / gas-cooled thermal-hydraulics.

Re-exports [`outram_park_fork_coolprop`]'s finite-volume compressible
pipe-flow model (a `rhoPimpleFoam`-derived solver) under a
TAMPINES-local name. This is the natural backend for HTGR primary loops,
gas-cooled transients, and any fluid CoolProp backs but
[`crate::single_phase`]'s TUAS backend does not (water, air, helium, ...).

This is a thin wiring module: it does not add behaviour beyond naming.
The full TAMPINES fluid-array interface (unifying this with
[`crate::single_phase`] and the steam-tables backend behind one enum) is
tracked separately -- see the workspace's beads issue tracker, epic
`op-dt3`.

```rust
pub mod compressible { /* ... */ }
```

### Re-exports

#### Re-export `OPCPFluidArray`

A compressible pipe-flow loop segment (finite-volume, CoolProp-backed
thermophysical properties, lateral thermal coupling + heat source).

Alias for [`outram_park_fork_coolprop`]'s `OPCPFluidArray` -- see that
type's own documentation for the underlying physics and its `new`
constructor.

Note: after calling `step()`, `t`/`rho` lag `he` (specific enthalpy) by
one outer-corrector iteration -- call `correct_thermo()` afterwards if a
caller needs `t`/`rho` consistent with the just-solved `he`.

```rust
pub use outram_park_fork_coolprop::OPCPFluidArray as CompressibleFluidArray;
```

#### Re-export `OPCPFluidArrayError`

Error type returned by [`CompressibleFluidArray`] methods. Re-exported
from `outram-park-fork-coolprop` for convenience.

```rust
pub use outram_park_fork_coolprop::OPCPFluidArrayError as CompressibleFluidArrayError;
```

#### Re-export `Fluid`

The fluid substances a [`CompressibleFluidArray`] can be backed by.
Re-exported from `outram-park-fork-coolprop` for convenience.

```rust
pub use outram_park_fork_coolprop::Fluid as CoolPropFluid;
```

## Module `cooling_tower`

Cooling-tower thermal-hydraulics.

**Status: scaffold only.** This is the intended home for the actual
cooling-tower calculation engine (counter-flow air-water heat and mass
transfer -- the Merkel method / its effectiveness-NTU reformulation).
[`crate::components::CoolingTower`] is the user-facing struct/interface;
[`crate::humid_air`] is the psychrometrics module this engine will build
on. Neither the physics nor the wiring between them exists yet.

DWSIM has no cooling-tower unit-op to port (checked directly against the
DWSIM source -- only a saved FOSSEE flowsheet built from generic blocks,
not a unit-op class; see the workspace's `op-dt3.10` bead notes). So this
module's eventual physics will need to be implemented from published
theory rather than translated, e.g.:

- Merkel, F. (1925). *Verdunstungskühlung*. VDI-Zeitschrift.
- Braun, J. E., Klein, S. A., & Mitchell, J. W. (1989). Effectiveness
  models for cooling towers and cooling coils. *ASHRAE Transactions*,
  95(2), 164-174 (the effectiveness-NTU reformulation of Merkel's method).

Implementing and verifying that engine (against a published worked
example, per this workspace's V&V philosophy) is tracked as follow-up
work, not done in this pass.

```rust
pub mod cooling_tower { /* ... */ }
```

### Functions

#### Function `merkel_ntu_effectiveness`

Placeholder for the future cooling-tower calculation engine's entry
point. Not yet implemented -- see this module's doc for what it will
need to cover and why nothing exists here yet.

```rust
pub fn merkel_ntu_effectiveness() -> Result<(), crate::TampinesError> { /* ... */ }
```

## Module `critical_flow`

Choked (critical) two-phase flow.

Thin re-export of [`tampines_steam_tables`]'s choked-flow solvers
(Homogeneous Equilibrium Model critical flow). V&V status, re-read from
that crate's test source on 2026-08-11:

- **Moody (1975) Fig. 1** -- verified: 13 active isobar tests
  (`moody_critical_mass_flux_homogeneous_eqm.rs`, p0/p_ref = 0.25-30.0)
  assert `|log10 G_test - log10 G_ref| <= 0.06` (0.08 for
  deeply-subcooled Region-1 points). The validator is region-filtered:
  points that are neither in-dome (Region 4) nor deeply subcooled are
  skipped as a documented HEM limitation, not asserted.
- **Zaloudek** -- verified: ~20+ active tests per file across the
  in-dome, subcooled, superheated, generic-dispatcher, and
  backward-throat test files (critical-pressure relative tolerance
  0.005-0.05 by curve; mass-flux log10 tolerance 0.05). The reference
  curves are graph-read HEM curves, not raw experimental data.
- **Marviken is NOT validated.** The digitised NUREG/CR-2671 test-23/24
  data sits in `marviken_tests.rs`, but the test is
  `#[ignore = "skip first, Marviken is more complex"]`, its only
  assertion is commented out, and the body ends in `todo!()`. Do not
  cite Marviken as a validation basis for these solvers.

See that crate's own `CLAUDE.md` and README for the full V&V
methodology and results. (Separately, the Edwards-O'Brien blowdown
benchmark is implemented and tested in [`crate::multiphase_1d`] and in
`tampines-steam-tables`'s `tests/edwards_blowdown.rs` -- a different
module and benchmark from the nozzle critical-flow gates above.)
[`crate::hem`] provides the underlying two-phase state type these
solvers operate on.

```rust
pub mod critical_flow { /* ... */ }
```

### Re-exports

#### Re-export `get_critical_pressure_and_mass_flux_multiphase_ph`

Critical (choked) pressure and mass flux for a stagnation state `(p0,
h0)` anywhere relative to the vapour-liquid-equilibrium dome --
subcooled liquid, two-phase (in-dome), or superheated/supercritical
vapour. Unified dispatcher; routes internally by the stagnation state's
IAPWS-IF97 flash region.

Alias for [`tampines_steam_tables`]'s
`get_critical_pressure_and_mass_flux_multiphase_ph`.

```rust
pub use choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph as critical_pressure_and_mass_flux;
```

#### Re-export `get_choked_flow_massrate_and_state_from_stagnation_properties_and_area`

Mass flow rate and downstream thermodynamic state ([`crate::hem::HemSteamCv`])
for choked flow through a converging-diverging nozzle throat, given the
stagnation state and throat area.

Alias for [`tampines_steam_tables`]'s
`get_choked_flow_massrate_and_state_from_stagnation_properties_and_area`.

```rust
pub use tampines_steam_tables::prelude::get_choked_flow_massrate_and_state_from_stagnation_properties_and_area as choked_massrate_and_state;
```

## Module `error`

Crate-wide error type for TAMPINES.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `TampinesError`

Errors returned by TAMPINES's public API.

The framework is built out incrementally (see the crate-level docs for the
current module surface); a public item that exists as a documented stub
but has no working implementation yet returns
[`TampinesError::NotYetImplemented`] rather than panicking or silently
returning a placeholder value.

```rust
pub enum TampinesError {
    NotYetImplemented {
        component: &'static str,
    },
    InvalidInput(String),
    Unphysical(String),
    Numerical(String),
    Closure(outram_foam_multiphase::MultiphaseError),
}
```

##### Variants

###### `NotYetImplemented`

The called component's physics is not implemented yet.

`component` names the module or method (e.g. `"hem::future_multiphase::drift_flux"`)
so a caller hitting this can find the relevant stub and its tracking
bead.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `component` | `&'static str` | Path-like name of the unimplemented component. |

###### `InvalidInput`

A caller-supplied input is outside the range the model accepts.

Distinct from [`Unphysical`](Self::Unphysical): this is a *usage*
error — a mismatched slice length, a non-positive cell count, a
negative timestep — caught before any physics runs.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Unphysical`

A quantity left the range physics allows, during a solve.

Reported rather than clamped. A void fraction of `1.4` or a negative
absolute pressure means the discretisation has failed, and continuing
from a clamped value produces a plausible-looking answer that is
wrong — the failure mode this crate's V&V rules exist to prevent.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Numerical`

A linear solve or an iteration failed.

Carries what failed and where, so a caller can tell a singular matrix
from a non-converged fixed point.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Closure`

A closure borrowed from the OUTRAM-FOAM 3-D multiphase reference
rejected its inputs.

The 1-D solvers in [`crate::multiphase_1d`] evaluate
[`outram_foam_multiphase`]'s slip and drag correlations directly, so
that crate's own error surfaces here rather than being flattened into a
string — the provenance of a failure is part of the diagnosis.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_foam_multiphase::MultiphaseError` |  |

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
    fn from(source: outram_foam_multiphase::MultiphaseError) -> Self { /* ... */ }
    ```

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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `fluids`

Fluid selection unified across TAMPINES's backend property libraries.

This module belongs to the *fluid-selection* concern only -- picking which
backend evaluates a fluid's thermophysical properties. It does not itself
hold flow/loop state; see [`crate::single_phase`] and
[`crate::compressible`] for the component-level wrappers that use a
[`TampinesFluid`] to construct a loop segment.

```rust
pub mod fluids { /* ... */ }
```

### Types

#### Enum `TampinesFluid`

Which backend thermophysical-property source a TAMPINES fluid is
evaluated with.

This is a *property-backend* selector, not an independent substance list:
[`TampinesFluid::CoolProp`] carries an
[`outram_park_fork_coolprop::Fluid`] (137 fluids, including water, air,
nitrogen, helium, ...); [`TampinesFluid::Steam`] instead routes to the
dedicated IAPWS-IF97 `tampines-steam-tables` backend, this workspace's own
water/steam implementation, rather than CoolProp's own water equation of
state.

**V&V status of that backend**, re-read from its test source on
2026-08-11 -- state it accurately rather than calling it "validated"
wholesale:

- **IF97 property regions** -- verified against the Kretzschmar & Wagner
  (2019) IAPWS-IF97 reference tables in the per-region test modules.
- **Moody (1975) Fig. 1 choked flow** -- verified: 13 active isobar tests
  (`moody_critical_mass_flux_homogeneous_eqm.rs`, p0/p_ref = 0.25-30.0)
  assert `|log10 G_test - log10 G_ref| <= 0.06`, loosened to 0.08 for
  deeply-subcooled Region-1 points. The validator is region-filtered:
  points that are neither in-dome (Region 4) nor deeply subcooled are
  skipped as a documented HEM limitation, not asserted.
- **Zaloudek choked flow** -- verified: 89 tests across five files
  (21 in-dome / 21 generic-dispatcher / 21 backward-throat / 22
  subcooled, one of which is an `#[ignore]`d diagnostic sweep / 4
  superheated), at critical-pressure tolerance 0.005-0.05 by curve and
  mass-flux log10 tolerance 0.05. Caveat: the reference curves are
  graph-read HEM curves digitised from Saha (1978) NUREG/CR-0417, **not**
  raw experimental data.
- **Marviken is NOT validated.** The digitised NUREG/CR-2671 test-23/24
  data sits in `marviken_tests.rs`, but the single test is
  `#[ignore = "skip first, Marviken is more complex"]`, its only
  assertion is commented out, and the body ends in `todo!()`. Marviken is
  **not** a basis for choosing this backend. Status as of 2026-08-11:
  gating is being implemented in `tampines-steam-tables` (the follow-up
  to op-bcg/op-4ily); re-check that crate's test suite for the current
  result rather than treating this line as permanent.

Separately, the Edwards-O'Brien blowdown benchmark **is** genuinely gated
-- 2 active, passing tests in `tampines-steam-tables`'s
`tests/edwards_blowdown.rs` -- but that exercises the transient pipe
solver in [`crate::multiphase_1d`], not the property backend selected
here.

```rust
pub enum TampinesFluid {
    CoolProp(outram_park_fork_coolprop::Fluid),
    Steam,
}
```

##### Variants

###### `CoolProp`

Evaluate via the CoolProp-derived backend (`outram-park-fork-coolprop`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_park_fork_coolprop::Fluid` |  |

###### `Steam`

Evaluate water/steam via the IAPWS-IF97 `tampines-steam-tables` backend.

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
    fn clone(self: &Self) -> TampinesFluid { /* ... */ }
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
    fn eq(self: &Self, other: &TampinesFluid) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `gas_phase`

# Gas-phase thermal-hydraulics for the HTR-10 helium primary circuit

Single-phase **compressible gas** component models, at the low Mach
numbers of a gas-cooled reactor primary loop.

## Why this module exists (scope ruling, 2026-08-11)

[`tuas_boussinesq_solver`], which backs [`crate::single_phase`], models
nearly-**incompressible liquids**: its Boussinesq treatment assumes
density varies only weakly, and only in the buoyancy term. Helium at
3.0 MPa spans a factor of **1.85 in density** across the HTR-10 core's
250-700 C rise — `2.739989 kg/m^3` at 523.15 K down to
`1.478781 kg/m^3` at 973.15 K, a 46.0 % fall (measured 2026-08-11, see
[`properties`]) — so that assumption fails outright. Gas is therefore
**permanently out of TUAS scope**, and the gas layer belongs in TAMPINES
— the superset that composes TUAS, `outram-park-fork-coolprop` and the
steam tables.

## Low-Mach design assumption

The HTR-10 primary circuit runs at Mach numbers between **1.4e-3**
(the pebble-bed core, measured in `outram-park-fork-coolprop`) and
**2.2e-2** (a 0.30 m hot duct at the design flow, measured 2026-08-11 —
see [`pipe`]). The models here are therefore built for
**low-Mach compressible flow**: density is a full function of `(T, p)`
via the Helmholtz equation of state, but the kinetic-energy and
compressibility-work terms in the energy equation are dropped as
negligible at `Ma^2 ~ 1e-6`. This is stated rather than silently
assumed, and each model repeats it where it bites.

Consequently the `HybridAllMach` machinery in
`outram-park-fork-coolprop` is **not** needed for the primary circuit,
and neither is coolprop's 50 kg/m^3 density-taper window — HTR-10
helium runs at 1.23-2.74 kg/m^3, far below that floor, so the taper
would zero the all-Mach dissipation everywhere anyway.

## What belongs here

- [`properties`] — helium thermophysical-property adapter over
  `outram-park-fork-coolprop` (density, viscosity, conductivity, cp,
  Prandtl, speed of sound) at a `(T, p)` state point.
- [`pipe`] — steady-state gas duct: Darcy friction pressure drop
  (Churchill) and convective heat transfer (Dittus-Boelter /
  Gnielinski).
- [`kta_bed`] — KTA 3102.3 packed-bed (pebble-bed) pressure drop.
- [`circulator`] — idealised helium circulator (fixed pressure rise or
  fixed mass flow, isentropic-efficiency temperature rise).

## What does NOT belong here

- **Property correlations** — those live in
  `outram-park-fork-coolprop`; [`properties`] only adapts them.
- **Liquid thermal-hydraulics** — [`crate::single_phase`] (TUAS) and
  [`crate::hem`] (steam/water) own those.
- **Two-phase flow** — helium in the primary circuit is single-phase
  supercritical gas by a wide margin; [`crate::multiphase_1d`] owns
  two-phase.
- **Bed *conduction*** — the Zehner-Bauer-Schlunder effective
  conductivity lives in [`crate::pebble_bed::zbs`]. This module owns the
  bed's *friction* side only.
- **Reactor physics** — neutronics belongs to `teh-o-prke`,
  `outram-mc-libs` and `njoy-outram-park-fork`.

## Module placement note (2026-08-11)

[`crate::pebble_bed`]'s module docs list `kta` as a *planned* module
**there**. This session delivers it as [`kta_bed`] **here** instead,
because the KTA correlation is a gas-side friction closure that needs
[`properties`] and shares the low-Mach framing with [`pipe`], whereas
`pebble_bed` is the *conduction* stack. The stale note in
`pebble_bed/mod.rs` is left for the maintainer to reconcile.

## Status

**NOT VALIDATED against HTR-10 measurements.** The word "validated" is
not used for any HTR-10-specific claim in this module. [`kta_bed`] is
*verified against the VTB gold values*; everything else carries
code-to-code and analytic-limit checks only. AI-assisted draft pending
human review per `RESPONSIBLE_USE.md`.

```rust
pub mod gas_phase { /* ... */ }
```

### Modules

## Module `circulator`

# Idealised helium circulator

The pressure-raising machine that drives a gas-cooled reactor primary
circuit, modelled as an **idealisation** rather than from a
characteristic map.

## What belongs here / what does not

- **Belongs:** the thermodynamics of a single-stage compression at a
  prescribed duty — the isentropic temperature rise, the efficiency
  correction to it, and the shaft power that follows.
- **Does NOT belong:** a real machine **characteristic map**
  (pressure rise vs. volumetric flow vs. shaft speed, with surge and
  choke limits), variable-speed control, or rotor dynamics. Those are
  **future work** — see the "Limitations" section below. Nor the loop
  resistance the circulator works against, which is
  [`super::pipe`] and [`super::kta_bed`].

## Formulation

Given an inlet state `(T_1, p_1)` and an outlet pressure `p_2`, the
**isentropic** outlet state is the one at `p_2` with the same specific
entropy as the inlet, so the ideal specific work is
`w_s = h_2s - h_1`. The **actual** work follows from the isentropic
efficiency `eta_s`:

```text
w_actual = (h_2s - h_1) / eta_s
h_2      = h_1 + w_actual
T_2      = T(p_2, h_2)          (real-gas (p,h) flash)
P_shaft  = mdot * w_actual
```

All of it is done on **specific enthalpy** with real-gas `(p, s)` and
`(p, h)` flashes from the Helmholtz equation of state, not on an
ideal-gas `(p_2/p_1)^((gamma-1)/gamma)` shortcut. For helium at HTGR
conditions the two are close, but only because helium is nearly ideal
there — the shortcut is not assumed.

The efficiency loss appears entirely as **extra enthalpy in the gas**
(an adiabatic machine); no heat is lost to the surroundings.

## Limitations (read before using a result)

- **No characteristic map.** The duty is whatever the caller
  prescribes; the circulator cannot tell you whether a real machine
  could deliver that pressure rise at that flow, and it has no surge or
  choke limit. A real map, and the flow-vs-resistance intersection it
  would let you solve for, is **future work** and is not implemented.
- **No off-design efficiency.** `eta_s` is a constant the caller
  supplies, not a function of flow or speed.
- **Single stage, adiabatic.** No intercooling, no leakage, no bearing
  or windage losses beyond whatever the caller folds into `eta_s`.

## Status

**NOT VALIDATED.** Checked against thermodynamic limits (the
`eta_s = 1` case must be exactly isentropic; the temperature rise must
grow as efficiency falls) and self-consistency only. AI-assisted draft
pending human review per `RESPONSIBLE_USE.md`.

```rust
pub mod circulator { /* ... */ }
```

### Types

#### Enum `CirculatorDuty`

What the circulator is asked to deliver.

Enum dispatch, not a trait object, per the workspace's mandatory "no
trait objects" Rust design rule.

```rust
pub enum CirculatorDuty {
    PressureRise(uom::si::f64::Pressure),
    PressureRatio(uom::si::f64::Ratio),
}
```

##### Variants

###### `PressureRise`

A prescribed **pressure rise** `p_2 - p_1`, Pa. Must be positive.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Pressure` |  |

###### `PressureRatio`

A prescribed **pressure ratio** `p_2 / p_1`, dimensionless. Must be
greater than 1.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Ratio` |  |

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
    fn clone(self: &Self) -> CirculatorDuty { /* ... */ }
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
    fn eq(self: &Self, other: &CirculatorDuty) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Circulator`

An idealised single-stage helium circulator: a duty and an isentropic
efficiency.

Plain data; the physics lives in [`Circulator::compress_helium`].

```rust
pub struct Circulator {
    pub duty: CirculatorDuty,
    pub isentropic_efficiency: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `duty` | `CirculatorDuty` | What the machine is asked to deliver. |
| `isentropic_efficiency` | `uom::si::f64::Ratio` | Isentropic (adiabatic) efficiency, dimensionless, in `(0, 1]`.<br>A large helium circulator sits around 0.80-0.90; `1.0` gives the<br>exactly-isentropic ideal machine. |

##### Implementations

###### Methods

- ```rust
  pub fn new_fixed_pressure_rise(rise: Pressure, isentropic_efficiency: Ratio) -> Self { /* ... */ }
  ```
  A circulator delivering a fixed pressure rise at the given

- ```rust
  pub fn new_fixed_pressure_ratio(ratio_pp: Ratio, isentropic_efficiency: Ratio) -> Self { /* ... */ }
  ```
  A circulator delivering a fixed pressure ratio at the given

- ```rust
  pub fn outlet_pressure(self: &Self, inlet_pressure: Pressure) -> Result<Pressure, TampinesError> { /* ... */ }
  ```
  The outlet pressure this duty implies for the given inlet pressure.

- ```rust
  pub fn compress_helium(self: &Self, mass_flow: MassRate, inlet_temperature: ThermodynamicTemperature, inlet_pressure: Pressure) -> Result<CirculatorResult, TampinesError> { /* ... */ }
  ```
  Compress helium from the given inlet state at the given mass flow.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Circulator { /* ... */ }
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
    fn eq(self: &Self, other: &Circulator) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `CirculatorResult`

Everything [`Circulator::compress_helium`] computed.

```rust
pub struct CirculatorResult {
    pub inlet: super::properties::HeliumState,
    pub outlet: super::properties::HeliumState,
    pub isentropic_outlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub specific_work: super::properties::SpecificEnthalpy,
    pub isentropic_specific_work: super::properties::SpecificEnthalpy,
    pub shaft_power: uom::si::f64::Power,
    pub temperature_rise_kelvin: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inlet` | `super::properties::HeliumState` | Gas state at the circulator inlet. |
| `outlet` | `super::properties::HeliumState` | Gas state at the circulator outlet (real, efficiency-corrected). |
| `isentropic_outlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | Outlet temperature the machine would reach if it were exactly<br>isentropic, K — the lower bound on [`Self::outlet`]'s temperature. |
| `specific_work` | `super::properties::SpecificEnthalpy` | Actual specific work put into the gas, J/kg. |
| `isentropic_specific_work` | `super::properties::SpecificEnthalpy` | Ideal (isentropic) specific work, J/kg. Equals<br>[`Self::specific_work`] times the efficiency. |
| `shaft_power` | `uom::si::f64::Power` | Shaft power, W: `mdot` times [`Self::specific_work`]. |
| `temperature_rise_kelvin` | `f64` | Temperature rise across the machine, K. |

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
    fn clone(self: &Self) -> CirculatorResult { /* ... */ }
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
    fn eq(self: &Self, other: &CirculatorResult) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `kta_bed`

# KTA 3102.3 packed-bed (pebble-bed) pressure drop

The friction side of a pebble bed: the pressure gradient a gas coolant
sees flowing through a randomly packed bed of monosized spheres.

## What belongs here / what does not

- **Belongs:** the KTA-form packed-bed friction factor and the pressure
  gradient / total drop built from it, plus the bed geometry those need.
- **Does NOT belong:** the bed's *conduction* physics — the
  Zehner-Bauer-Schlunder effective conductivity lives in
  [`crate::pebble_bed::zbs`]. Nor particle-to-fluid heat transfer
  (Wakao), which is still unimplemented anywhere in the workspace.

## Formulation

With `eps` the bed porosity, `G = mdot/A` the **superficial** mass flux
(referred to the empty bed cross-section), `D_h` the pebble diameter,
`rho` the gas density and `mu` its dynamic viscosity:

```text
Re      = G * D_h / mu                      (superficial Reynolds number)
Re_mod  = Re / (1 - eps)                    (modified Reynolds number)
psi     = 320 / Re_mod + 6 / Re_mod^0.1     (KTA friction factor)
-dp/dx  = psi * (1 - eps)/eps^3 * G^2 / (2 * D_h * rho)
dp      = (-dp/dx) * L
```

The `320` coefficient sits on the **linear (viscous)** term and the `6`
on the weakly Reynolds-dependent **inertial** term. Note carefully that
[`packed_bed_reynolds`] returns the *plain superficial* `Re`; the
`1/(1-eps)` modification happens **inside** [`kta_friction_factor`].
Passing an already-modified Reynolds number in is the obvious way to get
this wrong.

The returned gradient is a **positive magnitude** (the drop), not a
signed derivative.

## Validity

Randomly packed monosized spheres near random-packing porosity (the
HTR-10 bed is `eps = 0.39`), for modified Reynolds numbers `Re/(1-eps)`
from about 1 to 1e5. The VTB worked example below sits at
`Re/(1-eps) = 6.6e4`, near the top of that band. The correlation is a
**steady, incompressible-within-a-slice** friction closure: for a long
bed with significant gas expansion, march it in slices with properties
re-evaluated per slice rather than applying it once with a single mean
density.

## Provenance, and why this is a reimplementation

The workspace already carries a verified implementation of this
correlation in **`crates/outram-park-digital-twin-engine/src/htr10/kta.rs`**
(read on 2026-08-11). TAMPINES **cannot depend on that crate** — it is a
downstream GUI/digital-twin crate, so the edge would invert the
dependency graph. This module is therefore a deliberate, faithful
**reimplementation** of the same formulation, verified against the same
gold values, rather than a shared type. The two are independent code
paths that must agree.

The upstream implementation's stated source is the **Virtual Test Bed
generic pebble-bed tutorial, step 2** (Open tier;
`reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md`,
with the porosity taken from `step2.i`). That is the equation set
transcribed above. **Honesty note:** neither that module nor this one
was written with page-level access to the KTA 3102.3 standard itself —
the "KTA" name is carried over from how the VTB tutorial and the
pebble-bed literature label this `320/6` form. A human should confirm
the coefficients against the standard before this module is promoted
past Prototype in the V&V pipeline.

## Verification & Validation

**Methodology.** Reproduce the VTB generic pebble-bed tutorial step-2
worked example, whose published inputs are `Re = 40125`, `eps = 0.39`,
`D_h = 0.06 m`, `mu = 1.991242e-5 Pa s`, `rho = 8.628204 kg/m^3`, over a
`10 m` bed, and whose published outputs are the friction factor
`psi = 1.983` and the pressure gradient `3493 Pa/m` (Pronghorn itself
reports a drop of `3.4933e4 Pa`). Pass criterion: friction factor within
`0.001`, gradient within `1 Pa/m`, drop within `0.01 kPa` — the
resolution of the published digits.

**Results.** See the `#[test]` doc comments in this module's `tests`
submodule for the numbers this implementation actually produced on
2026-08-11, measured by running the tests.

## Status

**Verified against the VTB gold values.** Not validated against HTR-10
measurements — no comparison against plant or experimental data has been
made, and none is claimed. AI-assisted draft pending human review per
`RESPONSIBLE_USE.md`.

```rust
pub mod kta_bed { /* ... */ }
```

### Types

#### Struct `KtaBed`

A packed bed of monosized spheres, described by the three parameters the
KTA pressure-drop correlation needs. Plain data; the physics lives in
[`KtaBed::pressure_gradient`] and [`KtaBed::pressure_drop`].

Construct with [`KtaBed::new`], or [`KtaBed::htr10`] for the cited
HTR-10 core.

```rust
pub struct KtaBed {
    pub porosity: uom::si::f64::Ratio,
    pub pebble_diameter: uom::si::f64::Length,
    pub cross_section: uom::si::f64::Area,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `porosity` | `uom::si::f64::Ratio` | Bed porosity (void fraction), dimensionless, strictly in `(0, 1)`.<br>Random close-packed sphere beds sit near 0.36-0.42; the HTR-10 bed<br>is 0.39 (filling fraction 0.61, IAEA HTR-10 benchmark description,<br>Open tier). |
| `pebble_diameter` | `uom::si::f64::Length` | Pebble (sphere) diameter, metres — the correlation's `D_h`.<br>HTR-10: 0.06 m. |
| `cross_section` | `uom::si::f64::Area` | Bed cross-sectional area, m^2, referred to the **empty** bed (the<br>full core barrel bore, not the free area between pebbles). HTR-10's<br>1.8 m core diameter gives about 2.545 m^2. |

##### Implementations

###### Methods

- ```rust
  pub fn new(porosity: Ratio, pebble_diameter: Length, cross_section: Area) -> Self { /* ... */ }
  ```
  A bed of the given porosity (dimensionless, in `(0,1)`), pebble

- ```rust
  pub fn htr10() -> Self { /* ... */ }
  ```
  The HTR-10 pebble-bed core: porosity 0.39 (filling fraction 0.61)

- ```rust
  pub fn free_flow_area(self: &Self) -> Area { /* ... */ }
  ```
  Free (open) flow area between the pebbles, `eps * A`, m^2.

- ```rust
  pub fn pressure_gradient_detailed(self: &Self, mass_flow: MassRate, density: MassDensity, dynamic_viscosity: DynamicViscosity) -> Result<KtaBedResult, TampinesError> { /* ... */ }
  ```
  Full KTA evaluation at a given mass flow and gas state, returning

- ```rust
  pub fn pressure_gradient(self: &Self, mass_flow: MassRate, density: MassDensity, dynamic_viscosity: DynamicViscosity) -> Result<PressureGradient, TampinesError> { /* ... */ }
  ```
  Pressure-drop magnitude per unit bed length, Pa/m (positive). Thin

- ```rust
  pub fn pressure_drop(self: &Self, mass_flow: MassRate, density: MassDensity, dynamic_viscosity: DynamicViscosity, bed_height: Length) -> Result<Pressure, TampinesError> { /* ... */ }
  ```
  Total pressure drop across a bed of height `bed_height`, Pa

- ```rust
  pub fn pressure_drop_helium_marched(self: &Self, mass_flow: MassRate, pressure: Pressure, inlet_temperature: ThermodynamicTemperature, outlet_temperature: ThermodynamicTemperature, bed_height: Length, n_slices: usize) -> Result<Pressure, TampinesError> { /* ... */ }
  ```
  Total helium pressure drop across a heated bed, marched in

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> KtaBed { /* ... */ }
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
    fn eq(self: &Self, other: &KtaBed) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `KtaBedResult`

Everything [`KtaBed::pressure_gradient_detailed`] computed, so a caller
can inspect the intermediate dimensionless groups rather than trust a
bare pressure number.

```rust
pub struct KtaBedResult {
    pub mass_flux: super::MassFlux,
    pub reynolds: uom::si::f64::Ratio,
    pub modified_reynolds: uom::si::f64::Ratio,
    pub friction_factor: uom::si::f64::Ratio,
    pub pressure_gradient: super::PressureGradient,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mass_flux` | `super::MassFlux` | Superficial mass flux `G = mdot/A`, kg/(m^2 s). |
| `reynolds` | `uom::si::f64::Ratio` | Superficial Reynolds number `Re = G D_h / mu`, dimensionless. |
| `modified_reynolds` | `uom::si::f64::Ratio` | Modified Reynolds number `Re/(1 - eps)`, dimensionless — the group<br>the correlation's stated 1 to 1e5 validity band is quoted on. |
| `friction_factor` | `uom::si::f64::Ratio` | KTA friction factor `psi`, dimensionless. |
| `pressure_gradient` | `super::PressureGradient` | Pressure-drop magnitude per unit bed length, Pa/m (positive). |

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
    fn clone(self: &Self) -> KtaBedResult { /* ... */ }
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
    fn eq(self: &Self, other: &KtaBedResult) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `superficial_mass_flux`

Superficial mass flux `G = mdot / A`, kg/(m^2 s), referred to the
**empty** bed cross-section (not the free-flow area between pebbles).

The KTA correlation is defined on the superficial flux; dividing by the
porous free area instead inflates `G` by `1/eps` and the gradient by
`1/eps^2`.

```rust
pub fn superficial_mass_flux(mass_flow: uom::si::f64::MassRate, bed_cross_section: uom::si::f64::Area) -> super::MassFlux { /* ... */ }
```

#### Function `packed_bed_reynolds`

Superficial packed-bed Reynolds number `Re = G D_h / mu`, dimensionless.

This is the **plain** superficial `Re`. [`kta_friction_factor`] applies
the `1/(1-eps)` modification itself — do not pre-divide.

```rust
pub fn packed_bed_reynolds(mass_flux: super::MassFlux, pebble_diameter: uom::si::f64::Length, dynamic_viscosity: uom::si::f64::DynamicViscosity) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `kta_friction_factor`

KTA packed-bed friction factor
`psi = 320/Re_mod + 6/Re_mod^0.1`, dimensionless, with
`Re_mod = Re/(1 - eps)`.

`reynolds` is the **superficial** Reynolds number from
[`packed_bed_reynolds`]; `porosity` is the bed void fraction, strictly
between 0 and 1.

Errors with [`TampinesError::InvalidInput`] for a porosity outside
`(0, 1)` or a non-positive / non-finite Reynolds number (the `320/Re`
term diverges at `Re = 0`; a stagnant bed has no friction gradient to
report, so the caller must handle that case rather than receive an
infinity).

```rust
pub fn kta_friction_factor(reynolds: uom::si::f64::Ratio, porosity: uom::si::f64::Ratio) -> Result<uom::si::f64::Ratio, crate::TampinesError> { /* ... */ }
```

## Module `pipe`

# Steady-state gas duct: low-Mach friction and convective heat transfer

A single straight duct or pipe carrying a compressible gas at low Mach
number. Given a mass flow, the duct geometry, the inlet `(T, p)` and a
thermal boundary condition, it returns the outlet `(T, p)` together with
the dimensionless groups the answer was built from.

## What belongs here / what does not

- **Belongs:** single-duct steady friction (Churchill) and single-phase
  forced-convection heat transfer (Dittus-Boelter / Gnielinski) for a
  gas, plus the low-Mach energy balance that ties them together.
- **Does NOT belong:** packed-bed friction ([`super::kta_bed`]), the
  property correlations themselves ([`super::properties`], which only
  adapts `outram-park-fork-coolprop`), transient/CFD duct flow (that is
  `outram-park-fork-coolprop`'s `OPCPFluidArray`, re-exported as
  [`crate::compressible::CompressibleFluidArray`]), or two-phase flow.

## The low-Mach assumption, stated explicitly

Measured on 2026-08-11 (see
`tests::htr10_hot_duct_is_deeply_subsonic`), a 0.30 m HTR-10 primary
hot duct at the design flow runs at **Ma = 2.234e-2**; the pebble-bed
core itself, with its much larger free-flow area, runs at
**Ma = 1.4e-3** (measured in `outram-park-fork-coolprop`). Taking the
larger of the two, `Ma^2 = 5.0e-4`. This module therefore drops two
terms from the steady energy equation:

- the **kinetic-energy** term `u^2/2` against the enthalpy, which is
  `O(Ma^2) <= 5e-4` of it; and
- the **compressibility work** `u dp/dx / rho`, negligible for the same
  reason.

Both are therefore below the 0.1 % level for the hot duct and below
the 1e-6 level for the core. What is **not** dropped is the density
variation itself: `rho` is a full
function of `(T, p)` from the Helmholtz equation of state, and the
momentum balance keeps the **acceleration** term `G^2 (1/rho_out -
1/rho_in)` that a heated gas duct generates, reported separately from
friction so a caller can see its size. Nothing here is valid at
transonic conditions; for those, reach for
`outram-park-fork-coolprop`'s `SolverMode::HybridAllMach`.

## Correlations and their validity ranges

**Friction — Churchill (1977).** A single expression covering laminar,
transitional and turbulent flow in rough pipes, asymptotic to `64/Re`
below `Re ~ 2000` and to Colebrook-White above `Re ~ 4000`:

```text
A  = [ -2.457 ln( (7/Re)^0.9 + 0.27 e/D ) ]^16
B  = (37530/Re)^16
f  = 8 [ (8/Re)^12 + (A + B)^-1.5 ]^(1/12)      (Darcy friction factor)
```

Churchill, S. W., "Friction factor equation spans all fluid-flow
regimes", *Chemical Engineering* 84(24), 1977, pp. 91-92. Chosen over
Colebrook-White because it is explicit (no inner iteration) and does not
blow up or go complex in the laminar and transitional regions, which a
startup or natural-circulation transient will visit.

**Heat transfer — Dittus-Boelter.** `Nu = 0.023 Re^0.8 Pr^n`, with
`n = 0.4` when the gas is being heated and `n = 0.3` when cooled.
Stated validity: `Re > 10000`, `0.6 <= Pr <= 160`, `L/D >= 10`. Helium's
Prandtl number runs **0.658469 to 0.661835** across the HTR-10 core
(measured 2026-08-11, see [`super::properties`]) — inside that band,
but with under 10 % margin on its lower bound, which is the reason
[`HeatTransferCorrelation::Gnielinski`] is the default here.

**Heat transfer — Gnielinski (1976).** Using the Churchill Darcy factor
`f`:

```text
Nu = (f/8)(Re - 1000) Pr / [ 1 + 12.7 sqrt(f/8) (Pr^(2/3) - 1) ]
```

Gnielinski, V., "New equations for heat and mass transfer in turbulent
pipe and channel flow", *Int. Chem. Eng.* 16(2), 1976, pp. 359-368.
Stated validity: `3000 <= Re <= 5e6`, `0.5 <= Pr <= 2000`. It reaches
lower Reynolds numbers than Dittus-Boelter and covers helium's Prandtl
number with margin, so it is the default.

Both are **fully-developed, constant-property, smooth-tube** forms. No
entrance-length correction and no property-ratio (`(mu/mu_w)^0.14`-type)
correction is applied — for a gas with a large wall-to-bulk temperature
ratio, as in a reactor core channel, that omission is a real modelling
error and is recorded here rather than hidden.

## Status

**NOT VALIDATED.** The correlations are standard and cited, but this
implementation is checked only against analytic limits, self-consistency
and published correlation properties. AI-assisted draft pending human
review per `RESPONSIBLE_USE.md`.

```rust
pub mod pipe { /* ... */ }
```

### Types

#### Enum `HeatTransferCorrelation`

Which single-phase forced-convection correlation a [`GasDuct`] uses for
its wall heat-transfer coefficient.

Enum dispatch, not a trait object, per the workspace's mandatory "no
trait objects" Rust design rule. See the module docs for each
correlation's equation, citation and stated validity range.

```rust
pub enum HeatTransferCorrelation {
    Gnielinski,
    DittusBoelterHeating,
    DittusBoelterCooling,
}
```

##### Variants

###### `Gnielinski`

Gnielinski (1976), `3000 <= Re <= 5e6`, `0.5 <= Pr <= 2000`. The
default: it covers helium's `Pr ~ 0.66` with margin and reaches
lower Reynolds numbers than Dittus-Boelter.

###### `DittusBoelterHeating`

Dittus-Boelter, `Nu = 0.023 Re^0.8 Pr^0.4` (gas being heated).
Stated validity `Re > 10000`, `0.6 <= Pr <= 160`.

###### `DittusBoelterCooling`

Dittus-Boelter, `Nu = 0.023 Re^0.8 Pr^0.3` (gas being cooled).

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
    fn clone(self: &Self) -> HeatTransferCorrelation { /* ... */ }
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
    fn default() -> HeatTransferCorrelation { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &HeatTransferCorrelation) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `DuctThermalBoundary`

What the duct's wall does to the gas.

Enum dispatch, not a trait object.

```rust
pub enum DuctThermalBoundary {
    Adiabatic,
    HeatInput(uom::si::f64::Power),
    WallTemperature(uom::si::f64::ThermodynamicTemperature),
}
```

##### Variants

###### `Adiabatic`

No heat exchange with the wall. The gas still cools slightly on
expansion through the friction pressure drop (a real Joule-Thomson
effect the enthalpy balance captures, since constant enthalpy at
falling pressure is not constant temperature).

###### `HeatInput`

A prescribed **total** heat input to the gas over the whole duct,
W. Positive heats the gas. The heat-transfer correlation is still
evaluated (and reported) but does not set the duty.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Power` |  |

###### `WallTemperature`

A prescribed **uniform wall temperature**. The duty follows from the
convective correlation through an NTU closure,
`T_out = T_w - (T_w - T_in) exp(-hA / (mdot cp))`, so the gas
approaches the wall temperature asymptotically and can never
overshoot it.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::ThermodynamicTemperature` |  |

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
    fn clone(self: &Self) -> DuctThermalBoundary { /* ... */ }
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
    fn eq(self: &Self, other: &DuctThermalBoundary) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GasDuct`

A straight gas duct: geometry plus the heat-transfer correlation to use.

Plain data; the physics lives in [`GasDuct::solve_helium`]. Construct
with [`GasDuct::new_circular`] for the common round-pipe case, or by
filling the fields for a non-circular channel.

```rust
pub struct GasDuct {
    pub hydraulic_diameter: uom::si::f64::Length,
    pub flow_area: uom::si::f64::Area,
    pub wetted_perimeter: uom::si::f64::Length,
    pub length: uom::si::f64::Length,
    pub roughness: uom::si::f64::Length,
    pub heat_transfer: HeatTransferCorrelation,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hydraulic_diameter` | `uom::si::f64::Length` | Hydraulic diameter `D_h = 4 A / P`, metres. For a circular pipe this<br>is simply the internal diameter. |
| `flow_area` | `uom::si::f64::Area` | Flow (cross-sectional) area, m^2. |
| `wetted_perimeter` | `uom::si::f64::Length` | Wetted perimeter, metres — sets the wall area `P L` available for<br>heat transfer. |
| `length` | `uom::si::f64::Length` | Duct length, metres. |
| `roughness` | `uom::si::f64::Length` | Absolute wall roughness `e`, metres. Enters the Churchill friction<br>factor as the relative roughness `e/D_h`. Drawn steel is about<br>4.5e-5 m; a machined graphite channel is rougher. |
| `heat_transfer` | `HeatTransferCorrelation` | Which forced-convection correlation to use for the wall<br>heat-transfer coefficient. |

##### Implementations

###### Methods

- ```rust
  pub fn new_circular(diameter: Length, length: Length, roughness: Length) -> Self { /* ... */ }
  ```
  A circular pipe of the given internal `diameter`, `length` and

- ```rust
  pub fn wall_area(self: &Self) -> Area { /* ... */ }
  ```
  Wall (wetted) surface area available for heat transfer, `P L`, m^2.

- ```rust
  pub fn relative_roughness(self: &Self) -> Ratio { /* ... */ }
  ```
  Relative wall roughness `e / D_h`, dimensionless.

- ```rust
  pub fn solve_helium(self: &Self, mass_flow: MassRate, inlet_temperature: ThermodynamicTemperature, inlet_pressure: Pressure, boundary: DuctThermalBoundary) -> Result<GasDuctResult, TampinesError> { /* ... */ }
  ```
  Solve the duct for helium: outlet `(T, p)` and the full

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> GasDuct { /* ... */ }
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
    fn eq(self: &Self, other: &GasDuct) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GasDuctResult`

Everything [`GasDuct::solve_helium`] computed: the outlet state, the
pressure-drop split, and the dimensionless groups behind them.

Returned in full rather than as a bare outlet state so a caller can
check that the flow actually sat inside the correlations' validity
ranges — a pressure drop with `Re = 800` from a turbulent correlation is
a number, but not an answer.

```rust
pub struct GasDuctResult {
    pub inlet: super::properties::HeliumState,
    pub outlet: super::properties::HeliumState,
    pub pressure_drop: uom::si::f64::Pressure,
    pub friction_pressure_drop: uom::si::f64::Pressure,
    pub acceleration_pressure_drop: uom::si::f64::Pressure,
    pub mass_flux: super::MassFlux,
    pub reynolds: uom::si::f64::Ratio,
    pub prandtl: uom::si::f64::Ratio,
    pub darcy_friction_factor: uom::si::f64::Ratio,
    pub nusselt: uom::si::f64::Ratio,
    pub heat_transfer_coefficient: uom::si::f64::HeatTransfer,
    pub heat_duty: uom::si::f64::Power,
    pub inlet_velocity: uom::si::f64::Velocity,
    pub outlet_velocity: uom::si::f64::Velocity,
    pub outlet_mach: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inlet` | `super::properties::HeliumState` | Gas state at the duct inlet. |
| `outlet` | `super::properties::HeliumState` | Gas state at the duct outlet. |
| `pressure_drop` | `uom::si::f64::Pressure` | Total pressure drop, Pa (positive = pressure falls along the duct).<br>The sum of [`Self::friction_pressure_drop`] and<br>[`Self::acceleration_pressure_drop`]. |
| `friction_pressure_drop` | `uom::si::f64::Pressure` | Frictional part of the pressure drop, Pa,<br>`f (L/D_h) G^2 / (2 rho_mean)`. |
| `acceleration_pressure_drop` | `uom::si::f64::Pressure` | Acceleration ("momentum flux") part, Pa,<br>`G^2 (1/rho_out - 1/rho_in)`. Positive when the gas is heated and<br>therefore expands and speeds up; **negative** when it is cooled. |
| `mass_flux` | `super::MassFlux` | Superficial mass flux `G = mdot / A`, kg/(m^2 s). Constant along a<br>duct of fixed area regardless of expansion. |
| `reynolds` | `uom::si::f64::Ratio` | Reynolds number `Re = G D_h / mu` at the mean state, dimensionless. |
| `prandtl` | `uom::si::f64::Ratio` | Prandtl number at the mean state, dimensionless. |
| `darcy_friction_factor` | `uom::si::f64::Ratio` | Churchill Darcy friction factor at the mean state, dimensionless. |
| `nusselt` | `uom::si::f64::Ratio` | Nusselt number from the selected correlation, dimensionless. |
| `heat_transfer_coefficient` | `uom::si::f64::HeatTransfer` | Wall heat-transfer coefficient `h = Nu lambda / D_h`, W/(m^2 K). |
| `heat_duty` | `uom::si::f64::Power` | Net heat added to the gas over the duct, W (negative = removed). |
| `inlet_velocity` | `uom::si::f64::Velocity` | Bulk gas velocity at the inlet, m/s. |
| `outlet_velocity` | `uom::si::f64::Velocity` | Bulk gas velocity at the outlet, m/s. |
| `outlet_mach` | `uom::si::f64::Ratio` | Mach number at the outlet, dimensionless — the larger of the two for<br>a heated duct, and the number that justifies (or refutes) the<br>low-Mach assumption for a given case. |

##### Implementations

###### Methods

- ```rust
  pub fn heat_transfer_correlation_in_range(self: &Self, correlation: HeatTransferCorrelation) -> bool { /* ... */ }
  ```
  Whether the flow sat inside the selected heat-transfer

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> GasDuctResult { /* ... */ }
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
    fn eq(self: &Self, other: &GasDuctResult) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `churchill_friction_factor`

Churchill (1977) Darcy friction factor, dimensionless, from the Reynolds
number and the relative roughness `e/D_h`.

Valid across all flow regimes: it is asymptotic to `64/Re` in laminar
flow and to Colebrook-White in the fully turbulent region. See the
module docs for the equation and the citation.

Errors with [`TampinesError::InvalidInput`] for a non-positive or
non-finite Reynolds number, or a negative relative roughness.

```rust
pub fn churchill_friction_factor(reynolds: uom::si::f64::Ratio, relative_roughness: uom::si::f64::Ratio) -> Result<uom::si::f64::Ratio, crate::TampinesError> { /* ... */ }
```

#### Function `nusselt_number`

Nusselt number from the selected correlation, dimensionless.

`darcy_friction_factor` is only used by
[`HeatTransferCorrelation::Gnielinski`]. See the module docs for each
correlation's equation, citation and stated validity range; validity is
**not** enforced here (use
[`GasDuctResult::heat_transfer_correlation_in_range`]), but a
non-positive result is rejected as unphysical.

```rust
pub fn nusselt_number(correlation: HeatTransferCorrelation, reynolds: uom::si::f64::Ratio, prandtl: uom::si::f64::Ratio, darcy_friction_factor: uom::si::f64::Ratio) -> Result<uom::si::f64::Ratio, crate::TampinesError> { /* ... */ }
```

## Module `properties`

# Helium thermophysical-property adapter for the HTR-10 primary circuit

A thin, `uom`-typed façade over [`outram_park_fork_coolprop`]'s helium
equation of state and transport correlations, evaluated at a
`(temperature, pressure)` state point.

## What belongs here

Pure, stateless property lookups for **gaseous helium**: density,
dynamic viscosity, thermal conductivity, isobaric specific heat,
specific enthalpy, speed of sound, and the derived Prandtl number. Each
is a single call with no hidden state, so a component model can evaluate
properties at whatever mean state it decides is appropriate.

## What does NOT belong here

- **Property *correlations* themselves.** The Helmholtz EOS and the
  transport fits live in `outram-park-fork-coolprop`; this module only
  adapts them. If a correlation is wrong, fix it there, not here.
- **Component physics** (pressure drop, heat transfer). Those live in
  the sibling modules [`crate::gas_phase::pipe`] and
  [`crate::gas_phase::kta_bed`].
- **Other gases.** The adapter is deliberately helium-only so the
  documented validity ranges below mean something. `coolprop`'s
  [`Fluid`] enum covers ~137 fluids if a caller needs another one.

## Provenance of the underlying correlations

Read out of `outram-park-fork-coolprop` on 2026-08-11:

- **Equation of state** — `crates/outram-park-fork-coolprop/src/fluids/helium.rs`,
  a reduced-Helmholtz (Span-Wagner form) EOS with Power + Gaussian
  residual terms only (no non-analytic critical term). That crate's
  `tests/helium_reference.rs` reproduces CoolProp's own tabulated
  triple-liquid, triple-vapour and **critical-point** pressures to
  better than 1e-3 relative.
- **Dynamic viscosity** — Arp, McCarty & Friend, *NIST Technical Note
  1334* (1998), as implemented by CoolProp's
  `viscosity_helium_hardcoded` and ported in
  `outram-park-fork-coolprop/src/transport.rs` (`helium_viscosity`).
- **Thermal conductivity** — Hands & Arp, as implemented by CoolProp's
  `conductivity_hardcoded_helium` and ported in the same file
  (`helium_conductivity`). The near-critical enhancement term `lambda_c`
  (only active over 3.5-12 K) is omitted upstream, which is irrelevant
  at HTGR temperatures.

## Why this module exists (a known defect it replaces)

The `htgr_sim_v1` example in the **read-only** downstream crate
`outram-park-digital-twin-engine` hard-codes a *constant* helium dynamic
viscosity even though the Arp-McCarty-Friend correlation is available in
`outram-park-fork-coolprop`. That remains a defect in that crate and is
**not** fixed by this module; this module is the correct replacement
path a future fix should call. Tracked in the workspace bead tracker
(see `op-wqk.9.1` and the follow-up filed on 2026-08-11).

## Validity range

The public functions guard for `T > 0`, `p > 0` and finite inputs, and
reject anything outside **2.2 K to 1500 K** and **1 Pa to 100 MPa** with
[`TampinesError::InvalidInput`]. That envelope comfortably contains the
HTR-10 primary circuit (3.0 MPa, 523.15 K core inlet to 973.15 K core
outlet). The bounds are a *usage* guard on the adapter, not a claim that
the upstream fits are equally accurate everywhere inside them — the
viscosity correlation in particular switches branch at 100 K and freezes
its `ln(T)` argument above 300 K.

## Status

**NOT VALIDATED against HTR-10 measurements.** The tests below are
code-to-code and self-consistency checks against the upstream CoolProp
port plus ideal-gas limits. AI-assisted draft pending human review per
`RESPONSIBLE_USE.md`.

```rust
pub mod properties { /* ... */ }
```

### Modules

## Module `htr10_design_point`

The HTR-10 primary-circuit design point, as the state inputs this module
takes.

Values from the IAEA HTR-10 benchmark description (Open tier): helium at
**3.0 MPa**, **4.3 kg/s** total core flow, core inlet **250 C**
(523.15 K), core outlet **700 C** (973.15 K), **10 MW** thermal.

```rust
pub mod htr10_design_point { /* ... */ }
```

### Functions

#### Function `pressure`

Primary-circuit helium pressure, 3.0 MPa.

```rust
pub fn pressure() -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `core_inlet_temperature`

Core inlet temperature, 250 C = 523.15 K.

```rust
pub fn core_inlet_temperature() -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

#### Function `core_outlet_temperature`

Core outlet temperature, 700 C = 973.15 K.

```rust
pub fn core_outlet_temperature() -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

#### Function `mass_flow_rate`

Total core helium mass flow rate, 4.3 kg/s.

```rust
pub fn mass_flow_rate() -> uom::si::f64::MassRate { /* ... */ }
```

#### Function `thermal_power`

Core thermal power, 10 MW.

```rust
pub fn thermal_power() -> uom::si::f64::Power { /* ... */ }
```

### Types

#### Type Alias `SpecificEnthalpy`

Specific enthalpy (energy per unit mass), J/kg.

A readable alias for `uom`'s `AvailableEnergy`, whose name does not
suggest "specific enthalpy" to a thermal-hydraulics reader.

```rust
pub type SpecificEnthalpy = uom::si::f64::AvailableEnergy;
```

#### Struct `HeliumState`

Every helium property this module can report, evaluated at one
`(temperature, pressure)` state point.

Returned as a bundle by [`helium_state`] because the underlying EOS
flash is the expensive step and yields all of these at once — asking for
density and then cp separately solves the same Newton iteration twice.

```rust
pub struct HeliumState {
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub pressure: uom::si::f64::Pressure,
    pub density: uom::si::f64::MassDensity,
    pub dynamic_viscosity: uom::si::f64::DynamicViscosity,
    pub thermal_conductivity: uom::si::f64::ThermalConductivity,
    pub specific_heat_cp: uom::si::f64::SpecificHeatCapacity,
    pub specific_enthalpy: SpecificEnthalpy,
    pub speed_of_sound: uom::si::f64::Velocity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature the state was evaluated at, kelvin. |
| `pressure` | `uom::si::f64::Pressure` | Pressure the state was evaluated at, pascal. |
| `density` | `uom::si::f64::MassDensity` | Mass density, kg/m^3. At the HTR-10 design point (3.0 MPa) helium<br>runs roughly 1.2-2.8 kg/m^3 across the 250-700 C core rise. |
| `dynamic_viscosity` | `uom::si::f64::DynamicViscosity` | Dynamic viscosity, Pa s. Order 3-4 x 10^-5 Pa s at HTGR<br>temperatures — it rises with temperature, unlike a liquid's. |
| `thermal_conductivity` | `uom::si::f64::ThermalConductivity` | Thermal conductivity, W/(m K). Helium conducts unusually well for a<br>gas (order 0.3 W/(m K) at HTGR temperatures, ~10x air). |
| `specific_heat_cp` | `uom::si::f64::SpecificHeatCapacity` | Isobaric specific heat `c_p`, J/(kg K). Near-constant at about<br>5193 J/(kg K) for a monatomic ideal gas (`5R/2M`). |
| `specific_enthalpy` | `SpecificEnthalpy` | Specific enthalpy, J/kg, on the upstream EOS's own reference-state<br>convention. **Only enthalpy *differences* are meaningful** — never<br>compare an absolute value here against another property library. |
| `speed_of_sound` | `uom::si::f64::Velocity` | Speed of sound, m/s. Used to form the Mach number that justifies the<br>low-Mach treatment in [`crate::gas_phase::pipe`]. |

##### Implementations

###### Methods

- ```rust
  pub fn prandtl(self: &Self) -> Ratio { /* ... */ }
  ```
  Prandtl number `Pr = c_p mu / lambda`, dimensionless.

- ```rust
  pub fn kinematic_viscosity_m2_per_s(self: &Self) -> f64 { /* ... */ }
  ```
  Kinematic viscosity `nu = mu / rho`, m^2/s, as a plain `f64` in SI

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> HeliumState { /* ... */ }
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
    fn eq(self: &Self, other: &HeliumState) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `helium_state`

Full helium thermophysical state at temperature `t` and pressure `p`.

This is the primary entry point — the single-property helpers below all
delegate to it. Inputs must lie in 2.2-1500 K and 1 Pa-100 MPa (see the
module docs); anything else returns [`TampinesError::InvalidInput`].

Errors with [`TampinesError::Numerical`] if the upstream `(p, T)` flash
fails to converge, and with [`TampinesError::Unphysical`] if the
transport correlations decline to return a value (which for helium means
the state landed outside their supported region).

```rust
pub fn helium_state(t: uom::si::f64::ThermodynamicTemperature, p: uom::si::f64::Pressure) -> Result<HeliumState, crate::TampinesError> { /* ... */ }
```

#### Function `helium_state_ph`

Full helium thermophysical state from **pressure and specific
enthalpy**, the natural pair for a steady-flow energy balance.

A duct that adds `Q` watts to a flow of `mdot` kg/s raises the specific
enthalpy by exactly `Q/mdot`; recovering the temperature from `(p, h)`
keeps that balance exact as `c_p` varies, where a `Q = mdot c_p dT`
shortcut would not.

`enthalpy` must be on the **same reference-state convention** as
[`HeliumState::specific_enthalpy`] — i.e. it must have come from this
module (or from `outram-park-fork-coolprop` directly). Absolute
enthalpies from another property library will silently give a wrong
temperature.

Errors with [`TampinesError::Numerical`] if the `(p, h)` flash does not
converge (which for helium means the requested enthalpy is outside the
EOS's reach), and otherwise as [`helium_state`].

```rust
pub fn helium_state_ph(p: uom::si::f64::Pressure, enthalpy: SpecificEnthalpy) -> Result<HeliumState, crate::TampinesError> { /* ... */ }
```

#### Function `helium_density`

Helium mass density, kg/m^3, at `(t, p)`. See [`helium_state`] for the
accepted input envelope and the error cases.

```rust
pub fn helium_density(t: uom::si::f64::ThermodynamicTemperature, p: uom::si::f64::Pressure) -> Result<uom::si::f64::MassDensity, crate::TampinesError> { /* ... */ }
```

#### Function `helium_viscosity`

Helium dynamic viscosity, Pa s, at `(t, p)` (Arp, McCarty & Friend,
NIST TN-1334). See [`helium_state`] for the accepted input envelope.

This is the correct replacement for any hard-coded constant helium
viscosity — see the module-level note on `htgr_sim_v1`.

```rust
pub fn helium_viscosity(t: uom::si::f64::ThermodynamicTemperature, p: uom::si::f64::Pressure) -> Result<uom::si::f64::DynamicViscosity, crate::TampinesError> { /* ... */ }
```

#### Function `helium_thermal_conductivity`

Helium thermal conductivity, W/(m K), at `(t, p)` (Hands & Arp). See
[`helium_state`] for the accepted input envelope.

```rust
pub fn helium_thermal_conductivity(t: uom::si::f64::ThermodynamicTemperature, p: uom::si::f64::Pressure) -> Result<uom::si::f64::ThermalConductivity, crate::TampinesError> { /* ... */ }
```

#### Function `helium_cp`

Helium isobaric specific heat `c_p`, J/(kg K), at `(t, p)`. See
[`helium_state`] for the accepted input envelope.

```rust
pub fn helium_cp(t: uom::si::f64::ThermodynamicTemperature, p: uom::si::f64::Pressure) -> Result<uom::si::f64::SpecificHeatCapacity, crate::TampinesError> { /* ... */ }
```

#### Function `helium_prandtl`

Helium Prandtl number, dimensionless, at `(t, p)`. See [`helium_state`]
for the accepted input envelope.

```rust
pub fn helium_prandtl(t: uom::si::f64::ThermodynamicTemperature, p: uom::si::f64::Pressure) -> Result<uom::si::f64::Ratio, crate::TampinesError> { /* ... */ }
```

#### Function `reynolds_number`

Reynolds number `Re = G D / mu` for a duct, dimensionless, from the mass
flux `G` \[kg/(m^2 s)\], the hydraulic diameter `d` and the state's
dynamic viscosity.

Written in the mass-flux form deliberately: `G = mdot/A` is constant
along a duct of fixed area even when the gas expands, so `Re` computed
this way does not silently depend on which density the caller picked.

```rust
pub fn reynolds_number(mass_flux_kg_per_m2_s: f64, hydraulic_diameter: uom::si::f64::Length, dynamic_viscosity: uom::si::f64::DynamicViscosity) -> uom::si::f64::Ratio { /* ... */ }
```

### Types

#### Type Alias `MassFlux`

Mass flux `G = mdot / A`, kg/(m^2 s) — mass flow per unit cross-section.

`uom` ships no named alias for this dimension, so it is spelled out
here. The same alias is defined in the read-only
`outram-park-digital-twin-engine` KTA module; this is an independent
definition of the identical dimension, not a shared type.

```rust
pub type MassFlux = uom::si::Quantity<uom::si::ISQ<uom::typenum::N2, uom::typenum::P1, uom::typenum::N1, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

#### Type Alias `PressureGradient`

Pressure gradient `dp/dx`, Pa/m.

`uom` ships no named alias for this dimension, so it is spelled out
here.

```rust
pub type PressureGradient = uom::si::Quantity<uom::si::ISQ<uom::typenum::N2, uom::typenum::P1, uom::typenum::N2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

### Re-exports

#### Re-export `Circulator`

```rust
pub use circulator::Circulator;
```

#### Re-export `CirculatorDuty`

```rust
pub use circulator::CirculatorDuty;
```

#### Re-export `KtaBed`

```rust
pub use kta_bed::KtaBed;
```

#### Re-export `KtaBedResult`

```rust
pub use kta_bed::KtaBedResult;
```

#### Re-export `GasDuct`

```rust
pub use pipe::GasDuct;
```

#### Re-export `GasDuctResult`

```rust
pub use pipe::GasDuctResult;
```

#### Re-export `HeatTransferCorrelation`

```rust
pub use pipe::HeatTransferCorrelation;
```

#### Re-export `helium_cp`

```rust
pub use properties::helium_cp;
```

#### Re-export `helium_density`

```rust
pub use properties::helium_density;
```

#### Re-export `helium_prandtl`

```rust
pub use properties::helium_prandtl;
```

#### Re-export `helium_state`

```rust
pub use properties::helium_state;
```

#### Re-export `helium_thermal_conductivity`

```rust
pub use properties::helium_thermal_conductivity;
```

#### Re-export `helium_viscosity`

```rust
pub use properties::helium_viscosity;
```

#### Re-export `HeliumState`

```rust
pub use properties::HeliumState;
```

#### Re-export `SpecificEnthalpy`

```rust
pub use properties::SpecificEnthalpy;
```

## Module `heat_transfer`

Shared heat-transfer correlation access point.

Thin re-export of the Nusselt-number correlation dispatch enum from
[`tuas_boussinesq_solver`] (used by [`crate::single_phase`]'s lumped
pipe/component model) and the thermal-conductivity/viscosity transport
functions from [`outram_park_fork_coolprop`] (used by
[`crate::compressible`]'s finite-volume model). This module adds no new
correlations -- see each backend crate's own documentation for the
correlations' physics and validated ranges.

```rust
pub mod heat_transfer { /* ... */ }
```

### Re-exports

#### Re-export `NusseltCorrelation`

Dispatches which Nusselt-number correlation a single-phase pipe/component
segment uses (Gnielinski, Dittus-Boelter-derived, Wakao, CIET-specific,
fixed, ...).

Alias for [`tuas_boussinesq_solver`]'s `NusseltCorrelation`.

```rust
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation;
```

#### Re-export `TuasLibError`

Error type returned by [`NusseltCorrelation`]'s methods. Re-exported from
`tuas_boussinesq_solver` for convenience.

```rust
pub use tuas_boussinesq_solver::tuas_lib_error::TuasLibError;
```

#### Re-export `viscosity`

Dynamic viscosity `μ` \[Pa·s\] of a CoolProp-backed fluid at temperature
`T` \[K\] and mass density `ρ` \[kg/m³\], or `None` if that fluid has no
supported viscosity model. Re-exported from `outram-park-fork-coolprop`.

```rust
pub use outram_park_fork_coolprop::viscosity;
```

#### Re-export `conductivity`

Thermal conductivity `λ` \[W/(m·K)\] of a CoolProp-backed fluid at
temperature `T` \[K\] and mass density `ρ` \[kg/m³\], or `None` if that
fluid has no supported conductivity model. Re-exported from
`outram-park-fork-coolprop`.

```rust
pub use outram_park_fork_coolprop::conductivity;
```

#### Re-export `FluidTransport`

Which viscosity/conductivity correlation backs a given
[`outram_park_fork_coolprop::Fluid`] (48 of 137 fluids have one).
Re-exported from `outram-park-fork-coolprop`.

```rust
pub use outram_park_fork_coolprop::FluidTransport;
```

## Module `hem`

Homogeneous Equilibrium Model (HEM) two-phase steam/water state.

Thin re-export of [`tampines_steam_tables`]'s validated HEM control-volume
type -- the pressure/enthalpy/quality steam-water state used by
[`crate::critical_flow`]'s choked-flow solvers and, in future, TAMPINES's
wider multiphase thermal-hydraulics work (HEM -> drift-flux -> CHF, see
the workspace's `op-21g.6` roadmap bead; deferred stubs for that land
separately, not in this module).

```rust
pub mod hem { /* ... */ }
```

### Re-exports

#### Re-export `TampinesSteamTableCV`

The HEM two-phase steam/water control volume: pressure, specific
enthalpy, specific entropy, quality, and derived properties (viscosity,
speed of sound, critical mass flux, ...).

Alias for [`tampines_steam_tables`]'s `TampinesSteamTableCV` -- see that
type's own documentation for its full getter/setter surface and
construction paths ((p,h), (p,s), (h,s), (T,p,x), saturation-line
constructors).

```rust
pub use tampines_steam_tables::prelude::TampinesSteamTableCV as HemSteamCv;
```

## Module `humid_air`

Humid-air (moist-air) psychrometrics for cooling towers and secondary-loop
air-side calculations.

Thin `uom`-typed wrapper over [`outram_park_fork_coolprop::humid_air`],
this workspace's `HAPropsSI`-equivalent port (ASHRAE RP-1485). See that
module's own documentation for the physics being wrapped -- coverage,
valid range (liquid-water branch only, `T > 273.16 K`), and caveats
(notably: `entropy`'s absolute reference-state convention has not been
independently cross-checked, though its temperature dependence is
verified).

```rust
pub mod humid_air { /* ... */ }
```

### Types

#### Struct `HumidAirState`

A fully-resolved humid-air (moist-air) state, dimensioned via `uom`.

All extensive properties ([`Self::enthalpy`], [`Self::entropy`],
[`Self::volume`]) are per kilogram of *dry* air, matching CoolProp's
`HAPropsSI` convention.

```rust
pub struct HumidAirState {
    pub t_dry_bulb: uom::si::f64::ThermodynamicTemperature,
    pub pressure: uom::si::f64::Pressure,
    pub water_mole_fraction: uom::si::f64::Ratio,
    pub humidity_ratio: uom::si::f64::Ratio,
    pub relative_humidity: uom::si::f64::Ratio,
    pub enthalpy: uom::si::f64::AvailableEnergy,
    pub entropy: uom::si::f64::SpecificHeatCapacity,
    pub volume: uom::si::f64::SpecificVolume,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_dry_bulb` | `uom::si::f64::ThermodynamicTemperature` | Dry-bulb temperature. |
| `pressure` | `uom::si::f64::Pressure` | Total (barometric) pressure. |
| `water_mole_fraction` | `uom::si::f64::Ratio` | Water-vapour mole fraction `ψ_w` [-]. |
| `humidity_ratio` | `uom::si::f64::Ratio` | Humidity ratio `W` [kg water / kg dry air]. |
| `relative_humidity` | `uom::si::f64::Ratio` | Relative humidity `R` [0, 1]. |
| `enthalpy` | `uom::si::f64::AvailableEnergy` | Specific enthalpy, per kg dry air. |
| `entropy` | `uom::si::f64::SpecificHeatCapacity` | Specific entropy, per kg dry air. See the module doc's caveat on this<br>port's absolute reference-state convention. |
| `volume` | `uom::si::f64::SpecificVolume` | Specific volume, per kg dry air. |

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
    fn clone(self: &Self) -> HumidAirState { /* ... */ }
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
    fn eq(self: &Self, other: &HumidAirState) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `state_from_t_p_w`

Resolve a humid-air state from dry-bulb temperature, pressure, and
humidity ratio `W` [kg water / kg dry air] -- the input triple most
HVAC/cooling-tower and FHR-secondary-loop-air calculations use.

```rust
pub fn state_from_t_p_w(t: uom::si::f64::ThermodynamicTemperature, p: uom::si::f64::Pressure, w: uom::si::f64::Ratio) -> Result<HumidAirState, HumidAirError> { /* ... */ }
```

#### Function `state_from_t_p_r`

Resolve a humid-air state from dry-bulb temperature, pressure, and
relative humidity `R` [0, 1].

```rust
pub fn state_from_t_p_r(t: uom::si::f64::ThermodynamicTemperature, p: uom::si::f64::Pressure, r: uom::si::f64::Ratio) -> Result<HumidAirState, HumidAirError> { /* ... */ }
```

#### Function `state_from_inputs`

Resolve a full [`HumidAirState`] from any three input constraints
`outram_park_fork_coolprop::humid_air` accepts (`(T, p, {W|R|ψ_w|T_dp|T_wb})`
in any order) -- the general escape hatch for callers who need the dew-point
or wet-bulb input forms that [`state_from_t_p_w`]/[`state_from_t_p_r`] don't
cover. Note: performs one backend solve per output field (8 total), since
the backend only exposes a single-output `ha_props` entry point.

```rust
pub fn state_from_inputs(in1: outram_park_fork_coolprop::humid_air::HaInput, in2: outram_park_fork_coolprop::humid_air::HaInput, in3: outram_park_fork_coolprop::humid_air::HaInput) -> Result<HumidAirState, HumidAirError> { /* ... */ }
```

### Re-exports

#### Re-export `HumidAirError`

```rust
pub use outram_park_fork_coolprop::humid_air::HumidAirError;
```

#### Re-export `HumidAirParam`

```rust
pub use outram_park_fork_coolprop::humid_air::HumidAirParam as CoolPropHumidAirParam;
```

## Module `multiphase_1d`

One-dimensional two-phase system-code solvers, reduced from the OUTRAM-FOAM
3-D multiphase reference.

# What is in here

Two 1-D transient solvers for compressible steam/water pipe flow, one step
above the [`crate::hem`] homogeneous-equilibrium baseline:

| Solver | Equations | Slip between phases | Thermal non-equilibrium |
|---|---|---|---|
| [`crate::hem`] (HEM) | 3 | none (`u_g = u_l`) | none (both saturated) |
| [`drift_flux::DriftFlux1d`] | 4 | **algebraic** (`u_g − j = U_dm`) | none |
| [`two_fluid::TwoFluid1d`] | 6 | **dynamic** (own momentum equation) | **yes** (own energy equation) |

That is the standard system-code ladder — each rung relaxes one assumption
of the rung below and costs one more transported field per phase.

# The trace-back constraint, and exactly how it is honoured

Beads `op-dt3.12` and `op-dt3.13` impose a hard constraint: *1-D models must
trace back to the OUTRAM-FOAM 3-D reference in
[`outram_foam_multiphase`], never be invented independently.* Being precise
about what that does and does not mean here matters, because the two codes
do not solve the same equations.

**What is reused verbatim** — the *algebraic closures*, which are
dimensionality-agnostic:

- [`outram_foam_multiphase::drift_flux::SlipModel`] supplies the drift
  velocity `U_dm` for [`drift_flux::DriftFlux1d`]. Its `ZuberFindlay`
  variant is the classic `v_d = C₀ j + V_gj`.
- [`outram_foam_multiphase::two_fluid::DragModel`] supplies the volumetric
  drag coefficient `K_d` for [`two_fluid::TwoFluid1d`], including the
  Schiller-Naumann and Wen-Yu correlations ported from OpenFOAM.

These are consumed through the reference crate's own public API, so an
upstream correction to a correlation propagates here automatically and a
divergence cannot silently open up.

**What is NOT reused, and why.** The 3-D reference is
**incompressible with constant per-phase density** — its
`DriftFluxMixture` stores `ρ_d` and `ρ_c` as scalars, and its mixture
density is the linear rule `ρ_m = α ρ_d + (1−α) ρ_c`. A pipe blowdown is
the opposite regime: the pressure falls by two orders of magnitude, water
flashes to steam, and both phase densities move by orders of magnitude
along the transient. So the *field equations* here are compressible and the
densities come from an IAPWS-IF97 flash, not from the reference's linear
rule. Reducing an incompressible formulation to 1-D and then bolting
compressibility onto it would be a worse kind of "independent invention"
than being explicit about the difference.

The honest summary: **closures traced, field equations re-derived for
compressible flow, and the difference stated rather than papered over.**

# Numerical method, and why it is not explicit

Both solvers use a **semi-implicit pressure-based march**, the method
RELAP5 and TRACE use and for the same reason: an explicit compressible
march is limited by the acoustic CFL `Δt < Δx/(|u| + c)`, and in subcooled
water `c ≈ 1500 m/s`, so a 17 cm cell would need `Δt ≈ 10⁻⁴ s` purely to
chase sound waves that carry almost no energy. Each step:

1. Transport the conserved quantities explicitly with **donor-cell**
   (first-order upwind) fluxes at the old-time velocities.
2. Linearise the mass residual in pressure through the compressibility
   `ψ = ∂ρ/∂p|_h` and solve the resulting **tridiagonal** pressure equation
   with [`thomas_solve`].
3. Correct the velocities with the new pressure gradient.
4. Recover the thermodynamic state from the new pressure and the
   transported energy.

First-order upwind is a deliberate choice, not an oversight: it is
monotone, which matters far more than formal order at a flashing front,
and it is what the reference system codes use. It is also **diffusive**,
and that shows up as a smeared rarefaction wave — see the measured results
in the Edwards cases.

# Honest scope — what these do NOT do

Stated here so it is not discovered late:

- **No wall heat transfer.** Both solvers are adiabatic. A blowdown is
  dominated by depressurisation, not by wall heat, but a rewetting or
  reflood problem is not solvable with these as they stand.
- **No interfacial area transport.** [`two_fluid::TwoFluid1d`] takes a
  prescribed bubble diameter, so its drag and its interfacial heat transfer
  do not respond to a changing flow regime. That absence is not cosmetic: it
  is what makes the six-equation solver **refuse** a transient started at a
  low void fraction, because a phase with almost no interfacial area cannot
  shed its reversible expansion work fast enough to stay inside the bounded
  metastable branches. Measured boundary and diagnosis in
  `two_fluid_tests::six_equation_march_refuses_where_a_phase_cannot_shed_its_expansion_work`.
- **No flow-regime map.** A real system code selects closures from a
  regime map (bubbly / slug / annular / mist). Here the closure is whatever
  the caller passed, everywhere, for the whole transient.
- **No counter-current flow limitation, no critical-flow model of their
  own.** The break boundary reuses the crate's existing HEM critical-flow
  dispatcher, so the *break* is HEM even when the *pipe* is drift-flux or
  two-fluid. This is a real modelling inconsistency and it is called out
  again at the boundary condition itself.
- **One validation case, and only one.** As of 2026-08-11
  [`drift_flux::DriftFlux1d`] is compared against the digitised
  Edwards–O'Brien experimental GS-1 pressure curve by
  `edwards_drift_flux_gs1_pressure_history` (see `edwards_tests.rs` for the
  full V&V record: RMSE 29.0 psia over 0–0.30 s, plateau 354.3 psia inside
  the experimental band, and an explicit list of what is *not* gated).
  Every other test in this module is **verification** — closed-form
  identities and invariants, compared against no experiment — and
  [`two_fluid::TwoFluid1d`], implemented 2026-08-12, is compared against no
  experiment at all. Its `two_fluid_tests.rs` battery is invariants,
  degenerate limits and one **documented disagreement** with
  [`drift_flux::DriftFlux1d`] in the single-phase limit; read that file's
  header for what it does and does not establish.

# Units

Public constructors and accessors are `uom`-typed. The inner marching loops
carry raw `f64` in strict SI — pascal, kelvin, `J/kg`, `kg/m³`, `m/s` —
because they are per-cell per-timestep and `uom` round-trips inside them
cost more than they buy. Every raw-`f64` boundary says so.

```rust
pub mod multiphase_1d { /* ... */ }
```

### Modules

## Module `drift_flux`

The 1-D **drift-flux** solver — four equations, algebraic slip.

# The model, in one paragraph

Drift flux is the first rung above HEM. It keeps the two phases in *thermal*
equilibrium (one temperature, both on the saturation line) but lets them
move at *different velocities*, with the difference given by an algebraic
closure rather than by a second momentum equation. That buys the two things
HEM cannot represent — vapour rising through slower liquid, and the void
fraction consequently differing from the no-slip value at the same quality —
for one extra transported field.

# The four equations

On a pipe of constant area `A`, with `ρ_m` the mixture density, `u_m` the
mass-averaged (centre-of-mass) velocity and `h_m` the mixture enthalpy:

`∂ρ_m/∂t + ∂(ρ_m u_m)/∂x = 0`  (mixture mass)

`∂(ρ_m u_m)/∂t + ∂(ρ_m u_m²)/∂x = −∂p/∂x + ρ_m g_x − F_wall − ∂Φ/∂x`  (mixture momentum)

`∂(ρ_m h_m)/∂t + ∂(ρ_m u_m h_m)/∂x = ∂p/∂t`  (mixture energy)

`∂(α ρ_g)/∂t + ∂(α ρ_g v_g)/∂x = Γ_g`  (gas mass)

The momentum equation's `Φ` is the **drift momentum flux**, the term that
distinguishes drift flux from HEM-with-a-void-equation:

`Φ = α ρ_g ρ_l U_dm² / ((1−α) ρ_m)`

It is the extra momentum carried by the phases moving at different speeds.
Dropping it — as a naive "HEM plus a void equation" would — leaves the
momentum equation inconsistent with the velocity field the void equation is
using.

# How the slip closure is traced back

[`outram_foam_multiphase::drift_flux::SlipModel`] supplies `U_dm`, the
dispersed-phase velocity relative to the **mixture volumetric flux** `j`.
From it the phase velocities are reconstructed *exactly*, not
approximately. Starting from `ρ_m u_m = α ρ_g v_g + (1−α) ρ_l v_l`,
`j = α v_g + (1−α) v_l` and `U_dm = v_g − j`, eliminating `v_l` gives

`ρ_m u_m = j ρ_m + α U_dm (ρ_g − ρ_l)`

hence

`j = u_m + α U_dm (ρ_l − ρ_g) / ρ_m`,  `v_g = j + U_dm`,
`v_l = j − α U_dm / (1−α)`.

**One inherited approximation, stated plainly.** The reference's
`ZuberFindlay` arm computes `U_dm = (C₀ − 1) u_m + V_gj`, i.e. it uses the
mixture *velocity* `u_m` where the correlation calls for the volumetric
*flux* `j` — its own doc comment says so ("the volumetric-flux surrogate").
Since `j ≠ u_m` whenever the phase densities differ, this port inherits the
approximation rather than silently correcting it: correcting a closure
inside a consumer is how two codes quietly stop agreeing. The
reconstruction above is exact; only `U_dm` itself carries the surrogate.

# The interfacial mass transfer `Γ_g`

A four-equation model has one energy equation, so it cannot carry
independent phase enthalpies — the phases are in thermal equilibrium by
construction. `Γ_g` is therefore not free: it is whatever makes the
transported void consistent with the equilibrium void implied by
`(p, h_m)`. This solver relaxes toward that,

`Γ_g = (α_eq − α) ρ_g / τ`,

with `τ` the [`vapour_relaxation_time`](DriftFlux1d::vapour_relaxation_time).
As `τ → 0` the model collapses to HEM-with-slip (void pinned to
equilibrium); at finite `τ` the void lags, which is the physically real
delay of a flashing front. `τ` is a **model parameter, not a measured
constant** — it is exposed, defaulted, and documented as such rather than
buried in a literal.

# Honest scope

[`crate::multiphase_1d`] lists what applies to both solvers (no wall heat
transfer, no flow-regime map, no interfacial-area transport, HEM break
model). Specific to this solver:

- **The drift momentum flux is explicit**, at old-time `U_dm`. At a fast
  front it is a lagged term.
- **`α` outside `[0, 1]` is refused, not clipped.** Clipping would present
  a CFL violation as a plausible answer.

```rust
pub mod drift_flux { /* ... */ }
```

### Types

#### Enum `AxialBoundary`

What closes an end of the pipe.

Enum dispatch per the workspace rule: the set of 1-D end conditions is
closed, and adding one must force every `match` to be revisited.

```rust
pub enum AxialBoundary {
    Closed,
    PrescribedVelocity(f64),
    ChokedOutlet {
        area_fraction: f64,
        ambient_pressure: f64,
    },
    ReservoirInlet {
        stagnation_pressure: f64,
        stagnation_enthalpy: f64,
    },
    PressureOutlet {
        pressure: f64,
    },
}
```

##### Variants

###### `Closed`

A rigid closed end: `u = 0`, zero-gradient pressure. The `x = 0` end of
the Edwards–O'Brien pipe.

###### `PrescribedVelocity`

A prescribed face velocity \[m/s\], positive in `+x`, with
zero-gradient pressure.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `ChokedOutlet`

A **choked (critical) outlet** through a break of area
`area_fraction × A`.

Each step the adjacent cell's `(p, h)` is handed to the crate's
existing HEM critical-flow dispatcher for the throat mass flux `G*`,
and the equivalent full-face velocity `u = G* × area_fraction / ρ` is
imposed. The pressure keeps a zero-gradient condition, so it floats on
the compressibility diagonal — a blowdown has no Dirichlet pressure
anywhere; the mass depletion sets `p`.

**This is a modelling inconsistency and it is deliberate.** The break
model is HEM even though the pipe is drift-flux, because the
critical-flow dispatcher is the piece of this crate actually exercised
against a reference. Substituting an unvalidated drift-flux choking
model would trade a known inconsistency for an unknown one. It is
called out here, in the module docs, and in the Edwards case, so it
cannot be mistaken for an oversight.

**What "exercised against a reference" means here**, re-read from
`tampines-steam-tables`'s test source on 2026-08-11. The call path was
verified, not assumed: the Moody validator computes its test value via
`TampinesSteamTableCV::get_stagnation_critical_mass_flux`, which calls
`get_crit_pressure_and_massflux`, which calls
`get_critical_pressure_and_mass_flux_multiphase_ph` -- the same
dispatcher this boundary condition consumes -- and Zaloudek's
`generic_multiphase_stagnation` tests call
`get_crit_pressure_and_massflux` likewise. Note the Moody coverage of
the dispatcher is *incidental*: those tests predate the dispatcher and
reach it only because the OOP getter was rewired through it, so a
refactor of that getter could silently remove the coverage.

- **Moody (1975) Fig. 1** -- gated **through the dispatcher**: 13
  active isobar tests
  (p0/p_ref = 0.25-30.0) assert `|log10 G_test - log10 G_ref| <= 0.06`,
  0.08 for deeply-subcooled Region-1 points. Caveat: the validator is
  region-filtered -- points that are neither in-dome (Region 4) nor
  deeply subcooled are skipped as a documented HEM limitation rather
  than asserted.
- **Zaloudek** -- gated: 89 tests across five files (21 in-dome / 21
  generic-dispatcher / 21 backward-throat / 22 subcooled, one an
  `#[ignore]`d diagnostic sweep / 4 superheated), critical-pressure
  tolerance 0.005-0.05 by curve, mass-flux log10 tolerance 0.05. Only
  the 21 generic tests exercise the dispatcher itself; the other four
  files call the regime-specific solvers directly, pinning what the
  dispatcher routes *to* rather than the routing.
  Caveat: these are graph-read HEM curves digitised from Saha (1978)
  NUREG/CR-0417, **not** experimental data -- so they check this
  implementation against another HEM, not against reality.
- **Marviken is NOT gated.** The digitised NUREG/CR-2671 test-23/24
  data exists in `marviken_tests.rs`, but the single test is
  `#[ignore = "skip first, Marviken is more complex"]`, its only
  assertion is commented out, and the body ends in `todo!()`. Do not
  cite Marviken in support of this boundary condition. Status as of
  2026-08-11: gating is being implemented in `tampines-steam-tables`
  (the op-bcg/op-4ily follow-up); re-check that crate's test suite
  for the current result rather than treating this line as permanent.

The benchmark actually run on this solver path is **Edwards-O'Brien**,
gated by `edwards_drift_flux_gs1_pressure_history` in
`src/multiphase_1d/edwards_tests.rs` (measured 2026-08-11: full 600 ms,
GS-1 pressure RMSE 29.0 psia against the digitised experimental curve,
flashing plateau 354.3 psia inside the ~350-367 psia experimental band).
Read the caveats in that file's header before citing it: the reference
is a digitisation of a published figure, only GS-1 is gated, and the
break itself is this HEM dispatcher, so that result does **not**
validate a drift-flux critical-flow model.

**Correction, 2026-08-11.** This paragraph previously read "The
genuinely validated benchmark on this solver path is Edwards-O'Brien
(2 active, passing tests)" while no Edwards test existed anywhere in
this crate. It does now, and the sentence has been rewritten to say
what it gates.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `area_fraction` | `f64` | Break area as a fraction of the pipe flow area, in `(0, 1]`. |
| `ambient_pressure` | `f64` | Ambient / containment back-pressure \[Pa\]. The outlet unchokes<br>once the critical throat pressure falls below it. |

###### `ReservoirInlet`

A **large upstream plenum** feeding the pipe: a Dirichlet *stagnation*
pressure with a prescribed stagnation enthalpy, i.e. the vessel of a
discharge experiment.

# What it imposes, and on which staggered locations

Three separate things, at three places in the step:

1. **A Dirichlet pressure `p_0` in the pressure equation**, at the
   boundary *face*. The face is no longer excluded from the matrix as
   [`Closed`](Self::Closed) and [`PrescribedVelocity`](Self::PrescribedVelocity)
   are — it carries the usual velocity-correction coefficient over a
   **half cell**, `d = Δt / (ρ_face · Δx/2)`, because the distance from
   the boundary face to the first cell centre is `Δx/2` and not `Δx`.
   The resulting `ρ_face · d · p_0` term enters the first cell's
   right-hand side, which is what makes the pressure problem
   well-posed with a Dirichlet at each end instead of floating on the
   compressibility diagonal.
2. **A momentum balance at the boundary face**, giving the predictor
   velocity. For inflow the convective term is taken in *conservative*
   form over the half cell from a stagnant plenum,
   `d(u²/2)/dx ≈ (u²/2 − 0)/(Δx/2)`, rather than the non-conservative
   `u ∂u/∂x` the interior faces use. That is deliberate and it is the
   difference between a right and a wrong answer here: with the
   pressure correction, the steady state of the conservative form is
   exactly `u = sqrt(2 (p_0 − p_1) / ρ)` — Bernoulli acceleration from
   rest — whereas the non-conservative donor form across the plenum
   velocity jump returns `sqrt((p_0 − p_1)/ρ)`, low by `sqrt(2)`. The
   interior form is kept for backflow (`u < 0`), where there is no jump
   because the upstream state is the first cell.
   Wall friction uses the adjacent cell's mixture viscosity; the drift
   momentum flux is differenced over the same half cell against
   **zero**, since a stagnant plenum has no relative phase motion.
3. **An inflow donor state for the scalars.** The existing donor-cell
   helpers assume zero-gradient at the ends, which silently makes an
   inlet re-inject whatever is already in the first cell. Instead the
   entering fluid carries the reservoir's stagnation enthalpy `h_0`,
   arrives at the first cell's pressure `p_1`, and is flashed there:
   that equilibrium flash supplies the inflow density used for the face
   mass flux and the inflow void used by the vapour-mass equation. The
   enthalpy handed to the energy equation is the **static** value
   `h_0 − u²/2`, which is the consistent partner of this solver's
   `ρ Dh/Dt = Dp/Dt` energy equation (that equation reproduces
   `dh = dp/ρ`, so `h + u²/2` is conserved along a streamline and the
   boundary must supply the static, not the stagnation, enthalpy).

# The two standing assumptions

Both are load-bearing, so they are stated here rather than only at the
call sites that rely on them.

1. **The drift velocity is zero at the inlet face.** A stagnant plenum
   has no relative phase motion, so `U_dm = 0` there and the drift
   momentum flux `Φ` is differenced over the inlet half cell against
   **zero** rather than against an extrapolated interior value. The
   consequence to be aware of: entering fluid that is already two-phase
   arrives with its phases moving *together*, and any slip must develop
   over the first cells. A case whose answer depends on the inlet slip
   being non-zero — a vertical riser fed by a churning plenum, say — is
   outside what this boundary models.
2. **The plenum is infinite and unchanging.** It does not deplete, does
   not cool, and does not respond to what the pipe does. A blowdown
   whose vessel state moves during the window of interest must re-impose
   the boundary each step with the updated state, exactly as the
   Edwards case re-imposes its rupture-disc ramp.

# V&V status

**Verified** — for a single-phase subcooled inlet — by
`reservoir_inlet_reaches_the_bernoulli_limit_in_subcooled_water` in
`src/multiphase_1d/tests.rs`, which marches a 20 kPa discharge to steady
state and reproduces the closed form
`u = sqrt(2 (p_0 − p_out) / (ρ (1 + f L / D)))` to **1.1 parts per
million** (measured 2026-08-11, release: `6.225891` m/s against
`6.225884` m/s), with the inlet, interior and outlet pressure terms each
matching their predicted values to `≈0.01` Pa. Read that test's doc
comment for the full record.

**Not verified**: the two-phase inflow flash (the test's `α` is
identically zero, so `InflowState::void_fraction` and
`InflowState::rho_g` were never exercised), the `h_0 − u²/2` static
enthalpy split at any appreciable velocity head, backflow through this
boundary, and any transient behaviour. Do not describe those as checked.

# Other limitations, stated plainly

- **The inflow flash is an equilibrium one.** The metastability this
  solver represents — the vapour-generation relaxation `τ` — acts
  inside the domain, not at the boundary. Entering fluid is therefore
  on the equilibrium `(p, h)` locus by construction, and a case that
  wants delayed flashing must let it develop over the first cells.
- **The area change of a real convergent inlet is not resolved.** The
  pipe is constant-area, so the acceleration from a vessel into a bore
  happens as a jump at the face rather than over a contraction, and no
  geometric throat exists inside the domain.
- **Nothing here chokes the flow.** The pressure equation is elliptic,
  so information propagates upstream through it; this boundary imposes
  no critical-flow criterion, unlike [`ChokedOutlet`](Self::ChokedOutlet).
  Whether a critical flux emerges at all is a property of the solution,
  and a case relying on one must measure it rather than assume it.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `stagnation_pressure` | `f64` | Plenum stagnation pressure `p_0` \[Pa\]. Must be inside the<br>IAPWS-IF97 range. |
| `stagnation_enthalpy` | `f64` | Plenum stagnation specific enthalpy `h_0` \[J/kg\]. Fixes what<br>enters: subcooled liquid, saturated liquid, or a two-phase<br>mixture, according to where `(p_1, h_0)` lands. |

###### `PressureOutlet`

A **prescribed static pressure** at the boundary face: the receiver a
pipe discharges into.

# What it imposes, and on which staggered locations

- **A Dirichlet pressure in the pressure equation**, at the boundary
  face, with the same half-cell coefficient
  `d = Δt / (ρ_face · Δx/2)` described at
  [`ReservoirInlet`](Self::ReservoirInlet). The face velocity is
  corrected as `u ← u* − d (p_out − p_last)` at the `x = L` end, so a
  receiver pressure below the last cell's drives outflow.
- **A momentum balance at the boundary face** for the predictor. For
  outflow this is the *same* donor-cell `u ∂u/∂x` the interior faces
  use, taken against the last interior face — the field is smooth
  there, so no special treatment is warranted. For backflow it is the
  conservative half-cell form from a stagnant receiver, for the reason
  given at [`ReservoirInlet`](Self::ReservoirInlet).
- **Scalar outflow is upwinded from the last cell**, which is what the
  existing donor-cell helpers already do at an end face.

# Limitations, stated plainly

- **No inflow state.** If the pressure gradient reverses, fluid enters
  carrying the *last cell's* enthalpy and void (zero-gradient), because
  this boundary carries no description of the receiver's contents. A
  case with sustained backflow through this boundary is outside what it
  models; use a [`ReservoirInlet`](Self::ReservoirInlet) there instead.
- **It cannot choke.** A prescribed downstream pressure is felt
  upstream through the implicit pressure solve, which is exactly the
  behaviour a critical-flow model exists to suppress. This boundary
  imposes no critical-flux criterion of any kind, so a case that wants
  one must either use [`ChokedOutlet`](Self::ChokedOutlet) or maximise
  the steady flux over the receiver pressure externally — and must
  report honestly if no maximum is found.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | Receiver static pressure \[Pa\], inside the IAPWS-IF97 range. |

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
    fn clone(self: &Self) -> AxialBoundary { /* ... */ }
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
    fn eq(self: &Self, other: &AxialBoundary) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `DriftFluxReport`

Per-step diagnostics from [`DriftFlux1d::step`].

Returned rather than logged, so a case can assert on it and a V&V test can
record measured values.

```rust
pub struct DriftFluxReport {
    pub time: f64,
    pub outlet_mass_flow: f64,
    pub outlet_choked: bool,
    pub inventory: f64,
    pub max_void_fraction: f64,
    pub max_courant: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time` | `f64` | Simulated time at the end of the step \[s\]. |
| `outlet_mass_flow` | `f64` | Mass flow through the right-hand boundary \[kg/s\], positive outward. |
| `outlet_choked` | `bool` | Whether either boundary was choked this step. |
| `inventory` | `f64` | Total mass currently in the pipe \[kg\]. |
| `max_void_fraction` | `f64` | Largest void fraction anywhere \[-\]. |
| `max_courant` | `f64` | Largest material Courant number `|u| Δt / Δx` \[-\].<br><br>The stability figure that matters for the explicit transport. The<br>*acoustic* Courant number is not limiting, because the pressure solve<br>is implicit — that is the whole point of the semi-implicit method. A<br>value above 1 means donor-cell transport has stepped past a whole cell<br>and the answer is not to be trusted. |

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
    fn clone(self: &Self) -> DriftFluxReport { /* ... */ }
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
    fn eq(self: &Self, other: &DriftFluxReport) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `DriftFlux1d`

A 1-D transient drift-flux solver for compressible steam/water pipe flow.

# Layout

**Staggered**: scalars (`p`, `h`, `α`, `ρ`) at cell centres, velocities at
faces. Staggering rather than collocation because in 1-D it removes
pressure-velocity checkerboarding *by construction* — no Rhie-Chow
interpolation needed — and it is what the reference system codes do. Face
`j` is the left face of cell `j`; faces `0` and `n_cells` are the ends.

# Units

Constructors and accessors are `uom`-typed. Internal state is raw `f64` in
strict SI: pascal, `J/kg`, `kg/m³`, `m/s`, `[-]`.

```rust
pub struct DriftFlux1d {
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
  pub fn new(pipe: Pipe1d, slip: SlipModel, pressure: Pressure, temperature: ThermodynamicTemperature, dt: Time) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Build a solver on `pipe`, initialised to a uniform `(p, T)` state.

- ```rust
  pub fn set_temperature_profile(self: &mut Self, temperatures: &[ThermodynamicTemperature]) -> Result<(), TampinesError> { /* ... */ }
  ```
  Overwrite the cell temperatures from an axial profile, re-flashing

- ```rust
  pub fn set_left_boundary(self: &mut Self, bc: AxialBoundary) { /* ... */ }
  ```
  Set the boundary condition at the `x = 0` end.

- ```rust
  pub fn set_right_boundary(self: &mut Self, bc: AxialBoundary) { /* ... */ }
  ```
  Set the boundary condition at the `x = L` end.

- ```rust
  pub fn vapour_relaxation_time(self: &Self) -> Time { /* ... */ }
  ```
  The vapour-generation relaxation time `τ` \[s\]. A model parameter —

- ```rust
  pub fn set_vapour_relaxation_time(self: &mut Self, tau: Time) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the vapour-generation relaxation time `τ`.

- ```rust
  pub fn set_outer_correctors(self: &mut Self, n: usize) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the number of SIMPLE-style outer correctors per step.

- ```rust
  pub fn outer_correctors(self: &Self) -> usize { /* ... */ }
  ```
  The number of outer correctors per step.

- ```rust
  pub fn set_pressure_under_relaxation(self: &mut Self, alpha: uom::si::f64::Ratio) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the pressure under-relaxation factor `α_p ∈ (0, 1]`.

- ```rust
  pub fn time(self: &Self) -> Time { /* ... */ }
  ```
  Elapsed simulated time.

- ```rust
  pub fn pressure(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell pressures \[Pa\], read-only.

- ```rust
  pub fn void_fraction(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell void fractions \[-\], read-only.

- ```rust
  pub fn density(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell mixture densities \[kg/m³\], read-only.

- ```rust
  pub fn enthalpy(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell mixture specific enthalpies \[J/kg\], read-only.

- ```rust
  pub fn face_velocity(self: &Self) -> &[f64] { /* ... */ }
  ```
  Face velocities \[m/s\], read-only, length `n_cells + 1`.

- ```rust
  pub fn pipe(self: &Self) -> &Pipe1d { /* ... */ }
  ```
  The pipe geometry.

- ```rust
  pub fn temperature(self: &Self, cell: usize) -> Result<ThermodynamicTemperature, TampinesError> { /* ... */ }
  ```
  Cell temperature \[K\] — `T_sat` inside the dome, the flashed value

- ```rust
  pub fn inventory(self: &Self) -> Mass { /* ... */ }
  ```
  Total mass currently in the pipe.

- ```rust
  pub fn outlet_mass_flow(self: &Self) -> MassRate { /* ... */ }
  ```
  Mass flow through the right-hand boundary \[kg/s\], from the current

- ```rust
  pub fn phase_velocities(self: &Self) -> Result<(Vec<f64>, Vec<f64>), TampinesError> { /* ... */ }
  ```
  Phase velocities `(v_g, v_l)` \[m/s\] reconstructed at cell centres.

- ```rust
  pub fn step(self: &mut Self) -> Result<DriftFluxReport, TampinesError> { /* ... */ }
  ```
  Advance one timestep.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `as_velocity`

**Attributes:**

- `MustUse { reason: None }`

Wrap a raw SI velocity as a `uom` quantity, for callers outside the
marching loop.

```rust
pub fn as_velocity(u_si: f64) -> uom::si::f64::Velocity { /* ... */ }
```

#### Function `cell_position`

**Attributes:**

- `MustUse { reason: None }`

Axial position of a cell centre as a `uom` quantity.

```rust
pub fn cell_position(pipe: &super::geometry::Pipe1d, cell: usize) -> uom::si::f64::Length { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_VAPOUR_RELAXATION_TIME`

Default vapour-generation relaxation time `τ` \[s\].

`1e-3` s — the order of the acoustic transit time of one cell in a blowdown
mesh, so the void tracks the pressure without a flashing front being
instantaneous. It is a **model parameter with no measured provenance**; a
case whose answer depends sensitively on it is a case whose result should
be reported *with* that sensitivity, not one that has found the right
value.

```rust
pub const DEFAULT_VAPOUR_RELAXATION_TIME: f64 = 1.0e-3;
```

#### Constant `COMPRESSIBILITY_STEP`

Relative finite-difference step used for the compressibility `∂ρ/∂p|_h`.

```rust
pub const COMPRESSIBILITY_STEP: f64 = 1.0e-4;
```

#### Constant `DEFAULT_OUTER_CORRECTORS`

Default number of SIMPLE-style outer correctors per step.

`8`. See [`DriftFlux1d::set_outer_correctors`] for why one is not enough
when a subcooled cell has to cross the saturation line within a step.

```rust
pub const DEFAULT_OUTER_CORRECTORS: usize = 8;
```

#### Constant `DEFAULT_PRESSURE_UNDER_RELAXATION`

Default pressure under-relaxation `α_p`.

`0.7` — enough damping that the first corrector's large excursion does not
overshoot into a region the next linearisation cannot recover from, without
slowing convergence to the point of needing many more correctors.

```rust
pub const DEFAULT_PRESSURE_UNDER_RELAXATION: f64 = 0.7;
```

#### Constant `DEFAULT_OUTER_TOLERANCE`

Default outer-corrector convergence tolerance on `max |Δp|` \[Pa\].

`1.0` Pa — six orders below the 7 MPa initial pressure of a blowdown, and
far below any pressure difference a gauge comparison resolves.

```rust
pub const DEFAULT_OUTER_TOLERANCE: f64 = 1.0;
```

## Module `geometry`

The 1-D pipe both [`super::drift_flux`] and [`super::two_fluid`] march on.

# What belongs here

Geometry and geometry alone: length, flow area, hydraulic diameter, incline,
and the uniform axial cell layout derived from them. No state, no fluid
properties, no time.

# What does not

Anything that changes during a transient. Both solvers own their own field
state; this type is constructed once and read thereafter.

```rust
pub mod geometry { /* ... */ }
```

### Types

#### Struct `Pipe1d`

A straight pipe of constant cross-section, discretised into uniform axial
cells.

# The mesh

`n_cells` cells of equal length `Δx = L / n_cells`, indexed `0 … n_cells−1`
from the `x = 0` end. Cell `i` spans `[i Δx, (i+1) Δx]` and its centre is at
`(i + ½) Δx`. Faces are indexed `0 … n_cells`, face `i` sitting at `x = i Δx`
between cells `i−1` and `i`; faces `0` and `n_cells` are the two ends.

This deliberately matches the layout
[`outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh`]
produces, so a case written against the HEM [`crate::hem`] array maps onto
these solvers cell-for-cell.

# Units

Constructed and read through `uom`. The stored fields are raw `f64` in
strict SI — metre, square metre, radian — because the marching loops read
them per cell per step.

# Valid ranges

`length > 0`, `flow_area > 0`, `n_cells ≥ 1`, `hydraulic_diameter > 0`.
The incline is unrestricted: `+π/2` is vertically upward flow in the `+x`
direction, `−π/2` vertically downward, `0` horizontal.

```rust
pub struct Pipe1d {
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
  pub fn new(length: Length, flow_area: Area, hydraulic_diameter: Length, incline: Angle, n_cells: usize) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Build a pipe from its dimensions.

- ```rust
  pub fn circular(length: Length, inside_diameter: Length, incline: Angle, n_cells: usize) -> Result<Self, TampinesError> { /* ... */ }
  ```
  A circular pipe, whose hydraulic diameter *is* its inside diameter and

- ```rust
  pub fn length(self: &Self) -> Length { /* ... */ }
  ```
  Total pipe length `L` \[m\].

- ```rust
  pub fn flow_area(self: &Self) -> Area { /* ... */ }
  ```
  Cross-sectional flow area `A` \[m²\].

- ```rust
  pub fn hydraulic_diameter(self: &Self) -> Length { /* ... */ }
  ```
  Hydraulic diameter `D_h` \[m\].

- ```rust
  pub fn incline(self: &Self) -> Angle { /* ... */ }
  ```
  Incline from horizontal \[rad\], `+` for `+x` upward.

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of uniform axial cells.

- ```rust
  pub fn dx(self: &Self) -> f64 { /* ... */ }
  ```
  Cell length `Δx = L / n_cells` \[m\], raw SI.

- ```rust
  pub fn cell_volume(self: &Self) -> f64 { /* ... */ }
  ```
  Cell volume `V = A Δx` \[m³\], raw SI. Uniform, so one value serves

- ```rust
  pub fn area_si(self: &Self) -> f64 { /* ... */ }
  ```
  Flow area \[m²\] as raw SI, for the marching loops.

- ```rust
  pub fn hydraulic_diameter_si(self: &Self) -> f64 { /* ... */ }
  ```
  Hydraulic diameter \[m\] as raw SI, for the marching loops.

- ```rust
  pub fn axial_gravity(self: &Self) -> f64 { /* ... */ }
  ```
  The gravitational acceleration **along the pipe axis**,

- ```rust
  pub fn cell_centre(self: &Self, i: usize) -> f64 { /* ... */ }
  ```
  Centre position of cell `i` \[m\], raw SI: `(i + ½) Δx`.

- ```rust
  pub fn nearest_cell(self: &Self, x: Length) -> usize { /* ... */ }
  ```
  The index of the cell whose centre lies closest to axial position `x`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Pipe1d { /* ... */ }
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
    fn eq(self: &Self, other: &Pipe1d) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `interfacial`

The 1-D **interfacial exchange** layer — the three source terms that couple
the two phases of a six-equation two-fluid model.

# What belongs in here

Exactly the *interfacial closures* a six-equation model needs, evaluated
**one cell at a time** from that cell's local two-phase state:

| Term | Symbol | Units | Closed by |
|---|---|---|---|
| Interfacial drag | `F^i` | N/m³ | [`outram_foam_multiphase::two_fluid::DragModel`] |
| Interfacial heat transfer | `Q_g^i`, `Q_l^i` | W/m³ | [`outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer`] |
| Interfacial mass transfer | `Γ` | kg/(m³·s) | the energy jump across the interface (below) |

# What does NOT belong in here

- **The field equations.** This module computes source *terms*; the phase
  mass/momentum/energy equations, their discretisation, and the pressure
  solve live in [`super::two_fluid`].
- **The well-posedness regularisation.** The naive six-equation system has
  complex characteristics; whichever term fixes that (interfacial pressure,
  virtual mass, artificial viscosity) is a *field-equation* decision and is
  made in the solver, not here.
- **Wall heat transfer and wall friction.** Those couple a phase to the
  *wall*, not to the interface.
- **Interfacial-area transport and the flow-regime map.** See "Honest
  scope" below — both are absent, and their absence is the largest
  modelling limitation of this layer.
- **A second property path.** Every thermodynamic and transport number comes
  from [`super::properties`]; nothing is re-derived here.

# Trace-back: what is consumed rather than reinvented

Beads `op-dt3.12` / `op-dt3.13` require 1-D closures to trace back to the
OUTRAM-FOAM 3-D reference rather than be invented. Concretely:

- **Drag** — [`outram_foam_multiphase::two_fluid::DragModel`] (Schiller-
  Naumann, Wen-Yu, and a constant, all ported from OpenFOAM's
  `dispersedDragModel` chain) is called through its own public
  [`k_d`](outram_foam_multiphase::two_fluid::DragModel::k_d), on a
  single-cell [`outram_foam_multiphase::two_fluid::TwoFluidSystem`] packed
  from the local state. The slip Reynolds number is likewise taken from
  [`TwoFluidSystem::reynolds_number`], not recomputed. No drag correlation
  is written down in this file.
- **Heat transfer** —
  [`outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer`]
  (Ranz-Marshall, spherical conduction, Gunn — ported from OpenFOAM's
  `multiphaseEuler` heat-transfer models) supplies both volumetric
  coefficients. No Nusselt correlation is written down in this file either.

Note for readers of [`super::two_fluid`]'s module documentation: its
"what still has to be decided" point 2 says the 3-D reference has no
interfacial heat-transfer closure at all. That was true when it was
written; `outram_foam_multiphase::heat_transfer` has since been ported, and
this module consumes it. The mass-transfer closure below is the remaining
piece, and it is a *thermodynamic identity* rather than a correlation — see
the next section.

# The two-resistance framework, and the error it exists to prevent

The interface is taken to sit at the **saturation temperature** `T_sat(p)`.
Each phase then exchanges heat with that interface through its own
resistance:

`Q_k^i = K_k · (T_sat − T_k)`  \[W/m³\], `k ∈ {g, l}`

and the two `K_k` come from **different** correlations, because the two
resistances are physically different:

- the **continuous** phase sees a convective boundary layer *outside* the
  inclusion — closed by Ranz-Marshall or Gunn, which read the
  **continuous** phase's thermal conductivity `λ_c`;
- the **dispersed** phase sees conduction *inside* the inclusion — closed by
  the spherical-conduction model, which reads the **dispersed** phase's
  conductivity `λ_d`.

Swapping those two conductivities is a **silent** error: the code runs and
the answer is wrong by the conductivity ratio, which for steam/water is
close to a factor of ten (measured `λ_f/λ_g = 9.210375` at 7 MPa on
2026-08-12 — see `two_resistance_pair_reads_one_conductivity_from_each_side`,
which shows the swapped liquid-side flux coming out 9.21× low). Three
things guard against it here, deliberately and redundantly:

1. [`InterfacialExchange::new`] **rejects** a pair whose
   [`ResistanceSide`]s are not one `Continuous` and one `Dispersed`.
2. The conductivities are passed to the upstream closure **by role**
   (`λ_continuous`, `λ_dispersed`) in fixed argument positions; the variant
   itself decides which to read. This file never picks one.
3. [`DispersedPhase`] is an explicit constructor argument, so "which phase
   is dispersed" is a stated modelling choice rather than an accident of
   how the caller ordered its arguments.

# Interfacial mass transfer — derived, not invented

With the interface at `T_sat(p)` and no thermal capacity of its own, the
energy that arrives at the interface from both sides has nowhere to go
except into phase change. That closes `Γ` with no new correlation:

`Γ · h_fg = −(Q_g^i + Q_l^i)`, so `Γ = −(Q_g^i + Q_l^i) / (h_g^sat − h_l^sat)`.

## Sign convention — stated plainly

- `Q_k^i` is the heat flowing **into phase `k` from the interface**
  \[W/m³\]. This is the convention of
  [`InterfacialHeatTransfer`](outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer)
  (`Q = K (T_interface − T_phase)`) and it is the sign with which `Q_k^i`
  enters the phase energy equation as written in [`super::two_fluid`]
  (`… = α_k ∂p/∂t + Q_k^i + Γ_k h_k^i + q_k^wall`). A **positive** `Q_g^i`
  heats the vapour.
- **`Γ > 0` means evaporation** — liquid becoming vapour, mass leaving the
  liquid equation and entering the vapour equation (`Γ_g = +Γ`,
  `Γ_l = −Γ`). `Γ < 0` is condensation.
- The minus sign in `Γ = −(Q_g^i + Q_l^i)/h_fg` follows from those two
  choices together: superheated vapour (`T_g > T_sat`) gives `Q_g^i < 0` —
  the vapour *loses* heat to the interface — and that delivered heat
  evaporates liquid, so `Γ > 0`. Written instead with interface-directed
  fluxes `q_{k→i} = −Q_k^i`, the same relation reads
  `Γ = (q_{g→i} + q_{l→i})/h_fg` with a plus sign. Both are the same
  physics; this module reports the *into-phase* fluxes, so the minus sign
  is the one that belongs in the code.

## The degenerate case: `h_fg → 0` at the critical point

Physically `h_fg = h_g^sat − h_l^sat` vanishes at the critical point
(`p_c = 22.064 MPa`), so `Γ` becomes a `0/0` limit there and is not
computable from this energy balance. Above `p_c` there is no saturation
line at all, and [`SaturatedProperties::at`] does **not** itself refuse
supercritical pressures — its `P_MAX_IF97` is 100 MPa — so an unguarded
division would return a finite, entirely fictitious `Γ`.

**This module refuses, and it refuses on pressure rather than on `h_fg`.**
That is not the obvious choice, so here is why. Measured on 2026-08-12 (see
`latent_heat_guard_fires_only_next_to_the_critical_point`), the latent heat
this property layer reports on the approach to `p_c` is:

```text
p = 20.000 MPa   h_fg = 6.0058e5 J/kg
p = 22.000 MPa   h_fg = 3.8700e5 J/kg
p = 22.060 MPa   h_fg = 3.7988e5 J/kg
p = 22.064 MPa   h_fg = 3.7940e5 J/kg   <- p_c; the true value here is 0
```

It does not collapse. [`SaturatedProperties::at`] evaluates `h_f` and `h_g`
from the IF97 **Region 1 and Region 2** equations at `T_sat(p)`, and both
regions stop at `T = 623.15 K` while `T_sat(p_c) ≈ 647 K`; reproducing the
critical collapse needs Region 3, which that property layer does not
implement. So a threshold on `h_fg` alone would **never fire for water** and
would be a guard in name only — precisely the kind of reassuring-looking
check this project's V&V rules exist to prevent.

What actually guards the closure is therefore an explicit refusal at
`p ≥ ` [`P_CRITICAL_WATER`], with the
[`MIN_LATENT_HEAT_FOR_MASS_TRANSFER`] threshold kept behind it as a
backstop for a property set that degenerates for some other reason (a
non-finite or negative `h_fg`, or a future Region-3 implementation that
*does* reproduce the collapse just below `p_c`). Both return
[`TampinesError::Unphysical`]. Nothing is clamped, nothing is floored, and
no "large but finite" `Γ` is returned, because a plausible-looking number
here would propagate into the phase mass equations and be
indistinguishable from an answer. A caller at these pressures needs a
different model (single-phase supercritical), not a rescued division.

Note what this does *not* fix: at 22.0 MPa, just below the refusal, the
closure returns a number built on a `h_fg` that is already leaving IF97's
Region 1/2 validity. Nothing here has checked that number against
anything.

# Honest scope — what this layer does NOT do

- **No interfacial-area transport.** The inclusion diameter `d` is
  prescribed once at construction and never responds to breakup,
  coalescence, or the local flow. Every closure here scales as `1/d²`, so
  `d` is the single most influential number a caller supplies and the least
  justified.
- **No flow-regime map.** [`DispersedPhase`] is fixed for the whole
  calculation. A real transient runs bubbly at low void and droplet/mist at
  high void; this layer will happily evaluate bubbly closures at `α_g =
  0.99`, which is wrong and will not complain.
- **No lift, virtual mass, wall lubrication, or turbulent dispersion.**
  Upstream carries those as unported scaffolds
  ([`outram_foam_multiphase::two_fluid::InterfacialForce`]); only drag is
  available, so only drag is offered.
- **No condensation/evaporation rate limiter.** `Γ` is whatever the energy
  balance says. At small `α_g` with a large subcooling the implied `Γ` can
  remove more vapour than the cell contains within a timestep; bounding
  that is the *solver's* job (it needs `Δt` and the cell inventory, neither
  of which this layer sees) and it is not done here.
- **Transport properties are taken on the saturation line**, even for a
  metastable phase — see [`SaturatedTransport`] for why that is both safer
  and more accurate than the alternative over the bounded metastable
  departures [`PhaseState`] admits.
- **Not validated against any experiment.** Every test in this file is
  *verification* against a closed form or an invariant. No interfacial
  closure here has been compared with measured phase-change data.

# Units

Constructors and the `uom` accessors on [`InterfacialSources`] are
dimension-checked. [`InterfacialCellState`] and the raw fields of
[`InterfacialSources`] carry **raw `f64` in strict SI** — pascal, `J/kg`,
`m/s`, kelvin, `W/m³`, `N/m³`, `kg/(m³·s)` — because a 1-D system code
evaluates them once per cell per timestep, and every one of them is
documented individually with its unit spelled out.

```rust
pub mod interfacial { /* ... */ }
```

### Types

#### Enum `DispersedPhase`

Which phase is treated as **dispersed** (bubbles or droplets) and which as
**continuous** (the carrier).

Every closure in this module is a *dispersed-phase* closure: it assumes
inclusions of one phase carried in the other, with the interfacial area set
by the inclusion diameter. Which phase plays which role therefore changes
which conductivity closes which resistance, and it is an explicit modelling
choice, not an inference.

**There is no flow-regime map.** Whichever variant is chosen at
construction is used at every void fraction for the whole calculation. The
physical choice is `Vapour` for bubbly flow (low `α_g`) and `Liquid` for
droplet/mist flow (high `α_g`); nothing here checks that `α_g` is in the
matching range.

```rust
pub enum DispersedPhase {
    Vapour,
    Liquid,
}
```

##### Variants

###### `Vapour`

Vapour bubbles carried in continuous liquid — bubbly flow. The
continuous-side resistance then reads the **liquid** conductivity
`λ_f`, the dispersed-side resistance the **vapour** conductivity `λ_g`.

###### `Liquid`

Liquid droplets carried in continuous vapour — droplet / mist flow. The
roles above are exchanged: continuous side reads `λ_g`, dispersed side
reads `λ_f`.

##### Implementations

###### Methods

- ```rust
  pub fn is_vapour_dispersed(self: Self) -> bool { /* ... */ }
  ```
  `true` when the vapour is the dispersed phase (bubbly flow).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DispersedPhase { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &DispersedPhase) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `InterfacialCellState`

The local two-phase state of **one** cell, as a six-equation solver carries
it.

# Why six numbers

A six-equation model transports `α_g`, and for each phase a velocity and an
enthalpy. That — plus the pressure — is exactly the state the interfacial
closures need. Nothing here is a mixture quantity: the whole point of the
six-equation model is that the phases may differ in both velocity and
temperature, so `h_g` and `h_l` are independent and neither is required to
sit on the saturation line.

# Units and ranges (raw `f64`, strict SI — this is a marching-loop boundary)

Every field is a raw `f64`; use [`InterfacialCellState::new`] for a
dimension-checked construction from `uom` quantities.

- `pressure` — \[Pa\], inside the IAPWS-IF97 range
  `[611.657, 1.0e8]`, and (for a meaningful `Γ`) below the critical
  pressure — see [`MIN_LATENT_HEAT_FOR_MASS_TRANSFER`].
- `void_fraction` — `α_g` \[-\], in `[0, 1]`. Outside that it is refused,
  not clipped.
- `vapour_enthalpy`, `liquid_enthalpy` — \[J/kg\], within the bounded
  metastable brackets [`PhaseState`] admits (30 K of vapour subcooling,
  50 K of liquid superheat as of writing); further than that is refused by
  the property layer, not extrapolated.
- `vapour_velocity`, `liquid_velocity` — \[m/s\], axial, signed, finite.
  Only their **difference** enters any closure here.

```rust
pub struct InterfacialCellState {
    pub pressure: f64,
    pub void_fraction: f64,
    pub vapour_enthalpy: f64,
    pub liquid_enthalpy: f64,
    pub vapour_velocity: f64,
    pub liquid_velocity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | Pressure `p` \[Pa\], shared by both phases (single-pressure model). |
| `void_fraction` | `f64` | Vapour volume fraction `α_g` \[-\], in `[0, 1]`. |
| `vapour_enthalpy` | `f64` | Vapour specific enthalpy `h_g` \[J/kg\]. |
| `liquid_enthalpy` | `f64` | Liquid specific enthalpy `h_l` \[J/kg\]. |
| `vapour_velocity` | `f64` | Vapour axial velocity `u_g` \[m/s\], signed. |
| `liquid_velocity` | `f64` | Liquid axial velocity `u_l` \[m/s\], signed. |

##### Implementations

###### Methods

- ```rust
  pub fn new(pressure: Pressure, void_fraction: f64, vapour_enthalpy: AvailableEnergy, liquid_enthalpy: AvailableEnergy, vapour_velocity: Velocity, liquid_velocity: Velocity) -> Self { /* ... */ }
  ```
  Build a cell state from `uom` quantities, so the dimensions are checked

- ```rust
  pub fn slip_velocity(self: Self) -> f64 { /* ... */ }
  ```
  Slip velocity `u_g − u_l` \[m/s\], signed. Positive means the vapour

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> InterfacialCellState { /* ... */ }
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
    fn eq(self: &Self, other: &InterfacialCellState) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `InterfacialSources`

The three interfacial source terms for one cell, plus the intermediate
quantities a caller needs to check them.

# Units (raw `f64`, strict SI — this is a marching-loop boundary)

See each field. `uom`-typed accessors are provided for the quantities that
have an SI dimension in `uom`'s quantity set; `kg/(m³·s)` and `N/m³` do
not, and are documented in prose instead — the same compromise
[`InterfacialHeatTransfer::volumetric_coefficient`] makes upstream.

# Signs, in one place

- `drag_force_on_vapour > 0` accelerates the vapour in `+x`.
- `vapour_heat > 0` heats the vapour (heat flows interface → vapour).
- `mass_transfer > 0` is **evaporation**.

```rust
pub struct InterfacialSources {
    pub volumetric_drag_coefficient: f64,
    pub drag_force_on_vapour: f64,
    pub vapour_heat: f64,
    pub liquid_heat: f64,
    pub mass_transfer: f64,
    pub interface_temperature: f64,
    pub vapour_temperature: f64,
    pub liquid_temperature: f64,
    pub slip_reynolds_number: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `volumetric_drag_coefficient` | `f64` | Volumetric drag coefficient `K_d` \[kg/(m³·s)\] from the traced-back<br>[`DragModel`]. Always `≥ 0`. |
| `drag_force_on_vapour` | `f64` | Interfacial drag force on the **vapour** `F_g^i = K_d (u_l − u_g)`<br>\[N/m³\]. The force on the liquid is exactly its negative — interfacial<br>momentum exchange conserves momentum by construction, which is why only<br>one of the pair is stored.<br><br>This expression is independent of which phase is dispersed: the drag<br>force on the dispersed phase is `K_d (u_continuous − u_dispersed)`, and<br>substituting either role assignment gives `K_d (u_l − u_g)` for the<br>vapour. |
| `vapour_heat` | `f64` | Interfacial heat transfer **into the vapour** `Q_g^i = K_g (T_sat − T_g)`<br>\[W/m³\]. Negative when the vapour is superheated. |
| `liquid_heat` | `f64` | Interfacial heat transfer **into the liquid** `Q_l^i = K_l (T_sat − T_l)`<br>\[W/m³\]. Negative when the liquid is metastably superheated, positive<br>when it is subcooled. |
| `mass_transfer` | `f64` | Interfacial mass transfer `Γ` \[kg/(m³·s)\], **positive for<br>evaporation** (liquid → vapour). See the module docs for the derivation<br>and the full sign argument. |
| `interface_temperature` | `f64` | Interface temperature \[K\] — the saturation temperature `T_sat(p)`, by<br>assumption. Reported so a caller can see the driving temperature<br>differences rather than infer them. |
| `vapour_temperature` | `f64` | Vapour bulk temperature `T_g` \[K\], from<br>[`PhaseState::vapour_at`] (metastable branch included). |
| `liquid_temperature` | `f64` | Liquid bulk temperature `T_l` \[K\], from<br>[`PhaseState::liquid_at`] (metastable branch included). |
| `slip_reynolds_number` | `f64` | Slip Reynolds number `Re = |u_d − u_c| d / ν_c` \[-\], taken from<br>[`TwoFluidSystem::reynolds_number`] rather than recomputed here. Both<br>the drag and the continuous-side heat-transfer correlation are<br>evaluated at this `Re`. |

##### Implementations

###### Methods

- ```rust
  pub fn drag_force_on_liquid(self: Self) -> f64 { /* ... */ }
  ```
  Interfacial drag force on the **liquid** \[N/m³\] — the negative of

- ```rust
  pub fn vapour_heat_density(self: Self) -> VolumetricPowerDensity { /* ... */ }
  ```
  [`vapour_heat`](Self::vapour_heat) as a `uom` quantity \[W/m³\].

- ```rust
  pub fn liquid_heat_density(self: Self) -> VolumetricPowerDensity { /* ... */ }
  ```
  [`liquid_heat`](Self::liquid_heat) as a `uom` quantity \[W/m³\].

- ```rust
  pub fn saturation_temperature(self: Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  [`interface_temperature`](Self::interface_temperature) as a `uom`

- ```rust
  pub fn net_heat_to_interface(self: Self) -> f64 { /* ... */ }
  ```
  Net heat delivered **to the interface** from both phases \[W/m³\],

- ```rust
  pub fn is_evaporating(self: Self) -> bool { /* ... */ }
  ```
  `true` when the cell is evaporating (`Γ > 0`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> InterfacialSources { /* ... */ }
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
    fn eq(self: &Self, other: &InterfacialSources) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `InterfacialExchange`

The interfacial exchange closure set for a 1-D six-equation two-fluid model.

# What it is

A fixed choice of *three* closures — one drag model and a two-resistance
heat-transfer **pair** — plus the inclusion diameter they all scale with.
Given one cell's state it returns the drag, the two interfacial heat
fluxes, and the mass transfer they imply. It holds no field data and no
time state: it is a pure function of the cell state, evaluated once per
cell per timestep by the solver that owns the fields.

# Why it owns a mesh

The traced-back [`DragModel::k_d`] takes a
[`TwoFluidSystem`], which is field-based. Rather than re-implement its
arithmetic — which would be exactly the independent invention the trace-back
rule forbids — this type keeps a **single-cell** finite-volume mesh and
packs the local state into it for each call, the same round-trip
[`super::drift_flux`] performs for `SlipModel`. The mesh's geometry (a 1 m
long, 1 m² cell) is never read by any closure: only `n_cells == 1` matters.

The cost is a handful of small allocations per cell per timestep, because
[`Phase`]'s scalar density and viscosity are per-phase constants upstream
with no setters, so the phases must be rebuilt when the local density
changes — which in a blowdown is every cell and every step. That is a real
cost and it is the price of not duplicating the correlation. It is noted
here so a future profiler is not surprised.

# No derives

Neither [`DragModel`] nor this type derives `Debug`, `Clone`, or `Copy`:
the upstream enum derives nothing, and adding a wrapper that pretends
otherwise would mean cloning the mesh `Arc` under a `Clone` that looks
free.

```rust
pub struct InterfacialExchange {
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
  pub fn new(drag: DragModel, continuous_side: InterfacialHeatTransfer, dispersed_side: InterfacialHeatTransfer, dispersed: DispersedPhase, diameter: Length, residual_alpha: f64) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Assemble an interfacial closure set.

- ```rust
  pub fn bubbly(diameter: Length) -> Result<Self, TampinesError> { /* ... */ }
  ```
  The conventional **bubbly-flow** closure set: Schiller-Naumann drag,

- ```rust
  pub fn droplet(diameter: Length) -> Result<Self, TampinesError> { /* ... */ }
  ```
  The conventional **droplet / mist-flow** closure set: the same three

- ```rust
  pub fn dispersed_phase(self: &Self) -> DispersedPhase { /* ... */ }
  ```
  Which phase this closure set treats as dispersed.

- ```rust
  pub fn continuous_side_model(self: &Self) -> InterfacialHeatTransfer { /* ... */ }
  ```
  The continuous-side heat-transfer closure.

- ```rust
  pub fn dispersed_side_model(self: &Self) -> InterfacialHeatTransfer { /* ... */ }
  ```
  The dispersed-side heat-transfer closure.

- ```rust
  pub fn diameter(self: &Self) -> Length { /* ... */ }
  ```
  The prescribed inclusion diameter `d` \[m\].

- ```rust
  pub fn residual_alpha(self: &Self) -> f64 { /* ... */ }
  ```
  The residual volume-fraction floor `α_res` \[-\] used by the

- ```rust
  pub fn sources_si(self: &Self, cell: InterfacialCellState) -> Result<InterfacialSources, TampinesError> { /* ... */ }
  ```
  Evaluate all three interfacial source terms for one cell, evaluating the

- ```rust
  pub fn sources_with_properties(self: &Self, cell: InterfacialCellState, saturated: SaturatedProperties, transport: SaturatedTransport) -> Result<InterfacialSources, TampinesError> { /* ... */ }
  ```
  Evaluate all three interfacial source terms for one cell against

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Constants and Statics

#### Constant `P_CRITICAL_WATER`

The critical pressure of water `p_c` \[Pa\], IAPWS-95 / IF97 value
`22.064 MPa`.

# What it guards

At and above `p_c` there is no saturation line, so the interface cannot be
"at `T_sat`", `h_fg` has no meaning, and `Γ` is not defined by the energy
jump this module uses.
[`InterfacialExchange::sources_with_properties`] refuses at `p ≥ p_c` with
[`TampinesError::Unphysical`].

This is a **pressure** guard rather than a latent-heat guard for a measured
reason: [`SaturatedProperties::at`] reports `h_fg = 3.7940e5 J/kg` at
exactly `p_c` (2026-08-12), where the physical value is zero, because its
`h_f`/`h_g` come from IF97 Regions 1 and 2, whose validity stops at
`T = 623.15 K` while `T_sat(p_c) ≈ 647 K`. The module documentation gives
the full sweep and the reasoning. Guarding on `h_fg` alone would have been
a check that never fires.

```rust
pub const P_CRITICAL_WATER: f64 = 22.064e6;
```

#### Constant `MIN_LATENT_HEAT_FOR_MASS_TRANSFER`

Smallest latent heat `h_fg` \[J/kg\] at which the interfacial mass-transfer
balance is still considered meaningful — the **backstop** behind
[`P_CRITICAL_WATER`].

# What it guards

`Γ = −(Q_g^i + Q_l^i)/h_fg` divides by the latent heat. The pressure guard
already excludes the region where that latent heat physically vanishes, so
this threshold exists to catch a property set that has gone wrong some
*other* way — a non-finite or negative `h_fg`, a mismatched saturation set,
or a future Region-3 property implementation which, unlike the present one,
would correctly drive `h_fg` to zero in the last few kPa below `p_c`. It is
checked after the pressure guard, so for water as the property layer stands
today it does not fire; that is stated rather than hidden.

# Why `1e4` J/kg specifically

Far enough above zero that the division is not dominated by the difference
of two large, nearly equal enthalpies, and far below any subcritical value
the property layer produces — the measured minimum over the sweep in
`latent_heat_guard_fires_only_next_to_the_critical_point` (2026-08-12) is
`3.7940e5 J/kg` at `p_c`, thirty-eight times this threshold.

```rust
pub const MIN_LATENT_HEAT_FOR_MASS_TRANSFER: f64 = 1.0e4;
```

## Module `properties`

The steam/water property layer both 1-D solvers evaluate per cell per step.

# What belongs here

A thin, *cached* face onto [`tampines_steam_tables`]'s IAPWS-IF97
functions, shaped for what a two-phase system-code marching loop actually
asks for:

- [`SaturatedProperties`] — everything on the saturation line at one
  pressure (`T_sat`, `h_f`, `h_g`, `ρ_f`, `ρ_g`, `μ_f`, `μ_g`) plus the
  pressure derivatives the pressure equation needs.
- [`TwoPhaseState`] — the result of a `(p, h)` flash: quality, void
  fraction, mixture density, temperature.
- [`SaturatedTransport`] — the *conduction* properties on the saturation
  line (`λ_f`, `λ_g`, `c_p,f`, `c_p,g`), which only the six-equation solver
  needs and which are kept out of [`SaturatedProperties`] because they cost
  four extra IF97 evaluations per call.
- [`PhaseState`] — a **single-phase** `(p, h_k)` state for *one* phase,
  including the bounded **metastable** extensions (superheated liquid,
  subcooled vapour) that a six-equation model needs and a four-equation
  equilibrium model does not.

# What does not

The IAPWS correlations themselves. Those are `tampines-steam-tables`'s job
and this module never reimplements one — per the workspace rule that raw
property-table equations do not belong in `tampines`.

# Why a cache exists at all

`sat_temp_4` is a backward correlation and the saturated-property set costs
several IF97 evaluations. A two-phase march wants that set at *every* cell
at *every* step, and neighbouring cells in a blowdown sit at nearly the same
pressure. [`SaturatedProperties::at`] is therefore memo-free but cheap to
call, and the solvers hold one instance per cell and refresh it only when
the cell pressure has moved by more than a relative tolerance. That is a
performance decision with a correctness consequence, so the tolerance is
public and documented at [`SaturatedProperties::is_stale_for`].

# Units

Constructed from `uom` at the boundary; every field is raw `f64` in strict
SI — pascal, kelvin, `J/kg`, `kg/m³`, `Pa·s` — because these are read inside
the per-cell loop.

```rust
pub mod properties { /* ... */ }
```

### Types

#### Struct `SaturatedProperties`

Every saturated property at one pressure, plus the pressure derivatives the
semi-implicit pressure equation needs.

# What each field is

All on the saturation line at [`pressure`](Self::pressure), so
`T_f = T_g = T_sat` by definition — this is the *equilibrium* saturation
state, and a solver that wants thermal non-equilibrium (as
[`super::two_fluid::TwoFluid1d`] does) carries its own phase temperatures
and uses these only as the interface state.

# Units

Raw `f64`, strict SI: pascal, kelvin, `J/kg`, `kg/m³`, `Pa·s`.

```rust
pub struct SaturatedProperties {
    pub pressure: f64,
    pub t_sat: f64,
    pub h_f: f64,
    pub h_g: f64,
    pub rho_f: f64,
    pub rho_g: f64,
    pub mu_f: f64,
    pub mu_g: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | The pressure these were evaluated at \[Pa\]. |
| `t_sat` | `f64` | Saturation temperature `T_sat(p)` \[K\]. |
| `h_f` | `f64` | Saturated-liquid specific enthalpy `h_f` \[J/kg\]. |
| `h_g` | `f64` | Saturated-vapour specific enthalpy `h_g` \[J/kg\]. |
| `rho_f` | `f64` | Saturated-liquid density `ρ_f` \[kg/m³\]. |
| `rho_g` | `f64` | Saturated-vapour density `ρ_g` \[kg/m³\]. |
| `mu_f` | `f64` | Saturated-liquid dynamic viscosity `μ_f` \[Pa·s\]. |
| `mu_g` | `f64` | Saturated-vapour dynamic viscosity `μ_g` \[Pa·s\]. |

##### Implementations

###### Methods

- ```rust
  pub fn at(p: f64) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Evaluate the whole saturated set at pressure `p` \[Pa\].

- ```rust
  pub fn h_fg(self: Self) -> f64 { /* ... */ }
  ```
  Latent heat of vaporisation `h_fg = h_g − h_f` \[J/kg\].

- ```rust
  pub fn is_stale_for(self: Self, p: f64) -> bool { /* ... */ }
  ```
  Whether this cached set is too stale to use at pressure `p` \[Pa\].

- ```rust
  pub fn saturation_temperature(self: Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Saturation temperature as a `uom` quantity, for callers outside the

- ```rust
  pub fn liquid_density(self: Self) -> MassDensity { /* ... */ }
  ```
  Saturated-liquid density as a `uom` quantity.

- ```rust
  pub fn vapour_density(self: Self) -> MassDensity { /* ... */ }
  ```
  Saturated-vapour density as a `uom` quantity.

- ```rust
  pub fn latent_heat(self: Self) -> AvailableEnergy { /* ... */ }
  ```
  Latent heat as a `uom` quantity.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SaturatedProperties { /* ... */ }
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
    fn eq(self: &Self, other: &SaturatedProperties) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TwoPhaseState`

The thermodynamic state of one cell, obtained from a `(p, h)` flash.

# Sign and range conventions

- [`quality`](Self::quality) `x ∈ [0, 1]`: the **equilibrium** vapour mass
  fraction. A subcooled cell returns `0`, a superheated cell `1` — clipped
  deliberately, because the *thermodynamic* quality outside the dome is not
  a mass fraction and feeding a negative one into a void-fraction formula
  produces nonsense.
- [`void_fraction`](Self::void_fraction) `α ∈ [0, 1]`: the **volume**
  fraction of vapour. Related to quality by
  `α = x ρ_f / (x ρ_f + (1−x) ρ_g)` for a homogeneous mixture — note this
  is the *no-slip* relation, so it is the correct initial value but a
  drift-flux or two-fluid solver transports `α` independently thereafter
  and the two stop agreeing. That divergence is the physics, not an error.
- [`density`](Self::density) `ρ_m` \[kg/m³\]: the mixture density
  `α ρ_g + (1−α) ρ_f`.

# Units

Raw `f64` in strict SI.

```rust
pub struct TwoPhaseState {
    pub pressure: f64,
    pub enthalpy: f64,
    pub quality: f64,
    pub void_fraction: f64,
    pub density: f64,
    pub temperature: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | Pressure \[Pa\]. |
| `enthalpy` | `f64` | Mixture specific enthalpy \[J/kg\]. |
| `quality` | `f64` | Equilibrium vapour mass fraction `x ∈ [0, 1]` \[-\]. |
| `void_fraction` | `f64` | Vapour volume fraction `α ∈ [0, 1]` \[-\], at no slip. |
| `density` | `f64` | Mixture density `ρ_m` \[kg/m³\]. |
| `temperature` | `f64` | Temperature \[K\]. |

##### Implementations

###### Methods

- ```rust
  pub fn flash(p: f64, h: f64, saturated: SaturatedProperties) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Flash `(p, h)` to a full state, given the saturated set at that

- ```rust
  pub fn compressibility(self: Self, relative_step: f64) -> Result<f64, TampinesError> { /* ... */ }
  ```
  Compressibility `ψ = ∂ρ/∂p|_h` \[s²/m²\] by central finite difference

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TwoPhaseState { /* ... */ }
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
    fn eq(self: &Self, other: &TwoPhaseState) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SaturatedTransport`

The saturation-line **conduction** properties an interfacial heat-transfer
closure needs, at one pressure.

# What each field is, and why they are here rather than in [`SaturatedProperties`]

[`super::two_fluid`]'s two-resistance closure
([`outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer`]) wants a
thermal conductivity per phase and a Prandtl number per phase; the Prandtl
number needs `c_p`. Those four numbers cost four extra IF97 evaluations, and
[`super::drift_flux`] — which evaluates [`SaturatedProperties`] several
times per cell per step and does not have an energy equation per phase —
needs none of them. Keeping them in a separate type means the four-equation
solver does not pay for the six-equation solver's closures.

# Where they are evaluated, and the approximation that implies

**On the saturation line at `pressure`**, via the two-phase entry points at
quality `0` and `1`, exactly as [`SaturatedProperties`] does for viscosity
and for the same reason: taking both phases from one entry point keeps them
mutually consistent.

The approximation: [`super::two_fluid`] evaluates these at `T_sat(p)` even
when the phase itself is metastable at `T_k ≠ T_sat(p)`. That is deliberate.
The single-phase IF97 conductivity entry point flashes its own density from
`(T, p)` and would route a metastable-liquid state — which sits at
`p < p_sat(T_l)` — into the *vapour* region, returning a conductivity an
order of magnitude wrong, silently. Over the bounded metastable departure
this module admits ([`MAX_METASTABLE_LIQUID_SUPERHEAT`]) the real variation
in `λ` and `c_p` is under a percent, so the saturation-line value is both
more accurate and vastly safer than the branch it avoids.

# Units

Raw `f64`, strict SI: pascal, `W/(m·K)`, `J/(kg·K)`.

```rust
pub struct SaturatedTransport {
    pub pressure: f64,
    pub lambda_f: f64,
    pub lambda_g: f64,
    pub cp_f: f64,
    pub cp_g: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | The pressure these were evaluated at \[Pa\]. |
| `lambda_f` | `f64` | Saturated-liquid thermal conductivity `λ_f` \[W/(m·K)\]. |
| `lambda_g` | `f64` | Saturated-vapour thermal conductivity `λ_g` \[W/(m·K)\]. |
| `cp_f` | `f64` | Saturated-liquid isobaric specific heat `c_p,f` \[J/(kg·K)\]. |
| `cp_g` | `f64` | Saturated-vapour isobaric specific heat `c_p,g` \[J/(kg·K)\]. |

##### Implementations

###### Methods

- ```rust
  pub fn at(p: f64) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Evaluate the conduction set at pressure `p` \[Pa\].

- ```rust
  pub fn is_stale_for(self: Self, p: f64) -> bool { /* ... */ }
  ```
  Whether this cached set is too stale to use at pressure `p` \[Pa\].

- ```rust
  pub fn liquid_prandtl(self: Self, saturated: SaturatedProperties) -> Result<f64, TampinesError> { /* ... */ }
  ```
  Liquid Prandtl number `Pr_f = c_p,f μ_f / λ_f` \[-\].

- ```rust
  pub fn vapour_prandtl(self: Self, saturated: SaturatedProperties) -> Result<f64, TampinesError> { /* ... */ }
  ```
  Vapour Prandtl number `Pr_g = c_p,g μ_g / λ_g` \[-\].

- ```rust
  pub fn liquid_conductivity(self: Self) -> uom::si::f64::ThermalConductivity { /* ... */ }
  ```
  Saturated-liquid thermal conductivity as a `uom` quantity.

- ```rust
  pub fn vapour_conductivity(self: Self) -> uom::si::f64::ThermalConductivity { /* ... */ }
  ```
  Saturated-vapour thermal conductivity as a `uom` quantity.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SaturatedTransport { /* ... */ }
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
    fn eq(self: &Self, other: &SaturatedTransport) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `PhaseState`

The state of **one** phase at `(p, h_k)`, including the bounded metastable
extensions a six-equation model needs.

# How this differs from [`TwoPhaseState`], and why both exist

[`TwoPhaseState::flash`] answers *"what mixture does this `(p, h)` describe
at equilibrium?"* — it puts the state on the saturation line whenever `h`
lands inside the dome. That is exactly right for HEM and for drift flux,
which carry one energy equation and therefore one temperature.

A six-equation model carries `h_g` and `h_l` **separately**, and the whole
point is that neither is required to sit on the saturation line. `h_l` below
`h_f` is ordinary subcooled liquid; `h_l` *above* `h_f` is metastable
superheated liquid, which the equilibrium flash would call a two-phase
mixture and which is instead the state that drives flashing. So this type
answers a different question: *"given that this is the liquid (or the
vapour), what temperature and density does `(p, h_k)` imply?"*

# Sign convention of [`metastable_departure`](Self::metastable_departure)

Positive means "further into the metastable region": for a liquid,
`T − T_sat` (superheat); for a vapour, `T_sat − T` (subcooling). Zero or
negative means the state is thermodynamically stable as that phase. A caller
that wants to know whether it is relying on the extrapolated branch reads
this field rather than re-deriving it.

# Units

Raw `f64`, strict SI: pascal, `J/kg`, kelvin, `kg/m³`.

```rust
pub struct PhaseState {
    pub pressure: f64,
    pub enthalpy: f64,
    pub temperature: f64,
    pub density: f64,
    pub metastable_departure: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | Pressure \[Pa\]. |
| `enthalpy` | `f64` | This phase's specific enthalpy \[J/kg\]. |
| `temperature` | `f64` | This phase's temperature \[K\]. |
| `density` | `f64` | This phase's density \[kg/m³\]. |
| `metastable_departure` | `f64` | How far into the metastable region the state sits \[K\]; see the<br>type-level docs for the sign convention. Zero or negative for a stable<br>state. |

##### Implementations

###### Methods

- ```rust
  pub fn liquid_at(p: f64, h: f64, saturated: SaturatedProperties) -> Result<Self, TampinesError> { /* ... */ }
  ```
  The **liquid** state at `(p, h)`, allowing bounded metastable superheat.

- ```rust
  pub fn vapour_at(p: f64, h: f64, saturated: SaturatedProperties) -> Result<Self, TampinesError> { /* ... */ }
  ```
  The **vapour** state at `(p, h)`, allowing bounded metastable

- ```rust
  pub fn thermodynamic_temperature(self: Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Temperature as a `uom` quantity, for callers outside the marching loop.

- ```rust
  pub fn mass_density(self: Self) -> MassDensity { /* ... */ }
  ```
  Density as a `uom` quantity.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PhaseState { /* ... */ }
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
    fn eq(self: &Self, other: &PhaseState) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Constants and Statics

#### Constant `SATURATION_CACHE_TOLERANCE`

Relative pressure change beyond which a cached [`SaturatedProperties`] is
considered stale and must be re-evaluated.

`1e-4` — a 0.7 kPa move at 7 MPa. Chosen because the saturation temperature
varies as roughly `dT_sat/dp ≈ 3e-6 K/Pa` near 7 MPa, so this bounds the
staleness of `T_sat` at about `2e-3` K, three orders below any temperature
difference a solver resolves.

```rust
pub const SATURATION_CACHE_TOLERANCE: f64 = 1.0e-4;
```

#### Constant `P_MIN_IF97`

The IAPWS-IF97 lower pressure validity limit \[Pa\] — the triple point.

Below this the flash has no defined answer. Both solvers refuse rather than
clamp, for the reason set out in [`crate::multiphase_1d`]: a clamped
thermodynamic state produces a plausible number that is wrong.

```rust
pub const P_MIN_IF97: f64 = 611.657;
```

#### Constant `P_MAX_IF97`

The IAPWS-IF97 upper pressure validity limit \[Pa\].

```rust
pub const P_MAX_IF97: f64 = 100.0e6;
```

#### Constant `T_MAX_REGION_1`

The IAPWS-IF97 Region-1 upper temperature limit \[K\].

Region 1 (subcooled liquid) is defined for `273.15 K ≤ T ≤ 623.15 K`. The
metastable-liquid extension in [`PhaseState::liquid_at`] never brackets
above this.

```rust
pub const T_MAX_REGION_1: f64 = 623.15;
```

#### Constant `T_MAX_REGION_2`

The IAPWS-IF97 Region-2 upper temperature limit \[K\].

```rust
pub const T_MAX_REGION_2: f64 = 1073.15;
```

#### Constant `T_MIN_IF97`

The IAPWS-IF97 lower temperature limit \[K\] — the triple point.

```rust
pub const T_MIN_IF97: f64 = 273.16;
```

#### Constant `MAX_METASTABLE_LIQUID_SUPERHEAT`

How far above `T_sat(p)` the **metastable superheated liquid** branch of
[`PhaseState::liquid_at`] will go before refusing \[K\].

# Why this exists, and why it is bounded

A flashing blowdown does not put the liquid on the saturation line. The
pressure falls faster than the liquid can boil, so the liquid is left
**metastable — superheated relative to the local `T_sat(p)`** — and the
resulting superheat is precisely what drives the interfacial mass transfer
in [`super::two_fluid`]. A property layer that refuses `T_l > T_sat(p)`
cannot represent that state at all, which is why the equilibrium flash
[`TwoPhaseState::flash`] is not enough for a six-equation model.

`30.0 K`. IAPWS-IF97 publishes **no** metastable-liquid equation, so this
branch is an *extrapolation* of the Region-1 Gibbs equation past its stated
validity boundary — the same extrapolation the system codes make, and
smooth because the Region-1 equation is analytic. It is bounded rather than
unbounded because an extrapolation is only credible near the boundary:
measured flashing superheats in depressurisation transients are a few K to
a few tens of K, so 30 K covers the physics with margin while still
refusing a state that has clearly gone wrong. **Beyond this the state is
refused, not extrapolated** — see [`super::two_fluid`]'s
refusal-not-clamping rule.

```rust
pub const MAX_METASTABLE_LIQUID_SUPERHEAT: f64 = 30.0;
```

#### Constant `MAX_METASTABLE_VAPOUR_SUBCOOLING`

How far below `T_sat(p)` the **metastable subcooled vapour** branch of
[`PhaseState::vapour_at`] will go before refusing \[K\].

`30.0 K`, the mirror of [`MAX_METASTABLE_LIQUID_SUPERHEAT`] and bounded for
the same reason.

# Why the stable Region-2 equation is extrapolated rather than IAPWS's own
metastable-vapour equation

`tampines-steam-tables` does carry
[`tampines_steam_tables::region_2_vapour::metastable_region_2`], the IF97
metastable-vapour (supersaturated-steam) equation. It is **deliberately not
used here**, for two reasons:

1. **It is a different equation, so it does not meet the stable one at the
   saturation line.** [`SaturatedProperties`] takes `h_g` and `ρ_g` from the
   *stable* Region-2 equation, and [`super::two_fluid`]'s interfacial energy
   balance is built on the interface sitting exactly at those saturated
   values. Switching formulation at `T_sat` would put a small step in
   `h_g(T)` precisely at the state where every flashing term is evaluated,
   and the exact cancellation the conservation test pins would be lost.
2. **Its validity stops at 10 MPa** (IF97 restricts the metastable-vapour
   equation to `p ≤ 10 MPa` and to the region above the 5 % equilibrium
   moisture line), whereas this module's stated pressure range is the full
   IF97 span. Using it would mean a *third* branch keyed on pressure.

Both choices are extrapolations of comparable size a few K from the dome;
the one taken here is the one that stays continuous with everything else in
this module. If a future case needs genuine supersaturated-steam accuracy —
nozzle condensation shocks, say — the metastable equation is the right tool
and this constant is where that decision would be revisited.

```rust
pub const MAX_METASTABLE_VAPOUR_SUBCOOLING: f64 = 30.0;
```

#### Constant `TRANSPORT_CACHE_TOLERANCE`

Relative pressure change beyond which a cached [`SaturatedTransport`] is
considered stale.

`1e-2`, a hundred times looser than [`SATURATION_CACHE_TOLERANCE`], because
the quantities it holds enter the answer far more weakly. `λ` and `c_p` set
the Nusselt/Prandtl scaling of the interfacial heat transfer, and both vary
by well under a percent over a 1 % pressure change away from the critical
point — whereas `T_sat` sets the *driving temperature difference* of the
same closure and therefore has to be tight. The looser tolerance matters:
[`SaturatedTransport::at`] costs four IF97 evaluations, one of them the
R15-11 conductivity with its critical-enhancement term, which is the most
expensive property call in this module.

```rust
pub const TRANSPORT_CACHE_TOLERANCE: f64 = 1.0e-2;
```

## Module `two_fluid`

The 1-D **six-equation two-fluid** solver — separate mass, momentum and
energy equations for each phase.

# The model

Two-fluid drops *both* remaining equilibrium assumptions of
[`super::drift_flux`]. Each phase gets its own mass, momentum and energy
equation — six in total — so the phases may differ in both velocity *and*
temperature. That is what a blowdown actually needs: the liquid can stay
subcooled while vapour at the wall is superheated, and neither drift flux
nor HEM can represent it.

For `k ∈ {g, l}`, with `m_k = α_k ρ_k` the phase mass concentration
\[kg/m³\]:

`∂m_k/∂t + ∂(m_k u_k)/∂x = Γ_k`  (phase mass, `Γ_g = −Γ_l`)

`m_k (∂u_k/∂t + u_k ∂u_k/∂x) = −α_k ∂p/∂x + m_k g_x + F_k^d + F_k^vm + Γ_k (u^i − u_k) − F_k^wall`  (phase momentum)

`∂(m_k h_k)/∂t + ∂(m_k h_k u_k)/∂x = α_k ∂p/∂t + Q_k^i + Γ_k h_k^i`  (phase energy)

and the **volume constraint** `α_g + α_l = 1`, which is what the pressure
equation enforces (see "The pressure equation" below).

The momentum equation is written in the *non-conservative* velocity form —
the conservative form minus `u_k ×` the phase mass equation — which is why
the mass-transfer momentum source appears as `Γ_k (u^i − u_k)` rather than
`Γ_k u^i`.

# The regularisation, and exactly what is and is not claimed

The naive six-equation system with drag alone has complex characteristics.
What this solver does about that is **traced to upstream practice, and is
not claimed to fix it**:

> **The regularisation implemented here — virtual mass inside the implicit
> 2×2 phase-coupling block, with `C_vm = 0.5` and `residualAlpha`
> flooring — is TRACED TO UPSTREAM PRACTICE in OpenFOAM's
> `multiphaseEuler`. It is NOT proven, here or upstream, to restore
> hyperbolicity or to make the six-equation system well posed. No
> characteristic analysis has been performed by this project, and upstream
> states no reasoning of its own anywhere in its source.**

The evidence behind that sentence, from
`crates/tampines/docs/six-equation-regularisation.md` (a source study of the
vendored OpenFOAM tree, read 2026-08-12, every claim carrying a
`file:line`):

- **There is no interfacial-pressure term in `multiphaseEuler` at all** —
  no `pInterface`, no Stuhmiller, no Bestion, anywhere in the vendored tree
  (study §1). So none is added here. The scaffold this file replaces
  suggested one was needed; the source says upstream does not use one.
- **What upstream uses for a fluid-fluid pair is virtual mass**,
  `K_vm = max(α_d, α_res) C_vm ρ_c`
  (`dispersedVirtualMassModel.C:51-67`), and it is folded **inside** the
  implicit phase-coupling matrix — on the diagonal *and* the off-diagonal
  (`momentumTransferSystem.C:704-762`) — never beside the drag term and
  never as an explicit source. That is reproduced here exactly; see
  [`PhaseCouplingBlock`].
- `C_vm = 0.5` is what every gas-liquid tutorial in the vendored tree sets
  (study §3.3), and it is [`DEFAULT_VIRTUAL_MASS_COEFFICIENT`] here.
- **`residualAlpha` flooring** (`cellPressureCorrector.C:82-91`,
  `momentumTransferSystem.C:617-620`) is a *numerical* device, not physics,
  and it is what keeps the 2×2 block invertible at a blowdown front where a
  phase vanishes. Both of its uses are ported — see [`PhaseCouplingBlock`].

And the counter-evidence, reported rather than suppressed: the study could
**not** confirm that virtual mass *is* the well-posedness fix. Upstream
never says so, and two of its thirty tutorials (`damBreak4phase`,
`hydrofoil`) run with drag alone and no regularising term at all. Treat
"this is what OpenFOAM does" as exactly that, and nothing more.

A consequence worth stating plainly: because the regularisation is not
known to make the system well posed, **grid refinement is not guaranteed to
improve a result from this solver**, and a converged mesh-independent
solution may not exist to converge to. Any V&V case run on it owes a
refinement study and a `C_vm` sensitivity (study §9.5), not a single-mesh
number.

# Where the closures come from

Beads `op-dt3.12` / `op-dt3.13` require 1-D closures to trace back to the
OUTRAM-FOAM 3-D reference rather than be invented. Concretely:

1. **Interfacial drag, heat transfer and mass transfer** are *not* written
   down in this file. They come from [`super::interfacial`], which calls
   [`outram_foam_multiphase::two_fluid::DragModel`] and
   [`outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer`]
   through their own public API.
2. **Correction to this module's previous documentation.** It used to say,
   under "what still has to be decided", that the 3-D reference *"has no
   interfacial heat-transfer closure at all — it is isothermal — so there is
   nothing to trace back to"*. **That is false as of 2026-08-11:**
   `outram_foam_multiphase::heat_transfer` exists (Ranz-Marshall, spherical
   conduction, Gunn, ported from OpenFOAM's `multiphaseEuler` heat-transfer
   models), [`super::interfacial`] consumes it, and this solver consumes
   that. The old sentence is corrected here rather than left standing.
3. **Virtual mass is the one closure this file writes down itself**, and
   that is a deviation which is stated rather than hidden. The reference
   crate's [`outram_foam_multiphase::two_fluid::InterfacialForce::VirtualMass`]
   is an unported scaffold whose `momentum_coefficient` returns
   `MultiphaseError::NotImplemented`, so there is nothing to consume. The
   formula used here — [`virtual_mass_coefficient`] — is transcribed from
   OpenFOAM's `dispersedVirtualMassModel.C:51-67` and
   `constantVirtualMassCoefficient.C:71-79` via the source study, **not**
   invented. When the reference crate gains the closure, this function is
   the single place to replace.

# Numerical method

The same **semi-implicit pressure-based march** [`super::drift_flux`] uses,
and for the same reason (see [`crate::multiphase_1d`]): an explicit
compressible march would be limited by the acoustic CFL. Each step:

1. **Face momentum.** For every face, assemble the 2×2 implicit
   drag + virtual-mass coupling block ([`PhaseCouplingBlock`]) and invert it
   in closed form, giving the force-only face velocities `u*_k` and the
   per-phase pressure sensitivities `d_k`.
2. **Pressure.** Assemble and solve a tridiagonal pressure equation
   ([`thomas_solve`](super::thomas_solve)) built from the volume constraint.
3. **Correct** the face velocities with the new pressure gradient.
4. **Transport** the four conserved cell quantities `m_g`, `m_l`,
   `m_g h_g`, `m_l h_l` with donor-cell (first-order upwind) fluxes.
5. **Exchange** heat and mass at the interface, implicitly in the phase
   enthalpies, then recover `α_k`, `ρ_k`, `T_k` and the volume residual.

Steps 2-5 sit inside an **outer-corrector loop**, because the volume
constraint is nonlinear in `p`.

## The pressure equation

Divide each phase mass equation by `ρ_k` and sum. The `∂α_k/∂t` terms
cancel exactly against each other by `Σ α_k = 1`, leaving

`Σ_k (α_k ψ_k / ρ_k) ∂p/∂t + Σ_k (1/ρ_k) ∂(m_k u_k)/∂x = Γ (1/ρ_g − 1/ρ_l)`

with `ψ_k = ∂ρ_k/∂p|_{h_k}` \[s²/m²\] the **single-phase** compressibility
of phase `k` at frozen phase enthalpy. Two things about this are worth
reading carefully, because they are exactly where a six-equation model
differs from the four-equation one next door:

- **The right-hand side is the flashing term.** `1/ρ_g − 1/ρ_l` is the
  specific volume created per kilogram evaporated — about `0.076 m³/kg` for
  steam/water at 2.6 MPa — so interfacial mass transfer appears in the
  pressure equation as a volumetric source. That, and not an equilibrium
  flash, is what holds the pressure up on a flashing plateau here.
- **There is no kink to linearise across.** [`super::drift_flux`] needs a
  *secant* compressibility because its `ρ_m(p)|_h` has a kink at the
  saturation line where flashing switches on. Here `ρ_g(p, h_g)` lives
  entirely in IF97 Region 2 and `ρ_l(p, h_l)` entirely in Region 1, both
  smooth, so a plain one-sided tangent is the correct linearisation and the
  secant machinery is not needed. The stiffness moved out of the
  compressibility and into the interfacial exchange, which is why *that* is
  what gets solved implicitly here.

The assembled matrix is strictly diagonally dominant — the off-diagonals sum
to exactly the pressure-coefficient part of the diagonal, and the compliance
term `V Σ_k α_k ψ_k / ρ_k / Δt` is strictly positive on top of it — so
[`thomas_solve`](super::thomas_solve) cannot hit a zero pivot on a
correctly assembled system.

## The volume residual, and why nothing is renormalised

The four transported quantities are the **primary state**; `α_g`, `α_l`,
`ρ_k` and `T_k` are *derived* from them and the pressure. Nothing forces
`α_g + α_l = 1` afterwards. The residual

`R = α_g + α_l − 1 = m_g/ρ_g(p, h_g) + m_l/ρ_l(p, h_l) − 1`

is instead fed back as a source into the next corrector's pressure equation
(`δp = R / C`, the Newton step on the constraint), so the loop drives it to
zero. Upstream renormalises instead — `MULES::limitSumCorr` plus an optional
final re-scaling, `phaseSystemSolve.C:580`, `:820-` — which is cheaper but
silently destroys the exact conservation of the transported masses. Here
**the masses are exactly conserved by construction and the constraint
violation is reported** as
[`TwoFluidReport::max_volume_residual`], with
[`MAX_VOLUME_RESIDUAL`] a hard refusal beyond which the step is rejected
rather than returned.

## Bounded α transport

Study §9.3 asks for the 1-D analogue of MULES. What is here is its minimum:
first-order **donor-cell** phase-mass transport, which is monotone, plus a
**refusal** (not a clip) if `α_k` leaves `[0, 1]`. There is no flux limiter
and no interface compression. Donor-cell carries first-order numerical
diffusion of `α`, so a flashing front is smeared; that is stated here rather
than discovered in a plot.

# Honest scope — what this solver does NOT do

[`crate::multiphase_1d`] lists what applies to both 1-D solvers (no wall
heat transfer, no flow-regime map, no interfacial-area transport, HEM break
model). Specific to this one:

- **Not validated against anything.** Every test in `two_fluid_tests.rs` is
  *verification* — closed forms, invariants and degenerate limits. No result
  from this solver has been compared with an experiment. The Edwards and
  Marviken cases are a separate piece of work (beads `op-s1a0`,
  `op-dt3.13`).
- **Turbulent dispersion is absent and that is a real omission**, not a
  simplification: it is a genuinely regularising `D ∇α` term that upstream
  can call on (study §5.1) and a 1-D area-averaged model, having no
  resolved turbulence, cannot. Lift, wall lubrication, surface tension and
  interface compression are omitted for *geometric* reasons (study §9.4) and
  nothing is lost by it; turbulent dispersion is different.
- **Granular phase pressure is deliberately not implemented.** It is
  identically zero for fluid phases upstream
  (`phaseCompressibleMomentumTransportModel.C:99-108`), so omitting it
  reproduces upstream's fluid-fluid behaviour exactly.
- **The virtual-mass material derivative is only partly implicit.**
  Upstream assembles the whole of `DU/Dt = ∂_t U + (U·∇)U` implicitly
  (`MovingPhaseModel.C:533-536`). Here the `∂_t` half is implicit (the
  `K_vm/Δt` entries of the block) and the `(U·∇)U` half is carried
  explicitly at old-time velocities. **The full virtual-mass force is still
  in the equation** — only the implicit/explicit split differs, which
  affects the stability of the solve and not the equations solved.
- **Wall friction is partitioned by volume fraction**, `F_k^wall = α_k ρ_k f_k |u_k| u_k / (2 D_h)`
  with each phase's own Reynolds number. There is no regime-dependent
  wall-friction split (which phase wets the wall), so this is crude wherever
  friction matters.
- **The `u ∂p/∂x` pressure-work term is omitted** from the energy equations,
  which carry `α_k ∂p/∂t` only — matching the equation set stated above and
  matching [`super::drift_flux`], so the two solvers can be compared in the
  equilibrium limit.
- **Boundary conditions are velocity-type only**
  ([`TwoFluidBoundary`]): closed, prescribed velocity, and an HEM choked
  outlet. There is no Dirichlet-pressure boundary, so the pressure floats on
  the compliance diagonal — right for a blowdown, wrong for a pipe fed from
  a plenum. [`super::drift_flux`]'s `ReservoirInlet` and `PressureOutlet`
  have no counterpart here yet.
- **The metastable bounds are the binding constraint on a fast transient.**
  [`super::properties::MAX_METASTABLE_LIQUID_SUPERHEAT`] is 30 K, and a
  depressurisation fast enough to superheat the liquid past that is
  **refused**, not extrapolated. Whether a given case stays inside it is
  decided by the interfacial closure set — chiefly the bubble diameter and
  the initial void fraction — and a case that trips the bound needs a
  different closure set or a nucleation model, not a looser bound.

# Units

Constructors and accessors are `uom`-typed. The marching loop carries raw
`f64` in strict SI — pascal, kelvin, `J/kg`, `kg/m³`, `m/s`, `N/m³`,
`W/m³`, `kg/(m³·s)` — because every one of them is read per cell per
corrector per step. Every raw-`f64` boundary says so.

```rust
pub mod two_fluid { /* ... */ }
```

### Types

#### Enum `TwoFluidBoundary`

What closes an end of the pipe.

Enum dispatch per the workspace rule: the set of 1-D end conditions is
closed, and adding one must force every `match` to be revisited.

**Every variant is a *velocity* boundary.** That is a deliberate restriction
of the first implementation and not an oversight: a velocity face carries no
pressure sensitivity, so it stays out of the pressure matrix entirely and
the pressure floats on the compliance diagonal. That is exactly what a
closed-vessel blowdown wants and exactly what a pipe fed from a plenum does
not. [`super::drift_flux::AxialBoundary`]'s `ReservoirInlet` and
`PressureOutlet` have no counterpart here yet.

```rust
pub enum TwoFluidBoundary {
    Closed,
    PrescribedVelocity(f64),
    ChokedOutlet {
        area_fraction: f64,
        ambient_pressure: f64,
    },
}
```

##### Variants

###### `Closed`

A rigid closed end: `u_g = u_l = 0`. The `x = 0` end of the
Edwards–O'Brien pipe.

###### `PrescribedVelocity`

A prescribed face velocity \[m/s\], positive in `+x`, imposed on
**both** phases.

Inflow through this boundary re-injects the adjacent cell's state
(zero-gradient donor), because the variant carries no description of
what is entering. A case with sustained inflow needs a boundary that
does; there is none yet.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `ChokedOutlet`

A **choked (critical) outlet** through a break of area
`area_fraction × A`.

Each step the adjacent cell's mixture `(p, h_m)` is handed to the
crate's HEM critical-flow dispatcher for the throat mass flux `G*`, and
the equivalent full-face velocity `u = G* × area_fraction / ρ_m` is
imposed on **both** phases.

**Two modelling inconsistencies, both deliberate and both stated.**

1. *The break is HEM even though the pipe is six-equation*, exactly as
   [`super::drift_flux::AxialBoundary::ChokedOutlet`] documents at
   length. The critical-flow dispatcher is the piece of this crate
   actually exercised against a reference; substituting an unvalidated
   two-fluid choking model would trade a known inconsistency for an
   unknown one. Read that variant's doc comment for the dispatcher's
   real V&V status, including that Marviken is **not** gated.
2. *The break imposes no slip*, so the phases leave at the same
   velocity even where the pipe has developed slip. A real break
   separates the phases; nothing here does.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `area_fraction` | `f64` | Break area as a fraction of the pipe flow area, in `(0, 1]`. |
| `ambient_pressure` | `f64` | Ambient / containment back-pressure \[Pa\]. The outlet unchokes<br>once the critical throat pressure falls below it. |

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
    fn clone(self: &Self) -> TwoFluidBoundary { /* ... */ }
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
    fn eq(self: &Self, other: &TwoFluidBoundary) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `PhaseCouplingInputs`

Everything the 2×2 phase-coupling block at one face is assembled from.

Raw `f64` in strict SI throughout — this is a marching-loop boundary; every
field documents its unit.

```rust
pub struct PhaseCouplingInputs {
    pub alpha_g: f64,
    pub alpha_l: f64,
    pub rho_g: f64,
    pub rho_l: f64,
    pub k_d: f64,
    pub c_vm: f64,
    pub dispersed: super::interfacial::DispersedPhase,
    pub residual_alpha: f64,
    pub dt: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `alpha_g` | `f64` | Vapour volume fraction at the face `α_g` \[-\]. |
| `alpha_l` | `f64` | Liquid volume fraction at the face `α_l` \[-\]. |
| `rho_g` | `f64` | Vapour density at the face `ρ_g` \[kg/m³\], strictly positive. |
| `rho_l` | `f64` | Liquid density at the face `ρ_l` \[kg/m³\], strictly positive. |
| `k_d` | `f64` | Volumetric drag coefficient `K_d` \[kg/(m³·s)\] from the traced-back<br>[`outram_foam_multiphase::two_fluid::DragModel`], via<br>[`super::interfacial::InterfacialSources::volumetric_drag_coefficient`].<br>Must be `≥ 0`. |
| `c_vm` | `f64` | Added-mass coefficient `C_vm` \[-\], `≥ 0`. `0` disables virtual mass<br>entirely and leaves a pure drag block. |
| `dispersed` | `super::interfacial::DispersedPhase` | Which phase is dispersed — this decides which density and which volume<br>fraction enter `K_vm`; see [`virtual_mass_coefficient`]. |
| `residual_alpha` | `f64` | Residual volume fraction `α_res` \[-\], in `[0, 1)`. |
| `dt` | `f64` | Timestep `Δt` \[s\], strictly positive. |

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
    fn clone(self: &Self) -> PhaseCouplingInputs { /* ... */ }
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
    fn eq(self: &Self, other: &PhaseCouplingInputs) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `PhaseCouplingBlock`

The **2×2 implicit phase-coupling block** at one face — drag and virtual
mass together, inside one matrix, exactly as upstream forms them.

# What it is

Write each phase's discrete face momentum equation as

`A_k u_k^{n+1} = b_k − α_k ∂p/∂x`

with `A_k` the momentum diagonal \[kg/(m³·s)\] and `b_k` everything
explicit \[N/m³\]. Drag and virtual mass then couple the two rows:

- **drag** contributes `+K̃^d_k` to row `k`'s diagonal and `−K̃^d_k` to its
  off-diagonal (`momentumTransferSystem.C:617-634`);
- **virtual mass** contributes `+K̃^vm_k A^D_k` to the diagonal and
  `−K̃^vm_k A^D_j` to the off-diagonal
  (`momentumTransferSystem.C:704-762`);

where `A^D` is the diagonal of the material-derivative operator `DU/Dt`.
This implementation takes `A^D = 1/Δt` for both phases — the `∂_t` half of
`DU/Dt` — and carries the `(U·∇)U` half explicitly in `b_k`; see the module
docs, "Honest scope". With `A^D` equal for both phases the diagonal and
off-diagonal virtual-mass entries have the same magnitude, so drag and
virtual mass fold into a single coupling strength per row:

`c_k = [α_j / max(α_j, α_res)] · (K_d + K_vm / Δt)`

The `α_j / max(α_j, α_res)` factor — with `j` the **other** phase — is
upstream's vanishing-phase taper (`momentumTransferSystem.C:617-620`,
`:723-727`): exactly 1 wherever the other phase is present, tapering to 0
as it disappears.

The block is then

`[[A_g + c_g, −c_g], [−c_l, A_l + c_l]]`

with the residual-alpha-floored diagonals
`A_k = max(α_k, α_res) ρ_k / Δt` (`cellPressureCorrector.C:82-91`).

# Why it is invertible

`det = (A_g + c_g)(A_l + c_l) − c_g c_l = A_g A_l + A_g c_l + A_l c_g`.
Every term is a product of non-negative numbers, and `A_g A_l > 0` strictly
because the residual-alpha flooring keeps both diagonals positive even where
a phase has vanished. So `det > 0` always — no linear-algebra library, no
pivoting, no singularity at a blowdown front. Upstream reaches the same
answer through a per-cell LU (`momentumTransferSystem.C:461-499`); for two
phases that collapses to this closed form, which is the classical two-phase
**partial elimination algorithm**.

# Degenerate limits, which are the reason for the taper

- `α_g → 0`: `c_l → 0`, so row `l` decouples completely —
  [`solve`](Self::solve) returns `u_l = b_l / A_l` and
  [`pressure_coefficients`](Self::pressure_coefficients) returns
  `d_l = α_l / A_l`, the single-phase liquid answer. A vanishing phase
  cannot contaminate the phase that is present.
- `α_l → 0`: the mirror image.
- `K_d → ∞`: `u_g − u_l = (A_l b_g − A_g b_l)/det → 0`, the no-slip limit.

# Momentum conservation, and where the taper breaks it

Away from the vanishing limits both tapers are exactly 1, `c_g = c_l`, and
the off-diagonals are equal — so the drag and virtual-mass forces on the two
phases are exactly equal and opposite and mixture momentum is conserved to
machine precision. Inside the taper band (`α < α_res`) they are not, by
construction. That is the price of the numerical device and it is stated
rather than hidden.

# Units

Raw `f64`, strict SI. All four matrix entries are \[kg/(m³·s)\]; the tapered
virtual-mass coefficients are \[kg/m³\].

```rust
pub struct PhaseCouplingBlock {
    pub vapour_vapour: f64,
    pub vapour_liquid: f64,
    pub liquid_vapour: f64,
    pub liquid_liquid: f64,
    pub vapour_diagonal: f64,
    pub liquid_diagonal: f64,
    pub tapered_virtual_mass_vapour: f64,
    pub tapered_virtual_mass_liquid: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vapour_vapour` | `f64` | Row vapour, column vapour: `A_g + c_g` \[kg/(m³·s)\]. |
| `vapour_liquid` | `f64` | Row vapour, column liquid: `−c_g` \[kg/(m³·s)\]. |
| `liquid_vapour` | `f64` | Row liquid, column vapour: `−c_l` \[kg/(m³·s)\]. |
| `liquid_liquid` | `f64` | Row liquid, column liquid: `A_l + c_l` \[kg/(m³·s)\]. |
| `vapour_diagonal` | `f64` | The floored vapour momentum diagonal `A_g = max(α_g, α_res) ρ_g / Δt`<br>\[kg/(m³·s)\], kept so a caller can see how much of the diagonal is<br>inertia and how much is coupling. |
| `liquid_diagonal` | `f64` | The floored liquid momentum diagonal `A_l` \[kg/(m³·s)\]. |
| `tapered_virtual_mass_vapour` | `f64` | Tapered virtual-mass coefficient for the vapour row,<br>`K̃^vm_g = [α_l/max(α_l, α_res)] K_vm` \[kg/m³\]. Multiplies<br>`H^D_g − H^D_l` in the explicit remainder — see the module docs. |
| `tapered_virtual_mass_liquid` | `f64` | Tapered virtual-mass coefficient for the liquid row, `K̃^vm_l`<br>\[kg/m³\]. |

##### Implementations

###### Methods

- ```rust
  pub fn assemble(inputs: PhaseCouplingInputs) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Assemble the block from one face's state.

- ```rust
  pub fn determinant(self: &Self) -> f64 { /* ... */ }
  ```
  The determinant `A_g A_l + A_g c_l + A_l c_g` \[kg²/(m⁶·s²)\].

- ```rust
  pub fn solve(self: &Self, b_vapour: f64, b_liquid: f64) -> Result<(f64, f64), TampinesError> { /* ... */ }
  ```
  Solve `M u = b` for the two face velocities \[m/s\].

- ```rust
  pub fn pressure_coefficients(self: &Self, alpha_g: f64, alpha_l: f64) -> Result<(f64, f64), TampinesError> { /* ... */ }
  ```
  The per-phase pressure sensitivities `d_k = Σ_m (M⁻¹)_{km} α_m`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PhaseCouplingBlock { /* ... */ }
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
    fn eq(self: &Self, other: &PhaseCouplingBlock) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TwoFluidReport`

Per-step diagnostics from [`TwoFluid1d::step`].

Returned rather than logged, so a case can assert on it and a V&V test can
record measured values. Raw `f64` in strict SI throughout.

```rust
pub struct TwoFluidReport {
    pub time: f64,
    pub outlet_mass_flow: f64,
    pub max_void_fraction: f64,
    pub max_thermal_nonequilibrium: f64,
    pub min_void_fraction: f64,
    pub max_slip: f64,
    pub inventory: f64,
    pub max_courant: f64,
    pub max_volume_residual: f64,
    pub outer_correctors_used: usize,
    pub outlet_choked: bool,
    pub mass_transfer_limited_cells: usize,
    pub residual_alpha_floor_events: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time` | `f64` | Simulated time at the end of the step \[s\]. |
| `outlet_mass_flow` | `f64` | Mixture mass flow through the right-hand boundary \[kg/s\], positive<br>outward: `(m_g u_g + m_l u_l) A` at the last face. |
| `max_void_fraction` | `f64` | Largest void fraction anywhere \[-\]. |
| `max_thermal_nonequilibrium` | `f64` | Largest phase-temperature difference `|T_g − T_l|` anywhere \[K\] — the<br>quantity that justifies a six-equation model over drift flux. |
| `min_void_fraction` | `f64` | Smallest void fraction anywhere \[-\]. |
| `max_slip` | `f64` | Largest phase slip `|u_g − u_l|` at any face \[m/s\] — the quantity that<br>justifies a six-equation model over HEM. |
| `inventory` | `f64` | Total mass currently in the pipe \[kg\],<br>`Σ_i (m_g + m_l)_i × V_cell`. |
| `max_courant` | `f64` | Largest material Courant number `max_k |u_k| Δt / Δx` \[-\].<br><br>The stability figure that matters for the explicit donor-cell transport.<br>The *acoustic* Courant number is not limiting, because the pressure<br>solve is implicit. A value above 1 means transport has stepped past a<br>whole cell and the answer is not to be trusted. |
| `max_volume_residual` | `f64` | Largest volume-constraint residual `max_i |α_g + α_l − 1|` \[-\] left at<br>the end of the outer-corrector loop.<br><br>Zero to round-off means the correctors converged. See the module docs<br>for why this is reported rather than renormalised away, and<br>[`MAX_VOLUME_RESIDUAL`] for where it becomes a refusal. |
| `outer_correctors_used` | `usize` | How many outer correctors were actually taken \[-\]. |
| `outlet_choked` | `bool` | Whether either boundary was choked this step. |
| `mass_transfer_limited_cells` | `usize` | How many cells had their interfacial mass transfer **rate-limited** this<br>step \[-\].<br><br>Non-zero means `Γ` would have driven a phase mass below its<br>residual-alpha floor within the step and was scaled back (together with<br>both interfacial heat fluxes, so the interfacial energy balance stays<br>exact). It is a numerical device and it is counted so it cannot act<br>invisibly — see [`TwoFluid1d::step`]. |
| `residual_alpha_floor_events` | `usize` | How many cells had a phase mass floored at `α_res ρ_k` this step \[-\].<br><br>Non-zero means donor-cell transport drained a phase out of a cell<br>faster than the limiter could catch, and the residual-alpha floor<br>supplied the missing inventory. The mass it invents is of order<br>`α_res ρ_k`; at `α_res = 1e-6` that is `≈ 4e-5 kg/m³` for steam at<br>7 MPa. Counted for the same reason. |

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
    fn clone(self: &Self) -> TwoFluidReport { /* ... */ }
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
    fn eq(self: &Self, other: &TwoFluidReport) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TwoFluid1d`

A 1-D transient **six-equation two-fluid** solver for compressible
steam/water pipe flow.

# Layout

**Staggered**: scalars (`p`, `m_g`, `m_l`, `h_g`, `h_l`, and everything
derived from them) at cell centres, both phase velocities at faces.
Staggering rather than collocation because in 1-D it removes
pressure-velocity checkerboarding *by construction* — no Rhie-Chow
interpolation needed — and it is what the reference system codes do. Face
`j` is the left face of cell `j`; faces `0` and `n_cells` are the ends.

# The primary state, and what is derived from it

The four **transported** quantities are `m_g = α_g ρ_g`, `m_l = α_l ρ_l`
\[kg/m³\] and the two phase enthalpies `h_g`, `h_l` \[J/kg\]; with the
pressure `p` \[Pa\] those are the state. `α_g`, `α_l`, `ρ_g`, `ρ_l`, `T_g`
and `T_l` are **derived** — `ρ_k = ρ_k(p, h_k)` from
[`PhaseState`], then `α_k = m_k / ρ_k`. Nothing forces
`α_g + α_l = 1` after the fact; the pressure equation enforces it and the
leftover is reported as [`TwoFluidReport::max_volume_residual`].

# Units

Constructors and accessors are `uom`-typed. Internal state is raw `f64` in
strict SI: pascal, `J/kg`, `kg/m³`, `m/s`, `K`, `[-]`.

# What a validation case needs from this API

A benchmark harness (the Edwards–O'Brien and Marviken cases are separate
work, beads `op-s1a0` / `op-dt3.13`) needs exactly:
[`new`](Self::new) or [`bubbly`](Self::bubbly) to build it,
[`set_temperature_profile`](Self::set_temperature_profile) for a
non-isothermal initial condition,
[`set_left_boundary`](Self::set_left_boundary) /
[`set_right_boundary`](Self::set_right_boundary) for a closed end and a
choked break, [`step`](Self::step) in a loop, and the read-only accessors
[`pressure`](Self::pressure), [`void_fraction`](Self::void_fraction),
[`vapour_temperature`](Self::vapour_temperature),
[`liquid_temperature`](Self::liquid_temperature) and
[`slip_velocity`](Self::slip_velocity) to compare against gauge data.
[`set_virtual_mass_coefficient`](Self::set_virtual_mass_coefficient) and
[`set_initial_void_fraction`](Self::set_initial_void_fraction) are the two
knobs whose sensitivity such a case is obliged to report.

```rust
pub struct TwoFluid1d {
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
  pub fn new(pipe: Pipe1d, exchange: InterfacialExchange, pressure: Pressure, temperature: ThermodynamicTemperature, dt: Time) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Build a solver on `pipe`, initialised to a uniform `(p, T)` state.

- ```rust
  pub fn bubbly(pipe: Pipe1d, pressure: Pressure, temperature: ThermodynamicTemperature, dt: Time) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Build a solver with the conventional **bubbly-flow** closure set —

- ```rust
  pub fn set_temperature_profile(self: &mut Self, temperatures: &[ThermodynamicTemperature]) -> Result<(), TampinesError> { /* ... */ }
  ```
  Overwrite the cell temperatures from an axial profile, re-deriving the

- ```rust
  pub fn set_left_boundary(self: &mut Self, bc: TwoFluidBoundary) { /* ... */ }
  ```
  Set the boundary condition at the `x = 0` end.

- ```rust
  pub fn set_right_boundary(self: &mut Self, bc: TwoFluidBoundary) { /* ... */ }
  ```
  Set the boundary condition at the `x = L` end.

- ```rust
  pub fn virtual_mass_coefficient(self: &Self) -> Ratio { /* ... */ }
  ```
  The added-mass coefficient `C_vm` \[-\].

- ```rust
  pub fn set_virtual_mass_coefficient(self: &mut Self, c_vm: Ratio) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the added-mass coefficient `C_vm`.

- ```rust
  pub fn residual_alpha(self: &Self) -> f64 { /* ... */ }
  ```
  The residual volume fraction `α_res` \[-\] — see

- ```rust
  pub fn set_residual_alpha(self: &mut Self, residual_alpha: f64) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the residual volume fraction `α_res`, in `[0, 1)`.

- ```rust
  pub fn initial_void_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  The initial void fraction used where the `(p, T)` flash gives none —

- ```rust
  pub fn set_initial_void_fraction(self: &mut Self, alpha: f64) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the initial void fraction, in `(0, 1)`.

- ```rust
  pub fn set_outer_correctors(self: &mut Self, n: usize) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the number of outer correctors per step.

- ```rust
  pub fn outer_correctors(self: &Self) -> usize { /* ... */ }
  ```
  The number of outer correctors per step.

- ```rust
  pub fn set_pressure_under_relaxation(self: &mut Self, alpha: Ratio) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the pressure under-relaxation factor `α_p ∈ (0, 1]`.

- ```rust
  pub fn time(self: &Self) -> Time { /* ... */ }
  ```
  Elapsed simulated time.

- ```rust
  pub fn pipe(self: &Self) -> &Pipe1d { /* ... */ }
  ```
  The pipe geometry.

- ```rust
  pub fn interfacial_exchange(self: &Self) -> &InterfacialExchange { /* ... */ }
  ```
  The interfacial closure set.

- ```rust
  pub fn pressure(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell pressures \[Pa\], read-only.

- ```rust
  pub fn void_fraction(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell vapour volume fractions `α_g` \[-\], read-only.

- ```rust
  pub fn liquid_fraction(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell liquid volume fractions `α_l` \[-\], read-only.

- ```rust
  pub fn vapour_density(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell vapour densities \[kg/m³\], read-only.

- ```rust
  pub fn liquid_density(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell liquid densities \[kg/m³\], read-only.

- ```rust
  pub fn vapour_enthalpy(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell vapour specific enthalpies \[J/kg\], read-only.

- ```rust
  pub fn liquid_enthalpy(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell liquid specific enthalpies \[J/kg\], read-only.

- ```rust
  pub fn vapour_temperature(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell vapour temperatures \[K\], read-only. Above `T_sat(p)` where the

- ```rust
  pub fn liquid_temperature(self: &Self) -> &[f64] { /* ... */ }
  ```
  Cell liquid temperatures \[K\], read-only. Above `T_sat(p)` where the

- ```rust
  pub fn saturation_temperature(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Cell saturation temperatures `T_sat(p)` \[K\], read-only — the interface

- ```rust
  pub fn vapour_face_velocity(self: &Self) -> &[f64] { /* ... */ }
  ```
  Vapour face velocities \[m/s\], read-only, length `n_cells + 1`.

- ```rust
  pub fn liquid_face_velocity(self: &Self) -> &[f64] { /* ... */ }
  ```
  Liquid face velocities \[m/s\], read-only, length `n_cells + 1`.

- ```rust
  pub fn slip_velocity(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Face slip velocities `u_g − u_l` \[m/s\], length `n_cells + 1`.

- ```rust
  pub fn mixture_density(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Cell mixture densities `m_g + m_l` \[kg/m³\].

- ```rust
  pub fn inventory(self: &Self) -> Mass { /* ... */ }
  ```
  Total mass currently in the pipe.

- ```rust
  pub fn total_enthalpy(self: &Self) -> f64 { /* ... */ }
  ```
  Total enthalpy currently in the pipe \[J\], `Σ_i (m_g h_g + m_l h_l)_i V`.

- ```rust
  pub fn outlet_mass_flow(self: &Self) -> MassRate { /* ... */ }
  ```
  Mixture mass flow through the right-hand boundary \[kg/s\], from the

- ```rust
  pub fn cell_vapour_density(self: &Self, i: usize) -> MassDensity { /* ... */ }
  ```
  Vapour density of cell `i` as a `uom` quantity.

- ```rust
  pub fn cell_liquid_enthalpy(self: &Self, i: usize) -> AvailableEnergy { /* ... */ }
  ```
  Liquid specific enthalpy of cell `i` as a `uom` quantity.

- ```rust
  pub fn cell_vapour_temperature(self: &Self, i: usize) -> ThermodynamicTemperature { /* ... */ }
  ```
  Vapour temperature of cell `i` as a `uom` quantity.

- ```rust
  pub fn cell_liquid_temperature(self: &Self, i: usize) -> ThermodynamicTemperature { /* ... */ }
  ```
  Liquid temperature of cell `i` as a `uom` quantity.

- ```rust
  pub fn step(self: &mut Self) -> Result<TwoFluidReport, TampinesError> { /* ... */ }
  ```
  Advance one timestep.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `virtual_mass_coefficient`

**Attributes:**

- `MustUse { reason: None }`

The **volumetric virtual-mass coefficient** `K_vm` \[kg/m³\].

`K_vm = max(α_d, α_res) · C_vm · ρ_c`, with `α_d` the *dispersed*-phase
volume fraction and `ρ_c` the *continuous*-phase density.

# Provenance

Transcribed from OpenFOAM's
`dispersedVirtualMassModel.C:51-67` (the `max(α_d, α_res) · K_i` form) and
`constantVirtualMassCoefficient.C:71-79` (`K_i = C_vm ρ_c`), via
`crates/tampines/docs/six-equation-regularisation.md` §3.2.

It is written down here rather than consumed from the reference crate
because [`outram_foam_multiphase::two_fluid::InterfacialForce::VirtualMass`]
is an unported scaffold that returns `MultiphaseError::NotImplemented`.
**This is the one closure in this file that is not traced through a call
into the reference crate**, and this doc comment is the record of that.
When the reference gains the closure, replace this function's body with a
call to it and nothing else changes.

# Units and ranges

`alpha_dispersed` \[-\] in `[0, 1]`, `c_vm` \[-\] `≥ 0`, `rho_continuous`
\[kg/m³\] `> 0`, `residual_alpha` \[-\] in `[0, 1)`. Returns \[kg/m³\];
multiplied by a material acceleration \[m/s²\] it gives a force per unit
volume \[N/m³\], as required.

```rust
pub fn virtual_mass_coefficient(alpha_dispersed: f64, rho_continuous: f64, c_vm: f64, residual_alpha: f64) -> f64 { /* ... */ }
```

#### Function `as_velocity`

**Attributes:**

- `MustUse { reason: None }`

Wrap a raw SI velocity as a `uom` quantity, for callers outside the
marching loop.

```rust
pub fn as_velocity(u_si: f64) -> uom::si::f64::Velocity { /* ... */ }
```

#### Function `region_4_safe_pressure`

**Attributes:**

- `MustUse { reason: None }`

Nudge a pressure off the **exact** IF97 Region-4 saturation line, so that
[`SaturatedTransport::at`] does not panic.

# The defect this works around

`tampines_steam_tables`'s forward-equation region classifier
(`interfaces/functional_programming/pt_flash_eqm/mod.rs:134`) decides
"this `(T, p)` is the two-phase Region 4" with an **exact float equality**,
`pres == p_sat_reg4_pascal`. Region-4 then `panic!`s in `cp_tp_eqm_single_phase`
(`:211`) because `(T, p)` cannot resolve a saturated mixture without a
quality.

[`SaturatedTransport::at`] evaluates the conductivity at exactly
`(T_sat(p), p)`, so it lands on that equality — and therefore **panics** —
for every pressure that happens to round-trip bit-exactly through
`sat_pressure_4(sat_temp_4(p))`. Measured 2026-08-12 over a geometric sweep
of 10 790 pressures from 0.1 MPa to 22 MPa (ratio 1.0005): **105 of them
panic**, scattered rather than banded, and the bit-exact round-trip
predicts all 105 with **zero** false positives and **zero** false negatives.
`SaturatedProperties::at` panics at none of them, because it never routes
through the classifier.

A blowdown sweeps continuously through pressure, so hitting one is a
certainty rather than a risk; the panic aborts the whole march.

# What this does about it

Because the trigger is *exactly* the bit-equality, it can be predicted
rather than caught: this function evaluates
`sat_pressure_4(sat_temp_4(p))` and, only if it is bit-identical to `p`,
multiplies `p` by `1 + 4e-12`, retrying up to four times. Everywhere else it
returns `p` unchanged, bit for bit.

**This is a workaround in the consumer, not a fix.** The defect is in
`tampines-steam-tables` (a panic where an error belongs, and an exact float
comparison on a boundary) and belongs there; this function exists so the
six-equation solver is not blocked on it, and so the next reader finds the
diagnosis rather than a mysterious `+ 4e-12`.

# Units

`p` \[Pa\]; returns \[Pa\].

```rust
pub fn region_4_safe_pressure(p: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_VIRTUAL_MASS_COEFFICIENT`

Default added-mass coefficient `C_vm` \[-\].

`0.5`, the potential-flow value for a sphere, and the value **every**
gas-liquid tutorial in the vendored OpenFOAM tree sets
(`bubbleColumn/constant/momentumTransfer`; see the source study §3.3, which
surveyed all 30 `multiphaseEuler` tutorials). It is a **modelling choice
that changes the answer**, not a measured constant: study §9.5 requires any
benchmark run on this solver to report a sensitivity over at least
`C_vm ∈ [0, 0.5]`.

```rust
pub const DEFAULT_VIRTUAL_MASS_COEFFICIENT: f64 = 0.5;
```

#### Constant `DEFAULT_RESIDUAL_ALPHA`

Default residual volume fraction `α_res` \[-\].

[`RESIDUAL_ALPHA`] (`1e-6`), the value the OpenFOAM tutorials use
(`bubbleColumn/constant/phaseProperties:30,43`). A **numerical device**, not
physics: it floors the momentum diagonal so the 2×2 coupling block stays
invertible as a phase vanishes, and it tapers the drag and virtual-mass
coupling to zero as the *other* phase vanishes. See [`PhaseCouplingBlock`]
for exactly where each use enters.

```rust
pub const DEFAULT_RESIDUAL_ALPHA: f64 = RESIDUAL_ALPHA;
```

#### Constant `DEFAULT_BUBBLE_DIAMETER`

Default inclusion (bubble) diameter `d` \[m\] used by
[`TwoFluid1d::bubbly`].

`1e-3` m. Every interfacial closure scales as `1/d²`, so this is the single
most influential number a caller supplies and the least justified — there is
no interfacial-area transport here, so it never responds to breakup,
coalescence or the flow. Stated as a **model parameter with no measured
provenance**.

```rust
pub const DEFAULT_BUBBLE_DIAMETER: f64 = 1.0e-3;
```

#### Constant `DEFAULT_INITIAL_VOID_FRACTION`

Default initial vapour volume fraction `α_g^0` \[-\] when the initial state
flashes subcooled.

`1e-4`. A six-equation model cannot start from `α_g` exactly zero: with no
vapour there is no interfacial area, so no interfacial heat transfer, so no
evaporation, and a depressurising cell superheats its liquid without limit
until [`super::properties::MAX_METASTABLE_LIQUID_SUPERHEAT`] refuses it.
Real system codes resolve this with a wall-nucleation source; there is none
here, so a small pre-existing void stands in for one.

This is a **model parameter with no measured provenance**, exactly like
[`super::drift_flux::DEFAULT_VAPOUR_RELAXATION_TIME`], and it is
consequential: it sets how fast the liquid can shed superheat at the start
of a transient. Any case whose answer moves with it must report that
sensitivity. The mass it adds is negligible — at 7 MPa,
`α_g ρ_g / ρ_m ≈ 1e-4 × 36.5 / 833 ≈ 4.4e-6` of the inventory.

```rust
pub const DEFAULT_INITIAL_VOID_FRACTION: f64 = 1.0e-4;
```

#### Constant `COMPRESSIBILITY_STEP`

Relative finite-difference step used for the phase compressibilities
`ψ_k = ∂ρ_k/∂p|_{h_k}` \[-\].

```rust
pub const COMPRESSIBILITY_STEP: f64 = 1.0e-4;
```

#### Constant `MAX_MASS_TRANSFER_FRACTION_PER_STEP`

The largest fraction of the donor phase's mass concentration that
interfacial mass transfer may move within one step \[-\].

`0.5`. A **numerical device**, and it exists for a specific, measured
reason rather than as generic caution.

With the interface at `T_sat`, the transferring mass carries the
**saturation** enthalpy of its side — `h_g^sat` leaving or joining the
vapour, `h_f^sat` leaving or joining the liquid — which is what makes the
interfacial energy balance `Q_g + Q_l + Γ h_fg = 0` cancel exactly. Solve
the phase energy update for the enthalpy that leaves behind when a fraction
`f` of the phase transfers away:

`h_k^{n+1} = h_k^sat + (h_k^* − h_k^sat)/(1 − f) + Δt Q_k / (m_k^* (1 − f))`

The departure from saturation is amplified by `1/(1 − f)`. That is real
physics — mass leaving at the saturation enthalpy makes the remainder
*further* from saturation — but as `f → 1` it is unbounded, and it was
measured doing exactly that: a 57 K subcooled Edwards-like cell holding
`α_g = 1e-3` of saturated steam at 7 MPa condenses its entire vapour
inventory within one 30 µs step (`f = 1 − 1e-3`), and the amplification
produced `h_g = −1.7079e7 J/kg` on the very first step (measured
2026-08-12, release). The property layer then refused it, correctly, as a
vapour 30 K past its metastable bound — a correct refusal, of a number that
should never have been formed.

Capping `f` at `0.5` bounds the amplification at `2` per step. A phase that
genuinely wants to vanish still does, geometrically, over the following
steps, and once it reaches
[`PHASE_FLOOR_TRIGGER`] × its residual-alpha floor it is reset to the
saturated state outright. Every activation is counted into
[`TwoFluidReport::mass_transfer_limited_cells`].

```rust
pub const MAX_MASS_TRANSFER_FRACTION_PER_STEP: f64 = 0.5;
```

#### Constant `PHASE_FLOOR_TRIGGER`

Multiple of the residual-alpha mass floor `α_res ρ_k` at or below which a
phase is treated as **absent** and reset to its saturated state \[-\].

`2.0`. Strictly greater than 1 on purpose: the mass-transfer limiter drives
a vanishing phase *to* its floor, and floating-point rounding in the scale
factor leaves the result a few ULPs either side of it. A bare `m ≤ floor`
test therefore fires only about half the time, and the half that misses is
exactly where the `1/(1 − f)` amplification above is largest. Measured
consequence of getting this wrong: the `−1.7079e7 J/kg` vapour enthalpy
recorded at [`MAX_MASS_TRANSFER_FRACTION_PER_STEP`].

At `α_res = 1e-6` the reset happens at `α_k = 2e-6`, where the phase holds
about `7e-5 kg/m³` of steam at 7 MPa — eight decades below the mixture
density, so the mass and energy the reset invents are negligible but not
zero. Every activation is counted into
[`TwoFluidReport::residual_alpha_floor_events`].

```rust
pub const PHASE_FLOOR_TRIGGER: f64 = 2.0;
```

#### Constant `DEFAULT_OUTER_CORRECTORS`

Default number of outer correctors per step.

`8`, matching [`super::drift_flux::DEFAULT_OUTER_CORRECTORS`]. Each
corrector re-solves the pressure equation against the volume residual left
by the previous one; see the module docs.

```rust
pub const DEFAULT_OUTER_CORRECTORS: usize = 8;
```

#### Constant `DEFAULT_PRESSURE_UNDER_RELAXATION`

Default pressure under-relaxation `α_p` \[-\], applied once per outer
corrector as `p ← p_prev + α_p (p_solved − p_prev)`.

`0.7`, matching [`super::drift_flux::DEFAULT_PRESSURE_UNDER_RELAXATION`].

```rust
pub const DEFAULT_PRESSURE_UNDER_RELAXATION: f64 = 0.7;
```

#### Constant `DEFAULT_OUTER_TOLERANCE`

Default outer-corrector convergence tolerance on `max |Δp|` \[Pa\].

`1.0` Pa — six orders below the 7 MPa initial pressure of a blowdown.

```rust
pub const DEFAULT_OUTER_TOLERANCE: f64 = 1.0;
```

#### Constant `DEFAULT_VOLUME_RESIDUAL_TOLERANCE`

Default outer-corrector convergence tolerance on the volume residual
`max |α_g + α_l − 1|` \[-\].

`1e-9`. Tight, because the residual is a *constraint* the pressure equation
exists to enforce rather than an approximation being tolerated.

```rust
pub const DEFAULT_VOLUME_RESIDUAL_TOLERANCE: f64 = 1.0e-9;
```

#### Constant `MAX_VOLUME_RESIDUAL`

The volume-constraint violation beyond which [`TwoFluid1d::step`] **refuses**
the step \[-\].

`1e-4`. Past this the phases no longer fill the cell to any useful accuracy,
which means the outer correctors did not converge, which means the
pressure-velocity coupling has failed. Continuing from it would produce a
plausible-looking density field that is wrong by the residual, so it is
reported as [`TampinesError::Numerical`] instead.

```rust
pub const MAX_VOLUME_RESIDUAL: f64 = 1.0e-4;
```

#### Constant `MAX_REGION_4_NUDGE`

The largest relative pressure nudge
[`region_4_safe_pressure`] will apply \[-\].

`4e-12` — eight decades below
[`super::properties::TRANSPORT_CACHE_TOLERANCE`] and four below
[`super::properties::SATURATION_CACHE_TOLERANCE`], so nothing that reads a
property set can tell the difference. At 7 MPa the whole budget is 28 µPa.

```rust
pub const MAX_REGION_4_NUDGE: f64 = 4.0e-12;
```

### Functions

#### Function `thomas_solve`

Solve a tridiagonal linear system `A x = b` by the Thomas algorithm.

# What this is for

The pressure equation of a 1-D finite-volume solve couples each cell only
to its two axial neighbours, so its matrix is tridiagonal and the Thomas
algorithm — Gaussian elimination specialised to that structure — solves it
**exactly** in `O(n)` with no iteration and no convergence tolerance. On a
1-D mesh this is strictly better than the general Krylov solvers in
[`outram_foam_basic_lib::ldu_matrix`]: same answer, one pass, no residual
to check.

# Arguments

All slices have length `n` and carry raw `f64` in whatever consistent units
the caller's equation uses (for the pressure equation: `a`, `b`, `c` in
`m·s`, `d` in `kg/s`, giving `x` in `Pa`).

- `a` — sub-diagonal, `a[0]` is unused and may be any value.
- `b` — main diagonal.
- `c` — super-diagonal, `c[n-1]` is unused.
- `d` — right-hand side.

# Returns

The solution `x`, length `n`.

# Errors

[`crate::TampinesError::Numerical`] if the system is not diagonally
solvable — a zero (to within `1e-300`) pivot arises. For a pressure
equation assembled from positive compressibilities and positive face
coefficients this cannot happen, so a failure here means the assembly is
wrong, not that the solver is fussy.

# Example

```
use tampines::multiphase_1d::thomas_solve;

// The 1-D Laplacian with Dirichlet ends: -x[i-1] + 2 x[i] - x[i+1] = 1.
let n = 5;
let a = vec![-1.0; n];
let b = vec![2.0; n];
let c = vec![-1.0; n];
let d = vec![1.0; n];
let x = thomas_solve(&a, &b, &c, &d).unwrap();

// The exact solution is the discrete parabola x[i] = (i+1)(n-i)/2.
for (i, xi) in x.iter().enumerate() {
    let exact = (i as f64 + 1.0) * (n as f64 - i as f64) / 2.0;
    assert!((xi - exact).abs() < 1e-12, "cell {i}: {xi} vs {exact}");
}
```

```rust
pub fn thomas_solve(a: &[f64], b: &[f64], c: &[f64], d: &[f64]) -> Result<Vec<f64>, crate::TampinesError> { /* ... */ }
```

### Re-exports

#### Re-export `DriftFlux1d`

```rust
pub use drift_flux::DriftFlux1d;
```

#### Re-export `DriftFluxReport`

```rust
pub use drift_flux::DriftFluxReport;
```

#### Re-export `Pipe1d`

```rust
pub use geometry::Pipe1d;
```

#### Re-export `DispersedPhase`

```rust
pub use interfacial::DispersedPhase;
```

#### Re-export `InterfacialCellState`

```rust
pub use interfacial::InterfacialCellState;
```

#### Re-export `InterfacialExchange`

```rust
pub use interfacial::InterfacialExchange;
```

#### Re-export `InterfacialSources`

```rust
pub use interfacial::InterfacialSources;
```

#### Re-export `SaturatedProperties`

```rust
pub use properties::SaturatedProperties;
```

#### Re-export `TwoPhaseState`

```rust
pub use properties::TwoPhaseState;
```

#### Re-export `PhaseCouplingBlock`

```rust
pub use two_fluid::PhaseCouplingBlock;
```

#### Re-export `PhaseCouplingInputs`

```rust
pub use two_fluid::PhaseCouplingInputs;
```

#### Re-export `TwoFluid1d`

```rust
pub use two_fluid::TwoFluid1d;
```

#### Re-export `TwoFluidBoundary`

```rust
pub use two_fluid::TwoFluidBoundary;
```

#### Re-export `TwoFluidReport`

```rust
pub use two_fluid::TwoFluidReport;
```

## Module `pebble_bed`

# Pebble-bed thermal physics for high-temperature gas-cooled reactors

The nested conduction scales of a pebble-bed core (a *doubly
heterogeneous* medium — TRISO particles inside pebbles inside a packed
bed), being built as one coherent stack under the `op-jyyp` HTR-10 epic.

## The three nested conduction scales

Each level's effective property is the next level's input, and each level's
temperature is the one below it's boundary condition:

- [`triso`] — **level 1**, coated-particle conduction through the five
  concentric regions (UO2 kernel, porous carbon buffer, IPyC, SiC, OPyC):
  analytic series resistance for concentric shells with volumetric heat
  generation confined to the kernel, temperature- and fluence-dependent
  layer conductivities from the VTB HTR-PM pebble model, and an effective
  particle conductivity for level 2. Geometry is reused from `boon-lay`'s
  `TrisoCell` (maintainer-approved dependency edge); `boon-lay`'s
  fission-product *release* model is deliberately not consumed.
- [`pebble`] — **level 2**, two-zone pebble radial conduction: a fuelled
  zone whose effective conductivity comes from level 1 through a
  Maxwell-Eucken or Chiew-Glandt dispersion model, inside an unfuelled
  graphite shell. The double heterogeneity is kept explicit rather than
  homogenised; see the module docs for what homogenising would cost.
- [`cht`] — **level 3**, bed-to-helium conjugate heat transfer: the
  **correct** Wakao-Funazkri particle-to-fluid Nusselt correlation
  `Nu = 2 + 1.1 Pr^(1/3) Re^0.6` on the pebble diameter, the heat transfer
  coefficient it implies, and the bed volumetric form via
  `a_v = 6(1 - eps)/d`. **Warning, still current:** the TUAS `WakaoData`
  implementation has the Re and Pr exponents *swapped* relative to the
  published correlation (bead `op-4542`), diverging by a factor of about
  5.8 at Re = 1000, Pr = 0.71 — `cht` therefore implements the correlation
  independently and must **not** be cross-wired to TUAS until that bead is
  resolved.

## Also present

- [`zbs`] — Zehner-Bauer-Schlunder packed-bed effective thermal
  conductivity: stagnant-gas, solid, particle-contact and thermal
  radiation contributions, with a near-wall porosity hook. The
  formulation follows the dimensionless form in the van Antwerpen et al.
  (2010) review; the transcription has **not** been human-verified
  against the printed originals (tracked as `op-qoy4`), though its
  analytic limits are test-gated with measured tolerances. See the
  module docs for the measured finding (2026-08-11) that the VTB
  generic-pbr 18-point reference table is *not* reproduced by ZBS with
  helium in the pores — the model sits below the table at all 18 points
  (ratio 0.177 at 300 K to 0.644 at 2000 K), tracked as `op-jvua`.
- [`feedback`] — the graphite/moderator reactivity channel, held as its
  **own** state with its own coefficient and its own lumped thermal-mass
  ODE rather than folded into fuel Doppler, because the large graphite mass
  is what gives HTR-10 its long thermal time constant and self-limiting
  response. **No moderator temperature coefficient is supplied** — it must
  come from the caller's neutronics; the published HTR-10 *isothermal*
  coefficients are provided separately and clearly labelled as not being
  that quantity. Wiring the channel into point kinetics belongs in an
  example or in `nee_soon`, not in this library, which stays free of
  `teh-o-prke`.
- [`temperature_difference`] — the one shared helper, converting a pair of
  absolute temperatures into the `uom` `TemperatureInterval` that `uom`
  deliberately will not produce with `-`.

## Related but housed elsewhere

- KTA packed-bed pressure drop (the friction side of the bed) landed in
  [`crate::gas_phase`] as `KtaBed` (2026-08-11), alongside the helium
  circuit components it serves. Whether a `pebble_bed::kta` home is also
  wanted is the maintainer's call — tracked as `op-afz4`.

## Status

**NOT VALIDATED.** Every correlation carries its citation and access
tier; nothing here has been compared against HTR-10 measurements.
AI-assisted draft pending human review per `RESPONSIBLE_USE.md`.

```rust
pub mod pebble_bed { /* ... */ }
```

### Modules

## Module `cht`

# Bed-to-helium conjugate heat transfer — the outermost of the nested scales

Particle-to-fluid convective coupling in a packed bed of spheres: the
Nusselt number, the heat transfer coefficient it implies, the bed's
specific surface area, and the volumetric coefficient that a porous-medium
energy equation actually needs.

## Where this sits in the nest

Level 3 of three, and the boundary condition for the other two. The
coefficient computed here sets the pebble **surface** temperature that
[`super::pebble`] takes as its outer boundary condition, which in turn sets
the TRISO particle surface temperature in [`super::triso`]. Together with
[`super::zbs`] (effective conduction through the bed) it closes the solid
side of a pebble-bed thermal model.

## The correlation

**Wakao-Funazkri**, for particle-to-fluid heat transfer in a packed bed:

```text
Nu = 2 + 1.1 * Pr^(1/3) * Re^0.6
```

with `Nu = h d / k_f` and `Re = rho u d / mu` both formed on the **particle
(pebble) diameter** `d`, and `u` the **superficial** velocity — the volume
flow divided by the *empty* bed cross-section, not the interstitial
velocity. Getting that wrong changes `Re` by a factor of `1/eps` (about 2.6
for the HTR-10 bed), so [`PackedBedConvection::reynolds_number`] is provided
to make the convention explicit rather than assumed.

The additive 2 is the exact conduction limit for an isolated sphere in a
stagnant infinite medium, so the correlation degenerates correctly as the
flow stops — the limit that matters most for a gas-cooled reactor, because
it is the loss-of-forced-cooling case.

**Validity:** `Re` from about 15 to 8500, the range over which Wakao and
co-workers regressed the correlation against packed-bed data. Outside it
the expression still evaluates (and still tends to 2 as `Re -> 0`), but the
answer is an extrapolation; [`PackedBedConvection::is_within_validity_range`]
reports which side of that line a given `Re` falls on rather than silently
deciding for the caller.

**Citation:** Wakao, N. and Funazkri, T. (1978), *Effect of fluid
dispersion coefficients on particle-to-fluid mass transfer coefficients in
packed beds*, Chemical Engineering Science 33(10), 1375-1384.

*Attribution note, for honesty:* that 1978 paper establishes the
mass-transfer (Sherwood) form of this correlation; the identically-shaped
heat-transfer (Nusselt) version is commonly attributed to the companion
paper, Wakao, Kaguei and Funazkri (1979), Chem. Eng. Sci. 34, 325-336.
Neither was consulted at page level in this session — the equation form,
its coefficients and its stated `Re` range are transcribed from the
secondary literature, and a human should confirm them against the primary
source before this module is promoted past Prototype in the V&V pipeline.

## Why this is implemented here and not consumed from TUAS

[`tuas_boussinesq_solver`] carries a `WakaoData` Nusselt correlation, and
it would be the natural thing to reuse. **It must not be used**: its
implementation computes

```text
Nu = 2 + 1.1 * Re^0.333 * Pr^0.6        (TUAS -- exponents transposed)
```

— the Reynolds and Prandtl exponents are swapped relative to the published
correlation. This is not a rounding difference. At `Re = 1000`, `Pr = 0.71`
(representative of helium in an HTR-10 bed) the two forms differ by a
factor of about 5.8; see
`tests::divergence_from_the_tuas_wakao_implementation` for the measured
numbers. The defect is tracked in this workspace's issue tracker as
**`op-4542`**; TUAS is not modified from here, so this module implements
the correct form independently. **Do not "unify" the two until `op-4542`
is closed** — unification in the wrong direction would silently import the
defect.

## Status

**NOT VALIDATED.** Verified against analytic limits and hand evaluations
only; no comparison against any packed-bed heat-transfer measurement, and
none against HTR-10. AI-assisted draft pending human review per
`RESPONSIBLE_USE.md`.

**Belongs here:** particle-to-fluid convective closure and bed surface-area
geometry. **Does not belong here:** helium property data (that is
[`outram_park_fork_coolprop`]'s), bed pressure drop, effective conduction
through the bed ([`super::zbs`]), or anything inside a pebble.

```rust
pub mod cht { /* ... */ }
```

### Types

#### Type Alias `SpecificSurfaceArea`

Specific surface area of a packed bed — heat transfer area per unit **bed**
volume, 1/m.

An alias for `uom`'s [`LinearNumberDensity`], whose dimension (1/length) is
exactly right but whose name says nothing useful in this context. Produced
by [`PackedBedConvection::specific_surface_area`].

```rust
pub type SpecificSurfaceArea = uom::si::f64::LinearNumberDensity;
```

#### Type Alias `VolumetricHeatTransferCoefficient`

Volumetric heat transfer coefficient, W/(m^3 K) — the product `h * a_v` of
a surface coefficient and a specific surface area.

`uom` has no named quantity of this dimension (M L^-1 T^-3 Th^-1), so the
alias is spelled out here rather than leaking a raw
`Quantity<ISQ<...>, SI<f64>, f64>` into a public signature, per the
workspace's human-interface rule. This is the coefficient a porous-medium
two-temperature energy equation multiplies by `(T_solid - T_fluid)` to get
a volumetric power density.

```rust
pub type VolumetricHeatTransferCoefficient = uom::si::Quantity<uom::si::ISQ<uom::typenum::N1, uom::typenum::P1, uom::typenum::N3, uom::typenum::Z0, uom::typenum::N1, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

#### Struct `PackedBedConvection`

A packed bed of monosized spheres, described by the two parameters the
particle-to-fluid convective closure needs.

Plain data; the physics lives in
[`PackedBedConvection::nusselt_number`] and its derived coefficients.
Construct with [`PackedBedConvection::new`] (checked) or
[`PackedBedConvection::htr10`].

```rust
pub struct PackedBedConvection {
    pub pebble_diameter: uom::si::f64::Length,
    pub porosity: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pebble_diameter` | `uom::si::f64::Length` | Pebble (sphere) diameter, metres — the length scale of both the Nusselt<br>and the Reynolds number, and the scale that sets the bed's surface area<br>per unit volume. HTR-10: 0.06 m. |
| `porosity` | `uom::si::f64::Ratio` | Bed porosity (void fraction), dimensionless, strictly between 0 and 1.<br>Enters only through the specific surface area `a_v = 6(1 - eps)/d`, not<br>through the Nusselt number itself. HTR-10: 0.39 (filling fraction 0.61,<br>IAEA-TECDOC-1382 part 2, Chapter 4, Open tier). |

##### Implementations

###### Methods

- ```rust
  pub fn new(pebble_diameter: Length, porosity: Ratio) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Builds a packed-bed convective closure from the pebble diameter

- ```rust
  pub fn htr10() -> Self { /* ... */ }
  ```
  The HTR-10 pebble bed: 6.0 cm pebbles at a porosity of 0.39

- ```rust
  pub fn reynolds_number(self: &Self, superficial_velocity: Velocity, density: MassDensity, dynamic_viscosity: DynamicViscosity) -> Ratio { /* ... */ }
  ```
  Particle Reynolds number, `Re = rho u d / mu`, dimensionless.

- ```rust
  pub fn is_within_validity_range(self: &Self, reynolds: Ratio) -> bool { /* ... */ }
  ```
  Whether the given particle Reynolds number lies inside the range

- ```rust
  pub fn nusselt_number(self: &Self, reynolds: Ratio, prandtl: Ratio) -> Result<Ratio, TampinesError> { /* ... */ }
  ```
  Particle-to-fluid Nusselt number of the bed, dimensionless:

- ```rust
  pub fn heat_transfer_coefficient(self: &Self, reynolds: Ratio, prandtl: Ratio, fluid_conductivity: ThermalConductivity) -> Result<HeatTransfer, TampinesError> { /* ... */ }
  ```
  Particle-to-fluid heat transfer coefficient, W/(m^2 K):

- ```rust
  pub fn specific_surface_area(self: &Self) -> SpecificSurfaceArea { /* ... */ }
  ```
  Specific surface area of the bed, `a_v = 6 (1 - eps) / d`, in 1/m —

- ```rust
  pub fn volumetric_heat_transfer_coefficient(self: &Self, reynolds: Ratio, prandtl: Ratio, fluid_conductivity: ThermalConductivity) -> Result<VolumetricHeatTransferCoefficient, TampinesError> { /* ... */ }
  ```
  Volumetric heat transfer coefficient of the bed, W/(m^3 K):

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PackedBedConvection { /* ... */ }
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
    fn eq(self: &Self, other: &PackedBedConvection) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Constants and Statics

#### Constant `WAKAO_MIN_REYNOLDS`

Lowest particle Reynolds number of the Wakao correlation's regressed range,
15 (dimensionless).

```rust
pub const WAKAO_MIN_REYNOLDS: f64 = 15.0;
```

#### Constant `WAKAO_MAX_REYNOLDS`

Highest particle Reynolds number of the Wakao correlation's regressed
range, 8500 (dimensionless).

```rust
pub const WAKAO_MAX_REYNOLDS: f64 = 8500.0;
```

## Module `feedback`

# Graphite / moderator reactivity feedback — its own channel

The graphite moderator temperature of a pebble-bed HTGR, carried as an
independent state with its own thermal inertia and its own reactivity
coefficient.

## Why this is a separate channel and must stay one

It is tempting to lump moderator feedback into the fuel Doppler channel —
one temperature, one coefficient, one term. For an HTR-10 that destroys the
physics that defines the reactor.

The fuel and the graphite respond on completely different timescales. A UO2
kernel is micrometres across and follows a power change essentially
instantly; the graphite is the *bulk of the core mass* — pebble matrix,
pebble shells, dummy balls and reflector — and takes minutes to change
temperature. HTR-10's self-limiting response to a loss of flow is exactly
that separation: prompt negative Doppler arrests the excursion, then the
slow, large-mass graphite channel governs where the core settles and how
long it takes to get there. Collapse the two into one temperature and one
coefficient, and the model loses the long time constant altogether — it
will reach the right final state on entirely the wrong timescale, which is
the timescale a passive-safety argument is made on.

This is recorded as bead **`op-jyyp.6`** and in
`docs/reactor-scoping/htr10-neutronics.md` section 4.4: the graphite
channel "must come down rung 3 as its own reactivity coefficient, not be
folded into Doppler."

## What this module does and does not do

It provides [`GraphiteModeratorFeedback`]: a lumped graphite node with a
temperature, a thermal mass, and a linear reactivity coefficient. It can

- report the reactivity its current temperature implies
  ([`GraphiteModeratorFeedback::reactivity`]),
- advance that temperature under a heat balance
  ([`GraphiteModeratorFeedback::step`]), and
- report the thermal time constant that balance implies
  ([`GraphiteModeratorFeedback::thermal_time_constant`]).

It does **not** contain point kinetics. This crate's library is deliberately
free of [`teh_o_prke`](https://docs.rs/teh-o-prke) — that crate is only an
Android-gated *dev*-dependency of `tampines`, used by examples — so the
reactivity produced here is a plain dimensionless number for a caller to
feed into a kinetics solver. **Wiring this channel into PRKE is deliberate
future work** belonging in an example or in `nee_soon`, where the
neutronics dependency is appropriate; it is the remaining part of
`op-jyyp.6` and is tracked separately. Nothing here should grow a
dependency on a neutronics crate.

## The coefficient is the caller's, not this module's

**No moderator temperature coefficient is invented here, and none is
supplied as a default.** A moderator-only coefficient is an output of a
neutronics calculation for a specific core state — loading, burnup, rod
position — and inventing one would produce a plausible-looking transient
that means nothing.

The IAEA benchmark document *does* publish HTR-10 **isothermal** temperature
coefficients, and they are provided here as clearly-labelled constants (see
[`htr10_isothermal_coefficient_nrg_20_to_120c`] and its siblings). Read
their documentation before using them: an isothermal coefficient moves fuel
and moderator *together*, so it is the sum of the Doppler and moderator
channels, not the moderator channel alone. Substituting one for `alpha_m`
double-counts the fuel and defeats the entire purpose of this module. No
constructor in this file does that substitution for you.

## Status

**NOT VALIDATED.** The ODE and the reactivity relation are verified against
analytic limits; no HTR-10 transient has been reproduced, and no coefficient
here has been checked against a neutronics calculation. AI-assisted draft
pending human review per `RESPONSIBLE_USE.md`.

**Belongs here:** the moderator temperature state, its thermal balance, and
its reactivity mapping. **Does not belong here:** point kinetics, fuel
Doppler, decay heat, or the neutronics that produces the coefficient.

```rust
pub mod feedback { /* ... */ }
```

### Types

#### Type Alias `ReactivityTemperatureCoefficient`

A reactivity temperature coefficient: reactivity (dimensionless `dk/k`) per
kelvin of temperature change, 1/K.

An alias for `uom`'s [`TemperatureCoefficient`], named for what it means
here. Negative for a core with negative temperature feedback, which is the
physically desirable and the HTR-10 case. To read one in the reactor-physics
unit of **pcm per kelvin**, multiply the per-kelvin value by 1e5:
`-7.37e-5 /K` is `-7.37 pcm/K`.

```rust
pub type ReactivityTemperatureCoefficient = uom::si::f64::TemperatureCoefficient;
```

#### Struct `GraphiteModeratorFeedback`

A lumped graphite moderator node: one temperature, one thermal mass, one
linear reactivity coefficient.

Plain data with methods — no trait objects, no interior mutability, no
lifetimes, per the workspace Rust design rules. The temperature is the only
mutable state; [`GraphiteModeratorFeedback::step`] advances it and
everything else is a pure function of it.

Construct with [`GraphiteModeratorFeedback::new`], which requires the
coefficient explicitly — see the module documentation for why no default is
offered.

```rust
pub struct GraphiteModeratorFeedback {
    pub moderator_temperature: uom::si::f64::ThermodynamicTemperature,
    pub reference_temperature: uom::si::f64::ThermodynamicTemperature,
    pub temperature_coefficient: ReactivityTemperatureCoefficient,
    pub graphite_mass: uom::si::f64::Mass,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `moderator_temperature` | `uom::si::f64::ThermodynamicTemperature` | Current bulk temperature of the graphite moderator, kelvin. This is the<br>state variable: a single lumped temperature standing for the whole<br>graphite mass, which is an approximation whose validity rests on the<br>graphite being far more conductive and far more massive than the<br>gradients within it are steep. |
| `reference_temperature` | `uom::si::f64::ThermodynamicTemperature` | Reference temperature at which this channel contributes exactly zero<br>reactivity, kelvin. Usually the temperature at which the core's `k_eff`<br>was evaluated — a critical steady state, say — so that the channel<br>expresses a *departure* from that state. |
| `temperature_coefficient` | `ReactivityTemperatureCoefficient` | Moderator temperature coefficient of reactivity, `dk/k` per kelvin.<br>**Caller-supplied; no default exists.** See the module documentation. |
| `graphite_mass` | `uom::si::f64::Mass` | Mass of graphite this node represents, kilograms. Sets the thermal<br>inertia, and therefore the time constant that is the whole point of<br>keeping this channel separate from fuel Doppler. |

##### Implementations

###### Methods

- ```rust
  pub fn new(moderator_temperature: ThermodynamicTemperature, reference_temperature: ThermodynamicTemperature, temperature_coefficient: ReactivityTemperatureCoefficient, graphite_mass: Mass) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Builds a graphite moderator feedback channel.

- ```rust
  pub fn temperature_excursion(self: &Self) -> TemperatureInterval { /* ... */ }
  ```
  Temperature of the graphite above its reference, kelvin — positive when

- ```rust
  pub fn reactivity(self: &Self) -> Ratio { /* ... */ }
  ```
  Reactivity contributed by this channel, dimensionless `dk/k`:

- ```rust
  pub fn reactivity_pcm(self: &Self) -> f64 { /* ... */ }
  ```
  Reactivity contributed by this channel in **pcm** (per cent mille,

- ```rust
  pub fn specific_heat_capacity(self: &Self) -> Result<SpecificHeatCapacity, TampinesError> { /* ... */ }
  ```
  Specific heat capacity of the graphite at its current temperature,

- ```rust
  pub fn thermal_capacity(self: &Self) -> Result<HeatCapacity, TampinesError> { /* ... */ }
  ```
  Total heat capacity of the graphite node, `m cp`, J/K — the thermal

- ```rust
  pub fn step(self: &mut Self, heat_in: Power, heat_out: Power, timestep: Time) -> Result<ThermodynamicTemperature, TampinesError> { /* ... */ }
  ```
  Advances the moderator temperature by one explicit timestep under a

- ```rust
  pub fn thermal_time_constant(self: &Self, conductance: ThermalConductance) -> Result<Time, TampinesError> { /* ... */ }
  ```
  Thermal time constant of the graphite node against a given heat-removal

- ```rust
  pub fn thermal_time_constant_from_coefficient(self: &Self, heat_transfer_coefficient: HeatTransfer, area: Area) -> Result<Time, TampinesError> { /* ... */ }
  ```
  Thermal time constant against a heat transfer coefficient acting over

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> GraphiteModeratorFeedback { /* ... */ }
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
    fn eq(self: &Self, other: &GraphiteModeratorFeedback) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `htr10_isothermal_coefficient_nrg_20_to_120c`

HTR-10 **isothermal** temperature coefficient of reactivity over
20-120 degrees Celsius as calculated by **NRG**: -7.37e-5 per degree
(equivalently -7.37 pcm per kelvin).

Source: IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-33 (Open tier;
catalogued at
`crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.pdf`). The
document tabulates `delta-k/k` per degree Celsius; a coefficient *per
degree Celsius* and *per kelvin* are numerically identical, since only the
size of the degree matters.

# This is NOT a moderator-only coefficient

An **isothermal** coefficient is measured or calculated by changing the
temperature of the *entire core at once* — fuel, moderator and reflector
together. It is therefore the **sum** of the fuel Doppler channel and the
moderator channel (and everything else that moves with temperature).

Using it as [`GraphiteModeratorFeedback::temperature_coefficient`] would
count the fuel's contribution twice — once in whatever Doppler channel the
caller already has, and again here — and would give the graphite the fuel's
prompt feedback on the graphite's slow timescale. That is precisely the
error this module exists to prevent.

A genuine moderator-only coefficient must come from a neutronics
calculation that perturbs the moderator temperature alone. These constants
are provided for **validation of a whole-core isothermal calculation**
(compare your model's total isothermal coefficient against this number),
not as a stand-in for `alpha_m`.

```rust
pub fn htr10_isothermal_coefficient_nrg_20_to_120c() -> ReactivityTemperatureCoefficient { /* ... */ }
```

#### Function `htr10_isothermal_coefficient_nrg_200_to_250c`

HTR-10 **isothermal** temperature coefficient over 200-250 degrees Celsius
as calculated by **NRG**: -8.05e-5 per degree.

Same source, same caveat, as
[`htr10_isothermal_coefficient_nrg_20_to_120c`] — read that function's
documentation before using this value. Note the coefficient becomes more
negative with temperature, which is the expected and desirable trend.

```rust
pub fn htr10_isothermal_coefficient_nrg_200_to_250c() -> ReactivityTemperatureCoefficient { /* ... */ }
```

#### Function `htr10_isothermal_coefficient_inet_20_to_120c`

HTR-10 **isothermal** temperature coefficient over 20-120 degrees Celsius
as calculated by **INET with VSOP**: -7.49e-5 per degree.

Same source, same caveat, as
[`htr10_isothermal_coefficient_nrg_20_to_120c`]. INET's figure sits 1.6%
below NRG's over the same interval — a useful sense of the spread between
independent calculations of the same quantity, and of how much precision it
is reasonable to claim.

```rust
pub fn htr10_isothermal_coefficient_inet_20_to_120c() -> ReactivityTemperatureCoefficient { /* ... */ }
```

#### Function `htr10_isothermal_coefficient_inet_120_to_250c`

HTR-10 **isothermal** temperature coefficient over 120-250 degrees Celsius
as calculated by **INET with VSOP**: -9.15e-5 per degree.

Same source, same caveat, as
[`htr10_isothermal_coefficient_nrg_20_to_120c`]. Note this covers a
different interval from the NRG 200-250 C figure, so the two are not
directly comparable.

```rust
pub fn htr10_isothermal_coefficient_inet_120_to_250c() -> ReactivityTemperatureCoefficient { /* ... */ }
```

## Module `pebble`

# Pebble radial conduction — the middle of the nested scales

Steady-state radial heat conduction through a spherical fuel element: an
inner **fuelled zone**, in which TRISO coated particles are dispersed in
matrix graphite and in which all the fission power is released, surrounded
by an **unfuelled graphite shell** that produces no heat and only conducts.

For the HTR-10 element that is a 6.0 cm sphere with a 5.0 cm-diameter
fuelled zone — a 0.5 cm graphite shell — with matrix graphite of density
1.73 g/cm^3 (IAEA-TECDOC-1382 part 2, Chapter 4, Tables 4-2 and 4-17, Open
tier).

## Where this sits in the nest

Level 2 of three. Its fuelled-zone conductivity is built from level 1
([`super::triso`]) via a dispersion model; its pebble-surface temperature
is the boundary condition that level 3 ([`super::cht`], bed-to-helium
convection) and the bed effective conductivity ([`super::zbs`]) supply.

## The fuelled zone is NOT treated as homogeneous graphite

This is the deliberate physical choice of this module, and it is worth
stating plainly because the opposite choice is the common shortcut.

A pebble-bed core is *doubly heterogeneous*: fuel kernels inside particles
inside pebbles inside a bed. Smearing the TRISO particles into the matrix —
treating the fuelled zone as one homogeneous graphite-and-fuel medium with
volume-averaged properties — discards the kernel-to-matrix temperature
difference entirely. The neutronic cost of the analogous smearing is
quantified: Wang, Sheu, Peir and Liang (2014), *Criticality calculations of
the HTR-10 pebble-bed reactor with SCALE6/CSAS6 and MCNP5*, Ann. Nucl.
Energy 64, 1-7 (proprietary tier, catalogued at
`crates/kovan-literature/proprietary/papers/wang2014htr10criticality.json`
— cited, not reproduced) measure roughly **+2800 pcm** in `k_eff` for a
fully homogenised (INFHOMMEDIUM) unit cell against a continuous-energy
reference, falling to about +280 pcm once the double heterogeneity is
treated explicitly.

The thermal analogue is real for the same structural reason: the fuel is
not where the average says it is. This module therefore keeps the two
scales separate — the fuelled zone gets an *effective* conductivity from a
dispersion model ([`DispersionModel`]), and the kernel temperature is
recovered by superposing level 1's particle solution on the local matrix
temperature ([`Pebble::steady_state_temperatures`]). **No claim is made
that the thermal error of homogenisation is 2800 pcm-equivalent** — that
number is a neutronic result and is quoted only as evidence that the
heterogeneity matters, not as a thermal bound.

## Provenance

- Geometry and matrix density: IAEA-TECDOC-1382 part 2, Chapter 4
  (Open tier), Table 4-17.
- Matrix and shell graphite conductivity: consumed from
  [`tuas_boussinesq_solver`]'s `NuclearGraphiteMatrixA3` correlations
  rather than hardcoded here; those in turn transcribe the CC-BY-4.0
  Virtual Test Bed HTR-PM deck.
- Dispersion models: Maxwell-Eucken (Maxwell 1873) and Chiew & Glandt
  (1983) — see [`DispersionModel`] for the equations, their validity
  ranges, and an explicit transcription caveat.

## Status

**NOT VALIDATED.** Verified against analytic limits and bounds only; no
comparison against any HTR-10 measurement. AI-assisted draft pending human
review per `RESPONSIBLE_USE.md`.

**Belongs here:** pebble-scale geometry, the TRISO-in-matrix dispersion
rule, and the two-zone conduction solution. **Does not belong here:**
particle-internal conduction ([`super::triso`]), bed-scale effective
conductivity ([`super::zbs`]), or the convective boundary condition
([`super::cht`]).

```rust
pub mod pebble { /* ... */ }
```

### Types

#### Enum `DispersionModel`

How the conductivity of a dilute dispersion of spherical inclusions in a
continuous matrix is combined into one effective conductivity.

A closed set of two models — enum dispatch, no trait objects, per the
workspace Rust design rules. Both take the same three inputs (matrix
conductivity, inclusion conductivity, inclusion volume fraction) and both
reduce to the matrix conductivity as the volume fraction goes to zero.

Throughout, `kappa = k_particle / k_matrix` and
`beta = (kappa - 1) / (kappa + 2)`. `beta` lies in `(-0.5, 1)`: it is
positive when the inclusions conduct better than the matrix and negative
when they conduct worse, which is the TRISO-in-graphite case.

```rust
pub enum DispersionModel {
    MaxwellEucken,
    ChiewGlandt,
}
```

##### Variants

###### `MaxwellEucken`

**Maxwell-Eucken** (Maxwell's 1873 result for a dilute dispersion of
non-interacting spheres):

`k_eff / k_matrix = (1 + 2 beta phi) / (1 - beta phi)`

**Validity:** exact to first order in the inclusion volume fraction
`phi`, because it neglects sphere-sphere interactions entirely. It is
reliable for `phi` up to roughly 0.1 and is conventionally used up to
about 0.2; beyond that the neglected interactions matter. It always
lies within the Wiener (series/parallel) bounds, and in fact coincides
with one of the tighter Hashin-Shtrikman bounds.

###### `ChiewGlandt`

**Chiew & Glandt (1983)** — Maxwell extended to third order in the
inclusion volume fraction for a random dispersion of hard spheres:

`k_eff / k_matrix = [1 + 2 beta phi + (2 beta^3 - 0.1 beta) phi^2
                     + 0.05 phi^3 exp(4.5 beta)] / (1 - beta phi)`

Source: Chiew, Y. C. and Glandt, E. D., *The effect of structure on the
conductivity of a dispersion*, J. Colloid Interface Sci. 94(1) (1983)
90-104. This is the mixing rule the Virtual Test Bed / Pronghorn
pebble decks select for the fuel matrix (`k_mixing = 'chiew'`, e.g.
`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`
line 250; CC-BY-4.0, Open tier), which is why it is the default here.

**Validity:** derived for randomly dispersed, non-overlapping spheres;
the `phi^2` and `phi^3` terms extend usable accuracy to roughly
`phi = 0.6`. At the HTR-10 particle packing fraction of about 0.05 it
differs from Maxwell-Eucken by well under a percent — the two agree
wherever the dispersion is genuinely dilute, and the tests below
measure that agreement.

**Known artefact — measured, not assumed.** The third-order term
`0.05 phi^3 exp(4.5 beta)` does **not** vanish at `beta = 0`. When the
inclusions are made of the same material as the matrix (`kappa = 1`,
hence `beta = 0`), which is not a composite at all and must return the
matrix conductivity exactly, this expression instead returns
`k_matrix (1 + 0.05 phi^3)`. The same term lets the correlation stray
marginally outside the Wiener bounds near `beta = 0`: measured on
2026-08-11, the largest excursion over a 49-point sweep was
**1.0800e-2 relative, at `kappa` = 1 and `phi` = 0.6**, which is
exactly `0.05 phi^3` — so the artefact is bounded by that term and
nothing worse is hiding behind it. It is negligible in the dilute
regime this module uses: `0.05 phi^3` is 6.3e-6 at the HTR-10 `phi` of
0.0502. Use [`DispersionModel::MaxwellEucken`] if an exactly
bound-respecting rule is required.

**Transcription caveat** (honesty per `RESPONSIBLE_USE.md`): the
polynomial above was implemented without page-level access to Chiew &
Glandt (1983) in this session. The falsifiable checks in this module's
tests — Maxwell agreement at small `phi`, Wiener bounds, the `phi -> 0`
degeneracy — all pass, but a human should verify the coefficients
`2 beta^3 - 0.1 beta` and `0.05 exp(4.5 beta)` against the paper before
this module is promoted past Prototype in the V&V pipeline. The
`beta = 0` artefact above is exactly the kind of thing that check
should resolve: it may be faithful to the published fit, or it may be a
transcription error in the third-order coefficient.

##### Implementations

###### Methods

- ```rust
  pub fn effective_conductivity(self: &Self, matrix_conductivity: ThermalConductivity, particle_conductivity: ThermalConductivity, particle_volume_fraction: Ratio) -> Result<ThermalConductivity, TampinesError> { /* ... */ }
  ```
  Effective thermal conductivity, W/(m K), of `particle_volume_fraction`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DispersionModel { /* ... */ }
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
    fn eq(self: &Self, other: &DispersionModel) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Pebble`

A spherical fuel element: a fuelled inner zone of TRISO particles dispersed
in matrix graphite, inside an unfuelled graphite shell.

Plain data; the physics lives in [`Pebble::fuelled_zone_conductivity`] and
[`Pebble::steady_state_temperatures`]. Construct with [`Pebble::new`]
(checked) or [`Pebble::htr10`] (the cited HTR-10 fuel element).

```rust
pub struct Pebble {
    pub outer_radius: uom::si::f64::Length,
    pub fuelled_zone_radius: uom::si::f64::Length,
    pub particle: super::triso::TrisoParticle,
    pub particles_per_pebble: f64,
    pub dispersion_model: DispersionModel,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `outer_radius` | `uom::si::f64::Length` | Outer radius of the whole pebble, metres. HTR-10: 0.03 m (6.0 cm<br>diameter). |
| `fuelled_zone_radius` | `uom::si::f64::Length` | Outer radius of the fuelled zone, metres — the boundary between the<br>particle-bearing matrix and the unfuelled shell. HTR-10: 0.025 m<br>(5.0 cm diameter), leaving a 5 mm shell. |
| `particle` | `super::triso::TrisoParticle` | The coated particle dispersed in the fuelled zone (level 1). |
| `particles_per_pebble` | `f64` | Number of coated particles per pebble, dimensionless count stored as<br>`f64` because it enters volume-fraction and per-particle-power algebra<br>rather than being an index. HTR-10: 8335 (IAEA-TECDOC-1382 part 2,<br>Chapter 4; see [`coated_particles_per_pebble`], which reproduces that<br>figure from the published heavy-metal loading). |
| `dispersion_model` | `DispersionModel` | Which dispersion rule mixes the particle conductivity into the matrix. |

##### Implementations

###### Methods

- ```rust
  pub fn new(outer_radius: Length, fuelled_zone_radius: Length, particle: TrisoParticle, particles_per_pebble: f64, dispersion_model: DispersionModel) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Builds a pebble from its two radii, its particle, the particle count

- ```rust
  pub fn htr10() -> Self { /* ... */ }
  ```
  The HTR-10 fuel element, transcribed from **IAEA-TECDOC-1382 part 2,

- ```rust
  pub fn fuelled_zone_volume(self: &Self) -> Volume { /* ... */ }
  ```
  Volume of the fuelled zone, m^3. HTR-10: 65.45 cm^3.

- ```rust
  pub fn pebble_volume(self: &Self) -> Volume { /* ... */ }
  ```
  Volume of the whole pebble, m^3. HTR-10: 113.10 cm^3.

- ```rust
  pub fn triso_volume_fraction(self: &Self) -> Ratio { /* ... */ }
  ```
  Volume fraction of coated particles **within the fuelled zone**,

- ```rust
  pub fn matrix_conductivity(self: &Self, temperature: ThermodynamicTemperature, fluence: FastNeutronFluence) -> Result<ThermalConductivity, TampinesError> { /* ... */ }
  ```
  Thermal conductivity of the matrix / shell graphite, W/(m K), at the

- ```rust
  pub fn fuelled_zone_conductivity(self: &Self, temperature: ThermodynamicTemperature, fluence: FastNeutronFluence) -> Result<ThermalConductivity, TampinesError> { /* ... */ }
  ```
  Effective thermal conductivity of the **fuelled zone**, W/(m K), at the

- ```rust
  pub fn steady_state_temperatures(self: &Self, power: Power, surface_temperature: ThermodynamicTemperature, fluence: FastNeutronFluence) -> Result<PebbleTemperatureProfile, TampinesError> { /* ... */ }
  ```
  Steady-state radial temperature profile of the pebble, given its total

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Pebble { /* ... */ }
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
    fn eq(self: &Self, other: &Pebble) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `PebbleTemperatureProfile`

The steady radial temperature field of a fuel pebble, plus the hottest
coated particle superposed on it.

Every field is an absolute temperature (`uom`
`ThermodynamicTemperature`, kelvin in SI). Produced by
[`Pebble::steady_state_temperatures`].

```rust
pub struct PebbleTemperatureProfile {
    pub centre: uom::si::f64::ThermodynamicTemperature,
    pub fuelled_zone_boundary: uom::si::f64::ThermodynamicTemperature,
    pub surface: uom::si::f64::ThermodynamicTemperature,
    pub peak_kernel_centre: uom::si::f64::ThermodynamicTemperature,
    pub hottest_particle: super::triso::TrisoTemperatureProfile,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `centre` | `uom::si::f64::ThermodynamicTemperature` | Temperature at the geometric centre of the pebble — the hottest point<br>of the *matrix*, but not the hottest point of the fuel; see<br>[`Self::peak_kernel_centre`]. |
| `fuelled_zone_boundary` | `uom::si::f64::ThermodynamicTemperature` | Temperature at the fuelled-zone / unfuelled-shell boundary. |
| `surface` | `uom::si::f64::ThermodynamicTemperature` | Temperature imposed on the pebble outer surface (the model's boundary<br>condition, returned unchanged for convenience). |
| `peak_kernel_centre` | `uom::si::f64::ThermodynamicTemperature` | Temperature at the centre of the hottest UO2 kernel — the true peak<br>fuel temperature, obtained by superposing the level-1 particle solution<br>on [`Self::centre`]. This is the quantity a fuel-temperature limit<br>applies to. |
| `hottest_particle` | `super::triso::TrisoTemperatureProfile` | The full level-1 profile of that hottest particle, for callers that<br>want its internal breakdown (SiC temperature, for instance, which<br>governs fission-product retention). |

##### Implementations

###### Methods

- ```rust
  pub fn total_rise(self: &Self) -> uom::si::f64::TemperatureInterval { /* ... */ }
  ```
  Total temperature rise from the pebble surface to the hottest kernel

- ```rust
  pub fn matrix_rise(self: &Self) -> uom::si::f64::TemperatureInterval { /* ... */ }
  ```
  Temperature rise across the pebble alone, surface to centre, kelvin —

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PebbleTemperatureProfile { /* ... */ }
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
    fn eq(self: &Self, other: &PebbleTemperatureProfile) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `uranium_dioxide_heavy_metal_fraction`

Heavy-metal (uranium) mass fraction of UO2 at the given U-235 enrichment by
weight, dimensionless.

`enrichment` is the U-235 **weight** fraction of the uranium (HTR-10: 0.17).
The weight fraction is converted to an atom fraction before averaging the
uranium molar mass, because it is atoms, not grams, that pair with the two
oxygens:

`x_235 = (w_235 / M_235) / (w_235 / M_235 + w_238 / M_238)`

`M_U = x_235 M_235 + (1 - x_235) M_238`

`f_HM = M_U / (M_U + 2 M_O)`

with the IUPAC/CIAAW standard atomic weights in
[`MOLAR_MASS_U235_G_PER_MOL`], [`MOLAR_MASS_U238_G_PER_MOL`] and
[`MOLAR_MASS_OXYGEN_G_PER_MOL`]. At 17 wt% enrichment the result is about
0.8813; for natural uranium it is about 0.8815 — the enrichment dependence
is very weak, which is why quoting one figure for "UO2" is usually safe and
why this function exists anyway, so the assumption is visible.

Returns [`TampinesError::InvalidInput`] for an enrichment outside `[0, 1]`.

```rust
pub fn uranium_dioxide_heavy_metal_fraction(enrichment: uom::si::f64::Ratio) -> Result<uom::si::f64::Ratio, crate::TampinesError> { /* ... */ }
```

#### Function `coated_particles_per_pebble`

Number of coated particles in a pebble, derived from the published
heavy-metal loading rather than quoted.

`N = m_HM / (V_kernel * rho_UO2 * f_HM)`, where `V_kernel` is the sphere
volume of `kernel_radius`, `rho_UO2` the kernel density, and `f_HM` the
uranium mass fraction of UO2 from
[`uranium_dioxide_heavy_metal_fraction`]. The result is a real number, not
rounded — a fuel specification fixes the loading, and the particle count
that follows need not be an integer.

This is the check that a pebble's stated particle count and its stated
heavy-metal loading describe the same fuel element; see the unit test,
which recovers the HTR-10 figure of 8335 from the published 5.0 g loading.

Returns [`TampinesError::InvalidInput`] for a non-positive radius, density
or mass, or an enrichment outside `[0, 1]`.

```rust
pub fn coated_particles_per_pebble(heavy_metal_mass: uom::si::f64::Mass, kernel_radius: uom::si::f64::Length, uranium_dioxide_density: uom::si::f64::MassDensity, enrichment: uom::si::f64::Ratio) -> Result<f64, crate::TampinesError> { /* ... */ }
```

#### Function `htr10_heavy_metal_per_pebble`

The HTR-10 fuel element's published heavy-metal loading, 5.0 g of uranium
per pebble (IAEA-TECDOC-1382 part 2, Chapter 4, Tables 4-2 and 4-17, Open
tier).

```rust
pub fn htr10_heavy_metal_per_pebble() -> uom::si::f64::Mass { /* ... */ }
```

#### Function `htr10_uranium_dioxide_density`

The HTR-10 fuel kernel's published density, 10.4 g/cm^3
(IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-17, Open tier).

```rust
pub fn htr10_uranium_dioxide_density() -> uom::si::f64::MassDensity { /* ... */ }
```

#### Function `htr10_enrichment`

The HTR-10 fresh fuel's published U-235 enrichment, 17% by weight
(IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-17, Open tier).

```rust
pub fn htr10_enrichment() -> uom::si::f64::Ratio { /* ... */ }
```

### Constants and Statics

#### Constant `MOLAR_MASS_U235_G_PER_MOL`

Standard atomic weight of U-235, 235.0439299 g/mol (IUPAC/CIAAW).

```rust
pub const MOLAR_MASS_U235_G_PER_MOL: f64 = 235.0439299;
```

#### Constant `MOLAR_MASS_U238_G_PER_MOL`

Standard atomic weight of U-238, 238.0507882 g/mol (IUPAC/CIAAW).

```rust
pub const MOLAR_MASS_U238_G_PER_MOL: f64 = 238.0507882;
```

#### Constant `MOLAR_MASS_OXYGEN_G_PER_MOL`

Standard atomic weight of oxygen, 15.9994 g/mol (IUPAC/CIAAW).

```rust
pub const MOLAR_MASS_OXYGEN_G_PER_MOL: f64 = 15.9994;
```

## Module `triso`

# TRISO coated-particle conduction — the innermost of the nested scales

Steady-state radial heat conduction through the five concentric regions of
a TRISO coated fuel particle:

| Region | Material | Role |
|---|---|---|
| Kernel | UO2 | where the fission power is released |
| Buffer | porous pyrolytic carbon | accommodates fission gas and kernel swelling |
| IPyC | dense pyrolytic carbon | seals the kernel, protects the SiC from fission products |
| SiC | silicon carbide | the pressure-bearing, metallic-fission-product barrier |
| OPyC | dense pyrolytic carbon | protects the SiC mechanically |

All the fission power is generated in the kernel and conducts outward, so
every coating layer carries the *whole* particle power through a pure
series resistance. The temperature field is therefore closed-form, layer by
layer (see [`spherical_shell_temperature_rise`] and
[`solid_sphere_centre_temperature_rise`]) — no discretisation is needed,
and none is used.

## Where this sits in the nest

This module is level 1 of three. Its
[`TrisoParticle::effective_conductivity`] is the *input* to level 2
([`super::pebble`], which disperses these particles in matrix graphite),
whose pebble-surface result is in turn the input to level 3
([`super::cht`], the bed-to-helium coupling) and to the bed effective
conductivity in [`super::zbs`].

## Geometry: reuse of `boon-lay`, not a second copy

`boon-lay` already owns a five-layer TRISO CSG cell
(`TrisoCell`) with `uom`-typed concentric radii, built for its Lagrangian
fission-product diffusion model. This module **reuses that geometry** —
[`TrisoParticle::from_boon_lay_cell`] and
[`TrisoParticle::to_boon_lay_cell`] convert both ways — rather than
defining a rival geometry type. The dependency edge is maintainer-approved
(2026-08-11, `op-jyyp.5`) and declared in `Cargo.toml`.

**What is deliberately NOT consumed from `boon-lay`:** its fission-product
*release* model. Bead `op-jyyp.10` records that model's CRP-6 verification
test as defective — it wraps the reference assertion in `catch_unwind` and
discards the result, so it verifies nothing. Only geometry and the
per-layer property *pattern* are reused here; the conduction physics below
is this module's own.

## Property provenance

Layer conductivities are transcribed from the **Virtual Test Bed** HTR-PM
pebble model, vendored in this workspace at
`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`
(CC-BY-4.0, **Open tier**), `[Functions]` block, lines 165-197: `uo2_k`,
`buffer_k`, `pyc_k`, `sic_k`. The deck names no upstream literature source
for any of them. The fast-neutron-fluence damage factor shared by the
carbon layers is *not* re-implemented here — it is called from
[`tuas_boussinesq_solver`]'s already-tested
`nuclear_graphite_fluence_damage_factor`, which transcribes the same deck
expression.

Geometry for [`TrisoParticle::htr10`] comes from **IAEA-TECDOC-1382 part 2,
Chapter 4** (Open tier; catalogued at
`crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.pdf`),
Table 4-2 / Table 4-17: kernel radius 0.025 cm, UO2 density 10.4 g/cm^3,
coating layers PyC/PyC/SiC/PyC of thickness 0.009/0.004/0.0035/0.004 cm and
density 1.1/1.9/3.18/1.9 g/cm^3.

## Status

**NOT VALIDATED.** The conduction solution is verified against analytic
limits (see the tests at the bottom of this file) and the property
correlations are verified as transcriptions, but nothing here has been
compared against a TRISO temperature measurement. AI-assisted draft pending
human review per `RESPONSIBLE_USE.md`.

**Belongs here:** particle-scale geometry, per-layer conductivity, and the
particle's own steady conduction solution. **Does not belong here:**
fission-product diffusion or release (that is `boon-lay`'s), the matrix
graphite dispersion (level 2, [`super::pebble`]), or anything about the
bed.

```rust
pub mod triso { /* ... */ }
```

### Types

#### Type Alias `FastNeutronFluence`

Fast-neutron fluence, expressed as the dimensionless deck parameter `gam`
in units of **10^25 n/m^2 (E > 0.1 MeV)**.

This is the same quantity, with the same unit interpretation, that
[`tuas_boussinesq_solver`]'s `nuclear_graphite_fluence_damage_factor`
takes; see that function's documentation for why the interpretation is an
*interpretation* (the Virtual Test Bed deck declares no unit for `gam`).
Valid range is `[0, 15]`; `Ratio::new::<ratio>(0.0)` means fresh,
unirradiated fuel.

```rust
pub type FastNeutronFluence = uom::si::f64::Ratio;
```

#### Enum `TrisoLayer`

The five concentric material regions of a TRISO coated fuel particle,
innermost first.

A closed set — enum dispatch, no trait objects, per the workspace Rust
design rules. `InnerPyC` and `OuterPyC` are distinct variants even though
they share one conductivity correlation, because they occupy different
radii and therefore different series resistances.

```rust
pub enum TrisoLayer {
    Kernel,
    Buffer,
    InnerPyC,
    SiliconCarbide,
    OuterPyC,
}
```

##### Variants

###### `Kernel`

The UO2 fuel kernel — the only region in which fission power is
released.

###### `Buffer`

The porous (low-density) pyrolytic carbon buffer.

###### `InnerPyC`

The inner dense pyrolytic carbon layer.

###### `SiliconCarbide`

The silicon carbide pressure-bearing layer.

###### `OuterPyC`

The outer dense pyrolytic carbon layer.

##### Implementations

###### Methods

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TrisoLayer { /* ... */ }
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
    fn eq(self: &Self, other: &TrisoLayer) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TrisoParticle`

The concentric geometry and coating densities of one TRISO coated fuel
particle.

Plain data: the radii are *outer* radii of each layer, strictly increasing
outward, in metres; the two densities set the porosity factor of the carbon
conductivity correlations. The physics lives in
[`TrisoParticle::steady_state_temperatures`] and
[`TrisoParticle::effective_conductivity`].

Construct with [`TrisoParticle::new`] (checked), [`TrisoParticle::htr10`]
(the cited HTR-10 particle), or [`TrisoParticle::from_boon_lay_cell`]
(reusing a `boon-lay` CSG cell).

```rust
pub struct TrisoParticle {
    pub kernel_radius: uom::si::f64::Length,
    pub buffer_outer_radius: uom::si::f64::Length,
    pub inner_pyc_outer_radius: uom::si::f64::Length,
    pub silicon_carbide_outer_radius: uom::si::f64::Length,
    pub outer_pyc_outer_radius: uom::si::f64::Length,
    pub buffer_density: uom::si::f64::MassDensity,
    pub pyrocarbon_density: uom::si::f64::MassDensity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kernel_radius` | `uom::si::f64::Length` | Outer radius of the UO2 kernel, metres. HTR-10: 0.025 cm = 250 um. |
| `buffer_outer_radius` | `uom::si::f64::Length` | Outer radius of the porous carbon buffer, metres. HTR-10:<br>250 + 90 = 340 um. |
| `inner_pyc_outer_radius` | `uom::si::f64::Length` | Outer radius of the inner pyrolytic carbon layer, metres. HTR-10:<br>340 + 40 = 380 um. |
| `silicon_carbide_outer_radius` | `uom::si::f64::Length` | Outer radius of the silicon carbide layer, metres. HTR-10:<br>380 + 35 = 415 um. |
| `outer_pyc_outer_radius` | `uom::si::f64::Length` | Outer radius of the outer pyrolytic carbon layer — the particle's own<br>outer surface, metres. HTR-10: 415 + 40 = 455 um. |
| `buffer_density` | `uom::si::f64::MassDensity` | Mass density of the porous carbon buffer, kg/m^3. HTR-10:<br>1100 kg/m^3 (1.1 g/cm^3, IAEA-TECDOC-1382 part 2 Table 4-17). Enters<br>the buffer conductivity only through its porosity factor. |
| `pyrocarbon_density` | `uom::si::f64::MassDensity` | Mass density of the dense pyrolytic carbon layers (IPyC and OPyC share<br>one value), kg/m^3. HTR-10: 1900 kg/m^3 (1.9 g/cm^3, same table). |

##### Implementations

###### Methods

- ```rust
  pub fn new(kernel_radius: Length, buffer_outer_radius: Length, inner_pyc_outer_radius: Length, silicon_carbide_outer_radius: Length, outer_pyc_outer_radius: Length, buffer_density: MassDensity, pyrocarbon_density: MassDensity) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Builds a TRISO particle from its five outer radii and the two carbon

- ```rust
  pub fn htr10() -> Self { /* ... */ }
  ```
  The HTR-10 coated fuel particle, transcribed from **IAEA-TECDOC-1382

- ```rust
  pub fn from_boon_lay_cell(cell: &boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell, buffer_density: MassDensity, pyrocarbon_density: MassDensity) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Builds a [`TrisoParticle`] from a `boon-lay` `TrisoCell`, reusing that

- ```rust
  pub fn to_boon_lay_cell(self: &Self) -> boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell { /* ... */ }
  ```
  Converts this particle's geometry into a `boon-lay` `TrisoCell`, so the

- ```rust
  pub fn layer_outer_radius(self: &Self, layer: TrisoLayer) -> Length { /* ... */ }
  ```
  Outer radius of the given layer, metres.

- ```rust
  pub fn layer_inner_radius(self: &Self, layer: TrisoLayer) -> Length { /* ... */ }
  ```
  Inner radius of the given layer, metres. Zero for the kernel, which is

- ```rust
  pub fn layer_volume(self: &Self, layer: TrisoLayer) -> Volume { /* ... */ }
  ```
  Volume of the given layer, m^3 — the shell volume

- ```rust
  pub fn particle_volume(self: &Self) -> Volume { /* ... */ }
  ```
  Total volume of the whole coated particle, m^3, out to the OPyC

- ```rust
  pub fn layer_volume_fraction(self: &Self, layer: TrisoLayer) -> Ratio { /* ... */ }
  ```
  Volume fraction of the given layer within the whole particle,

- ```rust
  pub fn layer_thermal_conductivity(self: &Self, layer: TrisoLayer, temperature: ThermodynamicTemperature, fluence: FastNeutronFluence) -> Result<ThermalConductivity, TampinesError> { /* ... */ }
  ```
  Thermal conductivity of one layer, W/(m K), at the given temperature

- ```rust
  pub fn effective_conductivity(self: &Self, temperature: ThermodynamicTemperature, fluence: FastNeutronFluence) -> Result<ThermalConductivity, TampinesError> { /* ... */ }
  ```
  Effective (homogenised) thermal conductivity of the whole coated

- ```rust
  pub fn steady_state_temperatures(self: &Self, power: Power, surface_temperature: ThermodynamicTemperature, fluence: FastNeutronFluence) -> Result<TrisoTemperatureProfile, TampinesError> { /* ... */ }
  ```
  Steady-state radial temperature profile of the particle, given the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TrisoParticle { /* ... */ }
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
    fn eq(self: &Self, other: &TrisoParticle) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TrisoTemperatureProfile`

The steady radial temperature field of a TRISO particle, node by node from
the kernel centre outward.

Every field is an absolute temperature (`uom`
`ThermodynamicTemperature`, kelvin in SI), not a rise. Produced by
[`TrisoParticle::steady_state_temperatures`].

```rust
pub struct TrisoTemperatureProfile {
    pub kernel_centre: uom::si::f64::ThermodynamicTemperature,
    pub kernel_surface: uom::si::f64::ThermodynamicTemperature,
    pub buffer_outer: uom::si::f64::ThermodynamicTemperature,
    pub inner_pyc_outer: uom::si::f64::ThermodynamicTemperature,
    pub silicon_carbide_outer: uom::si::f64::ThermodynamicTemperature,
    pub particle_surface: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kernel_centre` | `uom::si::f64::ThermodynamicTemperature` | Temperature at the centre of the UO2 kernel — the hottest point in the<br>particle, and the figure of merit for fuel-temperature limits. |
| `kernel_surface` | `uom::si::f64::ThermodynamicTemperature` | Temperature at the kernel/buffer interface. |
| `buffer_outer` | `uom::si::f64::ThermodynamicTemperature` | Temperature at the buffer/IPyC interface. |
| `inner_pyc_outer` | `uom::si::f64::ThermodynamicTemperature` | Temperature at the IPyC/SiC interface. |
| `silicon_carbide_outer` | `uom::si::f64::ThermodynamicTemperature` | Temperature at the SiC/OPyC interface — the SiC layer's own outer face,<br>the temperature that governs its fission-product retention. |
| `particle_surface` | `uom::si::f64::ThermodynamicTemperature` | Temperature imposed on the OPyC outer surface (the model's boundary<br>condition, returned unchanged for convenience). |

##### Implementations

###### Methods

- ```rust
  pub fn total_rise(self: &Self) -> TemperatureInterval { /* ... */ }
  ```
  Total temperature rise across the particle, from the OPyC surface to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TrisoTemperatureProfile { /* ... */ }
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
    fn eq(self: &Self, other: &TrisoTemperatureProfile) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `solid_sphere_centre_temperature_rise`

Centre-to-surface temperature rise of a solid sphere of radius `radius`
and uniform conductivity `conductivity` generating total power `power`
uniformly throughout its volume:

`T(0) - T(R) = q''' R^2 / (6 k) = Q / (8 pi k R)`

with `q''' = Q / ((4/3) pi R^3)`. The two forms are algebraically
identical; the second is evaluated here because total power is what a
particle or pebble model carries.

Units: `power` in watts, `radius` in metres, `conductivity` in W/(m K);
the result is a `uom` `TemperatureInterval` in kelvin. No range checking —
this is exact algebra, valid for any positive radius and conductivity.

```rust
pub fn solid_sphere_centre_temperature_rise(power: uom::si::f64::Power, radius: uom::si::f64::Length, conductivity: uom::si::f64::ThermalConductivity) -> uom::si::f64::TemperatureInterval { /* ... */ }
```

#### Function `spherical_shell_temperature_rise`

Temperature rise across a spherical shell of uniform conductivity carrying
a fixed total power through it:

`T(r_inner) - T(r_outer) = Q / (4 pi k) * (1/r_inner - 1/r_outer)`

This is the exact steady solution of Laplace's equation in a shell with no
internal generation — the case of every TRISO coating layer, which carries
the kernel's power but produces none of its own.

Units: `power` in watts, radii in metres, `conductivity` in W/(m K); the
result is a `uom` `TemperatureInterval` in kelvin. The reciprocal
difference is formed as `(r_outer - r_inner) / (r_inner r_outer)` so the
expression stays `uom`-typed throughout. Requires
`0 < r_inner < r_outer`; a caller violating that gets a negative or
infinite rise rather than an error, which is why the public entry points
validate geometry at construction instead.

```rust
pub fn spherical_shell_temperature_rise(power: uom::si::f64::Power, inner_radius: uom::si::f64::Length, outer_radius: uom::si::f64::Length, conductivity: uom::si::f64::ThermalConductivity) -> uom::si::f64::TemperatureInterval { /* ... */ }
```

#### Function `uranium_dioxide_thermal_conductivity`

Thermal conductivity of **fresh, zero-burnup** UO2, W/(m K), at the given
temperature.

Implements the zero-burnup branch of the `uo2_k` function of the Virtual
Test Bed HTR-PM pebble model
(`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`,
lines 166-176; CC-BY-4.0, Open tier), with `t` the temperature in kelvin
and `x = t/1000`:

`k(t) = 115.8 / (7.5408 + 17.692 x + 3.6142 x^2) + 7410.5 x^(-5/2) exp(-16.35 / x)`

The first term is the phonon (lattice) conduction, falling with
temperature; the second is the electronic/small-polaron contribution, which
only becomes significant above about 1800 K. The deck names no upstream
literature source.

**Burnup is not modelled here.** The deck's non-zero-burnup branch degrades
`k` with fissions per initial metal atom; this function is the fresh-fuel
limit of that expression, which is the correct one for an unirradiated or
beginning-of-life particle and an *optimistic* one (conductivity too high,
kernel temperature too low) for burnt fuel. Extending to burnup is
deliberate future work, not an oversight.

Valid range: 300 K to 2000 K; outside it, returns
[`TampinesError::InvalidInput`].

```rust
pub fn uranium_dioxide_thermal_conductivity(temperature: uom::si::f64::ThermodynamicTemperature) -> Result<uom::si::f64::ThermalConductivity, crate::TampinesError> { /* ... */ }
```

#### Function `pyrocarbon_thermal_conductivity`

Thermal conductivity of **dense pyrolytic carbon** (the IPyC and OPyC
layers), W/(m K), at the given temperature, layer density and fast-neutron
fluence.

Implements the `pyc_k` function of the Virtual Test Bed HTR-PM pebble model
(`pebble_triso.i`, lines 182-186; CC-BY-4.0, Open tier), with the hardcoded
deck density 1900 kg/m^3 generalised to a caller-supplied `density` so the
HTR-10 particle's own 1.9 g/cm^3 (or any other grade) can be used:

`k(t, rho, gam) = 244.3 t^(-0.574) * rho / (2.2 (1930 - rho) + rho) * F(gam)`

with `t` in kelvin, `rho` in kg/m^3, 1930 kg/m^3 the theoretical carbon
density ([`THEORETICAL_CARBON_DENSITY_KG_PER_M3`]), and `F(gam)` the
fast-fluence damage factor
`1 - 0.336 (1 - exp(-1.005 gam)) - 0.035 gam`, which is **not**
re-implemented here — it is called from [`tuas_boussinesq_solver`]'s
already-tested `nuclear_graphite_fluence_damage_factor`, which transcribes
the same deck expression. The middle factor is a Maxwell-type porosity
correction; at the deck's own 1900 kg/m^3 it evaluates to about 0.9664.
The deck names no upstream literature source.

Valid range: temperature 300 K to 2000 K, fluence `gam` in `[0, 15]`,
density strictly between 0 and 1930 kg/m^3; outside any of these, returns
[`TampinesError::InvalidInput`].

```rust
pub fn pyrocarbon_thermal_conductivity(temperature: uom::si::f64::ThermodynamicTemperature, density: uom::si::f64::MassDensity, fluence: FastNeutronFluence) -> Result<uom::si::f64::ThermalConductivity, crate::TampinesError> { /* ... */ }
```

#### Function `buffer_carbon_thermal_conductivity`

Thermal conductivity of the **porous carbon buffer**, W/(m K), at the given
temperature, buffer density and fast-neutron fluence.

Implements the `buffer_k` function of the Virtual Test Bed HTR-PM pebble
model (`pebble_triso.i`, lines 177-181; CC-BY-4.0, Open tier), which is the
dense-pyrocarbon expression with its leading coefficient **halved**:

`k(t, rho, gam) = (244.3 / 2) t^(-0.574) * rho / (2.2 (1930 - rho) + rho) * F(gam)`

The deck applies the factor of one half only to the buffer, on top of the
porosity factor that already accounts for the buffer's low density; it
states no reason and names no upstream source. It is transcribed here as
written rather than "corrected", because the reference implementation is
what this module claims to reproduce. The deck's hardcoded 970 kg/m^3 is
generalised to the caller's `density` (HTR-10 uses 1.1 g/cm^3).

Valid range: as [`pyrocarbon_thermal_conductivity`].

```rust
pub fn buffer_carbon_thermal_conductivity(temperature: uom::si::f64::ThermodynamicTemperature, density: uom::si::f64::MassDensity, fluence: FastNeutronFluence) -> Result<uom::si::f64::ThermalConductivity, crate::TampinesError> { /* ... */ }
```

#### Function `silicon_carbide_thermal_conductivity`

Thermal conductivity of **silicon carbide**, W/(m K), at the given
temperature and fast-neutron fluence.

Implements the `sic_k` function of the Virtual Test Bed HTR-PM pebble model
(`pebble_triso.i`, lines 187-191; CC-BY-4.0, Open tier):

`k(t, gam) = (17885 / t + 2) exp(-0.1277 gam)`

with `t` in kelvin. SiC is by far the most conductive TRISO layer (about
19.9 W/(m K) at 1000 K unirradiated) but also the most fluence-sensitive:
its damage term is a bare exponential rather than the carbon layers'
saturating factor, so at `gam = 10` it retains only about 28% of its
unirradiated conductivity. The deck names no upstream literature source.

**Fluence range.** The exponential never goes negative, so this function
enforces the same `[0, 15]` window as the carbon layers purely for
consistency across the stack — not because the correlation itself breaks
down there.

Valid range: temperature 300 K to 2000 K, fluence `gam` in `[0, 15]`;
outside either, returns [`TampinesError::InvalidInput`].

```rust
pub fn silicon_carbide_thermal_conductivity(temperature: uom::si::f64::ThermodynamicTemperature, fluence: FastNeutronFluence) -> Result<uom::si::f64::ThermalConductivity, crate::TampinesError> { /* ... */ }
```

### Constants and Statics

#### Constant `MIN_TEMPERATURE_KELVIN`

Lowest temperature, 300 K, at which the layer conductivity correlations of
this module are evaluated.

The Virtual Test Bed deck states no validity range for any of them; 300 K
is adopted to match the window
[`tuas_boussinesq_solver`]'s nuclear-graphite correlations already enforce,
so every property in the pebble-bed stack shares one coded window.

```rust
pub const MIN_TEMPERATURE_KELVIN: f64 = 300.0;
```

#### Constant `MAX_TEMPERATURE_KELVIN`

Highest temperature, 2000 K, at which the layer conductivity correlations
of this module are evaluated. See [`MIN_TEMPERATURE_KELVIN`] for why this
window was chosen.

```rust
pub const MAX_TEMPERATURE_KELVIN: f64 = 2000.0;
```

#### Constant `THEORETICAL_CARBON_DENSITY_KG_PER_M3`

Theoretical (pore-free) density of carbon, 1930 kg/m^3, used as the
reference density in the pyrocarbon and buffer conductivity porosity
factors.

Source: the constant `1930.` appearing in `buffer_k` and `pyc_k` of the
Virtual Test Bed HTR-PM pebble deck (`pebble_triso.i`, lines 178-186,
CC-BY-4.0, Open tier). The deck names no upstream source.

```rust
pub const THEORETICAL_CARBON_DENSITY_KG_PER_M3: f64 = 1930.0;
```

## Module `zbs`

# Zehner-Bauer-Schlunder (ZBS) pebble-bed effective thermal conductivity

Analytic effective thermal conductivity of a packed bed of spheres,
summing four heat-transfer paths through a unit cell:

- conduction through the stagnant gas filling the voids,
- conduction through the touching solid spheres,
- conduction through the finite flattened *contact areas* between
  spheres (the `phi` term), and
- thermal radiation between sphere surfaces (the `k_r` term, growing as
  `T^3` — the dominant decay-heat path at loss-of-forced-cooling
  temperatures; never drop it).

## Homogenisation assumption

Everything here treats the bed as a **homogeneous effective medium**: a
single scalar conductivity that a continuum energy equation may use in
place of resolving individual pebbles, voids and contact points. The
correlation returns a volume-averaged quantity, so it says nothing about
the pebble-scale temperature field — local peaking at contact points,
the pebble-interior radial profile (that is [`super::pebble`]'s job) and
the near-wall channelling of a real bed are all averaged out. It is also
**isotropic and stagnant**: no directional dependence, and no
convective enhancement from through-flow.

## Formulation and provenance

The correlation is Bauer & Schlunder's (1978) extension of the
Zehner-Schlunder (1970) unit-cell model, in the dimensionless form given
in the review of van Antwerpen, du Toit & Rousseau, *Nucl. Eng. Des.*
240 (2010) 1803-1818 (**proprietary tier** — cited and implemented from,
not reproduced at length) and presented for gas-cooled reactors in
IAEA-TECDOC-1163 (**Open tier**). With the Knudsen (Smoluchowski) factor
set to one — valid for helium at reactor pressures, where the molecular
mean free path (tens of nanometres) is vanishingly small against a 6 cm
pebble — the equations are, with `eps` the porosity, `kappa = k_s/k_f`,
`d` the pebble diameter, `e_r` the surface emissivity and `sigma` the
Stefan-Boltzmann constant:

```text
B    = C * ((1 - eps)/eps)^(10/9)                    (deformation factor)
k_r  = 4*sigma*T^3*d / ((2/e_r - 1) * k_f)           (radiation ratio)
N    = 1 + (k_r - B)/kappa
k_c  = (2/N) * [ B*(kappa + k_r - 1)/(N^2 * kappa) * ln((kappa + k_r)/B)
                 + (B + 1)/(2B) * (k_r - B)
                 - (B - 1)/N ]
k_eff/k_f = (1 - sqrt(1-eps)) * (1 + eps*k_r)
            + sqrt(1-eps) * ( phi*kappa + (1 - phi)*k_c )
```

`C = 1.25` is the sphere shape factor and `phi = 0.0077` the standard
contact-area fraction, both from Bauer & Schlunder as quoted in the van
Antwerpen review. At `k_r = 0` the unit-cell term `k_c` reduces exactly
to the classic Zehner-Schlunder stagnant-bed form — a limit the test
`tests::zero_radiation_degenerates_to_the_classic_zehner_schlunder_form`
exercises. (The `tests` module is `#[cfg(test)]`, so the test names
quoted throughout these docs are code spans rather than doc links —
rustdoc cannot resolve into a test-only module.)

## Verification status (measured 2026-08-11)

**Transcription caveat (honesty per `RESPONSIBLE_USE.md`).** The
dimensionless form above was implemented without page-level access to
the printed originals. The analytic-limit checks in `tests` were
written and **run** on 2026-08-11; their measured outcomes are:

- **Uniform-medium collapse** (`k_s = k_f` must give `k_eff = k_f`) is
  *exact* where the unit cell is defined — maximum relative deviation
  `4.4e-16`, i.e. f64 roundoff, over porosities 0.6 and 0.8 at
  `k = 0.15`, `1.0` and `26.0` W/(m K). It is **not** defined at the
  HTR-10 porosity: `kappa = 1` requires `B < 1`, which for `C = 1.25`
  needs `eps` above about 0.55, and at `eps = 0.39` (`B = 2.0548`) the
  correlation correctly returns [`TampinesError::Unphysical`] instead
  of a number. An earlier revision of these docs claimed this check
  passed unconditionally; it does not, and the qualifier above is the
  measured behaviour.
- **Zehner-Schlunder degeneration** at `k_r -> 0`: agrees with the
  classic closed form to `1.398e-16` relative. This is an *algebraic*
  cross-check — the same published expression regrouped — so it
  verifies the limit and the arithmetic, **not** the transcription of
  the source equations.
- **Wiener bounds**: the conduction-only result lies strictly between
  the series and parallel bounds over a 27-point `(eps, k_s, k_f)`
  grid; the tightest margins measured were `k_eff/k_series = 2.3754`
  and `k_eff/k_parallel = 0.3721`. Radiation legitimately pushes the
  *full* result above the conduction-only parallel bound at high
  temperature (28.9649 vs 16.0896 W/(m K) at 2000 K), so the bound
  check deliberately covers the conduction-only regime.
- **Radiation monotonicity and `d`-scaling**: at fixed `k_f`, `k_eff`
  rises strictly monotonically over 1701 samples from 300 K to 2000 K
  (2.1132 to 28.6049 W/(m K)), and `k_r` is proportional to `d` to
  f64 exactness (measured deviation 0.0 on doubling and halving).

A human should still verify the transcription against van Antwerpen et
al. (2010) eqs. (12)-(16) before this module is promoted past Prototype
in the V&V pipeline. **Nothing here is validated** — no comparison
against measurement has been made, and the one reference tabulation
available is *not* reproduced (next section).

## The VTB 18-point table is NOT reproduced — measured finding

The Virtual Test Bed generic pebble-bed deck
(`reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i`, block
`keff_pebble_bed`, Open tier) carries an 18-point `k_eff` tabulation
(300-2000 K, 11.940293-44.9504677 W/(m K)) described there as
"calculated from the ZBS correlation". This implementation, evaluated at
that deck's own stated inputs (eps = 0.39, d = 0.06 m, e_r = 0.8,
graphite k_s = 26 W/(m K), helium at 6 MPa — all four read back out of
the deck on 2026-08-11), does **not** reproduce it. Measured on
2026-08-11: the model lies **below the table at all 18 points**, by a
factor of **5.65** at 300 K (2.1126 vs 11.9403 W/(m K), the worst
point) narrowing to **1.55** at 2000 K (28.9649 vs 44.9505 W/(m K), the
best point); the model/table ratio rises monotonically from 0.17693 to
0.64437.

The gap is not a tuning matter. Solving this implementation for the pore
conductivity that *would* land on the table's 300 K value gives
**k_f = 3.8367 W/(m K)** — **24.0 times** the 0.15992 W/(m K) that
`outram-park-fork-coolprop` gives for helium at 300 K and 6 MPa — and no
gas has such a conductivity. (An earlier revision of these docs also
appealed to "order 1-2 W/(m K) at ambient in SANA/HTTU-class
experiments". That is an **uncited recollection**, not checked against a
source here; it is retained only as a hypothesis for the human reviewer,
and none of the measured numbers above depend on it.) The full
quantitative comparison is pinned in
`tests::vtb_table_is_not_reproduced_by_zbs_with_helium`, and the
finding is tracked in the beads issue tracker. The table remains a
faithful transcription of what the VTB/SAM models *ran with*; whether it
is what ZBS *produces* is the open question that test documents.

## Near-wall region

Bed voidage rises toward a containing wall, which locally changes the
effective conductivity. [`ZbsBed::wall_region_porosity`] provides an
exponential bulk-to-wall porosity profile so callers can evaluate the
correlation with the local voidage; its coefficients are flagged
provisional in its own doc comment.

**Belongs here:** the ZBS correlation and its unit tests. **Does not
belong here:** graphite property data ([`tuas_boussinesq_solver`]'s
solid database), pressure drop ([`crate::gas_phase::KtaBed`]),
pebble-internal conduction ([`super::pebble`]).

```rust
pub mod zbs { /* ... */ }
```

### Types

#### Struct `ZbsBed`

A packed bed of monosized spheres, described by the four geometric and
surface parameters the Zehner-Bauer-Schlunder correlation needs. Plain
data; the physics lives in [`ZbsBed::effective_conductivity`].

Construct with [`ZbsBed::new`] (standard contact/shape factors) or
[`ZbsBed::htr10`] (the cited HTR-10 bed).

```rust
pub struct ZbsBed {
    pub porosity: uom::si::f64::Ratio,
    pub pebble_diameter: uom::si::f64::Length,
    pub emissivity: uom::si::f64::Ratio,
    pub contact_area_fraction: uom::si::f64::Ratio,
    pub shape_factor_c: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `porosity` | `uom::si::f64::Ratio` | Bed porosity (void fraction), dimensionless, strictly between 0<br>and 1. Random close-packed sphere beds sit near 0.36-0.42; the<br>HTR-10 bed is 0.39 (filling fraction 0.61, IAEA HTGR benchmark<br>document, Open tier). |
| `pebble_diameter` | `uom::si::f64::Length` | Pebble (sphere) diameter, metres. Sets the radiation length scale:<br>the `4 sigma T^3 d` radiation conductivity is proportional to it.<br>HTR-10: 0.06 m. |
| `emissivity` | `uom::si::f64::Ratio` | Total hemispherical surface emissivity of the spheres,<br>dimensionless in (0, 1]. Graphite in the pebble-bed literature is<br>taken as 0.8 (NEA PBMR-400 benchmark assumption, as quoted in the<br>VTB generic pebble-bed deck, Open tier). |
| `contact_area_fraction` | `uom::si::f64::Ratio` | Flattened contact-area fraction `phi` of the unit cell,<br>dimensionless. Standard value 0.0077 for spheres (Bauer &<br>Schlunder, as quoted in the van Antwerpen 2010 review). Governs the<br>solid-solid contact conduction path, which matters most when the<br>gas conducts poorly (vacuum/depressurised accident conditions). |
| `shape_factor_c` | `uom::si::f64::Ratio` | Sphere deformation/shape factor coefficient `C` in<br>`B = C ((1-eps)/eps)^(10/9)`, dimensionless. 1.25 for spheres<br>(Zehner & Schlunder 1970). |

##### Implementations

###### Methods

- ```rust
  pub fn new(porosity: Ratio, pebble_diameter: Length, emissivity: Ratio) -> Self { /* ... */ }
  ```
  A ZBS bed with the standard sphere constants (`phi` = 0.0077,

- ```rust
  pub fn htr10() -> Self { /* ... */ }
  ```
  The HTR-10 pebble bed: porosity 0.39 and pebble diameter 6.0 cm from

- ```rust
  pub fn deformation_factor(self: &Self) -> Ratio { /* ... */ }
  ```
  Deformation factor `B = C ((1-eps)/eps)^(10/9)`, dimensionless.

- ```rust
  pub fn radiation_ratio(self: &Self, fluid_conductivity: ThermalConductivity, temperature: ThermodynamicTemperature) -> Ratio { /* ... */ }
  ```
  Dimensionless radiation-to-fluid-conduction ratio

- ```rust
  pub fn effective_conductivity(self: &Self, solid_conductivity: ThermalConductivity, fluid_conductivity: ThermalConductivity, temperature: ThermodynamicTemperature) -> Result<ThermalConductivity, TampinesError> { /* ... */ }
  ```
  Effective bed thermal conductivity, W/(m K), from the full ZBS

- ```rust
  pub fn stagnant_conductivity(self: &Self, solid_conductivity: ThermalConductivity, fluid_conductivity: ThermalConductivity) -> Result<ThermalConductivity, TampinesError> { /* ... */ }
  ```
  Stagnant-bed conductivity: the ZBS evaluation with the radiation

- ```rust
  pub fn wall_region_porosity(self: &Self, distance_from_wall: Length) -> Ratio { /* ... */ }
  ```
  Local porosity a distance `y` from a containing wall, rising from

- ```rust
  pub fn with_porosity(self: &Self, porosity: Ratio) -> Self { /* ... */ }
  ```
  A copy of this bed with a different porosity — the intended way to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ZbsBed { /* ... */ }
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
    fn eq(self: &Self, other: &ZbsBed) -> bool { /* ... */ }
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `temperature_difference`

The difference between two absolute temperatures, as a `uom`
[`TemperatureInterval`] in kelvin.

`uom` deliberately refuses to subtract one [`ThermodynamicTemperature`]
from another with the `-` operator: absolute temperatures carry a
`TemperatureKind` marker, so that `20 degC - 10 degC` cannot be silently
mistaken for `10 degC` when it is really a 10 K *interval*. Every module in
this stack needs that interval — a conduction temperature rise, a
moderator temperature excursion above a reference — so the conversion is
written once, here, rather than open-coded per module.

Returns `hotter - colder` in kelvin; the result is negative if `colder` is
in fact the hotter of the two.

```rust
pub fn temperature_difference(hotter: uom::si::f64::ThermodynamicTemperature, colder: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::TemperatureInterval { /* ... */ }
```

### Re-exports

#### Re-export `PackedBedConvection`

```rust
pub use cht::PackedBedConvection;
```

#### Re-export `GraphiteModeratorFeedback`

```rust
pub use feedback::GraphiteModeratorFeedback;
```

#### Re-export `DispersionModel`

```rust
pub use pebble::DispersionModel;
```

#### Re-export `Pebble`

```rust
pub use pebble::Pebble;
```

#### Re-export `PebbleTemperatureProfile`

```rust
pub use pebble::PebbleTemperatureProfile;
```

#### Re-export `TrisoLayer`

```rust
pub use triso::TrisoLayer;
```

#### Re-export `TrisoParticle`

```rust
pub use triso::TrisoParticle;
```

#### Re-export `TrisoTemperatureProfile`

```rust
pub use triso::TrisoTemperatureProfile;
```

#### Re-export `ZbsBed`

```rust
pub use zbs::ZbsBed;
```

## Module `single_phase`

Lumped single-phase liquid thermal-hydraulics.

Re-exports [`tuas_boussinesq_solver`]'s finite-volume lumped-parameter
single-phase pipe/component model under a TAMPINES-local name. TUAS's
backend fluid list ([`LiquidMaterial`]) currently covers molten-salt and
thermal-oil loops (TherminolVP1, DowthermA, HITEC, YD325, FLiBe, ...) --
it does **not** yet back water, air, or helium. For those fluids, use
[`crate::compressible`] (CoolProp-backed) instead until TUAS grows
matching liquid coverage.

This is a thin wiring module: it does not add behaviour beyond naming.
The full TAMPINES fluid-array interface (unifying this with
[`crate::compressible`] and the steam-tables backend behind one enum) is
tracked separately -- see the workspace's beads issue tracker, epic
`op-dt3`.

```rust
pub mod single_phase { /* ... */ }
```

### Re-exports

#### Re-export `FluidArray`

A single-phase liquid loop segment (1D lumped finite-volume pipe/component
with lateral thermal coupling).

Alias for [`tuas_boussinesq_solver`]'s `FluidArray` -- see that type's own
documentation for the underlying physics and constructors
(`new_cylinder`, `new_annular_cylinder`, `new_odd_shaped_pipe`,
`new_custom_component`).

```rust
pub use tuas_boussinesq_solver::array_control_vol_and_fluid_component_collections::one_d_fluid_array_with_lateral_coupling::FluidArray as SinglePhaseFluidArray;
```

#### Re-export `LiquidMaterial`

The liquid substances a [`SinglePhaseFluidArray`] can currently be backed
by. Re-exported from `tuas_boussinesq_solver` for convenience.

```rust
pub use tuas_boussinesq_solver::boussinesq_thermophysical_properties::LiquidMaterial;
```

#### Re-export `TuasLibError`

Error type returned by [`SinglePhaseFluidArray`] methods. Re-exported from
`tuas_boussinesq_solver` for convenience.

```rust
pub use tuas_boussinesq_solver::tuas_lib_error::TuasLibError;
```

## Re-exports

### Re-export `TampinesError`

```rust
pub use error::TampinesError;
```

### Re-export `TampinesFluid`

```rust
pub use fluids::TampinesFluid;
```

