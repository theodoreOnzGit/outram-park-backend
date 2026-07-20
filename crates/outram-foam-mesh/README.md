<!--
SPDX-License-Identifier: GPL-3.0-only
Part of Outram Park (outram-park-backend).
A Rust translation of selected OpenFOAM mesh utilities. Independent fork, not
the official OpenFOAM software; see NOTICE / TRADEMARKS.md.
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

## Tools

| Module | OpenFOAM tool | What it does |
|---|---|---|
| `block_mesh` | `blockMesh` | Structured hex meshing from a `blockMeshDict`, incl. multi-grading and full per-edge `edgeGrading` |
| `snappy_hex_mesh` | `snappyHexMesh` | Split-hex meshing around STL surfaces (castellate → snap → layers) |
| `ideas_unv_to_foam` | `ideasUnvToFoam` | Import an I-DEAS `.unv` mesh into `polyMesh` |
| `poly_dual_mesh` | `polyDualMesh` | Polyhedral dual-mesh construction |

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

**Unit-tested ≠ validated.** These are AI-assisted translations checked against
the upstream source and analytical geometry, not yet validated against OpenFOAM
mesh outputs on production cases — see each module's "Honest scope" section for
the precise boundary of what is reproduced, and the Bookkeeping status above
(both human-review axes remain uncleared).
