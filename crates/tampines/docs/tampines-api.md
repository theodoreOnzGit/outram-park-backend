# Crate Documentation

**Version:** 0.0.3

**Format Version:** 61

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

## Reaching real time: evaluate a surrogate, not the full model

This crate holds the physics a transient evaluates *most often* — fluid
properties, HEM critical flow, drift-flux closures, packed-bed conjugate
heat transfer, the KTA and ZBS correlations. That makes it the inner loop
of every coupled run, and a digital twin at interactive rates cannot afford
a full property or closure solve per tick per node.

**The intended answer is a surrogate, and it is [`raffles`].** `raffles` is
this workspace's port of RAVEN's uncertainty-quantification core, where a
reduced-order model is a first-class object: sample the expensive model
over its state space once, fit a cheap stand-in, evaluate that per tick.

This is written down because it is not the obvious move. The natural
reaction to a slow closure is to put a lookup table next to it, and a dozen
bespoke tables scattered through this crate is precisely the outcome to
avoid — they drift from the correlations they approximate, and none of them
carries an error estimate. Prefer a `raffles` surrogate over a local table,
and if you find yourself writing a table anyway, treat it as a stopgap and
say so at the call site.

Two constraints on any implementation:

- **The surrogate must stay Android-clean.** Dense linear algebra for
  fitting comes from the pure-Rust `faer` already in the workspace
  dependencies — never a system BLAS or `ndarray-linalg`, per the
  workspace's Android/Termux rule.
- **A surrogate without an error bound is not usable here.** These feed
  safety-relevant transients, and a fast wrong answer is worse than a slow
  right one.

The ROM layer itself does not exist yet — `raffles::surrogate` is currently
a stub. Tracked in the workspace beads as `op-38my`; the dependency is
wired ahead of it so the intent is visible from the manifest.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

## Module `closures`

Closures shared by the 1-D solvers (RELAP-7 parity epic gh:#1056).

- [`wall_friction`]: the RELAP5 four-region Darcy friction factor with
  Zigrang–Sylvester's turbulent form (gh:#1061). It lives in
  `tampines_steam_tables::hem::wall_friction`, beside the HEM pipe that
  also uses it, and is re-exported here for `multiphase_1d`.

```rust
pub mod closures { /* ... */ }
```

### Modules

## Module `wall_friction`

The RELAP5 four-region wall friction (gh:#1061).

```rust
pub mod wall_friction { /* ... */ }
```

### Re-exports

#### Re-export `darcy_friction_factor`

```rust
pub use tampines_steam_tables::hem::wall_friction::darcy_friction_factor;
```

#### Re-export `darcy_friction_factor_with`

```rust
pub use tampines_steam_tables::hem::wall_friction::darcy_friction_factor_with;
```

#### Re-export `wall_friction_acceleration`

```rust
pub use tampines_steam_tables::hem::wall_friction::wall_friction_acceleration;
```

#### Re-export `zigrang_sylvester`

```rust
pub use tampines_steam_tables::hem::wall_friction::zigrang_sylvester;
```

#### Re-export `LowReynolds`

```rust
pub use tampines_steam_tables::hem::wall_friction::LowReynolds;
```

#### Re-export `RE_F_MAX`

```rust
pub use tampines_steam_tables::hem::wall_friction::RE_F_MAX;
```

#### Re-export `RE_LAMINAR_MAX`

```rust
pub use tampines_steam_tables::hem::wall_friction::RE_LAMINAR_MAX;
```

#### Re-export `RE_TURBULENT_MIN`

```rust
pub use tampines_steam_tables::hem::wall_friction::RE_TURBULENT_MIN;
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

**Since 2026-11-01 (RELAP-7 parity epic gh:#1056) the transient,
verified components live beside these:** [`junction`] (the HEM network's
flow junctions, volume branches, pump, valve, relief valve, turbine,
separator-dryer and downcomer), [`heat_structure`] (PipeWithHeatStructure,
CoreChannel, wall heat exchanger), [`reactor_power`] (core power
distribution). The steady wrappers below are not their front end.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `heat_structure`

Heat structures coupled to HEM pipes: PipeWithHeatStructure, CoreChannel
and a wall heat exchanger (gh:#1077; RELAP-7 Theory Manual §4.1 and
§6.2–6.4, cited, not quoted).

> Research, education and V&V only. AI draft, unreviewed.

# What this is

[`HeatStructure`] is transient 1-D conduction through a wall, a cylinder
(a fuel rod: fuel, gap, cladding) or a slab, at every axial cell of a pipe:
`ρc ∂T/∂t = ∇·(k∇T) + q'''`, finite volume across the thickness, backward
Euler in time, one tridiagonal solve per axial cell
([`crate::multiphase_1d::thomas_solve`]). Zones carry their own `k`, `ρc`
and volumetric source; a gap is a zone of gas conductivity (radiation
across it is not modelled).

[`ThermalCoupler`] couples heat structures to the pipes of a
[`HemNetwork`]. Each step the heat to the fluid, `h A (T_s − T_f)` with
the surface temperature eliminated through the half-cell resistance, is
put into the pipe cells with [`HemPipe::set_cell_heat_input`]; the network
steps; and each structure then advances the **same** `dt` with that same
heat leaving it. The exchange is frozen over the step, so energy is
conserved exactly, and explicit in the coupling; the acoustic step of the
fluid keeps `h dt/(ρc Δr)` small.

**What exists and why this is new (searched first):** TUAS's solid arrays
(`SolidColumn`) are axial with the wall lumped radially; `bedok`'s
`fuelrodheattime_1dcylnd` is a transient rod solver, but a port of the
MATLAB code with its own geometry and sparse types, and `tampines` does
not depend on `bedok`; OFFBEAT has no thermal sub-solver. This module
reuses `thomas_solve` and, for MATPRO properties, expects the caller to
tabulate `outram-park-fork-offbeat`'s `Relap7HeatStructure` (gh:#1070)
into zone constants.

# Limits

- Properties are constant per zone (no `k(T)` yet).
- The heat-transfer coefficient is the caller's (a constant per link);
  wiring the RELAP5 dispatcher ([`crate::heat_transfer`], gh:#1063) needs
  fluid conductivity and heat capacity, which the HEM state does not carry.
- No axial conduction in the structure.
- [`ThermalCoupler::step`] **replaces** each coupled pipe's cell heat
  input every step; a reactor power source acting on the same pipe must be
  given to the structure (its zone source), not to the pipe.

```rust
pub mod heat_structure { /* ... */ }
```

### Types

#### Enum `StructureShape`

The shape of a heat structure.

```rust
pub enum StructureShape {
    Cylinder {
        inner_radius: uom::si::f64::Length,
        multiplicity: f64,
    },
    Slab {
        width: uom::si::f64::Length,
    },
}
```

##### Variants

###### `Cylinder`

A cylinder (or annulus) from `inner_radius` outward; `multiplicity`
identical rods per axial cell.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `inner_radius` | `uom::si::f64::Length` | Inner radius (0 for a solid rod, whose inner side is then the<br>symmetry axis and must be adiabatic). |
| `multiplicity` | `f64` | Number of identical rods. |

###### `Slab`

A plane wall of this width (the wall area per axial cell is
`width · Δz`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `width` | `uom::si::f64::Length` | Width normal to the axis and the thickness. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ConductionZone`

One material layer across the thickness.

```rust
pub struct ConductionZone {
    pub thickness: uom::si::f64::Length,
    pub n_cells: usize,
    pub conductivity: uom::si::f64::ThermalConductivity,
    pub volumetric_heat_capacity: uom::si::f64::VolumetricHeatCapacity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `thickness` | `uom::si::f64::Length` | Layer thickness. |
| `n_cells` | `usize` | Number of finite-volume cells across it (≥ 1). |
| `conductivity` | `uom::si::f64::ThermalConductivity` | Thermal conductivity. |
| `volumetric_heat_capacity` | `uom::si::f64::VolumetricHeatCapacity` | Volumetric heat capacity `ρc`. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `StructureBoundary`

A boundary condition on one side of a structure, for
[`HeatStructure::step`].

```rust
pub enum StructureBoundary {
    Adiabatic,
    Temperature(uom::si::f64::ThermodynamicTemperature),
    Convective {
        h: uom::si::f64::HeatTransfer,
        ambient: uom::si::f64::ThermodynamicTemperature,
    },
    HeatInflow(Vec<f64>),
}
```

##### Variants

###### `Adiabatic`

No heat crosses (also the symmetry axis of a solid rod).

###### `Temperature`

Surface temperature held.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::ThermodynamicTemperature` |  |

###### `Convective`

`h (T_s − T_∞)` leaves the surface.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `h` | `uom::si::f64::HeatTransfer` | Heat-transfer coefficient. |
| `ambient` | `uom::si::f64::ThermodynamicTemperature` | Ambient temperature. |

###### `HeatInflow`

A prescribed heat **into** the structure per axial cell, W (negative
out); frozen over the step.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `StructureSide`

Which side of a structure.

```rust
pub enum StructureSide {
    Inner,
    Outer,
}
```

##### Variants

###### `Inner`

The inner surface (smaller radius, or `x = 0` of a slab).

###### `Outer`

The outer surface.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `HeatStructure`

Transient 1-D conduction across a wall at every axial cell.

```rust
pub struct HeatStructure {
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
  pub fn new(shape: StructureShape, zones: &[ConductionZone], n_axial: usize, dz: Length, initial: ThermodynamicTemperature) -> Result<Self, TampinesError> { /* ... */ }
  ```
  A structure of `zones` (from the inner side outward) at `n_axial`

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Cells across the thickness.

- ```rust
  pub fn n_axial(self: &Self) -> usize { /* ... */ }
  ```
  Axial cells.

- ```rust
  pub fn temperature(self: &Self, j: usize, i: usize) -> Option<ThermodynamicTemperature> { /* ... */ }
  ```
  Temperature of cell `i` (across) at axial cell `j`.

- ```rust
  pub fn set_source(self: &mut Self, j: usize, cells: core::ops::Range<usize>, q: VolumetricPowerDensity) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set a uniform volumetric source in the cells of zone-range

- ```rust
  pub fn set_power(self: &mut Self, j: usize, cells: core::ops::Range<usize>, p: Power) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the sources of cells `cells` at axial cell `j` so they hold the

- ```rust
  pub fn surface_area(self: &Self, side: StructureSide) -> uom::si::f64::Area { /* ... */ }
  ```
  Area of a surface at one axial cell.

- ```rust
  pub fn stored_energy(self: &Self) -> f64 { /* ... */ }
  ```
  Stored energy `Σ ρc V T`, J (relative to 0 K).

- ```rust
  pub fn source_power(self: &Self) -> f64 { /* ... */ }
  ```
  Total source power, W.

- ```rust
  pub fn heat_to_fluid(self: &Self, side: StructureSide, j: usize, t_fluid: f64, h: f64) -> f64 { /* ... */ }
  ```
  Heat leaving `side` at axial cell `j` into a fluid at `t_fluid` with

- ```rust
  pub fn step(self: &mut Self, dt: Time, inner: &StructureBoundary, outer: &StructureBoundary) -> Result<(), TampinesError> { /* ... */ }
  ```
  Advance `dt` by backward Euler with these boundary conditions.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `SideCondition`

What one side of a coupled structure sees.

```rust
pub enum SideCondition {
    Boundary(StructureBoundary),
    Pipe {
        pipe: usize,
        h: uom::si::f64::HeatTransfer,
    },
}
```

##### Variants

###### `Boundary`

A boundary independent of the network.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `StructureBoundary` |  |

###### `Pipe`

The fluid of network pipe `pipe`, cell for cell, with coefficient `h`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pipe` | `usize` | Pipe index in the network. |
| `h` | `uom::si::f64::HeatTransfer` | Heat-transfer coefficient (constant; see the module limits). |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `HeatStructureLink`

A heat structure and what each side of it sees: a PipeWithHeatStructure
(one side on a pipe), a CoreChannel (a fuel rod with a source, its outer
side on the coolant), or a heat-exchanger wall (each side on a pipe).

```rust
pub struct HeatStructureLink {
    pub structure: HeatStructure,
    pub inner: SideCondition,
    pub outer: SideCondition,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `structure` | `HeatStructure` | The structure. |
| `inner` | `SideCondition` | Its inner side. |
| `outer` | `SideCondition` | Its outer side. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ThermalCoupler`

Couples heat structures to the pipes of a [`HemNetwork`].

```rust
pub struct ThermalCoupler {
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
  An empty coupler.

- ```rust
  pub fn add<E: HemEquationOfState>(self: &mut Self, net: &HemNetwork<E>, link: HeatStructureLink) -> Result<usize, TampinesError> { /* ... */ }
  ```
  Add a link; its pipes must have as many cells as the structure has

- ```rust
  pub fn link(self: &Self, index: usize) -> Option<&HeatStructureLink> { /* ... */ }
  ```
  The links.

- ```rust
  pub fn link_mut(self: &mut Self, index: usize) -> Option<&mut HeatStructureLink> { /* ... */ }
  ```
  The links, mutably (to set sources between steps).

- ```rust
  pub fn heat_to_fluid(self: &Self) -> uom::si::f64::Energy { /* ... */ }
  ```
  Heat given to the fluid since construction.

- ```rust
  pub fn stored_energy(self: &Self) -> f64 { /* ... */ }
  ```
  Stored energy of every structure, J.

- ```rust
  pub fn step<E: HemEquationOfState>(self: &mut Self, net: &mut HemNetwork<E>) -> Result<Time, TampinesError> { /* ... */ }
  ```
  One coupled step: set the pipes' heat input from the walls, step the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `helical_coil_sg_standalone`

**Standalone helical-coil steam-generator harness** — the exchanger alone,
with no plant attached.

# Why this exists (GitHub #343, the instrument for #319)

`htgr_sim_v1` panics in the IAPWS-IF97 flash because this exchanger's cold
side reaches **273.15 K at 4 MPa**. The maintainer's objection is the one
that matters and the steam tables do not answer it: **a 4 MPa cold side has
no physical route to 273 K when feedwater enters at 313.15 K.** Feedwater
enthalpy at the design point is ~171 kJ/kg; the panicking cell sits at
~4 kJ/kg — a factor of 42 of energy unaccounted for. Something removes energy
that is not there to remove.

Inside a whole plant that is undiagnosable: point kinetics, the helium loop,
the outer correctors, a feedwater controller, a protection system, a
secondary-isolation interlock and a timestep accumulator all reach the
exchanger. This harness drives
[`NodalisedCounterFlowSteamGenerator`] and **nothing else**, with boundary
conditions held rigidly constant, so whatever it does is a property of the
exchanger.

**Maintainer direction, 2026-09-27: the standalone steam generator lives in a
`tampines` library file and is tested here.** That is why both this harness
and the exchanger itself now live in this crate rather than in the example.

# The boundary conditions are DELIBERATELY steady

Every default is the published HTR-10 design point. Holding them constant is
the experiment: a steady exchanger at its design point must reach a steady
state and stay there, so **any drift is the exchanger's own**. Driven with a
transient instead, it would be impossible to say whether a cooling cold side
was the boundary condition or the model.

The feedwater enthalpy comes from the **forward** region-1 equation
`h_tp_1(T_feed, p_cold)`, not from a `(p,h)` inverse. Deliberate, given what
is being chased: the backward equations disagree with the forward ones by
~18 mK at this pressure (#342), and the harness must not inherit that
residual in its own boundary condition.

# The substep is the knob this harness exists to turn

[`SteamGeneratorConfig::substep`]'s own table records the arrays as
**unstable both above and below** a window, ~~with **0.05 s marked "fails …
enthalpy goes odd-even and clamps"** and 0.0125 s marked stable~~ and
0.0125 s marked stable. `htgr_sim_v1` ships 0.05 s.
[`SgStandaloneConfig::substep_s`] defaults to 0.0125 s and is settable, so
the two can be compared directly rather than argued about.

**CORRECTED 2026-09-27 — the 0.05 s "fails" row was stale and has been
struck.** This harness is what settled it:
[`tests::how_far_the_implicit_coupling_raises_the_stable_substep`] (244.81 s)
measures 0.05 s and ~~0.075 s~~ both **completing** (0.075 s panics since
2026-09-29, when the cold side's convection became bounded -- gh:#319; see
that test's re-measured table), at 1 coupling iteration and
at 8; 0.1 s panics. The row was written at 10:12 on 2026-08-13, 15 minutes
before commit `68e35551c2` put the helium side's energy convection on
`EnergyBalanceMode::Implicit` and removed the ceiling it was measuring, and
was never re-measured. So the shipped 0.05 s substep was never "in the
failing row" — the row was simply out of date. See that test's third finding
for the full timeline.

**Measured 2026-09-27, and the substep turned out NOT to be the cause of
#319** — the two agree on the coldest water node to **0.014 K** over 200 s of
steady operation and neither approaches the floor. See
[`tests::the_substep_is_not_what_walks_the_cold_side_down`] for the numbers and
for what that excludes. The exchanger alone is stable at its design point, so
#319's cause is in the plant coupling or in the transient the plant imposes.

# Determinism

No wall clock, no RNG, no I/O inside the loop. Same config in, byte-identical
trace out — [`tests::the_standalone_harness_is_deterministic`].

# Scope

Illustrative geometry and conductances, **not** a licensed design's, and not
validated against a measured HTR-10 steam generator. Research, education and
V&V only.

```rust
pub mod helical_coil_sg_standalone { /* ... */ }
```

### Types

#### Struct `SgStandaloneConfig`

How to run the standalone exchanger.

```rust
pub struct SgStandaloneConfig {
    pub steps: usize,
    pub caller_dt_s: f64,
    pub substep_s: f64,
    pub sample_every: usize,
    pub hot_inlet_k: f64,
    pub hot_flow_kg_per_s: f64,
    pub feedwater_k: f64,
    pub cold_flow_kg_per_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `steps` | `usize` | Number of caller timesteps to advance. |
| `caller_dt_s` | `f64` | Caller timestep \[s\] — what a plant would hand `advance_timestep`. |
| `substep_s` | `f64` | The **array** substep \[s\]. See [`SHIPPED_SUBSTEP_S`] and<br>[`STABLE_SUBSTEP_S`]. |
| `sample_every` | `usize` | Emit one row every `sample_every` steps. |
| `hot_inlet_k` | `f64` | Hot-side (helium) inlet temperature \[K\]. |
| `hot_flow_kg_per_s` | `f64` | Hot-side mass flow \[kg/s\]. |
| `feedwater_k` | `f64` | Cold-side (feedwater) inlet temperature \[K\]. |
| `cold_flow_kg_per_s` | `f64` | Cold-side mass flow \[kg/s\]. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    The published HTR-10 design point, at the **stable** substep.

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `SgTraceRow`

One sampled row.

```rust
pub struct SgTraceRow {
    pub step: usize,
    pub time_s: f64,
    pub hot_duty_w: f64,
    pub cold_duty_w: f64,
    pub hot_outlet_k: f64,
    pub cold_outlet_k: f64,
    pub min_cold_k: f64,
    pub min_metal_k: f64,
    pub worst_cross_k: f64,
    pub odd_even_k: f64,
    pub coupling_iterations: usize,
    pub coupling_residual_k: f64,
    pub cold_nodes_k: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `step` | `usize` | Step index. |
| `time_s` | `f64` | Elapsed time \[s\]. |
| `hot_duty_w` | `f64` | Hot-side duty \[W\]. |
| `cold_duty_w` | `f64` | Cold-side duty \[W\]. |
| `hot_outlet_k` | `f64` | Helium outlet temperature \[K\]. |
| `cold_outlet_k` | `f64` | Steam outlet temperature \[K\]. |
| `min_cold_k` | `f64` | **Coldest water/steam node** \[K\] — the quantity this harness watches. |
| `min_metal_k` | `f64` | Coldest tube-metal node \[K\]. The SS304L correlation's lower bound is<br>300 K and is its own failure mode. |
| `worst_cross_k` | `f64` | Worst node temperature cross \[K\]. |
| `odd_even_k` | `f64` | **Odd-even roughness** of the cold-node profile \[K\] — see<br>[`odd_even_roughness`]. The signature the stability table describes. |
| `coupling_iterations` | `usize` | Lateral-coupling (Picard) iterations the step used. |
| `coupling_residual_k` | `f64` | Coupling convergence residual \[K\]. |
| `cold_nodes_k` | `Vec<f64>` | Every cold-side node temperature \[K\]. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `SgStop`

Why a run stopped.

```rust
pub enum SgStop {
    Completed,
    Error {
        step: usize,
        message: String,
    },
    NearIf97Floor {
        step: usize,
        min_cold_k: f64,
    },
}
```

##### Variants

###### `Completed`

Ran every requested step.

###### `Error`

The exchanger returned an error.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `step` | `usize` | Step at which it failed. |
| `message` | `String` | The exchanger's own message. |

###### `NearIf97Floor`

A water/steam node came within [`FLOOR_MARGIN_K`] of the IF97 floor.
Detected **before** the steam tables are asked anything further, so the
harness reports the state instead of dying inside a flash.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `step` | `usize` | Step at which it happened. |
| `min_cold_k` | `f64` | The coldest node \[K\]. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `htr10_illustrative_config`

The exchanger configuration the harness drives — the HTR-10-illustrative one.

Mirrors `htgr_sim_v1`'s `primary_loop::steam_generator_config` field for
field, with the substep supplied so it can be swept. The `UA` of 4.26e4 W/K
and the 75 % hot-side resistance split are that simulator's illustrative
numbers.

```rust
pub fn htr10_illustrative_config(substep_s: f64, node_count: usize) -> crate::components::helical_coil_steam_generator::SteamGeneratorConfig { /* ... */ }
```

#### Function `odd_even_roughness`

A scalar measure of **odd-even (checkerboard) oscillation** in a node profile
\[K\].

The mean absolute second difference, `|T[i-1] - 2 T[i] + T[i+1]|`. A smooth
monotone profile — which a counter-flow exchanger at steady state must have —
gives a small number set by real curvature. A profile alternating node to
node gives a large one, and it grows as the oscillation does.

This is the quantity to watch rather than the minimum temperature alone,
because it distinguishes *how* the cold side is failing: a uniformly cooling
profile is an energy-balance error, whereas a checkerboard is a numerical
instability. ~~`SteamGeneratorConfig::substep`'s table names the second one
explicitly at 0.05 s.~~ **CORRECTED 2026-09-27** — that table row was stale
and has been struck: 0.05 s does **not** fail, verified by running
[`tests::how_far_the_implicit_coupling_raises_the_stable_substep`] (244.81 s;
0.05 s and 0.075 s both complete at 1 and at 8 coupling iterations, 0.1 s
panics). So there is currently **no measured substep at which this metric has
been shown to diagnose a checkerboard** — it is a sound instrument with no
calibrated reading yet. A 0.075 s run reported 75.3475 K (before
2026-09-29; 0.075 s panics under the bounded cold-side scheme, gh:#319), but with no
smooth-profile baseline for this 8-node dome-crossing cold side that number
cannot yet be attributed to oscillation rather than real curvature.

```rust
pub fn odd_even_roughness(profile: &[f64]) -> f64 { /* ... */ }
```

#### Function `run_with_iterations`

Drive the exchanger alone with an explicit coupling-iteration ceiling.

`1` is the old block-Jacobi single sweep; higher values are Picard on the
helium-tube-steam system. Exists so a sweep can compare the two on identical
boundary conditions rather than by argument.

```rust
pub fn run_with_iterations(cfg: &SgStandaloneConfig, max_coupling_iterations: usize) -> (Vec<SgTraceRow>, SgStop) { /* ... */ }
```

#### Function `run`

Drive the exchanger alone. Deterministic.

```rust
pub fn run(cfg: &SgStandaloneConfig) -> (Vec<SgTraceRow>, SgStop) { /* ... */ }
```

#### Function `coldest_over_run`

The coldest water/steam node reached over a whole run \[K\], with its stop
reason — the one-line summary a sweep wants.

```rust
pub fn coldest_over_run(cfg: &SgStandaloneConfig) -> (f64, f64, SgStop) { /* ... */ }
```

#### Function `feedwater_enthalpy_j_per_kg`

Feedwater enthalpy at the harness's cold-side conditions \[J/kg\], for
reporting.

```rust
pub fn feedwater_enthalpy_j_per_kg(cfg: &SgStandaloneConfig) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `IF97_MIN_TEMPERATURE_K`

IF97's lower temperature bound \[K\] — the triple point.

```rust
pub const IF97_MIN_TEMPERATURE_K: f64 = 273.15;
```

#### Constant `FLOOR_MARGIN_K`

Margin above the IF97 floor at which the harness stops \[K\].

Stopping *at* the floor is too late: the panic in #342 comes from a `(p,h)`
point whose **backward** temperature is ~18 mK below the floor while its
forward temperature sits on it, so a node at 273.16 K can already crash a
flash. One kelvin of margin catches the approach and leaves the state
readable instead of dying inside a property call.

```rust
pub const FLOOR_MARGIN_K: f64 = 1.0;
```

#### Constant `SHIPPED_SUBSTEP_S`

The array substep `htgr_sim_v1` actually ships \[s\].

`PLANT_TIMESTEP_S / STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP` = `0.1 / 2`.
Commit `c0e85a503e` (2026-08-13) cut that divisor from 8 to 2 to reach 4.1x
real time, moving the substep from 0.0125 s to this. **The exchanger's own
stability table marks this row "fails — enthalpy goes odd-even and clamps",**
and the module doc still describes 0.0125 s as "the shipped substep".

```rust
pub const SHIPPED_SUBSTEP_S: f64 = 0.05;
```

#### Constant `STABLE_SUBSTEP_S`

The array substep the exchanger's stability table records as stable \[s\].

`Co_hot = 0.222`, settled `Q_hot = 9.6854 MW`, +0.003 % off a 0.00625 s
reference.

```rust
pub const STABLE_SUBSTEP_S: f64 = 0.0125;
```

## Module `helical_coil_steam_generator`

**Helical-coil once-through steam generator** -- spatially resolved,
counter-flow, hot fluid <-> tube metal <-> water/steam.

# Provenance: moved here from `htgr_sim_v1` on 2026-09-27

This exchanger was written and lived in
`outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/steam_generator.rs`.
**Maintainer direction, 2026-09-27: it belongs in a `tampines` library
file, and is to be tested here.** It is thermal-hydraulic component physics,
which is what this crate is for, and the engine crate's own `CLAUDE.md` says
in the first place that no new physics belongs in *its* library -- so moving
it out satisfies both rules at once.

The move was **mechanical**: the 1 303 lines of production code carried
**zero** references to anything in the example (verified by grep for
`crate::physics` and `super::` across the whole of it), so nothing had to be
redesigned to make it portable. Only three things changed:

1. `use tampines::compressible::...` became `use crate::compressible::...`;
2. doc links into the example's modules were rewritten, since they no longer
   resolve from here;
3. the array substep, which the example held as a physics constant, is now
   [`DEFAULT_SUBSTEP_SECONDS`] here -- it is a property of *these arrays*,
   not of any particular plant, and keeping it beside them is what stops the
   two copies drifting.

The example now re-exports this module rather than owning a second copy.

# Why it was moved: it is the subject of an open defect

GitHub #319 -- `htgr_sim_v1`'s cold side reaches 273.15 K at 4 MPa and
panics the IF97 flash. The maintainer's objection is the one that matters:
**a 4 MPa cold side has no physical route to 273 K when feedwater enters at
313.15 K.** Feedwater enthalpy at the design point is ~171 kJ/kg; the
panicking cell sits at ~4 kJ/kg, a factor of 42 of energy unaccounted for.

Diagnosing that inside a whole plant is hopeless -- point kinetics, the
helium loop, outer correctors, a feedwater controller, a protection system
and a timestep accumulator all reach the exchanger. [`standalone`] drives
**this** exchanger with fixed boundary conditions and nothing else, so what
it does is a property of the exchanger. That harness is the reason the move
happened now.

# Scope

Illustrative geometry and conductances (see
[`SteamGeneratorGeometry::htr10_illustrative`]), **not** a licensed design's,
and not validated against a measured HTR-10 steam generator. Research,
education and V&V only.
Nodalised counter-flow steam generator: hot fluid <-> tube metal <-> water/steam.

A **spatially resolved** once-through steam generator, built by composing
three axial arrays that already exist in the workspace and coupling them
laterally through real thermal conductances:

```text
  hot fluid  (CompressibleFluidArray)   ---->  flows +x
       |  UA_hot / node
  tube metal (SolidColumn, SteelSS304L) ---- thermal mass, no flow
       |  UA_cold / node
  water/steam (TampinesSteamArray)      <----  flows -x  (COUNTER-FLOW)
```

## The defect this replaces

Until 2026-08-12 `htgr_sim_v1`'s primary loop modelled the secondary side as an
**isothermal sink at saturation** and took the duty from an
effectiveness-NTU lump:

```text
Q = eps * m_dot * c_p * (T_hot - T_sat)
```

That is the correct shape for an *evaporator*, where the cold side really is
isothermal. It is wrong for a **once-through** unit, which is an economiser,
an evaporator **and a superheater** in series: as the steam superheats, the
local driving temperature difference collapses toward zero, but against a
fixed saturation sink it never does. With helium near 973 K and saturation at
523 K the model saw a permanent ~450 K driver, over-predicted the duty, and
the steam outlet -- computed downstream as `h_feed + Q/m_dot` -- ran far too
hot. A `max_absorbable_duty` cap in [`super::secondary_loop`] then clamped the
outlet at the *hot-side inlet temperature*, which turned a crash into a
visibly wrong number.

Here the driving difference is evaluated **node by node at local
temperatures**, so the superheater's collapsing pinch is represented rather
than assumed away.

## No temperature cross, structurally

Lateral heat between any two adjacent arrays is

```text
q_i = UA_i * (T_upstream_array_i - T_this_array_i)
```

at the **local node temperatures** of the two arrays. If the cold stream ever
became hotter than the hot stream at some station, `q_i` simply **changes
sign** -- heat flows back the other way, which is what the second law
requires. Nothing is clamped, nothing is capped, and no branch tests for a
cross. This is the property the effectiveness-NTU lump could never have had,
because a lump has only one temperature per side and therefore no local
difference to take the sign of.

[`SteamGeneratorState::worst_node_cross_kelvin`] reports the measured margin.

## Counter-flow is expressed as an index mapping, not a negative flow

Both fluid arrays run **forward in their own frame** (positive mass flow,
inlet at cell 0). The counter-flow arrangement is carried by the *lateral
index map*: cold cell `j` sits at the same physical station as hot cell
`n-1-j`, so the temperature vector handed to each array is reversed on the
way across.

TUAS's own [`SimpleShellAndTubeHeatExchanger`] instead gives its shell side a
**negative** mass flowrate and swaps which terminal carries the inlet
boundary condition. Both are correct and they are the same exchanger; the
index map was chosen here because it keeps each array on the numerically
well-exercised forward-flow path, and because a counter-flow arrangement
genuinely *is* nothing more than a permutation of which node faces which.
[`tests::counter_flow_index_map_is_its_own_inverse`] pins the mapping.

[`SimpleShellAndTubeHeatExchanger`]:
    tuas_boussinesq_solver::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger

## The metal is a real thermal mass, and that is the point

The tube wall is a [`SolidColumn`] of [`SolidMaterial::SteelSS304L`], whose
mass is **derived from the tube geometry** and whose specific heat comes from
the TUAS solid-property database. It therefore has a real thermal time
constant, and a step change in duty cannot be tracked instantly by the steam
outlet. Before this module the secondary loop's only integrated state was its
mass flow; the metal is what makes it genuinely transient.

## What is real

- **Both fluids are real equations of state.** The hot side is a CoolProp
  Helmholtz EOS through [`CompressibleFluidArray`]; the cold side is
  IAPWS-IF97 through [`TampinesSteamArray`], which carries the water/steam
  phase change including the latent heat.
- **The metal is real steel.** Density and specific heat come from
  `SolidMaterial::SteelSS304L`, so the thermal capacity is derived, not typed
  in.
- **Both streams are advection-driven** through prescribed mass-flow inlets
  and TUAS upwind-advection junction terminals (see
  `crates/tampines-steam-tables/docs/boundary-conditions-convention.md`), so
  each stream develops a genuine axial temperature profile.
- **The energy balance closes** across all three arrays -- see
  [`tests::energy_balance_closes_across_the_exchanger`].

## What is illustrative

- **The conductances are a calibration, not a correlation.** `UA_hot` and
  `UA_cold` are constructor parameters. The HTGR caller supplies values whose
  *series* combination is the same 1.0e5 W/K the previous effectiveness-NTU
  lump used -- deliberately **not** re-tuned as part of this change, so any
  movement in the design point is attributable to the nodalisation and not to
  a fitted number. There is no Dittus-Boelter / Gnielinski evaluation per
  node here, and no helical-coil correlation.
- **The tube geometry is part published, part invented.** See
  [`SteamGeneratorGeometry::htr10_illustrative`].
- **The steam pressure is held fixed**, so there is no sliding-pressure or
  inventory dynamics; the cold array's outlet pressure is prescribed.
- **The node count is small** (see [`SteamGeneratorConfig::node_count`]), so
  this resolves *that the zones exist and where the pinch is*, not a
  converged axial profile.
- **The coupling is explicit (Lie-split), so the energy balance closes to
  about 0.34%, not to round-off.** Each array's lateral conductance is
  evaluated against its neighbours' previous sub-timestep temperatures, and
  the two fluid arrays treat that source differently from the implicit solid.
  The residual converges with the sub-timestep and is measured, not assumed:
  see [`tests::energy_balance_closes_across_the_exchanger`].

## Two hard limits you will hit before the physics gets interesting

Both are **panics in dependencies**, not errors this module can return, and
both are documented rather than guarded because guarding them would mean
silently continuing with a nonphysical state:

- **The tube metal is tabulated to 1000 K.** `SolidMaterial::SteelSS304L`
  carries properties to 726.85 degC and TUAS panics rather than
  extrapolating. At the HTR-10 design point the hottest metal node sits at
  **765.5 K**, a 235 K margin, and the reactor protection system's
  core-outlet trip is at 750 degC = 1023 K -- but the protection system is
  *disarmed by default* in this simulator, so a deliberate excursion with it
  off can reach the ceiling. Measured 2026-08-12: throttling the feedwater to
  1.0 kg/s at full power for 100 s does it.
- ~~**The water side is tabulated to 1073.15 K** (IAPWS-IF97 regions 1, 2, 4),
  and `tampines-steam-tables` panics rather than returning an error. The
  exchanger cannot itself drive the steam past the helium, so this is only
  reachable via a hot side above 800 degC.~~
  **CORRECTED 2026-09-27 — the ceiling is 2273.15 K, and it IS reachable
  without a hot helium side.** Two separate errors:
  1. **The ceiling moved.** `tampines-steam-tables` commit `2ab91fefc3`
     (2026-09-14) gave the `(p,h)` flash an IF97 **Region 5** arm, so the
     water side now runs to **2273.15 K**, not 1073.15 K. The panic message
     the exchanger actually produces says so verbatim: `(p,h) point lies
     above the 2273.15 K isotherm, the upper temperature bound of
     IAPWS-IF97 Region 5.` It still panics rather than erring — that half
     stands.
  2. **"only reachable via a hot side above 800 degC" is false**, and this is
     the half that matters. Verified by running
     [`super::helical_coil_sg_standalone::tests::how_far_the_implicit_coupling_raises_the_stable_substep`]
     (244.81 s): at the **steady design point**, with the hot side at its
     ordinary temperature, a **0.1 s substep** drives a cold cell past
     2273.15 K and hits exactly that panic — at 1 coupling iteration and at
     8 alike. A numerical instability, not a hot inlet, is what reaches the
     ceiling. Above Region 5 there is **no IF97 formulation at all**, so the
     panic is the correct behaviour; what was wrong was the claim about how
     one gets there.
  The Region 5 temperatures the flash returns *below* 2273.15 K are this
  crate's in-house Chebyshev fits, not IAPWS values — IAPWS publishes no
  Region 5 backward `(p,h)` equation. See
  `tampines-steam-tables`'s `ph_flash_eqm::t_ph_eqm`.

The plant's crash modal names the failing component, so either shows up as
"steam generator + secondary steam loop" rather than an anonymous panic.

## Cost -- this module *is* the simulator's cost

Each `advance_timestep` runs three coupled array solves per sub-timestep,
and each solve's outer correctors each run the array's equation of state
over every cell. Those EOS flashes -- a CoolProp Helmholtz solve on the hot
side, an IAPWS-IF97 `(p, h)` flash on the cold -- are where essentially all
of the wall clock goes.

**Measured 2026-08-13** (release, 8 nodes, a ~~the shipped 0.0125 s~~
**0.0125 s** substep, then 4 outer correctors): **1.9585 s of compute per second of
simulated time** on its own, against **1.9469** for the whole plant around
it. The exchanger is therefore about **96% of `htgr_sim_v1`'s compute**, and
the remaining 4% is everything else in the plant put together.

**CORRECTED 2026-09-27 — 0.0125 s is NOT "the shipped substep".** The shipped
substep is **0.05 s**: `htgr_sim_v1` computes it as `PLANT_TIMESTEP_S /
STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP` = `0.1 / 2`, verified at
`crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/mod.rs:641`
(`PLANT_TIMESTEP_S = 0.1`) and `:720`
(`STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP = 2`) — cited, not edited, that
file is outside this crate. The divisor was cut 8 → 2 by commit
`c0e85a503e` on 2026-08-13 at **10:59**, *after* the measurement above was
taken, and this sentence was never updated.

**Which way the error goes matters:** the figures above were taken at a
substep **4x smaller** than ships, and cost is very nearly linear in
`1/substep` (see the next bullet), so they are roughly a **4x
over**-estimate of the shipped exchanger's cost — and the "96% of compute"
share is likewise an over-estimate. **Nobody has re-measured the cost at
0.05 s**; that is stated as open rather than scaled by hand, because the
linearity was itself only measured over 0.0125-0.025 s. See
[`SteamGeneratorConfig::substep`] for the stability sweep that *was* re-run.

Two consequences, both important:

- **Its cost does not depend on the plant timestep.** It is a multi-rate
  sub-model on its own clock, so raising the plant step from 1 ms to 0.1 s
  moved the whole-plant real-time ratio only from 0.492 to 0.514.
- **Cost is very nearly exactly linear in `n_outer / substep`.** Measured
  the same day: 0.51 at (1 corrector, 0.0125 s), 0.99 at (2, 0.0125 s), 1.96
  at (4, 0.0125 s), 0.50 at (2, 0.025 s). Dropping the shipped corrector
  count from 4 to 2 halved the cost and moved the settled duty by nothing
  measurable -- see
  [`tests::the_corrector_substep_trade_is_measured`].

The substep cannot be raised further because of the hot side's **Courant**
limit, and more correctors do not lift it -- see
`tests::the_courant_number_bounds_the_array_substep`.

This is a demonstration model, **not a validated steam-generator model**.

## Promotion

This module is written to be moved into `tampines` unchanged (bead
`op-szmi.14`): it imports nothing from the example (`crate::`), knows nothing
about `HtgrSnapshot` or the GUI, and takes geometry, conductances, node count
and **both fluids** as constructor parameters. The hot side does not assume
helium -- `fhr_sim_v2` will pass a molten salt through the same
[`CoolPropFluid`] parameter.

```rust
pub mod helical_coil_steam_generator { /* ... */ }
```

### Types

#### Struct `SteamGeneratorGeometry`

Tube-bundle geometry of a once-through steam generator.

Everything here is an **aggregate over the whole unit** -- all modules, all
tubes -- because the model resolves the exchanger axially, not tube by tube.

Units are spelled out even though `uom` enforces them:

| Field | Quantity | Unit |
|---|---|---|
| `tube_count` | number of parallel tubes (dimensionless) | - |
| `tube_length` | developed length of **one** tube | m |
| `tube_inner_diameter` | tube bore | m |
| `tube_outer_diameter` | tube outside diameter | m |
| `shell_flow_area` | aggregate hot-side free-flow area | m^2 |
| `shell_flow_length` | hot-side flow path length through the unit | m |

Valid ranges: every field must be strictly positive, and
`tube_outer_diameter > tube_inner_diameter`. [`Self::validate`] checks this.

```rust
pub struct SteamGeneratorGeometry {
    pub tube_count: f64,
    pub tube_length: uom::si::f64::Length,
    pub tube_inner_diameter: uom::si::f64::Length,
    pub tube_outer_diameter: uom::si::f64::Length,
    pub shell_flow_area: uom::si::f64::Area,
    pub shell_flow_length: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tube_count` | `f64` | Number of parallel heat-transfer tubes in the whole unit. |
| `tube_length` | `uom::si::f64::Length` | Developed length of one tube \[m\]. For a helical-coil unit this is the<br>wound length, not the module height. |
| `tube_inner_diameter` | `uom::si::f64::Length` | Tube bore \[m\] -- the water/steam side. |
| `tube_outer_diameter` | `uom::si::f64::Length` | Tube outside diameter \[m\] -- the hot-fluid side. |
| `shell_flow_area` | `uom::si::f64::Area` | Aggregate free-flow area seen by the hot fluid \[m^2\]. |
| `shell_flow_length` | `uom::si::f64::Length` | Hot-fluid flow path length through the unit \[m\]. Distinct from<br>`tube_length` for a helical-coil unit, where the tubes are much longer<br>than the shell they are wound inside. |

##### Implementations

###### Methods

- ```rust
  pub fn htr10_illustrative() -> Self { /* ... */ }
  ```
  The HTR-10 steam generator, **part published and part invented**.

- ```rust
  pub fn tube_flow_area(self: &Self) -> Area { /* ... */ }
  ```
  Aggregate bore cross-section the water/steam flows through \[m^2\].

- ```rust
  pub fn metal_cross_section(self: &Self) -> Area { /* ... */ }
  ```
  Aggregate **metal** cross-section of the tube walls \[m^2\]: the annulus

- ```rust
  pub fn metal_volume(self: &Self) -> Volume { /* ... */ }
  ```
  Total volume of tube metal \[m^3\] = metal cross-section x tube length.

- ```rust
  pub fn metal_mass(self: &Self, material: SolidMaterial, temperature: ThermodynamicTemperature, pressure: Pressure) -> Mass { /* ... */ }
  ```
  Mass of tube metal \[kg\], **derived** from [`Self::metal_volume`] and the

- ```rust
  pub fn metal_thermal_capacity(self: &Self, material: SolidMaterial, temperature: ThermodynamicTemperature, pressure: Pressure) -> HeatCapacity { /* ... */ }
  ```
  Thermal capacity of the tube metal \[J/K\] = `m c_p`, both derived from

- ```rust
  pub fn outer_heat_transfer_area(self: &Self) -> Area { /* ... */ }
  ```
  Outside heat-transfer area \[m^2\] -- the hot-fluid-wetted surface,

- ```rust
  pub fn inner_heat_transfer_area(self: &Self) -> Area { /* ... */ }
  ```
  Inside heat-transfer area \[m^2\] -- the water/steam-wetted surface.

- ```rust
  pub fn validate(self: &Self) -> Result<(), SteamGeneratorError> { /* ... */ }
  ```
  `Ok(())` if the geometry is physically constructible: every dimension

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `SteamGeneratorConfig`

Everything needed to build a [`NodalisedCounterFlowSteamGenerator`].

Deliberately a plain parameter struct with no defaults sourced from any one
plant: the same exchanger serves the HTGR (helium hot side) and the FHR
(molten salt hot side), so nothing here may assume a working fluid.

```rust
pub struct SteamGeneratorConfig {
    pub geometry: SteamGeneratorGeometry,
    pub hot_fluid: crate::compressible::CoolPropFluid,
    pub hot_pressure: uom::si::f64::Pressure,
    pub cold_pressure: uom::si::f64::Pressure,
    pub metal: tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial,
    pub hot_side_conductance: uom::si::f64::ThermalConductance,
    pub cold_side_conductance: uom::si::f64::ThermalConductance,
    pub node_count: usize,
    pub substep: uom::si::f64::Time,
    pub max_coupling_iterations: usize,
    pub coupling_tolerance_kelvin: f64,
    pub initial_hot_end_temperature: uom::si::f64::ThermodynamicTemperature,
    pub initial_cold_end_temperature: uom::si::f64::ThermodynamicTemperature,
    pub initial_cold_outlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub hot_correctors: PimpleCorrectors,
    pub cold_correctors: PimpleCorrectors,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `SteamGeneratorGeometry` | Tube-bundle geometry. |
| `hot_fluid` | `crate::compressible::CoolPropFluid` | Hot-side working fluid. **Not assumed to be helium** -- this is the<br>parameter that lets the FHR pass a salt through the same exchanger. |
| `hot_pressure` | `uom::si::f64::Pressure` | Hot-side operating pressure \[Pa\]. Held fixed; the hot array's outlet<br>pressure is prescribed at this value. |
| `cold_pressure` | `uom::si::f64::Pressure` | Cold-side (water/steam) operating pressure \[Pa\]. Held fixed. |
| `metal` | `tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial` | Tube-wall material. |
| `hot_side_conductance` | `uom::si::f64::ThermalConductance` | Total conductance between the **hot fluid and the metal** \[W/K\], summed<br>over the whole exchanger. Divided equally between nodes internally. |
| `cold_side_conductance` | `uom::si::f64::ThermalConductance` | Total conductance between the **metal and the water/steam** \[W/K\]. |
| `node_count` | `usize` | Number of axial nodes. Must be at least 3 (the [`SolidColumn`] carries a<br>back node, a front node and at least one interior node). |
| `substep` | `uom::si::f64::Time` | The **fixed timestep the three arrays are advanced with** \[s\].<br><br>The exchanger runs on its own clock. Whatever `dt` the caller passes to<br>[`NodalisedCounterFlowSteamGenerator::advance_timestep`] is accumulated,<br>and the arrays are advanced in whole steps of exactly this size; any<br>remainder is carried to the next call, so no simulated time is lost or<br>double-counted.<br><br># Why a fixed step and not "at most this large"<br><br>**The arrays are unstable both above and below a window**, so a caller's<br>timestep cannot simply be subdivided or passed through. Measured<br>2026-08-12, with the duty column re-measured 2026-08-13 against a<br>0.00625 s reference:<br><br>| Array timestep | `Co_hot` | Settled `Q_hot` | Outcome |<br>|---|---|---|---|<br>| 0.1 s | 1.776 | -- | **fails** (also at 8 and 32 outer correctors) |<br>| ~~0.05 s~~ | ~~0.888~~ | ~~--~~ | ~~**fails** (also at 4, 8, 16 outer correctors); enthalpy goes odd-even and clamps~~ **CORRECTED 2026-09-27 — 0.05 s COMPLETES CLEANLY, and so does 0.075 s** |<br>| 0.025 s | 0.444 | 9.8244 MW | stable, but **+1.44%** off converged |<br>| **0.0125 s** | **0.222** | **9.6854 MW** | **stable, +0.003% off converged** |<br>| 0.00625 s | 0.111 | 9.6851 MW | reference |<br>| 0.001 s | 0.018 | -- | water side resolves its own acoustic transient; the IF97 `(p,h)` flash leaves Region 5 range and **panics** |<br><br>## ~~0.05 s fails~~ — **CORRECTED 2026-09-27**<br><br>**What was verified, by running rather than reading.**<br>[`super::helical_coil_sg_standalone::tests::how_far_the_implicit_coupling_raises_the_stable_substep`]<br>(`#[ignore]`d for cost; `cargo test --release -j 3 -p tampines --lib<br>how_far_the_implicit_coupling_raises_the_stable_substep -- --ignored<br>--nocapture`, **244.81 s**) sweeps the substep at the steady design point<br>with 1 coupling iteration (the old block-Jacobi behaviour) and with 8:<br><br>```text<br>substep  0.0125 s  | n=1: Completed coldest=320.2K iters=1 | n=8: Completed coldest=320.2K iters=3<br>substep  0.0250 s  | n=1: Completed coldest=320.2K iters=1 | n=8: Completed coldest=320.2K iters=4<br>substep  0.0500 s  | n=1: Completed coldest=320.2K iters=1 | n=8: Completed coldest=320.2K iters=4<br>substep  0.0750 s  | n=1: Completed coldest=320.2K iters=1 | n=8: Completed coldest=320.2K iters=6<br>substep  0.1000 s  | n=1: PANIC                            | n=8: PANIC<br>```<br><br>The 0.1 s row still fails, and fails the way the table says — the panic is<br>`(p,h) point lies above the 2273.15 K isotherm`, i.e. the cold side leaves<br>IF97 entirely, which is the odd-even blow-up's signature. **0.05 s and<br>0.075 s complete**, and complete even at `n=1`, so the coupling iterations<br>added on 2026-09-27 are *not* what rescued them.<br><br>**RE-MEASURED 2026-09-29 (gh:#319) — the 0.075 s row no longer holds.**<br>`TampinesSteamArray`'s energy convection moved that day from an<br>unbounded linear face value to a bounded van Leer scheme (the linear one<br>walked the cold side's inlet cell to the IF97 273.15 K floor). The same<br>sweep, 115.74 s, `cargo test --release -j2 -p tampines --lib<br>how_far_the_implicit_coupling_raises_the_stable_substep -- --ignored<br>--nocapture`:<br><br>```text<br>substep  0.0125 s  | n=1: Completed coldest=320.3K iters=1 | n=8: Completed coldest=320.3K iters=3<br>substep  0.0250 s  | n=1: Completed coldest=320.3K iters=1 | n=8: Completed coldest=320.4K iters=4<br>substep  0.0500 s  | n=1: Completed coldest=320.3K iters=1 | n=8: Completed coldest=320.3K iters=4<br>substep  0.0750 s  | n=1: PANIC                            | n=8: PANIC<br>substep  0.1000 s  | n=1: PANIC                            | n=8: PANIC<br>```<br><br>The ceiling moved from (0.075, 0.1) s to **(0.05, 0.075) s**. The<br>0.1 s panic is preceded by a drained-cell hold in vapour cell 6<br>(`rho_old = 9.59 kg/m3`, `rho_old - dt div(phi) = -2.67 kg/m3`): the<br>array's **explicit continuity** is over-draining a low-density cell, a<br>mass-Courant limit inside the array. 0.05 s, which `htgr_sim_v1`<br>ships, still completes.<br><br>**Why the row was wrong, confirmed from the history.** Three commits<br>landed within 47 minutes on 2026-08-13 (`git log -1 --date=iso`):<br><br>| Time | Commit | What it did |<br>|---|---|---|<br>| 10:12 | `3acc95362e` | **wrote this table**, including the `0.05 s … fails` row (`git show 3acc95362e` adds the line verbatim) |<br>| 10:27 | `68e35551c2` | put the helium side's energy convection on **`EnergyBalanceMode::Implicit`** — 26 added lines, removing exactly the `Co < 1` ceiling the row was measuring |<br>| 10:59 | `c0e85a503e` | cut the substep divisor **8 → 2**, i.e. 0.0125 s → 0.05 s, on the strength of that |<br><br>So the row was measured **15 minutes before** the fix that made 0.05 s<br>viable, the caller was moved onto 0.05 s 32 minutes after the fix, and the<br>table was never re-measured. The row has been contradicted by the shipped<br>configuration ever since.<br><br>**The shipped substep is 0.05 s**, not the 0.0125 s this doc used to claim:<br>`htgr_sim_v1` computes it as `PLANT_TIMESTEP_S /<br>STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP` = `0.1 / 2`<br>(`crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/mod.rs:641`<br>and `:720` — cited, not edited; that file is outside this crate).<br><br>**Completing is NOT the same as being clean.** The corrected reading is<br>*"0.05 s and 0.075 s complete"*, **not** *"0.05 s and 0.075 s are good"*.<br>The sibling test<br>[`super::helical_coil_sg_standalone::tests::the_implicit_coupling_converges_and_beats_real_time_at_the_largest_stable_substep`]<br>runs 60 s at the 0.075 s substep and reports **`odd-even roughness =<br>75.3475 K`**<br>([`super::helical_coil_sg_standalone::odd_even_roughness`] — the mean<br>`|T[i-1] - 2 T[i] + T[i+1]|` over the cold nodes). **That number is<br>recorded, not interpreted**: no smooth-profile baseline has been measured<br>for this 8-node cold side, which crosses the saturation dome and so<br>carries large *genuine* curvature, and without one it cannot be said<br>whether 75 K is checkerboard or physics. Measuring that baseline is open<br>work; guessing which it is would be exactly the kind of<br>reason-from-the-answer this table already got wrong once.<br><br>**What is NOT claimed.** Nothing here re-measures the *accuracy* columns.<br>0.05 s completing is not 0.05 s being converged: the surviving<br>`0.025 s → +1.44%` row is evidence the duty is still moving at four times<br>the shipped step, so the shipped configuration is very likely *further*<br>from converged than +1.44%, and nobody has measured by how much. That is<br>an open question, not a resolved one.<br><br>The **upper** bound is a Courant limit on the hot gas side and is not<br>negotiable by raising the outer-corrector count -- see<br>[`PimpleCorrectors`] and<br>`tests::the_courant_number_bounds_the_array_substep`.<br><br>The **lower** bound used to be the one that bit in practice: until<br>2026-08-13 `htgr_sim_v1`'s GUI stepped its plant at **1 ms**, and handed<br>straight to the arrays that is the panicking case -- which is what the<br>simulator did on its first wiring, dying in the crash modal within 30 s<br>of launch. The plant now steps at<br>the caller's plant timestep = 0.1 s, which is above the window<br>rather than below it, so the accumulator is now protecting against the<br>*upper* bound; either way it is needed.<br><br>Accumulating to a fixed substep makes the exchanger a **multi-rate**<br>sub-model, which is also what its physics wants: nothing in it moves on a<br>millisecond scale, the fastest thing being the shell transport at a few<br>hundred milliseconds and the slowest the 38 s tube metal. It is also what<br>makes the exchanger's cost independent of the plant timestep -- and,<br>since it is 96% of that cost, what made raising the plant timestep worth<br>so little on its own. |
| `max_coupling_iterations` | `usize` | **Maximum lateral-coupling (Picard) iterations per substep.**<br><br># Why this exists: the coupling used to be explicit, and that is what<br># capped the timestep<br><br>The three arrays are linked by<br>`lateral_link_new_temperature_vector_avg_conductance`, and TUAS puts that<br>conductance **on the coefficient matrix diagonal** while the *neighbour's*<br>temperature enters as a **source term** from a snapshot<br>(`calculation.rs`: `coefficient_matrix[[i,i]] += sum_of_lateral_conductances[i]`<br>against `power_source_vector[i] = sum_of_lateral_conductance_times_lateral_temperatures[i]`).<br>Each array is therefore implicit in **its own** temperature and explicit<br>in its neighbour's — **block Jacobi**, one sweep per substep.<br><br>That lag is one thing bounding the stable timestep, and it is why raising<br>the PIMPLE outer-corrector count never bought a larger one: those<br>correctors iterate *inside* one array and never touch the coupling<br>*between* arrays.<br><br>~~Measured before this change: 0.05 s "fails -- enthalpy goes odd-even and<br>clamps", and 0.05 s panicked at 4, 8 and 16 correctors alike.~~<br>**CORRECTED 2026-09-27** — that quote was repeated here from<br>[`SteamGeneratorConfig::substep`]'s table, which had been stale since<br>2026-08-13. **It was never the "before this change" state.** Verified by<br>running the sweep this doc cites<br>([`super::helical_coil_sg_standalone::tests::how_far_the_implicit_coupling_raises_the_stable_substep`],<br>244.81 s): at **1** coupling iteration — which *is* the old single-sweep<br>block Jacobi — 0.05 s and 0.075 s both complete, `coldest = 320.2 K`,<br>`iters = 1`. So the coupling lag was **not** what stopped 0.05 s, and<br>this mechanism must not be credited with having rescued it. What the<br>iterations measurably do buy is recorded in the sweep's own output<br>(`iters` rises 3 → 4 → 4 → 6 as the substep grows, i.e. the coupling<br>residual genuinely needs more sweeps at a larger step); what bounds the<br>substep between 0.075 s and 0.1 s is **not identified**, and both counts<br>fail at 0.1 s alike.<br><br># What iterating buys<br><br>Repeating the sweep, each time restarting from the **start-of-substep**<br>state and re-linking from the **latest** iterate, is block Gauss-Seidel /<br>Picard on the helium-tube-steam system. At convergence the lagged<br>neighbour temperature equals the new one, which **is** the solution of the<br>monolithic implicit three-way matrix — without assembling one, and reusing<br>every array's own solver unchanged.<br><br>Maintainer direction, 2026-09-27: the exchanger is to run at the caller's<br>0.1 s timestep with **no substepping**, via an implicit<br>helium-tube-steam solve. This is that solve.<br><br>`1` reproduces the old single-sweep Jacobi behaviour exactly, which is how<br>the before/after comparison is made. |
| `coupling_tolerance_kelvin` | `f64` | Convergence tolerance for the coupling iteration \[K\]: the largest node<br>temperature change between successive iterates that counts as converged.<br><br>Compared against the **maximum over all three arrays and all nodes**, so<br>it is a worst-node criterion rather than an average. A residual this test<br>cannot reach within [`Self::max_coupling_iterations`] is reported on<br>[`SteamGeneratorState::coupling_residual_kelvin`] rather than silently<br>accepted -- a non-converged step must be visible. |
| `initial_hot_end_temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature the **hot-inlet end** of the exchanger is seeded at.<br><br>All three arrays are seeded on one linear *station* profile running from<br>this value at station 0 to [`Self::initial_cold_end_temperature`] at<br>station `n-1`. Seeding all three on the same profile means every node<br>starts at its neighbours' temperature, so the exchanger opens at zero<br>transfer with no temperature cross and no start-up shock. A uniform seed<br>instead slams a 450 K step onto the hot inlet cell, which at a gas<br>array's acoustic timescale is a genuine numerical transient rather than a<br>physical one. |
| `initial_cold_end_temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature the **cold-inlet end** of the exchanger is seeded at. See<br>[`Self::initial_hot_end_temperature`]. |
| `initial_cold_outlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature the **cold stream's own outlet** is seeded at -- its state at<br>station 0, the hot-inlet end.<br><br>This is a third seed rather than a reuse of<br>[`Self::initial_hot_end_temperature`] because the two streams do **not**<br>meet at the hot end: the whole point of a counter-flow exchanger is that<br>the cold stream leaves *below* the hot stream's inlet, by the hot-end<br>approach. Seeding the cold stream at the hot stream's temperature makes<br>the exchanger open with steam as hot as the helium, which is a start-up<br>artefact that looks exactly like the defect this module was built to<br>remove. Measured 2026-08-12: seeding the two streams together drove the<br>downstream second-law backstop to 99.6% utilisation on the very first<br>plant step; seeding them apart drops it well clear. |
| `hot_correctors` | `PimpleCorrectors` | PIMPLE corrector counts and under-relaxation for the **hot** fluid<br>array. |
| `cold_correctors` | `PimpleCorrectors` | PIMPLE corrector counts and under-relaxation for the **cold**<br>(water/steam) array. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `PimpleCorrectors`

PIMPLE outer/inner corrector counts and under-relaxation factors for one
fluid array.

These used to be hardcoded inside
[`NodalisedCounterFlowSteamGenerator::new`]. They are configuration now
because the **outer** corrector count is the knob that trades wall-clock
cost against the largest usable [`SteamGeneratorConfig::substep`], and that
trade is the whole of this exchanger's cost: it is ~96% of `htgr_sim_v1`'s
plant compute (measured 2026-08-13, see
[`super::PLANT_TIMESTEP_S`]).

# What each does

- `n_outer` -- outer (PIMPLE/SIMPLE-like) correctors per array timestep.
  Each one re-solves momentum, pressure and energy from the same old-time
  state with the latest iterate. Because ~~both arrays carry~~ the cold
  array carries (**CORRECTED 2026-09-29**: the hot array has been
  `EnergyBalanceMode::Implicit` since 2026-08-13, and the cold array's
  source was an unlimited linear `fvc::div` until 2026-09-29, gh:#319) its
  enthalpy convection as an **explicit** `fvc::div_limited` source inside this loop,
  the loop is a Picard iteration whose contraction factor is the cell
  Courant number `Co`: residual reduction over the step is roughly
  `Co^n_outer`. So raising `n_outer` **can** buy a larger substep while
  `Co < 1`, and cannot help at all once `Co >= 1`.
- `n_inner` -- inner pressure correctors within each outer corrector (the
  PISO loop). These fix the pressure-velocity coupling, not the advection,
  and are not the Courant knob.
- `pressure_relaxation` / `velocity_relaxation` -- outer-loop
  under-relaxation, in `(0, 1]`. Lower is more robust and slower to
  converge.

# Cost

Each outer corrector costs one full pass of the array's equation of state
over every cell, and the EOS flashes (CoolProp Helmholtz on the hot side,
IAPWS-IF97 on the cold) are what this model spends its time in. Cost is
therefore very close to linear in `n_outer * substeps_per_second`, which is
why `n_outer` and `substep` trade against each other almost exactly.
[`tests::the_corrector_substep_trade_is_measured`] records the measured
grid.

```rust
pub struct PimpleCorrectors {
    pub n_outer: usize,
    pub n_inner: usize,
    pub pressure_relaxation: f64,
    pub velocity_relaxation: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_outer` | `usize` | Outer correctors per array timestep. Clamped to at least 1 by the array. |
| `n_inner` | `usize` | Inner (PISO) pressure correctors per outer corrector. |
| `pressure_relaxation` | `f64` | Outer-loop pressure under-relaxation factor, `(0, 1]`. |
| `velocity_relaxation` | `f64` | Outer-loop velocity under-relaxation factor, `(0, 1]`. |

##### Implementations

###### Methods

- ```rust
  pub fn hot_gas_default() -> Self { /* ... */ }
  ```
  Hot-gas-array settings: **2** outer correctors, 2 inner, both

- ```rust
  pub fn water_steam_default() -> Self { /* ... */ }
  ```
  Water/steam-array settings: **2** outer correctors, 2 inner, both

- ```rust
  pub fn with_outer(self: Self, n_outer: usize) -> Self { /* ... */ }
  ```
  The same settings with a different outer-corrector count.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `SteamGeneratorError`

Things that can go wrong building or driving the exchanger.

```rust
pub enum SteamGeneratorError {
    NonPositiveGeometry,
    TubeWallNotPositive,
    TooFewNodes(usize),
    NonPositiveSubstep,
    Array(String),
}
```

##### Variants

###### `NonPositiveGeometry`

A geometric dimension was zero or negative.

###### `TubeWallNotPositive`

The tube outside diameter did not exceed the bore.

###### `TooFewNodes`

Fewer than 3 axial nodes were requested.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `NonPositiveSubstep`

`substep` was zero, negative or non-finite.

###### `Array`

One of the composed arrays refused to build or advance. Carries the
upstream message, because the three arrays have three unrelated error
types with no conversions between them.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `SteamGeneratorState`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: \"dead_code\", kind: Allow}])]")`

What one [`NodalisedCounterFlowSteamGenerator::advance_timestep`] produced.

All temperatures are in kelvin and all powers in watts, `uom`-typed. The node
vectors are all in **hot-side index order**: element 0 is the hot inlet end,
element `n-1` the hot outlet end. The cold stream's own inlet is therefore at
element `n-1`.

```rust
pub struct SteamGeneratorState {
    pub hot_side_duty: uom::si::f64::Power,
    pub cold_side_duty: uom::si::f64::Power,
    pub hot_outlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub cold_outlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub cold_outlet_enthalpy: uom::si::f64::AvailableEnergy,
    pub hot_node_temperatures: Vec<uom::si::f64::ThermodynamicTemperature>,
    pub metal_node_temperatures: Vec<uom::si::f64::ThermodynamicTemperature>,
    pub cold_node_temperatures: Vec<uom::si::f64::ThermodynamicTemperature>,
    pub coupling_iterations: usize,
    pub coupling_residual_kelvin: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hot_side_duty` | `uom::si::f64::Power` | Heat rate leaving the hot fluid, `m_dot (h_in - h_out)` on the hot side.<br>This is what the hot loop is cooled by. |
| `cold_side_duty` | `uom::si::f64::Power` | Heat rate entering the water/steam, `m_dot (h_out - h_in)` on the cold<br>side. This is what the steam cycle absorbs.<br><br>It differs from [`Self::hot_side_duty`] by the rate of change of energy<br>stored in the tube metal -- which is exactly the transient behaviour the<br>metal exists to provide, not a bookkeeping error. |
| `hot_outlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | Hot-fluid outlet temperature. |
| `cold_outlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | Water/steam outlet temperature -- the live steam temperature. |
| `cold_outlet_enthalpy` | `uom::si::f64::AvailableEnergy` | Water/steam outlet specific enthalpy. |
| `hot_node_temperatures` | `Vec<uom::si::f64::ThermodynamicTemperature>` | Hot-fluid node temperatures, hot-inlet-first. |
| `metal_node_temperatures` | `Vec<uom::si::f64::ThermodynamicTemperature>` | Tube-metal node temperatures, hot-inlet-first. |
| `cold_node_temperatures` | `Vec<uom::si::f64::ThermodynamicTemperature>` | Water/steam node temperatures **re-indexed into hot-side order**, so<br>`cold_node_temperatures[i]` is at the same physical station as<br>`hot_node_temperatures[i]`. |
| `coupling_iterations` | `usize` | How many lateral-coupling (Picard) iterations the last substep used.<br><br>`1` means the first sweep already met<br>[`SteamGeneratorConfig::coupling_tolerance_kelvin`]. Equal to<br>[`SteamGeneratorConfig::max_coupling_iterations`] means it ran out of<br>iterations -- check [`Self::coupling_residual_kelvin`] before trusting the<br>step. |
| `coupling_residual_kelvin` | `f64` | Largest node temperature change \[K\] between the last two coupling<br>iterates, over all three arrays.<br><br>The convergence residual. Reported rather than asserted so a caller can<br>see a step that did not converge instead of being told nothing. |

##### Implementations

###### Methods

- ```rust
  pub fn node_driving_differences_kelvin(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  Local driving temperature difference at each station \[K\],

- ```rust
  pub fn worst_node_cross_kelvin(self: &Self) -> f64 { /* ... */ }
  ```
  Largest **cross** over all nodes \[K\]: `max(0, max_i(T_cold,i - T_hot,i))`.

- ```rust
  pub fn hot_end_driving_difference_kelvin(self: &Self) -> f64 { /* ... */ }
  ```
  Driving temperature difference at the **hot inlet** end \[K\] -- the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `NodalisedCounterFlowSteamGenerator`

A nodalised counter-flow once-through steam generator.

See the module docs for the arrangement, the no-cross argument and what is
real versus illustrative. Build with [`Self::new`], drive with
[`Self::advance_timestep`].

```rust
pub struct NodalisedCounterFlowSteamGenerator {
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
  pub fn new(config: SteamGeneratorConfig) -> Result<Self, SteamGeneratorError> { /* ... */ }
  ```
  Build the exchanger and seed all three arrays at

- ```rust
  pub fn node_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of axial nodes.

- ```rust
  pub fn geometry(self: &Self) -> SteamGeneratorGeometry { /* ... */ }
  ```
  The geometry this exchanger was built from.

- ```rust
  pub fn metal_thermal_capacity(self: &Self, temperature: ThermodynamicTemperature) -> HeatCapacity { /* ... */ }
  ```
  Tube-metal thermal capacity \[J/K\], derived from the geometry and the

- ```rust
  pub fn overall_conductance(self: &Self) -> ThermalConductance { /* ... */ }
  ```
  Series overall conductance `UA` \[W/K\]: the hot-side and cold-side

- ```rust
  pub fn metal_time_constant(self: &Self, temperature: ThermodynamicTemperature) -> Time { /* ... */ }
  ```
  Metal thermal time constant \[s\], `tau = C_metal / UA_series`, evaluated

- ```rust
  pub fn state(self: &Self) -> &SteamGeneratorState { /* ... */ }
  ```
  The most recently computed state, without stepping.

- ```rust
  pub fn max_courant_numbers(self: &Self, dt: Time) -> (f64, f64) { /* ... */ }
  ```
  Advective Courant numbers `Co = |u| dt / dx` on the two fluid arrays at

- ```rust
  pub fn hot_enthalpy_clamp_events(self: &Self) -> usize { /* ... */ }
  ```
  Number of times either fluid array has clamped its enthalpy field

- ```rust
  pub fn advance_timestep(self: &mut Self, dt: Time, hot_inlet_temperature: ThermodynamicTemperature, hot_mass_flow: MassRate, cold_inlet_enthalpy: AvailableEnergy, cold_mass_flow: MassRate) -> Result<SteamGeneratorState, SteamGeneratorError> { /* ... */ }
  ```
  Advance the exchanger by `dt`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `junction`

Junctions joining 1-D pipes: the RELAP-7 FlowJunction and VolumeBranch
(gh:#1071, RELAP-7 parity epic gh:#1056).

The framework lives beside the HEM pipe solver in
`tampines_steam_tables::hem::network`, because it drives `HemPipe`'s
boundaries and steps; it is re-exported here so balance-of-plant
components (pump, valve, turbine, separator, downcomer, wet well: gh:#1072
to gh:#1075) find it where they live. Run it with the run-time closure
[`crate::hem::HemEos`], e.g. `HemNetwork<HemEos>`.

See the source module's doc for the method and the V&V record,
`crates/tampines-steam-tables/verification_and_validation/hem_network_junctions.md`.

```rust
pub mod junction { /* ... */ }
```

### Re-exports

#### Re-export `HemJunction`

```rust
pub use tampines_steam_tables::hem::HemJunction;
```

#### Re-export `HemNetwork`

```rust
pub use tampines_steam_tables::hem::HemNetwork;
```

#### Re-export `PipeEndRef`

```rust
pub use tampines_steam_tables::hem::PipeEndRef;
```

#### Re-export `PortDraw`

```rust
pub use tampines_steam_tables::hem::PortDraw;
```

#### Re-export `PumpHead`

```rust
pub use tampines_steam_tables::hem::PumpHead;
```

#### Re-export `TdvThermo`

```rust
pub use tampines_steam_tables::hem::TdvThermo;
```

#### Re-export `TurbineParams`

```rust
pub use tampines_steam_tables::hem::TurbineParams;
```

#### Re-export `TimeDependentBoundary`

```rust
pub use tampines_steam_tables::hem::TimeDependentBoundary;
```

#### Re-export `TimeTable`

```rust
pub use tampines_steam_tables::hem::TimeTable;
```

#### Re-export `VolumeBranch`

```rust
pub use tampines_steam_tables::hem::VolumeBranch;
```

#### Re-export `VolumePort`

```rust
pub use tampines_steam_tables::hem::VolumePort;
```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `pump`

Pump component.

Composes [`outram_park_fork_dwsim_libs::pump::modes`]'s already-working
calculation-mode algebra (`PumpSpecification`/`PumpInlet`/`evaluate`)
behind a TAMPINES-native struct.

**For a transient pipe network, use [`super::junction::HemJunction::Pump`]
(gh:#1072, RELAP-7 parity epic gh:#1056)**, which runs inside a
[`super::junction::HemNetwork`] with its own V&V
(`tampines-steam-tables/verification_and_validation/hem_network_junctions.md`).
This struct is the steady pump sizing/rating wrapper and is separate from it.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `reactor_power`

Reactor power source (gh:#1079; RELAP-7 Theory Manual §6.16, cited, not
quoted).

> Research, education and V&V only. AI draft, unreviewed.

[`ReactorPowerSource`] takes a total reactor power and distributes it over
core channels by a radial weight and, within each channel, over its cells
by an [`AxialPowerShape`]. The total comes from a [`TotalPower`]: a table
in time, or a value the caller sets each step. **`tampines` does not
depend on `teh-o-prke`** (as `crate::pebble_bed` already states): power
from its `PointReactor` (fission plus decay heat) arrives through
[`TotalPower::External`], and the coupling lives in `nee_soon` or an
example.

The distribution is exact by construction: the chopped-cosine shape is
integrated cell by cell, and every set of fractions is normalised, so the
cell powers sum to the total to round-off. Power is deposited directly in
the fluid of a [`HemPipe`] through [`HemPipe::set_cell_heat_input`]; a heat
structure between fuel and coolant is not modelled here.

```rust
pub mod reactor_power { /* ... */ }
```

### Types

#### Enum `AxialPowerShape`

The axial shape of the power within one channel.

```rust
pub enum AxialPowerShape {
    Uniform,
    ChoppedCosine {
        extrapolated_length_ratio: f64,
    },
    Table(Vec<f64>),
}
```

##### Variants

###### `Uniform`

Equal power in every cell.

###### `ChoppedCosine`

`cos(π (z − H/2) / H_e)` over the heated length `H`, with the
extrapolated length `H_e = ratio · H` (`ratio ≥ 1`; 1 is the bare
cosine reaching zero at both ends).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `extrapolated_length_ratio` | `f64` | `H_e / H`. |

###### `Table`

A relative weight per cell (normalised; length must be the cell
count, values ≥ 0 with a positive sum).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<f64>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn cell_fractions(self: &Self, n_cells: usize) -> Result<Vec<f64>, HemError> { /* ... */ }
  ```
  The fraction of the channel's power in each of `n_cells` equal cells;

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `CoreChannelShare`

One core channel's share of the power.

```rust
pub struct CoreChannelShare {
    pub radial_weight: f64,
    pub axial_shape: AxialPowerShape,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `radial_weight` | `f64` | Relative radial weight (normalised over the channels; ≥ 0). |
| `axial_shape` | `AxialPowerShape` | Axial shape within the channel. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `TotalPower`

Where the total power comes from.

```rust
pub enum TotalPower {
    Table(tampines_steam_tables::hem::TimeTable),
    External(uom::si::f64::Power),
}
```

##### Variants

###### `Table`

A table of power \[W\] against time \[s\].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines_steam_tables::hem::TimeTable` |  |

###### `External`

A value the caller sets with [`ReactorPowerSource::set_external_power`]
each step, e.g. from `teh-o-prke`'s `PointReactor`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Power` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ReactorPowerSource`

A reactor power source over a set of core channels.

```rust
pub struct ReactorPowerSource {
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
  pub fn new(channels: Vec<CoreChannelShare>, total: TotalPower) -> Result<Self, HemError> { /* ... */ }
  ```
  A source over `channels`, driven by `total`.

- ```rust
  pub fn set_external_power(self: &mut Self, power: Power) { /* ... */ }
  ```
  Set the externally supplied total power (switches to

- ```rust
  pub fn total_power(self: &Self, t: Time) -> Power { /* ... */ }
  ```
  The total power at time `t`.

- ```rust
  pub fn channel_powers(self: &Self, t: Time) -> Vec<Power> { /* ... */ }
  ```
  Each channel's power at time `t`; they sum to the total.

- ```rust
  pub fn cell_powers(self: &Self, channel: usize, n_cells: usize, t: Time) -> Result<Vec<Power>, HemError> { /* ... */ }
  ```
  The power in each of channel `channel`'s `n_cells` cells at time `t`.

- ```rust
  pub fn apply_to_hem_pipe<E: HemEquationOfState>(self: &Self, channel: usize, t: Time, pipe: &mut HemPipe<E>) -> Result<(), HemError> { /* ... */ }
  ```
  Deposit channel `channel`'s power at time `t` in the fluid of `pipe`

- ```rust
  pub fn n_channels(self: &Self) -> usize { /* ... */ }
  ```
  The number of channels.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `turbine`

Steam turbine component.

Composes [`crate::hem::HemSteamCv`] (the inlet/outlet steam state) with
[`outram_park_fork_dwsim_libs::expander::isentropic`]'s adiabatic/Schultz
polytropic turbine model.

**For a transient pipe network, use [`super::junction::HemJunction::Turbine`]
(gh:#1072, RELAP-7 parity epic gh:#1056)**, which runs inside a
[`super::junction::HemNetwork`] with its own V&V
(`tampines-steam-tables/verification_and_validation/hem_network_junctions.md`).
This struct is the steady turbine sizing/rating wrapper and is separate from it.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `valve`

Control valve component.

Composes [`outram_park_fork_dwsim_libs::valve::iec_60534`]'s IEC 60534
sizing types behind a TAMPINES-native struct.

**For a transient pipe network, use [`super::junction::HemJunction::Valve`]
(gh:#1072, RELAP-7 parity epic gh:#1056)**, which runs inside a
[`super::junction::HemNetwork`] with its own V&V
(`tampines-steam-tables/verification_and_validation/hem_network_junctions.md`).
This struct is the steady valve sizing/rating wrapper and is separate from it.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `wet_well`

Wet well (pressure-suppression pool) component (gh:#1075; RELAP-7 Theory
Manual §6.12, Eqs 389–400, cited, not quoted).

> Research, education and V&V only. AI draft, unreviewed.

A 0-D suppression chamber: a **gas space** (nitrogen plus steam, ideal
gases at one temperature) above a **liquid pool** (well mixed, one
temperature), in a prismatic vessel of cross-section `A_c` and effective
height `L_t`. The pool level is a state: `L_w = m_w / (ρ_w A_c)` (the
inverse of RELAP-7 Eq 398), and the gas occupies `A_c (L_t − L_w)` (Eq
397), so the gas is compressed as the pool rises.

## Ported, not written fresh

**The physics is ported from `sembawang::bwr_mark1`** (the Fukushima
Unit 1 torus, MELCOR-based, clean room from the MELCOR 2.2 reference
manuals). `tampines` cannot depend on `sembawang` (it would pull the whole
source-term chain into the thermal-hydraulic framework, and `sembawang`
is meant to consume this component later, which would close a cycle), so
the logic is copied with its operation order kept, and every function
names its counterpart so the two cannot drift unnoticed.
`tests/wet_well.rs` checks the copies **bit for bit** against `sembawang`'s
public functions (a dev-dependency only). `sembawang` itself is unchanged.

| Here | Ported from (`sembawang::bwr_mark1`) |
|---|---|
| [`exchange`] (pool-surface mass and energy transfer, MELCOR CVH/FL RM §5.1) | `pool_surface::exchange` with its property tables, steam and N2 only |
| [`bubble_efficiency`] (MELCOR CVH/FL RM Eqs 6-1 to 6-3) | `pool_surface::bubble_efficiency` |
| the injection's bubble step | `containment::Containment::bubble_through` |
| flashing, surface exchange and rain-out | `containment::Containment::pool_surface_step` |
| the vacuum breaker | the `ww_atmosphere` vacuum-breaker branch of `containment::Containment::step` (step 5) |
| the immersed heat exchanger's implicit form | `containment::Containment::ww_shell_step` (its implicit two-capacity exchange, the coolant an infinite capacity) |
| N2 `c_v(T)` and internal energy (MELCOR NCG library) | `gas::cv_n2`, `gas::u_n2`, `gas::ncg_u`, `gas::ncg_c` |
| [`t_sat`], [`steam_h`] and the steam/water energy conventions | `pool_surface::t_sat`, `pool_surface::steam_h`, `containment::steam_u`, `CP_WATER` |
| the pool's density (IF97 region 1) | `sparc::Pool::rho_l` |

**New here, not in `sembawang`** (each stated): the pool level and the
moving gas volume (RELAP-7 Eqs 397–398); the injection port's elevation,
giving the submergence and the junction pressure (Eq 400); the split of a
two-phase injection into its liquid (to the pool) and its vapour (through
the bubble model) by IF97's saturation enthalpies at the junction
pressure; the liquid outlet (Eqs 391–392); the immersed heat exchanger
(the `q̇` of Eq 393); the vacuum breaker's direction and its external
volume as a fixed-state reservoir; the pipe-end pressures (Eqs 395, 399,
400); and the compression work `p ΔV` the pool does on the gas, which
RELAP-7 neglects (the remark after Eq 390) and `sembawang` has no need of
(its gas volume is fixed).

## Mechanisms: ON by default, ablations named `without_*`

| Default | Ablation | What the ablation does |
|---|---|---|
| MELCOR's pool surface (diffusion through the N2, natural convection, radiation) | [`WetWell::without_surface_mass_transfer`] | RELAP-7 Eq 390: sensible heat only, `A_c α (T_w − T_g)` with a given `α`, no mass transfer |
| MELCOR's bubble efficiency (part of the steam passes uncondensed within 5 K of saturation) | [`WetWell::without_bubble_pass_through`] | full interaction always (`ε = 1`) |
| N2 `c_v(T)` (MELCOR NCG fit) | [`WetWell::without_nitrogen_cv_t`] | constant 20.8 J/(mol K), `sembawang`'s ablation |
| the pool's compression work on the gas | [`WetWell::without_pressure_work`] | RELAP-7's omission |

## Energy conventions (ported, stated)

Internal energies are `sembawang`'s: liquid `c_p (T − 273.15)` with
`c_p = 4186 J/(kg K)`; steam `2.5060e6 + 1500 (T − 373.15)` J/kg (IF97's
saturated vapour at 100 °C with a constant `c_v`); N2 from 0 K. An
injection's specific enthalpy is the caller's (IF97, whose reference is the
liquid at the triple point, within a fraction of a J/kg of the liquid
reference here). Total energy is conserved exactly by construction; what
the conventions approximate is the temperature a given energy maps to.

## Not modelled (stated)

Hydrogen and other non-condensables (the gas is N2 and steam, RELAP-7's
assumption 4 plus steam); the gravity terms `(z − L_w/2) g` of Eq 393
(`g L_w ≈ 25 J/kg` against `h_fg ≈ 2.3e6 J/kg`); kinetic energy; pool
stratification (`sembawang`'s `pool_layers`); the shell as a heat
structure (`sembawang`'s `ww_shell`); swell and void in the pool;
aerosol scrubbing; the liquid outlet only draws (no inflow); the vessel
is prismatic.

```rust
pub mod wet_well { /* ... */ }
```

### Types

#### Struct `Atmosphere`

The gas space's bulk state, for the surface correlations
(`pool_surface::Atmosphere` without hydrogen).

```rust
pub struct Atmosphere {
    pub t_k: f64,
    pub p_pa: f64,
    pub n_steam: f64,
    pub n_n2: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_k` | `f64` | Temperature, K. |
| `p_pa` | `f64` | Total pressure, Pa. |
| `n_steam` | `f64` | Moles of steam. |
| `n_n2` | `f64` | Moles of nitrogen. |

##### Implementations

###### Methods

- ```rust
  pub fn viscosity_and_conductivity(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Mixture viscosity, Pa s (Wilke, Eq 5-2) and conductivity, W/(m K)

- ```rust
  pub fn mass_properties(self: &Self) -> (f64, f64, f64) { /* ... */ }
  ```
  Molar mass, kg/mol, density, kg/m³, and `c_p`, J/(kg K)

- ```rust
  pub fn steam_diffusivity(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Diffusivity of water vapour in N2, m²/s: MELCOR's steam-air

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `PoolState`

The pool's bulk state for the surface (`pool_surface::Pool`).

```rust
pub struct PoolState {
    pub t_k: f64,
    pub kg: f64,
    pub rho: f64,
    pub cp: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_k` | `f64` | Temperature, K. |
| `kg` | `f64` | Mass, kg. |
| `rho` | `f64` | Density, kg/m³. |
| `cp` | `f64` | Heat capacity, J/(kg K). |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `SurfaceGeometry`

Geometry of the surface: its area, m², and the depths of the pool and of
the gas space, m (`pool_surface::Surface`).

```rust
pub struct SurfaceGeometry {
    pub area_m2: f64,
    pub pool_depth_m: f64,
    pub gas_depth_m: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `area_m2` | `f64` | Area, m². |
| `pool_depth_m` | `f64` | Pool depth, m. |
| `gas_depth_m` | `f64` | Gas-space depth, m. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `SurfaceExchange`

The surface's result for one step (`pool_surface::Exchange`):
evaporation, kg/s (negative is condensation), the heat flows from the pool
and from the gas to the surface, W, and the surface temperature, K.

```rust
pub struct SurfaceExchange {
    pub evaporation_kg_s: f64,
    pub q_pool_w: f64,
    pub q_atm_w: f64,
    pub t_surface_k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `evaporation_kg_s` | `f64` | Evaporation, kg/s. |
| `q_pool_w` | `f64` | Heat from the pool to the surface, W. |
| `q_atm_w` | `f64` | Heat from the gas to the surface, W. |
| `t_surface_k` | `f64` | Surface temperature, K. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `VacuumBreakerDirection`

Which way the vacuum breaker passes gas. It is a check valve: it opens
only when the upstream side exceeds the downstream by its set point, and
never passes gas the other way.

The direction is the caller's choice, because the two installations
differ: a Mark I's wetwell-to-drywell breakers (and RELAP-7 Eq 389's
`ṁ_v`, the flow to the dry well) pass gas **out of** the wet well; its
reactor-building-to-torus breakers pass air **into** it.

```rust
pub enum VacuumBreakerDirection {
    IntoGasSpace,
    OutOfGasSpace,
}
```

##### Variants

###### `IntoGasSpace`

From the external volume into the gas space, when the external
pressure exceeds the gas space's by the set point.

###### `OutOfGasSpace`

From the gas space to the external volume, when the gas space's
pressure exceeds the external's by the set point (`sembawang`'s
wetwell-to-drywell breakers).

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `VacuumBreaker`

The vacuum breaker: a one-way orifice `C_d A √(2 ρ ΔP)`, with `ΔP` the
pressure difference beyond the set point and `ρ` the donor's density
(`sembawang`'s vacuum-breaker orifice).

```rust
pub struct VacuumBreaker {
    pub direction: VacuumBreakerDirection,
    pub opening_pressure_difference: uom::si::f64::Pressure,
    pub flow_area: uom::si::f64::Area,
    pub discharge_coefficient: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `direction` | `VacuumBreakerDirection` | Which way it passes gas. |
| `opening_pressure_difference` | `uom::si::f64::Pressure` | The pressure difference at which it opens. |
| `flow_area` | `uom::si::f64::Area` | Flow area. |
| `discharge_coefficient` | `f64` | Discharge coefficient (dimensionless). |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `WetWellGeometry`

The vessel's geometry: prismatic, of cross-section `A_c` and effective
height `L_t` (RELAP-7 Eqs 397–398), with the injection port's exit at
`z_i` above the pool bottom (Eq 400) and its opening height (MELCOR's
`h` in `ε_z`; `sembawang` uses a downcomer's diameter).

```rust
pub struct WetWellGeometry {
    pub cross_section: uom::si::f64::Area,
    pub total_height: uom::si::f64::Length,
    pub injection_elevation: uom::si::f64::Length,
    pub injection_opening: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cross_section` | `uom::si::f64::Area` | Cross-section `A_c`. |
| `total_height` | `uom::si::f64::Length` | Effective total height `L_t`. |
| `injection_elevation` | `uom::si::f64::Length` | Elevation of the injection port's exit above the pool bottom, `z_i`. |
| `injection_opening` | `uom::si::f64::Length` | The injection opening's height. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `WetWellInitialState`

The initial state (RELAP-7 initialises from the level, the gas pressure
and the gas temperature; the pool temperature and the gas's relative
humidity are added here).

```rust
pub struct WetWellInitialState {
    pub pool_level: uom::si::f64::Length,
    pub gas_pressure: uom::si::f64::Pressure,
    pub pool_temperature: uom::si::f64::ThermodynamicTemperature,
    pub gas_temperature: uom::si::f64::ThermodynamicTemperature,
    pub relative_humidity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pool_level` | `uom::si::f64::Length` | Pool level `L_w(0)`. |
| `gas_pressure` | `uom::si::f64::Pressure` | Gas-space pressure `p_g(0)`. |
| `pool_temperature` | `uom::si::f64::ThermodynamicTemperature` | Pool temperature. |
| `gas_temperature` | `uom::si::f64::ThermodynamicTemperature` | Gas temperature `T_g(0)`. |
| `relative_humidity` | `f64` | Steam's partial pressure over its saturation pressure at the gas<br>temperature, in [0, 1]; the rest of the pressure is nitrogen. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Injection`

Steam, two-phase mixture or water discharged at the injection port below
the pool surface (a relief-valve quencher, a downcomer, an RCIC exhaust),
optionally with nitrogen.

```rust
pub struct Injection {
    pub mass_rate: uom::si::f64::MassRate,
    pub specific_enthalpy: uom::si::f64::AvailableEnergy,
    pub nitrogen_mass_rate: uom::si::f64::MassRate,
    pub nitrogen_temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mass_rate` | `uom::si::f64::MassRate` | Water mass flow (any phase), ≥ 0. |
| `specific_enthalpy` | `uom::si::f64::AvailableEnergy` | Its specific enthalpy (IF97 reference). |
| `nitrogen_mass_rate` | `uom::si::f64::MassRate` | Nitrogen carried with it, ≥ 0. |
| `nitrogen_temperature` | `uom::si::f64::ThermodynamicTemperature` | The nitrogen's temperature (it arrives with its enthalpy). |

##### Implementations

###### Methods

- ```rust
  pub fn none() -> Self { /* ... */ }
  ```
  No injection.

- ```rust
  pub fn water(mass_rate: MassRate, specific_enthalpy: AvailableEnergy) -> Self { /* ... */ }
  ```
  Water only (steam, two-phase or liquid) at `h`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ExternalVolume`

The external volume on the far side of the vacuum breaker: a reservoir of
fixed pressure, temperature and composition (as RELAP-7's time-dependent
volume, §6.13), whatever passes through.

```rust
pub struct ExternalVolume {
    pub pressure: uom::si::f64::Pressure,
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub steam_mole_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `uom::si::f64::Pressure` | Pressure. |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature. |
| `steam_mole_fraction` | `f64` | Steam's mole fraction, in [0, 1); the rest is nitrogen. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ImmersedHeatExchanger`

The immersed heat exchanger: `q̇ = UA (T_pool − T_hx)` (the `q̇` of
RELAP-7 Eq 393), with the coolant at a fixed temperature.

```rust
pub struct ImmersedHeatExchanger {
    pub conductance: uom::si::f64::ThermalConductance,
    pub coolant_temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `conductance` | `uom::si::f64::ThermalConductance` | `UA`, ≥ 0. |
| `coolant_temperature` | `uom::si::f64::ThermodynamicTemperature` | The coolant's temperature `T_hx`. |

##### Implementations

###### Methods

- ```rust
  pub fn off() -> Self { /* ... */ }
  ```
  Not running.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `WetWellInputs`

What drives one step.

```rust
pub struct WetWellInputs {
    pub injection: Injection,
    pub external: ExternalVolume,
    pub heat_exchanger: ImmersedHeatExchanger,
    pub liquid_outflow: uom::si::f64::MassRate,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `injection` | `Injection` | The injection port. |
| `external` | `ExternalVolume` | The vacuum breaker's external volume. |
| `heat_exchanger` | `ImmersedHeatExchanger` | The immersed heat exchanger. |
| `liquid_outflow` | `uom::si::f64::MassRate` | Liquid drawn from the pool at its own enthalpy (RELAP-7 Eqs 391–392),<br>≥ 0. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `SurfaceModel`

The surface model in use (see [`WetWell::without_surface_mass_transfer`]).

```rust
pub enum SurfaceModel {
    Melcor,
    SensibleOnly {
        coefficient: uom::si::f64::HeatTransfer,
    },
}
```

##### Variants

###### `Melcor`

MELCOR's pool surface ([`exchange`]): the default.

###### `SensibleOnly`

RELAP-7 Eq 390: sensible heat `A_c α (T_w − T_g)` only.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `coefficient` | `uom::si::f64::HeatTransfer` | `α`. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `WetWellMechanisms`

Which mechanisms are on. All are on by default.

```rust
pub struct WetWellMechanisms {
    pub surface: SurfaceModel,
    pub bubble_pass_through: bool,
    pub nitrogen_cv_t: bool,
    pub pressure_work: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `surface` | `SurfaceModel` | The surface model. |
| `bubble_pass_through` | `bool` | MELCOR's bubble efficiency (else `ε = 1`). |
| `nitrogen_cv_t` | `bool` | N2 `c_v(T)` (else constant). |
| `pressure_work` | `bool` | The pool's compression work on the gas. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `WetWellStepReport`

What one step did.

```rust
pub struct WetWellStepReport {
    pub bubble_efficiency: f64,
    pub steam_to_gas: uom::si::f64::Mass,
    pub vacuum_breaker_flow: uom::si::f64::Mass,
    pub heat_exchanger_heat: uom::si::f64::Energy,
    pub surface_evaporation: uom::si::f64::Mass,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bubble_efficiency` | `f64` | The bubble efficiency at the injection port this step. |
| `steam_to_gas` | `uom::si::f64::Mass` | Injected steam that passed through the pool uncondensed (plus the<br>vapour carried by the injected nitrogen), into the gas space. |
| `vacuum_breaker_flow` | `uom::si::f64::Mass` | Gas through the vacuum breaker, positive into the gas space. |
| `heat_exchanger_heat` | `uom::si::f64::Energy` | Heat removed by the immersed heat exchanger. |
| `surface_evaporation` | `uom::si::f64::Mass` | Net evaporation at the pool surface (negative is condensation),<br>including flashing and rain-out. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `WetWellBookkeeping`

Running totals of what crossed the boundary since construction, for mass
and energy bookkeeping. `total_energy_now − initial = energy_in −
energy_out` holds to round-off.

```rust
pub struct WetWellBookkeeping {
    pub water_in: uom::si::f64::Mass,
    pub water_out: uom::si::f64::Mass,
    pub nitrogen_in: uom::si::f64::Mass,
    pub nitrogen_out: uom::si::f64::Mass,
    pub energy_in: uom::si::f64::Energy,
    pub energy_out: uom::si::f64::Energy,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `water_in` | `uom::si::f64::Mass` | Water in (injection, steam through the breaker). |
| `water_out` | `uom::si::f64::Mass` | Water out (breaker, liquid outlet). |
| `nitrogen_in` | `uom::si::f64::Mass` | Nitrogen in. |
| `nitrogen_out` | `uom::si::f64::Mass` | Nitrogen out. |
| `energy_in` | `uom::si::f64::Energy` | Energy in (injected enthalpy, breaker inflow's enthalpy). |
| `energy_out` | `uom::si::f64::Energy` | Energy out (breaker outflow's enthalpy, liquid outlet, heat<br>exchanger). |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `WetWell`

The wet well. See the module documentation.

```rust
pub struct WetWell {
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
  pub fn new(geometry: WetWellGeometry, breaker: VacuumBreaker, initial: WetWellInitialState) -> Result<Self, TampinesError> { /* ... */ }
  ```
  A wet well of `geometry` at `initial`, its vacuum breaker `breaker`,

- ```rust
  pub fn without_surface_mass_transfer(self: Self, coefficient: HeatTransfer) -> Self { /* ... */ }
  ```
  Ablation: RELAP-7 Eq 390's sensible heat `A_c α (T_w − T_g)` with the

- ```rust
  pub fn without_bubble_pass_through(self: Self) -> Self { /* ... */ }
  ```
  Ablation: full bubble interaction always (`ε = 1`), no steam passing

- ```rust
  pub fn without_nitrogen_cv_t(self: Self) -> Self { /* ... */ }
  ```
  Ablation: N2 at a constant 20.8 J/(mol K) (`sembawang`'s

- ```rust
  pub fn without_pressure_work(self: Self) -> Self { /* ... */ }
  ```
  Ablation: no compression work from the pool on the gas (RELAP-7's

- ```rust
  pub fn mechanisms(self: &Self) -> WetWellMechanisms { /* ... */ }
  ```
  The mechanisms in use.

- ```rust
  pub fn gas_pressure(self: &Self) -> Pressure { /* ... */ }
  ```
  Gas-space pressure `p_g`.

- ```rust
  pub fn pool_level(self: &Self) -> Length { /* ... */ }
  ```
  Pool level `L_w` above the bottom.

- ```rust
  pub fn pool_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Pool temperature.

- ```rust
  pub fn gas_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Gas temperature.

- ```rust
  pub fn pool_mass(self: &Self) -> Mass { /* ... */ }
  ```
  Pool mass.

- ```rust
  pub fn steam_mass(self: &Self) -> Mass { /* ... */ }
  ```
  Steam in the gas space.

- ```rust
  pub fn nitrogen_mass(self: &Self) -> Mass { /* ... */ }
  ```
  Nitrogen in the gas space.

- ```rust
  pub fn gas_volume(self: &Self) -> Volume { /* ... */ }
  ```
  Gas-space volume `A_c (L_t − L_w)` (RELAP-7 Eq 397).

- ```rust
  pub fn pool_density(self: &Self) -> uom::si::f64::MassDensity { /* ... */ }
  ```
  Pool density `m_w / (A_c L_w)` (Eq 398).

- ```rust
  pub fn pool_reference_pressure(self: &Self) -> Pressure { /* ... */ }
  ```
  The water space's reference pressure at mid-depth, `p_g + L_w ρ_w g /

- ```rust
  pub fn pressure_at_elevation(self: &Self, z: Length) -> Pressure { /* ... */ }
  ```
  The pressure at elevation `z` above the pool bottom: `p_w + (L_w/2 −

- ```rust
  pub fn pipe_end_pressure(self: &Self, z: Length, form_loss: f64, rho_u_squared: Pressure) -> Pressure { /* ... */ }
  ```
  The boundary pressure a connected pipe end at elevation `z` sees,

- ```rust
  pub fn water_mass(self: &Self) -> Mass { /* ... */ }
  ```
  Water in the component (pool plus gas-space steam).

- ```rust
  pub fn total_energy(self: &Self) -> Energy { /* ... */ }
  ```
  Total internal energy, gas plus pool, in this module's conventions.

- ```rust
  pub fn bookkeeping(self: &Self) -> WetWellBookkeeping { /* ... */ }
  ```
  Running totals across the boundary.

- ```rust
  pub fn step(self: &mut Self, dt: Time, inputs: &WetWellInputs) -> Result<WetWellStepReport, TampinesError> { /* ... */ }
  ```
  Advance by `dt`, in `sembawang`'s order: the injection through the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `cv_n2`

Molar `c_v` of N2, J/(mol K), held at the fit's ends; `t_dependent`
false is the constant ablation (`gas::cv_n2`).

```rust
pub fn cv_n2(t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `u_n2`

Molar internal energy of N2 from 0 K, J/mol (`gas::u_n2`).

```rust
pub fn u_n2(t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `p_sat_pa`

Saturation pressure, Pa (IF97 region 4; `steam::p_sat_pa`).

```rust
pub fn p_sat_pa(t_k: f64) -> f64 { /* ... */ }
```

#### Function `t_sat`

Saturation temperature at `p`, K, by bisection on IF97's saturation
pressure (`pool_surface::t_sat`, operation for operation).

```rust
pub fn t_sat(p: f64) -> f64 { /* ... */ }
```

#### Function `steam_h`

Steam's specific enthalpy, `u + R T / M`, J/kg (`pool_surface::steam_h`).

```rust
pub fn steam_h(t: f64) -> f64 { /* ... */ }
```

#### Function `exchange`

MELCOR's pool-surface exchange (CVH/FL RM §5.1.1–5.1.2) at the start of a
step of `dt` s, limited as MELCOR limits it (Eq 5-17; condensation at most
90 % of the vapour). **Port of `sembawang::bwr_mark1::pool_surface::exchange`**,
operation for operation with hydrogen removed; `tests/wet_well.rs` checks
it bit for bit against the original with no hydrogen.

The surface temperature satisfies, at once, vapour diffusion through the
N2 (Eq 5-1, `C` from the heat-mass analogy, Eq 5-2), the surface energy
balance (Eq 5-7: natural convection plus radiation on each side) and
saturation (Eq 5-5), solved by bisection. Without N2 the surface is at the
saturation temperature of the total pressure (Eq 5-6).

```rust
pub fn exchange(pool: &PoolState, atm: &Atmosphere, surface: &SurfaceGeometry, dt: f64) -> SurfaceExchange { /* ... */ }
```

#### Function `bubble_efficiency`

MELCOR's bubble efficiency at a vent below the pool (CVH/FL RM Eqs 6-1 to
6-3): `ε_z = (depth − 0.01 m) / h` with `h` the opening's height, and
`ε_T = (T_sat(P_j) − T_P − 0.1 K) / 5 K` with `P_j` the pressure at the
opening's depth, each clamped to [0, 1]. **Port of
`sembawang::bwr_mark1::pool_surface::bubble_efficiency`.**

```rust
pub fn bubble_efficiency(depth_m: f64, opening_m: f64, p_junction_pa: f64, t_pool_k: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `M_H2O`

Molar masses, kg/mol, and the gas constant, J/(mol K)
(`sembawang::bwr_mark1::params::{M_H2O, M_N2, R_GAS}`).

```rust
pub const M_H2O: f64 = 0.018_015;
```

#### Constant `M_N2`

Nitrogen's molar mass, kg/mol.

```rust
pub const M_N2: f64 = 0.028_014;
```

#### Constant `R_GAS`

The gas constant, J/(mol K).

```rust
pub const R_GAS: f64 = 8.314_462;
```

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

#### Re-export `ConductionZone`

```rust
pub use heat_structure::ConductionZone;
```

#### Re-export `HeatStructure`

```rust
pub use heat_structure::HeatStructure;
```

#### Re-export `HeatStructureLink`

```rust
pub use heat_structure::HeatStructureLink;
```

#### Re-export `SideCondition`

```rust
pub use heat_structure::SideCondition;
```

#### Re-export `StructureBoundary`

```rust
pub use heat_structure::StructureBoundary;
```

#### Re-export `StructureShape`

```rust
pub use heat_structure::StructureShape;
```

#### Re-export `StructureSide`

```rust
pub use heat_structure::StructureSide;
```

#### Re-export `ThermalCoupler`

```rust
pub use heat_structure::ThermalCoupler;
```

#### Re-export `NodalisedCounterFlowSteamGenerator`

```rust
pub use helical_coil_steam_generator::NodalisedCounterFlowSteamGenerator;
```

#### Re-export `SteamGeneratorConfig`

```rust
pub use helical_coil_steam_generator::SteamGeneratorConfig;
```

#### Re-export `SteamGeneratorError`

```rust
pub use helical_coil_steam_generator::SteamGeneratorError;
```

#### Re-export `SteamGeneratorGeometry`

```rust
pub use helical_coil_steam_generator::SteamGeneratorGeometry;
```

#### Re-export `SteamGeneratorState`

```rust
pub use helical_coil_steam_generator::SteamGeneratorState;
```

#### Re-export `HemJunction`

```rust
pub use junction::HemJunction;
```

#### Re-export `HemNetwork`

```rust
pub use junction::HemNetwork;
```

#### Re-export `PipeEndRef`

```rust
pub use junction::PipeEndRef;
```

#### Re-export `VolumeBranch`

```rust
pub use junction::VolumeBranch;
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

#### Re-export `AxialPowerShape`

```rust
pub use reactor_power::AxialPowerShape;
```

#### Re-export `CoreChannelShare`

```rust
pub use reactor_power::CoreChannelShare;
```

#### Re-export `ReactorPowerSource`

```rust
pub use reactor_power::ReactorPowerSource;
```

#### Re-export `TotalPower`

```rust
pub use reactor_power::TotalPower;
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

#### Re-export `ExternalVolume`

```rust
pub use wet_well::ExternalVolume;
```

#### Re-export `ImmersedHeatExchanger`

```rust
pub use wet_well::ImmersedHeatExchanger;
```

#### Re-export `Injection`

```rust
pub use wet_well::Injection;
```

#### Re-export `VacuumBreaker`

```rust
pub use wet_well::VacuumBreaker;
```

#### Re-export `VacuumBreakerDirection`

```rust
pub use wet_well::VacuumBreakerDirection;
```

#### Re-export `WetWell`

```rust
pub use wet_well::WetWell;
```

#### Re-export `WetWellGeometry`

```rust
pub use wet_well::WetWellGeometry;
```

#### Re-export `WetWellInitialState`

```rust
pub use wet_well::WetWellInitialState;
```

#### Re-export `WetWellInputs`

```rust
pub use wet_well::WetWellInputs;
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
- ~~**Marviken is NOT validated.** The digitised NUREG/CR-2671 test-23/24
  data sits in `marviken_tests.rs`, but the test is
  `#[ignore = "skip first, Marviken is more complex"]`, its only
  assertion is commented out, and the body ends in `todo!()`. Do not
  cite Marviken as a validation basis for these solvers.~~
  **CORRECTED 2026-10-16** — no longer true. `tampines-steam-tables`'
  `marviken_tests.rs` now holds six plain `#[test]`s with assertions and
  no `todo!()`, including `validate_against_marviken_test_23` and
  `_24` for the HEM dispatcher; read their doc comments for the recorded
  deviations before citing them. (Separately, this crate's
  `multiphase_1d::marviken_tests` validates the drift-flux marcher on
  test 23.)

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
    Hem(tampines_steam_tables::hem::HemError),
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

###### `Hem`

The HEM pipe solver ([`crate::hem::HemPipe`], which lives in
`tampines_steam_tables::hem`) failed. Its own error is kept whole, so a
`CourantViolation` or the cell of an `Unphysical` state survives the
conversion.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines_steam_tables::hem::HemError` |  |

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
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private21::Error + ''static> { /* ... */ }
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

  - ```rust
    fn from(source: tampines_steam_tables::hem::HemError) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `fem1d`

1-D continuous-Galerkin finite elements for HEM pipe flow, built to be
**compared against** the finite-volume default (gh:#1069).

> Research, education and V&V only. AI draft, unreviewed. Not for nuclear
> facility operation, reactor control, licensing or safety-critical
> decisions. See `RESPONSIBLE_USE.md`.

# What this answers

*On the same physics, the same equation of state and the same boundary
conditions, how does the RELAP-7 style continuous-Galerkin discretisation
compare with this workspace's finite-volume HEM pipe?* Maintainer decision:
flow is finite volume (OpenFOAM style) **by default**; this module exists so
the two can be measured side by side, not to replace it. The comparison and
its results are `verification_and_validation/fem1d_vs_fv.md`, the tests
`tests/fem1d_vs_fv.rs`.

# Entry points

- [`Discretisation`]: `FiniteVolume` (the default) or
  `FiniteElement(Stabilisation)`.
- [`Pipe1d`]: one enum over both solvers, so one case runs on either.
- [`FemPipe`]: the finite-element solver itself.
- [`Stabilisation`], [`FemNumerics`], [`MassMatrix`].

# The discretisation (RELAP-7 Theory Manual §5.1, Eqs 235–245)

Linear Lagrange elements on the uniform mesh of the
[`HemPipeGeometry`]: `n + 1` nodes at `x_j = jL/n`, so element and
finite-volume cell have the same width. The unknowns are the nodal
area-weighted conserved variables `V = [ρA, ρuA, ρEA]` (Eqs 243–245); the
test functions are the same hat functions (Eq 238). The Galerkin weak form
(Eqs 239–242, frictionless, adiabatic, horizontal) is, for every node `i`,

```text
∫ φ_i ∂V/∂t dx = ∫ [ρuA φ_i', ((ρu² + p)A φ_i' + p A' φ_i), ρuHA φ_i'] dx − [G·n̂ φ_i]_ends
```

evaluated with 2-point Gauss–Legendre quadrature per element (the line rule
of `farrer_park::quadrature`, exact to degree 3), the equation of state
closing `(ρ, e)` **at each quadrature point** from the interpolated `V`
and the analytic area there. `G` at an end is the boundary flux (below).

**Mass matrix:** [`MassMatrix::Lumped`] by default (the row sums,
`∫φ_j`, so `M` is diagonal); [`MassMatrix::Consistent`] solves the
tridiagonal `∫φ_iφ_j` system by the Thomas algorithm. Either way the
global totals `Σ_j m_j V_j = ∫V dx` are what the scheme conserves.

**Time stepping:** explicit SSP Runge–Kutta, by default
[`TimeIntegrator::Ssp33`] (`petir::ssprk`). Not SSP22, `HemPipe`'s default:
the Galerkin operator is a central difference, its eigenvalues lie on the
imaginary axis, and the two-stage scheme's stability region does not
contain any of the imaginary axis but the origin; the three-stage one
contains `|Im| ≤ √3`. Explicit rather than implicit (RELAP-7 uses BDF and
JFNK) because the comparison is against an explicit finite-volume scheme
at the same Courant number; an implicit FEM would compare time
integrators, not spatial discretisations. The step is
`Δt = C h / max(|u| + c)` over nodes and quadrature points, `C` the
classical Courant number ([`FemNumerics::courant`], default 0.4, which is
`HemPipe`'s default central Courant number 0.2 in classical terms), divided
by `√3` with the consistent mass matrix.

# Stabilisation (§5.4), chosen by [`Stabilisation`]

- [`Stabilisation::Supg`] — §5.4.1, Eqs 275–298: the weighting
  `Σ_K ∫ Aᵀ ∂W/∂x · τ R̃` with `τ = (h/2)|A|⁻¹`, `|A| = P|D|P⁻¹`, the
  quasi-linear residual `R̃ = V_t + A V_x + (F − AU)A' − S`. The flux
  Jacobian is Eq 290 with the pressure partials of a general EOS through
  its sound speed and Grüneisen coefficient (the latter by a forward
  difference of the EOS); the eigenvectors are normalised to stay finite at
  rest, where the manual's Eq 293 divides by zero (see `eigen.rs`). `V_t`
  inside `R̃` is the Galerkin-only nodal rate.
- [`Stabilisation::EntropyViscosity`] — §5.4.2, single phase (Eqs 330–338
  with `α = 1`, `l = 0`, `β = 0`, `κ = µ`): dissipative fluxes
  `f = κρ_x`, `g = ρκu_x + uf`, `h = κ(ρe)_x − (u²/2)f + ug` added in
  divergence form, `κ = min(κ_max, κ_e)` per element with the first-order
  bound `(h/2)(|u| + c)` (Eq 334) and the entropy-residual viscosity with
  the jump and the low-Mach denominator (Eqs 335–338). Frozen over a step;
  the residual's time derivatives are backward differences over the
  previous step, and the first step uses `κ_max`.
- [`Stabilisation::Lapidus`] — the manual lists it as an option (§5.4) but
  gives no equations. Implemented as the classical Lapidus viscosity
  `ν = C_L h² |∂u/∂x|` acting on `∂V/∂x` in divergence form, with
  `C_L =` [`FemNumerics::lapidus_coefficient`] (default 1, fixed before any
  run, not tuned). **No reference for the form or the coefficient is in the
  kovan corpus**; the attribution (Lapidus 1967; Löhner, Morgan &
  Zienkiewicz 1985 for its finite-element use) is from memory and not
  verified.
- [`Stabilisation::Galerkin`] — none: the ablation.

# Boundary conditions

Every [`HemBoundary`] is accepted. The end flux `G` is built as
`HemPipe` builds its boundary face (re-implemented, since that function is
private): `Wall` takes the end node with `u = 0`, so its mass and energy
fluxes are exactly zero; `Transmissive` the end node itself;
`FixedPressure`, `Reservoir` and `ChokedBreak` the same face states; `Flux`
the prescribed flux. The stabilisation's own boundary fluxes are taken as
zero (natural boundary condition), so they cannot add or remove mass.

# Conservation

Continuous Galerkin is not locally conservative (no cell has a flux
balance of its own), but because the hats sum to one, summing the nodal
equations cancels every interior term, Galerkin and stabilisation alike,
and leaves only the two end fluxes: **global** mass and energy are
conserved to round-off. `tests/fem1d_vs_fv.rs` measures it.

# What is not here

Wall friction, gravity, wall heat, two-phase (7-equation) entropy
viscosity, implicit time integration, higher-order elements. The PCICE
finite-volume solver (gh:#1068) is not part of [`Discretisation`] yet; the
comparison test leaves a marked hook for it.

```rust
pub mod fem1d { /* ... */ }
```

### Types

#### Enum `Stabilisation`

Which stabilisation the continuous-Galerkin pipe adds (see the module
doc for each one's equations).

```rust
pub enum Stabilisation {
    Galerkin,
    Supg,
    EntropyViscosity,
    Lapidus,
}
```

##### Variants

###### `Galerkin`

No stabilisation: plain Galerkin, the ablation.

###### `Supg`

Streamline-upwind Petrov–Galerkin, RELAP-7 §5.4.1.

###### `EntropyViscosity`

Entropy viscosity, RELAP-7 §5.4.2 (single phase).

###### `Lapidus`

Lapidus artificial viscosity (named in RELAP-7 §5.4, no equations
there).

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `MassMatrix`

Lumped (diagonal, row-sum) or consistent (`∫φ_iφ_j`) mass matrix.

```rust
pub enum MassMatrix {
    Lumped,
    Consistent,
}
```

##### Variants

###### `Lumped`

Row-sum lumped: `m_j = ∫φ_j`. The default.

###### `Consistent`

Consistent tridiagonal, solved by the Thomas algorithm.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `FemNumerics`

Numerical settings of [`FemPipe`].

```rust
pub struct FemNumerics {
    pub courant: uom::si::f64::Ratio,
    pub max_dt: uom::si::f64::Time,
    pub mass_matrix: MassMatrix,
    pub time_integrator: tampines_steam_tables::hem::TimeIntegrator,
    pub lapidus_coefficient: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `courant` | `uom::si::f64::Ratio` | Classical Courant number `(|u| + c)Δt/h`, `> 0`. Default 0.4<br>(`HemPipe`'s default central 0.2 in classical terms). |
| `max_dt` | `uom::si::f64::Time` | Largest step allowed. Default 1 ms, as `HemPipe`. |
| `mass_matrix` | `MassMatrix` | Default [`MassMatrix::Lumped`]. |
| `time_integrator` | `tampines_steam_tables::hem::TimeIntegrator` | Default [`TimeIntegrator::Ssp33`] (see the module doc for why not SSP22). |
| `lapidus_coefficient` | `uom::si::f64::Ratio` | `C_L` of [`Stabilisation::Lapidus`], `≥ 0`. Default 1. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `Discretisation`

Finite volume (the default) or continuous-Galerkin finite elements, for
the same physics.

```rust
pub enum Discretisation {
    FiniteVolume,
    FiniteElement(Stabilisation),
}
```

##### Variants

###### `FiniteVolume`

`tampines_steam_tables::hem::HemPipe`, KNP central-upwind. The default.

###### `FiniteElement`

[`FemPipe`] with the given stabilisation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Stabilisation` |  |

##### Implementations

###### Methods

- ```rust
  pub fn solution_points(self: &Self, geometry: &HemPipeGeometry) -> Vec<Length> { /* ... */ }
  ```
  Where the solution lives on `geometry`: cell centres (finite volume,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Pipe1dNumerics`

The numerics of both solvers; [`Pipe1d::new`] uses the one its
[`Discretisation`] picks.

```rust
pub struct Pipe1dNumerics {
    pub finite_volume: tampines_steam_tables::hem::HemNumerics,
    pub finite_element: FemNumerics,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `finite_volume` | `tampines_steam_tables::hem::HemNumerics` | For [`Discretisation::FiniteVolume`]. |
| `finite_element` | `FemNumerics` | For [`Discretisation::FiniteElement`]. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `Pipe1d`

One HEM pipe on either discretisation: an enum, not a trait object.

```rust
pub enum Pipe1d<E: HemEquationOfState> {
    FiniteVolume(tampines_steam_tables::hem::HemPipe<E>),
    FiniteElement(FemPipe<E>),
}
```

##### Variants

###### `FiniteVolume`

The finite-volume `HemPipe`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines_steam_tables::hem::HemPipe<E>` |  |

###### `FiniteElement`

The continuous-Galerkin [`FemPipe`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FemPipe<E>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(geometry: HemPipeGeometry, eos: E, initial: HemInitialCondition, discretisation: Discretisation, numerics: Pipe1dNumerics) -> Result<Self, HemError> { /* ... */ }
  ```
  Build the pipe on `discretisation`. Both ends start as `Wall`.

- ```rust
  pub fn discretisation(self: &Self) -> Discretisation { /* ... */ }
  ```
  The discretisation this pipe runs.

- ```rust
  pub fn without_wall_friction(self: Self) -> Self { /* ... */ }
  ```
  Ablate the finite-volume wall friction (on by default for a closure

- ```rust
  pub fn set_boundary(self: &mut Self, end: PipeEnd, boundary: HemBoundary) -> Result<(), HemError> { /* ... */ }
  ```
  Set the boundary condition at `end`.

- ```rust
  pub fn run_until(self: &mut Self, t_end: Time) -> Result<HemRunSummary, HemError> { /* ... */ }
  ```
  Advance to exactly `t_end`. `max_courant` is the central Courant

- ```rust
  pub fn time(self: &Self) -> Time { /* ... */ }
  ```
  Simulation time.

- ```rust
  pub fn snapshot(self: &Self) -> Result<HemSnapshot, HemError> { /* ... */ }
  ```
  Every solution point's state (cells or nodes).

- ```rust
  pub fn total_mass(self: &Self) -> Mass { /* ... */ }
  ```
  Mass in the pipe.

- ```rust
  pub fn total_energy(self: &Self) -> Energy { /* ... */ }
  ```
  Total energy in the pipe.

- ```rust
  pub fn cumulative_mass_outflow(self: &Self, end: PipeEnd) -> Mass { /* ... */ }
  ```
  Mass that has left through `end` since `t = 0`.

- ```rust
  pub fn cumulative_energy_outflow(self: &Self, end: PipeEnd) -> Energy { /* ... */ }
  ```
  Total energy that has left through `end` since `t = 0`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Re-export `FemPipe`

```rust
pub use pipe::FemPipe;
```

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
- ~~**Marviken is NOT validated.** The digitised NUREG/CR-2671 test-23/24
  data sits in `marviken_tests.rs`, but the single test is
  `#[ignore = "skip first, Marviken is more complex"]`, its only
  assertion is commented out, and the body ends in `todo!()`.~~
  **CORRECTED 2026-10-16** (checked by reading
  `steam_turbine_equations/converging_diverging_nozzles/tests/marviken_tests.rs`:
  6 `#[test]`s, no `#[ignore]`, no `todo!()`): Marviken is gated, with a
  split result recorded in `tampines-steam-tables/CLAUDE.md` -- test 23
  (3 K subcooling) validated, test 24 (33 K subcooling) **not** validated
  (mean -48.5 %). Marviken is **not** a basis for choosing this backend
  for subcooled stagnation states.

Separately, the Edwards-O'Brien blowdown benchmark **is** genuinely gated
-- 2 active, passing tests in `tampines-steam-tables`'s
`tests/edwards_blowdown.rs` -- but that exercises ~~the transient pipe
solver in [`crate::multiphase_1d`]~~ **(CORRECTED 2026-10-16)** that
crate's own `TampinesSteamArray` PIMPLE/KNP pipe array, not the property
backend selected here. The [`crate::multiphase_1d`] solvers have their own
Edwards case in `multiphase_1d/edwards_tests.rs`.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

~~Consequently the `HybridAllMach` machinery in
`outram-park-fork-coolprop` is **not** needed for the primary circuit,
and neither is coolprop's 50 kg/m^3 density-taper window — HTR-10
helium runs at 1.23-2.74 kg/m^3, far below that floor, so the taper
would zero the all-Mach dissipation everywhere anyway.~~
**Updated 2026-10-16:** that machinery no longer exists. The KNP hybrid
of the coolprop and steam-tables PIMPLE arrays was retired (gh:#914). The
conclusion stands: a low-Mach circuit needs no shock capturing.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
transonic conditions; for those, reach for ~~`outram-park-fork-coolprop`'s
`SolverMode::HybridAllMach`~~ (retired 2026-10-16, gh:#914) the
density-based KNP solver `tampines::hem::HemPipe`, which takes the
CoolProp equation of state.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
[`crate::compressible`]'s finite-volume model). ~~This module adds no new
correlations~~ **CORRECTED 2026-11-01 (GitHub #1063):** it now also holds
[`relap5_htc`], the RELAP5 geometry-101/110 single-phase wall
heat-transfer selection, which adds the Seban–Shimazaki, Inayatov and
Kazimi–Carelli forms (its Dittus–Boelter and Churchill–Chu legs come
from `tuas_boussinesq_solver`). See each backend crate's own
documentation for the re-exported correlations' physics and validated
ranges.

```rust
pub mod heat_transfer { /* ... */ }
```

### Modules

## Module `relap5_htc`

RELAP5 geometry-101/110 single-phase wall heat-transfer selection
(laminar / Dittus–Boelter / Churchill–Chu maximum; Seban–Shimazaki and
Kazimi–Carelli liquid-metal forms; Inayatov bundle multiplier). GitHub
#1063.
RELAP5-style single-phase wall heat-transfer selection for hydraulic
geometries 101 (internal pipe flow) and 110 (vertical bundle of in-line
rods, parallel flow only). GitHub issue #1063, part of #1056.

> Research, education and V&V only. AI draft, unreviewed. Not for
> facility operation, licensing or safety decisions.

# Specification

RELAP-7 Theory Manual (INL), §2.2.2.1–2.2.2.2, Eqs 45–51, which state
RELAP5's single-phase model for these two geometries. The manual is not
redistributable and is cited by section and equation only. Every
correlation it uses is a standard published one, cited at its function:

| Leg | Correlation | Original source | Manual |
|---|---|---|---|
| forced laminar | `Nu = 4.36` | fully developed laminar tube flow, uniform wall flux (exact `48/11 = 4.3636`) | Eq. 45 |
| forced turbulent | Dittus–Boelter `Nu = C Re^0.8 Pr^n`, `C = 0.023`, `n = 0.4` heated / `0.3` cooled | Dittus & Boelter (1930) | Eq. 46 |
| free convection | Churchill–Chu vertical plate | Churchill & Chu (1975) | Eqs 47–48 |
| liquid metal, pipe | `Nu = 5.0 + 0.025 Pe^0.8` | Seban & Shimazaki (1951) | Eq. 49 |
| bundle multiplier | `C = 0.023 P/D`, `P/D` in `[1.1, 1.6]` | Inayatov (1975) | Eq. 50 |
| liquid metal, bundle | `Nu = 4.0 + 0.33 (P/D)^3.8 (Pe/100)^0.86 + 0.16 (P/D)^5`, `P/D` in `[1.1, 1.4]` | Kazimi & Carelli (1976), "Westinghouse correlation" | Eq. 51 |

# Selection logic

- **`Pr ≥ 0.1` (ordinary fluids):** the Nusselt number is the maximum of
  the forced-laminar, forced-turbulent and free-convection values, to
  avoid discontinuities between regimes. The turbulent leg is evaluated
  at **every** Reynolds number (no `Re > 2300` gate), as the manual
  states it. Geometry 110 differs only in the Inayatov multiplier on the
  turbulent leg.
- **`Pr < 0.1` (liquid metals):** one correlation for all regimes — Eq.
  49 for geometry 101, Eq. 51 for geometry 110. No max-selection.

# Consequences of the specification worth knowing (found by the V&V)

- **Dittus–Boelter overtakes the laminar value deep in laminar flow.**
  With the turbulent leg ungated, `0.023 Re^0.8 Pr^0.4 = 4.36` at
  `Re = (4.36/0.023/Pr^0.4)^1.25`: `Re ≈ 703` at `Pr = 1`, `Re ≈ 266` at
  `Pr = 7`. This is what the stated model does; it is not a defect here.
- **The `Pr = 0.1` switch is discontinuous.** At `Re = 10⁴`,
  Dittus–Boelter at `Pr = 0.1` gives `14.51`; Seban–Shimazaki just below
  gives `≈ 11.27`, a 22 % drop.

# Decisions not fixed by the manual (stated, not hidden)

- The heating direction is taken as "fluid heated" when
  `T_wall ≥ T_bulk` (a zero difference picks `n = 0.4`; the free
  convection leg is then at its conduction limit anyway).
- The Grashof number uses `|T_wall − T_bulk|`.
- An absent `P/D` defaults to 1.1 for the liquid-metal bundle form as
  well as for the Inayatov multiplier (the manual states the default for
  Eq. 50 and the clip range for Eq. 51).
- The selection compares Nusselt numbers on the hydraulic diameter. When
  the free-convection length `L` differs from `D_h`, the Churchill–Chu
  value (on `L`) is rescaled by `D_h/L` so the three legs compare the same
  coefficient `h`.
- Published validity ranges (Dittus–Boelter `Re ≳ 10⁴`; Churchill–Chu
  `Ra < 10¹²`; Kazimi–Carelli `10 ≤ Pe ≤ 5000`) are documented and not
  enforced, because the manual evaluates each correlation everywhere.

# Reuse (searched 2026-11-01)

Dittus–Boelter and Churchill–Chu come from `tuas_boussinesq_solver`
(`NusseltCorrelation::DittusBoelter`,
`NusseltCorrelation::ChurchillChuVerticalPlate`, both added for this
issue). The laminar constant is **not** taken from
`NusseltCorrelation::PipeConstantHeatFluxFullyDeveloped`, which returns
`4.354`, nor from `pipe_correlations::laminar_nusselt_uniform_heat_flux_fully_developed`,
which returns `4.36` but panics above `Re = 2300` and so cannot sit under
a max-selection; [`RELAP5_LAMINAR_NUSSELT`] is the manual's `4.36`.
GeN-Foam's generic `A + B Re^C Pr^D` HTC (`outram-foam-appbuilder-lib`,
`fs_htc.rs`) was checked and not used: tampines does not depend on that
crate, and it has no max-selection or bundle forms. No Seban–Shimazaki,
Inayatov or Kazimi–Carelli implementation existed in the workspace.

V&V record: `crates/tampines/docs/relap5-htc-dispatcher-vv.md`.

```rust
pub mod relap5_htc { /* ... */ }
```

### Types

#### Enum `Relap5HeatTransferGeometry`

RELAP5 hydraulic heat-transfer geometry. An enum, not a trait object:
the set is closed (only these two are in RELAP-7).

```rust
pub enum Relap5HeatTransferGeometry {
    InternalPipe,
    VerticalRodBundleParallelFlow {
        pitch_to_diameter: Option<uom::si::f64::Ratio>,
    },
}
```

##### Variants

###### `InternalPipe`

Geometry **101**, the default: internal pipe flow.

###### `VerticalRodBundleParallelFlow`

Geometry **110**: vertical bundle of in-line rods (square pitch, or
staggered on an equilateral triangle pitch), parallel flow only.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pitch_to_diameter` | `Option<uom::si::f64::Ratio>` | Rod pitch over rod diameter `P/D`; `None` means "not provided"<br>and selects the default 1.1. Clipped per correlation (see<br>[`inayatov_pitch_to_diameter`] and<br>[`kazimi_carelli_pitch_to_diameter`]). |

##### Implementations

###### Methods

- ```rust
  pub fn relap5_geometry_number(self: &Self) -> u16 { /* ... */ }
  ```
  The RELAP5 geometry number: 101 or 110.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `Relap5HeatTransferRegime`

Which correlation set the returned Nusselt number.

```rust
pub enum Relap5HeatTransferRegime {
    ForcedLaminar,
    ForcedTurbulent,
    FreeConvection,
    LiquidMetal,
}
```

##### Variants

###### `ForcedLaminar`

Forced laminar, `Nu = 4.36`, was the maximum.

###### `ForcedTurbulent`

Forced turbulent (Dittus–Boelter, with the Inayatov multiplier in
geometry 110) was the maximum.

###### `FreeConvection`

Free convection (Churchill–Chu) was the maximum.

###### `LiquidMetal`

`Pr < 0.1`: the liquid-metal correlation (no max-selection).

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Relap5DimensionlessGroups`

Dimensionless inputs to [`relap5_nusselt`].

```rust
pub struct Relap5DimensionlessGroups {
    pub reynolds: uom::si::f64::Ratio,
    pub prandtl: uom::si::f64::Ratio,
    pub grashof: uom::si::f64::Ratio,
    pub free_convection_length_over_hydraulic_diameter: uom::si::f64::Ratio,
    pub heating_direction: tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::FluidHeatingDirection,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `reynolds` | `uom::si::f64::Ratio` | Reynolds number on the hydraulic diameter, `≥ 0`. |
| `prandtl` | `uom::si::f64::Ratio` | Bulk Prandtl number, `> 0`. |
| `grashof` | `uom::si::f64::Ratio` | Grashof number on the free-convection length `L`, `≥ 0`. |
| `free_convection_length_over_hydraulic_diameter` | `uom::si::f64::Ratio` | `L / D_h`, the free-convection length over the hydraulic diameter<br>(1 for RELAP5's default `L = D_h`), `> 0`. |
| `heating_direction` | `tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::FluidHeatingDirection` | Whether the wall heats or cools the fluid (Dittus–Boelter exponent). |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Relap5NusseltSelection`

The selected Nusselt number and the regime that produced it.

```rust
pub struct Relap5NusseltSelection {
    pub nusselt: uom::si::f64::Ratio,
    pub regime: Relap5HeatTransferRegime,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nusselt` | `uom::si::f64::Ratio` | Nusselt number on the hydraulic diameter, `h D_h / k`. |
| `regime` | `Relap5HeatTransferRegime` | Which correlation won. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Relap5WallConvectionInputs`

Physical (`uom`) inputs for [`relap5_wall_heat_transfer`]. Properties
are bulk-fluid values (the manual evaluates `k` at the bulk
temperature).

```rust
pub struct Relap5WallConvectionInputs {
    pub density: uom::si::f64::MassDensity,
    pub dynamic_viscosity: uom::si::f64::DynamicViscosity,
    pub thermal_conductivity: uom::si::f64::ThermalConductivity,
    pub specific_heat_capacity: uom::si::f64::SpecificHeatCapacity,
    pub thermal_expansion_coefficient: uom::si::f64::TemperatureCoefficient,
    pub bulk_temperature: uom::si::f64::ThermodynamicTemperature,
    pub wall_temperature: uom::si::f64::ThermodynamicTemperature,
    pub flow_speed: uom::si::f64::Velocity,
    pub hydraulic_diameter: uom::si::f64::Length,
    pub free_convection_length: Option<uom::si::f64::Length>,
    pub gravity: uom::si::f64::Acceleration,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `uom::si::f64::MassDensity` | Fluid density `ρ`. |
| `dynamic_viscosity` | `uom::si::f64::DynamicViscosity` | Dynamic viscosity `μ`. |
| `thermal_conductivity` | `uom::si::f64::ThermalConductivity` | Thermal conductivity `k`. |
| `specific_heat_capacity` | `uom::si::f64::SpecificHeatCapacity` | Isobaric specific heat capacity `c_p` (for `Pr = μ c_p / k`). |
| `thermal_expansion_coefficient` | `uom::si::f64::TemperatureCoefficient` | Volumetric thermal expansion coefficient `β`. |
| `bulk_temperature` | `uom::si::f64::ThermodynamicTemperature` | Area-averaged bulk fluid temperature `T`. |
| `wall_temperature` | `uom::si::f64::ThermodynamicTemperature` | Wall temperature `T_w`. |
| `flow_speed` | `uom::si::f64::Velocity` | Mean flow speed `|u|` (its sign does not matter). |
| `hydraulic_diameter` | `uom::si::f64::Length` | Heat-transfer hydraulic diameter `D_h`. |
| `free_convection_length` | `Option<uom::si::f64::Length>` | Free-convection length scale `L`; `None` uses `D_h` (RELAP5's<br>default). |
| `gravity` | `uom::si::f64::Acceleration` | Gravitational acceleration `g`. |

##### Implementations

###### Methods

- ```rust
  pub fn dimensionless_groups(self: &Self) -> Result<Relap5DimensionlessGroups, TampinesError> { /* ... */ }
  ```
  Forms `Re = ρ |u| D_h / μ`, `Pr = μ c_p / k`,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Relap5WallHeatTransfer`

Result of [`relap5_wall_heat_transfer`].

```rust
pub struct Relap5WallHeatTransfer {
    pub groups: Relap5DimensionlessGroups,
    pub selection: Relap5NusseltSelection,
    pub heat_transfer_coefficient: uom::si::f64::HeatTransfer,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `groups` | `Relap5DimensionlessGroups` | The dimensionless groups formed from the inputs. |
| `selection` | `Relap5NusseltSelection` | The selected Nusselt number and regime. |
| `heat_transfer_coefficient` | `uom::si::f64::HeatTransfer` | Wall heat-transfer coefficient `h = Nu k / D_h`. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `seban_shimazaki_liquid_metal_nusselt`

Seban–Shimazaki liquid-metal correlation for turbulent flow in a pipe
with uniform wall temperature, `Nu = 5.0 + 0.025 Pe^0.8`.

Source: R. A. Seban and T. T. Shimazaki (1951), "Heat transfer to a
fluid flowing turbulently in a smooth pipe with walls at constant
temperature", *Transactions of the ASME* **73**, 803–809. (Lyon's 1951
uniform-heat-flux form has 7.0 in place of 5.0; the manual's Eq. 49 has
5.0.) Used for all liquid-metal regimes in geometry 101 (RELAP-7 Theory
Manual §2.2.2.1, Eq. 49).

Hand check: `Pe = 100` gives `100^0.8 = 39.8107`, `Nu = 5.99527`.

```rust
pub fn seban_shimazaki_liquid_metal_nusselt(peclet_number: f64) -> f64 { /* ... */ }
```

#### Function `inayatov_pitch_to_diameter`

`P/D` used by the Inayatov multiplier: an absent value or one below
1.1 becomes 1.1; above 1.6 becomes 1.6 (RELAP-7 Theory Manual §2.2.2.2,
following RELAP5).

```rust
pub fn inayatov_pitch_to_diameter(pitch_to_diameter: Option<f64>) -> f64 { /* ... */ }
```

#### Function `inayatov_bundle_coefficient`

Inayatov's bundle coefficient replacing Dittus–Boelter's 0.023 for
parallel flow along in-line (square) or staggered (equilateral
triangle) rod bundles: `C = 0.023 P/D`, `P/D` clipped by
[`inayatov_pitch_to_diameter`].

Source: A. Ya. Inayatov (1975), "Correlation of data on heat transfer
flow parallel to tube bundles at relative tube pitches of
1.1 < s/d < 1.6", *Heat Transfer — Soviet Research* **7**(3), 84–88.
As used in RELAP5 / RELAP-7 (Theory Manual §2.2.2.2, Eq. 50).

```rust
pub fn inayatov_bundle_coefficient(pitch_to_diameter: Option<f64>) -> f64 { /* ... */ }
```

#### Function `kazimi_carelli_pitch_to_diameter`

`P/D` used by the liquid-metal bundle correlation: an absent value
defaults to 1.1 (assumption, see the module docs), then clipped to
`[1.1, 1.4]` (RELAP-7 Theory Manual §2.2.2.2, below Eq. 51).

```rust
pub fn kazimi_carelli_pitch_to_diameter(pitch_to_diameter: Option<f64>) -> f64 { /* ... */ }
```

#### Function `kazimi_carelli_rod_bundle_liquid_metal_nusselt`

Liquid-metal Nusselt number for parallel flow in rod bundles,
`Nu = 4.0 + 0.33 (P/D)^3.8 (Pe/100)^0.86 + 0.16 (P/D)^5.0`. The
argument is used as given; clip it first with
[`kazimi_carelli_pitch_to_diameter`].

Source: M. S. Kazimi and M. D. Carelli (1976), "Heat transfer
correlation for analysis of CRBRP assemblies", Westinghouse report
CRBRP-ARD-0034, known as the Westinghouse correlation and reproduced in
N. E. Todreas and M. S. Kazimi, *Nuclear Systems I* (§10, liquid-metal
heat transfer in rod bundles) with validity `1.1 ≤ P/D ≤ 1.4`,
`10 ≤ Pe ≤ 5000`. The form, coefficients and `P/D` range match RELAP-7
Theory Manual §2.2.2.2, Eq. 51 (which does not name its source); the
identification is from the coefficients and range and should be
confirmed against the report by a reviewer.

Hand check: `P/D = 1.2, Pe = 100`: `1.2^3.8 = 1.99938`,
`1.2^5 = 2.48832`, `Nu = 4 + 0.65980 + 0.39813 = 5.05793`.

```rust
pub fn kazimi_carelli_rod_bundle_liquid_metal_nusselt(pitch_to_diameter: f64, peclet_number: f64) -> f64 { /* ... */ }
```

#### Function `relap5_nusselt`

RELAP5 single-phase Nusselt number on the hydraulic diameter, selected
as described in the module docs (RELAP-7 Theory Manual §2.2.2.1–2.2.2.2).

Errors on non-finite inputs, `Re < 0`, `Pr ≤ 0`, `Gr < 0`,
`L/D_h ≤ 0`, or a provided `P/D` that is not finite.

```rust
pub fn relap5_nusselt(geometry: Relap5HeatTransferGeometry, groups: Relap5DimensionlessGroups) -> Result<Relap5NusseltSelection, crate::error::TampinesError> { /* ... */ }
```

#### Function `relap5_wall_heat_transfer`

RELAP5 single-phase wall heat-transfer coefficient for geometry 101 or
110: forms the dimensionless groups, selects the Nusselt number with
[`relap5_nusselt`] and returns `h = Nu k / D_h`.

```rust
pub fn relap5_wall_heat_transfer(geometry: Relap5HeatTransferGeometry, inputs: &Relap5WallConvectionInputs) -> Result<Relap5WallHeatTransfer, crate::error::TampinesError> { /* ... */ }
```

### Constants and Statics

#### Constant `RELAP5_LAMINAR_NUSSELT`

Laminar Nusselt number for fully developed flow in a circular tube with
uniform wall heat flux, as RELAP5 / RELAP-7 use it (Theory Manual
§2.2.2.1, Eq. 45). The exact value is `48/11 = 4.3636…` (e.g. Incropera
& DeWitt, Eq. 8.53).

```rust
pub const RELAP5_LAMINAR_NUSSELT: f64 = 4.36;
```

#### Constant `LIQUID_METAL_PRANDTL_THRESHOLD`

Prandtl number below which a fluid is treated as a liquid metal
(Theory Manual §2.2.2.1–2.2.2.2). `Pr < 0.1` is liquid metal;
`Pr = 0.1` is an ordinary fluid.

```rust
pub const LIQUID_METAL_PRANDTL_THRESHOLD: f64 = 0.1;
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

#### Re-export `DittusBoelterData`

Dittus–Boelter inputs and the heating/cooling selector, re-exported
from `tuas_boussinesq_solver` (the `NusseltCorrelation::DittusBoelter`
variant's data).

```rust
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::DittusBoelterData;
```

#### Re-export `FluidHeatingDirection`

Dittus–Boelter inputs and the heating/cooling selector, re-exported
from `tuas_boussinesq_solver` (the `NusseltCorrelation::DittusBoelter`
variant's data).

```rust
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::FluidHeatingDirection;
```

#### Re-export `churchill_chu_vertical_plate_nusselt`

Churchill–Chu vertical-plate free convection, with the Grashof and
Rayleigh numbers, re-exported from `tuas_boussinesq_solver`.

```rust
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::churchill_chu_vertical_plate_nusselt;
```

#### Re-export `grashof_number`

Churchill–Chu vertical-plate free convection, with the Grashof and
Rayleigh numbers, re-exported from `tuas_boussinesq_solver`.

```rust
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::grashof_number;
```

#### Re-export `rayleigh_number`

Churchill–Chu vertical-plate free convection, with the Grashof and
Rayleigh numbers, re-exported from `tuas_boussinesq_solver`.

```rust
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::rayleigh_number;
```

#### Re-export `ChurchillChuData`

Churchill–Chu vertical-plate free convection, with the Grashof and
Rayleigh numbers, re-exported from `tuas_boussinesq_solver`.

```rust
pub use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::ChurchillChuData;
```

## Module `hem`

Homogeneous Equilibrium Model (HEM) two-phase flow: the steam-water state,
and the transient HEM pipe solver.

# What is here

- [`HemSteamCv`]: thin re-export of [`tampines_steam_tables`]'s validated
  HEM control-volume type, the pressure/enthalpy/quality steam-water state
  used by [`crate::critical_flow`]'s choked-flow solvers.
- [`HemPipe`]: the density-based KNP HEM pipe solver on `[ρ, ρu, ρE]`
  (gh #902). The solver lives in `tampines_steam_tables::hem` (maintainer
  decision D2, gh #896); it is re-exported here, and this module adds
  - [`CoolPropHem`], a CoolProp closure (only `tampines` depends on both
    property crates), and
  - [`HemEos`], an enum choosing IAPWS-IF97, CoolProp or an ideal gas at
    run time.

  `HemPipe` here is `HemPipeOf<HemEos>`, so one type covers every closure:

  ```no_run
  use tampines::hem::*;
  use uom::si::f64::*;
  use uom::si::{area::square_meter, length::meter, pressure::megapascal,
                thermodynamic_temperature::kelvin, time::second, velocity::meter_per_second};

  let geometry = HemPipeGeometry::new(
      Length::new::<meter>(1.0), Area::new::<square_meter>(1e-3), 50).unwrap();
  let initial = HemInitialCondition::Uniform(PrimitiveState::PressureTemperature {
      pressure: Pressure::new::<megapascal>(2.0),
      temperature: ThermodynamicTemperature::new::<kelvin>(400.0),
      velocity: Velocity::new::<meter_per_second>(0.0),
  });
  let mut pipe = HemPipe::new(geometry, HemEos::Iapws97, initial, HemNumerics::default()).unwrap();
  pipe.run_until(Time::new::<second>(1e-3)).unwrap();
  ```

  Errors are [`HemError`], which converts into [`crate::TampinesError`]
  with `?`.

# What is not here

Slip (drift-flux) and thermal non-equilibrium (two-fluid) are in
[`crate::multiphase_1d`]. The pipe solver is inviscid and frictionless; see
`tampines_steam_tables::hem` for its full scope, its deviations from
upstream `rhoCentralFoam`, and its V&V record.

```rust
pub mod hem { /* ... */ }
```

### Types

#### Type Alias `HemPipe`

The HEM pipe solver with its closure chosen at run time by [`HemEos`]
(IAPWS-IF97, CoolProp or ideal gas). See `tampines_steam_tables::hem::HemPipe`
for the methods.

```rust
pub type HemPipe = HemPipeOf<HemEos>;
```

### Re-exports

#### Re-export `CoolPropHem`

```rust
pub use coolprop_eos::CoolPropHem;
```

#### Re-export `HemEos`

```rust
pub use coolprop_eos::HemEos;
```

#### Re-export `COOLPROP_P_RHO_E_REL_TOL`

```rust
pub use coolprop_eos::COOLPROP_P_RHO_E_REL_TOL;
```

#### Re-export `pressure_from_rho_e`

```rust
pub use tampines_steam_tables::hem::pressure_from_rho_e;
```

#### Re-export `BreakFlow`

```rust
pub use tampines_steam_tables::hem::BreakFlow;
```

#### Re-export `FluxLimiter`

```rust
pub use tampines_steam_tables::hem::FluxLimiter;
```

#### Re-export `HemBoundary`

```rust
pub use tampines_steam_tables::hem::HemBoundary;
```

#### Re-export `HemCellState`

```rust
pub use tampines_steam_tables::hem::HemCellState;
```

#### Re-export `HemEquationOfState`

```rust
pub use tampines_steam_tables::hem::HemEquationOfState;
```

#### Re-export `HemError`

```rust
pub use tampines_steam_tables::hem::HemError;
```

#### Re-export `HemInitialCondition`

```rust
pub use tampines_steam_tables::hem::HemInitialCondition;
```

#### Re-export `HemNumerics`

```rust
pub use tampines_steam_tables::hem::HemNumerics;
```

#### Re-export `HemPipeGeometry`

```rust
pub use tampines_steam_tables::hem::HemPipeGeometry;
```

#### Re-export `HemRunSummary`

```rust
pub use tampines_steam_tables::hem::HemRunSummary;
```

#### Re-export `HemSnapshot`

```rust
pub use tampines_steam_tables::hem::HemSnapshot;
```

#### Re-export `HemStepReport`

```rust
pub use tampines_steam_tables::hem::HemStepReport;
```

#### Re-export `HemThermoState`

```rust
pub use tampines_steam_tables::hem::HemThermoState;
```

#### Re-export `Iapws97Hem`

```rust
pub use tampines_steam_tables::hem::Iapws97Hem;
```

#### Re-export `IdealGasHem`

```rust
pub use tampines_steam_tables::hem::IdealGasHem;
```

#### Re-export `PipeEnd`

```rust
pub use tampines_steam_tables::hem::PipeEnd;
```

#### Re-export `PrimitiveState`

```rust
pub use tampines_steam_tables::hem::PrimitiveState;
```

#### Re-export `BarotropicHem`

```rust
pub use tampines_steam_tables::hem::BarotropicHem;
```

#### Re-export `IsentropicStiffenedGasHem`

```rust
pub use tampines_steam_tables::hem::IsentropicStiffenedGasHem;
```

#### Re-export `LinearEosHem`

```rust
pub use tampines_steam_tables::hem::LinearEosHem;
```

#### Re-export `StiffenedGasHem`

```rust
pub use tampines_steam_tables::hem::StiffenedGasHem;
```

#### Re-export `StiffenedGasPair`

```rust
pub use tampines_steam_tables::hem::StiffenedGasPair;
```

#### Re-export `TimeIntegrator`

```rust
pub use tampines_steam_tables::hem::TimeIntegrator;
```

#### Re-export `TimeStepControl`

```rust
pub use tampines_steam_tables::hem::TimeStepControl;
```

#### Re-export `FIXED_STEP_COURANT_LIMIT`

```rust
pub use tampines_steam_tables::hem::FIXED_STEP_COURANT_LIMIT;
```

#### Re-export `HemPipe`

The generic solver, `HemPipeOf<E>` for any closure `E`; use it to run with
a concrete closure type (e.g. `HemPipeOf<CoolPropHem>`).

```rust
pub use tampines_steam_tables::hem::HemPipe as HemPipeOf;
```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

~~**CORRECTED 2026-10-16:** the HEM row names a *model level*, not a solver
in this crate. [`crate::hem`] holds only a re-export of the steam-table
control volume (`HemSteamCv`); the HEM pipe solver lives in
`tampines-steam-tables` (`TampinesSteamArray`). Exporting a HEM pipe solver
from `tampines` is gh:#896.~~ **CORRECTED 2026-11-01:** [`crate::hem`]
re-exports the KNP HEM pipe solver, `HemPipe` (`HemPipeOf<E>` for any
closure), from `tampines_steam_tables::hem`; its stability work is epic
gh:#896.
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

- ~~**No wall heat transfer.** Both solvers are adiabatic. A blowdown is
  dominated by depressurisation, not by wall heat, but a rewetting or
  reflood problem is not solvable with these as they stand.~~ **CORRECTED
  2026-11-01 (gh:#1062):** both take a [`wall_heat::PhasicWallHeat`]
  (adiabatic by default): a prescribed flux, a wall temperature with `h`,
  or `h` from the RELAP5 dispatcher, split between the phases by the
  linear wall-contact partition. There is still no boiling curve, CHF or
  rewetting logic, so a reflood problem is still out of reach.
- **No interfacial area transport.** [`two_fluid::TwoFluid1d`] takes a
  prescribed bubble diameter, so ~~its drag and its interfacial heat transfer
  do not respond to a changing flow regime~~ (**CORRECTED 2026-11-01,
  gh:#1060:** above `α_g = 0.5` the area now follows the regime map; in the
  bubbly window it is still `6 α_g / d_b`) the bubble size never responds to
  breakup or nucleation. That absence is not cosmetic: it
  is what makes the six-equation solver **refuse** a transient started at a
  low void fraction, because a phase with almost no interfacial area cannot
  shed its reversible expansion work fast enough to stay inside the bounded
  metastable branches. Measured boundary and diagnosis in
  `two_fluid_tests::six_equation_march_refuses_where_a_phase_cannot_shed_its_expansion_work`.
- ~~**No flow-regime map.** A real system code selects closures from a
  regime map (bubbly / slug / annular / mist). Here the closure is whatever
  the caller passed, everywhere, for the whole transient.~~ **CORRECTED
  2026-11-01 (gh:#1060):** [`two_fluid::TwoFluid1d`] takes its interfacial
  area from a void-fraction regime map (bubbly/slug to annular/mist, the
  reused GeN-Foam `RegimeMap1D` and `InterfacialArea`); the correlations
  themselves are still the caller's, and the low-void refusal below is
  unchanged by it (re-measured).
- **No counter-current flow limitation, no critical-flow model of their
  own.** The break boundary reuses the crate's existing HEM critical-flow
  dispatcher, so the *break* is HEM even when the *pipe* is drift-flux or
  two-fluid. This is a real modelling inconsistency and it is called out
  again at the boundary condition itself.
- ~~**One validation case, and only one.**~~ **CORRECTED 2026-10-16** —
  there are two: Edwards (below) and Marviken test 23, where
  `marviken_test_23_drift_flux_critical_mass_flux` (a plain `#[test]` in
  `marviken_tests.rs`) is recorded VALIDATED at 8.9 % mean |dev|; test 24 is
  characterisation only. See that file's header. As of 2026-08-11
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

[`crate::multiphase_1d`] lists what applies to both solvers (no
interfacial-area transport, HEM break model; ~~no wall heat transfer, no
flow-regime map~~ **CORRECTED 2026-11-01:** wall heat exists since gh:#1062,
see [`DriftFlux1d::set_wall_heat`]; the gh:#1060 regime map applies to the
two-fluid solver only). Specific to this solver:

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
  pub fn set_wall_heat(self: &mut Self, wall_heat: PhasicWallHeat) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the wall heat boundary condition (gh:#1062, RELAP-7 Eq. 201).

- ```rust
  pub fn wall_heat(self: &Self) -> PhasicWallHeat { /* ... */ }
  ```
  The wall heat boundary condition.

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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

# Variable flow area (gh:#1058)

[`Pipe1d::with_area_profile`] gives the pipe an area `A(x)` from the
**reused** [`tampines_steam_tables::hem::AreaProfile`] (`HemPipe`'s type,
evaluated through `HemPipeGeometry::flow_area_at`, so the two cannot
drift). As in `HemPipe`, faces carry `A(x_j)`, a cell's area is the mean
of its two faces and its volume `Δx` times that. The hydraulic diameter
scales as `D_h(x) = D_h √(A(x)/A)` with `A`, `D_h` the constructor's
values — exact for a circular section, an assumption otherwise. Without a
profile every accessor returns the constructor's value bit for bit.

# What does not

Anything that changes during a transient. Both solvers own their own field
state; this type is constructed once and read thereafter.

```rust
pub mod geometry { /* ... */ }
```

### Types

#### Struct `Pipe1d`

A straight pipe, of constant cross-section unless given an area profile
([`Pipe1d::with_area_profile`], gh:#1058), discretised into uniform axial
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
  pub fn with_area_profile(self: Self, profile: AreaProfile) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Total pipe length `L` \[m\].

- ```rust
  pub fn area_profile(self: &Self) -> Option<AreaProfile> { /* ... */ }
  ```
  The area profile, if one was given.

- ```rust
  pub fn face_area_si(self: &Self, j: usize) -> f64 { /* ... */ }
  ```
  Flow area at face `j` \[m²\] (`0 ≤ j ≤ n_cells`).

- ```rust
  pub fn cell_area_si(self: &Self, i: usize) -> f64 { /* ... */ }
  ```
  Flow area of cell `i` \[m²\], the mean of its two faces.

- ```rust
  pub fn cell_volume_at(self: &Self, i: usize) -> f64 { /* ... */ }
  ```
  Volume of cell `i` \[m³\]; equals [`cell_volume`](Self::cell_volume)

- ```rust
  pub fn face_hydraulic_diameter_si(self: &Self, j: usize) -> f64 { /* ... */ }
  ```
  Hydraulic diameter at face `j` \[m\].

- ```rust
  pub fn cell_hydraulic_diameter_si(self: &Self, i: usize) -> f64 { /* ... */ }
  ```
  Hydraulic diameter of cell `i` \[m\], from the cell area.

- ```rust
  pub fn length(self: &Self) -> Length { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
- ~~**Interfacial-area transport and the flow-regime map.** See "Honest
  scope" below — both are absent, and their absence is the largest
  modelling limitation of this layer.~~ **CORRECTED 2026-11-01 (gh:#1060):**
  a one-parameter **flow-regime map** now sets the interfacial area — see
  "Flow-regime interfacial area" below. Interfacial-area *transport* is
  still absent.
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
  (`… = α_k ∂p/∂t + Q_k^i + Γ_k h_k^i`; ~~`+ q_k^wall`~~ **CORRECTED
  2026-11-01:** the solver has no wall heat term, it is adiabatic;
  **CORRECTED AGAIN 2026-11-01, gh:#1062:** it has one now,
  `+ Q_k^wall` from [`super::wall_heat`], adiabatic by default). A
  **positive** `Q_g^i`
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

# Flow-regime interfacial area (gh:#1060)

RELAP-7's two-phase closures take the interfacial area concentration
`A_int` from the local flow regime (RELAP-7 Theory Manual, Eq. 117; the
manual is not public, so only the equation number is cited). This layer
does that through **reused** GeN-Foam closures rather than a second port:

- the regime is classified on `α_g` by
  [`outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::interfacial::regime_map::RegimeMap1D`]
  (GeN-Foam `regimeMapModels::oneParameter`), linear interpolation across
  the gap between the two windows;
- the area of each regime comes from
  [`outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::interfacial::area::InterfacialArea`]:
  `Spherical` (`6 α/d_b`) for **bubbly/slug** and `Annular` (`4 α/D_h`,
  vapour core of the pipe's hydraulic diameter) for **annular/mist**, both
  at the upstream default `cutoff_alpha = 0.9`;
- the windows are `α_g ≤ 0.5` bubbly/slug and `α_g ≥ 0.75` annular/mist
  ([`BUBBLY_SLUG_UPPER_VOID`], [`ANNULAR_MIST_LOWER_VOID`]), the void-only
  interfacial-drag regime map of the TRACE V5 Theory Manual. **Not
  re-checked** against that manual in this session (it is not in the
  repository); treat the two numbers as a cited modelling choice, not a
  verified transcription.

The drag and heat-transfer correlations consumed from the 3-D reference
take a *diameter*, not an area, so the regime area enters them as the
**Sauter-equivalent diameter** `d_eff = 6 α_g / A_int`. In the pure
bubbly window `A_int = 6 α_g / d_b` exactly, so `d_eff = d_b` and the
closures are **bit-identical** to the fixed-diameter set there (pinned by
`regime_map_is_bit_identical_to_the_fixed_diameter_set_in_the_bubbly_window`).
In the annular window below the cutoff `A_int = 4 α_g / D_h`, so
`d_eff = 1.5 D_h` — sphere correlations evaluated at the equivalent
diameter of the vapour core — **there is no
film correlation here**, and that is stated as a limitation, not hidden.

The regime map is opt-in at **this** layer
([`InterfacialExchange::with_regime_map`]) only because it needs the
pipe's hydraulic diameter, which a free-standing closure set does not
know. [`super::two_fluid::TwoFluid1d::new`] switches it **on** for every
vapour-dispersed closure set; the ablation is
[`super::two_fluid::TwoFluid1d::without_regime_map`].

# Honest scope — what this layer does NOT do

- **No interfacial-area transport.** The bubble diameter `d_b` is
  prescribed once at construction and never responds to breakup,
  coalescence, or the local flow. Every closure here scales as `1/d²`, so
  `d_b` is the single most influential number a caller supplies and the
  least justified — in the bubbly window it is still the whole story.
- ~~**No flow-regime map.** [`DispersedPhase`] is fixed for the whole
  calculation. A real transient runs bubbly at low void and droplet/mist at
  high void; this layer will happily evaluate bubbly closures at `α_g =
  0.99`, which is wrong and will not complain.~~ **CORRECTED 2026-11-01
  (gh:#1060):** with [`InterfacialExchange::with_regime_map`] the area
  follows the regime, but [`DispersedPhase`] is still fixed (vapour) — the
  annular/mist area is that of a vapour core, and no droplet field exists.
  The droplet closure set ([`InterfacialExchange::droplet`]) refuses the
  regime map rather than evaluate a vapour-core area for dispersed liquid.
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

#### Enum `InterfacialAreaModel`

How the interfacial area concentration `A_int` \[1/m\] is obtained.

Enum dispatch per the workspace rule.

```rust
pub enum InterfacialAreaModel {
    FixedDiameter,
    RegimeMap {
        hydraulic_diameter: f64,
    },
}
```

##### Variants

###### `FixedDiameter`

The pre-gh:#1060 behaviour: one prescribed inclusion diameter `d`
everywhere, so `A_int = 6 α_d / d` at every void fraction. The
**ablation** — what [`super::two_fluid::TwoFluid1d::without_regime_map`]
selects.

###### `RegimeMap`

`A_int` from the void-fraction regime map (module docs, "Flow-regime
interfacial area"): bubbly/slug spherical area at the bubble diameter,
annular/mist core area at the hydraulic diameter, blended across the
gap.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `hydraulic_diameter` | `f64` | Pipe hydraulic diameter `D_h` \[m\] that sizes the annular core. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    pub interfacial_area_concentration: f64,
    pub effective_diameter: f64,
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
| `interfacial_area_concentration` | `f64` | Interfacial area concentration `A_int` \[1/m\] the closures were<br>evaluated at (gh:#1060): `6 α_d / d` for<br>[`InterfacialAreaModel::FixedDiameter`], the regime-blended area for<br>[`InterfacialAreaModel::RegimeMap`]. |
| `effective_diameter` | `f64` | The inclusion diameter \[m\] the closures were evaluated at: the<br>prescribed `d`, or the Sauter-equivalent `6 α_d / A_int` under the<br>regime map. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
  pub fn with_regime_map(self: Self, hydraulic_diameter: Length) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Switch the interfacial area to the void-fraction **regime map**

- ```rust
  pub fn without_regime_map(self: Self) -> Self { /* ... */ }
  ```
  Back to one prescribed diameter everywhere — the **ablation** of

- ```rust
  pub fn area_model(self: &Self) -> InterfacialAreaModel { /* ... */ }
  ```
  Which interfacial-area model is active.

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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Constant `BUBBLY_SLUG_UPPER_VOID`

Upper void fraction of the **bubbly/slug** regime window \[-\] (gh:#1060).

`0.5`, from the void-only interfacial-drag regime map of the TRACE V5
Theory Manual (bubbly/slug to `α_g = 0.5`, annular/mist from `0.75`, linear
interpolation between). **Not re-checked** against the manual in this
session; see the module docs.

```rust
pub const BUBBLY_SLUG_UPPER_VOID: f64 = 0.5;
```

#### Constant `ANNULAR_MIST_LOWER_VOID`

Lower void fraction of the **annular/mist** regime window \[-\] (gh:#1060).
See [`BUBBLY_SLUG_UPPER_VOID`] for the provenance and its caveat.

```rust
pub const ANNULAR_MIST_LOWER_VOID: f64 = 0.75;
```

#### Constant `REGIME_AREA_CUTOFF_ALPHA`

`cutoff_alpha` handed to both reused GeN-Foam area models \[-\]: `0.9`, the
upstream default (`closures::interfacial::area`). Above it the spherical
area tapers as `(1−α)/(1−0.9)`; the annular one drops by
`1/(1+√0.9) ≈ 0.513` and tapers as `(1−√α)/(1−0.9)` — a jump that is
upstream's, reproduced, and documented there.

```rust
pub const REGIME_AREA_CUTOFF_ALPHA: f64 = 0.9;
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

`∂(m_k h_k)/∂t + ∂(m_k h_k u_k)/∂x = α_k ∂p/∂t + Q_k^i + Γ_k h_k^i + Q_k^wall`  (phase energy)

with `Q_k^wall` the phasic wall heat of gh:#1062 (RELAP-7 Eq. 201,
[`super::wall_heat`]), zero for the default adiabatic wall.

and the **volume constraint** `α_g + α_l = 1`, which is what the pressure
equation enforces (see "The pressure equation" below).

The momentum equation is written in the *non-conservative* velocity form —
the conservative form minus `u_k ×` the phase mass equation — which is why
the mass-transfer momentum source appears as `Γ_k (u^i − u_k)` rather than
`Γ_k u^i`.

# Interface velocity, friction work and total-enthalpy mass transfer (gh:#1060)

RELAP-7 carries the interphase exchange at an **interface velocity**
`u_int` (Theory Manual Eq. 200) and lets the transferred mass carry the
interface **total** enthalpy `H_int = h_int + ½ u_int²` (Eq. 208). The
manual is not public, so only equation numbers are cited; the definition
of `u_int` used here is the **mass-weighted** (centre-of-mass) velocity

`u_int = (α_l ρ_l u_l + α_g ρ_g u_g) / (α_l ρ_l + α_g ρ_g)`

as specified in the gh:#1060 brief — **not** re-checked against Eq. 200,
which was not available in this session. This solver's energy equations
are in *static-enthalpy* form, so the total-energy sources translate as
follows (derived here, not transcribed): subtract each phase's
kinetic-energy equation (`u_k ×` its momentum equation) from its
total-energy equation.

- **Mass-transfer momentum**: `Γ_k (u_int − u_k)` with `u_int` above,
  replacing the donor-phase velocity used before.
- **Friction work at `u_int`**: the interfacial drag `F_k` does work
  `F_k u_int` in phase `k`'s total energy, of which `F_k u_k` is
  kinetic, so `F_k (u_int − u_k)` appears in the static-enthalpy equation.
  Summed over both phases this is `K_d (u_g − u_l)² ≥ 0` — the drag
  dissipation, now split between the phases by `u_int` instead of being
  dropped.
- **Total-enthalpy mass transfer**: `Γ_k H_int` in total energy becomes
  `Γ_k (h_k^sat + ½ (u_int − u_k)²)` in static enthalpy. `Γ` itself is
  **unchanged**, because `H_g,int − H_l,int = h_g^sat − h_f^sat` exactly
  (both sides carry the same `½ u_int²`); what static `h_sat` got wrong was
  only the energy the transferred mass carries.

Each is **on by default**, with the ablations
[`TwoFluid1d::without_interface_velocity`] (donor velocity, no friction
work) and [`TwoFluid1d::without_total_enthalpy_mass_transfer`]. With both
ablated and [`TwoFluid1d::without_regime_map`], the march is bit-identical
to the pre-gh:#1060 solver (pinned by
`all_three_ablations_reproduce_the_pre_1060_march_bit_for_bit`).

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
heat transfer, no interfacial-area transport, HEM break model;
~~no flow-regime map~~ **CORRECTED 2026-11-01 (gh:#1060):** the interfacial
area now follows a void-fraction regime map, see [`super::interfacial`]).
Specific to this one:

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
- ~~**Boundary conditions are velocity-type only**~~ **CORRECTED
  2026-11-01 (gh:#1076):** pressure outlets, pressure inlets and velocity
  inlets with a described inflow state exist, with time-dependent
  versions ([`TwoFluidTimeDependentBoundary`]); what follows is the
  original note.
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

~~**Every variant is a *velocity* boundary.**~~ **CORRECTED 2026-11-01
(gh:#1076):** [`PressureOutlet`](Self::PressureOutlet) and
[`PressureInlet`](Self::PressureInlet) are pressure boundaries, and
[`VelocityInlet`](Self::VelocityInlet) is a velocity boundary that says
what enters. The paragraph below describes the original velocity-only set.

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
    PressureOutlet {
        pressure: f64,
    },
    PressureInlet {
        pressure: f64,
        inflow: TwoFluidInflow,
    },
    VelocityInlet {
        velocity: f64,
        inflow: TwoFluidInflow,
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
does; ~~there is none yet~~ **CORRECTED 2026-11-01 (gh:#1076):** use
[`VelocityInlet`](Self::VelocityInlet) or
[`PressureInlet`](Self::PressureInlet).

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

###### `PressureOutlet`

A **pressure** boundary \[Pa\] (gh:#1076): the face velocity of each
phase follows from its own momentum equation across the half cell
between the boundary and the first cell centre, with the 2×2
drag + virtual-mass block assembled on the adjacent cell's state.
Backflow re-injects the adjacent cell's state (zero-gradient donor).
The pressure is static: no `½ρu²` is subtracted.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | Static pressure at the face \[Pa\]. |

###### `PressureInlet`

A pressure boundary that also says **what enters** (gh:#1076): the
face velocities as for [`PressureOutlet`](Self::PressureOutlet), and
inflow carries the [`TwoFluidInflow`] state evaluated at `pressure`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `f64` | Static pressure at the face \[Pa\]. |
| `inflow` | `TwoFluidInflow` | The entering fluid. |

###### `VelocityInlet`

A prescribed face velocity \[m/s\] on both phases, positive in `+x`,
carrying the [`TwoFluidInflow`] state (evaluated at the adjacent
cell's start-of-step pressure) whenever it flows *into* the pipe
(gh:#1076). Outflow through it is zero-gradient, as for
[`PrescribedVelocity`](Self::PrescribedVelocity).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `velocity` | `f64` | Face velocity \[m/s\], positive in `+x`. |
| `inflow` | `TwoFluidInflow` | The entering fluid. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `TwoFluidInflowThermo`

How the thermal state of entering liquid is given (gh:#1076).

```rust
pub enum TwoFluidInflowThermo {
    LiquidTemperature(f64),
    LiquidEnthalpy(f64),
}
```

##### Variants

###### `LiquidTemperature`

Liquid temperature \[K\], at or below `T_sat` at the face pressure
(a superheated value is refused by the property layer, not clipped).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `LiquidEnthalpy`

Liquid specific enthalpy \[J/kg\], within the metastable bracket.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `TwoFluidInflow`

The state of fluid entering through an inflow boundary (gh:#1076):
liquid at the given temperature or enthalpy, vapour **saturated** at the
face pressure, in the volume ratio `void_fraction`. With the face pressure
that fixes `m_g = α ρ_g^sat`, `m_l = (1 − α) ρ_l(p, h_l)`, `h_g = h_g^sat`.

```rust
pub struct TwoFluidInflow {
    pub thermo: TwoFluidInflowThermo,
    pub void_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `thermo` | `TwoFluidInflowThermo` | Liquid thermal state. |
| `void_fraction` | `f64` | Entering vapour volume fraction \[-\], in `[0, 1)`. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `TwoFluidTimeDependentBoundary`

A time-dependent two-fluid boundary (gh:#1076): each quantity is a reused
[`TimeTable`] (`tampines_steam_tables::hem::TimeTable`, the type RELAP-7-
style time-dependent components take in `HemNetwork`). At the start of
every step the solver evaluates it at the current time `t^n` into the
static [`TwoFluidBoundary`] of the same shape, so a constant table gives
the static boundary exactly.

```rust
pub enum TwoFluidTimeDependentBoundary {
    PressureOutlet {
        pressure: tampines_steam_tables::hem::TimeTable,
    },
    PressureInlet {
        pressure: tampines_steam_tables::hem::TimeTable,
        liquid_temperature: tampines_steam_tables::hem::TimeTable,
        void_fraction: tampines_steam_tables::hem::TimeTable,
    },
    VelocityInlet {
        velocity: tampines_steam_tables::hem::TimeTable,
        liquid_temperature: tampines_steam_tables::hem::TimeTable,
        void_fraction: tampines_steam_tables::hem::TimeTable,
    },
}
```

##### Variants

###### `PressureOutlet`

[`TwoFluidBoundary::PressureOutlet`] with `p(t)` \[Pa\].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `tampines_steam_tables::hem::TimeTable` | Static pressure \[Pa\] against time \[s\]. |

###### `PressureInlet`

[`TwoFluidBoundary::PressureInlet`] with `p(t)`, `T_l(t)` and `α(t)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `tampines_steam_tables::hem::TimeTable` | Static pressure \[Pa\] against time. |
| `liquid_temperature` | `tampines_steam_tables::hem::TimeTable` | Liquid temperature \[K\] against time. |
| `void_fraction` | `tampines_steam_tables::hem::TimeTable` | Vapour volume fraction \[-\] against time. |

###### `VelocityInlet`

[`TwoFluidBoundary::VelocityInlet`] with `u(t)`, `T_l(t)` and `α(t)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `velocity` | `tampines_steam_tables::hem::TimeTable` | Face velocity \[m/s\] against time. |
| `liquid_temperature` | `tampines_steam_tables::hem::TimeTable` | Liquid temperature \[K\] against time. |
| `void_fraction` | `tampines_steam_tables::hem::TimeTable` | Vapour volume fraction \[-\] against time. |

##### Implementations

###### Methods

- ```rust
  pub fn at(self: &Self, t: Time) -> TwoFluidBoundary { /* ... */ }
  ```
  The static boundary this one prescribes at time `t`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    pub inlet_mass_flow: f64,
    pub inlet_energy_flow: f64,
    pub outlet_energy_flow: f64,
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
| `inlet_mass_flow` | `f64` | Mixture mass flow through the **left** face \[kg/s\], `+x` positive<br>(into the pipe), from the final corrector's donor fluxes (gh:#1076). |
| `inlet_energy_flow` | `f64` | Enthalpy flow `Σ_k ṁ_k h_k` through the left face \[W\], `+x`<br>positive, with the donor enthalpies the transport used (gh:#1076). |
| `outlet_energy_flow` | `f64` | Enthalpy flow through the **right** face \[W\], `+x` positive<br>(out of the pipe), likewise. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
  pub fn without_regime_map(self: Self) -> Self { /* ... */ }
  ```
  **Ablation** (gh:#1060): go back to one prescribed bubble diameter at

- ```rust
  pub fn without_interface_velocity(self: Self) -> Self { /* ... */ }
  ```
  **Ablation** (gh:#1060): carry the mass-transfer momentum at the

- ```rust
  pub fn without_total_enthalpy_mass_transfer(self: Self) -> Self { /* ... */ }
  ```
  **Ablation** (gh:#1060): the transferred mass carries the static

- ```rust
  pub fn set_wall_heat(self: &mut Self, wall_heat: PhasicWallHeat) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the wall heat boundary condition (gh:#1062, RELAP-7 Eq. 201).

- ```rust
  pub fn wall_heat(self: &Self) -> PhasicWallHeat { /* ... */ }
  ```
  The wall heat boundary condition.

- ```rust
  pub fn uses_interface_velocity(self: &Self) -> bool { /* ... */ }
  ```
  Whether the gh:#1060 interface-velocity terms are active.

- ```rust
  pub fn uses_total_enthalpy_mass_transfer(self: &Self) -> bool { /* ... */ }
  ```
  Whether the gh:#1060 total-enthalpy mass-transfer term is active.

- ```rust
  pub fn set_temperature_profile(self: &mut Self, temperatures: &[ThermodynamicTemperature]) -> Result<(), TampinesError> { /* ... */ }
  ```
  Overwrite the cell temperatures from an axial profile, re-deriving the

- ```rust
  pub fn set_left_boundary(self: &mut Self, bc: TwoFluidBoundary) { /* ... */ }
  ```
  Set the boundary condition at the `x = 0` end. Clears a

- ```rust
  pub fn set_right_boundary(self: &mut Self, bc: TwoFluidBoundary) { /* ... */ }
  ```
  Set the boundary condition at the `x = L` end. Clears a

- ```rust
  pub fn set_left_time_dependent_boundary(self: &mut Self, bc: TwoFluidTimeDependentBoundary) { /* ... */ }
  ```
  Set a time-dependent boundary at the `x = 0` end (gh:#1076),

- ```rust
  pub fn set_right_time_dependent_boundary(self: &mut Self, bc: TwoFluidTimeDependentBoundary) { /* ... */ }
  ```
  Set a time-dependent boundary at the `x = L` end (gh:#1076).

- ```rust
  pub fn left_boundary(self: &Self) -> TwoFluidBoundary { /* ... */ }
  ```
  The boundary currently in force at the `x = 0` end.

- ```rust
  pub fn right_boundary(self: &Self) -> TwoFluidBoundary { /* ... */ }
  ```
  The boundary currently in force at the `x = L` end.

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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
provenance**. Since gh:#1060 it sizes the bubbly/slug regime only; above
`α_g = 0.5` the regime map blends toward the annular core area.

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

## Module `wall_heat`

**Phasic wall heat** for the 1-D two-phase solvers (gh:#1062, RELAP-7
Theory Manual Eq. 201; the manual is not public, so only the equation
number is cited).

# What belongs here

The heat that crosses the pipe wall into each phase, as a volumetric
source \[W/m³\] for [`super::drift_flux::DriftFlux1d`] (one mixture energy
equation) and [`super::two_fluid::TwoFluid1d`] (one energy equation per
phase). The wall boundary condition itself is **reused**:
[`tampines_steam_tables::hem::WallHeat`] (`Adiabatic`, `HeatFlux`,
`WallTemperature` with a given `h`) is the same type `HemPipe` takes, and
[`PhasicWallHeat::Relap5WallTemperature`] takes `h` from the RELAP5
geometry-101/110 dispatcher
[`crate::heat_transfer::relap5_htc::relap5_wall_heat_transfer`] (gh:#1063)
from the phase's own properties.

# The phasic split, and which partition it is

The wall area per unit volume of a circular pipe is `4 / D_h`. Each phase
receives the heat through the fraction of the wall it wets, which is the
**linear contact partition** of the reused GeN-Foam closure
[`outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::interfacial::contact_partition::ContactPartition`]:
the vapour wets `α_g` of the wall and the liquid (`Complementary`)
`1 − α_g`. gh:#1060 brought a regime map for the *interfacial area*, not a
wall-contact map, so the linear partition is what is used; a boiling-curve
or film-boiling partition is **not** modelled.

- `HeatFlux(q'')`: `Q_k = c_k q'' 4/D_h`; the two shares sum to the
  prescribed heat exactly.
- `WallTemperature { T_w, h }`: `Q_k = c_k h (T_w − T_k) 4/D_h`, each phase
  against its own temperature.
- `Relap5WallTemperature { T_w, geometry }`: as above with `h_k` from the
  RELAP5 dispatcher evaluated on phase `k`'s density, viscosity,
  conductivity, `c_p`, thermal expansion and velocity. Single-phase
  convection correlations per phase — **no nucleate-boiling or CHF
  logic**, which the dispatcher does not carry.

# Default

[`PhasicWallHeat::default`] is adiabatic, like `HemPipe`'s: wall heat is a
boundary input, not physics a closure supplies, so "physics on by default"
does not apply to it.

# Honest scope

Explicit in the phase temperature (evaluated on the start-of-step state),
so a very large `h Δt / (ρ c_p D_h)` can overshoot; nothing here limits
it. Not validated against any experiment; the V&V record is
`crates/tampines/verification_and_validation/multiphase_1d_issues_1058_1060_1062_1076.md`.

```rust
pub mod wall_heat { /* ... */ }
```

### Types

#### Enum `PhasicWallHeat`

The wall heat boundary condition of a 1-D two-phase pipe (gh:#1062).

```rust
pub enum PhasicWallHeat {
    Hem(tampines_steam_tables::hem::WallHeat),
    Relap5WallTemperature {
        temperature: uom::si::f64::ThermodynamicTemperature,
        geometry: crate::heat_transfer::relap5_htc::Relap5HeatTransferGeometry,
    },
}
```

##### Variants

###### `Hem`

The reused `HemPipe` wall condition: adiabatic, a prescribed flux
\[W/m² of wall\], or a wall temperature with a given `h`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines_steam_tables::hem::WallHeat` |  |

###### `Relap5WallTemperature`

A wall at `temperature`, with each phase's `h` from the RELAP5
geometry-101/110 dispatcher on that phase's own properties.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Wall temperature. |
| `geometry` | `crate::heat_transfer::relap5_htc::Relap5HeatTransferGeometry` | RELAP5 heat-transfer geometry (101 pipe, 110 rod bundle). |

##### Implementations

###### Methods

- ```rust
  pub fn is_adiabatic(self: &Self) -> bool { /* ... */ }
  ```
  `true` for the adiabatic wall, which lets a solver skip all wall-heat

- ```rust
  pub fn needs_properties(self: &Self) -> bool { /* ... */ }
  ```
  `true` when evaluating this wall needs the phases' conductivity, `c_p`

- ```rust
  pub fn validate(self: &Self) -> Result<(), TampinesError> { /* ... */ }
  ```
  Refuse a non-finite flux, temperature or `h`.

- ```rust
  pub fn phasic_heat(self: &Self, void: f64, d_h: f64, vapour: WallPhaseState, liquid: WallPhaseState) -> Result<(f64, f64), TampinesError> { /* ... */ }
  ```
  Volumetric wall heat into `(vapour, liquid)` \[W/m³\] for a cell of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `WallPhaseState`

One phase's state as the wall sees it. Raw SI.

```rust
pub struct WallPhaseState {
    pub temperature: f64,
    pub density: f64,
    pub viscosity: f64,
    pub conductivity: f64,
    pub cp: f64,
    pub expansion: f64,
    pub velocity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `f64` | Bulk temperature \[K\]. |
| `density` | `f64` | Density \[kg/m³\]. |
| `viscosity` | `f64` | Dynamic viscosity \[Pa·s\]. |
| `conductivity` | `f64` | Thermal conductivity \[W/(m·K)\]. |
| `cp` | `f64` | Isobaric specific heat \[J/(kg·K)\]. |
| `expansion` | `f64` | Volumetric thermal expansion `β = −(1/ρ) ∂ρ/∂T|_p` \[1/K\]. |
| `velocity` | `f64` | Axial velocity \[m/s\]. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Re-export `PhasicWallHeat`

```rust
pub use wall_heat::PhasicWallHeat;
```

#### Re-export `WallPhaseState`

```rust
pub use wall_heat::WallPhaseState;
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

#### Re-export `TwoFluidInflow`

```rust
pub use two_fluid::TwoFluidInflow;
```

#### Re-export `TwoFluidInflowThermo`

```rust
pub use two_fluid::TwoFluidInflowThermo;
```

#### Re-export `TwoFluidReport`

```rust
pub use two_fluid::TwoFluidReport;
```

#### Re-export `TwoFluidTimeDependentBoundary`

```rust
pub use two_fluid::TwoFluidTimeDependentBoundary;
```

## Module `pcice`

PCICE: a pressure-corrected, semi-implicit solver for 1-D compressible flow
on a general equation of state, finite volume (gh:#1068).

> Research, education and V&V only. AI draft, unreviewed. Not for nuclear
> facility operation, reactor control, licensing or safety-critical
> decisions. See `RESPONSIBLE_USE.md`.

# What this module answers

*How do we march the compressible Euler equations with a time step set by
the flow speed `|u|` instead of the sound speed `|u| + c`?* An explicit
density-based scheme such as [`crate::hem::HemPipe`] (KNP) must resolve the
acoustic waves, so at low Mach number it takes `c/|u|` times more steps than
the material motion needs. PCICE treats the pressure implicitly, so the
step is limited by the material Courant number alone.

# References

- R. C. Martineau and R. A. Berry, "The pressure-corrected ICE finite element
  method for compressible flows on unstructured meshes", *J. Comput. Phys.*
  198 (2004) 659–685. The algorithm (explicit predictor without the pressure
  terms, a Crank–Nicolson pressure target reached through a pressure
  Poisson / Helmholtz equation closed by the EOS, then the corrected mass,
  momentum and total energy) is theirs; their discretisation is finite
  element, this one is a cell-centred finite volume.
- RELAP-7 Theory Manual, §5.3 (Eqs 257–274), which describes PCICE for the
  RELAP-7 single-phase equations. The manual is not public: it is cited by
  section and equation only, nothing is transcribed from it. This module was
  written from the open paper above and from first principles (clean room;
  no RELAP source was read).

# The equations

The quasi-1-D Euler equations in a duct of area `A(x)`:

- `∂(ρA)/∂t + ∂(ρuA)/∂x = 0`
- `∂(ρuA)/∂t + ∂((ρu² + p)A)/∂x = p ∂A/∂x`
- `∂(ρEA)/∂t + ∂((ρE + p)uA)/∂x = 0`, `E = e + ½u²`

closed by `p = p(ρ, e)` from any [`HemEquationOfState`] (ideal gas,
IAPWS-IF97, CoolProp through [`crate::hem::HemEos`], the reduced
verification closures).

# The algorithm, per step `Δt` (cell `i`, face `f`, volume `V_i = A_i Δx`)

1. **Explicit predictor, two-stage Runge–Kutta (Heun / SSP22).** Mass,
   momentum and total energy are advanced with their *advective* fluxes
   only, `ρu`, `ρu²`, `ρEu`: no pressure in the momentum flux, no pressure
   work in the energy flux. Face values are MUSCL-reconstructed with
   `outram_foam_basic_lib`'s [`FluxLimiter`] (the uniform-mesh form of
   `fvc::reconstruct_pos_neg`, as in `HemPipe`), and the face flux is a
   central flux with a local Lax–Friedrichs dissipation at the **material**
   speed `a = max(|u_L|, |u_R|)`, never `|u| + c`. The stage-averaged face
   fluxes `G*`, `F_m*`, `F_E*` are kept.
2. **Crank–Nicolson pressure target.** The pressure that acts over the step
   is `p^θ = (1 − θ) pⁿ + θ pⁿ⁺¹`, `θ = ½` by default
   ([`PciceNumerics::implicitness`]).
3. **Intermediate momentum and the corrected mass flux.** The face mass
   flux is the time average of `(ρu)ⁿ` and `(ρu)ⁿ⁺¹`, which is the predictor
   flux plus the pressure impulse over half the step:
   `G_f = G*_f − ½Δt (p^θ_R − p^θ_L)/d_f` (`d_f = Δx`, `Δx/2` at a pressure
   boundary). The compact face gradient is the Rhie–Chow-like coupling that
   keeps a collocated grid free of odd–even pressure modes. Momentum
   and energy fluxes follow: `F_m = F_m* + (G − G*) u*_f + p^θ_f`,
   `F_E = F_E* + (G − G*) E*_f + p^θ_f G/ρ*_f`, and the cell update is
   conservative, `V_i Δq_i = −Δt Σ_f ±A_f F_f` (+ `Δt p^θ_i (A_{i+½} − A_{i−½})`
   in momentum).
4. **Pressure-correction (Helmholtz) equation.** Linearise
   `pⁿ⁺¹ = p(ρ, ρE, ρu)` about the current iterate `p^k`:
   `δp_i − A_i δρ_i − B_i δ(ρE)_i − C_i δ(ρu)_i = p_EOS,i − p^k_i`, with
   `A = (∂p/∂ρ)_e − (∂p/∂e)_ρ (e − ½u²)/ρ`, `B = (∂p/∂e)_ρ/ρ`,
   `C = −(∂p/∂e)_ρ u/ρ`, and every `δq_i` a linear function of
   `δp_{i−1}, δp_i, δp_{i+1}` through step 3. That is a tridiagonal system,
   solved by [`crate::multiphase_1d::thomas_solve`]. For an ideal gas at
   rest it reduces to `δp − (cΔt/2Δx)² Δ²δp = r`, the Crank–Nicolson wave
   operator. **The EOS derivatives are finite differences** of
   [`HemEquationOfState::state_rho_e`] (forward, relative step `1e-6`),
   because the trait supplies no derivatives. They only shape the
   correction; the residual uses the EOS itself.
5. **Two correction passes** ([`PciceNumerics::correction_passes`]): the
   EOS, its derivatives and the lagged face coefficients are re-evaluated
   at the new pressure and step 4 is solved again.
6. **Energy update and closure.** With the final iterate, mass, momentum and
   total energy are updated once more (step 3) with the corrected mass flux
   and the time-centred pressure, and the new pressure is taken from the EOS
   at the new `(ρ, e)`, so the carried state is thermodynamically
   consistent. What is left of `p_EOS − p^k` is reported as
   [`PciceStepReport::pressure_residual`].

Every update is in flux form, so with two walls total mass and total energy
are conserved to round-off.

# Time step

`Δt = C_mat Δx / max|u|` ([`PciceNumerics::material_courant`]), capped by
[`PciceNumerics::max_dt`]. An optional acoustic cap
([`PciceNumerics::acoustic_courant_cap`]) is for problems where the
acoustic waves themselves are the answer (shock tubes); without it the
acoustic Courant number can be far above one, which is the method's point.

**Limit (measured, see the V&V record):** a fluid that starts at rest
against a strong pressure jump has no material speed to set the first step,
so `max_dt` sets it; on Sod with the default 1 ms (acoustic Courant ≈ 3.7)
the first step drives a density negative and is refused. Use the acoustic
cap, or a small `max_dt`, for such starts.

# Boundaries ([`PciceBoundary`])

- `Wall`: zero mass and energy flux, the cell pressure on the wall.
- `FixedPressure(p)`: the face pressure is `p`; the reconstruction ghost is
  the end cell (zero gradient), as `HemPipe` does.
- `Reservoir { p0, h0 }`: for inflow the face takes `p = p0 − ½ρu²` and
  `h = h0 − ½u²` with the end cell's velocity, density from the EOS
  (`state_p_h`); for outflow `p = p0` with the end cell's state.

# What is not here

Friction, gravity, heat sources, choked breaks and junctions (all in
`HemPipe`). Variable area is implemented (quasi-1-D, with the wall
pressure force) and verified only for a duct at rest.

# V&V

`crates/tampines/tests/pcice.rs`, recorded with methodology and results in
`crates/tampines/verification_and_validation/pcice.md`. The case setups are
public in [`cases`] so gh:#1069 (the finite-element comparison) can reuse
them.

```rust
pub mod pcice { /* ... */ }
```

### Modules

## Module `cases`

Reusable PCICE verification cases (gh:#1068), set up once so the
finite-element comparison (gh:#1069) and any other solver can run exactly
the same problems.

Each case is a [`PciceCase`]: geometry, EOS, initial condition, boundary
conditions, end time and the PCICE numerics the V&V record used. It builds
a [`PcicePipe`] ([`PciceCase::build_pcice`]) or the density-based
[`crate::hem::HemPipeOf`] on the same mesh ([`PciceCase::build_hem_pipe`],
inviscid like PCICE).

Methodology and results: `crates/tampines/verification_and_validation/pcice.md`.

```rust
pub mod cases { /* ... */ }
```

### Types

#### Struct `PciceCase`

One verification problem, solver-independent.

```rust
pub struct PciceCase<E: HemEquationOfState> {
    pub name: &'static str,
    pub geometry: tampines_steam_tables::hem::HemPipeGeometry,
    pub eos: E,
    pub initial: tampines_steam_tables::hem::HemInitialCondition,
    pub inlet: super::PciceBoundary,
    pub outlet: super::PciceBoundary,
    pub end_time: uom::si::f64::Time,
    pub numerics: super::PciceNumerics,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `&'static str` | Short name for messages. |
| `geometry` | `tampines_steam_tables::hem::HemPipeGeometry` | Mesh and flow area. |
| `eos` | `E` | Equation of state. |
| `initial` | `tampines_steam_tables::hem::HemInitialCondition` | Initial condition. |
| `inlet` | `super::PciceBoundary` | Boundary at `x = 0`. |
| `outlet` | `super::PciceBoundary` | Boundary at `x = L`. |
| `end_time` | `uom::si::f64::Time` | Time at which the case is compared. |
| `numerics` | `super::PciceNumerics` | The PCICE numerics the V&V record used for this case. |

##### Implementations

###### Methods

- ```rust
  pub fn build_pcice(self: &Self) -> Result<PcicePipe<E>, TampinesError> { /* ... */ }
  ```
  A [`PcicePipe`] set up for this case with [`Self::numerics`].

- ```rust
  pub fn build_hem_pipe(self: &Self, numerics: HemNumerics) -> Result<HemPipe<E>, TampinesError> { /* ... */ }
  ```
  The density-based KNP `HemPipe` on the same mesh, EOS, initial state

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `sod_end_time`

Sod's comparison time: dimensionless `t = 0.2` on the 10 m tube,
`0.2 · 10 / √(1e5)` s.

```rust
pub fn sod_end_time() -> uom::si::f64::Time { /* ... */ }
```

#### Function `sod_ideal_gas`

**Sod's shock tube** in SI (air, `γ = 1.4`), closed at both ends (the
waves do not reach the ends by [`sod_end_time`]). PCICE numerics: material
Courant 0.5, acoustic Courant capped at 0.5, since the shock and the
rarefaction are the answer here.

# Errors

Only for `n_cells = 0`.

```rust
pub fn sod_ideal_gas(n_cells: usize) -> Result<PciceCase<tampines_steam_tables::hem::IdealGasHem>, crate::TampinesError> { /* ... */ }
```

#### Function `low_mach_advection_exact_density`

The exact low-Mach advection solution: uniform `p` and `u`, and the
temperature bump carried with the flow, `ρ(x, t) = p / (R T0(x − u t))`.

```rust
pub fn low_mach_advection_exact_density(x: uom::si::f64::Length, t: uom::si::f64::Time) -> uom::si::f64::MassDensity { /* ... */ }
```

#### Function `low_mach_advection`

**Low-Mach advection** of a temperature (density) bump in air at
`u = 1 m/s`, `c ≈ 347 m/s`: a 1 m duct, a stagnation reservoir at the inlet
holding the background state and a fixed pressure at the outlet. The step
is set by the material Courant number 0.5 alone, so the acoustic Courant
number is about 175. Compared at `t = 0.3 s`.

# Errors

Only for `n_cells = 0`.

```rust
pub fn low_mach_advection(n_cells: usize) -> Result<PciceCase<tampines_steam_tables::hem::IdealGasHem>, crate::TampinesError> { /* ... */ }
```

#### Function `water_shock_tube`

**Water shock tube on IAPWS-IF97**: compressed liquid at 440 K, 10 MPa on
the left and 1 MPa on the right (`T_sat(1 MPa) ≈ 453 K`, so both sides stay
liquid), a 10 m closed tube, compared at 2 ms. Acoustic Courant capped at
0.5, as for Sod.

# Errors

Only for `n_cells = 0`.

```rust
pub fn water_shock_tube(n_cells: usize) -> Result<PciceCase<tampines_steam_tables::hem::Iapws97Hem>, crate::TampinesError> { /* ... */ }
```

#### Function `nozzle_at_rest`

**A converging–diverging duct at rest** (areas 2 : 1 : 2 m², throat at mid
length, 1 m, air at 1e5 Pa and 300 K, walls). The quasi-1-D wall pressure
force must balance the face pressure forces exactly, so nothing moves.

# Errors

Only for `n_cells = 0`.

```rust
pub fn nozzle_at_rest(n_cells: usize) -> Result<PciceCase<tampines_steam_tables::hem::IdealGasHem>, crate::TampinesError> { /* ... */ }
```

### Constants and Statics

#### Constant `SOD_LEFT`

Sod's left state `(p, ρ, u)` in SI: `(1e5 Pa, 1 kg/m³, 0)`.

```rust
pub const SOD_LEFT: (f64, f64, f64) = _;
```

#### Constant `SOD_RIGHT`

Sod's right state: `(1e4 Pa, 0.125 kg/m³, 0)`.

```rust
pub const SOD_RIGHT: (f64, f64, f64) = _;
```

#### Constant `SOD_LENGTH_M`

Sod's tube length (m) and diaphragm position (m), as in
`tampines-steam-tables`' `hem_pipe_verification.rs`.

```rust
pub const SOD_LENGTH_M: f64 = 10.0;
```

#### Constant `SOD_DIAPHRAGM_M`

Diaphragm position, m.

```rust
pub const SOD_DIAPHRAGM_M: f64 = 5.0;
```

#### Constant `LOW_MACH_PRESSURE_PA`

Low-Mach advection: background pressure, Pa.

```rust
pub const LOW_MACH_PRESSURE_PA: f64 = 1.0e5;
```

#### Constant `LOW_MACH_T0_K`

Background temperature, K.

```rust
pub const LOW_MACH_T0_K: f64 = 300.0;
```

#### Constant `LOW_MACH_DT_K`

Temperature bump amplitude, K.

```rust
pub const LOW_MACH_DT_K: f64 = 30.0;
```

#### Constant `LOW_MACH_X0_M`

Bump centre at `t = 0`, m.

```rust
pub const LOW_MACH_X0_M: f64 = 0.3;
```

#### Constant `LOW_MACH_SIGMA_M`

Bump width `σ` in `exp(−((x − x0)/σ)²)`, m.

```rust
pub const LOW_MACH_SIGMA_M: f64 = 0.05;
```

#### Constant `LOW_MACH_U_MS`

Flow velocity, m/s (Mach ≈ 0.003).

```rust
pub const LOW_MACH_U_MS: f64 = 1.0;
```

### Types

#### Enum `PciceBoundary`

A boundary condition at one end of a [`PcicePipe`].

```rust
pub enum PciceBoundary {
    Wall,
    FixedPressure(uom::si::f64::Pressure),
    Reservoir {
        stagnation_pressure: uom::si::f64::Pressure,
        stagnation_enthalpy: uom::si::f64::AvailableEnergy,
    },
}
```

##### Variants

###### `Wall`

Closed rigid end: no mass or energy flux; the end cell's pressure acts
on the wall.

###### `FixedPressure`

Fixed static pressure at the face; inflow brings the end cell's
density and energy (zero gradient).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Pressure` |  |

###### `Reservoir`

Stagnation reservoir `(p0, h0)`: inflow at `p = p0 − ½ρu²`,
`h = h0 − ½u²`; outflow at `p = p0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `stagnation_pressure` | `uom::si::f64::Pressure` | Stagnation pressure `p0`. |
| `stagnation_enthalpy` | `uom::si::f64::AvailableEnergy` | Stagnation specific enthalpy `h0`, J/kg. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `PciceNumerics`

Numerical settings of a [`PcicePipe`].

```rust
pub struct PciceNumerics {
    pub material_courant: uom::si::f64::Ratio,
    pub acoustic_courant_cap: Option<uom::si::f64::Ratio>,
    pub max_dt: uom::si::f64::Time,
    pub limiter: tampines_steam_tables::hem::FluxLimiter,
    pub correction_passes: usize,
    pub implicitness: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `material_courant` | `uom::si::f64::Ratio` | Target material Courant number `max|u| Δt/Δx`. Default `0.5`. |
| `acoustic_courant_cap` | `Option<uom::si::f64::Ratio>` | Optional cap on the acoustic Courant number `max(|u| + c) Δt/Δx`.<br>Default `None`: the step is set by the material speed only. |
| `max_dt` | `uom::si::f64::Time` | Largest step allowed (also the step of a fluid at rest). Default 1 ms. |
| `limiter` | `tampines_steam_tables::hem::FluxLimiter` | Reconstruction limiter of the predictor. Default van Leer. |
| `correction_passes` | `usize` | Pressure-correction passes per step, `≥ 1`. Default `2`. |
| `implicitness` | `uom::si::f64::Ratio` | Time weight `θ` of the new pressure, in `[½, 1]`: `½` is Crank–Nicolson<br>(the default, PCICE's choice), `1` backward Euler. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `PciceStepReport`

What one [`PcicePipe::step`] did.

```rust
pub struct PciceStepReport {
    pub time: uom::si::f64::Time,
    pub dt: uom::si::f64::Time,
    pub material_courant: uom::si::f64::Ratio,
    pub acoustic_courant: uom::si::f64::Ratio,
    pub pressure_residual: uom::si::f64::Ratio,
    pub min_pressure: uom::si::f64::Pressure,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time` | `uom::si::f64::Time` | Time at the end of the step. |
| `dt` | `uom::si::f64::Time` | Step taken. |
| `material_courant` | `uom::si::f64::Ratio` | `max|u| Δt/Δx` at the start of the step. |
| `acoustic_courant` | `uom::si::f64::Ratio` | `max(|u| + c) Δt/Δx` at the start of the step. |
| `pressure_residual` | `uom::si::f64::Ratio` | `max_i |p_EOS,i − p^k_i| / p_EOS,i` after the last correction pass:<br>how far the linearised pressure was from the EOS at the new state. |
| `min_pressure` | `uom::si::f64::Pressure` | Lowest cell pressure at the end of the step. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `PciceRunSummary`

What [`PcicePipe::run_until`] did.

```rust
pub struct PciceRunSummary {
    pub steps: usize,
    pub time: uom::si::f64::Time,
    pub max_material_courant: uom::si::f64::Ratio,
    pub max_acoustic_courant: uom::si::f64::Ratio,
    pub max_pressure_residual: uom::si::f64::Ratio,
    pub min_pressure: uom::si::f64::Pressure,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `steps` | `usize` | Steps taken. |
| `time` | `uom::si::f64::Time` | Time reached (exactly the requested end time). |
| `max_material_courant` | `uom::si::f64::Ratio` | Largest material Courant number of any step. |
| `max_acoustic_courant` | `uom::si::f64::Ratio` | Largest acoustic Courant number of any step. |
| `max_pressure_residual` | `uom::si::f64::Ratio` | Largest pressure residual of any step. |
| `min_pressure` | `uom::si::f64::Pressure` | Lowest cell pressure at the end of any step. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `PcicePipe`

The PCICE pipe solver, generic over the equation of state.

Built from the same geometry and initial-condition types as
[`crate::hem::HemPipe`] ([`HemPipeGeometry`], [`HemInitialCondition`]) and
reporting the same [`HemSnapshot`], so the two solvers run the same case
and compare cell by cell. Both ends start as walls.

```rust
pub struct PcicePipe<E: HemEquationOfState> {
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
  pub fn new(geometry: HemPipeGeometry, eos: E, initial: HemInitialCondition, numerics: PciceNumerics) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Build the pipe and close its initial state with `eos`. Both ends start

- ```rust
  pub fn set_boundary(self: &mut Self, end: PipeEnd, boundary: PciceBoundary) -> Result<(), TampinesError> { /* ... */ }
  ```
  Set the boundary condition at one end (effective from the next step).

- ```rust
  pub fn boundary(self: &Self, end: PipeEnd) -> PciceBoundary { /* ... */ }
  ```
  The boundary condition at one end.

- ```rust
  pub fn geometry(self: &Self) -> &HemPipeGeometry { /* ... */ }
  ```
  The geometry.

- ```rust
  pub fn equation_of_state(self: &Self) -> &E { /* ... */ }
  ```
  The equation of state.

- ```rust
  pub fn numerics(self: &Self) -> &PciceNumerics { /* ... */ }
  ```
  The numerical settings.

- ```rust
  pub fn time(self: &Self) -> Time { /* ... */ }
  ```
  Simulation time.

- ```rust
  pub fn snapshot(self: &Self) -> HemSnapshot { /* ... */ }
  ```
  Every cell's state, in the same form as [`crate::hem::HemPipe`]'s.

- ```rust
  pub fn total_mass(self: &Self) -> Mass { /* ... */ }
  ```
  Total mass in the pipe, `Σ ρ_i V_i`.

- ```rust
  pub fn total_energy(self: &Self) -> Energy { /* ... */ }
  ```
  Total (internal + kinetic) energy in the pipe, `Σ ρE_i V_i`.

- ```rust
  pub fn net_mass_inflow(self: &Self) -> Mass { /* ... */ }
  ```
  Net mass that has entered through both ends since construction

- ```rust
  pub fn net_energy_inflow(self: &Self) -> Energy { /* ... */ }
  ```
  Net total energy (`ρuH` flux) that has entered through both ends.

- ```rust
  pub fn stable_time_step(self: &Self) -> Time { /* ... */ }
  ```
  The step the controller would take now: material Courant, capped by

- ```rust
  pub fn step(self: &mut Self) -> Result<PciceStepReport, TampinesError> { /* ... */ }
  ```
  Take one step of [`Self::stable_time_step`].

- ```rust
  pub fn step_by(self: &mut Self, dt: Time) -> Result<PciceStepReport, TampinesError> { /* ... */ }
  ```
  Take one step of `dt`. The material Courant number is not checked: a

- ```rust
  pub fn run_until(self: &mut Self, t_end: Time) -> Result<PciceRunSummary, TampinesError> { /* ... */ }
  ```
  Step until `t_end` (the last step is shortened to land on it).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `htr10_isothermal_coefficient_nrg_20_to_120c`

HTR-10 **isothermal** temperature coefficient of reactivity over
20-120 degrees Celsius as calculated by **NRG**: -7.37e-5 per degree
(equivalently -7.37 pcm per kelvin).

Source: IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-33 (Open tier;
catalogued at
`iaea-tecdoc-1382-part2`, proprietary since 2026-09-22, held in the maintainer's private literature repository; see `crates/kovan-literature/CATALOGUE.md`). The
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    pub correlation_window: super::triso::CorrelationWindow,
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
| `correlation_window` | `super::triso::CorrelationWindow` | Coded temperature window of the matrix-graphite and particle-layer<br>correlations. [`CorrelationWindow::Standard`] from every constructor;<br>widen it with [`Pebble::with_correlation_window`] (which widens the<br>particle's to match, so the two scales cannot disagree). |

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
  pub fn htr10_high_temperature() -> Self { /* ... */ }
  ```
  The HTR-10 fuel element of [`Self::htr10`] with the whole stack —

- ```rust
  pub fn with_correlation_window(self: Self, window: CorrelationWindow) -> Self { /* ... */ }
  ```
  The same pebble with its matrix graphite and its particle both

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

- ```rust
  pub fn volume_average_temperature(self: &Self, profile: &PebbleTemperatureProfile) -> ThermodynamicTemperature { /* ... */ }
  ```
  Volume-average matrix temperature of the whole pebble, given a profile

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `htr10_silicon_carbide_density`

The HTR-10 coated particle's published SiC coating density, 3.18 g/cm^3
(IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-17, Open tier — the same row
[`TrisoParticle::htr10`] transcribes its geometry and carbon densities
from). Not stored on [`TrisoParticle`] because the SiC conductivity
correlation does not use it; a heat-capacity (thermal-mass) calculation
does. Added 2026-09-28 for `htgr_sim_v1`'s fuel node.

```rust
pub fn htr10_silicon_carbide_density() -> uom::si::f64::MassDensity { /* ... */ }
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
`iaea-tecdoc-1382-part2`, proprietary since 2026-09-22, held in the maintainer's private literature repository; see `crates/kovan-literature/CATALOGUE.md`),
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

#### Enum `CorrelationWindow`

Which coded temperature window the pebble-stack conductivity correlations
(matrix graphite, UO2, buffer, PyC, SiC) are evaluated in.

A closed set, enum-dispatched per the workspace Rust design rules. Added
2026-09-28 (gh:#350, gh:#351) so a transient that takes the fuel past
2000 K keeps a resolved pebble and kernel temperature instead of refusing.

**Neither window is a published validity range.** The 300-2000 K window
was adopted to match `tuas_boussinesq_solver`'s graphite cp table (see
[`MIN_TEMPERATURE_KELVIN`]); the Virtual Test Bed deck behind every layer
correlation states no range at all.

```rust
pub enum CorrelationWindow {
    Standard,
    HighTemperature,
}
```

##### Variants

###### `Standard`

300 K to 2000 K, matrix graphite from `NuclearGraphiteMatrixA3`. The
default, and the window every recorded `tampines` V&V number was
measured in.

###### `HighTemperature`

300 K to 3000 K, matrix graphite from
`NuclearGraphiteMatrixA3HighTemp`. **Above 2000 K every conductivity in
the stack is extrapolated**: the A3 matrix correlation (its measured
range — Gontard & Nabielek 1990 — is not yet confirmed; its fitted form
has a minimum at 2029.9 K and rises above it), the PyC/buffer and SiC
forms (no source named in the VTB deck), and the UO2 form. The UO2
expression is structurally the Fink (2000) recommendation at 100 %
theoretical density (the VTB constants 115.8 and 7410.5 are Fink's
100 and 6400 divided by the 95 %-TD porosity factor 0.95/1.1), a
correlation usually quoted to ~3120 K — **not re-checked against Fink
in this change**, so it is flagged with the rest. Use this window to
keep a transient's energy balance and feedback continuous past 2000 K,
never to quote a validated fuel temperature there.

##### Implementations

###### Methods

- ```rust
  pub fn min_kelvin(self: Self) -> f64 { /* ... */ }
  ```
  Lowest temperature of the window, kelvin (300 K for both).

- ```rust
  pub fn max_kelvin(self: Self) -> f64 { /* ... */ }
  ```
  Highest temperature of the window, kelvin.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    pub correlation_window: CorrelationWindow,
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
| `correlation_window` | `CorrelationWindow` | Coded temperature window the layer correlations are evaluated in.<br>[`CorrelationWindow::Standard`] from every constructor; widen it with<br>[`TrisoParticle::with_correlation_window`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(kernel_radius: Length, buffer_outer_radius: Length, inner_pyc_outer_radius: Length, silicon_carbide_outer_radius: Length, outer_pyc_outer_radius: Length, buffer_density: MassDensity, pyrocarbon_density: MassDensity) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Builds a TRISO particle from its five outer radii and the two carbon

- ```rust
  pub fn with_correlation_window(self: Self, window: CorrelationWindow) -> Self { /* ... */ }
  ```
  The same particle with its layer correlations evaluated in `window`.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `uranium_dioxide_thermal_conductivity_in`

[`uranium_dioxide_thermal_conductivity`] evaluated in an explicit
[`CorrelationWindow`] — **extrapolated above 2000 K** under
[`CorrelationWindow::HighTemperature`].

```rust
pub fn uranium_dioxide_thermal_conductivity_in(temperature: uom::si::f64::ThermodynamicTemperature, window: CorrelationWindow) -> Result<uom::si::f64::ThermalConductivity, crate::TampinesError> { /* ... */ }
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

#### Function `pyrocarbon_thermal_conductivity_in`

[`pyrocarbon_thermal_conductivity`] evaluated in an explicit
[`CorrelationWindow`] — **extrapolated above 2000 K** under
[`CorrelationWindow::HighTemperature`].

```rust
pub fn pyrocarbon_thermal_conductivity_in(temperature: uom::si::f64::ThermodynamicTemperature, density: uom::si::f64::MassDensity, fluence: FastNeutronFluence, window: CorrelationWindow) -> Result<uom::si::f64::ThermalConductivity, crate::TampinesError> { /* ... */ }
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

#### Function `silicon_carbide_thermal_conductivity_in`

[`silicon_carbide_thermal_conductivity`] evaluated in an explicit
[`CorrelationWindow`] — **extrapolated above 2000 K** under
[`CorrelationWindow::HighTemperature`].

```rust
pub fn silicon_carbide_thermal_conductivity_in(temperature: uom::si::f64::ThermodynamicTemperature, fluence: FastNeutronFluence, window: CorrelationWindow) -> Result<uom::si::f64::ThermalConductivity, crate::TampinesError> { /* ... */ }
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

#### Constant `HIGH_TEMPERATURE_MAX_KELVIN`

Highest temperature, 3000 K, of the [`CorrelationWindow::HighTemperature`]
window — the published upper limit of Butland & Maddison's graphite cp
polynomial, which is the only property in the pebble stack backed that far.
Everything else evaluated between 2000 K and this is **extrapolated**; see
[`CorrelationWindow::HighTemperature`].

```rust
pub const HIGH_TEMPERATURE_MAX_KELVIN: f64 = 3000.0;
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Re-export `CorrelationWindow`

```rust
pub use triso::CorrelationWindow;
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
pub use tuas_boussinesq_solver::array_fluid_collections::fluid_array_lateral_coupling::FluidArray as SinglePhaseFluidArray;
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

## Module `subchannel`

Steady-state single-phase subchannel solver with diversion crossflow and
turbulent mixing (gh:#1078, RELAP-7 parity epic gh:#1056).

**Research, education and V&V only. AI draft, unreviewed.**

# Provenance (clean room)

Written from the open, textbook COBRA-type subchannel formulation:

- N. E. Todreas & M. S. Kazimi, *Nuclear Systems*, Vol. I (2nd ed., CRC
  Press, 2011) and Vol. II (1990), the subchannel control-volume balance
  equations and the turbulent-mixing definition `w'_ij = β s_ij Ḡ`.
  Section and equation numbers are **not re-checked**: neither book is in
  the kovan corpus.
- D. S. Rowe, *COBRA-IIIC: A digital computer program for steady state and
  transient thermal-hydraulic analysis of rod bundle nuclear fuel
  elements*, BNWL-1695, Battelle Pacific Northwest Laboratories (1973):
  the lateral-momentum equation with a gap loss coefficient, and the
  turbulent-momentum factor `f_T`. Not re-checked (not in the corpus).

RELAP-7's subchannel component (RELAP-7 Theory Manual, INL/EXT-14-31366,
§6.15, Eqs 408–411) is the parity target named by gh:#1078; that manual is
private and was **not read**. Nothing here is taken from it or from any
RELAP source.

# What was searched before writing this (workspace rule)

`grep -ri "subchannel\|crossflow\|cross-flow\|turbulent mixing"` over
`crates/` (2026-11-01): `bedok`'s channels are closed (no crossflow,
`singleflow1devap.rs`); the `outram-foam-appbuilder-lib` hits are GeN-Foam
drag closures, not a solver; the `dwsim` and digital-twin hits are
shell-side crossflow in heat exchangers. No subchannel solver existed.
Reused: the wall-friction closure [`crate::closures::wall_friction`]
(RELAP5 four-region Darcy factor), PETIR's sparse LU
([`petir::sparse::SparseLu`]), and GSL's forward-difference step rule
([`petir::multiroots::GSL_SQRT_DBL_EPSILON`]). `petir::multiroots::fdjacobian`
itself was checked and not used: it fills a dense `n × n` matrix, which is
1 GB at a 25-channel, 100-cell bundle; the Jacobian here is built by
column colouring on its known block band instead (same step rule).

# Model

`N` subchannels of constant flow area `A_i`, each discretised into `N_z`
equal axial cells of height `Δz`, connected by gaps `g = (i → j)` of width
`s` and centroid distance `l`. Fluid properties (`ρ`, `c_p`, `μ`) are
constant: incompressible, no buoyancy. Unknowns: `ṁ_i`, `T_i`, `p_i` at the
axial nodes and the diversion crossflow `w_g` (kg m⁻¹ s⁻¹, positive
`i → j`) in each cell. Per cell `k` (overbars are cell averages of the two
nodes, `u = ṁ/(ρA)`, `e_gi = +1` for the gap's `from` channel and `-1` for
its `to` channel):

```text
mass      ṁ_i(k+1) − ṁ_i(k) + Δz Σ_g e_gi w_g = 0
energy    c_p[ṁT]_k^{k+1} − q'_i Δz + Δz Σ_g e_gi [w_g c_p T* + w'_g c_p (T̄_i − T̄_j)] = 0
axial     [ṁ²/(ρA)]_k^{k+1} + Δz Σ_g e_gi [w_g u* + f_T w'_g (ū_i − ū_j)]
            + A_i (p_i(k+1) − p_i(k)) + A_i Δz ρ (a_f(ū_i) + g) = 0
lateral   [(u_g w_g)_k − (u_g w_g)_{k−1}]/Δz − (s/l)(p̄_i − p̄_j) + K w_g|w_g|/(2ρ s l) = 0
```

`T*`, `u*` are the donor (upwind across the gap) cell values; `u_g` is the
mean of the two channels' axial velocities; `w_{−1} = 0` (no crossflow
upstream of the inlet); `a_f` is
[`wall_friction_acceleration`](crate::closures::wall_friction::wall_friction_acceleration).
The turbulent mixing rate is `w'_g = β s |Ḡ|`, `Ḡ` the mean of the two
channels' mass fluxes. It is **on by default** (the caller supplies `β`);
[`SubchannelProblem::without_turbulent_mixing`] ablates it. Mixing moves
equal and opposite mass, so it carries energy and momentum but no net
mass; its momentum effect carries the factor `f_T` (default 1).

Boundary conditions: each channel's inlet mass flow and temperature, and a
uniform outlet pressure. The whole system is solved at once by Newton's
method (finite-difference Jacobian, sparse LU, backtracking).

Every balance is in conservative form and the crossflow and mixing terms
are antisymmetric between the two channels of a gap, so the bundle's total
mass and energy are conserved to the Newton residual (V&V tests 2 and 4).

# What it does not do

- No two-phase flow, no density change with temperature (so no buoyancy
  and no thermal acceleration), no spacer-grid form losses, no rod
  conduction or wall temperature (the heat input is the linear power into
  the coolant). Those are named gaps, not silent omissions.
- Dense storage is avoided, but the linear solve is a general sparse LU of
  the whole bundle; no claim is made about large-bundle run time.

V&V: `crates/tampines/verification_and_validation/subchannel_crossflow_mixing.md`.

```rust
pub mod subchannel { /* ... */ }
```

### Types

#### Type Alias `LinearMassRate`

A mass flow per unit length (kg m⁻¹ s⁻¹): the diversion crossflow `w` and
the turbulent mixing rate `w'` per unit axial length. uom has no quantity
of that name; its dimension, M L⁻¹ T⁻¹, is that of dynamic viscosity, so
it is stored as one (`Pa·s ≡ kg m⁻¹ s⁻¹`). Read it with
`get::<pascal_second>()` as kg m⁻¹ s⁻¹.

```rust
pub type LinearMassRate = uom::si::f64::DynamicViscosity;
```

#### Struct `FluidProperties`

Constant fluid properties (incompressible single phase).

```rust
pub struct FluidProperties {
    pub density: uom::si::f64::MassDensity,
    pub specific_heat: uom::si::f64::SpecificHeatCapacity,
    pub viscosity: uom::si::f64::DynamicViscosity,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `uom::si::f64::MassDensity` | Density `ρ`. |
| `specific_heat` | `uom::si::f64::SpecificHeatCapacity` | Specific heat `c_p`; enthalpy is `c_p T`. |
| `viscosity` | `uom::si::f64::DynamicViscosity` | Dynamic viscosity `μ`, for the Reynolds number of the wall friction. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `AxialHeat`

The linear power into one subchannel's coolant.

```rust
pub enum AxialHeat {
    Uniform(uom::si::f64::LinearPowerDensity),
    PerCell(Vec<uom::si::f64::LinearPowerDensity>),
}
```

##### Variants

###### `Uniform`

The same `q'` in every axial cell.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::LinearPowerDensity` |  |

###### `PerCell`

One `q'` per axial cell, inlet first; the length must equal the
problem's `n_axial`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<uom::si::f64::LinearPowerDensity>` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Subchannel`

One subchannel: geometry, wall roughness, inlet state and heating.

```rust
pub struct Subchannel {
    pub flow_area: uom::si::f64::Area,
    pub hydraulic_diameter: uom::si::f64::Length,
    pub roughness: uom::si::f64::Length,
    pub shape_factor: f64,
    pub inlet_mass_flow: uom::si::f64::MassRate,
    pub inlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub heat: AxialHeat,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `flow_area` | `uom::si::f64::Area` | Flow area `A`. |
| `hydraulic_diameter` | `uom::si::f64::Length` | Hydraulic diameter `D_h = 4A/P_w`. |
| `roughness` | `uom::si::f64::Length` | Absolute wall roughness `ε` (the friction closure takes `ε/D_h`). |
| `shape_factor` | `f64` | Laminar shape factor `Φ_S` of the friction closure (1 = circular). |
| `inlet_mass_flow` | `uom::si::f64::MassRate` | Inlet mass flow (must be positive). |
| `inlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | Inlet temperature. |
| `heat` | `AxialHeat` | Linear power into the coolant. |

##### Implementations

###### Methods

- ```rust
  pub fn new(flow_area: Area, hydraulic_diameter: Length, inlet_mass_flow: MassRate, inlet_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  A smooth (`ε = 0`), circular-shape-factor (`Φ_S = 1`), unheated

- ```rust
  pub fn with_heat(self: Self, heat: AxialHeat) -> Self { /* ... */ }
  ```
  Set the linear power into the coolant.

- ```rust
  pub fn with_roughness(self: Self, roughness: Length) -> Self { /* ... */ }
  ```
  Set the absolute wall roughness.

- ```rust
  pub fn with_shape_factor(self: Self, shape_factor: f64) -> Self { /* ... */ }
  ```
  Set the laminar shape factor `Φ_S`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Gap`

A gap between subchannels `from` and `to`; crossflow is positive from
`from` to `to`.

```rust
pub struct Gap {
    pub from: usize,
    pub to: usize,
    pub width: uom::si::f64::Length,
    pub centroid_distance: uom::si::f64::Length,
    pub loss_coefficient: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `usize` | Index of the first channel. |
| `to` | `usize` | Index of the second channel. |
| `width` | `uom::si::f64::Length` | Gap width `s` (rod-to-rod clearance). |
| `centroid_distance` | `uom::si::f64::Length` | Centroid-to-centroid distance `l`, the lateral control-volume length. |
| `loss_coefficient` | `f64` | Gap loss coefficient `K` on the lateral velocity head (dimensionless). |

##### Implementations

###### Methods

- ```rust
  pub fn new(from: usize, to: usize, width: Length, centroid_distance: Length, loss_coefficient: f64) -> Self { /* ... */ }
  ```
  A gap between channels `from` and `to`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `TurbulentMixing`

Turbulent mixing parameters.

```rust
pub struct TurbulentMixing {
    pub beta: f64,
    pub momentum_factor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `beta` | `f64` | Mixing coefficient `β` in `w' = β s Ḡ`. |
| `momentum_factor` | `f64` | Momentum factor `f_T` on the axial-momentum exchange. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `SubchannelProblem`

A steady single-phase subchannel problem. Build with
[`SubchannelProblem::new`], solve with [`SubchannelProblem::solve`].

```rust
pub struct SubchannelProblem {
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
  pub fn new(channels: Vec<Subchannel>, gaps: Vec<Gap>, fluid: FluidProperties, length: Length, n_axial: usize, outlet_pressure: Pressure, beta: f64) -> Self { /* ... */ }
  ```
  A bundle of `channels` joined by `gaps`, heated length `length` in

- ```rust
  pub fn without_turbulent_mixing(self: Self) -> Self { /* ... */ }
  ```
  Ablation: no turbulent mixing (energy or momentum). An explicit act;

- ```rust
  pub fn with_momentum_mixing_factor(self: Self, factor: f64) -> Self { /* ... */ }
  ```
  Set the turbulent-momentum factor `f_T` (0 ablates the momentum part

- ```rust
  pub fn with_gravity_along_flow(self: Self, g: Acceleration) -> Self { /* ... */ }
  ```
  Set the component of gravity opposing the flow (`g cos θ`, θ from the

- ```rust
  pub fn mixing(self: &Self) -> Option<TurbulentMixing> { /* ... */ }
  ```
  The turbulent mixing in force, `None` when ablated.

- ```rust
  pub fn solve(self: &Self) -> Result<SubchannelSolution, TampinesError> { /* ... */ }
  ```
  Solve the steady state.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `SubchannelSolution`

The steady state of a [`SubchannelProblem`].

```rust
pub struct SubchannelSolution {
    pub z: Vec<uom::si::f64::Length>,
    pub mass_flow: Vec<Vec<uom::si::f64::MassRate>>,
    pub temperature: Vec<Vec<uom::si::f64::ThermodynamicTemperature>>,
    pub pressure: Vec<Vec<uom::si::f64::Pressure>>,
    pub crossflow: Vec<Vec<LinearMassRate>>,
    pub turbulent_mixing: Vec<Vec<LinearMassRate>>,
    pub newton_iterations: usize,
    pub residual_inf_norm: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `z` | `Vec<uom::si::f64::Length>` | Axial node positions, `n_axial + 1` of them, inlet at 0. |
| `mass_flow` | `Vec<Vec<uom::si::f64::MassRate>>` | Mass flow, `[channel][node]`. |
| `temperature` | `Vec<Vec<uom::si::f64::ThermodynamicTemperature>>` | Temperature, `[channel][node]`. |
| `pressure` | `Vec<Vec<uom::si::f64::Pressure>>` | Pressure, `[channel][node]`. |
| `crossflow` | `Vec<Vec<LinearMassRate>>` | Diversion crossflow per unit length, `[gap][cell]`, positive<br>`from → to`. |
| `turbulent_mixing` | `Vec<Vec<LinearMassRate>>` | Turbulent mixing rate per unit length `w' = β s |Ḡ|`, `[gap][cell]`<br>(zero when mixing is ablated). |
| `newton_iterations` | `usize` | Newton iterations taken. |
| `residual_inf_norm` | `f64` | Infinity norm of the scaled residual at the returned state. |

##### Implementations

###### Methods

- ```rust
  pub fn total_mass_flow(self: &Self, node: usize) -> MassRate { /* ... */ }
  ```
  The bundle's total mass flow at axial node `node`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Constant `TYPICAL_MIXING_COEFFICIENT_BETA`

A representative turbulent mixing coefficient `β` for rod bundles, 0.02.

The open literature puts `β` for PWR/BWR-type rod-bundle gaps at roughly
0.005–0.05 (Rogers & Rosehart's review, and the subchannel chapter of
Todreas & Kazimi, *Nuclear Systems*); 0.02 is a mid-range value. **Not
re-checked** against a source in the corpus. It is a starting value for a
caller, never a fitted one: `β` is always a caller input to
[`SubchannelProblem::new`].

```rust
pub const TYPICAL_MIXING_COEFFICIENT_BETA: f64 = 0.02;
```

#### Constant `DEFAULT_MOMENTUM_MIXING_FACTOR`

Default turbulent-momentum factor `f_T`: 1, i.e. the eddy exchange that
carries enthalpy carries axial momentum at the same rate (Reynolds
analogy). COBRA-IIIC (Rowe 1973) exposes `f_T` as an input;
[`SubchannelProblem::with_momentum_mixing_factor`]`(0.0)` ablates the
momentum part alone.

```rust
pub const DEFAULT_MOMENTUM_MIXING_FACTOR: f64 = 1.0;
```

#### Constant `STANDARD_GRAVITY`

Standard gravity, the default axial gravity for vertical upward flow.

```rust
pub const STANDARD_GRAVITY: f64 = 9.80665;
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

