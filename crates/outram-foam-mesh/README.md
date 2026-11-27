<!--
SPDX-License-Identifier: AGPL-3.0-only
Part of Outram Park (outram-park-backend).
A Rust translation of selected OpenFOAM mesh utilities. Independent fork, not
the official OpenFOAM software; see the workspace-root TRADEMARKS.md.
-->

# outram-foam-mesh

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. For research, education and V&V only. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

OpenFOAM **mesh generation and conversion** utilities translated to Rust, on top
of [`outram-foam-basic-lib`](../outram-foam-basic-lib)'s primitive + FV layer.
This is an **independent OUTRAM PARK fork**, not the official OpenFOAM software
and not endorsed by it (see `TRADEMARKS.md`).

**Name.** `outram` is the workspace's OUTRAM: **O**pen-source **U**nified
**TR**Ansient **M**ulti-Physics (from OUTRAM PARK, `docs/ecosystem-naming.md`);
`foam` marks the OpenFOAM lineage; `mesh` is its scope. **The question it
answers:** how do OpenFOAM's meshing and mesh-manipulation utilities
(`blockMesh`, `snappyHexMesh`, `polyDualMesh`, `checkMesh`, `decomposePar`,
mesh motion, particle tracking) build and grade a mesh, step by step?

**What this crate lets you study and challenge.** Meshing is where most CFD
errors start and where they are hardest to see. This crate writes each
OpenFOAM meshing step as readable Rust with the upstream `File.C:line` it
follows, and draws the result: castellation, snapping, layers, dual meshes and
projections are committed as images a reader can check by eye. The quality
metrics are checked against closed forms (a uniform shear gives exactly
`atan(k)` non-orthogonality; a 2:1 octree interface sits at exactly
25.2394 deg) and the generators against ESI OpenFOAM-v2506 runs value by value,
with each ulp difference traced to its cause (PETIR `cos`/`sin` against glibc).

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream code: OpenFOAM-dev commit `852527377c7e87e33d51b19ca8974b5db60af28b` (GPL-3.0-or-later; [`upstream_source/README.md`](upstream_source/README.md)) and OpenFOAM.com ESI v2506 (the comparison build and, for `snappyHexMesh`, the specification since gh:#1459). No `crates/kovan-literature/CATALOGUE.md` entry is tied to this crate. |
| Mathematical equations | Module docs: transfinite block interpolation and grading (`block_mesh`, `block_edges`), arc / spline / BSpline edges and surface projection (`block_projection`), Laplacian snapping with `pointConstraint` ranks (`snappy_hex_mesh`), the non-orthogonality, skewness and aspect-ratio definitions (`mesh_quality`, `check_mesh`), the swept-volume mesh flux `meshPhi` (`dynamic_mesh`), barycentric particle tracking (`particle_tracking`). |
| Source code | `driver`, `block_mesh`, `block_edges`, `block_projection`, `block_primitives`, `snappy_hex_mesh`, `poly_dual_mesh`, `poly_dual_mesh_upstream`, `ideas_unv_to_foam`, `mesh_quality`, `check_mesh`, `decompose`, `dynamic_mesh`, `particle_tracking`, `topo_set`, `section`. |
| Interactive simulations | None yet. CLI examples in `examples/` (`sphere_in_box.rs`, `decompose_par.rs`, the `draw_*` and `compare_*` tools); the binaries are in `outram-foam-cli`. |
| Visualisations | 32 committed PNGs under `verification_and_validation/` (`images/`, `esi_v2506_differential_gh1364/images/`, `snappy_hex_mesh_esi_v2506/images/`), drawn by `examples/draw_block_mesh.rs`, `draw_poly_dual_mesh.rs`, `draw_castellation_refinement.rs`, `draw_snap_differential.rs`, `draw_flange_differential.rs`; plane sections by the `section` module. |
| Verification and validation evidence | [`verification_and_validation/`](verification_and_validation/). As recorded: `esi_v2506_differential_gh1364` (2026-11-13): blockMesh ESI flavour 7 of 7 dicts identical in topology, points bit-identical on 5, the two arc dicts at 1 ulp; polyDualMesh 27 of 27 duals byte-identical; checkMesh every field bit-identical on 33 meshes. `decompose_par_gh1381`: byte-identical to ESI v2506 on 1098 files. `dynamic_mesh_gh1380`: 245,180 values bit-identical. `particle_tracking_gh1382`: 29,784 values bit-identical. `check_mesh_1480`, `poly_dual_mesh_gh1460.md`, `upstream_dev_852527377c7e` (against OpenFOAM-dev), `snappy_hex_mesh_esi_v2506`, `snappy_hex_mesh_1465`, `snappy_hex_mesh_1470`, `block_mesh_curved_edges.md`, `block_mesh_projection.md`, `block_mesh_topology_numbering.md`. Closed-form `mesh_quality` checks on five hand-built meshes (below). |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp (no `kind = "review"` block in any `review.md` here). No entry in `docs/human-corrections-to-ai-work.md` names this crate. The V&V records keep struck-through corrections, e.g. the "no upstream `blockMesh` binary" claim in `block_mesh_curved_edges.md` (gh:#1462). |

**Status:** blockMesh, polyDualMesh, checkMesh, decomposePar (`simple`,
`hierarchical`), solid-body mesh motion and particle tracking have AI-run
value-by-value evidence against ESI v2506; snappyHexMesh has stage
differentials (see its records for what agrees and what does not);
`mergePatchPairs`, solver-based mesh motion and topology changers are not
ported. Nothing here has been reviewed by a human.

## `no_std` + `alloc` (gh:#1638)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-foam-mesh --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). Every mesher and tool builds without it:
`blockMesh` from a parsed dictionary, `snappyHexMesh` (castellation, snapping,
layers), `polyDualMesh`, `checkMesh`, `decomposePar`'s geometric methods, the
UNV conversion and the STL parsers from bytes or text. What `std` adds is the
**filesystem**: `read_stl` / `read_stl_zoned`, `block_mesh_from_file`, the UNV
`convert_file`, reading a dictionary with `#include`, `write_case` and
`write_processor_cases`. Maps are std's with `std` (so every byte-identity pin
against ESI holds) and `hashbrown` without. The root workspace entry is
`default-features = false`, so a member that wants `std` asks for it.

## Quick start

One function turns a closed surface into an OpenFOAM `polyMesh`:

```rust
use outram_foam_mesh::{mesh_from_surface, MeshingControls, MeshingPhases};
use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;
use outram_foam_basic_lib::primitives::Vector3;

let sphere = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 32, 32);
let controls = MeshingControls::external_flow([12, 12, 12], 2)
    .with_phases(MeshingPhases::CastellateSnap);

let result = mesh_from_surface(&sphere, &controls)?;
println!("{}", result.quality.summary());   // checkMesh-style report
result.write_case("my_case")?;              // -> my_case/constant/polyMesh
```

The worked example runs exactly this and is readable top to bottom:

```bash
cargo run --release -p outram-foam-mesh --example sphere_in_box
```

Its measured output (2026-08-07, release): 6128 cells, max non-orthogonality
41.80 deg (mean 9.58), max skewness 0.668, max aspect ratio 1.965, 0 inverted
cells, meshed volume 59.8594 m^3 against the analytic box-minus-sphere
59.8112 m^3 (+0.081 %), verdict `GOOD`.

## Tools

| Module | OpenFOAM tool | What it does |
|---|---|---|
| `driver` | (the `snappyHexMesh` app) | **Start here** — surface + controls -> `PolyMesh`, phases picked by enum |
| `mesh_quality` | `checkMesh` | Non-orthogonality (max/mean), skewness, aspect ratio, inverted-cell count for *any* `polyMesh` |
| `block_mesh` | `blockMesh` | Structured hex meshing from a `blockMeshDict`, incl. multi-grading and full per-edge `edgeGrading` |
| `block_primitives` | (blockMeshDict generators) | A pipe as an O-grid (`PipeOGrid`, every wall vertex on the circle, no arc edges needed) and a box as one block (`BoxBlock`), with caller-named patches; built by `block_mesh` (gh:#1155). Since gh:#1156 also an `Annulus` on the O-grid's vertex angles and a `DoublePipe` (tube O-grid, tube-wall ring, annulus ring, meeting face to face for conjugate heat transfer); `PolyMesh::concatenate` puts region meshes side by side for drawing. Since gh:#1157 a `HelicalPipe`: the pipe O-grid swept along a helix on a rotation-minimising (untwisted) frame, with the centreline, Frenet frame, curvature, torsion and `locate` (a point's arc position and Frenet offsets) |
| `section` | (for drawing) | Axis-aligned plane sections of every cell (`plane_section`) and the boundary wireframe (`boundary_edges`) of a `PolyMesh` |
| `snappy_hex_mesh` | `snappyHexMesh` | Split-hex meshing around STL surfaces (castellate → snap → layers) |
| `ideas_unv_to_foam` | `ideasUnvToFoam` | Import an I-DEAS `.unv` mesh into `polyMesh` |
| `poly_dual_mesh` | `polyDualMesh` | Polyhedral dual-mesh construction |
| `check_mesh` | `checkMesh` (ESI v2506 forms) | The `esi_*` metrics, bit-identical to ESI on 33 meshes (gh:#1364); row added 2026-11-27 |
| `block_edges`, `block_projection` | `blockMesh` edges and `projection` | `arc`, `polyLine`, `spline`, `BSpline` edges and projection onto geometry (gh:#1461, gh:#1463); rows added 2026-11-27 |
| `dynamic_mesh` | `dynamicMotionSolverFvMesh` (solid-body) | See "Status" below (gh:#1380); row added 2026-11-27 |
| `particle_tracking` | `particle::track` / `trackToFace` | Barycentric particle tracking on a static `polyMesh`, ESI v2506 (gh:#1382); row added 2026-11-27 |
| `topo_set` | `topoSet` cell sources | Cell selections used by `setFields` / `topoSet` in `outram-foam-cli` (gh:#1384); row added 2026-11-27 |
| `decompose` | `decomposePar` (`simple`, `hierarchical`; processor patches, `processorN/constant/polyMesh`), libstdc++ `std::sort` tie order | gh:#1381; byte-identical to ESI v2506 on 1098 files, see `verification_and_validation/decompose_par_gh1381/README.md` |

### One mesh currency

Each generator has its own working representation, but every one of them
converts to `outram-foam-basic-lib`'s `io::PolyMesh` — the type that writes
`constant/polyMesh` to disk and that `mesh_quality` grades:

| Produced by | Convert with |
|---|---|
| `block_mesh` | `block_mesh::PolyMesh::to_foam_poly_mesh` |
| `snappy_hex_mesh` | `PolyPatchMesh::to_foam_poly_mesh` |
| `poly_dual_mesh` | `DualMesh::to_foam_poly_mesh` |
| `driver` | already a `PolyMesh` — `GeneratedMesh::write_case` |

## Status

Each tool is implemented and unit/integration-tested (translations of the
OpenFOAM-dev C++, with upstream `File.C:line` provenance in the module docs).
`snappyHexMesh` runs the full castellate → snap → layers pipeline:

- **`block_mesh`** — `simpleGrading` multi-grading and full per-edge `edgeGrading`
  (12-edge blend); ~~straight-edge blocks only (arc/spline edges deferred).~~
  **CORRECTED 2026-11-27** — `arc`, `polyLine`, `spline` and `BSpline` edges and
  surface projection are ported (`src/block_edges.rs`, `src/block_projection.rs`;
  V&V `verification_and_validation/block_mesh_curved_edges.md`).
- **`snappy_hex_mesh` castellation** — octree refinement around STL surfaces.
- **`snappy_hex_mesh` snapping** — surface projection + Laplacian smoothing with
  OpenFOAM-style `pointConstraint` feature snapping (surface/edge/corner ranks).
- **`snappy_hex_mesh` layers** — medial-axis interior shrink-and-insert (with a
  watertight outward-extrude fallback on refined/hanging-node regions).
  **Which of the two you get depends on the case**, so `mesh_from_surface`
  measures it (via total-volume conservation) and reports it as a
  `LayerOutcome`. Measured on the built-in sphere-in-box: a 12x12x12 background
  at level 2 gets the real interior insert; an 8x8x8 background at levels 1-2
  falls back to the outward extrusion, which grows the domain. Check the
  reported outcome — do not assume.
- **`mesh_quality`** — verified against closed-form values on five hand-built
  meshes: a Cartesian block (0 deg, 0 skew, AR exactly 1), a uniform shear
  (non-orthogonality exactly `atan(k)`, tested at 45/60/86 deg, skewness
  exactly 0), a laterally offset face (skewness exactly `s`, non-orthogonality
  exactly 0), an elongated cell (AR = `(ab+bc+ca)/(3(abc)^(2/3))`), and an
  inverted cell. Two findings worth knowing:
  - an **octree castellated mesh is not orthogonal** — every 2:1 refinement
    interface sits at exactly `arccos(1.5/sqrt(2.75)) = 25.2394 deg` with
    skewness exactly `1/3`;
  - **dualisation does not create non-orthogonality** — the dual of a uniform
    hex block measures exactly 0 deg and 0 skewness, so a polyhedral dual that
    measures 80 deg+ inherited that from its primal, not from the dual step.
    ~~(hex blocks only)~~ **Since gh:#1240 (2026-11-04)** `poly_dual_mesh` also
    runs on tetrahedral meshes (through `outram-park-fork-cfmesh`'s
    `DualKind::CellCentre`): on the cfMesh pipe at h 5 mm its internal
    non-orthogonality is 16.7° mean, 47.7° max, and its boundary follows
    upstream (one face per point per feature-bounded sector, patch seams are
    features, `FeaturePoints` rule). Not compared against an OpenFOAM run.
- **`dynamic_mesh`** (gh:#1380) — solid-body mesh motion
  (`linearMotion`, `rotatingMotion`, `axisRotationMotion`,
  `oscillatingLinearMotion` with constant coefficients) and the mesh motion flux
  `meshPhi` (`face::sweptVol`). Compared with OpenFOAM.com v2506
  (`dynamicMotionSolverFvMesh` through a `dumpMeshMotion` probe): 245,180 values
  (points, `meshPhi`, face and cell geometry; 2 meshes x 4 motions x 5 steps)
  **bit-identical**. Not ported: `displacementLaplacian` and the other solver
  based motion solvers, motion boundary conditions, topology changers, 6-DoF.
  Record: `verification_and_validation/dynamic_mesh_gh1380/README.md`.

**Unit-tested ≠ validated.** These are AI-assisted translations checked against
the upstream source and analytical geometry, ~~not yet validated against OpenFOAM
mesh outputs on production cases~~ **CORRECTED 2026-11-27** — and, for the tools
listed in "Knowledge in this crate", compared value by value with ESI v2506
and OpenFOAM-dev runs on tutorial and constructed cases (a verification, not a
validation) — see each module's "Honest scope" section for
the precise boundary of what is reproduced, and the Bookkeeping status above
(both human-review axes remain uncleared).
