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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **DistributionExt**
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

- **Imply**
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **DistributionExt**
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

- **Imply**
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
`0454c1ad1b` (one-ball bed, superseded -- see CURRENT NUMBERS below), 10 000 x [5 + 135], one seed per height, the residual is
`-896 +/- 30` pcm on ENDF/B-VIII.0 and `+288 +/- 31` pcm on ENDF/B-VII.0
(the reference's library), and **drifts `+7` pcm/cm with loading height in
every arm** (gh:#218, results posted there). Treat those numbers as tentative
until the items below are priced or fixed. Each is an issue; none has been
measured unless it says so.

**CURRENT NUMBERS (2026-09-25, two-ball cell, ENDF/B-VIII.0 + 5 thermal
laws, 10 000 x [5 + 135], 14 rings, 3 seeds each):** residual against RMC
`+1626 +/- 50` pcm at 97.98 cm, `+1646 +/- 62` at 122.47 cm, `+1958 +/- 33`
at 200.86 cm; slope `+3.41 +/- 0.54` pcm/cm. The model moved from BELOW
RMC to ABOVE it (+2231 to +2413 pcm against the one-ball model). **Nothing
was adjusted towards the reference; the residual is an open question**,
and the simplifications listed below that push `k` up are the first
candidates to investigate -- not to tune. Every earlier number in this
section predates the two-ball cell. Methodology, the twelve-height curve
and the sampling evidence: the V&V record
(`crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`,
"The two-ball prism cell").

**SUPERSEDED as "current" 2026-09-26:** those residuals predate the
explicit reflector (PR #327). On it (fast single-seed runs, 2000 x
[30 + 70], rod-steel Ni -> Fe and Fe-57 -> Fe-56 stated as assumptions),
the residual at the critical loading is ~~`-2365` pcm on ENDF/B-VIII.0 and
`-922` pcm on VII.0~~ **`-2726` pcm on ENDF/B-VIII.0 and `-1283` pcm on
VII.0** (CORRECTED 2026-09-27, gh:#333: matched on the paper's whole-ball
height; ~~`n_axial = 25` IS the paper's 123.576 cm loading~~), roughly -4000 pcm
from the numbers above; the drift is still there (~~`+10.2`~~ `+13.2 +/- 4.0`
pcm/cm on VIII.0). ~~Every "height-matched" residual in this section (and in
#218) used the volume-equivalent height and is low by 165-480 pcm.~~
**CORRECTED 2026-10-01 from Şeker & Çolak (2003) Table 3 (gh:#333):** the
reference rows hold `1346 N + 733` balls, which at 0.61 is **H − 0.55 cm**
of volume-equivalent bed (0.58 cm at N = 9, 0.48 cm at N = 20), not
H − 6 cm. `n_axial = 2N + 1` holds `(2N+1) x 4.899` cm, which is
**0.52–0.63 cm (≈ 60–75 balls) less** than the row it was matched to. So:
- the whole-ball-height residuals above are **low**;
- the earlier volume-equivalent ones were **high**;
- in both cases by roughly **70–250 pcm**. That magnitude is estimated from
  the reference's own slope, not measured, and it assumes Li's RMC model
  has Şeker's inventory.

The correct match is by **ball count**. Record:
`crates/outram-mc-libs/verification_and_validation/htr10_rmc/fast_ablation_2026_09_26.md`.

**Note 2026-10-01 (gh:#428).** Every residual in this section was computed
on a bed that is now **withdrawn**: the one-ball `core_model::assemble`,
then the two-ball `bed::TwoBallBed`. Both cut pebbles, and both now panic
if asked for. The default bed is Şeker & Çolak (2003)'s 13-ball cell
(`bed::SekerBed`, gh:#472), and it is compared to RMC at equal ball count
([`rmc_keff_at_ball_count`]). None of the numbers above was re-measured on
it for this note, so none of them describes the current default.

**Model defects, production path (`assemble_explicit_triso`):**
- ~~gh:#309 — one ball per hex tile clips the pebble shell: 4.76 % of all
  core carbon is missing while the heavy metal is exact (C/U low). Sign on
  `k` not predicted.~~ **FIXED 2026-09-25** by the paper's two-ball prism
  cell (`bed::TwoBallBed`): whole 6 cm pebbles, sampled filling fraction
  0.6096-0.6097, graphite restored (envelope graphite 0.4990 -> 0.5244 at
  122.47 cm), kernel fraction 0.998-1.000 of the paper-implied value.
  Worth `+2231` to `+2413` pcm (3-seed means, 98-201 cm): **k goes UP**.
  **SUPERSEDED 2026-10-01 (gh:#472):** the two-ball cell still cut the
  balls crossing the side wall and the bed top, and it is withdrawn
  (`OUTRAM_HTR10_TWO_BALL_CELL` now panics in `assemble_explicit_triso`).
  The default is `bed::SekerBed`, in which every ball is whole and is
  rejected at the side wall, the cone and the tube.
- ~~gh:#310 — the lattice drops the A-B layer offset, so axially adjacent
  pebbles touch and their fuel zones meet~~ **FIXED 2026-09-25**, same
  change: A-B stacking restored, minimum centre distance of the BUILT bed
  6.2102 cm (`tests::no_two_balls_of_the_built_bed_overlap`).
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
- gh:#218 — the `+7` pcm/cm drift itself. ~~**Cause now evidenced
  (2026-09-25):** a shrunk-pebble ablation with no #309 clip and no #310
  axial contact (all volume fractions the paper's) changes k by
  `-6.88 +/- 1.48` pcm/cm across 98-201 cm, equal and opposite to the
  drift.~~ **CORRECTED 2026-09-25 -- NOT evidenced.** The physical fix (the
  two-ball cell, real 6 cm pebble) changes the slope by only
  `-1.63 +/- 0.94` pcm/cm (1.7 sigma, unresolved) at the same three
  heights; the residual still drifts `+3.41 +/- 0.54` pcm/cm (develop:
  `+5.04 +/- 0.77`, its per-point sems taken from the pooled seed sd).
  The two arms' slope changes differ by `-5.3 +/- 1.8` pcm/cm (3 sigma), so
  most of the ablation's `-6.88` came from what else differed in it --
  chiefly its 18 % smaller pebble -- not from the clip or the contact
  (an inference, not a separate measurement). The drift remains open. Already ruled out: data library, source convergence, cavity,
  bottom-reflector mirroring, UO2 law source, B-11, TRISO count; the
  pebble construction accounts for at most about a third of it. Open: the
  reflector (explicit channels, PR #327), the height convention of the
  reference (gh:#333).

**Documented simplifications (not defects, each pushes `k` one way):**
- ~~every reflector region is TECDOC zone 22, the densest graphite in
  Table 4-3, and the boronated zones are not placed — raises `k`;~~
- ~~the control-rod boring band is solid zone-22 graphite
  (`OUTRAM_HTR10_BORINGS` is off: its core-height composition is unrecorded);~~
- ~~the core-height reflector zone map is not placed;~~
- rods fully withdrawn (the benchmark's B1 state); one temperature
  (300.15 K) everywhere.

**CHANGED 2026-09-25 (WIP, branch `claude/htr10-reflector`, NOT yet
priced):** the reflector is explicit 3-D geometry
([`reflector_geometry`]): the 20 coolant, 10 control-rod, 3 irradiation
and 7 absorber-ball channels at their own positions in solid graphite,
the hot gas duct, and every IAEA-TECDOC-1382 Fig. 4.10 zone with the
p. 242 corrections for explicit borings. The rods sit in their channels
at the withdrawn position with explicit B4C, steel and iron. The
discharge tube holds explicit whole graphite balls, with Li (2014)'s
rejection of balls crossing the cone or tube. What the specification does
not give (channel azimuths, the contents of the absorber-ball and
irradiation channels, the internal structure of zones 0-4, 8-16, 19-21,
48, 57) is listed in [`reflector_geometry`]'s module docs, and was
settled by the maintainer on 2026-09-27 (gh:#330: 18 degree convention,
KLAK and irradiation channels empty; gh:#332: those zones as the TECDOC
gives them). ~~**No `k` has been computed on this geometry yet, and it has
not yet been drawn**~~ **CORRECTED 2026-09-27:** it has been drawn
(`verification_and_validation/htr10_python_plots/`) and first priced at
fast statistics
(`outram-mc-libs/verification_and_validation/htr10_rmc/fast_ablation_2026_09_26.md`).
It is still an AI-drafted model awaiting human review.

**Other paths and plumbing:**
- gh:#308 — `assemble` (homogenised fuel) lacks the cavity, conus, bricks and
  annulus of the production path; do not use it for a `k` comparison. ~~It
  feeds `htr10_mgxs_genfoam`.~~ **CORRECTED 2026-10-01:** `assemble` is
  withdrawn and panics on entry (it cuts pebbles). `htr10_mgxs_genfoam`
  still calls it (checked: `examples/htr10_mgxs_genfoam.rs`), so that
  example now panics too.
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

## ~~NOT verifiable now~~ — the k-eff curve

**CORRECTED 2026-09-27:** the blocker described below is gone. The
TECDOC-1382 reflector is modelled (Table 4-3 zones, then every boring
explicit, draft PR #327), and the curve is computed against RMC at the
paper's own heights (gh:#333). See the "CURRENT NUMBERS" and the
SUPERSEDED note below and
`crates/outram-mc-libs/verification_and_validation/htr10_rmc/`. The
original text follows, unchanged, for the record.

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

- the **RMC** column is byte-identical in ~~**11 of 11**~~ **12 of 12**
  shared rows;
- the **MCNP** column ~~differs in **0 of 11** — that is, in every row~~
  **differs in 12 of 12** rows.

**CORRECTED 2026-10-01 (gh:#428):** Tables 3 and 4 have twelve rows, not
eleven (`the_reference_curve_is_monotonic_and_brackets_criticality` asserts
12), and "0 of 11" inverted the count. Checked against the stored
[`MCNP_TABLE3_KEFF_VS_HEIGHT`] and [`MCNP_TABLE4_KEFF_VS_HEIGHT`]: no row
is equal.

So the paper reports **one** RMC dataset against **two** MCNP results, and
one of the two captions is wrong. Consequence for anyone verifying against
it: there is a single RMC curve, not a vacuum/helium pair, and an attempt to
reproduce two would be chasing an artefact. [`RMC_KEFF_VS_HEIGHT`] carries
that single curve.

**RESOLVED 2026-10-01 from Şeker & Çolak (2003), NED 222:263, Table 3**
(the MCNP paper Li cites): Li's Table 3 MCNP column equals Şeker's
**vacuum** column and Li's Table 4 MCNP column equals Şeker's **helium**
column, in all 12 rows to 5 d.p. (checked by eye against the stored
constants). So Table 4's "(vacuum)" caption is the wrong one. Which medium
Li's single RMC curve was run in is still not stated (gh:#333).

**This model's coolant is helium by default since 2026-10-01**
(maintainer, gh:#426: "use helium, I think it is more accurate"): natural
helium at 300.15 K and an assumed 101.33 kPa in every coolant region.
Until then `mat::HELIUM` was an empty material, i.e. vacuum, which is now
the `OUTRAM_HTR10_VACUUM_COOLANT` ablation (`data::Coolant::Vacuum`).

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
> plane and six surrounding spheres. These six spheres are not centered at
> the corners of the hexagons, but rather, hexagonal prism side surfaces
> surround these balls. The intermediate section of each hexagonal prism
> contains three full balls as well as partial contributions from the
> neighboring hexagonal prism cells from all six sides."*

**CORRECTED 2026-10-01 (gh:#429):** the quotation above was previously
elided (`...`) exactly across the "not centered at the corners" sentence,
which is restored. The text is Li, Yu & Wei (2014) p.3, and it is
**verbatim from Şeker & Çolak (2003), p.266**, the MCNP model Li follows:
Şeker, V., Çolak, Ü. (2003), *HTR-10 full core first criticality analysis
with MCNP*, Nucl. Eng. Des. 222, 263–270, doi:10.1016/S0029-5493(03)00031-1.
Şeker is therefore the primary source for the cell; see "What Şeker & Çolak
(2003) settles" below.

— ~~does not determine a cell: it describes sharing between neighbours
without saying how much, and the figures carry no extractable dimensions.~~
**CORRECTED 2026-10-01:** the prose alone does not determine a cell, but the
figures do carry dimensions (the 6 cm ball is a built-in scale bar, gh:#429),
and Şeker's Table 3 ball counts constrain the cell independently (below).
The cell here is derived from the invariants the paper **does** state.

# The decode

The paper gives a layer height of 9.798 cm. For 6 cm spheres the close-packed
layer spacing is `d*sqrt(2/3)` = 4.8990 cm, and `9.798 / 4.8990 = 2.00001`.
The "layer" is exactly **two close-packed sphere layers**. That fixes the
axial structure and is not plausibly a coincidence.

The lattice is then **diluted** laterally until the filling matches the
paper's 61 %: ordered close packing would be 74.05 %, so the balls do not
touch. Expanding the pitch from the touching value `d` by
`sqrt(0.7405/0.61)` gives 6.6106 cm.

# ~~The check that makes it evidence~~ A consistency check, not evidence

The cell above was fitted to **two** stated quantities — layer height and
filling fraction. Tiling it through the stated core (180 cm diameter,
197 cm high) predicts **~27 038 balls** against the paper's stated **27 000**,
a 0.14 % difference. ~~Nothing was tuned to hit that, which is what makes the
reconstruction evidence rather than a fit.~~ **CORRECTED 2026-10-01
(gh:#430):** [`HexBedCell::balls_in_core`] reduces to
`V_core × 0.61 / V_ball` for **any** pitch and height, so the 27 038 follows
from the 0.61 put in and cannot fail. It checks arithmetic, not the cell.
The 27 000 is also the **equilibrium** (all-fuel) core, not the 57:43
initial core. The independent count the check lacked is Şeker & Çolak
(2003) Table 3 (below).

# What this is not

It reproduces the paper's stated *invariants*. It does **not** claim to be
their exact unit-cell tiling, which their text does not determine. Any
write-up must say so.

# What Şeker & Çolak (2003) settles (read 2026-10-01)

Şeker & Çolak (2003), NED 222:263–270 (full citation above), is the MCNP
model whose cell text Li (2014) reproduces. What it adds, and where **this
module departs from it**:

**1. An independent ball count (Table 3).** For N = 9…20 layers, every row
satisfies, exactly (checked arithmetically on all 12 rows):

| quantity | Şeker Table 3 |
|---|---|
| loading height | `9.798 N + 6.0` cm |
| total balls | `1346 N + 733` |
| fuel balls | `767 N + 418` (0.570 of the total) |

1346 balls per 9.798 cm layer in the r = 90 cm core is a filling fraction
of **0.6106**, Şeker's *"61%"*, measured **after** the wall rejection in
item 3. The constant 733 is one extra **basal** plane: both the top and the
bottom basal planes are whole balls. At the critical row (N = 12) the model
holds 16 885 balls against the experiment's 16 890 at 123.06 cm.

**2. The cell is clustered, not uniformly diluted (gh:#429).** The counts
split each layer into **733 balls in the basal plane and 613 in the central
plane**: areal fractions through the ball centres of **0.814 / 0.681** over
the whole bed. **This module's ~~cell~~ [`HexBedCell`] is uniform:
0.747 / 0.747.** A
reconstruction consistent with the text, the counts and Li's Fig. 3 (~~an
inference, medium confidence, not built~~ **built 2026-10-01 as
[`SekerCell`], gh:#472**): **13 balls per prism**, i.e. the 7
touching basal balls (7:6 = 1.167 against the counted 733:613 = 1.196) plus
3 full + 6 half central balls, with a hexagon circumradius ≈ 9.4–9.7 cm.
Fuel is assigned **per layer** to 0.57:0.43 (p.267), not as a fixed per-cell
pattern.

**3. Wall-crossing balls are rejected everywhere (gh:#331).** p.267:
*"Outer boundary of the array is the inner surface of the side reflector. If
any ball intersects with the reflector surface, it is rejected."* The same
applies at the cone and the discharge tube. ~~**This module cuts balls at the
side wall by default** and rejects them only at the cone and tube; see
[`TwoBallBed::rejecting_side_wall_crossers`].~~ **CORRECTED 2026-10-01
(gh:#428):** that was [`TwoBallBed`], now withdrawn. The default
[`SekerBed`] rejects at the side wall, the cone and the tube, so every kept
ball is whole.

**4. The top layer is whole.** p.267: *"The top layer is formed by adding
half spheres to each ball present in this layer."* The count (+733) says
the bottom plane is whole too. ~~**This module clips the top layer** at the
bed plane (gh:#429 item 3; see [`TwoBallBed`], "Axial layout").~~
**CORRECTED 2026-10-01 (gh:#428):** only the withdrawn [`TwoBallBed`]
clips it. [`SekerBed`]'s top and bottom basal planes are whole balls (see
its "Axial layout").

**5. The cone and discharge tube hold graphite balls only**, hexagonally
arranged and rejected at the surfaces (p.267), which is what
~~[`TwoBallBed`]~~ [`SekerBed`] builds (as the withdrawn [`TwoBallBed`]
did).

**6. The heights (gh:#333).** Şeker's height runs from the bottom of the
lowest ball to the top of the highest, but the same balls at 0.61 fill
**H − 0.55 cm** of volume-equivalent bed (0.58 cm at N = 9, 0.48 cm at
N = 20), not H − 6 cm. Matching a model to a row is therefore done
correctly by **ball count**. See [`super::RMC_KEFF_VS_HEIGHT`].

# Where it is built (2026-09-25)

**SUPERSEDED 2026-10-01 (gh:#472, noted gh:#428):** the transported bed is
now [`SekerBed`] on [`SekerCell`]. [`TwoBallBed`] is withdrawn (it cuts
pebbles; its environment knob panics), and [`HexBedCell`] survives for the
geometry closures (`super::geometry_closures`). The rest of this section is
the record.

[`TwoBallBed`] builds this cell as the transported bed of
`core_model::assemble_explicit_triso`: the hex tile IS the cell (two whole
balls per tile, A-B stacked), with fuel/dummy assigned per ball (gh:#309
step 2, gh:#310). Until then the lattice held one ball per half-height tile,
which dropped the A-B offset and made the pebbles interpenetrate.
[`bed_tile_levels`] (one identity per TILE) remains only for the
homogenised `core_model::assemble`, itself withdrawn 2026-10-01 (it cuts
pebbles, which is wrong physics).

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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
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

- **DistributionExt**
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

- **Imply**
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
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

- **DistributionExt**
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

- **Imply**
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
- **RuleType**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

- **DistributionExt**
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

- **Imply**
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
- **RuleType**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

- **DistributionExt**
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

- **Imply**
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TwoBallBed`

**The HTR-10 bed as the paper's two-ball prism cell**, with a fuel/dummy
identity for every BALL (gh:#309 step 2, gh:#310).

**WITHDRAWN 2026-10-01 (maintainer, gh:#472):** it cuts pebbles at the side
wall and the bed top, which is wrong physics, and it must not be run, not
even as an ablation. [`SekerBed`] is the bed. Kept as the record of what
the numbers of 2026-09-25 to 2026-09-30 were computed on.

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

**A departure from the source, stated 2026-10-01 (gh:#429 item 3):** Şeker
& Çolak (2003) p.267 build the top layer from whole balls (*"formed by
adding half spheres to each ball present in this layer"*), and their Table 3
count (`1346 N + 733`) implies a whole bottom basal plane as well. The clip
here is volume-equivalent, not ball-for-ball.

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
    pub tube: Option<DischargeTube>,
    pub rejected_balls: usize,
    pub side_wall_rejected: usize,
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
| `tube` | `Option<DischargeTube>` | The discharge tube below the conus, and with it Li's whole-ball<br>rejection, or `None` (see [`DischargeTube`]). |
| `rejected_balls` | `usize` | Balls removed by the rejection rule (0 without a tube). |
| `side_wall_rejected` | `usize` | Of [`Self::rejected_balls`], those removed at the bed side wall by<br>[`Self::rejecting_side_wall_crossers`] (0 by default). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(cell: HexBedCell, n_rings: usize, n_axial: usize, conus_height: f64, bed_radius: f64, assignment: FuelAssignment) -> Self { /* ... */ }
  ```
  Build the bed.

- ```rust
  pub fn new_with_tube(cell: HexBedCell, n_rings: usize, n_axial: usize, conus_height: f64, bed_radius: f64, tube: Option<DischargeTube>, assignment: FuelAssignment) -> Self { /* ... */ }
  ```
  Build the bed, with the discharge tube below the conus and Li's

- ```rust
  pub fn container_boundary_distance(self: &Self, centre: [f64; 3]) -> Option<f64> { /* ... */ }
  ```
  Distance \[cm\] from a point to the boundary of the conus-and-tube

- ```rust
  pub fn is_present(self: &Self, id: BallId) -> bool { /* ... */ }
  ```
  Whether ball `id` is in the model (not removed by the rejection rule).

- ```rust
  pub fn tile_present_mask(self: &Self, a: i32, b: i32, level: i32) -> u8 { /* ... */ }
  ```
  Presence mask of tile `(a, b, level)`: bit `i` set when the ball at

- ```rust
  pub fn rejecting_side_wall_crossers(self: Self) -> Self { /* ... */ }
  ```
  **Side-wall rejection (gh:#331 ablation, 2026-09-27).** Also remove

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
#### Struct `DischargeTube`

**The fuel discharge tube, filled with whole graphite balls, and the
rejection rule at the cone and tube surfaces.**

Li, Yu & Wei (2014), section on the RMC model: the cone region and the
discharge tube hold graphite balls only, arranged in the same hexagonal
geometry, and balls that intersect the cone or discharge-tube surface are
rejected. So a ball there is either wholly inside and kept, or removed, and
the space it would have taken is helium. The balls are all dummies (Terry
et al. 2005, section 2).

The container is the conus frustum (bed radius at the bed floor down to
`radius` at the conus floor) on top of a cylinder of `radius`, `depth`
deep. A ball is rejected when it crosses the cone, the tube wall or the
tube bottom. The bottom is where the model ends (TECDOC-1382 p. 242 gives
the tube to z = 6100 mm, the model bottom); rejecting there too keeps every
ball whole, as the rule intends.

**What this does NOT reject:** balls crossing the bed's side wall
(r = 90 cm) above the conus. Li says only that the array's outer boundary is
the side reflector's inner surface, not whether wall-crossing balls are cut
or removed. Those keep the CSG cut (the treatment before this), and the
~~choice is the maintainer's.~~
**DECIDED 2026-09-27 (maintainer, gh:#331): keep the cut.** Li states the
finished core's filling fraction is 61 %; the cut bed measures 0.6089, while
rejecting wall-crossers ([`TwoBallBed::rejecting_side_wall_crossers`], kept
as an ablation) drops it to 0.5737. *"Packing fraction wrong already changes
too much."* **SUPERSEDED 2026-10-01 (maintainer, gh:#331/#472):** the cut
is wrong physics. The default [`SekerBed`] rejects at the side wall too, and
the two-ball bed this paragraph describes is withdrawn.

`None` in [`TwoBallBed::new_with_tube`] (the `OUTRAM_HTR10_HOMOG_TUBE`
ablation) builds no tube balls and applies no rejection: the cone then cuts
partial balls, as it did before 2026-09-25.

```rust
pub struct DischargeTube {
    pub radius: f64,
    pub depth: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `f64` | Tube radius \[cm\] (25 cm, TECDOC-1382 p. 242). |
| `depth` | `f64` | Tube depth below the conus floor \[cm\], i.e. to the model bottom. |

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
    fn clone(self: &Self) -> DischargeTube { /* ... */ }
    ```

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &DischargeTube) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `SekerCell`

**Şeker & Çolak (2003)'s hexagonal-prism unit cell**, the default HTR-10 bed
cell since 2026-10-01 (gh:#472).

NEW WORK, not a port. The cell is reconstructed from the source's text and
Fig. 3; no dimension is fitted. Source: Şeker, V., Çolak, Ü. (2003), *HTR-10
full core first criticality analysis with MCNP*, Nucl. Eng. Des. 222,
263–270, doi:10.1016/S0029-5493(03)00031-1, pp. 266–267. Li, Yu & Wei
(2014) reproduce its text verbatim.

# What the source states

> *"The height of a layer is 9.798 cm. Top and bottom planes of hexagonal
> prisms are flat and contain half spheres. There are seven balls at these
> faces; one at the center of the basal plane and six surrounding spheres.
> These six spheres are not centered at the corners of the hexagons, but
> rather, hexagonal prism side surfaces surround these balls. The
> intermediate section of each hexagonal prism contains three full balls as
> well as partial contributions from the neighboring hexagonal prism cells
> from all six sides."*

# What Fig. 3 adds (read at 600 dpi, 2026-10-01, the 6 cm ball as scale)

Both panels show the same flat-topped hexagon (vertices at 0°, 60°, …; the
`HexOrientation::Y` convention), with circumradius ≈ 9.3–9.5 cm.
- **Basal plane:** a flower of 7 **touching** balls. The six outer balls
  point at the vertices and are tangent to the two side faces beside
  them. Neighbouring flowers touch.
- **Central plane:** a touching 6-ball triangle:
  - 3 balls at `d/√3` (90°, 210°, 330°), in the flower's hollows;
  - 3 at `2d/√3` (270°, 30°, 150°), each crossing a side face into the
    neighbouring cell;
  - through the other three faces, the neighbours' corner balls enter.

# What follows, with no free parameter

- apothem = `d cos 30° + d/2` = **8.196 cm** (3√3 + 3), pitch (flat to
  flat) **16.392 cm**;
- height = two close-packed layer spacings, `2 d sqrt(2/3)` = 9.798 cm, which
  is the stated layer;
- every contact is exactly `d`: the central balls sit `d/√3` from their
  three supporting balls laterally and `d sqrt(2/3)` vertically;
- **13 balls per cell** (7 basal + 6 central), filling fraction **0.6448**,
  areal fractions through the ball centres **0.851 basal / 0.729 central**.

The 0.6448 is the interior value. Şeker's *"61%"* is measured after the wall
rejection (see [`SekerBed`]).

```rust
pub struct SekerCell {
    pub apothem: f64,
    pub height: f64,
    pub ball_diameter: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `apothem` | `f64` | Hexagon apothem (centre to side face) \[cm\]. |
| `height` | `f64` | Cell height \[cm\]: one basal-to-basal layer. |
| `ball_diameter` | `f64` | Ball diameter \[cm\]. |

##### Implementations

###### Methods

- ```rust
  pub fn from_paper() -> Self { /* ... */ }
  ```
  The cell from Şeker & Çolak (2003) text and Fig. 3, for 6 cm balls. See

- ```rust
  pub fn pitch(self: &Self) -> f64 { /* ... */ }
  ```
  Flat-to-flat pitch \[cm\], the lattice pitch: `2 * apothem`.

- ```rust
  pub fn balls_per_cell(self: &Self) -> f64 { /* ... */ }
  ```
  Balls per cell: 7 basal (2 x 7 halves) + 6 central (3 whole, 3 x 2

- ```rust
  pub fn area(self: &Self) -> f64 { /* ... */ }
  ```
  Hexagon area \[cm^2\], `2 sqrt(3) apothem^2`.

- ```rust
  pub fn packing_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Ball volume fraction of the (interior) cell \[-\]: 0.6448.

- ```rust
  pub fn site_centre(self: &Self, site: SekerSite) -> [f64; 3] { /* ... */ }
  ```
  Tile-local centre \[cm\] of `site`. See [`SekerSite`].

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
    fn clone(self: &Self) -> SekerCell { /* ... */ }
    ```

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &SekerCell) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `SekerSite`

One of the 23 ball sites a Şeker tile holds a piece of. See [`SekerCell`].

| site | tile-local centre | shared with |
|---|---|---|
| `BottomFlower(k)` | flower ball `k` at `z = -h/2` | the tile below (half each) |
| `TopFlower(k)` | flower ball `k` at `z = +h/2` | the tile above (half each) |
| `Inner(k)` | `d/√3` at `90° + 120° k`, `z = 0` | nobody: wholly inside |
| `Outer(k)` | `2d/√3` at `270° + 120° k`, `z = 0` | the neighbour it crosses into |
| `Entering(k)` | `pitch - 2d/√3` at `90° + 120° k`, `z = 0` | the neighbour at `90° + 120° k`, whose `Outer(k)` it is |

Flower ball `k = 0` is the centre; `k = 1..=6` lie at `d` and angle
`60° (k - 1)`, pointing at the vertices.

```rust
pub enum SekerSite {
    BottomFlower(u8),
    TopFlower(u8),
    Inner(u8),
    Outer(u8),
    Entering(u8),
}
```

##### Variants

###### `BottomFlower`

Basal flower ball `k` (0..7) on the tile's bottom face.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |

###### `TopFlower`

Basal flower ball `k` (0..7) on the tile's top face.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |

###### `Inner`

Central-plane ball `k` (0..3) in the flower's hollows.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |

###### `Outer`

Central-plane corner ball `k` (0..3) of this tile's triangle.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |

###### `Entering`

A neighbour's central corner ball `k` (0..3) entering this tile.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |

##### Implementations

###### Methods

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
    fn clone(self: &Self) -> SekerSite { /* ... */ }
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

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &SekerSite) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SekerSite) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &SekerSite) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **RuleType**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `SekerBallId`

A ball of the Şeker bed, named by the tile that owns it. Every piece of one
ball, in every tile that holds a piece of it, resolves to the same id.

```rust
pub enum SekerBallId {
    Flower {
        a: i32,
        b: i32,
        face: i32,
        k: u8,
    },
    Central {
        a: i32,
        b: i32,
        level: i32,
        k: u8,
    },
}
```

##### Variants

###### `Flower`

Basal flower ball `k` of column `(a, b)` on lattice face `face` (the
bottom face of level `face`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `i32` | Skewed hex coordinate. |
| `b` | `i32` | Skewed hex coordinate. |
| `face` | `i32` | Lattice face index. |
| `k` | `u8` | Flower ball, 0..7. |

###### `Central`

Central-plane ball `k` of tile `(a, b, level)`: 0..3 `Inner`, 3..6
`Outer`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `i32` | Skewed hex coordinate. |
| `b` | `i32` | Skewed hex coordinate. |
| `level` | `i32` | Lattice level. |
| `k` | `u8` | 0..6. |

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
    fn clone(self: &Self) -> SekerBallId { /* ... */ }
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

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &SekerBallId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SekerBallId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &SekerBallId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **RuleType**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `SekerBed`

**The HTR-10 bed as Şeker & Çolak (2003) build it** (gh:#472): the
[`SekerCell`] lattice, every ball whole, rejected wherever it would cross a
boundary, and a fuel/dummy identity per ball.

NEW WORK, not a port.

# Axial layout: the loading height is Şeker's

A loading of `n_layers` = N holds basal planes `0..=N` and central planes
`0..N`. The lowest basal plane's balls sit **on** the bed floor (the top of
the conus) and the highest's top is the bed top. So the bed is
`9.798 N + 6.0` cm tall, **exactly Şeker's and Li's tabulated height**, and
the top and bottom layers are whole balls, as Şeker p.267 says (*"The top
layer is formed by adding half spheres to each ball present in this
layer"*). Şeker's Table 3 count (`1346 N + 733`) is one extra basal plane,
which is this layout.

Below the bed floor the same lattice continues through the conus and the
discharge tube. Şeker p.267: *"The cone region and discharge tube are
formed by only graphite balls ... also made by balls arranged in hexagonal
geometry"*.

# Rejection: every kept ball is whole

Şeker p.267: *"If any ball intersects with the reflector surface, it is
rejected"*, and *"Balls intersect with cone or discharge tube surface are
rejected."* A ball is kept only if it lies wholly inside the container, i.e.
the solid of revolution of the meridional profile:
- the bed top (`z = bed_top`, `rho < bed_radius`);
- the side wall;
- the cone from `(bed_radius, bed_bottom)` to `(tube_radius, conus_floor)`;
- the tube wall and the tube bottom.

That includes the **side wall** (gh:#331, decided by the maintainer
2026-10-01), where the two-ball bed cuts.

# Fuel/dummy identity

As [`TwoBallBed`]:
- the balls centred above the bed floor take the 57:43 split, by the
  low-discrepancy rule in layer order from the floor up (Şeker p.267:
  *"Fuel and moderator balls are selected in each layer such that 0.57:0.43
  ratio is established"*);
- the conus and the tube are all dummy.

# Checked against Şeker's own counts

`the_seker_bed_reproduces_table_3_per_plane_counts` compares the kept balls
per basal and per central plane with Şeker Table 3 (733 and 613).

```rust
pub struct SekerBed {
    pub cell: SekerCell,
    pub n_rings: usize,
    pub n_layers: usize,
    pub n_levels: usize,
    pub z_bottom: f64,
    pub floor_face: i32,
    pub bed_radius: f64,
    pub bed_bottom: f64,
    pub bed_top: f64,
    pub conus_floor: f64,
    pub tube_radius: f64,
    pub container_bottom: f64,
    pub assignment: FuelAssignment,
    pub eligible_balls: usize,
    pub fuel_balls: usize,
    pub rejected_balls: usize,
    pub boundary_rejected: usize,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell` | `SekerCell` | The unit cell. |
| `n_rings` | `usize` | Hex rings in the lattice (including the central tile). |
| `n_layers` | `usize` | Loading in Şeker layers N: bed height `cell.height * N + ball_diameter`. |
| `n_levels` | `usize` | Axial lattice levels. |
| `z_bottom` | `f64` | z \[cm\] of the bottom face of lattice level 0. |
| `floor_face` | `i32` | Lattice face index of the lowest bed basal plane (the one on the floor). |
| `bed_radius` | `f64` | Bed cylinder radius \[cm\]. |
| `bed_bottom` | `f64` | Bed floor \[cm\] (= top of the conus). |
| `bed_top` | `f64` | Bed top \[cm\] (= top of the highest ball). |
| `conus_floor` | `f64` | Conus floor \[cm\]. |
| `tube_radius` | `f64` | Discharge-tube radius \[cm\]. |
| `container_bottom` | `f64` | Bottom of the container \[cm\]: the tube bottom, or the conus floor when<br>the tube is not built (the `OUTRAM_HTR10_HOMOG_TUBE` ablation). |
| `assignment` | `FuelAssignment` | The rule the identities were assigned by. |
| `eligible_balls` | `usize` | Balls that took part in the 57:43 split. |
| `fuel_balls` | `usize` | Of which fuelled. |
| `rejected_balls` | `usize` | Lattice balls rejected (not wholly inside the container). |
| `boundary_rejected` | `usize` | Of [`Self::rejected_balls`], those whose centre is inside the container<br>but which cross its boundary: the balls Şeker's rule actually removes. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(cell: SekerCell, n_rings: usize, n_layers: usize, conus_height: f64, bed_radius: f64, tube_radius: f64, tube_depth: Option<f64>, assignment: FuelAssignment) -> Self { /* ... */ }
  ```
  Build the bed.

- ```rust
  pub fn lattice_centre_z(self: &Self) -> f64 { /* ... */ }
  ```
  z \[cm\] of the lattice centre, to pass to `HexLattice::from_rings_3d`.

- ```rust
  pub fn centre(self: &Self, id: SekerBallId) -> [f64; 3] { /* ... */ }
  ```
  Global centre \[cm\] of ball `id`.

- ```rust
  pub fn is_present(self: &Self, id: SekerBallId) -> bool { /* ... */ }
  ```
  Whether ball `id` is kept. Balls outside the lattice's range are absent.

- ```rust
  pub fn is_fuel(self: &Self, id: SekerBallId) -> bool { /* ... */ }
  ```
  Whether ball `id` is fuelled.

- ```rust
  pub fn tile_balls(self: &Self, a: i32, b: i32, level: i32) -> [SekerBallId; 23] { /* ... */ }
  ```
  The 23 balls tile `(a, b, level)` holds pieces of, in

- ```rust
  pub fn tile_masks(self: &Self, a: i32, b: i32, level: i32) -> (u32, u32) { /* ... */ }
  ```
  `(fuel, present)` masks of tile `(a, b, level)`: bit `i` for the ball at

- ```rust
  pub fn all_balls(self: &Self) -> Vec<SekerBallId> { /* ... */ }
  ```
  Every ball owned by a tile within the lattice's rings, deterministic

- ```rust
  pub fn plane_counts(self: &Self) -> (Vec<usize>, Vec<usize>) { /* ... */ }
  ```
  Kept balls in the bed (centre above the floor) per plane: `(basal

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SekerBed { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
#### Enum `PebbleBed`

**The pebble bed an explicit-TRISO core was built on**: Şeker & Çolak's
cell (the default since 2026-10-01, gh:#472) or the two-ball prism cell
(~~the `OUTRAM_HTR10_TWO_BALL_CELL` ablation~~ **withdrawn 2026-10-01**: it
cuts pebbles, which is wrong physics, and must never be run, not even as an
ablation; kept only as the record of earlier numbers).

```rust
pub enum PebbleBed {
    Seker(SekerBed),
    TwoBall(TwoBallBed),
}
```

##### Variants

###### `Seker`

[`SekerBed`], the default.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SekerBed` |  |

###### `TwoBall`

[`TwoBallBed`], ~~an ablation~~ withdrawn 2026-10-01 (it cuts pebbles);
never built by the default path, and its knob panics.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TwoBallBed` |  |

##### Implementations

###### Methods

- ```rust
  pub fn bed_bottom(self: &Self) -> f64 { /* ... */ }
  ```
  Bed floor \[cm\] (= top of the conus).

- ```rust
  pub fn bed_top(self: &Self) -> f64 { /* ... */ }
  ```
  Bed top \[cm\].

- ```rust
  pub fn conus_floor(self: &Self) -> f64 { /* ... */ }
  ```
  Conus floor \[cm\].

- ```rust
  pub fn bed_radius(self: &Self) -> f64 { /* ... */ }
  ```
  Bed cylinder radius \[cm\].

- ```rust
  pub fn n_rings(self: &Self) -> usize { /* ... */ }
  ```
  Hex rings of the lattice.

- ```rust
  pub fn n_levels(self: &Self) -> usize { /* ... */ }
  ```
  Axial lattice levels.

- ```rust
  pub fn z_bottom(self: &Self) -> f64 { /* ... */ }
  ```
  z \[cm\] of the bottom face of lattice level 0.

- ```rust
  pub fn lattice_centre_z(self: &Self) -> f64 { /* ... */ }
  ```
  z \[cm\] of the lattice centre.

- ```rust
  pub fn tile_pitch_and_height(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Lattice pitch (flat to flat) and tile height \[cm\].

- ```rust
  pub fn ball_diameter(self: &Self) -> f64 { /* ... */ }
  ```
  Ball diameter \[cm\].

- ```rust
  pub fn site_centres(self: &Self) -> Vec<[f64; 3]> { /* ... */ }
  ```
  Tile-local centres \[cm\] of the ball sites, in tile-mask bit order.

- ```rust
  pub fn tile_masks(self: &Self, a: i32, b: i32, level: i32) -> (u32, u32) { /* ... */ }
  ```
  `(fuel, present)` masks of tile `(a, b, level)`, bit `i` for site `i` of

- ```rust
  pub fn all_balls(self: &Self) -> Vec<BedBall> { /* ... */ }
  ```
  Every ball of the lattice.

- ```rust
  pub fn centre(self: &Self, ball: BedBall) -> [f64; 3] { /* ... */ }
  ```
  Global centre \[cm\] of `ball`.

- ```rust
  pub fn is_fuel(self: &Self, ball: BedBall) -> bool { /* ... */ }
  ```
  Whether `ball` is fuelled (see [`Self::centre`] for the panic).

- ```rust
  pub fn is_present(self: &Self, ball: BedBall) -> bool { /* ... */ }
  ```
  Whether `ball` is kept (see [`Self::centre`] for the panic).

- ```rust
  pub fn eligible_and_fuel_balls(self: &Self) -> (usize, usize) { /* ... */ }
  ```
  Balls that took part in the 57:43 split, and of which fuelled.

- ```rust
  pub fn tube_radius_and_bottom(self: &Self) -> Option<(f64, f64)> { /* ... */ }
  ```
  The discharge tube below the conus as `(radius, bottom z)` \[cm\], or

- ```rust
  pub fn core_balls(self: &Self) -> Option<usize> { /* ... */ }
  ```
  The bed's ball inventory: kept balls centred above the bed floor (the

- ```rust
  pub fn rejected_balls(self: &Self) -> usize { /* ... */ }
  ```
  Balls removed by a rejection rule.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PebbleBed { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
#### Enum `BedBall`

A ball of a [`PebbleBed`].

```rust
pub enum BedBall {
    Seker(SekerBallId),
    TwoBall(BallId),
}
```

##### Variants

###### `Seker`

A ball of a [`SekerBed`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SekerBallId` |  |

###### `TwoBall`

A ball of a [`TwoBallBed`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `BallId` |  |

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
    fn clone(self: &Self) -> BedBall { /* ... */ }
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

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &BedBall) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BedBall) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &BedBall) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **RuleType**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `seker_tile_ball`

**Attributes:**

- `MustUse { reason: None }`

The ball at `site` of Şeker tile `(a, b, level)`.

`Entering(k)` is the `Outer(k)` ball of the neighbour at `90° + 120° k`:
in the `HexOrientation::Y` skewed coordinates of [`tile_xy`] those are
`(a, b+1)`, `(a-1, b)` and `(a+1, b-1)`. Checked numerically by
`every_seker_tile_resolves_a_shared_ball_to_one_position`.

```rust
pub fn seker_tile_ball(a: i32, b: i32, level: i32, site: SekerSite) -> SekerBallId { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
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

## Module `reflector_geometry`

**The HTR-10 reflector as explicit 3-D geometry**: every boring at its own
position, in solid graphite, inside the benchmark's R-Z zone map.

# Why this replaced the homogenised bands (2026-09-25)

Maintainer direction: *"Don't average, just make channels explicit."* Until
then the reflector was TECDOC zone 22 almost everywhere, a full-height void
annulus at r 140.6-148.6 stood in for the twenty coolant channels, and the
control-rod, absorber-ball and irradiation borings were absent (their
smeared zones 31-40 existed only behind `OUTRAM_HTR10_BORINGS`).

# The specification

All inputs come from **IAEA-TECDOC-1382 part 2, § 4.1.2**, transcribed with
page references in `crates/kovan-literature/derived/tecdoc1382-htr10-mc-borings-and-zone-map.md`:

- **p. 241-242, the borings to model in a Monte Carlo calculation:** count,
  diameter, radial centre and axial extent of the coolant, control-rod,
  irradiation and small-absorber-ball (KLAK) channels, and of the hot gas
  duct. p. 242 has no text layer in the PDF, which is why this never
  reached the workspace's Markdown copy.
- **p. 242, the density corrections** that go with them: once the borings
  are explicit, the zones that had homogenised them take the solid
  graphite (zone 22), boronated brick (17) or carbon brick (18) densities,
  and zones 29/42 and 60 are scaled back up. See
  [`super::core_model::mat::for_zone_mc`].
- **p. 241, Fig. 4.10, the zone map**, as [`FIG_4_10_BOXES`].
- **p. 234, Fig. 4.7**, for the KLAK slot's shape (100 mm straight + R30).

Li, Yu & Wei (2014), the reference being compared against, states that its
top and side reflectors house the control rods, small absorber balls,
helium flow channels and irradiation channels, and refers every reflector
detail to that TECDOC. It never says any of them was homogenised.

# What the specification does NOT give, and what is done instead

**Channel azimuths.** The text gives counts and radii only, and Fig. 4.7 is
a 416 x 233 px raster (about 2 cm per pixel) that cannot resolve them. So
the placements below are a stated convention, NOT data: the 20 inner-ring
borings on an 18 degree pitch, the 20 coolant channels offset by 9
degrees, the hot gas duct along +x. The only constraint used is
geometric: TECDOC's own zone 44/62 densities are exactly additive in the
duct and channel voids, i.e. the duct overlaps none of them, and this
layout honours that. The assignment of the 20 inner-ring positions to 10
rods, 3 irradiation and 7 KLAK channels is likewise a convention. It
matters for the one-rod worth problems (B32, B42), not for B1.
**ACCEPTED 2026-09-27 (maintainer, gh:#330): "18 degree pitch is
acceptable"**, so this convention is the model.

**SUPERSEDED 2026-10-01 by data (gh:#330).** Şeker & Çolak (2003), NED
222:263, Fig. 4 draws every channel. It was read at 600 dpi, at a scale of
3.0 px/cm.
- **Inner ring:** 20 positions on an 18° pitch. The control rods sit at
  every other position, evenly spaced at 36°, as the text says (*"placed
  symmetrically in the side reflector"*, p.268). The KLAK slots and the
  irradiation channels fill the positions between them (the figure's 18°,
  54°, 90°, 126°, 198°, 270°, 306° and 162°, 234°, 342°).
- **Coolant ring:** at the **same** azimuths as the inner ring.

The earlier convention had the rods uneven (two adjacent pairs) and the
coolant offset by 9°.

The figure's pattern is used here, rotated by −171° so that the hot gas
duct (kept on +x) lies in the gap between an irradiation channel and a rod.
Both of those stop above the duct (`z_T` ≤ 450 cm against the duct's 465–495).
Both rings then sit at 9° + 18° k. Two things remain conventions:
- the duct's absolute azimuth relative to the channels, which no source
  gives;
- the figure's handedness (viewed from above or below is not stated). A
  mirror image does not change B1.

`the_channel_layout_is_sekers_fig_4` and
`the_hot_gas_duct_clears_every_channel_at_its_height` pin both.

**Contents.** B1 is defined with no rod inserted (p. 242), and the rods'
withdrawn position is given (lower end at 119.2 cm), so the rods ARE in
their channels, in the top reflector, with their B4C, steel sleeves and
iron joints as explicit geometry. The absorber-ball system is a reserve
shutdown system, so its channels are empty. The irradiation channels are
empty. Nothing is said about either; ~~both are open items~~ **DECIDED
2026-09-27 (maintainer, gh:#330): leave them empty.**

**Zones whose internal structure is unspecified** (the cold helium chamber,
zone 3; the bottom structures, zones 0 and 8-16; the partly-void layers 21,
29, 48, 57): these are explicit REGIONS at their Fig. 4.10 positions, but
each carries the source's own Table 4-3 composition because no geometry
for their contents is given anywhere. That is the source's homogenisation,
not ours. ~~It is recorded as an open item.~~ **ACCEPTED 2026-09-27
(maintainer, gh:#332): model these zones as TECDOC-1382 gives them.** No
source documents their internals, so the TECDOC composition is the
justified choice.

# Coordinates

TECDOC's axial coordinate `z_T` runs **downward** from the model top
(0) to the bottom (610 cm). The model's local z runs upward with the origin
at bed mid-height, so `z_local = refl_top - z_T`. Every zone boundary here
is fixed hardware: nothing but the bed top moves with the loading.

```rust
pub mod reflector_geometry { /* ... */ }
```

### Types

#### Enum `ChannelKind`

What a reflector channel is.

```rust
pub enum ChannelKind {
    Coolant,
    ControlRod,
    Irradiation,
    AbsorberBall,
}
```

##### Variants

###### `Coolant`

Cold-helium coolant channel: empty.

###### `ControlRod`

Control-rod channel: holds a rod at its withdrawn position.

###### `Irradiation`

Irradiation channel: empty.

###### `AbsorberBall`

Small-absorber-ball (KLAK) channel: empty (reserve shutdown system).

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
    fn clone(self: &Self) -> ChannelKind { /* ... */ }
    ```

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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ChannelKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `ReflectorChannel`

One vertical channel in the reflector.

```rust
pub struct ReflectorChannel {
    pub kind: ChannelKind,
    pub azimuth_deg: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `ChannelKind` | What it is. |
| `azimuth_deg` | `f64` | Azimuth of its centre \[deg\] from +x. A convention: see the module docs. |

##### Implementations

###### Methods

- ```rust
  pub fn centre_radius_cm(self: &Self) -> f64 { /* ... */ }
  ```
  Radius \[cm\] of the channel centre from the core axis.

- ```rust
  pub fn centre_xy(self: &Self) -> [f64; 2] { /* ... */ }
  ```
  Channel centre `(x, y)` \[cm\].

- ```rust
  pub fn zt_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Axial extent, `(z_T top, z_T bottom)` \[cm\].

- ```rust
  pub fn radial_extent_cm(self: &Self, zt: f64) -> (f64, f64) { /* ... */ }
  ```
  Radial extent `(r_min, r_max)` \[cm\] from the core axis at `z_T`

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
    fn clone(self: &Self) -> ReflectorChannel { /* ... */ }
    ```

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ReflectorChannel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `Fig410Box`

One rectangle of the Fig. 4.10 zone map, in `(r, z_T)`.

```rust
pub struct Fig410Box {
    pub zone: usize,
    pub r_in: f64,
    pub r_out: f64,
    pub zt_top: f64,
    pub zt_bot: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `zone` | `usize` | Table 4-3 zone number, as printed in the figure. |
| `r_in` | `f64` | Inner radius \[cm\]. |
| `r_out` | `f64` | Outer radius \[cm\]. |
| `zt_top` | `f64` | Top, `z_T` \[cm\] (smaller number: `z_T` runs downward). |
| `zt_bot` | `f64` | Bottom, `z_T` \[cm\]. |

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
    fn clone(self: &Self) -> Fig410Box { /* ... */ }
    ```

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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Fig410Box) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `is_klak_slot_zt`

**Attributes:**

- `MustUse { reason: None }`

Whether `z_T` lies in the KLAK channel's slot-shaped section.

```rust
pub fn is_klak_slot_zt(zt: f64) -> bool { /* ... */ }
```

#### Function `reflector_channels`

**Attributes:**

- `MustUse { reason: None }`

Every vertical channel in the reflector: 20 coolant, 10 control-rod,
3 irradiation, 7 KLAK. Positions follow the conventions in the module docs.

```rust
pub fn reflector_channels() -> Vec<ReflectorChannel> { /* ... */ }
```

### Constants and Statics

#### Constant `REFLECTOR_CELL_ID_BASE`

First cell id of the reflector's cells, clear of the bed-tile id ranges.

```rust
pub const REFLECTOR_CELL_ID_BASE: i32 = 100_000;
```

#### Constant `MODEL_BOTTOM_ZT_CM`

Model bottom, `z_T` \[cm\] (Fig. 4.10).

```rust
pub const MODEL_BOTTOM_ZT_CM: f64 = 610.0;
```

#### Constant `N_COOLANT_CHANNELS`

Coolant (cold helium flow) channels: count (p. 241).

```rust
pub const N_COOLANT_CHANNELS: usize = 20;
```

#### Constant `COOLANT_RADIUS_CM`

Coolant channel radius \[cm\]: 80 mm diameter (p. 241).

```rust
pub const COOLANT_RADIUS_CM: f64 = 4.0;
```

#### Constant `COOLANT_CENTRE_RADIUS_CM`

Coolant channel centre radius \[cm\]: 1446 mm (p. 241).

```rust
pub const COOLANT_CENTRE_RADIUS_CM: f64 = 144.6;
```

#### Constant `COOLANT_ZT_CM`

Coolant channel axial extent, `z_T` \[cm\]: 1050-6100 mm (p. 241).

```rust
pub const COOLANT_ZT_CM: (f64, f64) = _;
```

#### Constant `ROD_CHANNEL_RADIUS_CM`

Control-rod and irradiation channel radius \[cm\]: 130 mm diameter (p. 242).
Corroborated by Şeker & Çolak (2003), NED 222:263, p.268: *"Each hole
housing control rods are 13 cm in diameter."*

```rust
pub const ROD_CHANNEL_RADIUS_CM: f64 = 6.5;
```

#### Constant `ROD_CHANNEL_CENTRE_RADIUS_CM`

Control-rod and irradiation channel centre radius \[cm\]: 1021 mm (p. 242).
Corroborated by Şeker & Çolak (2003) p.268: ten rods *"placed symmetrically
in the side reflector and 102.1 cm away from the center"*, each *"five B4C
ring segments enclosed in stainless steel sleeves"*. Their channel azimuths
are drawn (Fig. 4) but not stated (gh:#330).

```rust
pub const ROD_CHANNEL_CENTRE_RADIUS_CM: f64 = 102.1;
```

#### Constant `ROD_CHANNEL_ZT_CM`

Control-rod and irradiation channel axial extent, `z_T` \[cm\]: 0-4500 mm
(p. 242).

```rust
pub const ROD_CHANNEL_ZT_CM: (f64, f64) = _;
```

#### Constant `N_IRRADIATION_CHANNELS`

Irradiation channels: count (p. 234, p. 242).

```rust
pub const N_IRRADIATION_CHANNELS: usize = 3;
```

#### Constant `N_KLAK_CHANNELS`

Small-absorber-ball (KLAK) channels: count (p. 234, p. 242).

```rust
pub const N_KLAK_CHANNELS: usize = 7;
```

#### Constant `KLAK_CENTRE_RADIUS_CM`

KLAK channel centre radius \[cm\]: 986 mm (p. 242; Fig. 4.7).

```rust
pub const KLAK_CENTRE_RADIUS_CM: f64 = 98.6;
```

#### Constant `KLAK_RADIUS_CM`

KLAK channel radius \[cm\]: round, 60 mm diameter, above and below core
height (p. 242); also the R30 end radius of the slot (Fig. 4.7).

```rust
pub const KLAK_RADIUS_CM: f64 = 3.0;
```

#### Constant `KLAK_SLOT_STRAIGHT_CM`

KLAK slot straight length between the two end-arc centres \[cm\]: 100 mm
(Fig. 4.7). Slot area `pi 3^2 + 6 x 10 = 88.27 cm^2`.

```rust
pub const KLAK_SLOT_STRAIGHT_CM: f64 = 10.0;
```

#### Constant `KLAK_ZT_CM`

KLAK channel axial extent, `z_T` \[cm\] (p. 242).

```rust
pub const KLAK_ZT_CM: (f64, f64) = _;
```

#### Constant `KLAK_SLOT_ZT_CM`

Where the KLAK channel is the slot rather than round, `z_T` \[cm\]:
1300-3887.64 mm (p. 242).

```rust
pub const KLAK_SLOT_ZT_CM: (f64, f64) = _;
```

#### Constant `HOT_GAS_DUCT_RADIUS_CM`

Hot gas duct radius \[cm\]: 300 mm diameter (p. 242).

```rust
pub const HOT_GAS_DUCT_RADIUS_CM: f64 = 15.0;
```

#### Constant `HOT_GAS_DUCT_AXIS_ZT_CM`

Hot gas duct axis, `z_T` \[cm\]: z = 4800 mm (p. 242).

```rust
pub const HOT_GAS_DUCT_AXIS_ZT_CM: f64 = 480.0;
```

#### Constant `HOT_GAS_DUCT_RHO_CM`

Hot gas duct radial extent \[cm\]: R = 900-1900 mm (p. 242).

```rust
pub const HOT_GAS_DUCT_RHO_CM: (f64, f64) = _;
```

#### Constant `INNER_RING_PITCH_DEG`

Angular pitch \[deg\] of the 20 inner-ring borings (10 rods + 3
irradiation + 7 KLAK). Şeker & Çolak (2003) Fig. 4 (module docs).

```rust
pub const INNER_RING_PITCH_DEG: f64 = 18.0;
```

#### Constant `RING_OFFSET_DEG`

Azimuth \[deg\] of inner-ring position 0, and of coolant channel 0: the
two rings are aligned (Şeker Fig. 4). The 9° itself places the hot gas duct
(on +x) in the gap between irradiation position 19 and rod position 0;
that part is a convention (module docs).
~~`COOLANT_OFFSET_DEG = 9.0`, with the inner ring at 0°: coolant offset
against the inner ring.~~ **CHANGED 2026-10-01 (gh:#330).**

```rust
pub const RING_OFFSET_DEG: f64 = 9.0;
```

#### Constant `KLAK_POSITIONS`

Inner-ring positions holding a KLAK channel: Şeker Fig. 4 (18°, 54°, 90°,
126°, 198°, 270°, 306°) rotated by −171°.
~~`[1, 4, 7, 10, 12, 15, 18]`, a convention~~ **CHANGED 2026-10-01
(gh:#330).**

```rust
pub const KLAK_POSITIONS: [usize; 7] = _;
```

#### Constant `IRRADIATION_POSITIONS`

Inner-ring positions holding an irradiation channel: Şeker Fig. 4 (162°,
234°, 342°) rotated by −171°. ~~`[3, 9, 16]`, a convention~~ **CHANGED
2026-10-01 (gh:#330).** The rods are the ten even positions.

```rust
pub const IRRADIATION_POSITIONS: [usize; 3] = _;
```

#### Constant `FIG_4_10_BOXES`

**IAEA-TECDOC-1382 Fig. 4.10**, every zone outside the pebble-filled
interior, as printed (transcription and cross-checks in
`kovan-literature/derived/tecdoc1382-htr10-mc-borings-and-zone-map.md`).

Not listed, because they are built from the bed's own surfaces: zone 5
(the cavity), the bed, the conus, zone 0 (between the cone and r = 90) and
the discharge tube, zones 6, 7 and 81, which Li (2014) fills with graphite
balls. Zones 31-40 are one box: the figure does not label their internal
boundaries and all ten share one density. Zone 66 is L-shaped and appears
twice.

```rust
pub const FIG_4_10_BOXES: &[Fig410Box] = _;
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

~~Not yet a benchmark model. The pebbles here carry a **homogenised** fuel
sphere rather than an explicit TRISO lattice, so the double heterogeneity is
absent and `k` from this is not comparable to the paper. That is deliberate:
this exists to measure the cost of the BED, which is the part at unprecedented
scale. The TRISO nesting multiplies on top and is priced separately.~~
**CORRECTED 2026-10-01 (gh:#428):** that described [`assemble`], which is
withdrawn and panics (its one-ball tile cuts pebbles). The production path
is [`assemble_explicit_triso`]: an explicit TRISO lattice in every fuel
pebble, Şeker & Çolak (2003)'s 13-ball bed (every ball whole, gh:#472), the
explicit TECDOC-1382 reflector (PR #327) and the withdrawn rods. It is the
model compared against RMC. It is still AI-drafted and awaiting human
review, so its results are tentative (see the parent module docs).

```rust
pub mod core_model { /* ... */ }
```

### Modules

## Module `mat`

```rust
pub mod mat { /* ... */ }
```

### Functions

#### Function `table_zone_slot`

**Attributes:**

- `MustUse { reason: None }`

Slot of a Table 4-3 zone listed in [`TABLE_4_3_ZONES`].

```rust
pub fn table_zone_slot(zone: usize) -> Option<usize> { /* ... */ }
```

#### Function `for_zone_mc`

**Attributes:**

- `MustUse { reason: None }`

**Material slot of a Fig. 4.10 zone in the Monte Carlo model**, with
IAEA-TECDOC-1382 p. 242's corrections for explicit borings applied.

- zones 23, 25-26, 28, 30-41, 43-45, 49-50, 52-54, 58-59, 61-63, 66-67,
  69-71, 80, 82 take zone 22's density: [`REFLECTOR`];
- zones 27, 46, 55, 64, 72, 74-79 take zone 17's: [`BORONATED`];
- zones 47, 56, 65, 73 take zone 18's (carbon brick);
- zones 29, 42 and 60 are scaled, and every other zone keeps its own
  Table 4-3 value: see [`TABLE_4_3_ZONES`].

# Panics

For zone 5 (the void cavity) and zones 6, 7 and 81 (the discharge tube,
which holds explicit graphite balls), none of which is a material zone
in this model, and for zones that do not exist.

```rust
pub fn for_zone_mc(zone: usize) -> usize { /* ... */ }
```

#### Function `for_zone_uniform`

**Attributes:**

- `MustUse { reason: None }`

**ABLATION** (`OUTRAM_HTR10_NO_ZONE_MAP`): the reflector this model had
before the zone map, i.e. zone-22 graphite everywhere except the
boronated bricks at r > 167.793 cm (zones 75-79).

```rust
pub fn for_zone_uniform(zone: usize) -> usize { /* ... */ }
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

The coolant: between pebbles, in the core cavity, the empty channels
and between the discharge-tube balls. Natural helium (ideal gas at
300.15 K and 101.33 kPa) since 2026-10-01 (gh:#426); until then an empty
material, i.e. exact vacuum, which is now the
`OUTRAM_HTR10_VACUUM_COOLANT` ablation (`htr10_rmc::data::Coolant`).

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

**No longer placed by [`super::assemble_explicit_triso`] (2026-09-25).**
The borings are explicit geometry there (`super::super::reflector_geometry`),
and the smeared band and its `OUTRAM_HTR10_BORINGS` knob are gone. The
slot is kept so every index after it stays put.

```rust
pub const BORED_GRAPHITE: usize = 9;
```

#### Constant `HOMOG_DUMMY`

**Homogenised dummy pebbles** — pebble graphite at the bed's filling
fraction.

~~What the discharge tube actually contains.~~ **CORRECTED 2026-09-25:**
the tube contains whole graphite balls, and since then
[`super::assemble_explicit_triso`] places them explicitly (Li, Yu & Wei
2014, following Şeker & Çolak 2003 p.267: *"The cone region and
discharge tube are formed by only graphite balls"*, arranged in
hexagonal geometry, and balls intersecting the cone or tube surface are
rejected). This smear is only the `OUTRAM_HTR10_HOMOG_TUBE`
ablation now.

```rust
pub const HOMOG_DUMMY: usize = 10;
```

#### Constant `FUEL`

Homogenised fuel zone, used only by [`super::assemble`] (withdrawn
2026-10-01; it panics).

```rust
pub const FUEL: usize = KERNEL;
```

#### Constant `ZONE_TABLE_FIRST`

First slot of the IAEA-TECDOC-1382 Table 4-3 zone materials that keep a
composition of their own in the Monte Carlo model.

```rust
pub const ZONE_TABLE_FIRST: usize = 11;
```

#### Constant `TABLE_4_3_ZONES`

Table 4-3 zones that keep a composition of their own once the borings
are explicit, in slot order from [`ZONE_TABLE_FIRST`], with the factor
TECDOC p. 242 applies to each (1.0 = the table value unchanged).

Which zones, and the factors, are p. 242's: zones 29 and 42 are
multiplied by 1.29978 and zone 60 by 1.16051, which puts back exactly
the boring void those zones had homogenised (see
`kovan-literature/derived/tecdoc1382-htr10-mc-borings-and-zone-map.md`,
§ 3). Zone 18 is the plain carbon brick; zones 51 and 68 share zone 24's
row of the table and use its slot.

Zones 0-4, 8-16, 19-21, 48 and 57 are **homogenised by the source**:
it gives no geometry for what is inside them (the cold helium chamber,
the hot-gas borings under the conus, the bottom structures). They are
explicit regions at their Fig. 4.10 positions carrying the source's
composition. ~~That is an open item~~ **Accepted 2026-09-27 (maintainer,
gh:#332)** as the model: undocumented internals are modelled as the
TECDOC gives them. Not a modelling choice made here.

```rust
pub const TABLE_4_3_ZONES: [(usize, f64); 24] = _;
```

#### Constant `ROD_B4C`

B4C of the control-rod absorber rings (TECDOC ~~§ 4.1.2~~ § 4.1.1.5
"Control of HTR-10", pp. 235-236: 1.7 g/cm³; CORRECTED 2026-10-01,
gh:#428). ~~natural boron~~ The section gives no isotopics: natural
boron is this model's reading (consistent with MIT's TECDOC Table 4-36).

```rust
pub const ROD_B4C: usize = _;
```

#### Constant `ROD_STEEL`

Stainless steel of the control-rod sleeves (TECDOC ~~§ 4.1.2~~
§ 4.1.1.5, pp. 235-236: 7.9 g/cm³, Cr 18 / Fe 68.1 / Ni 10 / Si 1 /
Mn 2 / C 0.1 / Ti 0.8 wt%; section CORRECTED 2026-10-01, gh:#428).

```rust
pub const ROD_STEEL: usize = _;
```

#### Constant `ROD_IRON`

Iron of the control-rod joints and ends (TECDOC ~~§ 4.1.2~~ § 4.1.1.5,
pp. 235-236: Fe only, 0.04 atoms/(b cm), for 27.5 mm < R < 55 mm;
section CORRECTED 2026-10-01, gh:#428).

```rust
pub const ROD_IRON: usize = _;
```

#### Constant `COUNT`

Number of material slots.

```rust
pub const COUNT: usize = _;
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
    pub bed: Option<super::bed::PebbleBed>,
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
| `lat_pitch` | `f64` | Hex pitch \[cm\] of the bed lattice. [`assemble_explicit_triso`]:<br>Şeker's 13-ball prism, 16.392 cm ([`SekerCell::from_paper`], since<br>2026-10-01); the two-ball ~~ablation's~~ cell's 6.6106 cm<br>([`HexBedCell::from_paper`]; withdrawn 2026-10-01).<br>[`assemble`] (withdrawn 2026-10-01, it panics): ~~solved from the<br>fuel-zone target~~ was solved so its axially clipped one-ball tile<br>realised the paper's fuel-zone fraction, 6.6086 cm (gh:#308). |
| `lat_height` | `f64` | Axial tile height \[cm\]. [`assemble_explicit_triso`]: 9.798 cm, one<br>Şeker layer (or, in the withdrawn two-ball cell, one A-B pair).<br>[`assemble`] (withdrawn): 4.899 cm, one ball. The bed height is `2 * bed_half_height`: `9.798 N +<br>6` cm for Şeker's bed, `n_axial x 4.899` cm for the others. |
| `conus_floor` | `f64` | Bottom of the conus \[cm\] — the deepest fuelled z. Equal to<br>`-bed_half_height` when no conus is modelled. |
| `cavity_top` | `f64` | Top of the empty core cavity \[cm\], i.e. where the axial reflector<br>begins. Equals `bed_half_height` when no reflector is built. |
| `refl_top` | `f64` | Top of the whole assembled model \[cm\]: cavity top + the 130 cm axial<br>reflector. Equals `bed_half_height` when no reflector is built. |
| `refl_bottom` | `f64` | Bottom of the whole assembled model \[cm\] (negative): conus floor less<br>the fixed [`HTR10_BOTTOM_REFLECTOR_CM`]. Equals `-bed_half_height` when<br>no reflector is built.<br><br>The model is **not** symmetric about `z = 0` (the bed mid-height): see<br>[`HTR10_BOTTOM_REFLECTOR_CM`] for why the old mirrored bottom was wrong.<br>With a reflector, `refl_top - refl_bottom` is [`HTR10_MODEL_HEIGHT_CM`]<br>at every loading. |
| `bed` | `Option<super::bed::PebbleBed>` | The ball-level description of the bed (every ball's centre, identity and<br>presence) that [`assemble_explicit_triso`] built its lattice from;<br>`None` for the one-ball [`assemble`]. Added 2026-09-26 so plots can cut<br>through ball centres chosen from the built bed rather than from constants.<br>~~`Option<TwoBallBed>`~~ **CHANGED 2026-10-01 (gh:#472):** a<br>[`PebbleBed`], Şeker's cell by default. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **DistributionExt**
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

- **Imply**
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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
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
is the void at **one** loading — ~~the benchmark's~~ the **experimental**
first-criticality loading of 123.06 cm (CORRECTED 2026-10-01, gh:#428:
IAEA-TECDOC-1382 p. 251, 16 890 balls in 15 °C air with as-built materials;
RMC's tabulated benchmark row is 123.576 cm), where
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

**Attributes:**

- `Other("#[allow(unreachable_code, unused_variables)]")`

**Assemble a delta-tracked pebble bed inside a surface-tracked reflector.**

**WITHDRAWN 2026-10-01 (maintainer): this function panics.** Its one-ball
tile clips every pebble at the tile faces, so its pebbles are cut, and
*"the cut pebble is wrong physics, never run it for ablation again"*. It
is not to be run as a diagnostic or an ablation. Use
[`assemble_explicit_triso`], whose balls are all whole. The body is kept
only as the record of what earlier numbers were computed on.

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

# The bed: Şeker & Çolak (2003)'s 13-ball cell (default since 2026-10-01, gh:#472)

The hex lattice tile is [`SekerCell::from_paper`]:
- pitch 16.392 cm, height 9.798 cm;
- per tile, 7 basal balls (a flower at each face, half each) and 6 central
  balls (a 6-ball triangle, 3 of them crossing into neighbours);
- every tile universe holds pieces of **23** balls ([`super::bed::SekerSite`]).

The bed is a [`SekerBed`]:
- `layers` Şeker layers, `9.798 N + 6` cm from the bottom of the lowest ball
  to the top of the highest, i.e. Şeker's and Li's own height axis;
- every ball whole, rejected wherever it would cross the side wall, the
  cone or the tube;
- the conus and tube hold graphite balls of the same lattice.

Each tile gets the universe for its (fuel, presence) masks, built only once
per distinct pair. Source and derivation: [`SekerCell`], [`SekerBed`].

~~**Ablation:** `OUTRAM_HTR10_TWO_BALL_CELL=1` builds the two-ball bed below
instead, with `n_axial = 2 N + 1` half-layers. With it,
`OUTRAM_HTR10_REJECT_SIDE_WALL=1` rejects wall-crossers there too.~~
**WITHDRAWN 2026-10-01 (maintainer):** *"The cut pebble is wrong physics,
never run it for ablation again."* The two-ball bed cuts pebbles: at the
side wall by default, and at the bed-top plane always. A cut pebble is not
a pebble, so an ablation against it measures nothing physical. The knob now
panics. The code below is kept only as the record of what the earlier
numbers were computed on.

# The two-ball bed (gh:#309 step 2, gh:#310), WITHDRAWN 2026-10-01 (history)

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
- `layers` — the fuel loading in **Şeker layers N** (since 2026-10-01): the
  bed is `9.798 N + 6` cm, so N = 9 … 20 are exactly the rows of
  [`super::RMC_KEFF_VS_HEIGHT`] (N = 12 is the critical 123.576 cm).
  ~~`n_axial`, the loading height in half-layers of 4.899 cm (20 / 25 / 41
  gave 97.980 / 122.474 / 200.858 cm)~~: that is now only the two-ball
  ablation's internal `2 N + 1`.
- `majorant_index` — which entry of the caller's majorant table the bed
  uses; `usize::MAX` surface-tracks the bed.

Universes: 0 root, [`TRISO_PARTICLE_UNIVERSE`], [`TRISO_MATRIX_UNIVERSE`],
then one per distinct bed-tile (fuel, presence) mask pair. ~~(35 in all at 14
rings, all 32 masks occur)~~ **CORRECTED 2026-10-01:** on Şeker's bed most
tiles are unique, 1 502 universes at 14 x 12 (printed by
`examples/htr10_geometry_images.rs`). Tile cell ids encode their role, see
[`tile_cell_role`].

~~Four coordinate levels~~ **CORRECTED 2026-09-25 — three coordinate
levels**: root → (bed hex lattice) → bed-tile universe (pieces of 23
pebbles on Şeker's cell since 2026-10-01; five on the two-ball cell; one
before that) → (TRISO rect lattice, entered
through the fuel-zone cell's translation to its ball centre) → TRISO
particle universe. A lattice selects the next level's universe but is not a
level itself. Verified by locating a kernel in the assembled core:
`path.levels.len() == 3`, lattices `[None, Some(0), Some(1)]`
(`examples/htr10_geometry_images.rs` prints it; re-checked on the two-ball
cell 2026-09-25, and on Şeker's cell at 14 x 12 on 2026-10-01). Depth-3 descent was gated
in `outram-mc-libs` `tests/nested_lattice_depth3.rs`, which also counts
`levels.len()`; ~~this is depth 4~~ this is the **same** depth.

# The TRISO lattice

A cubic array clipped to the fuel zone, keeping only whole particles, per
`cubic_array_in_ball`. Expressed as a `RectLattice` whose tiles hold either a
particle universe or matrix graphite, with `outer` = matrix so anything
beyond the array's extent is graphite.

~~The realised particle count is **8340**, not the paper's stated 8335.~~
**CHANGED 2026-10-01 (gh:#430):** the realised count is the stated
**8335**, through a generic lattice offset (see `TRISO_OFFSET` in the body).
The struck history follows.
~~See `cubic_array_in_ball`'s docs for why 8335 is unattainable (the count
moves in symmetry shells).~~ **CORRECTED 2026-10-01 (gh:#430):** 8335 **is**
attainable: a generic lattice offset gives exactly 8335, and Şeker & Çolak
(2003) p.266 build the same whole-particle cubic lattice and state that
*"the number of full fuel particles inside a fuel ball is verified to be
8335"*. Only the symmetric offsets tried here miss it. That is +0.060 % in
fuel volume, < 5 pcm.

```rust
pub fn assemble_explicit_triso(n_rings: usize, layers: usize, majorant_index: usize) -> AssembledCore { /* ... */ }
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
~~**CORRECTED 2026-09-25:** [`assemble_explicit_triso`] realises it by
construction (the paper's two-ball cell, [`HexBedCell::from_paper`], whose
pitch is derived from it); only [`assemble`] still solves its pitch for it.~~
**CORRECTED 2026-10-01 (gh:#428):** neither builder uses it for the bed any
more. [`assemble`] is withdrawn, and [`assemble_explicit_triso`] builds
Şeker's 13-ball cell, which is not built to a filling fraction (its
interior is 0.6448; whole-ball rejection at the wall brings the bed to
about 0.60). This constant still sets the discharge-tube smear
(`mat::HOMOG_DUMMY`), which is only the `OUTRAM_HTR10_HOMOG_TUBE` ablation,
~~which since the two-ball cell agrees with the bed it homogenises (sampled
0.6096-0.6097)~~ and it is the stated value the geometry closures compare
against.

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
z = 388.764, corroborated against ~~TECDOC-1382 Table 2's~~ **Terry (2005)
Table 2's** stated 36.946 (CORRECTED 2026-10-01, gh:#428: the TECDOC text
carries no 36.946; checked by search of its chapter 4 text. Whether its
Fig. 4.10 labels give it is Not re-checked: the figure has no text layer).
~~**It is full of pebbles**, so omitting it omits fuel.~~ **CORRECTED
2026-10-01 (gh:#428):** it is full of **dummy** pebbles only, so omitting it
omits graphite, not fuel: TECDOC-1382 p. 235 (*"dummy balls ... will be
firstly placed into the discharge tube and the bottom conus region"*),
Şeker & Çolak (2003) p. 267, and the code (the conus balls are all dummy,
`tests::the_built_bed_is_57_percent_fuel_balls_and_the_conus_none`).

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

**Not placed by [`assemble_explicit_triso`] since 2026-09-25** (noted
2026-10-01, gh:#428): the borings are explicit there, so this smear only
fills the unused `mat::BORED_GRAPHITE` slot and is read by the
`htr10_deterministic_vs_mc` and `htr10_mgxs_genfoam` examples. It duplicates zones 31-40 of
[`super::reflector::zone_composition`] (they agree today; a drift risk).

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
[`assemble_explicit_triso`]. A tile cell's id is
`base + TILE_SITE_STRIDE*v + site` (`site` the index in the bed's site list,
[`super::bed::SekerSite::ALL`] or [`super::bed::BallSite::ALL`]; `v` the ordinal of the tile's
universe variant, i.e. its (fuel mask, presence mask) pair, see
[`PebbleBed::tile_masks`]), or `base + TILE_SITE_STRIDE*v` for the helium
cell. Read back with [`tile_cell_role`].

~~`base + 10*mask`, bases 1000-4000~~ **CHANGED 2026-09-25**: a variant
is now a pair of 5-bit masks (up to 1024 combinations), so the bases moved
to 10 000-40 000 and `v` counts the variants actually built.
~~`base + 10*v`, bases 10 000-40 000~~ **CHANGED 2026-10-01 (gh:#472)**:
Şeker's tile holds 23 sites and the bed needs thousands of variants (most
tiles are unique), so the stride is 100 and the bases 1 000 000-4 000 000,
clear of the reflector's ids (from
[`super::reflector_geometry::REFLECTOR_CELL_ID_BASE`]).

```rust
pub const TILE_FUEL_ZONE_CELL_ID: i32 = 1_000_000;
```

#### Constant `TILE_FUEL_SHELL_CELL_ID`

See [`TILE_FUEL_ZONE_CELL_ID`].

```rust
pub const TILE_FUEL_SHELL_CELL_ID: i32 = 2_000_000;
```

#### Constant `TILE_DUMMY_BALL_CELL_ID`

See [`TILE_FUEL_ZONE_CELL_ID`].

```rust
pub const TILE_DUMMY_BALL_CELL_ID: i32 = 3_000_000;
```

#### Constant `TILE_HELIUM_CELL_ID`

See [`TILE_FUEL_ZONE_CELL_ID`].

```rust
pub const TILE_HELIUM_CELL_ID: i32 = 4_000_000;
```

#### Constant `TILE_SITE_STRIDE`

Id stride between tile-universe variants (more than the 23 sites).

```rust
pub const TILE_SITE_STRIDE: i32 = 100;
```

#### Constant `TILE_VARIANTS_MAX`

Variants the id scheme can hold: `1_000_000 / TILE_SITE_STRIDE`.

```rust
pub const TILE_VARIANTS_MAX: usize = 10_000;
```

## Module `control_rod`

HTR-10 control rods — the published rod geometry, and a smeared composition
of the side-reflector boring band.

# What this is for

~~The HTR-10 core model in [`super::core_model`] carries the ten control-rod
borings as a single homogenised annulus (`mat::BORED_GRAPHITE`, TECDOC zones
31-40, r 95.6-108.6 cm) at **28 % less carbon** than solid reflector
graphite. That band represents the borings as *empty*: there is no absorber
anywhere in the model, so it can only ever represent rods **fully
withdrawn**, and control-rod worth cannot be computed from it at all.~~

**CORRECTED 2026-10-01 (gh:#428, related gh:#312).** That described the core
model before 2026-09-25. Today:
- The Monte Carlo production path,
  [`assemble_explicit_triso`](super::core_model::assemble_explicit_triso),
  never places `mat::BORED_GRAPHITE`. Its ten rods are **explicit**
  geometry in their own channels (the rod universe of
  [`super::reflector_geometry`]): B4C rings, steel sleeves and iron joints,
  at the withdrawn position by default. They are built from this module's
  [`AXIAL_SECTIONS_CM`], [`AXIAL_IS_B4C`], [`LOWER_END_WITHDRAWN_CM`] and
  [`N_CONTROL_RODS`] (checked by search of `reflector_geometry.rs`).
- The smeared composition below is the absorber the band would carry with
  the rods in it, derived from the published rod geometry and B4C density
  with nothing fitted. Its one consumer is `crate::rod_insertion`, which
  adds it to multigroup constants for deterministic rod sweeps. It is not
  used by the Monte Carlo model.

# Source

IAEA-TECDOC-1382 ~~§ 4.1.2~~ **§ 4.1.1.5 "Control of HTR-10" (printed
pp. 235-236; CORRECTED 2026-10-01, gh:#428)**: ring radii, axial sequence,
B4C density 1.7 g/cm³, withdrawn and inserted lower-end positions. That
section gives no B4C isotopics; natural boron is this model's reading
(consistent with MIT's homogenised rod, TECDOC Table 4-36). Transcribed in
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
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

## Module `data`

**The HTR-10 nuclear-data set, in one place: which nuclides, from which
tapes, with which thermal laws, at which slots.**

# Why this module exists (2026-10-01)

Until 2026-10-01 every HTR-10 driver built its own nuclide array:
`htr10_rmc_keff::nuclides`, `htr10_endf8_height_sweep::nuclides_endf8`, and
a private `const NUC: Htr10Nuclides` in half a dozen other examples. The
sweep's own docs said *"the two must not drift: if a tape name or a thermal
law changes there, change it here"*, which is the drift this workspace
forbids, stated as an instruction. Three maintainer decisions of 2026-10-01
changed the nuclide set at once (natural carbon, gh:#425; helium coolant,
gh:#426; real nickel, gh:#329), so the set is built here and every driver
asks for it.

# Two steps, so the composition can be checked without loading anything

1. [`Htr10NuclideLayout::plan`] is **pure**: from an [`Htr10DataConfig`]
   it decides every slot (name, tape, thermal law) and the index tables
   the materials use ([`Htr10Nuclides`], [`CoolantNuclides`],
   [`RodMetalNuclides`]). Unit tests build the material set from a plan and
   check compositions with no nuclear data on disk.
2. [`load_htr10_nuclides`] reads the tapes the plan names, binds the
   thermal laws, and records every item in a [`RunDiagnostics`].

# The default is the correct physics (workspace hard rule)

[`Htr10DataConfig::default`] is ENDF/B-VIII.0 with:
- **natural carbon**, C-12 / C-13 at 98.93 / 1.07 at.% (gh:#425);
- **helium coolant** at 300.15 K and 101.33 kPa (gh:#426);
- **real nickel and iron** in the rod steel, Ni-58/60/61/62/64 and
  Fe-54/56/57/58 (gh:#329). ~~which cannot be loaded until gh:#339 is
  fixed~~ #339 is fixed (2026-10-01; see [`FE57_RECONSTRUCTION_FIXED`]);
- every bound thermal law: graphite (30P), C-in-SiC, Si-in-SiC, U-in-UO2,
  O-in-UO2.

Every other choice is a named ablation on one field of the config. The
pin is `tests/htr10_correct_physics_is_default.rs`.

```rust
pub mod data { /* ... */ }
```

### Types

#### Enum `NuclearDataLibrary`

Which evaluated library the nuclides come from.

```rust
pub enum NuclearDataLibrary {
    EndfB8,
    EndfB7,
}
```

##### Variants

###### `EndfB8`

ENDF/B-VIII.0. **Default.**

###### `EndfB7`

ENDF/B-VII.0, the library Li, Yu & Wei (2014) state for RMC
(`OUTRAM_HTR10_ENDF7`). Carbon is elemental C-nat (MAT 600); there is no
SiC thermal law; the rod metals and helium come from VIII.0 because the
checkout has no VII.0 tapes for them (a mixed-library arm, stated in the
diagnostics).

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
    fn clone(self: &Self) -> NuclearDataLibrary { /* ... */ }
    ```

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
    fn default() -> NuclearDataLibrary { /* ... */ }
    ```

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NuclearDataLibrary) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `CarbonTreatment`

How natural carbon is represented on the ENDF/B-VIII.0 path.

The VII.0 arm ignores this: its evaluation is elemental C-nat, which is
natural carbon already.

```rust
pub enum CarbonTreatment {
    Natural,
    AllC12,
}
```

##### Variants

###### `Natural`

**Default (maintainer decision 2026-10-01, gh:#425):** natural carbon,
C-12 / C-13 at 98.93 / 1.07 at.% (IUPAC), each kind of carbon two
nuclides with the same thermal law bound to both.

###### `AllC12`

ABLATION (`OUTRAM_HTR10_CARBON_AS_C12`): all carbon loaded as C-12 at
the natural-carbon atom density. This was the VIII.0 model until
2026-10-01; kept so the C-13 term can be priced.

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
    fn clone(self: &Self) -> CarbonTreatment { /* ... */ }
    ```

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
    fn default() -> CarbonTreatment { /* ... */ }
    ```

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CarbonTreatment) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `Coolant`

What fills the coolant regions (`mat::HELIUM`: between the pebbles, the
core cavity, the discharge tube between balls, the empty control-rod,
KLAK, coolant and irradiation channels).

```rust
pub enum Coolant {
    Helium,
    Vacuum,
}
```

##### Variants

###### `Helium`

**Default (maintainer decision 2026-10-01, gh:#426, "use helium, I
think it is more accurate"):** natural helium, ideal gas at the
material temperature and [`super::materials::HELIUM_PRESSURE_KPA`].

###### `Vacuum`

ABLATION (`OUTRAM_HTR10_VACUUM_COOLANT`): an empty material, i.e. exact
vacuum. Kept because Li, Yu & Wei (2014) report *"Calculations are
performed for vacuum and helium"*, and this was the model until
2026-10-01.

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
    fn clone(self: &Self) -> Coolant { /* ... */ }
    ```

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
    fn default() -> Coolant { /* ... */ }
    ```

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Coolant) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `RodMetalTreatment`

How the withdrawn control rods' sleeve steel and joint iron are
represented (TECDOC-1382 § 4.1.1.5).

```rust
pub enum RodMetalTreatment {
    Full,
    Simplified,
    NiAsFeOnly,
    Fe57AsFe56Only,
    NotModelled,
}
```

##### Variants

###### `Full`

**Default, the FULL case (maintainer decision 2026-10-01, gh:#329):**
real Ni-58/60/61/62/64 (tapes from the `reference-data/ace` submodule)
and real Fe-54/56/57/58. **Cannot be loaded until gh:#339 is fixed**
([`FE57_RECONSTRUCTION_FIXED`]); the loader refuses it rather than
swapping Fe-57.

###### `Simplified`

The SIMPLIFIED case: Ni replaced atom for atom by Fe (Ni-58/60 -> Fe-56,
Ni-61 -> Fe-57 -> Fe-56, Ni-62 -> Fe-54, Ni-64 -> Fe-58) and Fe-57 taken
as Fe-56. Both are stated modelling assumptions (maintainer 2026-09-26),
equivalent to `OUTRAM_HTR10_NI_AS_FE=1 OUTRAM_HTR10_FE57_AS_FE56=1`.

###### `NiAsFeOnly`

`OUTRAM_HTR10_NI_AS_FE` alone: Ni -> Fe, real Fe-57 (so also blocked
by gh:#339).

###### `Fe57AsFe56Only`

`OUTRAM_HTR10_FE57_AS_FE56` alone: real Ni, Fe-57 -> Fe-56.

###### `NotModelled`

`OUTRAM_HTR10_NO_WITHDRAWN_RODS`: the rod channels are empty helium and
no rod-metal tape is loaded. The rod materials are still built; the
caller strips their unloaded components and proves no cell uses them.

##### Implementations

###### Methods

- ```rust
  pub fn ni_as_fe(self: Self) -> bool { /* ... */ }
  ```
  Whether Ni is replaced by Fe.

- ```rust
  pub fn fe57_as_fe56(self: Self) -> bool { /* ... */ }
  ```
  Whether Fe-57 is replaced by Fe-56.

- ```rust
  pub fn loads_rod_metal(self: Self) -> bool { /* ... */ }
  ```
  Whether any rod-metal tape is loaded.

- ```rust
  pub fn from_knobs(ni_as_fe: bool, fe57_as_fe56: bool, no_withdrawn_rods: bool) -> Self { /* ... */ }
  ```
  The treatment the three legacy environment knobs select.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  One line for logs and diagnostics.

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
    fn clone(self: &Self) -> RodMetalTreatment { /* ... */ }
    ```

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
    fn default() -> RodMetalTreatment { /* ... */ }
    ```

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RodMetalTreatment) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `ThermalScatteringTreatment`

Whether the bound thermal scattering laws are applied.

```rust
pub enum ThermalScatteringTreatment {
    Bound,
    FreeGas,
}
```

##### Variants

###### `Bound`

**Default:** every bound law the library provides (graphite, C-in-SiC,
Si-in-SiC) plus the UO2 laws.

###### `FreeGas`

ABLATION (`OUTRAM_HTR10_NO_SAB`): every nuclide a free gas. A harness
check: in a graphite-moderated core this must be worth a large,
resolved amount.

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
    fn clone(self: &Self) -> ThermalScatteringTreatment { /* ... */ }
    ```

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
    fn default() -> ThermalScatteringTreatment { /* ... */ }
    ```

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermalScatteringTreatment) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `U238Evaluation`

Which U-238 evaluation the VIII.0 arm uses.

```rust
pub enum U238Evaluation {
    Library,
    Jendl33,
}
```

##### Variants

###### `Library`

The library's own. **Default.** (VIII.0 tape `n-092_U_238.endf`.)

###### `Jendl33`

ABLATION (`OUTRAM_HTR10_U238_JENDL`): JENDL-3.3. A different-library
bound on the dominant absorber, never the VII.0 offset.

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
    fn clone(self: &Self) -> U238Evaluation { /* ... */ }
    ```

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
    fn default() -> U238Evaluation { /* ... */ }
    ```

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &U238Evaluation) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `Uo2Laws`

Where the UO2 thermal laws come from.

```rust
pub enum Uo2Laws {
    GeneratedFromLeapr,
    Tapes(std::path::PathBuf),
}
```

##### Variants

###### `GeneratedFromLeapr`

**Default:** generated in-process from the LEAPR decks committed in
`njoy-outram-park-fork` (the VIII.0 evaluation).

###### `Tapes`

Tabulated tapes `tsl-UinUO2.endf` (MAT 76) and `tsl-OinUO2.endf`
(MAT 75) in this directory (`OUTRAM_HTR10_UO2_TAPE_DIR`); how the VII.0
arm gets its own UO2 laws. A tape that fails to load is an error.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::path::PathBuf` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Uo2Laws { /* ... */ }
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
    fn default() -> Uo2Laws { /* ... */ }
    ```

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Uo2Laws) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `Htr10DataConfig`

Everything the nuclide set depends on.

```rust
pub struct Htr10DataConfig {
    pub library: NuclearDataLibrary,
    pub graphite_law: super::materials::GraphiteLaw,
    pub carbon: CarbonTreatment,
    pub coolant: Coolant,
    pub rod_metal: RodMetalTreatment,
    pub thermal: ThermalScatteringTreatment,
    pub u238: U238Evaluation,
    pub uo2_laws: Uo2Laws,
    pub temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `library` | `NuclearDataLibrary` | Evaluated library. |
| `graphite_law` | `super::materials::GraphiteLaw` | Graphite S(a,b) on the VIII.0 path. Ignored by VII.0 (one law only). |
| `carbon` | `CarbonTreatment` | Carbon representation on the VIII.0 path. |
| `coolant` | `Coolant` | Coolant regions: helium or vacuum. |
| `rod_metal` | `RodMetalTreatment` | Rod steel and joint iron. |
| `thermal` | `ThermalScatteringTreatment` | Bound thermal laws on or off. |
| `u238` | `U238Evaluation` | U-238 evaluation (VIII.0 only). |
| `uo2_laws` | `Uo2Laws` | Source of the UO2 thermal laws. |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature every nuclide is reconstructed and broadened at. 300.15 K<br>(27 °C) is the temperature Li, Yu & Wei (2014) and Şeker & Çolak (2003)<br>state. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Htr10DataConfig { /* ... */ }
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
    The correct-physics default: see the module docs.

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Htr10DataConfig) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `DataDir`

A directory of evaluated tapes.

```rust
pub enum DataDir {
    Endf,
    AceSubmoduleEndfB8,
}
```

##### Variants

###### `Endf`

`reference-data/endf/`, tracked in this repository.

###### `AceSubmoduleEndfB8`

`reference-data/ace/endf/endf-b-viii.0/`, in the `ace_and_other_data`
submodule (the nickel tapes since 2026-10-01; MANIFEST.tsv one level
up). `git submodule update --init reference-data/ace`.

##### Implementations

###### Methods

- ```rust
  pub fn path(self: Self) -> PathBuf { /* ... */ }
  ```
  Absolute directory, honouring `OUTRAM_PARK_REFERENCE_DATA_DIR`.

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
    fn clone(self: &Self) -> DataDir { /* ... */ }
    ```

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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DataDir) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `Tape`

One evaluated tape: directory and file name.

```rust
pub struct Tape {
    pub dir: DataDir,
    pub file: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dir` | `DataDir` | Directory. |
| `file` | `&'static str` | File name. |

##### Implementations

###### Methods

- ```rust
  pub const fn endf(file: &'static str) -> Self { /* ... */ }
  ```
  A tape in `reference-data/endf/`.

- ```rust
  pub fn path(self: &Self) -> PathBuf { /* ... */ }
  ```
  Absolute path.

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
    fn clone(self: &Self) -> Tape { /* ... */ }
    ```

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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Tape) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `ThermalLaw`

A bound thermal scattering law a slot is to carry.

```rust
pub enum ThermalLaw {
    Graphite {
        file: &'static str,
        mat: i32,
    },
    CInSiC,
    SiInSiC,
    UInUO2,
    OInUO2,
}
```

##### Variants

###### `Graphite`

Graphite S(a,b) from a tape: VIII.0 `GraphiteLaw` (MAT 30/31/32) or the
VII.0 `tsl-graphite-ENDF7.0.endf` (MAT 31).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `file` | `&'static str` | Tape file in `reference-data/endf/`. |
| `mat` | `i32` | MAT in the tape's control columns. |

###### `CInSiC`

C-in-SiC, VIII.0 `tsl-CinSiC.endf`, MAT 44.

###### `SiInSiC`

Si-in-SiC, VIII.0 `tsl-SiinSiC.endf`, MAT 43.

###### `UInUO2`

U-in-UO2 (LEAPR deck or tape, see [`Uo2Laws`]).

###### `OInUO2`

O-in-UO2 (LEAPR deck or tape, see [`Uo2Laws`]).

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short label for slot names.

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
    fn clone(self: &Self) -> ThermalLaw { /* ... */ }
    ```

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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermalLaw) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `NuclideSlot`

One slot of the nuclide array.

```rust
pub struct NuclideSlot {
    pub name: &'static str,
    pub tape: Tape,
    pub thermal: Option<ThermalLaw>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `&'static str` | Nuclide name passed to the reconstruction (e.g. `"C13"`). |
| `tape` | `Tape` | Tape it is read from. |
| `thermal` | `Option<ThermalLaw>` | Thermal law bound to it, if any (already `None` under the free-gas<br>ablation and for VII.0 SiC). |

##### Implementations

###### Methods

- ```rust
  pub fn label(self: &Self) -> String { /* ... */ }
  ```
  Human-readable label: name and thermal treatment.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NuclideSlot { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &NuclideSlot) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `CoolantNuclides`

Where the coolant's nuclides sit, or that there are none.

```rust
pub enum CoolantNuclides {
    Helium {
        he3: usize,
        he4: usize,
    },
    Vacuum,
}
```

##### Variants

###### `Helium`

Natural helium: He-3 and He-4 slots.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `he3` | `usize` | He-3 slot. |
| `he4` | `usize` | He-4 slot. |

###### `Vacuum`

The vacuum ablation: no coolant nuclides.

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
    fn clone(self: &Self) -> CoolantNuclides { /* ... */ }
    ```

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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoolantNuclides) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `Htr10NuclideLayout`

The complete nuclide plan: every slot in order, and the index tables the
material set reads.

```rust
pub struct Htr10NuclideLayout {
    pub pebble: outram_mc_libs::pebble_beds::htr10::Htr10Nuclides,
    pub coolant: CoolantNuclides,
    pub metal: super::materials::RodMetalNuclides,
    pub rod_metal: RodMetalTreatment,
    pub slots: Vec<NuclideSlot>,
    pub notes: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pebble` | `outram_mc_libs::pebble_beds::htr10::Htr10Nuclides` | Pebble, reflector and B4C slots (carbon as [`CarbonSlot`]s). |
| `coolant` | `CoolantNuclides` | Coolant slots. |
| `metal` | `super::materials::RodMetalNuclides` | Rod-metal slots. Under [`RodMetalTreatment::Simplified`] the Ni (and<br>Fe-57) fields point at Fe slots; under<br>[`RodMetalTreatment::NotModelled`] they point past [`Self::len`]. |
| `rod_metal` | `RodMetalTreatment` | The rod-metal treatment this layout realises. |
| `slots` | `Vec<NuclideSlot>` | The slots, in array order. |
| `notes` | `Vec<String>` | Modelling statements a run must record (mixed-library arms,<br>substitutions, ablations), one line each. |

##### Implementations

###### Methods

- ```rust
  pub fn plan(cfg: &Htr10DataConfig) -> Result<Self, Htr10DataError> { /* ... */ }
  ```
  Decide every slot for `cfg`. Pure: reads nothing from disk.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of slots, i.e. the length of the nuclide array.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the layout has no slots (never, for a planned layout).

- ```rust
  pub fn loads_fe57(self: &Self) -> bool { /* ... */ }
  ```
  Whether the plan loads the Fe-57 tape (blocked by gh:#339 while

- ```rust
  pub fn nuclide_name(self: &Self, idx: usize) -> String { /* ... */ }
  ```
  Label of slot `idx`, or `"unknown"`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Htr10NuclideLayout { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
#### Enum `Htr10DataError`

Why the nuclide set could not be built.

```rust
pub enum Htr10DataError {
    BlockedByGh339,
    InvalidConfig(String),
    MissingTape {
        name: String,
        path: std::path::PathBuf,
    },
    LoadFailed {
        name: String,
        path: std::path::PathBuf,
    },
}
```

##### Variants

###### `BlockedByGh339`

The layout loads Fe-57 and gh:#339 is not fixed.

###### `InvalidConfig`

A configuration that does not exist (e.g. a VIII.0-only ablation on
the VII.0 arm).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `MissingTape`

A tape is not on disk.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Slot or law name. |
| `path` | `std::path::PathBuf` | Path tried. |

###### `LoadFailed`

A tape is present but did not load.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Slot or law name. |
| `path` | `std::path::PathBuf` | Path tried. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Enum `LoadProgress`

One step of [`load_htr10_nuclides_with_progress`], for a caller that shows
progress (Dhoby Ghaut's workbench, gh:#568).

```rust
pub enum LoadProgress {
    Started {
        item: String,
    },
    Finished {
        item: String,
        seconds: f64,
    },
}
```

##### Variants

###### `Started`

A thermal-scattering law or a nuclide is about to be processed.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `item` | `String` | What is being processed (`"graphite S(a,b)"`, `"U235"`). |

###### `Finished`

It finished, after `seconds` of wall time.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `item` | `String` | Same text as the matching [`LoadProgress::Started`]. |
| `seconds` | `f64` | Wall-clock seconds. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> LoadProgress { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &LoadProgress) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `load_htr10_nuclides`

Read every tape `layout` names at `cfg.temperature`, bind the thermal
laws, and record each item (path, timing, success) in `diag`, together with
the layout's modelling notes.

Reconstruction tolerance is NJOY's 1e-3. Each slot is reconstructed
separately, as before (a nuclide carrying a thermal law is a distinct
object).

# Errors

- [`Htr10DataError::BlockedByGh339`] before anything is read, if the layout
  loads Fe-57 and [`FE57_RECONSTRUCTION_FIXED`] is `false`;
- [`Htr10DataError::MissingTape`] / [`Htr10DataError::LoadFailed`] for a
  nuclide or graphite law that is absent or fails. A missing SiC law falls
  back to free gas with a note, as it always has; a UO2 tape that fails is
  an error.

```rust
pub fn load_htr10_nuclides(cfg: &Htr10DataConfig, layout: &Htr10NuclideLayout, diag: &mut outram_mc_libs::run_diagnostics::RunDiagnostics) -> Result<Vec<outram_mc_libs::material::nuclide::Nuclide>, Htr10DataError> { /* ... */ }
```

#### Function `load_htr10_nuclides_with_progress`

[`load_htr10_nuclides`], calling `progress` before and after every
thermal law and every nuclide slot. The processing, its order and its
result are exactly those of [`load_htr10_nuclides`], which is this with a
no-op `progress`.

# Errors

As [`load_htr10_nuclides`].

```rust
pub fn load_htr10_nuclides_with_progress<F: FnMut(LoadProgress)>(cfg: &Htr10DataConfig, layout: &Htr10NuclideLayout, diag: &mut outram_mc_libs::run_diagnostics::RunDiagnostics, progress: F) -> Result<Vec<outram_mc_libs::material::nuclide::Nuclide>, Htr10DataError> { /* ... */ }
```

### Constants and Statics

#### Constant `FE57_RECONSTRUCTION_FIXED`

**Whether ENDF/B-VIII.0 Fe-57 can be reconstructed on this code base.**

`false` until gh:#339 is fixed: reconstructing `n-026_Fe_057-ENDF8.0.endf`
(LRU=1, LRF=7, three particle pairs) exhausted 13.4 GB and was OOM-killed
on 2026-09-26, which can take the whole session down on a 15 GB desktop.
While it is `false`, [`load_htr10_nuclides`] **refuses** any layout that
loads the Fe-57 tape, with [`Htr10DataError::BlockedByGh339`], before it
reads a single tape. It does not swap Fe-57 for anything: the simplified
treatment that does ([`RodMetalTreatment::Simplified`]) must be asked for
by name.

Whoever fixes gh:#339 flips this to `true` in the same change, after
measuring the reconstruction's peak memory.

**FLIPPED to `true` 2026-10-01 (gh:#339 fixed in `njoy-outram-park-fork`).**
The cause was a wrong operand in the ported LINPACK `xdot` (upstream
`samm.f90:6189, 6201-6204`), which mis-inverted every spin group with four
or more coupled channels, plus the missing non-negativity guard of upstream
`reconr.f90:2641-2645`. Fe-57 now reconstructs in 0.10 s at 24 MB peak RSS,
word for word NJOY2016's PENDF
(`njoy-outram-park-fork/tests/reconr_lrf7_threshold_channels_vs_njoy2016.rs`).

```rust
pub const FE57_RECONSTRUCTION_FIXED: bool = true;
```

## Module `materials`

**The HTR-10 material set, in one place.**

# Why this module exists

These materials (eleven until 2026-09-25, `mat::COUNT` since the explicit
reflector) were assembled inline in
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
length is asserted against `mat::COUNT` for that reason.

# Provenance

Compositions: Li et al. (2014) Table 2 for the pebble (via
[`outram_mc_libs::pebble_beds::htr10::fuel_pebble_materials`]);
IAEA-TECDOC-1382 Table 4-3 for every reflector zone, with its p. 242
corrections; TECDOC ~~§ 4.1.2~~ **§ 4.1.1.5 "Control of HTR-10" (printed
pp. 235-236; CORRECTED 2026-10-01, gh:#428 — § 4.1.2 is the benchmark
problem descriptions)** for the control-rod B4C density, steel and iron.
That section gives no B4C isotopics: natural boron is this model's reading,
consistent with MIT's homogenised rod in TECDOC Table 4-36 but not stated in
the specification. IUPAC/CIAAW for atomic weights and isotopic compositions.

# The nuclide slots come from a layout (2026-10-01)

~~[`htr10_material_set`] takes an `Htr10Nuclides` and a
[`RodMetalNuclides`].~~ **CHANGED 2026-10-01:** it takes an
[`Htr10NuclideLayout`] ([`super::data`]), the one object that says which
nuclide sits in which slot. Three maintainer decisions of that date are
carried through it:
- **natural carbon** in every carbon-bearing material, C-12 / C-13 at
  98.93 / 1.07 at.% on ENDF/B-VIII.0, elemental C-nat on VII.0 (gh:#425);
- **helium coolant** in [`mat::HELIUM`], not vacuum (gh:#426);
- **real nickel and iron** in the rod steel, or the simplified Ni -> Fe,
  Fe-57 -> Fe-56 mapping when asked for (gh:#329, gh:#339).

```rust
pub mod materials { /* ... */ }
```

### Modules

## Module `atomic_weight`

Standard atomic weights \[g/mol\], IUPAC/CIAAW (Meija et al., *Pure Appl.
Chem.* 88 (2016) 265-291, Table 1; conventional values where an interval is
given): the seven constituents of the rod steel.

```rust
pub mod atomic_weight { /* ... */ }
```

### Constants and Statics

#### Constant `CR`

Chromium.

```rust
pub const CR: f64 = 51.9961;
```

#### Constant `FE`

Iron.

```rust
pub const FE: f64 = 55.845;
```

#### Constant `NI`

Nickel.

```rust
pub const NI: f64 = 58.6934;
```

#### Constant `SI`

Silicon (conventional value).

```rust
pub const SI: f64 = 28.085;
```

#### Constant `MN`

Manganese.

```rust
pub const MN: f64 = 54.938_044;
```

#### Constant `C`

Carbon (conventional value).

```rust
pub const C: f64 = 12.011;
```

#### Constant `TI`

Titanium.

```rust
pub const TI: f64 = 47.867;
```

#### Constant `B`

Boron (conventional value).

```rust
pub const B: f64 = 10.811;
```

## Module `abundance`

Representative natural isotopic compositions \[atom fraction\], IUPAC/CIAAW
(Meija et al., *Pure Appl. Chem.* 88 (2016) 293-306, Table 1).

```rust
pub mod abundance { /* ... */ }
```

### Constants and Statics

#### Constant `FE`

Fe-54, Fe-56, Fe-57, Fe-58.

```rust
pub const FE: [f64; 4] = _;
```

#### Constant `CR`

Cr-50, Cr-52, Cr-53, Cr-54.

```rust
pub const CR: [f64; 4] = _;
```

#### Constant `NI`

Ni-58, Ni-60, Ni-61, Ni-62, Ni-64.

```rust
pub const NI: [f64; 5] = _;
```

#### Constant `TI`

Ti-46, Ti-47, Ti-48, Ti-49, Ti-50.

```rust
pub const TI: [f64; 5] = _;
```

### Types

#### Enum `GraphiteLaw`

Graphite thermal scattering law, S(α,β), applied to every graphite region
on the ENDF/B-VIII.0 path (VII.0 has one graphite law only).

**Default: [`GraphiteLaw::Reactor30P`]** (maintainer decision 2026-09-27).
The choice rests on density and was not made to match k. HTR-10 graphite is
porous. The reflector is 1.76 g/cm³ ([`super::reflector::REFLECTOR_GRAPHITE_DENSITY`],
TECDOC-1382), against a crystal density of about 2.25 g/cm³, so its
porosity is about **22 %**. Hawari's reactor-graphite laws (Hawari &
Gillette, NDS 118 (2014) 176; ENDF/B-VIII.0) are tabulated at 10 % and 30 %
porosity only, and 30 % is the nearer. Neither is exact, and no law at 22 %
exists to interpolate to.

The "porosity" in these laws is vacancy disorder in the molecular-dynamics
phonon spectrum (atoms removed at random; the coherent elastic part is
kept crystalline). It is **not** a bulk-density scaling. The carbon atom
density of each material is set separately and does not change with this
choice.

**Comparison caveat.** Li, Yu & Wei (2014) used ENDF/B-VII.0, whose only
graphite law is crystalline. Against RMC this default therefore carries a
TSL term as well as the VII-vs-VIII library term. The fast single-seed
worth at n = 25 was +947 ± 483 pcm against crystalline
(`outram-mc-libs/verification_and_validation/htr10_rmc/fast_ablation_2026_09_26.md`).
~~A pooled re-measurement is in progress.~~ **CORRECTED 2026-10-01
(gh:#428):** the pooled re-measurement is complete and recorded in the same
file: **30P − crystalline = +705 ± 86 pcm** (8.2σ; 4 seeds × 1.0 M active
histories per arm, two-ball bed). [`GraphiteLaw::Crystalline`] is the
explicit ablation.

```rust
pub enum GraphiteLaw {
    Crystalline,
    Reactor10P,
    Reactor30P,
}
```

##### Variants

###### `Crystalline`

Ideal crystalline graphite, VIII.0 MAT 30. The like-for-like law for a
VII.0 reference. Ablation only.

###### `Reactor10P`

Hawari reactor graphite, 10 % porosity, VIII.0 MAT 31.

###### `Reactor30P`

Hawari reactor graphite, 30 % porosity, VIII.0 MAT 32. **Default.**

##### Implementations

###### Methods

- ```rust
  pub fn from_name(s: &str) -> Option<Self> { /* ... */ }
  ```
  Parse `crystalline`, `10P` or `30P`, the values accepted by

- ```rust
  pub fn tape(self: Self) -> &'static str { /* ... */ }
  ```
  Tape file name in `reference-data/endf/`.

- ```rust
  pub fn mat(self: Self) -> i32 { /* ... */ }
  ```
  MAT number as it appears in the tape's control columns. The 10P and

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
    fn clone(self: &Self) -> GraphiteLaw { /* ... */ }
    ```

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
    fn default() -> GraphiteLaw { /* ... */ }
    ```

- **DistributionExt**
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GraphiteLaw) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
| `reflector_zone` | `usize` | Which TECDOC Table 4-3 zone's composition fills material slot<br>[`mat::REFLECTOR`].<br><br>~~Stands in for the whole reflector; 22 is the default and the<br>OPTIMISTIC bound (cleanest graphite).~~ **CORRECTED 2026-10-01<br>(gh:#428):** since the explicit reflector and the Fig. 4.10 zone map,<br>`mat::REFLECTOR` fills only the zones TECDOC p. 242 says take zone 22's<br>density once the borings are explicit (the zone-22 list of<br>[`mat::for_zone_mc`]). Every other<br>zone has its own slot. (Under the `OUTRAM_HTR10_NO_ZONE_MAP` ablation it<br>fills every zone but the boronated bricks.) 22 is therefore the model,<br>not a bound; any other value is a sensitivity on those zones only. |
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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
#### Struct `RodMetalNuclides`

**Attributes:**

- `Other("#[allow(missing_docs)]")`

Indices, into the caller's nuclide array, of the nuclides the withdrawn
control rods need beyond the pebble's `Htr10Nuclides`: the sleeve steel
and the iron joints.

Silicon appears here AGAIN, as free gas: the pebble's silicon is bound in
SiC with its own S(alpha, beta), which is wrong for silicon dissolved in
steel. Carbon in steel and in B4C uses the pebble table's free carbon
(`Htr10Nuclides::c_free`, natural C-12 / C-13 since 2026-10-01) for the
same reason.

Under the simplified rod-metal case the Ni (and Fe-57) fields point at Fe
slots; see [`super::data::RodMetalTreatment`].

```rust
pub struct RodMetalNuclides {
    pub fe54: usize,
    pub fe56: usize,
    pub fe57: usize,
    pub fe58: usize,
    pub cr50: usize,
    pub cr52: usize,
    pub cr53: usize,
    pub cr54: usize,
    pub ni58: usize,
    pub ni60: usize,
    pub ni61: usize,
    pub ni62: usize,
    pub ni64: usize,
    pub mn55: usize,
    pub ti46: usize,
    pub ti47: usize,
    pub ti48: usize,
    pub ti49: usize,
    pub ti50: usize,
    pub si28: usize,
    pub si29: usize,
    pub si30: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fe54` | `usize` |  |
| `fe56` | `usize` |  |
| `fe57` | `usize` |  |
| `fe58` | `usize` |  |
| `cr50` | `usize` |  |
| `cr52` | `usize` |  |
| `cr53` | `usize` |  |
| `cr54` | `usize` |  |
| `ni58` | `usize` |  |
| `ni60` | `usize` |  |
| `ni61` | `usize` |  |
| `ni62` | `usize` |  |
| `ni64` | `usize` |  |
| `mn55` | `usize` |  |
| `ti46` | `usize` |  |
| `ti47` | `usize` |  |
| `ti48` | `usize` |  |
| `ti49` | `usize` |  |
| `ti50` | `usize` |  |
| `si28` | `usize` | Free-gas Si-28 (NOT the SiC-bound slot). |
| `si29` | `usize` | Free-gas Si-29. |
| `si30` | `usize` | Free-gas Si-30. |

##### Implementations

###### Methods

- ```rust
  pub const fn contiguous(first: usize) -> Self { /* ... */ }
  ```
  The slots laid out consecutively from `first`, in field order

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
    fn clone(self: &Self) -> RodMetalNuclides { /* ... */ }
    ```

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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RodMetalNuclides) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `ideal_gas_atom_density`

**Attributes:**

- `MustUse { reason: None }`

Atom density of an ideal gas \[atoms/(b cm)\]: `N = p / (k_B T)`.

At 300.15 K and 101.33 kPa this is **2.4452e-5 atoms/(b cm)**. Helium at
one atmosphere is ideal to ~5e-4 (second virial coefficient ~12 cm3/mol),
far below anything an eigenvalue sees.

```rust
pub fn ideal_gas_atom_density(temperature: uom::si::f64::ThermodynamicTemperature, pressure: uom::si::f64::Pressure) -> f64 { /* ... */ }
```

#### Function `htr10_material_set`

**Attributes:**

- `MustUse { reason: None }`

Build the material set, indexed by [`mat`] (`mat::COUNT` materials).

**Signature changed 2026-09-25** to take [`RodMetalNuclides`]: the ten
control rods are now explicit geometry at their withdrawn position, and
their steel sleeves and iron joints need nuclides the pebble set has no
slots for. **Changed again 2026-10-01** to take the whole
[`Htr10NuclideLayout`] (pebble, coolant and rod-metal slots in one), so
the carbon split, the coolant and the rod-metal treatment are decided once,
where the nuclides are.

# Panics

If `reflector_zone` is not listed in TECDOC Table 4-3, or if the assembled
length does not match the index table.

```rust
pub fn htr10_material_set(layout: &super::data::Htr10NuclideLayout, cfg: Htr10MaterialConfig) -> Vec<outram_mc_libs::material::material::Material> { /* ... */ }
```

#### Function `nuclide_name`

**Attributes:**

- `MustUse { reason: None }`

Human-readable name for each nuclide slot, for reporting.

A material component carries an index and nothing else, so anything that
reports a composition needs this to turn the index back into a name.
~~Took `Htr10Nuclides` and named only the eleven pebble slots.~~
**CHANGED 2026-10-01:** reads the layout, so it names every slot (C-13,
helium, rod metals) with the thermal law the layout binds to it.

```rust
pub fn nuclide_name(layout: &super::data::Htr10NuclideLayout, idx: usize) -> String { /* ... */ }
```

### Constants and Statics

#### Constant `B10_OF_NATURAL`

Natural boron is 19.9 at.% B-10; the other 80.1 at.% is B-11, a
non-absorber that is placed anyway (gh:#311) because the reference states
natural boron.

```rust
pub const B10_OF_NATURAL: f64 = 0.199;
```

#### Constant `ROD_METAL_TAPES_ENDF8`

ENDF/B-VIII.0 tapes for the rod-metal nuclides, in
[`RodMetalNuclides::contiguous`] order, as `(name, tape)`.

Every tape is in `reference-data/endf/` except the five **nickel** tapes,
which are in the `reference-data/ace` submodule
(`ace/endf/endf-b-viii.0/`, listed in its `MANIFEST.tsv`) since 2026-10-01:
the maintainer decided on 2026-09-25 that no Ni data is committed to this
repository (gh:#329, ~48 MB). ~~(in `reference-data/endf/`)~~

The silicon tapes are the same files as the SiC slots'; they are loaded a
second time WITHOUT a thermal law. There is no ENDF/B-VII.0 counterpart in
the checkout for the metals, so a VII.0 run takes these VIII.0 tapes for
the rod metal only, and must say so.

```rust
pub const ROD_METAL_TAPES_ENDF8: [(&str, super::data::Tape); 22] = _;
```

#### Constant `HE3_ATOM_FRACTION_OF_NATURAL_HE`

He-3 atom fraction of natural (atmospheric) helium, 1.343e-6.

Source: IUPAC/CIAAW representative isotopic composition of helium,
0.000 001 343(13) He-3 / 0.999 998 657(13) He-4 (Meija et al., *Pure Appl.
Chem.* 88 (2016) 293-306, Table 1). He-3's 5333 b thermal (n,p) makes it
the only part of the coolant that absorbs at all; at this fraction it is
~3e-11 atoms/(b cm).

```rust
pub const HE3_ATOM_FRACTION_OF_NATURAL_HE: f64 = 1.343e-6;
```

#### Constant `HELIUM_PRESSURE_KPA`

Helium coolant pressure \[kPa\]: **101.33 kPa, an ASSUMPTION** (gh:#426).

Şeker & Çolak (2003), NED 222:263, p.267 states atmospheric pressure,
101.33 kPa, for its air case and gives no other pressure; the HTR-10
first-criticality loading was at room temperature (27 °C, as Li and Şeker
state), and neither paper states the helium pressure. The helium case is
therefore taken as atmospheric, and this is stated rather than implied.

```rust
pub const HELIUM_PRESSURE_KPA: f64 = 101.33;
```

#### Constant `ROD_STEEL_DENSITY`

Rod sleeve steel density \[g/cm³\], TECDOC ~~§ 4.1.2~~ § 4.1.1.5, pp. 235-236
(CORRECTED 2026-10-01, gh:#428; *"a density of 7.9g/cm3 is assumed"*).

```rust
pub const ROD_STEEL_DENSITY: f64 = 7.9;
```

#### Constant `ROD_STEEL_WT`

Rod sleeve steel composition \[weight fraction\], TECDOC ~~§ 4.1.2~~
§ 4.1.1.5, pp. 235-236 (CORRECTED 2026-10-01, gh:#428):
Cr 18, Fe 68.1, Ni 10, Si 1, Mn 2, C 0.1, Ti 0.8 (sums to 100 %).

```rust
pub const ROD_STEEL_WT: [(&str, f64); 7] = _;
```

#### Constant `ROD_JOINT_IRON_DENSITY`

Iron atom density of the rod joints and ends \[atoms/(b cm)\], TECDOC
~~§ 4.1.2~~ § 4.1.1.5, pp. 235-236 (CORRECTED 2026-10-01, gh:#428): iron alone,
filling 27.5 mm < R < 55 mm.

```rust
pub const ROD_JOINT_IRON_DENSITY: f64 = 0.04;
```

## Module `plots`

**HTR-10 cross-section plots, drawn with the ported `openmc.Model.plot`**
([`outram_mc_libs::geometry::plot::ModelPlot`]).

NEW WORK (helpers), on top of a verified port: every picture is
`Model.plot`'s own output (0 differing pixels against OpenMC 0.16.1.dev25,
`outram-mc-libs/verification_and_validation/python_plotting_parity/`),
coloured by material, with a legend that lists only the materials the slice
actually contains.

**Every cut is chosen from the BUILT bed, not from constants.** The planes
that "cut across the pebbles" are picked from
[`PebbleBed`](super::bed::PebbleBed)'s ball centres — the same
description the lattice was assembled from — and each [`PlotJob`] records
how many ball centres lie on its plane, so a reader can check the claim:

- [`Htr10Plotter::rz_through_pebbles`]: the `x-z` plane `y = y0`, where `y0`
  is the `y` shared by the most present ball centres;
- [`Htr10Plotter::r_theta_at`]: the `x-y` plane at the ball-centre height
  nearest the requested `z`, when that `z` is inside the pebble column
  (bed, conus or discharge tube); outside it, the requested `z` as given;
- [`Htr10Plotter::pebble_cross_section`]: a plane through the centre of a
  fuel (or dummy) ball on that `x-z` plane;
- [`Htr10Plotter::triso_cross_section`]: a plane through the centre of one
  TRISO particle of that fuel ball, found by locating a kernel.

[`Htr10Plotter::standard_set`] is the whole-core set: R-Z through the
pebbles, and `x-y` at the bed bottom, middle and top, the conus, the
discharge tube (the defuelling chute, holding dummy balls), the top
reflector through the withdrawn rods, and the hot-gas duct, so that the
control-rod, absorber-ball (KLAK), irradiation and coolant borings all show.

```rust
pub mod plots { /* ... */ }
```

### Types

#### Struct `PlotJob`

One plot to emit: a configured [`ModelPlot`], a file stem, and a note of
how its plane was chosen (recorded next to the images).

```rust
pub struct PlotJob {
    pub name: String,
    pub plot: outram_mc_libs::geometry::plot::ModelPlot,
    pub note: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | File stem (`<name>.py`, `<name>.png`). |
| `plot` | `outram_mc_libs::geometry::plot::ModelPlot` | The plot. |
| `note` | `String` | How the plane was chosen, with the number of ball centres on it. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PlotJob { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
#### Struct `Htr10Plotter`

**Draws an assembled HTR-10 core.** Owns the core and the material table
(built with [`htr10_material_set`] and the benchmark configuration, the
same call the eigenvalue example makes; a plot reads only material ids and
names from it).

```rust
pub struct Htr10Plotter {
    pub core: super::core_model::AssembledCore,
    pub materials: Vec<outram_mc_libs::material::material::Material>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `core` | `super::core_model::AssembledCore` | The assembled core ([`super::core_model::assemble_explicit_triso`]). |
| `materials` | `Vec<outram_mc_libs::material::material::Material>` | Material table, indexed as the geometry's cells index it. |

##### Implementations

###### Methods

- ```rust
  pub fn new(core: AssembledCore) -> Self { /* ... */ }
  ```
  Wrap an assembled core. Panics if the core has no ball description

- ```rust
  pub fn pebble_plane_y(self: &Self) -> (f64, usize) { /* ... */ }
  ```
  `y0` of the `x-z` plane holding the most present ball centres (ties:

- ```rust
  pub fn nearest_ball_layer(self: &Self, z: f64) -> Option<(f64, usize)> { /* ... */ }
  ```
  The ball-centre height nearest `z` and how many centres sit at it, or

- ```rust
  pub fn ball_on_plane(self: &Self, fuel: bool, z: f64) -> Option<[f64; 3]> { /* ... */ }
  ```
  A present ball of the requested kind on the pebble plane, nearest the

- ```rust
  pub fn triso_centre(self: &Self, peb: [f64; 3]) -> Option<Position> { /* ... */ }
  ```
  Centre of one TRISO particle of the fuel ball centred at `peb`: scan a

- ```rust
  pub fn rz_through_pebbles(self: &Self, cm_per_px: f64) -> PlotJob { /* ... */ }
  ```
  Whole-model `x-z` (R-Z) slice on the plane through the most ball centres.

- ```rust
  pub fn rz_lower_column(self: &Self, cm_per_px: f64) -> PlotJob { /* ... */ }
  ```
  Zoomed `x-z` slice of the lower bed, conus and discharge tube on the

- ```rust
  pub fn r_theta_at(self: &Self, name: &str, what: &str, z: f64, cm_per_px: f64) -> PlotJob { /* ... */ }
  ```
  Whole-model `x-y` (r-theta) slice at height `z`, snapped to the nearest

- ```rust
  pub fn pebble_cross_section(self: &Self, fuel: bool, cm_per_px: f64) -> Option<PlotJob> { /* ... */ }
  ```
  `x-y` slice through the centre of a fuel (or dummy) ball on the pebble

- ```rust
  pub fn triso_cross_section(self: &Self, name: &str, width: f64, cm_per_px: f64) -> Option<PlotJob> { /* ... */ }
  ```
  `x-y` slice through the centre of one TRISO particle, `width` cm wide.

- ```rust
  pub fn standard_set(self: &Self) -> Vec<PlotJob> { /* ... */ }
  ```
  The whole set: TRISO, pebbles, R-Z, and `x-y` at the heights that show

- ```rust
  pub fn script(self: &Self, job: &PlotJob) -> Result<String, ModelPlotError> { /* ... */ }
  ```
  Emit a job's script, with the legend restricted to the materials present

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

#### Function `palette`

Material palette, indexed by [`mat`]. Chosen so the TRISO layers read as a
warm-to-cool sequence from the kernel out and the graphites stay grey/brown.

```rust
pub fn palette() -> Vec<(outram_mc_libs::geometry::plot::Rgb, &'static str)> { /* ... */ }
```

## Module `keff_vs_height`

**HTR-10 `k_eff` against loading height: the shared run machinery and
the record it writes** (gh:#501, 2026-10-02).

Orchestration only — no physics is implemented here. One HTR-10 core
(Şeker & Çolak 2003's 13-ball bed, `N` layers, gh:#472) is transported
with `outram_mc_libs`' hybrid delta/surface tracker, and the result is
compared with Li, Yu & Wei (2014)'s RMC curve and the paper's two MCNP
columns, each read at the height where Şeker's model holds as many balls
as the built bed ([`super::seker_height_for_balls`]).

Before this module, every HTR-10 example carried its own copy of the
majorant grid, the fissile source box, the entropy mesh and the reference
interpolation. Those copies now call:

- [`bed_majorant`] — the region-local majorant over the bed's materials;
- [`fissile_source_box`] / [`fissile_entropy_mesh`] — both span the WHOLE
  fissile region, conus floor included (a mesh blind to part of the core
  reports convergence of the part it can see);
- [`run_core`] — one k-eigenvalue run with a pinned thread count, returned
  as a [`HeightPoint`] plus the raw [`KeffResult`];
- [`script`] — the deterministic standalone matplotlib script, the
  markdown results table and the reconstruction parameters.

The reference curves are taken from [`super::RMC_KEFF_VS_HEIGHT`],
[`super::MCNP_TABLE3_KEFF_VS_HEIGHT`] and
[`super::MCNP_TABLE4_KEFF_VS_HEIGHT`] directly, never retyped.

Heights cross the public API as `uom` [`Length`]s; `k_eff` and its
standard deviation are dimensionless `f64`.

```rust
pub mod keff_vs_height { /* ... */ }
```

### Modules

## Module `script`

**The k-vs-height record: a standalone matplotlib script, a markdown
results table, and a reconstruction block** (gh:#501).

Like `outram_mc_libs::geometry::plot::ModelPlot` and
[`Htr10Plotter::script`](crate::htr10_rmc::plots::Htr10Plotter::script),
the figure is not drawn in Rust: [`plot_script`] writes a Python script
that needs only matplotlib, with **every number embedded**, so the figure
can be redrawn or restyled years later without the run.

**Deterministic.** [`plot_script`] is a pure function of the points and
the [`SweepProvenance`]; every float is printed at a fixed precision, and
no timing goes into it. The same run gives a byte-identical script.
Timings live in [`results_table_md`], next to the hardware they were
measured on.

**Style** (maintainer convention, 2026-10-01): published curves are THICK
SOLID lines; this project's calculation is markers with ±1σ bars joined by
a thin dotted line ("guesses"); horizontal guides (k = 1, 0 pcm, the
±500/±1000 pcm band edges) are dashed. Every series names its source in
the legend, and the footer carries the key.

```rust
pub mod script { /* ... */ }
```

### Types

#### Struct `SweepProvenance`

Who ran what, where: everything the footer and the parameters block need
that is not a result. Supplied by the caller (an example reads the git
commit and the host); kept here as plain strings so the emitters stay
pure.

```rust
pub struct SweepProvenance {
    pub example: String,
    pub library: String,
    pub statistics: super::SweepStatistics,
    pub threads: usize,
    pub commit: String,
    pub ace_commit: String,
    pub host: String,
    pub cpu_affinity: String,
    pub model_notes: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `example` | `String` | The example that produced the run, e.g. `htr10_endf8_kvsh_quick`. |
| `library` | `String` | Library label, e.g. `ENDF/B-VIII.0`. |
| `statistics` | `super::SweepStatistics` | The transport statistics. |
| `threads` | `usize` | Pinned transport threads per run. |
| `commit` | `String` | Workspace commit the binary was built from (`-dirty` if the tracked<br>tree differed from it). |
| `ace_commit` | `String` | Commit of the `reference-data/ace` submodule. |
| `host` | `String` | Hardware headline (CPU model, cores, RAM, OS). |
| `cpu_affinity` | `String` | CPUs the process was allowed to run on (`Cpus_allowed_list`). |
| `model_notes` | `Vec<String>` | Model and data configuration, one line each, as the run printed it. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SweepProvenance { /* ... */ }
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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SweepProvenance) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `plot_script`

**Attributes:**

- `MustUse { reason: None }`

The standalone matplotlib script. Usage of the result:
`python3 <script> [out.png]` (default: the script's own name with `.png`).

Draws three panels sharing the height axis: `k_eff` with RMC and both MCNP
columns; `k − RMC` \[pcm\]; `k − MCNP T3` and `k − MCNP T4` \[pcm\]; each
residual with this model's within-run 1σ (the references quote none).

```rust
pub fn plot_script(points: &[super::HeightPoint], prov: &SweepProvenance) -> String { /* ... */ }
```

#### Function `results_table_md`

**Attributes:**

- `MustUse { reason: None }`

The markdown results table: one row per height, then a summary of the
residuals against each reference, then a timing table with the hardware.

```rust
pub fn results_table_md(points: &[super::HeightPoint], prov: &SweepProvenance) -> String { /* ... */ }
```

#### Function `run_parameters_md`

**Attributes:**

- `MustUse { reason: None }`

The reconstruction block: everything needed to rebuild the run.

```rust
pub fn run_parameters_md(prov: &SweepProvenance, layers: &[usize]) -> String { /* ... */ }
```

### Types

#### Struct `SweepStatistics`

Particles per cycle, cycles and seed of one sweep. Fixed per example: the
committed source is the specification of the run.

```rust
pub struct SweepStatistics {
    pub name: &'static str,
    pub particles: usize,
    pub inactive: usize,
    pub active: usize,
    pub seed: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `&'static str` | Short name, used in file names and the plot title. |
| `particles` | `usize` | Particles per cycle. |
| `inactive` | `usize` | Inactive (discarded) cycles. |
| `active` | `usize` | Active cycles. |
| `seed` | `u64` | RNG seed (one seed per point). |

##### Implementations

###### Methods

- ```rust
  pub fn cycles(self: &Self) -> usize { /* ... */ }
  ```
  Total cycles.

- ```rust
  pub fn planned_histories(self: &Self) -> u64 { /* ... */ }
  ```
  Histories the run should simulate, inactive cycles included.

- ```rust
  pub fn keff_settings(self: &Self, threads: usize) -> KeffSettings { /* ... */ }
  ```
  The transport settings for this sweep on `threads` pinned threads.

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
    fn clone(self: &Self) -> SweepStatistics { /* ... */ }
    ```

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

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SweepStatistics) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Struct `HeightPoint`

One height of a sweep: the built core, its result, and the three
reference values at equal ball count.

```rust
pub struct HeightPoint {
    pub layers: usize,
    pub balls: Option<usize>,
    pub built_height: uom::si::f64::Length,
    pub reference_height: uom::si::f64::Length,
    pub k: f64,
    pub sigma: f64,
    pub rmc: Option<f64>,
    pub mcnp_t3: Option<f64>,
    pub mcnp_t4: Option<f64>,
    pub histories: u64,
    pub lost_locate: u64,
    pub entropy_first_last: Option<(f64, f64)>,
    pub transport_time: uom::si::f64::Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `layers` | `usize` | Şeker layers N. |
| `balls` | `Option<usize>` | Balls in the built bed (`None` only for a bed that does not count them;<br>Şeker's bed always does). |
| `built_height` | `uom::si::f64::Length` | Built bed height, `2 × bed_half_height` (`9.798 N + 6` cm). |
| `reference_height` | `uom::si::f64::Length` | Height at which Şeker's model holds `balls` balls; every reference is<br>read here (gh:#472). Equal to `built_height` if `balls` is `None`. |
| `k` | `f64` | `k_eff`, mean over active cycles. |
| `sigma` | `f64` | Within-run 1σ of `k` (single seed; seed-to-seed scatter is not in it). |
| `rmc` | `Option<f64>` | RMC (Li, Yu & Wei 2014) at `reference_height`; `None` outside its table. |
| `mcnp_t3` | `Option<f64>` | MCNP Table 3 (Şeker vacuum column) at `reference_height`; gauge only. |
| `mcnp_t4` | `Option<f64>` | MCNP Table 4 (Şeker helium column) at `reference_height`; gauge only. |
| `histories` | `u64` | Histories simulated, inactive cycles included. |
| `lost_locate` | `u64` | Histories whose position could not be located in any cell. |
| `entropy_first_last` | `Option<(f64, f64)>` | Shannon entropy \[bits\] of the first and last cycle's source. |
| `transport_time` | `uom::si::f64::Time` | Wall-clock time of the transport call alone. |

##### Implementations

###### Methods

- ```rust
  pub fn built_height_cm(self: &Self) -> f64 { /* ... */ }
  ```
  Built height in cm.

- ```rust
  pub fn reference_height_cm(self: &Self) -> f64 { /* ... */ }
  ```
  Reference height in cm.

- ```rust
  pub fn residual_pcm(self: &Self, reference: Option<f64>) -> Option<f64> { /* ... */ }
  ```
  `(k − reference) × 1e5` \[pcm\], or `None` when there is no reference.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> HeightPoint { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &HeightPoint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

#### Function `majorant_energy_grid`

**Attributes:**

- `MustUse { reason: None }`

The 4096-point log energy grid, 1e-4 eV to 20 MeV, on which the bed
majorant is tabulated.

```rust
pub fn majorant_energy_grid() -> Vec<f64> { /* ... */ }
```

#### Function `bed_majorant`

**Attributes:**

- `MustUse { reason: None }`

The region-local majorant over the BED's materials (pebble layers and
coolant, `0..=mat::HELIUM`), with a 0.3 safety margin. The reflector is
surface-tracked, so it must not raise the bed's tracking cost. It depends
on the materials only, so a sweep builds it once for every height.

```rust
pub fn bed_majorant(mats: &[outram_mc_libs::material::material::Material], nucs: &[outram_mc_libs::material::nuclide::Nuclide]) -> outram_mc_libs::pebble_beds::delta_tracking::Majorant { /* ... */ }
```

#### Function `fissile_source_box`

**Attributes:**

- `MustUse { reason: None }`

The initial-source box: the whole fissile region, from the conus floor
to the bed top, full bed radius.

```rust
pub fn fissile_source_box(core: &super::core_model::AssembledCore) -> outram_mc_libs::physics::transport_csg::SourceBox { /* ... */ }
```

#### Function `fissile_entropy_mesh`

**Attributes:**

- `MustUse { reason: None }`

The 4 × 4 × 4 Shannon-entropy mesh over the same region as
[`fissile_source_box`]. Its ceiling is 6 bits.

```rust
pub fn fissile_entropy_mesh(core: &super::core_model::AssembledCore) -> outram_mc_libs::tally::mesh::RegularMesh { /* ... */ }
```

#### Function `run_core`

**Attributes:**

- `MustUse { reason: None }`

Transport one assembled core and compare it with the references.

`layers` is the Şeker layer count the core was built with (it is
recorded, not used to build). The majorant is the caller's, built once by
[`bed_majorant`]; the source box and entropy mesh come from the core.

```rust
pub fn run_core(layers: usize, core: &super::core_model::AssembledCore, mats: &[outram_mc_libs::material::material::Material], nucs: &[outram_mc_libs::material::nuclide::Nuclide], majorant: &outram_mc_libs::pebble_beds::delta_tracking::Majorant, stats: &SweepStatistics, threads: usize) -> (HeightPoint, outram_mc_libs::physics::keff::KeffResult) { /* ... */ }
```

### Constants and Statics

#### Constant `TEMPERATURE_K`

Temperature \[K\] of every material, 27 °C, as Li, Yu & Wei (2014) and
Şeker & Çolak (2003) state.

```rust
pub const TEMPERATURE_K: f64 = 300.15;
```

#### Constant `RINGS`

Radial ring count of the full-radius bed (the bed radius is always the
physical 90 cm; the ring count is a floor on the tiling).

```rust
pub const RINGS: usize = 14;
```

#### Constant `SWEEP_LAYERS`

The Şeker layer counts a k-vs-height sweep runs: N = 10 … 20.

N = 9 (94.182 cm) is not run: at equal ball count it reads the reference
at 92.9 cm, below RMC's lowest tabulated point, so there is nothing to
compare with short of extrapolating.

```rust
pub const SWEEP_LAYERS: [usize; 11] = _;
```

## Module `table1`

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

#### Function `seker_height_for_balls`

**Attributes:**

- `MustUse { reason: None }`

**The reference loading height \[cm\] that holds `balls` balls** in Şeker &
Çolak (2003)'s model: `9.798 (balls - 733) / 1346 + 6.0`. It is exact at
the tabulated rows and linear between them.

# Why a model is matched to the reference by ball count (gh:#472, 2026-10-01)

Our Şeker bed keeps every ball whole. It holds 721 + 609 balls per layer;
Şeker's table implies 733 + 613. The difference was chased down: it is
exactly the next shells out, 12 basal balls centred at ρ = 87.209 cm and 6
central balls at ρ = 87.080 cm, which would cross the r = 90 cm reflector
by 0.21 and 0.08 cm. Keeping them reproduces Şeker's basal count exactly
(733) and the central one to +2 (615). No lattice offset does it under the
whole-ball rule. So the reference keeps balls that cross the wall: cut by
the core cylinder in MCNP, or kept by a tolerance.

Cut pebbles are wrong physics (maintainer, 2026-10-01), so they are not
added. Instead the maintainer chose to compare **at the same ball
inventory**: the reference at the height where its model holds as many
balls as ours. At N = 12 ours holds 16 681 balls, which in Şeker's model is
122.09 cm, 1.49 cm below the 123.576 cm row.

```rust
pub fn seker_height_for_balls(balls: usize) -> f64 { /* ... */ }
```

#### Function `rmc_keff_at_ball_count`

**Attributes:**

- `MustUse { reason: None }`

RMC's `k_eff` (Li, Yu & Wei 2014) for a bed of `balls` balls: the
reference curve [`RMC_KEFF_VS_HEIGHT`] read at [`seker_height_for_balls`],
linear between rows. `None` outside the tabulated range (no
extrapolation). This is the comparison point for a whole-ball model; see
[`seker_height_for_balls`] for why.

```rust
pub fn rmc_keff_at_ball_count(balls: usize) -> Option<f64> { /* ... */ }
```

#### Function `keff_curve_at_height`

**Attributes:**

- `MustUse { reason: None }`

A tabulated `(height_cm, k_eff)` curve — [`RMC_KEFF_VS_HEIGHT`],
[`MCNP_TABLE3_KEFF_VS_HEIGHT`] or [`MCNP_TABLE4_KEFF_VS_HEIGHT`] — read at
`h_cm`, linear between rows. `None` outside the tabulated range (1e-6 cm
tolerance at the ends): past the ends the curves flatten and a linear
extension would invent reactivity, so nothing is extrapolated.

The one interpolation every HTR-10 comparison uses (added 2026-10-02,
gh:#501, replacing per-example copies). Read a curve at the height where
Şeker's model holds the built bed's ball count
([`seker_height_for_balls`]), not at the built height (gh:#472).

```rust
pub fn keff_curve_at_height(curve: &[(f64, f64)], h_cm: f64) -> Option<f64> { /* ... */ }
```

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

One curve, not two — see the module docs on the duplicated column. ~~The MCNP
columns are deliberately **not** carried here: they are a second code's
results on a third model, and mixing them in would invite a comparison that
is not ours to make.~~ **CHANGED 2026-09-27 (maintainer direction: "save
mcnp data too since it's there, just so we have a rough gauge"):** they are
now carried separately, in [`MCNP_TABLE3_KEFF_VS_HEIGHT`] and
[`MCNP_TABLE4_KEFF_VS_HEIGHT`]. RMC stays the reference.

Heights are the paper's convention: bottom of the lowest ball to top of the
highest, `9.798 N + 6.0` cm (gh:#333).

**The inventory behind each height (2026-10-01).** These are exactly the
heights of Şeker & Çolak (2003), NED 222:263, Table 3 (the MCNP model Li
follows), which also gives the ball counts: `1346 N + 733` balls, of which
`767 N + 418` are fuel, for N = 9…20. At a 0.61 filling fraction that is
**H − 0.55 cm** of volume-equivalent bed (0.58 cm at N = 9, 0.48 cm at
N = 20). Match a model to a row **by ball count**, not by height. The
experiment's first criticality was 16 890 balls at 123.06 cm; Şeker's
N = 12 row holds 16 885. Whether Li's RMC model holds the same count is not
stated.

```rust
pub const RMC_KEFF_VS_HEIGHT: &[(f64, f64)] = _;
```

#### Constant `MCNP_TABLE3_KEFF_VS_HEIGHT`

MCNP `k_eff` against loading height as printed in the paper's **Table 3**
(captioned "Critical result 1 (vacuum)"), `(height_cm, k_eff)`.

**A rough gauge, not a reference.** Li, Yu & Wei describe these as the
results *"of MCNP reported in paper listed in reference"*, i.e. MCNP on a
different, independently built model (*"model used in this calculation is
constructed relatively independently"*), with no uncertainty quoted. Use
them to see how far two codes on two models already spread, which bounds
how much agreement with RMC alone can mean: the paper's own RMC-MCNP
differences reach 0.95 %. Do not fit to them and do not replace RMC with
them.

**Their data library (2026-10-01, gh:#428).** Li's abstract says RMC and
MCNP both used *"continuous energy cross section based on ENDF/B-7.0"*. The
MCNP numbers are not Li's own runs, though. They equal Şeker & Çolak
(2003)'s Table 3 (see the module docs), and Şeker p.265 states:
*"ENDF/B-VI continuous energy cross sections are used in calculations for
all materials except graphite. Cross sections for graphite are taken from
TMCCS library."* So these MCNP columns are **ENDF/B-VI**, with TMCCS
graphite, and not VII.0. Only RMC's library is the VII.0 Li states.

Both Tables 3 and 4 are captioned "(vacuum)" while the text says the
calculations were for vacuum *and* helium, so one caption is wrong ~~and it
is not known which table is which~~ **CORRECTED 2026-10-01**: Şeker &
Çolak (2003) Table 3 shows Table 3 is vacuum and Table 4 is **helium**
(see the module docs). They are kept under their table numbers. Transcribed 2026-09-27;
`the_mcnp_columns_reproduce_the_papers_relative_differences` re-derives the
paper's "Re-diff" column from them.

```rust
pub const MCNP_TABLE3_KEFF_VS_HEIGHT: &[(f64, f64)] = _;
```

#### Constant `MCNP_TABLE4_KEFF_VS_HEIGHT`

MCNP `k_eff` against loading height from the paper's **Table 4** (also
captioned "(vacuum)"; see [`MCNP_TABLE3_KEFF_VS_HEIGHT`] for what these are
and are not).

```rust
pub const MCNP_TABLE4_KEFF_VS_HEIGHT: &[(f64, f64)] = _;
```

#### Constant `SEKER_BALLS_PER_LAYER`

Design characteristics from the paper's **Table 1**.
Balls per 9.798 cm layer in Şeker & Çolak (2003)'s HTR-10 model, Table 3:
every row holds `1346 N + 733` balls (gh:#333, gh:#472).

Source: Şeker, V., Çolak, Ü. (2003), *HTR-10 full core first criticality
analysis with MCNP*, Nucl. Eng. Des. 222, 263–270,
doi:10.1016/S0029-5493(03)00031-1, Table 3. Li, Yu & Wei (2014) tabulate
RMC at exactly these heights.

```rust
pub const SEKER_BALLS_PER_LAYER: usize = 1346;
```

#### Constant `SEKER_BALLS_EXTRA_PLANE`

The constant in Şeker's `1346 N + 733`: one extra basal plane. See
[`SEKER_BALLS_PER_LAYER`].

```rust
pub const SEKER_BALLS_EXTRA_PLANE: usize = 733;
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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

~~`outram-mc` takes **one** temperature for the whole problem
(`KeffSettings::temperature_k`); it is not per-material.~~ **CORRECTED
2026-09-30 (GitHub #313):** the CSG transport reads each material's
temperature (and each pointwise nuclide's data temperature), not the run
temperature. This loop sets every material to the one fuel temperature.
So the feedback this loop applies is a single fuel temperature applied
everywhere, which is
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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
## Module `blender_bridge`

outram-blender -> outram-mc CSG bridge (`to_mc_geometry`). Moved here from
outram-blender's retired `mc-export` feature on 2026-10-02 (GitHub #486).
**outram-blender -> outram-mc geometry bridge.**

[`to_mc_geometry`] fits an authored surface [`Mesh`] to analytic CSG and
returns it as outram-mc-libs' CSG geometry. ~~It maps the fit onto
outram-mc-libs' CSG types~~ Since #486 stage 6 (2026-10-02) the types are
the same (outram-blender's `csg`, re-exported by outram-mc-libs) and it
delegates to `outram_blender::export::to_csg_geometry`. It lived in outram-blender behind the
`mc-export` feature until 2026-10-02, when GitHub issue #486 made
outram-mc-libs depend on outram-blender for its geometry description: a
blender -> outram-mc edge, even an optional one, would then be a cycle, so
the bridge moved up to this coupling crate, which depends on both.

```rust
pub mod blender_bridge { /* ... */ }
```

### Functions

#### Function `to_mc_geometry`

Convert `mesh` to an `outram-mc-libs` CSG `Geometry` — the Monte Carlo
export bridge.

**Since 2026-10-02 (GitHub #486, plan stage 6) a thin wrapper over
[`outram_blender::export::to_csg_geometry`]**: the CSG types moved into
outram-blender, and outram-mc-libs' `Geometry` *is*
`outram_blender::csg::geometry::Geometry` (re-exported), so no mapping is
left to do here. Kept so existing callers (`nee_soon::sim`, MC Studio)
keep their name. See that function for the surface and region mapping,
the `Void` placeholder fill and the error case.

# Errors

[`ExportError::NotImplemented`] for a mesh that is not a fittable convex
primitive (propagated from [`outram_blender::export::to_csg_primitive`]).

```rust
pub fn to_mc_geometry(mesh: &outram_blender::mesh::Mesh) -> Result<outram_mc_libs::prelude::Geometry, outram_blender::export::ExportError> { /* ... */ }
```

## Module `sim`

Monte Carlo setup + run driver behind the MC Studio GUI. Moved here from
`outram_blender::sim` on 2026-10-02 (GitHub #486).
Monte Carlo **simulation setup + run** — the backend the **MC Studio** egui
app (`dhoby-ghaut`'s `examples/mc_studio`) drives.

~~outram-blender `sim` (feature `mc-export`)~~ **MOVED here 2026-10-02**
(GitHub #486): outram-mc-libs takes its CSG description from
outram-blender, so outram-blender may not depend on outram-mc-libs, and
this driver, which needs both, sits in the coupling crate.

[`crate::blender_bridge`] turns authored geometry into an `outram-mc-libs`
[`Geometry`]. This module is the rest of "set up and run a *basic* Monte
Carlo simulation": build **materials** from a friendly nuclide-name + density
spec, bundle geometry + materials + source + run settings into an
[`McSimSetup`], and [`McSimSetup::run`] a **k-eigenvalue** (criticality)
calculation, returning `k_eff ± σ`.

Scope is deliberately "basic": `outram-mc-libs` implements a **k-eigenvalue**
solver only (no fixed-source), so this backend targets criticality problems —
a bare homogeneous sphere (the validated `run_keff` driver) or a general CSG
model authored in outram-blender (`run_keff_csg`, with an optional cell/flux
tally). Cross sections come from the embedded offline data
([`Nuclide::from_core`]) — no downloads, no HDF5.

> Offline demonstration / education / research / V&V only, per the workspace
> `RESPONSIBLE_USE.md` — **not** for reactor operation, licensing, or
> safety-critical decisions.

# Recipe

```no_run
use outram_blender::primitives::uv_sphere;
use nee_soon::sim::{MaterialSpec, McSimSetup, SimGeometry, atom_density};
use nee_soon::sim::KeffSettings;

// 1. A material: Godiva HEU by atom density [atoms/barn·cm].
let heu = MaterialSpec {
    name: "Godiva HEU".into(),
    temperature_k: 293.6,
    nuclides: vec![
        ("U234".into(), 4.9184e-4),
        ("U235".into(), 4.4994e-2),
        ("U238".into(), 2.4984e-3),
    ],
};

// 2. Set up and run a bare-sphere criticality at the Godiva radius.
let sim = McSimSetup {
    geometry: SimGeometry::BareSphere { radius_cm: 8.7407 },
    materials: vec![heu],
    settings: KeffSettings { n_particles: 2000, n_inactive: 20, n_active: 50, ..Default::default() },
};
let result = sim.run().unwrap();
println!("k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
```

```rust
pub mod sim { /* ... */ }
```

### Types

#### Struct `MaterialSpec`

A material specification in friendly terms: a name, a temperature, and its
nuclides as `(name, atom density)` pairs.

`atom density` is in **atoms/barn·cm** (the unit `outram-mc-libs` materials
use). Use [`atom_density`] to convert from a mass density if that is what you
have. Nuclide names are the `njoy-outram-park-fork` CORE-library keys, e.g.
`"U235"`, `"U238"`, `"H1"`, `"O16"`.

```rust
pub struct MaterialSpec {
    pub name: String,
    pub temperature_k: f64,
    pub nuclides: Vec<(String, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable material name (e.g. `"Godiva HEU"`, `"UO2 fuel"`). |
| `temperature_k` | `f64` | Temperature [K], fed to the Doppler-broadened cross-section lookup. |
| `nuclides` | `Vec<(String, f64)>` | `(nuclide name, atom density [atoms/barn·cm])` pairs. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MaterialSpec { /* ... */ }
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
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &MaterialSpec) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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
#### Enum `SimError`

Errors from building or running a [`McSimSetup`].

```rust
pub enum SimError {
    Nuclide(String),
    NoMaterial,
    BadMaterialIndex(usize),
    Export(outram_blender::export::ExportError),
}
```

##### Variants

###### `Nuclide`

A nuclide name was not found in the embedded CORE data library.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NoMaterial`

The setup has no materials (need at least one).

###### `BadMaterialIndex`

A `material_idx` referenced a material that does not exist.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Export`

The mesh could not be exported to `outram-mc-libs` geometry.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_blender::export::ExportError` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
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
    fn from(e: ExportError) -> Self { /* ... */ }
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
#### Enum `SimGeometry`

What geometry a [`McSimSetup`] runs on.

```rust
pub enum SimGeometry {
    BareSphere {
        radius_cm: f64,
    },
    Csg {
        geometry: outram_mc_libs::prelude::Geometry,
        source: SourceBox,
    },
}
```

##### Variants

###### `BareSphere`

A bare homogeneous sphere of radius `radius_cm` [cm], vacuum outside — the
validated `run_keff` driver. Uses the setup's **first** material.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `radius_cm` | `f64` | Sphere radius [cm]. |

###### `Csg`

A general CSG model (authored geometry via [`csg_from_mesh`], or built by
hand) run with `run_keff_csg`. `source` bounds the initial fission-source
sampling box and must overlap the fissile region.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `outram_mc_libs::prelude::Geometry` | The `outram-mc-libs` CSG geometry (cells reference material indices). |
| `source` | `SourceBox` | Initial source sampling box [cm]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
#### Struct `McSimSetup`

A complete **k-eigenvalue** simulation: geometry + materials + run settings.
The GUI (or any caller) builds one of these and calls [`McSimSetup::run`].

```rust
pub struct McSimSetup {
    pub geometry: SimGeometry,
    pub materials: Vec<MaterialSpec>,
    pub settings: KeffSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `SimGeometry` | What to run on. |
| `materials` | `Vec<MaterialSpec>` | Material specs. For [`SimGeometry::BareSphere`], the first is used; for<br>[`SimGeometry::Csg`], cells index into this list (spec order = index). |
| `settings` | `KeffSettings` | Power-iteration settings (histories/generation, inactive/active counts,<br>seed, compute backend). [`KeffSettings::default`] is a sensible start. |

##### Implementations

###### Methods

- ```rust
  pub fn run(self: &Self) -> Result<KeffResult, SimError> { /* ... */ }
  ```
  Run the criticality calculation, returning `k_eff` (mean ± standard error)

- ```rust
  pub fn run_with_tally(self: &Self, tally: &mut Tally) -> Result<KeffResult, SimError> { /* ... */ }
  ```
  Run with a track-length [`Tally`] attached (CSG geometry only; the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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

#### Function `atom_density`

Number density of a nuclide from a **mass density**.

`N = ρ · w · N_A / M`, converted to **atoms/barn·cm** (the `1e-24` barn
factor). `mass_density_g_cc` is ρ [g/cm³], `molar_mass_g_mol` is the nuclide's
molar mass M [g/mol], `weight_fraction` is its mass fraction `w` in the
material (`1.0` for a pure element/isotope). `outram-mc-libs` has no such
helper, so this fills the gap for GUI/user input given in g/cm³.

# Example
```
use nee_soon::sim::atom_density;
// Pure U-235 metal at 18.74 g/cm³, M ≈ 235.04 g/mol → ~4.8e-2 atoms/b·cm.
let n = atom_density(18.74, 235.04, 1.0);
assert!((n - 4.8e-2).abs() < 2e-3);
```

```rust
pub fn atom_density(mass_density_g_cc: f64, molar_mass_g_mol: f64, weight_fraction: f64) -> f64 { /* ... */ }
```

#### Function `build_materials`

Build the `(nuclides, materials)` arrays `outram-mc-libs` needs from a list of
[`MaterialSpec`]s.

Nuclide names are deduplicated into one shared `Vec<Nuclide>` (cross sections
loaded once from the embedded CORE library); each returned [`Material`]
references them by index, matching the `outram-mc-libs` convention. Material
ids are assigned `1, 2, …` in spec order.

```rust
pub fn build_materials(specs: &[MaterialSpec]) -> Result<(Vec<outram_mc_libs::material::nuclide::Nuclide>, Vec<outram_mc_libs::material::material::Material>), SimError> { /* ... */ }
```

#### Function `csg_from_mesh`

Build a [`SimGeometry::Csg`] from an authored [`Mesh`], assigning
`material_idx` (an index into a [`McSimSetup::materials`] list) to its cell.

The mesh is exported via [`to_mc_geometry`] (box / sphere / Z-cylinder /
convex-faceted), the single placeholder `Void` cell is filled with the chosen
material, and an initial source box is fitted to the mesh's vertex bounds
(inflated slightly so it overlaps the body). Returns [`SimError::Export`] for
a mesh that is not a fittable primitive (e.g. a non-convex solid).

```rust
pub fn csg_from_mesh(mesh: &outram_blender::mesh::Mesh, material_idx: usize) -> Result<SimGeometry, SimError> { /* ... */ }
```

#### Function `cell_flux_tally`

Build a track-length **cell tally** scoring flux and ν-fission over the given
cell indices, ready to attach via [`McSimSetup::run_with_tally`]. Bins are
pre-sized to `cell_indices.len() × 2` and laid out `[cell * 2 + score]`
(score 0 = flux, 1 = ν-fission). After the run, read a bin with
[`tally_value`].

```rust
pub fn cell_flux_tally(cell_indices: Vec<usize>) -> outram_mc_libs::prelude::Tally { /* ... */ }
```

#### Function `tally_value`

Read one bin of a tally built by [`cell_flux_tally`] as `(mean, rel_std_dev)`
over `n_active` generations. `cell` indexes the tally's cell list; `score` is
`0` (flux) or `1` (ν-fission). Returns `None` if the bin index is out of range.

```rust
pub fn tally_value(tally: &outram_mc_libs::prelude::Tally, cell: usize, score: usize, n_active: u64) -> Option<(f64, f64)> { /* ... */ }
```

### Re-exports

#### Re-export `ComputeType`

```rust
pub use outram_mc_libs::physics::compute::ComputeType;
```

#### Re-export `ThreadCount`

```rust
pub use outram_mc_libs::physics::compute::ThreadCount;
```

#### Re-export `KeffResult`

```rust
pub use outram_mc_libs::physics::keff::KeffResult;
```

#### Re-export `KeffSettings`

```rust
pub use outram_mc_libs::physics::keff::KeffSettings;
```

#### Re-export `SourceBox`

```rust
pub use outram_mc_libs::physics::transport_csg::SourceBox;
```

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

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
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

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

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
## Re-exports

### Re-export `NordheimFuchsExactTimestepper`

```rust
pub use teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper;
```

### Re-export `TehOPrkeError`

```rust
pub use teh_o_prke::teh_o_prke_error::TehOPrkeError;
```

