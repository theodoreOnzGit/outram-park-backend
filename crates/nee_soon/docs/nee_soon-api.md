# Crate Documentation

**Version:** 0.0.2

**Format Version:** 60

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

~~**Mostly scaffold.**~~ **CORRECTED 2026-09-19.** The claim that "the
nuclear-data and Monte Carlo integration points are not wired yet" is no
longer true, and is struck rather than deleted because it shaped how this
crate was described for months.

What is real and tested today:

| Module | What it does |
|---|---|
| [`htr10_rmc`] | one shared HTR-10 geometry and composition, so the stochastic and deterministic ends cannot drift apart |
| [`mgxs`] | condenses a Monte Carlo run into multigroup constants: flux-weighted reaction rates, a nu-scatter matrix, and a **measured** fission spectrum |
| [`genfoam_xs`] | hands those constants to GeN-Foam through its own `nuclearData` input path |
| [`coupling`] | [`coupling::McToGenFoam`], the facade: construct, `generate_mgxs()`, `solve_infinite_medium()` |
| [`direct_coupling`] | [`direct_coupling::McGenFoamDirect`], Monte Carlo iterated directly against GeN-Foam's lumped thermal region |

[`NeeSoon::new_prompt_excursion_model`] remains real, wired code exposing
`teh-o-prke`'s Nordheim-Fuchs exact timestepper.

Measured 2026-09-19: on a leakage-free medium GeN-Foam reproduces the Monte
Carlo eigenvalue to **+328 pcm, 1.7 sigma**; the HTR-10 core condenses to
four zones GeN-Foam accepts and solves; the direct loop converges in 8 outer
iterations over a +248 K temperature swing.

**What is still NOT here:** delayed-neutron data, so everything above is
**steady state only** and a transient would run without delayed neutrons;
`P0` scattering only, so no SP3 `P1` and an untransport-corrected diffusion
coefficient; one state point per run, so no feedback parametrisation; and
**no validation of any kind** -- no experiment and no published benchmark.
The [`xin_wang_sp3_workflow`] scaffold is still a scaffold.

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
## Module `htr10_rmc`

# HTR-10 code-to-code verification against the RMC paper

**Reference.** Li Wanlin, Yu Ganglin & Wei Chunlin, *"Research on Benchmark
Calculation and Analysis of HTR-10 with RMC Code"*, 7th International Topical
Meeting on High Temperature Reactor Technology (HTR 2014), Weihai, China,
27-31 October 2014.

This is the coupling layer's job: the paper specifies one reactor, and both
the Monte Carlo and the deterministic ends of this suite should reproduce it
from **the same** geometry and composition. That shared model lives here so
the two ends cannot drift apart and quietly turn a composition difference
into an apparent transport difference.

# Verification status: TENTATIVE, and what is still open (2026-09-25)

The twelve-height `k_eff` curve IS now computed against RMC (the "NOT
verifiable now" section below predates the TECDOC reflector model). At
`0454c1ad1b`, 10 000 x [5 + 135], one seed per height, the residual is
`-896 +/- 30` pcm on ENDF/B-VIII.0 and `+288 +/- 31` pcm on ENDF/B-VII.0
(the reference's library), and **drifts `+7` pcm/cm with loading height in
every arm** (gh:#218, results posted there). Treat those numbers as tentative
until the items below are priced or fixed. Each is an issue; none has been
measured unless it says so.

**Model defects, production path (`assemble_explicit_triso`):**
- gh:#309 — one ball per hex tile clips the pebble shell: 4.76 % of all core
  carbon is missing while the heavy metal is exact (C/U low). Sign on `k`
  not predicted.
- gh:#310 — the lattice drops the A-B layer offset, so axially adjacent
  pebbles touch and their fuel zones meet; pebble-scale self-shielding and
  Dancoff factors are those of welded columns, not a packing. The fix for
  both is the two-ball sub-universe cell `bed.rs` already reconstructs.
- ~~gh:#311 — only B-10 is placed~~ **FIXED 2026-09-25**: B-11 now goes in
  beside B-10 in every material, from the selected library, pinned by
  `every_boron_bearing_material_carries_natural_b11`. Its worth is priced
  on #311. Every k in this section predates it.
- ~~gh:#316 — the built core carried ~1.2 % less heavy metal~~ **FIXED
  2026-09-25**: the TRISO count was taken on one grid offset and the
  lattice built on another (8340 counted, 8240 built). Both now use one
  offset; built == counted is asserted. Resampled: 0.9971 +/- 0.0014 of the
  paper-implied kernel fraction (was 0.9875). Worth **+353 +/- 111 pcm**
  at 122.47 cm (three paired seeds). Every k in this section predates it.
- gh:#218 — the `+7` pcm/cm drift itself. **Cause now evidenced
  (2026-09-25):** a shrunk-pebble ablation with no #309 clip and no #310
  axial contact (all volume fractions the paper's) changes k by
  `-6.88 +/- 1.48` pcm/cm across 98-201 cm, equal and opposite to the
  drift. Its absolute offset mixes in an 18 % smaller pebble, so the fix is
  #309's two-ball cell with the real 6 cm pebble, re-measured across the
  range. Already ruled out: data library, source convergence, cavity,
  bottom-reflector mirroring, UO2 law source, B-11, TRISO count.

**Documented simplifications (not defects, each pushes `k` one way):**
- every reflector region is TECDOC zone 22, the densest graphite in
  Table 4-3, and the boronated zones are not placed — raises `k`;
- the control-rod boring band is solid zone-22 graphite
  (`OUTRAM_HTR10_BORINGS` is off: its core-height composition is unrecorded);
- the core-height reflector zone map is not placed;
- rods fully withdrawn; one temperature (300.15 K) everywhere.

**Other paths and plumbing:**
- gh:#308 — `assemble` (homogenised fuel) lacks the cavity, conus, bricks and
  annulus of the production path; do not use it for a `k` comparison. It
  feeds `htr10_mgxs_genfoam`.
- gh:#313 — `Cell::temperature` is never read by transport and every cell
  hardcodes 293.6 K; the material temperature (300.15 K) is what is used.
- gh:#312 — the control-rod smeared composition drops the steel sections
  (not exercised by the rods-out benchmark).

# What this module can verify today, and what it cannot

Read this before quoting anything from here. The honest scope is narrower
than "reproduce the paper", and the reason is in the paper itself.

## Verifiable now — the model's construction

Tables 1 and 2 are **over-determined**: they state quantities that are also
derivable from other quantities they state. Every such closure is a genuine
code-to-code check that our reconstruction matches theirs, and none of them
needs a transport solve. [`GeometryClosure`] carries them.

## NOT verifiable now — the k-eff curve

The paper's Tables 3 and 4 give `k_eff` against fuel-loading height, which is
the headline result. **We cannot reproduce it yet, and the blocker is the
paper's own**:

> *"Modeling details of reflector and structural material are referred to
> paper released by IAEA which is listed in reference."*

That reference is IAEA-TECDOC-1382. The HTR-10 core is 180 cm across inside
roughly a metre of graphite reflector which *"house\[s\] control rods, small
absorber balls, helium flow channels, and irradiation channels"* — and that
reflector is most of the reason so small a fissile inventory reaches
criticality. Without it the eigenvalue is not close, and pretending otherwise
would be reporting a number that looks like a comparison and is not one.

## A defect in the reference, found while reading it

Tables 3 and 4 are **both captioned "(vacuum)"**, while the text says
*"Calculations are performed for vacuum and helium."* Checking them row by
row:

- the **RMC** column is byte-identical in **11 of 11** shared rows;
- the **MCNP** column differs in **0 of 11** — that is, in every row.

So the paper reports **one** RMC dataset against **two** MCNP results, and
one of the two captions is wrong. Consequence for anyone verifying against
it: there is a single RMC curve, not a vacuum/helium pair, and an attempt to
reproduce two would be chasing an artefact. [`RMC_KEFF_VS_HEIGHT`] carries
that single curve.

## How close is close, for this reference

The paper's own RMC-vs-MCNP relative differences reach ~0.9 %, and it states
plainly that *"model used in this calculation is constructed relatively
independently"*. These are indicative code-to-code numbers, **not** a
benchmark reference. Agreement to ~500 pcm would be a real success here;
agreement to 50 pcm would be suspicious and should prompt a search for a
coincidence rather than a celebration.

```rust
pub mod htr10_rmc { /* ... */ }
```

### Modules

## Module `bed`

The HTR-10 pebble bed as a hexagonal lattice, reconstructed from the paper.

# Why this had to be reconstructed rather than read off

The paper describes the core as hexagonal prism unit cells but **never states
the pitch**. Its prose —

> *"There are seven balls at these faces; one at the center of the basal
> plane and six surrounding spheres ... The intermediate section of each
> hexagonal prism contains three full balls as well as partial contributions
> from the neighboring hexagonal prism cells from all six sides."*

— does not determine a cell: it describes sharing between neighbours without
saying how much, and the figures carry no extractable dimensions. So the cell
is derived from the invariants the paper **does** state, and then checked
against one it states that was *not* used in the derivation.

# The decode

The paper gives a layer height of 9.798 cm. For 6 cm spheres the close-packed
layer spacing is `d*sqrt(2/3)` = 4.8990 cm, and `9.798 / 4.8990 = 2.00001`.
The "layer" is exactly **two close-packed sphere layers**. That fixes the
axial structure and is not plausibly a coincidence.

The lattice is then **diluted** laterally until the filling matches the
paper's 61 %: ordered close packing would be 74.05 %, so the balls do not
touch. Expanding the pitch from the touching value `d` by
`sqrt(0.7405/0.61)` gives 6.6106 cm.

# The check that makes it evidence

The cell above was fitted to **two** stated quantities — layer height and
filling fraction. Tiling it through the stated core (180 cm diameter,
197 cm high) predicts **~27 038 balls** against the paper's stated **27 000**,
a 0.14 % difference. Nothing was tuned to hit that, which is what makes the
reconstruction evidence rather than a fit.

# What this is not

It reproduces the paper's stated *invariants*. It does **not** claim to be
their exact unit-cell tiling, which their text does not determine. Any
write-up must say so.

# Where it is built (2026-09-25)

[`TwoBallBed`] builds this cell as the transported bed of
`core_model::assemble_explicit_triso`: the hex tile IS the cell (two whole
balls per tile, A-B stacked), with fuel/dummy assigned per ball (gh:#309
step 2, gh:#310). Until then the lattice held one ball per half-height tile,
which dropped the A-B offset and made the pebbles interpenetrate.
[`bed_tile_levels`] (one identity per TILE) remains only for the
homogenised `core_model::assemble`.

```rust
pub mod bed { /* ... */ }
```

### Types

#### Struct `HexBedCell`

One hexagonal-prism unit cell of the HTR-10 bed.

Flat-to-flat `pitch` is the centre-to-centre distance between neighbouring
cells, so the hexagon's area is `(sqrt(3)/2) * pitch^2`.

```rust
pub struct HexBedCell {
    pub pitch: f64,
    pub height: f64,
    pub balls: f64,
    pub ball_diameter: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pitch` | `f64` | Flat-to-flat pitch \[cm\]. |
| `height` | `f64` | Cell height \[cm\] — one "layer" in the paper's sense. |
| `balls` | `f64` | Balls per cell (one per close-packed layer). |
| `ball_diameter` | `f64` | Ball diameter \[cm\]. |

##### Implementations

###### Methods

- ```rust
  pub fn from_paper() -> Self { /* ... */ }
  ```
  The cell reconstructed from the paper — see the module docs.

- ```rust
  pub fn volume(self: &Self) -> f64 { /* ... */ }
  ```
  Cell volume \[cm^3\].

- ```rust
  pub fn packing_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Ball volume fraction in the cell \[-\].

- ```rust
  pub fn in_plane_spacing(self: &Self) -> f64 { /* ... */ }
  ```
  Nearest-neighbour centre distance **within** a layer \[cm\] — simply the

- ```rust
  pub fn interlayer_spacing(self: &Self) -> f64 { /* ... */ }
  ```
  Nearest-neighbour centre distance **between** layers \[cm\], a ball to

- ```rust
  pub fn is_non_overlapping(self: &Self) -> bool { /* ... */ }
  ```
  Whether any two balls overlap. A lattice that overlaps is not a packing,

- ```rust
  pub fn balls_in_core(self: &Self, core_diameter_cm: f64, core_height_cm: f64) -> f64 { /* ... */ }
  ```
  Balls in a cylindrical core of the given diameter and height \[cm\].

- ```rust
  pub fn fuel_and_moderator_balls(self: &Self, core_diameter_cm: f64, core_height_cm: f64) -> (f64, f64) { /* ... */ }
  ```
  Fuel and moderator balls in the core at the paper's 0.57/0.43 ratio.

- ```rust
  pub fn vertex_radius(self: &Self) -> f64 { /* ... */ }
  ```
  Tile centre to vertex distance \[cm\], `pitch/sqrt(3)` — the lateral

- ```rust
  pub fn site_centre(self: &Self, site: BallSite) -> [f64; 3] { /* ... */ }
  ```
  Tile-local centre \[cm\] of `site` in a `HexOrientation::Y` lattice of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HexBedCell { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &HexBedCell) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `BedTile`

Which universe each tile of the bed lattice is filled with.

The RMC paper selects fuel and moderator balls **per layer** to give the
57:43 ratio, so the assignment is a property of the whole stack rather than
of any one tile.

```rust
pub enum BedTile {
    Fuel,
    Moderator,
}
```

##### Variants

###### `Fuel`

A fuel pebble (TRISO in graphite).

###### `Moderator`

A moderator / dummy pebble (graphite only).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BedTile { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &BedTile) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `BallSite`

One of the five ball sites a **two-ball** hex tile holds a piece of.

The tile is the paper's prism ([`HexBedCell::from_paper`]): flat-to-flat
`pitch`, height `height` = one A-B layer pair. In tile-local coordinates
(tile centre at the origin, a `HexOrientation::Y` lattice, whose vertices
sit at polar angles 0, 60, ..., 300 degrees and distance `pitch/sqrt(3)`):

| site | centre | shared by |
|---|---|---|
| `ABottom` | `(0, 0, -height/2)` | this tile and the one below (half each) |
| `ATop` | `(0, 0, +height/2)` | this tile and the one above (half each) |
| `BEast` | `(pitch/sqrt(3), 0, 0)` — the 0 degree vertex | the three tiles meeting there (a third each) |
| `BNorthWest` | the 120 degree vertex | three tiles |
| `BSouthWest` | the 240 degree vertex | three tiles |

`2 x 1/2 + 3 x 1/3 = 2` balls per tile. The B layer uses **alternate**
vertices only; the other three (60, 180, 300 degrees) are empty, which is
what makes the stacking A-B rather than a column.

```rust
pub enum BallSite {
    ABottom,
    ATop,
    BEast,
    BNorthWest,
    BSouthWest,
}
```

##### Variants

###### `ABottom`

A-layer ball on the tile axis at the bottom face.

###### `ATop`

A-layer ball on the tile axis at the top face.

###### `BEast`

B-layer ball at the 0 degree vertex, mid-height.

###### `BNorthWest`

B-layer ball at the 120 degree vertex, mid-height.

###### `BSouthWest`

B-layer ball at the 240 degree vertex, mid-height.

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
    fn clone(self: &Self) -> BallSite { /* ... */ }
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
    fn cmp(self: &Self, other: &BallSite) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BallSite) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &BallSite) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `BallId`

A ball of the global bed, named by the tile that **owns** it.

`(a, b)` are the skewed axial hex coordinates of a tile, with the central
tile at `(0, 0)` (`a = ix - (n_rings-1)`, `b = iy - (n_rings-1)` in
`HexLattice`'s index triplet). An A ball belongs to a column and a face; a
B ball to the tile whose `BEast` vertex it sits on. Every piece of one ball,
in every tile that holds a piece of it, resolves to the SAME `BallId` — that
is what makes the fuel/dummy identity per ball rather than per tile.

```rust
pub enum BallId {
    A {
        a: i32,
        b: i32,
        face: i32,
    },
    B {
        a: i32,
        b: i32,
        level: i32,
    },
}
```

##### Variants

###### `A`

A-layer ball on the axis of column `(a, b)`, on the bottom face of
lattice level `face` (so the top face of level `face - 1`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `i32` | Skewed hex coordinate. |
| `b` | `i32` | Skewed hex coordinate. |
| `face` | `i32` | Face index, 0 = bottom face of lattice level 0. |

###### `B`

B-layer ball at the `BEast` vertex of tile `(a, b)`, mid-height of
lattice level `level`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `i32` | Skewed hex coordinate of the owning tile. |
| `b` | `i32` | Skewed hex coordinate of the owning tile. |
| `level` | `i32` | Lattice level. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BallId { /* ... */ }
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
    fn cmp(self: &Self, other: &BallId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BallId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &BallId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `FuelAssignment`

How fuel and dummy identities are handed out over the balls.

```rust
pub enum FuelAssignment {
    Paper,
    FuelledConus,
    AllFuel,
}
```

##### Variants

###### `Paper`

The paper and Terry (2005): 57:43 over every ball with volume inside
the bed cylinder, the conus (every ball centred below the bed floor)
all dummy.

###### `FuelledConus`

ABLATION: the conus takes the 57:43 split too (`OUTRAM_HTR10_FUEL_CONUS`).

###### `AllFuel`

ABLATION: every ball is fuelled (`OUTRAM_HTR10_ALLFUEL`).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelAssignment { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &FuelAssignment) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `TwoBallBed`

**The HTR-10 bed as the paper's two-ball prism cell**, with a fuel/dummy
identity for every BALL (gh:#309 step 2, gh:#310).

NEW WORK, not a port.

# Axial layout

Ball layers sit every `height/2` = 4.899 cm, alternating A (on the tile
axis, at the tile faces) and B (at alternate vertices, at mid-height). The
bed of `n_axial` half-layers spans `[-n_axial*height/4, +n_axial*height/4]`
and its ball layers are centred at `bed_bottom + (j + 1/2) * height/2`,
`j = 0 .. n_axial-1`, so each layer is centred in its own 4.899 cm slab and
the laterally-averaged filling fraction of the bed slab is exactly the
cell's 0.61 (the lateral average is periodic with period `height/2`, and
the bed is a whole number of periods).

**The stacking phase is anchored at the bed FLOOR**: layer `j = 0` is
always an A layer. The bed floor is fixed hardware (the top of the conus),
so a taller loading only adds layers on top and everything below is
identical at every loading, exactly as in the reactor's loading sequence.
An odd `n_axial` therefore ends on an A layer and an even one on a B layer;
nothing else distinguishes them.

The lattice runs from below the conus floor to above the bed top, so every
ball that has volume in the bed or the conus is present — including the
layer centred 2.449 cm above the bed top, whose lower 0.55 cm is inside the
bed and replaces the top layer's upper 0.55 cm, which the bed plane clips
off. (Without it the top slab would be under-packed.)

# Fuel/dummy identity

Assigned to BALLS, never to tiles: every tile holding a piece of a ball
(2 for an A ball, 3 for a B ball) sees the same identity, because each
reads it from [`Self::is_fuel`] through the ball's [`BallId`].

Under [`FuelAssignment::Paper`] the eligible balls are those centred above
the bed floor with volume inside the bed cylinder (centre within one ball
radius of it, radially and at the top). Balls centred below the floor are
the conus and are all dummy (Terry 2005 s2). The eligible balls are taken
in **layer order from the floor up, then by distance from the axis, then by
angle**, and ball `n` is fuel when `floor((n+1) f) > floor(n f)`, `f = 0.57`
— the same low-discrepancy (Bresenham) rule [`bed_tile_levels`] used for
tiles. That order makes the split right in every prefix of layers and of
every annulus, and, because it runs from the floor up, a taller loading
never reshuffles the balls below it.

```rust
pub struct TwoBallBed {
    pub cell: HexBedCell,
    pub n_rings: usize,
    pub n_levels: usize,
    pub z_bottom: f64,
    pub bed_radius: f64,
    pub bed_bottom: f64,
    pub bed_top: f64,
    pub conus_floor: f64,
    pub assignment: FuelAssignment,
    pub eligible_balls: usize,
    pub fuel_balls: usize,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell` | `HexBedCell` | The unit cell (pitch 6.6106 cm, height 9.798 cm, 6 cm balls). |
| `n_rings` | `usize` | Hex rings in the lattice (including the central tile). |
| `n_levels` | `usize` | Axial lattice levels, each one A-B pair (`cell.height`) tall. |
| `z_bottom` | `f64` | z \[cm\] of the bottom face of lattice level 0. |
| `bed_radius` | `f64` | Bed cylinder radius \[cm\]. |
| `bed_bottom` | `f64` | Bed floor \[cm\] (= top of the conus). |
| `bed_top` | `f64` | Bed top \[cm\]. |
| `conus_floor` | `f64` | Conus floor \[cm\]. |
| `assignment` | `FuelAssignment` | The rule the identities were assigned by. |
| `eligible_balls` | `usize` | Balls that took part in the 57:43 split. |
| `fuel_balls` | `usize` | Of which fuelled. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(cell: HexBedCell, n_rings: usize, n_axial: usize, conus_height: f64, bed_radius: f64, assignment: FuelAssignment) -> Self { /* ... */ }
  ```
  Build the bed.

- ```rust
  pub fn lattice_centre_z(self: &Self) -> f64 { /* ... */ }
  ```
  z \[cm\] of the lattice centre, to pass to `HexLattice::from_rings_3d`.

- ```rust
  pub fn centre(self: &Self, id: BallId) -> [f64; 3] { /* ... */ }
  ```
  Global centre \[cm\] of ball `id`.

- ```rust
  pub fn is_fuel(self: &Self, id: BallId) -> bool { /* ... */ }
  ```
  Whether ball `id` is fuelled. Balls outside the lattice's range are

- ```rust
  pub fn tile_balls(self: &Self, a: i32, b: i32, level: i32) -> [BallId; 5] { /* ... */ }
  ```
  The five balls tile `(a, b, level)` holds pieces of, in

- ```rust
  pub fn tile_mask(self: &Self, a: i32, b: i32, level: i32) -> u8 { /* ... */ }
  ```
  Fuel mask of tile `(a, b, level)`: bit `i` set when the ball at

- ```rust
  pub fn all_balls(self: &Self) -> Vec<BallId> { /* ... */ }
  ```
  Every ball any tile of the lattice holds a piece of, in a deterministic

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TwoBallBed { /* ... */ }
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
### Functions

#### Function `close_packed_layer_spacing`

**Attributes:**

- `MustUse { reason: None }`

Close-packed layer spacing for spheres of diameter `d` \[cm\]: `d*sqrt(2/3)`.

```rust
pub fn close_packed_layer_spacing(d: f64) -> f64 { /* ... */ }
```

#### Function `close_packed_fraction`

**Attributes:**

- `MustUse { reason: None }`

Packing fraction of ordered close packing (FCC/HCP), `pi/(3*sqrt(2))`.

```rust
pub fn close_packed_fraction() -> f64 { /* ... */ }
```

#### Function `bed_tile_levels`

**Tile assignment for the hex-prism pebble bed**, `levels[axial][ring][elem]`,
in the layout [`outram_mc_libs::geometry::lattice::HexLattice::from_rings_3d`]
expects.

NEW WORK, not a port.

**Used only by the one-ball-per-tile `core_model::assemble` since
2026-09-25.** The explicit-TRISO bed assigns identities per BALL through
[`TwoBallBed`], because in the two-ball cell a tile holds pieces of five
balls and a per-tile identity would make one pebble part fuel, part
graphite.

# The paper's recipe

> hexagonal prism unit cells assembled as layers, layer height 9.798 cm …
> fuel and moderator selected per layer to give **0.57 : 0.43** … the fuel
> step size is one layer precisely so no fractional balls appear

# How the split is realised

Deterministically, by a **low-discrepancy rule** rather than a random draw:
tile `n` (counting through the whole stack in ring-major order) is fuel when
`floor((n+1) * f) > floor(n * f)` for `f = 0.57`. That is Bresenham's line
algorithm, and it gives the closest achievable ratio at **every prefix**,
not just overall — so a partially built or truncated core still carries the
right mixture, which a block assignment would not.

A random draw at 0.57 would also converge, but it would make the geometry
seed-dependent, and a code-to-code comparison should not be.

# Parameters
- `n_rings` — hexagonal rings, including the central tile.
- `n_axial` — layers in the stack.
- `fuel_universe`, `moderator_universe` — universe indices to place.

```rust
pub fn bed_tile_levels(n_rings: usize, n_axial: usize, fuel_universe: usize, moderator_universe: usize) -> Vec<Vec<Vec<usize>>> { /* ... */ }
```

#### Function `tile_ball`

**Attributes:**

- `MustUse { reason: None }`

The ball at `site` of tile `(a, b)` in lattice level `level`.

The vertex ownership follows from the Y-orientation tile centres
`(sqrt(3)/2 a, b + a/2) * pitch`: tile `(a, b)`'s 120 degree vertex is the
0 degree vertex of tile `(a-1, b+1)`, and its 240 degree vertex that of
`(a-1, b)`. Checked numerically by
`every_tile_resolves_a_shared_ball_to_one_position` below.

```rust
pub fn tile_ball(a: i32, b: i32, level: i32, site: BallSite) -> BallId { /* ... */ }
```

#### Function `tile_xy`

**Attributes:**

- `MustUse { reason: None }`

Planar centre \[cm\] of tile `(a, b)` in a `HexOrientation::Y` lattice
centred on the origin — the same arithmetic as `HexLattice::center_offset`.

```rust
pub fn tile_xy(a: i32, b: i32, pitch: f64) -> [f64; 2] { /* ... */ }
```

#### Function `hex_ring`

**Attributes:**

- `MustUse { reason: None }`

Hexagonal ring index (0 = centre) of skewed coordinates `(a, b)`.

```rust
pub fn hex_ring(a: i32, b: i32) -> usize { /* ... */ }
```

#### Function `count_tiles`

**Attributes:**

- `MustUse { reason: None }`

Count `(fuel, moderator)` tiles in an assignment from [`bed_tile_levels`].

```rust
pub fn count_tiles(levels: &[Vec<Vec<usize>>], fuel_universe: usize) -> (usize, usize) { /* ... */ }
```

## Module `reflector`

**HTR-10 reflector zone compositions**, IAEA-TECDOC-1382 part 2, Table 4-3
(`bn:op-867c.8`, gh #214).

# Why these and not the RMC paper's

Li, Yu & Wei (2014) **defer** reflector and structural modelling to
IAEA-TECDOC-1382 — which is why `htr10_rmc`'s module docs record the k-vs-height
curve as not reproducible without it. This is that data.

# What the table is

Spatially **homogenised** atom densities for the R-Z reactor-physics model of
Figure 4.10, zones 0..=82, in atoms/(barn·cm). Two nuclides only: carbon and
**natural** boron. The model is recommended to include core structures "only
until the carbon bricks".

Additional parameters stated alongside the table:
reflector graphite density **1.76 g/cm³**, and B4C weight ratio in boronated
carbon brick **5 %**.

# Two things to be careful about

- **The boron is NATURAL boron**, not B-10. Only 19.9 at.% of it absorbs.
  Reading these as B-10 densities over-absorbs by ~5x — the same misreading
  `pebble_beds::htr10::BoronReading::AsElementalB10` exists to price, where it
  was worth −5711 pcm on a single pebble.
- **The densities are homogenised in R-Z.** TECDOC says so explicitly: for a
  three-dimensional model they "are to be corrected by taking into
  consideration of the boring geometries". Using them unadjusted in 3-D
  smears the control-rod and helium-flow borings uniformly, which is a real
  approximation and not a transcription detail.

# Zone 5

The top core cavity carries **no entry** in the table — it is void, and is
represented here by its absence rather than by zeros, so a caller that asks
for it gets `None` instead of a suspiciously empty material.

```rust
pub mod reflector { /* ... */ }
```

### Types

#### Struct `ZoneComposition`

Homogenised composition of one reflector zone \[atoms/(barn·cm)\].

```rust
pub struct ZoneComposition {
    pub carbon: f64,
    pub natural_boron: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `carbon` | `f64` | Carbon atom density. |
| `natural_boron` | `f64` | **Natural** boron atom density — not B-10. See the module docs. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ZoneComposition { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ZoneComposition) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `zone_composition`

**Attributes:**

- `MustUse { reason: None }`

Composition of reflector `zone`, or `None` for a zone the table does not
list — which is zone 5, the top core cavity, and any index above
[`MAX_ZONE`].

```rust
pub fn zone_composition(zone: usize) -> Option<ZoneComposition> { /* ... */ }
```

#### Function `listed_zones`

**Attributes:**

- `MustUse { reason: None }`

Every zone the table lists, ascending.

```rust
pub fn listed_zones() -> Vec<usize> { /* ... */ }
```

### Constants and Statics

#### Constant `REFLECTOR_GRAPHITE_DENSITY`

Density of reflector graphite \[g/cm³\], stated with Table 4-3.

```rust
pub const REFLECTOR_GRAPHITE_DENSITY: f64 = 1.76;
```

#### Constant `B4C_WEIGHT_RATIO_IN_BORONATED_BRICK`

Weight ratio of B4C in boronated carbon brick \[-\], stated with Table 4-3.

```rust
pub const B4C_WEIGHT_RATIO_IN_BORONATED_BRICK: f64 = 0.05;
```

#### Constant `MAX_ZONE`

Highest zone number in the table.

```rust
pub const MAX_ZONE: usize = 82;
```

## Module `core_model`

**Assembled HTR-10 core geometry** — `bn:op-867c.14`, gh #214.

Puts together the pieces gated separately elsewhere: the hex-prism bed
([`super::bed`]), the reflector ([`super::reflector`]), the TRISO array
(`outram_mc_libs::pebble_beds::sphere_packing::cubic_array_in_ball`) and
hybrid delta/surface tracking (`outram_mc_libs` `TrackingMethod`).

# Parameterised by SIZE, deliberately

The full core is ~24,000 hex tiles with a nested TRISO lattice in each
fuelled one. The largest lattice ever built in this crate before today was
**37 tiles**, so full scale is ~650x beyond anything exercised, and the
runtime, the memory and whether depth-3 transport holds up there are all
unmeasured.

Building it only at full size would be one all-or-nothing attempt. Taking
`n_rings` and `n_axial` as parameters makes the measurement incremental and
lets the cost be extrapolated rather than guessed — which is what
`op-867c.14` actually asks for.

# What this is NOT

Not yet a benchmark model. The pebbles here carry a **homogenised** fuel
sphere rather than an explicit TRISO lattice, so the double heterogeneity is
absent and `k` from this is not comparable to the paper. That is deliberate:
this exists to measure the cost of the BED, which is the part at unprecedented
scale. The TRISO nesting multiplies on top and is priced separately.

```rust
pub mod core_model { /* ... */ }
```

### Modules

## Module `mat`

```rust
pub mod mat { /* ... */ }
```

### Constants and Statics

#### Constant `KERNEL`

TRISO UO2 kernel.

```rust
pub const KERNEL: usize = 0;
```

#### Constant `BUFFER`

Buffer PyC.

```rust
pub const BUFFER: usize = 1;
```

#### Constant `IPYC`

Inner PyC.

```rust
pub const IPYC: usize = 2;
```

#### Constant `SIC`

SiC.

```rust
pub const SIC: usize = 3;
```

#### Constant `OPYC`

Outer PyC.

```rust
pub const OPYC: usize = 4;
```

#### Constant `GRAPHITE`

Matrix graphite inside the fuel zone, and the pebble shell.

```rust
pub const GRAPHITE: usize = 5;
```

#### Constant `HELIUM`

Helium between pebbles.

```rust
pub const HELIUM: usize = 6;
```

#### Constant `REFLECTOR`

Reflector graphite (TECDOC Table 4-3).

```rust
pub const REFLECTOR: usize = 7;
```

#### Constant `BORONATED`

Boronated carbon brick — the outermost reflector annulus.

```rust
pub const BORONATED: usize = 8;
```

#### Constant `BORED_GRAPHITE`

Side-reflector graphite homogenised with its control-rod borings
(TECDOC zones 31–40): 28 % less carbon than solid zone-22 graphite.

```rust
pub const BORED_GRAPHITE: usize = 9;
```

#### Constant `HOMOG_DUMMY`

**Homogenised dummy pebbles** — pebble graphite at the bed's filling
fraction, i.e. what the discharge tube actually contains.

Terry (2005) §2: *"the conus and discharge tube contained only dummy
pebbles"*. Solid reflector graphite there over-reflects; pure helium
(the bounding ablation) under-reflects. This is the physical value.

```rust
pub const HOMOG_DUMMY: usize = 10;
```

#### Constant `FUEL`

Homogenised fuel zone, used only by [`super::assemble`].

```rust
pub const FUEL: usize = KERNEL;
```

### Types

#### Struct `AssembledCore`

A built core and the sizes that describe it.

```rust
pub struct AssembledCore {
    pub geometry: outram_mc_libs::geometry::geometry::Geometry,
    pub tiles: usize,
    pub cells: usize,
    pub universes: usize,
    pub bed_radius: f64,
    pub bed_half_height: f64,
    pub lat_pitch: f64,
    pub lat_height: f64,
    pub conus_floor: f64,
    pub cavity_top: f64,
    pub refl_top: f64,
    pub refl_bottom: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `outram_mc_libs::geometry::geometry::Geometry` | The geometry. |
| `tiles` | `usize` | Hex tiles in the bed lattice (every axial level, conus included). |
| `cells` | `usize` | Cells in the geometry. |
| `universes` | `usize` | Universes in the geometry. |
| `bed_radius` | `f64` | Bed cylinder radius \[cm\]. |
| `bed_half_height` | `f64` | Bed half-height \[cm\]. |
| `lat_pitch` | `f64` | Hex pitch \[cm\] of the bed lattice. [`assemble_explicit_triso`]: the<br>paper's two-ball prism, 6.6106 cm ([`HexBedCell::from_paper`]).<br>[`assemble`]: ~~solved from the fuel-zone target~~ still solved so its<br>axially clipped one-ball tile realises the paper's fuel-zone fraction,<br>6.6086 cm (gh:#308: that path keeps the one-ball construction). |
| `lat_height` | `f64` | Axial tile height \[cm\]. [`assemble_explicit_triso`]: 9.798 cm, one A-B<br>layer pair holding two balls. [`assemble`]: 4.899 cm, one ball. In both<br>the bed is `n_axial x 4.899` cm tall, i.e. `2 * bed_half_height`, NOT<br>`n_axial * lat_height`. |
| `conus_floor` | `f64` | Bottom of the conus \[cm\] — the deepest fuelled z. Equal to<br>`-bed_half_height` when no conus is modelled. |
| `cavity_top` | `f64` | Top of the empty core cavity \[cm\], i.e. where the axial reflector<br>begins. Equals `bed_half_height` when no reflector is built. |
| `refl_top` | `f64` | Top of the whole assembled model \[cm\]: cavity top + the 130 cm axial<br>reflector. Equals `bed_half_height` when no reflector is built. |
| `refl_bottom` | `f64` | Bottom of the whole assembled model \[cm\] (negative): conus floor less<br>the fixed [`HTR10_BOTTOM_REFLECTOR_CM`]. Equals `-bed_half_height` when<br>no reflector is built.<br><br>The model is **not** symmetric about `z = 0` (the bed mid-height): see<br>[`HTR10_BOTTOM_REFLECTOR_CM`] for why the old mirrored bottom was wrong.<br>With a reflector, `refl_top - refl_bottom` is [`HTR10_MODEL_HEIGHT_CM`]<br>at every loading. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
#### Enum `TileCellRole`

What a cell of a bed tile universe is: which kind of pebble a point in it
belongs to. Lets a sampler measure the realised fuel-BALL fraction from the
assembled geometry rather than from the assignment that built it.

```rust
pub enum TileCellRole {
    FuelZone,
    FuelShell,
    DummyBall,
    Helium,
}
```

##### Variants

###### `FuelZone`

Inside a fuelled pebble's 2.5 cm fuel zone (the TRISO lattice).

###### `FuelShell`

In a fuelled pebble's fuel-free graphite shell.

###### `DummyBall`

Inside a dummy (all-graphite) pebble.

###### `Helium`

Helium between pebbles.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TileCellRole { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &TileCellRole) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `cavity_above_bed`

**Attributes:**

- `MustUse { reason: None }`

Void height \[cm\] above a bed of `bed_full_height` cm.

# There is only one treatment, and this is it

The core cavity is **fixed hardware** ([`HTR10_CORE_CAVITY_CM`], Terry 2005
Fig. 2: `z = 130.0` to `351.818`). It does not grow when fuel is added — it
is the *void above the bed* that shrinks. So the void is simply whatever
the bed does not occupy.

## What was here before, and why it is gone

~~The model applied a constant 98.758 cm of void at every loading, behind
an `OUTRAM_HTR10_FIXED_CAVITY=1` opt-in, "so no committed result moves
silently".~~ **REMOVED 2026-09-24 (maintainer direction).** That constant
is the void at **one** loading — the benchmark's 123.06 cm, where
`221.818 - 123.06 = 98.758` — and applying it everywhere silently grows the
whole cavity with the bed.

It was wrong in a known direction away from that point: at lower loading
the model had too little void, so reflector graphite sat where the reactor
has helium and `k` read HIGH; at higher loading it had too much void and
`k` read LOW. Measured 2026-09-18 across four loadings (dk vs RMC,
height-matched): `-54` at 102.9 cm, `-1259` at 122.5 cm, `-2194` at
147.0 cm, `-1640` at 171.5 cm — exactly that signature. **The -54 pcm
agreement at 102.9 cm was two errors cancelling, not correctness.**

Keeping the correct treatment behind an opt-in meant nobody passed it and a
loading sweep could be run incoherently with nothing to catch it (gh:#292);
it also violated the workspace rule that correct physics is the default and
an ablation must be the explicit act. Both the flag and the constant are
now deleted, so every caller gets the fixed cavity with no way to opt out.

**This changes results measured under the old default.** Any recorded
number that did not set `OUTRAM_HTR10_FIXED_CAVITY=1` was computed with the
constant void and must be re-measured before it is quoted again; the shift
is ~zero at the benchmark loading and grows with distance from it.

```rust
pub fn cavity_above_bed(bed_full_height_cm: f64) -> f64 { /* ... */ }
```

#### Function `assemble`

**Assemble a delta-tracked pebble bed inside a surface-tracked reflector.**

# NOT a benchmark model, and not only because the fuel is homogenised

**This is a COST INSTRUMENT.** Beyond the homogenised fuel zone its
reflector is a single zone-22 graphite cell filling everything inside
r = 190 cm that is not the bed, so it is missing the **empty core cavity**
(solid graphite sits there), the **boronated carbon bricks**, the **cold
coolant annulus**, the **conus**, the **discharge tube** and the **bored
control-rod band**. Materials 8-10 of [`super::materials::htr10_material_set`]
are never referenced. Against [`assemble_explicit_triso`] that is roughly
**+15 500 pcm** in terms the V&V record has already priced individually.
Use [`assemble_explicit_triso`] for anything that reports `k` or feeds
group constants to another solver. gh:#308.

~~The bed cell's pitch and layer height come from [`HexBedCell::from_paper`],
so the geometry is the paper's even when the size is scaled down.~~
**CORRECTED 2026-09-25:** only the layer height does, halved (one ball per
4.899 cm tile); the pitch is SOLVED (6.6086 cm) so the clipped one-ball
tile realises the paper's fuel-zone fraction, and the pebbles
interpenetrate (gh:#309/#310, fixed in [`assemble_explicit_triso`] only).

# Parameters
- `n_rings` — a FLOOR on the lattice ring count. The bed radius is fixed at
  the physical [`HTR10_CORE_RADIUS_CM`] and the lattice is sized to tile it
  completely, so this cannot shrink the core; it can only over-tile.
- `n_axial` — axial layers, each [`HexBedCell::height`]/2 tall.
- `majorant_index` — which entry of the caller's majorant table the bed uses.

```rust
pub fn assemble(n_rings: usize, n_axial: usize, majorant_index: usize) -> AssembledCore { /* ... */ }
```

#### Function `assemble_explicit_triso`

**Assemble the core with an EXPLICIT TRISO lattice in each fuelled pebble** —
the double-heterogeneous model the benchmark actually specifies.

~~**Open defects in this construction:** gh:#309 (pebble-shell carbon
clipped), gh:#310 (pebbles in axial contact).~~ **FIXED 2026-09-25 — the
bed is now the paper's two-ball prism cell** (see "The bed" below): no ball
is clipped by its own tile, no two balls overlap, and every pebble is the
whole 6 cm sphere. ~~gh:#311 (no B-11)~~ **CORRECTED 2026-09-25:** #311 is
closed and B-11 is placed (`materials::htr10_material_set`, the `b11`
closure). Results from it are tentative — see the module docs,
"Verification status".

# The bed: two balls per tile (gh:#309 step 2, gh:#310)

The hex lattice tile IS the paper's prism, [`HexBedCell::from_paper`]:
pitch 6.6106 cm, height 9.798 cm = one A-B layer pair, two balls per tile,
packing 0.610. Each tile universe holds pieces of five balls
([`super::bed::BallSite`]): two A balls on its axis at its top and bottom
faces (half each), and three B balls at alternate vertices at mid-height (a
third each). The spheres are centred OUTSIDE or ON the tile boundary; that
is exact here, because `Geometry::locate` evaluates a tile universe's cell
regions in tile-local coordinates only for points the lattice has already
placed in that tile, so each tile draws exactly its own piece and the
neighbours holding the rest of the same ball draw theirs.

Fuel/dummy is a property of the BALL ([`super::bed::TwoBallBed`]), and a
tile's universe is the variant for its five balls' identities (up to 2^5,
only those used are built), so a ball split across 2 or 3 tiles is the same
kind of pebble in all of them.

Nearest centre distances: in-plane 6.6106 cm, A to B
`hypot(pitch/sqrt(3), height/2)` = 6.2102 cm, A to A (axial) 9.798 cm, all
greater than the 6.0 cm diameter; the smallest gap is 0.210 cm.
`htr10_rmc::tests::no_two_balls_of_the_built_bed_overlap` checks it on the
built geometry.

# Parameters
- `n_rings` — a FLOOR on the lattice ring count (the bed radius is the
  physical 90 cm and the lattice is sized to tile it).
- `n_axial` — the fuel LOADING HEIGHT in half-layers of 4.899 cm (one ball
  layer each), so the bed is `n_axial x 4.899` cm: 20 / 25 / 41 give
  97.980 / 122.474 / 200.858 cm, the same heights as before the two-ball
  change. The A-B stacking is anchored at the bed floor (layer 0 is an A
  layer), so an odd `n_axial` simply ends on an A layer and an even one on
  a B layer; see [`super::bed::TwoBallBed`].
- `majorant_index` — which entry of the caller's majorant table the bed
  uses; `usize::MAX` surface-tracks the bed.

Universes: 0 root, [`TRISO_PARTICLE_UNIVERSE`], [`TRISO_MATRIX_UNIVERSE`],
then one per bed-tile fuel mask in use (35 in all at 14 rings, all 32 masks
occur). Tile cell ids encode their role, see [`tile_cell_role`].

~~Four coordinate levels~~ **CORRECTED 2026-09-25 — three coordinate
levels**: root → (bed hex lattice) → bed-tile universe (pieces of five
pebbles since 2026-09-25; one pebble before) → (TRISO rect lattice, entered
through the fuel-zone cell's translation to its ball centre) → TRISO
particle universe. A lattice selects the next level's universe but is not a
level itself. Verified by locating a kernel in the assembled 14 x 25 core:
`path.levels.len() == 3`, lattices `[None, Some(0), Some(1)]`
(`examples/htr10_geometry_images.rs` prints it; re-checked on the two-ball
cell 2026-09-25). Depth-3 descent was gated
in `outram-mc-libs` `tests/nested_lattice_depth3.rs`, which also counts
`levels.len()`; ~~this is depth 4~~ this is the **same** depth.

# The TRISO lattice

A cubic array clipped to the fuel zone, keeping only whole particles, per
`cubic_array_in_ball`. Expressed as a `RectLattice` whose tiles hold either a
particle universe or matrix graphite, with `outer` = matrix so anything
beyond the array's extent is graphite.

The realised particle count is **8340**, not the paper's stated 8335 — see
`cubic_array_in_ball`'s docs for why 8335 is unattainable (the count moves in
symmetry shells). That is +0.060 % in fuel volume.

```rust
pub fn assemble_explicit_triso(n_rings: usize, n_axial: usize, majorant_index: usize) -> AssembledCore { /* ... */ }
```

#### Function `tile_cell_role`

**Attributes:**

- `MustUse { reason: None }`

The role of a bed-tile cell from its id, or `None` for any other cell.

```rust
pub fn tile_cell_role(cell_id: i32) -> Option<TileCellRole> { /* ... */ }
```

### Constants and Statics

#### Constant `PAPER_FILLING_FRACTION`

Material slots the assembled geometry expects, in order.

The first five mirror `DhUniverse::pebble`'s TRISO layer order so
`pebble_beds::htr10::fuel_pebble_materials` can be used directly.
Pebble-bed filling fraction stated by Li, Yu & Wei (2014) for the HTR-10
core — the fraction of bed volume occupied by pebbles. Sets the fuel per
unit volume. ~~so the assembled geometry is solved to realise it~~
**CORRECTED 2026-09-25:** [`assemble_explicit_triso`] realises it by
construction (the paper's two-ball cell, [`HexBedCell::from_paper`], whose
pitch is derived from it); only [`assemble`] still solves its pitch for it.
Also the discharge-tube smear (`mat::HOMOG_DUMMY`), which since the
two-ball cell agrees with the bed it homogenises (sampled 0.6096-0.6097).

```rust
pub const PAPER_FILLING_FRACTION: f64 = 0.61;
```

#### Constant `HTR10_CORE_RADIUS_CM`

HTR-10 active core radius \[cm\] — 180 cm diameter (IAEA-TECDOC-1382).
The bed cylinder is built at this radius and the hex lattice is sized to
tile it completely.

```rust
pub const HTR10_CORE_RADIUS_CM: f64 = 90.0;
```

#### Constant `HTR10_GRAPHITE_OUTER_CM`

Outer radius \[cm\] of the graphite reflector, where the boronated carbon
bricks begin — Terry et al. (2005) Fig. 2, via
`kovan-literature/derived/terry2005-htr10-rz-zone-geometry.md`.

```rust
pub const HTR10_GRAPHITE_OUTER_CM: f64 = 167.793;
```

#### Constant `HTR10_COOLANT_INNER_CM`

Inner radius \[cm\] of the cold coolant flow annulus — Terry (2005) Fig. 2,
independently corroborated as channel r 144.6 − diameter 8.0 / 2.

```rust
pub const HTR10_COOLANT_INNER_CM: f64 = 140.6;
```

#### Constant `HTR10_COOLANT_OUTER_CM`

Outer radius \[cm\] of the cold coolant flow annulus (144.6 + 8.0 / 2).

```rust
pub const HTR10_COOLANT_OUTER_CM: f64 = 148.6;
```

#### Constant `HTR10_CORE_CAVITY_CM`

Total height \[cm\] of the **core cavity**, conus top to cavity top.

Terry (2005) Fig. 2 / IAEA-TECDOC-1382: `z = 130.0` to `351.818`. This is
**fixed geometry** — it does not depend on how much fuel is loaded.

```rust
pub const HTR10_CORE_CAVITY_CM: f64 = 221.818;
```

#### Constant `HTR10_AXIAL_REFLECTOR_CM`

Axial reflector thickness \[cm\] beyond the core cavity / bed.

The full benchmark model is 610 cm tall (Terry 2005, corroborated against
Table 2), with the core cavity ending 130 cm below the model top — so there
is ~130 cm of graphite above the cavity, not the ~1 cm an unextended
`bed_half_height + 100` leaves once the cavity is carved out of it.

```rust
pub const HTR10_AXIAL_REFLECTOR_CM: f64 = 130.0;
```

#### Constant `HTR10_BOTTOM_REFLECTOR_CM`

Bottom reflector thickness \[cm\] below the conus floor.

Terry (2005) Fig. 2: the conus ends at `z = 388.764` and the model at
`z = 610.0`, so **221.236 cm, fixed** -- it does not depend on the loading.

## What was here before, and why it is gone

~~The outer box was symmetric: its bottom plane was the mirror of the top,
`-(bed_half_height + cavity + 130)`.~~ **REMOVED 2026-09-25.** Because the
origin is the bed's MID-HEIGHT, the mirrored bottom rode up with the bed, so
the bottom reflector was `314.872 - bed_height` cm: 216.9 cm at the lowest
loading (97.98 cm), 192.4 cm at the benchmark (122.47 cm), **114.0 cm** at
the tallest (200.86 cm) -- up to 107 cm thinner than the reactor's, by an
amount that grew with the bed, and a model 581 cm tall rather than 610. Same
failure class as the constant-cavity void: a loading-dependent geometry
error hidden by a construction that is exact at one point. Every result
computed before this change used the mirrored bottom.

```rust
pub const HTR10_BOTTOM_REFLECTOR_CM: f64 = _;
```

#### Constant `HTR10_MODEL_HEIGHT_CM`

Full axial extent \[cm\] of the benchmark model, Terry (2005) Fig. 2
(`z = 0` to `610`): top reflector + core cavity + conus + bottom reflector.

```rust
pub const HTR10_MODEL_HEIGHT_CM: f64 = 610.0;
```

#### Constant `HTR10_CONUS_HEIGHT_CM`

Height \[cm\] of the **conus** — the sloping bottom of the pebble bed,
tapering from the core radius to the discharge tube.

Terry (2005) Fig. 2: z = 351.818 (conus top, "zero core height") to
z = 388.764, corroborated against TECDOC-1382 Table 2's stated 36.946.
**It is full of pebbles**, so omitting it omits fuel.

```rust
pub const HTR10_CONUS_HEIGHT_CM: f64 = 36.946;
```

#### Constant `HTR10_DISCHARGE_TUBE_RADIUS_CM`

Fuel-discharge-tube radius \[cm\] — the conus's lower radius.

```rust
pub const HTR10_DISCHARGE_TUBE_RADIUS_CM: f64 = 25.0;
```

#### Constant `HTR10_REFLECTOR_OUTER_CM`

Outer radius \[cm\] of the boronated carbon bricks = reflector outer
boundary (380 cm diameter / 2).

```rust
pub const HTR10_REFLECTOR_OUTER_CM: f64 = 190.0;
```

#### Constant `HTR10_CONTROL_ROD_INNER_CM`

Inner radius \[cm\] of the side-reflector band carrying the control-rod
borings — Terry (2005) Fig. 2, corroborated as channel r 102.1 − 13/2.

```rust
pub const HTR10_CONTROL_ROD_INNER_CM: f64 = 95.6;
```

#### Constant `HTR10_CONTROL_ROD_OUTER_CM`

Outer radius \[cm\] of that band (102.1 + 13/2).

```rust
pub const HTR10_CONTROL_ROD_OUTER_CM: f64 = 108.6;
```

#### Constant `HTR10_BORED_CARBON`

Carbon atom density \[atoms/b·cm\] of the **bored** side-reflector band.

IAEA-TECDOC-1382 Table 4-3 zones 31–40 — ten consecutive zones sharing one
reduced density, which is what a homogenised boring region looks like.
Against zone 22's 8.82418e-2 this is **28.1 % less carbon**.

Modelling that band as solid zone-22 graphite (as this did) over-reflects
and over-moderates in the reflector band *nearest the core*, which is the
highest-leverage place in the whole reflector to get wrong.

```rust
pub const HTR10_BORED_CARBON: f64 = 0.634459E-01;
```

#### Constant `HTR10_BORED_BORON`

Natural-boron atom density \[atoms/b·cm\] of the same zones 31–40.

```rust
pub const HTR10_BORED_BORON: f64 = 0.340640E-06;
```

#### Constant `TRISO_PARTICLE_UNIVERSE`

Universe index of one TRISO particle (kernel, four coatings, matrix
beyond) in [`assemble_explicit_triso`]'s geometry — what the TRISO
`RectLattice` places where a whole particle fits.

```rust
pub const TRISO_PARTICLE_UNIVERSE: usize = 1;
```

#### Constant `TRISO_MATRIX_UNIVERSE`

Universe index of plain matrix graphite, the TRISO lattice's `outer` and
its non-particle tiles.

```rust
pub const TRISO_MATRIX_UNIVERSE: usize = 2;
```

#### Constant `TILE_FUEL_ZONE_CELL_ID`

Cell-id bases of the pebble cells inside a bed tile universe of
[`assemble_explicit_triso`]. A tile cell's id is `base + 10*mask + site`
(`site` the index in [`BallSite::ALL`], `mask` the tile's 5-bit fuel mask),
or `base + 10*mask` for the helium cell. Read back with
[`tile_cell_role`].

```rust
pub const TILE_FUEL_ZONE_CELL_ID: i32 = 1000;
```

#### Constant `TILE_FUEL_SHELL_CELL_ID`

See [`TILE_FUEL_ZONE_CELL_ID`].

```rust
pub const TILE_FUEL_SHELL_CELL_ID: i32 = 2000;
```

#### Constant `TILE_DUMMY_BALL_CELL_ID`

See [`TILE_FUEL_ZONE_CELL_ID`].

```rust
pub const TILE_DUMMY_BALL_CELL_ID: i32 = 3000;
```

#### Constant `TILE_HELIUM_CELL_ID`

See [`TILE_FUEL_ZONE_CELL_ID`].

```rust
pub const TILE_HELIUM_CELL_ID: i32 = 4000;
```

## Module `control_rod`

HTR-10 control rods — smeared composition of the side-reflector boring band.

# What this is for

The HTR-10 core model in [`super::core_model`] carries the ten control-rod
borings as a single homogenised annulus (`mat::BORED_GRAPHITE`, TECDOC zones
31-40, r 95.6-108.6 cm) at **28 % less carbon** than solid reflector
graphite. That band represents the borings as *empty*: there is no absorber
anywhere in the model, so it can only ever represent rods **fully
withdrawn**, and control-rod worth cannot be computed from it at all.

This module supplies the missing half — what the band contains when rods are
in it — derived from the published rod geometry and B4C density, with
nothing fitted.

# Source

IAEA-TECDOC-1382 § 4.1.2, transcribed with its benchmark values in
`crates/kovan-literature/derived/tecdoc1382-htr10-control-rods.md`. Read that
record before changing any constant here.

# The approximation this makes, stated plainly

Ten discrete rods spaced azimuthally are smeared into one axisymmetric
annulus. **TECDOC explicitly warns about this**: its Table 4-3 densities are
spatially homogenised and "if one is to consider three-dimensional effects,
the homogenized densities are to be corrected by taking into consideration
of the boring geometries".

Consequences, in order of how much they matter:

- **Self-shielding is lost.** A B4C annulus is intensely black to thermal
  neutrons; smearing it over 14x its own volume replaces a strongly
  self-shielded absorber with a dilute one, which **over-predicts** worth.
  This is the dominant error and it has a known sign.
- **The one-rod problems (B32, B42) cannot be represented at all.** One rod
  of ten is not an axisymmetric perturbation. Do not compare a smeared
  result against them.
- Azimuthal flux tilt is absent by construction.

So the ten-rod problems (B31, B41) are the only ones this geometry can
honestly address, and even there the expected bias is positive.

```rust
pub mod control_rod { /* ... */ }
```

### Types

#### Struct `SmearedRodComposition`

Atom densities added to the boring band when rods occupy it
\[atoms/(barn·cm)\].

Both are **additions** to the band's existing homogenised graphite
([`super::core_model::HTR10_BORED_CARBON`] and `HTR10_BORED_BORON`), not
replacements: the reduced-density band already represents graphite plus
void, and inserting a rod fills part of that void.

```rust
pub struct SmearedRodComposition {
    pub natural_boron: f64,
    pub carbon: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `natural_boron` | `f64` | **Natural** boron from the B4C — not B-10. Only 19.9 at.% absorbs<br>strongly; reading this as B-10 over-absorbs by roughly 5x, the same<br>misreading [`super::reflector`] documents. |
| `carbon` | `f64` | Carbon from the B4C, over and above the band's graphite. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SmearedRodComposition { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &SmearedRodComposition) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `b4c_molecular_density`

**Attributes:**

- `MustUse { reason: None }`

Atom density of B4C **molecules** in the solid absorber \[atoms/(barn·cm)\].

`rho * N_A / M`, with `M = 4 M_B + M_C`. Boron is natural, so `M_B` is the
natural atomic weight and no isotopic split is applied here.

```rust
pub fn b4c_molecular_density() -> f64 { /* ... */ }
```

#### Function `radial_packing_fraction`

**Attributes:**

- `MustUse { reason: None }`

Fraction of the boring band's cross-sectional area occupied by B4C when all
ten rods are present \[-\].

`10 * pi (r_out^2 - r_in^2) / (pi (R_out^2 - R_in^2))`, with the band radii
taken from [`super::core_model`] so the two cannot drift apart.

```rust
pub fn radial_packing_fraction() -> f64 { /* ... */ }
```

#### Function `axial_duty_fraction`

**Attributes:**

- `MustUse { reason: None }`

Fraction of the rod's own length that is actually B4C \[-\].

The absorber is **not** a continuous column: five 487 mm segments separated
by 36 mm steel joints, with steel ends. A model that treats the whole rod
span as absorber over-predicts worth by the reciprocal of this.

```rust
pub fn axial_duty_fraction() -> f64 { /* ... */ }
```

#### Function `smeared_composition`

**Attributes:**

- `MustUse { reason: None }`

Composition added to the boring band over the axially-inserted length.

`fraction_inserted` is the fraction of the band's height the rods occupy,
`0.0` fully withdrawn to `1.0` fully inserted. It scales the result
linearly, which is what smearing an absorber over a taller region means —
it is **not** a model of differential worth, because worth depends on where
in the flux shape the absorber sits, not only on how much of it there is.
Use an axially resolved geometry for that.

Values outside `[0, 1]` are clamped rather than extrapolated.

```rust
pub fn smeared_composition(fraction_inserted: f64) -> SmearedRodComposition { /* ... */ }
```

### Constants and Statics

#### Constant `B4C_INNER_RADIUS_CM`

Inner radius of the B4C ring \[cm\] — 60 mm diameter.

```rust
pub const B4C_INNER_RADIUS_CM: f64 = 3.0;
```

#### Constant `B4C_OUTER_RADIUS_CM`

Outer radius of the B4C ring \[cm\] — 105 mm diameter.

```rust
pub const B4C_OUTER_RADIUS_CM: f64 = 5.25;
```

#### Constant `N_CONTROL_RODS`

Number of control rods in the side reflector.

```rust
pub const N_CONTROL_RODS: usize = 10;
```

#### Constant `B4C_DENSITY_G_PER_CM3`

Density of boron carbide in the rod \[g/cm³\], stated by TECDOC.

```rust
pub const B4C_DENSITY_G_PER_CM3: f64 = 1.7;
```

#### Constant `AXIAL_SECTIONS_CM`

Axial section lengths of one rod \[cm\], lower to upper, as published:
`45/487/36/487/36/487/36/487/36/487/23` mm. The five 487 mm sections are
B4C; every other section is stainless steel.

```rust
pub const AXIAL_SECTIONS_CM: [f64; 11] = _;
```

#### Constant `AXIAL_IS_B4C`

Which entries of [`AXIAL_SECTIONS_CM`] are B4C rather than steel.

```rust
pub const AXIAL_IS_B4C: [bool; 11] = _;
```

#### Constant `LOWER_END_WITHDRAWN_CM`

Axial coordinate of the rod lower end when **fully withdrawn** \[cm\].

```rust
pub const LOWER_END_WITHDRAWN_CM: f64 = 119.2;
```

#### Constant `LOWER_END_INSERTED_CM`

Axial coordinate of the rod lower end when **fully inserted** \[cm\].

```rust
pub const LOWER_END_INSERTED_CM: f64 = 394.2;
```

## Module `materials`

**The HTR-10 material set, in one place.**

# Why this module exists

These eleven materials were assembled inline in
`examples/htr10_rmc_keff.rs`. That was fine while the example was the only
consumer, and stopped being fine the moment a second one appeared
(`examples/htr10_geometry_export.rs`, which writes the model specification
for the double-heterogeneity manuscript). Two copies of a material set is
exactly the drift this workspace forbids: the exported table would go on
describing the model the eigenvalue *used to* be computed with.

So the assembly lives here and both callers use it. The **environment
knobs stay in the example** — this function takes an explicit
[`Htr10MaterialConfig`] instead, so a caller that wants the default model
asks for the default and gets exactly what the benchmark runs.

# The indices are load-bearing

The returned `Vec` is indexed by [`super::core_model::mat`], and the
geometry refers to materials by that index. Reordering it silently
repoints every cell in the core at the wrong material, which is not a
failure that announces itself — `k_eff` simply comes out wrong. The
length is asserted against `mat::HOMOG_DUMMY + 1` for that reason.

# Provenance

Compositions: Li et al. (2014) Table 2 for the pebble (via
[`outram_mc_libs::pebble_beds::htr10::fuel_pebble_materials`]);
IAEA-TECDOC-1382 Table 4-3 for every reflector zone.

```rust
pub mod materials { /* ... */ }
```

### Types

#### Struct `Htr10MaterialConfig`

Everything the material set depends on, stated rather than read from the
environment — see the module docs.

```rust
pub struct Htr10MaterialConfig {
    pub temperature_k: f64,
    pub boron: outram_mc_libs::pebble_beds::htr10::BoronReading,
    pub reflector_zone: usize,
    pub reflector_carbon_scale: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature_k` | `f64` | Temperature \[K\] every material is built at. |
| `boron` | `outram_mc_libs::pebble_beds::htr10::BoronReading` | How the two "ppm natural boron" rows of Table 2 are read. |
| `reflector_zone` | `usize` | Which TECDOC Table 4-3 zone stands in for the whole reflector.<br>22 is the default and the OPTIMISTIC bound (cleanest graphite). |
| `reflector_carbon_scale` | `f64` | Multiplier on the reflector's carbon density. `1.0` is unmodified.<br>A sensitivity bound, not a model. |

##### Implementations

###### Methods

- ```rust
  pub fn benchmark_default(temperature_k: f64) -> Self { /* ... */ }
  ```
  The configuration the reported benchmark results are computed with.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10MaterialConfig { /* ... */ }
    ```

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

#### Function `htr10_material_set`

**Attributes:**

- `MustUse { reason: None }`

Build the eleven-material set, indexed by [`mat`].

# Panics

If `reflector_zone` is not listed in TECDOC Table 4-3, or if the assembled
length does not match the index table.

```rust
pub fn htr10_material_set(n: outram_mc_libs::pebble_beds::htr10::Htr10Nuclides, cfg: Htr10MaterialConfig) -> Vec<outram_mc_libs::material::material::Material> { /* ... */ }
```

#### Function `nuclide_name`

**Attributes:**

- `MustUse { reason: None }`

Human-readable name for each nuclide slot, for reporting.

[`Htr10Nuclides`] is a table of *indices into the caller's nuclide array*,
so a material component carries an index and nothing else. Anything that
reports a composition needs this to turn that index back into a name.

```rust
pub fn nuclide_name(n: outram_mc_libs::pebble_beds::htr10::Htr10Nuclides, idx: usize) -> &'static str { /* ... */ }
```

### Constants and Statics

#### Constant `B10_OF_NATURAL`

Natural boron is 19.9 at.% B-10; the other 80.1 at.% is B-11, a
non-absorber that is placed anyway (gh:#311) because the reference states
natural boron.

```rust
pub const B10_OF_NATURAL: f64 = 0.199;
```

## Module `table1`

Design characteristics from the paper's **Table 1**.

```rust
pub mod table1 { /* ... */ }
```

### Constants and Statics

#### Constant `THERMAL_POWER_MW`

Thermal power \[MW\].

```rust
pub const THERMAL_POWER_MW: f64 = 10.0;
```

#### Constant `CORE_HEIGHT_CM`

Mean core height \[cm\].

```rust
pub const CORE_HEIGHT_CM: f64 = 197.0;
```

#### Constant `CORE_DIAMETER_CM`

Core diameter \[cm\].

```rust
pub const CORE_DIAMETER_CM: f64 = 180.0;
```

#### Constant `FUEL_BALL_FRACTION`

Fuel-to-moderator ball ratio, as the paper writes it (`0.57/0.43`).

```rust
pub const FUEL_BALL_FRACTION: f64 = 0.57;
```

#### Constant `MODERATOR_BALL_FRACTION`

Moderator (graphite) ball fraction.

```rust
pub const MODERATOR_BALL_FRACTION: f64 = 0.43;
```

#### Constant `FUEL_ELEMENTS`

Total fuel elements in the core, from the paper's body text.

```rust
pub const FUEL_ELEMENTS: f64 = 27_000.0;
```

#### Constant `LAYER_HEIGHT_CM`

Fuel-ball loading step, i.e. one layer \[cm\] — the paper's own
quantisation, *"selected as the height of a layer ... in order to avoid
fractional fuel or moderator balls"*.

```rust
pub const LAYER_HEIGHT_CM: f64 = 9.798;
```

#### Constant `BALL_FILLING_FRACTION`

Ball filling fraction in the core region, from the paper's body text.

```rust
pub const BALL_FILLING_FRACTION: f64 = 0.61;
```

#### Constant `BALL_DIAMETER_CM`

Ball diameter \[cm\] (Table 2, both fuel and moderator).

```rust
pub const BALL_DIAMETER_CM: f64 = 6.0;
```

### Types

#### Struct `GeometryClosure`

One over-determined quantity in the paper: a value the paper **states**,
beside the value our reconstruction **derives** from other stated quantities.

A closure is only evidence if the derivation does not use the stated value —
otherwise it is a tautology. Each entry below says what it was derived from.

```rust
pub struct GeometryClosure {
    pub quantity: &'static str,
    pub stated: f64,
    pub derived: f64,
    pub units: &'static str,
    pub derived_from: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `quantity` | `&'static str` | What is being closed. |
| `stated` | `f64` | What the paper states. |
| `derived` | `f64` | What our reconstruction implies. |
| `units` | `&'static str` | Units, for reporting. |
| `derived_from` | `&'static str` | What the derivation used — so a reader can check it is independent of<br>`stated`. |

##### Implementations

###### Methods

- ```rust
  pub fn relative(self: &Self) -> f64 { /* ... */ }
  ```
  Relative difference `(derived - stated)/stated`, dimensionless.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GeometryClosure { /* ... */ }
    ```

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

#### Function `geometry_closures`

**Attributes:**

- `MustUse { reason: None }`

Every geometry closure the paper supports, computed from
[`bed::HexBedCell`] and [`TrisoSpec::HTR10_LI2014`].

```rust
pub fn geometry_closures() -> Vec<GeometryClosure> { /* ... */ }
```

#### Function `heavy_metal_per_ball`

**Attributes:**

- `MustUse { reason: None }`

Heavy metal per fuel ball \[g\], from Table 2's kernel count, radius, density
and enrichment — none of which is the stated 5 g.

```rust
pub fn heavy_metal_per_ball() -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `RMC_KEFF_VS_HEIGHT`

The paper's single RMC `k_eff` curve against fuel-loading height, Tables 3
and 4 (`(height_cm, k_eff)`).

One curve, not two — see the module docs on the duplicated column. The MCNP
columns are deliberately **not** carried here: they are a second code's
results on a third model, and mixing them in would invite a comparison that
is not ours to make.

```rust
pub const RMC_KEFF_VS_HEIGHT: &[(f64, f64)] = _;
```

## Module `mgxs`

# Multigroup cross sections (MGXS) condensed from a Monte Carlo run

This is the bridge from **stochastic transport to deterministic transport**:
it runs [`outram_mc_libs`] over a geometry, tallies flux-weighted reaction
rates on a user group structure, and divides them into macroscopic
multigroup cross sections that a deterministic solver — GeN-Foam's
diffusion / SP3 / SN neutronics — can consume.

It implements no physics. Every number here is a ratio of two Monte Carlo
tallies produced by `outram-mc-libs`, which is exactly the composition role
[`crate`] exists for.

## The definition being applied

For zone `z`, group `g`, reaction `x`, the flux-weighted group constant is

```text
  Sigma_x,g = ( integral over group g of Sigma_x(E) phi(E) dE ) / ( integral over group g of phi(E) dE )
```

The numerator is a track-length reaction-rate tally and the denominator a
track-length flux tally, both over the same spatial and energy filters, so
the ratio is the standard MC-condensed group constant. The scattering matrix
needs one extra axis:

```text
  Sigma_s,g->g' = ( scatter events from g into g' ) / ( integral over group g of phi(E) dE )
```

and that numerator is the analog estimator
[`outram_mc_libs::tally::scoring::score_scatter_matrix`], reachable only
because a tally can now carry an outgoing-energy filter.

## Two passes, one seed

A [`outram_mc_libs::tally::tally::Tally`] carries one filter set, and the
scattering matrix needs an outgoing-energy axis the scalar reaction rates
must not have — a scalar tally binned by outgoing energy would be wrong, and
the matrix tally deliberately refuses to score flux (see
`score_scatter_matrix`). So the generator makes **two** k-eigenvalue passes
over the same model at the **same seed**, which transport byte-identical
histories:

| pass | filters | scores |
|---|---|---|
| scalar | `[Material, Energy]` | `Flux, Total, Absorption, NuFission, KappaFission` |
| matrix | `[Material, Energy, EnergyOut]` | `ScatterN` |

Because the histories are identical, the flux denominator from the scalar
pass is the correct denominator for the matrix pass; they are not two
independent estimates that happen to be close.

## Group ordering — read this before indexing

Energy filter bins ascend in energy, so **group index 0 is the LOWEST
energy** here. Reactor convention is the opposite: group 0 is usually the
fastest. This module keeps the filter's own ascending order throughout,
because silently reversing it is how an MGXS set ends up transposed, and
offers [`MgxsLibrary::in_descending_energy`] for the conventional view.
Nothing is reversed implicitly.

## What this does NOT do

- **No Legendre scattering moments.** Only the isotropic `P0` matrix is
  tallied. GeN-Foam's `ZoneNuclearData` stores `scattering[moment][..][..]`
  and SP3 wants `P1`; that needs a scattering-cosine axis on the matrix
  tally and is not done here.
- **No delayed-neutron data.** `beta`, `lambda` and the delayed spectrum are
  not condensed; a deterministic transient needs them and they must come
  from elsewhere.
- **No feedback parametrisation.** `ZoneNuclearData` interpolates cross
  sections over reactor state (fuel temperature, density). One run gives one
  state point, the reference state.
- **No validation.** Condensing correctly is not the same as the group
  constants reproducing a reference eigenvalue. Nothing here is validated.

```rust
pub mod mgxs { /* ... */ }
```

### Types

#### Enum `MgxsError`

Errors from condensing a Monte Carlo run into group constants.

```rust
pub enum MgxsError {
    TooFewEdges {
        n: usize,
    },
    EdgesNotAscending {
        i: usize,
        j: usize,
        lo: f64,
        hi: f64,
    },
    BinCountMismatch {
        name: String,
        got: usize,
        want: usize,
        zones: usize,
        groups: usize,
        out_groups: usize,
    },
}
```

##### Variants

###### `TooFewEdges`

A group structure needs at least two edges to define one group.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `usize` | Number of edges supplied. |

###### `EdgesNotAscending`

Group edges must ascend strictly; a flat or descending pair makes the
bin mapping ambiguous.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `i` | `usize` | Index of the lower edge. |
| `j` | `usize` | Index of the upper edge. |
| `lo` | `f64` | Lower edge value \[eV\]. |
| `hi` | `f64` | Upper edge value \[eV\]. |

###### `BinCountMismatch`

A tally's bin count does not match the filter structure it was declared
with, so the flat index arithmetic would read the wrong bin.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Tally name. |
| `got` | `usize` | Bins actually present. |
| `want` | `usize` | Bins the filter structure implies. |
| `zones` | `usize` | Zone count used in the calculation. |
| `groups` | `usize` | Group count used in the calculation. |
| `out_groups` | `usize` | Outgoing-group count (1 for a scalar tally). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn from(source: MgxsError) -> Self { /* ... */ }
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

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GroupStructure`

An energy group structure, as ascending bin edges in eV.

`n + 1` edges define `n` groups, and **group 0 is the lowest-energy group**
— see the module note on ordering.

```rust
pub struct GroupStructure {
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
  pub fn new(edges: Vec<f64>) -> Result<Self, MgxsError> { /* ... */ }
  ```
  Build a structure from ascending edges in eV.

- ```rust
  pub fn two_group(split_ev: f64) -> Result<Self, MgxsError> { /* ... */ }
  ```
  The conventional **two-group** structure: thermal below `split_ev`,

- ```rust
  pub fn log_spaced(e_lo: f64, e_hi: f64, n: usize) -> Result<Self, MgxsError> { /* ... */ }
  ```
  `n` log-spaced groups between `e_lo` and `e_hi` \[eV\].

- ```rust
  pub fn n_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of groups (`edges.len() - 1`).

- ```rust
  pub fn edges(self: &Self) -> &[f64] { /* ... */ }
  ```
  The ascending edges in eV.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GroupStructure { /* ... */ }
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
#### Struct `ZoneMgxs`

One zone's condensed group constants.

Every vector is indexed by group in the structure's own **ascending-energy**
order. Cross sections are macroscopic, in cm^-1; `kappa_fission` is a power
density per unit flux (J cm^-1).

```rust
pub struct ZoneMgxs {
    pub name: String,
    pub flux: Vec<f64>,
    pub total: Vec<f64>,
    pub absorption: Vec<f64>,
    pub nu_fission: Vec<f64>,
    pub kappa_fission: Vec<f64>,
    pub scatter: Vec<Vec<f64>>,
    pub chi: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable zone name, carried through from the material. |
| `flux` | `Vec<f64>` | Group scalar flux \[arbitrary, per source particle\]. This is the<br>weighting denominator, kept because a zero here is what makes every<br>other entry in the group meaningless, and callers must be able to see it. |
| `total` | `Vec<f64>` | Total macroscopic cross section \[cm^-1\]. |
| `absorption` | `Vec<f64>` | Absorption (capture + fission) \[cm^-1\]. |
| `nu_fission` | `Vec<f64>` | Neutron production `nu * Sigma_f` \[cm^-1\]. |
| `kappa_fission` | `Vec<f64>` | Fission energy deposition `kappa * Sigma_f` \[J cm^-1\]. |
| `scatter` | `Vec<Vec<f64>>` | Isotropic (`P0`) scattering matrix, `scatter[g_in][g_out]` \[cm^-1\]. |
| `chi` | `Vec<f64>` | Fission spectrum `chi_g`, normalised to sum to 1 over groups.<br><br>**Measured, not assumed**: condensed from the energies fission neutrons<br>were actually born with during the run, via<br>`outram_mc_libs::tally::scoring::score_fission_birth`. A zone that<br>fissioned not at all carries all zeros, and the sum is then 0 rather<br>than 1 -- check before using it as a source distribution. |

##### Implementations

###### Methods

- ```rust
  pub fn n_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of groups.

- ```rust
  pub fn removal(self: &Self, g: usize) -> Option<f64> { /* ... */ }
  ```
  Removal cross section for group `g`: everything that takes a neutron out

- ```rust
  pub fn diffusion_length(self: &Self, g: usize) -> Option<f64> { /* ... */ }
  ```
  Diffusion length `L = sqrt(D / Sigma_a)` \[cm\] for group `g`.

- ```rust
  pub fn inferred_absorption(self: &Self, g: usize) -> Option<f64> { /* ... */ }
  ```
  Absorption as the **GeN-Foam bridge infers it**, `Sigma_t,g - sum_g'

- ```rust
  pub fn diffusion_coefficient(self: &Self, g: usize) -> Option<f64> { /* ... */ }
  ```
  Diffusion coefficient `D_g = 1 / (3 Sigma_tr,g)` \[cm\].

- ```rust
  pub fn scatter_out(self: &Self, g: usize) -> Option<f64> { /* ... */ }
  ```
  Scattering production out of group `g`, summed over outgoing groups.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ZoneMgxs { /* ... */ }
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
#### Struct `MgxsLibrary`

A full MGXS set: one group structure, one entry per zone.

```rust
pub struct MgxsLibrary {
    pub groups: GroupStructure,
    pub zones: Vec<ZoneMgxs>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `groups` | `GroupStructure` | The group structure every zone is condensed onto. |
| `zones` | `Vec<ZoneMgxs>` | Per-zone group constants, in the order the zones were supplied. |

##### Implementations

###### Methods

- ```rust
  pub fn homogenised</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>) -> MgxsLibrary { /* ... */ }
  ```
  Collapse every zone into ONE flux-weighted zone — the whole system as a

- ```rust
  pub fn homogenised_subset</* synthetic */ impl Into<String>: Into<String>>(self: &Self, which: &[usize], name: impl Into<String>) -> MgxsLibrary { /* ... */ }
  ```
  Homogenise only the zones named by `which`, leaving the rest out.

- ```rust
  pub fn with_buckling(self: &Self, b2: f64) -> MgxsLibrary { /* ... */ }
  ```
  Add a **measured** buckling leakage `D_g * B^2` to every group's

- ```rust
  pub fn balance_check(self: &Self) -> Vec<(String, Vec<(f64, f64, f64)>)> { /* ... */ }
  ```
  Per-group check that `Sigma_t` agrees with `Sigma_a + sum_g' Sigma_s,g->g'`.

- ```rust
  pub fn buckling_from_leakage(self: &Self, leak_fraction: f64) -> Option<f64> { /* ... */ }
  ```
  Solve for the buckling that reproduces a **measured** leakage fraction.

- ```rust
  pub fn k_inf(self: &Self) -> f64 { /* ... */ }
  ```
  Infinite-multiplication factor implied by these constants, zero leakage.

- ```rust
  pub fn rebalanced(self: &Self) -> Self { /* ... */ }
  ```
  The same library with every group's total rebalanced to

- ```rust
  pub fn reflector_savings(self: &Self, core_zone: usize, refl_zone: usize, thermal_group: usize, refl_thickness_cm: f64) -> Option<f64> { /* ... */ }
  ```
  Reflector savings `delta` \[cm\] from one-group diffusion theory.

- ```rust
  pub fn k_inf_inferred_absorption(self: &Self) -> f64 { /* ... */ }
  ```
  `k_inf` recomputed with the absorption the **bridge infers** rather than

- ```rust
  pub fn in_descending_energy(self: &Self) -> Self { /* ... */ }
  ```
  The same library with every group axis reversed, so index 0 is the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MgxsLibrary { /* ... */ }
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
### Functions

#### Function `condense`

Condense a completed pair of Monte Carlo tallies into group constants.

# Parameters
- `groups` — the structure both tallies were binned on.
- `zone_names` — one name per zone, in **material-index order**, matching
  the `MaterialFilter` the tallies carried.
- `scalar` — a tally with filters `[Material, Energy]` and scores exactly
  [`SCALAR_SCORES`], already run.
- `matrix` — a tally with filters `[Material, Energy, EnergyOut]` and score
  `[ScatterN]`, already run at the same seed.
- `n_realizations` — active batch count, for [`TallyBin::mean`].

# Errors

[`MgxsError::BinCountMismatch`] if either tally's bin count disagrees with
`zone_names.len()` and `groups.n_groups()`. That check exists because the
flat index arithmetic below would otherwise read a neighbouring zone's bin
and produce plausible nonsense.

# Zero-flux groups

A group a zone never saw has zero flux and no defined cross section. Rather
than emit a NaN or an infinity that would propagate into a solver, every
cross section in such a group is set to `0.0` and the zero flux is left
visible in [`ZoneMgxs::flux`] so a caller can detect it. This is a real
situation in a thermal reactor: a fast group in a deep reflector can go
unvisited at modest particle counts.

```rust
pub fn condense(groups: &GroupStructure, zone_names: &[String], scalar: &outram_mc_libs::tally::tally::Tally, matrix: &outram_mc_libs::tally::tally::Tally, n_realizations: u64) -> Result<MgxsLibrary, MgxsError> { /* ... */ }
```

#### Function `scalar_tally`

**Attributes:**

- `MustUse { reason: None }`

Build the empty scalar-pass tally for `n_zones` materials on `groups`.

The caller runs this through a k-eigenvalue driver and hands the result to
[`condense`]. Provided so the filter order and score order cannot drift from
what `condense` indexes.

```rust
pub fn scalar_tally(id: i32, groups: &GroupStructure, material_indices: Vec<usize>) -> outram_mc_libs::tally::tally::Tally { /* ... */ }
```

#### Function `matrix_tally`

**Attributes:**

- `MustUse { reason: None }`

Build the empty scattering-matrix tally for `n_zones` materials on `groups`.

Carries an outgoing-energy filter, which is what switches
`outram-mc-libs`'s analog scatter estimator on; see that crate's
`score_scatter_matrix`.

```rust
pub fn matrix_tally(id: i32, groups: &GroupStructure, material_indices: Vec<usize>) -> outram_mc_libs::tally::tally::Tally { /* ... */ }
```

### Constants and Statics

#### Constant `MATRIX_SCORES`

The score order the scalar pass must declare, and that [`condense`] assumes.

Exposed so a caller building the tally cannot silently disagree with the
reader about which score sits at which offset.
Scores carried by the matrix pass, in order: the scattering matrix and the
fission-birth spectrum. Both need the same `[Material, Energy, EnergyOut]`
filters, so they share one tally and one transport pass.

```rust
pub const MATRIX_SCORES: usize = 2;
```

#### Constant `SCALAR_SCORES`

```rust
pub const SCALAR_SCORES: [outram_mc_libs::tally::tally::ScoreType; 5] = _;
```

## Module `genfoam_xs`

# Handing Monte Carlo MGXS to the GeN-Foam deterministic solvers

[`crate::mgxs`] condenses a Monte Carlo run into group constants. This turns
that set into a [`NuclearDataInput`], the dictionary GeN-Foam's own
cross-section machinery reads, so the data enters through
[`CrossSectionData::from_input`] and is validated by the same code path a
hand-written `nuclearData` file would be. Nothing here reaches around that
machinery to poke fields into a solver directly.

## UNITS — the thing most likely to silently ruin a result

`outram-mc-libs` works in **centimetres**: cross sections in cm^-1, lengths
in cm. GeN-Foam, following OpenFOAM, works in **base SI**: cross sections in
m^-1, the diffusion coefficient in m, `sigma_pow` in J/m.

So every cross section is multiplied by [`CM_INV_TO_M_INV`] = 100 and every
length by [`CM_TO_M`] = 0.01. Getting this wrong does not crash anything --
it produces a reactor that is wrong by two orders of magnitude in optical
thickness, which looks like a physics problem rather than a units problem.
The conversion is therefore done in exactly one place, named, and tested.

## What is carried across, and what is supplied

| GeN-Foam field | Source |
|---|---|
| `d` | `1 / (3 Sigma_t)`, converted to m |
| `nu_sigma_eff` | tallied `nu Sigma_f` |
| `sigma_pow` | tallied `kappa Sigma_f` |
| `sigma_removal` | `Sigma_t - Sigma_s,g->g` |
| `chi_prompt` | tallied fission-birth spectrum |
| `chi_delayed` | **copied from `chi_prompt`** — see below |
| `scattering[0]` | tallied `P0` matrix, transposed into GeN-Foam's `[into][from]` |
| `iv` | `1/v` **derived** from each group's midpoint energy |
| `integral_flux` | the tallied group flux |
| `beta`, `lambda` | **zero / placeholder** — see below |

## Materials that are not in the geometry are skipped, not refused

A material with zero flux in **every** group was never reached — usually it
is not instantiated by this geometry at all. It is not a zone of the
deterministic model and is silently dropped. A material with flux in some
groups but not others is a different matter and is refused: that zone
exists, and its missing groups would be singular.

## Delayed neutrons are NOT carried, and that bounds what this can do

The Monte Carlo passes tally no delayed-neutron data, so `beta` is written
as **all zeros** and `chi_delayed` is copied from `chi_prompt`.

With `beta = 0` the delayed precursors carry no source, so the delayed
spectrum multiplies nothing and the copy is inert rather than a smuggled
assumption. That is correct for a **steady-state eigenvalue**, where `k_eff`
does not depend on the prompt/delayed split at all.

It is **wrong for a transient**. A kinetics solve driven from this data has
no delayed neutrons and would run prompt-critical nonsense. The transient
path must not be used until beta and lambda are tallied; nothing here
prevents a caller trying, so this is stated loudly rather than guarded.

## Scattering matrix orientation — NO transpose

[`crate::mgxs::ZoneMgxs::scatter`] is indexed `[g_from][g_into]`, and
GeN-Foam's `ZoneStateInput::scattering` documents `[moment][j][i]` as
`Sigma_{s, j -> i}`, which is the **same** orientation. Nothing is
transposed; only the units change.

This is spelled out because it was got wrong in the first version, and the
failure mode is worth knowing. Transposing fed the solver zero in-scatter,
so the softer group was populated by its fission-spectrum share alone: the
spectrum came out 12x too hard and `k_inf` dropped 4393 pcm. That reads as
a physics disagreement rather than an indexing bug, which is exactly why the
leakage-free cross-check below exists.

```rust
pub mod genfoam_xs { /* ... */ }
```

### Types

#### Enum `GenfoamXsError`

Errors from handing a Monte Carlo MGXS set to GeN-Foam.

```rust
pub enum GenfoamXsError {
    UnvisitedGroup {
        zone: String,
        group: usize,
        n_groups: usize,
    },
    NoZonesWithFlux,
}
```

##### Variants

###### `UnvisitedGroup`

A group carries no flux in some zone, so its cross sections were never
measured and [`crate::mgxs::condense`] left them at zero.

**This must not be passed to a diffusion solver.** A group with zero
removal and zero diffusion coefficient has the equation `0 * phi = 0`,
which is singular: the solver returns an arbitrary flux there, and that
spurious flux dilutes the real spectrum and moves the eigenvalue.
Measured on a bare HEU medium 2026-09-19, two empty thermal groups
absorbed 27.9% of the converged flux and pulled `k_inf` down by
4873 pcm.

The fix is a group structure the problem actually populates, or more
particles -- not a fabricated cross section for a group no neutron
visited.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `zone` | `String` | Name of the zone with the empty group. |
| `group` | `usize` | Index of the empty group, in the order supplied. |
| `n_groups` | `usize` | Total group count. |

###### `NoZonesWithFlux`

No material in the library carried any flux at all, so there is no
deterministic model to build. Usually means the transport never reached
the geometry, or the tallies were filtered to the wrong materials.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn from(source: GenfoamXsError) -> Self { /* ... */ }
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

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `neutron_speed_m_per_s`

**Attributes:**

- `MustUse { reason: None }`

Non-relativistic neutron speed \[m/s\] at energy `e_ev`.

`v = c * sqrt(2E / (m c^2))`. Non-relativistic is accurate to better than a
percent below ~10 MeV, which covers every group a reactor spectrum uses; at
20 MeV it overestimates by about 1.5%. Stated rather than hidden because
`1/v` feeds the time-derivative term of a transient, and this module does
not support transients anyway (see the module note on delayed neutrons).

```rust
pub fn neutron_speed_m_per_s(e_ev: f64) -> f64 { /* ... */ }
```

#### Function `to_nuclear_data_input`

Build the GeN-Foam `nuclearData` input from a Monte Carlo MGXS set.

The result has a single reactor state named `"reference"` with no feedback
variables, because one Monte Carlo run is one state point. Feeding it to
[`outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData::from_input`]
gives a `CrossSectionData` a diffusion or SP3 solve can use.

`library` must be in **descending-energy** group order — GeN-Foam's
convention, group 0 fastest. Call
[`MgxsLibrary::in_descending_energy`](crate::mgxs::MgxsLibrary::in_descending_energy)
first; this function cannot tell the two orders apart and will not guess.

# Errors

[`GenfoamXsError::UnvisitedGroup`] if any zone has a group with zero flux.
That is refused rather than passed on, because zeros make the group's
diffusion equation singular and the solver then invents a flux there — see
that variant's documentation for the measured consequence.

```rust
pub fn to_nuclear_data_input(library: &crate::mgxs::MgxsLibrary) -> Result<outram_foam_appbuilder_lib::genfoam::neutronics::xs::input::NuclearDataInput, GenfoamXsError> { /* ... */ }
```

### Constants and Statics

#### Constant `CM_INV_TO_M_INV`

Macroscopic cross sections: cm^-1 to m^-1.

```rust
pub const CM_INV_TO_M_INV: f64 = 100.0;
```

#### Constant `CM_TO_M`

Lengths: cm to m.

```rust
pub const CM_TO_M: f64 = 0.01;
```

## Module `det_six_factor`

Six-factor decomposition of a **deterministic** (diffusion / SP3) solve.

# What this is

[`outram_mc_libs::prelude::run_keff_reactor_physics`] produces a three-group,
leakage-resolved six-factor decomposition (η, f, p, ε, `P_FNL`, `P_TNL`) from
a Monte Carlo run. This module produces the **same decomposition** from a
deterministic solve, so the two can be compared term by term and mapped
against the same parameters.

# Why it calls the Monte Carlo crate's assembly instead of its own

`p` and `ε` are **not convention-free**. `outram-mc-libs`' own module docs
record a case where two differently-defined `p` values looked 8.8 % apart
while their *product* differed by 2.2 % — the fingerprint of a definitional
mismatch being read as physics. Writing a second set of formulas here
against the same six names would rebuild exactly that trap.

So this module does **no** six-factor algebra. It forms the group-resolved
reaction rates from the deterministic flux and the multigroup cross
sections, collapses them onto the same three bands, and hands them to
[`outram_mc_libs::prelude::assemble_six_factors`]. One definition, two
solvers.

# Units and normalisation

The six factors are **ratios**, so any consistent flux normalisation gives
the same answer — a deterministic eigenvalue solve fixes the flux only up to
a constant, and that is sufficient here. Rates are
`sum_cells Sigma_x,g,zone(cell) * phi_g,cell * V_cell`, in whatever units
the flux carries.

A deterministic solve has no statistical uncertainty, so every
[`Estimate`] carries `std = 0` and the propagated `1σ` fields are zero.
**That is not a claim of accuracy** — it records that the *sampling* error
is zero, while the discretisation and condensation errors, which are the
large ones here, are not represented at all.

```rust
pub mod det_six_factor { /* ... */ }
```

### Types

#### Struct `BandRates`

Group-resolved reaction rates collapsed onto the three six-factor bands.

Public because a caller mapping a parameter sweep usually wants the rates
as well as the factors — the rates are what make a change in `p` or `f`
attributable to a band rather than merely observed.

```rust
pub struct BandRates {
    pub absorption: [f64; 3],
    pub production: [f64; 3],
    pub thermal_absorption_fuel: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `absorption` | `[f64; 3]` | Absorption rate per band `[thermal, resonance, fast]`. |
| `production` | `[f64; 3]` | `nu`-fission production rate per band. |
| `thermal_absorption_fuel` | `f64` | Thermal absorption restricted to the fuel zones. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BandRates { /* ... */ }
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
    fn default() -> BandRates { /* ... */ }
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

#### Function `band_rates`

**Attributes:**

- `MustUse { reason: None }`

Collapse a deterministic solve's group fluxes into three-band reaction rates.

# Arguments

- `lib` — the multigroup library **in the same group ordering as
  `group_flux`**. Pass the ascending-energy library with an ascending flux,
  or the descending one with a descending flux; this function reads the
  group edges from `lib` and does not reorder anything.
- `zone_of_cell` — zone index per mesh cell, as handed to the solver.
- `cell_volume` — per-cell volume. A uniform mesh may pass all ones; the
  factors are ratios and a constant cancels.
- `group_flux` — `[group][cell]` scalar flux from the converged solve.
- `fuel_zone` — per-zone flag: does this zone contain fuel? Drives `f` and
  `η`, and getting it wrong silently changes both.
- `thermal_cutoff_ev`, `resonance_upper_ev` — the two band edges.

Groups whose index exceeds `lib`'s group count, and cells whose zone is out
of range, are skipped rather than panicking: a truncated or mismatched solve
should yield visibly wrong rates, not abort a parameter sweep mid-map.

```rust
pub fn band_rates(lib: &crate::mgxs::MgxsLibrary, zone_of_cell: &[usize], cell_volume: &[f64], group_flux: &[Vec<f64>], fuel_zone: &[bool], thermal_cutoff_ev: f64, resonance_upper_ev: f64) -> BandRates { /* ... */ }
```

#### Function `six_factors_from_deterministic`

**Attributes:**

- `MustUse { reason: None }`

The six factors of a deterministic solve, using
[`outram_mc_libs::prelude::assemble_six_factors`] — the same assembly the
Monte Carlo path uses.

`leakage_by_group` is `[thermal, resonance, fast]` in the same units as the
rates. **A zero-leakage (reflective) solve must pass `[0.0; 3]`**, which
makes `P_FNL = P_TNL = 1` exactly; passing a leakage a reflective solve did
not have is the single easiest way to make this disagree with a `k_inf`
reference for a reason that has nothing to do with the physics.

```rust
pub fn six_factors_from_deterministic(rates: &BandRates, leakage_by_group: [f64; 3], thermal_cutoff_ev: f64, resonance_upper_ev: f64) -> outram_mc_libs::prelude::SixFactors { /* ... */ }
```

## Module `rod_insertion`

Apply a smeared control-rod absorber to a multigroup library.

# What this does

[`crate::htr10_rmc::control_rod`] gives the **atom densities** a smeared rod
adds to the side-reflector boring band. This module turns those into a
**macroscopic absorption cross section per energy group** and adds it to a
zone of an [`MgxsLibrary`], so a deterministic solve can be run at any rod
position without a new Monte Carlo run.

That is the whole point: a Monte Carlo point costs 1-2 hours, so a rod
position sweep dense enough to fit a surrogate to is not reachable through
MC. The rod is a pure absorber addition, which is the one axis that *can*
be applied to already-condensed constants honestly.

# The 1/v approximation, and why it is defensible here

B-10's `(n, alpha)` absorption is the textbook `1/v` absorber from thermal
energies up to roughly 100 keV. Rather than assume a group-average, this
module **integrates** `sigma(E) = sigma_0 sqrt(E_0/E)` over each group
against a `1/E` (constant-lethargy) weighting spectrum, which is the right
weight for the epithermal range where most of the groups sit:

```text
<sigma>_g = sigma_0 sqrt(E_0) * 2 (E_lo^-1/2 - E_hi^-1/2) / ln(E_hi / E_lo)
```

**Where this is wrong, stated up front:**

- The thermal group's true weight is Maxwellian, not `1/E`. For a `1/v`
  absorber the Maxwellian average carries the familiar `sqrt(pi)/2` factor
  against the `2200 m/s` value; this module does not apply it, so the
  thermal group is **over-estimated by about 13 %**.
- B-10 departs from `1/v` above ~100 keV, where this will be wrong — but
  its absorption there is negligible against the resonance and thermal
  contributions, so the error in `k` is small.
- **Self-shielding is absent.** This is the large one, and it compounds the
  smearing error already documented in [`crate::htr10_rmc::control_rod`]:
  a real B4C annulus is black to thermal neutrons and shields its own
  interior, while a smeared dilute absorber does not. Both errors push the
  same way, so rod worth from this path is expected to be **over-predicted**.

Treat results from this module as an *uncalibrated hierarchical surrogate*
in the sense of the workspace model-hierarchy rule: derived from geometry
and published densities with nothing tuned, and to be reported with its
disagreement rather than corrected into agreement.

```rust
pub mod rod_insertion { /* ... */ }
```

### Functions

#### Function `one_over_v_group_average`

**Attributes:**

- `MustUse { reason: None }`

Group-averaged `1/v` cross section over `[e_lo, e_hi]` eV against a `1/E`
weighting spectrum \[barn\].

Returns `sigma_0` evaluated at the group midpoint if the group is
degenerate (`e_hi <= e_lo`), rather than dividing by a zero logarithm.

```rust
pub fn one_over_v_group_average(sigma_2200: f64, e_lo: f64, e_hi: f64) -> f64 { /* ... */ }
```

#### Function `rod_absorption_per_group`

**Attributes:**

- `MustUse { reason: None }`

Macroscopic absorption added per group by a rod insertion \[cm^-1\].

`group_edges` must be ascending in eV and have `n_groups + 1` entries —
i.e. the ordering [`MgxsLibrary::groups`] stores, **not** the descending
order the GeN-Foam bridge wants.

```rust
pub fn rod_absorption_per_group(fraction_inserted: f64, group_edges: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `with_rod_inserted`

**Attributes:**

- `MustUse { reason: None }`

A copy of `lib` with the rod absorber added to zone `zone_idx`.

Both `absorption` **and** `total` are increased by the same amount, because
an absorber removes neutrons from the group: raising absorption alone would
leave the balance `Sigma_t = Sigma_a + Sigma_s,row` broken, and
[`crate::genfoam_xs`] infers the solver's absorption from exactly that
difference — so the rod would have had no effect at all on the eigenvalue.
See `op-q6yy` for the measured consequence of getting that wrong.

Returns `lib` unchanged if `zone_idx` is out of range, so a mis-indexed
sweep produces a visibly flat curve rather than a panic mid-map.

```rust
pub fn with_rod_inserted(lib: &crate::mgxs::MgxsLibrary, zone_idx: usize, fraction_inserted: f64) -> crate::mgxs::MgxsLibrary { /* ... */ }
```

### Constants and Statics

#### Constant `SIGMA_A_B10_2200`

B-10 `(n, alpha)` cross section at 2200 m/s \[barn\].

The standard thermal reference value. Used with the `1/v` law below; no
resonance structure is represented because B-10 has none of consequence.

```rust
pub const SIGMA_A_B10_2200: f64 = 3837.0;
```

#### Constant `E_2200`

Energy of a 2200 m/s neutron \[eV\].

```rust
pub const E_2200: f64 = 0.0253;
```

#### Constant `B10_ABUNDANCE`

Natural abundance of B-10 in boron \[atom fraction\].

```rust
pub const B10_ABUNDANCE: f64 = 0.199;
```

#### Constant `SIGMA_A_B11_2200`

B-11 absorption is ~5 mb thermal — four orders below B-10's, and it is
carried explicitly rather than silently dropped so a reader can see it was
considered.

```rust
pub const SIGMA_A_B11_2200: f64 = 0.005;
```

## Module `coupling`

# Coupling Monte Carlo transport to the GeN-Foam deterministic solvers

One type, [`McToGenFoam`], carries a reactor model from a Monte Carlo
transport run to a deterministic solve **on the same geometry and the same
compositions**, so the two ends cannot quietly describe different reactors.

## The shortest thing that works

```no_run
use nee_soon::coupling::McToGenFoam;
use nee_soon::mgxs::GroupStructure;
# use outram_mc_libs::geometry::geometry::Geometry;
# use outram_mc_libs::material::material::Material;
# use outram_mc_libs::material::nuclide::Nuclide;
# use outram_mc_libs::physics::transport_csg::SourceBox;
# fn demo(geometry: Geometry, materials: Vec<Material>, nuclides: Vec<Nuclide>, source: SourceBox)
# -> Result<(), Box<dyn std::error::Error>> {
let coupling = McToGenFoam::new(geometry, materials, nuclides, source)
    .with_groups(GroupStructure::two_group(2.38)?)
    .with_particles(2_000);

let mgxs = coupling.generate_mgxs()?;          // Monte Carlo -> group constants
println!("Monte Carlo k = {:.5}", mgxs.k_eff);

let k_det = coupling.solve_infinite_medium(&mgxs.library)?;
println!("GeN-Foam k_inf = {k_det:.5}");
# Ok(())
# }
```

Three calls: construct, [`generate_mgxs`](McToGenFoam::generate_mgxs),
[`solve_infinite_medium`](McToGenFoam::solve_infinite_medium). The builder
setters all have defaults, so none of them is required.

## What each stage costs you

[`generate_mgxs`](McToGenFoam::generate_mgxs) runs the transport **twice**
at one seed — once for scalar reaction rates, once for the scattering matrix
and fission spectrum, which need an outgoing-energy axis the scalar rates
must not have. Budget accordingly: it is twice the cost of a plain
k-eigenvalue run.

## Honest limits, stated once

- **Steady state only.** No delayed-neutron data is tallied, so a transient
  driven from these constants would have no delayed neutrons. See
  [`crate::genfoam_xs`].
- **`P0` scattering only**, so the diffusion coefficient is the `1/(3 Sigma_t)`
  approximation rather than transport-corrected.
- **One state point.** One Monte Carlo run is one temperature and one
  density; there is no feedback parametrisation.
- [`solve_infinite_medium`](McToGenFoam::solve_infinite_medium) is a
  zero-leakage collapse. It is the right check that the group constants are
  self-consistent, and it is **not** the reactor's eigenvalue whenever
  leakage matters — which for a real core it does.

```rust
pub mod coupling { /* ... */ }
```

### Types

#### Enum `CouplingError`

Anything that can go wrong carrying a model from Monte Carlo to GeN-Foam.

Each variant names which stage failed and what to do about it, because the
stages have genuinely different remedies: a condensation failure is a tally
setup problem, an unvisited group is a group-structure problem, and a solver
failure is a numerics problem.

```rust
pub enum CouplingError {
    Condensation(crate::mgxs::MgxsError),
    Bridge(crate::genfoam_xs::GenfoamXsError),
    GenfoamXs(String),
    Solve(String),
}
```

##### Variants

###### `Condensation`

The Monte Carlo tallies could not be condensed into group constants.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mgxs::MgxsError` |  |

###### `Bridge`

The group constants were rejected on the way to GeN-Foam.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::genfoam_xs::GenfoamXsError` |  |

###### `GenfoamXs`

GeN-Foam rejected the `nuclearData` dictionary.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Solve`

The deterministic solve did not produce an eigenvalue.

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
    fn from(source: MgxsError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: GenfoamXsError) -> Self { /* ... */ }
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

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `MgxsRun`

What a Monte Carlo MGXS pass produced.

```rust
pub struct MgxsRun {
    pub library: crate::mgxs::MgxsLibrary,
    pub k_eff: f64,
    pub k_std: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `library` | `crate::mgxs::MgxsLibrary` | The condensed group constants, in the group structure's ascending-energy<br>order. Call<br>[`in_descending_energy`](MgxsLibrary::in_descending_energy) for the<br>reactor convention. |
| `k_eff` | `f64` | The Monte Carlo eigenvalue from the same run — the reference a<br>deterministic solve on these constants should reproduce where the<br>comparison is fair. |
| `k_std` | `f64` | Its one-sigma statistical uncertainty. Quote it: a deterministic result<br>agreeing to better than this is agreeing with noise. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MgxsRun { /* ... */ }
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
#### Struct `McToGenFoam`

Carries one reactor model from Monte Carlo transport to a GeN-Foam
deterministic solve.

Construct with [`new`](Self::new), adjust with the `with_*` setters (all
optional), then call [`generate_mgxs`](Self::generate_mgxs).

```rust
pub struct McToGenFoam {
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
  pub fn new(geometry: Geometry, materials: Vec<Material>, nuclides: Vec<Nuclide>, source: SourceBox) -> Self { /* ... */ }
  ```
  A coupling over `geometry`, with `materials` indexed as the geometry's

- ```rust
  pub fn with_groups(self: Self, groups: GroupStructure) -> Self { /* ... */ }
  ```
  Use `groups` as the energy group structure.

- ```rust
  pub fn with_particles(self: Self, n: usize) -> Self { /* ... */ }
  ```
  Transport `n` particles per batch.

- ```rust
  pub fn with_batches(self: Self, inactive: usize, active: usize) -> Self { /* ... */ }
  ```
  Use `inactive` source-convergence batches and `active` scoring batches.

- ```rust
  pub fn with_seed(self: Self, seed: u64) -> Self { /* ... */ }
  ```
  Fix the random seed. Both MGXS passes use it, which is what makes the

- ```rust
  pub fn with_majorants(self: Self, majorants: Vec<Majorant>) -> Self { /* ... */ }
  ```
  Supply delta-tracking majorants, for a model with delta-tracked regions

- ```rust
  pub fn groups(self: &Self) -> &GroupStructure { /* ... */ }
  ```
  The group structure in use.

- ```rust
  pub fn generate_mgxs(self: &Self) -> Result<MgxsRun, CouplingError> { /* ... */ }
  ```
  Run the Monte Carlo passes and condense them into group constants.

- ```rust
  pub fn solve_infinite_medium(self: &Self, library: &MgxsLibrary) -> Result<f64, CouplingError> { /* ... */ }
  ```
  Solve `k_inf` with GeN-Foam's multigroup diffusion on zone 0 of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
## Module `direct_coupling`

# Direct coupling: Monte Carlo transport against GeN-Foam thermal-hydraulics

The other coupling in this crate, [`crate::coupling`], goes *through*
multigroup cross sections: Monte Carlo condenses them, a deterministic
solver consumes them. This one does not. Here `outram-mc` **is** the
neutronics, iterated directly against GeN-Foam's thermal model until the two
agree on a temperature.

## The loop

```text
  repeat:
    Monte Carlo transport at the current fuel temperature  -> k_eff, power shape
    write powerDensityNeutronics into the coupling region
    GeN-Foam LumpedThermal.correct()                       -> new TFuel
    feed TFuel back as the transport temperature
  until |dT| and |dk| are both below tolerance
```

This is a Picard (fixed-point) iteration, the same scheme GeN-Foam's own
`multiPhysicsSolver` outer loop uses, and the thermal half is literally
GeN-Foam's [`LumpedThermal`] driven through its
[`RegionKernel`] contract and its [`CouplingRegion`] field exchange. The
thermal physics is not reimplemented here.

## The Doppler feedback is GLOBAL, and that is a real limitation

`outram-mc` takes **one** temperature for the whole problem
(`KeffSettings::temperature_k`); it is not per-material. So the feedback
this loop applies is a single fuel temperature applied everywhere, which is
coherent with a **lumped** thermal model of one node and would be wrong for
a spatially resolved one. A mesh-resolved coupling needs per-cell
temperature in the transport, which the transport does not yet accept.
Stated here rather than discovered later.

## Power normalisation

Monte Carlo returns reaction rates per source particle, not watts. The
caller therefore states the reactor's thermal power, and that is what drives
the thermal model; the transport supplies `k_eff` and the temperature
feedback. This is honest for a lumped node — there is only one power density
to distribute — and would need the tallied `KappaFission` shape the moment
more than one thermal node exists.

```rust
pub mod direct_coupling { /* ... */ }
```

### Types

#### Enum `DirectCouplingError`

Why a coupled solve stopped without converging.

```rust
pub enum DirectCouplingError {
    NotConverged {
        iterations: usize,
        last_dt_k: f64,
        tol_k: f64,
        last_dk: f64,
        tol_k_eff: f64,
    },
    Thermal(String),
}
```

##### Variants

###### `NotConverged`

The outer loop hit its iteration cap.

Carries the last residuals so the caller can see whether it was close or
diverging — those need opposite responses, and a bare "did not converge"
cannot distinguish them.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations performed. |
| `last_dt_k` | `f64` | Last temperature change \[K\]. |
| `tol_k` | `f64` | Temperature tolerance \[K\]. |
| `last_dk` | `f64` | Last eigenvalue change. |
| `tol_k_eff` | `f64` | Eigenvalue tolerance. |

###### `Thermal`

GeN-Foam's thermal region failed to advance.

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
#### Struct `Convergence`

Convergence criteria for the outer Picard loop.

```rust
pub struct Convergence {
    pub temperature_k: f64,
    pub k_eff: f64,
    pub max_iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature_k` | `f64` | Stop when the fuel temperature moves less than this between outer<br>iterations \[K\]. |
| `k_eff` | `f64` | Stop when `k_eff` moves less than this between outer iterations.<br><br>Setting this below the Monte Carlo statistical uncertainty is asking the<br>loop to converge on noise; it will burn iterations and stop on chance. |
| `max_iterations` | `usize` | Maximum outer iterations before giving up. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Convergence { /* ... */ }
    ```

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
    1 K and 1e-3 in `k`, capped at 20 outer iterations.

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
#### Struct `CoupledIteration`

One outer iteration's state, kept so a caller can see the approach rather
than only the answer.

```rust
pub struct CoupledIteration {
    pub iteration: usize,
    pub temperature_k: f64,
    pub k_eff: f64,
    pub k_std: f64,
    pub temperature_after_k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `iteration` | `usize` | 1-based iteration number. |
| `temperature_k` | `f64` | Fuel temperature the transport ran at \[K\]. |
| `k_eff` | `f64` | Eigenvalue at that temperature. |
| `k_std` | `f64` | Its one-sigma statistical uncertainty. |
| `temperature_after_k` | `f64` | Fuel temperature after the thermal solve \[K\]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoupledIteration { /* ... */ }
    ```

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
#### Struct `CoupledSteadyState`

A converged coupled steady state.

```rust
pub struct CoupledSteadyState {
    pub temperature_k: f64,
    pub k_eff: f64,
    pub k_std: f64,
    pub history: Vec<CoupledIteration>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature_k` | `f64` | Equilibrium fuel temperature \[K\]. |
| `k_eff` | `f64` | Eigenvalue at equilibrium. |
| `k_std` | `f64` | Its one-sigma statistical uncertainty. |
| `history` | `Vec<CoupledIteration>` | Every outer iteration, in order. |

##### Implementations

###### Methods

- ```rust
  pub fn feedback_pcm_per_k(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Temperature reactivity feedback across the whole solve, in pcm/K.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoupledSteadyState { /* ... */ }
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
#### Struct `McGenFoamDirect`

Monte Carlo neutronics iterated directly against GeN-Foam thermal-hydraulics.

Build with [`new`](Self::new), then [`solve_steady_state`](Self::solve_steady_state).

```rust
pub struct McGenFoamDirect {
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
  pub fn new(geometry: Geometry, materials: Vec<Material>, nuclides: Vec<Nuclide>, source: SourceBox, thermal: LumpedThermal, power: Power, region_volume_m3: f64) -> Self { /* ... */ }
  ```
  A direct coupling of `geometry` to a GeN-Foam lumped thermal region.

- ```rust
  pub fn with_particles(self: Self, n: usize) -> Self { /* ... */ }
  ```
  Transport `n` particles per batch.

- ```rust
  pub fn with_batches(self: Self, inactive: usize, active: usize) -> Self { /* ... */ }
  ```
  Use `inactive` source-convergence batches and `active` scoring batches.

- ```rust
  pub fn with_majorants(self: Self, majorants: Vec<Majorant>) -> Self { /* ... */ }
  ```
  Supply delta-tracking majorants for a delta-tracked model.

- ```rust
  pub fn with_time_step(self: Self, dt: Time) -> Self { /* ... */ }
  ```
  The pseudo-time step handed to the thermal model each outer iteration.

- ```rust
  pub fn solve_steady_state(self: &mut Self, criteria: Convergence) -> Result<CoupledSteadyState, DirectCouplingError> { /* ... */ }
  ```
  Iterate transport and thermal-hydraulics to a converged steady state.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

