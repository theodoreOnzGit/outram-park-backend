# outram-blender

> **This is a fork.** `outram-blender` is an independent, GPL-licensed
> **fork / derivative of [Blender](https://github.com/blender/blender)'s
> mesh-authoring architecture**, reusing its mesh/geometry design under the GPL
> for the OUTRAM PARK multiphysics suite. It is **not** affiliated with,
> endorsed by, or sanctioned by the Blender Foundation, and bundles no Blender
> source code. "Blender" identifies only the upstream project it derives from.

A pure-Rust, headless **mesh-authoring frontend** for the OUTRAM PARK
multiphysics suite, inspired by the **architecture** of
[Blender](https://github.com/blender/blender). The eventual goal is to author
and procedurally generate geometry that feeds the OUTRAM PARK solvers — an
`outram-foam-mesh` `polyMesh` for CFD, or an `outram-mc-libs` CSG universe for
Monte Carlo neutron transport.

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
> results. The main remaining modelling gap is the multi-segment rounded bevel
> (bead `op-hzs.11`).
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
mesh/geometry architecture inspired this scaffold. No Blender source code is
included.

## Licensing & provenance

- This crate is **GPL-3.0-only** (workspace default), like the other GPL
  members of OUTRAM PARK.
- Blender is licensed **GPLv2-or-later**, which is **GPLv3-compatible** — so a
  future literal port of a Blender algorithm into this crate is license-clean,
  *provided* the GPL attribution header block (upstream project, source file,
  version/commit, copyright, license) is added to any ported file, per the
  workspace provenance rule.
- At scaffold stage **nothing is ported** — only concepts and architecture are
  reused, which does not carry Blender's copyright. The moment real Blender code
  (or a third-party library's algorithm) is transcribed, its provenance header
  and a license re-check are mandatory.

## Module map

| Module | Blender analogue | Status |
|---|---|---|
| `math` | `blenlib` `BLI_math` vectors | **real** — a minimal `Vec3` |
| `mesh` | `bmesh` (`BMVert`/`BMEdge`/`BMLoop`/`BMFace`) | **real** — index-based half-edge topology |
| `primitives` | Add-Mesh primitive operators | **real** — cube / UV-sphere / cylinder / grid, unit-tested |
| `ops` | `bmesh/operators` (`bmo_*`) | **real** — extrude / midpoint-subdivide / vertex-bevel (boolean delegates to `boolean`; multi-segment bevel is a follow-up) |
| `subdivision` | OpenSubdiv / `MOD_subsurf` | **real** — Catmull-Clark surface subdivision (local stencils) |
| `boolean` | `bmo_boolean` (Manifold upstream) | **real** — CSG entry point: exact convex-`Intersect` fast path, else delegates to `boolean_general` |
| `boolean_general` | `mesh_boolean.cc` / `mesh_intersect.cc` arrangement | **real** — general union / difference / intersect on non-convex closed meshes (arrangement + winding classification) |
| `boolean_predicates` | `blenlib` `math_boolean.cc` (Shewchuk) | **real** — robust `orient2d/3d`, `incircle`, `insphere` (adaptive f64 + double-double) |
| `boolean_classify` | `mesh_boolean.cc` inside/outside classification | **real** — point-in-closed-mesh via generalized winding number |
| `modifiers` | `modifiers/intern/MOD_*` | **real** — mirror / array / subsurf |
| `procedural` | Geometry Nodes | **real** — node-graph evaluator (primitive / transform / join / subdivide / boolean / output) |
| `export` | I/O exporters | **real** — `triangulate`, OpenFOAM polyMesh text, CSG fitting (box / sphere / Z-cylinder / any convex polyhedron faceted), and a DAGMC-style faceted-solid route (winding `contains`) for non-convex meshes |

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
cargo run -p outram-blender --example authoring_primitives --release
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

## Feeding the OUTRAM PARK solvers (planned)

An authored mesh is a **boundary surface**. The two solver targets consume
geometry differently, so each bridge (in `export`) has real work beyond a format
copy:

- **`outram-foam-mesh` polyMesh (CFD)** is a finite-volume *volume* mesh
  (points, faces, owner/neighbour, boundary patches). A surface has no cells, so
  the bridge emits a boundary patch that a volume mesher (blockMesh /
  snappyHexMesh) then fills — it is not a ready-to-solve volume mesh.
- **`outram-mc-libs` CSG (Monte Carlo)** is analytic constructive solid
  geometry (`SurfaceKind` primitives combined by a signed-half-space region).
  The near-term route is *primitive fitting*: emit the exact analytic CSG for a
  mesh that came from a `primitives` generator.

Both bridges are **implemented** — `export::to_polymesh_text` emits the OpenFOAM
polyMesh ASCII files and `export::to_csg_primitive` fits cube/sphere primitives
into a CSG description — but the crate intentionally does **not** yet take a
path dependency on `outram-foam-*` / `outram-mc-libs` (it emits standalone text
and local mirror types), to avoid churn while those crates are under active
development. Wiring to their real types is tracked in `op-hzs.6` / `op-hzs.7`.
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
