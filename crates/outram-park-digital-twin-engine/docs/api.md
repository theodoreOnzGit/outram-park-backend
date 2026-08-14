# Crate Documentation

**Version:** 0.0.1

**Format Version:** 60

# Module `outram_park_digital_twin_engine`

# outram-park-digital-twin-engine

Reusable visualization framework for OUTRAM PARK digital twins.

Provides visual process objects (Pipe, Pump, Valve, HeatExchanger, Steam
Generator, Turbine, Condenser, Cooling Tower, Reactor Vessel,
Instrumentation) whose rendering derives directly from physics state:
cell count drives displayed cells, temperature drives cell colour, mass
flow drives tracer direction, residence time drives tracer travel time.

## Design philosophy

Avoid separating physics and rendering unnecessarily. Each visual
component bundles physics state (from [`tampines`]/[`nee_soon`]),
its visual representation, and its animation logic together, rather than
maintaining a physics model and a separate rendering model that must be
kept in sync by hand.

## What it composes

| Piece | Provided by | Role |
|---|---|---|
| Thermal-hydraulic physics | [`tampines`] | Component state (temperature, pressure, flow, quality, ...) to visualize |
| Reactor-vessel / instrumentation | [`nee_soon`] | Neutronics/kinetics state to visualize |
| Process control | [`chem_eng_real_time_process_control_simulator`] | Controller state (setpoints, PID output) to visualize |

## What belongs here / what does not

- **Belongs here:** visual process object wrappers, colour-map functions,
  tracer/animation logic, the `eframe::App` threading/locking scaffold
  reusable across digital-twin GUI applications.
- **Does NOT belong here:** any new physics -- if a visualization needs a
  physical quantity `tampines`/`nee_soon` don't yet expose, add it there,
  not here. The one maintainer-directed exception is [`htr10`] (bead
  `op-jyyp`, 2026-08-11): the HTR-10 simulator rewrite's *cited* design
  constants and packed-bed reference correlations, kept here with their
  V&V unit tests so the example rewrite and its tests share one
  provenance-checked source.

## Android / portability

This crate makes **no Android-portability claim** -- unlike the rest of
the workspace, GUI dependencies (`egui`/`eframe`/`egui_plot`/`egui_extras`)
are real dependencies here, not confined to `examples/`, since this
crate's entire purpose is presentation.

## Status

The four modules below are implemented: [`color_maps`] and [`app_scaffold`]
are ports of already-working code, [`components`] wraps the physics types
it visualizes, and [`animation`] carries the tracer kinematics. The one
deliberate stand-in is [`components::InstrumentationVisual`], which stays a
label/value placeholder because `nee_soon` exposes no instrumentation
readout type to wrap yet.

Per `RESPONSIBLE_USE.md`, everything here is **untrusted draft material
until human-reviewed** — see the crate README's bookkeeping-status block for
the maintainer sign-off state. The example simulators are **offline
demonstrations only**.

See the workspace's beads issue tracker for the live module plan.

## Modules

## Module `animation`

Tracer / travel-time animation.

A "tracer" is a small visual marker that travels along a component's flow
path, giving an at-a-glance sense of flow direction and speed -- e.g. dots
drifting along a pipe, faster when mass flow is higher, running backwards
when flow reverses.

## Design intent

[`FlowTracer`] is the trait a visual component implements to support this:
it exposes the mass flow driving the tracer's direction/speed and owns the
tracer's current position along the flow path, advanced once per animation
frame. [`TravelTime`] is a separate, smaller trait for components whose
end-to-end residence time matters for animation timing (a long pipe should
take visibly longer for a tracer to cross than a short one, at the same
flow velocity) -- kept separate from [`FlowTracer`] since not every
tracer-bearing component necessarily needs to expose a travel time (a
tank's "residence time" is a different calculation than a pipe's).

[`control_rod_drive`] is the same idea for a different mechanism: a control
rod travels toward its commanded depth at a bounded drive speed rather than
teleporting there, and [`ControlRodDrive`] owns that slew. It follows the
same state-ownership rule as [`TracerTrain`] (below) for the same reason.

[`TracerTrain`] is the concrete, reusable implementation of that motion:
evenly-spaced tracer marks sharing one phase, advanced by
[`TracerTrain::advance`]. [`residence_time_from_flow`] computes the
residence time that drives it from a component's fluid inventory and mass
flow.

## Where tracer state lives

Tracer motion is *stateful across frames*, but the visual components in
[`crate::components`] are `egui::Widget`s consumed by value and rebuilt
every frame from the current physics snapshot. So a [`TracerTrain`] is
owned by the **application** (typically alongside its physics state, or in
the [`crate::app_scaffold`]-driven app struct), advanced once per frame,
and copied into the widget at build time -- not owned by the widget, which
would reset its phase to zero on every repaint.

## What belongs here / what does not

- **Belongs here:** the tracer/travel-time trait contracts, the
  animation-frame update logic, and the residence-time kinematics.
- **Does NOT belong here:** the underlying flow-rate/fluid-inventory
  *physics* -- that comes from [`tampines`] (via whichever
  [`crate::components`] wrapper a tracer-bearing visual component
  composes); this module only turns that physics into on-screen motion.
  Rendering does not belong here either -- this module is deliberately
  `egui`-free so it keeps building for Android (see the crate root's
  module gating); the drawing lives with each visual component.

## No trait objects

Per the workspace's mandatory Rust design rules, these traits are a
compiler-enforced contract on each concrete visual component, not a
dispatch mechanism -- callers should match on a concrete component type
or an enum wrapping the tracer-bearing components, never
`&dyn FlowTracer`/`&dyn TravelTime`.

```rust
pub mod animation { /* ... */ }
```

### Modules

## Module `control_rod_drive`

Control-rod **drive kinematics** — the finite-speed travel of an absorber
rod toward the depth its operator or controller has commanded.

A real control rod is driven by a motor at a bounded speed; it does not
teleport to a new depth when a setpoint changes. A GUI that snaps the drawn
rod straight to the commanded fraction therefore misrepresents the machine in
a way that matters for an educational simulator: the whole point of watching
a rod bank is seeing that reactivity insertion takes *time*.

This module is the kinematics only, and is deliberately **`egui`-free** like
the rest of [`crate::animation`], so it keeps building for Android. The
drawing lives with the vessel widget, and the egui-side persistence lives in
[`crate::components::control_rod_drive`].

# Where the state lives

Same rule as [`crate::animation::TracerTrain`], for the same reason: the
visual components in [`crate::components`] are `egui::Widget`s consumed by
value and rebuilt on every repaint, so a [`ControlRodDrive`] owned by a
widget would reset to its initial position every frame and never move. The
**application** owns the drive, advances it once per frame, and copies the
resulting fraction into the widget at build time.

# Scope

This is display kinematics, not a rod-drive model. It carries no motor
dynamics, no backlash, no rod-drop/scram free-fall, and no coupling to
reactivity — the commanded fraction is whatever the simulator's own model
says, and this only governs how the *drawn* rod catches up to it.

```rust
pub mod control_rod_drive { /* ... */ }
```

### Types

#### Enum `RodDriveMotion`

Which way a rod drive is travelling, as of its last advance.

An enum rather than a signed number so a status readout cannot accidentally
print "-0.00 inserting"; and an enum rather than a trait object, per this
workspace's Rust design rules.

```rust
pub enum RodDriveMotion {
    Inserting,
    Withdrawing,
    AtRest,
}
```

##### Variants

###### `Inserting`

The rod is being driven **into** the core — insertion fraction rising,
reactivity falling.

###### `Withdrawing`

The rod is being driven **out of** the core — insertion fraction
falling, reactivity rising.

###### `AtRest`

The rod is at its commanded depth and is not moving.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RodDriveMotion { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RodDriveMotion) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `ControlRodDrive`

A control-rod drive slewing the **drawn** insertion fraction toward the
commanded one at a bounded speed.

Insertion fraction is dimensionless in `[0, 1]`: `0.0` fully withdrawn,
`1.0` fully inserted. `stroke` is the physical travel the fraction spans and
`speed` the drive speed, so the fraction rate is `speed / stroke` per second
— expressing it that way rather than as a bare "fraction per second" keeps
the two physical quantities visible and `uom`-checked at the call site.

`Copy`, small, and free of any `egui` type, so an application can keep one
per rod bank in whatever state it already owns.

```rust
pub struct ControlRodDrive {
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
  pub fn new(initial_insertion_fraction: f64, stroke: Length, speed: Velocity) -> Self { /* ... */ }
  ```
  A drive starting at `initial_insertion_fraction`, travelling `stroke` at

- ```rust
  pub fn htr10(initial_insertion_fraction: f64) -> Self { /* ... */ }
  ```
  The HTR-10 rod bank at `initial_insertion_fraction`, using the published

- ```rust
  pub fn insertion_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Where the rod is currently **drawn**, dimensionless in `[0, 1]`.

- ```rust
  pub fn motion(self: &Self) -> RodDriveMotion { /* ... */ }
  ```
  Direction of travel as of the last [`Self::advance`].

- ```rust
  pub fn advance(self: &mut Self, commanded: f64, dt: Time) { /* ... */ }
  ```
  Move the drawn fraction toward `commanded` over one animation timestep

- ```rust
  pub fn snap_to(self: &mut Self, fraction: f64) { /* ... */ }
  ```
  Put the rod at `fraction` immediately, with no travel.

- ```rust
  pub fn fraction_per_second(self: &Self) -> f64 { /* ... */ }
  ```
  The drive rate expressed as insertion fraction per second,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ControlRodDrive { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `htr10_illustrative_rod_drive_speed`

**Attributes:**

- `MustUse { reason: None }`

Drive speed used for the HTR-10 rod animation.

# ⚠️ ILLUSTRATIVE — NOT A PLANT FIGURE

**No published HTR-10 control-rod drive speed was found** in this project's
scoping notes (`docs/reactor-scoping/htr10-plant-data.md`,
`docs/reactor-scoping/htr10-neutronics.md`) or in its literature archive
(`crates/kovan-literature/open/`), searched 2026-08-12. The value returned
here is therefore **invented for legibility**: it is exactly
[`HTR10_ROD_STROKE_METRES`] divided by
[`HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS`], i.e. a full stroke in 20 s, which
is about 0.0985 m/s.

It must **not** be cited as an HTR-10 design or operating figure, and nothing
in this repository derives a physical result from it — it governs only how
fast a drawing moves. If a sourced value is ever found, replace this and say
where it came from.

```rust
pub fn htr10_illustrative_rod_drive_speed() -> uom::si::f64::Velocity { /* ... */ }
```

### Constants and Statics

#### Constant `HTR10_ROD_STROKE_METRES`

Full travel of the HTR-10 control rods, taken as the published **mean pebble
bed height, 1.97 m**.

Provenance: the same IAEA HTR-10 description the vessel artwork is
proportioned from (see
[`crate::components::htr10_reactor_vessel`]'s module docs — pebble bed 1.8 m
diameter by 1.97 m mean height). The rods run in ten borings in the side
reflector alongside that bed, so the active height is the right order for the
stroke.

**This is the stroke, which is sourced. The drive *speed* below is not.**

```rust
pub const HTR10_ROD_STROKE_METRES: f64 = 1.97;
```

#### Constant `HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS`

Time the illustrative HTR-10 rod drive takes to cross its whole stroke, in
seconds.

Chosen so that dragging a rod-bank slider produces travel a viewer can
actually watch, rather than a jump or a wait. See
[`htr10_illustrative_rod_drive_speed`] for the honesty caveat.

```rust
pub const HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS: f64 = 20.0;
```

### Types

#### Struct `TracerPulse`

A single tracer mark released at a fixed minimum interval.

[`TracerTrain`] keeps `count` marks permanently on the path, evenly spaced.
That is right for showing a continuous flow, but on a short run — or a fast
one — it produces a stream of marks flickering past, which is hard to read.
A pulse instead shows **one** mark at a time, released no more often than
`min_interval`, so a fast pipe blinks a single plug through at a comfortable
rate rather than strobing.

The mark still crosses the run in exactly one residence time; the interval
only controls the gap between releases. When the residence time is longer
than the interval, the mark is in flight continuously and the interval has
no effect.

```rust
pub struct TracerPulse {
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
  pub fn new(min_interval: Time) -> Self { /* ... */ }
  ```
  Create a pulse releasing a mark no more often than `min_interval`.

- ```rust
  pub fn advance(self: &mut Self, dt: Time, residence_time: Time, mass_flow: MassRate) { /* ... */ }
  ```
  Advance by `dt`, given the run's `residence_time` and `mass_flow`.

- ```rust
  pub fn position(self: &Self, residence_time: Time) -> Option<f64> { /* ... */ }
  ```
  Position of the mark along the path in `[0, 1]`, or `None` when no mark

- ```rust
  pub fn direction(self: &Self) -> FlowDirection { /* ... */ }
  ```
  Direction of travel as of the last [`Self::advance`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TracerPulse { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TracerPulse) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `FlowDirection`

Which way a [`TracerTrain`] is currently moving along its flow path.

Enum, not a signed scalar, so that a `match` on flow direction is
exhaustive at every call site (per the workspace's enum-dispatch rule).

```rust
pub enum FlowDirection {
    Forward,
    Reverse,
    Stagnant,
}
```

##### Variants

###### `Forward`

Flow runs inlet -> outlet; tracers advance toward position `1`.

###### `Reverse`

Flow is reversed; tracers advance back toward position `0`.

###### `Stagnant`

No flow (or no finite residence time) -- tracers hold position.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `TracerTrain`

A train of evenly-spaced tracer marks sharing a single phase, moving along
one component's flow path.

Rather than storing each mark's position independently (which lets them
drift apart through floating-point accumulation), the train stores one
`phase` in `[0, 1)` and derives mark `i` of `count` as
`(phase + i/count) mod 1`. The marks therefore stay exactly evenly spaced
for the lifetime of the animation.

## Timing

[`Self::advance`] moves the phase at `1 / residence_time` of the path per
second, so a mark takes exactly one residence time to traverse the
component -- the animation is a direct readout of the physical travel
time, not a free-running decorative loop. Doubling the mass flow halves
the residence time (see [`residence_time_from_flow`]) and so visibly
doubles the tracer speed.

`Copy`, so an application can hold the authoritative train in its own
state and hand a copy to a per-frame widget without ceremony.

```rust
pub struct TracerTrain {
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
  pub fn new(count: usize) -> Self { /* ... */ }
  ```
  A train of `count` evenly-spaced marks, starting at phase `0` and

- ```rust
  pub fn advance(self: &mut Self, dt: Time, residence_time: Time, mass_flow: MassRate) { /* ... */ }
  ```
  Advance the train by one animation timestep `dt`, given the

- ```rust
  pub fn phase(self: &Self) -> f64 { /* ... */ }
  ```
  Phase of the leading mark, in `[0, 1)`.

- ```rust
  pub fn count(self: &Self) -> usize { /* ... */ }
  ```
  Number of marks in the train (always >= 1).

- ```rust
  pub fn direction(self: &Self) -> FlowDirection { /* ... */ }
  ```
  Direction of travel as of the last [`Self::advance`] call.

- ```rust
  pub fn positions(self: &Self) -> impl Iterator<Item = f64> { /* ... */ }
  ```
  Positions of every mark along the flow path, each in `[0, 1)`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TracerTrain { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TracerTrain) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Traits

#### Trait `FlowTracer`

A visual tracer that moves along a component's flow path, its direction
and speed derived from mass flow.

Implementors own the tracer's current position; [`Self::advance`] is
called once per animation frame by the GUI's update loop (see
[`crate::app_scaffold`], which owns that loop). Most implementors will
delegate the motion itself to a [`TracerTrain`] field rather than
re-deriving the kinematics.

```rust
pub trait FlowTracer {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `mass_flow`: Current mass flow rate driving this tracer's direction and speed.
- `tracer_position`: Tracer position along the flow path, `[0, 1]` (`0` = inlet, `1` =
- `advance`: Advance the tracer's position by one animation timestep `dt`,

#### Trait `TravelTime`

A component whose end-to-end residence time should influence tracer
animation timing (a tracer should visibly take longer to cross a
component with a longer travel time, at the same flow velocity).

```rust
pub trait TravelTime {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `residence_time`: Residence time for the current flow state -- how long a fluid parcel

### Functions

#### Function `residence_time_from_flow`

Mean residence time of a fluid parcel in a component holding
`fluid_inventory` kilograms of fluid and passing `mass_flow`.

This is the standard well-mixed/plug-flow residence-time identity

```text
    tau = m / m_dot
```

where `m` is the fluid mass currently held in the component and `m_dot`
the mass flow through it. For a constant-area pipe this is equivalent to
`L / v` (length over bulk velocity), since `m = rho * A * L` and
`m_dot = rho * A * v`.

Only the **magnitude** of `mass_flow` is used -- a reversed flow takes
just as long to traverse the component as a forward one; direction is
[`TracerTrain::advance`]'s concern, not the travel time's.

## Valid ranges and degenerate cases

`fluid_inventory` is expected non-negative and `mass_flow` non-zero. At
exactly zero flow the residence time is unbounded, which has no useful
finite representation; this returns [`infinite_residence_time`] there, and
[`TracerTrain::advance`] reads that as "no motion" (a stagnant component's
tracers should sit still, which is the physically honest display). A
negative `fluid_inventory` is not meaningful and likewise yields
[`infinite_residence_time`] rather than a negative travel time.

```rust
pub fn residence_time_from_flow(fluid_inventory: uom::si::f64::Mass, mass_flow: uom::si::f64::MassRate) -> uom::si::f64::Time { /* ... */ }
```

#### Function `infinite_residence_time`

The unbounded residence time of a component with no flow through it --
an infinite [`Time`], i.e. "a parcel never traverses this component".

`uom` has no `Time::INFINITY` associated constant, so this names the
value once rather than spelling `Time::new::<second>(f64::INFINITY)` at
each site. [`TracerTrain::advance`] treats it (and any other non-finite
residence time) as [`FlowDirection::Stagnant`].

```rust
pub fn infinite_residence_time() -> uom::si::f64::Time { /* ... */ }
```

#### Function `residence_time_from_velocity`

Residence time of a plug-flow run of `length` moving at `velocity`.

`tau = length / |velocity|`. This is the same quantity
[`residence_time_from_flow`] returns, reached from kinematics instead of
inventory: for a run of uniform cross-section `A` and density `rho` the
inventory is `m = rho*A*L` and the mass flow is `m_dot = rho*A*u`, so
`m/m_dot = L/u` — the density and area cancel. Use this form when the
velocity is known and the density is not.

Returns [`infinite_residence_time`] for zero, negative-length or
non-finite input: those are the cases with no well-defined tracer speed,
which [`TracerTrain::advance`] then freezes rather than guessing at.

```rust
pub fn residence_time_from_velocity(length: uom::si::f64::Length, velocity: uom::si::f64::Velocity) -> uom::si::f64::Time { /* ... */ }
```

### Re-exports

#### Re-export `ControlRodDrive`

```rust
pub use control_rod_drive::ControlRodDrive;
```

#### Re-export `RodDriveMotion`

```rust
pub use control_rod_drive::RodDriveMotion;
```

## Module `ciet_opcua`

OPC-UA (IEC 62541) interface layer for the CIET Educational Simulator v2.

This module is the **shared interface** between the two CIET v2 binaries:

- `ciet_educational_simulator_v2` — the simulator, which runs the physics and
  hosts the OPC-UA server;
- `ciet_v2_opcua_client` — the bundled demo client, which discovers a
  simulator on the local network and drives it.

It contains **no physics** (per this crate's `CLAUDE.md`) and **no GUI**, so
it compiles everywhere the workspace targets — including headless on Termux
/ `aarch64-linux-android`, with no target gate. `async-opcua` was chosen
precisely for that: its crypto is RustCrypto, not `openssl-sys`.

## What is CIET's, and what is shared

The reactor-agnostic half — TCP transport, the server thread and its tokio
runtime, PKI paths, mDNS, address-space construction, the read/write
callbacks — lives in [`opcua_core`](crate::opcua_core) and serves any OUTRAM
PARK simulator. **This module is only CIET's half of that contract**: the
plant state, the node map, the identity strings, and the mapping between
them. If you are adding a variable, everything you need is here; if you are
adding a *second simulator*, read [`opcua_core::simulator`](crate::opcua_core::simulator)
instead.

## Layout

| Module | Role |
|---|---|
| [`state`] | [`CietState`], the flat plant snapshot shared between threads |
| [`node_map`] | the enums defining every OPC-UA variable — the single source of truth |
| [`user_controls`] | the pending-write mailbox remote writes are parked in |
| [`simulator`] | CIET's identity profile and [`CietNode`](simulator::CietNode), the shared layer's view of the node map |
| [`server`] | starting the shared OPC-UA server, bound to CIET |
| [`pki_paths`] | where CIET's PKI directory lives (`~/.outram-park/...`) |
| [`discovery`] | CIET's mDNS marker, and a browser bound to it |

## Reading this module for the first time

Start at [`node_map`]. Its three enums — [`CietSignal`] (read-only outputs),
[`CietControl`] (writable continuous set points) and [`CietSwitch`]
(writable on/off controls) — define the entire interface. The server's
address space, its read/write callbacks, the simulator's "how to connect"
table and the demo client's variable list are all generated from them, so
there is exactly one place to look up what a node means.

## Security: there is none, deliberately

The server runs with **`SecurityPolicy::None` and anonymous access**. Anyone
who can reach the port can read every output and write every control. That
is a deliberate choice for a throwaway teaching demonstrator — it makes
"point UaExpert at it and poke the loop" a ten-second exercise — and it is
the reason the simulator prints a plain warning banner whenever it is bound
to anything other than loopback.

Hardening this (certificates, a trust list, user tokens, an audit trail) is
explicitly **out of scope** here and left to security researchers. Do not
describe this interface as secured, and do not deploy it anywhere that
matters.

## Scope limit (`RESPONSIBLE_USE.md`)

OPC-UA is a plant-connectivity protocol, so the boundary matters: this
interface exists so an **offline educational simulator** can be driven by
standard OPC-UA tooling on a bench or in a classroom. It must **never** be
connected to live operational systems, plant systems, safety-critical
infrastructure, real-time plant monitoring, or institutional production
systems, and its outputs are not authoritative for any operational,
licensing or safety purpose.

```rust
pub mod ciet_opcua { /* ... */ }
```

### Modules

## Module `discovery`

CIET v2's binding to the shared mDNS / DNS-SD discovery layer.

The announcement and browsing machinery is reactor-agnostic and lives in
[`opcua_core::discovery`](crate::opcua_core::discovery). This module supplies
the two strings that make an announcement *CIET's* — the instance-name prefix
and the `product` TXT marker — and binds the browser to them, so a
[`SimulatorBrowser`] reports CIET simulators and ignores every other OPC-UA
server on the link.

| Direction | Entry point |
|---|---|
| Simulator announces itself | handled inside [`super::server::spawn_opcua_server_thread`] |
| Client listens for simulators | [`SimulatorBrowser::start`] → [`SimulatorBrowser::discovered`] |

## This is announcement only — never scanning

The only network traffic originated is a multicast DNS-SD announcement of
*this* machine's own service, and multicast queries for the
`_opcua-tcp._tcp` service type. Nothing probes, sweeps, enumerates or
fingerprints another host, and nothing here may ever grow a port scanner or
a subnet sweeper — see [`opcua_core::discovery`](crate::opcua_core::discovery)
for the full statement of that rule, and `RESPONSIBLE_USE.md` for why it is
not negotiable.

## Practical caveat: many networks break this

Campus and enterprise WiFi commonly enable client isolation, and many managed
networks filter multicast outright, so discovery finds nothing *and* the
subsequent OPC-UA connection fails even with a hand-typed URL. A phone
hotspot or a home router works. That is a property of the network, not a bug.

## Units

Everything here is transport metadata — host names, ports, IP addresses, DNS
labels. No physical quantities, no units.

```rust
pub mod discovery { /* ... */ }
```

### Types

#### Type Alias `SimulatorBrowser`

Listens for CIET v2 simulators announcing themselves on the local link.

The shared [`MdnsBrowser`] bound to CIET, so it filters on
[`CIET_PRODUCT_TXT_VALUE`] and nothing else needs to know that.

Construct once with [`start`](MdnsBrowser::start), then poll
[`discovered`](MdnsBrowser::discovered) whenever convenient — from an egui
repaint, from a CLI loop, from anywhere. Polling never blocks, so it is safe
to call at frame rate.

```rust
pub type SimulatorBrowser = crate::opcua_core::discovery::MdnsBrowser<super::simulator::CietOpcuaSimulator>;
```

### Constants and Statics

#### Constant `CIET_MDNS_INSTANCE_PREFIX`

Prefix of the DNS-SD instance name the simulator announces.

The full instance name is this prefix, optionally followed by `-` and a
caller-supplied suffix (a machine name, a bench number, a student's name) so
several simulators on one link stay distinguishable. It is also the fallback
when a supplied instance name sanitises to nothing.

```rust
pub const CIET_MDNS_INSTANCE_PREFIX: &str = "CIET-Educational-Simulator-v2";
```

#### Constant `CIET_PRODUCT_TXT_VALUE`

TXT record value that identifies an announcement as *this* simulator.

[`SimulatorBrowser::discovered`] returns only services whose
[`PRODUCT_TXT_KEY`] equals this, so a CIET instance is never confused with
some other OPC-UA server that happens to share the link — `_opcua-tcp._tcp`
is the generic OPC-UA service type and everything answers to it.

```rust
pub const CIET_PRODUCT_TXT_VALUE: &str = "ciet-educational-simulator-v2";
```

### Re-exports

#### Re-export `DiscoveredSimulator`

```rust
pub use crate::opcua_core::discovery::DiscoveredSimulator;
```

#### Re-export `DiscoveryError`

```rust
pub use crate::opcua_core::discovery::DiscoveryError;
```

#### Re-export `MdnsAdvertisement`

```rust
pub use crate::opcua_core::discovery::MdnsAdvertisement;
```

#### Re-export `OPCUA_MDNS_SERVICE_TYPE`

```rust
pub use crate::opcua_core::discovery::OPCUA_MDNS_SERVICE_TYPE as CIET_MDNS_SERVICE_TYPE;
```

#### Re-export `PATH_TXT_KEY`

```rust
pub use crate::opcua_core::discovery::PATH_TXT_KEY;
```

#### Re-export `PRODUCT_TXT_KEY`

```rust
pub use crate::opcua_core::discovery::PRODUCT_TXT_KEY;
```

## Module `node_map`

The OPC-UA node map: the single source of truth for what CIET v2 exposes.

Three enums describe every OPC-UA variable the simulator publishes:

| Enum | Direction | OPC-UA type | Meaning |
|---|---|---|---|
| [`CietSignal`] | read-only (`CurrentRead`) | `Double` | a measurement or diagnostic the simulator produces |
| [`CietControl`] | read/write (`CurrentRead \| CurrentWrite`) | `Double` | a continuous set point a client may drive |
| [`CietSwitch`] | read/write (`CurrentRead \| CurrentWrite`) | `Boolean` | an on/off control a client may drive |

Everything downstream is derived from these enums — the server's address
space, its read and write callbacks, the GUI's "how to connect" node table,
and the demo client's browse list. Adding a variable means adding one enum
variant and filling in its `match` arms; the compiler then points at every
place that must be updated. That exhaustiveness is exactly why these are
enums rather than a table of trait objects (see the workspace Rust design
rules: no `Box<dyn Trait>` for dispatch).

## Node identifiers

Nodes live in the namespace [`CIET_NAMESPACE_URI`] and use **string**
identifiers, so a client can address them by name without browsing:

```text
ns=<index>;s=CIET.Heater.PowerKw
ns=<index>;s=CIET.Temperature.BT12HeaterOutletDegC
```

The namespace index is assigned by the server at start-up (typically `2`,
after the OPC-UA core namespace `0` and the server's own namespace `1`), and
the running index is what the GUI displays. Do not hard-code `2` in a client
— read it from the server, or use [`browse_name`](CietSignal::browse_name)
and let the client resolve it.

## Safety envelope

Every writable variable carries a documented [`valid_range`](CietControl::valid_range)
and writes are clamped to it by the state setters in
[`super::state`]. A client cannot push the solver outside its stable
envelope, no matter what it sends.

## Scope

`RESPONSIBLE_USE.md` applies: this interface exists so an **offline**
educational simulator can be driven by standard OPC-UA tooling. It must
never be connected to live operational systems, plant systems,
safety-critical infrastructure, or institutional production systems.

```rust
pub mod node_map { /* ... */ }
```

### Types

#### Enum `CietSignal`

A read-only quantity the simulator publishes.

These are the values a client would trend, log, or display: the
instrumented temperatures and flowrates a real CIET operator would watch,
plus the controller outputs and timing diagnostics. All are `f64` in the
units named by [`unit`](Self::unit).

```rust
pub enum CietSignal {
    HeaterPowerKw,
    Bt11HeaterInletDegC,
    Bt12HeaterOutletDegC,
    Bt43CtahInletDegC,
    Bt41CtahOutletDegC,
    Bt60DhxTubeInletDegC,
    Bt21DhxTubeOutletDegC,
    Bt21DhxShellInletDegC,
    Bt27DhxShellOutletDegC,
    Bt65TchxInletDegC,
    Bt66TchxOutletDegC,
    Fm40CtahBranchKgPerS,
    Fm20DhxBranchKgPerS,
    Fm60DracsKgPerS,
    CtahHtcWattPerM2K,
    TchxHtcWattPerM2K,
    TopMixingNodeDegC,
    BottomMixingNodeDegC,
    SimulationTimeSeconds,
    ElapsedTimeSeconds,
    CalcTimeMs,
}
```

##### Variants

###### `HeaterPowerKw`

Heater electrical power actually applied this timestep, kW. Differs from
the requested value when the over-temperature killswitch has tripped.

###### `Bt11HeaterInletDegC`

BT-11, heater inlet bulk temperature, degC.

###### `Bt12HeaterOutletDegC`

BT-12, heater outlet bulk temperature, degC.

###### `Bt43CtahInletDegC`

BT-43, CTAH inlet bulk temperature, degC.

###### `Bt41CtahOutletDegC`

BT-41, CTAH outlet bulk temperature, degC.

###### `Bt60DhxTubeInletDegC`

BT-60, DHX tube inlet bulk temperature, degC.

###### `Bt21DhxTubeOutletDegC`

BT-21, DHX tube outlet bulk temperature, degC.

###### `Bt21DhxShellInletDegC`

BT-21, DHX shell inlet bulk temperature, degC.

###### `Bt27DhxShellOutletDegC`

BT-27, DHX shell outlet bulk temperature, degC.

###### `Bt65TchxInletDegC`

BT-65, TCHX inlet bulk temperature, degC.

###### `Bt66TchxOutletDegC`

BT-66, TCHX outlet bulk temperature, degC.

###### `Fm40CtahBranchKgPerS`

FM-40, CTAH-branch mass flowrate, kg/s. Signed; negative is reverse flow.

###### `Fm20DhxBranchKgPerS`

FM-20, DHX-branch mass flowrate, kg/s.

###### `Fm60DracsKgPerS`

FM-60, DRACS-loop mass flowrate magnitude, kg/s.

###### `CtahHtcWattPerM2K`

CTAH air-side heat transfer coefficient commanded by its PID controller,
W/(m^2 K).

###### `TchxHtcWattPerM2K`

TCHX air-side heat transfer coefficient commanded by its PID controller,
W/(m^2 K).

###### `TopMixingNodeDegC`

Top mixing node (branches 5a/5b/4) temperature, degC.

###### `BottomMixingNodeDegC`

Bottom mixing node (branches 17a/17b/18) temperature, degC.

###### `SimulationTimeSeconds`

Simulated time elapsed, s.

###### `ElapsedTimeSeconds`

Wall-clock time elapsed, s. Compare with the simulated time to see
whether the simulation is keeping up with real time.

###### `CalcTimeMs`

Wall-clock cost of the last timestep, ms.

##### Implementations

###### Methods

- ```rust
  pub fn node_identifier(self: &Self) -> &'static str { /* ... */ }
  ```
  The string part of this signal's `NodeId`, e.g.

- ```rust
  pub fn browse_name(self: &Self) -> &'static str { /* ... */ }
  ```
  Short OPC-UA browse name, e.g. `"BT12HeaterOutletDegC"`.

- ```rust
  pub fn display_name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-facing label for the GUI and client tables.

- ```rust
  pub fn unit(self: &Self) -> &'static str { /* ... */ }
  ```
  Engineering unit, as a short display string.

- ```rust
  pub fn read(self: &Self, state: &CietState) -> f64 { /* ... */ }
  ```
  Read this signal's current value out of a state snapshot.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CietSignal { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CietSignal) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `CietControl`

A continuous control a client may write.

Writes are **clamped** to [`valid_range`](Self::valid_range) — an
out-of-range write is honoured at the nearest limit rather than rejected, so
a client that sends 1000 kW gets the 15 kW ceiling and a clear read-back.

```rust
pub enum CietControl {
    HeaterPowerKw,
    CtahPumpPressurePascals,
    Bt41CtahOutletSetPointDegC,
    Bt66TchxOutletSetPointDegC,
    HeaterSteadyStatePowerKw,
    FrequencyResponseAmplitudeKw,
    FrequencyResponseAngularVelocityRadPerS,
    TimestepSeconds,
}
```

##### Variants

###### `HeaterPowerKw`

Heater electrical power set point, kW. Ignored while advanced heater
control is switched on (that driver overwrites it every timestep).

###### `CtahPumpPressurePascals`

CTAH pump pressure rise, Pa. The forced-circulation driver; negative
reverses the flow direction.

###### `Bt41CtahOutletSetPointDegC`

Set point for BT-41, the CTAH outlet temperature, degC.

###### `Bt66TchxOutletSetPointDegC`

Set point for BT-66, the TCHX outlet temperature, degC.

###### `HeaterSteadyStatePowerKw`

Steady (mean) heater power used by advanced heater control, kW.

###### `FrequencyResponseAmplitudeKw`

Peak amplitude of the frequency-response perturbation, kW.

###### `FrequencyResponseAngularVelocityRadPerS`

Angular frequency of the frequency-response perturbation, rad/s.

###### `TimestepSeconds`

Requested solver timestep, s. Only honoured in slow-motion mode, and
clamped to the Courant stability limit either way.

##### Implementations

###### Methods

- ```rust
  pub fn index(self: &Self) -> usize { /* ... */ }
  ```
  Position of this control in [`Self::ALL`].

- ```rust
  pub fn node_identifier(self: &Self) -> &'static str { /* ... */ }
  ```
  The string part of this control's `NodeId`.

- ```rust
  pub fn browse_name(self: &Self) -> &'static str { /* ... */ }
  ```
  Short OPC-UA browse name.

- ```rust
  pub fn display_name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-facing label for the GUI and client tables.

- ```rust
  pub fn unit(self: &Self) -> &'static str { /* ... */ }
  ```
  Engineering unit, as a short display string.

- ```rust
  pub fn valid_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Inclusive `(minimum, maximum)` this control accepts.

- ```rust
  pub fn read(self: &Self, state: &CietState) -> f64 { /* ... */ }
  ```
  Read this control's current value out of a state snapshot, so a client

- ```rust
  pub fn write(self: &Self, state: &mut CietState, value: f64) -> f64 { /* ... */ }
  ```
  Apply a client's write to plant state, clamped to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CietControl { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CietControl) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `CietSwitch`

A boolean control a client may write.

```rust
pub enum CietSwitch {
    AdvancedHeaterControlOn,
    FrequencyResponseOn,
    CtahBranchBlocked,
    DhxBranchBlocked,
    FastForwardOn,
    SlowMotionOn,
    CoarseHeaterMesh,
}
```

##### Variants

###### `AdvancedHeaterControlOn`

Master switch for advanced heater control. While on, the heater power is
driven by the steady-state + frequency-response settings each timestep
and direct writes to the heater power set point are overwritten.

###### `FrequencyResponseOn`

Add the sinusoidal perturbation to the steady heater power.

###### `CtahBranchBlocked`

Block flow through the CTAH branch, as if a valve were shut.

###### `DhxBranchBlocked`

Block flow through the DHX branch.

###### `FastForwardOn`

Run faster than real time where the machine allows it.

###### `SlowMotionOn`

Run slower than real time, honouring the requested timestep.

###### `CoarseHeaterMesh`

Use the coarse 8-node heater mesh instead of the fine 15-node mesh.
Cheaper per timestep; useful on slow hardware and on Termux.

##### Implementations

###### Methods

- ```rust
  pub fn index(self: &Self) -> usize { /* ... */ }
  ```
  Position of this switch in [`Self::ALL`].

- ```rust
  pub fn node_identifier(self: &Self) -> &'static str { /* ... */ }
  ```
  The string part of this switch's `NodeId`.

- ```rust
  pub fn browse_name(self: &Self) -> &'static str { /* ... */ }
  ```
  Short OPC-UA browse name.

- ```rust
  pub fn display_name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-facing label for the GUI and client tables.

- ```rust
  pub fn read(self: &Self, state: &CietState) -> bool { /* ... */ }
  ```
  Read this switch's current value out of a state snapshot.

- ```rust
  pub fn write(self: &Self, state: &mut CietState, value: bool) { /* ... */ }
  ```
  Apply a client's write to plant state.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CietSwitch { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CietSwitch) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `total_node_count`

Total number of OPC-UA variables the simulator publishes.

Useful for the GUI's "N nodes served" line and for a sanity check in tests.

```rust
pub fn total_node_count() -> usize { /* ... */ }
```

### Constants and Statics

#### Constant `CIET_NAMESPACE_URI`

Namespace URI for every CIET v2 variable.

```rust
pub const CIET_NAMESPACE_URI: &str = "urn:outram-park:ciet-educational-simulator-v2";
```

#### Constant `DEFAULT_OPCUA_PORT`

Default OPC-UA TCP port. 4840 is the IANA-registered port for `opcua-tcp`.

```rust
pub const DEFAULT_OPCUA_PORT: u16 = 4840;
```

#### Constant `ENDPOINT_PATH`

Endpoint path appended to the server URL, giving
`opc.tcp://<host>:<port>/ciet`.

```rust
pub const ENDPOINT_PATH: &str = "/ciet";
```

#### Constant `CONTROLS_FOLDER_NAME`

Browse name of the folder holding the writable controls.

```rust
pub const CONTROLS_FOLDER_NAME: &str = "Controls";
```

#### Constant `OUTPUTS_FOLDER_NAME`

Browse name of the folder holding the read-only outputs.

```rust
pub const OUTPUTS_FOLDER_NAME: &str = "Outputs";
```

## Module `pki_paths`

Where the CIET v2 OPC-UA server keeps its PKI directory.

The path resolution itself is reactor-agnostic and lives in
[`opcua_core::pki`](crate::opcua_core::pki); this module names CIET's
directory and binds the shared helpers to it, so callers never pass the
directory name by hand.

`async-opcua` needs a writable directory to hold its **application instance
certificate**. Putting it under the user's home means the simulator never
scatters `./pki` folders into whatever working directory it happened to be
launched from.

## Layout

| Platform | Root | PKI dir ([`ciet_v2_pki_dir`]) |
|---|---|---|
| Linux / macOS / Termux | `$HOME/.outram-park` | `$HOME/.outram-park/ciet-v2-opcua-pki` |
| Windows | `%APPDATA%\outram-park` | `%APPDATA%\outram-park\ciet-v2-opcua-pki` |

Inside the PKI directory, `async-opcua` populates the usual OPC-UA
certificate-store subtree itself on first run:

```text
ciet-v2-opcua-pki/
  own/cert.der          <- this server's self-signed instance certificate
  private/private.pem   <- the matching private key
  trusted/              <- client certificates the server would trust
  rejected/             <- client certificates it has seen and refused
```

## Nothing sensitive is stored here

The CIET v2 server runs with `SecurityPolicy::None` and anonymous access
(see [`super::server`]). No channel is ever encrypted or signed with the
keypair, no client certificate is ever validated against the trust list, and
no user credential of any kind is written. What lands on disk is therefore a
**throwaway self-signed keypair** that authenticates nothing — deleting the
whole directory costs nothing but a regeneration on next start-up. Do not
describe it as a credential store, and do not reuse the key for anything.

## No credentials, ever (`RESPONSIBLE_USE.md`)

This module names a directory and reports its path. It must never grow code
that reads institutional credentials, API keys, access tokens, or any
certificate belonging to a real facility or production system.

## Units

Everything here is a filesystem path or a name. No physical quantities, no
units.

```rust
pub mod pki_paths { /* ... */ }
```

### Functions

#### Function `ciet_v2_pki_dir`

The PKI directory for the CIET v2 OPC-UA server, created if it does not
exist.

This is `<`[`outram_park_home`]`>/ciet-v2-opcua-pki`. Pass it straight to
`ServerBuilder::pki_dir`; `async-opcua` creates the `own/`, `private/`,
`trusted/` and `rejected/` subdirectories itself and writes a self-signed
application instance certificate into `own/` on first start-up.

A creation failure is warned about rather than returned as an error, so a
read-only home directory cannot take the whole simulator down.

```rust
pub fn ciet_v2_pki_dir() -> std::path::PathBuf { /* ... */ }
```

#### Function `ciet_v2_instance_pki_dir`

A per-instance PKI directory underneath [`ciet_v2_pki_dir`], created if
missing.

Returns `<`[`ciet_v2_pki_dir`]`>/<sanitised instance_tag>`.

## Why this exists: parallel instances clobber a shared certificate store

`async-opcua` writes its self-signed keypair into the PKI directory on
start-up. Two servers starting **concurrently** against the same directory
race on `own/cert.der` and `private/private.pem`, and can read a
half-written file — a real hazard for the headless CIET tests, which may run
several simulators at once, and for a developer running the simulator while a
test suite runs.

The tag makes each instance's store disjoint. The shared server layer derives
it from the TCP port, which is the one thing two servers that can coexist on
a machine must differ in, so isolation is automatic and needs no
configuration. Tests that want a stronger guarantee — a fresh directory per
run rather than per port — can pass [`unique_instance_tag`].

`instance_tag` is sanitised to ASCII alphanumerics, `-` and `_`; anything
else becomes `-`, and an empty result becomes `"default"`. That keeps a
caller from escaping the directory with `../` or breaking on a path
separator.

```rust
pub fn ciet_v2_instance_pki_dir(instance_tag: &str) -> std::path::PathBuf { /* ... */ }
```

#### Function `describe_pki_location`

A one-line, human-readable summary of where the PKI directory is, for the
simulator's "how to connect" panel and its start-up log line.

The wording deliberately states that nothing sensitive is stored, so a
reader of the GUI is not misled into thinking the interface is secured.

# Example output

```text
PKI directory: /home/alice/.outram-park/ciet-v2-opcua-pki (self-signed keypair only -- SecurityPolicy::None stores no credentials)
```

```rust
pub fn describe_pki_location() -> String { /* ... */ }
```

### Constants and Statics

#### Constant `CIET_V2_PKI_DIR_NAME`

Directory name of the CIET v2 PKI store, relative to the OUTRAM PARK
per-user root ([`outram_park_home`]).

```rust
pub const CIET_V2_PKI_DIR_NAME: &str = "ciet-v2-opcua-pki";
```

### Re-exports

#### Re-export `outram_park_home`

```rust
pub use crate::opcua_core::pki::outram_park_home;
```

#### Re-export `outram_park_home_path`

```rust
pub use crate::opcua_core::pki::outram_park_home_path;
```

#### Re-export `unique_instance_tag`

```rust
pub use crate::opcua_core::pki::unique_instance_tag;
```

## Module `server`

Starting the CIET v2 OPC-UA server.

The transport, the server thread, the address space and the callbacks all
live in [`opcua_core::server`](crate::opcua_core::server), which serves any
OUTRAM PARK simulator. This module is CIET's binding to it: the shared types
under their CIET names, plus a concrete
[`spawn_opcua_server_thread`] so callers never write a turbofish.

[`spawn_opcua_server_thread`] is the whole entry point. Give it the shared
plant state, the shared remote-write mailbox and an [`OpcuaServerConfig`], and
it returns an [`OpcuaServerHandle`] describing where clients should connect.

```text
physics thread ──write outputs, apply_and_clear()──┐
GUI thread ─────write controls────────────────────►├─► Arc<RwLock<CietState>>
                                                   │        ▲        ▲
OPC-UA server thread   read callbacks ─────────────────read──┘        │
(own tokio runtime)    200 ms updater ─────────────────read──────────-┘
                       write callbacks ──► Arc<RwLock<CietUserControls>>
```

## Reads are live; writes are deferred

**Reads** are served straight from [`CietState`](super::state::CietState)
under a read lock, so a
client always sees the *effective* value the solver is using — write 1000 kW
and read back the 15 kW ceiling.

**Writes** do not touch plant state. They are parked in
[`CietUserControls`](super::user_controls::CietUserControls) and applied by
the physics thread at the top of its next timestep, which is where clamping
and NaN rejection happen. That removes the lost-update race against the GUI's
wholesale `overwrite_state`, and keeps a room full of clients off the
plant-state lock. The mapping from a node to the field it reads, and to the
request slot it writes, is [`super::simulator::CietNode`].

## Security: there is none, deliberately

The endpoint uses **`SecurityPolicy::None`, `MessageSecurityMode::None` and
anonymous user tokens**: traffic is unencrypted and unsigned, no client
certificate is checked, no credential is required, and therefore **anyone who
can reach the TCP port can write every control** — heater power, pump
pressure, both branch valves, the timestep.

That is a deliberate choice for a throwaway teaching demonstrator, so that
"point UaExpert at it and poke the loop" is a ten-second exercise. Hardening
(certificates, trust lists, user tokens, audit trails) is explicitly left to
security researchers rather than half-done here.

The only mitigations you should rely on: every request is **clamped** to its
documented envelope on apply and NaN is ignored, so a hostile client can
annoy the simulation but not destabilise it; the bind address can be set to
loopback ([`OpcuaServerConfig::is_loopback_only`]); and a warning is printed
whenever it binds wider. Do not describe this interface as secured, and do not
run it on a network you do not control.

## Scope (`RESPONSIBLE_USE.md`)

This serves an **offline educational simulator**. It must never be connected
to live operational systems, plant systems, safety-critical infrastructure,
real-time plant monitoring, or institutional production systems, and its
values are not authoritative for any operational, licensing or safety purpose.

```rust
pub mod server { /* ... */ }
```

### Functions

#### Function `default_ciet_server_config`

The CIET v2 server configuration with CIET's own defaults filled in.

Bind every interface on [`DEFAULT_OPCUA_PORT`](super::node_map::DEFAULT_OPCUA_PORT)
(4840), announce over mDNS, and call ourselves
`"CIET Educational Simulator v2"`.

Binding all interfaces is the default because the demonstration is "connect
from the phone in your hand". It is also the configuration that exposes every
control to the network, which is why the simulator prints a warning in that
case — pass `bind_address: "127.0.0.1".to_owned()` to keep it on this
machine.

```ignore
// loopback only, no announcement -- the safe default for a shared network
let config = OpcuaServerConfig {
    bind_address: "127.0.0.1".to_owned(),
    advertise_over_mdns: false,
    ..default_ciet_server_config()
};
```

```rust
pub fn default_ciet_server_config() -> OpcuaServerConfig { /* ... */ }
```

#### Function `spawn_opcua_server_thread`

Start the CIET v2 OPC-UA server on its own thread.

A thin binding of
[`opcua_core::server::spawn_opcua_server_thread`](crate::opcua_core::server::spawn_opcua_server_thread)
to [`CietNode`]: it spawns a dedicated `std::thread` with its own
multi-threaded `tokio` runtime, builds the server, populates its address
space from [`super::node_map`], wires the callbacks, and serves connections.
It blocks only until the server thread reports whether construction succeeded
(a certificate load and 36 node insertions), then optionally announces over
mDNS and returns. The signature is synchronous, so a GUI can call it with no
`async` in sight.

`state` is read to serve reads; `user_controls` receives writes, which the
physics thread applies on its next timestep (see the module docs). Neither
lock is held across an `await`.

# What lands in the address space

Under `Objects`, two folders: `Outputs` holds one read-only `Double` per
[`CietSignal`](super::node_map::CietSignal); `Controls` holds one writable
`Double` per [`CietControl`](super::node_map::CietControl) and one writable
`Boolean` per [`CietSwitch`](super::node_map::CietSwitch). Node ids are
`ns=<index>;s=<node_identifier()>`, with display names and unit-bearing
descriptions from the node map. The namespace index is assigned at start-up
(2 in this configuration) — clients must resolve it, never hard-code it.

# Security

There is none, deliberately: `SecurityPolicy::None` and anonymous access, so
**anyone who can reach the port can write every control**. Requests are still
clamped on apply. Read the module documentation before running this anywhere
but a bench.

# Errors

See [`OpcuaServerError`]. A failure to announce over mDNS is *not* an error —
it is reported and the server runs regardless.

```rust
pub fn spawn_opcua_server_thread(state: super::state::SharedCietState, user_controls: super::user_controls::SharedUserControls, config: OpcuaServerConfig) -> Result<OpcuaServerHandle, OpcuaServerError> { /* ... */ }
```

### Re-exports

#### Re-export `OpcuaEndpointInfo`

```rust
pub use crate::opcua_core::server::OpcuaEndpointInfo;
```

#### Re-export `OpcuaServerConfig`

```rust
pub use crate::opcua_core::server::OpcuaServerConfig;
```

#### Re-export `OpcuaServerError`

```rust
pub use crate::opcua_core::server::OpcuaServerError;
```

#### Re-export `OpcuaServerHandle`

```rust
pub use crate::opcua_core::server::OpcuaServerHandle;
```

#### Re-export `SUBSCRIPTION_PUSH_INTERVAL`

```rust
pub use crate::opcua_core::server::SUBSCRIPTION_PUSH_INTERVAL;
```

## Module `simulator`

What CIET v2 supplies to the shared OPC-UA layer.

[`opcua_core`](crate::opcua_core) serves any OUTRAM PARK digital twin; this
module is CIET's half of that contract, and nothing here is transport code:

| Supplied | Item | Meaning |
|---|---|---|
| who CIET is | [`CietOpcuaSimulator`] / [`CIET_OPCUA_PROFILE`] | namespace URI, endpoint path, PKI directory name, mDNS marker, ... |
| what CIET publishes | [`CietNode`] | one variant per OPC-UA variable, wrapping the three [`super::node_map`] enums |

[`CietNode`] is where the node map meets the wire: it says how each variable
is read out of a [`CietState`] snapshot and how a client's write is recorded
in a [`CietUserControls`](super::user_controls::CietUserControls) mailbox.
Adding a variable is still a matter of adding one variant to the node map —
the compiler then points at every `match` arm here that must be updated.

## Units

Values cross into OPC-UA as bare `Variant`s, so the engineering unit travels
in the variable's *description* text (see [`CietNode::description`]), which
is what a client displays next to the number. The units themselves are
defined once, in [`super::node_map`].

## Scope (`RESPONSIBLE_USE.md`)

CIET v2 is an **offline educational simulator**. This interface must never be
connected to live operational systems, plant systems, safety-critical
infrastructure, real-time plant monitoring, or institutional production
systems.

```rust
pub mod simulator { /* ... */ }
```

### Types

#### Struct `CietOpcuaSimulator`

The CIET Educational Simulator v2, as the shared OPC-UA layer sees it.

A zero-sized marker: it carries no data and costs nothing at runtime. It
exists so shared types can be bound to CIET at compile time — that is what
makes [`super::discovery::SimulatorBrowser`] find CIET simulators and nothing
else.

```rust
pub struct CietOpcuaSimulator;
```

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CietOpcuaSimulator { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **OpcuaSimulator**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CietOpcuaSimulator) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `CietNode`

One CIET variable, tagged by which node-map enum it came from.

Enum dispatch rather than a trait object, per the workspace Rust design
rules: a fourth kind of variable becomes a compile error at every `match`
instead of a runtime surprise.

```rust
pub enum CietNode {
    Signal(super::node_map::CietSignal),
    Control(super::node_map::CietControl),
    Switch(super::node_map::CietSwitch),
}
```

##### Variants

###### `Signal`

A read-only output, served as an OPC-UA `Double`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::node_map::CietSignal` |  |

###### `Control`

A writable continuous control, served as an OPC-UA `Double`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::node_map::CietControl` |  |

###### `Switch`

A writable on/off control, served as an OPC-UA `Boolean`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::node_map::CietSwitch` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CietNode { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **OpcuaVariable**
  - ```rust
    fn all() -> Vec<Self> { /* ... */ }
    ```
    Every CIET variable, outputs first, in address-space order.

  - ```rust
    fn node_identifier(self: &Self) -> &'static str { /* ... */ }
    ```
    The string part of this variable's `NodeId`.

  - ```rust
    fn browse_name(self: &Self) -> &'static str { /* ... */ }
    ```
    Short OPC-UA browse name (one path segment).

  - ```rust
    fn display_name(self: &Self) -> &'static str { /* ... */ }
    ```
    Human-facing label.

  - ```rust
    fn description(self: &Self) -> String { /* ... */ }
    ```
    Description shown by a client, naming the engineering unit and, for a

  - ```rust
    fn data_type(self: &Self) -> DataTypeId { /* ... */ }
    ```
    OPC-UA data type: `Double` for the continuous variables, `Boolean` for

  - ```rust
    fn access_level(self: &Self) -> AccessLevel { /* ... */ }
    ```
    Access level: outputs are read-only, controls and switches are writable.

  - ```rust
    fn folder(self: &Self) -> OpcuaFolder { /* ... */ }
    ```
    Outputs are filed under the outputs folder, controls and switches under

  - ```rust
    fn read(self: &Self, state: &CietState) -> Variant { /* ... */ }
    ```
    Read this variable out of a plant-state snapshot.

  - ```rust
    fn record_write(self: &Self, requests: &mut CietUserControls, value: DataValue) -> StatusCode { /* ... */ }
    ```
    Record a client's write as a pending request, never applying it here.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CietNode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `record_control_request`

Record a client's write to a continuous control as a *pending request*.

The write is **not** applied to [`CietState`] here; it is parked in
[`CietUserControls`] and applied by the physics thread on its next timestep,
which is where clamping and NaN rejection happen (see
[`super::user_controls`] for why). The client therefore reads back the
*effective* value: write 1000 kW, read back the 15 kW ceiling.

`value` carries the control's own unit (see [`CietControl::unit`]).

Returns `BadTypeMismatch` for a non-numeric payload and `BadNothingToDo` for
no payload.

```rust
pub fn record_control_request(requests: &mut super::user_controls::CietUserControls, control: super::node_map::CietControl, value: opcua::types::DataValue) -> opcua::types::StatusCode { /* ... */ }
```

#### Function `record_switch_request`

Record a client's write to an on/off control as a *pending request*.

Same deferred-apply contract as [`record_control_request`]. Strictly typed:
only an OPC-UA `Boolean` is accepted, because a switch has no meaningful
numeric interpretation and silently treating `0.0` as `false` would hide a
client bug.

```rust
pub fn record_switch_request(requests: &mut super::user_controls::CietUserControls, switch: super::node_map::CietSwitch, value: opcua::types::DataValue) -> opcua::types::StatusCode { /* ... */ }
```

#### Function `record_control_request_shared`

Take the request mailbox's write lock and record a control request through
it, reporting `BadInternalError` if the lock is poisoned.

A convenience for callers holding a [`SharedUserControls`] rather than the
mailbox itself; the shared layer's write callback does the same thing.

```rust
pub fn record_control_request_shared(user_controls: &super::user_controls::SharedUserControls, control: super::node_map::CietControl, value: opcua::types::DataValue) -> opcua::types::StatusCode { /* ... */ }
```

#### Function `record_switch_request_shared`

Take the request mailbox's write lock and record a switch request through
it, reporting `BadInternalError` if the lock is poisoned.

```rust
pub fn record_switch_request_shared(user_controls: &super::user_controls::SharedUserControls, switch: super::node_map::CietSwitch, value: opcua::types::DataValue) -> opcua::types::StatusCode { /* ... */ }
```

### Constants and Statics

#### Constant `CIET_APPLICATION_URI`

Base OPC-UA application URI. The port is appended per instance.

**This must never equal [`CIET_NAMESPACE_URI`].** `async-opcua`'s diagnostics
node manager registers the application URI as *its own* namespace and claims
every node at that index (`owns_node` is `id.namespace == self.namespace_index`).
Identical strings resolve to one index, so the diagnostics manager would
shadow the whole CIET namespace and every CIET read would return
`BadNodeIdUnknown` despite the nodes being present and browsable. Keeping them
distinct is what makes the server's own namespace index 1 and CIET's index 2,
as [`super::node_map`] documents.

```rust
pub const CIET_APPLICATION_URI: &str = "urn:outram-park:ciet-educational-simulator-v2:server";
```

#### Constant `CIET_DEFAULT_APPLICATION_NAME`

Default human-facing OPC-UA application name.

```rust
pub const CIET_DEFAULT_APPLICATION_NAME: &str = "CIET Educational Simulator v2";
```

#### Constant `CIET_OUTPUTS_FOLDER_NODE_ID`

Node id (string identifier) of the folder holding the read-only outputs.

```rust
pub const CIET_OUTPUTS_FOLDER_NODE_ID: &str = "CIET.Outputs";
```

#### Constant `CIET_CONTROLS_FOLDER_NODE_ID`

Node id (string identifier) of the folder holding the writable controls.

```rust
pub const CIET_CONTROLS_FOLDER_NODE_ID: &str = "CIET.Controls";
```

#### Constant `CIET_NODE_MANAGER_NAME`

Name given to the CIET node manager inside `async-opcua`.

```rust
pub const CIET_NODE_MANAGER_NAME: &str = "ciet";
```

#### Constant `CIET_LOG_PREFIX`

Prefix the shared layer stamps on CIET's console lines, giving
`"CIET v2 OPC-UA: ..."` and `"CIET v2 mDNS: ..."`.

```rust
pub const CIET_LOG_PREFIX: &str = "CIET v2";
```

#### Constant `CIET_OPCUA_PROFILE`

Every naming and identity string CIET v2's OPC-UA interface is built from.

This is the one place those strings are collected; the shared layer reads
them and never hard-codes anything CIET-shaped. None of them is a physical
quantity, so none carries a unit.

```rust
pub const CIET_OPCUA_PROFILE: crate::opcua_core::simulator::OpcuaSimulatorProfile = _;
```

## Module `state`

The shared plant state of the CIET Educational Simulator v2.

[`CietState`] is a flat, `Clone`-able snapshot of every quantity the
simulator either **accepts as a control input** or **publishes as an
output**. It is the single rendezvous point between three threads:

| Thread | Access | Role |
|---|---|---|
| Physics | read controls, write outputs | integrates the CIET loop one timestep at a time |
| OPC-UA server | write controls, read outputs | serves remote clients (IEC 62541) |
| GUI (desktop only) | write controls, read outputs | the egui front end |

Shared as [`SharedCietState`] = `Arc<RwLock<CietState>>`. `RwLock` rather
than `Mutex` per the workspace Rust design rules: the GUI and the OPC-UA
read callbacks are both readers and must not serialise against each other
while the physics thread is between writes.

## Provenance

The field set is carried over from the CIET Educational Simulator **v1**
`CIETState`
(`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/ciet_data.rs`),
deliberately keeping the same names so the v2 physics port stays a faithful
translation rather than a re-derivation. Field semantics, the 150 degC
heater killswitch thresholds and the 2-decimal-place rounding of published
temperatures are all v1 behaviour.

## What v2 adds over v1

The **frequency-response / advanced heater control settings live here**, in
shared state. In v1 they lived inside the `eframe::App` struct and were
applied from the GUI repaint callback, which means they could only be driven
by a human at the keyboard. Moving them into shared state is what lets an
OPC-UA client — or the headless Termux build, which has no GUI at all —
drive a frequency-response experiment. The signal itself is evaluated once
per timestep by the physics thread, not per repaint.

## Status

Per `RESPONSIBLE_USE.md` this is **untrusted draft material until
human-reviewed**, and the simulator is an **offline educational
demonstration** — not for facility operation, reactor control,
safety-critical decisions, or licensing.

```rust
pub mod state { /* ... */ }
```

### Types

#### Type Alias `SharedCietState`

The CIET plant state, shared between the physics, OPC-UA and GUI threads.

See the [module documentation](self) for the threading contract. Cloning is
cheap-ish (it is a flat `Copy`-of-scalars struct with one small enum) and is
the intended way to take a consistent snapshot without holding the lock:
`let snapshot = shared.read().unwrap().clone();`

```rust
pub type SharedCietState = std::sync::Arc<std::sync::RwLock<CietState>>;
```

#### Enum `HeaterType`

Which discretisation of the CIET heater the physics thread should integrate.

CIET's heater is a porous-media annular test section. The two options trade
axial resolution against real-time performance; on a slower machine (or on
Termux) the coarse mesh keeps the simulation in real time.

Carried over unchanged from v1's `HeaterType`.

```rust
pub enum HeaterType {
    InsulatedHeaterV1Fine15Mesh,
    InsulatedHeaterV1Coarse8Mesh,
}
```

##### Variants

###### `InsulatedHeaterV1Fine15Mesh`

15 axial nodes (13 heated + 2 head nodes). Default; better resolved.

###### `InsulatedHeaterV1Coarse8Mesh`

8 axial nodes (6 heated + 2 head nodes). Cheaper; for slower hardware.

##### Implementations

###### Methods

- ```rust
  pub fn axial_node_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of axial fluid nodes in the heated section for this mesh choice.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HeaterType { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut fmt::Formatter<''_>) -> fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeaterType) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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

- **ToSmolStr**
  - ```rust
    fn to_smolstr(self: &Self) -> SmolStr { /* ... */ }
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
- **WithSubscriber**
#### Struct `HeaterControlSettings`

Advanced heater-control settings: steady power plus an optional sinusoidal
perturbation, for frequency-response (Bode) experiments.

The perturbation is

`P(t) = steady_state_power_kw + total_amplitude_kw * sin(omega * t)`

with `omega = angular_velocity_rad_per_s` and `t` the **simulation** time,
not wall-clock time. Ported from v1's `FreqResponseAndTransientSettings`
with the same formula; what changed is *where* it is evaluated (see the
module docs).

Frequency-response testing of CIET's heater is the experimental technique of
De Wet and Poresky (Bode plots of heater-outlet temperature against heater
power); this reproduces the input side of that experiment in simulation.
**No validation against their published data has been performed here** —
that remains open work.

```rust
pub struct HeaterControlSettings {
    pub advanced_heater_control_switched_on: bool,
    pub frequency_response_switched_on: bool,
    pub steady_state_power_kw: f64,
    pub total_amplitude_kw: f64,
    pub angular_velocity_rad_per_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `advanced_heater_control_switched_on` | `bool` | Master switch. When `false` the heater power is whatever was last<br>written to [`CietState::heater_power_kilowatts`] directly, and the<br>settings below are ignored. |
| `frequency_response_switched_on` | `bool` | When `true`, add the sinusoidal perturbation to the steady power. |
| `steady_state_power_kw` | `f64` | Steady-state (mean) heater power. Valid range 0..=15 kW; CIET's heater<br>is rated to about 10 kW, and the simulator's killswitch trips on<br>over-temperature well before this ceiling matters. |
| `total_amplitude_kw` | `f64` | Peak amplitude of the sinusoidal perturbation, kW. Valid range 0..=4. |
| `angular_velocity_rad_per_s` | `f64` | Angular frequency of the perturbation, rad/s. Valid range 0..=10. |

##### Implementations

###### Methods

- ```rust
  pub fn demanded_power(self: &Self, current_sim_time: Time) -> Power { /* ... */ }
  ```
  Heater power demanded at simulation time `current_sim_time`.

- ```rust
  pub fn sine_wave_label(self: &Self) -> String { /* ... */ }
  ```
  Human-readable description of the perturbation signal, for the UI.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HeaterControlSettings { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeaterControlSettings) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `CietState`

Flat snapshot of the whole CIET loop: controls in, measurements out.

Naming follows CIET's instrument tags so that a reader who knows the
facility can find things: `BT-11` is a bulk-temperature thermocouple,
`FM-40` a flowmeter, and bare `pipe_N` fields are the simulated bulk
temperature of loop component `N` from the CIET nodalisation diagram.
Temperatures are degrees Celsius, mass flows kg/s, power kW, pressure Pa.

`f32` on the `pipe_*` fields (and `f64` on the instrumented `bt_*`/`fm_*`
fields) is v1's choice, kept for fidelity: the `pipe_*` values feed the
schematic colouring, the instrumented values feed plots and OPC-UA.

```rust
pub struct CietState {
    pub simulation_time_seconds: f64,
    pub elapsed_time_seconds: f64,
    pub calc_time_ms: f64,
    pub heater_power_kilowatts: f64,
    pub ctah_pump_temp_degc: f64,
    pub bt_41_ctah_outlet_set_pt_deg_c: f64,
    pub bt_66_tchx_outlet_set_pt_deg_c: f64,
    pub pipe_18_temp_degc: f32,
    pub pipe_1a_temp_degc: f32,
    pub bt_11_heater_inlet_deg_c: f64,
    pub bt_12_heater_outlet_deg_c: f64,
    pub pipe_1b_temp_degc: f32,
    pub pipe_2a_temp_degc: f32,
    pub pipe_2_temp_degc: f32,
    pub pipe_3_temp_degc: f32,
    pub pipe_4_temp_degc: f32,
    pub pipe_5a_temp_degc: f32,
    pub pipe_26_temp_degc: f32,
    pub pipe_25_temp_degc: f32,
    pub pipe_25a_temp_degc: f32,
    pub bt_21_dhx_shell_inlet_deg_c: f64,
    pub pipe_23_temp_degc: f32,
    pub bt_27_dhx_shell_outlet_deg_c: f64,
    pub pipe_23a_temp_degc: f32,
    pub pipe_22_temp_degc: f32,
    pub fm20_label_21a_temp_degc: f32,
    pub fm20_dhx_branch_kg_per_s: f32,
    pub pipe_21_temp_degc: f32,
    pub pipe_20_temp_degc: f32,
    pub pipe_19_temp_degc: f32,
    pub pipe_17b_temp_degc: f32,
    pub bt_21_dhx_tube_outlet_deg_c: f64,
    pub pipe_30b_temp_degc: f32,
    pub pipe_31a_temp_degc: f32,
    pub pipe_31_temp_degc: f32,
    pub pipe_32_temp_degc: f32,
    pub pipe_33_temp_degc: f32,
    pub pipe_34_temp_degc: f32,
    pub bt_65_tchx_inlet_deg_c: f64,
    pub bt_66_tchx_outlet_deg_c: f64,
    pub tchx_htc_watt_per_m2_kelvin: f64,
    pub pipe_36a_temp_degc: f32,
    pub pipe_36_temp_degc: f32,
    pub pipe_37_temp_degc: f32,
    pub fm_60_dracs_kg_per_s: f64,
    pub fm60_label_37a_temp_degc: f32,
    pub pipe_38_temp_degc: f32,
    pub pipe_39_temp_degc: f32,
    pub pipe_30a_temp_degc: f32,
    pub bt_60_dhx_tube_inlet_deg_c: f64,
    pub pipe_5b_temp_degc: f32,
    pub pipe_6a_temp_degc: f32,
    pub pipe_6_temp_degc: f32,
    pub bt_43_ctah_inlet_deg_c: f64,
    pub bt_41_ctah_outlet_deg_c: f64,
    pub ctah_htc_watt_per_m2_kelvin: f64,
    pub pipe_8a_temp_degc: f32,
    pub pipe_8_temp_degc: f32,
    pub pipe_9_temp_degc: f32,
    pub pipe_10_temp_degc: f32,
    pub pipe_11_temp_degc: f32,
    pub pipe_12_temp_degc: f32,
    pub pipe_13_temp_degc: f32,
    pub pipe_14_temp_degc: f32,
    pub fm40_label_14a_temp_degc: f32,
    pub fm40_ctah_branch_kg_per_s: f64,
    pub pipe_15_temp_degc: f32,
    pub pipe_16_temp_degc: f32,
    pub pipe_17a_temp_degc: f32,
    pub top_mixing_node_5a_5b_4_temp_degc: f32,
    pub bottom_mixing_node_17a_17b_18_temp_degc: f32,
    pub timestep_seconds: f32,
    pub fast_forward_settings_turned_on: bool,
    pub slow_motion_settings_turned_on: bool,
    pub ctah_pump_pressure_pascals: f32,
    pub is_ctah_branch_blocked: bool,
    pub is_dhx_branch_blocked: bool,
    pub current_heater_type: HeaterType,
    pub heater_control: HeaterControlSettings,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `simulation_time_seconds` | `f64` | Simulated time elapsed, seconds. Advances by the timestep each loop. |
| `elapsed_time_seconds` | `f64` | Wall-clock time elapsed since the physics thread started, seconds.<br>Compare with `simulation_time_seconds` to see whether the simulation is<br>keeping up with real time. |
| `calc_time_ms` | `f64` | Wall-clock cost of the last timestep, milliseconds. |
| `heater_power_kilowatts` | `f64` | Electrical power into the heater, kW. Written by the GUI, by an OPC-UA<br>client, or by the advanced-heater-control driver. Forced to zero by the<br>over-temperature killswitch. |
| `ctah_pump_temp_degc` | `f64` | CTAH pump fluid temperature, degC (diagnostic readout). |
| `bt_41_ctah_outlet_set_pt_deg_c` | `f64` | Set point for the CTAH outlet temperature (BT-41), degC. Drives the<br>CTAH's PID-controlled air-side heat transfer coefficient. |
| `bt_66_tchx_outlet_set_pt_deg_c` | `f64` | Set point for the TCHX outlet temperature (BT-66), degC. Drives the<br>TCHX's PID-controlled air-side heat transfer coefficient. |
| `pipe_18_temp_degc` | `f32` | Pipe 18 bulk temperature, degC. |
| `pipe_1a_temp_degc` | `f32` | Heater top head (1a) bulk temperature, degC. |
| `bt_11_heater_inlet_deg_c` | `f64` | BT-11, heater inlet bulk temperature, degC. |
| `bt_12_heater_outlet_deg_c` | `f64` | BT-12, heater outlet bulk temperature, degC. |
| `pipe_1b_temp_degc` | `f32` | Heater bottom head (1b) bulk temperature, degC. |
| `pipe_2a_temp_degc` | `f32` | Pipe 2a bulk temperature, degC. |
| `pipe_2_temp_degc` | `f32` | Static mixer 10 (label 2) bulk temperature, degC. |
| `pipe_3_temp_degc` | `f32` | Pipe 3 bulk temperature, degC. |
| `pipe_4_temp_degc` | `f32` | Pipe 4 bulk temperature, degC. |
| `pipe_5a_temp_degc` | `f32` | Pipe 5a bulk temperature, degC. |
| `pipe_26_temp_degc` | `f32` | Pipe 26 bulk temperature, degC. |
| `pipe_25_temp_degc` | `f32` | Static mixer 21 (label 25) bulk temperature, degC. |
| `pipe_25a_temp_degc` | `f32` | Pipe 25a bulk temperature, degC. |
| `bt_21_dhx_shell_inlet_deg_c` | `f64` | BT-21 on the DHX shell inlet, degC. |
| `pipe_23_temp_degc` | `f32` | Static mixer 20 (label 23) bulk temperature, degC. |
| `bt_27_dhx_shell_outlet_deg_c` | `f64` | BT-27 on the DHX shell outlet, degC. |
| `pipe_23a_temp_degc` | `f32` | Pipe 23a bulk temperature, degC. |
| `pipe_22_temp_degc` | `f32` | Pipe 22 bulk temperature, degC. |
| `fm20_label_21a_temp_degc` | `f32` | Flowmeter FM-20 (label 21a) fluid temperature, degC. |
| `fm20_dhx_branch_kg_per_s` | `f32` | FM-20, DHX-branch mass flowrate, kg/s. |
| `pipe_21_temp_degc` | `f32` | Pipe 21 bulk temperature, degC. |
| `pipe_20_temp_degc` | `f32` | Pipe 20 bulk temperature, degC. |
| `pipe_19_temp_degc` | `f32` | Pipe 19 bulk temperature, degC. |
| `pipe_17b_temp_degc` | `f32` | Pipe 17b bulk temperature, degC. |
| `bt_21_dhx_tube_outlet_deg_c` | `f64` | BT-21 on the DHX tube outlet, degC. |
| `pipe_30b_temp_degc` | `f32` | DHX tube side 30b bulk temperature, degC. |
| `pipe_31a_temp_degc` | `f32` | Pipe 31a bulk temperature, degC. |
| `pipe_31_temp_degc` | `f32` | Static mixer 61 (label 31) bulk temperature, degC. |
| `pipe_32_temp_degc` | `f32` | Pipe 32 bulk temperature, degC. |
| `pipe_33_temp_degc` | `f32` | Pipe 33 bulk temperature, degC. |
| `pipe_34_temp_degc` | `f32` | Pipe 34 bulk temperature, degC. |
| `bt_65_tchx_inlet_deg_c` | `f64` | BT-65, TCHX inlet bulk temperature, degC. |
| `bt_66_tchx_outlet_deg_c` | `f64` | BT-66, TCHX outlet bulk temperature, degC. The PID-controlled variable. |
| `tchx_htc_watt_per_m2_kelvin` | `f64` | TCHX air-side heat transfer coefficient, W/(m^2 K), as commanded by the<br>TCHX PID controller. Output only. |
| `pipe_36a_temp_degc` | `f32` | Pipe 36a bulk temperature, degC. |
| `pipe_36_temp_degc` | `f32` | Static mixer 60 (label 36) bulk temperature, degC. |
| `pipe_37_temp_degc` | `f32` | Pipe 37 bulk temperature, degC. |
| `fm_60_dracs_kg_per_s` | `f64` | FM-60, DRACS-loop mass flowrate, kg/s (absolute value; v1 solves the<br>DRACS loop for magnitude only, so reverse flow is not represented). |
| `fm60_label_37a_temp_degc` | `f32` | Flowmeter FM-60 (label 37a) fluid temperature, degC. |
| `pipe_38_temp_degc` | `f32` | Pipe 38 bulk temperature, degC. |
| `pipe_39_temp_degc` | `f32` | Pipe 39 bulk temperature, degC. |
| `pipe_30a_temp_degc` | `f32` | DHX tube side 30a bulk temperature, degC. |
| `bt_60_dhx_tube_inlet_deg_c` | `f64` | BT-60 on the DHX tube inlet, degC. |
| `pipe_5b_temp_degc` | `f32` | Pipe 5b bulk temperature, degC. |
| `pipe_6a_temp_degc` | `f32` | Pipe 6a bulk temperature, degC. |
| `pipe_6_temp_degc` | `f32` | Static mixer 41 (label 6) bulk temperature, degC. |
| `bt_43_ctah_inlet_deg_c` | `f64` | BT-43, CTAH inlet bulk temperature, degC. |
| `bt_41_ctah_outlet_deg_c` | `f64` | BT-41, CTAH outlet bulk temperature, degC. The PID-controlled variable. |
| `ctah_htc_watt_per_m2_kelvin` | `f64` | CTAH air-side heat transfer coefficient, W/(m^2 K), as commanded by the<br>CTAH PID controller. Output only. |
| `pipe_8a_temp_degc` | `f32` | Pipe 8a bulk temperature, degC. |
| `pipe_8_temp_degc` | `f32` | Static mixer 40 (label 8) bulk temperature, degC. |
| `pipe_9_temp_degc` | `f32` | Pipe 9 bulk temperature, degC. |
| `pipe_10_temp_degc` | `f32` | Pipe 10 bulk temperature, degC. |
| `pipe_11_temp_degc` | `f32` | Pipe 11 bulk temperature, degC. |
| `pipe_12_temp_degc` | `f32` | Pipe 12 bulk temperature, degC. |
| `pipe_13_temp_degc` | `f32` | Pipe 13 bulk temperature, degC. |
| `pipe_14_temp_degc` | `f32` | Pipe 14 bulk temperature, degC. |
| `fm40_label_14a_temp_degc` | `f32` | Flowmeter FM-40 (label 14a) fluid temperature, degC. |
| `fm40_ctah_branch_kg_per_s` | `f64` | FM-40, CTAH-branch mass flowrate, kg/s. Signed: negative means reverse<br>(upward) flow through the CTAH branch. |
| `pipe_15_temp_degc` | `f32` | Pipe 15 bulk temperature, degC. |
| `pipe_16_temp_degc` | `f32` | Pipe 16 bulk temperature, degC. |
| `pipe_17a_temp_degc` | `f32` | Pipe 17a bulk temperature, degC. |
| `top_mixing_node_5a_5b_4_temp_degc` | `f32` | Top mixing node joining branches 5a, 5b and 4, degC. |
| `bottom_mixing_node_17a_17b_18_temp_degc` | `f32` | Bottom mixing node joining branches 17a, 17b and 18, degC. |
| `timestep_seconds` | `f32` | User-requested timestep, seconds. Only honoured when<br>`slow_motion_settings_turned_on` is `true`, and clamped to 0.1 s<br>regardless — larger steps break the Courant-number stability limit. |
| `fast_forward_settings_turned_on` | `bool` | Run faster than real time where the machine allows it. |
| `slow_motion_settings_turned_on` | `bool` | Run slower than real time, honouring `timestep_seconds`. |
| `ctah_pump_pressure_pascals` | `f32` | Pressure rise across the CTAH pump, Pa. This is the forced-circulation<br>driver. Valid range -17000..=17000 Pa; negative values reverse the flow. |
| `is_ctah_branch_blocked` | `bool` | Block flow through the CTAH branch (as if a valve were shut). |
| `is_dhx_branch_blocked` | `bool` | Block flow through the DHX branch. |
| `current_heater_type` | `HeaterType` | Which heater discretisation the physics thread integrates. |
| `heater_control` | `HeaterControlSettings` | Advanced heater control / frequency-response settings. v2 addition to<br>shared state — see the module docs. |

##### Implementations

###### Methods

- ```rust
  pub fn overwrite_state(self: &mut Self, ciet_state: Self) { /* ... */ }
  ```
  Overwrite this state wholesale with another snapshot.

- ```rust
  pub fn get_heater_power_kilowatts(self: &Self) -> f64 { /* ... */ }
  ```
  Heater electrical power, kW.

- ```rust
  pub fn set_heater_power_kilowatts(self: &mut Self, heater_power_kw: f64) { /* ... */ }
  ```
  Set the heater electrical power, kW, clamped to

- ```rust
  pub fn get_heater_outlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-12, heater outlet temperature, degC.

- ```rust
  pub fn get_heater_inlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-11, heater inlet temperature, degC.

- ```rust
  pub fn get_dhx_shell_outlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-27, DHX shell outlet temperature, degC.

- ```rust
  pub fn get_dhx_shell_inlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-21, DHX shell inlet temperature, degC.

- ```rust
  pub fn get_dhx_tube_outlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-21, DHX tube outlet temperature, degC.

- ```rust
  pub fn get_dhx_tube_inlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-60, DHX tube inlet temperature, degC.

- ```rust
  pub fn get_tchx_outlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-66, TCHX outlet temperature, degC.

- ```rust
  pub fn get_tchx_inlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-65, TCHX inlet temperature, degC.

- ```rust
  pub fn get_ctah_outlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-41, CTAH outlet temperature, degC.

- ```rust
  pub fn get_ctah_inlet_temp_degc(self: &Self) -> f64 { /* ... */ }
  ```
  BT-43, CTAH inlet temperature, degC.

- ```rust
  pub fn get_timestep_seconds(self: &Self) -> f32 { /* ... */ }
  ```
  Requested timestep, seconds.

- ```rust
  pub fn set_timestep_seconds(self: &mut Self, timestep_seconds: f64) { /* ... */ }
  ```
  Set the requested timestep, seconds, clamped to

- ```rust
  pub fn is_fast_fwd_on(self: &Self) -> bool { /* ... */ }
  ```
  Whether fast-forward pacing is on.

- ```rust
  pub fn get_ctah_pump_pressure_f64(self: &Self) -> f64 { /* ... */ }
  ```
  CTAH pump pressure rise, Pa, as an `f64`.

- ```rust
  pub fn set_ctah_pump_pressure_pascals(self: &mut Self, pressure_pascals: f64) { /* ... */ }
  ```
  Set the CTAH pump pressure rise, Pa, clamped to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CietState { /* ... */ }
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
    Isothermal 21 degC loop at rest: no heater power, no pump pressure, no

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CietState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `new_shared_state`

Build a fresh shared state handle at defaults (isothermal 21 degC, at rest).

```rust
pub fn new_shared_state() -> SharedCietState { /* ... */ }
```

### Constants and Statics

#### Constant `CTAH_PUMP_PRESSURE_LIMIT_PASCALS`

Hard bound on the CTAH pump pressure rise, Pa.

CIET's CTAH pump cannot deliver more than about 17 kPa; the simulator
enforces the same envelope in both directions so a client cannot drive the
loop into a non-physical regime. v1 applied this bound in its GUI slider.

```rust
pub const CTAH_PUMP_PRESSURE_LIMIT_PASCALS: f64 = 17000.0;
```

#### Constant `HEATER_POWER_LIMIT_KILOWATTS`

Hard bound on requested heater power, kW.

CIET's heater is rated near 10 kW; 15 kW leaves headroom for transient
experiments while keeping the killswitch (150 degC at heater inlet/outlet,
160 degC in any fluid node, 350 degC in any shell node) as the real
protection.

```rust
pub const HEATER_POWER_LIMIT_KILOWATTS: f64 = 15.0;
```

#### Constant `MAX_TIMESTEP_SECONDS`

Largest timestep the solver is allowed to take, seconds.

Above this the advection Courant number in the shortest loop component
exceeds unity and the explicit advection coupling goes unstable. Enforced by
the physics thread regardless of what a client requests.

```rust
pub const MAX_TIMESTEP_SECONDS: f64 = 0.1;
```

## Module `user_controls`

Pending remote control requests, held separately from the plant state.

This is the fix for a **lost-update race**. The obvious design — have OPC-UA
write callbacks mutate [`CietState`] directly — puts remote clients, the GUI
and the physics thread all in contention for the same struct, and whichever
writer stores last wins. v1's GUI made that concrete: it cloned the whole
state early in an egui repaint, mutated two control fields on the clone, then
stored the whole clone back (`overwrite_state`) about twenty times a second.
In v1 that was almost harmless, because the state is a *publication surface*
and the physics thread re-derives its outputs from its own component objects
every timestep. Add remote control and it stops being harmless: a client
write landing between the GUI's clone and its store is silently discarded,
the write still returns `Good`, and there is nothing on the client side to
diagnose.

So remote intent lives here instead, in its own small struct:

```text
 OPC-UA write callback ──> CietUserControls (pending requests)
                                   │
                                   │  apply_and_clear(), once per timestep
                                   v
 GUI writes ─────────────────> CietState ──> physics reads controls,
                                   ^          writes outputs
                                   └── OPC-UA read callbacks (outputs)
```

Three properties fall out of that:

1. **No clobbering.** Whatever the GUI does to [`CietState`], it cannot erase
   a remote request, because the request is not stored there. It is re-applied
   at the top of the next timestep.
2. **No contention with physics.** A write callback locks this small struct,
   not the plant state, so a room full of clients writing set points does not
   serialise against the solver or the repaint.
3. **Requests are sparse.** Each field is an `Option`, so a client that never
   touches the heater power does not fight the GUI slider for ownership of
   it. Only what was actually written is applied, and each request is
   consumed once.

## Semantics

Last-write-wins per field between successive timesteps: if two clients write
the same control 5 ms apart, the physics thread sees only the later value.
That is the honest behaviour of a shared plant with no interlocks, and it
matches what a real facility with two operators at two panels would do.
Values are clamped on the way in by [`CietControl::write`], so a pending
request is always inside the documented envelope.

Read-back for a control still comes from [`CietState`] — the *effective*
value the solver is using — not from the pending request. A client that
writes 1000 kW and reads back 15 kW is being told the truth about what the
simulator is doing.

## Scope

Per `RESPONSIBLE_USE.md` this is part of an **offline educational
demonstration** interface, never to be connected to live operational
systems, plant systems, or safety-critical infrastructure. There are no
interlocks, no authority model, and no arbitration between clients here,
because there is no operational role for any of that in a teaching demo.

```rust
pub mod user_controls { /* ... */ }
```

### Types

#### Struct `CietUserControls`

Pending control requests from remote (OPC-UA) clients.

One `Option` slot per [`CietControl`] and per [`CietSwitch`], indexed by
their `index()` methods, so adding a control to the node map automatically
gets a slot here with no second place to update. `None` means "no request
outstanding".

```rust
pub struct CietUserControls {
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
  An empty request store: nothing pending, nothing applied yet.

- ```rust
  pub fn request_control(self: &mut Self, control: CietControl, value: f64) { /* ... */ }
  ```
  Record a client's request to set `control` to `value`.

- ```rust
  pub fn request_switch(self: &mut Self, switch: CietSwitch, value: bool) { /* ... */ }
  ```
  Record a client's request to set `switch` to `value`.

- ```rust
  pub fn pending_control(self: &Self, control: CietControl) -> Option<f64> { /* ... */ }
  ```
  The outstanding request for `control`, if any.

- ```rust
  pub fn pending_switch(self: &Self, switch: CietSwitch) -> Option<bool> { /* ... */ }
  ```
  The outstanding request for `switch`, if any.

- ```rust
  pub fn has_pending_requests(self: &Self) -> bool { /* ... */ }
  ```
  Whether anything is waiting to be applied.

- ```rust
  pub fn applied_request_count(self: &Self) -> u64 { /* ... */ }
  ```
  Total requests applied since start-up. Diagnostic only.

- ```rust
  pub fn apply_and_clear(self: &mut Self, state: &mut CietState) -> usize { /* ... */ }
  ```
  Apply every outstanding request to `state`, then clear them.

- ```rust
  pub fn clear_without_applying(self: &mut Self) { /* ... */ }
  ```
  Discard every outstanding request without applying it.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CietUserControls { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CietUserControls) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Type Alias `SharedUserControls`

Shared handle to the pending-request store.

`RwLock` rather than `Mutex` for consistency with the rest of the interface
layer, though contention here is negligible by construction: the critical
sections are a single array slot write and a once-per-timestep drain.

```rust
pub type SharedUserControls = std::sync::Arc<std::sync::RwLock<CietUserControls>>;
```

### Functions

#### Function `new_shared_user_controls`

Build a fresh shared pending-request store.

```rust
pub fn new_shared_user_controls() -> SharedUserControls { /* ... */ }
```

### Re-exports

#### Re-export `CietControl`

```rust
pub use node_map::CietControl;
```

#### Re-export `CietSignal`

```rust
pub use node_map::CietSignal;
```

#### Re-export `CietSwitch`

```rust
pub use node_map::CietSwitch;
```

#### Re-export `CIET_NAMESPACE_URI`

```rust
pub use node_map::CIET_NAMESPACE_URI;
```

#### Re-export `DEFAULT_OPCUA_PORT`

```rust
pub use node_map::DEFAULT_OPCUA_PORT;
```

#### Re-export `ENDPOINT_PATH`

```rust
pub use node_map::ENDPOINT_PATH;
```

#### Re-export `CietNode`

```rust
pub use simulator::CietNode;
```

#### Re-export `CietOpcuaSimulator`

```rust
pub use simulator::CietOpcuaSimulator;
```

#### Re-export `CIET_OPCUA_PROFILE`

```rust
pub use simulator::CIET_OPCUA_PROFILE;
```

#### Re-export `CietState`

```rust
pub use state::CietState;
```

#### Re-export `HeaterControlSettings`

```rust
pub use state::HeaterControlSettings;
```

#### Re-export `HeaterType`

```rust
pub use state::HeaterType;
```

#### Re-export `SharedCietState`

```rust
pub use state::SharedCietState;
```

## Module `opcua_core`

Reactor-agnostic OPC-UA (IEC 62541) server layer for OUTRAM PARK digital
twins.

This is the half of an OPC-UA interface that has nothing to do with any
particular plant: the TCP transport, the server thread and its `tokio`
runtime, the PKI directory, mDNS announcement and browsing, address-space
construction, the read/write callbacks and the subscription push. A
simulator supplies only **who it is** and **what it publishes**; everything
else lives here and is written once.

## Layout

| Module | Role |
|---|---|
| [`simulator`] | the seam — [`OpcuaSimulator`] (identity) and [`OpcuaVariable`] (variables, snapshot, requests) |
| [`server`] | the server, run on its own thread with its own tokio runtime |
| [`pki`] | where the certificate store lives (`~/.outram-park/...`) |
| [`discovery`] | cooperative mDNS announce (server) and browse (client) |

## Adding a second simulator

Read [`simulator`] first — it is the whole contract. In outline:

1. Declare a marker type and give it an [`OpcuaSimulatorProfile`] (namespace
   URI, endpoint path, PKI directory name, mDNS marker, ...).
2. Declare a `Copy` enum whose variants are the variables, and implement
   [`OpcuaVariable`] on it, naming the snapshot type reads come from and the
   request type writes are parked in.
3. Wrap [`server::spawn_opcua_server_thread`] in a concrete per-simulator
   function, so callers never write a turbofish.

`ciet_opcua` is the worked example of all three steps.

## Compile-time dispatch only

Both traits are used as generic bounds — no `Box<dyn Trait>`, no
`&dyn Trait`, per the workspace `CLAUDE.md` Rust design rules. There are no
lifetime parameters anywhere in this module; shared state is
`Arc<RwLock<T>>`.

## Portability

No GUI, no physics. `async-opcua` is pure Rust (RustCrypto, not
`openssl-sys`), so this module builds on Android/Termux with no target gate
— a headless Termux build serves OPC-UA exactly as a desktop one does.

## Security: there is none, deliberately

Servers built here run with **`SecurityPolicy::None` and anonymous access**.
Anyone who can reach the port can read every output and write every control.
That is a deliberate choice for throwaway teaching demonstrators, and it is
why a warning banner is printed whenever a server binds to anything other
than loopback. Hardening (certificates, trust lists, user tokens, audit
trails) is explicitly **out of scope** and left to security researchers. Do
not describe anything built on this as secured.

## Scope limit (`RESPONSIBLE_USE.md`)

OPC-UA is a plant-connectivity protocol, so the boundary matters: this layer
exists so **offline educational simulators** can be driven by standard
OPC-UA tooling on a bench or in a classroom. It must **never** be connected
to live operational systems, plant systems, safety-critical infrastructure,
real-time plant monitoring, or institutional production systems, and its
outputs are not authoritative for any operational, licensing or safety
purpose.

```rust
pub mod opcua_core { /* ... */ }
```

### Modules

## Module `discovery`

Cooperative mDNS / DNS-SD announcement and browsing, for any simulator.

The problem this solves: a student runs a simulator on a laptop, a
demonstrator runs a client on a phone, and neither wants to type an IP
address. So the **server volunteers its presence** on the local link with a
standard DNS-SD announcement, and the **client listens** for it.

| Direction | Entry point |
|---|---|
| Server announces itself | [`advertise_simulator`] → [`MdnsAdvertisement`] |
| Client listens for servers | [`MdnsBrowser::start`] → [`MdnsBrowser::discovered`] |

Both are bound to one simulator at compile time through
[`OpcuaSimulator`], which supplies the service marker, the endpoint path and
the instance-name prefix. A browser therefore reports **only** the simulator
it was parameterised with, even though `_opcua-tcp._tcp` is shared by every
OPC-UA server on the link.

## This is announcement only — never scanning

**The only network traffic this module originates is a multicast DNS-SD
announcement of *this* machine's own service, and multicast queries for the
`_opcua-tcp._tcp` service type.** Nothing here probes, sweeps, enumerates or
fingerprints another host. A server that has not chosen to announce itself is
simply not discovered, and that is the correct behaviour.

**Do not add a port scanner, a subnet sweeper, an ARP/ICMP host prober, or
any "just try every address in the /24" loop to this module or anywhere else
in this crate.** Unsolicited scanning of a network you do not administer
breaches institutional acceptable-use policy, and it is out of scope per the
workspace `RESPONSIBLE_USE.md` — this project is for education, research and
V&V, not for network reconnaissance. If discovery does not work on a given
network, the supported answer is "type the URL in by hand", not "find it by
probing".

## Practical caveat: many networks break this

mDNS is a link-local multicast protocol, and a great many networks
deliberately stop it working:

- **Campus and enterprise WiFi** commonly enable **client isolation** (also
  sold as "AP isolation" / "peer-to-peer blocking"), so two devices on the
  same SSID cannot reach each other at all — no multicast, and no direct TCP
  either. Discovery fails, *and* so does the subsequent OPC-UA connection,
  even with a hand-typed URL.
- Many managed networks **filter multicast / UDP 5353** outright, or place
  wired and wireless clients in different VLANs.
- Guest networks, VPNs and container bridge networks routinely do both.

A home router or a phone hotspot generally works fine, and so does a single
machine talking to itself over loopback. When demonstrating on institutional
WiFi, expect to need a hotspot. This is a property of the network, not a bug
in this module, and no amount of code here can fix it.

## Units

Everything here is transport metadata — host names, ports, IP addresses,
DNS labels. No physical quantities, no units.

## Scope (`RESPONSIBLE_USE.md`)

What gets announced is an **offline educational simulator**. This module must
never be used to discover, enumerate or connect to live operational systems,
plant systems, safety-critical infrastructure or institutional production
systems.

```rust
pub mod discovery { /* ... */ }
```

### Types

#### Struct `DiscoveredSimulator`

A simulator that announced itself on the local link.

Every field is transport metadata — host names, ports, IP addresses. No
physical quantity and no units are involved.

```rust
pub struct DiscoveredSimulator {
    pub instance_name: String,
    pub host: String,
    pub port: u16,
    pub endpoint_url: String,
    pub addresses: Vec<std::net::IpAddr>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `instance_name` | `String` | DNS-SD instance name as announced, e.g.<br>`"CIET-Educational-Simulator-v2-bench3"`. This is what a user picks from<br>in a list. |
| `host` | `String` | mDNS host name of the machine running the simulator, e.g.<br>`"ciet-bench3.local."`. |
| `port` | `u16` | TCP port the OPC-UA server listens on. Normally 4840. |
| `endpoint_url` | `String` | Ready-to-use OPC-UA endpoint URL, e.g.<br>`"opc.tcp://192.168.1.42:4840/ciet"`.<br><br>Built from the first announced IPv4 address where there is one (an IP<br>literal connects on more networks than an mDNS host name, which needs a<br>working `.local.` resolver), otherwise from [`Self::host`]. The path<br>comes from the announcement's `path` TXT record. |
| `addresses` | `Vec<std::net::IpAddr>` | Every IP address announced for the service, in the order IPv4 first then<br>IPv6. Useful when the first-choice address is unreachable. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DiscoveredSimulator { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DiscoveredSimulator) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `MdnsAdvertisement`

A live mDNS announcement of a running server, which is withdrawn when
dropped.

Keep it alive for as long as the server is listening: dropping it unregisters
the service and shuts down the mDNS daemon thread, so clients stop seeing the
simulator. There are no lifetime parameters — the value owns everything it
needs.

```rust
pub struct MdnsAdvertisement {
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
  pub fn instance_name(self: &Self) -> &str { /* ... */ }
  ```
  The DNS-SD instance name actually announced.

- ```rust
  pub fn fullname(self: &Self) -> &str { /* ... */ }
  ```
  The fully-qualified DNS-SD service name, e.g.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Drop**
  - ```rust
    fn drop(self: &mut Self) { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **WithSubscriber**
#### Struct `MdnsBrowser`

Listens for announcements from simulator `S` on the local link.

Construct once with [`start`](Self::start), then poll
[`discovered`](Self::discovered) whenever convenient — from an egui repaint,
from a CLI loop, from anywhere. Polling never blocks, so it is safe to call
at frame rate.

The type parameter is the *filter*: a `MdnsBrowser<CietOpcuaSimulator>` sees
only CIET simulators, even on a link crowded with other OPC-UA servers.
Bind it once with a type alias (`type SimulatorBrowser =
MdnsBrowser<CietOpcuaSimulator>;`) and callers never name the parameter
again.

There are no lifetime parameters: the browser owns its daemon, its event
receiver, and an `Arc<RwLock<..>>` of everything found so far.

```rust
pub struct MdnsBrowser<S: OpcuaSimulator> {
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
  pub fn start() -> Result<Self, DiscoveryError> { /* ... */ }
  ```
  Start listening for announcements from simulator `S`.

- ```rust
  pub fn discovered(self: &Self) -> Vec<DiscoveredSimulator> { /* ... */ }
  ```
  Every simulator found so far, sorted by instance name.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Drop**
  - ```rust
    fn drop(self: &mut Self) { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **WithSubscriber**
#### Enum `DiscoveryError`

Things that can go wrong while announcing or browsing over mDNS.

None of these is fatal to a simulator or a client: OPC-UA still works with a
hand-typed URL. Report them, do not abort on them.

```rust
pub enum DiscoveryError {
    DaemonStart(String),
    Register(String),
    Browse(String),
}
```

##### Variants

###### `DaemonStart`

No mDNS daemon could be started — typically no usable multicast
interface, or UDP 5353 already taken by a system responder that will not
share it.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Register`

The service announcement was rejected, e.g. an instance name that is
still invalid after sanitisation, or a TXT record over the 255-byte
per-record limit.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Browse`

The browse operation could not be started.

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToSmolStr**
  - ```rust
    fn to_smolstr(self: &Self) -> SmolStr { /* ... */ }
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
- **WithSubscriber**
### Functions

#### Function `advertise_simulator`

Announce simulator `S` on the local link over mDNS / DNS-SD.

`port` is the TCP port the OPC-UA server listens on (normally 4840).
`instance_name` is a short human-facing label; it is sanitised to
DNS-SD-safe characters and, if empty after sanitisation, replaced by the
simulator's
[`mdns_instance_prefix`](super::simulator::OpcuaSimulatorProfile::mdns_instance_prefix).

The announcement carries two TXT records so a browser can tell this
simulator apart from any other OPC-UA server on the link:

```text
path=<profile.endpoint_path>
product=<profile.mdns_product_marker>
```

Addresses are filled in automatically by `mdns-sd` (`enable_addr_auto`), and
tracked as interfaces come and go, so a laptop that moves from ethernet to
WiFi keeps announcing a reachable address.

Keep the returned [`MdnsAdvertisement`] alive for as long as the server runs;
dropping it withdraws the announcement.

# Errors

Returns [`DiscoveryError::DaemonStart`] if no mDNS daemon can be started (no
usable multicast interface, or UDP 5353 unavailable), and
[`DiscoveryError::Register`] if the announcement itself is rejected. Neither
is fatal to a simulator: the OPC-UA server still works, users just have to
type the URL.

```rust
pub fn advertise_simulator<S: OpcuaSimulator>(port: u16, instance_name: &str) -> Result<MdnsAdvertisement, DiscoveryError> { /* ... */ }
```

### Constants and Statics

#### Constant `OPCUA_MDNS_SERVICE_TYPE`

DNS-SD service type an OPC-UA server announces itself under.

`_opcua-tcp._tcp` is the service type registered with IANA for OPC-UA over
TCP, and is what OPC-UA tooling (UaExpert's "local discovery", for instance)
already looks for. The trailing `.local.` is the mDNS domain.

Because this is the *generic* OPC-UA type, any other OPC-UA server on the
link will also appear under it — which is why every announcement also carries
the simulator's
[`mdns_product_marker`](super::simulator::OpcuaSimulatorProfile::mdns_product_marker)
TXT record and [`MdnsBrowser::discovered`] filters on it.

```rust
pub const OPCUA_MDNS_SERVICE_TYPE: &str = "_opcua-tcp._tcp.local.";
```

#### Constant `PATH_TXT_KEY`

TXT record key carrying the OPC-UA endpoint path, e.g. `/ciet`.

A client needs this to build the full endpoint URL, because DNS-SD gives it
only a host and a port.

```rust
pub const PATH_TXT_KEY: &str = "path";
```

#### Constant `PRODUCT_TXT_KEY`

TXT record key carrying the product marker that identifies which simulator
is announcing.

```rust
pub const PRODUCT_TXT_KEY: &str = "product";
```

## Module `pki`

Where an OUTRAM PARK OPC-UA server keeps its PKI directory.

`async-opcua` needs a writable directory to hold its **application instance
certificate**. This module decides where that directory lives, so a
simulator never scatters `./pki` folders into whatever working directory it
happened to be launched from. It is reactor-agnostic: the only per-simulator
input is the directory *name*, which comes from
[`OpcuaSimulatorProfile::pki_dir_name`](super::simulator::OpcuaSimulatorProfile::pki_dir_name).

## Layout

| Platform | Root ([`outram_park_home`]) | PKI dir ([`pki_dir`]) |
|---|---|---|
| Linux / macOS / Termux | `$HOME/.outram-park` | `$HOME/.outram-park/<pki_dir_name>` |
| Windows | `%APPDATA%\outram-park` | `%APPDATA%\outram-park\<pki_dir_name>` |

A single dot-directory under the user's home was chosen by the maintainer so
every OUTRAM PARK tool that needs persistent scratch space has one obvious
place to put it, with the conventional Windows equivalent (`%APPDATA%`,
i.e. `directories`' `BaseDirs::data_dir()`).

Inside the PKI directory, `async-opcua` populates the usual OPC-UA
certificate-store subtree itself on first run:

```text
<pki_dir_name>/
  own/cert.der          <- this server's self-signed instance certificate
  private/private.pem   <- the matching private key
  trusted/              <- client certificates the server would trust
  rejected/             <- client certificates it has seen and refused
```

## Nothing sensitive is stored here

The servers built on [`opcua_core`](super) run with `SecurityPolicy::None`
and anonymous access (see [`super::server`]). No channel is ever encrypted or
signed with the keypair, no client certificate is ever validated against the
trust list, and no user credential of any kind is written. What lands on disk
is therefore a **throwaway self-signed keypair** that authenticates nothing —
deleting the whole directory costs nothing but a regeneration on next
start-up. Do not describe it as a credential store, and do not reuse the key
for anything.

## No credentials, ever (`RESPONSIBLE_USE.md`)

This module creates a directory and reports its path. It must never grow
code that reads institutional credentials, API keys, access tokens, or any
certificate belonging to a real facility or production system.

## Units

Everything here is a filesystem path or a name. No physical quantities, no
units.

```rust
pub mod pki { /* ... */ }
```

### Functions

#### Function `outram_park_home`

The OUTRAM PARK per-user root directory, created if it does not exist.

Returns `$HOME/.outram-park` on Linux, macOS and Termux, and
`%APPDATA%\outram-park` on Windows. This is a **filesystem path**, not a
physical quantity — it carries no units.

## Resolution order

1. `directories::BaseDirs` — `data_dir()` on Windows, `home_dir()`
   elsewhere. This is the normal path on every supported platform.
2. The `HOME` environment variable, if `BaseDirs` could not be constructed
   (it returns `None` when no home directory can be determined at all).
3. The current directory (`.`), as a last resort so the server can still
   start in a stripped-down container or sandbox.

The directory is created with `std::fs::create_dir_all` if missing. A
creation failure is **not** an error here: the path is still returned and a
warning is printed, because `async-opcua` will report the real problem when
it tries to write its certificate. That keeps a read-only home directory
from taking the whole simulator down.

```rust
pub fn outram_park_home() -> std::path::PathBuf { /* ... */ }
```

#### Function `outram_park_home_path`

Work out the OUTRAM PARK per-user root **without creating it**.

Same resolution order as [`outram_park_home`], no filesystem side effects.
Use it to display or test a path on a machine whose home directory may be
read-only.

```rust
pub fn outram_park_home_path() -> std::path::PathBuf { /* ... */ }
```

#### Function `pki_dir`

The PKI directory for one simulator, created if it does not exist.

This is `<`[`outram_park_home`]`>/<dir_name>`, where `dir_name` is the
simulator's
[`pki_dir_name`](super::simulator::OpcuaSimulatorProfile::pki_dir_name).
Pass the result straight to `ServerBuilder::pki_dir`; `async-opcua` creates
the `own/`, `private/`, `trusted/` and `rejected/` subdirectories itself and
writes a self-signed application instance certificate into `own/` on first
start-up.

As with [`outram_park_home`], a creation failure is warned about rather than
returned as an error.

```rust
pub fn pki_dir(dir_name: &str) -> std::path::PathBuf { /* ... */ }
```

#### Function `instance_pki_dir`

A per-instance PKI directory underneath [`pki_dir`], created if missing.

Returns `<`[`pki_dir`]`>/<sanitised instance_tag>`.

## Why this exists: parallel instances clobber a shared certificate store

`async-opcua` writes its self-signed keypair into the PKI directory on
start-up. Two servers starting **concurrently** against the same directory
race on `own/cert.der` and `private/private.pem`, and can read a
half-written file — a real hazard for headless simulator tests, which may run
several servers at once, and for a developer running a simulator while a test
suite runs.

The tag makes each instance's store disjoint. [`server`](super::server)
derives it from the TCP port, which is the one thing two servers that can
coexist on a machine must differ in, so isolation is automatic and needs no
configuration. Tests that want a stronger guarantee — a fresh directory per
run rather than per port — can pass [`unique_instance_tag`].

`instance_tag` is sanitised to ASCII alphanumerics, `-` and `_`; anything
else becomes `-`, and an empty result becomes `"default"`. That keeps a
caller from escaping the directory with `../` or breaking on a path
separator.

```rust
pub fn instance_pki_dir(dir_name: &str, instance_tag: &str) -> std::path::PathBuf { /* ... */ }
```

#### Function `unique_instance_tag`

A short tag that is unique to this process **and** this thread, for isolating
a PKI directory (or any other per-instance scratch path) in parallel tests.

The tag is `<process id>-<thread id digits>-<nanoseconds>`, e.g.
`"18342-7-913204771"`. It is deliberately not cryptographic — it only has to
stop two concurrent instances picking the same directory.

Cargo runs `#[test]` functions as threads inside one process, so the process
id alone does not separate them; the thread id and a nanosecond timestamp do.
Note that each call returns a **new** value, so capture it once per instance
rather than calling it repeatedly.

```rust
pub fn unique_instance_tag() -> String { /* ... */ }
```

#### Function `describe_pki_location`

A one-line, human-readable summary of where a simulator's PKI directory is,
for a "how to connect" panel or a start-up log line.

The wording deliberately states that nothing sensitive is stored, so a
reader of the GUI is not misled into thinking the interface is secured.

# Example output

```text
PKI directory: /home/alice/.outram-park/ciet-v2-opcua-pki (self-signed keypair only -- SecurityPolicy::None stores no credentials)
```

```rust
pub fn describe_pki_location(dir_name: &str) -> String { /* ... */ }
```

### Constants and Statics

#### Constant `UNIX_HOME_DIR_NAME`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"windows\"), span: crates/outram-park-digital-twin-engine/src/opcua_core/pki.rs:60:11: 60:32 (#0) }, crates/outram-park-digital-twin-engine/src/opcua_core/pki.rs:60:10: 60:33 (#0))])]")`

Directory name of the OUTRAM PARK per-user root, on Linux/macOS/Termux.

Dot-prefixed, because it sits directly in `$HOME`.

```rust
pub const UNIX_HOME_DIR_NAME: &str = ".outram-park";
```

## Module `server`

The shared OPC-UA server: address space, callbacks, and its own thread.

[`spawn_opcua_server_thread`] is the whole entry point. Give it the shared
plant state, the shared remote-write mailbox and an [`OpcuaServerConfig`],
and it returns an [`OpcuaServerHandle`] describing where clients should
connect. Everything else here supports that one call.

Nothing in this module knows what a reactor is. It is parameterised by the
simulator's [`OpcuaVariable`] enum, which supplies the variables, the
snapshot type they are read from, and the request type writes are recorded
in — see [`super::simulator`] for the seam.

```text
physics thread ──write outputs, apply pending requests──┐
GUI thread ─────write controls─────────────────────────►├─► Arc<RwLock<V::Snapshot>>
                                                        │        ▲        ▲
OPC-UA server thread   read callbacks ──────────────────────read──┘        │
(own tokio runtime)    200 ms updater ──────────────────────read───────────┘
                       write callbacks ──► Arc<RwLock<V::Requests>>
```

## Threading

The server runs on its **own `std::thread` with its own multi-threaded
`tokio` runtime**, created inside that thread. It shares nothing with a GUI's
event loop, so a stalled repaint cannot stall an OPC-UA client and a headless
build (Termux, CI) can serve OPC-UA with no GUI at all. Nothing here panics on
failure: a server that cannot start reports why and the simulator carries on
without it.

## Reads are live; writes are deferred

**Reads** are served straight from the snapshot under a read lock, so a
client always sees the *effective* value the solver is using — write 1000 kW
and read back the 15 kW ceiling.

**Writes** do not touch plant state. They are parked in the simulator's
request mailbox ([`OpcuaVariable::Requests`]) and applied by the physics
thread at the top of its next timestep, which is where clamping and NaN
rejection happen. That removes the lost-update race against a GUI's wholesale
state overwrite, and keeps a room full of clients off the plant-state lock.

## Why a periodic push as well as read callbacks

A `Read` service call is served by a read callback, so polling is never
stale. A **subscription / monitored item**, though, reports when the value
stored *in the address space* changes — so a task pushes current values in
every [`SUBSCRIPTION_PUSH_INTERVAL`] (200 ms) via `set_values`, which is what
makes trending work in a standard OPC-UA client.

## Security: there is none, deliberately

`ServerBuilder::new_anonymous` gives one endpoint with **`SecurityPolicy::None`,
`MessageSecurityMode::None` and anonymous user tokens**: traffic is
unencrypted and unsigned, no client certificate is checked, no credential is
required, and therefore **anyone who can reach the TCP port can write every
control**.

That is a deliberate choice for throwaway teaching demonstrators, so that
"point UaExpert at it and poke the loop" is a ten-second exercise. Hardening
(certificates, trust lists, user tokens, audit trails) is explicitly left to
security researchers rather than half-done here.

The only mitigations you should rely on: a simulator is expected to **clamp**
every request on apply and ignore NaN, so a hostile client can annoy the
simulation but not destabilise it; the bind address can be set to loopback
([`OpcuaServerConfig::is_loopback_only`]); and a warning is printed whenever
it binds wider. Do not describe this interface as secured, and do not run it
on a network you do not control.

## Units

Nothing here is a physical quantity. Values cross this layer as OPC-UA
`Variant`s whose engineering unit is documented by the simulator's own node
map; the only dimensioned constant in the module is
[`SUBSCRIPTION_PUSH_INTERVAL`], which is wall-clock time.

## Scope (`RESPONSIBLE_USE.md`)

This serves **offline educational simulators**. It must never be connected to
live operational systems, plant systems, safety-critical infrastructure,
real-time plant monitoring, or institutional production systems, and its
values are not authoritative for any operational, licensing or safety
purpose.

```rust
pub mod server { /* ... */ }
```

### Types

#### Struct `OpcuaServerConfig`

How a simulator's OPC-UA server should be brought up.

All four fields are transport / naming configuration; none is a physical
quantity, so none carries units. There is deliberately **no `Default`**:
build one with [`for_simulator`](Self::for_simulator), which seeds the
application name and port from the simulator's own profile, then override
what you need. A neutral default would silently advertise the wrong
simulator's name.

```ignore
// loopback only, no announcement -- the safe default for a shared network
let config = OpcuaServerConfig {
    bind_address: "127.0.0.1".to_owned(),
    advertise_over_mdns: false,
    ..OpcuaServerConfig::for_simulator::<MySimulator>()
};
```

```rust
pub struct OpcuaServerConfig {
    pub bind_address: String,
    pub port: u16,
    pub advertise_over_mdns: bool,
    pub application_name: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bind_address` | `String` | Local address to bind the listening socket to.<br><br>`"0.0.0.0"` accepts connections on every IPv4 interface, so other<br>machines on the same network can connect — which is the point of the<br>demo, and also the case in which **anyone who can reach the port can<br>write every control** (see the module docs). `"127.0.0.1"` keeps the<br>interface on this machine; [`is_loopback_only`](Self::is_loopback_only)<br>reports which of the two you have. |
| `port` | `u16` | TCP port to listen on.<br><br>[`for_simulator`](Self::for_simulator) seeds this from the simulator's<br>`default_port`, normally 4840 — the IANA-registered `opcua-tcp` port,<br>which is where OPC-UA tooling looks first. Use something above 1024 and<br>unregistered if it is taken. |
| `advertise_over_mdns` | `bool` | Whether to announce the running server on the local link over mDNS /<br>DNS-SD, so a client can find it without a typed URL.<br><br>Announcement is cooperative and one-way — see [`super::discovery`],<br>which explains both that this never scans anything and that many<br>campus/enterprise networks block it outright. A failure to announce is<br>logged and otherwise ignored; the server still runs. |
| `application_name` | `String` | OPC-UA `ApplicationName`, shown by clients in their server list and in<br>the endpoint description.<br><br>Also used, sanitised, as the mDNS instance name. |

##### Implementations

###### Methods

- ```rust
  pub fn for_simulator<S: OpcuaSimulator>() -> Self { /* ... */ }
  ```
  The configuration a simulator ships with: bind every interface on its

- ```rust
  pub fn is_loopback_only(self: &Self) -> bool { /* ... */ }
  ```
  `true` if [`bind_address`](Self::bind_address) reaches this machine only.

- ```rust
  pub fn binds_all_interfaces(self: &Self) -> bool { /* ... */ }
  ```
  `true` if [`bind_address`](Self::bind_address) is an all-interfaces

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OpcuaServerConfig { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `OpcuaEndpointInfo`

Where a client should point, and what it will find when it gets there.

Produced by [`spawn_opcua_server_thread`] and displayed by a simulator's
"how to connect" panel. Pure connection metadata — no physical quantities, no
units.

```rust
pub struct OpcuaEndpointInfo {
    pub loopback_url: String,
    pub lan_url: Option<String>,
    pub bound_to_all_interfaces: bool,
    pub namespace_uri: &'static str,
    pub node_count: usize,
    pub pki_dir_display: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `loopback_url` | `String` | Endpoint URL that always works from this machine, e.g.<br>`"opc.tcp://127.0.0.1:4840/ciet"`. |
| `lan_url` | `Option<String>` | Endpoint URL other machines on the same network should use, e.g.<br>`"opc.tcp://192.168.1.42:4840/ciet"`.<br><br>`None` when this machine's LAN address could not be determined (no<br>non-loopback interface, or the query failed). It is also **not** a promise<br>of reachability: a network with client isolation will refuse the<br>connection anyway — see [`super::discovery`]. |
| `bound_to_all_interfaces` | `bool` | Whether the listening socket was bound to every interface rather than to<br>one specific address.<br><br>When `false`, [`primary_url`](Self::primary_url) reports the loopback URL,<br>because a server bound to `127.0.0.1` is genuinely not reachable from<br>elsewhere no matter what LAN address the machine holds. |
| `namespace_uri` | `&'static str` | Namespace URI every one of this simulator's variables lives in, i.e. its<br>profile's `namespace_uri`.<br><br>A client resolves this to a running namespace *index* (usually 2) from the<br>server's namespace array; the index is not stable across versions and must<br>not be hard-coded. |
| `node_count` | `usize` | Number of variables served, i.e. `V::all().len()`. |
| `pki_dir_display` | `String` | The PKI directory, ready to print. See [`super::pki`] — it holds a<br>self-signed keypair and nothing sensitive. |

##### Implementations

###### Methods

- ```rust
  pub fn primary_url(self: &Self) -> &str { /* ... */ }
  ```
  The URL to show a user first.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OpcuaEndpointInfo { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `OpcuaServerHandle`

A running OPC-UA server.

Holding this value keeps the server up: it owns the `async-opcua` server
handle and, when mDNS announcement is enabled, the announcement guard. No
lifetime parameters — everything is owned or shared through `Arc`.

Dropping the handle withdraws the mDNS announcement (its guard's `Drop`) but
does **not** stop the server; call [`shutdown`](Self::shutdown) for that.
That split is deliberate: a simulator's GUI keeps the handle for the whole
process lifetime and never wants an accidental move to kill the interface.

```rust
pub struct OpcuaServerHandle {
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
  pub fn endpoint_info(self: &Self) -> &OpcuaEndpointInfo { /* ... */ }
  ```
  Where clients should connect, and what they will find.

- ```rust
  pub fn shutdown(self: &Self) { /* ... */ }
  ```
  Ask the server to stop.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **WithSubscriber**
#### Enum `OpcuaServerError`

Things that can stop an OPC-UA server from starting.

Every one of these is a start-up failure. A failure to *announce* over mDNS
is not in this list on purpose: it is logged and the server runs anyway,
because discovery is a convenience and the endpoint still works with a typed
URL.

```rust
pub enum OpcuaServerError {
    Build(String),
    NodeManagerUnavailable,
    NamespaceUnavailable(&'static str),
    AddressSpace(String),
    ThreadSpawn(std::io::Error),
    Runtime(String),
    StartupAborted,
}
```

##### Variants

###### `Build`

`async-opcua` rejected the configuration, or could not read/write its
certificate store. The string is its own message.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NodeManagerUnavailable`

The simulator's node manager was not present on the built server. This
would mean `with_node_manager` did not take effect, i.e. an `async-opcua`
version change rather than a user error.

###### `NamespaceUnavailable`

The simulator's namespace URI was not registered, so no node id could be
formed. Same class of cause as
[`NodeManagerUnavailable`](Self::NodeManagerUnavailable). The payload is
the URI that was expected.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `AddressSpace`

A folder or variable could not be inserted into the address space —
almost always a duplicate node id, which a simulator's own
"node identifiers are unique" test is there to prevent.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `ThreadSpawn`

The operating system refused to start the server thread.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::io::Error` |  |

###### `Runtime`

The server thread could not create its own `tokio` runtime — normally a
thread or file-descriptor limit.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `StartupAborted`

The server thread ended before reporting whether construction succeeded,
which means it panicked. The panic message itself is on stderr.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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
    fn from(source: std::io::Error) -> Self { /* ... */ }
    ```

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToSmolStr**
  - ```rust
    fn to_smolstr(self: &Self) -> SmolStr { /* ... */ }
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
- **WithSubscriber**
### Functions

#### Function `spawn_opcua_server_thread`

Start simulator `V`'s OPC-UA server on its own thread.

Spawns a dedicated `std::thread` with its own multi-threaded `tokio` runtime;
that thread builds the server, populates its address space from
[`V::all()`](OpcuaVariable::all), wires the callbacks, and serves
connections. This function blocks only until the thread reports whether
construction succeeded (a certificate load and one insertion per variable),
then optionally announces over mDNS and returns.

**The build must happen on the server thread**, not here: `ServerBuilder::build`
spawns a tokio task internally (`ServerStatusWrapper::new`) and panics with
"there is no reactor running" outside a runtime. The result therefore comes
back over a `std::sync::mpsc` channel, which is what keeps this signature
synchronous so a GUI can call it with no `async` in sight.

`state` is read to serve reads; `requests` receives writes, which the physics
thread applies on its next timestep (see the module docs). Neither lock is
held across an `await`.

The type parameter is not inferable from the arguments (they are its
associated types), so call it with a turbofish —
`spawn_opcua_server_thread::<CietNode>(..)` — or wrap it in a concrete
per-simulator function, which is what `ciet_opcua::server` does.

# What lands in the address space

Under `Objects`, two folders named by the simulator's profile: the outputs
folder holds every [`OpcuaFolder::Outputs`] variable, the controls folder
every [`OpcuaFolder::Controls`] one. Node ids are
`ns=<index>;s=<node_identifier()>`, with display names and unit-bearing
descriptions from the variable. The namespace index is assigned at start-up
(2 in this configuration) — clients must resolve it, never hard-code it.

# Security

There is none, deliberately: `SecurityPolicy::None` and anonymous access, so
**anyone who can reach the port can write every control**. Read the module
documentation before running this anywhere but a bench.

# Errors

See [`OpcuaServerError`]. A failure to announce over mDNS is *not* an error —
it is reported and the server runs regardless.

```rust
pub fn spawn_opcua_server_thread<V: OpcuaVariable>(state: std::sync::Arc<std::sync::RwLock<<V as >::Snapshot>>, requests: std::sync::Arc<std::sync::RwLock<<V as >::Requests>>, config: OpcuaServerConfig) -> Result<OpcuaServerHandle, OpcuaServerError> { /* ... */ }
```

### Constants and Statics

#### Constant `SUBSCRIPTION_PUSH_INTERVAL`

How often current plant values are pushed into the address space so OPC-UA
**subscriptions and monitored items** report changes.

200 ms of wall-clock time. This is a notification cadence, not a solver
timestep — it has no effect whatsoever on the physics, and a polling `Read`
is always served live regardless of this interval.

```rust
pub const SUBSCRIPTION_PUSH_INTERVAL: std::time::Duration = _;
```

## Module `simulator`

The seam: what a simulator supplies, and what the shared layer owns.

[`opcua_core`](super) serves *any* OUTRAM PARK digital twin over OPC-UA. To
be served, a simulator supplies exactly two things:

| It supplies | Trait | Meaning |
|---|---|---|
| **who it is** | [`OpcuaSimulator`] | one [`OpcuaSimulatorProfile`] of naming/identity strings (namespace URI, endpoint path, mDNS marker, ...) |
| **what it publishes** | [`OpcuaVariable`] | one `Copy` enum whose variants are its variables, plus the snapshot and request types they read and write |

Everything else — the TCP transport, the tokio runtime and its thread, the
PKI directory, mDNS announcement, address-space construction, the read and
write callbacks and the subscription push — belongs to the shared layer and
is written once.

## Compile-time dispatch, no trait objects

Both traits are used as **generic bounds**, never as `Box<dyn Trait>` or
`&dyn Trait` (workspace `CLAUDE.md`, Rust design rules). The simulator's
variable type is a plain enum, so adding a variable is a compile error at
every `match` arm rather than a runtime surprise, and the whole address
space is monomorphised with no dynamic dispatch and no heap indirection.

## Physical quantities and units

Nothing in this module is a physical quantity. Node identifiers, browse
names and folder names are OPC-UA naming strings; the *values* a simulator
publishes carry their engineering unit in the variable's
[`description`](OpcuaVariable::description) text, which is what an OPC-UA
client displays. Unit correctness therefore lives in the simulator's own
node map, not here.

## Scope (`RESPONSIBLE_USE.md`)

OPC-UA is a plant-connectivity protocol, so the boundary matters: this layer
exists so **offline educational simulators** can be driven by standard
OPC-UA tooling on a bench or in a classroom. Nothing built on it may be
connected to live operational systems, plant systems, safety-critical
infrastructure, real-time plant monitoring, or institutional production
systems, and its values are not authoritative for any operational, licensing
or safety purpose.

```rust
pub mod simulator { /* ... */ }
```

### Types

#### Struct `OpcuaSimulatorProfile`

The naming and identity strings that make one simulator's OPC-UA interface
distinguishable from another's.

Every field is a *name*, not a physical quantity, so none carries a unit.
A simulator declares one of these as a `const` and returns it from
[`OpcuaSimulator::PROFILE`]; the shared layer reads it wherever it would
otherwise have hard-coded a string.

## The one rule that will bite you

**[`application_uri`](Self::application_uri) must never equal
[`namespace_uri`](Self::namespace_uri).** `async-opcua`'s diagnostics node
manager registers the application URI as *its own* namespace and claims
every node at that index (`owns_node` is `id.namespace ==
self.namespace_index`). Identical strings resolve to one index, so the
diagnostics manager would shadow the simulator's whole namespace and every
read would return `BadNodeIdUnknown` despite the nodes being present and
browsable.

```rust
pub struct OpcuaSimulatorProfile {
    pub namespace_uri: &'static str,
    pub application_uri: &'static str,
    pub endpoint_path: &'static str,
    pub default_application_name: &'static str,
    pub default_port: u16,
    pub node_manager_name: &'static str,
    pub outputs_folder_name: &'static str,
    pub outputs_folder_node_id: &'static str,
    pub controls_folder_name: &'static str,
    pub controls_folder_node_id: &'static str,
    pub pki_dir_name: &'static str,
    pub mdns_instance_prefix: &'static str,
    pub mdns_product_marker: &'static str,
    pub log_prefix: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `namespace_uri` | `&'static str` | Namespace URI every one of this simulator's variables lives in, e.g.<br>`"urn:outram-park:ciet-educational-simulator-v2"`.<br><br>A client resolves this to a running namespace *index* (usually 2) from<br>the server's namespace array; the index is not stable across versions<br>and must not be hard-coded by a client. |
| `application_uri` | `&'static str` | Base OPC-UA `ApplicationUri` / `ProductUri`. The TCP port is appended to<br>the application URI per instance, so two simulators on one machine do<br>not present the same application identity.<br><br>Must differ from [`namespace_uri`](Self::namespace_uri) — see the type<br>documentation. |
| `endpoint_path` | `&'static str` | Endpoint path appended to the server URL, e.g. `"/ciet"`, giving<br>`opc.tcp://<host>:<port>/ciet`. Announced in the mDNS `path` TXT record<br>so a discovering client can rebuild the full URL. |
| `default_application_name` | `&'static str` | Human-facing `ApplicationName` used when the caller does not override<br>it. Shown by clients in their server list and in the endpoint<br>description. |
| `default_port` | `u16` | TCP port used when the caller does not override it. 4840 is the<br>IANA-registered `opcua-tcp` port and is where OPC-UA tooling looks<br>first; a second simulator on the same machine needs a different one. |
| `node_manager_name` | `&'static str` | Name given to this simulator's node manager inside `async-opcua`.<br>Diagnostic only — it appears in `async-opcua`'s own logging. |
| `outputs_folder_name` | `&'static str` | Browse name of the folder holding the read-only outputs, e.g.<br>`"Outputs"`. |
| `outputs_folder_node_id` | `&'static str` | String node identifier of the outputs folder, e.g. `"CIET.Outputs"`.<br>Must not collide with any variable's node identifier. |
| `controls_folder_name` | `&'static str` | Browse name of the folder holding the writable controls, e.g.<br>`"Controls"`. |
| `controls_folder_node_id` | `&'static str` | String node identifier of the controls folder, e.g.<br>`"CIET.Controls"`. |
| `pki_dir_name` | `&'static str` | Directory name of this simulator's PKI store, relative to the OUTRAM<br>PARK per-user root — see [`super::pki`]. Holds a throwaway self-signed<br>keypair and nothing sensitive. |
| `mdns_instance_prefix` | `&'static str` | Prefix of the DNS-SD instance name announced over mDNS, e.g.<br>`"CIET-Educational-Simulator-v2"`. Used as the fallback when a<br>caller-supplied instance name sanitises to nothing. |
| `mdns_product_marker` | `&'static str` | Value of the mDNS `product` TXT record that identifies an announcement<br>as *this* simulator, e.g. `"ciet-educational-simulator-v2"`.<br><br>`_opcua-tcp._tcp` is the generic OPC-UA service type, so every other<br>OPC-UA server on the link appears under it too; this marker is what a<br>browser filters on. |
| `log_prefix` | `&'static str` | Short prefix for this simulator's console lines, e.g. `"CIET v2"`. The<br>shared layer prints `"<prefix> OPC-UA: ..."` and `"<prefix> mDNS: ..."`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OpcuaSimulatorProfile { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &OpcuaSimulatorProfile) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `OpcuaFolder`

Which of the two address-space folders a variable is filed under.

Enum rather than a boolean so a third folder becomes a compile error at
every match site rather than an inverted flag.

```rust
pub enum OpcuaFolder {
    Outputs,
    Controls,
}
```

##### Variants

###### `Outputs`

Read-only values the simulator publishes.

###### `Controls`

Values a client may write.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OpcuaFolder { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &OpcuaFolder) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Traits

#### Trait `OpcuaSimulator`

A simulator that can be served over OPC-UA: the identity half of the seam.

Implement it on a zero-sized marker type and give it one
[`OpcuaSimulatorProfile`]. It carries no data and no behaviour, so it costs
nothing at runtime; it exists so the shared layer's types
([`super::server::OpcuaServerConfig`], [`super::discovery::MdnsBrowser`])
can be bound to one simulator at compile time.

```ignore
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MySimulator;

impl OpcuaSimulator for MySimulator {
    const PROFILE: OpcuaSimulatorProfile = OpcuaSimulatorProfile { /* ... */ };
}
```

```rust
pub trait OpcuaSimulator: Copy + Send + Sync + ''static {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Associated Constants

- `PROFILE`: This simulator's naming and identity strings.

##### Implementations

This trait is implemented for the following types:

- `CietOpcuaSimulator`

#### Trait `OpcuaVariable`

One OPC-UA variable a simulator publishes: the variable half of the seam.

Implement it on a **`Copy` enum** whose variants enumerate every variable —
typically one that wraps the simulator's own signal / control / switch
enums, as `ciet_opcua`'s `CietNode` does. The shared layer calls these
methods to build the address space, to serve reads, and to route writes; it
never needs to know what any of them mean.

## Associated types

- [`Snapshot`](Self::Snapshot) is the simulator's flat plant state, shared
  as `Arc<RwLock<Snapshot>>`. Reads clone it under a read lock.
- [`Requests`](Self::Requests) is the simulator's pending-write mailbox,
  shared as `Arc<RwLock<Requests>>`. Writes are **recorded** there, never
  applied to the snapshot, so the simulator's own thread stays the only
  writer of plant state.

## Units

Values cross this seam as bare OPC-UA `Variant`s, so the unit is *not*
carried in the type. State the engineering unit in
[`description`](Self::description) — that string is what a client displays
next to the number, and it is the only place a remote operator can learn
whether they are writing kW or W.

```rust
pub trait OpcuaVariable: Copy + Send + Sync + ''static {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Associated Types

- `Simulator`: The simulator this variable belongs to, supplying the naming profile.
- `Snapshot`: The plant-state snapshot this variable is read out of.
- `Requests`: The pending-request mailbox a client's write is recorded in.

###### Required Methods

- `all`: Every variable, in the order the address space presents them.
- `node_identifier`: The string part of this variable's `NodeId`, e.g.
- `browse_name`: Short OPC-UA browse name — a **single path segment**, so it must be
- `display_name`: Human-facing label shown by clients and by the simulator's own UI.
- `description`: Description a client displays. **Name the engineering unit here**, and
- `data_type`: OPC-UA data type of the served value. Must agree with what
- `access_level`: Access level. The shared layer registers a write callback for exactly
- `folder`: Which folder this variable is filed under.
- `read`: Read this variable's current value out of a plant-state snapshot.
- `record_write`: Record a client's write as a *pending request*.

##### Implementations

This trait is implemented for the following types:

- `CietNode`

### Functions

#### Function `variant_as_f64`

Interpret an OPC-UA `Variant` as an `f64`, or `None` if it is not a number.

A helper for implementing [`OpcuaVariable::record_write`] on a continuous
control. `Double` is normally such a control's declared type, but real
clients also send `Float` and the integer types (a spin box bound to an
`i32`, a script writing `5` rather than `5.0`), so those are accepted and
widened — exactly, at any magnitude. Strings, booleans and structures are
refused, because silently reinterpreting them would hide a client bug.

The value is dimensionless here: its unit is whatever the variable's
[`description`](OpcuaVariable::description) says it is.

```rust
pub fn variant_as_f64(variant: &opcua::types::Variant) -> Option<f64> { /* ... */ }
```

### Re-exports

#### Re-export `advertise_simulator`

```rust
pub use discovery::advertise_simulator;
```

#### Re-export `DiscoveredSimulator`

```rust
pub use discovery::DiscoveredSimulator;
```

#### Re-export `DiscoveryError`

```rust
pub use discovery::DiscoveryError;
```

#### Re-export `MdnsAdvertisement`

```rust
pub use discovery::MdnsAdvertisement;
```

#### Re-export `MdnsBrowser`

```rust
pub use discovery::MdnsBrowser;
```

#### Re-export `OPCUA_MDNS_SERVICE_TYPE`

```rust
pub use discovery::OPCUA_MDNS_SERVICE_TYPE;
```

#### Re-export `spawn_opcua_server_thread`

```rust
pub use server::spawn_opcua_server_thread;
```

#### Re-export `OpcuaEndpointInfo`

```rust
pub use server::OpcuaEndpointInfo;
```

#### Re-export `OpcuaServerConfig`

```rust
pub use server::OpcuaServerConfig;
```

#### Re-export `OpcuaServerError`

```rust
pub use server::OpcuaServerError;
```

#### Re-export `OpcuaServerHandle`

```rust
pub use server::OpcuaServerHandle;
```

#### Re-export `SUBSCRIPTION_PUSH_INTERVAL`

```rust
pub use server::SUBSCRIPTION_PUSH_INTERVAL;
```

#### Re-export `variant_as_f64`

```rust
pub use simulator::variant_as_f64;
```

#### Re-export `OpcuaFolder`

```rust
pub use simulator::OpcuaFolder;
```

#### Re-export `OpcuaSimulator`

```rust
pub use simulator::OpcuaSimulator;
```

#### Re-export `OpcuaSimulatorProfile`

```rust
pub use simulator::OpcuaSimulatorProfile;
```

#### Re-export `OpcuaVariable`

```rust
pub use simulator::OpcuaVariable;
```

## Module `htr10`

# HTR-10 cited design data and packed-bed correlations

Foundation layer for the HTR-10 pebble-bed simulator rewrite (bead
`op-jyyp`): the published design constants of the HTR-10 test reactor and
the packed-bed closure correlations its core model needs, each carrying its
literature citation, together with unit tests that reproduce published
numbers. This module is the start of the simulator's V&V record.

## What belongs here / what does not

- **Belongs here:** *cited* HTR-10 design constants ([`design`]), the KTA
  packed-bed pressure-drop correlation ([`kta`]), the tabulated
  Zehner-Bauer-Schlunder effective bed conductivity ([`zbs`]), the IAEA
  core-physics benchmark specification and published reference eigenvalues
  ([`neutronics`]), and the unit tests that check them against the
  published values they came from.
- **Does NOT belong here:** the simulator itself (solver loops, transient
  models, GUI) — that is the `htgr_sim_v1` rewrite tracked under bead
  `op-jyyp` — and any number that cannot cite a published source. Do not
  add uncited "reasonable" constants to this module.

This module is a deliberate, maintainer-directed exception (bead `op-jyyp`,
2026-08-11) to this crate's "no new physics in the library" rule: cited
constants and their reference correlations live here so that the example
rewrite and its V&V tests share one provenance-checked source.

## Sources

| Source | Access tier | On-disk catalogue |
|---|---|---|
| IAEA-TECDOC-1382, *Evaluation of high temperature gas cooled reactor performance: Benchmark analysis related to initial testing of the HTTR and HTR-10*, IAEA Vienna, November 2003 — Chapter 4 is the HTR-10 core physics benchmark | Open | `crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.json` (Chapter 4; `part1` is the HTTR half and front matter). |
| Gao & Shi (2002), Nucl. Eng. Des. 218, 51-64, doi 10.1016/S0029-5493(02)00198-X | Proprietary (cited, not re-hosted) | `crates/kovan-literature/proprietary/papers/gao2002htr10th.json` (kovan-ddb61cb136fb98a9) |
| Virtual Test Bed generic pebble-bed tutorial, step 2 (KTA worked example) | Open | `reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md` |
| Virtual Test Bed generic PBR input (ZBS conductivity tabulation) | Open | `reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i` |

## Status: NOT VALIDATED

Nothing in this module validates an HTR-10 *simulator*. The tests here
establish only that the constants are transcribed correctly, are
self-consistent, and that the correlations reproduce published worked
examples. Per `RESPONSIBLE_USE.md` this is untrusted draft material until
human-reviewed; the full V&V sequence (PBMR-400 coupled benchmark, HTR-10
criticality, safety demonstration tests) is bead `op-jyyp.11`.

## Android / portability

This module is GUI-free and, like [`crate::animation`], builds on
Android/Termux. Do not add `egui`/`eframe` imports here.

```rust
pub mod htr10 { /* ... */ }
```

### Modules

## Module `design`

# HTR-10 published design constants

The HTR-10 operating point, core/pebble geometry, fuel specification, and
fuel-temperature limits, transcribed from published literature. Every value
carries its citation and access tier. Nothing here is measured, fitted, or
invented by this project.

- **Belongs here:** cited design constants and trivial derived quantities
  (bed porosity from filling fraction) whose derivation is stated.
- **Does NOT belong here:** correlations ([`super::kta`], [`super::zbs`]),
  solver state, or any uncited number.

Sources (see [`super`] for the on-disk catalogue paths):

- **IAEA HTGR benchmark document, Chapter 4** — Open tier. Design and
  operating data plus the benchmark-problem definitions.
- **Gao & Shi (2002), "Thermal hydraulic calculation of the HTR-10 for the
  initial and equilibrium core", Nucl. Eng. Des. 218, 51-64,
  doi 10.1016/S0029-5493(02)00198-X** — Proprietary tier (Elsevier). Cited
  with provenance; numbers used, text not reproduced.

```rust
pub mod design { /* ... */ }
```

### Types

#### Struct `Htr10DesignPoint`

The HTR-10 design point as specified in the IAEA HTGR benchmark document,
Chapter 4 (Open tier; catalogued at
`crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.json`; the
document is IAEA-TECDOC-1382, IAEA Vienna, November 2003).

All fields are `uom` quantities; the doc comment of each field spells out
the published value and unit. Construct with
[`Htr10DesignPoint::iaea_benchmark`]. The struct is plain data — it holds
no solver state and performs no physics.

```rust
pub struct Htr10DesignPoint {
    pub thermal_power: uom::si::f64::Power,
    pub primary_pressure: uom::si::f64::Pressure,
    pub core_diameter: uom::si::f64::Length,
    pub average_core_height: uom::si::f64::Length,
    pub core_volume: uom::si::f64::Volume,
    pub helium_outlet_phase1: uom::si::f64::ThermodynamicTemperature,
    pub helium_inlet_phase1: uom::si::f64::ThermodynamicTemperature,
    pub helium_outlet_phase2: uom::si::f64::ThermodynamicTemperature,
    pub helium_inlet_phase2: uom::si::f64::ThermodynamicTemperature,
    pub helium_mass_flow: uom::si::f64::MassRate,
    pub main_steam_pressure: uom::si::f64::Pressure,
    pub main_steam_temperature: uom::si::f64::ThermodynamicTemperature,
    pub feedwater_temperature: uom::si::f64::ThermodynamicTemperature,
    pub main_steam_mass_flow: uom::si::f64::MassRate,
    pub fuel_element_count: u32,
    pub pebble_diameter: uom::si::f64::Length,
    pub fuelled_zone_diameter: uom::si::f64::Length,
    pub graphite_density: uom::si::f64::MassDensity,
    pub heavy_metal_per_ball: uom::si::f64::Mass,
    pub enrichment: uom::si::f64::Ratio,
    pub fuel_kernel_radius: uom::si::f64::Length,
    pub uo2_density: uom::si::f64::MassDensity,
    pub filling_fraction: uom::si::f64::Ratio,
    pub side_reflector_thickness: uom::si::f64::Length,
    pub rpv_inner_diameter: uom::si::f64::Length,
    pub rpv_height: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `thermal_power` | `uom::si::f64::Power` | Reactor thermal power: 10 MW. |
| `primary_pressure` | `uom::si::f64::Pressure` | Primary helium pressure: 3.0 MPa. |
| `core_diameter` | `uom::si::f64::Length` | Reactor core diameter: 180 cm (1.8 m). |
| `average_core_height` | `uom::si::f64::Length` | Average core height: 197 cm (1.97 m). |
| `core_volume` | `uom::si::f64::Volume` | Core volume as stated in the source: 5.0 m^3. See the geometry<br>self-consistency test for how well this closes against diameter and<br>height. |
| `helium_outlet_phase1` | `uom::si::f64::ThermodynamicTemperature` | Average helium temperature at reactor outlet, phase 1 operation: 700 degrees Celsius. |
| `helium_inlet_phase1` | `uom::si::f64::ThermodynamicTemperature` | Average helium temperature at reactor inlet, phase 1 operation: 250 degrees Celsius. |
| `helium_outlet_phase2` | `uom::si::f64::ThermodynamicTemperature` | Helium outlet temperature for the planned phase 2 (gas-turbine) operation: 900 degrees Celsius. |
| `helium_inlet_phase2` | `uom::si::f64::ThermodynamicTemperature` | Helium inlet temperature for the planned phase 2 operation: 300 degrees Celsius. |
| `helium_mass_flow` | `uom::si::f64::MassRate` | Helium mass flow rate at full power: 4.3 kg/s. (Gao & Shi 2002, Table 2,<br>carries 4.32 kg/s for the equilibrium core at 100% load.) |
| `main_steam_pressure` | `uom::si::f64::Pressure` | Main steam pressure at the steam-generator outlet: 4.0 MPa. |
| `main_steam_temperature` | `uom::si::f64::ThermodynamicTemperature` | Main steam temperature at the steam-generator outlet: 440 degrees Celsius. |
| `feedwater_temperature` | `uom::si::f64::ThermodynamicTemperature` | Feedwater temperature: 104 degrees Celsius. |
| `main_steam_mass_flow` | `uom::si::f64::MassRate` | Main steam flow rate: 12.5 t/hr = 12500/3600 kg/s (about 3.472 kg/s). |
| `fuel_element_count` | `u32` | Number of fuel elements in the equilibrium core: 27,000. |
| `pebble_diameter` | `uom::si::f64::Length` | Fuel-element (pebble) outer diameter: 6.0 cm. |
| `fuelled_zone_diameter` | `uom::si::f64::Length` | Diameter of the fuelled zone inside a pebble: 5.0 cm. |
| `graphite_density` | `uom::si::f64::MassDensity` | Graphite density in the fuelled zone and outer shell: 1.73 g/cm^3. |
| `heavy_metal_per_ball` | `uom::si::f64::Mass` | Heavy-metal (uranium) loading per fuel pebble: 5.0 g. |
| `enrichment` | `uom::si::f64::Ratio` | Enrichment of U-235 in fresh fuel, by weight: 17%. |
| `fuel_kernel_radius` | `uom::si::f64::Length` | Radius of the UO2 fuel kernel inside a coated particle: 0.025 cm<br>(0.25 mm). |
| `uo2_density` | `uom::si::f64::MassDensity` | UO2 kernel density: 10.4 g/cm^3. |
| `filling_fraction` | `uom::si::f64::Ratio` | Volumetric filling fraction of balls in the core, f = 0.61<br>(dimensionless). Bed porosity follows as eps = 1 - f = 0.39; use<br>[`Htr10DesignPoint::bed_porosity`]. |
| `side_reflector_thickness` | `uom::si::f64::Length` | Side reflector thickness: 100 cm (1.0 m). |
| `rpv_inner_diameter` | `uom::si::f64::Length` | Reactor pressure vessel inner diameter: 4.2 m. |
| `rpv_height` | `uom::si::f64::Length` | Reactor pressure vessel height: 11.1 m. |

##### Implementations

###### Methods

- ```rust
  pub fn iaea_benchmark() -> Self { /* ... */ }
  ```
  The HTR-10 design point, transcribed from the IAEA HTGR benchmark

- ```rust
  pub fn bed_porosity(self: &Self) -> Ratio { /* ... */ }
  ```
  Bed porosity (helium void fraction of the pebble bed), dimensionless:

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10DesignPoint { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Htr10DesignPoint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `Htr10FuelTemperatureLimits`

HTR-10 fuel-temperature limits and core-flow-distribution figures from
Gao & Shi (2002), Nucl. Eng. Des. 218, 51-64,
doi 10.1016/S0029-5493(02)00198-X (Proprietary tier; catalogued at
`crates/kovan-literature/proprietary/papers/gao2002htr10th.json`).

**Do not conflate the two temperature figures in this struct with the
1600 degrees Celsius coated-particle figure** — see
[`generic_coated_particle_retention_limit`]. Mixing them up misstates the
HTR-10 safety margin by 370 K.

```rust
pub struct Htr10FuelTemperatureLimits {
    pub max_fuel_temperature_at_120_percent_overload: uom::si::f64::ThermodynamicTemperature,
    pub fuel_temperature_limit: uom::si::f64::ThermodynamicTemperature,
    pub max_bypass_flow_fraction: uom::si::f64::Ratio,
    pub min_core_flow_fraction: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_fuel_temperature_at_120_percent_overload` | `uom::si::f64::ThermodynamicTemperature` | Calculated maximum fuel temperature at 120% overload, equilibrium<br>core: 1046.6 degrees Celsius (Gao & Shi 2002, Table 2; often quoted<br>rounded as 1046 degrees Celsius). This is a *calculated best-estimate<br>peak*, not a limit. |
| `fuel_temperature_limit` | `uom::si::f64::ThermodynamicTemperature` | The HTR-10's *own specified* maximum fuel temperature limit under<br>normal and accident conditions: 1230 degrees Celsius (Gao & Shi 2002,<br>set from the experimental demonstration that the coating retains<br>fission products up to 1250 degrees Celsius). This is **not** the<br>generic 1600 degrees Celsius figure of the modular-HTR literature. |
| `max_bypass_flow_fraction` | `uom::si::f64::Ratio` | Maximum bypass flow through the gaps between graphite components:<br>less than 10% of rated flow (Gao & Shi 2002). Stored as the bounding<br>fraction 0.10, dimensionless. A bed model that ignores bypass is<br>therefore making up to a 10% error in core flow. |
| `min_core_flow_fraction` | `uom::si::f64::Ratio` | Conservative minimum fraction of rated flow through the pebble-bed<br>core: 86% (Gao & Shi 2002: less than 10% bypass, about 2.5% through<br>control-rod tubes, at least 1% through the fuel discharge tube).<br>Stored as 0.86, dimensionless. |

##### Implementations

###### Methods

- ```rust
  pub fn gao_shi_2002() -> Self { /* ... */ }
  ```
  The values published in Gao & Shi (2002) — see each field's doc

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10FuelTemperatureLimits { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Htr10FuelTemperatureLimits) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `generic_coated_particle_retention_limit`

The GENERIC coated-particle fission-product retention limit that pervades
the modular-HTR literature: 1600 degrees Celsius. **This is NOT an HTR-10
limit.** The HTR-10's own specified maximum fuel temperature limit is
1230 degrees Celsius
([`Htr10FuelTemperatureLimits::fuel_temperature_limit`], Gao & Shi 2002).
This function exists precisely so the two numbers cannot be silently
conflated: any HTR-10 margin calculation must use 1230 degrees Celsius,
not this figure.

```rust
pub fn generic_coated_particle_retention_limit() -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

## Module `kta`

# KTA packed-bed pressure-drop correlation

The German Kerntechnischer Ausschuss (KTA) pressure-drop correlation for
flow through a pebble bed, as stated (with a fully worked example) in the
Virtual Test Bed generic pebble-bed tutorial, step 2 (Open tier;
`reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md`).
The correlation set is:

- `-dp/dx = psi * ((1-eps)/eps^3) * (1/(2*D_h*rho)) * (mdot/A)^2`
- `psi = 320/(Re/(1-eps)) + 6/(Re/(1-eps))^0.1`
- `Re = (mdot/A) * D_h / mu`

where `eps` is the bed porosity, `D_h` the hydraulic diameter (for pebble
beds, the pebble diameter), `rho` the fluid density, `mu` the dynamic
viscosity, and `mdot/A` the superficial mass flux through the bed
cross-section `A`.

- **Belongs here:** the KTA correlation functions and the type aliases
  ([`MassFlux`], [`PressureGradient`]) their signatures need, plus the
  tests reproducing the VTB worked example.
- **Does NOT belong here:** design constants ([`super::design`]), bed
  conductivity ([`super::zbs`]), or any solver loop.

**Validity.** The KTA correlation is stated for packed beds of spheres at
porosities near the random-packing range (the HTR-10 bed has eps = 0.39)
and modified Reynolds numbers `Re/(1-eps)` from 1 to about 1e5; the VTB
worked example sits at `Re/(1-eps)` = 6.6e4. Outside that range the
friction factor is an extrapolation.

**Status:** the tests below check the *correlation implementation* against
a published worked example. That is code verification, not validation of
any HTR-10 simulator.

```rust
pub mod kta { /* ... */ }
```

### Types

#### Type Alias `MassFlux`

Superficial mass flux through the bed cross-section, `mdot/A`, in
kilograms per square metre per second (kg m^-2 s^-1). This is the flux
over the *whole* bed cross-section, not the interstitial (pore) flux.

```rust
pub type MassFlux = uom::si::Quantity<uom::si::ISQ<uom::typenum::N2, uom::typenum::P1, uom::typenum::N1, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

#### Type Alias `PressureGradient`

Magnitude of a pressure gradient, in pascals per metre (Pa/m =
kg m^-2 s^-2). [`kta_pressure_gradient`] returns the magnitude of the
streamwise pressure *drop* per unit length, i.e. `-dp/dx > 0` for flow in
the `+x` direction.

```rust
pub type PressureGradient = uom::si::Quantity<uom::si::ISQ<uom::typenum::N2, uom::typenum::P1, uom::typenum::N2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

### Functions

#### Function `superficial_mass_flux`

Superficial mass flux `G = mdot / A` (kg m^-2 s^-1) from the bed mass flow
rate `mdot` (kg/s) and the bed cross-sectional area `A` (m^2). `A` is the
full (empty-tube) cross-section of the cylindrical flow channel, not the
pore area.

```rust
pub fn superficial_mass_flux(mass_flow: uom::si::f64::MassRate, flow_area: uom::si::f64::Area) -> MassFlux { /* ... */ }
```

#### Function `packed_bed_reynolds`

Packed-bed Reynolds number `Re = G * D_h / mu` (dimensionless), with `G`
the superficial mass flux (kg m^-2 s^-1), `D_h` the hydraulic diameter
(m; the pebble diameter for pebble beds), and `mu` the fluid dynamic
viscosity (Pa s). This is the plain superficial-velocity Reynolds number;
the KTA friction factor internally uses the modified form `Re/(1-eps)`.

```rust
pub fn packed_bed_reynolds(mass_flux: MassFlux, hydraulic_diameter: uom::si::f64::Length, dynamic_viscosity: uom::si::f64::DynamicViscosity) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `kta_friction_factor`

KTA friction factor `psi = 320/(Re/(1-eps)) + 6/(Re/(1-eps))^0.1`
(dimensionless), with `Re` the superficial-velocity packed-bed Reynolds
number and `eps` the bed porosity (dimensionless, 0 < eps < 1; HTR-10
bed: 0.39). Source: VTB generic pebble-bed tutorial, step 2 (Open tier).

```rust
pub fn kta_friction_factor(reynolds: uom::si::f64::Ratio, porosity: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `kta_pressure_gradient`

KTA pressure-drop magnitude per unit bed length (Pa/m):
`-dp/dx = psi * ((1-eps)/eps^3) * (1/(2*D_h*rho)) * G^2`, with `G` the
superficial mass flux (kg m^-2 s^-1), `D_h` the hydraulic diameter (m),
`eps` the bed porosity (dimensionless), `rho` the fluid density (kg/m^3),
and `mu` the dynamic viscosity (Pa s) entering through the Reynolds
number. Returns the *positive* drop per length; the pressure falls in the
flow direction. Source: VTB generic pebble-bed tutorial, step 2 (Open
tier).

```rust
pub fn kta_pressure_gradient(mass_flux: MassFlux, hydraulic_diameter: uom::si::f64::Length, porosity: uom::si::f64::Ratio, density: uom::si::f64::MassDensity, dynamic_viscosity: uom::si::f64::DynamicViscosity) -> PressureGradient { /* ... */ }
```

#### Function `pressure_drop_over_bed`

Reference pressure drop over a bed of length `L`: `dp = (-dp/dx) * L`
(Pa), with the gradient magnitude from [`kta_pressure_gradient`]. Valid
only when the gradient is uniform over the bed (constant properties and
flux), as in the VTB worked example.

```rust
pub fn pressure_drop_over_bed(gradient: PressureGradient, bed_length: uom::si::f64::Length) -> uom::si::f64::Pressure { /* ... */ }
```

## Module `neutronics`

# HTR-10 core-physics benchmark specification (B1-B4) as data

The IAEA HTGR coordinated-research-programme benchmark problems for the
HTR-10 — initial criticality (B1), isothermal temperature coefficient (B2),
control-rod worth for the full core (B3) and for the initial core (B4) —
transcribed as typed, cited data, together with the *measured* first
criticality of December 2000 and the published values other codes obtained.

Nothing in this module computes neutron transport. It is the problem
statement and the reference answers, in a form a future transport
calculation can be judged against. **No k_eff in this module was computed
by this project.**

## The trap this module exists to prevent

**"B1 as defined" and "B1 as measured" are different problems.** After the
benchmark was specified and before the core was loaded, two conditions
changed (IAEA benchmark document, section 4.2.1.3):

1. The dummy (graphite) balls actually manufactured had density
   1.84 g/cm^3, not the specified 1.73 g/cm^3, and boron-equivalent
   impurity 0.125 ppm, not the specified 1.3 ppm.
2. First criticality was reached under atmospheric **air**, not helium, and
   at **15 degrees Celsius**, not the 20 degrees Celsius of the definition.

The literature therefore speaks of the **original** benchmark (as defined)
and the **deviated** benchmark (as built and measured). They differ by
roughly 1000 pcm. Every quantity here is tagged with
[`BenchmarkVariant`] so the two cannot be silently compared. See
[`BenchmarkVariant`] for the full deviation list.

## What belongs here / what does not

- **Belongs here:** the benchmark problem definitions, the fuel/dummy
  pebble and TRISO specifications the benchmark prescribes, the core
  geometry the sources state *in text*, published k_eff / critical-height /
  rod-worth values from named codes, and the measured first criticality —
  each carrying its source and that source's [`AccessTier`].
- **Does NOT belong here:** any transport solver, any homogenised
  cross-section set, any number this project computed, and any number
  whose source cannot be named. Do not add "reasonable" values.

## Sources and access tiers

| Source | Tier | On-disk |
|---|---|---|
| **IAEA-TECDOC-1382**, *Evaluation of high temperature gas cooled reactor performance: Benchmark analysis related to initial testing of the HTTR and HTR-10*, IAEA Vienna, November 2003. Chapter 4 is the HTR-10 core physics benchmark | Open | `crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.json` (markdown at `generated/markdown/open/iaea-tecdoc-1382-part2.md`) |
| Choo, A. J. Y. and Xiao, S. (2024), *Criticality Analysis of HTR-10 Using the High-Temperature Gas-Cooled Reactor Code Package*, SNRSI/NUS | Open | `crates/kovan-literature/open/papers/choo-htr10-criticality.json` |
| Wang, M.-J., Sheu, R.-J., Peir, J.-J. and Liang, J.-H. (2014), *Criticality calculations of the HTR-10 pebble-bed reactor with SCALE6/CSAS6 and MCNP5*, Ann. Nucl. Energy 64, 1-7, doi 10.1016/j.anucene.2013.09.031 | Proprietary (cited, not re-hosted) | `crates/kovan-literature/proprietary/papers/wang2014htr10criticality.json` |
| Tantillo, F. et al. (2020), *HTR code package neutronics developments and benchmarks*, Nucl. Eng. Des. 362, 110603, doi 10.1016/j.nucengdes.2020.110603 | Proprietary (cited, not re-hosted) | `crates/kovan-literature/proprietary/papers/tantillo2020hcpneutronics.json` |

## Status: NOT VALIDATED, and NOT COMPUTABLE HERE YET

This workspace cannot currently compute any of these eigenvalues. Graphite
bound-atom S(alpha,beta) thermal scattering does not reach the pebble-bed
transport path (beads `op-6tz.35`, `op-hc2o`) and carbon is absent from the
nuclear-data crate's `well_known_mat` table (bead `op-h23`). A k_eff
computed with free-gas scattering on a graphite-moderated thermal system is
not a meaningful criticality result and must not be presented as one. See
`docs/reactor-scoping/htr10-neutronics.md`.

## Android / portability

Plain data and arithmetic — no GUI, no BLAS. Builds on Android/Termux like
the rest of [`super`].

```rust
pub mod neutronics { /* ... */ }
```

### Types

#### Enum `AccessTier`

How openly available a cited source is, in the sense of `DATA_POLICY.md`.

Open-tier material may be quoted and re-hosted in this repository;
proprietary-tier material may be **cited and implemented from**, but its
text and PDF must not be reproduced here.

```rust
pub enum AccessTier {
    Open,
    Proprietary,
}
```

##### Variants

###### `Open`

Openly published; the document itself is committed under
`crates/kovan-literature/open/`.

###### `Proprietary`

Publisher-restricted; catalogued under
`crates/kovan-literature/proprietary/` which is gitignored. Cite and
implement from it; never re-host it.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AccessTier { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AccessTier) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `LiteratureSource`

A published source of HTR-10 neutronics numbers used in this module.

Every reference value carries one of these, so a reader can trace any
number to a document and know whether that document may be redistributed.

```rust
pub enum LiteratureSource {
    IaeaHtgrBenchmark,
    ChooXiao2024,
    Wang2014,
    Tantillo2020,
}
```

##### Variants

###### `IaeaHtgrBenchmark`

IAEA HTGR coordinated-research-programme benchmark document, Chapter 4
(HTR-10 core physics). The primary specification and the source of the
measured first criticality.

###### `ChooXiao2024`

Choo, A. J. Y. and Xiao, S. (2024), SNRSI / National University of
Singapore. Simplified Serpent 2 and HTR Code Package models of HTR-10
initial criticality, ENDF/B-VII.0.

###### `Wang2014`

Wang, M.-J. et al. (2014), Ann. Nucl. Energy 64, 1-7. SCALE6/CSAS6 and
MCNP5 criticality calculations, ENDF/B-VII.0.

###### `Tantillo2020`

Tantillo, F. et al. (2020), Nucl. Eng. Des. 362, 110603. HTR Code
Package / TRISHA versus Serpent, ENDF/B-VII.0.

##### Implementations

###### Methods

- ```rust
  pub fn access_tier(self: &Self) -> AccessTier { /* ... */ }
  ```
  The access tier of this source, per `DATA_POLICY.md`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LiteratureSource { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LiteratureSource) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `NeutronicsCode`

Which neutronics code produced a published value.

Kept as an enum rather than a string so that a reader can enumerate the
codes the benchmark has been run with, and so a comparison cannot name a
code that is not in this list.

```rust
pub enum NeutronicsCode {
    Vsop,
    Mcnp4a,
    Mcnp5,
    Serpent2,
    HtrCodePackage,
    Scale6ContinuousEnergy,
    Scale6Multigroup(UnitCellTreatment),
}
```

##### Variants

###### `Vsop`

VSOP — the diffusion-based pebble-bed system code (GAM/THERMOS spectrum,
CITATION four-group R-Z diffusion) used by INET for the benchmark.

###### `Mcnp4a`

MCNP-4A continuous-energy Monte Carlo, ENDF/B-V, as used by INET.

###### `Mcnp5`

MCNP5 continuous-energy Monte Carlo, ENDF/B-VII.0 (Wang et al. 2014).

###### `Serpent2`

Serpent 2 continuous-energy Monte Carlo, ENDF/B-VII.0.

###### `HtrCodePackage`

HTR Code Package (TRISHA spectrum + MGT-N diffusion), ENDF/B-VII.0.

###### `Scale6ContinuousEnergy`

SCALE6/CSAS6 with continuous-energy cross sections.

###### `Scale6Multigroup`

SCALE6/CSAS6 multigroup with a named unit-cell self-shielding treatment.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `UnitCellTreatment` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NeutronicsCode { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NeutronicsCode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `UnitCellTreatment`

SCALE6 resonance self-shielding unit-cell treatments, as compared by
Wang et al. (2014) for the doubly heterogeneous HTR-10 fuel.

The variant chosen changes k_eff by thousands of pcm — see
[`wang_2014_unit_cell_bias`]. This enum exists so that a homogenisation
choice is always stated alongside the eigenvalue it produced.

```rust
pub enum UnitCellTreatment {
    InfHomMedium,
    LatticeCell,
    MultiRegion,
    LatticeCellCellMix,
    MultiRegionCellMix,
    DoubleHet,
}
```

##### Variants

###### `InfHomMedium`

Treats the cell as an infinite homogeneous medium: no spatial
self-shielding at all. Wrong for a pebble bed by about +2800 pcm.

###### `LatticeCell`

One-dimensional repeating lattice cell; the four TRISO coatings must be
homogenised to fit its fuel/gap/clad description.

###### `MultiRegion`

Flexible one-dimensional multi-region cell; preserves the TRISO layer
structure but approximates the lattice by a white boundary condition.

###### `LatticeCellCellMix`

[`Self::LatticeCell`] with `CELLMIX`, i.e. a cell-weighted homogenised
mixture used in the pebble fuel zone.

###### `MultiRegionCellMix`

[`Self::MultiRegion`] with `CELLMIX`.

###### `DoubleHet`

The doubly heterogeneous treatment: multi-region for the grains inside
the matrix, lattice-cell for the pebbles in the core. The intended
treatment for pebble-bed fuel.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> UnitCellTreatment { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &UnitCellTreatment) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `BenchmarkVariant`

Which of the two HTR-10 benchmark definitions a quantity belongs to.

**Never compare a value tagged [`Self::Original`] with one tagged
[`Self::Deviated`] without saying so** — the two differ by roughly 1000 pcm
in k_eff for the initial core.

```rust
pub enum BenchmarkVariant {
    Original,
    Deviated,
}
```

##### Variants

###### `Original`

The benchmark **as defined**, before the core was built: dummy balls of
density 1.73 g/cm^3 with 1.3 ppm boron equivalent, helium atmosphere,
core temperature 20 degrees Celsius (many later papers evaluate this
case at 27 degrees Celsius instead — see
[`BenchmarkVariant::definition_temperature`]).

###### `Deviated`

The benchmark **as built and measured** (IAEA benchmark document
section 4.2.1.3, "deviated benchmark"): dummy balls of density
1.84 g/cm^3 with 0.125 ppm boron equivalent, humid air atmosphere at
0.1013 MPa, core temperature 15 degrees Celsius in the experiment.

##### Implementations

###### Methods

- ```rust
  pub fn dummy_pebble(self: &Self) -> DummyPebbleSpec { /* ... */ }
  ```
  The dummy (graphite, unfuelled) pebble specification for this variant.

- ```rust
  pub fn atmosphere(self: &Self) -> CoreAtmosphere { /* ... */ }
  ```
  The core atmosphere this variant prescribes.

- ```rust
  pub fn definition_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  The core temperature the *definition* prescribes for the initial-core

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BenchmarkVariant { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BenchmarkVariant) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `CoreAtmosphere`

The gas filling the pebble interstices and the cavity above the bed.

Enum rather than a trait object: the benchmark admits exactly these two
atmospheres, and a future transport model must handle both exhaustively.

```rust
pub enum CoreAtmosphere {
    Helium,
    HumidAir(HumidAirComposition),
}
```

##### Variants

###### `Helium`

Helium, as in the original benchmark definition and in normal
operation. The benchmark does not state a helium density for the
criticality problems; at the stated 0.1 MPa-scale cavity conditions its
neutronic effect is negligible compared with air.

###### `HumidAir`

Atmospheric humid air at 0.1013 MPa, as during the actual first
criticality experiment.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `HumidAirComposition` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoreAtmosphere { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoreAtmosphere) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `HumidAirComposition`

The humid-air composition the IAEA document prescribes for the deviated
benchmark, filling the upper cavity and the inter-pebble spaces.

All values are as published; nothing is derived. Note that the stated
oxygen and nitrogen percentages sum to 98.67%, not 100% — the balance
(argon and trace gases) is not stated in the source, and this struct does
not invent it.

```rust
pub struct HumidAirComposition {
    pub pressure: uom::si::f64::Pressure,
    pub air_density: uom::si::f64::MassDensity,
    pub water_vapour_density: uom::si::f64::MassDensity,
    pub oxygen_fraction: uom::si::f64::Ratio,
    pub nitrogen_fraction: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pressure` | `uom::si::f64::Pressure` | Atmospheric pressure: 0.1013 MPa. |
| `air_density` | `uom::si::f64::MassDensity` | Air density: 1.149e-3 g/cm^3. |
| `water_vapour_density` | `uom::si::f64::MassDensity` | Water-vapour density: 2.57e-5 g/cm^3. |
| `oxygen_fraction` | `uom::si::f64::Ratio` | Oxygen fraction of the air, as published: 23.14%. |
| `nitrogen_fraction` | `uom::si::f64::Ratio` | Nitrogen fraction of the air, as published: 75.53%. |

##### Implementations

###### Methods

- ```rust
  pub fn iaea_deviated() -> Self { /* ... */ }
  ```
  The composition stated in the IAEA benchmark document for the deviated

- ```rust
  pub fn unaccounted_fraction(self: &Self) -> Ratio { /* ... */ }
  ```
  The fraction of the air not accounted for by the published oxygen and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HumidAirComposition { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HumidAirComposition) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `DummyPebbleSpec`

An unfuelled graphite "dummy" pebble.

Same 6 cm outer diameter as a fuel pebble; only density and boron-equivalent
impurity differ between the benchmark variants. Obtain one from
[`BenchmarkVariant::dummy_pebble`].

```rust
pub struct DummyPebbleSpec {
    pub diameter: uom::si::f64::Length,
    pub graphite_density: uom::si::f64::MassDensity,
    pub equivalent_boron_ppm: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `diameter` | `uom::si::f64::Length` | Outer diameter: 6.0 cm in both variants. |
| `graphite_density` | `uom::si::f64::MassDensity` | Graphite density: 1.73 g/cm^3 (original) or 1.84 g/cm^3 (deviated).<br><br>**Literature discrepancy.** The IAEA benchmark document states<br>1.84 g/cm^3 for the deviated case (twice, in section 4.2.1.3). Tantillo<br>et al. (2020) and the prose of Choo and Xiao (2024) both state<br>1.86 g/cm^3, while Choo and Xiao's own Table 1 states 1.84 g/cm^3.<br>This module follows the primary source (IAEA, 1.84). A model that used<br>1.86 would be ~1% denser in the moderator balls. |
| `equivalent_boron_ppm` | `f64` | Equivalent natural boron content of impurities in the graphite, in<br>parts per million by weight: 1.3 (original) or 0.125 (deviated).<br><br>Held as a plain `f64` ppm rather than a `uom` `Ratio` because it is a<br>*boron-equivalent* impurity figure — a neutronic equivalence, not a<br>measured mass fraction of any one element. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DummyPebbleSpec { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DummyPebbleSpec) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `TrisoParticle`

The TRISO coated fuel particle of the HTR-10 fuel pebble.

Layer stack from the kernel outward: UO2 kernel, porous carbon buffer,
inner pyrolytic carbon, silicon carbide, outer pyrolytic carbon. The IAEA
benchmark document writes the coating materials as "PyC/PyC/SiC/PyC"
(the first "PyC" being the low-density buffer).

All radii and thicknesses are `Length`; all densities are `MassDensity`.
Published thicknesses are in millimetres in the source and are converted to
centimetres here because the benchmark's other lengths are in centimetres.

```rust
pub struct TrisoParticle {
    pub kernel_radius: uom::si::f64::Length,
    pub kernel_density: uom::si::f64::MassDensity,
    pub buffer_thickness: uom::si::f64::Length,
    pub buffer_density: uom::si::f64::MassDensity,
    pub inner_pyc_thickness: uom::si::f64::Length,
    pub inner_pyc_density: uom::si::f64::MassDensity,
    pub sic_thickness: uom::si::f64::Length,
    pub sic_density: uom::si::f64::MassDensity,
    pub outer_pyc_thickness: uom::si::f64::Length,
    pub outer_pyc_density: uom::si::f64::MassDensity,
    pub particles_per_pebble: u32,
    pub enrichment: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kernel_radius` | `uom::si::f64::Length` | UO2 kernel radius: 0.25 mm = 0.025 cm. |
| `kernel_density` | `uom::si::f64::MassDensity` | UO2 kernel density: 10.4 g/cm^3. |
| `buffer_thickness` | `uom::si::f64::Length` | Porous carbon buffer thickness: 0.09 mm = 0.009 cm. |
| `buffer_density` | `uom::si::f64::MassDensity` | Buffer density: 1.1 g/cm^3. |
| `inner_pyc_thickness` | `uom::si::f64::Length` | Inner pyrolytic carbon thickness: 0.04 mm = 0.004 cm. |
| `inner_pyc_density` | `uom::si::f64::MassDensity` | Inner PyC density: 1.9 g/cm^3. |
| `sic_thickness` | `uom::si::f64::Length` | Silicon carbide thickness: 0.035 mm = 0.0035 cm. |
| `sic_density` | `uom::si::f64::MassDensity` | SiC density: 3.18 g/cm^3. |
| `outer_pyc_thickness` | `uom::si::f64::Length` | Outer pyrolytic carbon thickness: 0.04 mm = 0.004 cm. |
| `outer_pyc_density` | `uom::si::f64::MassDensity` | Outer PyC density: 1.9 g/cm^3. |
| `particles_per_pebble` | `u32` | Average number of coated particles dispersed in one fuel pebble: 8335. |
| `enrichment` | `uom::si::f64::Ratio` | U-235 enrichment of the fresh fuel, by weight: 17%. |

##### Implementations

###### Methods

- ```rust
  pub fn iaea_benchmark() -> Self { /* ... */ }
  ```
  The HTR-10 coated particle as specified in the IAEA benchmark document,

- ```rust
  pub fn outer_radius(self: &Self) -> Length { /* ... */ }
  ```
  Outer radius of the whole coated particle: kernel radius plus the four

- ```rust
  pub fn kernel_volume(self: &Self) -> Volume { /* ... */ }
  ```
  Volume of one UO2 kernel, (4/3) pi r^3.

- ```rust
  pub fn particle_volume(self: &Self) -> Volume { /* ... */ }
  ```
  Volume of one whole coated particle, (4/3) pi R^3 with R from

- ```rust
  pub fn uranium_molar_mass_g_per_mol(self: &Self) -> f64 { /* ... */ }
  ```
  Molar mass of the uranium in the fuel, in g/mol, for the stated

- ```rust
  pub fn uranium_mass_fraction_of_uo2(self: &Self) -> Ratio { /* ... */ }
  ```
  Uranium mass fraction of the UO2 kernel material, M_U / (M_U + 2 M_O).

- ```rust
  pub fn heavy_metal_per_pebble(self: &Self) -> Mass { /* ... */ }
  ```
  Heavy-metal (uranium) mass in one fuel pebble, derived from the kernel

- ```rust
  pub fn packing_fraction_in_fuel_zone(self: &Self, fuel_zone_radius: Length) -> Ratio { /* ... */ }
  ```
  Volumetric packing fraction of coated particles inside the fuelled zone

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `FuelPebbleSpec`

The fuel pebble: a 6 cm sphere with a 5 cm fuelled zone of graphite matrix
holding [`TrisoParticle`]s, inside an unfuelled 0.5 cm graphite shell.

```rust
pub struct FuelPebbleSpec {
    pub diameter: uom::si::f64::Length,
    pub fuelled_zone_diameter: uom::si::f64::Length,
    pub graphite_density: uom::si::f64::MassDensity,
    pub heavy_metal_loading: uom::si::f64::Mass,
    pub equivalent_boron_ppm_in_uranium: f64,
    pub equivalent_boron_ppm_in_graphite: f64,
    pub particle: TrisoParticle,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `diameter` | `uom::si::f64::Length` | Outer diameter of the pebble: 6.0 cm. |
| `fuelled_zone_diameter` | `uom::si::f64::Length` | Diameter of the fuelled zone: 5.0 cm (so the unfuelled outer shell is<br>0.5 cm thick). |
| `graphite_density` | `uom::si::f64::MassDensity` | Density of the graphite in the matrix and outer shell: 1.73 g/cm^3. |
| `heavy_metal_loading` | `uom::si::f64::Mass` | Heavy-metal (uranium) loading per pebble as *specified*: 5.0 g. |
| `equivalent_boron_ppm_in_uranium` | `f64` | Equivalent natural boron content of impurities in the uranium: 4 ppm. |
| `equivalent_boron_ppm_in_graphite` | `f64` | Equivalent natural boron content of impurities in the graphite:<br>1.3 ppm. Unlike the dummy balls, this value is the same in the original<br>and deviated benchmarks — only the *dummy* ball impurity changed. |
| `particle` | `TrisoParticle` | The coated particle dispersed in the fuelled zone. |

##### Implementations

###### Methods

- ```rust
  pub fn iaea_benchmark() -> Self { /* ... */ }
  ```
  The HTR-10 fuel pebble as specified in the IAEA benchmark document,

- ```rust
  pub fn fuelled_zone_radius(self: &Self) -> Length { /* ... */ }
  ```
  Radius of the fuelled zone (half of [`Self::fuelled_zone_diameter`]).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FuelPebbleSpec { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FuelPebbleSpec) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `Htr10CoreGeometry`

The core geometry the sources state **in text**, in the R-Z core-physics
model of the IAEA benchmark (its Figure 4.10).

**Deliberately incomplete.** The full zone map — conus angle, discharge-tube
radius, individual reflector block boundaries, and the axial coordinates of
the 83 material zones — exists in the source only as a *figure*, and is not
recoverable from the text. Those dimensions are therefore absent here
rather than guessed. See `docs/reactor-scoping/htr10-neutronics.md` for the
routes to obtaining them.

```rust
pub struct Htr10CoreGeometry {
    pub core_diameter: uom::si::f64::Length,
    pub mean_core_height: uom::si::f64::Length,
    pub full_core_volume: uom::si::f64::Volume,
    pub side_reflector_thickness: uom::si::f64::Length,
    pub reflector_graphite_density: uom::si::f64::MassDensity,
    pub reflector_equivalent_boron_ppm: f64,
    pub boronated_carbon_brick_density: uom::si::f64::MassDensity,
    pub boronated_brick_b4c_weight_fraction: uom::si::f64::Ratio,
    pub control_rod_channel_count: u32,
    pub control_rod_channel_diameter: uom::si::f64::Length,
    pub control_rod_channel_radius: uom::si::f64::Length,
    pub absorber_ball_channel_count: u32,
    pub irradiation_channel_count: u32,
    pub helium_channel_count: u32,
    pub helium_channel_diameter: uom::si::f64::Length,
    pub helium_channel_radius: uom::si::f64::Length,
    pub helium_channel_bottom: uom::si::f64::Length,
    pub helium_channel_top: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `core_diameter` | `uom::si::f64::Length` | Active pebble-bed diameter: 180 cm. |
| `mean_core_height` | `uom::si::f64::Length` | Mean height of the equilibrium core: 197 cm. |
| `full_core_volume` | `uom::si::f64::Volume` | Volume of the full core including the conus region: 5.0 m^3. |
| `side_reflector_thickness` | `uom::si::f64::Length` | Side reflector thickness, including a layer of carbon bricks: 100 cm. |
| `reflector_graphite_density` | `uom::si::f64::MassDensity` | Reflector graphite density: 1.76 g/cm^3. |
| `reflector_equivalent_boron_ppm` | `f64` | Equivalent natural boron impurity in the reflector graphite:<br>4.8366 ppm. |
| `boronated_carbon_brick_density` | `uom::si::f64::MassDensity` | Density of the boronated carbon brick including its B4C: 1.59 g/cm^3. |
| `boronated_brick_b4c_weight_fraction` | `uom::si::f64::Ratio` | Weight fraction of B4C in the boronated carbon brick: 5%. |
| `control_rod_channel_count` | `u32` | Number of control-rod borings in the side reflector: 10. |
| `control_rod_channel_diameter` | `uom::si::f64::Length` | Control-rod channel diameter: 13 cm. |
| `control_rod_channel_radius` | `uom::si::f64::Length` | Radial coordinate of a control-rod channel centre: 102.1 cm. |
| `absorber_ball_channel_count` | `u32` | Number of small-absorber-ball borings: 7. |
| `irradiation_channel_count` | `u32` | Number of irradiation borings (13 cm diameter): 3. |
| `helium_channel_count` | `u32` | Number of cold-helium flow channels in the side reflector: 20. |
| `helium_channel_diameter` | `uom::si::f64::Length` | Helium flow channel diameter: 8 cm (80 mm). |
| `helium_channel_radius` | `uom::si::f64::Length` | Radial coordinate of a helium channel centre: 144.6 cm. |
| `helium_channel_bottom` | `uom::si::f64::Length` | Axial coordinate of the lower end of a helium flow channel: 105 cm. |
| `helium_channel_top` | `uom::si::f64::Length` | Axial coordinate of the upper end of a helium flow channel: 610 cm. |

##### Implementations

###### Methods

- ```rust
  pub fn iaea_benchmark() -> Self { /* ... */ }
  ```
  The textual geometry of the IAEA benchmark core-physics model

- ```rust
  pub fn core_radius(self: &Self) -> Length { /* ... */ }
  ```
  Radius of the active pebble bed: half of [`Self::core_diameter`].

- ```rust
  pub fn pebble_count_for_loading_height(self: &Self, loading_height: Length, pebble_diameter: Length, filling_fraction: Ratio) -> f64 { /* ... */ }
  ```
  Number of pebbles of the given diameter that fill a *cylindrical* bed

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10CoreGeometry { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Htr10CoreGeometry) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `LoadingCurvePoint`

One point on a calculated k_eff-versus-loading-height curve.

The benchmark's B1 answer is not a k_eff but a *height*: the loading at
which k_eff = 1. Codes report a curve and interpolate, so the curve is the
primary datum and the critical height is derived from it — see
[`critical_height_from_two_points`].

```rust
pub struct LoadingCurvePoint {
    pub loading_height: uom::si::f64::Length,
    pub fuel_balls: u32,
    pub dummy_balls: u32,
    pub keff: uom::si::f64::Ratio,
    pub keff_standard_deviation: Option<uom::si::f64::Ratio>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `loading_height` | `uom::si::f64::Length` | Loading height from the upper surface of the conus region. |
| `fuel_balls` | `u32` | Number of fuel balls at this loading. Zero where the source does not<br>state it (the MCNP original-benchmark table gives heights only). |
| `dummy_balls` | `u32` | Number of dummy (graphite) balls at this loading. Zero where the source<br>does not state it. |
| `keff` | `uom::si::f64::Ratio` | Calculated effective multiplication factor at this loading,<br>dimensionless. |
| `keff_standard_deviation` | `Option<uom::si::f64::Ratio>` | Monte Carlo standard deviation on `keff`, where the source states one.<br>`None` for deterministic (VSOP) results, which carry no statistical<br>uncertainty — and whose *modelling* uncertainty the source does not<br>quantify. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LoadingCurvePoint { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LoadingCurvePoint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `FirstCriticalityMeasurement`

The **measured** first criticality of the HTR-10, December 2000.

This is the only experimental datum in this module; everything else is a
specification or a calculation. It is the target any B1 calculation is
ultimately judged against — but only after the calculation is set up for
the *deviated* conditions recorded here, not the original definition.

```rust
pub struct FirstCriticalityMeasurement {
    pub total_balls: u32,
    pub fuel_balls: u32,
    pub dummy_balls: u32,
    pub loading_height: uom::si::f64::Length,
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub atmosphere: CoreAtmosphere,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total_balls` | `u32` | Total mixed balls loaded when criticality was reached: 16,890. |
| `fuel_balls` | `u32` | Fuel balls: 9,627. |
| `dummy_balls` | `u32` | Dummy graphite balls: 7,263. |
| `loading_height` | `uom::si::f64::Length` | Corresponding loading height from the upper surface of the conus:<br>123.06 cm. |
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Core atmosphere temperature at criticality: 15 degrees Celsius. |
| `atmosphere` | `CoreAtmosphere` | The atmosphere: air (not helium). |

##### Implementations

###### Methods

- ```rust
  pub fn iaea_reported() -> Self { /* ... */ }
  ```
  The measured first criticality as recorded in the IAEA benchmark

- ```rust
  pub fn fuel_ball_fraction(self: &Self) -> Ratio { /* ... */ }
  ```
  Fuel-ball fraction of the loading: 9627/16890. The design intent is

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FirstCriticalityMeasurement { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FirstCriticalityMeasurement) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `PublishedKeff`

A published k_eff (or k_inf) from a named code, for a named benchmark
problem and variant.

Every field that matters for a fair comparison is present and mandatory:
which problem, which variant, which code, which source. There is no way to
record an eigenvalue here without saying where it came from.

```rust
pub struct PublishedKeff {
    pub problem: BenchmarkProblem,
    pub variant: BenchmarkVariant,
    pub code: NeutronicsCode,
    pub keff: uom::si::f64::Ratio,
    pub standard_deviation: Option<uom::si::f64::Ratio>,
    pub source: LiteratureSource,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `problem` | `BenchmarkProblem` | The problem this value answers. |
| `variant` | `BenchmarkVariant` | Original (as defined) or deviated (as built) conditions. |
| `code` | `NeutronicsCode` | The code that produced it. |
| `keff` | `uom::si::f64::Ratio` | The eigenvalue, dimensionless. |
| `standard_deviation` | `Option<uom::si::f64::Ratio>` | Statistical standard deviation where the source states one. |
| `source` | `LiteratureSource` | Where it was published. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PublishedKeff { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PublishedKeff) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `BenchmarkProblem`

The IAEA HTR-10 core-physics benchmark problems.

B2, B3 and B4 each have sub-problems, which are separate variants here
because they are separate calculations with separate answers.

```rust
pub enum BenchmarkProblem {
    B1InitialCriticality,
    B21FullCore20C,
    B22FullCore120C,
    B23FullCore250C,
    B31TenRodsFullCore,
    B32OneRodFullCore,
    B41TenRodsInitialCore,
    B42OneRodDifferential,
    InfinitePebbleBedKinf,
}
```

##### Variants

###### `B1InitialCriticality`

**B1 — initial criticality.** Determine the loading height (from the
upper surface of the conus) at which k_eff = 1, under helium at a core
temperature of 20 degrees Celsius, with no control rod inserted. The
answer is a *height*, not a k_eff.

###### `B21FullCore20C`

**B21** — full core (5 m^3) k_eff under helium at 20 degrees Celsius,
no rods inserted. Pebble-bed height 180.114 cm.

###### `B22FullCore120C`

**B22** — full core k_eff under helium at 120 degrees Celsius.

###### `B23FullCore250C`

**B23** — full core k_eff under helium at 250 degrees Celsius.

###### `B31TenRodsFullCore`

**B31** — reactivity worth of the ten fully inserted control rods, full
core, helium, 20 degrees Celsius.

###### `B32OneRodFullCore`

**B32** — reactivity worth of one fully inserted control rod (the other
nine withdrawn), full core, helium, 20 degrees Celsius.

###### `B41TenRodsInitialCore`

**B41** — reactivity worth of the ten fully inserted control rods for
the initial core at a loading height of 126 cm, helium,
20 degrees Celsius.

###### `B42OneRodDifferential`

**B42** — differential worth of one control rod for the initial core at
126 cm loading, at seven stated axial positions of the rod's lower end.

###### `InfinitePebbleBedKinf`

Infinite pebble-bed lattice k_inf — not an IAEA problem, but the
standard first step several papers report before the full core.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BenchmarkProblem { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BenchmarkProblem) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `ControlRodWorth`

A control-rod reactivity worth, in percent delta-k/k as published.

The sources report rod worths as percentages, not in dollars — converting
to dollars needs a delayed-neutron fraction the benchmark does not state,
so no conversion is offered here.

```rust
pub struct ControlRodWorth {
    pub problem: BenchmarkProblem,
    pub variant: BenchmarkVariant,
    pub code: NeutronicsCode,
    pub worth: uom::si::f64::Ratio,
    pub source: LiteratureSource,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `problem` | `BenchmarkProblem` | Which control-rod problem this is. |
| `variant` | `BenchmarkVariant` | Original or deviated benchmark conditions. |
| `code` | `NeutronicsCode` | The code that produced it. |
| `worth` | `uom::si::f64::Ratio` | The worth, as a dimensionless ratio (a published "15.24%" is stored as<br>0.1524). |
| `source` | `LiteratureSource` | Where it was published. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ControlRodWorth { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ControlRodWorth) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `critical_height_from_two_points`

Linearly interpolate (or extrapolate) the loading height at which
k_eff = 1, from two points on a loading curve.

h_crit = h_low + (h_high - h_low) * (1 - k_low) / (k_high - k_low).

This is the procedure the IAEA benchmark document itself used to state its
B1 answers, and the unit tests in this module reproduce all four of its
published critical heights with it. When both k values are below 1 the
result is an **extrapolation** beyond `high` — the IAEA's own MCNP
original-benchmark answer (126.116 cm from points at 120 and 126 cm) is
exactly such a case, so this is intended behaviour, not a bug.

Returns `None` if the two k values are equal (no slope to invert).

```rust
pub fn critical_height_from_two_points(low: &LoadingCurvePoint, high: &LoadingCurvePoint) -> Option<uom::si::f64::Length> { /* ... */ }
```

#### Function `vsop_original_loading_curve_20c`

INET's VSOP k_eff-versus-loading-height curve for the **original**
benchmark B1 under helium, at 20 degrees Celsius (IAEA benchmark document,
Table 4-4, Open tier; the same table's 27 degrees Celsius column is
[`vsop_original_loading_curve_27c`]).

Twelve loadings from 90 cm to 190 cm. The document derives a critical
height of 125.804 cm from the 120 cm and 126 cm points of this curve.

```rust
pub fn vsop_original_loading_curve_20c() -> [LoadingCurvePoint; 12] { /* ... */ }
```

#### Function `vsop_original_loading_curve_27c`

INET's VSOP curve for the original benchmark B1 at **27 degrees Celsius**
(IAEA benchmark document, Table 4-4, Open tier).

Provided alongside the 20 degrees Celsius curve because the benchmark text
defines B1 at 20 degrees Celsius while much of the later literature
evaluates it at 27 degrees Celsius. The difference is about 15 pcm at the
critical loading.

```rust
pub fn vsop_original_loading_curve_27c() -> [LoadingCurvePoint; 12] { /* ... */ }
```

#### Function `mcnp_original_loading_curve_27c`

INET's MCNP curve for the **original** benchmark B1 under helium at
27 degrees Celsius (IAEA benchmark document, Table 4-5, Open tier).

Five loadings. Ball counts are not given in that table and are recorded as
zero. The document derives a critical height of 126.116 cm by
extrapolating the 120 cm and 126 cm points.

```rust
pub fn mcnp_original_loading_curve_27c() -> [LoadingCurvePoint; 5] { /* ... */ }
```

#### Function `vsop_deviated_loading_curve_27c`

INET's VSOP curve for the **deviated** benchmark B1 under humid air at
27 degrees Celsius (IAEA benchmark document, Table 4-10, Open tier).

Only two loadings were computed. The document derives a critical height of
122.558 cm (16,821 balls) from them.

```rust
pub fn vsop_deviated_loading_curve_27c() -> [LoadingCurvePoint; 2] { /* ... */ }
```

#### Function `mcnp_deviated_loading_curve_27c`

INET's MCNP curve for the **deviated** benchmark B1 under humid air at
27 degrees Celsius (IAEA benchmark document, Table 4-11, Open tier).

The document derives a critical height of 122.874 cm (16,864 balls) from
these two points.

```rust
pub fn mcnp_deviated_loading_curve_27c() -> [LoadingCurvePoint; 2] { /* ... */ }
```

#### Function `inet_b2_results`

INET's B2 full-core results, calculated with VSOP, for both variants
(IAEA benchmark document, Tables 4-6 and 4-12, Open tier).

The full core is the 5 m^3 core, corresponding to a pebble-bed height of
180.114 cm (14,091 fuel + 10,630 dummy balls = 24,721 mixed balls).
All six values are under **helium** — the deviated B2 differs from the
original only in the dummy-ball density and impurity, not in atmosphere,
because B2 was defined as a helium problem and INET kept it that way.

```rust
pub fn inet_b2_results() -> [PublishedKeff; 7] { /* ... */ }
```

#### Function `choo_xiao_2024_results`

Choo and Xiao (2024), Table 2 — simplified-model Serpent 2 and HTR Code
Package results for B1 and B2, both variants (Open tier).

Model: azimuthally symmetric simplified benchmark geometry; pebble
positions from a LAMMPS discrete-element packing, TRISO positions from
Serpent's automated disperser; ENDF/B-VII.0. Serpent used 5000 neutrons per
cycle, with a stated statistical uncertainty range of +/-0.00080 to
+/-0.00089 across the runs — the paper does not attribute a specific sigma
to each entry, so `standard_deviation` is `None` here rather than being
filled with a guess.

One caveat carried from the paper: their B1 uses the measured 123.06 cm
loading height for **both** variants, so their "original B1" is the
as-measured *loading* evaluated with as-defined *materials*.

```rust
pub fn choo_xiao_2024_results() -> [PublishedKeff; 16] { /* ... */ }
```

#### Function `tantillo_2020_infinite_pebble_bed`

Tantillo et al. (2020) infinite-pebble-bed k_inf comparison
(Proprietary tier — cited, not re-hosted).

An HTR-10 infinite pebble-bed lattice with reflective boundaries, 8335
coated particles per pebble, 5 g heavy metal, 17% enrichment,
ENDF/B-VII.0: HTR Code Package 1.6416 versus Serpent 1.6321, a relative
difference of 0.58% (about 950 pcm). This is the cheapest possible
first target for any new code — no core geometry, no reflector, no
leakage.

**Only this table and the temperature coefficients could be read reliably
from that paper's markdown conversion**; its B1/B2 result tables are
corrupted by an OCR substitution artefact and are deliberately not
transcribed here. See `docs/reactor-scoping/htr10-neutronics.md`.

```rust
pub fn tantillo_2020_infinite_pebble_bed() -> [PublishedKeff; 2] { /* ... */ }
```

#### Function `wang_2014_continuous_energy`

Wang et al. (2014) continuous-energy results for three pebble-bed
configurations (Proprietary tier — cited, not re-hosted).

Configuration (a) is an infinite simple-cubic lattice of fuel pebbles
(k_inf), (b) a body-centred-cubic lattice mixing fuel pebbles and reduced-
diameter graphite balls at the 57:43 ratio and 61% packing (k_inf), and
(c) a detailed three-dimensional HTR-10 initial-critical core model with
peripheral reflectors, helium tubes, irradiation channels and control rods
(k_eff), at the measured 16,890-ball / 123.06 cm loading.

The MCNP5-versus-SCALE6 spread on configuration (c) is 683 +/- 22 pcm using
the *same* ENDF/B-VII.0 library and the same geometry — the paper's own
conclusion is that this is a code discrepancy, not a modelling one, and
that the MCNP5 values are the more reliable. **Any code-to-code agreement
target tighter than about 700 pcm on this problem is therefore tighter than
the published spread between two mature codes.**

```rust
pub fn wang_2014_continuous_energy() -> [PublishedKeff; 6] { /* ... */ }
```

#### Function `wang_2014_unit_cell_bias`

The k_eff bias, in pcm, that each SCALE6 multigroup unit-cell treatment
introduces on the detailed HTR-10 initial-critical model, relative to
continuous-energy MCNP5 (Wang et al. 2014, Table 2, configuration (c);
Proprietary tier — cited, not re-hosted).

**Read this before choosing any homogenisation scheme.** Getting the double
heterogeneity wrong is not a small correction: treating the fuel as an
infinite homogeneous medium costs +2820 pcm, which on a system whose whole
excess reactivity at first criticality is a few hundred pcm is a completely
different reactor. Even the correct doubly heterogeneous treatment leaves
+276 pcm.

Returns `(treatment, bias_pcm, bias_uncertainty_pcm)`.

```rust
pub fn wang_2014_unit_cell_bias() -> [(UnitCellTreatment, f64, f64); 6] { /* ... */ }
```

#### Function `inet_control_rod_worths`

INET's B3 and B4 control-rod worths for both variants (IAEA benchmark
document, Tables 4-7, 4-8, 4-13, 4-14 and the summary Table 4-16; Open
tier). All are for a 27 degrees Celsius helium atmosphere; the document
states that humid air was found to have a negligible effect on rod worth
and so was not modelled for B3/B4 even in the deviated case.

```rust
pub fn inet_control_rod_worths() -> [ControlRodWorth; 14] { /* ... */ }
```

#### Function `b42_differential_rod_worth_curve`

The B42 differential rod-worth curve: integral worth of one control rod as
its lower end moves to each of seven stated axial positions, initial core
at 126 cm loading, VSOP (IAEA benchmark document, Tables 4-9 and 4-15;
Open tier).

Returns `(axial_position, worth_original, worth_deviated)`. The rod's fully
withdrawn lower end is at 119.2 cm and fully inserted at 394.2 cm, so the
last point of this curve is the fully inserted position.

```rust
pub fn b42_differential_rod_worth_curve() -> [(uom::si::f64::Length, uom::si::f64::Ratio, uom::si::f64::Ratio); 7] { /* ... */ }
```

#### Function `measured_s3_rod_worth`

The measured integral worth of control rod S3, from the rod-worth
calibration experiment (IAEA benchmark document, Open tier): 1.4693%
delta-k/k, with the core loaded to 17,000 balls (a loading height of
123.86 cm) and the rod's lower end moved from z = 171.2 cm to
z = 394.2 cm.

**This is not directly comparable to B42.** The B42 calculation specifies
126 cm loading (17,293-17,294 balls) and a rod travel from z = 119.2 cm,
against the experiment's 17,000 balls and travel from z = 171.2 cm. The
IAEA document argues the difference of about 293 balls and the air
atmosphere have a minor effect, but the comparison is not like-for-like and
must be described as such.

```rust
pub fn measured_s3_rod_worth() -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `rod_calibration_loading_height`

The control-rod loading and travel used in the *measured* rod-worth
calibration: 17,000 balls at a 123.86 cm loading height.

```rust
pub fn rod_calibration_loading_height() -> uom::si::f64::Length { /* ... */ }
```

#### Function `vsop_temperature_corrected_prediction`

INET's VSOP prediction of the critical loading after correcting from
27 degrees Celsius to the experiment's 15 degrees Celsius: 16,759 mixed
balls, corresponding to 122.11 cm (IAEA benchmark document, Open tier).

This is the number the document itself compares to the measured 16,890
balls / 123.06 cm when it states the calculation error was "less than one
percent" — see the unit test
`published_predictions_are_within_one_percent_of_the_measurement`.

```rust
pub fn vsop_temperature_corrected_prediction() -> (u32, uom::si::f64::Length) { /* ... */ }
```

### Constants and Statics

#### Constant `U235_MOLAR_MASS_G_PER_MOL`

Molar mass of U-235 in g/mol (standard nuclide mass, open reference data).

```rust
pub const U235_MOLAR_MASS_G_PER_MOL: f64 = 235.0439;
```

#### Constant `U238_MOLAR_MASS_G_PER_MOL`

Molar mass of U-238 in g/mol (standard nuclide mass, open reference data).

```rust
pub const U238_MOLAR_MASS_G_PER_MOL: f64 = 238.0508;
```

#### Constant `OXYGEN_MOLAR_MASS_G_PER_MOL`

Standard atomic weight of oxygen in g/mol (open reference data). The
kernel is UO2 of natural oxygen.

```rust
pub const OXYGEN_MOLAR_MASS_G_PER_MOL: f64 = 15.9994;
```

## Module `zbs`

# Zehner-Bauer-Schlunder effective pebble-bed thermal conductivity

An 18-point tabulation of the effective thermal conductivity of a pebble
bed computed from the Zehner-Bauer-Schlunder (ZBS) correlation, together
with a linear interpolant over it. The tabulation is transcribed verbatim
from the Virtual Test Bed generic PBR input deck (Open tier;
`reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i`, block
`keff_pebble_bed`), which spans 300 K to 2000 K and 11.94 to
44.95 W/(m K).

- **Belongs here:** the tabulated values, the interpolant, and the tests
  proving the table is transcribed exactly and the interpolant behaves.
- **Does NOT belong here:** an analytic ZBS implementation (a future
  closure under bead `op-jyyp.3` — implementing the full correlation and
  checking it against this table would itself be a V&V step), design
  constants, or solver state.

**Caveat.** The tabulation is for the VTB *generic* pebble bed (computed
by its authors from ZBS at that bed's conditions), not specifically for
the HTR-10 bed; it is checked in here as the published reference an
analytic ZBS implementation must reproduce, and as an interim effective
conductivity of a 6 cm-pebble helium bed. Not validated against HTR-10
measurements.

```rust
pub mod zbs { /* ... */ }
```

### Functions

#### Function `zbs_effective_conductivity`

Effective pebble-bed thermal conductivity (W/(m K)) at the given
temperature (K), by linear interpolation over the VTB ZBS tabulation
(300 K to 2000 K). Outside the tabulated range the value is **clamped**
to the nearest endpoint (11.940293 W/(m K) below 300 K, 44.9504677
W/(m K) above 2000 K) — extrapolating a conductivity table is worse than
clamping it, and the clamp is exercised by a test. At a tabulated node
the tabulated value is returned exactly.

```rust
pub fn zbs_effective_conductivity(temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::ThermalConductivity { /* ... */ }
```

### Constants and Statics

#### Constant `ZBS_TABLE_LEN`

Number of points in the VTB ZBS tabulation.

```rust
pub const ZBS_TABLE_LEN: usize = 18;
```

#### Constant `ZBS_TEMPERATURE_KELVIN`

Temperatures of the VTB ZBS tabulation, in kelvin: 300 K to 2000 K in
100 K steps. Transcribed from `pbr.i` block `keff_pebble_bed`, row `x`
(Open tier).

```rust
pub const ZBS_TEMPERATURE_KELVIN: [f64; 18] = _;
```

#### Constant `ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN`

Effective bed thermal conductivity of the VTB ZBS tabulation, in watts
per metre-kelvin, matching [`ZBS_TEMPERATURE_KELVIN`] point-for-point.
Transcribed verbatim from `pbr.i` block `keff_pebble_bed`, row `y` (Open
tier).

```rust
pub const ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN: [f64; 18] = _;
```

## Module `app_scaffold`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-park-digital-twin-engine/src/lib.rs:92:11: 92:32 (#0) }, crates/outram-park-digital-twin-engine/src/lib.rs:92:10: 92:33 (#0))])]")`

Reusable `eframe::App` threading/locking + panel-dispatch scaffold.

Ported from the shared-state/physics-thread pattern used by
`tampines-steam-tables`'s `fhr_sim_v1`/`fhr_sim_v2` examples
(`examples/fhr_sim_v2/main.rs`'s `FHRSimulatorApp`/`FHRState`: one
`Arc<Mutex<FHRState>>` cloned into three spawned threads -- PRKE
kinetics, thermal-hydraulics, and plot-data updates -- each looping
indefinitely, locking to read inputs and write results back, while the
GUI thread locks briefly to clone a rendering snapshot). Real, working
code -- this pattern is simple and already proven twice; the content of
any given panel is still the calling application's job.

**Deliberate deviation from the original:** this uses
[`Arc`]`<`[`RwLock`]`<T>>`, not `Arc<Mutex<T>>`, per this workspace's
mandatory Rust design rules -- `RwLock` allows concurrent reads from
multiple threads, where `Mutex` serialises even read-only access and so
defeats parallelism during a timestep's compute phase.

```rust
pub mod app_scaffold { /* ... */ }
```

### Modules

## Module `crash`

Physics-thread panic detection + a "please restart" crash-notification
modal, shared by every digital-twin simulator built on this scaffold.

A digital-twin simulator spawns one or more background *physics threads*
(see [`spawn_physics_thread`](super::spawn_physics_thread)) that update an
[`Arc`]`<`[`RwLock`]`<T>>` while the GUI thread reads it each frame. If one
of those threads panics -- e.g. a thermal-hydraulics step drives a property
out of its valid IAPWS range and unwraps a `NonConvergent`, or a `(p, h)`
flash lands off the steam dome -- the physics simply stops. Without this
module the GUI would keep painting stale numbers forever, giving the user no
signal that the simulation is dead.

The pieces here close that gap:

- [`ThreadHealth`] -- a cheap-to-clone shared flag. Clone one handle into
  each monitored thread and one into the GUI.
- [`spawn_monitored`] / [`spawn_physics_thread_monitored`] -- spawn a thread
  whose body is wrapped in [`std::panic::catch_unwind`]; on a panic they
  record the panic message (downcast from the payload) into the shared
  [`ThreadHealth`] instead of letting the thread die silently.
- [`show_crash_modal_if_crashed`] -- a one-line GUI helper: if any monitored
  thread has crashed it draws an unmissable [`egui::Modal`] (centered,
  backdrop-dimmed, input-blocking) telling the user to restart the
  simulator, with the captured panic message under a details header.
- [`show_crash_modal_with_restart`] -- the same modal plus a **Restart
  simulation** button, returning a [`CrashModalOutcome`] so the caller can
  act on the click.

It deliberately does **not** try to restart the *process*, and it never
resumes the crashed run: a panicked physics thread may have poisoned a
shared lock mid-write, so the state it was building is not trustworthy. To
keep the GUI itself from cascade-panicking on that poisoned lock,
[`SharedState`](super::SharedState) recovers poisoned guards, and simulators
should early-return from their frame once the modal is shown (so they never
touch a poisoned `Mutex`).

# What "restart" means here

[`show_crash_modal_with_restart`] only *reports the click*. Honouring it is
the simulator's job, and the only safe way to honour it is to **start a new
run from defaults, never to resume the old one**: build fresh
[`SharedState`](super::SharedState) handles and a fresh [`ThreadHealth`],
spawn new monitored threads against them, and drop the old handles without
reading them. Anything that reaches back into the crashed run's state --
carrying over a snapshot, reusing the old `ThreadHealth`, re-locking the
poisoned `RwLock` to "recover" a value -- reintroduces exactly the hazard
this module exists to contain.

# A run's threads stop together

[`spawn_physics_thread_monitored`] checks [`ThreadHealth::is_running`] at the
top of every iteration, so all of a run's loops end when the run does --
whether because **any** one of them panicked, or because the application
called [`ThreadHealth::retire`] to end the run deliberately.

A simulator typically runs several loops against one `ThreadHealth` (physics,
plot sampler, ...). Without a shared stop condition only the loop that
actually panicked would stop, leaving the survivors spinning against a dead
run's state forever -- one leaked thread per restart. That is what makes a
restart a clean swap rather than an accumulation.

```rust
pub mod crash { /* ... */ }
```

### Types

#### Struct `CrashReport`

A short, `Clone`-able record of the first background thread to panic:
which thread it was and the panic message extracted from its payload.

The `message` is the human-readable panic string (`panic!("...")` text or a
`.unwrap()`/`.expect()` message) recovered by downcasting the panic payload
to `&str` / `String`; if the payload was some other type it is a fixed
placeholder rather than the real value.

```rust
pub struct CrashReport {
    pub thread_name: String,
    pub message: String,
    pub location: Option<String>,
    pub component: Option<&'static str>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `thread_name` | `String` | The name passed to [`spawn_monitored`] /<br>[`spawn_physics_thread_monitored`] for the thread that panicked. |
| `message` | `String` | The panic message (best-effort, downcast from the panic payload). |
| `location` | `Option<String>` | Source location of the panic as `file:line:column`, e.g.<br>`"examples/htgr_sim_v1/physics/secondary_loop.rs:388:9"`.<br><br>**This is the field that identifies which component failed.** The<br>`thread_name` only names the thread, and a simulator that runs its whole<br>plant on one physics thread (as `htgr_sim_v1` does) would otherwise<br>report nothing more useful than `"htgr-physics"`.<br><br>`None` if the panic hook did not fire or the panic carried no location —<br>possible for a panic raised through a path that bypasses the standard<br>hook. Treat it as best-effort diagnostics, not a guarantee. |
| `component` | `Option<&'static str>` | The **plant component** being stepped when the panic happened, as set by<br>[`mark_component`] -- e.g. `"steam generator"`.<br><br>This is what a crash report should lead with: a source location tells a<br>developer where to look, but this tells the operator which piece of<br>equipment failed. `None` if the simulator does not mark its components. |

##### Implementations

###### Methods

- ```rust
  pub fn summary(self: &Self) -> String { /* ... */ }
  ```
  One-line human summary: the thread, the location if known, and the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CrashReport { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `ThreadHealth`

A shared "is this simulator run still going, and if not why not?" flag,
plus the [`CrashReport`] of the first panic.

Backed by an [`Arc`] internally, so [`Clone`] just bumps the refcount --
clone one handle per monitored thread (they *record* into it) and one for
the GUI (which *queries* it every frame via
[`has_crashed`](Self::has_crashed) / [`crash_report`](Self::crash_report)).
Only the first panic is kept; later panics from other threads are ignored so
the reported cause is the root one.

It carries **two** independent reasons a run can end, and they mean different
things to a user:

- **crashed** -- a monitored thread panicked. Recorded automatically by
  [`spawn_monitored`]; surfaced by [`has_crashed`](Self::has_crashed) and the
  crash modal.
- **retired** -- the application deliberately ended the run, e.g. because the
  operator restarted the simulator. Set by [`retire`](Self::retire); shows up
  *only* in [`is_running`](Self::is_running).

Keeping them apart is what stops a restart from looking like a fault:
retiring a run never invents a crash report, and never clears a real one.

```rust
pub struct ThreadHealth {
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
  Create a fresh, healthy handle (no crash recorded, not retired).

- ```rust
  pub fn has_crashed(self: &Self) -> bool { /* ... */ }
  ```
  Has any monitored thread panicked? Cheap (a single atomic load) -- safe

- ```rust
  pub fn is_running(self: &Self) -> bool { /* ... */ }
  ```
  Should this run's monitored loops keep stepping?

- ```rust
  pub fn retire(self: &Self) { /* ... */ }
  ```
  Ask this run's monitored loops to stop, without recording a fault.

- ```rust
  pub fn crash_report(self: &Self) -> Option<CrashReport> { /* ... */ }
  ```
  The [`CrashReport`] of the first thread to panic, or `None` if all

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThreadHealth { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `CrashModalOutcome`

What [`show_crash_modal_with_restart`] did this frame, and what the caller
should do about it.

Returned instead of a bare `bool` because a crash modal carrying a restart
button has **three** outcomes, not two, and the extra one is the whole point:
"the simulation is dead" and "the user asked for a new one" call for
different actions and must not collapse into one flag.

```rust
pub enum CrashModalOutcome {
    Healthy,
    Showing,
    RestartRequested,
}
```

##### Variants

###### `Healthy`

No monitored thread has panicked. Nothing was drawn; render normally.

###### `Showing`

The modal is up and the user has not asked for anything. Stop rendering
this frame (the run's state is frozen and possibly poisoned) and repaint.

###### `RestartRequested`

The user clicked **Restart simulation**. The modal was drawn, so still
stop rendering this frame -- but start a fresh run first. See the module
docs for what "fresh" has to mean.

##### Implementations

###### Methods

- ```rust
  pub fn is_crashed(self: Self) -> bool { /* ... */ }
  ```
  Whether a crash modal was drawn -- i.e. anything other than

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CrashModalOutcome { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CrashModalOutcome) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `mark_component`

Records which **plant component** the calling physics thread is about to
step, so a crash can be attributed to a piece of equipment rather than only
to a source file.

# Why this exists

A source location tells a *developer* where a panic happened. It does not
tell the person running the simulator which part of the plant misbehaved,
and in a simulator whose whole plant runs on one physics thread the thread
name is no help either. Calling this at the head of each subsystem's step
turns "panicked in `secondary_loop.rs:388`" into "the steam generator
failed", which is what a crash report should lead with.

# Usage

Call it as the plant walks its subsystems, in the same order they step:

```ignore
mark_component("reactor kinetics");
self.kinetics.step(dt, rho);
mark_component("pebble bed");
self.core.step(dt, power, ...);
mark_component("steam generator");
self.secondary.step(dt, duty, hot_side);
```

The name should read as **equipment a plant operator would recognise**
("steam generator", "helium circulator", "hot gas duct"), not as a module
path. Take `&'static str` so marking costs a pointer store per subsystem per
timestep -- negligible beside the physics it precedes.

It is *not* a stack: each call replaces the previous mark, so the report
names the innermost component that was marked, not a nesting chain.

```rust
pub fn mark_component(component: &'static str) { /* ... */ }
```

#### Function `spawn_monitored`

Spawn `body` on a new OS thread, wrapping it in
[`std::panic::catch_unwind`] so that a panic is *reported* to `health`
(with `thread_name` and the panic message) instead of unwinding out of the
thread and being lost.

This is the general escape hatch for simulators that spawn their physics
loops by hand (e.g. the `fhr_sim_v2` example, which drives `Arc<Mutex<_>>`
directly rather than through [`SharedState`]). For the common looping
physics-thread pattern prefer [`spawn_physics_thread_monitored`].

`thread_name` is a human label surfaced in the crash modal (e.g.
`"fhr-thermal-hydraulics"`); it does not need to be unique but should
identify the subsystem. The returned [`JoinHandle`] joins cleanly (returns
`Ok`) even when `body` panicked, because the panic is caught inside.

```rust
pub fn spawn_monitored<F, /* synthetic */ impl Into<String>: Into<String>>(thread_name: impl Into<String>, health: ThreadHealth, body: F) -> std::thread::JoinHandle<()>
where
    F: FnOnce() + Send + ''static { /* ... */ }
```

#### Function `spawn_physics_thread_monitored`

The monitored counterpart of
[`spawn_physics_thread`](super::spawn_physics_thread): repeatedly calls
`step` against a cloned [`SharedState`] handle, but if `step` ever panics the
panic is caught and reported to `health` rather than silently killing the
thread.

Use this in place of `spawn_physics_thread` wherever you want a crashed
physics loop to surface the [`show_crash_modal_if_crashed`] restart prompt.
`thread_name` labels the subsystem in that modal.

# The loop is not infinite: it ends when the *run* does

[`ThreadHealth::is_running`] is checked at the top of every iteration, so
this thread returns as soon as the run it belongs to has ended -- either
because **any** monitored thread sharing that [`ThreadHealth`] panicked (not
only this one), or because the application called
[`ThreadHealth::retire`].

Both exits matter for the same reason. A simulator runs several of these
loops against a single `ThreadHealth` (a physics loop and a plot sampler,
say), and without a shared stop condition a panic in one would leave the
others alive, still stepping and still writing into a dead run's state. That
is merely untidy while the process is about to be closed, and becomes a
leaked thread per click once an in-app restart button exists (see
[`show_crash_modal_with_restart`]) -- the restart hands the GUI new state
while the survivors keep the old state alive and keep burning a core.

The check is two `Acquire` atomic loads per timestep. A loop already inside a
long sleep or a long `step` exits at the end of that call, not instantly; the
exit is prompt, not immediate.

```rust
pub fn spawn_physics_thread_monitored<T, F, /* synthetic */ impl Into<String>: Into<String>>(thread_name: impl Into<String>, state: super::SharedState<T>, health: ThreadHealth, step: F) -> std::thread::JoinHandle<()>
where
    T: Send + Sync + ''static,
    F: FnMut(&super::SharedState<T>) + Send + ''static { /* ... */ }
```

#### Function `show_crash_modal_if_crashed`

If any monitored thread has panicked, draw an unmissable modal telling the
user the simulation has crashed and to restart it, and return `true`;
otherwise draw nothing and return `false`.

The modal is an [`egui::Modal`]: centered, with a backdrop that dims and
blocks input to the rest of the UI, so it cannot be missed or dismissed by
clicking elsewhere. The captured panic message is shown under a collapsible
"Technical details" header.

This variant tells the user to close and relaunch the process. For an
in-app restart button, use [`show_crash_modal_with_restart`] instead.

Intended call pattern -- at the very top of a simulator's `eframe::App`
frame, so a crashed run never renders (and never touches a possibly-poisoned
lock):

```ignore
fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    if show_crash_modal_if_crashed(ui.ctx(), &self.thread_health) {
        ui.ctx().request_repaint();
        return;
    }
    // ... normal rendering ...
}
```

```rust
pub fn show_crash_modal_if_crashed(ctx: &egui::Context, health: &ThreadHealth) -> bool { /* ... */ }
```

#### Function `show_crash_modal_with_restart`

[`show_crash_modal_if_crashed`] plus a **Restart simulation** button,
reporting the click through a [`CrashModalOutcome`].

# What this does and does not do

It draws and it reports. It does **not** restart anything itself -- it has
no access to the caller's threads or state, and could not safely touch the
crashed run's state even if it did. Honouring
[`RestartRequested`](CrashModalOutcome::RestartRequested) means *starting a
new run from defaults*: fresh [`SharedState`](super::SharedState) handles,
a fresh [`ThreadHealth`], new monitored threads, and the old handles dropped
unread. See this module's "What restart means here" for why resuming is not
on the table.

The sibling threads of the crashed run stop by themselves -- see
[`spawn_physics_thread_monitored`] -- so the swap does not leak a thread per
click.

# Intended call pattern

```ignore
fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    let outcome = show_crash_modal_with_restart(ui.ctx(), &self.thread_health);
    if outcome == CrashModalOutcome::RestartRequested {
        self.restart_simulation();
    }
    if outcome.is_crashed() {
        ui.ctx().request_repaint();
        return;
    }
    // ... normal rendering ...
}
```

Note the order: restart *then* return. Rendering is skipped on the restart
frame too, because the new run has not taken its first step yet.

```rust
pub fn show_crash_modal_with_restart(ctx: &egui::Context, health: &ThreadHealth) -> CrashModalOutcome { /* ... */ }
```

## Module `gui_frame_metrics`

GUI-thread frame timing, shared by every simulator built on this scaffold.

The example simulators instrument their *physics* threads carefully -- the
PRKE and thermal-hydraulics loops publish their per-timestep wall-clock cost
into shared state and the side panels display it. Nothing, however, measured
the **GUI thread**, so a report of "the simulator feels laggy" could not be
attributed: a slow render and a slow physics step look identical from the
outside.

This module closes that gap. It was written for `fhr_sim_v2` on 2026-08-12
and promoted here the same day, because the lag it was written to diagnose
turned out to be in `src/components/` and therefore common to every simulator
the engine hosts (see `op-szmi.12`).

It is deliberately **GUI-owned state** -- it belongs on the `eframe::App`
struct, not inside [`crate::app_scaffold::SharedState`], because writing a
frame time into shared state would mean taking the physics lock once per
frame purely to record a diagnostic.

# What the two numbers mean

The distinction matters for diagnosis, and they are not interchangeable:

- **`update_cpu_time_ms`** -- wall-clock time spent inside the app's own
  [`eframe::App::update`] body: cloning state, laying out panels, building
  widgets. This is the part of the frame the application is responsible for
  and the only part it can make faster by editing its own code.
- **`frame_interval_ms`** -- the interval between presented frames, taken
  from egui's own smoothed `stable_dt`. This is what the eye actually
  perceives as smooth or laggy.

**The gap between them is not the app's.** `update_cpu_time_ms` stops when
the closure returns; egui then tessellates the shapes and the backend paints
and presents them, and with vsync enabled the present blocks until the
display is ready. So on a healthy 60 Hz system the expected reading is a
*small* CPU time inside a `frame_interval_ms` of about 16.7 -- the difference
is idle waiting for vsync, not work.

# Reading it

| Observation | Interpretation |
|---|---|
| CPU low, interval ~16.7 ms | Healthy. Vsync-limited; the GUI is idle most of the frame. |
| CPU low, interval much > 16.7 ms | Cost is outside the `update` body -- tessellation, GPU, compositor, or a display running below 60 Hz. |
| CPU approaching the interval | The app's own render is the bottleneck; optimising it will help. |
| CPU peak >> CPU mean | Intermittent hitching rather than uniform slowness -- look for work done on only some frames. |

That third row is exactly how `op-szmi.12` was diagnosed: a small `update`
time inside a long frame interval said the cost was **egui tessellation**, so
the fix was fewer shapes (baking the pebble bed to a texture -- see
[`crate::components::pebble_bed_texture`]) rather than a faster `ui` body.

The **peak** column exists because perceived lag tracks the worst frames,
not the average: a mean of 4 ms with occasional 60 ms spikes feels far worse
than a steady 10 ms, and a mean alone hides that entirely.

# Honest limits of this measurement

- It measures **the `update` body only** -- not egui's tessellation, not the
  `wgpu`/`glow` paint, not present/swap. A frame can be slow in ways this
  number cannot see. It bounds the application's contribution from above; it
  does not account for the whole frame.
- `frame_interval_ms` is egui's *smoothed* `stable_dt`, chosen so the label
  is readable rather than a blur of jitter. A single catastrophic frame is
  therefore damped in the interval column -- the peak CPU column is the
  place to look for one-offs.
- Both figures are exponentially smoothed here as well (see
  [`GuiFrameMetrics::SMOOTHING`]); they are estimates for a human reading a
  panel, not a profiler's output. Treat a surprising reading as a reason to
  run a real profiler, not as a measurement to quote.
- This is a diagnostic, not a benchmark: the numbers depend on window size,
  which panel is open, compositor, and display refresh rate.

```rust
pub mod gui_frame_metrics { /* ... */ }
```

### Types

#### Struct `GuiFrameMetrics`

Rolling GUI-thread frame timings, owned by the app and updated once per
repaint.

Construct via [`Default`] and drive it with [`GuiFrameMetrics::begin_frame`]
at the top of the render and [`GuiFrameMetrics::end_frame`] at the bottom.
Frames that early-return (a crash modal, for instance) simply do not record;
the next complete frame overwrites the pending start instant, so a skipped
frame cannot inflate a later reading.

All stored times are in **milliseconds**.

```rust
pub struct GuiFrameMetrics {
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
  pub fn begin_frame(self: &mut Self) { /* ... */ }
  ```
  Marks the start of a frame. Call at the top of the render body.

- ```rust
  pub fn end_frame(self: &mut Self, ctx: &egui::Context) { /* ... */ }
  ```
  Closes out the frame: records how long the `update` body took, and reads

- ```rust
  pub fn update_cpu_time_ms(self: &Self) -> f64 { /* ... */ }
  ```
  Smoothed time spent inside the app's own `update` body \[ms\].

- ```rust
  pub fn peak_update_cpu_time_ms(self: &Self) -> f64 { /* ... */ }
  ```
  Decaying peak of the per-frame `update` body time \[ms\].

- ```rust
  pub fn frame_interval_ms(self: &Self) -> f64 { /* ... */ }
  ```
  Smoothed interval between presented frames \[ms\].

- ```rust
  pub fn frames_per_second(self: &Self) -> f64 { /* ... */ }
  ```
  Frames per second implied by [`GuiFrameMetrics::frame_interval_ms`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GuiFrameMetrics { /* ... */ }
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
    fn default() -> GuiFrameMetrics { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
## Module `real_time_pacing`

Real-time pacing for fixed-timestep simulation loops, in this workspace's
house pattern.

# Where the pattern comes from

This is the pacing scheme the maintainer wrote for the `fhr_sim_v2` example
(`examples/fhr_sim_v2/app/prke_backend/mod.rs` and
`app/thermal_hydraulics_backend/mod.rs`), lifted into the shared scaffold so
the simulators do not each carry a hand-rolled copy. Its shape:

1. Keep a **cumulative** comparison between the plant clock and wall clock,
   both measured from the start of the loop — not a per-tick one.
2. If the plant clock is **at or ahead of** wall clock, sleep whatever is
   left of this tick's budget after the work.
3. If the plant clock is **behind** wall clock, sleep only a cursory amount
   ([`CATCH_UP_SLEEP`]) and run the next tick straight away, so the loop
   works off the deficit.

The cumulative comparison is the part worth keeping. A purely per-tick
deadline loses every millisecond an overrunning tick costs and never gets it
back; comparing totals means a slow patch is repaid by the ticks that follow
it, and the loop converges back on 1:1 instead of drifting.

The one deliberate change from the original is the cursory sleep. The
original does not sleep at all on the behind-real-time branch, which spins a
physics thread flat out against the GUI thread; `fhr_sim_v2` already uses a
5 microsecond token sleep on its fast-forward branch for exactly this
reason, so that value is reused here.

# The defect this replaces

A loop that advances a fixed slice of plant time and then sleeps a **fixed**
wall period cannot run in real time. Its wall period is `compute + sleep`,
so with `sleep` fixed at the plant slice the achieved ratio is

```text
simulated / (compute + simulated)
```

which is below 1.0 for any nonzero compute cost and can never reach it. No
choice of the sleep constant fixes it, because the compute cost is never
subtracted. That was `htgr_sim_v1`'s physics thread (kopi-beans `op-v5zb`).

# The arithmetic is the bug-prone part

Two mistakes recur in hand-rolled versions of this, both recorded as
kopi-beans `op-xvye`, and both are impossible here by construction (see
[`pace_tick`]):

- **Sign collapse.** Computing the remaining budget as
  `(budget_us - compute_us).round().abs()` turns an *overrun* into a
  positive sleep, so a tick that blew its budget by 50 ms sleeps another
  50 ms on top of it. [`Duration`] subtraction here is checked, and an
  overrun yields [`Duration::ZERO`] plus a reported
  [`TickPacing::overrun`].
- **Unsigned underflow.** Computing `Duration::from_micros(remaining - 1)`
  on a `u64` *before* checking `remaining > 1` wraps to `u64::MAX` when the
  remainder is zero — a ~584 000-year sleep under the mandatory release
  profile, and a panic in debug. Nothing here converts a duration to an
  integer and subtracts from it.

# Over-budget policy: fall behind, work it off, and say so

When the work does not fit in the budget, the plant clock falls behind wall
clock and the loop stops sleeping until it has caught up. Simulated time is
never skipped and the timestep is never grown — the first would fabricate
plant state that was never computed, and the second would change the
integration of a stiffly coupled plant, which pacing has no business doing.
If the compute cost is *persistently* over budget the deficit grows without
bound; [`RealTimePacer::is_behind_real_time`] and
[`RealTimePacer::real_time_deficit`] exist so that shows on screen instead
of being absorbed silently.

It will never run **faster** than real time: once the plant clock is ahead,
the loop sleeps out the rest of the budget.

```rust
pub mod real_time_pacing { /* ... */ }
```

### Types

#### Struct `TickPacing`

What one tick's pacing arithmetic decided.

```rust
pub struct TickPacing {
    pub sleep_for: std::time::Duration,
    pub overrun: std::time::Duration,
    pub ahead_of_real_time: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sleep_for` | `std::time::Duration` | How long to sleep before starting the next tick.<br><br>[`Duration::ZERO`] when the tick's work consumed the whole budget, and<br>[`CATCH_UP_SLEEP`] when the loop is behind real time and working off a<br>deficit. |
| `overrun` | `std::time::Duration` | By how much the tick's work exceeded its budget.<br><br>[`Duration::ZERO`] whenever the work fitted. This is the quantity a<br>sign-collapsing implementation mistakes for extra sleep. |
| `ahead_of_real_time` | `bool` | Whether the plant clock was at or ahead of wall clock at the end of this<br>tick — the condition that decides whether the loop sleeps out its budget<br>or hurries on. |

##### Implementations

###### Methods

- ```rust
  pub fn is_over_budget(self: &Self) -> bool { /* ... */ }
  ```
  Whether this tick's work exceeded its budget.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TickPacing { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TickPacing) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `RealTimePacer`

Paces a fixed-timestep simulation loop against wall clock, and measures how
well it is keeping up.

One instance per simulation thread. The loop shape it expects is:

```no_run
# use std::thread;
# use std::time::{Duration, Instant};
# use uom::si::f64::Time;
# use uom::si::time::second;
# use outram_park_digital_twin_engine::app_scaffold::RealTimePacer;
// 10 ms of plant time per tick, paced to 10 ms of wall clock.
let mut pacer = RealTimePacer::new(Time::new::<second>(0.010), Duration::from_millis(10));
let loop_start = Instant::now();
loop {
    let tick_start = Instant::now();

    // ... advance the plant by one tick's worth of simulated time ...

    let pacing = pacer.pace(tick_start.elapsed(), loop_start.elapsed());
    thread::sleep(pacing.sleep_for);

    if pacer.is_behind_real_time() {
        // publish the shortfall so the GUI can say so
    }
}
```

Both elapsed times are the caller's to measure, which keeps this type free
of a clock and therefore testable with synthetic sequences: `tick_start`
gives the **work**, which the deadline is computed from, and `loop_start`
gives **wall clock since the loop began**, which the cumulative comparison
is made against.

```rust
pub struct RealTimePacer {
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
  pub fn new(simulated_per_tick: Time, budget: Duration) -> Self { /* ... */ }
  ```
  A pacer that advances `simulated_per_tick` of plant time per tick and

- ```rust
  pub fn budget(self: &Self) -> Duration { /* ... */ }
  ```
  The wall-clock budget one tick is given.

- ```rust
  pub fn simulated_per_tick(self: &Self) -> Duration { /* ... */ }
  ```
  The simulated time one tick advances.

- ```rust
  pub fn pace(self: &mut Self, compute: Duration, wall_elapsed: Duration) -> TickPacing { /* ... */ }
  ```
  Advance the plant clock by one tick and decide how long to sleep.

- ```rust
  pub fn simulated_total(self: &Self) -> Duration { /* ... */ }
  ```
  Simulated time advanced since the loop began.

- ```rust
  pub fn real_time_deficit(self: &Self) -> Duration { /* ... */ }
  ```
  How far the plant clock is behind wall clock.

- ```rust
  pub fn measured_real_time_ratio(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Simulated seconds advanced per wall-clock second since the loop began.

- ```rust
  pub fn is_behind_real_time(self: &Self) -> bool { /* ... */ }
  ```
  Whether the plant clock has fallen measurably behind wall clock.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RealTimePacer { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `pace_tick`

Sleep budget for one tick, given the whole-tick wall `budget`, how long the
tick's work took, and whether the plant clock is at or ahead of wall clock.

The whole of the pacing arithmetic, kept as a pure function of two durations
and a flag so it can be exercised with synthetic timings rather than a real
clock.

- Ahead of real time, work inside the budget: sleep the remainder.
- Ahead of real time, work exactly on or past the budget: sleep nothing, and
  report the overrun — never a negative-turned-positive sleep, and never an
  unsigned wrap.
- Behind real time: sleep [`CATCH_UP_SLEEP`] regardless, and press on.

```rust
pub fn pace_tick(budget: std::time::Duration, compute: std::time::Duration, ahead_of_real_time: bool) -> TickPacing { /* ... */ }
```

### Constants and Statics

#### Constant `CATCH_UP_SLEEP`

Token sleep taken on a tick that is already behind real time.

Long enough to yield the CPU so the GUI thread is not starved by a physics
thread spinning to catch up, short enough not to be part of the pacing.
Matches the fast-forward token sleep in `fhr_sim_v2`.

```rust
pub const CATCH_UP_SLEEP: std::time::Duration = _;
```

#### Constant `BEHIND_REAL_TIME_DEFICIT`

How far the plant clock may fall behind wall clock before the loop is
reported as behind real time.

A quarter of a second: past ordinary scheduling jitter, and about the point
at which a person watching a transient would notice the plant lagging their
slider.

```rust
pub const BEHIND_REAL_TIME_DEFICIT: std::time::Duration = _;
```

### Types

#### Struct `SharedState`

A physics/simulation state shared between a rendering thread (the GUI)
and one or more computation threads, matching `fhr_sim_v2`'s
`Arc<Mutex<FHRState>>` pattern but backed by [`RwLock`] instead of
[`std::sync::Mutex`] (see this module's doc for why).

Cheap to [`Clone`] (it is just an `Arc` underneath) -- clone one handle
per thread that needs access, the same way `fhr_sim_v2` clones
`Arc<Mutex<FHRState>>` once per spawned thread.

```rust
pub struct SharedState<T>(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Methods

- ```rust
  pub fn new(initial: T) -> Self { /* ... */ }
  ```
  Wrap `initial` as a new shared state.

- ```rust
  pub fn snapshot(self: &Self) -> T
where
    T: Clone { /* ... */ }
  ```
  A cloned snapshot of the current state, for rendering -- the

- ```rust
  pub fn update</* synthetic */ impl FnOnce(&mut T): FnOnce(&mut T)>(self: &Self, f: impl FnOnce(&mut T)) { /* ... */ }
  ```
  Mutate the state in place via `f`, holding the write lock only for

- ```rust
  pub fn read_with<R, /* synthetic */ impl FnOnce(&T) -> R: FnOnce(&T) -> R>(self: &Self, f: impl FnOnce(&T) -> R) -> R { /* ... */ }
  ```
  Read the state via a closure without cloning it -- for callers who

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Self { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Traits

#### Trait `PanelSet`

A set of selectable top-level GUI panels/pages, implemented by the
calling application's own enum (matching `fhr_sim_v2`'s `Panel` enum:
`MainPage`/`ReactorPowerGraphs`/`PoisonGraphs`).

```rust
pub trait PanelSet: Copy + PartialEq + ''static {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Associated Constants

- `ALL`: All selectable panels, in the order they should appear as tabs.

###### Required Methods

- `label`: Human-readable label for this panel's tab/button.

### Functions

#### Function `spawn_physics_thread`

Spawn a physics-computation thread that repeatedly calls `step` against
a cloned handle to `state`, looping indefinitely -- matching
`fhr_sim_v2`'s `thread::spawn(move || { FHRSimulatorApp::calculate_..._loop(state_ptr) })`
pattern, one call site per physics subsystem (that function's own
internal `loop { ... }` is `step`'s job here, called once per
iteration by this wrapper instead of written out at each call site).

`step` is handed a fresh clone of `state` each call, so it can read/write
the shared state itself; the *loop* (and thus the timestep cadence,
sleep/backoff, etc.) is entirely `step`'s responsibility.

```rust
pub fn spawn_physics_thread<T, F>(state: SharedState<T>, step: F) -> std::thread::JoinHandle<()>
where
    T: Send + Sync + ''static,
    F: FnMut(&SharedState<T>) + Send + ''static { /* ... */ }
```

#### Function `panel_selector_ui`

Render a horizontal row of selectable buttons, one per `P::ALL` panel,
updating `current` when the user picks a different one -- the reusable
half of `fhr_sim_v2`'s `ui.horizontal(|ui| { ui.selectable_value(&mut
self.open_panel, Panel::X, "X"); ... })` top-bar row.

```rust
pub fn panel_selector_ui<P: PanelSet>(ui: &mut egui::Ui, current: &mut P) { /* ... */ }
```

### Re-exports

#### Re-export `mark_component`

```rust
pub use crash::mark_component;
```

#### Re-export `show_crash_modal_if_crashed`

```rust
pub use crash::show_crash_modal_if_crashed;
```

#### Re-export `show_crash_modal_with_restart`

```rust
pub use crash::show_crash_modal_with_restart;
```

#### Re-export `spawn_monitored`

```rust
pub use crash::spawn_monitored;
```

#### Re-export `spawn_physics_thread_monitored`

```rust
pub use crash::spawn_physics_thread_monitored;
```

#### Re-export `CrashModalOutcome`

```rust
pub use crash::CrashModalOutcome;
```

#### Re-export `CrashReport`

```rust
pub use crash::CrashReport;
```

#### Re-export `ThreadHealth`

```rust
pub use crash::ThreadHealth;
```

#### Re-export `GuiFrameMetrics`

```rust
pub use gui_frame_metrics::GuiFrameMetrics;
```

#### Re-export `pace_tick`

```rust
pub use real_time_pacing::pace_tick;
```

#### Re-export `RealTimePacer`

```rust
pub use real_time_pacing::RealTimePacer;
```

#### Re-export `TickPacing`

```rust
pub use real_time_pacing::TickPacing;
```

## Module `color_maps`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-park-digital-twin-engine/src/lib.rs:94:11: 94:32 (#0) }, crates/outram-park-digital-twin-engine/src/lib.rs:94:10: 94:33 (#0))])]")`

Colour-map functions for physics-state-driven rendering.

Ported verbatim from the duplicated `hot_to_cold_colour`/
`steam_quality_colour` functions in `tampines-steam-tables`'s
`fhr_sim_v1`/`fhr_sim_v2` examples and `tuas_boussinesq_solver`'s
`ciet_educational_simulator/ciet_simulator_v1` example (confirmed
byte-identical between `fhr_sim_v1`/`v2`) -- real, already-validated code,
not new physics or a stub. Each function takes a plain `f32` already
computed by the caller (e.g. a normalised temperature or quality), the
same signature the existing call sites use.

```rust
pub mod color_maps { /* ... */ }
```

### Modules

## Module `crameri`

Fabio Crameri's Scientific colour maps (MIT). Perceptually uniform,
CVD-friendly and greyscale-safe -- see the module docs for provenance and
for when to prefer them over the maps in this file.
Fabio Crameri's **Scientific colour maps** — perceptually uniform,
colour-vision-deficiency friendly, greyscale-safe.

# Provenance

| | |
|---|---|
| Project | Scientific colour maps |
| Author | Fabio Crameri |
| Version | 8.0.1 |
| DOI | [10.5281/zenodo.1243862](https://doi.org/10.5281/zenodo.1243862) |
| Home | <https://www.fabiocrameri.ch/colourmaps/> |
| Licence | **MIT**, Copyright (c) 2023, Fabio Crameri |
| Retrieved | 2026-08-06, from the official `ScientificColourMaps8.zip` |

Cite as: Crameri, F. (2018). *Scientific colour maps.* Zenodo.
<https://doi.org/10.5281/zenodo.1243862>

MIT is compatible with this workspace's `GPL-3.0-only`. The full licence
text ships as `LICENSE.crameri.pdf` at the crate root, per the MIT
requirement that the copyright and permission notice accompany the work.

The tables were transcribed from the **official release**, not from any
third-party wrapper. Two wrappers were considered and rejected on licence
grounds: NASA GISS Panoply's colorbar collection carries no licence at all
and aggregates third-party tables, and `github.com/chadagreene/crameri`
(a MATLAB wrapper) declares no licence, so its code is all-rights-reserved
regardless of the data being MIT underneath.

# Why these rather than the existing maps

[`crate::color_maps::hot_to_cold_colour_mark_1`] and friends remain, and
callers depending on their exact values are unaffected — this module is an
addition, not a replacement.

The difference that matters is **perceptual uniformity**: equal steps in the
underlying quantity produce equal-looking steps in colour. A map without
that property manufactures structure — stretches where the colour changes
quickly read as steep gradients and flat stretches read as uniform, whether
or not the data does anything of the sort. For widgets whose whole premise
is that the rendering derives from physics state, a colour map that invents
features is actively misleading.

# Choosing a map

- **Diverging** ([`vik`], [`roma`]) — a quantity with a meaningful midpoint,
  where deviation either way matters: temperature about a reference,
  a residual, an error.
- **Sequential** ([`batlow`], [`lajolla`]) — a quantity read as magnitude
  with no special centre: burnup, flux, steam quality.
- **Cyclic** ([`roma_o`]) — a quantity that wraps, so the ends must join
  without a visible seam: a rotor angle, a phase.

```rust
pub mod crameri { /* ... */ }
```

### Modules

## Module `tables`

Colour-table data for [`super`], generated from the official
Scientific colour maps release.

DO NOT HAND-EDIT. These are Fabio Crameri's published values, transcribed
from the ASCII `.txt` tables in `ScientificColourMaps8.zip`
(v8.0.1, DOI 10.5281/zenodo.1243862). Editing them would destroy the
perceptual uniformity that is the entire reason for using these maps.

Stored as 8-bit sRGB triplets rather than the source floats because
`egui::Color32` is 8 bits per channel, so this is the value that would be
produced anyway -- the quantisation happens at the display boundary either
way, and storing it directly keeps the tables compact.

Licence: MIT, Copyright (c) 2023, Fabio Crameri. See `LICENSE.crameri` at
the crate root.

```rust
pub mod tables { /* ... */ }
```

### Constants and Statics

#### Constant `VIK`

`vik` -- diverging. Blue-white-red. The temperature map: symmetric about its midpoint, so a mid-range value sits at the neutral centre and equal excursions either way read as equally strong.

```rust
pub const VIK: [[u8; 3]; 256] = _;
```

#### Constant `ROMA`

`roma` -- diverging. Red-yellow-blue diverging alternative to vik, for when a second distinguishable diverging field is on screen at the same time.

```rust
pub const ROMA: [[u8; 3]; 256] = _;
```

#### Constant `BATLOW`

`batlow` -- sequential. The general-purpose sequential map: monotonic in lightness, so it survives being printed in greyscale.

```rust
pub const BATLOW: [[u8; 3]; 256] = _;
```

#### Constant `LAJOLLA`

`lajolla` -- sequential. Warm sequential, light at the low end. Suits quantities read as intensity (burnup, flux, steam quality).

```rust
pub const LAJOLLA: [[u8; 3]; 256] = _;
```

#### Constant `ROMA_O`

`romaO` -- cyclic. Cyclic: first and last entries match, so a quantity that wraps (a rotor angle, a phase) has no false seam at the wrap point.

```rust
pub const ROMA_O: [[u8; 3]; 256] = _;
```

### Functions

#### Function `vik`

`vik` — blue-white-red **diverging**. The default temperature map.

`t = 0.0` is the cold end, `0.5` the neutral centre, `1.0` the hot end.
Normalise a temperature onto `[0, 1]` about its reference before calling,
so that `0.5` lands where the physics says "neither hot nor cold".

```rust
pub fn vik(t: f32) -> egui::Color32 { /* ... */ }
```

#### Function `roma`

`roma` — red-yellow-blue **diverging**. Use when a second, visually
distinguishable diverging field shares the screen with [`vik`].

```rust
pub fn roma(t: f32) -> egui::Color32 { /* ... */ }
```

#### Function `batlow`

`batlow` — general-purpose **sequential**, monotonic in lightness.

The safe default for a magnitude with no meaningful midpoint, and the one
to reach for if the figure may be printed in greyscale.

```rust
pub fn batlow(t: f32) -> egui::Color32 { /* ... */ }
```

#### Function `lajolla`

`lajolla` — warm **sequential**, light at the low end.

```rust
pub fn lajolla(t: f32) -> egui::Color32 { /* ... */ }
```

#### Function `roma_o`

`romaO` — **cyclic**. Wraps, so `t` and `t + 1` are the same colour.

For quantities with no beginning or end: a rotor angle, a phase. A
non-cyclic map used for these shows a hard seam at the wrap, implying a
discontinuity the physics does not have.

```rust
pub fn roma_o(t: f32) -> egui::Color32 { /* ... */ }
```

### Functions

#### Function `hot_to_cold_colour_mark_1`

Hot-to-cold colour map, variant 1: blue (cold) to red (hot), with a
green tint that fades out as `hotness` rises.

`hotness` is clamped to \[0, 1\] before mapping (out-of-range values
saturate at their nearest bound rather than producing an invalid
colour). This is the primary hot/cold colour map used across the
existing FHR/CIET simulator examples (e.g. reactor core/downcomer
temperature-driven fill colours).

```rust
pub fn hot_to_cold_colour_mark_1(hotness: f32) -> egui::Color32 { /* ... */ }
```

#### Function `hot_to_cold_colour_mark_2`

Hot-to-cold colour map, variant 2: black (cold) to red (hot), no green
or blue tint.

`hotness` is clamped to \[0, 1\] before mapping, same convention as
[`hot_to_cold_colour_mark_1`]. Used for a subset of pipe segments in the
existing FHR simulator examples where the simpler black-to-red map was
preferred over `mark_1`.

```rust
pub fn hot_to_cold_colour_mark_2(hotness: f32) -> egui::Color32 { /* ... */ }
```

#### Function `steam_quality_colour_mark_1`

Steam-quality colour map: blue (`quality = 0`, saturated liquid) to
white (`quality = 1`, saturated vapour).

`steam_quality` is clamped to \[0, 1\] before mapping, same convention as
[`hot_to_cold_colour_mark_1`].

```rust
pub fn steam_quality_colour_mark_1(steam_quality: f32) -> egui::Color32 { /* ... */ }
```

## Module `components`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-park-digital-twin-engine/src/lib.rs:96:11: 96:32 (#0) }, crates/outram-park-digital-twin-engine/src/lib.rs:96:10: 96:33 (#0))])]")`

Visual process object wrappers.

One file per visual process object, each composing its
[`tampines::components`] (or, for [`reactor_vessel`], `nee_soon`) physics
counterpart with visual-only fields (screen position/size, min/max
temperature for colour mapping) and a minimal `egui::Widget`
implementation. Deliberately composes rather than duplicates state --
avoid separating physics and rendering unnecessarily.
[`instrumentation`] stays a generic label/value placeholder -- `nee_soon`
does not yet expose a dedicated instrumentation-readout type to wrap.

```rust
pub mod components { /* ... */ }
```

### Modules

## Module `bend`

Smooth bend between two pipe runs.

# The geometry

Two rectangular runs meeting at an angle leave a wedge-shaped gap on the
outside of the turn and overlap on the inside. Drawing them butted together
is what makes elbows look wrong. The fix is to make the joint an explicit
piece of geometry:

- the two runs' **inner** corners are coincident, at a single point `P`;
- their **outer** corners sit one pipe-thickness from `P`, along each run's
  outward normal;
- the gap between those outer corners is closed by a **circular arc**
  centred on `P`, radius equal to the pipe thickness.

The filled region is therefore a circular sector — a quarter circle for a
90-degree bend, narrower or wider as the turn angle changes. Both outer
corners are exactly one thickness from `P` by construction, so the arc
meets each run's outer edge tangentially and the silhouette is continuous.

```text
       run A
   ┌──────────────┐
   │              │ P  <- inner corners coincide here
   └──────────┐   ●
              │ ╲ │      the sector is centred on P,
   outer arc  │  ╲│      radius = pipe thickness
              ╰───┤
                  │ run B
```

# Why it is coloured the way it is

The bend is a control volume shared between two runs, so it is filled with
the **mean** of the two adjacent cell temperatures rather than either one.
Taking one side's value would draw a discontinuity at a joint where the
physics has none, and would flip which side looked hotter depending on
which run happened to be drawn first.

```rust
pub mod bend { /* ... */ }
```

### Types

#### Enum `TurnSense`

Which way the joint turns, seen on screen.

Needed because the turn sense cannot always be inferred. At exactly 180
degrees the two directions are antiparallel and the cross product vanishes,
so a U-bend is genuinely ambiguous — it may belly to either side, and both
are correct pipework. Inferring would make the sector flip sides the instant
the angle reached 180.

```rust
pub enum TurnSense {
    Auto,
    Clockwise,
    Anticlockwise,
}
```

##### Variants

###### `Auto`

Infer from the cross product of the two directions. Correct for every
angle strictly between 0 and 180.

###### `Clockwise`

Turn clockwise on screen (screen y grows downward, so this is
"downward" for a run heading right).

###### `Anticlockwise`

Turn anticlockwise on screen.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TurnSense { /* ... */ }
    ```

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
    fn default() -> TurnSense { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TurnSense) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `PipeBendVisual`

A smooth bend joining two pipe runs.

Construct it from the joint's inner corner and the two runs' directions;
see the module docs for the construction. The directions are the flow
directions of each run — `in_direction` points *towards* the joint,
`out_direction` points *away* from it.

```rust
pub struct PipeBendVisual {
    pub inner_corner: egui::Pos2,
    pub in_direction: egui::Vec2,
    pub out_direction: egui::Vec2,
    pub thickness: f32,
    pub upstream_temperature: uom::si::f64::ThermodynamicTemperature,
    pub downstream_temperature: uom::si::f64::ThermodynamicTemperature,
    pub min_temp: uom::si::f64::ThermodynamicTemperature,
    pub max_temp: uom::si::f64::ThermodynamicTemperature,
    pub shade: crate::components::PipePhaseShade,
    pub turn_sense: TurnSense,
    pub sweep_override: Option<f32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inner_corner` | `egui::Pos2` | The coincident inner corner of the two runs. |
| `in_direction` | `egui::Vec2` | Flow direction of the incoming run, pointing towards the joint. |
| `out_direction` | `egui::Vec2` | Flow direction of the outgoing run, pointing away from the joint. |
| `thickness` | `f32` | Pipe thickness in points, which is also the sector's radius. |
| `upstream_temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature of the last cell of the incoming run. |
| `downstream_temperature` | `uom::si::f64::ThermodynamicTemperature` | Temperature of the first cell of the outgoing run. |
| `min_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature drawn in the coldest displayable colour. |
| `max_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature drawn in the hottest displayable colour. |
| `shade` | `crate::components::PipePhaseShade` | Phase shading, matching the runs it joins. |
| `turn_sense` | `TurnSense` | Which way the joint turns. See [`TurnSense`] — set this explicitly for<br>a 180-degree return bend, where it cannot be inferred. |
| `sweep_override` | `Option<f32>` | Explicit signed sweep in radians, positive clockwise on screen.<br><br>`None` infers the sweep from the two directions, which is correct for<br>any turn up to half a circle. Beyond that the inference breaks down:<br>the angle between two vectors is only ever `[0, pi]`, so a 270-degree<br>turn is indistinguishable from a 90-degree one and would draw as the<br>wrong sector. State the sweep whenever the joint may exceed 180. |

##### Implementations

###### Methods

- ```rust
  pub fn new(inner_corner: Pos2, in_direction: Vec2, out_direction: Vec2, thickness: f32, upstream_temperature: ThermodynamicTemperature, downstream_temperature: ThermodynamicTemperature, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  A bend at `inner_corner`, turning from `in_direction` to

- ```rust
  pub fn with_sweep(self: Self, sweep: Angle) -> Self { /* ... */ }
  ```
  State the swept angle explicitly, positive clockwise on screen.

- ```rust
  pub fn with_turn_sense(self: Self, turn_sense: TurnSense) -> Self { /* ... */ }
  ```
  State the turn sense explicitly. Builder-style.

- ```rust
  pub fn with_shade(self: Self, shade: PipePhaseShade) -> Self { /* ... */ }
  ```
  Set the phase shading so the bend matches the runs it joins.

- ```rust
  pub fn mean_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Mean of the two adjacent cell temperatures — what the bend is filled

- ```rust
  pub fn signed_sweep(self: &Self) -> f32 { /* ... */ }
  ```
  The signed angle actually swept, in radians, positive clockwise.

- ```rust
  pub fn turn_angle(self: &Self) -> f32 { /* ... */ }
  ```
  The angle between the two runs, in radians, always in `[0, pi]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Fills the sector with the mean adjacent temperature, then strokes the

- **WithSubscriber**
## Module `condenser`

Schematic surface-condenser art.

A condenser is the cold end of a Rankine cycle, and the only interesting
thing about it is the phase change: turbine exhaust steam arrives at the top
of a shell held well below atmospheric pressure, gives up its latent heat to
cooling water flowing inside a bundle of tubes, and falls as condensate into
a hotwell at the bottom. Two fluid circuits therefore cross in one vessel —
**steam outside the tubes, cooling water inside them** — and a reader has to
be able to see which is which.

So the artwork draws, top to bottom:

- the **exhaust neck** flaring down from the turbine into the shell;
- the **steam space**, tinted by the exhaust steam quality and streaked
  downward, because the steam flows *across and down* over the bundle;
- the **tube bundle**, whose rows are graded along the cooling-water path so
  the water visibly heats up as it crosses;
- the **tubesheets and waterboxes** at each end, with the cooling-water
  nozzles arranged according to [`CondenserKind`];
- the **air offtake** at the cold end of the bundle, because a condenser
  only holds its vacuum if the non-condensables are continuously pulled out;
- **condensate raining** off the bundle into the **hotwell**, drawn as a
  pool with a level.

# Two colour axes, deliberately

Steam-side regions are tinted by **quality** through
[`crate::color_maps::steam_quality_colour_mark_1`] (blue at `x = 0`,
saturated liquid; white at `x = 1`, dry vapour), because in a condenser the
quality genuinely traverses that whole range from the exhaust neck to the
hotwell — no display remap is needed or applied. Everything that carries a
**temperature** — the tubes, the waterboxes, the condensate drops and the
pool — is coloured by the shared
[`crate::components::temperature_colour`] map, so a condenser grades
temperature identically to every other widget in this library. The same two
axes already coexist in [`crate::components::TurbineVisual`].

One consequence worth watching for: the drops falling off the bundle are
drawn at the **condensing** temperature and the pool at the **condensate**
temperature, so a subcooled hotwell reads as a visibly colder pool than the
rain landing in it.

# Dispatch

[`CondenserKind`] and [`CondenserVisualState`] are enums, not trait objects,
per the workspace's mandatory "no trait objects" Rust design rule: both sets
are closed and known at compile time, so adding a member is a variant and
the compiler then points at every match that needs handling.

# What is drawn from real state, and what is left neutral

[`tampines::components::Condenser`] stores an **operating pressure** and a
**target outlet quality** — a set-point — and nothing else. It has no fluid
state, no cooling-water side and no hotwell level, and its `condense`
returns `TampinesError::NotYetImplemented`. So the physics-backed path
([`CondenserVisual::new`], whose signature is preserved) draws the complete
machine in neutral metal tones with **no** temperature or quality colour at
all, and labels only the operating pressure it really holds. The target
outlet quality is readable through
[`CondenserVisual::target_outlet_quality`] but is deliberately never
painted: it is what the condenser is *asked* to achieve, not what the fluid
*is*, and colouring the condensate by it would be fabricating state.

The state-driven path is [`CondenserVisual::from_scalars`], the same
contract as [`crate::components::PipeVisual::from_scalars`]: the caller
passes **real state from its own model** — exhaust quality, condensing and
condensate temperatures, cooling-water inlet and outlet temperatures, and
optionally the hotwell level. That is a narrower interface, not a fabricated
one, and it is not a stub. Anything the caller cannot supply honestly stays
neutral grey, which is visibly distinct from every point on the colour scale
— the hotwell level in particular is an [`Option`], and a `None` level draws
a hatched, empty hotwell rather than a plausible half-full one.

# What this is not

**Offline demonstration artwork, not a validated model and not a design
drawing.** Unlike [`crate::components::steam_generator`], whose envelopes
come from published vessel dimensions, *every* proportion here — including
[`SURFACE_CONDENSER_ASPECT_RATIO`] — is chosen by eye for legibility on
screen and is dimensioned from no design whatsoever. Nothing in this module
may be cited or re-used as condenser design data. Per `RESPONSIBLE_USE.md`
this is for education, research and V&V only — not for facility operation,
reactor control, or safety-critical decisions.

```rust
pub mod condenser { /* ... */ }
```

### Types

#### Enum `CondenserKind`

Which cooling-water arrangement to draw.

Both variants are the same machine — a shell full of steam with water inside
the tubes — differing only in how the water is routed through the bundle,
which is what the waterboxes at each end are for.

```rust
pub enum CondenserKind {
    TwoPass,
    SinglePass,
}
```

##### Variants

###### `TwoPass`

Two-pass surface condenser: cooling water enters and leaves at the
**same end**, through a waterbox divided by a pass partition. It crosses
the lower half of the bundle, turns in the plain waterbox at the far
end, and returns through the upper half.

The usual arrangement when the cooling water comes back to the same
place it left from — a closed circuit through a cooling tower — because
both large-bore circulating-water lines then land on one end of the
condenser.

###### `SinglePass`

Single-pass surface condenser: cooling water enters one end and leaves
the other, crossing the bundle once.

Typical of once-through cooling, where the supply and the return go to
different places anyway, and of very large units where the pressure drop
of a second pass is not wanted.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short display name, for a picker or a card caption.

- ```rust
  pub fn description(self: Self) -> &'static str { /* ... */ }
  ```
  Where this arrangement is normally used, in words.

- ```rust
  pub fn cooling_water_path(self: Self) -> &'static str { /* ... */ }
  ```
  How the cooling water crosses the bundle — the one fact that explains

- ```rust
  pub fn passes(self: Self) -> u8 { /* ... */ }
  ```
  How many times the cooling water crosses the bundle: 2 or 1.

- ```rust
  pub fn native_aspect_ratio(self: Self) -> f32 { /* ... */ }
  ```
  Width-to-height ratio the artwork is drawn at, dimensionless.

- ```rust
  pub fn fit_native_aspect(self: Self, available: Rect) -> Rect { /* ... */ }
  ```
  The largest sub-rectangle of `available` carrying this kind's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CondenserKind { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CondenserKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `CondenserDisplayRange`

The temperature range the artwork's colours are graded against.

Both bounds are absolute thermodynamic temperatures (`uom`-typed, so the
compiler enforces the unit; kelvin internally, conventionally quoted in
degrees Celsius). The shared map is **diverging** — blue at `min_temp`,
neutral white at the *midpoint*, red at `max_temp` — so set the range about
a reference that matters rather than clamping it to the extremes seen.

A condenser is the coldest equipment on a plant schematic, so a range picked
for the whole plant will usually draw the entire condenser blue. That is
correct: it *is* the cold end.

```rust
pub struct CondenserDisplayRange {
    pub min_temp: uom::si::f64::ThermodynamicTemperature,
    pub max_temp: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `min_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature drawn in the coldest displayable colour (blue). |
| `max_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature drawn in the hottest displayable colour (red). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CondenserDisplayRange { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CondenserDisplayRange) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `CondenserScalars`

Scalar state of a condenser, as the caller's own model holds it.

Every field is **real state the caller already has**, not a placeholder —
see the module documentation and
[`crate::components::PipeVisual::from_scalars`] for why this narrower
interface exists. Nothing here is invented by the widget.

```rust
pub struct CondenserScalars {
    pub exhaust_quality: f64,
    pub condensing_temp: uom::si::f64::ThermodynamicTemperature,
    pub condensate_temp: uom::si::f64::ThermodynamicTemperature,
    pub cooling_water_inlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub cooling_water_outlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub hotwell_level_frac: Option<f32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `exhaust_quality` | `f64` | Steam quality of the turbine exhaust entering the shell, dimensionless<br>in `[0, 1]` (`0.0` saturated liquid, `1.0` dry saturated vapour).<br><br>Tints the exhaust neck and the steam space. Values outside `[0, 1]`<br>saturate at the ends of [`steam_quality_colour_mark_1`] rather than<br>producing an invalid colour. |
| `condensing_temp` | `uom::si::f64::ThermodynamicTemperature` | Shell-side condensing temperature — the saturation temperature at the<br>condenser's operating pressure.<br><br>Colours the condensate raining off the bundle and the hotwell surface. |
| `condensate_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature of the condensate leaving the hotwell.<br><br>Equal to [`Self::condensing_temp`] for a saturated hotwell, and lower<br>when the condensate is subcooled — which is drawn, as a pool visibly<br>colder than the rain falling into it. |
| `cooling_water_inlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Cooling water entering the inlet waterbox.<br><br>Colours the inlet waterbox, the inlet nozzle, and the start of the tube<br>gradient. |
| `cooling_water_outlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Cooling water leaving the outlet waterbox.<br><br>Colours the outlet waterbox, the outlet nozzle, and the end of the tube<br>gradient. Above [`Self::cooling_water_inlet_temp`] whenever the<br>condenser is doing anything. |
| `hotwell_level_frac` | `Option<f32>` | Hotwell level, dimensionless in `[0, 1]`, or `None` when the caller's<br>model does not have one.<br><br>`None` draws a hatched, empty hotwell — see [`drawn_hotwell_level`]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CondenserScalars { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CondenserScalars) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `CondenserVisualState`

Where a [`CondenserVisual`] gets the state it renders.

Enum dispatch, not a trait object, per the workspace's mandatory "no trait
objects" Rust design rule.

```rust
pub enum CondenserVisualState {
    Physics(tampines::components::Condenser),
    Scalars(CondenserScalars, CondenserDisplayRange),
}
```

##### Variants

###### `Physics`

Backed by a [`tampines::components::Condenser`] alone.

That component holds an operating pressure and a **target** outlet
quality and nothing else — no fluid state, no cooling-water side, no
hotwell level — so this path draws the complete machine in neutral metal
tones and paints no temperature or quality anywhere. See the module
documentation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines::components::Condenser` |  |

###### `Scalars`

Backed by caller-supplied scalars from the caller's own plant model,
graded against the accompanying [`CondenserDisplayRange`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `CondenserScalars` |  |
| 1 | `CondenserDisplayRange` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CondenserVisualState { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CondenserVisualState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `CondenserVisual`

Visual representation of a surface condenser, in one of two cooling-water
arrangements.

Built either from a [`tampines::components::Condenser`] ([`Self::new`],
which draws the machine neutrally because that component holds no fluid
state) or from the caller's own scalar plant state ([`Self::from_scalars`]).
See the module documentation for what each path is allowed to paint.

The artwork letterboxes to [`CondenserKind::native_aspect_ratio`] inside the
box it is given, so it never stretches.

```rust
pub struct CondenserVisual {
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
  pub fn new(physics: Condenser, screen_position: Pos2, screen_vector: Vec2) -> Self { /* ... */ }
  ```
  Wrap a [`Condenser`] with the given screen geometry.

- ```rust
  pub fn from_scalars(kind: CondenserKind, screen_position: Pos2, screen_vector: Vec2, range: CondenserDisplayRange, scalars: CondenserScalars) -> Self { /* ... */ }
  ```
  Build a condenser visual from the caller's own scalar plant state.

- ```rust
  pub fn with_kind(self: Self, kind: CondenserKind) -> Self { /* ... */ }
  ```
  Draw a different cooling-water arrangement with the same state.

- ```rust
  pub fn with_operating_pressure(self: Self, pressure: Pressure) -> Self { /* ... */ }
  ```
  Label the shell with a known operating (shell-side) pressure.

- ```rust
  pub fn without_labels(self: Self) -> Self { /* ... */ }
  ```
  Turn the internal component labels off — for thumbnails.

- ```rust
  pub fn kind(self: &Self) -> CondenserKind { /* ... */ }
  ```
  Which cooling-water arrangement this visual draws.

- ```rust
  pub fn size(self: &Self) -> Vec2 { /* ... */ }
  ```
  On-screen size of the box the artwork letterboxes into, in points.

- ```rust
  pub fn state(self: &Self) -> &CondenserVisualState { /* ... */ }
  ```
  Where this visual gets its state.

- ```rust
  pub fn scalars(self: &Self) -> Option<CondenserScalars> { /* ... */ }
  ```
  The scalar state the artwork is drawn from, or `None` on the

- ```rust
  pub fn operating_pressure(self: &Self) -> Option<Pressure> { /* ... */ }
  ```
  The shell-side operating pressure, if one is known.

- ```rust
  pub fn target_outlet_quality(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The wrapped component's **target** outlet quality, dimensionless in

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws the condenser for [`CondenserVisual::kind`]: shell, exhaust neck,

- **WithSubscriber**
### Functions

#### Function `cooling_water_path_span`

How far along the cooling-water path the two ends of drawn tube row `row`
sit, as `(left_end, right_end)` fractions in `[0, 1]`.

`0.0` is the cooling-water inlet nozzle and `1.0` the outlet nozzle, so
colouring each row between these two fractions makes the water visibly heat
up as it crosses the bundle — including *back* along the return pass, where
the hottest water is at the left, next to its own inlet.

Rows are indexed **top to bottom**, `row` in `0..rows`.

- [`CondenserKind::SinglePass`]: every row spans `(0.0, 1.0)`.
- [`CondenserKind::TwoPass`]: the water enters the **lower** half at the
  left, so those rows span `(0.0, 0.5)`; it turns at the far waterbox and
  returns along the **upper** half to an outlet beside its own inlet, so
  those rows span `(1.0, 0.5)` — left end hottest.

A bundle drawn with fewer than two rows cannot show two passes, so a
[`CondenserKind::TwoPass`] bundle degenerates to the single-pass span rather
than drawing only half the circuit.

```rust
pub fn cooling_water_path_span(kind: CondenserKind, row: usize, rows: usize) -> (f32, f32) { /* ... */ }
```

#### Function `drawn_hotwell_level`

The hotwell level actually drawn, dimensionless in `[0, 1]`.

`0.0` is an empty hotwell and `1.0` a full one. Returns `None` when the
caller supplied no level, which is drawn as a hatched empty sump — a
condenser hotwell level comes from a level transmitter and a
condensate-pump control loop, and a model that does not have one must not be
drawn as though it did.

Out-of-range values are clamped rather than rejected, because a level comes
from a controller that can transiently overshoot and artwork must not panic
on it. A **non-finite** level draws as empty (`0.0`) — deliberately the most
visible outcome, so a NaN in the caller's model shows up on screen instead
of hiding behind a plausible half-full hotwell. Same contract as
[`crate::components::steam_generator::drawn_water_level`].

```rust
pub fn drawn_hotwell_level(frac: Option<f32>) -> Option<f32> { /* ... */ }
```

### Constants and Statics

#### Constant `SURFACE_CONDENSER_ASPECT_RATIO`

Width-to-height ratio the condenser artwork is drawn at, dimensionless.

**Chosen by eye, not taken from a design.** A surface condenser is a broad,
low box rather than a slender vessel — the tubes have to be long enough to
pick up the duty and there have to be a great many of them, so the shell
grows sideways — and 1.55 : 1 is enough to read that way while still leaving
room above the bundle for a steam space and below it for a hotwell. See the
module's "What this is not" section.

```rust
pub const SURFACE_CONDENSER_ASPECT_RATIO: f32 = 1.55;
```

## Module `control_rod_drive`

`egui`-side plumbing for the control-rod drive animation.

The kinematics live in [`crate::animation::control_rod_drive`] and are
deliberately `egui`-free. This module is the thin layer that keeps a
[`ControlRodDrive`] alive **across repaints** and advances it once per frame,
so a vessel widget can be handed a *drawn* insertion fraction that travels
toward the commanded one instead of jumping to it.

# Where the state lives, and why it is here

Visual components are `egui::Widget`s consumed by value and rebuilt every
repaint (see the crate `CLAUDE.md`), so animation state inside a widget would
reset every frame and never move. The established pattern for
[`crate::animation::TracerTrain`] is that the **application** owns the state
on its `eframe::App` struct and copies it into the widget at build time.

[`slewed_control_rod_insertion`] keeps that ownership rule — the state is
outside the widget and survives repaints — but parks it in `egui`'s own
per-context store, keyed by an [`Id`] the caller chooses, rather than on the
app struct. Two consequences worth knowing:

- **It is still not widget-owned.** The widget receives a plain `f32` and has
  no memory of its own, exactly as before.
- **It costs the caller nothing to adopt.** A simulator that already owns its
  animation state on the app struct can drive [`ControlRodDrive`] directly
  and skip this helper entirely; that remains the more explicit option and is
  preferable when the drive's state needs to be read elsewhere (a status
  readout, a recorded trace, a saved session).

# Repaint

While a rod is travelling the helper calls `Context::request_repaint`, so the
animation runs to completion even in an application that only repaints on
demand. The example simulators repaint continuously anyway; this makes the
helper correct for one that does not.

```rust
pub mod control_rod_drive { /* ... */ }
```

### Functions

#### Function `slewed_control_rod_insertion`

Advance a persisted control-rod drive one frame toward `commanded` and return
where the rod should be **drawn**.

`commanded` and the return value are both dimensionless insertion fractions
in `[0, 1]` — `0.0` fully withdrawn, `1.0` fully inserted. `commanded` is the
simulator's own setpoint (for the HTGR example, `HtgrSnapshot`'s
`control_rod_insertion_fraction`); the return value lags it by the travel the
drive has not yet completed.

`id` names this rod bank's animation state. It must be **stable across
frames** and **distinct per rod bank**, or two banks will fight over one
drive. Deriving it from the vessel widget's own response id is the easiest
way to get both.

`drive_for_first_frame` is used only when there is no stored state yet — on
the very first frame the rod starts wherever that drive says, so a simulator
that begins at 60 % insertion does not animate up from zero on load.

The timestep is `egui`'s smoothed `stable_dt`, clamped to
[`MAX_ANIMATION_TIMESTEP_SECONDS`].

```rust
pub fn slewed_control_rod_insertion(ctx: &egui::Context, id: egui::Id, commanded: f64, drive_for_first_frame: crate::animation::control_rod_drive::ControlRodDrive) -> f32 { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_ANIMATION_TIMESTEP_SECONDS`

Longest animation timestep this helper will take in one frame, in seconds.

`egui`'s frame time can be huge after the window was hidden, minimised, or
the process was suspended; feeding that straight in would let a rod cross its
whole stroke in one frame, which is the teleport the animation exists to
avoid. Clamping to 100 ms means the worst a stall can do is advance the rod
by a tenth of a second's travel and then carry on smoothly.

```rust
pub const MAX_ANIMATION_TIMESTEP_SECONDS: f64 = 0.1;
```

## Module `cooling_tower`

Schematic cooling-tower art, one architecture per draught type.

A cooling tower is a **psychrometric** machine, not a heat exchanger with a
wall in it: warm circulating water is broken up over a fill pack and put in
direct contact with air, and most of the cooling comes from evaporating a
little of that water into the air stream. Two consequences set everything
this widget draws.

- **The wet-bulb temperature governs the machine.** Evaporation can cool the
  water towards the air's wet-bulb temperature but never below it, so the
  number that says how good a tower is doing is the **approach** —
  `T_water,out - T_wb` — not the dry-bulb temperature. See
  [`approach_to_wet_bulb`]. The other headline number is the **range**,
  `T_water,in - T_water,out`, which is set by the heat load rather than by
  the tower ([`cooling_range`]).
- **The plume is condensed water, not steam.** Air leaving the fill is at or
  near saturation; when it mixes with cooler ambient air some of the water
  it carries condenses into visible droplets. So how visible the plume is
  depends on how far the exit air sits into saturation — which is why
  [`plume_opacity`] is driven by the exit air's *relative humidity* and by
  nothing else.

# Where the psychrometrics come from

Both air states are [`tampines::humid_air::HumidAirState`], which is a
`uom`-typed wrapper over
`outram_park_fork_coolprop::humid_air::ha_props` — this workspace's
`HAPropsSI` port, humid air as a real-gas mixture per ASHRAE RP-1485. The
widget therefore reads **real psychrometric properties** (dry-bulb,
pressure, humidity ratio, relative humidity, enthalpy, specific volume) that
the caller resolved through that backend, rather than anything invented
here.

**The wet-bulb temperature is the one exception, and it is supplied by the
caller.** The CoolProp backend can produce it
(`HumidAirParam::TWetBulb`), but [`tampines::humid_air::HumidAirState`] has
no field for it and `tampines` does not re-export the raw `ha_props` entry
point, so there is nothing on the state object to read. Solving for it here
would be new physics inside a presentation crate, which this crate's
`CLAUDE.md` forbids — the fix belongs in `tampines`, not in this file. Until
then [`CoolingTowerScalars::inlet_wet_bulb`] is a caller-supplied scalar,
exactly like every other quantity on the
[`crate::components::PipeVisual::from_scalars`] path: real state from the
caller's own model, not a placeholder.

# What the physics component can and cannot supply

[`tampines::components::CoolingTower`] holds a **real** ambient air inlet
state, a real circulating-water inlet temperature and flow rate, and a
**target** approach — a set-point. Its `evaluate` returns
`TampinesError::NotYetImplemented`, so there is no water outlet temperature
and **no exit air state at all**.

[`CoolingTowerVisual::new`] therefore draws the air inlet and the hot water
it really knows about, and leaves everything downstream of the fill
deliberately blank: the basin is neutral grey, no approach is reported (only
the target, labelled as a target), and **no plume is drawn**. An invented
ambient condition or an assumed saturated exit would make a plume appear out
of nothing, which is exactly the failure this crate refuses. For a fully
painted tower, pass the state you actually have to
[`CoolingTowerVisual::from_scalars`].

# Dispatch

[`CoolingTowerKind`] and [`CoolingTowerVisualState`] are enums, not trait
objects, per the workspace's mandatory "no trait objects" Rust design rule.

# Simulation time is application-owned

An induced-draught tower has a fan, and it is drawn at
`theta = omega * t` from a caller-supplied shaft speed and the
**application's** simulation clock — the same contract as
[`crate::components::PumpVisual`] and
[`crate::components::TurbineVisual`], and for the same reason: widgets are
rebuilt every repaint, so a clock owned by the widget would reset to zero
each frame. `CoolingTower` carries no fan speed, so the default is zero and
the fan is drawn **stationary but complete** rather than at a fabricated
speed.

# What this is not

**Offline demonstration artwork, not a validated model and not a design
drawing.** The hyperboloid shell really is drawn from the hyperbola that
defines a hyperboloid of one sheet ([`hyperboloid_half_width`]), but its
proportions — and every other proportion in this module — are chosen by eye
for legibility and are dimensioned from no design. Nothing here may be cited
or re-used as cooling-tower design data. Per `RESPONSIBLE_USE.md` this is
for education, research and V&V only.

```rust
pub mod cooling_tower { /* ... */ }
```

### Types

#### Enum `CoolingTowerKind`

Which cooling-tower architecture to draw.

The two differ in **how the draught is produced**, which is why one is a
150-metre concrete chimney and the other a box with a fan on it. What
happens inside — hot water sprayed over a fill pack, air passing through it,
cooled water collected in a basin — is the same in both, and is drawn by the
same code.

```rust
pub enum CoolingTowerKind {
    NaturalDraught,
    InducedDraught,
}
```

##### Variants

###### `NaturalDraught`

Natural-draught hyperbolic tower: no fan at all.

The buoyancy of the warm, moist air inside the shell drives the flow, so
the shell has to be tall. Its hyperboloid form is structural — a
hyperboloid of one sheet is a doubly-ruled surface, so a thin shell can
be built from straight reinforcement and still resist wind. Drawn from
the meridian hyperbola in [`hyperboloid_half_width`].

###### `InducedDraught`

Mechanical induced-draught cell: a fan on the roof pulls air up through
the fill.

"Induced" means the fan is downstream of the fill and works in the warm
wet air leaving it, which is why the fan sits above the drift
eliminators in the drawing.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short display name, for a picker or a card caption.

- ```rust
  pub fn draught(self: Self) -> &'static str { /* ... */ }
  ```
  What drives the air through the fill — the one fact that explains every

- ```rust
  pub fn description(self: Self) -> &'static str { /* ... */ }
  ```
  Where this architecture is normally used, in words.

- ```rust
  pub fn has_fan(self: Self) -> bool { /* ... */ }
  ```
  Whether this architecture has a fan to draw.

- ```rust
  pub fn native_aspect_ratio(self: Self) -> f32 { /* ... */ }
  ```
  Width-to-height ratio the artwork is drawn at, dimensionless.

- ```rust
  pub fn fit_native_aspect(self: Self, available: Rect) -> Rect { /* ... */ }
  ```
  The largest sub-rectangle of `available` carrying this kind's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoolingTowerKind { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoolingTowerKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `CoolingTowerScalars`

Scalar state of a cooling tower, as the caller's own model holds it.

Every field is **real state the caller already has** — see the module
documentation and [`crate::components::PipeVisual::from_scalars`] for why
this narrower interface exists. Nothing here is invented by the widget.

The two air states are full [`tampines::humid_air::HumidAirState`] values,
so the caller resolves them through the CoolProp-backed psychrometrics
(`tampines::humid_air::state_from_t_p_r` and friends) and this widget only
reads properties off them.

```rust
pub struct CoolingTowerScalars {
    pub air_inlet: tampines::humid_air::HumidAirState,
    pub air_outlet: tampines::humid_air::HumidAirState,
    pub inlet_wet_bulb: uom::si::f64::ThermodynamicTemperature,
    pub water_inlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub water_outlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub water_flow_rate: uom::si::f64::VolumeRate,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `air_inlet` | `tampines::humid_air::HumidAirState` | Ambient air drawn in at the base.<br><br>Its dry-bulb temperature colours the inlet-air arrows and its relative<br>humidity is reported next to them. |
| `air_outlet` | `tampines::humid_air::HumidAirState` | Air leaving the tower, past the drift eliminators.<br><br>Its dry-bulb temperature colours the exit-air arrows and its **relative<br>humidity drives the plume** through [`plume_opacity`]. |
| `inlet_wet_bulb` | `uom::si::f64::ThermodynamicTemperature` | Wet-bulb temperature of the entering air.<br><br>Supplied by the caller because [`HumidAirState`] carries no wet-bulb<br>field — see the module documentation. Sets the approach through<br>[`approach_to_wet_bulb`], and is drawn as a marker on the water-side<br>scale that the cold water can approach but not cross. |
| `water_inlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Warm water returning from the plant to the distribution deck. |
| `water_outlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Cooled water leaving the basin. |
| `water_flow_rate` | `uom::si::f64::VolumeRate` | Circulating-water volumetric flow rate.<br><br>A **zero** flow draws no spray and no rain: a tower with nothing<br>circulating through it is not cooling anything, and drawing water<br>falling through it would be a lie about the plant's state. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoolingTowerScalars { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoolingTowerScalars) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `CoolingTowerVisualState`

Where a [`CoolingTowerVisual`] gets the state it renders.

Enum dispatch, not a trait object, per the workspace's mandatory "no trait
objects" Rust design rule.

```rust
pub enum CoolingTowerVisualState {
    Physics(tampines::components::CoolingTower),
    Scalars(CoolingTowerScalars),
}
```

##### Variants

###### `Physics`

Backed by a [`tampines::components::CoolingTower`].

Its air inlet state, water inlet temperature and water flow rate are
real and are drawn. Its `evaluate` is not implemented, so there is no
water outlet temperature and no exit air state — the basin stays
neutral, no approach is reported, and no plume is drawn. See the module
documentation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines::components::CoolingTower` |  |

###### `Scalars`

Backed by caller-supplied scalars from the caller's own plant model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `CoolingTowerScalars` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoolingTowerVisualState { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoolingTowerVisualState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `CoolingTowerVisual`

Visual representation of a cooling tower, in one of two draught
architectures.

Built either from a [`tampines::components::CoolingTower`] ([`Self::new`],
whose signature is preserved) or from the caller's own psychrometric plant
state ([`Self::from_scalars`]). See the module documentation for what each
path is allowed to paint — in particular, why the physics path draws no
plume.

All temperatures are absolute thermodynamic temperatures (`uom`-typed).
`min_temp`/`max_temp` bound the diverging colour scale; because the map is
diverging (blue at min, neutral white at the *midpoint*, red at max), set
them about a reference that matters rather than clamping to the extremes
seen.

```rust
pub struct CoolingTowerVisual {
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
  pub fn new(physics: CoolingTower, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Wrap a [`CoolingTower`] with the given screen geometry and

- ```rust
  pub fn from_scalars(kind: CoolingTowerKind, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature, scalars: CoolingTowerScalars) -> Self { /* ... */ }
  ```
  Build a cooling-tower visual from the caller's own psychrometric plant

- ```rust
  pub fn with_kind(self: Self, kind: CoolingTowerKind) -> Self { /* ... */ }
  ```
  Draw a different draught architecture with the same state.

- ```rust
  pub fn with_fan_speed(self: Self, fan_speed: AngularVelocity) -> Self { /* ... */ }
  ```
  Set the fan shaft speed. Builder-style.

- ```rust
  pub fn at_time(self: Self, simulation_time: Time) -> Self { /* ... */ }
  ```
  Supply the **application's** simulation time, which sets the fan phase

- ```rust
  pub fn without_labels(self: Self) -> Self { /* ... */ }
  ```
  Turn the internal component labels and readouts off — for thumbnails.

- ```rust
  pub fn kind(self: &Self) -> CoolingTowerKind { /* ... */ }
  ```
  Which draught architecture this visual draws.

- ```rust
  pub fn size(self: &Self) -> Vec2 { /* ... */ }
  ```
  On-screen size of the box the artwork letterboxes into, in points.

- ```rust
  pub fn state(self: &Self) -> &CoolingTowerVisualState { /* ... */ }
  ```
  Where this visual gets its state.

- ```rust
  pub fn scalars(self: &Self) -> Option<CoolingTowerScalars> { /* ... */ }
  ```
  The scalar state the artwork is drawn from, or `None` on the

- ```rust
  pub fn physics(self: &Self) -> Option<CoolingTower> { /* ... */ }
  ```
  The wrapped component, or `None` on the scalar path.

- ```rust
  pub fn fan_angle(self: &Self) -> Option<Angle> { /* ... */ }
  ```
  Current fan phase angle, `theta = omega * t`, or `None` for a kind with

- ```rust
  pub fn approach(self: &Self) -> Option<TemperatureInterval> { /* ... */ }
  ```
  The **achieved** approach to the entering wet-bulb temperature, or

- ```rust
  pub fn target_approach(self: &Self) -> Option<TemperatureInterval> { /* ... */ }
  ```
  The wrapped component's **target** approach — a set-point, not an

- ```rust
  pub fn cooling_range(self: &Self) -> Option<TemperatureInterval> { /* ... */ }
  ```
  The cooling range across the tower, or `None` when the cold-water

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws the cooling tower for [`CoolingTowerVisual::kind`], coloured by

- **WithSubscriber**
### Functions

#### Function `approach_to_wet_bulb`

The **approach**: how far the cold water leaving the tower sits above the
entering air's wet-bulb temperature, `T_water,out - T_wb`.

This is the number that governs a cooling tower. Evaporative cooling drives
the water towards the wet-bulb temperature and cannot pass it, so a smaller
approach means a better (or more generously sized, or more lightly loaded)
tower. Both arguments are absolute thermodynamic temperatures (`uom`-typed,
kelvin internally); the result is a [`TemperatureInterval`], because a
difference of two temperatures is an interval and not a temperature.

**A non-positive approach is not clamped away.** It means the caller's model
has the water leaving at or below the wet-bulb temperature, which a real
tower cannot do, and the widget displays it as it is so the reader can see
the model is out of range. Hiding it would be the more dangerous choice.

```rust
pub fn approach_to_wet_bulb(water_outlet: uom::si::f64::ThermodynamicTemperature, wet_bulb: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::TemperatureInterval { /* ... */ }
```

#### Function `cooling_range`

The **range**: how much the circulating water is cooled across the tower,
`T_water,in - T_water,out`.

Unlike the approach, the range is set by the heat load and the water flow
rate rather than by the tower itself — a tower does not "choose" its range.
Both arguments are absolute thermodynamic temperatures; the result is a
[`TemperatureInterval`].

```rust
pub fn cooling_range(water_inlet: uom::si::f64::ThermodynamicTemperature, water_outlet: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::TemperatureInterval { /* ... */ }
```

#### Function `plume_opacity`

How strongly the exit plume is drawn, dimensionless in `[0, 1]`, from the
exit air's relative humidity.

`0.0` draws no plume at all, `1.0` the densest one. The ramp runs from
[`PLUME_VISIBLE_RH_MIN`] to saturation (`R = 1`), so air leaving the fill
well short of saturation carries its water invisibly and air leaving at
saturation gives a full plume.

**This is a display mapping of a real supplied property, not a plume model.**
Whether a plume is actually visible depends on the mixing line between the
exit air and the ambient air crossing the saturation curve, which is a
psychrometric mixing calculation and belongs in `tampines`, not in this
presentation crate. What is honest to say — and all that is claimed here —
is that a plume needs the exit air to be at or very near saturation, and
this ramp shows how near it is.

A relative humidity above 1 saturates at full opacity rather than growing
further: `HumidAirState` describes single-phase moist air, so exit air
already carrying condensed droplets is outside what the caller's own state
object can represent. A **non-finite** relative humidity draws no plume,
which is the visible outcome — a NaN in the caller's model must not appear
as a confident cloud.

```rust
pub fn plume_opacity(relative_humidity: uom::si::f64::Ratio) -> f32 { /* ... */ }
```

#### Function `hyperboloid_half_width`

Half-width of the hyperboloid shell at height fraction `f`, in screen
points.

`f` is measured **from the top** of the drawing: `0.0` at the top rim,
`1.0` at the base. The meridian of a hyperboloid of one sheet is the
hyperbola

```text
x(y) = throat_half * sqrt(1 + ((f - throat_fraction) / flare)^2)
```

so the shell is genuinely drawn from the curve that defines the surface
rather than sketched freehand: the minimum is exactly at the throat, and the
flare is symmetric in the *distance* from the throat, which is what gives a
cooling tower its waist.

`throat_half` is the half-width at the waist in screen points and `flare`
(see [`HYPERBOLOID_FLARE`]) is in units of the drawn height — smaller values
flare faster. A non-positive `flare` would divide by zero, so it is treated
as a straight cylinder of constant `throat_half`.

```rust
pub fn hyperboloid_half_width(f: f32, throat_fraction: f32, throat_half: f32, flare: f32) -> f32 { /* ... */ }
```

### Constants and Statics

#### Constant `NATURAL_DRAUGHT_ASPECT_RATIO`

Width-to-height ratio of the natural-draught (hyperbolic) tower,
dimensionless.

**Chosen by eye, not taken from a design.** A natural-draught shell is
taller than it is wide because the chimney height *is* the draught: the
column of warm, moist — and therefore less dense — air inside the shell is
what pulls fresh air through the fill, so there is no fan.

```rust
pub const NATURAL_DRAUGHT_ASPECT_RATIO: f32 = 0.72;
```

#### Constant `INDUCED_DRAUGHT_ASPECT_RATIO`

Width-to-height ratio of the mechanical induced-draught tower,
dimensionless.

**Chosen by eye, not taken from a design.** A fan-driven cell needs no
chimney, so it is a low broad box with the fan on the roof.

```rust
pub const INDUCED_DRAUGHT_ASPECT_RATIO: f32 = 1.45;
```

#### Constant `HYPERBOLOID_THROAT_FRACTION`

Height fraction of the hyperboloid's throat, measured **from the top** of
the drawing (`0.0` the top rim, `1.0` the base). Dimensionless.

The waist sits well up the shell, with a short flare above it to the cornice
and a long flare below it to the air inlet.

```rust
pub const HYPERBOLOID_THROAT_FRACTION: f32 = 0.16;
```

#### Constant `HYPERBOLOID_FLARE`

Flare parameter of the hyperboloid meridian, dimensionless — the `b` in
`x = a * sqrt(1 + (y / b)^2)`, in units of the drawn height.

Smaller values flare faster. Chosen so the base is about 1.45 times the
throat width, which reads as a cooling tower rather than as a chimney.

```rust
pub const HYPERBOLOID_FLARE: f32 = 0.80;
```

#### Constant `PLUME_VISIBLE_RH_MIN`

Exit-air relative humidity below which no plume is drawn, dimensionless.

See [`plume_opacity`] — this is a **display threshold**, not a
plume-formation criterion.

```rust
pub const PLUME_VISIBLE_RH_MIN: f32 = 0.90;
```

## Module `excursion`

Fuel-excursion overlay — the annotation a reactor gets when its fuel goes
past the temperature it is specified for.

This is an **overlay over an arbitrary screen rectangle**, not part of any
vessel widget. It composes over
[`crate::components::Htr10ReactorVesselVisual`],
[`crate::components::FhrReactorVesselVisual`] or anything else the
application draws, by handing it the same rectangle the vessel was drawn
into. Nothing in this module knows what is underneath it.

# What this depicts, and what it refuses to depict

**It does not depict an explosion, and it must never be changed to.** An
earlier version of this module drew a shock front and flying debris. That
was wrong on the physics and it inverted the central claim of the fuel form
it was annotating:

- **A modular HTGR has no blast mechanism available at these conditions.**
  The coolant is helium — no phase change, no stored pressure energy of the
  kind a water reactor has — over a graphite core of low power density and
  very large heat capacity. There is no energy source that produces a blast.
- **TRISO fuel is *retaining* across most of the band this overlay covers.**
  Coating integrity for the HTR-10 was experimentally proven to
  **1250 degC** and its design maximum fuel temperature is **1230 degC**
  (Gao & Shi 2002 — see [`ExcursionTrigger::htr10_fuel_temperature`]); the
  German heating tests showed **no particle failures and no noticeable
  caesium or strontium release during the first few hundred hours of any
  1600 degC test**, i.e. near-100 % retention at the generic limit itself
  (Kugeler et al. 2017, section 4.2.1). Drawing destruction there would
  contradict the evidence the same workspace cites.
- **The real failure mode is progressive fission-product release** —
  coating degradation and gradual release as temperature and *time at
  temperature* accumulate. It is slow and it is passive. That is what
  [`ExcursionStage::FissionProductRelease`] draws: a fuel region escalating
  in incandescence with release marks drifting out of it, on a palette that
  is deliberately not the temperature scale.

# The sourced landmarks

| Temperature | What it is | Source |
|---|---|---|
| 1230 degC | The HTR-10's **own specified** maximum fuel temperature, normal and accident conditions | Gao & Shi (2002), via [`crate::htr10::design::Htr10FuelTemperatureLimits::fuel_temperature_limit`] |
| 1250 degC | Coating integrity **experimentally proven** to this temperature — the basis of the 1230 degC design limit | Gao & Shi (2002) section 1, recorded in `docs/reactor-scoping/htr10-plant-data.md` |
| 1600 degC | The **generic** modular-HTR fuel temperature limit; set from an estimated ~1500 degC maximum core temperature plus allowance for thermal-property uncertainty. Heating tests show near-100 % retention here for the first hundred hours or more | [`crate::htr10::design::generic_coated_particle_retention_limit`]; Kugeler et al. (2017) section 4.2.1 |
| 1700-1800 degC | Where particle failures and release inventories **increase**; at 1800 degC there is no delay in caesium release and SiC becomes permeable to most fission products | Kugeler et al. (2017) section 4.2.1 |

**1230 and 1600 degC are different numbers from different sources and must
not be conflated** — `crate::htr10::design` warns that mixing them up
misstates the HTR-10 margin by 370 K. This module uses them as two distinct
landmarks: the annotation *starts* at the reactor's own limit and reaches
full intensity at the generic figure.

**Kugeler, K., Nabielek, H. and Buckthorpe, D. (2017).** *The High
Temperature Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul.*
EUR 28712 EN, JRC107642, Publications Office of the European Union,
doi:10.2760/270321. Open tier, catalogued in this workspace as
`kugeler2017vhtr`; reuse authorised provided the source is acknowledged
(EC Decision 2011/833/EU). Facts are cited here, not re-hosted.

# Why the escalation happens only at the far landmark

[`ExcursionStage::FissionProductRelease`] is reached only at
[`RELEASE_INTENSITY`], which is the **top** of the ramp — the generic
1600 degC figure. Escalating earlier would draw release across a band in
which retention is precisely what the heating tests demonstrate. Between the
two landmarks the annotation says what is true and no more: the fuel is
above the limit it is specified for.

# What the overlay claims above the far landmark

Not a specific physical outcome. Above the generic limit the honest
statement is that **this model has left its valid envelope** — this crate
has no fission-product release model, no source term and no coating-failure
model, and nothing drawn here is one. The escalating incandescence and the
drifting release marks are a *warning annotation* naming the mechanism that
applies, not a prediction of it. Per `RESPONSIBLE_USE.md` this crate's
examples are educational demonstrations and must never be presented as
authoritative for safety analysis, licensing or emergency response — the
overlay says so on screen, in [`ExcursionStage::caption`].

# What drives it

The trigger is an **input** ([`ExcursionTrigger`]), not something read out
of a plant model inside the widget: this crate's `CLAUDE.md` keeps `src/`
presentation-only, so the fuel temperature is computed by the caller's
physics and handed in. [`ExcursionTrigger::Intensity`] is for callers whose
criterion is not a fuel temperature at all.

# Animation state is application-owned

Release grows with **time held above the limit** — the heating-test releases
are quoted in hundreds of hours, not instants — so the annotation is
time-phased. Widgets here are consumed by value and rebuilt on every
repaint, so a phase stored inside the widget would reset to zero every frame
and never advance. The **application** owns the elapsed time and passes it in
with [`ExcursionOverlay::since_trigger`], the same ownership rule as
[`crate::animation::TracerTrain`] and [`crate::components::PumpVisual`]'s
shaft phase. The phase is a function of the caller's **simulation** clock,
never a wall clock, so a paused simulation shows a still overlay and a
replayed one reproduces frame for frame.

**The on-screen ramp is a presentation constant and is not a release rate.**
See [`RELEASE_RAMP_SECONDS`].

```rust
pub mod excursion { /* ... */ }
```

### Types

#### Enum `ExcursionTrigger`

What tells the overlay how far past its limit the fuel is.

Enum dispatch, not a trait object, per the workspace's mandatory "no trait
objects" Rust design rule. The set of triggers is closed: either the caller
has a fuel temperature and the landmarks to judge it against, or it has
already reduced its criterion to a number.

```rust
pub enum ExcursionTrigger {
    Intensity(f32),
    FuelTemperature {
        fuel: uom::si::f64::ThermodynamicTemperature,
        limit: uom::si::f64::ThermodynamicTemperature,
        full_intensity_at: uom::si::f64::ThermodynamicTemperature,
    },
}
```

##### Variants

###### `Intensity`

A caller-computed intensity in `[0, 1]`, for a criterion that is not a
fuel temperature (a cladding limit, a pressure, an operator's own
judgement in a teaching scenario). Values outside `[0, 1]` are clamped;
a non-finite value gives full intensity, for the reason given on
[`excursion_intensity`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f32` |  |

###### `FuelTemperature`

A fuel temperature judged against two caller-supplied landmarks.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `fuel` | `uom::si::f64::ThermodynamicTemperature` | The fuel temperature the caller's physics produced. |
| `limit` | `uom::si::f64::ThermodynamicTemperature` | The temperature limit this fuel is specified to stay below. The<br>annotation starts here. |
| `full_intensity_at` | `uom::si::f64::ThermodynamicTemperature` | The temperature at which the annotation reaches full intensity and<br>escalates. A **display** landmark chosen by the caller — it is not a<br>destruction threshold, and no such threshold is published or<br>invented here. See the module documentation. |

##### Implementations

###### Methods

- ```rust
  pub fn htr10_fuel_temperature(fuel: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Judge an HTR-10 fuel temperature against the HTR-10's **own** limit.

- ```rust
  pub fn intensity(self: Self) -> f32 { /* ... */ }
  ```
  Intensity in `[0, 1]` this trigger resolves to.

- ```rust
  pub fn fuel_temperature(self: Self) -> Option<ThermodynamicTemperature> { /* ... */ }
  ```
  The fuel temperature behind this trigger, or `None` for

- ```rust
  pub fn limit(self: Self) -> Option<ThermodynamicTemperature> { /* ... */ }
  ```
  The limit the fuel temperature is judged against, or `None` for

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExcursionTrigger { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExcursionTrigger) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `ExcursionStage`

How far the annotation has escalated.

Enum dispatch per the workspace's "no trait objects" rule; derived from the
intensity by [`Self::from_intensity`], so the thresholds live in one place.

```rust
pub enum ExcursionStage {
    Quiescent,
    LimitExceeded,
    FissionProductRelease,
}
```

##### Variants

###### `Quiescent`

At or below the limit: **nothing is drawn**. A reactor inside its
specification gets no annotation at all, not a faint one.

###### `LimitExceeded`

Above the limit the fuel is specified for, but below
[`RELEASE_INTENSITY`]: a hazard border and a banner saying so.

**The coating is not assumed to have failed here, and nothing is drawn
as though it had.** For the HTR-10 landmarks this band runs from
1230 degC to 1600 degC, across which the heating tests show near-100 %
retention for the first hundred hours or more (Kugeler et al. 2017,
section 4.2.1). The vessel underneath is left visible and unobscured —
it is still the useful picture.

###### `FissionProductRelease`

At or above [`RELEASE_INTENSITY`]: the fuel region escalates in
incandescence and release marks drift out of it.

This names the mechanism that actually applies to coated-particle fuel —
progressive coating degradation and fission-product release, which is
slow and passive — and simultaneously states that **the model has left
its valid envelope**: this crate has no release model, so nothing drawn
is a source term or a prediction. The vessel underneath is shrouded,
because it no longer depicts anything the model can stand behind.

##### Implementations

###### Methods

- ```rust
  pub fn from_intensity(intensity: f32) -> Self { /* ... */ }
  ```
  The stage an intensity in `[0, 1]` corresponds to.

- ```rust
  pub fn is_drawn(self: Self) -> bool { /* ... */ }
  ```
  Whether this stage draws anything at all.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short banner headline for this stage.

- ```rust
  pub fn caption(self: Self) -> &'static str { /* ... */ }
  ```
  The sentence printed under the headline.

- ```rust
  pub fn mechanism(self: Self) -> &'static str { /* ... */ }
  ```
  A second line naming the mechanism, for the stage that has one.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ExcursionStage { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExcursionStage) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `ReleaseSpecies`

One fission product the release annotation can name.

Held as `&'static str` fields rather than owned strings — no lifetime
*parameters* are introduced, per the workspace rule.

```rust
pub struct ReleaseSpecies {
    pub symbol: &'static str,
    pub nuclide: &'static str,
    pub note: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `symbol` | `&'static str` | Short label drawn on the annotation, e.g. `"Cs"`. |
| `nuclide` | `&'static str` | The nuclide the label stands for, e.g. `"137Cs"`. |
| `note` | `&'static str` | Why this one appears where it does in the order. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReleaseSpecies { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReleaseSpecies) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `ExcursionOverlay`

An overlay that annotates a fuel excursion over an arbitrary screen
rectangle.

Composed **on top of** whatever vessel the application drew, by giving it
the same centre and size:

```ignore
ui.add(Htr10ReactorVesselVisual::new(/* ... */));
ui.add(
    ExcursionOverlay::new(
        ExcursionTrigger::htr10_fuel_temperature(peak_fuel_temperature),
        vessel_centre,
        vessel_size,
    )
    .since_trigger(time_above_the_limit),
);
```

Nothing is drawn while the fuel is within its limit, so the overlay can be
added unconditionally every frame. See the module documentation for what the
annotation claims (very little) and what it refuses to claim (an explosion,
a release rate, a source term, or a destruction temperature).

```rust
pub struct ExcursionOverlay {
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
  pub fn new(trigger: ExcursionTrigger, screen_position: Pos2, screen_vector: Vec2) -> Self { /* ... */ }
  ```
  Build an overlay for `trigger` over the box centred at

- ```rust
  pub fn since_trigger(self: Self, elapsed: Time) -> Self { /* ... */ }
  ```
  Supply the **application-owned** simulation time elapsed since the

- ```rust
  pub fn with_subject(self: Self, subject: String) -> Self { /* ... */ }
  ```
  Name what is being annotated, e.g. `"HTR-10 core"`. Builder-style.

- ```rust
  pub fn without_labels(self: Self) -> Self { /* ... */ }
  ```
  Turn the banner and readouts off, leaving only the graphic — for

- ```rust
  pub fn trigger(self: &Self) -> ExcursionTrigger { /* ... */ }
  ```
  The trigger this overlay was built from.

- ```rust
  pub fn intensity(self: &Self) -> f32 { /* ... */ }
  ```
  Intensity in `[0, 1]` the trigger resolves to.

- ```rust
  pub fn stage(self: &Self) -> ExcursionStage { /* ... */ }
  ```
  The stage the overlay is at.

- ```rust
  pub fn phase(self: &Self) -> f32 { /* ... */ }
  ```
  Drawn phase in `[0, 1]`; see [`release_phase`].

- ```rust
  pub fn size(self: &Self) -> Vec2 { /* ... */ }
  ```
  On-screen size of the annotated box, in points.

- ```rust
  pub fn overshoot_kelvin(self: &Self) -> Option<f64> { /* ... */ }
  ```
  How far the fuel is past its limit, in kelvin, or `None` when the

- ```rust
  pub fn named_species(self: &Self) -> Vec<ReleaseSpecies> { /* ... */ }
  ```
  The species named on the annotation at this phase, in

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Paints the annotation for [`ExcursionOverlay::stage`] over the given

- **WithSubscriber**
### Functions

#### Function `excursion_intensity`

Intensity of a fuel-temperature excursion, dimensionless in `[0, 1]`.

`0.0` at or below `limit`, `1.0` at or above `full_intensity_at`, linear in
temperature between them. All three arguments are absolute thermodynamic
temperatures (`uom`-typed, kelvin internally, conventionally quoted in
degrees Celsius).

The caller chooses both landmarks, deliberately: reactors do not share a
fuel-temperature limit, and this crate must not pick one on a caller's
behalf. See [`ExcursionTrigger::htr10_fuel_temperature`] for the HTR-10's
own pair and the warning that goes with them.

**A non-finite fuel temperature gives full intensity**, not zero. A model
that has produced a NaN or an infinity has certainly left its valid
envelope, and the dangerous failure direction here is the quiet one — a
broken model must not look like a healthy reactor. A degenerate span
(`full_intensity_at` at or below `limit`) is treated as a step: anything
above the limit is full intensity.

```rust
pub fn excursion_intensity(fuel: uom::si::f64::ThermodynamicTemperature, limit: uom::si::f64::ThermodynamicTemperature, full_intensity_at: uom::si::f64::ThermodynamicTemperature) -> f32 { /* ... */ }
```

#### Function `species_visible`

Whether release species `index` of [`RELEASE_ORDER`] is named yet, at drawn
phase `phase` in `[0, 1]`.

The species are spread evenly across the phase so the sourced *order* reads
on screen. **The spacing is a display device and carries no timescale** —
see [`RELEASE_ORDER`] and [`RELEASE_RAMP_SECONDS`].

```rust
pub fn species_visible(index: usize, phase: f32) -> bool { /* ... */ }
```

#### Function `release_phase`

How far the annotation has progressed, dimensionless in `[0, 1]`, for an
elapsed **simulation** time since the excursion was triggered.

Reaches `1.0` after [`RELEASE_RAMP_SECONDS`] and stays there: released
fission products do not go back into the fuel, so the annotation does not
fade away. Negative or non-finite elapsed times give `0.0` — the instant of
the trigger — rather than anything undefined.

```rust
pub fn release_phase(elapsed: uom::si::f64::Time) -> f32 { /* ... */ }
```

#### Function `release_reach`

How far the release marks have drifted from the fuel region at phase
`phase`, in screen points.

Grows as `max_reach * sqrt(phase)`, so the marks move away quickly at first
and then settle. That is a **display easing chosen because a linear ramp
reads as a mechanical wipe**, and it is *not* a transport calculation: this
module solves nothing and must not be cited as though it did.

`phase` outside `[0, 1]` is clamped; a non-finite phase or reach gives zero.

```rust
pub fn release_reach(phase: f32, max_reach: f32) -> f32 { /* ... */ }
```

#### Function `banner_pulse`

Banner pulse in `[0, 1]` at elapsed **simulation** time `elapsed`.

A slow sine at [`BANNER_PULSE_HZ`], used only to keep the warning banner
from being read as a static decoration. Being a function of the caller's
simulation clock, a paused simulation shows a still banner and a replay
reproduces it exactly. A non-finite time gives full brightness — a broken
clock must not hide the warning.

```rust
pub fn banner_pulse(elapsed: uom::si::f64::Time) -> f32 { /* ... */ }
```

### Constants and Statics

#### Constant `RELEASE_INTENSITY`

Intensity at or above which the overlay escalates from a limit warning to
the fission-product-release annotation, dimensionless.

**This sits at the top of the ramp — `1.0` — deliberately**, so the release
annotation appears only once the fuel reaches the far landmark the caller
supplied (the generic 1600 degC figure, for
[`ExcursionTrigger::htr10_fuel_temperature`]).

The reason is evidential, not stylistic. In the German core-heat-up
simulation tests on irradiated LEU UO2 TRISO spherical fuel elements, *no*
single particle failures and no noticeable caesium or strontium release were
observed during the first few hundred hours of any 1600 degC heating test —
near-100 % retention at the limit itself (Kugeler et al. 2017, section
4.2.1). Escalating below that would depict release across the very band in
which retention is demonstrated.

```rust
pub const RELEASE_INTENSITY: f32 = 1.0;
```

#### Constant `RELEASE_RAMP_SECONDS`

How long, in **simulation** seconds, the release annotation takes to reach
its full drawn extent.

**A presentation constant. It is not a release rate and implies no
timescale.** The real measurements are in hundreds of hours — the heating
tests report near-complete retention "for the accident-specific first
hundred hours or more" at 1600 degC, and release from already-exposed
kernels approaching 100 % only "after 50 to 100 h" (Kugeler et al. 2017,
section 4.2.1). A few seconds of screen time is a legibility choice and
nothing else; this crate has no release model to derive a rate from.

```rust
pub const RELEASE_RAMP_SECONDS: f64 = 1.4;
```

#### Constant `BANNER_PULSE_HZ`

Frequency, in hertz of **simulation** time, at which the warning banner
pulses. A presentation constant; see [`banner_pulse`].

```rust
pub const BANNER_PULSE_HZ: f64 = 1.6;
```

#### Constant `HAZARD`

Hazard amber: the warning border, the banner rule, the release marks.

```rust
pub const HAZARD: egui::Color32 = _;
```

#### Constant `INCANDESCENT`

Incandescent white: the hottest part of the fuel region.

```rust
pub const INCANDESCENT: egui::Color32 = _;
```

#### Constant `RELEASE_ORDER`

The order release marks are named in, earliest first.

**The order is sourced; the phase at which each label appears is
ILLUSTRATIVE.** Kugeler et al. (2017) section 4.2.1 state that caesium is
retained at 1600 degC by the kernel, the SiC and the A3 matrix, and that at
1800 degC there is no delay in caesium release and SiC becomes permeable to
most fission products; that **krypton is always released later than
caesium**, because of the additional retention provided by the dense intact
pyrocarbon layers; and that **strontium is retained much better than
caesium** in oxide kernels and the sphere matrix, so strontium release
generally occurs later still. That gives the sequence below.

It does **not** give a time or a temperature at which each appears on
screen, and none is claimed: [`species_visible`] spreads them across the
drawn phase purely so the sequence is legible. The wider set of products the
same section calls most relevant — 90Sr, 110mAg, 134Cs, 137Cs, 85Kr, 131I
and 133Xe — is named in the module documentation rather than drawn, because
this crate has no inventory to draw it from.

```rust
pub const RELEASE_ORDER: &[ReleaseSpecies] = _;
```

## Module `fhr_reactor_vessel`

Visual FHR (fluoride-salt-cooled high-temperature reactor) vessel.

A pebble-bed FHR vessel drawn as cut-away art: the pebble bed, the coolant
passing through it, the inlet and outlet plena, two downcomers, and two
control rods at their commanded insertion depth. Every region is filled
from a temperature the caller supplies, so the vessel reads as a
temperature field rather than as a static picture with numbers beside it.

Unlike [`crate::components::reactor_vessel::ReactorVesselVisual`] — which
wraps a `nee_soon` kinetics model and colours a single rectangle by lumped
fuel temperature — this widget takes **fourteen independent temperatures**
and owns no physics. It is deliberately scalar-fed, for the same reason
[`crate::components::pipe::PipeVisual::from_scalars`] is: an FHR simulator
already holds these temperatures in its own plant model, and requiring a
particular physics type here would force every caller to adopt it.

Scalar-fed does **not** mean placeholder. Callers pass real state from
their own model; do not fabricate values to feed it.

Migrated into the shared component library from `fhr_sim_v2`'s local widget
set (bead `op-wqk.8`, step 2), so other reactor simulators can draw a
pebble-bed vessel without re-deriving the art.

# Why this bed is drawn UPSIDE DOWN — the pebbles float

The pebbles here are placed from the same baked, gravity-settled DEM
packing the HTR-10 widget uses ([`crate::components::pebble_packing`]), but
**inverted**, and that inversion is physics rather than a drawing accident.

An FHR's coolant is molten FLiBe at roughly 1940 kg/m³ at operating
temperature, while a graphite pebble is roughly 1740–1800 kg/m³. The
pebbles are therefore **less dense than the salt they sit in, and float**.
They rise, pack upward against a retaining structure at the **top** of the
core, and are injected low and removed high — the mirror image of HTR-10,
where helium is a gas, the pebbles settle downward under their own weight,
and the bed drains through a cone at the bottom.

So the bed's dense, compressed base is drawn at the **top** of the core and
its loose free surface faces **down**. If a future edit makes the bottom of
this bed the dense end, it has silently turned an FHR into a gas-cooled
reactor. The inversion is expressed once, as
[`crate::components::htr10_reactor_vessel::VerticalSense::Buoyant`].

```rust
pub mod fhr_reactor_vessel { /* ... */ }
```

### Types

#### Struct `FhrReactorVesselVisual`

Visual representation of a pebble-bed FHR reactor vessel.

Holds one temperature per drawn region plus the two control-rod insertion
fractions. All temperatures are absolute thermodynamic temperatures
(`uom`-typed, so the unit is carried by the type); the colour mapping works
in degrees Celsius internally but callers never need to know that.

Insertion fractions are dimensionless in `[0, 1]`: `0.0` is fully
withdrawn, `1.0` fully inserted. Values outside that range are clamped at
render time rather than rejected, so a controller that transiently
overshoots does not panic the GUI thread.

```rust
pub struct FhrReactorVesselVisual {
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
  pub fn new(size: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature, pebble_core_temp: ThermodynamicTemperature, pebble_bed_coolant_temp: ThermodynamicTemperature, core_bottom_temp: ThermodynamicTemperature, core_top_temp: ThermodynamicTemperature, core_inlet_temp: ThermodynamicTemperature, core_outlet_temp: ThermodynamicTemperature, left_downcomer_upper_temp: ThermodynamicTemperature, left_downcomer_mid_temp: ThermodynamicTemperature, left_downcomer_lower_temp: ThermodynamicTemperature, right_downcomer_upper_temp: ThermodynamicTemperature, right_downcomer_mid_temp: ThermodynamicTemperature, right_downcomer_lower_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a vessel visual from screen size, a colour-mapping temperature

- ```rust
  pub fn hotness(self: &Self, temp: ThermodynamicTemperature) -> f32 { /* ... */ }
  ```
  Where `temp` falls in the display range, as a dimensionless fraction.

- ```rust
  pub fn set_min_temp(self: &mut Self, min_temp: ThermodynamicTemperature) { /* ... */ }
  ```
  Sets the temperature mapped to the coldest displayable colour.

- ```rust
  pub fn set_max_temp(self: &mut Self, max_temp: ThermodynamicTemperature) { /* ... */ }
  ```
  Sets the temperature mapped to the hottest displayable colour.

- ```rust
  pub fn size(self: &Self) -> Vec2 { /* ... */ }
  ```
  On-screen size of the widget, in points.

- ```rust
  pub fn set_left_cr_frac(self: &mut Self, left_control_rod_insertion_frac: f32) { /* ... */ }
  ```
  Sets how far the left control rod is inserted.

- ```rust
  pub fn set_right_cr_frac(self: &mut Self, right_control_rod_insertion_frac: f32) { /* ... */ }
  ```
  Sets how far the right control rod is inserted.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut egui::Ui) -> egui::Response { /* ... */ }
    ```
    Renders the vessel cut-away: pebble bed, coolant regions, inlet and

- **WithSubscriber**
### Functions

#### Function `fit_native_aspect`

The largest sub-rectangle of `available` carrying
[`NATIVE_ASPECT_RATIO`], centred within it.

This is a letterbox: the vessel keeps its proportions and the leftover space
is simply not drawn into, rather than the artwork being stretched to fill.

A caller that already sizes to the native ratio — as `fhr_sim_v2` does, with
its hardcoded 225 x 1050 box — gets the identical rectangle back, so this is
invisible there and only takes effect for callers using a different shape.

```rust
pub fn fit_native_aspect(available: egui::Rect) -> egui::Rect { /* ... */ }
```

### Constants and Statics

#### Constant `NATIVE_ASPECT_RATIO`

Width-to-height ratio the artwork was authored against.

The drawing was laid out by hand at 225 x 1050 points in `fhr_sim_v2`, and
every internal coordinate is a fraction of the box it is given. Fractions
scale, but they do not preserve *proportion*: hand the widget a square box
and the vessel stretches, because the horizontal features expand while the
vertical ones do not shrink to match.

[`fit_native_aspect`] resolves that by fitting this ratio inside whatever
box the caller allocated, so the artwork stays correctly proportioned at
any size.

```rust
pub const NATIVE_ASPECT_RATIO: f32 = _;
```

## Module `heat_exchanger`

Schematic two-stream recuperator art.

A heat exchanger is the most general piece of equipment on a plant
schematic: two single-phase streams, separated by a wall, one giving heat to
the other. Unlike a condenser — where the interesting thing is the phase
change — nothing about a recuperator is interesting *except the arrangement*.
Which way the two streams run relative to each other decides how much of the
available temperature difference the exchanger can actually use, and it is
the one thing a reader must be able to see at a glance.

So the artwork draws, in order:

- the **body**, either a shell with a tube bundle or a plate pack between two
  end frames, per [`HeatExchangerConstruction`];
- the **two streams**, each graded along **its own path** from its inlet to
  its outlet, so a stream visibly cools (or heats) as it crosses;
- **flow arrows on both streams**, drawn from each stream's real inlet to its
  real outlet — in [`HeatExchangerKind::CounterFlow`] they point in opposite
  directions, and that opposition is drawn, not merely captioned;
- the **nozzles / ports** at the ends the streams really enter and leave
  from, which move when the arrangement changes;
- the **terminal approaches** at each end, bracketed between the two streams
  and labelled with the temperature difference there;
- a **temperature-profile strip** under the body, plotting both streams
  against length, which is where the two profiles converging toward each
  other — or failing to — is unmistakable.

# Why the arrangement is the whole point

In parallel flow both streams enter at the same end, so the cold stream is
chasing a target that is running away from it: the two profiles converge
toward a common temperature and **the cold outlet can never reach, let alone
exceed, the hot outlet**. In counter-flow the cold stream leaves at the end
where the hot stream *arrives*, so it is exchanging against the hottest fluid
in the machine at exactly the point it is hottest itself, and the cold outlet
**can** come out above the hot outlet. That is a *temperature cross*, and it
is the single most useful fact about flow arrangement.

Both statements are checkable rather than decorative:
[`HeatExchangerKind::permits_temperature_cross`] states which arrangement can
do it, and [`approach_verdict`] decides, from the four temperatures the
caller supplied, whether those numbers are consistent with the arrangement
being drawn. A parallel-flow exchanger handed a crossed pair of outlets is
drawn with an explicit "impossible for this arrangement" tag rather than
quietly rendered as though it were fine.

**That check is a sign convention, not a model.** It compares the two
terminal approaches the caller's own numbers imply; it computes no duty, no
effectiveness and no outlet temperature, and it is not a rating method. The
rating algebra already exists in
[`outram_park_fork_dwsim_libs::heat_exchanger`] and belongs there, not in a
presentation crate — see this crate's "no new physics" rule.

# Dispatch

[`HeatExchangerKind`], [`HeatExchangerConstruction`] and
[`HeatExchangerVisualState`] are enums, not trait objects, per the
workspace's mandatory "no trait objects" Rust design rule: all three sets are
closed and known at compile time, so adding a member is a variant and the
compiler then points at every match that needs handling.

Two axes are separated deliberately. **Arrangement** ([`HeatExchangerKind`])
is thermodynamics and changes where the streams enter and which way they run.
**Construction** ([`HeatExchangerConstruction`]) is mechanical and changes
what the inside of the body looks like. They are independent — every
construction can be plumbed either way round — so folding them into one enum
would have produced a variant list that lies about the geometry.

# What is drawn from real state, and what is left neutral

This is the honesty rule the whole widget library is built on, and this
component is a more interesting case than [`crate::components::condenser`],
because [`tampines::components::HeatExchanger`] is **not** state-free.

It stores three things, all of them real:

| Field | Drawn? |
|---|---|
| `arrangement` (co- / counter-current) | **yes** — it picks [`HeatExchangerKind`], so the neutral card still shows its true flow directions |
| `area` (heat-transfer area) | **yes**, as a label |
| `overall_coefficient` (`U`) | **yes**, as a label |

and nothing else. Its `calculate` returns
`TampinesError::NotYetImplemented`, so there are **no temperatures behind
it**. The physics-backed path ([`HeatExchangerVisual::new`], whose signature
is preserved) therefore draws the complete machine with its real flow
directions, real arrows, real nozzle positions and its area and `U`
labelled — and paints **no temperature colour anywhere**, draws no approach
values and no profile. Every fluid region is neutral grey.

The one thing that path picks by itself is the **construction**, which
defaults to [`HeatExchangerConstruction::ShellAndTube`]: the component does
not say whether it is a shell-and-tube or a plate unit, and something has to
be drawn. That is a drawing convention, stated here and changeable with
[`HeatExchangerVisual::with_construction`] — the same status as
[`crate::components::CondenserVisual::new`] defaulting to a two-pass
waterbox. It is not a claim about the caller's equipment.

The state-driven path is [`HeatExchangerVisual::from_scalars`], the same
contract as [`crate::components::PipeVisual::from_scalars`]: the caller
passes **real state from its own model** — both streams' inlet and outlet
temperatures, and optionally the duty. That is a narrower interface, not a
fabricated one, and it is not a stub. The duty in particular is an
[`Option`], so a caller that has temperatures but no measured duty gets no
duty label rather than a plausible-looking number.

# Colour

Both streams are graded by the shared
[`crate::components::temperature_colour`] map, so a heat exchanger reads
identically to every other widget in this library: blue at the cold end of
the display range, neutral white at its **midpoint**, red at the hot end.
There is no second colour axis here — unlike a condenser or a turbine, a
recuperator carries no quality, because both sides stay single-phase.

The gradient along each stream is a **display interpolation** between the two
temperatures the caller supplied (see [`lerp_temperature`]). A real profile
along a recuperator is exponential in position, not linear; computing it is
rating work and belongs in `tampines`, not here. What the artwork claims is
only what it is given: the endpoints are exact, and the path between them is
drawn straight.

# What this is not

**Offline demonstration artwork, not a validated model and not a design
drawing.** Every proportion here — including
[`SHELL_AND_TUBE_ASPECT_RATIO`] and [`PLATE_FRAME_ASPECT_RATIO`] — is chosen
by eye for legibility on screen and is dimensioned from no design whatsoever.
Nothing in this module may be cited or re-used as heat-exchanger design data.
Per `RESPONSIBLE_USE.md` this is for education, research and V&V only — not
for facility operation, reactor control, or safety-critical decisions.

```rust
pub mod heat_exchanger { /* ... */ }
```

### Types

#### Enum `HeatExchangerKind`

Which way the two streams run relative to each other.

This is the thermodynamically significant axis and the reason this widget
exists in more than one form. It maps one-to-one onto
[`FlowArrangement`], the enum
[`tampines::components::HeatExchanger`] actually stores, so the
physics-backed path draws the arrangement the component really holds rather
than a default.

```rust
pub enum HeatExchangerKind {
    CounterFlow,
    ParallelFlow,
}
```

##### Variants

###### `CounterFlow`

Counter-flow: the streams enter at **opposite ends** and run in opposite
directions.

The cold stream leaves at the end the hot stream arrives at, so it does
its last exchanging against the hottest fluid in the machine. This is
always at least as effective as parallel flow for the same `U`, `A` and
heat-capacity rates, and it is the only single-pass arrangement that can
produce a **temperature cross** — a cold outlet above the hot outlet.

###### `ParallelFlow`

Parallel (co-current) flow: the streams enter at the **same end** and run
in the same direction.

The temperature difference is largest at the inlet and decays along the
length as the two profiles converge toward a common value, so the cold
outlet can only ever approach the hot outlet from below and never pass
it. Used where that is a feature — it limits the wall temperature at the
inlet and bounds how far a temperature-sensitive cold stream can be
heated.

##### Implementations

###### Methods

- ```rust
  pub fn from_arrangement(arrangement: FlowArrangement) -> Self { /* ... */ }
  ```
  The arrangement [`tampines::components::HeatExchanger`] stores, drawn as

- ```rust
  pub fn arrangement(self: Self) -> FlowArrangement { /* ... */ }
  ```
  This kind as the [`FlowArrangement`] the physics libraries take, so a

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short display name, for a picker or a card caption.

- ```rust
  pub fn description(self: Self) -> &'static str { /* ... */ }
  ```
  What the arrangement does thermodynamically, in words.

- ```rust
  pub fn cold_stream_path(self: Self) -> &'static str { /* ... */ }
  ```
  Where the cold stream enters and leaves, relative to the hot stream —

- ```rust
  pub fn permits_temperature_cross(self: Self) -> bool { /* ... */ }
  ```
  Whether this arrangement can put the **cold outlet above the hot

- ```rust
  pub fn cold_stream_direction(self: Self) -> f32 { /* ... */ }
  ```
  Direction the cold stream is drawn in, as `+1.0` (left to right, the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HeatExchangerKind { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeatExchangerKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `HeatExchangerConstruction`

What the inside of the body looks like — the mechanical axis, independent of
[`HeatExchangerKind`].

Both constructions can be plumbed either way round, which is why this is a
separate enum rather than more variants on the arrangement.

```rust
pub enum HeatExchangerConstruction {
    ShellAndTube,
    PlateFrame,
}
```

##### Variants

###### `ShellAndTube`

Shell-and-tube: a bundle of tubes carrying the **hot** stream, inside a
shell carrying the **cold** stream over segmental baffles.

The workhorse of process and power plant: tolerant of high pressure and
large temperature difference, mechanically cleanable, and easy to build
large. Drawn long and slim
([`SHELL_AND_TUBE_ASPECT_RATIO`]).

###### `PlateFrame`

Plate-and-frame: a stack of thin pressed plates clamped between two end
frames, with the two streams in **alternating channels**.

Much more surface per unit volume than a shell-and-tube unit and capable
of a very close approach, at the price of gasket-limited pressure and
temperature. Drawn squat ([`PLATE_FRAME_ASPECT_RATIO`]), and the
alternating channels make a counter-flow arrangement especially legible:
adjacent channels carry arrows pointing opposite ways.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short display name, for a picker or a card caption.

- ```rust
  pub fn description(self: Self) -> &'static str { /* ... */ }
  ```
  Where this construction is normally used, in words.

- ```rust
  pub fn hot_stream_location(self: Self) -> &'static str { /* ... */ }
  ```
  Which stream is drawn inside the inner passages, in words — the tubes for

- ```rust
  pub fn native_aspect_ratio(self: Self) -> f32 { /* ... */ }
  ```
  Width-to-height ratio the artwork is drawn at, dimensionless.

- ```rust
  pub fn fit_native_aspect(self: Self, available: Rect) -> Rect { /* ... */ }
  ```
  The largest sub-rectangle of `available` carrying this construction's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HeatExchangerConstruction { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeatExchangerConstruction) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `ApproachVerdict`

What the caller's four temperatures imply about the arrangement being drawn.

This is a **sign check on supplied numbers**, not a model: it computes no
duty, no effectiveness and no outlet temperature. Its only job is to let the
artwork say so when it is being asked to draw a combination that cannot
happen, instead of rendering it as though it were an ordinary operating
point.

```rust
pub enum ApproachVerdict {
    Feasible,
    TemperatureCross,
    Impossible,
}
```

##### Variants

###### `Feasible`

Both terminal approaches are positive and the cold outlet is at or below
the hot outlet: an ordinary operating point for either arrangement.

###### `TemperatureCross`

Both terminal approaches are positive, but the **cold outlet is above the
hot outlet** — a temperature cross.

Reachable only in [`HeatExchangerKind::CounterFlow`]; see
[`HeatExchangerKind::permits_temperature_cross`]. Worth drawing
distinctly, because it is the headline capability of counter-flow and the
reason it is chosen.

###### `Impossible`

A terminal approach is zero or negative: at one end the "cold" stream is
at or above the "hot" stream, so heat would have to flow backwards.

The combination cannot occur in the arrangement being drawn. For
[`HeatExchangerKind::ParallelFlow`] this is what a crossed pair of
outlets reduces to, which is why that kind can never reach
[`Self::TemperatureCross`].

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ApproachVerdict { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ApproachVerdict) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `HeatExchangerDisplayRange`

The temperature range the artwork's colours are graded against.

Both bounds are absolute thermodynamic temperatures (`uom`-typed, so the
compiler enforces the unit; kelvin internally, conventionally quoted in
degrees Celsius). The shared map is **diverging** — blue at `min_temp`,
neutral white at the *midpoint*, red at `max_temp` — so set the range about a
reference that matters rather than clamping it to the extremes seen.

A recuperator usually sits in the middle of a plant's temperature span, so a
plant-wide range renders it in the pale middle of the scale. Narrow the range
to the exchanger's own span to see the approach.

```rust
pub struct HeatExchangerDisplayRange {
    pub min_temp: uom::si::f64::ThermodynamicTemperature,
    pub max_temp: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `min_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature drawn in the coldest displayable colour (blue). |
| `max_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature drawn in the hottest displayable colour (red). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HeatExchangerDisplayRange { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeatExchangerDisplayRange) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `HeatExchangerScalars`

Scalar state of a heat exchanger, as the caller's own model holds it.

Every field is **real state the caller already has**, not a placeholder — see
the module documentation and
[`crate::components::PipeVisual::from_scalars`] for why this narrower
interface exists. Nothing here is invented by the widget.

```rust
pub struct HeatExchangerScalars {
    pub hot_inlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub hot_outlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub cold_inlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub cold_outlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub duty: Option<uom::si::f64::Power>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hot_inlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Hot stream entering the exchanger.<br><br>Colours the left end of the hot stream and the left inlet nozzle. The hot<br>stream is always drawn left to right, whatever the arrangement. |
| `hot_outlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Hot stream leaving the exchanger, at the right end.<br><br>Below [`Self::hot_inlet_temp`] whenever the exchanger is doing anything. |
| `cold_inlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Cold stream entering the exchanger.<br><br>Which **end** that is depends on the arrangement — the right end for<br>[`HeatExchangerKind::CounterFlow`], the left end for<br>[`HeatExchangerKind::ParallelFlow`] — which is exactly what the drawing<br>exists to show. |
| `cold_outlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Cold stream leaving the exchanger.<br><br>Above [`Self::cold_inlet_temp`] whenever the exchanger is doing anything,<br>and — in counter-flow only — possibly above<br>[`Self::hot_outlet_temp`] as well. See [`ApproachVerdict`]. |
| `duty` | `Option<uom::si::f64::Power>` | Heat duty transferred between the streams, or `None` when the caller's<br>model does not have one.<br><br>An [`Option`] deliberately: a caller with four temperatures but no mass<br>flows has no duty, and a duty label it never computed must not appear.<br>`None` draws no duty label at all. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HeatExchangerScalars { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeatExchangerScalars) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `HeatExchangerVisualState`

Where a [`HeatExchangerVisual`] gets the state it renders.

Enum dispatch, not a trait object, per the workspace's mandatory "no trait
objects" Rust design rule.

```rust
pub enum HeatExchangerVisualState {
    Physics(tampines::components::HeatExchanger),
    Scalars(HeatExchangerScalars, HeatExchangerDisplayRange),
}
```

##### Variants

###### `Physics`

Backed by a [`tampines::components::HeatExchanger`] alone.

That component holds a flow arrangement, a heat-transfer area and an
overall coefficient — all of which **are** drawn or labelled — and no
fluid state at all. Its `calculate` returns
`TampinesError::NotYetImplemented`, so this path paints no temperature
colour, no approaches and no profile. See the module documentation.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines::components::HeatExchanger` |  |

###### `Scalars`

Backed by caller-supplied scalars from the caller's own plant model,
graded against the accompanying [`HeatExchangerDisplayRange`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `HeatExchangerScalars` |  |
| 1 | `HeatExchangerDisplayRange` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HeatExchangerVisualState { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeatExchangerVisualState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `HeatExchangerVisual`

Visual representation of a two-stream recuperator, in one of two flow
arrangements and one of two constructions.

Built either from a [`tampines::components::HeatExchanger`] ([`Self::new`],
which draws the machine's real flow directions and labels its area and `U`
but paints no temperature, because that component holds no fluid state) or
from the caller's own scalar plant state ([`Self::from_scalars`]). See the
module documentation for what each path is allowed to paint.

The artwork letterboxes to
[`HeatExchangerConstruction::native_aspect_ratio`] inside the box it is
given, so it never stretches.

```rust
pub struct HeatExchangerVisual {
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
  pub fn new(physics: HeatExchanger, screen_position: Pos2, screen_vector: Vec2) -> Self { /* ... */ }
  ```
  Wrap a [`HeatExchanger`] with the given screen geometry.

- ```rust
  pub fn from_scalars(kind: HeatExchangerKind, screen_position: Pos2, screen_vector: Vec2, range: HeatExchangerDisplayRange, scalars: HeatExchangerScalars) -> Self { /* ... */ }
  ```
  Build a heat-exchanger visual from the caller's own scalar plant state.

- ```rust
  pub fn with_kind(self: Self, kind: HeatExchangerKind) -> Self { /* ... */ }
  ```
  Draw a different flow arrangement with the same state.

- ```rust
  pub fn with_construction(self: Self, construction: HeatExchangerConstruction) -> Self { /* ... */ }
  ```
  Draw a different construction with the same state.

- ```rust
  pub fn with_surface(self: Self, area: Area, overall_coefficient: HeatTransfer) -> Self { /* ... */ }
  ```
  Label the body with a known heat-transfer area (square metres) and

- ```rust
  pub fn without_labels(self: Self) -> Self { /* ... */ }
  ```
  Turn the internal component labels off — for thumbnails.

- ```rust
  pub fn kind(self: &Self) -> HeatExchangerKind { /* ... */ }
  ```
  Which flow arrangement this visual draws.

- ```rust
  pub fn construction(self: &Self) -> HeatExchangerConstruction { /* ... */ }
  ```
  Which construction this visual draws.

- ```rust
  pub fn size(self: &Self) -> Vec2 { /* ... */ }
  ```
  On-screen size of the box the artwork letterboxes into, in points.

- ```rust
  pub fn state(self: &Self) -> &HeatExchangerVisualState { /* ... */ }
  ```
  Where this visual gets its state.

- ```rust
  pub fn scalars(self: &Self) -> Option<HeatExchangerScalars> { /* ... */ }
  ```
  The scalar state the artwork is drawn from, or `None` on the

- ```rust
  pub fn heat_transfer_area(self: &Self) -> Option<Area> { /* ... */ }
  ```
  The heat-transfer area, if one is known, in `uom` [`Area`].

- ```rust
  pub fn overall_coefficient(self: &Self) -> Option<HeatTransfer> { /* ... */ }
  ```
  The overall heat-transfer coefficient `U`, if one is known, in `uom`

- ```rust
  pub fn duty(self: &Self) -> Option<Power> { /* ... */ }
  ```
  The heat duty the caller supplied, or `None` on the physics-backed path

- ```rust
  pub fn approaches(self: &Self) -> Option<(TemperatureInterval, TemperatureInterval)> { /* ... */ }
  ```
  The two terminal approaches, or `None` on the physics-backed path.

- ```rust
  pub fn verdict(self: &Self) -> Option<ApproachVerdict> { /* ... */ }
  ```
  What the supplied temperatures imply about the arrangement, or `None` on

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws the exchanger for [`HeatExchangerVisual::construction`], plumbed

- **WithSubscriber**
### Functions

#### Function `path_fractions`

How far along **its own path** each stream is, at drawn length position `s`.

Returns `(hot, cold)`, both dimensionless fractions in `[0, 1]` where `0.0`
is that stream's own inlet and `1.0` its own outlet. `s` is a position across
the drawn body, `0.0` at the left edge and `1.0` at the right edge, clamped.

The hot stream is always drawn left to right, so its fraction is simply `s`.
The cold stream depends on the arrangement, and this is the single function
that encodes the difference:

- [`HeatExchangerKind::ParallelFlow`]: the cold stream also runs left to
  right, so its fraction is `s` too — both streams are at their inlets at the
  same end.
- [`HeatExchangerKind::CounterFlow`]: the cold stream runs right to left, so
  its fraction is `1 - s` — at the left edge it is at its **outlet**, beside
  the hot stream's inlet.

Colouring each stream between these fractions is what makes the two profiles
converge (or not) along the body.

```rust
pub fn path_fractions(kind: HeatExchangerKind, s: f32) -> (f32, f32) { /* ... */ }
```

#### Function `terminal_approaches`

The two **terminal approaches**, as `(left_end, right_end)`.

A terminal approach is the temperature difference between the two streams at
one end of the exchanger — the driving force there. Both are returned as
signed [`TemperatureInterval`]s (`hot - cold` at that end), because the sign
is the whole diagnostic: a negative approach means heat would have to flow
from the cold stream to the hot one at that end, which cannot happen.

The pair returned is exactly the `(dt1, dt2)` that
[`outram_park_fork_dwsim_libs::heat_exchanger::lmtd::lmtd`] forms for the
same arrangement, which is why the drawing's end-brackets can be read as the
two ends of the log-mean:

- [`HeatExchangerKind::CounterFlow`]: `(T_hot_in - T_cold_out,
  T_hot_out - T_cold_in)`.
- [`HeatExchangerKind::ParallelFlow`]: `(T_hot_in - T_cold_in,
  T_hot_out - T_cold_out)`.

No log-mean is taken here and no duty is computed — this is the geometry the
end brackets are drawn from, not a rating.

```rust
pub fn terminal_approaches(kind: HeatExchangerKind, hot_inlet_temp: uom::si::f64::ThermodynamicTemperature, hot_outlet_temp: uom::si::f64::ThermodynamicTemperature, cold_inlet_temp: uom::si::f64::ThermodynamicTemperature, cold_outlet_temp: uom::si::f64::ThermodynamicTemperature) -> (uom::si::f64::TemperatureInterval, uom::si::f64::TemperatureInterval) { /* ... */ }
```

#### Function `approach_verdict`

Classify the caller's four temperatures against the arrangement — see
[`ApproachVerdict`].

Non-finite inputs give [`ApproachVerdict::Impossible`]: a NaN must be the
most visible outcome on screen rather than hiding behind a plausible label.

```rust
pub fn approach_verdict(kind: HeatExchangerKind, hot_inlet_temp: uom::si::f64::ThermodynamicTemperature, hot_outlet_temp: uom::si::f64::ThermodynamicTemperature, cold_inlet_temp: uom::si::f64::ThermodynamicTemperature, cold_outlet_temp: uom::si::f64::ThermodynamicTemperature) -> ApproachVerdict { /* ... */ }
```

#### Function `profile_temperature_bounds`

The temperature window the profile strip is plotted against, as
`(bottom, top)`.

Scaled to the **four terminal temperatures**, not to the display range: the
colour scale is usually set for a whole plant, and plotting a 20 K approach
against a 600 K scale would draw two flat lines on top of each other. A 12 %
margin is added at both ends so the extreme profiles do not sit on the strip
border, and the window is never narrower than **1 K**, so a degenerate state
(all four temperatures equal, which is what zero duty gives) still draws a
strip rather than collapsing to a line or dividing by zero.

Colour is unaffected — every colour in the artwork, profile strip included,
still comes from the caller's [`HeatExchangerDisplayRange`].

```rust
pub fn profile_temperature_bounds(scalars: &HeatExchangerScalars) -> (uom::si::f64::ThermodynamicTemperature, uom::si::f64::ThermodynamicTemperature) { /* ... */ }
```

#### Function `lerp_temperature`

Linear interpolation between two temperatures, in kelvin.

`t` is a dimensionless position along whatever path is being coloured,
clamped to `[0, 1]`. **This is a display interpolation, not physics**: the
real temperature profile along a recuperator is exponential in position, and
computing it is rating work that belongs in `tampines`, not in this
presentation crate. The endpoints are exact; the path between them is drawn
straight and is documented as such.

```rust
pub fn lerp_temperature(from: uom::si::f64::ThermodynamicTemperature, to: uom::si::f64::ThermodynamicTemperature, t: f32) -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

### Constants and Statics

#### Constant `SHELL_AND_TUBE_ASPECT_RATIO`

Width-to-height ratio a shell-and-tube exchanger is drawn at, dimensionless.

**Chosen by eye, not taken from a design.** A shell-and-tube unit is long and
slim — the tubes have to be long enough to transfer the duty, and the shell
is only as tall as the bundle plus its baffle clearance — and 2.05 : 1 is
enough to read that way while still leaving a band under the body for the
temperature-profile strip. See the module's "What this is not" section.

```rust
pub const SHELL_AND_TUBE_ASPECT_RATIO: f32 = 2.05;
```

#### Constant `PLATE_FRAME_ASPECT_RATIO`

Width-to-height ratio a plate-and-frame exchanger is drawn at,
dimensionless.

**Chosen by eye, not taken from a design.** A plate pack seen edge-on is much
squatter than a shell — a great many thin channels stacked between two heavy
end frames — so 1.35 : 1 reads as a different machine at a glance in a
gallery, which is the point of drawing the second construction at all.

```rust
pub const PLATE_FRAME_ASPECT_RATIO: f32 = 1.35;
```

## Module `htr10_reactor_vessel`

Visual HTR-10 (pebble-bed high-temperature gas-cooled reactor) vessel.

Built on the same pattern as
[`crate::components::fhr_reactor_vessel::FhrReactorVesselVisual`] — a
scalar-fed cut-away that colours each region from a temperature the caller
supplies — but the geometry is HTR-10's, not the FHR's, and the differences
are physical rather than cosmetic.

# How HTR-10 differs from the FHR

Both are pebble beds, but almost nothing else lines up:

- **Flow direction is reversed.** Cold helium enters the vessel and rises
  through channels *inside the side reflector*, reverses at the top of the
  core, and flows **downward** through the pebble bed into a hot gas
  chamber in the bottom reflector. The FHR's salt rises through its bed.
- **Control rods sit in the side reflector**, not in the bed — ten borings
  in the graphite around the active core, entered from the vessel head.
- **The bed drains through a cone into a central discharge tube** that
  penetrates the lower head, because fuel circulates multi-pass and is
  assayed for burnup before being returned to the top.
- **Heat leaves sideways**, through a hot gas duct nozzle low on the vessel
  wall, to a separate steam-generator vessel standing beside the reactor.
- **The vessel is a capsule** — a cylindrical shell closed by domed heads —
  rather than a squared-off box.

# Provenance of the geometry

Proportions follow the HTR-10 reactor vertical cross-section (Figure 4.6)
and the core-configuration and vessel-system descriptions in the IAEA
coordinated-research-programme report on HTGR performance, ingested into
this workspace's literature layer at
`crates/kovan-literature/open/reports/htr-10-iaea.json`.

Published dimensions used directly: reactor pressure vessel 4.2 m inner
diameter by 11.1 m high (which sets [`NATIVE_ASPECT_RATIO`]); pebble bed
1.8 m diameter by 1.97 m mean height; side reflector 1.0 m thick including
carbon bricks.

**This is schematic art, not a design drawing.** Feature positions are
proportioned by eye from the figure, not dimensioned from it, and nothing
here is a validated model. See `RESPONSIBLE_USE.md`.

```rust
pub mod htr10_reactor_vessel { /* ... */ }
```

### Types

#### Struct `Htr10ReactorVesselVisual`

Visual representation of the HTR-10 reactor vessel.

Scalar-fed and owns no physics, for the same reason the FHR vessel is: a
simulator already holds these temperatures in its own plant model.

All temperatures are absolute thermodynamic temperatures (`uom`-typed).
Control-rod insertion is dimensionless in `[0, 1]` — `0.0` fully withdrawn,
`1.0` fully inserted — clamped at render time so a transient overshoot from
a controller draws fully in or out rather than panicking.

# Control rods are drawn, not animated, here

The insertion fraction this widget holds is **where the rod is drawn**, not
where it has been commanded to go. Rod travel takes real time, so the
application slews the drawn fraction toward the commanded one with
[`crate::components::control_rod_drive::slewed_control_rod_insertion`] and
passes the result in. It cannot be done inside the widget: widgets here are
consumed by value and rebuilt on every repaint, so animation state living in
one would reset every frame and the rod would never move. Same rule, same
reason, as [`crate::animation::TracerTrain`].

```rust
pub struct Htr10ReactorVesselVisual {
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
  pub fn new(size: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature, pebble_temp: ThermodynamicTemperature, inlet_temp: ThermodynamicTemperature, outlet_temp: ThermodynamicTemperature, reflector_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build an HTR-10 vessel visual.

- ```rust
  pub fn size(self: &Self) -> Vec2 { /* ... */ }
  ```
  On-screen size, in points.

- ```rust
  pub fn set_control_rod_frac(self: &mut Self, frac: f32) { /* ... */ }
  ```
  Sets where the control-rod bank is **drawn**. Dimensionless `[0, 1]`:

- ```rust
  pub fn without_labels(self: Self) -> Self { /* ... */ }
  ```
  Turn the internal component labels off — for thumbnails.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws the HTR-10 cut-away: capsule pressure vessel, graphite reflector

- **WithSubscriber**
### Functions

#### Function `fit_native_aspect`

The largest sub-rectangle of `available` carrying [`NATIVE_ASPECT_RATIO`],
centred within it.

Same letterbox contract as the FHR vessel: the artwork keeps its
proportions at any size rather than stretching to fill its box.

```rust
pub fn fit_native_aspect(available: egui::Rect) -> egui::Rect { /* ... */ }
```

### Constants and Statics

#### Constant `NATIVE_ASPECT_RATIO`

Width-to-height ratio the vessel is drawn at.

Taken from the published reactor pressure vessel dimensions — 4.2 m inner
diameter by 11.1 m high — so the silhouette carries the real slenderness of
the machine rather than an invented one.

```rust
pub const NATIVE_ASPECT_RATIO: f32 = _;
```

## Module `instrumentation`

Visual instrumentation (gauge/readout).

**No backing physics type yet.** `nee_soon` (the intended source for an
instrumentation physics type) is currently a scaffold with a single
empty `NeeSoon {}` struct -- no dedicated instrumentation type exists
there to wrap. This is a visual-only placeholder that displays a caller-
supplied label and value; once `nee_soon` exposes a real type, this
struct should gain a `physics` field the same way every other visual
component here does.

```rust
pub mod instrumentation { /* ... */ }
```

### Types

#### Struct `InstrumentationVisual`

Visual placeholder for an instrument readout (no `nee_soon` physics type
to wrap yet -- see this module's doc). Displays a fixed label and value
string supplied by the caller, rather than reading live physics state.

```rust
pub struct InstrumentationVisual {
    pub screen_position: egui::Pos2,
    pub label: String,
    pub value: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `screen_position` | `egui::Pos2` | On-screen anchor position. |
| `label` | `String` | Instrument label (e.g. `"T_core"`). |
| `value` | `String` | Instrument value, pre-formatted by the caller (e.g. `"573.15 K"`). |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(screen_position: Pos2, label: impl Into<String>, value: impl Into<String>) -> Self { /* ... */ }
  ```
  Construct a placeholder instrumentation visual with the given label

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Minimal-static rendering: a text label showing `"{label}: {value}"`.

- **WithSubscriber**
## Module `pebble_packing`

Baked **pebble-bed packing artwork** for the reactor-vessel widgets.

A single settled, cut-away pebble packing, computed **once** offline and
committed here as a `const` table so widget painting costs nothing at
runtime. Paint [`PACKED_PEBBLES`] **in order**; **never** regenerate a
packing at runtime.

Each entry is a whole **sphere centre** `(x, y, z)`, not a flat cut. The
bed is monodisperse, so there is no per-pebble radius: every pebble draws
at [`SPHERE_RADIUS`]. What varies is `z`, how far the pebble sits *behind*
the cut plane — which is what lets a widget draw a bed with depth (overlap,
shading, slight foreshortening) instead of a flat slice.

# How it was generated

| | |
|---|---|
| Generator | `crates/outram-park-fork-liggghts/examples/bake_pebble_packing.rs` |
| Engine | `outram-park-fork-liggghts` `DemSimulation` (soft-sphere DEM, velocity-Verlet, linked-cell neighbours) |
| Contact model | `ContactModel::Hooke` — linear spring-dashpot, `k_n = 1.0e6 N/m`, `γ_n = 2500 N·s/m`, `k_t = 8.0e5 N/m`, `γ_t = 2500 N·s/m`, `μ = 0.4` |
| Integration | `dt = 1.0e-4 s`, **74000 steps** (7.40 s simulated) |
| Spheres settled (3-D) | **2525** monodisperse, radius `0.075 R`, graphite density 1750 kg/m³ |
| Solid fraction (interior control volume) | **0.6112** |
| Solid fraction (whole filled vessel) | **0.5751** |
| Reference (monodisperse RCP, Scott & Kilgour 1969) | 0.6366 |
| Residual motion | over a final 0.5 s window: **3.7 %** of a pebble radius rms, 20.2 % worst case; residual kinetic energy `1.2e-4 J` per pebble |
| Depth window kept | `-0.3 <= z <= 0` — 2.0 pebble diameters behind the cut plane |
| Pebbles in this baked window | **523** |
| Vessel silhouette they cover | **91.9 %** |
| Generator wall clock | 169 s |
| Baked on | 2026-08-06 |

# ⚠️ Artwork data, NOT a validated physics result

`outram-park-fork-liggghts` is a **scaffold** crate with no human V&V.
These coordinates exist so an offline demonstration GUI can draw a
believable cut-away pebble bed — pebbles resting on one another rather
than floating on a jittered lattice. They are **not** a validated packing
prediction, must not be cited as one, and must not inform any facility,
licensing, safety, or operational decision. The measured solid fraction is
recorded above precisely so a reader can see how far it sits from the
literature value instead of having to trust it.

One known limitation is worth stating outright, because it bounds what
"settled" can mean here. The DEM engine's tangential contact is
**history-free** (its own `simulation` module documents this): it carries
no accumulated tangential spring between steps, so it has a
Coulomb-capped dashpot but **no static friction**. A grain resting on an
inclined contact therefore creeps at a small terminal velocity forever,
and a strict zero-velocity rest state is unreachable no matter how long
the run. The generator confirmed this by measuring two back-to-back
windows: the creep was steady, not decaying, while the local solid
fraction was unchanged between them. So the *structure* below is a
genuinely settled packing; the coordinates are a valid instantaneous
snapshot of it, and because the bake is a still image the residual creep
does not appear in it at all.

# Coordinate convention (read this before drawing)

Lengths are **normalised to the vessel barrel inner radius**, `R = 1`.
The origin sits **on the vessel axis, at the plane where the conical
bottom meets the cylindrical barrel**; `+x` is to the right and `+y` is
up. So the vessel outline the widget should draw is:

- **Barrel** — `|x| <= 1` for `0 <= y <= 2.2` ([`BARREL_HEIGHT`]).
- **Cone** — for `-0.9 <= y <= 0` ([`CONE_HEIGHT`]) the half-width
  tapers linearly from `0.18` ([`CHUTE_RADIUS`]) at the bottom to `1`
  at `y = 0`. Use [`vessel_half_width`].

# Which way `z` points — get this backwards and the bed draws inside-out

The frame is right-handed, so with `+x` right and `+y` up, **`+z` points
out of the screen, toward the viewer**. The bed was sawn open on the
vertical plane `z = 0` and the half in front of it (`z > 0`, between the
cut and the viewer) was thrown away, which is what makes the interior
visible. So:

- **every baked `z` is negative or zero** — the pebbles recede *into* the
  screen, away from the viewer;
- `z = 0` is the **nearest** pebble, sitting on the cut face;
- `z = -`[`DEPTH_WINDOW`] is the **farthest** pebble kept.

A renderer that treats `z` as growing away from the viewer will shade the
near pebbles as if they were far and paint them in the wrong order — the
bed will look hollow rather than solid.

# Painting order — the table is already sorted for you

[`PACKED_PEBBLES`] is sorted **back to front** (`z` ascending: most
negative, i.e. farthest, first). Paint it straight through in the order
given, first entry first, and the painter's algorithm does the occlusion
for you — each nearer pebble covers the ones behind it, with no depth
buffer and no per-frame sorting. Do **not** reorder the table (e.g. by
`y`) unless you are prepared to re-sort by `z` before drawing.

# Why only a window of depth

Only the first few pebble layers behind the cut are visible; the rest are
occluded. Baking the whole half-bed would therefore cost draw calls for
pixels nobody sees, and each pebble carries a TRISO speckle of order 50
dots, so the circle count is ~50x the pebble count. The window was chosen
from a measured sweep in the generator (retained count versus the fraction
of the vessel silhouette actually covered); the numbers for the baked
choice are in the table above.

# Drawing it

```
use outram_park_digital_twin_engine::components::pebble_packing::{
    depth_fraction, BARREL_HEIGHT, CONE_HEIGHT, PACKED_PEBBLES, SPHERE_RADIUS,
};

// Map the bed's normalised box onto a screen rect, y flipped (screen y
// grows downward), preserving aspect ratio via a single scale factor.
let (rect_x, rect_y, rect_w) = (10.0_f32, 10.0_f32, 120.0_f32);
let scale = rect_w / 2.0; // the barrel spans x in [-1, 1]
let top_y = rect_y; // screen y of the bed coordinate y = BARREL_HEIGHT

// Already sorted farthest-first: just paint straight through.
for pebble in PACKED_PEBBLES {
    let cx = rect_x + rect_w / 2.0 + pebble.x * scale;
    let cy = top_y + (BARREL_HEIGHT - pebble.y) * scale;
    let cr = SPHERE_RADIUS * scale; // one radius for every pebble
    // 0 at the back of the window, 1 on the cut face: darken the far ones.
    let lit = 0.45 + 0.55 * pebble.depth();
    let _ = (cx, cy, cr, lit); // paint a filled circle here
}

assert!((depth_fraction(0.0) - 1.0).abs() < 1e-6); // the cut face is nearest
let _total_height = BARREL_HEIGHT + CONE_HEIGHT;
```

```rust
pub mod pebble_packing { /* ... */ }
```

### Types

#### Struct `PackedPebble`

One pebble in the baked cut-away bed — a whole **sphere centre**.

All three fields are in the normalised vessel frame documented at the
module level: barrel inner radius `R = 1`, origin on the vessel axis at
the cone/barrel junction, `+x` right, `+y` up, `+z` **toward the viewer**.
There is no radius field — the bed is monodisperse, so every pebble draws
at [`SPHERE_RADIUS`].

```rust
pub struct PackedPebble {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f32` | Horizontal centre coordinate, in vessel radii. `x = 0` is the axis. |
| `y` | `f32` | Vertical centre coordinate, in vessel radii. `y = 0` is the<br>cone/barrel junction; `+y` is up. |
| `z` | `f32` | Depth centre coordinate, in vessel radii — how far the pebble sits<br>**behind the cut plane**, so `-`[`DEPTH_WINDOW`]` <= z <= 0`. `z = 0`<br>is nearest the viewer (on the cut face) and more negative is farther<br>away. For shading, prefer [`PackedPebble::depth`] over raw `z`. |

##### Implementations

###### Methods

- ```rust
  pub const fn new(x: f32, y: f32, z: f32) -> Self { /* ... */ }
  ```
  Construct a pebble from its normalised centre `(x, y, z)`.

- ```rust
  pub fn depth(self: &Self) -> f32 { /* ... */ }
  ```
  This pebble's dimensionless depth cue in `[0, 1]` — `0` at the back of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PackedPebble { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PackedPebble) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
### Functions

#### Function `depth_fraction`

**Attributes:**

- `MustUse { reason: None }`

Map a pebble's depth `z` to a dimensionless fraction in `[0, 1]`:
`0` at the far edge of the baked window, `1` on the cut face nearest the
viewer. Values outside the window clamp.

**This is a display cue, not physics.** It carries no units and means
nothing thermally, neutronically, or mechanically — it exists so a widget
can shade, tint, or slightly shrink a pebble by how far back it sits
without having to know [`DEPTH_WINDOW`] itself. Typical use: multiply a
base colour's brightness by `0.45 + 0.55 * depth`, so the back of the bed
falls into shadow and the cut face reads as lit.

Monotone non-decreasing in `z`, so ordering the table by `z` (as it is
baked) also orders it by this fraction.

```rust
pub fn depth_fraction(z: f32) -> f32 { /* ... */ }
```

#### Function `vessel_half_width`

**Attributes:**

- `MustUse { reason: None }`

Inner half-width of the vessel outline at height `y`, in vessel radii.

This is the silhouette the widget should stroke around the pebbles: `1`
throughout the barrel (`y >= 0`), tapering linearly to [`CHUTE_RADIUS`] at
the bottom of the cone (`y = -`[`CONE_HEIGHT`]). Outside the vessel
(`y < -CONE_HEIGHT`) it clamps to [`CHUTE_RADIUS`].

```rust
pub fn vessel_half_width(y: f32) -> f32 { /* ... */ }
```

### Constants and Statics

#### Constant `SPHERE_RADIUS`

Radius of every packed pebble, in vessel radii (`0.075 R`).

The bed is monodisperse, so this one value is the drawn radius of every
entry in [`PACKED_PEBBLES`] — there is no per-pebble radius to look up.
(An earlier bake stored a per-pebble *chord* radius from a strict flat cut;
it drew as a distracting mix of large and tiny circles and was replaced by
this depth-window bake.)

```rust
pub const SPHERE_RADIUS: f32 = 0.075;
```

#### Constant `DEPTH_WINDOW`

Depth of the baked slab behind the cut plane, in vessel radii.

Every entry in [`PACKED_PEBBLES`] has `-DEPTH_WINDOW <= z <= 0`. This is
2.0 pebble diameters — deep enough that overlapping pebbles read as a solid
bed with depth, shallow enough that the widget is not paying to draw
pebbles the front layers occlude. See the module docs for the measured
count/coverage trade behind the number.

```rust
pub const DEPTH_WINDOW: f32 = 0.3;
```

#### Constant `DEPTH_BOUNDS`

Measured `[min_z, max_z]` of the baked pebble centres, in vessel radii.

Both lie inside `[-`[`DEPTH_WINDOW`]`, 0]` by construction; this records
where the data actually landed, which is not exactly the window bounds
because it is a finite sample of discrete sphere centres.

```rust
pub const DEPTH_BOUNDS: [f32; 2] = _;
```

#### Constant `BARREL_HEIGHT`

Height of the cylindrical barrel above the cone junction, in vessel radii.

```rust
pub const BARREL_HEIGHT: f32 = 2.2;
```

#### Constant `CONE_HEIGHT`

Height of the conical bottom below the cone junction, in vessel radii.
The cone occupies `-CONE_HEIGHT <= y <= 0`.

```rust
pub const CONE_HEIGHT: f32 = 0.9;
```

#### Constant `CHUTE_RADIUS`

Radius of the discharge chute at the very bottom of the cone, in vessel
radii. The bed rests on a plug at that level (no discharge is modelled).

```rust
pub const CHUTE_RADIUS: f32 = 0.18;
```

#### Constant `BED_TOP`

Height of the top of the settled bed, in vessel radii — measured from the
full 3-D packing (the top edge of its highest sphere), not assumed.

This is the bed's free-surface level, so it is the right thing to compare
a fill-level indicator against. It is an upper bound for every pebble in
[`PACKED_PEBBLES`] (the depth window may not contain the tallest sphere,
so the window's own top, [`BED_BOUNDS`]`[3]`, can be slightly lower).

```rust
pub const BED_TOP: f32 = 2.18117;
```

#### Constant `BED_BOUNDS`

Tight bounding box of the baked pebbles as drawn, in the plane of the
screen: `[min_x, max_x, min_y, max_y]`, each centre expanded by
[`SPHERE_RADIUS`]. Measured from the data below. For the out-of-plane
extent see [`DEPTH_BOUNDS`].

```rust
pub const BED_BOUNDS: [f32; 4] = _;
```

#### Constant `PACKED_PEBBLES`

The baked packing: 523 sphere centres from the settled pebble bed, taken
from the slab just behind the cut plane and **sorted back to front**
(`z` ascending — farthest first).

Paint them in this order and the painter's algorithm handles occlusion
for you. Every pebble draws at [`SPHERE_RADIUS`]; use
[`PackedPebble::depth`] for the depth shading.

See the module documentation for the coordinate convention, the `z`
sign convention, and the honest-scope caveat (artwork, not validated
physics).

```rust
pub const PACKED_PEBBLES: &[PackedPebble] = _;
```

## Module `pipe`

Visual pipe.

Renders a pipe run as a coloured line whose colour comes from the fluid
temperature and whose optional tracer marks come from the mass flow --
the crate's core "rendering derives directly from physics state" idea,
applied to the component that carries most of a plant schematic's flow.

## Two ways to supply the state

[`PipeVisualState`] is an enum (not a trait object, per the workspace's
mandatory design rules) with two variants, because digital-twin
applications reach this widget from two directions:

- [`PipeVisualState::Physics`] wraps a full
  [`tampines::components::Pipe`]. The pipe's per-cell temperature profile
  is read straight off its flow backend, so the run is drawn as one
  coloured segment **per finite-volume cell** -- cell count drives
  displayed cells, cell temperature drives cell colour.
- [`PipeVisualState::Scalars`] takes a [`PipeScalars`] triple
  (temperature, mass flow, residence time) directly. A
  `tampines::components::Pipe` can only be built around a
  `SinglePhaseFluidArray` or `CompressibleFluidArray`, which is a heavy
  object to stand up for what may be a short connector line between two
  pieces of equipment. Simulators whose loop physics is their own lumped
  model (rather than a TAMPINES fluid array) supply that model's scalars
  here and still get correct colour and tracer motion.

The scalar variant is *not* a placeholder for missing physics: the caller
is expected to pass real state from its own model. It is a narrower
interface, not a fabricated one.

```rust
pub mod pipe { /* ... */ }
```

### Types

#### Struct `PipeScalars`

Scalar fluid state for a pipe run whose physics is not a
[`tampines::components::Pipe`].

Every field is the caller's own real model state -- see
[`PipeVisualState::Scalars`] for why this narrower interface exists.

```rust
pub struct PipeScalars {
    pub temperature: uom::si::f64::ThermodynamicTemperature,
    pub mass_flow: uom::si::f64::MassRate,
    pub residence_time: uom::si::f64::Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | Bulk fluid temperature of the run, used for the colour map. |
| `mass_flow` | `uom::si::f64::MassRate` | Mass flow through the run. Positive is `screen_position` -><br>`screen_position + screen_vector`; negative runs tracers in reverse. |
| `residence_time` | `uom::si::f64::Time` | End-to-end residence time, setting how long a tracer mark takes to<br>cross the run (see [`crate::animation::residence_time_from_flow`]). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PipeScalars { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `PipeVisualState`

Where a [`PipeVisual`] gets the physics it renders.

Enum dispatch, not a trait object, per the workspace's mandatory
"no trait objects" Rust design rule.

```rust
pub enum PipeVisualState {
    Physics(tampines::components::Pipe),
    Scalars(PipeScalars),
}
```

##### Variants

###### `Physics`

Backed by a full TAMPINES pipe, drawn one coloured segment per
finite-volume cell.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines::components::Pipe` |  |

###### `Scalars`

Backed by caller-supplied scalars, drawn as a single coloured run.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PipeScalars` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PipeVisualState { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `PipeScale`

How plant-space metres map to screen points.

The two scales are deliberately **separate**. A pipe run is metres long and
its bore is millimetres across; a single scale would render every pipe as an
invisible hairline. Keeping them apart lets length stay a true scale drawing
while the cross-section is exaggerated enough to see.

That exaggeration is honest but must be stated: two pipes drawn side by side
have *lengths* in true proportion to each other and *cross-sections* in true
proportion to each other, but thickness is not on the same scale as length.

```rust
pub struct PipeScale {
    pub points_per_metre: f32,
    pub points_per_square_metre: f32,
    pub min_thickness_points: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points_per_metre` | `f32` | Screen points per metre of pipe length. |
| `points_per_square_metre` | `f32` | Screen points of drawn thickness per square metre of flow<br>cross-sectional area.<br><br>Thickness is proportional to **area**, not diameter, so a pipe carrying<br>four times the flow area draws four times as thick. |
| `min_thickness_points` | `f32` | Floor on drawn thickness, in points, so a small-bore pipe stays visible<br>rather than vanishing. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PipeScale { /* ... */ }
    ```

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
    Chosen so a 3 m run of 50 mm bore draws about 240 points long and

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PipeScale) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `PipePhaseShade`

How the working fluid's phase is reflected in the drawing.

Enum dispatch per the workspace's "no trait objects" rule.

```rust
pub enum PipePhaseShade {
    Liquid,
    Gas,
    TwoPhase,
}
```

##### Variants

###### `Liquid`

Liquid: full-strength colour.

###### `Gas`

Gas or vapour: lightened. A gas is orders of magnitude less dense than
the liquid at the same temperature, and washing the colour out is the
cheapest way to make that legible at a glance without adding a second
colour axis the reader has to learn.

###### `TwoPhase`

Two-phase, where the backend carries phase information but this widget
is not yet reading a per-cell quality from it: drawn between the two.

##### Implementations

###### Methods

- ```rust
  pub fn apply(self: Self, c: Color32) -> Color32 { /* ... */ }
  ```
  Apply this shade's lightening to a colour.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PipePhaseShade { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PipePhaseShade) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `PipeVisual`

Visual representation of a pipe run.

`screen_vector` gives the pipe's on-screen direction and length (from
`screen_position` to `screen_position + screen_vector`), which is also the
direction a positive mass flow's tracers travel.

```rust
pub struct PipeVisual {
    pub state: PipeVisualState,
    pub screen_position: egui::Pos2,
    pub screen_vector: egui::Vec2,
    pub min_temp: uom::si::f64::ThermodynamicTemperature,
    pub max_temp: uom::si::f64::ThermodynamicTemperature,
    pub scale: PipeScale,
    pub wall_alarm_temp: Option<uom::si::f64::ThermodynamicTemperature>,
    pub mark_at: Option<f64>,
    pub tracer: Option<crate::animation::TracerTrain>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `state` | `PipeVisualState` | The physics state this widget renders. |
| `screen_position` | `egui::Pos2` | On-screen anchor position (the inlet endpoint of the pipe). |
| `screen_vector` | `egui::Vec2` | On-screen direction and length, from `screen_position` to the outlet. |
| `min_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature drawn in the coldest displayable colour (blue).<br><br>The map is diverging, so the MIDPOINT of `[min_temp, max_temp]` is the<br>neutral white point -- set the range symmetrically about a meaningful<br>reference, not just to the extremes observed. |
| `max_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature mapped to `hotness = 1.0` (hottest displayable colour). |
| `scale` | `PipeScale` | Metres-to-points mapping for length and cross-section. |
| `wall_alarm_temp` | `Option<uom::si::f64::ThermodynamicTemperature>` | Metal temperature at or above which the pipe wall is drawn **red**.<br><br>`None` (the default) never reddens. There is deliberately no built-in<br>limit: an allowable metal temperature depends on the material, the<br>code of construction and the duty, so the caller must supply the one<br>that applies rather than inheriting a number invented here. |
| `mark_at` | `Option<f64>` | A single tracer mark at an explicit position in `[0, 1]`, `0` = inlet.<br><br>Set from [`crate::animation::TracerPulse`], which shows one mark at a<br>time and reports `None` between releases — a train of marks strobes on<br>a short or fast run. Independent of [`Self::tracer`]; both may be set. |
| `tracer` | `Option<crate::animation::TracerTrain>` | Optional flow-tracer marks drawn along the run.<br><br>The train is *advanced by the application*, once per frame, and copied<br>in here at widget-build time -- widgets are rebuilt every repaint, so a<br>train owned by the widget would reset its phase to zero each frame.<br>See [`crate::animation`] for the ownership rationale. |

##### Implementations

###### Methods

- ```rust
  pub fn new(physics: Pipe, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Wrap a [`Pipe`] with the given screen geometry and colour-mapping

- ```rust
  pub fn from_scalars(scalars: PipeScalars, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a pipe run from caller-supplied [`PipeScalars`] rather than a

- ```rust
  pub fn with_mark_at(self: Self, position: f64) -> Self { /* ... */ }
  ```
  Place a single tracer mark at `position` in `[0, 1]` along the run.

- ```rust
  pub fn with_wall_alarm(self: Self, alarm: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Set the metal temperature at or above which the wall is drawn red.

- ```rust
  pub fn with_scale(self: Self, scale: PipeScale) -> Self { /* ... */ }
  ```
  Override the metres-to-points mapping. Builder-style.

- ```rust
  pub fn with_tracer(self: Self, tracer: TracerTrain) -> Self { /* ... */ }
  ```
  Attach an application-owned [`TracerTrain`] so this run draws flow

- ```rust
  pub fn cross_sectional_area(self: &Self) -> Option<Area> { /* ... */ }
  ```
  Flow cross-sectional area, from the pipe's bore.

- ```rust
  pub fn run_length(self: &Self) -> Option<Length> { /* ... */ }
  ```
  Physical run length, `None` for scalar-backed runs (no geometry).

- ```rust
  pub fn inclination(self: &Self) -> Option<Angle> { /* ... */ }
  ```
  Inclination from horizontal, positive uphill. `None` for scalar runs.

- ```rust
  pub fn wall_temperatures(self: &Self) -> Option<Vec<ThermodynamicTemperature>> { /* ... */ }
  ```
  Per-cell **metal wall** temperatures along the run, inlet -> outlet.

- ```rust
  pub fn peak_wall_temperature(self: &Self) -> Option<ThermodynamicTemperature> { /* ... */ }
  ```
  Hottest metal wall temperature on the run, if the backend reports one.

- ```rust
  pub fn phase_shade(self: &Self) -> PipePhaseShade { /* ... */ }
  ```
  How this backend's fluid phase is shaded.

- ```rust
  pub fn drawn_size(self: &Self) -> (f32, f32) { /* ... */ }
  ```
  Drawn size in screen points: `(length, thickness)`.

- ```rust
  pub fn cell_temperatures(self: &Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  Per-cell fluid temperatures along the run, inlet -> outlet.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws the run as a rectangle divided into one box per finite-volume

- **WithSubscriber**
## Module `pump`

Visual **pump**.

Three machines that all raise the pressure of a liquid, drawn as three
genuinely different pieces of artwork because they are three genuinely
different machines — see [`PumpKind`]. Each carries its own
[`PumpKind::native_aspect_ratio`] and letterboxes to it, so a squat volute
stays squat and a vertical canned-rotor pump stays slender no matter what
box the caller hands it.

## What actually turns, and what drives it

The rotating element is drawn at `theta = omega * t`, where `omega` is the
shaft angular velocity the caller supplies and `t` is the elapsed
**simulation** time. It is not an animation constant and it is not read from
a wall clock inside the widget.

Like [`crate::animation::TracerTrain`] and
[`crate::components::TurbineVisual`], the clock is **application-owned**.
Visual components here are consumed by value and rebuilt on every repaint,
so a clock owned by the widget would reset to zero each frame and the
impeller would never turn. The application advances its own
[`uom::si::f64::Time`] and passes it in via [`PumpVisual::at_time`].

A consequence worth stating because it is a *feature*: a pump given zero
shaft speed draws a **stationary but complete** impeller — every vane in its
place, simply not moving. A stopped pump must look stopped, not look
broken and not disappear. `stopped_pump_keeps_a_full_stationary_impeller`
pins that.

## Where the state comes from

**Scalar-fed, deliberately.** [`tampines::components::Pump`] carries an
operating-point specification and an efficiency, but its `evaluate` returns
`TampinesError::NotYetImplemented` — there is no outlet state, no head, no
shaft speed and no fluid temperature to read off it. Rather than fabricate
those, this widget takes the two scalars it draws (shaft angular velocity,
fluid temperature) directly from the caller, exactly as
[`crate::components::PipeVisual::from_scalars`] does, and treats a wrapped
`Pump` as optional API compatibility rather than a state source. When
`Pump::evaluate` lands, the head/flow it returns is what should size the
discharge and grade the passage, and this module should compose it.

Fluid temperature is optional. `None` renders the passages in neutral grey,
which is the honest drawing of "not known" and is visibly distinct from any
point on the colour scale. When a temperature *is* supplied it goes through
the shared [`crate::components::temperature_colour`] map, so a pump grades
identically to every other widget in this library.

## Determinism

The cast-surface stipple on the casings is hashed from its own index (see
[`pump_hash`]), never drawn from a random source. The widget is rebuilt
every repaint, so a real random draw would make the casings crawl with
shimmer frame to frame.

## Status

**Offline demonstration artwork, not a validated model and not a design
drawing.** Proportions are chosen for legibility on screen; nothing here is
dimensioned from, or represents, any specific pump. Per
`RESPONSIBLE_USE.md` this is for education, research and V&V only — not for
facility operation, reactor control, or safety-critical decisions.

```rust
pub mod pump { /* ... */ }
```

### Types

#### Enum `PumpKind`

Which kind of pump is drawn.

Enum dispatch, not a trait object, per the workspace's mandatory "no trait
objects" Rust design rule: the set of pump architectures a reactor
schematic needs is closed and small, and an exhaustive `match` makes adding
one a compile error at every site rather than a runtime surprise.

The three here are genuinely different machines, not three skins:

| Kind | View drawn | Silhouette |
|---|---|---|
| [`PumpKind::Centrifugal`] | face-on, along the shaft | roughly square volute with a vertical discharge |
| [`PumpKind::VerticalCannedRotor`] | side elevation | tall and slender: motor stacked above the casing |
| [`PumpKind::AxialPropeller`] | side elevation | wide: a propeller in a straight-through duct |

```rust
pub enum PumpKind {
    Centrifugal,
    VerticalCannedRotor,
    AxialPropeller,
}
```

##### Variants

###### `Centrifugal`

**Centrifugal, volute casing** — the general workhorse, drawn face-on
along the shaft.

Fluid enters axially at the suction eye in the centre, is flung
outwards by backswept vanes, is collected by a spiral volute whose area
grows in the direction of rotation, and leaves tangentially through the
discharge. The volute's job is to convert velocity head to pressure
head at roughly constant angular momentum, which is why the passage
must open out as it wraps.

Typical service in the scoped plants: feedwater and condensate pumps on
every Rankine secondary (`docs/reactor-scoping/`).

###### `VerticalCannedRotor`

**Vertical canned-rotor / glandless** — what a PWR reactor coolant pump
or a molten-salt pump actually is, drawn in side elevation.

The motor stacks directly above the casing on one shaft, and the whole
rotor runs *inside* the pressure boundary: the stator is sealed off
behind a thin can and the pumped fluid fills the motor cavity, cooling
and lubricating it. There is therefore **no rotating seal to
atmosphere** — which is the entire point for a radioactive primary
coolant or a molten salt, where a shaft seal is the leak path you cannot
accept. A flywheel above the casing extends the coastdown so flow decays
gracefully rather than stopping with the power.

Typical service: PWR/iPWR primary coolant pumps, the MSRE-style
sump/salt pump, EBR-II's submerged sodium pumps.

###### `AxialPropeller`

**Axial / propeller** — high flow at low head, drawn in side elevation.

A propeller in a straight-through duct: flow enters and leaves along the
axis with no radial turning, so there is no volute and no velocity head
to recover in a spiral. Stationary guide vanes downstream take the swirl
back out of the flow, which is where such pressure rise as there is
comes from. Few, broad, highly staggered blades — not the many short
blades of an axial turbine stage.

Typical service: circulating-water and pool-circulation duty, where the
volumetric flow is enormous and the head is a metre or two.

##### Implementations

###### Methods

- ```rust
  pub const fn native_aspect_ratio(self: Self) -> f32 { /* ... */ }
  ```
  Width-to-height ratio this kind's artwork is drawn at.

- ```rust
  pub fn fit_native_aspect(self: Self, available: Rect) -> Rect { /* ... */ }
  ```
  The largest sub-rectangle of `available` carrying this kind's

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short human-readable name, for gallery captions.

- ```rust
  pub fn description(self: Self) -> &'static str { /* ... */ }
  ```
  One-line description of what the machine is and how it works.

- ```rust
  pub fn typical_service(self: Self) -> &'static str { /* ... */ }
  ```
  Where this kind shows up in the reactors scoped under

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PumpKind { /* ... */ }
    ```

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
    fn default() -> PumpKind { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PumpKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `PumpVisual`

Visual representation of a pump.

Placement follows the convention shared by every widget in
[`crate::components`]: `screen_position` is the on-screen centre and
`screen_vector` the box size, so a pump can be placed absolutely on a
schematic canvas. The artwork then letterboxes inside that box to its
kind's [`PumpKind::native_aspect_ratio`].

Scalar-fed by design — see the module docs for why, and for what should
replace it once `tampines::components::Pump::evaluate` is implemented.

```rust
pub struct PumpVisual {
    pub kind: PumpKind,
    pub physics: Option<tampines::components::Pump>,
    pub screen_position: egui::Pos2,
    pub screen_vector: egui::Vec2,
    pub shaft_speed: uom::si::f64::AngularVelocity,
    pub simulation_time: uom::si::f64::Time,
    pub fluid_temperature: Option<uom::si::f64::ThermodynamicTemperature>,
    pub min_temp: uom::si::f64::ThermodynamicTemperature,
    pub max_temp: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `PumpKind` | Which machine to draw. |
| `physics` | `Option<tampines::components::Pump>` | The wrapped TAMPINES component, when the caller has one.<br><br>Carried for API compatibility and future composition **only**: it<br>contributes nothing to the drawing today, because `Pump::evaluate`<br>returns `NotYetImplemented` and the struct itself holds no fluid state,<br>no head and no shaft speed. Drawing anything from its efficiency or<br>specification would be fabricating a reading. |
| `screen_position` | `egui::Pos2` | On-screen centre position. |
| `screen_vector` | `egui::Vec2` | On-screen size of the whole machine, in points. |
| `shaft_speed` | `uom::si::f64::AngularVelocity` | Shaft angular velocity, positive in the drawn direction of rotation.<br><br>Screen coordinates run y-downwards, so a positive angular velocity is<br>drawn turning **clockwise on screen**, and the volute is wrapped the<br>same way so it always collects in the direction of rotation. Negative<br>values simply run the rotor the other way; zero draws it stationary. |
| `simulation_time` | `uom::si::f64::Time` | Elapsed simulation time, owned and advanced by the **application**.<br><br>Combined with [`PumpVisual::shaft_speed`] to give the rotor phase<br>`theta = omega * simulation_time`. See the module docs for why the<br>widget must not own this clock. |
| `fluid_temperature` | `Option<uom::si::f64::ThermodynamicTemperature>` | Temperature of the pumped fluid, if the caller knows one.<br><br>`None` draws the passages neutral grey rather than inventing a point on<br>the colour scale. |
| `min_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature mapped to the coldest displayable colour. |
| `max_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature mapped to the hottest displayable colour. |

##### Implementations

###### Methods

- ```rust
  pub fn new(physics: Pump, screen_position: Pos2, screen_vector: Vec2) -> Self { /* ... */ }
  ```
  Wrap a [`tampines::components::Pump`] with screen geometry.

- ```rust
  pub fn from_scalars(kind: PumpKind, screen_position: Pos2, screen_vector: Vec2, shaft_speed: AngularVelocity, simulation_time: Time, fluid_temperature: Option<ThermodynamicTemperature>, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a pump from the scalars it actually draws: shaft speed, the

- ```rust
  pub fn with_kind(self: Self, kind: PumpKind) -> Self { /* ... */ }
  ```
  Choose which machine to draw. Builder-style.

- ```rust
  pub fn with_shaft_speed(self: Self, shaft_speed: AngularVelocity) -> Self { /* ... */ }
  ```
  Set the shaft angular velocity. Builder-style.

- ```rust
  pub fn at_time(self: Self, simulation_time: Time) -> Self { /* ... */ }
  ```
  Set the application-owned simulation clock. Builder-style, matching

- ```rust
  pub fn with_fluid_temperature(self: Self, fluid_temperature: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Set the pumped-fluid temperature used to colour the passages.

- ```rust
  pub fn with_temperature_range(self: Self, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Set the colour-scale range. Builder-style.

- ```rust
  pub fn rotor_angle(self: &Self) -> Angle { /* ... */ }
  ```
  Current rotor phase angle, `theta = omega * t`.

- ```rust
  pub fn is_turning(self: &Self) -> bool { /* ... */ }
  ```
  Whether the shaft is turning at all.

- ```rust
  pub fn fluid_colour(self: &Self) -> Color32 { /* ... */ }
  ```
  Colour of the wetted passages: the shared temperature map when a fluid

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws the machine selected by [`PumpVisual::kind`], letterboxed inside

- **WithSubscriber**
## Module `reactor_archetype`

Schematic reactor-vessel art, one architecture per reactor type.

The six reactor types scoped in `docs/reactor-scoping/` do not share a
vessel shape, and the differences are the physics: a BWR's chimney and
separator exist to drive natural circulation, an integral PWR's steam
generator lives *inside* the vessel, EBR-II's core sits submerged in a
sodium pool, MSRE drains its fuel through a freeze valve. Drawing them all
as the same rectangle would hide exactly what makes each one interesting.

So this module draws each architecture distinctly, at schematic fidelity —
recognisable, labelled, and coloured by real temperatures, but not to
scale and not a design drawing.

# What this is not

**These are not validated models and carry no plant data.** They are
offline demonstration art for the widget gallery and for simulators that
have not yet earned bespoke vessel art. Geometry is illustrative and does
not represent any specific licensed design. See `RESPONSIBLE_USE.md`.

Where a reactor *has* earned bespoke art, this module **delegates to it
rather than redrawing it**: [`ReactorArchetype::Fhr`] renders the real
[`crate::components::fhr_reactor_vessel::FhrReactorVesselVisual`] — the
artwork migrated out of the `fhr_sim_v2` simulator — so the gallery and the
simulator show the same vessel and iterating on one improves both. Its
fourteen region temperatures are interpolated from the three this archetype
carries; see `draw_fhr` for exactly how, and prefer building that widget
directly if you hold real per-region state.

# Dispatch

[`ReactorArchetype`] is an enum, not a trait object, per the workspace
rule: the set of reactor architectures is closed and known at compile time,
so adding one is a variant and the compiler then points at every match that
needs handling.

```rust
pub mod reactor_archetype { /* ... */ }
```

### Types

#### Enum `ReactorArchetype`

Which reactor architecture to draw.

Each variant corresponds to a scoping document under
`docs/reactor-scoping/`.

```rust
pub enum ReactorArchetype {
    Htr10,
    Msre,
    IntegralPwr,
    Bwr,
    Fhr,
    EbrII,
}
```

##### Variants

###### `Htr10`

Pebble-bed high-temperature gas reactor (HTR-10): helium through a
graphite-moderated pebble bed, surrounded by a graphite reflector.

###### `Msre`

Molten Salt Reactor Experiment: fuel dissolved in the flowing salt,
graphite stringers in the core, and a drain tank below a freeze valve.

###### `IntegralPwr`

Integral PWR SMR: core, riser and a helical-coil steam generator all
inside one vessel, with the pressuriser in the head.

###### `Bwr`

Natural-circulation BWR: boiling core, chimney, steam separator and
dryer, with the downcomer annulus returning the liquid.

###### `Fhr`

Fluoride-salt-cooled high-temperature reactor: pebble bed in FLiBe with
downcomers either side.

###### `EbrII`

Pool-type sodium fast reactor (EBR-II): core, pumps and intermediate
heat exchanger all submerged in a sodium pool with a free surface.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short display name.

- ```rust
  pub fn description(self: Self) -> &'static str { /* ... */ }
  ```
  Reactor type in words, for a caption.

- ```rust
  pub fn coolant(self: Self) -> &'static str { /* ... */ }
  ```
  Primary coolant.

- ```rust
  pub fn secondary(self: Self) -> &'static str { /* ... */ }
  ```
  How heat leaves the plant, in words.

- ```rust
  pub fn approximate_thermal_power_mw(self: Self) -> f64 { /* ... */ }
  ```
  Approximate thermal power, in megawatts, for scaling a lumped model.

- ```rust
  pub fn illustrative_kinetics(self: Self) -> NordheimFuchsExactTimestepper { /* ... */ }
  ```
  A Nordheim-Fuchs prompt-excursion model with kinetics parameters

- ```rust
  pub fn scoping_doc(self: Self) -> &'static str { /* ... */ }
  ```
  The scoping document that covers this reactor.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReactorArchetype { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReactorArchetype) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `ReactorArchetypeVisual`

Schematic vessel art for one reactor architecture.

Placement follows the convention every widget in [`crate::components`]
uses: `screen_position` is the on-screen centre, `screen_vector` the box
size, so the vessel can be positioned absolutely on a schematic canvas.

Three temperatures drive the colouring. They are absolute thermodynamic
temperatures (`uom`-typed, so the unit rides with the type):

- `core_temp` — the hot region: fuel, pebble bed, or the boiling core.
- `inlet_temp` — coolant entering the vessel (the cold leg).
- `outlet_temp` — coolant leaving the vessel (the hot leg).

`min_temp`/`max_temp` bound the colour scale. Because the shared map is
diverging — blue, through neutral white, to red — the midpoint carries
meaning, so set the range symmetrically about whatever reference matters
rather than clamping it to the extremes seen.

```rust
pub struct ReactorArchetypeVisual {
    pub archetype: ReactorArchetype,
    pub screen_position: egui::Pos2,
    pub screen_vector: egui::Vec2,
    pub min_temp: uom::si::f64::ThermodynamicTemperature,
    pub max_temp: uom::si::f64::ThermodynamicTemperature,
    pub core_temp: uom::si::f64::ThermodynamicTemperature,
    pub inlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub outlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub control_rod_insertion_frac: f32,
    pub show_labels: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `archetype` | `ReactorArchetype` | Which architecture to draw. |
| `screen_position` | `egui::Pos2` | On-screen centre position. |
| `screen_vector` | `egui::Vec2` | On-screen size of the vessel box, in points. |
| `min_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature mapped to the coldest displayable colour. |
| `max_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature mapped to the hottest displayable colour. |
| `core_temp` | `uom::si::f64::ThermodynamicTemperature` | Core / fuel region temperature. |
| `inlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Coolant inlet (cold leg) temperature. |
| `outlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Coolant outlet (hot leg) temperature. |
| `control_rod_insertion_frac` | `f32` | Control-rod insertion, dimensionless in `[0, 1]`: `0.0` fully<br>withdrawn, `1.0` fully inserted. Clamped at render time, so a<br>controller that transiently overshoots draws fully in or fully out<br>instead of panicking. |
| `show_labels` | `bool` | Whether to draw the small component labels inside the vessel. Off for<br>thumbnails, where they would be unreadable. |

##### Implementations

###### Methods

- ```rust
  pub fn new(archetype: ReactorArchetype, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature, core_temp: ThermodynamicTemperature, inlet_temp: ThermodynamicTemperature, outlet_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Build a vessel visual for `archetype`.

- ```rust
  pub fn with_rod_insertion(self: Self, frac: f32) -> Self { /* ... */ }
  ```
  Set control-rod insertion. Builder-style. Dimensionless `[0, 1]`.

- ```rust
  pub fn without_labels(self: Self) -> Self { /* ... */ }
  ```
  Turn the internal component labels off — for thumbnails.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws the vessel for [`Self::archetype`], coloured by the three

- **WithSubscriber**
## Module `reactor_vessel`

Visual reactor vessel.

Wraps [`nee_soon::NordheimFuchsExactTimestepper`] (the prompt-excursion
"Prompt Excursion Layer" model) with screen geometry and a temperature
range for colour mapping, the same pattern
[`crate::components::pipe::PipeVisual`] uses for [`tampines::components::Pipe`].

```rust
pub mod reactor_vessel { /* ... */ }
```

### Types

#### Struct `ReactorVesselVisual`

Visual representation of a reactor vessel driven by a
[`NordheimFuchsExactTimestepper`].

The vessel fills with a colour derived from the model's lumped fuel
temperature, so a prompt excursion is visible as the vessel heating up
rather than as a number changing elsewhere on screen.

```rust
pub struct ReactorVesselVisual {
    pub physics: nee_soon::NordheimFuchsExactTimestepper,
    pub screen_position: egui::Pos2,
    pub screen_vector: egui::Vec2,
    pub min_temp: uom::si::f64::ThermodynamicTemperature,
    pub max_temp: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `physics` | `nee_soon::NordheimFuchsExactTimestepper` | The underlying physics component (prompt power + fuel temperature). |
| `screen_position` | `egui::Pos2` | On-screen centre position. |
| `screen_vector` | `egui::Vec2` | On-screen size. |
| `min_temp` | `uom::si::f64::ThermodynamicTemperature` | Fuel temperature mapped to<br>the coldest displayable colour (`hotness = 0.0`)<br>(coldest displayable colour). |
| `max_temp` | `uom::si::f64::ThermodynamicTemperature` | Fuel temperature mapped to `hotness = 1.0` (hottest displayable<br>colour). |

##### Implementations

###### Methods

- ```rust
  pub fn new(physics: NordheimFuchsExactTimestepper, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Wrap a [`NordheimFuchsExactTimestepper`] with the given screen

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Renders the vessel as a rectangle filled by the lumped fuel

- **WithSubscriber**
## Module `steam_generator`

Schematic steam-generator art, one architecture per steam-generator type.

The three steam generators this workspace's reactors actually use do not
share a shape, and the differences are the physics rather than the styling:

- A **vertical U-tube** generator (the Western PWR standard) is a
  *recirculating* machine. Only steam leaves the top, so it needs moisture
  separators and dryers above the bundle, a water level to separate at, and
  a downcomer annulus to return the separated water — with the feedwater
  mixed in — to the bottom of the bundle.
- A **horizontal U-tube** generator (the VVER standard) does the same job
  lying down. The bundle runs horizontally between two vertical collectors,
  and because the free surface now runs the whole length of the vessel the
  steam space is broad and shallow instead of tall — which is exactly why
  it needs no separator stack.
- A **helical coil** generator (HTGR and integral-PWR practice) is
  *once-through*. Feedwater enters one end and superheated steam leaves the
  other, so there is no recirculation, no separator, no downcomer, and **no
  water level at all** — see [`SteamGeneratorKind::has_water_level`].

Drawing all three as the same coloured rectangle would hide where the phase
change happens and how the water gets back to the bundle, which is the only
interesting thing about a steam generator.

# Dispatch

[`SteamGeneratorKind`] is an enum, not a trait object, per the workspace
rule: the set of architectures is closed and known at compile time, so
adding one is a variant and the compiler then points at every match that
needs handling. The same applies to [`SteamGeneratorVisualState`], which
chooses between caller-supplied scalars and a live
[`tampines::components::SteamGenerator`].

# Provenance of the proportions

Each variant carries its own [`SteamGeneratorKind::native_aspect_ratio`]
and letterboxes to it (see [`SteamGeneratorKind::fit_native_aspect`]), so
the artwork keeps its real slenderness at any box size:

| Variant | Ratio (w : h) | Where the numbers come from |
|---|---|---|
| [`SteamGeneratorKind::VerticalUTube`] | 4.5 m / 20.6 m | the widely published envelope of a large Western PWR recirculating steam generator |
| [`SteamGeneratorKind::HorizontalUTube`] | 13.84 m / 4.0 m | the published PGV-1000M horizontal steam-generator vessel length and inner diameter |
| [`SteamGeneratorKind::HelicalCoil`] | 2.5 m / 11.3 m | the HTR-10 steam-generator pressure vessel, "approximately 11.3 m in height, 2.5 m inner diameter" |

The helical-coil *arrangement* also follows the HTR-10 description in the
IAEA coordinated-research-programme report ingested into this workspace's
literature layer at
`crates/kovan-literature/generated/markdown/open/htr-10-iaea.md`: a
once-through modular helical tube type, hot helium arriving through a
**centre tube** to the top of the unit and then flowing **down** around the
tubes (cooling 700 degC to 250 degC) before returning up along the vessel
wall to the blower, with the water flowing through the helical tubes
**from the bottom to the top** (feedwater 104 degC, steam 435 degC at the
turbine inlet).

# What this is not

**This is offline demonstration artwork, not a validated model and not a
design drawing.** Only the overall envelope ratios above are taken from
published dimensions; every internal feature is proportioned by eye for
legibility, and nothing here represents a specific licensed design. The
drawn temperature gradients are display interpolations between the scalars
the caller supplies — they are not the output of a heat balance, and this
crate deliberately owns no physics (see the crate `CLAUDE.md`). See
`RESPONSIBLE_USE.md`.

```rust
pub mod steam_generator { /* ... */ }
```

### Types

#### Enum `SteamGeneratorKind`

Which steam-generator architecture to draw.

Each variant is a genuinely different machine, not a restyling: see the
module documentation for how recirculating and once-through units differ.

```rust
pub enum SteamGeneratorKind {
    VerticalUTube,
    HorizontalUTube,
    HelicalCoil,
}
```

##### Variants

###### `VerticalUTube`

Vertical recirculating U-tube generator — the Western PWR standard.

Inverted-U bundle in the lower shell, moisture separators and dryers
above it, a downcomer annulus outside the tube wrapper, and a
hemispherical channel head split by a divider plate into hot-leg and
cold-leg chambers.

###### `HorizontalUTube`

Horizontal recirculating U-tube generator — the VVER standard.

Horizontal cylindrical vessel; primary water rises and falls through
two vertical cylindrical collectors, with the tube bundle running
horizontally between them under a water level that spans the vessel.

###### `HelicalCoil`

Once-through helical-coil generator — HTGR and integral-PWR practice.

Helical bundle wound around a central column, counter-current, with no
separators, no downcomer and no water level.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Short display name, for a picker or a card caption.

- ```rust
  pub fn description(self: Self) -> &'static str { /* ... */ }
  ```
  Which reactor family uses this architecture, in words.

- ```rust
  pub fn circulation(self: Self) -> &'static str { /* ... */ }
  ```
  How the secondary side is circulated — the single fact that explains

- ```rust
  pub fn has_water_level(self: Self) -> bool { /* ... */ }
  ```
  Whether this architecture has a secondary-side water level at all.

- ```rust
  pub fn native_aspect_ratio(self: Self) -> f32 { /* ... */ }
  ```
  Width-to-height ratio the artwork is drawn at, dimensionless.

- ```rust
  pub fn fit_native_aspect(self: Self, available: Rect) -> Rect { /* ... */ }
  ```
  The largest sub-rectangle of `available` carrying this kind's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SteamGeneratorKind { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SteamGeneratorKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `SteamGeneratorScalars`

Scalar thermal state of a steam generator, as the caller's own model holds
it.

All four temperatures are absolute thermodynamic temperatures (`uom`-typed,
so the compiler enforces the unit; they are kelvin internally and are
conventionally quoted in degrees Celsius). They are used only for colour,
via the shared [`crate::components::temperature_colour`] map.

```rust
pub struct SteamGeneratorScalars {
    pub primary_inlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub primary_outlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub feedwater_temp: uom::si::f64::ThermodynamicTemperature,
    pub steam_temp: uom::si::f64::ThermodynamicTemperature,
    pub water_level_frac: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `primary_inlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Primary coolant entering the generator — the hot leg. Drawn on the<br>inlet plenum/collector and at the start of the tube gradient. |
| `primary_outlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Primary coolant leaving the generator — the cold leg. Drawn on the<br>outlet plenum/collector and at the end of the tube gradient. |
| `feedwater_temp` | `uom::si::f64::ThermodynamicTemperature` | Secondary feedwater entering the generator. Drawn on the feedwater<br>nozzle, the distribution ring/header and the downcomer. |
| `steam_temp` | `uom::si::f64::ThermodynamicTemperature` | Secondary steam leaving the generator. Drawn on the steam space, the<br>steam nozzle, and — for a recirculating unit, whose bulk water sits at<br>saturation — on the water region too. |
| `water_level_frac` | `f32` | Secondary water level, dimensionless in `[0, 1]`; see<br>[`drawn_water_level`]. Ignored by<br>[`SteamGeneratorKind::HelicalCoil`]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SteamGeneratorScalars { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SteamGeneratorScalars) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `SteamGeneratorPhysicsState`

A [`tampines::components::SteamGenerator`] plus the scalars it does not
itself expose.

The physics component owns a secondary-side
[`tampines::hem::HemSteamCv`], whose `get_temperature()` is a real working
getter — that is where [`SteamGeneratorScalars::steam_temp`] comes from on
this path, so the steam space and the steam nozzle track the real state.

It exposes **no primary-side temperature** (its `step` is not yet
implemented), so the primary inlet/outlet, the feedwater temperature and
the level are supplied alongside it by the caller. They are not fabricated:
[`SteamGeneratorVisual::new`] starts them all at the secondary temperature,
which draws an isothermal generator until a caller supplies real values
with [`SteamGeneratorVisual::with_primary_temperatures`],
[`SteamGeneratorVisual::with_feedwater_temperature`] and
[`SteamGeneratorVisual::with_water_level`].

```rust
pub struct SteamGeneratorPhysicsState {
    pub physics: tampines::components::SteamGenerator,
    pub primary_inlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub primary_outlet_temp: uom::si::f64::ThermodynamicTemperature,
    pub feedwater_temp: uom::si::f64::ThermodynamicTemperature,
    pub water_level_frac: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `physics` | `tampines::components::SteamGenerator` | The underlying TAMPINES component. |
| `primary_inlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Primary coolant entering the generator (hot leg). |
| `primary_outlet_temp` | `uom::si::f64::ThermodynamicTemperature` | Primary coolant leaving the generator (cold leg). |
| `feedwater_temp` | `uom::si::f64::ThermodynamicTemperature` | Secondary feedwater entering the generator. |
| `water_level_frac` | `f32` | Secondary water level, dimensionless in `[0, 1]`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SteamGeneratorPhysicsState { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SteamGeneratorPhysicsState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `SteamGeneratorVisualState`

Where a [`SteamGeneratorVisual`] gets the state it renders.

Enum dispatch, not a trait object, per the workspace's mandatory "no trait
objects" Rust design rule.

```rust
pub enum SteamGeneratorVisualState {
    Scalars(SteamGeneratorScalars),
    Physics(SteamGeneratorPhysicsState),
}
```

##### Variants

###### `Scalars`

Backed by caller-supplied scalars — a simulator that already holds
these temperatures in its own plant model.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SteamGeneratorScalars` |  |

###### `Physics`

Backed by a live TAMPINES steam generator; the steam temperature is
read from its secondary-side control volume every frame.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SteamGeneratorPhysicsState` |  |

##### Implementations

###### Methods

- ```rust
  pub fn resolve(self: &Self) -> SteamGeneratorScalars { /* ... */ }
  ```
  The scalars the artwork is actually drawn from.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SteamGeneratorVisualState { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SteamGeneratorVisualState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `SteamGeneratorVisual`

Visual representation of a steam generator, in one of three architectures.

Scalar-fed like the reactor vessels, and for the same reason: a simulator
already holds these temperatures in its own plant model, and standing up a
whole secondary-side control volume to colour a schematic would be
disproportionate. The physics-backed path is still available and still
real — see [`Self::new`] and [`SteamGeneratorVisualState::Physics`].

All temperatures are absolute thermodynamic temperatures (`uom`-typed).
`min_temp`/`max_temp` bound the diverging colour scale; because the map is
diverging (blue at min, neutral white at the *midpoint*, red at max), set
them about a reference that matters rather than clamping to the extremes
seen.

```rust
pub struct SteamGeneratorVisual {
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
  pub fn new(physics: SteamGenerator, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Wrap a [`SteamGenerator`] with the given screen geometry and

- ```rust
  pub fn from_scalars(kind: SteamGeneratorKind, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature, scalars: SteamGeneratorScalars) -> Self { /* ... */ }
  ```
  Build a generator visual from the caller's own scalar plant state.

- ```rust
  pub fn with_kind(self: Self, kind: SteamGeneratorKind) -> Self { /* ... */ }
  ```
  Draw a different architecture with the same state.

- ```rust
  pub fn with_primary_temperatures(self: Self, inlet: ThermodynamicTemperature, outlet: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Set the primary hot-leg and cold-leg temperatures.

- ```rust
  pub fn with_feedwater_temperature(self: Self, feedwater: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Set the secondary feedwater temperature.

- ```rust
  pub fn with_water_level(self: Self, frac: f32) -> Self { /* ... */ }
  ```
  Set the secondary water level, dimensionless in `[0, 1]`.

- ```rust
  pub fn without_labels(self: Self) -> Self { /* ... */ }
  ```
  Turn the internal component labels off — for thumbnails.

- ```rust
  pub fn kind(self: &Self) -> SteamGeneratorKind { /* ... */ }
  ```
  Which architecture this visual draws.

- ```rust
  pub fn size(self: &Self) -> Vec2 { /* ... */ }
  ```
  On-screen size of the box the artwork letterboxes into, in points.

- ```rust
  pub fn scalars(self: &Self) -> SteamGeneratorScalars { /* ... */ }
  ```
  The scalars the artwork is drawn from, resolving the physics path if

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws the steam generator for [`SteamGeneratorVisual::kind`], coloured

- **WithSubscriber**
### Functions

#### Function `drawn_water_level`

The secondary-side water level actually drawn, dimensionless in `[0, 1]`.

`0.0` is a drained generator (level at the tubesheet, or at the bottom of a
horizontal vessel) and `1.0` is a full one (level at the separator inlet,
or at the top of a horizontal vessel). Normal operation sits well inside
that range.

Returns `None` for [`SteamGeneratorKind::HelicalCoil`], which has no free
surface to hold a level on.

Out-of-range values are clamped rather than rejected, because a level comes
from a controller that can transiently overshoot and artwork must not panic
on it. A **non-finite** level draws as empty (`0.0`) — deliberately the most
visible outcome, so a NaN in the caller's model shows up on screen instead
of hiding behind a plausible mid-drum level.

```rust
pub fn drawn_water_level(kind: SteamGeneratorKind, frac: f32) -> Option<f32> { /* ... */ }
```

### Constants and Statics

#### Constant `VERTICAL_U_TUBE_ASPECT_RATIO`

Width-to-height ratio of the vertical U-tube generator, dimensionless.

A large Western PWR recirculating steam generator stands roughly 20.6 m
high over an upper-shell diameter of about 4.5 m, so the silhouette is very
slender — which is what makes room for the separator stack above the
bundle.

```rust
pub const VERTICAL_U_TUBE_ASPECT_RATIO: f32 = _;
```

#### Constant `HORIZONTAL_U_TUBE_ASPECT_RATIO`

Width-to-height ratio of the horizontal U-tube (VVER) generator,
dimensionless.

From the published PGV-1000M vessel: about 13.84 m long over a 4.0 m inner
diameter. This is the only variant wider than it is tall, and the reason is
physical — the free surface has to run the length of the vessel.

```rust
pub const HORIZONTAL_U_TUBE_ASPECT_RATIO: f32 = _;
```

#### Constant `HELICAL_COIL_ASPECT_RATIO`

Width-to-height ratio of the helical-coil generator, dimensionless.

From the HTR-10 steam-generator pressure vessel: approximately 11.3 m high
by 2.5 m inner diameter. That vessel also houses the helium blower above
the tube bundle; only the bundle's own envelope is drawn here.

```rust
pub const HELICAL_COIL_ASPECT_RATIO: f32 = _;
```

## Module `temperature_button`

Temperature-coloured buttons for control panels and legends.

A button whose fill colour is driven by a temperature, so a side panel of
setpoints or readouts reads as a temperature scale rather than as a list of
identical grey buttons. These are the button-shaped counterpart to the
widgets in this module: same colour maps, same display-range convention, so
a panel button and the vessel it refers to agree about what "hot" looks
like.

Two maps are offered because they suit different backgrounds — see
[`blue_red`] and [`black_red`]. Both are the older non-perceptual maps from
[`crate::color_maps`], kept because existing call sites depend on their
exact values; new *field* visualisations should prefer the perceptually
uniform Crameri map used by [`super::temperature_colour`].

```rust
pub mod temperature_button { /* ... */ }
```

### Functions

#### Function `hotness`

Where `temp` falls in a display range, as a dimensionless fraction.

Returns `0.0` at `min_temp` and `1.0` at `max_temp`, linearly between.
**Not clamped** — a temperature outside the range returns a value outside
`[0, 1]`, which the colour maps then saturate. That is deliberate: a
readout pinned at the top of its scale should look saturated, not wrap
around.

```rust
pub fn hotness(temp: uom::si::f64::ThermodynamicTemperature, min_temp: uom::si::f64::ThermodynamicTemperature, max_temp: uom::si::f64::ThermodynamicTemperature) -> f32 { /* ... */ }
```

#### Function `blue_red`

A labelled button filled on a **blue-to-red** scale.

Blue at `min_temp`, red at `max_temp`. Suits light panel backgrounds, where
a cold reading should still be clearly visible.

`min_temp`/`max_temp` bound the colour scale; pick them to span the range
the panel expects to show, so a normal reading does not sit pinned at
either end.

```rust
pub fn blue_red<''a>(temp: uom::si::f64::ThermodynamicTemperature, min_temp: uom::si::f64::ThermodynamicTemperature, max_temp: uom::si::f64::ThermodynamicTemperature, label: &'a str) -> egui::Button<''a> { /* ... */ }
```

#### Function `black_red`

A labelled button filled on a **black-to-red** scale.

Black at `min_temp`, red at `max_temp`. Suits dark panel backgrounds, and
reads more like a glowing-hot surface than a diverging scale.

`min_temp`/`max_temp` bound the colour scale; pick them to span the range
the panel expects to show, so a normal reading does not sit pinned at
either end.

```rust
pub fn black_red<''a>(temp: uom::si::f64::ThermodynamicTemperature, min_temp: uom::si::f64::ThermodynamicTemperature, max_temp: uom::si::f64::ThermodynamicTemperature, label: &'a str) -> egui::Button<''a> { /* ... */ }
```

#### Function `blue_red_degc`

Convenience for callers that already hold plain degrees Celsius.

Panel code frequently carries setpoints as bare `f64` degC rather than
`uom` quantities. This wraps them so such a call site does not have to
spell out the unit conversion three times per button.

Prefer [`blue_red`] where the caller already has `uom` quantities — the
typed path is the one that catches unit mistakes.

```rust
pub fn blue_red_degc(temp_degc: f64, min_degc: f64, max_degc: f64, label: &str) -> egui::Button<''_> { /* ... */ }
```

#### Function `black_red_degc`

Degrees-Celsius convenience for [`black_red`]. See [`blue_red_degc`].

```rust
pub fn black_red_degc(temp_degc: f64, min_degc: f64, max_degc: f64, label: &str) -> egui::Button<''_> { /* ... */ }
```

## Module `turbine`

Visual **steam** turbine.

Renders a multi-stage axial turbine as rows of blades whose height grows
monotonically from inlet to exhaust — the annulus opens out along the flow,
as it does in a real machine where steam expands from short high-pressure
blades onto long low-pressure ones. A spinning rotor is drawn on top; the
angle is not an animation constant: it is `theta = omega * t`, where
`omega` is read from a real torque-balance model, so what you see is the
machine's actual shaft speed.

## Where the physics comes from

[`TurbineVisualState`] is an enum (not a trait object, per the workspace's
mandatory design rules) because the set of state sources is closed and
growing:

- [`TurbineVisualState::SteamGenerator`] wraps
  [`ThreePhaseElectricGeneratorTurbine`] from `tampines-steam-tables`. This
  is a **working** lumped model: an explicit torque balance advances rotor
  angular velocity, and per-phase EMF, current and total electrical power
  are read off it. It is the only variant that can report a real shaft
  speed, so it is the only one whose blades genuinely spin.
- [`TurbineVisualState::SteamThermo`] wraps
  [`tampines::components::Turbine`], which carries the inlet steam state and
  an adiabatic efficiency. It supplies the casing colour but **no** rotation
  — `Turbine::expand_to` is not implemented yet, so there is no shaft speed
  to draw and the rotor is rendered stationary rather than at a fabricated
  speed.

Both variants are steam turbines. Gas and supercritical-CO2 turbines are
future work and deliberately have no variant here yet; the blade artwork
itself is working-fluid agnostic (axial machines look alike), so they are
expected to add state variants rather than a new widget.

## Simulation time is application-owned

Like [`crate::animation::TracerTrain`], the rotor phase depends on elapsed
simulation time, and widgets are rebuilt every repaint — a clock owned by
the widget would reset to zero each frame and the turbine would never turn.
The **application** owns the clock and passes it in via
[`TurbineVisual::at_time`].

## Provenance

The blade geometry is ported from this crate's own `fhr_sim_v2` example
(`app/local_widgets_and_buttons/turbine_widget.rs`), generalised so the
rotation angle derives from a physics model instead of being set by the
caller.

```rust
pub mod turbine { /* ... */ }
```

### Types

#### Enum `TurbineFlowPath`

How steam is admitted to, and exhausted from, the machine.

Enum dispatch per the workspace's "no trait objects" rule — the set of flow
paths is closed.

```rust
pub enum TurbineFlowPath {
    DoubleFlow,
    SingleFlow,
}
```

##### Variants

###### `DoubleFlow`

Steam is admitted at the centre and expands **outward in both
directions**, exhausting at each end. The rotor is therefore mirrored
about the admission plane.

This is the default because it is standard practice for large PWR
turbine-generators — both the HP cylinder and the LP cylinders are
commonly double-flow. Two reasons drive it: a PWR steam generator
delivers *saturated* steam with a small enthalpy drop per kilogram, so
the mass (and hence volumetric) flow is large and needs a lot of
annulus area even at the HP inlet; and splitting the flow symmetrically
cancels most of the axial thrust that a single-flow rotor would dump
into its thrust bearing.

###### `SingleFlow`

Steam is admitted at one end and expands monotonically to an exhaust at
the other. More typical of fossil plant, where superheated steam is
denser at inlet so one flow path carries the volume.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TurbineFlowPath { /* ... */ }
    ```

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
    fn default() -> TurbineFlowPath { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
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

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **NoneValue**
  - ```rust
    fn null_value() -> T { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TurbineFlowPath) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `StageAngles`

Placeholder blade angles for one stage.

# These numbers are placeholders, not turbine design

A real multi-stage steam turbine is a *mixture* of impulse and reaction
stages, and the blade angles differ stage by stage. The **degree of
reaction** is the fraction of a stage's enthalpy drop taken across the
rotor: an impulse stage is near 0 (essentially all the expansion happens in
the fixed nozzle, and the rotor only turns the flow), while a reaction
stage is typically around 0.5 (expansion split between stator and rotor).
Classic practice puts impulse stages at the high-pressure admission end and
reaction stages towards the low-pressure exhaust.

[`TurbineVisual`] reproduces that *trend* so the drawing is not uniform
nonsense, but the specific angles below are illustrative and are **not
derived from any turbine design**. They must not be presented, cited, or
re-used as though they were. When the detailed turbine model lands
(workspace bead `op-dt3.18`), the angles are to be taken from it and this
schedule deleted — see bead `op-wqk.14.11`.

```rust
pub struct StageAngles {
    pub stator_deg: f32,
    pub rotor_deg: f32,
    pub reaction: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stator_deg` | `f32` | Stator (nozzle) blade angle, degrees from the axial direction. |
| `rotor_deg` | `f32` | Rotor blade angle, degrees from the axial direction. |
| `reaction` | `f32` | Degree of reaction, dimensionless: 0.0 = pure impulse, 0.5 = typical<br>reaction stage. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> StageAngles { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &StageAngles) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Enum `TurbineVisualState`

Where a [`TurbineVisual`] gets the physics it renders.

Enum dispatch, not a trait object, per the workspace's mandatory "no trait
objects" Rust design rule. See the module docs for why each variant exists
and what it can and cannot show.

```rust
pub enum TurbineVisualState {
    SteamGenerator(tampines_steam_tables::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine),
    SteamThermo(tampines::components::Turbine),
}
```

##### Variants

###### `SteamGenerator`

Steam turbine coupled to a three-phase synchronous generator. Reports a
real, torque-balance-derived shaft speed, so the rotor spins.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines_steam_tables::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine` |  |

###### `SteamThermo`

Steam turbine known only by its thermodynamic inlet state. Colours the
casing; cannot report a shaft speed, so the rotor is drawn stationary.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `tampines::components::Turbine` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TurbineVisualState { /* ... */ }
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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TurbineVisualState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
- **WithSubscriber**
#### Struct `TurbineVisual`

Visual representation of a steam turbine.

Placement follows the same convention as every other widget in
[`crate::components`]: `screen_position` is the on-screen centre and
`screen_vector` the box size, so the machine can be positioned absolutely
on a schematic canvas. Blade-row radii and the stepped silhouette are
derived from that box; the *rotation* is derived from physics.

```rust
pub struct TurbineVisual {
    pub state: TurbineVisualState,
    pub screen_position: egui::Pos2,
    pub screen_vector: egui::Vec2,
    pub flow_path: TurbineFlowPath,
    pub simulation_time: uom::si::f64::Time,
    pub min_temp: uom::si::f64::ThermodynamicTemperature,
    pub max_temp: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `state` | `TurbineVisualState` | The physics state this widget renders. |
| `screen_position` | `egui::Pos2` | On-screen centre position. |
| `screen_vector` | `egui::Vec2` | On-screen size of the whole machine, in points. |
| `flow_path` | `TurbineFlowPath` | How steam is admitted and exhausted. Defaults to<br>[`TurbineFlowPath::DoubleFlow`], standard for large PWR cylinders. |
| `simulation_time` | `uom::si::f64::Time` | Elapsed simulation time, owned and advanced by the application.<br><br>Combined with the model's angular velocity to give the rotor phase<br>`theta = omega * simulation_time`. See the module docs for why this is<br>not owned by the widget. |
| `min_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature mapped to `hotness = 0.0` (coldest displayable colour). |
| `max_temp` | `uom::si::f64::ThermodynamicTemperature` | Temperature mapped to `hotness = 1.0` (hottest displayable colour). |

##### Implementations

###### Methods

- ```rust
  pub fn new_generator(generator: ThreePhaseElectricGeneratorTurbine, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Wrap a [`ThreePhaseElectricGeneratorTurbine`] — the variant with a real

- ```rust
  pub fn new_thermo(physics: Turbine, screen_position: Pos2, screen_vector: Vec2, min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Wrap a [`tampines::components::Turbine`] — colour only, no rotation

- ```rust
  pub fn at_time(self: Self, simulation_time: Time) -> Self { /* ... */ }
  ```
  Set the application-owned simulation clock. Builder-style, so it chains

- ```rust
  pub fn with_flow_path(self: Self, flow_path: TurbineFlowPath) -> Self { /* ... */ }
  ```
  Override the flow path. Builder-style; the default is

- ```rust
  pub fn stage_quality(self: &Self, stage_fraction: f32) -> f32 { /* ... */ }
  ```
  Placeholder steam quality at a given stage, admission (0.0) to exhaust

- ```rust
  pub fn stage_angles(self: &Self, stage_fraction: f32) -> StageAngles { /* ... */ }
  ```
  Placeholder blade angles at a given stage, admission (0.0) to exhaust

- ```rust
  pub fn rotor_angle(self: &Self) -> Angle { /* ... */ }
  ```
  Current rotor phase angle, `theta = omega * t`.

- ```rust
  pub fn casing_temperature(self: &Self) -> Option<ThermodynamicTemperature> { /* ... */ }
  ```
  Casing colour source: the inlet steam temperature, when the variant

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Draws `2 * BLADE_ROWS_PER_SIDE + 1` stator rows of growing radius, then

- **WithSubscriber**
## Module `valve`

Visual valve.

Wraps [`tampines::components::Valve`] with screen geometry. Colours
itself by opening percentage (fully closed = grey, fully open = green) --
[`Valve::current_kv`] is real/working, so this is real data, not a
placeholder.

```rust
pub mod valve { /* ... */ }
```

### Types

#### Struct `ValveVisual`

Visual representation of a [`Valve`].

```rust
pub struct ValveVisual {
    pub physics: tampines::components::Valve,
    pub screen_position: egui::Pos2,
    pub screen_vector: egui::Vec2,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `physics` | `tampines::components::Valve` | The underlying physics component. |
| `screen_position` | `egui::Pos2` | On-screen centre position. |
| `screen_vector` | `egui::Vec2` | On-screen size. |

##### Implementations

###### Methods

- ```rust
  pub fn new(physics: Valve, screen_position: Pos2, screen_vector: Vec2) -> Self { /* ... */ }
  ```
  Wrap a [`Valve`] with the given screen geometry.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

  - ```rust
    fn into_any(self: Box<T>) -> Box<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn into_any_rc(self: Rc<T>) -> Rc<dyn Any> { /* ... */ }
    ```

  - ```rust
    fn as_any(self: &Self) -> &dyn Any + ''static { /* ... */ }
    ```

  - ```rust
    fn as_any_mut(self: &mut Self) -> &mut dyn Any + ''static { /* ... */ }
    ```

- **DowncastSync**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Instrument**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoAnyArc**
  - ```rust
    fn into_any_arc(self: Arc<T>) -> Arc<dyn Any + Sync + Send> { /* ... */ }
    ```

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
- **SimdFrom**
  - ```rust
    fn simd_from(value: T, _simd: S) -> T { /* ... */ }
    ```

- **SimdInto**
  - ```rust
    fn simd_into(self: Self, simd: S) -> T { /* ... */ }
    ```

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
- **Widget**
  - ```rust
    fn ui(self: Self, ui: &mut Ui) -> Response { /* ... */ }
    ```
    Minimal-static rendering: a filled rectangle, green fraction of which

- **WithSubscriber**
### Re-exports

#### Re-export `PipeBendVisual`

```rust
pub use bend::PipeBendVisual;
```

#### Re-export `CondenserDisplayRange`

```rust
pub use condenser::CondenserDisplayRange;
```

#### Re-export `CondenserKind`

```rust
pub use condenser::CondenserKind;
```

#### Re-export `CondenserScalars`

```rust
pub use condenser::CondenserScalars;
```

#### Re-export `CondenserVisual`

```rust
pub use condenser::CondenserVisual;
```

#### Re-export `CondenserVisualState`

```rust
pub use condenser::CondenserVisualState;
```

#### Re-export `slewed_control_rod_insertion`

```rust
pub use control_rod_drive::slewed_control_rod_insertion;
```

#### Re-export `CoolingTowerVisual`

```rust
pub use cooling_tower::CoolingTowerVisual;
```

#### Re-export `ExcursionOverlay`

```rust
pub use excursion::ExcursionOverlay;
```

#### Re-export `ExcursionStage`

```rust
pub use excursion::ExcursionStage;
```

#### Re-export `ExcursionTrigger`

```rust
pub use excursion::ExcursionTrigger;
```

#### Re-export `FhrReactorVesselVisual`

```rust
pub use fhr_reactor_vessel::FhrReactorVesselVisual;
```

#### Re-export `ApproachVerdict`

```rust
pub use heat_exchanger::ApproachVerdict;
```

#### Re-export `HeatExchangerConstruction`

```rust
pub use heat_exchanger::HeatExchangerConstruction;
```

#### Re-export `HeatExchangerDisplayRange`

```rust
pub use heat_exchanger::HeatExchangerDisplayRange;
```

#### Re-export `HeatExchangerKind`

```rust
pub use heat_exchanger::HeatExchangerKind;
```

#### Re-export `HeatExchangerScalars`

```rust
pub use heat_exchanger::HeatExchangerScalars;
```

#### Re-export `HeatExchangerVisual`

```rust
pub use heat_exchanger::HeatExchangerVisual;
```

#### Re-export `HeatExchangerVisualState`

```rust
pub use heat_exchanger::HeatExchangerVisualState;
```

#### Re-export `Htr10ReactorVesselVisual`

```rust
pub use htr10_reactor_vessel::Htr10ReactorVesselVisual;
```

#### Re-export `InstrumentationVisual`

```rust
pub use instrumentation::InstrumentationVisual;
```

#### Re-export `LegendUnit`

```rust
pub use legend::LegendUnit;
```

#### Re-export `TemperatureLegend`

```rust
pub use legend::TemperatureLegend;
```

#### Re-export `PipePhaseShade`

```rust
pub use pipe::PipePhaseShade;
```

#### Re-export `PipeScalars`

```rust
pub use pipe::PipeScalars;
```

#### Re-export `PipeScale`

```rust
pub use pipe::PipeScale;
```

#### Re-export `PipeVisual`

```rust
pub use pipe::PipeVisual;
```

#### Re-export `PipeVisualState`

```rust
pub use pipe::PipeVisualState;
```

#### Re-export `PipeComponent`

```rust
pub use pipe_component::PipeComponent;
```

#### Re-export `PumpVisual`

```rust
pub use pump::PumpVisual;
```

#### Re-export `ReactorArchetype`

```rust
pub use reactor_archetype::ReactorArchetype;
```

#### Re-export `ReactorArchetypeVisual`

```rust
pub use reactor_archetype::ReactorArchetypeVisual;
```

#### Re-export `ReactorVesselVisual`

```rust
pub use reactor_vessel::ReactorVesselVisual;
```

#### Re-export `SteamGeneratorVisual`

```rust
pub use steam_generator::SteamGeneratorVisual;
```

#### Re-export `StageAngles`

```rust
pub use turbine::StageAngles;
```

#### Re-export `TurbineFlowPath`

```rust
pub use turbine::TurbineFlowPath;
```

#### Re-export `TurbineVisual`

```rust
pub use turbine::TurbineVisual;
```

#### Re-export `TurbineVisualState`

```rust
pub use turbine::TurbineVisualState;
```

#### Re-export `ValveVisual`

```rust
pub use valve::ValveVisual;
```

