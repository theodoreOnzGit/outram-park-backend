# Crate Documentation

**Version:** 0.0.5

**Format Version:** 61

# Module `nee_soon`

# NEE_SOON

**NE**utron **E**nergy-dependent **S**imulation using **O**pen-source
**O**bject-Oriented **N**umerics. (Lettering corrected 2026-10-13: it
spelled NESOOON before.)

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
| [`power_shape`] | power shapes (lumps, cosine/sine, `J0 x cos`, Legendre-z, Zernike(-Legendre), Fourier-Bessel, spatial spherical harmonics, R-Z tables), read from Monte Carlo tallies and put on TH cells conserving the power; the HTR-10 frames (gh:#1184) |
| [`htr10_coupling`] | [`htr10_coupling::Htr10DirectCoupling`]: the whole-core HTR-10 Monte Carlo (`ParametricStudy`) iterated against a lumped node or the `htr10_rz` R-Z core, reprocessing the data exactly at each zone temperature (gh:#1186) |
| [`mgxs_genfoam`] | four isothermal Monte Carlo MGXS records stacked with `TFuel` into a GeN-Foam R-Z diffusion core on the `htr10_rz` mesh (gh:#1187) |

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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
## Module `htr10_th_transient`

HTR-10 coupled thermal-hydraulic transients: point kinetics with the
`tampines` primary loop (gh:#1151; moved from `tampines` 2026-11-03).
# HTR-10 coupled thermal-hydraulic transients (gh:#1151)

Point kinetics (`teh-o-prke`) coupled to the `tampines` primary loop and
its core models: the LOFC ATWS case with its named sensitivity cases.
**Moved here from `tampines::htr10_primary_loop` on 2026-11-03**
(maintainer: "any neutronics and TH shld live in nee soon"; "Tampines
will have fixed or user determined heat sources"): `tampines` keeps the
loop and core thermal-hydraulics with the power as an input; everything
that computes power from reactivity lives here.

| Module | What |
|---|---|
| [`kinetics`] | PRKE + ANS decay heat + Xe-135 + fuel/moderator/reflector feedback + the start-up neutron source + external reactivity |
| [`transient`] | the coupled step, the LOFC and CRW cases (lumped or R-Z core) and their named sensitivities, metrics, thermocouple probes |
| [`data`] | embedded copies of the measured traces and the Tab 2.60 reactivity |

V&V records: `crates/nee_soon/verification_and_validation/htr10_loop_lofc/README.md`
(part A, lumped stand-in) and `.../htr10_rz_transients/README.md`
(part B, R-Z core).

Inputs: `docs/reactor-scoping/htr10-th-benchmark.md` and
`docs/reactor-scoping/htr10-th-data/`. Source keys are the sheet's.

> Research, education and V&V only. Not for licensing, operation or
> safety decisions. AI draft, unreviewed.

```rust
pub mod htr10_th_transient { /* ... */ }
```

### Modules

## Module `data`

Embedded validation tables of the HTR-10 coupled transients (gh:#1151),
moved from `tampines::htr10_primary_loop::data` on 2026-11-03.

Each file under `data/` is a byte-for-byte copy of the file of the same
name in `docs/reactor-scoping/htr10-th-data/`, which carries the citation
header, the digitisation method and the read error. The test
`data_copies_match_the_input_sheet` fails if a copy drifts. The loop's own
tables (power map, coastdown) stay in `tampines`.

> Research, education and V&V only. AI draft, unreviewed.

```rust
pub mod data { /* ... */ }
```

### Types

#### Struct `MeasuredTrace`

One measured thermocouple trace of [`LOFC_THERMOCOUPLES_C09`]: its label
(`SR6`, `TR1_TR4`, ...) and `(t s, T deg C, read band deg C)` rows.

```rust
pub struct MeasuredTrace {
    pub label: String,
    pub rows: Vec<(f64, f64, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `label` | `String` | Label as in [C09]. |
| `rows` | `Vec<(f64, f64, f64)>` | `(t, T, band)`. |

##### Implementations

###### Methods

- ```rust
  pub fn value_at(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Linear interpolation in `T` (held at the ends).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `lofc_thermocouple_traces`

The 17 traces of [`LOFC_THERMOCOUPLES_C09`], in file order.

# Errors
[`TampinesError::InvalidInput`] for a row that does not parse.

```rust
pub fn lofc_thermocouple_traces() -> Result<Vec<MeasuredTrace>, tampines::TampinesError> { /* ... */ }
```

### Constants and Statics

#### Constant `POWER_LONG_C09_FIG6`

Chen et al. (2009) Fig 6, measured relative fission power, 0-10800 s.

```rust
pub const POWER_LONG_C09_FIG6: &str = "# Source: Chen F., Dong Y., Zhang Z., Zheng Y., Shi L., Hu S., \"Post-test analysis of helium circulator trip\n# without scram at 3 MW power level on the HTR-10\", Nucl. Eng. Des. 239 (2009) 1010-1018. [C09]\n# Redistribution: (c) 2009 Elsevier B.V., all rights reserved. Numbers only, cited by figure (maintainer decision D1, 2026-11-03).\n# Digitised 2026-11-03 for gh:#1151 by an AI agent (draft until human review). Method: the embedded raster figure\n# was extracted at native resolution with `pdfimages -png` (no resampling). The plot frame was located from its\n# solid axis lines; the axes were calibrated from the frame lines and the printed tick labels (and checked against\n# the dotted gridlines where present). Curve pixels were selected by colour and reduced per pixel column (scripts\n# kept in the session scratchpad, not committed). The legend box was masked. No point was interpolated or invented:\n# a column with no curve pixel (hidden behind another curve) is simply absent.\n# Content: [C09] Fig 6, curve \"Fission power_test\" (MEASURED), relative fission power 0-10800 s.\n# Resolution: 1 px = 19.7 s and 0.0054 relative (= 18 kW at 3315 kW). Read error: +-0.006 relative and\n# +-20 s, or the half-width if larger. Checks against the text: first peak read 0.246 at 4395 s vs [C09]\n# Sec 5 \"24.7 % ... at 4400 s\"; second peak 0.097 at ~5610 s vs [T1] \"0.1 at 1250 s after the first\";\n# plateau 0.054 (179 kW) vs [C09] \"about 180 kW within 3 h\".\nt_s,relative_power,halfwidth\n19.71,0.92703,0.0676\n39.42,0.73784,0.1919\n59.12,0.47568,0.0811\n78.83,0.33243,0.0676\n98.54,0.22703,0.0378\n118.25,0.16216,0.0324\n137.96,0.12162,0.0189\n157.66,0.08919,0.0135\n177.37,0.06757,0.0081\n197.08,0.05405,0.0054\n216.79,0.04054,0.0027\n236.50,0.03243,0.0054\n256.20,0.02432,0.0027\n275.91,0.01892,0.0027\n295.62,0.01351,0.0027\n315.33,0.01081,0.0027\n335.04,0.01081,0.0027\n354.74,0.01081,0.0027\n374.45,0.00541,0.0027\n394.16,0.00541,0.0027\n413.87,0.00541,0.0027\n433.58,0.00541,0.0027\n453.28,0.00541,0.0027\n472.99,0.00270,0.0027\n492.70,0.00000,0.0027\n512.41,0.00000,0.0027\n532.12,0.00000,0.0027\n551.82,0.00000,0.0027\n571.53,0.00000,0.0027\n591.24,0.00000,0.0027\n610.95,0.00000,0.0027\n630.66,0.00000,0.0027\n650.36,0.00000,0.0027\n670.07,0.00000,0.0027\n689.78,0.00000,0.0027\n709.49,0.00000,0.0027\n729.20,0.00000,0.0027\n748.91,0.00000,0.0027\n768.61,0.00000,0.0027\n788.32,0.00000,0.0027\n808.03,0.00000,0.0027\n827.74,0.00000,0.0027\n847.45,0.00000,0.0027\n867.15,0.00000,0.0027\n886.86,0.00000,0.0027\n906.57,0.00000,0.0027\n926.28,0.00000,0.0027\n945.99,0.00000,0.0027\n965.69,0.00000,0.0027\n985.40,0.00000,0.0027\n1005.11,0.00000,0.0027\n1024.82,0.00000,0.0027\n1044.53,0.00000,0.0027\n1064.23,0.00000,0.0027\n1083.94,0.00000,0.0027\n1103.65,0.00000,0.0027\n1123.36,0.00000,0.0027\n1143.07,0.00000,0.0027\n1162.77,0.00000,0.0027\n1182.48,0.00000,0.0027\n1202.19,0.00000,0.0027\n1221.90,0.00000,0.0027\n1241.61,0.00000,0.0027\n1261.31,0.00000,0.0027\n1281.02,0.00000,0.0027\n1300.73,0.00000,0.0027\n1320.44,0.00000,0.0027\n1340.15,0.00000,0.0027\n1359.85,0.00000,0.0027\n1379.56,0.00000,0.0027\n1399.27,0.00000,0.0027\n1418.98,0.00000,0.0027\n1438.69,0.00000,0.0027\n1458.39,0.00000,0.0027\n1478.10,0.00000,0.0027\n1497.81,0.00000,0.0027\n1517.52,0.00000,0.0027\n1537.23,0.00000,0.0027\n1556.93,0.00000,0.0027\n1576.64,0.00000,0.0027\n1596.35,0.00000,0.0027\n1616.06,0.00000,0.0027\n1635.77,0.00000,0.0027\n1655.47,0.00000,0.0027\n1675.18,0.00000,0.0027\n1694.89,0.00000,0.0027\n1714.60,0.00000,0.0027\n1734.31,0.00000,0.0027\n1754.01,0.00000,0.0027\n1773.72,0.00000,0.0027\n1793.43,0.00000,0.0027\n1813.14,0.00000,0.0027\n1832.85,0.00000,0.0027\n1852.55,0.00000,0.0027\n1872.26,0.00000,0.0027\n1891.97,0.00000,0.0027\n1911.68,0.00000,0.0027\n1931.39,0.00000,0.0027\n1951.09,0.00000,0.0027\n1970.80,0.00000,0.0027\n1990.51,0.00000,0.0027\n2010.22,0.00000,0.0027\n2029.93,0.00000,0.0027\n2049.64,0.00000,0.0027\n2069.34,0.00000,0.0027\n2089.05,0.00000,0.0027\n2108.76,0.00000,0.0027\n2128.47,0.00000,0.0027\n2148.18,0.00000,0.0027\n2167.88,0.00000,0.0027\n2187.59,0.00000,0.0027\n2207.30,0.00000,0.0027\n2227.01,0.00000,0.0027\n2246.72,0.00000,0.0027\n2266.42,0.00000,0.0027\n2286.13,0.00000,0.0027\n2305.84,0.00000,0.0027\n2325.55,0.00000,0.0027\n2345.26,0.00000,0.0027\n2364.96,0.00000,0.0027\n2384.67,0.00000,0.0027\n2404.38,0.00000,0.0027\n2424.09,0.00000,0.0027\n2443.80,0.00000,0.0027\n2463.50,0.00000,0.0027\n2483.21,0.00000,0.0027\n2502.92,0.00000,0.0027\n2522.63,0.00000,0.0027\n2542.34,0.00000,0.0027\n2562.04,0.00000,0.0027\n2581.75,0.00000,0.0027\n2601.46,0.00000,0.0027\n2621.17,0.00000,0.0027\n2640.88,0.00000,0.0027\n2660.58,0.00000,0.0027\n2680.29,0.00000,0.0027\n2700.00,0.00000,0.0027\n2719.71,0.00000,0.0027\n2739.42,0.00000,0.0027\n2759.12,0.00000,0.0027\n2778.83,0.00000,0.0027\n2798.54,0.00000,0.0027\n2818.25,0.00000,0.0027\n2837.96,0.00000,0.0027\n2857.66,0.00000,0.0027\n2877.37,0.00000,0.0027\n2897.08,0.00000,0.0027\n2916.79,0.00000,0.0027\n2936.50,0.00000,0.0027\n2956.20,0.00000,0.0027\n2975.91,0.00000,0.0027\n2995.62,0.00000,0.0027\n3015.33,0.00000,0.0027\n3035.04,0.00000,0.0027\n3054.74,0.00000,0.0027\n3074.45,0.00000,0.0027\n3094.16,0.00000,0.0027\n3113.87,0.00000,0.0027\n3133.58,0.00000,0.0027\n3153.28,0.00000,0.0027\n3172.99,0.00000,0.0027\n3192.70,0.00000,0.0027\n3212.41,0.00000,0.0027\n3232.12,0.00000,0.0027\n3251.82,0.00000,0.0027\n3271.53,0.00000,0.0027\n3291.24,0.00000,0.0027\n3310.95,0.00000,0.0027\n3330.66,0.00000,0.0027\n3350.36,0.00000,0.0027\n3370.07,0.00000,0.0027\n3389.78,0.00000,0.0027\n3409.49,0.00000,0.0027\n3429.20,0.00000,0.0027\n3448.91,0.00000,0.0027\n3468.61,0.00000,0.0027\n3488.32,0.00000,0.0027\n3508.03,0.00000,0.0027\n3527.74,0.00000,0.0027\n3547.45,0.00000,0.0027\n3567.15,0.00000,0.0027\n3586.86,0.00000,0.0027\n3606.57,0.00000,0.0027\n3626.28,0.00000,0.0027\n3645.99,0.00000,0.0027\n3665.69,0.00000,0.0027\n3685.40,0.00000,0.0027\n3705.11,0.00000,0.0027\n3724.82,0.00000,0.0027\n3744.53,0.00000,0.0027\n3764.23,0.00000,0.0027\n3783.94,0.00000,0.0027\n3803.65,0.00000,0.0027\n3823.36,0.00000,0.0027\n3843.07,0.00000,0.0027\n3862.77,0.00000,0.0027\n3882.48,0.00000,0.0027\n3902.19,0.00000,0.0027\n3921.90,0.00000,0.0027\n3941.61,0.00000,0.0027\n3961.31,0.00000,0.0027\n3981.02,0.00000,0.0027\n4000.73,0.00000,0.0027\n4020.44,0.00270,0.0027\n4040.15,0.00000,0.0027\n4059.85,0.00000,0.0027\n4079.56,0.00000,0.0027\n4099.27,0.00000,0.0027\n4118.98,0.00000,0.0027\n4138.69,0.00000,0.0027\n4158.39,0.00270,0.0027\n4178.10,0.00541,0.0027\n4197.81,0.00541,0.0027\n4217.52,0.01081,0.0027\n4237.23,0.01351,0.0027\n4256.93,0.02703,0.0054\n4276.64,0.04324,0.0108\n4296.35,0.06757,0.0135\n4316.06,0.10541,0.0243\n4335.77,0.14595,0.0270\n4355.47,0.19730,0.0243\n4375.18,0.22973,0.0135\n4394.89,0.24595,0.0027\n4414.60,0.23784,0.0054\n4434.31,0.22162,0.0108\n4454.01,0.19730,0.0135\n4473.72,0.17027,0.0135\n4493.43,0.14595,0.0108\n4513.14,0.12703,0.0081\n4532.85,0.11081,0.0081\n4552.55,0.09189,0.0054\n4572.26,0.08108,0.0054\n4591.97,0.06757,0.0027\n4611.68,0.05676,0.0027\n4631.39,0.05135,0.0027\n4651.09,0.04595,0.0027\n4670.80,0.04054,0.0027\n4690.51,0.03514,0.0027\n4710.22,0.02973,0.0027\n4729.93,0.02703,0.0027\n4749.64,0.02162,0.0027\n4769.34,0.02162,0.0027\n4789.05,0.01892,0.0027\n4808.76,0.01622,0.0027\n4828.47,0.01622,0.0027\n4848.18,0.01622,0.0027\n4867.88,0.01351,0.0027\n4887.59,0.01081,0.0027\n4907.30,0.01081,0.0027\n4927.01,0.01081,0.0027\n4946.72,0.01081,0.0027\n4966.42,0.01081,0.0027\n4986.13,0.01081,0.0027\n5005.84,0.01081,0.0027\n5025.55,0.01081,0.0027\n5045.26,0.01081,0.0027\n5064.96,0.01081,0.0027\n5084.67,0.01081,0.0027\n5104.38,0.01081,0.0027\n5124.09,0.01081,0.0027\n5143.80,0.01081,0.0027\n5163.50,0.01081,0.0027\n5183.21,0.01081,0.0027\n5202.92,0.01081,0.0027\n5222.63,0.01351,0.0027\n5242.34,0.01622,0.0027\n5262.04,0.01622,0.0027\n5281.75,0.01622,0.0027\n5301.46,0.02162,0.0027\n5321.17,0.02162,0.0027\n5340.88,0.02432,0.0027\n5360.58,0.02973,0.0027\n5380.29,0.03514,0.0027\n5400.00,0.04054,0.0027\n5419.71,0.04595,0.0027\n5439.42,0.05135,0.0027\n5459.12,0.05676,0.0027\n5478.83,0.06216,0.0027\n5498.54,0.07297,0.0027\n5518.25,0.07838,0.0027\n5537.96,0.08378,0.0027\n5557.66,0.08919,0.0027\n5577.37,0.09459,0.0027\n5597.08,0.09730,0.0027\n5616.79,0.09730,0.0027\n5636.50,0.09730,0.0027\n5656.20,0.09459,0.0027\n5675.91,0.09189,0.0027\n5695.62,0.08919,0.0027\n5715.33,0.08649,0.0027\n5735.04,0.08108,0.0027\n5754.74,0.07838,0.0027\n5774.45,0.07297,0.0027\n5794.16,0.06757,0.0027\n5813.87,0.06486,0.0027\n5833.58,0.05946,0.0027\n5853.28,0.05676,0.0027\n5872.99,0.05405,0.0027\n5892.70,0.04865,0.0027\n5912.41,0.04865,0.0027\n5932.12,0.04324,0.0027\n5951.82,0.04324,0.0027\n5971.53,0.04054,0.0027\n5991.24,0.03784,0.0027\n6010.95,0.03784,0.0027\n6030.66,0.03784,0.0027\n6050.36,0.03514,0.0027\n6070.07,0.03243,0.0027\n6089.78,0.03243,0.0027\n6109.49,0.03243,0.0027\n6129.20,0.03243,0.0027\n6148.91,0.03243,0.0027\n6168.61,0.03243,0.0027\n6188.32,0.03243,0.0027\n6208.03,0.03243,0.0027\n6227.74,0.03243,0.0027\n6247.45,0.03243,0.0027\n6267.15,0.03243,0.0027\n6286.86,0.03514,0.0027\n6306.57,0.03784,0.0027\n6326.28,0.03784,0.0027\n6345.99,0.03784,0.0027\n6365.69,0.03784,0.0027\n6385.40,0.04324,0.0027\n6405.11,0.04324,0.0027\n6424.82,0.04595,0.0027\n6444.53,0.04865,0.0027\n6464.23,0.04865,0.0027\n6483.94,0.05135,0.0027\n6503.65,0.05405,0.0027\n6523.36,0.05676,0.0027\n6543.07,0.05946,0.0027\n6562.77,0.05946,0.0027\n6582.48,0.06216,0.0027\n6602.19,0.06486,0.0027\n6621.90,0.06486,0.0027\n6641.61,0.06486,0.0027\n6661.31,0.06486,0.0027\n6681.02,0.06486,0.0027\n6700.73,0.06486,0.0027\n6720.44,0.06486,0.0027\n6740.15,0.06486,0.0027\n6759.85,0.06486,0.0027\n6779.56,0.06486,0.0027\n6799.27,0.06486,0.0027\n6818.98,0.06486,0.0027\n6838.69,0.05946,0.0027\n6858.39,0.05946,0.0027\n6878.10,0.05946,0.0027\n6897.81,0.05676,0.0027\n6917.52,0.05405,0.0027\n6937.23,0.05405,0.0027\n6956.93,0.05405,0.0027\n6976.64,0.05405,0.0027\n6996.35,0.05135,0.0027\n7016.06,0.05135,0.0027\n7035.77,0.04865,0.0027\n7055.47,0.04865,0.0027\n7075.18,0.04865,0.0027\n7094.89,0.04865,0.0027\n7114.60,0.04865,0.0027\n7134.31,0.04865,0.0027\n7154.01,0.04865,0.0027\n7173.72,0.04595,0.0027\n7193.43,0.04324,0.0027\n7213.14,0.04324,0.0027\n7232.85,0.04595,0.0027\n7252.55,0.04865,0.0027\n7272.26,0.04865,0.0027\n7291.97,0.04595,0.0027\n7311.68,0.04865,0.0027\n7331.39,0.04865,0.0027\n7351.09,0.04865,0.0027\n7370.80,0.04865,0.0027\n7390.51,0.04865,0.0027\n7410.22,0.04865,0.0027\n7429.93,0.04865,0.0027\n7449.64,0.05135,0.0027\n7469.34,0.05135,0.0027\n7489.05,0.05405,0.0027\n7508.76,0.05405,0.0027\n7528.47,0.05405,0.0027\n7548.18,0.05405,0.0027\n7567.88,0.05405,0.0027\n7587.59,0.05405,0.0027\n7607.30,0.05405,0.0027\n7627.01,0.05405,0.0027\n7646.72,0.05676,0.0027\n7666.42,0.05946,0.0027\n7686.13,0.05946,0.0027\n7705.84,0.05946,0.0027\n7725.55,0.05946,0.0027\n7745.26,0.05946,0.0027\n7764.96,0.05946,0.0027\n7784.67,0.05946,0.0027\n7804.38,0.05946,0.0027\n7824.09,0.05946,0.0027\n7843.80,0.05676,0.0027\n7863.50,0.05405,0.0027\n7883.21,0.05405,0.0027\n7902.92,0.05405,0.0027\n7922.63,0.05405,0.0027\n7942.34,0.05405,0.0027\n7962.04,0.05405,0.0027\n7981.75,0.05405,0.0027\n8001.46,0.05405,0.0027\n8021.17,0.05405,0.0027\n8040.88,0.05405,0.0027\n8060.58,0.05405,0.0027\n8080.29,0.05405,0.0027\n8100.00,0.05405,0.0027\n8119.71,0.05135,0.0027\n8139.42,0.05135,0.0027\n8159.12,0.05135,0.0027\n8178.83,0.04865,0.0027\n8198.54,0.04865,0.0027\n8218.25,0.04865,0.0027\n8237.96,0.04865,0.0027\n8257.66,0.04865,0.0027\n8277.37,0.05135,0.0027\n8297.08,0.04865,0.0027\n8356.20,0.05405,0.0027\n8415.33,0.05135,0.0027\n8435.04,0.05405,0.0027\n8454.74,0.05405,0.0027\n8474.45,0.05405,0.0027\n8494.16,0.05405,0.0027\n8513.87,0.05405,0.0027\n8533.58,0.05405,0.0027\n8553.28,0.05405,0.0027\n8572.99,0.05405,0.0027\n8592.70,0.05405,0.0027\n8612.41,0.05405,0.0027\n8632.12,0.05405,0.0027\n8651.82,0.05405,0.0027\n8671.53,0.05405,0.0027\n8691.24,0.05405,0.0027\n8710.95,0.05405,0.0027\n8730.66,0.05405,0.0027\n8750.36,0.05405,0.0027\n8770.07,0.05405,0.0027\n8789.78,0.05405,0.0027\n8809.49,0.05405,0.0027\n8829.20,0.05405,0.0027\n8848.91,0.05405,0.0027\n8868.61,0.05405,0.0027\n8888.32,0.05405,0.0027\n8908.03,0.05405,0.0027\n8927.74,0.05405,0.0027\n8947.45,0.05405,0.0027\n8967.15,0.05405,0.0027\n8986.86,0.05405,0.0027\n9006.57,0.05405,0.0027\n9026.28,0.05405,0.0027\n9045.99,0.05405,0.0027\n9065.69,0.05405,0.0027\n9085.40,0.05405,0.0027\n9105.11,0.05405,0.0027\n9124.82,0.05405,0.0027\n9144.53,0.05405,0.0027\n9164.23,0.05405,0.0027\n9183.94,0.05405,0.0027\n9203.65,0.05405,0.0027\n9223.36,0.05405,0.0027\n9243.07,0.05405,0.0027\n9262.77,0.05405,0.0027\n9282.48,0.05405,0.0027\n9302.19,0.05405,0.0027\n9321.90,0.05405,0.0027\n9341.61,0.05405,0.0027\n9361.31,0.05405,0.0027\n9381.02,0.05405,0.0027\n9400.73,0.05405,0.0027\n9420.44,0.05405,0.0027\n9440.15,0.05405,0.0027\n9459.85,0.05405,0.0027\n9479.56,0.05405,0.0027\n9499.27,0.05405,0.0027\n9518.98,0.05405,0.0027\n9538.69,0.05405,0.0027\n9558.39,0.05405,0.0027\n9578.10,0.05405,0.0027\n9597.81,0.05405,0.0027\n9617.52,0.05405,0.0027\n9637.23,0.05405,0.0027\n9656.93,0.05405,0.0027\n9676.64,0.05405,0.0027\n9696.35,0.05405,0.0027\n9716.06,0.05405,0.0027\n9735.77,0.05405,0.0027\n9755.47,0.05405,0.0027\n9775.18,0.05405,0.0027\n9794.89,0.05405,0.0027\n9814.60,0.05405,0.0027\n9834.31,0.05405,0.0027\n9854.01,0.05405,0.0027\n9873.72,0.05405,0.0027\n9893.43,0.05405,0.0027\n9913.14,0.05405,0.0027\n9932.85,0.05405,0.0027\n9952.55,0.05405,0.0027\n9972.26,0.05405,0.0027\n9991.97,0.05405,0.0027\n10011.68,0.05405,0.0027\n10031.39,0.05405,0.0027\n10051.09,0.05405,0.0027\n10070.80,0.05405,0.0027\n10090.51,0.05405,0.0027\n10110.22,0.05405,0.0027\n10129.93,0.05405,0.0027\n10149.64,0.05405,0.0027\n10169.34,0.05405,0.0027\n10189.05,0.05405,0.0027\n10208.76,0.05405,0.0027\n10228.47,0.05405,0.0027\n10248.18,0.05405,0.0027\n10267.88,0.05405,0.0027\n10287.59,0.05405,0.0027\n10307.30,0.05405,0.0027\n10327.01,0.05405,0.0027\n10346.72,0.05405,0.0027\n10366.42,0.05405,0.0027\n10386.13,0.05405,0.0027\n10405.84,0.05405,0.0027\n10425.55,0.05405,0.0027\n10445.26,0.05405,0.0027\n10464.96,0.05405,0.0027\n10484.67,0.05405,0.0027\n10504.38,0.05405,0.0027\n10524.09,0.05405,0.0027\n10543.80,0.05405,0.0027\n10563.50,0.05405,0.0027\n10583.21,0.05405,0.0027\n10602.92,0.05405,0.0027\n10622.63,0.05405,0.0027\n10642.34,0.05405,0.0027\n10662.04,0.05405,0.0027\n10681.75,0.05405,0.0027\n10701.46,0.05405,0.0027\n10721.17,0.05405,0.0027\n10740.88,0.05405,0.0027\n10760.58,0.05405,0.0027\n10780.29,0.05405,0.0027\n";
```

#### Constant `POWER_SHORT_C09_FIG4`

Chen et al. (2009) Fig 4, measured relative fission power, 0-500 s.

```rust
pub const POWER_SHORT_C09_FIG4: &str = "# Source: Chen F., Dong Y., Zhang Z., Zheng Y., Shi L., Hu S., \"Post-test analysis of helium circulator trip\n# without scram at 3 MW power level on the HTR-10\", Nucl. Eng. Des. 239 (2009) 1010-1018. [C09]\n# Redistribution: (c) 2009 Elsevier B.V., all rights reserved. Numbers only, cited by figure (maintainer decision D1, 2026-11-03).\n# Digitised 2026-11-03 for gh:#1151 by an AI agent (draft until human review). Method: the embedded raster figure\n# was extracted at native resolution with `pdfimages -png` (no resampling). The plot frame was located from its\n# solid axis lines; the axes were calibrated from the frame lines and the printed tick labels (and checked against\n# the dotted gridlines where present). Curve pixels were selected by colour and reduced per pixel column (scripts\n# kept in the session scratchpad, not committed). The legend box was masked. No point was interpolated or invented:\n# a column with no curve pixel (hidden behind another curve) is simply absent.\n# Content: [C09] Fig 4, curve \"Fission power_test\" (MEASURED), relative fission power 0-500 s after the\n# circulator trip; 1.0 = initial power (3.3 MW per [C09] Tab 1; 3315 kW per [J09] Tab 2).\n# Resolution 1 px = 0.43 s, 0.0028 relative. The calculated (black) THERMIX curve overlies the test curve in\n# places; columns where red is hidden are absent. Read error +-0.01 relative, or the half-width if larger.\nt_s,relative_power,halfwidth\n0.86,1.00000,0.0014\n3.44,0.99722,0.0014\n6.02,0.99722,0.0014\n8.61,0.99444,0.0014\n11.19,0.98333,0.0028\n13.77,0.96250,0.0042\n16.35,0.94167,0.0028\n18.93,0.91944,0.0028\n21.51,0.88333,0.0083\n24.10,0.83889,0.0083\n26.68,0.79444,0.0083\n29.26,0.74722,0.0083\n31.84,0.71111,0.0056\n34.42,0.67778,0.0056\n37.01,0.64861,0.0014\n39.59,0.60972,0.0069\n42.17,0.58194,0.0042\n44.75,0.55278,0.0056\n47.33,0.52500,0.0056\n49.91,0.49722,0.0028\n52.50,0.47361,0.0042\n55.08,0.45139,0.0042\n57.66,0.42917,0.0042\n60.24,0.40833,0.0028\n62.82,0.39028,0.0014\n65.40,0.37361,0.0014\n67.99,0.35833,0.0028\n70.57,0.34167,0.0028\n73.15,0.32500,0.0028\n75.73,0.30833,0.0028\n78.31,0.29167,0.0028\n80.90,0.27639,0.0014\n83.48,0.26667,0.0028\n86.06,0.25556,0.0028\n88.64,0.24583,0.0014\n91.22,0.23611,0.0014\n93.80,0.22639,0.0014\n96.39,0.21667,0.0014\n98.97,0.20694,0.0014\n101.55,0.19861,0.0014\n104.13,0.19028,0.0014\n106.71,0.18194,0.0014\n109.29,0.17500,0.0014\n111.88,0.16806,0.0014\n114.46,0.16250,0.0014\n117.04,0.15556,0.0014\n119.62,0.15000,0.0014\n122.20,0.14444,0.0014\n124.78,0.13750,0.0014\n127.37,0.13056,0.0014\n129.95,0.12500,0.0014\n132.53,0.12083,0.0014\n135.11,0.11667,0.0014\n137.69,0.11250,0.0014\n140.28,0.10833,0.0014\n142.86,0.10417,0.0014\n145.44,0.10000,0.0014\n148.02,0.09722,0.0014\n150.60,0.09306,0.0014\n153.18,0.09028,0.0014\n155.77,0.08611,0.0014\n158.35,0.08333,0.0014\n160.93,0.08056,0.0014\n163.51,0.07778,0.0014\n166.09,0.07500,0.0014\n168.67,0.07222,0.0014\n171.26,0.06944,0.0014\n173.84,0.06667,0.0014\n176.42,0.06389,0.0014\n179.00,0.06250,0.0014\n181.58,0.06111,0.0014\n184.17,0.05833,0.0014\n186.75,0.05556,0.0014\n189.33,0.05417,0.0014\n191.91,0.05278,0.0014\n194.49,0.05000,0.0014\n197.07,0.05000,0.0014\n199.66,0.04722,0.0014\n202.24,0.04583,0.0014\n204.82,0.04444,0.0014\n207.40,0.04306,0.0014\n209.98,0.04167,0.0014\n212.56,0.04028,0.0014\n215.15,0.03889,0.0014\n217.73,0.03889,0.0014\n220.31,0.50556,0.4694\n222.89,0.50556,0.4694\n225.47,0.50417,0.4708\n228.06,0.50278,0.4722\n230.64,0.50278,0.4722\n233.22,0.50278,0.4722\n235.80,0.50278,0.4722\n238.38,0.50139,0.4736\n240.96,0.50139,0.4736\n243.55,0.50139,0.4736\n246.13,0.50000,0.4750\n248.71,0.50000,0.4750\n251.29,0.50000,0.4750\n253.87,0.97500,0.0014\n256.45,0.49861,0.4764\n259.04,0.02222,0.0014\n261.62,0.02222,0.0014\n264.20,0.02222,0.0014\n266.78,0.01944,0.0014\n269.36,0.01944,0.0014\n271.94,0.01944,0.0014\n274.53,0.01944,0.0014\n277.11,0.01944,0.0014\n282.27,0.01667,0.0014\n284.85,0.01667,0.0014\n287.44,0.01667,0.0014\n290.02,0.01667,0.0014\n292.60,0.01667,0.0014\n295.18,0.01389,0.0014\n297.76,0.01389,0.0014\n300.34,0.01389,0.0014\n302.93,0.01389,0.0014\n305.51,0.01389,0.0014\n308.09,0.01389,0.0014\n315.83,0.01111,0.0014\n318.42,0.01111,0.0014\n321.00,0.01111,0.0014\n323.58,0.01111,0.0014\n326.16,0.01111,0.0014\n328.74,0.01111,0.0014\n331.33,0.01111,0.0014\n333.91,0.01111,0.0014\n336.49,0.00833,0.0014\n339.07,0.00833,0.0014\n341.65,0.00833,0.0014\n344.23,0.00833,0.0014\n346.82,0.00833,0.0014\n349.40,0.00833,0.0014\n351.98,0.00833,0.0014\n354.56,0.00833,0.0014\n357.14,0.00833,0.0014\n359.72,0.00833,0.0014\n362.31,0.00833,0.0014\n364.89,0.00833,0.0014\n367.47,0.00694,0.0014\n370.05,0.00556,0.0014\n372.63,0.00556,0.0014\n375.22,0.00556,0.0014\n377.80,0.00556,0.0014\n380.38,0.00556,0.0014\n382.96,0.00556,0.0014\n385.54,0.00556,0.0014\n388.12,0.00556,0.0014\n390.71,0.00556,0.0014\n393.29,0.00556,0.0014\n395.87,0.00556,0.0014\n398.45,0.00556,0.0014\n401.03,0.00556,0.0014\n403.61,0.00556,0.0014\n406.20,0.00556,0.0014\n408.78,0.00556,0.0014\n411.36,0.00556,0.0014\n413.94,0.00556,0.0014\n416.52,0.00556,0.0014\n419.10,0.00556,0.0014\n421.69,0.00556,0.0014\n424.27,0.00556,0.0014\n426.85,0.00556,0.0014\n429.43,0.00278,0.0014\n432.01,0.00278,0.0014\n434.60,0.00278,0.0014\n437.18,0.00278,0.0014\n439.76,0.00278,0.0014\n442.34,0.00278,0.0014\n450.09,0.00278,0.0014\n452.67,0.00278,0.0014\n455.25,0.00278,0.0014\n457.83,0.00278,0.0014\n460.41,0.00278,0.0014\n462.99,0.00278,0.0014\n465.58,0.00278,0.0014\n468.16,0.00278,0.0014\n470.74,0.00278,0.0014\n473.32,0.00278,0.0014\n475.90,0.00278,0.0014\n478.49,0.00278,0.0014\n481.07,0.00278,0.0014\n483.65,0.00278,0.0014\n486.23,0.00278,0.0014\n488.81,0.00278,0.0014\n491.39,0.00278,0.0014\n493.98,0.00278,0.0014\n496.56,0.00278,0.0014\n499.14,0.00278,0.0014\n";
```

#### Constant `CRW_REACTIVITY_TAB2_60`

TECDOC-1694 Tab 2.60, CRW inserted reactivity (mk) against s, 0-128 s.

```rust
pub const CRW_REACTIVITY_TAB2_60: &str = "# Source: IAEA-TECDOC-1694 (2013), Evaluation of High Temperature Gas Cooled Reactor Performance:\n# Benchmark Analysis Related to the PBMR-400, PBMM, GT-MHR, HTR-10 and the ASTRA Critical Facility. IAEA, Vienna.\n# Redistribution: IAEA publication, no reuse licence. Numbers only, cited by table (maintainer decision D1, 2026-11-03).\n# Transcribed 2026-11-03 for gh:#1147 from pdftotext -layout of the PDF, checked against rendered pages.\n# Methodology, confidence and conflicts: docs/reactor-scoping/htr10-th-benchmark.md\n# Content: Table 2.60 \'CRD insertion speed vs induced positive reactivity\', HTR-10 control-rod withdrawal\n# without scram benchmark (Sec 2.4.1), 0-128 s in 1 s steps. Unit: mk (1 mk = 1e-3 dk/k = 100 pcm).\n# Derived here, not in the source: rho/beta with beta = 0.726 % (Sec 2.4.1) is $0.685 at 128 s.\nt_s,rho_mk\n0,0\n1,0.0825\n2,0.1905\n3,0.2984\n4,0.4064\n5,0.5144\n6,0.6223\n7,0.7303\n8,0.8383\n9,0.9462\n10,1.0542\n11,1.1086\n12,1.172\n13,1.2353\n14,1.2987\n15,1.3621\n16,1.4254\n17,1.4888\n18,1.5521\n19,1.6155\n20,1.6788\n21,1.7422\n22,1.8052\n23,1.8682\n24,1.9312\n25,1.9943\n26,2.0573\n27,2.1204\n28,2.1834\n29,2.2464\n30,2.3095\n31,2.3725\n32,2.4356\n33,2.4986\n34,2.5385\n35,2.5806\n36,2.6226\n37,2.6646\n38,2.7066\n39,2.7487\n40,2.7907\n41,2.8327\n42,2.8748\n43,2.9168\n44,2.9588\n45,3.0009\n46,3.0429\n47,3.0849\n48,3.1269\n49,3.169\n50,3.211\n51,3.253\n52,3.2951\n53,3.3371\n54,3.3713\n55,3.3976\n56,3.424\n57,3.4503\n58,3.4767\n59,3.5031\n60,3.5294\n61,3.5558\n62,3.5821\n63,3.6085\n64,3.6348\n65,3.6612\n66,3.6876\n67,3.7139\n68,3.7403\n69,3.7666\n70,3.793\n71,3.8193\n72,3.8457\n73,3.8721\n74,3.8963\n75,3.9192\n76,3.942\n77,3.9649\n78,3.9878\n79,4.0106\n80,4.0335\n81,4.0564\n82,4.0792\n83,4.1021\n84,4.125\n85,4.1478\n86,4.1707\n87,4.1936\n88,4.2164\n89,4.2393\n90,4.2622\n91,4.285\n92,4.3079\n93,4.3307\n94,4.3536\n95,4.3765\n96,4.3993\n97,4.4222\n98,4.4451\n99,4.4618\n100,4.4795\n101,4.4973\n102,4.515\n103,4.5327\n104,4.5505\n105,4.5682\n106,4.586\n107,4.6037\n108,4.6214\n109,4.6392\n110,4.6569\n111,4.6747\n112,4.6924\n113,4.7101\n114,4.7279\n115,4.7456\n116,4.7634\n117,4.7811\n118,4.7988\n119,4.8166\n120,4.8343\n121,4.852\n122,4.8698\n123,4.8875\n124,4.9053\n125,4.923\n126,4.9407\n127,4.9585\n128,4.9762\n";
```

#### Constant `LOFC_THERMOCOUPLES_C09`

Chen et al. (2009) Figs 8-11, the 17 measured internals thermocouple
traces of the LOFC test (180 s bins).

```rust
pub const LOFC_THERMOCOUPLES_C09: &str = "# Source: Chen F., Dong Y., Zhang Z., Zheng Y., Shi L., Hu S., \"Post-test analysis of helium circulator trip\n# without scram at 3 MW power level on the HTR-10\", Nucl. Eng. Des. 239 (2009) 1010-1018. [C09]\n# Redistribution: (c) 2009 Elsevier B.V., all rights reserved. Numbers only, cited by figure (maintainer decision D1, 2026-11-03).\n# Digitised 2026-11-03 for gh:#1151 by an AI agent (draft until human review). Method: the embedded raster figure\n# was extracted at native resolution with `pdfimages -png` (no resampling). The plot frame was located from its\n# solid axis lines; the axes were calibrated from the frame lines and the printed tick labels (and checked against\n# the dotted gridlines where present). Curve pixels were selected by colour and reduced per pixel column (scripts\n# kept in the session scratchpad, not committed). The legend box was masked. No point was interpolated or invented:\n# a column with no curve pixel (hidden behind another curve) is simply absent.\n# Content: [C09] Figs 8-11, curves \"Test\" (MEASURED), internals thermocouples during the LOFC ATWS test,\n# 0-10800 s. Pairs (TR1_TR4 etc.) are the averages of symmetric thermocouples as plotted in [C09].\n# Positions: [C09] Fig 3 (schematic only; no coordinates are printed). TR = top internals, SR1-6 / SR7-12\n# = side reflector and carbon brick at two heights, BR = bottom reflector.\n# Reduction: pixel columns binned in 180 s windows; T_C = mean of per-column centres of the red trace;\n# band_C = half the spread of every red pixel in the window plus half a pixel, i.e. the plotted noise band\n# plus read resolution. The noisy traces make band_C the read error. BR2_BR4 first bin contains the\n# initial spike from ~800 C (band 37 C). Checks against [C09] Sec 5: TR1&TR4 232 -> 405 C in 3 h (read\n# 232.7 -> 405.2); SR6 minimum ~295 C (read 295-297 C near 2000-2800 s).\ntc,figure,t_s,T_C,band_C\nTR3_TR6,Fig 8(a),92,202.69,0.86\nTR3_TR6,Fig 8(a),266,202.70,0.98\nTR3_TR6,Fig 8(a),450,202.90,1.14\nTR3_TR6,Fig 8(a),634,203.07,1.49\nTR3_TR6,Fig 8(a),808,203.25,0.94\nTR3_TR6,Fig 8(a),983,203.46,0.83\nTR3_TR6,Fig 8(a),1166,203.61,1.49\nTR3_TR6,Fig 8(a),1350,203.91,1.02\nTR3_TR6,Fig 8(a),1534,204.15,1.34\nTR3_TR6,Fig 8(a),1708,203.89,0.83\nTR3_TR6,Fig 8(a),1883,204.37,1.18\nTR3_TR6,Fig 8(a),2066,204.45,0.90\nTR3_TR6,Fig 8(a),2250,204.55,1.61\nTR3_TR6,Fig 8(a),2434,205.06,0.94\nTR3_TR6,Fig 8(a),2608,204.99,1.02\nTR3_TR6,Fig 8(a),2783,205.01,1.14\nTR3_TR6,Fig 8(a),2966,205.21,1.38\nTR3_TR6,Fig 8(a),3150,205.58,0.98\nTR3_TR6,Fig 8(a),3334,205.72,0.71\nTR3_TR6,Fig 8(a),3508,205.87,0.83\nTR3_TR6,Fig 8(a),3683,206.21,0.83\nTR3_TR6,Fig 8(a),3866,206.47,0.94\nTR3_TR6,Fig 8(a),4050,206.34,1.14\nTR3_TR6,Fig 8(a),4234,206.97,0.86\nTR3_TR6,Fig 8(a),4408,207.04,0.83\nTR3_TR6,Fig 8(a),4583,207.22,1.14\nTR3_TR6,Fig 8(a),4766,207.40,0.98\nTR3_TR6,Fig 8(a),4950,207.78,0.94\nTR3_TR6,Fig 8(a),5134,207.83,0.79\nTR3_TR6,Fig 8(a),5308,208.17,1.02\nTR3_TR6,Fig 8(a),5483,208.44,1.14\nTR3_TR6,Fig 8(a),5666,208.82,0.83\nTR3_TR6,Fig 8(a),5850,208.77,1.14\nTR3_TR6,Fig 8(a),6034,209.19,0.98\nTR3_TR6,Fig 8(a),6208,209.57,0.98\nTR3_TR6,Fig 8(a),6383,209.94,0.98\nTR3_TR6,Fig 8(a),6566,210.22,1.42\nTR3_TR6,Fig 8(a),6750,210.36,1.26\nTR3_TR6,Fig 8(a),6934,210.88,1.42\nTR3_TR6,Fig 8(a),7108,211.18,0.98\nTR3_TR6,Fig 8(a),7283,211.67,1.10\nTR3_TR6,Fig 8(a),7466,211.69,1.26\nTR3_TR6,Fig 8(a),7650,212.21,0.98\nTR3_TR6,Fig 8(a),7834,212.54,0.83\nTR3_TR6,Fig 8(a),8008,212.98,0.98\nTR3_TR6,Fig 8(a),8183,213.64,0.83\nTR3_TR6,Fig 8(a),8366,213.89,0.67\nTR3_TR6,Fig 8(a),8550,214.32,0.98\nTR3_TR6,Fig 8(a),8734,214.53,1.10\nTR3_TR6,Fig 8(a),8908,214.87,0.98\nTR3_TR6,Fig 8(a),9083,215.37,0.98\nTR3_TR6,Fig 8(a),9266,215.96,0.98\nTR3_TR6,Fig 8(a),9450,216.20,0.83\nTR3_TR6,Fig 8(a),9634,216.69,1.14\nTR3_TR6,Fig 8(a),9808,217.27,0.90\nTR3_TR6,Fig 8(a),9983,217.75,0.71\nTR3_TR6,Fig 8(a),10166,218.08,0.67\nTR3_TR6,Fig 8(a),10350,218.75,0.94\nTR3_TR6,Fig 8(a),10534,219.29,0.83\nTR3_TR6,Fig 8(a),10708,219.55,0.98\nTR2_TR5,Fig 8(b),92,218.52,1.43\nTR2_TR5,Fig 8(b),266,218.72,1.10\nTR2_TR5,Fig 8(b),450,218.98,1.32\nTR2_TR5,Fig 8(b),634,219.27,1.43\nTR2_TR5,Fig 8(b),808,219.44,1.21\nTR2_TR5,Fig 8(b),983,219.59,1.10\nTR2_TR5,Fig 8(b),1166,219.89,1.21\nTR2_TR5,Fig 8(b),1350,219.98,1.21\nTR2_TR5,Fig 8(b),1534,220.22,1.21\nTR2_TR5,Fig 8(b),1708,220.85,0.99\nTR2_TR5,Fig 8(b),1883,221.16,1.10\nTR2_TR5,Fig 8(b),2066,221.46,1.10\nTR2_TR5,Fig 8(b),2250,221.75,1.10\nTR2_TR5,Fig 8(b),2434,222.77,0.99\nTR2_TR5,Fig 8(b),2608,223.42,0.88\nTR2_TR5,Fig 8(b),2783,223.74,1.43\nTR2_TR5,Fig 8(b),2966,224.25,1.65\nTR2_TR5,Fig 8(b),3150,225.22,1.10\nTR2_TR5,Fig 8(b),3334,226.03,1.32\nTR2_TR5,Fig 8(b),3508,226.63,0.99\nTR2_TR5,Fig 8(b),3683,227.21,1.43\nTR2_TR5,Fig 8(b),3866,227.98,1.43\nTR2_TR5,Fig 8(b),4050,229.12,1.43\nTR2_TR5,Fig 8(b),4234,230.04,1.21\nTR2_TR5,Fig 8(b),4408,230.78,0.99\nTR2_TR5,Fig 8(b),4583,231.26,1.21\nTR2_TR5,Fig 8(b),4766,232.75,1.43\nTR2_TR5,Fig 8(b),4950,233.91,0.99\nTR2_TR5,Fig 8(b),5134,234.70,1.32\nTR2_TR5,Fig 8(b),5308,235.77,1.43\nTR2_TR5,Fig 8(b),5483,236.82,1.54\nTR2_TR5,Fig 8(b),5666,238.02,1.43\nTR2_TR5,Fig 8(b),5850,239.08,1.32\nTR2_TR5,Fig 8(b),6034,240.43,1.32\nTR2_TR5,Fig 8(b),6208,241.38,1.87\nTR2_TR5,Fig 8(b),6383,242.61,1.21\nTR2_TR5,Fig 8(b),6566,243.50,1.10\nTR2_TR5,Fig 8(b),6750,244.75,1.65\nTR2_TR5,Fig 8(b),6934,246.08,0.99\nTR2_TR5,Fig 8(b),7108,247.10,1.32\nTR2_TR5,Fig 8(b),7283,248.05,1.54\nTR2_TR5,Fig 8(b),7466,249.19,1.43\nTR2_TR5,Fig 8(b),7650,250.80,1.21\nTR2_TR5,Fig 8(b),7834,251.89,1.21\nTR2_TR5,Fig 8(b),8008,252.64,1.21\nTR2_TR5,Fig 8(b),8183,253.92,1.32\nTR2_TR5,Fig 8(b),8366,255.06,1.21\nTR2_TR5,Fig 8(b),8550,256.27,1.21\nTR2_TR5,Fig 8(b),8734,257.39,1.32\nTR2_TR5,Fig 8(b),8908,258.57,1.32\nTR2_TR5,Fig 8(b),9083,259.67,1.32\nTR2_TR5,Fig 8(b),9266,260.69,1.43\nTR2_TR5,Fig 8(b),9450,262.10,1.10\nTR2_TR5,Fig 8(b),9634,263.18,1.43\nTR2_TR5,Fig 8(b),9808,264.02,1.32\nTR2_TR5,Fig 8(b),9983,265.37,1.32\nTR2_TR5,Fig 8(b),10166,266.35,1.76\nTR2_TR5,Fig 8(b),10350,267.47,1.43\nTR2_TR5,Fig 8(b),10534,268.38,1.21\nTR2_TR5,Fig 8(b),10708,269.35,0.77\nTR1_TR4,Fig 8(c),92,232.65,2.75\nTR1_TR4,Fig 8(c),266,237.48,3.85\nTR1_TR4,Fig 8(c),450,242.32,3.85\nTR1_TR4,Fig 8(c),634,247.38,3.85\nTR1_TR4,Fig 8(c),808,251.91,3.30\nTR1_TR4,Fig 8(c),983,257.12,3.85\nTR1_TR4,Fig 8(c),1166,262.13,3.85\nTR1_TR4,Fig 8(c),1350,267.41,3.85\nTR1_TR4,Fig 8(c),1534,272.04,3.30\nTR1_TR4,Fig 8(c),1708,276.43,3.85\nTR1_TR4,Fig 8(c),1883,280.51,3.85\nTR1_TR4,Fig 8(c),2066,285.02,3.30\nTR1_TR4,Fig 8(c),2250,289.26,3.30\nTR1_TR4,Fig 8(c),2434,293.66,3.30\nTR1_TR4,Fig 8(c),2608,297.96,3.85\nTR1_TR4,Fig 8(c),2783,302.74,3.85\nTR1_TR4,Fig 8(c),2966,307.09,4.40\nTR1_TR4,Fig 8(c),3150,311.55,3.30\nTR1_TR4,Fig 8(c),3334,315.90,3.85\nTR1_TR4,Fig 8(c),3508,319.54,2.75\nTR1_TR4,Fig 8(c),3683,323.49,3.30\nTR1_TR4,Fig 8(c),3866,327.18,3.30\nTR1_TR4,Fig 8(c),4050,331.14,2.75\nTR1_TR4,Fig 8(c),4234,334.83,2.75\nTR1_TR4,Fig 8(c),4408,338.37,2.75\nTR1_TR4,Fig 8(c),4583,342.15,3.85\nTR1_TR4,Fig 8(c),4766,345.67,3.30\nTR1_TR4,Fig 8(c),4950,348.53,2.75\nTR1_TR4,Fig 8(c),5134,352.27,3.30\nTR1_TR4,Fig 8(c),5308,355.62,2.75\nTR1_TR4,Fig 8(c),5483,358.55,3.85\nTR1_TR4,Fig 8(c),5666,361.68,2.75\nTR1_TR4,Fig 8(c),5850,364.82,2.75\nTR1_TR4,Fig 8(c),6034,367.85,2.20\nTR1_TR4,Fig 8(c),6208,370.41,2.75\nTR1_TR4,Fig 8(c),6383,372.85,2.75\nTR1_TR4,Fig 8(c),6566,375.66,2.75\nTR1_TR4,Fig 8(c),6750,378.14,2.20\nTR1_TR4,Fig 8(c),6934,380.39,2.75\nTR1_TR4,Fig 8(c),7108,382.52,2.20\nTR1_TR4,Fig 8(c),7283,384.14,2.20\nTR1_TR4,Fig 8(c),7466,386.06,1.65\nTR1_TR4,Fig 8(c),7650,388.04,2.20\nTR1_TR4,Fig 8(c),7834,389.69,2.20\nTR1_TR4,Fig 8(c),8008,391.08,2.20\nTR1_TR4,Fig 8(c),8183,392.72,1.65\nTR1_TR4,Fig 8(c),8366,393.82,1.65\nTR1_TR4,Fig 8(c),8550,395.09,2.20\nTR1_TR4,Fig 8(c),8734,396.24,2.20\nTR1_TR4,Fig 8(c),8908,397.50,2.20\nTR1_TR4,Fig 8(c),9083,398.55,2.75\nTR1_TR4,Fig 8(c),9266,399.54,2.20\nTR1_TR4,Fig 8(c),9450,400.31,2.20\nTR1_TR4,Fig 8(c),9634,401.19,2.20\nTR1_TR4,Fig 8(c),9808,401.90,2.20\nTR1_TR4,Fig 8(c),9983,402.85,1.65\nTR1_TR4,Fig 8(c),10166,403.62,1.65\nTR1_TR4,Fig 8(c),10350,404.17,2.20\nTR1_TR4,Fig 8(c),10534,404.33,1.65\nTR1_TR4,Fig 8(c),10708,405.15,1.65\nSR6,Fig 9(a),101,324.30,1.63\nSR6,Fig 9(a),280,323.35,2.04\nSR6,Fig 9(a),458,319.68,2.86\nSR6,Fig 9(a),637,315.85,2.59\nSR6,Fig 9(a),816,311.72,2.72\nSR6,Fig 9(a),995,307.70,1.77\nSR6,Fig 9(a),1174,305.05,2.18\nSR6,Fig 9(a),1353,301.92,1.90\nSR6,Fig 9(a),1532,299.98,1.63\nSR6,Fig 9(a),1711,298.10,1.09\nSR6,Fig 9(a),1889,297.35,0.68\nSR6,Fig 9(a),2068,296.38,0.95\nSR6,Fig 9(a),2247,295.87,1.63\nSR6,Fig 9(a),2426,295.66,1.09\nSR6,Fig 9(a),2605,296.09,1.36\nSR6,Fig 9(a),2784,296.07,1.22\nSR6,Fig 9(a),2963,296.62,0.95\nSR6,Fig 9(a),3142,297.48,1.22\nSR6,Fig 9(a),3320,297.91,0.95\nSR6,Fig 9(a),3499,298.62,1.77\nSR6,Fig 9(a),3689,299.52,1.22\nSR6,Fig 9(a),3880,300.37,1.22\nSR6,Fig 9(a),4058,301.51,1.22\nSR6,Fig 9(a),4237,302.70,1.63\nSR6,Fig 9(a),4416,304.44,1.50\nSR6,Fig 9(a),4595,307.69,2.04\nSR6,Fig 9(a),4774,310.03,1.36\nSR6,Fig 9(a),4953,311.22,1.63\nSR6,Fig 9(a),5132,312.28,0.95\nSR6,Fig 9(a),5311,312.91,1.50\nSR6,Fig 9(a),5489,313.72,1.36\nSR6,Fig 9(a),5668,315.85,1.36\nSR6,Fig 9(a),5847,317.53,1.22\nSR6,Fig 9(a),6026,318.98,1.90\nSR6,Fig 9(a),6205,320.00,0.95\nSR6,Fig 9(a),6384,320.95,1.36\nSR6,Fig 9(a),6563,322.19,1.36\nSR6,Fig 9(a),6742,323.59,1.50\nSR6,Fig 9(a),6920,324.71,1.36\nSR6,Fig 9(a),7099,326.00,0.95\nSR6,Fig 9(a),7289,327.10,1.22\nSR6,Fig 9(a),7480,328.13,1.22\nSR6,Fig 9(a),7658,329.15,0.95\nSR6,Fig 9(a),7837,330.26,1.22\nSR6,Fig 9(a),8016,330.83,0.95\nSR6,Fig 9(a),8195,332.18,1.09\nSR6,Fig 9(a),8374,332.89,0.95\nSR6,Fig 9(a),8553,333.57,1.22\nSR6,Fig 9(a),8732,334.46,0.95\nSR6,Fig 9(a),8911,335.03,1.22\nSR6,Fig 9(a),9089,336.11,0.95\nSR6,Fig 9(a),9268,337.01,0.95\nSR6,Fig 9(a),9447,337.79,1.50\nSR6,Fig 9(a),9626,338.54,1.36\nSR6,Fig 9(a),9805,339.08,1.36\nSR6,Fig 9(a),9984,339.69,0.82\nSR6,Fig 9(a),10163,340.49,1.36\nSR6,Fig 9(a),10342,340.99,0.95\nSR6,Fig 9(a),10520,341.45,0.95\nSR6,Fig 9(a),10699,342.01,1.09\nSR5,Fig 9(b),101,293.20,0.95\nSR5,Fig 9(b),280,293.26,0.95\nSR5,Fig 9(b),459,293.11,1.19\nSR5,Fig 9(b),639,292.65,1.31\nSR5,Fig 9(b),818,292.62,0.95\nSR5,Fig 9(b),997,292.11,1.43\nSR5,Fig 9(b),1176,291.88,1.19\nSR5,Fig 9(b),1356,291.55,1.31\nSR5,Fig 9(b),1535,291.18,1.31\nSR5,Fig 9(b),1714,290.55,1.19\nSR5,Fig 9(b),1893,290.12,0.95\nSR5,Fig 9(b),2073,289.85,1.19\nSR5,Fig 9(b),2252,289.30,0.71\nSR5,Fig 9(b),2431,288.97,1.31\nSR5,Fig 9(b),2610,289.08,1.07\nSR5,Fig 9(b),2790,288.82,0.71\nSR5,Fig 9(b),2969,288.29,1.19\nSR5,Fig 9(b),3148,288.62,0.95\nSR5,Fig 9(b),3327,288.33,0.95\nSR5,Fig 9(b),3507,288.38,1.55\nSR5,Fig 9(b),3686,288.29,1.19\nSR5,Fig 9(b),3865,288.32,1.07\nSR5,Fig 9(b),4044,288.54,1.19\nSR5,Fig 9(b),4224,288.78,0.71\nSR5,Fig 9(b),4403,288.87,1.19\nSR5,Fig 9(b),4582,289.00,1.55\nSR5,Fig 9(b),4761,289.73,1.55\nSR5,Fig 9(b),4941,290.16,1.31\nSR5,Fig 9(b),5120,290.71,1.31\nSR5,Fig 9(b),5299,291.22,1.31\nSR5,Fig 9(b),5490,291.26,1.31\nSR5,Fig 9(b),5680,292.10,1.19\nSR5,Fig 9(b),5859,292.74,1.19\nSR5,Fig 9(b),6039,293.36,1.19\nSR5,Fig 9(b),6218,293.82,1.19\nSR5,Fig 9(b),6397,294.54,1.55\nSR5,Fig 9(b),6576,294.90,1.55\nSR5,Fig 9(b),6756,295.71,1.55\nSR5,Fig 9(b),6935,296.58,1.07\nSR5,Fig 9(b),7114,296.98,0.95\nSR5,Fig 9(b),7293,297.05,1.19\nSR5,Fig 9(b),7473,298.04,0.95\nSR5,Fig 9(b),7652,298.44,1.67\nSR5,Fig 9(b),7831,299.20,0.95\nSR5,Fig 9(b),8010,299.81,1.55\nSR5,Fig 9(b),8190,300.10,1.31\nSR5,Fig 9(b),8369,301.03,0.83\nSR5,Fig 9(b),8548,301.59,0.95\nSR5,Fig 9(b),8727,302.10,0.95\nSR5,Fig 9(b),8907,302.44,1.31\nSR5,Fig 9(b),9086,303.10,1.31\nSR5,Fig 9(b),9265,303.81,1.43\nSR5,Fig 9(b),9444,304.24,0.95\nSR5,Fig 9(b),9624,304.79,1.31\nSR5,Fig 9(b),9803,305.40,0.95\nSR5,Fig 9(b),9982,306.00,1.31\nSR5,Fig 9(b),10161,306.95,1.19\nSR5,Fig 9(b),10341,307.01,1.31\nSR5,Fig 9(b),10520,307.57,1.55\nSR5,Fig 9(b),10699,307.92,1.19\nSR4,Fig 9(c),101,278.83,1.22\nSR4,Fig 9(c),280,279.74,1.22\nSR4,Fig 9(c),458,280.96,1.33\nSR4,Fig 9(c),637,281.49,1.33\nSR4,Fig 9(c),816,281.59,1.33\nSR4,Fig 9(c),995,282.21,0.92\nSR4,Fig 9(c),1174,282.45,1.22\nSR4,Fig 9(c),1353,283.05,1.22\nSR4,Fig 9(c),1532,283.01,1.53\nSR4,Fig 9(c),1711,283.52,1.02\nSR4,Fig 9(c),1889,283.24,1.33\nSR4,Fig 9(c),2068,283.64,1.22\nSR4,Fig 9(c),2247,283.55,1.02\nSR4,Fig 9(c),2426,283.65,0.92\nSR4,Fig 9(c),2605,283.53,1.43\nSR4,Fig 9(c),2784,283.41,1.53\nSR4,Fig 9(c),2963,283.47,1.33\nSR4,Fig 9(c),3142,283.30,1.02\nSR4,Fig 9(c),3320,283.65,1.22\nSR4,Fig 9(c),3499,283.53,1.22\nSR4,Fig 9(c),3689,283.51,1.22\nSR4,Fig 9(c),3880,283.80,1.22\nSR4,Fig 9(c),4058,283.84,1.22\nSR4,Fig 9(c),4237,283.78,1.22\nSR4,Fig 9(c),4416,283.99,1.33\nSR4,Fig 9(c),4595,284.38,1.02\nSR4,Fig 9(c),4774,284.26,1.22\nSR4,Fig 9(c),4953,284.72,1.33\nSR4,Fig 9(c),5132,284.95,1.22\nSR4,Fig 9(c),5311,285.14,1.22\nSR4,Fig 9(c),5489,285.60,1.02\nSR4,Fig 9(c),5668,285.59,1.02\nSR4,Fig 9(c),5847,286.26,1.02\nSR4,Fig 9(c),6026,286.63,1.12\nSR4,Fig 9(c),6205,287.19,1.22\nSR4,Fig 9(c),6384,287.14,1.22\nSR4,Fig 9(c),6563,287.63,1.53\nSR4,Fig 9(c),6742,288.29,1.02\nSR4,Fig 9(c),6920,288.25,1.22\nSR4,Fig 9(c),7099,288.83,1.02\nSR4,Fig 9(c),7289,289.23,1.33\nSR4,Fig 9(c),7480,289.76,1.22\nSR4,Fig 9(c),7658,289.94,1.22\nSR4,Fig 9(c),7837,290.46,0.92\nSR4,Fig 9(c),8016,291.03,1.53\nSR4,Fig 9(c),8195,291.35,1.33\nSR4,Fig 9(c),8374,291.88,1.53\nSR4,Fig 9(c),8553,292.59,1.33\nSR4,Fig 9(c),8732,292.70,1.33\nSR4,Fig 9(c),8911,293.51,1.02\nSR4,Fig 9(c),9089,293.67,1.53\nSR4,Fig 9(c),9268,294.03,0.92\nSR4,Fig 9(c),9447,294.27,1.22\nSR4,Fig 9(c),9626,294.91,1.22\nSR4,Fig 9(c),9805,295.50,1.02\nSR4,Fig 9(c),9984,295.94,0.71\nSR4,Fig 9(c),10163,296.39,1.22\nSR4,Fig 9(c),10342,296.68,1.22\nSR4,Fig 9(c),10520,297.08,0.71\nSR4,Fig 9(c),10699,297.86,0.92\nSR3,Fig 9(d),101,264.77,1.22\nSR3,Fig 9(d),280,264.04,1.53\nSR3,Fig 9(d),459,264.29,1.53\nSR3,Fig 9(d),639,264.68,1.22\nSR3,Fig 9(d),818,264.94,0.92\nSR3,Fig 9(d),997,265.14,0.92\nSR3,Fig 9(d),1176,265.70,1.22\nSR3,Fig 9(d),1356,265.99,1.53\nSR3,Fig 9(d),1535,266.65,1.53\nSR3,Fig 9(d),1714,266.76,1.33\nSR3,Fig 9(d),1893,267.22,1.33\nSR3,Fig 9(d),2073,267.77,1.02\nSR3,Fig 9(d),2252,267.74,1.53\nSR3,Fig 9(d),2431,268.14,1.02\nSR3,Fig 9(d),2610,268.69,1.33\nSR3,Fig 9(d),2790,268.78,1.33\nSR3,Fig 9(d),2969,268.99,1.53\nSR3,Fig 9(d),3148,269.43,1.22\nSR3,Fig 9(d),3327,269.64,0.92\nSR3,Fig 9(d),3507,269.80,1.02\nSR3,Fig 9(d),3686,269.94,1.02\nSR3,Fig 9(d),3865,270.28,0.92\nSR3,Fig 9(d),4044,270.33,1.22\nSR3,Fig 9(d),4224,270.93,1.53\nSR3,Fig 9(d),4403,270.83,1.02\nSR3,Fig 9(d),4582,271.15,1.22\nSR3,Fig 9(d),4761,271.22,1.22\nSR3,Fig 9(d),4941,271.62,1.22\nSR3,Fig 9(d),5120,271.33,1.33\nSR3,Fig 9(d),5299,271.57,1.33\nSR3,Fig 9(d),5490,271.78,1.33\nSR3,Fig 9(d),5680,272.42,0.71\nSR3,Fig 9(d),5859,272.33,1.02\nSR3,Fig 9(d),6039,272.73,1.02\nSR3,Fig 9(d),6218,272.54,1.02\nSR3,Fig 9(d),6397,272.92,1.33\nSR3,Fig 9(d),6576,273.29,1.22\nSR3,Fig 9(d),6756,273.48,1.33\nSR3,Fig 9(d),6935,273.64,1.53\nSR3,Fig 9(d),7114,273.78,1.43\nSR3,Fig 9(d),7293,274.18,1.22\nSR3,Fig 9(d),7473,274.49,0.71\nSR3,Fig 9(d),7652,274.52,1.33\nSR3,Fig 9(d),7831,275.09,1.02\nSR3,Fig 9(d),8010,275.17,1.63\nSR3,Fig 9(d),8190,275.65,1.33\nSR3,Fig 9(d),8369,275.85,1.33\nSR3,Fig 9(d),8548,276.26,1.02\nSR3,Fig 9(d),8727,276.30,1.02\nSR3,Fig 9(d),8907,276.66,2.14\nSR3,Fig 9(d),9086,277.05,1.22\nSR3,Fig 9(d),9265,277.35,1.02\nSR3,Fig 9(d),9444,277.47,1.22\nSR3,Fig 9(d),9624,278.11,0.71\nSR3,Fig 9(d),9803,278.09,1.22\nSR3,Fig 9(d),9982,278.19,1.63\nSR3,Fig 9(d),10161,278.57,1.33\nSR3,Fig 9(d),10341,278.86,0.82\nSR3,Fig 9(d),10520,279.31,0.92\nSR3,Fig 9(d),10699,279.66,1.22\nSR2,Fig 9(e),101,244.17,0.67\nSR2,Fig 9(e),280,244.27,0.92\nSR2,Fig 9(e),458,244.69,1.23\nSR2,Fig 9(e),637,244.55,0.92\nSR2,Fig 9(e),816,245.08,0.87\nSR2,Fig 9(e),995,244.81,1.18\nSR2,Fig 9(e),1174,245.08,0.92\nSR2,Fig 9(e),1353,245.15,0.92\nSR2,Fig 9(e),1532,245.25,0.92\nSR2,Fig 9(e),1711,245.23,0.97\nSR2,Fig 9(e),1889,244.93,0.92\nSR2,Fig 9(e),2068,245.27,1.18\nSR2,Fig 9(e),2247,245.48,0.67\nSR2,Fig 9(e),2426,245.43,0.92\nSR2,Fig 9(e),2605,245.68,0.92\nSR2,Fig 9(e),2784,246.07,0.67\nSR2,Fig 9(e),2963,245.92,1.23\nSR2,Fig 9(e),3142,246.14,1.23\nSR2,Fig 9(e),3320,246.28,0.97\nSR2,Fig 9(e),3499,246.42,0.92\nSR2,Fig 9(e),3689,246.17,1.23\nSR2,Fig 9(e),3880,246.47,0.67\nSR2,Fig 9(e),4058,246.54,0.67\nSR2,Fig 9(e),4237,246.58,0.97\nSR2,Fig 9(e),4416,246.94,0.92\nSR2,Fig 9(e),4595,246.74,0.87\nSR2,Fig 9(e),4774,247.09,1.23\nSR2,Fig 9(e),4953,246.93,1.54\nSR2,Fig 9(e),5132,247.22,1.18\nSR2,Fig 9(e),5311,247.24,1.48\nSR2,Fig 9(e),5489,247.21,0.92\nSR2,Fig 9(e),5668,247.35,0.92\nSR2,Fig 9(e),5847,247.44,1.23\nSR2,Fig 9(e),6026,247.69,1.23\nSR2,Fig 9(e),6205,247.83,1.23\nSR2,Fig 9(e),6384,247.91,0.87\nSR2,Fig 9(e),6563,248.15,1.23\nSR2,Fig 9(e),6742,248.31,1.23\nSR2,Fig 9(e),6920,248.60,1.23\nSR2,Fig 9(e),7099,248.64,0.92\nSR2,Fig 9(e),7289,248.29,0.97\nSR2,Fig 9(e),7480,248.57,1.23\nSR2,Fig 9(e),7658,248.29,1.23\nSR2,Fig 9(e),7837,248.68,0.97\nSR2,Fig 9(e),8016,248.46,1.18\nSR2,Fig 9(e),8195,248.58,0.67\nSR2,Fig 9(e),8374,248.65,1.28\nSR2,Fig 9(e),8553,248.77,0.97\nSR2,Fig 9(e),8732,249.02,1.18\nSR2,Fig 9(e),8911,249.04,0.97\nSR2,Fig 9(e),9089,249.13,0.92\nSR2,Fig 9(e),9268,249.35,0.92\nSR2,Fig 9(e),9447,249.48,0.92\nSR2,Fig 9(e),9626,249.44,0.97\nSR2,Fig 9(e),9805,249.51,0.92\nSR2,Fig 9(e),9984,249.75,0.67\nSR2,Fig 9(e),10163,249.98,1.28\nSR2,Fig 9(e),10342,250.01,1.28\nSR2,Fig 9(e),10520,250.10,0.67\nSR2,Fig 9(e),10699,250.01,0.97\nSR1,Fig 9(f),101,219.23,1.19\nSR1,Fig 9(f),280,219.17,0.94\nSR1,Fig 9(f),459,219.02,1.54\nSR1,Fig 9(f),639,218.47,1.54\nSR1,Fig 9(f),818,218.44,1.49\nSR1,Fig 9(f),997,218.02,0.94\nSR1,Fig 9(f),1176,217.85,1.28\nSR1,Fig 9(f),1356,218.04,0.98\nSR1,Fig 9(f),1535,217.80,1.24\nSR1,Fig 9(f),1714,217.79,1.24\nSR1,Fig 9(f),1893,217.53,1.24\nSR1,Fig 9(f),2073,217.53,1.19\nSR1,Fig 9(f),2252,217.52,0.98\nSR1,Fig 9(f),2431,217.17,1.54\nSR1,Fig 9(f),2610,217.19,1.54\nSR1,Fig 9(f),2790,217.35,0.98\nSR1,Fig 9(f),2969,217.34,1.19\nSR1,Fig 9(f),3148,216.92,1.54\nSR1,Fig 9(f),3327,217.06,1.54\nSR1,Fig 9(f),3507,216.97,1.24\nSR1,Fig 9(f),3686,216.80,1.24\nSR1,Fig 9(f),3865,216.87,0.94\nSR1,Fig 9(f),4044,216.56,0.90\nSR1,Fig 9(f),4224,216.56,1.24\nSR1,Fig 9(f),4403,216.77,1.83\nSR1,Fig 9(f),4582,217.00,0.90\nSR1,Fig 9(f),4761,216.61,1.28\nSR1,Fig 9(f),4941,216.26,0.90\nSR1,Fig 9(f),5120,216.34,0.94\nSR1,Fig 9(f),5299,216.54,1.28\nSR1,Fig 9(f),5490,216.46,1.83\nSR1,Fig 9(f),5680,216.34,0.98\nSR1,Fig 9(f),5859,216.54,1.24\nSR1,Fig 9(f),6039,216.55,1.24\nSR1,Fig 9(f),6218,217.04,0.94\nSR1,Fig 9(f),6397,216.52,1.24\nSR1,Fig 9(f),6576,216.56,1.24\nSR1,Fig 9(f),6756,216.77,0.94\nSR1,Fig 9(f),6935,216.61,1.49\nSR1,Fig 9(f),7114,216.66,1.49\nSR1,Fig 9(f),7293,216.57,1.49\nSR1,Fig 9(f),7473,216.71,1.19\nSR1,Fig 9(f),7652,216.40,0.94\nSR1,Fig 9(f),7831,216.19,1.24\nSR1,Fig 9(f),8010,216.23,1.24\nSR1,Fig 9(f),8190,216.19,0.94\nSR1,Fig 9(f),8369,216.20,0.64\nSR1,Fig 9(f),8548,216.38,0.94\nSR1,Fig 9(f),8727,216.35,0.94\nSR1,Fig 9(f),8907,216.55,0.94\nSR1,Fig 9(f),9086,216.40,1.07\nSR1,Fig 9(f),9265,216.11,1.24\nSR1,Fig 9(f),9444,215.91,1.28\nSR1,Fig 9(f),9624,215.86,0.94\nSR1,Fig 9(f),9803,215.96,1.24\nSR1,Fig 9(f),9982,216.03,0.94\nSR1,Fig 9(f),10161,215.95,0.68\nSR1,Fig 9(f),10341,215.72,1.24\nSR1,Fig 9(f),10520,216.13,0.81\nSR1,Fig 9(f),10699,216.28,1.54\nSR12,Fig 10(a),101,462.66,2.22\nSR12,Fig 10(a),281,458.69,2.22\nSR12,Fig 10(a),460,454.77,2.05\nSR12,Fig 10(a),640,450.74,2.56\nSR12,Fig 10(a),820,446.51,2.39\nSR12,Fig 10(a),999,442.38,2.39\nSR12,Fig 10(a),1179,438.28,2.05\nSR12,Fig 10(a),1358,434.51,2.05\nSR12,Fig 10(a),1538,430.64,2.05\nSR12,Fig 10(a),1718,427.25,2.05\nSR12,Fig 10(a),1897,423.54,1.54\nSR12,Fig 10(a),2077,420.19,2.05\nSR12,Fig 10(a),2257,417.16,1.88\nSR12,Fig 10(a),2436,413.98,1.71\nSR12,Fig 10(a),2616,411.19,1.37\nSR12,Fig 10(a),2795,408.59,1.37\nSR12,Fig 10(a),2975,406.18,1.19\nSR12,Fig 10(a),3155,403.68,1.37\nSR12,Fig 10(a),3334,401.72,1.02\nSR12,Fig 10(a),3514,399.42,1.71\nSR12,Fig 10(a),3694,397.73,0.85\nSR12,Fig 10(a),3873,395.98,1.54\nSR12,Fig 10(a),4053,394.08,1.02\nSR12,Fig 10(a),4232,392.57,1.02\nSR12,Fig 10(a),4412,391.93,0.68\nSR12,Fig 10(a),4592,391.93,0.68\nSR12,Fig 10(a),4771,391.95,0.34\nSR12,Fig 10(a),4951,391.40,0.68\nSR12,Fig 10(a),5131,390.71,0.68\nSR12,Fig 10(a),5310,389.88,0.68\nSR12,Fig 10(a),5490,389.26,0.85\nSR12,Fig 10(a),5669,389.01,0.51\nSR12,Fig 10(a),5849,389.03,0.68\nSR12,Fig 10(a),6029,388.86,0.68\nSR12,Fig 10(a),6208,388.49,0.51\nSR12,Fig 10(a),6388,388.20,0.68\nSR12,Fig 10(a),6568,387.98,0.34\nSR12,Fig 10(a),6747,387.98,0.68\nSR12,Fig 10(a),6927,387.85,0.68\nSR12,Fig 10(a),7106,387.79,0.85\nSR12,Fig 10(a),7286,387.85,0.34\nSR12,Fig 10(a),7466,387.62,0.68\nSR12,Fig 10(a),7645,387.51,0.68\nSR12,Fig 10(a),7825,387.38,0.51\nSR12,Fig 10(a),8005,387.41,0.68\nSR12,Fig 10(a),8184,387.43,0.68\nSR12,Fig 10(a),8364,387.43,0.51\nSR12,Fig 10(a),8543,387.41,0.51\nSR12,Fig 10(a),8723,387.36,0.51\nSR12,Fig 10(a),8903,387.28,0.51\nSR12,Fig 10(a),9082,387.21,0.68\nSR12,Fig 10(a),9262,387.30,0.51\nSR12,Fig 10(a),9442,387.02,0.34\nSR12,Fig 10(a),9621,387.26,0.68\nSR12,Fig 10(a),9801,387.15,0.51\nSR12,Fig 10(a),9980,387.24,0.68\nSR12,Fig 10(a),10160,387.15,0.68\nSR12,Fig 10(a),10340,386.92,0.51\nSR12,Fig 10(a),10519,387.11,0.68\nSR12,Fig 10(a),10699,386.96,0.34\nSR11,Fig 10(b),101,371.02,0.61\nSR11,Fig 10(b),281,371.19,0.68\nSR11,Fig 10(b),460,371.37,0.68\nSR11,Fig 10(b),640,371.97,0.68\nSR11,Fig 10(b),820,372.29,0.55\nSR11,Fig 10(b),999,372.20,0.96\nSR11,Fig 10(b),1179,372.29,0.41\nSR11,Fig 10(b),1358,372.19,0.34\nSR11,Fig 10(b),1538,372.01,0.68\nSR11,Fig 10(b),1718,371.93,0.61\nSR11,Fig 10(b),1897,371.52,0.61\nSR11,Fig 10(b),2077,371.30,0.61\nSR11,Fig 10(b),2257,370.93,0.68\nSR11,Fig 10(b),2436,370.42,0.68\nSR11,Fig 10(b),2616,369.93,0.41\nSR11,Fig 10(b),2795,369.44,0.61\nSR11,Fig 10(b),2975,368.99,0.61\nSR11,Fig 10(b),3155,368.52,0.68\nSR11,Fig 10(b),3334,367.88,0.89\nSR11,Fig 10(b),3514,367.41,0.61\nSR11,Fig 10(b),3694,366.95,0.61\nSR11,Fig 10(b),3873,366.48,0.41\nSR11,Fig 10(b),4053,366.08,0.68\nSR11,Fig 10(b),4232,365.50,0.61\nSR11,Fig 10(b),4412,365.20,0.34\nSR11,Fig 10(b),4592,364.69,0.68\nSR11,Fig 10(b),4771,364.48,0.61\nSR11,Fig 10(b),4951,364.10,0.61\nSR11,Fig 10(b),5131,363.80,0.61\nSR11,Fig 10(b),5310,363.53,0.89\nSR11,Fig 10(b),5490,363.31,0.61\nSR11,Fig 10(b),5669,363.06,0.61\nSR11,Fig 10(b),5849,362.89,0.96\nSR11,Fig 10(b),6029,362.74,0.61\nSR11,Fig 10(b),6208,362.53,0.68\nSR11,Fig 10(b),6388,362.41,0.41\nSR11,Fig 10(b),6568,362.39,0.41\nSR11,Fig 10(b),6747,362.29,0.61\nSR11,Fig 10(b),6927,362.12,0.61\nSR11,Fig 10(b),7106,362.05,0.61\nSR11,Fig 10(b),7286,361.87,0.34\nSR11,Fig 10(b),7466,361.87,0.61\nSR11,Fig 10(b),7645,361.75,0.68\nSR11,Fig 10(b),7825,361.76,0.34\nSR11,Fig 10(b),8005,361.70,0.61\nSR11,Fig 10(b),8184,361.64,0.68\nSR11,Fig 10(b),8364,361.43,0.61\nSR11,Fig 10(b),8543,361.52,0.61\nSR11,Fig 10(b),8723,361.58,0.61\nSR11,Fig 10(b),8903,361.47,0.34\nSR11,Fig 10(b),9082,361.25,0.61\nSR11,Fig 10(b),9262,361.20,0.34\nSR11,Fig 10(b),9442,361.36,0.34\nSR11,Fig 10(b),9621,361.13,0.61\nSR11,Fig 10(b),9801,361.20,0.61\nSR11,Fig 10(b),9980,361.32,0.34\nSR11,Fig 10(b),10160,361.05,0.61\nSR11,Fig 10(b),10340,360.93,0.41\nSR11,Fig 10(b),10519,361.12,0.34\nSR11,Fig 10(b),10699,361.02,0.61\nSR10,Fig 10(c),101,341.66,1.19\nSR10,Fig 10(c),281,343.05,1.43\nSR10,Fig 10(c),460,344.83,1.31\nSR10,Fig 10(c),640,346.10,1.25\nSR10,Fig 10(c),820,347.00,0.96\nSR10,Fig 10(c),999,348.09,0.60\nSR10,Fig 10(c),1179,348.60,1.19\nSR10,Fig 10(c),1358,349.38,1.43\nSR10,Fig 10(c),1538,349.87,1.19\nSR10,Fig 10(c),1718,350.70,0.90\nSR10,Fig 10(c),1897,351.03,1.19\nSR10,Fig 10(c),2077,351.21,0.96\nSR10,Fig 10(c),2257,351.45,0.90\nSR10,Fig 10(c),2436,351.85,0.90\nSR10,Fig 10(c),2616,351.67,0.96\nSR10,Fig 10(c),2795,351.85,0.90\nSR10,Fig 10(c),2975,352.14,0.96\nSR10,Fig 10(c),3155,352.20,1.25\nSR10,Fig 10(c),3334,352.42,0.96\nSR10,Fig 10(c),3514,352.08,1.19\nSR10,Fig 10(c),3694,352.26,1.19\nSR10,Fig 10(c),3873,352.29,1.25\nSR10,Fig 10(c),4053,352.32,0.96\nSR10,Fig 10(c),4232,352.29,0.90\nSR10,Fig 10(c),4412,352.21,1.49\nSR10,Fig 10(c),4592,352.45,0.90\nSR10,Fig 10(c),4771,352.17,0.96\nSR10,Fig 10(c),4951,351.89,0.96\nSR10,Fig 10(c),5131,351.88,1.19\nSR10,Fig 10(c),5310,351.96,0.90\nSR10,Fig 10(c),5490,351.80,1.19\nSR10,Fig 10(c),5669,352.00,0.96\nSR10,Fig 10(c),5849,351.99,0.90\nSR10,Fig 10(c),6029,351.93,0.90\nSR10,Fig 10(c),6208,351.67,1.25\nSR10,Fig 10(c),6388,351.49,0.66\nSR10,Fig 10(c),6568,351.63,0.96\nSR10,Fig 10(c),6747,351.78,0.96\nSR10,Fig 10(c),6927,351.55,0.66\nSR10,Fig 10(c),7106,351.41,1.19\nSR10,Fig 10(c),7286,351.53,0.90\nSR10,Fig 10(c),7466,351.64,0.60\nSR10,Fig 10(c),7645,351.46,0.60\nSR10,Fig 10(c),7825,351.22,0.90\nSR10,Fig 10(c),8005,351.39,0.96\nSR10,Fig 10(c),8184,351.61,0.66\nSR10,Fig 10(c),8364,351.51,0.66\nSR10,Fig 10(c),8543,351.55,0.96\nSR10,Fig 10(c),8723,351.43,0.90\nSR10,Fig 10(c),8903,351.59,0.96\nSR10,Fig 10(c),9082,351.43,0.96\nSR10,Fig 10(c),9262,351.09,0.90\nSR10,Fig 10(c),9442,351.15,0.90\nSR10,Fig 10(c),9621,351.19,1.19\nSR10,Fig 10(c),9801,351.31,1.19\nSR10,Fig 10(c),9980,351.29,0.60\nSR10,Fig 10(c),10160,351.10,1.19\nSR10,Fig 10(c),10340,351.22,1.19\nSR10,Fig 10(c),10519,351.12,0.66\nSR10,Fig 10(c),10699,351.43,0.90\nSR9,Fig 10(d),101,314.64,0.92\nSR9,Fig 10(d),281,314.06,1.00\nSR9,Fig 10(d),460,314.06,1.23\nSR9,Fig 10(d),640,314.35,1.23\nSR9,Fig 10(d),820,315.11,0.92\nSR9,Fig 10(d),999,315.48,0.92\nSR9,Fig 10(d),1179,316.13,1.23\nSR9,Fig 10(d),1358,316.82,0.84\nSR9,Fig 10(d),1538,317.56,1.23\nSR9,Fig 10(d),1718,317.93,1.15\nSR9,Fig 10(d),1897,318.61,0.92\nSR9,Fig 10(d),2077,319.06,1.31\nSR9,Fig 10(d),2257,319.87,1.46\nSR9,Fig 10(d),2436,320.59,0.61\nSR9,Fig 10(d),2616,321.14,1.23\nSR9,Fig 10(d),2795,321.59,0.92\nSR9,Fig 10(d),2975,321.79,0.61\nSR9,Fig 10(d),3155,322.49,1.23\nSR9,Fig 10(d),3334,323.03,1.23\nSR9,Fig 10(d),3514,323.67,1.23\nSR9,Fig 10(d),3694,323.71,1.15\nSR9,Fig 10(d),3873,324.36,0.92\nSR9,Fig 10(d),4053,324.57,1.23\nSR9,Fig 10(d),4232,325.24,0.92\nSR9,Fig 10(d),4412,325.38,0.92\nSR9,Fig 10(d),4592,325.70,1.23\nSR9,Fig 10(d),4771,326.02,1.23\nSR9,Fig 10(d),4951,326.17,1.23\nSR9,Fig 10(d),5131,326.32,0.92\nSR9,Fig 10(d),5310,326.64,0.92\nSR9,Fig 10(d),5490,326.77,0.61\nSR9,Fig 10(d),5669,327.19,0.92\nSR9,Fig 10(d),5849,327.34,0.92\nSR9,Fig 10(d),6029,327.40,0.92\nSR9,Fig 10(d),6208,327.40,0.92\nSR9,Fig 10(d),6388,327.66,1.23\nSR9,Fig 10(d),6568,328.10,1.46\nSR9,Fig 10(d),6747,328.25,1.23\nSR9,Fig 10(d),6927,328.56,1.23\nSR9,Fig 10(d),7106,328.55,0.92\nSR9,Fig 10(d),7286,328.58,0.92\nSR9,Fig 10(d),7466,328.58,0.92\nSR9,Fig 10(d),7645,328.96,1.00\nSR9,Fig 10(d),7825,328.92,0.92\nSR9,Fig 10(d),8005,328.97,0.92\nSR9,Fig 10(d),8184,329.19,0.92\nSR9,Fig 10(d),8364,329.36,0.92\nSR9,Fig 10(d),8543,329.23,1.23\nSR9,Fig 10(d),8723,329.52,0.92\nSR9,Fig 10(d),8903,329.24,1.15\nSR9,Fig 10(d),9082,329.59,1.23\nSR9,Fig 10(d),9262,329.74,0.92\nSR9,Fig 10(d),9442,329.92,0.92\nSR9,Fig 10(d),9621,329.87,0.92\nSR9,Fig 10(d),9801,329.91,0.92\nSR9,Fig 10(d),9980,329.84,0.92\nSR9,Fig 10(d),10160,330.08,0.61\nSR9,Fig 10(d),10340,330.40,0.92\nSR9,Fig 10(d),10519,330.23,0.92\nSR9,Fig 10(d),10699,329.91,1.00\nSR8,Fig 10(e),101,267.94,0.97\nSR8,Fig 10(e),281,268.47,0.92\nSR8,Fig 10(e),460,268.63,1.18\nSR8,Fig 10(e),640,268.66,1.13\nSR8,Fig 10(e),820,269.06,0.87\nSR8,Fig 10(e),999,269.09,1.33\nSR8,Fig 10(e),1179,269.23,1.18\nSR8,Fig 10(e),1358,269.30,1.23\nSR8,Fig 10(e),1538,269.36,1.23\nSR8,Fig 10(e),1718,269.81,0.61\nSR8,Fig 10(e),1897,269.45,1.13\nSR8,Fig 10(e),2077,269.72,1.23\nSR8,Fig 10(e),2257,269.98,0.92\nSR8,Fig 10(e),2436,270.24,1.23\nSR8,Fig 10(e),2616,270.58,0.92\nSR8,Fig 10(e),2795,270.84,0.92\nSR8,Fig 10(e),2975,270.74,0.92\nSR8,Fig 10(e),3155,270.96,1.23\nSR8,Fig 10(e),3334,271.19,0.97\nSR8,Fig 10(e),3514,271.47,0.92\nSR8,Fig 10(e),3694,271.72,1.18\nSR8,Fig 10(e),3873,271.86,1.28\nSR8,Fig 10(e),4053,272.20,0.92\nSR8,Fig 10(e),4232,272.48,0.92\nSR8,Fig 10(e),4412,272.66,1.18\nSR8,Fig 10(e),4592,272.80,1.18\nSR8,Fig 10(e),4771,273.35,0.92\nSR8,Fig 10(e),4951,273.55,0.67\nSR8,Fig 10(e),5131,273.89,1.23\nSR8,Fig 10(e),5310,273.85,1.23\nSR8,Fig 10(e),5490,274.25,0.92\nSR8,Fig 10(e),5669,274.08,0.92\nSR8,Fig 10(e),5849,274.44,0.87\nSR8,Fig 10(e),6029,274.85,0.92\nSR8,Fig 10(e),6208,275.31,1.54\nSR8,Fig 10(e),6388,275.32,0.97\nSR8,Fig 10(e),6568,275.69,1.18\nSR8,Fig 10(e),6747,276.04,0.92\nSR8,Fig 10(e),6927,276.27,0.67\nSR8,Fig 10(e),7106,276.46,0.97\nSR8,Fig 10(e),7286,276.33,1.02\nSR8,Fig 10(e),7466,276.56,0.92\nSR8,Fig 10(e),7645,276.75,0.92\nSR8,Fig 10(e),7825,277.14,1.23\nSR8,Fig 10(e),8005,277.23,1.23\nSR8,Fig 10(e),8184,277.16,0.97\nSR8,Fig 10(e),8364,277.53,0.72\nSR8,Fig 10(e),8543,277.70,1.23\nSR8,Fig 10(e),8723,278.05,0.67\nSR8,Fig 10(e),8903,278.02,0.92\nSR8,Fig 10(e),9082,278.37,0.97\nSR8,Fig 10(e),9262,278.41,0.92\nSR8,Fig 10(e),9442,278.67,0.92\nSR8,Fig 10(e),9621,278.49,1.23\nSR8,Fig 10(e),9801,278.96,1.23\nSR8,Fig 10(e),9980,278.99,1.23\nSR8,Fig 10(e),10160,279.48,1.23\nSR8,Fig 10(e),10340,279.59,0.97\nSR8,Fig 10(e),10519,279.68,0.97\nSR8,Fig 10(e),10699,279.75,0.92\nSR7,Fig 10(f),101,222.41,1.23\nSR7,Fig 10(f),281,222.66,0.92\nSR7,Fig 10(f),460,222.86,0.87\nSR7,Fig 10(f),640,223.07,1.23\nSR7,Fig 10(f),820,222.78,1.54\nSR7,Fig 10(f),999,223.02,0.92\nSR7,Fig 10(f),1179,223.12,1.23\nSR7,Fig 10(f),1358,222.96,1.54\nSR7,Fig 10(f),1538,222.84,0.97\nSR7,Fig 10(f),1718,223.03,1.28\nSR7,Fig 10(f),1897,222.98,1.48\nSR7,Fig 10(f),2077,222.74,1.23\nSR7,Fig 10(f),2257,223.08,0.92\nSR7,Fig 10(f),2436,222.88,0.97\nSR7,Fig 10(f),2616,222.83,1.23\nSR7,Fig 10(f),2795,222.74,1.23\nSR7,Fig 10(f),2975,222.91,1.79\nSR7,Fig 10(f),3155,223.07,1.18\nSR7,Fig 10(f),3334,222.73,1.54\nSR7,Fig 10(f),3514,222.87,1.54\nSR7,Fig 10(f),3694,223.01,1.54\nSR7,Fig 10(f),3873,223.05,1.23\nSR7,Fig 10(f),4053,223.03,1.48\nSR7,Fig 10(f),4232,223.30,1.28\nSR7,Fig 10(f),4412,222.96,1.54\nSR7,Fig 10(f),4592,223.22,1.23\nSR7,Fig 10(f),4771,222.91,1.28\nSR7,Fig 10(f),4951,223.07,1.18\nSR7,Fig 10(f),5131,223.34,1.23\nSR7,Fig 10(f),5310,223.26,1.23\nSR7,Fig 10(f),5490,223.12,1.54\nSR7,Fig 10(f),5669,223.14,1.02\nSR7,Fig 10(f),5849,223.26,1.23\nSR7,Fig 10(f),6029,223.15,1.54\nSR7,Fig 10(f),6208,223.41,0.92\nSR7,Fig 10(f),6388,223.34,1.54\nSR7,Fig 10(f),6568,223.62,1.54\nSR7,Fig 10(f),6747,223.42,0.92\nSR7,Fig 10(f),6927,223.42,1.54\nSR7,Fig 10(f),7106,223.21,1.28\nSR7,Fig 10(f),7286,223.62,0.92\nSR7,Fig 10(f),7466,223.92,0.97\nSR7,Fig 10(f),7645,223.86,1.23\nSR7,Fig 10(f),7825,224.17,0.92\nSR7,Fig 10(f),8005,223.77,1.23\nSR7,Fig 10(f),8184,223.75,1.54\nSR7,Fig 10(f),8364,223.83,1.28\nSR7,Fig 10(f),8543,224.16,1.54\nSR7,Fig 10(f),8723,223.98,1.18\nSR7,Fig 10(f),8903,224.13,1.23\nSR7,Fig 10(f),9082,224.05,1.18\nSR7,Fig 10(f),9262,223.85,1.18\nSR7,Fig 10(f),9442,223.72,1.28\nSR7,Fig 10(f),9621,223.69,1.23\nSR7,Fig 10(f),9801,223.94,0.97\nSR7,Fig 10(f),9980,223.42,0.97\nSR7,Fig 10(f),10160,223.91,0.92\nSR7,Fig 10(f),10340,223.69,0.67\nSR7,Fig 10(f),10519,223.56,0.92\nSR7,Fig 10(f),10699,224.22,1.23\nBR1_BR3,Fig 11(a),92,642.82,2.26\nBR1_BR3,Fig 11(a),268,643.34,1.81\nBR1_BR3,Fig 11(a),452,640.86,3.61\nBR1_BR3,Fig 11(a),637,634.22,3.61\nBR1_BR3,Fig 11(a),812,630.07,4.07\nBR1_BR3,Fig 11(a),988,622.52,4.07\nBR1_BR3,Fig 11(a),1172,616.19,3.61\nBR1_BR3,Fig 11(a),1357,610.05,4.52\nBR1_BR3,Fig 11(a),1532,603.71,3.16\nBR1_BR3,Fig 11(a),1708,597.98,4.07\nBR1_BR3,Fig 11(a),1892,592.56,3.16\nBR1_BR3,Fig 11(a),2068,587.10,3.61\nBR1_BR3,Fig 11(a),2243,582.67,3.16\nBR1_BR3,Fig 11(a),2428,576.34,4.07\nBR1_BR3,Fig 11(a),2612,570.78,4.07\nBR1_BR3,Fig 11(a),2797,564.95,3.16\nBR1_BR3,Fig 11(a),2972,559.59,4.07\nBR1_BR3,Fig 11(a),3148,553.48,3.16\nBR1_BR3,Fig 11(a),3332,548.69,3.61\nBR1_BR3,Fig 11(a),3517,542.86,4.52\nBR1_BR3,Fig 11(a),3692,537.05,3.16\nBR1_BR3,Fig 11(a),3868,532.52,3.61\nBR1_BR3,Fig 11(a),4052,526.69,3.16\nBR1_BR3,Fig 11(a),4228,521.69,3.16\nBR1_BR3,Fig 11(a),4403,517.15,4.07\nBR1_BR3,Fig 11(a),4588,511.96,3.16\nBR1_BR3,Fig 11(a),4772,507.76,2.71\nBR1_BR3,Fig 11(a),4948,504.62,2.26\nBR1_BR3,Fig 11(a),5123,500.17,3.16\nBR1_BR3,Fig 11(a),5308,496.05,3.16\nBR1_BR3,Fig 11(a),5492,492.21,2.26\nBR1_BR3,Fig 11(a),5677,488.96,2.71\nBR1_BR3,Fig 11(a),5852,485.54,2.71\nBR1_BR3,Fig 11(a),6028,482.41,2.26\nBR1_BR3,Fig 11(a),6212,479.74,2.26\nBR1_BR3,Fig 11(a),6397,476.76,2.26\nBR1_BR3,Fig 11(a),6572,474.30,1.81\nBR1_BR3,Fig 11(a),6748,471.75,2.26\nBR1_BR3,Fig 11(a),6932,468.99,2.26\nBR1_BR3,Fig 11(a),7117,466.55,1.81\nBR1_BR3,Fig 11(a),7292,464.21,1.81\nBR1_BR3,Fig 11(a),7468,462.21,2.26\nBR1_BR3,Fig 11(a),7652,460.05,1.81\nBR1_BR3,Fig 11(a),7837,457.79,1.36\nBR1_BR3,Fig 11(a),8012,456.07,1.81\nBR1_BR3,Fig 11(a),8188,454.08,1.36\nBR1_BR3,Fig 11(a),8372,452.23,2.26\nBR1_BR3,Fig 11(a),8548,450.45,1.36\nBR1_BR3,Fig 11(a),8723,448.84,2.26\nBR1_BR3,Fig 11(a),8908,446.81,1.81\nBR1_BR3,Fig 11(a),9092,445.05,1.36\nBR1_BR3,Fig 11(a),9268,443.12,1.81\nBR1_BR3,Fig 11(a),9443,441.57,1.36\nBR1_BR3,Fig 11(a),9628,439.80,1.81\nBR1_BR3,Fig 11(a),9812,438.40,1.36\nBR1_BR3,Fig 11(a),9988,436.70,1.81\nBR1_BR3,Fig 11(a),10163,435.33,1.81\nBR1_BR3,Fig 11(a),10348,433.89,1.81\nBR1_BR3,Fig 11(a),10532,432.30,1.36\nBR1_BR3,Fig 11(a),10708,431.07,1.81\nBR2_BR4,Fig 11(b),92,747.97,37.27\nBR2_BR4,Fig 11(b),268,723.54,7.45\nBR2_BR4,Fig 11(b),452,712.02,6.10\nBR2_BR4,Fig 11(b),637,702.39,6.10\nBR2_BR4,Fig 11(b),812,692.47,5.42\nBR2_BR4,Fig 11(b),988,683.08,6.78\nBR2_BR4,Fig 11(b),1172,674.27,5.42\nBR2_BR4,Fig 11(b),1357,666.07,5.42\nBR2_BR4,Fig 11(b),1532,659.64,4.07\nBR2_BR4,Fig 11(b),1708,653.26,6.10\nBR2_BR4,Fig 11(b),1892,644.92,5.42\nBR2_BR4,Fig 11(b),2068,636.97,4.74\nBR2_BR4,Fig 11(b),2243,630.35,4.74\nBR2_BR4,Fig 11(b),2428,623.31,4.74\nBR2_BR4,Fig 11(b),2612,616.60,4.74\nBR2_BR4,Fig 11(b),2797,610.02,4.74\nBR2_BR4,Fig 11(b),2972,603.69,4.07\nBR2_BR4,Fig 11(b),3148,597.35,4.74\nBR2_BR4,Fig 11(b),3332,590.23,4.74\nBR2_BR4,Fig 11(b),3517,583.12,4.74\nBR2_BR4,Fig 11(b),3692,577.48,4.07\nBR2_BR4,Fig 11(b),3868,572.00,4.07\nBR2_BR4,Fig 11(b),4052,566.04,4.07\nBR2_BR4,Fig 11(b),4228,560.24,3.39\nBR2_BR4,Fig 11(b),4403,554.99,4.07\nBR2_BR4,Fig 11(b),4588,549.10,3.39\nBR2_BR4,Fig 11(b),4772,543.88,3.39\nBR2_BR4,Fig 11(b),4948,539.38,4.74\nBR2_BR4,Fig 11(b),5123,534.39,3.39\nBR2_BR4,Fig 11(b),5308,529.44,4.07\nBR2_BR4,Fig 11(b),5492,524.36,3.39\nBR2_BR4,Fig 11(b),5677,520.36,3.39\nBR2_BR4,Fig 11(b),5852,516.34,3.39\nBR2_BR4,Fig 11(b),6028,511.89,3.39\nBR2_BR4,Fig 11(b),6212,507.96,3.39\nBR2_BR4,Fig 11(b),6397,503.96,2.71\nBR2_BR4,Fig 11(b),6572,500.45,2.71\nBR2_BR4,Fig 11(b),6748,496.78,2.71\nBR2_BR4,Fig 11(b),6932,493.66,3.39\nBR2_BR4,Fig 11(b),7117,490.20,3.39\nBR2_BR4,Fig 11(b),7292,487.20,2.71\nBR2_BR4,Fig 11(b),7468,484.04,3.39\nBR2_BR4,Fig 11(b),7652,481.12,2.71\nBR2_BR4,Fig 11(b),7837,478.21,2.71\nBR2_BR4,Fig 11(b),8012,475.90,2.03\nBR2_BR4,Fig 11(b),8188,473.33,3.39\nBR2_BR4,Fig 11(b),8372,470.62,2.03\nBR2_BR4,Fig 11(b),8548,468.83,2.03\nBR2_BR4,Fig 11(b),8723,466.35,2.03\nBR2_BR4,Fig 11(b),8908,464.11,2.71\nBR2_BR4,Fig 11(b),9092,461.40,2.71\nBR2_BR4,Fig 11(b),9268,459.26,2.03\nBR2_BR4,Fig 11(b),9443,457.47,2.03\nBR2_BR4,Fig 11(b),9628,455.30,2.03\nBR2_BR4,Fig 11(b),9812,453.40,2.71\nBR2_BR4,Fig 11(b),9988,451.58,2.03\nBR2_BR4,Fig 11(b),10163,449.81,2.03\nBR2_BR4,Fig 11(b),10348,447.64,2.03\nBR2_BR4,Fig 11(b),10532,446.08,2.03\nBR2_BR4,Fig 11(b),10708,444.13,2.71\n";
```

## Module `kinetics`

Point kinetics with fuel / moderator / reflector temperature feedback,
Xe-135 and decay heat, for the HTR-10 transients (gh:#1151).

Everything numerical is `teh-o-prke`'s: [`PointReactor`] (N-group PRKE by
backward Euler + ANS decay heat, RELAP-7 Eqs 412-429). This module only
supplies the HTR-10 data and the reactivity.

# Kinetics data (input sheet `htr10-th-benchmark.md` Sec 9.1)

| Quantity | Base | Source | Sensitivity |
|---|---|---|---|
| `beta_eff` | 0.00726 | [T1] Sec 2.3.1.1; [J09] Tab 3; [C09] Tab 1 | none (all agree) |
| `Lambda` | 1.68e-3 s | [T1], [J09], [C09] (D4: the as-run sources agree) | `KIN-LAMBDA-H06`: 1.68e-4 s |
| `alpha_f / alpha_m / alpha_r` | -2.13 / -16.2 / +7.71 pcm/K | [T1], [J09] Tab 3 | `KIN-ALPHA-G02-IC`, `-G02-EQ` |
| six delayed groups | **Unknown in every source** | — | see below |

**The six-group constants are a named assumption**: Keepin's U-235
thermal-fission set (G. R. Keepin, T. F. Wimett, R. K. Zeigler, "Delayed
neutrons from fissionable isotopes of uranium, plutonium and thorium",
Phys. Rev. 107 (1957) 1044, Table; reproduced in G. R. Keepin, *Physics
of Nuclear Kinetics*, Addison-Wesley 1965, Table 4-7): half-lives
55.72 / 22.72 / 6.22 / 2.30 / 0.610 / 0.230 s, relative abundances
0.033 / 0.219 / 0.196 / 0.395 / 0.115 / 0.042. Chosen because HTR-10 fuel
is 17 % enriched UO2 in a thermal spectrum, so almost every fission is
U-235 thermal fission; only the relative abundances are used, rescaled to
the benchmark's `beta_eff`. Sensitivities: `teh-o-prke`'s own tabulated
U-235 set (~~rounded half-lives and fractions~~ **since 2026-11-10,
gh:#1319: ENDF/B-VIII.0 U-235 thermal, MAT 9228 MT=455; mean precursor
life 11.06 s against Keepin's 13.02 s**, also rescaled), and the
one-group equivalent (same `sum beta_i / lambda_i`).

# Feedback

`rho = alpha_f (T_f - T_f0) + alpha_m (T_m - T_m0) + alpha_r (T_r - T_r0)
+ (rho_Xe - rho_Xe0)`, the overall coefficients applied to core-average
temperatures; [T1] Sec 2.1.1.6 splits them over zones by power fraction,
which is what the core model's power-weighted averages do.

# Xenon (default ON; `XE-OFF` is the explicit ablation)

I-135 / Xe-135 with burnout, the yields and decay constants of
`teh-o-prke`'s [`Xenon135Poisoning`] (Lamarsh 1975 Tabs 7.5-7.6). The
reactivity of an inventory `X` (atoms in the core) is, one-group,

```text
rho_Xe = -(sigma_Xe / sigma_f) (X / N_235) (Sigma_f / Sigma_a),
Sigma_f / Sigma_a = k_inf / nu,       sigma_Xe phi = (sigma_Xe / sigma_f) F / N_235
```

with `F` the fission rate. Inputs, each cited and each an assumption:
`sigma_Xe = 2.65e6 b`, `sigma_f(U-235) = 585 b` (2200 m/s values;
S. F. Mughabghab, *Atlas of Neutron Resonances*, 2006), so the ratio is
taken at 2200 m/s, not spectrum-averaged; `nu = 2.43` (U-235 thermal,
Lamarsh Tab 6.5); `k_inf = 1.77`, the MCNP5 value for the 57:43
fuel/dummy BCC lattice recorded in `docs/reactor-scoping/htr10-neutronics.md`
(Wang 2014, config b); `N_235` from the as-run 13,651 fuel pebbles
([J09] Tab 2) x 5.0 g U x 17 % ([T1] Tab 2.2). Only the CHANGE from the
equilibrium inventory enters the reactivity.

> Research, education and V&V only. AI draft, unreviewed.

```rust
pub mod kinetics { /* ... */ }
```

### Types

#### Enum `DelayedNeutronSet`

Which delayed-neutron group constants to use (none is published for
HTR-10; see the module docs).

```rust
pub enum DelayedNeutronSet {
    KeepinU235Thermal,
    TehOPrkeU235,
    OneGroupKeepin,
}
```

##### Variants

###### `KeepinU235Thermal`

Keepin (1957) U-235 thermal, six groups. The named base assumption.

###### `TehOPrkeU235`

`teh-o-prke`'s tabulated U-235 set (~~rounded Keepin-like
half-lives and fractions~~ **ENDF/B-VIII.0 U-235 thermal since
2026-11-10, gh:#1319**), group shares rescaled to `beta_eff`.
Sensitivity `KIN-DN-TEHO`.

###### `OneGroupKeepin`

One group with the Keepin set's `sum a_i / lambda_i`. Sensitivity
`KIN-DN-1G`.

##### Implementations

###### Methods

- ```rust
  pub fn groups(self: &Self, beta_eff: Ratio) -> (Vec<Ratio>, Vec<Frequency>) { /* ... */ }
  ```
  `(beta_i, lambda_i)` rescaled so `sum beta_i = beta_eff`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `FeedbackCoefficients`

Fuel, moderator and reflector temperature coefficients.

```rust
pub struct FeedbackCoefficients {
    pub fuel: uom::si::f64::TemperatureCoefficient,
    pub moderator: uom::si::f64::TemperatureCoefficient,
    pub reflector: uom::si::f64::TemperatureCoefficient,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel` | `uom::si::f64::TemperatureCoefficient` | Fuel, 1/K. |
| `moderator` | `uom::si::f64::TemperatureCoefficient` | Moderator (pebble graphite), 1/K. |
| `reflector` | `uom::si::f64::TemperatureCoefficient` | Reflector, 1/K. |

##### Implementations

###### Methods

- ```rust
  pub fn tecdoc1694() -> Self { /* ... */ }
  ```
  [T1] Sec 2.3.1.1 / [J09] Tab 3: -2.13 / -16.2 / +7.71 pcm/K. Base.

- ```rust
  pub fn g02_initial_core() -> Self { /* ... */ }
  ```
  [G02] Sec 4.3.2, initial core: -1.42 / -10.8 / +5.14 pcm/K.

- ```rust
  pub fn g02_equilibrium_core() -> Self { /* ... */ }
  ```
  [G02] Sec 4.3.2, equilibrium core: -2.55 / -5.62 / +4.29 pcm/K.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `XenonModel`

The Xe-135 channel.

```rust
pub enum XenonModel {
    Off,
    OneGroupBurnout {
        xe_to_fission_cross_section_ratio: f64,
        fission_to_absorption_ratio: f64,
        u235_atoms: f64,
    },
}
```

##### Variants

###### `Off`

No xenon reactivity (ablation `XE-OFF`).

###### `OneGroupBurnout`

One-group I/Xe with burnout (see the module docs).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `xe_to_fission_cross_section_ratio` | `f64` | `sigma_Xe / sigma_f(U-235)`. |
| `fission_to_absorption_ratio` | `f64` | `Sigma_f / Sigma_a = k_inf / nu`. |
| `u235_atoms` | `f64` | U-235 atoms in the core. |

##### Implementations

###### Methods

- ```rust
  pub fn htr10() -> Self { /* ... */ }
  ```
  The HTR-10 values of the module docs.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `KineticsParameters`

Kinetics parameters.

```rust
pub struct KineticsParameters {
    pub beta_eff: uom::si::f64::Ratio,
    pub generation_time: uom::si::f64::Time,
    pub delayed_set: DelayedNeutronSet,
    pub coefficients: FeedbackCoefficients,
    pub xenon: XenonModel,
    pub decay_heat_scale: f64,
    pub neutron_source: NeutronSource,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `beta_eff` | `uom::si::f64::Ratio` | `beta_eff`. |
| `generation_time` | `uom::si::f64::Time` | Prompt generation time `Lambda`. |
| `delayed_set` | `DelayedNeutronSet` | Delayed group constants. |
| `coefficients` | `FeedbackCoefficients` | Temperature coefficients. |
| `xenon` | `XenonModel` | Xenon. |
| `decay_heat_scale` | `f64` | Multiplier on the ANS decay power (the standard's `F_gamma`); 1 is<br>best estimate. `DH-HALF` (0.5) mirrors [C14] Tab 4 case 3. |
| `neutron_source` | `NeutronSource` | External neutron source (gh:#1151 part B; see [`NeutronSource`]). |

##### Implementations

###### Methods

- ```rust
  pub fn htr10_base() -> Self { /* ... */ }
  ```
  The base case of the module docs, with the start-up source

- ```rust
  pub fn without_neutron_source(self: Self) -> Self { /* ... */ }
  ```
  The ablation `SRC-OFF`: no external source (part A's kinetics).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `NeutronSource`

An external (non-fission) neutron source in the point kinetics
(gh:#1151 part B).

**Why it matters.** Deep subcritical, the fission power of a
source-free PRKE decays with the longest precursor period for as long
as the reactivity stays negative (part A: hours, to ~1e-20 of the
initial power), and the return to power then has to climb those decades
back; a source holds the fission power at its subcritical
multiplication `S / |rho|` instead.

**Magnitude, cited.** HTR-10 was started with a **20 Ci Am-Be source
placed in the side reflector, emitting 4.4e7 n/s** (IAEA-TECDOC-1382
(2003) Sec 4.2.1.5, "Initial criticality"; the input sheet
`htr10-neutronics.md` records the 20 Ci). **Whether it was still in the
reactor on 15 Oct 2003 is Unknown**: no source in either corpus says it
was removed, and a graphite core at this burnup has no other source
strong enough for a source-range start-up. Its importance relative to a
fission neutron is Unknown; 1 is used, an upper bound for a reflector
source. The intrinsic source of the fuel (U-238 spontaneous fission) is
the floor that is certainly present ([`Self::htr10_intrinsic`]).

**Units.** For a source `S` (n/s) of importance `w`, the PRKE term is
`q = w S / (Lambda nu v Sigma_f V)` in the conversion's units, so that
the subcritical fission-neutron rate is `w S / |rho|` (derivation in
[`Htr10Kinetics::source_rate`]).

```rust
pub enum NeutronSource {
    None,
    Constant {
        neutrons_per_second: f64,
        importance: f64,
    },
}
```

##### Variants

###### `None`

No source (ablation `SRC-OFF`).

###### `Constant`

A constant source.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `neutrons_per_second` | `f64` | Emission rate, n/s. |
| `importance` | `f64` | Importance relative to a fission neutron. |

##### Implementations

###### Methods

- ```rust
  pub fn htr10_am_be_startup() -> Self { /* ... */ }
  ```
  The 20 Ci Am-Be start-up source, 4.4e7 n/s (TECDOC-1382

- ```rust
  pub fn htr10_intrinsic() -> Self { /* ... */ }
  ```
  `SRC-INTRINSIC`: U-238 spontaneous fission in the as-run fuel only.

- ```rust
  pub fn effective_strength(self: &Self) -> f64 { /* ... */ }
  ```
  `w S`, n/s.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `ReactivityBreakdown`

Reactivity split into its parts (dimensionless `dk/k`).

```rust
pub struct ReactivityBreakdown {
    pub fuel: f64,
    pub moderator: f64,
    pub reflector: f64,
    pub xenon: f64,
    pub external: f64,
    pub source_compensation: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel` | `f64` | Fuel temperature term. |
| `moderator` | `f64` | Moderator temperature term. |
| `reflector` | `f64` | Reflector temperature term. |
| `xenon` | `f64` | Xenon change from equilibrium. |
| `external` | `f64` | Externally inserted reactivity (rod withdrawal; zero in the LOFC). |
| `source_compensation` | `f64` | The constant offset that makes the initial state with the source a<br>steady state, `-q Lambda / n0` (the plant was held critical with the<br>source in; about -1.8e-10 for the Am-Be source at 3.3 MW). |

##### Implementations

###### Methods

- ```rust
  pub fn total(self: &Self) -> f64 { /* ... */ }
  ```
  Sum of the parts.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `Htr10Kinetics`

Point kinetics + decay heat + xenon + temperature feedback.

```rust
pub struct Htr10Kinetics {
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
  pub fn new(params: KineticsParameters, total_power: Power, reference: FeedbackTemperatures) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Critical at `total_power` (immediate + saturated decay heat) with the

- ```rust
  pub fn equilibrium_xenon_worth(self: &Self) -> f64 { /* ... */ }
  ```
  Equilibrium xenon worth at the initial state, `dk/k` (negative).

- ```rust
  pub fn reactivity(self: &Self, temps: &FeedbackTemperatures) -> ReactivityBreakdown { /* ... */ }
  ```
  The reactivity at `temps` and the current xenon.

- ```rust
  pub fn source_rate(self: &Self) -> VolumetricNumberRate { /* ... */ }
  ```
  The PRKE source term of [`KineticsParameters::neutron_source`] in the

- ```rust
  pub fn step(self: &mut Self, dt: Time, temps: &FeedbackTemperatures) -> Result<ReactivityBreakdown, TampinesError> { /* ... */ }
  ```
  Advance by `dt` with the feedback evaluated at `temps` (the core's

- ```rust
  pub fn step_with_external(self: &mut Self, dt: Time, temps: &FeedbackTemperatures, external: f64) -> Result<ReactivityBreakdown, TampinesError> { /* ... */ }
  ```
  As [`Self::step`], with an externally inserted reactivity `external`

- ```rust
  pub fn fission_power(self: &Self) -> Power { /* ... */ }
  ```
  Immediate fission power.

- ```rust
  pub fn fission_rate(self: &Self) -> Frequency { /* ... */ }
  ```
  Fission rate, 1/s (the conversion's `F = n v Sigma_f V`).

- ```rust
  pub fn decay_power(self: &Self) -> Power { /* ... */ }
  ```
  Fission-product decay power.

- ```rust
  pub fn total_power(self: &Self) -> Power { /* ... */ }
  ```
  Total thermal power deposited in the core.

- ```rust
  pub fn parameters(self: &Self) -> &KineticsParameters { /* ... */ }
  ```
  The parameters.

- ```rust
  pub fn relative_xenon(self: &Self) -> f64 { /* ... */ }
  ```
  Xenon inventory relative to its initial equilibrium.

- ```rust
  pub fn delayed_mean_life_weight(self: &Self) -> f64 { /* ... */ }
  ```
  `sum beta_i / lambda_i` of the group set in use, s (the delayed

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Re-export `FeedbackTemperatures`

```rust
pub use tampines::htr10_primary_loop::FeedbackTemperatures;
```

## Module `transient`

The coupled transient: loop + core + kinetics, the LOFC ATWS and CRW
ATWS cases and their named sensitivity cases (gh:#1151).

# Step order (dt)

1. kinetics: backward-Euler PRKE with the reactivity of the core's
   start-of-step temperatures (explicit feedback) plus any external
   reactivity at the end of the step (the CRW rod withdrawal), and the
   external neutron source; then decay heat and xenon at the end-of-step
   fission rate;
2. loop: speed, baffle, isolation, momentum against the core's
   start-of-step resistance and gravity head; SG; cold-nozzle temperature;
3. core: the lumped stand-in steps every `dt`; the R-Z core
   (`tampines::htr10_primary_loop::RzPorousCore`) sub-cycles at its own step with the time-mean
   power.

# Initial state (D4: as-run)

3315 kW ([J09] Tab 2), RPV inlet 212 degC ([J09] Tab 2, [C09] Tab 1),
2476 kPa ([J09] Tab 2), 1.457 kg/s (Derived, sheet Sec 6). The core is
solved to steady state at that point, the kinetics is made critical at
the resulting temperatures, and the circulator head is set to hold the
flow.

# CRW ATWS (input sheet Sec 8)

The external reactivity is [T1] Tab 2.60 (0-128 s, held at 4.9762 mk
after). As run (D4 base, `CRW-BASE`): the protection system trips the
circulator, closes the baffle and isolates the secondary at **12 s**
([H06] Tab 2, Sec 4.2); the baffle closure stops the loop flow at once.
The specification (`CRW-TRIP9`, [T1] Sec 2.4.1) stops the flow
instantly at 9 s.

> Research, education and V&V only. AI draft, unreviewed.

```rust
pub mod transient { /* ... */ }
```

### Types

#### Struct `InitialState`

The initial operating point.

```rust
pub struct InitialState {
    pub power: uom::si::f64::Power,
    pub inlet_temperature: uom::si::f64::ThermodynamicTemperature,
    pub pressure: uom::si::f64::Pressure,
    pub mass_flow: uom::si::f64::MassRate,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `power` | `uom::si::f64::Power` | Total thermal power. |
| `inlet_temperature` | `uom::si::f64::ThermodynamicTemperature` | RPV inlet (cold nozzle) helium temperature. |
| `pressure` | `uom::si::f64::Pressure` | Primary pressure. |
| `mass_flow` | `uom::si::f64::MassRate` | Loop mass flow. |

##### Implementations

###### Methods

- ```rust
  pub fn as_run() -> Self { /* ... */ }
  ```
  As-run (D4): 3315 kW, 212 degC, 2476 kPa, 1.457 kg/s.

- ```rust
  pub fn hu2006() -> Self { /* ... */ }
  ```
  `TR-HU`: [H06] 3000 kW, 215 degC, 2500 kPa; flow by energy balance

- ```rust
  pub fn spec() -> Self { /* ... */ }
  ```
  `TR-SPEC`: [T1] 3 MW, 250 degC, 2.5 MPa, flow 1.444 kg/s (sheet Sec 6).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `CoreChoice`

Which core model a case runs.

```rust
pub enum CoreChoice {
    LumpedStandIn,
    RzPorous {
        options: outram_foam_appbuilder_lib::htr10_rz::TransientCoreOptions,
        time_step: tampines::htr10_primary_loop::RzCoreTimeStep,
    },
}
```

##### Variants

###### `LumpedStandIn`

The lumped stand-in of part A.

###### `RzPorous`

The R-Z porous core of part B. Its bypass split is replaced by the
case's [`LofcCase::bypass`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `options` | `outram_foam_appbuilder_lib::htr10_rz::TransientCoreOptions` | The R-Z model options. |
| `time_step` | `tampines::htr10_primary_loop::RzCoreTimeStep` | Its own time step. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `LofcCase`

One transient case (LOFC or CRW): everything that defines a run.

```rust
pub struct LofcCase {
    pub name: String,
    pub initial: InitialState,
    pub kinetics: super::kinetics::KineticsParameters,
    pub bypass: tampines::htr10_primary_loop::BypassSplit,
    pub loop_config: tampines::htr10_primary_loop::PrimaryLoopConfig,
    pub duration: uom::si::f64::Time,
    pub core: CoreChoice,
    pub external_reactivity: Option<tampines::htr10_primary_loop::data::TabulatedSeries>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The case name (input sheet Sec 9.2). |
| `initial` | `InitialState` | Initial point. |
| `kinetics` | `super::kinetics::KineticsParameters` | Kinetics. |
| `bypass` | `tampines::htr10_primary_loop::BypassSplit` | Bypass split. |
| `loop_config` | `tampines::htr10_primary_loop::PrimaryLoopConfig` | Loop (coastdown, baffle, ...). |
| `duration` | `uom::si::f64::Time` | Simulated duration after the start. |
| `core` | `CoreChoice` | The core model. |
| `external_reactivity` | `Option<tampines::htr10_primary_loop::data::TabulatedSeries>` | Externally inserted reactivity, `dk/k` against time since the start<br>(`None` in the LOFC). |

##### Implementations

###### Methods

- ```rust
  pub fn base() -> Result<Self, TampinesError> { /* ... */ }
  ```
  The base case (as-run, D4) with the lumped stand-in (part A).

- ```rust
  pub fn rz_base() -> Result<Self, TampinesError> { /* ... */ }
  ```
  The LOFC base case with the R-Z core (part B): `RZ-BASE`.

- ```rust
  pub fn crw_base() -> Result<Self, TampinesError> { /* ... */ }
  ```
  The CRW ATWS base (as-run, D4; module docs): `CRW-BASE`, R-Z core,

- ```rust
  pub fn crw_trip9() -> Result<Self, TampinesError> { /* ... */ }
  ```
  `CRW-TRIP9`: the specification's instantaneous flow stop and

- ```rust
  pub fn named(self: Self, name: &str) -> Self { /* ... */ }
  ```
  Rename (builder).

- ```rust
  pub fn with_coastdown(self: Self, c: CoastdownModel) -> Self { /* ... */ }
  ```
  Replace the coastdown (builder).

- ```rust
  pub fn with_initial(self: Self, s: InitialState) -> Self { /* ... */ }
  ```
  Replace the initial state, keeping the loop pressure consistent.

- ```rust
  pub fn with_rz_options</* synthetic */ impl Fn(TransientCoreOptions) -> TransientCoreOptions: Fn(TransientCoreOptions) -> TransientCoreOptions>(self: Self, f: impl Fn(TransientCoreOptions) -> TransientCoreOptions) -> Self { /* ... */ }
  ```
  Change the R-Z core options (no-op for the lumped stand-in).

- ```rust
  pub fn with_rz_time_step(self: Self, step: RzCoreTimeStep) -> Self { /* ... */ }
  ```
  Change the R-Z core time step (no-op for the lumped stand-in).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `TransientSample`

One sample of a run.

```rust
pub struct TransientSample {
    pub time_s: f64,
    pub total_power_w: f64,
    pub fission_power_w: f64,
    pub decay_power_w: f64,
    pub reactivity: super::kinetics::ReactivityBreakdown,
    pub mass_flow_kg_s: f64,
    pub speed: f64,
    pub fuel_k: f64,
    pub moderator_k: f64,
    pub reflector_k: f64,
    pub core_outlet_k: f64,
    pub heat_to_rccs_w: f64,
    pub heat_to_coolant_w: f64,
    pub bed_upflow_kg_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time_s` | `f64` | Time after the start, s. |
| `total_power_w` | `f64` | Total power, W. |
| `fission_power_w` | `f64` | Immediate fission power, W. |
| `decay_power_w` | `f64` | Decay power, W. |
| `reactivity` | `super::kinetics::ReactivityBreakdown` | Reactivity parts. |
| `mass_flow_kg_s` | `f64` | Loop flow, kg/s. |
| `speed` | `f64` | Relative circulator speed. |
| `fuel_k` | `f64` | Power-weighted fuel, moderator; reflector, K. |
| `moderator_k` | `f64` | See `fuel_k`. |
| `reflector_k` | `f64` | See `fuel_k`. |
| `core_outlet_k` | `f64` | Hot-nozzle helium, K. |
| `heat_to_rccs_w` | `f64` | Heat to the RCCS, W. |
| `heat_to_coolant_w` | `f64` | Heat to the coolant, W. |
| `bed_upflow_kg_s` | `f64` | In-vessel natural circulation: upward mass flow across the bed<br>mid-height, kg/s (R-Z core only; 0 for the stand-in). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `LofcMetrics`

Summary numbers of a LOFC run.

```rust
pub struct LofcMetrics {
    pub time_to_one_percent_s: Option<f64>,
    pub min_reactivity: (f64, f64),
    pub reactivity_zero_crossing_s: Option<f64>,
    pub power_return_s: Option<f64>,
    pub first_peak: Option<(f64, f64)>,
    pub late_total_power_w: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `time_to_one_percent_s` | `Option<f64>` | First time the fission power is at or below 1 % of its initial value. |
| `min_reactivity` | `(f64, f64)` | Most negative total reactivity, dk/k, and its time. |
| `reactivity_zero_crossing_s` | `Option<f64>` | First upward zero crossing of the total reactivity after its minimum. |
| `power_return_s` | `Option<f64>` | First time after the 1 % point that the fission power climbs back<br>above 1 % of its initial value (the measured-trace definition). |
| `first_peak` | `Option<(f64, f64)>` | Largest relative fission power after the return, and its time. |
| `late_total_power_w` | `f64` | Mean total power over the last 1800 s, W. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `CrwMetrics`

Summary numbers of a CRW run (input sheet Sec 8.3).

```rust
pub struct CrwMetrics {
    pub first_peak: (f64, f64),
    pub first_peak_total_w: f64,
    pub decay_level_s: Option<f64>,
    pub min_reactivity: (f64, f64),
    pub recriticality_s: Option<f64>,
    pub recriticality_peak: Option<(f64, f64)>,
    pub power_at_7200_w: f64,
    pub late_total_power_w: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `first_peak` | `(f64, f64)` | Largest relative fission power in the first 300 s, and its time. |
| `first_peak_total_w` | `f64` | That peak's total power, W. |
| `decay_level_s` | `Option<f64>` | First time after the peak that the fission power is below the decay<br>power ("power to decay level"). |
| `min_reactivity` | `(f64, f64)` | Most negative total reactivity after the peak, dk/k, and its time. |
| `recriticality_s` | `Option<f64>` | First upward zero crossing of the total reactivity after its<br>minimum (re-criticality). |
| `recriticality_peak` | `Option<(f64, f64)>` | Largest relative fission power after the re-criticality, and its<br>time. |
| `power_at_7200_w` | `f64` | Total power at 7200 s, W (or the last sample). |
| `late_total_power_w` | `f64` | Mean total power over the last 1800 s, W. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `CoupledTransient`

Loop + core + kinetics.

```rust
pub struct CoupledTransient {
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
  pub fn new(case: &LofcCase) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Build at the case's initial state with the case's core and the

- ```rust
  pub fn with_parts(case: &LofcCase, core: CoreModel, sg: SteamGeneratorModel) -> Result<Self, TampinesError> { /* ... */ }
  ```
  Build with an explicit core model and steam generator.

- ```rust
  pub fn initial_response(self: &Self) -> CoreResponse { /* ... */ }
  ```
  The core's steady-state response at the initial point.

- ```rust
  pub fn primary_loop(self: &Self) -> &PrimaryLoop { /* ... */ }
  ```
  The loop.

- ```rust
  pub fn core(self: &Self) -> &CoreModel { /* ... */ }
  ```
  The core.

- ```rust
  pub fn kinetics(self: &Self) -> &Htr10Kinetics { /* ... */ }
  ```
  The kinetics.

- ```rust
  pub fn time_s(self: &Self) -> f64 { /* ... */ }
  ```
  Time since the start, s.

- ```rust
  pub fn step(self: &mut Self, dt: Time) -> Result<TransientSample, TampinesError> { /* ... */ }
  ```
  Advance by `dt`.

- ```rust
  pub fn settle_steam_generator(self: &mut Self, hot: ThermodynamicTemperature, duration: Time) -> Result<ThermodynamicTemperature, TampinesError> { /* ... */ }
  ```
  Settle a helical-coil steam generator at the hot-nozzle temperature

- ```rust
  pub fn initial_fission_power_w(self: &Self) -> f64 { /* ... */ }
  ```
  Initial immediate fission power, W.

- ```rust
  pub fn thermocouples_c(self: &Self, tcs: &[Thermocouple]) -> Option<Vec<f64>> { /* ... */ }
  ```
  The R-Z core's model value at each thermocouple, deg C (`None` for

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Type Alias `ProbeRow`

Thermocouple readings at one time: `(t_s, deg C per thermocouple)`.

```rust
pub type ProbeRow = (f64, Vec<f64>);
```

### Functions

#### Function `crw_reactivity`

[T1] Tab 2.60 as `dk/k` against s (mk / 1000), held after 128 s.

# Errors
Embedded-table parse errors.

```rust
pub fn crw_reactivity() -> Result<tampines::htr10_primary_loop::data::TabulatedSeries, tampines::TampinesError> { /* ... */ }
```

#### Function `run_lofc`

Run a case: 0.1 s steps for the first 1000 s, 0.5 s after; one sample
per second (plus `t = 0`).

# Errors
As [`CoupledTransient::step`].

```rust
pub fn run_lofc(case: &LofcCase) -> Result<(CoupledTransient, Vec<TransientSample>), tampines::TampinesError> { /* ... */ }
```

#### Function `run_case`

As [`run_lofc`], also reading the R-Z core at `probes` every
`probe_every_s` seconds (and at `t = 0`; nothing for the stand-in or
for `probe_every_s <= 0`).

# Errors
As [`CoupledTransient::step`].

```rust
pub fn run_case(case: &LofcCase, probes: &[outram_foam_appbuilder_lib::htr10_rz::Thermocouple], probe_every_s: f64) -> Result<(CoupledTransient, Vec<TransientSample>, Vec<ProbeRow>), tampines::TampinesError> { /* ... */ }
```

#### Function `lofc_metrics`

The metrics of [`LofcMetrics`] from a trace.

```rust
pub fn lofc_metrics(trace: &[TransientSample]) -> LofcMetrics { /* ... */ }
```

#### Function `crw_metrics`

The metrics of [`CrwMetrics`] from a trace.

```rust
pub fn crw_metrics(trace: &[TransientSample]) -> CrwMetrics { /* ... */ }
```

#### Function `c09_thermocouple_positions`

Model positions of the 17 [C09] LOFC thermocouple traces (labels as in
`lofc-thermocouples-c09-figs8-11.csv`), cm, TECDOC `z` downward.

**[C09] prints no coordinates** (its Fig 3 is a schematic), so every
position is **Uncertain**, and was fixed before any model result:

- `SR1`-`SR12`: [T1] Tab 2.7 thermocouples 1-12 (`r` 193 / 189 / 167 /
  133 / 117 / 93 cm at `z` 80, then the same at `z` 170). Basis: [C09]
  Sec 4 has SR1-6 and SR7-12 as two rows "at two different heights"
  from the carbon brick inward, Fig 3 draws SR1-6 as the upper row, and
  in both rows the measured initial temperatures rise monotonically from
  SR1 to SR6 and SR7 to SR12 as Tab 2.7's radii fall.
- `TR1_TR4`: [T1] Tab 2.7 thermocouple 13 (`r 60, z -40`), the top
  reflector just above the core cavity ([C09] Sec 5: "located above the
  top cavity"). `TR2_TR5` and `TR3_TR6`: read from Fig 3 as the same
  column (`r ~ 55 cm`) about 60 cm and 95 cm higher (`z -100`, `-135`:
  upper top reflector and the top carbon brick, which [C09] Sec 5 names
  for TR3&TR6), +-15 cm.
- `BR1_BR3`, `BR2_BR4`: read from Fig 3 just under the cone at `r ~ 78`
  and `~ 45 cm`, placed at Tab 2.7's under-core height `z 234`. **Which
  pair is inner is not readable**: BR2&BR4 is put inner because it reads
  the hotter at `t = 0` (748 against 643 deg C), as the inner of Tab
  2.7's two `z 234` points (TC 14, `r 40`) is at full power.

```rust
pub fn c09_thermocouple_positions() -> Vec<outram_foam_appbuilder_lib::htr10_rz::Thermocouple> { /* ... */ }
```

### Re-exports

#### Re-export `DelayedNeutronSet`

```rust
pub use kinetics::DelayedNeutronSet;
```

#### Re-export `FeedbackCoefficients`

```rust
pub use kinetics::FeedbackCoefficients;
```

#### Re-export `FeedbackTemperatures`

```rust
pub use kinetics::FeedbackTemperatures;
```

#### Re-export `Htr10Kinetics`

```rust
pub use kinetics::Htr10Kinetics;
```

#### Re-export `KineticsParameters`

```rust
pub use kinetics::KineticsParameters;
```

#### Re-export `NeutronSource`

```rust
pub use kinetics::NeutronSource;
```

#### Re-export `ReactivityBreakdown`

```rust
pub use kinetics::ReactivityBreakdown;
```

#### Re-export `XenonModel`

```rust
pub use kinetics::XenonModel;
```

#### Re-export `c09_thermocouple_positions`

```rust
pub use transient::c09_thermocouple_positions;
```

#### Re-export `crw_metrics`

```rust
pub use transient::crw_metrics;
```

#### Re-export `crw_reactivity`

```rust
pub use transient::crw_reactivity;
```

#### Re-export `lofc_metrics`

```rust
pub use transient::lofc_metrics;
```

#### Re-export `run_case`

```rust
pub use transient::run_case;
```

#### Re-export `run_lofc`

```rust
pub use transient::run_lofc;
```

#### Re-export `CoreChoice`

```rust
pub use transient::CoreChoice;
```

#### Re-export `CoupledTransient`

```rust
pub use transient::CoupledTransient;
```

#### Re-export `CrwMetrics`

```rust
pub use transient::CrwMetrics;
```

#### Re-export `InitialState`

```rust
pub use transient::InitialState;
```

#### Re-export `LofcCase`

```rust
pub use transient::LofcCase;
```

#### Re-export `LofcMetrics`

```rust
pub use transient::LofcMetrics;
```

#### Re-export `ProbeRow`

```rust
pub use transient::ProbeRow;
```

#### Re-export `TransientSample`

```rust
pub use transient::TransientSample;
```

## Module `msre_frequency_response`

MSRE power-to-reactivity frequency response (MSRE T-1, gh:#1132).
**MSRE power-to-reactivity frequency response (MSRE T-1, gh:#1132).**

> Research, education and V&V only. AI draft (Claude Opus 5.5 in Claude
> Code), unreviewed. Not for facility operation.

A linear model of the whole MSRE heat-transport chain, solved at each
frequency as one complex linear system (`petir::depletion::solve_complex`).
The pieces, with page numbers from the ORNL-TM-1070 scan (Ball and
Kerlin, 1965):

- **Kinetics:** circulating-fuel point kinetics, six groups. The
  precursors leave the core in a well-stirred core of transit time τ_c
  and return after the loop transit τ_L, decayed. This is the equation set
  of `teh_o_prke::zero_power_prke::circulating` (T-C1, gh:#1131),
  linearised; it is TM-1070 eq. (A.29)/(A.30) in reactivity form.
  - λ_i and β'_i from App. B p. 69; Λ, τ_c and τ_L from Table 1 "present
    study".
  - The steady-state reactivity ρ₀ is the circulation loss (A.31).
- **Core:** single region, two fuel lumps and a graphite lump, eqs.
  (A.1)-(A.3) p. 49, with App. B's "1965 extrema determination"
  coefficients (pp. 67-68). Reactivity follows the first fuel lump and the
  graphite (importances 1, 0, 1).
  - The delayed-gamma share of the power, 0.0564, arrives through a
    188 s first-order lag (Table 1).
- **Heat exchanger and radiator:** two lumps per fluid plus the tube
  (and, in the heat exchanger, the shell), eqs. (A.32)-(A.37) pp. 58-60,
  with App. B p. 69's 1965 coefficients at 10 MW.
  - The radiator air side scales with power as ORNL scaled it (p. 60):
    W_air ∝ P, h_air ∝ W_air^0.6, so n_air ∝ P^-0.4, τ_tube→air ∝ P^-0.6
    and t*_air ∝ P^-1. The air inlet temperature does not change.
- **Piping:** pure transport delays (plug flow), App. B p. 70's 1965 lags:
  core → HX 5.77 s, HX → core 8.67 s, HX → radiator 4.71 s,
  radiator → HX 8.24 s.
  - TM-1070 p. 16 calls plug flow "probably superior" (Re ≈ 240 000 and
    120 000) to the reduced model's first-order lags. Its complete model
    used pure delays.

**The fuel and graphite temperature coefficients are inputs.** For T-1
they come from this workspace's own Monte Carlo model (N-3, gh:#1129),
not from TM-1070's −4.84 and −3.7 × 10⁻⁵ /°F. Those are kept as
[`ORNL_1965_FUEL_PER_F`] and [`ORNL_1965_GRAPHITE_PER_F`] for the
code-to-code check against ORNL's own predicted curve.

Everything inside is in ORNL's units (MW, °F, s); the public API is `uom`.

```rust
pub mod msre_frequency_response { /* ... */ }
```

### Types

#### Struct `HxSection`

One counterflow heat-exchange section: two lumps per fluid, a tube and
an optional shell (TM-1070 eqs. A.32-A.37). Fluid 1 is inside the tube.

```rust
pub struct HxSection {
    pub n1: f64,
    pub n2: f64,
    pub ns: f64,
    pub t1: f64,
    pub t2: f64,
    pub tau_t1: f64,
    pub tau_t2: f64,
    pub tau_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n1` | `f64` | n₁ = hA/(W c_p) on fluid 1. |
| `n2` | `f64` | n₂ on fluid 2. |
| `ns` | `f64` | n_s, fluid 2 to the shell (0: no shell). |
| `t1` | `f64` | Fluid 1 transit time \[s\]. |
| `t2` | `f64` | Fluid 2 transit time \[s\]. |
| `tau_t1` | `f64` | Tube time constant to fluid 1 \[s\]. |
| `tau_t2` | `f64` | Tube time constant to fluid 2 \[s\]. |
| `tau_s` | `f64` | Shell time constant \[s\] (unused when `ns == 0`). |

##### Implementations

###### Methods

- ```rust
  pub fn radiator_at(p_mw: f64) -> Self { /* ... */ }
  ```
  The radiator at `p_mw`, its air side scaled from 10 MW as TM-1070

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `MsreLoopModel`

The MSRE loop model at one power level.

```rust
pub struct MsreLoopModel {
    pub alpha_fuel: f64,
    pub alpha_graphite: f64,
    pub power_mw: f64,
    pub generation_time: f64,
    pub core_transit: f64,
    pub loop_transit: f64,
    pub hx: HxSection,
    pub radiator: HxSection,
    pub delays: [f64; 4],
    pub loops: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `alpha_fuel` | `f64` | Fuel temperature coefficient \[1/°F\]. |
| `alpha_graphite` | `f64` | Graphite temperature coefficient \[1/°F\]. |
| `power_mw` | `f64` | Steady power \[MW\]. |
| `generation_time` | `f64` | Prompt generation time Λ \[s\] (Table 1: 0.00024). |
| `core_transit` | `f64` | Core transit τ_c \[s\] (Table 1: 8.46). |
| `loop_transit` | `f64` | External loop transit τ_L \[s\] (Table 1: 16.73). |
| `hx` | `HxSection` | Heat exchanger. |
| `radiator` | `HxSection` | Radiator at this power. |
| `delays` | `[f64; 4]` | Piping delays \[s\]: core→HX, HX→core, HX→radiator, radiator→HX. |
| `loops` | `bool` | Whether the external loops respond (false: core alone, inlet held). |

##### Implementations

###### Methods

- ```rust
  pub fn tm1070(power: Power, fuel: TemperatureCoefficient, graphite: TemperatureCoefficient) -> Self { /* ... */ }
  ```
  TM-1070's 1965 parameters at `power`, with the given coefficients.

- ```rust
  pub fn circulation_loss(self: &Self) -> f64 { /* ... */ }
  ```
  The steady reactivity of the circulating core, eq. (A.31) in

- ```rust
  pub fn response(self: &Self, omega: f64) -> Complex64 { /* ... */ }
  ```
  δP/P₀ per unit δρ (δk/k) of external reactivity at `omega` \[rad/s\].

- ```rust
  pub fn natural_period(self: &Self, points: usize) -> Option<Time> { /* ... */ }
  ```
  The natural period of oscillation: 2π/ω at the peak of |δP/δρ| on a

- ```rust
  pub fn amplitude_phase(self: &Self, omega: AngularVelocity) -> (f64, f64) { /* ... */ }
  ```
  `|δP/P₀ / δρ|` and phase \[deg\] at `omega`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `per_f`

A coefficient from ORNL's per-°F to `uom`.

```rust
pub fn per_f(alpha_per_f: f64) -> uom::si::f64::TemperatureCoefficient { /* ... */ }
```

### Constants and Statics

#### Constant `ORNL_1965_FUEL_PER_F`

TM-1070 Table 1 "present study" fuel coefficient, −4.84 × 10⁻⁵ /°F.

```rust
pub const ORNL_1965_FUEL_PER_F: f64 = -4.84e-5;
```

#### Constant `ORNL_1965_GRAPHITE_PER_F`

TM-1070 Table 1 "present study" graphite coefficient, −3.7 × 10⁻⁵ /°F.

```rust
pub const ORNL_1965_GRAPHITE_PER_F: f64 = -3.7e-5;
```

#### Constant `MEASURED_PERIODS_TM1647`

TM-1647 (Kerlin, Ball and Steffy, 1966) Fig. 29: the **measured**
natural periods of oscillation, `(power MW, period min)`.

**Digitised 2026-11-09, a processing step.** The scan's PDF page 46 was
rendered at 300 dpi in grey. Both log axes were calibrated on the
gridlines:
- x: 0.02 MW at px 428 and 10 MW at px 2086; the 0.1 and 1 MW lines fall
  within 2 px of where the calibration puts them;
- y: 1 min at px 2085 and 50 min at px 1020; the 2, 5, 10 and 20 min
  lines fall within 5 px.

Each value is the centroid of the dot's dense core (pixels whose 13 × 13
neighbourhood is ≥ 95 % dark). The dot radius, ~13 px, makes each reading
good to about ±2 % in both power and period. The two highest-power dots
touch, and were separated with a 10 px search radius.

```rust
pub const MEASURED_PERIODS_TM1647: [(f64, f64); 7] = _;
```

#### Constant `LAMBDA`

Decay constants λ_i \[1/s\], TM-1070 App. B p. 69.

```rust
pub const LAMBDA: [f64; 6] = _;
```

#### Constant `BETA`

Delayed-neutron fractions β'_i (total, "1965 frequency-response"), App. B
p. 69. They sum to 0.006661, Table 1's 0.00666.

```rust
pub const BETA: [f64; 6] = _;
```

#### Constant `HX_1965`

The 1965 heat exchanger (fluid 1 coolant, fluid 2 fuel), App. B p. 69.

```rust
pub const HX_1965: HxSection = _;
```

#### Constant `RADIATOR_1965_10MW`

The 1965 radiator at 10 MW (fluid 1 coolant, fluid 2 air), App. B p. 69.

```rust
pub const RADIATOR_1965_10MW: HxSection = _;
```

#### Constant `TAU_F`

Fuel-lump residence time τ_F \[s\] (each of the two lumps), App. B pp. 67-68.

```rust
pub const TAU_F: f64 = 4.23;
```

#### Constant `K_F`

Fraction of the fission power deposited in each fuel lump, K_F.

```rust
pub const K_F: f64 = 0.47;
```

#### Constant `K_G`

Fraction of the fission power deposited in each graphite half, K_G
(the graphite lump takes 2 K_G).

```rust
pub const K_G: f64 = 0.03;
```

#### Constant `MC_F`

Heat capacity of each fuel lump, MC_F \[MW·s/°F\].

```rust
pub const MC_F: f64 = 0.750;
```

#### Constant `MC_G`

Heat capacity of the graphite lump, MC_G \[MW·s/°F\].

```rust
pub const MC_G: f64 = 3.58;
```

#### Constant `HA`

Graphite-to-fuel conductance, hA \[MW/°F\].

```rust
pub const HA: f64 = 0.020;
```

#### Constant `GAMMA_SHARE`

Delayed-gamma share of the power (Table 1).

```rust
pub const GAMMA_SHARE: f64 = 0.0564;
```

#### Constant `GAMMA_LAG`

Delayed-gamma first-order lag \[s\] (Table 1).

```rust
pub const GAMMA_LAG: f64 = 188.0;
```

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
- ~~gh:#313 — `Cell::temperature` is never read by transport and every cell
  hardcodes 293.6 K; the material temperature (300.15 K) is what is used.~~
  **CORRECTED 2026-11-14:** cell temperature is now the transport default
  and the HTR-10 cells are `None` (the material's temperature), so the
  hardcoded 293.6 K is gone and cannot override a 300.15 K material.
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
    fn clone(self: &Self) -> Self { /* ... */ }
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

- **Forget**
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
- **Move**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
    fn clone(self: &Self) -> Self { /* ... */ }
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

- **Forget**
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
- **Move**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
    pub fuel_fraction: f64,
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
| `fuel_fraction` | `f64` | Fraction of the eligible balls that are fuel under<br>[`FuelAssignment::Paper`] / [`FuelAssignment::FuelledConus`]: the<br>paper's 0.57 unless [`Self::with_fuel_fraction`] changed it (gh:#566). |
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
  pub fn with_fuel_fraction(self: Self, f: f64) -> Self { /* ... */ }
  ```
  The same bed with fraction `f` (clamped to `[0, 1]`) of its eligible

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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
    Explicit(super::explicit_bed::ExplicitBed),
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

###### `Explicit`

An explicit ball list, normally a DEM fill (added 2026-10-05): see
[`super::explicit_bed`]. Built only by
[`super::explicit_bed::assemble_explicit_triso_from_centres`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::explicit_bed::ExplicitBed` |  |

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
  Balls removed by a rejection rule. For an explicit bed: DEM balls

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Enum `BedBall`

A ball of a [`PebbleBed`].

```rust
pub enum BedBall {
    Seker(SekerBallId),
    TwoBall(BallId),
    Explicit(usize),
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

###### `Explicit`

Ball `i` of an [`super::explicit_bed::ExplicitBed`] (its index in
`centres`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
iron joints as explicit geometry. **Since 2026-10-05 (gh:#580)** the same
explicit rods can be built at any position of their published travel,
~~all ten together (`core_design::Htr10CoreDesign::rod_insertion`;~~
**each rod at its own position since 2026-10-23 (gh:#1025,
`core_design::Htr10CoreDesign::rod_insertions`, one rod universe per
distinct position; `with_bank` is the old all-ten case, cell for cell;**
lower end
from `z_T` 119.2 cm withdrawn to 394.2 cm fully inserted); withdrawn stays
the default and the benchmark's state. The absorber-ball system is a reserve
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

Control-rod channel: holds a rod, withdrawn unless the design inserts
it (gh:#580).

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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `control_rod_azimuths_deg`

**Attributes:**

- `MustUse { reason: None }`

Azimuth \[deg\] of each control rod's channel, in the order the rods are
numbered everywhere in this model (`Htr10CoreDesign::rod_insertions`,
`AssembledCore::rod_lower_ends_zt_cm`, the web demo's "rod 1" to "rod 10"):
the order of [`reflector_channels`], i.e. increasing azimuth from 9°.

**Not TECDOC's names.** IAEA-TECDOC-1382 Fig. 4.7 (printed p. 234) labels
one rod, "Control Rod S3", by a leader line in a 416 x 233 px raster; no
source in reach names the other nine or fixes which of these channels S3
is (checked 2026-10-23, gh:#1025). Source pending.

```rust
pub fn control_rod_azimuths_deg() -> [f64; 10] { /* ... */ }
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

#### Function `regional_slot`

**Attributes:**

- `MustUse { reason: None }`

The copy of composition slot `base` for reflector region `region`
(0 top, 1 side, 2 bottom), or `None` if `base` has no regional copies.

```rust
pub fn regional_slot(base: usize, region: usize) -> Option<usize> { /* ... */ }
```

#### Function `composition_of`

**Attributes:**

- `MustUse { reason: None }`

The composition slot a zoned slot copies: [`GRAPHITE`] for
[`MODERATOR_BALL`], the base for a regional copy, and the slot itself
otherwise.

```rust
pub fn composition_of(slot: usize) -> usize { /* ... */ }
```

#### Function `region_of_regional`

**Attributes:**

- `MustUse { reason: None }`

Region (0 top, 1 side, 2 bottom) of a regional copy, if `slot` is one.

```rust
pub fn region_of_regional(slot: usize) -> Option<usize> { /* ... */ }
```

#### Function `for_zone_zoned`

**Attributes:**

- `MustUse { reason: None }`

**Material slot of a Fig. 4.10 zone in a zoned design**: the slot
[`for_zone_mc`] (or, under the `zone_map` ablation, [`for_zone_uniform`])
gives, moved to its copy for `region` (0 top, 1 side, 2 bottom);
[`BORONATED`] stays itself.

# Panics
As [`for_zone_mc`], and for a region above 2.

```rust
pub fn for_zone_zoned(zone: usize, region: usize, zone_map: bool) -> usize { /* ... */ }
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

#### Constant `MODERATOR_BALL`

**The graphite (dummy) balls**: [`GRAPHITE`]'s composition in a slot
of its own, so the moderator pebbles can sit at another temperature
than the fuel pebbles' matrix and shell.

```rust
pub const MODERATOR_BALL: usize = COUNT;
```

#### Constant `REGIONAL_BASES`

The reflector composition slots that get one copy per reflector
region (top, side, bottom): [`REFLECTOR`] and every Table 4-3 zone
slot. [`BORONATED`] is not among them: the boronated bricks are a
temperature zone of their own wherever they sit.

```rust
pub const REGIONAL_BASES: [usize; 25] = _;
```

#### Constant `REGIONAL_FIRST`

First regional copy: [`REGIONAL_BASES`] for the top reflector, then
the side, then the bottom.

```rust
pub const REGIONAL_FIRST: usize = _;
```

#### Constant `ZONED_COUNT`

Number of material slots of a zoned design.

```rust
pub const ZONED_COUNT: usize = _;
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
    pub triso_particles: Option<usize>,
    pub rod_lower_end_zt_cm: Option<f64>,
    pub rod_lower_ends_zt_cm: Option<[f64; 10]>,
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
| `bed` | `Option<super::bed::PebbleBed>` | The ball-level description of the bed (every ball's centre, identity and<br>presence) that [`assemble_explicit_triso`] built its lattice from;<br>`None` for the one-ball [`assemble`]. Added 2026-09-26 so plots can cut<br>through ball centres chosen from the built bed rather than from constants.<br>~~`Option<TwoBallBed>`~~ **CHANGED 2026-10-01 (gh:#472):** a<br>[`PebbleBed`], Şeker's cell by default; `PebbleBed::Explicit` when<br>built by [`super::explicit_bed::assemble_explicit_triso_from_centres`]<br>(2026-10-05), whose lattice uses Şeker's tile shape purely as a<br>spatial index. |
| `triso_particles` | `Option<usize>` | Whole TRISO particles the built lattice places in each fuel zone<br>(gh:#566): the design's `particles_per_pebble` when the pitch search<br>reaches it, else the closest count it found. `None` for the one-ball<br>[`assemble`]. |
| `rod_lower_end_zt_cm` | `Option<f64>` | TECDOC `z_T` \[cm\] (downward from the model top) of the ten control<br>rods' lower end as built (gh:#580): 119.2 withdrawn to 394.2 fully<br>inserted. `None` when no rods are built (no reflector, the<br>`OUTRAM_HTR10_NO_WITHDRAWN_RODS` ablation, or the one-ball [`assemble`])<br>**and, since 2026-10-23 (gh:#1025), when the rods are not all at one<br>position**: read [`Self::rod_lower_ends_zt_cm`] then. |
| `rod_lower_ends_zt_cm` | `Option<[f64; 10]>` | `z_T` \[cm\] of EACH rod's lower end as built, in the order of<br>[`super::reflector_geometry::control_rod_azimuths_deg`] (gh:#1025).<br>`None` exactly when no rods are built. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: None, name: \"unreachable_code\" },\nkind: Allow},\nLintCheck {lint_name: LintName { tool: None, name: \"unused_variables\" },\nkind: Allow}])]")`

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

#### Function `assemble_explicit_triso_with`

**Attributes:**

- `MustUse { reason: None }`

[`assemble_explicit_triso`] built to `design` (gh:#566, gh:#580): the
fuel-ball fraction, fuel-zone radius, TRISO radii and count, and the
control-rod insertion are the design's. With
[`Htr10CoreDesign::default`] the geometry is the record's, cell for cell
(`the_default_design_builds_the_geometry_unchanged`). What a design cannot
change (the 6 cm ball, materials, fertile or poison balls) is listed in
[`super::core_design`].

# Panics
As [`assemble_explicit_triso`], and if `design` fails
[`Htr10CoreDesign::check`].

```rust
pub fn assemble_explicit_triso_with(n_rings: usize, layers: usize, majorant_index: usize, design: &super::core_design::Htr10CoreDesign) -> AssembledCore { /* ... */ }
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

## Module `core_design`

# What the HTR-10 geometry builders take as a design

Added 2026-10-05 (gh:#566, gh:#580) so that a caller (Dhoby Ghaut's
workbench) can change the parts of the model the builders are able to
represent, and the assembled geometry the solver transports changes with
them. [`Htr10CoreDesign::default`] is the model of the RMC code-to-code
record, value for value: every builder called with it builds the same
geometry, cell for cell, as before this type existed (pinned by
`the_default_design_builds_the_geometry_unchanged` and by the workbench's
geometry fixture).

# What it can change

| field | what it moves in the geometry | source of the default |
|---|---|---|
| [`fuel_ball_fraction`](Htr10CoreDesign::fuel_ball_fraction) | the fuel/dummy split over the balls above the bed floor (same low-discrepancy rule) | Li, Yu & Wei (2014) Table 1, 0.57 |
| [`fuel_zone_radius_cm`](Htr10CoreDesign::fuel_zone_radius_cm) | the fuelled-zone sphere of every fuel ball, and the TRISO lattice clipped to it | Li (2014) Table 2, 2.5 cm |
| [`triso_radii_cm`](Htr10CoreDesign::triso_radii_cm) | the five TRISO shells | TECDOC-1382 / Li Table 2, as adjudicated (op-867c.12) |
| [`particles_per_pebble`](Htr10CoreDesign::particles_per_pebble) | the TRISO lattice pitch, solved so the zone holds exactly this many whole particles | Li Table 2 and Şeker & Çolak (2003) p. 266, 8335 |
| [`rod_insertions`](Htr10CoreDesign::rod_insertions) | the axial position of EACH of the ten explicit control rods (since 2026-10-23, gh:#1025; one shared value before) | TECDOC-1382 § 4.1.1.5: withdrawn (0) is the benchmark state |

# What it cannot change, stated plainly

- **The pebble outer radius.** Şeker's 13-ball lattice cell and the DEM
  bed's overlap splitting are built for 6 cm balls; a different ball needs a
  different lattice, which no builder here makes.
- **Fertile or poison pebbles.** Every ball is fuel or graphite (dummy).
- **Materials** (kernel composition, enrichment, matrix, coolant): those are
  [`super::materials`]' and [`super::data`]'s, fixed to the benchmark.
- ~~**Rod insertion is one position for all ten rods.** A single rod (the
  B32 / B42 problems) is not representable.~~ **CHANGED 2026-10-23
  (gh:#1025, maintainer: "move all rods individually, or sync their
  movements together"):** each rod has its own insertion,
  [`Htr10CoreDesign::rod_insertions`], indexed as
  [`super::reflector_geometry::control_rod_azimuths_deg`] orders the
  rods; [`Htr10CoreDesign::with_bank`] puts all ten at one position and
  builds, cell for cell, the geometry the single shared value built.

# Rod insertion, defined

Each entry of `rod_insertions` is the fraction of the published travel, linear in the
position of the rod's lower end: 0 puts it at
[`LOWER_END_WITHDRAWN_CM`] (`z_T` = 119.2 cm, in the top reflector, the
benchmark's state), 1 at [`LOWER_END_INSERTED_CM`] (`z_T` = 394.2 cm),
the two positions TECDOC-1382 § 4.1.1.5 states. It is a position, not a
worth fraction: the rod (264.7 cm) is shorter than its travel (275 cm), and
worth depends on where the absorber sits in the flux.

# Units

Lengths are plain `f64` in **centimetres**, named `_cm`, as every length of
`htr10_rmc`'s geometry is (`AssembledCore`, the `HTR10_*_CM` constants).
Not `uom`: a `Length` stores metres, and the round trip cm -> m -> cm moves
119.2 cm to 119.19999999999999 cm, which would have changed the record's
geometry in its last bit (measured 2026-10-05 by
`the_default_design_builds_the_geometry_unchanged`).

```rust
pub mod core_design { /* ... */ }
```

### Types

#### Struct `Htr10CoreDesign`

The parts of the HTR-10 model a caller may change, and the builders
represent. See the module docs for what is and is not covered.

```rust
pub struct Htr10CoreDesign {
    pub fuel_ball_fraction: f64,
    pub fuel_zone_radius_cm: f64,
    pub triso_radii_cm: [f64; 5],
    pub particles_per_pebble: usize,
    pub rod_insertions: [f64; 10],
    pub temperature_zones: bool,
    pub triso_packing: TrisoPacking,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_ball_fraction` | `f64` | Fraction \[-\] of the balls centred above the bed floor that are fuel;<br>the rest are graphite dummies. In `[0, 1]`. The conus and the tube stay<br>all dummy whatever this is (Terry 2005 sec. 2). Default 0.57. |
| `fuel_zone_radius_cm` | `f64` | Radius \[cm\] of a fuel ball's fuelled zone. Must be smaller than the<br>3 cm ball and larger than a TRISO particle. Default 2.5 cm. |
| `triso_radii_cm` | `[f64; 5]` | TRISO shell radii \[cm\], kernel outward (kernel, buffer, IPyC, SiC,<br>OPyC), strictly increasing. Default [`TRISO_RADII_CM`]. |
| `particles_per_pebble` | `usize` | Whole TRISO particles in each fuel zone; the lattice pitch is solved<br>to give exactly this count. Default [`PARTICLES_PER_PEBBLE`]. |
| `rod_insertions` | `[f64; 10]` | Control-rod insertion of each rod, fraction \[-\] of the published<br>travel, `[0, 1]`: 0 withdrawn (the benchmark), 1 fully inserted. Rod<br>`i` sits in the channel at<br>[`super::reflector_geometry::control_rod_azimuths_deg`]`()[i]`.<br>~~`rod_insertion: f64`, all ten rods together~~ **CHANGED 2026-10-23<br>(gh:#1025)**: [`Self::with_bank`] is that case. |
| `temperature_zones` | `bool` | **Temperature zones** (gh:#1138): place the extra material slots<br>(`mat::MODERATOR_BALL` for the graphite balls, a copy of each<br>reflector composition per top, side and bottom region) so every<br>material belongs to one of `super::zones`' eight zones. Off by default:<br>the record's geometry, cell for cell. `super::zones::zoned_material_set`<br>supplies the materials it needs. |
| `triso_packing` | `TrisoPacking` | How the TRISO particles sit in a fuel zone (gh:#1243): the record's<br>regular lattice, or random. Read by the explicit (DEM) bed only; the<br>Şeker bed always uses the lattice. Default [`TrisoPacking::Lattice`]. |

##### Implementations

###### Methods

- ```rust
  pub fn with_temperature_zones(self: Self) -> Self { /* ... */ }
  ```
  This design with [`Self::temperature_zones`] on.

- ```rust
  pub fn moderator_ball_slot(self: &Self) -> usize { /* ... */ }
  ```
  Material slot of a graphite (dummy) ball: `mat::MODERATOR_BALL` with

- ```rust
  pub fn with_bank(self: Self, f: f64) -> Self { /* ... */ }
  ```
  This design with **all ten rods at insertion `f`** (the bank, synced):

- ```rust
  pub fn with_rod(self: Self, rod: usize, f: f64) -> Self { /* ... */ }
  ```
  This design with rod `rod` alone moved to insertion `f`, the other

- ```rust
  pub fn bank_insertion(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The bank insertion when every rod sits at the same position, `None`

- ```rust
  pub fn lower_end_zt_cm_of(f: f64) -> f64 { /* ... */ }
  ```
  TECDOC axial coordinate `z_T` \[cm\] (downward from the model top)

- ```rust
  pub fn rod_lower_ends_zt_cm(self: &Self) -> [f64; 10] { /* ... */ }
  ```
  `z_T` \[cm\] of each rod's lower end, [`Self::lower_end_zt_cm_of`]

- ```rust
  pub fn rod_insertion_for_lower_end_zt_cm(z_t_cm: f64) -> f64 { /* ... */ }
  ```
  The insertion fraction that puts a rod's lower end at `z_t_cm`

- ```rust
  pub fn check(self: &Self) -> Result<(), String> { /* ... */ }
  ```
  Why the builders would refuse this design, if they would: a fraction

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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
    The model of the RMC code-to-code record (see the module table).

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `TrisoPacking`

**How the TRISO particles sit in a fuel zone** (gh:#1243).

```rust
pub enum TrisoPacking {
    Lattice,
    Random {
        realisations: usize,
        seed: u64,
    },
}
```

##### Variants

###### `Lattice`

The record's regular cubic lattice, the same in every fuel pebble.

###### `Random`

**Random sequential addition** of exactly the lattice's particle count
(so the fuel mass is unchanged), every particle wholly inside the fuel
zone (`outram_mc_libs::dh_universe::pack_count_in_ball`), each packing
wrapped in OpenMC's tracking lattice
(`outram_mc_libs::pebble_beds::triso_lattice::create_triso_lattice`).
`realisations` different packings (seeds `seed`, `seed + 1`, ...);
fuel pebble number i takes packing i mod `realisations`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `realisations` | `usize` | Different packings, 1..=64. |
| `seed` | `u64` | Seed of the first. |

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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Constant `TRISO_RADII_CM`

The TRISO shell radii of the record's model \[cm\], kernel outward:
kernel, buffer, IPyC, SiC, OPyC. Adjudicated radii (op-867c.12):
TECDOC-1382, 90 µm buffer; equal to `TrisoSpec::HTR10_LI2014`.

```rust
pub const TRISO_RADII_CM: [f64; 5] = _;
```

#### Constant `PARTICLES_PER_PEBBLE`

Whole TRISO particles per fuel pebble: Li (2014) Table 2, and Şeker &
Çolak (2003) p. 266 (*"verified to be 8335"*).

```rust
pub const PARTICLES_PER_PEBBLE: usize = 8335;
```

## Module `explicit_bed`

# The HTR-10 core built from an explicit list of pebble centres (a DEM fill)

NEW WORK, not a port. Added 2026-10-05 so the HTR-10 Monte Carlo model can
be built on the bed a DEM pour (`outram-park-fork-liggghts`) actually
produced, instead of only on Şeker & Çolak (2003)'s ordered lattice
([`super::bed::SekerBed`]).

[`assemble_explicit_triso_from_centres`] takes pebble centres in the **DEM
frame** and a fuel/dummy flag per pebble, and returns the same
[`AssembledCore`] that [`super::core_model::assemble_explicit_triso`]
returns, with [`PebbleBed::Explicit`] as its bed. Everything around the bed
(TRISO particle and lattice, bed envelope, conus, discharge tube, cavity,
TECDOC zone map, borings, withdrawn rods) is the same code,
[`super::core_shell`], so the two builders cannot drift apart.

# Frame

- **DEM frame** (input): metres, `z = 0` is the bed floor (the top of the
  conus, where the core cylinder meets the cone), the conus below it to
  `z = -0.36946 m`, the discharge tube below that. This is the frame of
  `reference-data/liggghts/in.htr10_conus` and its outputs.
- **MC frame** (output): cm, the bed floor at `-bed_half_height`.

`bed_half_height` is half the height of the **top of the highest pebble**
above the floor, so that `z_MC = 100 z_DEM - bed_half_height`. The top of
the highest ball is Şeker's own definition of the bed top (`SekerBed`), and
it is the only choice under which the bed envelope's top plane cuts no
pebble. The cavity above the bed is fixed hardware measured from the floor
([`super::core_model::cavity_above_bed`]), so the choice moves no material:
between the highest ball and the cavity top there is helium either way.

A poured surface is rough, so the highest ball is not a robust *loading
height*. [`ExplicitBed::surface_height`] records the robust one, the 99th
percentile of the centre heights above the floor plus one radius (the
measure the liggghts examples use). It is reported, not used to place
anything. The reference is matched by ball count
([`PebbleBed::core_balls`], balls centred above the floor), exactly as for
Şeker's bed.

# Overlap: lens split by the bisector plane (nothing shrunk)

A soft-sphere DEM bed has every contact slightly interpenetrating (at
`E = 5e8 Pa` up to ~1.7 % of the radius). Two overlapping CSG spheres make
the lens between them belong to two cells. **No pebble is shrunk or moved.**
Instead each overlapping pair's lens is split by the plane bisecting the
centre line (the plane through the midpoint, normal to the centre line):
each ball's graphite cell is its sphere intersected with the half-space on
its own side. For two spheres of **equal radius** the bisector is the
radical plane, and the cap of ball *i* beyond it lies wholly inside ball
*j*, so the split moves exactly half of each lens to each ball: no carbon or
fuel is invented or removed beyond the lens itself, whose volume is counted
once instead of twice. That once-counted lens volume is the only change in
inventory, reported as [`ExplicitBed::lens_volume_fraction`] (lens volume
over the summed nominal sphere volume). The 2.5 cm fuel zone is never
touched as long as the centre distance is at least `2 x 2.5 = 5.0` cm (an
overlap of 1.0 cm); a bed with a closer pair is refused with a panic rather
than silently clipped.

Pebbles touching (overlap below [`TOUCH_TOLERANCE_CM`]) get no plane.

**What the DEM wall does.** The DEM walls are soft too, so a ball can sit a
few hundredths of a millimetre into the side wall or the faceted conus. The
bed envelope region clips that cap (it is graphite in the reflector's
place, not inside the bed). It is measured as
[`ExplicitBed::max_wall_penetration`] and not corrected. A ball whose
CENTRE is outside the container is a frame error and panics.

# The discharge tube below the DEM column

The DEM tube is short (`in.htr10_conus`: 25 cm, valve at `z = -0.61946 m`);
the MC tube runs [`HTR10_BOTTOM_REFLECTOR_CM`] = 221.236 cm below the
conus floor. Below the lowest DEM ball the tube is filled with the **same
balls the default Şeker model puts there**: the discharge-tube balls of a
`SekerBed` (all dummy, every ball whole, rejected at the tube wall),
translated so its conus floor is this bed's, keeping only balls whose top
is at or below `cut = min(conus floor, bottom of the lowest DEM ball)`. The
plane `z = cut` therefore separates the two sets and no DEM ball can
overlap a filler ball. The helium gap between the highest filler ball and
`cut` is reported as [`ExplicitBed::tube_gap`]. The filler is ordered, not
poured: it is the default model's treatment, stated, not a DEM result.
Under the `OUTRAM_HTR10_HOMOG_TUBE` ablation the tube is the old smear,
no filler is built, and DEM balls centred below the conus floor are dropped
(counted in [`ExplicitBed::dropped_dem_balls`]); a ball straddling the
floor is then cut by it, as that ablation already does to Şeker's balls.

# Tiles

The bed lattice is a `HexLattice` of Şeker's tile shape (pitch 16.392 cm,
height 9.798 cm), used purely as a spatial index. Every ball is placed in
**every tile it intersects** (exact hexagonal-prism / sphere distance), at
its tile-local centre, as the same cells as Şeker's: a fuel ball is a
fuel-zone cell filled with the TRISO lattice translated to the centre plus
a graphite shell, a dummy ball a graphite sphere, the rest of the tile
helium. Cell ids follow [`super::core_model::tile_cell_role`]. Unlike
Şeker's bed, almost every tile is unique, so each non-empty tile is its own
universe; empty tiles and the lattice `outer` share one all-helium universe.

# Status

AI-drafted, awaiting human review. The geometry is drawn in
`crates/nee_soon/verification_and_validation/htr10_dem_bed_images/`. No
k_eff on a DEM bed has been validated; see that folder's README for the
one smoke run and what it does and does not show. **Since 2026-10-08
(gh:#787)** one full-statistics k is recorded, on the gh:#216 bed cut to
the lattice's 16 681 balls by [`trim_to_core_balls`]: 0.989293 ±
0.000962, −583 ± 143 pcm from the lattice at N = 12, on one pour
(`verification_and_validation/htr10_dem_bed_keff_2026_10_08/`).

```rust
pub mod explicit_bed { /* ... */ }
```

### Types

#### Enum `ExplicitBallSource`

Where a ball of an [`ExplicitBed`] came from.

```rust
pub enum ExplicitBallSource {
    Dem(usize),
    TubeFiller,
}
```

##### Variants

###### `Dem`

The caller's centre list, at this index.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `TubeFiller`

A Şeker discharge-tube ball below the DEM column (see the module docs).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `ExplicitBed`

A pebble bed given ball by ball (normally a DEM fill), in the MC frame.
Built by [`assemble_explicit_triso_from_centres`]; see the module docs for
the frame, the overlap split and the tube.

```rust
pub struct ExplicitBed {
    pub radius: f64,
    pub centres: Vec<[f64; 3]>,
    pub fuel: Vec<bool>,
    pub source: Vec<ExplicitBallSource>,
    pub overlaps: Vec<(usize, usize)>,
    pub tile_balls: std::collections::BTreeMap<(i32, i32, i32), Vec<usize>>,
    pub n_rings: usize,
    pub n_levels: usize,
    pub z_bottom: f64,
    pub tile_pitch: f64,
    pub tile_height: f64,
    pub bed_radius: f64,
    pub bed_bottom: f64,
    pub bed_top: f64,
    pub conus_floor: f64,
    pub tube_radius: f64,
    pub container_bottom: f64,
    pub surface_height: f64,
    pub dem_to_mc_dz: f64,
    pub tube_cut: Option<f64>,
    pub tube_gap: f64,
    pub max_overlap: f64,
    pub lens_volume_fraction: f64,
    pub max_wall_penetration: f64,
    pub dropped_dem_balls: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `f64` | Ball radius \[cm\]. |
| `centres` | `Vec<[f64; 3]>` | Ball centres \[cm\], MC frame. |
| `fuel` | `Vec<bool>` | Fuel flag per ball. |
| `source` | `Vec<ExplicitBallSource>` | Origin of each ball. |
| `overlaps` | `Vec<(usize, usize)>` | Overlapping pairs `(i, j)`, `i < j`, each split by its bisector plane. |
| `tile_balls` | `std::collections::BTreeMap<(i32, i32, i32), Vec<usize>>` | Balls per non-empty tile, keyed by `(a, b, level)`. |
| `n_rings` | `usize` | Hex rings of the lattice. |
| `n_levels` | `usize` | Axial lattice levels. |
| `z_bottom` | `f64` | z \[cm\] of the bottom face of lattice level 0. |
| `tile_pitch` | `f64` | Tile pitch (flat to flat) \[cm\]. |
| `tile_height` | `f64` | Tile height \[cm\]. |
| `bed_radius` | `f64` | Bed cylinder radius \[cm\]. |
| `bed_bottom` | `f64` | Bed floor \[cm\] (= top of the conus = DEM `z = 0`). |
| `bed_top` | `f64` | Bed top \[cm\]: the top of the highest ball. |
| `conus_floor` | `f64` | Conus floor \[cm\]. |
| `tube_radius` | `f64` | Discharge-tube radius \[cm\]. |
| `container_bottom` | `f64` | Bottom of the container \[cm\] (the tube bottom, or the conus floor<br>under the `OUTRAM_HTR10_HOMOG_TUBE` ablation). |
| `surface_height` | `f64` | Robust loading height \[cm\] above the floor: 99th percentile of the<br>centre heights above the floor, plus one radius. Reported only. |
| `dem_to_mc_dz` | `f64` | `z_MC - 100 z_DEM` \[cm\]. |
| `tube_cut` | `Option<f64>` | The plane \[cm\] separating DEM balls (above) from tube filler balls<br>(below); `None` when no filler is built. |
| `tube_gap` | `f64` | Helium gap \[cm\] between the highest filler ball and `tube_cut`. |
| `max_overlap` | `f64` | Largest pair overlap `2r - d` \[cm\] (0 if none). |
| `lens_volume_fraction` | `f64` | Summed lens volume over the summed nominal ball volume \[-\]. |
| `max_wall_penetration` | `f64` | Largest depth \[cm\] a ball reaches beyond the container wall (clipped<br>by the bed envelope; 0 if none). |
| `dropped_dem_balls` | `usize` | DEM balls dropped (centre below the conus floor with no explicit tube). |

##### Implementations

###### Methods

- ```rust
  pub fn lattice_centre_z(self: &Self) -> f64 { /* ... */ }
  ```
  z \[cm\] of the lattice centre.

- ```rust
  pub fn core_balls(self: &Self) -> usize { /* ... */ }
  ```
  Balls centred above the bed floor (the count Şeker's Table 3

- ```rust
  pub fn eligible_and_fuel_balls(self: &Self) -> (usize, usize) { /* ... */ }
  ```
  Balls centred above the floor, and of which fuelled.

- ```rust
  pub fn tile_centre(self: &Self, a: i32, b: i32, level: i32) -> [f64; 3] { /* ... */ }
  ```
  Tile-local centre \[cm\] of tile `(a, b, level)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Enum `DemBedError`

Why a DEM bed could not be read or cut ([`read_dem_centres_csv`],
[`trim_to_core_balls`]).

```rust
pub enum DemBedError {
    Header(String),
    Row {
        line: usize,
        why: String,
    },
    TooFewBalls {
        available: usize,
        wanted: usize,
    },
}
```

##### Variants

###### `Header`

The first line is not the `id,x,y,z,...` header of
`reference-data/liggghts/`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Row`

A row has too few fields or a field is not a number.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `line` | `usize` |  |
| `why` | `String` |  |

###### `TooFewBalls`

The bed holds fewer balls above the floor than were asked for.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `available` | `usize` |  |
| `wanted` | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
### Functions

#### Function `lens_volume`

**Attributes:**

- `MustUse { reason: None }`

Volume \[cm^3\] of the lens shared by two spheres of radius `r` whose
centres are `d` apart (`d < 2r`).

```rust
pub fn lens_volume(r: f64, d: f64) -> f64 { /* ... */ }
```

#### Function `paper_fuel_assignment`

**Attributes:**

- `MustUse { reason: None }`

**Fuel/dummy flags by the literature's rule**, for centres in the DEM
frame (`z = 0` the bed floor): balls centred above the floor take the
57:43 split ([`super::table1::FUEL_BALL_FRACTION`]) by the same
low-discrepancy rule `SekerBed` uses, in order of height from the floor up
(then radius, then angle); balls centred in the conus or the tube are all
dummy (Terry 2005 sec. 2, TECDOC-1382 p. 235, Şeker & Çolak 2003 p. 267).

```rust
pub fn paper_fuel_assignment(centres: &[[uom::si::f64::Length; 3]]) -> Vec<bool> { /* ... */ }
```

#### Function `fuel_assignment_at`

**Attributes:**

- `MustUse { reason: None }`

[`paper_fuel_assignment`] with fraction `f` (clamped to `[0, 1]`) of the
balls above the floor fuelled instead of 0.57 (gh:#566): the same
eligibility, order and low-discrepancy rule.

```rust
pub fn fuel_assignment_at(centres: &[[uom::si::f64::Length; 3]], f: f64) -> Vec<bool> { /* ... */ }
```

#### Function `assemble_explicit_triso_from_centres`

**Attributes:**

- `MustUse { reason: None }`

**Assemble the HTR-10 core on an explicit pebble list** (a DEM fill).

# Parameters
- `centres` — pebble centres in the **DEM frame**: `z = 0` the bed floor
  (top of the conus), the conus and the discharge tube below it, the core
  axis at `x = y = 0`. Every pebble is an HTR-10 6 cm ball.
- `is_fuel` — one flag per centre ([`paper_fuel_assignment`] gives the
  literature's 57:43 split with an all-dummy conus and tube).
- `n_rings` — a FLOOR on the bed-lattice ring count, as for
  [`super::core_model::assemble_explicit_triso`] (the lattice always tiles
  the 90 cm core).
- `majorant_index` — the bed's delta-tracking majorant (`usize::MAX`
  surface-tracks the bed).

Everything outside the bed is [`super::core_shell`], the code
`assemble_explicit_triso` uses. The ablation knobs that act on the
envelope (`OUTRAM_HTR10_NOREFL`, `_REFLECTIVE`, `_HOMOG_TUBE`,
`_NO_ZONE_MAP`, `_NO_WITHDRAWN_RODS`) apply; the fuel-identity knobs
(`_ALLFUEL`, `_FUEL_CONUS`) do not, because the caller gives the identity.

# Panics
If the lengths differ, the list is empty, a centre lies outside the
container, two centres are closer than 5.0 cm (the fuel zone would be
cut), the bed is taller than the core cavity, or a tile would hold more
than [`TILE_SITE_STRIDE`] balls.

```rust
pub fn assemble_explicit_triso_from_centres(centres: &[[uom::si::f64::Length; 3]], is_fuel: &[bool], n_rings: usize, majorant_index: usize) -> super::core_model::AssembledCore { /* ... */ }
```

#### Function `assemble_explicit_triso_from_centres_with`

**Attributes:**

- `MustUse { reason: None }`
- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"too_many_lines\" },\nkind: Allow}])]")`

[`assemble_explicit_triso_from_centres`] built to `design` (gh:#566,
gh:#580): its fuel-zone radius, TRISO radii and count, and rod insertion.
The fuel identity is the caller's `is_fuel` (use [`fuel_assignment_at`]
for the design's fraction); the design's `fuel_ball_fraction` is not read
here. Default design: the same geometry as before it existed.

# Panics
As [`assemble_explicit_triso_from_centres`], and if `design` fails
[`Htr10CoreDesign::check`]; two centres must be at least twice the
design's fuel-zone radius apart.

```rust
pub fn assemble_explicit_triso_from_centres_with(centres: &[[uom::si::f64::Length; 3]], is_fuel: &[bool], n_rings: usize, majorant_index: usize, design: &super::core_design::Htr10CoreDesign) -> super::core_model::AssembledCore { /* ... */ }
```

#### Function `read_dem_centres_csv`

Pebble centres, DEM frame, from the text of an `id,x,y,z,...` CSV (the
`reference-data/liggghts/` format, SI metres, e.g.
`htr10_conus_presettled_mu10_mur00.csv`), in file order.

# Errors
[`DemBedError::Header`] or [`DemBedError::Row`] on a malformed file.

```rust
pub fn read_dem_centres_csv(text: &str) -> Result<Vec<[uom::si::f64::Length; 3]>, DemBedError> { /* ... */ }
```

#### Function `trim_to_core_balls`

**A poured bed cut to a ball count** (gh:#787): every centre at or below
the bed floor (`z <= 0`, DEM frame: the conus and the tube) is kept, and of
the centres above it the lowest `core_balls`, ordered by height, then `x`,
then `y`. This is the rule of the web demo's beds view
(`dhoby-ghaut/examples/common/htr10_beds.rs`, `Bed::trimmed_to_core`), so a
`k` computed on the result is the `k` of the bed that view draws. The top
of the cut bed is a flat cut through the packing, not a poured surface.

Returned in that order: the kept floor-and-below centres in input order,
then the core centres from the floor up.

# Errors
[`DemBedError::TooFewBalls`] if fewer than `core_balls` centres lie above
the floor.

```rust
pub fn trim_to_core_balls(centres: &[[uom::si::f64::Length; 3]], core_balls: usize) -> Result<Vec<[uom::si::f64::Length; 3]>, DemBedError> { /* ... */ }
```

### Constants and Statics

#### Constant `PEBBLE_RADIUS_CM`

HTR-10 pebble radius \[cm\] (6 cm balls). Every centre handed to
[`assemble_explicit_triso_from_centres`] is taken to be a ball of this
radius.

```rust
pub const PEBBLE_RADIUS_CM: f64 = 3.0;
```

#### Constant `FUEL_ZONE_RADIUS_CM`

Fuel-zone radius \[cm\] of a fuel pebble.

```rust
pub const FUEL_ZONE_RADIUS_CM: f64 = 2.5;
```

#### Constant `TOUCH_TOLERANCE_CM`

Overlap \[cm\] below which two balls count as touching and get no
bisector plane.

```rust
pub const TOUCH_TOLERANCE_CM: f64 = 1.0e-9;
```

#### Constant `RANDOM_TRISO_LATTICE_ID`

Id of the first random TRISO packing's tracking lattice (gh:#1243).

```rust
pub const RANDOM_TRISO_LATTICE_ID: i32 = 100;
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
  at the withdrawn position by default, and since 2026-10-05 at any
  position between [`LOWER_END_WITHDRAWN_CM`] and [`LOWER_END_INSERTED_CM`]
  (`super::core_design::Htr10CoreDesign::rod_insertion`, gh:#580). They are built from this module's
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Constant `B42_LOWER_END_ZT_CM`

The rod lower-end positions `z_T` \[cm\] at which TECDOC-1382's problem
B42 asks for the differential worth of one rod (§ 4.1.2.4, printed
p. 243; the same list again with Table 4-9, p. 248), in increasing
insertion. The bank curve of gh:#1025 is computed at these positions too,
so it can be read beside any single-rod curve on the same axis; it is NOT
a B42 result (B42 moves one rod, the bank model moves ten).

```rust
pub const B42_LOWER_END_ZT_CM: [f64; 7] = _;
```

#### Constant `EXPERIMENT_START_ZT_CM`

The rod lower-end position `z_T` \[cm\] the measured rod-worth
calibration started from: "moved from z = 1712mm to z = 3942mm"
(TECDOC-1382 § 4.2.1.5, printed p. 252; restricted source, cited only).

```rust
pub const EXPERIMENT_START_ZT_CM: f64 = 171.2;
```

#### Constant `BANK_CURVE_ZT_CM`

The bank positions of the gh:#1025 k-vs-position curve \[cm\], lower end
`z_T`, increasing insertion: withdrawn, the experiment's start, then
[`B42_LOWER_END_ZT_CM`]. Every entry is a position a source states;
none was chosen from a result.

```rust
pub const BANK_CURVE_ZT_CM: [f64; 9] = _;
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

# Where the tapes are read from (gh:#581, 2026-10-05)

[`Htr10DataConfig::tapes`] is [`TapeSource::Workspace`] by default:
`reference-data/endf/` and, for the nickel tapes, the ACE submodule, as
always. [`TapeSource::Folder`] reads every tape from one folder the caller
names instead, flat or an extracted library (`neutrons/`,
`thermal_scatt/`), finding each tape by the MAT and NSUB of the
workspace copy's header rather than by file name
([`njoy_outram_park_fork::endf_folder`]). A tape the folder lacks is an
error ([`Htr10DataError::NotInFolder`]), never a silent fall-back to the
workspace copy; the one exception is the documented free-gas fall-back
for a missing SiC law, which applies to either source.

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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `ThermalLawSource`

Where the graphite and SiC thermal laws come from (gh:#1138).

```rust
pub enum ThermalLawSource {
    Tapes,
    Leapr,
}
```

##### Variants

###### `Tapes`

ABLATION since 2026-11-03 (`OUTRAM_HTR10_TAPE_LAWS`): the published
ENDF/B-VIII.0 `tsl-*.endf` tapes, read at the tabulated temperature
THERMR's tolerance matches. ~~**Default**~~ The recorded runs' laws
(graphite at 296 K for a 300.15 K run): every HTR-10 record before
2026-11-03 was made with these.

###### `Leapr`

**Default since 2026-11-03** (maintainer: "just use LEAPR by
default"): generated by LEAPR from the embedded ENDF/B-VIII.0 decks
**at the requested temperature exactly**, as the UO2 laws already are
(the workspace's laws come from a deck, tapes being oracles:
maintainer, 2026-10-14). LEAPR reproduces the published tapes' S(α,β)
at every temperature they tabulate (gh:#1140). Built by both the jobs
path ([`super::data_jobs`]) and [`load_htr10_nuclides`]. The VII.0
graphite tape has no deck and is read from its tape regardless.

##### Implementations

###### Methods

- ```rust
  pub fn leapr_deck(self: Self, law: ThermalLaw) -> Option<SabMaterial> { /* ... */ }
  ```
  The LEAPR deck `law` is generated from under this source, or `None`

- ```rust
  pub fn from_env() -> Self { /* ... */ }
  ```
  The source a run uses: the default, or the tapes under the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
    pub thermal_laws: ThermalLawSource,
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub tapes: TapeSource,
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
| `thermal_laws` | `ThermalLawSource` | Source of the graphite and SiC laws (gh:#1138). |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature every nuclide is reconstructed and broadened at. 300.15 K<br>(27 °C) is the temperature Li, Yu & Wei (2014) and Şeker & Çolak (2003)<br>state. |
| `tapes` | `TapeSource` | Where the tapes are read from: the workspace (default) or a folder<br>the user chose (gh:#581). Changes no physics choice: the same tapes<br>are read, found by identity. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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
    The correct-physics default: see the module docs.

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `TapeSource`

Where [`load_htr10_nuclides`] reads the evaluated tapes.

```rust
pub enum TapeSource {
    Workspace,
    Folder(std::path::PathBuf),
}
```

##### Variants

###### `Workspace`

Each tape at its [`Tape::path`]: `reference-data/endf/` (honouring
`OUTRAM_PARK_REFERENCE_DATA_DIR`) or the ACE submodule.

###### `Folder`

Every tape from this folder: flat, or an extracted ENDF library with
`neutrons/` and `thermal_scatt/` sub-folders. Each tape is found by
the MAT and NSUB of the workspace copy's header (the library's own
file names may differ), by file name when no workspace copy exists.

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

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
    IronMetal,
    AluminumMetal,
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

###### `IronMetal`

Iron metal, generated by LEAPR from the ENDF/B-VIII.0 deck
`tsl-026_Fe_056.leapr` (MAT 101 as supplied). Not used by HTR-10;
the HTTR model binds it to Fe-56 in its Alloy 800H control-rod
cladding, as the IRPhEP evaluators' deck binds `Fe.00t` (added
2026-10-16, `crate::httr`).

###### `AluminumMetal`

Aluminium metal, generated by LEAPR from `tsl-013_Al_027.leapr`
(MAT 101 as supplied). Not used by HTR-10; bound to Al-27 in the
HTTR Alloy 800H (added 2026-10-16).

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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- ```rust
  pub fn processing_jobs(self: &Self) -> Vec<ProcessingJob> { /* ... */ }
  ```
  Every distinct thermal law (in first-use order, as the loader takes

- ```rust
  pub fn tempered_jobs(self: &Self, entries: &[(usize, f64)]) -> Vec<TemperedJob> { /* ... */ }
  ```
  **The jobs of a nuclide array at several temperatures** (gh:#1138):

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
    NotInFolder {
        name: String,
        file: String,
        mat: Option<i32>,
        nsub: Option<i64>,
        folder: std::path::PathBuf,
        detail: String,
    },
    LoadFailed {
        name: String,
        path: std::path::PathBuf,
        reason: String,
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

###### `NotInFolder`

[`TapeSource::Folder`] does not hold a tape the plan needs (or holds
several candidates and none has the wanted name).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Slot or law name. |
| `file` | `String` | The workspace copy's file name. |
| `mat` | `Option<i32>` | MAT looked for. |
| `nsub` | `Option<i64>` | Sub-library looked for. |
| `folder` | `std::path::PathBuf` | The folder searched. |
| `detail` | `String` | Extra detail (the ambiguous candidates), or empty. |

###### `LoadFailed`

A tape is present but did not load.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Slot or law name. |
| `path` | `std::path::PathBuf` | Path tried. |
| `reason` | `String` | Why: the loader's own error (the `NjoyError` text), kept rather<br>than dropped by `.ok()` (GitHub #672 / #934). Empty only when the<br>loader gave none. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `FreeSlots`

**Every nuclide of a slot list loaded FREE, and the thermal laws to be
associated per material** (gh:#1635 item 3, gh:#462).

`table_of[i]` is the index into `tables` of the law slot `i` is to carry,
or `None` (no law, or the documented free-gas fall-back for a SiC law the
source lacks). The same shape serves a nuclide array that is not slot-
indexed (the per-temperature array of `ParametricStudy`): any vector of
`Option<usize>` of the array's length.

```rust
pub struct FreeSlots {
    pub nuclides: Vec<outram_mc_libs::material::nuclide::Nuclide>,
    pub tables: Vec<outram_mc_libs::material::thermal::ThermalScattering>,
    pub table_of: Vec<Option<usize>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclides` | `Vec<outram_mc_libs::material::nuclide::Nuclide>` | One free nuclide per slot, in slot order. |
| `tables` | `Vec<outram_mc_libs::material::thermal::ThermalScattering>` | Each distinct law, loaded once, in first-use order. |
| `table_of` | `Vec<Option<usize>>` | Per slot, the table it carries. |

##### Implementations

###### Methods

- ```rust
  pub fn bind_per_material(self: &Self, materials: &[Material]) -> Result<(Vec<Material>, Vec<Nuclide>), String> { /* ... */ }
  ```
  Lower the slots' laws onto `materials` and the nuclide array, per

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

#### Function `resolve_tapes`

Every tape `layout` reads, and where [`load_htr10_nuclides`] would read it
from under `cfg.tapes`: `(slot or law name, path)`, nuclide slots after
the thermal laws. Reads only headers, never the data, so a caller can
check a folder before a load that takes minutes. A SiC law the source
lacks is left out (the load falls back to free gas for it).

# Errors
[`Htr10DataError::NotInFolder`] for the first tape a folder does not hold.

```rust
pub fn resolve_tapes(cfg: &Htr10DataConfig, layout: &Htr10NuclideLayout) -> Result<Vec<(String, std::path::PathBuf)>, Htr10DataError> { /* ... */ }
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

#### Function `load_slots_with_progress`

Read every tape in `slots` at `temperature`, bind each slot's thermal
law (each distinct law loaded once), record each item in `diag` together
with `notes`, and report progress. This is the body of
[`load_htr10_nuclides_with_progress`], made independent of the HTR-10
layout on 2026-10-16 so the HTTR model (`crate::httr`) reads its own
nuclide list through the same code rather than a second copy of it.
HTR-10's processing, order and result are unchanged.

Reconstruction tolerance is NJOY's 1e-3.

# Errors

As [`load_htr10_nuclides`], except that the gh:#339 Fe-57 check is the
caller's (HTR-10's wrapper keeps it).

```rust
pub fn load_slots_with_progress<F: FnMut(LoadProgress)>(slots: &[NuclideSlot], notes: &[String], temperature: uom::si::f64::ThermodynamicTemperature, tapes: &TapeSource, uo2_laws: &Uo2Laws, diag: &mut outram_mc_libs::run_diagnostics::RunDiagnostics, progress: F) -> Result<Vec<outram_mc_libs::material::nuclide::Nuclide>, Htr10DataError> { /* ... */ }
```

#### Function `load_slots_with_laws`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"too_many_arguments\" },\nkind: Allow}])]")`

[`load_slots_with_progress`] with the graphite and SiC laws' source
stated (gh:#1138): LEAPR at `temperature` exactly (the default), or the
published tapes (the `OUTRAM_HTR10_TAPE_LAWS` ablation).

Laws are bound to the nuclide slots (the deprecated nuclide-level binding,
gh:#462): this is [`load_slots_free_with_laws`] followed by
[`bind_nuclide_level`]. New code takes [`FreeSlots`] and binds per
material.

# Errors
As [`load_slots_with_progress`].

```rust
pub fn load_slots_with_laws<F: FnMut(LoadProgress)>(slots: &[NuclideSlot], notes: &[String], temperature: uom::si::f64::ThermodynamicTemperature, tapes: &TapeSource, uo2_laws: &Uo2Laws, thermal_laws: ThermalLawSource, diag: &mut outram_mc_libs::run_diagnostics::RunDiagnostics, progress: F) -> Result<Vec<outram_mc_libs::material::nuclide::Nuclide>, Htr10DataError> { /* ... */ }
```

#### Function `load_slots_free_with_laws`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"too_many_arguments\" },\nkind: Allow}])]")`

**Every slot loaded FREE, with each distinct thermal law loaded once**
(gh:#1635 item 3): the body of [`load_slots_with_laws`] without its last
step. Processing, order, notes and diagnostics are exactly those of the
legacy loader; only the binding of the laws is left to the caller, to be
done per material ([`FreeSlots::bind_per_material`]).

# Errors
As [`load_slots_with_progress`].

```rust
pub fn load_slots_free_with_laws<F: FnMut(LoadProgress)>(slots: &[NuclideSlot], notes: &[String], temperature: uom::si::f64::ThermodynamicTemperature, tapes: &TapeSource, uo2_laws: &Uo2Laws, thermal_laws: ThermalLawSource, diag: &mut outram_mc_libs::run_diagnostics::RunDiagnostics, progress: F) -> Result<FreeSlots, Htr10DataError> { /* ... */ }
```

#### Function `bind_tables_per_material`

**Associate thermal laws per material, for a nuclide array** (gh:#1635
item 3): `table_of[i]` names the table (an index into `tables`) that
nuclide `i` carries in every material that references it, as the nuclide-
level binding did, but through OpenMC's per-material association
([`assign_material_sab`], fraction 1 from
[`MaterialSab::for_materials_containing`]). The nuclides must all be free.

**Not bit-identical to the nuclide-level binding.** The array is longer by
one bound copy per (nuclide, table) pair a material uses, and the URR
stream advances by the array length; the cross sections and the physics
are the same.

# Errors
`table_of` shorter than `nuclides`, or whatever [`assign_material_sab`]
refuses (a nuclide that is already bound, an index out of range).

```rust
pub fn bind_tables_per_material(materials: &[outram_mc_libs::material::material::Material], nuclides: &[outram_mc_libs::material::nuclide::Nuclide], table_of: &[Option<usize>], tables: &[outram_mc_libs::material::thermal::ThermalScattering]) -> Result<(Vec<outram_mc_libs::material::material::Material>, Vec<outram_mc_libs::material::nuclide::Nuclide>), String> { /* ... */ }
```

#### Function `bind_nuclide_level`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: None, name: \"deprecated\" },\nkind: Allow}])]")`

**The one nuclide-level bind left in this file** (gh:#462): attach each
slot's law to its nuclide, as the loader did before gh:#1635 item 3. For
the callers not yet migrated to [`FreeSlots::bind_per_material`].

```rust
pub fn bind_nuclide_level(free: FreeSlots) -> Vec<outram_mc_libs::material::nuclide::Nuclide> { /* ... */ }
```

#### Function `load_htr10_nuclides_free`

[`load_htr10_nuclides`] with every nuclide FREE and the laws returned for
per-material binding ([`FreeSlots::bind_per_material`]); same processing,
notes and errors (gh:#1635 item 3).

# Errors
As [`load_htr10_nuclides`].

```rust
pub fn load_htr10_nuclides_free(cfg: &Htr10DataConfig, layout: &Htr10NuclideLayout, diag: &mut outram_mc_libs::run_diagnostics::RunDiagnostics) -> Result<FreeSlots, Htr10DataError> { /* ... */ }
```

#### Function `load_htr10_nuclides_free_with_progress`

[`load_htr10_nuclides_free`] with the progress callback of
[`load_htr10_nuclides_with_progress`].

# Errors
As [`load_htr10_nuclides`].

```rust
pub fn load_htr10_nuclides_free_with_progress<F: FnMut(LoadProgress)>(cfg: &Htr10DataConfig, layout: &Htr10NuclideLayout, diag: &mut outram_mc_libs::run_diagnostics::RunDiagnostics, progress: F) -> Result<FreeSlots, Htr10DataError> { /* ... */ }
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

## Module `data_jobs`

**The HTR-10 nuclide set, built in jobs that can run in different
processes** (gh:#786).

[`super::data::load_htr10_nuclides`] builds the 38 slots one after the
other, in one thread, and reconstructs a tape once per slot (C-12 three
times: free, graphite-bound and SiC-bound). The browser demo of the full
core spreads the work over a pool of Web Workers that share nothing but
messages, so it is cut into:

1. **jobs** ([`Htr10NuclideLayout::processing_jobs`]): each distinct
   evaluated tape once (RECONR + BROADR + PURR,
   `Nuclide::process_evaluation`) and each distinct thermal law once
   (THERMR from its tape, or LEAPR for the UO₂ laws). [`process_job`] runs
   one; its [`JobProduct`] encodes to `f64`s exactly
   ([`JobProduct::to_f64s`]) to be shipped;
2. **assembly** ([`assemble_slots`]): every worker builds every slot from
   the products and its own copy of each tape (`Nuclide::from_processed`,
   the cheap half), binding the laws as the loader does.

**The nuclides are the loader's, bit for bit**: `Nuclide::from_tape` is
`process_evaluation` then `from_processed` (pinned in `outram-mc-libs` by
`tests/processed_evaluation_round_trip.rs`), and reconstructing a tape
twice gives the same numbers twice. `the_jobs_path_builds_the_loaders_nuclides`
checks it on a cut-down layout; the whole layout is the opt-in
`the_jobs_path_builds_the_whole_core_set`.

Orchestration only: no physics is implemented here.

```rust
pub mod data_jobs { /* ... */ }
```

### Types

#### Enum `ProcessingJob`

One unit of the expensive processing.

```rust
pub enum ProcessingJob {
    Tape {
        tape: super::data::Tape,
        name: &'static str,
    },
    Law(super::data::ThermalLaw),
}
```

##### Variants

###### `Tape`

One evaluated tape: RECONR, BROADR and PURR, shared by every slot
that reads it. `name` is the first such slot's.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `tape` | `super::data::Tape` | The tape. |
| `name` | `&'static str` | Nuclide name (`"C12"`). |

###### `Law`

One thermal-scattering law.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::data::ThermalLaw` |  |

##### Implementations

###### Methods

- ```rust
  pub fn label(self: &Self) -> String { /* ... */ }
  ```
  What the job is, for progress lines (`"U235"`, `"graphite S(a,b)"`).

- ```rust
  pub fn tape(self: &Self, cfg: &Htr10DataConfig) -> Result<Option<Tape>, Htr10DataError> { /* ... */ }
  ```
  The tape the job reads, or `None` for a law LEAPR generates.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `TemperedJob`

**A job at a temperature** (gh:#1138): what one zone temperature needs
of a tape or a law.

```rust
pub struct TemperedJob {
    pub job: ProcessingJob,
    pub temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `job` | `ProcessingJob` | The tape or law. |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | The temperature it is processed (or the law generated) at. |

##### Implementations

###### Methods

- ```rust
  pub fn label(self: &Self) -> String { /* ... */ }
  ```
  `"U235 @ 900.00 K"`.

- ```rust
  pub fn kelvin(self: &Self) -> f64 { /* ... */ }
  ```
  The temperature, K.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `JobProduct`

What a job produces.

```rust
pub enum JobProduct {
    Nuclide(outram_mc_libs::material::processed::ProcessedEvaluation),
    Law(outram_mc_libs::material::thermal::ThermalScattering),
}
```

##### Variants

###### `Nuclide`

A tape's RECONR + BROADR + PURR products.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_mc_libs::material::processed::ProcessedEvaluation` |  |

###### `Law`

A thermal law.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_mc_libs::material::thermal::ThermalScattering` |  |

##### Implementations

###### Methods

- ```rust
  pub fn to_f64s(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  `[0, ProcessedEvaluation words...]` or `[1, ThermalScattering words...]`.

- ```rust
  pub fn from_f64s(v: &[f64]) -> Result<Self, Htr10DataError> { /* ... */ }
  ```
  The inverse of [`Self::to_f64s`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

#### Function `leapr_material`

**Attributes:**

- `MustUse { reason: None }`

The LEAPR deck a law is generated from under `cfg`, or `None` when it is
read from a tape. The UO2 and metal laws are always LEAPR's (as the
loader); graphite and SiC are under
[`super::data::ThermalLawSource::Leapr`] (the default), except the VII.0 graphite tape, which has no deck.

```rust
pub fn leapr_material(law: super::data::ThermalLaw, cfg: &super::data::Htr10DataConfig) -> Option<njoy_outram_park_fork::leapr::decks::SabMaterial> { /* ... */ }
```

#### Function `process_job`

Run one job. `tape` is the tape [`ProcessingJob::tape`] names, already
read (`None` for a LEAPR law).

# Errors

[`Htr10DataError::InvalidConfig`] when the tape is missing or fails to
process, with the reason.

```rust
pub fn process_job(job: &ProcessingJob, cfg: &super::data::Htr10DataConfig, tape: Option<&njoy_outram_park_fork::endf::tape::Tape>) -> Result<JobProduct, super::data::Htr10DataError> { /* ... */ }
```

#### Function `process_job_at`

[`process_job`] at `temperature` instead of `cfg.temperature` (gh:#1138):
a [`TemperedJob`]'s processing.

# Errors
As [`process_job`].

```rust
pub fn process_job_at(job: &ProcessingJob, cfg: &super::data::Htr10DataConfig, tape: Option<&njoy_outram_park_fork::endf::tape::Tape>, temperature: uom::si::f64::ThermodynamicTemperature) -> Result<JobProduct, super::data::Htr10DataError> { /* ... */ }
```

#### Function `assemble_slots`

Build every slot of `layout` from the jobs' products, in slot order.
`product(i)` supplies the product of `jobs[i]` (as
[`Htr10NuclideLayout::processing_jobs`] lists them) when it is needed,
once each: the laws first, then one tape's product at a time, so only one
processed tape is decoded at once. `read_tape` supplies each tape; each is
read once, used for every slot that reads it, and dropped.

# Errors

A product missing or of the wrong kind, a tape that fails to read, or a
nuclide that fails to assemble.

```rust
pub fn assemble_slots</* synthetic */ impl FnMut(usize) -> Result<JobProduct, Htr10DataError>: FnMut(usize) -> Result<JobProduct, super::data::Htr10DataError>, /* synthetic */ impl FnMut(&Tape) -> Result<EndfTape, Htr10DataError>: FnMut(&super::data::Tape) -> Result<njoy_outram_park_fork::endf::tape::Tape, super::data::Htr10DataError>>(cfg: &super::data::Htr10DataConfig, layout: &super::data::Htr10NuclideLayout, jobs: &[ProcessingJob], product: impl FnMut(usize) -> Result<JobProduct, super::data::Htr10DataError>, read_tape: impl FnMut(&super::data::Tape) -> Result<njoy_outram_park_fork::endf::tape::Tape, super::data::Htr10DataError>) -> Result<Vec<outram_mc_libs::material::nuclide::Nuclide>, super::data::Htr10DataError> { /* ... */ }
```

#### Function `assemble_entries`

**Build a nuclide array at several temperatures** (gh:#1138): nuclide `i`
is slot `entries[i].0` of `layout` at `entries[i].1` K, from the products
of `jobs` ([`Htr10NuclideLayout::tempered_jobs`] of the same entries).
As [`assemble_slots`] otherwise, which is this with every entry at
`cfg.temperature`: the laws first, then one tape's product at a time,
each tape read once per temperature.

# Errors

A product missing or of the wrong kind, a tape that fails to read, or a
nuclide that fails to assemble.

```rust
pub fn assemble_entries</* synthetic */ impl FnMut(usize) -> Result<JobProduct, Htr10DataError>: FnMut(usize) -> Result<JobProduct, super::data::Htr10DataError>, /* synthetic */ impl FnMut(&Tape) -> Result<EndfTape, Htr10DataError>: FnMut(&super::data::Tape) -> Result<njoy_outram_park_fork::endf::tape::Tape, super::data::Htr10DataError>>(layout: &super::data::Htr10NuclideLayout, entries: &[(usize, f64)], jobs: &[TemperedJob], product: impl FnMut(usize) -> Result<JobProduct, super::data::Htr10DataError>, read_tape: impl FnMut(&super::data::Tape) -> Result<njoy_outram_park_fork::endf::tape::Tape, super::data::Htr10DataError>) -> Result<Vec<outram_mc_libs::material::nuclide::Nuclide>, super::data::Htr10DataError> { /* ... */ }
```

#### Function `assemble_entries_free`

**[`assemble_entries`] with every nuclide FREE** (gh:#1635 item 3): the
same processing and order, but the laws come back as tables and
`table_of[k]` names the one nuclide `k` carries, to be associated per
material ([`FreeSlots::bind_per_material`]). The nuclide-level
[`assemble_entries`] is this followed by `bind_nuclide_level`.

# Errors

As [`assemble_entries`].

```rust
pub fn assemble_entries_free</* synthetic */ impl FnMut(usize) -> Result<JobProduct, Htr10DataError>: FnMut(usize) -> Result<JobProduct, super::data::Htr10DataError>, /* synthetic */ impl FnMut(&Tape) -> Result<EndfTape, Htr10DataError>: FnMut(&super::data::Tape) -> Result<njoy_outram_park_fork::endf::tape::Tape, super::data::Htr10DataError>>(layout: &super::data::Htr10NuclideLayout, entries: &[(usize, f64)], jobs: &[TemperedJob], product: impl FnMut(usize) -> Result<JobProduct, super::data::Htr10DataError>, read_tape: impl FnMut(&super::data::Tape) -> Result<njoy_outram_park_fork::endf::tape::Tape, super::data::Htr10DataError>) -> Result<super::data::FreeSlots, super::data::Htr10DataError> { /* ... */ }
```

### Constants and Statics

#### Constant `TOLERANCE`

RECONR and BROADR tolerance of every slot, as the loader (NJOY's 1e-3).

```rust
pub const TOLERANCE: f64 = 1.0e-3;
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `RodMetalNuclides`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: None, name: \"missing_docs\" },\nkind: Allow}])]")`

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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

**GitHub #589 (2026-10-05):** until then `over_indices` tabulated only on
this 4096-point log grid, which left the UO2 kernel 14x above the majorant
at 661 eV on ENDF/B-VIII.0. Every k-vs-height record before that date
used it. `over_indices` now adds every nuclide breakpoint
(`Majorant::from_materials`); the old construction is the ablation
`Majorant::over_indices_without_breakpoints`.

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

## Module `zones`

**HTR-10 temperature zones** (gh:#1138, epic #1137).

The recorded model runs every material and every nuclide at one
temperature (300.15 K). This module splits the core into the eight zones
the maintainer set on 2026-11-03, each with a temperature of its own:

| zone | material slots |
|---|---|
| [`Htr10Zone::FuelPebble`] | the UO₂ kernel, the four TRISO coatings, the matrix graphite and the shell ([`mat::KERNEL`] to [`mat::GRAPHITE`]) |
| [`Htr10Zone::ModeratorPebble`] | the graphite (dummy) balls, in the bed and the discharge tube ([`mat::MODERATOR_BALL`]; [`mat::HOMOG_DUMMY`] under its ablation) |
| [`Htr10Zone::Coolant`] | helium, everywhere it is ([`mat::HELIUM`]) |
| [`Htr10Zone::TopReflector`], [`Htr10Zone::SideReflector`], [`Htr10Zone::BottomReflector`] | the reflector composition slots, one copy per region ([`mat::regional_slot`]); each Fig. 4.10 box is placed by [`region_of_box`] |
| [`Htr10Zone::ControlRods`] | B₄C, steel sleeves, iron joints ([`mat::ROD_B4C`], [`mat::ROD_STEEL`], [`mat::ROD_IRON`]) |
| [`Htr10Zone::BoronatedBricks`] | the boronated carbon bricks ([`mat::BORONATED`]) |

A zone's temperature sets the temperature its nuclides are processed at
(RECONR, BROADR, HEATR, PURR) and its bound thermal laws are generated at
(LEAPR), the materials' `temperature`, the cells' `temperature`, and, for
the coolant, the helium density (an ideal gas at the model's pressure).
[`split_by_temperature`] turns materials at several temperatures into one
nuclide array that holds each distinct (nuclide, temperature) once.

Orchestration only: no physics is implemented here.

```rust
pub mod zones { /* ... */ }
```

### Types

#### Enum `Htr10Zone`

One of the eight temperature zones.

```rust
pub enum Htr10Zone {
    FuelPebble,
    ModeratorPebble,
    Coolant,
    TopReflector,
    SideReflector,
    BottomReflector,
    ControlRods,
    BoronatedBricks,
}
```

##### Variants

###### `FuelPebble`

The whole fuel pebble: kernel, TRISO coatings, matrix and shell.

###### `ModeratorPebble`

The graphite (dummy) balls.

###### `Coolant`

The helium coolant.

###### `TopReflector`

Reflector above the core cavity.

###### `SideReflector`

Reflector beside the bed.

###### `BottomReflector`

Reflector below the bed, the conus region included.

###### `ControlRods`

The control rods: B₄C, steel sleeves and iron joints.

###### `BoronatedBricks`

The boronated carbon bricks.

##### Implementations

###### Methods

- ```rust
  pub fn index(self: Self) -> usize { /* ... */ }
  ```
  Position in [`Self::ALL`].

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Name for a slider or a log line.

- ```rust
  pub fn key(self: Self) -> &'static str { /* ... */ }
  ```
  Short machine name (`fuel`, `moderator`, ...), for the CLI and URLs.

- ```rust
  pub fn from_key(k: &str) -> Option<Self> { /* ... */ }
  ```
  The zone whose [`Self::key`] is `k`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Self) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Self) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
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
#### Enum `ReflectorRegion`

Reflector region, from a Fig. 4.10 box's place relative to the core.

```rust
pub enum ReflectorRegion {
    Top,
    Side,
    Bottom,
}
```

##### Variants

###### `Top`

Wholly above the core cavity's top.

###### `Side`

Overlapping the height of the cavity and the bed.

###### `Bottom`

Wholly below the bed floor.

##### Implementations

###### Methods

- ```rust
  pub fn index(self: Self) -> usize { /* ... */ }
  ```
  0 top, 1 side, 2 bottom: the index [`mat::regional_slot`] takes.

- ```rust
  pub fn zone(self: Self) -> Htr10Zone { /* ... */ }
  ```
  The temperature zone of this region.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `ZoneTemperatures`

**A temperature for each of the eight zones.**

```rust
pub struct ZoneTemperatures {
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
  pub fn uniform(t: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Every zone at `t`.

- ```rust
  pub fn record() -> Self { /* ... */ }
  ```
  The recorded model's: every zone at 300.15 K.

- ```rust
  pub fn from_kelvin(k: [f64; 8]) -> Self { /* ... */ }
  ```
  From one temperature per zone in K, in [`Htr10Zone::ALL`] order: the

- ```rust
  pub fn to_kelvin(self: &Self) -> [f64; 8] { /* ... */ }
  ```
  One temperature per zone in K, in [`Htr10Zone::ALL`] order.

- ```rust
  pub fn get(self: &Self, z: Htr10Zone) -> ThermodynamicTemperature { /* ... */ }
  ```
  Zone `z`'s temperature.

- ```rust
  pub fn kelvin(self: &Self, z: Htr10Zone) -> f64 { /* ... */ }
  ```
  Zone `z`'s temperature, K.

- ```rust
  pub fn set(self: &mut Self, z: Htr10Zone, t: ThermodynamicTemperature) { /* ... */ }
  ```
  Set zone `z` to `t`.

- ```rust
  pub fn with(self: Self, z: Htr10Zone, t: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  This, with zone `z` at `t`.

- ```rust
  pub fn is_uniform(self: &Self) -> bool { /* ... */ }
  ```
  Whether every zone is at one temperature.

- ```rust
  pub fn above_triso_limit(self: &Self) -> Vec<Htr10Zone> { /* ... */ }
  ```
  The zones above the TRISO limit ([`triso_limit`]).

- ```rust
  pub fn check(self: &Self) -> Result<(), String> { /* ... */ }
  ```
  Refuse a temperature outside [`min_temperature`]..=[`max_temperature`]

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `region_of_box`

**Attributes:**

- `MustUse { reason: None }`

**Region of a Fig. 4.10 box**, by height alone: wholly above the core
band is the top reflector, wholly below it the bottom reflector, and any
box overlapping it the side reflector. So the corners go with the top and
bottom: the top corners sit by the cold-helium plenum and the bottom ones
by the hot-gas region, which is where their temperatures come from.

```rust
pub fn region_of_box(b: &super::reflector_geometry::Fig410Box) -> ReflectorRegion { /* ... */ }
```

#### Function `zone_of_slot`

**Attributes:**

- `MustUse { reason: None }`

**The temperature zone of a material slot** of a zoned design (any slot
below [`mat::ZONED_COUNT`]).

The base composition slots ([`mat::REFLECTOR`] and the Table 4-3 slots)
are not placed by a zoned design, which uses their regional copies; they
answer [`Htr10Zone::SideReflector`], as does the unused
[`mat::BORED_GRAPHITE`] (the side reflector's bored zones).

# Panics
For a slot at or above [`mat::ZONED_COUNT`].

```rust
pub fn zone_of_slot(slot: usize) -> Htr10Zone { /* ... */ }
```

#### Function `min_temperature`

**Attributes:**

- `MustUse { reason: None }`

The lowest temperature a zone may be set to: 20 °C, the B1 benchmark
core's cold critical state.

```rust
pub fn min_temperature() -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

#### Function `max_temperature`

**Attributes:**

- `MustUse { reason: None }`

The highest temperature a zone may be set to: 1800 °C (maintainer,
2026-11-03: a little above the TRISO limit).

```rust
pub fn max_temperature() -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

#### Function `triso_limit`

**Attributes:**

- `MustUse { reason: None }`

Above this a zone is warned about: 1600 °C, the TRISO fuel limit.

```rust
pub fn triso_limit() -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

#### Function `zoned_material_set`

**Attributes:**

- `MustUse { reason: None }`

**The material set of a zoned design**: [`htr10_material_set`]'s slots,
then [`mat::MODERATOR_BALL`] and the regional copies (slot order up to
[`mat::ZONED_COUNT`]), each at its zone's temperature. The helium density
is the ideal gas at the coolant's temperature and the model's pressure.

Every component still indexes `layout`'s slots (one nuclide per slot, at
no particular temperature): [`split_by_temperature`] then gives each
(slot, temperature) pair a nuclide of its own.

```rust
pub fn zoned_material_set(layout: &super::data::Htr10NuclideLayout, cfg: super::materials::Htr10MaterialConfig, temps: &ZoneTemperatures) -> Vec<outram_mc_libs::material::material::Material> { /* ... */ }
```

#### Function `split_by_temperature`

**One nuclide per (slot, temperature)**: for each of the `n_slots` base
slots, in slot order, one entry for every distinct temperature of the
materials that use it (ascending), and the materials' components
re-pointed at their own temperature's entry. A slot no material uses
keeps one entry, at the lowest material temperature.

Returns the entries as `(base slot, temperature K)`. With every material
at one temperature the entries are `(k, T)` for `k = 0..n_slots` and no
component moves: the single-temperature nuclide array, unchanged.

# Panics
If a component indexes a slot at or above `n_slots`, or `mats` is empty.

```rust
pub fn split_by_temperature(mats: &mut [outram_mc_libs::material::material::Material], n_slots: usize) -> Vec<(usize, f64)> { /* ... */ }
```

#### Function `set_cell_temperatures`

**Give every material cell its material's temperature** (gh:#313: the
cells were built at a hardcoded 293.6 K, which transport never reads; the
material's temperature is what it reads). Fill cells are left alone.

```rust
pub fn set_cell_temperatures(geometry: &mut outram_mc_libs::geometry::geometry::Geometry, mats: &[outram_mc_libs::material::material::Material]) { /* ... */ }
```

#### Function `zone_palette`

**Attributes:**

- `MustUse { reason: None }`

**Palette by temperature zone** for `render_material_slice` (gh:#1138's
drawing): every slot of a zoned design coloured by its zone, so a slice
shows which cells each slider moves. Helium is pale so the pebbles show.

```rust
pub fn zone_palette() -> Vec<(outram_mc_libs::geometry::plot::Rgb, &'static str)> { /* ... */ }
```

### Constants and Statics

#### Constant `CORE_BAND_ZT_CM`

The core's height band in Fig. 4.10's `z_T` (cm, measured down from the
model's top): the core cavity's top (130 cm) to the bed floor at the
conus top (388.764 cm). The same band bounds the KLAK slots
([`super::reflector_geometry::KLAK_SLOT_ZT_CM`]).

```rust
pub const CORE_BAND_ZT_CM: (f64, f64) = super::reflector_geometry::KLAK_SLOT_ZT_CM;
```

## Module `parametric`

**`ParametricStudy`: every setting of an HTR-10 whole-core run in one
struct** (gh:#1139, epic #1137).

The command line (`examples/htr10_parametric.rs`) and the browser demo
(`dhoby-ghaut`'s `monte_carlo_web`, the HTR-10 whole-core tab) both build
their model through this struct, so a setting means the same in both:

| setting | field |
|---|---|
| the bed: Şeker & Çolak's lattice at N layers, or an explicit (DEM) ball list | [`ParametricStudy::bed`] |
| the ten control rods, each `0` withdrawn to `1` fully inserted | [`ParametricStudy::rod_insertions`] |
| a temperature per zone (eight zones, [`super::zones`]) | [`ParametricStudy::temperatures`] |
| particles, inactive and active generations, seed | [`ParametricStudy::statistics`] |
| the nuclear-data choices | [`ParametricStudy::data`] |

**What a run is.** [`ParametricStudy::plan`] gives the materials (each at
its zone's temperature), the nuclide array (each distinct (nuclide,
temperature) once, [`super::zones::split_by_temperature`]) and the
processing jobs: every tape processed exactly at each temperature it is
needed at (RECONR, BROADR, HEATR, PURR: `Nuclide::process_evaluation`)
and every bound law generated by LEAPR at each temperature
([`ThermalLawSource::Leapr`], the study's default). No interpolation
between temperatures. [`ParametricStudy::core`] builds the geometry with
the zone slots placed; [`ParametricStudy::run_keff`] runs it natively.

**With every zone at one temperature it is the single-temperature model**:
the same nuclide array and the same jobs as
[`Htr10NuclideLayout::processing_jobs`], materials that differ only by the
zone copies' indices (pinned by `tests` below). The thermal laws differ
from the recorded runs' unless `data.thermal_laws` is
[`ThermalLawSource::Tapes`]: the records read graphite at the tape's
296 K for a 300.15 K run.

Orchestration only: no physics is implemented here.

```rust
pub mod parametric { /* ... */ }
```

### Types

#### Enum `StudyBed`

The bed a study builds.

```rust
pub enum StudyBed {
    Seker {
        layers: usize,
    },
    Explicit {
        centres: Vec<[uom::si::f64::Length; 3]>,
        is_fuel: Vec<bool>,
    },
}
```

##### Variants

###### `Seker`

Şeker & Çolak (2003)'s ordered bed at `layers` layers (10..=20).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `layers` | `usize` | Şeker layers N. |

###### `Explicit`

An explicit ball list in the DEM frame (a LIGGGHTS-port pour).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `centres` | `Vec<[uom::si::f64::Length; 3]>` | Ball centres. |
| `is_fuel` | `Vec<bool>` | Whether each ball is a fuel pebble. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `ParametricStudy`

**Every setting of an HTR-10 whole-core run.**

```rust
pub struct ParametricStudy {
    pub bed: StudyBed,
    pub rod_insertions: [f64; 10],
    pub temperatures: super::zones::ZoneTemperatures,
    pub statistics: super::keff_vs_height::SweepStatistics,
    pub data: super::data::Htr10DataConfig,
    pub materials: super::materials::Htr10MaterialConfig,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bed` | `StudyBed` | The bed. |
| `rod_insertions` | `[f64; 10]` | Each control rod's insertion, `0` withdrawn (the benchmark) to `1`<br>fully inserted. |
| `temperatures` | `super::zones::ZoneTemperatures` | The eight zones' temperatures. |
| `statistics` | `super::keff_vs_height::SweepStatistics` | Particles, generations and seed. |
| `data` | `super::data::Htr10DataConfig` | The nuclear-data choices. Its `temperature` is not read: each zone's<br>is. |
| `materials` | `super::materials::Htr10MaterialConfig` | The material choices. Its `temperature_k` is not read either. |

##### Implementations

###### Methods

- ```rust
  pub fn from_args(words: &[String]) -> Result<(Self, Vec<String>), String> { /* ... */ }
  ```
  **Settings from command-line words**, on top of [`Self::default`]:

- ```rust
  pub fn check(self: &Self) -> Result<(), String> { /* ... */ }
  ```
  Refuse a setting the model cannot build: a temperature out of

- ```rust
  pub fn design(self: &Self) -> Htr10CoreDesign { /* ... */ }
  ```
  The core design: the rods, with the temperature-zone slots placed.

- ```rust
  pub fn plan(self: &Self) -> Result<StudyPlan, Htr10DataError> { /* ... */ }
  ```
  **The plan**: materials at their zones' temperatures, the nuclide

- ```rust
  pub fn plan_with_slot_temperatures(self: &Self, slots: &[(usize, ThermodynamicTemperature)]) -> Result<StudyPlan, Htr10DataError> { /* ... */ }
  ```
  **The plan with some material slots off their zone's temperature**

- ```rust
  pub fn process_job(self: &Self, plan: &StudyPlan, i: usize, tape: Option<&EndfTape>) -> Result<JobProduct, Htr10DataError> { /* ... */ }
  ```
  Run job `i` of `plan`. `tape` is the tape [`Self::job_tape`] names,

- ```rust
  pub fn job_tape(self: &Self, plan: &StudyPlan, i: usize) -> Result<Option<Tape>, Htr10DataError> { /* ... */ }
  ```
  The tape job `i` reads, or `None` for a LEAPR law.

- ```rust
  pub fn assemble_nuclides</* synthetic */ impl FnMut(usize) -> Result<JobProduct, Htr10DataError>: FnMut(usize) -> Result<JobProduct, Htr10DataError>, /* synthetic */ impl FnMut(&Tape) -> Result<EndfTape, Htr10DataError>: FnMut(&Tape) -> Result<EndfTape, Htr10DataError>>(self: &Self, plan: &StudyPlan, product: impl FnMut(usize) -> Result<JobProduct, Htr10DataError>, read_tape: impl FnMut(&Tape) -> Result<EndfTape, Htr10DataError>) -> Result<Vec<Nuclide>, Htr10DataError> { /* ... */ }
  ```
  Build the nuclide array from the jobs' products (`product(i)` for job

- ```rust
  pub fn core(self: &Self, plan: &StudyPlan) -> Result<AssembledCore, String> { /* ... */ }
  ```
  **The core**: the bed, the rods and the zone slots, every material

- ```rust
  pub fn majorant(plan: &StudyPlan, nuclides: &[Nuclide]) -> Majorant { /* ... */ }
  ```
  **The bed majorant**: as [`super::keff_vs_height::bed_majorant`], over

- ```rust
  pub fn process_all</* synthetic */ impl Fn(&str) + Sync: Fn(&str) + Sync>(self: &Self, plan: &StudyPlan, threads: usize, progress: impl Fn(&str) + Sync) -> Result<Vec<Nuclide>, String> { /* ... */ }
  ```
  **Process every job natively and assemble the nuclide array**: the

- ```rust
  pub fn transport(self: &Self, plan: &StudyPlan, nuclides: &[Nuclide], threads: usize) -> Result<KeffResult, String> { /* ... */ }
  ```
  **Transport the core on `nuclides`** (the array of `plan`) with

- ```rust
  pub fn transport_with_tally(self: &Self, plan: &StudyPlan, nuclides: &[Nuclide], threads: usize, tally: Option<&mut Tally>) -> Result<KeffResult, String> { /* ... */ }
  ```
  **As [`Self::transport`], scoring `tally` on the active generations**

- ```rust
  pub fn run_keff</* synthetic */ impl Fn(&str) + Sync: Fn(&str) + Sync>(self: &Self, threads: usize, progress: impl Fn(&str) + Sync) -> Result<KeffResult, String> { /* ... */ }
  ```
  **Run it natively**: [`Self::plan`], [`Self::process_all`],

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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
    The recorded model at N = 12, rods withdrawn, every zone at 300.15 K,

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `StudyPlan`

What a study needs before any data is processed: the materials, the
nuclide array's entries and the jobs that make them.

```rust
pub struct StudyPlan {
    pub layout: super::data::Htr10NuclideLayout,
    pub materials: Vec<outram_mc_libs::material::material::Material>,
    pub entries: Vec<(usize, f64)>,
    pub jobs: Vec<super::data_jobs::TemperedJob>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `layout` | `super::data::Htr10NuclideLayout` | The nuclide slots (one per nuclide, at no temperature). |
| `materials` | `Vec<outram_mc_libs::material::material::Material>` | The materials, every slot of a zoned design (`mat::ZONED_COUNT`), each<br>at its zone's temperature, its components indexing [`Self::entries`]. |
| `entries` | `Vec<(usize, f64)>` | The nuclide array: `(slot of layout, temperature K)` per nuclide. |
| `jobs` | `Vec<super::data_jobs::TemperedJob>` | The processing jobs, as [`Htr10NuclideLayout::tempered_jobs`] lists<br>them for [`Self::entries`]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `GraphiteWorthCase`

One case of the graphite-zone worth measurement (gh:#1174).

```rust
pub struct GraphiteWorthCase {
    pub name: &'static str,
    pub temperatures: super::zones::ZoneTemperatures,
    pub slots: Vec<(usize, uom::si::f64::ThermodynamicTemperature)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `&'static str` | Short name (`BASE`, `FUEL-GRAPHITE`, ...). |
| `temperatures` | `super::zones::ZoneTemperatures` | The zone temperatures. |
| `slots` | `Vec<(usize, uom::si::f64::ThermodynamicTemperature)>` | Slots moved off their zone's temperature<br>([`ParametricStudy::plan_with_slot_temperatures`]). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `graphite_worth_cases`

**Attributes:**

- `MustUse { reason: None }`

**The cases that split the graphite temperature worth by zone**
(gh:#1174): every zone at `base`, then one part at `base + delta_k` at a
time:

- `FUEL-GRAPHITE`: the fuel pebbles' graphite ([`mat::GRAPHITE`]: the
  fuel-zone matrix and the shell), kernels and coatings kept at `base`;
- `DUMMIES`: the moderator (dummy) pebbles;
- `TOP`, `SIDE`, `BOTTOM`: the three reflector zones;
- `KERNEL`: the rest of the fuel-pebble zone (kernel and the four
  coatings), the fuel coefficient's own part, as a check against
  TECDOC-1694's fuel coefficient.

The reactivity worth of each part is `1/k_base - 1/k_case`.

```rust
pub fn graphite_worth_cases(base: uom::si::f64::ThermodynamicTemperature, delta_k: f64) -> Vec<GraphiteWorthCase> { /* ... */ }
```

### Constants and Statics

#### Constant `RECORD_RINGS`

Rings of the bed tiling the recorded runs use.

```rust
pub const RECORD_RINGS: usize = 14;
```

#### Constant `DEFAULT_LAYERS`

Şeker layers of the default bed: N = 12, 16 681 core balls.

```rust
pub const DEFAULT_LAYERS: usize = 12;
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

## Module `httr`

# HTTR start-up core: an explicit-TRISO, delta-tracked Monte Carlo model

Japan's High Temperature Engineering Test Reactor (HTTR), a prismatic-block
HTGR, in its fully-loaded 30-fuel-column start-up core at 300 K: the
IRPhEP benchmark HTTR-GCR-RESR-001 (GitHub epic #888; data issues
#883-#887). It is HTR-10's counterpart (`crate::htr10_rmc`): the same
coolant, moderator and particle, in blocks instead of pebbles.

**Status: first build, 2026-10-16. AI-assisted, not yet reviewed by a
human.** The geometry is drawn in
`verification_and_validation/httr_startup_core/` (`examples/httr_geometry_images.rs`)
and its `k_eff` record is in the README there. Nothing in the model was
adjusted towards a reference value.

# Source

**R1:** J. D. Bess, N. Fujimoto, B. H. Dolphin, L. Snoj, A. Zukeran,
*Evaluation of the Start-Up Core Physics Tests at Japan's High Temperature
Engineering Test Reactor (Fully-Loaded Core)*, HTTR-GCR-RESR-001,
INL/EXT-08-14767 Rev. 2, Idaho National Laboratory, March 2010
(doi:10.2172/974753), in the IRPhEP Handbook, NEA/NSC/DOC(2006)1. A US
DOE report, openly downloadable from the INL digital library. Every
dimension is in [`spec`]; every composition in [`materials`]. The
geometry follows R1 §3.1 and its sample MCNP5 deck (App. A.1.8).

# What is modelled, and where it departs from R1

Modelled as R1's benchmark model specifies: every TRISO particle (an
ordered lattice of 333 per 0.1 cm layer, 181 818 per rod, 867 million in
the core), every fuel rod (4770) with its helium hole, gap, IG-110
sleeve and coolant annulus, every BP pin (2.0 and 2.5 wt.% B), 33- and
31-hole fuel blocks, reflector blocks with coolant channels, the 16
control columns with explicit 10-section rods at the measured critical
heights, the empty RSS and instrumentation holes, 12 solid reflector
columns, the 2 mm helium gaps, each column's own rotation, and the PGX
permanent reflector out to r = 212.5 cm. The evaluators' own
simplifications are kept (they are the benchmark): no dowels or handling
holes (a 0.5 % graphite density reduction instead), no instrumentation
(its removal is a bias already in the benchmark `k`), all holes 1060 mm
deep, the bottom-most reflector block like the others.

Departures, each physical or a choice between R1's own conflicting
statements, and each stated where it is made:

- **R1's text and its deck disagree in three places; the deck is
  followed**, because it produced every reference `k` in R1: reflector
  coolant channels are 23 mm, not the text's 21 mm
  ([`spec::REFLECTOR_CHANNEL_R`]); zones 3-4 have a 33-hole bottom
  reflector block, not 31 (`geometry`); the compact's TRISO lattice is
  placed so that each 0.1 cm layer is whole, where the deck's lattice
  origin cuts the end layers in half (both hold the specified 546 layers'
  worth of particles).
- **Impurity boron is natural boron** (B-11 added beside R1's B-10),
  **carbon is natural carbon** (C-12/C-13), and **each carbon carries the
  thermal law of its material** (SiC, B4C, graphite): see
  [`materials`].
- **Graphite S(α,β) is ENDF/B-VIII.0's 30 % porous reactor graphite**,
  chosen by density as HTR-10's was; R1 used crystalline-type graphite
  (ENDF/B-VI.8 and VII.0), which is the ablation.
- **ENDF/B-VIII.0 throughout.** R1's reference results are on ENDF/B-V.2
  to VII.0, JEFF-3.1 and JENDL-3.3; none used VIII.0.

# Delta tracking

The compact annulus (the TRISO lattice) is delta-tracked against a
majorant over the compact's materials ([`compact_majorant`]); the rest is
surface-tracked. `CompactTracking::Surface` is the ablation.

# One open ambiguity in R1's deck (stated, not resolved, not priced)

MCNP's `lat=2` second index runs at +60 degrees from the first, which is
how [`spec`] reads the deck. R1 §3.7.2 places the instrumentation tally
cylinders at (114.6005, 72.4), (5.4, -135.447) and (-120, 63.04693) cm:
the column centres this reading gives, but each offset at 60, 180 or 300
degrees, where the deck's instrumentation holes are at 0, 120 and 240.
Neither reading of the deck's lattice convention reproduces both, so the
tally positions and the deck disagree with each other. It moves the
empty holes of three instrumentation columns, not the fuel, and matters
for the axial fission-rate tally, not for `k_eff`. Not priced.

```rust
pub mod httr { /* ... */ }
```

### Modules

## Module `geometry`

# The HTTR start-up core as CSG

[`assemble_httr`] builds the fully-loaded 30-column core of R1 (see
[`super::spec`]) from the inside out:

| level | what | how |
|---|---|---|
| TRISO | kernel, four coatings, overcoat; matrix beyond | spheres, one universe per enrichment |
| compact | 0.5-1.3 cm annulus, 54.6 cm | a `RectLattice` 27 x 27 x 546 (+ 1 matrix layer above and below), **delta-tracked** |
| fuel rod | helium hole, compacts, gap, IG-110 sleeve, coolant annulus | cylinders and planes |
| block | 33- or 31-hole pin lattice, BP pins, graphite | a 4-ring `HexLattice` (flats perpendicular to x), one per block kind and column rotation |
| column | 9 blocks of 58 cm, or a control / instrumentation / reflector column | hexagonal prism (apothem 18 cm) in a 18.1 cm tile; helium gap |
| core | 61 columns | a 5-ring `HexLattice` (flats perpendicular to y), pitch 36.2 cm, PGX outside |
| model | PGX cylinder r 212.5 cm, `|z|` < 261 cm | vacuum beyond |

**Rotation.** The CSG supports translations only, so each column
rotation of R1's deck (`*trcl`, R1 Fig. 3.16) is built as its own
universe: the pin lattice's tiles are filled by looking up each tile's
position rotated back into the unrotated block, and a control column's
holes are placed at their rotated angles. The TRISO lattice inside each
compact is not rotated: it stands in for a random distribution (R1
§3.1.1), and rotating its square pattern by 60° would change nothing
physical.

**Delta tracking.** Only the compact annulus is delta-tracked, against
majorant 0 of the caller's table: the region that holds 181 818 particles
per rod. Everything else is surface-tracked, so the majorant covers
only the compact's materials (`materials::mat::COMPACT`), as HTR-10's
covers only its bed.

```rust
pub mod geometry { /* ... */ }
```

### Types

#### Struct `AssembledHttr`

What [`assemble_httr`] returns.

```rust
pub struct AssembledHttr {
    pub geometry: outram_mc_libs::geometry::geometry::Geometry,
    pub rods: super::spec::RodPositions,
    pub particles_per_rod: usize,
    pub fuel_rods: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `outram_mc_libs::geometry::geometry::Geometry` | The model. |
| `rods` | `super::spec::RodPositions` | The rod positions it was built with. |
| `particles_per_rod` | `usize` | TRISO particles per fuel rod as built (counted from the lattice). |
| `fuel_rods` | `usize` | Fuel rods in the core as built. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Enum `CompactTracking`

The TRISO-region tracking method.

```rust
pub enum CompactTracking {
    Delta,
    Surface,
}
```

##### Variants

###### `Delta`

Delta tracking inside the compact annulus (default).

###### `Surface`

ABLATION: surface tracking everywhere.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `assemble_httr`

**Attributes:**

- `MustUse { reason: None }`

Build the HTTR fully-loaded core with its rods at `rods`.

# Panics
If the built TRISO lattice does not hold [`spec::TRISO_PER_LAYER`]
particles per layer, or the core map is not 61 columns (both checked
against the spec, as the HTR-10 model checks its own counts, gh:#316).

```rust
pub fn assemble_httr(rods: super::spec::RodPositions, tracking: CompactTracking) -> AssembledHttr { /* ... */ }
```

### Constants and Statics

#### Constant `COMPACT_MAJORANT`

Index of the compact's majorant in the transport driver's table.

```rust
pub const COMPACT_MAJORANT: usize = 0;
```

#### Constant `ZONES`

Every fuel zone, for iteration in tests and reports.

```rust
pub const ZONES: [super::spec::FuelZone; 4] = _;
```

#### Constant `BANKS`

Every rod bank.

```rust
pub const BANKS: [super::spec::RodBank; 4] = _;
```

## Module `materials`

# HTTR nuclides and materials

The 48 nuclide slots ([`nuc`]) and 27 materials ([`mat`]) of the HTTR
start-up core, with every atom density from R1 §3.1.3 (Tables 3.1-3.9)
and the isotopic splits of R1's sample deck (App. A.1.8, material cards
m1-m27). See [`super::spec`] for what R1 is.

**Three departures from R1's specification, each physical and stated:**

1. **Boron impurities carry B-11 as well as B-10.** R1 specifies each
   impurity as an equivalent *natural-boron* content but models only its
   B-10 (R1 §2.1, §3.1.1). Natural boron is 19.9 at.% B-10 and 80.1 at.%
   B-11 ([`B11_PER_B10`]), so B-11 is added beside every impurity's B-10,
   as the HTR-10 model does (gh:#311). Predicted worth: nil at this
   resolution (B-11 absorbs ~5 mb; the impurity is ~1e-8 atoms/(b cm)).
   [`HttrMaterialConfig::without_impurity_b11`] is the ablation back to
   R1's specification.
2. **Carbon is natural carbon**, C-12 and C-13 at 98.93 / 1.07 at.%
   (IUPAC), where R1's deck uses elemental C-nat (ENDF/B-VIII.0 has no
   elemental carbon). The same as HTR-10 (gh:#425).
3. **Thermal binding follows the material, not the deck's one law.** The
   deck puts graphite S(α,β) on every carbon, the SiC's included. Here
   the SiC carries ENDF/B-VIII.0's C-in-SiC and Si-in-SiC laws, and the
   control rods' B4C/C absorber splits its carbon: the stoichiometric
   B4C carbon (a quarter of the boron atoms) is free, the rest of the
   compact's carbon is graphite (R1 Table 1.15: sintered B4C/C). Alloy
   800H's carbon and silicon are free; its Fe-56 and Al-27 carry the
   iron and aluminium metal laws, as the deck's `mt24 Fe.00t Al.00t`.

O-18 is not modelled separately: R1's oxygen total (Table 3.1) is split
by the deck into O-16 and O-17 only, so O-18's 0.2 % rides on O-16.

```rust
pub mod materials { /* ... */ }
```

### Modules

## Module `nuc`

Nuclide slot indices. Each slot is one nuclide with one thermal
treatment, so carbon and silicon appear more than once.

```rust
pub mod nuc { /* ... */ }
```

### Constants and Statics

#### Constant `U234`

U-234 (U-in-UO2 law).

```rust
pub const U234: usize = 0;
```

#### Constant `U235`

U-235 (U-in-UO2 law).

```rust
pub const U235: usize = 1;
```

#### Constant `U238`

U-238 (U-in-UO2 law).

```rust
pub const U238: usize = 2;
```

#### Constant `O16`

O-16 (O-in-UO2 law).

```rust
pub const O16: usize = 3;
```

#### Constant `O17`

O-17 (free; the O-in-UO2 law is O-16's, as OpenMC binds it).

```rust
pub const O17: usize = 4;
```

#### Constant `C12_GRAPHITE`

C-12 bound in graphite.

```rust
pub const C12_GRAPHITE: usize = 5;
```

#### Constant `C13_GRAPHITE`

C-13 bound in graphite.

```rust
pub const C13_GRAPHITE: usize = 6;
```

#### Constant `C12_SIC`

C-12 bound in SiC.

```rust
pub const C12_SIC: usize = 7;
```

#### Constant `C13_SIC`

C-13 bound in SiC.

```rust
pub const C13_SIC: usize = 8;
```

#### Constant `C12_FREE`

C-12 free (B4C carbon, steel carbon).

```rust
pub const C12_FREE: usize = 9;
```

#### Constant `C13_FREE`

C-13 free.

```rust
pub const C13_FREE: usize = 10;
```

#### Constant `B10`

B-10.

```rust
pub const B10: usize = 11;
```

#### Constant `B11`

B-11.

```rust
pub const B11: usize = 12;
```

#### Constant `SI28_SIC`

Si-28 bound in SiC.

```rust
pub const SI28_SIC: usize = 13;
```

#### Constant `SI29_SIC`

Si-29 bound in SiC.

```rust
pub const SI29_SIC: usize = 14;
```

#### Constant `SI30_SIC`

Si-30 bound in SiC.

```rust
pub const SI30_SIC: usize = 15;
```

#### Constant `SI28`

Si-28 free (Alloy 800H).

```rust
pub const SI28: usize = 16;
```

#### Constant `SI29`

Si-29 free.

```rust
pub const SI29: usize = 17;
```

#### Constant `SI30`

Si-30 free.

```rust
pub const SI30: usize = 18;
```

#### Constant `HE3`

He-3.

```rust
pub const HE3: usize = 19;
```

#### Constant `HE4`

He-4.

```rust
pub const HE4: usize = 20;
```

#### Constant `AL27`

Al-27 (aluminium metal law).

```rust
pub const AL27: usize = 21;
```

#### Constant `P31`

P-31.

```rust
pub const P31: usize = 22;
```

#### Constant `S`

S-32, S-33, S-34, S-36.

```rust
pub const S: [usize; 4] = _;
```

#### Constant `TI`

Ti-46 to Ti-50.

```rust
pub const TI: [usize; 5] = _;
```

#### Constant `CR`

Cr-50, 52, 53, 54.

```rust
pub const CR: [usize; 4] = _;
```

#### Constant `MN55`

Mn-55.

```rust
pub const MN55: usize = 36;
```

#### Constant `FE`

Fe-54, Fe-56 (iron metal law), Fe-57, Fe-58.

```rust
pub const FE: [usize; 4] = _;
```

#### Constant `NI`

Ni-58, 60, 61, 62, 64.

```rust
pub const NI: [usize; 5] = _;
```

#### Constant `CU`

Cu-63, Cu-65.

```rust
pub const CU: [usize; 2] = _;
```

#### Constant `COUNT`

Number of slots.

```rust
pub const COUNT: usize = 48;
```

## Module `mat`

Material indices, in the order of R1's deck (m1-m27, less one).

```rust
pub mod mat { /* ... */ }
```

### Constants and Statics

#### Constant `KERNEL0`

The twelve UO2 kernels, `KERNEL0 + i` for enrichment
`spec::ENRICHMENTS[i]` (deck m1-m12).

```rust
pub const KERNEL0: usize = 0;
```

#### Constant `BUFFER`

Buffer (m13).

```rust
pub const BUFFER: usize = 12;
```

#### Constant `IPYC`

Inner pyrolytic carbon (m14).

```rust
pub const IPYC: usize = 13;
```

#### Constant `SIC`

Silicon carbide (m15).

```rust
pub const SIC: usize = 14;
```

#### Constant `OPYC`

Outer pyrolytic carbon (m16).

```rust
pub const OPYC: usize = 15;
```

#### Constant `OVERCOAT`

Resinated-graphite overcoat (m17).

```rust
pub const OVERCOAT: usize = 16;
```

#### Constant `MATRIX`

Fuel-compact matrix (m18).

```rust
pub const MATRIX: usize = 17;
```

#### Constant `SLEEVE`

IG-110 fuel sleeve and end caps (m19).

```rust
pub const SLEEVE: usize = 18;
```

#### Constant `BP_2_0`

Burnable poison, 2.0 wt.% natural boron (m20).

```rust
pub const BP_2_0: usize = 19;
```

#### Constant `BP_2_5`

Burnable poison, 2.5 wt.% (m21).

```rust
pub const BP_2_5: usize = 20;
```

#### Constant `DISKS`

Graphite disks between the BP stacks (m22).

```rust
pub const DISKS: usize = 21;
```

#### Constant `ABSORBER`

B4C/C control-rod absorber (m23).

```rust
pub const ABSORBER: usize = 22;
```

#### Constant `ALLOY_800H`

Alloy 800H (m24).

```rust
pub const ALLOY_800H: usize = 23;
```

#### Constant `BLOCK`

IG-110 block graphite, every column (m25).

```rust
pub const BLOCK: usize = 24;
```

#### Constant `PGX`

PGX permanent reflector (m26).

```rust
pub const PGX: usize = 25;
```

#### Constant `HELIUM`

Helium (m27).

```rust
pub const HELIUM: usize = 26;
```

#### Constant `COUNT`

Number of materials.

```rust
pub const COUNT: usize = 27;
```

#### Constant `COMPACT`

The materials inside the fuel compact's delta-tracked region: the
kernels, the coatings, the overcoat and the matrix.

```rust
pub const COMPACT: [usize; 18] = _;
```

### Types

#### Enum `HttrThermal`

How thermal scattering is treated (the free-gas arm is the harness
check, as for HTR-10).

```rust
pub enum HttrThermal {
    Bound,
    FreeGas,
}
```

##### Variants

###### `Bound`

Every bound law (default).

###### `FreeGas`

ABLATION: every nuclide a free gas.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `HttrMaterialConfig`

Options for the material set.

```rust
pub struct HttrMaterialConfig {
    pub temperature_k: f64,
    pub impurity_b11: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature_k` | `f64` | Material temperature \[K\] (R1 §3.1.4: 300 K). |
| `impurity_b11` | `bool` | Add B-11 beside each impurity's B-10 (default `true`, see the module<br>docs). |

##### Implementations

###### Methods

- ```rust
  pub fn benchmark(temperature_k: f64) -> Self { /* ... */ }
  ```
  The default: 300 K, natural boron in every impurity.

- ```rust
  pub fn without_impurity_b11(self: Self) -> Self { /* ... */ }
  ```
  ABLATION: impurity boron as B-10 only, R1's own specification.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `httr_nuclide_slots`

**Attributes:**

- `MustUse { reason: None }`

The 48 nuclide slots in [`nuc`] order, every one ENDF/B-VIII.0, with
the thermal law each carries, and the modelling notes a run records.

The graphite law defaults to `GraphiteLaw::Reactor30P` for the reason
HTR-10 chose it (maintainer, 2026-09-27): IG-110 is 1.75-1.77 g/cm³ and
PGX 1.73 (R1 Tables 1.13, 1.19, 1.27) against crystal graphite's ~2.25,
i.e. ~22 % porous, and 30 % is the nearer of ENDF/B-VIII.0's two
porous laws. R1's own results used ENDF/B-VI.8 to VII.0 graphite, which
is crystalline: `GraphiteLaw::Crystalline` is the like-for-like ablation.

```rust
pub fn httr_nuclide_slots(graphite: crate::htr10_rmc::materials::GraphiteLaw, thermal: HttrThermal) -> (Vec<crate::htr10_rmc::data::NuclideSlot>, Vec<String>) { /* ... */ }
```

#### Function `httr_material_set`

**Attributes:**

- `MustUse { reason: None }`

The 27 materials of R1 §3.1.3, in [`mat`] order.

```rust
pub fn httr_material_set(cfg: HttrMaterialConfig) -> Vec<outram_mc_libs::material::material::Material> { /* ... */ }
```

### Constants and Statics

#### Constant `B11_PER_B10`

B-11 atoms per B-10 atom in natural boron, 0.801 / 0.199 (the 19.9 at.%
B-10 R1 itself uses, R1 §3.1.1).

```rust
pub const B11_PER_B10: f64 = _;
```

#### Constant `KERNEL_U`

UO2 kernel atom densities \[atoms/(b cm)\] by enrichment, R1 Table 3.1:
U-234, U-235, U-238. Oxygen (4.6404e-2) and B-10 (1.7299e-7) are the
same in all twelve.

```rust
pub const KERNEL_U: [[f64; 3]; 12] = _;
```

#### Constant `KERNEL_O`

Kernel O-16 and O-17 \[atoms/(b cm)\]: R1 Table 3.1's 4.6404e-2 split
as the deck's m1-m12 split it.

```rust
pub const KERNEL_O: [f64; 2] = _;
```

#### Constant `KERNEL_B10`

Kernel B-10 \[atoms/(b cm)\] (R1 Table 3.1).

```rust
pub const KERNEL_B10: f64 = 1.7299e-07;
```

#### Constant `ALLOY_800H`

Alloy 800H \[atoms/(b cm)\], R1 Table 3.8 element totals split into
isotopes as the deck's m24 card splits them. `(slot, density)`.

```rust
pub const ALLOY_800H: [(usize, f64); 31] = _;
```

## Module `spec`

# The HTTR start-up core benchmark specification, as numbers

Every dimension here is the IRPhEP evaluation's: Bess, Fujimoto, Dolphin,
Snoj & Zukeran, *Evaluation of the Start-Up Core Physics Tests at Japan's
HTTR (Fully-Loaded Core)*, HTTR-GCR-RESR-001, INL/EXT-08-14767 Rev. 2
(March 2010), cited as **R1**. Page numbers are R1's printed pages.

Where R1's §3 text and its sample MCNP5 input (App. A.1.8, pp. 235-246,
"the deck") give the same number, the deck's surface or cell card is
named as well. Where they disagree, this module follows the deck, says
so, and names the text it departs from: the deck is the model behind
every reference `k_eff` R1 tabulates, so following it keeps the
code-to-code comparison like for like. Each such case is listed in the
module docs of [`super`].

All lengths are cm, in the frame the deck uses: `z = 0` at the core's
axial mid-plane, the core 522 cm tall, the fuel from `z = -145` to
`z = +145`.

```rust
pub mod spec { /* ... */ }
```

### Types

#### Enum `RodBank`

Which rod bank a control column belongs to (R1 §3.1.2.5).

```rust
pub enum RodBank {
    C,
    R1,
    R2,
    R3,
}
```

##### Variants

###### `C`

The centre column A01.

###### `R1`

The six columns of ring C between the fuel.

###### `R2`

Six columns of ring E.

###### `R3`

Three columns of ring E.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
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
#### Struct `RodPositions`

Rod positions of a benchmark state: the height of each bank's rod bottom
above the bottom of the fuel \[cm\] (R1 §3.1.2.11-12).

```rust
pub struct RodPositions {
    pub c: f64,
    pub r1: f64,
    pub r2: f64,
    pub r3: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `c` | `f64` | C bank. |
| `r1` | `f64` | R1 bank. |
| `r2` | `f64` | R2 bank. |
| `r3` | `f64` | R3 bank. |

##### Implementations

###### Methods

- ```rust
  pub fn of(self: &Self, bank: RodBank) -> f64 { /* ... */ }
  ```
  The rod-bottom height of `bank` above the fuel bottom \[cm\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `BpLoading`

Burnable-poison natural-boron content of a fuel block.

```rust
pub enum BpLoading {
    Two,
    TwoAndHalf,
}
```

##### Variants

###### `Two`

2.0 wt.% natural boron (R1 Table 3.5).

###### `TwoAndHalf`

2.5 wt.% natural boron.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
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
#### Enum `FuelLayer`

The fuel layer of a block, top (`L1`) to bottom (`L5`). L4 and L5 carry
the same loading (R1 Table 1.11).

```rust
pub enum FuelLayer {
    L1,
    L2,
    L3,
    L45,
}
```

##### Variants

###### `L1`

Top fuel layer.

###### `L2`

Second.

###### `L3`

Third.

###### `L45`

Fourth or fifth (same loading).

##### Implementations

###### Methods

- ```rust
  pub fn bp(self: Self) -> BpLoading { /* ... */ }
  ```
  Burnable-poison loading of a block in this layer (R1 Table 1.11: 2.0

- ```rust
  pub fn at(k: i32) -> Option<Self> { /* ... */ }
  ```
  The fuel layer at column layer `k` (`-2..=2`, bottom to top), or

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
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
#### Enum `FuelZone`

Fuel zone 1-4 (R1 Fig. 3.17), by its column ring.

```rust
pub enum FuelZone {
    Z1,
    Z2,
    Z3,
    Z4,
}
```

##### Variants

###### `Z1`

Ring B.

###### `Z2`

Ring C, between the R1 columns.

###### `Z3`

Ring D, except its corners.

###### `Z4`

The six corners of ring D.

##### Implementations

###### Methods

- ```rust
  pub fn enrichment_index(self: Self, layer: FuelLayer) -> usize { /* ... */ }
  ```
  Index of the enrichment ([`ENRICHMENTS`]) in a block of this zone at

- ```rust
  pub fn has_31_holes(self: Self) -> bool { /* ... */ }
  ```
  Whether this zone's blocks have 31 fuel holes (zones 3 and 4) rather

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
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
#### Enum `PinSlot`

What occupies a hole position of a fuel block, in the block's own
(unrotated) frame. Index `(i, j)` is the deck's `lat=2` index
(cells 401-416): position `i * a1 + j * a2` with `a1 = (5.15, 0)` and
`a2 = 5.15 (cos 60, sin 60)`.

```rust
pub enum PinSlot {
    Fuel,
    Bp,
    EmptyBp,
    Graphite,
}
```

##### Variants

###### `Fuel`

A fuel rod.

###### `Bp`

A burnable-poison pin.

###### `EmptyBp`

The empty third BP hole (helium).

###### `Graphite`

Solid block graphite (the centre, and in 31-hole blocks two corners).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `ColumnRole`

What a column is.

```rust
pub enum ColumnRole {
    Fuel(FuelZone),
    Control(RodBank),
    Instrumentation,
    Reflector,
}
```

##### Variants

###### `Fuel`

A fuel column of the given zone.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FuelZone` |  |

###### `Control`

A control-rod guide column of the given bank.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RodBank` |  |

###### `Instrumentation`

An instrumentation column (three empty holes).

###### `Reflector`

A replaceable reflector column (solid graphite).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
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
- **Move**
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
#### Struct `ColumnSpec`

One column of the core map: its deck lattice index `(i, j)`, its role,
its rotation \[deg\] and its R1 name.

```rust
pub struct ColumnSpec {
    pub i: i32,
    pub j: i32,
    pub role: ColumnRole,
    pub rotation_deg: f64,
    pub name: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `i` | `i32` | Deck core-lattice index along `a1 = (0, 36.2)` cm. |
| `j` | `i32` | Deck core-lattice index along `a2 = 36.2 (cos 150, sin 150)` cm. |
| `role` | `ColumnRole` | Role. |
| `rotation_deg` | `f64` | Rotation of the column's contents about its axis \[deg\], the angle<br>of its local x axis in the core frame (deck `*trcl`). |
| `name` | `&'static str` | R1's column name (Fig. 3.17), as the deck's comments give it; ""<br>where the deck names none (reflector and instrumentation columns). |

##### Implementations

###### Methods

- ```rust
  pub fn centre(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Centre of the column \[cm\] in the core frame.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `fuel_block_slot`

**Attributes:**

- `MustUse { reason: None }`

The pin slot at block-lattice index `(i, j)` of an unrotated fuel block
(deck cells 401-416). `None` outside the three rings of holes.

```rust
pub fn fuel_block_slot(i: i32, j: i32, holes_31: bool) -> Option<PinSlot> { /* ... */ }
```

#### Function `hex_ring`

**Attributes:**

- `MustUse { reason: None }`

Hex distance of index `(i, j)` from the centre, on a lattice whose
second axis is 60 degrees from the first.

```rust
pub fn hex_ring(i: i32, j: i32) -> i32 { /* ... */ }
```

#### Function `core_map`

**Attributes:**

- `MustUse { reason: None }`

The 61 columns of the fully-loaded core, decoded from the deck's core
map. Pinned by `tests::the_core_map_has_rings_a_to_e`.

```rust
pub fn core_map() -> Vec<ColumnSpec> { /* ... */ }
```

### Constants and Statics

#### Constant `TRISO_RADII_CM`

Outer radii of the coated particle's six regions \[cm\]: kernel (600 µm
diameter), buffer (60 µm), IPyC (30 µm), SiC (25 µm), OPyC (45 µm) and
the resinated-graphite overcoat (200 µm). R1 Fig. 3.1; deck surfaces 1-6.

```rust
pub const TRISO_RADII_CM: [f64; 6] = _;
```

#### Constant `TRISO_PITCH_CM`

The ordered TRISO lattice's pitch \[cm\]: x, y, z. R1 §4.1.1 (the
sample-calculation approach; R1 §3.1.1 specifies a random distribution);
deck surfaces 911-916.

The overcoat sphere (r = 0.066 cm) is larger than half the pitch, so each
tile clips it; the rest of the tile is compact matrix. That is what the
deck builds (cell 7 is the matrix beyond surface 6 inside the tile).

```rust
pub const TRISO_PITCH_CM: [f64; 3] = _;
```

#### Constant `TRISO_LAYER`

One 0.1 cm layer of the fuel compact as the deck tiles it (cell 163,
`fill=-13:13 -13:13 0:0`): `'o'` is a TRISO particle, `'.'` compact
matrix. Row `k` is `y = (k - 13) * 0.106` cm, column `i` is
`x = (i - 13) * 0.106` cm, both in the fuel pin's frame.

**333 particles per layer**, 546 layers per 54.6 cm compact stack:
181 818 per rod, which is R1's 12 987 per 3.9 cm compact x 14 (R1
§3.1.1). Every particle's OPyC (r = 0.046 cm) lies wholly inside the
0.5-1.3 cm annulus: its centres run from r = 0.5708 to 1.2407 cm.
Pinned by `tests::the_layer_pattern_holds_333_whole_particles`.

```rust
pub const TRISO_LAYER: [&str; 27] = _;
```

#### Constant `TRISO_PER_LAYER`

Particles per 0.1 cm layer, the count of `'o'` in [`TRISO_LAYER`].

```rust
pub const TRISO_PER_LAYER: usize = 333;
```

#### Constant `TRISO_LAYERS`

Layers in one fuel rod's compact stack: 54.6 cm / 0.1 cm.

```rust
pub const TRISO_LAYERS: usize = 546;
```

#### Constant `COMPACT_INNER_R`

The compact stack, modelled as one annulus (R1 §3.1.2.1, Fig. 3.3 p.194;
deck surfaces 12, 13): inner radius 0.5 cm (helium inside), outer 1.3 cm,
54.6 cm tall, centred on the block.

```rust
pub const COMPACT_INNER_R: f64 = 0.5;
```

#### Constant `COMPACT_OUTER_R`

Outer radius of the compact annulus \[cm\].

```rust
pub const COMPACT_OUTER_R: f64 = 1.3;
```

#### Constant `COMPACT_HALF_HEIGHT`

Half-height of the compact stack \[cm\].

```rust
pub const COMPACT_HALF_HEIGHT: f64 = 27.3;
```

#### Constant `SLEEVE_INNER_R`

Graphite sleeve inner radius and half-height of its cavity \[cm\]
(deck surface 21: r 1.325, 54.9 cm). The cavity holds the compacts with a
0.025 cm helium gap and 0.15 cm of helium above and below.

```rust
pub const SLEEVE_INNER_R: f64 = 1.325;
```

#### Constant `SLEEVE_INNER_HALF_HEIGHT`

Half-height of the sleeve's inner cavity \[cm\].

```rust
pub const SLEEVE_INNER_HALF_HEIGHT: f64 = 27.45;
```

#### Constant `SLEEVE_OUTER_R`

Sleeve outer radius and half-height \[cm\] (deck surface 22: r 1.7,
57.7 cm; the 1.4 cm above and below the cavity are the end caps).

```rust
pub const SLEEVE_OUTER_R: f64 = 1.7;
```

#### Constant `SLEEVE_HALF_HEIGHT`

Half-height of the whole sleeve, end caps included \[cm\].

```rust
pub const SLEEVE_HALF_HEIGHT: f64 = 28.85;
```

#### Constant `FUEL_HOLE_R`

Fuel-hole radius \[cm\]: the 41 mm hole through the block (deck surface
31). Helium fills it outside the sleeve, top to bottom of the block.

```rust
pub const FUEL_HOLE_R: f64 = 2.05;
```

#### Constant `BP_R`

Burnable-poison pellet radius \[cm\] (deck surfaces 41-43).

```rust
pub const BP_R: f64 = 0.7;
```

#### Constant `BP_HOLE_R`

Burnable-poison hole radius \[cm\] (deck surface 44).

```rust
pub const BP_HOLE_R: f64 = 0.75;
```

#### Constant `BP_Z`

Axial layout of a BP pin in the block frame \[cm\] (deck surfaces 41-44):
poison from -25 to -5, graphite disks from -5 to +5, poison from +5 to
+25, the hole from -25 to +25. R1 Fig. 3.4: two 200 mm poison stacks
either side of a 100 mm disk stack.

```rust
pub const BP_Z: [f64; 4] = _;
```

#### Constant `PIN_PITCH`

Pitch of the fuel-hole lattice in a block \[cm\]: 51.5 mm, triangular
(R1 §3.1.2.3; deck surface 501, hexagon apothem 2.575 cm). The deck's
pin hexagons have flats perpendicular to x.

```rust
pub const PIN_PITCH: f64 = 5.15;
```

#### Constant `BLOCK_APOTHEM`

Fuel and reflector blocks: apothem of the hexagonal prism \[cm\], i.e.
half the 360 mm across-flats width (R1 §1.1.2.3; deck surfaces 502,
550). Flats perpendicular to y.

```rust
pub const BLOCK_APOTHEM: f64 = 18.0;
```

#### Constant `BLOCK_HEIGHT`

Block height \[cm\]: the column lattice pitch (deck surface 503). R1
gives 580 mm (Table 1.13); the deck's block body (59 cm, surface 502) is
truncated by the 58 cm lattice element, so the blocks touch.

```rust
pub const BLOCK_HEIGHT: f64 = 58.0;
```

#### Constant `LAYERS_PER_COLUMN`

Layers in a column: 2 bottom reflector, 5 fuel, 2 top reflector (R1
§3.1.2.9, Fig. 3.19 p.210). Layer `k` (`-4..=4`) is centred at
`z = 58 k`.

```rust
pub const LAYERS_PER_COLUMN: i32 = 9;
```

#### Constant `CORE_HALF_HEIGHT`

Half-height of the core: 9 x 58 / 2 = 261 cm (deck surfaces 551, 651).

```rust
pub const CORE_HALF_HEIGHT: f64 = 261.0;
```

#### Constant `FUEL_BOTTOM_Z`

Bottom of the fuel (the bottom of layer 5), the zero of every rod
position \[cm\] (R1 §3.1.2.11).

```rust
pub const FUEL_BOTTOM_Z: f64 = -145.0;
```

#### Constant `REFLECTOR_CHANNEL_R`

Reflector-block coolant channel radius \[cm\]: 23 mm diameter (R1 Figs.
3.12-3.13; deck surface 201). **R1's text (§3.1.1) says 21 mm**; the
figures and the deck say 23 mm, and the deck is followed. R1 Table 2.15
finds the difference negligible.

```rust
pub const REFLECTOR_CHANNEL_R: f64 = 1.15;
```

#### Constant `COLUMN_PITCH`

Column pitch \[cm\]: 362 mm (R1 §1.1.2.3; deck surface 551, apothem 18.1).
Every column is 360 mm across flats, so the 2 mm between columns is
helium. Columns have flats perpendicular to y.

```rust
pub const COLUMN_PITCH: f64 = 36.2;
```

#### Constant `PERMANENT_REFLECTOR_R`

Radius of the permanent-reflector cylinder \[cm\] (R1 §3.1.2.10; deck
surface 651). Vacuum beyond it and above and below `|z| = 261`.

```rust
pub const PERMANENT_REFLECTOR_R: f64 = 212.5;
```

#### Constant `CR_HOLE_R`

Control-rod hole radius \[cm\] (deck surfaces 154-156: 123 mm diameter).

```rust
pub const CR_HOLE_R: f64 = 6.15;
```

#### Constant `CR_HOLE_OFFSET`

Distance of each hole's axis from the column's \[cm\] (deck: 108 mm).

```rust
pub const CR_HOLE_OFFSET: f64 = 10.8;
```

#### Constant `CR_HOLE_ANGLES_DEG`

Angles of the three holes in an unrotated column \[deg\], in the order
rod, rod, reserve-shutdown (RSS) hole: deck surfaces 154 at (10.8, 0),
155 at (-5.4, -9.35307) and 156 at (-5.4, 9.35307). Rods sit in 154 and
155 (deck cells 611, 612); 156 is the empty RSS hole (cell 613).

```rust
pub const CR_HOLE_ANGLES_DEG: [f64; 3] = _;
```

#### Constant `CR_HOLE_BOTTOM_Z`

Bottom of every control-rod hole \[cm\]: 1060 mm above the core bottom
(R1 §3.1.2.5; deck surfaces 154-156 start at z = -155).

```rust
pub const CR_HOLE_BOTTOM_Z: f64 = -155.0;
```

#### Constant `ROD_SPINE_R`

Control-rod radii \[cm\] (R1 Figs. 3.7-3.8; deck surfaces 101-105):
spine, helium, inner clad, absorber, outer clad.

```rust
pub const ROD_SPINE_R: f64 = 0.5;
```

#### Constant `ROD_INNER_CLAD_IN_R`

Inner clad's inner radius \[cm\] (helium from the spine to here).

```rust
pub const ROD_INNER_CLAD_IN_R: f64 = 3.25;
```

#### Constant `ROD_ABSORBER_IN_R`

Inner clad's outer radius = absorber inner radius \[cm\].

```rust
pub const ROD_ABSORBER_IN_R: f64 = 3.75;
```

#### Constant `ROD_ABSORBER_OUT_R`

Absorber outer radius = outer clad inner radius \[cm\].

```rust
pub const ROD_ABSORBER_OUT_R: f64 = 5.25;
```

#### Constant `ROD_OUTER_R`

Outer clad outer radius \[cm\].

```rust
pub const ROD_OUTER_R: f64 = 5.65;
```

#### Constant `ROD_SECTION`

One rod section is 31 cm: a 1 cm Alloy 800H end plate, 29 cm of
absorber annulus, a 1 cm end plate (deck surfaces 101-105, lattice 151).

```rust
pub const ROD_SECTION: f64 = 31.0;
```

#### Constant `ROD_END_PLATE`

End-plate thickness \[cm\].

```rust
pub const ROD_END_PLATE: f64 = 1.0;
```

#### Constant `ROD_SECTIONS`

Sections per rod (R1 §3.1.2.5: ten).

```rust
pub const ROD_SECTIONS: usize = 10;
```

#### Constant `ENRICHMENTS`

The twelve enrichments \[wt.% U-235\] in R1 Table 3.1's order. Kernel
material `i` is enrichment `ENRICHMENTS[i]`.

```rust
pub const ENRICHMENTS: [f64; 12] = _;
```

### Types

#### Struct `HttrDataConfig`

What a run reads.

```rust
pub struct HttrDataConfig {
    pub graphite_law: crate::htr10_rmc::materials::GraphiteLaw,
    pub thermal: HttrThermal,
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub tapes: crate::htr10_rmc::data::TapeSource,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `graphite_law` | `crate::htr10_rmc::materials::GraphiteLaw` | Graphite law (default 30 % porous, see [`materials`]). |
| `thermal` | `HttrThermal` | Bound laws or the free-gas ablation. |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature of every nuclide and law. |
| `tapes` | `crate::htr10_rmc::data::TapeSource` | Where the tapes are. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `ReferenceK`

A reference `k` and where it is from.

```rust
pub struct ReferenceK {
    pub label: &'static str,
    pub k: f64,
    pub minus: f64,
    pub plus: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `label` | `&'static str` | What it is. |
| `k` | `f64` | `k`. |
| `minus` | `f64` | Lower 1-sigma uncertainty (equal to `plus` where symmetric). |
| `plus` | `f64` | Upper 1-sigma uncertainty. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `load_httr_nuclides`

Read and process every HTTR nuclide (48 slots, [`materials::nuc`]) and
thermal law, through the same loader as HTR-10.

# Errors
A tape that is absent or fails to process.

```rust
pub fn load_httr_nuclides<F: FnMut(crate::htr10_rmc::data::LoadProgress)>(cfg: &HttrDataConfig, diag: &mut outram_mc_libs::run_diagnostics::RunDiagnostics, progress: F) -> Result<Vec<outram_mc_libs::material::nuclide::Nuclide>, crate::htr10_rmc::data::Htr10DataError> { /* ... */ }
```

#### Function `compact_majorant`

**Attributes:**

- `MustUse { reason: None }`

The compact's majorant: the kernels, coatings, overcoat and matrix
(`materials::mat::COMPACT`), on HTR-10's energy grid plus every nuclide
breakpoint, with HTR-10's 0.3 margin.

```rust
pub fn compact_majorant(mats: &[outram_mc_libs::material::material::Material], nucs: &[outram_mc_libs::material::nuclide::Nuclide]) -> outram_mc_libs::physics::delta_tracking::majorant::Majorant { /* ... */ }
```

#### Function `fuel_source_box`

**Attributes:**

- `MustUse { reason: None }`

The initial-source box: every fuel column (rings B-D reach r = 129.5 cm
at a column corner) over the fuel's height.

```rust
pub fn fuel_source_box() -> outram_mc_libs::physics::transport_csg::SourceBox { /* ... */ }
```

#### Function `fuel_entropy_mesh`

**Attributes:**

- `MustUse { reason: None }`

A 9 x 9 x 5 Shannon-entropy mesh over [`fuel_source_box`]: one axial
bin per fuel layer, radial bins of about one column.

```rust
pub fn fuel_entropy_mesh() -> outram_mc_libs::tally::mesh::RegularMesh { /* ... */ }
```

### Constants and Statics

#### Constant `BENCHMARK_TEMPERATURE_K`

The benchmark temperature, 300 K (R1 §3.1.4).

```rust
pub const BENCHMARK_TEMPERATURE_K: f64 = 300.0;
```

#### Constant `BENCHMARK_CRITICAL`

Validation: the evaluated **benchmark** `k_eff` of the fully-loaded
critical, 1.0025 (-0.0060 / +0.0071) (R1 §3.1.5.1). The measured delayed
critical (k = 1.00025, R1 Table 1.17) plus the instrumentation-removal
bias (+0.254 ± 0.073 % Δk), with the evaluated uncertainties, dominated
by IG-110 and PGX boron impurity (R1 Table 2.64).

```rust
pub const BENCHMARK_CRITICAL: ReferenceK = _;
```

#### Constant `BENCHMARK_SUBCRITICAL`

Validation: the evaluated benchmark `k` of the fully-loaded
**subcritical** state (all rods at -55 mm), 0.6876 ± 0.0104 (R1 §3.1.5.2,
Table 2.65), from an inverse-kinetics measurement.

```rust
pub const BENCHMARK_SUBCRITICAL: ReferenceK = _;
```

#### Constant `MCNP5_R1`

Verification (code to code): R1's own MCNP5 results on the same benchmark
model, ordered TRISO lattice, 50 000 x [50 + 1000] histories, ±0.0001
(R1 Table 4.1 for the critical, Table 4.2 for the subcritical). Library,
critical, subcritical.

```rust
pub const MCNP5_R1: [(&str, f64, f64); 6] = _;
```

### Re-exports

#### Re-export `assemble_httr`

```rust
pub use geometry::assemble_httr;
```

#### Re-export `AssembledHttr`

```rust
pub use geometry::AssembledHttr;
```

#### Re-export `CompactTracking`

```rust
pub use geometry::CompactTracking;
```

#### Re-export `httr_material_set`

```rust
pub use materials::httr_material_set;
```

#### Re-export `httr_nuclide_slots`

```rust
pub use materials::httr_nuclide_slots;
```

#### Re-export `HttrMaterialConfig`

```rust
pub use materials::HttrMaterialConfig;
```

#### Re-export `HttrThermal`

```rust
pub use materials::HttrThermal;
```

#### Re-export `RodPositions`

```rust
pub use spec::RodPositions;
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
| scalar | `[Material, Energy]` | `Flux, Total, Absorption, NuFission, KappaFission, Fission` |
| matrix | `[Material, Energy, EnergyOut]` | `ScatterN` (nu-scatter), `NuFission` (births, for `chi`), `Scatter` (scatter-only, since 2026-10-13) |

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

## Collapsing to a coarser structure

[`MgxsLibrary::collapse`] merges groups of a library onto any structure
whose edges are a subset of its own. Every constant here is a ratio of
rates to flux, and both sum over the merged groups, so collapsing the
means is **exact** (reaction rates are conserved to rounding), not an
approximation. A run tallied once on a fine structure therefore yields
every nested coarse structure without another transport pass
([`crate::htr10_mgxs`] relies on this). What it does NOT carry over is the
statistical uncertainty: the tally keeps per-bin moments only, not the
covariance between bins of one batch, so the uncertainty of a merged group
is not recoverable from the fine one.

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
    NotNested {
        edge_ev: f64,
    },
    Inconsistent {
        zone: String,
        group: usize,
        expected: f64,
        row: f64,
        residual: f64,
        allowed: f64,
    },
    NoUncertainty {
        zone: String,
        group: usize,
        relative: f64,
    },
    NoScatterOnly {
        zone: String,
    },
    NotTransportable {
        zone: String,
        what: String,
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

###### `NotNested`

A collapse target has an edge that is not an edge of the library being
collapsed, so a fine group would have to be split between two coarse
ones. Splitting needs the spectrum inside the group, which the library
does not carry; refusing is the only exact answer.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `edge_ev` | `f64` | The offending coarse edge \[eV\]. |

###### `Inconsistent`

[`MgxsLibrary::to_physics_mg`]: a group's scattering row disagrees with
`Sigma_t - Sigma_a` by more than its statistics allow. The two come
from separate estimators, so this is a condensation defect (or a
library assembled wrongly), not noise.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `zone` | `String` | Zone name. |
| `group` | `usize` | Group index in the library's own ascending order. |
| `expected` | `f64` | `Sigma_t - Sigma_a` \[cm^-1\]. |
| `row` | `f64` | The scattering row sum \[cm^-1\]. |
| `residual` | `f64` | `row - expected` \[cm^-1\]. |
| `allowed` | `f64` | The bound it was held to \[cm^-1\]. |

###### `NoUncertainty`

[`MgxsLibrary::to_physics_mg`]: the library carries no standard errors
([`ZoneMgxs::sigma`] is `None`) and is not exactly balanced, so a
statistical disagreement cannot be told from a defect.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `zone` | `String` | Zone name. |
| `group` | `usize` | The first group found off balance (ascending order). |
| `relative` | `f64` | Its relative residual. |

###### `NoScatterOnly`

[`MgxsLibrary::to_physics_mg`]: the library has no scatter-only matrix
([`ZoneMgxs::scatter_only`] empty: condensed before 2026-10-13), so
the scattering multiplicity cannot be formed.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `zone` | `String` | Zone name. |

###### `NotTransportable`

[`MgxsLibrary::to_physics_mg`]: an ordering the multigroup kernel
requires does not hold (`Sigma_f <= Sigma_a <= Sigma_t`, a fissile zone
with a birth spectrum, matching group counts).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `zone` | `String` | Zone name. |
| `what` | `String` | What failed. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
- **Forget**
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
    fn from(source: MgxsError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `GroupStructure`

**Attributes:**

- `Other("#[serde(try_from = \"Vec<f64>\", into = \"Vec<f64>\")]")`

An energy group structure, as ascending bin edges in eV.

`n + 1` edges define `n` groups, and **group 0 is the lowest-energy group**
— see the module note on ordering.

Serialises as the plain edge list; deserialising re-runs [`Self::new`], so
a file cannot smuggle in edges that do not ascend.

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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(g: GroupStructure) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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

  - ```rust
    fn try_from(edges: Vec<f64>) -> Result<Self, MgxsError> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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
    pub fission: Vec<f64>,
    pub nu_fission: Vec<f64>,
    pub kappa_fission: Vec<f64>,
    pub scatter: Vec<Vec<f64>>,
    pub scatter_only: Vec<Vec<f64>>,
    pub chi: Vec<f64>,
    pub sigma: Option<ZoneMgxsSigma>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable zone name, carried through from the material. |
| `flux` | `Vec<f64>` | Group scalar flux \[arbitrary, per source particle\]. This is the<br>weighting denominator, kept because a zero here is what makes every<br>other entry in the group meaningless, and callers must be able to see it. |
| `total` | `Vec<f64>` | Total macroscopic cross section \[cm^-1\]. |
| `absorption` | `Vec<f64>` | Absorption (capture + fission) \[cm^-1\]. |
| `fission` | `Vec<f64>` | Fission `Sigma_f` \[cm^-1\] (added 2026-10-12, gh:#848). A multigroup<br>transport code needs it beside `nu * Sigma_f`: it is what `nu-bar`<br>and the capture/fission split at a collision are formed from<br>(`outram_mc_libs::physics::physics_mg::Mgxs::new`). |
| `nu_fission` | `Vec<f64>` | Neutron production `nu * Sigma_f` \[cm^-1\]. |
| `kappa_fission` | `Vec<f64>` | Fission energy deposition `kappa * Sigma_f` \[J cm^-1\]. |
| `scatter` | `Vec<Vec<f64>>` | Isotropic (`P0`) scattering matrix, `scatter[g_in][g_out]` \[cm^-1\].<br><br>**A nu-scatter matrix**: [`condense`] fills it from the tally's<br>`ScatterN` score, which counts every neutron a scatter emits, so an<br>(n,2n) contributes twice (once in each neutron's outgoing group). Its<br>row therefore exceeds `Sigma_t - Sigma_a` by the (n,xn) gain;<br>[`Self::scatter_only`] is the matrix that does not. |
| `scatter_only` | `Vec<Vec<f64>>` | The scatter-only matrix, one count per scattering collision on the<br>primary neutron's outgoing group \[cm^-1\] (added 2026-10-13,<br>gh:#682); OpenMC's `scatter matrix` beside its `nu-scatter matrix`.<br>`scatter[g][g'] / scatter_only[g][g']` is the multiplicity, and its<br>row is the tallied estimate of `Sigma_t - Sigma_a`. **Empty** for a<br>library condensed before the score existed (records deserialise with<br>it empty). |
| `chi` | `Vec<f64>` | Fission spectrum `chi_g`, normalised to sum to 1 over groups.<br><br>**Measured, not assumed**: condensed from the energies fission neutrons<br>were actually born with during the run, via<br>`outram_mc_libs::tally::scoring::score_fission_birth`. A zone that<br>fissioned not at all carries all zeros, and the sum is then 0 rather<br>than 1 -- check before using it as a source distribution. |
| `sigma` | `Option<ZoneMgxsSigma>` | Standard errors of the constants above, per group, as [`condense`]<br>estimated them (added 2026-10-13, gh:#848). `None` when they are not<br>known: a library built by hand, one condensed from fewer than two<br>active batches, one derived by [`MgxsLibrary::collapse`],<br>[`MgxsLibrary::homogenised`] or [`MgxsLibrary::rebalanced`] (none of<br>which can carry them, see [`ZoneMgxsSigma`]), and every record written<br>before the field existed, which deserialise with `None`. |

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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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
#### Struct `ZoneMgxsSigma`

One standard error per condensed constant of a [`ZoneMgxs`], with the
same shapes and the same ascending group order.

# How they are estimated, and what that misses

Every constant is a ratio `R = X / phi` of two tally means over the same
batches. [`condense`] takes each mean's standard error from its own bin
([`TallyBin::rel_std_dev`], the standard error of the batch mean) and
combines the two in quadrature:

```text
  sigma_R = |R| * sqrt( (sigma_X / X)^2 + (sigma_phi / phi)^2 )
```

That drops the covariance term `-2 rho (sigma_X/X)(sigma_phi/phi)`. The
numerator and the flux are scored on the same tracks in the same batches,
so `rho > 0` and the dropped term would make `sigma_R` **smaller**: these
are upper estimates. The tally keeps per-bin moments only, not the
covariance between bins, which is also why no `chi` uncertainty is given
(each `chi_g` is a sum of bins of one batch) and why a group merged by
[`MgxsLibrary::collapse`] has none.

`flux` is the flux's own absolute standard error.

```rust
pub struct ZoneMgxsSigma {
    pub flux: Vec<f64>,
    pub total: Vec<f64>,
    pub absorption: Vec<f64>,
    pub fission: Vec<f64>,
    pub nu_fission: Vec<f64>,
    pub kappa_fission: Vec<f64>,
    pub scatter: Vec<Vec<f64>>,
    pub scatter_only: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `flux` | `Vec<f64>` | Standard error of [`ZoneMgxs::flux`]. |
| `total` | `Vec<f64>` | Standard error of [`ZoneMgxs::total`] \[cm^-1\]. |
| `absorption` | `Vec<f64>` | Standard error of [`ZoneMgxs::absorption`] \[cm^-1\]. |
| `fission` | `Vec<f64>` | Standard error of [`ZoneMgxs::fission`] \[cm^-1\]. |
| `nu_fission` | `Vec<f64>` | Standard error of [`ZoneMgxs::nu_fission`] \[cm^-1\]. |
| `kappa_fission` | `Vec<f64>` | Standard error of [`ZoneMgxs::kappa_fission`] \[J cm^-1\]. |
| `scatter` | `Vec<Vec<f64>>` | Standard error of each [`ZoneMgxs::scatter`] element \[cm^-1\]. |
| `scatter_only` | `Vec<Vec<f64>>` | Standard error of each [`ZoneMgxs::scatter_only`] element<br>\[cm^-1\]; empty when that matrix is. |

##### Implementations

###### Methods

- ```rust
  pub fn scatter_row_bound(self: &Self, g: usize) -> Option<f64> { /* ... */ }
  ```
  An upper bound on the standard error of row `g`'s sum,

- ```rust
  pub fn scatter_only_row_bound(self: &Self, g: usize) -> Option<f64> { /* ... */ }
  ```
  The same bound for [`ZoneMgxs::scatter_only`]'s row.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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
  pub fn collapse(self: &Self, coarse: &GroupStructure) -> Result<MgxsLibrary, MgxsError> { /* ... */ }
  ```
  The same library on the coarser structure `coarse`, whose edges must be

- ```rust
  pub fn to_physics_mg(self: &Self) -> Result<outram_mc_libs::physics::physics_mg::MgxsLibrary, MgxsError> { /* ... */ }
  ```
  Convert to the multigroup transport kernel's library,

- ```rust
  pub fn to_physics_mg_without_multiplicity(self: &Self) -> Result<outram_mc_libs::physics::physics_mg::MgxsLibrary, MgxsError> { /* ... */ }
  ```
  **Ablation**: [`Self::to_physics_mg`] without the scattering

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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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

#### Function `condense`

Condense a completed pair of Monte Carlo tallies into group constants.

# Parameters
- `groups` — the structure both tallies were binned on.
- `zone_names` — one name per zone, in **material-index order**, matching
  the `MaterialFilter` the tallies carried.
- `scalar` — a tally with filters `[Material, Energy]` and scores exactly
  [`SCALAR_SCORES`], already run.
- `matrix` — a tally with filters `[Material, Energy, EnergyOut]` and
  scores exactly [`MATRIX_SCORE_TYPES`], already run at the same seed.
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

#### Constant `CONSISTENCY_Z`

How many standard errors a scattering row may differ from
`Sigma_t - Sigma_a` before [`MgxsLibrary::to_physics_mg`] refuses it.

Fixed at 4 before any library was converted. The bound it multiplies is
itself an upper bound on the standard error (see [`ZoneMgxsSigma`]), so a
genuine 4-sigma excursion is rarer than the nominal 6e-5 per group.

```rust
pub const CONSISTENCY_Z: f64 = 4.0;
```

#### Constant `CONSISTENCY_REL_FLOOR`

The relative balance a library must hold in every group when it carries
no standard errors, and the floor under the statistical bound when it
does: rounding, not statistics.

```rust
pub const CONSISTENCY_REL_FLOOR: f64 = 1.0e-9;
```

#### Constant `MATRIX_SCORE_TYPES`

The score order the matrix pass must declare, and that [`condense`]
assumes: the nu-scatter matrix (`ScatterN`), the fission births that give
`chi` (`NuFission`) and, appended 2026-10-13 (gh:#682), the scatter-only
matrix (`Scatter`). All three need the same
`[Material, Energy, EnergyOut]` filters, so they share one tally and one
transport pass. Exposed so a caller building its own matrix tally
(`dhoby-ghaut`'s mesh tally) cannot disagree with the reader.

```rust
pub const MATRIX_SCORE_TYPES: [outram_mc_libs::tally::tally::ScoreType; 3] = _;
```

#### Constant `MATRIX_SCORES`

Number of scores in the matrix pass (`MATRIX_SCORE_TYPES.len()`).

```rust
pub const MATRIX_SCORES: usize = _;
```

#### Constant `SCALAR_SCORES`

The score order the scalar pass must declare, and that [`condense`] assumes.

Exposed so a caller building the tally cannot silently disagree with the
reader about which score sits at which offset. `Fission` was appended last
(2026-10-12, gh:#848) so the offsets of the first five, which
`dhoby-ghaut`'s mesh tally also reads, did not move.

```rust
pub const SCALAR_SCORES: [outram_mc_libs::tally::tally::ScoreType; 6] = _;
```

## Module `htr10_mgxs`

HTR-10 MGXS generation cases: a registry of named runs, the runner, and
the record each run writes (gh:#845).
# HTR-10 MGXS generation cases (gh:#845)

Named, fixed Monte Carlo runs that condense HTR-10 multigroup cross
sections, built so a long run can be handed to a machine with cores to
spare and nothing else: one command per case, a record written to disk,
nothing to decide while it runs. The runbook is
`crates/nee_soon/docs/htr10-mgxs-desktop-runbook.md`.

Orchestration only, as everywhere in this crate: the transport is
`outram_mc_libs`' hybrid delta/surface tracker, the model is
[`crate::htr10_rmc`], and the condensation is [`crate::mgxs`].

## What a case does

1. Process the nuclear data at the case temperature
   ([`crate::htr10_rmc::data`], ENDF/B-VIII.0, every material at that
   temperature: an **isothermal** state point).
2. Assemble the model ([`cases::CaseModel`]): the full core, Şeker &
   Çolak's 13-ball bed with explicit TRISO, the geometry every recorded
   HTR-10 `k` in this crate is on; or, since 2026-10-13 (gh:#849), one of
   the small cells of [`cells`] built from the same particle and pebbles.
3. Run two k-eigenvalue passes at one seed, the scalar and the
   scattering-matrix tallies of [`crate::mgxs`], per material, on the
   **70-group tally structure** of [`groups`] (WIMS-69 plus 10-20 MeV).
4. Condense, then [`crate::mgxs::MgxsLibrary::collapse`] onto every
   structure of [`groups::Collapse`]. Collapsing is exact, so one run gives
   2, 4, 8 and 70 groups at once.
5. Write [`record::CaseRecord`]: `record.json` (everything, including the
   fine library), `summary.csv` (one row) and `README.md` (methodology and
   results, generated from the record, so the V&V rule's two halves cannot
   drift apart).

## What a case does NOT do yet

- **No multigroup `k_eff`.** The gate of #850 needs the converter to
  `physics_mg` (#848) and the scattering multiplicity (#682). ~~The saved
  library is what that gate will read, so these runs are not repeated
  when it lands.~~ **CORRECTED 2026-10-13:** both have landed, and the
  converter needs per-group standard errors and the scatter-only matrix,
  which a record written before that date lacks; such a record converts
  only through explicit ablations
  (`verification_and_validation/htr10_mgxs_runs/README.md`).
- **No uncertainty on the collapsed constants.** Tallies keep per-bin
  moments, not the covariance between the fine bins of one batch, so the
  σ of a merged group is not recoverable (see [`crate::mgxs`]). The
  replica-seed case measures the spread directly instead.
- **No separate fuel and moderator temperatures** (#852): every material
  is at the case temperature.

```rust
pub mod htr10_mgxs { /* ... */ }
```

### Modules

## Module `cases`

The registry of HTR-10 MGXS cases.

**The registry is the specification of every run.** A case's model,
temperature, particle budget and seed are fixed here, in committed source,
before it runs; the record a run writes names its case and commit, so the
two can always be matched. Changing a case after it has produced a record
means giving it a new name.

```rust
pub mod cases { /* ... */ }
```

### Types

#### Enum `CaseModel`

The model a case transports.

```rust
pub enum CaseModel {
    FullCore {
        layers: usize,
    },
    Cell(super::cells::CellKind),
}
```

##### Variants

###### `FullCore`

The full HTR-10 core: Şeker & Çolak (2003)'s 13-ball bed of `layers`
layers with explicit TRISO, the explicit reflector and the ten rods
withdrawn, as
[`crate::htr10_rmc::core_model::assemble_explicit_triso`] builds it.
`layers = 12` is the bed every recorded `k` in
`verification_and_validation/htr10_seker_2026_10_07_10k/` is on.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `layers` | `usize` | Şeker layers N (bed height `9.798 N + 6` cm). |

###### `Cell`

One of the small cells of [`super::cells`] (added 2026-10-13,
gh:#849): the single pebble or a Şeker cluster, built from the core's
own particle and pebbles, reflective all round.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::cells::CellKind` |  |

##### Implementations

###### Methods

- ```rust
  pub fn describe(self: Self) -> String { /* ... */ }
  ```
  One-line description for the record.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `MgxsCase`

One named MGXS run. See the module docs: the fields are the run's
specification and are fixed in [`MgxsCase::all`].

```rust
pub struct MgxsCase {
    pub name: &'static str,
    pub purpose: &'static str,
    pub issue: &'static str,
    pub model: CaseModel,
    pub particles: usize,
    pub inactive: usize,
    pub active: usize,
    pub seed: u64,
    pub is_record: bool,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `&'static str` | Unique name; the record folder and the command-line argument. |
| `purpose` | `&'static str` | Why the case exists, in one sentence. |
| `issue` | `&'static str` | The GitHub issue the result feeds. |
| `model` | `CaseModel` | What is transported. |
| `particles` | `usize` | Particles per generation. |
| `inactive` | `usize` | Inactive (discarded) generations. |
| `active` | `usize` | Active (tallied) generations. |
| `seed` | `u64` | RNG seed; both passes use it. |
| `is_record` | `bool` | `false` for a check of the pipeline whose numbers are not a record<br>(its output goes to a scratch folder, not the V&V tree). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn all() -> Vec<MgxsCase> { /* ... */ }
  ```
  Every case, in the order they should be run.

- ```rust
  pub fn find(name: &str) -> Option<MgxsCase> { /* ... */ }
  ```
  The case named `name`, if there is one.

- ```rust
  pub fn temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Every material's temperature.

- ```rust
  pub fn histories_per_pass(self: &Self) -> u64 { /* ... */ }
  ```
  Histories one pass simulates, inactive generations included. A case

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Constant `RECORDED_K_N12_T300`

The ENDF/B-VIII.0 `k_eff` already recorded for the N = 12 core at
300.15 K, `(k, sigma)`: 10 000 × \[5 + 135\], seed 20 260 917
(`verification_and_validation/htr10_seker_2026_10_07_10k/results_table.md`).
A case on the same core and temperature should agree with it within
statistics; it uses a different seed and more inactive generations, so
it is an independent repeat, not a copy.

```rust
pub const RECORDED_K_N12_T300: (f64, f64) = _;
```

#### Constant `SEED`

The seed of the recorded cases (the date the registry was written).

```rust
pub const SEED: u64 = 20_261_012;
```

#### Constant `REPLICA_SEED`

A second, independent seed: the replica case's.

```rust
pub const REPLICA_SEED: u64 = 20_261_013;
```

#### Constant `RECORD_PARTICLES`

Particles per generation of a recorded case: the reference paper's own
10 000 (Li, Yu & Wei 2014), as the 2026-10-07 `k` record uses.

```rust
pub const RECORD_PARTICLES: usize = 10_000;
```

#### Constant `RECORD_INACTIVE`

Inactive generations of a recorded case.

20, not the `k` record's 5: a tally is only as good as the source it is
scored from, and the HTR-10's dominance ratio is close to 1, so the source
converges slowly. The record keeps the entropy trace so this can be
checked rather than assumed.

```rust
pub const RECORD_INACTIVE: usize = 20;
```

#### Constant `RECORD_ACTIVE`

Active generations of a recorded case (the `k` record's 135).

```rust
pub const RECORD_ACTIVE: usize = 135;
```

#### Constant `PILOT_PARTICLES`

Particles per generation of a cell pilot (`pilot_*`).

```rust
pub const PILOT_PARTICLES: usize = 2_000;
```

#### Constant `PILOT_INACTIVE`

Inactive generations of a cell pilot. A reflective cell has no leakage
mode to settle, so 10 suffices where the full core needs 20; the record's
entropy trace says whether it did.

```rust
pub const PILOT_INACTIVE: usize = 10;
```

#### Constant `PILOT_ACTIVE`

Active generations of a cell pilot.

```rust
pub const PILOT_ACTIVE: usize = 40;
```

## Module `cells`

# HTR-10 cells for the MGXS gate (gh:#849)

Two small CSG models built from the **same** particle, pebble and Şeker
tile as the full core, so multigroup constants condensed on them describe
the core's fuel:

- [`Htr10Cell::single_pebble`]: one explicit-TRISO fuel pebble and helium
  in a reflective cube of side 6 cm, the simplest cell for the gate.
- [`Htr10Cell::seker_cluster`]: Şeker & Çolak (2003)'s 13-ball tiles at
  the core's packing, fuel and dummy balls by the core's rule, in a
  reflective hexagonal prism one tile high.

The TRISO particle and its lattice are [`TrisoBuild`] and the pebbles of a
tile [`build_tile_universes`], both extracted from the core builder for
this module (2026-10-13); nothing is copied.

# The single pebble's packing is 0.524, not 0.61

One 6 cm ball in a 6 cm cube is simple cubic packing: `(pi/6) = 0.5236`
of the volume is pebble. The core's bed is 0.61 (Şeker's 0.6448 in the
interior). The single pebble is a deliberately simple cell. ~~Its helium
fraction is too high for the core, though helium barely interacts.~~
**CORRECTED 2026-10-13 (pilots, `verification_and_validation/htr10_mgxs_cells/`):**
the helium matters little, but every ball in this cell is fuel, while the
core's bed is 43 % dummy graphite balls. The cell is therefore
under-moderated. Its kernel's one-group Σa is 28 % below the Şeker
cluster's, and its `k_inf` is 6100 ± 536 pcm lower. Its constants do not
describe the core's fuel; it serves as the simplest MG-against-CE gate.

# The Şeker cluster's boundary, and why it is a superlattice hexagon

Top and bottom are exact under reflection: the planes cut the flower
balls through their centres, and the close-packed A-B-A stacking is
mirror-symmetric about an A plane. **The side faces are not**: a tile's
`Outer(k)` balls cross three of its faces and its neighbours' balls enter
through the other three, so a reflected tile face distorts those balls.
Periodic boundaries, which would be exact, are refused by
`outram-mc-libs` (gh:#259).

So the side boundary is the Wigner–Seitz hexagon of a **hexagonal
superlattice** of the tile lattice, with superlattice vector
`L = i a1 + j a2` (`a1`, `a2` the tile lattice vectors of
`htr10_rmc::bed::tile_xy`): `(i, j) = (1, 0), (2, 1), (3, 2)` for
`rings = 1, 2, 3`, holding `N = i^2 + i j + j^2 = 1, 7, 19` tiles' worth of
bed. That hexagon is the exact periodic unit of the bed's geometry, so
under periodic boundaries it would be exact; with reflective ones it is an
approximation that **must be measured**, by the convergence of `k_inf` and
the constants over N = 1, 7, 19, not assumed away. For N = 1 it is the tile
itself; for N = 7 and 19 it is rotated against the tiles (by 19.1° and
23.4°) and cuts through them. The ball fraction inside it is Şeker's
0.6448 for every N, because the hexagon is a unit cell of the tiling.

**What the convex hexagon replaces.** A boundary following the cluster's
own zig-zag outline would need reflective planes that also pass through
the cluster's interior, and a reflective surface reflects wherever it is
crossed. A convex prism is the only reflective boundary this CSG can
express without that error.

# Fuel and dummy balls

Every ball with a piece inside the prism takes part; they are ordered as
`SekerBed::assign` orders the core's (by layer, then `rho^2`, then angle,
then id), and the core's low-discrepancy rule makes a ball fuel when
`floor((n + 1) f) > floor(n f)`, with `f` the design's
`fuel_ball_fraction` (0.57 by default). A cell this small cannot hold
exactly 0.57 of its ball volume as fuel; [`Htr10Cell::fuel_balls`] and
[`Htr10Cell::dummy_balls`] report what it holds.

**Research, education and V&V only** (`RESPONSIBLE_USE.md`).

```rust
pub mod cells { /* ... */ }
```

### Types

#### Enum `CellKind`

Which cell: the identity a case or a record names.

```rust
pub enum CellKind {
    SinglePebble,
    SekerCluster {
        rings: usize,
    },
}
```

##### Variants

###### `SinglePebble`

One fuel pebble and helium in a reflective 6 cm cube.

###### `SekerCluster`

Şeker tiles in the reflective superlattice prism of `rings` = 1, 2 or 3
(1, 7 or 19 tiles' worth of bed).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `rings` | `usize` | 1, 2 or 3. |

##### Implementations

###### Methods

- ```rust
  pub fn describe(self: &Self) -> String { /* ... */ }
  ```
  A short description for logs and records.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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
#### Struct `Htr10Cell`

An assembled HTR-10 cell: the CSG geometry and what it holds.

Material indices are those of `htr10_rmc::materials::htr10_material_set`
(`htr10_rmc::core_model::mat`), so the same material and nuclide tables
as the core run on it. The root cell is delta-tracked with majorant
`majorant_index`, as the core's bed is, or surface-tracked when
`majorant_index == usize::MAX`.

```rust
pub struct Htr10Cell {
    pub kind: CellKind,
    pub geometry: outram_mc_libs::geometry::geometry::Geometry,
    pub half_extent: [f64; 3],
    pub volume_cm3: f64,
    pub triso_particles: usize,
    pub fuel_balls: usize,
    pub dummy_balls: usize,
    pub balls: Vec<([f64; 3], bool)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `CellKind` | Which cell. |
| `geometry` | `outram_mc_libs::geometry::geometry::Geometry` | The assembled geometry. |
| `half_extent` | `[f64; 3]` | Half-extents of the cell's bounding box \[cm\], centred on the origin. |
| `volume_cm3` | `f64` | Volume of the cell \[cm^3\]. |
| `triso_particles` | `usize` | Whole TRISO particles per fuel pebble. |
| `fuel_balls` | `usize` | Fuel balls with a piece inside the cell. |
| `dummy_balls` | `usize` | Dummy (all-graphite) balls with a piece inside the cell. |
| `balls` | `Vec<([f64; 3], bool)>` | Centre \[cm\] of every ball with a piece inside the cell, and whether<br>it is fuel: the identities the geometry was built from, kept so a<br>check can recount the cell's contents without going through the CSG. |

##### Implementations

###### Methods

- ```rust
  pub fn single_pebble() -> Self { /* ... */ }
  ```
  The single pebble at the default design, delta-tracked with majorant 0.

- ```rust
  pub fn single_pebble_with(design: &Htr10CoreDesign, majorant_index: usize) -> Self { /* ... */ }
  ```
  The single pebble of `design`: one fuel pebble (6 cm ball, the design's

- ```rust
  pub fn seker_cluster(rings: usize) -> Self { /* ... */ }
  ```
  The Şeker cluster of `rings` (1, 2 or 3) at the default design,

- ```rust
  pub fn seker_cluster_with(rings: usize, design: &Htr10CoreDesign, majorant_index: usize) -> Self { /* ... */ }
  ```
  The Şeker cluster of `rings` for `design`. See the module docs for the

- ```rust
  pub fn source_box(self: &Self) -> SourceBox { /* ... */ }
  ```
  The bounding box, for the initial fission source (rejection-sampled

- ```rust
  pub fn entropy_mesh(self: &Self) -> RegularMesh { /* ... */ }
  ```
  A 4 x 4 x 4 Shannon-entropy mesh over the bounding box.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
## Module `groups`

The group structures of the HTR-10 MGXS cases.

Every case tallies once, on [`tally_structure`], and collapses onto each
[`Collapse`] afterwards. That only works because every coarse edge is an
edge of the tally structure; [`Collapse::structure`] is built from the
tally structure's own values so that holds by construction.

```rust
pub mod groups { /* ... */ }
```

### Types

#### Enum `Collapse`

A structure the fine library is collapsed onto.

None of these is chosen yet: #851 picks one from the measured MG-vs-CE
miss. Every edge is a WIMS-69 edge, so each is an exact collapse.

```rust
pub enum Collapse {
    Two,
    Four,
    Eight,
    Fine,
}
```

##### Variants

###### `Two`

Thermal below 0.625 eV, fast above.

###### `Four`

0.625 eV, 9.118 keV, 0.821 MeV: the four-group split `dhoby-ghaut`'s
workbench offers, all WIMS-69 edges.

###### `Eight`

The eight-group structure of `examples/htr10_deterministic_vs_mc.rs`
(1e-5, 0.14, 0.625, 4, 29, 130, 9.12e3, 1.35e6, 2e7 eV) **snapped to
the nearest WIMS-69 edges**: 29 → 27.7, 130 → 148.728,
9.12e3 → 9118, 1.35e6 → 1.353e6 eV. The example's own edges do not
nest in WIMS-69, so its numbers are not on exactly this structure.

###### `Fine`

The tally structure itself, 70 groups.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short label for tables and file names.

- ```rust
  pub fn structure(self: Self) -> GroupStructure { /* ... */ }
  ```
  The structure, with every edge taken from [`tally_structure`]'s own

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `tally_structure`

**Attributes:**

- `MustUse { reason: None }`

The 70-group tally structure: NJOY's WIMS-69 edges (`ign = 9`, from the
`njoy-outram-park-fork` port of `gengpn`), ascending from 1e-5 eV, plus
[`TOP_EDGE_EV`].

WIMS-69 because it is a thermal-reactor structure with 45 groups below
4 eV, so the graphite thermal spectrum and the low resonances are
resolved, and because every structure in [`Collapse`] nests in it.

# Panics

Never in practice: `ign = 9` is a built-in table of the port, checked by
`tally_structure_is_wims69_plus_one` below.

```rust
pub fn tally_structure() -> crate::mgxs::GroupStructure { /* ... */ }
```

### Constants and Statics

#### Constant `TOP_EDGE_EV`

Top of the tally structure \[eV\]. WIMS-69 stops at 10 MeV; a fission
spectrum does not, so one group from 10 to 20 MeV is added rather than
letting those neutrons fall outside every bin.

```rust
pub const TOP_EDGE_EV: f64 = 2.0e7;
```

#### Constant `WIMS69_IGN`

NJOY's `ign` for the EPRI-CPM/WIMS 69-group structure (`groupr.f90`).

```rust
pub const WIMS69_IGN: i32 = 9;
```

## Module `record`

What a case run leaves behind, and how it is written and read back.

A [`CaseRecord`] is plain data so it can be serialised: temperatures and
times are `f64` in the unit their field name states (`_k`, `_s`), the
file-format exception to the crate's `uom` rule. The run API itself takes
and returns `uom` quantities ([`super::MgxsCase::temperature`]).

```rust
pub mod record { /* ... */ }
```

### Types

#### Struct `Provenance`

Where the run came from, supplied by the caller (the record cannot
discover its own commit).

```rust
pub struct Provenance {
    pub commit: String,
    pub ace_commit: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `commit` | `String` | `git rev-parse --short=10 HEAD` of this repository, `-dirty` if a<br>tracked file differed. |
| `ace_commit` | `String` | The same for the `reference-data/ace` submodule. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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
#### Struct `PassRecord`

One pass of a case.

```rust
pub struct PassRecord {
    pub pass: String,
    pub k: f64,
    pub k_sigma: f64,
    pub histories: u64,
    pub lost_locate: u64,
    pub leak_vacuum: u64,
    pub entropy: Vec<f64>,
    pub transport_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pass` | `String` | `"scalar"` or `"matrix"`. |
| `k` | `f64` | `k_eff`, mean over active generations. |
| `k_sigma` | `f64` | Its within-run 1σ. |
| `histories` | `u64` | Histories simulated, inactive generations included. |
| `lost_locate` | `u64` | Histories whose position no cell claimed. |
| `leak_vacuum` | `u64` | Neutrons that left through the vacuum boundary. |
| `entropy` | `Vec<f64>` | Shannon entropy of every generation's source \[bits\] (4 × 4 × 4 mesh<br>over the fissile region, ceiling 6 bits). |
| `transport_s` | `f64` | Wall-clock time of the transport call \[s\]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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
#### Struct `CollapsedSummary`

The fine library collapsed onto one structure.

```rust
pub struct CollapsedSummary {
    pub structure: String,
    pub n_groups: usize,
    pub k_inf: f64,
    pub max_balance_residual: f64,
    pub weighted_balance_residual: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `structure` | `String` | [`super::Collapse::label`]. |
| `n_groups` | `usize` | Groups. |
| `k_inf` | `f64` | Algebraic `k_inf` of the constants (production over absorption,<br>every zone, no leakage). Collapse conserves it, so it must be the same<br>on every structure; it is NOT the core's `k_eff`, which has leakage. |
| `max_balance_residual` | `f64` | Largest `|Sigma_a + Sigma_s,row - Sigma_t| / Sigma_t` over every zone<br>and visited group: the scalar/matrix consistency (#682's ν-scatter<br>excess shows up here). Dominated by the most sparsely visited bin,<br>so it reads large on a short run; quote the flux-weighted one. |
| `weighted_balance_residual` | `f64` | The same residual weighted by `Sigma_t phi` (the collision rate) over<br>every zone and group: what the bulk of the collisions see. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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
#### Struct `Check`

One pass/fail check, recorded whichever way it went.

```rust
pub struct Check {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | What was checked. |
| `passed` | `bool` | Whether it held. |
| `detail` | `String` | The numbers behind it. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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
#### Struct `CaseRecord`

Everything a case run produced.

```rust
pub struct CaseRecord {
    pub case: String,
    pub purpose: String,
    pub issue: String,
    pub model: String,
    pub temperature_k: f64,
    pub particles: usize,
    pub inactive: usize,
    pub active: usize,
    pub seed: u64,
    pub threads: usize,
    pub data_notes: Vec<String>,
    pub provenance: Provenance,
    pub hardware: String,
    pub data_s: f64,
    pub passes: Vec<PassRecord>,
    pub library: crate::mgxs::MgxsLibrary,
    pub collapsed: Vec<CollapsedSummary>,
    pub checks: Vec<Check>,
    pub total_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `case` | `String` | Case name ([`super::MgxsCase::name`]). |
| `purpose` | `String` | Why the case exists. |
| `issue` | `String` | Issue the result feeds. |
| `model` | `String` | Model description. |
| `temperature_k` | `f64` | Every material's temperature \[K\]. |
| `particles` | `usize` | Particles per generation. |
| `inactive` | `usize` | Inactive generations. |
| `active` | `usize` | Active generations. |
| `seed` | `u64` | Seed of both passes. |
| `threads` | `usize` | Threads the transport ran on (changes the wall clock only). |
| `data_notes` | `Vec<String>` | Nuclear data library and treatment notes. |
| `provenance` | `Provenance` | Commit of the code and of the ACE submodule. |
| `hardware` | `String` | Hardware headline (CPU, memory). |
| `data_s` | `f64` | Wall-clock time of nuclear-data processing \[s\]. |
| `passes` | `Vec<PassRecord>` | The scalar and matrix passes. |
| `library` | `crate::mgxs::MgxsLibrary` | The fine (tally-structure) library, ascending energy, one zone per<br>material in material-index order. |
| `collapsed` | `Vec<CollapsedSummary>` | The library collapsed onto each reported structure. |
| `checks` | `Vec<Check>` | Checks, passed or not. |
| `total_s` | `f64` | Wall-clock time of the whole case \[s\]. |

##### Implementations

###### Methods

- ```rust
  pub fn k(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  `k_eff` of the scalar pass, the case's eigenvalue.

- ```rust
  pub fn all_checks_passed(self: &Self) -> bool { /* ... */ }
  ```
  Whether every check passed.

- ```rust
  pub fn summary_csv_row(self: &Self) -> String { /* ... */ }
  ```
  One CSV row: the numbers to quote.

- ```rust
  pub fn readme_md(self: &Self) -> String { /* ... */ }
  ```
  The case's `README.md`: methodology and results, generated from the

- ```rust
  pub fn write(self: &Self, dir: &Path) -> Result<(), String> { /* ... */ }
  ```
  Write `record.json`, `summary.csv` and `README.md` into `dir`,

- ```rust
  pub fn read(dir: &Path) -> Result<Self, String> { /* ... */ }
  ```
  Read `record.json` back from a case folder.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private229::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private229::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

- **StructuralPartialEq**
- **Sync**
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

#### Function `write_index`

Rewrite `INDEX.md` in `root` from every `<root>/<case>/record.json`:
one table row per case, in case-name order, so the folder's state can be
read in one file. Folders without a record (a run in progress) are listed
as such.

# Errors

An unreadable `root`, or a `record.json` that does not parse (named in the
message, since a broken record should not be silently skipped).

```rust
pub fn write_index(root: &std::path::Path) -> Result<(), String> { /* ... */ }
```

## Module `run`

Running one case: data, model, two passes, condensation, collapse, checks.

```rust
pub mod run { /* ... */ }
```

### Types

#### Enum `CaseError`

Why a case could not produce a record.

```rust
pub enum CaseError {
    Data(String),
    Condense(crate::mgxs::MgxsError),
    NoThreads,
}
```

##### Variants

###### `Data`

The nuclear data could not be planned or loaded (a missing tape
names the submodule to fetch).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Condense`

The tallies could not be condensed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mgxs::MgxsError` |  |

###### `NoThreads`

A thread count of zero.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

- **Forget**
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
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
### Functions

#### Function `run_case`

Run `case` on `threads` pinned threads and return its record. Nothing is
written; [`CaseRecord::write`] does that.

`say` receives a short line at each stage and every tenth of a pass, for
a log; a caller that wants silence passes `|_| {}`.

# Errors

[`CaseError`] if the data cannot be loaded or the tallies cannot be
condensed. A run that completes always returns a record, even when its
checks fail: a failed check is a result to report, not an error.

```rust
pub fn run_case</* synthetic */ impl FnMut(&str) + Send: FnMut(&str) + Send>(case: &super::cases::MgxsCase, threads: usize, provenance: super::record::Provenance, say: impl FnMut(&str) + Send) -> Result<super::record::CaseRecord, CaseError> { /* ... */ }
```

### Re-exports

#### Re-export `CaseModel`

```rust
pub use cases::CaseModel;
```

#### Re-export `MgxsCase`

```rust
pub use cases::MgxsCase;
```

#### Re-export `Collapse`

```rust
pub use groups::Collapse;
```

#### Re-export `write_index`

```rust
pub use record::write_index;
```

#### Re-export `CaseRecord`

```rust
pub use record::CaseRecord;
```

#### Re-export `Provenance`

```rust
pub use record::Provenance;
```

#### Re-export `run_case`

```rust
pub use run::run_case;
```

#### Re-export `CaseError`

```rust
pub use run::CaseError;
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
- **Forget**
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
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private21::Error + ''static> { /* ... */ }
    ```

- **Forget**
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
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `Convergence`

Convergence criteria for the outer Picard loop.

~~`temperature_k: f64`, `k_eff: f64`~~ **CHANGED 2026-11-03 (gh:#1186):**
dimensioned (`uom`), as every public quantity of this crate must be.

```rust
pub struct Convergence {
    pub temperature: uom::si::f64::TemperatureInterval,
    pub k_eff: uom::si::f64::Ratio,
    pub max_iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `uom::si::f64::TemperatureInterval` | Stop when the fuel temperature moves less than this between outer<br>iterations (a temperature *difference*; positive). |
| `k_eff` | `uom::si::f64::Ratio` | Stop when `k_eff` moves less than this between outer iterations<br>(dimensionless; positive).<br><br>Setting this below the Monte Carlo statistical uncertainty is asking the<br>loop to converge on noise; it will burn iterations and stop on chance. |
| `max_iterations` | `usize` | Maximum outer iterations before giving up. |

##### Implementations

###### Methods

- ```rust
  pub fn new(temperature: TemperatureInterval, k_eff: Ratio, max_iterations: usize) -> Self { /* ... */ }
  ```
  Criteria of `temperature` (K) and `k_eff`, capped at `max_iterations`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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
    1 K and 1e-3 in `k`, capped at 20 outer iterations.

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
  pub fn new(geometry: Geometry, materials: Vec<Material>, nuclides: Vec<Nuclide>, source: SourceBox, thermal: LumpedThermal, power: Power, region_volume: Volume) -> Self { /* ... */ }
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
## Module `power_shape`

Power shapes and their mapping from Monte Carlo tallies onto thermal-hydraulic
cells, with the HTR-10 frames (gh:#1184).
# `power_shape` — where the fission power goes, from Monte Carlo to a TH mesh

gh:#1184 (epic gh:#1183). A closed set of **power shapes** (analytic,
tabulated or reconstructed from a Monte Carlo tally), and the one operation
every coupling needs: **put a total power onto a set of thermal-hydraulic
cells so that the cells' powers add up to that total, to round-off.**

## The shortest thing that works

```
use nee_soon::power_shape::{CellGeometry, Frame, FrameMap, PowerCells, PowerShape};
use uom::si::f64::{Length, Power};
use uom::si::length::centimeter;
use uom::si::power::megawatt;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

let cm = Length::new::<centimeter>;
// A chopped cosine over a 180 cm bed (MC frame: z = 0 at bed mid-height).
let shape = PowerShape::ChoppedCosine { centre: cm(0.0), extrapolated_height: cm(220.0) };
// Two R-Z ring cells, in the Monte Carlo frame.
let cells = PowerCells::new(
    Frame::Mc,
    FrameMap::new(cm(90.0)),
    vec![
        CellGeometry::Annulus { r: [cm(0.0), cm(90.0)], z: [cm(-90.0), cm(0.0)] },
        CellGeometry::Annulus { r: [cm(0.0), cm(90.0)], z: [cm(0.0), cm(90.0)] },
    ],
);
let q = shape.cell_power_density(&cells, Power::new::<megawatt>(10.0)).unwrap();
// Symmetric shape, symmetric cells: equal densities, 5 MW each.
assert!((q[0].get::<watt_per_cubic_meter>() / q[1].get::<watt_per_cubic_meter>() - 1.0).abs() < 1e-12);
```

## Module map

| item | what |
|---|---|
| [`PowerShape`] | the closed set of shapes: uniform, region lumps, axial cosine/sine, `J0 x cos`, Legendre-z, Zernike(-Legendre), spatial spherical harmonics, an R-Z table |
| [`PowerShape::cell_power_density`] | total power -> W/m^3 per cell, conserving the total to round-off |
| [`PowerShape::from_tally`], [`PowerShape::zernike_legendre_from_tallies`] | the shape a Monte Carlo `KappaFission` tally measured, with the expansion normalisations |
| [`McPowerTally`] | builders for those tallies (`rz_mesh`, `legendre_z`, `zernike`) |
| [`zernike_to_bessel`], [`BesselRadial`], [`BesselBoundary`] | a Zernike (`m = 0`) shape re-expressed in Fourier-Bessel `J0` modes (Neumann/Dini by default, Dirichlet on request); [`PowerShape::bessel_legendre`] makes the R-Z product |
| [`Frame`], [`FrameMap`] | the three coordinate frames of the HTR-10 chain: MC (cm, z up, 0 at bed mid-height), TECDOC-1694 (cm, z down, 0 at the full-core bed top) and GeN-Foam meshes (metres, TECDOC orientation) |
| [`CellGeometry`], [`PowerCells`] | the cells power is put on: exact rings (R-Z meshes), boxes, or centroid points (a general `FvMesh`) |
| [`PowerShape::write_rz_png`] | an R-Z picture of a shape, for a human to check |

## How the cell integrals are done (and how exact they are)

A cell receives `total * I_c / sum_k I_k / V_c`, with `I_c` the integral of
the shape over the cell. **The total is therefore conserved by
construction, to round-off, whatever the shape or the mesh.** The cell
integrals themselves are:

- **rings** ([`CellGeometry::Annulus`], what an R-Z mesh has): composite
  Gauss-Legendre in `r` (with the `r dr` weight) and `z`, split at every
  discontinuity the shape has in `r` and `z` (table and lump edges, the
  support of a chopped cosine or of an expansion), so piecewise-constant and
  polynomial shapes are integrated **exactly**; smooth ones (`J0 x cos`) are
  refined by doubling until two estimates agree to `1e-12` relative (at
  most 16 subdivisions per piece).
  An axisymmetric shape is integrated in azimuth exactly (`2 pi f`); a
  non-axisymmetric one (Zernike `m != 0`, spherical harmonics) by the
  periodic trapezoid rule, refined with the rest;
- **boxes**: the same composite Gauss in `x, y, z`, refined by doubling to
  `1e-10` or 4 subdivisions per axis (a cylindrical discontinuity inside a
  box converges only at first order);
- **points** (centroid and volume of a general cell): the midpoint rule.
  Not exact; use it only where the cell geometry is not known.

**Power outside every cell is not lost silently**:
[`PowerShape::captured_fraction`] reports what share of a shape's integral
over a support the cells hold, for shapes whose integral is known.

## What does not belong here

No transport and no thermal-hydraulics: this module turns a tally into a
shape and a shape into cell power densities. The Picard loops that use it
are [`crate::htr10_coupling`] (Monte Carlo against the TH) and
[`crate::direct_coupling`].

Status: untrusted AI-generated draft (2026-11-03), research, education and
V&V only (`RESPONSIBLE_USE.md`).

```rust
pub mod power_shape { /* ... */ }
```

### Types

#### Type Alias `PowerDensity`

Power per unit volume, W/m^3 (`uom`'s `VolumetricPowerDensity`): what a
thermal-hydraulic cell receives (GeN-Foam's `powerDensityNeutronics`).

```rust
pub type PowerDensity = uom::si::f64::VolumetricPowerDensity;
```

#### Enum `PowerShapeError`

Why a shape could not be built or put on cells.

```rust
pub enum PowerShapeError {
    NoPowerInCells(f64),
    UnsupportedTally(String),
    Input(String),
}
```

##### Variants

###### `NoPowerInCells`

The shape integrates to zero (or less) over the cells, so there is no
power distribution to normalise.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `UnsupportedTally`

A tally this module cannot read as a power shape.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Input`

An argument out of range.

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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: crate::power_shape::PowerShapeError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `Frame`

A coordinate frame of the HTR-10 chain. Every frame shares the core axis
(`x = y = 0`, `r` from the axis); they differ in `z`.

- **`Mc`**: the Monte Carlo model (`htr10_rmc`): `z` **up**, `z = 0` at the
  bed mid-height of the loading built (so it moves with the loading);
- **`Tecdoc`**: IAEA-TECDOC-1694's R-Z frame (`htr10_rz`): `z` **down**,
  `z = 0` at the full-core bed top, fixed hardware.

A GeN-Foam mesh built by this crate (an `htr10_rz` `RzMesh`) is in the
`Tecdoc` frame in metres: see [`FrameMap::genfoam_m_to_mc_cm`].

```rust
pub enum Frame {
    Mc,
    Tecdoc,
}
```

##### Variants

###### `Mc`

Monte Carlo: `z` up, `0` at the loading's bed mid-height.

###### `Tecdoc`

TECDOC-1694: `z` down, `0` at the full-core bed top.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `FrameMap`

The map between the frames for one Monte Carlo loading.

The bed floor is common hardware ([`TECDOC_BED_FLOOR_Z_CM`]); the Monte
Carlo bed spans `-h <= z <= h` with `h` its half-height, so
`z_tecdoc = 180 cm - (z_mc + h)`.

```rust
pub struct FrameMap {
    pub bed_half_height: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bed_half_height` | `uom::si::f64::Length` | Half-height of the Monte Carlo bed's cylindrical part (cm in the model;<br>`AssembledCore::bed_half_height`). Range: positive, at most 90 cm (the<br>full core). |

##### Implementations

###### Methods

- ```rust
  pub fn new(bed_half_height: Length) -> Self { /* ... */ }
  ```
  The map for a bed of half-height `bed_half_height`.

- ```rust
  pub fn from_core(core: &crate::htr10_rmc::core_model::AssembledCore) -> Self { /* ... */ }
  ```
  The map for an assembled Monte Carlo core.

- ```rust
  pub fn z_to_mc(self: &Self, frame: Frame, z: Length) -> Length { /* ... */ }
  ```
  `z` in the Monte Carlo frame of a `z` given in `frame`.

- ```rust
  pub fn z_from_mc(self: &Self, frame: Frame, z_mc: Length) -> Length { /* ... */ }
  ```
  `z` in `frame` of a Monte Carlo `z`.

- ```rust
  pub fn to_mc(self: &Self, frame: Frame, p: [Length; 3]) -> [Length; 3] { /* ... */ }
  ```
  A point given in `frame`, in the Monte Carlo frame.

- ```rust
  pub fn from_mc(self: &Self, frame: Frame, p: [Length; 3]) -> [Length; 3] { /* ... */ }
  ```
  A Monte Carlo point, in `frame`.

- ```rust
  pub fn genfoam_m_to_mc_cm(self: &Self, p: [f64; 3]) -> [f64; 3] { /* ... */ }
  ```
  A GeN-Foam mesh point (metres, TECDOC orientation: `z` down, `0` at

- ```rust
  pub fn mc_cm_to_genfoam_m(self: &Self, p: [f64; 3]) -> [f64; 3] { /* ... */ }
  ```
  A Monte Carlo point (cm) as GeN-Foam mesh coordinates (metres, TECDOC

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `CellGeometry`

One cell that receives power.

```rust
pub enum CellGeometry {
    Annulus {
        r: [uom::si::f64::Length; 2],
        z: [uom::si::f64::Length; 2],
    },
    Box {
        min: [uom::si::f64::Length; 3],
        max: [uom::si::f64::Length; 3],
    },
    Point {
        centre: [uom::si::f64::Length; 3],
        volume: uom::si::f64::Volume,
    },
}
```

##### Variants

###### `Annulus`

A full ring about the core axis: `r[0] <= r <= r[1]`, `z[0] <= z <=
z[1]` (`z` in the cells' frame; either order). Integrated exactly for
piecewise-polynomial shapes.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `[uom::si::f64::Length; 2]` | Inner and outer radius (`0 <= r[0] < r[1]`). |
| `z` | `[uom::si::f64::Length; 2]` | The two axial faces. |

###### `Box`

An axis-aligned box.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `min` | `[uom::si::f64::Length; 3]` | Lower corner. |
| `max` | `[uom::si::f64::Length; 3]` | Upper corner. |

###### `Point`

A cell known only by its centroid and volume (a general polyhedral
`FvMesh` cell): the midpoint rule, not exact.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `centre` | `[uom::si::f64::Length; 3]` | Centroid. |
| `volume` | `uom::si::f64::Volume` | Volume (positive). |

##### Implementations

###### Methods

- ```rust
  pub fn volume(self: &Self) -> Volume { /* ... */ }
  ```
  The cell's volume.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `PowerCells`

The cells a power is put on, in one frame.

```rust
pub struct PowerCells {
    pub frame: Frame,
    pub map: FrameMap,
    pub cells: Vec<CellGeometry>,
    pub mesh_cell: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `frame` | `Frame` | The frame the cells' coordinates are in. |
| `map` | `FrameMap` | The map to the Monte Carlo frame (where shapes live). |
| `cells` | `Vec<CellGeometry>` | The cells. |
| `mesh_cell` | `Vec<usize>` | The mesh cell index of each entry of [`Self::cells`] (its position in<br>the mesh it came from; `0..n` when built by hand). |

##### Implementations

###### Methods

- ```rust
  pub fn new(frame: Frame, map: FrameMap, cells: Vec<CellGeometry>) -> Self { /* ... */ }
  ```
  Cells built by hand; `mesh_cell` is `0..cells.len()`.

- ```rust
  pub fn from_rz_mesh</* synthetic */ impl Fn(usize) -> bool: Fn(usize) -> bool>(mesh: &RzMesh, map: FrameMap, keep: impl Fn(usize) -> bool) -> Self { /* ... */ }
  ```
  The rings of an `htr10_rz` [`RzMesh`] (TECDOC frame), keeping only

- ```rust
  pub fn from_fv_mesh(mesh: &FvMesh, frame: Frame, map: FrameMap) -> Self { /* ... */ }
  ```
  The cells of a general GeN-Foam [`FvMesh`] (coordinates in metres, in

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `ZoneLump`

One region lump of [`PowerShape::Zones`]: a ring `r[0]..r[1]`,
`z[0]..z[1]` (Monte Carlo frame) holding `fraction` of the power,
uniformly.

```rust
pub struct ZoneLump {
    pub r: [uom::si::f64::Length; 2],
    pub z: [uom::si::f64::Length; 2],
    pub fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `[uom::si::f64::Length; 2]` | Inner and outer radius. |
| `z` | `[uom::si::f64::Length; 2]` | Bottom and top, Monte Carlo frame. |
| `fraction` | `f64` | Share of the power, `>= 0` (shares are renormalised over the cells). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `RadialProfile`

The radial profile of [`PowerShape::SphericalHarmonics`], as a function of
`rho = |p - centre| / radius` in `[0, 1]`.

```rust
pub enum RadialProfile {
    Uniform,
    Polynomial(Vec<f64>),
    BareSphere,
}
```

##### Variants

###### `Uniform`

Constant in `rho`.

###### `Polynomial`

`sum_k a_k rho^k`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<f64>` |  |

###### `BareSphere`

`sin(pi rho) / (pi rho)`: the bare sphere's fundamental mode.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `PowerShape`

**Where the power goes**: a closed set of shapes, all in the Monte Carlo
frame ([`Frame::Mc`]: cm, `z` up, `0` at the bed mid-height).

A shape is a *relative* power density: only its variation matters, because
[`Self::cell_power_density`] normalises it to the total power over the
cells it is given. Where a shape has a support (a chopped cosine, an
expansion interval, a sphere), it is zero outside it.

```rust
pub enum PowerShape {
    Uniform,
    Zones(Vec<ZoneLump>),
    ChoppedCosine {
        centre: uom::si::f64::Length,
        extrapolated_height: uom::si::f64::Length,
    },
    Sine {
        bottom: uom::si::f64::Length,
        height: uom::si::f64::Length,
    },
    J0Cosine {
        centre: uom::si::f64::Length,
        extrapolated_radius: uom::si::f64::Length,
        extrapolated_height: uom::si::f64::Length,
    },
    LegendreZ {
        z_min: uom::si::f64::Length,
        z_max: uom::si::f64::Length,
        coefficients: Vec<f64>,
    },
    Zernike {
        centre: [uom::si::f64::Length; 2],
        radius: uom::si::f64::Length,
        coefficients: Vec<f64>,
    },
    ZernikeLegendre {
        centre: [uom::si::f64::Length; 2],
        radius: uom::si::f64::Length,
        z_min: uom::si::f64::Length,
        z_max: uom::si::f64::Length,
        coefficients: Vec<Vec<f64>>,
    },
    BesselLegendre {
        radius: uom::si::f64::Length,
        alphas: Vec<f64>,
        z_min: uom::si::f64::Length,
        z_max: uom::si::f64::Length,
        coefficients: Vec<Vec<f64>>,
    },
    SphericalHarmonics {
        centre: [uom::si::f64::Length; 3],
        radius: uom::si::f64::Length,
        coefficients: Vec<f64>,
        radial: RadialProfile,
    },
    RzTable {
        r_edges: Vec<uom::si::f64::Length>,
        z_edges: Vec<uom::si::f64::Length>,
        values: Vec<f64>,
    },
}
```

##### Variants

###### `Uniform`

Uniform over whatever cells are given.

###### `Zones`

Region lumps: each [`ZoneLump`] holds its share uniformly (the
per-vessel / per-region lumped description).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<ZoneLump>` |  |

###### `ChoppedCosine`

`cos(pi (z - centre) / H_e)` on `|z - centre| < H_e / 2`, radially
uniform: the chopped cosine of a reflected core.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `centre` | `uom::si::f64::Length` | Axial centre of the cosine. |
| `extrapolated_height` | `uom::si::f64::Length` | Extrapolated height `H_e` (positive). |

###### `Sine`

`sin(pi (z - bottom) / height)` on `bottom < z < bottom + height`,
radially uniform: the bare slab's fundamental mode.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `bottom` | `uom::si::f64::Length` | Where the sine starts. |
| `height` | `uom::si::f64::Length` | Its length (positive). |

###### `J0Cosine`

`J0(2.405 r / R_e) cos(pi (z - centre) / H_e)` inside `r < R_e`,
`|z - centre| < H_e / 2`: the bare finite cylinder's fundamental mode.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `centre` | `uom::si::f64::Length` | Axial centre. |
| `extrapolated_radius` | `uom::si::f64::Length` | Extrapolated radius `R_e` (positive). |
| `extrapolated_height` | `uom::si::f64::Length` | Extrapolated height `H_e` (positive). |

###### `LegendreZ`

`sum_n a_n P_n(xi)`, `xi = 2 (z - z_min)/(z_max - z_min) - 1`, on
`z_min <= z <= z_max`, radially uniform. `coefficients[n]` is `a_n` in
power per unit length (any unit; relative).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `z_min` | `uom::si::f64::Length` | Lower end of the expansion interval. |
| `z_max` | `uom::si::f64::Length` | Upper end (must exceed `z_min`). |
| `coefficients` | `Vec<f64>` | `a_0 ... a_N`. |

###### `Zernike`

`sum_j c_j Z_j(rho, theta)` on the disc `rho = |(x, y) - centre| /
radius <= 1`, axially uniform, in Monte Carlo filter order (`n =
0..`, `m = -n, -n+2, .., n`; [`zernike_zn`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `centre` | `[uom::si::f64::Length; 2]` | Disc centre `(x, y)`. |
| `radius` | `uom::si::f64::Length` | Disc radius (positive). |
| `coefficients` | `Vec<f64>` | `c_j` per unit area (relative). |

###### `ZernikeLegendre`

`sum_n sum_j c[n][j] P_n(xi(z)) Z_j(rho, theta)`: Legendre in `z`
times Zernike in `(r, theta)`, on the cylinder.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `centre` | `[uom::si::f64::Length; 2]` | Disc centre `(x, y)`. |
| `radius` | `uom::si::f64::Length` | Disc radius. |
| `z_min` | `uom::si::f64::Length` | Lower end of the axial interval. |
| `z_max` | `uom::si::f64::Length` | Upper end. |
| `coefficients` | `Vec<Vec<f64>>` | `c[n][j]`. |

###### `BesselLegendre`

**Fourier-Bessel in `r` times Legendre in `z`**:
`sum_n sum_k c[n][k] P_n(xi(z)) J0(alpha_k r / radius)` on `r <=
radius`, `z_min <= z <= z_max`, axisymmetric about the core axis.
The radial modes come from [`zernike_to_bessel`] (a Zernike tally
re-expressed in Bessel modes); [`Self::bessel_legendre`] builds the
product with a Legendre-z axial shape.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `uom::si::f64::Length` | The Bessel radius `R_b` (support and argument scale). |
| `alphas` | `Vec<f64>` | `alpha_k` (zeros of `J0` for Dirichlet, `0` and the zeros of `J1`<br>for Neumann). |
| `z_min` | `uom::si::f64::Length` | Lower end of the axial interval. |
| `z_max` | `uom::si::f64::Length` | Upper end. |
| `coefficients` | `Vec<Vec<f64>>` | `c[n][k]`. |

###### `SphericalHarmonics`

**Spatial** real spherical harmonics on a sphere:
`R(rho) sum_{l,m} c_lm Y_l^m(theta, phi)` for `rho = |p - centre| /
radius <= 1`, `theta` the polar angle from `+z`. `coefficients` in
`(l, m)` order, `m = -l..=l` ([`real_spherical_harmonics`]'s order,
orthonormal over the unit sphere).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `centre` | `[uom::si::f64::Length; 3]` | Sphere centre. |
| `radius` | `uom::si::f64::Length` | Sphere radius (positive). |
| `coefficients` | `Vec<f64>` | `c_lm`, `(order + 1)^2` of them. |
| `radial` | `RadialProfile` | The radial factor. |

###### `RzTable`

A piecewise-constant R-Z table: `values[i + n_r k]` on `r_edges[i]..
r_edges[i+1]`, `z_edges[k]..z_edges[k+1]` (Monte Carlo frame, both
ascending), zero outside.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `r_edges` | `Vec<uom::si::f64::Length>` | Ascending radial edges. |
| `z_edges` | `Vec<uom::si::f64::Length>` | Ascending axial edges. |
| `values` | `Vec<f64>` | Relative density per cell, `r` fastest. |

##### Implementations

###### Methods

- ```rust
  pub fn relative_density(self: &Self, p: [Length; 3]) -> f64 { /* ... */ }
  ```
  The relative power density at Monte Carlo point `p` (any unit; only

- ```rust
  pub fn is_axisymmetric(self: &Self) -> bool { /* ... */ }
  ```
  True when the shape does not depend on the azimuth about the core axis.

- ```rust
  pub fn cell_integral(self: &Self, cell: &CellGeometry, frame: Frame, map: &FrameMap) -> f64 { /* ... */ }
  ```
  The integral of the shape over one cell, in (relative units) x cm^3.

- ```rust
  pub fn cell_integrals(self: &Self, cells: &PowerCells) -> Vec<f64> { /* ... */ }
  ```
  The shape's integral over each cell, (relative units) x cm^3.

- ```rust
  pub fn cell_power_density(self: &Self, cells: &PowerCells, total: Power) -> Result<Vec<PowerDensity>, PowerShapeError> { /* ... */ }
  ```
  **Put `total` on `cells`**: each cell's power density, so that

- ```rust
  pub fn captured_fraction(self: &Self, cells: &PowerCells, r_max: Length, z: [Length; 2]) -> f64 { /* ... */ }
  ```
  Share of the shape's integral over the ring `r < r_max`, `z_lo < z <

- ```rust
  pub fn from_tally(tally: &Tally, n_active: usize) -> Result<Self, PowerShapeError> { /* ... */ }
  ```
  **The shape a Monte Carlo power tally measured** ([`McPowerTally`]):

- ```rust
  pub fn zernike_legendre_from_tallies(zernike: &Tally, legendre: &Tally, n_active: usize) -> Result<Self, PowerShapeError> { /* ... */ }
  ```
  **Zernike in `(r, theta)` times Legendre in `z`, from two tallies**

- ```rust
  pub fn bessel_legendre(radial: &BesselRadial, legendre: &[f64], z_min: Length, z_max: Length) -> Self { /* ... */ }
  ```
  **A Fourier-Bessel radial shape times a Legendre-z axial one**:

- ```rust
  pub fn write_rz_png</* synthetic */ impl AsRef<std::path::Path>: AsRef<std::path::Path>>(self: &Self, path: impl AsRef<std::path::Path>, title: &str, r_max: Length, z: [Length; 2], px_per_cm: f64) -> std::io::Result<()> { /* ... */ }
  ```
  **Draw the shape in R-Z** (the `x >= 0, y = 0` half-plane, Monte

- ```rust
  pub fn write_xy_png</* synthetic */ impl AsRef<std::path::Path>: AsRef<std::path::Path>>(self: &Self, path: impl AsRef<std::path::Path>, title: &str, z: Length, half_width: Length, px_per_cm: f64) -> std::io::Result<()> { /* ... */ }
  ```
  **Draw the shape on the plane `z` (Monte Carlo frame)**, `x, y` in

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Enum `BesselBoundary`

Which Fourier-Bessel series a radial shape is re-expressed in.

**The default is [`Self::Neumann`]**, because a Zernike reconstruction of
a reflected core's power is generally **non-zero at the disc edge**: the
Dini (Neumann) modes `J0(alpha_k r/R)`, `J1(alpha_k) = 0`, with the
constant `k = 0` term, carry a non-zero edge value and converge fast. The
Dirichlet modes all vanish at `R_b`, so with `R_b = R` they can only
approach a non-zero edge value slowly and with Gibbs ringing; they are
the bare cylinder's modes, an explicit choice for a bare (or
extrapolated, `R_b > R`) description.

```rust
pub enum BesselBoundary {
    Dirichlet {
        extrapolated_radius: uom::si::f64::Length,
    },
    Neumann,
}
```

##### Variants

###### `Dirichlet`

`alpha_k` the zeros of `J0` at the radius `R_b = extrapolated_radius
>= R` (the disc radius): `f(R_b) = 0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `extrapolated_radius` | `uom::si::f64::Length` | `R_b`, at least the disc radius. |

###### `Neumann`

`alpha_0 = 0` and `alpha_k` the zeros of `J1` (`f'(R) = 0`), at the
disc radius: the Dini series.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `BesselRadial`

A radial Fourier-Bessel series `f(r) = sum_k b_k J0(alpha_k r / R_b)` on
`r <= R_b`.

```rust
pub struct BesselRadial {
    pub radius: uom::si::f64::Length,
    pub alphas: Vec<f64>,
    pub coefficients: Vec<f64>,
    pub boundary: BesselBoundary,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `uom::si::f64::Length` | `R_b`. |
| `alphas` | `Vec<f64>` | `alpha_k`. |
| `coefficients` | `Vec<f64>` | `b_k` (the units of the Zernike shape it came from). |
| `boundary` | `BesselBoundary` | The series' boundary condition. |

##### Implementations

###### Methods

- ```rust
  pub fn eval(self: &Self, r: Length) -> f64 { /* ... */ }
  ```
  `f(r)` (zero beyond `R_b`).

- ```rust
  pub fn disc_integral(self: &Self, r_max: Length) -> f64 { /* ... */ }
  ```
  `int_0^{r_max} 2 pi r f(r) dr`, cm^2 times the shape's unit: exact

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `McPowerTally`

Builders of the Monte Carlo **power tallies** whose result
[`PowerShape::from_tally`] reads: each scores
[`ScoreType::KappaFission`] (fission rate times the single recoverable
`Q`, `outram_mc_libs::tally::scoring::Q_FISSION_J`), J per source
particle, through one spatial filter. All coordinates in the Monte Carlo
frame (cm, `z` up).

In a delta-tracked region (the HTR-10 bed) the tally is scored at the
tentative-collision sites (gh:#598); elsewhere by track length at the
segment midpoint, which is exact for a mesh bin only while the bin is
large against a mean free path.

```rust
pub struct McPowerTally;
```

##### Implementations

###### Methods

- ```rust
  pub fn rz_mesh(r_edges: &[Length], z_edges: &[Length]) -> Result<Tally, PowerShapeError> { /* ... */ }
  ```
  An R-Z mesh tally on rings `r_edges` x slabs `z_edges` (both

- ```rust
  pub fn legendre_z(order: usize, z_min: Length, z_max: Length) -> Result<Tally, PowerShapeError> { /* ... */ }
  ```
  A Legendre expansion in `z` of order `order` over `[z_min, z_max]`.

- ```rust
  pub fn zernike(order: usize, centre: [Length; 2], radius: Length) -> Result<Tally, PowerShapeError> { /* ... */ }
  ```
  A Zernike expansion of radial order `order` on the disc of `radius`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `bessel_zeros`

**Attributes:**

- `MustUse { reason: None }`

The first `n` positive zeros of `J0` (`order = 0`) or `J1` (`order = 1`),
by Newton from McMahon's asymptotic start `(k - 1/4) pi` / `(k + 1/4)
pi` (A&S 9.5.12), on petir's `J0`, `J1` (`J0' = -J1`, `J1' = J0 - J1/x`),
to round-off.

```rust
pub fn bessel_zeros(order: usize, n: usize) -> Vec<f64> { /* ... */ }
```

#### Function `zernike_to_bessel`

**Re-express a Zernike radial shape in Fourier-Bessel modes** (no Monte
Carlo Bessel filter: the tally stays Zernike, this is a projection).

`zernike` is a coefficient vector in filter order on a disc of
`disc_radius` ([`PowerShape::Zernike`]'s, e.g. from
[`PowerShape::from_tally`]); its `m = 0` part `f(r)` is projected on
`n_terms` modes with the Bessel orthogonality weight `r`:

- **Dirichlet** (`alpha_k` zeros of `J0`, `R_b >= R`; `f = 0` beyond
  `R`): `b_k = 2 / (R_b^2 J1(alpha_k)^2) int_0^{R_b} f(r) J0(alpha_k r /
  R_b) r dr` (Watson, *Treatise on the Theory of Bessel Functions*, 2nd
  ed., Sec 18.24; the norm `int_0^1 x J0(alpha x)^2 dx = J1(alpha)^2/2`,
  A&S 11.4.5);
- **Neumann / Dini** (`alpha_0 = 0`, `alpha_k` zeros of `J1`, `R_b =
  R`): `b_0 = (2/R^2) int_0^R f r dr`, `b_k = 2 / (R^2 J0(alpha_k)^2)
  int_0^R f J0(alpha_k r/R) r dr` (Watson Sec 18.33, the Dini series with
  `H = 0`; the norm is `J0(alpha)^2/2` where `J1(alpha) = 0`).

The integrand is a polynomial times `J0`, integrated by composite
30-point Gauss-Legendre refined until two estimates agree to 1e-14.

**Power**: with Neumann the disc integral is carried by `b_0` alone
(`int_0^R J0(alpha_k r/R) r dr = 0`), so it is conserved exactly at every
`n_terms >= 1`. With Dirichlet every mode carries some, and the
truncated series' integral converges to the Zernike one as `n_terms`
grows ([`BesselRadial::disc_integral`] measures it).

# Errors
`n_terms == 0`, a non-positive radius, a Dirichlet radius below the disc
radius, or a vector whose length is not a Zernike count.

```rust
pub fn zernike_to_bessel(zernike: &[f64], disc_radius: uom::si::f64::Length, n_terms: usize, boundary: BesselBoundary) -> Result<BesselRadial, PowerShapeError> { /* ... */ }
```

#### Function `zernike_disc_integral`

**Attributes:**

- `MustUse { reason: None }`

`int_0^R 2 pi r f(r) dr` of the `m = 0` Zernike profile (cm^2 times its
unit): `pi R^2 c_00`, the only moment with a disc integral.

```rust
pub fn zernike_disc_integral(zernike: &[f64], disc_radius: uom::si::f64::Length) -> f64 { /* ... */ }
```

#### Function `write_bessel_comparison_png`

**Picture: a Zernike radial profile against its Fourier-Bessel
reconstructions** with `n_terms` of each listed count, both boundaries.

# Errors
The conversion fails or the file cannot be written.

```rust
pub fn write_bessel_comparison_png</* synthetic */ impl AsRef<std::path::Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>, zernike: &[f64], disc_radius: uom::si::f64::Length, dirichlet_radius: uom::si::f64::Length, n_terms: &[usize]) -> Result<(), String> { /* ... */ }
```

### Constants and Statics

#### Constant `J0_FIRST_ZERO`

The first zero of `J0`, `2.404 825 557 695 773` (Abramowitz & Stegun
Tab 9.5): the radial buckling root of a bare cylinder.

```rust
pub const J0_FIRST_ZERO: f64 = 2.404_825_557_695_773;
```

#### Constant `TECDOC_BED_FLOOR_Z_CM`

TECDOC-1694 `z` of the bed floor (the top of the discharge cone), cm:
the full-core bed occupies `0 <= z <= 180` cm, `z` downward
(`outram_foam_appbuilder_lib::htr10_rz::geometry`, the `BED` rectangle
`0..180`). This is fixed hardware: the Monte Carlo model's bed floor is at
the same place for every loading, and its bed top moves with the loading.
Cross-check: the Monte Carlo core cavity is 221.818 cm above the floor
(`htr10_rmc::core_model::HTR10_CORE_CAVITY_CM`), TECDOC-1694 puts the top of
the core-entry plenum at `z = -40`, i.e. 220 cm above it (-0.8 %).

```rust
pub const TECDOC_BED_FLOOR_Z_CM: f64 = 180.0;
```

## Module `htr10_coupling`

HTR-10 Monte Carlo iterated directly against a thermal-hydraulic target
(lumped node or the `htr10_rz` R-Z core), with exact reprocessing (gh:#1186).
# `htr10_coupling` — HTR-10 Monte Carlo iterated directly against its thermal-hydraulics

gh:#1186 (epic gh:#1183). The whole-core HTR-10 Monte Carlo model
([`ParametricStudy`], the one the web demo and `htr10_parametric` use)
iterated by Picard against a thermal-hydraulic target until the zone
temperatures and `k` stop moving. The neutronics is the Monte Carlo itself;
no multigroup constants are involved (that chain is [`crate::mgxs_genfoam`]).

## The shortest thing that works

```no_run
use nee_soon::direct_coupling::Convergence;
use nee_soon::htr10_coupling::{Htr10DirectCoupling, ThermalTarget};
use nee_soon::htr10_rmc::parametric::{ParametricStudy, StudyBed};
use uom::si::f64::Power;
use uom::si::power::megawatt;

let study = ParametricStudy { bed: StudyBed::Seker { layers: 17 }, ..ParametricStudy::default() };
let target = ThermalTarget::htr10_rz_fpic().unwrap(); // the TECDOC-1694 R-Z core at FPIC
let mut coupling = Htr10DirectCoupling::new(study, target, Power::new::<megawatt>(10.0));
let state = coupling.solve(Convergence::default(), |line| println!("{line}")).unwrap();
println!("k = {:?} at fuel {:?}", state.k_eff, state.zone_temperatures.get(
    nee_soon::htr10_rmc::zones::Htr10Zone::FuelPebble));
```

## One Picard iteration

```text
  1. Monte Carlo at the current zone temperatures, with an R-Z KappaFission
     tally over the bed (McPowerTally::rz_mesh)              -> k, power shape
  2. power_shape: the tally as an R-Z table, put on the TH cells, the
     total power conserved to round-off
  3. the TH solve (lumped node or the htr10_rz core's steady state)
  4. power-weighted fuel and moderator temperatures, volume-averaged
     reflector / brick / rod temperatures, the mean gas    -> ZoneTemperatures
  5. under-relax (omega), round to the temperature quantum, and
     REPROCESS the nuclear data EXACTLY at the new temperatures, every
     (nuclide, temperature) job cached so a temperature reached again is
     not reprocessed; the bed majorant is rebuilt from the new nuclides,
     so it bounds them
```

**Why reprocessing and not `Material.temperature`.** HTR-10's pointwise
(NJOY) nuclides carry their data temperature; the transport does not
re-broaden them, so setting a material's temperature alone moves only the
free-gas kinematics and leaves the Doppler broadening and the `S(alpha,
beta)` laws where they were: a silent no-op for the feedback. Every
temperature here goes through [`ParametricStudy::plan`], which processes
each (nuclide, temperature) exactly and generates each bound law by LEAPR
at its zone's temperature.

## Module map

| item | what |
|---|---|
| [`Htr10DirectCoupling`] | the coupling: build, set relaxation / quantum / threads / power mesh, [`Htr10DirectCoupling::solve`] |
| [`ThermalTarget`] | the TH side, a closed enum: GeN-Foam's [`LumpedThermal`] node, or the `htr10_rz` R-Z core ([`RzTarget`]) |
| [`PicardIteration`], [`CoupledHtr10State`] | the history (k +/- sigma, zone temperatures in / from the TH / out, residuals, costs) and the result |
| [`write_history_png`] | the history as a picture |
| [`zone_temperatures_from_rz`], [`relax_and_quantise`] | the pieces, public so they can be checked alone |

## Known limitations, stated rather than discovered

- **Statistical noise is in the loop.** Each iteration's `k` and shape
  carry Monte Carlo noise; a `k` tolerance below the run's sigma will not
  be met except by chance ([`Convergence::k_eff`]).
- **The frames are tied at the bed floor** ([`crate::power_shape::FrameMap`]):
  a Monte Carlo loading shorter than the TECDOC full core (N < 18)
  heats only the lower part of the R-Z bed, and the R-Z model's flow and
  heat capacity are the full core's.
- **The Monte Carlo geometry is cold**: no thermal expansion; the helium
  density follows the coolant temperature at the model's pressure.
- **The conus and discharge tube hold no fuel** in the Monte Carlo model,
  so the tally covers the cylindrical bed only.

Status: untrusted AI-generated draft (2026-11-03); research, education and
V&V only, never operation or safety (`RESPONSIBLE_USE.md`).

```rust
pub mod htr10_coupling { /* ... */ }
```

### Types

#### Enum `Htr10CouplingError`

Why the coupled solve failed.

```rust
pub enum Htr10CouplingError {
    NotConverged {
        iterations: usize,
        last_dt_k: f64,
        last_dk: f64,
        history: Vec<PicardIteration>,
    },
    MonteCarlo(String),
    Power(crate::power_shape::PowerShapeError),
    Thermal(String),
}
```

##### Variants

###### `NotConverged`

The cap was reached; the residuals say whether it was close.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations performed. |
| `last_dt_k` | `f64` | Last largest zone-temperature residual, K. |
| `last_dk` | `f64` | Last `k` change. |
| `history` | `Vec<PicardIteration>` | The history so far. |

###### `MonteCarlo`

The Monte Carlo side failed (settings, data, transport).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Power`

The power could not be put on the TH cells.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::power_shape::PowerShapeError` |  |

###### `Thermal`

The thermal-hydraulic side failed.

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
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private21::Error + ''static> { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: crate::power_shape::PowerShapeError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(e: Htr10RzError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `RzTarget`

The `htr10_rz` R-Z core as a TH target, with its operating point.

```rust
pub struct RzTarget {
    pub core: outram_foam_appbuilder_lib::htr10_rz::Htr10RzTransientCore,
    pub operating_point: outram_foam_appbuilder_lib::htr10_rz::OperatingPoint,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `core` | `outram_foam_appbuilder_lib::htr10_rz::Htr10RzTransientCore` | The R-Z core (its power map is replaced every iteration). |
| `operating_point` | `outram_foam_appbuilder_lib::htr10_rz::OperatingPoint` | Inlet temperature, pressure and flow; its `power` is overwritten by<br>the coupling's. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Enum `ThermalTarget`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {lint_name: LintName { tool: Some(\"clippy\"), name: \"large_enum_variant\" },\nkind: Allow}])]")`

The thermal-hydraulic side of the coupling: a closed set.

```rust
pub enum ThermalTarget {
    Lumped {
        thermal: outram_foam_appbuilder_lib::genfoam::multi_region::outer_iteration::LumpedThermal,
        volume: uom::si::f64::Volume,
        time_step: uom::si::f64::Time,
    },
    Rz(RzTarget),
}
```

##### Variants

###### `Lumped`

GeN-Foam's lumped node ([`LumpedThermal`]), marched in pseudo-time to
its steady state each iteration. Its one temperature is given to the
fuel and the moderator pebbles; the other zones keep theirs.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `thermal` | `outram_foam_appbuilder_lib::genfoam::multi_region::outer_iteration::LumpedThermal` | The node. |
| `volume` | `uom::si::f64::Volume` | The heated volume (positive). |
| `time_step` | `uom::si::f64::Time` | Pseudo time step of the march (positive). |

###### `Rz`

The TECDOC-1694 R-Z core ([`Htr10RzTransientCore`]): the Monte Carlo
shape is put on its heated bed cells and its steady state solved.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RzTarget` |  |

##### Implementations

###### Methods

- ```rust
  pub fn htr10_rz_fpic() -> Result<Self, Htr10RzError> { /* ... */ }
  ```
  The R-Z core at the FPIC specification (10 MW, 250 °C, 3 MPa,

- ```rust
  pub fn htr10_rz_fpic_coarse() -> Result<Self, Htr10RzError> { /* ... */ }
  ```
  As [`Self::htr10_rz_fpic`] on the coarse Fig 2.5 mesh (21 x 39

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `PicardIteration`

One Picard iteration, kept so the approach can be seen.

```rust
pub struct PicardIteration {
    pub iteration: usize,
    pub zone_temperatures_in: crate::htr10_rmc::zones::ZoneTemperatures,
    pub k_eff: uom::si::f64::Ratio,
    pub k_std: uom::si::f64::Ratio,
    pub zone_temperatures_th: crate::htr10_rmc::zones::ZoneTemperatures,
    pub zone_temperatures_out: crate::htr10_rmc::zones::ZoneTemperatures,
    pub temperature_residual: uom::si::f64::TemperatureInterval,
    pub k_change: Option<uom::si::f64::Ratio>,
    pub peaking: uom::si::f64::Ratio,
    pub majorant_violations: u64,
    pub jobs_processed: usize,
    pub jobs_cached: usize,
    pub processing_time: uom::si::f64::Time,
    pub transport_time: uom::si::f64::Time,
    pub thermal_time: uom::si::f64::Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `iteration` | `usize` | 1-based. |
| `zone_temperatures_in` | `crate::htr10_rmc::zones::ZoneTemperatures` | The zone temperatures the Monte Carlo ran at. |
| `k_eff` | `uom::si::f64::Ratio` | `k_eff`. |
| `k_std` | `uom::si::f64::Ratio` | Its one-sigma statistical uncertainty. |
| `zone_temperatures_th` | `crate::htr10_rmc::zones::ZoneTemperatures` | The zone temperatures the TH solve returned (unrelaxed). |
| `zone_temperatures_out` | `crate::htr10_rmc::zones::ZoneTemperatures` | The next iteration's (relaxed, quantised). |
| `temperature_residual` | `uom::si::f64::TemperatureInterval` | Fixed-point residual: the largest `|T_th - T_in|` over the zones. |
| `k_change` | `Option<uom::si::f64::Ratio>` | `|k - k_previous|` (`None` at the first iteration). |
| `peaking` | `uom::si::f64::Ratio` | Peak over mean power density of the TH cells. |
| `majorant_violations` | `u64` | Collisions above the majorant (must be zero). |
| `jobs_processed` | `usize` | Processing jobs run this iteration. |
| `jobs_cached` | `usize` | Jobs taken from the cache. |
| `processing_time` | `uom::si::f64::Time` | Wall time processing nuclear data. |
| `transport_time` | `uom::si::f64::Time` | Wall time transporting. |
| `thermal_time` | `uom::si::f64::Time` | Wall time of the TH solve. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `CoupledHtr10State`

A converged coupled state.

```rust
pub struct CoupledHtr10State {
    pub zone_temperatures: crate::htr10_rmc::zones::ZoneTemperatures,
    pub k_eff: uom::si::f64::Ratio,
    pub k_std: uom::si::f64::Ratio,
    pub power_shape: crate::power_shape::PowerShape,
    pub history: Vec<PicardIteration>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `zone_temperatures` | `crate::htr10_rmc::zones::ZoneTemperatures` | The zone temperatures (the last Monte Carlo ran at these). |
| `k_eff` | `uom::si::f64::Ratio` | `k_eff` of the last iteration. |
| `k_std` | `uom::si::f64::Ratio` | Its one-sigma uncertainty. |
| `power_shape` | `crate::power_shape::PowerShape` | The last measured power shape (Monte Carlo frame). |
| `history` | `Vec<PicardIteration>` | Every iteration. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `Htr10DirectCoupling`

**The HTR-10 Monte Carlo iterated against a thermal-hydraulic target.**

Build with [`Self::new`], adjust with the `with_*` builders, then
[`Self::solve`]. See the module docs for the loop.

```rust
pub struct Htr10DirectCoupling {
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
  pub fn new(study: ParametricStudy, target: ThermalTarget, power: Power) -> Self { /* ... */ }
  ```
  The coupling of `study` (its temperatures are the starting guess) to

- ```rust
  pub fn with_relaxation(self: Self, omega: Ratio) -> Self { /* ... */ }
  ```
  Under-relaxation `omega` in `(0, 1]`: `T_next = T_in + omega (T_th -

- ```rust
  pub fn with_temperature_quantum(self: Self, quantum: TemperatureInterval) -> Self { /* ... */ }
  ```
  Round every zone temperature to a multiple of `quantum` (positive)

- ```rust
  pub fn without_temperature_quantum(self: Self) -> Self { /* ... */ }
  ```
  No rounding: every Picard temperature is processed as it comes (the

- ```rust
  pub fn with_threads(self: Self, threads: usize) -> Self { /* ... */ }
  ```
  Threads for processing and transport (at least one).

- ```rust
  pub fn with_power_mesh(self: Self, r_edges: Vec<Length>, z_edges: Vec<Length>) -> Self { /* ... */ }
  ```
  The Monte Carlo power mesh: ring edges and axial edges (Monte Carlo

- ```rust
  pub fn default_power_mesh(h: Length) -> (Vec<Length>, Vec<Length>) { /* ... */ }
  ```
  The default power mesh of a bed of half-height `h`: nine 10 cm rings

- ```rust
  pub fn study(self: &Self) -> &ParametricStudy { /* ... */ }
  ```
  The study (its temperatures are the last ones run).

- ```rust
  pub fn target(self: &Self) -> &ThermalTarget { /* ... */ }
  ```
  The thermal target in its current state.

- ```rust
  pub fn cached_jobs(self: &Self) -> usize { /* ... */ }
  ```
  Processed (nuclide, temperature) jobs held in the cache.

- ```rust
  pub fn solve</* synthetic */ impl FnMut(&str): FnMut(&str)>(self: &mut Self, criteria: Convergence, progress: impl FnMut(&str)) -> Result<CoupledHtr10State, Htr10CouplingError> { /* ... */ }
  ```
  **Iterate to a coupled steady state.** `progress` gets a line per

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

#### Function `relax_and_quantise`

**Attributes:**

- `MustUse { reason: None }`

**The next Picard temperatures**: `T_in + omega (T_th - T_in)` per zone,
rounded to the nearest multiple of `quantum` (none when the quantum is
zero) and kept inside the zones' range (20..=1800 °C).

```rust
pub fn relax_and_quantise(t_in: &crate::htr10_rmc::zones::ZoneTemperatures, t_th: &crate::htr10_rmc::zones::ZoneTemperatures, omega: uom::si::f64::Ratio, quantum: uom::si::f64::TemperatureInterval) -> crate::htr10_rmc::zones::ZoneTemperatures { /* ... */ }
```

#### Function `rz_zone_of`

**Attributes:**

- `MustUse { reason: None }`

The `htr10_rz` region's Monte Carlo temperature zone, by its region and
TECDOC `z` (cm): graphite above the Monte Carlo cavity top (`z <
180 - 221.818`) is the top reflector, below the bed floor (`z > 180`)
the bottom, else the side; the boronated bricks are their own zone.
Gas, steel and the bed are `None` (the bed's temperatures are
power-weighted separately; steel is outside the Monte Carlo model).

```rust
pub fn rz_zone_of(region: usize, z_tecdoc_cm: f64) -> Option<crate::htr10_rmc::zones::Htr10Zone> { /* ... */ }
```

#### Function `zone_temperatures_from_rz`

**Attributes:**

- `MustUse { reason: None }`

**The Monte Carlo zone temperatures an R-Z core state gives**:

- fuel and moderator pebbles: the core's power-weighted feedback
  temperatures ([`Htr10RzTransientCore::feedback`]);
- top, side and bottom reflector and the boronated bricks:
  volume-weighted means of their cells ([`rz_zone_of`]);
- control rods: the volume mean of the control-rod channel ring
  (`CR_RING`), where the withdrawn rods sit;
- the coolant is kept from `t_in` (the coupling sets it from the gas).

A zone with no cells keeps its `t_in` value.

```rust
pub fn zone_temperatures_from_rz(core: &outram_foam_appbuilder_lib::htr10_rz::Htr10RzTransientCore, t_in: &crate::htr10_rmc::zones::ZoneTemperatures) -> crate::htr10_rmc::zones::ZoneTemperatures { /* ... */ }
```

#### Function `write_history_png`

**The history as a PNG**: `k` with its one-sigma bars against the
iteration (left), and the fuel, moderator, side-reflector and coolant
temperatures the Monte Carlo ran at (right).

# Errors
The file cannot be written.

```rust
pub fn write_history_png</* synthetic */ impl AsRef<std::path::Path>: AsRef<std::path::Path>>(history: &[PicardIteration], path: impl AsRef<std::path::Path>) -> std::io::Result<()> { /* ... */ }
```

## Module `mgxs_genfoam`

HTR-10 Monte Carlo MGXS state points into a GeN-Foam R-Z diffusion core with
`TFuel` feedback (gh:#1187).
# `mgxs_genfoam` — HTR-10 Monte Carlo MGXS into a GeN-Foam R-Z diffusion core

gh:#1187 (epic gh:#1183). The second coupling chain: **Monte Carlo ->
multigroup constants -> GeN-Foam**. Four isothermal whole-core Monte Carlo
records (`htr10_mgxs` cases `core_n12_t{300.15, 600, 900, 1200}`) become one
temperature-parametrised `nuclearData`, homogenised into the regions of an
R-Z mesh that is the **`htr10_rz` mesh** (the TH model's own lines),
cropped to the Monte Carlo model and with each cell's region read from the
**assembled Monte Carlo geometry** at the cell.

## The shortest thing that works

```no_run
use nee_soon::mgxs_genfoam::{Htr10MgxsRegions, Htr10RzNeutronicsMesh, MgxsStatePoints};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

let points = MgxsStatePoints::htr10_records().unwrap();          // the four on-disk records
let mesh = Htr10RzNeutronicsMesh::for_records(&points).unwrap(); // R-Z mesh + zone_of_cell
let regions = Htr10MgxsRegions::new(&mesh);                    // rebalanced, descending, SI: all ON
let nd = regions.nuclear_data(&points).unwrap();               // xsVariables { TFuel log; }
let t = vec![ThermodynamicTemperature::new::<kelvin>(600.0); mesh.n_cells()];
let sol = mesh.solve_diffusion(&nd, &t).unwrap();
println!("k = {}", sol.k_eff);
```

## Module map

| item | what |
|---|---|
| [`MgxsStatePoints`] | the state points: one Monte Carlo MGXS library and `k` per fuel temperature |
| [`Htr10RzNeutronicsMesh`] | the R-Z mesh, `zone_of_cell`, the regions' recipes, its picture, the diffusion solve with per-cell `TFuel` |
| [`RegionRecipe`] | how one neutronics region's constants are made from the record's per-material constants |
| [`Htr10MgxsRegions`] | regions -> collapse -> rebalance -> descending order -> SI units -> stacked `NuclearDataInput`; each step ON by default, `without_*` to ablate |
| [`stack_states`] | single-state inputs -> one input with `xsVariables { TFuel <law>; }` (ported from `dhoby-ghaut`) |

## The traps, each handled by a default (and each ablation pinned by a test)

- **Rebalance** (`MgxsLibrary::rebalanced`): `Sigma_t = Sigma_a + sum
  Sigma_s` row by row. The HTR-10 record went from -17 133 to -353 pcm
  against the Monte Carlo `k_inf` with it. Ablation:
  [`Htr10MgxsRegions::without_rebalance`].
- **Group order**: GeN-Foam reads group 0 as the fastest; the records are
  tallied ascending, and the converter cannot tell. Ablation:
  [`Htr10MgxsRegions::without_descending_order`]. **Measured 2026-11-03:
  it does not move the steady `k`** (a consistent reversal of every
  group axis is a relabelling, 1.02789 both ways); it does make the
  inverse velocities `genfoam_xs` writes wrong, which matters for a
  transient.
- **Units**: the records are cm; GeN-Foam is SI (`Sigma` x 100, `D` x
  0.01). Ablation: [`Htr10MgxsRegions::without_unit_conversion`].
- **One zone for the whole core** costs about -27 000 pcm: the regions
  here are the bed, the fuel-free pebble bed of the cone and the discharge
  tube, the cavity, and **one region per Monte Carlo reflector material**
  (each TECDOC-1382 zone composition sits where the Monte Carlo puts it).

## Assumptions, stated

- **The bed's helium flux.** The records tally per *material*, and helium
  fills both the bed's voids and the cavity above it, so the bed's helium
  flux cannot be separated from the cavity's. The bed region is therefore
  built by **volume mixing**: the solids' flux-weighted constants times
  their volume fraction `f` (sampled from the assembled geometry), plus the
  helium's times `1 - f`, i.e. the flux per unit volume is assumed equal in
  the pebbles and in the helium between them. The cone and the discharge
  tube (dummy balls only) are built the same way from the pebble graphite.
  The pebble-graphite constants are flux-weighted over every pebble's
  graphite, fuelled bed and cone alike.
- **P0 diffusion**: `D = 1/(3 Sigma_t)`, no transport correction (no P1
  matrix was tallied); in graphite that under-states `D` by about
  `mu_bar = 2/(3A) = 5.6 %`.
- **The cavity** (helium at 1 atm, `D` of order 10^2 m) is a diffusion
  region: diffusion cannot represent streaming across it.
- **Withdrawn rods** are not regions of their own: a cell's region is the
  material most of its sample points hit.
- **Boundaries**: Marshak vacuum (GeN-Foam albedo `gamma = 0.5`) on the
  outer radius, top and bottom of the Monte Carlo model; the axis
  reflective.

Status: untrusted AI-generated draft (2026-11-03), research, education
and V&V only (`RESPONSIBLE_USE.md`).

```rust
pub mod mgxs_genfoam { /* ... */ }
```

### Types

#### Enum `MgxsGenfoamError`

What went wrong.

```rust
pub enum MgxsGenfoamError {
    StatePoints(String),
    Constants(String),
    Mesh(String),
    Diffusion(String),
}
```

##### Variants

###### `StatePoints`

A record could not be read or the state points disagree.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Constants`

The constants could not be built.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Mesh`

The mesh could not be built.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Diffusion`

The diffusion solve failed.

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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `MgxsStatePoint`

One isothermal Monte Carlo MGXS state point.

```rust
pub struct MgxsStatePoint {
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub library: crate::mgxs::MgxsLibrary,
    pub k_eff: uom::si::f64::Ratio,
    pub k_std: uom::si::f64::Ratio,
    pub layers: usize,
    pub case: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | The temperature every material was at. |
| `library` | `crate::mgxs::MgxsLibrary` | The per-material library, as tallied (ascending groups, cm units). |
| `k_eff` | `uom::si::f64::Ratio` | The Monte Carlo `k_eff` of the run. |
| `k_std` | `uom::si::f64::Ratio` | Its one-sigma uncertainty. |
| `layers` | `usize` | Şeker layers of the run's bed (12 for every `core_n12_*` record). |
| `case` | `String` | The case name. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `MgxsStatePoints`

**The state points GeN-Foam interpolates between**: one MGXS library per
fuel temperature, ascending in temperature, the first the reference.

```rust
pub struct MgxsStatePoints {
    pub points: Vec<MgxsStatePoint>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<MgxsStatePoint>` | The points, ascending in temperature. |

##### Implementations

###### Methods

- ```rust
  pub fn records_root() -> PathBuf { /* ... */ }
  ```
  The folder holding the `htr10_mgxs` case records.

- ```rust
  pub fn htr10_records() -> Result<Self, MgxsGenfoamError> { /* ... */ }
  ```
  **The four isothermal HTR-10 records** ([`HTR10_ISOTHERMAL_RECORDS`]).

- ```rust
  pub fn from_record_dirs(dirs: &[PathBuf]) -> Result<Self, MgxsGenfoamError> { /* ... */ }
  ```
  State points from `htr10_mgxs` case folders (each with `record.json`).

- ```rust
  pub fn new(points: Vec<MgxsStatePoint>) -> Result<Self, MgxsGenfoamError> { /* ... */ }
  ```
  From points in any order; sorted ascending in temperature.

- ```rust
  pub fn layers(self: &Self) -> usize { /* ... */ }
  ```
  Şeker layers of the bed every point was run on.

- ```rust
  pub fn slot_has_flux(self: &Self, s: usize) -> bool { /* ... */ }
  ```
  Whether material slot `s` carries flux in every group of the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Enum `RegionRecipe`

How one neutronics region's constants are made from a per-material
library (fine groups, before collapse).

```rust
pub enum RegionRecipe {
    Bed {
        solid_fraction: f64,
    },
    DummyBed {
        solid_fraction: f64,
    },
    Cavity,
    Slot(usize),
}
```

##### Variants

###### `Bed`

The fuelled bed: the pebbles' solids (kernel, coatings, pebble
graphite) flux-weighted together, volume-mixed with helium at
`solid_fraction` (module docs).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `solid_fraction` | `f64` | Volume fraction of solids in the bed, `(0, 1)`. |

###### `DummyBed`

The cone and discharge tube: pebble graphite volume-mixed with helium.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `solid_fraction` | `f64` | Volume fraction of graphite, `(0, 1)`. |

###### `Cavity`

The helium of the cavity (and of any cell mostly gas).

###### `Slot`

One Monte Carlo material slot's own constants (a reflector zone).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Methods

- ```rust
  pub fn build(self: &Self, lib: &MgxsLibrary, name: &str) -> Result<ZoneMgxs, MgxsGenfoamError> { /* ... */ }
  ```
  The region's constants from `lib` (one zone per material slot), named

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `Htr10MgxsRegions`

**The constants pipeline**, every trap handled by default:
regions ([`RegionRecipe`]) -> collapse to `collapse` (4 groups default)
-> rebalance -> descending energy -> SI units -> one `NuclearDataInput`
with `xsVariables { TFuel log; }`.

```rust
pub struct Htr10MgxsRegions {
    pub regions: Vec<(String, RegionRecipe)>,
    pub collapse: crate::htr10_mgxs::Collapse,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `regions` | `Vec<(String, RegionRecipe)>` | The regions, in zone order: `(name, recipe)`. |
| `collapse` | `crate::htr10_mgxs::Collapse` | The coarse structure (an exact collapse of the tally's WIMS-69 + 1). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: &Htr10RzNeutronicsMesh) -> Self { /* ... */ }
  ```
  The regions of `mesh`, with every default ON and 4 groups.

- ```rust
  pub fn from_regions(regions: Vec<(String, RegionRecipe)>) -> Self { /* ... */ }
  ```
  Given regions (zone order), every default ON, 4 groups.

- ```rust
  pub fn with_collapse(self: Self, c: Collapse) -> Self { /* ... */ }
  ```
  Collapse to `c` instead of four groups.

- ```rust
  pub fn with_law(self: Self, law: VariableLaw) -> Self { /* ... */ }
  ```
  The `TFuel` interpolation law (default `log`, GeN-Foam's for Doppler).

- ```rust
  pub fn without_rebalance(self: Self) -> Self { /* ... */ }
  ```
  **Ablation**: keep the tallied `Sigma_t` (no `Sigma_t = Sigma_a +

- ```rust
  pub fn without_descending_order(self: Self) -> Self { /* ... */ }
  ```
  **Ablation**: hand GeN-Foam the tally's ascending group order (it reads

- ```rust
  pub fn without_unit_conversion(self: Self) -> Self { /* ... */ }
  ```
  **Ablation**: hand GeN-Foam the cm constants as if they were SI.

- ```rust
  pub fn defaults(self: &Self) -> (bool, bool, bool) { /* ... */ }
  ```
  Whether the rebalance, the descending order and the SI conversion are

- ```rust
  pub fn library(self: &Self, lib: &MgxsLibrary) -> Result<MgxsLibrary, MgxsGenfoamError> { /* ... */ }
  ```
  The regions' constants at one state point: built, collapsed, and

- ```rust
  pub fn single_state(self: &Self, lib: &MgxsLibrary) -> Result<NuclearDataInput, MgxsGenfoamError> { /* ... */ }
  ```
  One state point as a single-state GeN-Foam input (SI unless

- ```rust
  pub fn nuclear_data(self: &Self, points: &MgxsStatePoints) -> Result<NuclearDataInput, MgxsGenfoamError> { /* ... */ }
  ```
  **Every state point stacked** into one input, `xsVariables { TFuel

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `Htr10RzNeutronicsMesh`

**The R-Z neutronics mesh**: the `htr10_rz` mesh lines (Fig 2.5), cropped
to the Monte Carlo model (`r <= 190` cm, its top and bottom) with lines
added at the Monte Carlo bed top, cavity top and model ends, cells no
larger than `max_cell`. Each cell's region comes from sampling the
**assembled** Monte Carlo geometry over the cell (module docs). TECDOC
frame (`z` down, metres in the mesh).

```rust
pub struct Htr10RzNeutronicsMesh {
    pub mesh: outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solid_conduction::RzMesh,
    pub zone_of_cell: Vec<usize>,
    pub regions: Vec<(String, RegionRecipe)>,
    pub frame: crate::power_shape::FrameMap,
    pub solid_fractions: (f64, f64),
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solid_conduction::RzMesh` | The mesh. |
| `zone_of_cell` | `Vec<usize>` | Region (GeN-Foam zone) of each cell. |
| `regions` | `Vec<(String, RegionRecipe)>` | The regions, in zone order. |
| `frame` | `crate::power_shape::FrameMap` | The frame map of the Monte Carlo loading. |
| `solid_fractions` | `(f64, f64)` | The sampled solid fraction of the fuelled bed and of the cone/tube. |

##### Implementations

###### Methods

- ```rust
  pub fn for_records(points: &MgxsStatePoints) -> Result<Self, MgxsGenfoamError> { /* ... */ }
  ```
  The mesh for the bed the state points were run on (20 cm cells).

- ```rust
  pub fn build(core: &AssembledCore, present: &[bool], max_cell: Length) -> Result<Self, MgxsGenfoamError> { /* ... */ }
  ```
  The mesh for `core` (the record model's, non-zoned slots). `present[s]`

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Cells of the mesh.

- ```rust
  pub fn flux_boundary(self: &Self) -> Vec<BoundaryCondition<f64>> { /* ... */ }
  ```
  The flux boundary conditions, one per patch: zero gradient on the

- ```rust
  pub fn solve_diffusion(self: &Self, nd: &NuclearDataInput, t_fuel: &[ThermodynamicTemperature]) -> Result<RzDiffusionSolution, MgxsGenfoamError> { /* ... */ }
  ```
  **The diffusion `k` and power shape at per-cell fuel temperatures**

- ```rust
  pub fn write_png</* synthetic */ impl AsRef<Path>: AsRef<Path>>(self: &Self, path: impl AsRef<Path>, px_per_cm: f64) -> std::io::Result<()> { /* ... */ }
  ```
  **Draw the mesh's regions** (R-Z, TECDOC frame: the top of the picture

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `RzDiffusionSolution`

A diffusion eigenvalue on the R-Z mesh.

```rust
pub struct RzDiffusionSolution {
    pub k_eff: f64,
    pub outer_iterations: usize,
    pub converged: bool,
    pub power_density: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_eff` | `f64` | `k_eff` (dimensionless). |
| `outer_iterations` | `usize` | Outer iterations. |
| `converged` | `bool` | Whether the outer loop converged. |
| `power_density` | `Vec<f64>` | Power density per cell, W/m^3 at GeN-Foam's arbitrary normalisation<br>(scale to a total power as needed). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

#### Function `mix_by_volume`

**Attributes:**

- `MustUse { reason: None }`

**Volume mixing** of zones with equal flux per unit volume: every cross
section is `sum_i f_i Sigma_i`; the flux spectrum is the volume-weighted
sum of each component's normalised spectrum; `chi` is weighted by each
component's fission source. Standard errors are dropped (the mixture's
are not those of any tally).

```rust
pub fn mix_by_volume(parts: &[(&crate::mgxs::ZoneMgxs, f64)]) -> crate::mgxs::ZoneMgxs { /* ... */ }
```

#### Function `stack_states`

**Attributes:**

- `MustUse { reason: None }`

**Stack single-state inputs** into one with `xsVariables { TFuel <law>;
}`: the state at `reference` K is GeN-Foam's `reference` state, the others
follow named `T<K>K`, each carrying its `TFuel`.

Ported from `dhoby-ghaut`'s `src/bin/dhoby-ghaut/mgxs_run.rs::stack_states`
(gh:#573), which cannot be imported (dhoby-ghaut depends on this crate).

```rust
pub fn stack_states(inputs: &[(f64, outram_foam_appbuilder_lib::genfoam::neutronics::xs::input::NuclearDataInput)], reference: f64, law: outram_foam_appbuilder_lib::genfoam::neutronics::xs::variables::VariableLaw) -> outram_foam_appbuilder_lib::genfoam::neutronics::xs::input::NuclearDataInput { /* ... */ }
```

### Constants and Statics

#### Constant `HTR10_ISOTHERMAL_RECORDS`

The four isothermal full-core records (`htr10_mgxs` gh:#845), in
ascending temperature.

```rust
pub const HTR10_ISOTHERMAL_RECORDS: [&str; 4] = _;
```

## Module `system_coupling`

# Coupling a 1-D system code to point kinetics (gh:#1084)

> Research, education and V&V only. AI draft, unreviewed.

The RELAP-7 Theory Manual §8 (Fig 7; cited, not quoted) lays out a system
code exchanging node temperatures and powers with a kinetics code. This is
that interface for OUTRAM PARK: a 1-D loop (`tampines`, `tuas`) on one
side, `teh-o-prke`'s point kinetics on the other, and nothing in
`tampines` depending on either.

## What it reuses

- **Feedback:** GeN-Foam's [`ReactivityFeedback`] (Doppler, expansion and
  density terms, importance-weighted) evaluated on a [`CouplingRegion`]
  whose cells are the loop's core nodes. The loop's node temperatures are
  written into the region's fields and the port computes the reactivity;
  nothing is re-implemented.
- **Kinetics:** `teh_o_prke`'s [`PointKinetics`], advanced by its backward
  Euler step.
- **Outer iteration:** GeN-Foam's `MultiPhysicsSolver` loop dispatches over
  a closed `RegionModel` enum of its own region kinds, so a system code
  cannot be added to it without that crate depending on one; the same
  Picard pattern (repeat the step until the inter-code residual is below a
  tolerance) is applied here, by [`LoopKineticsCoupling::coupled_step`].

## The contract

The thermal-hydraulic side implements [`SystemCodeSide`], a compile-time
contract (never used as `dyn`): it reports node fields, takes node powers,
and advances. It must be `Clone` so a Picard pass can restart the step.

Each coupled step, iteration `k`:
1. write the side's end-of-step node fields (the start state on `k = 0`)
   into the region and evaluate `ρ = ρ_ext + ρ_fb`;
2. advance the kinetics from the saved start by `dt` with that `ρ`
   (backward Euler, so `ρ` is the end-of-step reactivity);
3. restore the side, give it `P f_i` per node, advance `dt`;
4. stop when `|P_k − P_{k−1}|/P_k < tol`.

With `max_iterations = 1` this is the loose (operator-split) coupling. At
convergence it is the fully implicit backward-Euler solution of the joint
system, which is what the V&V compares against a monolithic solve.

```rust
pub mod system_coupling { /* ... */ }
```

### Types

#### Enum `SystemCouplingError`

Errors of the system coupling.

```rust
pub enum SystemCouplingError {
    InvalidInput(String),
    Kinetics(String),
    Feedback(String),
    SystemCode(String),
    NotConverged {
        iterations: usize,
        residual: f64,
    },
}
```

##### Variants

###### `InvalidInput`

Inconsistent inputs.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Kinetics`

The kinetics step failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Feedback`

The feedback evaluation failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `SystemCode`

The system-code side failed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NotConverged`

The Picard iteration did not converge.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Iterations taken. |
| `residual` | `f64` | Last residual. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
#### Struct `PicardControl`

Picard controls for [`LoopKineticsCoupling::coupled_step`].

```rust
pub struct PicardControl {
    pub max_iterations: usize,
    pub tolerance: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iterations` | `usize` | Iterations per step (1 = loose coupling). |
| `tolerance` | `f64` | Relative power change at which the step is converged. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `CoupledStepReport`

The report of one coupled step.

```rust
pub struct CoupledStepReport {
    pub iterations: usize,
    pub residual: f64,
    pub reactivity: f64,
    pub power: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Picard iterations taken. |
| `residual` | `f64` | Last relative power change (0 after one iteration). |
| `reactivity` | `f64` | End-of-step reactivity. |
| `power` | `f64` | End-of-step power, W. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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
#### Struct `LoopKineticsCoupling`

Point kinetics coupled to a system code's core nodes.

```rust
pub struct LoopKineticsCoupling {
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
  pub fn new(kinetics: PointKinetics, generation_time: Time, p0: Power, node_volumes: &[f64], power_fractions: &[f64], feedback: ReactivityFeedback) -> Result<Self, SystemCouplingError> { /* ... */ }
  ```
  Kinetics at critical equilibrium at power `p0`, over core nodes of

- ```rust
  pub fn set_external_reactivity(self: &mut Self, rho: Ratio) { /* ... */ }
  ```
  Set the external (inserted) reactivity, absolute.

- ```rust
  pub fn power(self: &Self) -> Power { /* ... */ }
  ```
  The current power.

- ```rust
  pub fn node_powers(self: &Self) -> Vec<f64> { /* ... */ }
  ```
  The current node powers, W.

- ```rust
  pub fn reactivity_of<S: SystemCodeSide>(self: &mut Self, side: &S) -> Result<f64, SystemCouplingError> { /* ... */ }
  ```
  The reactivity for `side`'s current node fields: external plus

- ```rust
  pub fn coupled_step<S: SystemCodeSide>(self: &mut Self, side: &mut S, dt: Time, control: PicardControl) -> Result<CoupledStepReport, SystemCouplingError> { /* ... */ }
  ```
  One coupled step of `dt` (see the module docs).

- ```rust
  pub fn kinetics(self: &Self) -> &PointKinetics { /* ... */ }
  ```
  The kinetics (for its constants and state).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
### Traits

#### Trait `SystemCodeSide`

What a 1-D system code offers the coupling (a compile-time contract).

```rust
pub trait SystemCodeSide: Clone {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `node_field`: The value of node field `name` (e.g. `"TFuel"`, K) at every core node,
- `advance`: Advance `dt` with `node_powers` (W) deposited at the core nodes.

### Functions

#### Function `node_mesh`

A 1-D line mesh of `volumes.len()` cells, for a [`CouplingRegion`] whose
cells are loop nodes.

# Errors

When there are no cells or a volume is not positive.

```rust
pub fn node_mesh(volumes: &[f64]) -> Result<std::sync::Arc<outram_foam_basic_lib::mesh::fv_mesh::FvMesh>, SystemCouplingError> { /* ... */ }
```

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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
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

- **CastableFrom**
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

- **Error**
- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(e: ExportError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- **CastableFrom**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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

- ```rust
  pub fn new_loop_kinetics_coupling(self: &Self, kinetics: teh_o_prke::zero_power_prke::n_group_prke::PointKinetics, generation_time: Time, initial_power: Power, node_volumes: &[f64], power_fractions: &[f64], feedback: outram_foam_appbuilder_lib::genfoam::multi_region::ReactivityFeedback) -> Result<system_coupling::LoopKineticsCoupling, system_coupling::SystemCouplingError> { /* ... */ }
  ```
  Creates the coupling between a 1-D system code's core nodes and point

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

- **Forget**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Move**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
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
## Re-exports

### Re-export `NordheimFuchsExactTimestepper`

```rust
pub use teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper;
```

### Re-export `TehOPrkeError`

```rust
pub use teh_o_prke::teh_o_prke_error::TehOPrkeError;
```

