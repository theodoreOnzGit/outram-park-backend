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

## Extensions (clean-room, unit-tested)

- [`simulation`] — multi-particle DEM engine: linked-cell neighbor search +
  velocity-Verlet ensemble stepping composing [`contact`] + [`boundary`].
- [`rolling`] — rolling resistance (Ai et al.) + cohesion (JKR / linear).
- [`mesh_wall`] — triangulated (STL-style) walls + moving/rotating boundaries.
- [`thermal_radiation`] — grey-body radiation + near-field gas-gap conduction.

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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &BondForce) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Bond) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &BondModel) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Contact) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Boundary) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &ContactForce) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &HookeContact) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &HertzContact) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &ContactModel) -> bool { /* ... */ }
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
    fn eq(self: &Self, other: &CouplingScheme) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &LocalFluidState) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &CouplingExchange) -> bool { /* ... */ }
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

- **Eq**
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReservedFluidSource) -> bool { /* ... */ }
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
    fn eq(self: &Self, other: &ReservedDragModel) -> bool { /* ... */ }
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
    fn eq(self: &Self, other: &ReservedCoupling) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Triangle) -> bool { /* ... */ }
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
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `triangles` | `Vec<Triangle>` | The triangular facets `[m]`. Assumed consistently wound (outward normals<br>point into the domain) and to form a sensible surface; see the module<br>"Honest scope" for what is *not* checked. |

##### Implementations

###### Methods

- ```rust
  pub fn new(triangles: Vec<Triangle>) -> Result<Self, DemError> { /* ... */ }
  ```
  Construct a mesh wall from a non-empty list of facets.

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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &MeshWall) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &WallGeometry) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &MovingBoundary) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Vec3) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Particle) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &RollingTorque) -> bool { /* ... */ }
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

**Directional constant-torque** rolling resistance (Ai et al. 2011
"Model A"; Iwashita & Oda 1998).

The resisting torque has a fixed magnitude set by the normal load and
always points opposite the relative rolling direction:

`M_r = −μ_r · R* · |F_n| · ω̂_rel`,  with  `ω̂_rel = ω_rel / ‖ω_rel‖`.

When `‖ω_rel‖` is below [`ROLLING_OMEGA_EPS`] the direction is undefined
and the torque is zero (a non-rolling pair feels no rolling resistance).
This is the *directional* form: the magnitude does not depend on the
rolling *speed*, only its direction — hence "constant torque".

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mu_r` | `f64` | Rolling-friction coefficient `μ_r` `[-]`. Non-negative. |

###### `ViscousRolling`

**Viscous** rolling resistance (Ai et al. 2011, viscous family).

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
  pub fn rolling_torque(self: &Self, normal_force: f64, r_eff: f64, omega_rel: Vec3) -> RollingTorque { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &RollingModel) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &CohesionModel) -> bool { /* ... */ }
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
  Advance the whole system by one velocity-Verlet step of size `dt` `[s]`.

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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &DemSimulation) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &ThermalModel) -> bool { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &RadiationModel) -> bool { /* ... */ }
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
