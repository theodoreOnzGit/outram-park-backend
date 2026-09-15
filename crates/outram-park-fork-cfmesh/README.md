# outram-park-fork-cfmesh

> **This is a fork.** `outram-park-fork-cfmesh` is an independent, GPL-licensed
> **fork / Rust port of [cfMesh](https://github.com/wyldckat/cfMesh)** (Creative
> Fields' automatic unstructured mesh generator), for the OUTRAM PARK
> multiphysics suite. It is **not** affiliated with, endorsed by, or sanctioned
> by Creative Fields or the cfMesh project, and bundles no cfMesh source in the
> packaged crate. "cfMesh" identifies only the upstream project it derives from.

The **volume-mesh-generation** layer of OUTRAM PARK: it turns a closed surface
mesh into a solvable *volume* mesh with polyhedral cells and near-wall boundary
layers.

```text
  outram-blender  ──►  outram-park-fork-cfmesh  ──►  outram-foam-basic-lib
  (surface Mesh)       (tet / polyhedral cells        (PolyMesh → FvMesh,
                        + boundary layers)              solvable)
```

## Why this crate exists

The workspace has the mesh **representation** and finite-volume addressing
(`outram-foam-basic-lib`'s `FvMesh` / `PolyMesh`, with `read`/`write`/
`to_fv_mesh`) and the surface-**authoring** frontend (`outram-blender`). This
crate supplies the **cfMesh-lineage** automatic unstructured generator between
them: `outram-blender` surface → one call → solvable polyhedral volume mesh with
prism wall layers. Open-source, pure-Rust, GPLv3-clean tooling for that is
genuinely lacking, so this crate **ports the proven cfMesh workflows** instead of
reinventing them (and deliberately avoids the AGPL-licensed TetGen).

### Relationship to `outram-foam-mesh` (they overlap — read this)

`outram-foam-mesh` is a **separate, earlier** crate (added 2026-07-17; this one
2026-07-24) that ports the **OpenFOAM-lineage** mesh utilities: `blockMesh`,
`ideasUnvToFoam`, `snappyHexMesh` castellation, the cell-centre `polyDualMesh`,
and `checkMesh`-style `mesh_quality`. The two crates genuinely overlap — both
turn a surface into a volume mesh, both build a dual, both insert layers — so
pick deliberately:

| | `outram-foam-mesh` | `outram-park-fork-cfmesh` (this crate) |
|---|---|---|
| Lineage | OpenFOAM utilities | cfMesh (+ voro++ reference) |
| Dual | **cell-centre** (`polyDualMesh`: dual vertices at primal cell centroids) | **median / vertex-centred** (Donald) |
| Entry point | per-utility functions mirroring the OpenFOAM binaries | one composed `pipeline::surface_to_tet_dual_mesh` call |
| Input | OpenFOAM dictionaries / UNV / STL-style surfaces | an `outram-blender` surface triangle soup |

**The two duals are different algorithms and their V&V results are not
interchangeable** — see "Honest scope" below.

## Goal

Consume an `outram-blender` `Mesh` (a closed watertight surface), generate a
**tetrahedral** then **polyhedral** volume mesh (à la cfMesh `tetMesh` /
`cartesianMesh` + OpenFOAM `polyDualMesh`), with optional **wall boundary /
prism layers**, and emit a real-cell `outram-foam-basic-lib` `PolyMesh` for the
CFD/TH solvers and `outram-mc-libs` geometry for neutronics — the mesh substrate
for coupled **pebble-bed / molten-salt / light-water reactor** simulations.

## Vendored upstreams (reference only, GPLv3-clean, never shipped)

Both live under `upstream_source/` (gitignored, dev-only — see
[`upstream_source/README.md`](upstream_source/README.md) for the provenance
record and the exact clone commands):

| Upstream | URL | Licence | Role |
|---|---|---|---|
| **cfMesh** | <https://github.com/wyldckat/cfMesh> | **GPL-3.0-only** | Primary port target — `meshLibrary/{cartesianMesh, tetMesh, utilities}` (Cartesian/tet meshing, surface tools, boundary layers) |
| **voro++** | <https://github.com/chr1shr/voro> | modified-BSD (LBNL), GPLv3-compatible | Voronoi / polyhedral-dual (`polyDualMesh`-style) reference |

**Every `src/*.rs` file carries an SPDX identifier and a provenance header
block.** Because this crate re-implements published *algorithms* rather than
transcribing C++, that block names the upstream project, the source directory or
file the construction follows, the copyright holder and the licence — rather
than a per-file upstream commit, which would imply a line-level correspondence
that does not exist. Files that are original OUTRAM PARK work with no upstream
ancestor (`math`, `shapes`, `reactor`, `pipeline`, `patches`) say so explicitly
in the same block.

Non-cfMesh algorithmic sources are credited where they are used, with the
literature citation rather than an implied code lineage — notably
**Shewchuk's `orient3d` / `insphere` predicates** in `delaunay` (J. R. Shewchuk,
*Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric
Predicates*, Discrete & Computational Geometry 18(3):305-363, 1997). Note that
only the determinant *formulations* are taken from that work: the adaptive
exact-arithmetic expansions that make Shewchuk's `predicates.c` robust are **not**
implemented here, so these are plain `f64` evaluations and are not a substitute
for the real predicates on degenerate input.

## Licensing & provenance

- This crate is **GPL-3.0-only** (workspace default). cfMesh is GPL-3.0-only —
  a Rust port is licence-clean provided the provenance header block is kept on
  any ported file. voro++ is modified-BSD (permissive, GPLv3-compatible); its
  provenance is retained in-tree and in [`NOTICE`](NOTICE).
- The vendored clones under `upstream_source/` are **never committed** (see
  `.gitignore`) and **never packaged** (see `Cargo.toml` `exclude`).

## Status

**Milestone 1 — volume-mesh core + Cartesian block mesher (landed).** The
`VolumeMesh` data structure (points + faces + owner/neighbour + boundary
patches, mirroring cfMesh `polyMeshGen` / OpenFOAM `polyMesh`) and
`cartesian::cartesian_box(min, max, [nx,ny,nz])` — a regular hex grid of an
axis-aligned box — are implemented and verified (exact volume, closed cells via
the discrete-Gauss check, outward boundary normals). This is the un-refined
Cartesian background the cfMesh `cartesianMesh` workflow builds on.

**Milestone 2 — castellated surface carve (landed).** `carve::carve_box(points,
tris, cell_size)` overlays a uniform Cartesian grid on a closed triangle-soup
surface and keeps the cells inside it (ray-parity inside test), producing a
body-fitted **staircase** `VolumeMesh` with a `walls` boundary patch. Verified:
a grid-aligned box carves exactly (volume 8, 64 cells); an octahedron carve
converges to its analytic volume (within 5 % at cell size 0.05); every carved
cell is closed. v1 is a staircase boundary — no point snapping yet.

**Milestone 3 — snapping + the foam bridge (landed).** `snap::snap_to_surface`
projects the staircase boundary points onto the surface (body-fitting); and,
behind the `foam-export` feature, `foam::to_poly_mesh` converts a `VolumeMesh`
into a real `outram-foam-basic-lib` `PolyMesh` that yields a solvable `FvMesh`
via `to_fv_mesh()`. The full loop **surface → carve → snap → foam mesh** is
verified end-to-end (foam's own geometry engine agrees on the volume).

**Milestone 4 — the meshing kernel (landed).** The stages that turn a carved,
snapped background mesh into a solvable polyhedral-with-layers mesh are all
implemented and verified (exact/near-analytic volume, closed cells, no inverted
cells): `octree::refine_near_boundary` / `octree::refine_near_boundary_banded`
(graded near-wall refinement, by touching-shell or distance-band criterion; the
assembler splices hanging nodes into coarse face rings so every edge lies in
exactly two faces of its cell, which is what keeps `tetrahedralize` watertight on
a graded mesh),
`tet::tetrahedralize` (centroid subdivision to an all-tet mesh),
`delaunay::flip_to_delaunay` (bistellar 2→3 / 3→2 flips with Shewchuk
predicates, improve-or-noop), `dual::polyhedral_dual` /
`dual::polyhedral_dual_min_faces` (median dual, robust
quad-fan or face-minimal), `smooth::laplacian_smooth` (smart-Laplacian quality
smoothing), and `layers::add_boundary_layers` / `add_boundary_layers_adaptive`
(graded prism wall layers — flat walls, and curved/polyhedral walls with
per-point thickness limiting).

### Honest scope: what "verified" covers, and what is unmeasured

"Verified" above means **topological and volumetric** verification —
positive-volume cells, closed cells, exact volume conservation, boundary area
matching the input surface, expected cell/face counts. It does **not** mean the
cells are well *shaped*. Two specific gaps to know before citing this crate:

- **The dual is the *median* (Donald / vertex-centred) dual** — dual corners at
  edge midpoints and face/cell centroids. It is **not** the circumcentre Voronoi
  dual, and it is **not the same algorithm** as `outram-foam-mesh`'s
  `poly_dual_mesh`, which is the *cell-centre* dual (dual vertices at primal
  cell centroids, one dual face per primal edge). Consequently
  `outram-foam-mesh`'s measured result — *"dualisation does not create
  non-orthogonality; the dual of a uniform hex block measures exactly 0° and 0
  skewness"* — is a statement about **that** algorithm and **must not be
  transferred to this one**. No test in `dual` calls `checks::check_quality`, so
  this dual's orthogonality and skewness are **unmeasured**.
- **The tet primal's quality is unmeasured**, and is the *suspected* dominant
  source of the non-orthogonality reported downstream. `tet::tetrahedralize` is
  centroid subdivision; its two tests assert only tet count, positive volume,
  boundary area and exact volume — no shape metric. `delaunay`'s module docs
  already record the defect qualitatively (the subdivision "produces a valid,
  space-filling tet mesh, but not a *Delaunay* one — some interior faces fail the
  empty-circumsphere test, which is what leaves slivers"). Attributing the
  pipeline's 71–87° non-orthogonality to this stage is a **hypothesis**: nothing
  measures quality per stage yet, so do not cite the slivers as a proven cause.

### High-level pipeline (the recommended entry point)

`pipeline::surface_to_tet_dual_mesh(points, tris, &opts)` composes the whole
kernel into **one call** via the **tet → dual** path — tetrahedralize the carved
interior, take its polyhedral dual, then grow adaptive prism boundary layers:

```text
surface → carve_box → snap_to_surface → tetrahedralize → flip_to_delaunay
  → polyhedral_dual(_min_faces) → laplacian_smooth → add_boundary_layers_adaptive
```

It is tuned by `pipeline::TetDualOptions` (all lengths in **metres**: background
`cell_size`, `first_layer_thickness`; dimensionless `expansion ≥ 1`; per-stage
on/off toggles) and returns a `pipeline::TetDualReport` (cell count, total volume
in m³, checkMesh-style quality in degrees, and a note per skipped stage).

Each optional stage is applied **only if its result stays valid** — closed *and*
free of negative-volume (inverted) cells — otherwise it is **gracefully skipped**,
the previous mesh is kept, and a human-readable line is appended to
`TetDualReport::stage_notes`. The returned mesh is therefore always valid and
exportable, so a caller never has to unwind a broken partial mesh. This is the
coarse-grained entry point the **`outram-blender` Mesh Studio GUI** calls (in
place of hand-wiring the individual stages) to author a surface and generate a
polyhedral-with-layers volume mesh; the `box_tet_dual` / `sphere_tet_dual` /
`cylinder_tet_dual` wrappers mesh the built-in primitives directly.

#### Octree grading (opt-in): the same wall resolution for far fewer cells

Stage 1 can be **octree-graded** instead of uniform, via
`TetDualOptions::refinement_levels` (default `0` = the uniform carve, byte-for-byte
the original behaviour) and `refinement_band`. The interior stays at `cell_size`
and only the near-wall band is refined, so a given wall resolution costs far fewer
cells. Measured on a radius-3 m sphere (24 × 48 UV, analytic volume 113.0973 m³),
no boundary layers, everything else identical:

| stage-1 mesher | cells | volume (m³) | volume error | max non-orth (deg) | max skewness | run by a test? |
|---|---|---|---|---|---|---|
| uniform 0.60 m | 3271 | 110.6614 | 2.154 % | 71.71 | 0.267 | no — doc-recorded only |
| uniform 0.50 m | 5269 | 111.1537 | 1.719 % | 72.46 | 0.248 | **yes** — baseline arm |
| uniform 0.40 m | 42216 | 111.3923 | 1.508 % | 85.14 | 0.924 | no — doc-recorded only |
| uniform 0.30 m | 101376 | 111.8890 | 1.068 % | 85.38 | 2.435 | no — doc-recorded only |
| graded 1.20 m, 1 level | 3263 | 110.8886 | 1.953 % | 86.78 | 0.507 | no — doc-recorded only |
| graded 1.00 m, 1 level | 1381 | 111.2254 | 1.655 % | 83.80 | 0.227 | **yes** — graded arm |
| graded 0.80 m, 1 level | 1890 | 111.3568 | 1.539 % | 84.22 | 0.260 | no — doc-recorded only |
| graded 0.60 m, 1 level | 3921 | 111.8931 | 1.065 % | 85.58 | 0.300 | no — doc-recorded only |

**Only two of the eight rows are run by a test** (`pipeline`'s
`octree_grading_beats_the_uniform_carve_on_cells_per_accuracy`); the other six
were measured by hand on 2026-08-07 and are recorded as evidence, not as a gate.
Even for the two gated rows the assertions are **relative**, not value-for-value:
both meshes closed with no inverted cells, graded volume error ≤ uniform, graded
cell count × 2 < uniform, and `max non-orth < 90°` — a degeneracy floor that
would pass at 89.9°, so it does not pin the tabulated 72.46 / 83.80. **No
skewness figure is asserted anywhere**, so that whole column — including the
2.435 outlier — is doc-recorded only.

The graded family dominates the uniform family on the cells-versus-accuracy
curve: **1381 cells at 1.66 % against 5269 at 1.72 %**, and at the fine end
**3921 cells at 1.065 % against 101376 at 1.068 % — 25.9× fewer cells for the
same accuracy**, with far better skewness. That second, headline pair rests on
**two doc-recorded rows** (the uniform 0.30 m run costs ~13 s), so it is the
weaker of the two claims. Caveat: the two families degrade through *different*
stages on this geometry (the graded runs skip tetrahedralization, the fine
uniform runs skip the dual), so the outputs are both valid closed meshes but not
the same cell type — which also means the non-orthogonality and skewness columns
are not comparing like with like down the table. This is **verification** —
volume against the analytic sphere — **not** validation against a solve. See
`pipeline`'s test for the full methodology.

#### Named boundary patches (`inlet` / `outlet` / `walls`)

`pipeline::surface_to_tet_dual_mesh_multipatch(points, tris, &regions, &opts)`
carries an input surface's **named regions** (`patches::SurfaceRegions`) through
to the output mesh's boundary patches, so a solver `0/` boundary-condition
directory can actually be written against the result. Every stage after the carve
rebuilds the mesh through `from_cell_faces` (which matches faces by vertex set
and cannot preserve a tag) and the layer stage *creates* boundary faces, so the
assignment is instead recovered **geometrically at the end** by
`patches::assign_patches_by_region` — each boundary face takes the region of the
nearest input triangle, as snappyHexMesh does. Verified through the full pipeline
including prism layers: contiguous patches with correct counts and start offsets,
and geometrically correct membership. The single-patch
`surface_to_tet_dual_mesh` is unchanged.

Known limitation: because classification runs last, prism layers are grown over
the **whole** boundary, `inlet`/`outlet` included — snappyHexMesh's per-patch
`nSurfaceLayers` selection is future work.

Remaining roadmap (beads under the `op-hzs` epic):

1. `op-hzs.40` — **core `VolumeMesh` + Cartesian block mesher** ✅ (milestone 1)
2. `op-hzs.41` — **castellated surface carve** ✅ (milestone 2)
3. `op-hzs.42` — **boundary snapping** ✅ (milestone 3a)
4. `op-hzs.35` — **volume polyMesh bridge** to `outram-foam-basic-lib` ✅ (milestone 3b)
5. octree refinement near the surface (graded cell sizing) ✅
6. `op-hzs.33` — **polyhedral dual** (`polyDualMesh`-style, voro++ reference) ✅
7. `op-hzs.34` — **wall boundary / prism layers** ✅
8. octree grading wired into the high-level pipeline (`refinement_levels`) ✅
9. named boundary patches carried through to the output polyMesh
   (`surface_to_tet_dual_mesh_multipatch`) ✅
10. exact/adaptive predicates + size-driven point insertion (rest of `op-38z`)
11. **patch-selective** layer insertion (layers currently grow over the whole
    boundary, because patch classification runs last) and feature-aware layers
12. make the adaptive layerer work on graded near-wall meshes — it currently
    backs off to zero thickness there, which the pipeline now *reports* in
    `TetDualReport::stage_notes` rather than failing silently
13. feature-edge-aware snapping, so patch seams follow surface feature edges
    rather than mesh face edges

## Design rules (workspace `CLAUDE.md`)

- **Index-based topology** — newtype indices into `Vec`, no lifetimes/pointers.
- **Enums for dispatch, never trait objects.**
- **No `Box<T>`; `Arc<T>` for sharing.**
- **Pure Rust, Android/Termux-buildable** — no BLAS/C/Fortran.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
