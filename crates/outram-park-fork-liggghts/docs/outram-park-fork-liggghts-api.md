# Crate Documentation

**Version:** 0.0.0

**Format Version:** 60

# Module `outram_park_fork_liggghts`

# outram-park-fork-liggghts

Independent, pure-Rust **granular discrete-element-method (DEM)** library for
OUTRAM PARK (bead epic `op-t3l`): particles, contact mechanics, thermal DEM,
and pebble-/packed-bed physics — the DEM/granular pillar of the Phase II
architecture (kept separate from the thermophysical-property pillar
[`tampines`] and the CFD/multiphase pillar [`outram-foam-multiphase`], with
CFD-DEM coupling deferred to a future explicit seam).

> **Licensing (see `NOTICE`).** LIGGGHTS-PUBLIC's source headers declare
> **"GNU Public License, version 2 or later"**, which **is compatible with
> GPL-3.0** (the "or later" option permits use under GPLv3) — so
> LIGGGHTS-PUBLIC source may be ported into this GPL-3.0-only crate.
> (Correcting an earlier note that wrongly said "GPL-2.0-only / blocked".)
> When porting, confirm the specific file's "or later" header and keep its
> attribution + provenance. LAMMPS-proper headers are version-unspecified
> (murkier) — treat those with care. Phase 1 below is clean-room from public
> DEM literature (no upstream-derived code) regardless.

> **⚠️ Unverified until validated — scaffold.** No human V&V yet. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions.

## Roadmap (each physics bead's DoD: theory docs + verification tests +
## reference-benchmark comparison + unit-safe `uom`)

- **Phase 1 — Particle framework** ([`particle`]) — `Particle { position,
  velocity, angular_velocity, mass, radius, temperature }` + explicit time
  integration. **In progress** (bead `op-t3l.1`).
- **Phase 2 — Contact mechanics** ([`contact`]) — Hooke + Hertz-Mindlin
  normal/tangential contact (enum dispatch). Foundation done (`op-t3l.2`).
- **Phase 3 — Boundaries** ([`boundary`]) — Plane / Wall / Box / Cylinder
  signed-distance + particle overlap. Foundation done (`op-t3l.3`).
- **Phase 4 — Thermal DEM** ([`thermal`]) — particle/particle + particle/wall
  contact conduction + temperature integration. Foundation done (`op-t3l.4`).
- **Phase 5 — CFD-DEM coupling** ([`coupling`]) — reserved architecture only
  (interfaces defined, no physics). Done as reserved (`op-t3l.5`).

Phases 2-4 are **clean-room, unit-tested foundations, not benchmark-validated**
(that is a later human step) — see each module's "Honest scope".

## LIGGGHTS-faithful path (translated from upstream, cross-code verified)

Added 2026-09-15. These modules are a **direct translation of
LIGGGHTS-PUBLIC** (commit `3d5c00f2`), each carrying its upstream provenance
header, and are verified against an upstream run committed under
`reference-data/liggghts/` — see `docs/verification-and-validation.md`.

- [`granular`] — the contact chain (`surface_model_default` →
  `normal_model_hertz`/`hooke` → `tangential_model_history`/`no_history`),
  **including the per-contact tangential shear history** that [`contact`]
  omits.
- [`granular_system`] — the history-aware driver, in LIGGGHTS' step order.
- [`integrator`] — `fix nve/sphere` kick–drift–kick velocity-Verlet.
- [`timestep`] — `fix check/timestep/gran` Rayleigh and Hertz criteria.

**Use these for any packed-bed or settling problem.** [`contact`] hard-codes
the tangential displacement to zero, so it has no shear *spring*: a static
assembly built on it cannot carry shear, and a heap has zero angle of
repose. [`particle::Particle::integrate`] is likewise not symplectic (its
own doc comment explains the measured consequences) and should not drive
contact dynamics.

## Extensions (clean-room, unit-tested)

- [`simulation`] — multi-particle DEM engine: linked-cell neighbor search +
  velocity-Verlet ensemble stepping composing [`contact`] + [`boundary`].
- [`rolling`] — rolling resistance (Ai et al.) + cohesion (JKR / linear).
- [`mesh_wall`] — triangulated (STL-style) walls + moving/rotating boundaries.
- [`thermal_radiation`] — grey-body radiation + near-field gas-gap conduction.
- [`bonded`] — linear parallel-bond model (Potyondy & Cundall 2004) for
  cemented granular material, agglomerates, and TRISO/pebble-matrix bonds.

## Design rules (workspace `CLAUDE.md`)

Enum dispatch (no `Box<dyn>`), no lifetime parameters (own by value / index
ids), `uom`-typed API boundaries, Android-buildable (pure-Rust, no BLAS/GUI).

## Modules

## Module `bonded`

**Bonded-particle model** — the *linear parallel bond* of Potyondy & Cundall
(2004) for cemented granular material, agglomerates, and TRISO-particle /
pebble-matrix modelling.

Where [`crate::contact`] gives the *unbonded* normal/tangential contact that
only ever **pushes** overlapping particles apart, this module adds a
**cemented bond**: a finite-size elastic cylinder of "glue" spanning the gap
between two particles that transmits a **force and a moment** — resisting
tension, shear, bending, and twisting — until a stress-based breakage
criterion is met, after which the bond fails and transmits nothing.

## The parallel-bond idealisation

A parallel bond is pictured as a cylinder of cementitious material of radius
`R̄` acting in parallel with the point contact, glued across the contact
plane. Its cross-section carries:

- a **normal force** `F̄_n` (a signed scalar, **tension positive**) and a
  **shear force** `F̄_s` (a vector in the contact plane), and
- a **twisting moment** `M̄_t` (about the bond axis `n̂`) and a **bending
  moment** `M̄_b` (a vector in the contact plane).

The bond is **history-dependent**: like a real spring it accumulates force
and moment from the *increments* of relative motion over each time step, so
the [`Bond`] carries this accumulated state and [`Bond::update_bond`]
advances it incrementally (Cundall & Strack's incremental small-strain DEM
philosophy, 1979).

## Geometry, stiffness, and stress (Potyondy & Cundall 2004)

With bond radius `R̄` the cross-sectional geometric properties are the
textbook ones for a solid circular section:

- area `A = π R̄²` `[m²]`,
- second moment of area (bending) `I = π R̄⁴ / 4` `[m⁴]`,
- polar moment of area (twisting) `J = π R̄⁴ / 2` `[m⁴]`.

The bond has a **normal stiffness per unit area** `k̄_n` and a **shear
stiffness per unit area** `k̄_s`, both in `[Pa/m] = [N/m³]` (multiplying a
stiffness-per-area `[Pa/m]` by area `[m²]` and a displacement `[m]` gives a
force `[N]`). Over a step the elastic force/moment increments are

```text
  ΔF̄_n = +k̄_n · A · Δδ_n           (normal, tension positive)
  ΔF̄_s = −k̄_s · A · Δδ_s           (shear, vector)
  ΔM̄_t = −k̄_s · J · Δθ_t           (twisting, about n̂)
  ΔM̄_b = −k̄_n · I · Δθ_b           (bending, vector ⟂ n̂)
```

where `Δδ_n`, `Δδ_s` are the normal and shear relative-displacement
increments at the bond and `Δθ_t`, `Δθ_b` the twist and bending relative-
rotation increments (all over the step `Δt`). The maximum tensile normal
stress and maximum shear stress acting on the bond periphery are

```text
  σ_max = F̄_n / A + |M̄_b| · R̄ / I         (axial + bending fibre stress)
  τ_max = |F̄_s| / A + |M̄_t| · R̄ / J        (direct shear + torsional shear)
```

and the bond **breaks** the instant `σ_max > σ_c` (tensile strength) or
`τ_max > τ_c` (shear strength). A broken bond is permanent and transmits
zero force and zero moment thereafter ([`Bond::is_broken`]).

## Sign & geometry conventions (read once, applies everywhere)

For a bonded pair `(a, b)`, following [`crate::contact`]:

- The **bond axis** `n̂` is the unit vector from `a`'s centre toward `b`'s
  centre: `n̂ = (x_b − x_a) / ‖x_b − x_a‖`.
- The **normal displacement increment** `Δδ_n = [(v_b − v_a)·n̂]·Δt` is
  **positive when the particles separate**, so a stretched bond builds a
  **positive (tensile)** `F̄_n`, giving `σ_max > 0` in tension.
- The accumulated **`(F̄_n, F̄_s, M̄_t, M̄_b)` is bookkept as the load the bond
  exerts on particle `b`** (with `F̄_n` stored tension-positive); the load on
  `a` is its exact Newton-third-law reaction. Concretely the returned
  [`BondForce`] has `force_on_a = F̄_n·n̂ − F̄_s` and
  `force_on_b = −force_on_a`. A tensile bond therefore pulls `a` toward `b`
  and `b` toward `a`, as a real cement ligament would.
- The bond force acts at the contact point (offset `+r_a·n̂` from `a`'s centre
  and `−r_b·n̂` from `b`'s), so the shear force exerts a lever torque about
  each centre; the bond moment `M̄` is applied as an equal-and-opposite
  internal couple on the two particles.

## Unit convention

Following the crate convention (see [`crate::particle`] and
[`crate::contact`]), the `uom` boundary sits at the constructor where a clean
named `uom` type exists: [`Bond::new`] takes the bond radius as a [`Length`]
and the tensile/shear strengths as [`Pressure`]. The stiffnesses-per-area
`k̄_n`, `k̄_s` have no ergonomic named `uom` alias (`[Pa/m] = [N/m³]`), so they
are documented `f64`, consistent with the crate's f64-internal rule. Every
stored field and method spells out its SI unit in its doc comment.

## Honest scope

This is a **verified foundation, not a validated model** — no cross-code or
experimental benchmark comparison has been run yet (that is the later human
validation step). The inline tests below check the force/moment law against
**hand-computed analytical values** and invariants (Newton's third law, the
exact breakage threshold) only.

It deliberately implements **only** the **linear parallel bond** and nothing
else:

- **No contact-bond variant** (Potyondy & Cundall's alternative point-contact
  bond that carries force but no moment) — only the moment-carrying parallel
  bond is here.
- **No thermal or fluid bond degradation** — the strengths `σ_c`, `τ_c` are
  fixed constants; irradiation-, temperature-, or corrosion-driven weakening
  is out of scope.
- **Incremental small-strain** kinematics only: the force/moment are built
  from per-step relative-displacement and relative-rotation increments (valid
  for the small per-step motions of an explicit DEM loop). There is **no**
  large-rotation reference-frame update of the accumulated shear force /
  bending moment; the caller must keep the DEM time step small.
- **No bond-network solver.** [`Bond::update_bond`] is a *per-bond* force
  evaluation: it returns the force and torque on each particle for **one**
  bond and does **not** integrate the particles, assemble a bond network, or
  own connectivity. A caller's DEM loop applies the returned [`BondForce`]
  (e.g. via [`crate::particle::Particle::integrate`]) and owns which particle
  pairs are bonded.

## References (public literature — NOT LAMMPS/LIGGGHTS source)

- D. O. Potyondy and P. A. Cundall, "A bonded-particle model for rock,"
  *Int. J. Rock Mech. Min. Sci.* **41**(8), 1329–1364 (2004) — the linear
  parallel-bond force/moment–displacement law and the σ/τ breakage criterion
  implemented here.
- P. A. Cundall and O. D. L. Strack, "A discrete numerical model for granular
  assemblies," *Géotechnique* **29**(1), 47–65 (1979) — the incremental
  small-strain DEM force–displacement philosophy the bond update follows.

```rust
pub mod bonded { /* ... */ }
```

### Types

#### Struct `BondForce`

The resolved force and torque a bond applies to its two particles over one
[`Bond::update_bond`] step.

All forces are in newtons `[N]` and torques in newton-metres `[N·m]`. By
Newton's third law `force_on_b = −force_on_a` exactly. The torques are **not**
equal-and-opposite in general: each is the moment of the bond force about
that particle's own centre (different lever arms `r_a` vs `r_b`) plus the
particle's share of the internal bond couple.

```rust
pub struct BondForce {
    pub force_on_a: crate::particle::Vec3,
    pub force_on_b: crate::particle::Vec3,
    pub torque_on_a: crate::particle::Vec3,
    pub torque_on_b: crate::particle::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `force_on_a` | `crate::particle::Vec3` | Total force on particle `a` `[N]` (`F̄_n·n̂ − F̄_s` in the module sign<br>convention: a tensile bond pulls `a` toward `b`). |
| `force_on_b` | `crate::particle::Vec3` | Total force on particle `b` `[N]`. Equals `−force_on_a` exactly. |
| `torque_on_a` | `crate::particle::Vec3` | Torque on particle `a` about its centre `[N·m]`: the moment of<br>`force_on_a` at the contact point (`+r_a·n̂` from `a`'s centre) plus `a`'s<br>half of the bond couple `−M̄`. |
| `torque_on_b` | `crate::particle::Vec3` | Torque on particle `b` about its centre `[N·m]`: the moment of<br>`force_on_b` at the contact point (`−r_b·n̂` from `b`'s centre) plus `b`'s<br>half of the bond couple `+M̄`. |

##### Implementations

###### Methods

- ```rust
  pub const fn zero() -> Self { /* ... */ }
  ```
  The zero load — no force and no torque on either particle. Returned for a

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BondForce { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &BondForce) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Bond`

A **linear parallel bond** (Potyondy & Cundall 2004): a cemented, moment-
carrying elastic bond between two DEM particles.

The bond carries **history-dependent** accumulated state — a normal force, a
shear-force vector, a twisting moment, and a bending-moment vector — that
[`Bond::update_bond`] advances by the elastic increments of each time step
(see the module-level equations). Once the tensile or shear stress criterion
is exceeded the bond is permanently `broken` and carries no load.

# Parameters and units

| Field | Symbol | Quantity | SI unit | Valid range |
|---|---|---|---|---|
| `k_n` | `k̄_n` | normal stiffness per unit area | `[Pa/m] = [N/m³]` | `> 0` |
| `k_s` | `k̄_s` | shear stiffness per unit area | `[Pa/m] = [N/m³]` | `> 0` |
| `radius` | `R̄` | bond (cement cylinder) radius | `[m]` | `> 0` |
| `sigma_c` | `σ_c` | tensile strength | `[Pa]` | `> 0` |
| `tau_c` | `τ_c` | shear strength | `[Pa]` | `> 0` |

The accumulated-state fields (`normal_force`, `shear_force`, `twist_moment`,
`bend_moment`, `broken`) are **not** set by the caller; they start at zero /
intact from [`Bond::new`] and evolve only through [`Bond::update_bond`].

```rust
pub struct Bond {
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
  pub fn new(k_n: f64, k_s: f64, radius: Length, sigma_c: Pressure, tau_c: Pressure) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated, intact parallel bond with zero accumulated load.

- ```rust
  pub fn area(self: &Self) -> f64 { /* ... */ }
  ```
  Bond cross-sectional area `A = π R̄²` `[m²]`.

- ```rust
  pub fn bending_inertia(self: &Self) -> f64 { /* ... */ }
  ```
  Bond second moment of area (bending) `I = π R̄⁴ / 4` `[m⁴]` — the

- ```rust
  pub fn polar_inertia(self: &Self) -> f64 { /* ... */ }
  ```
  Bond polar moment of area (twisting) `J = π R̄⁴ / 2 = 2·I` `[m⁴]` — the

- ```rust
  pub fn normal_force(self: &Self) -> f64 { /* ... */ }
  ```
  Accumulated normal force `F̄_n` `[N]`, tension positive.

- ```rust
  pub fn shear_force(self: &Self) -> Vec3 { /* ... */ }
  ```
  Accumulated shear force `F̄_s` `[N]` (a vector in the contact plane).

- ```rust
  pub fn twist_moment(self: &Self) -> f64 { /* ... */ }
  ```
  Accumulated twisting moment `M̄_t` `[N·m]` about the bond axis `n̂`.

- ```rust
  pub fn bend_moment(self: &Self) -> Vec3 { /* ... */ }
  ```
  Accumulated bending moment `M̄_b` `[N·m]` (a vector ⟂ `n̂`).

- ```rust
  pub fn tensile_stress(self: &Self) -> f64 { /* ... */ }
  ```
  Maximum tensile normal stress on the bond periphery `[Pa]`:

- ```rust
  pub fn shear_stress(self: &Self) -> f64 { /* ... */ }
  ```
  Maximum shear stress on the bond periphery `[Pa]`:

- ```rust
  pub fn is_broken(self: &Self) -> bool { /* ... */ }
  ```
  Whether the bond has failed. Once `true` it stays `true`, and

- ```rust
  pub fn update_bond(self: &mut Self, a: &Particle, b: &Particle, dt: f64) -> BondForce { /* ... */ }
  ```
  Advance the bond's accumulated force/moment by the relative motion of the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Bond { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Bond) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `BondModel`

Closed set of bond models, dispatched by `match` with **no** `dyn` / heap
allocation (per the workspace design rules). This is the per-pair bond state
a solver holds; call [`BondModel::update_bond`] on it each step.

```rust
pub enum BondModel {
    ParallelBond(Bond),
    None,
}
```

##### Variants

###### `ParallelBond`

A linear parallel bond (Potyondy & Cundall 2004) carrying force and
moment until breakage.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Bond` |  |

###### `None`

No cohesive bond between the pair — transmits nothing. Provided so a
caller can hold a uniform `BondModel` per pair and represent "unbonded"
(or a bond that was never formed) without an `Option`.

##### Implementations

###### Methods

- ```rust
  pub fn update_bond(self: &mut Self, a: &Particle, b: &Particle, dt: f64) -> BondForce { /* ... */ }
  ```
  Advance the bond and return the force/torque on each particle, or

- ```rust
  pub fn is_broken(self: &Self) -> bool { /* ... */ }
  ```
  Whether this bond transmits no load. `true` for a broken

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BondModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &BondModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `boundary`

Phase 3 — **Boundaries** (bead `op-t3l.3`).

Geometric domain boundaries a DEM particle can collide with: an infinite
[`Boundary::Plane`], a one-sided half-space [`Boundary::Wall`], an
axis-aligned [`Boundary::Box`] container, and an infinite
[`Boundary::Cylinder`] container. Each primitive answers two purely
**geometric** questions:

- [`Boundary::signed_distance`] — the signed perpendicular distance from a
  query point to the boundary surface `[m]`.
- [`Boundary::particle_overlap`] — whether a sphere of radius `r` penetrates
  the boundary and, if so, by how much (penetration depth `δ`) and along
  which contact normal.

This module is **geometry only**. It computes the overlap `δ` and the
contact normal that a contact-force law would consume — it does **not**
compute forces. The force models (Hooke, Hertz) live in Phase 2's
[`crate::contact`] module; the [`Contact`] returned here is exactly the
geometric hand-off a force model needs. This is an independent
implementation from standard signed-distance geometry, **not** derived from
LIGGGHTS/LAMMPS source (see the crate `NOTICE`).

# Sign conventions (read once, applies to every variant)

All lengths are in metres `[m]`. Let `q` be a query point, `p` a particle of
radius `r` centred at `c = p.position`.

- **`signed_distance(q)`** is **positive when `q` is in the open domain**
  (the free region where particle centres are meant to live) and **negative**
  once `q` has crossed the boundary surface into the forbidden region; its
  magnitude is the perpendicular distance to the nearest surface. For the
  *container* primitives ([`Boundary::Wall`], [`Boundary::Box`],
  [`Boundary::Cylinder`]) the domain is unambiguous. The two-sided
  [`Boundary::Plane`] is the one exception: it has no "inside", so its
  `signed_distance` is the signed offset `(q − point) · n̂`, positive on the
  `+n̂` side (see that variant's docs).

- **`particle_overlap(p)`** returns `Some(`[`Contact`]`)` exactly when the
  sphere surface protrudes past the boundary, i.e. when the penetration depth

  ```text
    δ = r − s > 0,
  ```

  where `s` is the perpendicular distance from the centre `c` to the surface
  (for the container primitives `s = signed_distance(c)`; for the two-sided
  `Plane`, `s = |signed_distance(c)|`). The [`Contact::normal`] `n̂_c` is a
  **unit vector pointing from the boundary surface into the domain** — i.e.
  toward the particle centre while the particle is still inside the domain —
  so a repulsive penalty force `F = k · δ · n̂_c` pushes the particle back
  into the domain. The [`Contact::point`] lies on the boundary surface,
  `c − n̂_c · (r − δ)` (the foot of the perpendicular from `c`).

# Honest scope (Phase 3)

This module implements a **verified geometric foundation only** — it has
**not** been validated against a DEM reference code. Concretely it provides:

- **infinite planes** and **infinite half-space walls** (a `Wall` is a
  half-space plane — see [`Boundary::Wall`]); no finite/bounded planar
  patches,
- **axis-aligned** boxes only (no rotated/oriented boxes),
- **infinite** cylinders only (no capped/finite cylinders, no cones),
- **no meshed / triangulated (STL) walls**,
- **static boundaries only** — no moving/rotating walls, so a contact
  carries no wall velocity and the overlap is purely positional.

For a container primitive (`Box`, `Cylinder`) whose particle straddles a
corner or edge, only the **single nearest face/surface** contact is
returned; simultaneous multi-face contact is not resolved (documented per
variant). The tests below verify each primitive's signed distance and overlap
against hand-computed geometry; no cross-code benchmark comparison has been
run — that is the later human validation step in this bead's Definition of
Done.

# References (public literature / geometry — NOT LAMMPS/LIGGGHTS source)

- C. Ericson, *Real-Time Collision Detection* (Morgan Kaufmann, 2005) —
  point-to-plane / point-to-AABB / point-to-cylinder distance geometry.
- P. J. Schneider and D. H. Eberly, *Geometric Tools for Computer Graphics*
  (Morgan Kaufmann, 2003) — signed-distance and closest-point formulas.
- I. Quílez, "Distance functions" (analytic signed-distance functions,
  public reference), <https://iquilezles.org/articles/distfunctions/> — the
  exact axis-aligned-box signed distance used here.
- P. A. Cundall and O. D. L. Strack, "A discrete numerical model for
  granular assemblies," *Géotechnique* **29**(1), 47–65 (1979) — the DEM
  soft-sphere overlap `δ`.
- T. Pöschel and T. Schwager, *Computational Granular Dynamics: Models and
  Algorithms* (Springer, 2005) — particle–wall overlap and contact-normal
  conventions.

```rust
pub mod boundary { /* ... */ }
```

### Types

#### Struct `Contact`

A single geometric particle–boundary contact: the hand-off a contact-force
law consumes.

This carries **only geometry** — penetration depth and directions, no force.
It is produced by [`Boundary::particle_overlap`] and would be fed to a Phase 2
force model ([`crate::contact`]) to obtain the actual normal force.

# Fields and units

| Field | Quantity | SI unit |
|---|---|---|
| `overlap` | penetration depth `δ = r − s` | `[m]` |
| `normal` | unit contact normal, boundary surface → domain (toward the particle centre) | dimensionless |
| `point` | contact point on the boundary surface (foot of the perpendicular from the centre) | `[m]` |

`overlap` is strictly positive for every `Contact` that
[`Boundary::particle_overlap`] returns (a non-penetrating particle yields
`None`, not a zero-overlap `Contact`). `normal` is a unit vector; applying a
repulsive force `F = k · overlap · normal` pushes the particle back into the
domain.

```rust
pub struct Contact {
    pub overlap: f64,
    pub normal: crate::particle::Vec3,
    pub point: crate::particle::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `overlap` | `f64` | Penetration depth `δ = r − s > 0` `[m]`, where `s` is the perpendicular<br>distance from the particle centre to the boundary surface. |
| `normal` | `crate::particle::Vec3` | Unit contact normal, pointing **from the boundary surface into the<br>domain** — toward the particle centre while the particle is inside.<br>Dimensionless. |
| `point` | `crate::particle::Vec3` | Contact point on the boundary surface `[m]`: the foot of the<br>perpendicular dropped from the particle centre, `c − normal · (r − δ)`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Contact { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Contact) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `Boundary`

A geometric domain boundary a DEM particle can collide with.

Enum dispatch (no `Box<dyn>`), per the workspace design rules: the set of
boundary primitives is closed and known at compile time, so adding a variant
forces every `match` to handle it.

Construct via the validated constructors ([`Boundary::plane`],
[`Boundary::wall`], [`Boundary::aabb`], [`Boundary::cylinder`]), which check
physical validity (non-zero normals/axes, `min < max`, positive radius) and
normalise directions. The fields are public for pattern matching and direct
construction; the query methods normalise defensively, but direct
construction skips the validity checks.

All positions and lengths are in metres `[m]`. See the module-level "Sign
conventions" note for the shared meaning of `signed_distance` and the
contact normal.

```rust
pub enum Boundary {
    Plane {
        point: crate::particle::Vec3,
        normal: crate::particle::Vec3,
    },
    Wall {
        point: crate::particle::Vec3,
        normal: crate::particle::Vec3,
    },
    Box {
        min: crate::particle::Vec3,
        max: crate::particle::Vec3,
    },
    Cylinder {
        axis_point: crate::particle::Vec3,
        axis_dir: crate::particle::Vec3,
        radius: f64,
    },
}
```

##### Variants

###### `Plane`

An **infinite, two-sided** plane (a thin planar divider).

The plane passes through `point` `[m]` with unit outward-reference
`normal` `n̂`. Being two-sided, it has no "inside": its
[`Boundary::signed_distance`] is the signed offset `(q − point) · n̂`
(positive on the `+n̂` side, negative on the `−n̂` side, zero on the
surface), and a particle overlaps it from **either** side. The contact
normal flips to point from the plane toward whichever side the particle
centre is on. Use this for an internal splitter plate; for a one-sided
solid barrier (a floor) use [`Boundary::Wall`] instead.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `point` | `crate::particle::Vec3` | A point the plane passes through `[m]`. |
| `normal` | `crate::particle::Vec3` | Unit plane normal (dimensionless); `+n̂` labels the positive side. |

###### `Wall`

A **one-sided half-space wall**: solid material fills the closed
half-space on the `−normal` side, and the open domain is the `+normal`
side.

The wall face passes through `point` `[m]` with unit `normal` `n̂`
pointing **out of the wall, into the domain** (toward the particles). Its
[`Boundary::signed_distance`] is `(q − point) · n̂` = the perpendicular
distance into the domain (negative once inside the wall material). Unlike
[`Boundary::Plane`], a `Wall` only ever pushes along its fixed outward
normal `+n̂`: a particle overlaps when its centre is within `r` of the
face (including having passed through into the material), and the contact
normal is always `n̂`. This is the standard DEM floor/wall half-space.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `point` | `crate::particle::Vec3` | A point on the wall face `[m]`. |
| `normal` | `crate::particle::Vec3` | Unit outward normal (dimensionless), from the wall into the domain. |

###### `Box`

An **axis-aligned box** container; the open domain is the box interior.

`min` and `max` `[m]` are the lower and upper corners, with
`min.x < max.x`, `min.y < max.y`, `min.z < max.z`. Particles live inside;
[`Boundary::signed_distance`] is positive in the interior (equal to the
distance to the nearest face) and negative outside. Overlap is resolved
against the **single nearest interior face** (its inward normal is one of
`±x`, `±y`, `±z`); simultaneous two/three-face corner contact is not
resolved — the nearest (deepest-penetration) face is returned.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `min` | `crate::particle::Vec3` | Lower corner `[m]` (`min.i < max.i` on every axis `i`). |
| `max` | `crate::particle::Vec3` | Upper corner `[m]` (`max.i > min.i` on every axis `i`). |

###### `Cylinder`

An **infinite circular cylinder** container; the open domain is the
cylinder interior.

The axis passes through `axis_point` `[m]` along unit direction
`axis_dir`, and the cylindrical wall sits at radial distance `radius`
`[m] > 0` from the axis. Particles live inside;
[`Boundary::signed_distance`] is `radius − ρ` where `ρ` is the query
point's perpendicular distance from the axis (positive inside, negative
outside). The contact normal points radially inward (toward the axis).
The cylinder is infinite along its axis (no end caps). A particle whose
centre lies exactly on the axis has no defined radial direction and
yields `None` from [`Boundary::particle_overlap`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `axis_point` | `crate::particle::Vec3` | A point on the cylinder axis `[m]`. |
| `axis_dir` | `crate::particle::Vec3` | Unit axis direction (dimensionless). |
| `radius` | `f64` | Cylinder radius `[m]`, strictly positive. |

##### Implementations

###### Methods

- ```rust
  pub fn plane(point: Vec3, normal: Vec3) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct an infinite two-sided [`Boundary::Plane`] through `point` with

- ```rust
  pub fn wall(point: Vec3, normal: Vec3) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a one-sided half-space [`Boundary::Wall`] whose face passes

- ```rust
  pub fn aabb(min: Vec3, max: Vec3) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct an axis-aligned [`Boundary::Box`] with lower corner `min` and

- ```rust
  pub fn cylinder(axis_point: Vec3, axis_dir: Vec3, radius: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct an infinite [`Boundary::Cylinder`] with the given `axis_point`,

- ```rust
  pub fn signed_distance(self: &Self, point: Vec3) -> f64 { /* ... */ }
  ```
  Signed perpendicular distance from `point` to this boundary's surface

- ```rust
  pub fn particle_overlap(self: &Self, p: &Particle) -> Option<Contact> { /* ... */ }
  ```
  Geometric overlap of particle `p` (a sphere of radius `r = p.radius`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `compute`

Compute-backend selector for the DEM timestep and the bulk analysis kernels.

A single [`ComputeType`] value chooses *how* work executes — on one CPU
thread, across all CPU cores with [`rayon`], or with a GPU kernel where one
exists. The **physics is identical** across backends; only the execution
strategy differs. Enum dispatch is used deliberately — no trait objects —
so every `match self { … }` site is exhaustively checked at compile time
(workspace `CLAUDE.md`, "No trait objects").

This mirrors [`outram_mc_libs::physics::compute::ComputeType`] deliberately,
so the two pillars of the pebble-bed workflow present the same selector to a
caller. **One semantic differs, and it matters** — see the trust model
below.

# Trust model — stricter here than in the Monte Carlo pillar

In `outram-mc-libs` the multi-thread backend is documented as agreeing with
the single-thread reference only *within statistical uncertainty*, because
parallel transport restructures the RNG streams.

**This crate holds its parallel backend to bit-identity instead**, and
tests it. That is not gold-plating: the crate's entire verification claim
is that it reproduces upstream LIGGGHTS *bit-for-bit* on the deterministic
cases and to 61 µm median per-pebble on the HTR-10 bed. A backend that
perturbed the last bits would force every one of those comparisons to be
re-qualified per backend, and would quietly convert an exact claim into a
tolerance. So [`CpuMultiThread`](ComputeType::CpuMultiThread) is required to
reproduce [`CpuSingleThread`](ComputeType::CpuSingleThread) exactly, for
any thread count.

Achieving that is a design constraint on the force loop, not an accident:
floating-point addition is not associative, so the parallel path computes
per-contact forces concurrently but **accumulates them in the serial pair
order**. See [`crate::granular_system::GranularSystem::compute_forces`].

# Portability

The enum and every driver that dispatches on it compile on **all** targets.
On `wasm32` (no OS threads) the rayon call sites use [`crate::wasm_par`] and
run serially — which, given the bit-identity property above, is *numerically
exact*, not a degraded fallback. On Android the GPU module is target-gated
out and [`Gpu`](ComputeType::Gpu) transparently runs the CPU path, so
selecting it is always safe.

`Eq` is deliberately **not** derived: [`ThreadCount::Fraction`] carries an
`f64`, which is only `PartialEq`.

```rust
pub mod compute { /* ... */ }
```

### Types

#### Enum `ComputeType`

Which compute backend a DEM driver or bulk analysis kernel uses.

| This enum | Meaning |
|---|---|
| [`CpuSingleThread`](Self::CpuSingleThread) | scalar, single thread — the trusted reference |
| [`CpuMultiThread`](Self::CpuMultiThread) | rayon-parallel, **bit-identical** to the reference |
| [`Gpu`](Self::Gpu) | GPU kernel where one exists, CPU fallback otherwise |

# Which kernels honour it

| Kernel | `CpuSingleThread` | `CpuMultiThread` | `Gpu` |
|---|---|---|---|
| [`GranularSystem::step`](crate::granular_system::GranularSystem::step) | yes | yes | falls back to CPU |
| [`rdf`](crate::rdf) pair-separation histogram | yes | yes | **yes** |

**The timestep has no GPU path, deliberately.** DEM's expensive inner loop
carries a persistent per-contact tangential shear history
([`ShearHistory`](crate::granular::ShearHistory)) with contacts born and
dying every step — a stateful gather/scatter structure that is the *worst*
shape for a shader, while the Hertz force arithmetic that would port well is
not where the time goes. At HTR-10 scale (27 554 pebbles, ~165 000 live
contacts) the per-step working set is also far too small to amortise a
host↔device round trip unless the entire integrator became GPU-resident.
Selecting [`Gpu`](Self::Gpu) for a timestep is therefore not an error — it
runs the CPU path.

The radial distribution function is the opposite shape, which is why it has
a real GPU kernel: an all-pairs distance histogram over 27 554 positions is
3.8e8 stateless, independent pair evaluations.

```rust
pub enum ComputeType {
    CpuSingleThread,
    CpuMultiThread(ThreadCount),
    Gpu,
}
```

##### Variants

###### `CpuSingleThread`

Scalar, single-thread execution — the **deterministic trusted
reference**, and the default.

Every committed cross-code comparison in
`crates/outram-park-fork-liggghts/docs/` was produced on this backend.

###### `CpuMultiThread`

Rayon-parallel execution, sized by the carried [`ThreadCount`].

**Bit-identical to [`CpuSingleThread`](Self::CpuSingleThread)** for any
thread count — see the module-level trust model for why that is required
rather than merely nice, and
[`GranularSystem::compute_forces`](crate::granular_system::GranularSystem::compute_forces)
for how the accumulation order is preserved.

Work runs in a **dedicated pool** sized to [`ThreadCount`], never the
implicit global rayon pool, so a caller that is itself inside a rayon
scope cannot deadlock or oversubscribe.

Construct the default form with `CpuMultiThread(ThreadCount::Auto)`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ThreadCount` |  |

###### `Gpu`

GPU-accelerated execution where a kernel exists, with graceful CPU
fallback.

Today exactly one kernel is GPU-backed: the [`rdf`](crate::rdf) pair
separation histogram. Everything else runs the CPU path. If no GPU
adapter is available — a headless server, CI with no Vulkan loader, or
Android where the GPU module is compiled out — the driver falls back to
the CPU path. It **never errors on a missing GPU.**

GPU results are acceleration only and are held to a tolerance against
the CPU reference, never trusted above it.

##### Implementations

###### Methods

- ```rust
  pub fn threads(self: Self) -> usize { /* ... */ }
  ```
  The worker-thread count this backend will actually use (always `>= 1`).

- ```rust
  pub fn is_parallel_step(self: Self) -> bool { /* ... */ }
  ```
  Whether this backend runs the DEM timestep on more than one thread.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ComputeType { /* ... */ }
    ```

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
    fn default() -> ComputeType { /* ... */ }
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
    fn eq(self: &Self, other: &ComputeType) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `ThreadCount`

How many worker threads the [`ComputeType::CpuMultiThread`] backend uses,
sized to the CPU's strength.

Resolved to a concrete positive count with [`ThreadCount::resolve`], which
then sizes a dedicated [`rayon::ThreadPool`]. The default is
[`Auto`](Self::Auto), which reads the machine's logical core count via
[`std::thread::available_parallelism`] — a desktop naturally gets many
threads, an Android phone gets few, with no special-casing. All variants
resolve to **at least 1**.

```rust
pub enum ThreadCount {
    Auto,
    Fixed(usize),
    Fraction(f64),
}
```

##### Variants

###### `Auto`

Use every logical core: [`std::thread::available_parallelism`]. Falls
back to 1 if the query fails. The default.

###### `Fixed`

An explicit worker-thread count. Clamped up to a minimum of 1.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Fraction`

A fraction of the available logical cores, e.g. `0.5` = half. The
product `fraction * cores` is rounded to nearest and clamped to at least
1, so any positive fraction yields a runnable pool.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn resolve(self: Self) -> usize { /* ... */ }
  ```
  Resolve to a concrete worker-thread count (always `>= 1`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThreadCount { /* ... */ }
    ```

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
    fn default() -> ThreadCount { /* ... */ }
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
    fn eq(self: &Self, other: &ThreadCount) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `contact`

Phase 2 — **Contact mechanics** (bead `op-t3l.2`).

Pairwise particle–particle contact forces for spherical DEM particles, in
two standard flavours selected at compile time through the [`ContactModel`]
enum (no `dyn`, per the workspace design rules):

- [`HookeContact`] — the **linear spring-dashpot** law of Cundall & Strack
  (1979) with a tangential spring-dashpot and a Coulomb friction cap
  (Tsuji et al. 1992).
- [`HertzContact`] — the **nonlinear Hertz–Mindlin** law: a Hertzian
  (`δ^{3/2}`) normal spring, viscoelastic (restitution-based) damping, and a
  Mindlin tangential spring with a Coulomb cap (Hertz 1882; Mindlin &
  Deresiewicz 1953; Tsuji et al. 1992; Di Renzo & Di Maio 2004).

Both share the same [`ContactLaw`] contract (the compiler-enforced trait)
and the same geometry/assembly code, so Newton's third law and the
contact-point torque bookkeeping are implemented **once** and reused.

## Naming note (trait vs enum)

The workspace idiom is a *trait* that states the contract plus an *enum*
that dispatches over the closed set of models (mirroring the `CLAUDE.md`
`TurbulenceKernel` trait / `TurbulenceModel` enum example). A Rust trait and
enum cannot share one identifier (both live in the type namespace), so the
contract trait is [`ContactLaw`] and the public dispatch enum — the type a
user actually holds and calls [`ContactModel::contact_force`] on — is
[`ContactModel`].

# Sign & geometry conventions (read once, applies everywhere)

For a pair `(a, b)`:

- **Normal** `n̂` is the unit vector **from `a`'s centre toward `b`'s
  centre**: `n̂ = (x_b − x_a) / ‖x_b − x_a‖`.
- **Overlap** `δ_n = (r_a + r_b) − ‖x_b − x_a‖`. Contact exists iff
  `δ_n > 0`; otherwise [`ContactLaw::contact_force`] returns `None`.
- **Approach rate** `v_n = (v_a − v_b) · n̂` `[m/s]`, positive when the two
  centres are closing.
- The **normal force on `a` is repulsive**, i.e. directed along `−n̂`; the
  force on `b` is its Newton-third-law reaction `+…n̂`.

# Unit convention

Following the crate convention (see [`crate::particle`]), the `uom` boundary
sits at the constructors where a clean named `uom` type exists
([`HertzContact::new`] takes Young's modulus as a [`Pressure`]); everything
else is plain `f64` in **SI base units** with the unit spelled out in the
doc comment. The spring/damping constants of the Hooke model have no
ergonomic named `uom` alias (`k_n` is `[N/m]`, `γ_n` is `[N·s/m] = [kg/s]`),
so they are documented `f64`, consistent with the crate's f64-internal rule.

# Honest scope (Phase 2)

This is a **verified foundation, not a validated model** — no cross-code or
experimental benchmark comparison has been run yet (that is the later human
validation step in this bead's Definition of Done). The inline tests below
check the force laws against **hand-computed analytical values** and
invariants (Newton's third law, `δ^{3/2}` scaling, the Coulomb cap) only.

It deliberately implements **only**:

- **Sphere–sphere** contact. Particle–wall/boundary contact is Phase 3
  ([`crate::boundary`]); non-spherical shapes are out of scope entirely.
- A **stateless snapshot** force: [`ContactLaw::contact_force`] is a pure
  function of the two particles' instantaneous state. A full tangential
  spring needs the **accumulated tangential displacement** `ξ_t` integrated
  over the contact's lifetime, which is per-contact history the caller's
  time-integration loop must carry (a later phase). Here `ξ_t = 0`, so the
  tangential force reduces to its **dashpot** term `γ_t · v_t`, still
  Coulomb-capped at `μ|F_n|`. The tangential *stiffness* `k_t`
  ([`ContactLaw::tangential_spring_coeff`]) is exposed for a future
  history-aware caller.

It does **not** yet provide: rolling friction, cohesion/adhesion (van der
Waals, liquid bridges), bonded/parallel-bond contacts, or heat transfer
through the contact (thermal DEM is Phase 4, [`crate::thermal`]). The normal
force is **not clamped** to be purely repulsive: for a rapidly rebounding
contact the linear/viscoelastic damping term can momentarily exceed the
elastic term and yield a small tensile force — this is a known
spring-dashpot artifact, **not** a cohesion model, and clamping `F_n ≥ 0` is
a documented option a caller may add.

# References (public literature — NOT LAMMPS/LIGGGHTS source)

- P. A. Cundall and O. D. L. Strack, "A discrete numerical model for
  granular assemblies," *Géotechnique* **29**(1), 47–65 (1979) — linear
  spring-dashpot (Hooke) contact.
- H. Hertz, "Über die Berührung fester elastischer Körper," *J. reine angew.
  Math.* **92**, 156–171 (1882) — Hertzian normal contact (`δ^{3/2}`).
- R. D. Mindlin and H. Deresiewicz, "Elastic spheres in contact under
  varying oblique forces," *J. Appl. Mech.* **20**, 327–344 (1953) —
  tangential contact stiffness.
- Y. Tsuji, T. Tanaka, T. Ishida, "Lagrangian numerical simulation of plug
  flow of cohesionless particles in a horizontal pipe," *Powder Technol.*
  **71**(3), 239–250 (1992) — nonlinear viscoelastic damping and the
  Coulomb-capped tangential spring-dashpot.
- A. Di Renzo and F. P. Di Maio, "Comparison of contact-force models for the
  simulation of collisions in DEM-based granular flow codes," *Chem. Eng.
  Sci.* **59**(3), 525–541 (2004) — the restitution-based damping
  coefficient and Hertz–Mindlin assembly used here.

```rust
pub mod contact { /* ... */ }
```

### Types

#### Struct `ContactForce`

The resolved contact force (and torque) for one overlapping particle pair.

All forces are in newtons `[N]` and torques in newton-metres `[N·m]`. By
Newton's third law `force_on_b = −force_on_a`. Only the tangential
(friction) part produces a torque: the normal force is collinear with the
line of centres and so has zero moment about either centre.

```rust
pub struct ContactForce {
    pub force_on_a: crate::particle::Vec3,
    pub force_on_b: crate::particle::Vec3,
    pub torque_on_a: crate::particle::Vec3,
    pub torque_on_b: crate::particle::Vec3,
    pub overlap: f64,
    pub normal: crate::particle::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `force_on_a` | `crate::particle::Vec3` | Total force on particle `a` `[N]` (normal repulsion along `−n̂` plus<br>tangential friction). |
| `force_on_b` | `crate::particle::Vec3` | Total force on particle `b` `[N]`. Equals `−force_on_a` exactly. |
| `torque_on_a` | `crate::particle::Vec3` | Torque on particle `a` about its centre `[N·m]`, from the tangential<br>force acting at the contact point (offset `r_a·n̂` from `a`'s centre). |
| `torque_on_b` | `crate::particle::Vec3` | Torque on particle `b` about its centre `[N·m]`, from the reaction<br>tangential force at the contact point (offset `−r_b·n̂` from `b`'s<br>centre). |
| `overlap` | `f64` | Normal overlap `δ_n` `[m]`, strictly positive (a returned<br>[`ContactForce`] always corresponds to a real overlap). |
| `normal` | `crate::particle::Vec3` | Unit contact normal `n̂`, pointing from `a`'s centre toward `b`'s centre. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ContactForce { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ContactForce) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `HookeContact`

Linear **spring-dashpot** contact model (Cundall & Strack 1979; tangential
Coulomb-capped spring-dashpot from Tsuji et al. 1992).

Normal force magnitude: `F_n = k_n·δ_n + γ_n·v_n`, with overlap `δ_n` `[m]`
and approach rate `v_n` `[m/s]`. (Equivalently `k_n·δ_n − γ_n·ẋ` where
`ẋ = −v_n` is the rate of change of centre separation.) Tangential force:
`min(k_t·ξ_t + γ_t·v_t, μ|F_n|)` opposing slip; the stateless snapshot uses
`ξ_t = 0`.

# Parameters and units

| Field | Symbol | Quantity | SI unit | Valid range |
|---|---|---|---|---|
| `normal_stiffness` | `k_n` | normal spring stiffness | `[N/m]` | `> 0` |
| `normal_damping` | `γ_n` | normal dashpot coefficient | `[N·s/m] = [kg/s]` | `≥ 0` |
| `tangential_stiffness` | `k_t` | tangential spring stiffness | `[N/m]` | `≥ 0` |
| `tangential_damping` | `γ_t` | tangential dashpot coefficient | `[N·s/m] = [kg/s]` | `≥ 0` |
| `friction` | `μ` | Coulomb friction coefficient | `[-]` | `≥ 0` |

The stiffnesses are constant (linear model); they do not depend on overlap
or radius, so `R*` and `m*` are ignored by this model's scalar methods.

```rust
pub struct HookeContact {
    pub normal_stiffness: f64,
    pub normal_damping: f64,
    pub tangential_stiffness: f64,
    pub tangential_damping: f64,
    pub friction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `normal_stiffness` | `f64` | Normal spring stiffness `k_n` `[N/m]`. Strictly positive. |
| `normal_damping` | `f64` | Normal dashpot coefficient `γ_n` `[N·s/m]`. Non-negative. |
| `tangential_stiffness` | `f64` | Tangential spring stiffness `k_t` `[N/m]`. Non-negative. |
| `tangential_damping` | `f64` | Tangential dashpot coefficient `γ_t` `[N·s/m]`. Non-negative. |
| `friction` | `f64` | Coulomb friction coefficient `μ` `[-]`. Non-negative. |

##### Implementations

###### Methods

- ```rust
  pub fn new(normal_stiffness: f64, normal_damping: f64, tangential_stiffness: f64, tangential_damping: f64, friction: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated linear spring-dashpot model.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HookeContact { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContactLaw**
  - ```rust
    fn normal_force_scalar(self: &Self, delta_n: f64, v_n: f64, _r_eff: f64, _m_eff: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn tangential_damping_coeff(self: &Self, _delta_n: f64, _r_eff: f64, _m_eff: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn tangential_spring_coeff(self: &Self, _delta_n: f64, _r_eff: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn friction_coefficient(self: &Self) -> f64 { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &HookeContact) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `HertzContact`

Nonlinear **Hertz–Mindlin** contact model (Hertz 1882; Mindlin &
Deresiewicz 1953; viscoelastic damping and Coulomb-capped tangential spring
from Tsuji et al. 1992 / Di Renzo & Di Maio 2004).

Normal force magnitude:

`F_n = (4/3)·E*·√(R*)·δ_n^{3/2} + c_n·v_n`,

with effective modulus `E*`, effective radius `R*`, overlap `δ_n`, approach
rate `v_n`, and a restitution-based damping coefficient
`c_n = 2·√(5/6)·|β|·√(S_n·m*)`, where the normal contact stiffness is
`S_n = 2·E*·√(R*·δ_n)` and `β = ln(e)/√(ln²(e) + π²)`.

Tangential stiffness (Mindlin): `S_t = 8·G*·√(R*·δ_n)`, with tangential
damping `γ_t = 2·√(5/6)·|β|·√(S_t·m*)`; the tangential force
`min(S_t·ξ_t + γ_t·v_t, μ|F_n|)` opposes slip (`ξ_t = 0` in the stateless
snapshot).

# Material assumption

This model assumes **both particles share one isotropic linear-elastic
material** — a single Young's modulus `E`, Poisson ratio `ν`, and
coefficient of restitution `e`. The effective moduli then reduce to
`E* = E / (2(1 − ν²))` and `G* = G / (2(2 − ν))` with `G = E / (2(1 + ν))`.
Mixed-material effective moduli (`1/E* = Σ (1 − ν_i²)/E_i`) are a documented
future extension.

# Parameters and units

| Field | Symbol | Quantity | SI unit | Valid range |
|---|---|---|---|---|
| `youngs_modulus` | `E` | Young's modulus | `[Pa]` | `> 0` |
| `poisson_ratio` | `ν` | Poisson ratio | `[-]` | `0 ≤ ν < 0.5` |
| `restitution` | `e` | coefficient of restitution | `[-]` | `0 < e ≤ 1` |
| `friction` | `μ` | Coulomb friction coefficient | `[-]` | `≥ 0` |

```rust
pub struct HertzContact {
    pub youngs_modulus: f64,
    pub poisson_ratio: f64,
    pub restitution: f64,
    pub friction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `youngs_modulus` | `f64` | Young's modulus `E` `[Pa]` (shared by both particles). Strictly positive. |
| `poisson_ratio` | `f64` | Poisson ratio `ν` `[-]`. In `[0, 0.5)`. |
| `restitution` | `f64` | Coefficient of restitution `e` `[-]`. In `(0, 1]`. |
| `friction` | `f64` | Coulomb friction coefficient `μ` `[-]`. Non-negative. |

##### Implementations

###### Methods

- ```rust
  pub fn new(youngs_modulus: Pressure, poisson_ratio: f64, restitution: f64, friction: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated Hertz–Mindlin model.

- ```rust
  pub fn effective_modulus(self: &Self) -> f64 { /* ... */ }
  ```
  Effective (reduced) Young's modulus `E*` `[Pa]` for the equal-material

- ```rust
  pub fn effective_shear_modulus(self: &Self) -> f64 { /* ... */ }
  ```
  Effective (reduced) shear modulus `G*` `[Pa]` for the equal-material

- ```rust
  pub fn damping_beta(self: &Self) -> f64 { /* ... */ }
  ```
  Damping factor `β = ln(e) / √(ln²(e) + π²)` `[-]` (Tsuji 1992 /

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HertzContact { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **ContactLaw**
  - ```rust
    fn normal_force_scalar(self: &Self, delta_n: f64, v_n: f64, r_eff: f64, m_eff: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn tangential_damping_coeff(self: &Self, delta_n: f64, r_eff: f64, m_eff: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn tangential_spring_coeff(self: &Self, delta_n: f64, r_eff: f64) -> f64 { /* ... */ }
    ```

  - ```rust
    fn friction_coefficient(self: &Self) -> f64 { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &HertzContact) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `ContactModel`

Closed set of contact models, dispatched by `match` with **no** `dyn` /
heap allocation (per the workspace design rules). This is the type a solver
holds per material pair; call [`ContactModel::contact_force`] on it.

```rust
pub enum ContactModel {
    Hooke(HookeContact),
    Hertz(HertzContact),
}
```

##### Variants

###### `Hooke`

Linear spring-dashpot (Cundall & Strack 1979).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `HookeContact` |  |

###### `Hertz`

Nonlinear Hertz–Mindlin (Hertz 1882; Mindlin & Deresiewicz 1953).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `HertzContact` |  |

##### Implementations

###### Methods

- ```rust
  pub fn contact_force(self: &Self, a: &Particle, b: &Particle) -> Option<ContactForce> { /* ... */ }
  ```
  Pairwise contact force between spheres `a` and `b`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ContactModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ContactModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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

#### Trait `ContactLaw`

Compiler-enforced contract every contact model satisfies (the trait half of
the workspace's "trait states the interface, enum dispatches" idiom).

A model supplies four **scalar** force ingredients as pure functions of the
local contact geometry; the provided [`ContactLaw::contact_force`] method
then assembles them into a full [`ContactForce`] (geometry, Coulomb cap,
Newton's third law, contact-point torque) so that assembly logic is written
and verified once and shared by every model.

# Effective (reduced) contact quantities

Several methods take reduced pair quantities computed from the two
particles:

- **Effective radius** `R* = r_a·r_b / (r_a + r_b)` `[m]`.
- **Effective mass** `m* = m_a·m_b / (m_a + m_b)` `[kg]`.

```rust
pub trait ContactLaw {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `normal_force_scalar`: Repulsive **normal** force magnitude `[N]` (positive pushes the pair
- `tangential_damping_coeff`: **Tangential dashpot** coefficient `γ_t` `[N·s/m] = [kg/s]` for the given
- `tangential_spring_coeff`: **Tangential spring** stiffness `k_t` `[N/m]`, paired with the
- `friction_coefficient`: Coulomb sliding-friction coefficient `μ` `[-]`. The tangential force

##### Provided Methods

- ```rust
  fn contact_force(self: &Self, a: &Particle, b: &Particle) -> Option<ContactForce> { /* ... */ }
  ```
  Assemble the full pairwise contact force between spheres `a` and `b`.

##### Implementations

This trait is implemented for the following types:

- `HookeContact`
- `HertzContact`

## Module `coupling`

Phase 5 — **Future CFD-DEM coupling** (bead `op-t3l.5`).

# ⚠️ RESERVED ARCHITECTURE ONLY — NO PHYSICS IS IMPLEMENTED HERE

This module is a **deliberately minimal, documentation-first interface
layer**. It reserves the *seam* where the OUTRAM-FOAM CFD/multiphase side
(crate `outram-foam-multiphase`, bead `op-2kk`) will one day exchange data
with this granular-DEM library (beads `op-t3l.1`–`op-t3l.4`). It defines the
**shape** of that exchange — enums, traits, and unit-typed data records —
and **nothing else**. Every behavioural method returns
[`DemError::NotImplemented`]. There is **no drag law, no interpolation, no
volume averaging, and no fluid solve** in this file, and none is faked: an
honest `NotImplemented` is the *correct and intended* state of this phase,
not a shortfall. Do not read any returned number as physically meaningful —
there are none to return yet.

# Why a seam, not a dependency (Phase II separation principle)

The three OUTRAM PARK physics pillars — thermophysical properties
(`tampines`), CFD/multiphase (`outram-foam-multiphase`),
and granular DEM (**this crate**) — are kept as **independent** crates. This
module therefore **must not** add a dependency on the CFD crate: coupling is
expressed as an *interface* (traits the CFD side will implement, traits the
DEM side will implement) rather than a compile-time link. That keeps each
pillar independently buildable, testable, and publishable, and keeps this
crate Android-buildable (pure-Rust, no CFD/BLAS pull-in). The two sides meet
only at run time, through the trait objects-by-generics wiring sketched in
[`ReservedCoupling::couple_particle`] — no `dyn`, no `Box`, per the
workspace design rules.

# Intended volume-averaging / interpolation seam (design note)

Unresolved (point-particle) CFD-DEM coupling exchanges two kinds of data at
every DEM particle, and both cross a **spatial-averaging boundary** that is
the crux of the eventual implementation:

- **CFD → DEM (interpolation / sampling).** The fluid solver holds cell-
  averaged fields (velocity, pressure, void fraction). To force a *particle*
  the DEM side needs those fields **interpolated to the particle centre**
  (or, more carefully, filtered over the particle's neighbourhood so the
  particle does not "feel" its own back-reaction). [`LocalFluidState`]
  is the reserved record for that sampled snapshot at one particle.
- **DEM → CFD (volume averaging / projection).** The momentum the particles
  remove from the fluid, and the space they occupy, must be **averaged back
  onto the CFD mesh** — a per-cell solid volume fraction and a per-cell
  momentum sink. [`CouplingExchange`] is the reserved record for one
  particle's contribution: the drag force it received, and the particle
  volume fraction it projects back.

The averaging kernel (nearest-cell, divided/diffused, statistical-kernel,
or coarse-grained "parcel" filtering), the void-fraction definition, and the
two-way momentum-conservation bookkeeping are **the physics to be designed
later**, once both the CFD side (`op-2kk`) and the DEM side (`op-t3l.1`
through `op-t3l.4`: particles, contact, boundaries, thermal) have matured
enough to pin the data contract down. Until then this module only *names*
the quantities and their units so that day-one implementation has a typed,
documented target to fill in.

# Selected literature (public; for the eventual implementation)

- R. Sun and H. Xiao, "Diffusion-based coarse graining in hybrid
  continuum–discrete solvers," *Int. J. Multiphase Flow* **72**, 233–247
  (2015).
- Z. Peng et al., "Influence of void fraction calculation on the numerical
  simulation of gas–solid flows by CFD-DEM," *Powder Technol.* **265**,
  26–39 (2014).
- R. Garg, J. Galvin, T. Li, S. Pannala, "Open-source MFIX-DEM software for
  gas–solids flows," *Powder Technol.* **220**, 122–137 (2012).
- C. Goniva, C. Kloss, N. G. Deen, J. A. M. Kuipers, S. Pirker,
  "Influence of rolling friction on single spout fluidized bed simulation,"
  *Particuology* **10**(5), 582–591 (2012) — the CFDEM/LIGGGHTS coupling
  approach this seam is modelled after.

```rust
pub mod coupling { /* ... */ }
```

### Types

#### Enum `CouplingScheme`

The direction and fidelity of momentum exchange across the CFD-DEM seam.

This closed set of coupling regimes is dispatched by `match` (enum dispatch
per the workspace design rules — no trait objects). It selects *which* of
the reserved data paths a future driver would actually walk; it carries no
physics itself.

# Variants

- [`OneWay`](CouplingScheme::OneWay) — the fluid drives the particles but the
  particles do **not** feed momentum or volume back to the fluid. The CFD
  solve is independent of the DEM state; only the CFD → DEM interpolation
  path (sampling [`LocalFluidState`]) is exercised. Valid physical regime:
  dilute suspensions where the particle volume fraction is small enough
  (typically a solid fraction `≲ 1e-3`) that back-reaction is negligible.
- [`TwoWay`](CouplingScheme::TwoWay) — momentum is exchanged in **both**
  directions: the fluid drags the particles, and each particle's reaction
  force is projected back onto the fluid as a per-cell momentum sink. Both
  the interpolation and the volume-averaging/projection paths are exercised.
  Valid regime: moderate loadings where drag back-reaction matters but the
  local void fraction is still near unity.
- [`VolumeFiltered`](CouplingScheme::VolumeFiltered) — full "four-way"-style
  volume-filtered / coarse-grained coupling: in addition to two-way momentum
  exchange, the fluid equations carry the **particle volume fraction**
  explicitly (the void fraction departs meaningfully from unity), so the
  solid phase displaces fluid. This is the dense-bed regime (e.g. a
  pebble/packed bed), and the one where the volume-averaging kernel choice
  (see the module design note) dominates accuracy.

# Status

Reserved only. No variant drives any computation in this phase — the enum
exists so the eventual driver, and the trait method docs, can name the
regime they apply to.

```rust
pub enum CouplingScheme {
    OneWay,
    TwoWay,
    VolumeFiltered,
}
```

##### Variants

###### `OneWay`

Fluid → particles only; no back-reaction (dilute regime). See the type
docs for the valid physical range.

###### `TwoWay`

Bidirectional momentum exchange; void fraction still ≈ 1 (moderate
loading). See the type docs.

###### `VolumeFiltered`

Volume-filtered / coarse-grained; particle volume fraction enters the
fluid equations (dense regime). See the type docs.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CouplingScheme { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &CouplingScheme) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `LocalFluidState`

A snapshot of the **CFD fluid field sampled at one particle's location** —
the data the CFD side (`outram-foam-multiphase`, bead `op-2kk`) would
provide to the DEM side each coupling step (the CFD → DEM interpolation
path).

This is a plain unit-carrying record: it holds *what the fluid looks like*
at a single point, already interpolated/filtered from the CFD mesh by the
provider. It performs no interpolation itself (that is the provider's job)
and no physics.

# Fields, quantities, and units

| Field | Physical quantity | Unit | Valid range |
|---|---|---|---|
| `velocity` | local fluid (continuous-phase) velocity `u_f` | `[m/s]` | any finite vector |
| `pressure_gradient` | local fluid pressure gradient `∇p` | `[Pa/m]` | any finite vector |
| `void_fraction` | fluid volume fraction `ε_f` (fraction of the local cell occupied by fluid) | dimensionless | `(0, 1]` |

The two vector quantities use [`Vec3`] (unitless container; the SI unit is
fixed by this documentation, following the crate convention in
[`crate::particle`]). `void_fraction` uses the `uom` dimensionless
[`Ratio`] so it cannot be confused with a dimensional scalar at a call site;
physically it satisfies `ε_f = 1 - ε_s` with the particle (solid) volume
fraction `ε_s`.

```rust
pub struct LocalFluidState {
    pub velocity: crate::particle::Vec3,
    pub pressure_gradient: crate::particle::Vec3,
    pub void_fraction: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `velocity` | `crate::particle::Vec3` | Local fluid velocity `u_f` `[m/s]` at the particle centre (interpolated<br>from the CFD field by the provider). |
| `pressure_gradient` | `crate::particle::Vec3` | Local fluid pressure gradient `∇p` `[Pa/m]` at the particle centre. |
| `void_fraction` | `uom::si::f64::Ratio` | Local fluid volume fraction `ε_f` (dimensionless, `(0, 1]`): the fraction<br>of the surrounding averaging volume occupied by fluid rather than solid. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LocalFluidState { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &LocalFluidState) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `CouplingExchange`

One particle's **contribution back to the CFD solve** — the data the DEM
side returns each coupling step (the DEM → CFD volume-averaging/projection
path).

# Fields, quantities, and units

| Field | Physical quantity | Unit | Sign / range |
|---|---|---|---|
| `drag_force` | fluid → particle drag force applied to the DEM particle | `[N]` | any finite vector; its negative is the momentum sink the fluid feels |
| `particle_volume_fraction` | solid volume fraction `ε_s` this particle projects onto its CFD cell(s) | dimensionless | `[0, 1)` |

The `drag_force` is what the DEM integrator would add to a particle's force
balance; by Newton's third law the equal-and-opposite reaction is the
per-cell momentum sink handed to the fluid under
[`CouplingScheme::TwoWay`]/[`CouplingScheme::VolumeFiltered`].
`particle_volume_fraction` is `ε_s = 1 - ε_f`, the quantity the fluid
continuity/momentum equations need under [`CouplingScheme::VolumeFiltered`].

```rust
pub struct CouplingExchange {
    pub drag_force: crate::particle::Vec3,
    pub particle_volume_fraction: uom::si::f64::Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `drag_force` | `crate::particle::Vec3` | Fluid → particle drag force `[N]` applied to the DEM particle this step. |
| `particle_volume_fraction` | `uom::si::f64::Ratio` | Particle (solid) volume fraction `ε_s` (dimensionless, `[0, 1)`) this<br>particle projects back onto the CFD mesh. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CouplingExchange { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &CouplingExchange) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ReservedFluidSource`

A reserved stand-in for the future CFD provider, so this crate's tests and
docs can name the CFD side of the seam **without** depending on
`outram-foam-multiphase`.

It implements [`FluidCouplingSource`] with a body that returns
[`DemError::NotImplemented`] — it holds no field data and computes nothing.
The real provider lives in the CFD crate; this exists only to make the seam
constructible and testable here.

```rust
pub struct ReservedFluidSource;
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
    fn clone(self: &Self) -> ReservedFluidSource { /* ... */ }
    ```

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
    fn default() -> ReservedFluidSource { /* ... */ }
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

- **FluidCouplingSource**
  - ```rust
    fn sample_fluid_state(self: &Self, _position: Vec3) -> Result<LocalFluidState, DemError> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
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
    fn eq(self: &Self, other: &ReservedFluidSource) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ReservedDragModel`

A reserved stand-in for the future DEM drag/volume model, implementing
[`DemCouplingResponse`] with [`DemError::NotImplemented`] bodies.

No drag correlation and no volume-averaging kernel are implemented — this is
the reserved DEM half of the seam, present so the interface is constructible
and testable in this phase. Real physics is deferred (Phase 5).

```rust
pub struct ReservedDragModel;
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
    fn clone(self: &Self) -> ReservedDragModel { /* ... */ }
    ```

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
    fn default() -> ReservedDragModel { /* ... */ }
    ```

- **DemCouplingResponse**
  - ```rust
    fn drag_force(self: &Self, _particle: &Particle, _fluid: &LocalFluidState) -> Result<Vec3, DemError> { /* ... */ }
    ```

  - ```rust
    fn particle_volume_fraction(self: &Self, _particle: &Particle, _averaging_volume: Volume) -> Result<Ratio, DemError> { /* ... */ }
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
    fn eq(self: &Self, other: &ReservedDragModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ReservedCoupling`

The reserved **coupling driver**: it names a [`CouplingScheme`] and sketches
how the two sides of the seam would compose into one particle's exchange,
**without** implementing the exchange.

This type exists to fix the *wiring shape* — how a [`FluidCouplingSource`]
and a [`DemCouplingResponse`] combine per particle — using generics (no
`dyn`, no `Box`) so both sides stay statically dispatched and this crate
stays independent of the CFD crate.

# Status

Reserved only. [`couple_particle`](ReservedCoupling::couple_particle)
returns [`DemError::NotImplemented`]; no coupling loop runs.

```rust
pub struct ReservedCoupling {
    pub scheme: CouplingScheme,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `scheme` | `CouplingScheme` | The coupling regime this driver would apply (see [`CouplingScheme`]). |

##### Implementations

###### Methods

- ```rust
  pub const fn new(scheme: CouplingScheme) -> Self { /* ... */ }
  ```
  Construct a reserved driver for the given [`CouplingScheme`].

- ```rust
  pub fn couple_particle<F, D>(self: &Self, particle: &Particle, fluid_source: &F, drag_model: &D, averaging_volume: Volume) -> Result<CouplingExchange, DemError>
where
    F: FluidCouplingSource,
    D: DemCouplingResponse { /* ... */ }
  ```
  Sketch of **one particle's coupling step**: sample the fluid at the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ReservedCoupling { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ReservedCoupling) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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

#### Trait `FluidCouplingSource`

The **CFD side** of the seam: a source of interpolated fluid state at a
point. The future `outram-foam-multiphase` crate would implement this trait;
this crate only *declares* it, so the two pillars stay decoupled at compile
time (Phase II separation principle).

It is a compiler-enforced contract, **not** a dynamic-dispatch boundary —
consumers take `impl FluidCouplingSource` / a generic `F:
FluidCouplingSource` (see [`ReservedCoupling::couple_particle`]), never
`dyn`/`Box`, per the workspace design rules.

# Status

Reserved only. No implementor in this crate does real interpolation; the
bundled [`ReservedFluidSource`] returns [`DemError::NotImplemented`].

```rust
pub trait FluidCouplingSource {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `sample_fluid_state`: Sample the CFD fluid field at `position` `[m]` (a particle centre),

##### Implementations

This trait is implemented for the following types:

- `ReservedFluidSource`

#### Trait `DemCouplingResponse`

The **DEM side** of the seam: given a particle and the fluid state at it,
produce the momentum/volume feedback for the CFD solve. This crate would
implement this trait once a drag closure and a volume-averaging kernel
exist (Phase 5 physics, deferred).

Like [`FluidCouplingSource`] it is a compiler-enforced contract used through
generics, never `dyn`/`Box`.

# Status

Reserved only. The bundled [`ReservedDragModel`] returns
[`DemError::NotImplemented`] from every method — there is no drag physics in
this phase.

```rust
pub trait DemCouplingResponse {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `drag_force`: Compute the **fluid → particle drag force** `[N]` on `particle` given the
- `particle_volume_fraction`: Compute the **particle (solid) volume fraction** `ε_s` (dimensionless,

##### Implementations

This trait is implemented for the following types:

- `ReservedDragModel`

## Module `gnn_bridge`

**Attributes:**

- `Other("#[attr = CfgTrace([NameValue { name: \"feature\", value: Some(\"gnn\"), span: crates/outram-park-fork-liggghts/src/lib.rs:106:7: 106:22 (#0) }])]")`

Bridge to RAFFLES's graph-neural-network layer: the contact graph, and how
much message-passing reach a DEM surrogate needs.

Gated on this crate's `gnn` feature, which is **on by default** (maintainer
direction, 2026-09-17). Enabling it costs a `raffles` dependency but **no
tensor library**: the contact graph and the reach bound below are plain
combinatorics, and `raffles`'s own `burn` feature stays off unless a caller
asks for it. Build without it via `--no-default-features`.

# Why a DEM code wants this

A granular assembly is already a graph: particles are nodes and contacts
are edges. That makes it a natural target for a message-passing surrogate —
and it makes the *reach* question immediate and physical, because force is
transmitted along **force chains** that span many particles.

The question a surrogate builder has to answer before training anything is
how many message-passing steps the physics needs. Guess low and the network
under-reaches: a particle's predicted force cannot depend on a particle
further away than the step count, so a force chain longer than that cannot
be represented at all, however long the model trains.
[`reach_bound`] answers it from the material and the timestep, with no
training and no neural network involved.

# A DEM step is hyperbolic

Contact forces propagate at the elastic wave speed of the solid, which is
finite, so the bound is the CFL-style one: the message reach `M * h` must
cover the distance `c * dt` a stress wave travels in one step. For a
packing of particles of diameter `d`, one message hop covers roughly one
particle diameter, so

```text
M >= c * dt / d,   c = sqrt(E / rho)
```

That is usually a small number for a well-chosen DEM timestep — which is
itself limited by the Rayleigh criterion for the same physical reason — and
that is the useful result: **a DEM surrogate does not need a deep network,
and a paper reporting one should say why.**

# What this module is not

It builds graphs and computes bounds. It does not train, own or evaluate a
surrogate: that belongs to the caller, using
[`raffles::gnn::MessagePassingNet`] and `raffles::gnn::training`. Nothing
here changes how the DEM solver runs.

```rust
pub mod gnn_bridge { /* ... */ }
```

### Functions

#### Function `contact_graph`

Builds the contact graph of a particle assembly.

Two particles are connected when the gap between their surfaces is at most
`skin`. With `skin = 0` that is exactly the set of particles currently in
contact; a positive skin is the usual neighbour-list margin, and is what you
want if the graph will be reused for more than one timestep.

`skin` is in metres, like every length in this crate.

# Errors

[`DemError::InvalidInput`] if `particles` is empty or `skin` is negative.
The underlying graph builder also rejects a non-finite coordinate, which
cannot happen for particles built through [`Particle::new`] but can after a
diverged integration — and catching it here is better than training on it.

```rust
pub fn contact_graph(particles: &[crate::particle::Particle], skin: f64) -> Result<raffles::gnn::Graph, crate::DemError> { /* ... */ }
```

#### Function `reach_bound`

The message-passing reach a surrogate of this assembly needs, for a given
material and timestep.

- `particles` — the assembly, used for its contact graph and its mean
  particle diameter (the distance one message hop covers).
- `youngs_modulus` — `E` in pascals, as [`crate::contact::HertzContact`]
  stores it.
- `density` — solid density in kg/m³. Note this is the **solid** density,
  not the bulk density of the packing: the stress wave travels through the
  material, not through the voids.
- `time_step` — the DEM timestep in seconds.
- `message_passing_steps` — what a candidate model is configured with, if
  there is one; pass `None` to ask only what is required.

# Errors

[`DemError::InvalidInput`] if the assembly is empty, if `youngs_modulus`,
`density` or `time_step` is not strictly positive, or if the mean particle
diameter is not positive.

```rust
pub fn reach_bound(particles: &[crate::particle::Particle], youngs_modulus: f64, density: f64, time_step: f64, message_passing_steps: Option<usize>) -> Result<raffles::gnn::bound::IterationBound, crate::DemError> { /* ... */ }
```

## Module `gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([All([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-park-fork-liggghts/src/lib.rs:108:15: 108:36 (#0) }, crates/outram-park-fork-liggghts/src/lib.rs:108:14: 108:37 (#0)), Not(NameValue { name: \"target_arch\", value: Some(\"wasm32\"), span: crates/outram-park-fork-liggghts/src/lib.rs:108:43: 108:65 (#0) }, crates/outram-park-fork-liggghts/src/lib.rs:108:42: 108:66 (#0))], crates/outram-park-fork-liggghts/src/lib.rs:108:10: 108:67 (#0))])]")`

Optional headless GPU compute for this crate's one embarrassingly-parallel
kernel: the [`crate::rdf`] pair-separation histogram.

# Why only the RDF, and not the timestep

This is the deliberate scope limit, and it is worth stating where someone
will look for it. The DEM timestep carries a persistent per-contact
tangential shear history with contacts born and dying every step — a
stateful gather/scatter structure that is the worst shape for a shader —
while the Hertz arithmetic that *would* port well was measured at only
3.7 ms of a 17.8 ms step once parallelised on the CPU. At HTR-10 scale the
per-step working set is also far too small to amortise a host↔device round
trip unless the whole integrator became GPU-resident. See
[`crate::compute::ComputeType`].

The RDF is the opposite: 3.8e8 independent, stateless distance evaluations
over a fixed point set, computed once per bed rather than once per step.

# Contract

1. **Compiles always, runs on CPU when there is no GPU.** The whole module
   is target-gated out on Android and `wasm32` (no system Vulkan/Metal
   loader; `wgpu-hal` is not `Sync` on wasm), and [`rdf_histogram`] returns
   `None` whenever no usable adapter exists — a headless server, CI with no
   loader. Callers **must** treat `None` as "run the CPU path", never as an
   error. [`crate::rdf::radial_distribution`] does exactly that.
2. **CPU is the trusted reference.** WGSL has no `f64`, so this kernel is
   `f32` throughout while the CPU path is `f64`. A pair whose separation
   falls within `f32` rounding of a bin edge can therefore land in a
   neighbouring bin, and the two histograms are **not** bit-identical. This
   is a real, measured difference, not a hypothetical — see
   `tests/rdf_backends.rs`. Anything feeding a V&V number uses the CPU path.
3. **No new third-party dependency.** `wgpu`'s `request_adapter` /
   `request_device` return futures; rather than pull in an async runtime
   this module hand-rolls a tiny pure-`std` [`block_on`], the same shape
   `outram-mc-libs` uses.

```rust
pub mod gpu { /* ... */ }
```

### Types

#### Struct `GpuContext`

A live, headless GPU compute context — a [`wgpu::Device`] and
[`wgpu::Queue`] obtained with no window or surface.

```rust
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `device` | `wgpu::Device` | The logical GPU device. |
| `queue` | `wgpu::Queue` | The command queue. |
| `info` | `wgpu::AdapterInfo` | Which physical adapter was selected. Diagnostic only. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

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

- **Freeze**
- **From**
  - ```rust
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

#### Function `probe`

**Attributes:**

- `MustUse { reason: None }`

Probe for a usable headless compute GPU, or `None` when the caller must
fall back to the CPU path.

`None` is the *normal, expected* outcome on a headless host and is never an
error.

```rust
pub fn probe() -> Option<GpuContext> { /* ... */ }
```

#### Function `rdf_histogram`

**Attributes:**

- `MustUse { reason: None }`

GPU pair-separation histogram: for every centre in `centre_idx`, count the
pebbles of `centres` whose separation is below `r_max`, binned at width
`dr`.

Returns `None` — meaning *run the CPU path* — when no adapter is available,
or when `n_bins` exceeds [`MAX_BINS`] (the shader's workgroup histogram is a
compile-time size).

# Precision

`f32` throughout; see the module contract. Not bit-identical to the `f64`
CPU reference.

```rust
pub fn rdf_histogram(centres: &[crate::particle::Vec3], centre_idx: &[u32], r_max: f64, dr: f64, n_bins: usize) -> Option<Vec<u64>> { /* ... */ }
```

#### Function `block_on`

Minimal pure-`std` executor: block the current thread until `future`
resolves, driving it with a `Wake`-based thread-park waker.

The standard `pollster::block_on` shape. It exists so this crate needs **no**
async-runtime dependency to await `wgpu`'s two setup futures. Buffer
read-back does not use it — that uses `Device::poll`.

```rust
pub fn block_on<F: Future>(future: F) -> <F as >::Output { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_BINS`

Largest bin count the shader's workgroup-local histogram can hold; must
match `MAX_BINS` in `shaders/rdf_histogram.wgsl`.

A request for more bins than this falls back to the CPU rather than
silently truncating the histogram.

```rust
pub const MAX_BINS: usize = 1024;
```

## Module `granular`

# LIGGGHTS-faithful granular contact pipeline (`pair_style gran`)

A line-by-line translation of the LIGGGHTS contact chain — *surface model →
normal model → tangential model* — including the **tangential shear
history** that the stateless [`crate::contact`] module deliberately omits.

## Why this module exists next to [`crate::contact`]

[`crate::contact`] evaluates a contact from a *snapshot* of two particles:
it has nowhere to keep the accumulated tangential displacement `ξ_t`, so it
hard-codes `ξ_t = 0` and the tangential force degenerates to a Coulomb-capped
dashpot. That is fine for an instantaneous force query and useless for a
packed bed: with no tangential *spring*, a static assembly cannot carry
shear, so a heap has **zero angle of repose** and a pebble bed will not stand
up. Reproducing LIGGGHTS requires history, and history requires state that
outlives the call — hence [`ShearHistory`].

## Sign and geometry conventions (upstream's, kept verbatim)

LIGGGHTS defines the contact normal `ê_n` as pointing **from `j` to `i`**
(`delta = x_i − x_j`, `ê_n = delta/|delta|`) and the relative velocity as
`v_r = v_i − v_j`. Consequently

- `v_n = v_r · ê_n` is **negative while the pair approaches**;
- the normal force `F_n ê_n` pushes `i` away from `j` for `F_n > 0`;
- the damping term is `−γ_n v_n`, positive (repulsive) on approach.

This is the *opposite* sign convention to [`crate::contact`], which measures
`v_n` positive on approach along an `a → b` normal. Both are self-consistent;
this module keeps upstream's so that the translation can be checked against
upstream source without a mental sign flip on every line.

## Contact radii — an `O(δ)` term [`crate::contact`] drops

Upstream evaluates the lever arm and the surface-velocity moment at the
**contact plane**, not the particle centre distance:

```text
  c_ri = r_i − δ_n/2 ,    c_rj = r_j − δ_n/2
```

[`crate::contact`] uses `r_i` and `r_j` instead. The difference is `O(δ_n)`
and therefore small, but it is a genuine divergence from upstream and it
biases both the slip velocity and the spin-up torque. This module uses
upstream's.

## Unit system: SI only, and why the port drops two conversion factors

Upstream divides its stiffnesses by `force->nktv2p` and scales its force-to-
velocity step by `force->ftm2v`:

```text
  kn /= force->nktv2p;   kt /= force->nktv2p;      // normal models
  dtf = 0.5 * dt * force->ftm2v;                   // fix_nve_sphere
```

Both constants are **exactly `1.0` for `units si`** (`update.cpp`, the `si`
branch), which is the unit system every case in this crate uses and the only
one its `uom`-typed API admits. The port therefore omits both
multiplications rather than carrying a factor that is identically one.

This is a deliberate simplification, recorded here because it is the one
place the translation is not literal: **it is exact for SI and wrong for any
other LIGGGHTS unit style** (`lj`, `real`, `metal`, `cgs`, … have
`nktv2p` = 1.0, 68568.415, 1.6021765e6, 2.94210108e13 respectively). If this
crate ever grows a non-SI path, both factors must come back.

## Honest scope

- **Implemented:** default surface model; Hertz and Hooke normal models;
  history and no-history tangential models; per-pair shear-history storage
  with Coulomb rescaling of the stored displacement; the CDT
  (constant-directional-torque) rolling model.
- **Not implemented:** cohesion models, the EPSD rolling family,
  superquadrics, multi-contact surface corrections,
  the `limitForce`/`viscous`/`heating`/elastic-potential switches, and
  mixed-material property matrices (a single material is assumed, as in
  [`crate::contact`]).
- **Verified against upstream** — see `docs/verification-and-validation.md`
  and `tests/liggghts_cross_code.rs`.

```rust
pub mod granular { /* ... */ }
```

### Types

#### Struct `GranularMaterial`

Material and interaction properties shared by both normal models.

Assumes a **single isotropic linear-elastic material** for both partners
(see the module "Honest scope"). Upstream supports a per-type-pair matrix;
the same-material reduction of upstream's `createYeff`/`createGeff` is used
here and is reproduced exactly by [`GranularMaterial::y_eff`] /
[`GranularMaterial::g_eff`].

# Parameters and units

| Field | Symbol | Quantity | SI unit | Valid range |
|---|---|---|---|---|
| `youngs_modulus` | `E` | Young's modulus | `[Pa]` | `> 0` |
| `poisson_ratio` | `ν` | Poisson ratio | `[-]` | `0 ≤ ν < 0.5` |
| `restitution` | `e` | coefficient of restitution | `[-]` | `0.05 < e ≤ 1` |
| `friction` | `μ` | Coulomb friction coefficient | `[-]` | `≥ 0` |

The restitution lower bound `0.05` is upstream's own sanity check in
`MODEL_PARAMS::createCoeffRest` (`0.05 < coefficientRestitution <= 1
required`) and is enforced here for parity.

```rust
pub struct GranularMaterial {
    pub youngs_modulus: f64,
    pub poisson_ratio: f64,
    pub restitution: f64,
    pub friction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `youngs_modulus` | `f64` | Young's modulus `E` `[Pa]`. |
| `poisson_ratio` | `f64` | Poisson ratio `ν` `[-]`. |
| `restitution` | `f64` | Coefficient of restitution `e` `[-]`. |
| `friction` | `f64` | Coulomb friction coefficient `μ` `[-]`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(youngs_modulus: f64, poisson_ratio: f64, restitution: f64, friction: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Validate and build a material.

- ```rust
  pub fn y_eff(self: &Self) -> f64 { /* ... */ }
  ```
  Effective Young's modulus `Y_eff` `[Pa]`.

- ```rust
  pub fn g_eff(self: &Self) -> f64 { /* ... */ }
  ```
  Effective shear modulus `G_eff` `[Pa]`.

- ```rust
  pub fn beta_eff(self: &Self) -> f64 { /* ... */ }
  ```
  Damping ratio `β_eff` `[-]`, upstream `createBetaEff`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GranularMaterial { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &GranularMaterial) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ContactKinematics`

Contact kinematics — translation of upstream `SurfaceModel<SURFACE_DEFAULT>`.

Every quantity uses upstream's convention (module docs): `ê_n` points from
`j` to `i`, `v_r = v_i − v_j`.

# Fields and units

| Field | Symbol | Quantity | SI unit |
|---|---|---|---|
| `en` | `ê_n` | unit contact normal, `j → i` | `[-]` |
| `delta_n` | `δ_n` | overlap (positive in contact) | `[m]` |
| `vn` | `v_n` | normal relative velocity (`< 0` approaching) | `[m/s]` |
| `vtr` | `v_tr` | relative **surface** velocity in the tangent plane | `[m/s]` |
| `cri` / `crj` | `c_ri`, `c_rj` | contact radii `r − δ_n/2` | `[m]` |
| `r_eff` | `R*` | effective radius | `[m]` |
| `m_eff` | `m*` | effective mass | `[kg]` |
| `omega_i` / `omega_j` | `ω_i`, `ω_j` | angular velocities | `[rad/s]` |
| `is_wall` | — | particle–wall contact flag | `[-]` |

```rust
pub struct ContactKinematics {
    pub en: crate::particle::Vec3,
    pub delta_n: f64,
    pub vn: f64,
    pub vtr: crate::particle::Vec3,
    pub cri: f64,
    pub crj: f64,
    pub r_eff: f64,
    pub m_eff: f64,
    pub omega_i: crate::particle::Vec3,
    pub omega_j: crate::particle::Vec3,
    pub is_wall: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `en` | `crate::particle::Vec3` | Unit contact normal, pointing from `j` to `i` `[-]`. |
| `delta_n` | `f64` | Overlap `δ_n = r_i + r_j − |x_i − x_j|` `[m]`, strictly positive. |
| `vn` | `f64` | Normal relative velocity `v_r · ê_n` `[m/s]`; negative while approaching. |
| `vtr` | `crate::particle::Vec3` | Relative surface (slip) velocity in the tangent plane `[m/s]`. |
| `cri` | `f64` | Contact radius of `i`, `r_i − δ_n/2` `[m]`. |
| `crj` | `f64` | Contact radius of `j`, `r_j − δ_n/2` `[m]`. |
| `r_eff` | `f64` | Effective (reduced) radius `R*` `[m]`. |
| `m_eff` | `f64` | Effective (reduced) mass `m*` `[kg]`. |
| `omega_i` | `crate::particle::Vec3` | Angular velocity of `i` `[rad/s]` (needed by the rolling model). |
| `omega_j` | `crate::particle::Vec3` | Angular velocity of `j` `[rad/s]`; zero for a wall contact. |
| `is_wall` | `bool` | Whether this is a particle–wall contact (upstream's `is_wall`), which<br>changes `R*`, `m*` and the rolling model's rolling-velocity branch. |

##### Implementations

###### Methods

- ```rust
  pub fn pair(i: &Particle, j: &Particle) -> Option<Self> { /* ... */ }
  ```
  Resolve the kinematics of a particle–particle contact, or `None` if the

- ```rust
  pub fn wall(i: &Particle, wall_normal: Vec3, delta_n: f64, wall_velocity: Vec3) -> Option<Self> { /* ... */ }
  ```
  Resolve the kinematics of a particle–**wall** contact.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ContactKinematics { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ContactKinematics) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `NormalOutcome`

Scalar coefficients produced by a normal model for one contact.

# Fields and units

| Field | Symbol | Quantity | SI unit |
|---|---|---|---|
| `fn_scalar` | `F_n` | normal force magnitude along `ê_n` | `[N]` |
| `kn` | `k_n` | normal stiffness | `[N/m]` |
| `kt` | `k_t` | tangential stiffness | `[N/m]` |
| `gamman` | `γ_n` | normal damping coefficient | `[kg/s]` |
| `gammat` | `γ_t` | tangential damping coefficient | `[kg/s]` |

```rust
pub struct NormalOutcome {
    pub fn_scalar: f64,
    pub kn: f64,
    pub kt: f64,
    pub gamman: f64,
    pub gammat: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fn_scalar` | `f64` | Normal force magnitude `[N]`, applied to `i` along `+ê_n`. |
| `kn` | `f64` | Normal stiffness `[N/m]`. |
| `kt` | `f64` | Tangential stiffness `[N/m]`. |
| `gamman` | `f64` | Normal damping coefficient `[kg/s]`. |
| `gammat` | `f64` | Tangential damping coefficient `[kg/s]`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NormalOutcome { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &NormalOutcome) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `GranularNormalModel`

Closed set of normal contact models (enum dispatch, no `dyn`).

Both variants carry the same [`GranularMaterial`]; they differ in how the
stiffness and damping are derived from it.

```rust
pub enum GranularNormalModel {
    Hertz {
        material: GranularMaterial,
        tangential_damping: bool,
    },
    Hooke {
        material: GranularMaterial,
        characteristic_velocity: f64,
        tangential_damping: bool,
        kt_to_kn: bool,
    },
}
```

##### Variants

###### `Hertz`

Upstream `NormalModel<HERTZ>` — nonlinear, `F_n ∝ δ_n^{3/2}`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `material` | `GranularMaterial` | Shared material properties. |
| `tangential_damping` | `bool` | Upstream `tangential_damping` on/off switch (default **on**). |

###### `Hooke`

Upstream `NormalModel<HOOKE>` — linearised about a characteristic
collision velocity, `F_n ∝ δ_n`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `material` | `GranularMaterial` | Shared material properties. |
| `characteristic_velocity` | `f64` | Characteristic impact velocity `v_char` `[m/s]` about which the<br>Hertzian stiffness is linearised (upstream `characteristicVelocity`). |
| `tangential_damping` | `bool` | Upstream `tangential_damping` on/off switch (default **on**). |
| `kt_to_kn` | `bool` | Upstream `ktToKn`: when true, `k_t = (2/7)·k_n` instead of `k_t = k_n`. |

##### Implementations

###### Methods

- ```rust
  pub fn hertz(material: GranularMaterial) -> Self { /* ... */ }
  ```
  Build a Hertz model with upstream's default settings

- ```rust
  pub fn hooke(material: GranularMaterial, characteristic_velocity: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Build a Hooke model with upstream's default settings

- ```rust
  pub fn material(self: &Self) -> GranularMaterial { /* ... */ }
  ```
  The material this model carries.

- ```rust
  pub fn evaluate(self: &Self, k: &ContactKinematics) -> NormalOutcome { /* ... */ }
  ```
  Evaluate the normal force and the stiffness/damping coefficients the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GranularNormalModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &GranularNormalModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `TangentialModel`

Closed set of tangential contact models.

```rust
pub enum TangentialModel {
    History,
    NoHistory,
}
```

##### Variants

###### `History`

Upstream `TangentialModel<TANGENTIAL_HISTORY>` — a Mindlin shear spring
with an accumulated tangential displacement, Coulomb-rescaled on slip.
**This is the model a packed bed needs.**

###### `NoHistory`

Upstream `TangentialModel<TANGENTIAL_NO_HISTORY>` — Coulomb-capped
dashpot only, no spring. Equivalent to what [`crate::contact`] does.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TangentialModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &TangentialModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `ContactKey`

Key identifying one persistent contact in the [`ShearHistory`] store.

Particle–particle pairs are keyed by their **ordered** pair `(min, max)` so
the entry is found regardless of which way round the pair is visited.
Particle–wall contacts are keyed by the particle and a caller-chosen wall id.

# The identifier is a STABLE TAG, not a position in the particle array

This matters, and getting it wrong is silent. Upstream stores a contact's
partner as `partner_[i][m] = tag[j]` — LAMMPS' global **atom tag** — in
`fix_contact_history.cpp:393`, not the local index. That is precisely what
lets LAMMPS delete an atom by copying the last atom into the hole without
corrupting anybody's shear history.

[`GranularSystem`](crate::granular_system::GranularSystem) follows upstream
and passes its own per-particle tags here. **Do not pass array indices from
a system whose particle set can change**: after a removal every index
shifts, and each stored tangential spring silently re-attaches to a
different pair — plausible-looking forces that are entirely wrong.

~~Particle–particle pairs are keyed by their ordered **index** pair~~
**CORRECTED 2026-09-17** — the store was index-keyed, which was safe only
while the particle set was fixed. It is tag-keyed now, matching upstream,
so insertion and removal are possible at all.

```rust
pub enum ContactKey {
    Pair {
        lo: usize,
        hi: usize,
    },
    Wall {
        particle: usize,
        wall: usize,
    },
}
```

##### Variants

###### `Pair`

Contact between two particles, stored with `lo < hi`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `lo` | `usize` | Lower particle tag. |
| `hi` | `usize` | Higher particle tag. |

###### `Wall`

Contact between a particle and a wall.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `particle` | `usize` | Particle tag. |
| `wall` | `usize` | Caller-assigned wall identifier. |

##### Implementations

###### Methods

- ```rust
  pub fn pair(i: usize, j: usize) -> Self { /* ... */ }
  ```
  Key for the particle pair with tags `(i, j)`, normalised so that

- ```rust
  pub fn wall(particle: usize, wall: usize) -> Self { /* ... */ }
  ```
  Key for a particle–wall contact.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ContactKey { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ContactKey) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ShearHistory`

Persistent per-contact tangential shear displacement store.

Upstream keeps this in its neighbour list's `contact_history` array and
clears an entry when the contact is lost. Here the same lifecycle is
explicit: [`ShearHistory::begin_step`] marks every entry stale,
[`ShearHistory::update`] refreshes the ones touched this step, and
[`ShearHistory::end_step`] drops whatever was not touched.

**Forgetting the `begin_step`/`end_step` bracket leaks history into contacts
that have already separated**, which shows up as spurious cohesion.
[`GranularSystem`] does the bracketing for you.

The stored quantity is the tangential displacement vector `ξ_t` `[m]`.

```rust
pub struct ShearHistory {
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
  An empty history store.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of contacts currently carrying history.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the store holds no contacts.

- ```rust
  pub fn get(self: &Self, key: ContactKey) -> Option<Vec3> { /* ... */ }
  ```
  The stored tangential displacement `ξ_t` `[m]` for a contact, if any.

- ```rust
  pub fn begin_step(self: &mut Self) { /* ... */ }
  ```
  Mark every stored contact stale, at the top of a force evaluation.

- ```rust
  pub fn end_step(self: &mut Self) { /* ... */ }
  ```
  Drop the history of every contact not refreshed since

- ```rust
  pub fn store(self: &mut Self, key: ContactKey, shear: Vec3) { /* ... */ }
  ```
  Record a contact's updated tangential displacement and mark it live —

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ShearHistory { /* ... */ }
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
    fn default() -> ShearHistory { /* ... */ }
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
    fn eq(self: &Self, other: &ShearHistory) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BuildContactHasher`

`BuildHasher` for [`ContactKey`], replacing the standard library's SipHash.

# Why this is hand-rolled rather than a dependency

`ContactKey` is two small integers and a discriminant. SipHash is a
keyed, DoS-resistant hash designed for adversarial string keys, and it is
roughly an order of magnitude more expensive than what integer keys need.
The contact store is looked up ~370 000 times and written ~66 500 times per
timestep, so the hash *is* the cost.

A crate such as `rustc-hash` would do this, but adding one would mean a new
entry in the root `[workspace.dependencies]` for twenty lines of
multiply-xor. This is those twenty lines: a standard FxHash-style
multiply-and-rotate accumulator, pure Rust, `no_std`-compatible arithmetic,
and safe on every target the workspace builds for.

**This is not a security boundary.** Contact keys come from the
simulation's own particle tags, never from untrusted input, so hash-flooding
resistance buys nothing here.

```rust
pub struct BuildContactHasher;
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

- **BuildHasher**
  - ```rust
    fn build_hasher(self: &Self) -> ContactHasher { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BuildContactHasher { /* ... */ }
    ```

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
    fn default() -> BuildContactHasher { /* ... */ }
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
#### Struct `ContactHasher`

The FxHash-style accumulator built by [`BuildContactHasher`].

```rust
pub struct ContactHasher(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ContactHasher { /* ... */ }
    ```

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
    fn default() -> ContactHasher { /* ... */ }
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

- **Hasher**
  - ```rust
    fn finish(self: &Self) -> u64 { /* ... */ }
    ```

  - ```rust
    fn write(self: &mut Self, bytes: &[u8]) { /* ... */ }
    ```

  - ```rust
    fn write_u8(self: &mut Self, n: u8) { /* ... */ }
    ```

  - ```rust
    fn write_u64(self: &mut Self, n: u64) { /* ... */ }
    ```

  - ```rust
    fn write_usize(self: &mut Self, n: usize) { /* ... */ }
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `RollingModel`

Closed set of rolling-resistance models.

Rolling resistance is what gives a granular heap a finite **angle of
repose**: without it, spheres roll off each other and a pile spreads until
it is flat. For a pebble bed it is the dominant knob on packing structure,
so it is part of this pipeline rather than an optional extra.

# Relationship to [`crate::rolling`]

[`crate::rolling::RollingModel`] has a similar constant-torque variant, but
it diverges from upstream in three ways, all reproduced correctly here:

1. it scales the torque by the **total** normal force `|F_n|` (including the
   viscous damping term), where upstream CDT uses the **elastic** part only,
   `k_n·δ_n`;
2. it does **not** remove the torsion (normal) component of the resisting
   torque, which upstream does by default (`torsionTorque` is off unless
   asked for);
3. for a wall contact it uses `ω_i − ω_j`, where upstream uses the
   contact-point rolling velocity `w_r`.

```rust
pub enum RollingModel {
    Off,
    Cdt {
        mu_r: f64,
        torsion_torque: bool,
    },
}
```

##### Variants

###### `Off`

No rolling resistance (upstream `rolling_model off`).

###### `Cdt`

Upstream `RollingModel<ROLLING_CDT>` — **constant directional torque**.

`M_r = µ_r · k_n·δ_n · R* · ŵ_r`, applied as `−M_r` to `i` and `+M_r` to
`j`, with the component along `ê_n` removed unless `torsion_torque` is
set. The magnitude is set by the elastic normal load and does not depend
on rolling *speed* — only its direction, hence "constant torque".

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mu_r` | `f64` | Rolling-friction coefficient `µ_r` `[-]`, non-negative. |
| `torsion_torque` | `bool` | Upstream `torsionTorque` switch. **Default `false`**, which *removes*<br>the torque component along the contact normal. |

##### Implementations

###### Methods

- ```rust
  pub fn cdt(mu_r: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Build a validated CDT model with upstream's default settings

- ```rust
  pub fn rolling_torque(self: &Self, k: &ContactKinematics, normal: &NormalOutcome, omega_i: Vec3, omega_j: Vec3, is_wall: bool) -> Vec3 { /* ... */ }
  ```
  Resisting rolling torque `M_r` `[N·m]` for one contact.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RollingModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RollingModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GranularForce`

Force and torque contributions of one resolved contact.

Sign convention is upstream's (module docs): `force_i` acts on `i`,
`force_j = −force_i`, and both torques are computed from the *same*
`ê_n × F_t` product scaled by each partner's contact radius.

# Fields and units

| Field | Quantity | SI unit |
|---|---|---|
| `force_i` / `force_j` | force on `i` / `j` | `[N]` |
| `torque_i` / `torque_j` | torque on `i` / `j` | `[N·m]` |
| `fn_scalar` | normal force magnitude | `[N]` |
| `ft` | tangential force applied to `i` | `[N]` |

```rust
pub struct GranularForce {
    pub force_i: crate::particle::Vec3,
    pub force_j: crate::particle::Vec3,
    pub torque_i: crate::particle::Vec3,
    pub torque_j: crate::particle::Vec3,
    pub fn_scalar: f64,
    pub ft: crate::particle::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `force_i` | `crate::particle::Vec3` | Total force on `i` `[N]`. |
| `force_j` | `crate::particle::Vec3` | Total force on `j` `[N]` (zero for a wall contact). |
| `torque_i` | `crate::particle::Vec3` | Torque on `i` `[N·m]`. |
| `torque_j` | `crate::particle::Vec3` | Torque on `j` `[N·m]` (zero for a wall contact). |
| `fn_scalar` | `f64` | Normal force magnitude along `ê_n` `[N]`. |
| `ft` | `crate::particle::Vec3` | Tangential force on `i` `[N]`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GranularForce { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &GranularForce) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GranularContactModel`

The assembled contact law: surface + normal + tangential model.

This is the type a solver holds. It is `Copy` and carries no per-contact
state; the state lives in the [`ShearHistory`] you pass in.

```rust
pub struct GranularContactModel {
    pub normal: GranularNormalModel,
    pub tangential: TangentialModel,
    pub rolling: RollingModel,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `normal` | `GranularNormalModel` | The normal model (Hertz or Hooke). |
| `tangential` | `TangentialModel` | The tangential model (history or no-history). |
| `rolling` | `RollingModel` | The rolling-resistance model. Defaults to [`RollingModel::Off`] via<br>[`GranularContactModel::new`]; set it with<br>[`GranularContactModel::with_rolling`]. |

##### Implementations

###### Methods

- ```rust
  pub fn has_tangential_history(self: &Self) -> bool { /* ... */ }
  ```
  Whether this model stores a persistent tangential spring per contact.

- ```rust
  pub fn new(normal: GranularNormalModel, tangential: TangentialModel) -> Self { /* ... */ }
  ```
  Assemble a contact model from its normal and tangential halves, with

- ```rust
  pub fn with_rolling(self: Self, rolling: RollingModel) -> Self { /* ... */ }
  ```
  Return a copy of this model with the given rolling-resistance model.

- ```rust
  pub fn hertz_history(material: GranularMaterial) -> Self { /* ... */ }
  ```
  Upstream's default pairing: `pair_style gran model hertz tangential history`.

- ```rust
  pub fn resolve(self: &Self, key: ContactKey, k: &ContactKinematics, history: &mut ShearHistory, dt: f64) -> GranularForce { /* ... */ }
  ```
  Resolve one contact, advancing its shear history by `dt` `[s]`.

- ```rust
  pub fn resolve_pure(self: &Self, prior_shear: Vec3, k: &ContactKinematics, dt: f64) -> (GranularForce, Option<Vec3>) { /* ... */ }
  ```
  The **pure** core of [`GranularContactModel::resolve`]: the same contact

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GranularContactModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &GranularContactModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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

#### Constant `SQRT_FIVE_OVER_SIX`

`√(5/6)`, upstream's `sqrtFiveOverSix` damping constant `[-]`.

Appears in the Hertz viscoelastic damping coefficients
`γ = −2√(5/6)·β·√(S·m*)`. Upstream hard-codes the literal to 53 digits;
the `f64` value is identical.

```rust
pub const SQRT_FIVE_OVER_SIX: f64 = 0.912_870_929_175_276_9;
```

## Module `granular_system`

# History-aware DEM driver (LIGGGHTS `run` loop)

Composes [`crate::granular`] (contact law + shear history),
[`crate::integrator`] (velocity-Verlet), and [`crate::boundary`] (wall
primitives) into a runnable simulation that reproduces LIGGGHTS' step
ordering:

```text
  for each step:
    initial_integrate   (half-kick with F(t), then drift)
    compute forces      (pair contacts, then wall contacts, then gravity)
    final_integrate     (half-kick with F(t+dt))
```

## Relationship to [`crate::simulation::DemSimulation`]

[`crate::simulation::DemSimulation`] is the original stateless engine: it
uses [`crate::contact`] (no shear history) and
[`crate::particle::Particle::integrate`] (not symplectic — see
[`crate::integrator`]). It is kept for the cases it was written and tested
for, and because its neighbour-search code is shared. **For anything that
must settle, pack, or hold a static assembly — a pebble bed — use
[`GranularSystem`].**

## Honest scope

Single material; primitive [`Boundary`] walls only (no triangulated meshes —
see [`crate::mesh_wall`]); no cohesion or rolling models wired in; uniform
gravity; fixed time step; serial. Neighbour candidates come from an
all-pairs scan below [`GranularSystem::BRUTE_FORCE_THRESHOLD`] and a
uniform linked-cell grid above it.

```rust
pub mod granular_system { /* ... */ }
```

### Types

#### Struct `GranularSystem`

A runnable DEM simulation with **persistent tangential shear history**.

# Fields and units

| Field | Quantity | SI unit |
|---|---|---|
| `particles` | sphere ensemble | mixed (see [`Particle`]) |
| `boundaries` | primitive walls | mixed (see [`Boundary`]) |
| `model` | contact law (normal + tangential) | — |
| `history` | per-contact tangential displacement `ξ_t` | `[m]` |
| `gravity` | uniform gravitational acceleration | `[m/s²]` |
| `dt` | fixed velocity-Verlet step | `[s]` |
| `time` | elapsed simulated time | `[s]` |

```rust
pub struct GranularSystem {
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
  pub fn new(particles: Vec<Particle>, boundaries: Vec<Boundary>, model: GranularContactModel, gravity: Vec3, dt: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Build a simulation.

- ```rust
  pub fn with_moving_walls(self: Self, walls: Vec<MovingBoundary>) -> Self { /* ... */ }
  ```
  Attach kinematically-prescribed **moving walls** (upstream

- ```rust
  pub fn with_compute(self: Self, compute: ComputeType) -> Self { /* ... */ }
  ```
  Select the compute backend for the timestep, returning the updated

- ```rust
  pub fn compute(self: &Self) -> ComputeType { /* ... */ }
  ```
  The compute backend the timestep is using.

- ```rust
  pub fn moving_walls(self: &Self) -> &[MovingBoundary] { /* ... */ }
  ```
  The attached moving walls, in their current pose.

- ```rust
  pub fn moving_walls_mut(self: &mut Self) -> &mut [MovingBoundary] { /* ... */ }
  ```
  Mutable access to the moving walls, for **changing their prescribed

- ```rust
  pub fn particles(self: &Self) -> &[Particle] { /* ... */ }
  ```
  The particle ensemble `[m]`/`[m/s]`/… (see [`Particle`]).

- ```rust
  pub fn tags(self: &Self) -> &[u64] { /* ... */ }
  ```
  The stable tag of each particle, parallel to [`GranularSystem::particles`].

- ```rust
  pub fn insert_particle(self: &mut Self, p: Particle) -> u64 { /* ... */ }
  ```
  Insert a particle, returning its newly assigned stable tag.

- ```rust
  pub fn surface_height_at(self: &Self, x: f64, y: f64, radius: f64, fallback: f64) -> f64 { /* ... */ }
  ```
  The height at which a sphere of radius `r` dropped at `(x, y)` would

- ```rust
  pub fn insert_particles</* synthetic */ impl IntoIterator<Item = Particle>: IntoIterator<Item = Particle>>(self: &mut Self, particles: impl IntoIterator<Item = Particle>) -> Vec<u64> { /* ... */ }
  ```
  Insert several particles at once, returning their newly assigned stable

- ```rust
  pub fn remove_particles(self: &mut Self, indices: &[usize]) -> Vec<Particle> { /* ... */ }
  ```
  Remove the particles at the given **array indices**, returning them in

- ```rust
  pub fn time(self: &Self) -> f64 { /* ... */ }
  ```
  Elapsed simulated time `[s]`.

- ```rust
  pub fn dt(self: &Self) -> f64 { /* ... */ }
  ```
  The fixed time step `[s]`.

- ```rust
  pub fn live_contacts(self: &Self) -> usize { /* ... */ }
  ```
  Number of contacts currently carrying tangential history.

- ```rust
  pub fn contact_pairs(self: &Self) -> Vec<(usize, usize)> { /* ... */ }
  ```
  Every pair of particles **actually in contact** right now, as sorted

- ```rust
  pub fn coordination_number(self: &Self) -> f64 { /* ... */ }
  ```
  Mean coordination number `[-]`: contacts per particle, counting both

- ```rust
  pub fn kinetic_energy(self: &Self) -> f64 { /* ... */ }
  ```
  Total translational kinetic energy `Σ ½ m v²` `[J]`.

- ```rust
  pub fn rotational_energy(self: &Self) -> f64 { /* ... */ }
  ```
  Total rotational kinetic energy `Σ ½ I ω²` `[J]`, `I = (2/5) m r²`.

- ```rust
  pub fn step(self: &mut Self) { /* ... */ }
  ```
  Advance one velocity-Verlet step, in LIGGGHTS' order.

- ```rust
  pub fn run(self: &mut Self, n_steps: usize) { /* ... */ }
  ```
  Advance `n_steps` steps.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GranularSystem { /* ... */ }
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
    fn eq(self: &Self, other: &GranularSystem) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `htr10_fill`

# A fresh HTR-10 pebble pour, steppable from a UI

Pours `N` pebbles into the HTR-10 vessel from empty and settles them under
gravity, in chunks the caller drives, reporting progress after each. Built
for the Dhoby Ghaut workbench's Step 1 (gh:#561: "I should be able to run a
fresh DEM in the early stages"), so a user can choose how many pebbles to
load and watch the bed form, and the Monte Carlo model is then built on the
bed this produced.

**Before this module** every HTR-10 bed in the workspace started from
LIGGGHTS' own `fix insert/pack` output (`reference-data/liggghts/htr10_init.csv`);
the Rust side only re-settled it (`examples/htr10_recirculation_sweep.rs`).
The pieces here are lifted from those examples so the two cannot drift:

| piece | from |
|---|---|
| vessel: barrel, conus + discharge tube mesh, valve | `htr10_recirculation_sweep.rs` (system set-up), `reference-data/liggghts/make_htr10_discharge_stl.sh` (the mesh, generated here in code with the same 120-segment winding) |
| contact model and defaults | the same example; V&V `docs/verification-and-validation.md` § 4.7 (E, dt) and § 4.9 (µ, µ_r; gh:#216) |
| loose jittered-lattice seeding | `examples/bake_htr10_conus_slab.rs` `seed_column` |
| settle criterion (core KE per pebble / one-radius drop < 1e-3) | `htr10_recirculation_sweep.rs` adaptive pre-settle |
| surface height (99th percentile + r), whole-core φ | the same example's `bed_surface_height`, `whole_core_fraction` |

## The defaults, and what they rest on

- **µ = 0.1, µ_r = 0**: the setting at which this port's conus pre-settle
  gives a whole-core filling fraction of **0.6047 against the published
  0.61 (−0.9 %)** (V&V § 4.9, gh:#216). The docs justify µ as "graphite is
  a solid lubricant; graphite-on-graphite sliding friction 0.1–0.2", with
  **no specific paper cited**; treat it as a stated assumption, and ablate
  it rather than tune it. The older LIGGGHTS-deck default is µ = 0.4,
  µ_r = 0.1 (`reference-data/liggghts/in.htr10`).
- **E = 5e8 Pa** (graphite is ~9 GPa): the standard pebble-bed DEM
  softening, set by measuring contact overlap, not by its effect on packing
  (V&V § 4.7). Soft-sphere overlap at this stiffness is up to ~1.7 % of r.
- **dt = 3.5e-5 s**: 11.7 % of the Rayleigh time at E = 5e8.
- ν = 0.2, e = 0.5, ρ = 1730 kg/m³, r = 3 cm.

These are the settings the maintainer's HTR-10 pebble-bed DEM figure package
records (publications repository, `outram_park/outram_park_intro_paper/
outram_park_double_heterogeneity_arxiv/src/results_and_discussion/
pebble_bed_dem/README.md`, prepared 2026-09-18 from this crate's V&V): the
same µ = 0.1, µ_r = 0, E = 5e8 Pa, ν = 0.2, e = 0.5, dt = 35 µs, and the
2×2 friction ablation reaching 0.6047 against the quoted 0.61, which it
states is "an ablation, not a calibration".

The published 0.61 is a check, not an input: nothing here is tuned to it.
It is itself quoted from a specification table, not measured.

## Frame

Metres. `z = 0` is the conus inlet (the floor of the cylindrical core); the
conus runs down to `z = −0.36946`, then a 0.25 m length of the 0.25 m-radius
discharge tube to the valve at `z = −0.61946`. The real tube is ~6 m; the
DEM holds only this stub of it.

```rust
pub mod htr10_fill { /* ... */ }
```

### Types

#### Struct `Htr10FillSettings`

Everything a fill depends on.

```rust
pub struct Htr10FillSettings {
    pub n_pebbles: usize,
    pub friction: f64,
    pub rolling_friction: f64,
    pub youngs_modulus: uom::si::f64::Pressure,
    pub poisson_ratio: f64,
    pub restitution: f64,
    pub dt: uom::si::f64::Time,
    pub threads: crate::compute::ThreadCount,
    pub seed: u64,
    pub settle_target: f64,
    pub max_steps: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_pebbles` | `usize` | Pebbles to pour. |
| `friction` | `f64` | Sliding friction µ \[-\]. |
| `rolling_friction` | `f64` | Rolling friction µ_r \[-\] (0 = none). |
| `youngs_modulus` | `uom::si::f64::Pressure` | Young's modulus. |
| `poisson_ratio` | `f64` | Poisson's ratio \[-\]. |
| `restitution` | `f64` | Coefficient of restitution \[-\]. |
| `dt` | `uom::si::f64::Time` | Integration timestep. |
| `threads` | `crate::compute::ThreadCount` | Threads. |
| `seed` | `u64` | Seed of the jitter in the initial loose lattice. |
| `settle_target` | `f64` | Settled when the mean kinetic energy per core pebble falls below this<br>fraction of a one-radius gravitational drop. |
| `max_steps` | `usize` | Give up settling after this many steps (reported, never hidden). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10FillSettings { /* ... */ }
    ```

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
    The V&V § 4.9 setting (µ = 0.1, µ_r = 0) for the full core: 27 000

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
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
    fn eq(self: &Self, other: &Htr10FillSettings) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `FillProgress`

Where a fill is, after a chunk.

```rust
pub struct FillProgress {
    pub steps: usize,
    pub time: uom::si::f64::Time,
    pub ke_ratio_core: f64,
    pub phi_whole_core: f64,
    pub surface_height: uom::si::f64::Length,
    pub n_in_core: usize,
    pub settled: bool,
    pub gave_up: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `steps` | `usize` | Steps taken so far. |
| `time` | `uom::si::f64::Time` | Simulated time. |
| `ke_ratio_core` | `f64` | Mean KE per pebble in the core (`z > 0`) over a one-radius drop. |
| `phi_whole_core` | `f64` | Whole-core filling fraction `N V / (π R² h)` (`z > 0`, `h` = surface). |
| `surface_height` | `uom::si::f64::Length` | Bed surface: 99th-percentile core centre height + r. |
| `n_in_core` | `usize` | Pebbles above the conus inlet. |
| `settled` | `bool` | Below the settle target. |
| `gave_up` | `bool` | Hit `max_steps` without settling. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FillProgress { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &FillProgress) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Htr10Fill`

A pour in progress.

```rust
pub struct Htr10Fill {
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
  pub fn new(settings: Htr10FillSettings) -> Result<Self, DemError> { /* ... */ }
  ```
  Seed the pour and build the system. Nothing is stepped yet.

- ```rust
  pub fn settings(self: &Self) -> Htr10FillSettings { /* ... */ }
  ```
  The settings it was built with.

- ```rust
  pub fn advance(self: &mut Self, n: usize) -> FillProgress { /* ... */ }
  ```
  Step `n` more times (fewer if `max_steps` is reached) and report.

- ```rust
  pub fn progress(self: &Self) -> FillProgress { /* ... */ }
  ```
  Where the fill is now, without stepping.

- ```rust
  pub fn centres(self: &Self) -> Vec<Vec3> { /* ... */ }
  ```
  Pebble centres \[m\], in insertion order.

- ```rust
  pub fn system(self: &Self) -> &GranularSystem { /* ... */ }
  ```
  The underlying system (for `DemBed::from_granular_system`, overlap

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10Fill { /* ... */ }
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

#### Function `discharge_mesh`

The conus and discharge tube as an inward-facing triangle mesh, exactly as
`make_htr10_discharge_stl.sh` writes it (two bands of `segments` quads).

```rust
pub fn discharge_mesh(segments: usize) -> Result<crate::mesh_wall::MeshWall, crate::DemError> { /* ... */ }
```

#### Function `wall_radius_at`

Inner wall radius at height `z` \[m\]: tube, conus, then barrel.

```rust
pub fn wall_radius_at(z: f64) -> f64 { /* ... */ }
```

#### Function `seed_loose`

`n` pebble centres placed at random without overlap in the vessel, filling
it from the valve upward at a loose solid fraction (0.30), the way
LIGGGHTS' `fix insert/pack` seeds a region: random positions, rejected if
they overlap a placed pebble or the wall. Deterministic in `seed`
(SplitMix64, no RNG dependency). The pour then settles under gravity.

~~A jittered square lattice (pitch 1.1 d), from `bake_htr10_conus_slab.rs`
`seed_column`.~~ **REPLACED 2026-10-05:** a 16 890-pebble pour from that
lattice kept its order through the short drop: g(√3 d) 2.90 and g(√2 d)
0.898 against 1.29 and 0.599 for the LIGGGHTS-port reference bed
(`htr10_conus_presettled_mu10_mur00.csv`), and a whole-core φ of 0.6227.
A seed with no lattice in it carries no order to freeze in. Measured
with this seed, same 16 890 pebbles: g(√2 d) 0.619, g(√3 d) 1.284, g(2 d)
1.202, contact peak 19.55, against 0.599 / 1.286 / 1.190 / 19.56 for the
reference random bed (core region, `examples/bed_rdf_check.rs`); settled
after 26 000 steps (307 s, 12 threads) at whole-core φ 0.5954. That φ is
not compared with the package's 0.6047, which is a 27 554-pebble core.

```rust
pub fn seed_loose(n: usize, seed: u64) -> Vec<crate::particle::Vec3> { /* ... */ }
```

#### Function `surface_height_m`

Bed surface \[m\]: 99th-percentile centre height over the core (`z > 0`)
plus one radius. Robust to a single pebble on top (see the sweep example's
note on why `max z` is the wrong instrument). `None` if no pebble is in the
core.

```rust
pub fn surface_height_m(centres: &[crate::particle::Vec3]) -> Option<f64> { /* ... */ }
```

#### Function `whole_core_fraction`

Whole-core filling fraction `N V / (π R² h)` over `z > 0`: the like-for-like
comparison with the published 0.61, itself a whole-core figure.

```rust
pub fn whole_core_fraction(centres: &[crate::particle::Vec3]) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `PEBBLE_RADIUS_M`

Pebble radius \[m\].

```rust
pub const PEBBLE_RADIUS_M: f64 = 0.03;
```

#### Constant `PEBBLE_DENSITY`

Graphite pebble density \[kg/m³\].

```rust
pub const PEBBLE_DENSITY: f64 = 1730.0;
```

#### Constant `CORE_RADIUS_M`

Core (barrel) radius \[m\].

```rust
pub const CORE_RADIUS_M: f64 = 0.90;
```

#### Constant `CONE_HEIGHT_M`

Height of the conus \[m\].

```rust
pub const CONE_HEIGHT_M: f64 = 0.36946;
```

#### Constant `TUBE_RADIUS_M`

Discharge-tube radius \[m\].

```rust
pub const TUBE_RADIUS_M: f64 = 0.25;
```

#### Constant `TUBE_LENGTH_M`

Length of discharge tube the DEM holds \[m\].

```rust
pub const TUBE_LENGTH_M: f64 = 0.25;
```

#### Constant `VALVE_Z_M`

The valve at the bottom of the DEM's tube \[m\].

```rust
pub const VALVE_Z_M: f64 = _;
```

## Module `integrator`

# Velocity-Verlet integration for spheres (`nve/sphere`)

Faithful translation of LIGGGHTS' `FixNVESphere`, which is a genuine
**kick–drift–kick** velocity-Verlet propagator split across two half steps
around the force evaluation:

```text
  initial_integrate:   v += (dt/2)·F(t)/m
                       ω += (dt/2)·τ(t)/I
                       x += dt·v
  ---- forces and torques are recomputed at x(t+dt) ----
  final_integrate:     v += (dt/2)·F(t+dt)/m
                       ω += (dt/2)·τ(t+dt)/I
```

with `I = (2/5) m r²` for a solid sphere (LIGGGHTS' `INERTIA = 0.4`, applied
as `dtirotate = (dt/2)/INERTIA / (r² m)`).

## Why this module exists: [`Particle::integrate`] is not velocity-Verlet

[`Particle::integrate`](crate::particle::Particle::integrate) applies the
*same* acceleration `a(t)` to both the position and the velocity update:

```text
  x(t+dt) = x + v·dt + ½·a(t)·dt²
  v(t+dt) = v + a(t)·dt
```

That is **not** velocity-Verlet, and — despite what that method's own doc
comment claimed before 2026-09-15 — it is **not symplectic**. For a linear
restoring force `a = −ω²x` (which is exactly what a DEM contact spring is)
its one-step Jacobian is

```text
  M = [ 1 − ω²dt²/2    dt ]        det M = 1 + ω²dt²/2  >  1
      [ −ω²dt           1 ]
```

so phase-space volume — and with it the energy — **grows geometrically**,
by a factor `(1 + ω²dt²/2)` per step, no matter how well resolved the step
is. Measured on a unit oscillator at `dt = 0.1/ω` (≈ 63 steps per period, a
*comfortably* resolved DEM contact), `E/E₀ = 2.13 × 10⁴³` after 20 000
steps, against `0.99969` for the scheme in this module. See
`docs/verification-and-validation.md` § "Integrator".

The old scheme is exact for a **constant** force (free flight under gravity,
constant-torque spin-up), which is all its original unit tests exercised —
which is why the defect survived. It is kept, with a corrected doc comment,
because it is still the right thing for a single constant-force kick; every
*contact* integration should use [`VelocityVerlet`].

## Unit system: SI only

Upstream forms its half-step as `dtf = 0.5 * dt * force->ftm2v`. `ftm2v` is
**exactly `1.0` for `units si`** (`update.cpp`), which is the only unit
system this crate's `uom`-typed API admits, so the factor is omitted rather
than carried as an identity. Exact for SI; it would be wrong for LIGGGHTS'
`lj`/`real`/`metal`/`cgs` styles. See the same note in
[`crate::granular`].

## Honest scope

Translation of the integrator only. Orientation (quaternions) is not
tracked, matching the base crate: only `angular_velocity` is advanced, which
is sufficient for spheres with isotropic inertia.

```rust
pub mod integrator { /* ... */ }
```

### Types

#### Struct `VelocityVerlet`

Kick–drift–kick **velocity-Verlet** propagator for a sphere ensemble
(`fix nve/sphere`).

Holds only the time step, so it is `Copy` and carries no ensemble state; the
particles live in the caller's `Vec<Particle>` and are advanced in place.

# Usage

One full step is *always* three calls, in this order:

```
# use outram_park_fork_liggghts::integrator::VelocityVerlet;
# use outram_park_fork_liggghts::particle::{Particle, Vec3};
# let mut particles = vec![Particle::new(
#     Vec3::zero(), Vec3::new(1.0, 0.0, 0.0), Vec3::zero(), 1.0, 0.1, 300.0).unwrap()];
# let compute_forces = |_: &[Particle]| (vec![Vec3::zero()], vec![Vec3::zero()]);
let vv = VelocityVerlet::new(1.0e-6).unwrap();

let (mut f, mut t) = compute_forces(&particles);   // F(t), τ(t)
vv.initial_integrate(&mut particles, &f, &t);      // half-kick + drift
let (f_new, t_new) = compute_forces(&particles);   // F(t+dt), τ(t+dt)
vv.final_integrate(&mut particles, &f_new, &t_new); // half-kick
# f = f_new; t = t_new; let _ = (f, t);
```

Skipping [`VelocityVerlet::final_integrate`], or reusing the *old* forces in
it, degrades the scheme back to the non-symplectic form described in the
module docs — the energy will grow.

# Parameters and units

| Field | Symbol | Quantity | SI unit | Valid range |
|---|---|---|---|---|
| `dt` | `Δt` | integration time step | `[s]` | `> 0` |

```rust
pub struct VelocityVerlet {
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
  pub fn new(dt: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Build a propagator with time step `dt` `[s]`.

- ```rust
  pub fn dt(self: &Self) -> f64 { /* ... */ }
  ```
  The integration time step `[s]`.

- ```rust
  pub fn initial_integrate(self: &Self, particles: &mut [Particle], forces: &[Vec3], torques: &[Vec3]) { /* ... */ }
  ```
  First half of the step: velocity/spin half-kick with the **current**

- ```rust
  pub fn final_integrate(self: &Self, particles: &mut [Particle], forces: &[Vec3], torques: &[Vec3]) { /* ... */ }
  ```
  Second half of the step: velocity/spin half-kick with the force and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> VelocityVerlet { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &VelocityVerlet) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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

#### Constant `INERTIA`

LIGGGHTS' `INERTIA` constant for a solid sphere: `I = INERTIA · m · r²`.

Dimensionless `[-]`. Value `2/5`, the moment of inertia of a uniform solid
sphere about a diameter.

```rust
pub const INERTIA: f64 = 0.4;
```

## Module `mesh_wall`

Phase 3 (extension) — **Triangulated (mesh) & moving walls** (bead
`op-t3l.3` follow-up).

The analytic boundaries in [`crate::boundary`] cover infinite planes,
half-space walls, axis-aligned boxes, and infinite cylinders. This module
adds the two extensions those primitives cannot express:

- a **triangulated (STL-style) surface** — [`MeshWall`], a `Vec` of flat
  [`Triangle`] faces — so arbitrary complex wall geometry (hoppers, chutes,
  impellers, imported CAD/STL meshes) can collide with particles;
- **moving / rotating walls** — [`MovingBoundary`], a rigid-body wrapper that
  carries a translational velocity and an angular velocity about a pivot,
  advances the wrapped geometry in time, and reports the **local surface
  velocity** at a contact point.

Like [`crate::boundary`], this module is **geometry + kinematics only**. It
answers "does the sphere penetrate, by how much (`δ`), along which normal,
and how fast is the wall surface moving there?" — it does **not** compute
contact forces. The [`Contact`] it returns and the surface velocity it
reports are exactly the hand-off a contact-force + wall-friction law (Phase 2
[`crate::contact`]) consumes: the friction force needs the particle velocity
*relative to the moving wall surface*, `v_rel = v_particle − v_surface`, and
[`MovingBoundary::surface_velocity`] supplies the `v_surface` term. The
force computation itself stays in the contact model.

# Contact convention (shared with [`crate::boundary`])

A [`Contact`] (reused from [`crate::boundary::Contact`]) carries the
penetration depth `δ = r − d > 0` `[m]` (with `d` the distance from the
particle centre to the wall surface), a **unit** contact `normal` pointing
from the wall surface into the domain (toward the particle centre for a
particle on the outward side), and the `point` on the wall surface. A
repulsive penalty force `F = k · δ · normal` then pushes the particle off the
wall.

# Winding / outward-normal convention

Each [`Triangle`] `{a, b, c}` has an outward unit normal
`n̂ = normalize((b − a) × (c − a))`. Order the vertices **counter-clockwise
as seen from the domain (particle) side**, exactly as the STL format
requires, so `n̂` points out of the solid, into the free region where
particles live. A mesh wall is treated as one-sided: contact is meaningful
for particles approaching from the outward (`+n̂`) side.

# Units

Distances `[m]`, translational velocity `[m/s]`, angular velocity `[rad/s]`
(all `f64` in SI base units, matching [`crate::particle`]). Vertices,
contact points, pivots are positions `[m]`; the contact normal is
dimensionless.

# Honest scope (this extension)

A **verified geometric + kinematic foundation only** — it has **not** been
validated against a DEM reference code (that is the later human validation
step in this bead's Definition of Done; the tests below check hand-computed
geometry and closed-form rigid-body kinematics). Concretely:

- **Flat triangles only** — no curved/higher-order surface patches (Bézier,
  NURBS, subdivision); a curved wall must be pre-tessellated into triangles
  by the caller.
- **No self-collision** and no mesh-consistency checks: the triangles are
  assumed to form a sensible, non-self-intersecting surface. Overlapping or
  inconsistent-winding triangles are not detected.
- **Nearest-triangle search is O(N_tri) brute force** — every query tests
  every triangle. There is **no BVH / spatial acceleration structure yet**;
  this is adequate for small meshes and for verification, not for large
  production meshes (a BVH is future work).
- **One-sided contact.** The reported normal is the nearest triangle's stored
  outward normal; correct behaviour assumes the particle is on the outward
  side. A particle that has tunnelled fully behind a thin single-triangle
  sheet is not specially handled.
- **When the closest point lies on a shared edge or vertex** of the mesh, the
  contact normal is taken from whichever incident triangle is found nearest
  (ties broken by iteration order); the penetration `δ = r − d` uses the true
  Euclidean distance `d` to that closest point. A blended/averaged edge
  normal is not computed.
- **Rigid-body wall motion only** ([`MovingBoundary`]): pure translation plus
  rotation about a single moving pivot. No wall deformation, no per-vertex
  velocity fields. Rotation is applied to [`Boundary::Plane`],
  [`Boundary::Wall`], [`Boundary::Cylinder`], and [`MeshWall`] geometry; an
  **axis-aligned [`Boundary::Box`] is translated but not rotated** (a rotated
  AABB is no longer axis-aligned — wrap a [`MeshWall`] to rotate a
  box-shaped container).

# References (public literature — NOT LAMMPS/LIGGGHTS source)

- C. Ericson, *Real-Time Collision Detection* (Morgan Kaufmann, 2005),
  **§5.1.5 "Closest Point on Triangle to Point"** — the Voronoi-region
  barycentric closest-point algorithm used in [`Triangle::closest_point`].
- P. J. Schneider and D. H. Eberly, *Geometric Tools for Computer Graphics*
  (Morgan Kaufmann, 2003) — point/triangle distance geometry.
- O. Rodrigues, "Des lois géométriques qui régissent les déplacements d'un
  système solide…," *J. Math. Pures Appl.* **5**, 380–440 (1840) — the
  axis–angle rotation ("Rodrigues") formula used in [`MovingBoundary::advance`].
- H. Goldstein, C. Poole, J. Safko, *Classical Mechanics*, 3rd ed.
  (Addison-Wesley, 2002) — rigid-body kinematics, the surface-velocity
  relation `v = v_cm + ω × r`.
- T. Pöschel and T. Schwager, *Computational Granular Dynamics: Models and
  Algorithms* (Springer, 2005) — particle–wall overlap, contact-normal
  conventions, and relative-velocity handling for moving walls.
- J. Chen, A. B. Yu, et al. — the standard DEM triangulated-wall (STL)
  treatment in the granular literature (closest-point-on-facet contact
  detection); this is an independent reimplementation of that public method.

```rust
pub mod mesh_wall { /* ... */ }
```

### Types

#### Struct `Triangle`

A single flat triangular facet `{a, b, c}` of a triangulated wall surface.

# Fields and units

| Field | Quantity | SI unit |
|---|---|---|
| `a`, `b`, `c` | the three vertex positions | `[m]` |

# Outward normal and winding

The outward unit normal is `n̂ = normalize((b − a) × (c − a))` (see
[`Triangle::normal`]). Order the vertices **counter-clockwise as seen from
the domain (particle) side** so `n̂` points out of the solid toward the
particles — the same right-hand winding the STL file format uses. The three
vertices must not be collinear (a zero-area triangle has no defined normal);
[`Triangle::new`] enforces this.

`Copy` and stored inline (no heap allocation); a [`MeshWall`] owns a `Vec` of
these by value.

```rust
pub struct Triangle {
    pub a: crate::particle::Vec3,
    pub b: crate::particle::Vec3,
    pub c: crate::particle::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `crate::particle::Vec3` | First vertex `[m]`. |
| `b` | `crate::particle::Vec3` | Second vertex `[m]`. |
| `c` | `crate::particle::Vec3` | Third vertex `[m]`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated triangle from three vertices `[m]`.

- ```rust
  pub fn normal(self: &Self) -> Vec3 { /* ... */ }
  ```
  The outward **unit** normal `n̂ = normalize((b − a) × (c − a))`

- ```rust
  pub fn closest_point(self: &Self, p: Vec3) -> Vec3 { /* ... */ }
  ```
  The point on this triangle (interior, edge, or vertex) closest to `p`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Triangle { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Triangle) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `MeshWall`

A **triangulated (STL-style) wall**: a surface made of flat [`Triangle`]
facets.

Owns its facets by value in a `Vec` (indexed by `usize`), per the workspace
design rules — no trait objects, no lifetimes. Build one from any triangle
soup; the facets are assumed to share the winding convention on [`Triangle`]
so their outward normals all point into the domain.

[`MeshWall::particle_overlap`] finds, over **all** triangles (O(N_tri) brute
force — see the module "Honest scope"), the facet whose closest point is
nearest the particle centre, and reports the contact there.

```rust
pub struct MeshWall {
    pub triangles: Vec<Triangle>,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `triangles` | `Vec<Triangle>` | The triangular facets `[m]`. Assumed consistently wound (outward normals<br>point into the domain) and to form a sensible surface; see the module<br>"Honest scope" for what is *not* checked.<br><br>**If you mutate this directly**, the bounding-sphere cache below is<br>invalidated; [`MeshWall::particle_overlap`] detects the length mismatch<br>and falls back to the unaccelerated scan, so results stay correct but<br>slow. Rebuild with [`MeshWall::new`] to restore the acceleration. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(triangles: Vec<Triangle>) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a mesh wall from a non-empty list of facets.

- ```rust
  pub fn from_ascii_stl(text: &str) -> Result<Self, DemError> { /* ... */ }
  ```
  Parse an **ASCII STL** file body into a mesh wall.

- ```rust
  pub fn rebuild_bounds(self: &mut Self) { /* ... */ }
  ```
  Recompute the pruning caches from the current `triangles`.

- ```rust
  pub fn shift_bounds(self: &mut Self, disp: Vec3) { /* ... */ }
  ```
  Translate the pruning caches by `disp` `[m]`, the exact equivalent of

- ```rust
  pub fn particle_overlap(self: &Self, p: &Particle) -> Option<Contact> { /* ... */ }
  ```
  Geometric overlap of particle `p` (sphere of radius `r = p.radius` centred

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MeshWall { /* ... */ }
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
    fn eq(self: &Self, other: &MeshWall) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `WallGeometry`

The wall geometry a [`MovingBoundary`] carries: either an analytic
[`Boundary`] or a triangulated [`MeshWall`].

Enum dispatch (no `Box<dyn>`), per the workspace design rules — the set of
wall geometries is closed and known at compile time. This lets one
[`MovingBoundary`] type wrap *any* wall shape without trait objects.

```rust
pub enum WallGeometry {
    Analytic(crate::boundary::Boundary),
    Mesh(MeshWall),
}
```

##### Variants

###### `Analytic`

An analytic boundary primitive (plane, half-space wall, box, cylinder).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::boundary::Boundary` |  |

###### `Mesh`

A triangulated (STL-style) mesh wall.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `MeshWall` |  |

##### Implementations

###### Methods

- ```rust
  pub fn particle_overlap(self: &Self, p: &Particle) -> Option<Contact> { /* ... */ }
  ```
  Geometric overlap of particle `p` with the wrapped geometry, delegating to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> WallGeometry { /* ... */ }
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
    fn eq(self: &Self, other: &WallGeometry) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `MovingBoundary`

A **moving / rotating rigid wall**: a [`WallGeometry`] plus its rigid-body
kinematic state (translational velocity, angular velocity about a pivot).

# Purpose

This supplies the two things a moving-wall contact needs beyond static
geometry:

1. [`MovingBoundary::advance`] moves the geometry forward one time step so the
   next overlap query sees the wall in its new pose;
2. [`MovingBoundary::surface_velocity`] gives the wall's material velocity at
   a contact point, so the caller can form the particle-relative velocity
   `v_rel = v_particle − v_surface` that a wall-friction / damping law
   consumes.

**Force computation stays in the contact model.** This type provides geometry
(via [`MovingBoundary::particle_overlap`]) and kinematics only — no force.

# Fields and units

| Field | Quantity | SI unit |
|---|---|---|
| `geometry` | the wall shape (analytic or mesh) | positions `[m]` |
| `velocity` | translational velocity of the pivot / body | `[m/s]` |
| `angular_velocity` | angular velocity `ω` about the pivot | `[rad/s]` |
| `pivot` | the point the rotation is taken about | `[m]` |

The angular-velocity vector's direction is the rotation axis and its
magnitude the rotation rate (right-hand rule). A purely translating wall has
`angular_velocity = 0`; a wall spinning about a fixed axis has
`velocity = 0`.

```rust
pub struct MovingBoundary {
    pub geometry: WallGeometry,
    pub velocity: crate::particle::Vec3,
    pub angular_velocity: crate::particle::Vec3,
    pub pivot: crate::particle::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `geometry` | `WallGeometry` | The wall geometry, in its current pose. |
| `velocity` | `crate::particle::Vec3` | Translational velocity of the rigid body `[m/s]`. |
| `angular_velocity` | `crate::particle::Vec3` | Angular velocity `ω` about [`MovingBoundary::pivot`] `[rad/s]` (direction<br>= axis, magnitude = rate). |
| `pivot` | `crate::particle::Vec3` | The pivot point rotation is taken about `[m]`. Translates with the body<br>under [`MovingBoundary::advance`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(geometry: WallGeometry, velocity: Vec3, angular_velocity: Vec3, pivot: Vec3) -> Self { /* ... */ }
  ```
  Wrap `geometry` as a rigid wall with translational `velocity` `[m/s]`,

- ```rust
  pub fn surface_velocity(self: &Self, point: Vec3) -> Vec3 { /* ... */ }
  ```
  Material velocity of the wall surface at world point `point` `[m]`,

- ```rust
  pub fn particle_overlap(self: &Self, p: &Particle) -> Option<Contact> { /* ... */ }
  ```
  Geometric overlap of particle `p` with the wall in its **current** pose,

- ```rust
  pub fn advance(self: &mut Self, dt: f64) { /* ... */ }
  ```
  Advance the wall one time step `dt` `[s]`: rotate the geometry about the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MovingBoundary { /* ... */ }
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
    fn eq(self: &Self, other: &MovingBoundary) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `particle`

Phase 1 — **Particle framework** (bead `op-t3l.1`).

The fundamental DEM state carrier: a single spherical particle with
translational and rotational state, mass, radius, and temperature, plus
explicit velocity-Verlet time integration. Generic textbook DEM —
independent implementation, **not** derived from LIGGGHTS/LAMMPS source
(see the crate `NOTICE`).

# Honest scope (Phase 1)

This module implements **only** the single-particle data model and its
free-flight integration. It deliberately does **not** yet provide:

- contact mechanics / particle–particle or particle–wall forces (Phase 2),
- boundaries or walls (Phase 3),
- thermal DEM / heat transfer — `temperature` is carried as passive state
  and is **not** yet evolved (Phase 4),
- orientation / quaternion tracking — only the angular *velocity* vector is
  integrated; the particle's absolute orientation is not stored (a sphere's
  inertia is isotropic, so free rotation needs no orientation),
- any integrator other than velocity-Verlet.

No cross-code benchmark comparison has been run yet — that is the later
human validation step in this bead's Definition of Done. The verification
tests below check the integrator against closed-form analytical solutions
only.

# Unit convention

The **public constructor boundary** takes `uom` quantities
([`Mass`], [`Length`], [`ThermodynamicTemperature`]) so callers cannot pass
a dimensionally wrong scalar. Internally the particle stores plain `f64` in
**SI base units** (kilograms, metres, seconds, kelvin, radians). This split
is deliberate: the 3-vector kinematic state ([`Vec3`]) is bulk arithmetic in
a tight integration loop, where wrapping every component in a `uom`
`Quantity` would fight the vector algebra and add no safety the constructor
did not already give. Every stored field and every method therefore spells
out its SI unit in its doc comment, per the workspace `CLAUDE.md`
"f64-internal is acceptable if units are documented" allowance.

# References (public literature — NOT LAMMPS/LIGGGHTS source)

- P. A. Cundall and O. D. L. Strack, "A discrete numerical model for
  granular assemblies," *Géotechnique* **29**(1), 47–65 (1979).
- L. Verlet, "Computer 'Experiments' on Classical Fluids. I.," *Phys. Rev.*
  **159**, 98–103 (1967).
- W. C. Swope, H. C. Andersen, P. H. Berens, K. R. Wilson, "A computer
  simulation method …," *J. Chem. Phys.* **76**, 637–649 (1982) —
  velocity-Verlet form.
- H. Goldstein, C. Poole, J. Safko, *Classical Mechanics*, 3rd ed.
  (Addison-Wesley, 2002) — rigid-body rotation, solid-sphere inertia.

```rust
pub mod particle { /* ... */ }
```

### Types

#### Struct `Vec3`

A minimal 3-component Cartesian vector of `f64`, used for every kinematic
quantity in this crate (position, velocity, angular velocity, force,
torque).

This is a deliberately self-contained vector type: the DEM pillar is kept
independent of the CFD crates, so it does **not** reuse
`outram-foam-basic-lib`'s vector types. The physical meaning and SI unit of
a given `Vec3` depend on its use site and are documented there (e.g. a
position is in metres `[m]`, a velocity in `[m/s]`, an angular velocity in
`[rad/s]`). The type itself is unitless; it is `Copy` so it lives inline in
[`Particle`] with no heap allocation.

```rust
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` | x-component (SI unit set by the use site). |
| `y` | `f64` | y-component (SI unit set by the use site). |
| `z` | `f64` | z-component (SI unit set by the use site). |

##### Implementations

###### Methods

- ```rust
  pub const fn new(x: f64, y: f64, z: f64) -> Self { /* ... */ }
  ```
  Construct a vector from its three components (unit set by the use site).

- ```rust
  pub const fn zero() -> Self { /* ... */ }
  ```
  The zero vector `(0, 0, 0)`.

- ```rust
  pub fn add(self: Self, other: Self) -> Self { /* ... */ }
  ```
  Component-wise sum `self + other`. Both operands must share the same

- ```rust
  pub fn sub(self: Self, other: Self) -> Self { /* ... */ }
  ```
  Component-wise difference `self - other`. Both operands must share the

- ```rust
  pub fn scale(self: Self, s: f64) -> Self { /* ... */ }
  ```
  Scalar multiple `s * self`. If `self` has unit `[U]` and `s` has unit

- ```rust
  pub fn dot(self: Self, other: Self) -> f64 { /* ... */ }
  ```
  Euclidean dot product `self · other` (a scalar). For operands with units

- ```rust
  pub fn cross(self: Self, other: Self) -> Self { /* ... */ }
  ```
  Vector cross product `self × other`. For operands with units `[U]` and

- ```rust
  pub fn norm_squared(self: Self) -> f64 { /* ... */ }
  ```
  Squared Euclidean magnitude `self · self`. Cheaper than [`Vec3::norm`]

- ```rust
  pub fn norm(self: Self) -> f64 { /* ... */ }
  ```
  Euclidean magnitude (length) `‖self‖`, in the same unit as the vector's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Vec3 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Vec3) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Particle`

A single spherical DEM particle: its full kinematic state plus mass, radius,
and temperature.

# Fields and units

| Field | Quantity | SI unit |
|---|---|---|
| `position` | centre-of-mass position | `[m]` |
| `velocity` | centre-of-mass velocity | `[m/s]` |
| `angular_velocity` | angular velocity about the centre of mass | `[rad/s]` |
| `mass` | mass | `[kg]` |
| `radius` | sphere radius | `[m]` |
| `temperature` | absolute (thermodynamic) temperature | `[K]` |

All fields are stored as `f64` in SI base units; see the module-level
"Unit convention" note for why the `uom` boundary lives at the constructor
rather than on every field.

# Assumptions

- The particle is a homogeneous solid sphere (uniform density), so its
  moment of inertia is the isotropic solid-sphere value
  `I = (2/5) m r²` about any axis through the centre — see
  [`Particle::moment_of_inertia`].
- `mass > 0`, `radius > 0`, and `temperature > 0` K (enforced by
  [`Particle::new`]).
- `temperature` is passive Phase-1 state: it is stored but not evolved by
  [`Particle::integrate`] (thermal DEM is Phase 4).

```rust
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f64,
    pub radius: f64,
    pub temperature: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `Vec3` | Centre-of-mass position `[m]`. |
| `velocity` | `Vec3` | Centre-of-mass translational velocity `[m/s]`. |
| `angular_velocity` | `Vec3` | Angular velocity about the centre of mass `[rad/s]`. |
| `mass` | `f64` | Mass `[kg]`. Strictly positive (guaranteed by [`Particle::new`]). |
| `radius` | `f64` | Sphere radius `[m]`. Strictly positive (guaranteed by [`Particle::new`]). |
| `temperature` | `f64` | Absolute temperature `[K]`. Strictly positive. Passive in Phase 1. |

##### Implementations

###### Methods

- ```rust
  pub fn new(position: Vec3, velocity: Vec3, angular_velocity: Vec3, mass: Mass, radius: Length, temperature: ThermodynamicTemperature) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated particle.

- ```rust
  pub fn volume(self: &Self) -> f64 { /* ... */ }
  ```
  Volume of the sphere `[m³]`: `V = (4/3) π r³`.

- ```rust
  pub fn moment_of_inertia(self: &Self) -> f64 { /* ... */ }
  ```
  Moment of inertia of the particle `[kg·m²]`: `I = (2/5) m r²`.

- ```rust
  pub fn integrate(self: &mut Self, force: Vec3, torque: Vec3, dt: f64) { /* ... */ }
  ```
  Advance the particle one time step `dt` `[s]` under a constant applied

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Particle { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Particle) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `rdf`

Radial distribution function `g(r)` — the **structure** of a packed bed, as
opposed to its bulk packing fraction.

# What this measures and why a packing fraction is not enough

A solid fraction `φ` is one number: it says how much of the bed is pebble.
Two beds can share a `φ` and be structurally different — one crystallising
into ordered layers, one genuinely random — and that difference governs flow
resistance, effective conductivity and, for a pebble-bed reactor, the
neutron streaming paths. `g(r)` resolves it.

`g(r)` is the probability of finding another pebble centre at separation `r`
from a given centre, **relative to a uniform random arrangement of the same
mean density**. So:

- `g(r) = 1` means "as likely as random" — the value `g` tends to at large
  `r` in any disordered packing, by construction of the normalisation;
- `g(r) = 0` for `r < d` — hard spheres cannot interpenetrate, so the
  function is identically zero below one pebble diameter (a nonzero value
  there is a bug or an overlap, and this module's tests check exactly that);
- a **sharp first peak at `r = d`** is the contact shell: its area is the
  mean number of touching neighbours;
- the **split second peak**, at `r = √3 d ≈ 1.732 d` and `r = 2 d`, is the
  signature of a *random* close packing and is the single most-used
  diagnostic for distinguishing it from a crystalline one. In an FCC or HCP
  crystal the second-neighbour structure sits at `√2 d ≈ 1.414 d` instead.

That last point is why this module exists for the HTR-10 work: the question
is whether slow recirculation densifies the bed toward the published filling
fraction of 0.61, and *how*. A bed that densifies by crystallising is a
different physical claim from one that densifies while staying random, and
`φ` alone cannot tell them apart.

# The estimator, and why the domain is eroded

The normalisation is the whole difficulty. Naively,

```text
g(r) = <n(r)> / (rho * 4 * pi * r^2 * dr)
```

where `<n(r)>` is the mean number of neighbours in a shell. That is only
correct while the shell lies **entirely inside the bed**. Near a wall or the
free surface part of the shell is outside, no pebble can be there, and `g`
sags below 1 for a purely geometric reason that has nothing to do with
structure. In a bed 30 pebbles across, that artefact is large.

This module avoids it rather than correcting for it: **only pebbles at least
`r_max` from every boundary are used as shell centres** (an erosion of the
domain), while *every* pebble remains available as a neighbour. Every shell
is then fully enclosed, the normalisation above is exact, and no boundary
correction is needed or assumed. The price is fewer centres — for the HTR-10
bed at `r_max = 5 d` roughly a third of them — which costs statistics, not
correctness. [`Rdf::n_centres`] reports how many were used so a caller can
see what it paid.

The mean number density `rho` is measured **over the eroded region itself**,
so it is the local bulk density the shells actually sample, not a whole-bed
average contaminated by the loose free surface.

# Backends

The kernel is an all-pairs distance histogram — stateless, with no
dependence between pairs — which makes it the one part of this crate that
genuinely suits a GPU, and the reason [`ComputeType::Gpu`] exists here at
all. See [`crate::compute`] for the contrast with the DEM timestep, which
does not.

The two CPU backends are held to the **identical histogram**: binning is
integer counting and integer addition is associative, so a per-thread
histogram reduced in any order gives exactly the serial counts. No ordered
accumulation is needed here, unlike the DEM force loop.

**The GPU backend is not bit-identical, and cannot be.** WGSL has no `f64`,
so that kernel computes separations in `f32` while the CPU path uses `f64`.
A pair whose separation falls within `f32` rounding of a bin edge may land
in a neighbouring bin. The effect is bounded and small — `f32` resolves a
2 m coordinate to ~0.2 µm against a bin width of `d/50 = 1.2 mm` — but it is
real, and it is measured rather than asserted in `tests/htr10_rdf.rs`.
**Any number quoted in a V&V document comes from the CPU path.**

```rust
pub mod rdf { /* ... */ }
```

### Types

#### Struct `RdfDomain`

The region a bed occupies: a right circular cylinder with its axis along
`z`, which is the shape of every bed in this crate (core barrel, laboratory
cylinder, lifting cylinder).

Used only to decide which pebbles are far enough from a boundary to serve as
shell centres — see the module docs on erosion.

```rust
pub struct RdfDomain {
    pub radius: f64,
    pub z_min: f64,
    pub z_max: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `f64` | Cylinder radius `[m]`, about the `z` axis at `x = y = 0`. |
| `z_min` | `f64` | Lower bound of the occupied height `[m]`. |
| `z_max` | `f64` | Upper bound of the occupied height `[m]`. |

##### Implementations

###### Methods

- ```rust
  pub fn cylinder(radius: f64, z_min: f64, z_max: f64) -> Self { /* ... */ }
  ```
  A cylinder of `radius` spanning `z_min..z_max`, about the `z` axis.

- ```rust
  pub fn from_positions(centres: &[Vec3]) -> Self { /* ... */ }
  ```
  Infer the domain from the pebbles themselves: the largest cylindrical

- ```rust
  pub fn is_interior(self: &Self, p: Vec3, margin: f64) -> bool { /* ... */ }
  ```
  Whether `p` is at least `margin` `[m]` from the curved wall and from

- ```rust
  pub fn eroded_volume(self: &Self, margin: f64) -> f64 { /* ... */ }
  ```
  Volume `[m³]` of the region eroded by `margin` — the region

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RdfDomain { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RdfDomain) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `RdfSettings`

What to compute: how far out, and at what resolution.

```rust
pub struct RdfSettings {
    pub r_max: f64,
    pub n_bins: usize,
    pub domain: RdfDomain,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r_max` | `f64` | Largest separation `[m]` to histogram. Also the erosion margin, so<br>raising it costs shell centres quadratically — `5 d` is a good default<br>for a packed bed, comfortably past the split second peak at `2 d`. |
| `n_bins` | `usize` | Number of equal-width bins spanning `0..r_max`. |
| `domain` | `RdfDomain` | The region the bed occupies. |

##### Implementations

###### Methods

- ```rust
  pub fn for_diameter(d: f64, domain: RdfDomain) -> Self { /* ... */ }
  ```
  Default settings for a bed of pebble **diameter** `d`: out to `5 d` in

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RdfSettings { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RdfSettings) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Rdf`

A computed radial distribution function.

```rust
pub struct Rdf {
    pub r: Vec<f64>,
    pub g: Vec<f64>,
    pub coordination: Vec<f64>,
    pub counts: Vec<u64>,
    pub n_centres: usize,
    pub number_density: f64,
    pub dr: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `Vec<f64>` | Bin centre separations `[m]`, ascending, `n_bins` long. |
| `g` | `Vec<f64>` | `g(r)`, dimensionless, `n_bins` long. Tends to 1 at large `r`. |
| `coordination` | `Vec<f64>` | Running mean number of neighbours within each bin's outer edge — the<br>**cumulative coordination number**. `coordination[k]` counts every<br>neighbour out to `r[k] + dr/2`.<br><br>Read at the first minimum of `g` (just past the contact peak) this is<br>the contact coordination number, ~6 for a random loose packing and<br>~9–10 for a dense one. |
| `counts` | `Vec<u64>` | Raw pair counts per bin, before normalisation. Backends must agree on<br>these **exactly** — see the module docs. |
| `n_centres` | `usize` | How many pebbles survived the erosion to serve as shell centres. |
| `number_density` | `f64` | Mean number density `[1/m³]` over the eroded region, the `rho` of the<br>normalisation. |
| `dr` | `f64` | Bin width `[m]`. |

##### Implementations

###### Methods

- ```rust
  pub fn peak_r(self: &Self) -> f64 { /* ... */ }
  ```
  The separation `[m]` at which `g` is largest — the contact peak, which

- ```rust
  pub fn contact_coordination(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Coordination number at the first minimum of `g` after the contact peak

- ```rust
  pub fn to_csv_rows(self: &Self, diameter: f64) -> String { /* ... */ }
  ```
  Render as CSV rows (no header) for the V&V dataset:

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Rdf { /* ... */ }
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
    fn eq(self: &Self, other: &Rdf) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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

#### Function `radial_distribution`

**Attributes:**

- `MustUse { reason: None }`

Compute `g(r)` for a set of pebble centres.

# Methodology

See the module docs. In short: pebbles at least `r_max` from every boundary
serve as shell centres; every pebble is available as a neighbour; the number
density is measured over the eroded region; no boundary correction is
applied because none is needed.

# Arguments

`centres` are pebble centre positions `[m]`. `settings` fixes the range,
resolution and domain. `compute` selects the backend — all of which produce
the **same bin counts**, so this choice affects speed only.

# Panics

If `settings.n_bins` is zero, or `settings.r_max` is not finite and
positive.

# Returns

An [`Rdf`] with `n_bins` entries. If the erosion leaves no centres (a bed
smaller than `2 r_max` across) every `g` is `NaN` and `n_centres` is 0 —
deliberately, rather than silently returning a meaningless curve from an
un-eroded domain.

```rust
pub fn radial_distribution(centres: &[crate::particle::Vec3], settings: RdfSettings, compute: crate::compute::ComputeType) -> Rdf { /* ... */ }
```

## Module `rolling`

Phase 2 follow-up — **Rolling resistance & cohesion** contact extensions
(bead `op-t3l.2` follow-up).

Two independent, composable additions to the base normal/tangential contact
force of [`crate::contact`]:

- [`RollingModel`] — a resisting **torque** opposing the relative *rolling*
  of a contacting pair (directional constant torque, or viscous), and
- [`CohesionModel`] — an attractive **normal force** (a simple linear
  cohesive law, or the Johnson–Kendall–Roberts pull-off force).

Both are `enum`s dispatched by `match` (no `dyn`, per the workspace design
rules). They are deliberately kept **separate and additive**: neither
replaces the base contact force. A caller computes the base
[`crate::contact::ContactForce`] first, then *adds* the rolling torque
([`RollingModel::rolling_torque`]) to each particle's torque and *adds* the
cohesive normal scalar ([`CohesionModel::cohesive_force`]) to the base normal
force. This mirrors how DEM codes layer rolling/adhesion on top of a
Hooke/Hertz base — and keeps each piece unit-testable in isolation.

# Sign & geometry conventions (read once, applies everywhere)

These reuse the conventions of [`crate::contact`] so the pieces compose:

- **Normal** `n̂` points from `a`'s centre toward `b`'s centre.
- **Overlap** `δ_n` `[m]` is `(r_a + r_b) − ‖x_b − x_a‖`: `δ_n > 0` in
  contact, `δ_n < 0` means a surface *gap* of magnitude `−δ_n`.
- **Cohesive normal scalar** uses the **same sign as
  [`crate::contact::ContactLaw::normal_force_scalar`]**: *positive is
  repulsive*, so a cohesive (attractive) force is returned **negative**. A
  caller adds it to the base normal scalar `F_n` and applies the total along
  `−n̂` on `a` exactly as the base contact does; a net-negative total then
  points along `+n̂` (a pulled toward b), i.e. attraction.
- **Relative rolling angular velocity** `ω_rel = ω_a − ω_b` `[rad/s]` is the
  pair's rolling rate; the resistance torque opposes it. The returned torque
  on `a` and on `b` form a **couple** (`τ_b = −τ_a`): rolling resistance is a
  pure moment that does no net work on the pair's centre of mass.

# Unit convention

Following the crate convention (see [`crate::particle`] and
[`crate::contact`]), physical scalars are plain `f64` in **SI base units**
with the unit spelled out in each doc comment. The model coefficients here
(`μ_r` dimensionless, `c_r` in `[N·m·s]`, `k_c` in `[N/m]`, surface energy in
`[J/m²]`) have no ergonomic named `uom` alias, so — exactly as the Hooke
stiffnesses in [`crate::contact::HookeContact`] — they are documented `f64`.
Every constructor still validates its inputs' physical range.

# Honest scope

These are **clean-room, unit-tested foundations, not benchmark-validated**
models — no cross-code (e.g. LIGGGHTS) or experimental comparison has been
run; that is a later human validation step. The inline tests check the laws
against **hand-computed analytical values** only.

Deliberately **out of scope** (documented, not silently missing):

- **No history-dependent rolling spring (Ai et al. "Model C" /
  elastic–plastic spring-dashpot rolling resistance).** Both rolling models
  here are *stateless snapshots*: the constant-torque model needs no history,
  and the viscous model depends only on the instantaneous `ω_rel`. A rolling
  spring that accumulates a rolling displacement over the contact lifetime
  (Iwashita & Oda 1998; Ai et al. Model C) is **not** implemented — it needs
  per-contact state the caller's loop would have to carry.
- **No liquid-bridge / capillary cohesion** (pendular-bridge, van der Waals,
  or electrostatic adhesion). The cohesion here is a dry linear law and the
  JKR elastic pull-off force only.
- **No full JKR force–displacement curve** and **no hysteresis**. Only the
  JKR **pull-off (maximum adhesive) force** `F_pull = (3/2)π γ R*` is
  modelled, applied as a constant attractive force while the pair is in
  contact. The hysteretic neck (contact radius from the JKR cubic, tension
  sustained across a gap up to snap-off) is not solved.
- **No plastic, bonded, or parallel-bond contacts.**

# References (public literature — NOT LAMMPS/LIGGGHTS source)

- J. Ai, J.-F. Chen, J. M. Rotter, J. Y. Ooi, "Assessment of rolling
  resistance models in discrete element simulations," *Powder Technology*
  **206**(3), 269–282 (2011) — the canonical review classifying rolling
  resistance into the directional constant-torque ("Model A"), viscous, and
  elastic–plastic spring-dashpot ("Model C") families used here.
- K. Iwashita and M. Oda, "Rolling resistance at contacts in simulation of
  shear band development by DEM," *J. Eng. Mech.* **124**(3), 285–292
  (1998) — rolling resistance in granular DEM (the history-dependent rolling
  spring is cited but deliberately *not* implemented; see "Honest scope").
- K. L. Johnson, K. Kendall, A. D. Roberts, "Surface energy and the contact
  of elastic solids," *Proc. R. Soc. Lond. A* **324**(1558), 301–313
  (1971) — the JKR adhesion theory; pull-off force `F_pull = (3/2)π γ R*`.

```rust
pub mod rolling { /* ... */ }
```

### Types

#### Struct `RollingTorque`

The resisting rolling torque applied to each particle of a contacting pair.

Both torques are in newton-metres `[N·m]`. They form a **couple**:
`torque_on_b = −torque_on_a` exactly, so the pair feels a pure moment
resisting its relative rolling with no net force on the centre of mass.

```rust
pub struct RollingTorque {
    pub torque_on_a: crate::particle::Vec3,
    pub torque_on_b: crate::particle::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `torque_on_a` | `crate::particle::Vec3` | Rolling-resistance torque on particle `a` about its centre `[N·m]`,<br>directed to oppose the relative rolling `ω_rel = ω_a − ω_b`. |
| `torque_on_b` | `crate::particle::Vec3` | Rolling-resistance torque on particle `b` about its centre `[N·m]`.<br>Equals `−torque_on_a` (the reaction of the couple). |

##### Implementations

###### Methods

- ```rust
  pub const fn zero() -> Self { /* ... */ }
  ```
  The zero couple (no rolling resistance).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RollingTorque { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RollingTorque) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `RollingModel`

Closed set of rolling-resistance models, dispatched by `match` with **no**
`dyn` / heap allocation (per the workspace design rules).

Each variant, given the contact normal-force magnitude `|F_n|` `[N]`, the
effective rolling radius `R*` `[m]`, and the relative rolling angular
velocity `ω_rel` `[rad/s]`, returns the resisting [`RollingTorque`] via
[`RollingModel::rolling_torque`].

# Effective rolling radius

`R*` is the reduced radius of the pair, `R* = r_a·r_b / (r_a + r_b)` `[m]`,
the same reduced radius used by the contact models — it is passed in by the
caller (who already has it from the contact geometry), not recomputed here.

# Variants and parameters

| Variant | Symbol | Quantity | SI unit | Valid range |
|---|---|---|---|---|
| `None` | — | no rolling resistance | — | — |
| `ConstantDirectionalTorque` | `μ_r` | rolling-friction coefficient | `[-]` | `≥ 0` |
| `ViscousRolling` | `c_r` | rolling viscous-damping coefficient | `[N·m·s]` | `≥ 0` |

```rust
pub enum RollingModel {
    None,
    ConstantDirectionalTorque {
        mu_r: f64,
        torsion_torque: bool,
    },
    ViscousRolling {
        c_r: f64,
    },
}
```

##### Variants

###### `None`

No rolling resistance — always returns [`RollingTorque::zero`].

###### `ConstantDirectionalTorque`

**Directional constant-torque (CDT)** rolling resistance — a port of
upstream LIGGGHTS `rolling_model_cdt.h` (Ai et al. 2011 "Model A";
Iwashita & Oda 1998).

The resisting torque has a fixed magnitude set by the **elastic** normal
load and always points opposite the relative rolling direction:

`M_r = −μ_r · R* · (k_n·δ_n) · ω̂_rel`,  with  `ω̂_rel = ω_rel / ‖ω_rel‖`,

followed by removal of the component along the contact normal (the
*torsion* part) unless `torsion_torque` is set.

When `‖ω_rel‖` is below [`ROLLING_OMEGA_EPS`] the direction is undefined
and the torque is zero (a non-rolling pair feels no rolling resistance).
This is the *directional* form: the magnitude does not depend on the
rolling *speed*, only its direction — hence "constant torque".

# Faithfulness note (fixed 2026-09-16)

Until 2026-09-16 this variant scaled the torque by the **total** normal
force `|F_n|` (including the viscous damping term) and kept the torsion
component. Upstream does neither: it uses the elastic part `k_n·δ_n`
only, and removes torsion unless `torsionTorque` is explicitly asked
for (that switch defaults **off**). Both are now upstream's.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mu_r` | `f64` | Rolling-friction coefficient `μ_r` `[-]`. Non-negative. |
| `torsion_torque` | `bool` | Upstream's `torsionTorque` switch. **Default `false`**, which<br>removes the torque component along the contact normal. |

###### `ViscousRolling`

**Viscous** rolling resistance (Ai et al. 2011, viscous family).

> **Not an upstream LIGGGHTS model.** LIGGGHTS ships `cdt`, `epsd`,
> `epsd2`, `epsd3` and `luding`; a pure linear rolling dashpot is not
> among them. This variant is a clean-room addition from the DEM
> literature and is therefore **not** covered by the cross-code
> verification in `docs/verification-and-validation.md`.

The resisting torque is linear in the relative rolling angular velocity:

`M_r = −c_r · ω_rel`.

Unlike the constant-torque form this scales with rolling *speed* and its
direction follows `−ω_rel` continuously (so it needs no `ω̂_rel`
normalisation and is exactly zero at `ω_rel = 0`). The pure linear
dashpot is used here; the optional cap of the viscous torque at the
constant-torque limit `μ_r R*|F_n|` (Ai et al.) is **not** applied — see
the module "Honest scope".

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `c_r` | `f64` | Rolling viscous-damping coefficient `c_r` `[N·m·s]`<br>(torque per unit rolling angular velocity). Non-negative. |

##### Implementations

###### Methods

- ```rust
  pub const fn none() -> Self { /* ... */ }
  ```
  Construct the no-resistance model.

- ```rust
  pub fn constant_directional_torque(mu_r: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated directional constant-torque model (Ai et al.

- ```rust
  pub fn viscous(c_r: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated viscous rolling-resistance model.

- ```rust
  pub fn rolling_torque(self: &Self, elastic_normal_force: f64, r_eff: f64, omega_rel: Vec3, contact_normal: Vec3) -> RollingTorque { /* ... */ }
  ```
  Resisting rolling torque on each particle of the pair.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RollingModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RollingModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `CohesionModel`

Closed set of cohesion (attractive-normal-force) models, dispatched by
`match` with **no** `dyn` / heap allocation (per the workspace design rules).

Each variant, given the signed normal overlap `δ_n` `[m]` (`> 0` overlap,
`< 0` a gap of `−δ_n`) and the effective radius `R*` `[m]`, returns a
cohesive normal **scalar** `[N]` via [`CohesionModel::cohesive_force`]. The
scalar uses the base-contact sign convention (positive repulsive), so a
cohesive force is **negative** and a caller adds it to the base normal
scalar.

# Variants and parameters

| Variant | Symbol | Quantity | SI unit | Valid range |
|---|---|---|---|---|
| `None` | — | no cohesion | — | — |
| `LinearCohesion` | `k_c` | cohesive stiffness | `[N/m]` | `≥ 0` |
| `LinearCohesion` | `max_gap` | cohesion cut-off separation | `[m]` | `> 0` |
| `Jkr` | `γ` | surface energy (work of adhesion) | `[J/m²]` | `≥ 0` |

```rust
pub enum CohesionModel {
    None,
    LinearCohesion {
        k_c: f64,
        max_gap: f64,
    },
    Jkr {
        surface_energy: f64,
    },
}
```

##### Variants

###### `None`

No cohesion — always returns `0` from [`CohesionModel::cohesive_force`].

###### `LinearCohesion`

Simple **linear cohesion**: an attractive force that ramps linearly with
surface separation and vanishes beyond a cut-off gap.

Let the surface gap be `g = max(−δ_n, 0)` `[m]` (`0` while the pair
overlaps). The cohesive scalar is

`F_c = −k_c · (max_gap − g)`  for  `0 ≤ g ≤ max_gap`,

held at its peak `−k_c · max_gap` `[N]` while overlapping (`δ_n ≥ 0`,
`g = 0`), and `0` once the gap exceeds `max_gap`. Thus attraction is
strongest at/inside contact and falls **linearly** to zero at the cut-off
— a dry, non-hysteretic cohesive law. `k_c` `[N/m]` is a cohesive
*stiffness*; the peak attractive force is `k_c · max_gap` `[N]`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `k_c` | `f64` | Cohesive stiffness `k_c` `[N/m]`. Non-negative. |
| `max_gap` | `f64` | Cohesion cut-off separation `max_gap` `[m]`: beyond this surface gap<br>the attractive force is zero. Strictly positive. |

###### `Jkr`

**Johnson–Kendall–Roberts (JKR)** adhesion — pull-off force only.

Applies the constant JKR **pull-off (maximum adhesive) force**

`F_pull = (3/2) · π · γ · R*`  `[N]`

as an attractive scalar `−F_pull` while the pair is in contact
(`δ_n ≥ 0`), and `0` when separated (`δ_n < 0`). `γ` `[J/m²]` is the
surface energy (work of adhesion). Only the pull-off magnitude is
modelled — the full nonlinear JKR force–displacement curve and its
hysteresis are out of scope (see the module "Honest scope").

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `surface_energy` | `f64` | Surface energy / work of adhesion `γ` `[J/m²]`. Non-negative. |

##### Implementations

###### Methods

- ```rust
  pub const fn none() -> Self { /* ... */ }
  ```
  Construct the no-cohesion model.

- ```rust
  pub fn linear(k_c: f64, max_gap: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated linear-cohesion model.

- ```rust
  pub fn jkr(surface_energy: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a validated JKR adhesion model.

- ```rust
  pub fn max_attractive_force(self: &Self, r_eff: f64) -> f64 { /* ... */ }
  ```
  The **maximum attractive force magnitude** `[N]` this model can exert for

- ```rust
  pub fn cohesive_force(self: &Self, overlap: f64, r_eff: f64) -> f64 { /* ... */ }
  ```
  Cohesive normal **scalar** `[N]` for the given signed overlap and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CohesionModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &CohesionModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `simulation`

Phase 5 — **Multi-particle DEM simulation engine** (bead `op-t3l`).

This module ties the crate's verified foundations —
[`Particle`](crate::particle::Particle) (velocity-Verlet integration),
[`ContactModel`](crate::contact::ContactModel) (Hooke / Hertz pairwise
forces), and [`Boundary`](crate::boundary::Boundary) (particle–wall overlap
geometry) — into a single runnable **soft-sphere DEM** engine,
[`DemSimulation`]. It owns an ensemble of spheres, a set of static wall
boundaries, one contact law, a uniform gravitational body force, and a fixed
time step, and advances them with an explicit velocity-Verlet loop.

# The DEM time-step (one call to [`DemSimulation::step`])

A soft-sphere discrete-element step is the standard force-accumulate /
integrate cycle of Cundall & Strack (1979):

1. **Neighbour search.** Build a fresh uniform cell list and enumerate the
   candidate near pairs (see "Neighbour search" below).
2. **Zero accumulators.** Reset every particle's force `[N]` and torque
   `[N·m]` accumulator to zero for this step.
3. **Pairwise contact forces.** For each candidate pair, evaluate the
   [`ContactModel`](crate::contact::ContactModel); if the pair overlaps,
   accumulate the equal-and-opposite forces and the contact-point torques on
   the two partners (Newton's third law).
4. **Particle–wall forces.** For each particle overlapping a boundary,
   convert the geometric [`Contact`](crate::boundary::Contact) into a force
   with the **same** contact law by pairing the particle against an
   immovable "image" partner (see "Particle–wall coupling" below), and
   accumulate the force/torque on the particle only.
5. **Body force.** Add gravity `F = m·g` `[N]` to every particle.
6. **Integrate.** Advance every particle one velocity-Verlet step of size
   `dt` `[s]` with its accumulated force and torque.

# Neighbour search — uniform linked-cell (cell list)

Finding which of `N` particles are close enough to touch is the dominant
cost of a naive DEM step, `O(N²)` if every pair is tested. The textbook fix
is the **uniform cell list** (also "linked-cell" or "spatial hash"): overlay
the domain with a regular grid whose cell edge equals the **largest particle
diameter**, bin each particle into the cell containing its centre, and then
test a particle only against the particles in its own cell and the 26
neighbouring cells (a `3×3×3` stencil). Because the cell edge is one full
diameter, any two spheres that overlap (centre distance `< r_a + r_b ≤`
max diameter) are guaranteed to fall in the same or adjacent cells, so the
stencil misses no real contact. With a roughly uniform density the work is
`O(N)` instead of `O(N²)`. See Allen & Tildesley (1987) §5.3.2 and Pöschel &
Schwager (2005) §3.2.

Each unordered pair is emitted **once**: while visiting particle `i` we take
a stencil neighbour `j` only when `j > i`. For very small ensembles the cell
machinery is pure overhead, so at or below
[`DemSimulation::BRUTE_FORCE_THRESHOLD`] particles the engine falls back to a
direct all-pairs `O(N²)` scan; both paths return the identical set of
contacts (verified by [`tests::cell_list_matches_brute_force`]).

# Particle–wall coupling — the immovable image partner

[`Boundary::particle_overlap`](crate::boundary::Boundary::particle_overlap)
returns only geometry: a penetration depth `δ` `[m]` and an inward unit
normal `n̂_c` (pointing from the wall surface back into the domain, toward the
particle centre). To turn that into a force **using the same contact law as
the particle–particle contacts** — so a wall and a neighbouring grain are
modelled consistently — the engine builds a fictitious **image partner**: a
sphere of the *same radius and mass* as the real particle, held motionless
(zero linear and angular velocity — the walls here are static), placed on the
far (solid) side of the surface so that the pair geometry reproduces exactly
the wall overlap `δ` and normal `n̂_c`.

Concretely, with the real particle as contact partner `a` and the image as
`b`, the image centre is `c_b = c_a − n̂_c·(2r − δ)`. The pairwise law then
sees line-of-centres normal `n̂ = (c_b − c_a)/‖…‖ = −n̂_c`, centre distance
`2r − δ`, and hence overlap `(r + r) − (2r − δ) = δ` — the wall penetration —
and produces a repulsive force on `a` directed along `−n̂ = +n̂_c`, i.e. back
into the domain, exactly as a wall should. The force and contact-point torque
on the real particle are kept; the reaction on the (infinitely heavy,
immovable) wall is discarded. Because the image mirrors the particle, the
reduced quantities the Hertz law derives are `R* = r/2` and `m* = m/2`; this
is the standard "image-particle" wall convention (Pöschel & Schwager 2005,
§3.3). A rigid flat wall would instead have `R* = r` and `m* = m`; the
difference only rescales the Hertz normal stiffness / damping prefactor and
is documented here rather than silently chosen. For the linear
[`HookeContact`](crate::contact::HookeContact) law — whose scalar force
ignores `R*` and `m*` entirely — the two conventions coincide, so the wall
force is `k_n·δ (+ γ_n·v_n)` with no ambiguity.

# Unit convention

Following the crate convention (see [`crate::particle`]), state is stored as
plain `f64` in **SI base units** with the unit spelled out on every field and
method: positions `[m]`, velocities `[m/s]`, forces `[N]`, torques `[N·m]`,
gravity `[m/s²]`, time and step `[s]`, energy `[J]`, momentum `[kg·m/s]`.

# Honest scope (Phase 5)

This is a **verified engine, not a validated one** — no cross-code or
experimental benchmark comparison has been run. The inline tests check the
loop against conservation laws (linear momentum), analytical rest states, and
internal consistency (cell list vs brute force), **not** against a reference
DEM code; that quantitative validation is the later human step in this bead's
Definition of Done. The engine deliberately implements **only**:

- **Spheres**, single-threaded, on a **single uniform** cell size equal to
  the largest particle diameter. A strongly **polydisperse** packing (a wide
  spread of radii) therefore wastes work — the cell is sized for the biggest
  grain, so cells hold many small ones; a multi-level / per-size grid is
  future work, noted but not done.
- **Static** boundaries only: the image partner carries zero velocity, so
  moving/rotating walls are not modelled (they are out of scope in
  [`crate::boundary`] too).
- **No periodic boundaries** — the domain is open unless walled.
- A **stateless (history-free) tangential** contact: the engine passes the
  instantaneous particle states to the contact law each step and carries **no**
  accumulated tangential spring displacement `ξ_t` between steps (see the
  [`crate::contact`] "Honest scope" note). Tangential friction is therefore
  the Coulomb-capped dashpot term only; a history-dependent tangential spring
  would require per-contact state keyed by particle pair across steps and is
  future work.
- **No broad-phase parallelism.** The step is single-threaded; a
  thread-per-cell or spatial-decomposition parallel step (e.g. via `rayon`)
  is noted as future work and intentionally not added here.

# References (public literature — NOT LAMMPS/LIGGGHTS source)

- M. P. Allen and D. J. Tildesley, *Computer Simulation of Liquids* (Oxford
  University Press, 1987) — §5.3.2 cell lists / neighbour lists, and the
  velocity-Verlet propagator.
- T. Pöschel and T. Schwager, *Computational Granular Dynamics: Models and
  Algorithms* (Springer, 2005) — §3.2 linked-cell neighbour search, §3.3
  particle–wall (image-particle) contact, and the soft-sphere DEM loop.
- P. A. Cundall and O. D. L. Strack, "A discrete numerical model for granular
  assemblies," *Géotechnique* **29**(1), 47–65 (1979) — the soft-sphere DEM
  force-accumulate / explicit-integrate time step.
- L. Verlet, "Computer 'Experiments' on Classical Fluids. I.," *Phys. Rev.*
  **159**, 98–103 (1967); W. C. Swope et al., *J. Chem. Phys.* **76**, 637
  (1982) — the velocity-Verlet integrator used per particle.

```rust
pub mod simulation { /* ... */ }
```

### Types

#### Struct `DemSimulation`

A runnable multi-particle **soft-sphere DEM** simulation.

Owns its particle ensemble **by value** in a `Vec<Particle>` (particles are
referenced elsewhere by their `usize` index, never by reference or `Box`,
per the workspace design rules), a set of static wall
[`Boundary`](crate::boundary::Boundary) primitives, one
[`ContactModel`](crate::contact::ContactModel) applied to every contact, a
uniform gravitational acceleration, and a fixed integration time step.

Advance the system with [`DemSimulation::step`] (one velocity-Verlet step) or
[`DemSimulation::run`] (many). Query the state with
[`DemSimulation::particles`], [`DemSimulation::kinetic_energy`],
[`DemSimulation::total_momentum`], and [`DemSimulation::time`].

# Fields and units

| Field | Quantity | SI unit |
|---|---|---|
| `particles` | the sphere ensemble (state carriers) | mixed (see [`Particle`]) |
| `boundaries` | static domain walls | mixed (see [`Boundary`](crate::boundary::Boundary)) |
| `contact_model` | the pairwise & particle–wall force law | — |
| `gravity` | uniform gravitational acceleration `g` | `[m/s²]` |
| `dt` | fixed integration time step | `[s]` |
| `time` | elapsed simulated time since construction | `[s]` |

```rust
pub struct DemSimulation {
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
  pub fn new(particles: Vec<Particle>, boundaries: Vec<Boundary>, contact_model: ContactModel, gravity: Vec3, dt: f64) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a DEM simulation from its ensemble, walls, contact law, gravity,

- ```rust
  pub fn particles(self: &Self) -> &[Particle] { /* ... */ }
  ```
  The particle ensemble, as an immutable slice `[m]`/`[m/s]`/… (see

- ```rust
  pub fn boundaries(self: &Self) -> &[Boundary] { /* ... */ }
  ```
  The static wall boundaries of the domain.

- ```rust
  pub fn contact_model(self: &Self) -> ContactModel { /* ... */ }
  ```
  The contact force law applied to every contact.

- ```rust
  pub fn gravity(self: &Self) -> Vec3 { /* ... */ }
  ```
  The uniform gravitational acceleration `g` `[m/s²]`.

- ```rust
  pub fn dt(self: &Self) -> f64 { /* ... */ }
  ```
  The fixed integration time step `dt` `[s]`.

- ```rust
  pub fn time(self: &Self) -> f64 { /* ... */ }
  ```
  Elapsed simulated time since construction `[s]` (advances by `dt` each

- ```rust
  pub fn num_particles(self: &Self) -> usize { /* ... */ }
  ```
  Number of particles in the ensemble.

- ```rust
  pub fn kinetic_energy(self: &Self) -> f64 { /* ... */ }
  ```
  Total kinetic energy of the ensemble `[J]`.

- ```rust
  pub fn total_momentum(self: &Self) -> Vec3 { /* ... */ }
  ```
  Total linear momentum of the ensemble `[kg·m/s]`: `Σ mᵢ·vᵢ`.

- ```rust
  pub fn step(self: &mut Self) { /* ... */ }
  ```
  Advance the whole system by one **velocity-Verlet** step of size `dt`

- ```rust
  pub fn run(self: &mut Self, n_steps: usize) { /* ... */ }
  ```
  Run [`DemSimulation::step`] `n_steps` times, advancing the system by

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DemSimulation { /* ... */ }
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
    fn eq(self: &Self, other: &DemSimulation) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `thermal`

Phase 4 — **Thermal DEM** (bead `op-t3l.4`).

Particle–particle and particle–wall **contact conduction**: the rate at
which heat flows through the small circular contact spot where two touching
solids meet, plus an explicit-Euler temperature update that evolves each
particle's [`Particle::temperature`] field (passive in Phase 1) from the net
conductive heat rate. Clean-room implementation from public granular
heat-transfer literature — **not** derived from LIGGGHTS/LAMMPS source (see
the crate `NOTICE`).

# Physical model

When two solid spheres touch over a circular contact of radius `a_c`, heat
flows across the contact by conduction through the constriction. The
constriction resistance of a circular spot of radius `a_c` into a
semi-infinite solid of conductivity `k` is `R = 1/(4·k·a_c)` (the classical
Maxwell/Holm constriction result). Two solids meeting at the contact place
two such resistances in series, so the pair conductance is

```text
  h_c = 1 / (R_i + R_j) = 4·a_c / (1/k_i + 1/k_j) = 2·k_s·a_c,
```

where `k_s = 2·k_i·k_j / (k_i + k_j)` is the **harmonic mean** of the two
solid conductivities. This is the Batchelor & O'Brien (1977) single-contact
conductance, in the form used by Vargas & McCarthy (2001, 2002) for DEM heat
conduction. The conductive heat rate **into** particle `i` from a touching
partner `j` is then

```text
  Q_ij = h_c · (T_j − T_i)   [W],
```

which is antisymmetric (`Q_ji = −Q_ij`), so a two-body exchange conserves
energy exactly. Summing `Q` over a particle's contacts gives its net heat
rate `Q_net`, and an explicit forward-Euler step advances its temperature by

```text
  dT_i/dt = Q_net / (m_i·c_p)   ⇒   T_i(t+dt) = T_i(t) + Q_net·dt / (m_i·c_p).
```

# Contact radius

The contact radius `a_c` couples this thermal model to the mechanical
contact. For **Hertzian** elastic contact the mutual approach (overlap) `δ`
and the contact radius are related by `δ = a_c² / R*`, i.e.
`a_c = sqrt(R*·δ)`, with the effective (reduced) radius
`R* = r_i·r_j / (r_i + r_j)` — see [`hertzian_contact_radius`] and
[`effective_radius`]. (A purely *geometric* truncation of two overlapping
spheres gives the slightly larger `a_c = sqrt(2·R*·δ)`; the elastic value is
smaller because the surfaces deform rather than interpenetrate.) The
conductance functions here take `a_c` **as an input** so the caller may
supply it from a Hertzian mechanical solve (Phase 2), from the geometric
overlap, or — for particle–wall contact — from the boundary geometry
(Phase 3) without this module depending on those phases.

# Honest scope (Phase 4)

This module implements **only** solid–solid **contact conduction** and the
single-step temperature update. It deliberately does **not** provide:

- **thermal radiation** between particles or to walls (Stefan–Boltzmann,
  view factors) — a separate, later mode;
- **gas/film conduction** through the interstitial fluid or the near-contact
  gas gap (e.g. the Batchelor–O'Brien fluid-lens or Rong–Horio correction),
  nor any pressure/Knudsen dependence of it;
- **convection** or any CFD-DEM fluid coupling (that is Phase 5 / the CFD-DEM
  seam) — the surrounding fluid is treated as thermally inert here;
- internal temperature gradients within a particle (each particle is a
  single lumped, isothermal node — the Biot-number validity limit of the
  lumped-capacitance assumption is the caller's responsibility);
- any implicit or higher-order time integration (forward Euler only).

No cross-code benchmark comparison has been run yet — the tests below are
**verification** against hand-computed closed forms (conductance, flux sign
and magnitude, energy balance, one Euler step), not **validation** against a
reference DEM code or experiment. That validation is the later human step in
this bead's Definition of Done.

# Unit convention

Following [`crate::particle`], the numeric API is plain `f64` in SI base
units, with every parameter's unit spelled out in its doc comment: thermal
conductivity `k` `[W/m/K]`, contact radius `a_c` and radius `r` `[m]`,
overlap `δ` `[m]`, temperature `T` `[K]`, conductance `h_c` `[W/K]`, heat
rate `Q` `[W]`, mass `m` `[kg]`, specific heat capacity `c_p` `[J/kg/K]`,
time step `dt` `[s]`.

# References (public literature — NOT LAMMPS/LIGGGHTS source)

- G. K. Batchelor and R. W. O'Brien, "Thermal or electrical conduction
  through a granular material," *Proc. R. Soc. Lond. A* **355**(1682),
  313–333 (1977). — single-contact conductance `h_c = 2·k_s·a_c`.
- W. L. Vargas and J. J. McCarthy, "Heat conduction in granular materials,"
  *AIChE Journal* **47**(5), 1052–1059 (2001). — DEM particle-scale form of
  the contact conductance with harmonic-mean conductivity.
- W. L. Vargas and J. J. McCarthy, "Stress effects on the conductivity of
  particulate beds," *Chem. Eng. Sci.* **57**(15), 3119–3131 (2002). —
  Hertzian contact-radius coupling of conductance to load/overlap.
- H. Hertz, "Über die Berührung fester elastischer Körper," *J. reine angew.
  Math.* **92**, 156–171 (1882); K. L. Johnson, *Contact Mechanics*
  (Cambridge University Press, 1985), §4 — `δ = a_c²/R*`, `R* = r_i r_j/(r_i+r_j)`.

```rust
pub mod thermal { /* ... */ }
```

### Types

#### Enum `ThermalModel`

A contact-conduction thermal model.

Enum dispatch (no trait objects), per the workspace design rules: the set of
conductance laws is closed and known at compile time, so adding a variant
forces every `match` to handle it. The single method
[`ThermalModel::conductance`] maps a contact radius to a pair conductance
`h_c` `[W/K]`.

The same model type serves both particle–particle and particle–wall contact:
for a wall, build [`ThermalModel::ContactConduction`] from the particle's and
the wall's conductivities and pass the sphere–wall contact radius (e.g. from
[`hertzian_contact_radius`] with [`sphere_wall_effective_radius`], or from
Phase 3 boundary geometry).

```rust
pub enum ThermalModel {
    ContactConduction {
        k_i: f64,
        k_j: f64,
    },
    Constant(f64),
}
```

##### Variants

###### `ContactConduction`

Batchelor–O'Brien contact conduction. Stores the two solid thermal
conductivities `k_i`, `k_j` `[W/m/K]`; the conductance for a contact of
radius `a_c` `[m]` is `h_c = 2·k_s·a_c` with `k_s` the harmonic mean.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `k_i` | `f64` | Thermal conductivity of body `i` `[W/m/K]` (a particle). |
| `k_j` | `f64` | Thermal conductivity of body `j` `[W/m/K]` (the touching particle or wall). |

###### `Constant`

A directly-prescribed constant pair conductance `[W/K]`, independent of
contact radius. Useful for a fixed-conductance boundary condition or for
isolating the temperature-update logic in tests. The stored value is the
conductance `h_c` itself.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn conductance(self: &Self, contact_radius: f64) -> Result<f64, DemError> { /* ... */ }
  ```
  Pair conductance `h_c` `[W/K]` for a contact of radius

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermalModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ThermalModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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

#### Function `harmonic_mean_conductivity`

Harmonic-mean solid thermal conductivity `k_s` `[W/m/K]` of two contacting
materials with conductivities `k_i`, `k_j` `[W/m/K]`.

`k_s = 2·k_i·k_j / (k_i + k_j)`. This is the conductivity that makes the two
series constriction resistances combine into the single-contact conductance
`h_c = 2·k_s·a_c` (Batchelor & O'Brien 1977; Vargas & McCarthy 2001). For
equal conductivities it reduces to their common value `k_s = k`.

# Errors

Returns [`DemError::InvalidInput`] if either conductivity is not strictly
positive (a non-positive thermal conductivity is unphysical for a solid).

```rust
pub fn harmonic_mean_conductivity(k_i: f64, k_j: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `effective_radius`

Effective (reduced) contact radius `R*` `[m]` of two spheres of radii
`r_i`, `r_j` `[m]`: `R* = r_i·r_j / (r_i + r_j)`.

This is the standard reduced radius of Hertzian contact (Johnson, *Contact
Mechanics*, 1985) and appears in the overlap–contact-radius relation used by
[`hertzian_contact_radius`]. For a sphere against a flat wall, pass the
wall's radius as `+∞`; the limit `R* → r_i` is recovered by
[`sphere_wall_effective_radius`].

# Errors

Returns [`DemError::InvalidInput`] if either radius is not strictly positive.

```rust
pub fn effective_radius(r_i: f64, r_j: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `sphere_wall_effective_radius`

Effective (reduced) contact radius `R*` `[m]` of a sphere of radius `r`
against a **flat wall**: the flat is the `r_wall → ∞` limit of
[`effective_radius`], for which `R* = r`.

# Errors

Returns [`DemError::InvalidInput`] if `r` is not strictly positive.

```rust
pub fn sphere_wall_effective_radius(r: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `hertzian_contact_radius`

Hertzian elastic contact radius `a_c` `[m]` for two spheres of radii `r_i`,
`r_j` `[m]` pressed together with mutual approach (overlap) `overlap` `[m]`.

Uses the Hertz relation `δ = a_c²/R*` ⇒ `a_c = sqrt(R*·δ)`, with the
effective radius `R* = r_i·r_j/(r_i+r_j)` from [`effective_radius`] (Hertz
1882; Johnson, *Contact Mechanics*, 1985). Valid for small overlaps
`δ ≪ r_i, r_j` (the Hertzian small-strain assumption); at zero overlap the
contact radius — and hence the conductance — is zero.

A geometric truncation of two overlapping rigid spheres would instead give
`a_c = sqrt(2·R*·δ)`; the elastic value returned here is smaller by a factor
`sqrt(2)` because the surfaces deform. Callers wanting the geometric value
can scale by `sqrt(2)`.

# Errors

Returns [`DemError::InvalidInput`] if either radius is not strictly positive
or if `overlap` is negative (a negative overlap means the spheres are not in
contact, so no contact radius is defined). Zero overlap returns `0.0`.

```rust
pub fn hertzian_contact_radius(r_i: f64, r_j: f64, overlap: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `conductive_heat_rate`

**Attributes:**

- `MustUse { reason: None }`

Conductive heat rate `Q` `[W]` flowing **into** the body at temperature
`t_into` from a body at temperature `t_from`, through a contact of
conductance `h_c` `[W/K]`: `Q = h_c·(t_from − t_into)`.

The sign convention is deliberate: `Q > 0` when `t_from > t_into` (heat
flows into the colder body), `Q = 0` at equal temperatures, and swapping the
two temperatures negates `Q` — so for a particle pair the two half-rates are
equal and opposite and a two-body exchange conserves energy. Temperatures in
`[K]` (any consistent absolute scale works since only their difference
enters).

This one function serves both particle–particle conduction (pass the two
particle temperatures) and particle–wall conduction (pass the particle
temperature as `t_into` and the prescribed wall temperature as `t_from`).

```rust
pub fn conductive_heat_rate(h_c: f64, t_into: f64, t_from: f64) -> f64 { /* ... */ }
```

#### Function `particle_pair_heat_rate`

Conductive heat rate `Q` `[W]` flowing **into particle `i`** from a touching
particle `j`, for the given `model` and contact radius `a_c` `[m]`.

Convenience wrapper over [`ThermalModel::conductance`] +
[`conductive_heat_rate`] that reads the two particles' `temperature` fields:
`Q = h_c·(T_j − T_i)` with `h_c = model.conductance(a_c)`. The equal-and-
opposite rate into `j` is obtained by swapping the arguments (or negating).

# Errors

Propagates [`DemError::InvalidInput`] from [`ThermalModel::conductance`]
(negative contact radius or non-positive conductivity).

```rust
pub fn particle_pair_heat_rate(model: &ThermalModel, contact_radius: f64, particle_i: &crate::particle::Particle, particle_j: &crate::particle::Particle) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `wall_heat_rate`

Conductive heat rate `Q` `[W]` flowing **into a particle** from a boundary
held at the prescribed wall temperature `wall_temperature` `[K]`, for the
given `model` and sphere–wall contact radius `a_c` `[m]`.

Same form as [`particle_pair_heat_rate`], with the wall as an isothermal
reservoir: `Q = h_c·(T_wall − T_particle)`. The wall's finite heat capacity
is not tracked — it is treated as a fixed-temperature source/sink, the usual
Dirichlet thermal boundary condition. Build `model` as a
[`ThermalModel::ContactConduction`] from the particle and wall conductivities
(or a [`ThermalModel::Constant`] wall conductance), and obtain `a_c` from the
boundary geometry (Phase 3) or [`hertzian_contact_radius`] with
[`sphere_wall_effective_radius`].

# Errors

Propagates [`DemError::InvalidInput`] from [`ThermalModel::conductance`].

```rust
pub fn wall_heat_rate(model: &ThermalModel, contact_radius: f64, particle: &crate::particle::Particle, wall_temperature: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `explicit_euler_temperature`

**Attributes:**

- `MustUse { reason: None }`

New temperature `[K]` after one explicit forward-Euler step of the lumped
energy balance `dT/dt = Q_net / (m·c_p)`:
`T(t+dt) = T + Q_net·dt / (m·c_p)`.

- `temperature` — current lumped particle temperature `[K]`.
- `net_heat_rate` — net conductive heat rate `Q_net` `[W]` into the particle
  (the sum `Σ Q` over all its contacts; positive heats it).
- `mass` — particle mass `[kg]` (`> 0`).
- `specific_heat` — specific heat capacity `c_p` `[J/kg/K]` (`> 0`).
- `dt` — time step `[s]` (`> 0`).

This is a **total** function mirroring [`Particle::integrate`]: it does not
return a `Result` and assumes the documented preconditions (`mass > 0`,
`c_p > 0`, `dt > 0`). Being explicit (first-order), it is only stable for a
step below the thermal relaxation limit — for a single contact of
conductance `h_c`, roughly `dt < m·c_p / h_c`; larger steps can overshoot
and oscillate. Use [`apply_temperature_step`] to write the result straight
back into a [`Particle`].

```rust
pub fn explicit_euler_temperature(temperature: f64, net_heat_rate: f64, mass: f64, specific_heat: f64, dt: f64) -> f64 { /* ... */ }
```

#### Function `apply_temperature_step`

Advance a particle's `temperature` field one explicit forward-Euler step
under the net conductive heat rate `net_heat_rate` `[W]`, using the
particle's own `mass` `[kg]` and the supplied specific heat capacity
`specific_heat` `[J/kg/K]` over the step `dt` `[s]`.

In-place counterpart of [`explicit_euler_temperature`]; the temperature
update is `T += Q_net·dt / (m·c_p)`. Specific heat is passed per call rather
than stored on [`Particle`] because it is a material property that Phase 1's
particle model does not carry. Same stability caveat as
[`explicit_euler_temperature`].

```rust
pub fn apply_temperature_step(particle: &mut crate::particle::Particle, net_heat_rate: f64, specific_heat: f64, dt: f64) { /* ... */ }
```

## Module `thermal_radiation`

Phase 4 (extension) — **Radiative & near-field gas-gap heat transfer**
(bead `op-t3l.4` follow-up).

This module adds the two particle-scale heat paths that
[`crate::thermal`] deliberately excluded:

1. **Particle–particle (and particle–wall) thermal radiation** — grey-diffuse
   surface-to-surface exchange, `Q = sigma * eps_eff * A * (T_from^4 - T_into^4)`.
2. **Near-field gas-gap conduction** — conduction through the thin
   interstitial-gas lens between two spheres that are close but **not**
   touching (the dominant path in gas-fluidized and packed beds at small
   separations).

Both are clean-room implementations from public heat-transfer literature
(Incropera & DeWitt; Batchelor & O'Brien 1977; Rong & Horio 1999) — **not**
derived from LIGGGHTS/LAMMPS source (see the crate `NOTICE`). They share the
sign convention and lumped-node picture of [`crate::thermal`]: each particle
is a single isothermal node carrying [`Particle::temperature`] `[K]`, and a
heat rate `Q` `[W]` is **positive when it flows into the colder body**.

# 1. Radiative exchange (grey-diffuse two-surface model)

Two isothermal grey surfaces `i` and `j` exchange radiant energy at a net
rate, into surface `i`,

```text
  Q_ij = sigma * eps_eff * A * (T_j^4 - T_i^4)   [W],
```

where `sigma = 5.670374419e-8 W/m^2/K^4` is the Stefan–Boltzmann constant
([`STEFAN_BOLTZMANN`]), `A` `[m^2]` is the **radiative exchange area** (the
product `A_i * F_ij` of the emitting area and the view factor to the
partner — supplied by the caller from the pair geometry), and `eps_eff` is
the **effective (series) emissivity** of the two grey surfaces. For the
two-surface grey enclosure (the infinite-parallel-plate / small-gap limit
used here) the classical radiation-network result gives

```text
  eps_eff = 1 / (1/eps_i + 1/eps_j - 1),
```

which for equal emissivities `eps_i = eps_j = eps` reduces to
`eps_eff = 1/(2/eps - 1)` and for two black bodies (`eps = 1`) to
`eps_eff = 1`. This is the standard grey-body reciprocity result — see
Incropera & DeWitt, *Fundamentals of Heat and Mass Transfer*, the chapter
on radiation exchange between surfaces (the two-surface enclosure network,
`Q_12 = sigma (T_1^4 - T_2^4) / [ (1-eps_1)/(eps_1 A_1) + 1/(A_1 F_12)
+ (1-eps_2)/(eps_2 A_2) ]`, collapsed to `eps_eff` and a single exchange
area for the equal-area, `F_12 = 1` two-surface pair).

**Grey-diffuse assumption.** Each surface is treated as grey (emissivity
independent of wavelength), diffuse (emission and reflection independent of
direction), opaque, and isothermal over the exchange area. Spectral,
specular, and directional effects are not modelled.

# 2. Near-field gas-gap conduction (Batchelor–O'Brien lens integral)

When two spheres of radii `r_i`, `r_j` sit with a small surface separation
(gap) `g` `[m]`, the interstitial gas forms a thin lens whose local
thickness at radial distance `r` from the line of centres is, to leading
order in the paraboloid approximation of each surface,

```text
  h(r) = g + r^2 / (2 R*),    1/R* = 1/r_i + 1/r_j,
```

with `R* = r_i r_j / (r_i + r_j)` the reduced radius (identical to the
Hertzian effective radius, [`crate::thermal::effective_radius`]). Treating
the gas as conducting one-dimensionally across the gap in parallel annular
rings — each ring of radius `r`, width `dr`, contributing a conductance
`k_g * (2 pi r dr) / h(r)` — and integrating from the axis to an outer lens
radius `r_out` gives, with `k_g` `[W/m/K]` the gas thermal conductivity,

```text
  H_gas = integral_0^{r_out} k_g * 2 pi r / (g + r^2/(2 R*)) dr
        = 2 pi k_g R* * ln( 1 + r_out^2 / (2 R* g) )   [W/K].
```

The gas-gap heat rate into the colder body is then `Q = H_gas * (T_from - T_into)`.
This annular-lens integral is the near-field gas-conduction model of
Batchelor & O'Brien (1977) as applied to particulate/fluidized beds by, e.g.,
Rong & Horio (1999); it is the interstitial-gas counterpart of the
solid-contact constriction conductance in [`crate::thermal`]. The finite
outer radius `r_out` `[m]` is the physical cutoff of the lens (the projected
radius over which neighbouring surfaces are close enough for the lens
approximation to hold, e.g. a fraction of the particle radius); it is a
**required input** rather than an implicit constant, so the caller controls
(and documents) the cutoff for their bed.

# Sign convention

Every heat-rate function returns `Q` `[W]` **into** the body whose
temperature is passed as `t_into`, from the body at `t_from`:
`Q > 0` when `t_from > t_into` (heat flows into the colder body), `Q = 0` at
equal temperatures, and swapping the two bodies negates `Q` — so a two-body
exchange conserves energy exactly (`Q_ij = -Q_ji`) for both the radiative and
the gas-gap path. This matches [`crate::thermal::conductive_heat_rate`].

# Honest scope

- **Radiation is two-body grey exchange only.** There is **no** enclosure
  radiosity solve (no simultaneous multi-surface radiosity/irradiation
  balance), **no** participating/absorbing–emitting medium, and **no**
  spectral or specular treatment. The effective emissivity is the
  two-surface series form; a true `N`-surface enclosure would require solving
  the radiosity network, which this module does not do. The exchange area
  `A = A_i F_ij` is a caller input — this module does not compute view
  factors.
- **Gas conduction is the near-field gap model only.** It captures the
  stationary-gas lens between nearby surfaces. There is **no** forced- or
  natural-convection film, **no** Knudsen/rarefaction (temperature-jump)
  correction at very small gaps or low pressure, and **no** bulk interstitial
  convection — those belong to a later CFD-DEM seam. The lens integral
  diverges as `g -> 0`; that touching limit is the solid-contact regime
  handled by [`crate::thermal`], so this model requires `g > 0`.
- **Verification, not validation.** The tests below check hand-computed
  closed forms (zero heat at equal temperature, the grey-body `sigma eps A
  dT^4` law, energy antisymmetry, the monotonic gas-lens conductance, one
  hand value) — they are **not** a benchmark comparison against a reference
  DEM code or experiment. That validation is the later human step in this
  bead's Definition of Done.

# Unit convention

Following [`crate::particle`] and [`crate::thermal`], the numeric API is
plain `f64` in SI base units, each parameter's unit spelled out in its doc
comment: temperature `T` `[K]`, emissivity `eps` `[-]` (dimensionless, in
`(0, 1]`), exchange/lens area `A` `[m^2]`, gas thermal conductivity `k_g`
`[W/m/K]`, radius `r`, gap `g`, and outer lens radius `r_out` `[m]`,
conductance `H_gas` `[W/K]`, heat rate `Q` `[W]`.

# References (public literature — NOT LAMMPS/LIGGGHTS source)

- F. P. Incropera and D. P. DeWitt, *Fundamentals of Heat and Mass
  Transfer* (Wiley) — Stefan–Boltzmann law, grey-diffuse surface radiation,
  the two-surface enclosure network and effective emissivity
  `eps_eff = 1/(1/eps_1 + 1/eps_2 - 1)`.
- G. K. Batchelor and R. W. O'Brien, "Thermal or electrical conduction
  through a granular material," *Proc. R. Soc. Lond. A* **355**(1682),
  313–333 (1977) — near-field gas-gap (fluid-lens) conduction between close
  surfaces.
- Y. Rong and M. Horio, "DEM simulation of char combustion in a fluidized
  bed," in *Second International Conference on CFD in the Minerals and
  Process Industries* (CSIRO, 1999) — gas-lens conductance integral applied
  to DEM fluidized-bed heat transfer.

```rust
pub mod thermal_radiation { /* ... */ }
```

### Types

#### Enum `RadiationModel`

A grey-diffuse radiative-exchange model for a pair of surfaces.

Enum dispatch (no trait objects), per the workspace design rules: the set of
emissivity laws is closed and known at compile time, so adding a variant
forces every `match` to handle it. The single method
[`RadiationModel::effective_emissivity`] returns the dimensionless effective
(series) emissivity `eps_eff` `[-]` of the two grey surfaces, which the
heat-rate functions combine with the Stefan–Boltzmann law and the exchange
area.

The same model type serves both particle–particle and particle–wall
exchange: for a wall, build a [`RadiationModel::GreyPair`] from the
particle's and the wall's emissivities (or a [`RadiationModel::GreyBody`] if
they are equal) and pass the sphere–wall exchange area.

```rust
pub enum RadiationModel {
    GreyBody {
        emissivity: f64,
    },
    GreyPair {
        emissivity_i: f64,
        emissivity_j: f64,
    },
}
```

##### Variants

###### `GreyBody`

Two grey surfaces of **equal** emissivity `emissivity` `[-]`, in `(0, 1]`.
The effective emissivity is `eps_eff = 1/(2/eps - 1)`; `emissivity = 1`
(black bodies) gives `eps_eff = 1`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `emissivity` | `f64` | Common grey-diffuse emissivity of both surfaces `[-]`, in `(0, 1]`. |

###### `GreyPair`

Two grey surfaces of **different** emissivities `emissivity_i`,
`emissivity_j` `[-]`, each in `(0, 1]`. The effective emissivity is the
two-surface series form `eps_eff = 1/(1/eps_i + 1/eps_j - 1)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `emissivity_i` | `f64` | Grey-diffuse emissivity of body `i` `[-]`, in `(0, 1]`. |
| `emissivity_j` | `f64` | Grey-diffuse emissivity of body `j` `[-]`, in `(0, 1]`. |

##### Implementations

###### Methods

- ```rust
  pub fn effective_emissivity(self: &Self) -> Result<f64, DemError> { /* ... */ }
  ```
  Effective (series) emissivity `eps_eff` `[-]` of the two grey surfaces:

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RadiationModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RadiationModel) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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

#### Function `radiative_heat_rate`

**Attributes:**

- `MustUse { reason: None }`

Net radiative heat rate `Q` `[W]` flowing **into** the surface at
temperature `t_into` from a surface at temperature `t_from`, for a
pre-computed effective emissivity `eps_eff` `[-]` and exchange area `area`
`[m^2]`: `Q = sigma * eps_eff * area * (t_from^4 - t_into^4)`.

Pure function (mirrors [`crate::thermal::conductive_heat_rate`]): the caller
supplies `eps_eff` (from [`RadiationModel::effective_emissivity`]) and the
exchange area `A = A_i F_ij`. `Q > 0` when `t_from > t_into` (heat into the
colder body); swapping the temperatures negates `Q`. Temperatures are
**absolute** `[K]` because the fourth-power law is nonlinear — a relative
scale would give the wrong magnitude.

```rust
pub fn radiative_heat_rate(eps_eff: f64, area: f64, t_into: f64, t_from: f64) -> f64 { /* ... */ }
```

#### Function `net_radiative_heat_rate`

Net radiative heat rate `Q` `[W]` flowing **into particle `i`** from another
particle `j`, for the given `model` and radiative exchange area
`exchange_area` `[m^2]` (`= A_i F_ij`).

Convenience wrapper over [`RadiationModel::effective_emissivity`] +
[`radiative_heat_rate`] that reads the two particles' `temperature` fields:
`Q = sigma eps_eff A (T_j^4 - T_i^4)`. The equal-and-opposite rate into `j`
is obtained by swapping the two particle arguments (or negating).

# Errors

Returns [`DemError::InvalidInput`] if `exchange_area` is negative, or if the
model's emissivity is outside `(0, 1]` (surfaced via
[`RadiationModel::effective_emissivity`]).

```rust
pub fn net_radiative_heat_rate(model: &RadiationModel, exchange_area: f64, particle_i: &crate::particle::Particle, particle_j: &crate::particle::Particle) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `radiative_wall_heat_rate`

Net radiative heat rate `Q` `[W]` flowing **into a particle** from a wall
held at the prescribed temperature `wall_temperature` `[K]`, for the given
`model` and exchange area `exchange_area` `[m^2]`.

Same form as [`net_radiative_heat_rate`], with the wall as an isothermal
grey surface: `Q = sigma eps_eff A (T_wall^4 - T_particle^4)`. The wall's
finite heat capacity is not tracked (a fixed-temperature Dirichlet
radiative source/sink). Build `model` as a [`RadiationModel::GreyPair`] from
the particle and wall emissivities.

# Errors

Returns [`DemError::InvalidInput`] if `exchange_area` is negative or the
model's emissivity is outside `(0, 1]`.

```rust
pub fn radiative_wall_heat_rate(model: &RadiationModel, exchange_area: f64, particle: &crate::particle::Particle, wall_temperature: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `gas_gap_conductance`

Near-field gas-gap conductance `H_gas` `[W/K]` between two spheres of radii
`r_i`, `r_j` `[m]` separated by a surface gap `gap` `[m]`, through a gas of
thermal conductivity `k_g` `[W/m/K]`, integrated to an outer lens radius
`outer_radius` `[m]`.

`H_gas = 2 pi k_g R* ln(1 + outer_radius^2 / (2 R* gap))` with the reduced
radius `R* = r_i r_j / (r_i + r_j)` ([`crate::thermal::effective_radius`]).
This is the Batchelor–O'Brien (1977) gas-lens integral (Rong & Horio 1999).
The conductance **decreases monotonically as `gap` grows** (the gas layer
thickens) and vanishes in the limit `gap -> infinity`.

# Errors

Returns [`DemError::InvalidInput`] if any of `k_g`, `r_i`, `r_j`, `gap`,
`outer_radius` is not strictly positive (a non-positive gap is the contact
limit, handled by [`crate::thermal`], not by this near-field model).

```rust
pub fn gas_gap_conductance(k_g: f64, r_i: f64, r_j: f64, gap: f64, outer_radius: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `gas_wall_gap_conductance`

Near-field gas-gap conductance `H_gas` `[W/K]` between a sphere of radius
`r` `[m]` and a **flat wall**, separated by a surface gap `gap` `[m]`.

The flat wall is the `r_wall -> infinity` limit of [`gas_gap_conductance`],
for which the reduced radius `R* -> r`
([`crate::thermal::sphere_wall_effective_radius`]); only the sphere curves,
so the lens thickness is `h(r) = gap + r^2/(2 r)`. Same integral, with
`R* = r`.

# Errors

Returns [`DemError::InvalidInput`] if any of `k_g`, `r`, `gap`,
`outer_radius` is not strictly positive.

```rust
pub fn gas_wall_gap_conductance(k_g: f64, r: f64, gap: f64, outer_radius: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `gas_gap_heat_rate`

Near-field gas-gap heat rate `Q` `[W]` flowing **into particle `i`** from a
nearby (non-touching) particle `j`, through the interstitial gas.

Combines [`gas_gap_conductance`] (from the two particles' radii, the `gap`
`[m]`, gas conductivity `k_g` `[W/m/K]`, and outer lens radius
`outer_radius` `[m]`) with the two particles' `temperature` fields:
`Q = H_gas (T_j - T_i)`. `Q > 0` when `j` is hotter (heat into the colder
particle `i`); the equal-and-opposite rate into `j` is obtained by swapping
the particle arguments.

# Errors

Propagates [`DemError::InvalidInput`] from [`gas_gap_conductance`]
(non-positive `k_g`, radius, `gap`, or `outer_radius`).

```rust
pub fn gas_gap_heat_rate(k_g: f64, gap: f64, outer_radius: f64, particle_i: &crate::particle::Particle, particle_j: &crate::particle::Particle) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `gas_gap_wall_heat_rate`

Near-field gas-gap heat rate `Q` `[W]` flowing **into a particle** from a
flat wall at the prescribed temperature `wall_temperature` `[K]`, through the
interstitial gas lens.

Combines [`gas_wall_gap_conductance`] (from the particle radius, `gap` `[m]`,
gas conductivity `k_g` `[W/m/K]`, and outer lens radius `outer_radius` `[m]`)
with the particle temperature and the prescribed wall temperature:
`Q = H_gas (T_wall - T_particle)`. The wall is an isothermal reservoir (its
heat capacity is not tracked).

# Errors

Propagates [`DemError::InvalidInput`] from [`gas_wall_gap_conductance`].

```rust
pub fn gas_gap_wall_heat_rate(k_g: f64, gap: f64, outer_radius: f64, particle: &crate::particle::Particle, wall_temperature: f64) -> Result<f64, crate::DemError> { /* ... */ }
```

### Constants and Statics

#### Constant `STEFAN_BOLTZMANN`

Stefan–Boltzmann constant `sigma` `[W/m^2/K^4]`.

The 2019-SI exact value `5.670374419e-8 W·m^-2·K^-4`, i.e.
`sigma = 2 pi^5 k_B^4 / (15 h^3 c^2)`. Multiplies the difference of the
fourth powers of absolute temperatures in the grey-body radiative law.

```rust
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
```

## Module `timestep`

# Granular time-step criteria (`fix check/timestep/gran`)

Translation of LIGGGHTS' Rayleigh-wave and Hertz-contact time estimates —
the two limits that decide whether a DEM step is small enough.

**This is not a nicety for a pebble bed.** An explicit DEM step that
over-runs the contact duration does not merely lose accuracy, it goes
unstable and ejects particles; picking `dt` by eye is how a bed "explodes".
Upstream warns above ~20 % of the Rayleigh time, and this module reports the
same fractions so the two codes can be compared directly.

## The two criteria

**Rayleigh time** — the period of a Rayleigh surface wave crossing a
particle, the limit on how fast a contact signal can traverse it:

```text
  t_R = π·r·√(ρ/G) / (0.1631·ν + 0.8766)
```

with shear modulus `G = E / (2(1 + ν))`. (Upstream's own expression, from
the Thornton/Randall form quoted in the LIGGGHTS documentation.)

**Hertz time** — the duration of a Hertzian collision at the maximum
relative approach speed in the system:

```text
  t_H = 2.87·(m_eff² / (r_eff · E_eff² · v_rel,max))^{1/5}
```

Upstream evaluates this with the deliberately conservative choices
`m_eff = (4/3)π r³ ρ` (the *full* particle mass, not the reduced mass) and
`r_eff = r/2`, testing "collision of a particle with itself"; both are
reproduced here so the numbers match.

## Honest scope

Single material, monodisperse or polydisperse spheres, no moving meshes
(upstream folds mesh node speeds into `v_rel,max`; here `v_rel,max = 2·v_max`
over the particles, upstream's particle–particle branch). The estimates are
upstream's *heuristics*, not theorems — they are reproduced faithfully,
including their approximations.

```rust
pub mod timestep { /* ... */ }
```

### Types

#### Struct `TimestepEstimate`

The two limiting times for a granular ensemble, plus the step fractions.

# Fields and units

| Field | Symbol | Quantity | SI unit |
|---|---|---|---|
| `rayleigh_time` | `t_R` | minimum Rayleigh time over the ensemble | `[s]` |
| `hertz_time` | `t_H` | minimum Hertz collision time | `[s]` |
| `v_rel_max` | `v_rel,max` | maximum relative approach speed | `[m/s]` |
| `r_min` | `r_min` | smallest particle radius | `[m]` |

```rust
pub struct TimestepEstimate {
    pub rayleigh_time: f64,
    pub hertz_time: f64,
    pub v_rel_max: f64,
    pub r_min: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rayleigh_time` | `f64` | Minimum Rayleigh time over the ensemble `[s]`. |
| `hertz_time` | `f64` | Minimum Hertz collision time `[s]`; `f64::INFINITY` when nothing moves. |
| `v_rel_max` | `f64` | Maximum relative approach speed `2·v_max` `[m/s]`. |
| `r_min` | `f64` | Smallest particle radius `[m]`. |

##### Implementations

###### Methods

- ```rust
  pub fn rayleigh_fraction(self: &Self, dt: f64) -> f64 { /* ... */ }
  ```
  Fraction of the Rayleigh time a step of `dt` `[s]` uses `[-]`.

- ```rust
  pub fn hertz_fraction(self: &Self, dt: f64) -> f64 { /* ... */ }
  ```
  Fraction of the Hertz collision time a step of `dt` `[s]` uses `[-]`.

- ```rust
  pub fn is_stable(self: &Self, dt: f64) -> bool { /* ... */ }
  ```
  Whether `dt` `[s]` satisfies **both** of upstream's warning thresholds.

- ```rust
  pub fn recommended_dt(self: &Self) -> f64 { /* ... */ }
  ```
  The largest step `[s]` satisfying both thresholds.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TimestepEstimate { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &TimestepEstimate) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
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

#### Function `rayleigh_time`

Rayleigh time `[s]` for a single sphere of radius `r` `[m]` and density `ρ`
`[kg/m³]` in `material`.

`t_R = π·r·√(ρ/G) / (0.1631·ν + 0.8766)`, with `G = E/(2(1+ν))`.

# Errors

[`DemError::InvalidInput`] if `r` or `density` is not finite and strictly
positive.

```rust
pub fn rayleigh_time(radius: f64, density: f64, material: &crate::granular::GranularMaterial) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `hertz_time`

Hertz collision time `[s]` at relative approach speed `v_rel` `[m/s]`.

Upstream's conservative form: `m_eff = (4/3)π r³ ρ`, `r_eff = r/2`,
`E_eff = Y_eff`, `t_H = 2.87·(m_eff²/(r_eff·E_eff²·v_rel))^{1/5}`.

Returns `f64::INFINITY` for `v_rel == 0` (no collision to resolve), matching
upstream's guard.

# Errors

[`DemError::InvalidInput`] for a non-positive radius or density, or a
negative `v_rel`.

```rust
pub fn hertz_time(radius: f64, density: f64, v_rel: f64, material: &crate::granular::GranularMaterial) -> Result<f64, crate::DemError> { /* ... */ }
```

#### Function `estimate`

Both criteria for a whole ensemble, as `fix check/timestep/gran` computes
them.

`density` `[kg/m³]` is the shared material density. `v_rel,max` is taken as
`2·max_i|v_i|` (upstream's particle–particle branch; a moving mesh would
raise it).

# Errors

[`DemError::InvalidInput`] if `particles` is empty or `density` is invalid.

```rust
pub fn estimate(particles: &[crate::particle::Particle], density: f64, material: &crate::granular::GranularMaterial) -> Result<TimestepEstimate, crate::DemError> { /* ... */ }
```

### Constants and Statics

#### Constant `RAYLEIGH_WARN_FRACTION`

Upstream's recommended maximum fraction of the Rayleigh time `[-]`.

`fix check/timestep/gran` warns when `dt` exceeds this fraction; 20 % is the
value quoted in the LIGGGHTS documentation for the Rayleigh criterion.

```rust
pub const RAYLEIGH_WARN_FRACTION: f64 = 0.20;
```

#### Constant `HERTZ_WARN_FRACTION`

Upstream's recommended maximum fraction of the Hertz collision time `[-]`.

`fix check/timestep/gran` warns when `dt` exceeds this fraction of the
estimated contact duration.

```rust
pub const HERTZ_WARN_FRACTION: f64 = 0.10;
```

## Types

### Enum `DemError`

Errors produced by the DEM library in this crate.

```rust
pub enum DemError {
    InvalidInput(String),
    NotImplemented(String),
}
```

#### Variants

##### `InvalidInput`

A model input was outside its valid physical range (e.g. non-positive mass/radius).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `NotImplemented`

A requested feature is scaffolded but not yet implemented.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

#### Implementations

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
