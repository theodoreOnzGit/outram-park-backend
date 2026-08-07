<!--
SPDX-License-Identifier: GPL-3.0-only
Part of Outram Park (outram-park-backend).
A Rust translation of selected OpenFOAM mesh utilities. Independent fork, not
the official OpenFOAM software; see the workspace-root TRADEMARKS.md.
-->

# outram-foam-mesh

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

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
| `snappy_hex_mesh` | `snappyHexMesh` | Split-hex meshing around STL surfaces (castellate → snap → layers) |
| `ideas_unv_to_foam` | `ideasUnvToFoam` | Import an I-DEAS `.unv` mesh into `polyMesh` |
| `poly_dual_mesh` | `polyDualMesh` | Polyhedral dual-mesh construction |

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
  (12-edge blend); straight-edge blocks only (arc/spline edges deferred).
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

**Unit-tested ≠ validated.** These are AI-assisted translations checked against
the upstream source and analytical geometry, not yet validated against OpenFOAM
mesh outputs on production cases — see each module's "Honest scope" section for
the precise boundary of what is reproduced, and the Bookkeeping status above
(both human-review axes remain uncleared).
