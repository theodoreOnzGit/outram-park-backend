# Crate Documentation

**Version:** 0.0.1

**Format Version:** 61

# Module `outram_blender`

# outram-blender

A pure-Rust, headless **mesh-authoring frontend** for the OUTRAM PARK
multiphysics suite, inspired by the **architecture** of
[Blender](https://github.com/blender/blender) (GPLv2-or-later, which is
GPLv3-compatible). It authors and procedurally generates geometry, then
bridges it into two OUTRAM PARK solver workflows:

- **Monte Carlo neutron transport** (feature `mc-export`). Author a surface,
  fit it to an `outram-mc-libs` CSG universe ([`export`]), attach materials,
  and run a k-eigenvalue (criticality) calculation returning `k_eff ± σ`
  (the `sim` module). This path is driven by the **MC Studio** egui app
  (`examples/mc_studio`).
- **CFD / thermal-hydraulics volume meshing** (feature `foam-mesh`). Hand a
  closed surface to `outram-park-fork-cfmesh`'s tet→dual→boundary-layers
  pipeline and write out an OpenFOAM `polyMesh` (the `foam_mesh` module). This
  path is driven by the **Mesh Studio** egui app (`examples/mesh_studio`).

The base authoring library (primitives, mesh operators, modifiers, procedural
evaluator, geometry processing) pulls in neither solver — both bridges are
opt-in cargo features, so the default build stays light and Android-buildable.

> **⚠️ Not a Blender port.** Blender is millions of lines of C/C++/Python;
> this crate borrows its *concepts and data-structure architecture* (the
> BMesh half-edge topology, the mesh-operator model, the modifier stack,
> geometry-nodes-style procedural generation) — it does **not** port
> Blender's code (the only literally-ported piece is the Shewchuk robust
> predicates in [`boolean_predicates`], with its GPL provenance header). The
> algorithms here — primitives, mesh operators, subdivision (Catmull-Clark &
> Loop), the general CSG boolean, the sparse-solve geometry processing
> (Laplacian/Taubin smoothing, harmonic parameterization, ARAP deformation),
> QEM decimation, the modifier stack, the procedural evaluator, and the
> export bridges — are written from first principles and unit-tested against
> analytic references. See the module map for per-module status.
>
> **Not affiliated with the Blender Foundation.** "Blender" names the
> upstream project whose architecture inspired this work; nothing here is
> endorsed by or sanctioned by the Blender Foundation. See the README's
> "Naming & trademark" section — the crate name itself is a pending
> maintainer decision.
>
> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

## Module map — what belongs where

| Module | Blender analogue | Status |
|---|---|---|
| [`math`] | `blenlib` `BLI_math` vector types | **real** — a minimal pure-Rust [`math::Vec3`] |
| [`transform`] | `Object.matrix_world` affine placement | **real** — [`transform::Affine3`] per-vertex transform (CPU reference for the GPU kernel) |
| [`mesh`] | `bmesh` (`BMVert`/`BMEdge`/`BMLoop`/`BMFace`) | **real** — index-based half-edge topology |
| [`primitives`] | `editors/mesh/editmesh_add` primitive add-ops | **real** — cube / UV-sphere / cylinder / grid generators (unit-tested) |
| [`revolve`] | Spin (`bmo_spin`) | **real** — sweep a profile polyline around an axis into a surface of revolution (pipes / vessels / cones) |
| [`ops`] | `bmesh/operators/*` (`bmo_*`) mesh operators | **real** — extrude / midpoint-subdivide / vertex-bevel (flat or rounded multi-segment; boolean delegates to [`boolean`]) |
| [`subdivision`] | OpenSubdiv / `MOD_subsurf` | **real** — Catmull-Clark surface subdivision (local stencils) |
| [`loop_subdivision`] | `MOD_subsurf` (triangle path) | **real** — Loop subdivision surface for triangle meshes |
| [`laplacian`] | `MOD_laplaciansmooth` / `bmo_smooth_laplacian` | **real** — cotangent/uniform discrete Laplacian + implicit & Taubin smoothing (first `faer` sparse solve) |
| [`parameterize`] | UV unwrap (harmonic map) | **real** — Tutte/harmonic planar parameterization of a disk (reuses the Laplacian sparse solve) |
| [`arap`] | "As Rigid As Possible" deform | **real** — ARAP handle-based deformation (local rotation fit + cotangent-Laplacian global solve) |
| [`decimate`] | `MOD_decimate` (Collapse) | **real** — QEM (Garland–Heckbert) edge-collapse mesh simplification |
| [`convex_hull`] | `bmo_convex_hull` | **real** — 3D convex hull of a point set (incremental, robust `orient3d`) |
| [`weld`] | `bmo_remove_doubles` / Merge by Distance | **real** — merge coincident vertices within a tolerance (grid hash + union-find) |
| [`fill_holes`] | `bmo_holes_fill` / Fill Holes | **real** — cap open boundary loops with a centroid fan (watertight) |
| [`solidify`] | `MOD_solidify` (simple) | **real** — extrude a surface into a closed shell (inner offset + rim) |
| [`recalc_normals`] | `normals_make_consistent` (Recalculate Outside) | **real** — repair inconsistent winding (BFS) + flip each component outward |
| [`triangulate`] | `bmo_triangulate` (fan) | **real** — fan-triangulate every face into a triangle-only mesh |
| [`inset`] | `bmo_inset` (Individual) | **real** — per-face inset: shrunk inner copy + bridging ring quads |
| [`bisect`] | Bisect (plane cut) | **real** — half-space clip every face by a plane (Sutherland–Hodgman); leaves the cut open |
| [`edge_bevel`] | Bevel (edges) | **real** — chamfer every edge (cut faces back + fill edge/corner gaps); winding fixed by `recalc_normals` |
| [`boolean`] | `bmo_boolean` (Manifold upstream) | **real** — CSG entry point: exact convex-`Intersect` fast path, else delegates to [`boolean_general`] |
| [`boolean_general`] | `mesh_boolean.cc` / `mesh_intersect.cc` arrangement | **real** — general union/difference/intersect on non-convex closed meshes (arrangement + winding classification) |
| [`boolean_predicates`] | `blenlib` `math_boolean.cc` (Shewchuk) | **real** — robust `orient2d/3d`, `incircle`, `insphere` (adaptive f64 + double-double fallback) |
| [`boolean_classify`] | `mesh_boolean.cc` inside/outside classification | **real** — point-in-closed-mesh via generalized winding number (+ ray-parity cross-check) |
| [`modifiers`] | `modifiers/intern/MOD_*` modifier stack | **real** — subsurf / mirror / array |
| [`procedural`] | Geometry Nodes (`nodes/geometry/*`) | **real** — node-graph evaluator |
| [`export`] | I/O exporters (`io/*`) | **real** — OpenFOAM polyMesh text + CSG fitting (box/sphere/cylinder/convex-faceted) + DAGMC faceted-solid + feature-gated real-type bridges (`foam-export`, `mc-export`) |
| [`stl`] | STL I/O | **real** — ASCII + binary STL read/write (surface-mesh interchange / DAGMC / Monte-Carlo feed) |
| `sim` *(feature `mc-export`)* | — (no Blender analogue) | **real** — Monte Carlo setup + run: build materials, bundle geometry/source/settings, run a k-eigenvalue criticality calc (`k_eff ± σ`) via `outram-mc-libs`. Backend of **MC Studio** |
| `foam_mesh` *(feature `foam-mesh`)* | — (no Blender analogue) | **real** — volume-meshing bridge: blender surface → `outram-park-fork-cfmesh` tet→dual→boundary-layers pipeline → OpenFOAM `polyMesh`. Backend of **Mesh Studio** |

## Design rules honoured here (workspace `CLAUDE.md`)

- **Index-based topology, no lifetimes/pointers.** Every element is
  addressed by a newtype index ([`mesh::VertexId`], [`mesh::EdgeId`],
  [`mesh::LoopId`], [`mesh::FaceId`]) into a `Vec`, exactly as the workspace
  forbids `&'a`-linked graph nodes.
- **Enums for dispatch, never trait objects.** The operator, modifier, and
  procedural-node sets are closed and enumerated ([`ops::MeshOp`],
  [`modifiers::Modifier`], [`procedural::GeometryNode`]).
- **No `Box<T>`; `Arc<T>` for sharing.** Owned meshes are passed by value;
  shared read-only meshes use `std::sync::Arc`.

## Where to start reading

[`primitives`] is the primary entry point. Read [`primitives::cube`]
top-to-bottom, then the [`mesh::Mesh`] type it builds on; from there the
[`ops::MeshOp`] enum is the map of what you can *do* to a mesh (extrude,
bevel, boolean, smooth, decimate, subdivide, ARAP-deform).

```
use outram_blender::primitives;

// A unit cube centred at the origin: 8 vertices, 12 edges, 6 quad faces.
let cube = primitives::cube(1.0);
assert_eq!(cube.vertex_count(), 8);
assert_eq!(cube.edge_count(), 12);
assert_eq!(cube.face_count(), 6);
// Euler characteristic of a closed genus-0 surface: V - E + F = 2.
assert_eq!(cube.euler_characteristic(), 2);
```

## Modules

## Module `arap`

**As-Rigid-As-Possible (ARAP) surface deformation** (Sorkine & Alexa, 2007).

Blender analogue: the Laplacian-deform / "As Rigid As Possible" mesh tools.
This is the crate's most involved sparse-solve operator: it deforms a mesh to
match prescribed **handle** positions while keeping every local
neighbourhood **as rigid as possible** (bending and rotating, resisting
stretching/shearing). It reuses the cotangent-Laplacian machinery from
[`crate::laplacian`] — the global system matrix is exactly that Laplacian.

## Algorithm (local / global alternation)

Minimize `E = Σ_i Σ_{j∈N(i)} w_ij ‖(p'_i − p'_j) − R_i (p_i − p_j)‖²` over the
deformed positions `p'` and per-vertex rotations `R_i ∈ SO(3)`, where `p` is
the rest pose and `w_ij = (cot α + cot β)/2` are the cotangent weights.

- **Local step** (fix `p'`, solve each `R_i`): form the `3×3` covariance
  `S_i = Σ_j w_ij (p'_i − p'_j)(p_i − p_j)ᵀ` (deformed ⊗ rest) and take the
  closest rotation `R_i = U Vᵀ` (the orthogonal Procrustes solution, with a
  determinant sign-fix) from the SVD `S_i = U Σ Vᵀ` ([`closest_rotation`]) —
  the rotation that best maps the rest one-ring onto the deformed one.
- **Global step** (fix `{R_i}`, solve `p'`): solve `L p' = b` with
  `b_i = Σ_j (w_ij/2)(R_i + R_j)(p_i − p_j)`, where `L` is the cotangent
  Laplacian. Handle vertices are constrained (pinned to their targets), which
  grounds the otherwise-singular `L` — the exact same boundary-pinned SPD
  reduced system as [`crate::laplacian::laplacian_smooth`].

Because `L` and the free/fixed partition are fixed across iterations, the
sparse matrix is **factorized once** (`faer` sparse Cholesky) and only the
right-hand side `b` is rebuilt each iteration. The energy `E` is non-increasing
(each step is an exact partial minimizer); [`arap_energy`] exposes it for
convergence checks.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod arap { /* ... */ }
```

### Types

#### Enum `ArapError`

Errors from [`arap_deform`].

```rust
pub enum ArapError {
    NoConstraints,
    Assembly,
    NotPositiveDefinite,
}
```

##### Variants

###### `NoConstraints`

No handle/anchor constraints were given. Without at least one fixed
vertex per connected component the cotangent Laplacian is singular (the
constant/translation null space), so the solve is undefined.

###### `Assembly`

The sparse system could not be assembled (bad index).

###### `NotPositiveDefinite`

The reduced Laplacian is not positive definite — a component with no
constrained vertex, or a very obtuse (non-Delaunay) mesh whose cotangent
weights broke SPD.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn from(source: crate::arap::ArapError) -> Self { /* ... */ }
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

#### Function `arap_deform`

Deform `mesh` (its positions are the rest pose) so the given `handles`
(vertex → target position) are met while every one-ring stays as rigid as
possible, via `iterations` local/global ARAP steps.

Handle vertices are pinned to their targets; all other vertices are solved
for. Topology is preserved (only positions change). More `iterations` = tighter
convergence; 3–10 is typically plenty. Returns the deformed mesh.

# Errors

[`ArapError::NoConstraints`] if `handles` is empty;
[`ArapError::NotPositiveDefinite`] / [`ArapError::Assembly`] on a solve/setup
failure (see those variants).

```rust
pub fn arap_deform(mesh: &crate::mesh::Mesh, handles: &[(crate::mesh::VertexId, crate::math::Vec3)], iterations: u32) -> Result<crate::mesh::Mesh, ArapError> { /* ... */ }
```

#### Function `arap_energy`

The ARAP energy `E = Σ_i Σ_{j∈N(i)} w_ij ‖(d_i − d_j) − R_i (p_i − p_j)‖²` of a
deformed configuration `deformed` relative to `rest` (`R_i` the per-vertex
optimal rotation). Non-negative; non-increasing across [`arap_deform`]
iterations — a convergence diagnostic (see the module tests).

`deformed` must have one position per vertex, in [`crate::mesh::VertexId`]
order (e.g. `arap_deform(...)?.positions()`).

```rust
pub fn arap_energy(rest: &crate::mesh::Mesh, deformed: &[crate::math::Vec3]) -> f64 { /* ... */ }
```

## Module `bisect`

Bisect — cut a mesh by a plane and keep one half.

This is the pure-Rust analogue of Blender's **Bisect**: a plane (a point and
a normal) slices the mesh, and the half on the normal-negative side —
`n · (x − point) <= 0` — is kept. Every face is clipped against that
half-space with the Sutherland–Hodgman algorithm, so faces straddling the
plane are cut cleanly and faces fully on the discarded side vanish.

The cut is left **open**: bisect does not cap the exposed section. That is
deliberate — it composes with [`crate::fill_holes`], which caps the planar
boundary loop into a watertight solid. The pair `bisect` then `fill_holes`
is the half-space-cut primitive the CSG / Monte-Carlo workflow builds on.

# Shared crossing vertices

Where the plane crosses an edge, one new vertex is created and **shared** by
both faces on that edge — the crossing point is keyed by the undirected
original edge, so the result stays manifold along the cut rather than
splitting into a crack. No `faer`, no external dependency; Android-safe.

```rust
pub mod bisect { /* ... */ }
```

### Functions

#### Function `bisect`

Cut `mesh` by the plane through `point` with the given `normal`, keeping the
half where `normal · (x − point) <= 0`.

`normal` need not be unit length (only its sign and direction matter).
Faces straddling the plane are clipped; the exposed cut is left open (cap it
with [`crate::fill_holes::fill_holes`] for a closed solid). If the whole
mesh is on the kept side the mesh is returned unchanged; if none of it is,
an empty mesh is returned. This is infallible.

# Examples

```
use outram_blender::{primitives, bisect::bisect, fill_holes::fill_holes, math::Vec3};

// Slice a cube [-1,1]³ at z = 0, keeping the lower half, then cap it:
// the result is a closed box of half the volume.
let cube = primitives::cube(2.0);
let lower = bisect(&cube, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0));
let solid = fill_holes(&lower);
assert_eq!(solid.euler_characteristic(), 2);
```

```rust
pub fn bisect(mesh: &crate::mesh::Mesh, point: crate::math::Vec3, normal: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

## Module `boolean`

Mesh boolean / CSG operator — **entry point + exact convex fast path**.

Blender analogue: `bmesh/tools/bmesh_boolean` / the `bmo_boolean` operator,
which upstream is backed by the **Manifold** library. [`boolean`] is the
single public entry point for all three CSG modes; it dispatches between two
implementations:

- **Exact convex fast path (this module).** When both operands are convex
  and the mode is [`crate::ops::BooleanMode::Intersect`], the intersection is
  computed here by **half-space clipping** — an exact, non-triangulated
  result (a clean n-gon mesh).
- **General arrangement path** ([`crate::boolean_general`]). Union,
  Difference, and any **non-convex** operand are handled there, by cutting
  the two surfaces against each other and classifying the resulting patches
  with the generalized winding number. See that module for its contract and
  limitations (coplanar-overlap rejection, generic-position assumption,
  triangulated output).

## The convex fast path, in detail

Every face of operand `B` defines an outward half-space

- plane through [`Mesh::face_centroid`] `c` with outward unit normal
  [`Mesh::face_normal`] `n`;
- the solid interior of `B` is the set of points `p` with
  `dot(n, p - c) <= 0` for **every** face of `B`.

Operand `A`'s convex polytope is clipped against each of `B`'s face
half-spaces in turn (successive 3-D Sutherland–Hodgman polygon clipping):
for each clip plane every face polygon of the running result is clipped to
the inside half-space, the segments where faces cross the plane are collected,
and a fresh **cap** face is built from that loop of cut points (ordered by
angle about the plane normal, wound so its outward normal matches the clip
plane). The result is the convex intersection `A ∩ B` — a valid closed convex
mesh — for which Euler's identity `V - E + F = 2` holds.

A **non-overlapping / empty** convex intersection returns
[`BooleanError::Unsupported`] rather than an empty mesh, so a caller cannot
mistake "no overlap" for a valid degenerate solid. (When the operands are
*not* both convex, `Intersect` never reaches this path — it goes to the
general pipeline, which handles the empty/disjoint case there.)

> **Untrusted AI-generated draft** until a human reviews it, per the workspace
> `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions.

```rust
pub mod boolean { /* ... */ }
```

### Types

#### Enum `BooleanError`

Errors from a mesh boolean operation.

The only variant is [`BooleanError::Unsupported`], carrying a short static
reason. It is returned for the inputs neither the convex fast path nor the
general arrangement pipeline can resolve (see the module docs and
[`crate::boolean_general`]) — an honest signal, never a silently wrong mesh.

```rust
pub enum BooleanError {
    Unsupported(&'static str),
}
```

##### Variants

###### `Unsupported`

The boolean could not be resolved: an empty/non-overlapping intersection,
a **coplanar overlapping face** between the operands, an
exactly-degenerate arrangement, or a result that welds to fewer than four
faces. The `&'static str` names which.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn from(source: crate::boolean::BooleanError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: crate::boolean::BooleanError) -> Self { /* ... */ }
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

#### Function `boolean`

Compute the boolean of two closed meshes under `mode`.

Single entry point for all three CSG modes, dispatching between the exact
convex fast path (this module) and the general arrangement pipeline
([`crate::boolean_general`]) — see the module-level docs. In summary:

- [`BooleanMode::Intersect`] on two **convex** closed meshes returns the
  convex intersection `A ∩ B` (a valid closed mesh, `V - E + F = 2`), computed
  exactly by half-space clipping of `a` against every face half-space of `b`.
- [`BooleanMode::Union`], [`BooleanMode::Difference`], and any **non-convex**
  operand are computed by [`crate::boolean_general::boolean_general`]
  (surface arrangement + winding classification; triangulated output).
- An **empty / non-overlapping** intersection, or a coplanar-overlap /
  degenerate arrangement input, returns [`BooleanError::Unsupported`] rather
  than a silently wrong mesh.

Both meshes are dimensionless model-space geometry (see [`crate::math`]);
`mode` selects the CSG operation. Operands are taken by shared reference and
are not modified.

```rust
pub fn boolean(a: &crate::mesh::Mesh, b: &crate::mesh::Mesh, mode: crate::ops::BooleanMode) -> Result<crate::mesh::Mesh, BooleanError> { /* ... */ }
```

## Module `boolean_classify`

Inside/outside point classification for a closed triangle mesh.

This is the primitive a **general** mesh boolean needs to decide which
surface patches of the arrangement to keep: once two operand surfaces are
cut against each other, each resulting patch is kept or discarded by
sampling a point near it and asking "is this point inside the *other*
solid?" [`crate::boolean`]'s current convex half-space clipper does not
need this (a convex clip never has to ask "which side wins"), but the
honest `Unsupported` Union/Difference path documented there is exactly
the gap this module is a building block for.

Algorithm reference: Blender `mesh_boolean.cc`
(github.com/blender/blender @ 96294be75080bbf687fa7f108e344a1063713586,
GPL-2.0-or-later) — inside/outside classification concept (it tracks a
per-shape *winding number* per arrangement cell and applies a boolean-op
predicate to it — see its `Cell::winding_`, `propagate_windings_and_in_output_volume`,
and `apply_bool_op`). This module does **not** transcribe that code: it
implements the standalone **generalized winding number** point-in-solid
test (Jacobson, Kavan, Sorkine-Hornung 2013) from first principles, plus a
textbook ray-casting parity check as an independent cross-check. The
[`closest_point_on_triangle`] helper is the well-known point/triangle
closest-point algorithm (Ericson, *Real-Time Collision Detection*,
§5.1.5) — public-domain-style textbook algorithm, not Blender code.

## The two techniques

- [`winding_number`] — sum, over every triangle of the (fan-triangulated)
  mesh, of the **signed solid angle** that triangle subtends at query
  point `p` (Van Oosterom–Strackee formula), divided by `4*pi`. For a
  closed, outward-oriented, manifold surface this is (very close to) an
  **integer**: `0` outside, `+-1` inside a simply-connected solid (a
  surface wound inward, or a self-overlapping input, can in principle
  produce other integers — see Limitations). It degrades gracefully
  (continuously, not catastrophically) even for meshes with small gaps or
  local non-manifoldness, which is *why* Blender's own approach and this
  one both lean on the same underlying idea.
- a private ray-parity helper (Möller–Trumbore ray/triangle intersection,
  odd number of crossings along a ray from `p` to infinity ⇒ inside) —
  included as an independent numerical cross-check exercised in the test
  suite, not as the primary classifier.

## [`classify_point`] tolerance choices

All tolerances below are **relative to the mesh's bounding-box diagonal**
(`mesh_scale`, the same pattern [`crate::boolean::intersect_convex`] uses
for `clip_eps`/`weld`) so they hold at any model scale, not just
unit-sized test meshes:

- **`OnBoundary` detection** (`ON_BOUNDARY_REL_EPS = 1e-6`): `p` is
  `OnBoundary` if its distance to the *closest point on any triangle* is
  `<= 1e-6 * mesh_scale`. This is checked **before** computing the winding
  number, because a point essentially on the surface makes individual
  solid-angle terms numerically unstable (a near-zero denominator in the
  Van Oosterom–Strackee formula) even though the *sum* would still limit
  correctly for an exactly-on-surface point in exact arithmetic. `1e-6` is
  looser than `boolean.rs`'s `1e-7` convexity tolerance because a
  point-to-triangle closest-point query chains more floating-point
  operations (six dot products plus divisions) than a single plane
  distance, so it accumulates more rounding error.
- **`Inside`/`Outside` decision** (`|winding_number| > 0.5`): not a
  scale-dependent epsilon at all — for a point that is not on the surface,
  the winding number of a closed manifold is within numerical noise of an
  integer, so `0.5` is the natural half-way decision boundary with a huge
  safety margin (noise is typically `< 1e-6`, not anywhere near `0.5`).

## Limitations (read before trusting this on new geometry)

- **Requires a closed, manifold, consistently outward-oriented surface.**
  On an **open** mesh (a hole in the surface) the winding number varies
  continuously across the hole instead of jumping between integers, so
  `classify_point` can return a confident-looking `Inside`/`Outside` that
  is meaningless. This module does **not** check watertightness/manifoldness
  itself — callers must ensure the input is closed (e.g. everything
  [`crate::primitives`] generates, or [`crate::boolean::intersect_convex`]'s
  output).
- **Fan triangulation of n-gons is inline and naive** (`(v0, v_i, v_{i+1})`
  for `i = 1..n-1`): correct for the convex faces every generator in this
  crate produces, but a **non-convex** n-gon can fan-triangulate into
  triangles that overlap or fold outside the polygon, silently corrupting
  the classification. There is no general n-gon triangulator here.
- **Points exactly on the surface are inherently a hard case in floating
  point.** `OnBoundary` detection is a distance threshold, not an exact
  predicate; a point that is mathematically on the surface but lands just
  outside `ON_BOUNDARY_REL_EPS` due to input coordinate rounding will be
  classified `Inside` or `Outside` instead, and (rarely) a point that is
  merely *very close* to the surface without being on it can be
  misclassified `OnBoundary`. No epsilon-based test can avoid this
  trade-off; exact/rational arithmetic would be needed to eliminate it.
- The private ray-parity cross-check is deliberately **not** the primary
  classifier: a ray that grazes a shared edge between two triangles can
  double-count or miss a crossing. It defends against this by trying a
  fixed list of non-axis-aligned, mutually non-parallel directions and
  rejecting any direction whose barycentric hit coordinates land within
  `1e-9` of a triangle's edge (signalling "try another direction"); if
  *every* candidate direction is degenerate against a given mesh (never
  observed in this module's tests, but not provably impossible for
  adversarial input) it falls back to a majority vote across all
  candidates rather than panicking. It exists to validate
  [`winding_number`] in tests, not as a second production API.

> **Untrusted AI-generated draft until human-reviewed, per
> `RESPONSIBLE_USE.md`** — not for safety-critical use. Verified so far
> only against the analytic primitives ([`crate::primitives::cube`],
> [`crate::primitives::uv_sphere`]) and a hand-built concave L-prism (see
> tests below); not yet validated against arbitrary imported/scanned
> geometry.

```rust
pub mod boolean_classify { /* ... */ }
```

### Types

#### Enum `PointClass`

Result of classifying a point against a closed mesh's solid interior.

```rust
pub enum PointClass {
    Inside,
    Outside,
    OnBoundary,
}
```

##### Variants

###### `Inside`

The point is in the solid's interior (winding number `~= +-1`, away
from the surface).

###### `Outside`

The point is outside the solid (winding number `~= 0`, away from the
surface).

###### `OnBoundary`

The point lies on (within [`ON_BOUNDARY_REL_EPS`] * the mesh's
bounding-box diagonal of) the mesh surface itself — neither cleanly
inside nor outside. See the module docs' "Limitations" section for why
this is an epsilon-based judgement call, not an exact predicate.

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
    fn clone(self: &Self) -> PointClass { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &PointClass) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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

#### Function `winding_number`

The generalized winding number of `p` with respect to `mesh`'s (fan-
triangulated) surface.

This is `(1 / 4*pi)` times the sum, over every triangle, of the **signed
solid angle** that triangle subtends at `p`, computed per-triangle with
the Van Oosterom–Strackee formula:

`solid_angle = 2 * atan2(ra . (rb x rc), |ra||rb||rc| + (ra.rb)|rc| + (rb.rc)|ra| + (rc.ra)|rb|)`

where `ra, rb, rc` are the triangle's vertices relative to `p`
(`vertex - p`). Summed over a **closed**, outward-wound surface this
telescopes to (very close to) an integer: `0` for `p` outside the solid,
`+1` for `p` inside a simply-connected solid whose faces wind
counter-clockwise as seen from outside (the convention every
[`crate::primitives`] generator and [`crate::mesh::Mesh::face_normal`]
use) — see [`PointClass`] and the module docs' "Limitations" for what can
make this not hold (open meshes, inconsistent winding, self-overlap).

A triangle whose relative vertex is (numerically) coincident with `p`
(any of `|ra|, |rb|, |rc|` under `1e-12`) is skipped rather than dividing
by a near-zero denominator; [`classify_point`] avoids this case in the
first place by checking [`PointClass::OnBoundary`] before calling this
function, but `winding_number` stays well-defined (if slightly
approximate) for a caller that invokes it directly on a near-surface `p`.

```rust
pub fn winding_number(mesh: &crate::mesh::Mesh, p: crate::math::Vec3) -> f64 { /* ... */ }
```

#### Function `classify_point`

Classify `p` against `mesh`'s solid interior.

First checks [`PointClass::OnBoundary`] (distance to the closest point on
any triangle `<= ON_BOUNDARY_REL_EPS * mesh_scale`); if not on the
boundary, classifies by [`winding_number`]: `|w| > 0.5` is `Inside`,
otherwise `Outside`. See the module docs for the reasoning behind both
tolerances and the closed-manifold assumption this relies on.

```rust
pub fn classify_point(mesh: &crate::mesh::Mesh, p: crate::math::Vec3) -> PointClass { /* ... */ }
```

## Module `boolean_general`

General mesh boolean — Union / Difference / Intersect on arbitrary closed
meshes, by **surface arrangement + generalized-winding-number
classification**.

This is the "robust route" the restricted convex clipper in
[`crate::boolean`] deferred to: it lifts the boolean out of the
convex-`Intersect`-only case and handles **non-convex** (and self-contained
genus-`g`) operands in **all three modes**. It is the pipeline the boolean
*foundation* modules were built for — [`crate::boolean_predicates`] (robust
Shewchuk orientation) supplies the exact-sign tests that make the
retriangulation stable, and [`crate::boolean_classify`] (generalized winding
number) supplies the inside/outside decision.

## Provenance / reference

Architecture reference: Blender `mesh_boolean.cc` / `mesh_intersect.cc`
(github.com/blender/blender @ 96294be75080bbf687fa7f108e344a1063713586,
GPL-2.0-or-later; GPLv3-compatible per the workspace provenance rule). This
module does **not** transcribe Blender's code — Blender builds an exact
(rational-arithmetic) arrangement of the two surfaces, tracks a per-shape
*winding number* on each arrangement cell, and keeps a face when its two
adjacent cells disagree on membership in the output volume (see Blender's
`apply_bool_op` / `propagate_windings_and_in_output_volume`). We implement
the same *idea* with a self-contained, C-dependency-free pipeline suitable
for this crate (Android-buildable, no GMP): cut each operand's triangles
along the other surface, then classify each resulting patch by the winding
number of the *other* operand and keep/flip/drop it per the boolean op.

The keep/flip/drop rules below are the two-operand specialization of
Blender's winding test (`winding[0]`/`winding[1]` = inside operand A / B):

| Op | A-patch kept when | B-patch kept when | B-patch winding |
|---|---|---|---|
| Union      | outside B | outside A | preserved |
| Intersect  | inside B  | inside A  | preserved |
| Difference | outside B | inside A  | **flipped** |

(Union = "in either" ⇒ each operand's surface survives only where it is not
already buried inside the other; Intersect = "in both"; Difference `A \ B` =
"in A and not in B" ⇒ A's surface survives outside B, and B's surface,
flipped to face outward from the removed cavity, survives inside A.)

## The pipeline (per call)

1. **Triangulate** both operands (fan triangulation of each face).
2. **Intersect** every triangle of A against every triangle of B; each
   intersecting pair contributes one **segment** lying on both triangles.
   The *same* segment (identical `f64` endpoints) is handed to A's triangle
   *and* B's triangle, so the two operands' cut curves are numerically
   identical — the output welds watertight along the seam.
3. **Retriangulate** each cut triangle, constrained to its segments, via a
   2D **constrained Delaunay triangulation** ([`triangulate_constrained`],
   Bowyer–Watson + Anglada edge-insertion using the robust
   [`crate::boolean_predicates::orient2d`] / `incircle`). Uncut triangles
   pass through whole.
4. **Classify** each resulting sub-triangle by the winding number of the
   *other* operand at its centroid, and **select** (keep / keep-flipped /
   drop) per the table above.
5. **Weld** the kept triangles into one closed [`Mesh`].

## Why the constrained triangulation is well-conditioned here

The intersection of two **closed** surfaces is a set of closed 1-manifold
loops, so *within a single triangle* the constraint segments never cross
transversally — they meet only end-to-end where the curve passes between
adjacent triangles. That non-crossing property is what makes the
Anglada flip-insertion terminate cleanly (no need for Steiner points at
interior crossings).

## Limitations (read before trusting on new geometry — honest, not fake-green)

- **Generic position assumed.** Two operands that share a **coplanar
  overlapping face** are rejected with [`BooleanError::Unsupported`]
  ("coplanar faces") rather than guessed at — a coplanar overlap has no
  transverse intersection curve to cut along, so the winding classifier
  cannot resolve which coplanar patch wins. (Coplanarity *within* one
  operand is fine; only A-face-coplanar-with-B-face triggers this.)
- **Exactly-degenerate shared geometry** (a vertex of A exactly on a face of
  B, an edge of A exactly along an edge of B) can make a triangle pair's
  crossing ill-defined; such a pair is skipped (its cut is dropped) rather
  than forced. Nudging one operand by an irrational epsilon avoids it. This
  is the same class of hard case [`crate::boolean_classify`] documents for
  on-surface points.
- **Output is triangulated**, not merged back into co-planar n-gons. The
  Euler characteristic is still correct (triangulation-invariant), and
  downstream solvers triangulate anyway, but the face count is higher than a
  minimal polygonal result.
- **Requires closed, manifold, outward-oriented operands** — inherited from
  [`crate::boolean_classify::winding_number`]. Everything
  [`crate::primitives`] generates satisfies this; imported/scanned meshes
  must be repaired first.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod boolean_general { /* ... */ }
```

### Functions

#### Function `boolean_general`

Compute the boolean `A op B` of two **general closed** meshes by surface
arrangement + winding classification.

Handles [`BooleanMode::Union`], [`BooleanMode::Difference`], and
[`BooleanMode::Intersect`] on non-convex (but closed, manifold,
outward-oriented) operands — see the module docs for the full contract, the
keep/flip/drop rules, and the limitations (coplanar overlap, exact
degeneracy). Returns [`BooleanError::Unsupported`] for a coplanar-overlap
input or a result that welds to fewer than four faces (empty / degenerate).

Both meshes are dimensionless model-space geometry; `mode` selects the CSG
operation. Operands are taken by shared reference and are not modified.

```rust
pub fn boolean_general(a: &crate::mesh::Mesh, b: &crate::mesh::Mesh, mode: crate::ops::BooleanMode) -> Result<crate::mesh::Mesh, crate::boolean::BooleanError> { /* ... */ }
```

## Module `boolean_predicates`

Robust geometric predicates — orientation and in-circle/in-sphere tests.

Blender analogue / provenance: ported from `blender/blenlib`
`BLI_math_boolean.hh` / `intern/math_boolean.cc`, upstream repo
`github.com/blender/blender`, commit
`96294be75080bbf687fa7f108e344a1063713586`.

```text
SPDX-FileCopyrightText: 2023 Blender Authors
SPDX-License-Identifier: GPL-2.0-or-later
```

Adapted to pure Rust for `outram-blender` (GPL-3.0-only); GPL-2.0-or-later
is GPL-3.0-compatible per the workspace provenance rule. Only the `double`
(floating-point) predicate API is ported here — Blender's `mpq_class`
(GMP rational) overloads are intentionally **not** ported: this crate must
stay Android-buildable with **no C dependencies**, and GMP is a C library.

Blender's own `double` predicates are, in turn, a C++ adaptation of
Jonathan Shewchuk's `predicates.c` — "Routines for Arbitrary Precision
Floating-point Arithmetic and Fast Robust Geometric Predicates", placed in
the **public domain** by Jonathan Richard Shewchuk (Carnegie Mellon
University, May 1996). The error-bound coefficients used below
(`CCW_ERR_BOUND_A`, `O3D_ERR_BOUND_A`, `ICC_ERR_BOUND_A`) are taken
directly from Shewchuk's `exactinit()` derivation as reproduced in
Blender's `math_boolean.cc`.

## Robustness contract — what is actually implemented here

This is **not** a line-for-line port of Shewchuk's multi-stage adaptive
expansion arithmetic (`orient2dadapt`/`orient3dadapt`/`incircleadapt`,
which build growing exact "expansions" out of `double[]` arrays and only
spend as much precision as each case needs). That algorithm is correct
and fast, but reproducing its ~150-1000 lines of index-juggling C macros
by hand for `orient3d`/`incircle` carries real risk of a transcription bug
that silently produces a *wrong* sign — worse than an honest, simpler
scheme. Instead, each predicate here uses a **two-stage filter**:

1. **Fast path** — compute the determinant in plain `f64` (the `_fast`
   variant's formula) together with Shewchuk's rigorous `permanent`-based
   error bound (`errbound = C * permanent`, using the *exact* coefficients
   `C` from his error analysis). If `|det| > errbound`, the `f64` result's
   **sign is provably correct** — this stage is the real Shewchuk
   algorithm, not a simplification.
2. **Refine path** — if the fast filter is inconclusive (the near-
   degenerate case), recompute the *same* determinant formula using
   **double-double (`Dd`) arithmetic**: each `f64` is carried as an exact
   `(hi, lo)` pair via error-free transformations (Knuth's `two_sum`,
   FMA-based `two_prod`), giving roughly 106 bits of mantissa instead of
   53. The sign of the double-double result is returned.

Stage 2 is the **simplified/partial** part of this port, relative to
Shewchuk/Blender's full adaptive-precision expansion arithmetic:
double-double arithmetic is *not* arbitrary precision. It resolves the
sign correctly for the vast majority of practical near-degenerate
configurations (anything not degenerate below roughly the 106th
significant bit), and the [`tests`] module below demonstrates a concrete
case where the `_fast` plain-`f64` path returns the wrong sign and the
double-double-refined path returns the correct one. But it is not a
mathematical guarantee of exactness for *arbitrarily* degenerate
adversarial inputs (e.g. points constructed via exact expansion
arithmetic to be non-zero only at the 107th+ bit) — a full Shewchuk/CGAL-
style expansion port would be needed for that. This limitation is
intentional and documented rather than hidden.

[`insphere`] goes one step further in caution: deriving Shewchuk's precise
`isperrboundA` permanent formula correctly (it involves six 2x2
sub-determinants folded into four 3x3 cofactors) is easy to get subtly
wrong from a partial reading of the reference source, and a wrong
permanent bound could make the fast-path filter *unsoundly* trust an
incorrect `f64` sign. So [`insphere`] skips the fast-path filter entirely
and **always** evaluates via double-double arithmetic — slower, but never
unsoundly fast. This is called out again on the function itself.

## Sign convention

All predicates return a plain `i32` in `{-1, 0, +1}` (matching Blender's
`double` predicate signatures exactly), rather than an enum, so callers
can use ordinary integer comparisons/arithmetic on the result the same way
Blender's own callers do.

- [`orient2d`]/[`orient2d_fast`]: `+1` if `a, b, c` occur counter-clockwise,
  `-1` clockwise, `0` collinear.
- [`orient3d`]/[`orient3d_fast`]: `+1` if `d` lies *below* the plane through
  `a, b, c` (with `a, b, c` counter-clockwise when viewed from above that
  plane), `-1` above, `0` coplanar.
- [`incircle`]/[`incircle_fast`]: `+1` if `d` lies inside the circle
  through `a, b, c` (which must be given counter-clockwise, or the sign
  reverses), `-1` outside, `0` co-circular.
- [`insphere`]/[`insphere_fast`]: `+1` if `e` lies inside the sphere
  through `a, b, c, d` (which must be positively oriented per
  [`orient3d`], or the sign reverses), `-1` outside, `0` co-spherical.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod boolean_predicates { /* ... */ }
```

### Types

#### Struct `Vec2`

A 2-component vector, `f64` `x`/`y`.

Local to this module rather than added to [`crate::math`] — the boolean
predicates are the only 2D consumer in the crate today (see the task that
created this file). Dimensionless model-space coordinates, exactly like
[`crate::math::Vec3`].

```rust
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` | X component. |
| `y` | `f64` | Y component. |

##### Implementations

###### Methods

- ```rust
  pub const fn new(x: f64, y: f64) -> Self { /* ... */ }
  ```
  Construct a vector from explicit components.

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
    fn clone(self: &Self) -> Vec2 { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Vec2) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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

#### Function `orient2d_fast`

Plain-`f64` 2D orientation test. **Not robust** — near-collinear inputs
can silently return the wrong sign (or `0`) due to floating-point
cancellation. Use [`orient2d`] unless the caller has already established
the points are well away from collinear.

Returns `+1` if `a, b, c` occur counter-clockwise, `-1` if clockwise, `0`
if (numerically) collinear. The magnitude of the underlying determinant is
twice the signed area of triangle `abc`.

```rust
pub fn orient2d_fast(a: Vec2, b: Vec2, c: Vec2) -> i32 { /* ... */ }
```

#### Function `orient2d`

Robust 2D orientation test — see the module-level robustness contract.

Returns `+1` if `a, b, c` occur counter-clockwise, `-1` if clockwise, `0`
if *exactly* (to double-double precision) collinear. Correct even when
[`orient2d_fast`] would flip sign or misreport `0` due to cancellation —
see `tests::orient2d_fast_gets_near_collinear_case_wrong` /
`tests::orient2d_robust_resolves_the_same_case_correctly` below for a
concrete demonstration.

```rust
pub fn orient2d(a: Vec2, b: Vec2, c: Vec2) -> i32 { /* ... */ }
```

#### Function `orient3d_fast`

Plain-`f64` 3D orientation test. **Not robust** — see [`orient2d_fast`]'s
caveat; the same cancellation risk applies in 3D. Use [`orient3d`] unless
the caller has already established the points are well away from
coplanar.

Returns `+1` if `d` lies below the plane through `a, b, c` (with `a, b, c`
counter-clockwise viewed from above that plane), `-1` if above, `0` if
(numerically) coplanar. The magnitude of the underlying determinant is six
times the signed volume of tetrahedron `abcd`.

```rust
pub fn orient3d_fast(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3) -> i32 { /* ... */ }
```

#### Function `orient3d`

Robust 3D orientation test — see the module-level robustness contract.

Returns `+1` if `d` lies below the plane through `a, b, c` (with `a, b, c`
counter-clockwise viewed from above), `-1` if above, `0` if *exactly* (to
double-double precision) coplanar.

```rust
pub fn orient3d(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3) -> i32 { /* ... */ }
```

#### Function `incircle_fast`

Plain-`f64` 2D in-circle test. **Not robust** — see [`orient2d_fast`]'s
caveat; the same cancellation risk applies here. Use [`incircle`] unless
the caller has already established `d` is well away from the circle
through `a, b, c`.

Returns `+1` if `d` lies inside the circle through `a, b, c` (which must
be given counter-clockwise, or the sign reverses), `-1` if outside, `0` if
(numerically) co-circular.

```rust
pub fn incircle_fast(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> i32 { /* ... */ }
```

#### Function `incircle`

Robust 2D in-circle test — see the module-level robustness contract.

Returns `+1` if `d` lies inside the circle through `a, b, c` (which must
be given counter-clockwise, or the sign reverses), `-1` if outside, `0` if
*exactly* (to double-double precision) co-circular.

```rust
pub fn incircle(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> i32 { /* ... */ }
```

#### Function `insphere_fast`

Plain-`f64` 3D in-sphere test. **Not robust** — see [`orient2d_fast`]'s
caveat. Use [`insphere`] unless the caller has already established `e` is
well away from the sphere through `a, b, c, d`.

Returns `+1` if `e` lies inside the sphere through `a, b, c, d` (which
must be positively oriented per [`orient3d`], or the sign reverses), `-1`
if outside, `0` if (numerically) co-spherical.

```rust
pub fn insphere_fast(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3, e: crate::math::Vec3) -> i32 { /* ... */ }
```

#### Function `insphere`

Robust 3D in-sphere test.

Returns `+1` if `e` lies inside the sphere through `a, b, c, d` (which
must be positively oriented per [`orient3d`], or the sign reverses), `-1`
if outside, `0` if *exactly* (to double-double precision) co-spherical.

**Unlike [`orient2d`]/[`orient3d`]/[`incircle`] above, this always
evaluates in double-double precision — there is no `f64` fast-path
filter.** Shewchuk's real `isperrboundA`/permanent formula for insphere
folds together six 2x2 sub-determinants (`ab`, `bc`, `cd`, `da`, `ac`,
`bd`) into four 3x3 cofactors before the final 4-term combination; getting
that error-bound derivation subtly wrong from a partial read of the
reference source is a real risk, and a *too-tight* bound would make the
fast path unsoundly trust a wrong `f64` sign — silently worse than doing
no filtering at all. So this implementation is deliberately conservative:
always pay the double-double cost, never risk an unsound fast path. A
future contributor who re-derives and carefully verifies the exact
`isperrboundA` permanent formula against Shewchuk's source can add the
fast path the same way [`orient3d`]/[`incircle`] do.

```rust
pub fn insphere(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3, e: crate::math::Vec3) -> i32 { /* ... */ }
```

## Module `convex_hull`

**3D convex hull** of a point set — the incremental algorithm on the robust
[`crate::boolean_predicates::orient3d`] orientation test.

Given a set of points, [`convex_hull`] returns the closed, watertight,
outward-wound triangle [`Mesh`] of their convex hull (the smallest convex
solid containing them all). It complements the CSG / boolean suite — a hull
is the natural bounding volume, and the input to a convex decomposition.

## Method (incremental)

Start from a non-degenerate tetrahedron of four input points (chosen for good
conditioning: a farthest pair, the point of largest triangle area with that
edge, then the point farthest off their plane), with all four faces wound
**CCW as seen from outside**. Then add the remaining points one at a time:

- a face `(a, b, c)` is **visible** from a new point `p` iff `p` is on its
  outward side, i.e. `orient3d(a, b, c, p) == -1` (the crate's outward normal
  is the "above" side of `orient3d`, so "above" = `-1` = outward);
- if no face is visible, `p` is inside-or-on the hull and is skipped;
- otherwise the **horizon** (the loop of edges bordering a visible and a
  non-visible face) is found from the visible faces' directed edges — an edge
  `(a, b)` is on the horizon iff its reverse `(b, a)` is not also a visible
  edge — the visible faces are deleted, and each horizon edge `(a, b)` is
  coned to `p` as a new triangle `[a, b, p]` (already outward-wound).

All orientation decisions use the exact/robust `orient3d`, so the hull is
correct even for near-degenerate configurations that a naive `f64` sign would
misjudge. Coplanar points are handled by the strict `== -1` visibility rule
(a coplanar face is never deleted, so no sliver/duplicate faces arise; a
coplanar cube face still splits into two triangles via its non-coplanar
neighbours).

## Degeneracies

Duplicate points are removed first. Fewer than four distinct points, an
all-collinear set, or an all-coplanar set have no 3D hull and return a
[`HullError`] rather than a degenerate/open mesh.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod convex_hull { /* ... */ }
```

### Types

#### Enum `HullError`

Errors from [`convex_hull`].

```rust
pub enum HullError {
    NotEnoughPoints(usize),
    Collinear,
    Coplanar,
}
```

##### Variants

###### `NotEnoughPoints`

Fewer than four **distinct** points were supplied — a tetrahedron (the
minimal 3D hull) needs four.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Collinear`

Every point is collinear — the hull would be a line segment, not a solid.

###### `Coplanar`

Every point is coplanar — the hull would be a flat polygon, not a solid.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn from(source: crate::convex_hull::HullError) -> Self { /* ... */ }
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

#### Function `convex_hull`

The convex hull of `points`, as a closed outward-wound triangle [`Mesh`].

Every input point lies on or inside the returned hull; the result is a
watertight genus-0 mesh (`V − E + F = 2`) with all faces wound
counter-clockwise as seen from outside. See the module docs for the method.

# Errors

[`HullError::NotEnoughPoints`] for fewer than four distinct points,
[`HullError::Collinear`] / [`HullError::Coplanar`] for a degenerate (lower
dimensional) input.

```rust
pub fn convex_hull(points: &[crate::math::Vec3]) -> Result<crate::mesh::Mesh, HullError> { /* ... */ }
```

## Module `decimate`

**QEM mesh decimation** — Garland–Heckbert quadric-error-metric edge-collapse
simplification.

Blender analogue: the **Decimate** modifier (`MOD_decimate`, "Collapse"
mode). Reduces a mesh's triangle count while preserving its shape, by
repeatedly collapsing the edge whose removal introduces the least
*quadric error* (squared distance to the planes of the original surface).

## The method (Garland & Heckbert, 1997)

Each vertex carries a `4×4` symmetric **error quadric** `Q` — the sum of the
outer products `p pᵀ` of the plane equations `p = (a, b, c, d)` of its
incident triangles (area-weighted). The squared distance of a point `v` to
all those planes is `vᵀ Q v`. To collapse edge `(i, j)` we combine
`Q = Q_i + Q_j`, place the merged vertex at the `v` minimizing `vᵀ Q v` (a
`3×3` solve, with a fallback to the cheaper of the endpoints/midpoint when
that is singular), and use that minimum as the collapse **cost**. A min-heap
keyed on cost drives a greedy sequence of collapses until a target face count
is reached.

Guards keep the result sane: a **link-condition** check rejects collapses
that would make the mesh non-manifold, a **normal-flip** check rejects those
that would fold a triangle over, and **boundary** edges get a large penalty
quadric so an open border is preserved.

Everything here is closed-form (a hand-written symmetric-`3×3` solve and a
version-stamped lazy-deletion heap); no `faer` is needed. Works on the
polygon-soup view and rebuilds via [`Mesh::from_polygons`] — no half-edge
surgery.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod decimate { /* ... */ }
```

### Types

#### Enum `StopReason`

Why [`decimate`] stopped collapsing.

```rust
pub enum StopReason {
    ReachedTarget,
    NoLegalCollapse,
}
```

##### Variants

###### `ReachedTarget`

The requested target face count was reached.

###### `NoLegalCollapse`

No legal collapse remained (every candidate was rejected by the manifold
/ flip / boundary guards) before the target was reached.

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
    fn clone(self: &Self) -> StopReason { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &StopReason) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Struct `DecimateResult`

The result of a decimation: the simplified mesh and why it stopped.

```rust
pub struct DecimateResult {
    pub mesh: crate::mesh::Mesh,
    pub stop_reason: StopReason,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `crate::mesh::Mesh` | The simplified mesh. |
| `stop_reason` | `StopReason` | Why decimation stopped (target reached, or ran out of legal collapses). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DecimateResult { /* ... */ }
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

#### Function `decimate`

Simplify `mesh` down to roughly `target_faces` triangles by QEM edge
collapse, returning the simplified [`Mesh`].

Convenience wrapper over [`decimate_with_reason`] that discards the stop
reason. `target_faces` is a *lower bound goal*: the result has at most a
couple more faces than the target (collapses remove faces in pairs), or more
if no further legal collapse exists. Non-triangular faces are fan-triangulated
first, so the output is a triangle mesh.

```rust
pub fn decimate(mesh: &crate::mesh::Mesh, target_faces: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `decimate_with_reason`

Like [`decimate`] but also reports the [`StopReason`].

```rust
pub fn decimate_with_reason(mesh: &crate::mesh::Mesh, target_faces: usize) -> DecimateResult { /* ... */ }
```

## Module `edge_bevel`

Edge bevel — chamfer every edge of a closed mesh.

This is the pure-Rust analogue of Blender's **Bevel** on edges (as opposed
to the vertex-truncation [`crate::ops::bevel_vertices`]). Each face is cut
back from its edges by `width`, the gap left along every original **edge**
is filled by a bevel face, and the gap at every original **vertex** is
capped — turning each sharp edge into a flat chamfer.

# Construction

For each face and each of its corners, a new **face-corner vertex** is placed
by moving the corner inward along its two incident in-face edge directions by
`width` (exact for right-angle corners, a good approximation otherwise). The
output is then three families of faces:

- **shrunk faces** — each original face, rebuilt on its face-corner vertices;
- **edge chamfers** — one quad per original edge, bridging the two shrunk
  faces that met there;
- **vertex caps** — one polygon per original vertex, filling the truncated
  corner (its incident face-corner vertices in umbrella order).

Only the **connectivity** is built here; the final consistent, outward
winding is delegated to [`crate::recalc_normals::recalculate_normals`], so
the construction never has to reason about per-face orientation.

# Scope (v1)

Flat chamfer (a single bevel face per edge; rounded multi-segment bevels are
future work). Intended for **closed manifold** meshes; boundary edges/vertices
are left uncapped. `width` must be smaller than half the shortest edge, or a
face can invert. No `faer`, no external dependency; Android-safe.

```rust
pub mod edge_bevel { /* ... */ }
```

### Functions

#### Function `bevel_edges`

Chamfer every edge of `mesh` by `width`, returning the beveled mesh.

`width` is the distance each face is cut back from its edges, in mesh units;
keep it below half the shortest edge length to avoid inverting a face. The
result is consistently wound and outward-facing. This is infallible.

# Examples

```
use outram_blender::{primitives, edge_bevel::bevel_edges};

// Beveling a cube's 12 edges: 6 shrunk squares + 12 edge quads + 8 corner
// triangles = 26 faces, still a closed genus-0 surface (χ = 2).
let beveled = bevel_edges(&primitives::cube(2.0), 0.3);
assert_eq!(beveled.face_count(), 26);
assert_eq!(beveled.euler_characteristic(), 2);
```

```rust
pub fn bevel_edges(mesh: &crate::mesh::Mesh, width: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `export`

Export bridges from an authored [`Mesh`] to the OUTRAM PARK solvers.

The **default** exporters are self-contained and dependency-free: the
OpenFOAM bridge emits **text** (the polyMesh ASCII files as `String`s), the
CSG bridge emits **local mirror types** ([`CsgSurface`], [`RegionToken`],
[`CsgDescription`]) that shadow the consumer crate's geometry vocabulary, and
[`FacetedSolid`] carries a triangulated boundary. So the frontend stays light
and Android-buildable with no solver-crate dependency.

**Real-type bridges** to the actual solver crates are available behind opt-in
cargo features, so neither solver crate is a hard dependency:

- `foam-export` → `to_poly_mesh` returns a real
  `outram_foam_basic_lib::io::poly_mesh::PolyMesh` (the type its OpenFOAM
  reader/writer round-trips), and `from_poly_mesh` reads one back into a
  [`Mesh`] — combined with `PolyMesh::read(dir)` this **imports** an OpenFOAM
  `constant/polyMesh` directory, the inverse of `write_polymesh`;
- `mc-export` → `to_mc_geometry` returns a real
  `outram_mc_libs::prelude::Geometry` (surfaces + a cell region).

These are the wired counterparts of the text / mirror exporters (epic
`op-hzs`, beads `op-hzs.6`/`op-hzs.7`).

An authored mesh here is a **boundary surface** — a shell of vertices,
edges, and polygon faces, with *no cells*. That single fact shapes both
bridges, so it is stated up front and repeated at each export point rather
than hidden.

## 1. OpenFOAM polyMesh (CFD) — [`to_polymesh_text`] / [`write_polymesh`]

OpenFOAM's `polyMesh` (mirrored by `outram_foam_basic_lib`'s `io::poly_mesh`)
is normally a finite-volume **VOLUME** mesh: `points`, `faces`,
`owner[f]`/`neighbour[f]` (the two cells straddling each *internal* face),
and named boundary **patches**.

Our surface mesh has faces but **no cells**, so what we emit is a polyMesh
**boundary description**, not a solve-ready volume mesh:

- `points` — the vertex coordinates;
- `faces` — each face as `n(v0 v1 …)`, wound as the mesh winds it;
- `owner` — every face owned by a single **dummy cell `0`**, because a
  surface has no real cells;
- `neighbour` — **empty** (there are no internal faces);
- `boundary` — one patch `authoredSurface` of `type patch`, covering all
  faces.

This is the input a volume mesher (blockMesh / snappyHexMesh) would *fill*
to produce cells; it is a boundary patch, **not** a ready-to-solve mesh.
Coordinates are dimensionless model space — the caller assigns a length
unit (conventionally metres) when handing the mesh to a solver.

## 2. `outram-mc-libs` CSG (Monte Carlo transport) — [`to_csg_primitive`]

`outram-mc-libs`'s geometry is **constructive solid geometry**: analytic
surfaces (`XPlane`/`YPlane`/`ZPlane`, `Sphere`, `ZCylinder`, …) combined by
an RPN region of signed half-spaces into cells. A triangulated boundary
mesh does not map onto analytic surfaces directly, so the bridge takes the
**primitive-fitting** route: recognise that a mesh *came from* a
[`crate::primitives`] generator and emit the exact analytic CSG for it.

Implemented analytic fits: an axis-aligned **cube/box** (six planes), a
**uv-sphere** (one `Sphere`), a **Z-axis cylinder** (`ZCylinder` ∩ two
`ZPlane` caps), and **any convex polyhedron** (the faceted convex route: one
general [`CsgSurface::Plane`] per face, intersected — exact because a convex
solid is the intersection of its face half-spaces). A **non-convex** mesh is
not a half-space intersection, so [`to_csg_primitive`] returns
[`ExportError::NotImplemented`] for it.

## 3. Faceted / DAGMC boundary (non-convex Monte Carlo) — [`to_faceted_solid`]

For an arbitrary (non-convex) solid — e.g. a general boolean result — the
[`FacetedSolid`] route keeps the triangulated boundary as-is (outward
oriented) and answers inside/outside by the **generalized winding number**,
the DAGMC point-in-volume idea. This is the honest representation when no
analytic primitive fits.

## Shared foundation — [`triangulate`]

[`triangulate`] provides a dependency-free indexed triangle soup
([`IndexedTriangles`]) — the common denominator an OBJ/STL writer, a
polyMesh patch, or a faceted-CSG surface each build on. It is fully
implemented and tested.

```rust
pub mod export { /* ... */ }
```

### Types

#### Enum `ExportError`

Errors from an export bridge.

```rust
pub enum ExportError {
    NotImplemented(&'static str),
}
```

##### Variants

###### `NotImplemented`

A requested export path is documented but not implemented for this mesh.

Returned, for example, when [`to_csg_primitive`] is handed a mesh it
cannot recognise as one of the fitted [`crate::primitives`] shapes (the
general faceted/DAGMC route is not written yet). The payload is a
human-readable explanation of what was expected.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Struct `IndexedTriangles`

A dependency-free, flattened triangle mesh: the common export denominator.

Every polygon face of a [`Mesh`] is fan-triangulated into this indexed form
(`positions` + triangle `indices` triplets). This is what an OBJ/STL writer,
a polyMesh boundary patch, or a faceted-CSG surface would each build from —
so it is implemented and tested even while the solver bridges are stubs.

```rust
pub struct IndexedTriangles {
    pub positions: Vec<crate::math::Vec3>,
    pub indices: Vec<u32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `positions` | `Vec<crate::math::Vec3>` | Vertex positions, indexed by the entries of [`IndexedTriangles::indices`]. |
| `indices` | `Vec<u32>` | Flat list of triangle corner indices, three consecutive entries per<br>triangle, each indexing into [`IndexedTriangles::positions`]. |

##### Implementations

###### Methods

- ```rust
  pub fn triangle_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of triangles (== `indices.len() / 3`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> IndexedTriangles { /* ... */ }
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
    fn default() -> IndexedTriangles { /* ... */ }
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
#### Struct `PolyMeshText`

The five OpenFOAM `polyMesh` ASCII files, each held as a `String`.

Produced by [`to_polymesh_text`] with no filesystem access, so it is fully
unit-testable; [`write_polymesh`] is the only function that touches disk.

**These describe a boundary SURFACE, not a solve-ready volume mesh.** Every
face is owned by a single dummy cell `0` and there are no internal faces
(`neighbour` is empty). A volume mesher must fill the interior before this
is a mesh a CFD solver can march on. See the module docs.

Coordinates are dimensionless model space; the caller assigns a length unit
(conventionally metres) at export.

```rust
pub struct PolyMeshText {
    pub points: String,
    pub faces: String,
    pub owner: String,
    pub neighbour: String,
    pub boundary: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `String` | `constant/polyMesh/points` — `class vectorField; object points;`. One<br>`(x y z)` per mesh vertex, in [`crate::mesh::VertexId`] order. |
| `faces` | `String` | `constant/polyMesh/faces` — `class faceList; object faces;`. One<br>`n(v0 v1 …)` per mesh face, wound as the mesh winds it. |
| `owner` | `String` | `constant/polyMesh/owner` — `class labelList; object owner;`. One label<br>per face, all `0` (the single dummy cell). A `note` records that this is<br>a boundary patch, not a volume mesh. |
| `neighbour` | `String` | `constant/polyMesh/neighbour` — `class labelList; object neighbour;`.<br>Empty (zero entries): a boundary surface has no internal faces. |
| `boundary` | `String` | `constant/polyMesh/boundary` — `class polyBoundaryMesh; object boundary;`.<br>A single patch `authoredSurface` of `type patch`, `nFaces` = face count,<br>`startFace 0`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PolyMeshText { /* ... */ }
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
#### Enum `CsgSurface`

An analytic CSG surface — a **local mirror** of `outram-mc-libs`'s
`SurfaceKind`.

Each variant is the implicit surface `f(x, y, z) = 0`; its
[`CsgSurface::signed_value`] gives `f`, whose sign selects a half-space
(see [`Sense`]). Offsets and radii are dimensionless model-space lengths
(the caller assigns a length unit, conventionally metres, at export). The
variant set here is exactly the subset the primitive fitter emits;
`outram-mc-libs` defines the same shapes.

```rust
pub enum CsgSurface {
    XPlane {
        x0: f64,
    },
    YPlane {
        y0: f64,
    },
    ZPlane {
        z0: f64,
    },
    Sphere {
        x0: f64,
        y0: f64,
        z0: f64,
        r: f64,
    },
    ZCylinder {
        x0: f64,
        y0: f64,
        r: f64,
    },
    Plane {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
    },
}
```

##### Variants

###### `XPlane`

Plane `x = x0`, normal along +X. `f = x - x0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` | The X coordinate of the plane. |

###### `YPlane`

Plane `y = y0`, normal along +Y. `f = y - y0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `y0` | `f64` | The Y coordinate of the plane. |

###### `ZPlane`

Plane `z = z0`, normal along +Z. `f = z - z0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `z0` | `f64` | The Z coordinate of the plane. |

###### `Sphere`

Sphere of radius `r` centred at `(x0, y0, z0)`.
`f = (x-x0)^2 + (y-y0)^2 + (z-z0)^2 - r^2` (negative inside).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` | Centre X. |
| `y0` | `f64` | Centre Y. |
| `z0` | `f64` | Centre Z. |
| `r` | `f64` | Radius (`> 0`). |

###### `ZCylinder`

Infinite cylinder of radius `r` about the line `x = x0, y = y0`, axis
parallel to Z. `f = (x-x0)^2 + (y-y0)^2 - r^2` (negative inside).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` | Axis X. |
| `y0` | `f64` | Axis Y. |
| `r` | `f64` | Radius (`> 0`). |

###### `Plane`

General (arbitrarily oriented) plane with unit normal `(a, b, c)` at
signed distance `d` from the origin along that normal:
`f = a*x + b*y + c*z - d`. The `+normal` side is `f > 0`. Used by the
faceted convex route ([`to_csg_primitive`]), where each face of a convex
polyhedron becomes one such plane. `(a, b, c)` is expected to be unit
length so `f` is a true signed distance.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Normal X component (unit normal). |
| `b` | `f64` | Normal Y component (unit normal). |
| `c` | `f64` | Normal Z component (unit normal). |
| `d` | `f64` | Signed distance of the plane from the origin along `(a, b, c)`. |

##### Implementations

###### Methods

- ```rust
  pub fn signed_value(self: &Self, p: Vec3) -> f64 { /* ... */ }
  ```
  Evaluate the implicit function `f(p)` for this surface.

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
    fn clone(self: &Self) -> CsgSurface { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &CsgSurface) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Enum `Sense`

Which side of a [`CsgSurface`] a half-space selects — a local mirror of
`outram-mc-libs`'s surface sense.

```rust
pub enum Sense {
    Positive,
    Negative,
}
```

##### Variants

###### `Positive`

The `f > 0` side (outside a sphere/cylinder; +axis side of a plane).

###### `Negative`

The `f < 0` side (inside a sphere/cylinder; -axis side of a plane).

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
    fn clone(self: &Self) -> Sense { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Sense) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Enum `RegionToken`

One token of an RPN CSG region — a **local mirror** of `outram-mc-libs`'s
`RegionToken` (`Cell.region`).

The region is evaluated as a stack machine over booleans (see
[`CsgDescription::contains`]): a [`RegionToken::Halfspace`] pushes "point is
on the chosen side of surface `surface`"; [`RegionToken::Intersection`] /
[`RegionToken::Union`] pop two and push their AND / OR;
[`RegionToken::Complement`] pops one and pushes its negation.

```rust
pub enum RegionToken {
    Halfspace {
        surface: usize,
        sense: Sense,
    },
    Intersection,
    Union,
    Complement,
}
```

##### Variants

###### `Halfspace`

The signed half-space of surface index `surface` on side `sense`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `surface` | `usize` | Index into [`CsgDescription::surfaces`]. |
| `sense` | `Sense` | Which side of that surface this half-space is. |

###### `Intersection`

Boolean AND of the top two operands (set intersection).

###### `Union`

Boolean OR of the top two operands (set union).

###### `Complement`

Boolean NOT of the top operand (set complement).

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
    fn clone(self: &Self) -> RegionToken { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RegionToken) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Struct `CsgDescription`

A complete CSG solid: analytic `surfaces` plus an RPN `region` over them.

A **local mirror** of the geometry `outram-mc-libs` consumes — surfaces
(`SurfaceKind`) and a region (`Cell.region`) — so this crate need not depend
on it. Produced by [`to_csg_primitive`]. Lengths are dimensionless model
space; a length unit (conventionally metres) is assigned at export.

```rust
pub struct CsgDescription {
    pub surfaces: Vec<CsgSurface>,
    pub region: Vec<RegionToken>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `surfaces` | `Vec<CsgSurface>` | The analytic surfaces, referenced by index from `region`. |
| `region` | `Vec<RegionToken>` | The region as an RPN token stream over `surfaces` (see [`RegionToken`]). |

##### Implementations

###### Methods

- ```rust
  pub fn contains(self: &Self, p: Vec3) -> bool { /* ... */ }
  ```
  Test whether point `p` lies inside this CSG region.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> CsgDescription { /* ... */ }
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
    fn eq(self: &Self, other: &CsgDescription) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Struct `FacetedSolid`

A solid represented **directly by its triangulated boundary** — the
DAGMC-style representation for meshes that are *not* a recognised primitive
and are *not* convex (so they cannot be a CSG half-space intersection).

This is the honest export route for an arbitrary boolean result: rather than
forcing it into analytic surfaces, the solid *is* its outward-oriented
triangle surface, and inside/outside is decided by the **generalized winding
number** (the same test [`crate::boolean_classify`] uses and DAGMC's
point-in-volume query embodies). A local mirror — `outram-mc-libs` would
consume the triangle soup for ray-traced surface tracking; wiring that real
dependency is deferred with the rest of this module.

Coordinates are dimensionless model space; a length unit (conventionally
metres) is assigned when the solid reaches the transport solver.

```rust
pub struct FacetedSolid {
    pub positions: Vec<crate::math::Vec3>,
    pub triangles: Vec<[u32; 3]>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `positions` | `Vec<crate::math::Vec3>` | Vertex positions, indexed by [`FacetedSolid::triangles`]. |
| `triangles` | `Vec<[u32; 3]>` | Outward-wound triangles (each a corner-index triple into<br>[`FacetedSolid::positions`]). Outward orientation is enforced at<br>construction so face normals point out of the solid. |

##### Implementations

###### Methods

- ```rust
  pub fn triangle_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of boundary triangles.

- ```rust
  pub fn contains(self: &Self, p: Vec3) -> bool { /* ... */ }
  ```
  Test whether point `p` is inside the solid, by the **generalized winding

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FacetedSolid { /* ... */ }
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
    fn default() -> FacetedSolid { /* ... */ }
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

#### Function `triangulate`

Fan-triangulate every face of `mesh` into an [`IndexedTriangles`] soup.

Each `n`-gon face contributes `n - 2` triangles by a simple fan from its
first vertex — valid for the convex faces the [`crate::primitives`]
generators produce. Positions are copied 1:1 from the mesh (indices are
preserved), so `positions.len()` equals `mesh.vertex_count()`.

This is real, tested code — the dependency-free foundation the solver
bridges below build on.

```rust
pub fn triangulate(mesh: &crate::mesh::Mesh) -> IndexedTriangles { /* ... */ }
```

#### Function `to_polymesh_text`

Serialise `mesh` to the five OpenFOAM `polyMesh` files as strings.

Pure and filesystem-free (testable): builds `points`, `faces`, `owner`,
`neighbour`, and `boundary` from the mesh's public topology. Faces are
written in the mesh's own winding order. Every face is assigned owner cell
`0` (a surface has no real cells) and `neighbour` is empty, so the result is
a **boundary patch for a volume mesher to fill, not a solve-ready volume
mesh** (see [`PolyMeshText`]).

Coordinates are copied verbatim as dimensionless model-space `f64`s; the
caller assigns a length unit (conventionally metres) at export.

```rust
pub fn to_polymesh_text(mesh: &crate::mesh::Mesh) -> PolyMeshText { /* ... */ }
```

#### Function `write_polymesh`

Write the five [`PolyMeshText`] files into `dir/`, creating `dir` if needed.

The only filesystem-touching function in this module: it calls
[`to_polymesh_text`] and writes `points`, `faces`, `owner`, `neighbour`, and
`boundary` (no `.txt` extension — OpenFOAM's exact file names) into `dir`.
Typically `dir` is a case's `constant/polyMesh` directory. The output is a
boundary surface, not a solve-ready volume mesh (see [`to_polymesh_text`]).

```rust
pub fn write_polymesh(mesh: &crate::mesh::Mesh, dir: &std::path::Path) -> std::io::Result<()> { /* ... */ }
```

#### Function `to_csg_primitive`

Fit `mesh` to an analytic CSG solid consumable by `outram-mc-libs`.

Recognises the [`crate::primitives`] shapes it can express exactly and emits
the matching [`CsgDescription`] (local mirror types — no dependency on
`outram-mc-libs`):

- **axis-aligned cube/box** (8 vertices, 6 quad faces, all normals ±X/±Y/±Z,
  every vertex on the bounding box) → six planes at the box bounds, region =
  the intersection of the six inward half-spaces (the box interior);
- **uv-sphere** (vertices equidistant from their centroid, and vertex/face
  counts matching a `2 + (rings-1)*segments` / `rings*segments` uv-sphere) →
  one `Sphere` at the fitted centre/radius, region = its interior
  (`Negative` half-space);
- **Z-axis cylinder** (a [`crate::primitives::cylinder`]: `2*segments`
  vertices, `segments` side quads + two `segments`-gon caps, side vertices at
  a constant radius about a Z-parallel axis) → one `ZCylinder` intersected
  with two `ZPlane` caps;
- **any other convex closed polyhedron** → the **faceted convex** route: one
  general [`CsgSurface::Plane`] per face (outward normal), region = the
  intersection of every inward (`Negative`) half-space. This is exact for a
  convex solid — e.g. a rotated box or a convex boolean result — because a
  convex polyhedron *is* the intersection of its face half-spaces.

A **non-convex** mesh cannot be written as a single half-space intersection,
so it returns [`ExportError::NotImplemented`] here; use [`to_faceted_solid`]
for the DAGMC-style boundary representation of an arbitrary (non-convex)
solid.

Lengths are dimensionless model space; a length unit (conventionally metres)
is assigned when the description reaches the transport solver.

```rust
pub fn to_csg_primitive(mesh: &crate::mesh::Mesh) -> Result<CsgDescription, ExportError> { /* ... */ }
```

#### Function `to_faceted_solid`

Build the DAGMC-style [`FacetedSolid`] boundary of `mesh`: fan-triangulate
every face and orient the triangles **outward** (positive enclosed volume).

Works for any closed mesh, convex or not — this is the fallback the analytic
[`to_csg_primitive`] fitters do not cover. Outward orientation is enforced so
that a downstream consumer using face normals (not just the sign-agnostic
[`FacetedSolid::contains`]) sees them pointing out of the solid.

```rust
pub fn to_faceted_solid(mesh: &crate::mesh::Mesh) -> FacetedSolid { /* ... */ }
```

## Module `fill_holes`

Fill holes — cap the open boundary loops of a surface so it becomes
watertight.

This is the pure-Rust analogue of Blender's **Fill Holes**
(`bmesh` `bmo_holes_fill`): every boundary loop (a closed chain of edges
each incident to only one face) is capped with a triangle fan to the loop's
centroid, closing the surface. Together with [`crate::weld`] it is the
mesh-repair pair the export bridges rely on — weld stitches near-coincident
seams, fill-holes closes genuine gaps — so that an open authored surface can
become the closed geometry the [`crate::export`] Monte-Carlo CSG bridge
needs.

# How a hole is found and capped

A **boundary edge** is a directed half-edge `a → b` (from a face's winding)
whose reverse `b → a` appears in no face — i.e. the edge borders exactly one
face. Chaining boundary half-edges tail-to-head recovers each boundary
**loop**. Each loop is capped by adding one **centroid** vertex at the
average of the loop's positions and one triangle per boundary edge.

## Winding

For the cap to be consistent with the existing surface, two adjacent faces
must traverse their shared edge in **opposite** directions. A boundary edge
`a → b` belongs to an existing face that traverses it `a → b`, so its cap
triangle must traverse it `b → a`; the emitted triangle is therefore
`[b, a, centroid]`. This keeps every face's outward normal consistent
without any explicit normal computation.

# Scope

The centroid fan is always topologically valid and makes the surface
watertight, but for a strongly non-planar or non-convex hole the fan is a
*valid* cap, not a *minimal-area* or *beauty* triangulation — an honest
limitation, documented rather than hidden. A loop with fewer than three
edges (degenerate) is left uncapped. No `faer`, no external dependency;
Android-safe.

```rust
pub mod fill_holes { /* ... */ }
```

### Functions

#### Function `fill_holes`

Cap every open boundary loop of `mesh` with a centroid triangle fan,
returning the watertight mesh.

A mesh that is already closed (no boundary edges) is returned unchanged (a
no-op rebuild). Each hole adds one centroid vertex and one triangle per
boundary edge; the winding of every cap triangle is chosen to stay
consistent with the surrounding surface.

This is infallible: the result is always a valid mesh. Degenerate boundary
loops (fewer than three edges) are left uncapped.

# Examples

```
use outram_blender::{primitives, fill_holes::fill_holes};

// A closed cube has no holes, so filling is a no-op: still V=8, F=6, chi=2.
let cube = primitives::cube(2.0);
let filled = fill_holes(&cube);
assert_eq!(filled.vertex_count(), 8);
assert_eq!(filled.face_count(), 6);
assert_eq!(filled.euler_characteristic(), 2);
```

```rust
pub fn fill_holes(mesh: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

## Module `inset`

Inset faces — replace each face with a smaller inner copy plus a bridging
ring of quads.

This is the pure-Rust analogue of Blender's **Inset Faces**
(`bmo_inset`, *Individual* mode): every face is shrunk toward its own
centroid by a fraction, the shrunk copy becomes an **inner face**, and a
ring of quads bridges the original boundary to the inner face. It is the
standard modelling operation for adding a border loop around each face
(panelling, framing, controlled bevelling of the interior).

# Individual mode

Each face gets its **own** inner vertices — they are not shared between
adjacent faces — which is Blender's *Individual* inset. This keeps the
construction purely local and always valid: the original corner vertices
stay shared between neighbours (so the surface stays manifold and closed),
while the inset ring is independent per face.

# Winding

The inner face keeps the original winding (outward normal preserved). Each
ring quad `[vi, v(i+1), v(i+1)', vi']` traverses the inner edge opposite to
the inner face, so the whole result stays consistently wound — the same
adjacent-faces-oppose rule used elsewhere in the crate. No `faer`, no
external dependency; Android-safe.

```rust
pub mod inset { /* ... */ }
```

### Functions

#### Function `inset_faces`

Inset every face of `mesh` by `amount`, returning the new mesh.

`amount` is the fraction each corner moves **toward its face centroid**:
`0.0` leaves the face unchanged (a no-op), `0.5` halves the face, and values
approaching `1.0` collapse the inner face onto the centroid. Values `<= 0`
are treated as a no-op; values `>= 1` are clamped just below `1` to avoid a
degenerate zero-area inner face.

Each face becomes `1 + k` faces (one inner `k`-gon plus `k` ring quads) and
gains `k` new inner vertices. This is infallible.

# Examples

```
use outram_blender::{primitives, inset::inset_faces};

// Inset every face of a cube by 30%: 6 faces → 6 inner quads + 24 ring
// quads = 30 faces, still a closed genus-0 surface (χ = 2).
let cube = primitives::cube(2.0);
let inset = inset_faces(&cube, 0.3);
assert_eq!(inset.face_count(), 30);
assert_eq!(inset.euler_characteristic(), 2);
```

```rust
pub fn inset_faces(mesh: &crate::mesh::Mesh, amount: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `laplacian`

Discrete **Laplacian operators** over a mesh, and implicit **Laplacian
smoothing** (mesh fairing) built on them.

Blender analogue: the Smooth / "Smooth Vertices" and Laplacian-smooth mesh
operators (`MOD_laplaciansmooth`, `bmo_smooth_laplacian`). This is the first
module in the crate that assembles a **global sparse linear system over the
mesh** and solves it — the intended home for the `faer` sparse solver the
crate re-exports (see [`crate`] top-level docs). Future geometry-processing
operators that need the same machinery (ARAP deformation, mesh
parameterization) build on the operators here.

## What "the Laplacian" means here

For a mesh with `n` vertices, the discrete Laplacian is the symmetric `n x n`
matrix `L` with, for each edge `(i, j)` of weight `w_ij`,

- `L[i][j] = L[j][i] = -w_ij` (off-diagonal), and
- `L[i][i] = sum_j w_ij` (diagonal = the vertex's total edge weight).

So **every row sums to zero** (`L * 1 = 0`, the constant vector is in the
null space) and `L` is symmetric. Two weightings are offered
([`LaplacianWeighting`]):

- **Uniform / umbrella** (`w_ij = 1`): the graph Laplacian — connectivity
  only, ignores geometry. Always positive semidefinite.
- **Cotangent** (`w_ij = (cot α + cot β) / 2`, where `α`, `β` are the angles
  opposite edge `(i, j)` in the one or two triangles that share it): the
  discrete Laplace–Beltrami operator — geometry-aware (this is the one that
  approximates the smooth surface Laplacian). A boundary edge, in only one
  triangle, contributes a single `cot α / 2`.

Polygon faces are fan-triangulated before the cotangent weights are read off,
and the crate's [`crate::mesh`] has no edge→incident-face adjacency, so this
module builds the edge→opposite-vertex map itself from
[`crate::mesh::Mesh::polygons`].

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod laplacian { /* ... */ }
```

### Types

#### Enum `LaplacianWeighting`

Which discrete Laplacian weighting to assemble (enum dispatch, per the
workspace no-trait-objects rule).

```rust
pub enum LaplacianWeighting {
    Uniform,
    Cotangent,
}
```

##### Variants

###### `Uniform`

Graph / umbrella Laplacian — every edge weight is `1`. Depends only on
connectivity, not vertex positions; always positive semidefinite. Cheap
and robust, but not a geometric (Laplace–Beltrami) approximation.

###### `Cotangent`

Cotangent-weighted discrete **Laplace–Beltrami** operator — edge weight
`(cot α + cot β) / 2` from the angles opposite the edge in its incident
triangle(s). Geometry-aware (the correct discretization of the surface
Laplacian). Weights can go negative on very obtuse triangles, so the
bare operator is only positive semidefinite for a Delaunay-ish mesh.

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
    fn clone(self: &Self) -> LaplacianWeighting { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &LaplacianWeighting) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Enum `LaplacianError`

Errors from [`laplacian_smooth`].

```rust
pub enum LaplacianError {
    Assembly,
    NotPositiveDefinite,
}
```

##### Variants

###### `Assembly`

The sparse smoothing matrix could not be assembled (an invalid
row/column index reached the sparse-matrix constructor). Indicates a
caller/topology bug, not a numerical one.

###### `NotPositiveDefinite`

The sparse Cholesky factorization failed because the system `I + λL`
(restricted to the free vertices) is not positive definite. Can happen
with the [`LaplacianWeighting::Cotangent`] weighting on a very obtuse
(non-Delaunay) mesh at a large `lambda`. Try a smaller `lambda`, the
[`LaplacianWeighting::Uniform`] weighting, or a better-conditioned mesh.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn from(source: crate::laplacian::LaplacianError) -> Self { /* ... */ }
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

#### Function `laplacian_triplets`

Assemble the discrete Laplacian `L` of `mesh` under `weighting`, as
`(n, triplets)` — the vertex count and a list of `(row, col, value)` entries
(COO / triplet form) suitable for building a sparse matrix.

`L` is symmetric with zero row sums (see the module docs). Diagonal entries
are summed into one triplet per vertex; off-diagonals are one `(i, j)` and
one `(j, i)` per edge. Vertices with no incident edge (isolated) contribute
no entries (an all-zero row).

The triplet order is unspecified (it comes from a hash map); the assembled
matrix is independent of that order. This is the input a sparse-matrix
builder ([`crate::faer`]) consumes; it is also directly testable by
materializing a small dense matrix (see this module's tests).

```rust
pub fn laplacian_triplets(mesh: &crate::mesh::Mesh, weighting: LaplacianWeighting) -> (usize, Vec<(usize, usize, f64)>) { /* ... */ }
```

#### Function `boundary_vertices`

Flag each vertex that lies on the mesh **boundary** — incident to an edge
used by only one triangle (an open border, e.g. a [`crate::primitives::grid`]
patch). Returns a `bool` per vertex in [`crate::mesh::VertexId`] order.

Boundary vertices are the ones a smoothing operator pins in place so the
surface does not shrink at its border. For a closed mesh (every edge shared
by two triangles) this is all `false`.

```rust
pub fn boundary_vertices(mesh: &crate::mesh::Mesh) -> Vec<bool> { /* ... */ }
```

#### Function `laplacian_smooth`

**Implicit Laplacian smoothing** (mesh fairing): relax vertex positions by
solving `(I + λL) x' = x` once per iteration, with boundary vertices pinned.

# What it computes

Implicit (backward-Euler) smoothing of the surface by the discrete Laplacian
`L` ([`laplacian_triplets`], under `weighting`). Each iteration solves the
sparse SPD system `(I + λL) x' = x` — unconditionally stable for any `lambda`
(unlike explicit `x' = x - λL x`, which diverges for `λ` too large). With the
[`LaplacianWeighting::Cotangent`] weighting this approximates mean-curvature
flow (it denoises toward a smooth surface and gently shrinks); the
[`LaplacianWeighting::Uniform`] weighting smooths by connectivity only.

**Boundary vertices are pinned** (held fixed; see [`boundary_vertices`]) so an
open patch does not shrink at its border. The system is solved on the free
(interior) vertices only, as a symmetric-positive-definite reduced system
(pinned neighbours move to the right-hand side), factorized with `faer`'s
sparse Cholesky. For a closed mesh every vertex is free.

The cotangent weights are **recomputed from the current positions each
iteration**, so the flow tracks the evolving geometry.

# Inputs / units

- `mesh` — source mesh (borrowed, unmodified); topology is preserved, only
  positions change.
- `weighting` — [`LaplacianWeighting::Uniform`] or `Cotangent`.
- `lambda` — smoothing strength (dimensionless, `>= 0`); larger = smoother
  per step. `0` is a no-op; unconditionally stable at any value.
- `iterations` — number of implicit steps (`0` returns a clone).

# Errors

[`LaplacianError::NotPositiveDefinite`] if the reduced system is not SPD
(see that variant), or [`LaplacianError::Assembly`] on an internal indexing
failure.

```rust
pub fn laplacian_smooth(mesh: &crate::mesh::Mesh, weighting: LaplacianWeighting, lambda: f64, iterations: u32) -> Result<crate::mesh::Mesh, LaplacianError> { /* ... */ }
```

#### Function `taubin_smooth`

**Taubin `λ|μ` smoothing** — explicit, shrinkage-free mesh denoising.

# What it computes

Taubin's two-pass low-pass filter (Taubin, *A Signal Processing Approach to
Fair Surface Design*, 1995). Each iteration applies two **explicit**
normalized-Laplacian passes with opposite-sign factors:

- a **shrinking** pass `x ← x + λ Δx` (`λ > 0`), then
- an **un-shrinking** pass `x ← x + μ Δx` (`μ < 0`, with `|μ| > λ`),

where `Δx_i = (Σ_j w_ij (x_j − x_i)) / (Σ_j w_ij)` is the *normalized*
discrete Laplacian (a move toward the weighted one-ring average). Choosing
`μ` slightly more negative than `−λ` gives a filter that removes
high-frequency noise while (unlike plain [`laplacian_smooth`]) **not**
shrinking the surface — the low frequencies that carry the overall shape pass
through almost unchanged. Boundary vertices are pinned.

Unlike [`laplacian_smooth`] this needs **no linear solve** (it is an explicit
filter), so it is cheaper per step but only conditionally stable — keep `λ`
in `(0, 1)` and `μ ∈ (−1, −λ)`.

# Inputs / units

- `mesh` — source mesh (borrowed; topology preserved, positions change).
- `weighting` — [`LaplacianWeighting::Uniform`] (robust; `Σw = degree > 0`)
  or `Cotangent` (geometry-aware; a near-zero weight sum on a degenerate
  one-ring is skipped).
- `lambda` — the shrinking factor, `0 < λ < 1` (e.g. `0.5`).
- `mu` — the un-shrinking factor, `−1 < μ < −λ` (e.g. `−0.53`).
- `iterations` — number of `λ|μ` pairs (`0` is a no-op).

# Verification note

The defining property (shrinkage-free) is checked in the module tests: on a
noisy sphere, Taubin reduces the radial noise while preserving the mean
radius far better than plain Laplacian smoothing shrinks it.

```rust
pub fn taubin_smooth(mesh: &crate::mesh::Mesh, weighting: LaplacianWeighting, lambda: f64, mu: f64, iterations: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `loop_subdivision`

**Loop subdivision** — a smooth subdivision surface for **triangle** meshes.

Blender analogue: the Subdivision-Surface modifier in its triangle path
(Loop is the triangle analogue of Catmull–Clark, which this crate already
provides for quads in [`crate::subdivision`]). Each refinement step splits
every triangle into four and moves vertices toward a limit surface that is
`C²` almost everywhere (`C¹` at extraordinary vertices), producing a
progressively smoother mesh while preserving topology.

## The stencils (Loop, 1987; with Warren's β)

Given a triangle mesh, one step produces:

- **Edge points** — one new vertex per edge. An *interior* edge `(a, b)`
  shared by two triangles with opposite vertices `c, d` gets
  `3/8 (a + b) + 1/8 (c + d)`; a *boundary* edge gets the midpoint
  `1/2 (a + b)`.
- **Repositioned original vertices** — an *interior* vertex `v` of valence
  `n` with neighbours `v_k` moves to `(1 − nβ) v + β Σ_k v_k`, where
  `β = 3/16` for `n = 3` and `β = 3/(8n)` otherwise (Warren's simplified
  weights). A *boundary* vertex moves to `3/4 v + 1/8 (b₁ + b₂)`, using only
  its two boundary neighbours — so the boundary curve refines independently
  of the interior and is preserved.
- **New faces** — each old triangle `(a, b, c)` with edge points
  `e_ab, e_bc, e_ca` becomes four triangles: `(a, e_ab, e_ca)`,
  `(e_ab, b, e_bc)`, `(e_ca, e_bc, c)`, `(e_ab, e_bc, e_ca)`.

Non-triangular input faces are fan-triangulated first. Boundary edges (used
by a single triangle) are detected from the face soup, as elsewhere in the
crate.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod loop_subdivision { /* ... */ }
```

### Functions

#### Function `loop_subdivide`

Apply `iterations` steps of **Loop subdivision** to `mesh`, returning the
refined triangle mesh.

Each step quadruples the triangle count and smooths toward the Loop limit
surface; `iterations = 0` fan-triangulates and returns (a no-op-shaped
clone). Topology (Euler characteristic, boundary loops) is preserved. Input
faces are triangulated first, so the output is always a triangle mesh.

```rust
pub fn loop_subdivide(mesh: &crate::mesh::Mesh, iterations: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `math`

Minimal pure-Rust vector math for mesh authoring.

This module deliberately reimplements only the tiny slice of vector algebra
the mesh layer needs, rather than pulling in a linear-algebra crate. Blender
uses its own `blenlib` `BLI_math_vector` routines for the same reason: mesh
topology work needs 3-component positions, dot/cross products, and lengths —
nothing that justifies a heavy dependency. Keeping it dependency-free also
keeps the crate trivially Android-buildable (no BLAS, no C toolchain).

If richer linear algebra is ever needed, this is the single place to swap in
a pure-Rust crate such as `glam` (add it to the workspace `[dependencies]`
first). Positions are plain `f64` in model space; **no physical units are
attached here** — a mesh is dimensionless geometry until an [`crate::export`]
bridge assigns it a length unit for a solver.

```rust
pub mod math { /* ... */ }
```

### Types

#### Struct `Vec3`

A 3-component vector in model space, stored as `f64` components.

Used both for vertex positions and for direction vectors (normals, offsets).
Components are dimensionless model-space coordinates; the consuming solver
assigns a length unit at export time. `Vec3` is `Copy`, so it is passed by
value throughout the mesh layer (no borrows, per the workspace no-lifetimes
rule).

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
| `x` | `f64` | X component (model-space coordinate or direction). |
| `y` | `f64` | Y component (model-space coordinate or direction). |
| `z` | `f64` | Z component (model-space coordinate or direction). |

##### Implementations

###### Methods

- ```rust
  pub const fn new(x: f64, y: f64, z: f64) -> Self { /* ... */ }
  ```
  Construct a vector from explicit components.

- ```rust
  pub fn add(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Component-wise sum, `self + other`.

- ```rust
  pub fn sub(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Component-wise difference, `self - other`.

- ```rust
  pub fn scale(self: Self, s: f64) -> Vec3 { /* ... */ }
  ```
  Scale every component by `s`.

- ```rust
  pub fn dot(self: Self, other: Vec3) -> f64 { /* ... */ }
  ```
  Euclidean dot product `self · other`.

- ```rust
  pub fn cross(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Right-handed cross product `self × other`.

- ```rust
  pub fn length(self: Self) -> f64 { /* ... */ }
  ```
  Euclidean length (L2 norm), `sqrt(self · self)`.

- ```rust
  pub fn normalize(self: Self) -> Vec3 { /* ... */ }
  ```
  Unit vector in the same direction.

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
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
## Module `mesh`

BMesh-inspired **index-based half-edge** mesh topology.

This is the core data structure every other module operates on. It mirrors
the four-element model of Blender's **BMesh** (`source/blender/bmesh`):

| This crate | Blender BMesh | Role |
|---|---|---|
| [`Vertex`] | `BMVert` | a point in space |
| [`Edge`] | `BMEdge` | an undirected connection between two vertices |
| [`Loop`] | `BMLoop` | one corner of one face — a *directed* half-edge |
| [`Face`] | `BMFace` | a polygon, defined by its ring of loops |

The key idea borrowed from BMesh is the **loop**: a face is not stored as a
list of vertex indices but as a cycle of [`Loop`] records, each of which
knows its vertex, its edge, its face, and its `next`/`prev` neighbours
around the face. Two loops that share an edge but belong to different faces
are the two "half-edges" of that edge. This is what makes adjacency queries
(walk a face, walk around a vertex) O(1) instead of a search.

## No pointers, no lifetimes — indices only

Blender's C uses raw pointers between `BMVert`/`BMEdge`/`BMLoop`/`BMFace`.
The workspace design rules forbid `&'a`-linked graph nodes, so every link
here is a **newtype index** ([`VertexId`], [`EdgeId`], [`LoopId`],
[`FaceId`]) into one of the `Vec`s inside [`Mesh`]. This is the
`CellId(usize)`-into-a-`Vec` pattern the workspace `CLAUDE.md` prescribes.

## What is and is not implemented at scaffold stage

Implemented: construction ([`Mesh::add_vertex`], [`Mesh::add_face`] with
automatic edge deduplication) and read-only queries (element counts,
[`Mesh::face_vertices`], [`Mesh::euler_characteristic`]). This is enough for
the [`crate::primitives`] generators to build valid closed meshes and for
their tests to check `V - E + F`.

Not yet implemented: the full radial-cycle links around an edge (BMesh's
`radial_next`/`radial_prev`, needed to enumerate *all* faces on an edge for
non-manifold meshes), Euler operators (split/join), and per-element custom
data layers. Those are tracked in [`crate::ops`] and the crate's beads.

```rust
pub mod mesh { /* ... */ }
```

### Types

#### Struct `VertexId`

Index of a [`Vertex`] within a [`Mesh`]'s vertex array.

```rust
pub struct VertexId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

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
    fn clone(self: &Self) -> VertexId { /* ... */ }
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
    fn cmp(self: &Self, other: &VertexId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &VertexId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &VertexId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
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
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Struct `EdgeId`

Index of an [`Edge`] within a [`Mesh`]'s edge array.

```rust
pub struct EdgeId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

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
    fn clone(self: &Self) -> EdgeId { /* ... */ }
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
    fn cmp(self: &Self, other: &EdgeId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &EdgeId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &EdgeId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
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
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Struct `LoopId`

Index of a [`Loop`] within a [`Mesh`]'s loop array.

```rust
pub struct LoopId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

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
    fn clone(self: &Self) -> LoopId { /* ... */ }
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
    fn cmp(self: &Self, other: &LoopId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LoopId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &LoopId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
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
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Struct `FaceId`

Index of a [`Face`] within a [`Mesh`]'s face array.

```rust
pub struct FaceId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

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
    fn clone(self: &Self) -> FaceId { /* ... */ }
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
    fn cmp(self: &Self, other: &FaceId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FaceId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &FaceId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
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
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Struct `Vertex`

A mesh vertex: a point in model space plus one incident loop (BMesh `BMVert`).

```rust
pub struct Vertex {
    pub position: crate::math::Vec3,
    pub loop_id: Option<LoopId>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `crate::math::Vec3` | Position in dimensionless model space (see [`crate::math`]). |
| `loop_id` | `Option<LoopId>` | One [`Loop`] that starts at this vertex, or `None` for an isolated<br>vertex not yet used by any face. A full BMesh stores the disk cycle of<br>all incident edges; this scaffold keeps just a single representative. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Vertex { /* ... */ }
    ```

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
#### Struct `Edge`

An undirected edge between two vertices (BMesh `BMEdge`).

```rust
pub struct Edge {
    pub verts: [VertexId; 2],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `verts` | `[VertexId; 2]` | The two endpoint vertices. Order is the order the edge was first<br>created; [`Mesh::add_face`] treats `[a, b]` and `[b, a]` as the same<br>edge when deduplicating. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Edge { /* ... */ }
    ```

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
#### Struct `Loop`

One corner of one face — a directed half-edge (BMesh `BMLoop`).

A [`Face`] owns a cyclic list of loops. Following [`Loop::next`] repeatedly
walks the face's boundary counter-clockwise (as wound at construction) and
returns to the start after [`Face::len`] steps.

```rust
pub struct Loop {
    pub vert: VertexId,
    pub edge: EdgeId,
    pub face: FaceId,
    pub next: LoopId,
    pub prev: LoopId,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vert` | `VertexId` | The vertex this loop-corner is anchored at (the *from* vertex of the<br>directed half-edge). |
| `edge` | `EdgeId` | The undirected [`Edge`] this loop runs along (from [`Loop::vert`] to the<br>next loop's vertex). |
| `face` | `FaceId` | The [`Face`] this loop belongs to. |
| `next` | `LoopId` | The next loop counter-clockwise around [`Loop::face`]. |
| `prev` | `LoopId` | The previous loop counter-clockwise around [`Loop::face`]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Loop { /* ... */ }
    ```

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
#### Struct `Face`

A polygon face, stored as an entry point into its ring of loops (BMesh `BMFace`).

```rust
pub struct Face {
    pub loop_start: LoopId,
    pub len: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `loop_start` | `LoopId` | Any one loop on this face's boundary; walk [`Loop::next`] from here to<br>enumerate the whole face. |
| `len` | `usize` | Number of sides (vertices == edges == loops) on this face. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Face { /* ... */ }
    ```

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
#### Struct `Mesh`

An index-based half-edge mesh: the container all elements live in.

Build one with [`Mesh::new`], add geometry with [`Mesh::add_vertex`] and
[`Mesh::add_face`], then query it. All connectivity is by index into the
four `Vec`s below, so a `Mesh` is `Clone` and contains no borrows.

```rust
pub struct Mesh {
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
  Create an empty mesh with no vertices, edges, loops, or faces.

- ```rust
  pub fn add_vertex(self: &mut Self, position: Vec3) -> VertexId { /* ... */ }
  ```
  Add an isolated vertex at `position` and return its [`VertexId`].

- ```rust
  pub fn add_face(self: &mut Self, verts: &[VertexId]) -> FaceId { /* ... */ }
  ```
  Add a polygon face through the given vertices, in boundary order.

- ```rust
  pub fn vertex_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of vertices (`V`).

- ```rust
  pub fn edge_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of unique edges (`E`).

- ```rust
  pub fn loop_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of loops (directed half-edge corners) — equals the sum of all

- ```rust
  pub fn face_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of faces (`F`).

- ```rust
  pub fn vertex(self: &Self, id: VertexId) -> Option<&Vertex> { /* ... */ }
  ```
  Read-only access to a vertex by id, or `None` if out of range.

- ```rust
  pub fn edge(self: &Self, id: EdgeId) -> Option<&Edge> { /* ... */ }
  ```
  Read-only access to an edge by id, or `None` if out of range.

- ```rust
  pub fn loop_at(self: &Self, id: LoopId) -> Option<&Loop> { /* ... */ }
  ```
  Read-only access to a loop by id, or `None` if out of range.

- ```rust
  pub fn face(self: &Self, id: FaceId) -> Option<&Face> { /* ... */ }
  ```
  Read-only access to a face by id, or `None` if out of range.

- ```rust
  pub fn face_vertices(self: &Self, face: FaceId) -> Vec<VertexId> { /* ... */ }
  ```
  The vertices of a face, in boundary (loop) order.

- ```rust
  pub fn euler_characteristic(self: &Self) -> i64 { /* ... */ }
  ```
  The Euler characteristic `chi = V - E + F`.

- ```rust
  pub fn positions(self: &Self) -> Vec<Vec3> { /* ... */ }
  ```
  All vertex positions in [`VertexId`] order (`positions()[i]` is the

- ```rust
  pub fn polygons(self: &Self) -> Vec<Vec<VertexId>> { /* ... */ }
  ```
  Every face as its ring of [`VertexId`]s, in [`FaceId`] order

- ```rust
  pub fn from_polygons(positions: &[Vec3], faces: &[Vec<usize>]) -> Mesh { /* ... */ }
  ```
  Rebuild a [`Mesh`] from a positions array and a list of faces, each face

- ```rust
  pub fn face_normal(self: &Self, face: FaceId) -> Vec3 { /* ... */ }
  ```
  Unit outward normal of a face by **Newell's method**.

- ```rust
  pub fn face_centroid(self: &Self, face: FaceId) -> Vec3 { /* ... */ }
  ```
  Arithmetic mean (centroid) of a face's vertex positions.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Mesh { /* ... */ }
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
    fn default() -> Mesh { /* ... */ }
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
## Module `parameterize`

Planar **mesh parameterization** — flatten a disk-topology surface patch to
2D `(u, v)` coordinates by a harmonic / Tutte embedding.

Blender analogue: UV unwrapping (`uvedit` / `bmo_...unwrap`, the "Unwrap"
operator's underlying harmonic map). This is the crate's third sparse-solve
operator; it **reuses the same boundary-pinned SPD reduced system** as
[`crate::laplacian::laplacian_smooth`] — only the right-hand side and the
solution width (2, for `u`/`v`) differ.

## The map

For a mesh that is a **topological disk** (connected, orientable, genus 0,
with a single boundary loop), the parameterization:

1. **pins the boundary loop** to a convex target ([`BoundaryShape`] — a unit
   circle or square) by *arc length* along the loop, then
2. **solves the Laplace equation** `L x = 0` on the interior vertices with
   those boundary values fixed — a harmonic map. This is exactly the
   Dirichlet (grounded) Laplacian solve, `A_ff x_f = -L_fb x_b`, that the
   smoothing operator already assembles.

The [`crate::laplacian::LaplacianWeighting`] chooses the flavour:

- **Uniform** = *Tutte's barycentric embedding*. By Tutte's theorem, pinning
  the boundary of a 3-connected planar graph to a convex polygon yields a
  valid straight-line embedding — **no triangle flips or degeneracies**,
  guaranteed. Always solvable (SPD).
- **Cotangent** = *harmonic map*. Lower angle distortion (nearer conformal),
  but the guarantee is lost on obtuse (non-Delaunay) meshes, where a triangle
  can flip or the system can fail to be positive definite.

## Requirements

The mesh **must be an open disk**: a closed mesh (sphere) has no boundary to
pin and cannot be flattened without a cut; an annulus / disk-with-holes has
more than one boundary loop; a handle body is not genus 0. Each of these is
rejected with a specific [`ParamError`] rather than producing garbage.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod parameterize { /* ... */ }
```

### Types

#### Enum `BoundaryShape`

The convex boundary shape the mesh's border is pinned to.

```rust
pub enum BoundaryShape {
    Circle,
    Square,
}
```

##### Variants

###### `Circle`

The unit circle (radius 1, centred at the origin) — the natural default;
always convex, so Tutte's guarantee holds.

###### `Square`

The unit square `[0, 1]^2`, one quarter of the boundary length per side.

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
    fn clone(self: &Self) -> BoundaryShape { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &BoundaryShape) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Enum `ParamError`

Errors from [`parameterize`].

```rust
pub enum ParamError {
    NoBoundary,
    MultipleBoundaries,
    NonManifoldBoundary,
    NotADisk,
    Assembly,
    NotPositiveDefinite,
}
```

##### Variants

###### `NoBoundary`

The mesh has no boundary (it is closed). A closed surface cannot be
flattened to a disk without first introducing a cut/seam.

###### `MultipleBoundaries`

The mesh has more than one boundary loop (an annulus, or a disk with
holes). Only a single-boundary disk is supported.

###### `NonManifoldBoundary`

A boundary vertex has two outgoing boundary edges (a non-manifold
"bowtie" pinch); the boundary is not a simple loop.

###### `NotADisk`

The mesh is not a genus-0 disk (`Euler characteristic != 1`, e.g. it has
a handle).

###### `Assembly`

The sparse harmonic system could not be assembled.

###### `NotPositiveDefinite`

The sparse Cholesky factorization failed — the interior Laplacian system
is not positive definite (can happen with the cotangent weighting on an
obtuse mesh, or a disconnected interior). Try the uniform weighting.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

#### Function `parameterize`

Parameterize `mesh` (a topological disk) to 2D, returning one `(u, v)` per
vertex in [`crate::mesh::VertexId`] order (`result[i]` is the UV of vertex
`i`).

Boundary vertices land exactly on the [`BoundaryShape`] (by arc length);
interior vertices are the harmonic solution under `weighting` (see the module
docs). Use [`LaplacianWeighting::Uniform`] for a guaranteed flip-free (Tutte)
embedding, or [`LaplacianWeighting::Cotangent`] for lower angle distortion on
a well-shaped mesh.

# Errors

See [`ParamError`] — the mesh must be a single-boundary genus-0 disk.

```rust
pub fn parameterize(mesh: &crate::mesh::Mesh, weighting: crate::laplacian::LaplacianWeighting, boundary: BoundaryShape) -> Result<Vec<(f64, f64)>, ParamError> { /* ... */ }
```

#### Function `flatten_to_plane`

Flatten `mesh` into the `z = 0` plane using its [`parameterize`] UVs: a new
mesh with the same faces but positions `(u, v, 0)`. Convenience wrapper when
a 2D *mesh* (rather than a UV list) is wanted.

```rust
pub fn flatten_to_plane(mesh: &crate::mesh::Mesh, weighting: crate::laplacian::LaplacianWeighting, boundary: BoundaryShape) -> Result<crate::mesh::Mesh, ParamError> { /* ... */ }
```

## Module `modifiers`

Non-destructive **modifier stack** (Blender's `modifiers/intern/MOD_*`).

The [`Modifier`] enum and the [`ModifierStack`] that evaluates an ordered
list of them are real, compile, and produce geometry. Each modifier is a
pure function over the **polygon-soup** view of a mesh
([`Mesh::positions`] + [`Mesh::polygons`]): it reads the base mesh, computes
new positions and faces, and rebuilds a fresh [`Mesh`] through
[`Mesh::from_polygons`] (which recomputes edge dedup and loop wiring for
free). The base mesh is never mutated — that is the "non-destructive"
property that distinguishes a modifier from a [`crate::ops`] operator.

## Modifier stack vs. operators

A Blender **modifier** is *non-destructive*: it sits in an ordered stack on
an object and recomputes derived geometry from the original mesh every time,
leaving the base mesh untouched. That is the distinction from
[`crate::ops`], whose operators destructively edit a mesh in place. The
stack is evaluated top-to-bottom by [`ModifierStack::evaluate`].

## The modifiers (Blender analogue in parentheses)

- [`Modifier::Subsurf`] — Catmull-Clark subdivision surface at a view level
  (`MOD_subsurf`, backed by OpenSubdiv upstream). Forwards to the locked
  [`crate::subdivision::catmull_clark`] contract.
- [`Modifier::Mirror`] — mirror across one or more axis planes with a seam
  weld (`MOD_mirror`).
- [`Modifier::Array`] — repeat the mesh in a regular relative-offset pattern
  (`MOD_array`).

## Units

All coordinates are dimensionless model-space lengths (see [`crate::math`]);
the [`Modifier::Array`] `offset` is a *relative* offset expressed in
multiples of the input mesh's bounding-box extent along each axis, exactly
like Blender's Array modifier "Relative Offset" factor.

```rust
pub mod modifiers { /* ... */ }
```

### Types

#### Enum `ModifierError`

Errors returned while evaluating a [`Modifier`] or a [`ModifierStack`].

```rust
pub enum ModifierError {
    NotImplemented(&'static str),
}
```

##### Variants

###### `NotImplemented`

A modifier is scaffolded but its algorithm is not implemented yet.

Retained for forward compatibility (new modifier variants may land as
stubs); the three current variants — [`Modifier::Subsurf`],
[`Modifier::Mirror`], [`Modifier::Array`] — are all implemented and do
not return this.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Struct `MirrorAxes`

Which axis planes a [`Modifier::Mirror`] reflects across.

Each `bool` enables reflecting across the plane orthogonal to that axis; set
more than one to mirror sequentially (X, then Y, then Z).

```rust
pub struct MirrorAxes {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `bool` | Mirror across the YZ plane (reflect the X coordinate). |
| `y` | `bool` | Mirror across the XZ plane (reflect the Y coordinate). |
| `z` | `bool` | Mirror across the XY plane (reflect the Z coordinate). |

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
    fn clone(self: &Self) -> MirrorAxes { /* ... */ }
    ```

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
    fn default() -> MirrorAxes { /* ... */ }
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
    fn eq(self: &Self, other: &MirrorAxes) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Enum `Modifier`

A closed set of non-destructive modifiers.

```rust
pub enum Modifier {
    Subsurf {
        levels: u32,
    },
    Mirror {
        axes: MirrorAxes,
    },
    Array {
        count: u32,
        offset: [f64; 3],
    },
}
```

##### Variants

###### `Subsurf`

Catmull-Clark subdivision to `levels` refinement passes.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `levels` | `u32` | Number of subdivision levels (viewport render level). `0` returns<br>the input unchanged; each level quadruples the face count of a<br>quad mesh. |

###### `Mirror`

Mirror the mesh across the selected axis planes and weld the seam.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `axes` | `MirrorAxes` | Which axis planes to reflect across. |

###### `Array`

Repeat the mesh `count` times, each copy offset by a multiple of the
mesh's bounding-box extent along each axis (relative-offset array).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `count` | `u32` | Number of copies including the original (`>= 1`; `0` is treated as<br>`1`). |
| `offset` | `[f64; 3]` | Per-axis relative offset. Copy `k` is translated by<br>`k * offset[axis] * bbox_size[axis]`, where `bbox_size` is the<br>input mesh's bounding-box extent along that axis. |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, input: &Mesh) -> Result<Mesh, ModifierError> { /* ... */ }
  ```
  Evaluate this modifier against `input`, returning the derived mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Modifier { /* ... */ }
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
#### Struct `ModifierStack`

An ordered, non-destructive stack of [`Modifier`]s applied to a base mesh.

Mirrors Blender's per-object modifier stack: the base mesh is kept, and
[`ModifierStack::evaluate`] folds each modifier in order to produce the
final derived mesh.

```rust
pub struct ModifierStack {
    pub modifiers: Vec<Modifier>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `modifiers` | `Vec<Modifier>` | The modifiers, evaluated first-to-last (top-to-bottom in Blender's UI). |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  Create an empty stack (evaluates to the input mesh unchanged).

- ```rust
  pub fn push(self: Self, m: Modifier) -> Self { /* ... */ }
  ```
  Append a modifier to the end of the stack (builder style).

- ```rust
  pub fn evaluate(self: &Self, base: &Mesh) -> Result<Mesh, ModifierError> { /* ... */ }
  ```
  Evaluate the whole stack against `base`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ModifierStack { /* ... */ }
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
    fn default() -> ModifierStack { /* ... */ }
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
## Module `ops`

Mesh **operators** — the editing verbs (Blender's `bmesh/operators`, `bmo_*`).

Each operator is a **pure function over the polygon-soup view** of a mesh:
it reads [`Mesh::positions`] + [`Mesh::polygons`], builds a fresh
`positions: Vec<Vec3>` and `faces: Vec<Vec<usize>>`, and rebuilds through
[`Mesh::from_polygons`]. Because the rebuild goes back through
[`Mesh::add_face`], edge deduplication and loop wiring are recomputed for
free, so an operator never touches the half-edge cycles by hand.

## Why an enum, not trait objects

The operator set is closed and known at compile time, so per the workspace
design rules it is a single [`MeshOp`] enum dispatched by `match` — not
`Box<dyn Trait>`. Adding a new operator forces every `match` site to handle
it (exhaustiveness), and rust-analyzer's go-to-definition works on each
variant.

## The operators (Blender analogue in parentheses)

- [`extrude_faces`] / [`MeshOp::Extrude`] — duplicate a face region, offset
  the cap, and wall the boundary edges (`bmo_extrude`, the Extrude Region
  tool).
- [`extrude_edges`] — build a quad off each selected edge and its offset
  copy (the Extrude Edges tool).
- [`subdivide`] / [`MeshOp::Subdivide`] — **simple midpoint** subdivision
  (topological quad split, no smoothing; `bmo_subdivide` with smoothness 0).
  This is distinct from Catmull-Clark, which lives in
  [`crate::subdivision::catmull_clark`] and the non-destructive
  [`crate::modifiers::Modifier::Subsurf`].
- [`bevel_vertices`] / [`bevel_vertices_rounded`] / [`MeshOp::Bevel`] —
  vertex bevel / truncation (`bmo_bevel` in vertex-only mode): a single flat
  chamfer, or a rounded spherical cap for `segments >= 2`.
- [`MeshOp::Boolean`] — CSG union/difference/intersection of two meshes,
  delegated to [`crate::boolean`] (`bmo_boolean`, backed upstream by the
  Manifold library). This is the operator most relevant to feeding
  `outram-mc-libs` CSG geometry.

```rust
pub mod ops { /* ... */ }
```

### Types

#### Enum `MeshOpError`

Errors returned by [`MeshOp::apply`].

```rust
pub enum MeshOpError {
    NotImplemented(&'static str),
    Boolean(crate::boolean::BooleanError),
    Laplacian(crate::laplacian::LaplacianError),
    Arap(crate::arap::ArapError),
    Hull(crate::convex_hull::HullError),
}
```

##### Variants

###### `NotImplemented`

The operator is scaffolded but its algorithm is not implemented yet.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `Boolean`

Propagated from a mesh boolean (crate::boolean) — the operand meshes are
outside the supported restricted case.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::boolean::BooleanError` |  |

###### `Laplacian`

Propagated from Laplacian smoothing (crate::laplacian) — the sparse solve
failed (e.g. a non-positive-definite system).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::laplacian::LaplacianError` |  |

###### `Arap`

Propagated from ARAP deformation (crate::arap) — missing constraints or a
non-positive-definite system.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::arap::ArapError` |  |

###### `Hull`

Propagated from convex-hull construction (crate::convex_hull) — a
degenerate (fewer than four distinct / collinear / coplanar) point set.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::convex_hull::HullError` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn from(source: crate::boolean::BooleanError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: crate::laplacian::LaplacianError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: crate::arap::ArapError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: crate::convex_hull::HullError) -> Self { /* ... */ }
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
#### Enum `BooleanMode`

Boolean CSG mode for [`MeshOp::Boolean`] (mirrors Blender's Boolean modifier).

```rust
pub enum BooleanMode {
    Union,
    Difference,
    Intersect,
}
```

##### Variants

###### `Union`

Keep the volume in either mesh (A ∪ B).

###### `Difference`

Keep the volume of A outside B (A \ B).

###### `Intersect`

Keep the volume common to both (A ∩ B).

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
    fn clone(self: &Self) -> BooleanMode { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &BooleanMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Enum `MeshOp`

A closed set of mesh-editing operators.

Construct a variant with its parameters, then call [`MeshOp::apply`] on a
mesh. Parameters are captured by value (no borrows) so the enum carries no
lifetimes.

```rust
pub enum MeshOp {
    Extrude {
        offset: crate::math::Vec3,
    },
    Subdivide {
        iterations: u32,
    },
    Bevel {
        width: f64,
        segments: u32,
    },
    Boolean {
        other: crate::mesh::Mesh,
        mode: BooleanMode,
    },
    Smooth {
        weighting: crate::laplacian::LaplacianWeighting,
        lambda: f64,
        iterations: u32,
    },
    Taubin {
        weighting: crate::laplacian::LaplacianWeighting,
        lambda: f64,
        mu: f64,
        iterations: u32,
    },
    Arap {
        handles: Vec<(crate::mesh::VertexId, crate::math::Vec3)>,
        iterations: u32,
    },
    Decimate {
        target_faces: usize,
    },
    LoopSubdivide {
        iterations: u32,
    },
    ConvexHull,
    Weld {
        distance: f64,
    },
    FillHoles,
    Solidify {
        thickness: f64,
    },
    RecalculateNormals,
    Triangulate,
    Inset {
        amount: f64,
    },
    Bisect {
        point: crate::math::Vec3,
        normal: crate::math::Vec3,
    },
    BevelEdges {
        width: f64,
    },
}
```

##### Variants

###### `Extrude`

Extrude **the whole mesh's faces** along `offset` (direction and distance
combined). [`MeshOp::apply`] runs [`extrude_faces`] over every face; use
[`extrude_faces`] directly to extrude a chosen face region.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `offset` | `crate::math::Vec3` | Model-space translation applied to the newly created cap geometry. |

###### `Subdivide`

Apply `iterations` rounds of **simple midpoint** subdivision (topological
quad split, no smoothing — existing vertices stay put).

This is *not* Catmull-Clark: for a smoothed subdivision surface use the
non-destructive [`crate::modifiers::Modifier::Subsurf`] or the direct
[`crate::subdivision::catmull_clark`]. See [`subdivide`] for the rules.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `u32` | Number of refinement passes (each roughly quadruples face count);<br>`0` is a no-op clone. |

###### `Bevel`

Bevel (truncate) every vertex by `width` model-space units.

Dispatches to [`bevel_vertices_rounded`]: `segments <= 1` is the single
flat chamfer ([`bevel_vertices`], the polyhedral truncation);
`segments >= 2` rounds each cut corner into a spherical cap with
`segments - 1` intermediate rings.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `width` | `f64` | Bevel width in model-space units (clamped per-edge to half the edge<br>length). |
| `segments` | `u32` | Number of segments across the bevel: `1` = a single flat chamfer,<br>`>= 2` = a rounded spherical cap with `segments - 1` intermediate<br>rings (see [`bevel_vertices_rounded`]). |

###### `Boolean`

Combine the target mesh with `other` under a [`BooleanMode`], delegated to
[`crate::boolean::boolean`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `other` | `crate::mesh::Mesh` | The second operand mesh (owned by value — no lifetimes). |
| `mode` | `BooleanMode` | Union / difference / intersection. |

###### `Smooth`

**Implicit Laplacian smoothing** (mesh fairing), delegated to
[`crate::laplacian::laplacian_smooth`]. Solves `(I + λL) x' = x` per
iteration with boundary vertices pinned; unconditionally stable.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `weighting` | `crate::laplacian::LaplacianWeighting` | Uniform (umbrella) or cotangent (Laplace–Beltrami) weighting. |
| `lambda` | `f64` | Smoothing strength per step (`>= 0`; `0` is a no-op). |
| `iterations` | `u32` | Number of implicit steps (`0` is a no-op). |

###### `Taubin`

**Taubin `λ|μ` smoothing** (explicit, shrinkage-free denoising), delegated
to [`crate::laplacian::taubin_smooth`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `weighting` | `crate::laplacian::LaplacianWeighting` | Uniform or cotangent weighting. |
| `lambda` | `f64` | Shrinking factor `0 < λ < 1`. |
| `mu` | `f64` | Un-shrinking factor `−1 < μ < −λ`. |
| `iterations` | `u32` | Number of `λ|μ` iteration pairs. |

###### `Arap`

**As-Rigid-As-Possible deformation**, delegated to
[`crate::arap::arap_deform`]. Deforms the mesh to meet the `handles`
(vertex → target) while keeping one-rings maximally rigid.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `handles` | `Vec<(crate::mesh::VertexId, crate::math::Vec3)>` | Handle constraints: each `(vertex, target position)`. |
| `iterations` | `u32` | Number of local/global ARAP iterations. |

###### `Decimate`

**QEM mesh decimation** (quadric-error-metric simplification), delegated
to [`crate::decimate::decimate`]. Reduces the mesh to roughly
`target_faces` triangles.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `target_faces` | `usize` | The goal triangle count (a lower-bound target). |

###### `LoopSubdivide`

**Loop subdivision** (smooth triangle subdivision surface), delegated to
[`crate::loop_subdivision::loop_subdivide`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `u32` | Number of refinement steps (each quadruples the triangle count). |

###### `ConvexHull`

Replace the mesh with the **convex hull of its vertices**, delegated to
[`crate::convex_hull::convex_hull`].

###### `Weld`

**Weld / remove-doubles**: merge vertices closer than `distance` into
one, delegated to [`crate::weld::weld`]. `distance = 0` welds only
bit-identical duplicates (a safe no-op otherwise).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `distance` | `f64` | Euclidean merge tolerance in mesh units (`0` = exact duplicates only). |

###### `FillHoles`

**Fill holes**: cap every open boundary loop with a centroid triangle
fan, delegated to [`crate::fill_holes::fill_holes`]. A no-op on an
already-closed mesh.

###### `Solidify`

**Solidify**: extrude the surface into a closed shell of the given
`thickness`, delegated to [`crate::solidify::solidify`]. An open surface
becomes a slab; a closed surface becomes a hollow double shell.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `thickness` | `f64` | Shell thickness in mesh units; the inner shell is offset inward. |

###### `RecalculateNormals`

**Recalculate normals outside**: make the winding globally consistent and
outward-facing, delegated to
[`crate::recalc_normals::recalculate_normals`]. Repairs an
inconsistently-wound polygon soup.

###### `Triangulate`

**Triangulate**: fan-triangulate every face into triangles, delegated to
[`crate::triangulate::triangulate`]. Produces a triangle-only mesh for
the operators/bridges that require one.

###### `Inset`

**Inset faces**: replace each face with a shrunk inner copy plus a ring
of bridging quads, delegated to [`crate::inset::inset_faces`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `amount` | `f64` | Fraction each corner moves toward its face centroid, in `(0, 1)`. |

###### `Bisect`

**Bisect**: cut the mesh by a plane and keep the `normal`-negative half,
delegated to [`crate::bisect::bisect`]. The cut is left open (cap it with
[`MeshOp::FillHoles`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `point` | `crate::math::Vec3` | A point the cutting plane passes through. |
| `normal` | `crate::math::Vec3` | The plane normal; the kept half is where `normal · (x − point) <= 0`. |

###### `BevelEdges`

**Edge bevel**: chamfer every edge by `width`, delegated to
[`crate::edge_bevel::bevel_edges`]. Distinct from [`MeshOp::Bevel`], which
truncates *vertices*.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `width` | `f64` | Distance each face is cut back from its edges (mesh units). |

##### Implementations

###### Methods

- ```rust
  pub fn apply(self: &Self, mesh: Mesh) -> Result<Mesh, MeshOpError> { /* ... */ }
  ```
  Apply this operator to `mesh`, returning the edited mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MeshOp { /* ... */ }
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

#### Function `extrude_faces`

Extrude a set of faces: duplicate their vertices, offset the duplicates, cap
the region with the moved copies, and wall the boundary edges.

# What it computes

For the selected region (the faces named in `faces`, given as [`FaceId`]s):

1. **Cap.** Every vertex used by a selected face is duplicated and the copy
   is translated by `offset`. The selected faces are removed and replaced by
   "cap" faces on the moved copies, preserving winding.
2. **Side walls.** Each *boundary edge* of the region — an edge used by
   exactly one selected face — gets a side quad `[a, b, b', a']` connecting
   the old edge `a→b` (in that face's winding) to its moved copy `a'→b'`.
   The winding is chosen so the wall's outward normal points away from the
   region interior. Interior edges (shared by two selected faces) get no
   wall.
3. **Rest of the mesh.** Vertices and faces not in the selection are kept
   unchanged.

A single-quad selection therefore becomes a "cup": 4 side walls + 1 cap,
with the original quad's footprint left open.

# Inputs / units

- `mesh` — the source mesh (borrowed, unmodified).
- `faces` — the [`FaceId`]s to extrude; out-of-range ids are ignored.
- `offset` — model-space translation applied to the cap (dimensionless
  model-space units; direction and distance combined).

# Note

Extruding a *closed* region (every edge interior, e.g. all faces of a closed
solid) produces no walls and leaves the original vertices unreferenced by
the new topology — a whole-mesh extrude of a closed solid is a degenerate
case, not the intended use. Extrude an open region (a grid, a face patch).

```rust
pub fn extrude_faces(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId], offset: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `extrude_edges`

Extrude a set of edges: build one quad off each edge and its offset copy.

For each [`EdgeId`] in `edges`, the two endpoints are duplicated (shared
across edges that touch the same vertex) and translated by `offset`, then a
quad `[a, b, b', a']` is added spanning the original edge `a–b` and its moved
copy `a'–b'`. The original faces are kept; the new quads are appended. This
is the secondary, deliberately simple edge-extrude — it does not attempt to
reason about which side the wall should face.

# Inputs / units

- `mesh` — source mesh (borrowed, unmodified).
- `edges` — [`EdgeId`]s to extrude; out-of-range ids are ignored.
- `offset` — model-space translation of the copies (dimensionless model
  units).

```rust
pub fn extrude_edges(mesh: &crate::mesh::Mesh, edges: &[crate::mesh::EdgeId], offset: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `subdivide`

Simple midpoint subdivision — split every face into quads, no smoothing.

# What it computes

This is **topological** subdivision (Blender's Subdivide with smoothness 0),
distinct from Catmull-Clark ([`crate::subdivision::catmull_clark`] /
[`crate::modifiers::Modifier::Subsurf`]): new points are placed by plain
linear interpolation and existing vertices are **not** moved, so the surface
keeps its exact shape while gaining resolution.

Each iteration, for every `n`-gon face:

- add the midpoint of each of its edges (deduplicated across faces by a
  canonical key on the sorted endpoint-index pair, so a shared edge yields a
  single shared midpoint), and
- add the face centroid (the vertex mean),

then replace the face with `n` quads, one per corner `v[i]`, wound
`[centroid, midpoint(edge into v[i]), v[i], midpoint(edge out of v[i])]` —
which preserves the original winding.

# Counts per iteration

For a closed genus-0 mesh with `V, E, F`, one pass yields
`V' = V + E + F`, `F' = sum of face side-counts` (`4F` when all faces are
quads), and `chi = 2` is preserved. E.g. a cube (`V=8, E=12, F=6`) becomes
`V=26, F=24, chi=2` after one pass.

# Inputs

- `mesh` — source mesh (borrowed, unmodified).
- `iterations` — number of passes; `iterations == 0` returns a clone.

```rust
pub fn subdivide(mesh: &crate::mesh::Mesh, iterations: u32) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `bevel_vertices`

Vertex bevel / truncation — cut off every vertex, replacing it with a face.

# What it computes

This is the vertex-only bevel (a single chamfer per vertex, the polyhedral
*truncation* operation). For each vertex `V` and each incident edge `e`, one
**edge-point** is placed at
`V + w_e * normalize(other_end(e) - V)`, where `w_e = min(width, L_e / 2)`
clamps the offset so the two edge-points on an edge of length `L_e` never
pass its midpoint. That edge-point is shared by the two faces on `e`.

The result mesh contains **only** the edge-points (every original vertex is
cut away):

- **Truncated faces.** Each original `n`-gon becomes a `2n`-gon: every
  corner `V` is replaced, in winding order, by its two edge-points — the one
  on the edge entering `V` then the one on the edge leaving `V`.
- **Vertex faces.** Each original vertex contributes one new face joining its
  edge-points in angular order around `V` (the order is read off the fan of
  faces around `V`), wound so its normal points outward.

# Counts (verification)

Truncating a cube (`V=8, E=12, F=6`) gives the **truncated cube**:
`V=24` (two edge-points per edge), `E=36`, `F=14` (6 octagons + 8 triangles),
`chi=2`. This is asserted in the module tests.

# Inputs / units

- `mesh` — source mesh (borrowed, unmodified). Assumed manifold and
  orientable (as produced by [`crate::primitives`]); the angular ordering of
  a vertex face is derived from the face fan around each vertex.
- `width` — chamfer width in dimensionless model-space units, clamped
  per-edge to at most half the edge length. `width <= 0` collapses the
  edge-points onto the original vertices (a no-op-shaped degenerate result).

```rust
pub fn bevel_vertices(mesh: &crate::mesh::Mesh, width: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `bevel_vertices_rounded`

Rounded (multi-segment) vertex bevel — truncate every vertex and replace the
flat chamfer with a **spherical cap** of `segments` bands.

# What it computes

The truncation (edge-points, widened faces) is identical to
[`bevel_vertices`]; only the per-vertex cap differs. For `segments <= 1` this
*is* [`bevel_vertices`] (one flat face per vertex). For `segments >= 2`, each
vertex `V`'s edge-point ring is domed into a spherical cap on the sphere
centred at `V` of radius `~width`:

- the ring of edge-points is **band 0** (shared with the truncated faces, so
  the result stays watertight);
- `segments - 1` **intermediate rings** are inserted by spherical-linear
  interpolation ([`slerp`]) of each edge-point's direction toward the cap
  **apex** `V + R * n`, where `n` is the outward vertex normal (sum of
  incident face normals) and `R` the mean edge-point distance; the radius is
  interpolated linearly so band 0 stays exactly on the (possibly
  per-edge-clamped) edge-points;
- the cap closes with a triangle fan to the apex.

The corner therefore becomes a smooth convex spherical cap of radius `~width`
about the original vertex — a *rounded* bevel. This is a mesh-authoring
rounding (a spherical cap through the edge-points), **not** a CAD fillet
tangent to the adjacent faces; the apex bulges toward the original corner
along the outward normal. Higher `segments` gives a smoother cap.

# Counts (verification)

For a cube (`V=8, E=12, F=6`, every vertex degree 3) at `segments = s >= 2`:
`V = 24s + 8`, `F = 24s + 6` (6 octagons + `3s` cap faces per vertex),
`E = 48s + 12`, `chi = 2`. Asserted in the module tests.

# Inputs / units

- `mesh` — source mesh (borrowed, unmodified); assumed manifold/orientable.
- `width` — chamfer width (model-space units, clamped per-edge to half the
  edge length), same as [`bevel_vertices`].
- `segments` — bevel resolution: `0`/`1` = single flat chamfer, `>= 2` =
  rounded cap with `segments - 1` intermediate rings.

```rust
pub fn bevel_vertices_rounded(mesh: &crate::mesh::Mesh, width: f64, segments: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `primitives`

**Real** mesh primitive generators (Blender's "Add Mesh" primitives).

This is the one module in the crate with fully implemented, unit-tested
algorithms — the analogue of Blender's `editors/mesh/editmesh_add.cc` add
operators (Add Cube / UV Sphere / Cylinder / Grid). Each function returns a
valid [`Mesh`] built entirely through the public [`Mesh::add_vertex`] /
[`Mesh::add_face`] API, so edge deduplication and loop wiring come for free.

## Correctness checks

The generators are validated against **Euler's polyhedron formula**
`V - E + F = chi`:

- closed genus-0 solids ([`cube`], [`uv_sphere`], [`cylinder`]) give
  `chi = 2`;
- a flat [`grid`] patch is a topological disc, `chi = 1`.

These are exact topological identities, so the tests assert them exactly
(no tolerance). Vertex/edge/face counts are also checked against the closed
forms derived in each function's doc comment.

## Units

All size/radius/height arguments are dimensionless model-space lengths (see
[`crate::math`]); a length unit is attached only when a mesh is handed to a
solver through an [`crate::export`] bridge.

```rust
pub mod primitives { /* ... */ }
```

### Functions

#### Function `cube`

An axis-aligned cube of side `size`, centred on the origin.

Topology: **8** vertices, **12** edges, **6** quad faces, `chi = 2`. Faces
are wound counter-clockwise as seen from outside, so face normals point
outward. `size` is the full edge length (a `size` of `1.0` spans `-0.5..0.5`
on each axis).

```rust
pub fn cube(size: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `uv_sphere`

A UV (latitude/longitude) sphere of the given `radius`.

`segments` is the number of subdivisions **around** the polar axis
(longitude), `rings` the number of subdivisions **from pole to pole**
(latitude). Both must be `>= 3` / `>= 2` respectively for a sensible sphere;
this matches Blender's Add-UV-Sphere defaults of 32 segments and 16 rings.

Topology (closed, genus-0, `chi = 2`): **2 poles + (rings - 1) * segments**
vertices; the two pole caps are triangle fans (`segments` triangles each)
and the `rings - 2` intermediate bands are quad strips
(`(rings - 2) * segments` quads). Faces are wound counter-clockwise as seen
from outside, so face normals point **outward** (positive enclosed volume) —
consistent with [`cube`] and [`cylinder`].

# Panics

Panics if `segments < 3` or `rings < 2` — too few to close the surface.

```rust
pub fn uv_sphere(segments: usize, rings: usize, radius: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `cylinder`

A closed cylinder of the given `radius` and `height`, axis along Z.

`segments` is the number of sides around the axis (Blender's "Vertices"
field, default 32). The caps are single n-gon faces (Blender's "Ngon"
cap-fill), not triangle fans.

Topology (closed, genus-0, `chi = 2`): **2 * segments** vertices,
**3 * segments** edges, **segments + 2** faces (`segments` side quads plus
two `segments`-gon caps). The cylinder spans `-height/2 .. height/2` on Z.

# Panics

Panics if `segments < 3` — too few sides to enclose a volume.

```rust
pub fn cylinder(segments: usize, radius: f64, height: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `grid`

A flat rectangular grid (subdivided plane) in the Z = 0 plane.

`nx` and `ny` are the number of quad subdivisions along X and Y (each `>= 1`);
`size` is the full side length (the patch spans `-size/2 .. size/2` on both
axes). This is Blender's Add-Grid primitive.

Topology (a topological **disc**, `chi = 1`): **(nx + 1) * (ny + 1)**
vertices and **nx * ny** quad faces. Being an open patch (it has a boundary),
it is *not* closed — that is the intended difference from [`cube`] /
[`cylinder`] / [`uv_sphere`], and its Euler characteristic is 1, not 2.

# Panics

Panics if `nx < 1` or `ny < 1`.

```rust
pub fn grid(nx: usize, ny: usize, size: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `procedural`

Procedural geometry generation — a **Geometry-Nodes-style** node graph
(Blender's `nodes/geometry`, the Geometry Nodes system).

## The concept

Blender's Geometry Nodes builds geometry by evaluating a directed acyclic
graph of nodes: *input* nodes create primitives, *processing* nodes
transform or combine geometry (many wrap the same verbs as [`crate::ops`]),
and an *output* node yields the final mesh. The graph is data, not code, so
a design can be authored, stored, and replayed — which is exactly what an
OUTRAM PARK reactor-geometry generator wants (parametric fuel pins, lattices,
coolant channels driven by a few numeric inputs).

## What is implemented

[`GeometryGraph::evaluate`] is a **real** evaluator. It locates the single
[`GeometryNode::OutputMesh`] node and walks the graph upstream by
[`NodeId`], building a [`Mesh`] bottom-up:

- [`GeometryNode::Primitive`] emits a fresh primitive from [`crate::primitives`];
- [`GeometryNode::Transform`] applies a translation via [`crate::transform::Affine3`];
- [`GeometryNode::Join`] concatenates two meshes (offsetting the second
  operand's face indices by the first's vertex count);
- [`GeometryNode::Subdivide`] delegates to [`crate::subdivision::catmull_clark`];
- [`GeometryNode::Boolean`] delegates to [`crate::boolean::boolean`].

The walk is defensive: an out-of-range [`NodeId`] returns
[`ProceduralError::BadNode`] and a cyclic reference returns
[`ProceduralError::Cycle`] — a malformed graph never panics.

## Why an enum of nodes

The node kinds are a closed set, so [`GeometryNode`] is an enum (no trait
objects), and edges between nodes are **indices** ([`NodeId`]) into the
graph's node `Vec` — the same no-pointers/no-lifetimes discipline as
[`crate::mesh`].

```rust
pub mod procedural { /* ... */ }
```

### Types

#### Enum `ProceduralError`

Errors from evaluating a [`GeometryGraph`].

```rust
pub enum ProceduralError {
    NoOutput,
    BadNode(NodeId),
    Cycle(NodeId),
    Boolean(crate::boolean::BooleanError),
}
```

##### Variants

###### `NoOutput`

The graph has no [`GeometryNode::OutputMesh`] node to read a result from.

###### `BadNode`

A node referenced a [`NodeId`] that is out of range for the graph — a
malformed edge. Carries the offending id.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NodeId` |  |

###### `Cycle`

A cycle was detected while walking upstream from the output — the node
carried is one that was re-entered while already on the current
depth-first path. A valid geometry graph must be acyclic.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NodeId` |  |

###### `Boolean`

A [`GeometryNode::Boolean`] node's underlying CSG operation failed; the
[`crate::boolean::BooleanError`] is propagated unchanged.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::boolean::BooleanError` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn from(source: crate::boolean::BooleanError) -> Self { /* ... */ }
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
#### Struct `NodeId`

Index of a [`GeometryNode`] within a [`GeometryGraph`].

```rust
pub struct NodeId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

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
    fn clone(self: &Self) -> NodeId { /* ... */ }
    ```

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
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodeId) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Enum `PrimitiveKind`

Which primitive an input node emits.

Each variant maps one-to-one onto a generator in [`crate::primitives`]; the
parameters are the same dimensionless model-space lengths those generators
take (see [`crate::math`]).

```rust
pub enum PrimitiveKind {
    Cube {
        size: f64,
    },
    UvSphere {
        segments: usize,
        rings: usize,
        radius: f64,
    },
    Cylinder {
        segments: usize,
        radius: f64,
        height: f64,
    },
}
```

##### Variants

###### `Cube`

A cube of the given side length — see [`crate::primitives::cube`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `size` | `f64` | Full edge length. |

###### `UvSphere`

A UV sphere — see [`crate::primitives::uv_sphere`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `segments` | `usize` | Subdivisions around the polar axis (must be `>= 3`). |
| `rings` | `usize` | Subdivisions pole to pole (must be `>= 2`). |
| `radius` | `f64` | Sphere radius. |

###### `Cylinder`

A closed cylinder with axis along Z — see [`crate::primitives::cylinder`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `segments` | `usize` | Number of sides around the axis (must be `>= 3`). |
| `radius` | `f64` | Cylinder radius. |
| `height` | `f64` | Full height along Z (the cylinder spans `-height/2 .. height/2`). |

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
    fn clone(self: &Self) -> PrimitiveKind { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &PrimitiveKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
#### Enum `GeometryNode`

A node in a procedural geometry graph.

Each node references its inputs by [`NodeId`]. Evaluation performs a
depth-first walk from [`GeometryNode::OutputMesh`] back to the input nodes
(see [`GeometryGraph::evaluate`]).

```rust
pub enum GeometryNode {
    Primitive(PrimitiveKind),
    Transform {
        input: NodeId,
        translate: [f64; 3],
    },
    Join {
        a: NodeId,
        b: NodeId,
    },
    Subdivide {
        input: NodeId,
        levels: u32,
    },
    Boolean {
        a: NodeId,
        b: NodeId,
        mode: crate::ops::BooleanMode,
    },
    OutputMesh {
        input: NodeId,
    },
}
```

##### Variants

###### `Primitive`

Source node: emit a primitive mesh.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PrimitiveKind` |  |

###### `Transform`

Transform the geometry coming from `input` by a uniform translation.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `NodeId` | Upstream node whose geometry is transformed. |
| `translate` | `[f64; 3]` | Model-space translation `[dx, dy, dz]` applied to every vertex. |

###### `Join`

Join two geometry streams into one mesh (concatenate elements). The two
operands keep their own vertices — no welding/deduplication is performed,
so joining two disjoint solids yields a mesh with both, each still a
separate closed surface.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `NodeId` | First upstream node. |
| `b` | `NodeId` | Second upstream node. |

###### `Subdivide`

Refine the geometry from `input` with `levels` rounds of Catmull-Clark
subdivision — see [`crate::subdivision::catmull_clark`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `NodeId` | Upstream node whose geometry is subdivided. |
| `levels` | `u32` | Number of Catmull-Clark refinement passes. |

###### `Boolean`

Combine the geometry from `a` and `b` under a CSG [`crate::ops::BooleanMode`]
— see [`crate::boolean::boolean`]. A failure of the underlying operation
surfaces as [`ProceduralError::Boolean`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `NodeId` | First operand node (the `A` mesh). |
| `b` | `NodeId` | Second operand node (the `B` mesh). |
| `mode` | `crate::ops::BooleanMode` | Union / difference / intersection. |

###### `OutputMesh`

Terminal node: the graph's result is the geometry from `input`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `NodeId` | Upstream node providing the final mesh. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> GeometryNode { /* ... */ }
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
#### Struct `GeometryGraph`

A directed graph of [`GeometryNode`]s that evaluates to a single [`Mesh`].

Nodes are stored in a `Vec` and referenced by [`NodeId`]; add nodes with
[`GeometryGraph::add`], which returns the new node's id for wiring later
nodes to it.

```rust
pub struct GeometryGraph {
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
  Create an empty graph.

- ```rust
  pub fn add(self: &mut Self, node: GeometryNode) -> NodeId { /* ... */ }
  ```
  Add a node and return its [`NodeId`] for wiring downstream nodes.

- ```rust
  pub fn node(self: &Self, id: NodeId) -> Option<&GeometryNode> { /* ... */ }
  ```
  Read-only access to a node by id.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of nodes in the graph.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the graph has no nodes.

- ```rust
  pub fn evaluate(self: &Self) -> Result<Mesh, ProceduralError> { /* ... */ }
  ```
  Evaluate the graph to a final mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> GeometryGraph { /* ... */ }
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
    fn default() -> GeometryGraph { /* ... */ }
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
## Module `recalc_normals`

Recalculate normals — make a mesh's face winding globally consistent and
outward-facing.

This is the pure-Rust analogue of Blender's **Recalculate Normals Outside**
(`mesh.normals_make_consistent`). It repairs a polygon soup whose faces are
wound inconsistently — the common state of an imported or hand-assembled
mesh — so that adjacent faces agree on their shared edges and every
connected component's normals point outward.

# Why this is not just [`crate::boolean_general`]'s orient-outward

`boolean_general` flips a whole mesh by signed-volume sign, but only when it
is **already** consistently wound. This operator does the harder job first:
a breadth-first propagation across the face-adjacency graph that flips any
neighbour disagreeing on a shared edge, turning an inconsistent soup into a
consistently-wound one. Only then is each connected component flipped
outward.

# Algorithm

1. **Adjacency.** Index faces by their undirected edges (sorted vertex
   pair). Two faces sharing an edge are adjacent.
2. **Orientation propagation (BFS).** Seed each connected component with one
   face (unflipped). For a neighbour across a shared edge, the two faces are
   consistent iff they traverse that edge in **opposite** directions; if not,
   the neighbour is marked to flip. Directions compose by XOR, so the
   neighbour's flip is a closed-form function of the seed-relative
   orientation.
3. **Outward pass.** For each connected component, compute the signed volume
   (divergence theorem) of its now-consistent faces; if negative, flip the
   whole component so its normals point outward.

A non-orientable surface (e.g. a Möbius band) cannot be made globally
consistent — the BFS will visit it, but the result is best-effort along the
spanning tree, matching Blender's own behaviour. No `faer`, no external
dependency; Android-safe.

```rust
pub mod recalc_normals { /* ... */ }
```

### Functions

#### Function `recalculate_normals`

Return `mesh` with every face rewound so the surface is consistently wound
and each connected component faces outward (positive enclosed volume).

This is infallible. An already-correct mesh is returned with the same
winding; an inconsistently-wound soup is repaired; a fully-inward mesh is
flipped outward. Winding is the only thing changed — vertex positions and
the face-vertex sets are untouched.

# Examples

```
use outram_blender::{primitives, recalc_normals::recalculate_normals};

// A pristine cube is already outward-wound, so this is a no-op on topology.
let cube = primitives::cube(2.0);
let fixed = recalculate_normals(&cube);
assert_eq!(fixed.vertex_count(), 8);
assert_eq!(fixed.face_count(), 6);
assert_eq!(fixed.euler_characteristic(), 2);
```

```rust
pub fn recalculate_normals(mesh: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

## Module `revolve`

Revolve / spin — sweep a profile polyline around an axis into a surface of
revolution.

This is the pure-Rust analogue of Blender's **Spin**: a profile (an ordered
list of points) is rotated around an arbitrary axis in `segments` steps
through a total `angle`, and consecutive rings are stitched with quads. It
generates the surfaces of revolution a reactor geometry is full of — pipes,
tubes, pressure-vessel walls, cone frusta.

- A **full** sweep (`angle >= 2π`) closes the seam: the last ring wraps back
  onto the first, giving a closed loop of quads (an open-ended tube — cap it
  with [`crate::fill_holes`] for a solid).
- A **partial** sweep leaves the two end rings unstitched (an open strip).

# Profile orientation

The profile is a polyline in space; each point is rotated about the axis.
Neighbouring profile points become the two rails of a quad band, so the
profile's own ordering sets the surface's `v` direction and the sweep sets
its `u` direction. The winding is internally consistent across the whole
surface (fix its outward sense with
[`crate::recalc_normals::recalculate_normals`] if a particular profile winds
it inward).

# Limitation (v1): on-axis profile points

A profile point lying **on** the axis (a pole, radius ≈ 0) is rotated to the
same location on every ring, producing coincident duplicate vertices rather
than a shared pole. For a profile that touches the axis (e.g. a semicircle
swept into a sphere), run [`crate::weld::weld`] afterward to merge the pole
copies. Off-axis profiles need no post-processing. No `faer`, no external
dependency; Android-safe.

```rust
pub mod revolve { /* ... */ }
```

### Functions

#### Function `revolve`

Sweep `profile` around the axis through `axis_point` along `axis_dir` by
`angle` radians in `segments` steps, returning the surface of revolution.

`segments` is the number of quad bands around the sweep (`>= 3` for a full
revolution, `>= 1` for a partial one); `profile` needs at least two points.
An `angle` of `2π` (or more) makes a closed loop; a smaller angle leaves the
ends open. `axis_dir` need not be unit length. This is infallible for valid
inputs; a profile with fewer than two points or `segments == 0` yields an
empty mesh.

# Examples

```
use outram_blender::{revolve::revolve, fill_holes::fill_holes, math::Vec3};
use std::f64::consts::TAU;

// Revolve a vertical segment at radius 1 around Z into a 16-gon tube, then
// cap it into a closed prism.
let profile = [Vec3::new(1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0)];
let tube = revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 16, TAU);
assert_eq!(tube.face_count(), 16);
assert_eq!(fill_holes(&tube).euler_characteristic(), 2);
```

```rust
pub fn revolve(profile: &[crate::math::Vec3], axis_point: crate::math::Vec3, axis_dir: crate::math::Vec3, segments: usize, angle: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `solidify`

Solidify — give a surface thickness by extruding it into a closed shell.

This is the pure-Rust analogue of Blender's **Solidify** modifier
(`MOD_solidify`, simple mode): every vertex is duplicated and offset inward
along its area-weighted vertex normal by `thickness`, the offset copy forms
an **inner shell** with reversed winding (so its normals face the cavity),
and any open boundary edge is bridged to the inner shell by a **rim quad**.

The result depends on whether the input is open or closed:

- An **open** surface (a grid, a patch, a surface with holes) becomes a
  **closed solid slab** of the given thickness — the use case for turning an
  authored boundary surface into something with volume (e.g. a CFD wall).
- A **closed** surface (a cube, a sphere) becomes a **hollow shell** — the
  original outer surface plus a nested, inward-offset inner surface, with a
  cavity between them.

# Winding

The outer shell keeps the input winding (outward normals). The inner shell
reverses it, so its normals point into the cavity (away from the solid
material). Each rim quad traverses the shared boundary edge opposite to the
outer face that owns it, keeping the whole result consistently wound — the
same adjacent-faces-oppose rule used in [`crate::fill_holes`].

# Degenerate normals

A vertex whose incident face normals cancel gets a zero normal (via
[`crate::math::Vec3::normalize`], which returns zero rather than `NaN`), so
its inner copy coincides with the outer vertex instead of flying off to a
non-finite coordinate. No `faer`, no external dependency; Android-safe.

```rust
pub mod solidify { /* ... */ }
```

### Functions

#### Function `solidify`

Extrude `mesh` into a closed shell of the given `thickness`, returning the
solidified mesh.

`thickness` is a length in mesh units; the inner shell is offset inward
(against the outward vertex normals) by this amount. A `thickness` of `0`
places the inner shell exactly on the outer one (a degenerate but valid
mesh). Open boundaries are closed with rim quads; a closed input yields a
hollow double shell.

This is infallible: the result is always a valid mesh.

# Examples

```
use outram_blender::{primitives, solidify::solidify};

// A flat grid (open disk, chi = 1) solidifies into a closed slab (chi = 2).
let grid = primitives::grid(2, 2, 2.0);
assert_eq!(grid.euler_characteristic(), 1);
let slab = solidify(&grid, 0.2);
assert_eq!(slab.euler_characteristic(), 2);
```

```rust
pub fn solidify(mesh: &crate::mesh::Mesh, thickness: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `stl`

STL import / export — the lingua franca for surface-mesh interchange.

STL (STereoLithography) is an unstructured **triangle soup**: a flat list of
independent triangles, each with a facet normal and three vertices, with *no*
shared-vertex topology. It is the format most CAD/mesh tools and the DAGMC /
Monte-Carlo pipelines read and write, so it is the crate's interoperability
bridge alongside the OpenFOAM polyMesh and CSG exporters in
[`crate::export`].

# Writing

[`to_stl_ascii`] / [`to_stl_binary`] fan-triangulate every face and emit one
facet per triangle with an outward normal computed from the triangle
geometry (`(b − a) × (c − a)`, normalized). [`write_stl_ascii`] /
[`write_stl_binary`] are the disk counterparts. Binary is compact and exact
to `f32`; ASCII is human-readable and keeps full `f64` precision.

# Reading

[`from_stl_ascii`] / [`from_stl_binary`] parse a triangle soup into a
[`Mesh`]. Because STL has no shared vertices, the parsed mesh has one vertex
per triangle corner (a cube reads back as 36 vertices, 12 faces) — run
[`crate::weld::weld`] to merge coincident corners into a proper topological
mesh. [`from_stl_bytes`] auto-detects ASCII vs binary by the binary
size invariant, and [`read_stl`] does the same from a file.

No `faer`, no external dependency; Android-safe.

```rust
pub mod stl { /* ... */ }
```

### Types

#### Enum `StlError`

Errors from parsing an STL stream.

```rust
pub enum StlError {
    Truncated(usize),
    BadBinaryLength {
        got: usize,
        expected: usize,
    },
    Parse(String),
    Io(std::io::Error),
}
```

##### Variants

###### `Truncated`

The byte stream is too short to be a valid binary STL (needs at least
the 80-byte header plus the 4-byte triangle count).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `BadBinaryLength`

A binary STL whose length does not match `84 + 50 * triangle_count`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `got` | `usize` | The actual byte length. |
| `expected` | `usize` | The length implied by the header's triangle count. |

###### `Parse`

An ASCII STL with a malformed `vertex` line or a truncated triangle.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Io`

A filesystem error while reading an STL file.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::io::Error` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn from(source: std::io::Error) -> Self { /* ... */ }
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

#### Function `to_stl_ascii`

Fan-triangulate `mesh` and return it as an **ASCII** STL string.

Each triangle is written with an outward facet normal computed from its
geometry. Coordinates keep full `f64` precision (round-trippable). The solid
is named `outram_blender`.

# Examples

```
use outram_blender::{primitives, stl::to_stl_ascii};

let stl = to_stl_ascii(&primitives::cube(2.0));
// A cube's 6 quads fan-triangulate into 12 facets.
assert_eq!(stl.matches("facet normal").count(), 12);
```

```rust
pub fn to_stl_ascii(mesh: &crate::mesh::Mesh) -> String { /* ... */ }
```

#### Function `to_stl_binary`

Fan-triangulate `mesh` and return it as a **binary** STL byte buffer.

Layout: an 80-byte header, a little-endian `u32` triangle count, then 50
bytes per triangle (facet normal + three vertices as `f32`, plus a `u16`
attribute count of `0`). Coordinates are stored to `f32` precision.

```rust
pub fn to_stl_binary(mesh: &crate::mesh::Mesh) -> Vec<u8> { /* ... */ }
```

#### Function `write_stl_ascii`

Write `mesh` to `path` as ASCII STL.

```rust
pub fn write_stl_ascii(mesh: &crate::mesh::Mesh, path: &std::path::Path) -> std::io::Result<()> { /* ... */ }
```

#### Function `write_stl_binary`

Write `mesh` to `path` as binary STL.

```rust
pub fn write_stl_binary(mesh: &crate::mesh::Mesh, path: &std::path::Path) -> std::io::Result<()> { /* ... */ }
```

#### Function `from_stl_ascii`

Parse an **ASCII** STL string into a [`Mesh`] (an unwelded triangle soup).

Facet normals in the file are ignored (recomputed on any re-export). The
result has one vertex per triangle corner; [`crate::weld::weld`] it to
recover shared-vertex topology.

# Errors

[`StlError::Parse`] if a `vertex` line lacks three parseable coordinates or
the vertex count is not a multiple of three.

```rust
pub fn from_stl_ascii(text: &str) -> Result<crate::mesh::Mesh, StlError> { /* ... */ }
```

#### Function `from_stl_binary`

Parse a **binary** STL byte buffer into a [`Mesh`] (an unwelded triangle
soup).

# Errors

[`StlError::Truncated`] if shorter than the 84-byte minimum, or
[`StlError::BadBinaryLength`] if the length does not match the header's
triangle count.

```rust
pub fn from_stl_binary(bytes: &[u8]) -> Result<crate::mesh::Mesh, StlError> { /* ... */ }
```

#### Function `from_stl_bytes`

Parse an STL byte buffer, **auto-detecting** ASCII vs binary.

A stream is treated as binary when its length exactly matches the binary
size invariant `84 + 50 * count` (read from the header); otherwise it is
parsed as ASCII. This is the robust test — an ASCII file that merely starts
with the word `solid` will not accidentally satisfy the binary length.

```rust
pub fn from_stl_bytes(bytes: &[u8]) -> Result<crate::mesh::Mesh, StlError> { /* ... */ }
```

#### Function `read_stl`

Read an STL file from `path`, auto-detecting ASCII vs binary.

```rust
pub fn read_stl(path: &std::path::Path) -> Result<crate::mesh::Mesh, StlError> { /* ... */ }
```

## Module `subdivision`

Catmull-Clark subdivision surface via **local stencils** (no global solve).

Catmull-Clark refinement takes any polygon mesh and produces a smoother,
denser, **all-quad** mesh that converges toward the Catmull-Clark limit
surface as the level increases. This module implements one round of
refinement with the classic three local point stencils and iterates it
`levels` times. Each new point is a fixed affine combination of a small
neighbourhood of the input mesh — there is no linear system to solve.

# The three stencils (one refinement level)

Working from the polygon soup ([`Mesh::positions`] + [`Mesh::polygons`]),
this builds its own undirected-edge adjacency (canonical key = the sorted
pair of endpoint vertex indices) recording, for each edge, its two endpoints
and the faces incident to it (1 incident face = a boundary/crease edge,
2 = interior).

- **Face point** `F_f` — the centroid (vertex average) of face `f`.
- **Edge point**:
  - *interior* edge (2 incident faces): `(v0 + v1 + F_a + F_b) / 4`, the
    average of the two endpoints and the two adjacent face points.
  - *boundary* edge (1 incident face): the endpoint midpoint `(v0 + v1) / 2`
    (crease rule — the boundary curve is refined but not pulled inward).
- **Vertex point** for an old vertex `P` of valence `n` (number of incident
  edges):
  - *interior* vertex (every incident edge interior):
    `(F_avg + 2*R_avg + (n - 3)*P) / n`, where `F_avg` is the average of the
    incident face points and `R_avg` is the average of the incident edge
    **midpoints** (the raw edge midpoints, not the edge points above).
  - *boundary* vertex (at least one incident boundary edge):
    `(m0 + 6*P + m1) / 8`, where `m0`, `m1` are the midpoints of the two
    incident boundary edges (crease rule). This keeps a straight boundary
    run exactly on its line and holds the boundary curve's shape fixed.

# New topology

Each old `n`-gon face emits exactly `n` quads. For every corner vertex `Pi`
of the face, with incoming edge `e_prev` and outgoing edge `e_next` (in the
old face's winding), one quad is emitted:
`[FacePoint_f, EdgePoint(e_prev), VertexPoint(Pi), EdgePoint(e_next)]`,
wound consistently with the old face. Every output face is therefore a quad.

All new points are deduplicated (one face point per face, one edge point per
undirected edge, one vertex point per old vertex), assigned contiguous
indices `[vertex points | edge points | face points]`, and rebuilt through
[`Mesh::from_polygons`] (which recomputes edge dedup and loop wiring).

# Valid inputs and boundary handling

Accepts **any** polygon mesh: triangles, quads, or higher `n`-gons; closed
(e.g. a cube) or open patches with a boundary (e.g. a grid). Boundary edges
and boundary vertices use the crease stencils above, so an open patch stays
a disc (`chi` unchanged) and its boundary loop keeps its shape.

# Limitations

- **Non-manifold edges (>2 incident faces) are not a validated case.** They
  do not panic: an edge with `k >= 2` incident faces uses the generalised
  interior edge point `(v0 + v1 + sum of the k face points) / (2 + k)`, which
  reduces to the standard `/4` rule at `k = 2`. A boundary vertex that does
  not have exactly two incident boundary edges (a dangling or non-manifold
  boundary) is held fixed as a safe fallback. Neither case is covered by the
  V&V tests below.

```rust
pub mod subdivision { /* ... */ }
```

### Functions

#### Function `catmull_clark`

Apply `levels` rounds of Catmull-Clark subdivision to `mesh`.

Returns a new, denser, all-quad [`Mesh`] converging toward the Catmull-Clark
limit surface. Each level applies the face-point / edge-point / vertex-point
stencils documented at the module level ([`crate::subdivision`]) once.

- `levels == 0` returns a plain clone of the input (identical topology and
  positions).
- `levels == 1` on an `n`-gon mesh yields one all-quad mesh where every old
  face has become `n` quads.

Accepts any polygon mesh (tris, quads, or higher `n`-gons; closed or open
with a boundary). Boundary edges and vertices are treated with the crease
stencils, so an open patch keeps its boundary shape and its Euler
characteristic. The output is always an all-quad mesh.

```rust
pub fn catmull_clark(mesh: &crate::mesh::Mesh, levels: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `transform`

Affine transforms over mesh vertices — the **CPU reference path**.

An [`Affine3`] is a `3x3` linear map plus a translation, i.e. the standard
rigid/affine transform used to place, rotate, and scale mesh geometry
(Blender's `Object.matrix_world` is exactly this class of operation). It is
the "hello world" of an *embarrassingly parallel* mesh kernel: every vertex
is transformed independently, with no cross-vertex dependency, so the same
math maps cleanly onto a GPU compute shader.

## Why this lives in the always-compiled part of the crate

This module is compiled in **every** build, `gpu` feature or not. It is the
**trusted, deterministic reference**: the GPU path in [`crate::gpu`] exists
only to *accelerate* the exact same computation, and its result is checked
against [`Affine3::transform_points`] here. Per the workspace design rules,
the CPU path is what V&V and downstream solvers rely on; GPU output (f32,
non-deterministic reduction order across hardware) is never the source of
truth.

Precision: this CPU path is `f64`. The GPU compute shader is `f32` (the
portable storage type for WGSL). Agreement between the two is therefore an
*approximate* match within an `f32`-scale tolerance, not bit-exact — see the
GPU test in [`crate::gpu`].

```rust
pub mod transform { /* ... */ }
```

### Types

#### Struct `Affine3`

A 3D affine transform: a `3x3` linear map `M` followed by a translation `t`,
acting on a point `p` as `M p + t`.

The linear part [`Affine3::linear`] is stored **row-major** as three rows of
three `f64` components; `linear[i]` is row `i`, so the transformed
coordinate `i` is `dot(linear[i], p) + translation[i]`. All components are
dimensionless model-space quantities (see [`crate::math`]).

`Affine3` is `Copy` and holds no borrows, in line with the workspace
no-lifetimes rule.

```rust
pub struct Affine3 {
    pub linear: [[f64; 3]; 3],
    pub translation: crate::math::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `linear` | `[[f64; 3]; 3]` | Row-major `3x3` linear part. `linear[i]` is the `i`-th output row; the<br>`i`-th transformed coordinate is `linear[i] . p + translation[i]`. |
| `translation` | `crate::math::Vec3` | Translation added after the linear map. |

##### Implementations

###### Methods

- ```rust
  pub const fn translation(t: Vec3) -> Affine3 { /* ... */ }
  ```
  A pure translation by `t` (identity linear part).

- ```rust
  pub const fn scale(sx: f64, sy: f64, sz: f64) -> Affine3 { /* ... */ }
  ```
  A non-uniform scale about the origin by `(sx, sy, sz)`, no translation.

- ```rust
  pub const fn from_rows(linear: [[f64; 3]; 3], translation: Vec3) -> Affine3 { /* ... */ }
  ```
  Build from an explicit row-major `3x3` linear part and a translation.

- ```rust
  pub fn transform_point(self: Self, p: Vec3) -> Vec3 { /* ... */ }
  ```
  Transform a single point `p` by this affine map, returning `M p + t`.

- ```rust
  pub fn transform_points(self: Self, positions: &[Vec3]) -> Vec<Vec3> { /* ... */ }
  ```
  Transform a whole slice of vertex positions on the CPU (the reference

- ```rust
  pub fn transform_points_best_effort(self: Self, positions: &[Vec3]) -> Vec<Vec3> { /* ... */ }
  ```
  Transform every position, **using the GPU as far as possible and falling

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
    fn clone(self: &Self) -> Affine3 { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Affine3) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
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
## Module `triangulate`

Triangulate — convert every polygon face into triangles, returning a
triangle-only [`Mesh`].

This is the pure-Rust analogue of Blender's **Triangulate Faces**
(`bmo_triangulate`, fan mode). It fan-triangulates each face — a quad
becomes two triangles, an `n`-gon becomes `n − 2` — and rebuilds a mesh
whose every face is a triangle.

# Why not [`crate::export::triangulate`]?

[`crate::export::triangulate`] produces an
[`crate::export::IndexedTriangles`] — a flat positions + `u32` index buffer
for a GPU or a solver import, **not** a [`Mesh`]. This operator instead
returns a first-class [`Mesh`] with half-edge topology, so downstream
mesh operators that assume or prefer triangles — [`crate::loop_subdivision`]
(Loop subdivision is only defined on triangle meshes),
[`crate::decimate`] (QEM edge collapse), and the CSG/polyMesh bridges — can
consume the result directly.

# Winding

Fan triangulation from each face's first corner preserves the face's
winding, so a consistently-wound, outward-facing input stays that way. No
`faer`, no external dependency; Android-safe.

```rust
pub mod triangulate { /* ... */ }
```

### Functions

#### Function `triangulate`

Fan-triangulate every face of `mesh`, returning a triangle-only mesh.

Positions are unchanged; only faces are re-cut. A face with fewer than three
corners is dropped (it is already degenerate); a triangle is passed through
unchanged. This is infallible.

# Examples

```
use outram_blender::{primitives, triangulate::triangulate};

// A cube's 6 quads fan-triangulate into 12 triangles; still χ = 2.
let cube = primitives::cube(2.0);
let tris = triangulate(&cube);
assert_eq!(tris.face_count(), 12);
assert_eq!(tris.euler_characteristic(), 2);
```

```rust
pub fn triangulate(mesh: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

## Module `weld`

Weld / remove-doubles — merge coincident vertices within a distance
tolerance into a single vertex.

This is the pure-Rust analogue of Blender's **Merge by Distance**
(`bmesh` `bmo_remove_doubles` / the Weld modifier `MOD_weld`): vertices
closer than `distance` collapse to one, faces are rebuilt on the merged
vertex set, and any face that loses too many distinct corners (fewer than
3) is dropped. It is the cleanup primitive that hardens a polygon soup — a
boolean result, an imported mesh, or an "exploded" face-varying surface —
into a watertight, shared-vertex mesh before it feeds the CSG / polyMesh
[`crate::export`] bridges.

# What "coincident" means

Two vertices are welded when the Euclidean distance between them is
`<= distance`. The merge relation is **not transitive** — `A`–`B` within
tolerance and `B`–`C` within tolerance does not imply `A`–`C` within
tolerance — so clusters are formed by **connectivity** (a union-find over
all within-tolerance pairs), not by pairwise distance from a single seed.
Every vertex in one connected cluster collapses to one output vertex placed
at the **average** of the cluster's positions (deterministic and
order-independent, matching the averaging convention used elsewhere in the
crate — Catmull-Clark, centroids).

# Algorithm (O(n) expected)

1. **`distance <= 0`** — weld only *exactly* coincident vertices, keyed on
   the `f64` bit pattern. A no-op when the mesh has no exact duplicates.
2. **`distance > 0`** — hash every vertex into a uniform grid of cell size
   `distance`. Because a within-tolerance partner can straddle a cell
   boundary, each vertex is tested against candidates in the **3×3×3 block
   of neighbour cells** (a cell size equal to the tolerance makes one cell
   of reach provably sufficient). Each accepted pair is `union`-ed.
3. Average each cluster's positions, rebuild faces on the merged set
   (dropping consecutive-duplicate corners and any face left with `< 3`
   distinct vertices), and compact so no isolated vertices survive.

No `faer`, no external dependency — just the fixed-size [`crate::math`]
types and standard collections. Android-safe.

```rust
pub mod weld { /* ... */ }
```

### Functions

#### Function `weld`

Merge vertices closer than `distance` into one, returning the welded mesh.

`distance` is a Euclidean length in the mesh's coordinate units. A value of
`0.0` (or negative) welds only bit-identical duplicate vertices, so it is a
safe no-op on a mesh that has none. Faces whose corners collapse to fewer
than three distinct vertices after welding are discarded; the winding order
of every surviving face is preserved (only vertex identities are
substituted).

This is infallible: the result is always a valid mesh (possibly with fewer
vertices and faces than the input).

# Examples

```
use outram_blender::{primitives, weld::weld};

// A pristine cube has no coincident vertices, so a zero-tolerance weld is a
// no-op and even a generous tolerance below the edge length changes nothing.
let cube = primitives::cube(2.0);
let welded = weld(&cube, 0.1);
assert_eq!(welded.vertex_count(), 8);
assert_eq!(welded.face_count(), 6);
assert_eq!(welded.euler_characteristic(), 2);
```

```rust
pub fn weld(mesh: &crate::mesh::Mesh, distance: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-blender/src/lib.rs:186:11: 186:32 (#0) }, crates/outram-blender/src/lib.rs:186:10: 186:33 (#0))])]")`

Headless GPU compute via `wgpu`. Compiled **unconditionally on every desktop
target** (no cargo feature to opt in) so the GPU path is used as far as
possible; **absent only on Android** (`target_os = "android"`), which has no
system Vulkan/Metal loader and where the workspace Android rule forbids GPU
deps in the library build. Whether or not this module is present, callers get
a graceful CPU fallback: on Android the GPU attempt is compiled out entirely,
and on desktop [`gpu::probe`] returning `None` or a recoverable
[`gpu::GpuError`] routes to the CPU reference path. See
[`transform::Affine3::transform_points_best_effort`] for the unified
try-GPU-then-CPU entry point, and [`gpu`] for the fallback contract.
GPU compute (headless, target-gated OFF Android; no cargo feature).

Headless GPU acceleration via [`wgpu`] for the *embarrassingly parallel*
parts of mesh authoring — per-vertex / per-face kernels, subdivision
evaluation, deformation. **No window or surface** is created; this is
compute-only (WGSL compute shaders).

## The wired demonstrator kernel

This module now carries **one real, end-to-end kernel**: applying an
[`crate::transform::Affine3`] to every vertex of a mesh in parallel via a
WGSL compute shader ([`transform_vertices_gpu`]). The identical computation
on the CPU is [`crate::transform::Affine3::transform_points`], which is the
reference the GPU result is validated against (see the tests below). This
kernel is deliberately the simplest embarrassingly-parallel mesh operation —
it exists to prove the GPU compute path is live and CPU-checked, not because
an affine transform needs a GPU. Heavier per-vertex kernels (deformation,
subdivision) follow the same buffer/pipeline pattern.

## Non-negotiable contract for using this module

1. **Target-gated, not feature-gated.** This module is compiled
   **unconditionally on every desktop target** — there is no `gpu` cargo
   feature to enable — so the GPU path is always available and used as far as
   possible. It is present on all targets **except Android**
   (`target_os = "android"`), where the workspace Android rule forbids GPU
   deps in the library build; there the GPU attempt is compiled out and the
   CPU path runs.
2. **Runtime CPU fallback is mandatory.** Even where wgpu is compiled, at
   runtime there may be **no usable GPU adapter** (headless servers, VMs) or
   a submission may fail mid-flight. Callers MUST treat [`probe`] returning
   `None`, and [`try_transform_vertices_gpu`] returning `Err`, as "run the
   CPU path", never as a hard error. [`crate::transform::Affine3::transform_points_best_effort`]
   wraps exactly this: try GPU, fall back to CPU, always return a result.
3. **CPU is the trusted / reference path.** GPU float reduction order will
   not bit-match the CPU (`f64`, [`crate::transform`]) result, so anything
   that feeds V&V or a solver stays CPU-deterministic. GPU is *acceleration
   only*, and [`transform_vertices_gpu`] returns `f32`-precision results.

```rust
pub mod gpu { /* ... */ }
```

### Types

#### Enum `GpuError`

A **recoverable** GPU execution failure from [`try_transform_vertices_gpu`].

Every variant means the same thing to a caller: the GPU attempt did not
complete, so fall back to the CPU reference path
([`Affine3::transform_points`]). The GPU is acceleration only and never the
source of truth, so a `GpuError` is a routine "use the CPU" signal, not a
fatal condition — [`Affine3::transform_points_best_effort`] does this
automatically. This deliberately does **not** cover the "no adapter at all"
case, which surfaces earlier as [`probe`] returning `None`.

```rust
pub enum GpuError {
    Poll(String),
    Map(String),
    MapCallbackMissing,
}
```

##### Variants

###### `Poll`

Polling the device to drive the readback failed (e.g. device lost).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Map`

The staging buffer could not be mapped back to the CPU.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `MapCallbackMissing`

The buffer-map callback never fired despite a wait-indefinitely poll.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Struct `GpuContext`

A live GPU compute context: a headless [`wgpu::Device`] and its
[`wgpu::Queue`].

Obtain one from [`probe`]. A `GpuContext` owns its device and queue by value
(no borrows — workspace no-lifetimes rule) and is `!Clone`; share it across
threads behind an `Arc` if needed. Dropping it releases the GPU resources.

```rust
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `device` | `wgpu::Device` | The logical GPU device — used to create buffers, shaders, and pipelines. |
| `queue` | `wgpu::Queue` | The command queue — used to upload buffers and submit compute work. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

#### Function `probe`

Probe for a usable headless GPU compute adapter.

Creates a [`wgpu::Instance`] over all backends enabled for this platform,
requests an adapter with **no surface** (headless compute — `power_preference
= None`, no `compatible_surface`), then requests a device + queue with the
downlevel default limits (the broadest-compatibility profile, so software
adapters like Lavapipe/WARP also qualify). The blocking wait on wgpu's async
requests is done with a tiny in-crate executor ([`block_on`]) so this crate
pulls in no async-runtime dependency.

Returns `Some(GpuContext)` when a headless compute device is available, or
`None` when the caller must fall back to the CPU path
([`crate::transform::Affine3::transform_points`]). `None` is a normal,
expected outcome on headless CI and the Android emulator — it is **not** an
error.

```rust
pub fn probe() -> Option<GpuContext> { /* ... */ }
```

#### Function `try_transform_vertices_gpu`

Apply `affine` to every position in `positions` on the GPU, returning the
transformed positions in the same order — the **fallible** entry point.

This is the demonstrator GPU kernel. It uploads the positions as an `f32`
storage buffer, dispatches [`AFFINE_TRANSFORM_WGSL`] one invocation per
vertex, and reads the result back. **Results are `f32` precision** — the
caller must treat them as an acceleration of, and approximation to,
[`Affine3::transform_points`] (the trusted `f64` CPU reference), not as a
bit-exact match.

An empty `positions` slice returns an empty `Vec` without touching the GPU.

# Errors

Returns [`GpuError`] if the submitted work cannot be completed (device lost
during the readback poll, or buffer-map failure). This is **recoverable**:
the caller should fall back to [`Affine3::transform_points`] — which
[`Affine3::transform_points_best_effort`] does automatically. The "no adapter
at all" case is handled earlier by [`probe`] returning `None`, not here.

```rust
pub fn try_transform_vertices_gpu(ctx: &GpuContext, affine: crate::transform::Affine3, positions: &[crate::math::Vec3]) -> Result<Vec<crate::math::Vec3>, GpuError> { /* ... */ }
```

#### Function `transform_vertices_gpu`

Apply `affine` to every position on the GPU, panicking on failure — the
strict convenience wrapper over [`try_transform_vertices_gpu`].

Use this only when a GPU failure should abort (e.g. a benchmark that must run
on the GPU, or a test that has already confirmed an adapter via [`probe`]).
For normal use prefer [`Affine3::transform_points_best_effort`], which never
panics and falls back to the CPU. **Results are `f32` precision.**

# Panics

Panics if [`try_transform_vertices_gpu`] returns a [`GpuError`].

```rust
pub fn transform_vertices_gpu(ctx: &GpuContext, affine: crate::transform::Affine3, positions: &[crate::math::Vec3]) -> Vec<crate::math::Vec3> { /* ... */ }
```

### Re-exports

#### Re-export `wgpu`

Re-export of the GPU backend so callers can build pipelines without adding
their own `wgpu` dependency. Present on every desktop target (absent only on
Android, where this whole module is compiled out).

```rust
pub use wgpu;
```

## Re-exports

### Re-export `faer`

Heavy linear-algebra backend for the *large* mesh solves the advanced
operators will need — Laplacian mesh editing, ARAP deformation, and mesh
parameterization all build a sparse Laplacian over the mesh and solve
`A x = b`. Re-exports [`faer`], a pure-Rust, Android-safe dense **and**
sparse linear-algebra library (SIMD via `pulp`, no system BLAS).

**Division of labour:** per-element geometry math (positions, normals,
transforms) stays in the fixed-size [`math`] types — small, fast, no
allocation. `faer` is only for the big systems. For interactive editing
(same matrix, many right-hand sides) prefer `faer`'s sparse **Cholesky**
factorization over an iterative solve. An *optional* bridge to
`outram-foam-basic-lib`'s CG/GAMG iterative solvers is tracked separately for
large one-off sparse solves (see beads `op-hzs`).

**First consumer:** [`laplacian`] — the cotangent/uniform discrete Laplacian
and implicit Laplacian smoothing assemble a sparse system and solve it with
`faer`'s sparse Cholesky. Future ARAP / parameterization operators reuse the
same path.

```rust
pub use faer;
```

