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

The workspace already had the mesh **representation** and finite-volume
addressing (`outram-foam-basic-lib`'s `FvMesh` / `PolyMesh`, with
`read`/`write`/`to_fv_mesh`) and the surface-**authoring** frontend
(`outram-blender`) — but no unstructured mesh **generation** anywhere (no
blockMesh, snappyHexMesh, tet/Delaunay, polyhedral dual, or boundary layers).
Open-source, pure-Rust, GPLv3-clean tooling for this is genuinely lacking, so
this crate **ports the proven cfMesh workflows** instead of reinventing them
(and deliberately avoids the AGPL-licensed TetGen).

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

Ported files carry the upstream provenance header block (project, source file,
commit, copyright, licence) per the workspace provenance rule; algorithms are
re-implemented in Rust, not transcribed verbatim from C++.

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
cells): `octree::refine_near_boundary` (graded near-wall refinement),
`tet::tetrahedralize` (centroid subdivision to an all-tet mesh),
`delaunay::flip_to_delaunay` (bistellar 2→3 / 3→2 flips with Shewchuk
predicates, improve-or-noop), `dual::polyhedral_dual` /
`dual::polyhedral_dual_min_faces` (`polyDualMesh`-style median dual, robust
quad-fan or face-minimal), `smooth::laplacian_smooth` (smart-Laplacian quality
smoothing), and `layers::add_boundary_layers` / `add_boundary_layers_adaptive`
(graded prism wall layers — flat walls, and curved/polyhedral walls with
per-point thickness limiting).

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

Remaining roadmap (beads under the `op-hzs` epic):

1. `op-hzs.40` — **core `VolumeMesh` + Cartesian block mesher** ✅ (milestone 1)
2. `op-hzs.41` — **castellated surface carve** ✅ (milestone 2)
3. `op-hzs.42` — **boundary snapping** ✅ (milestone 3a)
4. `op-hzs.35` — **volume polyMesh bridge** to `outram-foam-basic-lib` ✅ (milestone 3b)
5. octree refinement near the surface (graded cell sizing) ✅
6. `op-hzs.33` — **polyhedral dual** (`polyDualMesh`-style, voro++ reference) ✅
7. `op-hzs.34` — **wall boundary / prism layers** ✅
8. exact/adaptive predicates + size-driven point insertion (rest of `op-38z`);
   multi-patch / feature-aware layer insertion

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
