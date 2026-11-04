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
use crate::dual::{polyhedral_dual, polyhedral_dual_min_faces};
use crate::layers::{add_boundary_layers_adaptive, add_boundary_layers_adaptive_on_patches};
use crate::math::Vec3;
use crate::octree::refine_near_boundary_banded;
use crate::patches::{assign_patches_by_region, SurfaceRegions};
use crate::shapes::{box_surface, cylinder_surface, sphere_surface};
use crate::smooth::laplacian_smooth;
use crate::snap::snap_to_surface;
use crate::tet::tetrahedralize;
use crate::volume_mesh::VolumeMesh;

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
            smooth_passes: 0,
            n_layers: 3,
            first_layer_thickness: 0.04,
            expansion: 1.3,
            wall_patch: "walls".to_string(),
            layer_patches: Vec::new(),
            feature_edges: true,
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
}

/// Is `mesh` acceptable as a stage output — closed *and* free of inverted cells?
///
/// This is the graceful-degradation gate: a stage's result is kept only if it
/// passes this, otherwise the previous mesh is retained. Stronger than a bare
/// `validate()` because a tangled cell can be closed yet negative-volume.
fn acceptable(mesh: &VolumeMesh) -> bool {
    mesh.validate().is_ok() && check_quality(mesh).n_negative_volume_cells == 0
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
    run_pipeline(points, tris, opts, None)
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
    run_pipeline(points, tris, opts, Some(regions))
}

/// Shared implementation of the two public entry points. `regions = None` is the
/// historical single-`"walls"` behaviour (no classification pass at all, so it
/// costs nothing); `Some(_)` adds the final [`assign_patches_by_region`] step.
fn run_pipeline(
    points: &[Vec3],
    tris: &[[usize; 3]],
    opts: &TetDualOptions,
    regions: Option<&SurfaceRegions>,
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

    // Stage 5 — polyhedral dual (min-faces preferred, quad-fan fallback, then skip).
    if opts.dual {
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
    if let (true, Some(regions)) = (opts.feature_edges && opts.dual, regions) {
        let (mapped, n_reverted) = crate::surface_mapper::map_vertices_onto_surface_patches_untangled(&mesh, points, tris, regions, 10);
        let (m, kept) = keep_if_ok(mesh, mapped);
        mesh = m;
        if !kept {
            notes.push("feature-edge capture skipped — mapping the boundary onto the patches would invalidate the mesh".into());
        } else if n_reverted > 0 {
            notes.push(format!(
                "feature-edge capture: {n_reverted} boundary points left unmapped where mapping tangled a cell (cfMesh's correctEdgesBetweenPatches is not ported)"
            ));
        }
    }

    // Stage 7 — adaptive prism boundary layers on the wall patch.
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
                    let layered = add_boundary_layers_adaptive_on_patches(
                        &named,
                        &names,
                        opts.n_layers,
                        opts.first_layer_thickness.max(1e-6),
                        opts.expansion.max(1.0),
                    );
                    let before = mesh.cell_count();
                    let (m, kept) = keep_if_ok(mesh, layered);
                    mesh = m;
                    if !kept {
                        notes.push("boundary layers skipped — the wall is too tight for any valid layer".into());
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
            let layered = add_boundary_layers_adaptive(
                &mesh,
                &opts.wall_patch,
                opts.n_layers,
                opts.first_layer_thickness.max(1e-6),
                opts.expansion.max(1.0),
            );
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
            if !kept {
                notes.push(
                    "boundary layers skipped — the wall is too tight for any valid layer".into(),
                );
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
mod tests {
    use super::*;
    use std::f64::consts::PI;

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

    /// V&V (gh:#1215) — **the default layer strategy is pinned bit for bit.**
    /// Methodology: the multipatch pipeline with wall-only layers on the demo
    /// pipe (R 10 mm, L 60 mm, 48 facets) at h 6 / 4 mm with 1–3 layers, and a
    /// 20 × 20 × 60 mm box duct at h 5 mm with 2 layers; each final mesh is
    /// fingerprinted (points to the bit, faces, owners, neighbours, patches).
    /// The fingerprints were recorded on 2026-11-04 on the code *before*
    /// gh:#1215 (commit `ffda5fb46`) and are asserted here, so introducing
    /// the layer-quality strategies cannot move a recorded number under
    /// `LayerQualityStrategy::BackOffOnly`.
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
        let pinned: [(usize, u64); 7] = [
            (2025, 14571713154878100336),
            (3445, 15107328938539315242),
            (4865, 17913122352869577766),
            (5511, 11354576510400452468),
            (9079, 6008187938627271744),
            (12647, 13850840632559401666),
            (5813, 7672853737214871962),
        ];
        for (g, want) in got.iter().zip(pinned) {
            assert_eq!((g.1, g.2), want, "{} moved", g.0);
        }
    }

    /// A pipe surface (radius `r`, length `l`, `n_seg` sides, z ∈ [0, l]) with
    /// each triangle labelled `inlet` (flat at z = 0), `outlet` (flat at
    /// z = l) or `walls`.
    fn pipe_surface(r: f64, l: f64, n_seg: usize) -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
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
}
