# Crate Documentation

**Version:** 0.0.1

**Format Version:** 60

# Module `outram_park_fork_cfmesh`

# outram-park-fork-cfmesh

A pure-Rust **fork / port of [cfMesh](https://github.com/wyldckat/cfMesh)**
(Creative Fields' automatic unstructured mesh generator, **GPL-3.0**) for the
OUTRAM PARK multiphysics suite — the *volume-mesh-generation* layer that sits
between the [`outram-blender`](https://crates.io/crates/outram-blender)
surface-authoring frontend and the OpenFOAM-style solvers.

```text
  outram-blender  ──►  outram-park-fork-cfmesh  ──►  outram-foam-basic-lib
  (surface Mesh)       (tet / polyhedral cells        (PolyMesh → FvMesh,
                        + boundary layers)              solvable)
```

## Why this crate exists

The workspace has the mesh **representation** and finite-volume addressing
(`outram-foam-basic-lib`'s `FvMesh` / `io::poly_mesh::PolyMesh`, with
`read`/`write`/`to_fv_mesh`), and the surface-**authoring** frontend
(`outram-blender`). This crate supplies the **cfMesh-lineage** automatic
unstructured generator between them — turning a closed `outram-blender`
surface into a solvable *volume* mesh with polyhedral cells and near-wall
boundary layers in one call. Open-source, pure-Rust, GPLv3-clean tooling for
that is genuinely lacking, so this crate ports the proven cfMesh workflows
rather than reinventing them.

## Relationship to `outram-foam-mesh` (they overlap)

`outram-foam-mesh` is a **separate, earlier** crate porting the
**OpenFOAM-lineage** utilities: `blockMesh`, `ideasUnvToFoam`,
`snappyHexMesh` castellation, the **cell-centre** `polyDualMesh`, and
`checkMesh`-style quality assessment. Both crates turn a surface into a
volume mesh, both build a dual and both insert layers, so the overlap is
real. The decisive differences: that crate mirrors the individual OpenFOAM
binaries and builds the **cell-centre** dual, whereas this one exposes a
single composed [`pipeline::surface_to_tet_dual_mesh`] over an
`outram-blender` surface and builds the **median (vertex-centred)** dual.
The two duals are different algorithms and **their V&V results are not
interchangeable** — see the [`dual`] module docs.

## Goal

Consume an `outram-blender` [`Mesh`](https://docs.rs/outram-blender) (a closed
watertight surface), generate a **tetrahedral** then **polyhedral** volume
mesh (à la cfMesh `tetMesh` / `cartesianMesh` + OpenFOAM `polyDualMesh`), with
optional **wall boundary/prism layers**, and emit a real-cell
`outram_foam_basic_lib::io::poly_mesh::PolyMesh` for the CFD/TH solvers and
`outram-mc-libs` geometry for neutronics — the mesh substrate for coupled
pebble-bed / molten-salt / light-water reactor simulations.

## Vendored upstreams (reference only, GPLv3-clean, never shipped)

Both live under `upstream_source/` (gitignored, dev-only — see
`upstream_source/README.md`):

- **cfMesh** — <https://github.com/wyldckat/cfMesh>, **GPL-3.0-only**. Primary
  port target: `meshLibrary/{cartesianMesh, tetMesh, utilities}` (Cartesian
  hex-dominant + tet meshing, surface tools, boundary-layer insertion).
- **voro++** — <https://github.com/chr1shr/voro>, modified-BSD (LBNL),
  GPLv3-compatible. Reference for the Voronoi / polyhedral-dual construction.

## Provenance of the source files

**Every `src/*.rs` file carries an SPDX identifier and a provenance header
block.** Because this crate re-implements published *algorithms* rather than
transcribing C++, the block names the upstream project, the source directory
or file the construction follows, the copyright holder, and the licence —
rather than a per-file upstream commit, which would imply a line-level
correspondence that does not exist. Files that are original OUTRAM PARK work
with no upstream ancestor ([`math`], [`shapes`], [`reactor`], [`pipeline`],
[`patches`]) say so explicitly in the same block.

Non-cfMesh algorithmic sources are credited where they are used, with the
literature citation rather than an implied code lineage. In particular
[`delaunay::orient3d`] / [`delaunay::insphere`] are **Shewchuk's
predicates** (J. R. Shewchuk, *Adaptive Precision Floating-Point Arithmetic
and Fast Robust Geometric Predicates*, Discrete & Computational Geometry
18(3):305-363, 1997) — note that only the determinant *formulations* are
taken: the adaptive exact-arithmetic expansions that make Shewchuk's
`predicates.c` robust are **not** implemented here, so these are fast `f64`
evaluations and are not a drop-in substitute for the real thing on
degenerate input.

## Status

- **Milestone 1 — volume-mesh core + Cartesian block mesher.**
  [`volume_mesh::VolumeMesh`] (points + faces + owner/neighbour + patches,
  mirroring cfMesh `polyMeshGen` / OpenFOAM `polyMesh`) and
  [`cartesian::cartesian_box`] (a regular hex grid of an axis-aligned box).
- **Milestone 2 — castellated surface carve.** [`carve::carve_box`] overlays
  a uniform Cartesian grid on a closed triangle-soup surface and keeps the
  cells inside it (ray-parity inside test), producing a body-fitted
  *staircase* volume mesh with a `walls` boundary patch.
  [`carve::carve_region`] extends this to the region *inside* an outer
  surface but *outside* inner holes — the shell/annular pattern reactor
  geometry needs (coolant around fuel pins or pebbles).
- **Milestone 3a — boundary snapping.** [`snap::snap_to_surface`] projects
  every boundary point onto the closest point of the surface, turning the
  staircase into a body-fitted boundary.
- **Milestone 3b — foam bridge (feature `foam-export`).** [`foam::to_poly_mesh`]
  converts a [`volume_mesh::VolumeMesh`] into a real `outram-foam-basic-lib`
  `PolyMesh`, which yields a solvable `FvMesh` via `to_fv_mesh()` — closing
  the loop: surface → carve → snap → foam mesh (verified end-to-end).
- **Mesh quality checks.** [`checks::check_quality`] reports face
  non-orthogonality, skewness, cell aspect ratio, min face area / cell
  volume, and negative-volume cells (cfMesh `polyMeshGenChecks`) — the gate
  for trusting a generated mesh before it is solved.
- **Octree near-wall refinement.** [`octree::refine_near_boundary`] grades
  the mesh finer next to the surface, splitting each coarse transition
  face into its four fine sub-faces (hanging nodes) so the coarse cell
  becomes a genuine **polyhedron** — the mesh stays conforming (verified:
  exact volume, closed cells, > 6-face transition cells). Every emitted face
  ring additionally has the hanging nodes on its edges spliced in, so each
  edge lies in exactly two faces of every cell — the **edge-manifold**
  property [`tet::tetrahedralize`] needs to stay watertight on a graded mesh
  (verified). [`octree::refine_near_boundary_banded`] is the size-field
  form used by the pipeline: refine where the cell centre is within a
  distance band of the surface, the band measured in the cell's own edge
  lengths.
- **Prism boundary layers.** [`layers::add_boundary_layers`] inserts graded
  near-wall inflation layers at a wall patch (snappyHexMesh *addLayers* /
  cfMesh's boundary-layer step): the interior wall points move inward and
  the vacated shell is filled with `n_layers` stacked prism cells per wall
  face. It is a repartition, so it preserves the mesh volume exactly
  (verified: exact volume, closed, `+n_layers × wall_faces` cells). Flat
  (box) walls only — the fixed-thickness march self-intersects on curvature.
  [`layers::add_boundary_layers_adaptive`] handles **curved / polyhedral
  walls** (smoothed normals + per-point thickness limiting + validity
  back-off), verified valid + volume-conserving on snapped spheres/cylinders.
- **Polyhedral dual.** [`dual::polyhedral_dual`] turns a primal mesh into a
  **polyhedral** one — one cell per primal vertex — via the **median**
  (Donald / vertex-centred) dual. It fills the same role as OpenFOAM's
  `polyDualMesh` but is **not the same construction**: `polyDualMesh` (and
  `outram-foam-mesh`'s `poly_dual_mesh`) is the *cell-centre* dual, whereas
  this places dual corners at edge midpoints and face/cell centroids, and it
  is not the circumcentre Voronoi dual either. The dual tiles the same
  region (verified: exact volume, closed cells, every internal face shared
  by exactly two cells, genuinely > 6-face cells). Two variants:
  [`dual::polyhedral_dual`] (robust median, quad-fan faces) and
  [`dual::polyhedral_dual_min_faces`] (**face-minimal** — one polygon per
  primal edge via edge-star walking, ~40% fewer faces, verified to conserve
  volume and stay closed). **Not verified:** no test in [`dual`] measures
  non-orthogonality or skewness, so this dual's orthogonality is
  **unmeasured** — and `outram-foam-mesh`'s "dualisation does not create
  non-orthogonality" result belongs to its *cell-centre* dual and does not
  carry over. See the [`dual`] module docs.
- **Tetrahedralization.** [`tet::tetrahedralize`] splits every cell into
  tetrahedra by centroid subdivision (the all-tet foundation; not a
  from-scratch Delaunay mesher). It conserves volume and triangulates the
  boundary surface (verified: positive-volume tets, boundary area == input
  surface, exact volume, every cell a 4-triangle tet). **Not verified:** no
  test gates any *shape*-quality metric, so the tet primal's quality is
  **unmeasured**; [`delaunay`] records qualitatively that the subdivision
  leaves slivers, making this the *suspected* — not demonstrated — dominant
  non-orthogonality source downstream. See the [`tet`] module docs.
- **Quality smoothing.** [`smooth::laplacian_smooth`] relaxes interior
  vertices toward their neighbour centroid (smart Laplacian — never inverts a
  cell, pins the boundary), improving cell shape while conserving volume
  exactly (verified: recovers perturbed tet quality, no inversions).
- **Flip-based Delaunay.** [`delaunay::flip_to_delaunay`] improves a tet mesh
  toward Delaunay by bistellar 2→3 / 3→2 flips (Shewchuk [`delaunay::orient3d`]
  / [`delaunay::insphere`] predicates). Volume- and boundary-preserving, with
  an *improve-or-noop* guard so it can never make a mesh worse and refuses
  tangled input (verified: fixes a non-Delaunay bipyramid exactly, conserves
  volume + bounded growth on a block, returns invalid input unchanged).
- **High-level pipeline (the recommended entry point).**
  [`pipeline::surface_to_tet_dual_mesh`] composes the stages above into one
  call: `carve → snap → tetrahedralize → Delaunay-improve → polyhedral dual
  → smooth → adaptive prism boundary layers`, driven by
  [`pipeline::TetDualOptions`] and returning a [`pipeline::TetDualReport`]
  (cell count, volume, quality, and a note for every stage skipped). Each
  optional stage is applied only if its result stays valid (closed + no
  inverted cells), otherwise it is **gracefully skipped** and the previous
  mesh kept, so the returned mesh is always valid and exportable. This is the
  coarse-grained meshing entry point the `outram-blender` "mesh studio" GUI
  calls; the [`pipeline::box_tet_dual`] / [`pipeline::sphere_tet_dual`] /
  [`pipeline::cylinder_tet_dual`] wrappers mesh the built-in primitives.
  Lengths in metres, layer thickness in metres, angles in degrees.

  Stage 1 can be **octree-graded** rather than uniform via
  [`pipeline::TetDualOptions::refinement_levels`] (default `0` = the uniform
  carve, unchanged): the interior stays at `cell_size` and the near-wall band
  is refined, so a given wall resolution costs far fewer cells. Measured on a
  radius-3 m sphere: **1381 cells at 1.66 % volume error** graded, versus
  **5269 cells at 1.72 %** for the uniform carve resolving the same 0.5 m
  wall; the widest measured gap is 3921 vs 101376 cells at ~1.07 % error.
  **Only that first pair is run by a test** — and even there the assertions
  are relative (graded no less accurate, graded uses < half the cells, both
  meshes closed and under 90 deg non-orthogonality), not the tabulated
  numbers; the widest-gap pair and the whole skewness column are
  **doc-recorded only**, measured by hand on 2026-08-07. See [`pipeline`]'s
  tests for the full table, its per-row gating column, and its caveats.
- **Named boundary patches (`inlet` / `outlet` / `walls`).**
  [`pipeline::surface_to_tet_dual_mesh_multipatch`] carries an input
  surface's named regions ([`patches::SurfaceRegions`]) through to the output
  mesh's [`volume_mesh::BoundaryPatch`] list, so a solver `0/`
  boundary-condition directory can be written against the result. The stages
  in between rebuild the mesh through
  [`volume_mesh::from_cell_faces`] and cannot preserve a tag, so the
  assignment is recovered geometrically at the end by
  [`patches::assign_patches_by_region`] — each boundary face takes the region
  of the nearest input triangle, as snappyHexMesh does (verified: contiguous
  patches with correct counts and start offsets, geometrically correct
  membership, through the full pipeline including prism layers). The
  single-patch [`pipeline::surface_to_tet_dual_mesh`] is unchanged.

Next on the `op-hzs` roadmap: exact/adaptive predicates + size-driven point
insertion (the rest of `op-38z`; gmsh — GPLv2+, GPLv3-compatible — is a
licence-clean reference); **patch-selective** layer insertion (layers are
currently grown over the whole boundary, inlet/outlet included, because patch
classification runs last); feature-edge-aware snapping and patch seams; and
making the adaptive layerer work on graded near-wall meshes, where it
currently backs off to zero thickness (the pipeline now reports that in
[`pipeline::TetDualReport::stage_notes`] instead of failing silently).

## Design rules (workspace `CLAUDE.md`)

Index-based topology (newtype indices into `Vec`, no lifetimes/pointers);
enums for dispatch, never trait objects; no `Box<T>` (own by value or share
with `Arc<T>`); pure Rust with no BLAS/C/Fortran so the crate builds on
Android/Termux.

> **Not affiliated with Creative Fields or the cfMesh project**, and not the
> official cfMesh software — an independent GPL fork. **Untrusted
> AI-assisted draft** until human-reviewed, per the workspace
> `RESPONSIBLE_USE.md`. For education / research / V&V only; not for reactor
> operation, licensing, or safety-critical decisions.

## Modules

## Module `cartesian`

Structured **Cartesian block mesher** — an axis-aligned box filled with a
regular grid of hexahedral cells.

This is the foundation cfMesh's `cartesianMesh` builds on: its generator
starts from a background Cartesian octree over the bounding box and then
refines/carves it to a surface. This module builds the un-refined,
un-carved base case — a full `nx × ny × nz` hex grid of a box — which is a
complete, valid [`VolumeMesh`] in its own right (the analogue of OpenFOAM's
`blockMesh` for a single block) and the milestone-1 output that proves the
[`VolumeMesh`] data structure and its conventions end to end.

Later milestones add the octree refinement and surface intersection that
turn this background grid into a body-fitted mesh, plus boundary layers.

```rust
pub mod cartesian { /* ... */ }
```

### Functions

#### Function `cartesian_box`

Mesh the axis-aligned box `[min, max]` into a regular `nx × ny × nz` grid of
hexahedral cells.

The result has `nx·ny·nz` cells, internal faces first (owner < neighbour,
normal pointing owner→neighbour), then six boundary patches
`xMin, xMax, yMin, yMax, zMin, zMax` (outward normals). Its
[`VolumeMesh::total_volume`] equals the box volume and every cell passes the
closure check in [`VolumeMesh::validate`].

# Panics

If any division count is zero, or `max <= min` on any axis.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, cartesian::cartesian_box};

// Unit box as a 2×2×2 grid: 8 cells, volume 1.
let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
assert_eq!(m.cell_count(), 8);
assert!((m.total_volume() - 1.0).abs() < 1e-12);
assert!(m.validate().is_ok());
```

```rust
pub fn cartesian_box(min: crate::math::Vec3, max: crate::math::Vec3, divisions: [usize; 3]) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `carve`

Castellated Cartesian **carve** — body-fit a closed surface (or a region
between surfaces) into a volume mesh by keeping the background-grid cells
that lie inside it.

This is the core of cfMesh's `cartesianMesh` (and snappyHexMesh's
castellation): overlay a uniform Cartesian grid on the surface's bounding
box, decide which cells are *inside* the region, and keep them. The kept
cells form a [`VolumeMesh`] whose boundary is the "staircase" approximation
of the input surface(s) (run [`crate::snap::snap_to_surface`] to body-fit it).

Two entry points:
- [`carve_box`] — the interior of a single closed surface.
- [`carve_region`] — the interior of an *outer* surface minus one or more
  inner *hole* surfaces (the shell/annular pattern: coolant around fuel,
  a vessel minus its internals).

# v1 scope

- **Staircase boundary** — cells are kept whole; snap the result to
  body-fit. - **Inside test** — ray parity (Möller–Trumbore) from each cell
  centre; correct for a **closed, watertight** triangle soup. All boundary
  faces land in a single `walls` patch (per-surface patch separation is a
  later refinement). Pure Rust, Android-safe.

```rust
pub mod carve { /* ... */ }
```

### Functions

#### Function `carve_box`

Carve the closed surface (`points`, triangle indices `tris`) into a
[`VolumeMesh`] of uniform `cell_size` hexahedra: every cell whose centre is
inside the surface is kept.

Returns an **empty** mesh if `cell_size <= 0`, there are fewer than four
points, or no cell centre lands inside.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, carve::carve_box};

// An axis-aligned box surface [0,2]³ carved at cell size 0.5 recovers the
// box exactly: 4³ = 64 cells, volume 8.
let (pts, tris) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
let m = carve_box(&pts, &tris, 0.5);
assert_eq!(m.cell_count(), 64);
assert!((m.total_volume() - 8.0).abs() < 1e-9);
# fn box_surface(a: Vec3, b: Vec3) -> (Vec<Vec3>, Vec<[usize; 3]>) {
#     let v = vec![
#         Vec3::new(a.x, a.y, a.z), Vec3::new(b.x, a.y, a.z), Vec3::new(b.x, b.y, a.z), Vec3::new(a.x, b.y, a.z),
#         Vec3::new(a.x, a.y, b.z), Vec3::new(b.x, a.y, b.z), Vec3::new(b.x, b.y, b.z), Vec3::new(a.x, b.y, b.z)];
#     let q = |a:usize,b:usize,c:usize,d:usize| vec![[a,b,c],[a,c,d]];
#     let mut t = Vec::new();
#     for f in [q(0,3,2,1), q(4,5,6,7), q(0,1,5,4), q(2,3,7,6), q(1,2,6,5), q(0,4,7,3)] { t.extend(f); }
#     (v, t)
# }
```

```rust
pub fn carve_box(points: &[crate::math::Vec3], tris: &[[usize; 3]], cell_size: f64) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

#### Function `carve_region`

Carve the region **inside** `outer` (`outer_points`, `outer_tris`) but
**outside** every surface in `holes`, into a [`VolumeMesh`] of uniform
`cell_size` hexahedra.

Each hole is a `(points, tris)` closed surface. A cell is kept iff its centre
is inside `outer` and inside *no* hole — the shell/annular pattern (coolant
around fuel pins or pebbles, a vessel minus its internals). All exposed faces
(outer *and* hole boundaries) land in one `walls` patch in v1.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, carve::carve_region};

// A hollow shell: box [0,4]³ minus box [1,3]³, at cell size 1 (grid-aligned).
let (op, ot) = box_surface(Vec3::ZERO, Vec3::new(4.0, 4.0, 4.0));
let (ip, it) = box_surface(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0));
let m = carve_region(&op, &ot, &[(&ip[..], &it[..])], 1.0);
assert!((m.total_volume() - (64.0 - 8.0)).abs() < 1e-9); // 56
# fn box_surface(a: Vec3, b: Vec3) -> (Vec<Vec3>, Vec<[usize; 3]>) {
#     let v = vec![
#         Vec3::new(a.x, a.y, a.z), Vec3::new(b.x, a.y, a.z), Vec3::new(b.x, b.y, a.z), Vec3::new(a.x, b.y, a.z),
#         Vec3::new(a.x, a.y, b.z), Vec3::new(b.x, a.y, b.z), Vec3::new(b.x, b.y, b.z), Vec3::new(a.x, b.y, b.z)];
#     let q = |a:usize,b:usize,c:usize,d:usize| vec![[a,b,c],[a,c,d]];
#     let mut t = Vec::new();
#     for f in [q(0,3,2,1), q(4,5,6,7), q(0,1,5,4), q(2,3,7,6), q(1,2,6,5), q(0,4,7,3)] { t.extend(f); }
#     (v, t)
# }
```

```rust
pub fn carve_region(outer_points: &[crate::math::Vec3], outer_tris: &[[usize; 3]], holes: &[(&[crate::math::Vec3], &[[usize; 3]])], cell_size: f64) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

#### Function `carve_around`

Ergonomic wrapper over [`carve_region`] taking **owned** triangle-soup
surfaces: carve the coolant region inside `domain` but outside every surface
in `holes`. Equivalent to `carve_region`, without the caller having to
re-borrow each soup as slices.

`domain` and each `hole` are `(points, triangles)` pairs (e.g. from
[`crate::shapes`] or [`crate::reactor`]).

```rust
pub fn carve_around(domain: &(Vec<crate::math::Vec3>, Vec<[usize; 3]>), holes: &[(Vec<crate::math::Vec3>, Vec<[usize; 3]>)], cell_size: f64) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `checks`

Mesh **quality checks** — the Rust analogue of cfMesh's `polyMeshGenChecks`.

A generated volume mesh must be *checked* before it is handed to a solver:
finite-volume discretisation error grows with face **non-orthogonality** and
**skewness**, ill-conditioning grows with cell **aspect ratio**, and a
**negative-volume** cell makes a case unsolvable. [`check_quality`] computes
these over a [`VolumeMesh`] and returns a [`QualityReport`].

Two of the metrics are specifically **sliver detectors**, because the angle
metrics above are blind to slivers — a cell can be arbitrarily flat while
every one of its owner→neighbour lines stays perfectly orthogonal to the
face it crosses:

- [`QualityReport::min_face_pyramid_volume`] catches a cell centre that has
  fallen onto or through one of its own faces (local inversion / tangling);
- [`QualityReport::min_cell_determinant`] catches flatness itself — the face
  normals collapsing towards a plane or a line — before the cell volume ever
  goes negative.

This matters here because this crate's tet primal is a **centroid
subdivision, not a Delaunay triangulation** (see [`crate::delaunay`]), so
slivers are an expected failure mode of the mesher rather than a rare one.

All geometry is the standard OpenFOAM pyramid decomposition — per-cell
volumes and centroids from the faces — so the metrics match what the solver
itself would compute. Pure Rust, no dependencies.

```rust
pub mod checks { /* ... */ }
```

### Types

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
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_non_orthogonality_deg` | `f64` | Maximum face **non-orthogonality**, in degrees: the angle between an<br>internal face's normal and the vector joining its two cell centres. `0°`<br>is perfectly orthogonal; OpenFOAM flags `> 70°`. |
| `max_skewness` | `f64` | Maximum face **skewness**: the offset of the face centre from where the<br>owner–neighbour centre line crosses the face, as a fraction of that<br>line's length. OpenFOAM flags `> 4`. |
| `max_aspect_ratio` | `f64` | Maximum cell **aspect ratio** (`Σ|Sf| / (6 · V^{2/3})`, which is `1` for<br>a cube). Large values mean sliver cells. |
| `min_face_area` | `f64` | Smallest face area in the mesh. |
| `min_cell_volume` | `f64` | Smallest (signed) cell volume in the mesh. |
| `n_negative_volume_cells` | `usize` | Number of cells with non-positive volume (a broken mesh has `> 0`). |
| `min_face_pyramid_volume` | `f64` | Smallest **face pyramid volume** in the mesh (a volume, so `m³` if the<br>points are in metres), minimised over every *(cell, face)* incidence —<br>an internal face is checked twice, once from each side.<br><br>For face `f` of cell `c`, the pyramid has the face as its base and the<br>**cell centre** as its apex, so its signed volume is<br>`V_pyr = (1/3) · (c_f − C_c) · S_f`, with `c_f` the face centroid and<br>`S_f` the face area vector taken **outward from `c`**.<br><br>Valid range: strictly `> 0` for every face of a well-formed cell — a<br>cube of side `h` gives `h³/6` on all six faces, and the pyramid volumes<br>of a cell sum exactly to its volume. A value `<= 0` means the cell<br>centre lies **on or outside the plane of one of its own faces**: the<br>cell is locally inverted or tangled, the face's flux stencil points the<br>wrong way, and OpenFOAM's `checkMesh` fails such a mesh outright.<br><br>This is not redundant with [`QualityReport::min_cell_volume`]: a<br>strongly concave cell can have a healthily positive *total* volume and<br>still have its centre outside one of its faces (verified in this<br>module's `concave_cell_has_negative_pyramid_but_positive_volume` test).<br>Conversely, a merely **flat** (sliver) cell keeps *positive* pyramid<br>volumes — they just become small — so flatness is<br>[`QualityReport::min_cell_determinant`]'s job, not this metric's.<br><br>`0.0` for a mesh with no faces. |
| `n_negative_pyramid_faces` | `usize` | Number of *(cell, face)* incidences whose pyramid volume is `<= 0`<br>(see [`QualityReport::min_face_pyramid_volume`]). `0` for a valid mesh;<br>any non-zero count is a hard failure in OpenFOAM's `checkMesh`. An<br>internal face can contribute up to `2` (once per adjacent cell).<br><br>Deliberately **not** wired into [`QualityReport::is_solvable`], whose<br>thresholds are left unchanged; test it explicitly if you want<br>`checkMesh`'s stricter gate. |
| `min_cell_determinant` | `f64` | Smallest **cell determinant** over the mesh (dimensionless) — the<br>sliver detector.<br><br># Definition implemented here<br><br>For cell `c` with faces `f`, area vectors `S_f` and unit normals<br>`n_f = S_f / |S_f|`, form the area-weighted normal-orientation tensor<br><br>```text<br>        Σ_f |S_f| (n_f ⊗ n_f)<br>  D_c = ─────────────────────<br>             Σ_f |S_f|<br>```<br><br>and report `27 · det(D_c)`, minimised over the cells. (Sign-free: `n_f`<br>enters only through an outer product, so the face winding is<br>irrelevant.)<br><br># Valid range and interpretation<br><br>`D_c` is symmetric positive semi-definite with `tr(D_c) = 1`, so by<br>AM–GM `det(D_c) <= (1/3)³` and the reported value lies in `[0, 1]`:<br><br>- `1.0` — the face normals are **isotropic** (`D_c = I/3`). An<br>  axis-aligned cube and a regular tetrahedron both give exactly `1`;<br>  both are verified in this module's tests.<br>- `→ 0` — the normals collapse towards a plane (flat sliver) or a line<br>  (needle), i.e. the cell is degenerate in at least one direction. The<br>  tensor loses rank *long before* the cell volume changes sign, which is<br>  what makes this the metric that sees slivers when non-orthogonality,<br>  skewness and `min_cell_volume` all read healthy.<br><br>A cell whose faces all have zero area contributes `0.0`; a mesh with no<br>cells reports `0.0`.<br><br># Parity caveat — read before quoting this against OpenFOAM<br><br>This is the same *form* and normalisation as the "cell determinant"<br>(`wellposedness`) figure OpenFOAM's `checkMesh` prints — a normalised<br>determinant of the summed face-normal outer products, scaled so a cube<br>reads `1` — but it was implemented from that description and from the<br>algebra above, **not** transcribed from OpenFOAM source, and it has<br>**not** been compared numerically against a `checkMesh` run on the same<br>mesh. Treat it as a documented equivalent with the properties proved and<br>tested here, **not** as verified `checkMesh` parity. No threshold from<br>`checkMesh` is reproduced here, and [`QualityReport::is_solvable`] does<br>not gate on it. |

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
    fn clone(self: &Self) -> QualityReport { /* ... */ }
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
    fn eq(self: &Self, other: &QualityReport) -> bool { /* ... */ }
    ```

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
### Functions

#### Function `check_quality`

Compute the [`QualityReport`] for `mesh`.

```rust
pub fn check_quality(mesh: &crate::volume_mesh::VolumeMesh) -> QualityReport { /* ... */ }
```

## Module `delaunay`

**Flip-based Delaunay** improvement of a tetrahedral mesh.

The centroid-subdivision [`crate::tet::tetrahedralize`] produces a valid,
space-filling tet mesh, but not a *Delaunay* one — some interior faces fail
the empty-circumsphere (locally-Delaunay) test, which is what leaves slivers.
This module improves the mesh toward Delaunay by **bistellar flips** (Lawson
flips): local retriangulations that keep the same vertices and the same
filled region but swap the connectivity of a small cluster of tets.

# The two flips

- **2→3 flip.** Two tets sharing a triangular face (a convex bipyramid over
  5 vertices) become three tets sharing the opposite edge. Used when the
  shared face is not locally Delaunay and the bipyramid is convex.
- **3→2 flip.** The reverse: three tets sharing an edge become two sharing a
  face. Used to remove a non-Delaunay interior edge.

A flip is applied only when the shared face is **not locally Delaunay** (the
opposite apex lies inside the circumsphere) **and** every resulting tet has
strictly positive volume — the validity guard that guarantees the mesh stays
a valid tiling (no inverted or overlapping tets). The driver sweeps until no
improving flip remains or an iteration cap is hit.

# Predicates — attribution

[`orient3d`] (signed volume) and [`insphere`] (in-circumsphere) are the two
geometric predicates of **Jonathan Richard Shewchuk**, *Adaptive Precision
Floating-Point Arithmetic and Fast Robust Geometric Predicates*, Discrete &
Computational Geometry 18(3):305-363, 1997, and the accompanying
`predicates.c` (Carnegie Mellon University,
<https://www.cs.cmu.edu/~quake/robust.html>, released by the author into the
public domain).

**What is and is not taken from that work.** Only the *determinant
formulations* are — the `(b-a)·((c-a)×(d-a))` orientation determinant and the
lifted 4x4 in-sphere determinant. Shewchuk's actual contribution, the
adaptive multi-stage exact-arithmetic expansions that make those determinants
*robust*, is **not implemented here**: these are plain `f64` evaluations.
They are therefore fast and adequate for the well-conditioned meshes this
crate generates, but they are **not** a substitute for `predicates.c` and
must not be described as robust predicates. Exact/adaptive arithmetic is
future work; until then, near-degenerate configurations are simply left
un-flipped rather than flipped wrongly, so the result is always a valid mesh.

# Guarantees & scope

Volume is conserved (flips retriangulate the same region), the boundary is
untouched (only interior faces/edges flip), and every intermediate mesh is
valid. Pure flipping reaches a **local** Delaunay optimum, not necessarily
the global Delaunay triangulation (that needs point insertion too, per
`op-38z`). Pure Rust, Android-safe.

```rust
pub mod delaunay { /* ... */ }
```

### Functions

#### Function `orient3d`

Signed volume × 6 of the tetrahedron `(a, b, c, d)`: `(b−a)·((c−a)×(d−a))`.
Strictly positive iff the tet is **positively oriented** (`d` on the
positive side of the plane `a, b, c`).

Units: cubic metres × 6, for inputs in metres. The sign is the meaningful
output; the magnitude is only used here as a positive-volume test.

**Attribution and accuracy.** This is Shewchuk's `orient3d` determinant
(J. R. Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast
Robust Geometric Predicates*, Discrete & Computational Geometry
18(3):305-363, 1997), evaluated in **plain `f64`** — *without* his adaptive
exact-arithmetic expansions. Near-degenerate (nearly coplanar) input can
therefore return the wrong sign. Callers in this crate treat a near-zero
result as "do not flip", which keeps the mesh valid; do not rely on this
function as a robust predicate. See the module docs.

```rust
pub fn orient3d(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3) -> f64 { /* ... */ }
```

#### Function `insphere`

In-sphere predicate. When `(a, b, c, d)` is **positively oriented**
(`orient3d(a,b,c,d) > 0`), returns a value `> 0` iff `e` lies strictly
**inside** the circumsphere of `a, b, c, d`, `< 0` if strictly outside, and
`0` if cospherical. Positions in metres; only the sign is meaningful.

**Attribution and accuracy.** This is Shewchuk's `insphere` — the standard
lifted 4x4 determinant with rows `(p - e, |p - e|^2)` (same reference as
[`orient3d`]) — evaluated in **plain `f64`**, *without* his adaptive
exact-arithmetic expansions, so it can return the wrong sign on
near-cospherical input. Callers treat a near-zero result as "not worth
flipping". See the module docs.

```rust
pub fn insphere(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3, e: crate::math::Vec3) -> f64 { /* ... */ }
```

#### Function `flip_to_delaunay`

Improve `mesh` toward Delaunay by bistellar (2→3 / 3→2) flips, returning the
flipped mesh. `mesh`'s cells must all be tetrahedra (e.g. from
[`crate::tet::tetrahedralize`]); if not, `mesh` is returned unchanged.

Volume and the boundary are preserved; every intermediate mesh is valid.
Flipping reaches a *local* Delaunay optimum (see the module docs).

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, tet::tetrahedralize, delaunay::flip_to_delaunay};

let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
let tets = tetrahedralize(&carve_box(&p, &t, 0.5));
let flipped = flip_to_delaunay(&tets, 10_000);

// Flips retriangulate the same region: volume conserved, still valid.
assert!((flipped.total_volume() - tets.total_volume()).abs() < 1e-9);
assert!(flipped.validate().is_ok());
```

```rust
pub fn flip_to_delaunay(mesh: &crate::volume_mesh::VolumeMesh, max_flips: usize) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `dual`

The **polyhedral dual** mesh — one polyhedral cell per primal *vertex*. It
fills the same *role* as OpenFOAM's `polyDualMesh` (voro++ is the reference
for the Voronoi/median-dual idea), but it is **not the same construction** —
see "What this dual is, and what it is not" below before transferring any
result from a cell-centre dual to this one.

A finite-volume solver is usually happier on a **polyhedral** mesh than on a
hex/tet mesh of the same region: polyhedra pack more neighbours per cell
(better gradient reconstruction) at a much lower cell count. The classic way
to get one is to take the *dual* of a primal mesh — turn every primal
**vertex** into a cell.

# What this dual is, and what it is not

This is the **median** (a.k.a. Donald / vertex-centred) dual. Stated
plainly, because the distinction is routinely lost:

- **It is not the circumcentre Voronoi dual.** Its dual-cell corners are
  edge *midpoints* and face/cell *centroids*, never circumcentres. The
  payoff is that it is well-defined for *any* primal mesh — the carved hex
  mesh this crate produces, not only a Delaunay tetrahedralisation. The
  price is that it inherits **none** of the Voronoi dual's orthogonality
  properties: a median-dual face is not in general perpendicular to the
  primal edge it separates.
- **It is not the same algorithm as `outram-foam-mesh`'s
  `poly_dual_mesh`.** That one is the **cell-centre** dual (dual vertices
  placed at primal *cell centroids*, one dual face per primal *edge*) — a
  genuinely different construction, on a different primal→dual entity map.
  Its measured V&V result — "dualisation does not create
  non-orthogonality; the dual of a uniform hex block measures exactly 0 deg
  and 0 skewness" — is a statement about *that* algorithm and **must not be
  transferred to this one**. Nothing in this module has been measured
  against it.

# Honest V&V scope (what the tests in this module actually gate)

The four tests below assert exactly four properties, and no more:

1. **closure** — `VolumeMesh::validate` passes (every cell is a closed
   surface), plus the internal/boundary face split is consistent;
2. **volume conservation** — `Σ dual-cell volumes == primal domain volume`
   to `1e-9`;
3. **boundary match** — the dual boundary area equals the primal surface
   area to `1e-9`;
4. **face count / polyhedrality** — one cell per primal vertex, interior
   cells have more than 6 faces, and the face-minimal variant has strictly
   fewer faces than the quad-fan one.

**There is no orthogonality or skewness test for this crate's dual.** No
test in this module calls [`crate::checks::check_quality`], so the
non-orthogonality and skewness of a median dual produced here are
**unmeasured** at the module level. The only numbers this crate records for
them are the whole-pipeline sphere table in [`crate::pipeline`]'s tests,
which measures the *composed* pipeline (carve → snap → tet → dual → smooth)
and therefore cannot attribute a figure to the dual step alone. Treat any
claim about this dual's orthogonality as unverified until such a test
exists.

Each primal cell is split into one **corner sub-cell** per vertex: the part
of the cell nearest that vertex, bounded by

- the primal vertex `v`,
- the **midpoints** of the primal edges at `v`,
- the **centroids** of the primal faces at `v`,
- the primal **cell centroid**.

The dual cell of `v` is the union of the corner sub-cells of every primal
cell that touches `v`. Two kinds of quad make up a sub-cell's boundary:

- **inner quads** `[edge-midpoint, face-centroid, cell-centroid,
  face-centroid]` — one per primal edge inside the cell. The inner quad of
  edge `(v, w)` is listed by *both* dual cells `v` and `w`, so it becomes an
  **internal** dual face separating them.
- **outer quads** `[v, edge-midpoint, face-centroid, edge-midpoint]` on the
  primal *boundary* faces — these tile each boundary face over its vertices
  and become the **boundary** faces of the dual mesh.

Because the sub-cells partition the domain exactly, the dual tiles the same
region: `Σ dual-cell volumes == primal domain volume`, every internal dual
face is shared by exactly two dual cells, and the dual boundary surface
coincides with the primal boundary surface.

# Implementation

Winding and owner/neighbour are recovered by [`from_cell_faces`], which
matches shared faces by vertex set and orients every face outward from its
owning cell's centroid — so this routine only has to emit each dual cell's
quads (in any winding) and hand them over. Geometry points (edge midpoints,
face/cell centroids) are allocated **per primal cell**; the inner quad of an
edge is always built inside a single cell, so the two endpoints reference the
same indices and the shared-face match is exact.

# Two variants

[`polyhedral_dual`] (this construction) splits the face between two dual
cells into one quad per surrounding primal cell — robust and watertight, but
more, smaller faces than a minimal `polyDualMesh`. [`polyhedral_dual_min_faces`]
produces the **face-minimal** dual: one polygon per primal edge, built by
walking the edge's star of incident faces/cells (~40% fewer faces on a
Cartesian block), verified to conserve volume and stay closed. Prefer the
face-minimal variant for solver meshes; the quad-fan variant is the simpler,
maximally-robust fallback.

Orientation assumes each dual cell is star-shaped about its primal vertex
(true for structured/graded blocks); [`VolumeMesh::validate`] is the gate
that catches any cell that is not. Pure Rust, Android-safe.

```rust
pub mod dual { /* ... */ }
```

### Functions

#### Function `polyhedral_dual`

Build the polyhedral **median dual** of `mesh`: one polyhedral cell per
primal vertex. See the module docs for the construction and its guarantees.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, dual::polyhedral_dual};

// Carve a unit box into 2×2×2 hexes (27 vertices), then take the dual:
// 27 polyhedral cells that tile the same volume.
let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
let hex = carve_box(&p, &t, 0.5);
let dual = polyhedral_dual(&hex);

assert_eq!(dual.cell_count(), hex.point_count()); // one dual cell per vertex
assert!((dual.total_volume() - hex.total_volume()).abs() < 1e-9);
assert!(dual.validate().is_ok());
```

```rust
pub fn polyhedral_dual(mesh: &crate::volume_mesh::VolumeMesh) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

#### Function `polyhedral_dual_min_faces`

Build the **face-minimal** polyhedral dual of `mesh`: like
[`polyhedral_dual`], one polyhedral cell per primal vertex, but the face
between two dual cells is a **single polygon** rather than a fan of quads.

This is the proper `polyDualMesh` face topology. For each primal **edge**
`(a, b)` the dual face separating cells `a` and `b` is one ring built by
walking the edge's star of incident faces and cells:

- **interior edge** — the star closes, giving the ring
  `[faceCentroid, cellCentroid, faceCentroid, cellCentroid, …]`;
- **boundary edge** — the star is an open fan; the ring is closed on the
  domain-boundary side through the **edge midpoint**:
  `[edgeMidpoint, faceCentroid₀, cellCentroid₀, …, faceCentroidₙ]`.

The domain boundary is tiled by the same per-vertex outer quads as
[`polyhedral_dual`] (`[vertex, edgeMidpoint, faceCentroid, edgeMidpoint]`),
so the dual boundary coincides with the primal boundary surface and the
**volume is conserved** (a shared internal face contributes equally and
oppositely to its two cells; only the boundary sets the volume).

Far fewer faces than [`polyhedral_dual`] (one per primal edge, not one per
(edge, surrounding cell)). Assumes a **manifold** primal mesh (each interior
edge ringed by a single closed cell cycle, each boundary edge by exactly two
boundary faces) — the meshes this crate generates. Owner/neighbour and
winding are recovered by [`from_cell_faces`].

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, dual::polyhedral_dual_min_faces};

let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
let hex = carve_box(&p, &t, 0.5);
let dual = polyhedral_dual_min_faces(&hex);

assert_eq!(dual.cell_count(), hex.point_count()); // one cell per primal vertex
assert!((dual.total_volume() - hex.total_volume()).abs() < 1e-9);
assert!(dual.validate().is_ok());
```

```rust
pub fn polyhedral_dual_min_faces(mesh: &crate::volume_mesh::VolumeMesh) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `layers`

Prism **boundary layers** — insert graded near-wall inflation layers at a
wall patch.

This is snappyHexMesh's *addLayers* / cfMesh's boundary-layer step. Given a
wall patch, it moves the interior mesh's wall points **inward** by the total
layer thickness and fills the vacated shell with `n_layers` stacked prism
cells per wall face, with a geometric thickness progression
(`first_thickness`, then × `expansion` each layer). The result resolves the
wall-normal gradients a CFD/TH solver needs (low y⁺).

# How it stays valid

Only the wall points move (they belong to the boundary), so the wall-adjacent
cells simply *shrink* — no interior topology changes and no hanging nodes.
The prism stack of each wall face is 1:1 with that face; adjacent stacks
share their side faces automatically (matched by [`from_cell_faces`]). The
operation is a **repartition** of the same region, so it preserves the mesh
volume exactly.

# v1 scope

A single wall patch, moved along the averaged per-point inward normal (so a
sharp convex corner shrinks along its diagonal — adequate for modest total
thickness). Multi-patch selection and corner-feature handling are future
work. The total thickness must be smaller than the wall cells or they would
invert. Pure Rust, Android-safe.

```rust
pub mod layers { /* ... */ }
```

### Functions

#### Function `add_boundary_layers`

Insert `n_layers` graded prism boundary layers at the patch named
`patch_name`, returning the new mesh.

`first_thickness` is the first (wall-nearest) layer thickness; each
subsequent layer is `expansion` times thicker. If the patch is missing or
`n_layers == 0`, `mesh` is returned unchanged (rebuilt).

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, layers::add_boundary_layers};

// Carve a box (single `walls` patch), then add 3 prism layers; the volume
// is preserved (a repartition) and the mesh stays closed.
let (p, t) = box_surface(Vec3::ZERO, Vec3::new(3.0, 3.0, 3.0));
let m = carve_box(&p, &t, 1.0);
let l = add_boundary_layers(&m, "walls", 3, 0.05, 1.3);
assert!((l.total_volume() - m.total_volume()).abs() < 1e-9);
assert!(l.validate().is_ok());
```

```rust
pub fn add_boundary_layers(mesh: &crate::volume_mesh::VolumeMesh, patch_name: &str, n_layers: usize, first_thickness: f64, expansion: f64) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

#### Function `add_boundary_layers_adaptive`

Insert graded prism boundary layers that **adapt to curved walls**.

[`add_boundary_layers`] marches every wall point inward by the same fixed
total thickness along its averaged normal — exact and ideal on flat (box)
walls, but on a **convex curved** wall those inward normals converge and the
layers self-intersect, invalidating the mesh. This variant makes curved-wall
layers work by:

1. **Smoothing the wall-normal field** (a few Laplacian passes over the
   wall-point adjacency) so neighbouring march directions vary gently — the
   core idea of advancing-layer meshers (VMTK / Netgen) versus snappyHexMesh's
   raw averaged normal.
2. **Per-point thickness limiting** to a fraction of the local wall spacing,
   so tight regions get thinner layers.
3. A **global validity back-off**: it builds the layered mesh and, if it is
   not closed (self-intersection), scales the thickness down and retries,
   keeping the thickest layers that still produce a valid mesh.

So it returns the thickest valid graded prism layers the geometry admits,
rather than an invalid mesh (or, on a wall too tight for any layer, the input
rebuilt with none). The requested `first_thickness` / `expansion` set the
*profile*; the achieved absolute thickness may be smaller after back-off.

# Limitations

The back-off is global (uniform scale), so one tight feature thins the whole
wall; true per-point marching with local layer termination (snappyHexMesh
`addLayers` / AFLR advancing-layer) is future work. Verified valid + solvable
on snapped spheres and cylinders. Pure Rust, Android-safe.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::sphere_surface, carve::carve_box,
    snap::snap_to_surface, dual::polyhedral_dual_min_faces, layers::add_boundary_layers_adaptive};

// A snapped, dualised sphere — a curved polyhedral wall — takes prism layers.
let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 20, 40);
let poly = polyhedral_dual_min_faces(&snap_to_surface(&carve_box(&p, &t, 0.6), &p, &t));
let layered = add_boundary_layers_adaptive(&poly, "walls", 3, 0.04, 1.3);
assert!(layered.validate().is_ok());
assert!(layered.cell_count() > poly.cell_count()); // prism cells were added
```

```rust
pub fn add_boundary_layers_adaptive(mesh: &crate::volume_mesh::VolumeMesh, patch_name: &str, n_layers: usize, first_thickness: f64, expansion: f64) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `math`

Minimal fixed-size 3-vector for mesh geometry.

A small, dependency-free [`Vec3`] (three `f64`s) — positions, edge vectors,
face area vectors, cell centres. Kept local to the crate (like
`outram-blender`'s `math::Vec3`) so the volume-meshing core stays pure Rust
and Android-buildable with no external linear-algebra dependency. The large
solves the mesher never needs; all of its geometry is these fixed-size ops.

```rust
pub mod math { /* ... */ }
```

### Types

#### Struct `Vec3`

A 3-component vector `(x, y, z)` of `f64`s. `Copy`, so pass by value.

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
| `x` | `f64` | X component. |
| `y` | `f64` | Y component. |
| `z` | `f64` | Z component. |

##### Implementations

###### Methods

- ```rust
  pub const fn new(x: f64, y: f64, z: f64) -> Self { /* ... */ }
  ```
  Construct from components.

- ```rust
  pub fn add(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Component-wise sum `self + other`.

- ```rust
  pub fn sub(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Component-wise difference `self - other`.

- ```rust
  pub fn scale(self: Self, s: f64) -> Vec3 { /* ... */ }
  ```
  Scalar multiple `self * s`.

- ```rust
  pub fn dot(self: Self, other: Vec3) -> f64 { /* ... */ }
  ```
  Dot product `self · other`.

- ```rust
  pub fn cross(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Cross product `self × other`.

- ```rust
  pub fn length(self: Self) -> f64 { /* ... */ }
  ```
  Euclidean length `|self|`.

- ```rust
  pub fn normalize(self: Self) -> Vec3 { /* ... */ }
  ```
  Unit vector in the same direction, or [`Vec3::ZERO`] for a zero-length

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `octree`

Octree near-wall **refinement** — grade the mesh finer near the surface,
keeping it conforming by splitting the coarse transition faces.

[`crate::carve::carve_box`] produces a *uniform* grid. This module refines
the cells near the boundary one or more levels finer (cfMesh's octree
`meshOctree` refinement, and snappyHexMesh's castellation refinement). Where
a coarse cell meets four finer cells, the coarse cell's shared face is
represented as the **four fine sub-faces** — the hanging-node treatment that
keeps the mesh conforming and turns the coarse transition cell into a
genuine **polyhedron** (more than six faces).

# Two refinement criteria (enum-dispatched, no trait objects)

- [`refine_near_boundary`] — refine a leaf if a same-level face-neighbour
  centre is *outside* the surface, i.e. the leaf touches the wall. This is
  the original one-cell-thick shell criterion.
- [`refine_near_boundary_banded`] — refine a leaf if its centre is within a
  **distance band** of the surface, the band measured in multiples of that
  leaf's own edge length. This is the criterion the high-level pipeline
  ([`crate::pipeline::TetDualOptions::refinement_levels`]) drives, because a
  band wider than one cell grades the transition out over several cells
  instead of jamming it against the wall.

# Edge conformity (hanging nodes inserted into coarse rings)

Splitting only the *shared* face is enough for the coarse cell to stay
**closed** (its face-area vectors still sum to zero), but it leaves the
coarse cell's *other* faces with a T-junction: an edge of a coarse side face
carries a hanging vertex that only the fine sub-faces reference. Such a cell
is closed but **not combinatorially manifold** — an edge lies in only one of
its faces — and [`crate::tet::tetrahedralize`]'s centroid subdivision then
emits interior triangles that never find a partner, silently punching holes
in the tet mesh (and corrupting its volume).

The mesh assembler therefore runs an **edge-conformity pass**: every emitted face
ring has each lattice point that lies strictly inside one of its edges
inserted into the ring. The insertion is geometrically a no-op (the points
are collinear, so the face's area vector and centroid are unchanged) but
makes every edge lie in exactly two faces of every cell, which is what the
downstream tet → dual path needs.

# Scope

Refinement is driven by proximity to the surface only — **no curvature or
feature-edge criterion yet**, and no size field from an external source.
Levels are 2:1-balanced (neighbouring leaves differ by at most one level)
across *faces*; edge- and vertex-diagonal neighbours may differ by more, and
the conformity pass above handles the extra hanging nodes that produces.
Pure Rust, Android-safe.

```rust
pub mod octree { /* ... */ }
```

### Functions

#### Function `refine_near_boundary`

Carve the closed surface (`points`, `tris`) at `base_cell_size`, then refine
the near-surface cells up to `max_level` levels finer, returning the graded
[`VolumeMesh`]. Refinement proceeds one level at a time on the boundary
leaves; a 2:1 **balancing** pass then guarantees neighbouring cells differ
by at most one level, so the hanging-node face split stays valid and coarse
transition cells become polyhedral (their shared face is the fine sub-faces).

`max_level = 0` is the uniform carve; `1` refines the immediate wall layer;
higher values grade progressively finer toward the surface. Returns an empty
mesh for a degenerate input.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, octree::refine_near_boundary};

// A box refined near its walls keeps the exact box volume and stays closed.
let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
let m = refine_near_boundary(&p, &t, 0.5, 1);
assert!((m.total_volume() - 8.0).abs() < 1e-9);
assert!(m.validate().is_ok());
# fn box_surface(a: Vec3, b: Vec3) -> (Vec<Vec3>, Vec<[usize; 3]>) {
#     let v = vec![
#         Vec3::new(a.x, a.y, a.z), Vec3::new(b.x, a.y, a.z), Vec3::new(b.x, b.y, a.z), Vec3::new(a.x, b.y, a.z),
#         Vec3::new(a.x, a.y, b.z), Vec3::new(b.x, a.y, b.z), Vec3::new(b.x, b.y, b.z), Vec3::new(a.x, b.y, b.z)];
#     let q = |a:usize,b:usize,c:usize,d:usize| vec![[a,b,c],[a,c,d]];
#     let mut t = Vec::new();
#     for f in [q(0,3,2,1), q(4,5,6,7), q(0,1,5,4), q(2,3,7,6), q(1,2,6,5), q(0,4,7,3)] { t.extend(f); }
#     (v, t)
# }
```

```rust
pub fn refine_near_boundary(points: &[crate::math::Vec3], tris: &[[usize; 3]], base_cell_size: f64, max_level: u8) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

#### Function `refine_near_boundary_banded`

Carve the closed surface (`points`, `tris`) at `base_cell_size`, then refine
every leaf whose centre lies within a **distance band** of the surface, up to
`max_level` levels finer, returning the graded [`VolumeMesh`].

This is the size-field form of [`refine_near_boundary`] and the one the
high-level pipeline uses.

# The band

`band_cells` is **dimensionless — a multiple of the candidate leaf's own edge
length**, not a length in metres. A level-`L-1` leaf (edge
`base_cell_size / 2^(L-1)` metres) is split into its eight level-`L` children
iff

```text
  distance(leaf centre, surface)  <  band_cells * base_cell_size / 2^(L-1)
```

Because the band shrinks with the cell, the refined region is a **graded
shell** that hugs the wall: level 1 covers a band `band_cells` base-cells
thick, level 2 the inner half of it, and so on. `band_cells = 1.0` (the
pipeline default) refines roughly the leaves that touch the surface, which
reproduces [`refine_near_boundary`]'s shell on grid-aligned geometry;
`band_cells = 2.0` grades the transition out over two cells, which is gentler
on cell-size jumps at the cost of more cells.

# Inputs and units

- `points` / `tris` — a **closed, watertight, outward-wound** triangle soup,
  vertex positions in metres.
- `base_cell_size` — level-0 cell edge, in metres; must be `> 0`.
- `max_level` — refinement depth. `0` is the uniform carve (identical to
  [`crate::carve::carve_box`] up to face ordering); practical values are
  `1`–`3` (each level halves the local edge, so level 3 is a 1/8 edge).
- `band_cells` — dimensionless, `> 0`. Non-positive means "never refine".

Returns an empty mesh for a degenerate input (non-positive `base_cell_size`,
fewer than four points, no triangles, or nothing carved).

# Cost

Each candidate leaf runs one exact point-to-surface distance over every
triangle (`O(leaves x triangles)`), so this is materially slower per cell
than the uniform carve — the payoff is far fewer cells for the same wall
resolution. See the [`crate::pipeline`] tests for measured numbers.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface,
    octree::refine_near_boundary_banded, carve::carve_box};

// A box [0,4]^3: refine the wall band one level at base size 1 m.
let (p, t) = box_surface(Vec3::ZERO, Vec3::new(4.0, 4.0, 4.0));
let graded = refine_near_boundary_banded(&p, &t, 1.0, 1, 1.0);

// Volume is exact and every cell (including the polyhedral transition
// cells) is closed.
assert!((graded.total_volume() - 64.0).abs() < 1e-9);
assert!(graded.validate().is_ok());
// Far fewer cells than carving the whole box at the refined size 0.5 m.
assert!(graded.cell_count() < carve_box(&p, &t, 0.5).cell_count());
```

```rust
pub fn refine_near_boundary_banded(points: &[crate::math::Vec3], tris: &[[usize; 3]], base_cell_size: f64, max_level: u8, band_cells: f64) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `patches`

**Named boundary patches** — carry the input surface's region names through
meshing so the output `polyMesh` has an `inlet` / `outlet` / `walls` split a
solver case can actually be set up against.

# Why this module exists

Every mesher in this crate emits its boundary into a *single* patch called
`"walls"`: [`crate::carve::carve_box`] hard-codes it, and
[`crate::volume_mesh::from_cell_faces`] — which every stage after the carve
(tetrahedralize, dual, layers) rebuilds through — has no patch information to
work from at all, because it recovers connectivity by matching face vertex
*sets*. A mesh with one patch is unusable for CFD: you cannot write a `0/`
boundary-condition directory that says "fixed velocity here, zero gradient
there" when there is only one "there".

# How the names survive

Rather than thread a patch tag through five stages that each rebuild the mesh
from scratch (and through the boundary-layer step, which *creates* boundary
faces that never existed in the input), this module recovers the assignment
**geometrically, once, at the end**: every boundary face of the finished mesh
is given the region of the input-surface triangle **closest to its centroid**.

That is exactly how snappyHexMesh assigns a cut face to a surface region, and
it is well-posed here because every boundary face of a finished mesh lies on
(post-snap) or within roughly one cell of the input surface. It also handles
the faces the layer stage invents, which no tag-threading scheme could.

# What it does not do

- **Region resolution is limited by the mesh.** Two regions closer together
  than a cell can be mixed up on the cells that straddle them; features are
  resolved by refining, not by this classifier.
- **Prism layers are still grown over the whole boundary**, because the
  classification runs after the layer stage. Selecting which patches get
  layers (snappyHexMesh's per-patch `nSurfaceLayers`) is future work — see
  [`crate::pipeline::surface_to_tet_dual_mesh_multipatch`].
- **No feature-edge snapping**, so a patch boundary follows the mesh's face
  edges, not the surface's feature edge.

Pure Rust, no dependencies, Android-safe.

```rust
pub mod patches { /* ... */ }
```

### Types

#### Struct `SurfaceRegions`

A labelling of an input surface's triangles into **named regions**, one
region per intended boundary patch (`inlet`, `outlet`, `walls`, ...).

This is the mesher's input side of the patch story: the caller says which
triangle belongs to which named region, and
[`assign_patches_by_region`] turns that into the output mesh's
[`BoundaryPatch`] list.

# Invariants

- `region_of_tri.len()` must equal the triangle count of the surface it
  labels; `region_of_tri[i]` is an index into [`Self::names`].
- `names` must be non-empty and should be unique; patch order in the output
  mesh follows `names` order.

Both are checked by [`Self::validate`], which the pipeline calls before
meshing so a mislabelled surface is a clear error, not a silent wrong mesh.

```rust
pub struct SurfaceRegions {
    pub names: Vec<String>,
    pub region_of_tri: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `names` | `Vec<String>` | Patch names, in the order the output mesh's patches will appear. |
| `region_of_tri` | `Vec<usize>` | `region_of_tri[i]` = index into [`Self::names`] for input triangle `i`. |

##### Implementations

###### Methods

- ```rust
  pub fn single(name: &str, n_tris: usize) -> Self { /* ... */ }
  ```
  Every triangle in one region — the single-patch behaviour the crate had

- ```rust
  pub fn from_labels(labels: &[&str]) -> Self { /* ... */ }
  ```
  Build from a **per-triangle name**: `labels[i]` is the patch name for

- ```rust
  pub fn region_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of named regions.

- ```rust
  pub fn validate(self: &Self, n_tris: usize) -> Result<(), String> { /* ... */ }
  ```
  Check the invariants against a surface of `n_tris` triangles: names

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SurfaceRegions { /* ... */ }
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
    fn eq(self: &Self, other: &SurfaceRegions) -> bool { /* ... */ }
    ```

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
### Functions

#### Function `assign_patches_by_region`

Re-bucket `mesh`'s boundary faces into **named patches**, one per region of
the input surface, and return the re-ordered mesh.

Each boundary face is assigned the region of the input triangle **nearest to
its centroid** (exact point-to-triangle distance, the same one
[`crate::snap::snap_to_surface`] projects with, so a snapped face lands on
the region it was snapped onto). Faces are then re-ordered so that:

- all internal faces come first (the OpenFOAM prefix rule), keeping their
  relative order and their owner/neighbour pairing;
- each non-empty patch is a **contiguous run** of boundary faces, in
  [`SurfaceRegions::names`] order, with a matching
  [`BoundaryPatch::start_face`] / [`BoundaryPatch::n_faces`].

Empty patches are dropped (a region no boundary face landed on produces no
patch), so the caller should not assume a 1:1 patch/region correspondence.

Points, cells and topology are untouched — only the face *ordering* and the
patch list change — so the mesh's volume, closure and quality are identical
to the input's.

# Inputs and units

- `mesh` — a finished [`VolumeMesh`]; must satisfy the usual invariant that
  `owner` / `neighbour` are per-face and in range.
- `points` / `tris` — the **input surface** the mesh was generated from,
  positions in metres. Must be the same surface, in the same frame.
- `regions` — labels for `tris`; see [`SurfaceRegions::validate`].

# Errors

`Err` if `regions` does not describe `tris` ([`SurfaceRegions::validate`]),
or if the surface has no triangles (nothing to classify against).

# Cost

`O(boundary_faces x triangles)` exact distance evaluations, with a
bounding-box reject that skips triangles which cannot beat the current best.
There is no spatial index yet, so a large surface with a fine mesh is slow;
the single-region case is short-circuited by the pipeline and costs nothing.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box,
    patches::{SurfaceRegions, assign_patches_by_region}};

// box_surface emits its 12 triangles face by face: -Z, +Z, -Y, +Y, +X, -X.
let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
let regions = SurfaceRegions::from_labels(&[
    "walls", "walls", "walls", "walls", "walls", "walls",
    "walls", "walls", "outlet", "outlet", "inlet", "inlet",
]);

let mesh = carve_box(&p, &t, 0.5);
let named = assign_patches_by_region(&mesh, &p, &t, &regions).unwrap();

// Three patches, and they tile the boundary contiguously.
assert_eq!(named.patches.len(), 3);
assert!((named.total_volume() - 1.0).abs() < 1e-9); // geometry untouched
```

```rust
pub fn assign_patches_by_region(mesh: &crate::volume_mesh::VolumeMesh, points: &[crate::math::Vec3], tris: &[[usize; 3]], regions: &SurfaceRegions) -> Result<crate::volume_mesh::VolumeMesh, String> { /* ... */ }
```

## Module `pipeline`

**High-level meshing pipeline** — one call turns a closed triangulated
surface into a polyhedral volume mesh with near-wall prism boundary layers,
via the **tetrahedral → dual** path.

This is the coarse-grained entry point the `outram-blender` "mesh studio"
GUI (and any programmatic caller) uses instead of hand-wiring the individual
`carve → snap → tet → dual → layers` stages. It composes the crate's existing
primitives; it introduces no new geometry algorithm of its own.

# The tet → dual path

```text
  surface        carve_box        snap_to_surface     tetrahedralize
  (tri soup) ──► (hex bg mesh) ──► (body-fit bndry) ──► (all-tet mesh)
                                                             │
                   add_boundary_layers_adaptive   polyhedral_dual_min_faces
  polyhedral   ◄── (prism wall layers)         ◄── (one cell / tet vertex)
  volume mesh
```

Taking the polyhedral **dual of a tetrahedralization** (rather than the dual
of the raw hex carve) is the classic route to a well-connected polyhedral
mesh: every tet vertex — the primal corners plus the per-face and per-cell
centroids the tetrahedralizer inserts — becomes one polyhedral cell, so the
interior packs many neighbours per cell for better gradient reconstruction at
a low cell count, the same idea as OpenFOAM's `polyDualMesh` fed a tet mesh.

# Graceful stage degradation

Curved geometry does not always survive every stage: a coarse snap can tangle
a wall cell, a dual can fail on a non-star-shaped cell, and a fixed prism
march can self-intersect. Rather than return a broken mesh, each optional
stage here is **applied only if its result is still acceptable** — closed
([`VolumeMesh::validate`]) *and* free of negative-volume cells
([`check_quality`](crate::checks::check_quality)). If a stage would break
that, it is **skipped**, the previous mesh is kept, and a human-readable line
is appended to [`TetDualReport::stage_notes`] so the caller (and the GUI) can
show exactly what ran and what was skipped. The returned mesh is therefore
always valid and exportable. This mirrors the `mesh_studio` example's
degradation approach, lifted into a reusable library entry point.

The negative-cell guard is deliberately stricter than the bare `validate()`
check the `mesh_studio` example uses: a mesh can be *closed* yet still be
tangled (a cell folded through itself sums its oriented face areas to zero but
has negative volume), and such a mesh is unusable by a solver, so the
pipeline refuses to accept a stage that produces one.

# Units and conventions

All lengths are in **metres**: [`TetDualOptions::cell_size`] (background
Cartesian edge), [`TetDualOptions::first_layer_thickness`] (first prism layer,
wall-normal). [`TetDualOptions::expansion`] is the dimensionless layer-to-layer
growth ratio (`>= 1`). Angles in [`TetDualReport`] are in **degrees**. Volume
([`TetDualReport::total_volume`]) is in **cubic metres**.

# Scope and trust

**Untrusted AI-assisted draft pending human V&V.** The V&V here is
*verification* — valid mesh topology (closed cells, no inverted cells) and
volume conservation versus the analytic volume of the built-in primitives —
**not** *validation* against a CFD/TH solve. See the module tests for
methodology and measured results. Pure Rust, no dependencies, Android-safe.

```rust
pub mod pipeline { /* ... */ }
```

### Types

#### Struct `TetDualOptions`

Tuning knobs for [`surface_to_tet_dual_mesh`] and the primitive wrappers.

Every optional stage can be turned off. Lengths are in **metres**, angles in
degrees, and [`Self::expansion`] is dimensionless. Construct with
[`TetDualOptions::default`] and override the fields you care about.

```rust
pub struct TetDualOptions {
    pub cell_size: f64,
    pub refinement_levels: u8,
    pub refinement_band: f64,
    pub snap: bool,
    pub delaunay: bool,
    pub max_flips: usize,
    pub dual: bool,
    pub dual_min_faces: bool,
    pub smooth_passes: usize,
    pub n_layers: usize,
    pub first_layer_thickness: f64,
    pub expansion: f64,
    pub wall_patch: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell_size` | `f64` | Background Cartesian **cell edge length**, in metres. Sets the base<br>resolution: smaller means more, finer cells (and slower meshing). Must be<br>`> 0`; a value that carves zero cells is a hard error.<br><br>With [`Self::refinement_levels`] `> 0` this is the **coarse interior**<br>edge; the near-wall cells end up `cell_size / 2^refinement_levels`. |
| `refinement_levels` | `u8` | Octree **near-wall refinement depth** (dimensionless count of levels).<br><br>`0` (the default) carves a *uniform* grid at [`Self::cell_size`] — the<br>crate's original behaviour, byte-for-byte. `L > 0` instead carves a<br>**graded** background mesh ([`refine_near_boundary_banded`]): the interior<br>stays at `cell_size`, and cells near the surface are split up to `L`<br>levels finer, so the wall is resolved at `cell_size / 2^L` metres while<br>the interior is not. Each level roughly halves the local wall spacing;<br>practical values are `1`-`2`.<br><br>Grading is what makes a given wall resolution affordable — see the module<br>tests for measured cell-count / accuracy trade-offs on a sphere. |
| `refinement_band` | `f64` | Width of the refinement band, **dimensionless — in multiples of the<br>candidate cell's own edge length** (not metres).<br><br>A cell is split toward the next level iff its centre lies within<br>`refinement_band x (its own edge)` of the input surface. Because the band<br>scales with the cell, the refined region is a graded shell hugging the<br>wall. `1.0` (the default) refines roughly the cells that touch the<br>surface; `2.0` spreads the transition over two cells (gentler size jumps,<br>more cells). Ignored when [`Self::refinement_levels`] is `0`; a<br>non-positive value refines nothing. |
| `snap` | `bool` | If `true`, project the carved staircase boundary onto the input surface<br>([`snap_to_surface`]) to body-fit it. Skipped (with a note) if it would<br>tangle a wall cell. |
| `delaunay` | `bool` | If `true`, improve the tetrahedralization toward Delaunay by bistellar<br>flips ([`flip_to_delaunay`]) before taking the dual. Safe: the flipper is<br>improve-or-noop, so this never makes the mesh worse. |
| `max_flips` | `usize` | Flip budget passed to [`flip_to_delaunay`] when [`Self::delaunay`] is set. |
| `dual` | `bool` | If `true`, take the polyhedral dual of the tet mesh (the "→ dual" step).<br>If `false`, the returned mesh is the tetrahedralization itself. |
| `dual_min_faces` | `bool` | Prefer the **face-minimal** dual ([`polyhedral_dual_min_faces`]); on<br>failure the pipeline falls back to the robust quad-fan<br>[`polyhedral_dual`], then to skipping the dual. Ignored if [`Self::dual`]<br>is `false`. |
| `smooth_passes` | `usize` | Smart-Laplacian smoothing passes ([`laplacian_smooth`]) applied to the<br>interior after the dual and before the layers. `0` disables it. Safe:<br>smoothing never inverts a cell and conserves volume exactly. |
| `n_layers` | `usize` | Number of prism **boundary layers** to grow on the wall patch<br>([`add_boundary_layers_adaptive`]). `0` disables the layer stage. |
| `first_layer_thickness` | `f64` | First (wall-nearest) prism layer thickness, in **metres** (wall-normal).<br>Subsequent layers grow by [`Self::expansion`]. The adaptive layerer may<br>reduce the achieved thickness to keep the mesh valid on tight curvature. |
| `expansion` | `f64` | Geometric layer-to-layer growth ratio (dimensionless, `>= 1`): layer `i`<br>is `expansion^i × first_layer_thickness` thick. |
| `wall_patch` | `String` | Name of the boundary patch to grow layers on. The crate's meshers place<br>all exposed faces in a single `"walls"` patch, which is the default. |

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
    fn clone(self: &Self) -> TetDualOptions { /* ... */ }
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
    Sensible defaults for a metre-scale primitive (e.g. a radius-3 m sphere):

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &TetDualOptions) -> bool { /* ... */ }
    ```

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
#### Struct `TetDualReport`

What the pipeline produced and how it got there — enough for the GUI to show
the outcome and which stages were skipped.

Volumes are in cubic metres, angles in degrees. [`Self::stage_notes`] carries
one line per gracefully-degraded (skipped) stage; an empty list means every
requested stage ran.

```rust
pub struct TetDualReport {
    pub stage_notes: Vec<String>,
    pub cell_count: usize,
    pub total_volume: f64,
    pub valid: bool,
    pub max_non_orthogonality_deg: f64,
    pub max_skewness: f64,
    pub n_negative_volume_cells: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stage_notes` | `Vec<String>` | One human-readable line per stage that was skipped to keep the mesh valid<br>(e.g. `"snap-to-surface skipped — it would invert a wall cell"`). Empty if<br>nothing was skipped. |
| `cell_count` | `usize` | Final cell count. |
| `total_volume` | `f64` | Final total enclosed volume, in cubic metres ([`VolumeMesh::total_volume`]). |
| `valid` | `bool` | `true` iff the final mesh passes [`VolumeMesh::validate`] (closed cells,<br>in-range addressing). The pipeline errors out rather than returning a<br>mesh for which this is `false`, so on `Ok` this is always `true`. |
| `max_non_orthogonality_deg` | `f64` | Maximum face **non-orthogonality**, in degrees (see [`check_quality`]).<br>Boundary-layer meshes legitimately exceed `checkMesh`'s 70° warning (see<br>the module tests); interpret against a looser near-wall bound. |
| `max_skewness` | `f64` | Maximum face **skewness** (dimensionless; see [`check_quality`]). |
| `n_negative_volume_cells` | `usize` | Number of negative-volume cells in the final mesh — `0` for an accepted<br>result (the acceptance gate rejects any stage that would raise it). |

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
    fn clone(self: &Self) -> TetDualReport { /* ... */ }
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
    fn eq(self: &Self, other: &TetDualReport) -> bool { /* ... */ }
    ```

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
### Functions

#### Function `surface_to_tet_dual_mesh`

Turn a **closed, watertight, outward-wound** triangulated surface into a
polyhedral volume mesh with near-wall prism boundary layers, via the
tetrahedral → dual path. This is the crate's high-level meshing entry point.

# Inputs

- `points` — surface vertex positions, in metres.
- `tris` — triangle vertex-index triples into `points`; the surface must be
  closed and consistently **outward**-wound (as produced by
  [`crate::shapes`]). A non-watertight or inward-wound soup gives an
  ill-defined inside test and is not supported.
- `opts` — [`TetDualOptions`] (resolution, which stages to run, layer spec).

# Pipeline (each optional stage gated by the private `acceptable` check —
closed *and* no inverted cells; see the module docs)

1. **Carve** a background hex mesh of the surface interior
   ([`carve_box`]) — the only mandatory stage; zero carved cells is an error.
2. **Snap** the boundary onto the surface ([`snap_to_surface`]) if
   `opts.snap`.
3. **Tetrahedralize** ([`tetrahedralize`]); if the result is unacceptable the
   mesh stays un-tetrahedralized (the dual then runs on the hex mesh).
4. **Delaunay-improve** the tets ([`flip_to_delaunay`]) if `opts.delaunay`.
5. **Dual** ([`polyhedral_dual_min_faces`] preferred, then
   [`polyhedral_dual`], then skip) if `opts.dual`.
6. **Smooth** ([`laplacian_smooth`]) `opts.smooth_passes` passes.
7. **Boundary layers** ([`add_boundary_layers_adaptive`]) if
   `opts.n_layers > 0`, on `opts.wall_patch`.

# Returns

`Ok((mesh, report))` with a **valid** mesh (guaranteed to pass
[`VolumeMesh::validate`]) and a [`TetDualReport`] describing the outcome and
any skipped stages. `Err(msg)` only when meshing cannot start or finish at all
(`cell_size <= 0`, the carve produced zero cells, or — should not happen — the
final mesh is somehow not closed).

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface,
    pipeline::{surface_to_tet_dual_mesh, TetDualOptions}};

let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
let opts = TetDualOptions { cell_size: 0.5, first_layer_thickness: 0.02, ..Default::default() };
let (mesh, report) = surface_to_tet_dual_mesh(&p, &t, &opts).unwrap();

assert!(mesh.validate().is_ok());
assert!(report.valid);
// A box survives every stage exactly: volume is conserved to 1 m³.
assert!((report.total_volume - 1.0).abs() < 1e-9);
```

```rust
pub fn surface_to_tet_dual_mesh(points: &[crate::math::Vec3], tris: &[[usize; 3]], opts: &TetDualOptions) -> Result<(crate::volume_mesh::VolumeMesh, TetDualReport), String> { /* ... */ }
```

#### Function `surface_to_tet_dual_mesh_multipatch`

The same pipeline as [`surface_to_tet_dual_mesh`], but carrying the input
surface's **named regions** through to the output mesh's boundary patches —
so the result has an `inlet` / `outlet` / `walls` split a solver case can be
set up against, instead of one undifferentiated `walls` patch.

# Why a separate entry point

Every stage after the carve rebuilds the mesh through
[`crate::volume_mesh::from_cell_faces`], which recovers connectivity by
matching face vertex sets and therefore cannot preserve a patch tag; the
boundary-layer stage additionally *creates* boundary faces that no input face
corresponds to. The names are therefore recovered **geometrically at the
end**, by [`assign_patches_by_region`]: each boundary face of the finished
mesh takes the region of the input triangle nearest its centroid. See
[`crate::patches`] for the rationale and the limits of that.

# Inputs

- `points` / `tris` — as [`surface_to_tet_dual_mesh`]: a closed, watertight,
  outward-wound surface, positions in metres.
- `regions` — one region index per triangle plus the patch names; build it
  with [`SurfaceRegions::from_labels`]. Validated before meshing starts.
- `opts` — [`TetDualOptions`], exactly as for the single-patch entry point.

# Limitation — layers are grown on the whole boundary

Because classification happens after stage 7, the boundary-layer stage still
sees the single `"walls"` patch and grows prisms over the **entire**
boundary, inlet and outlet included. That is a valid mesh, but it is not
snappyHexMesh's per-patch `nSurfaceLayers` behaviour; patch-selective layer
insertion is future work. Set `opts.n_layers = 0` if layers on the flow
openings are unacceptable for your case.

# Returns

As [`surface_to_tet_dual_mesh`], plus the patches on the returned mesh. A
region that no boundary face landed on yields **no** patch, so check
`mesh.patches` rather than assuming one patch per region. `Err` also when
`regions` does not describe `tris`.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface,
    patches::SurfaceRegions,
    pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions}};

// box_surface emits two triangles per side in the order -Z, +Z, -Y, +Y, +X, -X.
let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
let regions = SurfaceRegions::from_labels(&[
    "walls", "walls", "walls", "walls", "walls", "walls",
    "walls", "walls", "outlet", "outlet", "inlet", "inlet",
]);
let opts = TetDualOptions { cell_size: 0.5, n_layers: 0, ..Default::default() };
let (mesh, _report) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &opts).unwrap();

let names: Vec<&str> = mesh.patches.iter().map(|q| q.name.as_str()).collect();
assert!(names.contains(&"inlet") && names.contains(&"outlet") && names.contains(&"walls"));
```

```rust
pub fn surface_to_tet_dual_mesh_multipatch(points: &[crate::math::Vec3], tris: &[[usize; 3]], regions: &crate::patches::SurfaceRegions, opts: &TetDualOptions) -> Result<(crate::volume_mesh::VolumeMesh, TetDualReport), String> { /* ... */ }
```

#### Function `box_tet_dual`

Convenience wrapper: tet-dual mesh of an axis-aligned **box** `[min, max]`
(metres), using [`box_surface`]. See [`surface_to_tet_dual_mesh`].

```rust
pub fn box_tet_dual(min: crate::math::Vec3, max: crate::math::Vec3, opts: &TetDualOptions) -> Result<(crate::volume_mesh::VolumeMesh, TetDualReport), String> { /* ... */ }
```

#### Function `sphere_tet_dual`

Convenience wrapper: tet-dual mesh of a **sphere** of `radius` (metres) about
`centre`, triangulated with `n_lat` latitude bands × `n_lon` longitude
segments ([`sphere_surface`]). See [`surface_to_tet_dual_mesh`].

A sphere is a curved wall: expect the snap to body-fit the boundary and the
volume to sit within a few percent of the analytic `(4/3)πr³` at practical
resolutions (staircase/UV discretisation error), not exactly.

```rust
pub fn sphere_tet_dual(centre: crate::math::Vec3, radius: f64, n_lat: usize, n_lon: usize, opts: &TetDualOptions) -> Result<(crate::volume_mesh::VolumeMesh, TetDualReport), String> { /* ... */ }
```

#### Function `cylinder_tet_dual`

Convenience wrapper: tet-dual mesh of a Z-axis **cylinder** of `radius` and
`height` (metres) with base at `base`, `n_seg` circumferential segments
([`cylinder_surface`]). See [`surface_to_tet_dual_mesh`].

```rust
pub fn cylinder_tet_dual(base: crate::math::Vec3, radius: f64, height: f64, n_seg: usize, opts: &TetDualOptions) -> Result<(crate::volume_mesh::VolumeMesh, TetDualReport), String> { /* ... */ }
```

## Module `reactor`

Reactor geometry generators — structured packings of the shapes a reactor
model is built from, ready to hand to [`crate::carve::carve_around`].

- [`sphere_packing`] — a structured lattice of spheres (a **pebble bed** /
  TRISO packing).
- [`pin_lattice`] — a square lattice of vertical cylinders (**LWR** fuel
  pins / a channel bundle).
- [`bounding_domain`] — an axis-aligned box that wraps a set of surfaces
  with a margin (the coolant domain around a packing).

Each returns owned [`TriSoup`](crate::shapes::TriSoup)s; carve the coolant
region around them with [`crate::carve::carve_around`]. Pure Rust,
education/research use only (see the crate `RESPONSIBLE_USE.md`).

```rust
pub mod reactor { /* ... */ }
```

### Functions

#### Function `sphere_packing`

A structured `[nx, ny, nz]` cubic lattice of spheres of `radius`, centres
`spacing` apart, the first at the origin — a pebble-bed / particle packing.

`n_lat`/`n_lon` set each sphere's UV tessellation. Returns one
[`TriSoup`](crate::shapes::TriSoup) per pebble.

```rust
pub fn sphere_packing(counts: [usize; 3], spacing: f64, radius: f64, n_lat: usize, n_lon: usize) -> Vec<crate::shapes::TriSoup> { /* ... */ }
```

#### Function `pin_lattice`

A square `[nx, ny]` lattice of vertical cylinders of `radius` and `height`,
centre-to-centre `pitch`, bases on `z = 0`, the first at the origin — an LWR
fuel-pin bundle / channel array.

`n_seg` sets each cylinder's circumferential tessellation. Returns one
[`TriSoup`](crate::shapes::TriSoup) per pin.

```rust
pub fn pin_lattice(counts: [usize; 2], pitch: f64, radius: f64, height: f64, n_seg: usize) -> Vec<crate::shapes::TriSoup> { /* ... */ }
```

#### Function `bounding_domain`

An axis-aligned box surface wrapping all `soups` with a uniform `margin` —
the coolant domain enclosing a packing.

# Panics

If `soups` is empty or contains an empty surface.

```rust
pub fn bounding_domain(soups: &[crate::shapes::TriSoup], margin: f64) -> crate::shapes::TriSoup { /* ... */ }
```

## Module `shapes`

Closed **triangle-soup surface generators** for test and reactor geometry.

The carver ([`crate::carve`]) takes a triangle soup (`points` + `tris`); this
module produces watertight, **outward-wound** soups for the common shapes a
reactor model is built from — a domain [`box_surface`], a spherical
[`sphere_surface`] (a pebble / particle), and a [`cylinder_surface`] (a fuel
pin / channel) — with no dependency on the surface-authoring crate. Combined
with [`crate::carve::carve_region`] they build "coolant around a pebble" and
similar geometries directly.

[`surface_volume`] returns the enclosed volume of any outward-wound soup (the
divergence formula), used to sanity-check both these generators and any
imported surface. Pure Rust, no dependencies.

```rust
pub mod shapes { /* ... */ }
```

### Types

#### Type Alias `TriSoup`

A triangle-soup surface: point positions and triangle vertex-index triples.

```rust
pub type TriSoup = (Vec<crate::math::Vec3>, Vec<[usize; 3]>);
```

### Functions

#### Function `surface_volume`

Enclosed volume of an outward-wound closed triangle soup,
`V = (1/6) Σ v0 · (v1 × v2)` (the divergence theorem). Positive for outward
winding; its sign also reveals an inward-wound surface.

```rust
pub fn surface_volume(points: &[crate::math::Vec3], tris: &[[usize; 3]]) -> f64 { /* ... */ }
```

#### Function `box_surface`

Axis-aligned box `[min, max]` as 8 corners + 12 outward triangles.

```rust
pub fn box_surface(min: crate::math::Vec3, max: crate::math::Vec3) -> TriSoup { /* ... */ }
```

#### Function `sphere_surface`

UV sphere of `radius` about `centre` — `n_lat` latitude bands (pole to pole),
`n_lon` longitude segments. Outward-wound; a triangulated pebble/particle.

# Panics

If `n_lat < 2` or `n_lon < 3`.

```rust
pub fn sphere_surface(centre: crate::math::Vec3, radius: f64, n_lat: usize, n_lon: usize) -> TriSoup { /* ... */ }
```

#### Function `cylinder_surface`

Z-axis cylinder of `radius` and `height` with its base centred at `base` —
`n_seg` circumferential segments, capped both ends. Outward-wound; a
triangulated fuel pin / channel.

# Panics

If `n_seg < 3`.

```rust
pub fn cylinder_surface(base: crate::math::Vec3, radius: f64, height: f64, n_seg: usize) -> TriSoup { /* ... */ }
```

## Module `smooth`

Mesh-quality **smoothing** — smart Laplacian relaxation of interior vertices.

This is the first, robust increment of the tet-quality-refinement roadmap
(`op-38z`, follow-up to the tet foundation). A raw centroid-subdivision tet
mesh (or any generated mesh) can carry poorly-shaped cells near a snapped or
graded boundary; **Laplacian smoothing** improves cell shape by moving each
*free* (non-boundary) vertex toward the centroid of its edge-connected
neighbours.

# "Smart" — never inverts a cell

Plain Laplacian smoothing can turn a cell inside-out in concave regions. This
is the **smart** variant (Freitag): a vertex only moves if *every* cell
incident to it keeps a positive volume after the move — otherwise it stays
put. So the operation can only maintain or improve validity: no cell is ever
inverted, and [`VolumeMesh::validate`] stays `Ok`.

# Guarantees

- **Boundary preserved.** Only interior vertices move; every boundary vertex
  (any vertex used by a boundary face) is pinned. The boundary faces are
  therefore unchanged, so the **total volume is conserved exactly** (the
  divergence-theorem volume depends only on the boundary).
- **Topology preserved.** Points move; faces / owner / neighbour / patches
  are untouched. This is smoothing, not remeshing.
- **No inversion.** By the smart-acceptance rule above.

Sequential (Gauss–Seidel) sweeps: each vertex is relaxed against the
already-updated positions, which converges faster than a Jacobi sweep and
keeps the no-inversion invariant exact. Pure Rust, Android-safe.

# Scope

Smoothing improves *shape*; it does not change connectivity. Flip-based
Delaunay optimisation and size-driven point insertion (the rest of `op-38z`)
are separate, later increments.

```rust
pub mod smooth { /* ... */ }
```

### Functions

#### Function `laplacian_smooth`

Smart-Laplacian-smooth `mesh` for `passes` sweeps, returning the relaxed
mesh. Interior vertices move toward their neighbour centroid only when the
move inverts no incident cell; boundary vertices are pinned. Topology and
total volume are preserved. See the module docs for the guarantees.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, tet::tetrahedralize, smooth::laplacian_smooth};

let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
let tets = tetrahedralize(&carve_box(&p, &t, 0.5));
let smoothed = laplacian_smooth(&tets, 5);

// Boundary is pinned, so the volume is conserved exactly, and no cell inverts.
assert!((smoothed.total_volume() - tets.total_volume()).abs() < 1e-9);
assert!(smoothed.validate().is_ok());
```

```rust
pub fn laplacian_smooth(mesh: &crate::volume_mesh::VolumeMesh, passes: usize) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `snap`

Boundary **snapping** — pull the carved staircase boundary onto the surface.

[`crate::carve::carve_box`] produces a voxelised *staircase* boundary. This
module performs the snap step of cfMesh's `cartesianMesh` (and
snappyHexMesh's *snap*): every mesh point that lies on a boundary face is
moved to its closest point on the input surface, turning the staircase into
a body-fitted boundary. Interior points are left untouched.

# v1 scope

Direct closest-point projection — no feature-edge/corner detection and no
anti-inversion relaxation yet (cfMesh does iterative, constrained snapping).
For a boundary already within roughly one cell of a smooth, convex-ish
surface this is well-behaved; a point projected across a thin feature could
distort or invert its cell, which a later milestone addresses. Pure Rust,
Android-safe.

```rust
pub mod snap { /* ... */ }
```

### Functions

#### Function `snap_to_surface`

Return a copy of `mesh` with every **boundary** point projected onto the
closest point of the surface (`points`, triangle indices `tris`).

A point is a boundary point if any boundary face (one with no neighbour)
references it. Interior points keep their position. The topology is
unchanged — only boundary-point coordinates move — so cell closure is
preserved.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, carve::carve_box, snap::snap_to_surface};

// Snapping a grid-aligned box carve is a no-op: its boundary points already
// lie on the box surface, so the volume is unchanged.
let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
let carved = carve_box(&p, &t, 0.5);
let snapped = snap_to_surface(&carved, &p, &t);
assert!((snapped.total_volume() - carved.total_volume()).abs() < 1e-9);
# fn box_surface(a: Vec3, b: Vec3) -> (Vec<Vec3>, Vec<[usize; 3]>) {
#     let v = vec![
#         Vec3::new(a.x, a.y, a.z), Vec3::new(b.x, a.y, a.z), Vec3::new(b.x, b.y, a.z), Vec3::new(a.x, b.y, a.z),
#         Vec3::new(a.x, a.y, b.z), Vec3::new(b.x, a.y, b.z), Vec3::new(b.x, b.y, b.z), Vec3::new(a.x, b.y, b.z)];
#     let q = |a:usize,b:usize,c:usize,d:usize| vec![[a,b,c],[a,c,d]];
#     let mut t = Vec::new();
#     for f in [q(0,3,2,1), q(4,5,6,7), q(0,1,5,4), q(2,3,7,6), q(1,2,6,5), q(0,4,7,3)] { t.extend(f); }
#     (v, t)
# }
```

```rust
pub fn snap_to_surface(mesh: &crate::volume_mesh::VolumeMesh, points: &[crate::math::Vec3], tris: &[[usize; 3]]) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `tet`

**Tetrahedralization** — split every cell of a volume mesh into tetrahedra
(the all-tet foundation for the polyhedral-dual and boundary-layer paths).

# Approach: centroid subdivision (not from-scratch Delaunay)

Per the roadmap decision (`op-hzs.32`), this does **not** build a from-scratch
constrained-Delaunay mesher (TetGen is AGPL and off-limits). Instead it
follows the cfMesh-style route: take the crate's Cartesian/carved cell mesh
and decompose each cell into tets by **centroid subdivision** — the standard,
robust "polyhedron → tets" tiling:

- add the **cell centroid** `gC` and, for every face, its **face centroid**
  `gF`;
- for each face `f` of the cell and each edge `(a, b)` of `f`, emit the
  tetrahedron `{a, b, gF, gC}`.

A hex (6 faces × 4 edges) becomes 24 tets; a general polyhedral cell becomes
`Σ_faces (edges of face)` tets. The union tiles the cell exactly, so the tet
mesh conserves volume and its boundary is a triangulation of the input
surface (each boundary polygon fanned through its centroid).

# Why it stays a valid FV mesh

**Face centroids are shared globally** (one per primal face, keyed by face
index), so the two cells across an internal primal face emit the *same*
boundary triangle `{a, b, gF}` and [`from_cell_faces`] matches them into an
internal face; a primal boundary face's triangles stay boundary. Cell
centroids are per-cell, so the radial faces (`{a, b, gC}`, `{a, gF, gC}`,
`{b, gF, gC}`) match only among a cell's own tets. Winding and
owner/neighbour are recovered by [`from_cell_faces`].

# Scope & limitations

This is a *space-filling* tetrahedralization, **not** a quality
(Delaunay-refined) one: it guarantees positive-volume tets for convex /
star-shaped cells (the Cartesian/carved cells this crate produces) and exact
volume conservation, but it does not optimise tet shape or minimise count.
Delaunay-quality refinement is future work (gmsh — GPLv2+, GPLv3-compatible —
is a licence-clean reference for that). Pure Rust, Android-safe.

# Honest V&V scope — the tet quality is UNMEASURED

The two tests in this module assert exactly four things: the tet count
(`Σ_cells Σ_faces edges`), **positive volume** (no inverted tets), the
**boundary area** matching the input surface, and **exact volume
conservation** — plus that every cell really is a 4-triangle tet. They call
[`crate::checks::check_quality`] only for its `n_negative_volume_cells` /
`min_cell_volume` fields.

**No mesh-quality metric is gated here.** Neither non-orthogonality,
skewness, nor aspect ratio of the produced tets is asserted anywhere, so the
*shape* quality of this tet primal is unverified — "valid" here means
"positively oriented and watertight", not "well-shaped".

This matters because the tet primal is the **suspected dominant source of
the non-orthogonality** reported downstream. Centroid subdivision emits, per
face edge, a tet spanning a face edge, the face centroid and the cell
centroid — a systematically sliver-prone shape.
[`crate::delaunay`] already records the defect qualitatively: the centroid
subdivision "produces a valid, space-filling tet mesh, but not a *Delaunay*
one — some interior faces fail the empty-circumsphere (locally-Delaunay)
test, which is what leaves slivers". The whole-pipeline sphere table in
[`crate::pipeline`]'s tests measures max non-orthogonality in the 71-87 deg
band, but nothing isolates how much of that the tet stage contributes.

**This attribution is a hypothesis, not a measurement.** Confirming it needs
a per-stage quality measurement (run [`crate::checks::check_quality`] on the
mesh immediately after this stage and compare against the carve that fed it)
— which does not exist yet. Do not cite the slivers as a proven cause.

```rust
pub mod tet { /* ... */ }
```

### Functions

#### Function `tetrahedralize`

Tetrahedralize `mesh` by centroid subdivision: every cell is split into
`Σ_faces (edges of face)` tetrahedra. Returns an all-tet [`VolumeMesh`] that
conserves the domain volume and whose boundary triangulates the input
surface. See the module docs for the construction and its guarantees.

# Examples

```
use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface, carve::carve_box, tet::tetrahedralize};

// Carve a unit box into 2×2×2 hexes, then tetrahedralize: 8 hexes × 24 =
// 192 positive-volume tets that tile the same volume.
let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
let hex = carve_box(&p, &t, 0.5);
let tets = tetrahedralize(&hex);

assert_eq!(tets.cell_count(), 8 * 24);
assert!((tets.total_volume() - hex.total_volume()).abs() < 1e-9);
assert!(tets.validate().is_ok());
```

```rust
pub fn tetrahedralize(mesh: &crate::volume_mesh::VolumeMesh) -> crate::volume_mesh::VolumeMesh { /* ... */ }
```

## Module `volume_mesh`

The core **volume mesh** data structure — the Rust analogue of cfMesh's
`polyMeshGen` and OpenFOAM's `polyMesh`.

A [`VolumeMesh`] is a *finite-volume* mesh: a set of points, a set of
polygonal **faces** (each an ordered list of point indices), and, for every
face, the **owner** cell and an optional **neighbour** cell. A face with a
neighbour is *internal* (it separates two cells); a face without one is a
*boundary* face belonging to a [`BoundaryPatch`]. Cells are implicit — the
set of faces that reference a given cell index.

This is exactly the connectivity OpenFOAM's `constant/polyMesh` stores
(`points` / `faces` / `owner` / `neighbour` / `boundary`), and exactly what
`outram-foam-basic-lib`'s `io::poly_mesh::PolyMesh` consumes — so this type
is the generator's output substrate, deliberately shaped to bridge to the
solver with no restructuring.

# Conventions

- A face's geometric normal (its area vector) points **from its owner toward
  its neighbour**; on a boundary face it points **out of the domain** (the
  owner is inside). Meshers in this crate build faces to satisfy this.
- Faces are ordered **internal first**, then boundary faces grouped by
  patch, so [`VolumeMesh::n_internal_faces`] is a prefix count — the OpenFOAM
  ordering rule.

Index-based throughout (newtype-free `usize` indices into `Vec`s), no
lifetimes, no trait objects — per the workspace rules.

```rust
pub mod volume_mesh { /* ... */ }
```

### Types

#### Struct `BoundaryPatch`

A named boundary patch: a contiguous run of boundary faces.

```rust
pub struct BoundaryPatch {
    pub name: String,
    pub start_face: usize,
    pub n_faces: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Patch name (e.g. `"xMin"`, `"walls"`). |
| `start_face` | `usize` | Index of the first face of this patch in [`VolumeMesh::faces`]. |
| `n_faces` | `usize` | Number of faces in this patch. |

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
    fn clone(self: &Self) -> BoundaryPatch { /* ... */ }
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
    fn eq(self: &Self, other: &BoundaryPatch) -> bool { /* ... */ }
    ```

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
#### Struct `VolumeMesh`

A finite-volume mesh: points, faces, per-face owner/neighbour, and boundary
patches. See the module docs for the conventions this type guarantees.

```rust
pub struct VolumeMesh {
    pub points: Vec<crate::math::Vec3>,
    pub faces: Vec<Vec<usize>>,
    pub owner: Vec<usize>,
    pub neighbour: Vec<Option<usize>>,
    pub n_cells: usize,
    pub patches: Vec<BoundaryPatch>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<crate::math::Vec3>` | Vertex positions, indexed by the entries of [`VolumeMesh::faces`]. |
| `faces` | `Vec<Vec<usize>>` | Faces, each an ordered ring of point indices (outward/owner→neighbour<br>wound, see module docs). |
| `owner` | `Vec<usize>` | `owner[f]` — the cell that owns face `f`. |
| `neighbour` | `Vec<Option<usize>>` | `neighbour[f]` — the cell across face `f`, or `None` for a boundary face. |
| `n_cells` | `usize` | Number of cells. |
| `patches` | `Vec<BoundaryPatch>` | Boundary patches (each a contiguous run of boundary faces). |

##### Implementations

###### Methods

- ```rust
  pub fn point_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of points.

- ```rust
  pub fn face_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of faces (internal + boundary).

- ```rust
  pub fn cell_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of cells.

- ```rust
  pub fn n_internal_faces(self: &Self) -> usize { /* ... */ }
  ```
  Number of internal faces (those with a neighbour).

- ```rust
  pub fn n_boundary_faces(self: &Self) -> usize { /* ... */ }
  ```
  Number of boundary faces (those without a neighbour).

- ```rust
  pub fn face_area_vector(self: &Self, f: usize) -> Vec3 { /* ... */ }
  ```
  The area vector of face `f` (Newell's method): magnitude = face area,

- ```rust
  pub fn face_centroid(self: &Self, f: usize) -> Vec3 { /* ... */ }
  ```
  The centroid (vertex average) of face `f`.

- ```rust
  pub fn total_volume(self: &Self) -> f64 { /* ... */ }
  ```
  Total enclosed volume of the domain, via the divergence theorem over the

- ```rust
  pub fn validate(self: &Self) -> Result<(), String> { /* ... */ }
  ```
  Check structural validity: owner/neighbour cell indices in range, and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> VolumeMesh { /* ... */ }
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

#### Function `cells_faces`

Invert the mesh to a per-cell list of **outward-wound** face rings: each
cell gets its owner faces as stored (owner→neighbour = outward from the
owner) and its neighbour faces reversed (outward from that cell). The
inverse of [`from_cell_faces`].

```rust
pub fn cells_faces(mesh: &VolumeMesh) -> Vec<Vec<Vec<usize>>> { /* ... */ }
```

#### Function `from_cell_faces`

Assemble a [`VolumeMesh`] from `points` and a per-cell list of
**outward-wound** face rings. Faces shared by two cells (matched by their
vertex *set*) become internal faces (owner = the cell that listed it first);
unmatched faces are boundary faces in a single `walls` patch. Faces are
ordered internal-first.

This is the general "cells → connectivity" assembler used by mesh surgery
such as boundary-layer insertion, where owner/neighbour are easiest to
recover by matching shared faces rather than tracked directly.

```rust
pub fn from_cell_faces(points: Vec<crate::math::Vec3>, cells: &[Vec<Vec<usize>>]) -> VolumeMesh { /* ... */ }
```

