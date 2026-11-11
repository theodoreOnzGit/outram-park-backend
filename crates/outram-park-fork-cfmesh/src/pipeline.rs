// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Provenance: original OUTRAM PARK code. This file introduces no geometry
// algorithm of its own — it composes this crate's stages into one call and adds
// the graceful-degradation policy. The *staging* it composes mirrors cfMesh's
// `cartesianMesh` workflow and OpenFOAM snappyHexMesh's
// castellate -> snap -> addLayers driver:
//   cfMesh, Copyright (C) 2014-2017 Creative Fields, Ltd., GPL-3.0-only
//   OpenFOAM, Copyright (C) 2011-2016 OpenFOAM Foundation
//              Copyright (C) 2016-2023 OpenCFD Ltd., GPL-3.0-only
// Per-stage provenance is recorded in each stage's own module.
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

//! **High-level meshing pipeline** — one call turns a closed triangulated
//! surface into a polyhedral volume mesh with near-wall prism boundary layers,
//! via the **tetrahedral → dual** path.
//!
//! This is the coarse-grained entry point the `outram-blender` "mesh studio"
//! GUI (and any programmatic caller) uses instead of hand-wiring the individual
//! `carve → snap → tet → dual → layers` stages. It composes the crate's existing
//! primitives; it introduces no new geometry algorithm of its own.
//!
//! # The tet → dual path
//!
//! ```text
//!   surface        carve_box        snap_to_surface     tetrahedralize
//!   (tri soup) ──► (hex bg mesh) ──► (body-fit bndry) ──► (all-tet mesh)
//!                                                              │
//!                    add_boundary_layers_adaptive   polyhedral_dual_min_faces
//!   polyhedral   ◄── (prism wall layers)         ◄── (one cell / tet vertex)
//!   volume mesh
//! ```
//!
//! Taking the polyhedral **dual of a tetrahedralization** (rather than the dual
//! of the raw hex carve) is the classic route to a well-connected polyhedral
//! mesh: every tet vertex — the primal corners plus the per-face and per-cell
//! centroids the tetrahedralizer inserts — becomes one polyhedral cell, so the
//! interior packs many neighbours per cell for better gradient reconstruction at
//! a low cell count, the same idea as OpenFOAM's `polyDualMesh` fed a tet mesh.
//!
//! # Graceful stage degradation
//!
//! Curved geometry does not always survive every stage: a coarse snap can tangle
//! a wall cell, a dual can fail on a non-star-shaped cell, and a fixed prism
//! march can self-intersect. Rather than return a broken mesh, each optional
//! stage here is **applied only if its result is still acceptable** — closed
//! ([`VolumeMesh::validate`]) *and* free of negative-volume cells
//! ([`check_quality`](crate::checks::check_quality)). If a stage would break
//! that, it is **skipped**, the previous mesh is kept, and a human-readable line
//! is appended to [`TetDualReport::stage_notes`] so the caller (and the GUI) can
//! show exactly what ran and what was skipped. The returned mesh is therefore
//! always valid and exportable. This mirrors the `mesh_studio` example's
//! degradation approach, lifted into a reusable library entry point.
//!
//! The negative-cell guard is deliberately stricter than the bare `validate()`
//! check the `mesh_studio` example uses: a mesh can be *closed* yet still be
//! tangled (a cell folded through itself sums its oriented face areas to zero but
//! has negative volume), and such a mesh is unusable by a solver, so the
//! pipeline refuses to accept a stage that produces one.
//!
//! # Units and conventions
//!
//! All lengths are in **metres**: [`TetDualOptions::cell_size`] (background
//! Cartesian edge), [`TetDualOptions::first_layer_thickness`] (first prism layer,
//! wall-normal). [`TetDualOptions::expansion`] is the dimensionless layer-to-layer
//! growth ratio (`>= 1`). Angles in [`TetDualReport`] are in **degrees**. Volume
//! ([`TetDualReport::total_volume`]) is in **cubic metres**.
//!
//! # Scope and trust
//!
//! **Untrusted AI-assisted draft pending human V&V.** The V&V here is
//! *verification* — valid mesh topology (closed cells, no inverted cells) and
//! volume conservation versus the analytic volume of the built-in primitives —
//! **not** *validation* against a CFD/TH solve. See the module tests for
//! methodology and measured results. Pure Rust, no dependencies, Android-safe.

use crate::carve::carve_box;
use crate::checks::check_quality;
use crate::delaunay::flip_to_delaunay;
use crate::dual::{polyhedral_dual, polyhedral_dual_min_faces, polyhedral_dual_per_point_with, DualKind, FeaturePointRule};
use crate::boundary_layer_optimisation::{add_boundary_layers_cfmesh_on_surface, BoundaryLayerOptimisationControls};
use crate::layers::{add_boundary_layers_back_off, LayerOutcome, LayerQualityStrategy};
use crate::snappy_layers::{add_boundary_layers_snappy, SnappyLayerControls};
use crate::math::Vec3;
use crate::octree::refine_near_boundary_banded;
use crate::patches::{assign_patches_by_region, SurfaceRegions};
use crate::shapes::{box_surface, cylinder_surface, sphere_surface};
use crate::smooth::laplacian_smooth;
use crate::snap::snap_to_surface;
use crate::tet::tetrahedralize;
use crate::volume_mesh::VolumeMesh;

/// cfMesh's `tetMesh` workflow (`tetMeshGenerator`, gh:#1272), beside
/// [`surface_to_tet_dual_mesh`].
pub mod tet_mesh;
/// cfMesh `cartesianMesh` (gh:#1274).
pub mod cartesian_mesh;
/// cfMesh `cartesian2DMesh` (gh:#1275).
pub mod cartesian_2d_mesh;
/// cfMesh's `pMesh` workflow (`voronoiMeshGenerator`, gh:#1273).
pub mod p_mesh;

/// Tuning knobs for [`surface_to_tet_dual_mesh`] and the primitive wrappers.
///
/// Every optional stage can be turned off. Lengths are in **metres**, angles in
/// degrees, and [`Self::expansion`] is dimensionless. Construct with
/// [`TetDualOptions::default`] and override the fields you care about.
#[derive(Debug, Clone, PartialEq)]
pub struct TetDualOptions {
    /// Background Cartesian **cell edge length**, in metres. Sets the base
    /// resolution: smaller means more, finer cells (and slower meshing). Must be
    /// `> 0`; a value that carves zero cells is a hard error.
    ///
    /// With [`Self::refinement_levels`] `> 0` this is the **coarse interior**
    /// edge; the near-wall cells end up `cell_size / 2^refinement_levels`.
    pub cell_size: f64,
    /// Octree **near-wall refinement depth** (dimensionless count of levels).
    ///
    /// `0` (the default) carves a *uniform* grid at [`Self::cell_size`] — the
    /// crate's original behaviour, byte-for-byte. `L > 0` instead carves a
    /// **graded** background mesh ([`refine_near_boundary_banded`]): the interior
    /// stays at `cell_size`, and cells near the surface are split up to `L`
    /// levels finer, so the wall is resolved at `cell_size / 2^L` metres while
    /// the interior is not. Each level roughly halves the local wall spacing;
    /// practical values are `1`-`2`.
    ///
    /// Grading is what makes a given wall resolution affordable — see the module
    /// tests for measured cell-count / accuracy trade-offs on a sphere.
    pub refinement_levels: u8,
    /// Width of the refinement band, **dimensionless — in multiples of the
    /// candidate cell's own edge length** (not metres).
    ///
    /// A cell is split toward the next level iff its centre lies within
    /// `refinement_band x (its own edge)` of the input surface. Because the band
    /// scales with the cell, the refined region is a graded shell hugging the
    /// wall. `1.0` (the default) refines roughly the cells that touch the
    /// surface; `2.0` spreads the transition over two cells (gentler size jumps,
    /// more cells). Ignored when [`Self::refinement_levels`] is `0`; a
    /// non-positive value refines nothing.
    pub refinement_band: f64,
    /// If `true`, project the carved staircase boundary onto the input surface
    /// ([`snap_to_surface`]) to body-fit it. Skipped (with a note) if it would
    /// tangle a wall cell.
    pub snap: bool,
    /// If `true`, improve the tetrahedralization toward Delaunay by bistellar
    /// flips ([`flip_to_delaunay`]) before taking the dual. Safe: the flipper is
    /// improve-or-noop, so this never makes the mesh worse.
    pub delaunay: bool,
    /// Flip budget passed to [`flip_to_delaunay`] when [`Self::delaunay`] is set.
    pub max_flips: usize,
    /// If `true`, take the polyhedral dual of the tet mesh (the "→ dual" step).
    /// If `false`, the returned mesh is the tetrahedralization itself.
    pub dual: bool,
    /// Prefer the **face-minimal** dual ([`polyhedral_dual_min_faces`]); on
    /// failure the pipeline falls back to the robust quad-fan
    /// [`polyhedral_dual`], then to skipping the dual. Ignored if [`Self::dual`]
    /// is `false`.
    pub dual_min_faces: bool,
    /// **Which dual** the dual stage builds (gh:#1240):
    /// [`DualKind::MedianPerCorner`] (the default, unchanged),
    /// [`DualKind::MedianPerPoint`] (one boundary face per point per sector,
    /// `polyDualMesh`'s boundary rule on the median dual) or
    /// [`DualKind::CellCentre`] (`polyDualMesh` itself, feature
    /// `cell-centre-dual`). With named regions
    /// ([`surface_to_tet_dual_mesh_multipatch`]) the two new kinds classify
    /// the tet boundary into the regions first, so the patch seams are
    /// feature edges, as upstream's. [`Self::dual_min_faces`] applies to
    /// `MedianPerCorner` only.
    pub dual_kind: DualKind,
    /// Feature angle of the two new dual kinds, in degrees: a boundary edge
    /// whose faces' normals differ by more is kept sharp. `45` by default,
    /// the `polyDualMesh` CLI's (`outram-foam-cli`'s
    /// `DEFAULT_FEATURE_ANGLE_DEG`; upstream's utility takes it as a
    /// required argument).
    pub dual_feature_angle_deg: f64,
    /// Which boundary points the two new dual kinds keep as dual vertices
    /// ([`FeaturePointRule`]): every point on a feature edge (the default,
    /// OpenFOAM's `polyDualMesh` utility) or only the corners where more than
    /// two feature edges meet (the `polyDualMesh` class).
    pub dual_feature_points: FeaturePointRule,
    /// Smart-Laplacian smoothing passes ([`laplacian_smooth`]) applied to the
    /// interior after the dual and before the layers. `0` disables it. Safe:
    /// smoothing never inverts a cell and conserves volume exactly.
    pub smooth_passes: usize,
    /// Number of prism **boundary layers** to grow on the wall patch
    /// ([`add_boundary_layers_adaptive`]). `0` disables the layer stage.
    pub n_layers: usize,
    /// First (wall-nearest) prism layer thickness, in **metres** (wall-normal).
    /// Subsequent layers grow by [`Self::expansion`]. The adaptive layerer may
    /// reduce the achieved thickness to keep the mesh valid on tight curvature.
    pub first_layer_thickness: f64,
    /// Geometric layer-to-layer growth ratio (dimensionless, `>= 1`): layer `i`
    /// is `expansion^i × first_layer_thickness` thick.
    pub expansion: f64,
    /// Name of the boundary patch to grow layers on. The crate's meshers place
    /// all exposed faces in a single `"walls"` patch, which is the default.
    pub wall_patch: String,
    /// **Patch-selective layers** (gh:#1202), for
    /// [`surface_to_tet_dual_mesh_multipatch`] only: the named regions to grow
    /// layers on, e.g. `["walls"]` for a duct whose `inlet` and `outlet` must
    /// stay layer-free. The boundary is then classified into the regions
    /// *before* the layer stage, layers grow on these patches only
    /// ([`add_boundary_layers_adaptive_on_patches`], the port of cfMesh's
    /// per-patch `boundaryLayers`), and the boundary is classified again at the
    /// end. Empty (the default) keeps the historical behaviour: layers on the
    /// whole boundary, as [`Self::wall_patch`] says. Ignored by the
    /// single-patch entry points, whose boundary is one patch.
    pub layer_patches: Vec<String>,
    /// **Feature-edge capture** (gh:#1202), for
    /// [`surface_to_tet_dual_mesh_multipatch`] only: after the dual (and any
    /// smoothing) and before the layers, map the boundary vertices onto the
    /// surface patch by patch and onto the feature edges between patches
    /// ([`crate::surface_mapper::map_vertices_onto_surface_patches`], cfMesh's
    /// `meshSurfaceEdgeExtractorNonTopo`), so a pipe's rim is sharp and its
    /// inlet spans the section. **On by default**; `false` is the explicit
    /// ablation (the chamfered boundary the pipeline gave before).
    pub feature_edges: bool,
    /// **cfMesh's `correctEdgesBetweenPatches`** (gh:#1241) before the
    /// feature-edge capture: split the boundary faces that would fold on a
    /// feature line, and their cells into pyramids
    /// ([`crate::correct_edges::correct_edges_between_patches`]); the capture
    /// then reads the patches it was given. Upstream always runs it; **off
    /// by default here** so the recorded meshes (and Foam Studio's
    /// defaults) are unchanged until the maintainer decides. Ignored unless
    /// [`Self::feature_edges`] is on.
    pub correct_edges: bool,
    /// **cfMesh's `meshSurfaceOptimizer` in the feature-edge capture**
    /// (gh:#1259): instead of undoing the moves that tangle a cell
    /// (`revert_tangled`), run upstream's surface steps in
    /// `tetMeshGenerator`'s order: `mapMeshToSurface` (every boundary point
    /// to the surface, then `untangleSurface`,
    /// [`crate::surface_optimizer::map_mesh_to_surface`]), the patch
    /// mapping (`mapVerticesOntoSurfacePatches`, after
    /// [`Self::correct_edges`] when that is on), then `optimiseMeshSurface`
    /// (`optimizeSurface`, [`crate::surface_optimizer::optimise_mesh_surface`]).
    /// **Off by default** (the recorded meshes are unchanged). Ignored
    /// unless [`Self::feature_edges`] is on.
    pub untangle_surface: bool,
    /// **cfMesh's `optimiseFinalMesh`** (gh:#1283,
    /// [`crate::final_mesh::optimise_final_mesh`]), for
    /// [`surface_to_tet_dual_mesh_multipatch`] only: `optimizeSurface`,
    /// `optimizeMeshFV`, `optimizeLowQualityFaces`, `untangleMeshFV`, with
    /// the boundary moves, Laplace pass, LP untangler and projection that
    /// [`Self::cfmesh_layer_optimisation`] sets. On a mesh with no
    /// `CfMeshOptimise` layer it runs after the feature-edge capture; the
    /// `CfMeshOptimise` layer stage runs it itself, and with this on it also
    /// starts with `optimizeSurface`, as upstream. The result is kept only
    /// if valid. **Off by default** (the recorded meshes are unchanged).
    pub optimise_final_mesh: bool,
    /// **How the layer stage meets upstream's mesh-quality limits**
    /// (gh:#1215): [`LayerQualityStrategy::BackOffOnly`] (the default, this
    /// crate's global uniform back-off, unchanged),
    /// [`LayerQualityStrategy::CfMeshOptimise`] (cfMesh's local thinning and
    /// untangling) or [`LayerQualityStrategy::SnappyCollapse`]
    /// (snappyHexMesh's local layer collapse). Every strategy starts from the
    /// same columns ([`crate::layers`]).
    pub layer_strategy: LayerQualityStrategy,
    /// Settings of [`LayerQualityStrategy::SnappyCollapse`] (upstream's
    /// `addLayersControls` and quality defaults).
    pub snappy_layers: SnappyLayerControls,
    /// Settings of [`LayerQualityStrategy::CfMeshOptimise`] (cfMesh's
    /// `boundaryLayerOptimisation` defaults).
    pub cfmesh_layer_optimisation: BoundaryLayerOptimisationControls,
}

impl Default for TetDualOptions {
    /// Sensible defaults for a metre-scale primitive (e.g. a radius-3 m sphere):
    /// `cell_size = 0.6 m`, **uniform** (`refinement_levels = 0`), snap on,
    /// Delaunay improve on (`max_flips = 20_000`), face-minimal dual on, no extra
    /// smoothing, `3` boundary layers of first thickness `0.04 m` at expansion
    /// `1.3`, grown on the `"walls"` patch. Scale [`Self::cell_size`] and
    /// [`Self::first_layer_thickness`] to your geometry.
    ///
    /// Refinement defaults **off** so the default mesh is the uniform one this
    /// crate has always produced; opt in with [`Self::refinement_levels`].
    fn default() -> Self {
        TetDualOptions {
            cell_size: 0.6,
            refinement_levels: 0,
            refinement_band: 1.0,
            snap: true,
            delaunay: true,
            max_flips: 20_000,
            dual: true,
            dual_min_faces: true,
            dual_kind: DualKind::MedianPerCorner,
            dual_feature_angle_deg: 45.0,
            dual_feature_points: FeaturePointRule::OnFeatureEdge,
            smooth_passes: 0,
            n_layers: 3,
            first_layer_thickness: 0.04,
            expansion: 1.3,
            wall_patch: "walls".to_string(),
            layer_patches: Vec::new(),
            feature_edges: true,
            correct_edges: false,
            untangle_surface: false,
            optimise_final_mesh: false,
            layer_strategy: LayerQualityStrategy::BackOffOnly,
            snappy_layers: SnappyLayerControls::default(),
            cfmesh_layer_optimisation: BoundaryLayerOptimisationControls::default(),
        }
    }
}

/// What the pipeline produced and how it got there — enough for the GUI to show
/// the outcome and which stages were skipped.
///
/// Volumes are in cubic metres, angles in degrees. [`Self::stage_notes`] carries
/// one line per gracefully-degraded (skipped) stage; an empty list means every
/// requested stage ran.
#[derive(Debug, Clone, PartialEq)]
pub struct TetDualReport {
    /// One human-readable line per stage that was skipped to keep the mesh valid
    /// (e.g. `"snap-to-surface skipped — it would invert a wall cell"`). Empty if
    /// nothing was skipped.
    pub stage_notes: Vec<String>,
    /// Final cell count.
    pub cell_count: usize,
    /// Final total enclosed volume, in cubic metres ([`VolumeMesh::total_volume`]).
    pub total_volume: f64,
    /// `true` iff the final mesh passes [`VolumeMesh::validate`] (closed cells,
    /// in-range addressing). The pipeline errors out rather than returning a
    /// mesh for which this is `false`, so on `Ok` this is always `true`.
    pub valid: bool,
    /// Maximum face **non-orthogonality**, in degrees (see [`check_quality`]).
    /// Boundary-layer meshes legitimately exceed `checkMesh`'s 70° warning (see
    /// the module tests); interpret against a looser near-wall bound.
    pub max_non_orthogonality_deg: f64,
    /// Maximum face **skewness** (dimensionless; see [`check_quality`]).
    pub max_skewness: f64,
    /// Number of negative-volume cells in the final mesh — `0` for an accepted
    /// result (the acceptance gate rejects any stage that would raise it).
    pub n_negative_volume_cells: usize,
    /// What the layer stage did (gh:#1215): per wall point and face, the
    /// layers and thickness each got, under the strategy used. `None` when
    /// the stage did not run.
    pub layers: Option<LayerOutcome>,
}

/// Grow the layers on `patches` with `opts.layer_strategy` (gh:#1215).
fn grow_layers(mesh: &VolumeMesh, patches: &[&str], opts: &TetDualOptions, surface: Option<(&[Vec3], &[[usize; 3]], &SurfaceRegions)>) -> (VolumeMesh, LayerOutcome) {
    let (n, t, e) = (opts.n_layers, opts.first_layer_thickness.max(1e-6), opts.expansion.max(1.0));
    match opts.layer_strategy {
        LayerQualityStrategy::BackOffOnly => add_boundary_layers_back_off(mesh, patches, n, t, e),
        LayerQualityStrategy::CfMeshOptimise => {
            // gh:#1283: optimiseFinalMesh starts with optimizeSurface.
            let c = BoundaryLayerOptimisationControls { optimize_surface_first: opts.cfmesh_layer_optimisation.optimize_surface_first || opts.optimise_final_mesh, ..opts.cfmesh_layer_optimisation.clone() };
            add_boundary_layers_cfmesh_on_surface(mesh, patches, n, t, e, &c, surface)
        }
        LayerQualityStrategy::SnappyCollapse => add_boundary_layers_snappy(mesh, patches, n, t, e, &opts.snappy_layers),
    }
}

/// One stage note summarising a non-default layer strategy's outcome.
fn layer_note(o: &LayerOutcome) -> String {
    format!(
        "boundary layers ({:?}): {:.1} % of wall faces with all {} layers, {:.1} % of wall points with none, mean total thickness {:.3e} m; {}",
        o.strategy,
        100.0 * o.full_coverage(),
        o.n_layers_requested,
        100.0 * o.collapsed_point_fraction(),
        o.mean_total_thickness(),
        o.notes.join(" | ")
    )
}

/// [`DualKind::CellCentre`]'s dual: `outram-foam-mesh`'s `polyDualMesh`
/// port (feature `cell-centre-dual`), or an error naming the feature.
#[cfg(feature = "cell-centre-dual")]
fn cell_centre(mesh: &VolumeMesh, feature_angle_deg: f64, rule: FeaturePointRule) -> Result<VolumeMesh, String> {
    crate::cell_centre_dual::cell_centre_dual_with(mesh, feature_angle_deg, rule)
}

/// Without the `cell-centre-dual` feature: the error the pipeline turns
/// into its fall-back note.
#[cfg(not(feature = "cell-centre-dual"))]
fn cell_centre(_mesh: &VolumeMesh, _feature_angle_deg: f64, _rule: FeaturePointRule) -> Result<VolumeMesh, String> {
    Err("this build has no `cell-centre-dual` feature".into())
}

/// Is `mesh` acceptable as a stage output — closed, free of inverted cells,
/// and free of degenerate faces?
///
/// This is the graceful-degradation gate: a stage's result is kept only if it
/// passes this, otherwise the previous mesh is retained. Stronger than a bare
/// `validate()` because a tangled cell can be closed yet negative-volume.
///
/// **gh:#1286 (2026-11-05):** also refused, a mesh with a non-finite point
/// or a face whose area is below `VSMALL` (cfMesh's
/// `polyMeshGenChecks::checkFaceAreas(mesh, false, VSMALL)`, the check
/// upstream's `meshOptimizer::findBadFaces` runs; ported as
/// [`crate::poly_mesh_gen_checks::check_face_areas`]). A zero-area face
/// has no normal, so its non-orthogonality is NaN and the FV solve cannot
/// use it; the gate checked closure and cell volumes only, and kept such a
/// mesh (the Foam Studio pipe, h 4 mm, `CellCentre` + `Corners`, boundary
/// moves + `BoundaryProjection::EachPass`: two collinear inlet-plane
/// triangles). Upstream has no acceptance gate (it flags the face as bad
/// and goes on); this one is the port's.
fn acceptable(mesh: &VolumeMesh) -> bool {
    mesh.validate().is_ok() && check_quality(mesh).n_negative_volume_cells == 0 && faces_are_non_degenerate(mesh)
}

/// gh:#1286: every point finite, and no face with area below `VSMALL`
/// ([`crate::poly_mesh_gen_checks::check_face_areas`]; a non-finite
/// area would compare false there, so points are checked first).
pub(crate) fn faces_are_non_degenerate(mesh: &VolumeMesh) -> bool {
    mesh.points.iter().all(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite())
        && !crate::poly_mesh_gen_checks::check_face_areas(mesh, 1e-300, None).0
}

/// Keep `candidate` iff it is [`acceptable`], else fall back to `current`.
/// Returns `(mesh, kept)` — `kept` is `false` when the stage was degraded away.
fn keep_if_ok(current: VolumeMesh, candidate: VolumeMesh) -> (VolumeMesh, bool) {
    if acceptable(&candidate) {
        (candidate, true)
    } else {
        (current, false)
    }
}

/// Turn a **closed, watertight, outward-wound** triangulated surface into a
/// polyhedral volume mesh with near-wall prism boundary layers, via the
/// tetrahedral → dual path. This is the crate's high-level meshing entry point.
///
/// # Inputs
///
/// - `points` — surface vertex positions, in metres.
/// - `tris` — triangle vertex-index triples into `points`; the surface must be
///   closed and consistently **outward**-wound (as produced by
///   [`crate::shapes`]). A non-watertight or inward-wound soup gives an
///   ill-defined inside test and is not supported.
/// - `opts` — [`TetDualOptions`] (resolution, which stages to run, layer spec).
///
/// # Pipeline (each optional stage gated by the private `acceptable` check —
/// closed *and* no inverted cells; see the module docs)
///
/// 1. **Carve** a background hex mesh of the surface interior
///    ([`carve_box`]) — the only mandatory stage; zero carved cells is an error.
/// 2. **Snap** the boundary onto the surface ([`snap_to_surface`]) if
///    `opts.snap`.
/// 3. **Tetrahedralize** ([`tetrahedralize`]); if the result is unacceptable the
///    mesh stays un-tetrahedralized (the dual then runs on the hex mesh).
/// 4. **Delaunay-improve** the tets ([`flip_to_delaunay`]) if `opts.delaunay`.
/// 5. **Dual** ([`polyhedral_dual_min_faces`] preferred, then
///    [`polyhedral_dual`], then skip) if `opts.dual`.
/// 6. **Smooth** ([`laplacian_smooth`]) `opts.smooth_passes` passes.
/// 7. **Boundary layers** ([`add_boundary_layers_adaptive`]) if
///    `opts.n_layers > 0`, on `opts.wall_patch`.
///
/// # Returns
///
/// `Ok((mesh, report))` with a **valid** mesh (guaranteed to pass
/// [`VolumeMesh::validate`]) and a [`TetDualReport`] describing the outcome and
/// any skipped stages. `Err(msg)` only when meshing cannot start or finish at all
/// (`cell_size <= 0`, the carve produced zero cells, or — should not happen — the
/// final mesh is somehow not closed).
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface,
///     pipeline::{surface_to_tet_dual_mesh, TetDualOptions}};
///
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let opts = TetDualOptions { cell_size: 0.5, first_layer_thickness: 0.02, ..Default::default() };
/// let (mesh, report) = surface_to_tet_dual_mesh(&p, &t, &opts).unwrap();
///
/// assert!(mesh.validate().is_ok());
/// assert!(report.valid);
/// // A box survives every stage exactly: volume is conserved to 1 m³.
/// assert!((report.total_volume - 1.0).abs() < 1e-9);
/// ```
pub fn surface_to_tet_dual_mesh(
    points: &[Vec3],
    tris: &[[usize; 3]],
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    run_pipeline(points, tris, opts, None, true)
}

/// The same pipeline as [`surface_to_tet_dual_mesh`], but carrying the input
/// surface's **named regions** through to the output mesh's boundary patches —
/// so the result has an `inlet` / `outlet` / `walls` split a solver case can be
/// set up against, instead of one undifferentiated `walls` patch.
///
/// # Why a separate entry point
///
/// Every stage after the carve rebuilds the mesh through
/// [`crate::volume_mesh::from_cell_faces`], which recovers connectivity by
/// matching face vertex sets and therefore cannot preserve a patch tag; the
/// boundary-layer stage additionally *creates* boundary faces that no input face
/// corresponds to. The names are therefore recovered **geometrically at the
/// end**, by [`assign_patches_by_region`]: each boundary face of the finished
/// mesh takes the region of the input triangle nearest its centroid. See
/// [`crate::patches`] for the rationale and the limits of that.
///
/// # Inputs
///
/// - `points` / `tris` — as [`surface_to_tet_dual_mesh`]: a closed, watertight,
///   outward-wound surface, positions in metres.
/// - `regions` — one region index per triangle plus the patch names; build it
///   with [`SurfaceRegions::from_labels`]. Validated before meshing starts.
/// - `opts` — [`TetDualOptions`], exactly as for the single-patch entry point.
///
/// # Which patches get layers
///
/// ~~**Limitation — layers are grown on the whole boundary.** Because
/// classification happens after stage 7, the boundary-layer stage still sees
/// the single `"walls"` patch and grows prisms over the **entire** boundary,
/// inlet and outlet included. That is a valid mesh, but it is not
/// snappyHexMesh's per-patch `nSurfaceLayers` behaviour; patch-selective layer
/// insertion is future work.~~ **CORRECTED 2026-11-04 (gh:#1202):** set
/// [`TetDualOptions::layer_patches`] (e.g. `["walls"]`) and the boundary is
/// named *before* stage 7, layers grow on those patches only (the port of
/// cfMesh's per-patch `boundaryLayers`,
/// [`add_boundary_layers_adaptive_on_patches`]), and the openings stay flat
/// and layer-free. With `layer_patches` empty (the default) the old behaviour
/// is kept: prisms over the whole boundary, inlet and outlet included.
///
/// # Returns
///
/// As [`surface_to_tet_dual_mesh`], plus the patches on the returned mesh. A
/// region that no boundary face landed on yields **no** patch, so check
/// `mesh.patches` rather than assuming one patch per region. `Err` also when
/// `regions` does not describe `tris`.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cfmesh::{math::Vec3, shapes::box_surface,
///     patches::SurfaceRegions,
///     pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions}};
///
/// // box_surface emits two triangles per side in the order -Z, +Z, -Y, +Y, +X, -X.
/// let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
/// let regions = SurfaceRegions::from_labels(&[
///     "walls", "walls", "walls", "walls", "walls", "walls",
///     "walls", "walls", "outlet", "outlet", "inlet", "inlet",
/// ]);
/// let opts = TetDualOptions { cell_size: 0.5, n_layers: 0, ..Default::default() };
/// let (mesh, _report) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &opts).unwrap();
///
/// let names: Vec<&str> = mesh.patches.iter().map(|q| q.name.as_str()).collect();
/// assert!(names.contains(&"inlet") && names.contains(&"outlet") && names.contains(&"walls"));
/// ```
pub fn surface_to_tet_dual_mesh_multipatch(
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    regions.validate(tris.len())?;
    run_pipeline(points, tris, opts, Some(regions), true)
}

/// Shared implementation of the two public entry points. `regions = None` is the
/// historical single-`"walls"` behaviour (no classification pass at all, so it
/// costs nothing); `Some(_)` adds the final [`assign_patches_by_region`] step.
///
/// `upper_triangular` (gh:#1285) is `true` on every public route: the last
/// step puts the faces in OpenFOAM's upper-triangular order
/// ([`crate::renumber_mesh::upper_triangular`]). `false` is the pre-gh:#1285
/// face order, kept for the test that compares the two.
pub(crate) fn run_pipeline(
    points: &[Vec3],
    tris: &[[usize; 3]],
    opts: &TetDualOptions,
    regions: Option<&SurfaceRegions>,
    upper_triangular: bool,
) -> Result<(VolumeMesh, TetDualReport), String> {
    if opts.cell_size <= 0.0 {
        return Err(format!("cell_size must be > 0 (got {})", opts.cell_size));
    }
    let mut notes: Vec<String> = Vec::new();

    // Stage 1 — carve (mandatory). Uniform at `cell_size`, or octree-graded
    // toward the wall when `refinement_levels > 0`.
    let mut mesh = if opts.refinement_levels > 0 {
        refine_near_boundary_banded(
            points,
            tris,
            opts.cell_size,
            opts.refinement_levels,
            opts.refinement_band,
        )
    } else {
        carve_box(points, tris, opts.cell_size)
    };
    if mesh.cell_count() == 0 {
        return Err("carve produced 0 cells — try a smaller cell_size or check the surface is closed and outward-wound".into());
    }

    // Stage 2 — snap to the surface.
    if opts.snap {
        let snapped = snap_to_surface(&mesh, points, tris);
        let (m, kept) = keep_if_ok(mesh, snapped);
        mesh = m;
        if !kept {
            notes.push(
                "snap-to-surface skipped — it would tangle a wall cell on this geometry".into(),
            );
        }
    }

    // Stage 3 — tetrahedralize. (The tet mesh is a fresh mesh; if it is not
    // acceptable — e.g. a snapped, non-star-shaped cell produced inverted tets —
    // keep the pre-tet mesh so the dual/layer stages still run on a valid mesh.)
    if opts.dual || opts.n_layers > 0 || opts.delaunay {
        // Only bother tetrahedralizing if a later stage benefits; otherwise the
        // caller wanted just the carve/snap, which we already have.
        let tets = tetrahedralize(&mesh);
        let (m, kept) = keep_if_ok(mesh, tets);
        mesh = m;
        if !kept {
            notes.push(
                "tetrahedralization skipped — it produced inverted cells on this geometry".into(),
            );
        }
    }

    // Stage 4 — Delaunay flip-improvement (improve-or-noop; always safe).
    if opts.delaunay {
        let flipped = flip_to_delaunay(&mesh, opts.max_flips);
        let (m, kept) = keep_if_ok(mesh, flipped);
        mesh = m;
        if !kept {
            notes.push(
                "Delaunay improvement skipped — no valid improving flips on this geometry".into(),
            );
        }
    }

    // Stage 5 — polyhedral dual. The new kinds (gh:#1240) first; on failure
    // they fall back to the per-corner median dual with a note.
    let mut dual_done = false;
    if opts.dual && opts.dual_kind != DualKind::MedianPerCorner {
        // Name the tet boundary first, so the dual sees the patch seams.
        let named = match regions {
            Some(r) => assign_patches_by_region(&mesh, points, tris, r).unwrap_or_else(|_| mesh.clone()),
            None => mesh.clone(),
        };
        let candidate = match opts.dual_kind {
            DualKind::MedianPerPoint => Ok(polyhedral_dual_per_point_with(&named, opts.dual_feature_angle_deg, opts.dual_feature_points)),
            _ => cell_centre(&named, opts.dual_feature_angle_deg, opts.dual_feature_points),
        };
        match candidate {
            Ok(d) if acceptable(&d) => {
                mesh = d;
                dual_done = true;
            }
            Ok(_) => notes.push(format!("{:?} dual skipped — it would invalidate the mesh; fell back to the per-corner median dual", opts.dual_kind)),
            Err(e) => notes.push(format!("{:?} dual skipped — {e}; fell back to the per-corner median dual", opts.dual_kind)),
        }
    }
    if opts.dual && !dual_done {
        let mut placed = false;
        if opts.dual_min_faces {
            let d = polyhedral_dual_min_faces(&mesh);
            if acceptable(&d) {
                mesh = d;
                placed = true;
            }
        }
        if !placed {
            let d = polyhedral_dual(&mesh);
            if acceptable(&d) {
                if opts.dual_min_faces {
                    notes.push(
                        "face-minimal dual skipped — fell back to the robust quad-fan dual".into(),
                    );
                }
                mesh = d;
                placed = true;
            }
        }
        if !placed {
            notes.push("polyhedral dual skipped — both dual variants would invalidate the mesh on this geometry".into());
        }
    }

    // Stage 6 — smart-Laplacian smoothing (never inverts; conserves volume).
    if opts.smooth_passes > 0 {
        let smoothed = laplacian_smooth(&mesh, opts.smooth_passes);
        let (m, kept) = keep_if_ok(mesh, smoothed);
        mesh = m;
        if !kept {
            notes.push("smoothing skipped — unexpectedly invalid (should not happen)".into());
        }
    }

    // Stage 6b — feature-edge capture (gh:#1202): cfMesh maps the boundary
    // onto the surface patches and their feature edges after the volume mesh
    // exists and before the layers (`tetMeshGenerator::generateMesh`:
    // `mapMeshToSurface`, `mapEdgesAndCorners`, then `generateBoudaryLayers`).
    if let (true, true, Some(regions)) = (opts.feature_edges && opts.dual, opts.untangle_surface, regions) {
        // gh:#1259: upstream's surface steps, nothing reverted.
        let (pre, r1) = crate::surface_optimizer::map_mesh_to_surface(&mesh, points, tris, regions);
        let given = crate::surface_mapper::boundary_face_regions(&pre, points, tris, regions);
        let (corrected, given) = if opts.correct_edges {
            let (c, g, rep) = crate::correct_edges::correct_edges_between_patches(&pre, &given, &regions.names);
            notes.push(format!(
                "correctEdgesBetweenPatches: {} concave faces, {} boundary faces split, {} faces fanned for connections, {} cells into pyramids",
                rep.n_concave_faces, rep.n_patch_corrected_faces, rep.n_connection_faces, rep.n_decomposed_cells
            ));
            (c, g)
        } else {
            (pre, given)
        };
        let mut mapped = crate::surface_mapper::map_vertices_onto_surface_patches_given(&corrected, points, tris, regions, &given);
        let r2 = crate::surface_optimizer::optimise_mesh_surface(&mut mapped, points, tris, regions, &given);
        notes.push(format!(
            "untangleSurface (mapMeshToSurface): inverted points {} -> {} in {} passes; optimizeSurface's untangleSurface: {} -> {}",
            r1.n_inverted_first, r1.n_inverted_last, r1.passes, r2.n_inverted_first, r2.n_inverted_last
        ));
        if opts.correct_edges && !crate::surface_mapper::cells_with_nonpositive_pyramids(&mapped).is_empty() {
            let locked = vec![false; mapped.point_count()];
            let r = crate::mesh_optimizer::untangle_mesh_fv(&mut mapped, &locked, 10, 50, 2, false);
            notes.push(format!("correctEdgesBetweenPatches: untangleMeshFV {} -> {} bad faces in {} passes", r.flagged_before, r.flagged_after, r.iterations));
        }
        let (m, kept) = keep_if_ok(mesh, mapped);
        mesh = m;
        if !kept {
            notes.push("feature-edge capture (untangleSurface) skipped — the mapped and smoothed boundary would invalidate the mesh".into());
        }
    } else if let (true, Some(regions)) = (opts.feature_edges && opts.dual, regions) {
        let (mapped, n_reverted) = if opts.correct_edges {
            // tetMeshGenerator::mapEdgesAndCorners: correctEdgesBetweenPatches,
            // then mapVerticesOntoSurfacePatches on the patches it kept.
            // Upstream first maps every boundary point onto the surface
            // (mapMeshToSurface), then extracts the patches.
            let (premapped, n_pre) = crate::surface_mapper::map_vertices_onto_surface_untangled(&mesh, points, tris, 10);
            let given = crate::surface_mapper::boundary_face_regions(&premapped, points, tris, regions);
            let (corrected, given, rep) = crate::correct_edges::correct_edges_between_patches(&premapped, &given, &regions.names);
            notes.push(format!(
                "correctEdgesBetweenPatches: {n_pre} points left off the surface by the pre-map, {} concave faces, {} boundary faces split, {} faces fanned for connections, {} cells into pyramids",
                rep.n_concave_faces, rep.n_patch_corrected_faces, rep.n_connection_faces, rep.n_decomposed_cells
            ));
            let (mut mapped, n_rev) = crate::surface_mapper::map_vertices_onto_surface_patches_untangled_given(&corrected, points, tris, regions, 10, &given);
            // A pyramid's apex (the vertex average of a non-convex dual cell)
            // can lie outside it. Upstream leaves that to the final
            // optimiser (tetMeshGenerator::optimiseFinalMesh,
            // meshOptimizer::untangleMeshFV with its defaults 10 / 50 / 2);
            // here it runs at once, because the layer stage needs a valid
            // mesh. Only interior points move (the apexes among them).
            if !crate::surface_mapper::cells_with_nonpositive_pyramids(&mapped).is_empty() {
                let locked = vec![false; mapped.point_count()];
                let r = crate::mesh_optimizer::untangle_mesh_fv(&mut mapped, &locked, 10, 50, 2, false);
                notes.push(format!("correctEdgesBetweenPatches: untangleMeshFV {} -> {} bad faces in {} passes", r.flagged_before, r.flagged_after, r.iterations));
            }
            (mapped, n_rev)
        } else {
            crate::surface_mapper::map_vertices_onto_surface_patches_untangled(&mesh, points, tris, regions, 10)
        };
        let (m, kept) = keep_if_ok(mesh, mapped);
        mesh = m;
        if !kept {
            notes.push("feature-edge capture skipped — mapping the boundary onto the patches would invalidate the mesh".into());
        } else if n_reverted > 0 {
            notes.push(format!(
                "feature-edge capture: {n_reverted} boundary points left unmapped where mapping tangled a cell{}",
                if opts.correct_edges { "" } else { " (cfMesh's correctEdgesBetweenPatches is off: TetDualOptions::correct_edges)" }
            ));
        }
    }

    // Stage 6c — cfMesh's optimiseFinalMesh on the bare mesh (gh:#1283),
    // unless the CfMeshOptimise layer stage will run it.
    let cfmesh_layers = opts.n_layers > 0 && opts.layer_strategy == LayerQualityStrategy::CfMeshOptimise;
    if let (true, false, Some(regions)) = (opts.optimise_final_mesh, cfmesh_layers, regions) {
        let mut optimised = mesh.clone();
        let n = crate::final_mesh::optimise_final_mesh(&mut optimised, points, tris, regions, &opts.cfmesh_layer_optimisation);
        notes.extend(n);
        let (m, kept) = keep_if_ok(mesh, optimised);
        mesh = m;
        if !kept {
            notes.push("optimiseFinalMesh skipped — it would invalidate the mesh".into());
        }
    }

    // Stage 7 — adaptive prism boundary layers on the wall patch.
    let mut layer_outcome: Option<LayerOutcome> = None;
    let selective = regions.filter(|_| !opts.layer_patches.is_empty());
    if let (true, Some(regions)) = (opts.n_layers > 0, selective) {
        // Patch-selective (gh:#1202): name the boundary first, then grow
        // layers on the chosen patches only (cfMesh `addLayerForPatch`).
        match assign_patches_by_region(&mesh, points, tris, regions) {
            Err(e) => notes.push(format!("boundary layers skipped — the boundary could not be named first: {e}")),
            Ok(named) => {
                let names: Vec<&str> = opts.layer_patches.iter().map(String::as_str).collect();
                let n_layer_faces: usize =
                    named.patches.iter().filter(|p| names.contains(&p.name.as_str())).map(|p| p.n_faces).sum();
                if n_layer_faces == 0 {
                    notes.push(format!("boundary layers skipped — no boundary face is on {:?}", opts.layer_patches));
                } else {
                    let (layered, outcome) = grow_layers(&named, &names, opts, Some((points, tris, regions)));
                    let before = mesh.cell_count();
                    let (m, kept) = keep_if_ok(mesh, layered);
                    mesh = m;
                    let strategy = outcome.strategy;
                    layer_outcome = Some(if kept { outcome } else { LayerOutcome::none(strategy, opts.n_layers) });
                    if !kept {
                        notes.push("boundary layers skipped — the wall is too tight for any valid layer".into());
                    } else if strategy != LayerQualityStrategy::BackOffOnly {
                        notes.push(layer_note(layer_outcome.as_ref().unwrap()));
                    } else if mesh.cell_count() != before + opts.n_layers * n_layer_faces {
                        notes.push(
                            "boundary layers added none — the layerer backed off to zero thickness, or a point \
                             touches three or more layer-free patches (refused, as upstream cfMesh does)"
                                .into(),
                        );
                    }
                }
            }
        }
    } else if opts.n_layers > 0 {
        let has_patch = mesh.patches.iter().any(|p| p.name == opts.wall_patch);
        if !has_patch {
            notes.push(format!(
                "boundary layers skipped — no patch named '{}'",
                opts.wall_patch
            ));
        } else {
            let (layered, outcome) = grow_layers(&mesh, &[opts.wall_patch.as_str()], opts, regions.map(|r| (points, tris, r)));
            // The adaptive layerer already backs off to a valid mesh, but it can
            // return the input rebuilt (no layers) when the wall is too tight;
            // either way `acceptable` keeps only a valid result.
            //
            // A rebuilt-with-no-layers result is *valid*, so `keep_if_ok` accepts
            // it and the stage would otherwise look successful while having done
            // nothing at all. Compare the cell count so that silent no-op is
            // reported instead: a prism stack always adds
            // `n_layers x wall_faces` cells, so an unchanged count means none grew.
            let before = mesh.cell_count();
            let (m, kept) = keep_if_ok(mesh, layered);
            mesh = m;
            let strategy = outcome.strategy;
            layer_outcome = Some(if kept { outcome } else { LayerOutcome::none(strategy, opts.n_layers) });
            if !kept {
                notes.push(
                    "boundary layers skipped — the wall is too tight for any valid layer".into(),
                );
            } else if strategy != LayerQualityStrategy::BackOffOnly {
                notes.push(layer_note(layer_outcome.as_ref().unwrap()));
            } else if mesh.cell_count() == before {
                notes.push(
                    "boundary layers added none — the adaptive layerer backed off to zero thickness \
                     (every candidate thickness produced an invalid mesh on this wall)"
                        .into(),
                );
            }
        }
    }

    // Stage 8 — named boundary patches from the input surface's regions.
    // Geometric, and deliberately last: it is the only point at which every
    // boundary face of the finished mesh exists (the layer stage creates some
    // that no input face corresponds to). See `crate::patches`.
    if let Some(regions) = regions {
        match assign_patches_by_region(&mesh, points, tris, regions) {
            Ok(named) => mesh = named,
            Err(e) => notes.push(format!("named boundary patches skipped — {e}")),
        }
    }

    // Final step (gh:#1285) — OpenFOAM's upper-triangular face order, as
    // upstream's renumberMesh leaves it, with the cell labels kept. One
    // step here rather than in each builder: every stage above rebuilds
    // its mesh from cell face lists, so an order set inside one is lost at
    // the next. Points and boundary faces are untouched.
    if upper_triangular {
        mesh = crate::renumber_mesh::upper_triangular(&mesh);
    }

    // Final gate — never hand back a mesh that is not closed.
    if let Err(e) = mesh.validate() {
        return Err(format!("internal error: final mesh is not closed: {e}"));
    }
    let q = check_quality(&mesh);
    let report = TetDualReport {
        stage_notes: notes,
        cell_count: mesh.cell_count(),
        total_volume: mesh.total_volume(),
        valid: true,
        max_non_orthogonality_deg: q.max_non_orthogonality_deg,
        max_skewness: q.max_skewness,
        n_negative_volume_cells: q.n_negative_volume_cells,
        layers: layer_outcome,
    };
    Ok((mesh, report))
}

/// Convenience wrapper: tet-dual mesh of an axis-aligned **box** `[min, max]`
/// (metres), using [`box_surface`]. See [`surface_to_tet_dual_mesh`].
pub fn box_tet_dual(
    min: Vec3,
    max: Vec3,
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    let (p, t) = box_surface(min, max);
    surface_to_tet_dual_mesh(&p, &t, opts)
}

/// Convenience wrapper: tet-dual mesh of a **sphere** of `radius` (metres) about
/// `centre`, triangulated with `n_lat` latitude bands × `n_lon` longitude
/// segments ([`sphere_surface`]). See [`surface_to_tet_dual_mesh`].
///
/// A sphere is a curved wall: expect the snap to body-fit the boundary and the
/// volume to sit within a few percent of the analytic `(4/3)πr³` at practical
/// resolutions (staircase/UV discretisation error), not exactly.
pub fn sphere_tet_dual(
    centre: Vec3,
    radius: f64,
    n_lat: usize,
    n_lon: usize,
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    let (p, t) = sphere_surface(centre, radius, n_lat, n_lon);
    surface_to_tet_dual_mesh(&p, &t, opts)
}

/// Convenience wrapper: tet-dual mesh of a Z-axis **cylinder** of `radius` and
/// `height` (metres) with base at `base`, `n_seg` circumferential segments
/// ([`cylinder_surface`]). See [`surface_to_tet_dual_mesh`].
pub fn cylinder_tet_dual(
    base: Vec3,
    radius: f64,
    height: f64,
    n_seg: usize,
    opts: &TetDualOptions,
) -> Result<(VolumeMesh, TetDualReport), String> {
    let (p, t) = cylinder_surface(base, radius, height, n_seg);
    surface_to_tet_dual_mesh(&p, &t, opts)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// gh:#1286, analytic: a closed 3×1×1 box with one more internal face
    /// between cells 0 and 1 whose three points are collinear (zero area:
    /// it adds nothing to the closure sums, so `validate()` and the cell
    /// volumes pass) is refused by `acceptable`, and `keep_if_ok` keeps the
    /// previous mesh. The same box without it is accepted.
    #[test]
    fn zero_area_face_is_not_acceptable() {
        let good = crate::cartesian::cartesian_box(Vec3::ZERO, Vec3::new(3.0, 1.0, 1.0), [3, 1, 1]);
        assert!(acceptable(&good));
        let on_edge: Vec<usize> = (0..good.point_count()).filter(|&i| good.points[i].y == 0.0 && good.points[i].z == 0.0).collect();
        assert!(on_edge.len() >= 3);
        let mut bad = good.clone();
        bad.faces.insert(0, vec![on_edge[0], on_edge[1], on_edge[2]]);
        bad.owner.insert(0, 0);
        bad.neighbour.insert(0, Some(1));
        for p in &mut bad.patches {
            p.start_face += 1;
        }
        assert!(bad.validate().is_ok());
        assert_eq!(check_quality(&bad).n_negative_volume_cells, 0);
        assert!(!acceptable(&bad));
        let (kept, ok) = keep_if_ok(good.clone(), bad);
        assert!(!ok);
        assert_eq!(kept.face_count(), good.face_count());
        let mut nan = good.clone();
        nan.points[0].x = f64::NAN;
        assert!(!faces_are_non_degenerate(&nan));
    }


    // ---- Verification (not validation) test suite --------------------------
    //
    // Measured 2026-08-03 on this code. Each test meshes a built-in primitive
    // through the full tet→dual→layers pipeline and asserts the *verification*
    // properties: a valid closed mesh, no inverted cells, and volume conserved
    // versus the analytic volume within a resolution-appropriate tolerance.
    // These are correctness checks on the mesher, NOT validation against a
    // CFD/TH solve. Untrusted AI-assisted draft pending human V&V.

    /// V&V — headline (box). Methodology: a unit box `[0,1]³` (analytic volume
    /// 1 m³) meshed at `cell_size = 0.5 m` (a grid-aligned carve, so the carve is
    /// exact), snap on, tetrahedralize, Delaunay-improve, face-minimal dual, then
    /// 3 prism layers (first `0.02 m`, expansion 1.3) on `walls`. Pass criteria:
    /// the mesh is valid and closed; 0 inverted cells; **volume conserved
    /// exactly** (every stage from carve to layers conserves volume on a flat-wall
    /// box, so the analytic 1 m³ is held to 1e-9); the dual made it genuinely
    /// polyhedral; prism cells were added. Measured 2026-08-03: valid; 0 negative;
    /// volume = 1.00000 m³ (|Δ| < 1e-9); 935 cells; no stages skipped; max
    /// non-orthogonality ≈ 79.2°, max skewness ≈ 0.85 (the near-wall prism layers
    /// dominate both, even on a flat wall — see the sphere test's note on why BL
    /// meshes exceed checkMesh's 70° threshold).
    #[test]
    fn box_tet_dual_is_valid_and_conserves_volume_exactly() {
        let opts = TetDualOptions {
            cell_size: 0.5,
            first_layer_thickness: 0.02,
            ..Default::default()
        };
        let (mesh, rep) =
            box_tet_dual(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), &opts).expect("box meshes");

        mesh.validate().expect("box tet-dual mesh is closed");
        assert!(rep.valid);
        assert_eq!(rep.n_negative_volume_cells, 0, "no inverted cells");
        assert!(
            (rep.total_volume - 1.0).abs() < 1e-9,
            "box volume conserved exactly: {}",
            rep.total_volume
        );
        assert!(mesh.cell_count() > 0);
        // Genuinely polyhedral: at least one dual cell has more than 6 faces.
        use crate::volume_mesh::cells_faces;
        let max_faces = cells_faces(&mesh).iter().map(|c| c.len()).max().unwrap();
        assert!(
            max_faces > 6,
            "polyhedral cells present (max faces/cell = {max_faces})"
        );
        // A flat box needs no back-off, so nothing is skipped.
        assert!(
            rep.stage_notes.is_empty(),
            "no stages skipped on a box: {:?}",
            rep.stage_notes
        );
    }

    /// V&V — the same box without layers is an even tighter volume check and
    /// confirms the tet→dual core alone conserves volume exactly. Methodology:
    /// unit box, `cell_size = 0.5 m`, `n_layers = 0`. Measured 2026-08-03: valid;
    /// 0 negative; volume = 1.0 m³ (|Δ| < 1e-9).
    #[test]
    fn box_tet_dual_no_layers_conserves_volume() {
        let opts = TetDualOptions {
            cell_size: 0.5,
            n_layers: 0,
            ..Default::default()
        };
        let (mesh, rep) =
            box_tet_dual(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), &opts).expect("box meshes");
        mesh.validate().expect("closed");
        assert_eq!(rep.n_negative_volume_cells, 0);
        assert!(
            (rep.total_volume - 1.0).abs() < 1e-9,
            "volume {}",
            rep.total_volume
        );
    }

    /// V&V — headline (sphere, curved wall). Methodology: a radius-2 m sphere
    /// (analytic volume `(4/3)π·2³ ≈ 33.510 m³`) triangulated 16 lat × 32 lon,
    /// meshed at `cell_size = 0.8 m`, snap on, tet→face-minimal dual→3 adaptive
    /// prism layers (first `0.04 m`, expansion 1.3). Pass criteria: valid closed
    /// mesh; 0 inverted cells; volume within 8 % of analytic (staircase + UV
    /// discretisation at this coarse resolution — snap conforms the boundary but
    /// coarse cells under-fill the sphere); non-orthogonality strictly below the
    /// degenerate 90° (no folded faces) and reported for interpretation.
    ///
    /// Note on the angle: near-wall prism layers on a *curved* wall are
    /// intrinsically non-orthogonal and high-aspect-ratio, so a BL mesh
    /// legitimately exceeds checkMesh's conservative 70° warning — such meshes are
    /// solved with non-orthogonal correctors. The meaningful hard bound is 90°
    /// (a face at/beyond 90° would be folded/degenerate); this mesh stays clear of
    /// it. Skewness is likewise elevated near the wall (checkMesh flags > 4); it is
    /// recorded, not gated, for the same reason.
    ///
    /// Measured 2026-08-03: valid; 0 negative; 5083 cells; volume ≈ 31.35 m³
    /// (≈ 6.5 % low); max non-orthogonality ≈ 86.7°; max skewness ≈ 14.0. Stage
    /// skipped: the Delaunay flip stage (on the sliver-rich snapped tet mesh the
    /// re-Delaunay produced a marginally-negative sliver on reassembly and was
    /// gated out — the pipeline keeps the valid direct tetrahedralization; the dual
    /// and layers then ran normally).
    #[test]
    fn sphere_tet_dual_is_valid_and_near_analytic_volume() {
        let opts = TetDualOptions {
            cell_size: 0.8,
            first_layer_thickness: 0.04,
            ..Default::default()
        };
        let (mesh, rep) = sphere_tet_dual(Vec3::ZERO, 2.0, 16, 32, &opts).expect("sphere meshes");

        mesh.validate().expect("sphere tet-dual mesh is closed");
        assert_eq!(
            rep.n_negative_volume_cells, 0,
            "no inverted cells (notes: {:?})",
            rep.stage_notes
        );
        let analytic = 4.0 / 3.0 * PI * 8.0;
        let rel = (rep.total_volume - analytic).abs() / analytic;
        assert!(
            rel < 0.08,
            "sphere volume {} within 8% of {analytic} (rel {rel}); notes {:?}",
            rep.total_volume,
            rep.stage_notes
        );
        // Near-wall prism layers legitimately exceed checkMesh's 70°; the hard
        // bound is the degenerate 90° (no folded faces). Measured ≈ 86.7°.
        assert!(
            rep.max_non_orthogonality_deg < 90.0,
            "no folded faces: {}",
            rep.max_non_orthogonality_deg
        );
        assert!(
            rep.max_non_orthogonality_deg > 70.0,
            "curved BL wall is non-orthogonal as expected: {}",
            rep.max_non_orthogonality_deg
        );
    }

    /// V&V — headline (cylinder, mixed flat caps + curved side). Methodology: a
    /// radius-1.5 m, height-4 m Z-cylinder (analytic volume `π·1.5²·4 ≈
    /// 28.274 m³`), 32 segments, `cell_size = 0.7 m`, snap on, tet→face-minimal
    /// dual→3 adaptive prism layers (first `0.04 m`, expansion 1.3). Pass
    /// criteria: valid closed mesh; 0 inverted cells; volume within 8 % of
    /// analytic; non-orthogonality below the degenerate 90°; prism layers added
    /// (cell count exceeds the no-layer mesh). Measured 2026-08-03: valid; 0
    /// negative; 5127 cells; volume ≈ 26.99 m³ (≈ 4.6 % low); max
    /// non-orthogonality ≈ 84.7°; max skewness ≈ 11.4; layers grew (vs the
    /// no-layer mesh). Stage skipped: the Delaunay flip (same reason as the sphere
    /// test).
    #[test]
    fn cylinder_tet_dual_is_valid_and_near_analytic_volume() {
        let opts = TetDualOptions {
            cell_size: 0.7,
            first_layer_thickness: 0.04,
            ..Default::default()
        };
        let (mesh, rep) =
            cylinder_tet_dual(Vec3::ZERO, 1.5, 4.0, 32, &opts).expect("cylinder meshes");

        mesh.validate().expect("cylinder tet-dual mesh is closed");
        assert_eq!(
            rep.n_negative_volume_cells, 0,
            "no inverted cells (notes: {:?})",
            rep.stage_notes
        );
        let analytic = PI * 1.5 * 1.5 * 4.0;
        let rel = (rep.total_volume - analytic).abs() / analytic;
        assert!(
            rel < 0.08,
            "cylinder volume {} within 8% of {analytic} (rel {rel}); notes {:?}",
            rep.total_volume,
            rep.stage_notes
        );
        assert!(
            rep.max_non_orthogonality_deg < 90.0,
            "no folded faces: {}",
            rep.max_non_orthogonality_deg
        );

        // Prism layers were actually added: compare to the same mesh with none.
        let no_layers = TetDualOptions {
            n_layers: 0,
            ..opts.clone()
        };
        let (bare, _) = cylinder_tet_dual(Vec3::ZERO, 1.5, 4.0, 32, &no_layers)
            .expect("cylinder no-layer meshes");
        assert!(
            mesh.cell_count() > bare.cell_count(),
            "prism layers added: {} > {}",
            mesh.cell_count(),
            bare.cell_count()
        );
    }

    /// V&V — error paths and stage toggles. A non-positive `cell_size` is a hard
    /// error; a `cell_size` far larger than the geometry carves zero cells and is
    /// a hard error; turning the dual off returns the (valid) tetrahedralization.
    /// Measured 2026-08-03: both errors returned; dual-off mesh valid with a
    /// non-empty note about the dual being disabled is NOT emitted (disabling is
    /// intentional, not a degradation) — the mesh is simply the tet mesh.
    #[test]
    fn error_paths_and_dual_toggle() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));

        let bad = TetDualOptions {
            cell_size: -1.0,
            ..Default::default()
        };
        assert!(
            surface_to_tet_dual_mesh(&p, &t, &bad).is_err(),
            "negative cell_size errors"
        );

        let huge = TetDualOptions {
            cell_size: 100.0,
            n_layers: 0,
            ..Default::default()
        };
        assert!(
            surface_to_tet_dual_mesh(&p, &t, &huge).is_err(),
            "cell_size that carves nothing errors"
        );

        // Dual off: the result is the tetrahedralization (all-tet), still valid.
        let no_dual = TetDualOptions {
            cell_size: 0.5,
            dual: false,
            n_layers: 0,
            ..Default::default()
        };
        let (mesh, rep) = surface_to_tet_dual_mesh(&p, &t, &no_dual).expect("tet-only meshes");
        mesh.validate().expect("tet-only mesh valid");
        assert!(
            (rep.total_volume - 1.0).abs() < 1e-9,
            "tet-only volume {}",
            rep.total_volume
        );
        use crate::volume_mesh::cells_faces;
        for cell in cells_faces(&mesh) {
            assert_eq!(cell.len(), 4, "dual off -> every cell is a tetrahedron");
        }
    }

    /// V&V — **headline for octree grading** (`refinement_levels`): the graded
    /// background mesh reaches *better* volume accuracy than the uniform carve
    /// at a *fraction* of the cell count.
    ///
    /// # Methodology
    ///
    /// Geometry: the same radius-3 m sphere the `defaults_mesh_the_studio_sphere`
    /// test in this module uses, triangulated 24 lat x 48 lon;
    /// analytic volume `(4/3)*pi*3^3 = 113.0973 m^3`. Both meshes run the
    /// identical pipeline configuration — snap on, Delaunay improvement **off**
    /// (it is a no-op on this geometry and only adds runtime), face-minimal dual
    /// on, **no boundary layers** (so the comparison measures the background
    /// mesher, not the layerer) — and differ only in stage 1:
    ///
    /// - **uniform baseline**: `cell_size = 0.50 m`, `refinement_levels = 0` —
    ///   [`carve_box`] at one size everywhere, the crate's original behaviour.
    /// - **graded**: `cell_size = 1.00 m`, `refinement_levels = 1`,
    ///   `refinement_band = 1.0` — [`refine_near_boundary_banded`], so the
    ///   interior stays at 1.00 m and the wall band is refined to 0.50 m. The two
    ///   therefore resolve the **wall at the same 0.50 m**, which is the
    ///   like-for-like comparison.
    ///
    /// Pass criteria: the graded mesh must use **materially fewer cells**
    /// (asserted: at least 2x fewer) at **no worse volume error**, both meshes
    /// valid, closed and free of inverted cells.
    ///
    /// # Results (measured 2026-08-07 on this code, release build)
    ///
    /// **Read the last column before citing any row.** Only two of the eight
    /// rows are run by this test; the other six were measured by hand on the
    /// date above and are recorded here as evidence, not as a gate. A
    /// doc-recorded row will not fail CI if the code regresses.
    ///
    /// | stage-1 mesher | cells | volume (m^3) | volume error | max non-orth (deg) | max skewness | run by this test? |
    /// |---|---|---|---|---|---|---|
    /// | uniform 0.60 m | 3271 | 110.6614 | 2.154 % | 71.71 | 0.267 | no — doc-recorded only |
    /// | **uniform 0.50 m** | **5269** | **111.1537** | **1.719 %** | **72.46** | **0.248** | **yes — the baseline arm** |
    /// | uniform 0.40 m | 42216 | 111.3923 | 1.508 % | 85.14 | 0.924 | no — doc-recorded only |
    /// | uniform 0.30 m | 101376 | 111.8890 | 1.068 % | 85.38 | 2.435 | no — doc-recorded only |
    /// | graded 1.20 m L1 | 3263 | 110.8886 | 1.953 % | 86.78 | 0.507 | no — doc-recorded only |
    /// | **graded 1.00 m L1** | **1381** | **111.2254** | **1.655 %** | **83.80** | **0.227** | **yes — the graded arm** |
    /// | graded 0.80 m L1 | 1890 | 111.3568 | 1.539 % | 84.22 | 0.260 | no — doc-recorded only |
    /// | graded 0.60 m L1 | 3921 | 111.8931 | 1.065 % | 85.58 | 0.300 | no — doc-recorded only |
    ///
    /// ## Exactly what is asserted, even for the two gated rows
    ///
    /// The two `yes` rows are *run*, but their tabulated numbers are not
    /// checked value-for-value. The assertions below are only:
    ///
    /// - both meshes `validate()` (closed) and report `n_negative_volume_cells
    ///   == 0`;
    /// - `graded volume error <= uniform volume error` (a *relative* compare —
    ///   neither 1.655 % nor 1.719 % is asserted as such);
    /// - `graded cells * 2 < uniform cells` (again relative — neither 1381 nor
    ///   5269 is asserted);
    /// - `max_non_orthogonality_deg < 90.0` for both — a **degeneracy floor**,
    ///   not a quality gate. It would pass at 89.9 deg, so it does *not* gate
    ///   the tabulated 72.46 / 83.80.
    ///
    /// **Skewness is not asserted anywhere in this test** — the whole max-
    /// skewness column, including the 2.435 outlier at uniform 0.30 m, is
    /// doc-recorded only. Likewise no cell count, volume, or non-orthogonality
    /// figure in the table is pinned by an assertion; re-measure before quoting
    /// any of them as current.
    ///
    /// The two bold rows are the pair this test asserts: **1381 cells at 1.655 %
    /// error, versus 5269 cells at 1.719 % — 3.8x fewer cells and slightly
    /// better accuracy**, with max skewness also better (0.227 vs 0.248) and max
    /// non-orthogonality higher but still clear of the degenerate 90 deg (83.80
    /// vs 72.46; the graded mesh's transition cells are legitimately more
    /// oblique). Reading down the table, the whole graded family dominates the
    /// uniform family on the cells-versus-accuracy curve: the largest measured
    /// gap is **graded 0.60 m L1 (3921 cells, 1.065 %) against uniform 0.30 m
    /// (101376 cells, 1.068 %) — the same accuracy for 25.9x fewer cells, and a
    /// far better max skewness (0.300 vs 2.435)**. That pair is *not* asserted
    /// here only because the uniform 0.30 m run costs ~13 s — so the headline
    /// "25.9x" figure rests on two doc-recorded rows, and is the weaker of the
    /// two claims on this page precisely because nothing re-measures it.
    ///
    /// # Interpretation and caveats (do not over-read this)
    ///
    /// - This is **verification** (volume conservation against the analytic
    ///   sphere) and **not validation** — no CFD/TH solve was run on either mesh,
    ///   so "better mesh" here means "closer volume at fewer cells with no
    ///   inverted cells", nothing more.
    /// - The two families **degrade through different stages**, which is a real
    ///   confounder to state rather than hide. On this geometry the graded runs
    ///   report `"tetrahedralization skipped"` (snapping a coarse hanging-node
    ///   cell can make it non-star-shaped, so centroid subdivision inverts it) —
    ///   their output is the polyhedral dual taken directly of the graded hex
    ///   mesh. The fine uniform runs (0.40 m, 0.30 m) instead report
    ///   `"polyhedral dual skipped"` — their output is the tetrahedralization.
    ///   Both outputs are valid, closed, inverted-cell-free volume meshes, and
    ///   both are compared on the same footing (cells needed for a given volume
    ///   accuracy), but they are **not the same cell type**. Note that the two
    ///   `"polyhedral dual skipped"` rows (uniform 0.40 m and 0.30 m) are both
    ///   **doc-recorded only** — no test runs them, so that stage-skip
    ///   observation is itself unverified by CI and could silently change.
    /// - Because the two skipped-stage families produce different cell types,
    ///   the max non-orthogonality / max skewness columns are **not comparing
    ///   like with like** down the table: the fine uniform rows describe a
    ///   tet mesh, the graded rows a polyhedral dual. Do not read the column as
    ///   a single trend.
    /// - Volume error is not monotone in cell size for the uniform family
    ///   either (staircase + UV discretisation + which stages survive), so a
    ///   single pair proves less than the trend across the table.
    #[test]
    fn octree_grading_beats_the_uniform_carve_on_cells_per_accuracy() {
        let analytic = 4.0 / 3.0 * PI * 27.0;
        let common = TetDualOptions {
            delaunay: false,
            n_layers: 0,
            ..Default::default()
        };

        let uniform = TetDualOptions {
            cell_size: 0.50,
            refinement_levels: 0,
            ..common.clone()
        };
        let (um, ur) =
            sphere_tet_dual(Vec3::ZERO, 3.0, 24, 48, &uniform).expect("uniform sphere meshes");

        let graded = TetDualOptions {
            cell_size: 1.00,
            refinement_levels: 1,
            refinement_band: 1.0,
            ..common
        };
        let (gm, gr) =
            sphere_tet_dual(Vec3::ZERO, 3.0, 24, 48, &graded).expect("graded sphere meshes");

        um.validate().expect("uniform mesh is closed");
        gm.validate().expect("graded mesh is closed");
        assert_eq!(ur.n_negative_volume_cells, 0, "uniform: no inverted cells");
        assert_eq!(
            gr.n_negative_volume_cells, 0,
            "graded: no inverted cells (notes {:?})",
            gr.stage_notes
        );

        let u_err = (ur.total_volume - analytic).abs() / analytic;
        let g_err = (gr.total_volume - analytic).abs() / analytic;

        assert!(
            g_err <= u_err,
            "graded is no less accurate: graded {:.4} % ({} cells) vs uniform {:.4} % ({} cells)",
            g_err * 100.0,
            gr.cell_count,
            u_err * 100.0,
            ur.cell_count
        );
        assert!(
            gr.cell_count * 2 < ur.cell_count,
            "graded uses materially fewer cells: {} vs {}",
            gr.cell_count,
            ur.cell_count
        );
        // Neither mesh may contain a folded/degenerate face.
        assert!(ur.max_non_orthogonality_deg < 90.0 && gr.max_non_orthogonality_deg < 90.0);
    }

    /// V&V — `refinement_levels = 0` is **exactly** the historical uniform path.
    /// Methodology: mesh the unit box through the full default pipeline twice,
    /// once with the field left at its default `0` and once by calling
    /// [`carve_box`] + the same stages, and compare the reported cell count and
    /// volume. Pass criterion: identical. Measured 2026-08-07: both 935 cells,
    /// volume 1.0 m^3 — i.e. adding the octree option regressed nothing.
    #[test]
    fn refinement_level_zero_is_the_uniform_path() {
        let uniform = TetDualOptions {
            cell_size: 0.5,
            first_layer_thickness: 0.02,
            ..Default::default()
        };
        assert_eq!(uniform.refinement_levels, 0, "grading is off by default");
        let (_, a) = box_tet_dual(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), &uniform).expect("meshes");
        let explicit = TetDualOptions {
            refinement_levels: 0,
            ..uniform
        };
        let (_, b) = box_tet_dual(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), &explicit).expect("meshes");
        assert_eq!(a.cell_count, b.cell_count);
        assert!((a.total_volume - b.total_volume).abs() < 1e-12);
        assert!(
            (a.total_volume - 1.0).abs() < 1e-9,
            "volume {}",
            a.total_volume
        );
    }

    /// V&V — **headline for named boundary patches**
    /// ([`surface_to_tet_dual_mesh_multipatch`]): a surface authored with three
    /// named regions comes out of the *full* pipeline as three real patches, so a
    /// solver `0/` boundary-condition directory can be written against it.
    ///
    /// # Methodology
    ///
    /// Geometry: the unit box `[0,1]^3` from [`box_surface`], whose 12 triangles
    /// are emitted two per side in the order `-Z, +Z, -Y, +Y, +X, -X`. They are
    /// labelled `walls` (the four sides), `outlet` (`+X`) and `inlet` (`-X`) —
    /// the canonical duct set-up. The mesh runs the complete
    /// `carve -> snap -> tetrahedralize -> Delaunay -> face-minimal dual ->
    /// prism layers` pipeline at `cell_size = 0.5 m`, `first_layer_thickness =
    /// 0.02 m`, 3 layers — i.e. every stage that rebuilds the mesh through
    /// [`crate::volume_mesh::from_cell_faces`] and would otherwise destroy patch
    /// information, plus the layer stage, which *creates* boundary faces.
    ///
    /// Pass criteria: exactly the three named patches exist; their face counts
    /// are non-zero and sum to every boundary face; they are **contiguous** runs
    /// starting immediately after the internal faces (the OpenFOAM ordering
    /// rule); `inlet` faces all lie on the `x = 0` plane and `outlet` faces on
    /// `x = 1` (a geometric check, not just a count); and the mesh is still
    /// valid, closed, inverted-cell-free and volume-exact at 1 m^3.
    ///
    /// # Results (measured 2026-08-07 on this code, release build)
    ///
    /// ```text
    /// multipatch box: cells=935 volume=1.000000000 internal_faces=2998
    ///   boundary_faces=288
    ///   patches=[("walls", 2998, 192), ("outlet", 3190, 48), ("inlet", 3238, 48)]
    ///   notes=[]
    /// ```
    ///
    /// 935 cells; volume 1.000000000 m^3 (`|dV| < 1e-9` against the analytic
    /// 1 m^3); `validate()` Ok; 0 inverted cells; **no stage skipped**. The 288
    /// boundary faces split into three contiguous patches — `walls` 192 faces
    /// starting at face 2998 (immediately after the 2998 internal faces),
    /// `outlet` 48 starting at 3190, `inlet` 48 starting at 3238 — which sums to
    /// 3286 = the total face count. The 4:1:1 face split matches the geometry
    /// (four `walls` sides against one `inlet` and one `outlet` cap). Every
    /// `inlet` face centroid lies within 1e-9 m of `x = 0` and every `outlet`
    /// face centroid within 1e-9 m of `x = 1`.
    ///
    /// The pre-existing single-patch entry point [`surface_to_tet_dual_mesh`] on
    /// the same geometry gives the same 935 cells with one `walls` patch of 288
    /// faces, so nothing regressed — the two differ only in how the boundary is
    /// labelled.
    ///
    /// # Interpretation and caveats
    ///
    /// This verifies that region names **survive the pipeline** and that the
    /// resulting patch list is well-formed — it is not a claim that the patch
    /// *boundaries* are feature-accurate. On a curved or feature-rich surface the
    /// patch seam follows mesh face edges, not the surface's feature edge, and
    /// two regions closer together than a cell can be mixed on the straddling
    /// faces. Note also that layers are grown over the **whole** boundary
    /// (including `inlet`/`outlet`), because classification runs last — see
    /// [`surface_to_tet_dual_mesh_multipatch`].
    #[test]
    fn named_regions_survive_the_full_pipeline_as_patches() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let regions = SurfaceRegions::from_labels(&[
            "walls", "walls", // -Z
            "walls", "walls", // +Z
            "walls", "walls", // -Y
            "walls", "walls", // +Y
            "outlet", "outlet", // +X
            "inlet", "inlet", // -X
        ]);
        let opts = TetDualOptions {
            cell_size: 0.5,
            first_layer_thickness: 0.02,
            ..Default::default()
        };
        let (mesh, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &opts)
            .expect("multipatch box meshes");

        // Printed so the numbers recorded in this test's doc comment can be
        // re-derived by anyone with `--nocapture`, per the workspace V&V rule.
        println!(
            "multipatch box: cells={} volume={:.9} internal_faces={} boundary_faces={} patches={:?} notes={:?}",
            rep.cell_count,
            rep.total_volume,
            mesh.n_internal_faces(),
            mesh.n_boundary_faces(),
            mesh.patches.iter().map(|q| (q.name.clone(), q.start_face, q.n_faces)).collect::<Vec<_>>(),
            rep.stage_notes,
        );

        mesh.validate().expect("multipatch mesh is closed");
        assert_eq!(
            rep.n_negative_volume_cells, 0,
            "no inverted cells (notes {:?})",
            rep.stage_notes
        );
        assert!(
            (rep.total_volume - 1.0).abs() < 1e-9,
            "volume {}",
            rep.total_volume
        );

        // All three patches present, each non-empty.
        let names: Vec<&str> = mesh.patches.iter().map(|q| q.name.as_str()).collect();
        assert_eq!(mesh.patches.len(), 3, "three patches, got {names:?}");
        for n in ["walls", "outlet", "inlet"] {
            assert!(names.contains(&n), "patch '{n}' present in {names:?}");
        }
        let get = |n: &str| mesh.patches.iter().find(|q| q.name == n).unwrap();
        for n in ["walls", "outlet", "inlet"] {
            assert!(get(n).n_faces > 0, "patch '{n}' has faces");
        }

        // Contiguous runs, internal faces first, covering the boundary exactly.
        let mut sorted = mesh.patches.clone();
        sorted.sort_by_key(|q| q.start_face);
        let mut expect = mesh.n_internal_faces();
        for q in &sorted {
            assert_eq!(
                q.start_face, expect,
                "patch '{}' starts contiguously",
                q.name
            );
            expect += q.n_faces;
        }
        assert_eq!(
            expect,
            mesh.face_count(),
            "patches cover every boundary face"
        );
        let total: usize = mesh.patches.iter().map(|q| q.n_faces).sum();
        assert_eq!(
            total,
            mesh.n_boundary_faces(),
            "patch faces == boundary faces"
        );

        // Geometrically correct, not merely well-counted.
        for (name, x) in [("inlet", 0.0), ("outlet", 1.0)] {
            let q = get(name);
            for f in q.start_face..q.start_face + q.n_faces {
                assert!(
                    mesh.neighbour[f].is_none(),
                    "'{name}' face {f} is a boundary face"
                );
                let c = mesh.face_centroid(f);
                assert!(
                    (c.x - x).abs() < 1e-9,
                    "'{name}' face at x={x}, got {}",
                    c.x
                );
            }
        }

        // The single-patch entry point is unchanged by all this.
        let (plain, prep) =
            surface_to_tet_dual_mesh(&p, &t, &opts).expect("single-patch box meshes");
        assert_eq!(
            prep.cell_count, rep.cell_count,
            "same mesh, different patch labelling"
        );
        assert_eq!(plain.patches.len(), 1);
        assert_eq!(plain.patches[0].name, "walls");
        assert_eq!(plain.patches[0].n_faces, mesh.n_boundary_faces());
    }

    /// V&V — a mislabelled surface is a clear error, not a silently wrong mesh:
    /// [`surface_to_tet_dual_mesh_multipatch`] validates the region labelling
    /// against the triangle count *before* meshing. Measured 2026-08-07: a
    /// `SurfaceRegions` describing the wrong number of triangles returns `Err`.
    #[test]
    fn multipatch_rejects_a_mislabelled_surface() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let wrong = SurfaceRegions::single("walls", t.len() + 1);
        let opts = TetDualOptions {
            cell_size: 0.5,
            n_layers: 0,
            ..Default::default()
        };
        assert!(surface_to_tet_dual_mesh_multipatch(&p, &t, &wrong, &opts).is_err());
    }

    /// V&V — the default options produce a valid mesh on the default-scale sphere
    /// (radius 3 m), the mesh-studio default geometry. Confirms
    /// `TetDualOptions::default()` is self-consistent end-to-end. Measured
    /// 2026-08-03: valid; 0 negative; 20551 cells; volume ≈ 110.66 m³ (≈ 2.2 % low
    /// vs `(4/3)π·27 ≈ 113.10 m³`); max non-orthogonality ≈ 88.0°; max skewness
    /// ≈ 30.8 (finer curved BL layers raise both further than the coarse sphere —
    /// still no folded/degenerate faces).
    #[test]
    fn defaults_mesh_the_studio_sphere() {
        let opts = TetDualOptions::default();
        let (mesh, rep) =
            sphere_tet_dual(Vec3::ZERO, 3.0, 24, 48, &opts).expect("default sphere meshes");
        mesh.validate().expect("closed");
        assert_eq!(
            rep.n_negative_volume_cells, 0,
            "notes: {:?}",
            rep.stage_notes
        );
        let analytic = 4.0 / 3.0 * PI * 27.0;
        let rel = (rep.total_volume - analytic).abs() / analytic;
        assert!(
            rel < 0.08,
            "default sphere volume {} within 8% of {analytic} (rel {rel})",
            rep.total_volume
        );
    }

    /// Every stage of the tet-dual pipeline gives the same mesh, numbering
    /// included, when run twice in one process (each run's hash maps are
    /// seeded differently). On 2026-11-04 the face-minimal dual failed this
    /// (its edge map was iterated); the two duals now iterate a BTreeMap
    /// (gh:#1202).
    #[test]
    fn tet_dual_stages_are_deterministic() {
        let (p, t, regions) = pipe_surface(0.01, 0.04, 24);
        let same = |a: &VolumeMesh, b: &VolumeMesh| {
            a.faces == b.faces && a.owner == b.owner && a.points.iter().zip(&b.points).all(|(x, y)| x.x == y.x && x.y == y.y && x.z == y.z)
        };
        let c = carve_box(&p, &t, 0.004);
        assert!(same(&c, &carve_box(&p, &t, 0.004)), "carve");
        let s = snap_to_surface(&c, &p, &t);
        assert!(same(&s, &snap_to_surface(&c, &p, &t)), "snap");
        let te = tetrahedralize(&s);
        assert!(same(&te, &tetrahedralize(&s)), "tet");
        let f = flip_to_delaunay(&te, 20_000);
        assert!(same(&f, &flip_to_delaunay(&te, 20_000)), "flip");
        let d = polyhedral_dual_min_faces(&f);
        assert!(same(&d, &polyhedral_dual_min_faces(&f)), "min-face dual");
        assert!(same(&polyhedral_dual(&f), &polyhedral_dual(&f)), "quad-fan dual");
        let n = assign_patches_by_region(&d, &p, &t, &regions).unwrap();
        assert!(same(&n, &assign_patches_by_region(&d, &p, &t, &regions).unwrap()), "named");
        let l = crate::layers::add_boundary_layers_adaptive_on_patches(&n, &["walls"], 3, 0.0003, 1.2);
        assert!(same(&l, &crate::layers::add_boundary_layers_adaptive_on_patches(&n, &["walls"], 3, 0.0003, 1.2)), "layers");
    }

    /// FNV-1a over a mesh's point bits, faces, owners and neighbours: a
    /// bit-identity fingerprint for pinning a mesher's output.
    pub(crate) fn mesh_fingerprint(m: &VolumeMesh) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |x: u64| {
            for b in x.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for p in &m.points {
            eat(p.x.to_bits());
            eat(p.y.to_bits());
            eat(p.z.to_bits());
        }
        for (f, ring) in m.faces.iter().enumerate() {
            eat(ring.len() as u64);
            for &v in ring {
                eat(v as u64);
            }
            eat(m.owner[f] as u64);
            eat(m.neighbour[f].map_or(u64::MAX, |n| n as u64));
        }
        for pa in &m.patches {
            eat(pa.start_face as u64);
            eat(pa.n_faces as u64);
        }
        h
    }

    /// V&V (gh:#1285) — **the upper-triangular face order moves no
    /// geometry.** Methodology: the pipeline with and without its final
    /// [`crate::renumber_mesh::upper_triangular`] step (the private
    /// `run_pipeline` flag), on the demo pipe (R 10 mm, L 200 mm, 48 facets)
    /// at h 4 / 5 / 6 mm with no layer (Foam Studio's mesh before gh:#1298),
    /// at h 5 mm with 3 back-off layers, and on the `CellCentre` + `Corners`
    /// dual (feature `cell-centre-dual`). Compared: total volume (bits),
    /// each cell's volume (labels are kept, so cell by cell), the face-area
    /// multiset, the patch face sets, and `checkUpperTriangular` before and
    /// after. Predictions and results:
    /// `verification_and_validation/upper_triangular_gh1285/README.md`.
    #[test]
    fn upper_triangular_order_moves_no_geometry() {
        let (p, t, regions) = pipe_surface(0.01, 0.2, 48);
        let mut cases = vec![
            ("h4 bare", TetDualOptions { cell_size: 0.004, n_layers: 0, layer_patches: vec!["walls".into()], ..Default::default() }),
            ("h5 bare", TetDualOptions { cell_size: 0.005, n_layers: 0, layer_patches: vec!["walls".into()], ..Default::default() }),
            ("h6 bare", TetDualOptions { cell_size: 0.006, n_layers: 0, layer_patches: vec!["walls".into()], ..Default::default() }),
            ("h5 n3", TetDualOptions { cell_size: 0.005, n_layers: 3, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], ..Default::default() }),
        ];
        if cfg!(feature = "cell-centre-dual") {
            cases.push((
                "h5 CellCentre+Corners",
                TetDualOptions {
                    cell_size: 0.005,
                    n_layers: 0,
                    layer_patches: vec!["walls".into()],
                    dual_kind: DualKind::CellCentre,
                    dual_feature_points: FeaturePointRule::Corners,
                    ..Default::default()
                },
            ));
        }
        for (name, o) in cases {
            let (a, _) = run_pipeline(&p, &t, &o, Some(&regions), false).unwrap();
            let (b, rep) = run_pipeline(&p, &t, &o, Some(&regions), true).unwrap();
            let before = crate::poly_mesh_gen_checks::check_upper_triangular(&a).0;
            let after = crate::poly_mesh_gen_checks::check_upper_triangular(&b).0;
            assert!(!after, "{name}: not upper-triangular after the step");
            assert_eq!(a.points, b.points, "{name}: points");
            assert_eq!((a.n_cells, a.face_count(), a.n_internal_faces()), (b.n_cells, b.face_count(), b.n_internal_faces()));
            assert_eq!(a.total_volume().to_bits(), b.total_volume().to_bits(), "{name}: total volume bit for bit");
            assert_eq!(rep.total_volume.to_bits(), a.total_volume().to_bits());
            let (va, _) = crate::checks::cell_geometry(&a);
            let (vb, _) = crate::checks::cell_geometry(&b);
            let (mut same_v, mut worst_v) = (0usize, 0.0f64);
            for (x, y) in va.iter().zip(&vb) {
                same_v += (x.to_bits() == y.to_bits()) as usize;
                worst_v = worst_v.max((x - y).abs() / x.abs());
            }
            let areas = |m: &VolumeMesh| {
                let mut v: Vec<f64> = (0..m.face_count()).map(|f| m.face_area_vector(f).length()).collect();
                v.sort_by(|x, y| x.partial_cmp(y).unwrap());
                v
            };
            let (aa, ab) = (areas(&a), areas(&b));
            let (mut same_a, mut worst_a) = (0usize, 0.0f64);
            for (x, y) in aa.iter().zip(&ab) {
                same_a += (x.to_bits() == y.to_bits()) as usize;
                worst_a = worst_a.max((x - y).abs() / x.abs());
            }
            let flipped = (0..a.n_internal_faces()).filter(|&f| a.owner[f] > a.neighbour[f].unwrap()).count();
            println!(
                "gh1285 {name}: cells {}, internal faces {}, upper-triangular before {} after {}, faces reversed {flipped}; cell volumes bit-identical {same_v}/{} (worst rel {worst_v:e}); face areas bit-identical {same_a}/{} (worst rel {worst_a:e})",
                a.n_cells,
                a.n_internal_faces(),
                !before,
                !after,
                va.len(),
                aa.len()
            );
            assert!(worst_v < 1e-12 && worst_a < 1e-12, "{name}: round-off only");
            // Foam Studio's solver mesh: `outram_blender`'s `to_poly_mesh`
            // stable-sorts the internal faces by (owner, neighbour). On the
            // old order that sort gives exactly the new order (no face is
            // reversed), so the solver's polyMesh is the same either way.
            let mut sorted: Vec<usize> = (0..a.n_internal_faces()).collect();
            sorted.sort_by_key(|&f| (a.owner[f], a.neighbour[f]));
            let same_as_sorted = flipped == 0 && sorted.iter().enumerate().all(|(i, &f)| a.faces[f] == b.faces[i] && a.owner[f] == b.owner[i] && a.neighbour[f] == b.neighbour[i]);
            println!("gh1285 {name}: the new internal faces are to_poly_mesh's stable sort of the old ones: {same_as_sorted}");
            for (pa, pb) in a.patches.iter().zip(&b.patches) {
                assert_eq!((pa.name.as_str(), pa.start_face, pa.n_faces), (pb.name.as_str(), pb.start_face, pb.n_faces));
                for f in pa.start_face..pa.start_face + pa.n_faces {
                    assert_eq!((&a.faces[f], a.owner[f]), (&b.faces[f], b.owner[f]), "{name}: boundary face {f}");
                }
            }
        }
    }

    /// Images (gh:#1285), `--ignored`, feature `cell-centre-dual`: the
    /// internal-face order of the pipe's mesh before and after the final
    /// step. Each internal face `f` is a dot at (x = f, y = owner), red when
    /// `owner > neighbour`; an upper-triangular mesh is one rising staircase.
    /// The geometry does not change, so no slice is drawn (a slice of the
    /// two meshes is the same picture; `upper_triangular_order_moves_no_geometry`
    /// checks that numerically). PNGs go to `CFMESH_IMG_DIR` (default: the
    /// gh:#1285 V&V folder). Drawing only, nothing asserted.
    #[cfg(feature = "cell-centre-dual")]
    #[test]
    #[ignore]
    fn render_gh1285_face_order() {
        let dir = std::env::var("CFMESH_IMG_DIR").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/verification_and_validation/upper_triangular_gh1285").to_string());
        let (p, t, regions) = pipe_surface(0.01, 0.2, 48);
        let o = TetDualOptions { cell_size: 0.004, n_layers: 0, layer_patches: vec!["walls".into()], ..Default::default() };
        for (name, ordered) in [("before", false), ("after", true)] {
            let (m, _) = run_pipeline(&p, &t, &o, Some(&regions), ordered).unwrap();
            let (w, h) = (700usize, 700usize);
            let mut rgb = vec![255u8; 3 * w * h];
            let ni = m.n_internal_faces();
            for f in 0..ni {
                let o = m.owner[f];
                let x = f * (w - 1) / ni.max(1);
                let y = h - 1 - o * (h - 1) / m.n_cells.max(1);
                let c = if o > m.neighbour[f].unwrap() { [220u8, 30, 30] } else { [0u8, 0, 0] };
                rgb[3 * (y * w + x)..3 * (y * w + x) + 3].copy_from_slice(&c);
            }
            let path = std::path::Path::new(&dir).join(format!("face_order_h4_{name}.png"));
            crate::surface_optimizer::render_gh1259::write_png(&path, w, h, &rgb);
            println!("wrote {} ({} internal faces, {} cells)", path.display(), ni, m.n_cells);
        }
    }

    /// V&V (gh:#1215) — **the default layer strategy is pinned bit for bit.**
    /// Methodology: the multipatch pipeline with wall-only layers on the demo
    /// pipe (R 10 mm, L 60 mm, 48 facets) at h 6 / 4 mm with 1–3 layers, and a
    /// 20 × 20 × 60 mm box duct at h 5 mm with 2 layers; each final mesh is
    /// fingerprinted (points to the bit, faces, owners, neighbours, patches).
    ///
    /// Results (2026-11-04). ~~The fingerprints were recorded on the code
    /// before gh:#1215 and pin it.~~ **CORRECTED the same day:** recording them
    /// exposed that the pipeline was **not reproducible between processes**:
    /// the Delaunay flipper (`delaunay::flip_to_delaunay`) swept the faces in
    /// `HashMap` order, randomly seeded per process, so the h 6 mm pipe got a
    /// different mesh on every run (three runs, three fingerprints, same
    /// cell count); `tet_dual_stages_are_deterministic` compares two calls
    /// within one process and could not see it. The sweep now runs in
    /// `BTreeMap` order. The layer code itself was checked separately: the
    /// pre-gh:#1215 `add_boundary_layers_adaptive_on_patches` (a temporary
    /// copy from `ffda5fb46`) and the refactored one give identical meshes,
    /// points to the bit, on the bare h 6 / 4 / 5 mm pipes with 1–3 layers
    /// (7 cases). The fingerprints below are the deterministic ones; the h 4
    /// mm and box-duct values equal the first recording (the flip order did
    /// not change those meshes), the h 6 mm ones do not.
    /// **Re-pinned 2026-11-05 (gh:#1285):** the pipeline now ends in OpenFOAM's
    /// upper-triangular face order (`renumber_mesh::upper_triangular`); the
    /// geometry is unchanged (`upper_triangular_order_moves_no_geometry`:
    /// same points, cells and boundary faces, total volume bit for bit),
    /// but the fingerprint hashes the face order, so it moved. Cell counts
    /// did not. Old values struck through beside the new ones.
    #[test]
    fn back_off_only_layers_are_pinned_bit_for_bit() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let mut got = Vec::new();
        for (h, n) in [(0.006, 1), (0.006, 2), (0.006, 3), (0.004, 1), (0.004, 2), (0.004, 3)] {
            let o = TetDualOptions { cell_size: h, n_layers: n, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], ..Default::default() };
            let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
            got.push((format!("pipe h {h} n {n}"), m.cell_count(), mesh_fingerprint(&m)));
        }
        let (bp, bt) = box_surface(Vec3::ZERO, Vec3::new(0.02, 0.02, 0.06));
        let labels: Vec<&str> = bt
            .iter()
            .map(|tri| {
                let z = [bp[tri[0]].z, bp[tri[1]].z, bp[tri[2]].z];
                if z.iter().all(|&v| v.abs() < 1e-12) {
                    "inlet"
                } else if z.iter().all(|&v| (v - 0.06).abs() < 1e-12) {
                    "outlet"
                } else {
                    "walls"
                }
            })
            .collect();
        let br = SurfaceRegions::from_labels(&labels);
        let o = TetDualOptions { cell_size: 0.005, n_layers: 2, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], ..Default::default() };
        let (m, _) = surface_to_tet_dual_mesh_multipatch(&bp, &bt, &br, &o).unwrap();
        got.push(("box duct h 0.005 n 2".into(), m.cell_count(), mesh_fingerprint(&m)));
        // gh:#1285, old -> new: ~~7691211397797103741~~, ~~6777344945964783458~~,
        // ~~8963858472090204797~~, ~~11354576510400452468~~,
        // ~~6008187938627271744~~, ~~13850840632559401666~~,
        // ~~7672853737214871962~~ (pre-gh:#1285 face order).
        let pinned: [(usize, u64); 7] = [
            (2025, 6256401714239669657),
            (3445, 7331035643881703750),
            (4865, 18337414743125392665),
            (5511, 12495095278579800432),
            (9079, 10932642480450119360),
            (12647, 13769222349618449910),
            (5813, 15315517425586397770),
        ];
        for g in &got {
            eprintln!("fingerprint {g:?}");
        }
        for (g, want) in got.iter().zip(pinned) {
            assert_eq!((g.1, g.2), want, "{} moved", g.0);
        }
    }

    /// A box duct surface (`w` × `w` × `l`, z ∈ [0, l]) labelled `inlet`
    /// (z = 0), `outlet` (z = l) and `walls`.
    fn box_duct_surface(w: f64, l: f64) -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
        let (bp, bt) = box_surface(Vec3::ZERO, Vec3::new(w, w, l));
        let labels: Vec<&str> = bt
            .iter()
            .map(|tri| {
                let z = [bp[tri[0]].z, bp[tri[1]].z, bp[tri[2]].z];
                if z.iter().all(|&v| v.abs() < 1e-12) {
                    "inlet"
                } else if z.iter().all(|&v| (v - l).abs() < 1e-12) {
                    "outlet"
                } else {
                    "walls"
                }
            })
            .collect();
        (bp, bt, SurfaceRegions::from_labels(&labels))
    }

    const STRATEGIES: [LayerQualityStrategy; 3] =
        [LayerQualityStrategy::BackOffOnly, LayerQualityStrategy::CfMeshOptimise, LayerQualityStrategy::SnappyCollapse];

    /// V&V (gh:#1215) — **upstream sanity: a good mesh is left alone.**
    /// Methodology: a Cartesian (hexahedral, orthogonal) box duct 20 × 20 ×
    /// 60 mm at 5 mm, `inlet` / `outlet` at the ends, 3 layers (first
    /// 0.3 mm, × 1.2) on `walls`, by each strategy.
    ///
    /// Predictions (written 2026-11-04 before the first run): the layered
    /// duct passes snappy's strict controls everywhere, so
    /// `SnappyCollapse` converges in its first iteration with every wall
    /// point extruded, and its mesh is the back-off's to the bit (same
    /// columns, same builder); `CfMeshOptimise` finds no bad and no
    /// low-quality face on the flat walls, so the optimisers move no
    /// interior point (the duct's interior points stay bit-identical); its
    /// hairs are recomputed from the surface normals and smoothed, which on
    /// a flat side gives the side's normal back, so the layer thickness at
    /// the middle of each side equals the back-off's to 1e-12, and coverage
    /// is 100 %.
    #[test]
    fn every_strategy_leaves_a_good_box_duct_alone() {
        let (bp, bt, br) = box_duct_surface(0.02, 0.06);
        let hex = crate::patches::assign_patches_by_region(&carve_box(&bp, &bt, 0.005), &bp, &bt, &br).unwrap();
        let (back, bo) = crate::layers::add_boundary_layers_back_off(&hex, &["walls"], 3, 3e-4, 1.2);
        let (snap, so) = add_boundary_layers_snappy(&hex, &["walls"], 3, 3e-4, 1.2, &SnappyLayerControls::default());
        let (cfm, co) = crate::boundary_layer_optimisation::add_boundary_layers_cfmesh(&hex, &["walls"], 3, 3e-4, 1.2, &BoundaryLayerOptimisationControls::default());
        assert_eq!(bo.full_coverage(), 1.0);
        assert_eq!((so.full_coverage(), so.iterations, so.collapsed_point_fraction()), (1.0, 1, 0.0), "{:?}", so.notes);
        assert_eq!(mesh_fingerprint(&snap), mesh_fingerprint(&back), "snappy = back-off on a good duct");
        assert_eq!(co.full_coverage(), 1.0);
        assert_eq!(co.remaining_bad_faces, 0, "{:?}", co.notes);
        assert!(co.notes.iter().any(|n| n.starts_with("optimizeLowQualityFaces: low-quality faces 0 -> 0")), "{:?}", co.notes);
        // Interior (non-wall) points of the duct are not moved by cfMesh.
        let n_inner = hex.points.len() - bo.points.len();
        assert_eq!(&cfm.points[..n_inner], &back.points[..n_inner]);
        // Hairs at the middle point of each wall side, away from every edge
        // of the duct, stay along the face normal.
        for x in [Vec3::new(0.0, 0.01, 0.03), Vec3::new(0.01, 0.0, 0.03)] {
            let i = co.points.iter().position(|p| p.0.sub(x).length() < 1e-12).expect("mid point");
            assert!((co.points[i].2 - bo.points[i].2).abs() < 1e-12, "thickness kept at {x:?}: {} vs {}", co.points[i].2, bo.points[i].2);
        }
    }

    /// V&V (gh:#1215) — **both new strategies are deterministic**, within a
    /// process (two calls) and between processes (fingerprints pinned on
    /// 2026-11-04 and asserted here; any process gives these numbers).
    /// Methodology: the demo pipe (R 10 mm, L 60 mm, 48 facets) at h 5 mm
    /// with 3 wall-only layers, through the multipatch pipeline under each
    /// strategy.
    #[test]
    fn layer_strategies_are_deterministic() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let mut got = Vec::new();
        for strategy in [LayerQualityStrategy::CfMeshOptimise, LayerQualityStrategy::SnappyCollapse] {
            let o = TetDualOptions { cell_size: 0.005, n_layers: 3, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], layer_strategy: strategy, ..Default::default() };
            let (a, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
            let (b, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
            assert_eq!(mesh_fingerprint(&a), mesh_fingerprint(&b), "{strategy:?}");
            got.push((a.cell_count(), mesh_fingerprint(&a)));
        }
        eprintln!("strategy fingerprints {got:?}");
        // Recorded 2026-11-04 (serial smoothing); the threaded smoothing
        // must give the same bits. gh:#1285 (face order only, geometry
        // unchanged): ~~(7725, 11521056484853145326), (957,
        // 10164118906460112537)~~. gh:#1284 (Knupp makes one pass, as
        // upstream; CfMeshOptimise only, SnappyCollapse unchanged):
        // ~~(7725, 5422758738504758818)~~. gh:#1385 round 3 (the volume
        // optimiser, Knupp metric, tet-mesh centres and cell-list order
        // follow upstream; cell count unchanged, SnappyCollapse unchanged):
        // ~~(7725, 18177691582502802893)~~. gh:#1385 round 3, second move
        // (`findBadFaces` / `findLowQualityFaces` on the addressing's
        // geometry, `updateOrigMesh` marks every tet-mesh node as changed,
        // the LP untangler's divisions, `generateNewVertices`; cell count
        // unchanged): ~~(7725, 14814967247723517733)~~.
        assert_eq!(got, vec![(7725, 6667351423853767910), (957, 18444736245533413537)]);
    }

    /// Diagnostic (gh:#1215), `--ignored`: how many wall faces each
    /// wall-adjacent cell of the bare tet dual owns, and the angle between
    /// a wall face's normal and the line from its centre to its owner's
    /// centre (what a prism stacked on the face inherits as the
    /// top-prism/dual non-orthogonality).
    #[test]
    #[ignore]
    fn diagnose_wall_faces_per_dual_cell() {
        let (p, t, regions) = pipe_surface(0.01, 0.2, 48);
        let (bp, bt, br) = box_duct_surface(0.02, 0.2);
        for (name, sp, st, sr, h) in [("pipe h5", &p, &t, &regions, 0.005), ("pipe h4", &p, &t, &regions, 0.004), ("duct h5", &bp, &bt, &br, 0.005)] {
            let o = TetDualOptions { cell_size: h, n_layers: 0, ..Default::default() };
            let (m, _) = surface_to_tet_dual_mesh_multipatch(sp, st, sr, &o).unwrap();
            let (_, centres) = crate::checks::cell_geometry(&m);
            let walls = m.patches.iter().find(|q| q.name == "walls").unwrap();
            let mut per_cell: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
            let mut angles = Vec::new();
            for f in walls.start_face..walls.start_face + walls.n_faces {
                *per_cell.entry(m.owner[f]).or_insert(0) += 1;
                let s = m.face_area_vector(f);
                let d = centres[m.owner[f]].sub(m.face_centroid(f));
                angles.push((-d.dot(s) / (d.length() * s.length())).clamp(-1.0, 1.0).acos().to_degrees());
            }
            let mut hist = [0usize; 8];
            for &n in per_cell.values() {
                hist[n.min(7)] += 1;
            }
            angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let q = |f: f64| angles[((angles.len() - 1) as f64 * f) as usize];
            println!(
                "{name}: {} wall faces on {} cells; cells by wall-face count 1..7+: {:?}; face-to-owner angle median {:.1}, 90th {:.1}, max {:.1} deg; > 65 deg: {}",
                walls.n_faces,
                per_cell.len(),
                &hist[1..],
                q(0.5),
                q(0.9),
                q(1.0),
                angles.iter().filter(|&&a| a > 65.0).count()
            );
        }
    }

    /// Diagnostic (gh:#1215), `--ignored`: **the differential table.** Every
    /// strategy on the same inputs: the demo pipe (R 10 mm, L 200 mm, 48
    /// facets) at h 4 / 5 / 6 mm with 1–4 wall-only layers (first 0.3 mm,
    /// × 1.2), a tet-dual box duct 20 × 20 × 200 mm at h 5 mm with 2 and 3
    /// layers (its four long 90° edges are the sharp-feature case), and the
    /// same duct with snappy's `featureAngle` at the propeller tutorial's
    /// 60°. Prints mesh quality, counts beyond upstream's limits, coverage,
    /// thickness, time and a determinism check.
    #[test]
    #[ignore]
    fn diagnose_layer_strategies_differential() {
        let (p, t, regions) = pipe_surface(0.01, 0.2, 48);
        let (bp, bt, br) = box_duct_surface(0.02, 0.2);
        let mut cases: Vec<(String, &[Vec3], &[[usize; 3]], &SurfaceRegions, f64, usize, f64)> = Vec::new();
        for h in [0.006, 0.005, 0.004] {
            for n in 1..=4 {
                cases.push((format!("pipe h{} n{n}", h * 1e3), &p, &t, &regions, h, n, 100.0));
            }
        }
        for n in [2, 3] {
            cases.push((format!("duct h5 n{n}"), &bp, &bt, &br, 0.005, n, 100.0));
            cases.push((format!("duct h5 n{n} fa60"), &bp, &bt, &br, 0.005, n, 60.0));
        }
        let only = std::env::var("CFMESH_CASE").ok();
        for (name, sp, st, sr, h, n, fa) in cases {
            if only.as_deref().is_some_and(|o| !name.contains(o)) {
                continue;
            }
            for strategy in STRATEGIES {
                if fa < 100.0 && strategy != LayerQualityStrategy::SnappyCollapse {
                    continue;
                }
                let mut snappy = SnappyLayerControls::default();
                snappy.feature_angle_deg = fa;
                let o = TetDualOptions { cell_size: h, n_layers: n, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], layer_strategy: strategy, snappy_layers: snappy, ..Default::default() };
                let t0 = std::time::Instant::now();
                let (m, rep) = surface_to_tet_dual_mesh_multipatch(sp, st, sr, &o).unwrap();
                let secs = t0.elapsed().as_secs_f64();
                // Determinism (a second run) only on request: CFMESH_DET=1.
                let det = std::env::var("CFMESH_DET").is_err() || {
                    let (m2, _) = surface_to_tet_dual_mesh_multipatch(sp, st, sr, &o).unwrap();
                    mesh_fingerprint(&m) == mesh_fingerprint(&m2)
                };
                let q = check_quality(&m);
                let strict = crate::checks::faces_failing_mesh_quality(&m, &crate::checks::MeshQualityControls::default()).iter().filter(|&&b| b).count();
                let relaxed = crate::checks::faces_failing_mesh_quality(&m, &crate::checks::MeshQualityControls { max_non_ortho_deg: 75.0, ..Default::default() }).iter().filter(|&&b| b).count();
                let ortho_only = |deg: f64| {
                    crate::checks::faces_failing_mesh_quality(
                        &m,
                        &crate::checks::MeshQualityControls { max_non_ortho_deg: deg, max_boundary_skewness: -1.0, max_internal_skewness: -1.0, min_tet_quality: None, min_twist: -2.0, min_determinant: -1.0, min_face_weight: -1.0, min_vol_ratio: -1.0 },
                    )
                    .iter()
                    .filter(|&&b| b)
                    .count()
                };
                let lo = rep.layers.clone().unwrap_or_default();
                if strategy == LayerQualityStrategy::BackOffOnly && lo.full_coverage() == 1.0 {
                    // Which of snappy's checks the layer cells fail (faces of
                    // a cell added by the layerer), one check at a time.
                    let n0 = m.cell_count() - n * lo.faces.len();
                    let off = crate::checks::MeshQualityControls { max_non_ortho_deg: 180.0, max_boundary_skewness: -1.0, max_internal_skewness: -1.0, min_tet_quality: None, min_twist: -2.0, min_determinant: -1.0, min_face_weight: -1.0, min_vol_ratio: -1.0 };
                    let d = crate::checks::MeshQualityControls::default();
                    let one = |c: crate::checks::MeshQualityControls| {
                        let bad = crate::checks::faces_failing_mesh_quality(&m, &c);
                        (0..m.face_count()).filter(|&f| bad[f] && (m.owner[f] >= n0 || m.neighbour[f].is_some_and(|x| x >= n0))).count()
                    };
                    println!(
                        "  {name} layer-cell faces failing, one check at a time: nonOrtho65 {} nonOrtho75 {} intSkew4 {} bndSkew20 {} tet {} twist {} det {} weight {} volRatio {}",
                        one(crate::checks::MeshQualityControls { max_non_ortho_deg: 65.0, ..off.clone() }),
                        one(crate::checks::MeshQualityControls { max_non_ortho_deg: 75.0, ..off.clone() }),
                        one(crate::checks::MeshQualityControls { max_internal_skewness: d.max_internal_skewness, ..off.clone() }),
                        one(crate::checks::MeshQualityControls { max_boundary_skewness: d.max_boundary_skewness, ..off.clone() }),
                        one(crate::checks::MeshQualityControls { min_tet_quality: d.min_tet_quality, ..off.clone() }),
                        one(crate::checks::MeshQualityControls { min_twist: d.min_twist, ..off.clone() }),
                        one(crate::checks::MeshQualityControls { min_determinant: d.min_determinant, ..off.clone() }),
                        one(crate::checks::MeshQualityControls { min_face_weight: d.min_face_weight, ..off.clone() }),
                        one(crate::checks::MeshQualityControls { min_vol_ratio: d.min_vol_ratio, ..off.clone() }),
                    );
                }
                let layered: Vec<&(Vec3, usize, f64, f64)> = lo.points.iter().filter(|q| q.1 > 0).collect();
                let d1 = 1.0 / (1..=n).map(|k| 1.2f64.powi(k as i32 - 1)).sum::<f64>(); // first / total at n layers
                let first: Vec<f64> = layered.iter().filter(|q| q.1 == n).map(|q| q.2 * d1).collect();
                let mean = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
                let tot: Vec<f64> = layered.iter().map(|q| q.2).collect();
                let thinned = layered.iter().filter(|q| q.2 < 0.999 * q.3).count();
                println!(
                    "{name} {:?}: {} cells, {secs:.2} s, det {det}; nonortho max {:.1}, >65 {}, >75 {}; skew {:.2}; minDet {:.2e}; minVol {:.2e}; negPyr {}; fail strict {strict} relaxed {relaxed}; cover {:.3}, collapsed pts {:.3}, thinned pts {thinned}/{}, first {:.4} mm, total mean {:.4} min {:.4} mm; iters {}; notes {:?}",
                    strategy,
                    m.cell_count(),
                    q.max_non_orthogonality_deg,
                    ortho_only(65.0),
                    ortho_only(75.0),
                    q.max_skewness,
                    q.min_cell_determinant,
                    q.min_cell_volume,
                    q.n_negative_pyramid_faces,
                    lo.full_coverage(),
                    lo.collapsed_point_fraction(),
                    layered.len(),
                    1e3 * mean(&first),
                    1e3 * mean(&tot),
                    1e3 * tot.iter().cloned().fold(f64::MAX, f64::min).min(if tot.is_empty() { 0.0 } else { f64::MAX }),
                    lo.iterations,
                    rep.stage_notes
                );
            }
        }
    }

    /// A pipe surface (radius `r`, length `l`, `n_seg` sides, z ∈ [0, l]) with
    /// each triangle labelled `inlet` (flat at z = 0), `outlet` (flat at
    /// z = l) or `walls`.
    pub(crate) fn pipe_surface(r: f64, l: f64, n_seg: usize) -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
        let (p, t) = cylinder_surface(Vec3::ZERO, r, l, n_seg);
        let labels: Vec<&str> = t
            .iter()
            .map(|tri| {
                let z = [p[tri[0]].z, p[tri[1]].z, p[tri[2]].z];
                let flat = (z[0] - z[1]).abs() < 1e-12 && (z[1] - z[2]).abs() < 1e-12;
                match (flat, z[0] < 0.5 * l) {
                    (true, true) => "inlet",
                    (true, false) => "outlet",
                    _ => "walls",
                }
            })
            .collect();
        (p, t, SurfaceRegions::from_labels(&labels))
    }

    fn patch_faces(m: &VolumeMesh, name: &str) -> Vec<usize> {
        m.patches
            .iter()
            .find(|q| q.name == name)
            .map_or(Vec::new(), |q| (q.start_face..q.start_face + q.n_faces).collect())
    }

    /// V&V — **wall-only prism layers on a pipe** (gh:#1202, the port of
    /// cfMesh's per-patch `boundaryLayers`).
    ///
    /// Methodology: a pipe of radius 10 mm and length 40 mm (24-sided surface,
    /// `inlet` / `outlet` / `walls` regions), meshed through the multipatch
    /// pipeline at cell size 4 mm, once with no layers and once with 3 layers
    /// (first 0.3 mm, expansion 1.2) on `layer_patches = ["walls"]`; and once
    /// more with the same layers on the whole boundary (the old behaviour).
    ///
    /// Predictions, written before the first run (2026-11-04):
    /// 1. cells = cells(no layers) + 3 × (wall faces of the no-layer mesh),
    ///    exactly (every stage before the layers is deterministic and the
    ///    same in both runs);
    /// 2. the mesh is closed, has no inverted cell, and ~~keeps the no-layer
    ///    volume to 1e-9 relative (a repartition)~~;
    /// 3. ~~every inlet / outlet point stays on its plane (|z − z_end| < 1e-12)
    ///    and each end keeps its area to 1e-9 relative: the ends are
    ///    re-partitioned, not moved~~;
    ///
    /// **2 and 3 MISSED (first run, 2026-11-04), and the cause is upstream of
    /// the layers:** the end patches are not flat even with NO layers. The
    /// tet-dual pipeline has no feature-edge capture (cfMesh's
    /// `meshSurfaceEdgeExtractor` is not ported), so dual boundary faces at
    /// the rim wrap round the corner and are classified `inlet` by their
    /// nearest triangle: end-patch points reach 1.000 mm off the end planes
    /// on the bare mesh (a quarter of the 4 mm cell), and the bare volume is
    /// 1.8 % under the 24-gon prism (the rim is chamfered). A border point on
    /// such a bent face moves in the bent face's mean plane, so the boundary
    /// itself moves a little: volume −3.95e-6 relative, end-patch points
    /// 1.084 mm off the plane. The checks that replace them bound that by
    /// the geometry, not by the measurement: the volume may change by at most
    /// the band the wrapped faces sweep, `2 ends × 2πR × h_wrap × t / V`
    /// (h_wrap the bare off-plane distance, t the total layer thickness;
    /// 1.1e-2 here), and the off-plane distance may grow by at most t. The
    /// flat-end checks hold exactly on a box, whose ends are flat
    /// (`layers::tests::wall_only_layers_on_a_box_keep_the_ends`).
    /// 4. no prism is extruded off an end: the inlet faces owned by prism
    ///    cells are the layers' cross-section, an annulus about 2·t/R ≈ 0.2
    ///    of the inlet area (t ≈ 1.1 mm total, unless the caps thin it), so
    ///    between 0.02 and 0.35; with layers on the whole boundary every inlet
    ///    face is a prism's (fraction 1);
    /// 5. two runs give the identical mesh (points, faces, owners).
    #[test]
    fn wall_only_layers_end_on_the_pipe_ends() {
        let (r, l) = (0.01, 0.04);
        // CHANGED 2026-11-04 (gh:#1202): ~~24~~ 48 sides, with the feature-edge
        // capture on. On the 24-sided pipe (facet 2.6 mm under the 4 mm cell)
        // the capture leaves 105 boundary points unmapped
        // (`correctEdgesBetweenPatches` is not ported) and the boundary creases
        // (a 108° fold the layerer cannot clear); without the capture, the
        // prisms on the chamfered rim fail upstream's layer quality limits
        // (`layers::meets_layer_quality`), so no layer grows either way. On 48
        // sides, the demo's, the ends are flat: predictions 2 and 3 hold again
        // (volume kept to round-off; the bound below is the larger of the
        // wrapped band, now ~0, and 1e-9).
        let (p, t, regions) = pipe_surface(r, l, 48);
        let base = TetDualOptions { cell_size: 0.004, n_layers: 0, ..Default::default() };
        let (bare, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &base).unwrap();
        let opts = TetDualOptions {
            n_layers: 3,
            first_layer_thickness: 0.0003,
            expansion: 1.2,
            layer_patches: vec!["walls".into()],
            ..base.clone()
        };
        let (mesh, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &opts).unwrap();
        assert!(rep.stage_notes.iter().all(|n| !n.contains("boundary layers")), "{:?}", rep.stage_notes);
        // 1. exact cell count.
        let n_wall = patch_faces(&bare, "walls").len();
        assert_eq!(mesh.cell_count(), bare.cell_count() + 3 * n_wall);
        // 2. valid, and the volume moves only by the wrapped band (see above).
        mesh.validate().unwrap();
        assert_eq!(rep.n_negative_volume_cells, 0);
        let t_total = 0.0003 * (1.0 + 1.2 + 1.44);
        let off = |m: &VolumeMesh, name: &str, z0: f64| {
            patch_faces(m, name).iter().flat_map(|&f| m.faces[f].clone()).map(|v| (m.points[v].z - z0).abs()).fold(0.0_f64, f64::max)
        };
        let h_wrap = off(&bare, "inlet", 0.0).max(off(&bare, "outlet", l));
        let rel = (mesh.total_volume() - bare.total_volume()).abs() / bare.total_volume();
        let bound = (2.0 * 2.0 * PI * r * h_wrap * t_total / bare.total_volume()).max(1e-9);
        assert!(rel <= bound, "volume change {rel} over the wrapped band {bound}");
        // 3. the ends move by at most the layer thickness off their planes.
        let area = |m: &VolumeMesh, fs: &[usize]| fs.iter().map(|&f| m.face_area_vector(f).length()).sum::<f64>();
        let mut report = Vec::new();
        for (name, z0) in [("inlet", 0.0), ("outlet", l)] {
            let (o, o0) = (off(&mesh, name, z0), off(&bare, name, z0));
            assert!(o <= o0 + t_total, "{name} off its plane by {o} (bare {o0})");
            let a = area(&mesh, &patch_faces(&mesh, name)) / area(&bare, &patch_faces(&bare, name));
            report.push(format!("{name}: off-plane {:.3} mm (bare {:.3}), area x {a:.6}", 1e3 * o, 1e3 * o0));
        }
        eprintln!("volume change {rel:.3e} (bound {bound:.2e}); {}", report.join("; "));
        // 4. layers end on the inlet, not grown from it.
        let n0 = bare.cell_count();
        let inlet = patch_faces(&mesh, "inlet");
        let prism: Vec<usize> = inlet.iter().copied().filter(|&f| mesh.owner[f] >= n0).collect();
        let frac = area(&mesh, &prism) / area(&mesh, &inlet);
        assert!((0.02..0.35).contains(&frac), "prism share of the inlet {frac}");
        let whole = TetDualOptions { layer_patches: Vec::new(), ..opts.clone() };
        let (all, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &whole).unwrap();
        let inlet_all = patch_faces(&all, "inlet");
        let frac_all = inlet_all.iter().filter(|&&f| all.owner[f] >= n0).count() as f64 / inlet_all.len() as f64;
        assert!(frac_all > 0.99, "whole-boundary layers cover the inlet: {frac_all}");
        // 5. reproducible.
        let (again, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &opts).unwrap();
        assert_eq!(again.faces, mesh.faces);
        assert_eq!(again.owner, mesh.owner);
        assert!(again.points.iter().zip(&mesh.points).all(|(a, b)| a.x == b.x && a.y == b.y && a.z == b.z));
        eprintln!(
            "wall-only layers: {} cells ({} + 3 x {n_wall}), prism share of the inlet {frac:.4}, \
             max non-ortho {:.1} deg, max skewness {:.2}",
            mesh.cell_count(),
            n0,
            rep.max_non_orthogonality_deg,
            rep.max_skewness
        );
    }

    /// Diagnostic (gh:#1202), `--ignored`: list the boundary faces whose
    /// pyramid about their owner's centre is not positive (S·(c_f − C) ≤ 0)
    /// on the demo's layered pipes, with the owner's kind and the face's patch.
    #[test]
    #[ignore]
    fn diagnose_inward_boundary_faces_on_layered_pipes() {
        let (r, l) = (0.01, 0.2);
        let (p, t, regions) = pipe_surface(r, l, 48);
        for (h, n) in [(0.006, 0), (0.004, 0), (0.00334, 0), (0.006, 1), (0.005, 1), (0.005, 2), (0.004, 2), (0.005, 3), (0.004, 3)] {
            let base = TetDualOptions { cell_size: h, n_layers: 0, ..Default::default() };
            let (bare, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &base).unwrap();
            // Inlet / outlet projected areas against pi R^2, and the volume per
            // length of the middle half against pi R^2 (cells by centre).
            let proj = |name: &str| patch_faces(&bare, name).iter().map(|&f| bare.face_area_vector(f).z.abs()).sum::<f64>() / (PI * r * r);
            let (vols, cen) = crate::checks::cell_geometry(&bare);
            let mid: f64 = (0..bare.n_cells).filter(|&c| (cen[c].z - 0.5 * l).abs() < 0.25 * l).map(|c| vols[c]).sum::<f64>() / (0.5 * l) / (PI * r * r);
            println!("h {h} bare: inlet area/piR2 {:.4}, outlet {:.4}, mid-section area/piR2 {mid:.4}", proj("inlet"), proj("outlet"));
            let opts = TetDualOptions { n_layers: n, first_layer_thickness: 0.0003, expansion: 1.2, layer_patches: vec!["walls".into()], ..base.clone() };
            let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &opts).unwrap();
            let q = check_quality(&m);
            let (_, centres) = crate::checks::cell_geometry(&m);
            let n0 = bare.cell_count();
            let mut lines = Vec::new();
            for pa in &m.patches {
                for f in pa.start_face..pa.start_face + pa.n_faces {
                    let s = m.face_area_vector(f);
                    let c = m.face_centroid(f);
                    let o = m.owner[f];
                    let pyr = c.sub(centres[o]).dot(s);
                    if pyr <= 0.0 {
                        let rr = (c.x * c.x + c.y * c.y).sqrt();
                        let nz = s.z / s.length();
                        lines.push(format!(
                            "  {} face {f} ({} verts) owner {} {}: r {:.2} mm z {:.2} mm n_z {nz:.3} pyr/|S|^1.5 {:.3e}",
                            pa.name,
                            m.faces[f].len(),
                            o,
                            if o >= n0 { "prism" } else { "dual" },
                            1e3 * rr,
                            1e3 * c.z,
                            pyr / s.length().powf(1.5)
                        ));
                    }
                }
            }
            println!(
                "h {h} layers {n}: {} cells, neg pyramids {} (boundary {}), max non-ortho {:.1}, max skew {:.2}, min vol {:.3e}, min det {:.3e}, notes {:?}",
                m.cell_count(),
                q.n_negative_pyramid_faces,
                lines.len(),
                rep.max_non_orthogonality_deg,
                q.max_skewness,
                q.min_cell_volume,
                q.min_cell_determinant,
                rep.stage_notes
            );
            for li in lines.iter().take(30) {
                println!("{li}");
            }
        }
    }

    /// V&V (gh:#1202) — **every boundary face points outward** (a positive
    /// face pyramid about its owner's centre, `(c_f − C)·S_f > 0`) on layered
    /// pipes and a layered box duct, through the multipatch pipeline with
    /// wall-only layers.
    ///
    /// Methodology: a pipe R = 10 mm, L = 60 mm (48 facets) at h = 6 and
    /// 4 mm with 1 and 2 layers (first 0.3 mm, ×1.2), and a box duct
    /// 20 × 20 × 60 mm (`inlet` / `outlet` at the ends) at h = 5 mm with 1 and
    /// 2 layers. Pass: no boundary face with a non-positive pyramid.
    ///
    /// Result (2026-11-04): before the feature-edge capture
    /// (`surface_mapper`), the demo's 200 mm pipe at h 6 mm, 1 layer had 20
    /// such faces (19 on Foam Studio's cylinder), all on prisms extruded
    /// from the chamfered rim; with it, 0 on every mesh here.
    #[test]
    fn every_boundary_face_points_outward_on_layered_pipes_and_ducts() {
        let check = |m: &VolumeMesh, what: &str| {
            let (_, centres) = crate::checks::cell_geometry(m);
            let bad: Vec<usize> = (0..m.face_count())
                .filter(|&f| m.neighbour[f].is_none())
                .filter(|&f| !(m.face_centroid(f).sub(centres[m.owner[f]]).dot(m.face_area_vector(f)) > 0.0))
                .collect();
            assert!(bad.is_empty(), "{what}: {} inward boundary faces, e.g. {:?}", bad.len(), &bad[..bad.len().min(5)]);
        };
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        for (h, n) in [(0.006, 1), (0.004, 1), (0.004, 2)] {
            let o = TetDualOptions { cell_size: h, n_layers: n, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], ..Default::default() };
            let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
            check(&m, &format!("pipe h {h} layers {n} ({:?})", rep.stage_notes));
        }
        let (bp, bt) = box_surface(Vec3::ZERO, Vec3::new(0.02, 0.02, 0.06));
        let labels: Vec<&str> = bt
            .iter()
            .map(|tri| {
                let z = [bp[tri[0]].z, bp[tri[1]].z, bp[tri[2]].z];
                if z.iter().all(|&v| v.abs() < 1e-12) {
                    "inlet"
                } else if z.iter().all(|&v| (v - 0.06).abs() < 1e-12) {
                    "outlet"
                } else {
                    "walls"
                }
            })
            .collect();
        let bregions = SurfaceRegions::from_labels(&labels);
        for n in [1, 2] {
            let o = TetDualOptions { cell_size: 0.005, n_layers: n, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], ..Default::default() };
            let (m, rep) = surface_to_tet_dual_mesh_multipatch(&bp, &bt, &bregions, &o).unwrap();
            check(&m, &format!("box duct layers {n} ({:?})", rep.stage_notes));
        }
    }

    /// Diagnostic (gh:#1202), `--ignored`: mesh quality **per layer** on the
    /// demo's wall-only layered pipes (R 10 mm, L 200 mm, 48 facets): for the
    /// dual cells and for each prism layer k (cells `n0 + i·n + k`), the max
    /// non-orthogonality and OpenFOAM skewness over the cells' faces, the
    /// max OpenFOAM aspect ratio `(Σ|S_x| + Σ|S_y| + Σ|S_z|)/(6 V^(2/3))`
    /// (`primitiveMesh::checkClosedCells`), the min determinant, the mean
    /// layer thickness `V / |S_wall-side|`.
    #[test]
    #[ignore]
    fn diagnose_layer_quality_per_layer() {
        let (r, l) = (0.01, 0.2);
        let (p, t, regions) = pipe_surface(r, l, 48);
        for (h, n) in [(0.005, 1), (0.005, 2), (0.005, 3), (0.004, 1), (0.004, 2), (0.004, 3)] {
            let base = TetDualOptions { cell_size: h, n_layers: 0, ..Default::default() };
            let (bare, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &base).unwrap();
            let opts = TetDualOptions { n_layers: n, first_layer_thickness: 0.0003, expansion: 1.2, layer_patches: vec!["walls".into()], ..base.clone() };
            let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &opts).unwrap();
            let n0 = bare.cell_count();
            let (vols, centres) = crate::checks::cell_geometry(&m);
            let class = |c: usize| if c < n0 { 0 } else { 1 + (c - n0) % n };
            let k = n + 1;
            let (mut no, mut sk, mut ar, mut thick, mut cnt) = (vec![0.0_f64; k], vec![0.0_f64; k], vec![0.0_f64; k], vec![0.0_f64; k], vec![0usize; k]);
            let mut cmpt = vec![[0.0_f64; 3]; m.n_cells];
            let mut smax = vec![0.0_f64; m.n_cells];
            for f in 0..m.face_count() {
                let s = m.face_area_vector(f);
                let o = m.owner[f];
                for c in std::iter::once(o).chain(m.neighbour[f]) {
                    cmpt[c][0] += s.x.abs();
                    cmpt[c][1] += s.y.abs();
                    cmpt[c][2] += s.z.abs();
                    smax[c] = smax[c].max(s.length());
                }
                let Some(nb) = m.neighbour[f] else { continue };
                let d = centres[nb].sub(centres[o]);
                let cos = (d.dot(s) / (d.length() * s.length())).clamp(-1.0, 1.0);
                let ang = cos.acos().to_degrees();
                let fc = m.face_centroid(f);
                let cpf = fc.sub(centres[o]);
                let sv = cpf.sub(d.scale(s.dot(cpf) / (s.dot(d) + 1e-300)));
                let svh = sv.scale(1.0 / (sv.length() + 1e-300));
                let mut fd = 0.2 * d.length() + 1e-300;
                for &v in &m.faces[f] {
                    fd = fd.max(svh.dot(m.points[v].sub(fc)).abs());
                }
                let skew = sv.length() / fd;
                for c in [o, nb] {
                    no[class(c)] = no[class(c)].max(ang);
                    sk[class(c)] = sk[class(c)].max(skew);
                }
            }
            for c in 0..m.n_cells {
                let a = (cmpt[c][0] + cmpt[c][1] + cmpt[c][2]) / (6.0 * vols[c].powf(2.0 / 3.0));
                ar[class(c)] = ar[class(c)].max(a);
                if c >= n0 {
                    thick[class(c)] += vols[c] / smax[c];
                    cnt[class(c)] += 1;
                }
            }
            println!("h {h} layers {n}: {} cells, notes {:?}", m.cell_count(), rep.stage_notes);
            for i in 0..k {
                let name = if i == 0 { "dual".to_string() } else { format!("layer {i}") };
                let th = if cnt[i] > 0 { format!("{:.3} mm", 1e3 * thick[i] / cnt[i] as f64) } else { "-".into() };
                println!("  {name:8}: max non-ortho {:5.1} deg, max skew {:.2}, max aspect {:6.1}, mean thickness {th}", no[i], sk[i], ar[i]);
            }
        }
    }

    /// Per internal face, the non-orthogonality (deg) as [`check_quality`]
    /// measures it; per boundary face of `patch`, the angle between its normal
    /// and the line from its owner's centre to its centre (what a prism stacked
    /// on it inherits); and the number of `patch` faces each owner has.
    fn face_angles(m: &VolumeMesh, patch: &str) -> (Vec<f64>, Vec<f64>, std::collections::BTreeMap<usize, usize>) {
        let (_, c) = crate::checks::cell_geometry(m);
        let mut internal = Vec::new();
        for f in 0..m.face_count() {
            if let Some(nb) = m.neighbour[f] {
                let (d, s) = (c[nb].sub(c[m.owner[f]]), m.face_area_vector(f));
                internal.push((d.dot(s) / (d.length() * s.length())).clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        let mut wall = Vec::new();
        let mut per_cell = std::collections::BTreeMap::new();
        if let Some(w) = m.patches.iter().find(|q| q.name == patch) {
            for f in w.start_face..w.start_face + w.n_faces {
                *per_cell.entry(m.owner[f]).or_insert(0usize) += 1;
                let (d, s) = (m.face_centroid(f).sub(c[m.owner[f]]), m.face_area_vector(f));
                wall.push((d.dot(s) / (d.length() * s.length())).clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        (internal, wall, per_cell)
    }

    /// `p`-quantile of a sorted list (0 for an empty one).
    fn quantile(v: &[f64], p: f64) -> f64 {
        if v.is_empty() {
            0.0
        } else {
            v[((v.len() - 1) as f64 * p).round() as usize]
        }
    }

    /// Diagnostic (gh:#1240), `--ignored`, run with `--features
    /// cell-centre-dual`: **the dual-kind differential table.** Every
    /// [`DualKind`] on the same tet meshes: the demo pipe (R 10 mm, L 200 mm,
    /// 48 facets) at h 4 / 5 / 6 mm and the tet-dual box duct 20 × 20 ×
    /// 200 mm at h 5 mm. Per bare mesh: cells, wall faces per wall cell,
    /// the internal non-orthogonality distribution and the wall faces'
    /// face-to-owner angle, faceSkewness, closure, volume and inlet area
    /// against the analytic section, and a fingerprint; then 1–3 wall layers
    /// (first 0.3 mm, × 1.2) under each `LayerQualityStrategy`. Filters:
    /// `CFMESH_CASE`, `CFMESH_DUAL` (`corner`, `point`, `centre`),
    /// `CFMESH_LAYERS` (e.g. `0` for bare only, `3`), `CFMESH_STRATS`
    /// (`backoff,snappy,cfmesh`); `CFMESH_DET=1` re-runs each bare mesh.
    #[test]
    #[ignore]
    fn diagnose_dual_kinds_differential() {
        let (r, l) = (0.01, 0.2);
        let (p, t, regions) = pipe_surface(r, l, 48);
        let (bp, bt, br) = box_duct_surface(0.02, l);
        let polygon = 0.5 * 48.0 * r * r * (2.0 * PI / 48.0).sin();
        let env = |k: &str| std::env::var(k).ok();
        let cases: Vec<(String, &[Vec3], &[[usize; 3]], &SurfaceRegions, f64, f64)> = vec![
            ("pipe h4".into(), &p, &t, &regions, 0.004, PI * r * r),
            ("pipe h5".into(), &p, &t, &regions, 0.005, PI * r * r),
            ("pipe h6".into(), &p, &t, &regions, 0.006, PI * r * r),
            ("duct h5".into(), &bp, &bt, &br, 0.005, 0.02 * 0.02),
        ];
        let kinds = [(DualKind::MedianPerCorner, "corner"), (DualKind::MedianPerPoint, "point"), (DualKind::CellCentre, "centre")];
        // CFMESH_FP=corners: the polyDualMesh class's feature points.
        let fp = if env("CFMESH_FP").as_deref() == Some("corners") { FeaturePointRule::Corners } else { FeaturePointRule::OnFeatureEdge };
        let layers: Vec<usize> = env("CFMESH_LAYERS").map_or(vec![0, 1, 2, 3], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
        let strats: Vec<(LayerQualityStrategy, &str)> = [(LayerQualityStrategy::BackOffOnly, "backoff"), (LayerQualityStrategy::SnappyCollapse, "snappy"), (LayerQualityStrategy::CfMeshOptimise, "cfmesh")]
            .into_iter()
            .filter(|s| env("CFMESH_STRATS").is_none_or(|v| v.split(',').any(|x| x == s.1)))
            .collect();
        for (name, sp, st, sr, h, section) in cases {
            if env("CFMESH_CASE").is_some_and(|o| !name.contains(&o)) {
                continue;
            }
            for (kind, tag) in kinds {
                if env("CFMESH_DUAL").is_some_and(|o| !o.split(',').any(|x| x == tag)) {
                    continue;
                }
                for &n in &layers {
                    for (strategy, stag) in &strats {
                        if n == 0 && *strategy != LayerQualityStrategy::BackOffOnly {
                            continue;
                        }
                        let o = TetDualOptions { cell_size: h, n_layers: n, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], layer_strategy: *strategy, dual_kind: kind, dual_feature_points: fp, cfmesh_layer_optimisation: BoundaryLayerOptimisationControls {
                            stop_untangling_on_stall: env("CFMESH_STALL").as_deref() != Some("0"),
                            // CFMESH_BMOVES=1: untangleMeshFV's boundary loop (gh:#1255).
                            untangle_boundary_moves: env("CFMESH_BMOVES").as_deref() == Some("1"),
                            // CFMESH_LAPLACE=1: optimizeMeshFV's laplacianPC (gh:#1256).
                            optimize_mesh_fv_laplace: env("CFMESH_LAPLACE").as_deref() == Some("1"),
                            // CFMESH_LPU=1: the LP meshUntangler (gh:#1260).
                            untangle_mesh_untangler: env("CFMESH_LPU").as_deref() == Some("1"),
                            ..Default::default()
                        }, ..Default::default() };
                        let t0 = std::time::Instant::now();
                        let (m, rep) = match surface_to_tet_dual_mesh_multipatch(sp, st, sr, &o) {
                            Ok(x) => x,
                            Err(e) => {
                                println!("{name} {tag} n{n} {stag}: ERROR {e}");
                                continue;
                            }
                        };
                        let secs = t0.elapsed().as_secs_f64();
                        let q = check_quality(&m);
                        let (mut inner, mut wall, per_cell) = face_angles(&m, "walls");
                        inner.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        wall.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let above = |v: &[f64], a: f64| v.iter().filter(|&&x| x > a).count();
                        let mean = inner.iter().sum::<f64>() / inner.len().max(1) as f64;
                        if n == 0 {
                            let det = env("CFMESH_DET").is_none() || {
                                let (m2, _) = surface_to_tet_dual_mesh_multipatch(sp, st, sr, &o).unwrap();
                                mesh_fingerprint(&m) == mesh_fingerprint(&m2)
                            };
                            let mut hist = [0usize; 8];
                            for &k in per_cell.values() {
                                hist[k.min(7)] += 1;
                            }
                            let inlet: f64 = patch_faces(&m, "inlet").iter().map(|&f| m.face_area_vector(f).length()).sum();
                            println!(
                                "{name} {tag} bare: {} cells, {secs:.2} s, valid {}, det {det}, fp {}; wall faces {} on {} cells, per cell 1..7+ {:?}; \
                                 internal NO mean {mean:.1} p50 {:.1} p90 {:.1} p99 {:.1} max {:.1}, >65 {} >70 {} >75 {}; \
                                 wall face-to-owner p50 {:.1} p90 {:.1} max {:.1}, >65 {}; skew {:.2}; minDet {:.2e}; \
                                 V/(section L) {:.5} (48-gon {:.5}); inlet/section {:.5}; notes {:?}",
                                m.cell_count(),
                                m.validate().is_ok(),
                                mesh_fingerprint(&m),
                                wall.len(),
                                per_cell.len(),
                                &hist[1..],
                                quantile(&inner, 0.5),
                                quantile(&inner, 0.9),
                                quantile(&inner, 0.99),
                                quantile(&inner, 1.0),
                                above(&inner, 65.0),
                                above(&inner, 70.0),
                                above(&inner, 75.0),
                                quantile(&wall, 0.5),
                                quantile(&wall, 0.9),
                                quantile(&wall, 1.0),
                                above(&wall, 65.0),
                                q.max_skewness,
                                q.min_cell_determinant,
                                m.total_volume() / (section * l),
                                if name.starts_with("pipe") { m.total_volume() / (polygon * l) } else { f64::NAN },
                                inlet / section,
                                rep.stage_notes
                            );
                        } else {
                            let lo = rep.layers.clone().unwrap_or_default();
                            let strict = crate::checks::faces_failing_mesh_quality(&m, &crate::checks::MeshQualityControls::default()).iter().filter(|&&b| b).count();
                            if *strategy == LayerQualityStrategy::BackOffOnly && lo.full_coverage() == 1.0 {
                                // Which of snappy's checks the layer cells' faces fail, one at a time.
                                let n0 = m.cell_count() - n * lo.faces.len();
                                let off = crate::checks::MeshQualityControls { max_non_ortho_deg: 180.0, max_boundary_skewness: -1.0, max_internal_skewness: -1.0, min_tet_quality: None, min_twist: -2.0, min_determinant: -1.0, min_face_weight: -1.0, min_vol_ratio: -1.0 };
                                let d = crate::checks::MeshQualityControls::default();
                                let one = |c: crate::checks::MeshQualityControls| {
                                    let bad = crate::checks::faces_failing_mesh_quality(&m, &c);
                                    (0..m.face_count()).filter(|&f| bad[f] && (m.owner[f] >= n0 || m.neighbour[f].is_some_and(|x| x >= n0))).count()
                                };
                                println!(
                                    "  {name} {tag} n{n} layer-cell faces failing, one check at a time: nonOrtho65 {} intSkew4 {} bndSkew20 {} tet {} twist {} det {} weight {} volRatio {}",
                                    one(crate::checks::MeshQualityControls { max_non_ortho_deg: 65.0, ..off.clone() }),
                                    one(crate::checks::MeshQualityControls { max_internal_skewness: d.max_internal_skewness, ..off.clone() }),
                                    one(crate::checks::MeshQualityControls { max_boundary_skewness: d.max_boundary_skewness, ..off.clone() }),
                                    one(crate::checks::MeshQualityControls { min_tet_quality: d.min_tet_quality, ..off.clone() }),
                                    one(crate::checks::MeshQualityControls { min_twist: d.min_twist, ..off.clone() }),
                                    one(crate::checks::MeshQualityControls { min_determinant: d.min_determinant, ..off.clone() }),
                                    one(crate::checks::MeshQualityControls { min_face_weight: d.min_face_weight, ..off.clone() }),
                                    one(crate::checks::MeshQualityControls { min_vol_ratio: d.min_vol_ratio, ..off.clone() }),
                                );
                            }
                            println!(
                                "{name} {tag} n{n} {stag}: {} cells, {secs:.1} s; cover {:.3}, collapsed pts {:.3}, mean total {:.3} mm; NO max {:.1} p99 {:.1}, >65 {} >75 {}; skew {:.2}; minDet {:.2e}; fail strict {strict}; V/(section L) {:.5}, inlet/section {:.5}; notes {:?}",
                                m.cell_count(),
                                lo.full_coverage(),
                                lo.collapsed_point_fraction(),
                                1e3 * lo.mean_total_thickness(),
                                quantile(&inner, 1.0),
                                quantile(&inner, 0.99),
                                above(&inner, 65.0),
                                above(&inner, 75.0),
                                q.max_skewness,
                                q.min_cell_determinant,
                                m.total_volume() / (section * l),
                                patch_faces(&m, "inlet").iter().map(|&f| m.face_area_vector(f).length()).sum::<f64>() / section,
                                rep.stage_notes
                            );
                        }
                    }
                }
            }
        }
    }

    /// Differential (gh:#1261), `--ignored`: cfMesh's `checkMinTwist`,
    /// `checkCellDeterminant` and `checkMinVolRatio`
    /// ([`crate::poly_mesh_gen_checks`]) against OpenFOAM's definitions
    /// ([`crate::checks::faces_failing_mesh_quality`], one control at a
    /// time) on the same meshes: the 48-sided pipe (R 10 mm, L 200 mm) at h
    /// 5 mm, bare and with 3 back-off layers (the default dual and layer
    /// strategy), at OpenFOAM's thresholds (twist 0.02, determinant 0.001,
    /// volume ratio 0.01) for both.
    ///
    /// Predictions (written 2026-11-05, before the run):
    /// - **volume ratio:** the same formula on the same volumes (`min/max`
    ///   of the two cells' |V|), so the same faces, exactly;
    /// - **twist:** the same formula on internal faces (unit triangle
    ///   normals against the unit centre-to-centre direction). On boundary
    ///   faces the two differ: cfMesh gives a triangular boundary face a
    ///   zero direction, so with `minTwist` 0.02 every boundary triangle
    ///   fails, and OpenFOAM checks no triangle. So the cfMesh set ⊇ the
    ///   OpenFOAM set, and the extra faces are exactly the boundary
    ///   triangles (if the dual has any);
    /// - **determinant:** different definitions (cfMesh: all faces,
    ///   normalised by Σ|S| and 1/27; OpenFOAM: internal faces only, by
    ///   their mean area). They should agree on the bare mesh's interior to
    ///   within a few cells, and OpenFOAM's flags more cells at the wall
    ///   (it drops the boundary faces, so a wall cell's tensor loses a
    ///   direction): OpenFOAM's count ≥ cfMesh's.
    ///
    /// Results (2026-11-05): bare, 3 113 cells, no boundary triangles:
    /// volume ratio 0 / 0, twist 152 / 152 (the same faces), determinant
    /// 0 / 0. With 3 layers, 26 009 cells: volume ratio 2 536 / 2 536 and
    /// twist 160 / 160 (the same faces), determinant **0 (cfMesh) / 352
    /// (OpenFOAM)**. Every prediction held; the boundary-triangle case did
    /// not arise (the dual has none).
    #[test]
    #[ignore]
    fn diagnose_cfmesh_against_openfoam_checks() {
        use crate::checks::{faces_failing_mesh_quality, MeshQualityControls};
        use crate::poly_mesh_gen_checks::{check_cell_determinant, check_min_twist, check_min_vol_ratio};
        let (r, l) = (0.01, 0.2);
        let (p, t, regions) = pipe_surface(r, l, 48);
        let off = MeshQualityControls { max_non_ortho_deg: 180.0, max_boundary_skewness: -1.0, max_internal_skewness: -1.0, min_tet_quality: None, min_twist: -2.0, min_determinant: -1.0, min_face_weight: -1.0, min_vol_ratio: -1.0 };
        for n in [0usize, 3] {
            let o = TetDualOptions { cell_size: 0.005, n_layers: n, first_layer_thickness: 3e-4, expansion: 1.2, layer_patches: vec!["walls".into()], ..Default::default() };
            let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).expect("pipe meshes");
            let all = vec![true; m.face_count()];
            let cmp = |name: &str, cf: &[bool], of: &[bool]| {
                let both = (0..m.face_count()).filter(|&f| cf[f] && of[f]).count();
                let only_cf = (0..m.face_count()).filter(|&f| cf[f] && !of[f]).collect::<Vec<_>>();
                let only_of = (0..m.face_count()).filter(|&f| !cf[f] && of[f]).collect::<Vec<_>>();
                let tri_bnd = only_cf.iter().filter(|&&f| m.neighbour[f].is_none() && m.faces[f].len() == 3).count();
                println!(
                    "pipe h5 n{n} {name}: cfMesh {} OpenFOAM {} both {both}; only cfMesh {} ({} boundary triangles); only OpenFOAM {}",
                    cf.iter().filter(|&&b| b).count(),
                    of.iter().filter(|&&b| b).count(),
                    only_cf.len(),
                    tri_bnd,
                    only_of.len()
                );
            };
            println!("pipe h5 n{n}: {} cells, {} faces ({} boundary triangles)", m.n_cells, m.face_count(), (m.n_internal_faces()..m.face_count()).filter(|&f| m.faces[f].len() == 3).count());
            cmp("volume ratio 0.01", &check_min_vol_ratio(&m, 0.01, Some(&all)).1, &faces_failing_mesh_quality(&m, &MeshQualityControls { min_vol_ratio: 0.01, ..off.clone() }));
            cmp("twist 0.02", &check_min_twist(&m, 0.02, Some(&all)).1, &faces_failing_mesh_quality(&m, &MeshQualityControls { min_twist: 0.02, ..off.clone() }));
            cmp("determinant 0.001", &check_cell_determinant(&m, 0.001, Some(&all)).1, &faces_failing_mesh_quality(&m, &MeshQualityControls { min_determinant: 0.001, ..off.clone() }));
        }
    }

    /// Diagnostic (gh:#1240), `--ignored`: why the back-off grows no layer on
    /// the per-point duals. On the bare h 5 mm pipe of each dual kind, the
    /// back-off's columns at the cap scaled by 0.6^k, and what each candidate
    /// fails: closure, inverted cells, a folded face, `meets_layer_quality`'s
    /// skewness / pyramid / determinant.
    #[test]
    #[ignore]
    fn diagnose_back_off_on_dual_kinds() {
        let (p, t, regions) = pipe_surface(0.01, 0.2, 48);
        let h: f64 = std::env::var("CFMESH_H").ok().and_then(|v| v.parse().ok()).unwrap_or(0.005);
        for kind in [DualKind::MedianPerCorner, DualKind::MedianPerPoint, DualKind::CellCentre] {
            let fp = if std::env::var("CFMESH_FP").as_deref() == Ok("corners") { FeaturePointRule::Corners } else { FeaturePointRule::OnFeatureEdge };
            let o = TetDualOptions { cell_size: h, n_layers: 0, dual_kind: kind, dual_feature_points: fp, ..Default::default() };
            let (bare, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
            for n in [1usize, 3] {
                let s = crate::layers::layer_setup(&bare, &["walls"], n, 3e-4, 1.2).unwrap();
                let counts: std::collections::BTreeMap<usize, usize> = s.wall_pts.iter().map(|&wp| (wp, n)).collect();
                let caps: Vec<f64> = s.cap.values().copied().collect();
                let mut row = Vec::new();
                for k in 0..10 {
                    let sc = 0.6f64.powi(k);
                    let per: std::collections::BTreeMap<usize, f64> = s.cap.iter().map(|(&wp, &c)| (wp, c * sc)).collect();
                    let c = crate::layers::build_layered_variable(&bare, &s.wall_faces, &s.wall_pts, &s.inward, &s.d, &per, &counts);
                    let q = check_quality(&c);
                    row.push(format!(
                        "x{sc:.3}: valid {} neg {} NO {:.1} skew {:.2} negPyr {} minDet {:.1e}",
                        c.validate().is_ok(),
                        q.n_negative_volume_cells,
                        q.max_non_orthogonality_deg,
                        q.max_skewness,
                        q.n_negative_pyramid_faces,
                        q.min_cell_determinant
                    ));
                }
                println!(
                    "{kind:?} n{n}: {} wall faces, {} wall points, cap min {:.2e} mean {:.2e} m\n  {}",
                    s.wall_faces.len(),
                    s.wall_pts.len(),
                    caps.iter().cloned().fold(f64::MAX, f64::min),
                    caps.iter().sum::<f64>() / caps.len() as f64,
                    row.join("\n  ")
                );
            }
        }
    }

    /// Diagnostic (gh:#1240), `--ignored`: on the bare per-point h 5 mm pipe,
    /// the faces whose pyramid about the owner (or neighbour) centre is not
    /// positive, the wall faces with a zero-length edge, and the faces with
    /// the largest non-orthogonality, with where they are.
    #[test]
    #[ignore]
    fn diagnose_per_point_bad_faces() {
        let (p, t, regions) = pipe_surface(0.01, 0.2, 48);
        let o = TetDualOptions { cell_size: 0.005, n_layers: 0, dual_kind: DualKind::MedianPerPoint, ..Default::default() };
        let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        let (_, c) = crate::checks::cell_geometry(&m);
        let patch = |f: usize| m.patches.iter().find(|q| f >= q.start_face && f < q.start_face + q.n_faces).map_or("-", |q| q.name.as_str());
        let mut n_pyr = 0;
        for f in 0..m.face_count() {
            let (s, fc) = (m.face_area_vector(f), m.face_centroid(f));
            let own = s.dot(fc.sub(c[m.owner[f]])) <= 0.0;
            let nb = m.neighbour[f].is_some_and(|n| s.dot(c[n].sub(fc)) <= 0.0);
            let ring = &m.faces[f];
            let min_edge = (0..ring.len()).map(|i| m.points[ring[i]].sub(m.points[ring[(i + 1) % ring.len()]]).length()).fold(f64::MAX, f64::min);
            if own || nb {
                n_pyr += 1;
                if n_pyr <= 12 {
                    let r = (fc.x * fc.x + fc.y * fc.y).sqrt();
                    println!("negative pyramid: face {f} ({}), {} verts, centre r {:.3} mm z {:.3} mm, own {own} nb {nb}, min edge {:.2e}", patch(f), ring.len(), 1e3 * r, 1e3 * fc.z, min_edge);
                }
            }
            if m.neighbour[f].is_none() && min_edge < 1e-9 {
                println!("zero edge: face {f} ({}), {} verts, z {:.3} mm, ring {:?}", patch(f), ring.len(), 1e3 * fc.z, ring);
            }
        }
        println!("{n_pyr} faces with a non-positive pyramid");
    }

    /// V&V (gh:#1240) — **the dual kinds, pinned bit for bit across
    /// processes**, and the wall-face count they exist for. Methodology: the
    /// demo pipe (R 10 mm, L 200 mm, 48 facets) at h 5 mm, bare, through the
    /// multipatch pipeline with each dual kind and each feature-point rule;
    /// fingerprints recorded 2026-11-04 in two separate processes
    /// (`diagnose_dual_kinds_differential`, identical) and asserted here.
    /// Results: every kind gives 3 113 cells (one per primal point); the
    /// per-corner dual owns 7 632 wall faces on 1 288 wall cells, the
    /// per-point and cell-centre duals 1 320 (`OnFeatureEdge`) or 1 304
    /// (`Corners`), at most 2 per cell. Needs `cell-centre-dual`.
    #[cfg(feature = "cell-centre-dual")]
    #[test]
    fn dual_kinds_are_pinned_bit_for_bit() {
        let (p, t, regions) = pipe_surface(0.01, 0.2, 48);
        let cases = [
            // gh:#1285 (face order only, geometry unchanged), old pins:
            // ~~16736591880388189566~~, ~~5331245050290215594~~,
            // ~~1937584253539663578~~, ~~3093209209138859925~~,
            // ~~10005039613933009028~~.
            (DualKind::MedianPerCorner, FeaturePointRule::OnFeatureEdge, 7632, 12398942909958749874u64),
            (DualKind::MedianPerPoint, FeaturePointRule::OnFeatureEdge, 1320, 11035141598418972758),
            (DualKind::CellCentre, FeaturePointRule::OnFeatureEdge, 1320, 5798262695090360502),
            (DualKind::MedianPerPoint, FeaturePointRule::Corners, 1304, 6679129286926630001),
            (DualKind::CellCentre, FeaturePointRule::Corners, 1304, 2459340814925991440),
        ];
        let mut got = Vec::new();
        for (kind, fp, n_wall, pin) in cases {
            let o = TetDualOptions { cell_size: 0.005, n_layers: 0, dual_kind: kind, dual_feature_points: fp, ..Default::default() };
            let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
            assert!(rep.stage_notes.iter().all(|n| !n.contains("dual skipped")), "{kind:?}: {:?}", rep.stage_notes);
            assert_eq!(m.cell_count(), 3113, "{kind:?}");
            let (_, _, per_cell) = face_angles(&m, "walls");
            assert_eq!(per_cell.values().sum::<usize>(), n_wall, "{kind:?} {fp:?} wall faces");
            if kind != DualKind::MedianPerCorner {
                assert!(per_cell.values().all(|&k| k <= 2), "{kind:?} {fp:?}");
            }
            eprintln!("dual fingerprint {kind:?} {fp:?} {}", mesh_fingerprint(&m));
            got.push((kind, fp, mesh_fingerprint(&m), pin));
        }
        for (kind, fp, g, pin) in got {
            assert_eq!(g, pin, "{kind:?} {fp:?} moved");
        }
    }

    /// Diagnostic (gh:#1241 item 3), `--ignored`: snappy's unextrusion,
    /// iteration by iteration, under both face-cell rules
    /// (`FaceLayerCells::Upstream`, `MostOfItsPoints`), on the `Corners`
    /// pipes (`CFMESH_DUAL`: point, centre; `CFMESH_HS`, `CFMESH_LAYERS`).
    /// Prints each iteration's note and the coverage.
    #[test]
    #[ignore]
    fn diagnose_snappy_cascade() {
        use crate::snappy_layers::{FaceLayerCells, SnappyLayerControls};
        let (p, t, regions) = pipe_surface(0.01, 0.2, 48);
        let env = |k: &str| std::env::var(k).ok();
        let hs: Vec<f64> = env("CFMESH_HS").map_or(vec![0.005], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
        let layers: Vec<usize> = env("CFMESH_LAYERS").map_or(vec![1, 2, 3], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
        let kinds = [(DualKind::MedianPerPoint, "point"), (DualKind::CellCentre, "centre")];
        for h in hs {
            for (kind, tag) in kinds {
                if env("CFMESH_DUAL").is_some_and(|o| !o.split(',').any(|x| x == tag)) {
                    continue;
                }
                {
                    // Wall-face non-planarity of the bare mesh against the layers' thickness.
                    let o = TetDualOptions { cell_size: h, n_layers: 0, dual_kind: kind, dual_feature_points: FeaturePointRule::Corners, ..Default::default() };
                    let (bare, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
                    let mut warp: Vec<f64> = patch_faces(&bare, "walls")
                        .iter()
                        .map(|&f| {
                            let (s, c) = (bare.face_area_vector(f), bare.face_centroid(f));
                            let nrm = s.scale(1.0 / s.length());
                            bare.faces[f].iter().map(|&v| bare.points[v].sub(c).dot(nrm).abs()).fold(0.0, f64::max)
                        })
                        .collect();
                    warp.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let st = crate::layers::layer_setup(&bare, &["walls"], 3, 3e-4, 1.2).unwrap();
                    let mut caps: Vec<f64> = st.cap.values().copied().collect();
                    caps.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    println!(
                        "h {:.0} {tag}: wall-face non-planarity p50 {:.3} p90 {:.3} max {:.3} mm; per-point total (cap) p10 {:.3} p50 {:.3} mm; profile d {:?}",
                        h * 1e3,
                        1e3 * quantile(&warp, 0.5),
                        1e3 * quantile(&warp, 0.9),
                        1e3 * quantile(&warp, 1.0),
                        1e3 * quantile(&caps, 0.1),
                        1e3 * quantile(&caps, 0.5),
                        st.d
                    );
                }
                for &n in &layers {
                    for rule in [FaceLayerCells::MostOfItsPoints, FaceLayerCells::Upstream] {
                        let o = TetDualOptions {
                            cell_size: h,
                            n_layers: n,
                            first_layer_thickness: 3e-4,
                            expansion: 1.2,
                            layer_patches: vec!["walls".into()],
                            layer_strategy: LayerQualityStrategy::SnappyCollapse,
                            dual_kind: kind,
                            dual_feature_points: FeaturePointRule::Corners,
                            snappy_layers: SnappyLayerControls { face_cells: rule, ..Default::default() },
                            ..Default::default()
                        };
                        let t0 = std::time::Instant::now();
                        let (m, rep) = match surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o) {
                            Ok(x) => x,
                            Err(e) => {
                                println!("h {h} {tag} n{n} {rule:?}: ERROR {e}");
                                continue;
                            }
                        };
                        let lo = rep.layers.clone().unwrap_or_default();
                        let q = check_quality(&m);
                        println!(
                            "h {:.0} {tag} n{n} {rule:?}: {} cells, {:.1} s, cover {:.3}, collapsed pts {:.3}, mean total {:.3} mm, NO max {:.1}, minDet {:.1e}, neg {}, fp {}",
                            h * 1e3,
                            m.cell_count(),
                            t0.elapsed().as_secs_f64(),
                            lo.full_coverage(),
                            lo.collapsed_point_fraction(),
                            1e3 * lo.mean_total_thickness(),
                            q.max_non_orthogonality_deg,
                            q.min_cell_determinant,
                            q.n_negative_volume_cells,
                            mesh_fingerprint(&m)
                        );
                        for note in lo.notes.iter().filter(|s| s.starts_with("iteration")).take(8) {
                            println!("    {note}");
                        }
                        if let Some(last) = lo.notes.last() {
                            println!("    {last}");
                        }
                    }
                }
            }
        }
    }

    /// V&V (gh:#1241 item 1) — the `correct_edges` stage in the pipeline.
    /// Methodology: the demo pipe shortened to L 60 mm (R 10 mm, 48 facets),
    /// h 6 mm, `MedianPerPoint` with `Corners`, bare, with and without
    /// `correct_edges`. Expected (from the L 200 mm runs of
    /// `diagnose_corners_rim`, 2026-11-05): the stage runs and says what it
    /// did, splits boundary faces and decomposes their cells into pyramids
    /// (more cells), and the result is a closed mesh with no inverted cell;
    /// with the option off the mesh is the one it always was (no note).
    #[test]
    fn correct_edges_runs_in_the_pipeline_and_is_off_by_default() {
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let o = TetDualOptions { cell_size: 0.006, n_layers: 0, dual_kind: DualKind::MedianPerPoint, dual_feature_points: FeaturePointRule::Corners, ..Default::default() };
        let (bare, rep0) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        assert!(rep0.stage_notes.iter().all(|n| !n.starts_with("correctEdgesBetweenPatches")));
        let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &TetDualOptions { correct_edges: true, ..o }).unwrap();
        let note = rep.stage_notes.iter().find(|n| n.starts_with("correctEdgesBetweenPatches:")).expect("the stage reports");
        println!("{} -> {} cells; {:?}", bare.cell_count(), m.cell_count(), rep.stage_notes);
        assert!(m.cell_count() >= bare.cell_count(), "{note}");
        m.validate().unwrap();
        assert_eq!(check_quality(&m).n_negative_volume_cells, 0);
    }

    /// V&V (gh:#1241 item 5) — **the octree path is pinned bit for bit
    /// across processes.** Methodology: `refine_near_boundary_banded` (the
    /// `refinement_levels > 0` carve) of a 1 m box at 0.25 m with 2 levels,
    /// and the multipatch pipeline on the demo pipe (R 10 mm, L 60 mm,
    /// 48 facets) at h 6 mm with `refinement_levels = 1`, bare.
    /// Fingerprints recorded 2026-11-05 in one process and asserted here, so
    /// any other process must reproduce them. Prediction (written before the
    /// fix): before it, three processes give three different fingerprints
    /// (leaf cells numbered in `HashSet` order); after it, one. Results
    /// (2026-11-05): before, three processes gave three fingerprints of
    /// each mesh (2 528 and 664 cells every time: the leaf set is the same,
    /// its numbering is not); after (`BTreeSet` / `BTreeMap`), three
    /// processes gave the pins below.
    #[test]
    fn octree_path_is_pinned_bit_for_bit() {
        let (bp, bt) = crate::shapes::box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let oct = refine_near_boundary_banded(&bp, &bt, 0.25, 2, 1.0);
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let o = TetDualOptions { cell_size: 0.006, refinement_levels: 1, n_layers: 0, ..Default::default() };
        let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        let got = (oct.cell_count(), mesh_fingerprint(&oct), m.cell_count(), mesh_fingerprint(&m));
        println!("octree fingerprints {got:?}");
        assert_eq!(got, OCTREE_PINS);
    }
    /// gh:#1285: the octree mesh itself (`refine_near_boundary_banded`) is
    /// not a pipeline output and kept its pin; the pipeline's pipe moved
    /// with the face order (~~24335298658873933~~).
    const OCTREE_PINS: (usize, u64, usize, u64) = (2528, 16070505555120789497, 664, 2018287953755369937);

    /// Feature edges of `m`'s boundary (edges between two boundary faces in
    /// different patches), the number at each boundary point, and the
    /// boundary faces cfMesh's `correctEdgesBetweenPatches::patchCorrection`
    /// would decompose: a face with two consecutive feature edges meeting
    /// at an edge point (a point with exactly two feature edges,
    /// `meshSurfacePartitioner`'s rule).
    fn patch_correction_candidates(m: &VolumeMesh) -> (usize, Vec<usize>) {
        let patch_of = |f: usize| m.patches.iter().position(|q| f >= q.start_face && f < q.start_face + q.n_faces);
        let mut edge_faces: std::collections::BTreeMap<(usize, usize), Vec<usize>> = Default::default();
        for f in 0..m.face_count() {
            if m.neighbour[f].is_none() {
                let ring = &m.faces[f];
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    edge_faces.entry((a.min(b), a.max(b))).or_default().push(f);
                }
            }
        }
        let feature: std::collections::BTreeSet<(usize, usize)> =
            edge_faces.iter().filter(|(_, fs)| fs.len() == 2 && patch_of(fs[0]) != patch_of(fs[1])).map(|(&e, _)| e).collect();
        let mut n_at: std::collections::BTreeMap<usize, usize> = Default::default();
        for &(a, b) in &feature {
            *n_at.entry(a).or_default() += 1;
            *n_at.entry(b).or_default() += 1;
        }
        let is_f = |a: usize, b: usize| feature.contains(&(a.min(b), a.max(b)));
        let mut faces = Vec::new();
        for f in 0..m.face_count() {
            if m.neighbour[f].is_none() {
                let ring = &m.faces[f];
                let k = ring.len();
                if (0..k).any(|i| n_at.get(&ring[i]) == Some(&2) && is_f(ring[i], ring[(i + 1) % k]) && is_f(ring[(i + k - 1) % k], ring[i])) {
                    faces.push(f);
                }
            }
        }
        (feature.len(), faces)
    }

    /// Diagnostic (gh:#1241 item 1), `--ignored`: where the `Corners` rim's
    /// inlet area goes. For each h and each per-point kind with `Corners`,
    /// the inlet patch with and without the feature-edge capture: face
    /// count, Σ|S_f| and |Σ S_f·ẑ| against πR², the inlet points' radius and
    /// off-plane distance, the feature edges, and the boundary faces cfMesh's
    /// `correctEdgesBetweenPatches::patchCorrection` would decompose.
    #[test]
    #[ignore]
    fn diagnose_corners_rim() {
        let (r, l) = (0.01, 0.2);
        let (p, t, regions) = pipe_surface(r, l, 48);
        let section = PI * r * r;
        let hs: Vec<f64> = std::env::var("CFMESH_HS").ok().map_or(vec![0.004, 0.005, 0.006], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
        for h in hs {
            for kind in [DualKind::MedianPerPoint, DualKind::CellCentre] {
                for (fe, ce) in [(false, false), (true, false), (true, true)] {
                    let o = TetDualOptions { cell_size: h, n_layers: 0, dual_kind: kind, dual_feature_points: FeaturePointRule::Corners, feature_edges: fe, correct_edges: ce, ..Default::default() };
                    let t0 = std::time::Instant::now();
                    let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
                    if !fe && std::env::var("CFMESH_STEPS").is_ok() {
                        // The correction path stage by stage, from the bare (uncaptured) mesh.
                        let st = |x: &VolumeMesh| {
                            let q = check_quality(x);
                            format!("closed {:?} neg {} negPyr {}", x.validate().err().map(|e| e.chars().take(60).collect::<String>()), q.n_negative_volume_cells, crate::surface_mapper::cells_with_nonpositive_pyramids(x).len())
                        };
                        let (pre, n_pre) = crate::surface_mapper::map_vertices_onto_surface_untangled(&m, &p, &t, 10);
                        let given = crate::surface_mapper::boundary_face_regions(&pre, &p, &t, &regions);
                        let (cor, given, crep) = crate::correct_edges::correct_edges_between_patches(&pre, &given, &regions.names);
                        let (mut mapped, nrev) = crate::surface_mapper::map_vertices_onto_surface_patches_untangled_given(&cor, &p, &t, &regions, 10, &given);
                        let bad: Vec<usize> = crate::surface_mapper::cells_with_nonpositive_pyramids(&mapped).into_iter().collect();
                        let (vols, cen) = crate::checks::cell_geometry(&mapped);
                        for &c in bad.iter().take(6) {
                            let nf = (0..mapped.face_count()).filter(|&f| mapped.owner[f] == c || mapped.neighbour[f] == Some(c)).count();
                            let nb = (0..mapped.face_count()).filter(|&f| mapped.owner[f] == c && mapped.neighbour[f].is_none()).count();
                            println!("    bad cell {c} (pyramid {}): {nf} faces, {nb} boundary, vol {:.2e}, centre r {:.2} z {:.2} mm", c >= mapped.n_cells - 1000.min(mapped.n_cells), vols[c], (cen[c].x.hypot(cen[c].y)) * 1e3, cen[c].z * 1e3);
                        }
                        let locked = vec![false; mapped.point_count()];
                        let r = crate::mesh_optimizer::untangle_mesh_fv(&mut mapped, &locked, 10, 50, 2, false);
                        println!("    untangleMeshFV {r:?}: {}", st(&mapped));
                        println!("  steps h {:.0} {kind:?}: pre-map ({n_pre} kept) {}; corrected {crep:?} {}; mapped ({nrev} reverted) {}", h * 1e3, st(&pre), st(&cor), st(&mapped));
                    }
                    let inlet = patch_faces(&m, "inlet");
                    let a_abs: f64 = inlet.iter().map(|&f| m.face_area_vector(f).length()).sum();
                    let a_z: f64 = inlet.iter().map(|&f| m.face_area_vector(f).z).sum::<f64>().abs();
                    let pts: std::collections::BTreeSet<usize> = inlet.iter().flat_map(|&f| m.faces[f].iter().copied()).collect();
                    let rad = |v: usize| (m.points[v].x.powi(2) + m.points[v].y.powi(2)).sqrt();
                    let rmax = pts.iter().map(|&v| rad(v)).fold(0.0, f64::max);
                    let zmax = pts.iter().map(|&v| m.points[v].z.abs()).fold(0.0, f64::max);
                    let n_off = pts.iter().filter(|&&v| m.points[v].z.abs() > 1e-9).count();
                    let (n_feat, cand) = patch_correction_candidates(&m);
                    let cand_in: usize = cand.iter().filter(|&&f| inlet.contains(&f)).count();
                    let wall = patch_faces(&m, "walls");
                    // Wall faces touching z = 0 and their projected area on the inlet plane.
                    let wall_low: Vec<usize> = wall.iter().copied().filter(|&f| m.face_centroid(f).z < 0.5 * h).collect();
                    let wall_low_z: f64 = wall_low.iter().map(|&f| m.face_area_vector(f).z).sum();
                    println!(
                        "h {:.0} {kind:?} capture {fe} correct {ce} ({:.1} s): {} cells, valid {}, NO max {:.1}, fp {}; V/piR2L {:.5}; inlet {} faces, sum|S|/piR2 {:.5}, |sum Sz|/piR2 {:.5}; inlet pts {} (off plane {n_off}, max |z| {:.3} mm), max r {:.4} mm; \
                         feature edges {n_feat}; patchCorrection candidates {} ({cand_in} inlet); wall faces near inlet {} with sum Sz/piR2 {:.5}; notes {:?}",
                        h * 1e3, t0.elapsed().as_secs_f64(), m.cell_count(), m.validate().is_ok(), check_quality(&m).max_non_orthogonality_deg, mesh_fingerprint(&m), m.total_volume() / (section * l), inlet.len(), a_abs / section, a_z / section, pts.len(), zmax * 1e3, rmax * 1e3,
                        cand.len(), wall_low.len(), wall_low_z / section, rep.stage_notes
                    );
                    {
                        // The inlet's rim: inlet-face edges not shared with another inlet face.
                        let mut cnt: std::collections::BTreeMap<(usize, usize), usize> = Default::default();
                        for &f in &inlet {
                            let ring = &m.faces[f];
                            for i in 0..ring.len() {
                                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                                *cnt.entry((a.min(b), a.max(b))).or_default() += 1;
                            }
                        }
                        let rim: Vec<(usize, usize)> = cnt.iter().filter(|(_, &c)| c == 1).map(|(&e, _)| e).collect();
                        let rim_pts: std::collections::BTreeSet<usize> = rim.iter().flat_map(|&(a, b)| [a, b]).collect();
                        let shoelace: f64 = rim.iter().map(|&(a, b)| 0.5 * (m.points[a].x * m.points[b].y - m.points[b].x * m.points[a].y)).sum::<f64>().abs();
                        let rs: Vec<f64> = rim_pts.iter().map(|&v| rad(v)).collect();
                        let rmean = rs.iter().sum::<f64>() / rs.len() as f64;
                        let rmin = rs.iter().cloned().fold(f64::MAX, f64::min);
                        let near_corner = rim_pts.iter().filter(|&&v| {
                            let a = m.points[v].y.atan2(m.points[v].x).rem_euclid(2.0 * PI / 48.0);
                            a.min(2.0 * PI / 48.0 - a) < 1e-3
                        }).count();
                        // shoelace of rim edges, unordered: only valid when the rim is one loop.
                        println!(
                            "    rim: {} edges, {} points (at a 48-gon corner {near_corner}), r mean {:.4} min {:.4} mm; |sum over rim edges of the cross product|/2 /piR2 {:.5} (48-gon {:.5})",
                            rim.len(), rim_pts.len(), rmean * 1e3, rmin * 1e3, shoelace / section, 48.0 * (2.0 * PI / 48.0).sin() / (2.0 * PI)
                        );
                    }
                    if std::env::var("CFMESH_VERBOSE").is_ok() && fe && ce && kind == DualKind::MedianPerPoint {
                        let desc = |f: usize| -> String {
                            let s = m.face_area_vector(f);
                            let pts: Vec<String> = m.faces[f].iter().map(|&v| format!("({:.2},{:.2})", rad(v) * 1e3, m.points[v].z * 1e3)).collect();
                            format!("face {f} {} n {} |S| {:.3} mm2 nz {:.2} cand {} pts(r,z mm) {}", if inlet.contains(&f) { "inlet" } else if wall.contains(&f) { "walls" } else { "other" }, m.faces[f].len(), s.length() * 1e6, s.z / s.length(), cand.contains(&f), pts.join(" "))
                        };
                        for &f in &wall_low {
                            let s = m.face_area_vector(f);
                            if s.z / s.length() < -0.3 || cand.contains(&f) {
                                println!("  {}", desc(f));
                            }
                        }
                        for &f in &cand {
                            if inlet.contains(&f) {
                                println!("  {}", desc(f));
                            }
                        }
                    }
                }
            }
        }
    }
}
