# outram-park-fork-cfmesh

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. For research, education and V&V only. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

> **This is a fork.** `outram-park-fork-cfmesh` is an independent
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

**Name.** `outram-park` is the workspace name, OUTRAM PARK: **O**pen-source
**U**nified **TR**Ansient **M**ulti-Physics **A**dvanced **R**eactor simulation
**K**it (`docs/ecosystem-naming.md`); `fork-cfmesh` marks a fork of cfMesh, the
workspace's convention for forks. **The question it answers:** how does an
automatic mesher turn a closed surface into a volume mesh with wall layers,
and does a Rust port of cfMesh and TetGen produce the same mesh as the
originals?

**What this crate lets you study and challenge.** Automatic meshing is
usually trusted because nobody can read it. Here cfMesh's four workflows
(`cartesianMesh`, `tetMesh`, `pMesh`, `cartesian2DMesh`) and Hang Si's
TetGen 1.6.0 are transcribed routine by routine and run beside the upstream
binaries, stage by stage, with the bar set at bit identity; the records say
where that was reached, where it was not (one `pow` ulp in asmoOctree's
`meshOptimisation`) and why. They also keep what the port taught about the
originals: that an OpenFOAM `HashSet`'s iteration order can move a curvature
by O(1), that a C++ default argument silently binds a `bool`, that TetGen's
`-q` leaves tetrahedra above its own ratio near sharp corners. The
workspace's own tet → dual → layers pipeline sits beside these ports and is
labelled as original work.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream code (provenance in [`upstream_source/README.md`](upstream_source/README.md)): cfMesh `wyldckat/cfMesh` `6cf1d211f9cf` (GPL-3.0-only) and `DAFoam/cfmesh` `e944f6ba695f` (cfMesh v1.1, GPL-3.0-or-later, the differential target), TetGen 1.6.0 (`libigl/tetgen` `e05aca7df74e`, AGPL-3.0-or-later), OpenFOAM-dev `852527377c7e` (`polyDualMesh`, `snappyLayerDriver`), OpenFOAM ESI v2506 (the build cfMesh runs on) and voro++ (modified BSD). Shewchuk (1997), *Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates*, for the predicates. No `crates/kovan-literature/CATALOGUE.md` entry is tied to this crate. |
| Mathematical equations | Module docs: exact orientation and in-sphere predicates (`tetgen::predicates`), Bowyer-Watson insertion and bistellar flips (`tetgen::delaunay`, `tetgen::flip`, `delaunay`), radius-edge quality and Delaunay refinement (`tetgen::refine`), the median and cell-centre duals (`dual`, `cell_centre_dual`), prism layer thickness and grading (`layers`), cfMesh's mesh-quality metrics and optimisers (`checks`, `mesh_optimizer`, `surface_optimizer`). |
| Source code | cfMesh ports: `mesh_octree*`, `cartesian_mesh`, `tet_creator_octree`, `tet_mesh_extractor_octree`, `voronoi_mesh_extractor`, `surface_engine`, `surface_mapper(_2d)`, `edge_extractor`, `boundary_layer_optimisation`, `mesh_optimizer`, `surface_optimizer`, `final_mesh`, `renumber_mesh`, `mesh_dict`, `tri_surf_io`, `surface`, `pipeline::{cartesian_mesh, tet_mesh, p_mesh, cartesian_2d_mesh}`; TetGen port: `tetgen`; original work: `pipeline` (tet → dual → layers, `multi_region`, `region_join`), `carve`, `snap`, `octree`, `tet`, `dual`, `layers`, `math`, `shapes`, `reactor`, `patches`. |
| Interactive simulations | No app in this crate. Mesh Studio (`crates/dhoby-ghaut/examples/mesh_studio/`) and Foam Studio (`foam_studio_web`) run its pipeline; `outram-foam-cli` exposes the four workflows as `cartesianMesh`, `tetMesh`, `pMesh`, `cartesian2DMesh`. |
| Visualisations | 129 committed images (PNG/SVG) under `verification_and_validation/`, drawn from the assembled meshes. |
| Verification and validation evidence | [`verification_and_validation/`](verification_and_validation/), one folder per issue. As recorded in `AGENTS.md`: TetGen stage 1, 84 000 / 84 000 predicate values bit-identical (`tetgen_predicates_gh1790`); stage 2, 12 / 12 cases identical to `tetgen -zQ` (`tetgen_delaunay_gh1791`); stage 3, 13 / 13 (`tetgen_meshsurface_gh1792`); stage 4, 38 / 38 runs (`tetgen_recoverboundary_gh1793`); stage 5, 192 / 192 output files byte-identical (`tetgen_plc_gh1794`); stage 6, 164 / 164 files (`tetgen_quality_gh1795`); `-m`, 164 / 164 files (`tetgen_sizing_gh1825`), all 2026-11-26. cfMesh (`upstream_cfmesh_gh1385`, 2026-11-11): all seven steps and the final meshes of `cartesianMesh`, `tetMesh` and `pMesh` bit-identical on the sphere; asmoOctree bit-identical through `boundaryLayerGeneration`, `meshOptimisation` 71649/71650 (one `pow` ulp). Further records: `cartesian_mesh_gh1274`, `tet_mesh_gh1272`, `p_mesh_gh1273`, `cartesian_2d_mesh_gh1275`, `case_reading_gh1476`, `patch_types_gh1483.md`, and others in the folder. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp (no `kind = "review"` block in any `review.md` here). No entry in `docs/human-corrections-to-ai-work.md` names this crate. The maintainer accepted the moved anisotropic fingerprint pins (gh:#1336, 2026-11-12, recorded in `AGENTS.md`) and set the layers-first HARD RULE (gh:#1829); struck-through corrections in this README record TetGen's admission (AGPL) and patch-selective layers. |

**Status:** the TetGen stages and the cfMesh workflows above have AI-run
bit-for-bit evidence against the upstream binaries on the recorded cases; the
original tet → dual → layers pipeline has property checks (closure, volume,
no inverted cells) but no upstream to compare with. Nothing here has been
reviewed by a human or validated against a flow solution.

## Why this crate exists

The workspace has the mesh **representation** and finite-volume addressing
(`outram-foam-basic-lib`'s `FvMesh` / `PolyMesh`, with `read`/`write`/
`to_fv_mesh`) and the surface-**authoring** frontend (`outram-blender`). This
crate supplies the **cfMesh-lineage** automatic unstructured generator between
them: `outram-blender` surface → one call → solvable polyhedral volume mesh with
prism wall layers. Open-source, pure-Rust, GPLv3-clean tooling for that is
genuinely lacking, so this crate **ports the proven cfMesh workflows** instead of
reinventing them (~~and deliberately avoids the AGPL-licensed TetGen~~
**CORRECTED 2026-11-26:** AGPL is acceptable since the maintainer's
2026-10-20 direction, and TetGen 1.6.0 is being ported as the `tetgen`
module, `src/tetgen/`, epic gh:#1789; stage 1, the exact predicates, is in,
bit-identical to upstream, gh:#1790).

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
(Not re-checked as a present capability, 2026-11-27: no `outram-mc-libs`
export is in `src/` (grep finds only this sentence in `src/lib.rs`); Monte
Carlo meshes reach `outram-mc-libs` through `outram-blender`'s
`unstructured` mesh.)

## Vendored upstreams (reference only, GPLv3-clean, never shipped)

Both live under `upstream_source/` (gitignored, dev-only — see
[`upstream_source/README.md`](upstream_source/README.md) for the provenance
record and the exact clone commands):

| Upstream | URL | Licence | Role |
|---|---|---|---|
| **cfMesh** | <https://github.com/wyldckat/cfMesh> | **GPL-3.0-only** | Primary port target — `meshLibrary/{cartesianMesh, tetMesh, utilities}` (Cartesian/tet meshing, surface tools, boundary layers) |
| **voro++** | <https://github.com/chr1shr/voro> | modified-BSD (LBNL), GPLv3-compatible | Voronoi / polyhedral-dual (`polyDualMesh`-style) reference |

**CORRECTED 2026-11-27** — the table lists two upstreams; `upstream_source/README.md`
and `AGENTS.md` record more: cfMesh `DAFoam/cfmesh` `e944f6ba695f` (v1.1,
GPL-3.0-or-later; the target of the differential ports, built and run against
OpenFOAM ESI v2506), TetGen 1.6.0 (`libigl/tetgen` `e05aca7df74e`,
AGPL-3.0-or-later; `src/tetgen/`) and OpenFOAM-dev `852527377c7e`.

**Every `src/*.rs` file carries an SPDX identifier and a provenance header
block.** ~~Because this crate re-implements published *algorithms* rather than
transcribing C++, that block names the upstream project, the source directory or
file the construction follows, the copyright holder and the licence — rather
than a per-file upstream commit, which would imply a line-level correspondence
that does not exist.~~ **CORRECTED 2026-11-27** — that holds for the early
modules (`carve`, `snap`, `octree`, `tet`, `dual`, `layers`, ...). The cfMesh
workflow modules transcribe `DAFoam/cfmesh` `e944f6ba695f` and the `tetgen`
module transcribes TetGen 1.6.0 line for line, compared bit for bit with
upstream (`AGENTS.md`, "Differential against real upstream output" and "The
`tetgen` module"). Files that are original OUTRAM PARK work with no upstream
ancestor (`math`, `shapes`, `reactor`, `pipeline`, `patches`) say so explicitly
in the same block.

Non-cfMesh algorithmic sources are credited where they are used, with the
literature citation rather than an implied code lineage — notably
**Shewchuk's `orient3d` / `insphere` predicates** in `delaunay` (J. R. Shewchuk,
*Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric
Predicates*, Discrete & Computational Geometry 18(3):305-363, 1997). Note that
only the determinant *formulations* are taken from that work: the adaptive
exact-arithmetic expansions that make Shewchuk's `predicates.c` robust are **not**
implemented ~~here~~ in `delaunay`, so these are plain `f64` evaluations and are not a substitute
for the real predicates on degenerate input. **CORRECTED 2026-11-27** — the exact
adaptive predicates do exist in the crate, as `tetgen::predicates` (TetGen's
`predicates.cxx`, 84 000 / 84 000 values bit-identical, gh:#1790); `delaunay`
does not use them.

## Licensing & provenance

- This crate is ~~**GPL-3.0-only**~~ **AGPL-3.0-only** combined work (relicensed 2026-10-20, gh:#973; ported files keep GPL-3.0 and `LICENSE.GPL-3.0` ships beside `LICENSE`; see `docs/relicensing-agpl-2026-10-20.md`). cfMesh is GPL-3.0-only —
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
quad-fan or face-minimal; since gh:#1240 also `dual::polyhedral_dual_per_point`,
one boundary face per point per sector as OpenFOAM's `polyDualMesh`, and the
cell-centre dual through `outram-foam-mesh` with the `cell-centre-dual`
feature, chosen by `TetDualOptions::dual_kind`; the default is unchanged), `smooth::laplacian_smooth` (smart-Laplacian quality
smoothing), and `layers::add_boundary_layers` / `add_boundary_layers_adaptive`
(graded prism wall layers — flat walls, and curved/polyhedral walls with
per-point thickness limiting).

**Upstream cases (gh:#1476).** `mesh_dict` reads an upstream `system/meshDict`
(every entry honoured or reported) with `checkMeshDict`'s validation;
`tri_surf_io` reads the `surfaceFile` (`.fms`, `.ftr`, and STL with the
`surface-file` feature); `poly_mesh_gen_io` (`foam-export`) writes
`constant/polyMesh` with subsets and `meshMetaDict`. The executables are
`outram-foam-cli`'s `cartesianMesh`, `tetMesh`, `pMesh` and `cartesian2DMesh`
(`cartesianMesh -case <dir>`). Record:
`verification_and_validation/case_reading_gh1476/`.

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
  this dual's orthogonality and skewness are **unmeasured** at the module
  level. **UPDATED 2026-11-04 (gh:#1240):** the pipeline's differential
  (`pipeline::tests::diagnose_dual_kinds_differential`, ignored, not a gate)
  measures them on the pipe and duct for every dual kind (h 5 mm pipe:
  internal non-orthogonality 16.3° mean, 47.0° max; wall face-to-owner
  median 69° per corner, 26° per point); see the gh:#1240 section of
  `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
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
kernel into **one call** via the **tet → dual** path — ~~tetrahedralize the carved
interior, take its polyhedral dual, then grow adaptive prism boundary layers~~:

```text
surface → carve_box → snap_to_surface → tetrahedralize → flip_to_delaunay
  → polyhedral_dual(_min_faces) → laplacian_smooth → add_boundary_layers_adaptive
```

**CORRECTED 2026-11-27** (gh:#1829, HARD RULE in `AGENTS.md`): the diagram
above is the old layers-last route. The pipeline now grows its prism layers
**first**, inward from TetGen's wall triangulation (`pipeline::layers_first`),
then TetGen (the default `TetDualOptions::tet_source`) meshes the inner region
face to face with the prisms, and the optional dual is taken of both. The
carved source (carve, snap, centroid split, Delaunay flips) remains for bare
meshes only; with `n_layers > 0` it is an error. The octree-grading table
below was measured on the carved route on 2026-08-07.

It is tuned by `pipeline::TetDualOptions` (all lengths in **metres**: background
`cell_size`, `first_layer_thickness`; dimensionless `expansion ≥ 1`; per-stage
on/off toggles) and returns a `pipeline::TetDualReport` (cell count, total volume
in m³, checkMesh-style quality in degrees, and a note per skipped stage).

Each optional stage is applied **only if its result stays valid** — closed *and*
free of negative-volume (inverted) cells — otherwise it is **gracefully skipped**,
the previous mesh is kept, and a human-readable line is appended to
`TetDualReport::stage_notes`. The returned mesh is therefore always valid and
exportable, so a caller never has to unwind a broken partial mesh. This is the
coarse-grained entry point the ~~**`outram-blender` Mesh Studio GUI**~~ **Mesh Studio GUI** (in `dhoby-ghaut` since 2026-10-02, through `outram-blender`'s `foam_mesh`; CORRECTED 2026-11-27) calls (in
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

~~Known limitation: because classification runs last, prism layers are grown over
the **whole** boundary, `inlet`/`outlet` included — snappyHexMesh's per-patch
`nSurfaceLayers` selection is future work.~~ **CORRECTED 2026-11-04
(gh:#1202):** set `TetDualOptions::layer_patches` (e.g. `["walls"]`) and the
boundary is named before the layer stage and prisms grow on those patches only
(`layers::add_boundary_layers_adaptive_on_patches`, a port of cfMesh's
per-patch `boundaryLayers` border rules). Empty keeps the old whole-boundary
behaviour. ~~Still missing: **feature-edge capture** (cfMesh's
`meshSurfaceEdgeExtractor`), so on a pipe the dual's boundary faces wrap round
the rim and the "flat" ends reach about a quarter cell up the wall.~~
**UPDATED 2026-11-04 (gh:#1202):** feature-edge capture is ported
(`surface_mapper`, cfMesh's `meshSurfaceMapper::mapVerticesOntoSurfacePatches`
with edge and corner nodes) and runs in the multipatch pipeline after the dual
(`TetDualOptions::feature_edges`, on by default): the Foam Studio pipe's rim is
square and its inlet spans 0.9936 πR² (was 0.9396). ~~Not ported:
`correctEdgesBetweenPatches`;~~ **CORRECTED 2026-11-05:** `correctEdgesBetweenPatches`
is ported (`correct_edges`, gh:#1241), opt-in through
`TetDualOptions::correct_edges`. Where mapping would leave a face on a feature
edge, those points stay unmapped and the pipeline notes how many. Also since
2026-11-04: `checks::QualityReport::max_skewness` is OpenFOAM's
`faceSkewness` (it divided by the centre distance before), and the layer
back-off applies upstream's layer limits (`maxInternalSkewness 4`, no
non-positive face pyramid, `minDeterminant 0.001`).

What of upstream cfMesh is ported, partly ported or not, and the porting plan
for the rest (gh:#1251): [`docs/upstream_coverage.md`](docs/upstream_coverage.md).

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
10. ~~exact/adaptive predicates + size-driven point insertion (rest of `op-38z`)~~
    **CORRECTED 2026-11-27:** both exist in the `tetgen` port (exact predicates,
    gh:#1790; Delaunay refinement and `-m` point sizes, gh:#1795, gh:#1825), which
    the pipeline uses for its inner region; the carved route's `delaunay` still
    evaluates plain `f64`
11. ~~**patch-selective** layer insertion (layers currently grow over the whole
    boundary, because patch classification runs last) and~~ **patch-selective
    layers done 2026-11-04 (gh:#1202)**; ~~still to do: feature-edge capture~~
    feature-edge capture done 2026-11-04 (without `correctEdgesBetweenPatches`,
    which followed 2026-11-05 in `correct_edges`, gh:#1241);
    ~~still to do: feature-aware layers and **local layer collapse** (three
    layers on the pipe diverge in the solver; the interface to the dual sits
    at 75–84°, and only a local collapse, snappy-style, can meet upstream's
    relaxed `maxNonOrtho 75`)~~ ~~**UPDATED 2026-11-04 (gh:#1215):** both
    upstream repairs are ported beside the crate's own back-off and
    selectable by `TetDualOptions::layer_strategy`
    (`layers::LayerQualityStrategy`): `BackOffOnly` (default, unchanged),
    `CfMeshOptimise` (cfMesh v1.1 `boundaryLayerOptimisation` +
    `meshOptimizer::untangleMeshFV` / `optimizeLowQualityFaces`, modules
    `boundary_layer_optimisation`, `mesh_optimizer`) and `SnappyCollapse`
    (snappyHexMesh's per-point `extrudeMode` and `checkAndUnmark` loop with
    `meshQualityDict`'s limits, modules `snappy_layers`,
    `checks::faces_failing_mesh_quality`); the layer builder takes a
    per-point layer count (`layers::build_layered_variable`).~~
    **CHANGED 2026-11-26 (gh:#1829, maintainer HARD RULE):** the pipeline
    grows its layers **first**, from TetGen's wall triangulation, and TetGen
    meshes the inside face to face (`pipeline::layers_first`); the
    `layer_strategy` option, `snappy_layers` and `pipeline::tet_layers` are
    deleted. The back-off and `boundary_layer_optimisation` stay as library
    functions (the cfMesh workflow ports use the latter). The
    differential results are in
    `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`
    (gh:#1215 section). Also found: the tet-dual pipeline was not
    reproducible between processes (the Delaunay flipper swept a
    `HashMap`); fixed, pinned by
    `pipeline::tests::back_off_only_layers_are_pinned_bit_for_bit`
12. make the adaptive layerer work on graded near-wall meshes — it currently
    backs off to zero thickness there, which the pipeline now *reports* in
    `TetDualReport::stage_notes` rather than failing silently
13. feature-edge-aware snapping, so patch seams follow surface feature edges
    rather than mesh face edges

## `no_std` + `alloc` (gh:#1639)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-park-fork-cfmesh --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). Every generator and stage builds without
it (`cartesianMesh`, `tetMesh`, `pMesh`, `cartesian2DMesh`, the tet-dual
pipeline, layers, the optimisers and the surface tools). What `std` adds:

- file I/O: `surface-file` (STL, which therefore implies `std`), the
  `polyMeshGen` read/write and debug dumps;
- threads: the per-node optimiser map runs on scoped threads with `std`, and
  serially without it (as on wasm), with the same results;
- the wall clock behind the timing profile (`crate::time`: zero without it);
- the diagnostic environment variables (`GH1385_TRACE`, `CFMESH_*`, through
  `crate::env`: unset without it);
- printing, and `std` in `outram-foam-basic-lib` and `outram-foam-mesh`.

Maps are std's with `std`, so every fingerprint pin holds, and `hashbrown`
without it. The root workspace entry is `default-features = false`, so a
member that wants `std` asks for it.

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
