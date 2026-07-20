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

## Tools (scaffolded; implementation in progress)

| Module | OpenFOAM tool | What it does |
|---|---|---|
| `block_mesh` | `blockMesh` | Structured hex meshing from a `blockMeshDict` |
| `snappy_hex_mesh` | `snappyHexMesh` | Split-hex meshing around STL surfaces (castellate → snap → layers) |
| `ideas_unv_to_foam` | `ideasUnvToFoam` | Import an I-DEAS `.unv` mesh into `polyMesh` |
| `poly_dual_mesh` | `polyDualMesh` | Polyhedral dual-mesh construction |

## Status

Freshly scaffolded (crate skeleton + module docs). Each tool is being
implemented + tested under the `op-fm` beads epic. `snappyHexMesh` is the
largest and lands incrementally (castellation → snapping → layers).
