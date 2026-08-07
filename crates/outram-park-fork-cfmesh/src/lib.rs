// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This crate is a pure-Rust fork / port of:
//   cfMesh — https://github.com/wyldckat/cfMesh
//   Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
// with the Voronoi / polyhedral-dual construction informed by:
//   voro++ — https://github.com/chr1shr/voro
//   Copyright (C) 2008-2015 The Regents of the University of California,
//   through Lawrence Berkeley National Laboratory. Licence: modified BSD
//   (3-clause), GPL-3.0-compatible; see NOTICE. No voro++ code is included.
// and, where the same stage exists there, by OpenFOAM (snappyHexMesh,
// polyDualMesh, polyMesh, checkMesh):
//   Copyright (C) 2011-2016 OpenFOAM Foundation
//   Copyright (C) 2016-2023 OpenCFD Ltd. Licence: GPL-3.0-only.
// Per-file provenance blocks record which reference each module follows.
//
// Not affiliated with, or endorsed by, Creative Fields, the cfMesh project, the
// OpenFOAM Foundation, OpenCFD Ltd., or LBNL.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # outram-park-fork-cfmesh
//!
//! A pure-Rust **fork / port of [cfMesh](https://github.com/wyldckat/cfMesh)**
//! (Creative Fields' automatic unstructured mesh generator, **GPL-3.0**) for the
//! OUTRAM PARK multiphysics suite — the *volume-mesh-generation* layer that sits
//! between the [`outram-blender`](https://crates.io/crates/outram-blender)
//! surface-authoring frontend and the OpenFOAM-style solvers.
//!
//! ```text
//!   outram-blender  ──►  outram-park-fork-cfmesh  ──►  outram-foam-basic-lib
//!   (surface Mesh)       (tet / polyhedral cells        (PolyMesh → FvMesh,
//!                         + boundary layers)              solvable)
//! ```
//!
//! ## Why this crate exists
//!
//! The workspace has the mesh **representation** and finite-volume addressing
//! (`outram-foam-basic-lib`'s `FvMesh` / `io::poly_mesh::PolyMesh`, with
//! `read`/`write`/`to_fv_mesh`), and the surface-**authoring** frontend
//! (`outram-blender`). This crate supplies the **cfMesh-lineage** automatic
//! unstructured generator between them — turning a closed `outram-blender`
//! surface into a solvable *volume* mesh with polyhedral cells and near-wall
//! boundary layers in one call. Open-source, pure-Rust, GPLv3-clean tooling for
//! that is genuinely lacking, so this crate ports the proven cfMesh workflows
//! rather than reinventing them.
//!
//! ## Relationship to `outram-foam-mesh` (they overlap)
//!
//! `outram-foam-mesh` is a **separate, earlier** crate porting the
//! **OpenFOAM-lineage** utilities: `blockMesh`, `ideasUnvToFoam`,
//! `snappyHexMesh` castellation, the **cell-centre** `polyDualMesh`, and
//! `checkMesh`-style quality assessment. Both crates turn a surface into a
//! volume mesh, both build a dual and both insert layers, so the overlap is
//! real. The decisive differences: that crate mirrors the individual OpenFOAM
//! binaries and builds the **cell-centre** dual, whereas this one exposes a
//! single composed [`pipeline::surface_to_tet_dual_mesh`] over an
//! `outram-blender` surface and builds the **median (vertex-centred)** dual.
//! The two duals are different algorithms and **their V&V results are not
//! interchangeable** — see the [`dual`] module docs.
//!
//! ## Goal
//!
//! Consume an `outram-blender` [`Mesh`](https://docs.rs/outram-blender) (a closed
//! watertight surface), generate a **tetrahedral** then **polyhedral** volume
//! mesh (à la cfMesh `tetMesh` / `cartesianMesh` + OpenFOAM `polyDualMesh`), with
//! optional **wall boundary/prism layers**, and emit a real-cell
//! `outram_foam_basic_lib::io::poly_mesh::PolyMesh` for the CFD/TH solvers and
//! `outram-mc-libs` geometry for neutronics — the mesh substrate for coupled
//! pebble-bed / molten-salt / light-water reactor simulations.
//!
//! ## Vendored upstreams (reference only, GPLv3-clean, never shipped)
//!
//! Both live under `upstream_source/` (gitignored, dev-only — see
//! `upstream_source/README.md`):
//!
//! - **cfMesh** — <https://github.com/wyldckat/cfMesh>, **GPL-3.0-only**. Primary
//!   port target: `meshLibrary/{cartesianMesh, tetMesh, utilities}` (Cartesian
//!   hex-dominant + tet meshing, surface tools, boundary-layer insertion).
//! - **voro++** — <https://github.com/chr1shr/voro>, modified-BSD (LBNL),
//!   GPLv3-compatible. Reference for the Voronoi / polyhedral-dual construction.
//!
//! ## Provenance of the source files
//!
//! **Every `src/*.rs` file carries an SPDX identifier and a provenance header
//! block.** Because this crate re-implements published *algorithms* rather than
//! transcribing C++, the block names the upstream project, the source directory
//! or file the construction follows, the copyright holder, and the licence —
//! rather than a per-file upstream commit, which would imply a line-level
//! correspondence that does not exist. Files that are original OUTRAM PARK work
//! with no upstream ancestor ([`math`], [`shapes`], [`reactor`], [`pipeline`],
//! [`patches`]) say so explicitly in the same block.
//!
//! Non-cfMesh algorithmic sources are credited where they are used, with the
//! literature citation rather than an implied code lineage. In particular
//! [`delaunay::orient3d`] / [`delaunay::insphere`] are **Shewchuk's
//! predicates** (J. R. Shewchuk, *Adaptive Precision Floating-Point Arithmetic
//! and Fast Robust Geometric Predicates*, Discrete & Computational Geometry
//! 18(3):305-363, 1997) — note that only the determinant *formulations* are
//! taken: the adaptive exact-arithmetic expansions that make Shewchuk's
//! `predicates.c` robust are **not** implemented here, so these are fast `f64`
//! evaluations and are not a drop-in substitute for the real thing on
//! degenerate input.
//!
//! ## Status
//!
//! - **Milestone 1 — volume-mesh core + Cartesian block mesher.**
//!   [`volume_mesh::VolumeMesh`] (points + faces + owner/neighbour + patches,
//!   mirroring cfMesh `polyMeshGen` / OpenFOAM `polyMesh`) and
//!   [`cartesian::cartesian_box`] (a regular hex grid of an axis-aligned box).
//! - **Milestone 2 — castellated surface carve.** [`carve::carve_box`] overlays
//!   a uniform Cartesian grid on a closed triangle-soup surface and keeps the
//!   cells inside it (ray-parity inside test), producing a body-fitted
//!   *staircase* volume mesh with a `walls` boundary patch.
//!   [`carve::carve_region`] extends this to the region *inside* an outer
//!   surface but *outside* inner holes — the shell/annular pattern reactor
//!   geometry needs (coolant around fuel pins or pebbles).
//! - **Milestone 3a — boundary snapping.** [`snap::snap_to_surface`] projects
//!   every boundary point onto the closest point of the surface, turning the
//!   staircase into a body-fitted boundary.
//! - **Milestone 3b — foam bridge (feature `foam-export`).** [`foam::to_poly_mesh`]
//!   converts a [`volume_mesh::VolumeMesh`] into a real `outram-foam-basic-lib`
//!   `PolyMesh`, which yields a solvable `FvMesh` via `to_fv_mesh()` — closing
//!   the loop: surface → carve → snap → foam mesh (verified end-to-end).
//! - **Mesh quality checks.** [`checks::check_quality`] reports face
//!   non-orthogonality, skewness, cell aspect ratio, min face area / cell
//!   volume, and negative-volume cells (cfMesh `polyMeshGenChecks`) — the gate
//!   for trusting a generated mesh before it is solved.
//! - **Octree near-wall refinement.** [`octree::refine_near_boundary`] grades
//!   the mesh finer next to the surface, splitting each coarse transition
//!   face into its four fine sub-faces (hanging nodes) so the coarse cell
//!   becomes a genuine **polyhedron** — the mesh stays conforming (verified:
//!   exact volume, closed cells, > 6-face transition cells). Every emitted face
//!   ring additionally has the hanging nodes on its edges spliced in, so each
//!   edge lies in exactly two faces of every cell — the **edge-manifold**
//!   property [`tet::tetrahedralize`] needs to stay watertight on a graded mesh
//!   (verified). [`octree::refine_near_boundary_banded`] is the size-field
//!   form used by the pipeline: refine where the cell centre is within a
//!   distance band of the surface, the band measured in the cell's own edge
//!   lengths.
//! - **Prism boundary layers.** [`layers::add_boundary_layers`] inserts graded
//!   near-wall inflation layers at a wall patch (snappyHexMesh *addLayers* /
//!   cfMesh's boundary-layer step): the interior wall points move inward and
//!   the vacated shell is filled with `n_layers` stacked prism cells per wall
//!   face. It is a repartition, so it preserves the mesh volume exactly
//!   (verified: exact volume, closed, `+n_layers × wall_faces` cells). Flat
//!   (box) walls only — the fixed-thickness march self-intersects on curvature.
//!   [`layers::add_boundary_layers_adaptive`] handles **curved / polyhedral
//!   walls** (smoothed normals + per-point thickness limiting + validity
//!   back-off), verified valid + volume-conserving on snapped spheres/cylinders.
//! - **Polyhedral dual.** [`dual::polyhedral_dual`] turns a primal mesh into a
//!   **polyhedral** one — one cell per primal vertex — via the **median**
//!   (Donald / vertex-centred) dual. It fills the same role as OpenFOAM's
//!   `polyDualMesh` but is **not the same construction**: `polyDualMesh` (and
//!   `outram-foam-mesh`'s `poly_dual_mesh`) is the *cell-centre* dual, whereas
//!   this places dual corners at edge midpoints and face/cell centroids, and it
//!   is not the circumcentre Voronoi dual either. The dual tiles the same
//!   region (verified: exact volume, closed cells, every internal face shared
//!   by exactly two cells, genuinely > 6-face cells). Two variants:
//!   [`dual::polyhedral_dual`] (robust median, quad-fan faces) and
//!   [`dual::polyhedral_dual_min_faces`] (**face-minimal** — one polygon per
//!   primal edge via edge-star walking, ~40% fewer faces, verified to conserve
//!   volume and stay closed). **Not verified:** no test in [`dual`] measures
//!   non-orthogonality or skewness, so this dual's orthogonality is
//!   **unmeasured** — and `outram-foam-mesh`'s "dualisation does not create
//!   non-orthogonality" result belongs to its *cell-centre* dual and does not
//!   carry over. See the [`dual`] module docs.
//! - **Tetrahedralization.** [`tet::tetrahedralize`] splits every cell into
//!   tetrahedra by centroid subdivision (the all-tet foundation; not a
//!   from-scratch Delaunay mesher). It conserves volume and triangulates the
//!   boundary surface (verified: positive-volume tets, boundary area == input
//!   surface, exact volume, every cell a 4-triangle tet). **Not verified:** no
//!   test gates any *shape*-quality metric, so the tet primal's quality is
//!   **unmeasured**; [`delaunay`] records qualitatively that the subdivision
//!   leaves slivers, making this the *suspected* — not demonstrated — dominant
//!   non-orthogonality source downstream. See the [`tet`] module docs.
//! - **Quality smoothing.** [`smooth::laplacian_smooth`] relaxes interior
//!   vertices toward their neighbour centroid (smart Laplacian — never inverts a
//!   cell, pins the boundary), improving cell shape while conserving volume
//!   exactly (verified: recovers perturbed tet quality, no inversions).
//! - **Flip-based Delaunay.** [`delaunay::flip_to_delaunay`] improves a tet mesh
//!   toward Delaunay by bistellar 2→3 / 3→2 flips (Shewchuk [`delaunay::orient3d`]
//!   / [`delaunay::insphere`] predicates). Volume- and boundary-preserving, with
//!   an *improve-or-noop* guard so it can never make a mesh worse and refuses
//!   tangled input (verified: fixes a non-Delaunay bipyramid exactly, conserves
//!   volume + bounded growth on a block, returns invalid input unchanged).
//! - **High-level pipeline (the recommended entry point).**
//!   [`pipeline::surface_to_tet_dual_mesh`] composes the stages above into one
//!   call: `carve → snap → tetrahedralize → Delaunay-improve → polyhedral dual
//!   → smooth → adaptive prism boundary layers`, driven by
//!   [`pipeline::TetDualOptions`] and returning a [`pipeline::TetDualReport`]
//!   (cell count, volume, quality, and a note for every stage skipped). Each
//!   optional stage is applied only if its result stays valid (closed + no
//!   inverted cells), otherwise it is **gracefully skipped** and the previous
//!   mesh kept, so the returned mesh is always valid and exportable. This is the
//!   coarse-grained meshing entry point the `outram-blender` "mesh studio" GUI
//!   calls; the [`pipeline::box_tet_dual`] / [`pipeline::sphere_tet_dual`] /
//!   [`pipeline::cylinder_tet_dual`] wrappers mesh the built-in primitives.
//!   Lengths in metres, layer thickness in metres, angles in degrees.
//!
//!   Stage 1 can be **octree-graded** rather than uniform via
//!   [`pipeline::TetDualOptions::refinement_levels`] (default `0` = the uniform
//!   carve, unchanged): the interior stays at `cell_size` and the near-wall band
//!   is refined, so a given wall resolution costs far fewer cells. Measured on a
//!   radius-3 m sphere: **1381 cells at 1.66 % volume error** graded, versus
//!   **5269 cells at 1.72 %** for the uniform carve resolving the same 0.5 m
//!   wall; the widest measured gap is 3921 vs 101376 cells at ~1.07 % error.
//!   **Only that first pair is run by a test** — and even there the assertions
//!   are relative (graded no less accurate, graded uses < half the cells, both
//!   meshes closed and under 90 deg non-orthogonality), not the tabulated
//!   numbers; the widest-gap pair and the whole skewness column are
//!   **doc-recorded only**, measured by hand on 2026-08-07. See [`pipeline`]'s
//!   tests for the full table, its per-row gating column, and its caveats.
//! - **Named boundary patches (`inlet` / `outlet` / `walls`).**
//!   [`pipeline::surface_to_tet_dual_mesh_multipatch`] carries an input
//!   surface's named regions ([`patches::SurfaceRegions`]) through to the output
//!   mesh's [`volume_mesh::BoundaryPatch`] list, so a solver `0/`
//!   boundary-condition directory can be written against the result. The stages
//!   in between rebuild the mesh through
//!   [`volume_mesh::from_cell_faces`] and cannot preserve a tag, so the
//!   assignment is recovered geometrically at the end by
//!   [`patches::assign_patches_by_region`] — each boundary face takes the region
//!   of the nearest input triangle, as snappyHexMesh does (verified: contiguous
//!   patches with correct counts and start offsets, geometrically correct
//!   membership, through the full pipeline including prism layers). The
//!   single-patch [`pipeline::surface_to_tet_dual_mesh`] is unchanged.
//!
//! Next on the `op-hzs` roadmap: exact/adaptive predicates + size-driven point
//! insertion (the rest of `op-38z`; gmsh — GPLv2+, GPLv3-compatible — is a
//! licence-clean reference); **patch-selective** layer insertion (layers are
//! currently grown over the whole boundary, inlet/outlet included, because patch
//! classification runs last); feature-edge-aware snapping and patch seams; and
//! making the adaptive layerer work on graded near-wall meshes, where it
//! currently backs off to zero thickness (the pipeline now reports that in
//! [`pipeline::TetDualReport::stage_notes`] instead of failing silently).
//!
//! ## Design rules (workspace `CLAUDE.md`)
//!
//! Index-based topology (newtype indices into `Vec`, no lifetimes/pointers);
//! enums for dispatch, never trait objects; no `Box<T>` (own by value or share
//! with `Arc<T>`); pure Rust with no BLAS/C/Fortran so the crate builds on
//! Android/Termux.
//!
//! > **Not affiliated with Creative Fields or the cfMesh project**, and not the
//! > official cfMesh software — an independent GPL fork. **Untrusted
//! > AI-assisted draft** until human-reviewed, per the workspace
//! > `RESPONSIBLE_USE.md`. For education / research / V&V only; not for reactor
//! > operation, licensing, or safety-critical decisions.

pub mod cartesian;
pub mod carve;
pub mod checks;
pub mod delaunay;
pub mod dual;
pub mod layers;
pub mod math;
pub mod octree;
pub mod patches;
pub mod pipeline;
pub mod reactor;
pub mod shapes;
pub mod smooth;
pub mod snap;
pub mod tet;
pub mod volume_mesh;

/// Bridge to the real `outram-foam-basic-lib` `PolyMesh` (feature
/// `foam-export`) — the volume-mesh output path to the CFD/TH solver. Off by
/// default so the base crate stays dependency-free and Android-buildable.
#[cfg(feature = "foam-export")]
pub mod foam;
