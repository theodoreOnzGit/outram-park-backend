# outram-blender

> **This is a fork.** `outram-blender` is an independent, GPL-licensed
> **fork / derivative of [Blender](https://github.com/blender/blender)'s
> mesh-authoring architecture**, reusing its mesh/geometry design under the GPL
> for the OUTRAM PARK multiphysics suite. It is **not** affiliated with,
> endorsed by, or sanctioned by the Blender Foundation, and bundles no Blender
> source code. "Blender" identifies only the upstream project it derives from.

A pure-Rust, headless **mesh-authoring frontend** for the OUTRAM PARK
multiphysics suite, inspired by the **architecture** of
[Blender](https://github.com/blender/blender). It authors and procedurally
generates geometry, then bridges it into two OUTRAM PARK solver workflows, each
with its own egui studio app:

- **MC Studio** (`--features mc-export`) — author a surface, fit it to an
  `outram-mc-libs` CSG universe, attach materials, and run a k-eigenvalue
  (criticality) Monte Carlo calculation returning `k_eff ± σ`. Backed by the
  `sim` module.
- **Mesh Studio** (`--features foam-mesh`) — hand a closed surface to the
  `outram-park-fork-cfmesh` **tet → dual → boundary-layers** pipeline and export
  an OpenFOAM `polyMesh` for CFD / thermal-hydraulics. Backed by the `foam_mesh`
  module.

The base authoring library pulls in neither solver — both bridges are opt-in
cargo features, so the default build stays light and Android-buildable.

> **GPU compute (always compiled on desktop, off Android): `f32` for speed, CPU
> `f64` is the trusted path.** The headless GPU kernels (`wgpu`) are built
> **unconditionally on every desktop target** — no cargo feature to opt in — so
> the GPU path is used as far as possible; wgpu is target-gated off Android only
> (no system Vulkan/Metal loader there). They accelerate per-vertex work in
> single precision, while the CPU ([`math`]/[`faer`]) path stays the
> deterministic reference. Fallback to CPU is **graceful and automatic**: no
> adapter, or a recoverable GPU error, routes to the CPU path
> (`Affine3::transform_points_best_effort` is the unified try-GPU-then-CPU entry
> point). Same accepted tradeoff as the other OUTRAM PARK GPU paths —
> acceleration in `f32`, trusted result in `f64`.

> **Status: EARLY, but no longer a pure scaffold.** This crate borrows
> Blender's *concepts and data-structure architecture* — the BMesh half-edge
> topology, the mesh-operator model, the modifier stack, geometry-nodes-style
> procedural generation. It is **not** a port of Blender's code (Blender is
> millions of lines of C/C++/Python). The primitive generators, the mesh
> operators (extrude / midpoint-subdivide / vertex-bevel), Catmull-Clark
> subdivision, the modifier stack (mirror / array / subsurf), the procedural
> node evaluator, and the export bridges (OpenFOAM polyMesh text + CSG
> primitive fitting) are real, unit-tested algorithms. The mesh **boolean** now
> does **general union / difference / intersect on non-convex closed meshes**
> (surface arrangement + generalized-winding-number classification, built on the
> robust Shewchuk predicates), with an exact convex-`Intersect` fast path; it is
> verified against analytic CSG volumes (two offset boxes: `∪ = 15`, `∩ = 1`,
> `\ = 7`) and a faceted sphere ∩ box. Coplanar overlapping operand faces are
> rejected honestly (`Unsupported`), not guessed. The CSG export bridge fits a
> box / sphere / Z-cylinder / any convex polyhedron to analytic surfaces and
> falls back to a DAGMC-style faceted solid (winding inside-test) for non-convex
> results, and — behind opt-in cargo features (`foam-export`, `mc-export`) —
> emits the **real** `outram-foam-basic-lib` polyMesh and `outram-mc-libs` CSG
> types, not just local mirrors. The vertex bevel now rounds (a multi-segment
> spherical cap) as well as single-chamfers. A family of **sparse-solve
> geometry-processing operators** (built on `faer` sparse Cholesky) has landed:
> the cotangent/uniform discrete **Laplacian** with implicit and Taubin
> (shrinkage-free) **smoothing**, **harmonic/Tutte parameterization** (UV
> unwrap), and **ARAP** (as-rigid-as-possible) handle-based **deformation**.
> **QEM decimation**, **Loop subdivision**, a robust **3D convex hull**, and a
> **weld / remove-doubles** cleanup pass (merge coincident vertices within a
> tolerance), a **fill-holes** pass (cap open boundary loops into a watertight
> surface), **solidify** (extrude a surface into a closed shell), and
> **recalculate-normals** (repair an inconsistently-wound soup and flip it
> outward), **triangulate** (fan-triangulate every face into a triangle-only
> mesh), **inset** (per-face inset ring), and **bisect** (plane cut / half-space
> clip, pairs with fill-holes) round out the operator set, and **revolve / spin**
> sweeps a profile polyline around an axis into a surface of revolution
> (pipes / vessels / cone frusta). The epic's boolean / export / bevel /
> smoothing / parameterization / deformation / decimation / hull / weld /
> fill-holes / solidify / recalc-normals / triangulate / inset / bisect /
> revolve workstreams (`op-hzs.6`, `op-hzs.7`, `op-hzs.11`–`op-hzs.13`,
> `op-hzs.15`–`op-hzs.29`) are landed. Beyond authoring, two **end-to-end solver
> paths** have landed, each with an egui **studio** app: **MC Studio** (feature
> `mc-export`) authors geometry and runs a basic `outram-mc-libs` Monte Carlo
> **k-eigenvalue** criticality calculation (`k_eff ± σ`) via the `sim` module,
> and **Mesh Studio** (feature `foam-mesh`) volume-meshes an authored surface
> through the `outram-park-fork-cfmesh` **tet → dual → boundary-layers** pipeline
> and exports an OpenFOAM `polyMesh` via the `foam_mesh` module.
>
> **⚠️ AI-generated draft, untrusted until human-reviewed** per the workspace
> `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions.

## Naming & trademark (decided)

**Decision (2026-07-17): keep the name `outram-blender`, and clearly mark it as
a fork.** The workspace convention (bead `op-ahi`) names independent forks
`outram-park-fork-<project>` — e.g. `outram-park-fork-coolprop`. This crate
deliberately keeps `outram-blender`: the `outram-` prefix already marks it as an
OUTRAM PARK fork, and the `-blender` suffix credits the upstream lineage this
crate derives from. The fork status is stated up top and in `Cargo.toml`.

This crate is **not affiliated with, endorsed by, or sanctioned by the Blender
Foundation.** "Blender" is used only to identify the upstream project whose
mesh/geometry architecture inspired this fork. No Blender source code is
included.

## Licensing & provenance

- This crate is **GPL-3.0-only** (workspace default), like the other GPL
  members of OUTRAM PARK. The full text ships as `LICENSE`.
- Blender is licensed **GPLv2-or-later**, which is **GPLv3-compatible** — so a
  future literal port of a Blender algorithm into this crate is license-clean,
  *provided* the GPL attribution header block (upstream project, source file,
  version/commit, copyright, license) is added to any ported file, per the
  workspace provenance rule.
- **Verified against upstream on 2026-08-04**, not assumed — clone
  `786af64a`. Blender's root `COPYING` states it is available under the GNU GPL
  and no other licence; SPDX headers across `source/blender` are
  overwhelmingly `GPL-2.0-or-later` (5981 files), with small numbers of
  Apache-2.0 (167), MIT (10), BSD-3-Clause (3), Zlib (2), GPL-3.0-or-later (2)
  and BSL-1.0 (2). The "or later" clause is what makes GPL-3.0-only
  redistribution clean here. Upstream's GPL-2 text ships verbatim as
  `LICENSE.blender`; the lineage and non-affiliation notice are in `NOTICE`;
  the full record with clone instructions is in `upstream_source/README.md`.
- **Exactly one file is a literal port of Blender source:**
  `src/boolean_predicates.rs`, from `blenlib`'s `BLI_math_boolean.hh` /
  `intern/math_boolean.cc` at commit `96294be7…` (© 2023 Blender Authors,
  `GPL-2.0-or-later`). It carries that provenance in its own header block.
  Only the `double` predicate API is ported — Blender's `mpq_class` (GMP
  rational) overloads are deliberately not, so the crate stays
  Android-buildable with no C dependencies. Blender's `double` predicates are
  themselves a C++ adaptation of Shewchuk's public-domain `predicates.c`.
- **Every other module is an independent reimplementation** of Blender's
  concepts and data-structure architecture (BMesh's vert/edge/loop/face model,
  the modifier stack, the operator naming). Design is not copyrightable, so no
  Blender copyright attaches to those files.
- **Consequently this crate is a derivative work of `GPL-2.0-or-later` code**,
  and distributing it as `GPL-3.0-only` is permitted by that "or later" clause
  — the compatibility finding above is load-bearing, not just precautionary.
  The moment further real Blender code (or another third-party library's
  algorithm) is transcribed, its provenance header and a license re-check are
  mandatory.

## Module map

| Module | Blender analogue | Status |
|---|---|---|
| `math` | `blenlib` `BLI_math` vectors | **real** — a minimal `Vec3` |
| `mesh` | `bmesh` (`BMVert`/`BMEdge`/`BMLoop`/`BMFace`) | **real** — index-based half-edge topology |
| `primitives` | Add-Mesh primitive operators | **real** — cube / UV-sphere / cylinder / grid, unit-tested |
| `revolve` | Spin (`bmo_spin`) | **real** — sweep a profile polyline around an axis into a surface of revolution (pipes / vessels / cone frusta) |
| `ops` | `bmesh/operators` (`bmo_*`) | **real** — extrude / midpoint-subdivide / vertex-bevel (single chamfer or rounded multi-segment spherical cap; boolean delegates to `boolean`) |
| `subdivision` | OpenSubdiv / `MOD_subsurf` | **real** — Catmull-Clark surface subdivision (local stencils) |
| `loop_subdivision` | `MOD_subsurf` (triangle path) | **real** — Loop subdivision surface for triangle meshes |
| `laplacian` | `MOD_laplaciansmooth` / `bmo_smooth_laplacian` | **real** — cotangent/uniform discrete Laplacian + implicit & Taubin (shrinkage-free) smoothing (first `faer` sparse Cholesky solve) |
| `parameterize` | UV unwrap (harmonic map) | **real** — Tutte/harmonic planar parameterization of a disk (reuses the Laplacian sparse solve) |
| `arap` | "As Rigid As Possible" deform | **real** — handle-based ARAP deformation (local Procrustes rotation via 3×3 SVD + cotangent-Laplacian global solve) |
| `decimate` | `MOD_decimate` (Collapse) | **real** — QEM (Garland–Heckbert) edge-collapse mesh simplification |
| `convex_hull` | `bmo_convex_hull` | **real** — 3D convex hull of a point set (incremental, robust `orient3d`) |
| `weld` | `bmo_remove_doubles` / Merge by Distance | **real** — merge coincident vertices within a tolerance (grid hash + union-find; drops collapsed faces) |
| `fill_holes` | `bmo_holes_fill` / Fill Holes | **real** — cap open boundary loops with a centroid triangle fan (winding-consistent, watertight) |
| `solidify` | `MOD_solidify` (simple) | **real** — extrude a surface into a closed shell (area-weighted vertex normals, inner offset shell + rim quads) |
| `recalc_normals` | `normals_make_consistent` (Recalculate Outside) | **real** — repair an inconsistently-wound soup (BFS orientation propagation) + flip each component outward |
| `triangulate` | `bmo_triangulate` (fan) | **real** — fan-triangulate every face into a triangle-only mesh (distinct from `export::triangulate`'s index buffer) |
| `inset` | `bmo_inset` (Individual) | **real** — per-face inset: shrunk inner copy toward the centroid + bridging ring quads |
| `bisect` | Bisect (plane cut) | **real** — half-space clip every face by a plane (Sutherland–Hodgman); leaves the cut open (pairs with `fill_holes`) |
| `edge_bevel` | Bevel (edges) | **real** — chamfer every edge (cut faces back + fill edge/corner gaps); v1 flat chamfer on closed manifolds, winding fixed via `recalc_normals` |
| `boolean` | `bmo_boolean` (Manifold upstream) | **real** — CSG entry point: exact convex-`Intersect` fast path, else delegates to `boolean_general` |
| `boolean_general` | `mesh_boolean.cc` / `mesh_intersect.cc` arrangement | **real** — general union / difference / intersect on non-convex closed meshes (arrangement + winding classification) |
| `boolean_predicates` | `blenlib` `math_boolean.cc` (Shewchuk) | **real** — robust `orient2d/3d`, `incircle`, `insphere` (adaptive f64 + double-double) |
| `boolean_classify` | `mesh_boolean.cc` inside/outside classification | **real** — point-in-closed-mesh via generalized winding number |
| `modifiers` | `modifiers/intern/MOD_*` | **real** — mirror / array / subsurf |
| `procedural` | Geometry Nodes | **real** — node-graph evaluator (primitive / transform / join / subdivide / boolean / output) |
| `export` | I/O exporters | **real** — `triangulate`, OpenFOAM polyMesh **write** (text / disk) + **read** (`from_poly_mesh`, feature `foam-export`, full `constant/polyMesh` round-trip), CSG fitting (box / sphere / Z-cylinder / any convex polyhedron faceted), a DAGMC-style faceted-solid route for non-convex meshes, plus **feature-gated real-type bridges** to `outram-foam-basic-lib` (`foam-export`) and `outram-mc-libs` (`mc-export`) |
| `stl` | STL I/O | **real** — ASCII + binary STL read/write (auto-detect on read); the surface-mesh interchange / DAGMC / Monte-Carlo feed. Import is a triangle soup — `weld` it to recover topology |
| `sim` *(feature `mc-export`)* | — (no Blender analogue) | **real** — Monte Carlo setup + run: build materials from nuclide/density specs, bundle geometry + source + settings, run a k-eigenvalue criticality calc (`k_eff ± σ`, optional cell/flux tally) via `outram-mc-libs`. The **MC Studio** backend |
| `foam_mesh` *(feature `foam-mesh`)* | — (no Blender analogue) | **real** — volume-meshing bridge: blender surface → `outram-park-fork-cfmesh` tet → dual → boundary-layers pipeline → OpenFOAM `polyMesh` (with a quality report). The **Mesh Studio** backend |

## Design rules honoured (workspace `CLAUDE.md`)

- **Index-based topology, no lifetimes/pointers** — every element is a newtype
  index (`VertexId`/`EdgeId`/`LoopId`/`FaceId`) into a `Vec`.
- **Enums for dispatch, never trait objects** — `MeshOp`, `Modifier`,
  `GeometryNode` are closed enums matched exhaustively.
- **No `Box<T>`; `Arc<T>` for sharing; no `unsafe`.**
- **Pure-Rust, Android-buildable** — no OpenGL/Vulkan/GUI in the library; any
  future viewport lives in `examples/` only.

## Quick start

```bash
# Headless authoring demo (no solver features needed).
cargo run -p outram-blender --example authoring_primitives --release

# MC Studio — author geometry, set up + run a basic Monte Carlo criticality calc.
cargo run -p outram-blender --example mc_studio --features mc-export --release

# Mesh Studio — author a surface, volume-mesh it (tet → dual → boundary layers),
# export an OpenFOAM polyMesh.
cargo run -p outram-blender --example mesh_studio --features foam-mesh --release
```

```rust
use outram_blender::primitives;

// A unit cube: 8 vertices, 12 edges, 6 quad faces.
let cube = primitives::cube(1.0);
assert_eq!(cube.vertex_count(), 8);
assert_eq!(cube.edge_count(), 12);
assert_eq!(cube.face_count(), 6);
// Euler characteristic of a closed genus-0 surface: V - E + F = 2.
assert_eq!(cube.euler_characteristic(), 2);
```

## Feeding the OUTRAM PARK solvers

An authored mesh is a **boundary surface**. The two solver targets consume
geometry differently, so each bridge has real work beyond a format copy. Both
are now end-to-end runnable, each with a studio app (see **Quick start**):

- **Monte Carlo (`outram-mc-libs`, feature `mc-export`).** `export` fits the
  authored surface to an analytic CSG universe (`SurfaceKind` primitives combined
  by a signed-half-space region, with a DAGMC faceted-solid fallback for
  non-convex bodies); the `sim` module then attaches materials and runs a
  **k-eigenvalue criticality** calculation (`k_eff ± σ`, optional cell/flux
  tally). **MC Studio** drives this path.
- **CFD volume mesh (`outram-park-fork-cfmesh` → OpenFOAM `polyMesh`, feature
  `foam-mesh`).** A finite-volume solve needs a *volume* mesh (cells, not just a
  boundary surface). The `foam_mesh` bridge hands the authored surface to
  cfmesh's **tet → dual → boundary-layers** pipeline, which carves, snaps,
  tetrahedralises, dualises into polyhedra, grows near-wall prism layers, and
  writes a ready-to-solve `polyMesh` — no external blockMesh / snappyHexMesh
  step. **Mesh Studio** drives this path.

The lower-level `export` bridge additionally emits a raw boundary-patch polyMesh
(via `export::to_polymesh_text` / `write_polymesh`) for callers that supply their
own volume mesher; the `foam-mesh` route above supersedes it for a
ready-to-solve mesh.

The base `export` bridges are **implemented** — `export::to_polymesh_text` / `write_polymesh`
emit the OpenFOAM polyMesh ASCII files and `export::to_csg_primitive` fits
cube/sphere primitives into a CSG description. polyMesh I/O is **round-trip**:
behind the opt-in `foam-export` feature, `export::from_poly_mesh` reads a real
`outram_foam_basic_lib` `PolyMesh` back into a `Mesh`, so
`PolyMesh::read(dir)` + `from_poly_mesh` **imports** an OpenFOAM
`constant/polyMesh` directory (verified by a write→read→convert round-trip on a
cube: V/E/F/χ = 8/12/6/2). By default the crate emits standalone text and local
mirror types and takes **no** hard path dependency on `outram-foam-*` /
`outram-mc-libs`; the real-type read/write bridges live behind `foam-export` /
`mc-export` (tracked in `op-hzs.6` / `op-hzs.7`; polyMesh read in `op-hzs.30`).
See `export`'s module docs.

## Dependency map

Blender's ~80 third-party dependencies are audited for relevance to a Rust
mesh-authoring frontend in
[`docs/blender-dependencies.md`](docs/blender-dependencies.md) — grouped by
purpose, each with a Rust-ecosystem path (equivalent crate / reimplement /
skip) and an Android-hostility flag.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
