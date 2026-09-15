# Crate Documentation

**Version:** 0.0.2

**Format Version:** 61

# Module `nee_soon`

# NEE_SOON

**N**eutron **E**nergy-dependent **S**imulation using **O**pen-source
**O**bject-**O**riented **N**umerics.

NEE_SOON is the **coupling / integration layer** of the OUTRAM PARK suite.
It does not implement transport, nuclear-data processing, or kinetics
itself — those live in dedicated crates. Instead it composes them behind a
single, human-navigable object-oriented API so that a user can assemble the
simulation pieces they want without wiring the crates together by hand.

## What it composes

| Piece | Provided by | Role |
|---|---|---|
| Nuclear data / cross sections | [`njoy_outram_park_fork`] | energy-dependent σ(E), ν̄, χ, WMP |
| Monte Carlo transport | [`outram_mc_libs`] | CSG geometry, k-eigenvalue, Woodcock tracking |
| Point reactor kinetics | [`teh_o_prke`] | PRKE precursor/reactivity time response |
| Prompt excursion (Nordheim-Fuchs) | [`teh_o_prke::nordheim_fuchs`] | real-time-friendly closed-form prompt excursion + adiabatic fuel feedback, the "Prompt Excursion Layer" beneath full PRKE |
| GeN-Foam SP3 multiphysics | [`outram_foam_appbuilder_lib::genfoam`] | SP3 neutronics + porous-media TH + multi-region coupling (host for the Xin Wang workflow) |

## Worked coupling: the Xin Wang SP3 workflow

[`xin_wang_sp3_workflow`] is a **scaffold** of the four-stage
njoy → openmc → genfoam pipeline that reproduces Figure 4.29 (Mk1 PB-FHR
control-rod-removal transient) of Xin Wang's 2018 UC Berkeley PhD
dissertation. Each stage is a documented, beaded placeholder; the extracted
thesis methodology and case data live in the crate's `docs/xin-wang-thesis/`.

## Entry point

The whole crate is reached through **one struct**, [`NeeSoon`]. It is the
object-oriented facade: the user constructs a `NeeSoon`, then asks it to
create the relevant simulation pieces (a data provider, a transport model, a
kinetics model, a coupled run) rather than importing each underlying crate
directly. This keeps the mental context load low — one type to learn, with
`rust-analyzer` autocompletion revealing the available pieces.

## What belongs here / what does not

- **Belongs here:** orchestration, the object-oriented facade, cross-crate
  glue types, ergonomic constructors, coupling schedules, and any *new*
  user-facing functionality that only makes sense once the pieces are joined.
- **Does NOT belong here:** raw physics kernels. New cross-section code goes
  to `njoy-outram-park-fork`; new transport code to `outram-mc-libs`; new
  kinetics to `teh-o-prke`. NEE_SOON only *exposes and integrates* them.

## Status

**Mostly scaffold.** [`NeeSoon::new_prompt_excursion_model`] is real,
wired code -- it exposes `teh-o-prke`'s Nordheim-Fuchs exact
timestepper. The nuclear-data (`njoy-outram-park-fork`) and Monte Carlo
(`outram-mc-libs`) integration points are not wired yet; those crates
are declared as dependencies but the coupling logic for them is future
work, deliberately out of scope for this pass.

## Modules

## Module `xin_wang_sp3_workflow`

# Xin Wang SP3 multiphysics workflow (Figure-4.29 reproduction)

Scaffold of the four-stage reactor-multiphysics pipeline that reproduces
**Figure 4.29** — the maximum fuel temperature during a control-rod-removal
transient — of Xin Wang's 2018 UC Berkeley PhD dissertation *"Coupled
neutronics and thermal-hydraulics modeling for pebble-bed FHR"*
(<https://escholarship.org/uc/item/40q3985m>, open literature). The extracted
methodology and case data live in the crate's `docs/xin-wang-thesis/`.

Wang used **Serpent** (Monte Carlo) + **COMSOL** (SP3 via user-defined PDEs).
OUTRAM PARK re-implements that on **njoy → openmc → genfoam**. This module is
the coupling driver; it composes the public APIs of the data / transport /
neutronics crates — it does not re-implement any physics kernel.

## Module map

| Item | Role |
|---|---|
| [`case`] | typed Mk1 case data: 8-group structure, transient definition, digitised Fig. 4.29 curve |
| [`mgxs::MgxsGenerationStage`] | **Stage 1** — 8-group MGXS from ENDF via `njoy` (bead op-fr2.2.2) |
| [`mesh_mc::MeshMonteCarloStage`] | **Stage 2** — Mk1 mesh + Monte Carlo model + MGXS/power tallies via `outram-mc` (bead op-fr2.2.3) |
| [`sp3_multiphysics::Sp3MultiphysicsStage`] | **Stage 3** — GeN-Foam SP3 neutronics + porous-media TH transient (bead op-fr2.2.4) |
| [`validation::Fig429ValidationStage`] | **Stage 4** — compare vs the Fig. 4.29 reference (bead op-fr2.2.5) |
| [`XinWangSp3Workflow`] | the driver that owns all four stages in pipeline order |

## Status — scaffold only

Every stage's `run()` is a documented **placeholder** that returns
[`WorkflowError::NotYetImplemented`] naming its tracking bead. No MGXS is
generated, no MC model built, no SP3 transient run, and Fig. 4.29 is **not**
reproduced. The scaffold exists so the coupling surface compiles and each
stage is a navigable, beaded Rust type. The dependency ordering is
**MGXS → mesh → SP3 → validation**; several stages are blocked on capabilities
still being built in `njoy-outram-park-fork`, `outram-mc-libs`, and the
in-progress GeN-Foam SP3 port (see each stage's bead references).

```rust
pub mod xin_wang_sp3_workflow { /* ... */ }
```

### Modules

## Module `case`

Mk1 PB-FHR case data for the Xin Wang (2018) Figure-4.29 reproduction.

This module holds the *typed, machine-readable* form of the case data
extracted into `docs/xin-wang-thesis/04-transients-fig4-29.md`: the 8-group
energy structure (Table 3.4), the control-rod-removal transient definition
(§4.5.2), and the digitised Figure-4.29 reference curve (max fuel temperature
vs time). These are the inputs the workflow stages target and the reference
the validation stage compares against.

Source: Xin Wang, PhD dissertation, UC Berkeley, 2018,
<https://escholarship.org/uc/item/40q3985m> (open literature). All values are
AI-assisted extractions requiring human verification against the source PDF;
the Figure-4.29 curve in particular is **digitised by eye** and approximate.

All public quantities are [`uom`]-dimensioned (never bare `f64`).

```rust
pub mod case { /* ... */ }
```

### Types

#### Struct `Mk1DesignPoint`

Mk1 core design parameters relevant to the coupled transient (Table 4.1).

Only the parameters the workflow needs at its API boundary are typed here;
the full table is in `docs/xin-wang-thesis/04-transients-fig4-29.md`.

```rust
pub struct Mk1DesignPoint {
    pub thermal_power: uom::si::f64::Power,
    pub coolant_inlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub coolant_outlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub coolant_mass_flow: uom::si::f64::MassRate,
    pub fuel_enrichment: uom::si::f64::Ratio,
    pub pebble_packing_fraction: uom::si::f64::Ratio,
    pub triso_packing_fraction: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `thermal_power` | `uom::si::f64::Power` | Rated thermal power (236 MW). |
| `coolant_inlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | Coolant (flibe) inlet temperature (600 °C). |
| `coolant_outlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | Coolant bulk-average outlet temperature (700 °C). |
| `coolant_mass_flow` | `uom::si::f64::MassRate` | Total coolant mass flow (976 kg/s). |
| `fuel_enrichment` | `uom::si::f64::Ratio` | U-235 fuel enrichment as a fraction (0.199). |
| `pebble_packing_fraction` | `uom::si::f64::Ratio` | Pebble packing fraction (0.60). |
| `triso_packing_fraction` | `uom::si::f64::Ratio` | TRISO packing fraction inside the fuel annulus (0.40). |

##### Implementations

###### Methods

- ```rust
  pub fn nominal() -> Self { /* ... */ }
  ```
  The nominal Mk1 operating point from Table 4.1.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Mk1DesignPoint { /* ... */ }
    ```

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
#### Struct `ControlRodRemovalTransient`

Definition of the control-rod-removal transient (§4.5.2), the scenario that
produces Figure 4.29.

```rust
pub struct ControlRodRemovalTransient {
    pub total_control_rods: usize,
    pub rods_removed: usize,
    pub all_rods_out_excess_reactivity: uom::si::f64::Ratio,
    pub duration: uom::si::f64::Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total_control_rods` | `usize` | Total number of control rods in the core (8, Table 4.6). |
| `rods_removed` | `usize` | Number of rods removed to trigger the transient (3 of 8). |
| `all_rods_out_excess_reactivity` | `uom::si::f64::Ratio` | Excess reactivity available if *all* rods were removed from the initial<br>symmetric insertion (3941 pcm). |
| `duration` | `uom::si::f64::Time` | Simulated transient duration (100 s). |

##### Implementations

###### Methods

- ```rust
  pub fn fig_4_29() -> Self { /* ... */ }
  ```
  The Figure-4.29 transient as defined in §4.5.2: rods pre-inserted

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ControlRodRemovalTransient { /* ... */ }
    ```

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
#### Struct `Fig429Point`

A single digitised point on the Figure-4.29 reference curve.

```rust
pub struct Fig429Point {
    pub time: uom::si::f64::Time,
    pub max_fuel_temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time` | `uom::si::f64::Time` | Time since transient start. |
| `max_fuel_temperature` | `uom::si::f64::ThermodynamicTemperature` | Maximum fuel temperature (centre of the hottest fuel kernel). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Fig429Point { /* ... */ }
    ```

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
### Functions

#### Function `energy_group_lower_bounds`

Lower energy boundary of each of the 8 groups, in electron-volts
(Table 3.4). Index 0 is group 1 (fast). Group 8's lower bound is 0 (thermal
cutoff). The upper bound of group 1 is the ENDF maximum (~20 MeV).

Returns `uom` [`Energy`] values; construction is not `const` because
`Energy::new` is not a `const fn`.

```rust
pub fn energy_group_lower_bounds() -> [uom::si::f64::Energy; 8] { /* ... */ }
```

#### Function `fuel_safety_limit`

Fuel-failure safety limit for the graphite-based FHR fuel element
(dissertation §Abstract / §4.5.2): 1600 °C. The Fig. 4.29 result stays far
below this.

```rust
pub fn fuel_safety_limit() -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

#### Function `fig_4_29_reference_curve`

The digitised Figure-4.29 reference curve: maximum fuel temperature vs time
during the control-rod-removal transient.

**Approximate** — read by eye off the printed plot to ~5 °C resolution (the
thesis does not tabulate it). Shape: prompt jump to a ~988 °C peak near 8 s,
a shallow dip to ~975 °C near 28 s, then a slow climb to ~1006 °C at 100 s;
the maximum stays ~600 °C below the 1600 °C safety limit. A careful
re-digitisation should replace these before any quantitative pass/fail claim.

```rust
pub fn fig_4_29_reference_curve() -> Vec<Fig429Point> { /* ... */ }
```

### Constants and Statics

#### Constant `NUM_ENERGY_GROUPS`

Number of energy groups in the Mk1 multi-group model (Table 3.4).

```rust
pub const NUM_ENERGY_GROUPS: usize = 8;
```

#### Constant `NUM_DELAYED_GROUPS`

Number of delayed-neutron precursor groups (Eq. 2.20 / 2.22).

```rust
pub const NUM_DELAYED_GROUPS: usize = 6;
```

## Module `mesh_mc`

Stage 2 — mesh + Monte Carlo model (OpenMC / `outram-mc-libs`).

Builds the Mk1 PB-FHR Monte Carlo reference model — annular pebble-bed
geometry (center reflector, active fuel region, blanket-pebble ring, outer
reflector, core barrel/downcomer/vessel; Tables 4.1/4.4/4.8–4.9 + Appendix C),
an FCC pebble lattice (packing 60 %, 3 cm pebbles, 4730 TRISO/pebble) — and
sets up `RegularMesh` + energy/spatial tallies that produce the flux weighting
for the 8-group MGXS (Eq. 2.23) and the per-burnup power fraction (Fig. 4.7).

**Placeholder stage.** `outram-mc-libs` is data-free and pulls cross sections
from `njoy-outram-park-fork`. The enabling capabilities — multigroup mode +
`MGXSLibrary` (bead **op-6tz.15**) and `RegularMesh`/`MeshFilter` tallies
(bead **op-6tz.13**) — are still being built, so this stage composes their
public API only. Tracked by bead **op-fr2.2.3**.

```rust
pub mod mesh_mc { /* ... */ }
```

### Types

#### Struct `Mk1CoreGeometry`

Key radial dimensions of the Mk1 annular core (Table 4.1 / Appendix C),
used to lay out the CSG/mesh geometry. Radii are outer radii from the axis.

```rust
pub struct Mk1CoreGeometry {
    pub center_reflector_radius: uom::si::f64::Length,
    pub fuel_region_outer_radius: uom::si::f64::Length,
    pub blanket_outer_radius: uom::si::f64::Length,
    pub outer_reflector_radius: uom::si::f64::Length,
    pub vessel_outer_radius: uom::si::f64::Length,
    pub pebble_diameter: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `center_reflector_radius` | `uom::si::f64::Length` | Center (inner) graphite reflector outer radius (35 cm, Table 4.1). |
| `fuel_region_outer_radius` | `uom::si::f64::Length` | Active fuel-region outer radius (105 cm, Appendix C Table C.2). |
| `blanket_outer_radius` | `uom::si::f64::Length` | Blanket-pebble ring outer radius (125 cm, Appendix C Table C.3). |
| `outer_reflector_radius` | `uom::si::f64::Length` | Outer graphite reflector outer radius (165 cm, Appendix C Table C.4). |
| `vessel_outer_radius` | `uom::si::f64::Length` | Reactor vessel outer radius (175 cm, Table 4.9). |
| `pebble_diameter` | `uom::si::f64::Length` | Fuel-pebble outer diameter (3 cm, Table 4.3). |

##### Implementations

###### Methods

- ```rust
  pub fn reference() -> Self { /* ... */ }
  ```
  The reference Mk1 geometry (no outer shield; Table 4.9 / Appendix C).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Mk1CoreGeometry { /* ... */ }
    ```

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
#### Struct `MeshMonteCarloStage`

Stage-2 driver: builds the Mk1 Monte Carlo model + MGXS/power tallies.

```rust
pub struct MeshMonteCarloStage {
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
  Creates the stage with the reference Mk1 geometry and packing.

- ```rust
  pub fn geometry(self: &Self) -> Mk1CoreGeometry { /* ... */ }
  ```
  The Mk1 core geometry this stage meshes.

- ```rust
  pub fn pebble_packing_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  FCC pebble packing fraction (0.60).

- ```rust
  pub fn triso_per_pebble(self: &Self) -> usize { /* ... */ }
  ```
  TRISO particles per fuel pebble (4730).

- ```rust
  pub fn run(self: &Self) -> Result<(), WorkflowError> { /* ... */ }
  ```
  Build the Monte Carlo model, run k-eigenvalue, and tally the 8-group MGXS

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MeshMonteCarloStage { /* ... */ }
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
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
### Constants and Statics

#### Constant `STAGE_BEAD`

Bead tracking the mesh + Monte Carlo stage.

```rust
pub const STAGE_BEAD: &str = "op-fr2.2.3";
```

## Module `mgxs`

Stage 1 — multigroup cross-section (MGXS) generation via `njoy`.

Produces the cell-homogenised 8-group macroscopic cross sections the
deterministic SP3 model consumes, from ENDF/B-VII.0, following Wang Eq. 2.23
(flux-weighted tally) and the feedback parametrisation Eqs. 2.24–2.26
(linear-in-density for flibe, linear-in-log-T for fuel Doppler).

**Placeholder stage.** `njoy-outram-park-fork` owns all nuclear-data code and
does not yet expose a public MGXS export entry point for deterministic
consumers — that API is requested in bead **op-cjw.24**. This stage scaffolds
the *call* into that future API; it does not (and must not) implement MGXS
inside `njoy` from here. Tracked by bead **op-fr2.2.2**.

```rust
pub mod mgxs { /* ... */ }
```

### Types

#### Enum `Mk1MaterialRegion`

The set of homogenised material regions the Mk1 SP3 model needs cross
sections for (Table 4.4). One MGXS set is generated per region, each
parametrised for feedback.

```rust
pub enum Mk1MaterialRegion {
    FuelPebble,
    BlanketPebble,
    CenterReflector,
    OuterReflector,
    ControlRod,
    StainlessSteel,
    Flibe,
}
```

##### Variants

###### `FuelPebble`

Fuel-pebble region (graphite shell + fuel annulus + graphite core + flibe).

###### `BlanketPebble`

Graphite blanket-pebble region + flibe.

###### `CenterReflector`

Center graphite reflector.

###### `OuterReflector`

Outer graphite (borated) reflector.

###### `ControlRod`

Boron-carbide control rod.

###### `StainlessSteel`

Structural stainless steel (SS316 core barrel / vessel).

###### `Flibe`

Flibe coolant channels.

##### Implementations

###### Methods

- ```rust
  pub fn all() -> [Mk1MaterialRegion; 7] { /* ... */ }
  ```
  All regions that require an MGXS set for the Mk1 core model.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Mk1MaterialRegion { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Mk1MaterialRegion) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `MgxsGenerationStage`

Stage-1 driver: generates 8-group MGXS for every [`Mk1MaterialRegion`].

```rust
pub struct MgxsGenerationStage {
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
  Creates the stage with the Mk1 8-group structure (Table 3.4).

- ```rust
  pub fn group_lower_bounds(self: &Self) -> [Energy; 8] { /* ... */ }
  ```
  The 8-group lower energy boundaries this stage generates constants on.

- ```rust
  pub fn run(self: &Self) -> Result<(), WorkflowError> { /* ... */ }
  ```
  Generate the 8-group MGXS for all Mk1 material regions.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MgxsGenerationStage { /* ... */ }
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
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
### Constants and Statics

#### Constant `STAGE_BEAD`

Bead tracking the MGXS-generation stage.

```rust
pub const STAGE_BEAD: &str = "op-fr2.2.2";
```

#### Constant `NJOY_API_BEAD`

Bead requesting the required MGXS-export API from `njoy-outram-park-fork`.

```rust
pub const NJOY_API_BEAD: &str = "op-cjw.24";
```

## Module `sp3_multiphysics`

Stage 3 — SP3 multiphysics (GeN-Foam, `outram-foam-appbuilder-lib`).

Drives the GeN-Foam SP3 neutronics coupled to the porous-media TH model + the
multi-scale fuel-pebble/TRISO conduction feedback over the control-rod-removal
transient. The SP3 system is Wang Eq. 2.22 / D.12 (two moment fields
`Phi0 = phi0 + 2*phi2` and `phi2`; six delayed groups), fed with the Stage-1/2
MGXS, coupled through the `multi_region` outer loop with Ergun/Wakao TH
closures ($E_1=150$, $E_2=1.75$, $c_F=0.52$).

**Placeholder stage, blocked on the in-progress GeN-Foam SP3 port.** It codes
against the intended interface in
[`outram_foam_appbuilder_lib::genfoam::neutronics`]:
`Sp3Neutronics::with_cross_sections(..)` → `solve_eigenvalue()` → `step(dt)`,
or wrapped as `NeutronicsModel::Sp3(..)` for the shared `power()`/`k_eff()`
surface. Two blockers remain:

1. the SP3 solver port itself (`Sp3Neutronics` eigenvalue/transient solvers +
   boundary handling / benchmark) — bead **op-p6p.15**; today
   `Sp3Neutronics::new` is a state-only scaffold whose solvers return
   `ModelNotImplemented(Sp3)`;
2. mesh-based neutronics is **not yet a `RegionModel` variant** in
   `multi_region::outer_iteration`, so SP3 cannot be driven through
   `MultiPhysicsSolver` yet ("wired-in-waiting") — bead **op-p6p.8.4**.

Tracked by bead **op-fr2.2.4**.

```rust
pub mod sp3_multiphysics { /* ... */ }
```

### Types

#### Struct `PorousMediaClosures`

Ergun / Wakao porous-media closure values for the Mk1 pebble bed (Table 4.10).

```rust
pub struct PorousMediaClosures {
    pub ergun_e1: f64,
    pub ergun_e2: f64,
    pub forchheimer_cf: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ergun_e1` | `f64` | Ergun viscous coefficient `E_1` (150). |
| `ergun_e2` | `f64` | Ergun inertial coefficient `E_2` (1.75). |
| `forchheimer_cf` | `f64` | Non-dimensional Forchheimer drag coefficient `c_F` (0.52). |

##### Implementations

###### Methods

- ```rust
  pub fn mk1() -> Self { /* ... */ }
  ```
  The Mk1 values from Table 4.10.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PorousMediaClosures { /* ... */ }
    ```

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
#### Struct `Sp3MultiphysicsStage`

Stage-3 driver: SP3 neutronics coupled to porous-media TH for the transient.

```rust
pub struct Sp3MultiphysicsStage {
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
  Creates the stage for the 8-group / 6-delayed-group Mk1 model and the

- ```rust
  pub fn energy_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups the SP3 model runs (8).

- ```rust
  pub fn delayed_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of delayed-neutron precursor groups (6).

- ```rust
  pub fn porous_media_closures(self: &Self) -> PorousMediaClosures { /* ... */ }
  ```
  The porous-media TH closures used for the coupled run.

- ```rust
  pub fn transient(self: &Self) -> ControlRodRemovalTransient { /* ... */ }
  ```
  The transient this stage drives.

- ```rust
  pub fn target_neutronics_kind(self: &Self) -> NeutronicsModelKind { /* ... */ }
  ```
  The GeN-Foam neutronics model kind this stage targets — SP3.

- ```rust
  pub fn run(self: &Self) -> Result<(), WorkflowError> { /* ... */ }
  ```
  Solve the SP3 eigenvalue steady state, then step the coupled SP3 + TH

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Sp3MultiphysicsStage { /* ... */ }
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
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
### Constants and Statics

#### Constant `STAGE_BEAD`

Bead tracking the SP3-multiphysics stage.

```rust
pub const STAGE_BEAD: &str = "op-fr2.2.4";
```

#### Constant `GENFOAM_SP3_PORT_BEAD`

Bead for the in-progress GeN-Foam SP3 solver port (blocker).

```rust
pub const GENFOAM_SP3_PORT_BEAD: &str = "op-p6p.15";
```

#### Constant `GENFOAM_COUPLING_BEAD`

Bead for the missing mesh-neutronics `RegionModel` coupling variant (blocker).

```rust
pub const GENFOAM_COUPLING_BEAD: &str = "op-p6p.8.4";
```

## Module `validation`

Stage 4 — Figure-4.29 validation (the reproduction target).

Compares the Stage-3 coupled SP3 transient against the digitised Figure-4.29
reference curve (maximum fuel temperature vs time during the control-rod-
removal transient; [`super::case::fig_4_29_reference_curve`]). Also cross-
checks Fig. 4.27 (full-core power: ~236 MW → ~+30 % peak → ~+30 % settle).

**Nature of the check.** Wang's own reference is a code-to-code result
(Serpent + COMSOL), and PB-FHR has no experimental data — so this is
code-to-code **verification**, not experimental validation. The V&V write-up
must state methodology *and* measured numbers with uncertainty (workspace V&V
rule).

**Placeholder stage.** Depends on Stages 1–3 (none of which run yet). Tracked
by bead **op-fr2.2.5**.

```rust
pub mod validation { /* ... */ }
```

### Types

#### Struct `Fig429Target`

The expected qualitative features of Figure 4.29 the reproduction must match.

```rust
pub struct Fig429Target {
    pub peak_max_fuel_temperature_celsius: f64,
    pub end_max_fuel_temperature_celsius: f64,
    pub peak_power_fraction_above_initial: f64,
    pub initial_power: uom::si::f64::Power,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `peak_max_fuel_temperature_celsius` | `f64` | Approximate peak maximum-fuel-temperature (~988 °C near 8 s), °C. |
| `end_max_fuel_temperature_celsius` | `f64` | Approximate end-of-transient value (~1006 °C at 100 s), °C. |
| `peak_power_fraction_above_initial` | `f64` | Fig. 4.27 peak power as a fraction above initial (~+30 %). |
| `initial_power` | `uom::si::f64::Power` | Full-core initial power (236 MW). |

##### Implementations

###### Methods

- ```rust
  pub fn digitised() -> Self { /* ... */ }
  ```
  The Figure-4.29 / 4.27 acceptance features (digitised, approximate).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Fig429Target { /* ... */ }
    ```

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
#### Struct `Fig429ValidationStage`

Stage-4 driver: loads the reference curve and defines the comparison.

```rust
pub struct Fig429ValidationStage {
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
  Creates the stage with the digitised Figure-4.29 reference curve loaded.

- ```rust
  pub fn reference_curve(self: &Self) -> &[Fig429Point] { /* ... */ }
  ```
  The digitised Figure-4.29 reference curve (max fuel temperature vs time).

- ```rust
  pub fn target(self: &Self) -> Fig429Target { /* ... */ }
  ```
  The qualitative acceptance features to match.

- ```rust
  pub fn run(self: &Self) -> Result<(), WorkflowError> { /* ... */ }
  ```
  Run the Stage-3 transient and compare its max-fuel-temperature history

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Fig429ValidationStage { /* ... */ }
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
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
### Constants and Statics

#### Constant `STAGE_BEAD`

Bead tracking the Figure-4.29 validation stage.

```rust
pub const STAGE_BEAD: &str = "op-fr2.2.5";
```

### Types

#### Enum `WorkflowStage`

Which stage of the workflow an error or status refers to. Pipeline order is
the declaration order below (MGXS → mesh → SP3 → validation).

```rust
pub enum WorkflowStage {
    MgxsGeneration,
    MeshAndMonteCarlo,
    Sp3Multiphysics,
    Fig429Validation,
}
```

##### Variants

###### `MgxsGeneration`

Stage 1 — multigroup cross-section generation (njoy).

###### `MeshAndMonteCarlo`

Stage 2 — mesh + Monte Carlo model (openmc / outram-mc).

###### `Sp3Multiphysics`

Stage 3 — SP3 multiphysics transient (genfoam).

###### `Fig429Validation`

Stage 4 — Figure-4.29 comparison (validation target).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> WorkflowStage { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &WorkflowStage) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `WorkflowError`

Error type for the Xin Wang SP3 workflow scaffold.

```rust
pub enum WorkflowError {
    NotYetImplemented {
        stage: WorkflowStage,
        bead: &'static str,
    },
}
```

##### Variants

###### `NotYetImplemented`

A workflow stage is scaffolded but not yet implemented. Carries the
tracking bead so the caller knows where the work is queued.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `stage` | `WorkflowStage` | The stage that is not yet implemented. |
| `bead` | `&'static str` | The beads issue id tracking that stage's implementation. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
#### Struct `XinWangSp3Workflow`

The four-stage Xin Wang SP3 multiphysics workflow driver.

Owns one instance of each stage in pipeline order. Constructing it wires the
Mk1 case data into every stage; running the pipeline is future work (each
stage's `run()` is a placeholder — see the module-level status note).

# Example

```
use nee_soon::xin_wang_sp3_workflow::{XinWangSp3Workflow, WorkflowStage};

let workflow = XinWangSp3Workflow::new();

// The case data is real and available now:
assert_eq!(workflow.mgxs().group_lower_bounds().len(), 8);

// Running any stage is a documented placeholder for now:
let err = workflow.mgxs().run().unwrap_err();
assert!(matches!(
    err,
    nee_soon::xin_wang_sp3_workflow::WorkflowError::NotYetImplemented {
        stage: WorkflowStage::MgxsGeneration,
        ..
    }
));
```

```rust
pub struct XinWangSp3Workflow {
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
  Creates the workflow with all four stages wired to the Mk1 case data.

- ```rust
  pub fn mgxs(self: &Self) -> &MgxsGenerationStage { /* ... */ }
  ```
  Stage 1 — MGXS generation (njoy).

- ```rust
  pub fn mesh_mc(self: &Self) -> &MeshMonteCarloStage { /* ... */ }
  ```
  Stage 2 — mesh + Monte Carlo (openmc / outram-mc).

- ```rust
  pub fn sp3_multiphysics(self: &Self) -> &Sp3MultiphysicsStage { /* ... */ }
  ```
  Stage 3 — SP3 multiphysics (genfoam).

- ```rust
  pub fn validation(self: &Self) -> &Fig429ValidationStage { /* ... */ }
  ```
  Stage 4 — Figure-4.29 validation.

- ```rust
  pub fn run(self: &Self) -> Result<(), WorkflowError> { /* ... */ }
  ```
  Run the full pipeline in order (MGXS → mesh → SP3 → validation).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> XinWangSp3Workflow { /* ... */ }
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
    fn default() -> XinWangSp3Workflow { /* ... */ }
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
## Types

### Struct `NeeSoon`

**Attributes:**

- `NonExhaustive`

Object-oriented facade for the OUTRAM PARK neutronics + kinetics suite.

`NeeSoon` is the single entry point of the crate (the "one big struct"): a
user constructs one of these and then creates the relevant simulation pieces
through it — a nuclear-data provider ([`njoy_outram_park_fork`]), a Monte
Carlo transport model ([`outram_mc_libs`]), a point-kinetics model
([`teh_o_prke`]), and, ultimately, coupled runs that thread data between
them.

# Physical scope

This type owns no physics of its own; it is a builder/orchestrator over the
composed crates. Physical quantities exchanged across its API are dimensioned
via [`uom`] (never bare `f64`).

# Status

[`Self::new_prompt_excursion_model`] is wired to `teh-o-prke`'s
[`NordheimFuchsExactTimestepper`]. Nuclear-data-provider and Monte
Carlo transport / coupled-run construction are not implemented yet.
The planned shape is a builder that holds:
- a nuclear-data provider handle (cross-section source),
- an optional transport model,
- an optional kinetics model,
- coupling / orchestration configuration.

```rust
pub struct NeeSoon {
}
```

#### Fields

| Name | Type | Documentation |
|------|------|---------------|

#### Implementations

##### Methods

- ```rust
  pub fn new_prompt_excursion_model(self: &Self, prompt_neutron_generation_time: Time, delayed_neutron_fraction: Ratio, fuel_heat_capacity: HeatCapacity, fuel_feedback_coefficient: TemperatureCoefficient, fuel_reference_temperature: ThermodynamicTemperature, initial_fuel_temperature: ThermodynamicTemperature, initial_power: Power) -> Result<NordheimFuchsExactTimestepper, TehOPrkeError> { /* ... */ }
  ```
  Creates a Nordheim-Fuchs exact-timestepper prompt-excursion model

##### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NeeSoon { /* ... */ }
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
    fn default() -> NeeSoon { /* ... */ }
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
## Re-exports

### Re-export `NordheimFuchsExactTimestepper`

```rust
pub use teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper;
```

### Re-export `TehOPrkeError`

```rust
pub use teh_o_prke::teh_o_prke_error::TehOPrkeError;
```

