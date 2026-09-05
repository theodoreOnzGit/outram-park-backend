# Crate Documentation

**Version:** 0.1.0

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
2. [`BlockMeshDict::build`] subdivides every block, merges globally coincident
   points, deduplicates faces (a face shared by two cells becomes internal),
   assigns the remaining boundary faces to patches, and returns a [`PolyMesh`].
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

- `edges` blocks (arc / spline / polyLine) are parsed and **skipped**: all
  block edges are treated as straight lines (bilinear/trilinear block map).
  Curved-edge point projection is a later phase.
- `simpleGrading (gx gy gz)` is fully supported, where each of the three
  directions is **either** a single scalar expansion ratio **or** a
  multi-grading list of `( fractionLength fractionCells expansionRatio )`
  segments (OpenFOAM `gradingDescriptors`), graded piecewise-geometrically.
- `edgeGrading (e0 … e11)` is **fully supported**: each of the hex's 12
  edges carries its own [`Grading`] (single ratio or multi-segment), and
  interior nodes are blended from all 12 edge distributions exactly as
  OpenFOAM's `block::createPoints` does (the straight-edge, no-curvature
  case). When the four edges of a direction happen to agree, the block
  collapses to the equivalent `simpleGrading` fast path (bit-identical to a
  single per-direction distribution); when they disagree, the genuine
  per-edge 12-edge blend is used. See `edge_blended_node`.
- `mergePatchPairs` (face-merging of separately-meshed patch pairs) is
  parsed and ignored; coincident-point merging across blocks is always done.

## Honest scope — what the per-edge blend does NOT model

Only the **straight-edge** branch of OpenFOAM `block::createPoints`
(`blockCreate.C:45-183`, `nCurvedEdges == 0`) is ported. The curved-edge
correction (`blockCreate.C:155-180`) and the curved-**face** projection
passes (`blockCreate.C:185-345`) are **not** implemented — consistent with
the deferred `arc`/`spline`/`polyLine` edge and `projectFace` support above.
For blocks with planar faces (all edges straight — the current scope) the
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
    fn clone(self: &Self) -> GradingSegment { /* ... */ }
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
    fn eq(self: &Self, other: &GradingSegment) -> bool { /* ... */ }
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
    fn clone(self: &Self) -> Grading { /* ... */ }
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
    fn eq(self: &Self, other: &Grading) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
for inspection. When it is `None`, `grading` is authoritative and the fast
trilinear per-direction map is used (this covers `simpleGrading` and every
`edgeGrading` whose four edges per direction agree).

```rust
pub struct Block {
    pub vertices: [usize; 8],
    pub cells: [usize; 3],
    pub grading: [Grading; 3],
    pub edge_grading: Option<[Grading; 12]>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vertices` | `[usize; 8]` | The 8 block-corner vertex indices, in OpenFOAM hex order. |
| `cells` | `[usize; 3]` | Cell counts `(nx, ny, nz)` along the three local directions. |
| `grading` | `[Grading; 3]` | Per-direction node distributions `(gx, gy, gz)`. Authoritative when<br>`edge_grading` is `None`; a per-direction representative otherwise. |
| `edge_grading` | `Option<[Grading; 12]>` | The 12 per-edge gradings, present only for a genuinely per-edge<br>`edgeGrading` (edges 0–3 = x, 4–7 = y, 8–11 = z; see<br>`edge_blended_node`). `None` selects the trilinear fast path. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Block { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> PatchDef { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `convert_to_meters` | `f64` | Scale factor `[m per dict unit]` applied to every vertex (`convertToMeters`). |
| `vertices` | `Vec<outram_foam_basic_lib::primitives::Vector3>` | The vertex list (raw, unscaled dict coordinates). |
| `blocks` | `Vec<Block>` | The `hex` blocks. |
| `patches` | `Vec<PatchDef>` | The named boundary patches. |

##### Implementations

###### Methods

- ```rust
  pub fn build(self: &Self) -> Result<PolyMesh, MeshError> { /* ... */ }
  ```
  Subdivide every block, merge coincident points, dedupe faces, assign

- ```rust
  pub fn parse(text: &str) -> Result<Self, MeshError> { /* ... */ }
  ```
  Parse `blockMeshDict` text into a [`BlockMeshDict`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> BlockMeshDict { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> MeshFace { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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

##### Implementations

###### Methods

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
    fn clone(self: &Self) -> PolyMesh { /* ... */ }
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
    fn clone(self: &Self) -> MeshingPhases { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &MeshingPhases) -> bool { /* ... */ }
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
    fn clone(self: &Self) -> KeepRegion { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &KeepRegion) -> bool { /* ... */ }
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

##### Implementations

###### Methods

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
    fn clone(self: &Self) -> MeshingControls { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> LayerOutcome { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &LayerOutcome) -> bool { /* ... */ }
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
    fn clone(self: &Self) -> GeneratedMesh { /* ... */ }
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
    fn clone(self: &Self) -> UnvPolyMesh { /* ... */ }
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
    fn clone(self: &Self) -> QualityVerdict { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &QualityVerdict) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> MeshQualityReport { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &MeshQualityReport) -> bool { /* ... */ }
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

## Feature angle

`feature_angle_deg` is the OpenFOAM-style feature angle. A boundary point is
kept as an explicit dual vertex (preserving a sharp edge or corner) when the
surface bends across it by **more than** the feature angle; otherwise the
surface is locally flat there and the point is *merged* away — its ring of
boundary quads collapses to a single coplanar polygon. On a flat patch this
removes the redundant vertex without changing the enclosed volume; at a sharp
feature (e.g. a 90° block edge) merging it away would cut the corner and lose
volume, which is exactly why the feature is preserved. See the crate tests
for the numerical demonstration of both regimes.

## Scope / honesty

- The interior construction (cell-centre rings around interior edges) is
  general — it works for any manifold polyhedral primal mesh.
- The boundary construction is implemented and verified for **manifold**
  surfaces (each boundary edge shared by exactly two boundary faces). It has
  been validated on structured hexahedral blocks; non-manifold surfaces and
  degenerate fans are rejected with [`MeshError::Construction`] rather than
  silently mis-meshed.
- Feature handling implements point retention/merge. It does **not** yet
  split coplanar boundary faces belonging to *different* patches that meet
  below the feature angle (not needed for the block case, where patch seams
  coincide with feature edges); this is the one documented boundary
  limitation.

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
    fn clone(self: &Self) -> PolyMesh { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> DualMesh { /* ... */ }
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

#### Function `poly_dual_mesh`

Construct the polyhedral dual of `primal`.

`feature_angle_deg` controls boundary-point retention (see the module docs):
a boundary point is kept as an explicit dual vertex when the surface bends
across it by more than this angle; otherwise it is merged into a coplanar
boundary polygon.

# Errors
Returns [`MeshError::Construction`] for a non-manifold boundary or an
inconsistent edge fan.

```rust
pub fn poly_dual_mesh(primal: &PolyMesh, feature_angle_deg: f64) -> Result<DualMesh, crate::MeshError> { /* ... */ }
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
    fn clone(self: &Self) -> Bounds { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Bounds) -> bool { /* ... */ }
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
    fn clone(self: &Self) -> BackgroundMesh { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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

##### Implementations

###### Methods

- ```rust
  pub fn new(background: BackgroundMesh, surface_level: usize, keep_point: Vector3) -> Self { /* ... */ }
  ```
  Convenience constructor with the common defaults (auto band, surface

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
    fn clone(self: &Self) -> CastellationControls { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> SurfaceFace { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> CastellatedMesh { /* ... */ }
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
    fn clone(self: &Self) -> CyclicError { /* ... */ }
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
    fn eq(self: &Self, other: &CyclicError) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> CyclicPair { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &CyclicPair) -> bool { /* ... */ }
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
    fn clone(self: &Self) -> CyclicPointConstraints { /* ... */ }
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
    fn default() -> CyclicPointConstraints { /* ... */ }
    ```

- **Freeze**
- **From**
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
    fn clone(self: &Self) -> LayerControls { /* ... */ }
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
    fn clone(self: &Self) -> PolyPatchMesh { /* ... */ }
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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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
    fn clone(self: &Self) -> MeshQuality { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &MeshQuality) -> bool { /* ... */ }
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
    fn clone(self: &Self) -> QualityLimits { /* ... */ }
    ```

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

#### Function `face_area_and_centre`

Area vector `[m²]` and centre `[m]` of one polygonal face, by fan triangulation
about the point average (OpenFOAM `makeFaceCentresAndAreas`).

The returned area vector obeys the right-hand rule over the point order; a
triangle is handled directly for exactness. Units: `points` in metres, area
in `m²`, centre in metres.

```rust
pub fn face_area_and_centre(face: &[usize], points: &[outram_foam_basic_lib::primitives::Vector3]) -> (outram_foam_basic_lib::primitives::Vector3, outram_foam_basic_lib::primitives::Vector3) { /* ... */ }
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
    fn clone(self: &Self) -> SnapControls { /* ... */ }
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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TriangleSoup { /* ... */ }
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
    fn default() -> TriangleSoup { /* ... */ }
    ```

- **Freeze**
- **From**
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
    fn clone(self: &Self) -> SnappyHexMeshControls { /* ... */ }
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

#### Re-export `SurfaceFace`

```rust
pub use castellation::SurfaceFace;
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

The input asks for a feature this crate does not support yet. Currently
raised only by [`block_mesh`] for a non-`hex` block shape; deferred dict
features that are safe to ignore (curved `edges`, `mergePatchPairs`) are
skipped silently instead — see the [`block_mesh`] module docs.

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
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
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

