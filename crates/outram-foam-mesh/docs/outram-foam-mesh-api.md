# Crate Documentation

**Version:** 0.1.3

**Format Version:** 61

# Module `outram_foam_mesh`

# outram-foam-mesh

OpenFOAM **mesh generation and conversion** utilities, translated to Rust on
top of [`outram_foam_basic_lib`]'s primitive + finite-volume layer (`FvMesh`,
`polyMesh` topology, points/faces/cells).

> **Independent OUTRAM PARK fork, not the official OpenFOAM.** This crate is
> not affiliated with, endorsed by, or sanctioned by OpenCFD Ltd. / the
> OpenFOAM Foundation / ESI Group. "OpenFOAM" and the tool names
> (blockMesh, snappyHexMesh, …) are used only to identify the upstream
> algorithms this crate re-implements. See `TRADEMARKS.md`.
>
> **⚠️ Unverified until validated.** Everything here is a work-in-progress
> translation; use at your own risk. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

## Start here

If you have a geometry and want a mesh, call one function —
[`mesh_from_surface`] — and grade the result with
[`assess_quality`](mesh_quality::assess_quality):

```no_run
use outram_foam_mesh::{mesh_from_surface, MeshingControls, MeshingPhases};
use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;
use outram_foam_basic_lib::primitives::Vector3;

let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 24, 24);
let controls = MeshingControls::external_flow([12, 12, 12], 2)
    .with_phases(MeshingPhases::CastellateSnap);

let result = mesh_from_surface(&surface, &controls).unwrap();
println!("{}", result.quality.summary());
result.write_case("my_case").unwrap();   // → my_case/constant/polyMesh
```

The worked example `examples/sphere_in_box.rs` runs exactly this end to end
and is readable top to bottom.

## What belongs here

Mesh **construction** and **format conversion** — producing / importing a
`polyMesh` (points, faces, owner/neighbour, boundary patches) that the
Layer-1–4 crate and the solver crates then operate on. The tools:

- [`driver`] — **the high-level entry point**: surface + controls → a
  [`PolyMesh`](outram_foam_basic_lib::io::PolyMesh), with the phases chosen
  by enum.
- [`mesh_quality`] — `checkMesh`: non-orthogonality, skewness, aspect ratio
  and inverted-cell count for **any** `polyMesh`, including ones this crate
  did not generate.
- [`block_mesh`] — structured hexahedral block meshing from a `blockMeshDict`
  (the OpenFOAM `blockMesh` utility).
- [`snappy_hex_mesh`] — automatic split-hex meshing around STL surfaces:
  castellation (octree refinement), snapping, and boundary layers
  (`snappyHexMesh`).
- [`ideas_unv_to_foam`] — import an I-DEAS `.unv` (UNV) mesh into `polyMesh`
  (`ideasUnvToFoam`).
- [`poly_dual_mesh`] — construct the polyhedral dual of a mesh
  (`polyDualMesh`).

Solver loops, turbulence models, and thermophysics do **not** belong here —
they live in the solver crates and `outram-foam-basic-lib`.

## One mesh currency

Each generator has its own working representation, but all of them convert
to `outram-foam-basic-lib`'s
[`PolyMesh`](outram_foam_basic_lib::io::PolyMesh) — the type that writes
`constant/polyMesh` to disk and that [`mesh_quality`] grades:

| Produced by | Convert with |
|---|---|
| [`block_mesh`] | [`block_mesh::PolyMesh::to_foam_poly_mesh`] |
| [`snappy_hex_mesh`] | [`snappy_hex_mesh::PolyPatchMesh::to_foam_poly_mesh`] |
| [`poly_dual_mesh`] | [`poly_dual_mesh::DualMesh::to_foam_poly_mesh`] |
| [`driver`] | already a `PolyMesh` — [`GeneratedMesh::write_case`] |

## Modules

## Module `block_edges`

Curved block edges for `blockMesh` (gh:#1461, epic gh:#1458).

A `blockMeshDict` `edges` entry replaces the straight line between two block
vertices by a curve. Upstream registers `line`, `arc`, `polyLine`, `spline`
(Catmull-Rom) and `BSpline` in dev `852527377c7e`. ~~`project` and
`projectCurve` need `geometry` and are reported as not implemented~~
**CORRECTED 2026-11-11 (gh:#1462)**: they are ported, resolved against the
`geometry` by [`BlockEdge::resolve_with`] ([`crate::block_projection`]).

Every curve answers one question, [`EdgeCurve::position`]: the point at the
edge parameter `lambda` in `[0, 1]`. The caller places the `n + 1`
parameters by the edge's grading as `lineDivide` does, and
[`crate::block_mesh`] blends the edge points into the block interior as
`block::createPoints` does.

Coordinates are the **raw dictionary coordinates** (before `convertToMeters`
/ `units`): upstream builds edges from the unscaled `vertices_` and scales
the finished points (`blockMeshCreate.C:223`).

Transcendentals (`cos`, `sin`, `tan`, `acos`) go through
`petir::mathf::RealMath`; `sqrt` and `floor` are exact on `std`.

## Upstream quirks kept on purpose

- `arc v0 v1 angle (axis)` converts `angle` to radians and *then* checks
  `0 < theta < 360` (`arcEdge.C`, `calc(theta, axis)`), so the bound is 360
  **radians**, not degrees. Kept; a larger angle is rejected as upstream does.
- The `spline` / `BSpline` position is not the raw segment polynomial but an
  iteration (`Spline::position`, at most 16 iterations, tolerance
  `rootSmall`) that re-parameterises so that `lambda` is the fraction of
  straight-line distance from the start, as the grading requires.
- Optional trailing start/end tangents of `spline` / `BSpline` are read and
  discarded by the parser, as upstream does.

```rust
pub mod block_edges { /* ... */ }
```

### Types

#### Enum `BlockEdgeKind`

What an `edges` entry says about the curve, before the vertex positions are
known.

```rust
pub enum BlockEdgeKind {
    Line,
    ArcThroughPoint(outram_foam_basic_lib::primitives::Vector3),
    ArcAngleAxis {
        angle_deg: f64,
        axis: outram_foam_basic_lib::primitives::Vector3,
    },
    PolyLine(Vec<outram_foam_basic_lib::primitives::Vector3>),
    Spline(Vec<outram_foam_basic_lib::primitives::Vector3>),
    BSpline(Vec<outram_foam_basic_lib::primitives::Vector3>),
    Project(Vec<String>),
    ProjectCurve(Vec<String>),
}
```

##### Variants

###### `Line`

`line v0 v1`: a straight edge. Upstream still treats it as a listed
(curved-counted) edge in `block::createPoints`.

###### `ArcThroughPoint`

`arc v0 v1 (x y z)`: the circle through both vertices and this point.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_foam_basic_lib::primitives::Vector3` |  |

###### `ArcAngleAxis`

`arc v0 v1 angle (ax ay az)`: angle in degrees about an axis (a helix
when the chord has an axial component).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `angle_deg` | `f64` | Angle `[deg]`. |
| `axis` | `outram_foam_basic_lib::primitives::Vector3` | Axis (not necessarily unit length in the dictionary). |

###### `PolyLine`

`polyLine v0 v1 ((x y z) ...)`: straight segments through the points.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<outram_foam_basic_lib::primitives::Vector3>` |  |

###### `Spline`

`spline v0 v1 ((x y z) ...)`: Catmull-Rom spline through the points.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<outram_foam_basic_lib::primitives::Vector3>` |  |

###### `BSpline`

`BSpline v0 v1 ((x y z) ...)`: uniform cubic B-spline.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<outram_foam_basic_lib::primitives::Vector3>` |  |

###### `Project`

`project v0 v1 (surface ...)`: the straight edge projected onto the
named `geometry` surfaces (gh:#1462).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<String>` |  |

###### `ProjectCurve`

`projectCurve v0 v1 (surface ...)` (same algorithm as `project` unless
an `extrudedCircle` is listed, which is not ported; gh:#1462).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<String>` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BlockEdge`

One parsed `edges` entry: the two block-vertex labels and the curve kind.

```rust
pub struct BlockEdge {
    pub start: usize,
    pub end: usize,
    pub kind: BlockEdgeKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `usize` | Start vertex label (index into the `vertices` list). |
| `end` | `usize` | End vertex label. |
| `kind` | `BlockEdgeKind` | The curve. |

##### Implementations

###### Methods

- ```rust
  pub fn compare(self: &Self, start: usize, end: usize) -> i32 { /* ... */ }
  ```
  `blockEdge::compare(start, end)`: `1` when this edge runs `start` ->

- ```rust
  pub fn resolve(self: &Self, vertices: &[Vector3]) -> Result<EdgeCurve, MeshError> { /* ... */ }
  ```
  Resolve the curve against the (raw) vertex positions.

- ```rust
  pub fn resolve_with(self: &Self, vertices: &[Vector3], geometry: &Arc<Geometry>) -> Result<EdgeCurve, MeshError> { /* ... */ }
  ```
  As [`resolve`](Self::resolve), with the `geometry` that `project` /

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `EdgeCurve`

A resolved edge curve (enum dispatch, no trait objects).

```rust
pub enum EdgeCurve {
    Line {
        p0: outram_foam_basic_lib::primitives::Vector3,
        p1: outram_foam_basic_lib::primitives::Vector3,
    },
    Arc(ArcEdge),
    PolyLine(PolyLine),
    CatmullRom(Vec<outram_foam_basic_lib::primitives::Vector3>),
    BSpline(Vec<outram_foam_basic_lib::primitives::Vector3>),
    Project(crate::block_projection::ProjectEdge),
}
```

##### Variants

###### `Line`

`lineEdge`: `p0 + lambda*(p1 - p0)`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `p0` | `outram_foam_basic_lib::primitives::Vector3` | Start point. |
| `p1` | `outram_foam_basic_lib::primitives::Vector3` | End point. |

###### `Arc`

`arcEdge`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ArcEdge` |  |

###### `PolyLine`

`polyLineEdge`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PolyLine` |  |

###### `CatmullRom`

`splineEdge` (Catmull-Rom); all knots including both end points.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<outram_foam_basic_lib::primitives::Vector3>` |  |

###### `BSpline`

`BSplineEdge`; all knots including both end points.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<outram_foam_basic_lib::primitives::Vector3>` |  |

###### `Project`

`projectEdge` / `projectCurveEdge`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::block_projection::ProjectEdge` |  |

##### Implementations

###### Methods

- ```rust
  pub fn position(self: &Self, lambda: f64) -> Vector3 { /* ... */ }
  ```
  The point at edge parameter `lambda` (`blockEdge::position(lambda)`).

- ```rust
  pub fn positions(self: &Self, lambdas: &[f64]) -> Vec<Vector3> { /* ... */ }
  ```
  `blockEdge::position(const scalarList&)`: the points at each parameter.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ArcEdge`

A circular (or helical) arc: `arcEdge.C`.

```rust
pub struct ArcEdge {
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
  pub fn through_point(start: usize, end: usize, p0: Vector3, p1: Vector3, pa: Vector3) -> Result<Self, MeshError> { /* ... */ }
  ```
  `arcEdge::calc(const point& pA)`: the arc through `p0`, `pa`, `p1`.

- ```rust
  pub fn from_angle(start: usize, end: usize, p0: Vector3, p1: Vector3, theta: f64, axis: Vector3) -> Result<Self, MeshError> { /* ... */ }
  ```
  `arcEdge::calc(theta, axis)`; `theta` in radians, `axis` a unit vector.

- ```rust
  pub fn position(self: &Self, lambda: f64) -> Vector3 { /* ... */ }
  ```
  `arcEdge::position(lambda)`.

- ```rust
  pub fn centre(self: &Self) -> Vector3 { /* ... */ }
  ```
  Arc centre (raw dictionary coordinates).

- ```rust
  pub fn theta(self: &Self) -> f64 { /* ... */ }
  ```
  Swept angle `[rad]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PolyLine`

`polyLine`: straight segments through the knots, parameterised by
normalised cumulative length.

```rust
pub struct PolyLine {
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
  pub fn new(points: Vec<Vector3>) -> Self { /* ... */ }
  ```
  `polyLine::calcParam` over all knots (end points included).

- ```rust
  pub fn position(self: &Self, mu: f64) -> Vector3 { /* ... */ }
  ```
  `polyLine::position(mu)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `block_mesh`

`blockMesh` — structured hexahedral block meshing from a `blockMeshDict`.

Reads a `blockMeshDict` (vertices, blocks with cell counts + grading, edges,
boundary patches) and produces a `polyMesh` (points, faces, owner/neighbour,
boundary patches) via the standard OpenFOAM `blockMesh` algorithm: each block
is a hexahedron mapped from the unit cube through its 8 vertices, subdivided
by its `(nx, ny, nz)` cell counts with optional `simpleGrading` geometric
expansion; faces that coincide on shared block boundaries are merged into
internal faces.

## Pipeline

1. [`BlockMeshDict::parse`] tokenises and parses the dict text into a
   [`BlockMeshDict`] (`convertToMeters`, [`vertices`](BlockMeshDict::vertices),
   [`blocks`](BlockMeshDict::blocks), boundary [`patches`](BlockMeshDict::patches)).
2. [`BlockMeshDict::build`] generates every block's points (12-edge blend),
   merges points **topologically** across the block faces the block topology
   shares, numbers points, faces and cells as `blockMesh` does (gh:#1464),
   checks the topology, and returns a [`PolyMesh`]. ~~merges globally
   coincident points by quantised coordinates~~ **CORRECTED 2026-11-11
   (gh:#1464)**: coordinates play no part in the merge except to pair
   points on a shared block face, exactly as upstream's `calcMergeInfo`;
   blocks that only touch geometrically are not merged.
3. [`PolyMesh::to_fv_mesh`] computes the finite-volume geometry (cell volumes
   and centres, face-area vectors and centres) and emits the
   `outram-foam-basic-lib` [`FvMesh`].

The convenience free function [`block_mesh`] runs the whole pipeline.

## Units

Dict coordinates are dimensionless and are multiplied by `convertToMeters`
(metres per dict unit) to obtain SI positions. All emitted geometry is SI:
points/centres in metres `[m]`, face areas in `[m^2]`, cell volumes in
`[m^3]`.

## Deferred dict features

- ~~`edges` blocks (arc / spline / polyLine) are parsed and **skipped**: all
  block edges are treated as straight lines~~ **CORRECTED 2026-11-11
  (gh:#1461)**: `edges` (`line`, `arc` in both forms, `polyLine`, `spline`,
  `BSpline`) are parsed and applied, see [`crate::block_edges`]; a block
  with a listed edge takes the full 12-edge blend plus upstream's
  curved-edge correction. ~~`project` / `projectCurve` edges, `geometry`,
  `faces`~~ **CORRECTED 2026-11-11 (gh:#1462)**: `geometry` (sphere,
  cylinder, plane), `project` vertices, `project` / `projectCurve` edges and
  `project` / `sweep` block faces are ported, see
  [`crate::block_projection`]; a surface type that is not ported (including
  `triSurface`, `extrudedCircle`, `plate`) returns
  [`MeshError::NotImplemented`]. `#calc` / `#include` and `namedBlocks`
  return [`MeshError::NotImplemented`]; they are never skipped.
- `units [mm]` (a single length unit), `convertToMeters` and `scale` are
  read, at most one of the three (gh:#1463). `defaultPatch { name; type; }`
  is read; without it the unassigned faces form `defaultFaces` of type
  `empty`, as upstream (it was `patch` before 2026-11-11). `$name` of an
  earlier top-level entry is expanded. Block zone names are kept in
  [`PolyMesh::cell_zones`]; the `blockMesh` CLI writes them as
  `constant/polyMesh/cellZones` (`io::PolyMesh::write_cell_zones`).
- `simpleGrading (gx gy gz)` is fully supported, where each of the three
  directions is **either** a single scalar expansion ratio **or** a
  multi-grading list of `( fractionLength fractionCells expansionRatio )`
  segments (OpenFOAM `gradingDescriptors`), graded piecewise-geometrically.
- `edgeGrading (e0 … e11)` is **fully supported**: each of the hex's 12
  edges carries its own [`Grading`] (single ratio or multi-segment), and
  interior nodes are blended from all 12 edge distributions exactly as
  OpenFOAM's `block::createPoints` does (the straight-edge, no-curvature
  case). ~~When the four edges of a direction happen to agree, the block
  collapses to the equivalent `simpleGrading` fast path (bit-identical to a
  single per-direction distribution)~~ **CORRECTED 2026-11-11 (gh:#1464):**
  every block, `simpleGrading` or not, takes the 12-edge blend, as upstream
  (`blockCreate.C` has no trilinear path); the trilinear map differed from
  it by round-off. See `edge_blended_node_with`.
- ~~`mergePatchPairs` is parsed and ignored~~ **CORRECTED 2026-11-11**: a
  non-empty `mergePatchPairs` returns [`MeshError::NotImplemented`]
  (gh:#1464). `fastMerge yes` also returns `NotImplemented`
  (`blockMeshMergeFast.C`).

## Honest scope — what the per-edge blend does NOT model

The straight-edge branch and, since 2026-11-11 (gh:#1461), the curved-edge
correction of OpenFOAM `block::createPoints` (`blockCreate.C:45-183`) are
ported. ~~The curved-**face** projection passes (`blockCreate.C:185-345`)
are **not** implemented (gh:#1462)~~ **CORRECTED 2026-11-11 (gh:#1462)**:
they are, in [`crate::block_projection::apply_curved_faces`]; a block with no
`faces` entry never enters them. For blocks with planar faces (straight or
curved edges only) the
blend is exact and volume-preserving; a warped internal face between cells of
a per-edge-graded block is expected and handled by the divergence-theorem
cell geometry. For multi-block meshes, matching gradings on any **shared**
edge remain the dict author's responsibility, exactly as in OpenFOAM
`blockMesh` (a mismatch leaves the shared boundary nodes un-merged).

```rust
pub mod block_mesh { /* ... */ }
```

### Types

#### Struct `GradingSegment`

One segment of a multi-grading list, as it appears inside a parenthesised
per-direction grading entry `( fractionLength fractionCells expansionRatio )`.

Mirrors OpenFOAM's `Foam::gradingDescriptor`
(`src/mesh/blockMesh/gradingDescriptor/gradingDescriptor.C`). All three
fields are **dimensionless**:

- `fraction_length` — the fraction of the local block edge length this
  segment occupies (OpenFOAM `blockFraction`). Relative; the segment
  fractions of a direction are normalised to sum to `1` at mesh-build time.
- `fraction_cells` — the fraction of the direction's cell count allocated to
  this segment (OpenFOAM `nDivFraction`). Relative; likewise normalised.
- `expansion` — the geometric expansion ratio over the segment, defined as
  `end-cell-width / start-cell-width`. `1.0` is uniform spacing; a negative
  value is trapped and treated as its inverse (OpenFOAM
  `gradingDescriptor::correct`).

```rust
pub struct GradingSegment {
    pub fraction_length: f64,
    pub fraction_cells: f64,
    pub expansion: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fraction_length` | `f64` | Fraction of the block edge length occupied by this segment<br>(dimensionless, relative; normalised to sum to `1` per direction). |
| `fraction_cells` | `f64` | Fraction of the direction's cell count in this segment (dimensionless,<br>relative; normalised to sum to `1` per direction). |
| `expansion` | `f64` | Geometric expansion ratio `end-cell-width / start-cell-width`<br>(dimensionless, `> 0`; negatives trapped as their inverse). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Grading`

The node distribution along one local block direction.

Mirrors OpenFOAM's `Foam::gradingDescriptors`
(`src/mesh/blockMesh/gradingDescriptor/gradingDescriptors.C`): a direction is
either a single `simpleGrading` expansion ratio, or a list of
[`GradingSegment`]s (multi-grading). Both are dimensionless — they only
redistribute the `n + 1` node positions along the `[0, 1]` parametric edge,
never the cell count.

```rust
pub enum Grading {
    Uniform(f64),
    Multi(Vec<GradingSegment>),
}
```

##### Variants

###### `Uniform`

A single expansion ratio (OpenFOAM scalar `simpleGrading` entry),
`end-cell-width / start-cell-width`; `1.0` is uniform.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Multi`

A multi-grading list: the direction is split into contiguous
[`GradingSegment`]s, each graded geometrically over its own portion of
the edge. Segments are listed start → end along the local direction.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<GradingSegment>` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn eq(self: &Self, other: &f64) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Block`

One `hex` block from the `blocks` list.

The block is a hexahedron whose 8 corners are indices into
[`BlockMeshDict::vertices`], subdivided into `cells[0] * cells[1] * cells[2]`
hexahedral cells. `grading` holds the three per-direction [`Grading`]
distributions (from `simpleGrading` or, when all four edges of a direction
agree, `edgeGrading`); each direction only redistributes node positions
along its `[0, 1]` parametric edge (`1.0` uniform expansion is even
spacing).

`edge_grading` is the genuine per-edge case: `Some([Grading; 12])` when an
`edgeGrading` entry gives four edges of some direction *different* gradings,
so interior nodes must be blended from all 12 edge distributions
(OpenFOAM `block::createPoints`) rather than one distribution per direction.
When it is `Some`, the block-build path uses `edge_blended_node` and
`grading` merely holds a representative (the first edge of each direction)
for inspection. When it is `None`, `grading` is authoritative and each of a
direction's four edges takes it (`simpleGrading`, and every `edgeGrading`
whose four edges per direction agree). ~~`None` took a trilinear fast
path~~ **CORRECTED 2026-11-11 (gh:#1464):** both cases take the 12-edge blend.

```rust
pub struct Block {
    pub vertices: [usize; 8],
    pub cells: [usize; 3],
    pub grading: [Grading; 3],
    pub edge_grading: Option<[Grading; 12]>,
    pub zone: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vertices` | `[usize; 8]` | The 8 block-corner vertex indices, in OpenFOAM hex order. |
| `cells` | `[usize; 3]` | Cell counts `(nx, ny, nz)` along the three local directions. |
| `grading` | `[Grading; 3]` | Per-direction node distributions `(gx, gy, gz)`. Authoritative when<br>`edge_grading` is `None`; a per-direction representative otherwise. |
| `edge_grading` | `Option<[Grading; 12]>` | The 12 per-edge gradings, present only for a genuinely per-edge<br>`edgeGrading` (edges 0–3 = x, 4–7 = y, 8–11 = z; see<br>`edge_blended_node`). `None`: each direction's four edges share `grading`. |
| `zone` | `Option<String>` | The block's zone name (`hex (...) zoneName (nx ny nz) ...`), which<br>upstream turns into a `cellZone`; `None` when the block is unzoned. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PatchDef`

One named boundary patch from the `boundary` list.

`faces` are quads given as **block-corner** vertex indices (into
[`BlockMeshDict::vertices`]) — the coarse block-face they cover, not the fine
mesh faces. `build` expands each coarse quad to all the fine boundary faces
lying on that block face.

```rust
pub struct PatchDef {
    pub name: String,
    pub kind: outram_foam_basic_lib::mesh::PatchKind,
    pub faces: Vec<[usize; 4]>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Patch name (e.g. `"movingWall"`). |
| `kind` | `outram_foam_basic_lib::mesh::PatchKind` | Topological patch kind, mapped from the dict `type` keyword. |
| `faces` | `Vec<[usize; 4]>` | Coarse block-face quads (block-corner vertex indices). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BlockMeshDict`

A parsed `blockMeshDict`.

Produced by [`BlockMeshDict::parse`]; consumed by [`BlockMeshDict::build`].

```rust
pub struct BlockMeshDict {
    pub convert_to_meters: f64,
    pub vertices: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub blocks: Vec<Block>,
    pub patches: Vec<PatchDef>,
    pub edges: Vec<crate::block_edges::BlockEdge>,
    pub default_patch_name: String,
    pub default_patch_kind: outram_foam_basic_lib::mesh::PatchKind,
    pub fast_merge: bool,
    pub check_face_correspondence: bool,
    pub check_block_face_orientation: bool,
    pub geometry: std::sync::Arc<crate::block_projection::Geometry>,
    pub faces: Vec<crate::block_projection::BlockFace>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `convert_to_meters` | `f64` | Scale factor `[m per dict unit]` applied to every vertex (`convertToMeters`). |
| `vertices` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | The vertex list (raw, unscaled dict coordinates). |
| `blocks` | `Vec<Block>` | The `hex` blocks. |
| `patches` | `Vec<PatchDef>` | The named boundary patches. |
| `edges` | `Vec<crate::block_edges::BlockEdge>` | The `edges` list: curved (or explicitly `line`) block edges, in<br>dictionary order (the first match wins, as upstream's<br>`blockDescriptor::edgePointsWeights`). Empty means every edge is straight. |
| `default_patch_name` | `String` | Name of the patch that collects block faces no `boundary` entry names<br>(`defaultPatch { name ...; }`, default `defaultFaces`). |
| `default_patch_kind` | `outram_foam_basic_lib::mesh::PatchKind` | Type of that patch (`defaultPatch { type ...; }`). Upstream's default<br>when the entry is absent is `empty` (`blockMeshTopology.C`,<br>`defaultPatchType = emptyPolyPatch::typeName`). |
| `fast_merge` | `bool` | `fastMerge` (default `no`). `yes` selects `blockMeshMergeFast.C`, which<br>is not ported: `build` returns `NotImplemented` for it. |
| `check_face_correspondence` | `bool` | `checkFaceCorrespondence` (default `yes`): neighbouring blocks must<br>have the same number of faces on their shared block face. |
| `check_block_face_orientation` | `bool` | `checkBlockFaceOrientation` (default `yes`): every block's faces must<br>point away from its centre (`blockDescriptor::check`). |
| `geometry` | `std::sync::Arc<crate::block_projection::Geometry>` | The `geometry` dictionary: searchable surfaces that `project` vertices,<br>edges and faces refer to (gh:#1462). Empty when absent. |
| `faces` | `Vec<crate::block_projection::BlockFace>` | The `faces` list: `project` and `sweep` block faces (gh:#1462). |

##### Implementations

###### Methods

- ```rust
  pub fn build(self: &Self) -> Result<PolyMesh, MeshError> { /* ... */ }
  ```
  Build the mesh the way upstream `blockMesh` does (gh:#1464): every

- ```rust
  pub fn parse(text: &str) -> Result<Self, MeshError> { /* ... */ }
  ```
  Parse `blockMeshDict` text into a [`BlockMeshDict`].

- ```rust
  pub fn parse_with_flavour(text: &str, flavour: ProjectionFlavour) -> Result<Self, MeshError> { /* ... */ }
  ```
  As [`parse`](Self::parse), following v2506 instead of dev where the two

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MeshFace`

A single mesh face: its point-index loop plus owner / neighbour cells.

`verts` is wound so the face normal points **from `owner` towards
`neighbour`** (outward from the owner cell). Boundary faces have
`neighbour == None` and their normal points out of the domain.

```rust
pub struct MeshFace {
    pub verts: Vec<usize>,
    pub owner: usize,
    pub neighbour: Option<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `verts` | `Vec<usize>` | Ordered point indices (into [`PolyMesh::points`]) forming the face loop. |
| `owner` | `usize` | Owning cell index. |
| `neighbour` | `Option<usize>` | Neighbour cell index (internal faces only). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PolyMesh`

The generated poly-mesh: merged points, ordered faces, and boundary patches.

Faces are ordered OpenFOAM-style: internal faces first
(`[0, n_internal_faces)`), then boundary faces grouped by patch in dict
order. This is the crate's own lightweight `polyMesh`; call
[`PolyMesh::to_fv_mesh`] to obtain the `outram-foam-basic-lib` [`FvMesh`]
with full finite-volume geometry.

```rust
pub struct PolyMesh {
    pub points: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub faces: Vec<MeshFace>,
    pub n_internal_faces: usize,
    pub n_cells: usize,
    pub patches: Vec<outram_foam_basic_lib::mesh::BoundaryPatch>,
    pub cell_zones: Vec<(String, Vec<usize>)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Mesh points `[m]` (already scaled by `convertToMeters`, coincident block<br>nodes merged). |
| `faces` | `Vec<MeshFace>` | All faces, internal first then boundary (see struct docs). |
| `n_internal_faces` | `usize` | Number of internal faces (the count of leading internal entries in<br>`faces`). |
| `n_cells` | `usize` | Number of cells. |
| `patches` | `Vec<outram_foam_basic_lib::mesh::BoundaryPatch>` | Boundary patches, covering `[n_internal_faces, faces.len())` contiguously. |
| `cell_zones` | `Vec<(String, Vec<usize>)>` | Cell zones, one per distinct block zone name in dictionary order:<br>`(name, cell indices)`. Upstream writes these as `cellZones`; the<br>`io::PolyMesh` carries no zones, so [`PolyMesh::to_foam_poly_mesh`]<br>drops them; write them with<br>`outram_foam_basic_lib::io::poly_mesh::PolyMesh::write_cell_zones`<br>(~~"no zone output yet"~~ **CORRECTED 2026-11-11, gh:#1463**). |

##### Implementations

###### Methods

- ```rust
  pub fn concatenate(parts: &[(&str, &PolyMesh)]) -> PolyMesh { /* ... */ }
  ```
  Several region meshes side by side as one `PolyMesh`, for drawing a

- ```rust
  pub fn n_points(self: &Self) -> usize { /* ... */ }
  ```
  Number of points.

- ```rust
  pub fn n_faces(self: &Self) -> usize { /* ... */ }
  ```
  Total number of faces (internal + boundary).

- ```rust
  pub fn n_boundary_faces(self: &Self) -> usize { /* ... */ }
  ```
  Number of boundary faces.

- ```rust
  pub fn total_volume(self: &Self) -> f64 { /* ... */ }
  ```
  Total mesh volume `[m^3]` — the sum of all cell volumes.

- ```rust
  pub fn to_foam_poly_mesh(self: &Self) -> outram_foam_basic_lib::io::PolyMesh { /* ... */ }
  ```
  Convert to the writable `outram-foam-basic-lib`

- ```rust
  pub fn from_foam_poly_mesh(m: &outram_foam_basic_lib::io::PolyMesh) -> PolyMesh { /* ... */ }
  ```
  The inverse of [`PolyMesh::to_foam_poly_mesh`]: take an

- ```rust
  pub fn to_fv_mesh(self: &Self) -> Result<FvMesh, MeshError> { /* ... */ }
  ```
  Convert to the `outram-foam-basic-lib` [`FvMesh`], computing all

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `block_mesh`

Parse a `blockMeshDict` (as text) and build the [`PolyMesh`] in one call.

Equivalent to `BlockMeshDict::parse(dict_text)?.build()`.

# Errors
[`MeshError::DictParse`] on a malformed dict, [`MeshError::NotImplemented`]
for an unsupported block shape (only `hex` is handled; all grading forms —
`simpleGrading`, multi-grading, and full per-edge `edgeGrading` — are
supported), or [`MeshError::Construction`] on a topological inconsistency.

```rust
pub fn block_mesh(dict_text: &str) -> Result<PolyMesh, crate::MeshError> { /* ... */ }
```

#### Function `block_mesh_from_file`

Parse a `blockMeshDict` from a file path and build the [`PolyMesh`].

# Errors
As [`block_mesh`], plus [`MeshError::DictParse`] wrapping any I/O error.

```rust
pub fn block_mesh_from_file</* synthetic */ impl AsRef<Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>) -> Result<PolyMesh, crate::MeshError> { /* ... */ }
```

## Module `block_primitives`

Structured hex meshes of simple primitives, first a **pipe** (cylinder) and a
**box**, generated as `blockMeshDict` text and built by this crate's
[`block_mesh`](crate::block_mesh) port (gh:#1155, Foam Studio stage 1).

# The pipe O-grid

A central `n × n` block (half-width `square_frac · R`) and `4 n` outer
blocks of `nr × 1` cells, one per circumferential cell. Every block edge is
straight and every outer vertex lies on the circle, so the wall is a
`4n`-gon inscribed in it: exactly what any finite-volume mesh of a circle
is, and blockMesh's missing arc edges are never needed. Moved here from the
stage 0 harness (`outram-foam-appbuilder-lib/verification_and_validation/
wasm_pimple_3d_pipe`, gh:#1154), where it was measured: max
non-orthogonality 19.7 / 26.0 / 29.7° at 5k / 20k / 50k cells, no negative
cells, volume equal to the inscribed-polygon prism to round-off.

**Why this crate.** The blockMesh port lives here, and `outram-blender`'s
`convert::block_mesh` only calls it; `outram_blender::foam_mesh` triangulates
surfaces for cfMesh, which gives wrong pipe flow (gh:#1160). A generator
that writes a blockMeshDict belongs beside the mesher that reads it, and it
keeps the solver path free of `outram-blender`.

# The annulus and the double pipe (gh:#1156)

[`Annulus`]: `n_theta` blocks of `nr × 1 × nz` cells between two
polygons, inner radius `r_inner` and outer `r_outer`, every vertex on its
circle at the O-grid's angles `θ_k = −3π/4 + 2πk/n_theta`. With
`n_theta = 4n` and `r_inner` equal to a [`PipeOGrid`]'s radius, the
annulus's inner wall has exactly the pipe's wall vertices, so the two
meshes meet face to face (conformal), which is what chtMultiRegionFoam's
coupled interface needs. [`DoublePipe`] builds the three regions of a
double-pipe heat exchanger that way: the inner fluid (an O-grid), the
tube wall (an annulus) and the outer fluid (an annulus), all with `4n`
cells around and the same axial layers.

# The helical pipe (gh:#1157)

[`HelicalPipe`]: a [`PipeOGrid`] as long as the coil's tube, its points
mapped onto the helix on a rotation-minimising frame (no twist between
layers); same topology and cell order as the straight grid. Not a
blockMeshDict: blockMesh's straight block edges cannot follow a helix
without one block per layer, and the point map is exact.

# The box

One hex block, `nx × ny × nz`, axis-aligned.

# Patches

Each primitive face carries a patch name chosen by the caller. Faces with
the same name go into one patch; a name containing `wall` gets
`type wall`, any other `type patch`. Patches appear in first-use order.

```rust
pub mod block_primitives { /* ... */ }
```

### Types

#### Struct `PipeOGrid`

A circular pipe along +z, as an O-grid.

```rust
pub struct PipeOGrid {
    pub radius: uom::si::f64::Length,
    pub length: uom::si::f64::Length,
    pub origin: [uom::si::f64::Length; 3],
    pub n: usize,
    pub nr: usize,
    pub nz: usize,
    pub square_frac: f64,
    pub patch_names: [String; 3],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `uom::si::f64::Length` | Pipe radius. |
| `length` | `uom::si::f64::Length` | Pipe length; the pipe runs from `origin.z` to `origin.z + length`. |
| `origin` | `[uom::si::f64::Length; 3]` | Centre of the `z`-minimum end face. |
| `n` | `usize` | Cells along each side of the central square (4 n around the wall). |
| `nr` | `usize` | Radial cells between the square and the wall. |
| `nz` | `usize` | Axial cells. |
| `square_frac` | `f64` | Half-width of the central square as a fraction of the radius, in (0, 1/√2). |
| `patch_names` | `[String; 3]` | Patch names of the `z`-min end, the `z`-max end and the side wall. |

##### Implementations

###### Methods

- ```rust
  pub fn new(radius: Length, length: Length, n: usize, nr: usize, nz: usize) -> Self { /* ... */ }
  ```
  A pipe at the origin with patches `inlet` (z-min), `outlet` (z-max) and

- ```rust
  pub fn level(radius: Length, length: Length, m: usize) -> Self { /* ... */ }
  ```
  The geometrically similar family used for grid studies and the demo:

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of cells: `(n² + 4 n nr) nz`.

- ```rust
  pub fn polygon_area(self: &Self) -> f64 { /* ... */ }
  ```
  The cross-section area of the inscribed `4n`-gon, `½ (4n) R² sin(2π/4n)`.

- ```rust
  pub fn block_mesh_dict(self: &Self) -> String { /* ... */ }
  ```
  The blockMeshDict text.

- ```rust
  pub fn mesh(self: &Self) -> Result<PolyMesh, MeshError> { /* ... */ }
  ```
  Build the mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Annulus`

A circular annulus along +z between radii `r_inner` and `r_outer`, as
`n_theta` blocks of `nr × 1 × nz` cells; see the module docs for the
vertex angles (shared with [`PipeOGrid`]).

```rust
pub struct Annulus {
    pub r_inner: uom::si::f64::Length,
    pub r_outer: uom::si::f64::Length,
    pub length: uom::si::f64::Length,
    pub origin: [uom::si::f64::Length; 3],
    pub n_theta: usize,
    pub nr: usize,
    pub nz: usize,
    pub patch_names: [String; 4],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r_inner` | `uom::si::f64::Length` | Inner radius (the inner polygon's vertices lie on this circle). |
| `r_outer` | `uom::si::f64::Length` | Outer radius. |
| `length` | `uom::si::f64::Length` | Length; the annulus runs from `origin.z` to `origin.z + length`. |
| `origin` | `[uom::si::f64::Length; 3]` | Centre of the `z`-minimum end. |
| `n_theta` | `usize` | Cells around (4 n to meet a [`PipeOGrid`] of side n). |
| `nr` | `usize` | Radial cells. |
| `nz` | `usize` | Axial cells. |
| `patch_names` | `[String; 4]` | Patch names of the `z`-min end, the `z`-max end, the inner wall and<br>the outer wall. |

##### Implementations

###### Methods

- ```rust
  pub fn new(r_inner: Length, r_outer: Length, length: Length, n_theta: usize, nr: usize, nz: usize) -> Self { /* ... */ }
  ```
  An annulus at the origin with patches `inlet`, `outlet`, `inner_wall`

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of cells: `n_theta · nr · nz`.

- ```rust
  pub fn polygon_area(self: &Self) -> f64 { /* ... */ }
  ```
  Cross-section area between the two inscribed `n_theta`-gons.

- ```rust
  pub fn block_mesh_dict(self: &Self) -> String { /* ... */ }
  ```
  The blockMeshDict text.

- ```rust
  pub fn mesh(self: &Self) -> Result<PolyMesh, MeshError> { /* ... */ }
  ```
  Build the mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DoublePipe`

The three conformal region meshes of a double-pipe heat exchanger along
+z: the inner fluid (`r < r_tube_inner`), the tube wall
(`r_tube_inner < r < r_tube_outer`) and the outer (annulus) fluid
(`r_tube_outer < r < r_shell`), each with `4 n` cells around and `nz`
axial layers, so every interface is face to face.

Patch names (the defaults; the interface patch has the same name on both
sides):

| region | z-min | z-max | inner | outer |
|---|---|---|---|---|
| inner fluid | `inlet` | `outlet` | — | `tube_inner_wall` |
| tube wall | `tube_ends` | `tube_ends` | `tube_inner_wall` | `tube_outer_wall` |
| outer fluid | `outlet` | `inlet` | `tube_outer_wall` | `shell_wall` |

The outer fluid's inlet is at z-max: counterflow.

```rust
pub struct DoublePipe {
    pub r_tube_inner: uom::si::f64::Length,
    pub r_tube_outer: uom::si::f64::Length,
    pub r_shell: uom::si::f64::Length,
    pub length: uom::si::f64::Length,
    pub origin: [uom::si::f64::Length; 3],
    pub n: usize,
    pub nr: [usize; 3],
    pub nz: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r_tube_inner` | `uom::si::f64::Length` | Inner radius of the tube (the inner fluid's radius). |
| `r_tube_outer` | `uom::si::f64::Length` | Outer radius of the tube. |
| `r_shell` | `uom::si::f64::Length` | Inner radius of the shell (the outer fluid's outer radius). |
| `length` | `uom::si::f64::Length` | Length. |
| `origin` | `[uom::si::f64::Length; 3]` | Centre of the z-min end. |
| `n` | `usize` | O-grid side: `4 n` cells around every region. |
| `nr` | `[usize; 3]` | Radial cells: inner fluid ring (beyond its central square), tube<br>wall, outer fluid. |
| `nz` | `usize` | Axial cells. |

##### Implementations

###### Methods

- ```rust
  pub fn n_cells(self: &Self) -> [usize; 3] { /* ... */ }
  ```
  Number of cells in each region (inner fluid, tube wall, outer fluid).

- ```rust
  pub fn region_grids(self: &Self) -> (PipeOGrid, Annulus, Annulus) { /* ... */ }
  ```
  The three region grids, positioned and named (see the type docs).

- ```rust
  pub fn meshes(self: &Self) -> Result<[PolyMesh; 3], MeshError> { /* ... */ }
  ```
  Build the three meshes (inner fluid, tube wall, outer fluid).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `HelicalPipe`

A circular pipe coiled into a helix (gh:#1157, the CFD coil section): a
[`PipeOGrid`] of the tube's arc length, its points swept along the helix
centreline on a **rotation-minimising frame**, so successive cross-sections
are not twisted about the tube axis.

# Geometry

The helix axis is +z through `axis_origin`; the centreline starts at
`axis_origin + (R, 0, 0)` and winds counter-clockwise seen from +z,
`C(φ) = (R cos φ, R sin φ, c φ)`, with `R = D_w/2` (half the coil
diameter at the tube centreline), `c = h/(2π)` (`h` the pitch, rise per
turn) and arc length `s = ℓ φ`, `ℓ = √(R² + c²)`. Its Frenet frame is

- `T = (−R sin φ, R cos φ, c)/ℓ`,
- `N = (−cos φ, −sin φ, 0)`: towards the helix axis (the inner side of the
  bend),
- `B = T × N = (c sin φ, −c cos φ, R)/ℓ`,

with constant curvature `κ = R/ℓ²` and torsion `τ = c/ℓ²`
(Frenet–Serret: `T' = κN`, `N' = −κT + τB`, `B' = −τN`). So the
**curvature diameter** is `1/(κ/2) = D_w [1 + (h/(π D_w))²]`, the same
formula as `tampines::components::helical_coil_otsg::correlations::curvature_diameter`
(VDI Heat Atlas Gc, there), derived here from κ rather than called, because
this crate does not depend on `tampines`.

**Why not the Frenet frame.** It turns about `T` at the rate τ, so a
cross-section swept on it is rotated by `τ Δs` against its neighbour a
distance `Δs` along: over one turn (`τ · 2πℓ = h/ℓ` rad) of a `h = 2d`,
`D_w = 10d` coil that is 23°, and every cell between layers is sheared by
it. The rotation-minimising
(Bishop) frame `U = cos θ N + sin θ B`, `V = −sin θ N + cos θ B` with
`θ = −τ s` has `U' = −κ cos θ T` and `V' = κ sin θ T`, i.e. no component
about `T` (derived from Frenet–Serret above; the frame is Bishop's, R. L.
Bishop (1975), "There is more than one way to frame a curve", *Amer. Math.
Monthly* 82(3), 246–251, doi:10.2307/2319846, cited for the idea, not
read in this session). The test `helix_frames_are_orthonormal_and_untwisted`
checks `U'·V = 0` numerically.

# Mesh

[`PipeOGrid::new`]`(radius, arc_length, n, nr, n_layers)`, patches
`patch_names` (inlet at `s = 0`, outlet at `s = L`, the tube wall), every
point `(x, y, z)` mapped to `C(z) + x U(z) + y V(z)`. The topology, cell
order and patch faces are the straight grid's, so cell `i` of
[`HelicalPipe::mesh`] is cell `i` of [`HelicalPipe::straight_grid`]'s
mesh (the blockMesh port's cell order is not layer by layer, so use
[`HelicalPipe::locate`] for a cell's arc position). Faces between layers
are bilinear, not planar; the face-area vectors of the FV mesh are the
usual triangle-fan sums.

With `pitch = 0` the coil is a torus section; turns must then not exceed 1
or the tube overlaps itself (`h ≥ 2 r_tube` is the general no-overlap
condition, which [`HelicalPipe::check`] enforces for more than one turn).

```rust
pub struct HelicalPipe {
    pub radius: uom::si::f64::Length,
    pub coil_diameter: uom::si::f64::Length,
    pub pitch: uom::si::f64::Length,
    pub turns: f64,
    pub axis_origin: [uom::si::f64::Length; 3],
    pub n: usize,
    pub nr: usize,
    pub n_layers: usize,
    pub square_frac: f64,
    pub patch_names: [String; 3],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `uom::si::f64::Length` | Tube (inner) radius. |
| `coil_diameter` | `uom::si::f64::Length` | Coil diameter `D_w` at the tube centreline. |
| `pitch` | `uom::si::f64::Length` | Pitch `h`: rise of the centreline per turn. |
| `turns` | `f64` | Number of turns (any positive real). |
| `axis_origin` | `[uom::si::f64::Length; 3]` | The point of the helix axis at the inlet's height. |
| `n` | `usize` | O-grid side (`4 n` cells around the wall). |
| `nr` | `usize` | Radial cells between the O-grid square and the wall. |
| `n_layers` | `usize` | Cells along the tube. |
| `square_frac` | `f64` | Half-width of the O-grid's central square over the radius. |
| `patch_names` | `[String; 3]` | Patch names of the inlet (`s = 0`), outlet (`s = L`) and wall. |

##### Implementations

###### Methods

- ```rust
  pub fn new(radius: Length, coil_diameter: Length, pitch: Length, turns: f64, n: usize, nr: usize, n_layers: usize) -> Self { /* ... */ }
  ```
  A coil with its axis on the z axis, patches `inlet`, `outlet`, `walls`

- ```rust
  pub fn check(self: &Self) -> Result<(), MeshError> { /* ... */ }
  ```
  Why the coil cannot be meshed, if it cannot: positive sizes, the tube

- ```rust
  pub fn arc_length(self: &Self) -> Length { /* ... */ }
  ```
  Developed length of the tube, `turns · √((π D_w)² + h²)`.

- ```rust
  pub fn curvature(self: &Self) -> f64 { /* ... */ }
  ```
  Curvature `κ = R/(R² + c²)` \[1/m\].

- ```rust
  pub fn torsion(self: &Self) -> f64 { /* ... */ }
  ```
  Torsion `τ = c/(R² + c²)` \[1/m\].

- ```rust
  pub fn curvature_diameter(self: &Self) -> Length { /* ... */ }
  ```
  Curvature diameter `2/κ = D_w [1 + (h/(π D_w))²]`.

- ```rust
  pub fn cells_per_section(self: &Self) -> usize { /* ... */ }
  ```
  Cells in one cross-section, `n² + 4 n nr`.

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of cells.

- ```rust
  pub fn centreline(self: &Self, s: Length) -> [f64; 3] { /* ... */ }
  ```
  Centreline point at arc length `s` \[m\].

- ```rust
  pub fn frenet(self: &Self, s: Length) -> [[f64; 3]; 3] { /* ... */ }
  ```
  The Frenet frame `[T, N, B]` at arc length `s`.

- ```rust
  pub fn frame(self: &Self, s: Length) -> [[f64; 3]; 3] { /* ... */ }
  ```
  The rotation-minimising frame `[T, U, V]` at `s` (see the type docs),

- ```rust
  pub fn straight_grid(self: &Self) -> PipeOGrid { /* ... */ }
  ```
  The straight O-grid that is swept: radius, arc length, `n`, `nr`,

- ```rust
  pub fn mesh(self: &Self) -> Result<PolyMesh, MeshError> { /* ... */ }
  ```
  Build the mesh: [`HelicalPipe::straight_grid`]'s mesh with every

- ```rust
  pub fn locate(self: &Self, p: [f64; 3]) -> CoilPoint { /* ... */ }
  ```
  The nearest centreline point to `p` \[m\] and `p`'s Frenet offsets

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CoilPoint`

Where a point lies relative to a [`HelicalPipe`]'s centreline:
arc position and Frenet coordinates of its offset from `C(s)`.

```rust
pub struct CoilPoint {
    pub s: f64,
    pub n: f64,
    pub b: f64,
    pub frenet: [[f64; 3]; 3],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `s` | `f64` | Arc length of the nearest centreline point \[m\]. |
| `n` | `f64` | Offset along `N` (positive towards the helix axis, the inner side) \[m\]. |
| `b` | `f64` | Offset along `B` \[m\]. |
| `frenet` | `[[f64; 3]; 3]` | The Frenet frame at `s`: `T`, `N`, `B`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BoxBlock`

An axis-aligned box as one hex block.

```rust
pub struct BoxBlock {
    pub origin: [uom::si::f64::Length; 3],
    pub size: [uom::si::f64::Length; 3],
    pub cells: [usize; 3],
    pub patch_names: [String; 6],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `origin` | `[uom::si::f64::Length; 3]` | Minimum corner. |
| `size` | `[uom::si::f64::Length; 3]` | Edge lengths along x, y, z. |
| `cells` | `[usize; 3]` | Cells along x, y, z. |
| `patch_names` | `[String; 6]` | Patch names of the faces x-min, x-max, y-min, y-max, z-min, z-max. |

##### Implementations

###### Methods

- ```rust
  pub fn new(size: [Length; 3], cells: [usize; 3]) -> Self { /* ... */ }
  ```
  A box at the origin with `inlet` on z-min, `outlet` on z-max and

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of cells.

- ```rust
  pub fn block_mesh_dict(self: &Self) -> String { /* ... */ }
  ```
  The blockMeshDict text.

- ```rust
  pub fn mesh(self: &Self) -> Result<PolyMesh, MeshError> { /* ... */ }
  ```
  Build the mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Constants and Statics

#### Constant `DEFAULT_SQUARE_FRAC`

Fraction of R used by default for the central square's half-width.

```rust
pub const DEFAULT_SQUARE_FRAC: f64 = 0.55;
```

## Module `block_projection`

`blockMesh` projection: the `geometry` dictionary (searchable surfaces),
`project` vertices, `project` / `projectCurve` edges, `project` and
`sweep` block faces, and the curved-face correction passes of
`block::createPoints` (gh:#1462, epic gh:#1458).

## Scope

Searchable surfaces: `sphere` (`searchableSphere`), `cylinder`
(`searchableCylinder`) and `plane` (`searchablePlane`), the shapes the
`mesh/blockMesh/{sphere7ProjectedEdges,pipe}` tutorials use besides
`extrudedCircle` and `plate`. Any other `type` (including `triSurface`) is
refused with [`MeshError::NotImplemented`] naming it; nothing is skipped.

## Dev against v2506

The specification is OpenFOAM-dev `852527377c7e`. Its `projectFace` loop
stops after adding the i residual and testing the combined residual; the
v2506 loop projects, tests the previous residual and stops on a projected
point. Both are implemented ([`ProjectionFlavour`]); the default is dev.
The multi-surface `findNearest` is dev's (alternate surfaces, intersect the
tangent planes); v2506 additionally clips the step, see
`verification_and_validation/block_mesh_projection.md`.

Coordinates are the raw dictionary coordinates (before `convertToMeters`):
upstream projects before scaling.

Transcendentals (`sin`, `cos`, `asin` in `sweep`) go through PETIR
`RealMath`; `sqrt` is exact.

```rust
pub mod block_projection { /* ... */ }
```

### Types

#### Enum `SearchableSurface`

A searchable surface (enum dispatch, no trait objects).

```rust
pub enum SearchableSurface {
    Sphere {
        centre: outram_foam_basic_lib::primitives::Vector3,
        radius: f64,
    },
    Cylinder {
        point1: outram_foam_basic_lib::primitives::Vector3,
        point2: outram_foam_basic_lib::primitives::Vector3,
        radius: f64,
    },
    Plane {
        point: outram_foam_basic_lib::primitives::Vector3,
        normal: outram_foam_basic_lib::primitives::Vector3,
    },
}
```

##### Variants

###### `Sphere`

`searchableSurfaces::sphere`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `centre` | `outram_foam_basic_lib::primitives::Vector3` | Centre. |
| `radius` | `f64` | Radius. |

###### `Cylinder`

`searchableSurfaces::cylinder` (finite, with end caps).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `point1` | `outram_foam_basic_lib::primitives::Vector3` | Axis start. |
| `point2` | `outram_foam_basic_lib::primitives::Vector3` | Axis end. |
| `radius` | `f64` | Radius. |

###### `Plane`

`searchableSurfaces::plane` (infinite).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `point` | `outram_foam_basic_lib::primitives::Vector3` | A point on the plane. |
| `normal` | `outram_foam_basic_lib::primitives::Vector3` | Unit normal. |

##### Implementations

###### Methods

- ```rust
  pub fn plane(point: Vector3, normal: Vector3) -> Self { /* ... */ }
  ```
  `Foam::plane(basePoint, normalVector)`: the normal is normalised.

- ```rust
  pub fn find_nearest(self: &Self, sample: Vector3, dist_sqr: f64, flavour: ProjectionFlavour) -> Option<Vector3> { /* ... */ }
  ```
  `findNearest(sample, nearestDistSqr)` for one sample: the nearest point

- ```rust
  pub fn normal_at(self: &Self, hit: Vector3, flavour: ProjectionFlavour) -> Vector3 { /* ... */ }
  ```
  `getNormal` at a point returned by [`find_nearest`](Self::find_nearest).

- ```rust
  pub fn bounds(self: &Self) -> (Vector3, Vector3) { /* ... */ }
  ```
  `bounds()`: `(min, max)`; the plane's are `-+vGreat` except along an

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Geometry`

`searchableSurfaceList`: the named surfaces of the `geometry` dictionary.

```rust
pub struct Geometry {
    pub surfaces: Vec<(String, SearchableSurface)>,
    pub flavour: ProjectionFlavour,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `surfaces` | `Vec<(String, SearchableSurface)>` | `(name, surface)` in dictionary order. |
| `flavour` | `ProjectionFlavour` | Which OpenFOAM's arithmetic and loops to follow. |

##### Implementations

###### Methods

- ```rust
  pub fn find_surface_id(self: &Self, name: &str) -> Option<usize> { /* ... */ }
  ```
  `findSurfaceID`.

- ```rust
  pub fn surface_ids(self: &Self, names: &[String]) -> Result<Vec<usize>, MeshError> { /* ... */ }
  ```
  Ids of the listed surface names.

- ```rust
  pub fn bounds(self: &Self, ids: &[usize]) -> (Vector3, Vector3) { /* ... */ }
  ```
  `searchableSurfacesQueries::bounds`.

- ```rust
  pub fn find_nearest(self: &Self, ids: &[usize], start: &[Vector3], dist_sqr: &[f64], n_iter: usize) -> Vec<Vector3> { /* ... */ }
  ```
  `searchableSurfacesQueries::findNearest` (multi-surface form, dev): the

- ```rust
  pub fn project_vertex(self: &Self, ids: &[usize], p: Vector3) -> Vector3 { /* ... */ }
  ```
  `blockVertices::projectVertex::operator point()`.

- ```rust
  pub fn from_entries(entries: &[(String, DictEntry)], flavour: ProjectionFlavour) -> Result<Self, MeshError> { /* ... */ }
  ```
  Build the surface list from the body of the `geometry { … }` entry.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `ProjectEdge`

A `project` or `projectCurve` block edge: the straight line between the two
vertices projected onto the listed surfaces.

```rust
pub struct ProjectEdge {
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
  pub fn new(geometry: Arc<Geometry>, surfaces: Vec<usize>, p0: Vector3, p1: Vector3) -> Self { /* ... */ }
  ```
  Build the edge. `projectCurve` shares the algorithm of `project` when

- ```rust
  pub fn position(self: &Self, lambda: f64) -> Vector3 { /* ... */ }
  ```
  `projectEdge::position(lambda)`: the straight-line point projected once,

- ```rust
  pub fn positions(self: &Self, lambdas: &[f64]) -> Vec<Vector3> { /* ... */ }
  ```
  `projectEdge::position(const scalarList&)`.

- ```rust
  pub fn positions_counted(self: &Self, lambdas: &[f64]) -> (Vec<Vector3>, usize) { /* ... */ }
  ```
  As [`positions`](Self::positions), also returning the number of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `ProjectionFlavour`

Which OpenFOAM's projection code to follow where dev and v2506 differ.

The differences found (gh:#1462): the `projectFace` loop, the multi-surface
`findNearest` (step clipping, collinearity test), the sphere's nearest point
(`n/magN*radius` against `radius*n/magN`, one ulp), its normal, and the
`linearInterpolationWeights` (`1 - f, f` against `(t1 - t)/dT, 1 - w0`).

```rust
pub enum ProjectionFlavour {
    Dev,
    Esi,
}
```

##### Variants

###### `Dev`

OpenFOAM-dev `852527377c7e` (the specification, and the default).

###### `Esi`

OpenFOAM.com v2506 (the binary available for comparison).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `BlockFaceKind`

What a `faces` entry does.

```rust
pub enum BlockFaceKind {
    Project(String),
    Sweep(f64),
}
```

##### Variants

###### `Project`

`project (v0 v1 v2 v3) surfaceName`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Sweep`

`sweep (v0 v1 v2 v3) rotationFraction`.

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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BlockFace`

One `faces` entry: the four vertex labels and what to do to the face.

```rust
pub struct BlockFace {
    pub vertices: [usize; 4],
    pub kind: BlockFaceKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vertices` | `[usize; 4]` | The face's vertex labels, in the user's order. |
| `kind` | `BlockFaceKind` | The operation. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `DictEntry`

One entry of a parsed dictionary body.

```rust
pub enum DictEntry {
    Tokens(Vec<String>),
    Dict(Vec<(String, DictEntry)>),
}
```

##### Variants

###### `Tokens`

`key tokens… ;`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<String>` |  |

###### `Dict`

`key { … }`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<(String, DictEntry)>` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `project_face`

Dev `projectFace::project` (and its v2506 variant) on the face points
`points` (`n.0 * n.1`, first index fastest), with `n = (n.first, n.second)`.

```rust
pub fn project_face(geometry: &Geometry, surface: usize, n: (usize, usize), points: &mut [outram_foam_basic_lib::primitives::Vector3]) { /* ... */ }
```

#### Function `sweep_face`

`sweepFace::project`: `points` are the face points of block face
`block_face_i` (point `(row, col)` at `row * n_cols + col`, transposed when
the user's vertex order starts along the other direction);
`transpose` is decided by the caller from the vertex order
(see [`sweep_transpose`]).

```rust
pub fn sweep_face(density: [usize; 3], block_face_i: usize, transpose: bool, rotation_fraction: f64, points: &mut [outram_foam_basic_lib::primitives::Vector3]) { /* ... */ }
```

#### Function `sweep_transpose`

The `transpose` decision of `sweepFace::project`: look along the user's face
loop for the first edge equal to the block face's anchor edge; transpose
when it is an odd edge of the loop.

`block_labels` are the block's 8 vertex labels, `block_face_i` the hex face
index, `user_face` the `sweep (…)` vertex labels.

```rust
pub fn sweep_transpose(block_labels: &[usize; 8], block_face_i: usize, user_face: &[usize; 4]) -> bool { /* ... */ }
```

#### Function `find_curved_faces`

`blockDescriptor::findCurvedFaces`: for each hex face, the index of the
first `faces` entry covering the same four vertices.

```rust
pub fn find_curved_faces(block_labels: &[usize; 8], faces: &[BlockFace]) -> [Option<usize>; 6] { /* ... */ }
```

#### Function `apply_curved_faces`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {tool: clippy#0,\nlint_name: \"too_many_arguments\", kind: Allow}])]")`

The curved-face half of `block::createPoints` (`blockCreate.C`): extract the
six face point sets, project/sweep the curved ones
(`blockDescriptor::correctFacePoints`), then apply the i-, j- and k-face
corrections to the block points in place.

`w` are the 12 edge weight arrays (`edgesPointsWeights`), `n` the cell
counts, `points` the block's `(n0+1)(n1+1)(n2+1)` points, `i` fastest.

# Errors
[`MeshError::DictParse`] for a `project` face naming a surface that is not
in `geometry`.

```rust
pub fn apply_curved_faces(geometry: &Geometry, faces: &[BlockFace], curved: &[Option<usize>; 6], block_labels: &[usize; 8], n: [usize; 3], w: &[Vec<f64>; 12], points: &mut [outram_foam_basic_lib::primitives::Vector3]) -> Result<(), crate::MeshError> { /* ... */ }
```

#### Function `read_dict_body`

Read `{ key value; key { … } … }` starting at the `{` at `*pos`.

# Errors
[`MeshError::DictParse`] on an unterminated dictionary.

```rust
pub fn read_dict_body(tokens: &[String], pos: &mut usize) -> Result<Vec<(String, DictEntry)>, crate::MeshError> { /* ... */ }
```

## Module `section`

Plane sections and boundary edges of a [`PolyMesh`], for drawing the mesh
the solver gets and the cell fields on it (gh:#1155, Foam Studio).

[`plane_section`] cuts every cell with an axis-aligned plane: each face
edge that crosses the plane gives a point to both cells of the face, and a
cell's section is the convex hull of its points. That is exact for convex
cells (every hex of [`block_mesh`](crate::block_mesh)); for a non-convex
polyhedron it is the hull of the true section. A vertex lying on the plane
counts as above it, so a plane through a layer of faces gives one polygon
per cell below it rather than two.

```rust
pub mod section { /* ... */ }
```

### Types

#### Enum `Axis`

The normal of an axis-aligned cutting plane.

```rust
pub enum Axis {
    X,
    Y,
    Z,
}
```

##### Variants

###### `X`

Plane x = const; in-plane coordinates (y, z).

###### `Y`

Plane y = const; in-plane coordinates (x, z).

###### `Z`

Plane z = const; in-plane coordinates (x, y).

##### Implementations

###### Methods

- ```rust
  pub fn in_plane(self: Self, p: Vector3) -> [f64; 2] { /* ... */ }
  ```
  The two in-plane coordinates of `p`, in metres.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CellSection`

One cell's section: the cell index and its polygon (counter-clockwise,
in-plane coordinates in metres, see [`Axis`]).

```rust
pub struct CellSection {
    pub cell: usize,
    pub polygon: Vec<[f64; 2]>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell` | `usize` | Cell index in the mesh (the solver's cell order). |
| `polygon` | `Vec<[f64; 2]>` | Polygon vertices \[m\]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `plane_section`

Cut `mesh` with the plane `axis = offset`; one [`CellSection`] per cell the
plane passes through, in cell order.

```rust
pub fn plane_section(mesh: &crate::block_mesh::PolyMesh, axis: Axis, offset: uom::si::f64::Length) -> Vec<CellSection> { /* ... */ }
```

#### Function `boundary_edges`

Every distinct edge of the boundary faces, as two points \[m\] and the
index of the patch it was first met on: the wireframe of what the solver
meshes.

```rust
pub fn boundary_edges(mesh: &crate::block_mesh::PolyMesh) -> Vec<([f64; 3], [f64; 3], usize)> { /* ... */ }
```

#### Function `polygon_area`

Area of a polygon (shoelace), in the square of its coordinates' unit.

```rust
pub fn polygon_area(p: &[[f64; 2]]) -> f64 { /* ... */ }
```

## Module `driver`

**Start here.** The one-call driver: a closed surface in, an OpenFOAM
`polyMesh` out.

Everything else in this crate is a component of this pipeline (or a
different tool entirely — see [`crate::block_mesh`],
[`crate::ideas_unv_to_foam`], [`crate::poly_dual_mesh`]). If you have a
geometry and want a mesh, call [`mesh_from_surface`] and nothing else:

```no_run
use outram_foam_mesh::driver::{mesh_from_surface, MeshingControls, MeshingPhases};
use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;
use outram_foam_basic_lib::primitives::Vector3;

// A unit sphere, meshed as the obstacle in an external-flow domain.
let surface = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 24, 24);
let controls = MeshingControls::external_flow([12, 12, 12], 2)
    .with_phases(MeshingPhases::CastellateSnap);

let result = mesh_from_surface(&surface, &controls).unwrap();
println!("{}", result.quality.summary());
result.write_case("my_case").unwrap();   // → my_case/constant/polyMesh
```

## What the pipeline does

1. Wraps the surface in a uniform hexahedral **background mesh** — either
   the [`MeshingControls::domain`] you give, or the surface's bounding box
   grown by [`MeshingControls::domain_margin`].
2. **Castellates** it (octree refinement toward the surface, then carving
   away the side you did not keep) — see [`crate::snappy_hex_mesh::castellation`].
3. **Snaps** the carved staircase onto the surface, if the phase enum asks
   for it — see [`crate::snappy_hex_mesh::snapping`].
4. **Adds boundary layers**, if the phase enum asks for it — see
   [`crate::snappy_hex_mesh::layers`], and read the honest caveat below.
5. Converts to [`PolyMesh`] and grades the result with
   [`assess_quality`](crate::mesh_quality::assess_quality).

## Honest caveat on layer addition

The layer phase in this crate is **restricted**: it has two placements, and
which one a given case gets is decided inside the layer phase by a
watertightness gate, not by you.

- [`LayerOutcome::InsertedInterior`] — the real `snappyLayerDriver`
  behaviour: the near-wall mesh is shrunk *inward* and prisms fill the gap,
  so the meshed volume is conserved exactly and the wall stays where the
  geometry says it is.
- [`LayerOutcome::ExtrudedOutward`] — the fallback for regions where
  displacing a wall point would open an octree hanging-node gap. The prism
  block grows *outward* along the wall normal, so the meshed domain gets
  **bigger**: for an external-flow case the obstacle shrinks, which is not
  the mesh you asked for. It is watertight and inversion-free, but it is
  not a faithful boundary layer.

The driver detects which happened by measuring the change in total meshed
volume and reports it in [`GeneratedMesh::layers`] — check it, do not
assume. Measured on the built-in sphere-in-box case (2026-08-07): a
`12×12×12` background at refinement level 2 gets `InsertedInterior`
(volume conserved to `< 1e-9` relative), while an `8×8×8` background at
levels 1 and 2 falls back to `ExtrudedOutward` (volume grows by
`0.3–0.9 m³`). Either way, **no layered mesh from this driver has been
validated against OpenFOAM output**; treat the layer phase as the least
trustworthy part of the pipeline.

```rust
pub mod driver { /* ... */ }
```

### Types

#### Enum `MeshingPhases`

Which phases of the `snappyHexMesh` pipeline to run.

The phases are strictly cumulative — you cannot snap without castellating,
or add layers without snapping — so a single enum expresses every valid
combination. Mirrors the `castellatedMesh` / `snap` / `addLayers` switches
of a `snappyHexMeshDict`.

```rust
pub enum MeshingPhases {
    Castellate,
    CastellateSnap,
    CastellateSnapLayers,
}
```

##### Variants

###### `Castellate`

Phase 1 only: an octree-refined staircase mesh. Fast, always valid, and
the boundary is a "castle wall" — cell faces are axis-aligned, so the
mesh is perfectly orthogonal but geometrically crude near the surface.

###### `CastellateSnap`

Phases 1–2: castellate, then morph the boundary onto the surface. This
is the sensible default — body-fitted, and the phase pair with the
strongest V&V in this crate.

###### `CastellateSnapLayers`

Phases 1–3: also add prismatic boundary layers. **Read the module-level
caveat first** — the layer phase is restricted, and on some cases it
falls back to an outward extrusion that grows the domain rather than
inserting layers inside it. Always check
[`GeneratedMesh::layers`] afterwards.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `KeepRegion`

Which side of the closed input surface becomes the meshed fluid region.

Corresponds to `locationInMesh` in a `snappyHexMeshDict`, but stated as
intent rather than as a magic coordinate.

```rust
pub enum KeepRegion {
    Outside,
    Inside,
    At(outram_foam_basic_lib::primitives::Vector3),
}
```

##### Variants

###### `Outside`

Keep the region **outside** the surface: the surface becomes an
obstacle inside the background box (external flow around a body). The
driver seeds this from a corner of the domain and errors if that corner
turns out to be inside the surface.

###### `Inside`

Keep the region **inside** the surface: the surface becomes the domain
wall (internal flow through a duct or vessel). The driver seeds this
from the surface's bounding-box centre and errors if that point is not
actually inside — which happens for strongly non-convex geometry, in
which case use [`KeepRegion::At`].

###### `At`

Keep the region containing this explicit point `[m]` — the literal
`locationInMesh`. Use this whenever the two automatic choices cannot
find a valid seed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_foam_basic_lib::primitives::Vector3` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MeshingControls`

Everything the driver needs besides the geometry itself.

Build one with [`MeshingControls::external_flow`] or
[`MeshingControls::internal_flow`] and adjust with the `with_*` builders;
the per-phase structs ([`SnapControls`], [`LayerControls`]) are exposed
directly for fine control.

```rust
pub struct MeshingControls {
    pub domain: Option<crate::snappy_hex_mesh::background::Bounds>,
    pub domain_margin: f64,
    pub background_divisions: [usize; 3],
    pub refinement_level: usize,
    pub keep: KeepRegion,
    pub phases: MeshingPhases,
    pub surface_patch_name: String,
    pub snap: crate::snappy_hex_mesh::snapping::SnapControls,
    pub layers: crate::snappy_hex_mesh::layers::LayerControls,
    pub refinement: crate::snappy_hex_mesh::castellation::RefinementSelection,
    pub min_refine_cells: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `domain` | `Option<crate::snappy_hex_mesh::background::Bounds>` | The background domain `[m]`. `None` (the default) derives it from the<br>surface's bounding box grown by [`domain_margin`](Self::domain_margin).<br>Give it explicitly for an external-flow case where the far field must<br>reach a specific extent. |
| `domain_margin` | `f64` | When [`domain`](Self::domain) is `None`, the automatic domain is the<br>surface bounding box expanded on every side by this fraction of the<br>box's space diagonal (dimensionless, `>= 0`). Default `0.5`. |
| `background_divisions` | `[usize; 3]` | Background hex-cell divisions `[nx, ny, nz]` over the domain. These set<br>the far-field cell size; each must be `>= 1`. |
| `refinement_level` | `usize` | Octree refinement level applied near the surface. Level `n` cells are<br>`2ⁿ` times finer per axis than the background, so cost grows roughly as<br>`4ⁿ` (a surface shell is 2-D). Levels 0–3 are practical. |
| `keep` | `KeepRegion` | Which side of the surface to keep. |
| `phases` | `MeshingPhases` | Which phases to run. |
| `surface_patch_name` | `String` | Name of the boundary patch created on the carved surface. |
| `snap` | `crate::snappy_hex_mesh::snapping::SnapControls` | Snapping controls (ignored when [`phases`](Self::phases) is<br>[`MeshingPhases::Castellate`]). |
| `layers` | `crate::snappy_hex_mesh::layers::LayerControls` | Layer controls (ignored unless [`phases`](Self::phases) is<br>[`MeshingPhases::CastellateSnapLayers`]). |
| `refinement` | `crate::snappy_hex_mesh::castellation::RefinementSelection` | How cells are chosen for refinement (default: the historical distance<br>band; [`RefinementSelection::SurfaceIntersection`] is OpenFOAM's rule,<br>gh:#1465). The `snappyHexMesh` binary sets the latter. |
| `min_refine_cells` | `usize` | `minRefinementCells`, read only by the surface-intersection rule. |

##### Implementations

###### Methods

- ```rust
  pub fn with_surface_intersection_refinement(self: Self) -> Self { /* ... */ }
  ```
  Select refinement cells as OpenFOAM's `surfaceOnlyRefine` does

- ```rust
  pub fn external_flow(background_divisions: [usize; 3], refinement_level: usize) -> Self { /* ... */ }
  ```
  Controls for **flow around** the surface: the surface is an obstacle and

- ```rust
  pub fn internal_flow(background_divisions: [usize; 3], refinement_level: usize) -> Self { /* ... */ }
  ```
  Controls for **flow inside** the surface: the surface is the domain wall

- ```rust
  pub fn with_phases(self: Self, phases: MeshingPhases) -> Self { /* ... */ }
  ```
  Set which phases run (builder style).

- ```rust
  pub fn with_domain(self: Self, domain: Bounds) -> Self { /* ... */ }
  ```
  Set an explicit background domain instead of the automatic one.

- ```rust
  pub fn with_snap(self: Self, snap: SnapControls) -> Self { /* ... */ }
  ```
  Replace the snapping controls (builder style).

- ```rust
  pub fn with_layers(self: Self, layers: LayerControls) -> Self { /* ... */ }
  ```
  Replace the layer controls (builder style). Note this does **not**

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    External flow, `10×10×10` background, refinement level 2, castellate +

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `LayerOutcome`

What the layer phase actually did — an honest report, because the layer
phase has two placements and only one of them is the real thing.

See the module-level "Honest caveat on layer addition".

```rust
pub enum LayerOutcome {
    NotRequested,
    NoneAdded,
    InsertedInterior,
    ExtrudedOutward,
}
```

##### Variants

###### `NotRequested`

The layer phase was not run ([`MeshingPhases`] did not ask for it).

###### `NoneAdded`

The layer phase ran but added no cells — every candidate layer count
failed the watertightness / quality gate, so the unlayered mesh was
returned. This is a legitimate outcome, not an error.

###### `InsertedInterior`

Layers were inserted **inside** the domain: the near-wall mesh was
shrunk and prisms filled the gap, leaving the outer boundary where it
was. This is the correct `snappyLayerDriver` behaviour.

###### `ExtrudedOutward`

Layers were **extruded outward**: the prism block grew along the wall
normal and the meshed domain got larger. The watertight fallback for
hanging-node regions — correct as an extrusion, but not what an
external-flow case wants. Do not treat such a mesh as validated.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `GeneratedMesh`

The driver's output: the mesh, its quality, and what actually happened.

```rust
pub struct GeneratedMesh {
    pub mesh: outram_foam_basic_lib::io::PolyMesh,
    pub quality: crate::mesh_quality::MeshQualityReport,
    pub phases_run: MeshingPhases,
    pub layers: LayerOutcome,
    pub n_cells_castellated: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `outram_foam_basic_lib::io::PolyMesh` | The finished mesh in `constant/polyMesh` form. Write it with<br>[`write_case`](Self::write_case), or convert it to a solver-ready<br>[`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) with<br>[`PolyMesh::to_fv_mesh`]. |
| `quality` | `crate::mesh_quality::MeshQualityReport` | Quality metrics of [`mesh`](Self::mesh) — non-orthogonality, skewness,<br>aspect ratio, inverted-cell count, with a one-word verdict. |
| `phases_run` | `MeshingPhases` | The phases that were requested and run. |
| `layers` | `LayerOutcome` | What the layer phase did (see [`LayerOutcome`]). |
| `n_cells_castellated` | `usize` | Cell count immediately after castellation, before snapping/layers — the<br>baseline the other counts are read against. |

##### Implementations

###### Methods

- ```rust
  pub fn write_case</* synthetic */ impl AsRef<std::path::Path>: AsRef<std::path::Path>>(self: &Self, case_dir: impl AsRef<std::path::Path>) -> Result<(), MeshError> { /* ... */ }
  ```
  Write an OpenFOAM case skeleton: `<case_dir>/constant/polyMesh/{points,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `mesh_from_surface`

**Generate a mesh from a closed surface.** The one function to call.

Runs the phases named by [`MeshingControls::phases`] and returns the
finished mesh together with its quality report.

# Inputs
- `surface` — a **closed**, outward-wound triangle soup in metres (read one
  with [`read_stl`](crate::snappy_hex_mesh::read_stl), or use a built-in
  like [`TriangleSoup::uv_sphere`]). Closedness matters: the region-removal
  step decides which cells to keep with a ray-cast inside/outside test, and
  an open surface makes that test meaningless.
- `controls` — domain, resolution, which side to keep, which phases to run.

# Returns
A [`GeneratedMesh`]: the [`PolyMesh`], its [`MeshQualityReport`], and a
[`LayerOutcome`] saying what the layer phase really did. The returned mesh
has always passed `FvMesh::validate` internally, but "valid" is not
"good" — check [`MeshQualityReport::verdict`].

# Errors
- [`MeshError::Construction`] if the surface is empty, if the automatic
  `locationInMesh` seed for [`KeepRegion::Outside`] / [`KeepRegion::Inside`]
  lands on the wrong side (use [`KeepRegion::At`] instead), if region
  removal discards every cell, or if any phase produces a mesh that fails
  validation.

# Cost
Dominated by castellation: roughly `n_background_cells + 8^level ·
n_surface_cells` inside/outside tests, each a brute-force ray cast over the
whole triangle soup (there is no spatial acceleration structure yet), so
cost scales linearly in the facet count. Keep the tessellation modest.

```rust
pub fn mesh_from_surface(surface: &crate::snappy_hex_mesh::stl::TriangleSoup, controls: &MeshingControls) -> Result<GeneratedMesh, crate::MeshError> { /* ... */ }
```

## Module `ideas_unv_to_foam`

`ideasUnvToFoam` — import an I-DEAS Universal File (`.unv`) mesh into a
`polyMesh`.

## What this does

Parses the three UNV dataset records needed to describe a finite-volume
mesh and assembles them into an [`outram_foam_basic_lib`] `FvMesh` (the
flat polyMesh: points, faces, owner/neighbour, boundary patches, and the
derived geometry — cell volumes/centres and face area-vectors/centres):

- **Dataset 2411** — nodes: a node label plus its `(x, y, z)` coordinate.
  Each node becomes a mesh *point*. Coordinates are read in the file's
  length unit and multiplied by an optional `scale` (metres per file unit;
  `1.0` if the file is already in metres).
- **Dataset 2412** — elements: an FE-descriptor id plus node connectivity.
  *Volume* elements become **cells**; *surface* elements are matched to
  boundary faces so they can be grouped into named patches.
- **Dataset 2467 / 2452 / 2435** — permanent groups: a named set of
  element labels. A group of surface elements becomes one **boundary
  patch** carrying that group's name.

## Supported element types

| FE descriptor | Element | Role | Nodes |
|---|---|---|---|
| 115 | linear brick | cell (hexahedron) | 8 |
| 112 | linear wedge | cell (prism) | 6 |
| 111 | linear tetrahedron | cell | 4 |
| (5-node fallback) | pyramid | cell | 5 |
| 44, 94 | linear quadrilateral / thin-shell quad | boundary face | 4 |
| 41, 91 | linear triangle / thin-shell triangle | boundary face | 3 |

**Deferred** (silently skipped, counted in [`UnvPolyMesh::n_skipped_elements`]):
parabolic / higher-order elements (descriptors 42, 45, 116, 118, …), beam /
rod 1-D elements (descriptors 11–32), and any descriptor not in the table
above (except the 5-node pyramid fallback). Pyramid has no universally
agreed UNV descriptor id, so any 5-node element is treated as a pyramid.

## Assembly

Each volume cell is decomposed into its bounding faces (outward-oriented via
the cell centroid). Faces shared by exactly two cells are merged into a
single *internal* face with `owner = min(cellA, cellB)`, `neighbour = max`,
and its area-vector oriented owner → neighbour. Faces belonging to a single
cell are *boundary* faces. Internal faces come first (upper-triangular
order: sorted by `(owner, neighbour)`), then boundary faces grouped by
patch; boundary faces matched to no group land in a trailing `defaultFaces`
patch. Face centres/areas and cell centres/volumes are computed with the
OpenFOAM `primitiveMesh` decomposition.

## Entry points

- [`convert`] — parse a UNV string (scale 1.0, i.e. file already in metres).
- [`convert_scaled`] — parse a UNV string with an explicit metre-per-unit scale.
- [`convert_file`] — read and parse a `.unv` file from disk.

```rust
pub mod ideas_unv_to_foam { /* ... */ }
```

### Types

#### Struct `UnvPolyMesh`

The result of importing a `.unv` file: a `polyMesh` in flat form.

Bundles the point cloud and the global face → point-index connectivity
(the parts `FvMesh` does not itself store) together with the assembled
[`FvMesh`] (topology + geometry). Together these are a complete polyMesh:
`points`, `faces`, owner/neighbour and boundary patches (in `fv_mesh`).

```rust
pub struct UnvPolyMesh {
    pub points: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub faces: Vec<Vec<usize>>,
    pub fv_mesh: outram_foam_basic_lib::mesh::FvMesh,
    pub n_skipped_elements: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Mesh points `[m]` — node coordinates, indexed by point id (0-based). |
| `faces` | `Vec<Vec<usize>>` | Global face list: each face is an ordered loop of point indices.<br>Length equals `fv_mesh.n_faces`; internal faces first, then boundary<br>faces grouped by patch. Owner-oriented (normal points owner → neighbour<br>for internal faces, outward for boundary faces). |
| `fv_mesh` | `outram_foam_basic_lib::mesh::FvMesh` | The assembled finite-volume mesh: owner/neighbour, patches, cell<br>volumes/centres, face area-vectors/centres. |
| `n_skipped_elements` | `usize` | Number of 2412 elements that were recognised but deferred (parabolic,<br>beam, or otherwise unsupported types). |

##### Implementations

###### Methods

- ```rust
  pub fn total_volume(self: &Self) -> f64 { /* ... */ }
  ```
  Total mesh volume `[m³]` — the sum of all cell volumes. A convenience for

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `convert`

Convert an I-DEAS `.unv` mesh (given as text) into a `polyMesh`, treating
the file's coordinates as already being in metres (scale `1.0`).

See the module docs for the supported dataset records and element types.

```rust
pub fn convert(unv_text: &str) -> Result<UnvPolyMesh, crate::MeshError> { /* ... */ }
```

#### Function `convert_scaled`

Convert an I-DEAS `.unv` mesh (given as text) into a `polyMesh`, scaling
every coordinate by `scale` [metres per file unit] — e.g. `1.0e-3` for a
file authored in millimetres.

```rust
pub fn convert_scaled(unv_text: &str, scale: f64) -> Result<UnvPolyMesh, crate::MeshError> { /* ... */ }
```

#### Function `convert_file`

Read a `.unv` file from disk and convert it into a `polyMesh` (scale `1.0`).

```rust
pub fn convert_file</* synthetic */ impl AsRef<Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>) -> Result<UnvPolyMesh, crate::MeshError> { /* ... */ }
```

## Module `check_mesh`

**The** `checkMesh` of this workspace (gh:#1480): OpenFOAM's mesh-quality
checks, once.

Before gh:#1480 these checks existed three times: `outram-park-fork-cfmesh`
`checks.rs` (the most complete, and the reference followed here),
[`crate::mesh_quality::assess_quality`] and
[`PolyPatchMesh::quality`](crate::snappy_hex_mesh::PolyPatchMesh::quality).
All three now call this module:

- [`check_quality`] -> [`QualityReport`]: every metric (non-orthogonality,
  internal and boundary skewness, aspect ratio, volumes, face pyramids,
  cell determinant);
- [`faces_failing_mesh_quality`]: the per-face `meshQualityControls` gate
  ([`MeshQualityControls`], defaults = `meshQualityDict`).

## What is shared and what is not

The *checks* are shared. The *geometry* they read is an input,
[`CheckGeometry`] (face area vectors and centres, cell centres and
volumes), because the three callers' meshes deliberately keep different
geometry: cfMesh uses vertex-average face centres, `outram-foam-mesh` uses
OpenFOAM's area-weighted ones (and sums the pyramids in different orders).
Unifying the geometry would move every pinned cfMesh mesh, so it is **not**
done here; it is listed as remaining work on gh:#1480.

Where the three copies differed, **each difference is an explicit option**
([`CheckOptions`]: [`BoundarySkewness`], [`SvHat`], the skewness guard,
[`DegenerateFaces`], and the aspect ratio reported in both forms), and every
caller passes the values that reproduce what it computed before, so no
number moved (maintainer decision, gh:#1480; bit-identity measured in
`tests/check_mesh_consolidation.rs` and the cfMesh pins). Which value
matches upstream dev `852527377c7e` is documented on each option; cfMesh
([`CheckOptions::default`]) deviates in `svHat`, the report's guard,
degenerate-face skipping and its aspect-ratio field; `outram-foam-mesh`
([`CheckOptions::foam_mesh_legacy`]) deviates in the boundary-skewness
definition (its mirrored form is identically zero, so the snapping gate
never sees a boundary skew). Whether callers should move to upstream's
definitions is a maintainer decision, filed as gh:#1486.

Pure Rust, wasm- and Android-safe.

```rust
pub mod check_mesh { /* ... */ }
```

### Types

#### Enum `BoundarySkewness`

Which definition of boundary-face skewness [`check_quality`] reports in
[`QualityReport::max_boundary_skewness`].

```rust
pub enum BoundarySkewness {
    NormalProjection,
    Mirrored,
}
```

##### Variants

###### `NormalProjection`

OpenFOAM-dev `meshCheck::boundaryFaceSkewness`: `d` is the normal
projection of `Cf - C_own`, floor `0.4 |d|`. Used by cfMesh's gate.

###### `Mirrored`

The owner centre mirrored across the face, `d = 2 (Cf - C_own)`,
floor `0.2 |d|`: what `outram-foam-mesh` computed before gh:#1480.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `SvHat`

How `svHat = sv / (|sv| + guard)` is evaluated in [`face_skewness`].

```rust
pub enum SvHat {
    Divide,
    ReciprocalMultiply,
}
```

##### Variants

###### `Divide`

`sv / (|sv| + guard)`: **what upstream does** (`meshCheck::faceSkewness`)
and what `outram-foam-mesh` did before gh:#1480.

###### `ReciprocalMultiply`

`sv * (1 / (|sv| + guard))`: what cfMesh did; differs from
[`SvHat::Divide`] in the last bit.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `DegenerateFaces`

What [`check_quality`] does with a degenerate internal face
(`|d| <= 1e-30` or `|Sf| <= 1e-30`).

```rust
pub enum DegenerateFaces {
    Skip,
    Include,
}
```

##### Variants

###### `Skip`

Skip it in non-orthogonality and skewness (cfMesh's behaviour).

###### `Include`

Evaluate it with the `+1e-300` denominators, so a zero `d` reads 90
degrees (`outram-foam-mesh`'s behaviour; upstream's `faceOrthogonality`
has no skip either).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CheckOptions`

Options of [`check_quality`]. Every difference between the three
pre-gh:#1480 copies is one explicit option here; each caller passes the
values that reproduce what it computed before, so no number moved. Which
value matches upstream dev `852527377c7e` is stated on each variant.

```rust
pub struct CheckOptions {
    pub boundary_skewness: BoundarySkewness,
    pub severe_non_ortho_deg: f64,
    pub sv_hat: SvHat,
    pub skew_guard: f64,
    pub degenerate_faces: DegenerateFaces,
    pub sliver_metrics: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `boundary_skewness` | `BoundarySkewness` | Boundary skewness definition; see [`BoundarySkewness`]. |
| `severe_non_ortho_deg` | `f64` | Non-orthogonality `[degrees]` above which a face counts as severely<br>non-orthogonal (`checkMesh`'s `primitiveMesh::nonOrthThreshold`, 70). |
| `sv_hat` | `SvHat` | `svHat` evaluation; see [`SvHat`]. |
| `skew_guard` | `f64` | The guard added to the skewness denominators (upstream `rootVSmall`<br>`1e-150`; cfMesh's report and `outram-foam-mesh` used `1e-300`). |
| `degenerate_faces` | `DegenerateFaces` | Degenerate-face handling; see [`DegenerateFaces`]. |
| `sliver_metrics` | `bool` | Compute the face-pyramid and cell-determinant sliver metrics<br>(`false` leaves them `0`; the snapping loop does not read them). |

##### Implementations

###### Methods

- ```rust
  pub fn foam_mesh_legacy() -> Self { /* ... */ }
  ```
  The choices `outram-foam-mesh` made before gh:#1480

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    The cfMesh `check_quality` choices (the gh:#1480 reference).

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CheckGeometry`

The geometry the checks read, supplied by the caller's mesh type (see the
module docs for why it is an input). All arrays are in face / cell order.

```rust
pub struct CheckGeometry {
    pub face_areas: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub face_centres: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub cell_centres: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub cell_volumes: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `face_areas` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Face area vectors `Sf` (owner -> neighbour / outward). |
| `face_centres` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Face centres `Cf`. |
| `cell_centres` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Cell centres `C`. |
| `cell_volumes` | `Vec<f64>` | Signed cell volumes `V`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `QualityReport`

Summary of mesh-quality metrics (worst-case values over the mesh).

```rust
pub struct QualityReport {
    pub max_non_orthogonality_deg: f64,
    pub max_skewness: f64,
    pub max_aspect_ratio: f64,
    pub min_face_area: f64,
    pub min_cell_volume: f64,
    pub n_negative_volume_cells: usize,
    pub min_face_pyramid_volume: f64,
    pub n_negative_pyramid_faces: usize,
    pub min_cell_determinant: f64,
    pub mean_non_orthogonality_deg: f64,
    pub n_severely_non_orthogonal_faces: usize,
    pub max_boundary_skewness: f64,
    pub max_aspect_ratio_cmpt: f64,
    pub mean_aspect_ratio_cmpt: f64,
    pub max_cell_volume: f64,
    pub total_volume: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_non_orthogonality_deg` | `f64` | Maximum face **non-orthogonality**, in degrees: the angle between an<br>internal face's normal and the vector joining its two cell centres. `0°`<br>is perfectly orthogonal; OpenFOAM flags `> 70°`. |
| `max_skewness` | `f64` | Maximum face **skewness** over the internal faces, OpenFOAM's<br>`primitiveMeshTools::faceSkewness`: the offset `s` of the face centre<br>from where the owner–neighbour centre line crosses the face plane,<br>divided by the face's own extent along `s` (the largest projection of<br>a face vertex, measured from the face centre, on `s`), floored at<br>`0.2 |d|`. OpenFOAM flags `> 4` (`maxInternalSkewness 4`).<br><br>~~as a fraction of that line's length~~ **CORRECTED 2026-11-04<br>(gh:#1202):** that was this port's normalisation, not upstream's; on<br>a stack of thin prism layers it read width/thickness (6.6–7.2 on<br>3-layer pipes) where upstream's reads the actual skew. Ported from<br>the workspace's existing port, `outram_foam_mesh::mesh_quality::<br>face_skewness` (OpenFOAM `primitiveMeshTools.C`). |
| `max_aspect_ratio` | `f64` | Maximum cell **aspect ratio** (`Σ|Sf| / (6 · V^{2/3})`, which is `1` for<br>a cube). Large values mean sliver cells. This is cfMesh's definition<br>(sum of face-area magnitudes); OpenFOAM's `checkClosedCells` sums the<br>per-component magnitudes instead, see<br>[`QualityReport::max_aspect_ratio_cmpt`]. |
| `min_face_area` | `f64` | Smallest face area in the mesh. |
| `min_cell_volume` | `f64` | Smallest (signed) cell volume in the mesh. |
| `n_negative_volume_cells` | `usize` | Number of cells with non-positive volume (a broken mesh has `> 0`). |
| `min_face_pyramid_volume` | `f64` | Smallest **face pyramid volume** in the mesh (a volume, so `m³` if the<br>points are in metres), minimised over every *(cell, face)* incidence —<br>an internal face is checked twice, once from each side.<br><br>For face `f` of cell `c`, the pyramid has the face as its base and the<br>**cell centre** as its apex, so its signed volume is<br>`V_pyr = (1/3) · (c_f − C_c) · S_f`, with `c_f` the face centroid and<br>`S_f` the face area vector taken **outward from `c`**.<br><br>Valid range: strictly `> 0` for every face of a well-formed cell — a<br>cube of side `h` gives `h³/6` on all six faces, and the pyramid volumes<br>of a cell sum exactly to its volume. A value `<= 0` means the cell<br>centre lies **on or outside the plane of one of its own faces**: the<br>cell is locally inverted or tangled, the face's flux stencil points the<br>wrong way, and OpenFOAM's `checkMesh` fails such a mesh outright.<br><br>This is not redundant with [`QualityReport::min_cell_volume`]: a<br>strongly concave cell can have a healthily positive *total* volume and<br>still have its centre outside one of its faces (verified in cfMesh's<br>`concave_cell_has_negative_pyramid_but_positive_volume` test).<br>Conversely, a merely **flat** (sliver) cell keeps *positive* pyramid<br>volumes — they just become small — so flatness is<br>[`QualityReport::min_cell_determinant`]'s job, not this metric's.<br><br>`0.0` for a mesh with no faces. |
| `n_negative_pyramid_faces` | `usize` | Number of *(cell, face)* incidences whose pyramid volume is `<= 0`<br>(see [`QualityReport::min_face_pyramid_volume`]). `0` for a valid mesh;<br>any non-zero count is a hard failure in OpenFOAM's `checkMesh`. An<br>internal face can contribute up to `2` (once per adjacent cell).<br><br>Deliberately **not** wired into [`QualityReport::is_solvable`], whose<br>thresholds are left unchanged; test it explicitly if you want<br>`checkMesh`'s stricter gate. |
| `min_cell_determinant` | `f64` | Smallest **cell determinant** over the mesh (dimensionless) — the<br>sliver detector.<br><br># Definition implemented here<br><br>For cell `c` with faces `f`, area vectors `S_f` and unit normals<br>`n_f = S_f / |S_f|`, form the area-weighted normal-orientation tensor<br><br>```text<br>        Σ_f |S_f| (n_f ⊗ n_f)<br>  D_c = ─────────────────────<br>             Σ_f |S_f|<br>```<br><br>and report `27 · det(D_c)`, minimised over the cells. (Sign-free: `n_f`<br>enters only through an outer product, so the face winding is<br>irrelevant.)<br><br># Valid range and interpretation<br><br>`D_c` is symmetric positive semi-definite with `tr(D_c) = 1`, so by<br>AM–GM `det(D_c) <= (1/3)³` and the reported value lies in `[0, 1]`:<br><br>- `1.0` — the face normals are **isotropic** (`D_c = I/3`). An<br>  axis-aligned cube and a regular tetrahedron both give exactly `1`;<br>  both are verified in cfMesh's tests.<br>- `→ 0` — the normals collapse towards a plane (flat sliver) or a line<br>  (needle), i.e. the cell is degenerate in at least one direction. The<br>  tensor loses rank *long before* the cell volume changes sign, which is<br>  what makes this the metric that sees slivers when non-orthogonality,<br>  skewness and `min_cell_volume` all read healthy.<br><br>A cell whose faces all have zero area contributes `0.0`; a mesh with no<br>cells reports `0.0`.<br><br># Parity caveat — read before quoting this against OpenFOAM<br><br>This is the same *form* and normalisation as the "cell determinant"<br>(`wellposedness`) figure OpenFOAM's `checkMesh` prints — a normalised<br>determinant of the summed face-normal outer products, scaled so a cube<br>reads `1` — but it was implemented from that description and from the<br>algebra above, **not** transcribed from OpenFOAM source, and it has<br>**not** been compared numerically against a `checkMesh` run on the same<br>mesh. Treat it as a documented equivalent with the properties proved and<br>tested here, **not** as verified `checkMesh` parity. No threshold from<br>`checkMesh` is reproduced here, and [`QualityReport::is_solvable`] does<br>not gate on it. |
| `mean_non_orthogonality_deg` | `f64` | Mean non-orthogonality `[degrees]` over the internal faces (degenerate<br>faces count as 0 in the sum; the divisor is the internal-face count). |
| `n_severely_non_orthogonal_faces` | `usize` | Number of internal faces above [`CheckOptions::severe_non_ortho_deg`]. |
| `max_boundary_skewness` | `f64` | Maximum **boundary-face** skewness, by the<br>[`CheckOptions::boundary_skewness`] definition. |
| `max_aspect_ratio_cmpt` | `f64` | Maximum cell aspect ratio by OpenFOAM's `checkClosedCells` definition<br>([`cell_aspect_ratio_cmpt`]). |
| `mean_aspect_ratio_cmpt` | `f64` | Mean of [`cell_aspect_ratio_cmpt`] over all cells. |
| `max_cell_volume` | `f64` | Largest signed cell volume. |
| `total_volume` | `f64` | Sum of the signed cell volumes. |

##### Implementations

###### Methods

- ```rust
  pub fn is_solvable(self: &Self) -> bool { /* ... */ }
  ```
  A conservative "good enough to solve" gate: no negative-volume cells,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MeshQualityControls`

snappyHexMesh's mesh-quality controls (`meshQualityControls`, the file
`etc/caseDicts/mesh/generation/meshQualityDict` of OpenFOAM-dev
`852527377c7e`), as checked by [`faces_failing_mesh_quality`]. The
defaults are that file's values.

```rust
pub struct MeshQualityControls {
    pub max_non_ortho_deg: f64,
    pub max_boundary_skewness: f64,
    pub max_internal_skewness: f64,
    pub min_tet_quality: Option<f64>,
    pub min_twist: f64,
    pub min_determinant: f64,
    pub min_face_weight: f64,
    pub min_vol_ratio: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_non_ortho_deg` | `f64` | `maxNonOrtho` [degrees]; 180 disables. |
| `max_boundary_skewness` | `f64` | `maxBoundarySkewness`; negative disables. |
| `max_internal_skewness` | `f64` | `maxInternalSkewness`; negative disables. |
| `min_tet_quality` | `Option<f64>` | `minTetQuality`: here only its sign is checked (a decomposition<br>tetrahedron of non-positive volume fails); `None` disables. |
| `min_twist` | `f64` | `minTwist`; below −1 disables. |
| `min_determinant` | `f64` | `minDeterminant`; negative disables. |
| `min_face_weight` | `f64` | `minFaceWeight`; outside [0, 1) disables. |
| `min_vol_ratio` | `f64` | `minVolRatio`; outside [0, 1) disables. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `face_non_orthogonality_deg`

Non-orthogonality `[degrees]` of an internal face with centre-to-centre
vector `d` and area vector `sf`: `None` for a degenerate face
(`|d| <= 1e-30` or `|Sf| <= 1e-30`), which the cfMesh reference skips.

```rust
pub fn face_non_orthogonality_deg(d: outram_foam_basic_lib::primitives::Vector3, sf: outram_foam_basic_lib::primitives::Vector3) -> Option<f64> { /* ... */ }
```

#### Function `face_skewness`

**Attributes:**

- `Other("#[attr = LintCheck([LintCheck {tool: clippy#0,\nlint_name: \"too_many_arguments\", kind: Allow}])]")`

OpenFOAM `faceSkewness` / `boundaryFaceSkewness` core: the offset `sv` of
the face centre from where the line of `d` pierces the face plane, over the
face's own extent along `sv` (largest projection of a ring vertex from
`cf`), floored at `floor * |d|`. `own_cc` is the owner cell centre, `ring`
the face's point indices. `guard` is the small number added to the
denominators (`1e-150` upstream's `rootVSmall`, `1e-300` in cfMesh's
report). `sv_hat_kind` selects divide (upstream) or reciprocal-multiply
(cfMesh), see [`SvHat`].

```rust
pub fn face_skewness(cf: outram_foam_basic_lib::primitives::Vector3, sf: outram_foam_basic_lib::primitives::Vector3, own_cc: outram_foam_basic_lib::primitives::Vector3, d: outram_foam_basic_lib::primitives::Vector3, floor: f64, guard: f64, sv_hat_kind: SvHat, ring: &[usize], points: &[outram_foam_basic_lib::primitives::Vector3]) -> f64 { /* ... */ }
```

#### Function `cell_aspect_ratio_cmpt`

Cell aspect ratio, OpenFOAM `checkClosedCells`:
`(1/6) (Σ|Sf_x| + Σ|Sf_y| + Σ|Sf_z|) / V^(2/3)`; `0` for `V <= 0`.

```rust
pub fn cell_aspect_ratio_cmpt(sum_cmpt_mag: outram_foam_basic_lib::primitives::Vector3, volume: f64) -> f64 { /* ... */ }
```

#### Function `check_quality`

Compute the [`QualityReport`] of a mesh given as points, face rings,
owner / neighbour (`None` for a boundary face) and its [`CheckGeometry`].

This is the cfMesh `check_quality` (gh:#1480 reference) extended with the
metrics `outram-foam-mesh` reported (boundary skewness, OpenFOAM aspect
ratio, mean / severe non-orthogonality, volume extrema and total).

```rust
pub fn check_quality<F: AsRef<[usize]>>(points: &[outram_foam_basic_lib::primitives::Vector3], faces: &[F], owner: &[usize], neighbour: &[Option<usize>], geom: &CheckGeometry, opts: &CheckOptions) -> QualityReport { /* ... */ }
```

#### Function `faces_failing_mesh_quality`

The faces that fail `controls` (gh:#1215): the port of OpenFOAM's
`meshCheck::checkMesh(report, mesh, dict, checkFaces, baffles,
wrongFaces)` (OpenFOAM-dev `852527377c7e`, `src/meshCheck/checkMesh.C`,
with the per-check definitions of `polyMeshCheck/polyMeshCheckQuality.C`
and `primitiveMeshCheck/primitiveMeshCheck.C`), which snappyHexMesh's
layer driver runs on every layered mesh (`snappyLayerDriver::
checkAndUnmark`) and its snap driver on every motion step
(`motionSmoother`).

Checks ported: `checkFaceOrthogonality` (internal faces),
`checkFaceSkewness` (`faceSkewness` on internal faces,
`boundaryFaceSkewness` with its 0.4 |d| floor on boundary faces),
`checkFaceTets` (sign only), `checkFaceTwist`, `checkCellDeterminant`
(`cellDeterminant`: det of Σ (S/⟨|S|⟩)² over the cell's internal faces; a
failing cell marks all its faces), `checkFaceWeights`, `checkVolRatio`.
Not ported: `maxConcave` (`checkFaceAngles`), `minVol` (disabled, −1e30,
in the default dictionary) and `minFaceFlatness` (absent from it).
Centres are the caller's [`CheckGeometry`] (cfMesh passes vertex-average
face centres, not OpenFOAM's area-weighted ones).

```rust
pub fn faces_failing_mesh_quality<F: AsRef<[usize]>>(points: &[outram_foam_basic_lib::primitives::Vector3], faces: &[F], owner: &[usize], neighbour: &[Option<usize>], geom: &CheckGeometry, controls: &MeshQualityControls) -> Vec<bool> { /* ... */ }
```

### Constants and Statics

#### Constant `ROOT_V_SMALL`

OpenFOAM's `rootVSmall` (`1e-150`), the guard in `faceSkewness` and
friends.

```rust
pub const ROOT_V_SMALL: f64 = 1e-150;
```

## Module `mesh_quality`

Mesh-quality assessment — "is this mesh usable by a finite-volume solver?"

This is the `checkMesh` half of the crate: given any mesh in
[`PolyMesh`] form (points + face vertex loops + owner/neighbour), it reports
the four metrics that decide whether a solver can run on it at all, and
grades them against OpenFOAM's own thresholds.

```no_run
use outram_foam_mesh::mesh_quality::assess_quality;
use outram_foam_basic_lib::io::PolyMesh;

let mesh = PolyMesh::read("case/constant/polyMesh").unwrap();
let q = assess_quality(&mesh);
println!("{}", q.summary());
```

Every mesh this crate produces converts into [`PolyMesh`] — see
[`block_mesh::PolyMesh::to_foam_poly_mesh`](crate::block_mesh::PolyMesh::to_foam_poly_mesh),
[`PolyPatchMesh::to_foam_poly_mesh`](crate::snappy_hex_mesh::PolyPatchMesh::to_foam_poly_mesh),
[`DualMesh::to_foam_poly_mesh`](crate::poly_dual_mesh::DualMesh::to_foam_poly_mesh) —
and a mesh written by any other tool can be read back with
[`PolyMesh::read`], so this function grades foreign meshes too (e.g. a
polyhedral dual coming out of a different mesher).

## The four metrics

All definitions mirror OpenFOAM's `primitiveMeshTools` /
`primitiveMesh::checkClosedCells` so the numbers are directly comparable
with what `checkMesh` prints.

1. **Non-orthogonality** `[degrees]` — for internal face `f` between cells
   `o` and `n`, the angle between the face-area vector `Sf` and the
   centre-to-centre vector `d = C_n − C_o`. Zero for a perfectly orthogonal
   (e.g. Cartesian) mesh. This is the metric that governs whether a
   diffusion term needs explicit non-orthogonal correction, and how many
   corrector sweeps it needs.
2. **Skewness** (dimensionless) — how far the face centre `Cf` sits from
   the point where the owner–neighbour line pierces the face, normalised by
   the face's own extent in that direction. Non-orthogonality and skewness
   are independent: a uniformly sheared mesh is non-orthogonal but not
   skewed, and a mesh whose faces are offset sideways is skewed but
   orthogonal (both cases are in this module's tests).
3. **Aspect ratio** (dimensionless) — per cell,
   `(1/6) · Σ_f (|Sf_x| + |Sf_y| + |Sf_z|) / V^(2/3)`, which is exactly `1`
   for a cube. High values mean thin, stretched cells (a boundary layer is
   deliberately high-aspect; the far field should not be).
4. **Negative-volume cells** — cells whose pyramid-decomposition volume is
   `<= 0`, i.e. inverted or degenerate. Any non-zero count is fatal: no
   solver can run on such a mesh.

## Thresholds and what they mean

See [`NON_ORTHO_OK_DEG`], [`NON_ORTHO_SEVERE_DEG`], [`SKEWNESS_LIMIT`] and
[`BOUNDARY_SKEWNESS_LIMIT`]. [`MeshQualityReport::verdict`] folds them into
a single [`QualityVerdict`].

> **Caveat for this workspace.** Passing this check means the mesh is not
> pathological by OpenFOAM's standards. It does **not** mean the solver's
> numerics are verified at that quality: `outram-foam-basic-lib`'s
> non-orthogonal correction has its own, separate V&V range, and a mesh
> whose non-orthogonality exceeds it will produce discretisation error the
> correction has not been shown to control. Check that crate's V&V before
> trusting a solve on a mesh near these limits.

## Verification & validation

**Methodology.** The metrics are checked on four hand-built meshes whose
exact values are derived in closed form (see each test's doc comment):
a Cartesian block, a uniformly sheared block (non-orthogonality
`= atan(k)` exactly), a laterally offset-face pair (skewness `= s` exactly),
and an elongated block (aspect ratio `= (ab+bc+ca) / (3·(abc)^(2/3))`).

**Results (measured 2026-08-07, release, x86_64).** See the individual test
doc comments in this module for the measured values; every case agrees with
its closed form to `< 1e-12`.

```rust
pub mod mesh_quality { /* ... */ }
```

### Types

#### Enum `QualityVerdict`

One-word grade for a mesh, derived from a [`MeshQualityReport`].

The grades are ordered `Good < Marginal < Unusable` in severity; see
[`MeshQualityReport::verdict`] for the exact rule.

```rust
pub enum QualityVerdict {
    Good,
    Marginal,
    Unusable,
}
```

##### Variants

###### `Good`

No inverted cells, non-orthogonality `<=` [`NON_ORTHO_OK_DEG`],
skewness within both limits. A solver should be able to run without
special measures.

###### `Marginal`

No inverted cells, but non-orthogonality exceeds [`NON_ORTHO_OK_DEG`]
(possibly exceeding [`NON_ORTHO_SEVERE_DEG`] too). Solvable, but
non-orthogonal correction is mandatory and the discretisation error is
no longer bounded by the correction's verified range.

###### `Unusable`

At least one inverted / zero-volume cell, or skewness beyond
[`SKEWNESS_LIMIT`] / [`BOUNDARY_SKEWNESS_LIMIT`], or a
non-orthogonality of `90°` or more (the face plane no longer separates
the two cell centres). Do not solve on this mesh.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
#### Struct `MeshQualityReport`

A full mesh-quality report — the output of [`assess_quality`].

All angles are in degrees, volumes in `m³`, and skewness / aspect ratio are
dimensionless. Fields are public so a caller can gate on any single metric;
[`summary`](Self::summary) formats them the way `checkMesh` does.

```rust
pub struct MeshQualityReport {
    pub n_cells: usize,
    pub n_points: usize,
    pub n_faces: usize,
    pub n_internal_faces: usize,
    pub n_boundary_faces: usize,
    pub total_volume: f64,
    pub min_cell_volume: f64,
    pub max_cell_volume: f64,
    pub n_negative_volume_cells: usize,
    pub max_non_ortho_deg: f64,
    pub mean_non_ortho_deg: f64,
    pub n_severely_non_ortho_faces: usize,
    pub max_skewness: f64,
    pub max_boundary_skewness: f64,
    pub max_aspect_ratio: f64,
    pub mean_aspect_ratio: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_cells` | `usize` | Number of cells. |
| `n_points` | `usize` | Number of mesh points (vertices). |
| `n_faces` | `usize` | Total number of faces (internal + boundary). |
| `n_internal_faces` | `usize` | Number of internal faces (those with a neighbour cell). |
| `n_boundary_faces` | `usize` | Number of boundary faces. |
| `total_volume` | `f64` | Sum of all cell volumes `[m³]`. |
| `min_cell_volume` | `f64` | Smallest cell volume `[m³]`. Non-positive means an inverted cell. |
| `max_cell_volume` | `f64` | Largest cell volume `[m³]`. |
| `n_negative_volume_cells` | `usize` | Count of cells with volume `<= 0` (inverted / degenerate). Fatal if<br>non-zero. |
| `max_non_ortho_deg` | `f64` | Worst internal-face non-orthogonality `[degrees]`. |
| `mean_non_ortho_deg` | `f64` | Arithmetic mean of internal-face non-orthogonality `[degrees]`. `0` if<br>the mesh has no internal faces (a single-cell mesh). |
| `n_severely_non_ortho_faces` | `usize` | Number of internal faces above [`NON_ORTHO_SEVERE_DEG`]. |
| `max_skewness` | `f64` | Worst **internal**-face skewness (dimensionless); limit<br>[`SKEWNESS_LIMIT`]. |
| `max_boundary_skewness` | `f64` | Worst **boundary**-face skewness (dimensionless), computed with<br>OpenFOAM's mirrored-neighbour treatment; limit<br>[`BOUNDARY_SKEWNESS_LIMIT`]. |
| `max_aspect_ratio` | `f64` | Worst cell aspect ratio (dimensionless); exactly `1` for a cube. |
| `mean_aspect_ratio` | `f64` | Mean cell aspect ratio (dimensionless). |

##### Implementations

###### Methods

- ```rust
  pub fn verdict(self: &Self) -> QualityVerdict { /* ... */ }
  ```
  Grade the mesh — see [`QualityVerdict`] for the rule.

- ```rust
  pub fn summary(self: &Self) -> String { /* ... */ }
  ```
  A multi-line human-readable summary, laid out like OpenFOAM's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `assess_quality`

Assess the quality of any [`PolyMesh`] — the crate's `checkMesh`.

# Inputs
`mesh` — points in metres, faces as vertex loops wound so the right-hand
normal points owner → neighbour (internal) or out of the domain (boundary),
faces ordered internal-first then by patch. That is exactly the invariant
[`PolyMesh`] documents, so any mesh read from a `constant/polyMesh` or
produced by this crate satisfies it.

# Returns
A [`MeshQualityReport`]. This function never fails: it is meant to be run
on *bad* meshes, so it reports inverted cells rather than refusing them. A
mesh with fewer than 3 points in a face contributes a zero area vector and
is simply counted through the volume metrics.

# Cost
`O(n_faces)` with two passes over the faces plus one over the cells; all
geometry is recomputed from `points`, so the report always reflects the
current point positions rather than any cached geometry.

```rust
pub fn assess_quality(mesh: &outram_foam_basic_lib::io::PolyMesh) -> MeshQualityReport { /* ... */ }
```

### Constants and Statics

#### Constant `NON_ORTHO_OK_DEG`

Non-orthogonality `[degrees]` at or below which a mesh is unremarkable —
`snappyHexMesh`'s own default `maxNonOrtho` accept limit.

```rust
pub const NON_ORTHO_OK_DEG: f64 = 65.0;
```

#### Constant `NON_ORTHO_SEVERE_DEG`

Non-orthogonality `[degrees]` above which OpenFOAM's `checkMesh` counts a
face as **severely non-orthogonal** and prints a warning
(`primitiveMesh::setNonOrthThreshold` default).

```rust
pub const NON_ORTHO_SEVERE_DEG: f64 = 70.0;
```

#### Constant `SKEWNESS_LIMIT`

Internal-face skewness above which `checkMesh` reports a failure
(OpenFOAM default `maxSkewness`).

```rust
pub const SKEWNESS_LIMIT: f64 = 4.0;
```

#### Constant `BOUNDARY_SKEWNESS_LIMIT`

Boundary-face skewness above which `checkMesh` reports a failure. Boundary
faces use a mirrored neighbour cell, so the limit is much looser.

```rust
pub const BOUNDARY_SKEWNESS_LIMIT: f64 = 20.0;
```

## Module `poly_dual_mesh`

`polyDualMesh` — construct the polyhedral **dual** of a primal mesh.

This is the OUTRAM PARK re-implementation of the OpenFOAM `polyDualMesh`
utility. The dual is the *cell-centre dual*: it exchanges the roles of the
primal mesh entities.

| primal entity        | dual entity                                   |
|----------------------|-----------------------------------------------|
| cell                 | **point** (placed at the primal cell centroid)|
| point                | **cell**                                      |
| edge                 | **face** (a ring of dual points)              |
| face                 | edge (between two dual points)                |

Concretely: every primal cell contributes one dual point at its centroid;
every primal point becomes one dual cell; and every primal **interior edge**
becomes one dual face — the closed ring of cell-centres of the cells that
share that edge, separating the dual cells of the edge's two endpoints.

## Boundary treatment

The interior of the dual uses **only cell-centres** as vertices — that is the
defining property of `polyDualMesh` and it is honoured here exactly. The
domain boundary needs extra dual vertices so that the dual mesh reproduces
the primal boundary *surface* (and hence encloses exactly the same volume).
Three further classes of dual vertex are added on the boundary:

- one **boundary-face centre** per primal boundary face,
- one **boundary-edge midpoint** per primal boundary edge,
- one **feature-point** vertex per primal boundary point that the
  feature-angle test flags as a geometric feature (an edge or corner of the
  surface).

A primal **boundary edge** then becomes an *internal* dual face closed along
the surface through the two boundary-face centres and the edge midpoint; and
each primal **boundary point** contributes a dual **boundary** face — the
2-D cell-centre dual of the surface mesh around that point.

## Feature angle, feature edges and patch seams

~~`feature_angle_deg` is the OpenFOAM-style feature angle. A boundary point is
kept as an explicit dual vertex (preserving a sharp edge or corner) when the
surface bends across it by **more than** the feature angle; otherwise the
surface is locally flat there and the point is *merged* away.~~
**CHANGED 2026-11-04 (gh:#1240), to upstream's rule.** Features are found on
**edges** first, as OpenFOAM-dev's `polyDualMesh` utility does
(`simpleMarkFeatures` in
`applications/utilities/mesh/manipulation/polyDualMesh/polyDualMesh.C`,
commit `852527377c7e`; the same rule as `polyDualMesh::calcFeatures` in
`src/conversion/polyDualMesh/polyDualMesh.C`):

- a boundary edge is a **feature edge** when its two boundary faces lie in
  **different patches** (step 1, "mark all edges between patches", whatever
  the angle), or when their normals differ by **more than**
  `feature_angle_deg` (step 2);
- every point on a feature edge is a **feature point** (upstream's
  `singleCellFeaturePoints`) and is kept as an explicit dual vertex.

A non-feature boundary point is merged away: its ring of boundary quads
collapses to one polygon `[fc, em, fc, em, …]`. On a flat patch this
removes the redundant vertex without changing the enclosed volume; at a
sharp feature (e.g. a 90° block edge) merging it away would cut the corner
and lose volume, which is exactly why the feature is preserved.

## Boundary faces: one per point per feature-bounded sector (gh:#1240)

~~A feature point emits one quad `[p, em, fc, em]` per incident boundary
face.~~ **CHANGED 2026-11-04 (gh:#1240), to upstream's construction**
(`meshDualiser::createFacesAroundBoundaryPoint`, same file set and commit):
the boundary faces around a feature point are walked in order and a dual
face is **closed at every feature edge**, so the point gives one face per
sector between consecutive feature edges, `[p, em(feature), fc, em, fc, …,
em(feature)]`. A point on a straight feature line gets two faces (one per
side), a box corner three; a patch seam is a feature edge, so no face spans
two patches. With fewer than two feature edges at the point the ring is
kept whole (`polyDualMesh::splitFace`: "not enough feature edges, no
split"). The quad-per-face construction gave a tetrahedral primal's
feature points 4–8 boundary faces per dual cell, each badly
non-orthogonal to its owner (gh:#1215, gh:#1240).

**Deviation from upstream, kept on purpose:** upstream puts a dual vertex
at the midpoint of *feature* edges only; this port puts one on **every**
boundary edge, so the boundary-edge dual faces close on the surface and a
flat patch is reproduced exactly (volume conserved to round-off on a
block). Upstream's surface is the polygon through the boundary-face
centres, which cuts every curved patch slightly.

## Scope / honesty

- The interior construction (cell-centre rings around interior edges) is
  general — it works for any manifold polyhedral primal mesh,
  ~~validated on structured hexahedral blocks only~~ and since gh:#1240
  (2026-11-04) run on tetrahedral meshes too, through
  `outram-park-fork-cfmesh`'s `DualKind::CellCentre` (its `cell-centre-dual`
  feature), where it is compared differentially against the median dual
  (`crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`).
- The boundary construction is implemented for **manifold** surfaces (each
  boundary edge shared by exactly two boundary faces); non-manifold
  surfaces and degenerate fans are rejected with [`MeshError::Construction`]
  rather than silently mis-meshed.
- ~~It does **not** yet split coplanar boundary faces belonging to
  *different* patches that meet below the feature angle~~ **DONE
  2026-11-04 (gh:#1240):** patch seams are feature edges, as upstream.
- ~~Not ported: upstream's `concaveMultiCells`, `splitAllFaces` and
  face-zone preservation.~~ **CORRECTED 2026-11-11 (gh:#1471, gh:#1472):**
  they are ported in the explicit upstream mode,
  [`poly_dual_mesh_upstream`](crate::poly_dual_mesh_upstream) (a
  line-for-line `polyDualMesh` + `meshDualiser`, beside this surface-
  preserving default). This default still has none of the three.

```rust
pub mod poly_dual_mesh { /* ... */ }
```

### Types

#### Struct `PolyMesh`

A general polyhedral primal mesh — the input to [`poly_dual_mesh`].

This is a full connectivity `polyMesh`: points, faces as **ordered vertex
loops**, `owner`/`neighbour` cell links, and boundary patches. (The
`outram-foam-basic-lib` [`FvMesh`] stores only *flat geometry* — cell/face
centres and areas — with no point/face-vertex connectivity, so the dual
construction, which is purely topological, needs this richer input type.)

## Conventions (OpenFOAM)

- Faces are ordered **internal faces first**, then boundary faces grouped by
  patch: `faces[0 .. n_internal_faces)` are internal, the rest boundary.
- `owner[f]` is defined for every face; `neighbour[f]` only for internal
  faces (`neighbour.len() == n_internal_faces`).
- Each face's vertex loop is wound so its right-hand-rule normal points from
  `owner` toward `neighbour` (internal) or **outward** from the domain
  (boundary).

```rust
pub struct PolyMesh {
    pub points: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub faces: Vec<Vec<usize>>,
    pub owner: Vec<usize>,
    pub neighbour: Vec<usize>,
    pub n_internal_faces: usize,
    pub patches: Vec<outram_foam_basic_lib::mesh::BoundaryPatch>,
    pub n_cells: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Mesh points `[m]`. |
| `faces` | `Vec<Vec<usize>>` | Faces — each an ordered loop of point indices. |
| `owner` | `Vec<usize>` | Owning cell of each face (length `faces.len()`). |
| `neighbour` | `Vec<usize>` | Neighbour cell of each internal face (length `n_internal_faces`). |
| `n_internal_faces` | `usize` | Number of internal faces (faces with both owner and neighbour). |
| `patches` | `Vec<outram_foam_basic_lib::mesh::BoundaryPatch>` | Boundary patch descriptors (reusing the basic-lib type). |
| `n_cells` | `usize` | Number of cells. |

##### Implementations

###### Methods

- ```rust
  pub fn n_points(self: &Self) -> usize { /* ... */ }
  ```
  Number of points.

- ```rust
  pub fn is_internal_face(self: &Self, f: usize) -> bool { /* ... */ }
  ```
  True if face `f` is internal.

- ```rust
  pub fn total_volume(self: &Self) -> f64 { /* ... */ }
  ```
  Total enclosed volume `[m³]` — sum of primal cell volumes.

- ```rust
  pub fn structured_hex_block(nx: usize, ny: usize, nz: usize, lx: f64, ly: f64, lz: f64) -> Self { /* ... */ }
  ```
  Construct a uniform structured hexahedral block: `nx × ny × nz` cells

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DualMesh`

The polyhedral dual mesh produced by [`poly_dual_mesh`].

Full connectivity: `points` (dual vertices), `faces` (vertex loops), and per
face an `owner` dual-cell id (== the primal point index it surrounds) plus an
optional `neighbour` (`None` ⇒ boundary face) and an optional source patch
index. Convert to a flat `outram-foam-basic-lib` [`FvMesh`] with
[`DualMesh::to_fv_mesh`].

```rust
pub struct DualMesh {
    pub points: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub faces: Vec<Vec<usize>>,
    pub owner: Vec<usize>,
    pub neighbour: Vec<Option<usize>>,
    pub face_patch: Vec<Option<usize>>,
    pub n_cells: usize,
    pub cell_is_interior: Vec<bool>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Dual points `[m]`. |
| `faces` | `Vec<Vec<usize>>` | Dual faces (ordered vertex loops). |
| `owner` | `Vec<usize>` | Owning dual cell of each face (== primal point index). |
| `neighbour` | `Vec<Option<usize>>` | Neighbour dual cell of each face; `None` for boundary faces. |
| `face_patch` | `Vec<Option<usize>>` | Source primal patch index for each boundary face; `None` for internal. |
| `n_cells` | `usize` | Number of dual cells (== number of primal points). |
| `cell_is_interior` | `Vec<bool>` | Whether each dual cell corresponds to an interior primal point. |

##### Implementations

###### Methods

- ```rust
  pub fn n_interior_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of dual cells whose primal point is interior (not on the

- ```rust
  pub fn total_volume(self: &Self) -> f64 { /* ... */ }
  ```
  Total enclosed volume `[m³]` — sum of dual cell volumes. Equals the primal

- ```rust
  pub fn max_closure_residual(self: &Self) -> f64 { /* ... */ }
  ```
  Largest closure residual over all dual cells: `max_c |Σ_f Sf_out|`.

- ```rust
  pub fn first_bad_euler(self: &Self) -> Option<(usize, i64)> { /* ... */ }
  ```
  Verify that every non-empty dual cell is a genus-0 closed polyhedron

- ```rust
  pub fn to_foam_poly_mesh(self: &Self, patch_names: &[String]) -> Result<outram_foam_basic_lib::io::PolyMesh, MeshError> { /* ... */ }
  ```
  Convert the dual to the writable `outram-foam-basic-lib`

- ```rust
  pub fn to_fv_mesh(self: &Self, patch_names: &[String]) -> Result<FvMesh, MeshError> { /* ... */ }
  ```
  Convert the dual to a flat `outram-foam-basic-lib` [`FvMesh`]: faces are

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `FeaturePoints`

Which boundary points keep their own dual vertex (gh:#1240). OpenFOAM's
two `polyDualMesh`s differ here; both are kept callable. The same choice
is `outram-park-fork-cfmesh`'s `dual::FeaturePointRule`.

```rust
pub enum FeaturePoints {
    OnFeatureEdge,
    Corners,
}
```

##### Variants

###### `OnFeatureEdge`

Every point on a feature edge: the `polyDualMesh` **utility**
(`simpleMarkFeatures`' `singleCellFeaturePoints`, `meshDualiser`).

###### `Corners`

Only points with more than two feature edges: the `polyDualMesh`
**class** (`polyDualMesh::calcFeatures`); a point on a feature line is
dropped and its ring split along the line (`splitFace`), the sides
meeting on the chord between the two feature-edge midpoints.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `poly_dual_mesh`

Construct the polyhedral dual of `primal`.

`feature_angle_deg` is the feature angle (see the module docs).
~~A boundary point is kept when the surface bends across it by more than
this angle~~ **CORRECTED 2026-11-11 (gh:#1471; the rule changed in
gh:#1240, 2026-11-04):** a boundary *edge* is a feature edge when its two
boundary faces lie in different patches (whatever the angle) or their
normals differ by more than this angle; every point on a feature edge is
kept as an explicit dual vertex, and any other boundary point is merged
into a coplanar boundary polygon.

# Errors
Returns [`MeshError::Construction`] for a non-manifold boundary or an
inconsistent edge fan.

```rust
pub fn poly_dual_mesh(primal: &PolyMesh, feature_angle_deg: f64) -> Result<DualMesh, crate::MeshError> { /* ... */ }
```

#### Function `poly_dual_mesh_with`

[`poly_dual_mesh`] with the feature-point rule named (gh:#1240).

# Errors
As [`poly_dual_mesh`].

```rust
pub fn poly_dual_mesh_with(primal: &PolyMesh, feature_angle_deg: f64, rule: FeaturePoints) -> Result<DualMesh, crate::MeshError> { /* ... */ }
```

## Module `poly_dual_mesh_upstream`

`polyDualMesh` in **upstream mode** (gh:#1472, gh:#1471, gh:#1473): a
line-for-line port of OpenFOAM's `polyDualMesh` utility driver
(`simpleMarkFeatures`) and its `meshDualiser` engine, with `polyTopoChange`'s
point / cell / face numbering, so the output can be compared with an
upstream run file by file.

This is **beside** the surface-preserving default of
[`poly_dual_mesh`](crate::poly_dual_mesh); nothing here changes that
default. Choose it explicitly with [`poly_dual_mesh_upstream`].

## What differs from the default (`poly_dual_mesh_with`)

| | default (surface-preserving) | upstream mode (this file) |
|---|---|---|
| dual points | cell centres, every boundary-face centre, **every** boundary-edge midpoint, feature points | cell centres, boundary-face centres, **feature**-edge midpoints (and midpoints of boundary edges at feature points), feature points; more only where dual cells differ |
| dual cells | one per primal point, numbered as the primal point | feature points first (hash-set order), multi-cell points, then the other points; numbered as `polyTopoChange` adds them |
| options | `featureAngle` | `featureAngle`, `splitAllFaces`, `concaveMultiCells`, face-zone preservation |
| face / cell centres | the crate's own fan formulas | [`CentreFormula::Dev`] (OpenFOAM-dev `face::areaAndCentre`, area weights projected on the normal) or [`CentreFormula::Esi2506`] (`mag(n)` weights) |
| face order | internal faces then patches, in construction order | `polyTopoChange::getFaceOrder`: upper-triangular by dual cell, then patches |

## Hash-set order

`simpleMarkFeatures` returns its feature sets with `labelHashSet::toc()`,
so the numbering of dual points and cells follows the hash table's bucket
order. [`FoamLabelHashSet`] reproduces OpenFOAM-dev's table (identity hash,
power-of-two table, insertion at the bucket head, doubling above load 0.8,
re-insertion in iteration order). It is checked against the OpenFOAM.com
v2506 binary in the V&V record, not assumed.

## Honest limits

- Face zones are supplied by the caller ([`FaceZoneInput`]); the crate's I/O
  [`PolyMesh`](crate::poly_dual_mesh::PolyMesh) has no zones.
- Parallel (processor) patches and cyclic couplings are not handled.
- OpenFOAM.com's `polyDualMesh` also carries fields and writes the face
  zones of the dual; neither is done here.

```rust
pub mod poly_dual_mesh_upstream { /* ... */ }
```

### Types

#### Enum `CentreFormula`

Which OpenFOAM line's face-centre routine to use.

The two lines differ for faces with more than three vertices (gh:#1473):
OpenFOAM-dev weights the point-average triangles by their area projected
on the face normal; OpenFOAM.com v2506 weights by `mag(n)`. For a planar
face the two agree only to round-off.

```rust
pub enum CentreFormula {
    Dev,
    Esi2506,
}
```

##### Variants

###### `Dev`

OpenFOAM-dev `face::areaAndCentre` (faceTemplates.C:79-150), the line
the port cites (`852527377c7e`).

###### `Esi2506`

OpenFOAM.com v2506 `primitiveMeshTools::makeFaceCentresAndAreas`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FoamLabelHashSet`

OpenFOAM-dev `labelHashSet`, reproduced for its **iteration order** only.

`HashTable<nil, label, Hash<label>>`: `Hash<label>` is the identity, the
table size is a power of two (`canonicalSize`), a new key goes to the head
of its bucket, the table doubles when `nElmts/tableSize > 0.8`, a resize
re-inserts keys in the old iteration order, and `toc()` walks the buckets
0.. and each chain head to tail. `HashSet(size = 128)` by default.

```rust
pub struct FoamLabelHashSet {
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
  pub fn with_size(size: usize) -> Self { /* ... */ }
  ```
  A set constructed with `size` (`HashSet(const label size = 128)`).

- ```rust
  pub fn with_reserve_esi2506(n: usize) -> Self { /* ... */ }
  ```
  OpenFOAM.com v2506's `reserve(n)` on an empty set: capacity

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  The default-constructed set (`size = 128`).

- ```rust
  pub fn insert(self: &mut Self, key: usize) -> bool { /* ... */ }
  ```
  Insert `key`; `false` if present (`HashTable::set`, protect = true).

- ```rust
  pub fn erase(self: &mut Self, key: usize) -> bool { /* ... */ }
  ```
  Remove `key` (the chain order of the others is kept).

- ```rust
  pub fn found(self: &Self, key: usize) -> bool { /* ... */ }
  ```
  Whether `key` is in the set.

- ```rust
  pub fn toc(self: &Self) -> Vec<usize> { /* ... */ }
  ```
  `toc()`: the keys in bucket order.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of keys.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  True if empty.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FaceZoneInput`

A face zone of the primal mesh, as `simpleMarkFeatures` reads it: only the
member faces matter (flip maps do not affect the dual's connectivity in
OpenFOAM-dev, which clears the zones before `meshDualiser`).

```rust
pub struct FaceZoneInput {
    pub name: String,
    pub faces: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Zone name. |
| `faces` | `Vec<usize>` | Member face indices into the primal face list. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `UpstreamOptions`

Options of upstream-mode `polyDualMesh` (the utility's command line).

```rust
pub struct UpstreamOptions {
    pub feature_angle_deg: f64,
    pub split_all_faces: bool,
    pub concave_multi_cells: bool,
    pub preserve_face_zones: bool,
    pub face_zones: Vec<FaceZoneInput>,
    pub centres: CentreFormula,
    pub esi2506_hash_reserve: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `feature_angle_deg` | `f64` | `featureAngle` in degrees `[0, 180]`. |
| `split_all_faces` | `bool` | `-splitAllFaces`: one dual face per primal cell an edge passes through. |
| `concave_multi_cells` | `bool` | `-concaveMultiCells`: points on concave feature edges become several<br>cells. |
| `preserve_face_zones` | `bool` | Preserve face zones (default); `-doNotPreserveFaceZones` clears it. |
| `face_zones` | `Vec<FaceZoneInput>` | The primal mesh's face zones. |
| `centres` | `CentreFormula` | Face-centre formula; see [`CentreFormula`]. |
| `esi2506_hash_reserve` | `bool` | OpenFOAM.com v2506 sizes the feature-face hash set with<br>`reserve(n)` (capacity `2n`), OpenFOAM-dev with `labelHashSet(n)`<br>(capacity `n`); the capacity fixes the `toc()` order of that set and<br>so the numbering of the face-centre dual points. |

##### Implementations

###### Methods

- ```rust
  pub fn new(feature_angle_deg: f64) -> Self { /* ... */ }
  ```
  Defaults of the utility: no `-splitAllFaces`, no `-concaveMultiCells`,

- ```rust
  pub fn with_esi2506_flavour(self: Self) -> Self { /* ... */ }
  ```
  Everything that differs between the two OpenFOAM lines, set to

- ```rust
  pub fn with_split_all_faces(self: Self) -> Self { /* ... */ }
  ```
  Enable `-splitAllFaces`.

- ```rust
  pub fn with_concave_multi_cells(self: Self) -> Self { /* ... */ }
  ```
  Enable `-concaveMultiCells`.

- ```rust
  pub fn without_face_zone_preservation(self: Self) -> Self { /* ... */ }
  ```
  `-doNotPreserveFaceZones`.

- ```rust
  pub fn with_face_zones(self: Self, zones: Vec<FaceZoneInput>) -> Self { /* ... */ }
  ```
  Supply the primal mesh's face zones.

- ```rust
  pub fn with_centre_formula(self: Self, f: CentreFormula) -> Self { /* ... */ }
  ```
  Choose the face-centre formula.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FeatureSets`

The feature sets `simpleMarkFeatures` hands to `meshDualiser`, each in
`labelHashSet::toc()` order (the order that numbers the dual points and
cells). The utility dumps the same sets to `featureFaces.obj`,
`featureEdges.obj`, `singleCellFeaturePoints.obj` and
`multiCellFeaturePoints.obj`, so this is what the differential compares
first (gh:#1472).

```rust
pub struct FeatureSets {
    pub faces: Vec<usize>,
    pub edges: Vec<usize>,
    pub single_cell_points: Vec<usize>,
    pub multi_cell_points: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `faces` | `Vec<usize>` | Face labels. |
| `edges` | `Vec<usize>` | Edge labels, in [`primal_edges`] numbering. |
| `single_cell_points` | `Vec<usize>` | Points that become one dual cell. |
| `multi_cell_points` | `Vec<usize>` | Points that become several dual cells. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `face_area_and_centre`

Area vector and centre of one face, exactly as the chosen OpenFOAM line
computes them (operation order included).

```rust
pub fn face_area_and_centre(formula: CentreFormula, ps: &[outram_foam_basic_lib::primitives::Vector3]) -> (outram_foam_basic_lib::primitives::Vector3, outram_foam_basic_lib::primitives::Vector3) { /* ... */ }
```

#### Function `cell_centres_and_volumes`

Cell centres `primitiveMesh::makeCellCentresAndVols` (identical in
OpenFOAM-dev and v2506): pyramids about the mean face centre,
`pc = 3/4 fc + 1/4 cEst`. Returns `(centres, volumes)`.

```rust
pub fn cell_centres_and_volumes(n_cells: usize, owner: &[usize], neighbour: &[usize], f_ctrs: &[outram_foam_basic_lib::primitives::Vector3], f_areas: &[outram_foam_basic_lib::primitives::Vector3]) -> (Vec<outram_foam_basic_lib::primitives::Vector3>, Vec<f64>) { /* ... */ }
```

#### Function `poly_dual_mesh_upstream`

The polyhedral dual of `primal` by OpenFOAM's `polyDualMesh` utility
(`simpleMarkFeatures` + `meshDualiser` + `polyTopoChange` ordering).

Faces come out in `polyTopoChange::changeMesh` order (internal faces
upper-triangular in the dual cells, then patch by patch), and each dual
patch is the primal patch of the same index, so the result writes
directly.

# Errors
[`MeshError::Construction`] when the primal mesh breaks an assumption the
utility also aborts on (non-manifold fans, a feature-less change of dual
cell across a face, ...).

```rust
pub fn poly_dual_mesh_upstream(primal: &crate::poly_dual_mesh::PolyMesh, opts: &UpstreamOptions) -> Result<crate::poly_dual_mesh::DualMesh, crate::MeshError> { /* ... */ }
```

#### Function `upstream_feature_sets`

`simpleMarkFeatures` on `primal` (no dual is built).

# Errors
[`MeshError::Construction`] for a face-zone label out of range.

```rust
pub fn upstream_feature_sets(primal: &crate::poly_dual_mesh::PolyMesh, opts: &UpstreamOptions) -> Result<FeatureSets, crate::MeshError> { /* ... */ }
```

#### Function `primal_edges`

The mesh edges in OpenFOAM's `primitiveMesh::calcEdges` numbering for an
unordered polyMesh: `(min, max)` point pairs in lexicographic order.

```rust
pub fn primal_edges(primal: &crate::poly_dual_mesh::PolyMesh) -> Vec<(usize, usize)> { /* ... */ }
```

## Module `snappy_hex_mesh`

`snappyHexMesh` — automatic split-hex meshing around triangulated (STL)
surfaces.

> **Provenance.** The three-phase structure, the split-hex hanging-node
> handling, and the control-parameter names re-implemented here are derived
> from OpenFOAM's `snappyHexMesh` utility
> (`src/mesh/snappyHexMesh`, © OpenFOAM Foundation / OpenCFD Ltd., GPL-3.0).
> This is an independent OUTRAM PARK re-implementation in Rust, not the
> official OpenFOAM software (see the crate-level notice and `TRADEMARKS.md`).

Starting from a background hex mesh (see [`background`]; typically the output
of [`crate::block_mesh`]), `snappyHexMesh` runs three phases:

1. **Castellation** ([`castellation`], **implemented**) — octree cell
   refinement to the surface level, then removal of cells on the far side of
   the surface from a `keep_point`. Produces a valid, conforming refined
   [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) plus a point/face
   [`PolyPatchMesh`](poly_topology::PolyPatchMesh) the later phases move.
2. **Snapping** ([`snapping`], **implemented**) — morph the castellated
   boundary onto the STL via nearest-point projection, Laplacian patch
   smoothing, and a quality-gated relaxation, then rebuild the `FvMesh`.
   Feature-edge snapping is a restricted (tested) addition.
3. **Layer addition** ([`layers`], **implemented, restricted**) — insert
   graded prismatic boundary layers at the wall patch with expansion-ratio
   grading and quality-limited collapse. Two placements exist and the driver
   picks per case: the medial-axis **interior shrink-and-insert** (the real
   `snappyLayerDriver` behaviour, volume-conserving) wherever it stays
   watertight, and an **outward extrusion** fallback on octree hanging-node
   regions, which grows the domain. Which one you got is not something to
   assume — see the [`layers`] module docs, and prefer
   [`crate::driver::mesh_from_surface`], which measures it and reports a
   [`LayerOutcome`](crate::driver::LayerOutcome).

Run all three together with [`generate`] (this module's top-level entry), or
call the phase functions individually. For a one-call path that also picks
the background mesh, converts to `PolyMesh` and grades the result, use
[`crate::driver::mesh_from_surface`] instead.

## Status (bead op-ax7.2)

| Phase | State | What works |
|---|---|---|
| STL input | ✅ done | ASCII + binary reader, inside/outside, nearest point |
| Castellation | ✅ done | octree refinement + region removal → valid `FvMesh` + topology |
| Snapping | ✅ done | projection + smoothing + quality-gated morph + rebuild; feature-edge (restricted) |
| Layer addition | 🟡 restricted | graded prism insertion + collapse; medial-axis interior shrink-and-insert where watertight, outward-extrusion fallback elsewhere |

## Minimal example

```no_run
use outram_foam_mesh::snappy_hex_mesh::{
    background::{BackgroundMesh, Bounds},
    castellation::{castellate, CastellationControls},
    stl::read_stl,
};
use outram_foam_basic_lib::primitives::Vector3;

let surface = read_stl("sphere.stl").unwrap();
let (lo, hi) = surface.bounding_box().unwrap();
let domain = Bounds::new(lo, hi).expanded(0.5);
let background = BackgroundMesh::uniform(domain, 10, 10, 10);

// Keep the region OUTSIDE the sphere (external-flow domain).
let keep_point = domain.min; // a far corner, outside the closed surface
let controls = CastellationControls::new(background, 2, keep_point);

let castellated = castellate(&surface, &controls).unwrap();
println!("refined mesh has {} cells", castellated.n_cells());
```

```rust
pub mod snappy_hex_mesh { /* ... */ }
```

### Modules

## Module `background`

Background hex mesh for `snappyHexMesh` castellation.

`snappyHexMesh` never starts from nothing: it refines and carves an existing
all-hex background mesh (in OpenFOAM, the output of `blockMesh`). This module
provides the minimal thing castellation needs — an axis-aligned box divided
into a uniform `nx × ny × nz` grid of hexahedral cells. Everything downstream
(the octree) subdivides these level-0 cells.

```rust
pub mod background { /* ... */ }
```

### Types

#### Struct `Bounds`

Axis-aligned bounding box `[m]`.

Stored as the two extreme corners `min` (lowest x, y, z) and `max`
(highest). Used both for the background-mesh extent and for individual
octree cell boxes.

```rust
pub struct Bounds {
    pub min: outram_foam_basic_lib::primitives::Vector3,
    pub max: outram_foam_basic_lib::primitives::Vector3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `min` | `outram_foam_basic_lib::primitives::Vector3` | Lower corner (min x, y, z) `[m]`. |
| `max` | `outram_foam_basic_lib::primitives::Vector3` | Upper corner (max x, y, z) `[m]`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: Vector3, b: Vector3) -> Self { /* ... */ }
  ```
  Construct from two corner points; the components are sorted so `min` is

- ```rust
  pub fn expanded(self: &Self, pad: f64) -> Self { /* ... */ }
  ```
  Uniformly grow the box by `pad` `[m]` on every side (used to wrap a

- ```rust
  pub fn centre(self: &Self) -> Vector3 { /* ... */ }
  ```
  Box centre `[m]`.

- ```rust
  pub fn span(self: &Self) -> Vector3 { /* ... */ }
  ```
  Side lengths `(dx, dy, dz)` `[m]`.

- ```rust
  pub fn volume(self: &Self) -> f64 { /* ... */ }
  ```
  Volume `dx·dy·dz` `[m³]`.

- ```rust
  pub fn diagonal(self: &Self) -> f64 { /* ... */ }
  ```
  Length of the space diagonal `[m]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `BackgroundMesh`

A uniform hexahedral background mesh: a box split into `nx × ny × nz`
equal cells.

This is the level-0 grid that castellation refines. Cell `(i, j, k)` (with
`0 ≤ i < nx`, etc.) spans
`[min + (i,j,k)·h, min + (i+1,j+1,k+1)·h]` where `h = span / (nx,ny,nz)`.

```rust
pub struct BackgroundMesh {
    pub bounds: Bounds,
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bounds` | `Bounds` | Overall domain extent `[m]`. |
| `nx` | `usize` | Cell divisions along x, y, z. |
| `ny` | `usize` | Cell divisions along y. |
| `nz` | `usize` | Cell divisions along z. |

##### Implementations

###### Methods

- ```rust
  pub fn uniform(bounds: Bounds, nx: usize, ny: usize, nz: usize) -> Self { /* ... */ }
  ```
  Build a uniform background mesh over `bounds` with `nx × ny × nz` cells.

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Total number of level-0 cells (`nx·ny·nz`).

- ```rust
  pub fn cell_size(self: &Self) -> Vector3 { /* ... */ }
  ```
  Level-0 cell size `(dx, dy, dz)` `[m]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `castellation`

Castellation — Phase 1 of `snappyHexMesh` (**implemented**).

Castellation turns a uniform background hex mesh into a body-fitted "castle
wall" staircase around a surface, in two steps:

1. **Octree refinement.** Level-0 background cells near the surface are
   recursively split into 8 children until they reach the requested surface
   refinement level. Proximity is measured with
   [`TriangleSoup::distance_to`], so a shell of small cells forms around the
   STL while the far field stays coarse.
2. **Region removal.** Each surviving leaf cell is kept or discarded by an
   inside/outside test ([`TriangleSoup::contains_point`]) against the
   `keep_point` (`locationInMesh` in OpenFOAM): cells on the same side of the
   closed surface as `keep_point` are kept, the rest are carved away. The
   faces newly exposed by removal become a boundary patch on the surface.

## Conforming split-hex output (2:1 and beyond)

When a refined (fine) cell abuts a coarse cell, the coarse cell's shared side
is emitted as several smaller faces — one per fine neighbour — so the coarse
cell simply becomes a polyhedron with more than six faces. This is exactly
OpenFOAM's split-hex handling of hanging nodes, and it keeps the output a
valid, closed [`FvMesh`] with no T-junctions inside any single face. Faces
are always emitted at the **finer** of the two neighbours' resolutions.

## What is *not* modelled here

Feature-edge refinement, `nCellsBetweenLevels` gap filling, and 2:1 level
balancing are not implemented — the face-emission scheme above stays valid
for arbitrary level jumps, so balancing is a mesh-quality nicety rather than
a correctness requirement. Snapping and layer addition are separate phases
(see [`crate::snappy_hex_mesh::snapping`] and
[`crate::snappy_hex_mesh::layers`]).

```rust
pub mod castellation { /* ... */ }
```

### Types

#### Struct `CastellationControls`

User controls for the castellation phase.

Mirrors the `castellatedMeshControls` block of a `snappyHexMeshDict`, reduced
to the parameters this Phase-1 implementation honours.

```rust
pub struct CastellationControls {
    pub background: crate::snappy_hex_mesh::background::BackgroundMesh,
    pub surface_level: usize,
    pub refinement_distance: Option<f64>,
    pub keep_point: outram_foam_basic_lib::primitives::Vector3,
    pub surface_patch_name: String,
    pub cyclic_axes: [bool; 3],
    pub selection: RefinementSelection,
    pub min_refine_cells: usize,
    pub removal: CellRemoval,
    pub surface_regions: Option<SurfaceRegionPatches>,
    pub problem_cell_order: Option<Vec<[usize; 4]>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `background` | `crate::snappy_hex_mesh::background::BackgroundMesh` | The uniform background hex mesh to refine. |
| `surface_level` | `usize` | Target octree refinement level applied to cells near the surface.<br>Level `n` cells are `2ⁿ` times finer than the background per axis. |
| `refinement_distance` | `Option<f64>` | Width of the refinement band around the surface `[m]`. A cell is refined<br>while its centre lies within this distance of the surface. `None`<br>auto-selects a one-cell band (the cell's half space-diagonal at its<br>current level), which refines the cells the surface actually passes<br>through plus their immediate shell. |
| `keep_point` | `outram_foam_basic_lib::primitives::Vector3` | `locationInMesh` — a point in the region to KEEP. Cells on the same side<br>of the closed surface as this point survive region removal. |
| `surface_patch_name` | `String` | Name of the boundary patch created on the carved surface. |
| `cyclic_axes` | `[bool; 3]` | Which background-box axes are **periodic**: `cyclic_axes[a]` makes the<br>two outer patches normal to axis `a` (`0 = x → xMin/xMax`,<br>`1 = y → yMin/yMax`, `2 = z → zMin/zMax`) a<br>[`PatchKind::Cyclic`] pair instead of two independent<br>[`PatchKind::Patch`]es, with their `cyclic_partner` links resolved.<br><br>Defaults to all `false` (the historical behaviour — six plain patches).<br><br>Marking an axis cyclic only makes sense when the *geometry* is periodic<br>along it; the halves must end up conformal, which<br>[`check_conformity`](crate::snappy_hex_mesh::check_conformity) verifies. |
| `selection` | `RefinementSelection` | How cells are selected for refinement; see [`RefinementSelection`].<br>Defaults to [`RefinementSelection::DistanceBand`] so existing callers'<br>meshes (and their pins) are unchanged. |
| `min_refine_cells` | `usize` | `minRefinementCells` (upstream default in the tutorials: 0). Only read<br>by [`RefinementSelection::SurfaceIntersection`]. |
| `removal` | `CellRemoval` | How cells are removed after refinement; see [`CellRemoval`]. Defaults to<br>[`CellRemoval::CentreParity`] so existing callers' meshes are unchanged. |
| `surface_regions` | `Option<SurfaceRegionPatches>` | Named patches per surface region (ESI `patchInfo` / one `flange_patch1`<br>per STL solid); `None` puts every carved face in the single patch<br>[`Self::surface_patch_name`]. Needs [`CellRemoval::EsiSplitMesh`]. |
| `problem_cell_order` | `Option<Vec<[usize; 4]>>` | Order in which `markFacesOnProblemCells` visits the cells (ESI: polyMesh cell<br>label). `None` visits in the port's leaf order. Only read by<br>[`CellRemoval::EsiSplitMesh`]; see<br>[`Self::with_esi_problem_cell_order`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(background: BackgroundMesh, surface_level: usize, keep_point: Vector3) -> Self { /* ... */ }
  ```
  Convenience constructor with the common defaults (auto band, surface

- ```rust
  pub fn with_esi_problem_cell_order(self: Self, order: Vec<[usize; 4]>) -> Self { /* ... */ }
  ```
  Visit the cells in this order (`[level, i, j, k]` on each cell's own lattice,

- ```rust
  pub fn with_surface_region_patches(self: Self, patch_names: Vec<String>, region_of_tri: Vec<usize>) -> Self { /* ... */ }
  ```
  One patch per surface region, named `patch_names[region]` (ESI's

- ```rust
  pub fn with_baffle_region_removal(self: Self) -> Self { /* ... */ }
  ```
  Remove cells the way OpenFOAM's `baffleAndSplitMesh` does

- ```rust
  pub fn with_esi_split_mesh_removal(self: Self) -> Self { /* ... */ }
  ```
  Remove cells the way ESI v2506's `doRefine` does after

- ```rust
  pub fn with_surface_intersection_refinement(self: Self) -> Self { /* ... */ }
  ```
  Select cells the way OpenFOAM's `surfaceOnlyRefine` does

- ```rust
  pub fn with_cyclic_axis(self: Self, axis: usize) -> Self { /* ... */ }
  ```
  Mark background-box axis `axis` (`0 = x`, `1 = y`, `2 = z`) periodic, so

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SurfaceRegionPatches`

Which surface region each triangle belongs to and what its patch is called
(gh:#1466). `region_of_tri[i]` indexes `patch_names`; for a surface read
with [`crate::snappy_hex_mesh::stl::read_stl_zoned`] these are
`StlZoned::zones` and the `<surface>_<solid>` names ESI gives its patches.

```rust
pub struct SurfaceRegionPatches {
    pub patch_names: Vec<String>,
    pub region_of_tri: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `patch_names` | `Vec<String>` | One patch name per region, in patch order. |
| `region_of_tri` | `Vec<usize>` | Region index of each triangle of the surface. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `CellRemoval`

How [`castellate`] decides which refined cells to keep.

```rust
pub enum CellRemoval {
    CentreParity,
    BaffleRegion,
    EsiSplitMesh,
}
```

##### Variants

###### `CentreParity`

Historical rule of this port: keep a cell when the ray-parity inside
test of its centre agrees with the keep point's. NOT what OpenFOAM does.

###### `BaffleRegion`

OpenFOAM's `baffleAndSplitMesh` outcome: keep the face-connected region
holding `locationInMesh`, faces cut by the surface (cell-centre
segments) being baffles. See
[`crate::snappy_hex_mesh::refinement::baffle_region`].

###### `EsiSplitMesh`

ESI v2506's whole post-refinement chain: `removeInsideCells(1)`,
`danglingCellRefine(21)` and `(24)`, `baffleAndSplitMesh` with
`markFacesOnProblemCells`. Needs
[`RefinementSelection::SurfaceIntersection`]. See
[`crate::snappy_hex_mesh::esi_split`].

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `RefinementSelection`

How [`castellate`] chooses the cells to split.

```rust
pub enum RefinementSelection {
    DistanceBand,
    SurfaceIntersection,
}
```

##### Variants

###### `DistanceBand`

Historical rule of this port: a cell is split while its centre lies
within a distance band of the surface. NOT what OpenFOAM does.

###### `SurfaceIntersection`

OpenFOAM's `snappyRefineDriver::surfaceOnlyRefine`: a cell is split when
a cell-centre to cell-centre segment through one of its faces
intersects the surface and the surface's level exceeds the cell's,
followed by `hexRef8::consistentRefinement` (face 2:1). See
[`crate::snappy_hex_mesh::refinement`].

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SurfaceFace`

A boundary face lying on the carved surface, retained (with its corner
points) so the Phase-2 snapping pass has explicit geometry to project.

```rust
pub struct SurfaceFace {
    pub owner_cell: usize,
    pub corners: [usize; 4],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `owner_cell` | `usize` | Owning kept-cell index in the output [`FvMesh`]. |
| `corners` | `[usize; 4]` | The four corner point indices into [`CastellatedMesh::points`],<br>counter-clockwise as seen from outside the mesh. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CastellatedMesh`

Result of castellation: a valid refined [`FvMesh`] plus enough octree/point
bookkeeping for the later phases to build on.

```rust
pub struct CastellatedMesh {
    pub fv_mesh: outram_foam_basic_lib::mesh::FvMesh,
    pub points: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub topology: crate::snappy_hex_mesh::poly_topology::PolyPatchMesh,
    pub surface_faces: Vec<SurfaceFace>,
    pub cells_by_level: Vec<usize>,
    pub max_level: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fv_mesh` | `outram_foam_basic_lib::mesh::FvMesh` | The refined, region-removed finite-volume mesh (validated). |
| `points` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Deduplicated mesh points `[m]` (corners of the kept cells). `FvMesh` itself<br>stores only geometry, so these are carried separately for snapping. |
| `topology` | `crate::snappy_hex_mesh::poly_topology::PolyPatchMesh` | Full point + face-connectivity view of the same mesh, in OpenFOAM face<br>order (internal faces first, then boundary faces by patch). This is the<br>moving-mesh substrate the snapping and layer phases mutate and rebuild<br>via [`PolyPatchMesh::build_fvmesh`]. It shares [`points`](Self::points)'<br>indexing. |
| `surface_faces` | `Vec<SurfaceFace>` | Boundary faces on the carved surface, with their corner points. |
| `cells_by_level` | `Vec<usize>` | Number of kept cells at each refinement level (`cells_by_level[l]`). |
| `max_level` | `usize` | The finest refinement level reached. |

##### Implementations

###### Methods

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Total kept cell count (same as `fv_mesh.n_cells`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `castellate`

Run the castellation phase, producing a refined, region-removed [`FvMesh`].

# Errors
- [`MeshError::Construction`] if the surface is empty, or if the assembled
  mesh fails [`FvMesh::validate`] (a bug guard — should not happen).

```rust
pub fn castellate(surface: &crate::snappy_hex_mesh::stl::TriangleSoup, controls: &CastellationControls) -> Result<CastellatedMesh, crate::MeshError> { /* ... */ }
```

## Module `cyclic`

# Cyclic (periodic) patch support for `snappyHexMesh`

A cyclic patch pair is *conformal*: local face `i` of half A couples to local
face `i` of half B, and the two halves are related by a rigid **translation**
— the separation vector `t`. `FvMeshBuilder` enforces the bookkeeping half of
that contract (mutual partners, equal `size`; see
[`BoundaryPatch::new_cyclic`]) but performs **no geometric check**, so a mesh
whose halves have drifted out of alignment still builds. This module supplies
the missing geometric half:

- [`check_conformity`] — the **V&V gate**. Asserts that every local face pair
  has the same vertex count and that its face centres differ by one common
  separation vector, to a tolerance. This is what makes a cyclic BC valid.
- [`CyclicPointConstraints`] — the machinery that lets the snapping phase
  *move* points that lie on a cyclic plane without breaking the above.

## Why snapping needs this

`snappyHexMesh`'s snapping phase projects wall-patch points onto the STL.
Before this module, any wall point that also lay on a non-wall boundary face
was **frozen** ([`super::snapping`]'s `frozen_patch_points`), cyclic planes
included. That is conservative and conformity-safe — a point that never moves
cannot break the pairing — but it means the geometry is **never body-fitted
where it crosses a periodic plane**. For a fuel-subchannel mesh, whose rods
and spacer grid are cut by the periodic planes, the rod walls stay at their
staircase (castellated) position exactly on the seams.

The fix is the constraint OpenFOAM applies to coupled patches
(`syncTools::syncPointList` + the patch's own `pointConstraint`): a point on a
cyclic plane may move **within that plane**, and its displacement is
**synchronised with its partner point** so both halves move identically.
Because the separation vector is then unchanged, conformity is preserved
exactly while the in-plane geometry is properly snapped.

Formally, for a partner pair `(p, p + t)` with plane normal `n̂ = t̂`:

1. **Project into the plane** — `d ← d − (d · n̂) n̂`, so the periodic plane
   stays flat (it is a domain boundary).
2. **Synchronise** — `d_A = d_B = ½(d_A + d_B)`, so `(p_B + d_B) − (p_A + d_A)`
   remains exactly `t`.

Step 2 alone is sufficient for conformity; step 1 additionally keeps the
periodic planes planar, which is what a translationally-periodic domain wants.

## Scope and limits

**Translational cyclics only.** Rotational (`cyclicPolyPatch` with a rotation
transform) and non-conformal `cyclicAMI` pairs are out of scope here and are
rejected rather than silently mishandled — see [`CyclicError`]. Extending to
rotational periodicity means replacing the constant `t` with a rotation about
an axis in both the pairing and the sync step.

All lengths are metres.

```rust
pub mod cyclic { /* ... */ }
```

### Types

#### Enum `CyclicError`

Why a cyclic pairing or conformity check failed.

```rust
pub enum CyclicError {
    UnresolvedPartner {
        patch: usize,
        name: String,
    },
    AsymmetricPartner {
        patch: usize,
        partner: usize,
    },
    SizeMismatch {
        patch: usize,
        size: usize,
        partner: usize,
        partner_size: usize,
    },
    FaceShapeMismatch {
        local: usize,
        n_a: usize,
        n_b: usize,
    },
    SeparationMismatch {
        local: usize,
        found: [f64; 3],
        expected: [f64; 3],
        error: f64,
    },
    NotTranslational {
        patch: usize,
    },
}
```

##### Variants

###### `UnresolvedPartner`

A patch marked [`PatchKind::Cyclic`] has no resolved partner.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `patch` | `usize` | Index of the offending patch. |
| `name` | `String` | Its name. |

###### `AsymmetricPartner`

The two halves disagree on who their partner is.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `patch` | `usize` | Index of the patch whose partner does not name it back. |
| `partner` | `usize` | The partner it names. |

###### `SizeMismatch`

The two halves have different face counts, so face `i` ↔ face `i` cannot
hold.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `patch` | `usize` | Index of the first half. |
| `size` | `usize` | Its face count. |
| `partner` | `usize` | Index of the partner half. |
| `partner_size` | `usize` | The partner's face count. |

###### `FaceShapeMismatch`

A local face pair has differing vertex counts.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `local` | `usize` | Local face index within the patch. |
| `n_a` | `usize` | Vertex count on the first half. |
| `n_b` | `usize` | Vertex count on the partner half. |

###### `SeparationMismatch`

A local face pair's centres are not related by the pair's separation
vector — the halves have drifted out of alignment.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `local` | `usize` | Local face index within the patch. |
| `found` | `[f64; 3]` | The separation this pair exhibits `[m]`. |
| `expected` | `[f64; 3]` | The separation the pair is supposed to have `[m]`. |
| `error` | `f64` | Magnitude of the discrepancy `[m]`. |

###### `NotTranslational`

A rotational or otherwise non-translational cyclic was encountered.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `patch` | `usize` | Index of the offending patch. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
#### Struct `CyclicPair`

One resolved, translational cyclic patch pair.

```rust
pub struct CyclicPair {
    pub patch_a: usize,
    pub patch_b: usize,
    pub separation: outram_foam_basic_lib::primitives::Vector3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `patch_a` | `usize` | Patch index of the first half. |
| `patch_b` | `usize` | Patch index of the partner half. |
| `separation` | `outram_foam_basic_lib::primitives::Vector3` | Separation `t` `[m]` such that `centre_b = centre_a + t` for every local<br>face pair. |

##### Implementations

###### Methods

- ```rust
  pub fn plane_normal(self: &Self) -> Vector3 { /* ... */ }
  ```
  Unit normal of the cyclic planes — the normalised separation direction.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CyclicPointConstraints`

Per-point cyclic constraints for the snapping phase.

Built once from the castellated topology; consumed every snapping iteration
by [`Self::constrain_and_sync`].

```rust
pub struct CyclicPointConstraints {
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
  pub fn is_constrained(self: &Self, point: usize) -> bool { /* ... */ }
  ```
  Is this point constrained by any cyclic plane?

- ```rust
  pub fn n_pairings(self: &Self) -> usize { /* ... */ }
  ```
  Number of partner point pairings resolved.

- ```rust
  pub fn build(topo: &PolyPatchMesh, tol: f64) -> Result<Self, CyclicError> { /* ... */ }
  ```
  Build the constraints from a topology's cyclic patches.

- ```rust
  pub fn constrain_and_sync(self: &Self, disp: &mut [Vector3], local_of: &HashMap<usize, usize>) { /* ... */ }
  ```
  Apply the cyclic constraint to a displacement field, in place.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `resolve_pairs`

Resolve every cyclic pair in `topo`, verifying the bookkeeping contract.

Each pair is reported **once** (with `patch_a < patch_b`). The separation is
taken from local face `0` and then required of every other local face by
[`check_conformity`].

# Errors

[`CyclicError::UnresolvedPartner`], [`CyclicError::AsymmetricPartner`] or
[`CyclicError::SizeMismatch`] if the patch bookkeeping is inconsistent.

```rust
pub fn resolve_pairs(topo: &super::poly_topology::PolyPatchMesh) -> Result<Vec<CyclicPair>, CyclicError> { /* ... */ }
```

#### Function `check_conformity`

**The V&V gate.** Verify every cyclic pair is geometrically conformal.

For each pair and each local face `i`, requires that half A's face `i` and
half B's face `i` have the same vertex count and that their centres differ by
the pair's separation vector to within `tol` metres.

Passing this is what makes a `cyclic` boundary condition valid on the mesh:
the solver couples face `i` to face `i` and assumes they are the same patch
of geometry displaced by `t`.

# Errors

Any [`CyclicError`]; in particular [`CyclicError::SeparationMismatch`] naming
the first local face that has drifted, and by how much.

```rust
pub fn check_conformity(topo: &super::poly_topology::PolyPatchMesh, tol: f64) -> Result<(), CyclicError> { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_CYCLIC_TOL`

Default geometric tolerance `[m]` for matching cyclic faces and points.

Chosen well below any realistic cell size so a genuine mismatch is caught,
while absorbing the round-off of centroid arithmetic.

```rust
pub const DEFAULT_CYCLIC_TOL: f64 = 1e-9;
```

## Module `dict`

`snappyHexMeshDict` reader (gh:#1470).

**Upstream followed:** OpenFOAM-dev `852527377c7e87e33d51b19ca8974b5db60af28b`,
`applications/utilities/mesh/generation/snappyHexMesh/snappyHexMesh.C` and
`snappyHexMeshDriver/{refinementParameters,snapParameters}/*.C`; the
tutorial dictionary is `tutorials/mesh/snappyHexMesh/flange/system/snappyHexMeshDict`.
Note the keyword is `insidePoint` / `insidePoints` in dev (`locationInMesh`
is the ESI spelling); both are read.

**No silent skip.** Every keyword in the file is either *honoured* (it
changes the controls this crate builds) or listed in
[`SnappyDict::not_ported`] with its dotted path. A caller (the
`snappyHexMesh` binary) prints that list, so a keyword that does nothing is
never invisible. What is honoured is deliberately small; the rest of the
dictionary is the work of gh:#1465 to #1469.

`#include "file"` is resolved by [`SnappyDict::read`] (via
`FoamFile::read_with_includes`); `#includeEtc` is not supported by the
parser and is reported by [`SnappyDict::parse`] as a not-ported keyword.

```rust
pub mod dict { /* ... */ }
```

### Types

#### Struct `GeometryEntry`

One `geometry` entry.

```rust
pub struct GeometryEntry {
    pub name: String,
    pub kind: String,
    pub file: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Name used by `refinementSurfaces` and region keywords. |
| `kind` | `String` | `type`: `triSurface` is read; `sphere`, `box`, ... are reported. |
| `file` | `Option<String>` | `file` (triSurface only). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SnappyDict`

The parts of a `snappyHexMeshDict` this crate acts on, and a report of the
rest.

```rust
pub struct SnappyDict {
    pub castellated_mesh: bool,
    pub snap: bool,
    pub add_layers: bool,
    pub geometry: Vec<GeometryEntry>,
    pub surface_levels: Vec<(String, usize, usize)>,
    pub inside_points: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub n_cells_between_levels: Option<usize>,
    pub min_refinement_cells: Option<usize>,
    pub n_smooth_patch: Option<usize>,
    pub n_solve_iter: Option<usize>,
    pub honoured: Vec<String>,
    pub not_ported: Vec<(String, String)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `castellated_mesh` | `bool` | `castellatedMesh`. |
| `snap` | `bool` | `snap`. |
| `add_layers` | `bool` | `addLayers`. |
| `geometry` | `Vec<GeometryEntry>` | `geometry`. |
| `surface_levels` | `Vec<(String, usize, usize)>` | `refinementSurfaces.<name>.level (min max)`. |
| `inside_points` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | `insidePoint` / `insidePoints` / `locationInMesh`. |
| `n_cells_between_levels` | `Option<usize>` | `nCellsBetweenLevels`. |
| `min_refinement_cells` | `Option<usize>` | `minRefinementCells`. |
| `n_smooth_patch` | `Option<usize>` | `snapControls.nSmoothPatch`. |
| `n_solve_iter` | `Option<usize>` | `snapControls.nSolveIter`. |
| `honoured` | `Vec<String>` | Dotted paths of keywords that were read and used. |
| `not_ported` | `Vec<(String, String)>` | Dotted paths of keywords present in the file and NOT acted on, each<br>with the reason. |

##### Implementations

###### Methods

- ```rust
  pub fn parse(text: &str) -> Result<Self, MeshError> { /* ... */ }
  ```
  Parse dictionary text (no `#include` resolution).

- ```rust
  pub fn read</* synthetic */ impl AsRef<Path>: AsRef<Path>>(path: impl AsRef<Path>) -> Result<Self, MeshError> { /* ... */ }
  ```
  Read `path`, resolving `#include "name"` relative to it.

- ```rust
  pub fn to_meshing_controls(self: &Self, domain: Bounds, divisions: [usize; 3]) -> Result<MeshingControls, MeshError> { /* ... */ }
  ```
  Build [`MeshingControls`] from this dictionary on a background mesh of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `background_from_points`

Recover the bounds and per-axis cell counts of an axis-aligned uniform
background hex mesh (a `blockMesh` box) from its points: the distinct
coordinates per axis, merged within `1e-9` of the span, give `n + 1` planes.

# Errors
[`MeshError::Construction`] for fewer than two planes on an axis (or no
points).

```rust
pub fn background_from_points(points: &[outram_foam_basic_lib::primitives::Vector3]) -> Result<(crate::snappy_hex_mesh::background::Bounds, [usize; 3]), crate::MeshError> { /* ... */ }
```

## Module `esi_snap`

Snapping stages of ESI v2506 `snappyHexMesh`, translated (gh:#1467, epic #1459):
the plain (no feature) loop of `snappySnapDriver::doSnap`, on a polyMesh read in.

**Upstream (OpenFOAM.com ESI v2506, `src/mesh/snappyHexMesh/snappyHexMeshDriver/snappySnapDriver.C`):**
`calcSnapDistance` (832), `smoothPatchDisplacement` (290), `preSmoothPatch`
(872), `calcNearestSurface` (1797), `outwardsDisplacement` (706),
`smoothDisplacement` (2134), `edgePatchDist` (631); and from
`src/dynamicMesh/motionSmoother/motionSmootherAlgo.C`: `setDisplacement` (467),
`smooth`/`avg` (`motionSmootherAlgoTemplates.C`), `curPoints` (748);
`src/meshTools/algorithms/PointEdgeWave/` (`PointEdgeWave.C`, `pointEdgePointI.H`);
`primitiveMesh::calcEdges` (edge order), `primitiveMeshTools::makeFaceCentresAndAreas`,
`face::centre`, `PrimitivePatch::calcMeshData` / `calcPointFaces`.

**What is translated:** the operation order of each routine (loops over the
same index orders, the same operand grouping), so that results can be compared
bit for bit; see `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`
(round 3, task 2) for the stage table. **What is not:** features
(`snappySnapDriverFeature.C`), `detectNearSurfaces` (ESI overrides 0 of 12130
points on the flange), `freezeExposedPoints` (no frozen zone), baffles and
faceZones, parallel synchronisation (serial only), `repatchToSurface`,
`mergePatchFaces`, and the quality check of `scaleMesh`. Everything here is opt-in; the
older [`crate::snappy_hex_mesh::snapping::snap`] is untouched.

Numerics: plain IEEE `+ - * /` and `sqrt` only, in upstream's grouping, no
`mul_add`; no transcendental is involved, so there is no PETIR residual here.

```rust
pub mod esi_snap { /* ... */ }
```

### Types

#### Type Alias `V`

A vector as three doubles, so every operation is spelled out.

```rust
pub type V = [f64; 3];
```

#### Struct `SnapMesh`

A polyMesh (serial): the faces of the boundary patches are
`faces[neighbour.len()..]`, all of them adapt patches.

```rust
pub struct SnapMesh {
    pub points: Vec<V>,
    pub faces: Vec<Vec<usize>>,
    pub owner: Vec<usize>,
    pub neighbour: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<V>` | Point coordinates. |
| `faces` | `Vec<Vec<usize>>` | Face vertex rings, internal faces first. |
| `owner` | `Vec<usize>` | Owner cell of every face. |
| `neighbour` | `Vec<usize>` | Neighbour cell of every internal face. |

##### Implementations

###### Methods

- ```rust
  pub fn n_internal_faces(self: &Self) -> usize { /* ... */ }
  ```
  Number of internal faces.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PatchAddr`

`indirectPrimitivePatch` addressing of all boundary faces:
`meshPoints` in order of first appearance, `pointFaces` per local point.

```rust
pub struct PatchAddr {
    pub face_ids: Vec<usize>,
    pub mesh_points: Vec<usize>,
    pub point_faces: Vec<Vec<usize>>,
    pub local_faces: Vec<Vec<usize>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `face_ids` | `Vec<usize>` | Mesh face index of each patch face. |
| `mesh_points` | `Vec<usize>` | Mesh point of each local patch point. |
| `point_faces` | `Vec<Vec<usize>>` | Patch faces (local indices) using each local point, ascending. |
| `local_faces` | `Vec<Vec<usize>>` | Faces as local point indices. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MeshEdges`

Mesh edges and `pointEdges` in `primitiveMesh::calcEdges` order.

```rust
pub struct MeshEdges {
    pub edges: Vec<[usize; 2]>,
    pub point_edges: Vec<Vec<usize>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `edges` | `Vec<[usize; 2]>` | Edge end points in creation order. |
| `point_edges` | `Vec<Vec<usize>>` | Edges of each point in creation order. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `patch_addressing`

`PrimitivePatch::calcMeshData` and `calcPointFaces` for the boundary faces.

```rust
pub fn patch_addressing(mesh: &SnapMesh) -> PatchAddr { /* ... */ }
```

#### Function `face_centres_areas`

`primitiveMeshTools::makeFaceCentresAndAreas` (ESI v2506): face centres and
area vectors.

```rust
pub fn face_centres_areas(points: &[V], faces: &[Vec<usize>]) -> (Vec<V>, Vec<V>) { /* ... */ }
```

#### Function `face_centre`

`face::centre(points)` (ESI v2506 `face.C`), which `smoothPatchDisplacement`
uses for the boundary faces (it is not the same arithmetic as the mesh's face centre).

```rust
pub fn face_centre(points: &[V], f: &[usize]) -> V { /* ... */ }
```

#### Function `calc_snap_distance`

`calcSnapDistance` (v2506 `snappySnapDriver.C:832`): `snapTol` times the
longest patch edge at each patch point.

```rust
pub fn calc_snap_distance(mesh: &SnapMesh, pa: &PatchAddr, snap_tol: f64) -> Vec<f64> { /* ... */ }
```

#### Function `calc_nearest_surface`

`calcNearestSurface` (v2506 `snappySnapDriver.C:1797`), one unzoned surface,
no region restriction: the displacement to the nearest surface point within
`snap_dist` (zero where none is). Returns the displacement and the number of
points with no surface in range.

```rust
pub fn calc_nearest_surface(local_points: &[V], snap_dist: &[f64], surface: &crate::snappy_hex_mesh::stl::TriangleSoup) -> (Vec<V>, usize) { /* ... */ }
```

#### Function `outwards_displacement`

`outwardsDisplacement` (v2506 `snappySnapDriver.C:706`): the local index of the
first patch point whose displacement points through its surrounding patch faces
(ESI prints a warning and stops looking), if any. Does not change the displacement.

```rust
pub fn outwards_displacement(mesh: &SnapMesh, pa: &PatchAddr, patch_disp: &[V]) -> Option<usize> { /* ... */ }
```

#### Function `smooth_patch_displacement`

`smoothPatchDisplacement` (v2506 `snappySnapDriver.C:290`, serial, no baffles):
the displacement of each patch point towards the blend of the average of its
boundary-face centres and of its internal-face centres.

```rust
pub fn smooth_patch_displacement(mesh: &SnapMesh, pa: &PatchAddr) -> Vec<V> { /* ... */ }
```

#### Function `cell_centres`

`primitiveMeshTools::makeCellCentresAndVols` (v2506), cell centres only.

```rust
pub fn cell_centres(mesh: &SnapMesh) -> Vec<V> { /* ... */ }
```

#### Function `pre_smooth_patch_iteration`

One iteration of `preSmoothPatch`'s loop body when the mesh check finds no face
in error (`scaleMesh` with scale 1): `setDisplacement(patchDisp)` then
`points = oldPoints + 1 * displacement`, where the displacement is zero off the
patch. ESI reports "Moving mesh using displacement scaling : min:1 max:1" and
0 faces in error for each of the three iterations of the flange. Returns the
displacement used.

```rust
pub fn pre_smooth_patch_iteration(mesh: &mut SnapMesh, pa: &PatchAddr) -> Vec<V> { /* ... */ }
```

#### Function `mesh_edges`

`primitiveMesh::calcEdges(false)`, serial, `nInternalPoints` unknown: edges are
found face by face (`getEdge` creates `(point, next)`), then renumbered by
increasing lower end point and, within it, increasing higher end point (the
`oldToNew` pass); `pointEdges` are sorted. The end-point order of each edge
is the creation order.

```rust
pub fn mesh_edges(mesh: &SnapMesh) -> MeshEdges { /* ... */ }
```

#### Function `edge_patch_dist`

`edgePatchDist` (v2506 `snappySnapDriver.C:631`): for every mesh edge, the
distance from its centre to the origin of the wall point `PointEdgeWave`
propagated to it (seeds: the patch points). Returns the distances and the
number of wave iterations.

```rust
pub fn edge_patch_dist(mesh: &SnapMesh, me: &MeshEdges, pa: &PatchAddr) -> (Vec<f64>, usize) { /* ... */ }
```

#### Function `smooth_displacement`

`snappySnapDriver::smoothDisplacement` (v2506 `snappySnapDriver.C:2134`):
`n_iter` times `motionSmootherAlgo::smooth(oldDisp, edgeGamma, disp)` with
`edgeGamma = 1/(edgePatchDist + SMALL)`; points off the patch move to
`0.5 * old + 0.5 * weighted neighbour average`, patch points keep their value.
`disp` is the displacement field on all mesh points.

```rust
pub fn smooth_displacement(mesh: &SnapMesh, me: &MeshEdges, pa: &PatchAddr, disp: &mut [V], n_iter: usize) { /* ... */ }
```

#### Function `displacement_field`

The displacement field handed to `smoothDisplacement`: the patch displacement on
the patch points (`motionSmootherAlgo::setDisplacement`), zero elsewhere.

```rust
pub fn displacement_field(mesh: &SnapMesh, pa: &PatchAddr, patch_disp: &[V]) -> Vec<V> { /* ... */ }
```

## Module `esi_split`

The cell-removal stages of `snappyRefineDriver::doRefine` that follow
`surfaceOnlyRefine`, translated for the octree (gh:#1466).

**Upstream: OpenFOAM.com ESI v2506**, `src/mesh/snappyHexMesh`:

| stage | ESI v2506 | here |
|---|---|---|
| `removeInsideCells(nBufferLayers)` | `snappyRefineDriver.C:1760`, `meshRefinementBaffles.C:4927` and `:5014` (`splitMesh`) | [`remove_inside_cells`] |
| `danglingCellRefine(nFaces)` | `snappyRefineDriver.C:1002` | [`dangling_cell_refine`] |
| `baffleAndSplitMesh` | `snappyRefineDriver.C:2754`, `meshRefinementBaffles.C:4635` | [`baffle_and_split_mesh`] |
| `handleSnapProblems` -> `markFacesOnProblemCells` | `meshRefinementBaffles.C:3795`, `meshRefinementProblemCells.C:383` | inside [`baffle_and_split_mesh`] |

The order in `doRefine` (`snappyRefineDriver.C:3560-3690`) is
`removeInsideCells(1)`, `danglingCellRefine(21)`, `danglingCellRefine(24)`,
`refinementInterfaceRefine` (selected 0 cells on the flange; not ported),
`baffleAndSplitMesh`. [`esi_stages`] runs them in that order.

## How the octree stands in for the polyMesh

The mesh is the full octree (every leaf ever created, dead or alive) with an
`alive` flag per leaf: a removed cell leaves its faces behind as boundary
faces of its alive neighbours, exactly as `removeCells` exposes them. Faces
are enumerated by [`octree_faces`] (the split-hex faces of the finer side).
A face's vertex ring is the finer-side quad with every lattice edge midpoint
that exists in the octree inserted (hexRef8 inserts the mid-points of refined
edges into every face that uses the edge, also faces of cells that are not
refined); a mesh point exists when it is a corner of any leaf, dead or alive,
because removal never deletes a point from the faces of a surviving cell.
This is the assumption P10-P14 of the V&V record test.

## Cell order

`markFacesOnProblemCells` visits cells in order and updates its boundary
marks as it goes, so its result can depend on cell order. The port visits
alive leaves in leaf-vector order (the nested order of
[`crate::snappy_hex_mesh::castellation::castellate`]), not OpenFOAM's
(blockMesh order, a refined cell keeps its label, children 1..7 appended).
The V&V record states whether that mattered.

## Not ported

`locationsOutsideMesh` and leak closure, `frozenPoints`/`frozenFaces`, more
than one `locationInMesh`, cellZones/faceZones (`zonify`), the
`removeEdgeConnectedCells` branch and the `minVolCollapseRatio` collapse
check of `markFacesOnProblemCells` (both off in the flange dictionary:
`removeEdgeConnectedCells false`, `minVolCollapseRatio` unset), and
`refinementInterfaceRefine`.

```rust
pub mod esi_split { /* ... */ }
```

### Types

#### Type Alias `CellKey`

A cell's identity on its own level lattice: `[level, i, j, k]` (the key the ESI
reference fixtures use).

```rust
pub type CellKey = [usize; 4];
```

#### Struct `BaffleStats`

What [`baffle_and_split_mesh`] counted, to compare with the ESI log.

```rust
pub struct BaffleStats {
    pub n_hit_internal: usize,
    pub n_hit_boundary: usize,
    pub n_problem_faces: usize,
    pub n_kept: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_hit_internal` | `usize` | Internal faces cut by the surface (ESI: "Introducing baffles for N faces"). |
| `n_hit_boundary` | `usize` | Faces cut by the surface that are boundary faces (exposed or domain). |
| `n_problem_faces` | `usize` | Additional internal faces baffled by `markFacesOnProblemCells`<br>(ESI: "marked N additional internal faces"). |
| `n_kept` | `usize` | Cells kept after the final `splitMeshRegions`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `EsiStageCells`

The cells after each ESI stage, for the differential against the ESI
intermediate meshes. Each entry is `(level, centre)`.

```rust
pub struct EsiStageCells {
    pub after_remove_inside: Vec<(usize, outram_foam_basic_lib::primitives::Vector3)>,
    pub dangling_selected: [Vec<usize>; 2],
    pub after_dangling: Vec<(usize, outram_foam_basic_lib::primitives::Vector3)>,
    pub final_cells: Vec<(usize, outram_foam_basic_lib::primitives::Vector3)>,
    pub baffle: BaffleStats,
    pub problem_faces: Vec<(CellKey, CellKey)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `after_remove_inside` | `Vec<(usize, outram_foam_basic_lib::primitives::Vector3)>` | After `removeInsideCells(nBufferLayers = 1)` (ESI: "Selected for keeping"). |
| `dangling_selected` | `[Vec<usize>; 2]` | Cells selected per iteration by `danglingCellRefine(21)`, then `(24)`. |
| `after_dangling` | `Vec<(usize, outram_foam_basic_lib::primitives::Vector3)>` | After both dangling-cell stages. |
| `final_cells` | `Vec<(usize, outram_foam_basic_lib::primitives::Vector3)>` | After `baffleAndSplitMesh` (the final castellated cells). |
| `baffle` | `BaffleStats` | The counts of the baffle step. |
| `problem_faces` | `Vec<(CellKey, CellKey)>` | The faces `markFacesOnProblemCells` marked, as the two cells each separates<br>(smaller key first). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `esi_stage_cells`

The cells after each ESI stage for a whole case: `surfaceOnlyRefine`, then
[`esi_stages`]. Exists for the ESI v2506 differential
(`verification_and_validation/snappy_hex_mesh_esi_v2506`); `None` if
`locationInMesh` is outside the mesh or removed.

```rust
pub fn esi_stage_cells(surface: &crate::snappy_hex_mesh::stl::TriangleSoup, controls: &crate::snappy_hex_mesh::castellation::CastellationControls) -> Option<EsiStageCells> { /* ... */ }
```

## Module `layers`

Layer addition — Phase 3 of `snappyHexMesh` (**implemented, restricted
scope: medial-axis interior shrink-and-insert for isolated flat/convex
walls, with an always-watertight outward-extrusion fallback elsewhere**).

The final phase inserts graded prismatic ("boundary layer") cells at a wall
boundary patch so that near-wall gradients can be resolved. Two placements
are available and the driver ([`add_layers`]) picks per candidate: the
**interior shrink-and-insert** (the real `snappyLayerDriver` behaviour,
preferred) is used wherever it stays watertight, and an **outward
extrusion** (which never moves an existing point) is the robust fallback for
near-wall regions where the interior insert cannot — see "Honest scope".
This module ports the *geometric core* of OpenFOAM's `snappyLayerDriver`
(`src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C`) and its
medial-axis mesh mover: the per-point normal displacement field, the
**inward shrink of the near-wall mesh** limited by a medial-axis distance
(`medialAxisMeshMover::calculateDisplacement`,
`externalDisplacementMeshMover/medialAxisMeshMover.C:1622`; the
thickness/medial-distance truncation at `medialAxisMeshMover.C:1770` and
`:1801`, and `snappyLayerDriver::shrinkMeshMedialDistance`,
`snappyHexMeshDriver/snappyLayerDriverShrink.C:1348`), the graded thickness
distribution (`layerParameters::layerThickness`, `layerParameters.C:697`),
the prism-block insertion (in OpenFOAM `addPatchCellLayer::setRefinement`,
`polyTopoChange/polyTopoChange/addPatchCellLayer.C`), and the
quality-limited collapse (in OpenFOAM the `EXTRUDE`/`NOEXTRUDE` unextrusion
loop, `snappyLayerDriver.C:217`).

**CORRECTED 2026-11-04 (gh:#1215):** the "quality-limited collapse" here is
a *global* reduction of the layer count ([`add_layers`] tries `n`, then
`n − 1`, … everywhere); there is no per-point `extrudeMode` and no local
unextrusion, as "Honest scope" below says. The per-point port of
`snappyLayerDriver`'s loop (`extrudeMode`, `handleFeatureAngle`,
`setupLayerInfoTruncation`, `checkAndUnmark`) is
`outram_park_fork_cfmesh::snappy_layers`; it was not factored into a shared
place because this module works on `PolyPatchMesh` / `CastellatedMesh`
and that crate builds without `outram-foam-basic-lib`.

## Algorithm implemented here — interior shrink-and-insert

Unlike a bare outward extrusion (which would push the domain boundary out
along the wall normal), this **keeps the outer wall boundary fixed** and
makes room for the layers by displacing the near-wall mesh *inward* on the
fluid side, then inserts the graded prisms into the opened gap. The domain
occupies exactly the same outer envelope before and after; the near-wall
cells are compressed rather than the boundary being extended.

1. **Pick the wall patch.** The first [`PatchKind::Wall`] patch (typically
   the carved surface produced by castellation) is chosen as the layer
   patch; every other patch is untouched.
2. **Point normals + medial-axis distance.** Each wall *point* gets a normal
   equal to the area-weighted average of the OUTWARD area vectors of the
   incident wall faces, normalised (this is the shrink/insert direction).
   Each point also gets a **medial-axis distance proxy**: the smallest
   incident owner-cell depth normal to the wall, `owner cell volume /
   wall-face area` — a first-order stand-in for `medialDist_` in
   `medialAxisMeshMover.C:1766` (the distance from the wall to where
   displacements from opposite walls would meet).
3. **Graded, capped block thickness.** For `n = n_surface_layers` layers
   with first-layer thickness `t` and expansion ratio `r` the thicknesses
   are `[t, t·r, t·r², …]` ([`layer_thicknesses`]); their sum `T` is the
   block thickness. `T` is scaled down per point by whichever of two caps
   binds: the face-size cap ([`LayerControls::max_thickness_fraction`] `· √A`,
   the convex-corner self-intersection guard) or the **medial-axis cap**
   ([`LayerControls::medial_axis_thickness_ratio`] `· owner-depth`, which
   keeps the inward shrink from inverting the owner cell — the analogue of
   `maxThicknessToMedialRatio`, `medialAxisMeshMover.C:1646`).
4. **Shrink + graded rings.** The original wall points are moved INWARD by
   the capped block thickness `T_l` (shrinking the owner cells and carrying
   the old wall faces to the new fluid-side interface). `n` new point rings
   are laid from that shrunk interface back out to the ORIGINAL wall
   position, so ring `n` coincides with the untouched boundary. The grading
   is **reversed** relative to a bare extrusion: the THINNEST layer (`t`)
   sits adjacent to the wall and the thickest adjacent to the owner cell —
   the correct near-wall distribution.
5. **Prism cells.** For each wall face (an `m`-gon) `n` stacked prism cells
   are created between successive point rings. The *original* wall face
   (now at the shrunk interface) becomes an INTERNAL face between its old
   owner cell and the first prism; interfaces between successive prisms are
   internal; side faces on edges shared by two wall faces are internal
   (owner→neighbour wound); side faces on the patch rim tile the gap the
   shrink opened on the domain's side boundaries; the outer cap of the
   outermost prism lands on the original wall and forms the NEW wall patch
   (same name/kind). Rim sides form a `layerSide` wall patch. Everything is
   assembled into a fresh [`PolyPatchMesh`] (internal faces first, then
   boundary patches) and rebuilt with [`PolyPatchMesh::build_fvmesh`].
6. **Watertight + quality gate, with mode and layer-count fallback.** Each
   candidate is checked for zero non-positive-volume cells (min volume `≥`
   [`QualityLimits::min_vol`]) AND watertightness (every cell's signed
   face-area-vector sum vanishes to a tiny fraction of the largest face
   area). For each layer count — the full count first, then fewer — the
   interior insert is tried first; if it fails the gate the **outward
   extrusion** of the same count is tried (it moves no existing point, so it
   is always watertight); if neither passes, one fewer layer is tried, down
   to zero (original mesh returned unchanged). **No mesh with a
   negative-volume or non-watertight cell is ever returned.** The
   medial-axis + face-size caps in step 3 normally prevent inversion
   outright, so the volume half of the gate is a coarse backstop; the
   watertight half is what routes hanging-node / embedded near-wall regions
   to the outward-extrusion fallback (see "Honest scope").

## Honest scope — what IS and is NOT modelled

**What is now real (vs the earlier extrude-outward primitive).** The layers
are inserted *inside* the original domain: the outer wall boundary does not
move, the near-wall owner cells are compressed by an inward point
displacement, and the prism block fills the opened gap with the thinnest
layer at the wall. The inward displacement is limited by a medial-axis-style
distance cap so the owner cell cannot invert. This is the
shrink-then-`addPatchCellLayer` structure of the real driver, reduced to its
geometric essentials and verified on a flat wall.

**Where the interior insert applies, and the fallback.** Moving the wall
points is only watertight when no wall point is a **hanging node** on a
coarser neighbour: on an octree-refined castellated mesh a wall point that
sits at the mid-edge of a coarse cell's face is not tracked by that coarse
face, so displacing it opens a coarse/fine gap and the coarse cell fails
closure. The driver therefore restricts the interior insert to regions where
it stays watertight — in practice **uniform / isolated flat or convex
near-wall regions** (the hand-built flat-wall V&V case) — and falls back to
the earlier **outward-extrusion primitive** everywhere it does not (a carved,
refined surface patch such as the sphere-in-box case). The fallback grows the
prism block *outward* along the wall normal and so extends the mesh rather
than inserting inside it; it is a correct, watertight, quality-checked
extrusion but NOT the interior-conserving behaviour. Turning the whole
carved-surface case into a true interior insert needs the point-displacement
machinery below.

**What still differs from OpenFOAM's `snappyLayerDriver`.** Several parts of
the full medial-axis mover are deliberately NOT ported and are honest
limitations of this increment:
- **Single-cell shrink, no interior smoothing.** Only the wall points are
  displaced, so the whole block thickness is absorbed by the *one* near-wall
  cell. OpenFOAM smooths the displacement several cells deep
  (`nSmoothDisplacement`, lambda-mu smoothing, `smoothLambdaMuDisplacement`,
  `medialAxisMeshMover.C:1424`) so the compression spreads into the interior.
  This restricts the safe block thickness to a fraction of the *first* cell's
  depth, not the medial distance of the whole channel.
- **Medial distance is a first-order proxy** (owner-cell depth), not the true
  medial-axis skeleton distance computed by the point-wave in
  `medialAxisSmoothingInfo` (`snappyLayerDriverShrink.C:861`). It is correct
  for an isolated flat/convex wall but conservative in a narrow channel and
  not validated for concave corners where opposite walls interact.
- **No per-face layer termination / feature-edge handling.** Layer count is
  reduced globally (fewer layers everywhere), never per-point or per-face,
  and there is no `handleFeatureAngleLayerTerminations`
  (`snappyLayerDriverShrink.C:491`) sharp-edge stop or
  `findIsolatedRegions` island removal.
- **No multi-patch layer coupling** across shared feature edges, and no
  baffle / faceZone handling.

## Verification & validation (V&V)

**Methodology.** A hand-built two-cell flat-wall [`CastellatedMesh`] (two
unit cubes side by side in `x`, their `z = 1` top faces forming a 2-quad
wall patch, fluid on the `z < 1` side) has layers inserted with `n = 3`,
`t = 0.1 m`, `r = 1.5`. The reference is the closed-form geometric grading:
thicknesses `[0.1, 0.15, 0.225] m`, total `0.475 m`. Pass criteria: the
outer wall boundary does NOT move outward (`max z ≤ 1` — the distinguishing
test vs the old extrude-outward behaviour); the owner cells shrink to
`z ∈ [0, 0.525]`; exactly `n` prism cells per wall face; the thinnest layer
(`0.1 m`) is adjacent to the wall and the successive layer heights reproduce
the expansion ratio; the rebuilt [`FvMesh`] validates; zero negative-volume
cells; each cell's signed face-area-vector sum vanishes (watertight); the
default quality gate accepts.

**Results (measured 2026-07-20).** New cell count `8` (was `2`) — `3` new
cells per wall face. Maximum point `z = 1.000 m` (outer boundary fixed; the
old extrusion would have reached `z = 1.475 m`). Shrunk owner top at
`z = 0.525 m` (`= 1 − 0.475`). Column point heights from the wall inward
`[0.100, 0.150, 0.225] m` (thinnest `0.100 m` layer at the wall), successive
ratios `1.5000` and `1.5000` (err `< 1e-12`). Rebuilt mesh validates;
`n_negative_volume_cells = 0`; `min_cell_volume = 0.100 m³` (thinnest prism,
`1 m² × 0.1 m`); `max_non_ortho ≈ 0°`, `max_skewness ≈ 0`; default
[`QualityLimits`] accepts. Per-cell area-vector sum magnitude `< 1e-12 m²`
(watertight). An oversized `t = 100 m` request is capped by the medial-axis
and face-size caps to a non-inverting block (`min_cell_volume > 0`).

A second case exercises the fallback boundary on a REFINED carved mesh (a
unit sphere carved from a `[-2,2]³` box at level 1, `848` cells). There the
interior-insert candidate is measurably NON-watertight (`max |ΣSf| ≈ 2.0e-3
m²`, `48` unclosed coarse cells) because wall points on hanging nodes are
displaced; the driver rejects it and returns the outward-extrusion fallback
(`1472` cells, `max |ΣSf| < 1e-9 m²` watertight, zero negative-volume cells).
See the tests in this module.

[`PatchKind::Wall`]: outram_foam_basic_lib::mesh::PatchKind::Wall
[`FvMesh`]: outram_foam_basic_lib::mesh::FvMesh

```rust
pub mod layers { /* ... */ }
```

### Types

#### Struct `LayerControls`

Controls for the layer-addition phase (subset of `addLayersControls`).

The thickness of the wall-nearest layer can be given directly
([`first_layer_thickness`](Self::first_layer_thickness)) or implied by a
target [`final_layer_thickness`](Self::final_layer_thickness); see
[`LayerControls::first_thickness`].

```rust
pub struct LayerControls {
    pub n_surface_layers: usize,
    pub expansion_ratio: f64,
    pub first_layer_thickness: f64,
    pub final_layer_thickness: Option<f64>,
    pub max_thickness_fraction: f64,
    pub medial_axis_thickness_ratio: f64,
    pub quality_limits: crate::snappy_hex_mesh::QualityLimits,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_surface_layers` | `usize` | Number of prism layers to add at the wall. |
| `expansion_ratio` | `f64` | Geometric expansion ratio `r` between successive layers (`> 0`, usually<br>`> 1` so cells grow away from the wall). Dimensionless. |
| `first_layer_thickness` | `f64` | Thickness of the layer nearest the wall `[m]`. Used directly unless<br>[`final_layer_thickness`](Self::final_layer_thickness) is `Some`. |
| `final_layer_thickness` | `Option<f64>` | Optional target thickness of the OUTERMOST layer `[m]`. When `Some`, the<br>first-layer thickness is derived from it via the OpenFOAM<br>`FIRST_AND_EXPANSION`/`FINAL_AND_EXPANSION` relation (see<br>`layerParameters.C:927`): `first = final / rⁿ⁻¹` for `r ≠ 1`, so the<br>geometric series ends on the requested final thickness. |
| `max_thickness_fraction` | `f64` | Cap on the total layer-block thickness at each wall point, as a fraction<br>of the local wall-face size `√(face area)` [dimensionless, `(0, 1]`].<br>If the graded total would exceed `max_thickness_fraction · √A` at a<br>point, that point's offsets are scaled down. This is the geometric guard<br>that keeps a prism from inverting where surface normals diverge; `0.5`<br>keeps the block below half the local cell size. |
| `medial_axis_thickness_ratio` | `f64` | Cap on the inward shrink at each wall point, as a fraction of the local<br>**medial-axis distance** proxy `owner cell volume / wall-face area` (the<br>near-wall cell's depth normal to the wall) [dimensionless, `(0, 1)`].<br>The layer block is inserted by displacing the wall points inward by the<br>block thickness; limiting that displacement to `medial_axis_thickness_ratio<br>· owner-depth` keeps the shrink from collapsing/inverting the owner cell.<br>This is the analogue of OpenFOAM's `maxThicknessToMedialRatio`<br>(`medialAxisMeshMover.C:1646`), with the true medial-axis distance<br>replaced by the first-order owner-cell-depth proxy (see the module docs'<br>"Honest scope"). `0.5` keeps the owner cell at least half its original<br>depth. |
| `quality_limits` | `crate::snappy_hex_mesh::QualityLimits` | Quality thresholds the layered mesh is gated on. A candidate that yields<br>any non-positive-volume cell (or a cell below `min_vol`) is retried with<br>fewer layers; see the module docs. |

##### Implementations

###### Methods

- ```rust
  pub fn first_thickness(self: &Self) -> f64 { /* ... */ }
  ```
  Effective first-layer thickness `[m]` — the thickness of the wall-nearest

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `layer_thicknesses`

Geometric layer thicknesses `[t, t·r, t·r², …]` `[m]` for `n` layers with
effective first-layer thickness `t` ([`LayerControls::first_thickness`]) and
expansion ratio `r`.

This is the grading arithmetic of Phase 3 in isolation (fully testable). The
returned vector has length `controls.n_surface_layers`; the total boundary-
layer thickness is its sum, `t·(rⁿ − 1)/(r − 1)` for `r ≠ 1`. Mirrors
`layerParameters::layerThickness` (`layerParameters.C:697`).

```rust
pub fn layer_thicknesses(controls: &LayerControls) -> Vec<f64> { /* ... */ }
```

#### Function `total_layer_thickness`

Total boundary-layer thickness `[m]` — the sum of [`layer_thicknesses`].

```rust
pub fn total_layer_thickness(controls: &LayerControls) -> f64 { /* ... */ }
```

#### Function `add_layers`

Insert graded prism layers at the wall patch, preferring medial-axis
interior shrink-and-insert and falling back to outward extrusion where the
interior insertion cannot stay watertight (see the module docs for the full
algorithm, restricted scope, and V&V).

For each candidate layer count (full count first, then fewer — the
quality-limited collapse), the `InteriorInsert`
topology is tried first at the first
[`PatchKind::Wall`](outram_foam_basic_lib::mesh::PatchKind::Wall) patch: it
displaces the near-wall mesh inward and inserts `n` graded prisms into the
opened gap, keeping the outer boundary fixed. That candidate is accepted
only if it is both **watertight** (every cell's signed face-area-vector sum
vanishes) and free of non-positive-volume cells; otherwise the
`ExtrudeOutward` topology (always watertight) is
tried at the same `n`. On success the returned [`CastellatedMesh`] has:
- `fv_mesh` / `topology` — the rebuilt, validated layered mesh. For an
  interior insert the original wall faces are internal at the shrunk
  fluid-side interface and a new wall patch of the same name caps the block
  back at the original boundary; for an outward extrusion the original wall
  faces are internal and the new wall patch caps the far (extruded) end. Both
  add a `layerSide` patch for rim faces.
- `surface_faces` — the NEW wall patch's quadrilateral faces (owner prism
  cell + corner points); non-quad wall faces are omitted from this list,
- `cells_by_level` — the original counts with the new prism cells added to
  the finest-level bucket,
- `points` / `max_level` — the new point list / unchanged finest level.

If no layer count in either mode meets the gate, the input mesh is returned
unchanged (never a mesh with a negative-volume or non-watertight cell).

# Errors
[`MeshError::Construction`] if the mesh has no wall patch, if the chosen wall
patch has no faces, or if a rebuilt mesh fails
[`FvMesh::validate`](outram_foam_basic_lib::mesh::FvMesh::validate).

```rust
pub fn add_layers(mesh: &crate::snappy_hex_mesh::castellation::CastellatedMesh, controls: &LayerControls) -> Result<crate::snappy_hex_mesh::castellation::CastellatedMesh, crate::MeshError> { /* ... */ }
```

## Module `poly_topology`

Point-based polyhedral mesh topology — the moving-mesh substrate the
snapping ([`crate::snappy_hex_mesh::snapping`]) and layer-addition
([`crate::snappy_hex_mesh::layers`]) phases operate on.

## Why this module exists

[`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) stores only *geometry* —
cell volumes/centres and face areas/centres — and no point coordinates or
face→point connectivity. Snapping and layer addition **move and add points**
and must then recompute all that geometry. This module carries the missing
half: the deduplicated point list plus, for every face, the ordered list of
point indices that form it (its polygon). From points + connectivity the
finite-volume geometry is regenerated exactly, so the workflow is:

1. build / obtain a [`PolyPatchMesh`] (castellation produces one),
2. move [`PolyPatchMesh::points`] and/or splice in new faces/cells,
3. call [`PolyPatchMesh::build_fvmesh`] to get a fresh, validated `FvMesh`.

## Geometry provenance

The face and cell geometry recomputation mirrors OpenFOAM's
`primitiveMeshTools`:
- face centre/area — `primitiveMeshFaceCentresAndAreas.C` /
  `makeFaceCentresAndAreas` (fan triangulation about the point average,
  area-weighted centroid). See
  `src/OpenFOAM/meshes/primitiveMesh/primitiveMeshCheck/primitiveMeshTools.C`.
- cell centre/volume — `primitiveMeshCellCentresAndVols.C` /
  `makeCellCentresAndVols` (face-pyramid decomposition about the face-centre
  average, `pyr3Vol = Sf · (Cf − cEst)`, volume ×1/3).
- non-orthogonality / skewness quality — `primitiveMeshTools::faceOrthogonality`
  and `faceSkewness`.

## Face-winding convention (REQUIRED of producers)

Every face's point list must be ordered so the polygon's right-hand-rule
normal points **owner → neighbour** for an internal face and **outward from
the owner cell** for a boundary face — identical to OpenFOAM's `faceAreas`
sign convention and to what [`FvMesh`] expects. [`build_fvmesh`] trusts this
ordering rather than re-deriving it from cell centres.

[`build_fvmesh`]: PolyPatchMesh::build_fvmesh

```rust
pub mod poly_topology { /* ... */ }
```

### Types

#### Struct `PolyPatchMesh`

A polyhedral mesh in point + face-connectivity form.

Faces are stored in OpenFOAM order: the `n_internal_faces` internal faces
(both owner and neighbour) first, then the boundary faces grouped by patch
exactly as [`patches`](Self::patches) describes. See the module docs for the
face-winding convention this type requires.

```rust
pub struct PolyPatchMesh {
    pub points: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub faces: Vec<Vec<usize>>,
    pub owner: Vec<usize>,
    pub neighbour: Vec<usize>,
    pub n_internal_faces: usize,
    pub n_cells: usize,
    pub patches: Vec<outram_foam_basic_lib::mesh::BoundaryPatch>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | Mesh point coordinates `[m]`. Moving snapping/layer motion edits this. |
| `faces` | `Vec<Vec<usize>>` | Per-face ordered point indices into [`points`](Self::points). Internal<br>faces first (`[0, n_internal_faces)`), then boundary faces by patch. |
| `owner` | `Vec<usize>` | `owner[f]` — owning cell of face `f` (all faces). |
| `neighbour` | `Vec<usize>` | `neighbour[f]` — neighbour cell of internal face `f`<br>(length == `n_internal_faces`). |
| `n_internal_faces` | `usize` | Number of internal faces (faces with a neighbour cell). |
| `n_cells` | `usize` | Number of cells. |
| `patches` | `Vec<outram_foam_basic_lib::mesh::BoundaryPatch>` | Boundary patch descriptors over the boundary faces (in face-index order). |

##### Implementations

###### Methods

- ```rust
  pub fn n_faces(self: &Self) -> usize { /* ... */ }
  ```
  Number of faces (internal + boundary).

- ```rust
  pub fn n_boundary_faces(self: &Self) -> usize { /* ... */ }
  ```
  Number of boundary faces.

- ```rust
  pub fn face_geometry(self: &Self) -> (Vec<Vector3>, Vec<Vector3>) { /* ... */ }
  ```
  Face area vectors `[m²]` and centres `[m]` for every face, in face order.

- ```rust
  pub fn cell_geometry(self: &Self, face_areas: &[Vector3], face_centres: &[Vector3]) -> (Vec<Vector3>, Vec<f64>) { /* ... */ }
  ```
  Cell centres `[m]` and volumes `[m³]` from the face geometry, by the

- ```rust
  pub fn build_fvmesh(self: &Self) -> Result<FvMesh, MeshError> { /* ... */ }
  ```
  Regenerate a validated [`FvMesh`] from the current points + connectivity.

- ```rust
  pub fn quality(self: &Self) -> MeshQuality { /* ... */ }
  ```
  Mesh-quality metrics (non-orthogonality, skewness, min volume) computed

- ```rust
  pub fn to_foam_poly_mesh(self: &Self) -> outram_foam_basic_lib::io::PolyMesh { /* ... */ }
  ```
  Convert to the writable `outram-foam-basic-lib`

- ```rust
  pub fn patch_face_ids(self: &Self, patch_index: usize) -> std::ops::Range<usize> { /* ... */ }
  ```
  Global face indices belonging to boundary patch `patch_index`.

- ```rust
  pub fn patch_point_ids(self: &Self, patch_index: usize) -> Vec<usize> { /* ... */ }
  ```
  The set of point indices used by boundary patch `patch_index`

- ```rust
  pub fn boundary_point_flags(self: &Self) -> Vec<bool> { /* ... */ }
  ```
  A boolean flag per point: `true` if the point lies on any boundary face.

- ```rust
  pub fn point_faces(self: &Self) -> Vec<Vec<usize>> { /* ... */ }
  ```
  For each point, the global face indices that use it (point→face adjacency).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `MeshQuality`

Mesh-quality summary — the metrics `snappyHexMesh` gates point motion and
layer addition on (a subset of `meshQualityControls`).

```rust
pub struct MeshQuality {
    pub max_non_ortho_deg: f64,
    pub max_skewness: f64,
    pub min_cell_volume: f64,
    pub n_negative_volume_cells: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_non_ortho_deg` | `f64` | Worst internal-face non-orthogonality `[degrees]` — the angle between the<br>face area vector and the owner→neighbour centre-to-centre vector. `0` is<br>perfectly orthogonal; OpenFOAM's default reject threshold is `65°`. |
| `max_skewness` | `f64` | Worst face skewness (dimensionless) over all faces — the normalised<br>offset of the face centre from the owner–neighbour line. OpenFOAM's<br>default reject threshold is `4.0`. |
| `min_cell_volume` | `f64` | Smallest cell volume `[m³]`. Non-positive means an inverted / degenerate<br>cell (fatal). |
| `n_negative_volume_cells` | `usize` | Number of cells whose volume is `<= 0` (negative-volume / inverted). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `QualityLimits`

Acceptance thresholds for [`MeshQuality`] — the quality gate.

Mirrors the handful of `meshQualityControls` limits the snapping and layer
phases enforce. A candidate mesh passes iff every metric is within limits.

```rust
pub struct QualityLimits {
    pub max_non_ortho_deg: f64,
    pub max_skewness: f64,
    pub min_vol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_non_ortho_deg` | `f64` | Maximum allowed non-orthogonality `[degrees]` (OpenFOAM default `65`). |
| `max_skewness` | `f64` | Maximum allowed skewness (OpenFOAM default `4.0`). |
| `min_vol` | `f64` | Minimum allowed cell volume `[m³]` (OpenFOAM default `1e-13`). |

##### Implementations

###### Methods

- ```rust
  pub fn accepts(self: &Self, q: &MeshQuality) -> bool { /* ... */ }
  ```
  True if `q` satisfies every limit (mesh is acceptable).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `face_area_and_centre`

Area vector `[m²]` and centre `[m]` of one polygonal face, by fan triangulation
about the point average (OpenFOAM `makeFaceCentresAndAreas`).

The returned area vector obeys the right-hand rule over the point order; a
triangle is handled directly for exactness. Units: `points` in metres, area
in `m²`, centre in metres.

```rust
pub fn face_area_and_centre(face: &[usize], points: &[outram_foam_basic_lib::primitives::Vector3]) -> (outram_foam_basic_lib::primitives::Vector3, outram_foam_basic_lib::primitives::Vector3) { /* ... */ }
```

## Module `refinement`

Surface-intersection refinement: `snappyRefineDriver::surfaceOnlyRefine`.

**Upstream followed:** OpenFOAM.com ESI **v2506** (maintainer decision on
gh:#1459, 2026-11-12, "ESI preferred"; the run that checks it is
`verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`).
~~OpenFOAM-dev (OpenFOAM Foundation) commit
`852527377c7e87e33d51b19ca8974b5db60af28b`~~ **CORRECTED 2026-11-12** to
ESI v2506, with the line numbers re-read from the v2506 sources (dev's in
brackets, background only):

- `snappyHexMeshDriver/snappyRefineDriver.C:342` `surfaceOnlyRefine`
  [dev :179] (loop, stop rule `nCellsToRefine == 0 || (iter >=
  overallMaxLevel && nCellsToRefine <= minRefineCells)`, `maxIter` 100);
- `meshRefinement/meshRefinementRefine.C:945` `markSurfaceRefinement`
  [dev :908] (one segment per face from `meshRefinement.C:288`
  `calcCellCellRays`: owner centre to neighbour centre, or, on a non-coupled
  boundary face, owner centre to the face centre from `calcNeighbourData`;
  segment extended by `rootSmall` of its length at each end; a cell is
  marked when the segment hits a surface whose level exceeds the cell's);
- `refinementSurfaces/refinementSurfaces.C:1265` `findHigherIntersection`
  [dev :410] (`findLineAny` on the triSurfaceMesh);
- `dynamicMesh/polyTopoChange/polyTopoChange/hexRef8/hexRef8.C:1567`
  `faceConsistentRefinement` [dev :1426] and `:2233` `consistentRefinement`
  [dev :2127] (face 2:1, `maxSet = true`);
- `meshRefinement/meshRefinementBaffles.C:4927` `splitMesh` and
  `snappyRefineDriver.C:2754` `baffleAndSplitMesh` for [`baffle_region`].

**Checked against the ESI binary (gh:#1459, 2026-11-12):** on the flange the
selected-cell counts per iteration and all 42788 cells and levels equal
ESI's, when the surface is read as upstream reads it
([`TriangleSoup::from_zoned`]: every `solid`, single-precision corners).

## What is ported, and what is not

Ported: one surface with a single (min = max) level; the face-segment test;
the 2:1 closure; the stop rule. The octree's face-adjacent leaf pairs are
the same pairs as upstream's split-hex faces (a coarse face adjoining four
fine cells is four faces, each with its own fine neighbour), so the
segments are the same segments.

Not ported (still [`RefinementSelection::DistanceBand`] territory):
`markSurfaceCurvatureRefinement` (needs `(min max)` levels and
`resolveFeatureAngle`; with `level (n n)` upstream skips it too),
`refinementRegions`, `features`, gap refinement, `maxGlobalCells` /
`maxLocalCells`, `nCellsBetweenLevels > 1` (`consistentSlowRefinement`),
coupled (cyclic) boundary faces, which are treated as ordinary boundary
faces here. Cell *removal* after refinement is by default this port's inside test;
~~not upstream's baffle-and-split (gh:#1466)~~ **UPDATED 2026-11-12:** upstream's
chain (`removeInsideCells(1)`, `danglingCellRefine`, `baffleAndSplitMesh` with
`markFacesOnProblemCells`) is [`crate::snappy_hex_mesh::esi_split`], opt-in
through `CastellationControls::with_esi_split_mesh_removal`.

[`RefinementSelection::DistanceBand`]: crate::snappy_hex_mesh::castellation::RefinementSelection::DistanceBand

```rust
pub mod refinement { /* ... */ }
```

### Functions

#### Function `refined_octree`

The refined octree of `surface_only_refine` as `(level, centre)` pairs plus
the per-iteration selected-cell counts, with no cell removal. Exists so the
ESI v2506 differential can compare the refinement stage on its own
(`verification_and_validation/snappy_hex_mesh_esi_v2506`).

```rust
pub fn refined_octree(surface: &crate::snappy_hex_mesh::stl::TriangleSoup, controls: &crate::snappy_hex_mesh::castellation::CastellationControls, max_iter: usize) -> (Vec<(usize, outram_foam_basic_lib::primitives::Vector3)>, Vec<usize>) { /* ... */ }
```

## Module `snapping`

Snapping — Phase 2 of `snappyHexMesh` (**implemented**).

Snapping morphs the castellated staircase boundary onto the STL surface so
the mesh becomes body-fitted. This is a pure-Rust port of the core of
OpenFOAM's `snappySnapDriver` (`src/mesh/snappyHexMesh/snappyHexMeshDriver/
snappySnapDriver.C`). The driver mirrored here is [`snap`], which reproduces
the essential loop of `snappySnapDriver::doSnap` (line 2574) minus the
parallel/coupled-patch and displacement-diffusion machinery that a serial,
single-region mesh does not need:

1. **Find patch points.** Collect the deduplicated points of the carved
   surface (`PatchKind::Wall`) patch — the points snapping projects. Mirrors
   the `indirectPrimitivePatch` (`ppPtr`) that `doSnap` builds over the
   adapt-patch faces.
2. **Compute displacements** (`calcNearestSurface`, line 1797). For each
   patch point find the nearest point on the STL
   ([`TriangleSoup::nearest_point`], exact closest surface point); the
   displacement is `target - current`. [`raw_surface_displacements`] exposes
   this projection primitive on its own.
3. **Patch smoothing** (`smoothPatchDisplacement`, line 290). The raw
   displacement field is Laplacian-smoothed over the patch-point adjacency
   (each point's displacement ← mean of its patch-neighbour displacements),
   for `n_smooth_patch` sweeps. This keeps the motion coherent so interior
   cells are not sheared apart. (OpenFOAM's version additionally blends
   boundary- and internal-face-centre averages with a manifold test; we use
   the simpler edge-Laplacian the task calls for and document the difference
   honestly.)
4. **Mesh-quality-gated relaxation** (`smoothDisplacement` +
   `scaleMesh`/`meshMover.scaleMesh`, lines 2134/2208). The smoothed
   displacement is applied scaled by a relaxation factor `lambda` (start
   1.0). The moved [`PolyPatchMesh`](crate::snappy_hex_mesh::PolyPatchMesh) is rebuilt ([`PolyPatchMesh::build_fvmesh`](crate::snappy_hex_mesh::PolyPatchMesh::build_fvmesh))
   and its [`quality`](crate::snappy_hex_mesh::PolyPatchMesh::quality) checked against
   [`QualityLimits`]. If the move is accepted it is committed; otherwise
   `lambda` is halved and retried (a few backoffs), reproducing OpenFOAM's
   "relax displacement until correct mesh" loop. Nearest-surface targets are
   recomputed every solve iteration so points converge onto the surface.
5. **Feature snapping with `pointConstraint` accumulation** (port of
   `snappySnapDriverFeature.C` — `binFeatureFace`/reconstruction lines
   931–993 and `featureAttractionUsingReconstruction` line 997 — together
   with the accumulator itself in `pointConstraintI.H`:
   `applyConstraint` line 48 and `constrainDisplacement` line 185). When
   [`SnapControls::feature_snap`] is set, STL feature edges (creases whose
   dihedral angle exceeds `feature_angle_deg`) **and feature points**
   (corners where ≥2 non-collinear feature edges meet) are extracted, and
   every wall patch point near a feature is classified with a per-point
   `PointConstraint` that governs how it is allowed to move:
   - **rank 0 — free on surface:** ordinary nearest-surface projection (as
     for a point with no nearby feature). `constrainDisplacement` is the
     identity here.
   - **rank 2 — on a feature edge:** the point is pulled onto the crease and
     its *smoothing correction* is constrained to the unit edge direction
     (`constrainDisplacement` keeps only the along-edge component), so the
     point slides along the crease but never drifts off it.
   - **rank 3 — on a feature point:** the point is pulled onto the corner
     vertex and fully fixed (its displacement is anchored, zero smoothing),
     so it lands exactly on the corner.

   The rank labels and the free→edge→point accumulation mirror OpenFOAM's
   `pointConstraint` exactly: applying one constraining direction promotes
   free→edge, a second non-collinear direction promotes edge→point, and a
   saturated point stays fixed. The one honest difference from OpenFOAM is
   the *source* of those directions — here they are feature-edge directions
   extracted from the STL geometry, not surface normals read from an
   `extendedFeatureEdgeMesh` file (see the *Honest scope* section below).

# Honest scope — what is and is not modelled

Implemented and tested here: geometry-derived feature-edge and
feature-point extraction; the full `pointConstraint` free/edge/point
accumulation and `constrainDisplacement`; exact corner snapping on convex
feature points; along-edge-constrained crease snapping; and preservation of
mesh validity + watertightness under all of the above.

Deliberately **not** modelled (documented, not silently skipped):
- **No `extendedFeatureEdgeMesh` file input.** OpenFOAM reads a precomputed
  feature-edge/point file (`surfaceFeatures`) with per-edge classification
  (external/internal/flat/open/multiple). Here features are detected from
  the triangle soup by dihedral angle only; concave vs convex creases are
  not distinguished and open/non-manifold edges are ignored.
- **No multi-region / multi-patch constraint combination.** OpenFOAM's
  `combine` merges constraints contributed by several surfaces/patches
  meeting at a point. This serial single-region port classifies each point
  against one feature set and does not combine across regions.
- **No `snapDist`-scaled trimming or per-point feature attraction from the
  binned pointFace surface normals.** Proximity uses a single feature band
  (a fraction of the local patch edge length), not OpenFOAM's per-point
  `snapDist`. Attraction is geometric (nearest point on edge / corner
  vertex), not reconstructed from plane–plane intersection of binned face
  normals.

Because [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) stores only geometry
and no point/face-vertex topology, snapping works on the point-based
[`PolyPatchMesh`](crate::snappy_hex_mesh::PolyPatchMesh) carried in [`CastellatedMesh::topology`]: it moves points,
then [`PolyPatchMesh::build_fvmesh`](crate::snappy_hex_mesh::PolyPatchMesh::build_fvmesh) recomputes all finite-volume geometry.

# Verification & validation

**Methodology.** `snap_sphere_projects_onto_surface` castellates a
unit-radius UV sphere inside a `[-2,2]³` background box meshed `8×8×8` with
surface refinement level 2 (finest cell ≈ 0.125 m), then snaps with the
default controls. It measures, for every wall patch point, the distance to
the exact nearest surface point before and after snapping, and checks the
rebuilt mesh validates, passes the default quality gate, has zero
negative-volume cells, and stays watertight (per-cell sum of signed
face-area vectors ≈ 0).

**Results (2026-07-17, host x86_64, release).**
- Pre-snap max patch-point→surface distance: **0.10500 m** (staircase error).
- Post-snap max patch-point→surface distance: **1.5e-4 m** — a **≈715×**
  reduction; mean post-snap distance **1.0e-4 m**.
- Rebuilt mesh: `FvMesh::validate()` Ok, `QualityLimits::default().accepts`
  true, 0 negative-volume cells, min cell volume **5.20e-4 m³**.
- Watertightness: max per-cell |Σ Sf| = **3.2e-17 m²** (machine zero).

Interpretation: the quality-gated blended Laplacian morph pulls the
castellated staircase essentially onto the analytic sphere (residual ~1e-4 m,
i.e. ~1/1000 of a finest cell) while keeping every cell valid, non-inverted,
and closed. Watertightness across the octree's 2:1 interfaces is preserved by
constraining T-junction hanging nodes to stay collinear with their coarse-edge
parents (`HangingConstraint`); without that constraint an unconstrained move
of the same points opens gaps of ~9e-4 m² per cell.

```rust
pub mod snapping { /* ... */ }
```

### Types

#### Struct `SnapControls`

Controls for the snapping phase (subset of OpenFOAM `snapControls`).

All lengths are in metres. Defaults mirror the commonly used
`snappyHexMeshDict` values (30 solve iterations, 3 patch-smoothing sweeps).

```rust
pub struct SnapControls {
    pub n_solve_iter: usize,
    pub n_smooth_patch: usize,
    pub tolerance: f64,
    pub quality: crate::snappy_hex_mesh::poly_topology::QualityLimits,
    pub feature_snap: bool,
    pub feature_angle_deg: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_solve_iter` | `usize` | Number of quality-gated point-displacement relaxation iterations<br>(`snapControls::nSolveIter`). Nearest-surface targets are recomputed at<br>the start of each, so points converge onto the surface progressively. |
| `n_smooth_patch` | `usize` | Number of Laplacian patch-smoothing sweeps applied to the displacement<br>field each solve iteration (`snapControls::nSmoothPatch`). |
| `tolerance` | `f64` | Relative distance (in cell sizes) a point may travel to the surface<br>(`snapControls::tolerance`). Retained for parity with the OpenFOAM<br>dictionary; the current serial morph gates motion on mesh quality rather<br>than this ratio, so it is advisory. |
| `quality` | `crate::snappy_hex_mesh::poly_topology::QualityLimits` | Mesh-quality limits the relaxation must satisfy for a move to be<br>committed (non-orthogonality `[deg]`, skewness, min cell volume `[m³]`). |
| `feature_snap` | `bool` | Enable feature snapping (port of `snappySnapDriverFeature.C` with the<br>`pointConstraint` accumulator of `pointConstraintI.H`): patch points near<br>an STL crease are constrained to slide along the feature edge, and points<br>near a corner (≥2 non-collinear feature edges meeting) are fully fixed on<br>the corner vertex. See the module docs (item 5 + *Honest scope*). |
| `feature_angle_deg` | `f64` | Dihedral-angle threshold `[degrees]` above which a shared STL edge counts<br>as a feature edge. Only used when `feature_snap` is set. A box has 90°<br>creases, so any threshold below 90 detects its edges. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FeatureEdge`

A detected STL feature edge: the two endpoints (metres) of a crease.

Public since 2026-11-05 (gh:#1276) so that `outram-park-fork-cfmesh`'s
port of cfMesh's `triSurfaceDetectFeatureEdges` can be tested against it.

```rust
pub struct FeatureEdge {
    pub a: outram_foam_basic_lib::primitives::Vector3,
    pub b: outram_foam_basic_lib::primitives::Vector3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `outram_foam_basic_lib::primitives::Vector3` | One endpoint `[m]`. |
| `b` | `outram_foam_basic_lib::primitives::Vector3` | The other endpoint `[m]`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `raw_surface_displacements`

The displacement each surface point *would* move under one unconstrained
projection to the surface — the raw input to the relaxation solve.

`result[p]` is the vector from `mesh.points[p]` (in metres) to its nearest
surface point, for every point that appears in a surface face; other points
map to the zero vector. This is the projection primitive
(`calcNearestSurface`, `snappySnapDriver.C:1797`) exposed on its own so
callers/tests can inspect the pull without running the full morph.

```rust
pub fn raw_surface_displacements(mesh: &crate::snappy_hex_mesh::castellation::CastellatedMesh, surface: &crate::snappy_hex_mesh::stl::TriangleSoup) -> Vec<outram_foam_basic_lib::primitives::Vector3> { /* ... */ }
```

#### Function `snap`

Morph the castellated boundary onto the surface.

Port of the core of OpenFOAM `snappySnapDriver::doSnap`
(`snappySnapDriver.C:2574`): iteratively projects the carved wall-patch
points onto `surface`, Laplacian-smooths the displacement over the patch,
and applies it under a mesh-quality gate, rebuilding the finite-volume
geometry from the moved points. See the module docs for the algorithm and
V&V results.

# Inputs
- `mesh` — a castellated mesh (from
  [`castellate`](crate::snappy_hex_mesh::castellation::castellate)); its
  [`topology`](CastellatedMesh::topology) must carry a `PatchKind::Wall`
  patch (the carved surface). Point coordinates are in metres.
- `surface` — the STL triangle soup to snap onto (metres).
- `controls` — solve/smoothing counts, quality limits, and feature-snap
  options.

# Returns
A new [`CastellatedMesh`] with the wall-patch points moved onto (or toward)
the surface and a freshly rebuilt, validated `fv_mesh`. Cell/face
connectivity and `surface_faces` corner indices are unchanged — only point
coordinates move.

# Errors
- [`MeshError::Construction`] if no `PatchKind::Wall` patch exists, or if the
  final moved mesh fails [`FvMesh::validate`](outram_foam_basic_lib::mesh::FvMesh::validate).

```rust
pub fn snap(mesh: &crate::snappy_hex_mesh::castellation::CastellatedMesh, surface: &crate::snappy_hex_mesh::stl::TriangleSoup, controls: &SnapControls) -> Result<crate::snappy_hex_mesh::castellation::CastellatedMesh, crate::MeshError> { /* ... */ }
```

#### Function `detect_feature_edges`

Detect feature edges of a triangle soup: edges shared by two triangles whose
dihedral angle exceeds `angle_deg`.

Two triangles share an edge when they share two vertices (matched by
coordinate, within a small absolute tolerance). The dihedral angle is the
angle between the two facet normals; a shared edge is a feature edge when
that angle exceeds the threshold. Restricted port of the feature-edge
extraction underlying `snappySnapDriverFeature.C`.

The output order follows a `HashMap` and is not deterministic; compare
it as a set. Open and non-manifold edges are never reported, and the
facets' relative orientation is not checked. Public since 2026-11-05
(gh:#1276), for the cfMesh differential.

```rust
pub fn detect_feature_edges(surface: &crate::snappy_hex_mesh::stl::TriangleSoup, angle_deg: f64) -> Vec<FeatureEdge> { /* ... */ }
```

## Module `stl`

Triangulated-surface (STL) input for `snappyHexMesh`.

A stereolithography (STL) file is a "triangle soup": an unordered list of
flat triangular facets, each with three corner points `[m]` and a (frequently
unreliable) stored normal. `snappyHexMesh` uses this surface for three
purposes, all provided here:

- **Proximity** — how close a background cell sits to the surface, driving
  octree refinement ([`TriangleSoup::nearest_point`],
  [`TriangleSoup::distance_to`]).
- **Inside/outside** — which side of a *closed* surface a point lies on,
  driving region removal ([`TriangleSoup::contains_point`], by odd/even ray
  crossing).
- **Projection** — the nearest surface point a boundary vertex snaps to
  (Phase 2; [`Triangle::closest_point`]).

Both ASCII and little-endian binary STL are read. Coordinates are treated as
metres to match the rest of the mesh layer; STL itself is unit-less, so the
caller is responsible for supplying a metre-scaled file.

```rust
pub mod stl { /* ... */ }
```

### Types

#### Struct `Triangle`

One triangular facet of a surface `[m]`.

The three corners `a`, `b`, `c` are stored in the file's winding order. The
geometric normal is recomputed from the corners (via [`Triangle::normal`])
rather than trusting the file's stored normal, which many CAD exporters get
wrong.

```rust
pub struct Triangle {
    pub a: outram_foam_basic_lib::primitives::Vector3,
    pub b: outram_foam_basic_lib::primitives::Vector3,
    pub c: outram_foam_basic_lib::primitives::Vector3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `outram_foam_basic_lib::primitives::Vector3` | First corner `[m]`. |
| `b` | `outram_foam_basic_lib::primitives::Vector3` | Second corner `[m]`. |
| `c` | `outram_foam_basic_lib::primitives::Vector3` | Third corner `[m]`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: Vector3, b: Vector3, c: Vector3) -> Self { /* ... */ }
  ```
  Construct a triangle from its three corner points `[m]`.

- ```rust
  pub fn normal(self: &Self) -> Vector3 { /* ... */ }
  ```
  Geometric (right-hand-rule) unit normal `(b-a) × (c-a)`, normalised.

- ```rust
  pub fn area(self: &Self) -> f64 { /* ... */ }
  ```
  Twice the triangle area (magnitude of the un-normalised cross product) `[m²]`.

- ```rust
  pub fn centroid(self: &Self) -> Vector3 { /* ... */ }
  ```
  Centroid (arithmetic mean of the three corners) `[m]`.

- ```rust
  pub fn closest_point(self: &Self, p: Vector3) -> Vector3 { /* ... */ }
  ```
  Closest point on the (filled) triangle to `p` `[m]`.

- ```rust
  pub fn ray_intersection(self: &Self, origin: Vector3, dir: Vector3) -> Option<f64> { /* ... */ }
  ```
  Signed ray/triangle intersection distance for a ray `origin + t*dir`,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TriangleSoup`

An unordered collection of triangular facets — a surface "triangle soup".

For inside/outside queries to be meaningful the surface must be closed and
(ideally) manifold; `snappyHexMesh` requires this of its input STL too.

```rust
pub struct TriangleSoup {
    pub triangles: Vec<Triangle>,
    pub name: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `triangles` | `Vec<Triangle>` | The facets making up the surface `[m]`. |
| `name` | `String` | Optional solid name from the STL `solid <name>` line. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, triangles: Vec<Triangle>) -> Self { /* ... */ }
  ```
  Build a soup from an explicit list of triangles (used by tests that

- ```rust
  pub fn from_zoned(z: &StlZoned) -> Self { /* ... */ }
  ```
  The surface as OpenFOAM's `triSurface::readSTL` hands it to

- ```rust
  pub fn uv_sphere(centre: Vector3, r: f64, n_lat: usize, n_lon: usize) -> Self { /* ... */ }
  ```
  A closed UV sphere of radius `r` `[m]` centred at `centre` `[m]`, as an

- ```rust
  pub fn cuboid(min: Vector3, max: Vector3) -> Self { /* ... */ }
  ```
  A closed axis-aligned box spanning `min`..`max` `[m]`, as an

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of facets.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  True if the soup has no facets.

- ```rust
  pub fn bounding_box(self: &Self) -> Option<(Vector3, Vector3)> { /* ... */ }
  ```
  Axis-aligned bounding box `(min, max)` of every vertex `[m]`.

- ```rust
  pub fn nearest_point(self: &Self, p: Vector3) -> Option<Vector3> { /* ... */ }
  ```
  Closest point on the whole surface to `p` `[m]` (brute force over all

- ```rust
  pub fn distance_to(self: &Self, p: Vector3) -> f64 { /* ... */ }
  ```
  Euclidean distance from `p` to the nearest surface point `[m]`.

- ```rust
  pub fn contains_point(self: &Self, p: Vector3) -> bool { /* ... */ }
  ```
  True if `p` is inside the *closed* surface, by ray-crossing parity

- ```rust
  pub fn segment_hits(self: &Self, a: Vector3, b: Vector3) -> bool { /* ... */ }
  ```
  True if the closed segment `a`–`b` crosses any facet (any-hit, the

- ```rust
  pub fn segment_first_hit(self: &Self, a: Vector3, b: Vector3) -> Option<usize> { /* ... */ }
  ```
  Index of the facet the segment `a -> b` meets FIRST walking from `a`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `StlZoned`

An STL as OpenFOAM's `triSurface::readSTL` returns it: points merged,
one zone (patch) per `solid` name (ASCII) or per attribute value (binary).

Differences from [`read_stl_bytes`] that matter for parity with upstream:
coordinates are rounded to **single precision** (`STLpoint` is `float`),
every `solid` of a multi-solid ASCII file is read (not only the first),
and coincident corners are merged into shared points.

```rust
pub struct StlZoned {
    pub points: Vec<outram_foam_basic_lib::primitives::Vector3>,
    pub triangles: Vec<[usize; 3]>,
    pub zones: Vec<usize>,
    pub names: Vec<String>,
    pub n_zones: usize,
    pub binary: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | The merged points `[m]`, in order of first appearance. |
| `triangles` | `Vec<[usize; 3]>` | Triangles (indices into [`Self::points`]), file order. |
| `zones` | `Vec<usize>` | Zone of each triangle (index into [`Self::names`] for ASCII). |
| `names` | `Vec<String>` | Zone names, ASCII only (`solid <name>`; a nameless solid is<br>`"solid"`). Empty for a binary file, as upstream: `triSurface::readSTL`<br>sets patch names only for ASCII. |
| `n_zones` | `usize` | Number of zones (for binary: distinct attribute values). |
| `binary` | `bool` | `true` if the file was read as binary. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `read_stl`

Read an STL file from `path`, auto-detecting ASCII vs binary.

Detection: a file whose leading non-whitespace token is `solid` *and* whose
body contains the `facet` keyword is treated as ASCII; otherwise it is read
as little-endian binary. (A binary STL may legally begin with the bytes
`solid`, so the `facet` check disambiguates.)

```rust
pub fn read_stl</* synthetic */ impl AsRef<std::path::Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>) -> Result<TriangleSoup, crate::MeshError> { /* ... */ }
```

#### Function `read_stl_bytes`

Parse STL from an in-memory byte buffer, auto-detecting ASCII vs binary.

```rust
pub fn read_stl_bytes(bytes: &[u8]) -> Result<TriangleSoup, crate::MeshError> { /* ... */ }
```

#### Function `read_stl_ascii_str`

Parse an ASCII STL from a string.

Grammar (whitespace-insensitive):
```text
solid <name>
  facet normal nx ny nz
    outer loop
      vertex x y z
      vertex x y z
      vertex x y z
    endloop
  endfacet
  ...
endsolid <name>
```
Several such blocks in one file are all read (one soup, region boundaries
not kept; see [`read_stl_zoned_bytes`] for the per-region reader).
The stored `normal` is parsed but ignored (the geometric normal is used).

```rust
pub fn read_stl_ascii_str(text: &str) -> Result<TriangleSoup, crate::MeshError> { /* ... */ }
```

#### Function `read_stl_binary`

Parse a little-endian binary STL from a byte buffer.

Layout: 80-byte header, `u32` facet count, then per facet 12 × `f32`
(normal + 3 vertices) plus a `u16` attribute-byte-count, all little-endian.

```rust
pub fn read_stl_binary(bytes: &[u8]) -> Result<TriangleSoup, crate::MeshError> { /* ... */ }
```

#### Function `read_stl_zoned_bytes`

Read an STL buffer as OpenFOAM's `triSurface::readSTL` does (see
[`StlZoned`]): format by `detectBinaryHeader`, then the ASCII or binary
reader, then `mergePointsMap` with OpenFOAM's tolerance (`100 *
doubleScalarSMALL` for ASCII, `10 *` for binary, absolute).

# Errors
[`MeshError::DictParse`] on a truncated binary file or a facet with a
vertex component that is not a number.

```rust
pub fn read_stl_zoned_bytes(bytes: &[u8]) -> Result<StlZoned, crate::MeshError> { /* ... */ }
```

#### Function `read_stl_zoned`

Read an STL file with [`read_stl_zoned_bytes`].

# Errors
As [`read_stl_zoned_bytes`], plus [`MeshError::DictParse`] wrapping an
I/O error.

```rust
pub fn read_stl_zoned</* synthetic */ impl AsRef<std::path::Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>) -> Result<StlZoned, crate::MeshError> { /* ... */ }
```

#### Function `merge_points`

OpenFOAM's `mergePoints(points, mergeTol, verbose, pointToUnique)`
(`Detail::mergePoints`): a point within `merge_tol` (absolute) of an
earlier point in distance-from-the-minimum order merges with it. Returns
the map to the unique points and their number; unique points are numbered
in order of their lowest-index member, as upstream's renumbering does.

```rust
pub fn merge_points(points: &[outram_foam_basic_lib::primitives::Vector3], merge_tol: f64) -> (Vec<usize>, usize) { /* ... */ }
```

### Types

#### Struct `SnappyHexMeshControls`

Controls for the full three-phase `snappyHexMesh` pipeline.

Bundles the per-phase controls and lets the snapping and layer phases be
switched off individually (`None`), mirroring the `snap`/`addLayers` toggles
in a `snappyHexMeshDict`. Castellation always runs (it produces the mesh the
later phases refine).

```rust
pub struct SnappyHexMeshControls {
    pub castellation: CastellationControls,
    pub snap: Option<SnapControls>,
    pub layers: Option<LayerControls>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `castellation` | `CastellationControls` | Phase 1 — octree refinement + region removal (always run). |
| `snap` | `Option<SnapControls>` | Phase 2 — morph the boundary onto the surface. `None` skips snapping. |
| `layers` | `Option<LayerControls>` | Phase 3 — insert graded boundary layers. `None` skips layer addition. |

##### Implementations

###### Methods

- ```rust
  pub fn castellation_only(castellation: CastellationControls) -> Self { /* ... */ }
  ```
  Castellation-only controls (snapping and layers disabled).

- ```rust
  pub fn with_snap(self: Self, snap: SnapControls) -> Self { /* ... */ }
  ```
  Enable snapping with the given controls (builder style).

- ```rust
  pub fn with_layers(self: Self, layers: LayerControls) -> Self { /* ... */ }
  ```
  Enable layer addition with the given controls (builder style).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `generate`

Run the full `snappyHexMesh` pipeline: castellation → (snapping) → (layers).

Executes the enabled phases in order, threading the [`CastellatedMesh`]
(which carries the point/face [`PolyPatchMesh`] topology and the validated
[`FvMesh`](outram_foam_basic_lib::mesh::FvMesh)) from one phase to the next,
and returns the final mesh. Phases whose controls are `None` are skipped.

This is the single top-level entry point corresponding to running the
`snappyHexMesh` utility; the individual phase functions ([`castellate`],
[`snap`], [`add_layers`]) remain public for finer control.

# Errors
Propagates the first phase error — [`MeshError::Construction`] from
castellation (empty surface, all cells removed, invalid assembly) or from a
snapping/layer rebuild.

# Example
```no_run
use outram_foam_mesh::snappy_hex_mesh::{
    background::{BackgroundMesh, Bounds},
    castellation::CastellationControls,
    generate, SnappyHexMeshControls, SnapControls, LayerControls,
    stl::read_stl,
};

let surface = read_stl("sphere.stl").unwrap();
let (lo, hi) = surface.bounding_box().unwrap();
let domain = Bounds::new(lo, hi).expanded(0.5);
let background = BackgroundMesh::uniform(domain, 10, 10, 10);
let controls = SnappyHexMeshControls::castellation_only(
    CastellationControls::new(background, 2, domain.min),
)
.with_snap(SnapControls::default())
.with_layers(LayerControls::default());

let mesh = generate(&surface, &controls).unwrap();
println!("final mesh has {} cells", mesh.n_cells());
```

```rust
pub fn generate(surface: &TriangleSoup, controls: &SnappyHexMeshControls) -> Result<CastellatedMesh, crate::MeshError> { /* ... */ }
```

### Re-exports

#### Re-export `BackgroundMesh`

```rust
pub use background::BackgroundMesh;
```

#### Re-export `Bounds`

```rust
pub use background::Bounds;
```

#### Re-export `castellate`

```rust
pub use castellation::castellate;
```

#### Re-export `CastellatedMesh`

```rust
pub use castellation::CastellatedMesh;
```

#### Re-export `CastellationControls`

```rust
pub use castellation::CastellationControls;
```

#### Re-export `CellRemoval`

```rust
pub use castellation::CellRemoval;
```

#### Re-export `RefinementSelection`

```rust
pub use castellation::RefinementSelection;
```

#### Re-export `SurfaceFace`

```rust
pub use castellation::SurfaceFace;
```

#### Re-export `SurfaceRegionPatches`

```rust
pub use castellation::SurfaceRegionPatches;
```

#### Re-export `check_conformity`

```rust
pub use cyclic::check_conformity;
```

#### Re-export `resolve_pairs`

```rust
pub use cyclic::resolve_pairs;
```

#### Re-export `CyclicError`

```rust
pub use cyclic::CyclicError;
```

#### Re-export `CyclicPair`

```rust
pub use cyclic::CyclicPair;
```

#### Re-export `CyclicPointConstraints`

```rust
pub use cyclic::CyclicPointConstraints;
```

#### Re-export `DEFAULT_CYCLIC_TOL`

```rust
pub use cyclic::DEFAULT_CYCLIC_TOL;
```

#### Re-export `add_layers`

```rust
pub use layers::add_layers;
```

#### Re-export `layer_thicknesses`

```rust
pub use layers::layer_thicknesses;
```

#### Re-export `total_layer_thickness`

```rust
pub use layers::total_layer_thickness;
```

#### Re-export `LayerControls`

```rust
pub use layers::LayerControls;
```

#### Re-export `face_area_and_centre`

```rust
pub use poly_topology::face_area_and_centre;
```

#### Re-export `MeshQuality`

```rust
pub use poly_topology::MeshQuality;
```

#### Re-export `PolyPatchMesh`

```rust
pub use poly_topology::PolyPatchMesh;
```

#### Re-export `QualityLimits`

```rust
pub use poly_topology::QualityLimits;
```

#### Re-export `raw_surface_displacements`

```rust
pub use snapping::raw_surface_displacements;
```

#### Re-export `snap`

```rust
pub use snapping::snap;
```

#### Re-export `SnapControls`

```rust
pub use snapping::SnapControls;
```

#### Re-export `read_stl`

```rust
pub use stl::read_stl;
```

#### Re-export `read_stl_ascii_str`

```rust
pub use stl::read_stl_ascii_str;
```

#### Re-export `read_stl_binary`

```rust
pub use stl::read_stl_binary;
```

#### Re-export `read_stl_bytes`

```rust
pub use stl::read_stl_bytes;
```

#### Re-export `Triangle`

```rust
pub use stl::Triangle;
```

#### Re-export `TriangleSoup`

```rust
pub use stl::TriangleSoup;
```

## Types

### Enum `MeshError`

Errors produced by the mesh utilities in this crate.

```rust
pub enum MeshError {
    DictParse(String),
    NotImplemented(String),
    Construction(String),
    Io(String),
}
```

#### Variants

##### `DictParse`

A dictionary / input file could not be parsed (bad syntax, missing key).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `NotImplemented`

The input asks for a feature this crate does not support yet: raised by
[`block_mesh`] for a non-`hex` block shape, a macro that is not expanded,
a searchable-surface type that is not ported, `mergePatchPairs`, ...
~~deferred dict features that are safe to ignore are skipped silently~~
**CORRECTED 2026-11-11 (gh:#1462)**: nothing is skipped; see the
[`block_mesh`] module docs.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `Construction`

A geometric / topological inconsistency was detected while building the mesh.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### `Io`

A mesh could not be written to (or read from) disk — see
[`GeneratedMesh::write_case`]. Carries the formatted underlying
`outram_foam_basic_lib::io::IoError` plus the path involved.

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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Re-exports

### Re-export `mesh_from_surface`

```rust
pub use driver::mesh_from_surface;
```

### Re-export `GeneratedMesh`

```rust
pub use driver::GeneratedMesh;
```

### Re-export `KeepRegion`

```rust
pub use driver::KeepRegion;
```

### Re-export `LayerOutcome`

```rust
pub use driver::LayerOutcome;
```

### Re-export `MeshingControls`

```rust
pub use driver::MeshingControls;
```

### Re-export `MeshingPhases`

```rust
pub use driver::MeshingPhases;
```

### Re-export `assess_quality`

```rust
pub use mesh_quality::assess_quality;
```

### Re-export `MeshQualityReport`

```rust
pub use mesh_quality::MeshQualityReport;
```

### Re-export `QualityVerdict`

```rust
pub use mesh_quality::QualityVerdict;
```

