// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Crate root. This crate is INSPIRED BY Blender's architecture
// (github.com/blender/blender, GPL-2.0-or-later) but is not a port of it: the only
// file carrying upstream Blender code is boolean_predicates.rs, which retains its
// own provenance block. Every other module is written from published algorithms
// (cited in each module's header) or from first principles.
// Not affiliated with or endorsed by the Blender Foundation.
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

//! # outram-blender
//!
//! A pure-Rust, headless **mesh-authoring frontend** for the OUTRAM PARK
//! multiphysics suite, inspired by the **architecture** of
//! [Blender](https://github.com/blender/blender) (GPLv2-or-later, which is
//! GPLv3-compatible). It authors and procedurally generates geometry, then
//! bridges it into two OUTRAM PARK solver workflows:
//!
//! - **Monte Carlo neutron transport** (feature `mc-export`). Author a surface,
//!   fit it to an `outram-mc-libs` CSG universe ([`export`]), attach materials,
//!   and run a k-eigenvalue (criticality) calculation returning `k_eff ± σ`
//!   (the `sim` module). This path is driven by the **MC Studio** egui app
//!   (`examples/mc_studio`).
//! - **CFD / thermal-hydraulics volume meshing** (feature `foam-mesh`). Hand a
//!   closed surface to `outram-park-fork-cfmesh`'s tet→dual→boundary-layers
//!   pipeline and write out an OpenFOAM `polyMesh` (the `foam_mesh` module). This
//!   path is driven by the **Mesh Studio** egui app (`examples/mesh_studio`).
//!
//! The base authoring library (primitives, mesh operators, modifiers, procedural
//! evaluator, geometry processing) pulls in neither solver — both bridges are
//! opt-in cargo features, so the default build stays light and Android-buildable.
//!
//! > **⚠️ Not a Blender port.** Blender is millions of lines of C/C++/Python;
//! > this crate borrows its *concepts and data-structure architecture* (the
//! > BMesh half-edge topology, the mesh-operator model, the modifier stack,
//! > geometry-nodes-style procedural generation) — it does **not** port
//! > Blender's code (the only literally-ported piece is the Shewchuk robust
//! > predicates in [`boolean_predicates`], with its GPL provenance header). The
//! > algorithms here — primitives, mesh operators, subdivision (Catmull-Clark &
//! > Loop), the general CSG boolean, the sparse-solve geometry processing
//! > (Laplacian/Taubin smoothing, harmonic parameterization, ARAP deformation),
//! > QEM decimation, the modifier stack, the procedural evaluator, and the
//! > export bridges — are written from first principles and unit-tested against
//! > analytic references. See the module map for per-module status.
//! >
//! > **Not affiliated with the Blender Foundation.** "Blender" names the
//! > upstream project whose architecture inspired this work; nothing here is
//! > endorsed by or sanctioned by the Blender Foundation. See the README's
//! > "Naming & trademark" section — the maintainer decided on 2026-07-17 to
//! > keep the name `outram-blender` and mark the fork status explicitly.
//! >
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.
//!
//! ## Module map — what belongs where
//!
//! | Module | Blender analogue | Status |
//! |---|---|---|
//! | [`math`] | `blenlib` `BLI_math` vector types | **real** — a minimal pure-Rust [`math::Vec3`] |
//! | [`transform`] | `Object.matrix_world` affine placement | **real** — [`transform::Affine3`] per-vertex transform (CPU reference for the GPU kernel) |
//! | `gpu` *(desktop only)* | — (no Blender analogue) | **real** — headless `wgpu` compute (WGSL); one wired kernel (parallel affine vertex transform) with probe + graceful CPU fallback. Compiled unconditionally on desktop, absent on Android |
//! | [`mesh`] | `bmesh` (`BMVert`/`BMEdge`/`BMLoop`/`BMFace`) | **real** — index-based half-edge topology |
//! | [`selection`] | `editmesh_select.cc` / `BM_select_*` | **real** — select modes + flush; all/none/invert; box/sphere/lasso region; linked; mirror; edge/face loop, ring, boundary loop, shortest path; more/less; select similar; checker deselect; non-manifold / loose / interior-faces / faces-by-sides (GH issue #37 §A — `op-hzs.54.1`–`.4`) |
//! | [`topology`] | `bmesh_queries.cc` / `bmesh_walkers_impl.cc` | **real** — precomputed radial (edge→faces) + disk (vertex→edges) adjacency; edge-loop / edge-ring / face-loop walkers; Dijkstra + BFS path helpers |
//! | [`loop_cut`] | `editmesh_loopcut.cc` / `bmo_subdivide_edgering` | **real** — Loop Cut and Slide: N parallel loops across an edge ring, with a slide factor; quad-only, splices terminal n-gons (GH issue #37 §B) |
//! | [`knife`] | `editmesh_knife.cc` | **real** — split faces along a path of boundary-point chords (edge-split / vertex); polyline→chord resolver is follow-up (GH issue #37 §B) |
//! | [`slide`] | `transform_mode_edge_slide.cc` / `_vert_slide.cc` | **real** — position-only edge-loop / vertex slide along rail edges, consistent side propagation (GH issue #37 §B) |
//! | [`subdivide`] | `bmo_subdivide.cc` | **real** — N-cut subdivide (quad grid / tri lattice / n-gon fan) with smoothness + deterministic fractal; un-subdivide halves a quad grid (GH issue #37 §B) |
//! | [`bevel`] | `bmesh_bevel.cc` | **real** — multi-segment rounded edge bevel over [`edge_bevel`]: `segments`, `profile`, `WidthType` (offset/width/depth/percent), clamp-overlap; corner fan-filled (rounded corner patch + selected-subset are follow-up) (GH issue #37 §B) |
//! | [`extrude`] | `bmo_extrude.cc` / `editmesh_extrude.cc` | **real** — extrude individual faces (own normal), region along averaged normals, vertices, manifold; complements [`ops`]'s region/edge extrude (GH issue #37 §B) |
//! | [`merge`] | `editmesh_tools.cc` `MESH_OT_merge` | **real** — merge vertices at centre / point / first / last; collapse edges; merge-by-distance over a subset (Auto-Merge) (GH issue #37 §B) |
//! | [`rip_split`] | `editmesh_rip.cc` / `MESH_OT_separate` / `_split` | **real** — split a face group into an island; separate by selection / loose parts / group; rip a slit along edges (GH issue #37 §B) |
//! | [`bridge`] | `bmo_bridge.cc` | **real** — join two equal-length edge loops with a face strip; twist / cuts / flip / weld; ordered_ring walker (GH issue #37 §B) |
//! | [`fill`] | `bmo_grid_fill.cc` / `bmo_triangle_fill.cc` / `MESH_OT_edge_face_add` | **real** — make_face (F), grid_fill (Coons quad grid from a 4-sided loop), beauty_fill (Delaunay diagonal flips) (GH issue #37 §B) |
//! | [`dissolve`] | `bmo_dissolve.cc` / `MESH_OT_delete` | **real** — dissolve faces/edges/vertices (merge to one n-gon), limited dissolve (planar cleanup), the delete/erase matrix (GH issue #37 §B) |
//! | [`connect`] | `bmo_connect.cc` | **real** — connect vertex path / pairs (J) via knife face-chord splits (GH issue #37 §B) |
//! | [`poke_quads`] | `bmo_poke.cc` / `bmo_join_triangles.cc` | **real** — poke faces (centroid fan + offset), triangulate quads by method, tris↔quads join (GH issue #37 §B) |
//! | [`edge_tools`] | `bmo_rotate_edges.cc` / mesh_edge_flow / `MOD_edgesplit.cc` | **real** — rotate edge CW/CCW, set edge flow (loop relax), edge split operator (GH issue #37 §B) |
//! | [`transform_ops`] | `transform_mode_*.cc` | **real** — to-sphere / shear / bend / warp / push-pull / shrink-fatten / randomize / smooth-verts, position-only over a selection (GH issue #37 §C) |
//! | [`proportional`] | `transform_proportional_*` | **real** — proportional-edit falloff (smooth/sphere/root/inv-sq/sharp/linear/constant/random), Euclidean or connected-only distance (GH issue #37 §C) |
//! | [`symmetry`] | `bmo_symmetrize.cc` / `MESH_OT_symmetry_snap` | **real** — symmetrize (mirror + weld a half), snap-to-symmetry (average with partner), mirror_selection (GH issue #37 §C) |
//! | [`spin_screw`] | `bmo_spin_exec` / `MOD_screw.cc` | **real** — spin a profile selection around an axis (bridged or duplicates), screw = spin + axial translation (helix) (GH issue #37 §C) |
//! | [`transform_input`] | `transform_input.cc` / `transform_constraints.cc` | **real** — Constraint (free/axis/plane), TransformBasis (global/normal), NumericEntry with an expression evaluator (pi/tau/e, ^ right-assoc), grid-increment snap; the CAD precision-input model (GH issue #37 §D) |
//! | [`snap`] | `transform_snap.cc` / `transform_snap_object.cc` | **real** — snap to grid increment / vertex / edge-midpoint / nearest-on-edge / nearest-on-face; SnapBase closest/center/median/active; align-rotation-to-target; snap-onto-self exclusion (GH issue #37 §D) |
//! | [`cursor_pivot`] | view3d_cursor_snap / transform_orientations.cc | **real** — 3D cursor placement, PivotPoint (bbox/cursor/individual-origins/median/active), rotate/scale about pivot, custom orientation from a vertex/edge/face selection (GH issue #37 §D) |
//! | [`measure`] | mesh-statistics overlay / ruler gizmo / mesh-analysis | **real** — edge/area/angle/dihedral readouts, volume, dimensions, Ruler + Protractor, overhang / distortion / sharp-edges / self-intersection (tri-tri) / thickness (ray cast) (GH issue #37 §D) |
//! | [`primitives`] | `editors/mesh/editmesh_add` primitive add-ops | **real** — cube / UV-sphere / cylinder / grid generators (unit-tested) |
//! | [`revolve`] | Spin (`bmo_spin`) | **real** — sweep a profile polyline around an axis into a surface of revolution (pipes / vessels / cones) |
//! | [`ops`] | `bmesh/operators/*` (`bmo_*`) mesh operators | **real** — extrude / midpoint-subdivide / vertex-bevel (flat or rounded multi-segment; boolean delegates to [`boolean`]) |
//! | [`subdivision`] | OpenSubdiv / `MOD_subsurf` | **real** — Catmull-Clark surface subdivision (local stencils) |
//! | [`loop_subdivision`] | `MOD_subsurf` (triangle path) | **real** — Loop subdivision surface for triangle meshes |
//! | [`laplacian`] | `MOD_laplaciansmooth` / `bmo_smooth_laplacian` | **real** — cotangent/uniform discrete Laplacian + implicit & Taubin smoothing (first `faer` sparse solve) |
//! | [`parameterize`] | UV unwrap (harmonic map) | **real** — Tutte/harmonic planar parameterization of a disk (reuses the Laplacian sparse solve) |
//! | [`arap`] | "As Rigid As Possible" deform | **real** — ARAP handle-based deformation (local rotation fit + cotangent-Laplacian global solve) |
//! | [`decimate`] | `MOD_decimate` (Collapse) | **real** — QEM (Garland–Heckbert) edge-collapse mesh simplification |
//! | [`convex_hull`] | `bmo_convex_hull` | **real** — 3D convex hull of a point set (incremental, robust `orient3d`) |
//! | [`weld`] | `bmo_remove_doubles` / Merge by Distance | **real** — merge coincident vertices within a tolerance (grid hash + union-find) |
//! | [`fill_holes`] | `bmo_holes_fill` / Fill Holes | **real** — cap open boundary loops with a centroid fan (watertight) |
//! | [`solidify`] | `MOD_solidify` (simple) | **real** — extrude a surface into a closed shell (inner offset + rim) |
//! | [`recalc_normals`] | `normals_make_consistent` (Recalculate Outside) | **real** — repair inconsistent winding (BFS) + flip each component outward |
//! | [`triangulate`] | `bmo_triangulate` (fan) | **real** — fan-triangulate every face into a triangle-only mesh |
//! | [`inset`] | `bmo_inset` (Individual) | **real** — per-face inset: shrunk inner copy + bridging ring quads |
//! | [`bisect`] | Bisect (plane cut) | **real** — half-space clip every face by a plane (Sutherland–Hodgman); leaves the cut open |
//! | [`edge_bevel`] | Bevel (edges) | **real** — chamfer every edge (cut faces back + fill edge/corner gaps); winding fixed by `recalc_normals` |
//! | [`boolean`] | `bmo_boolean` (Manifold upstream) | **real** — CSG entry point: exact convex-`Intersect` fast path, else delegates to [`boolean_general`] |
//! | [`boolean_general`] | `mesh_boolean.cc` / `mesh_intersect.cc` arrangement | **real** — general union/difference/intersect on non-convex closed meshes (arrangement + winding classification) |
//! | [`boolean_predicates`] | `blenlib` `math_boolean.cc` (Shewchuk) | **real** — robust `orient2d/3d`, `incircle`, `insphere` (adaptive f64 + double-double fallback) |
//! | [`boolean_classify`] | `mesh_boolean.cc` inside/outside classification | **real** — point-in-closed-mesh via generalized winding number (+ ray-parity cross-check) |
//! | [`modifiers`] | `modifiers/intern/MOD_*` modifier stack | **real** — subsurf / mirror / array |
//! | [`procedural`] | Geometry Nodes (`nodes/geometry/*`) | **real** — node-graph evaluator |
//! | [`export`] | I/O exporters (`io/*`) | **real** — OpenFOAM polyMesh text + CSG fitting (box/sphere/cylinder/convex-faceted) + DAGMC faceted-solid (with an opt-in closed-2-manifold gate, [`export::to_faceted_solid_checked`]) + feature-gated real-type bridges (`foam-export`, `mc-export`) |
//! | [`stl`] | STL I/O | **real** — ASCII + binary STL read/write (surface-mesh interchange / DAGMC / Monte-Carlo feed) |
//! | `sim` *(feature `mc-export`)* | — (no Blender analogue) | **real** — Monte Carlo setup + run: build materials, bundle geometry/source/settings, run a k-eigenvalue criticality calc (`k_eff ± σ`) via `outram-mc-libs`. Backend of **MC Studio** |
//! | `foam_mesh` *(feature `foam-mesh`)* | — (no Blender analogue) | **real** — volume-meshing bridge: blender surface → `outram-park-fork-cfmesh` tet→dual→boundary-layers pipeline → OpenFOAM `polyMesh`, gated by a closed-2-manifold check on the surface. Backend of **Mesh Studio** |
//!
//! ## Design rules honoured here (workspace `CLAUDE.md`)
//!
//! - **Index-based topology, no lifetimes/pointers.** Every element is
//!   addressed by a newtype index ([`mesh::VertexId`], [`mesh::EdgeId`],
//!   [`mesh::LoopId`], [`mesh::FaceId`]) into a `Vec`, exactly as the workspace
//!   forbids `&'a`-linked graph nodes.
//! - **Enums for dispatch, never trait objects.** The operator, modifier, and
//!   procedural-node sets are closed and enumerated ([`ops::MeshOp`],
//!   [`modifiers::Modifier`], [`procedural::GeometryNode`]).
//! - **No `Box<T>`; `Arc<T>` for sharing.** Owned meshes are passed by value;
//!   shared read-only meshes use `std::sync::Arc`.
//!
//! ## Where to start reading
//!
//! [`primitives`] is the primary entry point. Read [`primitives::cube`]
//! top-to-bottom, then the [`mesh::Mesh`] type it builds on; from there the
//! [`ops::MeshOp`] enum is the map of what you can *do* to a mesh (extrude,
//! bevel, boolean, smooth, decimate, subdivide, ARAP-deform).
//!
//! ```
//! use outram_blender::primitives;
//!
//! // A unit cube centred at the origin: 8 vertices, 12 edges, 6 quad faces.
//! let cube = primitives::cube(1.0);
//! assert_eq!(cube.vertex_count(), 8);
//! assert_eq!(cube.edge_count(), 12);
//! assert_eq!(cube.face_count(), 6);
//! // Euler characteristic of a closed genus-0 surface: V - E + F = 2.
//! assert_eq!(cube.euler_characteristic(), 2);
//! ```

pub mod arap;
pub mod bisect;
pub mod bevel;
pub mod boolean;
pub mod boolean_classify;
pub mod boolean_general;
pub mod boolean_predicates;
pub mod bridge;
pub mod connect;
pub mod convex_hull;
pub mod cursor_pivot;
pub mod decimate;
pub mod dissolve;
pub mod edge_bevel;
pub mod edge_tools;
pub mod export;

/// Monte Carlo simulation setup + run (feature `mc-export`) — build materials,
/// bundle geometry + source + settings, run a k-eigenvalue criticality
/// calculation. The backend the MC Studio GUI drives.
#[cfg(feature = "mc-export")]
pub mod sim;

/// Volume-meshing bridge (feature `foam-mesh`) — hand a blender surface [`mesh::Mesh`]
/// (or a built-in primitive) to `outram-park-fork-cfmesh`'s tet→dual→boundary-layers
/// `pipeline`, get back a polyhedral `VolumeMesh` + quality report, and export an
/// OpenFOAM `polyMesh`. The backend the Mesh Studio GUI drives.
#[cfg(feature = "foam-mesh")]
pub mod foam_mesh;
pub mod extrude;
pub mod fill;
pub mod fill_holes;
pub mod inset;
pub mod knife;
pub mod laplacian;
pub mod loop_cut;
pub mod loop_subdivision;
pub mod math;
pub mod measure;
pub mod merge;
pub mod mesh;
pub mod parameterize;
pub mod modifiers;
pub mod ops;
pub mod poke_quads;
pub mod primitives;
pub mod procedural;
pub mod reactor;
pub mod proportional;
pub mod recalc_normals;
pub mod rip_split;
pub mod revolve;
pub mod selection;
pub mod slide;
pub mod snap;
pub mod solidify;
pub mod spin_screw;
pub mod stl;
pub mod subdivide;
pub mod subdivision;
pub mod symmetry;
pub mod topology;
pub mod transform;
pub mod transform_input;
pub mod transform_ops;
pub mod triangulate;
pub mod weld;

/// Heavy linear-algebra backend for the *large* mesh solves the advanced
/// operators will need — Laplacian mesh editing, ARAP deformation, and mesh
/// parameterization all build a sparse Laplacian over the mesh and solve
/// `A x = b`. Re-exports [`faer`], a pure-Rust, Android-safe dense **and**
/// sparse linear-algebra library (SIMD via `pulp`, no system BLAS).
///
/// **Division of labour:** per-element geometry math (positions, normals,
/// transforms) stays in the fixed-size [`math`] types — small, fast, no
/// allocation. `faer` is only for the big systems. For interactive editing
/// (same matrix, many right-hand sides) prefer `faer`'s sparse **Cholesky**
/// factorization over an iterative solve. An *optional* bridge to
/// `outram-foam-basic-lib`'s CG/GAMG iterative solvers is tracked separately for
/// large one-off sparse solves (see beads `op-hzs`).
///
/// **First consumer:** [`laplacian`] — the cotangent/uniform discrete Laplacian
/// and implicit Laplacian smoothing assemble a sparse system and solve it with
/// `faer`'s sparse Cholesky. Future ARAP / parameterization operators reuse the
/// same path.
pub use faer;

/// Headless GPU compute via `wgpu`. Compiled **unconditionally on every desktop
/// target** (no cargo feature to opt in) so the GPU path is used as far as
/// possible; **absent only on Android** (`target_os = "android"`), which has no
/// system Vulkan/Metal loader and where the workspace Android rule forbids GPU
/// deps in the library build. Whether or not this module is present, callers get
/// a graceful CPU fallback: on Android the GPU attempt is compiled out entirely,
/// and on desktop [`gpu::probe`] returning `None` or a recoverable
/// [`gpu::GpuError`] routes to the CPU reference path. See
/// [`transform::Affine3::transform_points_best_effort`] for the unified
/// try-GPU-then-CPU entry point, and [`gpu`] for the fallback contract.
#[cfg(not(target_os = "android"))]
pub mod gpu;
